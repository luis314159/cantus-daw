# Cantus interface audit (September 2026)

An honest look at Cantus's own interface as it stands on branch `rounds-13-17` (with the
uncommitted round-13–17 work in the tree), against what the user said: *ugly and plain,
the mixer channel's side panel very ugly, letters running into each other, labels like
loose words, panels that cannot be resized or moved, a mixer that cannot be scrolled
down, and many small things that make moving around slow.*

Everything below was either **seen** in a render (the file is named) or **read** in the
code (file and line are named). Where a claim comes only from reading code, it says so.

---

## 0. Method

**Renders.** All at 1.0 scale through the software renderer `cantus_ui::raster`, in both
shipped themes: *Paper* (the light default) and *Ink & Gold* (dark). Folder:
`research/ui/screenshots/cantus-audit/` (git-ignored, recompressed losslessly).

| Files (`-light` / `-dark`) | What | How |
|---|---|---|
| `mixer`, `eq`, `comp`, `limiter`, `space`, `multiband`, `deesser`, `gate`, `plugin`, `plugins` | Mixer alone (1500×940), each effect editor over it, the generic plug-in editor, the plug-in manager | `cargo run -p cantus-ui --example shoot -- <dir> 1.0 "" eq comp …` |
| `wavetable`, `analog`, `fm`, `sampler`, `bass`, `organ`, `epiano`, `pulsus`, `pulsus-play` | Every instrument editor (Unda, Calor, Modulus, Exemplar, Gravis, Organum, Lamina, Pulsus Edit and Play) | same tool, instrument kinds |
| `full-pianoroll`, `full-rack`, `full-mixer`, `full-noinsp` | **The whole window** at 1500×940: transport, arrangement, inspector, central view, status bar | a scratch harness outside the repo that calls `eframe::App::ui` through `raster::shoot` (Appendix A) |
| `full-mixer-small` | Whole window at **1280×720** (a laptop) with the mixer | harness |
| `full-mixer-noarr`, `full-mixer-many` | Arrangement hidden; 30 strips | harness |
| `full-mixer-kick`, `full-mixer-native`, `full-mixer-fx`, `full-mixer-strip5` | **Tall windows (1600–2300 px)** so the whole inspector is visible: a Pulsus track, a Calor track, a strip with 3 inserts and a send, and a strip no track feeds | harness |
| `zoom-*.png` | 17 enlarged crops of the defects cited below (Appendix B) | Pillow, nearest-neighbour |

**Limits.** The renderer shows the resting state: no hover, no drag, no open menus. The
engine is the dummy one, so the desk's meters are silent (the editors are fed demo data).
Behaviour on the mouse wheel, focus and shortcuts was established by reading the code
(and egui 0.36's source where it matters).

---

## 1. Verdict

The engineering underneath is good — the widgets behave well (fine drag, double-click to
default, right-click to type a value, arrow keys), the 4-px grid is real, the themes pass
WCAG. What the user sees is a **debug-panel skin on a serious tool**: egui's default
buttons, combos and `selectable_label`s carry most of the chrome; surfaces in the light
theme differ from each other by 1.08:1; there is one accent colour (gold) for every
meaning; type runs in seven sizes with one weight step; labels are painted as free text
with no plate and no collision rule, so they land on curves, handles, buttons and each
other. On top of that, three of the four main views **cannot scroll vertically at all**,
and the mixer's side panel is not a channel panel — it is the *track's* inspector with
the clicked strip appended ~900 px down.

The user is right on every point. None of it needs a new toolkit; most of it needs a
component layer between egui and the views, a new palette and type scale, scroll areas
where there are none, and a panel system.

---

## 2. The ten worst problems, ranked by how much they hurt

1. **The mixer cannot scroll vertically, and the wheel does not scroll it horizontally
   either.** Strips need ~500 px; the desk is a `ScrollArea::horizontal()` only
   (`views/mixer.rs:247`), the fader shrinks to a floor of 144 px and stops
   (`mixer.rs:77`, `:226`), and anything below is clipped. At 1280×720 the bottom 170 px of
   every strip — gain readout, M/S/●/FX, output routing, sends — is unreachable
   (`full-mixer-small-*.png`). The wheel over a strip does nothing unless Shift is held
   (egui's `always_scroll_the_only_direction` is left `false`); the wheel over a **fader
   moves the gain** (`mixer.rs:983`, `:1606`), so trying to scroll the desk changes the mix.
   Scroll bars are invisible until hovered (egui's floating bars with dormant opacity 0;
   `design.rs:240-242` never changes it), so 24 of 30 strips can be off-screen with no hint
   (`full-mixer-many-*.png`).
2. **The mixer's side panel is the wrong panel.** Clicking a strip selects it and the
   inspector follows — but the inspector shows, top to bottom: master gain, the *track*
   name and colours, the track's gain and pan, the instrument and all its knobs, and only
   then the strip (`views/inspector.rs:15-173`). The strip starts at y≈907 in a 1600-px
   window (`full-mixer-kick-light.png`), below the fold of any normal screen; its inserts
   start at y≈1545–1730. Gain and pan appear twice (track "Channel" and "Strip"). Clicking
   a strip no track feeds shows the *previously selected* track's instrument above it
   (`app.rs:634-642` keeps `selected_track`; `full-mixer-strip5-dark.png`).
3. **Letters collide, systemically.** Five root causes, each repeated across the code:
   knob labels wider than the knob's cell (`widgets/knob.rs:169-171`: cell = diameter +
   20 px; "Osc 2 semitones" runs into "Osc 2 detune", `zoom-inspector-knob-labels-collide.png`);
   right-to-left button groups that overflow leftwards over a title ("Aequalis EQ" under the
   "Up" button, `inspector.rs:1138-1189`, `zoom-inspector-effect-title-under-buttons.png`);
   canvas labels painted with no plate or halo and no collision test (~240 raw
   `painter.text` calls: Pulsus axis vs handles, Space tail labels, sampler seam, gate
   axis); labels drawn *before* the data that then covers them (`views/fx/gate.rs:901-926`);
   fixed-fraction widths that ignore egui's item spacing (`views/fx/space.rs:584` clips the
   Output card by ~40 px).
4. **The arrangement cannot scroll vertically, and the plain wheel does nothing in it.**
   Rows are laid from the top with no offset (`views/arrangement.rs:869-881`); track 6 and
   beyond are unreachable in the default 260-px panel. Horizontal scroll reads
   `scroll.y` when Shift is held (`arrangement.rs:919`), but egui has already moved a
   Shift+wheel into `x` (egui `input_state/wheel_state.rs:125-128`), so Shift+wheel reads 0.
   The same pattern is in `piano_roll.rs:441` and `channel_rack.rs:414`; the channel rack
   has no vertical scroll either (`channel_rack.rs:383`). *Code reading; verify with a
   mouse — a trackpad's native x-scroll does work.*
