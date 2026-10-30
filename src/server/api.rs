//! Read endpoints: GET /api/board, GET /api/tasks/:id.

use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use super::AppState;
use crate::gitinfo::{self, Evidence};
use crate::model::{self, Derived, Facts, Kind, Status};
use crate::store::{
    Actor, Backend, ClaimRequest, ClaimRow, Command, Ctx, FbError, Location, SqliteBackend,
    TaskFilter, TaskRow, fmt_id, parse_id,
};

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn internal_error(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(json!({
            "v": 1,
            "ok": false,
            "error": {
                "code": "INTERNAL",
                "message": e.to_string(),
            }
        })),
    )
        .into_response()
}

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

fn live_evidence(db: &SqliteBackend, host: &str, gh: bool, c: &ClaimRow, now: DateTime<Utc>) -> Ev {
    if c.host == host
        && let (Some(base), Some(branch)) = (&c.base_commit, &c.branch)
    {
        let e = gitinfo::evidence(Path::new(&c.cwd), base, branch, c.claimed_at, gh);
        if e.reachable {
            return Ev {
                ev: e,
                source: Source::Live,
                computed_at: Some(now),
            };
        }
    }
    if let Ok(Some((data, at))) = db.snapshot(c.id)
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
        ev: Evidence {
            commits: 0,
            rebased: false,
            diffstat: None,
            dirty: false,
            pr: None,
            reachable: false,
        },
        source: Source::None,
        computed_at: None,
    }
}

fn claim_is_stale(
    adapter: &dyn crate::adapters::RuntimeAdapter,
    c: &ClaimRow,
    host: &str,
    now: DateTime<Utc>,
    stale_after: chrono::Duration,
) -> bool {
    if let Some(status) = adapter.agent_status(&c.actor.external_id)
        && status == crate::adapters::AgentRuntimeStatus::Gone
    {
        return true;
    }
    let last_commit = if c.host == host {
        c.branch
            .as_deref()
            .and_then(|b| gitinfo::last_commit_time(Path::new(&c.cwd), b))
    } else {
        None
    };
    model::is_stale(now, c.last_activity, last_commit, stale_after)
}

fn derive_for(
    adapter: &dyn crate::adapters::RuntimeAdapter,
    db: &SqliteBackend,
    host: &str,
    gh: bool,
    now: DateTime<Utc>,
    stale_after: chrono::Duration,
    t: &TaskRow,
) -> Derived {
    let blockers: Vec<Status> = t.blocked_by.iter().map(|b| b.status).collect();
    let commits = if t.status == Status::Review && t.kind == Kind::Code {
        t.claim
            .as_ref()
            .map(|c| live_evidence(db, host, gh, c, now))
            .and_then(|e| e.commits())
    } else {
        None
    };
    let pending_permissions = t
        .claim
        .as_ref()
        .map_or(0, |c| adapter.pending_permissions(&c.actor.external_id));

    model::derive(&Facts {
        status: t.status,
        kind: t.kind,
        needs_plan_approval: t.needs_plan_approval,
        has_plan: t.plan.is_some(),
        plan_approved: t.plan_approved_at.is_some(),
        blockers: &blockers,
        claim_stale: t.status == Status::InProgress
            && t.claim
                .as_ref()
                .is_some_and(|c| claim_is_stale(adapter, c, host, now, stale_after)),
        commits,
        pending_permissions,
        permission_required: t.claim.as_ref().is_some_and(|c| {
            adapter.agent_status(&c.actor.external_id)
                == Some(crate::adapters::AgentRuntimeStatus::Permission)
        }),
    })
}

