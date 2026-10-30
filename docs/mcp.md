# Model Context Protocol (MCP) Server (`fb mcp`)

fleetboard provides an official Model Context Protocol (MCP) server that runs over standard input/output (`stdio`).

It is a thin wrapper over the exact same Rust core library that powers the `fb` CLI (governed by the same state machine, authorization rules, and git evidence verification, with zero duplicated business logic).

---

## Capabilities & Tools

### Standard Agent Tools (9)

When an agent connects to `fb mcp`, the following 9 tools are exposed by default:

| Tool | Parameters | Description |
|---|---|---|
| `fb_prime` | *none* | Prints workflow instructions, caller identity, and active claim with latest handoff. |
| `fb_ready` | `project` (str), `all` (bool) | Lists claimable tasks whose dependencies are satisfied. |
| `fb_claim` | `id` (str, opt), `steal` (bool, opt), `for_actor` (str, opt) | Atomically claims a task (or highest priority ready task if `id` omitted). |
| `fb_show` | `id` (str, req), `evidence` (bool, opt) | Displays full task details, criteria, claim history, and git evidence. |
| `fb_note` | `id` (str, req), `text` (str, req), `kind` (str, opt) | Appends a note (`handoff`, `comment`, or `finding`) to a task. |
| `fb_plan` | `id` (str, req), `text` (str, req) | Writes or updates the task plan. |
| `fb_done` | `id` (str, req), `result` (str, opt) | Submits work for review; computes live git evidence (commits since claim). |
| `fb_add` | `title` (req), `goal`, `ac` (list), `kind`, `prio`, `blocked_by`, `ticket`, `project`, `plan_approval` | Creates a new task (enters `draft` for agents; `todo` for human/trusted). |
| `fb_list` | `status` (list), `project`, `agent`, `host`, `needs_me`, `all`, `evidence` | Queries and filters tasks across projects. |

### Human-Only Verbs & Delegation

Human-only commands (`promote`, `approve`, `verify`, `reject`, `release`, `steal`, `drop`, `reopen`) are **not exposed** to standard agents.

When a human grants specific rights to an agent via `fb delegate <agent> --allow <verbs>`, the MCP server dynamically reflects this:
- In `tools/list`, only the delegated verbs are exposed (e.g. `fb_promote`, `fb_verify`).
- Other human-only tools remain strictly absent.
- Actions performed by the delegated agent carry `on_behalf_of: "human"` in the audit log.

---

## Identity Resolution (D3)

The MCP server resolves identity identically to the `fb` CLI:
- It inspects the process ancestry, environment variables (e.g., `CLAUDE_PROJECT_DIR`, `AI_AGENT_DIR`, `PASEO_AGENT_ID`), hostname, and current working directory.
- Each tool call automatically touches the caller's active claim, keeping last-activity timestamps and git branches up to date.

---

## Rust MCP SDK Choice

fleetboard uses [`rmcp`](https://crates.io/crates/rmcp) (v3.8.0), the official Rust SDK developed and maintained by the Model Context Protocol organization (`modelcontextprotocol/rust-sdk`).

**Why `rmcp`?**
1. **Official specification alignment**: Developed directly under the `modelcontextprotocol` GitHub org, tracking the latest MCP specifications and schema updates (including protocol versions `2024-11-05` and `2026-07-28`).
2. **Active maintenance**: Frequently released on crates.io, with active commits, issue triage, and security maintenance.
3. **Robust stdio transport**: Clean Tokio-based asynchronous I/O with framed JSON-RPC stream encoding/decoding.
4. **Lightweight integration**: Feature flags allow enabling only `server` and `transport-io`, avoiding unnecessary dependencies while keeping build times minimal.

---

## Client Configuration Snippets

### Claude Code

Add fleetboard to `~/.claude.json` (or `.claude.json` in your project root):

```json
{
  "mcpServers": {
    "fleetboard": {
      "command": "fb",
      "args": ["mcp"]
    }
  }
}
```

Or add it using the Claude Code CLI:
```bash
claude mcp add fleetboard -- fb mcp
```

### OpenAI Codex

Add to your Codex configuration file (e.g., `~/.codex/config.toml`):

```toml
[mcp_servers.fleetboard]
command = "fb"
args = ["mcp"]
```

Or in JSON format (`~/.codex/config.json`):

```json
{
  "mcpServers": {
    "fleetboard": {
      "command": "fb",
      "args": ["mcp"]
    }
  }
}
```

### Google Antigravity

Add fleetboard to Antigravity MCP configuration (`~/.gemini/antigravity-cli/mcp_config.json` or your workspace settings):

```json
{
  "mcpServers": {
    "fleetboard": {
      "command": "fb",
      "args": ["mcp"]
    }
  }
}
```

---

## Environment Variables

- `FB_HOME`: Path to the fleetboard data directory (default: `~/.fleetboard`). If configuring agents across distinct projects or testing in isolation, set `FB_HOME` in the `env` block of the server configuration:
  ```json
  {
    "mcpServers": {
      "fleetboard": {
        "command": "fb",
        "args": ["mcp"],
        "env": {
          "FB_HOME": "/custom/path/.fleetboard"
        }
      }
    }
  }
  ```
