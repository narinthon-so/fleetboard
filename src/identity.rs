//! Caller identity resolution (DESIGN.md D3).

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ActorKind {
    Human,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Caller {
    pub kind: ActorKind,
    pub provider: Option<String>,
    pub external_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    #[error("--as-human refused: agent markers present ({0})")]
    AgentMarkers(String),
}

/// Known agent binary names recognized in process ancestry.
const KNOWN_AGENT_BINARIES: &[&str] = &["claude", "codex", "agy", "antigravity", "gemini"];

/// Normalise an agent provider name to one of `claude | codex | antigravity | unknown`.
///
/// Mapping rules (case-insensitive, by prefix or contains):
/// - `claude*` or contains `claude` -> `claude`
/// - `codex*` or contains `codex` -> `codex`
/// - `agy*`, `antigravity*`, `gemini*` or contains `agy`, `antigravity`, `gemini` -> `antigravity`
/// - else -> `unknown`
pub fn normalize_provider(raw: &str) -> &'static str {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() {
        return "unknown";
    }
    if s.contains("claude") {
        "claude"
    } else if s.contains("codex") {
        "codex"
    } else if s.contains("antigravity") || s.contains("gemini") || s.contains("agy") {
        "antigravity"
    } else {
        "unknown"
    }
}

/// Match a process name against known agent binaries.
/// Handles paths (e.g. `/usr/bin/claude`), casing, and optional `.exe` extension.
fn match_known_binary(name: &str) -> Option<&'static str> {
    let lower = name.trim().to_ascii_lowercase();
    let base = Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&lower);
    let base = base.strip_suffix(".exe").unwrap_or(base);

    for &known in KNOWN_AGENT_BINARIES {
        if base == known {
            return Some(known);
        }
    }

    if base.contains("claude") {
        Some("claude")
    } else if base.contains("codex") {
        Some("codex")
    } else if base.contains("antigravity") {
        Some("antigravity")
    } else if base.contains("gemini") {
        Some("gemini")
    } else if base.contains("agy") {
        Some("agy")
    } else {
        None
    }
}

/// DESIGN.md "Identity" rules 1-4. Never returns Human.
pub fn resolve_agent(
    env: &dyn Fn(&str) -> Option<String>,
    chain: &[ProcInfo],
    host: &str,
    cwd: &Path,
) -> Caller {
    // 1. PASEO_AGENT_ID -> agent, external_id = the Paseo id.
    // Provider from AI_AGENT / known provider variables, else from process ancestry (rule 3), else unknown.
    if let Some(paseo_id) = env("PASEO_AGENT_ID").filter(|s| !s.trim().is_empty()) {
        let external_id = paseo_id.trim().to_string();
        let provider = if let Some(ai_agent) = env("AI_AGENT").filter(|s| !s.trim().is_empty()) {
            normalize_provider(&ai_agent).to_string()
        } else if env("CLAUDECODE").is_some() || env("CLAUDE_CODE_SESSION_ID").is_some() {
            "claude".to_string()
        } else if let Some((_, known)) = find_known_agent_in_chain(chain) {
            normalize_provider(known).to_string()
        } else {
            "unknown".to_string()
        };

        let short_id: String = external_id.chars().take(7).collect();
        let label = format!("{provider} {short_id}");

        return Caller {
            kind: ActorKind::Agent,
            provider: Some(provider),
            external_id,
            label,
        };
    }

    // 2. Provider variables -> agent:
    // Claude Code: CLAUDECODE=1, external_id = CLAUDE_CODE_SESSION_ID.
    let ai_agent = env("AI_AGENT").filter(|s| !s.trim().is_empty());
    let is_claudecode = env("CLAUDECODE").is_some() || env("CLAUDE_CODE_SESSION_ID").is_some();
    if is_claudecode || ai_agent.is_some() {
        let provider = if let Some(ref raw_agent) = ai_agent {
            normalize_provider(raw_agent).to_string()
        } else {
            "claude".to_string()
        };
        let (external_id, label) = if let Some(session_id) =
            env("CLAUDE_CODE_SESSION_ID").filter(|s| !s.trim().is_empty())
        {
            let sid = session_id.trim().to_string();
            let short_sid: String = sid.chars().take(7).collect();
            let label = format!("{provider} {short_sid}");
            (sid, label)
        } else if let Some((proc, _)) = find_known_agent_in_chain(chain) {
            (
                format!("proc:{host}:{provider}:{}", proc.pid),
                format!("{provider} {}", proc.pid),
            )
        } else {
            (
                format!("anon:{host}:{}", cwd.display()),
                format!("{provider} {host}"),
            )
        };

        return Caller {
            kind: ActorKind::Agent,
            provider: Some(provider),
            external_id,
            label,
        };
    }

    // 3. Process ancestry contains a known agent binary (claude, codex, agy, ...) -> agent,
    // provider from the binary, external_id = proc:<host>:<provider>:<pid of that agent process>.
    if let Some((proc, known)) = find_known_agent_in_chain(chain) {
        let provider = normalize_provider(known).to_string();
        let external_id = format!("proc:{host}:{provider}:{}", proc.pid);
        let label = format!("{provider} {}", proc.pid);

        return Caller {
            kind: ActorKind::Agent,
            provider: Some(provider),
            external_id,
            label,
        };
    }

    // 4. Nothing found -> agent actor provider='unknown', external_id = anon:<host>:<cwd>. Not human.
    let provider = "unknown".to_string();
    let external_id = format!("anon:{host}:{}", cwd.display());
    let label = format!("anon {host}");

    Caller {
        kind: ActorKind::Agent,
        provider: Some(provider),
        external_id,
        label,
    }
}