fn latest_claim(db: &SqliteBackend, t: &TaskRow) -> Option<ClaimRow> {
    t.claim.clone().or_else(|| {
        db.task_detail(t.id)
            .ok()
            .and_then(|d| d.claims.into_iter().next())
    })
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

fn task_json(
    adapter: &dyn crate::adapters::RuntimeAdapter,
    t: &TaskRow,
    d: &Derived,
    ev: Option<&Ev>,
) -> Value {
    let mut val = json!({
        "id": fmt_id(t.id),
        "title": t.title,
        "kind": t.kind.as_str(),
        "status": t.status.as_str(),
        "priority": t.priority,
        "project": t.project.as_ref().map(|p| json!({
            "key": p.key,
            "alias": p.alias,
            "name": p.display(),
        })),
        "ticket": t.ticket,
        "goal": t.goal,
        "acceptance": t.acceptance,
        "plan": t.plan,
        "result": t.result,
        "needs_plan_approval": t.needs_plan_approval,
        "plan_approved_at": t.plan_approved_at.map(ts),
        "blocked_by": t.blocked_by.iter().map(|b| json!({
            "id": fmt_id(b.id),
            "status": b.status.as_str(),
            "title": b.title,
        })).collect::<Vec<_>>(),
        "claim": t.claim.as_ref().map(claim_json),
        "derived": d,
        "created_by": t.created_by,
        "created_at": ts(t.created_at),
        "updated_at": ts(t.updated_at),
    });
    val["evidence"] = ev.map_or(Value::Null, |e| e.to_json());

    if let Some(c) = &t.claim {
        if let Some(st) = adapter.agent_status(&c.actor.external_id) {
            let perms = adapter.pending_permissions(&c.actor.external_id);
            val["runtime"] = json!({
                "status": st.as_str(),
                "pending_permissions": perms,
            });
            val["runtime_status"] = json!(st.as_str());
            val["pending_permissions"] = json!(perms);
        } else {
            val["runtime"] = Value::Null;
            val["runtime_status"] = Value::Null;
            val["pending_permissions"] = Value::Null;
        }
    } else {
        val["runtime"] = Value::Null;
        val["runtime_status"] = Value::Null;
        val["pending_permissions"] = Value::Null;
    }

    val
}

/// GET /api/board
/// Returns tasks with derived states + claim + evidence line, projects, instances, actors.
pub async fn board_handler(State(state): State<Arc<AppState>>) -> Response {
    let db = match SqliteBackend::open(&state.db_path) {
        Ok(db) => db,
        Err(e) => return internal_error(e),
    };

    let cfg = crate::config::load(&state.config.home).unwrap_or_default();
    let now = Utc::now();

    let tasks = match db.list_tasks(&TaskFilter {
        include_terminal: true,
        ..Default::default()
    }) {
        Ok(t) => t,
        Err(e) => return internal_error(e),
    };

    let mut task_values = Vec::new();
    for t in &tasks {
        let d = derive_for(
            state.adapter.as_ref(),
            &db,
            &state.host,
            cfg.gh,
            now,
            cfg.stale_after,
            t,
        );
        let ev = latest_claim(&db, t).map(|c| live_evidence(&db, &state.host, cfg.gh, &c, now));
        task_values.push(task_json(state.adapter.as_ref(), t, &d, ev.as_ref()));
    }

    let projects = match db.all_projects() {
        Ok(p) => p
            .into_iter()
            .map(|pr| {
                json!({
                    "id": pr.id,
                    "key": pr.key,
                    "alias": pr.alias,
                    "name": pr.display(),
                })
            })
            .collect::<Vec<_>>(),
        Err(e) => return internal_error(e),
    };

    let instances = match db.all_instances() {
        Ok(inst) => inst
            .into_iter()
            .map(|i| {
                json!({
                    "host": i.host,
                    "cwd": i.cwd,
                    "branch": i.branch,
                    "workspace_id": i.workspace_id,
                    "actor": i.actor,
                    "project": i.project.map(|p| json!({
                        "id": p.id,
                        "key": p.key,
                        "alias": p.alias,
                        "name": p.display(),
                    })),
                })
            })
            .collect::<Vec<_>>(),
        Err(e) => return internal_error(e),
    };

    let actors = match db.all_actors() {
        Ok(a) => a,
        Err(e) => return internal_error(e),
    };

    (
        StatusCode::OK,
        axum::Json(json!({
            "v": 1,
            "ok": true,
            "data": {
                "tasks": task_values,
                "projects": projects,
                "instances": instances,
                "actors": actors,
            }
        })),
    )
        .into_response()
}

/// GET /api/tasks/:id
/// Returns full detail for one task incl. notes, evidence, events.
pub async fn task_detail_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
) -> Response {
    let id = match parse_id(&id_str) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                axum::Json(json!({
                    "v": 1,
                    "ok": false,
                    "error": {
                        "code": "NOT_FOUND",
                        "message": format!("task {id_str:?} not found")
                    }
                })),
            )
                .into_response();
        }
    };

    let db = match SqliteBackend::open(&state.db_path) {
        Ok(db) => db,
        Err(e) => return internal_error(e),
    };

    let detail = match db.task_detail(id) {
        Ok(d) => d,
        Err(FbError::NotFound(_)) => {
            return (
                StatusCode::NOT_FOUND,
                axum::Json(json!({
                    "v": 1,
                    "ok": false,
                    "error": {
                        "code": "NOT_FOUND",
                        "message": format!("task {} not found", fmt_id(id))
                    }
                })),
            )
                .into_response();
        }
        Err(e) => return internal_error(e),
    };

    let events = match db.task_events(id) {
        Ok(ev) => ev,
        Err(e) => return internal_error(e),
    };

    let cfg = crate::config::load(&state.config.home).unwrap_or_default();
    let now = Utc::now();
    let derived = derive_for(
        state.adapter.as_ref(),
        &db,
        &state.host,
        cfg.gh,
        now,
        cfg.stale_after,
        &detail.task,
    );
    let ev = detail
        .claims
        .first()
        .map(|c| live_evidence(&db, &state.host, cfg.gh, c, now));

    let mut j = task_json(state.adapter.as_ref(), &detail.task, &derived, ev.as_ref());
    j["claims"] = json!(detail.claims.iter().map(claim_json).collect::<Vec<_>>());
    j["notes"] = json!(
        detail
            .notes
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
    j["events"] = json!(
        events
            .iter()
            .map(|e| json!({
                "id": e.id,
                "task_id": e.task_id.map(fmt_id),
                "actor": e.actor,
                "type": e.event_type,
                "data": e.data,
                "pid": e.pid,
                "ppid": e.ppid,
                "proc_chain": e.proc_chain,
                "created_at": ts(e.created_at),
                "on_behalf_of": e.on_behalf_of,
                "source": e.source,
            }))
            .collect::<Vec<_>>()
    );

    (
        StatusCode::OK,
        axum::Json(json!({
            "v": 1,
            "ok": true,
            "data": {
                "task": j
            }
        })),
    )
        .into_response()
}

