//! Stdio Model Context Protocol (MCP) server for fleetboard.
//! Exposes agent-facing tools and delegated human-only verbs over stdio.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    model::*,
    service::{RequestContext, RoleServer},
};
use serde_json::{Value, json};

use crate::cli::{self, Cmd, Out};
use crate::model;
use crate::store::{Backend, FbError};

/// MCP server instance managing state and tool dispatch.
#[derive(Clone)]
pub struct McpServer {
    home: PathBuf,
    as_human: bool,
}

impl McpServer {
    pub fn new(home: PathBuf, as_human: bool) -> Self {
        Self { home, as_human }
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        ServerConfig::new(capabilities)
            .with_server_info(Implementation::new("fleetboard", env!("CARGO_PKG_VERSION")))
            .with_instructions("fleetboard MCP server for task and claim management")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let s = match cli::create_session(&self.home, self.as_human) {
            Ok(s) => s,
            Err(e) => return Err(McpError::internal_error(e.to_string(), None)),
        };

        let delegations = if s.actor.is_human() {
            model::DELEGATABLE_ACTIONS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        } else {
            s.db.get_delegations(s.actor.id).unwrap_or_default()
        };

        let mut tools = vec![
            tool_fb_prime(),
            tool_fb_ready(),
            tool_fb_claim(),
            tool_fb_show(),
            tool_fb_note(),
            tool_fb_plan(),
            tool_fb_done(),
            tool_fb_add(),
            tool_fb_list(),
        ];

        for verb in &delegations {
            if let Some(tool) = delegated_tool_for(verb) {
                tools.push(tool);
            }
        }

        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let s = match cli::create_session(&self.home, self.as_human) {
            Ok(s) => s,
            Err(e) => return Ok(call_fberror(&e).into()),
        };

        let tool_name = request.name.strip_prefix("fb_").unwrap_or(&request.name);
        let args = request.arguments.unwrap_or_default();

        let cmd = match tool_name {
            "prime" => Cmd::Prime,
            "ready" => {
                let project = get_str(&args, "project").map(ToString::to_string);
                let all = get_bool(&args, "all").unwrap_or(false);
                Cmd::Ready { project, all }
            }
            "claim" => {
                let id = get_str(&args, "id").map(ToString::to_string);
                let steal = get_bool(&args, "steal").unwrap_or(false);
                let for_actor = get_str(&args, "for_actor")
                    .or_else(|| get_str(&args, "for"))
                    .map(ToString::to_string);
                Cmd::Claim {
                    id,
                    steal,
                    for_actor,
                }
            }
            "show" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let evidence = get_bool(&args, "evidence").unwrap_or(false);
                Cmd::Show { id, evidence }
            }
            "note" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let text = match get_str(&args, "text") {
                    Some(text) => text.to_string(),
                    None => return Ok(call_error("missing required argument 'text'").into()),
                };
                let kind = get_str(&args, "kind").unwrap_or("handoff").to_string();
                Cmd::Note { id, text, kind }
            }
            "plan" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let text = match get_str(&args, "text") {
                    Some(text) => text.to_string(),
                    None => return Ok(call_error("missing required argument 'text'").into()),
                };
                Cmd::Plan { id, text }
            }
            "done" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let result = get_str(&args, "result").map(ToString::to_string);
                Cmd::Done { id, result }
            }
            "add" => {
                let title = match get_str(&args, "title") {
                    Some(t) => t.to_string(),
                    None => return Ok(call_error("missing required argument 'title'").into()),
                };
                let goal = get_str(&args, "goal").map(ToString::to_string);
                let ac = get_str_vec(&args, "ac");
                let kind = get_str(&args, "kind").unwrap_or("code").to_string();
                let prio = get_i64(&args, "prio").unwrap_or(2);
                let blocked_by = get_str_vec(&args, "blocked_by");
                let ticket = get_str(&args, "ticket").map(ToString::to_string);
                let ticket_title = get_str(&args, "ticket_title").map(ToString::to_string);
                let ticket_url = get_str(&args, "ticket_url").map(ToString::to_string);
                let project = get_str(&args, "project").map(ToString::to_string);
                let plan_approval = get_bool(&args, "plan_approval").unwrap_or(false);
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
                }
            }
            "list" => {
                let status = get_str_vec(&args, "status");
                let project = get_str(&args, "project").map(ToString::to_string);
                let agent = get_str(&args, "agent").map(ToString::to_string);
                let host = get_str(&args, "host").map(ToString::to_string);
                let needs_me = get_bool(&args, "needs_me").unwrap_or(false);
                let all = get_bool(&args, "all").unwrap_or(false);
                let evidence = get_bool(&args, "evidence").unwrap_or(false);
                Cmd::List {
                    status,
                    project,
                    agent,
                    host,
                    needs_me,
                    all,
                    evidence,
                }
            }
            "promote" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                Cmd::Promote { id }
            }
            "approve" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                Cmd::Approve { id }
            }
            "verify" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let force = get_bool(&args, "force").unwrap_or(false);
                Cmd::Verify { id, force }
            }
            "reject" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let comment = match get_str(&args, "comment") {
                    Some(c) => c.to_string(),
                    None => return Ok(call_error("missing required argument 'comment'").into()),
                };
                Cmd::Reject { id, comment }
            }
            "release" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                Cmd::Release { id }
            }
            "steal" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let for_actor = get_str(&args, "for_actor")
                    .or_else(|| get_str(&args, "for"))
                    .map(ToString::to_string);
                Cmd::Claim {
                    id: Some(id),
                    steal: true,
                    for_actor,
                }
            }
            "drop" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                let reason = get_str(&args, "reason").map(ToString::to_string);
                Cmd::Drop { id, reason }
            }
            "reopen" => {
                let id = match get_str(&args, "id") {
                    Some(id) => id.to_string(),
                    None => return Ok(call_error("missing required argument 'id'").into()),
                };
                Cmd::Reopen { id }
            }
            _ => {
                return Err(McpError::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!("unknown tool: {}", request.name),
                    None,
                ));
            }
        };

        match cli::dispatch(&s, &cmd) {
            Ok(out) => Ok(call_success(out).into()),
            Err(e) => Ok(call_fberror(&e).into()),
        }
    }
}

