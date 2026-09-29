# FL Studio: interface research for Cantus (September 2026)

The current version is **FL Studio 2026** (v26.1.x, released 7 July 2026; 26.1.6 on 7 September 2026).
The interface is essentially FL Studio 2025's, plus a rebuilt FLEX and a wider Browser. The
default theme has not changed in its fundamentals since FL 20/21 (FL 21 added the Theme
editor). So the online manual's screenshots, most of which were taken in FL 20 to 21, still show
the current look. I checked them against the FL 2025 and 2026 product-page captures (files 02–04, 08, 35).

**What the user asked for.** FL Studio is the *primary* visual reference for Cantus's redesign.
This report aims to be precise enough to build from. Section 12 ("FL's style, translated to Cantus")
is the implementable spec. Sections 1–11 are the evidence behind it.

**How I measured.** Every hex value below comes from a pixel in an official screenshot. I read them
with a small PIL script that prints colour runs along rows and columns, and ink bounding boxes for text.
When a value comes from a palette-quantised PNG (a "P" image) I say so: flat fills there are exact,
but gradients are approximate. **Scale:** the manual's close-ups are taken at about **150% UI scale**.
For example, the channel-rack row pitch is 45 px in `12-channel-rack-annotated.png` and 30 px in the
100% main-window capture `01-main-window-annotated-panels.png`, and the step pitch is 24 against 16.
Sizes are marked **@150** (as measured in a close-up) or **@100** (measured at 100%, or derived by
dividing by 1.5). Cantus should use the @100 values, in egui points.

---

## 0. Why FL looks beautiful: the rules, measured

These are the rules another engineer can follow. Each one is backed by measurements later in the report.

1. **One hue for the whole machine.** Every neutral surface in the default theme sits at **hue
   199–207°** (a cool slate), with saturation **6–10%** on chrome and **15–22%** on the dark editor
   grounds. Only the lightness changes. The steps are 15% (browser) → 23–30% (editor grids) → 30%
   (mixer strips) → 34% (window chrome) → 38% (toolbar) → 41–48% (panels and tiles) → 75% (menus).
   Nothing is neutral grey and nothing is black, so the whole window reads as a single material.
2. **Surfaces are mid-dark, not dark.** The main surfaces sit at L 30–48%, well above the L 5–15%
   typical of "dark mode". That leaves room for *both* darker wells and lighter raised caps, which
   is why FL can have visible depth everywhere.
3. **Accents are few, fully saturated, light, and small.** Selection is lime `#c3ff5a`, highlight
   is orange `#ffaa54`, the mute LED is `#8def11`, option/record is `#ff6135`, the FX-on bulb is
   `#ffe34f`, and the active tab text is `#3fc3ff`. All have **S 87–100%, L 50–68%**. They appear
   only on LEDs, meters, active toggles, the selected fader and small icons, never on large areas.