pub fn fb_error_response(e: FbError) -> Response {
    let status = match &e {
        FbError::Usage(_) => StatusCode::BAD_REQUEST,
        FbError::NotFound(_) => StatusCode::NOT_FOUND,
        FbError::ClaimHeld(_)
        | FbError::NotReady(_)
        | FbError::BadTransition(_)
        | FbError::DepCycle(_) => StatusCode::CONFLICT,
        FbError::HumanOnly(_) | FbError::NotClaimant(_) => StatusCode::FORBIDDEN,
        FbError::SchemaAhead { .. } | FbError::DbLocked | FbError::Store(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (
        status,
        axum::Json(json!({
            "v": 1,
            "ok": false,
            "error": {
                "code": e.code(),
                "message": e.to_string(),
            }
        })),
    )
        .into_response()
}

fn get_context(state: &AppState) -> Result<(SqliteBackend, Actor, Ctx), FbError> {
    let db = SqliteBackend::open(&state.db_path)?;
    let actor = db.find_actor("human")?;
    let ctx = Ctx {
        now: Utc::now(),
        pid: std::process::id(),
        ppid: 0,
        proc_chain: "web".to_string(),
        source: Some("web".to_string()),
    };
    Ok((db, actor, ctx))
}

fn parse_task_id(s: &str) -> Result<i64, FbError> {
    parse_id(s).map_err(|_| FbError::NotFound(format!("task {s:?} not found")))
}

fn task_success_response(state: &AppState, db: &SqliteBackend, task: &TaskRow) -> Response {
    let cfg = crate::config::load(&state.config.home).unwrap_or_default();
    let now = Utc::now();
    let derived = derive_for(
        state.adapter.as_ref(),
        db,
        &state.host,
        cfg.gh,
        now,
        cfg.stale_after,
        task,
    );
    let ev = latest_claim(db, task).map(|c| live_evidence(db, &state.host, cfg.gh, &c, now));
    let t_json = task_json(state.adapter.as_ref(), task, &derived, ev.as_ref());
    (
        StatusCode::OK,
        axum::Json(json!({
            "v": 1,
            "ok": true,
            "data": {
                "task": t_json
            }
        })),
    )
        .into_response()
}

/// POST /api/tasks/:id/promote
pub async fn promote_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    match db.apply(&actor, id, &Command::Promote, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

/// POST /api/tasks/:id/approve
pub async fn approve_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    match db.apply(&actor, id, &Command::Approve, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

#[derive(serde::Deserialize, Default)]
pub struct VerifyPayload {
    #[serde(default)]
    pub force: bool,
}

/// POST /api/tasks/:id/verify
pub async fn verify_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let payload: VerifyPayload = if bytes.is_empty() {
        VerifyPayload::default()
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(p) => p,
            Err(e) => return fb_error_response(FbError::Usage(e.to_string())),
        }
    };
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    let task = match db.get_task(id) {
        Ok(t) => t,
        Err(e) => return fb_error_response(e),
    };
    let cfg = crate::config::load(&state.config.home).unwrap_or_default();
    let commits = if task.kind == Kind::Code {
        latest_claim(&db, &task)
            .map(|c| live_evidence(&db, &state.host, cfg.gh, &c, ctx.now))
            .and_then(|e| e.commits())
    } else {
        None
    };
    let cmd = Command::Verify {
        force: payload.force,
        commits,
    };
    match db.apply(&actor, id, &cmd, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

#[derive(serde::Deserialize, Default)]
pub struct RejectPayload {
    pub comment: Option<String>,
    pub text: Option<String>,
    pub body: Option<String>,
}

/// POST /api/tasks/:id/reject
pub async fn reject_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let payload: RejectPayload = if bytes.is_empty() {
        RejectPayload::default()
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(p) => p,
            Err(e) => return fb_error_response(FbError::Usage(e.to_string())),
        }
    };
    let comment = payload
        .comment
        .or(payload.text)
        .or(payload.body)
        .unwrap_or_default();
    if comment.trim().is_empty() {
        return fb_error_response(FbError::Usage("reject comment cannot be empty".into()));
    }
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    let cmd = Command::Reject { comment };
    match db.apply(&actor, id, &cmd, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

/// POST /api/tasks/:id/release
pub async fn release_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    match db.apply(&actor, id, &Command::Release, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

#[derive(serde::Deserialize, Default)]
pub struct StealPayload {
    #[serde(rename = "for")]
    pub for_actor: Option<String>,
    pub actor: Option<String>,
}

/// POST /api/tasks/:id/steal
pub async fn steal_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let payload: StealPayload = if bytes.is_empty() {
        StealPayload::default()
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(p) => p,
            Err(e) => return fb_error_response(FbError::Usage(e.to_string())),
        }
    };
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    let for_spec = payload.for_actor.or(payload.actor);
    let target_actor = match for_spec {
        Some(spec) => match db.find_actor(&spec) {
            Ok(a) => Some(a),
            Err(e) => return fb_error_response(e),
        },
        None => None,
    };
    let req = ClaimRequest {
        id: Some(id),
        steal: true,
        for_actor: target_actor,
        loc: Location {
            host: state.host.clone(),
            cwd: String::new(),
            branch: None,
            head: None,
        },
        scope_project: None,
        workspace_id: None,
    };
    match db.claim(&actor, &req, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

#[derive(serde::Deserialize, Default)]
pub struct DropPayload {
    pub reason: Option<String>,
}

/// POST /api/tasks/:id/drop
pub async fn drop_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let payload: DropPayload = if bytes.is_empty() {
        DropPayload::default()
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(p) => p,
            Err(e) => return fb_error_response(FbError::Usage(e.to_string())),
        }
    };
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    let cmd = Command::Drop {
        reason: payload.reason,
    };
    match db.apply(&actor, id, &cmd, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

/// POST /api/tasks/:id/reopen
pub async fn reopen_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    match db.apply(&actor, id, &Command::Reopen, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}

#[derive(serde::Deserialize, Default)]
pub struct NotePayload {
    pub body: Option<String>,
    pub text: Option<String>,
    pub comment: Option<String>,
    pub kind: Option<String>,
}

/// POST /api/tasks/:id/notes and POST /api/tasks/:id/comment
pub async fn note_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id_str): axum::extract::Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let id = match parse_task_id(&id_str) {
        Ok(id) => id,
        Err(e) => return fb_error_response(e),
    };
    let payload: NotePayload = if bytes.is_empty() {
        NotePayload::default()
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(p) => p,
            Err(e) => return fb_error_response(FbError::Usage(e.to_string())),
        }
    };
    let body = payload
        .body
        .or(payload.text)
        .or(payload.comment)
        .unwrap_or_default();
    if body.trim().is_empty() {
        return fb_error_response(FbError::Usage("note body cannot be empty".into()));
    }
    let kind = payload.kind.unwrap_or_else(|| "comment".to_string());
    if !["handoff", "comment", "finding"].contains(&kind.as_str()) {
        return fb_error_response(FbError::Usage(format!(
            "bad note kind {kind:?} (handoff | comment | finding)"
        )));
    }
    let (db, actor, ctx) = match get_context(&state) {
        Ok(c) => c,
        Err(e) => return fb_error_response(e),
    };
    let cmd = Command::Note { kind, body };
    match db.apply(&actor, id, &cmd, &ctx) {
        Ok(task) => task_success_response(&state, &db, &task),
        Err(e) => fb_error_response(e),
    }
}
