# fleetboard — design

Status: draft (2026-10-30). Covers M0–M2 in detail, with M3 in outline.
Decisions D1–D11 and the prior-art review are in [IDEA.md](IDEA.md); this file
turns them into a schema, a command reference and rules an implementation can
be tested against.

## One-liner

A task board for one person running many coding agents across many repos.
Agents claim, hand off and finish tasks through the `fb` CLI. The board knows
which agent works where, and "done" has to agree with git.

## Goals / non-goals

| Goals (M0–M3) | Non-goals |
|---|---|
| One central board across every repo and checkout | Starting, stopping or prompting agents (the orchestrator does that) |
| Agents work with nothing but a shell (`fb --json`) | Multi-user teams, accounts, permissions beyond human vs agent |
| Task ↔ agent instance binding (host, cwd, branch, agent) | Two-way sync with Jira / Linear / GitHub issues |
| Review gate: git evidence before Done | Code review UI, diff commenting |
| Orchestrator optional; Paseo as the first adapter | Hosted / cloud service |

## Concepts

| Term | Meaning |
|---|---|
| **Task** | One unit of agent-sized work. Has a `kind`: `code` (default) or `note`. |
| **Ticket** | Optional external parent (Jira key, GitHub issue …): system, key, title, URL. Link only (D1). |
| **Project** | One repo, keyed by its normalised git remote (D11). |
| **Actor** | Who made a call: the human, or one agent identity (D3). |
| **Claim** | An actor's exclusive hold on a task, bound to an *instance*. |
| **Instance** | Where the claim runs: host + cwd + branch (+ workspace id when an adapter provides it). |
| **Evidence** | What git shows for a claim: commits since the claim, worktree state, PR / CI (D4). |
| **Needs me** | Derived view of everything waiting on the human (D6). |

## Data model (SQLite, WAL)

Database: `$FB_HOME/board.db` (default `FB_HOME=~/.fleetboard`). One file for
all projects. Schema version in `PRAGMA user_version`; a binary refuses to
open a database newer than it knows (exit 5).

