# Bitwig Studio 5 (and 6): UI research for Cantus

*Research only. Written September 2026 for the Cantus UI redesign. Screenshots are in
`research/ui/screenshots/bitwig-studio/` (git-ignored); every file is listed with its source in §12.*

**Which version.** The brief asks for Bitwig Studio 5. Bitwig Studio 6 has since shipped
(announced September 2025). Most user-guide screenshots are from v5.x (their file names end in
`v5`/`v5.1`), with some from v2–v4 that were never re-shot. The product pages now show v6 only.
The interface grammar is the same across both. v6 changed the finish: darker greys, rounded
panels, gradient clip loops, a dark-only theme, a tool palette on the right edge, and smaller
track headers and mixer strips. Where the two differ, both are given, marked **v5** and **v6**.

**How the measurements were made.** The guide screenshots are 2× (Retina/200 %) captures, and
all numbers below are converted to **1× logical px**. One guide image is a true 1× window capture
(`ug-01`), and it agrees with the converted 2× numbers to within about 5 %. The v6 product shots
(3456×1944) are also 2×, but taken at a UI scale about 12 % smaller than the guide's. For those I
give ratios rather than pixels. The v5 PNGs are true-colour, so their hex values are exact. The v6
PNGs are palette-quantised, so their hex values are within about ±4.

---

## 0. The short version: seven things that make Bitwig look professional

1. **A strict grey ladder with five steps and no in-between greys.** Colour sits on top of it only
   when it means something. Orange means "on / active / an editable value". Blue means "time and
   selection". Track colours mean identity and nothing else.
2. **Every form is a two-column grid.** Right-aligned labels hug dark value wells. Rows share one
   height, related rows are grouped into blocks separated by a hairline, and each block sits under
   an UPPERCASE section band. No label floats freely, which is the direct cure for the user's
   "loose words" complaint.
3. **Values look different from labels.** Values are always drawn inside a dark well, in the
   accent colour, with fixed-pitch digits, right-aligned. Labels are light grey on the mid-grey
   surface. A glance separates what can be edited from what only describes it.
4. **The window has fixed places for panels, toggled from the footer.** There is a centre view, a
   bottom secondary slot, a right-hand access slot and a left-hand inspector slot. Each slot shows
   one panel at a time, each panel has a single-key shortcut, and its edge facing the centre can be
   dragged. There is no free-form docking, and the result is calm and predictable.
5. **A footer status line explains whatever is under the mouse.** It shows the parameter's full
   name and precise value, then the gestures available (`DRAG Adjust value · CMD+CLICK Type
   value · DOUBLE-CLICK Set to default`). Because of it, on-screen labels can stay short.
6. **The mixer never scrolls vertically.** Each section (remotes, devices, sends, I/O, crossfader,
   comments) is a view toggle. When the panel is short, sections that are switched on but don't fit
   are hidden and their toggle greys out. The fader section stretches to take the remaining height.
7. **Density with discipline.** Controls are small (about 11–12 px text at 1×), but everything sits
   in rigid rows and blocks. Empty space goes between blocks and sections, never between a label
   and its value.

---

## 1. Visual language

### 1.1 Palette (measured)

**v5 grey ladder.** Exact values, from `ug-02`, `ug-03` and `ug-08`:

| Role | Hex | Where |
|---|---|---|
| Content well / "void" | `#0d0d0d` | Empty arranger and launcher area, device panel background, empty mixer area, gaps between mixer strips |
| Deep well | `#141414` | Transport display interior |
| Field well | `#1d1d1d` | Every value field, comment box, meter background, pan well |
| Window / panel base | `#2e2e2e` | Window header, footer, gaps between panels, lower inspector |
| Control shadow edge | `#323232` | 1 px outline around buttons and choosers |
| Section surface | `#414141` | Inspector blocks, mixer row-label column, sends block |
| Section band / divider | `#535353` | "INSTRUMENT TRACK" band, 1 px block dividers, unselected track header |
| Chooser (drop-down) | gradient `#6b6b6b → #4f4f4f` | Channel "All ▾", I/O choosers |
| Toggle button, off | gradient `#757575 → #5e5e5e` | "M" mute, the "Show Help" button |
| Focus rim | `#787878`–`#8e8e8e` | Outline of the focused panel |
| Selected track header | `#9c9c9c` | Silver background with dark text (selection is shown by *inverting*) |

**v6 grey ladder.** Approximate, from `web-01`, `web-03` and `web-04`. The ladder is shifted darker
and the steps are more even:

| Role | Hex |
|---|---|
| Field well | `#1b1b1b` |
| Inactive field / recessed | `#272727` |
| Section surface (inspector) | `#323232` |
| Device body | `#3d3c3c` |
| Section band / divider | `#494949` |
| Buttons | `#595959` (gradient to `#4f5050`) |
| Focused panel rim | `#686868` |

Each step is 16–19 levels of luminance (0x10–0x13). That regular spacing is why the surfaces read
as layers without needing shadows.

**Accent and signal colours:**

| Meaning | Hex | Uses |
|---|---|---|
| Active / on / editable value | v5 `#f1832f`–`#fc8931`, value text `#e28435`; v6 `#fc8714` | Toggles that are on (orange fill, gradient `#f08c38 → #e48535`), value digits, the active view word ("ARRANGE"), active panel icons, focused tool |
| Dim active / level fill | `#67411f` (v6 `#673f18`) | Fader fill column, the volume bar in track headers; always capped by a 1–2 px line in full orange |
| Inactive value | ≈ `#8c5b30` on `#272727` | A value that exists but doesn't currently apply (clip "Stop" while looping). Dimmed, never hidden |
| Transport and time readouts | `#46b9fa` | Tempo, position and time digits; playhead flag |
| Record | `#f91f1d` | Arm buttons |
| Browser selection | result row `#2a333c`, tree row `#3e6389` | Slate and blue, never orange: selection is not the same as "active" |
| Meter | green gradient `#83c144 → #2f7f36`, yellow near the top, orange peak-hold line | |
| Modulation | blue = monophonic source, green = polyphonic source | Routing buttons, modulation rings on knobs, and the mapping-mode tint (`web-05`) |
| Sends | yellow ring = post-fader, blue ring = pre-fader | Mixer and inspector send knobs |
| Grid ports | yellow = logic, purple = phase, red = generic signal, turquoise = control module, blue = secondary input | The Grid |
| Browser device kind | yellow = instrument, red = audio FX, blue = note FX | Icon tints and filter chips |

**Text colours:**

| Role | Hex |
|---|---|
| Primary label | v5 `#e5e5e5`, v6 `#c2c2c2` |
| Emphasis (names, "Signature 4/4") | `#ffffff` |
| Secondary: metadata keys, counts, peak readouts | about `#8a8a8a`–`#9c9c9c` |
| Placeholder / auto-generated name | the same grey, set in *italic* |

**Contrast** (computed from the values above):

| Pair | Ratio |
|---|---|
| `#e5e5e5` on `#414141` | ≈ 8.1:1 |
| `#c2c2c2` on `#323232` | ≈ 7.2:1 |
| Orange `#fc8714` on `#1b1b1b` | ≈ 7.1:1 |
| Blue `#46b9fa` on `#1d1d1d` | ≈ 7.7:1 |
| Dim inactive value | ≈ 2.6:1 (deliberately low) |
| Metadata key `#9c9c9c` on `#292929` | ≈ 5.3:1 |

So the dark UI stays comfortably above WCAG AA for everything that is meant to be read.

### 1.2 How colour carries meaning

- **Track colour = identity only.** It appears as a full-height stripe on arranger headers (about
  7 px), as the filled background of mixer headers (optional: "Fill header backgrounds"), as a
  3 px bar at the foot of each mixer strip, on clip bodies, and as a stripe on the device panel's
  track header. It never tints controls.
