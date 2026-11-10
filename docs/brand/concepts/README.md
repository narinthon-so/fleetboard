# fleetboard logo concepts

**Selected: A — Parallel F.** The completed [logo kit](../parallel-f/README.md) follows this exploration.

Original concept review. Start with [overview.png](overview.png); [preview.html](preview.html)
contains size, background, mirror, rotation, context, and library comparison tests.
The current drawings are the `v2` SVGs. Earlier drawings remain for comparison.

## Working brief

Assumed subject: this repository's **fleetboard**, spelled lowercase. No tagline.
The README describes one central task board for a person operating many coding
agents across repositories, with human review and git evidence.

Audience: independent developers and operators coordinating coding agents.
Desired character, inferred for this exploration: calm, precise, capable,
independent. Main settings: favicon, dashboard header, CLI documentation, README.
The operator makes the final design choice. Colours and typography are provisional.

Promise: independent work streams, one place to see and review them.

## Strategy and exploration

The skill's local library has 295 developer-tool marks. Its common devices include
containers, negative space, code brackets, blue, and geometric symbols. Avoided
devices: robot heads, AI sparkles, circuit nodes, generic checkmarks, ships, and
literal miniature Kanban boards. The reference set is a developer-tool comparison,
not an exhaustive direct-competitor or trademark search.

Word map: fleet → independent units, formation, parallel; board → common ground,
shared edge, overview; review → evidence, handoff, human judgement.
Intersection: distinct units forming one clear identifier.

Scores are subjective design judgements, 1–5 each: clarity / distinction /
simplicity / relevance / small-size strength.

| Idea | Type | Score | Selection |
| --- | --- | --- | --- |
| Parallel F: separate horizontal lanes resolve into a single F. | Letterform | 5/3/5/5/5 | A: strongest immediate reading |
| Common Ground: three unequal pieces surround the same empty centre. | Abstract | 4/3/5/5/5 | B: quieter, collective alternative |
| Shared Stroke: f and b join through one continuous crossbar. | Monogram | 5/4/4/5/3 | C: most explicit connection to the name |
| Review Gate: a square task passes through an open upright bracket. | Abstract | 4/2/4/5/4 | Too close to ordinary interface icons |
| Echelon: three offset pennants imply a fleet moving together. | Pictorial | 4/2/5/4/5 | Suggests travel and speed too strongly |
| Evidence Seal: a cut corner reveals a second layer inside a square. | Abstract | 3/3/4/4/3 | Relies too much on its explanation |
| Shared Baseline: three independent stems land on one common rail. | Abstract | 4/2/5/4/5 | Too close to charts and equalizers |
| Handoff: two squared loops pass one empty cell between them. | Abstract | 3/2/3/5/3 | Familiar chain-link silhouette |
| Joined Name: custom fleetboard letters share the double-e crossbar. | Wordmark | 3/4/3/4/2 | Weak fit for the favicon requirement |
| Operator: a quiet central dot interrupts three parallel tracks. | Abstract | 3/3/4/5/3 | Looks like a generic routing diagram |

## Construction

All symbols use a 256-unit square, filled outlines, and transparent negative space.

- A: two separate polygons; 40-unit bars, a 24-unit channel, 45-degree terminals.
  The channel creates parallel lanes while the lower piece anchors the F.
- B: three pieces on an 8-unit grid; 40-unit bands, 28-unit outer corner radii,
  and 32-unit openings. The unequal pieces hold a shared open centre.
- C: hand-constructed lowercase initials; 32-unit stems and crossbar, circular
  hook and bowl, and a real counter using even-odd fill. A shared crossbar joins
  both letters without depending on an installed typeface.

The exploratory lockups use **DejaVu Sans Mono Bold**, converted to SVG paths.
Its permissive Bitstream Vera licence allows use; the installed package's licence
notice is reproduced in [font-license.txt](font-license.txt). No font binary is
included. Typography is an exploratory setting, not an approved final wordmark.

## Review and limitations

Two rounds were rendered with CairoSVG and inspected. HTML previews were also
rendered in Chromium. Inspected sizes: 16–256 px; additionally inspected black,
reversed white, blur, mirror, 180-degree rotation, browser tab, avatar crop, header,
and a shelf of eleven developer-tool library marks.

- A retained its silhouette and readable F at 16 px; its sharp terminals can feel
  sporty. The second round retains A's original geometry.
- B's first drawing had an 8-unit cutout that disappeared when small. The second
  removes it, opening the lower-right shape into a consistent elbow. It is calmer
  but can resemble a focus or scan frame.
- C's second drawing extends the hook and merges the crossbar with the b stem.
  It reads as fb but is the densest option at 16 px; 24 px is a safer working size.

All three current symbols passed the skill's structural SVG audit without
reported issues (100/100 heuristic). This is not a judgement of legal originality.
The audit did not flag the first round's tiny cutout; visual inspection did.
The preview deliberately includes black-on-dark failure examples: use the white
treatment on dark surfaces. Approved reversed geometry and small-size cuts remain
part of the selected direction's kit.

Recommendation: **A — Parallel F**, for immediate reading and a simple silhouette.
No professional trademark clearance or exhaustive similarity search was performed.
Arrange a professional trademark and reverse-image search before public adoption.

## Concept checkpoint

Choose a direction, request changes, or combine specific qualities. The invoked
logo-design skill requires choosing a direction and agreeing to the kit before
Phase 7. The full kit would contain a colour palette, mono and reversed drawings,
horizontal and stacked lockups, a small-size cut, favicon/app/web icons, relevant
presentation mockups, and a compact usage guide.
