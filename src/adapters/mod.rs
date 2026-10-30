//! Runtime adapters (M2): link fleetboard to orchestrators (Paseo first).
//! See DESIGN.md (Architecture/adapters) and IDEA.md (Orchestrator is optional).

pub mod hooks;
pub mod none;
pub mod paseo;

use chrono::Duration;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use self::hooks::{HookEvent, HookStdinPayload, HooksAdapter};
pub use self::none::NoneAdapter;
pub use self::paseo::{PaseoAdapter, find_matching_workspace, normalize_path};

/// Live status of an agent reported by a runtime orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntimeStatus {
    Running,
    Idle,
    Permission,
    Gone,
}

impl AgentRuntimeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentRuntimeStatus::Running => "running",
            AgentRuntimeStatus::Idle => "idle",
            AgentRuntimeStatus::Permission => "permission",
            AgentRuntimeStatus::Gone => "gone",
        }
    }
}

/// Runtime instance discovered from the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInstance {
    pub host: String,
    pub cwd: String,
    pub branch: Option<String>,
    pub workspace_id: Option<String>,
    pub agent_id: Option<String>,
}

/// Consolidated runtime summary for an agent or claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub status: AgentRuntimeStatus,
    pub pending_permissions: u32,
}

/// Trait implemented by orchestrator adapters.
pub trait RuntimeAdapter: Send + Sync {
    /// Active workspace / agent instances known to the orchestrator.
    fn instances(&self) -> Vec<RuntimeInstance>;

    /// Current status of an agent by its external ID (e.g. Paseo agent ID).
    /// Returns `None` if the adapter has no live report for the agent or is disabled.
    fn agent_status(&self, external_id: &str) -> Option<AgentRuntimeStatus>;

    /// Number of permissions waiting on human operator approval.
    fn pending_permissions(&self, external_id: &str) -> u32;

    /// Look up a workspace ID for an agent and/or working directory.
    fn workspace_id(&self, _external_id: &str, _cwd: &str) -> Option<String> {
        None
    }
}

/// Construct a runtime adapter given the configured name and optional home/stale_after.
pub fn make_adapter(
    name: &str,
    home: Option<&Path>,
    stale_after: Option<Duration>,
) -> Arc<dyn RuntimeAdapter> {
    match name.trim().to_lowercase().as_str() {
        "paseo" => Arc::new(PaseoAdapter::new()),
        "hooks" => {
            let h = home.map(|p| p.to_path_buf()).unwrap_or_else(|| {
                std::env::var("FB_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        std::env::var("HOME")
                            .map(|h| PathBuf::from(h).join(".fleetboard"))
                            .unwrap_or_else(|_| PathBuf::from(".fleetboard"))
                    })
            });
            let s = stale_after.unwrap_or_else(|| Duration::hours(2));
            Arc::new(HooksAdapter::new(h, s))
        }
        _ => Arc::new(NoneAdapter::new()),
    }
}
