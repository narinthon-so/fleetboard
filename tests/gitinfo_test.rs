use chrono::{Duration, Utc};
use fleetboard::gitinfo::{
    Evidence, Instance, PrInfo, compact_diffstat, evidence, instance, last_commit_time,
    normalize_remote, project_key,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn create_temp_git_repo() -> (TempDir, PathBuf) {
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

fn add_commit(repo_path: &Path, file_name: &str, content: &str, msg: &str) -> String {
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
fn test_normalize_remote_comprehensive() {
    // 1. scp-style
    assert_eq!(
        normalize_remote("git@github.com:Org/Repo.git"),
        "github.com/Org/Repo"
    );

    // 2. https with user
    assert_eq!(
        normalize_remote("https://developer@github.com/Org/Repo.git"),
        "github.com/Org/Repo"
    );

    // 3. .git suffix
    assert_eq!(
        normalize_remote("https://github.com/Org/Repo.git"),
        "github.com/Org/Repo"
    );
    assert_eq!(
        normalize_remote("https://github.com/Org/Repo"),
        "github.com/Org/Repo"
    );

    // 4. uppercase host
    assert_eq!(
        normalize_remote("git@GITHUB.COM:Org/Repo.git"),
        "github.com/Org/Repo"
    );
    assert_eq!(
        normalize_remote("https://GITHUB.COM/Org/Repo.git"),
        "github.com/Org/Repo"
    );

    // 5. with credentials & port
    assert_eq!(
        normalize_remote("https://user:secret@github.com:443/Org/Repo.git"),
        "github.com/Org/Repo"
    );
    assert_eq!(
        normalize_remote("ssh://git@github.com:22/Org/Repo.git"),
        "github.com/Org/Repo"
    );

    // 6. multi-tier path
    assert_eq!(
        normalize_remote("git@gitlab.com:team/subgroup/project.git"),
        "gitlab.com/team/subgroup/project"
    );
}

#[test]
fn test_project_key_temp_git_repo_cases() {
    let (_tmp, path) = create_temp_git_repo();
    add_commit(&path, "init.txt", "initial", "initial commit");

    // Case 1: with remote origin added
    Command::new("git")
        .arg("-C")
        .arg(&path)
        .args([
            "remote",
            "add",
            "origin",
            "git@GITHUB.COM:Acme/FleetBoard.git",
        ])
        .output()
        .expect("git remote add");

    let key = project_key(&path).expect("must find project key");
    assert_eq!(key, "github.com/Acme/FleetBoard");

    // Case 2: remove remote origin -> fallback to dir:<name>
    Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["remote", "remove", "origin"])
        .output()
        .expect("git remote remove");

    let folder_name = path.file_name().unwrap().to_str().unwrap();
    let key_fallback = project_key(&path).expect("must fallback to dir");
    assert_eq!(key_fallback, format!("dir:{folder_name}"));

    // Case 3: outside git -> None
    let non_git_tmp = TempDir::new().expect("create non-git temp dir");
    assert_eq!(project_key(non_git_tmp.path()), None);
}

#[test]
fn test_instance_temp_repo() {
    let (_tmp, path) = create_temp_git_repo();
    let head = add_commit(&path, "first.txt", "content", "first commit");

    let inst = instance(&path).expect("instance should be detected");
    assert!(!inst.host.is_empty());
    assert_eq!(inst.toplevel, path.canonicalize().unwrap_or(path.clone()));
    assert_eq!(inst.branch.as_deref(), Some("main"));
    assert_eq!(inst.head.as_deref(), Some(head.as_str()));

    // Outside git
    let non_git = TempDir::new().expect("non-git temp dir");
    assert_eq!(instance(non_git.path()), None);
}

#[test]
fn test_last_commit_time_temp_repo() {
    let (_tmp, path) = create_temp_git_repo();
    add_commit(&path, "doc.txt", "doc", "add doc");

    let ts = last_commit_time(&path, "main");
    assert!(ts.is_some());
    let diff = Utc::now().signed_duration_since(ts.unwrap());
    assert!(diff.num_seconds().abs() < 60);

    // Unknown branch
    assert_eq!(last_commit_time(&path, "does-not-exist"), None);
}

#[test]
fn test_evidence_commits_since_base() {
    let (_tmp, path) = create_temp_git_repo();
    let base = add_commit(&path, "file1.txt", "v1", "base commit");
    let claim_time = Utc::now();

    // Add 3 commits
    add_commit(&path, "file1.txt", "v2", "commit 1");
    add_commit(&path, "file2.txt", "new file", "commit 2");
    add_commit(&path, "file3.txt", "another", "commit 3");

    let ev = evidence(&path, &base, "main", claim_time, false);
    assert!(ev.reachable);
    assert!(!ev.rebased);
    assert_eq!(ev.commits, 3);
    assert!(!ev.dirty);
    assert!(ev.diffstat.is_some());
    assert_eq!(ev.pr, None);
}

#[test]
fn test_evidence_dirty_worktree() {
    let (_tmp, path) = create_temp_git_repo();
    let base = add_commit(&path, "file.txt", "original", "base");
    let claim_time = Utc::now();

    // Modify tracked file
    std::fs::write(path.join("file.txt"), "modified content").unwrap();

    let ev = evidence(&path, &base, "main", claim_time, false);
    assert!(ev.reachable);
    assert!(ev.dirty);
    assert_eq!(ev.commits, 0);

    // Stage it: still dirty
    Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["add", "file.txt"])
        .output()
        .unwrap();
    let ev_staged = evidence(&path, &base, "main", claim_time, false);
    assert!(ev_staged.dirty);

    // Commit it: now clean
    Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["commit", "-m", "clean now"])
        .output()
        .unwrap();
    let ev_clean = evidence(&path, &base, "main", claim_time, false);
    assert!(!ev_clean.dirty);
    assert_eq!(ev_clean.commits, 1);
}

