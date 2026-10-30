//! CLI behaviour against a temp FB_HOME and temp git repos.

mod common;

use common::{Env, Who};
use serde_json::json;

const A: Who = Who::Agent("agent-aaaaaaa");
const B: Who = Who::Agent("agent-bbbbbbb");
const H: Who = Who::Human;

#[test]
fn agent_tasks_are_drafts_and_the_project_key_is_normalised() {
    let e = Env::new();
    let d = e.ok(
        A,
        &[
            "add",
            "Write docs",
            "--ac",
            "builds",
            "--ac",
            "has examples",
        ],
    );
    let t = &d["task"];
    assert_eq!(t["id"], "fb-1");
    assert_eq!(t["status"], "draft");
    assert_eq!(t["project"]["key"], "github.com/Org/Repo");
    assert_eq!(t["project"]["name"], "Repo");
    assert_eq!(t["acceptance"], "- [ ] builds\n- [ ] has examples");
    assert_eq!(t["derived"]["needs_me"], true);
    let d = e.ok(H, &["add", "Human task"]);
    assert_eq!(d["task"]["status"], "todo");
    assert_eq!(d["task"]["created_by"]["kind"], "human");
}

#[test]
fn project_key_without_a_remote_is_the_folder_name() {
    let e = Env::new();
    e.git(&["remote", "remove", "origin"]);
    let d = e.ok(H, &["add", "x"]);
    let folder = e
        .work_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(d["task"]["project"]["key"], format!("dir:{folder}"));
}

#[test]
fn outside_git_there_is_no_project_and_ready_lists_everything() {
    let e = Env::new();
    e.ok(H, &["add", "in repo"]);
    let elsewhere = tempfile::tempdir().unwrap();
    let r = e.run_in(elsewhere.path(), H, &["add", "loose"]);
    assert_eq!(r.data()["task"]["project"], json!(null));
    let r = e.run_in(elsewhere.path(), H, &["ready"]);
    assert_eq!(r.data()["tasks"].as_array().unwrap().len(), 2);
    // In the repo only the repo's tasks are Ready.
    let d = e.ok(H, &["ready"]);
    assert_eq!(d["tasks"].as_array().unwrap().len(), 1);
    let d = e.ok(H, &["ready", "--all"]);
    assert_eq!(d["tasks"].as_array().unwrap().len(), 2);
}

#[test]
fn human_only_commands_are_refused_for_agents() {
    let e = Env::new();
    e.ok(A, &["add", "draft"]);
    for args in [
        vec!["promote", "fb-1"],
        vec!["approve", "fb-1"],
        vec!["verify", "fb-1"],
        vec!["reject", "fb-1", "no"],
        vec!["release", "fb-1"],
        vec!["drop", "fb-1"],
        vec!["reopen", "fb-1"],
        vec!["trust", "1"],
        vec!["untrust", "1"],
        vec!["project", "alias", "github.com/Org/Repo", "x"],
        vec!["claim", "fb-1", "--steal"],
    ] {
        e.fails(A, &args, 4, "HUMAN_ONLY");
    }
    e.ok(H, &["promote", "fb-1"]);
}

#[test]
fn as_human_is_refused_under_every_agent_marker() {
    let e = Env::new();
    let human = |setup: &dyn Fn(&mut assert_cmd::Command)| {
        let mut c = e.cmd(H, &["ready", "--json"]);
        setup(&mut c);
        let out = c.output().unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    };
    let (code, _) = human(&|_| {});
    assert_eq!(code, 0, "clean environment is accepted");
    let (code, out) = human(&|c| {
        c.env("PASEO_AGENT_ID", "x");
    });
    assert_eq!(code, 4, "{out}");
    assert!(out.contains("HUMAN_ONLY"), "{out}");
    let (code, _) = human(&|c| {
        c.env("CLAUDECODE", "1").env("CLAUDE_CODE_SESSION_ID", "s1");
    });
    assert_eq!(code, 4);
    let (code, _) = human(&|c| {
        c.env("FB_TEST_CHAIN", "bash,codex,node");
    });
    assert_eq!(code, 4, "agent binary in the process chain");
}

