//! clap CLI: argument parsing, command dispatch, output formatting, exit codes.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::gitinfo::{self, Evidence};
use crate::identity::{self, ProcInfo};
use crate::model::{self, Derived, Facts, Kind, Status};
use crate::prime::{self, Handoff, PlanState, PrimeClaim, PrimeInput};
use crate::store::{
    Actor, Backend, ClaimRequest, ClaimRow, Command, Ctx, FbError, Location, NewTask, ProjectSel,
    Result, SqliteBackend, TaskDetail, TaskFilter, TaskRow, TicketSpec, fmt_id, parse_id,
};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};

use crate::config::{self, Config};

#[derive(Parser)]
#[command(
    name = "fb",
    version,
    about = "Task board for one person running many coding agents"
)]
pub struct Cli {
    /// Print one JSON value on stdout and nothing else
    #[arg(long, global = true)]
    pub json: bool,
    /// Act as the human (refused when agent markers are present)
    #[arg(long = "as-human", global = true)]
    pub as_human: bool,
    /// Data directory (default: $FB_HOME, else ~/.fleetboard)
    #[arg(long, global = true, value_name = "DIR")]
    pub home: Option<PathBuf>,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Create a task (agents: draft; human or trusted: todo)
    Add {
        #[arg(allow_hyphen_values = true)]
        title: String,
        #[arg(long, allow_hyphen_values = true)]
        goal: Option<String>,
        /// Acceptance criterion (repeatable)
        #[arg(long = "ac", allow_hyphen_values = true)]
        ac: Vec<String>,
        /// code (default) or note
        #[arg(long, default_value = "code")]
        kind: String,
        /// Priority 0 (highest) to 3
        #[arg(long, default_value_t = 2)]
        prio: i64,
        #[arg(long = "blocked-by", value_name = "ID")]
        blocked_by: Vec<String>,
        /// External parent as system:key, e.g. jira:ABC-36
        #[arg(long, value_name = "SYSTEM:KEY")]
        ticket: Option<String>,
        #[arg(long = "ticket-title", allow_hyphen_values = true)]
        ticket_title: Option<String>,
        #[arg(long = "ticket-url")]
        ticket_url: Option<String>,
        /// Project key or alias (default: detected from git)
        #[arg(long)]
        project: Option<String>,
        /// Hold the task in a plan-pending state until the human approves
        #[arg(long = "plan-approval")]
        plan_approval: bool,
    },
    /// List Ready tasks
    Ready {
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Claim a task (no id: the best Ready task in this project)
    Claim {
        id: Option<String>,
        /// Take over an active claim (human or trusted only)
        #[arg(long)]
        steal: bool,
        /// With --steal: hand the claim to this actor
        #[arg(long = "for", value_name = "ACTOR")]
        for_actor: Option<String>,
    },
    /// Write or replace the plan ("-" reads stdin)
    Plan {
        id: String,
        #[arg(allow_hyphen_values = true)]
        text: String,
    },
    /// Add a note ("-" reads stdin)
    Note {
        id: String,
        #[arg(allow_hyphen_values = true)]
        text: String,
        /// handoff | comment | finding
        #[arg(long, default_value = "handoff")]
        kind: String,
    },
    /// Finish: move to review
    Done {
        id: String,
        #[arg(long, allow_hyphen_values = true)]
        result: Option<String>,
    },
    /// Show a task in full
    Show {
        id: String,
        #[arg(long)]
        evidence: bool,
    },
    /// Filtered task list
    List {
        #[arg(long)]
        status: Vec<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long = "needs-me")]
        needs_me: bool,
        /// Include done and dropped tasks
        #[arg(long)]
        all: bool,
        #[arg(long)]
        evidence: bool,
    },
    /// Workflow guide plus your active claim and its latest handoff
    Prime,
    /// Print caller identity, host, project and instance
    Whoami,
    /// Edit dependencies
    Dep {
        #[command(subcommand)]
        cmd: DepCmd,
    },
    /// (human) draft -> todo
    Promote { id: String },
    /// (human) approve the plan
    Approve { id: String },
    /// (human) review -> done
    Verify {
        id: String,
        #[arg(long)]
        force: bool,
    },
    /// (human) review -> in_progress with a comment
    Reject {
        id: String,
        #[arg(allow_hyphen_values = true)]
        comment: String,
    },
    /// (human) release the active claim; task -> todo
    Release { id: String },
    /// (human) drop a task
    Drop {
        id: String,
        #[arg(long, allow_hyphen_values = true)]
        reason: Option<String>,
    },
    /// (human) reopen a done or dropped task
    Reopen { id: String },
    /// (human) grant rights to act on human behalf
    Delegate {
        actor: String,
        /// any of promote, approve, verify, reject, release, steal, drop, reopen
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        allow: Vec<String>,
    },
    /// (human) revoke rights to act on human behalf
    Undelegate {
        actor: String,
        /// specific rights to revoke; if omitted, revokes all
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        allow: Option<Vec<String>>,
    },
    /// (human) grant the tracker role
    Trust { actor: String },
    /// (human) remove the tracker role
    Untrust { actor: String },
    /// Project operations
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Start the loopback HTTP server and web UI
    Serve {
        /// Port to listen on (default: 7878)
        #[arg(long, default_value_t = 7878)]
        port: u16,
    },
    /// Start the stdio Model Context Protocol (MCP) server
    Mcp,
    /// Record agent lifecycle hook event (session-start | stop | tool-use | prompt)
    Hook {
        /// session-start | stop | tool-use | prompt
        event: String,
    },
}

#[derive(Subcommand)]
pub enum DepCmd {
    /// task depends on blocker
    Add {
        task: String,
        blocker: String,
    },
    Rm {
        task: String,
        blocker: String,
    },
}

#[derive(Subcommand)]
pub enum ProjectCmd {
    /// (human) rename a project for display
    Alias {
        key: String,
        #[arg(allow_hyphen_values = true)]
        alias: String,
    },
}

/// What a command returns: the JSON `data` and the plain-text rendering.
pub struct Out {
    pub data: Value,
    pub text: String,
}