```sql
CREATE TABLE projects (
  id          INTEGER PRIMARY KEY,
  key         TEXT NOT NULL UNIQUE,      -- github.com/org/repo, or dir:<name> without a remote
  alias       TEXT,                      -- display name; defaults to repo name
  created_at  TEXT NOT NULL
);

CREATE TABLE tickets (
  id          INTEGER PRIMARY KEY,
  system      TEXT NOT NULL,             -- jira | linear | github | other
  key         TEXT NOT NULL,             -- e.g. ABC-36
  title       TEXT,
  url         TEXT,
  created_at  TEXT NOT NULL,
  UNIQUE (system, key)
);

CREATE TABLE actors (
  id              INTEGER PRIMARY KEY,
  kind            TEXT NOT NULL CHECK (kind IN ('human','agent')),
  provider        TEXT,                  -- claude | codex | antigravity | unknown
  external_id     TEXT NOT NULL,         -- never NULL (SQLite treats NULLs as distinct in UNIQUE):
                                         -- PASEO_AGENT_ID | provider session id |
                                         -- proc:<host>:<provider>:<ppid> | anon:<host>:<cwd> | human
  label           TEXT,                  -- human-friendly, e.g. "codex 403254b"
  trusted         INTEGER NOT NULL DEFAULT 0,   -- tracker role (D2), set by the human only
  first_seen      TEXT NOT NULL,
  last_seen       TEXT NOT NULL,
  UNIQUE (kind, provider, external_id)
);

CREATE TABLE tasks (
  id                    INTEGER PRIMARY KEY,   -- shown as fb-<id>
  project_id            INTEGER REFERENCES projects(id),
  ticket_id             INTEGER REFERENCES tickets(id),
  title                 TEXT NOT NULL,
  goal                  TEXT,                  -- markdown
  acceptance            TEXT,                  -- markdown checklist
  plan                  TEXT,                  -- written by the agent (fb plan)
  result                TEXT,                  -- closing summary; required for kind=note
  kind                  TEXT NOT NULL DEFAULT 'code' CHECK (kind IN ('code','note')),
  status                TEXT NOT NULL CHECK (status IN
                          ('draft','todo','in_progress','review','done','dropped')),
  priority              INTEGER NOT NULL DEFAULT 2 CHECK (priority BETWEEN 0 AND 3),  -- 0 = highest
  needs_plan_approval   INTEGER NOT NULL DEFAULT 0,
  plan_approved_at      TEXT,
  created_by            INTEGER NOT NULL REFERENCES actors(id),
  created_at            TEXT NOT NULL,
  updated_at            TEXT NOT NULL
);

CREATE TABLE deps (                       -- blocker must be done before task is ready
  task_id     INTEGER NOT NULL REFERENCES tasks(id),
  blocker_id  INTEGER NOT NULL REFERENCES tasks(id),
  PRIMARY KEY (task_id, blocker_id),
  CHECK (task_id <> blocker_id)
);

CREATE TABLE claims (
  id              INTEGER PRIMARY KEY,
  task_id         INTEGER NOT NULL REFERENCES tasks(id),
  actor_id        INTEGER NOT NULL REFERENCES actors(id),
  host            TEXT NOT NULL,
  cwd             TEXT NOT NULL,         -- latest observed, like branch
  branch          TEXT,                  -- latest branch observed for the claimant (refreshed on every call)
  base_commit     TEXT,                  -- HEAD at claim time; evidence starts here
  workspace_id    TEXT,                  -- from a runtime adapter, if any
  claimed_at      TEXT NOT NULL,
  last_activity   TEXT NOT NULL,         -- any fb call by this actor on this task
  released_at     TEXT,
  release_reason  TEXT                   -- done | released | stolen | dropped
);
CREATE UNIQUE INDEX one_active_claim ON claims(task_id) WHERE released_at IS NULL;

CREATE TABLE notes (
  id          INTEGER PRIMARY KEY,
  task_id     INTEGER NOT NULL REFERENCES tasks(id),
  actor_id    INTEGER NOT NULL REFERENCES actors(id),
  kind        TEXT NOT NULL CHECK (kind IN ('handoff','comment','finding')),
  body        TEXT NOT NULL,
  created_at  TEXT NOT NULL
);

CREATE TABLE evidence_snapshots (         -- remote clients only (D7); local evidence is live
  claim_id     INTEGER PRIMARY KEY REFERENCES claims(id),
  data         TEXT NOT NULL,             -- JSON, same shape as live evidence
  computed_at  TEXT NOT NULL
);

CREATE TABLE events (                     -- append-only audit log
  id          INTEGER PRIMARY KEY,
  task_id     INTEGER REFERENCES tasks(id),
  actor_id    INTEGER NOT NULL REFERENCES actors(id),
  type        TEXT NOT NULL,              -- created, promoted, claimed, planned, approved, ...
  data        TEXT,                       -- JSON
  pid         INTEGER NOT NULL,
  ppid        INTEGER NOT NULL,
  proc_chain  TEXT,                       -- parent process names, for the D3 audit
  created_at  TEXT NOT NULL
);
```

Times are UTC RFC 3339 strings. IDs are sequential integers shown as `fb-<n>`.
A central store has no merge problem, so the hash IDs Beads needs for git
merges are not required.

**Concurrent writers.** Every connection sets `PRAGMA journal_mode=WAL` and
`PRAGMA busy_timeout=5000`. Every write that reads state first (claim, status
transitions, dependency edits) runs in `BEGIN IMMEDIATE`, so two agents never
both read "ready" and then race on the upgrade to a write lock. `DB_LOCKED`
(exit 5) is reported only after the busy timeout runs out.

**Seed rows.** The migration that creates `actors` inserts the single human
actor (`kind='human'`, `provider=NULL`, `external_id='human'`).

### Derived states (D6)

Computed in queries, never stored:

| State | Rule |
|---|---|
| Ready | `status='todo'` and every blocker is `done` |
| Blocked | `status='todo'` and at least one blocker is not `done` |
| Orphaned | `status='todo'` and at least one blocker is `dropped`: still Blocked, and listed in Needs me so the human removes the dependency or drops the task |
| Plan pending | `status='in_progress'`, `needs_plan_approval=1`, `plan IS NOT NULL`, `plan_approved_at IS NULL` |
| Stale | `status='in_progress'` and the active claim is stale (see Claims) |
| No evidence | `status='review'`, `kind='code'`, evidence has zero commits |
| **Needs me** | draft ∪ plan pending ∪ stale ∪ orphaned ∪ review ∪ no evidence ∪ adapter pending permissions |

### Status transitions

```
draft ──promote(H)──▶ todo ──claim──▶ in_progress ──done──▶ review ──verify(H)──▶ done
  │                    ▲                  │  ▲                │
  └─drop(H)─▶ dropped  └──release(H)──────┘  └──reject(H)─────┘
```

`(H)` = human only. `reject` sends a task back to `in_progress` with a
required comment; the claim stays with the same agent. Any non-terminal task
can be dropped by the human. `done` and `dropped` are terminal; `reopen(H)`
moves a task back to `todo`.

