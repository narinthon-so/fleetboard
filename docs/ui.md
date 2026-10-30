# fleetboard — UI spec (M1)

Status: draft (2026-10-30), for review before the M1 tasks start.
Stack (D9): React + Vite + shadcn/ui (Tailwind v4), embedded in the `fb`
binary. shadcn supplies the components; this page fixes what is specific to
fleetboard, so every M1 task builds the same board.

## Principles

1. **The human is the bottleneck.** What needs the human (Needs me) is always visible without scrolling or filtering.
2. **Instances, not repos.** Five copies of one repo must never look identical: every claimed card names its agent and its checkout.
3. **Never colour alone.** Every status and signal has an icon and a text label, too (colour-blind safe, readable in screenshots).
4. **Dense, dark, calm.** Dark mode by default, compact cards, 50+ cards on one 1440×900 screen. Colour is reserved for state, not decoration.
5. **The CLI is the source of truth.** The UI shows the same data and allows only the human actions; it never invents states the CLI does not have.

## Tokens

Use shadcn's CSS variables (`--background`, `--card`, `--muted`, `--border`, `--primary` …) with the **zinc** base, dark by default, light as a toggle. Add these semantic tokens in `web/src/index.css`; components use only these names, never raw Tailwind colours.

| Token | Use | Dark value (approx.) | Icon (lucide) |
|---|---|---|---|
| `--status-draft` | draft | zinc-500, dashed card border | `PencilLine` |
| `--status-todo` | todo / ready | zinc-300 | `Circle` |
| `--status-progress` | in_progress | sky-400 | `CircleDot` |
| `--status-review` | review | amber-400 | `Eye` |
| `--status-done` | done | emerald-400 | `CircleCheck` |
| `--status-dropped` | dropped | zinc-600, title struck through | `CircleSlash` |
| `--signal-needs-me` | Needs me | violet-400; 3 px left border on the card | `Hand` |
| `--signal-stale` | stale claim | orange-400 | `Clock` |
| `--signal-no-evidence` | review without commits | red-400 | `GitCommitVertical` with a slash, label "no commits" |
| `--signal-blocked` | blocked / orphaned | zinc-400 / red-400 | `Lock` / `Unlink` |

Priority: `P0` red-400 text, `P1` amber-400, `P2` default, `P3` muted. Shown as a small mono badge, never as card colour.

**Agents.** A provider monogram chip (`C` claude, `X` codex, `A` antigravity, `?` unknown) on a neutral background, followed by the 7-char short id in mono: `[A] 20aabd3`. No provider colours, so they never clash with status colours.

**Type.** Geist Sans 13 px for UI, Geist Mono for ids, branches, paths and commit counts. Two weights only (400, 600).