4. **User colours are pastel and equal in lightness.** Mixer header colours measure **S 24–39%,
   L 58–64%** for every hue, so no track shouts. A picker option, "Lock to safe colours" (the
   manual's "aesthetics committee"), clamps what the user picks and shows the chosen colour
   next to its "safe" version. Large fills such as playlist headers and clips use darker versions (L 36–46%).
5. **Text on a coloured item is a tint of that colour, not white.** Examples: a track header
   `#407840` carries its name in `#cbe9d5`, a picker clip `#8a8550` carries `#f5efa7`, and a mint
   note `#bcf5c9` carries its label in dark green `#4f8858`. This one rule is a large part of why
   FL looks harmonious rather than stuck-on.
6. **Everything is lit from above, the same way.** Raised things (step keys, fader caps, channel
   buttons, tiles, knobs) have a 1-px lighter top edge, a vertical gradient from lighter to darker,
   a 1-px dark outline and a 1–3 px soft shadow below. Sunken things (dropdowns, number boxes, meter
   wells, LCDs) are darker than their surround, with a darker top edge. It is the same recipe on
   every control, only scaled.
7. **Physical metaphors, drawn small and crisp.** A fader cap has a groove line. A step button is a
   keycap with a slit 7 px from its top. An LED is a lit core inside a dark ring. These are vectors,
   not bitmaps, and every one fits a 16–30 px cell @100.
8. **Condensed type, set small, on a strict rhythm.** The UI face is **PT Sans Narrow**. Body text
   is about 12–13 px @100 and labels are 11 px. It is condensed, so long names fit in 20–44 px
   strips. Mixer names run *vertically*, and overlong names are elided in the middle ("GHO..ICK",
   "KIC..RUM"). Text never collides because every label owns a fixed box.
9. **Labels belong to controls.** Plug-in labels are UPPERCASE, centred 6–8 px under their knob.
   Sections carry full-width header bands. Some plug-ins (LuxeVerb) wrap each control in a *cell*:
   the value in a pill on top, the knob, then the label. Values are not printed next to every control.
   They appear in the **hint bar** on hover and while dragging, which keeps the surface quiet.
10. **A strict row rhythm.** Mixer slot tiles are 18 + 1 px @100 (26 + 1 @150). Channel rows are
    30 px @100. Steps are 12 px wide on a 16 px pitch @100. Compact strips are about 20 px @100 and
    Wide strips 44 px. Repeated items are identical and sit on the same pitch.
11. **Displays are controls.** You drag EQ band tokens and Maximus curve points directly. The
    playlist scrollbar is also the zoom (drag its ends). You drag LCD numbers, or click their top or
    bottom edge to step them.

---

## 1. Visual language

### 1.1 The neutral ladder of the default theme (measured)

| Role | Hex | H / S / L | Where measured |
|---|---|---|---|
| Deepest ground: Browser and lists | `#1f282d` | 201° / 18% / 15% | 01 (100%), 03 (FL 2026) |
| Browser selected folder band | `#273035` | 201° / 15% / 18% | 03 |
| Playlist grid ground (FL 2026) | `#2e3e48` | 203° / 22% / 23% | 03 |
| Timeline strip above the grid | `#2d3a42` | — | 15 |
| Piano-roll black-key row | `#374954` | 203° / 21% / 27% | 15 (RGB) |
| Piano-roll white-key row | `#3e4f5a` | 204° / 18% / 30% | 15 |
| Piano-roll downbeat shading (first beat of each bar) | `#42545f` | — | 15 |
| Sunken field (dropdown), gradient | `#42484c` → `#3b4145` → `#353a3f` | 204° / 8% / 25% | 06 (RGB) |
| Meter well | `#3f494d` | — | 06 |
| Mixer strip body | `#454f55` | 202° / 10% / 30% | 06 |
| Mixer strip body, **selected** | `#4c575d` | 201° / 10% / 33% | 06 |
| Window chrome, title and tool rows | `#4e585e` | 202° / 9% / 34% | 06, 15 |
| Window title bar (100% capture) | `#535d62` | 200° / 8% / 35% | 01 |
| Main toolbar background | `#596267` | 201° / 7% / 38% | 03 |
| Dialog page | `#5e686e` | 202° / 8% / 40% | 19 |
| FX slot tile (loaded) | `#606b71` | 201° / 8% / 41% | 06 |
| FX slot tile (empty) | `#687378` | 199° / 7% / 44% | 06 |
| Panel behind tiles: inspector, channel-rack body | `#727c81` | 200° / 6% / 48% | 06, 12 |
| Neutral mixer header cell | `#768086` | 202° / 6% / 49% | 06 |
| Menu (FL 20/21) | `#bdc2c6` | 207° / 7% / 76% | 42 |
| Menu (FL 2026) | `#b8c3c9` | 201° / 14% / 75% | 36 |

Lines and edges:

- **Piano-roll grid.** Step lines `#394b56`. Beat lines `#2e404b`. **Bar lines are three pixels:**
  `#354652 | #1a2c37 | #354652`, a dark core with softened sides. The octave row line is `#32444f`.
- **Mixer dock divider.** It is 4 px: `#384248` ×2, `#293238`, `#364045`.
- **Window frame.** A 1-px outer line `#374146` (or `#2a3338`), then a 1-px inner highlight `#566065`.
- **Slot tiles.** A 1-px border `#4e595f`, with a 1-px gap of the panel colour `#727c81` between tiles.
- **Menu separators** `#949ca2`. **FL 2026 menu section bands** `#8f9da5`, hover row `#5a6c76`.

### 1.2 Accents: the theme's role colours

These are the defaults, read from the Theme settings swatches (19) and from the UI.

| Role (Theme setting) | Hex | Used on |
|---|---|---|
| Selection | `#c3ff5a` (LED core `#c3ff65`, rim `#cfff79`) | selected tracks and faders, the selected channel, LEDs |
| Highlight | `#ffaa54` | active toolbar toggles (gradient `#ffa558` → `#ffeb9d`, glyph `#b13f00`) |
| Mute (LED) | `#8def11` | mute/enable LEDs |
| Option | `#ff6135` | option LEDs; record is a similar red |
| Steps | odd `#aabcc7`, even `#d8a6a8` | step groups of 4 (lit: `#d1e3ee` and `#fdccce`) |
| Meters | lime `#c4fc55`, yellow `#ffdf5a` (≥ −6 dB), orange `#ff8d3e` (≥ 0 dB) | all peak meters |
| Waves | `#daf5d2` … `#f69792` | frequency-coloured waveforms (optional) |
| Text | `#e7eef2` | global text colour, trimmed by the Text knob |
| FX on (mixer bulb) | `#ffe34f` | per-track FX switch |
| Active tab text | `#3fc3ff` | dialog tabs |
| PDC state | grey, orange (auto), blue (manual) | the track-delay clock icon |

Contrast check (WCAG): lime on the strip is 7.1:1, and orange on the chrome is 3.9:1. That is fine for glyphs and LEDs, which are not text.

### 1.3 User colours, and the "safe colours" rule

- Mixer header cells (06, 01) measure `#bb7fc4 #96b176 #7b9cba #75b188 #b97ec0 #b58b77 #827fc4 #a5c47f
  #83afbb #8da5ab #bc9985 #9a8cb8 #80c7bd #c580a6 #7e92c3`. Across all hues that is **S 15–39%,
  L 58–64%**. That is the whole trick: every hue at the same lightness, all muted.
- In FL 2026, playlist track headers and clips (03) are darker and slightly richer: `#407840` (green),
  `#8a8550` (olive), `#7d624f` (brown), `#a44a48` (red), all **S 23–39%, L 36–46%**. Large fills are
  darker than small ones.
- The colour picker (20) has three modes: Circle, Box and Preset. It offers **Quantize** (snaps
  hue and saturation) and **Lock to safe colours** (narrows the saturation and luminance range). It
  shows the chosen colour and its safe version side by side, and has history, random-safe (dice),
  `#` for HTML entry, **D** for default, and editable HSL/RGB fields. "Random colour" and F2
  (press again to re-roll) give quick recolouring.
- In the mixer menu, "Colorful mixer" has four settings: Off, Low, Medium and High. At Off only the
  header is coloured. The others tint the whole strip height, as in 08, where a green group runs
  full height at roughly 30% strength. "Auto color group" spreads one colour across a group.

### 1.4 Depth: one lighting recipe, applied everywhere (measured profiles, @150)

- **Step button (keycap)**, from 12, top to bottom. The cap is 18 px wide and 30 px tall, on a
  24 px pitch with 6 px gaps.
  - Off state: top band `#81888c`–`#8c9397` (5 px), then a **1-px slit `#2a3035`**, then the body
    `#787f83` → `#71777c` → `#696f74` → `#61686c`. It ends with 2 px of bottom edge `#3f454a`/`#323a3f`
    and a fading shadow `#4d575d`, `#626c72`.
  - Lit state: the same structure, with `#f9ffff` on top, the slit `#485964`, and a body going from
    `#e6f8ff` to `#acbec9`.
  - Horizontally the corners are rounded about 2 px. The outer columns are `#656f75`/`#77838a`, then
    `#abbbc5`/`#becfd9`, then the face.
- **Mixer fader cap**, from 06. It is 24 × 37 px @150, about 16 × 25 @100. The upper half ramps
  `#a7aab4` → `#f6f9ff` (brightest just above the middle). Then comes a **1-px groove `#3c3e40`**,
  and the lower half ramps `#41444e` → `#aeb1bb`. The sides are shaded `#3c454a` / `#596065` /
  `#9da0aa` / `#e6e9f3` into the face `#d9dce6`. It is the lightest object on the strip: L 85%
  against L 30%.
  - Selected: the cap turns lime and the fader groove below the cap fills with `#a5d95a`.
  - Armed for recording: the cap turns orange.
- **Channel button**, from 12. It is 124 × 38 px @150, about 83 × 25 @100: a 1-px outline
  `#424c52`, a 1-px top highlight `#808a90`, a body `#6f797f` → `#697378` → `#626c72`, and 2 px of
  shadow. When selected, it takes the channel colour (for example mauve `#614f59` → `#5a4953`).
  When its plug-in is missing, it turns red.
- **Sunken dropdown**, from 06. It is 34 px tall @150, with a 1-px border `#3b454b`, a 1-px top
  inner line `#60656a` and a fill of `#42484c` → `#3b4145` → `#353a3f`. The text is `#d8dcdf`, with a
  small ▸ at the right end.
- **Mixer routing number box** (channel rack). It is sunken: a 1-px border `#414b50` and a fill of
  `#636d73` → `#6a7479`, with the number right-aligned and a small ‹ › spinner.
- **LED**, from 12. It is 16 px @150 (about 11 @100), built from the outside in: an outer ring
  `#a2a9ae`, a halo `#4d6552`, a dark ring `#569912` (the accent × 0.55), a bright rim `#cfff79`,
  and the core `#c3ff67`. A "lit" LED is really a lit lens.
- **Knob in the mixer and channel rack.** It is small (28 px mixer pan @150, 20 px rack @150). It has
  a dark flat face `#4c545a`/`#505960`, a 1-px darker outline `#2d373c` and a light pointer line.
  There is no value arc on the mixer knob; the arc appears on plug-in knobs (section 7).
- **Plug-in knob** (Fruity Limiter, 24; Delay 3, 30). A dark face with a two-tone diagonal
  (lit top-left) sits inside a **recessed ring track 4–6 px wide, `#2a3338`**. The **value arc** is
  painted in that ring in the section's colour (cyan `#38a9bf`, pale `#b8c1c9`, lime in Delay 3). A
  short pointer runs from about 40% to 85% of the radius in the same colour.
- **Floating windows.** They have a soft drop shadow (visible in 29 and in the FL 2026 hero image
  `fls-26`). With "Transparent windows" enabled (General settings), inactive windows become translucent.

### 1.5 Density and space

- Mixer @100: a Compact strip is about 20 px wide and a Wide strip 44 px. A 1670 px window at 100% (01)
  shows about 11 Wide strips plus the inspector; 06 shows 35+ Compact strips.
- There is little empty space inside controls and generous regularity between them. The "air" comes
  from the **gaps being identical** (1-px tile gaps, 6-px step gaps, 7-px channel-row gaps) rather
  than from large margins.
- Empty things are drawn, dimmed: "Slot 5" in dim *italic*, "Track 29" in dim text, empty mixer
  inserts named "Insert 126". The geometry never jumps when content changes.

### 1.6 Iconography

The icons are small (14–18 px @100), vector, in mono light (`#e1e7ec`–`#f2f8fc`) on the chrome.
Only a few carry colour: the magnet (snap), the pencil and paint tools, the cyan mixer-docks icon,
the yellow FX bulb, and the orange PDC clock. Every window's top-left has a **▸ menu arrow**, a
consistent place to find that window's menu. The icons are playful but consistent in stroke. The
manual admits that some are "tiny, so keep your eye out for them", which is FL's weak spot
(section 11, "not to copy").

### 1.7 Themes (Options › Theme)

- **Knobs:** Hue, Saturation, Brightness, Contrast, and Text (text brightness). The manual warns that
  thousands of elements update, so the knobs should be moved slowly.
- **Role colour pickers:** Selection, Highlight, Mute, Option, Steps (odd/even), Meters (a gradient
  editor with dB stops, 21), Waves (a frequency-to-colour map) and Text.
- **Options:** "Light mode", which keeps elements visible at extreme settings, and "Audio and
  automation clips use note colors". There are A/B slots, Reset, and Save preset.
- **Factory presets:** (Default), Dark orange, Dark, Light cherry, Light fruit, Light pear and more.
  From the thumbnails in 19, the light themes use a white ground, light-grey controls and **one**
  strong accent (pink, orange/lime or green) on the selected and active items and the channel buttons.
- Note colours, the grid colour and the background image are saved with the theme.
- Other look settings (General settings): Animations ("Don't distract me" … "Entertain me"),
  Smoothing, the refresh rate (¼, ½ or full monitor rate), "Thick lines" (≥ 200%), "High visibility"
  (hover rectangles around switches, white scrollbar hover, menu captions copied to hints),
  "Colorful waveforms", and "Small scrollbars in editors" (50% width).

---

## 2. Typography

### 2.1 The faces

- **PT Sans Narrow** (ParaType, SIL OFL 1.1) is FL's UI face. It has shipped as
  `Artwork\Fonts\PT_Sans-Narrow-Web-Regular.ttf` since FL 11, according to Image-Line forum users
  (see Sources). The mixer, channel rack, browser, playlist, menu bar and plug-in labels are all
  clearly this condensed humanist sans, and the measured ratios are consistent with it (x-height about
  0.75 of cap height).
- Window titles in the FL 2025/2026 captures ("Channel rack" in the `fls-26` hero image, the "Update"
  dialog) and the menus of 36 appear in a *regular-width* humanist sans, similar to Open Sans or Noto
  Sans. That identification is visual and **unconfirmed**.
- Plug-ins bring their own display faces: the FLEX headings are a geometric condensed caps, and
  Emphasis uses a letter-spaced wordmark. The *labels*, though, stay in the house face.
- **Open equivalents for Cantus.** The best is **IBM Plex Sans Condensed** (OFL, the same family as
  Cantus's bundled Plex Sans). PT Sans Narrow itself is OFL and can be bundled. Other options are
  Roboto Condensed and Fira Sans Condensed.

### 2.2 Sizes by role (measured ink heights)

| Role | Measured | ≈ font size @100 | Case / weight / colour |
|---|---|---|---|
| Main menu bar "FILE EDIT ADD…" | caps 12 px @100 (01) | ~17 px | UPPERCASE, regular, `#fdffff` on an LCD-like panel `#5f7374` (FL 2026) |
| Hint line (toolbar hint panel) | cap 8 px @100 | ~12 px | sentence case, `#eafefc` / FL 2026 `#bad7da` |
| Project title line (hint panel) | similar | ~12 px | dim `#cdd3d9` |
| Window title ("Piano roll - FLEX", "Mixer - Synths Out") | cap about 11 px @150 | ~13 px | Title case, `#f2f8f9` |
| Mixer insert slot name ("Fruity Limiter") | cap 10 px @100 / 12 @150 | ~13–14 px | as named; empty slots *italic* in dim `#9ca2a8` |
| Mixer track number (header cell) | digits 15 px @150 | ~14 px | dark on a pastel cell, or light when selected |
| Mixer track name (vertical) | cap 12 px @150 (width of rotated text) | ~12 px | as typed, often UPPERCASE; `#c4cbd2`; middle-elided |
| Channel button name | about 9 px @100 | ~12 px | centred, `#fcfee7` |
| Playlist track name | 10 px @100 | ~12–13 px | top-left of the header, text tinted with the track colour |
| Browser item | cap about 11 px @100 | ~14–15 px | special folders tinted (`#cf9e92`, `#bf9477`), items `#a9c6d1` |
| Piano-roll note label ("B5") | 9–11 px @150 | ~9–10 px | a **darkened tint** of the note colour |
| Timeline bar numbers | 9 px @150 | ~9 px | muted `#a8b9c0` |
| Micro labels ("NOTE CHAN PAT", "ON PAN VOL PITCH RANGE TRACK") | caps 5–7 px | ~7–8 px | **bold UPPERCASE**, dim or tinted |
| Plug-in control label ("THRES", "HIGH CUT") | caps 11–12 px @150 | ~11 px | UPPERCASE, centred under the control, `#bfd4bf` / `#e6f3f6` |
| Plug-in section header ("LOUDNESS", "REVERB") | caps 17–18 px @150 | ~15 px | UPPERCASE; Limiter bold `#aabcc5`, LuxeVerb regular white |
| LCD numbers: tempo `125.000`, time `1:01:00` | tempo digits 19 px, clock 36 px (FL 2026 crop) | 16 / 28 px | a light condensed face; decimals **smaller** than integers ("106.**512**") |

### 2.3 Case, spacing, colour

- **UPPERCASE** is used for the main menu, plug-in parameter labels, section headers and micro labels.
  **Title case** is used for window titles and FLEX-style labels ("Cutoff", "Feedback"). Sentence
  case is used for hints and menus.
- The labels are not letter-spaced (the condensed face needs no tracking). The exception is wordmarks,
  such as "E M P H A S I S" and "LuxeVERB", which are widely spaced as *logos*.
- Colour does the hierarchy work, not weight. Three levels are typical: near-white (L 93–94%) for
  primary text, about L 70–80% for labels, and L 58–64% for placeholders and disabled text.
- **Placeholders are deliberately low-contrast** (2.0–2.1:1: "Slot 5" on its tile, "Track 1" on its
  header). They read as "empty", not as text to be read.

### 2.4 How labels attach to controls

- **Mixer.** It has no text labels at all except the track number and name. Every control is an icon
  or a shape (LED, knob, fader, bulb, dot, arrow) whose meaning comes from its row position, and the
  hint bar names it on hover.
- **Channel rack.** The row reads left to right: LED, pan knob, volume knob, routing number, then the
  name *inside* the button, then the selector bar and the steps. The only text is the name and the
  number.
- **Plug-ins, four patterns:**
  - Knob with an UPPERCASE label centred **below**, 6–8 px gap (Limiter, Delay 3, Reeverb 2).
  - A section **header band** spanning the section above a group of knobs (Limiter, Delay 3, Vintage
    Chorus and Kepler use coloured bars).
  - A **control cell** (LuxeVerb): a rounded cell holds a pale **value pill** on top, the knob in the
    middle and the label at the bottom.
  - A **label that is also a button**: a bordered "CUTOFF" in Delay 3 marks a modulation target.
- **Brackets** join related controls. In the Limiter, a ⊢⊣ line connects ATT and REL above the
  CURVE/SIDECHAIN spinners, so the relationship is drawn rather than written.

### 2.5 Numbers

- **LCD displays** (tempo, pattern number, routing number) sit in sunken boxes. The integer part is
  large and the fraction small ("125.000", "106.512"). They have ▴▾ micro spinners at the right edge.
  You drag vertically to change them, click the top or bottom *edge* to step ±1 (the cursor changes),
  or click-hold and type.
- **The transport clock** is the largest text in the app (about 28 px @100) in a thin light face,
  with a tiny caption "B:S:T" (bars:steps:ticks) above the last digits.
- **Knob values are not printed** on the surface in native FL controls. They go to the hint bar.
  Newer plug-ins (LuxeVerb, Emphasis) show values in pills or inside the knob.
- **Units are context-aware** ("dB when adjusting volume"). Typed entry offers context units or
  normalised units (0–1 or %).

### 2.6 How FL avoids "loose words" and collisions

1. Every text has a **box it owns**: a slot tile, a header cell, a button, a pill or a band. Text
   never floats on a bare panel.
2. Overlong names are **middle-elided** ("SNAR..AP", "Wave..aper", "Harm..Pan"), so both the
   beginning and the ending stay recognisable.
3. Narrow columns get **vertical text** instead of squeezed horizontal text.
4. Values move to one place, the hint bar, instead of cluttering every control.
5. Groups get **one** header (a band or tab) rather than repeating a word on every control.
6. Labels align on a shared baseline per row. In the Limiter every label in a row shares y
   354–365 @150.

---

## 3. Layout and panels

### 3.1 The window

- **Toolbar** (top, two rows, about 38 px each @100). From left to right:
  - the menu bar (in an inset LCD-like panel), and the title and hint panel under it;
  - master volume and master pitch;
  - the transport (PAT/SONG, play, stop, record, tempo LCD);
  - recording toggles, the song-position slider and the big time LCD;
  - the global snap, pattern selector, window buttons and CPU/memory panel;
  - online news.

  The toolbar is fully user-editable: rearrange and hide panels, presets, "Separators", and
  "Flat buttons" (removes shading). Ctrl+F11 hides it.
- **Browser** (left, docked). It auto-hides optionally, can be dragged to the right edge, or undocked
  to float. Its tabs are ALL / PROJECT / PLUGINS / LIBRARY / SOUNDS / STARRED, the active tab shown
  with an orange outline. It has a view zoom (Compact, Normal, Large, Huge, Massive), view layouts
  (Standard, Columns, Samples, Plugins), a tags footer, and a preview panel with Title/Date/Size/Path.
  FL 2026 made it wider by default and added in-line waveforms.
- **Workspace.** The Playlist, Channel rack, Piano roll, Mixer, plug-in windows and Event editors are
  **free windows inside the FL desktop** (MDI). Each has a title bar with ▸ menu, title, and
  minimise/maximise/close buttons.

### 3.2 How windows float, snap, maximise and detach

- **Snap.** "Windows will snap together when they are dragged close to each other. Override this
  behavior by holding the Alt key while dragging." (manual, User Interface page.)
- **Resize.** Drag any window edge. The maximise button fills the workspace.
- **Detached.** From each window's ▸ menu (Playlist, Piano roll, Mixer, plug-ins, Browser), a window
  becomes an OS-level window that can move to another monitor. Detached plug-ins become separate
  applications to Windows, which has focus side effects that the manual lists.
- **FL desktop across monitors.** "Maximize to all monitors" (right-click ✕), or a windowed FL that
  you resize across screens (41).
- **Layouts** (View › Arrange windows):
  - Realign (Shift+Ctrl+H), and "Into workspace" (rescues off-screen windows);
  - the presets Desktop (default), Alternate, Tablet (default) and Tablet (alternate), which puts key
    toolbar items at the bottom;
  - Custom, and "Save custom layout";
  - Background (image, dynamic HTML wallpaper or colour).
- **Closing and aligning.** Close all windows (F12), close plug-in windows (Alt+F12), close unfocused
  windows (Ctrl+F12), and align channel editors (Shift+F12).
- **Plug-in window focus.** With "Auto select linked modules", opening one instrument closes the
  previous one. Alt-click keeps several open.
- **Multi-touch.** A 2-finger drag on a title bar moves the window, a 2-finger tap maximises, and a
  spread tap closes.

### 3.3 Splitters and in-window resizing

- **Browser divider.** Drag to resize. It "can snap to a second position": after moving it,
  right-click swaps between the new and the original width. This is a two-state splitter.
- **Piano-roll splitter** (notes over event lane). Drag to resize. **Right-click returns it to the
  original position.**
- **Mixer.** The three vertical dots on the dock dividers are a **drag handle for the fader-to-meter
  height ratio**. Clicking the dock splitters opens or closes the left and right docks.
- **Playlist.** Drag the dividers between track headers to resize one track. Dragging a track's top
  edge further up **groups** it with the track above. "Resize all tracks" offers 33–200%, and the
  **track-height** handle at the top right zooms all tracks.
- **Channel rack.** Resize buttons (⋮) sit in the title. Dragging the bottom-right corner changes the
  pattern length.
- **Two-dimensional scrolling.** Right-drag a scrollbar to move horizontally and vertically at once,
  or middle-drag inside the Playlist, Piano roll or Event editor.

### 3.4 Keyboard

The window keys are F5 (Playlist), F6 (Channel rack), F7 (Piano roll), F9 (Mixer), Alt+F8
(Browser), F8 (Plugin picker), Ctrl+F8 (Project picker) and F10 (settings). The
mixer keys are F2 (rename/colour), Ctrl+L (route selected channels here), Ctrl+Shift+L (route in
sequence), Alt+←/→ (move track), Alt+W (waveform meters), S / Alt+S (solo / solo with routing) and
Alt+R (render armed). Also: Page Up/Down zooms around the cursor, Shift+1/2/3 are quick zooms,
Shift+4 zooms out and Shift+5 zooms to the selection.

---

## 4. The mixer (the reference Cantus cares about most)

### 4.1 Chrome

The top row (06) is part of the window's title bar, 40 px @150 and `#4e585e`. From the left:

- ▸ mixer menu;
- the multi-touch toggle;
- the meter-wave view toggle;
- extra track properties (the cyan docks icon);
- the **Track inspector / FX panel** toggle (yellow square);
- **+** add tracks (right-click offers Add one, Add multiple…, Add from state);
- the **layout menu** ("Compact", "Wide", …).

On the right, the **window title shows the selected track: "Mixer - Synths Out"**. The context is in
the title, so the inspector needs no header of its own.

### 4.2 Strip anatomy, top to bottom (06, Compact layout, @150; about ÷1.5 for @100)

| # | Part | Size @150 | Look |
|---|---|---|---|
| 1 | Track number cell | 30 × 23 px (full strip width, no gaps) | track colour, pastel; number centred, 15 px digits; selected: lighter/outlined |
| 2 | Name | about 85 px tall, vertical (reads bottom to top), bottom-aligned | 12 px cap, `#c4cbd2`, middle-elided. "Alt" layouts put names under the meters; "Names at top" pins them |
| 3 | Peak meter | a well 28 px wide, about 75 px tall; two bars 7 px each, 2 px apart | well `#3f494d`/`#3b4449`; bars solid lime, turning yellow and orange near the top; peak-hold line; the meter can switch to a **waveform** view (Alt+W: "width equals peak level") |
| 4 | Mute LED | 22 px ring, 12 px core | green lens; left-click mutes, right-click or Ctrl-click solos, Shift-click locks, Alt-click solos with routing |
| 5 | Pan knob | 28 px | dark flat face and pointer; circular pan law |
| 6 | Fader | groove about 130 px; cap 24 × 37 px | cap light grey (section 1.4); green when selected, orange when armed; clicking above or below the cap glides to it |
| 7 | Invert phase / swap L-R | 16 px icons | small light glyphs |
| 8 | Stereo separation | 28 px knob | centre = off; right = toward mono |
| 9 | FX on/off (bulb) | 16 px | yellow `#ffe34f` = FX active; dark grey = no FX. Right-click opens every FX window of the track |
| 10 | Track delay / PDC clock | 16 px | grey (none), orange (auto), blue (manual) |
| 11 | Record arm | 8 px dot | grey `#cdd3d8`, turning red |
| 12 | Send switch / send knob | 16 px arrow | white ▲ = send available (click to send the *selected* track here); outlined = unavailable (it would loop); the selected track shows a green ▼; a linked send becomes a **knob**; sidechain-only sends are drawn low-contrast |
| 13 | Routing cables | — | curves from the selected track to its destinations ("Jiggly goodness!", optional) |
| 14 | Track scrollbar | below the sends | — |

The strip body is `#454f55` (selected `#4c575d`). There are no divider lines between strips unless
"Lines between tracks" is on. There are three docks: left and right are **fixed**, the middle scrolls.

### 4.3 Layouts ("8 view modes, from Compact to Extra large")

- The manual (mixer page) says the layouts interact with the window size you set. **Extra large**
  shows the FX list under each track: left-click an FX to open it, right-click an empty slot to add
  one. **Alt** views place names under the meters.
- Third-party sources name the modes **Compact, Compact 2, Wide, Wide 2, Wide 3, Extra large**, plus
  the "alt" variants. On one reference screen they show about 75, 35 and 26 tracks for Compact 2,
  Wide 3 and Extra large.
- Measured: Compact is 30 px @150 (about 20 @100) and Wide is 44 px @100 (01).
- "If your Mixer looks different… choose the Compact view and stretch the Mixer height to match
  (some features will not be visible if the height is too small)." **Visibility depends on height:**
  rows drop out rather than scroll.

### 4.4 Track inspector and FX panel (right side; can move left) with the Mixer Track Properties

This is the equivalent of Cantus's "ugly side panel", measured from 06 and 07 at @150.

| Part | Size @150 | Look |
|---|---|---|
| Panel | about 334 px wide, **fixed width** (a forum user notes it cannot be widened by dragging) | `#727c81`, inset in the chrome `#4e585e` |
| Input row | 34 px dropdown | icon at left (recording pick-up: EXT/MIX/FX/EQ/LVL/POST, with a micro caption under the icon); sunken dropdown "(none) ▸"; input-latency clock at right |
| 10 FX slot tiles | **26 px + 1 px gap** (18 + 1 @100) | ▸ menu chevron; name (loaded: `#eef1ef` upright; empty: "Slot 5" dim *italic*); **mix knob** with a white ring (ring = wet amount); **green LED** (enable). Mouse wheel over a slot **reorders** it. Right-click: Replace, Delete, Patcherize, presets, spare state, Move up/down, Rename/colour/icon, Help |
| EQ graph | about 210 × 130 px | 3 bands (low shelf, peak, high shelf). Left-drag = frequency/gain, right-drag = bandwidth/gain. Band readouts drawn in the graph ("3.9k -12.7") |
| Band gain faders | 3 mini faders beside the graph | same cap style, smaller |
| "Equalizer" caption and 6 mini knobs | 2 rows: frequency and bandwidth, each with an icon | caption large, dim |
| PDC row | clock icon, "20ms / 882smp", ▸ | wheel = 10 ms, Ctrl+wheel = 1 ms, Ctrl+Alt+wheel = 1 sample |
| Output row | 34 px dropdown | "(none) ▸", or "Front L/R" |

Why it looks good, even though it is only a list: every row has the **same height, the same left
gutter (the ▸ at the same x), the same right-hand furniture (knob and LED at the same x)**. The panel
is lighter than the strips, so it reads as a separate, raised surface. Empty rows are drawn rather
than hidden, so nothing jumps.

### 4.5 Scrolling (horizontal *and* vertical) and zoom

- **Horizontal:** mouse wheel over the track names or meters, a scrollbar below the sends, Shift+wheel
  over a track **reorders** it, a 2-finger drag on touch screens. The left and right docks never scroll.
- **Vertical:** FL's mixer **does not scroll vertically**. The window stretches vertically, the
  fader/meter height handle sets the ratio, and the layout mode decides which rows exist. If the
  window is too short, rows disappear. This is FL's answer to the user's "can't scroll down".
  Cantus can do better (section 12.5).
- **Zoom** means the layout modes (strip widths) and the UI scaling. There is no continuous zoom.

### 4.6 Selection, groups and routing

- Click a pan knob or fader to select a strip. Ctrl+Shift+click or drag on the label and meter area
  selects several; then moving one fader moves all of them. The first selected strip is highlighted
  slightly lighter ("this track" for the routing commands).
- Menu › Group: Separator (a visual gap, "not a dock"), Create group (separators on both sides, a name
  and an icon), and Auto color group.
- Menu › Routing: route or sidechain selected→this or this→selected, with "…only" variants that also
  cut the master send. "Create submix…" does the whole submix in one step. "Select tracks routed
  to/from this" (Ctrl+Alt+L).
- **FL 2025:** "Dynamic Mixer Tracks". Tracks are added and removed (up to 500), a **+** button sits
  after the last track (08: an outlined yellow trapezoid), deleting tracks can be undone, and you can
  drag an instrument onto the empty area. **FL 2026:** Alt-click resets I/O to "(none)", and clicking
  an empty FX slot shows only the plug-in menu.

---

## 5. Channel rack, playlist, piano roll, browser (anatomy and measurements)

- **Channel rack** (12, 13, 38, 37).
  - Rows are 30 px @100 (45 @150). Each row reads: LED 11 px; pan and volume knobs 13 px; routing
    LCD (sunken); the **channel button** 83 × 25 px @100 with the name centred and an instrument icon
    at the right edge; a selector bar; then 16 steps (12 px wide, 16 px pitch @100) in alternating
    groups of four (grey and maroon-tinted, `#6d7478` / `#7f6c6d` faces @150).
  - A melodic channel shows a **mini piano roll** in place of the steps.
  - The header holds ▸, the Loop Starter dice (FL 2025, 38), the filter "All ▸", "Channel rack", swing,
    the steps LCD, and graph-editor and piano-mode toggles.
  - FL 2026's hero image (`fls-26`) is made of step keys only, which shows how much Image-Line
    considers that keycap its signature.
- **Playlist** (14, 03, 04).
  - The picker panel (left) lists patterns and clips as coloured tiles with a waveform or mini-notes
    preview. Track headers are coloured blocks with the name, a type icon and a green enable LED, and
    grouped child tracks are indented under an ▴ toggle.
  - Clips have a **title bar** with a ▸ clip menu and a name (with "keep labels on screen" while
    scrolling), over the content in the same hue but darker.
  - The timeline has markers ("Start", "Repeat", "Chorus") in small tab shapes. The zoom-and-scroll
    bar shows a **mini playlist preview**.
  - FL 2026 clip title bars show pan and pitch only when they differ from the default.
- **Piano roll** (15).
  - The grid palette is in section 1.1. Notes are **pastel with a top-lit gradient** (mint `#ccffd5`
    → `#b8ecc2`, 1-px outline `#83ab89`; blue group `#c3d7fb` → `#b5caef`, outline `#7f8da9`). Ghost
    notes are flat `#647782`. Note names appear inside the notes.
  - The preview keyboard is realistic (black keys `#404143`–`#5e5f61`, white keys `#dadee6`–`#e7ecf3`).
    The note-colour selector is a big slanted tab at the top-left.
  - The event lane sits below a **splitter handle** (a dotted grip; right-click resets it). Velocity
    stems have circles on top.
  - FL 2026 adds user key labels and note labels (a pill editor, see `web_contact`), and a chord panel.
- **Browser** (16, 03). The deepest surface (`#1f282d`). Tree rows are about 25–27 px @100 with icons,
  special folders get coloured text, and the plug-in database shows **thumbnails of each plug-in's
  UI** under its name.

---

## 6. Plug-in and instrument windows

### 6.1 The wrapper frame (22, 09, 29, 35)

- **Title bar**: 24 px @100, `#3f484d`, a 1-px top highlight `#495257` and a 1-px outer `#2a3338`.
  From the left:
  - **▸** plug-in menu;
  - a **gear** (settings; it lights up for instruments);
  - **"Fruity Limiter (Master)"**, the plug-in name in light text with **the host track in dimmer
    text in parentheses**;
  - the typing-keyboard-priority icon;
  - a **preset box**: an inset `#374045` with the preset name in dim text (or "Presets"), then
    **◀ ▶** in `#d2d8dc`;
  - minimise and close.
- **Instruments add a channel-settings strip** (45 px, `#21272b`): the plug-in, settings and
  wrench tabs, then ON, PAN, VOL, PITCH, RANGE and TRACK. These are small knobs with **bold micro
  caps labels** (about 7 px) below.
- **The plug-in menu** has: presets (Randomize, Hybridize), save preset (*draggable*), a spare state
  (store and flip, a compare feature), link all parameters, CPU options, make an editor thumbnail,
  rename/colour/icon, **Scaling** (Host / System / 100–400%, for native plug-ins), and **Detached**.
- Resizing: Parametric EQ 2, FLEX and other native plug-ins resize freely or by scaling factor.

### 6.2 What the native plug-ins share

- **A display at the top that is also a control**, full-bleed, darker than the controls: the EQ
  spectrum with draggable tokens, the Limiter's gain envelope, the Maximus curve, the Frequency
  Splitter's band fills, the LuxeVerb spectrogram.
- **Sections** with an UPPERCASE header band or tab, separated by 1-px lines or gaps.
- **Knobs with value arcs in a per-section colour.** In the Limiter, loudness is purple/red,
  compression cyan, envelope pale and gate brown. The **primary** parameter of a section is a bigger
  knob: Delay 3's TIME and LEVEL, LuxeVerb's DECAY, FLEX's Cutoff and Mix are about 1.4–1.8× the
  size of the others.
- **UPPERCASE labels under the knobs**, all on one baseline per row.
- **Colour coding of repeated units.** Parametric EQ 2 gives each of its 7 bands one hue (purple,
  pink, orange, yellow, green, teal, blue) and uses it on the token, the band fader, the type icon,
  the slope dots and the FREQ/BW knob arcs. Maximus colour-stripes its LOW / MID / HIGH / MASTER tabs.
- **A wordmark** in a corner, large and dim: "PARAMETRIC EQ 2", "REEVERB 2", "FREQUENCY Splitter".
  It signs the plug-in without competing with the controls.

### 6.3 Per plug-in

- **Fruity Parametric EQ 2** (23).
  - A dark spectrum area with a **note ruler (C1…C9) and a band-name ruler (SUB, BASS, LOW MID, MID,
    HIGH MID, PRS, TREBLE)** along the top, and boxed Hz cells (20, 50, 100 … 10k) along the bottom.
  - Numbered circular band tokens; a white summed curve; a coloured histogram behind it.
  - On the right, 7 vertical band faders plus a master gain rail, each topped by its band-type glyph
    and slope dots.
  - At the bottom, round icon buttons (▸ menu, LIN, HQ+, ✓, MONITOR, COMPARE).
  - Interaction: drag a token (wheel changes bandwidth, Alt resets, Ctrl is fine, Shift-click solos
    with "band solo on shift"); right-click a token for Type / Order / Key. The window resizes.
- **Fruity Limiter** (24, 09). The display is on top, with a vertical tool strip at its left. Below:
  **LOUDNESS | ENVELOPE | NOISE GATE** header bands, knob rows with per-section arc colours, a
  **LIMIT | COMP** segmented switch (selected = dark fill with white text), and a ⊢⊣ bracket tying
  ATT and REL to the CURVE and SIDECHAIN spinners.
- **Vintage Chorus** (25) and **Kepler** (34), homages to the Juno. They share a system: a black
  brushed ground; **coloured header bars** (blue MODE/ARPEGGIO, red DELAYS/LFO/DCO/VCF, cream LEVELS)
  with **matching coloured rules along the bottom** of each section; slider faders with tick scales;
  big square lit pads (yellow, amber, orange); red LEDs; a large chrome wordmark.
- **FLEX** (26, 27; FL 2021–2025 UI).
  - Header: the "FLEX" wordmark; text tabs **Play · Edit | Library · Store** (the active one bright,
    the others dim); a search pill with Tags, history and star; the master knob; a ⋮ menu (Theme,
    panel locks, presets panel left or right, …).
  - The preset list on the left carries artwork.
  - Sections have **Title Case caps headings with a power icon** (POST FILTER, DELAY, REVERB, LIMITER),
    white knob faces with thin arcs, and Title Case labels (Pitch, Cutoff, Res).
  - A **macro row over artwork** runs along the bottom.
  - **FL 2026 rebuilt FLEX**: new engine, "redesigned UI with simplified controls", an expanded preset
    browser. I found no official full screenshot of the 2026 FLEX UI in the manual or on the product
    pages. The manual's FLEX images show the previous UI.
- **LuxeVerb** (28, 29, FL 2026 close-up).
  - The top half is a saturated cyan panel `#0891b9`. Each control is a **cell** `#0882a6` (rounded
    about 8 px) containing a **value pill** `#d0e6ee` with dark text `#413f4f` (7.9:1), then the knob,
    then the white caps label.
  - Section headings are INPUT, REVERB, PITCH and OUTPUT (white, about 17 px @150). Small "T" and
    "L" boxes are sync and link toggles.
  - The bottom half is a dark envelope and spectrogram area.
- **Fruity Delay 3** (30). Every section is its own raised panel with a header band (DELAY TIME,
  DELAY MODEL, FEEDBACK, MODULATION, DIFFUSION, FEEDBACK DISTORTION). Lime arcs, an orange arc for
  the primary LEVEL, a vertical WET fader in an INPUT column, an OUTPUT column, and a small
  transfer-curve display.
- **Emphasis** (35, FL 2025). Near-black; orange arcs; section labels as tiny letter-spaced
  UPPERCASE **tabs** (INPUT GAIN, ENHANCEMENT, ROUTING, OUTPUT); an L/R | M/S segmented switch; a
  **footer of text toggles** (LOCK, TRUE PEAK, 2X, BYPASS, DELTA, LEVEL MATCHING); tall meters on
  the right with numeric readouts under them. It is the most modern-looking native plug-in.
- **Maximus** (32). A transfer-curve display beside a waveform display, and band tabs in large
  *italic* (LOW, MID, HIGH, MASTER) with coloured left stripes. Knobs sit in grouped darker panels
  (THRES / SAT / CEIL), with ▼ markers under the envelope knobs.
- **Frequency Splitter** (31) and **Reeverb 2** (33). Dim section captions set *inside the bottom-left*
  of each group (CROSSOVER, MAIN OUT, SENDS), and a glowing 3D room display.

---

## 7. Controls reference (behaviour)

- **Knobs and sliders** (every FL control):
  - drag vertically (sliders horizontally);
  - **Ctrl, or both mouse buttons, for fine adjustment**;
  - they **pause at the default and at other useful values while dragging; Shift disables this**;
  - **Alt-click or middle-click resets** to the default;
  - right-click opens a menu: Type in value (context or normalised units), **"Set", which opens a
    larger control for accuracy**, Create automation clip, Link to controller, Edit events, Copy and
    Paste value;
  - **every automatable control shares this menu**.
- **LCD numbers**: drag, click the top or bottom edge to step, or click-hold and type.
- **Menus**:
  - Checkable items can be **right-clicked to toggle without closing the menu**.
  - Items with a drag icon (save preset, save track state) can be **dragged** onto targets. Dropping a
    saved mixer track state onto another track copies its whole FX chain.
- **Hover**: by default it is subtle (icons brighten a little; the cursor changes over editable LCD
  edges and splitters). Its main effect is to **update the hint bar**. "High visibility" adds hover
  rectangles around switches and white scrollbar hover.
- **Active state**: an orange gradient key (toolbar), a lit LED, a lime fader cap, or a bright tab
  outline (browser tab "ALL" outlined in orange, dialog tab text `#3fc3ff` on a darker tab `#32383c`).
- **Toggles**:
  - vertical pill switches with a white ball (the Limiter's MID/SIDE, Delay 3's delay model);
  - segmented text switches (LIMIT | COMP, L/R | M/S);
  - LEDs;
  - lit square pads (Vintage Chorus, Kepler).

---

## 8. The hint bar (exact behaviour, from the manual)

1. **Position.** It is a panel at the left of the toolbar, under the menu bar: line 1 shows the
   license and project name (dim), line 2 the hint (bright). At the right edge sits a **MIDI/sync
   indicator**: orange = handled, blue = global link, green = unhandled, grey = nothing.
2. **Hover.** "If you place your cursor over any control a short description of it will appear
   here." Example: "Mix level".
3. **Drag.** "The hint bar will also display the value of a wheel or slider during adjustment."
4. **Click.** "If you click on a control, the value will be displayed." The value can be
   right-clicked and copied.
5. **Meters.** Hovering over most peak meters shows the **value at the pointer position**.
6. **Capability icons at the right.** A record icon (automatable), a MIDI plug (linkable), a right
   mouse button (has a right-click menu), happy and sad faces (a search worked or failed), a
   **warning** (for example a plug-in adding latency), a clock (long operation), fast-forward and
   rewind (scrubbing), a group icon (linked events).
7. **Context readouts.** Hovering a mixer track's PDC icon shows the detected latency.
   "Latency X ms (manual) + Y ms (plugin)" appears for the wrapper latency.
8. **Extended Hint Panel.** It is optional (View › Toolbars › Hint bar): a **larger floating panel,
   transparent to mouse clicks**, with the same content, a **progress circle** for long operations,
   and a centre grip to move it. It fades slightly when the pointer is over it. "On larger displays
   the empty tool-bar area along the top has proven popular."

---

## 9. Navigation and speed

- Wheel over mixer names or meters scrolls the strips. Shift+wheel reorders strips. Wheel over an FX
  slot reorders slots. Wheel over the PDC value steps it in 10 ms / 1 ms / 1 sample.
- **Zoom by the scrollbar**: drag either end of the horizontal scroll handle. **Ctrl+wheel** zooms at
  the cursor. The vertical zoom box sits at the top of the vertical scrollbar. Page Up/Down,
  Shift+1–6 quick zooms, **Ctrl+right-drag zooms to an area**, and Ctrl+right-click un-zooms.
- **Middle-drag pans** the Playlist, Piano roll and Event editor in two dimensions. Right-drag on a
  scrollbar also scrolls in 2D.
- **Drag targets everywhere**: plug-ins from the browser or picker onto a mixer track (which names
  the track), samples onto the Channel rack *button* (the window comes to the front while you drag),
  presets onto plug-ins, mixer states onto tracks, and a menu's "save" item onto a target.
- **Middle-click** reset and rename. **F2** rename/recolour (press F2 again to re-roll the colour).
  **Alt** disables snap while dragging events or windows.
- Linking: select channels, click a mixer track, Ctrl+L.
- Undo history is available as a Browser view. In FL 2025, deleting mixer or playlist tracks is undoable.

---

## 10. FL in 2025–2026: what changed in the look

- **FL 2025**: dynamic mixer tracks with a **+** add button; the Loop Starter; Gopher, an assistant
  panel; vectorial UIs for BassDrum, Sakura and Drumaxx; the Emphasis limiter; Mobile Rack.
- **FL 2026**:
  - FLEX rebuilt, with a simplified UI and a large browser;
  - the Browser 3 packs wide in Sounds, with in-line waveforms;
  - clip title bars that show gain, pan and pitch only when changed;
  - user key labels in the piano roll, and a chord panel;
  - a "Remix a song" dialog;
  - modal dialogs over a **dimmed** workspace (02);
  - menus with **section header bands** (36).

---

## 11. What Cantus should take (ranked), and what not to copy

### Take, in order of impact

1. **The colour system**: a hue-locked neutral ladder (L 15→49%), 4–6 fully saturated light accents
   used only on small elements, pastel user colours at equal lightness, and a "safe colour" clamp in
   the picker. This is the single biggest source of FL's beauty.
2. **A condensed UI face at small, fixed sizes**, with middle-elision, vertical strip names, and every
   label in a box it owns. This directly answers "the letters run into each other" and "loose words".
3. **One lighting recipe** for raised caps, sunken wells and tiles, reused by every control.
4. **The hint bar**: one place where every control names itself and shows its value on hover and
   drag, with capability icons.
5. **The mixer strip**: a fixed row order, a flexible meter and fader with a ratio handle, width
   modes, pinned docks, wheel scrolling, and Shift+wheel reordering.
6. **The inspector as a uniform tile column**: title "Mixer – <track>", then input, 10 (or N)
   identical slot rows with ▸, name, mix ring and LED, then EQ, delay and output. This replaces
   Cantus's current side panel.
7. **The plug-in frame and vocabulary**: a 24 px header with menu, name, "(track)", preset ◀ ▶ and
   close; a display on top that is a control; section bands; UPPERCASE labels under knobs; arc colour
   per section; a bigger knob for the primary parameter; a quiet wordmark.
8. **Windows**: snapping (Alt to override), detach, named layouts, and splitters with a **right-click
   reset / second position**.
9. **Theme knobs**: Hue, Saturation, Brightness, Contrast and Text drive the whole ladder, plus role
   colour pickers. Users can make it theirs without breaking the harmony.
10. **Navigation gestures**: zoom by dragging the scrollbar ends, middle-drag pan, Ctrl+right-drag
    zoom to area, quick-zoom keys, wheel over a list to reorder it.
11. **Text on coloured items tinted toward the item's hue**, lightened on dark and darkened on light.
12. **The channel-rack keycap**, as the signature control: the step button with its slit.

### Do not copy

- **MDI windows piling up** inside a workspace. FL users constantly move and close windows (F12
  exists for a reason). Cantus should dock by default and float by choice.
- **Hidden functionality**: right-click-only features and tiny unlabelled icons. FL leans on the hint
  bar because its surface is cryptic (invert, swap, PDC clocks, send arrows). Cantus should keep the
  hint bar, *and* give a visible label or tooltip for every icon.
- **The fixed-width, non-resizable inspector** and the need to "stretch the mixer vertically to see"
  the EQ.
- **Each plug-in inventing its own style** (brushed Juno homages, cyan LuxeVerb, near-black Emphasis).
  Cantus's editors should share one system, with per-module hue only.
- **Low-contrast placeholder text and 2:1 dim labels** as body text. Cantus's WCAG tests should keep
  body text at 4.5:1 or more. Use FL's dim level only for true placeholders.
- **Humorous UI strings** ("Plugin-be-gone", "Feeling lucky punk?") and "Transparent menus".
- **Bitmap textures** (brushed metal). Keep the vector gradients.

### What "better than FL" means, concretely

- An inspector that is **resizable** (drag its edge, with snap widths and a right-click reset) and
  **collapsible**.
- A mixer that **fits first, then scrolls**. Rows drop out as FL does, but when even the minimum does
  not fit, it scrolls vertically with a visible scrollbar and the wheel. FL never scrolls vertically.
- **An on-control value readout while dragging** (a small pill by the cursor) *in addition to* the
  hint bar. FL only shows the value in the hint bar or in newer plug-ins' pills.
- **Keyboard reachability** for every control, with focus rings (Cantus has these already). FL is
  mouse-first.
- Docked panels **and** floating windows **and** named layouts, all in one serialisable tree
  (egui_dock plus `layout.rs`).
- One plug-in design system instead of FL's many.

---

## 12. FL's style, translated to Cantus (implementable spec)

All sizes are **egui points at 100%**. All colours are **theme tokens (data)**, so the user's colour
editor keeps working. The component names follow `cantus-audit.md` §5.1 (`Section`, `Param`,
`Toggle`, `Segmented`, `IconToggle`, `TitleRow`, `Readout`).

### 12.1 Default theme: make the FL-derived dark theme the default

**Recommendation:** ship a new **"Slate"** theme built from FL's ladder, and make it the default.
Keep **Paper** as the light option, rebuilt with the same rules (12.2).

Reasons:

- The user named FL as the style they like most.
- FL's look *depends* on mid-dark neutral surfaces (L 30–48%). Those are what let pastel colours,
  light caps and 100%-saturation accents glow, and what make depth visible in both directions.
- FL itself treats light themes as secondary presets.

The comment in `theme.rs` ("a DAW is looked at in daylight…") argues for *offering* light, not for
defaulting to it.

**Slate tokens.** These are existing `Palette` fields plus the new ones marked **NEW**, taken
directly from the measurements above.

| Token | Slate (dark, default) | Role |
|---|---|---|
| `ink` | `#1f282d` | deepest: lists, browser, LCD and meter-well backgrounds |
| `well` NEW | `#3b4145` (gradient `#42484c` → `#353a3f`) | sunken fields: dropdowns, readouts, number boxes |
| `meter_well` NEW | `#3f494d` | meter background |
| `row_black` / `row_white` | `#374954` / `#3e4f5a` | piano-roll rows; arrangement grid ground `#2e3e48` |
| `grid` / `grid_strong` / `grid_bar` | `#394b56` / `#2e404b` / `#1a2c37` (with `#354652` sides) | step, beat and bar lines |
| `strip` NEW | `#454f55` (selected `#4c575d`) | mixer strip bodies |
| `panel` | `#4e585e` | window chrome, title rows, the desk |
| `toolbar` NEW | `#596267` | the top toolbar |
| `panel_raised` | `#606b71` | tiles: FX slots, list rows, cells |
| `panel_light` NEW | `#727c81` | surfaces behind tiles (inspector, rack body); **no body text directly on it** (3.8:1) |
| `header_cell` NEW | `#768086` | the neutral track-number cell |
| `text` | `#eef1ef` | primary text: 4.8:1 on tiles, 7.4:1 on strips |
| `text_dim` | `#b0bac0` | labels (4.25:1 on strips; use `#c4cbd2` where 4.5:1 is required) |
| `text_placeholder` NEW | `#9ca2a8` | "Slot 5", "Insert 12": placeholders only |
| `accent` (highlight) | `#ffaa54` (Cantus identity alternative: gold `#f2c14e`) | active toggles; gradient `#ffa558` → `#ffeb9d`, glyph `#b13f00` |
| `selection` | `#c3ff5a` | selected strip/fader, LEDs, selected channel |
| `focus` | `#3fc3ff` | keyboard focus ring, active tab text |
| `ok` / mute LED | `#8def11` | enable/mute LEDs |
| `danger` / option, record | `#ff6135` | record arm, option LEDs, clip |
| `fx_on` NEW | `#ffe34f` | FX-active bulb |
| `meter_low` / `meter_high` / `meter_clip` | `#c4fc55` / `#ffdf5a` (from −6 dB) / `#ff8d3e` (from 0 dB) | meters |
| `step_odd` / `step_even` NEW | `#6d7478`→lit `#d1e3ee` / `#7f6c6d`→lit `#fdccce` | step groups |
| `menu` / `menu_band` / `menu_hover` NEW | `#b8c3c9` / `#8f9da5` / `#5a6c76` | light menus on the dark app (FL 2026) |
| `shadow` | `#000000` at alpha 0x80, blur 12, offset (0, 3) | floating windows and menus |
| `corner_radius` | 2 (caps, tiles), 6–8 (plug-in cells), 3 (windows) | — |

**Track colours (dark).** Use the FL pastel band: **S 25–40%, L 58–64%** for small fills (headers,
mixer number cells), and **S 25–40%, L 36–46%** for large fills (clips, lane headers, full-height
"colourful mixer" tints at alpha 0.10 / 0.20 / 0.35). The picker's **"safe colours" clamp** enforces
the band and shows the chosen colour next to its safe version.

**Text on coloured fills.** Use the fill's hue at **L 85–90%, S 40–80%** on dark fills (FL:
`#cbe9d5` on `#407840`). Keep the WCAG floor: raise L until the pair reaches 4.5:1, since FL's own
tints land at 3.2–4.1:1.

### 12.2 The light theme with the same rules ("Paper" rebuilt)

Invert the ladder, keep the hue (≈202°) and the recipes.

| Token | Paper (light) |
|---|---|
| `panel` (chrome) | `#dfe4e7` |
| `toolbar` | `#d5dbdf` |
| `panel_light` | `#e9edf0` |
| `panel_raised` (tiles) | `#f8fafb` |
| `strip` / selected | `#e4e9ec` / `#d8e0e5` |
| `well` / `meter_well` | `#cfd7dc` (top inner edge `#b9c3c9`) / `#2f3a40` (**keep meters dark**: lime stays readable) |
| grid rows | `#f4f6f7` / `#eaeef1`; lines `#dbe1e5` / `#c5ced4` / `#9fabb3` |
| `text` / `text_dim` / placeholder | `#172024` / `#4d5a62` / `#8a959c` |
| accents | as FL's light themes: **one** strong accent, e.g. Cantus gold `#956608` or FL-orange `#c9650f`; selection `#2563eb` or a deep lime `#4d8a00`; LED cores keep their bright colour inside a darker ring |
| track colours | S 35–45%, L 50–58%; text on fills = **darkened** tint (L 18–25%), as in FL's note labels |
| bevels | highlight edge `#ffffff`, shadow edge −12% L; drop shadows `#1a1d23` at alpha 0x38 |

### 12.3 Typography spec

- **Face:** bundle **IBM Plex Sans Condensed**, Regular, Medium and SemiBold (OFL, and the same
  family as Cantus's current fonts), as the UI face for names, labels and lists. Keep IBM Plex Sans
  for long-form text (dialogs, help) and IBM Plex Mono for numeric readouts **if** the condensed
  face's digits turn out proportional. egui cannot turn on OpenType `tnum`, so verify the digit
  advance widths once when the font is added.
- **Roles.** They go in `design.rs`, and a test should ban `FontId::new` elsewhere, as the audit
  proposes.

| Role | Size | Weight | Case | Colour token | Use |
|---|---|---|---|---|---|
| `micro` | 8.5 | SemiBold | UPPER, +0.4 pt tracking | `text_dim` | captions under tiny controls (ON, PAN, VOL), column captions |
| `label` | 11 | Regular | UPPER (plug-ins) or as named | `text_dim` | knob and fader labels, strip names |
| `body` | 12.5 | Regular | as named | `text` | slot names, list rows, channel names, menus |
| `title` | 13 | Medium | Title case | `text` (context part `text_dim`) | window, editor and panel titles |
| `section` | 14 | Medium | UPPER | `text_dim` on a band | section header bands |
| `readout` | 12.5 / 16 | Regular | digits | `text` | value pills and LCDs; fraction at 0.75× the size |
| `display` | 26 | Light or Regular | digits | `text` | transport clock |

- **Rules:**
  1. Every text call takes a **box** (`Rect`) and a policy: `Elide::End`, `Elide::Middle` (default
     for names) or `Rotate90`. It never paints outside the box. Middle elision measures candidate
     strings with egui's `layout_no_wrap` and binary-searches the cut, joining with "…".
  2. **Mixer names are vertical** (egui `TextShape` with `angle = -π/2`) when the strip is under
     36 pt wide, and horizontal and elided above that.
  3. One baseline per row: all labels of a knob row share a y.
  4. Placeholders are italic (Plex Sans Condensed Italic) in `text_placeholder`.

### 12.4 Component recipes (egui custom painting)

Use `epaint::Mesh` with per-vertex colours for vertical gradients (4 vertices per rect; rounded rects
as a polygon fan, or as a plain gradient rect with the corners covered by an outline stroke).

1. **`Cap`**: the raised recipe for fader caps, step keys, channel buttons and toolbar keys.
   - `outline` 1 px = base −20% L;
   - `face` a vertical gradient from base +6% L at the top to base −6% L at the bottom;
   - `top_edge` a 1-px line at +15% L inside the top;
   - `groove` a 1-px line at base −35% L, 30% of the height from the top (step keys, fader caps);
   - `drop` 2 px below: base −25% L at alpha 0.6, fading to 0 (or `epaint::Shadow { offset: [0, 1],
     blur: 3 }`).
   - **Lit** = the same recipe with the base swapped for the role colour (a lime fader, a white-blue
     or pink lit step).
2. **`Well`**: the sunken recipe for dropdowns, readouts, meter wells and number boxes. Fill
   `well` → −4% L at the bottom; a 1-px border base −8% L; a 1-px **inner top line** at −12% L; text
   right-aligned for numbers and left-aligned for names; a ▸ chevron 6 pt from the right for menus.
3. **`Tile`** (FX slot, list row). 18 pt tall with a 1-pt gap, `panel_raised` fill, 1-px border base
   −10% L. Left to right: ▸ chevron (8 pt), name (`body`, elided), **mix ring knob** (14 pt; a white
   ring = wet amount), **LED** (9 pt). Empty = the same geometry, the italic placeholder "Slot 5",
   and fill +3% L.
4. **`Led`** (11 pt). From the outside in: a 1-px ring at L 60% alpha 0.5; a 1-px dark ring (role
   × 0.55); a 1-px bright rim (role +8% L); the core role colour. Off = the same geometry, core base
   `#3d4a42`.
5. **`Knob`** (plug-in style). An outer diameter of 28–40 pt, and 1.5× for the primary parameter.
   - a **ring track** 4 pt wide at `well` −6% L;
   - a **value arc** in the section colour, drawn **from the default value** (as Cantus already does);
   - a face disc inset 2 pt, gradient top-left +8% L to bottom-right −8% L;
   - a pointer from 40% to 88% of the radius, 2 pt, in the arc colour;
   - the label (`label`, UPPER) centred 6 pt below.

   The mixer and rack knobs are smaller (14–20 pt), with no arc: a flat face, a 1-px outline and a
   light pointer.
6. **`Fader`**. A groove 4 pt wide at `well` −10% L, with tick marks at ±30% alpha every 6 dB. The
   **cap is a `Cap` 16 × 25 pt**, base `#c9ccd6` for the light grey cap (lime when the strip is
   selected, orange when armed). The groove below the cap is filled with the selection colour when
   selected.
7. **`Meter`**. A `meter_well` with a 1-px darker border. Two bars, each 40% of the well width,
   with a 1–2 pt gap. **Solid** fills (no segments): `meter_low` up to −6 dB, `meter_high` up to
   0 dB, `meter_clip` above. A 2-pt peak-hold line. Hovering shows the dB value at the pointer in the
   hint bar (FL behaviour). A waveform mode toggles with Alt+W.
8. **`SectionBand`**. A full-width band 20 pt tall at the panel −8% L, with the `section` label left
   (or centred), 1-px vertical dividers between sections, and an optional power `IconToggle` at the
   left (FLEX). The dialog variant is a tab with a **45° slanted right edge** (FL settings 19).
9. **`Cell`** (LuxeVerb pattern, recommended for Cantus plug-ins). A rounded rect (radius 6), the
   panel −6% L, padding 6. Inside, top to bottom: a **value pill** (`Readout`, 16 pt tall, pale fill
   L 88% with dark text), the knob or fader, the label. This is the cure for "loose words": value,
   control and name become one object.
10. **`Segmented`** (LIMIT | COMP). The selected segment is filled at `well` −10% L with `text`; the
    others show `text_dim` on the panel; there are 1-px dividers.
11. **`ToolbarKey`**. 28 × 28 pt, `Cap` with base `#4a5358`. Active = the `accent` gradient
    (`#ffa558` edge → `#ffeb9d` centre band) with a glyph at accent −55% L.
12. **`Menu`** (optional FL look). Rows 22 pt, `menu` fill, dark text; section headers as 18-pt
    `menu_band` rows with light text; hover `menu_hover` with white text; 1-px separators; a
    check/radio column at the left and a chevron at the right. Right-click on a checkable item toggles
    it without closing.

### 12.5 Mixer spec

- **Width modes** (Ctrl+wheel over the strip area cycles them; the choice is saved per layout):
  Compact 24 pt, Wide 48 pt, Extra large 76 pt (with an FX list inside the strip, as in FL). FL's
  Compact is about 20 pt, but Plex Condensed needs a little more.
- **Strip, top to bottom:**

  | Row | Height | Contents |
  |---|---|---|
  | Number cell | 16 | the track colour |
  | Name | 64 | vertical below 36 pt of width |
  | Meter | flexible | well, minimum 48 |
  | Mute / solo | 14 | LED, plus a solo LED |
  | Pan | 22 | knob |
  | Fader | flexible | minimum 80 |
  | Phase / swap / width | 16 | icons, optional |
  | FX and record | 14 | FX bulb and record dot |
  | Sends | 18 | arrows or knobs |
  | Scrollbar | 8 | — |

  The meter and the fader share the flexible height. A **3-dot handle** between the docks sets the
  ratio (FL); double-clicking it resets.
- **Vertical fit, then scroll.** When the window is short, drop rows in this order: width knob →
  phase/swap → name height (to 40) → sends (collapse into one "→" chip). If it still does not fit,
  **scroll vertically** with the wheel and show a visible 6-pt scrollbar. FL never scrolls vertically;
  this is where Cantus is better.
- **Horizontal:** the wheel over the name, number or meter area scrolls strips (a plain wheel, as in
  FL). **Shift+wheel reorders** the strip under the pointer. Master and pinned strips live in a
  **fixed left dock**, and a right dock holds returns. The middle scrolls.
- **Selection:** strip body `strip` → selected; lime fader cap and groove; an outlined number cell.
  Ctrl+Shift drag selects several, and moving one fader moves the whole group.
- **Colours:** the number cell takes the track colour. The "Colourful mixer" setting is Off / Low /
  Med / High = full-height tint at alpha 0 / 0.10 / 0.20 / 0.35.
- **Inspector** (the right side panel, replacing `views/inspector.rs`'s current look):
  - default width 280 pt, **resizable 220–420 by dragging its edge**, a snap stop at 280,
    right-click on the edge to reset, collapsible to a 24-pt rail;
  - title in the panel's own `TitleRow`: "Mixer – <track name>" (the context part dimmed);
  - then: input row (`Well`, 22 pt) → N `Tile` slots → the EQ graph (a 140 pt tall display that is a
    control, with 3 band tokens and 3 mini faders) → 2 rows of 6 mini knobs → a delay/PDC row with a
    readout → the output row;
  - every row shares the same left gutter (8 pt) and the same right furniture column.

### 12.6 Plug-in and instrument editor frame

- **Header**: 24 pt, `panel` with a 1-px top highlight (+6% L) and a 1-px outer line (−20% L). From
  the left: ▸ menu, module icon, **"Aequalis (Insert 3)"** (the name in `title`, the host in
  `text_dim`), then the flexible space, then a **preset box** (`Well`, 160 pt, the name elided,
  ◀ ▶ buttons), A/B, bypass `IconToggle`, minimise and close.
- **Body**: the display on top (full-bleed, `ink`-based, and interactive), then `SectionBand`s over
  rows of `Cell`s or knobs. Give each module one **hue** for its arcs and bands (FL's per-section
  arc colours; the audit's "per-module hues").
- **The primary parameter is 1.5× the size.** Add a wordmark bottom-right in `section` × 1.6 at
  alpha 0.25.
- **Scaling** per editor (100–200%). Detached = its own OS viewport, when egui multi-viewport is
  used; otherwise floating.

### 12.7 Windows, docking and splitters

- Main areas go in **egui_dock** (per the audit), skinned with these tokens: tab bar `panel`, active
  tab `panel_raised` with an accent underline of 2 pt, and 4-pt splitters that show a 1-px `grid_bar`
  line on hover.
- Floating editors are `egui::Window`s with the frame from 12.6. They **snap**: while dragging, if
  any edge comes within 8 pt of another window's edge or of the workspace edge, align to it; **Alt
  disables snapping** (FL).
- **Every splitter**: drag to resize; **right-click resets** to its default; double-click toggles
  between the last two positions (FL's Browser divider). Remember sizes in `layout.rs`.
- View › Layouts: Desktop, Alternate, Custom (save). F5 / F6 / F7 / F9 toggle Arrangement, Channel
  rack, Piano roll and Mixer. F12 closes all floating editors.

### 12.8 The hint bar in Cantus

- Add a toolbar panel (about 280 × 34 pt) with two lines: the project name (`text_dim`) and the
  hint (`text`), plus a right-aligned **capability icon strip** (automatable, has a menu, warning).
- Each widget, on hover, writes `app.hint = Hint { name, value: Some(formatted), caps }`. During a
  drag, the value updates live. A **floating value pill** also appears 12 pt right of the pointer
  while dragging (better than FL). On a meter, the hint shows the dB value at the pointer.
- The optional **extended hint** is an `egui::Area` that is `interactable(false)` (click-through), at
  the bottom-left, larger text, movable by its grip.

### 12.9 Theme knobs (FL's Theme page, in Cantus's colour editor)

Add five sliders at the top of the colour editor: **Hue** (rotate every ladder token around its
current hue), **Saturation** (scale S), **Brightness** (shift L), **Contrast** (scale the L-distance
between ladder steps around the mid step) and **Text** (shift text L). Below them go the role
pickers: Selection, Highlight, Mute, Option, Steps (odd/even), Meters (3 stops), Text. Keep A/B
compare and "Save preset". Store the knob values in the theme JSON, derive the tokens, and keep the
existing per-token overrides and WCAG tests.

---

## 13. Screenshots

All are in `c:\projects\daw\research\ui\screenshots\fl-studio\` (git-ignored, local only).

Sources:

- **Manual base** = `https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/`
- **Assets** = `https://www.image-line.com/static/assets/` (the FL Studio product page
  `https://www.image-line.com/fl-studio/` and the 2026 release page
  `https://www.image-line.com/fl-studio/release/2026`)

| File | Shows | Source |
|---|---|---|
| 01-main-window-annotated-panels.png | whole window at 100% (toolbar, browser, rack, Wide mixer, playlist, piano roll) | Manual base + `img_shot/basics_interface.png` |
| 02-main-window-fl2026.jpg | FL 2026 (v26.1.4) window with a modal dialog over the dimmed workspace | Assets `frame-316129723.edaa27d.jpg` |
| 03-main-window-fl2026-browser-playlist-crop.png | FL 2026 toolbar, browser with plug-in thumbnails, picker, coloured track headers | Assets `frame-316129732.91f3a4d.png` |
| 04-main-window-fl2025-playlist-macos.jpg | FL 2025 on macOS, playlist | Assets `main-fl-studio-poster.b36e712.jpg` |
| 05-mixer-annotated-anatomy.png | the mixer, annotated (every strip control, docks, inspector) | Manual `img_shot/mixer_main.png` |
| 06-mixer-compact-layout-1to1.png | Compact mixer, true colour, ≈150% (main measurement source) | Manual `img_shot/mixer_intro.png` |
| 07-mixer-track-inspector-eq-pdc.png | Track inspector: slots, EQ, PDC and I/O menus | Manual `img_shot/mixer_trackproperties.png` |
| 08-mixer-fl2025-colorful-add-track.png | FL 2025 mixer: colourful tracks, + add button | CDM press image `https://cdm.link/app/uploads/2025/07/mixer.webp` (converted to PNG) |
| 09-mixer-fx-slot-opens-fruity-limiter.png | FX slot → plug-in window, wrapper title | Manual `img_shot/mixer_plugin.png` |
| 10-mixer-routing-send-icons.png | send and route iconography | Manual `img_shot/mixer_routingicons.png` |
| 11-mixer-fx-slot-bank.png | FX slot bank | Manual `img_shot/effects_pluginbank.png` |
| 12-channel-rack-annotated.png | channel rack, true colour ≈150% | Manual `img_shot/channelrack_main.png` |
| 13-channel-rack-to-plugin-wrapper-channel-settings.png | rack → instrument window with a channel-settings strip | Manual `img_shot/channelrack_plugin.png` |
| 14-playlist-annotated.png | playlist anatomy | Manual `img_shot/playlist_main.png` |
| 15-piano-roll-annotated.png | piano roll, true colour | Manual `img_shot/pianoroll_general.png` |
| 16-browser-tabs-and-views.png | browser tabs and views | Manual `img_shot/browser.png` |
| 17-toolbar-menu-title-hint-panel.png | menu bar, title and hint panel | Manual `img_shot/panel_main_menubar.png` |
| 18-extended-hint-panel.png | extended hint panel | Manual `img_shot/panel_Extended_Hintbar.png` |
| 19-settings-theme-hue-saturation-brightness.png | Theme settings: knobs, role colours, presets | Manual `img_shot/system_settings_themes.png` |
| 20-color-picker-circle-box-preset-safe-colors.png | colour picker, three modes, safe colours | Manual `img_shot/basics_interface_color.png` |
| 21-theme-level-meter-colour-editor.png | meter gradient editor | Manual `img_shot/settings_themes_meter.png` |
| 22-plugin-wrapper-header.png | wrapper header and channel strip | Manual `img_plug/plugin_wrapper.png` |
| 23-plugin-fruity-parametric-eq2.png | Parametric EQ 2 | Manual `img_plug/plugin_fx_FruityParametricEQ2.png` |
| 24-plugin-fruity-limiter.png | Fruity Limiter | Manual `img_plug/fruity_limiter.png` |
| 25-plugin-vintage-chorus.png | Vintage Chorus | Manual `img_plug/Vintage_Chorus.png` |
| 26-plugin-flex-edit-view.png | FLEX (pre-2026 UI) | Manual `img_plug/FLEX.png` |
| 27-plugin-flex-header-menu.png | FLEX header and menu | Manual `img_plug/FLEX_header.png` |
| 28-plugin-luxeverb-value-cells.png | LuxeVerb control cells | Manual `img_plug/Luxeverb.png` |
| 29-plugin-luxeverb-closeup-fl2026.png | LuxeVerb close-up with the preset header | Assets `frame-316129677.175f4a7.png` |
| 30-plugin-fruity-delay-3.png | Fruity Delay 3 | Manual `img_plug/Fruity_delay_3.png` |
| 31-plugin-frequency-splitter.png | Frequency Splitter | Manual `img_plug/Frequency_Splitter.png` |
| 32-plugin-maximus.png | Maximus | Manual `img_plug/Maximus.png` |
| 33-plugin-fruity-reeverb-2.png | Fruity Reeverb 2 | Manual `img_plug/reeverb2.png` |
| 34-plugin-kepler.png | Kepler | Manual `img_plug/Kepler.png` |
| 35-plugin-emphasis-fl2025.png | Emphasis (FL 2025) | CDM press image `https://cdm.link/app/uploads/2025/07/emphasis.webp` (converted to PNG) |
| 36-menu-style-fl2026.png | FL 2026 menu with section bands | Assets `frame-316129678.12fd441.png` |
| 37-theme-gold-channel-rack-closeup.png | a user theme (gold) on the channel rack | Assets `asset-theme-3.70ae72b.png` |
| 38-channel-rack-loop-starter-closeup.png | FL 2025 channel rack and Loop Starter, 2× | Assets `group-316127757.76e476d.png` |
| 39-scroll-zoom-bars.png | scroll and zoom bars | Manual `img_shot/basics_interface_scrollbars.png` |
| 40-knob-and-slider.png | basic knob and slider | Manual `img_shot/basics_interface_knobnslider.png` |
| 41-windowed-application-multi-monitor.png | windowed FL across monitors | Manual `img_glob/windowed_application.png` |
| 42-playlist-track-headers-context-menu.png | track headers and the FL 20/21 menu style | Manual `img_shot/playlist_trackheaders.png` |

**Download notes.**

- No download failed.
- The Image-Line server was slow (20–60 s per image), so I ran the downloads as background jobs.
- An early version of my download script used a relative `cd` and briefly wrote 8 PNGs into the
  repository root. I moved them to the scratchpad straight away. `git status` confirms the root is
  clean, and nothing was committed.
- Screenshots 08 and 35 are Image-Line press images as republished by CDM, because the
  image-line.com pages I fetched had no mixer or Emphasis capture at that size.
- I found **no official screenshot of the FL 2026 FLEX UI**.

## Sources (text)

- Manual pages:
  - User Interface: `basics_interface.htm`
  - Mixer: `mixer.htm`
  - Mixer menu: `mixer_mixermenu.htm`
  - Mixer track properties: `mixer_trackprops.htm`
  - Toolbar and hint bar: `toolbar_panels.htm`
  - View menu: `menu_view.htm`
  - Theme settings: `envsettings_themes.htm`
  - General settings: `envsettings_general.htm`
  - Browser: `browser.htm`
  - Playlist: `playlist.htm`
  - Piano roll: `pianoroll.htm`
  - Channel rack: `channelrack.htm`
  - Plug-in wrapper: `plugins/wrapper.htm`
  - Parametric EQ 2: `plugins/Fruity Parametric EQ 2.htm`
  - Vintage Chorus: `plugins/Vintage Chorus.htm`

  All of them are under the manual base URL.
- [What's new in FL Studio 2026](https://www.image-line.com/fl-studio/release/2026)
- [FL Studio 2025 released](https://www.image-line.com/fl-studio-news/fl-studio-2025-whats-new-2)
- [FL Studio 2026 release notes](https://forum.image-line.com/viewtopic.php?t=341294)
- [CDM on FL Studio 2025](https://cdm.link/fl-studio-2025/)
- UI font: [Image-Line forum, "FL11 Browser Font"](https://forum.image-line.com/viewtopic.php?t=109057),
  which gives PT Sans Narrow in `Artwork\Fonts`.
- Mixer layout names and track counts, third-party:
  - [Image-Line forum, "Custom mixer layouts"](https://forum.image-line.com/viewtopic.php?t=322842),
    which also notes the inspector cannot be widened;
  - YouTube, "8 FL Studio MIXER View Modes".
