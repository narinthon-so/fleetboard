//! Integration tests for the Paseo runtime adapter (M2):
//! - Fake paseo binary on PATH
//! - Fixtures for: running, idle, permission, archived, missing, binary absent, garbage output
//! - Longest prefix workspace matching
//! - Caching (~5 s)
//! - Archiving an agent marks its claim stale within one poll
//! - Pending permissions join the Needs-me view
//! - Graceful degradation when binary is absent or errors

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use assert_cmd::Command;
use common::{Env, Who};
use fleetboard::adapters::{
    AgentRuntimeStatus, PaseoAdapter, RuntimeAdapter, find_matching_workspace,
};
use serde_json::Value;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

struct FakePaseo {
    bin_dir: tempfile::TempDir,
    state_file: PathBuf,
    cwd_file: PathBuf,
}

impl FakePaseo {
    fn new() -> Self {
        let bin_dir = tempfile::tempdir().unwrap();
        let state_file = bin_dir.path().join("state.txt");
        let cwd_file = bin_dir.path().join("cwd.txt");
        let script_path = bin_dir.path().join("paseo");

        let script = r#"#!/bin/bash
DIR="$(dirname "$0")"
STATE=$(cat "$DIR/state.txt" 2>/dev/null || echo "running")
CWD=$(cat "$DIR/cwd.txt" 2>/dev/null || echo "/tmp")

case "$1" in
  workspace)
    if [ "$STATE" = "garbage" ]; then
      echo "not valid json {{"
      exit 0
    elif [ "$STATE" = "missing" ]; then
      echo "[]"
      exit 0
    else
      echo "[{\"workspaceId\": \"wks_001\", \"cwd\": \"$CWD\", \"project\": \"test-proj\"}]"
      exit 0
    fi
    ;;
  ls)
    if [ "$STATE" = "garbage" ]; then
      echo "not valid json {{"
      exit 0
    elif [ "$STATE" = "archived" ] || [ "$STATE" = "missing" ]; then
      echo "[]"
      exit 0
    else
      echo "[{\"id\": \"agent-123\", \"shortId\": \"a123\", \"status\": \"$STATE\", \"cwd\": \"$CWD\"}]"
      exit 0
    fi
    ;;
  inspect)
    if [ "$STATE" = "garbage" ]; then
      echo "not valid json {{"
      exit 0
    elif [ "$STATE" = "missing" ]; then
      echo '{"error": {"code": "AGENT_NOT_FOUND", "message": "Agent not found: agent-123"}}' >&2
      exit 1
    elif [ "$STATE" = "archived" ]; then
      echo "{\"Id\": \"agent-123\", \"Status\": \"archived\", \"Archived\": true, \"Cwd\": \"$CWD\", \"PendingPermissions\": []}"
      exit 0
    elif [ "$STATE" = "permission" ]; then
      echo "{\"Id\": \"agent-123\", \"Status\": \"permission\", \"Archived\": false, \"Cwd\": \"$CWD\", \"PendingPermissions\": [{\"id\": \"p1\"}]}"
      exit 0
    elif [ "$STATE" = "idle" ]; then
      echo "{\"Id\": \"agent-123\", \"Status\": \"idle\", \"Archived\": false, \"Cwd\": \"$CWD\", \"PendingPermissions\": []}"
      exit 0
    else
      echo "{\"Id\": \"agent-123\", \"Status\": \"running\", \"Archived\": false, \"Cwd\": \"$CWD\", \"PendingPermissions\": []}"
      exit 0
    fi
    ;;
  *)
    exit 1
    ;;
esac
"#;

        std::fs::write(&script_path, script).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        std::fs::write(&state_file, "running").unwrap();
        std::fs::write(&cwd_file, "/tmp").unwrap();

        Self {
            bin_dir,
            state_file,
            cwd_file,
        }
    }

    fn set_state(&self, state: &str) {
        std::fs::write(&self.state_file, state).unwrap();
    }

    fn set_cwd(&self, cwd: &str) {
        std::fs::write(&self.cwd_file, cwd).unwrap();
    }

    fn path_env(&self) -> String {
        format!("{}:/usr/bin:/bin", self.bin_dir.path().display())
    }
}

