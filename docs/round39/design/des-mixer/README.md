# The mixer and its inspector — build sheets in Slate (round 39, `des-mixer`)

**The idea.** A strip reads top to bottom in signal order, in three blocks that are the same
in every density: **the sound** (number, name, inserts, EQ, polarity/swap/width), **the mix**
(pan, mute/solo/arm, the fader with its meter on one scale, the level), **the route** (sends,
the output, the socket of the cable band). No label column, no "Slot 3 … Slot 10", no "−∞":
a strip at rest shows its grooves and its numbers. Colour is spent only on state (mute, solo,
arm, clip, the selected strip's tint, the selected strip's cables), so a 24-strip desk stays
calm and the eye goes to the one lit strip. The desk **fits first, then scrolls**: as it gets
shorter, rows leave in a fixed order and the fader keeps the rest.

**Why it beats FL Studio and today's Cantus.**
- *Against beta.13:* the saturated slabs, the label column, ten empty slot rows and the "−∞"
  boxes are gone; the meter sits on the fader's own law (one tick reads both); names that do not
  fit wrap to two lines before they ever truncate; the selected strip's routes are drawn in one
  band (output solid, sends thin, sidechain dashed) instead of a lane of triangles.
- *Against FL:* as calm and finished, but it keeps what Cantus does better and FL does not show:
  the inserts and a live EQ curve **in the strip** (FL keeps them in the side panel), numbers on
  every strip (level, sends in dB in Large), a return that lists who feeds it, a VCA that shows
  where it puts its followers, a frozen strip that shows which half is a render (the sound block
  hatched, the mix live), and a "silenced by solo" state distinct from "muted".
- *Honest risk:* at maximised height the Wide strip shows 4 insert rows and an EQ curve per
  strip — more information than FL's Wide strip. It is drawn quietly (a panel one step darker,
  no edges), but the owner should judge whether it reads as "rich" or "busy"
  (`mixer-wide-night-1x.png` against `research/ui/round39/des-mixer/compare-mixer.png`).

## Sheets (all 1280 px logical; `-1x` = 100 %, `-1.5x` = 150 %; Night and Paper)