#[test]
fn agents_are_distinguished_by_their_environment() {
    let e = Env::new();
    e.ok(H, &["add", "t"]);
    e.ok(A, &["claim", "fb-1"]);
    let d = e.ok(B, &["show", "fb-1"]);
    let who = &d["task"]["claim"]["actor"];
    assert_eq!(who["external_id"], "agent-aaaaaaa");
    assert_eq!(who["kind"], "agent");
    e.fails(B, &["claim", "fb-1"], 3, "CLAIM_HELD");
}

#[test]
fn claim_done_verify_with_git_evidence() {
    let e = Env::new();
    e.ok(H, &["add", "Ship it"]);
    let base = e.head();
    let d = e.ok(A, &["claim", "fb-1"]);
    let c = &d["task"]["claim"];
    assert_eq!(d["task"]["status"], "in_progress");
    assert_eq!(c["branch"], "main");
    assert_eq!(c["base_commit"], base.as_str());
    assert_eq!(c["cwd"], e.work_path.to_str().unwrap());

    // The agent moves to a feature branch and commits; its next fb call refreshes the claim.
    e.git(&["checkout", "-q", "-b", "feature"]);
    e.write("a.txt", "one\n");
    e.commit("add a");
    e.write("b.txt", "two\n");
    e.commit("add b");
    e.ok(A, &["note", "fb-1", "halfway", "--kind", "handoff"]);
    let d = e.ok(A, &["done", "fb-1"]);
    assert_eq!(d["task"]["status"], "review");
    assert_eq!(d["evidence"]["commits"], 2);
    assert_eq!(d["evidence"]["source"], "live");
    assert_eq!(d["warnings"], json!([]));
    assert_eq!(d["task"]["claim"]["branch"], "feature");
    assert_eq!(
        d["task"]["claim"]["base_commit"],
        base.as_str(),
        "base never changes"
    );

    let d = e.ok(H, &["show", "fb-1", "--evidence"]);
    let ev = &d["task"]["evidence"];
    assert_eq!(ev["commits"], 2);
    assert_eq!(ev["rebased"], false);
    assert_eq!(ev["dirty"], false);
    assert_eq!(ev["reachable"], true);
    assert_eq!(ev["pr"], json!(null));
    assert!(ev["diffstat"].as_str().unwrap().contains("2 files changed"));
    assert_eq!(d["task"]["derived"]["needs_me"], true);

    let d = e.ok(H, &["verify", "fb-1"]);
    assert_eq!(d["task"]["status"], "done");
    assert_eq!(d["task"]["claim"], json!(null));
    let d = e.ok(H, &["show", "fb-1"]);
    assert_eq!(d["task"]["claims"][0]["release_reason"], "done");
    assert_eq!(
        d["task"]["evidence"],
        json!(null),
        "evidence only with --evidence"
    );
}

#[test]
fn done_without_commits_goes_to_review_flagged_and_verify_needs_force() {
    let e = Env::new();
    e.ok(H, &["add", "Empty"]);
    e.ok(A, &["claim", "fb-1"]);
    let d = e.ok(A, &["done", "fb-1"]);
    assert_eq!(d["task"]["status"], "review");
    assert_eq!(d["warnings"], json!(["claimed, no commits"]));
    assert_eq!(d["task"]["derived"]["no_evidence"], true);
    assert_eq!(d["task"]["derived"]["needs_me"], true);
    let t = e.text(A, &["show", "fb-1"]);
    assert!(t.stdout.contains("no-evidence"), "{}", t.stdout);
    e.fails(H, &["verify", "fb-1"], 3, "BAD_TRANSITION");
    let d = e.ok(H, &["verify", "fb-1", "--force"]);
    assert_eq!(d["task"]["status"], "done");
}

