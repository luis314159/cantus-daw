# Cantus icon family — round 39 (`icons`)

One icon family for Cantus, drawn by hand for every device size it is shown at, on the
owner's chosen direction **Slate** (`dir-a`). Nothing is taken from the current kit
(`kit/glyph.rs` is only the inventory), from an icon set or from another DAW.

**State:** set 2 (transport, tools, window keys) **approved by the owner 2026-11-09**; set 6 (the track-icon library, 316 icons) through its final pass, ready for review; sets 3 (mixer strip and inspector), 4 (browser) and 5 (the rest of `glyph.rs`) ready for review. The family's account is `docs/round39/icons.md`; every `kit::Glyph` is mapped in `spec-rest.md`.

**Earlier state: second pass, then ready for the owner's review.** The first pass (`v1/`) was not
approved. The owner, 2026-11-09: *"Vuelve a compararlos con FL y otros DAWs, creo que lo
podemos hacer mejor: más elegantes, bonitos y mejor pensados."* The second pass studied
Ableton Live, Bitwig Studio, Logic Pro, Studio One and FL Studio at the same key height (the
notes are in the private `research/ui/round39/icons/study.md`). It then rethought every
metaphor and redrew the set for Slate's key heights. The mixer-strip, inspector and browser
sets come after the owner's review.

## What changed from the first pass, and why

- **Slate's key heights.** Slate's keys are 16, 20 and 24 px at 100 % (24, 30 and 36 px at
  150 %). The first pass drew canvases inside keys 1.5 times their size. Every glyph is now
  drawn for the key it sits in, on Slate's wells, in Slate's inks.
- **One vocabulary.** Every glyph is monoline at one weight per size: 1 px at keys of
  16–24 and 2 px at 30–36. The transport is filled. A mass is filled: speaker, gear, small
  plug, metronome. The first pass mixed solid blocks, outlines and diagrams in one row.
- **Icons, not diagrams.** Count-in is now clicks, then play. Follow is the playhead, then
  an arrow. Slice is scissors. The window keys are now one idea each: clips as pills, a
  piano's keys, rows of steps, faders, a folder, a plug, a panel with its sidebar.
- **Designed details:**
  - Loop is two opposed arrows with rotational symmetry, and each head is its own
    stroke's end.
  - The metronome is a solid tapered body with its rod cut out (negative space).
  - The magnet's caps are cut from the U by one pixel.
  - The pointer stands alone; the marquee was a diagram.
  - The gear has six teeth at 16 px and eight from 20 px up.
