//! Git repository metadata, project keys, and live claim evidence (DESIGN.md D4, D11).

use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How long a `gh pr view` result (including "no PR") is reused. The board
/// refetches on every change event, so without this each refresh paid one
/// network round trip per claimed task.
const GH_CACHE_TTL: Duration = Duration::from_secs(60);
/// A `gh` call slower than this is killed and treated as "no PR info".
const GH_TIMEOUT: Duration = Duration::from_secs(8);
/// Concurrent `gh` processes during [`prefetch_prs`].
const GH_PARALLEL: usize = 8;

/// Normalise a git remote URL into a project key (DESIGN.md "Projects and instances").
///
/// Examples:
/// `git@github.com:Org/Repo.git` -> `github.com/Org/Repo`
/// `https://user@GITHUB.COM/Org/Repo.git` -> `github.com/Org/Repo`
pub fn normalize_remote(url: &str) -> String {
    let mut s = url.trim();

    // 1. Strip scheme if present (e.g. "https://", "ssh://", "git://", "http://")
    let had_scheme = if let Some(idx) = s.find("://") {
        s = &s[idx + 3..];
        true
    } else {
        false
    };

    // 2. Strip user / authentication info if present before '@'
    if let Some(at_idx) = s.find('@')
        && s.find('/').is_none_or(|slash| at_idx < slash)
    {
        s = &s[at_idx + 1..];
    }

    // 3. Separate host and path
    let (raw_host, raw_path) = match (s.find(':'), s.find('/')) {
        (Some(colon_idx), Some(slash_idx)) if colon_idx < slash_idx && !had_scheme => {
            (&s[..colon_idx], &s[colon_idx + 1..])
        }
        (Some(colon_idx), None) if !had_scheme => (&s[..colon_idx], &s[colon_idx + 1..]),
        (_, Some(slash_idx)) => (&s[..slash_idx], &s[slash_idx + 1..]),
        _ => (s, ""),
    };

    // Strip port from host if present (e.g. "github.com:22" -> "github.com")
    let host = if let Some(colon_idx) = raw_host.rfind(':') {
        let potential_port = &raw_host[colon_idx + 1..];
        if potential_port.chars().all(|c| c.is_ascii_digit()) && !potential_port.is_empty() {
            &raw_host[..colon_idx]
        } else {
            raw_host
        }
    } else {
        raw_host
    };
    let host_norm = host.trim().to_ascii_lowercase();

    // 4. Clean path: trim leading/trailing slashes, strip .git suffix
    let mut path_norm = raw_path.trim().trim_matches('/');
    if let Some(stripped) = path_norm.strip_suffix(".git") {
        path_norm = stripped.trim_matches('/');
    }

    if path_norm.is_empty() {
        host_norm
    } else {
        format!("{}/{}", host_norm, path_norm)
    }
}

