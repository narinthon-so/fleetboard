//! Integration tests for the Hooks runtime adapter (M3):
//! - fb hook <event> (session-start | stop | tool-use | prompt)
//! - Claude Code (hookSpecificOutput JSON) and Codex (plain text) session-start output
//! - Runtime status transitions: tool-use/prompt -> running, stop -> idle, gone after stale_after
//! - Exit code always 0 on garbage stdin, missing board, unknown events
//! - Sub-100ms execution time on small boards
//! - Touch/activity refresh on claims

mod common;

use std::time::{Duration, Instant};

use common::{Env, Who};
use fleetboard::adapters::{AgentRuntimeStatus, HooksAdapter, RuntimeAdapter};
use serde_json::Value;

#[test]
fn test_each_hook_event_and_status_transitions() {
    let env = Env::new();
    std::fs::write(
        env.home.path().join("config.toml"),
        "adapter = \"hooks\"\nstale_after = \"2h\"\ngh = false\n",
    )
    .unwrap();

    let task_id = env.todo("Hook test task");
    let agent = Who::Agent("agent-hook-1");
    env.ok(agent, &["claim", &task_id]);

    let adapter = HooksAdapter::new(env.home.path().to_path_buf(), chrono::Duration::hours(2));

    // Initially with no hook events, adapter has no live report for the agent
    assert_eq!(adapter.agent_status("agent-hook-1"), None);

    // 1. session-start -> running
    let payload = serde_json::json!({
        "session_id": "agent-hook-1",
        "cwd": env.work_path.to_str().unwrap(),
        "hook_event_name": "SessionStart"
    });
    let mut cmd = env.cmd(agent, &["hook", "session-start"]);
    cmd.write_stdin(payload.to_string());
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        adapter.agent_status("agent-hook-1"),
        Some(AgentRuntimeStatus::Running)
    );

    // 2. prompt -> running
    let payload = serde_json::json!({
        "session_id": "agent-hook-1",
        "cwd": env.work_path.to_str().unwrap(),
        "prompt": "write more tests"
    });
    let mut cmd = env.cmd(agent, &["hook", "prompt"]);
    cmd.write_stdin(payload.to_string());
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        adapter.agent_status("agent-hook-1"),
        Some(AgentRuntimeStatus::Running)
    );

    // 3. tool-use -> running
    let payload = serde_json::json!({
        "session_id": "agent-hook-1",
        "cwd": env.work_path.to_str().unwrap(),
        "tool_name": "Bash",
        "tool_input": { "command": "cargo test" }
    });
    let mut cmd = env.cmd(agent, &["hook", "tool-use"]);
    cmd.write_stdin(payload.to_string());
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        adapter.agent_status("agent-hook-1"),
        Some(AgentRuntimeStatus::Running)
    );

    // 4. stop -> idle
    let payload = serde_json::json!({
        "session_id": "agent-hook-1",
        "cwd": env.work_path.to_str().unwrap(),
        "hook_event_name": "Stop"
    });
    let mut cmd = env.cmd(agent, &["hook", "stop"]);
    cmd.write_stdin(payload.to_string());
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        adapter.agent_status("agent-hook-1"),
        Some(AgentRuntimeStatus::Idle)
    );

    // 5. gone after stale_after with no event
    let t0 = chrono::Utc::now();
    let t_stale = t0 + chrono::Duration::hours(3);
    assert_eq!(
        adapter.agent_status_at("agent-hook-1", t_stale),
        Some(AgentRuntimeStatus::Gone)
    );

    // Verify task is marked stale in fleetboard list --needs-me
    env.set_now(t_stale);
    let list_data = env.ok(Who::Human, &["list", "--needs-me"]);
    let tasks = list_data["tasks"].as_array().unwrap();
    assert!(
        tasks.iter().any(|t| t["id"] == task_id),
        "stale task should appear in needs-me"
    );
}