5. **"Loose words."** Toggles and switches are bare text: `selectable_label` is used 145
   times as a toggle, a tab or a switch. On a strip: `M  S  ○  FX`, `Ø  ⇄  Mono`, a floating
   `C` between two unlabelled bars, `SENDS  +`, `1 in`, `·`, `PAN LAW -3` with no unit
   (`mixer.rs:761-775`, `:879-929`, `:1143-1150`, `zoom-mixer-strip-*.png`). In the editors:
   style rows `Clean  Punch*  Opto  Bus  Vocal…` and run-on prose in gold ("Custom (from
   Punch) · peak detector · 40 % feedback · release as set, a second stage holds 35 %",
   `views/comp_editor.rs:1017-1058`; `views/fx/gate.rs:596-615`).
6. **Dull colour, no depth.** Paper's canvas, panel and raised card are `#ffffff`,
   `#f4f5f7`, `#eaecf0` (1.09:1 and 1.08:1); strips are the same colour as the desk; meter
   wells and fader grooves are the same grey `#d2d7df`; knobs are a flat disc. One gold
   does headings, selection, toggles, knob arcs, meters' upper range and focus. Track
   colours are tinted to 7–14 % in the arrangement and 42 % on mixer plates — on purpose,
   as an anti-FL rule (`theme.rs:564-572`, `mixer.rs:695-697`), which is exactly what the
   user, who calls FL beautiful, is reacting to.
7. **Type without a system.** Seven sizes (10, 10.5, 11, 12, 13, 16, 19 px) in three
   parallel mechanisms — egui `TextStyle`s, `design::paint::*` `FontId`s, and 8 ad-hoc
   `FontId::new` calls — one weight step (Regular/SemiBold), no letter-spacing on the 12
   upper-case captions. One 96-px strip uses six text styles. Knob labels are 10 px dim;
   values 10 px mono. Headings are inconsistent (gold 16 px, black 16 px, grey 10.5 px caps)
   within one panel.
8. **Panels cannot be moved, only two can be resized.** The window is fixed: transport
   top, status bottom, inspector right (resizable, 240 px min), arrangement top (resizable,
   120 px min), and one central view that is *either* piano roll *or* channel rack *or*
   mixer (`app.rs:3777-3838`). Track header width (220 px, `arrangement.rs:111`) and the
   velocity lane (70 px, `piano_roll.rs:27`) are fixed; mixer strip width is three presets.
   Editors are floating `egui::Window`s that land on top of the desk. No docking, no
   collapse, no second view side by side, no shortcut to toggle panels.
9. **Editor frames waste the window and bury the sound.** Every instrument window spends
   about 200–230 px (a third of the window) on a band of 20–30 tiny chips ("ENV1• 0",
   "LFO3• 0"), eight macro knobs, a 48-px keyboard and a footer (`views/inst/frame.rs:117-125`);
   the Edit views scroll inside the fixed window (`frame.rs:129`; Pulsus Edit is cut at
   SHAPE, `pulsus-dark.png`). The generic plug-in editor opens 980×620 for three knobs
   (`views/fx/mod.rs:435`, `plugin-light.png`). The inspector's native-instrument strip
   shows four knobs all labelled just "Macro" (`frame.rs:791` uses `param_knob`, not the
   named `param_knob_as`).
10. **Chrome that looks unfinished.** Menus render as five boxed buttons (`File Edit Clip
    View Help`, `views/transport.rs:44-154` uses `menu_button` in a plain row, not a
    `MenuBar`); the master meter is an empty white box that reads as a text field in the
    light theme (`transport.rs:342-362` fills it with `ink` = `#ffffff`); "Arrangement"
    touches the window's left edge (`app.rs:3822`: the Canvas frame has no margin); the
    view switcher is three text labels, then the view repeats its own name ("Mixer" tab,
    then "Mixer" in gold); the piano-roll keyboard vanishes (black keys `#14161c` on
    `#171a22` in dark, 1.04:1; white keys `#ffffff` on `#ffffff` in light;
    `piano_roll.rs:890-906`, `zoom-pianoroll-keys-dark.png`); the clip-map band in each
    track header runs under the track's name (`arrangement.rs:478-497`,
    `zoom-track-headers.png`).

---

## 3. Every defect, ranked

Severity: **C** critical (blocks work or changes the mix by accident), **H** high (seen
at once, hurts the look everywhere), **M** medium, **L** low.