- **One accent for "active".** Every toggle, view word, panel icon and editable value uses the
  same orange. The guide states the rule itself: *"When a button is tinted orange, that control is
  active. The inactive form of a control uses a neutral color."*
- **Blue is time and selection**, so it never competes with orange.
- **Groups tint their children.** In v6 the backgrounds of child tracks carry the group's colour,
  and the group header wraps above its children like a rounded tab (`crop-09`).
- **Multi-selection with differing values** shows diagonal "hazard" stripes behind the field
  (v6; visible on "Pitch 1.00" in `web-04`).

### 1.3 Depth

- **v5 is almost entirely flat.** Surfaces are separated by the luminance ladder and 1 px rules.
  The only gradients are on buttons and choosers, a vertical ramp of about 0x17. Buttons have a
  1 px darker outline (`#323232`). There are no drop shadows except on floating things: the
  Pop-up Browser, menus and notifications.
- **v6 adds a little finish.** Panels, the window and fields get rounded corners: about 2 px on
  fields, 3–4 px on panels, about 6 px on device cards and 10 px or more on floating windows.
  Backgrounds are darker. Clip loops get a gradient per loop pass, and automation curves get
  gradient fills. Sound On Sound: *"Backgrounds are a darker grey, and a few visual features like
  clip loops and automation curves have acquired gradient colour fills, adding a touch of depth."*
- **Content recedes and chrome floats.** Content areas (arranger, device panel, launcher) are
  almost black (`#0d0d0d`), controls live on mid-grey blocks, and value wells are near-black holes
  in those blocks. Three depth levels in all, all achieved with flat fills.

### 1.4 Density and use of space

- Rows are tight: inspector rows repeat every 24 px with 21 px wells, leaving 3 px between rows.
  Space goes **between blocks**: a divider with about 3 px of padding on each side, plus a band per
  section.
- In v6 the inspector splits its sections into a **top-anchored stack** (the container: slot,
  clip, time range) and a **bottom-anchored stack** (the contents: notes, expressions). Empty space
  collects in the middle (`crop-01`, `web-04`). The note block therefore stays in the same place
  next to the editor however much the clip block grows.
- Things that don't apply are **dimmed, not removed.** For example, "Retrigger" dims when voices
  are above 1 (`ug-27`). The layout never jumps.

### 1.5 Iconography

- Small pictograms of about 14–16 px, filled or 1.5 px line, grey `#c2c2c2`, orange when active.
- Each panel icon *pictures* its panel. The inspector is a seriffed lowercase "i". The device panel
  is a rounded rectangle with a shaded left side, like a device header. The mixer is three
  vertical bars. The detail editor is a set of dashed "piano-roll" lines.
- Each track type has an icon: piano keys (instrument), waveform (audio), half-and-half (hybrid),
  down-arrow (FX), folder (group), crown (master). Icons always come *before* the name.
- Icons stand alone only where a tooltip plus the footer explain them. Menu buttons carry a small
  "dog-ear" triangle in the bottom-right corner, meaning "this unfolds".

---

## 2. Typography

### 2.1 Typeface

- A humanist sans in several weights.
  - A community font-patching script extracts `SourceSansPro-{ExtraLight, Light, Regular,
    Semibold, Bold, Black}.ttf` from `bitwig.jar` (Bitwig v2/3 era;
    github.com/jhorology/bitwig-studio-japanese-font).
  - Bitwig's own release notes are styled in Source Sans Pro.
  - Glyphs in current screenshots (single-storey **g**, angled **t** terminals; `ug-27`, `ug-13`)
    also look very like **Lato**, so treat the exact family as unverified.
  - What matters for Cantus: one humanist sans, used at 2–3 weights. IBM Plex Sans, already
    bundled, fits this role.
- **Digits are fixed-pitch.** All times, positions and values use tabular figures with **wide
  tracking**. The dots in `3.1.1.00` get a full digit-width cell, so bars, beats and ticks line up
  column by column across rows (`crop-02`, `ug-03`). In v5 the transport digits (`110.00`,
  `1.1.1.00`) are larger, about 17 px, in blue.

### 2.2 Sizes, weights and case at 1×

| Element | Size | Weight | Case | Colour |
|---|---|---|---|---|
| Inspector / form label | ≈ 11.5 px (cap height ≈ 7.5 px) | Regular | Sentence ("Vel Spread") | `#e5e5e5` / `#c2c2c2` |
| Value in a well | ≈ 11.5 px, tabular, tracked +10–15 % | Regular | – | Orange (editable), white (plain), dim brown (inactive) |
| Section band ("INSTRUMENT TRACK", "NOTES (16)") | ≈ 11 px | Regular/Medium | UPPERCASE, slightly tracked | `#e7e7e7` on `#535353` |
| Device name in the device header | ≈ 11 px, rotated 90° (reads bottom to top) | Bold | UPPERCASE ("MICRO-PITCH", "EQ+") | White |
| Knob label | ≈ 11 px, centred *under* the knob | Regular | Title/short ("Pitch", "A D S R") | `#c2c2c2` |
| Track name (header) | ≈ 12 px | Regular | As typed | White (dark on a selected header) |
| Mixer header name | ≈ 12 px, centred, wraps to 2 lines | Regular | As typed | White on track colour |
| View words ("ARRANGE MIX EDIT") | ≈ 12 px | Medium | UPPERCASE | Orange if active, grey otherwise |
| Footer hint | modifier ≈ 11 px **bold UPPERCASE** + action ≈ 11 px regular | – | "SHIFT+CLICK Select range" | Modifier `#e0e0e0`, action `#9c9c9c` |
| Browser counts | ≈ 10 px, directly after the name ("Cymbal 652") | Regular | – | `#8a8a8a` |
| Browser metadata key / value | ≈ 10.5 px UPPERCASE key; ≈ 11.5 px value | Regular | "CATEGORY Synth Bass" | Key `#9c9c9c`, value white, linked value orange |
| Placeholders / default names | Same size | *Italic* | "Comment", "*Inst 1*", "*User-defined name*" | Grey |

Case rules are strict. **UPPERCASE** is reserved for section bands, device names in headers, view
words, footer modifier keys, metadata keys and menu names (FILE, PLAY, ADD, EDIT, CLIP). Everything
the user reads as content is in sentence or title case.

### 2.3 How labels attach to controls, and how "loose words" are avoided

1. **Forms (inspector).** Labels are **right-aligned** in a fixed label column and end about 7 px
   before the value well, which fills the rest of the width. Every row in a panel shares the same
   two column edges. (Early v2 inspectors left-aligned their labels, as in `ug-05`. Bitwig moved to
   right-aligned labels, which pull each label onto its value.)
2. **Blocks.** Related rows are grouped: *Start/Stop*, *Looping/Start/Length*, *Shuffle/Accent*.
   Each block sits on the section surface and is separated from the next by a 1 px rule. A label
   is never further than about 7 px from its value, and blocks are 7–10 px apart. Proximity alone
   shows what belongs to what.
3. **Column headers instead of repeated labels.** Launcher-clip settings use a "Main | ALT" header
   over two columns of choosers (`crop-01`). The mixer has a **row-label column** on its left
   ("Remotes", "Devices", "Sends", "I/O", "Crossfade", "Comments"; `ug-08`), so strips carry no
   repeated labels.
4. **Knobs.** The label goes *under* the knob, centred on its axis. The value appears either above
   the knob (e.g. "440 Hz") or in the footer on hover. Labels in a row share a baseline.
5. **Labels shrink to fit.** Narrow mixer strips abbreviate send names ("Reverb" becomes "Rvb",
   "Delay-2" becomes "Dl-2"; `ug-06`). Long device names are truncated with "…" ("Mod Amt",
   "Depth S…"; `web-02`).
