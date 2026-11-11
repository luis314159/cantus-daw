# Round 39 — the icon family (`icons`)

*The owner, 2026-11-08: "hay botones que se ven super feos … vamos por lo difícil, a mano".*
Round 39's `icons` agent drew one icon family for Cantus by hand, size by size. It replaces
every drawing in `kit/glyph.rs` and adds the glyphs that the new views (Slate,
des-mixer, des-views) need. It is design only: no Rust was changed. The kit builder turns
the sheets into code.

## How it was made

1. **First pass** (`docs/round39/design/icons/v1/`): a grammar and the transport, tool and
   window sets. The owner did not approve it: *"Vuelve a compararlos con FL y otros DAWs,
   creo que lo podemos hacer mejor: más elegantes, bonitos y mejor pensados."*
2. **A study** of Ableton Live, Bitwig Studio, Logic Pro, Studio One and FL Studio at the
   same key height (private: `research/ui/round39/icons/study.md`). Each family is elegant
   for its own reasons: Ableton's strict monoline and reduced metaphors; Bitwig's
   silhouettes and arrowheads that grow from their stroke; Logic's symmetry and its tuned
   optical sizes. The first pass mixed vocabularies, drew diagrams instead of icons, set
   arrowheads on top of their strokes, and was drawn for keys 1.5× too large for Slate.
3. **Variants** of each hard metaphor (`variants.html`), chosen by looking.
4. **Second pass** (set 2) on Slate's tokens and keys: **approved by the owner on
   2026-11-09.** The kit builder started on it.
5. **Sets 3, 4 and 5** were drawn with the same grammar at the sizes des-mixer and
   des-views specify. They are **in use in the app** (el-mixer, el-views, el-controls,
   lo-mixer, lo-rest) and **pending the owner's review** (ADR 0111 §6).

## The grammar

- **Drawn per device size.** No size is scaled from another. Slate's keys are 16, 20 and
  24 px at 100 %, and 24, 30 and 36 at 150 %. Set 3 and set 4 marks sit at 8–18 px and
  their 150 %. A 16-px drawing at 150 % is the 24-px drawing.
- **One vocabulary.** Monoline at one weight per size: 1 px up to 24 px, 2 px at 27–36.
  Transport primitives are filled. A glyph that is a mass is filled: the speaker, gear,
  metronome, small plug and arm dot. Nothing else is.
- **Proportions.** Play is 0.4 of its key. Stop is smaller and record larger, for equal
  optical weight. A line glyph fills about 0.6 of its key. Triangles sit right of centre
  by a sixth of their width.
- **Pixels where they matter.** Below 24 px, 45° lines, small rings, arrowheads and chevrons
  are hand-placed pixel cells. Straight runs sit on whole pixels, or on half pixels for
  1-px strokes. Curves are true arcs.
- **Heads grow from strokes.** A 3–5-row pixel head ends a 1-px stroke. At 2 px the
  head is a chevron of the stroke's own weight.
- **Negative space.** The metronome's rod is cut out of its body. The magnet's caps and the
  speaker's cone are cut apart by one pixel. A plug that stopped is drawn pulled apart.
- **Two inks with a meaning.** The main ink is the subject. The second ink is structure:
  count-in's clicks, fader slots, the pencil's wood, unlit steps, the staff.
- **State is colour, never shape.** Slate's tokens:

  | State | Main ink | Second ink | Well |
  |---|---|---|---|
  | quiet (bare glyphs) | ink 3 | ink 4 | none |
  | rest | ink 2 | ink 3 | well |
  | hover | ink 1 | ink 2 | well-hi |
  | on | accent | accent mixed into the well | well + 20 % of the accent |
  | off | ink 4 | ink 4 | well |

  On is green for play and enable, red for record, arm and a stopped plug-in, and amber
  for a favourite and a warning.

## The sets and what each replaced