| # | Sev | Defect | Where (file:line) | Evidence |
|---|---|---|---|---|
| 1 | C | Mixer: no vertical scroll; strips clipped below ~555 px of central height | `views/mixer.rs:213-263`, `:77-82`, `:226`, `:247` | `full-mixer-small-*.png` |
| 2 | C | Wheel over a mixer fader changes gain; wheel over the rest of the strip does nothing (needs Shift) | `mixer.rs:983`, `:1606`; `widgets/fader.rs:372-378` (does not consume the delta); `design.rs:224-253` (no `always_scroll_the_only_direction`) | code |
| 3 | C | Arrangement: no vertical scroll; tracks past the panel's height unreachable | `views/arrangement.rs:382-393`, `:869-881` | code; 4 tracks fit in the default 260 px |
| 4 | C | Shift+wheel horizontal scroll reads `scroll.y`, which egui has zeroed → dead in arrangement, piano roll, channel rack | `arrangement.rs:919`, `piano_roll.rs:441`, `channel_rack.rs:414`; egui `wheel_state.rs:125-128` | code (verify on hardware) |
| 5 | H | Channel rack: no vertical scroll (rows from the top, no offset) | `views/channel_rack.rs:374-383` | code |
| 6 | H | Inspector shows the track, not the strip; strip section ~900 px down; inserts ~1550+ | `views/inspector.rs:15-173`, `:595-763`, `:1097-1259` | `full-mixer-kick-light.png`, `full-mixer-fx-light.png` |
| 7 | H | Clicking a strip no track feeds keeps the old track's instrument on top | `app.rs:634-642`; `inspector.rs:49-116` | `full-mixer-strip5-dark.png` |
| 8 | H | Gain/pan shown twice (track channel and strip), two faders that look identical | `inspector.rs:490-541` vs `:619-684` | `full-mixer-kick-light.png` |
| 9 | H | Effect card titles covered by Up/Down/Remove | `inspector.rs:1138-1189` | `zoom-inspector-effect-title-under-buttons.png` |
| 10 | H | Knob labels overflow the knob's 56/64-px cell and collide | `widgets/knob.rs:169-171`, `:331-338`; `inspector.rs:319-324` (4-px gap) | `zoom-inspector-knob-labels-collide.png`; `zoom-comp-header-rows.png` ("Second stage" / "Release curve") |
| 11 | H | Scroll bars invisible at rest everywhere (egui floating, dormant opacity 0) | `design.rs:240-242` | every render |
| 12 | H | Switches as bare text: `M S ○ FX`, `Ø ⇄ Mono`, density `Wave Compact Normal Wide` | `mixer.rs:761-775`, `:930-970`, `:1007-1054`, `:527-547` | `zoom-mixer-strip-*.png` |
| 13 | H | Pan and width bars unlabelled; the readout `C` floats between them and belongs to neither visibly | `mixer.rs:898-929` | `zoom-mixer-strip-top.png` |
| 14 | H | Light theme surfaces indistinguishable (1.08–1.09:1); strip = desk colour | `theme.rs:305-334`; `mixer.rs:801-818` | `mixer-light.png` |
| 15 | H | One accent for every meaning | `theme.rs:424-476` | all |
| 16 | H | Track colour deliberately muted (7–14 % lanes, 42 % plates) | `arrangement.rs:470-473`; `mixer.rs:712-715`; `theme.rs:564-572` | `full-pianoroll-light.png` |
| 17 | H | Seven type sizes, three font mechanisms, ad-hoc FontIds | `design.rs:121-140`, `:191-214`; `transport.rs:369`; `pulsus_editor_display.rs:343`; `inst/fm_*.rs` (6) | — |
| 18 | H | Menus drawn as boxed buttons | `transport.rs:44-154` | `zoom-transport.png` |
| 19 | H | Instrument bottom band ~a third of the window | `inst/frame.rs:117-125`, `:374-503`, `:671-744` | `zoom-instrument-bottom-band.png` |
| 20 | H | Only one of piano roll / rack / mixer visible; no docking | `app.rs:3825-3838` | all `full-*` |
| 21 | M | Canvas labels over handles and axes (Pulsus: "100" under the T handle, A/H stacked on the frame) | `views/pulsus_editor_display.rs:384-416` | `zoom-pulsus-*.png` |
| 22 | M | Space tail labels over the pre-delay bar and curves ("+0.0 dB", "pre 125 ms", "early 58.1 ms", "125 Hz") | `views/fx/space_tail.rs:529-563` | `zoom-space-tail-labels.png` |
| 23 | M | Space editor Output card clipped ~40 px (widths ignore item spacing) | `views/fx/space.rs:580-593` | `zoom-space-output-card-clipped.png` |
| 24 | M | Gate axis labels painted before the fills that cover them; "−4 s" under hit bars | `views/fx/gate.rs:901-926` | `zoom-gate-axis-labels.png` |
| 25 | M | Sampler "seam −27.5 dB" crossed by the seam line and the curve | `views/inst/sampler_wave.rs:486-493` | `zoom-sampler-seam-label.png` |
| 26 | M | EQ band dots and M/S badges overlap (band 6 and an "S") | `views/eq_editor_display.rs:1063-1064` and the badge placement around it | `zoom-eq-band-markers.png` |
| 27 | M | Inspector EQ curve "10.0 kHz" label runs past the curve's right edge | `inspector.rs:884-893` | `full-mixer-kick-light.png` |
| 28 | M | Section captions inside `ui.horizontal` get an 8-px *horizontal* `add_space`, so "PAD", "EQUALISER", "SENDS" sit 8 px right of the column | `inspector.rs:1028-1035` called at `:274`, `:770`, `:925` | `full-mixer-kick-light.png` |
| 29 | M | Heading styles differ within one panel (gold heading, black heading for the track name, grey caps) | `inspector.rs:58`, `:1023-1035` | `full-mixer-kick-light.png` |
| 30 | M | Track name overlapped by the header's clip-map band | `arrangement.rs:478-497` vs `:520-536` | `zoom-track-headers.png` |
| 31 | M | Piano-roll keyboard unreadable (keys vanish into the background) | `piano_roll.rs:890-906`; `theme.rs:287-290`, `:324-327` | `zoom-pianoroll-keys-dark.png` |
| 32 | M | Master meter looks like an empty text field (light) | `transport.rs:342-362` | `zoom-transport.png` |
| 33 | M | "Arrangement" and its toolbar flush against the window edge | `app.rs:3822` | `zoom-transport.png` |
| 34 | M | Duplicate titles: "Piano roll" tab then "Piano roll Motif"; "Mixer" tab then "Mixer 16 strips" | `transport.rs:302-313`; `mixer.rs:512`; piano roll toolbar | `full-*.png` |
| 35 | M | Mixer strip has no insert slots (only "1 in · 3 fx"); inserts only in the inspector, far down | `mixer.rs:879-892` | `full-mixer-fx-light.png` |
| 36 | M | Middle scroll area cuts a strip in half at the right dock; no count of hidden strips | `mixer.rs:246-257` | `full-mixer-*.png` (strip 6) |
| 37 | M | Generic plug-in editor: 980×620 window, 3 knobs, 80 % empty | `views/fx/mod.rs:432-438` | `plugin-light.png` |
| 38 | M | Instrument Edit views scroll inside a fixed window; Pulsus Edit cut at SHAPE | `inst/frame.rs:129`; `views/pulsus_editor.rs:208-214` | `pulsus-dark.png` |
| 39 | M | Inspector macros unnamed ("Macro" ×4; wraps 3+1) | `inst/frame.rs:780-794` | `full-mixer-native-dark.png` |
| 40 | M | Modulation chips cryptic ("ENV1• 0": tag, a bullet for per-voice scope, a count) | `inst/frame.rs:455-503` (`:466-471`) | `zoom-instrument-bottom-band.png` |
| 41 | M | Editor toolbars: 10–15 text toggles in a row, same look as actions ("Freeze", "Grab" beside "In", "Out") | `views/eq_editor.rs:1114+`; `comp_editor.rs:1150+`; `fx/gate.rs:596+`; `fx/limiter.rs:1955+` | `eq-light.png`, `comp-dark.png` |
| 42 | M | Prose in the interface: style descriptions, empty states as sentences, "feedback: really 2.6:1" | `comp_editor.rs:1053-1058`; `gate.rs:610-614`; `inspector.rs:960-966` | `comp-dark.png`, `gate-light.png`, `full-mixer-kick-light.png` |
| 43 | L | Send level labelled "Gain" but shown in % | `inspector.rs:1006-1010` | `full-mixer-fx-light.png` |
| 44 | L | "PAN LAW −3" without a unit | `mixer.rs:1663+` | `mixer-light.png` |
| 45 | L | Two different mini-controls per rack row (a slider-fader and a bar) | `views/channel_rack.rs:879`, `:891` | `zoom-rack-rows.png` |
| 46 | L | Persistent red "Audio device unavailable…" on the status bar | `app.rs` status | all `full-*` |
| 47 | L | Editor windows use egui's plain title bar, 1-px border and cascade position | `inst/frame.rs:106-112`; `fx/mod.rs:432-438` | all editor renders |