/// Normalised origin remote, else "dir:<toplevel folder name>", None outside git.
pub fn project_key(dir: &Path) -> Option<String> {
    let toplevel_out = run_git(dir, &["rev-parse", "--show-toplevel"])?;
    if !toplevel_out.status.success() {
        return None;
    }
    let toplevel_str = std::str::from_utf8(&toplevel_out.stdout).ok()?.trim();
    if toplevel_str.is_empty() {
        return None;
    }

    // Try `git remote get-url origin`
    if let Some(remote_out) = run_git(dir, &["remote", "get-url", "origin"])
        && remote_out.status.success()
    {
        let url = String::from_utf8_lossy(&remote_out.stdout)
            .trim()
            .to_string();
        if !url.is_empty() {
            return Some(normalize_remote(&url));
        }
    }

    // Fallback: "dir:<toplevel folder name>"
    let toplevel_path = Path::new(toplevel_str);
    let folder_name = toplevel_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    Some(format!("dir:{}", folder_name))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Instance {
    pub host: String,
    pub toplevel: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
}

/// Instance = host + toplevel path + current branch + HEAD commit.
pub fn instance(dir: &Path) -> Option<Instance> {
    let toplevel_out = run_git(dir, &["rev-parse", "--show-toplevel"])?;
    if !toplevel_out.status.success() {
        return None;
    }
    let toplevel_str = std::str::from_utf8(&toplevel_out.stdout).ok()?.trim();
    if toplevel_str.is_empty() {
        return None;
    }
    let toplevel = PathBuf::from(toplevel_str);

    let host = gethostname::gethostname().to_string_lossy().to_string();

    let branch = run_git(dir, &["branch", "--show-current"])
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        });

    let head = run_git(dir, &["rev-parse", "HEAD"])
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        });

    Some(Instance {
        host,
        toplevel,
        branch,
        head,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrInfo {
    pub number: u64,
    pub state: String,
    pub url: String,
    pub checks: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Evidence {
    pub commits: u32,
    pub rebased: bool,
    pub diffstat: Option<String>,
    pub dirty: bool,
    pub pr: Option<PrInfo>,
    pub reachable: bool,
}

/// Compact diffstat formatting: " 4 files changed, 133 insertions(+)" -> "4 files changed, +133".
pub fn compact_diffstat(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return String::new();
    }
    let files_part = parts[0];
    let mut out_parts = Vec::new();
    out_parts.push(files_part.to_string());

    let mut recognized = true;
    for part in &parts[1..] {
        if part.contains("insertion") {
            let digits: String = part.chars().filter(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                out_parts.push(format!("+{digits}"));
            } else {
                recognized = false;
            }
        } else if part.contains("deletion") {
            let digits: String = part.chars().filter(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                out_parts.push(format!("-{digits}"));
            } else {
                recognized = false;
            }
        } else {
            recognized = false;
        }
    }
    if !recognized {
        return raw.to_string();
    }

    out_parts.join(", ")
}

impl Evidence {
    /// One-line evidence summary for `fb done`, e.g. "evidence: 1 commit, 4 files changed, +133, clean".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        parts.push(if self.commits == 1 {
            "1 commit".to_string()
        } else {
            format!("{} commits", self.commits)
        });
        if self.rebased {
            parts.push("rebased".to_string());
        }
        if let Some(ds) = &self.diffstat {
            let c = compact_diffstat(ds);
            if !c.is_empty() {
                parts.push(c);
            }
        }
        parts.push(if self.dirty {
            "dirty".to_string()
        } else {
            "clean".to_string()
        });
        if let Some(pr) = &self.pr {
            parts.push(format!("PR #{} {}", pr.number, pr.state));
        }
        format!("evidence: {}", parts.join(", "))
    }
}

