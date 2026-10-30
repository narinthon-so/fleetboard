# fleetboard

A task board for one person running many coding agents across many repos.

Agents claim, hand off, and finish tasks through the `fb` CLI. The board tracks which agent works where (host, checkout directory, git branch, and agent identity), and "done" has to agree with git before work is accepted.

## Highlights

- **Central multi-repo board**: A single database (`~/.fleetboard/board.db`) coordinates tasks across every checkout and repository.
- **Instance-bound claims**: Tasks bind to `(host, cwd, branch, agent)`, avoiding collisions when multiple agents work across different branches or workspaces.
- **Git evidence review gate**: When an agent finishes a `code` task with `fb done`, the task moves to `review` instead of `done`. Live git evidence (commits since claim, clean worktree) is verified before the human marks it `done`.
- **Agent agnostic**: Works with Claude Code, Codex, Antigravity, or any agent with shell access. Agents can use `--json` for machine-readable output.

## Installation

```bash
cargo install --git https://github.com/narinthon-so/fleetboard
```

Ensure `~/.cargo/bin` is in your `PATH`.

## Human Quickstart

As the operator, use `--as-human` (or run commands when no agent environment markers are detected):

```bash
# Create a task (enters 'todo' directly when run by human)
fb add "Implement rate limiting" --goal "Limit API requests per IP" --ac "Returns 429 when over limit" --as-human

# Check tasks that need your attention (drafts, reviews, stale claims)
fb list --needs-me

# Inspect task details and live git evidence
fb show fb-1 --evidence

# Verify and accept completed work (review -> done)
fb verify fb-1
```

## Agent Workflow

Agents interact with `fb` entirely via the CLI (or with `--json`):

1. **Session startup**: Run `fb prime` to inspect caller identity, active claims, and workflow instructions:
   ```bash
   fb prime
   ```
2. **Find work**: Discover unblocked ready tasks:
   ```bash
   fb ready
   ```
3. **Claim task**: Atomically claim the top ready task (or a specific task ID):
   ```bash
   fb claim fb-1
   ```
4. **Plan & Notes**: Document findings or leave handoff notes before stopping:
   ```bash
   fb note fb-1 "Added middleware; tests pass on branch task/fb-1"
   ```
5. **Finish**: Submit work for human review:
   ```bash
   fb done fb-1
   ```
   For `code` tasks, git evidence (commits on the current branch since claim) is recorded and the task moves to `review`. For `note` tasks, provide `--result "..."`.

## Human-Only Commands

The following commands require human authorization (`--as-human`):

| Command | Description |
|---|---|
| `fb promote <id>` | Promote an agent-created `draft` to `todo` |
| `fb approve <id>` | Approve a task plan held for approval |
| `fb verify <id> [--force]` | Approve review and mark task `done` (`--force` allows zero commits) |
| `fb reject <id> <comment>` | Return a task from `review` back to `in_progress` |
| `fb release <id>` | Release an active claim back to `todo` |
| `fb claim <id> --steal [--for <actor>]` | Take over or reassign an active claim |
| `fb drop <id>` | Move a task to `dropped` |
| `fb reopen <id>` | Reopen a `done` or `dropped` task back to `todo` |
| `fb trust <actor>` / `fb untrust <actor>` | Grant or revoke the trusted tracker role |
| `fb project alias <key> <alias>` | Set a friendly display alias for a project |

## Known Limitations

- **Cooperative boundary**: The human/agent boundary is a cooperative guard to keep well-behaved agents in their lanes, not a security sandbox against adversarial agents.
- **Direct database access**: An agent with arbitrary shell access could inspect or modify `$FB_HOME/board.db` directly using `sqlite3`.
- **Process isolation**: An agent stripping its environment variables and running detached outside its process hierarchy (e.g., via `setsid`) can mask its agent identity.
- **Local storage**: M0 uses a local SQLite database (`~/.fleetboard/board.db` with WAL mode). Multi-host daemon sync arrives in later milestones.