---

## 4. Findings, complaint by complaint

### 4.1 Letters that run into each other

All the collisions seen trace back to five patterns:

| Pattern | Where it bites | Root cause | General fix |
|---|---|---|---|
| **A cell narrower than its label** | Knob rows in the inspector, comp engine row, EQ/multiband rows | `Knob` allocates `diameter + 20` px wide (`knob.rs:170`) and paints the label centred with `painter.text` (`knob.rs:332-338`), no measuring, no elision. With a 4-px gap (`inspector.rs:321`), a 78-px label on a 56-px cell overlaps both neighbours. | The knob measures its label and value and allocates `max(diameter, label, value) + gap`; or the knob takes a fixed column width from its grid and elides with a tooltip. |
| **Right-to-left groups that overflow** | Effect cards' `Up Down Remove` over the name; the same pattern in send cards and section headers with a button | `ui.horizontal(… label …; with_layout(right_to_left, buttons))` (`inspector.rs:1138-1189`): when the buttons are wider than what is left, egui places them past the left edge of their region, over the label. | A title row component: fixed icon buttons on the right, the title in the remaining width with `.truncate()`. Icons instead of words (↑ ↓ ×) cut the width from ~154 to ~72 px. |
| **Canvas labels with no plate** | Pulsus envelope, Space tail, sampler seam, gate history, EQ markers, multiband band labels | 243 raw `painter.text` calls. Labels sit on data at fixed offsets (`at + vec2(8, -8)`, `pulsus_editor_display.rs:410-416`), with no background, no halo and no test against other labels or the curve. | One `plot_label()` helper: text on a 2-px-padded plate of the canvas colour at 85 % (or a 1-px halo), with a per-plot list of occupied rects; a label that would overlap moves or hides. Axis labels live *outside* the plot area. |
| **Draw order** | Gate history axis | Axis labels painted first, fills painted over them (`gate.rs:901-926`). | Paint labels last, on plates. |
| **Fractions that ignore spacing** | Space card row | `widths = [w*.29, w*.17, w*.33, w*.21 - 3·gap]` plus `add_space(gap)` ×3, but egui also adds `item_spacing.x` (8 px) between each of the 7 items (`space.rs:582-592`) → the row is ~48 px too wide and the last card is clipped. | A row helper that subtracts `(n-1)·spacing`, or `ui.columns`, or set `item_spacing.x = 0` inside the row. |

### 4.2 "Loose words"

What makes text read as thrown-down words is the absence of a *container* that says
"these belong together, and this one is a control":

- **Switches that look like text.** `selectable_label` draws nothing at rest — no frame,
  no fill, no icon. `M`, `S`, `FX`, `Ø`, `⇄`, `Mono` (mixer strip), `Wave Compact Normal
  Wide` (mixer toolbar), `Play Edit Matrix` (instrument header), `Clean Punch Opto…`
  (style rows), `Piano roll Channel rack Mixer` (view tabs) all render as words with
  gaps. 145 call sites.
- **Values without their label, labels without their value.** On a strip, the pan bar
  and width bar have no label; the pan readout `C` is centred on its own line between
  them (`mixer.rs:910-914`). `1 in` sits alone under the plate. `SENDS` is followed, 70
  px later, by a `+` in a box.
- **Captions that do not attach.** In the inspector, `Gain` (10.5 px, grey, left) and
  `+0.0 dB` (13 px, mono, right) are on one line, the fader under them, the scale under
  that — four rows for one control, each row a different style.
- **Prose.** Style descriptions and empty states are full sentences in 10.5 px
  (`"No sends. A send is a post-fader copy into another strip — a reverb return, a
  parallel compressor."`), and the compressor puts a 90-character status sentence in
  gold on its style row. A first-rank plug-in shows a *state* ("Custom · from Punch")
  and keeps the sentence in the tooltip.