/// Helper to locate the first ancestor process matching a known agent binary.
fn find_known_agent_in_chain(chain: &[ProcInfo]) -> Option<(&ProcInfo, &'static str)> {
    for proc in chain {
        if let Some(known) = match_known_binary(&proc.name) {
            return Some((proc, known));
        }
    }
    None
}

/// True when any ancestor process is a recognised agent binary.
pub fn chain_has_agent_marker(chain: &[ProcInfo]) -> bool {
    find_known_agent_in_chain(chain).is_some()
}

/// Human caller (external_id "human") if no rule 1-3 marker present, else Err.
pub fn resolve_human(
    env: &dyn Fn(&str) -> Option<String>,
    chain: &[ProcInfo],
) -> Result<Caller, IdentityError> {
    let mut markers = Vec::new();

    if let Some(val) = env("PASEO_AGENT_ID").filter(|s| !s.trim().is_empty()) {
        let _ = val;
        markers.push("PASEO_AGENT_ID".to_string());
    }

    if let Some(val) = env("CLAUDECODE").filter(|s| !s.trim().is_empty()) {
        let _ = val;
        markers.push("CLAUDECODE".to_string());
    }

    if let Some(val) = env("CLAUDE_CODE_SESSION_ID").filter(|s| !s.trim().is_empty()) {
        let _ = val;
        markers.push("CLAUDE_CODE_SESSION_ID".to_string());
    }

    if let Some(val) = env("AI_AGENT").filter(|s| !s.trim().is_empty()) {
        let _ = val;
        markers.push("AI_AGENT".to_string());
    }

    for proc in chain {
        if let Some(bin) = match_known_binary(&proc.name) {
            markers.push(bin.to_string());
        }
    }

    if !markers.is_empty() {
        return Err(IdentityError::AgentMarkers(markers.join(", ")));
    }

    Ok(Caller {
        kind: ActorKind::Human,
        provider: None,
        external_id: "human".to_string(),
        label: "human".to_string(),
    })
}

/// Ancestor processes of the current process, nearest first (Linux /proc, macOS `ps`).
pub fn process_chain() -> Vec<ProcInfo> {
    if Path::new("/proc").exists() {
        linux_process_chain()
    } else {
        ps_process_chain()
    }
}

fn linux_process_chain() -> Vec<ProcInfo> {
    let mut chain = Vec::new();
    let mut cur = std::process::id();
    let mut visited = std::collections::HashSet::new();
    visited.insert(cur);

    while let Some(ppid) = read_ppid_from_proc(cur) {
        if ppid == 0 || !visited.insert(ppid) {
            break;
        }
        let name = read_name_from_proc(ppid).unwrap_or_default();
        chain.push(ProcInfo { pid: ppid, name });
        cur = ppid;
    }
    chain
}