#[test]
fn test_session_start_claude_code_and_codex_output() {
    let env = Env::new();
    let task_id = env.todo("Rate limit endpoints");
    let agent = Who::Agent("agent-claude-test");
    env.ok(agent, &["claim", &task_id]);

    // Claude Code caller (identified by source: startup or CLAUDECODE=1)
    let payload = serde_json::json!({
        "session_id": "agent-claude-test",
        "source": "startup",
        "cwd": env.work_path.to_str().unwrap(),
        "hook_event_name": "SessionStart"
    });
    let mut cmd = env.cmd(agent, &["hook", "session-start"]);
    cmd.write_stdin(payload.to_string());
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("Claude session-start stdout was not valid JSON: {stdout} ({e})")
    });
    assert_eq!(
        v["hookSpecificOutput"]["hookEventName"], "SessionStart",
        "should output Claude Code hookSpecificOutput"
    );
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additionalContext should be string");
    assert!(
        ctx.contains("fleetboard — you are"),
        "context must contain fb prime greeting"
    );
    assert!(
        ctx.contains("Rate limit endpoints"),
        "context must contain claimed task title"
    );

    // Codex caller (plain text output)
    let payload_codex = serde_json::json!({
        "session_id": "agent-codex-test",
        "cwd": env.work_path.to_str().unwrap(),
        "hook_event_name": "SessionStart"
    });
    let mut cmd = env.cmd(Who::Agent("agent-codex-test"), &["hook", "session-start"]);
    cmd.write_stdin(payload_codex.to_string());
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.trim_start().starts_with('{'),
        "Codex session-start output should be plain text, not JSON: {stdout}"
    );
    assert!(
        stdout.contains("fleetboard — you are"),
        "should output fb prime greeting in plain text"
    );
    assert!(
        stdout.contains("You have no active claim"),
        "should state no active claim for this agent"
    );
}

#[test]
fn test_hook_garbage_stdin_exits_zero() {
    let env = Env::new();
    let agent = Who::Agent("agent-garbage-test");

    for event in &[
        "session-start",
        "prompt",
        "tool-use",
        "stop",
        "unknown-event",
    ] {
        let mut cmd = env.cmd(agent, &["hook", event]);
        cmd.write_stdin("!! NOT JSON DATA %%^&*() << garbage >>");
        let out = cmd.output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "fb hook {event} must exit 0 on garbage stdin"
        );
    }
}

#[test]
fn test_hook_missing_board_exits_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let missing_home = tmp.path().join("definitely-missing-board");
    let mut cmd = assert_cmd::Command::cargo_bin("fb").unwrap();
    cmd.args(["--home", missing_home.to_str().unwrap(), "hook", "stop"]);
    cmd.write_stdin("{\"session_id\":\"xyz\"}");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0), "must exit 0 on missing board");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty(),
        "should report warning to stderr on missing board"
    );
}

#[test]
fn test_hook_timing_under_100ms() {
    let env = Env::new();
    let task_id = env.todo("Benchmark task");
    let agent = Who::Agent("agent-bench");
    env.ok(agent, &["claim", &task_id]);

    let payload = serde_json::json!({
        "session_id": "agent-bench",
        "cwd": env.work_path.to_str().unwrap(),
        "tool_name": "Bash",
        "tool_input": { "command": "echo test" }
    });

    // Run once to warm up any filesystem buffers
    let mut cmd = env.cmd(agent, &["hook", "tool-use"]);
    cmd.write_stdin(payload.to_string());
    let _ = cmd.output().unwrap();

    let start = Instant::now();
    let mut cmd = env.cmd(agent, &["hook", "tool-use"]);
    cmd.write_stdin(payload.to_string());
    let out = cmd.output().unwrap();
    let elapsed = start.elapsed();

    assert_eq!(out.status.code(), Some(0));
    assert!(
        elapsed < Duration::from_millis(100),
        "fb hook tool-use took {:?}, expected < 100ms",
        elapsed
    );
}

#[test]
fn test_hook_bumps_claim_activity() {
    let env = Env::new();
    let task_id = env.todo("Keepalive task");
    let agent = Who::Agent("agent-keepalive");
    env.ok(agent, &["claim", &task_id]);

    let t0 = chrono::Utc::now();
    env.set_now(t0);

    // Check initial claim activity
    let detail = env.ok(Who::Human, &["show", &task_id]);
    let initial_activity = detail["task"]["claim"]["last_activity"]
        .as_str()
        .unwrap()
        .to_string();

    // Advance clock by 30 minutes
    let t1 = t0 + chrono::Duration::minutes(30);
    env.set_now(t1);

    // Call fb hook tool-use
    let payload = serde_json::json!({
        "session_id": "agent-keepalive",
        "cwd": env.work_path.to_str().unwrap(),
        "tool_name": "Edit",
        "tool_input": { "path": "file.txt" }
    });
    let mut cmd = env.cmd(agent, &["hook", "tool-use"]);
    cmd.write_stdin(payload.to_string());
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));

    // Verify claim's last_activity was updated to t1
    let detail2 = env.ok(Who::Human, &["show", &task_id]);
    let updated_activity = detail2["task"]["claim"]["last_activity"]
        .as_str()
        .unwrap()
        .to_string();

    assert_ne!(
        initial_activity, updated_activity,
        "last_activity should be refreshed"
    );
}
