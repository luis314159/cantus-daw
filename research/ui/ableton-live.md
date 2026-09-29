# Ableton Live 12: UI research for Cantus

Research only. Sources are Ableton's Live 12 reference manual, the Live 12 release notes and
the ableton.com product pages. The screenshots are in `research/ui/screenshots/ableton-live/`
(git-ignored). Their source URLs are listed at the end.

## How the numbers were measured

- The Live 12 manual screenshots were captured on a Retina Mac at **2×**. The release notes
  say the track-colour stripe under each mixer strip is "6-pixel-high", and it measures
  12 px in `session-view-mixer.png`. **Every size below is a logical pixel at Live's 100 %
  zoom**, which is the measured value divided by 2. Sizes were taken from pixel runs with PIL,
  not estimated.
- Many manual screenshots use an annotation style: everything except the subject is covered
  by roughly 45 % white, and the subject gets a black outline. Colours were taken only from
  the uncovered subject, or from screenshots that have no veil (the device shots, the mixer
  shots and the product images). The veil was cross-checked by inverting it. For example, a
  veiled `#bababa` inverts to `#818181`, which is the value the unveiled Control Bar shows.
- The product-page images are JPEG-like, so their colours are accurate to about ±3 per
  channel. They were sampled as medians over flat areas.
- Two product images (`product-session-view.png`, `product-arrangement-view.png`) come from
  the "What is Live" page and show **Live 11**. You can tell from the word "Master" and the
  pre-12 mixer. They are kept only for comparison.

---

## 1. Visual language

### 1.1 The idea in one sentence

Live is **flat grey chrome in a few steps of luminance, carrying three signal colours
(amber, cyan, track colour) and one dark "display" colour**. Hierarchy comes from placement
and tiling, not from borders, shadows, gradients or type size.

### 1.2 The surface ladder (Light, Neutral tone: the manual's theme)

The same values show up in the unveiled device shots, the browser shot and the product shot
titled "lavender-light", so they are the shipping Light theme.

| Role | Hex | Where |
|---|---|---|
| Desktop / window ground | `#818181` | the 7 px gutters between views, the Control Bar, the Status Bar |
| View surface | `#a5a5a5` | browser, Session and Arrangement backgrounds, Clip View, device bodies, Info View body |
| Raised header | `#bcbcbc` | device title bars, browser column header ("Name"), Info View title, the gaps *between* sub-panels inside a device |
| Field | `#dfdfdf` (choosers, number boxes); `#cfcfcf` (toggle off) | always with a 1 px `#4f4f4f` outline |
| Control Bar field | `#a5a5a5` on `#818181` | no outline, because the step in luminance is the edge |
| Ink (all text) | `#121212` | labels, values, names |
| Secondary ink | `#4f4f4f` | browser section headers ("Collections", "Library", "Places") |
| Display inset | `#242424`, grid `#343434`, axis labels `#797979`, spectrum fill `#565656` | EQ Eight, Compressor, Wavetable graphs |
| Selection (list row) | `#b0c5ca` / `#aac3c4` | browser selection: a desaturated cyan-grey, not a saturated highlight |
| "Filters active" bar | `#fadf82` | browser Results bar |

