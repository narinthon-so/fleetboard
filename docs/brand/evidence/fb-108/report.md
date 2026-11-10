# fb-108 QA Report: Parallel F Branding Rollout Verification

- **Task**: `fb-108` (QA Parallel F branding rollout)
- **Target Task**: `fb-107` (Apply Parallel F branding)
- **Reviewer**: `antigravity dd5cceb` (gemini-3.8-flash-high)
- **Branch**: `task/fb-108`
- **Application Implementation Commit**: `894ccd7cb164a1c2700e946a02f9c26648136ed1`
- **Diff Base**: `de3af437d00c834483b863e570c8bd734ecaeded`
- **Checkout HEAD**: `9cd057d1b0eb78a143073c2b22e6c3166645dd1e`
- **Worktree Status**: Clean (no uncommitted changes, push disabled)
- **Overall Verdict**: **PASS**

---

## 1. Scope & Verification Summary

Antigravity independently validated the Codex implementation of the approved Parallel F branding kit across visual, responsive, network, security, and interaction dimensions. Codex's preliminary implementation evidence mocked API data in Vite preview; this QA closed the evidence gap through independent Playwright testing with realistic board data, active SSE connection states, live UI theme switching controls, keyboard navigation, and embedded server test integration.

### Acceptance Criteria Assessment

| Acceptance Criterion | Result | Evidence / Notes |
|----------------------|:------:|------------------|
| **1. Header & Login Branding**<br>Check dashboard and login branding in light and dark mode at desktop (1440px) and narrow mobile (375px, 320px) widths; verify legibility, spacing, proportions, and absence of overflow. | **PASS** | Bounding box widths: 240px centered on login screen, 200px on board top bar. Clear visual identification and contrast against both Paper/White and Ink/Dark backgrounds. Zero whole-page horizontal overflow (`scrollWidth <= clientWidth` on all viewports). |
| **2. Served Icons & Manifest**<br>Verify actual served favicon, Apple and app icons, manifest paths, and maskable icon; check unauthenticated availability without security policy relaxation. | **PASS** | All 12 runtime assets byte-for-byte identical to approved `docs/brand/parallel-f/` masters. Maskable 512px icon strictly adheres to the 80% diameter safe circle (max symbol radius 152.1px vs 204.8px safe radius limit). Favicon uses the dedicated small-cut drawing aligned to whole pixels at 16px. Embedded Axum server routes serve all assets unauthenticated with correct `Content-Type`. |
| **3. README & Board Interactions**<br>Check README artwork rendering and confirm board controls, filters, theme switching, and existing functional status colours still behave correctly. | **PASS** | README uses HTML `<picture>` with dark/light sources referencing existing committed SVGs at width 360px. Theme toggle button dynamically switches DOM classes and toggles white/color logo variants. Filter row smoothly wraps to scrollable container on narrow screens. Keyboard shortcuts (`j`/`k`/`Enter`/`Escape`) operate normally. Status colors remain intact. |
| **4. Record Evidence & Handoff**<br>Record tested commit, checks run, screenshots/evidence paths, pass/fail findings; link fixes back to fb-107 and leave final QA result for human verification. | **PASS** | 15 full-page PNG screenshots, command logs, asset logs, and test drivers preserved under `/tmp/fb-108-evidence/`. No defects requiring fixes; fb-107 remains in Review for human verification. |

---

## 2. Detailed Verification Results & Exit Codes

### A. Asset Byte-for-Byte Equality (Exit Code: 0)
All 12 public runtime assets match their approved design kit originals in `docs/brand/parallel-f/` and their tracked copies in `web/dist/`:
- `apple-touch-icon.png` (943 B, SHA-256: `08440224a71d`)
- `favicon-16.png` (269 B, SHA-256: `2c7cc3f406c6`)
- `favicon-32.png` (414 B, SHA-256: `0d6ce282f42e`)
- `favicon-48.png` (627 B, SHA-256: `cbb50aa78d65`)
- `favicon.ico` (1303 B, SHA-256: `cc884c9a1277`)
- `favicon.svg` (298 B, SHA-256: `ff463625d36f`)
- `icon-192.png` (949 B, SHA-256: `4983ece65b35`)
- `icon-512.png` (2459 B, SHA-256: `9d19281255da`)
- `maskable-512.png` (2354 B, SHA-256: `bba2244b93f7`)
- `site.webmanifest` (483 B, SHA-256: `ac6d2c562d61`)
- `brand/fleetboard-horizontal-color.svg` (3263 B, SHA-256: `877344694663`)
- `brand/fleetboard-horizontal-white.svg` (3263 B, SHA-256: `6dfa3ad52652`)

### B. Geometry, Safe Area & Small Cut Inspection (Exit Code: 0)
- **Maskable Icon Safe Area**: Analyzed `maskable-512.png` with Pillow. Center is (256, 256), W3C safe circle radius is 204.8px (80% diameter). The maximum radial distance of white symbol artwork from center is 152.1px, well inside the safe zone (74% of limit).
- **Small-Cut Favicon**: `web/public/favicon.svg` uses the approved 48-unit band / 32-unit channel coordinates (`d="M32 32H224L176 80H32ZM32 112H192L144 160H80V224H32Z"`). On a 256px canvas mapped to 16px (16 units/px), all coordinates divide cleanly by 16 (stem at 2px–5px, bars at 2px–5px, channel at 2px, second bar at 7px–10px), guaranteeing crisp pixel-aligned rendering.

### C. Build, Typecheck, and Code Quality Checks (Exit Code: 0)
- `pnpm --dir web typecheck` -> Exit code 0 (`tsc --noEmit`)
- `pnpm --dir web lint` -> Exit code 0 (`eslint .`)
- `pnpm --dir web build` -> Exit code 0 (`tsc && vite build`)
- `cargo fmt --check` -> Exit code 0
- `git diff --check` -> Exit code 0

