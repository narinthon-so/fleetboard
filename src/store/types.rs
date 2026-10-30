//! Plain data carried between the CLI and the store.

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::error::{FbError, Result};
use crate::model::{Kind, Role, Status};

/// Per-invocation context written into the audit log.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub now: DateTime<Utc>,
    pub pid: u32,
    pub ppid: u32,
    pub proc_chain: String,
}

/// Where the caller is: host plus the git toplevel (or cwd outside git).
#[derive(Debug, Clone)]
pub struct Location {
    pub host: String,
    pub cwd: String,
    pub branch: Option<String>,
    pub head: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Actor {
    pub id: i64,
    pub kind: String,
    pub provider: Option<String>,
    pub external_id: String,
    pub label: String,
    pub trusted: bool,
}

impl Actor {
    pub fn is_human(&self) -> bool {
        self.kind == "human"
    }

    pub fn role(&self) -> Role {
        if self.is_human() {
            Role::Human
        } else if self.trusted {
            Role::Trusted
        } else {
            Role::Agent
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub key: String,
    pub alias: Option<String>,
}

impl Project {
    /// Alias if set, else the last path segment of the key.
    pub fn display(&self) -> String {
        self.alias.clone().unwrap_or_else(|| {
            let tail = self.key.rsplit(['/', ':']).next().unwrap_or(&self.key);
            tail.to_string()
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Ticket {
    pub system: String,
    pub key: String,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlockerRef {
    pub id: i64,
    pub status: Status,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct ClaimRow {
    pub id: i64,
    pub task_id: i64,
    pub actor: Actor,
    pub host: String,
    pub cwd: String,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub workspace_id: Option<String>,
    pub claimed_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub release_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: i64,
    pub project: Option<Project>,
    pub ticket: Option<Ticket>,
    pub title: String,
    pub goal: Option<String>,
    pub acceptance: Option<String>,
    pub plan: Option<String>,
    pub result: Option<String>,
    pub kind: Kind,
    pub status: Status,
    pub priority: i64,
    pub needs_plan_approval: bool,
    pub plan_approved_at: Option<DateTime<Utc>>,
    pub created_by: Actor,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub blocked_by: Vec<BlockerRef>,
    /// The active claim, if any.
    pub claim: Option<ClaimRow>,
}

#[derive(Debug, Clone)]
pub struct NoteRow {
    pub id: i64,
    pub task_id: i64,
    pub actor: Actor,
    pub kind: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TaskDetail {
    pub task: TaskRow,
    /// Every claim, newest first.
    pub claims: Vec<ClaimRow>,
    /// Oldest first.
    pub notes: Vec<NoteRow>,
}

#[derive(Debug, Clone)]
pub struct TicketSpec {
    pub system: String,
    pub key: String,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ProjectSel {
    /// Key detected from git; created on first use.
    Detected(String),
    /// `--project`: an existing key or alias (a full key is created if new).
    Explicit(String),
}

#[derive(Debug, Clone)]
pub struct NewTask {
    pub title: String,
    pub goal: Option<String>,
    pub acceptance: Option<String>,
    pub kind: Kind,
    pub priority: i64,
    pub blocked_by: Vec<i64>,
    pub ticket: Option<TicketSpec>,
    pub project: Option<ProjectSel>,
    pub needs_plan_approval: bool,
}

#[derive(Debug, Clone)]
pub struct ClaimRequest {
    pub id: Option<i64>,
    pub steal: bool,
    /// `--for`: actor that receives a stolen claim.
    pub for_actor: Option<Actor>,
    pub loc: Location,
    /// Scope for picking a ready task when `id` is None.
    pub scope_project: Option<i64>,
}

/// One state-changing operation on an existing task.
#[derive(Debug, Clone)]
pub enum Command {
    Promote,
    Approve,
    Release,
    Drop {
        reason: Option<String>,
    },
    Reopen,
    /// `commits` is the evidence the CLI computed (None = unknown).
    Verify {
        force: bool,
        commits: Option<u32>,
    },
    Reject {
        comment: String,
    },
    Plan {
        text: String,
    },
    Done {
        result: Option<String>,
    },
    Note {
        kind: String,
        body: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub statuses: Vec<Status>,
    pub project_id: Option<i64>,
    /// Include done and dropped tasks.
    pub include_terminal: bool,
    /// Only tasks whose active claim belongs to this actor.
    pub claimant: Option<i64>,
}

pub fn fmt_id(id: i64) -> String {
    format!("fb-{id}")
}

/// Accepts `fb-12` or `12`.
pub fn parse_id(s: &str) -> Result<i64> {
    let n = s.strip_prefix("fb-").unwrap_or(s);
    n.parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| FbError::Usage(format!("bad task id {s:?} (expected fb-<n>)")))
}
