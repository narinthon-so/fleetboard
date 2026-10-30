//! Paseo runtime adapter (M2).
//! Shells out to `paseo workspace ls --json`, `paseo ls --json`, `paseo inspect <id> --json`.
//! Joins agent -> workspace on cwd (longest prefix match).
//! Caches results for ~5 seconds.
//! Degrades to `none` with a warning if the binary is missing or errors.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{AgentRuntimeStatus, RuntimeAdapter, RuntimeInstance};

const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize, Clone)]
pub struct PaseoWorkspace {
    #[serde(alias = "workspaceId", alias = "workspace_id")]
    pub workspace_id: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub cwd: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaseoAgentListEntry {
    pub id: String,
    #[serde(alias = "shortId", alias = "short_id", default)]
    pub short_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaseoPermission {
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaseoInspectEntry {
    #[serde(alias = "Id", alias = "id")]
    pub id: String,
    #[serde(alias = "Status", alias = "status", default)]
    pub status: Option<String>,
    #[serde(alias = "Archived", alias = "archived", default)]
    pub archived: Option<bool>,
    #[serde(
        alias = "ArchivedAt",
        alias = "archived_at",
        alias = "archivedAt",
        default
    )]
    pub archived_at: Option<String>,
    #[serde(alias = "Cwd", alias = "cwd", default)]
    pub cwd: Option<String>,
    #[serde(
        alias = "PendingPermissions",
        alias = "pending_permissions",
        alias = "pendingPermissions",
        default
    )]
    pub pending_permissions: Vec<PaseoPermission>,
}

#[derive(Debug, Clone)]
enum InspectResult {
    Found(PaseoInspectEntry),
    NotFound,
    #[allow(dead_code)]
    Error(String),
}

struct Cache {
    ttl: Duration,
    workspaces: Option<(Instant, Vec<PaseoWorkspace>)>,
    agents: Option<(Instant, Vec<PaseoAgentListEntry>)>,
    inspect: HashMap<String, (Instant, InspectResult)>,
}

impl Cache {
    fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            workspaces: None,
            agents: None,
            inspect: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.workspaces = None;
        self.agents = None;
        self.inspect.clear();
    }
}

pub struct PaseoAdapter {
    cache: Mutex<Cache>,
    warned_missing: AtomicBool,
    warned_error: AtomicBool,
}

impl Default for PaseoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PaseoAdapter {
    pub fn new() -> Self {
        Self::with_ttl(DEFAULT_CACHE_TTL)
    }

    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(Cache::new(ttl)),
            warned_missing: AtomicBool::new(false),
            warned_error: AtomicBool::new(false),
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }

    fn warn_missing(&self) {
        if !self.warned_missing.swap(true, Ordering::Relaxed) {
            eprintln!("warning: paseo binary not found; degrading to none adapter");
        }
    }

    fn warn_error(&self, msg: &str) {
        if !self.warned_error.swap(true, Ordering::Relaxed) {
            eprintln!("warning: paseo command failed ({msg}); degrading to none adapter");
        }
    }

    fn exec_json<T: for<'de> Deserialize<'de>>(&self, args: &[&str]) -> Option<T> {
        let output = match Command::new("paseo").args(args).output() {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.warn_missing();
                return None;
            }
            Err(e) => {
                self.warn_error(&e.to_string());
                return None;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            self.warn_error(&format!("exit status {}: {}", output.status, stderr.trim()));
            return None;
        }

        match serde_json::from_slice::<T>(&output.stdout) {
            Ok(val) => Some(val),
            Err(e) => {
                self.warn_error(&format!("invalid json output: {e}"));
                None
            }
        }
    }

    pub fn list_workspaces(&self) -> Vec<PaseoWorkspace> {
        let now = Instant::now();
        if let Ok(c) = self.cache.lock()
            && let Some((fetched_at, list)) = &c.workspaces
            && now.duration_since(*fetched_at) < c.ttl
        {
            return list.clone();
        }

        let res: Option<Vec<PaseoWorkspace>> = self.exec_json(&["workspace", "ls", "--json"]);
        let list = res.unwrap_or_default();

        if let Ok(mut c) = self.cache.lock() {
            c.workspaces = Some((now, list.clone()));
        }
        list
    }

    pub fn list_agents(&self) -> Vec<PaseoAgentListEntry> {
        let now = Instant::now();
        if let Ok(c) = self.cache.lock()
            && let Some((fetched_at, list)) = &c.agents
            && now.duration_since(*fetched_at) < c.ttl
        {
            return list.clone();
        }

        let res: Option<Vec<PaseoAgentListEntry>> = self.exec_json(&["ls", "--json"]);
        let list = res.unwrap_or_default();

        if let Ok(mut c) = self.cache.lock() {
            c.agents = Some((now, list.clone()));
        }
        list
    }

    fn inspect_agent(&self, external_id: &str) -> InspectResult {
        let now = Instant::now();
        if let Ok(c) = self.cache.lock()
            && let Some((fetched_at, res)) = c.inspect.get(external_id)
            && now.duration_since(*fetched_at) < c.ttl
        {
            return res.clone();
        }

        let output = match Command::new("paseo")
            .args(["inspect", external_id, "--json"])
            .output()
        {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.warn_missing();
                return InspectResult::Error("binary not found".into());
            }
            Err(e) => {
                self.warn_error(&e.to_string());
                return InspectResult::Error(e.to_string());
            }
        };

        let res = if output.status.success() {
            match serde_json::from_slice::<PaseoInspectEntry>(&output.stdout) {
                Ok(entry) => InspectResult::Found(entry),
                Err(e) => {
                    self.warn_error(&format!("invalid inspect json: {e}"));
                    InspectResult::Error(e.to_string())
                }
            }
        } else {
            let out_str = String::from_utf8_lossy(&output.stdout);
            let err_str = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{out_str} {err_str}");

            if combined.contains("AGENT_NOT_FOUND")
                || combined.contains("Agent not found")
                || combined.contains("not found")
            {
                InspectResult::NotFound
            } else {
                self.warn_error(&format!(
                    "inspect exit {}: {}",
                    output.status,
                    combined.trim()
                ));
                InspectResult::Error(combined)
            }
        };

        if let Ok(mut c) = self.cache.lock() {
            c.inspect
                .insert(external_id.to_string(), (now, res.clone()));
        }
        res
    }
}