**Light, Cool tone** (the manual's mixer and Drift captures): strip surface `#a2a5ad`, strip separator `#6a6e79` (2 px), selected strip
`#babcc3`, field `#dedfe3` with a `#4c4f5b` outline, meter trough `#21242d`. In Drift the
body is `#9b9fa9`, the title bar `#b4b6bf` and the display `#1f232d`; the text is `#10121a`.

**Dark, Neutral** (product image "mixer improvements"): surface `#353535`, deepest background
`#2b2b2b`, number box `#222222`, meter trough `#191919`, primary text `#d4d4d4`, scale and
ruler text `#a1a1a1`–`#ababad`, amber `#f3a84e`, meter green `#16f05c`.

**Dark, Cool** (product image "stacked views"): chrome `#252932`, surface `#32363f`, tab
`#42464f`, display `#14171c`, text `#c3c5d1`–`#d0d4dd`, loop amber `#fead58`.

### 1.3 Contrast: strong for text, weak between surfaces

Measured WCAG ratios:

| Pair | Ratio |
|---|---|
| ink `#121212` on surface `#a5a5a5` | **7.6 : 1** |
| ink on raised header `#bcbcbc` | 9.9 : 1 |
| ink on field `#dfdfdf` | 14.1 : 1 |
| Drift ink `#10121a` on `#9b9fa9` | 7.1 : 1 |
| dark-theme text `#c3c5d1` on `#32363f` | **7.1 : 1** |
| dark-theme scale text `#ababad` on `#353535` | 5.4 : 1 |
| cyan `#07edff` on display `#242424` | 10.8 : 1 |
| amber `#feb902` on display `#242424` | 9.0 : 1 |
| surface `#a5a5a5` on desktop `#818181` | 1.6 : 1 |
| raised `#bcbcbc` on surface `#a5a5a5` | 1.3 : 1 |
| dark surface `#32363f` on chrome `#252932` | 1.2 : 1 |
| cyan `#07edff` on light surface `#a5a5a5` | **1.7 : 1**, which Live avoids (see below) |

Three rules follow from these numbers:

- Primary text sits at about 7:1 in every theme. Live does not push it to black on white.
- Surfaces differ by only 1.2–1.6:1, and that is enough because they are big, flat areas.
  No panel ever has an outline.
- Signal colours are used where they contrast. **Cyan text only appears on a dark display.**
  On light surfaces cyan is a *fill*, such as the filled part of a number slider or a knob's
  value arc, with black text drawn over it.

### 1.4 Colour carries meaning (and nothing else does)

- **Amber** (`#feb902`, and `#f5bb41` in the cool variant) means **on or active**: the
  device activator dot, track activators (the numbered buttons), Loop, the "Auto" monitor
  state, Post sends, "Adapt. Q On", EQ band markers, the Clip View "Loop" bar.
- **Cyan** (`#07edff` / `#00e5f6`) means **a parameter's value**: knob value arcs, slider
  fills, the EQ curve, the Compressor threshold line, envelope curves, readouts inside
  displays.
- **Red / pink** (`#f87680`) means **record-armed**, and is also Drift's Noise section.
- **Green** means playing and metering. The meter gradient starts at −16 dB and runs to
  0 dB, according to the 12.0 release notes.
- **Purple "signifies scale awareness across Live"**, in the manual's own words. It is
  used for the scale toggle and for highlighted scale rows in the piano roll.
- **Track colours** mean identity only: track title bars, clips, and the 6 px stripe under
  each mixer strip.
- **Inside an instrument, colour ties each source to its controls.** Drift's Osc 1 is amber
  and Osc 2 is cyan, and their mix toggles "1" and "2" use the same colours.
- Clip and track colours are light pastels of fairly high chroma (for example `#d2e498`,
  `#e0feea`, `#cb9dd6`, `#faf696`), with black text on them. On a grey ground this is where
  most of Live's "beauty" comes from. The chrome is deliberately colourless so that the
  user's content is the colour.

### 1.5 Depth

- **Flat.** There are no gradients except the meter's, no bevels, and no drop shadows
  inside the layout. In the manual shots the only shadow is the annotation's.
- Since Live 12, "the borders around all of Live's views have been removed" and "all main
  views now have a slight rounding at their edges" (release notes). The radius measures
  about **3 px**. Views float on the `#818181` desktop with **about 7 px gutters**.
- Depth inside a device is made by **tiling**. Sub-panels are rectangles of the body colour
  separated by **2 px gaps of the lighter title-bar colour** (EQ Eight's band cells: `#a5a5a5`
  cells, `#bcbcbc` gaps). Groups are never outlined.
- The one dramatic contrast is the **dark display inset** (`#242424`/`#1f232d`), used for
  anything that is a picture of the signal: EQ curves, envelopes, wavetables, compressor
  history, and filter responses.

### 1.6 Density and empty space

- Rows are 17–19 px and controls 13 px tall (details in section 2.6). A 1469×917 window shows
  a 15-track Session, a 13-strip mixer with sends, and a full Drift instrument together
  (`control-bar.png`).
- Empty space is used only as **drop zones** that say what they are for ("Drop Audio
  Effects Here", "Drop Files and Devices Here", "Drop Instrument Here"). Otherwise every
  area is filled edge to edge, and the space between items inside a device is 2–4 px.

### 1.7 Iconography

- Icons are tiny monoline pictograms, about 10 px with 1 px strokes. They appear in the
  browser sidebar (All, Sounds, Drums, and so on), on the device title bar (hot-swap, save,
  "…" menu) and on the Control Bar.
- Filled triangles are used for play, fold and the fader handle. The filled circle is the
  device activator.
- **Text is preferred over icons wherever a word is short**: "S" (solo), "In/Auto/Off",
  "Post", "Key", "MIDI", "H"/"W" (optimise height/width), "Set", "Del", "Raw".

---

## 2. Typography (the part that matters most)

### 2.1 Typeface

Live uses **Ableton Sans**, a custom face by Letters from Sweden. It first appeared on Push 2,
has been in Live since Live 10, and is not sold. Ableton staff said on the forum that Live 12
still uses "the Ableton Font", though it may render "a bit thinner in L12".

### 2.2 One size, one weight

| Measure (logical px at 100 %) | Value |
|---|---|
| Cap height | **7.0 px** (14 px in the 2× capture), the same for track names, knob labels, values, browser rows, device titles, menu items and the Info View |
| x-height | **5.0–5.5 px**, a ratio to cap height of about 0.73 (large, which suits UI text) |
| Implied nominal size | about 10 px |
| Weights | effectively **one** (a regular that reads slightly medium). Hierarchy never uses bold |
| Letter spacing | default (0). No tracking, no small caps |

Section headers ("Osc Mix", "Filter", "Envelopes" in Drift; "Collections", "Library",
"Places" in the browser) are **the same size** as everything else. They are set apart only by:

- **position**: centred in the title-bar row above their column in Drift, or on a line of
  their own with a gap above in the browser;
- **colour**: in the browser, `#4f4f4f` instead of `#121212`.

The only larger type in the main window is the grey grid-resolution readout ("1/2", "1/8")
in the Arrangement's bottom-right corner, drawn at about 2× in a light grey. In-app menus use
the same 7 px cap height too; their hover row is a pale cyan (`#d5f7fe` in the Cool
variant). Uppercase appears almost nowhere; "STAGE 2" in Roar's Modulation Matrix is the
lone exception.

**IBM Plex Sans, which Cantus bundles, is almost exactly Ableton Sans's metrics at 10 px.**
Measured from Cantus's own `IBMPlexSans-Regular.ttf`: cap height 0.698 em and x-height
0.516 em, which gives cap 6.98 px and x-height 5.16 px at 10 px. Plex's digits are also
**tabular by default**: all ten digits have a 600-unit advance, in Regular and SemiBold alike.

### 2.3 Case and wording

- **Title Case for control labels**: "Dry/Wet", "Freq Mod", "PB Range", "Vel > Vol",
  "Audio To", "Track Delay".
- **Sentence case for prose**: Info View text, the Status Bar, menu commands.
- **Abbreviate before you shrink.** Common forms are "Freq", "Res", "HP", "Oct", "Dly",
  "Adapt. Q", "Look.", "Env.". In narrow mixer strips the labels themselves change:
  "MIDI From" becomes "M. Fr.", "Monitor" becomes "Mon.", "Audio To" becomes "A. To"
  (`mixer-tall-meters-wide-strips.png`).
- **Text never wraps and never overflows.** A track name is hard-clipped at its cell edge
  with no ellipsis ("The Beat Of Histo", "Vocals Doubl"). A chooser clips its value before
  the ▼ ("All Channe▼").

### 2.4 How labels attach to controls: there are no loose words

Every word in Live is **inside a control, or directly bound to one on a shared axis**. The
five patterns, all measured:

1. **Knob cell** (label / knob / value, all centred on one vertical axis). Drift's envelope
   row puts four cells on a 48.5 px pitch. The label baseline sits 2–4 px above the knob and
   the value 1–3 px below it (Attack / Decay / Sustain / Release in `device-drift.png`).
2. **Field cell** (label above-left, field below, left edges aligned). The mixer's
   "MIDI From" over its "All Ins ▼" chooser, and Wavetable's "Volume" over the "−17 dB"
   slider field.
3. **Text in the control**: "Peak/RMS/Expand", "In/Auto/Off", "Hz/ms", "Classic / Ensemble
   / Vibrato". Each of these is a radio group of 13 px buttons with 3 px gaps.
4. **Number slider** (the value *is* the control). A 13 px field whose left part fills with
   cyan in proportion to the value, with the number centred over it: Drift's "35 %" and
   "7.2 %", EQ Eight's "Scale 100 %", Wavetable's "Amount 36 %". It removes the knob where
   space is tight.
5. **Inline readout in a display.** Dark header and footer strips hold "label value" pairs,
   with the label in grey and the value in accent. Compressor shows
   "Thresh **−11.9 dB** · ■ GR · □ Output · Out **0.00 dB**" across the top and
   "Knee **0.0 dB** · Look. **10 ms▾** · Env. **Log**" along the bottom. Wavetable's filter
   section shows "Res 91 % · Drive 4.69 dB".

Why this reads as "set" rather than "thrown down":

- **One size and one colour**, so no word pops out of its group.
- **A column grid.** Cells in a row share a pitch, and labels and values share the control's
  centre axis.
- **Groups are containers, not headings.** A section is a tile (a lighter-gapped panel or a
  dark display), and its name sits in the title-bar row, not floating above loose widgets.
- **Tight vertical rhythm.** There is no extra item spacing: label, control and value stack
  with 1–4 px between them.
- **Narrow space means shorter words**, never overlap. This is the fix for the user's
  complaint that "the letters run into each other".

### 2.5 How numbers are set

- **About 3 significant digits with auto-ranging units**: "3.57 ms", "124 ms", "4.35 s",
  "455 Hz", "1.82 kHz", "36 %", "−6.0 dB", "0.00".
- **A space before every unit, including %**: "36 %", "100 %".
- Silence is shown as "−∞" and an infinite ratio as "inf : 1".
- **Bars.beats.sixteenths use segmented fields**: "1. 1. 1" and "9. 1. 1", right-aligned in
  fixed slots, and each segment can be dragged on its own.
- Values are centred under knobs and centred in fields. The digits are the UI face, not a
  monospace face; the rhythm comes from fixed formats and fixed cell widths.

### 2.6 Size grid (logical px)

| Element | Size |
|---|---|
| Control height (chooser, number box, toggle, S / arm button) | **13** (a 12 px fill plus a 1 px outline) |
| Control Bar field | **17** tall, in a **29** px bar |
| Track title bar, device title bar, browser column header | **17** |
| List / browser row pitch | **19** |
| Session clip-slot pitch | **18** (17 + 1 line) |
| Arrangement folded track pitch | **19** (17 + 2 separator) |
| Gap between toggle buttons | 3 |
| Gap between sub-panels inside a device | 2 |
| Gutter between main views | about 7 |
| Small knob diameter | about **24** (mixer pan, Drift envelopes) |
| Large knob diameter | about **34–36** (Drift Freq, Wavetable Frequency) |
| Knob stroke | 2 px open arc (about 300°, gap at the bottom) plus a 2 px needle drawn from about 40 % radius outward. No knob body, no fill |
| Device activator dot | about 12 |

---

## 3. Layout and panels

### 3.1 The single window

The layout below is measured from the 2× full-window shots at 1469×917 logical px
(`clip-and-device-view-stacked.png`, `control-bar.png`):

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ Control Bar  (29 px; 9 groups: browser toggle · Link/Tap/Tempo/Sig · scale ·     │
│  Follow+position · transport · automation/capture/session-rec · loop/punch ·     │
│  draw/key/MIDI map · CPU/sample-rate · Session|Arrangement selector)             │
├──────────────┬───────────────────────────────────────────────────────────────────┤
│ Browser      │ Main view: Session  ⇆  Arrangement   (Tab)                        │
│  sidebar 100 │  Session: track columns 76–95 px, clip slots 18 px                │
│  + content   │  Arrangement: overview + beat ruler + tracks + track controls     │
│  ≈190        │ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │
│ (drag edge)  │ Mixer (inside the main view; drag its top edge; 76–95 px strips) │
├──────────────┴───────────────────────────────────────────────────────────────────┤
│ Detail View: Clip View (≈290) stacked on Device View (≈187)  — drag the splitter │
├───────────────┬──────────────────────────────────────────────────────────────────┤
│ Info View box │                                                                  │
│ (above status)│                                                                  │
├───────────────┴──────────────────────────────────────────────────────────────────┤
│ Status Bar (≈30): Info toggle · message line · clip/device selectors · mixer ▾   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**The regions are fixed. Nothing docks, floats or rearranges.** Only plug-in windows float,
and a Second Window (Ctrl/Cmd+Shift+W) can hold the other main view on a second display.
Flexibility comes from three mechanisms:

1. **Show/hide every region.** Since Live 12, the toggle for each region sits on the
   **outer corner nearest to it**:
   - the Browser toggle is at the top-left of the Control Bar;
   - the Info View toggle is at the bottom-left;
   - the Clip/Device View selectors and the Mixer View toggle (with its ▾ section menu) are
     at the bottom-right (`view-selectors-bottom-right.png`);
   - the Session/Arrangement selector is at the top-right.

   Each toggle also has a shortcut: Ctrl/Cmd+Alt/Opt + B or 5 (Browser), 3 (Clip), 4
   (Device), M (Mixer), I (In/Out), S (Sends), R (Returns), O (Overview), 6 (Groove Pool),
   7 (Learn View), and "?" for the Info View.
2. **Resize by dragging edges.** The splitters are:
   - the browser's right edge;
   - the Detail View's top edge;
   - the mixer's top edge ("dragging upwards on the top of the mixer will extend the height
     of the track meters");
   - Session track widths, dragged at the title-bar edge (Alt/Opt-drag resizes all tracks);
   - Arrangement track heights, dragged at the split line or with Alt/Opt +/−;
   - the Learn View splitter on the right.

   Resize cursors (double arrows) show where each one is. Since 12.x the Arrangement *zooms*
   instead of pushing content off-screen when the browser or the window is resized.
3. **Stack views instead of tabbing them.** The Clip and Device Views can be shown one at a
   time (Shift+Tab switches between them) or **stacked** so you can edit notes while you
   turn the instrument's knobs. The triangle toggles next to their selectors control this.
   This was a headline Live 12 feature.

Global zoom is 50–200 % under Settings › Display & Input, set per window, and Ctrl/Cmd +/−
also works. The whole UI scales uniformly, so Live never has to design a second density.
A **"Outline View in Focus"** option draws a frame around whichever view has keyboard focus.
The theme token for it is `MainViewFocusIndicator`.

### 3.2 Browser

The browser has three parts (`browser-layout-annotated.png`):

- **Sidebar** of labels (Collections / Library / Places). Section headers are in `#4f4f4f`,
  items are an icon plus a name, and the row pitch is 19.
- **Content pane**, which contains:
  - a search field with back/forward history;
  - filter groups of *tag chips*. Plain text means available, a lighter chip means
    selected, amber means the active group;
  - an amber **Results bar** ("Results · 2 Filters · Clear");
  - a sortable "Name" column with a `#bcbcbc` header;
  - a tags footer;
  - a **preview strip** with a waveform scrub and a "Raw" toggle.
- A Tag Editor column that can be shown or hidden.

Selection is `#b0c5ca`. Double-click or Enter loads an item onto the selected track, or
creates a track if none is selected.

### 3.3 Detail View: Clip View

- **Left side:** narrow property panels switched by icon tabs (Clip / Launch / Pitch & Time /
  Transform / Generate). They can be laid out side by side or stacked vertically when space
  is short (`clip-view-panels-vertical.png`).
- **Panel layout:** a 2-column grid of "Label (Set)" rows, each over a segmented field
  ("Start (Set) End (Set)", "1. 1. 1 | 9. 1. 1"). Full-width pill buttons ("Duplicate") and
  an amber full-width "Loop" bar.
- **Editor:** takes the rest of the width. A small toolbar row sits on top (Fold, Scale,
  Highlight Scale, Notes/Envelopes/MPE tabs).

### 3.4 Detail View: Device View (the horizontal chain)

- One row of devices, **all at the same height** (about 187 px: a 17 px title bar plus a
  body of about 169), each at its own width:

  | Device | Width (logical px) |
  |---|---|
  | Glue Compressor | 278 |
  | Compressor | 375 |
  | EQ Eight | 486 |
  | Drift | 818 |
  | Wavetable | 982 |

- Signal flows left to right. The chain scrolls horizontally, and since 12.0 scroll bars
  appear only while scrolling.
- A drop zone ends the chain ("Drop Audio Effects Here").
- **Stereo meters sit between devices** (`meters-between-devices.png`), so a dead link in
  the chain is visible without opening anything.
- **Folding.** Double-clicking a title bar folds the device into a narrow vertical strip:
  activator dot on top, the name rotated 90°, and hot-swap / save / menu icons at the
  bottom (`devices-folded.png`, about 30 px wide). Roar and the folded Wavetable also use
  rotated side labels ("Sidechain", "MIDI In", "Modulation").
- **Expanded views** open *above* the Device View at full width, for EQ Eight's big display
  and Roar's gain stages and matrix (`device-eq-eight-expanded.png`,
  `device-roar-expanded.png`). EQ Eight's expanded view also shows all eight bands' knobs
  in the device.
- Every built-in device has an **A/B compare**: the P key, and "(B)" is shown in the title.

### 3.5 Info View

This is a docked help box at the bottom-left (about 240×240 px in `info-view.png`), with a
`#bcbcbc` title row ("Info View") and a `#a5a5a5` body in the same 7 px cap type. It shows
the **name and function of whatever the mouse is over**, followed by its shortcut in
brackets ("[?] Show/Hide Info View"). Users can write their own info text for tracks, clips
and devices. The Status Bar shows transient messages and values on hover, such as
"Note Selection Time: 1.1.2 – 1.1.3 Pitch: G3 Velocity: 65".

### 3.6 Keyboard navigation (Live 12)

- **Focus areas:**

  | Alt/Opt + | Focus moves to |
  |---|---|
  | 0 | Control Bar |
  | 1 | Session |
  | 2 | Arrangement |
  | 3 | Clip View |
  | 4 | Device View |
  | 5 | Browser |
  | 6 | Groove Pool |
  | 7 | Learn View |
  | 8 | selected clip panel |
  | Shift+P | clip panels |
  | Shift+M | Arrangement mixer |

  There is also a **Navigate** menu (Alt+N) that lists them.
- **Within an area**, with "Use Tab to Move Focus" enabled:
  - Tab / Shift+Tab move to the next or previous control;
  - Ctrl+Tab (Win) or Opt+Tab (Mac) moves to the *same control on the next track*, the
    "neighbour" command;
  - "Wrap Tab Navigation" is optional.
- **Esc** moves from a slot, lane or mixer control to that track's title bar.
- **Arrow keys** move between title bars, slots and scenes. PgUp/PgDn move eight scenes at
  a time. In the Arrangement, arrows move by grid step and Ctrl/Opt+arrows jump to clip
  edges or locators.
- The Control Bar **remembers its last-focused control**.
- **Values:** ↑/↓ nudge, Shift+↑/↓ adjusts coarsely (octaves) or finely, typing digits
  enters a value directly, Delete resets to the default (so does double-click), Enter
  confirms and Esc cancels.
- **Zoom:** + and − zoom around the selection, Z zooms to the selection and X zooms back
  (repeatable), H and W fit height and width, Ctrl/Cmd+scroll zooms, Alt/Opt+scroll zooms
  one track vertically, Shift+scroll scrolls horizontally, and a held middle button pans
  any view.
- **Momentary latch.** Holding a single-key shortcut (A, B, S, Z, F1–F8, Tab) for more than
  about 500 ms makes it momentary: the state reverts when the key is released.
- **Single-letter shortcuts still work while the computer MIDI keyboard is on** if Shift is
  added.

---

## 4. The mixer

### 4.1 Where it lives

- **Session View**: the mixer sits under the clip grid, and each strip is the bottom of its
  track column. Strips share the column width and the horizontal scroll, so a track header,
  its clips and its strip are always aligned.
- **Arrangement View** (new in 12.0): the same mixer opens under the arrangement, toggled
  with Ctrl/Cmd+Alt+M. Its strips scroll horizontally on their own, independent of the
  vertical track list. The mixer can be focused separately (Shift+Alt+M) and has its own
  focus frame.
- **Arrangement Track Controls**: a compact per-track mixer on the right of each
  arrangement lane (activator number, S, arm, volume, pan, meter). It is configured on its
  own through View › Arrangement Track Controls. When a track is made taller, **more
  controls appear in the header** (input chooser, monitor, output, sends), so vertical space
  is spent on detail only when the user asks for it
  (`arrangement-mixer-track-controls.png`).

### 4.2 Strip anatomy, top to bottom

Measured from the 2× `session-view-mixer.png` (Light, Cool tone):

| Section | Contents | Height |
|---|---|---|
| Track title bar | track colour fill, black name, fold ▾ for groups | 17 |
| *(Session: clip slots and the stop button)* | | 18 per slot |
| **In/Out** | "MIDI From" label; input chooser (13); channel chooser (13); "Monitor" label; In/Auto/Off radio (13); "Audio To" label; output chooser (13); a spare chooser (sub-routing) | ≈123 |
| **Sends** | "Sends" label; send knobs A, B… staggered in 2 columns (≈24 dia); return strips show Pre/Post toggles instead | ≈55 |
| **Volume** | peak-hold pill "−∞" (35×13, rounded; click resets); volume number box "−3.0" (35×13); **fader: a black triangle handle (≈10×8) riding the left edge of the meter**; stereo meter of 2×6 px bars with a 1 px gap in a `#21242d` trough; dB scale 6, 0, 6, 12 … 60 to the right; pan knob (≈24) | stretches with the mixer height |
| Buttons | **track activator** = the track number in an amber 22×23 button (it mutes when off); **S** 22×13; **arm** 22×13 (circle icon, red when armed) | |
| Performance Impact (optional) | 6 small rectangles, a per-track CPU meter | |
| Colour stripe | the track colour, **6 px**, "to help with navigation and track identification" | 6 |

- The Returns ("A Reverb", "B Delay") and the **Main** strip sit on the right. Main has the
  Cue Out and Main Out choosers, a Solo/Cue toggle and a blue cue-volume knob.
- Strips are separated by 2 px lines (`#6a6e79`). The **selected strip is lighter**
  (`#babcc3` against `#a2a5ad`); it has no outline and no glow.

### 4.3 Width options

Strip width is **continuous**: drag the title-bar edge, or Alt/Opt-drag to change all
strips at once. The default is about 76 px in the Arrangement mixer and 95 px in the manual's
Session example. `mixer-tall-meters-wide-strips.png` shows the full range:

- **About 18 px**: only the activator number, S, arm and a meter. The title shows one
  letter ("B", "R") and the labels abbreviate ("M. Fr.", "Mon.").
- **About 35 px**: labels abbreviated, choosers clipped.
- **Normal**: everything, as above.
- **Wide** (about 250 px): choosers grow, and once the strip is also tall a **dB scale
  appears beside the meter**.

### 4.4 Height: why Live's mixer does not need vertical scrolling

The user says Cantus's mixer "can't be scrolled down easily". **Live's mixer is designed so
that it never has to scroll vertically.** Three mechanisms make that work:

1. **Sections are optional.** In/Out, Sends, Volume, Track Options (track delay), Crossfader,
   Performance Impact and Return Tracks are switched from the ▾ next to the mixer toggle
   (`mixer-view-control-options-menu.png`), from View › Mixer Controls, or with shortcuts
   (Ctrl/Cmd+Alt+I, S, R). The choice is stored *per view*, so Session and Arrangement can
   differ.
2. **The Volume section absorbs the height.** Dragging the mixer's top edge up lengthens the
   meters and adds tick marks, the numeric volume box and the resettable peak indicator.
   Widening the strips then adds a dB scale.
3. **It degrades gracefully when short** (12.x). "Reduced the minimum mixer height… the
   mixer pan controls will be hidden as the mixer's size is reduced; the track activator
   buttons will become compressed."

Horizontal scrolling uses the view's scrollbar, which only appears while scrolling in 12.x.
Returns and Main are laid out on the right-hand side of the mixer, after the tracks, as
their own column group. Zoom is global (Ctrl/Cmd +/−).

### 4.5 A channel's details

**Live has no channel inspector.** Selecting a track shows its **device chain in the Device
View**: its "inserts", laid out horizontally, full height and fully editable. Anything else
about the track, such as routing or delay, lives in the strip's optional sections. Double-
clicking a title bar opens the Device View. Track multi-selection makes one mixer control
move all selected tracks while keeping their offsets.

### 4.6 Grouping and colour

- **Group Tracks** are summing buses with a fold button. A folded group in the Arrangement
  shows an overview of its children's clips.
- A group's Solo button is **half-coloured** when a child is soloed.
- "Assign Track Color to Grouped Tracks and Clips" recolours a whole group.
- Colours can be auto-assigned. A "Reduced Automatic Colors" option uses fewer hues.

---

## 5. Device and instrument panels

### 5.1 The common frame, for every device

- **Title bar (17 px, `#bcbcbc`)**, left to right: an amber activator dot (about 12); an
  optional ▾ for the expanded view; the **name** in the normal 7 px-cap type; then
  device-specific toggles (scale awareness, "A|S" for Electric, Osc tabs for Wavetable,
  section headers for Drift). The **icons on the right** are hot-swap preset, save preset
  and a "…" options menu.
- **The body is tiled.** Sub-panels are separated by 2 px gaps of the title-bar colour,
  with dark display tiles where a picture helps. There are no bezels and no skeuomorphic
  hardware.
- **Presets are not in the device at all.** You hot-swap (Q, or the ↻ icon), which turns
  the *browser* into the preset list for that device. Save goes to the User Library.
  A single browser means a single place to look for presets.
- The body height is fixed and the width is free. The Device View hosts built-ins at their
  own sizes; third-party plug-ins open in floating windows or show a generic parameter
  panel.

### 5.2 EQ Eight (486 px; `device-eq-eight.png`, `device-eq-eight-expanded.png`)

- **Left column**: Freq / Gain / Q knob cells for the *selected* band. The knob's arc is
  cyan and the value is below it ("5.49 kHz", "3.10 dB", "0.42").
- **Centre**: a `#242424` display with the spectrum filled in `#565656`, grid lines
  `#343434`, grey axis labels ("12, 6, 0, −6, −12"; "100, 1k, 10k"), the **cyan curve**
  and **amber numbered band handles** (outlined circles, filled when selected). The display
  *is* the main control: you drag the handles.
- **Bottom band strip**: 8 cells, each with a filter-shape chooser (a pictogram and ▼), a
  cyan on/off square and the band number.
- **Right column**: an Audition headphone icon; a Spectrum toggle (amber); Mode "Stereo ▼";
  Edit "A"; "Adapt. Q" with an amber **On** button; "Scale 100 %" and "Gain 0.00 dB" as
  cyan-filled number sliders.
- **Expanded view**: the display grows to full width above the Device View, and the device
  shows all 8 bands as columns of Freq / Gain / Q, with Analyze, Block and Refresh settings
  on the left.

### 5.3 Compressor (375 px; `device-compressor.png`)

- **Left column**: Ratio ("inf : 1"), Attack and Release knob cells, and "Auto" (a
  toggle).
- **Centre**: a display with a **header strip** of readouts (Thresh value in cyan, GR and
  Output legend squares, Out value in cyan), the level history in grey, the gain-reduction
  trace in amber, a cyan threshold line and a cyan GR readout in the corner. Its **footer
  strip** holds view-mode icons (collapsed, transfer curve, activity) followed by
  Knee / Look. / Env. readouts. Knee and the others are drag values, and Look. is a
  chooser (▾).
- **Right column**: "Makeup"; a Peak / RMS / Expand radio stack (13 px buttons with 3 px
  gaps, amber when selected); and a Dry/Wet knob cell at the bottom.

### 5.4 Wavetable (982 px; `device-wavetable.png`)

- **Title row**: Osc 1 / Osc 2 tabs, each with an amber "on" square. On the right are the
  Mod Sources / Matrix / MIDI / MPE tabs, so the title bar doubles as a tab strip.
- **Three dark display tiles**:
  - the **wavetable** stack, with a cyan and amber position slider;
  - the **filter response**, with numbered amber handles 1 and 2, Serial routing, and
    Res / Frequency / Drive knobs;
  - the **envelope**, with tabs Amp / Env 2 / Env 3 / LFO 1 / LFO 2, a cyan curve with
    amber breakpoints, and A/D/S/R values in cyan along the bottom.
- **Light columns** on the left (Sub, Gain, Tone, Octave 0/−1/−2, Transpose) and on the
  right (Volume, Poly, Glide, Unison Noise, Voices, Amount).

### 5.5 Drift (818 px; `device-drift.png`)

- The title row carries the **section headers**: "Osc Mix", "Filter", "Envelopes", then
  LFO and Mod as tabs. Each header is centred over its column, in the same type as the rest.
- The **oscillators are a dark tile**. Osc 1's controls are **amber** (Oct, Shape, value
  readouts) and Osc 2's are **cyan**. In the mixer, the "1", "2" and "N" mix toggles use the
  same colours. Colour ties each source to its level control across the device.