What cures it (to be designed from the other agents' research): labelled **units**
(label + control + value in one framed cell, fixed width, one alignment), **segmented
controls** for choices, **icon toggles with a lit state** for M/S/Ø/FX, section **cards
with a title bar**, and a hard rule that the interface never shows a sentence where a
tooltip will do.

### 4.3 Type sizes and weights

Inventory of what the code asks for (px at 100 %):

| Size | Face | Used for |
|---|---|---|
| 10 | Plex Sans | `paint::caption` — knob labels, lane captions, plot captions, chip text; `transport.rs:369` "CLIP" |
| 10 | Plex Mono | `paint::ruler` — knob values, rulers, fader scales, strip numbers, axis labels |
| 10.5 | Plex Sans | `caption` — section caps, switch letters, "1 in", status hints |
| 10.5 | Plex Mono | `mono-small` — `C`, `-∞`, `L M H` |
| 11 | Plex Sans | `Small`, `paint::label` — strip names, clip names, note names |
| 11 | Plex Mono | ad hoc in `inst/fm_env.rs:447`, `fm_graph.rs:443`, `fm_matrix.rs:34`, `fm_ops.rs:50` |
| 12 | Plex Mono / SemiBold | `Monospace`; ad hoc `fm_ops.rs:217` |
| 13 | Plex Sans | `Body`, `Button` — every button, combo, menu |
| 13 | Plex SemiBold | `strong`; ad hoc `pulsus_editor_display.rs:343`, `fm_graph.rs:358` |
| 13 | Plex Mono Medium | `readout-small` — dB readouts, clock; `paint::empty_state` |
| 16 | Plex SemiBold | `Heading` — panel titles |
| 19 | Plex Mono Medium | `readout` — the transport position |

Problems: seven sizes where three or four would do; 10 and 10.5 side by side (they read as
a mistake, not a step); only two sans weights, so hierarchy falls to colour (gold vs
grey), which is also the state colour; labels at 10 px in `text_dim` are the faintest
thing on screen *and* the most numerous; no letter-spacing on the 12 upper-case captions
(`to_uppercase()` with no `extra_letter_spacing` anywhere) so caps look cramped; numbers
are mono (good) but at 10 px mono next to 10.5 px sans the baselines do not match. A
single 96-px mixer strip uses six styles.

### 4.4 Alignment and spacing

- The 4-px grid is honoured in constants, but **columns are not**: in the inspector, text
  starts at x=1209, section captions inside horizontal rows at x=1217 (the `add_space` bug,
  #28), knob rows centre their labels, sliders start at 1209 and end at the panel edge,
  the EQ curve's label runs past it. There is no left rail that everything hangs from.
- **Vertical rhythm is uniform, so nothing groups.** `item_spacing.y = 4` everywhere; a
  section break is an `add_space(8)` plus a caption, or a `ui.separator()` (134 call
  sites). Related rows and unrelated rows are 4 px apart alike.
- **Strips centre everything** (`vertical_centered`, `mixer.rs:825`), so readouts, captions
  and switches float at different x in each row.
- **Editors lay knobs out with `horizontal_wrapped`**, so a row of 4 wraps 3+1 in a narrow
  inspector (`full-mixer-native-dark.png`) and knob groups have ragged gaps (comp editor
  "Hold"/"Lookahead" separate from "Attack"/"Release" by a hole).

### 4.5 Hierarchy

Every region leads with a 16-px gold word ("Mixer", "Master", "Channel", "Instrument",
"Strip 2", "Effects"), then 10–11 px grey for everything else. There is no middle level —
no card title bar, no group header with a background — so the eye gets "big gold word,
then a field of small grey". The primary controls (a strip's fader, an effect's main
display) do not dominate: the mixer strip spends as much height on pan/width/EQ bars and
captions as on the fader. In the editors, the toolbar rows (2–3 rows of text toggles)
compete with the display.

### 4.6 Colour and depth (measured)

| Token (Paper / Ink & Gold) | Value | Contrast with its neighbour |
|---|---|---|
| canvas `ink` | `#ffffff` / `#101218` | canvas↔panel 1.09:1 / 1.08:1 |
| `panel` (transport, inspector, desk, strips) | `#f4f5f7` / `#171a22` | panel↔raised 1.08:1 / 1.11:1 |
| `panel_raised` (selected strip, cards) | `#eaecf0` / `#1f232d` | — |
| `grid_strong` (meter wells, grooves in light) | `#d2d7df` | vs panel 1.33:1 |
| accent `gold` | `#956608` / `#d4a24c` | 4.60:1 on panel (light) |
| `text_dim` | `#5b6270` / `#8f939e` | 5.6:1 — legible, but everything secondary is this one grey |
| Keys plate on the desk (tinted 42 %) | `#eccaa8` on `#f4f5f7` | 1.42:1 |

Reading: the light theme is a single sheet of near-white; only the tinted strip plates
and the gold carry any colour. The dark theme is better (the meter well `#101218` sinks
into the `#2f3442` bar groove), but the same monochrome logic applies. Depth: `Elevation`
forbids shadows inside the layout and has three flat fills (`design.rs:256-287`); no
gradient, no bevel, no inner shadow on wells, no specular on caps. The only relief in the
app is the channel rack's step buttons (a lighter top half), which is why the rack is the
best-looking view today (`zoom-rack-rows.png`).

Colour carries almost no meaning: gold is the heading colour, the selection colour, the
"on" colour of every toggle, the knob arc, the meter's upper range and the EQ curve.
Mute is red text, solo is gold text — on a white row, 10.5 px.

### 4.7 The mixer's side panel (`views/inspector.rs`)

Anatomy, measured in `full-mixer-kick-light.png` (panel x=1197–1500, 304 px wide):

| y (px) | Block | Notes |
|---|---|---|
| 55–150 | **Master** (gold 16) · Gain · horizontal fader with −∞…+12 scale | always first, whatever you clicked |
| 165–210 | Track name (**black** 16) · `Color` + 8 swatches | a different heading style |
| 237–400 | **Channel** (gold) · Gain fader · Pan bar · `Mixer [Strip 2 ▾]` | the *track's* level, before the strip |
| 427–870 | **Instrument** (gold) + combo · PADS chips · `Kick · Kick` · `Open Pulsus editor` · PAD · Voice · Choke · Mute · 9 knobs in rows of 4 | for a Firmus track, 17 knobs (`full-pianoroll-light.png` is cut at "OUTPUT") |
| 907–1100 | **Strip 2** (gold) · Gain · Pan · Width · `Ø Mono M S ⇄` | what the user actually clicked |
| 1120–1470 | EQUALISER · curve · Low shelf / Bell / High shelf × 3 knobs | label spills past the curve |
| 1470–1520 | SENDS · `+ Add send` · a sentence | |
| 1545– | **Effects** 0/10 · `+ Add effect` · cards (each ~200 px: title row, preview, "Open the editor", a Mix knob) | titles under buttons (#9) |

Why it looks "very, very ugly": it is one 1500-px column of egui defaults with four
heading styles, two identical fader blocks, a long knob wall, sentences as empty states,
bare-text switches, and cards only at the very bottom. It is also *about the wrong
thing*: it answers "what is the selected track" when the user asked "what is this
channel". FL's mixer shows the selected channel's **inserts and sends** beside the desk
in the desk's own visual language; Cantus shows them 1500 px away in a different one.

### 4.8 Can the mixer scroll?

**Vertically: no.** `show()` (`mixer.rs:213-263`) is `toolbar` + `horizontal_top` with a
`ScrollArea::horizontal()` in the middle. The fader's height is `available_height −
STRIP_CHROME_H (384)` clamped to 144…440 (`:226`). A strip at the minimum fader measures
~500 px (`full-mixer-light.png`: frame 357 → 858). With the default layout (transport
~34, arrangement 260, tabs + toolbar ~54, status ~24) the desk needs a window at least
~875 px tall; on a 720-px or 768-px laptop the bottom 150–200 px of every strip is
silently clipped (`full-mixer-small-*.png` shows the fader cut at −30 dB; readout, M/S,
routing and sends gone). The only way to reach them is to hide or shrink the arrangement.

**Horizontally: only with Shift or the invisible bar.** A mouse wheel gives `y`; a
horizontal-only area uses `x` unless `style.always_scroll_the_only_direction` is set
(egui default `false`, not set in `design.rs`). With Shift, egui converts the wheel to
`x` and the desk scrolls — unless the pointer is over a fader, which also reacts (it reads
`smooth_scroll_delta.y`, 0 after conversion, so Shift+wheel is safe; plain wheel moves the
fader). Scroll bars are egui's floating kind with dormant opacity 0: invisible until the
pointer is over them. With 30 strips, 24 are off-screen and nothing says so
(`full-mixer-many-dark.png`). There is no "scroll the selected strip into view" (no
`scroll_to_rect` anywhere in the UI crate).

### 4.9 What can be resized or moved today

| Region | Resize | Move / dock | Collapse | Remembered |
|---|---|---|---|---|
| Transport (top) | no | no | no | — |
| Status bar (bottom) | no | no | no | — |
| Inspector (right) | **yes**, drag edge, min 240, default 304 (`app.rs:3794-3811`) | no | only via View → Layout checkbox | yes, named layouts (`layout.rs`) |
| Arrangement (top) | **yes**, drag edge, min 120, default 260 (`app.rs:3813-3824`) | no | View → Layout | yes |
| Central view | takes the rest | no; *one* of piano roll / rack / mixer (tabs, F6/F7/F9) | no | view remembered per layout |
| Track header column | no (220 px, `arrangement.rs:111`) | — | — | — |
| Track heights | yes, per-row divider drag + presets menu | reorder by dragging the header | — | in project |
| Piano-roll velocity lane | no (70 px, `piano_roll.rs:27`) | — | — | — |
| Mixer strip width | 3 presets (`mixer.rs:61-67`) | strips: Alt+←/→, dock left/middle/right via menu | groups fold | project/settings |
| Effect / instrument editors | yes (egui Window, min 560×400 … 760×560) | free-floating inside the main window | no (`collapsible(false)`) | position per session only |

So the user's two asks — *drag an edge to resize* and *move things inside the interface* —
are met for two edges and nothing else.

### 4.10 Navigation friction

| View | Plain wheel | Shift+wheel | Ctrl+wheel | Vertical scroll | Notes |
|---|---|---|---|---|---|
| Arrangement | nothing (reads `x`) | nothing (reads `y`, zeroed) | zoom time | **none** | Z = zoom to selection; no vertical zoom |
| Piano roll | rows up/down | nothing (reads `y`) | zoom time | yes (wheel) | no key-height zoom by wheel |
| Channel rack | nothing | nothing | zoom steps + rows | **none** | "fit" resets on grid change |
| Mixer | fader under pointer moves; else nothing | desk scrolls | nothing | **none** | bar invisible |
| Inspector | scrolls | — | — | yes | bar invisible; 1000+ px of content |
| Editors | knob/fader under pointer may take it | — | varies | Edit views scroll inside the window | windows cover the desk |

Plus: no shortcut to show/hide the inspector or arrangement, or to cycle focus between
panels; one central view at a time, so arranging and mixing means switching; selecting a
track does not bring its strip into view; editors open at egui's cascade position over
the strips they belong to; `View → Layout` is three clicks deep. What is good and should
stay: FL's F6/F7/F9, Ctrl+↑/↓ for tracks, Alt+←/→ for strips, the tool digits, the
visible focus ring, the keymap editor.

### 4.11 Effect editors (Aequalis, Comprimo, Terminus, Spatium, Multiplex, Sibilus, Porta)

The displays are strong — real analysers, curves you drag, history views — and that is
the part to keep. Around them:

- **Header** (`fx/mod.rs:459-501`): colour chip, strip name, `Presets` (egui button),
  `A` `B` (text toggles), `Copy`, and far right `Active`. Plain egui controls, no bar, no
  preset name shown.
- **Toolbars**: EQ has two rows (Zero latency / Natural phase / Linear phase / latency
  pill / Analyser In Out Key Freeze Grab Collisions / High · Medium · 90 dB · 4.5 dB/oct /
  Key strip / Match / Piano / ±18 dB) — 20 controls, one look for toggles, actions and
  menus. Comprimo has three rows of the same.
- **Labels on the displays** collide (#21–#26).
- **Knob rows**: 36-px knobs with 10-px labels and values, in groups separated by a 10.5-px
  caps caption ("DETECT", "TIME", "KEY" in Porta) — the knobs are the least legible part of
  each editor.
- **Frames**: egui's title bar; windows open at 980×620–1180×860 over the mixer; the
  generic plug-in editor wastes 80 % of its window.

### 4.12 Instrument editors (Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Pulsus)

Judged with the same eye, the user's suspicion is right: they are the effects' look, with
more chrome.

- **Shared frame** (`inst/frame.rs`): header (chip, track name, instrument name in gold,
  `Presets`, `A B Copy`, `Play Edit Matrix` as text toggles) — the view switch looks
  like three words, not tabs (`frame.rs:168-176`).
- **Bottom band** (`frame.rs:117-125`), measured in `wavetable-dark.png`: modulation strip
  y 450–495 (two rows, 24 chips), macro bar 500–600 (chips M1–M8 above eight 36-px knobs,
  then "→ Position", "→ Cutoff" captions), keyboard 607–655 (48 px, C2–C7), footer
  660–680 ("Voices 4 / 16"): **~230 of the window's ~626 content px (37 %)**, the same in
  every instrument whatever its needs (Organum shows 4 chips but keeps the height). The
  chip text `ENV1• 0` is tag + a bullet meaning "per-voice" + a connection count
  (`frame.rs:466-471`) — unreadable without the source.
- **Macros** are labelled twice (chip `M3` above knob `Macro 3`); named ones say what they
  drive in a third line.
- **Edit views** scroll inside the fixed window (`frame.rs:129`) with an invisible bar;
  Pulsus Edit is cut at "SHAPE" (`pulsus-dark.png`).
- **Per instrument**: the Play views are the best screens in Cantus (Unda's 3-D wavetable,
  Calor's filter curve and voice cards, Gravis's fretboard, Lamina's pickup field,
  Organum's drawbars and rotor). They are let down by the frame and the finish: Calor's
  sixteen voice cards are thin outlined boxes with 10-px contents; Modulus's operator
  graph sits in a black box whose spare operators E–H are bare letters in circles;
  Exemplar's zone map is one flat tan with 9-px zone names; Pulsus's pad grid starts with
  a half-empty row (Cowbell, Claves) and its Edit graph has the collisions of #21.
- **Inspector strip** for a native instrument (`frame.rs:780-794`): a 190×60 picture,
  `Open editor`, and four knobs labelled "Macro" (not their names), wrapping 3+1.

---

## 5. The building blocks

### 5.1 The design system

**Keep (reusable as is):**

- `design::space` — the 4-px grid and its constants; the discipline is real.
- The named-style mechanism (`design::caption()` etc.) and the test that every named style
  exists (`design.rs:355-372`); it is the right place for a real type scale.
- `paint_focus_ring` (`design.rs:327-349`) — visible keyboard focus everywhere, for free.
- `theme.rs` machinery: tokens as data, the JSON theme file with hot reload, the colour
  editor that exposes every token (and its test), the WCAG test for every shipped theme,
  and the helpers `tint`, `on_color`, `recess`, `on_light_key`.
- `widgets::{Knob, Fader, Bar}` **behaviour**: 200-px travel, Shift fine, double-click to
  default, right-click to type, arrows, wheel only when asked, arc from the default,
  modulation rings. That is a better interaction model than many commercial plug-ins.
- `icons.rs` — painted vector icons that scale; the set just needs to grow.
- `raster.rs` + `examples/shoot.rs` — screenshot-driven review with no display. Extend it
  to whole-window renders (Appendix A shows how; ~90 lines) and commit that harness.
- `layout.rs` — named layouts; it becomes the persistence layer of whatever panel system
  comes next.

**Must change for a first-rank look:**

1. **A component layer between egui and the views.** Today every view builds its own
   rows from `ui.horizontal`, `selectable_label`, `ui.label(RichText…)` and
   `ui.separator()`. Needed, each used everywhere: `Section`/card with a title bar;
   `Param` (label + control + value as one fixed-width unit, measured, elided);
   `Toggle` (framed, lit state, optional icon) and `Segmented` (for Play/Edit/Matrix,
   styles, densities, view tabs); `IconToggle` for M/S/Ø/FX/●; `TitleRow` (title
   truncates, icon buttons right); `PlotLabel` (plate/halo + collision list); `Toolbar`
   (groups with dividers; actions vs toggles visibly different); `Chip`; `Readout`
   (value in a recessed well).
2. **A type scale by role**, not by size: e.g. *title*, *section*, *label*, *value*,
   *micro* — 3–4 sizes (e.g. 11 / 12 / 14 / 18), a medium weight added (Plex Sans Medium
   is free), tracking on caps, one mechanism for painted and widget text (fold
   `design::paint` into the same table), and a test that bans `FontId::new` outside
   `design.rs`.
3. **Palette with roles and range.** Surfaces that step visibly (≥1.2:1 between levels, or
   a hue shift); a desk colour distinct from a strip colour; wells darker than faces in
   both themes; a second and third accent for meaning (e.g. mute amber, solo green/blue,
   record red, selection distinct from "on"); track colour at full strength on plates and
   headers (the user likes FL: its colour *is* the identity); meter gradients; per-module
   hues for editors. The anti-FL comments in `theme.rs:564-572` and `mixer.rs:695-697`
   should be retired with the rule they state.
4. **Depth.** Allow a controlled vocabulary: gradients on control faces and strip plates,
   inner shadow on wells (meters, grooves, displays), a 1-px highlight edge on raised
   surfaces, drop shadow for floating windows (already there). egui can paint all of this
   with meshes (`Mesh::colored_vertex`) and `epaint::Shadow`.
5. **Scroll affordance and wheel policy.** Set `spacing.scroll` to a visible style
   (`ScrollStyle::thin()` or `solid()`), set `always_scroll_the_only_direction = true`,
   and adopt one rule: the wheel scrolls; a control takes the wheel only with a modifier
   (Ctrl or Alt) or when focused.
6. **Editor chrome.** A custom title bar (instrument/effect name, preset name with
   prev/next, A/B, bypass, close) instead of egui's `Window` title; content that fits
   (min sizes derived from content, no inner scroll for Play views).

### 5.2 Docking and panels

**egui_dock** — **0.21.1** (released 2026-08-06; 0.21.0 on 08-05) depends on
**`egui ^0.36`** (crates.io dependency metadata), MIT licence (compatible with Cantus's
GPL-3.0-or-later), serde optional. README (main branch): *opening and closing tabs; moving
tabs between nodes and resizing; dragging tabs out into new egui windows; highly
customizable look and feel; control over the behaviour of the dock area and of
individual tabs; manipulating tabs and layout from code.* Binary splits only
(left/right, top/bottom), nested.

What it would give Cantus: arrangement, piano roll, channel rack, mixer, inspector (and
a browser, later) as **tabs in a split tree** the user rearranges by dragging, with
resizable splitters everywhere; mixer and piano roll visible at the same time; editors
dockable as tabs or floating; the whole state serialisable into `layout.rs`'s named
layouts. Fit with the code: every view is already `fn show(app: &mut CantusApp, ui: &mut
Ui)`, which is what `TabViewer::ui` wants (hold `&mut CantusApp` in the viewer; take the
`DockState` out of the app with `std::mem::take` for the frame to avoid a double borrow).
Costs: its tab bar looks like an IDE until styled (`egui_dock::Style` covers tab bar,
tabs, separators, overlay); floating windows are egui windows inside the main viewport,
not OS windows; one more dependency to move with each egui release (it has shipped a
release for each egui from 0.33 to 0.36: 0.18.0, 0.19.x, 0.20.x, 0.21.x).

**egui_tiles** — **0.17.1** (2026-08-18) depends on **`egui ^0.36.0`**, MIT OR Apache-2.0.
Tree of tiles: horizontal and vertical **N-way** linear layouts, grids, tabs,
drag-and-drop docking; behaviour and look through a `Behavior` trait; used by Rerun. No
floating windows and no built-in close buttons or scroll areas (per its README).

**egui alone (0.36)** already provides more than Cantus uses:

- `egui::Panel::{left,right,top,bottom}` with `resizable`, `default_size`, `min_size`,
  `max_size`, `size_range`, `exact_size` (used by `layout.rs` to force a layout);
- `show_collapsible(ui, &mut expanded, …)` — **drag the edge past the minimum to collapse,
  double-click the edge to toggle**, animated; with `drag_to_open(true)` a collapsed panel
  keeps an invisible grab handle that lights up on hover and drags it back open;
- `show_switched(ui, &mut expanded, collapsed_panel, expanded_panel, …)` — a thin rail
  when collapsed (icons, a vertical title), the full panel when expanded, sizes kept per id;
- `show_animated_inside` / `show_animated_between_inside` for panels inside a region;
- `egui::Resize` for a resizable box inside a panel (e.g. the velocity lane, the header
  column), `egui::Area` for free-moving overlays, `egui::Window` for floating editors
  (with `default_pos`, `anchor`, `constrain_to`).

**A small custom dock** on those parts, if the team prefers no dependency: a layout tree
(`Split { axis, ratio, a, b } | Leaf(PanelKind)`) stored in `layout.rs`; splitters as
4-px `ui.interact` strips that set `ratio`; each leaf with a 24-px header (title,
collapse chevron, a tab strip when two panels share a leaf); dragging a header shows a
five-zone drop overlay (left/right/top/bottom/centre-as-tab) on the leaf under the
pointer and rewrites the tree on release. Roughly 600–900 lines plus tests — which is
what egui_dock already is, maintained.

**Recommendation.** Do the egui-native part now — it fixes the complaints without new
dependencies: collapsible, drag-to-open inspector (`show_collapsible` + `drag_to_open`);
a bottom panel so the mixer (or piano roll) can sit *under* the arrangement instead of
replacing the central view; resizable header column and velocity lane (`Resize`); editor
windows positioned beside the desk. Then adopt **egui_dock 0.21** for "move them around"
(it has floating tabs and the maturity; egui_tiles if N-way splits turn out to matter
more), skinned with Cantus's tokens so it does not look like an IDE.

---

## 6. What to fix first

**Hours, not days (each is a few lines):**

1. `design.rs`: visible scroll bars (`ScrollStyle::thin()`), `always_scroll_the_only_direction = true`.
2. Mixer: wrap the desk in `ScrollArea::both()` (or give the middle a vertical area and
   let the fader floor rise), and take `wheel(true)` off the faders unless Ctrl/Alt is held.
3. Arrangement, piano roll, channel rack: read `scroll.x` when Shift is held (egui has
   already converted it); add a vertical offset to the arrangement and the rack.
4. Inspector: move `strip_section` and `effects` to the top when the mixer is the
   central view (or when a strip was the last thing clicked); clear `selected_track` when a
   strip no track feeds is selected; drop the duplicate channel gain/pan there.
5. Knob: measure label and value, allocate the widest (`knob.rs:169-171`).
6. Effect card title row: truncate the title, icon buttons (`inspector.rs:1138-1189`).
7. `section()` inside horizontals: use a vertical-only spacer (`inspector.rs:1029`).
8. Space card row widths (`space.rs:584`), gate label draw order (`gate.rs:901-926`),
   sampler seam label position (`sampler_wave.rs:487-493`), Pulsus handle glyphs
   (`pulsus_editor_display.rs:410-416`).
9. Menu bar via `egui::MenuBar`; master meter well in a recessed colour; margin on the
   arrangement toolbar; inspector macros by name (`frame.rs:791` → `param_knob_as`).
10. Piano-roll keyboard: a real piano shape (short black keys over full white keys) and
    key colours that contrast with `panel`.

**Weeks (the redesign the user asked for):** the component layer, the type scale, the
palette with roles and depth (§5.1), a mixer strip rebuilt around a large fader, lit
switches and insert slots, a channel panel beside the desk in the desk's language, and the
panel system (§5.2). The other agents' research on FL Studio and the market's DAWs says
what to aim at; this audit says where the present one breaks.

---

## Appendix A — reproducing the renders

Editors and the mixer alone (the tool in the repo):

```
cargo run -p cantus-ui --example shoot -- research/ui/screenshots/cantus-audit 1.0 "" eq comp limiter space multiband deesser gate plugin plugins
cargo run -p cantus-ui --example shoot -- research/ui/screenshots/cantus-audit 1.0 wavetable analog fm sampler bass organ epiano pulsus pulsus-play
```

The whole window: the repo has no tool for it, so a scratch crate outside the repository
was used (path dependencies on `crates/cantus-ui`, `cantus-engine`, `cantus-core`; the
repo's `Cargo.lock` copied; the workspace's dev profile copied; its own target dir). Its
core, for whoever turns it into `examples/shoot_window.rs`:

```rust
let ctx = egui::Context::default();
cantus_ui::design::install_fonts(&ctx);
cantus_ui::design::install_style(&ctx);
cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx); // or ink_and_gold()
let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
app.central = CentralView::Mixer;                    // PianoRoll / ChannelRack
app.layouts.current.show_arrangement = true;         // panels as a layout would set them
if let Some(s) = app.project().tracks[1].strip { app.select_strip(s); }
let mut frame = eframe::Frame::_new_kittest();       // public, doc(hidden), eframe 0.36
cantus_ui::raster::shoot(&ctx, egui::vec2(1500.0, 940.0), 1.0, 6, Some(&path), |root| {
    eframe::App::ui(&mut app, root, &mut frame);
});
```

Cases used: `full-pianoroll`, `full-rack`, `full-mixer` (1500×940), `full-mixer-small`
(1280×720), `full-mixer-noarr` (`show_arrangement = false`), `full-mixer-many` (+14
strips via `add_strip`), `full-mixer-native` (Keys set to `NativeSpec::new(Analog)`,
1500×1600), `full-mixer-kick` (Kick track and strip, 1500×1600), `full-mixer-fx` (Eq,
Compressor, Space and a send on strip 1, 1500×2300), `full-mixer-strip5` (strip 5 selected
after the Keys track, 1500×2300), `full-noinsp` (`show_inspector = false`).

## Appendix B — evidence crops

| File | Shows |
|---|---|
| `zoom-inspector-knob-labels-collide.png` | "Osc 2 semitones" into "Osc 2 detune" |
| `zoom-inspector-effect-title-under-buttons.png` | "Aequalis EQ" under "Up" |
| `zoom-mixer-strip-top.png`, `zoom-mixer-strip-bottom.png` | loose words on a strip |
| `zoom-transport.png` | boxed menus, empty-looking master meter, "Arrangement" at x=0 |
| `zoom-track-headers.png` | clip-map band under the track name |
| `zoom-pianoroll-keys-dark.png` | keyboard as disconnected bars |
| `zoom-rack-rows.png` | the rack (best-looking view) and its two mixed mini-controls |
| `zoom-comp-header-rows.png` | text-toggle style row, gold prose, touching knob labels |
| `zoom-eq-band-markers.png` | band 6 and an "S" badge overlapping |
| `zoom-space-tail-labels.png` | labels over the pre-delay bar and curves |
| `zoom-space-output-card-clipped.png` | Output card cut at the right |
| `zoom-gate-axis-labels.png` | axis labels under the fills |
| `zoom-sampler-seam-label.png` | seam label crossed by the seam |
| `zoom-pulsus-axis-vs-handles.png`, `zoom-pulsus-handle-letters.png` | axis labels under handles; stacked A/H |
| `zoom-instrument-bottom-band.png` | the chips / macros / keyboard band |
