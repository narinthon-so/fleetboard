// PLACEHOLDER — replaced by m0-identity-git
//! Caller identity (D3). Placeholder that follows the contract only.

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

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("--as-human refused: agent markers present ({0})")]
    AgentMarkers(String),
}

const PROVIDERS: [&str; 3] = ["claude", "codex", "agy"];

fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}

fn chain_agent(chain: &[ProcInfo]) -> Option<(&'static str, u32)> {
    chain.iter().find_map(|p| {
        PROVIDERS
            .iter()
            .find(|n| p.name == **n)
            .map(|n| (*n, p.pid))
    })
}

pub fn resolve_agent(
    env: &dyn Fn(&str) -> Option<String>,
    chain: &[ProcInfo],
    host: &str,
    cwd: &Path,
) -> Caller {
    let chain_provider = chain_agent(chain);
    if let Some(id) = env("PASEO_AGENT_ID") {
        let provider = env("AI_AGENT")
            .or_else(|| chain_provider.map(|(n, _)| n.to_string()))
            .unwrap_or_else(|| "unknown".into());
        return Caller {
            label: format!("{provider} {}", short(&id)),
            kind: ActorKind::Agent,
            provider: Some(provider),
            external_id: id,
        };
    }
    if env("CLAUDECODE").as_deref() == Some("1") {
        let id = env("CLAUDE_CODE_SESSION_ID").unwrap_or_else(|| format!("anon:{host}"));
        return Caller {
            label: format!("claude {}", short(&id)),
            kind: ActorKind::Agent,
            provider: Some("claude".into()),
            external_id: id,
        };
    }
    if let Some((name, pid)) = chain_provider {
        return Caller {
            label: format!("{name} {pid}"),
            kind: ActorKind::Agent,
            provider: Some(name.into()),
            external_id: format!("proc:{host}:{name}:{pid}"),
        };
    }
    Caller {
        label: "unknown agent".into(),
        kind: ActorKind::Agent,
        provider: Some("unknown".into()),
        external_id: format!("anon:{host}:{}", cwd.display()),
    }
}

pub fn resolve_human(
    env: &dyn Fn(&str) -> Option<String>,
    chain: &[ProcInfo],
) -> Result<Caller, IdentityError> {
    if env("PASEO_AGENT_ID").is_some() {
        return Err(IdentityError::AgentMarkers("PASEO_AGENT_ID".into()));
    }
    if env("CLAUDECODE").is_some() {
        return Err(IdentityError::AgentMarkers("CLAUDECODE".into()));
    }
    if let Some((name, _)) = chain_agent(chain) {
        return Err(IdentityError::AgentMarkers(format!("process {name}")));
    }
    Ok(Caller {
        kind: ActorKind::Human,
        provider: None,
        external_id: "human".into(),
        label: "human".into(),
    })
}

pub fn process_chain() -> Vec<ProcInfo> {
    Vec::new()
}

pub fn resolve_current(as_human: bool) -> Result<Caller, IdentityError> {
    let env = |k: &str| std::env::var(k).ok();
    let chain = process_chain();
    if as_human {
        return resolve_human(&env, &chain);
    }
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let cwd = std::env::current_dir().unwrap_or_default();
    Ok(resolve_agent(&env, &chain, &host, &cwd))
}