/// DESIGN.md "Evidence" table. Missing cwd -> reachable=false, zeros.
pub fn evidence(
    cwd: &Path,
    base_commit: &str,
    branch: &str,
    claimed_at: DateTime<Utc>,
    use_gh: bool,
) -> Evidence {
    // 1. Missing cwd or not a directory -> reachable = false, zeros
    if !cwd.is_dir() {
        return Evidence {
            commits: 0,
            rebased: false,
            diffstat: None,
            dirty: false,
            pr: None,
            reachable: false,
        };
    }

    // 2. Not in a git repo -> reachable = false, zeros
    let is_git = run_git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !is_git {
        return Evidence {
            commits: 0,
            rebased: false,
            diffstat: None,
            dirty: false,
            pr: None,
            reachable: false,
        };
    }

    // 3. Rebase detection: base_commit is an ancestor of branch
    let is_ancestor = run_git(cwd, &["merge-base", "--is-ancestor", base_commit, branch])
        .map(|o| o.status.success())
        .unwrap_or(false);
    let rebased = !is_ancestor;

    // 4. Commits count:
    // If not rebased: git rev-list --count <base_commit>..<branch>
    // If rebased: git rev-list --count --since=<claimed_at> <branch>
    let commits = if !rebased {
        let rev_spec = format!("{base_commit}..{branch}");
        run_git(cwd, &["rev-list", "--count", &rev_spec])
            .and_then(|o| {
                if o.status.success() {
                    let s = std::str::from_utf8(&o.stdout).ok()?.trim();
                    s.parse::<u32>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    } else {
        let since_arg = format!("--since={}", claimed_at.to_rfc3339());
        run_git(cwd, &["rev-list", "--count", &since_arg, branch])
            .and_then(|o| {
                if o.status.success() {
                    let s = std::str::from_utf8(&o.stdout).ok()?.trim();
                    s.parse::<u32>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    };

    // 5. Diffstat: git diff --shortstat <base_commit>...<branch>
    let diff_spec = format!("{base_commit}...{branch}");
    let diffstat = run_git(cwd, &["diff", "--shortstat", &diff_spec]).and_then(|o| {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        } else {
            None
        }
    });

    // 6. Dirty worktree: git status --porcelain is non-empty
    let dirty = run_git(cwd, &["status", "--porcelain"])
        .map(|o| o.status.success() && !o.stdout.trim_ascii().is_empty())
        .unwrap_or(false);

    // 7. PR info via gh CLI
    let pr = if use_gh {
        query_gh_pr(cwd, branch)
    } else {
        None
    };

    Evidence {
        commits,
        rebased,
        diffstat,
        dirty,
        pr,
        reachable: true,
    }
}

/// Last commit time on the given branch (committer date).
pub fn last_commit_time(cwd: &Path, branch: &str) -> Option<DateTime<Utc>> {
    let out = run_git(cwd, &["log", "-1", "--format=%ct", branch])?;
    if !out.status.success() {
        return None;
    }
    let s = std::str::from_utf8(&out.stdout).ok()?.trim();
    let ts: i64 = s.parse().ok()?;
    DateTime::from_timestamp(ts, 0)
}

/// Helper to execute git command in specified directory.
fn run_git(dir: &Path, args: &[&str]) -> Option<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(dir.as_os_str())
        .args(args)
        .output()
        .ok()
}

#[derive(serde::Deserialize)]
struct GhPrJson {
    number: u64,
    state: String,
    url: String,
    #[serde(rename = "statusCheckRollup")]
    status_check_rollup: Option<serde_json::Value>,
}

type PrKey = (PathBuf, String);
type PrCache = HashMap<PrKey, (Instant, Option<PrInfo>)>;

fn pr_cache() -> &'static Mutex<PrCache> {
    static CACHE: OnceLock<Mutex<PrCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_pr(key: &PrKey) -> Option<Option<PrInfo>> {
    let cache = pr_cache().lock().unwrap_or_else(|e| e.into_inner());
    cache
        .get(key)
        .filter(|(at, _)| at.elapsed() < GH_CACHE_TTL)
        .map(|(_, pr)| pr.clone())
}

fn store_pr(key: PrKey, pr: Option<PrInfo>) {
    let mut cache = pr_cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, (at, _)| at.elapsed() < GH_CACHE_TTL);
    cache.insert(key, (Instant::now(), pr));
}

/// Whether `gh` runs at all; checked once per process.
fn gh_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE
        .get_or_init(|| run_with_timeout(Command::new("gh").arg("--version"), GH_TIMEOUT).is_some())
}

/// Run a command, capturing stdout; `None` on spawn failure, non-zero exit or
/// timeout (the child is killed).
fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<Vec<u8>> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Drain stdout on a thread so a large reply cannot fill the pipe and stall
    // the child until the timeout.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let out = reader.join().ok()?;
    status.filter(|s| s.success()).map(|_| out)
}

fn query_gh_pr(cwd: &Path, branch: &str) -> Option<PrInfo> {
    let key = (cwd.to_path_buf(), branch.to_string());
    if let Some(hit) = cached_pr(&key) {
        return hit;
    }
    let pr = query_gh_pr_uncached(cwd, branch);
    store_pr(key, pr.clone());
    pr
}

/// Warm the PR cache for many (cwd, branch) pairs with up to `GH_PARALLEL`
/// concurrent `gh` calls, so a following loop of [`evidence`] calls hits the
/// cache instead of paying one network round trip per task in sequence.
pub fn prefetch_prs(pairs: &[(PathBuf, String)]) {
    if pairs.is_empty() || !gh_available() {
        return;
    }
    let mut seen = HashSet::new();
    let todo: Vec<&(PathBuf, String)> = pairs
        .iter()
        .filter(|k| seen.insert((*k).clone()) && k.0.is_dir() && cached_pr(k).is_none())
        .collect();
    for chunk in todo.chunks(GH_PARALLEL) {
        std::thread::scope(|scope| {
            for (cwd, branch) in chunk {
                scope.spawn(move || {
                    let pr = query_gh_pr_uncached(cwd, branch);
                    store_pr((cwd.clone(), branch.clone()), pr);
                });
            }
        });
    }
}

fn query_gh_pr_uncached(cwd: &Path, branch: &str) -> Option<PrInfo> {
    if !gh_available() {
        return None;
    }
    let out = run_with_timeout(
        Command::new("gh").current_dir(cwd).args([
            "pr",
            "view",
            branch,
            "--json",
            "number,state,url,statusCheckRollup",
        ]),
        GH_TIMEOUT,
    )?;

    let parsed: GhPrJson = serde_json::from_slice(&out).ok()?;
    let checks = parse_status_check_rollup(parsed.status_check_rollup);

    Some(PrInfo {
        number: parsed.number,
        state: parsed.state,
        url: parsed.url,
        checks,
    })
}

fn parse_status_check_rollup(val: Option<serde_json::Value>) -> Option<String> {
    let val = val?;
    match val {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        serde_json::Value::Array(arr) if !arr.is_empty() => {
            for item in &arr {
                if let Some(conclusion) = item.get("conclusion").and_then(|v| v.as_str()) {
                    let c = conclusion.to_ascii_uppercase();
                    if c == "FAILURE"
                        || c == "TIMED_OUT"
                        || c == "ACTION_REQUIRED"
                        || c == "CANCELLED"
                    {
                        return Some("failure".to_string());
                    }
                }
            }
            let all_success = arr.iter().all(|item| {
                let concl = item.get("conclusion").and_then(|v| v.as_str());
                concl == Some("SUCCESS")
            });
            if all_success {
                Some("success".to_string())
            } else {
                Some("pending".to_string())
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_temp_repo() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("create temp dir");
        let path = dir.path().to_path_buf();

        Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(["init", "-b", "main"])
            .output()
            .expect("git init");

        Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(["config", "user.name", "Test User"])
            .output()
            .expect("git config user.name");

        Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .expect("git config user.email");

        (dir, path)
    }

    fn commit_file(repo_path: &Path, file_name: &str, content: &str, msg: &str) -> String {
        std::fs::write(repo_path.join(file_name), content).expect("write file");
        Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["add", file_name])
            .output()
            .expect("git add");
        Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["commit", "-m", msg])
            .output()
            .expect("git commit");

        let out = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn test_normalize_remote_cases() {
        assert_eq!(
            normalize_remote("git@github.com:Org/Repo.git"),
            "github.com/Org/Repo"
        );
        assert_eq!(
            normalize_remote("https://user@GITHUB.COM/Org/Repo.git"),
            "github.com/Org/Repo"
        );
        assert_eq!(
            normalize_remote("https://user:password@github.com/Org/Repo.git"),
            "github.com/Org/Repo"
        );
        assert_eq!(
            normalize_remote("git@GITHUB.COM:Org/Repo"),
            "github.com/Org/Repo"
        );
        assert_eq!(
            normalize_remote("ssh://git@github.com:22/Org/Repo.git"),
            "github.com/Org/Repo"
        );
        assert_eq!(
            normalize_remote("git://github.com/Org/Repo.git"),
            "github.com/Org/Repo"
        );
        assert_eq!(
            normalize_remote("git@gitlab.com:deep/sub/group/repo.git"),
            "gitlab.com/deep/sub/group/repo"
        );
        assert_eq!(
            normalize_remote("https://github.com/Org/Repo/"),
            "github.com/Org/Repo"
        );
    }

    #[test]
    fn test_project_key_with_remote() {
        let (_tmp, path) = setup_temp_repo();
        commit_file(&path, "README.md", "hello", "initial commit");

        Command::new("git")
            .arg("-C")
            .arg(&path)
            .args([
                "remote",
                "add",
                "origin",
                "git@github.com:AcmeCorp/AwesomeProject.git",
            ])
            .output()
            .expect("git remote add");

        let key = project_key(&path).expect("project key should exist");
        assert_eq!(key, "github.com/AcmeCorp/AwesomeProject");
    }

    #[test]
    fn test_project_key_no_remote() {
        let (_tmp, path) = setup_temp_repo();
        commit_file(&path, "README.md", "hello", "initial commit");

        let folder = path.file_name().unwrap().to_str().unwrap();
        let key = project_key(&path).expect("project key should exist");
        assert_eq!(key, format!("dir:{folder}"));
    }

    #[test]
    fn test_project_key_outside_git() {
        let tmp = TempDir::new().expect("create temp dir");
        assert_eq!(project_key(tmp.path()), None);
    }

    #[test]
    fn test_instance_resolution() {
        let (_tmp, path) = setup_temp_repo();
        let head_commit = commit_file(&path, "file.txt", "v1", "first");

        let inst = instance(&path).expect("instance should be found");
        assert!(!inst.host.is_empty());
        assert_eq!(inst.toplevel, path.canonicalize().unwrap_or(path.clone()));
        assert_eq!(inst.branch.as_deref(), Some("main"));
        assert_eq!(inst.head.as_deref(), Some(head_commit.as_str()));
    }

    #[test]
    fn test_last_commit_time() {
        let (_tmp, path) = setup_temp_repo();
        commit_file(&path, "file.txt", "v1", "first");

        let dt = last_commit_time(&path, "main");
        assert!(dt.is_some());

        let invalid = last_commit_time(&path, "nonexistent-branch");
        assert_eq!(invalid, None);
    }

    #[test]
    fn test_evidence_normal_flow() {
        let (_tmp, path) = setup_temp_repo();
        let base = commit_file(&path, "file.txt", "v1", "base commit");
        let claim_time = Utc::now();

        // Add 2 commits
        commit_file(&path, "file.txt", "v2", "commit 1");
        commit_file(&path, "file2.txt", "new", "commit 2");

        let ev = evidence(&path, &base, "main", claim_time, false);
        assert!(ev.reachable);
        assert!(!ev.rebased);
        assert_eq!(ev.commits, 2);
        assert!(!ev.dirty);
        assert!(ev.diffstat.is_some());
        assert_eq!(ev.pr, None);
    }

    #[test]
    fn test_evidence_dirty_worktree() {
        let (_tmp, path) = setup_temp_repo();
        let base = commit_file(&path, "file.txt", "v1", "base");
        let claim_time = Utc::now();

        // Modify file without committing
        std::fs::write(path.join("file.txt"), "modified").unwrap();

        let ev = evidence(&path, &base, "main", claim_time, false);
        assert!(ev.reachable);
        assert!(ev.dirty);
        assert_eq!(ev.commits, 0);
    }

    #[test]
    fn test_evidence_rebase_fallback() {
        let (_tmp, path) = setup_temp_repo();
        let _base = commit_file(&path, "base.txt", "base", "base commit");

        // Create feature branch from base
        Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(["checkout", "-b", "feature"])
            .output()
            .expect("checkout -b feature");

        let claim_time = Utc::now() - chrono::Duration::minutes(10);

        // Add a commit on feature branch
        commit_file(&path, "feat.txt", "feature", "feature commit");

        // Simulate rebase or base_commit no longer being an ancestor
        // by testing with an unrelated fake base commit or rewritten history
        // Let's create an orphan commit for base
        Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(["checkout", "--orphan", "unrelated"])
            .output()
            .expect("orphan branch");
        let orphan_base = commit_file(&path, "orphan.txt", "orphan", "orphan commit");

        let ev = evidence(&path, &orphan_base, "feature", claim_time, false);
        assert!(ev.reachable);
        assert!(ev.rebased);
        assert!(ev.commits >= 1);
    }

    #[test]
    fn test_evidence_missing_cwd() {
        let non_existent = Path::new("/nonexistent/directory/for/testing/12345");
        let ev = evidence(non_existent, "abc", "main", Utc::now(), false);
        assert!(!ev.reachable);
        assert_eq!(ev.commits, 0);
        assert!(!ev.rebased);
        assert_eq!(ev.diffstat, None);
        assert!(!ev.dirty);
        assert_eq!(ev.pr, None);
    }
}