| File | What |
|---|---|
| `mixer-wide-{night,paper}-1x.png`, `-1.5x.png` | **the whole window, mixer maximised, Wide**, playing at bar 44: master, 10 strips, 3 empty, Reverb and Delay; Chords selected with its cables; inspector open on inserts, EQ, sends |
| `mixer-wide-{night,paper}-idle-1x.png` | the same, stopped: grooves, no meter level, no −∞ |
| `mixer-compact-{night,paper}-1x.png`, `-1.5x.png` | Compact with a 24-strip session (where Compact is used): names up the strip, a VCA and the returns pinned right, armed, muted, frozen strips |
| `mixer-large-{night,paper}-1x.png`, `-1.5x.png` | Large: unit names, EQ with band dots, pan values, sends with dB, receives, LUFS on the master; the middle scrolls (pill "6 ›") |
| `strips-{night,paper}-1x.png`, `-1.5x.png` | **master, track, return in Compact, Wide, Large**, side by side; Wide at 440 and 288 px (what leaves) |
| `redline-{night,paper}-1x.png` | the track strip of each density at ×1.5 with every row's y and height; Wide's horizontal measures |
| `states-{night,paper}-1x.png`, `-1.5x.png` | **every state, one strip each** (Wide and Compact): rest, playing, clipping, armed, muted, soloed, silenced by a solo, selected, inserts bypassed, sends, sidechain key, VCA, VCA follower, frozen, empty; and the routing shown for Chords |
| `inspector-{night,paper}-1x.png`, `-1.5x.png` | **the inspector, every section open**: head, source, inserts (mini view, pointer, bypassed), EQ, sends, routing, sidechain, recording, automation, level, freeze (live and frozen), all folded; an insert row's states; the empty sections |
| `overflow-wide-{night,paper}-1x.png` | 24 strips with long names, Wide, scrolled by 6 (pills ‹ 6 and 6 ›), VCA + returns pinned, a long-named strip selected |
| `overflow-es-{night,paper}-1x.png`, `-1.5x.png` | the same desk **in Spanish**, a solo on (header chip "1 en solo · Quitar"), long Spanish names wrapping, the inspector in Spanish |
| `zoom/*-4x.png` | 1:1 cuts enlarged ×4, nearest-neighbour: Wide top, fader, bottom and cable band, master, Compact top, Large top, inspector inserts, the state keys, VCA/frozen heads |
| `research/ui/round39/des-mixer/compare-mixer.png` (`-half`) | private: FL Studio's mixer (1920, as captured) · Slate Night 150 % · Slate Paper 150 % · Cantus beta.12 150 % |
| `…/compare-strips-1to1.png`, `compare-strips-3x.png` | private: four strips of each at their physical pixels, then ×3 |
| `…/compare-daws.png` | private: Ableton, Bitwig, Logic, Studio One mixers beside Slate Large, scaled to one height |
| `src/` | the generator: `strip.py` (rows, densities, states), `parts.py` (fader, meter, knob, keys, inserts, EQ, sends), `mglyphs.py` (the strip's glyphs per size), `desk.py` (header, docks, scroll, cables), `insp*.py` (the inspector), `window.py`, `sheets*.py`, `data.py`; `base.py`, `glyphs.py`, `topbar.py` are dir-a's, copied; `render_all.sh` rebuilds everything with Edge headless (`--disable-lcd-text`) |

## The spec (1× logical px; every layout number even or a half so 150 % lands on whole pixels)

### The desk

| | |
|---|---|
| Header | 28 high, `surface`, 1-px lit edge; tabs (Medium 13 ink 1 on, Regular 12 ink 3 off, 2-px ink 2 underline); the density segmented control (20 high); **Rows** menu (which rows show); a solo chip when anything is soloed (`solo` at 16 % on `surface`, "1 soloed · Clear"); right: Snapshots ▾, `+` add a strip (24 × 20 well), restore (24 × 20) |
| Body | `base`; strips 4 below the header and 4 above the bottom edge |
| Left dock | the master at x 8; 8 px to the middle |
| Middle | scrolls by whole strips; pitch = strip width + 2 (2 px of `base` between strips: value, not borders) |
| Right dock | pinned: VCAs, then returns; 8 px inset; a 1-px seam `mix(base, edge, .6)` 6 px left of it |
| Overflow | edge fade 20 px to `base` at 85 %; a pill 18 high, r 9, `raised` with a 1-px lit top, the count (SemiBold 10 ink 2) and a chevron, vertically centred; scrollbar 3 px at the desk bottom, track `mix(base, edge, .3)`, thumb `ink4` |
| Cable band | in the sockets' row of every strip; only the **selected** strip's routes: the output on lane +6 (1.5 px `accent`), sends on +9 (1 px `accent` 85 %), a sidechain key on +12 (1 px `ink2`, dash 3/2), corners r 3; a reached socket lights (`accent`, or `ink2` for a key) |

### Strip widths (pitch = width + 2)

| | Compact | Wide | Large |
|---|---|---|---|
| track, bus, return, VCA, empty | **30** | **60** | **94** |
| master | 40 | 68 | 110 |

(Today's pitches are 24 / 48 / 76: Compact grows so a rotated 12-px name and two keys fit;
Wide and Large grow to hold the insert list and the numbers.)

### Rows, top to bottom (y from the strip top; the fader row is flexible)

Gaps between rows: 4 (Large 6). The fader row's travel starts 10 below its row top and ends
10 above the next row (the cap's overhang).

| Row | Compact | Wide | Large |
|---|---|---|---|
| head | 112 (number, rotated name) | 36 | 44 (+ kind line) |
| inserts | 10 (pips) | 68 (4 rows × 16 + 4) | 94 (5 rows × 18 + 4) |
| EQ | — | 22 | 36 (band dots) |
| polarity · swap · width | — | 16 | 18 (+ mono) |
| pan | 22 (r 7) | 28 (r 9) | 40 (r 11 + value) |
| M · S · arm | 30 (2 rows) | 16 | 18 |
| fader + meter | flex, min 120 | flex, min 112 | flex, min 150 |
| level | — | 16 | 18 (value · peak) |
| sends | — | 28 (2 × 14) | 60 (3 × 20) |
| output | 12 (bus dot) | 14 | 18 |
| socket | 18 | 18 | 18 |

**When the desk is short**, rows leave in this order: EQ, the polarity row, the inserts fold to
one summary line (14 high, "Comp · EQ", Regular 10 ink 3), sends, the socket, the level, the
output. The redlines sheet measures Compact, Wide and Large at 616; `strips-*` shows Wide at 440
(nothing leaves) and 288 (docked under the arrangement: EQ, polarity, sends, socket leave; the
inserts fold).

### Wide, across (x from the strip's left edge; Compact and Large in `src/strip.py`)

| Part | Spec |
|---|---|
| Body | w 60, r 3, fill `strip-` gradient (surface + 3.5 % ink 1 at the top → surface at 35 %) |
| Stripe | h 3, the user's colour via `hue_for` at 72 % (Paper 70 %), 100 % selected, 32 % muted or silenced |
| Number | Regular 10 ink 3, centred, baseline 15 (master: "M" SemiBold) |
| Name | Regular 12 ink 1 (Medium selected), baseline 30; 11 px if wider than 54; then **two lines** (baseline 15 and 29, the number leading line 1 in ink 3); then the ellipsis — never cut mid-glyph |
| Inserts | panel x 3, w 54, `mix(surface, well, .45)`, r 3, no edge; rows 16; dot r 2 at x 10 (on: ink 2; bypassed: 1-px ring ink 4); kind name Regular 10 ink 2 at x 16 (bypassed ink 4); the first empty row: a 5-px `+` ink 4; more than 4: "+2" SemiBold 9 ink 3 at the panel's bottom right; a sidechained insert: the key glyph (12) at the row's right |
| EQ | panel as the inserts, h 22; curve 1 px ink 2, fill `accent` 12 %; a flat EQ: one line ink 4 |
| Polarity row | Ø and ⇄ drawn for 16 px, centred x 11 and x 29, ink 3 **without a well at rest**; on: a well tinted `accent` 35 % with the glyph in `accent`; width knob r 6 at x 48 (arc from the minimum) |
| Pan | knob r 9 at x 30: ring `edge`, cap radial `kcap`, pointer 1.6 px ink 1; value arc r 12, 1.5 px ink 3 from the centre; automated: arc `auto` and a 2-px dot at the cap's lower right |
| M · S · arm | three keys sharing x 3–57, 2 apart (17 · 16 · 17), h 16, r 3; letters SemiBold 10 |
| Fader | groove 3 px at x 21 (`mix(well, edge, .55)`, 3 past each end); ticks at x 35 (0 dB: 3 px ink 3; others 2 px `mix(ink4, base, .25)`); cap 22 × 13 with a 1-px `edge`, `fcap` gradient, two grip lines, centre line ink 1 (the user's colour when selected) |
| Meter | 2 bars 4 px, 1 apart, at x 37, on the fader's law; grooves `mix(well, ink4, .22)`; green to −12, yellow-green to −3, amber to 0, `clip` above; peak hold 1 px (its zone's colour + 35 % ink 1; `clip` above 0); clip cell 9 × 3, 8 above the travel, `mix(well, ink4, .3)`, lit `clip` until clicked |
| Level | Regular 11 ink 2 (ink 1 selected), centred, baseline +12; clipping: the peak in SemiBold 11 `clip` on a `clip` 22 % pill |
| Sends | rows 14; the return's name Regular 10 ink 2 at x 5 (fits 29, then …); bar 2 px from x 37 to x 55, track `mix(well, ink4, .35)`, level `accent` (off: ink 4, name ink 4); the first empty row: `+` |
| Output | to a bus: the arrow glyph + the bus name Regular 10 ink 2, centred; to the master: "Master" Regular 10 ink 4 (Paper ink 3) |
| Socket | ring r 2.5, 1 px `mix(ink4, base, .2)` at the centre, 7 below the row top; lit: dot r 2.5 + halo r 4 at 25 % |

**The fader's law** (fraction of the travel from the top): +6 → 0, 0 → .16, −6 → .31,
−12 → .45, −18 → .56, −24 → .65, −36 → .79, −48 → .88, −∞ → 1. Ticks at +6, 0, −6, −12, −18,
−24, −36, −48. Numbers (Regular 9, ink 3; 0 in ink 2) only on the master (right-aligned left of
the groove) and in Large (between the cap and the meter).

**Fader and meter geometry** (groove x, cap w × h, meter x, bar w):
Compact 10, 16 × 10, 19, 2 · Wide 21, 22 × 13, 37, 4 · Large 28, 26 × 14, 64, 5 ·
master Compact 14, 18 × 10, 26, 3 · master Wide 31, 22 × 13, 47, 4 · master Large 42, 26 × 14, 70, 6.

### States (`states-*.png`)

| State | How it shows |
|---|---|
| At rest (stopped) | meter grooves only; level, sends and output still as numbers; no "−∞" anywhere |
| Playing | bars and peak holds |
| Clipping | bars into `clip`, peak hold `clip`, the clip cell lit; the level row shows the peak (+1.4) in `clip` |
| Armed | arm key filled `rec`, dark dot; the meter shows the input |
| Muted | M filled ink 2 (Paper ink 3); stripe 32 %, name ink 3, meter at 38 % strength |
| Soloed | S filled `solo` |
| Silenced by a solo | M stays a well, ringed 1 px `solo` at 45 %; stripe, name and meter as muted |
| Selected | body `mix(surface, hue, .13)` (Paper .14), stripe 100 %, name Medium, level ink 1, fader centre line in its colour, its cables drawn |
| Insert bypassed | hollow dot, name ink 4 (Compact: a hollow pip) |
| Sends | Reverb on, Delay off (name and bar ink 4) |
| Sidechain key | the key glyph on the keyed insert; its source's socket lit `ink2`, a dashed cable when the strip is selected |
| VCA | no colour of its own (two 1-px ink 3 hairlines), "VCA" SemiBold 9 ink 2 before its number; M S, fader, level, "4 strips"; no meter |
| VCA follower | "2 VCA 1" (number + tag SemiBold 9 ink 2) as one centred group; a 1-px ink 3 tick across the groove where the VCA puts the fader |
| Frozen | the snowflake (12) in `accent` at the head's right; the sound block hatched: 1-px `accent` lines at 22 %, 4 px apart, 45°; inserts ink 4/3; the mix block stays live and unchanged |
| Empty (nothing routed) | body `mix(surface, base, .55)`; number ink 4, a groove, a flat 4-px cap ghost, its socket |

### Compact and Large, what differs

- **Compact:** number Regular 10 ink 3 at baseline 14; the name Regular 12 rotated −90°,
  reading upward, its end 4 above the head's bottom, fit to head − 26 = 86 px, then …; inserts
  as pips (r 1.5, 5 apart, ink 3; bypassed a 0.75-px ring); M and S keys 13 × 14 (SemiBold 9);
  **arm as a dot alone at rest** (ink 4, no well), a 28 × 14 `rec` key when armed; no level,
  sends or output text — a bus is a 2.5-px dot of the bus's colour; the rest in the hint line.
- **Large:** name Regular 13 at 31 and a kind line Regular 10 ink 3 at 42 ("Track",
  "Return · 5 in", "Bus · 4 in", "4 strips", master "Out 1–2"); inserts show unit names
  (Regular 11 ink 1, Comprimo, Aequalis); EQ with band dots (r 2, 1 px in solo/play/accent);
  polarity, swap and mono glyphs 22 wide + width knob r 7; pan value under the knob (Regular 10
  ink 3: "C", "16 R"); level row: value left (Regular 11), peak right (ink 3, `clip` above 0);
  sends on two lines (name Regular 11, value right, bar 2 px across under them); a return lists
  who feeds it ("← Clap", "+2"); the master's output row reads "−9.4 LUFS short-term".

### The inspector (246 wide at 1280; `inspector-*.png`)

| Part | Spec |
|---|---|
| Padding, columns | 12 each side; labels Regular 11 ink 3 in a 64-px column; values Regular 12 ink 1 (numbers Regular 11 ink 1, right-aligned) |
| Head | 28, `surface`, 3-px stripe in the strip's colour; number Regular 10 ink 3; name Medium 13 ink 1 (fit, then …); "Mixer strip" Regular 11 ink 3 right; the more key |
| Source | a 28-high well: the instrument glyph, its name Medium 12, its preset Regular 10 ink 3, a chevron (opens the editor) |
| Section | a hairline `mix(base, line, .55)` at the top; title at +20: open = a small chevron down (ink 4) + SemiBold 11 ink 3, its state Regular 10 ink 3 right; folded = chevron right ink 3 + Medium 11 ink 2, summary Regular 11 ink 3 right, 28 per row; content from +30 |
| Insert row | 26 high, 2 apart, `surface`, r 4; number Regular 10 ink 3 at +8; name Regular 12 ink 1 at +22, kind Regular 10 ink 3 after 6; LED at the right (r 2.5 `play` + a 4.5 halo at 18 %; bypassed a 1-px ink 4 ring, name ink 4, "· Bypassed"); a keyed insert: key glyph + the source's name Regular 10 ink 2 |
| Insert row, pointer over it | surface + 4 % ink 1 (Paper `raised`); a 2 × 3-dot grip replaces the number; the mini-view key (16 × 16) appears |
| Insert row, dragged | lifted to `raised`, offset 4 → right and 2 down, a blurred shadow (σ 3); a 2-px `accent` line where it lands |
| Mini view (a compressor) | 46 high under its row, `mix(base, surface, .55)`, three values (label Regular 10 ink 3; value Regular 13; unit Regular 10 ink 3), the reduction in `solo` with a bar from the right; stopped: 0.0 in ink 3, no bar |
| EQ | a 72-high graph (panel, curve 1 px, fill `accent` 12 %, band dots r 2), 100 / 1k / 10k Regular 9; the band table: a 6 × 2 colour mark, name Regular 10 ink 3, frequency Regular 11 ink 2, gain Regular 11 ink 1 right; "Flat" (reset) Regular 10 ink 2 at the title's right |
| Sends | rows 26; the switch: a 14 × 14 well with an `accent` dot r 3 (off: a hollow ring); name Regular 12 (fit 74); a level bar 64 wide from +100 (3 px, handle r 4); value Regular 11 right; "after the fader" at the title's right |
| Routing | Output and Input fields (24 high, r 4, a glyph, the value, its detail Regular 10 ink 3, a chevron); Channel: polarity, swap, mono as glyph + word (Regular 10), no well at rest, an `accent` 20 % pill when on — they wrap under the label when the words do not fit (Spanish) |
| Sidechain | "Key into" (the insert), "From" (the source, its colour as a 6-px square, "Pre-fader"), "Hear the key" toggle |
| Recording | Input field + a 24 × 24 arm key; Monitor segmented (Off · Auto · In); Record segmented (Before FX · After FX); Low latency toggle; the title's state in `rec` when armed |
| Automation | Mode segmented (Read · Touch · Latch · Write); one line per lane: name Regular 11 ink 2, a 56 × 12 sparkline in `auto`, the point count Regular 10 ink 3; "Show in the arrangement" Regular 11 `accent` |
| Level | Volume, Pan (bipolar bar, a centre tick), Width as label · bar (96) · value; VCA field; Safe from snapshots, Safe from solo toggles (26 × 14) |
| Freeze | a key (well, snowflake 14, Medium 11) "Freeze strip"; its effect in Regular 10 ink 3, wrapped; frozen: the state "Frozen" in `accent`, the key "Unfreeze", what was rendered and what it freed |
| Scrolling | the head stays; the body scrolls; a 3-px `ink4` thumb at the right edge |

### Type roles used (Barlow Semi Condensed, Slate's)

Medium 13 (inspector name, Large strip name is Regular 13) · Medium 12 / Regular 12 (strip
names, values, field values) · Regular 11 (levels, labels, inspector numbers) · SemiBold 11
(section titles; a clip peak) · Regular 10 (numbers, kinds, sends, outputs, details) ·
SemiBold 10 (key letters, pill counts) · Regular 9 / SemiBold 9 (scale numbers; tags: VCA,
VCA 1, "+2"). Tabular figures on.

### Tokens

Only Slate's (`dir-a/README.md`, `system.png`): `gutter base surface raised well edge lip line
ink1–4 accent play rec solo clip auto meter_lo/mid/hi shadow` and the user's colours through
`hue_for`. The mixer derives a few mixes (named here so the kit can name them):
`strip.panel = mix(surface, well, .45)` · `strip.empty = mix(surface, base, .55)` ·
`strip.selected = mix(surface, hue, .13 Night / .14 Paper)` · `fader.groove = mix(well, edge, .55)` ·
`meter.groove = mix(well, ink4, .22)` · `meter.clip_idle = mix(well, ink4, .30)` ·
`bar.track = mix(well, ink4, .35)` · `frozen.hatch = accent @ 22 %` ·
`mute.on = ink2 (Night) / ink3 (Paper)`. **No new colour.**

## What it costs to build

- Flat fills, 1-px lines, three small gradients (knob cap, fader cap, the strip's light) —
  the kit draws them today. The hatch is a clipped set of 1-px lines (or a 4-px tile).
- Rotated text for Compact names (egui's `TextShape` takes an angle) and a two-line name
  layout for Wide/Large.
- The EQ thumbnail per strip: a 50-point polyline, cached until the strip's EQ changes.
- The insert list in the strip (Wide/Large) reads data the inspector already has.
- The dragged insert row needs the kit's existing soft shadow.
- The density modes keep their three slots (`WidthMode`), with new pitches 32 / 62 / 96 and the
  rows of this sheet; the drop order changes (sends before the level).
- New words (both locales): the Rows menu, "{n} soloed · Clear", "{n} sends in", "{n} strips",
  "Bus · {n} in", "Return · {n} in", "LUFS short-term", and the inspector's
  (`src/insp_text.py` holds both languages; existing keys are reused where they exist).
- The strip glyphs (Ø, ⇄, mono, arm, key, snowflake, in/out, return) are drawn here per key
  size in Slate's grammar; the `icons` agent's approved family replaces their drawing, not their
  sizes and places.

## Found on the way (for others)

- **The top bar in Spanish collides** (`overflow-es-*`): "Archivo Editar Clip Ver Opciones
  Ayuda" runs into the Pat/Canción control — dir-a's top bar needs its Spanish fold.
- `compare-strips-1to1.png`: FL's capture shows its Wide strips about 56 px wide on a 1920 screen;
  Slate's Wide is 60 logical (90 at 150 %). At the owner's 150 % Slate's strips are physically
  wider than FL's; Compact (30) is the narrow desk.

## For the owner to judge

1. Inserts and the EQ curve **in the Wide strip** when the mixer is maximised (Studio One,
   Logic) versus FL's choice (only in the side panel): rich or busy?
2. The signal-order strip (sound → mix → route), with the meter beside the fader, versus FL's
   meter at the top.
3. Compact at 30 px (FL's is narrower) so a 12-px name and two keys fit.
4. Mute lit in grey (ink), solo in amber, the "silenced by a solo" ring.
5. Cables only for the selected strip (as FL), or a "show all" option in Rows.

## Options for the owner: the strip's keys and the routing cables

`options-keys-cables-{night,paper}.png`: drawn at 125 %, with 4× crops below the sheet. A
private comparison with FL Studio, Bitwig and Ableton is in
`research/ui/round39/des-mixer/options-keys-beside-daws.png`. The sources are
`src/options*.py`.

- **Keys.** Each target size is given in logical px, then physical px at 125 %.
  - **A, as built:** Compact 13×14 (16×18); Wide 27×18 (34×23); Large 28×18 (35×23).
  - **B, true 24×24:** Compact 24×24 (30×30), with M, S and arm stacked; Wide 24×24, with arm on a second row; Large 26×24 (33×30).
  - **C, one segmented pill 24 tall:** Wide 26×24 (33×30); Compact 26×23 (33×29), M over S; Large 43×24 (54×30). Arm becomes a 24×24 dot in the head. Polarity and swap share one quiet pill of 26×22 cells. The width knob moves to the pan row.
  - **Recommended: C.** It draws one shape where A and B draw two or three boxes, and every target is 24 px tall without B's extra rows.
- **Cables.** The example desk has a bus, a send, a sidechain key and a return.
  - **A, as built:** one band at the foot of the desk.
  - **B, curved cables:** each cable runs from an output (a dot) to an input (an arrow), under the strips, with a "Show all" toggle.
  - **C, no cables by default:** the destinations light up with an arrow and the kind of route.
  - **Recommended: C, with B on demand.** C costs nothing at rest and labels each route. A "Cables" key in the desk header opens B, and B has "Show all".

## The routing cables, designed (`options-cables-{night,paper}.png`)

The sheet shows three directions on a busy desk at 125 %, with 4× crops under it. Each
direction is shown in six states: selected with Show all off, Show all on, hover, drag to a
valid target, drag refused as a loop, and a legend (for B, the stopped state instead). The
sources are `src/optcab*.py`.

- **A, patch cables.** One cubic Bézier per cable that hangs from its jacks.
- **B, signal flow (recommended).** A, plus a gleam that travels along each cable while audio flows.
- **C, subway.** Orthogonal routes in lanes, one lane per kind of route.

**Recommendation: B.** It looks and reads like a patch bay. The glow says what is flowing and
how loud it is, and it stops when the transport stops. It costs no extra frames, because the
mixer already repaints its meters while playing. Hovering a cable reads the whole route in
one gesture.

C is the tidiest on very dense desks (40 strips or more). It could become a per-user
preference later.

### Spec for the kit builder

**Jacks**
- Every strip's foot has two jacks, 7 px above the strip's bottom edge: the **input** at
  the strip's centre −11 px and the **output** at the centre +11 px.
- Each jack is a hole of radius 4 (`edge`; Paper `mix(well, edge, .6)`) with a 1.25-px ring.
  - At rest the ring is `mix(ink4, ink3, .5)`.
  - A connected jack takes its strongest cable's colour, in this order: output, then send, then key.
  - The output jack carries a 1.5-px dot.
- **Drop target:** a halo of radius 8 in `accent` at 22 %.
- **Refused target:** the ring turns `rec`, with a short slash through the hole.

**The curve (A and B)**
- One cubic: P0 = source (sx, y), P1 = (sx, y + k), P2 = (tx, y + k), P3 = (tx, y).
- The vertical tangents at both ends make the cable hang from its jacks.
- The sag is k = min(58, 16 + 0.15·|tx − sx|), so the deepest point is 0.75·k below the jacks.
- Longer cables hang lower, so crossings stay readable without any routing.
- **While dragging,** the free end is (px, py) and P2 = (px, py + 0.55·k).
  - It ends in a plug of radius 3.5 in the cable's colour with a 1-px under ring and a 1-px highlight.
  - Every direction draws the dragged cable this way.

**The curve (C)**
- Down from the jack, across the channel, then up into the target jack, with corners of radius 4.
- Lanes are measured below the jacks: output +14, sends +26, keys +40.
- Routes of the same kind whose spans overlap (with 6 px of margin) get sub-lanes 4 px apart.
  Allocate the shortest span first (greedy interval colouring).

**Strokes**
- Under every cable, a stroke 2 px wider in a darker shade of the cable's own colour:
  `mix(col, edge, .55)` on Night, `mix(col, ink1, .28)` on Paper. It reads on any background.
- Round caps.

**Widths**
- Output: 2 px.
- Send: 1 + 1.6 · clamp((dB + 36) / 36, 0, 1). A send at −6 dB is 2.3 px; at −22 dB, 1.6 px.
- Key: 1.5 px, dashed 4/3.
- Refused: 2 px.
- A hovered route: +0.5 px.

**Colours (cable tokens)**

| Token | Night | Paper |
|---|---|---|
| `cable.out` | `accent` | `accent` |
| `cable.send` | `mix(accent, ink1, .45)` | `mix(accent, base, .38)` |
| `cable.key` (amber) | `mix(solo, rec, .25)` | `mix(solo, #ff9a2e, .40)` |
| `cable.refused` | `rec` | `rec` |
| `cable.channel` (the band under the strips) | `mix(base, gutter, .40)` | `mix(base, gutter, .40)` |

**Visibility**
- **Show all off:** only the selected strip's cables, in and out.
- **Show all on:** every cable, with the other strips' cables at 45 % opacity.
- **Hover:** the whole route from source to master is lit (for example Kick → Drum bus → Master),
  and everything else drops to 22 %. A tip names the route.
- A track's route to the master is implied (its foot says "Master").
- Strips that receive (buses and returns) draw their route to the master, so a chain reads
  end to end.

**Drag to connect**
- Starts from an output jack.
- A valid target's jack shows the halo, with the tip "Route Chords to Drum bus".
- A target that would loop turns the cable `rec`, with the tip "Refused: Kick already feeds
  Drum bus (a loop)". Releasing does nothing; no command is sent.

**B's gleam**
- A dash of 7 px every 64 px along the cable (`stroke-dasharray 7 57`), drawn in
  `mix(col, white, .6)` (Paper `.7`).
- Opacity: 0.35 + 0.65 · level, where level is the cable's signal level 0…1, taken from the meter tap already read for the strip.
- Speed: 40 + 80 · level px/s. The offset is computed from transport time (offset = t · speed mod 64),
  never accumulated per frame, so it is deterministic and costs nothing to pause.

**Frame budget**
- **Stopped, or the Cables view closed:** no gleam, no repaint for the cables. A cable repaints only when the routing,
  the hover or the selection changes.
- **Playing:** at most one repaint at the ticker's rate (the one the meters already ask for).
  The gleam never requests a frame of its own.
- **Cost:** about seven cables × two strokes plus one dashed stroke. Build each cubic once,
  flattened to about 24 segments, and cache it until the strips move.

## Cable style D, "Jack" (`options-cables-jack-{night,paper}.png`)

Real ¼-inch patch cables. Jack is a fourth style the user can pick in every theme; the
vintage theme will make it its default. The sheet uses the same busy desk as
options-cables, drawn at 125 % with 4× crops. It shows every state: selected, Show all,
hover, a drag to a valid jack, a refused drag, the settle after a drop and playing. One
panel shows the vintage look: cloth cords, nickel plugs and bakelite boots on a cream-enamel
or black panel with a brushed-metal jack rail. The sources are `src/optjack*.py`.

### Geometry (logical px)

Jacks sit on each strip's bottom edge: the input at the strip's centre −11 and the output
at the centre +11.

- **Jack:** a nut drawn as an ellipse 12 × 6, with a 10 × 4 lit top.
  - An empty jack shows its 6 × 2.8 hole.
  - A valid drop target gets a halo of radius 10 at 25 %.
  - A refused target gets a ring 14 × 7.6 in `rec`.
- **Seated plug** (it hangs below the edge, 22 long):
  - collar 5 × 4;
  - barrel 7 × 7, r 1, with three knurl lines;
  - a kind band 7 × 1.5 in the cable's colour;
  - a boot that tapers from 6.6 to 4.2 over 8.5, with three ribs (`#000` at 35 %).
- **Free plug** (while dragging, pointing up, 36 long):
  - tip 3.2 × 3, rounded;
  - insulator 1;
  - ring 3.6 × 2.2;
  - insulator 1;
  - sleeve 3.8 × 8, with a 0.6-px highlight;
  - then the seated plug's parts.
- **Cable widths:**
  - output 3.2;
  - send 2.2 + 1.2·level, where level is clamp((dB + 36) / 36, 0, 1);
  - key 2.8, with dark spiral marks (dash 1.4/3.2);
  - the dragged cable 3.2.

### Shading layers, back to front (tokens)

| Layer | Spec |
|---|---|
| `jack.shadow` (two passes) | offset (1.5, 2.5); width w + 3 at 30 % on Night, 18 % on Paper; width w + 6 at 12 % / 7 %. Colour `#000` on Night, `#1c2a38` on Paper. Hover lifts the cable: offset (3, 5) and lighter passes. |
| `jack.edge` | w + 1.5 in `mix(col, edge, .55)` on Night, `mix(col, ink1, .35)` on Paper |
| `jack.body` | w in the kind's colour: `cable.out`, `cable.send` or `cable.key` (the same tokens as A/B) |
| `jack.highlight` | 0.35·w in `mix(col, #fff, .6)`, offset (−0.3, −0.22·w), at 60 % opacity (85 % on hover) |
| `jack.metal` | a horizontal gradient: `lo #4a535d`, `hi #f1f4f7` at 32 %, `mid #a3adb7` at 60 %, `lo` |
| `jack.boot` | a horizontal gradient from `#1b2026` through `#323a43` at 35 % (Paper `#2c353e` / `#46515c`). A refused drag turns the boot `rec`. |
| `jack.nut` | `#5d6772` on Night, `#9aa5b0` on Paper; the hole is `#0b0e11` |

### Sag (a catenary)

- The sag depth is s = min(52, 12 + 0.16·span).
- The curve is y(x) = yb + s − a·(cosh((x − xm)/a) − 1).
- a is solved once per route by bisection on f(a) = a(cosh(L/2a) − 1) − s. The function
  decreases as a grows; 80 iterations are plenty.
- The curve is sampled at 28 points.
- Each end leaves its boot vertically: a quadratic runs from the boot's end (control
  point 7 px below it) to the catenary's third point.
- While dragging, the free cable is a cubic with vertical tangents: P1 = source + (0, k) and
  P2 = pointer + (0, 0.7·k), where k = min(60, 18 + 0.18·span).
- On drop, the cable becomes the catenary through the settle.

### Settle (the drop)

- The sag follows s(t) = s0·(1 + 0.12·e^(−t/90 ms)·cos(2πt/160 ms)) for 0 ≤ t < 300 ms.
  It overshoots by 12 %, rings down, and is exactly s0 and still from 300 ms on.
- The sheet draws the sag at t = 0, 80 and 160 ms (faint) and the final cable (solid).

### Budget

- Each route's mesh (the shadow and edge/body/highlight strips) is built once and cached.
  It is rebuilt only when its endpoints, its kind, its level or its width bucket change, or
  when the strips scroll.
- At rest nothing moves and nothing repaints.
- Dragging and settling repaint at the display's rate, and only the moving cable's mesh is
  rebuilt (28 points). Settling stops at 300 ms.
- While playing, B's gleam may run along D's cords. It uses the same rule: at most one
  repaint at the ticker's rate (the meters' repaint), and none when stopped.
- Plugs are drawn from a small shape set per scale (about 20 primitives each). Cache them per
  kind colour and scale.

### Vintage panel (for the vintage round)

- The panel is black (Night) or cream enamel (Paper).
- The jack rail is brushed metal: a vertical sheen gradient with 12 hair streaks.
- Labels are engraved: SemiBold 9, tracked +1.2, in cream on black or near-black on cream.
- The cords are cloth: the body plus a braid of two dash layers, 1.6/1.6, one lighter and
  one darker, offset half a period.
- Cord colours: cobalt `#38608f` for out, ivory `#cfc4a6` for send, mustard `#c8902c` for key.
