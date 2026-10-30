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
    Backend, ClaimRow, FbError, SqliteBackend, TaskFilter, TaskRow, fmt_id, parse_id,
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
    c: &ClaimRow,
    host: &str,
    now: DateTime<Utc>,
    stale_after: chrono::Duration,
) -> bool {
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
                .is_some_and(|c| claim_is_stale(c, host, now, stale_after)),
        commits,
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

fn task_json(t: &TaskRow, d: &Derived, ev: Option<&Ev>) -> Value {
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
        let d = derive_for(&db, &state.host, cfg.gh, now, cfg.stale_after, t);
        let ev = latest_claim(&db, t).map(|c| live_evidence(&db, &state.host, cfg.gh, &c, now));
        task_values.push(task_json(t, &d, ev.as_ref()));
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
    let derived = derive_for(&db, &state.host, cfg.gh, now, cfg.stale_after, &detail.task);
    let ev = detail
        .claims
        .first()
        .map(|c| live_evidence(&db, &state.host, cfg.gh, c, now));

    let mut j = task_json(&detail.task, &derived, ev.as_ref());
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
