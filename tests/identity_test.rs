use fleetboard::identity::{
    ActorKind, Caller, IdentityError, ProcInfo, process_chain, resolve_agent, resolve_current,
    resolve_human,
};
use std::collections::HashMap;
use std::path::Path;

fn mock_env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |key: &str| map.get(key).cloned()
}

#[test]
fn test_identity_rule1_paseo_variants() {
    // With AI_AGENT
    let env = mock_env(&[
        ("PASEO_AGENT_ID", "1111111-2222-3333"),
        ("AI_AGENT", "gemini"),
    ]);
    let caller = resolve_agent(&env, &[], "testhost", Path::new("/cwd"));
    assert_eq!(caller.kind, ActorKind::Agent);
    assert_eq!(caller.provider.as_deref(), Some("gemini"));
    assert_eq!(caller.external_id, "1111111-2222-3333");
    assert_eq!(caller.label, "gemini 1111111");

    // With ClaudeCode env
    let env = mock_env(&[("PASEO_AGENT_ID", "abcdef0-1234"), ("CLAUDECODE", "1")]);
    let caller = resolve_agent(&env, &[], "testhost", Path::new("/cwd"));
    assert_eq!(caller.provider.as_deref(), Some("claude"));
    assert_eq!(caller.label, "claude abcdef0");

    // With process chain fallback
    let env = mock_env(&[("PASEO_AGENT_ID", "9999999-8888")]);
    let chain = vec![ProcInfo {
        pid: 101,
        name: "antigravity".into(),
    }];
    let caller = resolve_agent(&env, &chain, "testhost", Path::new("/cwd"));
    assert_eq!(caller.provider.as_deref(), Some("antigravity"));
    assert_eq!(caller.label, "antigravity 9999999");

    // Unknown provider
    let env = mock_env(&[("PASEO_AGENT_ID", "5555555")]);
    let caller = resolve_agent(&env, &[], "testhost", Path::new("/cwd"));
    assert_eq!(caller.provider.as_deref(), Some("unknown"));
    assert_eq!(caller.label, "unknown 5555555");
}

#[test]
fn test_identity_rule2_claudecode() {
    let env = mock_env(&[
        ("CLAUDECODE", "1"),
        ("CLAUDE_CODE_SESSION_ID", "session-abcdef-123456"),
    ]);
    let caller = resolve_agent(&env, &[], "testhost", Path::new("/cwd"));
    assert_eq!(caller.kind, ActorKind::Agent);
    assert_eq!(caller.provider.as_deref(), Some("claude"));
    assert_eq!(caller.external_id, "session-abcdef-123456");
    assert_eq!(caller.label, "claude session");
}

#[test]
fn test_identity_rule3_all_known_binaries() {
    let binaries = ["claude", "codex", "agy", "antigravity", "gemini"];
    for bin in binaries {
        let env = mock_env(&[]);
        let chain = vec![ProcInfo {
            pid: 4321,
            name: bin.into(),
        }];
        let caller = resolve_agent(&env, &chain, "host_x", Path::new("/path"));
        assert_eq!(caller.kind, ActorKind::Agent);
        assert_eq!(caller.provider.as_deref(), Some(bin));
        assert_eq!(caller.external_id, format!("proc:host_x:{bin}:4321"));
        assert_eq!(caller.label, format!("{bin} 4321"));
    }
}

#[test]
fn test_identity_rule4_anon_agent() {
    let env = mock_env(&[]);
    let chain = vec![ProcInfo {
        pid: 1,
        name: "systemd".into(),
    }];
    let caller = resolve_agent(&env, &chain, "devbox", Path::new("/home/user/project"));
    assert_eq!(caller.kind, ActorKind::Agent);
    assert_eq!(caller.provider.as_deref(), Some("unknown"));
    assert_eq!(caller.external_id, "anon:devbox:/home/user/project");
    assert_eq!(caller.label, "anon devbox");
}

#[test]
fn test_identity_resolve_human_allowed() {
    let env = mock_env(&[]);
    let chain = vec![ProcInfo {
        pid: 12,
        name: "zsh".into(),
    }];
    let caller = resolve_human(&env, &chain).expect("human resolution should succeed");
    assert_eq!(caller.kind, ActorKind::Human);
    assert_eq!(caller.provider, None);
    assert_eq!(caller.external_id, "human");
    assert_eq!(caller.label, "human");
}

#[test]
fn test_identity_resolve_human_refused_all_markers() {
    // 1. Paseo agent id
    let env = mock_env(&[("PASEO_AGENT_ID", "p-agent-1")]);
    let err = resolve_human(&env, &[]).unwrap_err();
    assert_eq!(
        err,
        IdentityError::AgentMarkers("PASEO_AGENT_ID".to_string())
    );

    // 2. Claude code env vars
    let env = mock_env(&[("CLAUDECODE", "1")]);
    let err = resolve_human(&env, &[]).unwrap_err();
    assert_eq!(err, IdentityError::AgentMarkers("CLAUDECODE".to_string()));

    let env = mock_env(&[("CLAUDE_CODE_SESSION_ID", "sess-1")]);
    let err = resolve_human(&env, &[]).unwrap_err();
    assert_eq!(
        err,
        IdentityError::AgentMarkers("CLAUDE_CODE_SESSION_ID".to_string())
    );

    // 3. AI_AGENT env var
    let env = mock_env(&[("AI_AGENT", "codex")]);
    let err = resolve_human(&env, &[]).unwrap_err();
    assert_eq!(err, IdentityError::AgentMarkers("AI_AGENT".to_string()));

    // 4. Known binary in process chain
    for bin in ["claude", "codex", "agy", "antigravity", "gemini"] {
        let env = mock_env(&[]);
        let chain = vec![ProcInfo {
            pid: 777,
            name: bin.into(),
        }];
        let err = resolve_human(&env, &chain).unwrap_err();
        assert_eq!(err, IdentityError::AgentMarkers(bin.to_string()));
    }

    // 5. Multiple markers together
    let env = mock_env(&[("PASEO_AGENT_ID", "paseo-1"), ("CLAUDECODE", "1")]);
    let chain = vec![ProcInfo {
        pid: 888,
        name: "codex".into(),
    }];
    let err = resolve_human(&env, &chain).unwrap_err();
    assert_eq!(
        err,
        IdentityError::AgentMarkers("PASEO_AGENT_ID, CLAUDECODE, codex".to_string())
    );
}

#[test]
fn test_identity_resolve_current() {
    // Running as agent should succeed
    let caller = resolve_current(false).expect("agent caller resolution should succeed");
    assert_eq!(caller.kind, ActorKind::Agent);
    assert!(!caller.external_id.is_empty());
    assert!(!caller.label.is_empty());

    let chain = process_chain();
    assert!(!chain.is_empty());
}

#[test]
fn test_identity_serialization() {
    let caller = Caller {
        kind: ActorKind::Agent,
        provider: Some("codex".to_string()),
        external_id: "403254b".to_string(),
        label: "codex 403254b".to_string(),
    };
    let json = serde_json::to_string(&caller).expect("Caller should serialize to json");
    assert!(json.contains("\"kind\":\"Agent\""));
    assert!(json.contains("\"provider\":\"codex\""));
    assert!(json.contains("\"external_id\":\"403254b\""));
}
