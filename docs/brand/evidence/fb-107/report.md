# fb-107 implementation evidence

Branch: task/fb-107
Implementation commit: 894ccd7 feat(brand): apply Parallel F identity

Changes: shared accessible BrandLogo with the supplied color/white outlined horizontal SVGs; dashboard responsive filter row; login artwork; supplied browser/platform icons and relative manifest in web/public, copied into tracked web/dist; theme-aware README artwork. No functional status color, server policy, auth policy, or design archive changes.

Checks (all exit 0):
- web/: pnpm typecheck
- web/: pnpm lint
- web/: pnpm build
- FB_HOME=$(mktemp -d) cargo test --test server_test test_serves_embedded_web_ui (includes unauthenticated branding routes, content types, manifest-relative icon resolution, PNG signatures and SPA fallback)
- web/: FB_HOME=$(mktemp -d) pnpm exec node scripts/test-keyboard.js
- web/: FB_HOME=$(mktemp -d) pnpm exec node scripts/test-dependencies.js
- FB_HOME=$(mktemp -d) pnpm --dir web exec node scripts/test-note-verification.js
- FB_HOME=$(mktemp -d) node /tmp/fb-107-evidence/check-brand.mjs (Vite production preview, API mocked, dark/light dashboard and login at 1440/375/320 px; accessible names, loaded image, visible logo/theme control, no page errors)
- Python byte comparison: all 12 public runtime assets equal approved originals and dist copies
- git diff --check
- cargo fmt --check

No test failures. pnpm emits an existing ~/.npmrc warning about an unset NPM_TOKEN interpolation; commands succeed.

Screenshots: board-{dark,light}-{1440,375,320}.png and login-{dark,light}-{1440,375,320}.png in this directory (12 screenshots).
Test driver: check-brand.mjs. Screenshots use mocked empty-board API data; reconnecting status reflects the mocked event stream closing.

Implementation ready for lead diff review and Agy QA task fb-108. QA and final human verification remain pending. No push, PR, merge, tag or deployment performed.