// -----------------------------------------------------------------------------
// Direct Unit Tests for PaseoAdapter
// -----------------------------------------------------------------------------

#[test]
fn test_longest_prefix_workspace_matching() {
    use fleetboard::adapters::paseo::PaseoWorkspace;

    let ws = vec![
        PaseoWorkspace {
            workspace_id: "wks_root".into(),
            project: None,
            name: None,
            cwd: "/home/user/code/repo".into(),
        },
        PaseoWorkspace {
            workspace_id: "wks_sub".into(),
            project: None,
            name: None,
            cwd: "/home/user/code/repo/workspaces/ws1".into(),
        },
    ];

    // Child in sub-workspace matches the more specific workspace
    let m1 = find_matching_workspace(&ws, "/home/user/code/repo/workspaces/ws1/src");
    assert_eq!(m1.unwrap().workspace_id, "wks_sub");

    // File in root matches root workspace
    let m2 = find_matching_workspace(&ws, "/home/user/code/repo/src");
    assert_eq!(m2.unwrap().workspace_id, "wks_root");

    // Unrelated path does not match
    let m3 = find_matching_workspace(&ws, "/other/path");
    assert!(m3.is_none());
}

#[test]
fn test_adapter_fixtures_with_fake_binary() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let fake = FakePaseo::new();
    let old_path = std::env::var("PATH").unwrap_or_default();

    unsafe {
        std::env::set_var("PATH", fake.path_env());
    }

    let adapter = PaseoAdapter::with_ttl(Duration::from_millis(50));

    // 1. Fixture: running
    fake.set_state("running");
    adapter.clear_cache();
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Running)
    );
    assert_eq!(adapter.pending_permissions("agent-123"), 0);

    // 2. Fixture: idle
    fake.set_state("idle");
    adapter.clear_cache();
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Idle)
    );
    assert_eq!(adapter.pending_permissions("agent-123"), 0);

    // 3. Fixture: permission
    fake.set_state("permission");
    adapter.clear_cache();
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Permission)
    );
    assert_eq!(adapter.pending_permissions("agent-123"), 1);

    // 4. Fixture: archived
    fake.set_state("archived");
    adapter.clear_cache();
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Gone)
    );
    assert_eq!(adapter.pending_permissions("agent-123"), 0);

    // 5. Fixture: missing
    fake.set_state("missing");
    adapter.clear_cache();
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Gone)
    );
    assert_eq!(adapter.pending_permissions("agent-123"), 0);

    // 6. Fixture: garbage output -> degrades gracefully to none (None)
    fake.set_state("garbage");
    adapter.clear_cache();
    assert_eq!(adapter.agent_status("agent-123"), None);
    assert_eq!(adapter.pending_permissions("agent-123"), 0);

    // 7. Fixture: binary absent -> degrades gracefully to none (None)
    unsafe {
        std::env::set_var("PATH", "/nonexistent_bin_dir");
    }
    adapter.clear_cache();
    assert_eq!(adapter.agent_status("agent-123"), None);
    assert_eq!(adapter.pending_permissions("agent-123"), 0);

    // Restore environment
    unsafe {
        std::env::set_var("PATH", old_path);
    }
}

#[test]
fn test_adapter_caching_behavior() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let fake = FakePaseo::new();
    let old_path = std::env::var("PATH").unwrap_or_default();
    unsafe {
        std::env::set_var("PATH", fake.path_env());
    }

    // 5-second cache
    let adapter = PaseoAdapter::with_ttl(Duration::from_secs(5));
    fake.set_state("running");
    adapter.clear_cache();
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Running)
    );

    // Change fake state behind the scenes
    fake.set_state("archived");

    // Still returns cached running value
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Running)
    );

    // After clearing cache, observes the new archived state
    adapter.clear_cache();
    assert_eq!(
        adapter.agent_status("agent-123"),
        Some(AgentRuntimeStatus::Gone)
    );

    unsafe {
        std::env::set_var("PATH", old_path);
    }
}