| Set | Glyphs | Replaced |
|---|---|---|
| 2 — transport, tools, window keys (**approved**) | 29: to start, back, play, pause, stop, record, forward, loop, metronome, count-in, follow; snap, undo, redo, draw, select, slice, erase, mute, zoom, eyedropper, settings; arrangement, piano roll, channel rack, mixer, browser, plug-ins, inspector | round 37's 12-unit-grid glyphs (white blocks at 65–75 % of the key, orange strokes at rest, a mushy gear and eyedropper at 16–20 px) and the first pass's diagrams |
| 3 — mixer strip and inspector | 23: polarity, swap, mono, link, arm, enable, FX, send, output, input, return, sidechain key, routing, freeze, VCA, insert lamp on/bypassed, grip, mini view, more, add, section open/folded | round 37's strip buttons (Ø, ⇄ and the rings at 24 pt in raised caps, the bulb, the dot) and des-mixer's placeholders (`mglyphs.py`), whose sizes and places are kept |
| 4 — browser | 17: folder (closed, open), file, audio, MIDI, preset, unit marks (instrument, effect), pack, favourite, search, history, clear, sounding, installed, get | today's browser marks and des-views' placeholders |
| 5 — the rest of `glyph.rs` | 30: help, warning, agent, device, typing keyboard, score, mouse menu, automate, filter, replace, warp, plug-in stopped, controller, lane pencil/line/curve, session, stage; menu, down, prev, next, close, minimise, maximise, restore, resize grip, resize, typed, send (would loop) | the remaining `Glyph` drawings; the bulb is retired (enable and the insert lamp take its place) |

| 6 — track icons (the owner's request, 2026-11-09, extended after his review) | an instrument library of 82 icons a user picks for a track, in 12 families (drums, percussion, tuned percussion, guitars and bass, strings, winds and brass, keys, electronic, voice, sound and FX, mix, marks and shapes), each with English and Spanish names; the picker (search, None, groups, scrolling) | new: tracks had only their type icon |

Set 6 is **constructed per size** (a 24-unit design snapped to each size's pixels, detail
per size) rather than placed pixel by pixel, because it holds about 820 drawings. Instruments are silhouettes with their naming feature cut out (Logic's track icons were the benchmark); the hard ones were drawn in variants and chosen at 14/16/24 px (`spec-tracks.md`).

`design/icons/spec-rest.md` maps every `kit::Glyph` to its drawing in the family and lists
the glyphs the kit must add.

## Files

All in `docs/round39/design/icons/` unless named otherwise.

| | |
|---|---|
| `README.md` | the family's working index |
| `grammar-{night,paper}.png` | the grammar sheet |
| `set2-*`, `set3-mixer-*`, `set4-browser-*`, `set5-rest-*` (`-1x`, `-4x`) | each set in Slate Night and Paper, at 1:1 and at 4× by nearest neighbour |
| `before-after-*.png` | today's Cantus beside each set |
| `spec-mixer.md`, `spec-browser.md`, `spec-rest.md` | the kit builder's specs: sizes, places, inks per state, construction, the whole map |
| `glyphs-n-*.js`, `glyphs-m-*.js`, `glyphs-b-*.js`, `glyphs-r-*.js`, `family.js` | the drawings (pixel rows and snapped SVG) and the renderer, which are the source to port |
| `sheet.html`, `sheet-mixer.html`, `grammar.html`, `variants.html`, `render.py`, `compare.py`, `compare_sets.py` | the sheets and their tools (Edge headless at 1:1, 4× by nearest neighbour) |
| `v1/` | the first pass, kept for the record |
| `research/ui/round39/icons/` (private) | `study.md`; `vs-daws-*.png`: each set beside FL Studio, Ableton Live, Bitwig Studio, Logic Pro and Studio One, scaled to our key height |

## Verified only on the sheets

Every drawing was rendered with Edge headless at 1:1 device pixels. Each sheet was read at
1:1 and at 4×, and what did not hold was redrawn: the metronome, the playhead marker, the
16/20-px gear, the eyedropper, the piano roll, routing and VCA. When this was written no glyph
had been seen in the running app; since then sets 2–6 are in the app and shot on its CPU
renderer, but not yet seen on a real screen. egui's tessellation of arcs, and the way it places a crisp-edged
pixel run, can differ from Chromium's. The kit builder should check each set with
`cargo run -p cantus-ui --example shoot` at 100 % and 150 %, against these sheets, before
the owner judges whole windows.

## Left for the owner

- Set 3 (mixer strip and inspector), set 4 (browser) and set 5 (the rest) are in use in
  the app and await the owner's review (ADR 0111 §6).
- Open questions:
  - the plug, solid at 16–20 px and drawn in line from 24 px;
  - the rack's unlit steps in the second ink;
  - the six-tooth gear at 16 px;
  - the 12-px outline star, which is soft, as all small stars are;
  - the session grid's lit cell at 16 px, which differs only by ink.