- The Filter and Envelopes are light tiles with knob cells on a regular pitch. The two
  envelope rows (1 and 2) share one dark display showing both curves, with "1" and "2"
  selectors.
- The right-hand columns (Mode, Voices, Drift, Legato, Glide, Volume, Vel > Vol,
  Transpose, Note PB, PB Range) are number sliders and choosers of 13 px each.

### 5.6 Displays that are also controls

This is a consistent rule across devices: **anything drawn in a display can be dragged
there**. Examples are EQ band dots, filter handles, envelope breakpoints, the wavetable
position and the Compressor threshold line.

---

## 6. Navigation and speed

- **Everything is one click or one key away.** Views are shown and hidden, not navigated
  through. There is no page-switching.
- **Zooming is fast and in place.** Drag vertically on a ruler or the overview to zoom and
  horizontally to scroll. Double-click the beat ruler to zoom to the selection, or out to
  the whole song. Z and X zoom in and back; H and W optimise height and width.
- **Direct manipulation is everywhere.** Drag values, double-click (or Delete) to reset,
  hold Shift for fine adjustment, and type digits on a focused control.
- **Hover gives help, not noise.** There is no tooltip lag because there are no tooltips:
  the **Info View** and the **Status Bar** carry the explanation and the value, and the
  cursor changes shape to show resize zones.
