//! Integration tests for the stdio MCP server (`fb mcp`).

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use common::{Env, Who};
use serde_json::{Value, json};

const AGENT_A: Who = Who::Agent("agent-aaaaaaa");
const AGENT_DELEGATED: Who = Who::Agent("agent-delegated");

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    req_id: i64,
}

impl McpClient {
    fn spawn(env: &Env, who: Who) -> Self {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("fb"));
        cmd.current_dir(&env.work_path)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", env.home.path())
            .env("FB_HOME", env.home.path())
            .env("FB_TEST_CHAIN", "")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .args(["mcp"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        if let Some(now) = env.now.borrow().as_ref() {
            cmd.env("FB_TEST_NOW", now);
        }

        match who {
            Who::Human => {
                cmd.arg("--as-human");
            }
            Who::Agent(id) => {
                cmd.env("PASEO_AGENT_ID", id).env("AI_AGENT", "codex");
            }
        }

        let mut child = cmd.spawn().expect("failed to spawn fb mcp");
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");
        let reader = BufReader::new(stdout);

        Self {
            child,
            stdin,
            reader,
            req_id: 0,
        }
    }

    fn send_request(&mut self, method: &str, params: Value) -> Value {
        self.req_id += 1;
        let id = self.req_id;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&req).unwrap();
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();

        let mut resp_line = String::new();
        self.reader.read_line(&mut resp_line).unwrap();
        assert!(
            !resp_line.is_empty(),
            "expected response for {method}, got EOF from fb mcp"
        );
        let resp: Value = serde_json::from_str(&resp_line)
            .unwrap_or_else(|e| panic!("invalid json ({e}): {resp_line}"));
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], id);
        resp
    }

    fn send_notification(&mut self, method: &str, params: Option<Value>) {
        let mut notif = json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if let Some(p) = params {
            notif["params"] = p;
        }
        let line = serde_json::to_string(&notif).unwrap();
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn initialize(&mut self) -> Value {
        let resp = self.send_request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-mcp-client", "version": "1.0" }
            }),
        );
        assert!(
            resp.get("result").is_some(),
            "initialize failed: {:?}",
            resp
        );
        self.send_notification("notifications/initialized", None);
        resp["result"].clone()
    }

    fn list_tools(&mut self) -> Vec<Value> {
        let resp = self.send_request("tools/list", json!({}));
        assert!(
            resp.get("result").is_some(),
            "tools/list failed: {:?}",
            resp
        );
        resp["result"]["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    fn call_tool(&mut self, name: &str, args: Value) -> Value {
        self.send_request(
            "tools/call",
            json!({
                "name": name,
                "arguments": args,
            }),
        )
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn test_mcp_initialize_and_tools_list_standard_agent() {
    let env = Env::new();
    let mut client = McpClient::spawn(&env, AGENT_A);

    // 1. Initialize
    let init_res = client.initialize();
    assert_eq!(init_res["serverInfo"]["name"], "fleetboard");
    assert!(init_res["capabilities"]["tools"].is_object());

    // 2. tools/list
    let tools = client.list_tools();
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    // The 9 standard tools MUST be present
    let expected_tools = [
        "fb_prime", "fb_ready", "fb_claim", "fb_show", "fb_note", "fb_plan", "fb_done", "fb_add",
        "fb_list",
    ];
    for expected in expected_tools {
        assert!(
            tool_names.contains(&expected),
            "expected tool '{expected}' not found in: {tool_names:?}"
        );
    }

    // Human-only tools MUST be absent for standard un-delegated agent
    let human_only_tools = [
        "fb_promote",
        "fb_approve",
        "fb_verify",
        "fb_reject",
        "fb_release",
        "fb_steal",
        "fb_drop",
        "fb_reopen",
    ];
    for human_tool in human_only_tools {
        assert!(
            !tool_names.contains(&human_tool),
            "human-only tool '{human_tool}' should NOT be exposed to standard agent: {tool_names:?}"
        );
    }
}

#[test]
fn test_mcp_each_tool_call_over_stdio() {
    let env = Env::new();
    let mut client = McpClient::spawn(&env, AGENT_A);

    // Initialize
    client.initialize();

    // 1. Tool call: fb_add
    let add_resp = client.call_tool(
        "fb_add",
        json!({
            "title": "Implement MCP Server",
            "goal": "Expose fleetboard over stdio MCP",
            "ac": ["supports 9 tools", "stdio transport", "handles delegations"],
            "prio": 1,
            "kind": "code"
        }),
    );
    let add_result = &add_resp["result"];
    assert_eq!(add_result["isError"], false);
    let task_data = &add_result["structuredContent"];
    assert_eq!(task_data["task"]["id"], "fb-1");
    assert_eq!(task_data["task"]["status"], "draft");

    // 2. Tool call: fb_list
    let list_resp = client.call_tool("fb_list", json!({ "status": ["draft"] }));
    let list_result = &list_resp["result"];
    assert_eq!(list_result["isError"], false);
    let tasks = list_result["structuredContent"]["tasks"]
        .as_array()
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "fb-1");

    // Add a ready task as human (fb-2) so the agent can find and claim work
    let fb2_id = env.todo("Human created task");
    assert_eq!(fb2_id, "fb-2");

    // 3. Tool call: fb_ready
    let ready_resp = client.call_tool("fb_ready", json!({}));
    let ready_result = &ready_resp["result"];
    assert_eq!(ready_result["isError"], false);
    let ready_tasks = ready_result["structuredContent"]["tasks"]
        .as_array()
        .unwrap();
    assert!(ready_tasks.iter().any(|t| t["id"] == "fb-2"));

    // 4. Tool call: fb_claim
    let claim_resp = client.call_tool("fb_claim", json!({ "id": "fb-2" }));
    let claim_result = &claim_resp["result"];
    assert_eq!(claim_result["isError"], false);
    assert_eq!(claim_result["structuredContent"]["task"]["id"], "fb-2");
    assert_eq!(
        claim_result["structuredContent"]["task"]["status"],
        "in_progress"
    );

    // 5. Tool call: fb_prime
    let prime_resp = client.call_tool("fb_prime", json!({}));
    let prime_result = &prime_resp["result"];
    assert_eq!(prime_result["isError"], false);
    assert_eq!(
        prime_result["structuredContent"]["claims"][0]["task"]["id"],
        "fb-2"
    );
    let prime_text = prime_result["content"][0]["text"].as_str().unwrap();
    assert!(prime_text.contains("Your claim: fb-2"));

    // 6. Tool call: fb_show
    let show_resp = client.call_tool("fb_show", json!({ "id": "fb-2", "evidence": true }));
    let show_result = &show_resp["result"];
    assert_eq!(show_result["isError"], false);
    assert_eq!(show_result["structuredContent"]["task"]["id"], "fb-2");

    // 7. Tool call: fb_plan
    let plan_resp = client.call_tool(
        "fb_plan",
        json!({
            "id": "fb-2",
            "text": "1. Write tests first\n2. Implement MCP\n3. Run clippy"
        }),
    );
    let plan_result = &plan_resp["result"];
    assert_eq!(plan_result["isError"], false);
    assert_eq!(
        plan_result["structuredContent"]["task"]["plan"],
        "1. Write tests first\n2. Implement MCP\n3. Run clippy"
    );

    // 8. Tool call: fb_note
    let note_resp = client.call_tool(
        "fb_note",
        json!({
            "id": "fb-2",
            "text": "Completed core implementation",
            "kind": "handoff"
        }),
    );
    let note_result = &note_resp["result"];
    assert_eq!(note_result["isError"], false);

    // Make a commit in the git work tree so git evidence is satisfied
    env.write("work.txt", "some changes");
    env.commit("implement fb-2");

    // 9. Tool call: fb_done
    let done_resp = client.call_tool("fb_done", json!({ "id": "fb-2" }));
    let done_result = &done_resp["result"];
    assert_eq!(done_result["isError"], false);
    assert_eq!(done_result["structuredContent"]["task"]["status"], "review");
}

#[test]
fn test_mcp_delegated_agent_sees_only_its_verbs() {
    let env = Env::new();

    // 1. Initial agent task (lands as draft fb-1)
    env.ok(AGENT_DELEGATED, &["add", "Task to promote"]);

    // 2. Delegate "promote" and "verify" to AGENT_DELEGATED
    env.ok(
        Who::Human,
        &["delegate", "agent-delegated", "--allow", "promote,verify"],
    );

    // 3. Start fb mcp as AGENT_DELEGATED
    let mut client = McpClient::spawn(&env, AGENT_DELEGATED);
    client.initialize();

    // 4. tools/list should now include ONLY the 9 standard tools PLUS fb_promote and fb_verify
    let tools = client.list_tools();
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(
        tool_names.contains(&"fb_promote"),
        "fb_promote should be present for delegated agent"
    );
    assert!(
        tool_names.contains(&"fb_verify"),
        "fb_verify should be present for delegated agent"
    );

    // Other delegatable tools MUST still be absent
    let absent_tools = [
        "fb_approve",
        "fb_reject",
        "fb_release",
        "fb_steal",
        "fb_drop",
        "fb_reopen",
    ];
    for t in absent_tools {
        assert!(
            !tool_names.contains(&t),
            "un-delegated tool '{t}' must NOT be in tools/list: {tool_names:?}"
        );
    }

    // 5. Call fb_promote over stdio!
    let promote_resp = client.call_tool("fb_promote", json!({ "id": "fb-1" }));
    let promote_result = &promote_resp["result"];
    assert_eq!(promote_result["isError"], false);
    assert_eq!(
        promote_result["structuredContent"]["task"]["status"],
        "todo"
    );

    // 6. Verify audit event carries on_behalf_of = human
    let db = rusqlite::Connection::open(env.home.path().join("board.db")).unwrap();
    let data_str: String = db
        .query_row(
            "SELECT data FROM events WHERE task_id = 1 AND type = 'promoted'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let event_data: Value = serde_json::from_str(&data_str).unwrap();
    assert_eq!(event_data["on_behalf_of"], "human");
}

#[test]
fn test_mcp_human_only_tools_refused_without_delegation() {
    let env = Env::new();
    env.ok(AGENT_A, &["add", "Draft task"]);

    let mut client = McpClient::spawn(&env, AGENT_A);
    client.initialize();

    // Calling fb_promote without delegation should return an error
    let resp = client.call_tool("fb_promote", json!({ "id": "fb-1" }));
    let result = &resp["result"];
    assert_eq!(
        result["isError"], true,
        "calling un-delegated tool should be an error"
    );
    let err_msg = result["structuredContent"]["error"]["message"]
        .as_str()
        .unwrap();
    assert!(
        err_msg.contains("human-only"),
        "expected human-only error, got: {err_msg}"
    );
}

#[test]
fn test_mcp_unknown_tool_returns_method_not_found() {
    let env = Env::new();
    let mut client = McpClient::spawn(&env, AGENT_A);
    client.initialize();

    let resp = client.call_tool("fb_nonexistent_tool", json!({}));
    assert!(resp.get("error").is_some());
    assert_eq!(resp["error"]["code"], -32601); // METHOD_NOT_FOUND
}