pub fn run(args: Vec<OsString>) -> i32 {
    let is_hook = args.iter().any(|a| a == "hook");
    let json_flag = args.iter().any(|a| a == "--json");
    let cli = match Cli::try_parse_from(&args) {
        Ok(c) => c,
        Err(e) => {
            if is_hook {
                eprintln!("{e}");
                return 0;
            }
            if !e.use_stderr() {
                let _ = e.print();
                return 0;
            }
            if json_flag {
                let msg = e.to_string();
                emit_error(true, &FbError::Usage(msg.trim().to_string()));
            } else {
                let _ = e.print();
            }
            return 1;
        }
    };
    match execute(&cli) {
        Ok(out) => {
            if matches!(cli.cmd, Cmd::Mcp) {
                return 0;
            }
            if cli.json {
                println!("{}", json!({ "v": 1, "ok": true, "data": out.data }));
            } else if !out.text.is_empty() {
                print!("{}", out.text);
                if !out.text.ends_with('\n') {
                    println!();
                }
            }
            0
        }
        Err(e) => {
            if is_hook {
                eprintln!("{e}");
                return 0;
            }
            if matches!(cli.cmd, Cmd::Mcp) {
                eprintln!("fb mcp: {}: {e}", e.code());
            } else {
                emit_error(cli.json, &e);
            }
            e.exit_code()
        }
    }
}

fn emit_error(json: bool, e: &FbError) {
    if json {
        println!(
            "{}",
            json!({ "v": 1, "ok": false, "error": { "code": e.code(), "message": e.to_string() } })
        );
    } else {
        eprintln!("fb: {}: {e}", e.code());
    }
}

/// Everything a command needs about this invocation.
pub struct Session {
    pub db: SqliteBackend,
    pub cfg: Config,
    pub actor: Actor,
    pub ctx: Ctx,
    pub host: String,
    pub cwd: PathBuf,
    /// Present only inside a git repo.
    pub loc: Option<Location>,
    pub adapter: std::sync::Arc<dyn crate::adapters::RuntimeAdapter>,
}

pub fn fb_home_path(home: Option<&Path>) -> Result<PathBuf> {
    if let Some(h) = home {
        return Ok(h.to_path_buf());
    }
    if let Some(h) = std::env::var_os("FB_HOME").filter(|h| !h.is_empty()) {
        return Ok(PathBuf::from(h));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| FbError::Usage("cannot find a home directory; set FB_HOME".into()))?;
    Ok(PathBuf::from(home).join(".fleetboard"))
}

pub fn fb_home(cli: &Cli) -> Result<PathBuf> {
    fb_home_path(cli.home.as_deref())
}

/// Process ancestry. Debug builds accept `FB_TEST_CHAIN` (comma-separated names,
/// possibly empty) so tests do not depend on the agent running the test suite.
pub fn proc_chain() -> Vec<ProcInfo> {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("FB_TEST_CHAIN") {
        return v
            .split(',')
            .filter(|n| !n.is_empty())
            .map(|n| ProcInfo {
                pid: 0,
                name: n.to_string(),
            })
            .collect();
    }
    identity::process_chain()
}

/// Current time. Debug builds accept `FB_TEST_NOW` (RFC 3339) so tests can move the clock.
pub fn now() -> DateTime<Utc> {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("FB_TEST_NOW")
        && let Ok(t) = DateTime::parse_from_rfc3339(&v)
    {
        return t.with_timezone(&Utc);
    }
    Utc::now()
}

pub fn create_session(home: &Path, as_human: bool) -> Result<Session> {
    let cfg = config::load(home)?;
    let adapter = crate::adapters::make_adapter(&cfg.adapter, Some(home), Some(cfg.stale_after));
    let db = SqliteBackend::open(&home.join("board.db"))?;

    let chain = proc_chain();
    let env = |k: &str| std::env::var(k).ok();
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let cwd = std::env::current_dir()?;
    let caller = if as_human {
        identity::resolve_human(&env, &chain).map_err(|e| FbError::HumanOnly(e.to_string()))?
    } else {
        identity::resolve_agent(&env, &chain, &host, &cwd)
    };
    let ctx = Ctx {
        now: now(),
        pid: std::process::id(),
        ppid: std::os::unix::process::parent_id(),
        proc_chain: chain
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(" < "),
        source: None,
    };
    let actor = db.resolve_actor(&caller, &ctx)?;
    let loc = gitinfo::instance(&cwd).map(|i| Location {
        host: host.clone(),
        cwd: i.toplevel.to_string_lossy().into_owned(),
        branch: i.branch,
        head: i.head,
    });
    db.touch(&actor, loc.as_ref(), &ctx)?;
    Ok(Session {
        db,
        cfg,
        actor,
        ctx,
        host,
        cwd,
        loc,
        adapter,
    })
}

fn serve_cmd(cli: &Cli, home: &Path, port: u16) -> Result<Out> {
    let token = crate::server::auth::generate_random_token();
    let bind_addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| FbError::Store(format!("failed to start tokio runtime: {e}")))?;

    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|e| FbError::Store(format!("failed to bind to {bind_addr}: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| FbError::Store(format!("failed to get local address: {e}")))?;

        if cli.json {
            println!(
                "{}",
                json!({
                    "v": 1,
                    "ok": true,
                    "data": {
                        "url": format!("http://{local_addr}"),
                        "port": local_addr.port(),
                        "token": token,
                    }
                })
            );
        } else {
            println!("fleetboard serving at http://{local_addr}");
            println!("One-time login token: {token}");
        }

        let config = crate::server::ServerConfig {
            addr: local_addr,
            token,
            home: home.to_path_buf(),
        };
        let state = crate::server::AppState::new(config)
            .map_err(|e| FbError::Store(format!("failed to initialize server state: {e}")))?;

        crate::server::run_server(listener, state)
            .await
            .map_err(|e| FbError::Store(format!("server error: {e}")))
    })?;

    Ok(Out {
        data: Value::Null,
        text: String::new(),
    })
}

fn is_claude_payload(p: &crate::adapters::hooks::HookStdinPayload) -> bool {
    p.source.is_some() || p.transcript_path.is_some() || p.permission_mode.is_some()
}

fn is_claude_caller(
    caller: &identity::Caller,
    payload: Option<&crate::adapters::hooks::HookStdinPayload>,
) -> bool {
    if caller.provider.as_deref() == Some("claude") {
        return true;
    }
    if std::env::var("CLAUDECODE").is_ok() || std::env::var("CLAUDE_CODE_SESSION_ID").is_ok() {
        return true;
    }
    if let Some(p) = payload {
        if let Some(ref prov) = p.provider
            && prov.to_ascii_lowercase().contains("claude")
        {
            return true;
        }
        if is_claude_payload(p) {
            return true;
        }
    }
    false
}

