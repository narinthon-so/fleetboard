# fleetboard — Parallel F logo kit

Direction A, selected by the user. The original symbol silhouette is retained.
Start with [the identity overview](presentation/identity-overview.png) and the
[one-page usage guide](usage-guide.pdf).

## Files

| Folder | Contents |
| --- | --- |
| `digital/` | Outlined SVGs and transparent PNGs: symbol, horizontal, stacked, wordmark; colour, ink, black, white, orange; small-size drawings, app tile, avatar |
| `web/` | SVG/ICO favicon, 16/32/48 PNGs, Apple 180 PNG, 192/512 icons, maskable 512, manifest, head snippet |
| `print/` | Vector RGB reference PDFs, with a note on production conversion |
| `presentation/` | Identity overview, self-contained HTML board, slide PNGs/PDF, six developer-tool placement mockups, visual check sheet |
| `source/` | Rebuild script, geometry, palette, typography study, font licence |

Use `digital/fleetboard-horizontal-color.svg` on a light surface and
`digital/fleetboard-horizontal-white.svg` on a dark one. Below 24 px, use the small
symbol. Browser favicons use a white small symbol on an Ember tile so they stay
legible in light and dark browser chrome. The white PNGs are transparent.

The palette is Ember `#B84A2A`, Ink `#202522`, and Paper `#F5F3EE`, with white
reversed artwork. The supporting mockup headline is illustrative copy, not a
required tagline. The board illustrates placements; no application code was changed.

## Refinement and validation

- Compared 24 installed type settings. Retained DejaVu Sans Mono Bold, outlined
  with custom spacing to remove uneven gaps while keeping its coding-tool voice.
- Retained the selected 40-unit symbol bands and 24-unit channel. White symbol
  bands are 38 units to reduce their apparent weight on dark surfaces. Typography
  keeps its original outline weight for readability.
- Built a separate small drawing with 48-unit bands and a 32-unit channel.
  Its straight edges align to whole pixels at 16 px. Tested the favicon tile in
  light and dark browser-tab mockups.
- Checked regular/reversed/small symbols, colour/black/white lockups, 16–256 px
  sizes, blur, mirror/rotation, and developer-tool reference comparisons in Chromium.
- Digital artwork contains outlined vector shapes, with no live text, embedded
  bitmap, filter, mask, or stroke. Presentation labels are ordinary text.
- Structural audit output and export checks are recorded in
  `source/validation.txt`. Maskable artwork stays within its central safe circle.
- SVG and PNG masters use sRGB values. The twelve vector PDFs are RGB reference
  files, not press-profiled CMYK artwork. CMYK numbers in the guide are unprofiled
  estimates. A printer must convert/proof for the intended stock and process;
  no unverified Pantone number is supplied. Print minimum sizes are recommendations
  pending a physical proof.
- No professional trademark clearance was performed. Arrange a professional
  trademark and reverse-image search before public adoption.

Reference logos in the visual check sheet are trademarks of their respective
owners and are included only for comparison and study.

## Rebuilding

`source/build.py` rebuilds the digital masters, rasters, web icons, and vector PDFs
using Python 3 with `fonttools`, `cairosvg`, and `Pillow` installed. It expects a
local DejaVu Sans Mono Bold font; use `--font /path/to/DejaVuSansMono-Bold.ttf` to
select it. No font binaries are included. The licence is copied in
`source/font-license.txt`. Presentation and guide files are maintained separately.

Copy the contents of `web/` together when adopting the web icons. The manifest uses
relative icon paths; the example head snippet assumes deployment at the public
root. Adjust paths if placing the set under a prefix.
