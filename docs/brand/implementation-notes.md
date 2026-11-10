# fb-107 / fb-108 implementation tracker

## Resume here

The user asked this session to lead implementation and confirmed: "i trust U so
let do". The agreed scope is approved: Codex applies the branding; Agy performs
QA; this session routes work, checks evidence, and handles routine decisions.

Repo: `/home/nsubuntu22/code/fleetboard`; branch: `task/fb-107`.
Design base commit: `bfefca8`. Design handoff: [rollout.md](rollout.md).

Both tasks are currently Draft. No implementation or QA agent has been launched.
This session is `codex ba16e05` and has no Fleetboard tracker privileges. Promotion
must succeed through the normal CLI before an implementation claim or kickoff.
Never use `--as-human`, alter the database, strip agent identity, or ask another
agent to bypass a refusal.

## Decisions

- D1 — User selected A, Parallel F. Preserve approved SVG geometry and palette.
- D2 — User assigned implementation to Codex and QA to Agy.
- D3 — User asked this session to lead the rollout and approved routine execution
  within the existing task acceptance criteria. No additional scope approval is
  needed for these implementation and QA briefs.
- D4 — QA starts when fb-107 reaches Review, before human verification. A hard
  Done dependency is intentionally absent; see rollout.md.
- D5 — One writer at a time in this checkout. Codex first, Agy afterward. No
  extra worktree is needed. Tests against Fleetboard always use a temporary
  FB_HOME; never mutate or test against the real board.

## Agents

| Role | Agent | State |
| --- | --- | --- |
| Lead | Current Codex session, ba16e05, gpt-6.1-sol high | Preparing and checking board gate |
| Implementation | New Codex agent, not created | Awaiting fb-107 promotion/claim |
| QA | New Antigravity/Agy agent, not created | Awaiting implementation Review and fb-108 promotion |

Paseo host: RPI, server `srv_rvF3fpEOPEZD`. Codex and Antigravity providers are
available; no named launch profiles are configured. Discover current models and
check quota immediately before launch. Use the existing checkout sequentially.

## Kickoff briefs

- [Codex implementation](prompts/codex-apply.md)
- [Agy QA](prompts/agy-qa.md)

These briefs stay within the user-approved task scope. Completion reports are
claims: inspect commits and diffs before accepting them. Sequence heavy checks;
record command exit codes and evidence and avoid redundant reruns.

## Remaining

- [ ] Promote fb-107 and fb-108 through permitted Fleetboard controls.
- [ ] Check provider models/quota; launch Codex with the saved implementation brief.
- [ ] Codex claims fb-107, applies branding, checks and commits, and leaves Review evidence.
- [ ] Lead inspects the implementation commit and routes concrete fixes if needed.
- [ ] Launch Agy with the saved QA brief against the implementation commit.
- [ ] Confirm QA evidence; route findings to Codex and retest fixes.
- [ ] Report final commits and verification to the user for final human verification.

## Open findings

None; implementation has not started. Board promotion is the initial prerequisite.
