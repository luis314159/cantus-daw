# Cantus icon family — round 39 (`icons`)

One icon family for Cantus, drawn by hand from a blank sheet for every device size it is
shown at. Nothing here comes from the current kit (`kit/glyph.rs` was read only as an
inventory), an icon set or FL Studio: FL is the reference for *quality* — calm, light grey
ink with a second tone, a lot of air, colour only for state — and none of its shapes or
values is reproduced.

**State: delivery 1 (the grammar, the transport, the top bar and toolbar) is ready for the
owner's review.** Next, once the owner has looked: the mixer strip and inspector set, then
the browser set (see *Inventory*).

## The grammar (`grammar-night.png`, `grammar-paper.png`)

- **Five drawings per glyph**, one per device size: 16, 20, 24, 30, 36 px. The 150 % sizes
  are their own drawings (16 @150 % = 24, 20 @150 % = 30, 24 @150 % = 36); a 16-px glyph at
  150 % and a 24-px glyph at 100 % are the same device pixels and share one drawing.
- **The key is one and a half canvases**: 24, 30, 36 px at 100 %; 45, 54 at 150 %. The canvas
  is centred and floored to a whole pixel.
- **Proportion**: a solid primitive is a third of the key (play 8 in 24, 12 in 36); stop is a
  quarter so it weighs the same to the eye; a line glyph stays inside half the key. The air
  is the point (BACKLOG §6j, the button study: ours filled 65–75 %).
- **Weights**: primary line 1, 1, 2, 2, 3 px (16 → 36); detail line 1, 1, 1, 1, 2 px. Whole
  pixels only: odd widths on half-pixel centres, even widths on whole ones.
- **Two inks**: `ink-1` is the subject (the arrow, the rod, the clips, the notes, the poles
  that snap); `ink-2` is its structure (the start line, the magnet's body, fader slots,
  keyboard keys, frames, a wake). Plus `ink-on`, `ink-on-dim`, `ink-off`, `ink-rec`,
  `ink-rec-on`. Every direction sets the values; the sheets use neutral placeholders with
  contrast ratios shown.
- **Filled or outlined**: transport filled; tools are lines with filled heads and masses;
  window keys are small diagrams of filled content on ink-2 structure — never a box drawn
  inside the key's box.
- **Terminals** cut square; arrowheads solid triangles; 45° lines at 16 and 20 px drawn pixel
  by pixel as clean staircases; curves with hand anti-aliasing below 24 px, true arcs above.
- **Corners**: solids square up to 20 px, 1–1.5 px radius from 24; clips and notes 1 px.
- **Air cut**: one pixel of the key's well between touching parts (magnet poles, speaker body
  and cone, fader grips).
- **On**: ink-1 → ink-on, ink-2 → ink-on-dim, the well tinted 13 % with a 1-px edge in the
  on colour. No glyph changes shape when on. Record is the one colour at rest (a muted red).
- **The key** in these sheets is a placeholder (a flat well a little darker than the bar,
  1-px dark edge, a barely-there top line, §6j item 2); the directions own the real one.

## Files

| File | What |
|---|---|
| `grammar-{night,paper}.png` | the grammar sheet |
| `set1-{night,paper}-1x.png` | delivery 1 whole: transport, tools, windows; every size, rest and on |
| `set1-{transport,tools,windows}-{night,paper}-4x.png` | the same at 4×, nearest neighbour (the device pixels, not a vector re-render) |
| `before-after-{transport,tools,windows}-{1x,4x}.png` | today's Cantus (beta.13) beside the new family, both themes |
| `research/ui/round39/icons/vs-fl-{transport,tools,windows}-{1x,4x}.png` | **private**: FL Studio 24's equivalents cut from the owner's screenshots, today's Cantus, the new family — same key height (FL's keys ≈ 37 px ↔ our 36-px key) |
| `family.js` | tokens, the placeholder key, the pixel notation renderer |
| `glyphs-*.js` | the drawings — pixel rows at 16/20 px, snapped SVG at 24/30/36 px |
| `sheet.html`, `grammar.html`, `grammar2.js` | the sheets (`?t=night|paper&groups=…`, `?strip=<size>&ids=…`) |
| `render.py`, `compare.py` | Edge headless at 1:1, 4× by nearest neighbour; the comparison composer |

Render: `python render.py sheet.html "t=night&groups=transport" out.png --x4 out-4x.png`;
`python compare.py <private-dir> <public-dir>`.

## Inventory and state

| Set | Glyphs | State |
|---|---|---|
| Transport | to start, skip back, play, pause, stop, record, skip forward, loop, metronome, count-in, follow | **drawn, 5 sizes** |
| Tools | snap, undo, redo, draw, select, slice, erase, mute, zoom, eyedropper, settings | **drawn, 5 sizes** |
| Windows | arrangement, piano roll, channel rack, mixer, browser, plug-ins, inspector | **drawn, 5 sizes** |
| Mixer strip and inspector | phase Ø, L/R swap, link, mono, bulb/enable, dot, solo, mute, arm, FX on/off, send, sidechain, routing | next |
| Browser | folder, file, audio, MIDI, preset, plug-in, pack, favourite, search, history | after |
| The rest of `glyph.rs` | menu, down, prev/next, star, more, help, power, resize, close, agent, device, plus, grip, check, keyboard, score, warning, mouse menu, automate, typed, minimise/maximise/restore, replace, key, cables, filter, warp, plug-in stopped, frozen, controller, lane pencil/line/curve, session, stage | after |

Redrawn after looking at 1:1 in this delivery: the metronome (an outlined pyramid turned to
hatching at 16 px → a solid ink-2 body with an ink-1 rod), the playhead marker in count-in
and follow (read as a "T" → a tab with a point), the 20-px gear (detached diagonal teeth read
as a snowflake), the 20-px eyedropper (tube too thin), the piano roll (notes read as a
letter, then as squares → narrow keys and long legato notes).

Open for the owner's eyes: whether line glyphs at half the key are right beside solids at a
third; the slice key's metaphor (a clip cut in two, no knife); the eraser at 16 px.