For `kind=note`, `fb done --result "…"` also lands in `review`; `verify` needs
no evidence.

## Identity (D3)

Resolution, first match wins:

1. `PASEO_AGENT_ID` → `agent`, `external_id` = the Paseo id. Provider from `AI_AGENT` / known provider variables, else from process ancestry (rule 3), else `unknown`.
2. Provider variables → `agent`:
   - Claude Code: `CLAUDECODE=1`, `external_id` = `CLAUDE_CODE_SESSION_ID`.
   - Codex, Antigravity: **to confirm** (open question in IDEA.md).
3. Process ancestry contains a known agent binary (`claude`, `codex`, `agy`, …) → `agent`, provider from the binary, `external_id = proc:<host>:<provider>:<pid of that agent process>`.
4. Nothing found → agent actor `provider='unknown'`, `external_id = anon:<host>:<cwd>`. **Not human.**

Rules 1 and 3 together cover a Codex or agy agent started by Paseo, so the
open question about their own variables (IDEA.md) does not block the M0
done-criterion; it only affects agents started outside Paseo.

The human actor exists only through:

- `fb … --as-human`: refused (exit 4) if step 1, 2 or 3 matches.
- The web UI (M1), which binds to loopback only.

Every human action logs `pid`, `ppid` and the process chain in `events`, so an
agent that strips its environment to pass as the human is usually visible
afterwards.

### Delegation

A human can delegate specific human-only rights to an agent actor with `fb delegate <actor> --allow <list>`. The allowed actions are any subset of `promote`, `approve`, `verify`, `reject`, `release`, `steal`, `drop`, `reopen`.

A delegated agent can run those specific commands directly without `--as-human`; standard store and transition rules apply as they do for the human. Every action executed under delegation logs an event in the audit log with `actor_id` = delegate's actor id and `data.on_behalf_of = 'human'`. `fb show` and the web UI History view display delegated actions as `<actor> on behalf of human` (e.g., `claude 60a1168 on behalf of human`).

Delegation is revoked via `fb undelegate <actor> [--allow <list>]`. When `--allow` is omitted, all delegated rights for that actor are revoked immediately. `delegate` and `undelegate` themselves are strictly human-only and cannot be delegated. Delegation grants and revocations are logged as audit events (`delegated` / `undelegated`).

