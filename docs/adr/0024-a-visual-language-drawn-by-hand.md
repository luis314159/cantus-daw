# ADR 0024 — A visual language drawn by hand, FL Studio first

**Status:** Accepted · **Date:** 2026-09-26 · **Supersedes:** the look of ADR 0009 (theme data
stays), the family rules of `research/fabfilter/family.md` §7.2 and
`research/instruments/family.md` §6 where they disagree with this ADR.

## Context

On 2026-09-25 the user tried the build and judged it plainly:

> The application looks super ugly and plain, and so do our plug-ins … the side panel of the
> mixer looks very, very ugly, the letters sometimes run into each other … I'd like to resize
> the interface's elements by dragging their edge, or move them … the text looks like loose
> words thrown down … rethink the whole style so it's at the level of the best DAWs — FL
> Studio is beautiful … the mixer can't be scrolled down easily, and many details like that
> make it hard to move quickly.

Then: *"the style I like most, by far, is FL Studio — take even more cool things from the
others; let's make something super pro"*. And, after reading how FL is built (its own vector
renderer since FL 12): *"make it as excellent as FL; it doesn't matter if we vectorize
components by hand — it must look excellent and have incredible performance … as hardcore
as FL, almost from the bare metal, but in Rust."* They also designed ten logos
(`docs/icons/cantus—Iconos.html`) and asked for them to be selectable, tinted by the theme,
with #2 as the default, and shown on a splash while loading, as FL does.