### D. Embedded Server & Security Policy Tests (Exit Code: 0)
Executed against an isolated temporary directory (`FB_HOME=$(mktemp -d)`):
- `cargo test --test server_test -- test_serves_embedded_web_ui test_login_then_board_and_task_ok` -> Exit code 0 (2 passed).
- Confirmed unauthenticated static asset serving: `/favicon.svg` (`image/svg+xml`), `/favicon.ico` (`image/x-icon`), `/apple-touch-icon.png` (`image/png`), `/brand/fleetboard-horizontal-color.svg` (`image/svg+xml`), `/brand/fleetboard-horizontal-white.svg` (`image/svg+xml`), and `/site.webmanifest` (`application/manifest+json`).
- Manifest icon resolution verified for relative paths without auth cookie.
- SPA fallback serves `index.html` for routes without extensions (e.g. `/task/fb-101`), while paths with extensions that do not exist return HTTP 404.
- Login cookie contains `HttpOnly` and `SameSite=Strict`, with no client-side `Max-Age` / `Expires`.

### E. Existing Interaction Regression Tests (Exit Code: 0)
Executed against temporary isolated environments:
- `pnpm --dir web exec node scripts/test-keyboard.js` -> Exit code 0
- `pnpm --dir web exec node scripts/test-dependencies.js` -> Exit code 0
- `pnpm --dir web exec node scripts/test-note-verification.js` -> Exit code 0

### F. Independent Real-Browser QA Driver (Exit Code: 0)
Test script `/tmp/fb-108-evidence/qa-browser-check.mjs` executed using Chromium:
1. **Login Screen**:
   - Tested 1440px desktop, 375px mobile, 320px narrow mobile in dark and light themes.
   - Logo dimensions: 240.0px width, 53.2px height.
   - Horizontal centering verified: x=600px (1440px), x=67.5px (375px), x=40.0px (320px).
   - Theme switching verified: `.brand-logo-dark` visible and complete in dark mode; `.brand-logo-light` visible and complete in light mode.
   - Screen accessibility: Accessible `alt=""` inside `role="img" aria-label="fleetboard"`, with sr-only `auth` heading text.
   - 0 horizontal scrolling on all viewports (`scrollWidth <= clientWidth`).
2. **Authentication Flow**:
   - Invalid token input displays clear error feedback.
   - Valid token submission successfully logs in, transitions view, and loads board header.
3. **Board Header & TopBar Layout**:
   - Tested 1440px desktop, 375px mobile, 320px narrow mobile in dark and light themes.
   - Logo dimensions: 200.0px width, 44.3px height (satisfies >= 200px minimum width guideline).
   - Theme toggle button: Clicked to switch from dark to light, and from light to dark. Verified DOM `.dark` class toggles and switches logo images accordingly.
   - Narrow mobile responsiveness: TopBar wrapping (`flex-wrap lg:flex-nowrap`) keeps brand logo and system controls (theme toggle, SSE status) on top row, while filter controls wrap into an independent horizontally scrollable container (`order-last w-full overflow-x-auto min-w-0`). Zero whole-page horizontal overflow.
4. **Board Controls & Status Badges**:
   - Keyboard navigation (`j`/`k`, `Enter`, `Escape`) opens and closes task detail modal.
   - Filter comboboxes expand and interact without layout shifts.
   - Semantic status badge colors (Todo, In Progress, Review, Done) maintain standard contrast and readability.
   - 0 browser page errors or unhandled exceptions logged.

---

## 3. Evidence Artifacts

All screenshots and logs are located in `/tmp/fb-108-evidence/`:

```text
/tmp/fb-108-evidence/
├── asset-verification.log             # SHA-256 and byte equality comparison
├── browser-qa.log                     # Full network, console, and assertion log
├── commands.log                       # Commands executed and exit codes
├── qa-browser-check.mjs               # Independent Playwright test runner
├── report.md                          # This report
├── login-dark-1440.png                # Desktop login in dark mode
├── login-light-1440.png               # Desktop login in light mode
├── login-dark-375.png                 # Mobile login in dark mode
├── login-light-375.png                # Mobile login in light mode
├── login-dark-320.png                 # Narrow mobile login in dark mode
├── login-light-320.png                # Narrow mobile login in light mode
├── board-dark-1440.png                # Desktop board in dark mode
├── board-light-1440.png               # Desktop board in light mode
├── board-dark-375.png                 # Mobile board in dark mode
├── board-light-375.png                # Mobile board in light mode
├── board-dark-320.png                 # Narrow mobile board in dark mode
├── board-light-320.png                # Narrow mobile board in light mode
├── board-filter-active-1440.png       # Active combobox dropdown on desktop
├── board-task-modal-1440.png          # Task detail sheet opened via keyboard
└── board-narrow-filter-scroll-320.png # Narrow mobile view showing scrollable filters
```

---

## 4. Findings & Observations

- **Blocker / Critical / Major / Minor**: **None**.
- **Informational Notes**:
  - `~/.npmrc` warning regarding unset `${NPM_TOKEN}` is pre-existing and does not affect builds or tests.
  - `fb serve` CLI correctly enforces an agent execution guard when run without human permission markers, preventing unauthorized server creation by automated agents.
  - Vite preview uses `historyApiFallback` for missing files during client preview; production embedded asset serving and 404 behavior are correctly governed by Axum and validated via Rust server tests.

---

## 5. Pass/Fail Decision & Next Steps

- **QA Verdict**: **PASS**
- **Action on fb-108**: Completed with `--result PASS`.
- **Action on fb-107**: Retained in **Review** for final human verification.