fn call_success(out: Out) -> CallToolResult {
    let text = if out.text.is_empty() {
        serde_json::to_string_pretty(&out.data).unwrap_or_default()
    } else {
        out.text
    };
    let mut res = CallToolResult::success(vec![ContentBlock::text(text)]);
    res.structured_content = Some(out.data);
    res
}

fn call_error(msg: impl Into<String>) -> CallToolResult {
    let msg = msg.into();
    let mut res = CallToolResult::error(vec![ContentBlock::text(msg.clone())]);
    res.structured_content = Some(json!({
        "ok": false,
        "error": { "code": "USAGE", "message": msg }
    }));
    res
}

fn call_fberror(e: &FbError) -> CallToolResult {
    let msg = format!("{}: {}", e.code(), e);
    let mut res = CallToolResult::error(vec![ContentBlock::text(msg)]);
    res.structured_content = Some(json!({
        "ok": false,
        "error": { "code": e.code(), "message": e.to_string() }
    }));
    res
}

fn get_str<'a>(args: &'a JsonObject, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn get_bool(args: &JsonObject, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn get_i64(args: &JsonObject, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}

fn get_str_vec(args: &JsonObject, key: &str) -> Vec<String> {
    match args.get(key) {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(ToString::to_string))
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn schema_empty() -> Arc<JsonObject> {
    let schema = json!({
        "type": "object",
        "properties": {}
    });
    Arc::new(schema.as_object().unwrap().clone())
}

fn tool_fb_prime() -> Tool {
    Tool::new(
        "fb_prime",
        "Workflow guide plus caller's active claim and latest handoff note",
        schema_empty(),
    )
}

fn tool_fb_ready() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "project": {
                "type": "string",
                "description": "Project key or alias (default: detected from git)"
            },
            "all": {
                "type": "boolean",
                "description": "List all ready tasks across projects"
            }
        }
    });
    Tool::new(
        "fb_ready",
        "List Ready tasks claimable by agents",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_claim() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "Task id (e.g. fb-1). Omit to pick the next ready task."
            },
            "steal": {
                "type": "boolean",
                "description": "Take over an active claim (human or trusted/delegated only)"
            },
            "for_actor": {
                "type": "string",
                "description": "With steal: hand the claim to this actor"
            }
        }
    });
    Tool::new(
        "fb_claim",
        "Claim a task (no id: picks the best Ready task in this project)",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_show() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "Task id (e.g. fb-1)"
            },
            "evidence": {
                "type": "boolean",
                "description": "Include git evidence computation"
            }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_show",
        "Show a task in full with goal, criteria, claim history and comments",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_note() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "Task id (e.g. fb-1)"
            },
            "text": {
                "type": "string",
                "description": "Note content"
            },
            "kind": {
                "type": "string",
                "description": "handoff | comment | finding (default: handoff)",
                "enum": ["handoff", "comment", "finding"]
            }
        },
        "required": ["id", "text"]
    });
    Tool::new(
        "fb_note",
        "Add a note to a task (handoff, comment, or finding)",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_plan() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "Task id (e.g. fb-1)"
            },
            "text": {
                "type": "string",
                "description": "Plan markdown text"
            }
        },
        "required": ["id", "text"]
    });
    Tool::new(
        "fb_plan",
        "Write or replace the task plan",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_done() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "Task id (e.g. fb-1)"
            },
            "result": {
                "type": "string",
                "description": "Summary result text (especially for note tasks)"
            }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_done",
        "Finish work and move task to review",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_add() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": "Short title describing the task"
            },
            "goal": {
                "type": "string",
                "description": "Goal and background explanation"
            },
            "ac": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Acceptance criteria list"
            },
            "kind": {
                "type": "string",
                "description": "code (default) or note",
                "enum": ["code", "note"]
            },
            "prio": {
                "type": "integer",
                "description": "Priority 0 (highest) to 3 (default: 2)"
            },
            "blocked_by": {
                "type": "array",
                "items": { "type": "string" },
                "description": "IDs of tasks that block this task"
            },
            "ticket": {
                "type": "string",
                "description": "External parent ticket as system:key (e.g. jira:ABC-36)"
            },
            "ticket_title": {
                "type": "string",
                "description": "External ticket title"
            },
            "ticket_url": {
                "type": "string",
                "description": "External ticket URL"
            },
            "project": {
                "type": "string",
                "description": "Project key or alias (default: detected from git)"
            },
            "plan_approval": {
                "type": "boolean",
                "description": "Hold task in plan_pending until human approves"
            }
        },
        "required": ["title"]
    });
    Tool::new(
        "fb_add",
        "Create a task (agents: draft; human or trusted: todo)",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_list() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Status filter: draft, todo, in_progress, review, done, dropped"
            },
            "project": {
                "type": "string",
                "description": "Project key or alias"
            },
            "agent": {
                "type": "string",
                "description": "Filter by agent label or external id"
            },
            "host": {
                "type": "string",
                "description": "Filter by host name"
            },
            "needs_me": {
                "type": "boolean",
                "description": "Filter to tasks needing owner attention"
            },
            "all": {
                "type": "boolean",
                "description": "Include done and dropped tasks"
            },
            "evidence": {
                "type": "boolean",
                "description": "Include git evidence computation"
            }
        }
    });
    Tool::new(
        "fb_list",
        "Filtered task list",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_promote() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_promote",
        "(delegated) Promote draft task to todo",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_approve() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_approve",
        "(delegated) Approve task plan",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_verify() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" },
            "force": { "type": "boolean", "description": "Force verify even without commits" }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_verify",
        "(delegated) Verify review task and mark done",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_reject() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" },
            "comment": { "type": "string", "description": "Reason for rejection" }
        },
        "required": ["id", "comment"]
    });
    Tool::new(
        "fb_reject",
        "(delegated) Reject review task back to in_progress with comment",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_release() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_release",
        "(delegated) Release active claim: task returns to todo",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_steal() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" },
            "for_actor": { "type": "string", "description": "Hand claim over to this actor" }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_steal",
        "(delegated) Steal active claim on a task",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_drop() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" },
            "reason": { "type": "string", "description": "Reason for dropping task" }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_drop",
        "(delegated) Drop a task",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn tool_fb_reopen() -> Tool {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "description": "Task id (e.g. fb-1)" }
        },
        "required": ["id"]
    });
    Tool::new(
        "fb_reopen",
        "(delegated) Reopen a done or dropped task",
        Arc::new(schema.as_object().unwrap().clone()),
    )
}