- **Drag targets name themselves.** Drop zones say what they accept. Dragging a device into
  empty space creates a track of the right type. Hovering a dragged item over a view toggle
  opens that view.
- **One browser for everything**: sounds, devices, plug-ins, presets through hot-swap,
  samples, clips and projects. Search is Ctrl/Cmd+F, and there are history and
  similarity search.

---

## 7. What Cantus should take

Cantus today uses a 4-px grid with rows of 24/20 px. Its type scale is caption 10.5,
small 11, body 13, strong 13 SemiBold, heading 16 SemiBold, mono 12 / 10.5, readout
13 / 19 mono. It also has an inspector side panel beside the mixer desk. Several
recommendations below are about *removing* sizes rather than adding them.

### Ranked recommendations

1. **Reset the typography to one size, one weight and one family.**
   - Use **IBM Plex Sans Regular at 11 px** for *all* chrome text: labels, values, track
     names, menus and headers. It gives a 7.7 px cap height, slightly larger than Live's
     7.0, to allow for egui's unhinted rasteriser on 1× Windows screens. Plex at 10 px
     matches Live exactly and suits rulers, meter scales and hi-DPI.
   - Keep 13 px only for dialog body copy, and 19 px mono only for the transport readout.
   - **Drop monospace for values.** Plex Sans digits are already tabular (600 units each),
     so "−6.0 dB" in Sans does not jitter. Mixing mono and sans in one strip is a large
     part of the "loose words" look.
   - Use **SemiBold nowhere in the chrome.** Mark hierarchy with position and one secondary
     ink (Live uses `#4f4f4f` against `#121212`).
