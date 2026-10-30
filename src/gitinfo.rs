// PLACEHOLDER — replaced by m0-identity-git
//! Project key, instance and evidence. Placeholder that follows the contract only.

use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn normalize_remote(url: &str) -> String {
    let mut s = url.trim();
    if let Some((_, rest)) = s.split_once("://") {
        s = rest;
    }
    if let Some((_, rest)) = s.split_once('@') {
        s = rest;
    }
    let s = s
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .replacen(':', "/", 1);
    match s.split_once('/') {
        Some((host, path)) => format!("{}/{path}", host.to_lowercase()),
        None => s,
    }
}

pub fn project_key(dir: &Path) -> Option<String> {
    let top = git(dir, &["rev-parse", "--show-toplevel"])?;
    if let Some(url) = git(dir, &["remote", "get-url", "origin"]) {
        return Some(normalize_remote(&url));
    }
    let name = Path::new(&top).file_name()?.to_string_lossy().into_owned();
    Some(format!("dir:{name}"))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Instance {
    pub host: String,
    pub toplevel: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
}

pub fn instance(dir: &Path) -> Option<Instance> {
    let top = git(dir, &["rev-parse", "--show-toplevel"])?;
    Some(Instance {
        host: gethostname::gethostname().to_string_lossy().into_owned(),
        toplevel: PathBuf::from(top),
        branch: git(dir, &["branch", "--show-current"]).filter(|b| !b.is_empty()),
        head: git(dir, &["rev-parse", "HEAD"]),
    })
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrInfo {
    pub number: u64,
    pub state: String,
    pub url: String,
    pub checks: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Evidence {
    pub commits: u32,
    pub rebased: bool,
    pub diffstat: Option<String>,
    pub dirty: bool,
    pub pr: Option<PrInfo>,
    pub reachable: bool,
}

pub fn evidence(
    cwd: &Path,
    base_commit: &str,
    branch: &str,
    claimed_at: DateTime<Utc>,
    _use_gh: bool,
) -> Evidence {
    let none = Evidence {
        commits: 0,
        rebased: false,
        diffstat: None,
        dirty: false,
        pr: None,
        reachable: false,
    };
    if !cwd.exists() {
        return none;
    }
    let rebased = git(cwd, &["merge-base", "--is-ancestor", base_commit, branch]).is_none();
    let count = if rebased {
        git(
            cwd,
            &[
                "log",
                "--format=%H",
                &format!("--since={}", claimed_at.to_rfc3339()),
                branch,
            ],
        )
        .map(|s| s.lines().count())
    } else {
        git(
            cwd,
            &["rev-list", "--count", &format!("{base_commit}..{branch}")],
        )
        .and_then(|s| s.parse().ok())
    };
    Evidence {
        commits: count.unwrap_or(0) as u32,
        rebased,
        diffstat: git(
            cwd,
            &["diff", "--shortstat", &format!("{base_commit}...{branch}")],
        )
        .filter(|s| !s.is_empty()),
        dirty: git(cwd, &["status", "--porcelain"]).is_some_and(|s| !s.is_empty()),
        pr: None,
        reachable: true,
    }
}

pub fn last_commit_time(cwd: &Path, branch: &str) -> Option<DateTime<Utc>> {
    let s = git(cwd, &["log", "-1", "--format=%cI", branch])?;
    DateTime::parse_from_rfc3339(&s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}
