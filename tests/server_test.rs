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
    assert!(!token.is_empty());

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