2. **Make every label part of a cell, and never let text overflow.** Adopt Live's five
   patterns:
   - knob cell (label, knob, value on one centre axis);
   - field cell (label above-left, field below, left edges aligned);
   - text-in-button radio groups;
   - **number sliders** (a value field filled in accent), which save a lot of space in
     strips and inspectors;
   - inline "label value" readouts in displays.

   Add one text helper that takes `[full, short, abbrev]` variants, picks the first that
   fits the cell width, and otherwise **clips with the painter's clip rect**. Never elide
   by overlapping. This directly fixes "letters run into each other".
3. **Build a surface ladder with no outlines on panels.** Use four luminance steps: desktop
   and gutters, view surface, raised header, field. Add one **dark display** token for
   every graph in the plug-ins and instruments. Draw 1 px outlines only around interactive
   controls, and give views a 3 px radius on a 6–8 px gutter of the desktop colour. Tile
   sub-panels with 2 px gaps of the header colour instead of `Elevation::Card` strokes.
   Keep text at about 7:1 and surfaces 1.2–1.6:1 apart. Cantus's existing theme legibility
   test can assert both.
4. **Adopt Live's control metrics** (on Cantus's grid, rounded to 12/16/20):

   | Element | Live | Cantus |
   |---|---|---|
   | Control height | 13 | 12–16 |
   | Header / title bar | 17 | 16–20 |
   | List row | 19 | 20 |
   | Knob | 24 dia | 24 dia, open arc with a needle, no body |

   `ROW = 24` is too tall for strips and inspectors, and it is one reason the mixer
   overflows vertically.
