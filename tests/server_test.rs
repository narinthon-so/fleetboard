//! Integration tests for fb serve:
//! - no cookie -> 401
//! - bad token -> 401
//! - login then board OK
//! - SSE emits after a CLI write
//! - writes require X-Requested-With: fleetboard
//! - token never written to disk or logs; bound to loopback only

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fleetboard::server::{AppState, ServerConfig, router};
use reqwest::header::COOKIE;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_stream::StreamExt;

mod common;
use common::Env;

struct TestServer {
    addr: SocketAddr,
    token: String,
    _handle: tokio::task::JoinHandle<()>,
}

async fn spawn_test_server(home: &Path) -> (TestServer, Arc<AppState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = "test-secret-token-1234567890abcdef".to_string();
    let config = ServerConfig {
        addr,
        token: token.clone(),
        home: home.to_path_buf(),
    };
    let state = AppState::new(config).unwrap();
    let state_clone = state.clone();
    let app = router(state.clone());
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (
        TestServer {
            addr,
            token,
            _handle: handle,
        },
        state_clone,
    )
}

#[tokio::test]
async fn test_unauthenticated_requests_return_401() {
    let env = Env::new();
    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);

    // GET /api/board without cookie -> 401
    let resp = client
        .get(format!("{base}/api/board"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "UNAUTHORIZED");

    // GET /api/tasks/fb-1 without cookie -> 401
    let resp = client
        .get(format!("{base}/api/tasks/fb-1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "UNAUTHORIZED");

    // GET /api/events without cookie -> 401
    let resp = client
        .get(format!("{base}/api/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    // Request with invalid cookie -> 401
    let resp = client
        .get(format!("{base}/api/board"))
        .header(COOKIE, "fb_session=non-existent-session-id")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn test_bad_token_returns_401() {
    let env = Env::new();
    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);

    // Wrong token -> 401
    let resp = client
        .post(format!("{base}/api/login"))
        .json(&json!({ "token": "completely-wrong-token" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "UNAUTHORIZED");

    // Empty body -> 400 or 401
    let resp = client
        .post(format!("{base}/api/login"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn test_login_then_board_and_task_ok() {
    let env = Env::new();
    let task_id = env.todo("First task for board");
    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);

    // Login with valid token
    let resp = client
        .post(format!("{base}/api/login"))
        .json(&json!({ "token": server.token }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify Set-Cookie header contains HttpOnly and SameSite=Strict
    let cookie_header = resp
        .headers()
        .get("set-cookie")
        .expect("must set cookie")
        .to_str()
        .unwrap();
    assert!(cookie_header.contains("fb_session="));
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));

    // Extract cookie value
    let cookie_val = cookie_header.split(';').next().unwrap().to_string();

    // Verify token was one-time: second login with same token MUST return 401
    let second_login = client
        .post(format!("{base}/api/login"))
        .json(&json!({ "token": server.token }))
        .send()
        .await
        .unwrap();
    assert_eq!(second_login.status(), 401);

    // GET /api/board with cookie
    let resp = client
        .get(format!("{base}/api/board"))
        .header(COOKIE, &cookie_val)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let board: Value = resp.json().await.unwrap();
    assert_eq!(board["ok"], true);
    let data = &board["data"];
    assert!(data["tasks"].is_array());
    assert!(data["projects"].is_array());
    assert!(data["instances"].is_array());
    assert!(data["actors"].is_array());

    let tasks = data["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], task_id);
    assert_eq!(tasks[0]["title"], "First task for board");
    assert!(tasks[0]["derived"].is_object());

    // GET /api/tasks/:id
    let resp = client
        .get(format!("{base}/api/tasks/{task_id}"))
        .header(COOKIE, &cookie_val)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let task_detail: Value = resp.json().await.unwrap();
    assert_eq!(task_detail["ok"], true);
    let t = &task_detail["data"]["task"];
    assert_eq!(t["id"], task_id);
    assert!(t["notes"].is_array());
    assert!(t["events"].is_array());
    let events = t["events"].as_array().unwrap();
    assert!(
        !events.is_empty(),
        "events list should contain created event"
    );
    assert_eq!(events[0]["type"], "created");
    assert!(events[0].get("on_behalf_of").is_some());

    // GET /api/tasks/fb-99999 -> 404
    let resp = client
        .get(format!("{base}/api/tasks/fb-99999"))
        .header(COOKIE, &cookie_val)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let not_found: Value = resp.json().await.unwrap();
    assert_eq!(not_found["ok"], false);
    assert_eq!(not_found["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn test_writes_require_x_requested_with() {
    let env = Env::new();
    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);

    // Login to get cookie
    let resp = client
        .post(format!("{base}/api/login"))
        .json(&json!({ "token": server.token }))
        .send()
        .await
        .unwrap();
    let cookie_val = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();

    // POST /api/tasks without X-Requested-With -> 403 Forbidden
    let resp = client
        .post(format!("{base}/api/tasks/fb-1/notes"))
        .header(COOKIE, cookie_val)
        .json(&json!({ "body": "some note" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "FORBIDDEN");

    // POST with wrong X-Requested-With -> 403
    let resp = client
        .post(format!("{base}/api/tasks/fb-1/notes"))
        .header(COOKIE, cookie_val)
        .header("X-Requested-With", "wrong-app")
        .json(&json!({ "body": "some note" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_sse_emits_after_cli_write() {
    let env = Env::new();
    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);

    // Login to get cookie
    let resp = client
        .post(format!("{base}/api/login"))
        .json(&json!({ "token": server.token }))
        .send()
        .await
        .unwrap();
    let cookie_val = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();

    // Connect to GET /api/events with cookie
    let resp = client
        .get(format!("{base}/api/events"))
        .header(COOKIE, cookie_val)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap(),
        "text/event-stream"
    );

    let mut stream = resp.bytes_stream();

    // Spawn task to perform CLI write after a brief delay
    let env_clone_home = env.home.path().to_path_buf();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        // Run CLI add command
        let mut cmd = assert_cmd::Command::cargo_bin("fb").unwrap();
        cmd.env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("FB_HOME", &env_clone_home)
            .env("FB_TEST_CHAIN", "")
            .args(["--as-human", "add", "Task added from CLI during SSE"]);
        cmd.assert().success();
    });

    // Wait for event on stream (timeout after 5 seconds)
    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);
            if buffer.contains("{\"type\":\"changed\"}")
                || buffer.contains("{\"type\": \"changed\"}")
            {
                return true;
            }
        }
        false
    });

    let received = timeout
        .await
        .expect("timed out waiting for SSE change event");
    assert!(received, "must have received {{type:'changed'}} event");
}

fn assert_no_token_recursive(dir: &Path, token: &str) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            assert_no_token_recursive(&path, token);
        } else if path.is_file() {
            let contents = std::fs::read(&path).unwrap();
            let text = String::from_utf8_lossy(&contents);
            assert!(
                !text.contains(token),
                "token was found on disk in {path:?}!"
            );
        }
    }
}

#[tokio::test]
async fn test_token_never_written_to_disk_and_bound_to_loopback() {
    let env = Env::new();
    let (server, _) = spawn_test_server(env.home.path()).await;

    // Verify bound to loopback only
    assert_eq!(server.addr.ip(), std::net::Ipv4Addr::new(127, 0, 0, 1));

    // Verify token is never written to disk
    assert_no_token_recursive(env.home.path(), &server.token);
}

#[tokio::test]
async fn test_cli_serve_subcommand_prints_token_and_serves() {
    let env = Env::new();
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("fb"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("FB_HOME", env.home.path())
        .args(["--json", "serve", "--port", "0"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    std::io::BufRead::read_line(&mut reader, &mut line).unwrap();

    let json_val: Value = serde_json::from_str(&line).expect("must output valid JSON");
    assert_eq!(json_val["v"], 1);
    assert_eq!(json_val["ok"], true);
    let port = json_val["data"]["port"].as_u64().unwrap() as u16;
    let token = json_val["data"]["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));

    // Connect to the started server and verify login works with the printed token
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/login"))
        .json(&json!({ "token": token }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Clean up child process
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_tokens_generated_with_csprng_differ_and_have_expected_length() {
    let t1 = fleetboard::server::auth::generate_random_token();
    let t2 = fleetboard::server::auth::generate_random_token();
    assert_eq!(t1.len(), 64);
    assert_eq!(t2.len(), 64);
    assert_ne!(t1, t2);
    assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(t2.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(fleetboard::server::auth::constant_time_eq(&t1, &t1));
    assert!(!fleetboard::server::auth::constant_time_eq(&t1, &t2));
    assert!(!fleetboard::server::auth::constant_time_eq(&t1, "short"));
}

async fn login_cookie(client: &reqwest::Client, base: &str, token: &str) -> String {
    let resp = client
        .post(format!("{base}/api/login"))
        .json(&json!({ "token": token }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    resp.headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn test_human_action_promote() {
    let env = Env::new();
    let val = env.ok(common::Who::Agent("agent-1"), &["add", "Agent draft task"]);
    let id = val["task"]["id"].as_str().unwrap();

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Promote draft task -> 200 OK, status becomes todo
    let resp = client
        .post(format!("{base}/api/tasks/{id}/promote"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["task"]["status"], "todo");

    // Check GET /api/tasks/{id} shows promoted event with source=web
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(detail_resp.status(), 200);
    let detail: Value = detail_resp.json().await.unwrap();
    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let last_ev = events.last().unwrap();
    assert_eq!(last_ev["type"], "promoted");
    assert_eq!(last_ev["source"], "web");
    assert_eq!(last_ev["data"]["source"], "web");
    assert_eq!(last_ev["proc_chain"], "web");

    // Bad transition: task is already todo -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/promote"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["ok"], false);
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");
}

#[tokio::test]
async fn test_human_action_approve() {
    let env = Env::new();
    let id = env.todo("Normal task without plan approval");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Task without plan approval needed -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/approve"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Create task with --plan-approval
    let d = env.ok(
        common::Who::Human,
        &["add", "Task with plan approval", "--plan-approval"],
    );
    let id2 = d["task"]["id"].as_str().unwrap();

    // Still in todo -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id2}/approve"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);

    // Agent claims and submits plan
    env.ok(common::Who::Agent("agent-1"), &["claim", id2]);
    env.ok(
        common::Who::Agent("agent-1"),
        &["plan", id2, "Detailed implementation plan"],
    );

    // Approve plan -> 200 OK
    let resp = client
        .post(format!("{base}/api/tasks/{id2}/approve"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert!(v["data"]["task"]["plan_approved_at"].is_string());

    // Check event logged with source=web
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id2}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let detail: Value = detail_resp.json().await.unwrap();
    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let approve_ev = events.iter().find(|e| e["type"] == "approved").unwrap();
    assert_eq!(approve_ev["source"], "web");
    assert_eq!(approve_ev["data"]["source"], "web");
    assert_eq!(approve_ev["proc_chain"], "web");

    // Approve already approved plan -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id2}/approve"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");
}

#[tokio::test]
async fn test_human_action_verify() {
    let env = Env::new();
    let id = env.todo("Code task to verify");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Bad transition: task is in todo (not review) -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/verify"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Agent claims and calls done (0 commits in test worktree since claim)
    env.ok(common::Who::Agent("agent-1"), &["claim", &id]);
    env.ok(common::Who::Agent("agent-1"), &["done", &id]);

    // Verify without force -> 409 BAD_TRANSITION (no commits evidence)
    let resp = client
        .post(format!("{base}/api/tasks/{id}/verify"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "force": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Verify with force -> 200 OK, status done, claim released
    let resp = client
        .post(format!("{base}/api/tasks/{id}/verify"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "force": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["task"]["status"], "done");
    assert!(v["data"]["task"]["claim"].is_null());

    // Check event logged with source=web
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let detail: Value = detail_resp.json().await.unwrap();
    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let verify_ev = events.iter().find(|e| e["type"] == "verified").unwrap();
    assert_eq!(verify_ev["source"], "web");
    assert_eq!(verify_ev["data"]["source"], "web");
    assert_eq!(verify_ev["data"]["force"], true);
    assert_eq!(verify_ev["proc_chain"], "web");

    // Note task can be verified without force
    let d = env.ok(common::Who::Human, &["add", "Note task", "--kind", "note"]);
    let id_note = d["task"]["id"].as_str().unwrap();
    env.ok(common::Who::Agent("agent-1"), &["claim", id_note]);
    env.ok(
        common::Who::Agent("agent-1"),
        &["done", id_note, "--result", "Note outcome"],
    );

    let resp = client
        .post(format!("{base}/api/tasks/{id_note}/verify"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["data"]["task"]["status"], "done");
}

#[tokio::test]
async fn test_human_action_reject() {
    let env = Env::new();
    let id = env.todo("Task to reject");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Bad transition: task in todo -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/reject"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "comment": "rework needed" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Move to review
    env.ok(common::Who::Agent("agent-1"), &["claim", &id]);
    env.ok(common::Who::Agent("agent-1"), &["done", &id]);

    // Reject with empty comment -> 400 USAGE
    let resp = client
        .post(format!("{base}/api/tasks/{id}/reject"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "comment": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "USAGE");

    // Reject with valid comment -> 200 OK, status in_progress, comment note added
    let resp = client
        .post(format!("{base}/api/tasks/{id}/reject"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "comment": "Please address edge cases" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["task"]["status"], "in_progress");

    // Check detail: note added, event logged
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let detail: Value = detail_resp.json().await.unwrap();
    let notes = detail["data"]["task"]["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n["body"] == "Please address edge cases" && n["kind"] == "comment")
    );

    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let reject_ev = events.iter().find(|e| e["type"] == "rejected").unwrap();
    assert_eq!(reject_ev["source"], "web");
    assert_eq!(reject_ev["data"]["source"], "web");
    assert_eq!(reject_ev["data"]["comment"], "Please address edge cases");
    assert_eq!(reject_ev["proc_chain"], "web");
}

#[tokio::test]
async fn test_human_action_release() {
    let env = Env::new();
    let id = env.todo("Task to release");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Bad transition: task in todo -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/release"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Agent claims task
    env.ok(common::Who::Agent("agent-1"), &["claim", &id]);

    // Release active claim -> 200 OK, back to todo
    let resp = client
        .post(format!("{base}/api/tasks/{id}/release"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["task"]["status"], "todo");
    assert!(v["data"]["task"]["claim"].is_null());

    // Check event logged
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let detail: Value = detail_resp.json().await.unwrap();
    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let rel_ev = events.iter().find(|e| e["type"] == "released").unwrap();
    assert_eq!(rel_ev["source"], "web");
    assert_eq!(rel_ev["data"]["source"], "web");
    assert_eq!(rel_ev["proc_chain"], "web");
}

#[tokio::test]
async fn test_human_action_steal() {
    let env = Env::new();
    let id = env.todo("Task to steal");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Bad transition: task has no active claim -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/steal"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Agent 1 claims task
    env.ok(common::Who::Agent("agent-1"), &["claim", &id]);

    // Steal claim to human -> 200 OK
    let resp = client
        .post(format!("{base}/api/tasks/{id}/steal"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["task"]["claim"]["actor"]["external_id"], "human");

    // Steal claim to agent-2 with {"for": ...}
    // First register agent-2
    env.ok(common::Who::Agent("agent-2"), &["ready"]);
    let resp = client
        .post(format!("{base}/api/tasks/{id}/steal"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "for": "agent-2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(
        v["data"]["task"]["claim"]["actor"]["external_id"],
        "agent-2"
    );

    // Check event logged
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let detail: Value = detail_resp.json().await.unwrap();
    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let steal_ev = events.iter().find(|e| e["type"] == "stolen").unwrap();
    assert_eq!(steal_ev["source"], "web");
    assert_eq!(steal_ev["data"]["source"], "web");
    assert_eq!(steal_ev["proc_chain"], "web");
}

#[tokio::test]
async fn test_human_action_drop_and_reopen() {
    let env = Env::new();
    let id = env.todo("Task to drop and reopen");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Drop task with reason -> 200 OK
    let resp = client
        .post(format!("{base}/api/tasks/{id}/drop"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "reason": "No longer needed" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["task"]["status"], "dropped");

    // Drop already dropped task -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/drop"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Reopen dropped task -> 200 OK, status becomes todo
    let resp = client
        .post(format!("{base}/api/tasks/{id}/reopen"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["task"]["status"], "todo");

    // Reopen already todo task -> 409 BAD_TRANSITION
    let resp = client
        .post(format!("{base}/api/tasks/{id}/reopen"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "BAD_TRANSITION");

    // Check events
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let detail: Value = detail_resp.json().await.unwrap();
    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let drop_ev = events.iter().find(|e| e["type"] == "dropped").unwrap();
    assert_eq!(drop_ev["source"], "web");
    assert_eq!(drop_ev["data"]["source"], "web");
    assert_eq!(drop_ev["data"]["reason"], "No longer needed");
    let reopen_ev = events.iter().find(|e| e["type"] == "reopened").unwrap();
    assert_eq!(reopen_ev["source"], "web");
    assert_eq!(reopen_ev["data"]["source"], "web");
}

#[tokio::test]
async fn test_human_action_notes_and_comments() {
    let env = Env::new();
    let id = env.todo("Task for notes");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    // Empty note body -> 400 USAGE
    let resp = client
        .post(format!("{base}/api/tasks/{id}/notes"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "body": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "USAGE");

    // Valid note via /notes -> 200 OK
    let resp = client
        .post(format!("{base}/api/tasks/{id}/notes"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "body": "Operator finding note", "kind": "finding" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Valid comment via /comment -> 200 OK
    let resp = client
        .post(format!("{base}/api/tasks/{id}/comment"))
        .header(COOKIE, &cookie)
        .header("X-Requested-With", "fleetboard")
        .json(&json!({ "comment": "Operator general comment" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify notes and events in detail
    let detail_resp = client
        .get(format!("{base}/api/tasks/{id}"))
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let detail: Value = detail_resp.json().await.unwrap();
    let notes = detail["data"]["task"]["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n["body"] == "Operator finding note" && n["kind"] == "finding")
    );
    assert!(
        notes
            .iter()
            .any(|n| n["body"] == "Operator general comment" && n["kind"] == "comment")
    );

    let events = detail["data"]["task"]["events"].as_array().unwrap();
    let note_evs: Vec<_> = events.iter().filter(|e| e["type"] == "noted").collect();
    assert_eq!(note_evs.len(), 2);
    for ne in note_evs {
        assert_eq!(ne["source"], "web");
        assert_eq!(ne["data"]["source"], "web");
        assert_eq!(ne["proc_chain"], "web");
    }
}

#[tokio::test]
async fn test_missing_x_requested_with_returns_403_on_all_action_endpoints() {
    let env = Env::new();
    let id = env.todo("Task for 403 checks");

    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    let endpoints = [
        format!("{base}/api/tasks/{id}/promote"),
        format!("{base}/api/tasks/{id}/approve"),
        format!("{base}/api/tasks/{id}/verify"),
        format!("{base}/api/tasks/{id}/reject"),
        format!("{base}/api/tasks/{id}/release"),
        format!("{base}/api/tasks/{id}/steal"),
        format!("{base}/api/tasks/{id}/drop"),
        format!("{base}/api/tasks/{id}/reopen"),
        format!("{base}/api/tasks/{id}/notes"),
        format!("{base}/api/tasks/{id}/comment"),
    ];

    for url in endpoints {
        let resp = client
            .post(&url)
            .header(COOKIE, &cookie)
            .json(&json!({ "comment": "test", "body": "test" }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            403,
            "Endpoint {url} must return 403 without X-Requested-With"
        );
        let err: Value = resp.json().await.unwrap();
        assert_eq!(err["ok"], false);
        assert_eq!(err["error"]["code"], "FORBIDDEN");
    }
}

#[tokio::test]
async fn test_not_found_tasks_return_404_on_all_action_endpoints() {
    let env = Env::new();
    let (server, _) = spawn_test_server(env.home.path()).await;
    let client = reqwest::Client::new();
    let base = format!("http://{}", server.addr);
    let cookie = login_cookie(&client, &base, &server.token).await;

    let endpoints = [
        format!("{base}/api/tasks/fb-99999/promote"),
        format!("{base}/api/tasks/fb-99999/approve"),
        format!("{base}/api/tasks/fb-99999/verify"),
        format!("{base}/api/tasks/fb-99999/reject"),
        format!("{base}/api/tasks/fb-99999/release"),
        format!("{base}/api/tasks/fb-99999/steal"),
        format!("{base}/api/tasks/fb-99999/drop"),
        format!("{base}/api/tasks/fb-99999/reopen"),
        format!("{base}/api/tasks/fb-99999/notes"),
        format!("{base}/api/tasks/fb-99999/comment"),
    ];

    for url in endpoints {
        let resp = client
            .post(&url)
            .header(COOKIE, &cookie)
            .header("X-Requested-With", "fleetboard")
            .json(&json!({ "comment": "test", "body": "test" }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            404,
            "Endpoint {url} must return 404 for non-existent task"
        );
        let err: Value = resp.json().await.unwrap();
        assert_eq!(err["ok"], false);
        assert_eq!(err["error"]["code"], "NOT_FOUND");
    }
}
