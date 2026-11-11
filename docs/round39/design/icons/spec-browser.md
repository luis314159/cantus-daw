# Spec for the kit — the browser glyphs (set 4)

The drawings are in `glyphs-b-browser.js` and `glyphs-b-browser2.js`. Each size is its own
drawing: pixel cells or snapped SVG. **Never scale one size to make another.** The places
and sizes are des-views' (`des-views/README.md`, Browser; `browser-pages-*`,
`rack-browser-*`, `browser-*-4x-*`). The grammar and the inks per state are those of set 3
(`spec-mixer.md`). In the rows, a glyph sits bare in the quiet ink, or in ink 2 when its
row is a folder or a pack. A selected row lights it in the accent.

| Glyph | Sizes (100 % → 150 %) | Where (des-views) | Notes |
|---|---|---|---|
| `folder` / `folderOpen` | 12, 14 → 18, 21 | tree rows (icon at +14), "Places"; open when its row is open | outline with a tab and the flap line; open: the back and a slanted front |
| `file` | 12, 14 → 18, 21 | a file of an unknown kind; "Show in folder" | page with a folded corner (second ink) |
| `audio` | 12, 14, 16 → 18, 21, 24 | the Sounds kind, kind chips, results' section titles (a sound's own row shows its waveform thumbnail, not this glyph) | bars every other pixel, heights of a sound, centred |
| `midi` | 12, 14, 16 → 18, 21, 24 | MIDI files and clips | two beamed notes; heads are rounded rects |
| `preset` | 12, 16 → 18, 24 | presets in results and the Presets page | a knob: ring + pointer |
| `instrument` / `effect` | 16 → 24 | inside des-views' 16-px unit mark (the square, filled for Cantus units, outlined for hosted) | instrument: set 2's 16-px piano keys; effect: a filter's curve, 1 px |
| `pack` | 12, 14 → 18, 21 | packs in the tree (when a pack has no cover yet), the Packs page title, menus | a box: lid, body, handle slot |
| `star` / `starFilled` | 12, 14 → 18, 21 | favourites: the preview's star, the plug-ins page | outline (ink 3) / filled; a favourite is `solo` amber |
| `search` | 12, 14 → 18, 21 | the search well (24 high) | set 2's magnifier, drawn for these pixels |
| `history` | 12, 14 → 18, 21 | Recent / history | a clock whose ring runs back, its head where it opens |
| `clear` | 10, 12 → 15, 18 | the search's clear, a chip's × | a pixel cross |
| `speaker` | 12 → 18 | the row being previewed (right end), accent | a speaker and two waves |
| `check` | 10, 12 → 15, 18 | a pack installed (on a disc, `play`) | pixel check |
| `download` | 12, 14 → 18, 21 | a pack's Get key, "Update · 4 MB" | an arrow down into a tray |

The preview's play and loop keys are set 2's `play` and `loop` at the key's size (20 or
24). The tree's fold chevrons are set 3's `foldOpen` and `foldShut`.