5. **Keep a fixed window skeleton, resizable by edges, with show/hide toggles at the
   corners and a shortcut for each.**
   - In egui this is `TopBottomPanel` (control bar), `SidePanel::left().resizable(true)`
     (browser), a bottom resizable panel (detail area), and the main view with the mixer as
     an inner bottom panel whose top edge is draggable.
   - Persist the sizes and use `show_animated` for the toggles.
   - **Put the mixer toggle and its ▾ section menu at the bottom-right, and the browser
     toggle at the top-left.**
   - Add Alt+0…9 focus shortcuts and a visible focus frame around the focused *view*.
     `paint_focus_ring` already exists for widgets.
6. **Replace the inspector side panel with a bottom Detail area that can stack.**
   - A selected mixer channel shows its **effects as a horizontal chain of equal-height
     device panels**. Each gets a 16–20 px title bar with an activator dot, the name,
     preset/save/menu icons and a fold that collapses it to a 32 px strip with a rotated
     name. Meters go between devices, and a named drop zone goes at the end.
   - The per-strip insert *slots* can stay as the FL-style list in the strip; the Detail
     area is where the chain is *edited*.
   - The Clip / piano-roll editor and the device chain should be able to **stack**, which
     answers "resize or move the interface's elements" without free docking.
7. **Rebuild the mixer's vertical behaviour the way Live does.**
   - Sections (I/O, Inserts, Sends, Volume, CPU) become toggles in a ▾ menu, remembered per
     view.
   - The **Volume/meter section takes the remaining height**, with pan hiding and buttons
     compressing when short.
   - Strip width becomes continuous (drag an edge, Alt-drag for all), with abbreviated
     labels at narrow widths and a dB scale added at wide widths.
   - Returns and Master are **pinned** on the right, outside the horizontal scroll. Add a
     6 px colour stripe under each strip and make the selected strip lighter.
   - Only the *insert list* inside a strip should ever scroll vertically. The desk itself
     should not need to.