6. **Units stay with their numbers.** Units follow a thin space inside the same well and in the
   same colour ("-10.0 dB", "0.00 %", "2.33 kHz").
7. **Counts go in the header.** A multi-selection shows its count in the band: "NOTES (18)".
8. **Two-line headers: name, then path.** Automation-lane headers set the parameter name in white
   ("Cutoff Frequency") and its path in grey below, with » separators ("Filter+ » Low-pass MG";
   `web-09`). The primary word stays short while the context is still there.

---

## 3. Layout and panels (the user's explicit ask)

### 3.1 Window anatomy: four horizontal slices

From the 1× window capture `ug-01`, cross-checked against the v6 shots:

```
┌ Header  ~32 px  project tabs (left) · Dashboard button (centre) · controller icons · window controls ┐
├ Menus/transport ~44–48 px  FILE[+pinned] PLAY[transport] … transport display … ADD EDIT <object menu> ┤
├ Body ────────────────────────────────────────────────────────────────────────────────────────────── ┤
│ Inspector │                 Central panel (view-defined)                  │ Access panel     │
│ ~157–160  │   Arranger (Arrange) · Mixer (Mix) · Detail editor (Edit)     │ Browser ~214 px  │
│ px, fixed │───────────────────────────────────────────────────────────────│ Project, Output, │
│           │   Secondary panel (bottom): Device panel ~220–240 px /        │ Mappings         │
│           │   Mixer / Detail editor. Resizable vertically.                │ Resizable horiz. │
├ Footer ~34 px  [i] | ARRANGE MIX EDIT | panel icons | available actions / hover info … | right panel icons ┤
```

- **Panel gaps** are about 3 px of `#2e2e2e`. Each panel is a rounded rectangle with a 1 px border
  (`#535353`). **The focused panel's border turns silver** (`#787878`/`#8e8e8e`; v6 `#686868`).
  Only one panel has focus, focus follows the last click, and panel-specific shortcuts go to the
  focused panel.
- **The central panel can't be hidden.** If every other panel is off, it fills the body.
- **Secondary panel (bottom): "Most secondary panels can be vertically resized."** You drag the
  border facing the centre of the window, and the cursor becomes a two-way arrow.
- **Access panels (right)** are browser, project, output monitoring and mappings. **"Each of these
  panels can be horizontally resized."** When none is open, the centre takes the space back.
- **Inspector (left): "This panel is not resizable"** (v5 guide). Its width stays constant relative
  to the UI scale in the v6 shots too: about 141 logical px at the hero's scale and about 160 at
  the guide's.
- **Views** are named layouts that set which panel is central.
  - **Arrange**: arranger, with the launcher optional.
  - **Mix**: mixer, with the launcher optional.
  - **Edit**: detail editor, with automation below it. In v6 automation is merged into the detail
    editor, and `Shift+Tab` gives a full-screen Edit view.
- **Footer panel clusters.** Icons sit in three clusters: far-left (left slot), centre-left
  (bottom slot) and far-right (right slot). **Within a cluster only one panel is shown at a time.**
  Each slot is therefore an exclusive, tab-like place, not a free dock.
- **Shortcuts** open or focus panels: `I`/`Alt+I` inspector, `E` detail editor, `A` automation,
  `D` device panel, `M` mixer, `Alt+B` browser. Shortcuts stay live *while dragging*, so you can
  drag a clip, press `M` to open the mixer, and drop it there.

### 3.2 Display profiles (multi-monitor)

A display profile is a fixed window arrangement, chosen from the right-click menu on the window
header or in Settings → User Interface.

| Profile | Arrangement |
|---|---|
| Single Display (Large) | The default: one window, one view at a time |
| Single Display (Small) | Tighter layout for small screens |
| Tablet | Touch layout. A larger header holds the view words and panel icons, there is a track selector, and a **Play** view with an on-screen keyboard |
| Dual Display (Studio) | Arrange on the primary screen; secondary toggles Mix / Edit |
| Dual Display (Arranger/Mixer) | Fixed: Arrange on one screen, Mix on the other |
| Dual Display (Master/Detail) | Edit on the secondary; primary toggles Arrange / Mix |
| Dual Display (Studio/Touch) | A normal window plus a tablet-style window |
| Triple Display | Fixed: Arrange / Mix / Edit on three screens |

In dual profiles the footer shows compound view words (ARRANGE-MIX). Floating windows exist too:
the Expanded Device View can be undocked; Pop-out Editors (curve and step editors) are resizable
windows; plug-in UIs float. A setting called *Floating windows follow current track* hides a
track's floating windows when another track is selected, and a thumbtack pins any of them
permanently.

### 3.3 Other resizing and rearranging