#[test]
fn dirty_worktree_and_rebase_fallback() {
    let e = Env::new();
    e.ok(H, &["add", "Rewrite"]);
    e.ok(A, &["claim", "fb-1"]);
    e.write("scratch.txt", "uncommitted\n");
    let d = e.ok(H, &["show", "fb-1", "--evidence"]);
    assert_eq!(d["task"]["evidence"]["dirty"], true);
    std::fs::remove_file(e.work_path.join("scratch.txt")).unwrap();

    // Replace the history so the claim's base commit is no longer an ancestor.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    e.git(&["checkout", "-q", "--orphan", "rewritten"]);
    e.write("new.txt", "fresh\n");
    e.commit("rewritten root");
    e.ok(A, &["note", "fb-1", "rewrote history", "--kind", "comment"]);
    let d = e.ok(H, &["show", "fb-1", "--evidence"]);
    let ev = &d["task"]["evidence"];
    assert_eq!(ev["rebased"], true);
    assert_eq!(ev["commits"], 1, "falls back to commits since the claim");
}

#[test]
fn unreachable_checkout_falls_back_to_the_snapshot() {
    let e = Env::new();
    e.ok(H, &["add", "Gone"]);
    e.ok(A, &["claim", "fb-1"]);
    e.write("x.txt", "x\n");
    e.commit("work");
    e.ok(A, &["done", "fb-1"]);
    // The claim's cwd disappears; verification still sees the evidence recorded at `done`.
    let other = tempfile::tempdir().unwrap();
    std::fs::rename(&e.work_path, other.path().join("moved")).unwrap();
    let d = e
        .run_in(other.path(), H, &["show", "fb-1", "--evidence"])
        .data();
    let ev = &d["task"]["evidence"];
    assert_eq!(ev["source"], "snapshot");
    assert_eq!(ev["reachable"], false);
    assert_eq!(ev["commits"], 1);
    let d = e.run_in(other.path(), H, &["verify", "fb-1"]).data();
    assert_eq!(d["task"]["status"], "done");
}

#[test]
fn claim_without_an_id_picks_by_priority_then_age_in_this_project() {
    let e = Env::new();
    e.ok(H, &["add", "low", "--prio", "3"]);
    e.ok(H, &["add", "normal"]);
    e.ok(H, &["add", "urgent", "--prio", "0"]);
    let elsewhere = tempfile::tempdir().unwrap();
    e.run_in(
        elsewhere.path(),
        H,
        &[
            "add",
            "other project",
            "--prio",
            "0",
            "--project",
            "github.com/Org/Other",
        ],
    );
    assert_eq!(e.ok(A, &["claim"])["task"]["title"], "urgent");
    assert_eq!(e.ok(A, &["claim"])["task"]["title"], "normal");
    assert_eq!(e.ok(A, &["claim"])["task"]["title"], "low");
    e.fails(A, &["claim"], 2, "NOT_FOUND");
}

