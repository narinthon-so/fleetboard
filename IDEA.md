# fleetboard

A task board for one person running a fleet of coding agents (Claude Code, Codex, Antigravity, …) across many repos. The board doesn't start agents. It sits next to whatever orchestrator already does (Paseo first) and shows, for each task, which agent is working on it, where it runs, what state it is in, and whether git agrees with what the agent says it did.

**Status:** design settled in a grill session (2026-10-29, decisions D1–D11 below). Nothing built yet; next is M0.
**Goal:** portfolio project and hands-on experience. A public repo under `narinthon-so`, so no employer names or data in the code, docs or examples.

---

## Problem

Running five or more agents at once breaks the usual tools:

1. **Tasks are spread across repos.** Repo-local trackers (Beads, Backlog.md) store one project per store. Nothing gives one board across ~25 repos.
2. **The same repo runs as several instances.** One repo can be open in five agent workspaces at once (different checkouts, branches, agents). A filter by repo name can't tell them apart.
3. **Board and runtime are disconnected.** Passive boards don't know whether the agent is running, idle or waiting for a permission. Boards that dispatch agents replace the orchestrator and bring their own worktree model.
4. **Agents mark their own work "done".** "Done" is whatever the agent reports. In practice the operator checks the report against `git log` / the diff by hand every time.
5. **The human is the bottleneck.** What needs the operator (a decision, a permission, a review) is mixed in with everything else.

## Prior art (checked 2026-10-29)