Seven research reports back this ADR, all in `research/ui/` (screenshots local and
git-ignored): **`fl-studio.md`** (the primary reference, measured, with a spec in §12),
`ableton-live.md`, `bitwig-studio.md`, `logic-studio-one.md`, `plugin-instrument-uis.md` (a
device style guide in §7), **`cantus-audit.md`** (47 defects of today's UI with file:line), and
**`rendering-performance.md`** (what a frame costs today, measured, and how to draw).

What the audit found, in one paragraph: the mixer cuts off the bottom 170 px of every strip at
720 px and cannot scroll vertically; the inspector shows the track, not the channel, and
repeats gain and pan; text collides through five repeated patterns (cells narrower than their
labels, text painted without measuring); 145 toggles are bare words; surfaces differ by
1.08:1, one gold means everything, and track colours were muted on purpose as an "anti-FL"
rule; seven type sizes with no system; two panel edges resize and nothing docks; the
instrument windows spend a third of their height on chrome. The 64-strip mixer frame costs
9.7 ms of CPU and 28 600 allocations, although 13 strips are on screen.

## Decisions

### 1. Everything visible is drawn by us, in vectors

egui stays — as input, layout and text shaping, the layer under the look, not the look. No
stock egui widget is visible any more: every button, toggle, combo, menu, tab, scrollbar,
splitter, knob, fader, meter, slot, window frame and header is a **painter of our own** in a new
component kit (`crates/cantus-ui/src/kit/`). The look is built from:

- **meshes with per-vertex colour** for gradients (`epaint::Mesh`), **one lighting recipe**
  (§4) for everything raised or sunken, soft shadows (`epaint::Shadow`), highlights;
- **hairlines snapped to physical pixels** (`(w · ppp).round() / ppp`), so 1-px lines stay crisp
  at 125 % and 175 %;
- **static richness rasterised once per pixel size and cached as a texture** (knob bodies,
  glows, icons, logos) — measured 4× cheaper than re-tessellating 256 knob bodies — and **live
  shapes** only for what moves (values, meters, curves). The hybrid is `rendering-performance.md`
  decision 2;
- **vector icons and logos** authored as SVG, rendered with `resvg` per size and cached
  (`lyon` tessellation was measured aliased and cannot do filters).

The backend stays `glow` (the exported plug-in windows are OpenGL too, so the same kit runs in
both). Where epaint cannot do an effect well (a real blur, a glow over moving content), we
write a shader of our own through `egui::PaintCallback` — measured need, not by default.

### 2. The default theme becomes "Slate", derived from FL Studio

FL's beauty is mostly its colour system (`fl-studio.md` §1, §11.1): **a hue-locked neutral
ladder** (one hue ≈ 200°, low saturation, steps only in lightness), **a handful of fully
saturated light accents used only on small things**, **pastel user colours at one lightness**,
and **text on a coloured item tinted toward that colour**. Cantus ships:

- **Slate** (dark, **the new default**), tokens as measured in `fl-studio.md` §12.1: `ink`
  `#1f282d`, `well` `#3b4145`, `strip` `#454f55`, `panel` `#4e585e`, `toolbar` `#596267`,
  `panel_raised` `#606b71`, `panel_light` `#727c81`, `text` `#eef1ef`, `text_dim` `#c4cbd2`
  where 4.5:1 is required; accents: selection lime `#c3ff5a`, highlight orange `#ffaa54`,
  focus `#3fc3ff`, mute/on LED `#8def11`, record/clip `#ff6135`, FX bulb `#ffe34f`; meters
  `#c4fc55` → `#ffdf5a` (from −6 dB) → `#ff8d3e` (from 0 dB).
- **Paper** (light), rebuilt on the same rules and the same hue (`fl-studio.md` §12.2) —
  meters stay dark inside, one strong accent.
- **Parchment** and **Ink & Gold** stay as presets, re-expressed in the new tokens.
- **Function hues** — six, one per kind of section (`plugin-instrument-uis.md` §7.2): source
  (blue), tone (teal), dynamics (amber), modulation (violet), space (rose), output (olive), each
  ≥ 4.5:1 on its deck in every theme. They colour arcs, LEDs, curves, a selected segment's
  underline — never a surface bigger than a chip. Gold becomes Cantus's brand and focus colour,
  no longer "on".
- **Track colours** return to FL's pastel band (S 25–40 %, L 58–64 % for small fills), with a
  **"safe colours" clamp** in the picker. The "anti-FL" muting rule is withdrawn.
- **Theme knobs as in FL**: Hue, Saturation, Brightness, Contrast and Text shift the whole
  ladder at once; role pickers below; the existing per-token overrides and WCAG tests stay.

### 3. Type: one condensed family, a few roles, every word in a box

- **Face: IBM Plex Sans Condensed** (Regular, Medium, SemiBold; OFL, the family Cantus already
  bundles) as the UI face — FL's condensed face is what lets labels fit; **Plex Mono** for
  numbers whose digits must not move; Plex Sans for long text (dialogs, help).
- **Roles, and only roles** (a test bans `FontId::new` outside `design.rs`):

  | Role | Size | Weight | Case | Colour | Use |
  |---|---|---|---|---|---|
  | `micro` | 8.5 | SemiBold | CAPS +0.4 | `text_dim` | captions under tiny controls |
  | `label` | 11 | Medium | CAPS +0.5 in devices, as named elsewhere | `label` | knob and fader names, strip names |
  | `body` | 12.5 | Regular | as named | `text` | slot names, list rows, menus |
  | `control` | 12 | Medium | Title | `text` | buttons, segmented, menu fields, tabs |
  | `title` | 13 | Medium | Title | `text` (context in `text_dim`) | panel and window titles |
  | `section` | 11 | SemiBold | CAPS +0.8 | `label` on a band | section bands, card titles |
  | `value` | 12 | Mono Medium | — | `text`, function hue while dragging | value pills and wells |
  | `hero` | 18 | Mono Medium | — | `text` | one hero readout per device |
  | `display` | 26 | Mono Regular | — | `text` | the transport clock |

- **Every text is painted into a box with a policy** — `Elide::End`, `Elide::Middle` (default
  for names: "GHO…ICK", as FL), `Rotate90` (strip names when a strip is narrower than 36 pt),
  or `Abbreviate` (the i18n table holds `ATK`, `RES`, `FREQ`… next to each full name, EN and ES).
  It **measures before it paints** and never paints outside the box. This removes the
  collisions at their root (`cantus-audit.md` §4.1: `widgets/knob.rs` painted unmeasured).
- **No loose words.** A word is always one of: a control's own label inside its cell, text inside
  a control, a section band's title, a legend printed once for a whole desk (Logic's mixer), or
  the left cell of a label | value row. A test lints `views/` for bare `ui.label` and
  `selectable_label` inside rows of controls.

