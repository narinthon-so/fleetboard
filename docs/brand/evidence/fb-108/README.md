# QA evidence and lead interpretation

Application implementation: `894ccd7cb164a1c2700e946a02f9c26648136ed1`.
Agy tested checkout `9cd057d` on `task/fb-108`, which adds only documentation
after that application commit. This directory preserves Agy's report, driver,
logs, and fifteen screenshots.

## Scope clarification

`qa-browser-check.mjs` runs the production bundle through **Vite preview**, with
intercepted API, login, task data, and SSE responses. Its screenshots, theme-button
checks, layout assertions, keyboard interaction, and static-asset requests are
real browser checks. Its authentication and board responses are mocked; it is
not a browser-to-live-Axum end-to-end test. The SSE fixture closes, so the
screenshots show reconnecting; this does not establish a production SSE defect.

Actual embedded asset routes and login/board behavior are covered separately by
`test_serves_embedded_web_ui` and `test_login_then_board_and_task_ok` in the Rust
integration harness, using isolated temporary board storage. The lead retained
raw stdout and exit codes for its own focused verification in
[lead-verification.log](lead-verification.log). Agy's `commands.log` contains
reported command summaries rather than raw build/test stdout.

The lead inspected the application diff, checked exact equality of all twelve
runtime assets against the approved originals and built copies, and viewed
representative light/dark/mobile, task-modal, and filter screenshots. No application
defects were found. This is the basis for the branding QA pass; it does not imply
that every product workflow was tested against a live backend.

The lead's clean production build pruned four unused CSS classes left from the
old login screen. The JavaScript bytes are identical; only CSS and generated
asset filenames/index references change. No application source changed. The
lead reran the browser driver on this refreshed bundle; its output is retained
in `lead-browser.log`, with selected refreshed screenshots prefixed `lead-`.

Both fb-107 and fb-108 are in **Review**. `fb done` submits work for review; it
does not perform human verification. The original report's references to
"completed" describe finishing QA execution, not a final Done board state.
