//! `SqliteBackend`: WAL, busy timeout, `BEGIN IMMEDIATE` for every write.

use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};

use super::Backend;
use super::error::{FbError, Result};
use super::schema::{MIGRATIONS, SCHEMA_VERSION};
use super::types::*;
use crate::identity::{ActorKind, Caller};
use crate::model::{self, Action, Kind, Role, Status};

const BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

pub struct SqliteBackend {
    conn: Connection,
}

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| FbError::Store(format!("bad timestamp {s:?}: {e}")))
}

fn opt_ts(s: Option<String>) -> Result<Option<DateTime<Utc>>> {
    s.as_deref().map(parse_ts).transpose()
}

impl SqliteBackend {
    /// Open (creating and migrating if needed) the database file.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        // Refuse a newer database before touching it.
        check_not_ahead(&conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let this = Self { conn };
        this.migrate()?;
        Ok(this)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let this = Self { conn };
        this.migrate()?;
        Ok(this)
    }

    fn migrate(&self) -> Result<()> {
        if user_version(&self.conn)? == SCHEMA_VERSION {
            return Ok(());
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        // Another process may have migrated while we waited for the lock.
        let v = user_version(&tx)?;
        if v > SCHEMA_VERSION {
            return Err(FbError::SchemaAhead {
                found: v,
                supported: SCHEMA_VERSION,
            });
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(v as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn write<T>(&self, f: impl FnOnce(&Transaction) -> Result<T>) -> Result<T> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    pub fn has_delegation(&self, actor_id: i64, action: &str) -> Result<bool> {
        tx_has_delegation(&self.conn, actor_id, action)
    }
}

fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

fn check_not_ahead(conn: &Connection) -> Result<()> {
    let v = user_version(conn)?;
    if v > SCHEMA_VERSION {
        return Err(FbError::SchemaAhead {
            found: v,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(())
}

// ---------- row loading ----------

const ACTOR_COLS: &str = "id, kind, provider, external_id, label, trusted";

fn actor_from(r: &Row, o: usize) -> rusqlite::Result<Actor> {
    let external_id: String = r.get(o + 3)?;
    Ok(Actor {
        id: r.get(o)?,
        kind: r.get(o + 1)?,
        provider: r.get(o + 2)?,
        label: r
            .get::<_, Option<String>>(o + 4)?
            .unwrap_or_else(|| external_id.clone()),
        external_id,
        trusted: r.get::<_, i64>(o + 5)? != 0,
    })
}

fn load_actor(conn: &Connection, id: i64) -> Result<Actor> {
    conn.query_row(
        &format!("SELECT {ACTOR_COLS} FROM actors WHERE id = ?1"),
        [id],
        |r| actor_from(r, 0),
    )
    .optional()?
    .ok_or_else(|| FbError::NotFound(format!("no actor with id {id}")))
}

fn tx_get_delegations(conn: &Connection, actor_id: i64) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT action FROM delegations WHERE actor_id = ?1 ORDER BY action")?;
    let list = stmt
        .query_map([actor_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(list)
}

fn tx_has_delegation(conn: &Connection, actor_id: i64, action: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM delegations WHERE actor_id = ?1 AND action = ?2",
        params![actor_id, action],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

const CLAIM_COLS: &str = "id, task_id, actor_id, host, cwd, branch, base_commit, workspace_id, \
                          claimed_at, last_activity, released_at, release_reason";

fn load_claims(
    conn: &Connection,
    where_sql: &str,
    p: &[&dyn rusqlite::ToSql],
) -> Result<Vec<ClaimRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CLAIM_COLS} FROM claims WHERE {where_sql} ORDER BY id DESC"
    ))?;
    let raw = stmt
        .query_map(p, |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<String>>(11)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    raw.into_iter()
        .map(
            |(
                id,
                task_id,
                actor_id,
                host,
                cwd,
                branch,
                base_commit,
                workspace_id,
                ca,
                la,
                ra,
                rr,
            )| {
                Ok(ClaimRow {
                    id,
                    task_id,
                    actor: load_actor(conn, actor_id)?,
                    host,
                    cwd,
                    branch,
                    base_commit,
                    workspace_id,
                    claimed_at: parse_ts(&ca)?,
                    last_activity: parse_ts(&la)?,
                    released_at: opt_ts(ra)?,
                    release_reason: rr,
                })
            },
        )
        .collect()
}

fn active_claim(conn: &Connection, task_id: i64) -> Result<Option<ClaimRow>> {
    Ok(
        load_claims(conn, "task_id = ?1 AND released_at IS NULL", &[&task_id])?
            .into_iter()
            .next(),
    )
}

const TASK_SELECT: &str = "SELECT t.id, t.title, t.goal, t.acceptance, t.plan, t.result, t.kind, \
    t.status, t.priority, t.needs_plan_approval, t.plan_approved_at, t.created_by, t.created_at, \
    t.updated_at, p.id, p.key, p.alias, tk.system, tk.key, tk.title, tk.url \
    FROM tasks t LEFT JOIN projects p ON p.id = t.project_id \
    LEFT JOIN tickets tk ON tk.id = t.ticket_id";

fn load_task(conn: &Connection, id: i64) -> Result<TaskRow> {
    load_tasks(conn, "WHERE t.id = ?1", &[&id])?
        .into_iter()
        .next()
        .ok_or_else(|| FbError::NotFound(format!("{} not found", fmt_id(id))))
}

fn load_tasks(
    conn: &Connection,
    tail_sql: &str,
    p: &[&dyn rusqlite::ToSql],
) -> Result<Vec<TaskRow>> {
    let mut stmt = conn.prepare(&format!("{TASK_SELECT} {tail_sql}"))?;
    let raw = stmt
        .query_map(p, |r| {
            let project = match r.get::<_, Option<i64>>(14)? {
                Some(id) => Some(Project {
                    id,
                    key: r.get(15)?,
                    alias: r.get(16)?,
                }),
                None => None,
            };
            let ticket = match r.get::<_, Option<String>>(17)? {
                Some(system) => Some(Ticket {
                    system,
                    key: r.get(18)?,
                    title: r.get(19)?,
                    url: r.get(20)?,
                }),
                None => None,
            };
            Ok((
                r.get::<_, i64>(0)?,
                project,
                ticket,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, i64>(9)? != 0,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, String>(12)?,
                r.get::<_, String>(13)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    raw.into_iter()
        .map(
            |(
                id,
                project,
                ticket,
                title,
                goal,
                acceptance,
                plan,
                result,
                kind,
                status,
                priority,
                npa,
                paa,
                cb,
                ca,
                ua,
            )| {
                Ok(TaskRow {
                    id,
                    project,
                    ticket,
                    title,
                    goal,
                    acceptance,
                    plan,
                    result,
                    kind: Kind::parse(&kind)
                        .ok_or_else(|| FbError::Store(format!("bad kind {kind}")))?,
                    status: Status::parse(&status)
                        .ok_or_else(|| FbError::Store(format!("bad status {status}")))?,
                    priority,
                    needs_plan_approval: npa,
                    plan_approved_at: opt_ts(paa)?,
                    created_by: load_actor(conn, cb)?,
                    created_at: parse_ts(&ca)?,
                    updated_at: parse_ts(&ua)?,
                    blocked_by: blockers(conn, id)?,
                    claim: active_claim(conn, id)?,
                })
            },
        )
        .collect()
}

fn blockers(conn: &Connection, task_id: i64) -> Result<Vec<BlockerRef>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.status, b.title FROM deps d JOIN tasks b ON b.id = d.blocker_id \
         WHERE d.task_id = ?1 ORDER BY b.id",
    )?;
    let rows = stmt
        .query_map([task_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(id, status, title)| {
            Ok(BlockerRef {
                id,
                status: Status::parse(&status)
                    .ok_or_else(|| FbError::Store(format!("bad status {status}")))?,
                title,
            })
        })
        .collect()
}

fn ready_ids(conn: &Connection, project_id: Option<i64>) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT t.id FROM tasks t WHERE t.status = 'todo' \
         AND (?1 IS NULL OR t.project_id = ?1) \
         AND NOT EXISTS (SELECT 1 FROM deps d JOIN tasks b ON b.id = d.blocker_id \
                         WHERE d.task_id = t.id AND b.status <> 'done') \
         ORDER BY t.priority, t.created_at, t.id",
    )?;
    let ids = stmt
        .query_map([project_id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok(ids)
}

fn event(
    tx: &Connection,
    task_id: Option<i64>,
    actor: &Actor,
    ty: &str,
    mut data: Value,
    ctx: &Ctx,
) -> Result<()> {
    if let Some(src) = &ctx.source {
        match &mut data {
            Value::Object(map) => {
                map.insert("source".to_string(), json!(src));
            }
            Value::Null => {
                data = json!({ "source": src });
            }
            _ => {}
        }
    }
    tx.execute(
        "INSERT INTO events (task_id, actor_id, type, data, pid, ppid, proc_chain, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            task_id,
            actor.id,
            ty,
            data.to_string(),
            ctx.pid,
            ctx.ppid,
            ctx.proc_chain,
            ts(ctx.now)
        ],
    )?;
    Ok(())
}

fn bump(tx: &Connection, id: i64, ctx: &Ctx) -> Result<()> {
    tx.execute(
        "UPDATE tasks SET updated_at = ?1 WHERE id = ?2",
        params![ts(ctx.now), id],
    )?;
    Ok(())
}

fn set_status(tx: &Connection, id: i64, status: Status, ctx: &Ctx) -> Result<()> {
    tx.execute(
        "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status.as_str(), ts(ctx.now), id],
    )?;
    Ok(())
}

fn release_claim(tx: &Connection, task_id: i64, reason: &str, ctx: &Ctx) -> Result<()> {
    tx.execute(
        "UPDATE claims SET released_at = ?1, release_reason = ?2 \
         WHERE task_id = ?3 AND released_at IS NULL",
        params![ts(ctx.now), reason, task_id],
    )?;
    Ok(())
}

fn project_by(conn: &Connection, key_or_alias: &str) -> Result<Option<Project>> {
    Ok(conn
        .query_row(
            "SELECT id, key, alias FROM projects WHERE key = ?1 \
             UNION ALL SELECT id, key, alias FROM projects WHERE alias = ?1 LIMIT 1",
            [key_or_alias],
            |r| {
                Ok(Project {
                    id: r.get(0)?,
                    key: r.get(1)?,
                    alias: r.get(2)?,
                })
            },
        )
        .optional()?)
}

fn ensure_project(tx: &Connection, key: &str, ctx: &Ctx) -> Result<Project> {
    if let Some(p) = project_by(tx, key)? {
        return Ok(p);
    }
    tx.execute(
        "INSERT INTO projects (key, created_at) VALUES (?1, ?2)",
        params![key, ts(ctx.now)],
    )?;
    Ok(Project {
        id: tx.last_insert_rowid(),
        key: key.to_string(),
        alias: None,
    })
}

/// A string that can only be a project key, not an alias.
fn looks_like_key(s: &str) -> bool {
    s.contains('/') || s.starts_with("dir:")
}

fn is_claimant(task: &TaskRow, actor: &Actor) -> bool {
    task.claim.as_ref().is_some_and(|c| c.actor.id == actor.id)
}

fn require_claimant(task: &TaskRow, actor: &Actor, what: &str) -> Result<()> {
    if is_claimant(task, actor) || actor.role() == Role::Human {
        return Ok(());
    }
    let held = match &task.claim {
        Some(c) => format!("claimed by {}", c.actor.label),
        None => "not claimed".to_string(),
    };
    Err(FbError::NotClaimant(format!(
        "only the claimant can {what} {} ({held})",
        fmt_id(task.id)
    )))
}

fn non_empty(s: &str, what: &str) -> Result<()> {
    if s.trim().is_empty() {
        return Err(FbError::Usage(format!("{what} must not be empty")));
    }
    Ok(())
}

impl Backend for SqliteBackend {
    fn resolve_actor(&self, caller: &Caller, ctx: &Ctx) -> Result<Actor> {
        self.write(|tx| {
            let (kind, provider) = match caller.kind {
                ActorKind::Human => ("human", None),
                ActorKind::Agent => (
                    "agent",
                    Some(caller.provider.clone().unwrap_or_else(|| "unknown".into())),
                ),
            };
            let now = ts(ctx.now);
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM actors WHERE kind = ?1 AND external_id = ?2 AND provider IS ?3",
                    params![kind, caller.external_id, provider],
                    |r| r.get(0),
                )
                .optional()?;
            let id = match existing {
                Some(id) => {
                    tx.execute(
                        "UPDATE actors SET label = ?1, last_seen = ?2 WHERE id = ?3",
                        params![caller.label, now, id],
                    )?;
                    id
                }
                None => {
                    tx.execute(
                        "INSERT INTO actors (kind, provider, external_id, label, first_seen, last_seen) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                        params![kind, provider, caller.external_id, caller.label, now],
                    )?;
                    tx.last_insert_rowid()
                }
            };
            load_actor(tx, id)
        })
    }

    fn touch(&self, actor: &Actor, loc: Option<&Location>, ctx: &Ctx) -> Result<()> {
        let host = loc.map(|l| l.host.clone()).unwrap_or_default();
        self.write(|tx| {
            match loc {
                Some(l) => tx.execute(
                    "UPDATE claims SET last_activity = ?1, cwd = ?2, branch = COALESCE(?3, branch) \
                     WHERE actor_id = ?4 AND released_at IS NULL AND host = ?5",
                    params![ts(ctx.now), l.cwd, l.branch, actor.id, l.host],
                )?,
                None => tx.execute(
                    "UPDATE claims SET last_activity = ?1 \
                     WHERE actor_id = ?2 AND released_at IS NULL AND (?3 = '' OR host = ?3)",
                    params![ts(ctx.now), actor.id, host],
                )?,
            };
            Ok(())
        })
    }

    fn find_actor(&self, spec: &str) -> Result<Actor> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ACTOR_COLS} FROM actors WHERE CAST(id AS TEXT) = ?1 OR label = ?1 OR external_id = ?1"
        ))?;
        let mut found = stmt
            .query_map([spec], |r| actor_from(r, 0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        match found.len() {
            0 => Err(FbError::NotFound(format!("no actor matches {spec:?}"))),
            1 => Ok(found.remove(0)),
            _ => Err(FbError::Usage(format!(
                "{spec:?} matches {} actors; use the numeric actor id",
                found.len()
            ))),
        }
    }

    fn set_trusted(
        &self,
        actor: &Actor,
        target: &Actor,
        trusted: bool,
        ctx: &Ctx,
    ) -> Result<Actor> {
        model::require_human(actor.role(), if trusted { "trust" } else { "untrust" })?;
        if target.is_human() {
            return Err(FbError::Usage(
                "the human actor cannot be trusted or untrusted".into(),
            ));
        }
        self.write(|tx| {
            tx.execute(
                "UPDATE actors SET trusted = ?1 WHERE id = ?2",
                params![trusted as i64, target.id],
            )?;
            event(
                tx,
                None,
                actor,
                if trusted { "trusted" } else { "untrusted" },
                json!({ "actor": target.id, "label": target.label }),
                ctx,
            )?;
            load_actor(tx, target.id)
        })
    }

    fn delegate(
        &self,
        actor: &Actor,
        target: &Actor,
        allowed: &[String],
        ctx: &Ctx,
    ) -> Result<Vec<String>> {
        model::require_human(actor.role(), "delegate")?;
        if target.is_human() {
            return Err(FbError::Usage(
                "the human actor cannot be delegated or undelegated".into(),
            ));
        }
        if allowed.is_empty() {
            return Err(FbError::Usage(
                "--allow requires at least one action".into(),
            ));
        }
        for a in allowed {
            if !model::is_delegatable(a) {
                return Err(FbError::Usage(format!(
                    "bad action {a:?} to delegate (must be one of: {})",
                    model::DELEGATABLE_ACTIONS.join(", ")
                )));
            }
        }
        self.write(|tx| {
            for a in allowed {
                tx.execute(
                    "INSERT INTO delegations (actor_id, action, created_at) VALUES (?1, ?2, ?3) \
                     ON CONFLICT (actor_id, action) DO NOTHING",
                    params![target.id, a, ts(ctx.now)],
                )?;
            }
            event(
                tx,
                None,
                actor,
                "delegated",
                json!({ "actor": target.id, "label": target.label, "allowed": allowed }),
                ctx,
            )?;
            tx_get_delegations(tx, target.id)
        })
    }

    fn undelegate(
        &self,
        actor: &Actor,
        target: &Actor,
        revoked: Option<&[String]>,
        ctx: &Ctx,
    ) -> Result<Vec<String>> {
        model::require_human(actor.role(), "undelegate")?;
        if target.is_human() {
            return Err(FbError::Usage(
                "the human actor cannot be delegated or undelegated".into(),
            ));
        }
        if let Some(rev) = revoked {
            for a in rev {
                if !model::is_delegatable(a) {
                    return Err(FbError::Usage(format!(
                        "bad action {a:?} to undelegate (must be one of: {})",
                        model::DELEGATABLE_ACTIONS.join(", ")
                    )));
                }
            }
        }
        self.write(|tx| {
            let revoked_list = match revoked {
                Some(rev) => {
                    for a in rev {
                        tx.execute(
                            "DELETE FROM delegations WHERE actor_id = ?1 AND action = ?2",
                            params![target.id, a],
                        )?;
                    }
                    rev.to_vec()
                }
                None => {
                    let all = tx_get_delegations(tx, target.id)?;
                    tx.execute(
                        "DELETE FROM delegations WHERE actor_id = ?1",
                        params![target.id],
                    )?;
                    all
                }
            };
            event(
                tx,
                None,
                actor,
                "undelegated",
                json!({ "actor": target.id, "label": target.label, "revoked": revoked_list }),
                ctx,
            )?;
            tx_get_delegations(tx, target.id)
        })
    }

    fn get_delegations(&self, actor_id: i64) -> Result<Vec<String>> {
        tx_get_delegations(&self.conn, actor_id)
    }

    fn find_project(&self, key_or_alias: &str) -> Result<Option<Project>> {
        project_by(&self.conn, key_or_alias)
    }

    fn set_alias(
        &self,
        actor: &Actor,
        key_or_alias: &str,
        alias: &str,
        ctx: &Ctx,
    ) -> Result<Project> {
        model::require_human(actor.role(), "project alias")?;
        non_empty(alias, "alias")?;
        self.write(|tx| {
            let p = project_by(tx, key_or_alias)?
                .ok_or_else(|| FbError::NotFound(format!("no project {key_or_alias:?}")))?;
            tx.execute(
                "UPDATE projects SET alias = ?1 WHERE id = ?2",
                params![alias, p.id],
            )?;
            event(
                tx,
                None,
                actor,
                "project_alias",
                json!({ "key": p.key, "alias": alias }),
                ctx,
            )?;
            Ok(Project {
                alias: Some(alias.to_string()),
                ..p
            })
        })
    }

    fn add_task(&self, actor: &Actor, new: &NewTask, ctx: &Ctx) -> Result<TaskRow> {
        non_empty(&new.title, "title")?;
        if !(0..=3).contains(&new.priority) {
            return Err(FbError::Usage("--prio must be 0-3".into()));
        }
        self.write(|tx| {
            let project_id = match &new.project {
                None => None,
                Some(ProjectSel::Detected(key)) => Some(ensure_project(tx, key, ctx)?.id),
                Some(ProjectSel::Explicit(sel)) => match project_by(tx, sel)? {
                    Some(p) => Some(p.id),
                    None if looks_like_key(sel) => Some(ensure_project(tx, sel, ctx)?.id),
                    None => {
                        return Err(FbError::NotFound(format!(
                            "no project {sel:?} (use a full key like host/org/repo to create one)"
                        )));
                    }
                },
            };
            let ticket_id = match &new.ticket {
                None => None,
                Some(t) => {
                    tx.execute(
                        "INSERT INTO tickets (system, key, title, url, created_at) VALUES (?1, ?2, ?3, ?4, ?5) \
                         ON CONFLICT(system, key) DO UPDATE SET \
                           title = COALESCE(excluded.title, title), url = COALESCE(excluded.url, url)",
                        params![t.system, t.key, t.title, t.url, ts(ctx.now)],
                    )?;
                    Some(tx.query_row(
                        "SELECT id FROM tickets WHERE system = ?1 AND key = ?2",
                        params![t.system, t.key],
                        |r| r.get::<_, i64>(0),
                    )?)
                }
            };
            for b in &new.blocked_by {
                load_task(tx, *b)?;
            }
            let status = model::initial_status(actor.role());
            let now = ts(ctx.now);
            tx.execute(
                "INSERT INTO tasks (project_id, ticket_id, title, goal, acceptance, kind, status, priority, \
                 needs_plan_approval, created_by, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    project_id,
                    ticket_id,
                    new.title,
                    new.goal,
                    new.acceptance,
                    new.kind.as_str(),
                    status.as_str(),
                    new.priority,
                    new.needs_plan_approval as i64,
                    actor.id,
                    now
                ],
            )?;
            let id = tx.last_insert_rowid();
            for b in &new.blocked_by {
                tx.execute(
                    "INSERT OR IGNORE INTO deps (task_id, blocker_id) VALUES (?1, ?2)",
                    params![id, b],
                )?;
            }
            event(tx, Some(id), actor, "created", json!({ "status": status.as_str() }), ctx)?;
            load_task(tx, id)
        })
    }

    fn get_task(&self, id: i64) -> Result<TaskRow> {
        load_task(&self.conn, id)
    }

    fn task_detail(&self, id: i64) -> Result<TaskDetail> {
        let task = load_task(&self.conn, id)?;
        let claims = load_claims(&self.conn, "task_id = ?1", &[&id])?;
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, actor_id, kind, body, created_at FROM notes WHERE task_id = ?1 ORDER BY id",
        )?;
        let raw = stmt
            .query_map([id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let notes = raw
            .into_iter()
            .map(|(id, task_id, actor_id, kind, body, created)| {
                Ok(NoteRow {
                    id,
                    task_id,
                    actor: load_actor(&self.conn, actor_id)?,
                    kind,
                    body,
                    created_at: parse_ts(&created)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let history = self.task_events(id)?;
        Ok(TaskDetail {
            task,
            claims,
            notes,
            history,
        })
    }

    fn list_tasks(&self, f: &TaskFilter) -> Result<Vec<TaskRow>> {
        let mut conds: Vec<String> = Vec::new();
        if !f.statuses.is_empty() {
            let list: Vec<String> = f
                .statuses
                .iter()
                .map(|s| format!("'{}'", s.as_str()))
                .collect();
            conds.push(format!("t.status IN ({})", list.join(",")));
        } else if !f.include_terminal {
            conds.push("t.status NOT IN ('done','dropped')".into());
        }
        if let Some(p) = f.project_id {
            conds.push(format!("t.project_id = {p}"));
        }
        if let Some(a) = f.claimant {
            conds.push(format!(
                "EXISTS (SELECT 1 FROM claims c WHERE c.task_id = t.id AND c.released_at IS NULL AND c.actor_id = {a})"
            ));
        }
        let where_sql = if conds.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conds.join(" AND "))
        };
        load_tasks(&self.conn, &format!("{where_sql} ORDER BY t.id"), &[])
    }

    fn ready(&self, project_id: Option<i64>) -> Result<Vec<TaskRow>> {
        ready_ids(&self.conn, project_id)?
            .into_iter()
            .map(|id| load_task(&self.conn, id))
            .collect()
    }

    fn claim(&self, actor: &Actor, req: &ClaimRequest, ctx: &Ctx) -> Result<TaskRow> {
        if req.steal && req.id.is_none() {
            return Err(FbError::Usage("--steal needs a task id".into()));
        }
        if req.for_actor.is_some() && !req.steal {
            return Err(FbError::Usage("--for only works with --steal".into()));
        }
        let action = if req.steal {
            Action::Steal
        } else {
            Action::Claim
        };
        let is_delegated_steal =
            !actor.is_human() && req.steal && self.has_delegation(actor.id, "steal")?;
        model::authorize_with_delegation(action, actor.role(), is_delegated_steal)?;
        self.write(|tx| {
            let id = match req.id {
                Some(id) => id,
                None => *ready_ids(tx, req.scope_project)?
                    .first()
                    .ok_or_else(|| FbError::NotFound("no ready task to claim".into()))?,
            };
            let task = load_task(tx, id)?;
            let label = fmt_id(id);
            let new_claimant = req.for_actor.as_ref().unwrap_or(actor);

            if req.steal {
                model::transition(task.status, Action::Steal)?;
                let old = task.claim.as_ref().ok_or_else(|| {
                    FbError::BadTransition(format!("{label} has no active claim to steal"))
                })?;
                release_claim(tx, id, "stolen", ctx)?;
                // `--for` hands over the old location; the new claimant refreshes it on its next call.
                let (host, cwd, branch, base) = if req.for_actor.is_some() {
                    (
                        old.host.clone(),
                        old.cwd.clone(),
                        old.branch.clone(),
                        old.base_commit.clone(),
                    )
                } else {
                    (
                        req.loc.host.clone(),
                        req.loc.cwd.clone(),
                        req.loc.branch.clone(),
                        req.loc.head.clone(),
                    )
                };
                insert_claim(tx, id, new_claimant.id, &host, &cwd, branch, base, ctx)?;
                bump(tx, id, ctx)?;
                let is_delegated = !actor.is_human() && tx_has_delegation(tx, actor.id, "steal")?;
                let mut stolen_data = json!({ "from": old.actor.id, "to": new_claimant.id });
                if is_delegated {
                    stolen_data["on_behalf_of"] = json!("human");
                }
                event(tx, Some(id), actor, "stolen", stolen_data, ctx)?;
                return load_task(tx, id);
            }

            // An active claim blocks everyone else, whatever the status (in_progress or review).
            if let Some(held) = &task.claim {
                if held.actor.id == actor.id {
                    return Ok(task); // already yours: claiming again is a no-op
                }
                return Err(FbError::ClaimHeld(format!(
                    "{label} is claimed by {}",
                    held.actor.label
                )));
            }
            match task.status {
                Status::Draft => {
                    return Err(FbError::NotReady(format!(
                        "{label} is a draft; a human must promote it first"
                    )));
                }
                Status::Todo => {
                    let open: Vec<String> = task
                        .blocked_by
                        .iter()
                        .filter(|b| b.status != Status::Done)
                        .map(|b| fmt_id(b.id))
                        .collect();
                    if !open.is_empty() {
                        return Err(FbError::NotReady(format!(
                            "{label} is blocked by {}",
                            open.join(", ")
                        )));
                    }
                }
                _ => {}
            }
            let next = model::transition(task.status, Action::Claim)?;
            insert_claim(
                tx,
                id,
                actor.id,
                &req.loc.host,
                &req.loc.cwd,
                req.loc.branch.clone(),
                req.loc.head.clone(),
                ctx,
            )?;
            set_status(tx, id, next, ctx)?;
            event(
                tx,
                Some(id),
                actor,
                "claimed",
                json!({ "host": req.loc.host, "cwd": req.loc.cwd, "branch": req.loc.branch }),
                ctx,
            )?;
            load_task(tx, id)
        })
    }

    fn apply(&self, actor: &Actor, id: i64, cmd: &Command, ctx: &Ctx) -> Result<TaskRow> {
        self.write(|tx| {
            let task = load_task(tx, id)?;
            let label = fmt_id(id);
            let role = actor.role();
            match cmd {
                Command::Promote => {
                    let delegated =
                        !actor.is_human() && tx_has_delegation(tx, actor.id, "promote")?;
                    model::authorize_with_delegation(Action::Promote, role, delegated)?;
                    let to = model::transition(task.status, Action::Promote)?;
                    set_status(tx, id, to, ctx)?;
                    let mut data = json!({});
                    if delegated {
                        data["on_behalf_of"] = json!("human");
                    }
                    event(tx, Some(id), actor, "promoted", data, ctx)?;
                }
                Command::Approve => {
                    let delegated =
                        !actor.is_human() && tx_has_delegation(tx, actor.id, "approve")?;
                    model::authorize_with_delegation(Action::Approve, role, delegated)?;
                    model::transition(task.status, Action::Approve)?;
                    if !(task.needs_plan_approval && task.plan.is_some())
                        || task.plan_approved_at.is_some()
                    {
                        return Err(FbError::BadTransition(format!(
                            "{label} has no plan awaiting approval"
                        )));
                    }
                    tx.execute(
                        "UPDATE tasks SET plan_approved_at = ?1, updated_at = ?1 WHERE id = ?2",
                        params![ts(ctx.now), id],
                    )?;
                    let mut data = json!({});
                    if delegated {
                        data["on_behalf_of"] = json!("human");
                    }
                    event(tx, Some(id), actor, "approved", data, ctx)?;
                }
                Command::Release => {
                    let delegated =
                        !actor.is_human() && tx_has_delegation(tx, actor.id, "release")?;
                    model::authorize_with_delegation(Action::Release, role, delegated)?;
                    let to = model::transition(task.status, Action::Release)?;
                    release_claim(tx, id, "released", ctx)?;
                    set_status(tx, id, to, ctx)?;
                    let mut data = json!({});
                    if delegated {
                        data["on_behalf_of"] = json!("human");
                    }
                    event(tx, Some(id), actor, "released", data, ctx)?;
                }
                Command::Drop { reason } => {
                    let delegated = !actor.is_human() && tx_has_delegation(tx, actor.id, "drop")?;
                    model::authorize_with_delegation(Action::Drop, role, delegated)?;
                    let to = model::transition(task.status, Action::Drop)?;
                    release_claim(tx, id, "dropped", ctx)?;
                    set_status(tx, id, to, ctx)?;
                    let mut data = json!({ "reason": reason });
                    if delegated {
                        data["on_behalf_of"] = json!("human");
                    }
                    event(tx, Some(id), actor, "dropped", data, ctx)?;
                }
                Command::Reopen => {
                    let delegated =
                        !actor.is_human() && tx_has_delegation(tx, actor.id, "reopen")?;
                    model::authorize_with_delegation(Action::Reopen, role, delegated)?;
                    let to = model::transition(task.status, Action::Reopen)?;
                    set_status(tx, id, to, ctx)?;
                    let mut data = json!({});
                    if delegated {
                        data["on_behalf_of"] = json!("human");
                    }
                    event(tx, Some(id), actor, "reopened", data, ctx)?;
                }
                Command::Verify { force, commits } => {
                    let delegated =
                        !actor.is_human() && tx_has_delegation(tx, actor.id, "verify")?;
                    model::authorize_with_delegation(Action::Verify, role, delegated)?;
                    let to = model::transition(task.status, Action::Verify)?;
                    if task.kind == Kind::Code && !force && !commits.is_some_and(|c| c > 0) {
                        return Err(FbError::BadTransition(format!(
                            "{label} has no commit evidence (claimed, no commits); use --force to verify anyway"
                        )));
                    }
                    release_claim(tx, id, "done", ctx)?;
                    set_status(tx, id, to, ctx)?;
                    let mut data = json!({ "force": force, "commits": commits });
                    if delegated {
                        data["on_behalf_of"] = json!("human");
                    }
                    event(tx, Some(id), actor, "verified", data, ctx)?;
                }
                Command::Reject { comment } => {
                    let delegated =
                        !actor.is_human() && tx_has_delegation(tx, actor.id, "reject")?;
                    model::authorize_with_delegation(Action::Reject, role, delegated)?;
                    non_empty(comment, "reject comment")?;
                    let to = model::transition(task.status, Action::Reject)?;
                    insert_note(tx, id, actor, "comment", comment, ctx)?;
                    set_status(tx, id, to, ctx)?;
                    let mut data = json!({ "comment": comment });
                    if delegated {
                        data["on_behalf_of"] = json!("human");
                    }
                    event(tx, Some(id), actor, "rejected", data, ctx)?;
                }
                Command::Plan { text } => {
                    non_empty(text, "plan")?;
                    require_claimant(&task, actor, "write the plan of")?;
                    if task.status != Status::InProgress {
                        return Err(FbError::BadTransition(format!(
                            "cannot plan {label}: it is {}, not in_progress",
                            task.status.as_str()
                        )));
                    }
                    // A replaced plan needs a fresh approval.
                    tx.execute(
                        "UPDATE tasks SET plan = ?1, plan_approved_at = NULL, updated_at = ?2 WHERE id = ?3",
                        params![text, ts(ctx.now), id],
                    )?;
                    event(tx, Some(id), actor, "planned", json!({}), ctx)?;
                }
                Command::Done { result } => {
                    model::authorize(Action::Done, role)?;
                    require_claimant(&task, actor, "finish")?;
                    let to = model::transition(task.status, Action::Done)?;
                    if task.kind == Kind::Note && result.as_deref().is_none_or(|r| r.trim().is_empty()) {
                        return Err(FbError::Usage(format!("{label} is a note task: --result is required")));
                    }
                    model::check_done_gate(task.needs_plan_approval, task.plan_approved_at.is_some())?;
                    if let Some(r) = result {
                        tx.execute("UPDATE tasks SET result = ?1 WHERE id = ?2", params![r, id])?;
                    }
                    set_status(tx, id, to, ctx)?;
                    event(tx, Some(id), actor, "done", json!({}), ctx)?;
                }
                Command::Note { kind, body } => {
                    non_empty(body, "note")?;
                    if !["handoff", "comment", "finding"].contains(&kind.as_str()) {
                        return Err(FbError::Usage(format!(
                            "bad note kind {kind:?} (handoff | comment | finding)"
                        )));
                    }
                    insert_note(tx, id, actor, kind, body, ctx)?;
                    bump(tx, id, ctx)?;
                    event(tx, Some(id), actor, "noted", json!({ "kind": kind }), ctx)?;
                }
            }
            load_task(tx, id)
        })
    }

    fn dep_add(&self, actor: &Actor, task: i64, blocker: i64, ctx: &Ctx) -> Result<TaskRow> {
        self.write(|tx| {
            load_task(tx, task)?;
            load_task(tx, blocker)?;
            if task == blocker {
                return Err(FbError::DepCycle(format!(
                    "{} cannot block itself",
                    fmt_id(task)
                )));
            }
            // Cycle if `task` is already reachable from `blocker` through blockers.
            let cycle: bool = tx.query_row(
                "WITH RECURSIVE r(id) AS (SELECT blocker_id FROM deps WHERE task_id = ?1 \
                   UNION SELECT d.blocker_id FROM deps d JOIN r ON d.task_id = r.id) \
                 SELECT EXISTS (SELECT 1 FROM r WHERE id = ?2)",
                params![blocker, task],
                |r| r.get(0),
            )?;
            if cycle {
                return Err(FbError::DepCycle(format!(
                    "{} already depends on {}; adding this would make a cycle",
                    fmt_id(blocker),
                    fmt_id(task)
                )));
            }
            tx.execute(
                "INSERT OR IGNORE INTO deps (task_id, blocker_id) VALUES (?1, ?2)",
                params![task, blocker],
            )?;
            bump(tx, task, ctx)?;
            event(
                tx,
                Some(task),
                actor,
                "dep_added",
                json!({ "blocker": blocker }),
                ctx,
            )?;
            load_task(tx, task)
        })
    }

    fn dep_rm(&self, actor: &Actor, task: i64, blocker: i64, ctx: &Ctx) -> Result<TaskRow> {
        self.write(|tx| {
            load_task(tx, task)?;
            let n = tx.execute(
                "DELETE FROM deps WHERE task_id = ?1 AND blocker_id = ?2",
                params![task, blocker],
            )?;
            if n == 0 {
                return Err(FbError::NotFound(format!(
                    "{} does not depend on {}",
                    fmt_id(task),
                    fmt_id(blocker)
                )));
            }
            bump(tx, task, ctx)?;
            event(
                tx,
                Some(task),
                actor,
                "dep_removed",
                json!({ "blocker": blocker }),
                ctx,
            )?;
            load_task(tx, task)
        })
    }

    fn save_snapshot(&self, claim_id: i64, json: &str, now: DateTime<Utc>) -> Result<()> {
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO evidence_snapshots (claim_id, data, computed_at) VALUES (?1, ?2, ?3)",
                params![claim_id, json, ts(now)],
            )?;
            Ok(())
        })
    }

    fn snapshot(&self, claim_id: i64) -> Result<Option<(String, DateTime<Utc>)>> {
        let raw: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT data, computed_at FROM evidence_snapshots WHERE claim_id = ?1",
                [claim_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        raw.map(|(d, t)| Ok((d, parse_ts(&t)?))).transpose()
    }

    fn all_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, key, alias FROM projects ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                key: r.get(1)?,
                alias: r.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn all_actors(&self) -> Result<Vec<Actor>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, provider, external_id, label, trusted FROM actors ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Actor {
                id: r.get(0)?,
                kind: r.get(1)?,
                provider: r.get(2)?,
                external_id: r.get(3)?,
                label: r.get(4)?,
                trusted: r.get::<_, i64>(5)? != 0,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn all_instances(&self) -> Result<Vec<InstanceRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT c.host, c.cwd, c.branch, c.workspace_id, \
             a.id, a.kind, a.provider, a.external_id, a.label, a.trusted, \
             p.id, p.key, p.alias \
             FROM claims c \
             JOIN actors a ON a.id = c.actor_id \
             JOIN tasks t ON t.id = c.task_id \
             LEFT JOIN projects p ON p.id = t.project_id \
             ORDER BY c.host, c.cwd, c.branch",
        )?;
        let rows = stmt.query_map([], |r| {
            let p_id: Option<i64> = r.get(10)?;
            let project = p_id.map(|id| Project {
                id,
                key: r.get(11).unwrap_or_default(),
                alias: r.get(12).unwrap_or_default(),
            });
            Ok(InstanceRow {
                host: r.get(0)?,
                cwd: r.get(1)?,
                branch: r.get(2)?,
                workspace_id: r.get(3)?,
                actor: Actor {
                    id: r.get(4)?,
                    kind: r.get(5)?,
                    provider: r.get(6)?,
                    external_id: r.get(7)?,
                    label: r.get(8)?,
                    trusted: r.get::<_, i64>(9)? != 0,
                },
                project,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn task_events(&self, task_id: i64) -> Result<Vec<EventRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.task_id, e.type, e.data, e.pid, e.ppid, e.proc_chain, e.created_at, \
             a.id, a.kind, a.provider, a.external_id, a.label, a.trusted \
             FROM events e \
             JOIN actors a ON a.id = e.actor_id \
             WHERE e.task_id = ?1 \
             ORDER BY e.id ASC",
        )?;
        let rows = stmt.query_map([task_id], |r| {
            let raw_data: Option<String> = r.get(3)?;
            let data = raw_data.and_then(|s| serde_json::from_str::<Value>(&s).ok());
            let on_behalf_of = data
                .as_ref()
                .and_then(|v| v.get("on_behalf_of"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let source = data
                .as_ref()
                .and_then(|v| v.get("source"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let created_at_str: String = r.get(7)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(EventRow {
                id: r.get(0)?,
                task_id: r.get(1)?,
                event_type: r.get(2)?,
                data,
                pid: r.get::<_, i64>(4)? as u32,
                ppid: r.get::<_, i64>(5)? as u32,
                proc_chain: r.get(6)?,
                created_at,
                on_behalf_of,
                source,
                actor: Actor {
                    id: r.get(8)?,
                    kind: r.get(9)?,
                    provider: r.get(10)?,
                    external_id: r.get(11)?,
                    label: r.get(12)?,
                    trusted: r.get::<_, i64>(13)? != 0,
                },
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn data_version(&self) -> Result<i64> {
        self.conn
            .query_row("PRAGMA data_version", [], |r| r.get(0))
            .map_err(Into::into)
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_claim(
    tx: &Connection,
    task_id: i64,
    actor_id: i64,
    host: &str,
    cwd: &str,
    branch: Option<String>,
    base: Option<String>,
    ctx: &Ctx,
) -> Result<()> {
    let now = ts(ctx.now);
    tx.execute(
        "INSERT INTO claims (task_id, actor_id, host, cwd, branch, base_commit, claimed_at, last_activity) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![task_id, actor_id, host, cwd, branch, base, now],
    )?;
    Ok(())
}

fn insert_note(
    tx: &Connection,
    task_id: i64,
    actor: &Actor,
    kind: &str,
    body: &str,
    ctx: &Ctx,
) -> Result<()> {
    tx.execute(
        "INSERT INTO notes (task_id, actor_id, kind, body, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![task_id, actor.id, kind, body, ts(ctx.now)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Caller;
    use std::sync::{Arc, Barrier};

    fn ctx() -> Ctx {
        Ctx {
            now: Utc::now(),
            pid: 1,
            ppid: 0,
            proc_chain: "test".into(),
            source: None,
        }
    }

    fn loc() -> Location {
        Location {
            host: "h".into(),
            cwd: "/w".into(),
            branch: Some("main".into()),
            head: Some("abc".into()),
        }
    }

    fn caller(id: &str) -> Caller {
        Caller {
            kind: ActorKind::Agent,
            provider: Some("codex".into()),
            external_id: id.into(),
            label: format!("codex {id}"),
        }
    }

    fn human(db: &SqliteBackend) -> Actor {
        db.resolve_actor(
            &Caller {
                kind: ActorKind::Human,
                provider: None,
                external_id: "human".into(),
                label: "human".into(),
            },
            &ctx(),
        )
        .unwrap()
    }

    fn agent(db: &SqliteBackend, id: &str) -> Actor {
        db.resolve_actor(&caller(id), &ctx()).unwrap()
    }

    fn new_task(title: &str) -> NewTask {
        NewTask {
            title: title.into(),
            goal: None,
            acceptance: None,
            kind: Kind::Code,
            priority: 2,
            blocked_by: vec![],
            ticket: None,
            project: None,
            needs_plan_approval: false,
        }
    }

    fn claim_req(id: Option<i64>) -> ClaimRequest {
        ClaimRequest {
            id,
            steal: false,
            for_actor: None,
            loc: loc(),
            scope_project: None,
        }
    }

    #[test]
    fn schema_version_wal_and_seeded_human() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.db");
        let db = SqliteBackend::open(&path).unwrap();
        assert_eq!(user_version(&db.conn).unwrap(), SCHEMA_VERSION);
        let mode: String = db
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let h = human(&db);
        assert_eq!(h.id, 1, "seeded row is reused, not duplicated");
        assert!(h.is_human());
        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM actors WHERE kind='human'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 1);
        drop(db);
        // Reopening is a no-op migration.
        SqliteBackend::open(&path).unwrap();
    }

    #[test]
    fn newer_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.db");
        drop(SqliteBackend::open(&path).unwrap());
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(conn);
        match SqliteBackend::open(&path) {
            Err(FbError::SchemaAhead { found, supported }) => {
                assert_eq!((found, supported), (SCHEMA_VERSION + 1, SCHEMA_VERSION));
            }
            other => panic!("expected SchemaAhead, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn agent_tasks_are_drafts_human_tasks_are_todo() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let a = agent(&db, "a1");
        let h = human(&db);
        assert_eq!(
            db.add_task(&a, &new_task("x"), &ctx()).unwrap().status,
            Status::Draft
        );
        assert_eq!(
            db.add_task(&h, &new_task("y"), &ctx()).unwrap().status,
            Status::Todo
        );
        db.set_trusted(&h, &a, true, &ctx()).unwrap();
        let a = db.find_actor("codex a1").unwrap();
        assert!(a.trusted);
        assert_eq!(
            db.add_task(&a, &new_task("z"), &ctx()).unwrap().status,
            Status::Todo
        );
    }

    #[test]
    fn agent_cannot_use_human_commands() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let a = agent(&db, "a1");
        let t = db.add_task(&a, &new_task("x"), &ctx()).unwrap();
        for cmd in [
            Command::Promote,
            Command::Release,
            Command::Reopen,
            Command::Approve,
        ] {
            let e = db.apply(&a, t.id, &cmd, &ctx()).unwrap_err();
            assert_eq!(e.code(), "HUMAN_ONLY", "{cmd:?}");
        }
        assert_eq!(
            db.set_trusted(&a, &a, true, &ctx()).unwrap_err().code(),
            "HUMAN_ONLY"
        );
        assert_eq!(
            db.set_alias(&a, "x", "y", &ctx()).unwrap_err().code(),
            "HUMAN_ONLY"
        );
    }

    #[test]
    fn full_lifecycle_and_audit_log() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let a = agent(&db, "a1");
        let h = human(&db);
        let t = db.add_task(&a, &new_task("x"), &ctx()).unwrap();
        db.apply(&h, t.id, &Command::Promote, &ctx()).unwrap();
        let t = db.claim(&a, &claim_req(Some(t.id)), &ctx()).unwrap();
        assert_eq!(t.status, Status::InProgress);
        assert_eq!(
            t.claim.as_ref().unwrap().base_commit.as_deref(),
            Some("abc")
        );
        db.apply(
            &a,
            t.id,
            &Command::Note {
                kind: "handoff".into(),
                body: "wip".into(),
            },
            &ctx(),
        )
        .unwrap();
        let t = db
            .apply(&a, t.id, &Command::Done { result: None }, &ctx())
            .unwrap();
        assert_eq!(t.status, Status::Review);
        let t = db
            .apply(
                &h,
                t.id,
                &Command::Reject {
                    comment: "again".into(),
                },
                &ctx(),
            )
            .unwrap();
        assert_eq!(t.status, Status::InProgress);
        assert_eq!(
            t.claim.as_ref().unwrap().actor.id,
            a.id,
            "claim stays with the agent"
        );
        db.apply(&a, t.id, &Command::Done { result: None }, &ctx())
            .unwrap();
        let e = db
            .apply(
                &h,
                t.id,
                &Command::Verify {
                    force: false,
                    commits: Some(0),
                },
                &ctx(),
            )
            .unwrap_err();
        assert_eq!(e.code(), "BAD_TRANSITION");
        let t = db
            .apply(
                &h,
                t.id,
                &Command::Verify {
                    force: false,
                    commits: Some(2),
                },
                &ctx(),
            )
            .unwrap();
        assert_eq!(t.status, Status::Done);
        assert!(t.claim.is_none());
        let d = db.task_detail(t.id).unwrap();
        assert_eq!(d.claims[0].release_reason.as_deref(), Some("done"));
        assert_eq!(d.notes.len(), 2);
        let types: Vec<String> = db
            .conn
            .prepare("SELECT type FROM events ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            types,
            [
                "created", "promoted", "claimed", "noted", "done", "rejected", "done", "verified"
            ]
        );
        let (pid, chain): (i64, String) = db
            .conn
            .query_row(
                "SELECT pid, proc_chain FROM events WHERE type='verified'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((pid, chain.as_str()), (1, "test"));
    }

    #[test]
    fn claim_rules() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let a = agent(&db, "a1");
        let b = agent(&db, "b2");
        let draft = db.add_task(&a, &new_task("d"), &ctx()).unwrap();
        assert_eq!(
            db.claim(&a, &claim_req(Some(draft.id)), &ctx())
                .unwrap_err()
                .code(),
            "NOT_READY"
        );
        let blocker = db.add_task(&h, &new_task("blocker"), &ctx()).unwrap();
        let mut nt = new_task("blocked");
        nt.blocked_by = vec![blocker.id];
        let blocked = db.add_task(&h, &nt, &ctx()).unwrap();
        let e = db
            .claim(&a, &claim_req(Some(blocked.id)), &ctx())
            .unwrap_err();
        assert_eq!(e.code(), "NOT_READY");
        assert!(e.to_string().contains("fb-2"), "{e}");
        db.claim(&a, &claim_req(Some(blocker.id)), &ctx()).unwrap();
        let e = db
            .claim(&b, &claim_req(Some(blocker.id)), &ctx())
            .unwrap_err();
        assert_eq!(e.code(), "CLAIM_HELD");
        assert!(e.to_string().contains("codex a1"), "{e}");
        // Claiming your own task again is a no-op.
        db.claim(&a, &claim_req(Some(blocker.id)), &ctx()).unwrap();
        // Steal: refused for agents, allowed for the human, and --for hands it over.
        let mut steal = claim_req(Some(blocker.id));
        steal.steal = true;
        assert_eq!(
            db.claim(&b, &steal, &ctx()).unwrap_err().code(),
            "HUMAN_ONLY"
        );
        steal.for_actor = Some(b.clone());
        let t = db.claim(&h, &steal, &ctx()).unwrap();
        assert_eq!(t.claim.unwrap().actor.id, b.id);
        let d = db.task_detail(blocker.id).unwrap();
        assert_eq!(d.claims[1].release_reason.as_deref(), Some("stolen"));
    }

    #[test]
    fn claim_without_id_takes_highest_priority_oldest_ready() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let a = agent(&db, "a1");
        let mut low = new_task("low");
        low.priority = 3;
        let low = db.add_task(&h, &low, &ctx()).unwrap();
        let first = db.add_task(&h, &new_task("first"), &ctx()).unwrap();
        let mut urgent = new_task("urgent");
        urgent.priority = 0;
        let urgent = db.add_task(&h, &urgent, &ctx()).unwrap();
        let got = db.claim(&a, &claim_req(None), &ctx()).unwrap();
        assert_eq!(got.id, urgent.id);
        let got = db.claim(&a, &claim_req(None), &ctx()).unwrap();
        assert_eq!(got.id, first.id);
        let got = db.claim(&a, &claim_req(None), &ctx()).unwrap();
        assert_eq!(got.id, low.id);
        assert_eq!(
            db.claim(&a, &claim_req(None), &ctx()).unwrap_err().code(),
            "NOT_FOUND"
        );
    }

    #[test]
    fn concurrent_claims_have_one_winner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.db");
        let id = {
            let db = SqliteBackend::open(&path).unwrap();
            let h = human(&db);
            db.add_task(&h, &new_task("contested"), &ctx()).unwrap().id
        };
        const N: usize = 8;
        let barrier = Arc::new(Barrier::new(N));
        let handles: Vec<_> = (0..N)
            .map(|i| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let db = SqliteBackend::open(&path).unwrap();
                    let a = agent(&db, &format!("t{i}"));
                    barrier.wait();
                    db.claim(&a, &claim_req(Some(id)), &ctx())
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let winners = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(winners, 1, "exactly one winner");
        for r in results.iter().filter_map(|r| r.as_ref().err()) {
            assert_eq!(r.code(), "CLAIM_HELD", "{r}");
        }
        let db = SqliteBackend::open(&path).unwrap();
        let active: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM claims WHERE released_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active, 1);
    }

    #[test]
    fn unique_index_is_the_backstop() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let t = db.add_task(&h, &new_task("x"), &ctx()).unwrap();
        db.claim(&h, &claim_req(Some(t.id)), &ctx()).unwrap();
        let e: FbError = db
            .conn
            .execute(
                "INSERT INTO claims (task_id, actor_id, host, cwd, claimed_at, last_activity) \
                 VALUES (?1, 1, 'h', '/', 'x', 'x')",
                [t.id],
            )
            .unwrap_err()
            .into();
        assert_eq!(e.code(), "CLAIM_HELD");
    }

    #[test]
    fn deps_refuse_cycles_and_drive_orphans() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let a = db.add_task(&h, &new_task("a"), &ctx()).unwrap().id;
        let b = db.add_task(&h, &new_task("b"), &ctx()).unwrap().id;
        let c = db.add_task(&h, &new_task("c"), &ctx()).unwrap().id;
        db.dep_add(&h, b, a, &ctx()).unwrap();
        db.dep_add(&h, c, b, &ctx()).unwrap();
        assert_eq!(
            db.dep_add(&h, a, c, &ctx()).unwrap_err().code(),
            "DEP_CYCLE"
        );
        assert_eq!(
            db.dep_add(&h, a, a, &ctx()).unwrap_err().code(),
            "DEP_CYCLE"
        );
        assert_eq!(
            db.dep_add(&h, a, 99, &ctx()).unwrap_err().code(),
            "NOT_FOUND"
        );
        let ready: Vec<i64> = db.ready(None).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(ready, [a]);
        db.apply(&h, a, &Command::Drop { reason: None }, &ctx())
            .unwrap();
        assert!(
            db.ready(None).unwrap().is_empty(),
            "dropped blocker keeps dependants blocked"
        );
        db.dep_rm(&h, b, a, &ctx()).unwrap();
        assert_eq!(db.ready(None).unwrap()[0].id, b);
        assert_eq!(db.dep_rm(&h, b, a, &ctx()).unwrap_err().code(), "NOT_FOUND");
    }

    #[test]
    fn plan_gate_and_note_results() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let a = agent(&db, "a1");
        let b = agent(&db, "b2");
        let mut nt = new_task("gated");
        nt.needs_plan_approval = true;
        let t = db.add_task(&h, &nt, &ctx()).unwrap();
        db.claim(&a, &claim_req(Some(t.id)), &ctx()).unwrap();
        let plan = Command::Plan {
            text: "do it".into(),
        };
        assert_eq!(
            db.apply(&b, t.id, &plan, &ctx()).unwrap_err().code(),
            "NOT_CLAIMANT"
        );
        db.apply(&a, t.id, &plan, &ctx()).unwrap();
        let done = Command::Done { result: None };
        assert_eq!(
            db.apply(&a, t.id, &done, &ctx()).unwrap_err().code(),
            "BAD_TRANSITION"
        );
        db.apply(&h, t.id, &Command::Approve, &ctx()).unwrap();
        assert_eq!(
            db.apply(&h, t.id, &Command::Approve, &ctx())
                .unwrap_err()
                .code(),
            "BAD_TRANSITION"
        );
        // A rewritten plan needs approval again.
        db.apply(&a, t.id, &plan, &ctx()).unwrap();
        assert!(db.get_task(t.id).unwrap().plan_approved_at.is_none());
        db.apply(&h, t.id, &Command::Approve, &ctx()).unwrap();
        db.apply(&a, t.id, &done, &ctx()).unwrap();

        let mut note = new_task("just a note");
        note.kind = Kind::Note;
        let n = db.add_task(&h, &note, &ctx()).unwrap();
        db.claim(&a, &claim_req(Some(n.id)), &ctx()).unwrap();
        assert_eq!(
            db.apply(&a, n.id, &done, &ctx()).unwrap_err().code(),
            "USAGE"
        );
        let t = db
            .apply(
                &a,
                n.id,
                &Command::Done {
                    result: Some("answer".into()),
                },
                &ctx(),
            )
            .unwrap();
        assert_eq!(t.result.as_deref(), Some("answer"));
        // Note tasks verify without evidence.
        db.apply(
            &h,
            n.id,
            &Command::Verify {
                force: false,
                commits: None,
            },
            &ctx(),
        )
        .unwrap();
    }

    #[test]
    fn touch_refreshes_location_of_own_claims_only() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let a = agent(&db, "a1");
        let b = agent(&db, "b2");
        let t1 = db.add_task(&h, &new_task("1"), &ctx()).unwrap();
        let t2 = db.add_task(&h, &new_task("2"), &ctx()).unwrap();
        db.claim(&a, &claim_req(Some(t1.id)), &ctx()).unwrap();
        db.claim(&b, &claim_req(Some(t2.id)), &ctx()).unwrap();
        let mut moved = loc();
        moved.branch = Some("feature".into());
        moved.cwd = "/w2".into();
        db.touch(&a, Some(&moved), &ctx()).unwrap();
        let c1 = db.get_task(t1.id).unwrap().claim.unwrap();
        assert_eq!(
            (c1.branch.as_deref(), c1.cwd.as_str()),
            (Some("feature"), "/w2")
        );
        assert_eq!(c1.base_commit.as_deref(), Some("abc"), "base never changes");
        let c2 = db.get_task(t2.id).unwrap().claim.unwrap();
        assert_eq!(
            (c2.branch.as_deref(), c2.cwd.as_str()),
            (Some("main"), "/w")
        );
    }

    #[test]
    fn project_ticket_and_alias() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let mut nt = new_task("x");
        nt.project = Some(ProjectSel::Detected("github.com/org/repo".into()));
        nt.ticket = Some(TicketSpec {
            system: "jira".into(),
            key: "ABC-1".into(),
            title: None,
            url: None,
        });
        let t = db.add_task(&h, &nt, &ctx()).unwrap();
        assert_eq!(t.project.as_ref().unwrap().display(), "repo");
        // A later add fills in the ticket title without a second row.
        nt.ticket = Some(TicketSpec {
            system: "jira".into(),
            key: "ABC-1".into(),
            title: Some("Parent".into()),
            url: None,
        });
        let t2 = db.add_task(&h, &nt, &ctx()).unwrap();
        assert_eq!(t2.ticket.unwrap().title.as_deref(), Some("Parent"));
        assert_eq!(
            db.get_task(t.id).unwrap().ticket.unwrap().title.as_deref(),
            Some("Parent")
        );
        let p = db
            .set_alias(&h, "github.com/org/repo", "board", &ctx())
            .unwrap();
        assert_eq!(p.display(), "board");
        nt.project = Some(ProjectSel::Explicit("board".into()));
        assert_eq!(
            db.add_task(&h, &nt, &ctx()).unwrap().project.unwrap().key,
            "github.com/org/repo"
        );
        nt.project = Some(ProjectSel::Explicit("nope".into()));
        assert_eq!(
            db.add_task(&h, &nt, &ctx()).unwrap_err().code(),
            "NOT_FOUND"
        );
    }

    #[test]
    fn delegation_store_operations() {
        let db = SqliteBackend::open_in_memory().unwrap();
        let h = human(&db);
        let a = agent(&db, "agent-1");

        // Human cannot be delegated
        assert_eq!(
            db.delegate(&h, &h, &["promote".into()], &ctx())
                .unwrap_err()
                .code(),
            "USAGE"
        );

        // Agent cannot delegate
        assert_eq!(
            db.delegate(&a, &a, &["promote".into()], &ctx())
                .unwrap_err()
                .code(),
            "HUMAN_ONLY"
        );

        // Invalid action
        assert_eq!(
            db.delegate(&h, &a, &["bogus".into()], &ctx())
                .unwrap_err()
                .code(),
            "USAGE"
        );

        // Empty allow list
        assert_eq!(
            db.delegate(&h, &a, &[], &ctx()).unwrap_err().code(),
            "USAGE"
        );

        // Delegate promote and verify
        let d = db
            .delegate(&h, &a, &["promote".into(), "verify".into()], &ctx())
            .unwrap();
        assert_eq!(d, vec!["promote".to_string(), "verify".to_string()]);
        assert_eq!(
            db.get_delegations(a.id).unwrap(),
            vec!["promote".to_string(), "verify".to_string()]
        );
        assert!(db.has_delegation(a.id, "promote").unwrap());
        assert!(db.has_delegation(a.id, "verify").unwrap());
        assert!(!db.has_delegation(a.id, "approve").unwrap());

        // Partial undelegate
        let rem = db
            .undelegate(&h, &a, Some(&["promote".into()]), &ctx())
            .unwrap();
        assert_eq!(rem, vec!["verify".to_string()]);
        assert!(!db.has_delegation(a.id, "promote").unwrap());
        assert!(db.has_delegation(a.id, "verify").unwrap());

        // Full undelegate
        let rem2 = db.undelegate(&h, &a, None, &ctx()).unwrap();
        assert!(rem2.is_empty());
        assert_eq!(db.get_delegations(a.id).unwrap().len(), 0);
    }
}
