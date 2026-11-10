# Codex follow-up — fb-107 / PR #5 CI

Fix the Web Frontend CI failure for https://github.com/narinthon-so/fleetboard/pull/5.
Work in the existing checkout on `task/fb-107`, currently based on `9d3b5ad` plus
the lead's documentation of this follow-up. The user asked to check CI; fixing
this failure is within the approved branding rollout scope.

CI run 42842747357: Rust Format, Clippy, and Ubuntu/macOS tests passed. Frontend
install, lint, typecheck, and build passed; only **Check for stale dist** failed.
On the clean runner the build produces `index-wc-9dy6C.css` (48.23 kB), whereas
the tracked bundle has `index-CMZvDXEI.css` (48.29 kB), with JS names changing
through the CSS dependency hash. Node 24 and pnpm 10.17.1 match local versions.
Raw job log: `/tmp/fb-107-ci-failure.log` (very long minified deleted lines; print
only concise excerpts). Workflow: `.github/workflows/ci.yml`.

The prior clean local build already removed four obsolete login classes. Its JS
bytes were identical to the prior implementation, but CI still differs. Find the
actual cause of clean-checkout versus local CSS differences; investigate content
scanning and ignored/generated files before assuming another stale rebuild will
solve it. Prefer a small deterministic source-scope fix if scanning is responsible.
Use official docs or installed primary package source for Tailwind syntax. Do not
disable or weaken CI's stale-dist check or update package versions unnecessarily.

Verify a clean build and at least one meaningful independent clean-checkout or
clean-export comparison, not merely two incremental builds in the same checkout.
Use a small temporary fixture/export if needed; do not create a git worktree or
modify the real board. Only one build/test process at a time. Rebuild tracked dist
and run web lint/typecheck plus focused browser checks if emitted styles change.
Preserve screenshots and raw outputs under `/tmp/fb-107-ci-fix/` for lead review.

Do not modify tracker notes, saved design masters or QA history. Do not spawn
agents, push, create a PR, merge or touch board human-only controls. fb-107 is
already in Review with your claim; leave it in Review and append a handoff note
with the fix/evidence, rather than invoking a forbidden human transition or
calling `fb done` on a task already in Review. Run `fb prime` first and stop on
any actual board refusal.

Commit locally with a concise Conventional Commit message and the required
Codex GPT-6.1-sol co-author trailer. Final REPORT: cause, changed files, commit,
commands with real exit codes, clean-build comparison evidence, evidence paths,
and remaining concerns. The lead will review, restore push config, push the fix,
and check the new CI run.