fn fallback_hook_out(
    cli: &Cli,
    hook_event: Option<crate::adapters::hooks::HookEvent>,
    is_claude: bool,
    caller: &identity::Caller,
    host: &str,
) -> Out {
    if hook_event == Some(crate::adapters::hooks::HookEvent::SessionStart) {
        let prime_text = format!(
            "fleetboard — you are {} on host {}\nYou have no active claim.\nNo claim? fb ready, then fb claim <id>. New work you find: fb add \"…\" (lands as a draft).\n",
            caller.label, host
        );
        if is_claude {
            let json_out = json!({
                "hookSpecificOutput": {
                    "hookEventName": "SessionStart",
                    "additionalContext": prime_text
                }
            });
            let rendered = serde_json::to_string(&json_out).unwrap_or_default();
            Out {
                data: json_out,
                text: rendered,
            }
        } else {
            Out {
                data: json!({ "prime": prime_text }),
                text: prime_text,
            }
        }
    } else if cli.json {
        Out {
            data: json!({ "recorded": false }),
            text: String::new(),
        }
    } else {
        Out {
            data: Value::Null,
            text: String::new(),
        }
    }
}

fn build_prime_text(
    db: &SqliteBackend,
    actor: &Actor,
    host: &str,
    cwd: &Path,
    ctx: &Ctx,
) -> String {
    let project = gitinfo::project_key(cwd);
    let tasks = db
        .list_tasks(&TaskFilter {
            claimant: Some(actor.id),
            ..Default::default()
        })
        .unwrap_or_default();
    let mut claims = Vec::new();
    for t in &tasks {
        let detail = db.task_detail(t.id).ok();
        let handoff = detail.as_ref().and_then(|d| {
            d.notes
                .iter()
                .rev()
                .find(|n| n.kind == "handoff")
                .map(|n| Handoff {
                    author: n.actor.label.clone(),
                    age: ctx.now - n.created_at,
                    body: n.body.clone(),
                })
        });
        let plan = match (t.needs_plan_approval, t.plan_approved_at.is_some()) {
            (false, _) => PlanState::NotRequired,
            (true, true) => PlanState::Approved,
            (true, false) => PlanState::Pending,
        };
        claims.push(PrimeClaim {
            id: fmt_id(t.id),
            title: t.title.clone(),
            status: t.status.as_str().to_string(),
            plan,
            handoff,
        });
    }
    prime::render(&PrimeInput {
        label: actor.label.clone(),
        host: host.to_string(),
        project,
        claims,
    })
}

fn hook_cmd(cli: &Cli, home_res: Result<PathBuf>, event_arg: &str) -> Result<Out> {
    use crate::adapters::hooks::{HookEvent, HookStdinPayload};
    use std::io::IsTerminal;

    let hook_event = HookEvent::parse(event_arg);
    if hook_event.is_none() {
        eprintln!("warning: unrecognized hook event: {event_arg:?}");
    }

    let mut stdin_str = String::new();
    if !std::io::stdin().is_terminal() {
        let _ = std::io::stdin().read_to_string(&mut stdin_str);
    }

    let payload: Option<HookStdinPayload> = if stdin_str.trim().is_empty() {
        None
    } else {
        match serde_json::from_str::<HookStdinPayload>(&stdin_str) {
            Ok(p) => Some(p),
            Err(err) => {
                eprintln!("warning: failed to parse hook stdin JSON: {err}");
                None
            }
        }
    };

    let chain = proc_chain();
    let host = gethostname::gethostname().to_string_lossy().into_owned();

    let cwd = payload
        .as_ref()
        .and_then(|p| p.cwd.as_deref())
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let env = |k: &str| -> Option<String> {
        if let Ok(v) = std::env::var(k) {
            return Some(v);
        }
        if let Some(ref p) = payload {
            if let Some(ref sid) = p.session_id
                && k == "CLAUDE_CODE_SESSION_ID"
            {
                return Some(sid.clone());
            }
            if let Some(ref prov) = p.provider
                && k == "AI_AGENT"
            {
                return Some(prov.clone());
            }
        }
        None
    };

    let mut caller = identity::resolve_agent(&env, &chain, &host, &cwd);
    if let Some(ref p) = payload
        && let Some(ref sid) = p.session_id
        && (caller.external_id.starts_with("anon:")
            || caller.provider.as_deref() == Some("unknown"))
    {
        let prov = if let Some(ref p_prov) = p.provider {
            identity::normalize_provider(p_prov).to_string()
        } else if is_claude_payload(p) {
            "claude".to_string()
        } else if let Some(ref cp) = caller.provider {
            if cp != "unknown" {
                cp.clone()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };
        let short_sid: String = sid.chars().take(7).collect();
        caller.label = format!("{prov} {short_sid}");
        caller.provider = Some(prov);
        caller.external_id = sid.clone();
    }

    let is_claude = is_claude_caller(&caller, payload.as_ref());

    let home = match home_res {
        Ok(h) => h,
        Err(e) => {
            eprintln!("warning: fleetboard board database not found: {e}");
            return Ok(fallback_hook_out(
                cli, hook_event, is_claude, &caller, &host,
            ));
        }
    };

    let db_path = home.join("board.db");
    if !db_path.exists() {
        eprintln!(
            "warning: fleetboard database not found at {}",
            db_path.display()
        );
        return Ok(fallback_hook_out(
            cli, hook_event, is_claude, &caller, &host,
        ));
    }

    if let Err(e) = std::fs::create_dir_all(&home) {
        eprintln!("warning: failed to create fleetboard home dir: {e}");
        return Ok(fallback_hook_out(
            cli, hook_event, is_claude, &caller, &host,
        ));
    }

    let db = match SqliteBackend::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!(
                "warning: could not open fleetboard database at {}: {e}",
                db_path.display()
            );
            return Ok(fallback_hook_out(
                cli, hook_event, is_claude, &caller, &host,
            ));
        }
    };

    let ctx = Ctx {
        now: now(),
        pid: std::process::id(),
        ppid: std::os::unix::process::parent_id(),
        proc_chain: chain
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(" < "),
        source: None,
    };

    let actor = match db.resolve_actor(&caller, &ctx) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("warning: failed to resolve actor: {e}");
            return Ok(fallback_hook_out(
                cli, hook_event, is_claude, &caller, &host,
            ));
        }
    };

    // Bump actor activity on any active claims
    let loc = gitinfo::instance(&cwd).map(|i| Location {
        host: host.clone(),
        cwd: i.toplevel.to_string_lossy().into_owned(),
        branch: i.branch,
        head: i.head,
    });
    let _ = db.touch(&actor, loc.as_ref(), &ctx);

    // Record the hook event
    let canonical_event = hook_event.map_or(event_arg, |e| e.as_str());
    let raw_val: Option<Value> = serde_json::from_str(&stdin_str).ok();
    if let Err(e) = db.record_hook_event(&actor, canonical_event, raw_val.as_ref(), &ctx) {
        eprintln!("warning: failed to record hook event: {e}");
    }

    if hook_event == Some(HookEvent::SessionStart) {
        let prime_text = build_prime_text(&db, &actor, &host, &cwd, &ctx);
        if is_claude {
            let json_out = json!({
                "hookSpecificOutput": {
                    "hookEventName": "SessionStart",
                    "additionalContext": prime_text
                }
            });
            let rendered = serde_json::to_string(&json_out).unwrap_or_default();
            Ok(Out {
                data: json_out,
                text: rendered,
            })
        } else {
            Ok(Out {
                data: json!({ "prime": prime_text }),
                text: prime_text,
            })
        }
    } else if cli.json {
        Ok(Out {
            data: json!({ "event": canonical_event, "recorded": true }),
            text: String::new(),
        })
    } else {
        Ok(Out {
            data: Value::Null,
            text: String::new(),
        })
    }
}

