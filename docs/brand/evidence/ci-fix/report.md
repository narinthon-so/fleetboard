# fb-107 / PR #5 frontend CI fix

Branch: task/fb-107
Commit: f592a0c76bca6309030b963ec8a7b33e1875f6be

Cause: Tailwind's default Vite scan rooted at web/ included tracked generated web/dist JavaScript/CSS, plus non-application scripts. Generated output became input to later builds. The installed Oxide scanner's full file/candidate inventory is local-scan.json. The clean archive export reproduced the failing CI output exactly: index-wc-9dy6C.css (48,235 bytes), versus tracked index-CMZvDXEI.css (48,291 bytes); that specific delta was obsolete --text-base theme declarations (clean-export-before-css-diff.txt). Removing dist entirely before the original-source local build produced 37,441 bytes instead, confirming the broader generated-input feedback. Rebuilding alone was insufficient.

Fix: web/src/index.css now uses @import "tailwindcss" source(".") plus @source "../index.html". The scan covers application sources and the HTML entry, excluding generated output and test/helper scripts. Installed primary package source confirming the syntax and scanner root handling is retained in installed-scanner-source.txt. No dependency, CI guard, auth policy, design master, QA history, or tracker changes.

Changed files: web/src/index.css; web/dist/index.html; removed web/dist/assets/index-CMZvDXEI.css; added web/dist/assets/index-DhoVFquS.css; renamed web/dist/assets/index-CtJGSoVQ.js to index-CRklKWaB.js. JavaScript content is unchanged after CSS dependency filename normalization.

Independent comparison:
- Exported git archive HEAD (90a5b64) into clean-export/, without a worktree or .git directory, using the existing installed dependency tree via a node_modules symlink.
- Original export build reproduced CI filename/hash before the fix (clean-export-before.log).
- Applied only the CSS source change to the export. Both the seeded export build and a fresh export build with dist absent produced the same complete dist tree as the checkout build.
- diff -qr for both comparisons exited 0. Output filenames: index-DhoVFquS.css (37,441 bytes) and index-CRklKWaB.js.
- All output hashes are retained in dist-sha256.json. Comparison evidence: seeded-export-comparison.log, unseeded-export-comparison.log, final-stale-dist-check.json.

Validation commands, all exit 0, one build/test process at a time:
- pnpm --dir web build (unseeded original diagnostic, scoped checkout build, post-commit final build)
- pnpm build in clean-export/web (original CI reproduction, patched seeded export, patched unseeded export)
- web/: pnpm lint
- web/: pnpm typecheck
- web/: FB_HOME=<new temporary directory> pnpm exec node scripts/test-keyboard.js
- FB_HOME=<new temporary directory> node /tmp/fb-107-ci-fix/check-brand.mjs
- diff -qr web/dist /tmp/fb-107-ci-fix/clean-export/web/dist
- git diff --exit-code web/dist
- git status --porcelain web/dist (empty; equivalent to CI's nonempty-status guard)
- git diff --check

Checks/logs: checks.json; lint.log; typecheck.log; keyboard.log; brand.log; scoped-build.log; clean-export-after.log; clean-export-unseeded-after.log; final-build.log. Original raw CI output is original-ci-failure.log. Investigative module resolution attempts were corrected to resolve pnpm's real package path; no validation check failed.

Browser evidence: 12 PNGs board-{dark,light}-{1440,375,320}.png and login-{dark,light}-{1440,375,320}.png, from the production preview with mocked API data and isolated FB_HOME. Loaded approved artwork, accessible names, visible logo/theme controls, and no page errors; keyboard task selection/detail/close test passes. Screenshots visually inspected at mobile widths. The mocked event stream closes, hence reconnecting status in board captures.

Remaining: lead review, push and replacement GitHub CI run. No push/PR/merge was performed. Existing unset NPM_TOKEN ~/.npmrc warning remains non-fatal. Worktree clean. fb-107 remains Review; no transition or human-only board control was invoked.
