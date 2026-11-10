# fb-107 / fb-108 implementation tracker

## Resume here

The user asked this session to lead implementation and confirmed: "i trust U so
let do". The agreed scope is approved: Codex applies the branding; Agy performs
QA; this session routes work, checks evidence, and handles routine decisions.

Repo: `/home/nsubuntu22/code/fleetboard`; branch: `task/fb-107`.
Design base commit: `bfefca8`. Design handoff: [rollout.md](rollout.md).

The user promoted both tasks; verified both are Todo/Ready on 2026-11-10.
No implementation or QA agent has been launched yet. This session is
`codex ba16e05` and has no Fleetboard tracker privileges. Agents may now use
the normal claim workflow.
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
| Lead | Current Codex session, ba16e05, gpt-6.1-sol high | Coordinating implementation |
| Implementation | New Codex agent, not created; gpt-6.1-sol medium, full-access | Ready to launch and claim fb-107 |
| QA | New Antigravity/Agy agent, not created; gemini-3.8-flash-high, full-access | Awaiting implementation Review |

Paseo host: RPI, server `srv_rvF3fpEOPEZD`. Codex and Antigravity providers are
available; no named launch profiles are configured. Discover current models and
check quota immediately before launch. Use the existing checkout sequentially.
Model discovery confirmed the configured choices above. The older Gemini Pro
mentioned by the generic tracker skill is not advertised by this installation;
the available Gemini high model is used for QA, with lead verification of findings.

Quota at kickoff: Codex OK, 57% of the 5-hour window and 84% of the week remaining;
Claude is not used by this rollout; Antigravity does not expose a remaining limit,
so watch actual errors. No named profiles exist; use the discovered installed
provider settings. Disable push in this checkout while delegated agents run;
the original origin push configuration had no explicit pushurl (fetch URL is
git@github.com:narinthon-so/fleetboard.git). Restore that configuration after all
workers are idle. No remote push is part of the current scope.

## Kickoff briefs

- [Codex implementation](prompts/codex-apply.md)
- [Agy QA](prompts/agy-qa.md)

These briefs stay within the user-approved task scope. Completion reports are
claims: inspect commits and diffs before accepting them. Sequence heavy checks;
record command exit codes and evidence and avoid redundant reruns.

## Remaining

- [x] Promote fb-107 and fb-108 through permitted Fleetboard controls (user).
- [ ] Check provider models/quota; launch Codex with the saved implementation brief.
- [ ] Codex claims fb-107, applies branding, checks and commits, and leaves Review evidence.
- [ ] Lead inspects the implementation commit and routes concrete fixes if needed.
- [ ] Launch Agy with the saved QA brief against the implementation commit.
- [ ] Confirm QA evidence; route findings to Codex and retest fixes.
- [ ] Report final commits and verification to the user for final human verification.

## Open findings

None; implementation is ready to start. The earlier HUMAN_ONLY promotion refusal
was resolved by the user promoting both tasks directly.