- **Mixer strips** can each be resized horizontally (v6: "Mixer channels can now be resized even
  smaller"). Launcher tracks in the mixer can be widened too.
- **Arranger tracks** have a normal/half height toggle. In v6, headers resize continuously and
  **flatten to a single line of essential controls** when small. v6 also adds **Auto-zoom**
  (`Shift+Z`): the selected track or lane grows while the rest stays put.
- **Browser Panel** has an internal splitter between the filter tree and the results; its handle
  is visible (`ug-13`). In the **Pop-up Browser**, the widths of the source/filter area and of the
  file area can be dragged and are **remembered per context** (adding a device versus a clip, and
  so on).
- **Grid editor**: zoom − / 100 % / + buttons, independent of the global UI scale.
- **Global scale**: the header's right-click menu has Increase / Decrease GUI Scaling, plus a
  per-display scaling setting. **v6** adds live *Midtone* and *Black level* controls in the same
  right-click menu. It is a contrast dial for the whole UI.
- **Pinnable menus.** Every menu item has a thumbtack. Pinning it adds an icon button beside that
  menu in the top bar (`crop-03`, `crop-04`). When the window gets narrow, pinned buttons drop out
  first. This is how the toolbar is "rearranged": items are added from menus, not dragged around.
- **Editing tools sit on each panel's own edge (v6).** Each timeline panel has a thin vertical tool
  column on its right edge (`web-09`). When the panel is short, the column collapses to the active
  tool, which then opens a menu. An *Editor Settings* button in the bottom-right corner holds that
  panel's appearance options.
- **Rearranging is limited.** You can't drag panels into other slots. Flexibility comes from views,
  display profiles, slot toggles, edge resizing and floating windows. This is the right trade-off
  for Cantus (§10).

---

## 4. The Inspector in detail (the user dislikes Cantus's)

The Inspector is one fixed column on the left. It **shows every property of whatever was selected
last**: track, clip, note(s), audio event, automation point(s), device, modulator or Grid module.
The guide's split is that the Inspector is for *viewing and changing parameters*, while the context
menu is for *running functions*.

### 4.1 One row grammar everywhere

Measured on `ug-03` and `ug-02` (v4/v5) at 1×:

```
┌──────────────── 160 px ────────────────┐
│        ARRANGER CLIP                   │  section band: 22 px, #535353, UPPERCASE ~11 px, centred
│ ┌ Barefoot Beat A 04 ───────────────┐  │  name field: full width, 20 px, well #1d1d1d
│ [■■■■■■■■■]  colour palette 9 × 3     │  15 px swatches, ✕ (inherit) and ▶ (palettes)
├────────────────────────────────────────┤  1 px rule #535353, ~3 px padding either side
│     Signature │            4/4 │         label column ≈ 62 px, RIGHT-aligned
├────────────────────────────────────────┤
│      Position │      2.1.1.00  │         value well ≈ 86 px, 21 px tall, 3 px gap below
│        Length │      3.2.0.00  │         row pitch 24 px; digits orange, tabular, tracked
│        Offset │      1.1.1.00  │
├────────────────────────────────────────┤
│       Looping │ [    ⟲ orange ]│         toggle = full-width button, orange when on
│         Start │      1.1.1.00  │
│        Length │      1.0.0.00  │
├────────────────────────────────────────┤
│         Muted │ [      M     ] │         off-state grey gradient #757575→#5e5e5e
├────────────────────────────────────────┤
│       Shuffle │ [    ≋ orange ]│
│        Accent │▏        0.00 % │         ▏= thin bar at the well's left edge showing the value
├────────────────────────────────────────┤
│          Seed │ [🎲] Random    │         compound control: die toggle + value
└────────────────────────────────────────┘
```

- **Proportions.** Label column ≈ 39 %, gap ≈ 4 %, value column ≈ 54 %, and 3 px insets.
- **Right-aligned labels.** Every label ends on the same vertical line, so the eye reads straight
  across to the value.
- **Every value is in a well.** Dark wells show the value in orange (editable), white (plain) or
  dim brown (inactive). The edit gestures are the same everywhere: drag vertically; Shift-drag for
  fine; Ctrl/Cmd-click or double-click to type; double-click a knob to reset.
- **Numeric add-ons.**
  - A thin **level bar** of 2–3 px at the left edge of the well shows the normalised value
    (velocity, gain, accent).
  - A **▶ button** at the right edge opens the **histogram editor** for a multi-selection. It
    provides Mean / Scale / Chaos editing of the spread of values (`ug-05`, `web-04`).
- **Multi-selection.** The band shows the count ("NOTES (16)"). Mixed values get hazard stripes
  (v6). Changing a value moves all the selected items.
- **Dim, don't hide.** Parameters that don't apply at the moment ("Stop" when looping, "Retrigger"
  when polyphonic) stay in place, dimmed.
- **Italic grey marks a name Bitwig generated.** Typing a name replaces it; deleting the name
  brings the automatic one back.
- **v6 anchoring.** The container sections anchor to the top of the column and the content sections
  to the bottom (§1.4).

### 4.2 What the Inspector shows for a selected **track** (`ug-02`, `ug-06`, `ug-07`)

Top to bottom, v5.1:

1. **Band:** "INSTRUMENT TRACK" / "AUDIO TRACK" / "HYBRID TRACK" / "EFFECT TRACK" / "MASTER".
2. **Name** field (italic when automatic).
3. **Colour palette** (the same 27 swatches as the context menu), with ✕ to inherit and ▶ to choose
   a palette.
4. **Comment**: a multi-line box, also shown in the mixer's Comments section.
5. **Active** toggle: deactivating a track removes its CPU load.
6. **MIDI input:** *Channel* (All ▾ ▸ 1 ▾), and *P. Bend* (→ Expr. ▾ −2 ▲ 2).
7. **A four-tab strip**, icons only, each a panel of its own:
   - (a) **Devices**: the top-level chain as a list. Each row has a category dot (yellow for
     instrument, etc.), the name, and a button that opens the device's expanded view.
     "Σ 1.0 ms" shows the chain's latency, and "+" adds a device.
   - (b) **Track remotes**.
   - (c) **Modulation by source**.
   - (d) **Modulation by destination**.
8. **I/O**: an input chooser (speaker icon plus "All ins ▾") and an output chooser ("Master ▾").
9. **A mixer strip pinned at the bottom**, two columns:
   - Left: **Sends**. Each has a name, a Pre/Post/Auto chooser under the name, and a knob whose
     ring is yellow for post and blue for pre.
   - Right: a **Rec/S/M** segmented bar, the **pan** well, a **volume** readout ("−10.0" in orange),
     and a **meter plus fader column** with a scale of 0, 6, 12, 18, 24, 36, ∞.
   - FX tracks add a **Pre-fader (Cue)** button. When the column is narrow, the strip stacks
     S and M vertically with a big pan knob (`ug-07`).

The inspector therefore **contains the selected track's whole channel strip**. You can mix one
track without opening the mixer, and clicking the Project or group header in the device panel
turns the Inspector into the master's or group's strip.

### 4.3 Other selections

- **Launcher clip** (`crop-01`):
  - SLOT: "Has stop/rec button".
  - LAUNCHER CLIP: name, palette, Signature, Start/Stop, Looping/Start/Length, Muted,
    Shuffle/Accent, Seed.
  - Launch settings under a **Main | ALT** column header: Launch Q, Play Mode, on Release,
    Q to Loop.
  - Next Action: power toggle, "Play Next ▾", ×1, 0.0.0.00.
- **Notes**:
  - NOTES (n): Start, Length, Muted, Channel, Key (two wells: A#2 and C4), Velocity, Vel Spread,
    R-Velocity.
  - OPERATORS: shown only when notes are selected.
  - EXPRESSIONS: Gain dB, Pan %, Pitch st, Timbre %, Pressure %. Each value is the *average* of the
    note's expression curve, and editing it shifts the curve.
- **Time range** (v6 Edit view): TIME RANGE with From / To / Time (s).
- **Device** (`ug-16`, `ug-17`, `ug-27`):
  - DEVICE band; the device's icon and "Name (Category)"; a one-line *italic grey description*.
  - A **Show Help** button; a user-name field; the Enable and Active toggles, with Enable mirroring
    the power button on the device.
  - Instruments add Voices, Voice Stacking (with a solo button, "S"), Playing voices "0 / 40",
    Retrigger (**Never | Note on | Always** segmented), Steal same key and Steal fade time.
  - **Two tabs:**
    - *By source*: each modulator's name has its routing button on the right, with its routings
      indented beneath: "✕ Gain [transfer-curve glyph] +0.878" in orange.
    - *By destination*: the same data turned around, grouped by parameter.
  - Routings in the list can be dragged to another source (hold Alt to copy). **Shift-click**
    bypasses a routing. The small curve glyph is a menu of transfer functions (Linear, Positives,
    Negatives, Absolute, Toward Zero, Exponential, Logarithmic).
- **Plug-in**: *Suspend* (Never / When silent / Trust plug-in) and MPE with a PB range.
- **Grid module** (`ug-21`): its parameters, plus **live oscilloscopes of every input and output
  port**. Unconnected ports are dimmed and folded away.

### 4.4 Why Bitwig's inspector works and Cantus's doesn't (from the user's complaints)

| User complaint | Bitwig's answer |
|---|---|
| "Very, very ugly side panel" | Uses three surfaces (band `#535353`, block `#414141`, well `#1d1d1d`), each with a single job. Nothing sits directly on the window background |
| "Letters run into each other" | A fixed row height with 3 px between wells; labels are truncated with an ellipsis instead of overflowing; label and value columns can't collide |
| "Loose words thrown down" | Two aligned columns, right-aligned labels, grouped blocks under uppercase bands. Headings are bands, not free-floating bold text |
| Current Cantus inspector | It puts a caption *above* each control and a readout at the far right, uses full-width faders, `ui.separator()` and free-floating headings (`views/inspector.rs`). This scatters label, readout and control across three places, which is the "loose words" look |

---

## 5. The mixer

### 5.1 Strip anatomy, top to bottom (`ug-08`, `web-03`, `crop-05`)

1. **Header.** About 46 px, filled with the track colour, the name centred and wrapping to two
   lines. A fold button unfolds Drum Machine or layer chains into extra strips to the right.
   Groups wrap their children with a rounded coloured tab (v6).
2. **Clip Launcher**, optional. When it's on, each strip becomes a vertical column of clip slots and
   the Mix view doubles as a performance view. v6 adds a per-track **playback pie** (track colour)
   in the stop row.
3. **Big meters**, optional. Tall stereo meters with a scale of 0, 6, 12 … 48, ∞. Only available
   when the launcher is hidden.
4. **Track remotes.** Up to eight knobs per strip, each labelled to its right. Unmapped slots show
   a Wi-Fi-style "map me" icon.
5. **Devices.** The chain as a list: category dot and name. Some devices show **mini displays in
   the row**: EQ curves, gain-reduction bars. Drag to reorder, Alt-drag to copy, Shift-drag to
   layer. There is a "+" to add a device and a latency total.
6. **Sends.** Each send has a name and a knob with a yellow (post) or blue (pre) ring. Clicking the
   name toggles the send; disabled sends are dimmed. Shift-click toggles all of a strip's sends.
7. **I/O.** Input and output choosers.
8. **Channel strip** (always shown):
   - The Rec / S / M segmented bar.
   - The pan well: a dark bar with an orange tick at the value.
   - A level box: peak hold on the left in grey, volume on the right in orange with "dB", and a
     1 px rule under them.
   - A meter plus fader column. **The fader is a filled column** in `#67411f` with a 2 px orange
     cap at the level. There is no fader cap to grab: the whole column is the hit target. Two thin
     meter bars sit beside the dB scale.
   - Hovering the peak shows a popup with peak and RMS per channel (`ug-26`).
9. **Crossfader** A / ◆ / B, optional.
10. **Comments**, optional.
11. **A 3 px colour bar** at the foot of the strip.

### 5.2 Widths and layout

- **Strip widths.** Default ≈ 82 px (1×). Strips can be widened (≈ 106–118 px are common in
  screenshots) and, in v6, made narrower. There is about 2 px of `#0d0d0d` between strips.
- **Pinned strips.** FX (return) tracks and the Master sit at the far right, apart from the rest.
- **Left column.** About 82 px wide. It holds the **row labels** for each section ("Remotes",
  "Devices", "Sends", …) in dim grey, and the **view toggles** at the bottom left: an icon grid
  for launcher / meters / remotes / devices / sends / I/O / crossfader / comments, plus FX-tracks
  and inactive-tracks toggles.
- **Header fill.** Right-clicking the toggles gives *Fill header backgrounds*: either colour-filled
  headers or a thin colour stripe.

### 5.3 Scrolling, height, zoom

- **Horizontally**, the strip area scrolls with a scrollbar; the pinned FX and Master strips don't
  move.
- **Vertically, nothing scrolls** (apart from the launcher's scene list). **Sections that are on but
  don't fit are dropped and their toggle greys out** (`ug-09` against `ug-10`). The channel strip
  always stays, and the fader takes whatever height is left. The fix for a cramped mixer is
  dragging the panel border up, or switching off sections you don't need.
- **Zoom.** There is no separate mixer zoom. Global GUI scaling plus per-strip width do that job.

### 5.4 Other places to mix

- A **secondary Mixer Panel** can be opened under the Arrange view (`M`). It works the same as the
  full mixer, except the launcher and big meters aren't available there.
- The **track Inspector** mixes one track (§4.2).
- The **FX track's header in the device panel** has a *send-receive* pane. It lists every track that
  feeds that FX track, each with a meter and a send amount: a mixer for one bus.

---

## 6. Device panel, devices and modulators

### 6.1 Device panel (`crop-08`, `web-01`, `web-02`, `ug-25`)

- A horizontal chain; signal flows left to right.
- **Every device has the same height**, about 215–240 px at 1×. Width depends on the device.
- The chain scrolls horizontally.
- **Headers on the far left, one per level.** There is one for the Project, one for each enclosing
  group, and one for the track. Each is a narrow vertical strip carrying the rotated name.
  Clicking one points the Inspector at that level, which is how you see the master's or group's
  strip.
- **Between devices** there is a thin column with a note-activity light, a "+" to insert a device,
  and **audio meters**, so you can watch the level change at every step of the chain.
- **Each device is a card** with rounded corners of about 6 px (v6).
  - The left edge is a **vertical header strip** about 28 px wide. From top to bottom it holds the
    power button (orange), the preset folder, the **name rotated, in bold UPPERCASE**, and then the
    expanded-view, remote-controls and modulators toggles at the bottom.
  - The body groups controls into **darker sub-panels** (`web-10`).
- **Knobs** (`ug-24`, `web-10`):
  - A grey body of about 24–26 px with a white pointer line.
  - A darker ring about 34 px across, with the **value arc in colour from the minimum**. Most arcs
    are orange; per-function colours appear in newer devices (blue for threshold, ratio and knee in
    Compressor+).
  - A small tick marks the default.
  - Label underneath.
- **Displays double as controls.** EQ+ nodes can be dragged; Polysynth's oscillator waveshapes are
  editable; the compressor curve shows threshold as a draggable blue line.
- **Selected state:** a dashed white border. Double-clicking the header minimises a device to its
  header strip.
- **Expanded Device View.** Opens *in the central panel area* (docked), or undocks to a floating
  window with ⇱ (`ug-15`, `ug-20`). It's used by EQ+, the compressor, the Grid, samplers and
  others. The small device in the chain and the big expanded view are two views of the same
  device.
- **Containers.**
  - Drum Machine shows a grid of 16 visible pads with a selected-chain strip.
  - Instrument and FX Layer have one row per layer, each with its own mini channel strip.
  - Chains inside devices (FX, Pre FX, Wet FX, FB FX) appear as tabs on the device.
  - When a nested chain is expanded, a bracket above its devices, drawn in a highlight colour, shows
    where it belongs.

### 6.2 Modulators: the Unified Modulation System

- Each device has a **Modulators pane**. It opens with three empty slots and adds three more
  whenever they fill.
  - Each slot holds a modulator tile (LFO, ADSR, Steps, Random, Expressions, Keytrack, …) with a
    **mini display** and a **routing button**, a port icon with a short cable.
  - A ▸ on the right edge of a tile opens its extra parameters.
- **Assigning a modulation.** Click the routing button; it flashes. Existing targets light up and
  every **possible target is shaded**. Across the whole device this is a teal-blue overlay tint
  (`web-05`). Then drag on a knob to set the modulation depth.
- **Knobs show modulation** as a coloured ring segment beside the value arc:
  - **Blue** for a monophonic source, **green** for a polyphonic one.
  - The range is relative, so it can extend past the knob's own range.
  - The *modulated* current value is marked live in cyan (`ug-18`).
- **The footer reports both values:** "Filter Resonance 39.5 % [27.1 %]". The bracketed value is
  the modulated one.
- **Modulators can target modulators.** Mixer controls (volume, pan, sends) can be modulated from
  track-level modulators, and transport parameters from the project level.
- **Editing.** Right-clicking a routing button lists every routing with a ✕ to remove it.
  Right-clicking a target parameter lists the routings that reach it.
- **Pop-out editors.** Curves, Steps and Pitches open resizable editor windows with tools and a
  snap grid.

### 6.3 Plug-in windows

- A plug-in appears in the chain as a small card: generic header plus remote controls.
- Its own UI **floats in an OS window**, opened from the card's header.
- Floating windows can follow the selected track, or be pinned.
- Plug-in options (Suspend, MPE) live in the Inspector, not in the plug-in window.

---

## 7. The Grid (modular editor)

Screenshots: `ug-20`, `ug-21`, `web-06`.

- **Where it opens.** It is an Expanded Device View: it replaces the central panel, or undocks. The
  small device in the chain shows a **patch overview thumbnail**. Clicking or dragging inside it
  scrolls the editor, like a minimap.
- **Header bar.**
  - Left: the power button, a padlock that locks the patch against edits, a ▦ button that hides
    the palette, and a search field.
  - Centre: the title "Poly Grid / Poly Grid".
  - Right: zoom − / 1:1 / +, undock, and close.
- **Module palette** along the top.
  - 16 categories in a 4-column text grid: I/O, Display, Phase, Data, Oscillator, Random, LFO,
    Envelope, Filter, Shaper, Delay, Mix, Level, Pitch, Math, Logic.
  - A horizontal strip of module icons for the chosen category.
  - Typing in search replaces the categories with results.
- **Canvas.** A dark "blueprint" grid of cells, and modules are dark rounded tiles that snap to it.
  - **Module titles are coloured by category**: red for generic signal, turquoise for control.
  - **Ports are small rings coloured by signal type**: yellow logic, purple phase, red or turquoise
    untyped, blue secondary. A cable takes its source's colour.
  - Locked mode removes the palette and the grid lines, so the patch reads as a performance panel.
- **Gestures.**
  - Drag a module onto a port and it connects automatically.
  - Drag onto a cable and the module is inserted in line.
  - Drag onto another module to replace it, keeping settings and cables.
  - Hold a modifier while dragging a cable to insert an Attenuate or a Mixer.
  - Deleting a module heals the cable through it.
  - Double-click a port to remove its cables.
  - The footer lists these modifiers while you drag.
- **Help and scopes.**
  - F1 opens **interactive help**: the real module, live, with every parameter annotated.
  - The Inspector shows **live scopes per port** (§4.3).

---

## 8. The browser

- **Browser Panel.** A right-hand access panel, tall and narrow, about 214–228 px at 1× (`ug-13`).
  From top to bottom:
  1. A row of **Quick Sources** icons, remembered per context, jumped to with F1–F9.
  2. The source title with a ☆ favourites filter.
  3. A search field with a menu (settings, snapshots, smart collections).
  4. **Filter chips in two columns**: Location, File Kind, Category, Creator, Device, Tags. When a
     filter is active, the chip's text becomes its value, in blue ("Cymbal").
  5. The filter tree, with counts in dim grey after each name.
  6. A **splitter with a visible handle**.
  7. The results list, rows about 23 px, icon plus name; the selected row is slate `#2a333c`. The
     list **fades out at its top and bottom edges** to show it continues.
  8. The file area: name, then Length, Beats and sample rate; a waveform preview player with a
     volume fader; and an auto-preview speaker toggle.
- **Pop-up Browser.** Appears wherever you click a "+" (`ug-14`, `web-02`). It is a floating
  rounded window with a shadow and four columns:
  1. A quick-source rail.
  2. The source's big icon above a filter grid and tree.
  3. Results.
  4. File info: a description, then an **UPPERCASE grey key / white value metadata grid**
     (CATEGORY, CREATOR in orange as a link, DEVICE, TAGS, PACKAGE, COLLECTIONS as coloured rings).
     Below that come *Show Presets for …*, **Live Preview**, and Cancel / OK.
- **Visual browsers** lay results out as a grid instead of a list: curve thumbnails, 3-D wavetable
  stacks, impulse envelopes.
- **Keyboard first.**
  - Typing goes straight into search, and suggestions appear as a blue chip accepted with Tab.
  - ↓ moves into the results, and ↑ from the first result goes back to search.
  - Letter keys jump to filters: L location, F kind/type, C category/creator, D devices, T tags,
    V vendor, X clears, S goes to search.
  - Number keys file the selected item into collections, 0 for favourites.
  - → shows a device's presets, and → also plays or stops a preview.

---

## 9. Navigation and speed

- **The footer status line (`crop-06`, `ug-23`, `ug-24`) is the most useful single feature for
  learning the program.**
  - Hovering any object shows its name and state ("Solo: Off").
  - Then the gestures available, each as a **bold UPPERCASE modifier and a grey action**. The one
    whose modifier is held lights up.
  - It updates *during* a drag: "DRAG Drag layers · SHIFT+CLICK Select range · CMD+CLICK Add to
    selection · DOUBLE-CLICK Show devices".
  - Hovering a parameter shows its **full name, exact value and, for frequencies, the MIDI note with
    a tuning glyph** (˥˦˧˨˩ from quite sharp to quite flat), plus the modulated value in brackets.
- **Zoom and scroll.**
  - In the beat ruler, dragging up or down zooms and dragging sideways scrolls. The cursor turns
    into a magnifier.
  - Other zoom inputs: Ctrl+Alt with drag or wheel, a middle-button drag, pinch, and `+`/`−`.
  - The beat grid adapts to the zoom level; `,` and `.` step it, and `Alt+,` and `Alt+.` change the
    subdivision.
  - In v6 the scrollbar **shows an overview of the content** and the current grid ("[1/16]").
    Alt-clicking a cue marker in it zooms to fit; Alt-double-clicking starts playback there.
  - Follow-playhead scrolls by page or continuously.
- **Selection by keyboard.** ↑/↓ moves between tracks (Shift extends the selection). Alt+←/→ moves
  to the next note. Arrow keys move or select, depending on the object. Time selection is its own
  tool (`2`), and editor tools are on the number keys `1`–`8`. v6 adds `F` to cycle editors and
  `Shift+Tab` for the full-screen editor.
- **Drag and drop everywhere.** Drag between projects, into the device panel, from the browser onto
  a device to replace it, or a clip onto a panel-toggle icon to reach a hidden panel. Panel
  shortcuts work mid-drag.
- **Fine control and reset.** Every control supports Shift for fine adjustment (even mid-drag),
  Ctrl/Cmd-click to type a value, and double-click to reset. The rule is stated once and holds
  everywhere, so it never needs a label.
- **Anything can be made one click.** Any menu item can be pinned to the toolbar (§3.3).

---

## 10. What Cantus should take: ranked

Each item notes the egui approach and what "better than Bitwig" would mean.

### 1. Rebuild the Inspector on a two-column form grammar (highest value; addresses the user's main complaint)

- **One widget kit.** `Section` (UPPERCASE band) ▸ `Group` (a surface block with a 1 px rule
  between groups) ▸ `Row` (a right-aligned label column and a value well).
- **Metrics on Cantus's 4 px grid** (`design::space`):

  | Element | Size |
  |---|---|
  | Band height | 24 (`ROW`) |
  | Row pitch | 24 |
  | Well height | 20 (`ROW_DENSE`) |
  | Label-to-well gap | 8 |
  | Group padding | 4 |
  | Label text | Plex Sans 12 px, regular |
  | Values | **Plex Mono** 12 px, or Plex Sans with tabular figures |
  | Band text | 11 px semibold UPPERCASE, +6 % tracking |

- **Label column** = clamp(label widths, 36 %–44 % of panel width). Labels are right-aligned and
  truncated with "…" (never wrapped), with the full label in the tooltip and footer.
- **Wells.** One colour for all wells. Editable values are drawn in the accent colour, units stay in
  the well, and a thin level bar sits at the left edge for 0–1 values. There is no separate
  full-width fader under a caption. The well itself is the drag target: vertical drag, Shift for
  fine, double-click to reset, Ctrl-click to type.
- **Layout: a scrolling top stack and a pinned bottom block.** Implement with
  `TopBottomPanel::bottom(...).show_inside(ui)` for the pinned block (the track's mini channel
  strip: meter + fader column, pan, M/S/arm), and a `CentralPanel::show_inside` containing a
  `ScrollArea` for everything else. Paint bands and rules with the `painter` directly; don't use
  `ui.heading`/`ui.separator`.
- **Content, by selection.**
  - *Track*: name (italic when automatic) and colour; instrument; inserts as a list (dot + name +
    open + "+"); sends (name | Pre/Post | knob); routing (in/out choosers); the pinned strip.
  - *Clip*: position, length, loop.
  - *Note(s)*: count in the band, key, velocity, and so on.
- **Dim what doesn't apply; don't hide it.**
- **Better than Bitwig.**
  - **Width can be dragged** (Bitwig's can't), within limits: default 248, 200–360 px, snapping to
    the 4 px grid. The label column scales with it.
  - Sections collapse by clicking the band, and the state is remembered per selection type.
  - Tab/Shift-Tab move between wells and arrow keys nudge values.
  - Labels on every surface pass **AA 4.5:1** contrast in *both* the light and dark themes.

### 2. Adopt a strict luminance ladder and a colour-meaning table in `theme`

- Five surface steps: **void / well / base / surface / band**, plus a focus rim. Keep the steps
  evenly spaced, about 16–19 levels in the dark theme, and mirror the ladder for the light theme.
- One **accent** (active / value). One **info blue** (time, playhead, selection). Mono and poly
  modulation colours. Pre- and post-send colours. **Track colour for identity only.**
- Put the table in `docs/` and apply it everywhere. Much of Bitwig's "polish" comes from this rule
  being followed without exceptions.

### 3. A footer status line

- It shows the hovered object and value, then the available gestures as `MODIFIER action` pairs.
- Implementation: widgets write a `HoverHint` into `ctx.data_mut` on hover, and the footer paints
  it. A one-frame delay is fine, or draw the footer last on a foreground layer.
- Localise both locales, as the project requires.
- It lets controls drop inline labels and units, and teaches every gesture. Cheap in immediate
  mode, high payoff.

### 4. Fixed panel slots toggled from the footer, with drag-resizable edges

- Four slots, as in §3.1: a left inspector; a bottom secondary slot (device chain / mixer / piano
  roll, one at a time); a right access slot (browser / project); and a central view (Arrange / Mix
  / Edit). Each has a single-key toggle and a footer icon.
- Edges resize (`SidePanel::resizable(true)`, `TopBottomPanel::resizable(true)`); sizes are
  persisted and snapped to 4 px; double-clicking an edge restores the default.
- A focus rim on the active panel, and panel-scoped shortcuts.
- **Don't start with free-form docking.** `egui_dock` tabs could come later for power users; Bitwig
  shows fixed slots are enough and calmer. "Moving elements" should first mean choosing which panel
  fills which slot.
- **Better than Bitwig:** also allow the right and left slots to swap sides (a common request),
  and a "Mix" layout preset that puts the mixer centre and the inspector left.

### 5. Mixer: row-label column, section toggles, collapse by priority, fader fills the rest

- Fixes the user's "mixer can't be scrolled down" complaint.
  - Put a narrow left column of section names and toggles.
  - Each frame, add the sections that are switched on, in priority order, while they fit. Grey the
    toggles of those that didn't fit.
  - The strip's fader/meter section takes whatever height is left. **No vertical scroll.**
- **Horizontal.** Scroll with a wheel (Shift+wheel, or plain wheel over the header row) and with a
  scrollbar. Keep return and master strips pinned right.
- **Strip width.** Drag the header's right edge, with presets of about 64 / 82 / 110 px. Abbreviate
  send and insert names to fit.
- **Fader as a filled column** with a thin accent cap, and the dB readout in a well above it. It
  reads well and gives a large drag target.
- **Headers filled with the track colour**, with a colour bar at the foot of each strip.

### 6. Device chain panel for effects and instruments

- Equal-height cards in a horizontally scrolling chain.
- A vertical header strip on each card: power, preset, rotated name, expand. There is a "+" and a
  meter between cards.
- **Expand** opens the full editor in the central area or a floating viewport. `egui` 0.36 supports
  deferred viewports, but they cost more per platform, so ship docked-in-centre first.
- This gives Cantus's native effect and instrument editors one consistent frame instead of loose
  windows.

### 7. Knob and modulation visual rules

- **Knob:** body, pointer, and a value arc from the minimum (or from centre for bipolar values) in
  the accent colour, with a default tick. The label goes below, the value in the footer and on
  hover.
- **Modulation:** if and when Cantus adds modulators, show a *separate* ring segment in the
  modulation colour, and tint every possible target while a modulator is in mapping mode.

### 8. Browser layout (when Cantus grows one)

- Quick sources, then filter chips, then a tree with dim counts, then results, then a metadata grid
  with UPPERCASE keys. The list is keyboard-first: type to search, ↓ into results, digits to file
  into collections.

### 9. Typography rules

- UPPERCASE only for bands, view words and footer modifier keys.
- Tabular figures for every number.
- Italic grey for placeholder or automatic text.
- Counts in dim grey directly after names.
- Never centre a label that has a value beside it.

### What not to copy

- **A fixed-width inspector.** The user explicitly wants to drag edges.
- **Text this small by default** (about 11 px at 1×, and the v6 hero runs about 10 px). Bitwig
  needed Contrast, Midtone and Black-level settings to compensate. Cantus should default to 12 px,
  keep ≥ 4.5:1 for labels, and respect the UI scale.
- **Dark-only.** v6 dropped the choice. Cantus defaults to a light theme; keep both, with the same
  ladder logic mirrored.
- **Leaning on right-click menus and pinning** for discoverability. Pinning is powerful but complex.
  Take the footer hints first; consider pinning later.
- **Rotated device names** for long names. They're fine for short names; Cantus's instrument names
  can be long, so allow a horizontal title when a card is wide enough.
- **Icon-only tabs** (the inspector's four device, remote and modulation tabs). They are opaque
  without the footer hint. Pair them with short text or a tooltip.
- **Early Bitwig's left-aligned labels** (`ug-05`). Bitwig itself abandoned them.

---

## 11. Quick reference: measured sizes at 1×

| Element | Size |
|---|---|
| Window header (project tabs) | ≈ 32 px |
| Menu/transport bar | ≈ 44–48 px. Buttons ≈ 34 × 31–36 px, grouped into rounded clusters with 1 px dividers |
| Footer | ≈ 34 px |
| Inspector width | ≈ 157–160 px (fixed) |
| Browser Panel width | ≈ 214–228 px (resizable) |
| Device panel height | ≈ 215–240 px. Device header strip ≈ 28 px wide |
| Gap between panels | ≈ 3 px, with a 1 px border and a focus rim |
| Inspector section band | ≈ 22 px |
| Inspector row pitch / well height | 24 / 21 px |
| Inspector label column / value column | ≈ 62 / 86 px (≈ 39 % / 54 %) |
| Colour swatches | ≈ 15 px, grid of 9 × 3 |
| Mixer strip | ≈ 82 px default (106–118 px when widened). Header ≈ 46 px |
| Mixer row-label column | ≈ 82 px |
| Arranger track header | Colour stripe ≈ 7 px |
| Browser result row | ≈ 23 px |
| Knob | body ≈ 24–26 px, ring ≈ 34 px. Big knobs ≈ 48–50 px |

---

## 12. Sources

### Screenshots

In `research/ui/screenshots/bitwig-studio/`. User-guide images are under
`https://assets.bitwig.net/media/bitwig_userguide/18/images/` (spaces are `%20`). The page each
appears on is under `https://www.bitwig.com/userguide/latest/`.

| File | What it shows | Image path (user guide) or URL | Page |
|---|---|---|---|
| ug-01-blank-project-full-window.png | Whole window at 1× (v2): inspector, arranger, device panel, browser, footer | `03arrange/SRC2 - v2.png` | intro_to_tracks |
| ug-02-inspector-track.png | Track Inspector (v5.1), window header, footer | `03arrange/10 - TRACK INSPECTOR - v5.1.png` | meet_inspector_panel |
| ug-03-inspector-arranger-clip.png | Arranger-clip Inspector rows | `04clips/400 - Clip Inspector v4.0.png` | the_inspector_panel_on_arranger_clips |
| ug-04-inspector-note-events.png | Note Inspector | `10note-clips/20 - INSPECTOR - v2.4.png` | inspecting_note_clips |
| ug-05-inspector-histograms.png | NOTES (18), histogram buttons, left-aligned labels (old) | `10note-clips/29 - HISTOGRAMS.png` | inspecting_note_clips |
| ug-06-inspector-track-mixing.png | Track Inspector with devices, sends and strip; mixer with device lists | `06mixer/622 - INSPECTOR MIXER - v5.1.png` | other_mixing_interfaces |
| ug-07-inspector-fx-track.png | FX-track Inspector, pre-fader cue, narrow strips | `06mixer/623 - FX TRACK INSPECTOR - v4.3.png` | other_mixing_interfaces |
| ug-08-mixer-big-meters.png | Full mixer anatomy with every section and the row-label column | `06mixer/2b - MIXER ANATOMY - v5.1.png` | the_mix_view |
| ug-09-secondary-mixer-small.png | Secondary mixer when short: sections dropped, toggles greyed | `06mixer/15 - SECONDARY MIXER SM - v5.1.png` | other_mixing_interfaces |
| ug-10-secondary-mixer-large.png | The same mixer resized taller | `06mixer/15a - SECONDARY MIXER LG - v5.1.png` | other_mixing_interfaces |
| ug-11-mixer-sends.png | Send section, pre/post colours | `06mixer/616 - SEND BIG - v4.3.png` | the_mix_view |
| ug-12-mixer-device-mini-views.png | Mini EQ and gain-reduction displays in the device list | `06mixer/3200 - DEVICE MINI VIEWS - v3.2.png` | the_mix_view |
| ug-13-browser-panel.png | Browser Panel | `04browser/20 - Browser Panel full - v5.png` | browsers |
| ug-14-popup-browser.png | Pop-up Browser | `04browser/21 - Pop-up Browser full - v5.png` | common_browser_elements |
| ug-15-expanded-device-docked.png | EQ-5 Expanded Device View docked in the centre | `07devices-intro/6c - EXPANDED DOCKED - v2.3.png` | introduction_to_devices |
| ug-16-inspector-device.png | Device Inspector, general elements | `14devices-advanced/199 - DEVICE INSPECTOR PANEL GENERAL ELEMENTS - v3.png` | the_unified_modulation_system |
| ug-17-inspector-modulation-sources.png | Modulation-sources tab and modulator tiles | `14devices-advanced/1 - MODULATOR SOURCE VIEW - v3.3.png` | the_unified_modulation_system |
| ug-18-modulation-button-on.png | Modulated values shown in cyan | `14devices-advanced/17b - BUTTON A ON - v2.png` | the_unified_modulation_system |
| ug-19-polysynth-mod-routing.png | Polyphonic (green) routing, tooltip | `14devices-advanced/21 - ONBOARD MODULATION ROUTING - v2.png` | the_unified_modulation_system |
| ug-20-grid-editor.png | Grid editor, Grid device Inspector, overview | `15grid/1 - OPENED GRID EDITOR - v3.png` | welcome_to_the_grid |
| ug-21-grid-inspector-scopes.png | Per-port scopes in the Inspector | `15grid/18 - INSPECTOR SCOPES - v3.png` | welcome_to_the_grid |
| ug-23-footer-status-message.png | Footer "available actions" | `02anatomy/12 - STATUS MESSAGE - v2.4.png` | the_window_footer |
| ug-24-footer-param-info.png | Polysynth device, footer parameter info | `02anatomy/310 - FOOTER CUTOFF - v3.1.png` | the_window_footer |
| ug-25-device-panel-track-headers.png | Project / group / track headers in the device panel | `07devices-intro/502 - Track headers - v5.png` | introduction_to_devices |
| ug-26-mixer-level-popup.png | Peak/RMS hover popup | `06mixer/14a - LEVEL POPUP - v5.1.png` | the_mix_view |
| ug-27-inspector-voice-stacking.png | Instrument Inspector: voices, stacking, dimmed Retrigger | `14devices-advanced/600 - VOICE STACKING ON - v5.1.png` | the_unified_modulation_system |
| web-01-bws6-full-gui-hero.png | v6 Arrange view: inspector, launcher, arranger, device chain | https://assets.bitwig.net/media/image/bitwig-studio_bws6_full-gui-hero/Bitwig-Studio_BWS6_Full-GUI-Hero-LG-HD.png | https://www.bitwig.com/overview/ |
| web-02-bws6-arranger-browser.png | v6 arranger with Pop-up Browser and devices | https://assets.bitwig.net/media/image/bitwig-studio_bws6_gui-arranger-browser/Bitwig-Studio_BWS6_GUI-Arranger-Browser-HD.png | overview |
| web-03-bws6-launcher-mixer.png | v6 Mix view with the launcher | https://assets.bitwig.net/media/image/bitwig-studio_bws6_gui-launcher-mixer/Bitwig-Studio_BWS6_GUI-Launcher-Mixer-HD.png | overview |
| web-04-bws6-note-editing.png | v6 Edit view; Inspector top and bottom stacks; hazard stripes | https://assets.bitwig.net/media/image/bitwig-studio_bws6_note-editing/Bitwig-Studio_BWS6_Note-Editing-HD.png | overview |
| web-05-bws6-modulation-polysynth.png | Modulation mapping mode (device tinted) | https://assets.bitwig.net/media/image/bitwig-studio_bws6_modulation_polysynth/Bitwig-Studio_BWS6_Modulation_Polysynth-HD.png | overview |
| web-06-bws6-polymer.png | Polymer (a synth built on the Grid) | https://assets.bitwig.net/media/image/bitwig-studio_bws6_polymer/Bitwig-Studio_BWS6_Polymer-HD.png | overview |
| web-07-bws6-eqplus.png | EQ+ device | https://assets.bitwig.net/media/image/bitwig-studio_bws6_eqplus/Bitwig-Studio_BWS6_EQplus-HD.png | overview |
| web-09-bws6-automation-editing.png | v6 automation lanes (two-line name/path headers), right-edge tool palettes, Browser Panel on the right, detail editor below | https://assets.bitwig.net/media/image/bitwig-studio_bws6_automation-editing/Bitwig-Studio_BWS6_Automation-Editing-HD.png | overview |
| web-10-bws6-compressorplus.png | Compressor+ device card | https://assets.bitwig.net/media/image/bitwig-studio_bws6_compressorplus/Bitwig-Studio_BWS6_Compressorplus-HD.png | overview |
| crop-01…09 | Measurement crops (inspector, inspector at 3×, toolbar, mixer strip, footer, track headers, device panel, group headers) | Cut from web-01 and web-03 above | – |

### Text sources

- Bitwig Studio User Guide (latest), https://www.bitwig.com/userguide/latest/. Chapters used:
  - a_musical_swiss_army_knife (panels, views, display profiles)
  - user_interfacing
  - anatomy_of_the_bitwig_studio_window, the_window_footer, the_window_menus_transport_area,
    the_window_body
  - the_arrange_view_and_tracks, intro_to_tracks, meet_inspector_panel
  - browsers, common_browser_elements, customizing_the_browsers
  - the_inspector_panel_on_arranger_clips, inspecting_note_clips, the_edit_view
  - the_mix_view, other_mixing_interfaces
  - introduction_to_devices, working_with_devices, advanced_device_concepts,
    the_unified_modulation_system
  - welcome_to_the_grid, on_grid_signals
  - the_dashboard, working_on_a_tablet_computer
- Release notes:
  - Bitwig Studio 6.0: https://downloads.bitwig.com/6.0/Release-Notes-6.0.html
  - Bitwig Studio 6.1: https://downloads.bitwig.com/6.1/Release-Notes-6.1.html
  - Bitwig Studio 5.0.9: https://downloads.bitwig.com/5.0.9/Release-Notes-5.0.9.html
- "Bitwig Studio 6: On Another Level":
  https://www.bitwig.com/stories/on-another-level-bitwig-studio-6-is-out-now-416/
- Sound On Sound review of Bitwig Studio 6: https://www.soundonsound.com/reviews/bitwig-studio-6
- Font evidence: https://github.com/jhorology/bitwig-studio-japanese-font (install script lists
  the `SourceSansPro-*.ttf` files extracted from `bitwig.jar`)