#[test]
fn test_evidence_rebase_fallback() {
    let (_tmp, path) = create_temp_git_repo();
    let _old_base = add_commit(&path, "initial.txt", "init", "initial commit");

    // Create branch feature
    Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["checkout", "-b", "feature"])
        .output()
        .unwrap();

    let claim_time = Utc::now() - Duration::hours(1);
    add_commit(&path, "feature.txt", "work", "feat commit");

    // Create an independent orphan branch and commit to simulate non-ancestor base
    Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["checkout", "--orphan", "other-root"])
        .output()
        .unwrap();
    let orphan_base = add_commit(&path, "other.txt", "other", "other root commit");

    let ev = evidence(&path, &orphan_base, "feature", claim_time, false);
    assert!(ev.reachable);
    assert!(ev.rebased);
    assert!(ev.commits >= 1);
}

#[test]
fn test_evidence_missing_cwd() {
    let non_existent = Path::new("/tmp/definitely_not_existing_path_12345");
    let ev = evidence(non_existent, "abc", "main", Utc::now(), false);
    assert!(!ev.reachable);
    assert_eq!(ev.commits, 0);
    assert!(!ev.rebased);
    assert_eq!(ev.diffstat, None);
    assert!(!ev.dirty);
    assert_eq!(ev.pr, None);
}

#[test]
fn test_evidence_serde() {
    let ev = Evidence {
        commits: 5,
        rebased: false,
        diffstat: Some("1 file changed, 1 insertion(+)".to_string()),
        dirty: false,
        pr: Some(PrInfo {
            number: 10,
            state: "OPEN".to_string(),
            url: "https://github.com/org/repo/pull/10".to_string(),
            checks: Some("success".to_string()),
        }),
        reachable: true,
    };

    let json = serde_json::to_string(&ev).expect("serialize evidence");
    let deserialized: Evidence = serde_json::from_str(&json).expect("deserialize evidence");
    assert_eq!(ev, deserialized);

    let inst = Instance {
        host: "testhost".to_string(),
        toplevel: PathBuf::from("/repo"),
        branch: Some("main".to_string()),
        head: Some("1234567890abcdef".to_string()),
    };
    let inst_json = serde_json::to_string(&inst).expect("serialize instance");
    assert!(inst_json.contains("\"host\":\"testhost\""));
}

#[test]
fn test_compact_diffstat() {
    assert_eq!(
        compact_diffstat(" 4 files changed, 133 insertions(+)"),
        "4 files changed, +133"
    );
    assert_eq!(
        compact_diffstat(" 4 files changed, 133 insertions(+), 12 deletions(-)"),
        "4 files changed, +133, -12"
    );
    assert_eq!(
        compact_diffstat(" 1 file changed, 1 insertion(+)"),
        "1 file changed, +1"
    );
    assert_eq!(
        compact_diffstat(" 1 file changed, 2 deletions(-)"),
        "1 file changed, -2"
    );
    assert_eq!(compact_diffstat(" 1 file changed"), "1 file changed");
    assert_eq!(compact_diffstat(""), "");
}

#[test]
fn test_evidence_summary() {
    let ev = Evidence {
        commits: 1,
        rebased: false,
        diffstat: Some("4 files changed, 133 insertions(+)".to_string()),
        dirty: false,
        pr: None,
        reachable: true,
    };
    assert_eq!(
        ev.summary(),
        "evidence: 1 commit, 4 files changed, +133, clean"
    );

    let ev_dirty = Evidence {
        commits: 2,
        rebased: true,
        diffstat: Some("1 file changed, 2 deletions(-)".to_string()),
        dirty: true,
        pr: None,
        reachable: true,
    };
    assert_eq!(
        ev_dirty.summary(),
        "evidence: 2 commits, rebased, 1 file changed, -2, dirty"
    );
}
