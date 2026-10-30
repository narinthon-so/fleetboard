//! Default `none` runtime adapter.
//! Self-report only: no orchestrator communication.

use super::{AgentRuntimeStatus, RuntimeAdapter, RuntimeInstance};

#[derive(Debug, Default, Clone)]
pub struct NoneAdapter;

impl NoneAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl RuntimeAdapter for NoneAdapter {
    fn instances(&self) -> Vec<RuntimeInstance> {
        Vec::new()
    }

    fn agent_status(&self, _external_id: &str) -> Option<AgentRuntimeStatus> {
        None
    }

    fn pending_permissions(&self, _external_id: &str) -> u32 {
        0
    }

    fn workspace_id(&self, _external_id: &str, _cwd: &str) -> Option<String> {
        None
    }
}