- **Optical sizes.** Play is 0.4 of the key; stop is smaller and record larger.
- **Inks.** At rest a glyph uses ink 2 with ink 3 as its second ink; hover uses ink 1 and
  ink 2. On is the accent (`#5CAFE9` / `#1F6EC2`), green for play and red for record, with
  20 % of the colour in the well. The first pass's second ink made some glyphs look
  disabled (the skip's wake, the start line). The second ink is now kept for real
  structure: count-in's clicks, unlit steps, fader slots, the pencil's wood.

Variants were drafted for the hard metaphors (`variants.html`: three loops, three
metronomes, three count-ins, three follows, three slices, two each for the window keys and
the magnet), and the best was chosen by looking.

## Files

| File | What |
|---|---|
| `grammar-{night,paper}.png` | the grammar: sizes, weights, proportions, magnified keys, inks per state, the rules |
| `set2-{night,paper}-1x.png` | the whole set at 1:1: transport, tools, windows; every key size, hover and on |
| `set2-{transport,tools,windows}-{night,paper}-4x.png` | the same at 4×, nearest neighbour (the real pixels) |
| `before-after-{transport,tools,windows}-{1x,4x}.png` | today's Cantus (beta.13) beside the second pass, both themes, at rest, hover and on |
| `research/ui/round39/icons/vs-daws-{transport,tools,windows}-{1x,4x}.png` | **private**: FL Studio, Ableton Live, Bitwig Studio, Logic Pro and Studio One, each scaled so its key is as tall as ours (24 px and 36 px), beside Cantus in Slate Night and Paper |
| `research/ui/round39/icons/study.md` | **private**: what makes each DAW's family elegant, and what the first pass got wrong |
| `variants.html` | the drafts of the hard metaphors |
| `family.js` | Slate's tokens, the well, the states, the pixel notation |
| `glyphs-n-*.js` | the drawings (pixel rows and snapped SVG); some 20, 24, 30 and 36-px keys reuse first-pass drawings that were made for exactly those pixels (`v1: [name, size]`) |
| `sheet.html`, `grammar.html`, `render.py`, `compare.py` | the sheets; Edge headless at 1:1, 4× by nearest neighbour |
| `set3-mixer-{night,paper}-{1x,4x}.png` | set 3: every mixer/inspector glyph at each of its sizes (100 % and 150 %), bare (quiet), in a well, on |
| `before-after-mixer-{1x,4x}.png` | today's strip buttons (round 37) beside set 3 |
| `research/ui/round39/icons/vs-daws-mixer-{1x,4x}.png` | **private**: FL, Ableton, Bitwig, Logic, Studio One strip controls scaled to our key height, beside set 3 |
| `spec-mixer.md` | the kit builder's spec for set 3: sizes, places (des-mixer's), inks per state, construction |
| `glyphs-m-mixer*.js`, `sheet-mixer.html`, `compare_sets.py` | set 3's drawings, sheet and comparisons |
| `set4-browser-{night,paper}-{1x,4x}.png`, `before-after-browser-{1x,4x}.png` | set 4 and today's icons beside it |
| `research/ui/round39/icons/vs-daws-browser-{1x,4x}.png` | **private**: FL, Ableton, Bitwig, Logic browser glyphs scaled to our key height, beside set 4 |
| `spec-browser.md`, `glyphs-b-browser*.js` | set 4's spec and drawings |
| `set5-rest-{night,paper}-{1x,4x}.png`, `before-after-rest-{1x,4x}.png`, private `vs-daws-rest-*` | set 5's sheets and comparisons |
| `spec-rest.md`, `glyphs-r-*.js` | set 5's spec (with the whole `Glyph` map) and drawings |
| `set6-tracks-grouped-{night,paper}-{1,2,3}.png`, `set6-tracks-{night,paper}-4x.png`, `set6-tracks-picker-{night,paper}-125.png`, private `vs-daws-tracks-*` | set 6: the track-icon library, **316 icons in 17 families** (the owner's order, `set6-additions.md`): every family titled, every icon at 24 px (125 %) with its English and Spanish names; every icon at 12/14/16/24 px at 4×; the picker (search, None, Recent, the families), track headers and mixer strip heads at 125 % |
| `guitars-before-after-{night,paper}-4x.png` | the single-cut and offset electric shapes the owner found odd, before and after, beside the double cutaway and the slab |
| `spec-tracks.md`, `glyphs-t-tracks*.js`, `glyphs-t-lib*.js`, `glyphs-t-add-{a,b,c}.js`, `glyphs-t-final.js`, `glyphs-t-families.js`, `sheet-tracks.html`, `picker.html` | set 6's one spec (every icon's names and aliases, the construction, the weights and angles, the final pass's redraws and variants, the picker's measures), the constructions in load order, and the sheets (`sheet-tracks.html?review=ids` draws any ids at every size) |
| `set6-add-{a,b,c}-*.png`, `sheet-add-*.html`, `sheet-tracks-add-a.html` | the three additions' own review sheets, before the final pass (kept for the record) |
| `guitars-v3-{night,paper}-{1x,4x}.png`, `guitars-v3-before-after-{night,paper}-{1x,4x}.png`, private `research/ui/round39/icons/guitars-v3-vs-photos-*.png` | the guitar family redrawn after the owner's second rejection: every type at 14, 16 and 24 px; before and after; one study photograph beside each icon |
| `guitars-study.md`, `glyphs-t-guitars*.js`, `sheet-guitars.html`, `fit.py` | the study (what tells each type apart, in the order it survives at 16 px), the constructions, the pixel rows at 12–16 px (fitted, then by hand), the sheets |
| `v1/` | the first pass, kept for the record |

## Inventory and state

| Set | Glyphs | State |
|---|---|---|
| Transport | to start, back, play, pause, stop, record, forward, loop, metronome, count-in, follow | drawn, 5 sizes |
| Tools | snap, undo, redo, draw, select, slice, erase, mute, zoom, eyedropper, settings | drawn, 5 sizes |
| Windows | arrangement, piano roll, channel rack, mixer, browser, plug-ins, inspector | drawn, 5 sizes |
| Mixer strip and inspector (set 3) | phase Ø, swap, mono, link, arm, enable, FX on/off, send, output, input, return, sidechain key, routing, freeze, VCA, insert lamp on/bypassed, grip, mini view, more, add, section open/folded (M and S are type) | drawn at des-mixer's sizes; `spec-mixer.md` |
| Browser (set 4) | folder, folder open, file, audio, MIDI, preset, unit marks (instrument, effect), pack, favourite (outline, filled), search, history, clear, sounding, installed, get | drawn at des-views' sizes; `spec-browser.md` |
| The rest of `glyph.rs` (set 5) | help, warning, agent, device, typing keyboard, score, mouse menu, automate, filter, replace, warp, plug-in stopped, controller, lane tools, session, stage, menu, down, prev/next, close, minimise/maximise/restore, resize grip, resize, typed, send (would loop) | drawn; `spec-rest.md` maps every `kit::Glyph` |

Open for the owner's eyes:
- The plug is solid at 16 and 20 px and drawn in line from 24 px.
- The rack's unlit steps are in the second ink, which is quiet on Night.
- The six-tooth gear at 16 px.
