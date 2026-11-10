# Parallel F rollout handoff

The user selected **A — Parallel F**. Codex applied it in commit `894ccd7`, Agy
reported a QA pass, and the lead reviewed the evidence and ran focused checks.
Both fb-107 and fb-108 are in **Review**, ready for final human verification.
All design work, implementation evidence and QA receipts are saved under
`docs/brand/`. See [implementation-notes.md](implementation-notes.md) and the
[QA coverage clarification](evidence/fb-108/README.md).

## Codex — fb-107

Task: **Apply Parallel F branding (Codex)**, code task.

Delivered on `task/fb-107`, which contains the design assets and implementation.
The original design base is `bfefca8`; do not recreate the approved artwork.
The following scope and acceptance criteria were used for the implementation.

Use the logo in the dashboard header, login screen, browser/platform icons, and
README. Preserve functional status colours and existing behaviour. Run the web
build, typecheck, and relevant existing checks. When the implementation is ready,
record its commit and evidence, move fb-107 to Review, and hand off to Agy on
fb-108. Keep final verification with the human after QA.

Likely integration points: `web/src/components/TopBar.tsx`,
`web/src/components/LoginScreen.tsx`, `web/index.html`, static asset handling in
the web build and embedded server, and the root `README.md`.

## Agy — fb-108

Task: **QA Parallel F branding rollout (Agy)**, note task.

QA ran after fb-107 had an implementation commit ready for Review. The two tasks
are linked by their goals and handoff notes rather than a hard `blocked-by` edge:
Fleetboard only clears a hard blocker at Done, which would put QA after the
human's final verification.

Check desktop and narrow mobile layouts in light/dark mode; actual served icons,
manifest paths, and maskable assets; README rendering; and board interactions and
theme switching. Record the exact tested commit, checks, screenshot paths, and
pass/fail findings. Route fixes to fb-107, retest as needed, and leave a written
QA result for human verification. Do not mark fb-107 verified on the human's behalf.

## Design sources

- Kit index: [parallel-f/README.md](parallel-f/README.md)
- Overview: [identity-overview.png](parallel-f/presentation/identity-overview.png)
- Usage guide: [usage-guide.pdf](parallel-f/usage-guide.pdf)
- Digital masters: `parallel-f/digital/`
- Browser/platform icons: `parallel-f/web/`
- Download bundle: [fleetboard-parallel-f.zip](fleetboard-parallel-f.zip)
- Original concepts and iterations: `concepts/`

Primary: `fleetboard-horizontal-color.svg` on light surfaces; use the white
lockup on dark surfaces. Use the supplied small symbol below 24 px. Follow the
guide's spacing, minimum sizes, and fixed geometry. The palette is Ember
`#B84A2A`, Ink `#202522`, and Paper `#F5F3EE`. Fonts are outlined in the masters;
the source font licence is included. Print PDFs are RGB reference artwork.

Both tasks were created as drafts and promoted by the user. Agents submitted
implementation and QA for review; final human verification remains outstanding.
