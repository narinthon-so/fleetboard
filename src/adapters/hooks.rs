//! Hooks runtime adapter (M3): tracks agent runtime status from hook events (`fb hook`).
//! See DESIGN.md (Architecture/adapters) and IDEA.md (Orchestrator is optional).

use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{AgentRuntimeStatus, RuntimeAdapter, RuntimeInstance};

/// Hook event types supported by `fb hook <event>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookEvent {
    SessionStart,
    Stop,
    ToolUse,
    Prompt,
}

impl HookEvent {
    pub fn parse(s: &str) -> Option<Self> {
        let clean = s.trim().to_ascii_lowercase().replace('_', "-");
        match clean.as_str() {
            "session-start" | "sessionstart" => Some(HookEvent::SessionStart),
            "stop" => Some(HookEvent::Stop),
            "tool-use" | "tooluse" | "pre-tool-use" | "pretooluse" | "post-tool-use"
            | "posttooluse" => Some(HookEvent::ToolUse),
            "prompt" | "user-prompt-submit" | "userpromptsubmit" => Some(HookEvent::Prompt),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::SessionStart => "session-start",
            HookEvent::Stop => "stop",
            HookEvent::ToolUse => "tool-use",
            HookEvent::Prompt => "prompt",
        }
    }
}

/// Standard hook stdin JSON payload from Claude Code or Codex.
#[derive(Debug, Default, Deserialize)]
pub struct HookStdinPayload {
    #[serde(alias = "session_id", alias = "sessionId")]
    pub session_id: Option<String>,

    #[serde(alias = "cwd")]
    pub cwd: Option<String>,

    #[serde(alias = "hook_event_name", alias = "hookEventName", alias = "event")]
    pub hook_event_name: Option<String>,

    #[serde(alias = "source")]
    pub source: Option<String>,

    #[serde(alias = "transcript_path", alias = "transcriptPath")]
    pub transcript_path: Option<String>,

    #[serde(alias = "permission_mode", alias = "permissionMode")]
    pub permission_mode: Option<String>,

    #[serde(alias = "agent", alias = "provider")]
    pub provider: Option<String>,

    #[serde(alias = "tool_name", alias = "toolName")]
    pub tool_name: Option<String>,

    #[serde(alias = "tool_input", alias = "toolInput")]
    pub tool_input: Option<serde_json::Value>,

    #[serde(alias = "prompt")]
    pub prompt: Option<serde_json::Value>,
}

/// Debug builds accept `FB_TEST_NOW` (RFC 3339) so tests can move the clock.
fn now() -> DateTime<Utc> {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("FB_TEST_NOW")
        && let Ok(t) = DateTime::parse_from_rfc3339(&v)
    {
        return t.with_timezone(&Utc);
    }
    Utc::now()
}

pub struct HooksAdapter {
    home: PathBuf,
    stale_after: Duration,
}

impl HooksAdapter {
    pub fn new(home: PathBuf, stale_after: Duration) -> Self {
        Self { home, stale_after }
    }

    pub fn agent_status_at(
        &self,
        external_id: &str,
        current_time: DateTime<Utc>,
    ) -> Option<AgentRuntimeStatus> {
        let db_path = self.home.join("board.db");
        if !db_path.exists() {
            return None;
        }

        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()?;

        let mut stmt = conn
            .prepare(
                "SELECT h.event, h.created_at \
                 FROM hook_events h \
                 JOIN actors a ON a.id = h.actor_id \
                 WHERE a.external_id = ?1 OR CAST(a.id AS TEXT) = ?1 OR a.label = ?1 \
                 ORDER BY h.id DESC LIMIT 1",
            )
            .ok()?;

        let res = stmt
            .query_row([external_id], |r| {
                let ev: String = r.get(0)?;
                let ts: String = r.get(1)?;
                Ok((ev, ts))
            })
            .optional()
            .ok()?;

        let (event_str, ts_str) = res?;
        let dt = DateTime::parse_from_rfc3339(&ts_str)
            .ok()?
            .with_timezone(&Utc);

        if current_time - dt > self.stale_after {
            return Some(AgentRuntimeStatus::Gone);
        }

        match event_str.as_str() {
            "stop" => Some(AgentRuntimeStatus::Idle),
            "tool-use" | "tool_use" | "prompt" | "session-start" | "session_start" => {
                Some(AgentRuntimeStatus::Running)
            }
            _ => Some(AgentRuntimeStatus::Running),
        }
    }
}

impl RuntimeAdapter for HooksAdapter {
    fn instances(&self) -> Vec<RuntimeInstance> {
        Vec::new()
    }

    fn agent_status(&self, external_id: &str) -> Option<AgentRuntimeStatus> {
        self.agent_status_at(external_id, now())
    }

    fn pending_permissions(&self, _external_id: &str) -> u32 {
        0
    }

    fn workspace_id(&self, _external_id: &str, _cwd: &str) -> Option<String> {
        None
    }
}
