# fleetboard — Hooks Adapter (`fb hook`)

Status: ready (2026-10-30). Implemented by `fb hook <event>` and `adapter = "hooks"`.

The **hooks adapter** provides runtime state for coding agents (Claude Code, Codex, and others) without requiring a heavy orchestrator daemon like Paseo. By wiring agent CLI lifecycle hooks to `fb hook`, fleetboard receives near-live signals about whether an agent is active, idle, or gone, refreshes claim activity timestamps to prevent busy agents from looking stale, and injects `fb prime` task context directly at session start.

---

## Key Guarantees

1. **Never Fails the Agent:** `fb hook` **always exits with code `0`**, even on malformed stdin JSON, unknown hook events, or missing board databases. Any operational warnings are emitted to `stderr` only, ensuring that hook execution never interrupts agent workflows or aborts turns.
2. **Sub-100ms Execution:** Fast, synchronous SQLite writes and indexed lookups complete in under 10ms, adding zero perceivable latency to agent tool invocations or turns.
3. **Context Injection:** On `session-start`, `fb hook` prints `fb prime` context:
   - **Claude Code:** Formatted as structured JSON containing `hookSpecificOutput` with `additionalContext`.
   - **Codex & Other Agents:** Emitted as plain text so any standard hook runner captures it directly into the session prompt.

---

## Fleetboard Configuration

To enable the hooks adapter in fleetboard, set `adapter = "hooks"` in `$FB_HOME/config.toml` (default `~/.fleetboard/config.toml`):

```toml
adapter = "hooks"
stale_after = "2h"     # agent marked gone if no hook events occur within this window
gh = true              # PR / CI evidence
```

---

## Setup for Claude Code

In Claude Code, hooks are configured in `~/.claude/settings.json` (global) or `.claude/settings.json` (project-level).

Add the following configuration to your `settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook session-start"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook prompt"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook tool-use"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook stop"
          }
        ]
      }
    ]
  }
}
```

### What Happens
- **Session start:** Claude Code executes `fb hook session-start` and passes session JSON on `stdin`. Fleetboard returns `hookSpecificOutput` containing `fb prime` text (current claim, branch, previous handoff note). Claude Code automatically injects this context into its system memory.
- **Turn activity:** `fb hook tool-use` and `fb hook prompt` update `claims.last_activity` and mark the agent's runtime status as `running`.
- **Turn end:** `fb hook stop` marks the agent's runtime status as `idle`.

---

## Setup for Codex

In Codex, hooks are configured in `~/.codex/hooks.json` (global) or `.codex/hooks.json` (project-level).

Add the following configuration to your `hooks.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook session-start"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook prompt"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook tool-use"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fb hook stop"
          }
        ]
      }
    ]
  }
}
```

---

## Event Lifecycle & Status Transitions

| Event | Invocation | Runtime Status | Claim Activity | Standard Output |
|---|---|---|---|---|
| `session-start` | Agent session launches or resumes | `running` | Refreshed | `fb prime` context (`hookSpecificOutput` for Claude Code; plain text for Codex) |
| `prompt` | User submits a prompt | `running` | Refreshed | (empty) |
| `tool-use` | Agent invokes a tool | `running` | Refreshed | (empty) |
| `stop` | Agent completes its turn | `idle` | Refreshed | (empty) |
| *(timeout)* | Elapsed time > `stale_after` | `gone` | — | Stale claim surfaces in `fb list --needs-me` |

### Staleness & Operator Queue (`Needs me`)

When an agent's last hook event is older than `stale_after` (default 2 hours), `adapter=hooks` reports runtime status `gone`.
Fleetboard derived-state logic automatically flags any active `in_progress` claim held by that agent as **Stale**, surfacing the task in `fb list --needs-me` and the pinned Needs-me board lane so the human operator can release or reassign it.

---

## Manual Verification

You can verify hook handling directly from your shell:

```bash
# Test session-start plain text output (Codex mode)
echo '{"session_id": "test-session", "cwd": "'"$PWD"'"}' | fb hook session-start

# Test session-start JSON output (Claude Code mode)
echo '{"session_id": "test-session", "source": "startup", "cwd": "'"$PWD"'"}' | fb hook session-start

# Test tool-use activity bump
echo '{"session_id": "test-session", "tool_name": "Bash"}' | fb hook tool-use

# Test stop event
echo '{"session_id": "test-session"}' | fb hook stop

# Test resilience against malformed stdin
echo 'not json {{' | fb hook tool-use
echo $?  # Always outputs 0
```