fn delegated_tool_for(verb: &str) -> Option<Tool> {
    match verb {
        "promote" => Some(tool_fb_promote()),
        "approve" => Some(tool_fb_approve()),
        "verify" => Some(tool_fb_verify()),
        "reject" => Some(tool_fb_reject()),
        "release" => Some(tool_fb_release()),
        "steal" => Some(tool_fb_steal()),
        "drop" => Some(tool_fb_drop()),
        "reopen" => Some(tool_fb_reopen()),
        _ => None,
    }
}

/// Serve the MCP server over any async read/write stream.
pub async fn serve_io<R, W>(server: McpServer, reader: R, writer: W) -> Result<(), FbError>
where
    R: tokio::io::AsyncRead + Send + Unpin + 'static,
    W: tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let service = server
        .serve((reader, writer))
        .await
        .map_err(|e| FbError::Store(format!("MCP server initialization failed: {e}")))?;
    let _ = service.waiting().await;
    Ok(())
}

/// Run the stdio MCP server blocking the current thread with a tokio runtime.
pub fn run_stdio_server(home: &Path, as_human: bool) -> Result<Out, FbError> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| FbError::Store(format!("failed to start tokio runtime: {e}")))?;

    let server = McpServer::new(home.to_path_buf(), as_human);
    rt.block_on(async move {
        let (reader, writer) = rmcp::transport::stdio();
        serve_io(server, reader, writer).await?;
        Ok(Out {
            data: Value::Null,
            text: String::new(),
        })
    })
}