// -----------------------------------------------------------------------------
// CLI Integration Tests with Fake Paseo
// -----------------------------------------------------------------------------

fn run_fb_with_fake(env: &Env, fake: &FakePaseo, who: Who, args: &[&str]) -> common::Run {
    let mut cmd = Command::cargo_bin("fb").unwrap();
    cmd.current_dir(&env.work_path)
        .env_clear()
        .env("PATH", fake.path_env())
        .env("HOME", env.home.path())
        .env("FB_HOME", env.home.path())
        .env("FB_TEST_CHAIN", "")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("FAKE_PASEO_STATE_FILE", &fake.state_file)
        .env("FAKE_PASEO_CWD_FILE", &fake.cwd_file);

    match who {
        Who::Human => {
            cmd.arg("--as-human");
        }
        Who::Agent(id) => {
            cmd.env("PASEO_AGENT_ID", id).env("AI_AGENT", "codex");
        }
    }

    let mut full_args = args.to_vec();
    full_args.push("--json");
    cmd.args(full_args);

    let output = cmd.output().unwrap();
    common::Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn test_cli_paseo_adapter_running_and_workspace_binding() {
    let fake = FakePaseo::new();
    let env = Env::new();
    fake.set_cwd(&env.work_path.to_string_lossy());
    fake.set_state("running");

    // Configure adapter = "paseo"
    std::fs::write(
        env.home.path().join("config.toml"),
        "gh = false\nadapter = \"paseo\"\n",
    )
    .unwrap();

    // Human creates task
    let r = run_fb_with_fake(&env, &fake, Who::Human, &["add", "Paseo M2 task"]);
    assert_eq!(r.code, 0);

    // Agent claims task
    let r = run_fb_with_fake(&env, &fake, Who::Agent("agent-123"), &["claim", "fb-1"]);
    assert_eq!(r.code, 0);
    let claim_data = r.data();
    let task = &claim_data["task"];
    assert_eq!(task["claim"]["workspace_id"], "wks_001");
    assert_eq!(task["runtime"]["status"], "running");
    assert_eq!(task["runtime"]["pending_permissions"], 0);

    // Show task: runtime status exposed
    let r = run_fb_with_fake(&env, &fake, Who::Human, &["show", "fb-1"]);
    assert_eq!(r.code, 0);
    let show_data = r.data();
    assert_eq!(show_data["task"]["runtime"]["status"], "running");
    assert_eq!(show_data["task"]["derived"]["stale"], false);
    assert_eq!(show_data["task"]["derived"]["needs_me"], false);
}

#[test]
fn test_cli_pending_permissions_join_needs_me() {
    let fake = FakePaseo::new();
    let env = Env::new();
    fake.set_cwd(&env.work_path.to_string_lossy());
    fake.set_state("permission");

    std::fs::write(
        env.home.path().join("config.toml"),
        "gh = false\nadapter = \"paseo\"\n",
    )
    .unwrap();

    // Human creates task
    run_fb_with_fake(&env, &fake, Who::Human, &["add", "Permission task"]);
    // Agent claims
    run_fb_with_fake(&env, &fake, Who::Agent("agent-123"), &["claim", "fb-1"]);

    // List with --needs-me: task MUST be present because of pending permission!
    let r = run_fb_with_fake(&env, &fake, Who::Human, &["list", "--needs-me"]);
    assert_eq!(r.code, 0);
    let data = r.data();
    let tasks = data["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "fb-1");
    assert_eq!(tasks[0]["runtime"]["status"], "permission");
    assert_eq!(tasks[0]["runtime"]["pending_permissions"], 1);
    assert_eq!(tasks[0]["derived"]["needs_me"], true);
}

#[test]
fn test_archiving_agent_marks_claim_stale_within_one_poll() {
    let fake = FakePaseo::new();
    let env = Env::new();
    fake.set_cwd(&env.work_path.to_string_lossy());
    fake.set_state("running");

    std::fs::write(
        env.home.path().join("config.toml"),
        "gh = false\nadapter = \"paseo\"\n",
    )
    .unwrap();

    run_fb_with_fake(&env, &fake, Who::Human, &["add", "Staleness test task"]);
    run_fb_with_fake(&env, &fake, Who::Agent("agent-123"), &["claim", "fb-1"]);

    // Check initially not stale
    let r = run_fb_with_fake(&env, &fake, Who::Human, &["show", "fb-1"]);
    assert_eq!(r.data()["task"]["derived"]["stale"], false);
    assert_eq!(r.data()["task"]["derived"]["needs_me"], false);

    // Operator archives the agent in Paseo
    fake.set_state("archived");

    // Within one poll/query, the claim is marked stale!
    let r = run_fb_with_fake(&env, &fake, Who::Human, &["show", "fb-1"]);
    let data = r.data();
    assert_eq!(data["task"]["runtime"]["status"], "gone");
    assert_eq!(data["task"]["derived"]["stale"], true);
    assert_eq!(data["task"]["derived"]["needs_me"], true);

    // Stale task joins the Needs-me queue
    let r = run_fb_with_fake(&env, &fake, Who::Human, &["list", "--needs-me"]);
    let list_data = r.data();
    assert_eq!(list_data["tasks"][0]["id"], "fb-1");
}

#[test]
fn test_missing_agent_marks_claim_stale() {
    let fake = FakePaseo::new();
    let env = Env::new();
    fake.set_cwd(&env.work_path.to_string_lossy());
    fake.set_state("running");

    std::fs::write(
        env.home.path().join("config.toml"),
        "gh = false\nadapter = \"paseo\"\n",
    )
    .unwrap();

    run_fb_with_fake(&env, &fake, Who::Human, &["add", "Missing agent task"]);
    run_fb_with_fake(&env, &fake, Who::Agent("agent-123"), &["claim", "fb-1"]);

    // Agent deleted/missing from Paseo
    fake.set_state("missing");

    let r = run_fb_with_fake(&env, &fake, Who::Human, &["show", "fb-1"]);
    let data = r.data();
    assert_eq!(data["task"]["runtime"]["status"], "gone");
    assert_eq!(data["task"]["derived"]["stale"], true);
    assert_eq!(data["task"]["derived"]["needs_me"], true);
}

#[test]
fn test_paseo_garbage_output_degrades_gracefully() {
    let fake = FakePaseo::new();
    let env = Env::new();
    fake.set_cwd(&env.work_path.to_string_lossy());
    fake.set_state("garbage");

    std::fs::write(
        env.home.path().join("config.toml"),
        "gh = false\nadapter = \"paseo\"\n",
    )
    .unwrap();

    run_fb_with_fake(&env, &fake, Who::Human, &["add", "Task"]);
    // Command succeeds with exit code 0 even when paseo gives garbage
    let r = run_fb_with_fake(&env, &fake, Who::Human, &["list"]);
    assert_eq!(r.code, 0);
    assert!(r.data()["tasks"].is_array());
}

#[test]
fn test_paseo_absent_binary_degrades_gracefully() {
    let _fake = FakePaseo::new();
    let env = Env::new();

    std::fs::write(
        env.home.path().join("config.toml"),
        "gh = false\nadapter = \"paseo\"\n",
    )
    .unwrap();

    // Point PATH to an empty directory without paseo binary
    let empty_dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("fb").unwrap();
    cmd.current_dir(&env.work_path)
        .env_clear()
        .env("PATH", empty_dir.path())
        .env("HOME", env.home.path())
        .env("FB_HOME", env.home.path())
        .env("FB_TEST_CHAIN", "")
        .arg("--as-human")
        .args(["add", "Task", "--json"]);

    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(0));

    let val: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(val["ok"], true);
}