## Board layout

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ fleetboard   [Project ▾ all] [Instance ▾] [Agent ▾] [Host ▾]  [/ search]   ● live  ☾ │
├──────────────────────────────────────────────────────────────────────────────────────┤
│ ✋ NEEDS ME (3)   [fb-7 review · no commits] [fb-9 plan pending] [fb-4 stale 2h 10m] → │
├────────────┬────────────┬────────────┬──────────────┬──────────────┬─────────────────┤
│ Draft 2    │ Ready 4    │ Blocked 1  │ In progress 3│ Review 2     │ Done 12  ▸      │
│ ┌────────┐ │ ┌────────┐ │ ┌────────┐ │ ┌──────────┐ │ ┌──────────┐ │ (collapsed:     │
│ │ card   │ │ │ card   │ │ │ card   │ │ │ card     │ │ │ card     │ │  last 10,       │
│ └────────┘ │ └────────┘ │ └────────┘ │ └──────────┘ │ └──────────┘ │  "show all")    │
└────────────┴────────────┴────────────┴──────────────┴──────────────┴─────────────────┘
```

- **Top bar:** filters are shadcn `Combobox` controls; the selection is kept in the URL query (`?project=…&agent=…`) so a view can be bookmarked. The `● live` dot shows SSE state (see States).
- **Needs-me strip:** a horizontal row of compact chips under the top bar, always visible and ignoring the column filters. Each chip shows the id plus the reason; clicking it opens the card. It stays as one row and scrolls sideways; with zero items it shows "Nothing needs you" in muted text.
- **Columns:** Draft · Ready · Blocked · In progress · Review · Done (D6). Done is collapsed to a narrow column by default. Dropped tasks are hidden unless the filter "show dropped" is on.
- **Instance filter:** a two-level list, project → instances (`host · path tail · branch · agent`). Picking a project shows all its instances; picking an instance narrows to one checkout. This is the answer to "five checkouts of the same repo".

## Card

```
┌───────────────────────────────────────────┐
│▌fb-12  P1                      ✋ ⏱ 2h10m │   ▌ = Needs-me border, flag icons right
│ Rate-limit public endpoints               │   title, max 2 lines
│ [X] 403254b · RPI · api · task/fb-12      │   agent chip · host · repo · branch (mono)
│ ⎇ 3 commits  +120 −8  ✓ clean   PR #41 ✓  │   evidence line (in_progress / review only)
└───────────────────────────────────────────┘
```

- **Unclaimed cards** (Draft, Ready, Blocked) show the project and the ticket key instead of the agent line, for example `api · ABC-36`.
- **The evidence line** appears only when a claim exists. With no commits it shows `no commits` in the no-evidence colour. With a snapshot it shows `snapshot 3h ago` in muted text.
- **Hover** shows the full branch and path in a tooltip. Cards never grow beyond 4 lines.

## Card detail

The detail opens as a shadcn `Sheet` from the right (about 560 px), so the board stays visible. URL: `/task/fb-12`.

```
fb-12  Rate-limit public endpoints                         [Verify] [Reject] [⋯]
review · P1 · api · ABC-36 ↗ · created by human 2d ago
────────────────────────────────────────────────────────────────────────────────
Overview | Plan | Notes (4) | Evidence | History
```

- **Overview:** goal, acceptance criteria as a checklist (read-only), dependencies (blocked by / blocks, as links), current claim.
- **Plan:** the agent's plan as rendered markdown, with an `Approve plan` button while `plan_pending`.
- **Notes:** handoff, comment and finding notes, newest first, each with its agent chip. A text box adds a human comment.
- **Evidence:** the full evidence table (commits, diffstat, dirty, rebased, PR with checks) and a `Refresh` button.
- **History:** the `events` audit log, including pid / process chain for human actions.
- **Action buttons** depend on state, and only human actions appear: Promote, Approve plan, Verify (`--force` behind a confirm dialog when there is no evidence), Reject (needs a comment), Release, Steal / reassign, Drop, Reopen.

## Drag and drop

Drag and drop only performs the human transitions the CLI allows. Any other drop snaps back, with a toast naming the reason (`fb-12 has no claim — agents move cards by claiming`).

| Drag | Action |
|---|---|
| Draft → Ready | `promote` |
| Review → Done | `verify`; opens a confirm dialog if there is no evidence |
| Review → In progress | `reject`; opens the comment dialog |
| In progress (stale) → Ready | `release` |
| any → Dropped zone (appears while dragging) | `drop`, with a confirm dialog |

## States

| State | What the user sees |
|---|---|
| Loading | Column skeletons (3 grey cards each), never a spinner over the whole page |
| Empty board | One centred message with the first commands: `fb add "…"` and `fb ready` |
| Empty column | Muted "—" with no extra copy |
| SSE live | `● live` in green |
| SSE reconnecting | `● reconnecting` in amber, plus a thin banner after 5 s. Data stays visible but read-only |
| Daemon down | Full-width banner "fb serve is not running — start it with `fb serve`". The last data stays visible, greyed |
| Auth needed | Login screen asking for the one-time token printed by `fb serve` (DESIGN.md, Identity → Known limits). Nothing else is shown |

## Keyboard

`/` focuses search · `j` / `k` move between cards · `Enter` opens the selected card · `Esc` closes it · `g n` jumps to Needs me · `v` verifies and `r` rejects the open card (confirm first) · `?` shows this list.

## Out of scope for M1

Editing task fields in the UI (titles, goals, criteria stay in the CLI), creating tasks from the UI, multi-host views, charts, mobile layout (it must not break below 1024 px, but needs no layout of its own), and notifications.