**Known limits** (one person's machine; the guard stops eager agents, not attackers):

- An agent can strip its environment *and* run outside its own process tree (e.g. through `setsid` or a scheduler). The process chain then looks human.
- An agent with shell access can write `board.db` directly with `sqlite3`. That bypasses every rule and the audit log.
- From M1, an agent can `curl` the loopback web UI. M1 therefore requires a session cookie obtained from a one-time login token that `fb serve` prints in the human's terminal, plus an `X-Requested-With` header on writes (the agent-kanban pattern). The token is never written where agents read.

## Projects and instances (D11)

On `add` and `claim`, `fb` reads git from the current directory:

- `project.key` = `git remote get-url origin`, normalised: strip scheme, user,
  `.git`, convert `host:path` to `host/path`, lowercase the host.
  `git@github.com:Org/Repo.git` → `github.com/Org/Repo`.
- No remote → `dir:<top-level folder name>`. Not in git → no project.
- Instance = `host` (from `gethostname`) + `git rev-parse --show-toplevel` + `git branch --show-current`.

`--project <key|alias>` overrides detection when adding a task from elsewhere.

## Claims (D5)

- `fb claim <id>` succeeds only if the task is Ready (or the caller is human / trusted with `--steal`). The unique index makes two simultaneous claims race-safe: the loser gets exit 3 `CLAIM_HELD`.
- `fb claim` without an id claims the highest-priority Ready task in the current project (oldest first). This is the `bd ready --claim` pattern.
- Claim records host, cwd, branch and `base_commit = git rev-parse HEAD`.
- Every `fb` call by the claimant updates `last_activity` for its active claims on the current host. Location (`cwd` and `branch`) refreshes only for tasks in `in_progress`; it freezes in `review` so evidence stays bound to the submitted work. If the claimant has a single `in_progress` claim on this host, it follows the current location (an agent usually claims on `main`, then creates its feature branch). With several `in_progress` claims on this host, only claims matching both the current `cwd` and `branch` refresh. `base_commit` never changes.
- New commits on the claim's branch also count as activity for staleness: agents rarely call `fb` mid-task, and a busy agent committing every 30 minutes must not look stale.
- **Stale** when either:
  - a runtime adapter reports the agent gone (M2), or
  - with no adapter, `now − max(last_activity, last commit time on the branch) > stale_after` (config, default `2h`).
- Stale claims are never released automatically. The human runs `fb release <id>` (task → `todo`) or `fb claim <id> --steal --for <actor>`.

## Evidence (D4)

Computed live for local claims whenever a task is shown or listed with `--evidence`:

| Field | How |
|---|---|
| `commits` | `git rev-list --count <base_commit>..<branch>` in the claim cwd, where `<branch>` is the latest observed branch (see Claims) |
| `rebased` | `true` if `base_commit` is no longer an ancestor of the branch; commits then fall back to `git log --since=<claimed_at> <branch>` |
| `diffstat` | `git diff --shortstat <base_commit>...<branch>` |
| `dirty` | `git status --porcelain` is non-empty |
| `pr` | `gh pr view <branch> --json number,state,url,statusCheckRollup` if `gh` is installed; cached 60 s; `null` otherwise |
| `reachable` | `false` if the cwd no longer exists or the claim is on another host; then the last snapshot is shown with its age |

`fb done` on a `code` task with zero commits is still allowed: the task goes to
Review with the "claimed, no commits" flag rather than being refused, so the
agent can explain in its handoff note. `fb verify` on such a task requires `--force`.

## Command reference (M0)

Global flags: `--json` (machine output), `--as-human`, `--home <dir>`.
Every command with `--json` prints one JSON value on stdout and nothing else.

### Agent and human

| Command | Does |
|---|---|
| `fb add <title> [--goal …] [--ac …]… [--kind code\|note] [--prio 0-3] [--blocked-by <id>]… [--ticket <system:key> [--ticket-title …] [--ticket-url …]] [--project …] [--plan-approval]` | Create a task. Agent → `draft`; human or trusted → `todo`. A new ticket key creates the ticket row; title and URL fill or update it. |
| `fb ready [--project …\|--all]` | List Ready tasks. Default scope: the current project; outside git, all projects. |
| `fb claim [<id>]` | Claim a task (see Claims). |
| `fb plan <id> <text\|->` | Write or replace the plan. |
| `fb note <id> <text\|-> [--kind handoff\|comment\|finding]` | Add a note. Default kind `handoff`. |
| `fb done <id> [--result …]` | Move to Review and release the claim on a later `verify`. `--result` required for `kind=note`. |
| `fb show <id> [--evidence]` | Full task: fields, deps, claim history, notes, evidence. |
| `fb list [--status …] [--project …] [--agent …] [--host …] [--needs-me] [--all]` | Filtered list. |
| `fb prime` | Workflow guide for agents plus the caller's active claim and its latest handoff note. |
| `fb whoami` | Print caller identity, host, project, instance, and delegated rights. |
| `fb hook <event>` | Record agent lifecycle hook event (`session-start`, `stop`, `tool-use`, `prompt`) from stdin JSON. On `session-start`, prints `fb prime` context (`hookSpecificOutput` for Claude Code, plain text for Codex). Always exits 0. |
| `fb dep add <task> <blocker>` / `fb dep rm …` | Edit dependencies; refuses cycles. A human, the task's claimant, or the task's creator while it is a `draft` may edit. |

### Human only (`--as-human`)

| Command | Does |
|---|---|
| `fb promote <id>` | `draft` → `todo`. |
| `fb approve <id>` | Approve the plan. |
| `fb verify <id> [--force]` | `review` → `done`; releases the claim. |
| `fb reject <id> <comment>` | `review` → `in_progress`. |
| `fb release <id>` | Release the active claim; task → `todo`. |
| `fb claim <id> --steal [--for <actor>]` | Take over or reassign a claim. |
| `fb drop <id> [--reason …]` / `fb reopen <id>` | End or reopen a task. |
| `fb trust <actor>` / `fb untrust <actor>` | Grant or remove the tracker role. |
| `fb delegate <actor> --allow <list>` | Grant rights to act on human behalf (promote, approve, verify, reject, release, steal, drop, reopen). |
| `fb undelegate <actor> [--allow …]` | Revoke rights to act on human behalf (all if --allow is omitted). |
| `fb project alias <key> <alias>` | Rename a project for display. |

### JSON envelope

```json
{ "v": 1, "ok": true,  "data": { … } }
{ "v": 1, "ok": false, "error": { "code": "CLAIM_HELD", "message": "fb-12 is claimed by codex 403254b" } }
```

| Exit | Meaning | Error codes |
|---|---|---|
| 0 | ok | |
| 1 | usage error | `USAGE` |
| 2 | not found | `NOT_FOUND` |
| 3 | conflict / bad transition | `CLAIM_HELD`, `NOT_READY`, `BAD_TRANSITION`, `DEP_CYCLE` |
| 4 | forbidden | `HUMAN_ONLY`, `NOT_CLAIMANT` |
| 5 | store problem | `SCHEMA_AHEAD`, `DB_LOCKED` |

### `fb prime` output (text)

```
fleetboard — you are codex 403254b on host RPI, project github.com/org/repo
Your claim: fb-12 "Rate-limit public endpoints" (in_progress, plan approved)
Last handoff (claude 6df8aed, 2h ago):
  …
Workflow: fb show fb-12 · fb note fb-12 "…" (handoff before you stop) · fb done fb-12
No claim? fb ready, then fb claim <id>. New work you find: fb add "…" (lands as a draft).
```

## Configuration

`$FB_HOME/config.toml`, all optional:

```toml
stale_after = "2h"
adapter     = "none"        # none | hooks | paseo  (M2+)
gh          = true          # use gh for PR / CI evidence when installed
```

## Architecture

One Rust crate, library + binary:

```
src/
  main.rs        clap CLI, output formatting, exit codes
  cli.rs         command parsing and execution
  config.rs      config.toml loading, durations, adapter selection
  store/         schema, migrations, queries (rusqlite); Backend trait (D7)
  model.rs       Task, Claim, Actor, status machine, derived states
  identity.rs    env + process-ancestry resolution (D3)
  gitinfo.rs     project key, instance, evidence (shells out to git / gh)
  prime.rs       fb prime context generation
  server/        M1 axum loopback server, auth tokens, read API, SSE
  adapters/      RuntimeAdapter trait: instances(), agent_status(id), pending_permissions(id)
    none.rs      default self-report adapter
    paseo.rs     Paseo adapter (workspace ls, ls, inspect; longest prefix join; 5 s cache)
    hooks.rs     hooks adapter (reads last hook event to report running, idle, gone)
```

`Backend` trait from day one with one `SqliteBackend`; `HttpBackend` arrives
with remote mode. The status machine and derived-state rules live in
`model.rs` with no I/O, so they are unit-testable alone.

The `RuntimeAdapter` trait provides live integration with agent orchestrators:
- `instances()` returns active instances joined on cwd (longest prefix).
- `agent_status(external_id)` returns `running`, `idle`, `permission`, or `gone`.
- `pending_permissions(external_id)` returns count of pending permissions.
- In `PaseoAdapter`, if `paseo` binary is missing or errors, fleetboard degrades to `none` with a warning on stderr and never fails commands.

## Testing

- **Model:** status transitions, derived states and the human-only rules as pure unit tests (table-driven).
- **Store:** in-memory SQLite; the one-active-claim index under concurrent claims (N threads, exactly one winner).
- **CLI:** `assert_cmd` against a temp `FB_HOME` and temp git repos (`git init`, commits, a fake remote URL); covers project key normalisation, evidence counts, rebase fallback, dirty worktree.
- **Identity:** env injection for each provider rule; `--as-human` refused under each agent marker.
- **JSON contract:** snapshot tests (`insta`) of `--json` output so the agent-facing format cannot change by accident.
- `gh` is stubbed with a fake binary on `PATH` in tests.
- **Adapters (M2):** tested with a fake `paseo` binary on `PATH` covering fixtures for `running`, `idle`, `permission`, `archived`, `missing`, `binary absent`, and `garbage output`. Verifies that archiving an agent marks its claim stale within one poll, and pending permissions join the Needs-me view.
- **Adapters (M3):** tested with sample hook payloads (Claude Code, Codex), garbage stdin, missing board database, timing benchmark (< 100 ms), and status transitions (`running` after tool-use/prompt, `idle` after stop, `gone` after `stale_after`).

## Milestones (D10)

| | Scope | Done when |
|---|---|---|
| **M0** | Schema, identity, evidence, every command above, `none` adapter | One real ticket runs through it: a Claude and a Codex agent each claim, hand off, reach Review; the human verifies both |
| **M1** | `fb serve`, React + shadcn/ui board, filters (project / instance / agent / host), Needs-me lane, SSE via `data_version` polling | The ticket above is followed in the browser without the CLI |
| **M2** | Paseo adapter: status dots, pending permissions, runtime-aware stale | Archiving a Paseo agent marks its claim stale within one poll |
| **M3** | `hooks` adapter, MCP wrapper, onesrc install entries, README + demo GIF, repo public | A new user without Paseo goes from install to a claimed task in under 5 minutes |