### 4. The component kit: one lighting recipe, a closed set of parts

`crates/cantus-ui/src/kit/` holds **pure painters** — `fn paint(painter, rect, &Style, state)` —
plus thin widget wrappers that allocate, sense and call them. Every state (rest, hover, active,
disabled, focus, lit) is painted by the same function, so the gallery test (§14) can draw them
all. Sizes are logical points at 100 %; `Style` carries the palette, the function hue and a
scale `k`.

**The lighting recipe** (FL, `fl-studio.md` §1.4): light comes from above. A **raised** part has
a 1-px outline at base −20 % L, a face that runs from +6 % L at the top to −6 % L at the bottom,
a 1-px top edge at +15 % L inside, and a drop of 2 px below (base −25 % L at 60 % alpha, fading);
a **sunken** part is filled darker toward the bottom, with a 1-px inner top line at −12 % L. Lit
parts use the same recipe with the role colour as the base. Nothing else makes depth.

The parts (recipes in `fl-studio.md` §12.4 and `plugin-instrument-uis.md` §7.7–7.8):

| Part | What it is |
|---|---|
| `Cap` | the raised recipe: fader caps, step keys, toolbar keys, channel buttons |
| `Well` | the sunken recipe: fields, dropdowns, readouts, meter wells |
| `Tile` | an 18-pt list row (FX slot, browser row): ▸, name, mix ring, LED; empty = italic placeholder |
| `Led` | 11-pt lamp, rim + core; off keeps the geometry |
| `Knob` | S 28 / M 36 / L 56 / XL 80 pt ring: 270° track, value arc **from the default** in the function hue with round ends, a bevelled body (cached texture per size and theme), a pointer; modulation rings outside |
| `Fader` | a 4-pt groove with ticks every 6 dB, a 16 × 25-pt `Cap`; lime when the strip is selected |
| `Meter` | solid two-bar fill in a `Well`, three stops, 2-pt peak line; all meters of a view in **one batched mesh** |
| `Cell` | **value pill on top, the control, the label below** — one object (FL LuxeVerb); the pill takes drag, wheel and typing like the knob |
| `Segmented` | equal segments in a `Well`, 1-px dividers; the selected one lit (fill + underline in the function hue) |
| `LedButton` | an LED and a caps word in one 20-pt button — every on/off in a device |
| `MenuField` | a `Well` with the value and ▾, optional ‹ › steppers |
| `SectionBand` | a 20-pt band with a `section` title, optional power LED, dividers between sections |
| `Card` | a device section: 24-pt title bar (LED, TITLE, type selector, ⋯), body of cells, display bleeding to the edges |
| `ToolbarKey` | 28 × 28 `Cap`; active = the highlight gradient |
| `Tab`, `Splitter`, `Scrollbar` | skinned for the workspace (§5); scrollbars always visible, 6 pt |
| `Menu` | FL-style rows of 22 pt, section bands, check column, chevrons; right-click toggles a checkable item without closing |

A bare text toggle, a stock `egui::Button`, or a label painted without a box does not exist in
`views/` after the migration; the lint of §3 enforces it.

### 5. The workspace: panels that dock, resize, move and remember

- **`egui_dock`** (MIT; 0.21 supports egui 0.36 — checked by the audit) holds the main areas:
  Browser, Arrangement, Piano roll, Channel rack, Mixer, Inspector, and any device editor. Tabs
  can be dragged to another area, split, stacked or **floated as their own window**; the tree is
  serialised in `layout.rs` next to the named layouts that exist today (ADR 0012).
