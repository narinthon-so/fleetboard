//! Storage: schema, migrations and the `Backend` trait (D7).

pub mod error;
pub mod schema;
pub mod sqlite;
pub mod types;

pub use error::{FbError, Result};
pub use sqlite::SqliteBackend;
pub use types::*;

use chrono::{DateTime, Utc};

use crate::identity::Caller;

/// Everything the CLI needs from storage. `SqliteBackend` is the only
/// implementation in M0; an HTTP client arrives with remote mode.
///
/// Every mutating method runs in one write transaction, checks the role and
/// the status machine inside it, and appends to the audit log.
pub trait Backend {
    /// Find or create the actor for this caller and bump `last_seen`.
    fn resolve_actor(&self, caller: &Caller, ctx: &Ctx) -> Result<Actor>;
    /// Refresh this actor's active claims on `loc.host`: `last_activity`, and
    /// cwd / branch when `loc` is given (inside a git repo).
    fn touch(&self, actor: &Actor, loc: Option<&Location>, ctx: &Ctx) -> Result<()>;
    /// By numeric id, label or external id.
    fn find_actor(&self, spec: &str) -> Result<Actor>;
    fn set_trusted(&self, actor: &Actor, target: &Actor, trusted: bool, ctx: &Ctx)
    -> Result<Actor>;

    fn find_project(&self, key_or_alias: &str) -> Result<Option<Project>>;
    fn set_alias(
        &self,
        actor: &Actor,
        key_or_alias: &str,
        alias: &str,
        ctx: &Ctx,
    ) -> Result<Project>;

    fn add_task(&self, actor: &Actor, new: &NewTask, ctx: &Ctx) -> Result<TaskRow>;
    fn get_task(&self, id: i64) -> Result<TaskRow>;
    fn task_detail(&self, id: i64) -> Result<TaskDetail>;
    fn list_tasks(&self, filter: &TaskFilter) -> Result<Vec<TaskRow>>;
    /// Ready tasks (todo, every blocker done), best first.
    fn ready(&self, project_id: Option<i64>) -> Result<Vec<TaskRow>>;

    fn claim(&self, actor: &Actor, req: &ClaimRequest, ctx: &Ctx) -> Result<TaskRow>;
    fn apply(&self, actor: &Actor, id: i64, cmd: &Command, ctx: &Ctx) -> Result<TaskRow>;
    fn dep_add(&self, actor: &Actor, task: i64, blocker: i64, ctx: &Ctx) -> Result<TaskRow>;
    fn dep_rm(&self, actor: &Actor, task: i64, blocker: i64, ctx: &Ctx) -> Result<TaskRow>;

    fn save_snapshot(&self, claim_id: i64, json: &str, now: DateTime<Utc>) -> Result<()>;
    fn snapshot(&self, claim_id: i64) -> Result<Option<(String, DateTime<Utc>)>>;

    fn all_projects(&self) -> Result<Vec<Project>>;
    fn all_actors(&self) -> Result<Vec<Actor>>;
    fn all_instances(&self) -> Result<Vec<InstanceRow>>;
    fn task_events(&self, task_id: i64) -> Result<Vec<EventRow>>;
    fn data_version(&self) -> Result<i64>;
}