8. **Colour semantics as tokens.**
   - One "on" colour: Cantus's gold can play Live's amber. One "value" colour (cyan-like).
     One record red, one scale colour, and track colours for identity only.
   - Accent text only on dark displays; on light surfaces accents are fills.
   - Follow Live's theme structure: **Appearance (Light/Dark/System) × Tone
     (Neutral/Cool/Warm) × High contrast**, generated from one token set. The brightness
     and hue sliders should move surfaces only and never the chromatic colours (a 12.x
     rule).
   - Live's theme files have about 200 semantic keys; for example `SurfaceBackground`,
     `SurfaceArea`, `DetailViewBackground`, `DisplayBackground`, `ControlForeground`,
     `ControlBackground`, `ChosenDefault`, `SelectionFrame`, `Poti`, `PotiNeedle`,
     `MainViewFocusIndicator`, meter keys `BelowZeroDecibel1/2`, `ZeroDecibel`,
     `AboveZeroDecibel`, and `Clip1…16`.
9. **Replace tooltips with an Info line.** A docked help strip or Status Bar line shows the
   hovered control's name, what it does and its shortcut, plus the live value while
   dragging. It avoids tooltip clutter over dense strips, and the i18n layer can serve it.
10. **Number formatting rules.** About 3 significant digits, auto-ranging units
    (ms→s, Hz→kHz), a space before every unit including %, "−∞" for silence, and a
    segmented bars.beats.ticks field that can be dragged per segment.

### What not to copy

- **Live's text size at 1× without zoom.** A 7 px cap height is small on a 1080p Windows
  panel. Live relies on the 50–200 % zoom. Cantus should default to 11 px and offer UI
  scaling.
- **The plain grey default on its own.** Live looks beautiful because clip colours and
  amber/cyan sit on the grey. If Cantus has fewer coloured clips on screen, a pure grey
  chrome will look "super plain", which is exactly the user's complaint. Keep Cantus's own
  identity accent (gold) and let the track colours reach the strips, headers and meters.
- **The fixed layout's limits.** Live cannot swap regions or save layouts, and the user
  explicitly wants to move things. Take Live's skeleton and toggles, then add **region swap**
  (browser left or right, detail area bottom or right) and **saved layouts**. A full
  `egui_dock` free-for-all would cost the clarity.
- **No inserts in the strip.** Live puts inserts only in the Device View. Cantus's
  FL-parity direction needs per-strip slots; combine both.
- **Features hidden in context menus** (split-stereo pan, enabling return sends, crossfader
  curves). Cantus should expose them in the ▾ section menus.
- **Cyan text on a light surface** (1.7:1). Live mostly avoids it; Cantus must not use
  accent-coloured text outside dark displays.
- **The Session/Arrangement duality.** It is Live's concept, not a layout lesson.

### What "better than Live" would mean, measurably

- The same density (13–16 px controls, 19–20 px rows), with an 11 px default that reads
  cleanly at 1×.
- A test that fails if any text at the default size ever overlaps or overflows its cell.
- Resizable regions **plus** swappable positions and saved layouts, which Live lacks.
- A mixer that fits any window height without a vertical scrollbar, while still offering
  FL-style insert slots.
- Every surface, text and accent pair checked by the theme tests: text ≥ 7:1 on its
  surface, accents ≥ 4.5:1 wherever they are used as text.