fn execute(cli: &Cli) -> Result<Out> {
    let home_res = fb_home(cli);
    if let Cmd::Hook { event } = &cli.cmd {
        return hook_cmd(cli, home_res, event);
    }
    let home = home_res?;
    std::fs::create_dir_all(&home)?;
    if let Cmd::Serve { port } = &cli.cmd {
        return serve_cmd(cli, &home, *port);
    }
    if let Cmd::Mcp = &cli.cmd {
        return crate::mcp::run_stdio_server(&home, cli.as_human);
    }
    let s = create_session(&home, cli.as_human)?;
    dispatch(&s, &cli.cmd)
}

pub fn dispatch(s: &Session, cmd: &Cmd) -> Result<Out> {
    match cmd {
        Cmd::Add {
            title,
            goal,
            ac,
            kind,
            prio,
            blocked_by,
            ticket,
            ticket_title,
            ticket_url,
            project,
            plan_approval,
        } => {
            let kind = Kind::parse(kind)
                .ok_or_else(|| FbError::Usage(format!("bad --kind {kind:?} (code | note)")))?;
            let ticket = parse_ticket(ticket.as_deref(), ticket_title, ticket_url)?;
            let project = match project {
                Some(p) => Some(ProjectSel::Explicit(p.clone())),
                None => gitinfo::project_key(&s.cwd).map(ProjectSel::Detected),
            };
            let acceptance = (!ac.is_empty()).then(|| {
                ac.iter()
                    .map(|a| {
                        let trimmed = a.trim();
                        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
                            format!("- [ ] {rest}")
                        } else if let Some(rest) = trimmed.strip_prefix("- [x] ") {
                            format!("- [x] {rest}")
                        } else if let Some(rest) = trimmed.strip_prefix("- ") {
                            format!("- [ ] {rest}")
                        } else {
                            format!("- [ ] {a}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            let new = NewTask {
                title: title.clone(),
                goal: goal.clone(),
                acceptance,
                kind,
                priority: *prio,
                blocked_by: blocked_by
                    .iter()
                    .map(|b| parse_id(b))
                    .collect::<Result<_>>()?,
                ticket,
                project,
                needs_plan_approval: *plan_approval,
            };
            let t = s.db.add_task(&s.actor, &new, &s.ctx)?;
            let text = format!(
                "created {} ({}): {}\n",
                fmt_id(t.id),
                t.status.as_str(),
                t.title
            );
            task_out(s, &t, text)
        }
        Cmd::Ready { project, all } => {
            let scope = match project {
                Some(p) => Scope::Project(find_project(s, p)?),
                None if *all => Scope::Any,
                None => detect_scope(s)?,
            };
            let tasks = match scope {
                Scope::Empty => vec![],
                Scope::Any => s.db.ready(None)?,
                Scope::Project(id) => s.db.ready(Some(id))?,
            };
            list_out(s, tasks, false)
        }
        Cmd::Claim {
            id,
            steal,
            for_actor,
        } => {
            let scope_project = if id.is_none() {
                match detect_scope(s)? {
                    Scope::Project(p) => Some(p),
                    Scope::Any => None,
                    Scope::Empty => {
                        return Err(FbError::NotFound("no ready task to claim".into()));
                    }
                }
            } else {
                None
            };
            let workspace_id = s
                .adapter
                .workspace_id(&s.actor.external_id, &s.cwd.to_string_lossy());
            let req = ClaimRequest {
                id: id.as_deref().map(parse_id).transpose()?,
                steal: *steal,
                for_actor: for_actor
                    .as_deref()
                    .map(|a| s.db.find_actor(a))
                    .transpose()?,
                loc: s.loc.clone().unwrap_or_else(|| Location {
                    host: s.host.clone(),
                    cwd: s.cwd.to_string_lossy().into_owned(),
                    branch: None,
                    head: None,
                }),
                scope_project,
                workspace_id,
            };
            let t = s.db.claim(&s.actor, &req, &s.ctx)?;
            let c = t.claim.as_ref().expect("claimed task has a claim");
            let text = format!(
                "claimed {} \"{}\"\n  {} @ {}:{}{}\n",
                fmt_id(t.id),
                t.title,
                c.actor.label,
                c.host,
                c.cwd,
                c.branch
                    .as_ref()
                    .map_or(String::new(), |b| format!(" ({b})")),
            );
            task_out(s, &t, text)
        }
        Cmd::Plan { id, text } => {
            let id = parse_id(id)?;
            let body = read_text(text)?;
            let t =
                s.db.apply(&s.actor, id, &Command::Plan { text: body }, &s.ctx)?;
            let msg = if t.needs_plan_approval {
                format!(
                    "plan saved on {}; waiting for a human to approve it\n",
                    fmt_id(id)
                )
            } else {
                format!("plan saved on {}\n", fmt_id(id))
            };
            task_out(s, &t, msg)
        }
        Cmd::Note { id, text, kind } => {
            let id = parse_id(id)?;
            let body = read_text(text)?;
            let cmd = Command::Note {
                kind: kind.clone(),
                body,
            };
            let t = s.db.apply(&s.actor, id, &cmd, &s.ctx)?;
            task_out(s, &t, format!("{kind} note added to {}\n", fmt_id(id)))
        }
        Cmd::Done { id, result } => {
            let id = parse_id(id)?;
            let cmd = Command::Done {
                result: result.clone(),
            };
            let t = s.db.apply(&s.actor, id, &cmd, &s.ctx)?;
            let ev = t
                .claim
                .as_ref()
                .map(|c| live_evidence(s, c))
                .filter(|e| e.source == Source::Live);
            if let (Some(ev), Some(c)) = (&ev, &t.claim) {
                // Best effort: the snapshot only matters if the checkout disappears later.
                if let Ok(j) = serde_json::to_string(&ev.ev) {
                    let _ = s.db.save_snapshot(c.id, &j, s.ctx.now);
                }
            }
            let no_commits = t.kind == Kind::Code && ev.as_ref().is_none_or(|e| e.ev.commits == 0);
            let mut text = format!("{} is now in review\n", fmt_id(id));
            if no_commits {
                text.push_str("warning: claimed, no commits. Explain why in a handoff note.\n");
            } else if let Some(ev) = &ev
                && t.kind == Kind::Code
            {
                text.push_str(&format!("{}\n", ev.ev.summary()));
            }
            let mut out = task_out(s, &t, text)?;
            out.data["evidence"] = ev.as_ref().map_or(Value::Null, Ev::to_json);
            out.data["warnings"] = if no_commits {
                json!(["claimed, no commits"])
            } else {
                json!([])
            };
            Ok(out)
        }
        Cmd::Show { id, evidence } => {
            let id = parse_id(id)?;
            show_out(s, s.db.task_detail(id)?, *evidence)
        }
        Cmd::List {
            status,
            project,
            agent,
            host,
            needs_me,
            all,
            evidence,
        } => {
            let statuses = status
                .iter()
                .flat_map(|v| v.split(','))
                .map(|v| {
                    Status::parse(v.trim()).ok_or_else(|| {
                        FbError::Usage(format!(
                            "bad --status {v:?} (draft | todo | in_progress | review | done | dropped)"
                        ))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let filter = TaskFilter {
                statuses,
                project_id: project.as_deref().map(|p| find_project(s, p)).transpose()?,
                include_terminal: *all,
                claimant: None,
            };
            let mut tasks = s.db.list_tasks(&filter)?;
            if let Some(a) = agent {
                let a = a.to_lowercase();
                tasks.retain(|t| {
                    t.claim.as_ref().is_some_and(|c| {
                        c.actor.label.to_lowercase().contains(&a)
                            || c.actor.provider.as_deref() == Some(a.as_str())
                            || c.actor.external_id == a
                    })
                });
            }
            if let Some(h) = host {
                tasks.retain(|t| t.claim.as_ref().is_some_and(|c| &c.host == h));
            }
            if *needs_me {
                tasks.retain(|t| derive_for(s, t).needs_me);
            }
            list_out(s, tasks, *evidence)
        }
        Cmd::Prime => prime_out(s),
        Cmd::Whoami => whoami_out(s),
        Cmd::Dep { cmd } => {
            let (task, blocker, add) = match cmd {
                DepCmd::Add { task, blocker } => (parse_id(task)?, parse_id(blocker)?, true),
                DepCmd::Rm { task, blocker } => (parse_id(task)?, parse_id(blocker)?, false),
            };
            let t = if add {
                s.db.dep_add(&s.actor, task, blocker, &s.ctx)?
            } else {
                s.db.dep_rm(&s.actor, task, blocker, &s.ctx)?
            };
            let verb = if add {
                "now depends on"
            } else {
                "no longer depends on"
            };
            task_out(
                s,
                &t,
                format!("{} {verb} {}\n", fmt_id(task), fmt_id(blocker)),
            )
        }
        Cmd::Promote { id } => simple(s, id, &Command::Promote, "promoted to todo"),
        Cmd::Approve { id } => simple(s, id, &Command::Approve, "plan approved"),
        Cmd::Verify { id, force } => {
            let tid = parse_id(id)?;
            let task = s.db.get_task(tid)?;
            let commits = if task.kind == Kind::Code {
                latest_claim(s, &task)
                    .map(|c| live_evidence(s, &c))
                    .and_then(|e| e.commits())
            } else {
                None
            };
            let cmd = Command::Verify {
                force: *force,
                commits,
            };
            let t = s.db.apply(&s.actor, tid, &cmd, &s.ctx)?;
            task_out(s, &t, format!("{} verified: done\n", fmt_id(tid)))
        }
        Cmd::Reject { id, comment } => {
            let cmd = Command::Reject {
                comment: read_text(comment)?,
            };
            simple(s, id, &cmd, "sent back to in_progress")
        }
        Cmd::Release { id } => simple(s, id, &Command::Release, "released: back to todo"),
        Cmd::Drop { id, reason } => simple(
            s,
            id,
            &Command::Drop {
                reason: reason.clone(),
            },
            "dropped",
        ),
        Cmd::Reopen { id } => simple(s, id, &Command::Reopen, "reopened: back to todo"),
        Cmd::Delegate { actor, allow } => {
            model::require_human(s.actor.role(), "delegate")?;
            let target = s.db.find_actor(actor)?;
            let allowed_actions: Vec<String> = allow
                .iter()
                .flat_map(|s| s.split(','))
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            let current = s.db.delegate(&s.actor, &target, &allowed_actions, &s.ctx)?;
            Ok(Out {
                text: format!(
                    "{} is now delegated: {}\n",
                    target.label,
                    current.join(", ")
                ),
                data: json!({
                    "actor": target,
                    "delegated": current,
                }),
            })
        }
        Cmd::Undelegate { actor, allow } => {
            model::require_human(s.actor.role(), "undelegate")?;
            let target = s.db.find_actor(actor)?;
            let revoked_actions: Option<Vec<String>> = allow.as_ref().map(|list| {
                list.iter()
                    .flat_map(|s| s.split(','))
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            });
            let remaining =
                s.db.undelegate(&s.actor, &target, revoked_actions.as_deref(), &s.ctx)?;
            let text = match &revoked_actions {
                Some(rev) => format!(
                    "{} revoked delegated rights: {}\n",
                    target.label,
                    rev.join(", ")
                ),
                None => format!("{} revoked all delegated rights\n", target.label),
            };
            Ok(Out {
                text,
                data: json!({
                    "actor": target,
                    "delegated": remaining,
                }),
            })
        }
        Cmd::Trust { actor } | Cmd::Untrust { actor } => {
            let trusted = matches!(cmd, Cmd::Trust { .. });
            let target = s.db.find_actor(actor)?;
            let a = s.db.set_trusted(&s.actor, &target, trusted, &s.ctx)?;
            Ok(Out {
                text: format!(
                    "{} is {} the tracker role\n",
                    a.label,
                    if trusted {
                        "now trusted with"
                    } else {
                        "no longer trusted with"
                    }
                ),
                data: json!({ "actor": a }),
            })
        }
        Cmd::Project {
            cmd: ProjectCmd::Alias { key, alias },
        } => {
            let p = s.db.set_alias(&s.actor, key, alias, &s.ctx)?;
            Ok(Out {
                text: format!("{} is now shown as {}\n", p.key, p.display()),
                data: json!({ "project": { "key": p.key, "alias": p.alias, "name": p.display() } }),
            })
        }
        Cmd::Serve { .. } | Cmd::Hook { .. } | Cmd::Mcp => unreachable!("handled in execute"),
    }
}

fn simple(s: &Session, id: &str, cmd: &Command, verb: &str) -> Result<Out> {
    let id = parse_id(id)?;
    let t = s.db.apply(&s.actor, id, cmd, &s.ctx)?;
    task_out(s, &t, format!("{} {verb}\n", fmt_id(id)))
}

fn read_text(arg: &str) -> Result<String> {
    if arg != "-" {
        return Ok(arg.to_string());
    }
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    Ok(buf.trim_end().to_string())
}

fn parse_ticket(
    ticket: Option<&str>,
    title: &Option<String>,
    url: &Option<String>,
) -> Result<Option<TicketSpec>> {
    let Some(t) = ticket else {
        if title.is_some() || url.is_some() {
            return Err(FbError::Usage(
                "--ticket-title / --ticket-url need --ticket".into(),
            ));
        }
        return Ok(None);
    };
    let (system, key) = t
        .split_once(':')
        .filter(|(s, k)| !s.is_empty() && !k.is_empty())
        .ok_or_else(|| FbError::Usage(format!("bad --ticket {t:?} (expected system:key)")))?;
    let system = system.to_lowercase();
    if !["jira", "linear", "github", "other"].contains(&system.as_str()) {
        return Err(FbError::Usage(format!(
            "bad ticket system {system:?} (jira | linear | github | other)"
        )));
    }
    Ok(Some(TicketSpec {
        system,
        key: key.to_string(),
        title: title.clone(),
        url: url.clone(),
    }))
}

// ---------- project scope ----------

enum Scope {
    Any,
    Project(i64),
    /// In a git repo the board has not seen yet: nothing to list.
    Empty,
}

fn detect_scope(s: &Session) -> Result<Scope> {
    match gitinfo::project_key(&s.cwd) {
        None => Ok(Scope::Any),
        Some(key) => Ok(s
            .db
            .find_project(&key)?
            .map_or(Scope::Empty, |p| Scope::Project(p.id))),
    }
}

fn find_project(s: &Session, key_or_alias: &str) -> Result<i64> {
    s.db.find_project(key_or_alias)?
        .map(|p| p.id)
        .ok_or_else(|| FbError::NotFound(format!("no project {key_or_alias:?}")))
}

// ---------- evidence ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Live,
    Snapshot,
    None,
}

struct Ev {
    ev: Evidence,
    source: Source,
    computed_at: Option<DateTime<Utc>>,
}

impl Ev {
    /// Commit count when it is known (live or from a snapshot).
    fn commits(&self) -> Option<u32> {
        (self.source != Source::None).then_some(self.ev.commits)
    }

    fn to_json(&self) -> Value {
        let mut v = serde_json::to_value(&self.ev).unwrap_or(Value::Null);
        v["source"] = json!(match self.source {
            Source::Live => "live",
            Source::Snapshot => "snapshot",
            Source::None => "none",
        });
        if let Some(t) = self.computed_at {
            v["computed_at"] = json!(ts(t));
        }
        v
    }
}

fn empty_evidence() -> Evidence {
    Evidence {
        commits: 0,
        rebased: false,
        diffstat: None,
        dirty: false,
        pr: None,
        reachable: false,
    }
}

/// Live git evidence for a claim on this host, else the last snapshot, else nothing.
fn live_evidence(s: &Session, c: &ClaimRow) -> Ev {
    if c.host == s.host
        && let (Some(base), Some(branch)) = (&c.base_commit, &c.branch)
    {
        let e = gitinfo::evidence(Path::new(&c.cwd), base, branch, c.claimed_at, s.cfg.gh);
        if e.reachable {
            return Ev {
                ev: e,
                source: Source::Live,
                computed_at: Some(s.ctx.now),
            };
        }
    }
    if let Ok(Some((data, at))) = s.db.snapshot(c.id)
        && let Ok(mut e) = serde_json::from_str::<Evidence>(&data)
    {
        e.reachable = false;
        return Ev {
            ev: e,
            source: Source::Snapshot,
            computed_at: Some(at),
        };
    }
    Ev {
        ev: empty_evidence(),
        source: Source::None,
        computed_at: None,
    }
}

/// Active claim, else the most recent one.
fn latest_claim(s: &Session, t: &TaskRow) -> Option<ClaimRow> {
    t.claim.clone().or_else(|| {
        s.db.task_detail(t.id)
            .ok()
            .and_then(|d| d.claims.into_iter().next())
    })
}

// ---------- derived state ----------

fn claim_is_stale(s: &Session, c: &ClaimRow) -> bool {
    if let Some(status) = s.adapter.agent_status(&c.actor.external_id)
        && status == crate::adapters::AgentRuntimeStatus::Gone
    {
        return true;
    }
    let last_commit = if c.host == s.host {
        c.branch
            .as_deref()
            .and_then(|b| gitinfo::last_commit_time(Path::new(&c.cwd), b))
    } else {
        None
    };
    model::is_stale(s.ctx.now, c.last_activity, last_commit, s.cfg.stale_after)
}

fn derive_for(s: &Session, t: &TaskRow) -> Derived {
    let blockers: Vec<Status> = t.blocked_by.iter().map(|b| b.status).collect();
    let commits = if t.status == Status::Review && t.kind == Kind::Code {
        t.claim
            .as_ref()
            .map(|c| live_evidence(s, c))
            .and_then(|e| e.commits())
    } else {
        None
    };
    let pending_permissions = t
        .claim
        .as_ref()
        .map_or(0, |c| s.adapter.pending_permissions(&c.actor.external_id));

    model::derive(&Facts {
        status: t.status,
        kind: t.kind,
        needs_plan_approval: t.needs_plan_approval,
        has_plan: t.plan.is_some(),
        plan_approved: t.plan_approved_at.is_some(),
        blockers: &blockers,
        claim_stale: t.status == Status::InProgress
            && t.claim.as_ref().is_some_and(|c| claim_is_stale(s, c)),
        commits,
        pending_permissions,
    })
}

// ---------- JSON shaping ----------

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn claim_json(c: &ClaimRow) -> Value {
    json!({
        "id": c.id,
        "actor": c.actor,
        "host": c.host,
        "cwd": c.cwd,
        "branch": c.branch,
        "base_commit": c.base_commit,
        "workspace_id": c.workspace_id,
        "claimed_at": ts(c.claimed_at),
        "last_activity": ts(c.last_activity),
        "released_at": c.released_at.map(ts),
        "release_reason": c.release_reason,
    })
}

fn task_json(s: &Session, t: &TaskRow, d: &Derived) -> Value {
    let mut val = json!({
        "id": fmt_id(t.id),
        "title": t.title,
        "kind": t.kind.as_str(),
        "status": t.status.as_str(),
        "priority": t.priority,
        "project": t.project.as_ref().map(|p| json!({ "key": p.key, "alias": p.alias, "name": p.display() })),
        "ticket": t.ticket,
        "goal": t.goal,
        "acceptance": t.acceptance,
        "plan": t.plan,
        "result": t.result,
        "needs_plan_approval": t.needs_plan_approval,
        "plan_approved_at": t.plan_approved_at.map(ts),
        "blocked_by": t.blocked_by.iter().map(|b| json!({
            "id": fmt_id(b.id), "status": b.status.as_str(), "title": b.title,
        })).collect::<Vec<_>>(),
        "claim": t.claim.as_ref().map(claim_json),
        "derived": d,
        "runtime": null,
        "runtime_status": null,
        "pending_permissions": null,
        "created_by": t.created_by,
        "created_at": ts(t.created_at),
        "updated_at": ts(t.updated_at),
    });

    if let Some(c) = &t.claim
        && let Some(st) = s.adapter.agent_status(&c.actor.external_id)
    {
        let perms = s.adapter.pending_permissions(&c.actor.external_id);
        val["runtime"] = json!({
            "status": st.as_str(),
            "pending_permissions": perms,
        });
        val["runtime_status"] = json!(st.as_str());
        val["pending_permissions"] = json!(perms);
    }

    val
}

fn task_out(s: &Session, t: &TaskRow, text: String) -> Result<Out> {
    Ok(Out {
        data: json!({ "task": task_json(s, t, &derive_for(s, t)) }),
        text,
    })
}

// ---------- text rendering ----------

fn flags(d: &Derived, perms: u32) -> String {
    let mut f = Vec::new();
    for (on, name) in [
        (d.ready, "ready"),
        (d.orphaned, "orphaned"),
        (d.blocked && !d.orphaned, "blocked"),
        (d.plan_pending, "plan-pending"),
        (d.stale, "stale"),
        (d.no_evidence, "no-evidence"),
        (perms > 0, "permission"),
        (d.needs_me, "needs-me"),
    ] {
        if on {
            f.push(name);
        }
    }
    if f.is_empty() {
        String::new()
    } else {
        format!(" [{}]", f.join(", "))
    }
}

fn line(s: &Session, t: &TaskRow, d: &Derived) -> String {
    let perms = t
        .claim
        .as_ref()
        .map_or(0, |c| s.adapter.pending_permissions(&c.actor.external_id));
    let who = t.claim.as_ref().map_or(String::new(), |c| {
        format!(
            "  {} @ {}{}",
            c.actor.label,
            c.host,
            c.branch.as_ref().map_or(String::new(), |b| format!(":{b}"))
        )
    });
    format!(
        "{:<7} {:<11} P{} {}{}{}{}\n",
        fmt_id(t.id),
        t.status.as_str(),
        t.priority,
        t.project
            .as_ref()
            .map_or(String::new(), |p| format!("[{}] ", p.display())),
        t.title,
        who,
        flags(d, perms)
    )
}

fn list_out(s: &Session, tasks: Vec<TaskRow>, evidence: bool) -> Result<Out> {
    let mut text = String::new();
    let mut items = Vec::new();
    for t in &tasks {
        let d = derive_for(s, t);
        text.push_str(&line(s, t, &d));
        let mut j = task_json(s, t, &d);
        if evidence && let Some(c) = latest_claim(s, t) {
            j["evidence"] = live_evidence(s, &c).to_json();
        }
        items.push(j);
    }
    if tasks.is_empty() {
        text.push_str("no tasks\n");
    }
    Ok(Out {
        data: json!({ "tasks": items }),
        text,
    })
}

fn show_out(s: &Session, d: TaskDetail, evidence: bool) -> Result<Out> {
    let t = &d.task;
    let derived = derive_for(s, t);
    let mut j = task_json(s, t, &derived);
    j["claims"] = json!(d.claims.iter().map(claim_json).collect::<Vec<_>>());
    j["notes"] = json!(
        d.notes
            .iter()
            .map(|n| json!({
                "id": n.id,
                "kind": n.kind,
                "actor": n.actor,
                "body": n.body,
                "created_at": ts(n.created_at),
            }))
            .collect::<Vec<_>>()
    );
    let ev = if evidence {
        d.claims.first().map(|c| live_evidence(s, c))
    } else {
        None
    };
    j["evidence"] = ev.as_ref().map_or(Value::Null, Ev::to_json);

    let mut text = line(s, t, &derived);
    if let Some(c) = t.claim.as_ref()
        && let Some(st) = s.adapter.agent_status(&c.actor.external_id)
    {
        let p = s.adapter.pending_permissions(&c.actor.external_id);
        text.push_str(&format!(
            "runtime: {} ({} pending {})\n",
            st.as_str(),
            p,
            if p == 1 { "permission" } else { "permissions" }
        ));
    }
    for (name, v) in [
        ("goal", &t.goal),
        ("acceptance", &t.acceptance),
        ("plan", &t.plan),
        ("result", &t.result),
    ] {
        if let Some(v) = v {
            text.push_str(&format!("{name}:\n"));
            for l in v.lines() {
                text.push_str(&format!("  {l}\n"));
            }
        }
    }
    if let Some(tk) = &t.ticket {
        text.push_str(&format!(
            "ticket: {}:{}{}\n",
            tk.system,
            tk.key,
            tk.title.as_ref().map_or(String::new(), |x| format!(" {x}"))
        ));
    }
    for b in &t.blocked_by {
        text.push_str(&format!(
            "blocked by: {} ({}) {}\n",
            fmt_id(b.id),
            b.status.as_str(),
            b.title
        ));
    }
    for c in &d.claims {
        text.push_str(&format!(
            "claim: {} @ {}:{}{} since {}{}\n",
            c.actor.label,
            c.host,
            c.cwd,
            c.branch
                .as_ref()
                .map_or(String::new(), |b| format!(" ({b})")),
            ts(c.claimed_at),
            c.release_reason
                .as_ref()
                .map_or(String::new(), |r| format!(", ended: {r}")),
        ));
    }
    for n in &d.notes {
        text.push_str(&format!(
            "{} note by {} at {}:\n",
            n.kind,
            n.actor.label,
            ts(n.created_at)
        ));
        for l in n.body.lines() {
            text.push_str(&format!("  {l}\n"));
        }
    }
    if !d.history.is_empty() {
        text.push_str("history:\n");
        for e in &d.history {
            let who = match &e.on_behalf_of {
                Some(obo) => format!("{} on behalf of {obo}", e.actor.label),
                None => e.actor.label.clone(),
            };
            text.push_str(&format!(
                "  {} by {} at {}\n",
                e.event_type,
                who,
                ts(e.created_at)
            ));
        }
    }
    if let Some(e) = &ev {
        let v = &e.ev;
        text.push_str(&format!(
            "evidence ({}): {} commit(s){}{}{}{}\n",
            match e.source {
                Source::Live => "live",
                Source::Snapshot => "snapshot",
                Source::None => "none",
            },
            v.commits,
            if v.rebased { ", rebased" } else { "" },
            if v.dirty { ", dirty worktree" } else { "" },
            if v.reachable {
                ""
            } else {
                ", checkout unreachable"
            },
            v.pr.as_ref()
                .map_or(String::new(), |p| format!(", PR #{} {}", p.number, p.state)),
        ));
        if let Some(ds) = &v.diffstat {
            text.push_str(&format!("  {ds}\n"));
        }
    }
    Ok(Out {
        data: json!({ "task": j }),
        text,
    })
}

fn prime_out(s: &Session) -> Result<Out> {
    let project = gitinfo::project_key(&s.cwd);
    let tasks = s.db.list_tasks(&TaskFilter {
        claimant: Some(s.actor.id),
        ..Default::default()
    })?;
    let mut claims = Vec::new();
    let mut claims_json = Vec::new();
    for t in &tasks {
        let detail = s.db.task_detail(t.id)?;
        let handoff = detail
            .notes
            .iter()
            .rev()
            .find(|n| n.kind == "handoff")
            .map(|n| Handoff {
                author: n.actor.label.clone(),
                age: s.ctx.now - n.created_at,
                body: n.body.clone(),
            });
        let plan = match (t.needs_plan_approval, t.plan_approved_at.is_some()) {
            (false, _) => PlanState::NotRequired,
            (true, true) => PlanState::Approved,
            (true, false) => PlanState::Pending,
        };
        claims_json.push(json!({
            "task": task_json(s, t, &derive_for(s, t)),
            "last_handoff": handoff.as_ref().map(|h| json!({
                "author": h.author,
                "age_seconds": h.age.num_seconds(),
                "body": h.body,
            })),
        }));
        claims.push(PrimeClaim {
            id: fmt_id(t.id),
            title: t.title.clone(),
            status: t.status.as_str().to_string(),
            plan,
            handoff,
        });
    }
    let text = prime::render(&PrimeInput {
        label: s.actor.label.clone(),
        host: s.host.clone(),
        project: project.clone(),
        claims,
    });
    Ok(Out {
        data: json!({
            "actor": s.actor,
            "host": s.host,
            "project": project,
            "claims": claims_json,
            "text": text,
        }),
        text,
    })
}

fn whoami_out(s: &Session) -> Result<Out> {
    let project = gitinfo::project_key(&s.cwd);
    let instance = gitinfo::instance(&s.cwd);

    let prov = s.actor.provider.as_deref().unwrap_or("(none)");
    let proj = project.as_deref().unwrap_or("(none)");
    let inst = match &instance {
        Some(i) => {
            let branch = i
                .branch
                .as_deref()
                .map(|b| format!(" ({b})"))
                .unwrap_or_default();
            format!("{}{branch}", i.toplevel.display())
        }
        None => "(none)".to_string(),
    };

    let delegated = s.db.get_delegations(s.actor.id)?;
    let delegated_str = if delegated.is_empty() {
        "(none)".to_string()
    } else {
        delegated.join(", ")
    };

    let text = format!(
        "kind:        {}\n\
         provider:    {}\n\
         external_id: {}\n\
         label:       {}\n\
         trusted:     {}\n\
         delegated:   {}\n\
         host:        {}\n\
         project:     {}\n\
         instance:    {}\n",
        s.actor.kind,
        prov,
        s.actor.external_id,
        s.actor.label,
        s.actor.trusted,
        delegated_str,
        s.host,
        proj,
        inst
    );

    let data = json!({
        "caller": {
            "kind": s.actor.kind,
            "provider": s.actor.provider,
            "external_id": s.actor.external_id,
            "label": s.actor.label,
            "trusted": s.actor.trusted,
            "delegated": delegated,
        },
        "host": s.host,
        "project": project,
        "instance": instance,
    });

    Ok(Out { data, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_runtime_is_explicitly_null() {
        let home = tempfile::tempdir().unwrap();
        let s = create_session(home.path(), false).unwrap();
        let task =
            s.db.add_task(
                &s.actor,
                &NewTask {
                    title: "Synthetic task".into(),
                    goal: None,
                    acceptance: None,
                    kind: Kind::Code,
                    priority: 2,
                    blocked_by: vec![],
                    ticket: None,
                    project: None,
                    needs_plan_approval: false,
                },
                &s.ctx,
            )
            .unwrap();
        let value = task_json(&s, &task, &derive_for(&s, &task));
        for key in ["runtime", "runtime_status", "pending_permissions"] {
            assert_eq!(value.get(key), Some(&Value::Null), "{key}");
        }
    }
}
