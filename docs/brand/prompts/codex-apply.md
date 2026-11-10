# Codex kickoff — fb-107

Implement the approved Parallel F branding in fleetboard. The user approved the
scope and assigned implementation to Codex, QA to Agy, with the parent session
leading. Work in `/home/nsubuntu22/code/fleetboard` on `task/fb-107`, based on the
saved design commit `bfefca8` and subsequent tracker-preparation commit.

First run `fb prime`, then inspect `fb ready` and `fb show fb-107`; claim fb-107
only after normal promotion. If the board refuses a command, stop and report the
exact refusal. Never use `--as-human`, bypass guards, change board storage directly,
or strip identity. Use a temporary `FB_HOME` for every board test or QA server.

Read repository guidance, `docs/brand/rollout.md`, `docs/brand/parallel-f/README.md`,
and `docs/brand/parallel-f/usage-guide.html`. Use the supplied outlined artwork;
preserve its geometry, spacing and Ember/Ink/Paper palette. The small cut is for
marks below 24 px; use the white lockup on dark surfaces.

Deliver the fb-107 acceptance criteria:

1. Dashboard header and login branding, correct in light/dark mode and narrow
   layouts. Preserve accessible names and readable product identification.
2. Supplied favicon, Apple/app icons and manifest, with working paths through both
   the web build and Rust embedded serving. Inspect current asset serving before
   deciding where the runtime files belong; do not copy the entire design archive
   into the runtime bundle.
3. README branding using the approved artwork.
4. Existing functionality and status colours preserved. No product redesign,
   unrelated refactor, deployment, or changes to board permission/auth policy.

Likely files: `web/src/components/TopBar.tsx`, `LoginScreen.tsx`, `web/index.html`,
web public/static assets and embedded serving, root README. Inspect actual code.
Use pnpm in web/. Run appropriate existing focused checks, typecheck and build,
one heavy command at a time. Do not add tests that merely mirror static artwork.
Never run existing board scripts against the real FB_HOME. Report any existing
failure separately with the command and exit code.

The lead owns tracker notes and routing; do not edit implementation-notes.md or
the kickoff briefs. Only this agent writes application code while implementing.
Commit locally; do not push, create a PR, merge, tag, or mark the task verified.
Every commit must end, after a blank line, with a Co-Authored-By trailer naming
Codex and the actual model. Use provider alone if the model cannot be established.
Prefer a concise Conventional Commit subject.

Before stopping, add a handoff with `fb note fb-107`, then use `fb done fb-107`
to move to Review with evidence. Name Agy QA task fb-108 in the handoff. Final
REPORT: branch, commit(s), changed files, checks with exit codes, evidence paths,
unresolved issues, and exact Fleetboard status. The lead will verify the diff and
then launch Agy; no final human verification is delegated.