- **Default layout** (Bitwig's and Ableton's clarity inside FL's look): browser left, arrangement
  centre, the lower zone holding mixer / piano roll / rack as tabs, the inspector right — each
  zone with a **show/hide key** (F5 arrangement, F6 rack, F7 piano roll, F9 mixer, as FL; B
  browser, I inspector) and a toggle at its outer corner.
- **Every splitter**: drag to resize (a 1-px line lights on hover); **right-click resets** it;
  **double-click toggles** between its last two positions (FL's browser divider). Sizes are kept
  per layout.
- **Floating windows snap** to each other and to the workspace edges within 8 pt while dragged;
  Alt disables snapping (FL). **Esc** or F12 closes floating device windows.
- **Device windows dock by default and float by choice** — the opposite of FL's MDI pile-up,
  which the FL report lists under "do not copy".

### 6. The mixer: FL's strip, fit first, then scroll

- **Strip**, top to bottom (`fl-studio.md` §12.5): colour number cell 16 · name (vertical below
  36 pt of width, elided in the middle above) · meter (flexible) · mute/solo LEDs 14 · pan 22 ·
  fader (flexible) · phase/swap/width 16 · FX bulb and record 14 · sends 18. Rows are the same
  in every strip, so they line up across the desk; empty insert slots are drawn as darker wells
  (Logic).
- **A legend column** at the left edge names the rows once for the whole desk (Logic), so strips
  carry values and icons, not repeated words ("SENDS", "Mono", "Ø" in every strip today).
- **Width modes** Compact 24 / Wide 48 / Extra large 76 pt (with an FX list inside the strip),
  cycled with Ctrl+wheel over the desk, saved per layout.
- **Meter and fader share the flexible height**; a 3-dot handle sets their ratio (FL),
  double-click resets.
- **Fit first, then scroll** — better than FL, which never scrolls vertically. When the desk is
  short, rows drop in this order: width knob → phase/swap → the name shrinks to 40 → sends
  collapse into one → chip; each row family also has an on/off in the desk's ▾ menu (Ableton,
  Bitwig). If the minimum still does not fit, **the desk scrolls vertically** with the wheel and
  a visible scrollbar. Horizontally, the plain wheel over names, numbers or meters scrolls
  strips; **Shift+wheel reorders** the strip under the pointer; the wheel over a fader changes it
  only with Ctrl held (today it changes gain by accident).
- **Master and pinned strips stay in docks** (left, and returns right, ADR 0016) while the middle
  scrolls. The selected strip is scrolled into view when it is chosen elsewhere.
- **Selection** lights the fader cap and groove lime and outlines the number cell; **Colourful
  mixer** off / low / medium / high tints whole strips in their track colour (FL).
- **Only visible strips are laid out and metered** (§13).

### 7. The inspector: the selected channel's whole strip, as a signal-flow sheet

It replaces today's side panel (`views/inspector.rs`), which shows the track, not the channel,
starts the strip ~900 px down and repeats gain and pan (`cantus-audit.md` §4.7).

- **What it shows follows what was clicked last**: a strip on the desk → that channel; a track
  header → that track (its channel below); a clip or notes → them (Bitwig §4.3). The title row
  says what: **"Mixer – Keys"** (FL), the context part dimmed.
- **Order, top to bottom** (FL's inspector column, Studio One's channel editor): input row →
  **inserts as `Tile`s** (▸ name · mix ring · LED; ▸ opens a mini view of the effect — a curve or
  its three main values — without opening the editor) → the channel EQ as a display that is a
  control → sends (one row each: destination, pre/post, level) → routing and delay compensation
  readout → the output row. The strip's own fader, pan and meter sit in a column pinned at the
  bottom (Bitwig), so a channel can be mixed without the desk.
- **One row grammar**, everywhere below the tiles (Bitwig §4.1): **label | value** rows of 24 pt,
  labels right-aligned in a fixed column, every value in a `Well` (a 2-pt level bar at its left
  edge for normalised values), rows grouped under 20-pt `SectionBand`s; what does not apply is
  **dimmed, never hidden**.
- **Resizable and collapsible**: 280 pt by default, 220–420 by dragging its edge, a snap at 280,
  right-click resets, and it collapses to a 24-pt rail (I toggles it).

### 8. The device frame: effects, instruments and hosted plug-ins share one shell

One shell for Cantus's effects (ADR 0017), its instruments (ADR 0023) and the generic editor of
hosted plug-ins (ADR 0019); the exported plug-ins draw the same shell (`FxHost`, ADR 0020).

- **Header, 28 pt**, which *is* the title bar (no egui title bar): ▸ device menu and glyph ·
  **name in the device hue** · "(Insert 3 · Keys)" dimmed · a **centred preset field** (a `Well`
  of 240 pt: ☆, the name — italic with a gold dot when modified — ▾, and ‹ › outside) · A|B ·
  undo/redo filtered to this device · (instruments) **Play | Edit | Matrix** · bypass lamp · ? ·
  ⋯ · size ⤢. Dragging the header's background moves a floating window.
- **Body**: a display on top when the device has one (≥ 70 % of the height for display-first
  effects: EQ, multiband, analysers; 40–55 % for instruments), then **cards** (§4) in a grid with
  4-pt gutters on the device ground. Each card takes its **function hue**; the device hue is the
  one of its main function (Aequalis tone, Comprimo dynamics, Spatium space, Unda source…).
  **One XL knob at most** per device, for its primary parameter.
- **Footer, 24 pt**: the **help line** (the hovered control's full name · what it does · its
  gestures), chips ("Latency 2.0 ms", "Voices 5/8", "CPU 3.1 %"), an output meter, and a resize
  grip. It is the device's local hint bar (§9).
- **Instruments' bands** (what the instruments session asked for): the **macro bar** is a card of
  **up to 8 L knobs, 88 pt high**, at the bottom of the body, collapsible to a 24-pt title bar;
  the **modulation strip** is a card of source chips (20 pt, the family hue as a 3-pt bar, the tag
  as text) that collapses to its title bar and can be moved into a side column when the window is
  wider than tall; the **keyboard is not a permanent band** — it is a 56-pt card toggled with K (or
  from ⋯), **docked at the bottom of the frame when shown**, hidden by default except in Play view.
  Together these take **at most 22 %** of the frame's height when all are open (today ~33 %), and
  none when collapsed. Edit views grow the window to fit (up to the workspace) instead of
  scrolling inside a fixed one; they scroll only past that.
- **Scale per window**: 75, 90, 100, 125, 150, 175, 200 %; the corner grip resizes freely and
  snaps to the nearest step on release (Pigments); the choice is kept per device kind.
- **Pulsus II** moves into the same shell (today it has its own window).

### 9. The hint bar and the value pill

- A **hint panel in the toolbar** (FL): two lines — the project name (dim) and the hint — and a
  strip of capability icons (automatable, has a menu, warning). Every kit widget writes
  `app.hint = Hint { name, value, caps }` on hover; the value updates live while dragging; a
  meter reports the dB under the pointer.
- **A value pill** follows the pointer (12 pt to its right) **while dragging** any control — better
  than FL, which shows the value only in the hint bar.
- Devices have their own **help line** in the footer (§8). Icons always have a tooltip too; the
  hint bar is not an excuse for an unexplained icon (the FL report's "do not copy").

### 10. Moving fast

- **Scrollbars are always visible** (6 pt) and `always_scroll_the_only_direction` is on: the plain
  wheel scrolls a horizontal-only area. **Shift+wheel works** in the arrangement, piano roll and
  channel rack (today it reads a value egui has already converted, so it does nothing), and all
  three scroll vertically.
- **Zoom** by dragging a scrollbar's ends (FL), Ctrl+wheel, and zoom-to-selection (Z, exists).
- **Keyboard**: arrows move between strips; M, S, R act on the selection; Tab moves between
  controls with the focus ring; find a channel by name (Ctrl+F in the mixer); F-keys toggle
  areas (§5).
- **A drag shows where it lands** (drop targets light in the selection colour), everywhere.

### 11. Scale

- A **global UI zoom** (Ctrl +/−, 75–200 %, saved in settings) through `pixels_per_point`; every
  size in the kit is logical, hairlines are pixel-snapped, textures are rasterised per physical
  size — measured: 4K costs about +10 % CPU and stays crisp.
- Device windows add their own scale on top (§8).

### 12. Logos, splash and icons

- **The ten logos** the user designed become `crates/cantus-ui/assets/logos/01.svg … 10.svg`,
  extracted from the bundle in `docs/icons/` (its pages carry each SVG with `sc-camel-*`
  attribute names; the extraction restores them). Each colour in them is given a **role** —
  tile, wave, playhead, patina, neutral — declared in the SVG (`data-role`) with a table in code
  as a fallback.
- **Selectable** in Settings › Appearance: `logo: 1…10`, `logo_tint: Original | Theme`. **The
  default is #2 "C de onda con playhead-cruz", Original** — exactly as designed.
- **Tinted by the theme** (`rendering-performance.md` §10.2, method A): each role takes its hue
  and saturation from a theme token (tile ← the accent at low saturation, wave ← the accent,
  playhead ← the bright accent, patina ← the selection colour) and **keeps its designed
  lightness**, so the gradients keep their depth. The rewrite is done on the SVG text before
  parsing.
- **Drawn natively in vectors**: `resvg` (Apache-2.0 OR MIT, 0.48) renders each logo at the exact
  physical size it is shown at, on a worker thread, cached in memory and on disk keyed by
  (SVG, tint, px); the previous texture stays until the new one is ready. The detailed logos'
  filters cost 36 ms at 256 px and 526 ms at 1024 px (measured), so a large logo is never
  rendered on the UI thread.
- **Small sizes get a small master**: below 48 px #2's glow becomes a blur, so a simplified
  16/24/32-px master of the same design (fewer, thicker wave spokes; the playhead cross) is drawn
  on the pixel grid for the title bar, the taskbar and the small `.ico` entries.
- **The splash, like FL's**: the window **starts as the splash** — 520 × 320, no decorations,
  centred, the default logo baked at build time (1× and 2× PNG, so the first frame is instant),
  the version, and a progress line ("Opening audio…", "Scanning MIDI…", "Loading song…").
  The app becomes `enum Shell { Booting(Boot), Running(CantusApp) }`: the MIDI scan (701 ms today
  with no ports at all), settings, theme, the plug-in database and the project's parsing move to
  a thread; the audio stream and the fonts stay on the main thread, one stage per frame. When all
  is in, the same window turns into the main one (decorations on, the saved size or maximised).
  The user's chosen logo and tint are shown from the second launch on (its PNG is cached).
- **Window, taskbar and `.exe` icons** from the default logo's masters: `ViewportBuilder::with_icon`
  at run time, and a Windows resource at build time (`winresource`, MIT) with 16–256 px entries.

### 13. Performance: a budget, and a test that holds it

Measured today (`rendering-performance.md` §2, one P-core of an i7-12700H, release, 1080p):
the whole window with the 64-strip mixer is **9.7 ms** of CPU (8.2 layout + 1.5 tessellation) and
**28 600 allocations** a frame; the sends list alone is O(N³) and rebuilt every frame (2.8 ms);
the arrangement copies 18 MB a frame on the stress project; idle repaints 10 times a second.

- **Budget** (p50 / p95, CPU, that machine): mixer 64 strips with meters, arrangement and
  inspector shown **≤ 4 ms / ≤ 6 ms**, ≤ 1 500 allocations and ≤ 256 KB a frame; mixer 16 ≤ 2.5 ms;
  a device editor over the mixer ≤ 4 ms (the editor alone ≤ 1.5 ms); stress piano roll ≤ 8 ms with
  no copy of notes; 4K ≤ 4.5 ms; GPU ≤ 2 ms at 1080p on Iris Xe-class; **idle: 0 frames a second**;
  meters, playhead and animation at **≤ 60 Hz**.
- **How**: lay out and meter **only visible strips and rows**; build lists (sends, menus) **when
  they open**, not every frame; no per-frame clones or `format!`; text layouts and static meshes
  cached (`kit::cache`, keyed by asset, size, state, theme generation and ppp); **all meters of a
  view in one batched mesh** (30× cheaper, measured); **one ticker** requests repaints at ≤ 60 Hz
  while something moves and none otherwise.
- **Held by `crates/cantus-ui/tests/frame_budget.rs`**: a counting global allocator and counters
  (strips laid out, triangles, draw calls, idle repaint requests) asserted in every `cargo test`,
  deterministic across machines; the timing assertions run in release and are `#[ignore]` locally.
  A `criterion` bench tracks the trend.
- **Start-up**: the MIDI scan and the file work off the main thread (§12).

### 14. Tests that keep the look

- **The gallery**: every kit part × every state × both default themes × 1×, 1.5×, 2×, rendered
  through the software renderer (`raster.rs`) into golden images; a change to a part's look is a
  reviewed change to its golden image.
- **No overlap, no clipping**: every editor and view rendered at 75 % and 200 %, in English and
  Spanish, asserts that each text galley fits its box.
- **Contrast** (the existing WCAG test, extended): text ≥ 4.5:1 on its surface; function hues ≥
  4.5:1 on their decks; outlines and tracks ≥ 3:1.
- **One font table**, **no bare words**, **no stock egui widget in `views/`**: lints over the
  source, as tests.
- **`examples/shoot`** stays the design review, extended to every view and the whole window.

## Order of work

**Phase A — the foundation, and a look the user approves before anything else moves.** One
builder, because everything below depends on it:

1. Tokens: Slate (default) and Paper, the function hues, the theme knobs (H/S/L/contrast/text),
   the safe-colour clamp; the contrast test extended.
2. Type: Plex Sans Condensed bundled, the roles, the boxed text with its policies and the
   abbreviation table; the `FontId` lint.
3. The kit (§4): every part and state, cached layers, the batched meter mesh, the gallery and its
   golden images.
4. A **proof render** of the new look: the mixer desk, the inspector and one device (Aequalis) in
   Slate and Paper, drawn with the kit — shown to the user before Phase B.

**Phase B — the migration, in parallel** once the look is approved (worktrees, file ownership as
in rounds 16–17):

| Builder | Owns |
|---|---|
| Workspace | `egui_dock`, the default layout, splitters, snapping, F-keys, the hint bar, the value pill, the global zoom, logos, splash and icons |
| Mixer | the strip, legend column, width modes, fit-then-scroll, docks, selection, colourful mixer, culling |
| Inspector | the channel sheet, the row grammar, mini views of inserts |
| Effects | the seven effect editors and the generic hosted editor onto the shell, cards and cells |
| Views | arrangement, piano roll, channel rack: the kit, scrolling both ways, Shift+wheel, zoom |
| Performance | the frame budget test, the ticker, caches, per-frame allocations; the sends list |

**The instruments** (the eight editors and Pulsus II into the shell, §8's bands) are the
instruments session's, on the same kit, at the same time.

## Consequences

- The look is ours end to end: a new theme default (Slate, dark), a new UI face (Plex Sans
  Condensed), new tokens, and a kit that replaces every stock widget in the views. Themes users
  saved keep loading (tokens are added with defaults; ADR 0009's data model stays).
- `research/fabfilter/family.md` §7.2 (label under the knob, value replacing it on hover) and
  `research/instruments/family.md` §6's bands are superseded by §4's `Cell` and §8's shell.
- New dependencies: `egui_dock` (MIT), `resvg` / `usvg` / `tiny-skia` (Apache-2.0 OR MIT,
  BSD-3-Clause), `winresource` (MIT, build only).
- The UI gets a frame budget and a gallery of golden images as tests: a visual regression or a
  slow frame fails the build.
- What this ADR leaves open: the exact function-hue values in Slate (set by the contrast test in
  Phase A), the icon set's full drawing (authored in Phase B), and whether a custom shader is
  needed anywhere (only if a measured effect needs it).
