# Agy kickoff — fb-108

QA the Codex implementation of approved Parallel F branding in fleetboard. The
user assigned QA to Agy and the parent session is the lead. This is QA of fb-107,
before final human verification. Do not start before the lead supplies the exact
implementation commit and confirms Codex is no longer writing in this checkout.

Repo: `/home/nsubuntu22/code/fleetboard`. First run `fb prime`, inspect
`fb show fb-108` and `fb show fb-107`, then claim fb-108 after normal promotion.
Respect the prescribed task branch; if a branch change is required, coordinate
with the lead before changing it. If Fleetboard refuses a command, stop and report
the refusal. Never use `--as-human` or bypass board guards. All board tests and
servers must use `FB_HOME=$(mktemp -d)`; never test against the real board.

Read repository guidance, `docs/brand/rollout.md`, the implementation diff, and
`docs/brand/parallel-f/usage-guide.html`. Validate against the real approved files.

Check:

- Header and login branding in both themes and desktop/narrow mobile layouts:
  clear identification, contrast, proportions, spacing, no new overflow.
- Actual served SVG/ICO favicon, Apple/app PNGs and manifest paths. Exercise the
  built/embedded route, not only source files; inspect HTTP responses/content types
  and expected unauthenticated availability without relaxing security policy.
- README images and theme treatment; 16/32 px favicon reading and maskable safe area.
- Board controls, filters, theme switching, and existing functional status colours.

Use a real browser when available. Record the exact tested commit, commands with
exit codes, screenshot paths, and reproducible findings with expected/actual
behaviour. The lead sequences heavy checks, so coordinate before a full build or
suite. Do not claim tests ran without actual output. Do not implement fixes; route
them to Codex through the lead. Do not mutate the real board in QA tests.

No push, PR, merge, tags, final verification, or edits to tracker notes/briefs.
If saving tracked QA evidence requires a commit, coordinate with the lead first;
every commit must end in a blank-line-separated co-author trailer naming the real
provider/model, never a guessed model. The lead records durable findings.

Before stopping, add a handoff using `fb note fb-108`. When QA is complete, use the
note-task result required by `fb done fb-108 --result ...`, accurately distinguishing
PASS from FAIL and listing any remaining failures. Never verify fb-107 on the
human's behalf. Final REPORT: tested branch/commit, checks and exit codes,
screenshots/evidence paths, findings by severity, pass/fail, and task state.