/// Normalize path for prefix comparison:
/// Expands leading `~` or `~/` to $HOME, strips trailing slashes.
pub fn normalize_path(p: &str) -> PathBuf {
    let s = p.trim();
    if s == "~"
        && let Some(h) = std::env::var_os("HOME")
    {
        return PathBuf::from(h);
    } else if let Some(rest) = s.strip_prefix("~/")
        && let Some(h) = std::env::var_os("HOME")
    {
        return PathBuf::from(h).join(rest);
    }
    PathBuf::from(s)
}

/// Longest prefix match: given an agent's directory, find the workspace whose cwd is
/// the longest prefix of the agent cwd.
pub fn find_matching_workspace<'a>(
    workspaces: &'a [PaseoWorkspace],
    agent_cwd: &str,
) -> Option<&'a PaseoWorkspace> {
    let norm_agent = normalize_path(agent_cwd);

    let mut best: Option<(&'a PaseoWorkspace, usize)> = None;

    for w in workspaces {
        let norm_wks = normalize_path(&w.cwd);
        if norm_agent.starts_with(&norm_wks) {
            let len = norm_wks.as_os_str().len();
            match best {
                Some((_, best_len)) if len > best_len => {
                    best = Some((w, len));
                }
                None => {
                    best = Some((w, len));
                }
                _ => {}
            }
        }
    }

    best.map(|(w, _)| w)
}

impl RuntimeAdapter for PaseoAdapter {
    fn instances(&self) -> Vec<RuntimeInstance> {
        let workspaces = self.list_workspaces();
        let agents = self.list_agents();
        let host = gethostname::gethostname().to_string_lossy().into_owned();

        let mut instances = Vec::new();

        for ag in agents {
            let ag_cwd = ag.cwd.unwrap_or_default();
            let matched_wks = find_matching_workspace(&workspaces, &ag_cwd);
            let workspace_id = matched_wks.map(|w| w.workspace_id.clone());

            let branch = if !ag_cwd.is_empty() {
                crate::gitinfo::instance(Path::new(&ag_cwd)).and_then(|i| i.branch)
            } else {
                None
            };

            instances.push(RuntimeInstance {
                host: host.clone(),
                cwd: ag_cwd,
                branch,
                workspace_id,
                agent_id: Some(ag.id),
            });
        }

        instances
    }

    fn agent_status(&self, external_id: &str) -> Option<AgentRuntimeStatus> {
        match self.inspect_agent(external_id) {
            InspectResult::Found(entry) => {
                if entry.archived.unwrap_or(false)
                    || entry.archived_at.is_some()
                    || entry.status.as_deref() == Some("archived")
                    || entry.status.as_deref() == Some("closed")
                    || entry.status.as_deref() == Some("gone")
                {
                    return Some(AgentRuntimeStatus::Gone);
                }

                if !entry.pending_permissions.is_empty()
                    || entry.status.as_deref() == Some("permission")
                    || entry.status.as_deref() == Some("waiting_permission")
                {
                    return Some(AgentRuntimeStatus::Permission);
                }

                match entry.status.as_deref() {
                    Some("running") => Some(AgentRuntimeStatus::Running),
                    Some("idle") => Some(AgentRuntimeStatus::Idle),
                    Some("archived") | Some("closed") | Some("gone") => {
                        Some(AgentRuntimeStatus::Gone)
                    }
                    _ => Some(AgentRuntimeStatus::Idle),
                }
            }
            InspectResult::NotFound => Some(AgentRuntimeStatus::Gone),
            InspectResult::Error(_) => None,
        }
    }

    fn pending_permissions(&self, external_id: &str) -> u32 {
        match self.inspect_agent(external_id) {
            InspectResult::Found(entry) => entry.pending_permissions.len() as u32,
            _ => 0,
        }
    }

    fn workspace_id(&self, external_id: &str, cwd: &str) -> Option<String> {
        let workspaces = self.list_workspaces();
        if let Some(w) = find_matching_workspace(&workspaces, cwd) {
            return Some(w.workspace_id.clone());
        }

        // Try lookup by agent inspect cwd
        if let InspectResult::Found(entry) = self.inspect_agent(external_id)
            && let Some(c) = entry.cwd
            && let Some(w) = find_matching_workspace(&workspaces, &c)
        {
            return Some(w.workspace_id.clone());
        }

        None
    }
}