| Tool | Stars / licence / activity | Model | Worth taking | Gap for this problem |
|---|---|---|---|---|
| [gastownhall/beads](https://github.com/gastownhall/beads) | 28.3k · MIT · v1.3.2, ≥100 commits/30d | CLI, Dolt DB per project; `--stealth` keeps it out of git, `--contributor` routes issues to a separate repo | `bd ready` (dependency-aware "claimable" list), atomic `--claim`, hash IDs (no collisions between agents), `bd prime` context injection, compaction of old closed tasks, `bd setup codex/claude` | One project per store; CLI-first, weak visual board; no runtime state |
| [MrLesk/Backlog.md](https://github.com/MrLesk/Backlog.md) | 7.2k · MIT · v1.55.3, ≥100 commits/30d | Markdown files per project (`--no-git`, custom dir), CLI + MCP + web board | Three review checkpoints (spec → plan → code), acceptance criteria + Definition of Done, plan written into the task before coding, `--json --watch`, archive keeps the record | One project per folder; no runtime state; agent self-report only |
| [Jasper-Tsai/agent-kanban](https://github.com/Jasper-Tsai/agent-kanban) | 1 star · MIT · created 2026-10-14 | Hono + `node:sqlite`, `kb` CLI, MCP (15 tools), WebMCP, SSE | **Closest prior art.** One board for many projects, per-agent identity on every write, handoff notes, SessionStart hook loads a "task packet", Blocked / Needs-owner flags, "Today's focus" | Keys tasks by project, not by runtime instance; no orchestrator link; no git verification |
| [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban) | 28.5k · Apache-2.0 · last release 2026-04, 3 commits/30d | Dispatcher: board + workspace (branch, terminal, dev server) per issue | Inline diff review that feeds comments back to the agent, PR creation | Replaces the orchestrator; activity slowing |
| [leodavinci1/kanbots](https://github.com/leodavinci1/kanbots) | 615 · MIT · stale since 2026-05 | Desktop dispatcher, SQLite, worktree per run | Inbox column, live tool_use stream, "promote commit", Sentry import | Dispatcher; stale |
| [DanWahlin/ai-agent-board](https://github.com/DanWahlin/ai-agent-board) | 67 · MIT · quiet since 2026-09 | Dispatcher with `AgentProvider` / `AgentSession` abstraction | Clean provider abstraction, transition validation, safe worktree cleanup rules | Dispatcher |
| [makeplane/plane](https://github.com/makeplane/plane) | 61.7k · AGPL-3.0 · active | Full Jira/Linear-style tracker, MCP servers | Projects, cycles, a mature UI | Heavy (Postgres, Valkey, RabbitMQ, MinIO); knows nothing about agents |
| Task Master | 28.3k · MIT + Commons Clause · no push since 2026-04 | PRD → JSON task breakdown | Task decomposition | No board; licence restriction |

**Takeaway:** dispatchers (Vibe Kanban, KanBots, AI Agent Board) compete with the orchestrator. Passive boards (Beads, Backlog.md, agent-kanban) know nothing about the runtime. **No tool joins tasks to the live agent runtime and checks the agent's claims against git.** That gap is the project.

## Differentiators (vs agent-kanban, the closest)

1. **Runtime-instance binding.** A task binds to `(host, workspaceId, cwd, branch, agentId)`, not just to a repo. The board filters by project, by workspace instance or by agent.
2. **Orchestrator adapter, never a dispatcher.** It reads state from the orchestrator and launches nothing. The card shows a live dot (running / idle / waiting for permission) and the number of pending permissions.
3. **Verified done.** Each card shows two columns: *agent says* (its handoff note) and *git shows* (commits on the bound branch since the claim, diff stat, PR + CI status). "Done" can require git evidence.
4. **Operator queue.** One "Needs me" lane collects pending permissions, Needs-owner flags, plans waiting for approval (Backlog.md checkpoint #2), and cards that claim done without evidence.
5. **Multi-host.** Several orchestrator daemons (e.g. a Linux box and a laptop) feed one board.
6. **Adapters for the outside world.** An optional external ticket link (Jira, Linear, GitHub issue) on a parent task, using the same adapter pattern as `onesrc`.

## Feasibility: what Paseo exposes (checked with Paseo 0.12.0)

- `paseo workspace ls --json` → `workspaceId` (stable, `wks_…`), `project`, `name`, `cwd`, `isolation`.
- `paseo ls --json` / `paseo inspect <id> --json` → agent `Id`, `Provider`, `Model`, `Status` (running / idle), `Cwd`, `PendingPermissions`, `UpdatedAt`, `ParentAgentId`.
- `paseo status --json` → `serverId`, `hostname`, `listen` (daemon on `127.0.0.1:6767`); `--host` reaches other daemons over TCP/SSH.
- Agent JSON has **no `workspaceId`**, so the adapter joins agent → workspace on `cwd` (longest-prefix match). Inside an agent, Paseo sets `PASEO_AGENT_ID`, so `fb` calls bind to the agent directly (D3). The branch comes from `git -C <cwd> branch --show-current`.
- `paseo hooks <agent> <event>` exists for recording hook activity. It may give a push signal later; the MVP polls the CLI every few seconds.
- Not checked yet: whether the daemon's WebSocket protocol is stable enough to use instead of polling the CLI.

## Orchestrator is optional (decided 2026-10-29)

Paseo is **not** a dependency. The core (store, CLI/MCP, board, git verifier) works with no orchestrator at all. Runtime state comes through a `RuntimeAdapter` interface (list instances, agent status, pending permissions):

| Adapter | Source of live state | Notes |
|---|---|---|
| `none` | Agent self-report: `fb claim` / `fb note` / `fb done` record host + cwd + branch + provider from the agent's environment | Always available; status is "last seen", not live |
| `hooks` | Agent CLI hooks (session start / stop / tool use) call `fb hook …` | Near-live for any user of Claude Code / Codex hooks; no orchestrator needed |
| `paseo` | `paseo workspace ls` / `paseo ls --json` (agent ↔ workspace joined on cwd) | Adds workspace ids, pending permissions, multi-host |

The binding key without an orchestrator is `(host, cwd, branch, agent session)`; `workspaceId` is filled in only when an adapter provides it. Paseo is the first adapter because it is what the author uses, but the README must work for someone who has never heard of Paseo.

## MVP scope (original proposal — superseded by D7–D10)

- **Store:** one central SQLite file (e.g. `~/.fleetboard/board.db`), outside every repo.
- **Agent interface:** a `fb` CLI with `--json` everywhere (`fb ready`, `fb claim`, `fb note`, `fb done`) plus an MCP server exposing the same verbs. Actor identity comes from the environment: provider, plus the Paseo agent id if set.
- **Task fields:** title, goal, acceptance criteria, plan, status, priority, dependencies (`ready` = no open blockers), binding (host/workspace/cwd/branch/agent), handoff notes, external link.
- **Web board:** columns Backlog / Ready / In progress / Needs me / Review / Done. Filters for project, workspace instance, agent and host. Live updates over SSE.
- **Paseo adapter:** polls workspaces and agents and shows status and pending permissions on bound cards.
- **Git verifier:** for bound cards, shows commits since claim, diff stat, and the PR/CI state through `gh`.
- **Out of scope for the MVP:** dispatching agents, multi-user auth (loopback only, like Paseo), external tracker sync, compaction.

## Open questions

- Paseo adapter: poll the CLI (M2 default) or speak the daemon protocol later?
- Codex / agy environment variables for identity (D3) — check inside a running agent.
- Kanban component: Kibo UI vs ReUI — check maintenance at M1.
- Import from / export to Beads or Backlog.md formats so their users can try fleetboard?
- Notifications for Needs-me (push, desktop) — after M2.

## Decisions (grill session 2026-10-29)

- **D1 Task source — parent link + owned children.** An external ticket (Jira / Linear / GitHub issue) is an optional parent: key, title, URL, entered by hand. fleetboard owns the child tasks. No sync in the MVP; sync is a later adapter.
- **D2 Who creates tasks — agents create drafts.** Agent-created tasks land as `draft` in Needs-me; the human promotes or drops them. A tracker session with the trusted role skips the draft step.
- **D3 Identity — environment, no tokens; human actions guarded.** Caller identity is read from the environment: `PASEO_AGENT_ID` (set by Paseo in every agent it starts; gives direct agent binding, no cwd join needed), then provider variables (Claude Code: `CLAUDE_CODE_SESSION_ID`, `AI_AGENT`; Codex / agy: to check), then host + cwd + branch + pid. Human-only actions (promote draft, approve plan, verify done, grant the trusted role) work only from the web UI or `fb --as-human` in a shell with no agent variables; `fb` refuses `--as-human` when it sees them. Known limit: an agent can strip its variables on purpose; the audit log records every human action with its pid and parent process to make that visible.
- **D4 Done — gated by kind.** Each task has `kind`: `code` (default) or `note`. For `code`, an agent's `fb done` moves the card to Review, never to Done; evidence is computed automatically (≥1 commit on the bound branch after claim time, clean worktree, PR + CI state when `gh` is available). Only the human moves Review → Done. No evidence → red "claimed, no commits" badge in Needs-me. For `note`, the task closes on a written result with no git check. Evidence is computed on read (live git query), not stored at claim time, so a force-push or rebase that drops commits shows up.
- **D5 Claims — exclusive, runtime-aware stale, human release.** One active claimant per task; past claimants stay in history and the handoff note carries context forward. A claim is *stale* when the runtime adapter reports the agent gone (archived or missing in Paseo) or, with the `none` adapter, after no activity for a threshold (default 2 h). Stale claims go to Needs-me with Release / Reassign; never auto-released. `fb claim --steal` is refused for agents and allowed for the human or the trusted tracker role. Multiple agents on one piece of work = separate tasks linked by a dependency.
- **D6 Status model — six stored, the rest derived.** Stored: `draft → todo → in_progress → review → done`, plus `dropped`. Derived (never stored): *Ready* = `todo` with all blockers done (`fb ready`); *Blocked* = `todo` with an open blocker; *Stale* = flag on `in_progress` (D5); *Needs me* = view over drafts, plans awaiting approval, stale claims, Review cards, no-evidence cards, and runtime pending permissions. Plan approval is an opt-in per-task flag: while set, the task sits in `in_progress` with sub-state `plan_pending` until the human runs `fb approve`. Board columns: Draft · Ready · Blocked · In progress · Review · Done, with Needs me as a pinned lane (a card can appear in both).
- **D7 Storage — direct SQLite by default, remote later.** One core library, `Backend` trait with two implementations: local SQLite (WAL) by default, HTTP client when `FB_URL` is set (Beads `bd connect` pattern). Agents keep working when the daemon is down; the daemon serves only the web UI, adapters and SSE, and notices CLI writes by polling SQLite `data_version` (~500 ms). MVP is single-host. Git evidence is always computed on the host where the repo lives: locally it is computed on read (D4); a remote client sends an evidence snapshot with `fb done` and refreshes it on later `fb` calls from that host.
- **D8 Agent interface — CLI first, MCP later.** `fb … --json` is the contract; any agent with a shell can use it with no per-agent config. `fb prime` prints the workflow guide plus the caller's current claim and latest handoff note; agents run it at session start (one line in AGENTS.md / CLAUDE.md, or a SessionStart hook). MCP server is phase 2, a thin wrapper over the same core. onesrc installs the instruction line and hook into each agent.
- **D9 Stack — Rust core + embedded React UI.** Rust: one binary for `fb` CLI and `fb serve` daemon; `rusqlite` (WAL), axum, SSE, adapters, git through the `git` CLI (not `git2`, so behaviour matches the terminal). UI: React + Vite + **shadcn/ui** (Tailwind v4), built to `dist/` and embedded with `rust-embed`. Board drag-and-drop from a shadcn-registry kanban component built on dnd-kit (candidates: Kibo UI, ReUI — check maintenance before picking; components are copied in, so no lock-in). Server state via TanStack Query, invalidated by SSE events.
- **D10 Milestones — CLI first.** M0 "agents can use it": schema for D1–D6, `fb add | ready | claim | note | plan | done | show | list --json | prime` plus human `promote | approve | verify | release`, env identity, git evidence on read, `none` adapter; done when one real ticket runs through it (a Claude and a Codex agent each claim, hand off, reach Review; the human verifies both). M1 "I can see it": `fb serve`, web board with project / workspace / agent / host filters, Needs-me lane, SSE. M2 "it knows the runtime": Paseo adapter (status dots, pending permissions, stale claims). M3 "public": `hooks` adapter, MCP wrapper, onesrc install entries, README + demo GIF, public repo under narinthon-so. Repo stays private until M3.
- **D11 Project key — normalised git remote.** `project` = `git remote get-url origin` normalised (`git@github.com:org/repo.git` → `github.com/org/repo`); display name defaults to the repo name, renamable with `fb project alias`; no remote → top-level folder name. The *instance* (host + cwd + branch + agent, D3) separates copies, so filters have two levels: project (all work on one repo) → instance (one checkout one agent runs in). Same key across workspaces, hosts and re-clones.