fn read_ppid_from_proc(pid: u32) -> Option<u32> {
    let status_path = format!("/proc/{pid}/status");
    let content = std::fs::read_to_string(status_path).ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            return rest.trim().parse::<u32>().ok();
        }
    }
    None
}

fn read_name_from_proc(pid: u32) -> Option<String> {
    let comm_path = format!("/proc/{pid}/comm");
    if let Ok(comm) = std::fs::read_to_string(comm_path) {
        let trimmed = comm.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let status_path = format!("/proc/{pid}/status");
    let content = std::fs::read_to_string(status_path).ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Name:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn ps_process_chain() -> Vec<ProcInfo> {
    let output = match std::process::Command::new("ps")
        .args(["-eo", "pid=,ppid=,comm="])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output);
    let mut table: std::collections::HashMap<u32, (u32, String)> = std::collections::HashMap::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3
            && let (Ok(pid), Ok(ppid)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
        {
            let comm = parts[2..].join(" ");
            table.insert(pid, (ppid, comm));
        }
    }

    let mut chain = Vec::new();
    let mut cur = std::process::id();
    let mut visited = std::collections::HashSet::new();
    visited.insert(cur);

    while let Some(&(ppid, ref comm)) = table.get(&cur) {
        if ppid == 0 || !visited.insert(ppid) {
            break;
        }
        let name = table
            .get(&ppid)
            .map(|(_, c)| c.clone())
            .unwrap_or_else(|| comm.clone());
        chain.push(ProcInfo { pid: ppid, name });
        cur = ppid;
    }

    chain
}

/// Convenience: real env + process_chain + hostname + current dir.
pub fn resolve_current(as_human: bool) -> Result<Caller, IdentityError> {
    let env_fn = |key: &str| std::env::var(key).ok();
    let chain = process_chain();
    if as_human {
        resolve_human(&env_fn, &chain)
    } else {
        let host = gethostname::gethostname().to_string_lossy().to_string();
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        Ok(resolve_agent(&env_fn, &chain, &host, &cwd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    type EnvFn = Box<dyn Fn(&str) -> Option<String>>;

    fn make_env(vars: &[(&str, &str)]) -> EnvFn {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Box::new(move |key: &str| map.get(key).cloned())
    }

    #[test]
    fn test_rule1_paseo_with_ai_agent() {
        let env = make_env(&[
            ("PASEO_AGENT_ID", "60a11683-bbfa-4445-ae66-5b271d1bffb3"),
            ("AI_AGENT", "codex"),
        ]);
        let chain = vec![ProcInfo {
            pid: 123,
            name: "bash".into(),
        }];
        let caller = resolve_agent(&env, &chain, "myhost", Path::new("/workspace"));

        assert_eq!(caller.kind, ActorKind::Agent);
        assert_eq!(caller.provider.as_deref(), Some("codex"));
        assert_eq!(caller.external_id, "60a11683-bbfa-4445-ae66-5b271d1bffb3");
        assert_eq!(caller.label, "codex 60a1168");
    }

    #[test]
    fn test_normalize_provider_table() {
        let cases = [
            ("claude-code_2-1-310_agent", "claude"),
            ("claude", "claude"),
            ("claude-code", "claude"),
            ("claude_agent", "claude"),
            ("CLAUDE", "claude"),
            ("Claude-3", "claude"),
            ("codex", "codex"),
            ("codex-cli", "codex"),
            ("codex_agent", "codex"),
            ("CODEX", "codex"),
            ("agy", "antigravity"),
            ("AGY", "antigravity"),
            ("agy_agent", "antigravity"),
            ("antigravity", "antigravity"),
            ("antigravity-cli", "antigravity"),
            ("ANTIGRAVITY", "antigravity"),
            ("gemini", "antigravity"),
            ("gemini-1.5-pro", "antigravity"),
            ("gemini_agent", "antigravity"),
            ("GEMINI", "antigravity"),
            ("unknown", "unknown"),
            ("something_else", "unknown"),
            ("gpt-4", "unknown"),
            ("", "unknown"),
            ("   ", "unknown"),
        ];

        for (input, expected) in cases {
            assert_eq!(
                normalize_provider(input),
                expected,
                "failed for input: {input:?}"
            );
        }
    }

    #[test]
    fn test_rule1_paseo_fallback_to_chain() {
        let env = make_env(&[("PASEO_AGENT_ID", "403254b1-2345")]);
        let chain = vec![
            ProcInfo {
                pid: 50,
                name: "bash".into(),
            },
            ProcInfo {
                pid: 40,
                name: "agy".into(),
            },
        ];
        let caller = resolve_agent(&env, &chain, "myhost", Path::new("/workspace"));

        assert_eq!(caller.kind, ActorKind::Agent);
        assert_eq!(caller.provider.as_deref(), Some("antigravity"));
        assert_eq!(caller.external_id, "403254b1-2345");
        assert_eq!(caller.label, "antigravity 403254b");
    }

    #[test]
    fn test_rule1_paseo_unknown_provider() {
        let env = make_env(&[("PASEO_AGENT_ID", "abc1234567")]);
        let chain = vec![ProcInfo {
            pid: 50,
            name: "bash".into(),
        }];
        let caller = resolve_agent(&env, &chain, "myhost", Path::new("/workspace"));

        assert_eq!(caller.kind, ActorKind::Agent);
        assert_eq!(caller.provider.as_deref(), Some("unknown"));
        assert_eq!(caller.external_id, "abc1234567");
        assert_eq!(caller.label, "unknown abc1234");
    }

    #[test]
    fn test_rule2_claudecode() {
        let env = make_env(&[
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_SESSION_ID", "sess-1234567890"),
        ]);
        let chain = vec![];
        let caller = resolve_agent(&env, &chain, "myhost", Path::new("/workspace"));

        assert_eq!(caller.kind, ActorKind::Agent);
        assert_eq!(caller.provider.as_deref(), Some("claude"));
        assert_eq!(caller.external_id, "sess-1234567890");
        assert_eq!(caller.label, "claude sess-12");
    }

    #[test]
    fn test_rule3_process_ancestry_known_binaries() {
        for (bin_name, expected_prov) in [
            ("claude", "claude"),
            ("codex", "codex"),
            ("agy", "antigravity"),
            ("antigravity", "antigravity"),
            ("gemini", "antigravity"),
            ("/usr/local/bin/agy", "antigravity"),
        ] {
            let env = make_env(&[]);
            let chain = vec![
                ProcInfo {
                    pid: 10,
                    name: "sh".into(),
                },
                ProcInfo {
                    pid: 999,
                    name: bin_name.into(),
                },
            ];
            let caller = resolve_agent(&env, &chain, "box1", Path::new("/workspace"));

            assert_eq!(caller.kind, ActorKind::Agent);
            assert_eq!(caller.provider.as_deref(), Some(expected_prov));
            assert_eq!(caller.external_id, format!("proc:box1:{expected_prov}:999"));
            assert_eq!(caller.label, format!("{expected_prov} 999"));
        }
    }

    #[test]
    fn test_rule4_anon_nothing_found() {
        let env = make_env(&[]);
        let chain = vec![ProcInfo {
            pid: 1,
            name: "init".into(),
        }];
        let caller = resolve_agent(&env, &chain, "box2", Path::new("/test/repo"));

        assert_eq!(caller.kind, ActorKind::Agent);
        assert_eq!(caller.provider.as_deref(), Some("unknown"));
        assert_eq!(caller.external_id, "anon:box2:/test/repo");
        assert_eq!(caller.label, "anon box2");
    }

    #[test]
    fn test_resolve_human_success() {
        let env = make_env(&[]);
        let chain = vec![
            ProcInfo {
                pid: 100,
                name: "bash".into(),
            },
            ProcInfo {
                pid: 1,
                name: "systemd".into(),
            },
        ];
        let caller = resolve_human(&env, &chain).expect("should resolve as human");

        assert_eq!(caller.kind, ActorKind::Human);
        assert_eq!(caller.provider, None);
        assert_eq!(caller.external_id, "human");
        assert_eq!(caller.label, "human");
    }

    #[test]
    fn test_resolve_human_refused_paseo() {
        let env = make_env(&[("PASEO_AGENT_ID", "agent-123")]);
        let chain = vec![];
        let err = resolve_human(&env, &chain).unwrap_err();
        assert_eq!(
            err,
            IdentityError::AgentMarkers("PASEO_AGENT_ID".to_string())
        );
        assert_eq!(
            err.to_string(),
            "--as-human refused: agent markers present (PASEO_AGENT_ID)"
        );
    }

    #[test]
    fn test_resolve_human_refused_claudecode() {
        let env = make_env(&[("CLAUDECODE", "1")]);
        let chain = vec![];
        let err = resolve_human(&env, &chain).unwrap_err();
        assert_eq!(err, IdentityError::AgentMarkers("CLAUDECODE".to_string()));
    }

    #[test]
    fn test_resolve_human_refused_known_binaries() {
        for bin in &["claude", "codex", "agy", "antigravity", "gemini"] {
            let env = make_env(&[]);
            let chain = vec![ProcInfo {
                pid: 42,
                name: (*bin).to_string(),
            }];
            let err = resolve_human(&env, &chain).unwrap_err();
            assert_eq!(err, IdentityError::AgentMarkers((*bin).to_string()));
        }
    }

    #[test]
    fn test_rule1_paseo_ai_agent_normalization_table() {
        let cases = [
            ("claude-code_2-1-310_agent", "claude", "claude 855f6f3"),
            ("codex", "codex", "codex 855f6f3"),
            ("agy", "antigravity", "antigravity 855f6f3"),
            ("antigravity", "antigravity", "antigravity 855f6f3"),
            ("gemini", "antigravity", "antigravity 855f6f3"),
            ("gemini-1.5-pro", "antigravity", "antigravity 855f6f3"),
            ("unknown_provider", "unknown", "unknown 855f6f3"),
        ];

        for (ai_agent, expected_prov, expected_label) in cases {
            let env = make_env(&[
                ("PASEO_AGENT_ID", "855f6f3a-bbfa-4445-ae66-5b271d1bffb3"),
                ("AI_AGENT", ai_agent),
            ]);
            let chain = vec![];
            let caller = resolve_agent(&env, &chain, "myhost", Path::new("/workspace"));
            assert_eq!(
                caller.provider.as_deref(),
                Some(expected_prov),
                "provider for AI_AGENT={ai_agent}"
            );
            assert_eq!(
                caller.label, expected_label,
                "label for AI_AGENT={ai_agent}"
            );
        }
    }

    #[test]
    fn test_non_ascii_id_no_panic() {
        // Multi-byte emoji where byte index 7 would split a char:
        // "hello!🦀123": "hello!" is 6 bytes; '🦀' is 4 bytes (bytes 6..10).
        // Slicing at byte 7 panics with byte slicing.
        let env = make_env(&[("PASEO_AGENT_ID", "hello!🦀123"), ("AI_AGENT", "claude")]);
        let caller = resolve_agent(&env, &[], "myhost", Path::new("/workspace"));
        assert_eq!(caller.label, "claude hello!🦀");

        // Japanese characters: 3 bytes each.
        // "日本語ID123": '日'=3 bytes, '本'=3 bytes, '語'=3 bytes. Byte 7 is inside '語'.
        let env = make_env(&[("PASEO_AGENT_ID", "日本語ID123"), ("AI_AGENT", "codex")]);
        let caller = resolve_agent(&env, &[], "myhost", Path::new("/workspace"));
        assert_eq!(caller.label, "codex 日本語ID12");

        // Non-ASCII in CLAUDE_CODE_SESSION_ID
        let env = make_env(&[
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_SESSION_ID", "hello!🦀123"),
        ]);
        let caller = resolve_agent(&env, &[], "myhost", Path::new("/workspace"));
        assert_eq!(caller.label, "claude hello!🦀");
    }

    #[test]
    fn test_process_chain_non_empty() {
        let chain = process_chain();
        assert!(!chain.is_empty(), "process_chain should find ancestors");
    }
}