- Keyboard focus visible at both the view level and the widget level, with a
  "same control on the next channel" key (Live's Ctrl/Opt+Tab).

---

## 8. Screenshots (all in `research/ui/screenshots/ableton-live/`)

Manual image base URL: `https://ableton-production.imgix.net/live-manual/12/<file>`. The
chapter pages are under `https://www.ableton.com/en/live-manual/12/`.

| File | Shows | Source file · chapter |
|---|---|---|
| `control-bar.png` | full window, Light Neutral (Control Bar highlighted) | `ControlBarL12.png` · live-concepts |
| `clip-and-device-view-stacked.png` | full window, Session + mixer + stacked Clip/Device | `StackedClipDeviceViewL12.png` · live-concepts |
| `session-view-scene.png` | Session grid, scene row, drop zone | `SessionViewSceneL12.png` · live-concepts |
| `mixer-in-arrangement.png` | Arrangement with the mixer open | `MixerInArrangementL12.png` · live-concepts |
| `clip-view-midi.png` | Clip View: panels + note editor | `ClipViewL12.png` · live-concepts |
| `device-view-chain.png` | Device View chain | `DeviceViewL12.png` · live-concepts |
| `arrangement-view-full-demo-set.png` | Arrangement, 15 tracks, track controls | `DemoArrangementL12.png` · arrangement-view |
| `arrangement-top-overview-ruler-scrub.png` | overview, beat ruler, scrub, locators | `ArrangementViewTop.png` · arrangement-view |
| `arrangement-track-controls-mixer-zoom.png` | track controls, expanded track, mixer, H/W | `ArrangementViewLayout.png` · arrangement-view |
| `arrangement-mixer-track-controls.png` | tall tracks revealing more controls + mixer | `ArrangementMixerTrackControls.png` · arrangement-view |
| `view-menu-mixer-controls-submenu.png` | View › Mixer Controls submenu | `MixerControlsSubMenu.png` · arrangement-view |
| `arrangement-view-mixer-strips.png` | Arrangement mixer | `ArrangementViewMixer.png` · mixing |
| `session-view-mixer.png` | Session mixer (Light Cool), measured strip anatomy | `SessionViewMixer.png` · mixing |
| `mixer-controls-annotated.png` | the mixer controls | `SessionAndArrangerMixerWithBreakouts.png` · mixing |
| `mixer-tall-meters-wide-strips.png` | strip widths 18 → 250 px, tall meters, dB scale | `ProSessionMixerSizes.png` · mixing |
| `mixer-view-control-options-menu.png` | mixer ▾ section menu | `MixerViewControlOptionsIO.png` · mixing |
| `track-title-bars.png` | track title bars | `TrackHeadersCircled.png` · mixing |
| `browser-layout-annotated.png` | browser anatomy | `BrowserLayoutL12.png` · working-with-the-browser |
| `clip-view-panels-vertical.png` | clip panels stacked vertically | `ClipViewVerticalL12.png` · clip-view |
| `device-view-devices-annotated.png` | Velocity, Drift, Chorus-Ensemble chain | `DeviceViewDevicesL12.png` · working-with-instruments-and-effects |
| `devices-folded.png` | folded devices with rotated names | `FoldedDevicesL12.png` · working-with-instruments-and-effects |
| `meters-between-devices.png` | meters between devices | `MetersBetweenDevicesL12.png` · working-with-instruments-and-effects |
| `device-roar-expanded.png` | Roar expanded view, rotated side labels | `RoarExpandedViewL12.png` · working-with-instruments-and-effects |
| `view-selectors-bottom-right.png` | Clip/Device selectors, mixer toggle | `ClipDeviceViewSelectorsL12.png` · working-with-instruments-and-effects |
| `device-eq-eight.png` | EQ Eight | `EQEightL12.png` · live-audio-effect-reference |
| `device-eq-eight-expanded.png` | EQ Eight expanded | `EQEightExpandedViewL12.png` · live-audio-effect-reference |
| `device-compressor.png` | Compressor | `CompressorL12.png` · live-audio-effect-reference |
| `device-wavetable.png` | Wavetable | `WavetableInstrumentL12.png` · live-instrument-reference |
| `device-drift.png` | Drift (Light Cool) | `DriftL12.png` · live-instrument-reference |
| `info-view.png` | Info View | `InfoViewL12.png` · first-steps |
| `product-mixer-improvements.png` | Live 12 Dark Neutral: Arrangement + mixer | `https://beta-ableton.imgix.net/media/gn4nwrrd/web-mixer-improvements.png` · ableton.com/en/live/ |
| `product-stacked-detail-views.png` | Live 12 Dark Cool: Session + Clip + Meld | `https://beta-ableton.imgix.net/media/l1fn0apk/web-stacked-views.png` · ableton.com/en/live/ |
| `product-browser-lavender-light-theme.png` | Live 12 light theme, browser + arrangement + devices | `https://beta-ableton.imgix.net/media/q4lp4ni5/web-browser-lavender-light.png` · ableton.com/en/live/ |
| `product-glue-compressor-2x.png` | Glue Compressor | `https://beta-ableton.imgix.net/media/n05lflms/glue-compressor-2x.png` · ableton.com/en/live/what-is-live/ |
| `product-session-view.png` | *Live 11* Session + mixer (for comparison) | `https://beta-ableton.imgix.net/media/b2qnxryw/screenshot-session-view.png` · ableton.com/en/live/what-is-live/ |
| `product-arrangement-view.png` | *Live 11* Arrangement with comping (for comparison) | `https://beta-ableton.imgix.net/media/tgll5joj/screenshot-arrangement-view.png` · ableton.com/en/live/what-is-live/ |

## 9. Text sources

- Live 12 manual chapters, at `https://www.ableton.com/en/live-manual/12/<slug>/`:
  first-steps (Settings, Theme & Colors, Display & Input, Info View), live-concepts,
  arrangement-view, mixing, working-with-the-browser, clip-view,
  working-with-instruments-and-effects, live-audio-effect-reference,
  live-instrument-reference, live-keyboard-shortcuts, accessibility-and-keyboard-navigation.
- Live 12 release notes: https://www.ableton.com/en/release-notes/live-12/. These cover
  "UI View Styling", "Mixer Improvements", "Keyboard Navigation", "Keyboard Workflow",
  "Mixer in Arrangement", the 12.x minimum-mixer-height change, and the brightness change
  that affects surfaces only.
- Product pages: https://www.ableton.com/en/live/ and
  https://www.ableton.com/en/live/all-new-features/ ("New themes… cool, neutral or warm
  tones and even a high contrast mode").
- Ableton Sans: https://lettersfromsweden.se/ableton/. Ableton forum, "12 font is different
  from 11!": https://forum.ableton.com/viewtopic.php?t=250660.
- Theme structure (Appearance / Tone / Contrast): https://forum.ableton.com/viewtopic.php?t=250625.
- Theme key names were read from a third-party Live 12 `.ask` theme file. The keys are
  Live's schema; the values are not Ableton's.
  https://github.com/M0RDI0/mordio-ableton-theme
- Zoom range 50–200 %: Live 11 manual, First Steps
  (https://www.ableton.com/en/live-manual/11/first-steps/).