#[test]
fn blocked_tasks_are_not_ready_and_dependencies_refuse_cycles() {
    let e = Env::new();
    e.ok(H, &["add", "first"]);
    e.ok(H, &["add", "second", "--blocked-by", "fb-1"]);
    let d = e.ok(H, &["ready"]);
    let ids: Vec<_> = d["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].clone())
        .collect();
    assert_eq!(ids, [json!("fb-1")]);
    e.fails(A, &["claim", "fb-2"], 3, "NOT_READY");
    e.fails(H, &["dep", "add", "fb-1", "fb-2"], 3, "DEP_CYCLE");
    e.fails(H, &["dep", "add", "fb-1", "fb-1"], 3, "DEP_CYCLE");
    e.fails(H, &["dep", "add", "fb-1", "fb-9"], 2, "NOT_FOUND");
    e.ok(A, &["claim", "fb-1"]);
    e.ok(A, &["done", "fb-1"]);
    e.ok(H, &["verify", "fb-1", "--force"]);
    assert_eq!(e.ok(H, &["ready"])["tasks"][0]["id"], "fb-2");
    e.ok(H, &["dep", "rm", "fb-2", "fb-1"]);
    e.fails(H, &["dep", "rm", "fb-2", "fb-1"], 2, "NOT_FOUND");
}

#[test]
fn dropped_blocker_orphans_its_dependants() {
    let e = Env::new();
    e.ok(H, &["add", "blocker"]);
    e.ok(H, &["add", "dependant", "--blocked-by", "1"]);
    e.ok(H, &["drop", "fb-1", "--reason", "not needed"]);
    let d = e.ok(H, &["show", "fb-2"]);
    let der = &d["task"]["derived"];
    assert_eq!(
        (
            der["orphaned"].clone(),
            der["blocked"].clone(),
            der["ready"].clone()
        ),
        (json!(true), json!(true), json!(false))
    );
    let d = e.ok(H, &["list", "--needs-me"]);
    assert_eq!(d["tasks"][0]["id"], "fb-2");
    e.ok(H, &["dep", "rm", "fb-2", "fb-1"]);
    assert_eq!(e.ok(H, &["ready"])["tasks"][0]["id"], "fb-2");
    e.ok(H, &["reopen", "fb-1"]);
    assert_eq!(e.ok(H, &["show", "fb-1"])["task"]["status"], "todo");
}

#[test]
fn reject_sends_it_back_with_a_comment_and_keeps_the_claim() {
    let e = Env::new();
    e.ok(H, &["add", "t"]);
    e.ok(A, &["claim", "fb-1"]);
    e.ok(A, &["done", "fb-1"]);
    e.fails(H, &["reject", "fb-1", ""], 1, "USAGE");
    let d = e.ok(H, &["reject", "fb-1", "tests are missing"]);
    assert_eq!(d["task"]["status"], "in_progress");
    assert_eq!(d["task"]["claim"]["actor"]["external_id"], "agent-aaaaaaa");
    let d = e.ok(H, &["show", "fb-1"]);
    let note = &d["task"]["notes"][0];
    assert_eq!(
        (note["kind"].clone(), note["body"].clone()),
        (json!("comment"), json!("tests are missing"))
    );
    e.ok(A, &["done", "fb-1"]);
}

#[test]
fn claim_conflicts_and_claimant_checks() {
    let e = Env::new();
    e.ok(H, &["add", "t"]);
    e.ok(A, &["claim", "fb-1"]);
    e.fails(B, &["claim", "fb-1"], 3, "CLAIM_HELD");
    e.fails(B, &["plan", "fb-1", "mine now"], 4, "NOT_CLAIMANT");
    e.fails(B, &["done", "fb-1"], 4, "NOT_CLAIMANT");
    e.ok(B, &["note", "fb-1", "drive-by", "--kind", "comment"]);
    e.fails(A, &["claim", "fb-9"], 2, "NOT_FOUND");
    e.fails(A, &["claim", "banana"], 1, "USAGE");
    let d = e.ok(H, &["release", "fb-1"]);
    assert_eq!(d["task"]["status"], "todo");
    assert_eq!(d["task"]["claim"], json!(null));
    e.ok(B, &["claim", "fb-1"]);
}

#[test]
fn steal_and_the_trusted_role() {
    let e = Env::new();
    e.ok(H, &["add", "t"]);
    e.ok(A, &["claim", "fb-1"]);
    e.fails(B, &["claim", "fb-1", "--steal"], 4, "HUMAN_ONLY");
    // Human hands the claim to B by its actor id.
    let b_id = e.ok(B, &["prime"])["actor"]["id"].to_string();
    let d = e.ok(H, &["claim", "fb-1", "--steal", "--for", &b_id]);
    assert_eq!(d["task"]["claim"]["actor"]["external_id"], "agent-bbbbbbb");
    assert_eq!(
        e.ok(H, &["show", "fb-1"])["task"]["claims"][1]["release_reason"],
        "stolen"
    );
    // Trusted agents skip the draft step and may steal.
    e.fails(A, &["trust", &b_id], 4, "HUMAN_ONLY");
    e.ok(H, &["trust", &b_id]);
    assert_eq!(e.ok(B, &["add", "tracker task"])["task"]["status"], "todo");
    e.ok(A, &["note", "fb-1", "still here", "--kind", "comment"]);
    let d = e.ok(B, &["claim", "fb-1", "--steal"]);
    assert_eq!(d["task"]["claim"]["actor"]["external_id"], "agent-bbbbbbb");
    e.ok(H, &["untrust", &b_id]);
    assert_eq!(e.ok(B, &["add", "again"])["task"]["status"], "draft");
}

#[test]
fn plan_approval_gate() {
    let e = Env::new();
    e.ok(H, &["add", "gated", "--plan-approval"]);
    e.ok(A, &["claim", "fb-1"]);
    let d = e.ok(A, &["plan", "fb-1", "1. do the thing"]);
    assert_eq!(d["task"]["derived"]["plan_pending"], true);
    assert_eq!(d["task"]["derived"]["needs_me"], true);
    e.fails(A, &["done", "fb-1"], 3, "BAD_TRANSITION");
    e.ok(H, &["approve", "fb-1"]);
    let d = e.ok(H, &["show", "fb-1"]);
    assert_eq!(d["task"]["derived"]["plan_pending"], false);
    e.fails(H, &["approve", "fb-1"], 3, "BAD_TRANSITION");
    e.ok(A, &["done", "fb-1"]);
}

#[test]
fn note_tasks_close_on_a_result_without_git_evidence() {
    let e = Env::new();
    e.ok(H, &["add", "Investigate", "--kind", "note"]);
    e.ok(A, &["claim", "fb-1"]);
    e.fails(A, &["done", "fb-1"], 1, "USAGE");
    let d = e.ok(A, &["done", "fb-1", "--result", "It is the cache."]);
    assert_eq!(d["task"]["result"], "It is the cache.");
    assert_eq!(d["task"]["derived"]["no_evidence"], false);
    let d = e.ok(H, &["verify", "fb-1"]);
    assert_eq!(d["task"]["status"], "done");
}

#[test]
fn list_filters() {
    let e = Env::new();
    e.ok(A, &["add", "draft one"]);
    e.ok(H, &["add", "todo one"]);
    e.ok(H, &["add", "todo two"]);
    e.ok(A, &["claim", "fb-2"]);
    let titles = |args: &[&str]| -> Vec<String> {
        e.ok(H, args)["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(titles(&["list"]).len(), 3);
    assert_eq!(titles(&["list", "--status", "draft"]), ["draft one"]);
    assert_eq!(titles(&["list", "--status", "draft,todo"]).len(), 2);
    assert_eq!(titles(&["list", "--agent", "codex"]), ["todo one"]);
    assert_eq!(titles(&["list", "--agent", "nobody"]).len(), 0);
    let host = e.ok(H, &["show", "fb-2"])["task"]["claim"]["host"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(titles(&["list", "--host", &host]), ["todo one"]);
    assert_eq!(titles(&["list", "--host", "elsewhere"]).len(), 0);
    assert_eq!(titles(&["list", "--needs-me"]), ["draft one"]);
    assert_eq!(
        titles(&["list", "--project", "github.com/Org/Repo"]).len(),
        3
    );
    e.fails(H, &["list", "--project", "nope"], 2, "NOT_FOUND");
    e.fails(H, &["list", "--status", "weird"], 1, "USAGE");
    e.ok(A, &["done", "fb-2"]);
    e.ok(H, &["verify", "fb-2", "--force"]);
    assert_eq!(
        titles(&["list"]).len(),
        2,
        "done tasks are hidden by default"
    );
    assert_eq!(titles(&["list", "--all"]).len(), 3);
}

#[test]
fn stale_claims_show_up_in_needs_me_and_activity_keeps_them_fresh() {
    let e = Env::new();
    std::fs::write(
        e.home.path().join("config.toml"),
        "gh = false\nstale_after = \"5s\"\n",
    )
    .unwrap();
    e.ok(H, &["add", "slow"]);
    e.ok(A, &["claim", "fb-1"]);
    assert_eq!(
        e.ok(H, &["show", "fb-1"])["task"]["derived"]["stale"],
        false
    );
    std::thread::sleep(std::time::Duration::from_millis(1500));
    e.ok(A, &["note", "fb-1", "alive", "--kind", "comment"]);
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert_eq!(
        e.ok(H, &["show", "fb-1"])["task"]["derived"]["stale"],
        false
    );
    std::thread::sleep(std::time::Duration::from_millis(5500));
    let d = e.ok(H, &["list", "--needs-me"]);
    assert_eq!(d["tasks"][0]["id"], "fb-1");
    assert_eq!(d["tasks"][0]["derived"]["stale"], true);
    // Never auto-released.
    assert_eq!(d["tasks"][0]["status"], "in_progress");
}

#[test]
fn a_recent_commit_keeps_a_quiet_claim_fresh() {
    let e = Env::new();
    std::fs::write(
        e.home.path().join("config.toml"),
        "gh = false\nstale_after = \"2s\"\n",
    )
    .unwrap();
    e.ok(H, &["add", "busy"]);
    e.ok(A, &["claim", "fb-1"]);
    std::thread::sleep(std::time::Duration::from_millis(2500));
    e.write("w.txt", "w\n");
    e.commit("progress");
    assert_eq!(
        e.ok(H, &["show", "fb-1"])["task"]["derived"]["stale"],
        false
    );
}

#[test]
fn prime_shows_the_claim_and_the_previous_agents_handoff() {
    let e = Env::new();
    let r = e.text(A, &["prime"]);
    assert_eq!(r.code, 0);
    assert!(
        r.stdout.contains("You have no active claim."),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("project github.com/Org/Repo"),
        "{}",
        r.stdout
    );

    e.ok(H, &["add", "Rate-limit"]);
    e.ok(A, &["claim", "fb-1"]);
    e.ok(
        A,
        &[
            "note",
            "fb-1",
            "left off at the middleware\nsee TODO",
            "--kind",
            "handoff",
        ],
    );
    e.ok(H, &["release", "fb-1"]);
    e.ok(B, &["claim", "fb-1"]);
    let r = e.text(B, &["prime"]);
    assert!(
        r.stdout
            .contains("Your claim: fb-1 \"Rate-limit\" (in_progress)"),
        "{}",
        r.stdout
    );
    assert!(r.stdout.contains("Last handoff (codex"), "{}", r.stdout);
    assert!(
        r.stdout
            .contains("  left off at the middleware\n  see TODO\n"),
        "{}",
        r.stdout
    );
    assert!(r.stdout.contains("fb done fb-1"), "{}", r.stdout);
    let d = e.ok(B, &["prime"]);
    assert_eq!(d["claims"][0]["task"]["id"], "fb-1");
    assert!(
        d["claims"][0]["last_handoff"]["body"]
            .as_str()
            .unwrap()
            .contains("middleware")
    );
    assert!(d["text"].as_str().unwrap().starts_with("fleetboard"));
}

#[test]
fn text_and_stdin_input() {
    let e = Env::new();
    let r = e.text(H, &["add", "Plain"]);
    assert_eq!(
        (r.code, r.stdout.as_str()),
        (0, "created fb-1 (todo): Plain\n")
    );
    e.ok(A, &["claim", "fb-1"]);
    let mut c = e.cmd(A, &["plan", "fb-1", "-", "--json"]);
    c.write_stdin("step one\nstep two\n");
    let out = c.output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        e.ok(H, &["show", "fb-1"])["task"]["plan"],
        "step one\nstep two"
    );
    let r = e.text(H, &["list"]);
    assert!(
        r.stdout.starts_with("fb-1    in_progress P2 [Repo] Plain"),
        "{}",
        r.stdout
    );
    let r = e.text(H, &["show", "fb-99"]);
    assert_eq!(r.code, 2);
    assert!(
        r.stdout.is_empty() && r.stderr.contains("NOT_FOUND"),
        "{r:?}",
        r = r.stderr
    );
}

#[test]
fn json_mode_puts_one_value_on_stdout_for_errors_too() {
    let e = Env::new();
    let r = e.run(H, &["show", "fb-1"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.is_empty(), "{}", r.stderr);
    let v = r.json();
    assert_eq!((v["v"].clone(), v["ok"].clone()), (json!(1), json!(false)));
    assert_eq!(v["error"]["code"], "NOT_FOUND");
    let r = e.run(H, &["add"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.error_code(), "USAGE");
    let r = e.run(H, &["frobnicate"]);
    assert_eq!((r.code, r.error_code().as_str()), (1, "USAGE"));
    let r = e.text(H, &["--version"]);
    assert_eq!(r.code, 0);
}

#[test]
fn tickets_are_linked_and_filled_in_by_later_adds() {
    let e = Env::new();
    let d = e.ok(H, &["add", "a", "--ticket", "jira:ABC-36"]);
    assert_eq!(d["task"]["ticket"]["key"], "ABC-36");
    assert_eq!(d["task"]["ticket"]["title"], json!(null));
    e.ok(
        H,
        &[
            "add",
            "b",
            "--ticket",
            "jira:ABC-36",
            "--ticket-title",
            "Parent",
            "--ticket-url",
            "https://example.com/ABC-36",
        ],
    );
    let d = e.ok(H, &["show", "fb-1"]);
    assert_eq!(d["task"]["ticket"]["title"], "Parent");
    assert_eq!(d["task"]["ticket"]["url"], "https://example.com/ABC-36");
    e.fails(H, &["add", "c", "--ticket", "nonsense"], 1, "USAGE");
    e.fails(H, &["add", "c", "--ticket", "svn:1"], 1, "USAGE");
    e.fails(H, &["add", "c", "--ticket-title", "x"], 1, "USAGE");
    e.fails(H, &["add", "c", "--kind", "epic"], 1, "USAGE");
    e.fails(H, &["add", "c", "--prio", "9"], 1, "USAGE");
    e.fails(H, &["add", "c", "--blocked-by", "fb-77"], 2, "NOT_FOUND");
}

#[test]
fn project_alias_and_explicit_project() {
    let e = Env::new();
    e.ok(H, &["add", "a"]);
    let d = e.ok(H, &["project", "alias", "github.com/Org/Repo", "board"]);
    assert_eq!(d["project"]["name"], "board");
    let elsewhere = tempfile::tempdir().unwrap();
    let d = e
        .run_in(elsewhere.path(), H, &["add", "b", "--project", "board"])
        .data();
    assert_eq!(d["task"]["project"]["key"], "github.com/Org/Repo");
    let d = e.ok(H, &["list", "--project", "board"]);
    assert_eq!(d["tasks"].as_array().unwrap().len(), 2);
    e.fails(H, &["project", "alias", "nope", "x"], 2, "NOT_FOUND");
}

#[test]
fn every_action_is_audited_with_the_process_chain() {
    let e = Env::new();
    let mut c = e.cmd(H, &["add", "audited", "--json"]);
    c.env("FB_TEST_CHAIN", "bash,sshd");
    c.assert().success();
    let db = rusqlite::Connection::open(e.home.path().join("board.db")).unwrap();
    let (ty, actor, chain, pid): (String, i64, String, i64) = db
        .query_row(
            "SELECT type, actor_id, proc_chain, pid FROM events WHERE type='created'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (ty.as_str(), actor, chain.as_str()),
        ("created", 1, "bash < sshd")
    );
    assert!(pid > 0);
}

#[test]
fn a_newer_database_is_refused_with_exit_5() {
    let e = Env::new();
    e.ok(H, &["add", "x"]);
    let db = rusqlite::Connection::open(e.home.path().join("board.db")).unwrap();
    db.pragma_update(None, "user_version", 99).unwrap();
    drop(db);
    e.fails(H, &["list"], 5, "SCHEMA_AHEAD");
}

#[test]
fn a_bad_config_is_a_usage_error() {
    let e = Env::new();
    std::fs::write(
        e.home.path().join("config.toml"),
        "stale_after = \"soon\"\n",
    )
    .unwrap();
    e.fails(H, &["list"], 1, "USAGE");
}

#[test]
fn concurrent_processes_claim_with_exactly_one_winner() {
    let e = Env::new();
    e.ok(H, &["add", "contested"]);
    let ids = ["c0", "c1", "c2", "c3", "c4", "c5"];
    let handles: Vec<_> = ids
        .iter()
        .map(|id| {
            let mut c = e.cmd(Who::Agent(id), &["claim", "fb-1", "--json"]);
            std::thread::spawn(move || c.output().unwrap().status.code().unwrap())
        })
        .collect();
    let codes: Vec<i32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(codes.iter().filter(|c| **c == 0).count(), 1, "{codes:?}");
    assert!(codes.iter().all(|c| *c == 0 || *c == 3), "{codes:?}");
}
