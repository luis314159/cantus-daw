# Logic Pro 11 and Studio One 7 / Fender Studio Pro 8: UI research for Cantus

*September 2026. Research only. The screenshots are in `research/ui/screenshots/logic-studio-one/`
(git-ignored); the index with source URLs is at the end.*

**Scope.** Two polished DAWs built around their mixers:

- **Logic Pro 11.** Mac, plus the iPad version where it adds something.
- **Studio One.** PreSonus's Studio One Pro 7 is now **Fender Studio Pro 8**. The old
  `s1manual.presonus.com` links redirect to `fenderstudiopromanual.fender.com`. The manual still
  uses Studio One 6/7 screenshots (files named `SO60…`, `sO-6.6…`) for the Console, next to new
  `…_v8` ones. The Console's layout is the same in 7 and 8. Version 8 added the Channel Overview
  and "a tiny bit of shading" (Sound On Sound's words). Below, "Studio One" means both, and "v8"
  marks what is new.

**Units.** "pt" means logical points, the same as egui points.

- **Apple user-guide images** are Retina crops at 2 px per pt. I checked this against the Logic
  strip width.
- **Apple marketing images** of a MacBook screen run at about 1.08 px per pt: the 1,637 px screen
  shows the 1,512-pt display.
- **Studio One manual images** are 2x macOS captures, except `fsp8-session-page.png`, which is a
  1x Windows capture.

Colours were sampled from the PNGs with PIL. JPEG colours are ±3 per channel.

---

## 0. The five findings that matter most for Cantus

1. **Neither DAW ever prints a loose label.** Every word sits in one of three places:
   - a **legend column** printed once for the whole desk (Logic's mixer);
   - the left cell of a **label | value row** on a fixed grid (Logic's inspector, and Studio
     One's inspector, micro views and EQ band cards);
   - **above or below the one control it names**, with the value directly under it (Studio One's
     compact views, Logic's instruments).

   Cantus's mixer repeats small captions such as "SENDS", "Mono" and "Ø" in every strip, and its
   inspector stacks `label`, slider and readout vertically. That is what reads as "words thrown
   down".
2. **The panel for a selected channel is a signal-flow sheet, not a form.**
   - **Logic's inspector** shows the selected track's channel strip beside its **destination** (the
     output, or the aux that a send feeds), under collapsible "Region:" and "Track:" sheets.
   - **Studio One's Channel Editor** puts the strip on the left, then **Inserts | Sends | Cue mix**
     as equal columns of 20-pt rows. Each insert row can open into a **micro view**: rows of
     parameter name and value with a 3-pt fill bar underneath, or a small curve.
   - **v8's Channel Overview** lays the same content out horizontally as plug-in **cards**, each
     with a knob grid.

   This is the model for the Cantus side panel.
3. **Rows are shared across the whole desk.**
   - In Logic, every strip reserves the same rows: the insert area is as tall as the longest chain,
     and empty slots are drawn as darker inset wells, so the eye reads a table even when it is
     empty.
   - Logic scrolls the whole desk vertically when the window is short (it has a real vertical
     scroll bar) and offers *Long Faders*.
   - Studio One splits the strip into **resizable zones**: the Inserts rack, the Sends rack and the
     fader block. You drag the dividers between them, and each rack scrolls on its own.
4. **Colour means something; decoration is almost absent.**
   - Logic uses **neutral greys** (#2e2e2e → #575757 → #737373). Blue fills mark audio plug-ins,
     green marks instruments, yellow and orange mark the automation modes, and the track colour
     appears only on the name bar.
   - Studio One uses **blue-tinted greys** (#393f44, #2e3033) and one cyan-blue accent (#37a6fa)
     for everything that is "on". Optionally it tints the whole strip, the Channel Editor and the
     plug-in header with the channel colour.
5. **Resizing is everywhere and discoverable, and the fast paths are keyboard ones.**
   - Logic's split bars light up blue (#83bffd, 2 pt) with a resize cursor.
   - Studio One v8 added a divider handle that appears on hover.
   - The mixer, Channel Editor and plug-in windows can all be detached. Plug-in windows scale from
     50 % to 200 %.
   - Selection follows the arrow keys; M and S act on the selection; there is a
     find-channel-by-name command, *Scroll To* by channel type, a channel list with a filter, and
     screensets.

The ranked list of what Cantus should take is in section 7.

---

## 1. Visual language

### 1.1 Logic Pro: a neutral grey ladder

| Role | Hex | Source image |
|---|---|---|
| Arrange canvas | `#2e2e2e` (bar lines `#3f3f3f`/`#454545`) | `logic-main-window-overview.png` |
| Local menu / toolbar row | `#474747` | same |
| Mixer background, legend column | `#4c4c4c`–`#4f4f4f` | `apple-logic-mixing-hero.jpg` |
| Channel strip / track header | `#575757` | `logic-channel-strip-anatomy.png` |
| **Selected** strip / track | `#737373` (a whole step lighter) | `logic-mixer-window.png` |
| Empty slot "well" (inset) | `#525252` (darker than the strip) | anatomy |
| Strip separator | 1 px `#464646`; legend divider 1 px `#161616` | mixer window |
| Button fill (Setting, Output, M/S) | `#707070`, top edge `#676767` | anatomy |
| Numeric fields (dB, peak) | `#333333`–`#393839`, white digits | hero, anatomy |
| Control bar (top) | light `#a5a5a5`, with a dark LCD `#24262d` in the centre | main window |
| Focus / resize highlight | `#82befd`–`#83bffd` (about 2 pt line) | `logic-resize-bar-loop-browser.png` |
| Segmented control, selected | `#2772bf` with white text; unselected text `#c6c6c7` on `#343637` | resize-bar image |

**Colour carries meaning, not decoration.**

- **Audio-effect slot:** blue fill `#266fb7`–`#2163b9` with white text.
- **Instrument slot:** green `#409852`.
- **Send slot:** blue, with a small knob and a green value ring `#4cd865`.
- **Automation button:** grey with green text for *Read* (`#66ff7f`); ochre fill for *Touch*
  (`#b68d2b`); orange for *Latch* (`#bd6617`).
- **Group slot:** near-black `#2f3217` with tan/yellow text `#b4a68b` ("1: Vocals").
- **Peak field:** green digits `#5da36d`, turning red above 0 dB. Orange means over 0 dB but not
  clipping.
- **Track colour** appears only on the **name bar** at the foot of the strip, full width, with
  white text, and on the track icon. The measured bars are saturated mid-tones: audio blue
  `#366faa`, instrument green `#2ea940`/`#1aa331`, aux yellow `#a3a31a`, output magenta
  `#a31a87`, master purple `#6639ac`.

**Depth is flat.**

- Pills have a 1-pt lighter top edge and a corner radius of about 3 pt.
- The only skeuomorphic elements are the **fader cap**, a brushed-metal horizontal bar, and the
  knobs.
- Wells are *darker* than the strip, so the structure recedes and the filled pills pop.
- The Smart Controls pane and some vintage plug-ins go fully skeuomorphic (brushed metal, black
  knobs). That is *not* the house style and should not be copied (section 7).

**Density.** A wide strip holds about 18 rows of controls in about 70 pt. Empty space appears only
as the well of unused slots, which keeps the rows aligned across the desk.

### 1.2 Studio One / Fender Studio Pro: blue-grey, one accent

| Role | Hex | Source image |
|---|---|---|
| Top chrome / menu | `#333941`; toolbar `#2e343b`; panel headers `#25292f` | `fsp8-session-page.png` |
| Track list rows | `#393f44`, 1 px separator `#1a1e21` | same |
| Arrange canvas | `#202528`, bar lines `#43494d` | same |
| Browser panel / cards | `#35393f` panel; cards `#32383c` with 1 px `#292c31` border | same |
| Inspector panel | `#373a3d`; header `#272a2d`; track-name row `#1b1c1e` | `fsp8-track-inspector.png` |
| Drop-down field | `#2b2d2f` with 1 px `#1d1e1f` border | same |
| Rack column (Channel Editor) | `#181a1e`; insert row `#51565b` with 1 px `#3b3f43` rule | `fsp8-channel-editor.png` |
| Micro-view row | `#2e3033`; value bar `#579cdc` on track `#242627` | `s1-console-small.png` |
| Plug-in card (v8 compact view) | `#1e2329`→`#24272d`, 1 px `#30353c` edge, top highlight `#3e434d` | `fsp8-channel-overview-compact-views.png` |
| Graph background | `#080a0b`–`#111217`, grid `#13222d`/`#1e3647` (blue) | compact views, `fsp8-fat-channel-xt.png` |
| Text | primary `#bdbfc1`–`#c6c9cc`; names `#d7d8d9`; dim labels `#999999` | several |
| **Accent (on, active, value arcs)** | `#37a6fa`–`#43abfc`; power icons `#77b7fc`; send bars `#469cdf` | several |
| Section heading in Fat Channel | cyan `#87d2f3` | Fat Channel XT |

**Colour meaning.**

- Blue marks **anything that is on or active**: power buttons, knob arcs, pan fill, meters.
- The automation chip is green (`Read`).
- Solo is cyan and record is red (`fsp8-channel-overview-controls.png`).
- **Channel colour** always tints the name label and the channel tabs.
- With *Colorize Channel Strips* on, it tints the whole strip, the Channel Editor window (rose
  `#965f5e` behind `#805050` channel column in `fsp8-channel-editor.png`) and the Channel
  Overview's left column.
- *Colorize Plug-in Header* tints the plug-in window title bar with the channel colour.
- Macro knobs each get their own colour (green, violet, teal, pink), with the label in the same
  colour (`fsp8-channel-editor-macros.png`).

**Depth.** Studio One 7 was "flat and flawless". v8 sinks grouped sections into the background
with very subtle shading and makes faders, knobs and buttons more three-dimensional (SOS review).
Cards have a radius of about 4–6 pt. Knobs are dark (`#323232`) with a coloured value arc.

**Appearance setting.** The user sets hue, saturation, brightness and contrast separately for the
*Background* and the *Arrangement*, and can store presets. Plug-ins have Dark / Light / "Colored"
modes.

**Iconography.** Both use thin monoline glyphs: power ⏻, ▼ menu arrows, a "+" to add, a
pack-of-cards icon for the Channel Editor, a wrench for options, and the Studio One waveform "ʌʌʌ"
icon for audio tracks.

---

## 2. Typography (what matters most to the user)

### 2.1 Fonts and sizes

| | Logic Pro | Studio One |
|---|---|---|
| Family | **SF Pro** (the system font). The LCD uses its own large digits. | The system UI face: renders as **Segoe UI** on Windows (`fsp8-session-page.png`) and Helvetica-like on macOS |
| Mixer labels, slot text | **≈11 pt regular**. Cap height measured at 8 pt (16 px at 2x) in a **17-pt pill**, so text fills about 0.65 of the pill | Rack rows **≈10 pt** (cap 7 pt) in **20-pt rows**; row pitch 21 pt |
| Numeric readouts | dB and peak fields ≈12 pt (cap 8.5 pt), white | Fader value ≈12 pt above the fader ("-5.5", "0dB") |
| Meter / fader scale | **≈7 pt** (cap 5 pt), grey, tabular | ≈7–8 pt, grey, blue tick marks |
| Name bar | ≈11 pt; one line (25 pt bar), or **2 lines wrapped** in Logic 11 (41-pt bar) | ≈11–12 pt on the colour bar |
| Inspector rows | Label and value the same size, about 1.75× smaller than the row height. There is a "Large inspectors" setting | ≈11.5 pt in 27-pt rows (`fsp8-track-inspector.png`) |
| Section header | Sheet header ≈1.25× the row text: "Region:" in grey `#b5b5b5` plus the name in white `#ffffff`, on a darker band (`#474747`/`#373737`) about 1.9× a row tall, with a disclosure chevron | Column header "Inserts / Sends / Cue mix" ≈12 pt on `#181a1e`. Fat Channel sections ≈15 pt in accent cyan with a power icon |
| Plug-in body labels | Title Case ≈11–12 pt, grey-blue `#9ba5ae`; section heads **UPPERCASE** ("PITCH", "FILTER", "AMP") | Title Case ≈11.5 pt: label `#999999`, value `#cccccc` (Fat Channel) |
| Weights | Regular for nearly everything; **semibold** only for window titles and sheet names | Regular; bold for the channel name in the editor ("Pop Lead Vox") |

### 2.2 How labels attach to controls: the patterns that avoid "loose words"

1. **Logic mixer legend.** A column about 50 pt wide at the left of the desk holds the row names,
   right-aligned ("Input", "Audio FX", "Sends", "Output", "Group", "Automation", "Pan", "dB") at
   the y of each row.
   - The strips carry only values.
   - A 1-px near-black rule separates the legend from the strips.
   - *View ▸ Hide Legend* removes it.
   - (`apple-logic-mixing-hero.jpg`, `logic-mixer-window.png`.)
2. **Logic inspector form.** Labels are **right-aligned to a centre gutter and end with a colon**
   ("Mute:", "Transpose:", "Velocity Offset:"). Values start about 4 pt right of the gutter.
   - Steppers (⌃⌄) sit at the far right edge.
   - Rows are about 22 pt tall with an almost invisible zebra (`#525252` / `#505051`).
   - Colours: label `#c8c8c8`, value `#ffffff`, disabled label `#898989`.
   - An empty value shows "– –".
   - Over-long labels end-truncate ("Default Region…", "MIDI Out Chan…").
   - The Settings window uses the same form: right-aligned "Display Time As:" and a pop-up
     aligned left (`logic-settings-view-general.png`).
3. **Studio One row = label left, value right, bar below** (micro views, `fsp8-micro-views.png`,
   `s1-console-small.png`). "Low Boost … 8.45", "Feedback … 40.00", "Mode … FFT Curve".
   - One 21-pt row per parameter.
   - The **value bar is a 3-pt strip along the bottom of the row**, blue on dark.
   - The row itself is the slider: drag anywhere on it.
   - Fat Channel's EQ band cards use the same table: "LF | Freq. 109.6 Hz / Gain 3.35 dB / Q
     0.58", with label and value columns aligned and the values right-aligned.
4. **Knob with its words beside it (Fat Channel).** Knob about 40 pt, then two stacked lines to
   the right: "Threshold" (grey) over "-49.41 dB" (white). This gives a compact block of 3
   columns × 3 rows, with every block the same width.
5. **Knob with its words above and below (Studio One compact views; Logic instruments and
   ChromaVerb).**
   - Label above in Title Case, secondary colour. Knob about 32–40 pt. Value below in primary
     colour (Studio One) or **in the plug-in's accent colour** (Logic: orange in ChromaVerb, yellow
     in Quick Sampler).
   - **The unit is set smaller and dimmer than the number** ("10 %", "1.80 s", "46 ms", "+29.3 %").
   - Toggles and menus are small dark chips with accent text ("Mono", "ON", "OFF", "2 ⌃⌄").
6. **Section headers structure the grid.**
   - Logic instruments centre UPPERCASE section titles over their block, or put a power toggle
     plus the title at the left and a mode menu at the right ("⏻ FILTER … LP 12dB Lush ⌃⌄").
   - Blocks are separated by 1-px darker rules.
   - Studio One puts a **power + name + ▼ + "+"** header above every rack column.
7. **Truncation keeps both ends.**
   - Logic middle-truncates names ("Conc…iano", "Ambi…ence", "Larg…Plate"). Logic 11 and iPad
     **wrap names onto two centred lines** in the name bar ("Vocal / Textures") rather than cut
     them.
   - Built-in plug-ins have **short names** made for slots ("Cnsl EQ", "Chan EQ", "St-Delay",
     "Multipr").
   - Studio One middle-truncates with ".." ("Crus..Perc", "Hou..Bass") in the channel list and
     tabs.
   - Studio One's narrow console, however, **clips glyphs** ("Snare :", "Oveead") and some tabs
     clip at the left edge ("Gtr Rhy 1 L DI"). Do not copy that.
8. **Numbers.**
   - One decimal for fader dB ("-12.5", "0.0").
   - Two decimals in plug-ins ("-14.40", "4.56").
   - Signed values carry an explicit "+" ("+29.3 %", "+12.0 dB").
   - Minus infinity is shown as "-∞".
   - Readouts sit in recessed fields (Logic) or plain above the fader (Studio One).
   - Pan reads "<C>", "<L>", "L60 66% R72" (Studio One) or appears only on the knob (Logic).
9. **Case discipline.** UPPERCASE only for instrument section heads and tab bars ("MAIN / DETAILS",
   "CLASSIC / ONE SHOT / SLICE"). Title Case for control labels. Sentence case for settings rows
   (Studio One inspector: "Layers follow events", "Play overlaps"). Studio One mixes cases in
   places ("Follow Global Transpose"); Logic is stricter.

---

## 3. Layout and panels

### 3.1 Logic Pro

**Single main window.** The window holds the control bar (transport and LCD) and a local toolbar,
with areas that are **shown or hidden by buttons or single keys**:

- Library and **Inspector (I)** on the left.
- Tracks area in the centre.
- The bottom area switches between **Mixer (X)**, Smart Controls, Editors and List Editors.
- Note Pad, Loop Browser and Browsers on the right.

*Window ▸ Open Mixer* (**⌘2**) opens the Mixer as its own window, and you can open several
mixers, each with its own view settings. **⌘1** opens another main window.

**Resizing.**

- Drag the split line between the Tracks area and the bottom area.
- The pointer becomes a resize cursor and the 2-pt line lights up **blue #83bffd**.
- "**The vertical height of the Mixer is independent of the height you set for the editors**":
  each bottom area remembers its own height.
- Windows resize from any corner.
- Resize bars exist wherever an element can be sized.

**Focus.** The area with key focus gets a thin blue frame (visible on the left and bottom edges of
`logic-mixer-window.png`). Key commands act on the focused area, so arrow keys select strips when
the Mixer has focus.

**Screensets.** Named window layouts, recalled with the number keys and lockable. Windows can be
*linked* (content follows selection) and put in *Catch* mode (scroll to follow the playhead).

**The inspector has a fixed width.** You can only enlarge its text ("Large inspectors", "Large
local window menus"). It is split into collapsible sheets: *Region:* and *Track:*, each with a
disclosure chevron and a darker header band. Below them sit the **two inspector channel strips**.

**iPad.**

- The Mixer, when enlarged, shows the selected track above it.
- A **resize handle** sets how much ruler and track shows.
- A **meter bridge** under the Mixer's menu bar can be swiped to scroll through strips (a
  mini-map).
- The **Plug-ins area** is a separate bottom panel (section 4.3).

### 3.2 Studio One / Fender Studio Pro

**Session page** (`fsp8-session-page.png`, Windows 1x):

- Menu bar (`#333941`) and toolbar with tool groups.
- **Inspector** on the left (**F4**).
- Track list and arrangement.
- **Browser** docked on the right: large cards 60 px tall with 10 px gaps, each with an icon, a
  title and a subtitle ("Effects / Show installed Effects").
- **A bottom panel slot** that switches between the **Console (F3)**, the Editor and, in v8, the
  **Channel Overview**.
- A **transport bar at the bottom** in v8. The Inspector and Track List buttons moved to its left;
  Browse, Mix and Edit are icon buttons at its right.
- v8 adds an "inch-high" **Arrangement Overview** band at the top for navigation.

**Detaching and resizing.**

- The Console has a **Detach** button at the top of its navigation column and becomes a sizable,
  maximisable window.
- The Channel Editor floats, is **pinnable**, and follows the selection.
- The Channel Overview detaches but can then be resized **horizontally only**. Docked, it is
  fixed; SOS criticised it as "static, you can't resize it in any direction".
- In the Console, the dividers between Inserts and Sends, and between the racks and the fader
  block, are **draggable**. v8 adds a hover handle to make them easier to find.

**The Console navigation column** is a 60-pt column at the far left. From top to bottom:

- Close, Detach, I/O (with ▼ to add Bus / FX / VCA / Aux), Options (wrench), groups/VCA.
- Then **Small/Large (⧗)** and **Narrow/Normal (⧓)**.
- Then panel toggles: Inputs, Outputs, Instruments, External, Scenes, Groups, Channel List
  (`s1-console-nav-column-wrench.png`).

**Plug-in windows.** By default **one** reused window, with **tabs for every insert on the
channel** ("1 - Compressor", "2 - Pro EQ ▾"…). A **Pin** keeps a window open, so opening another
plug-in makes a new window.

---

## 4. The mixer

### 4.1 Logic Pro mixer strip anatomy

Measured from `logic-channel-strip-anatomy.png` at 2 px per pt. The wide strip is **≈71 pt**;
`apple-logic-mixing-hero.jpg` gives 69 pt. Slots are inset **5 pt** each side, so pills are about
60 pt wide.

| # | Row (top → bottom) | Height (pt) | Notes |
|---|---|---|---|
| 1 | Setting button | 17 | Channel-strip preset menu |
| 2 | Gain-reduction meter | 10 | Shows the first compressor or limiter |
| 3 | **EQ thumbnail** | 31 | Mini curve of the Channel EQ. Click inserts or opens the EQ; ⌥/⇧/⌘ variants |
| 4 | MIDI FX slot(s) | 16 each | Instrument strips |
| 5 | Input / Instrument slot | 17 | Format button (◎ mono/stereo) + input; instrument slot is green |
| 6 | **Audio FX slots** | 17 each, ~1-pt gaps | Blue pills inside a darker well. **The well grows to the longest chain in the desk, so every strip keeps the same row grid** |
| 7 | Sends | 17 each | Blue pill + 18-pt level knob, then an empty well slot |
| 8 | Output | 17 | Grey pill ("St Out", "Surround") |
| 9 | Group | 18 | Dark pill, yellow text |
| 10 | Automation mode | 17 | Read / Touch / Latch colours |
| 11 | Track icon | 23 | |
| 12 | Pan / balance knob | ~34 incl. ring | Green arc from centre; stereo pans differ |
| 13 | dB field + peak field | 19 | Two recessed fields side by side |
| 14 | **Fader + meter** | ~150 by default | Tick ruler left of the fader; meter with its own scale (0 … 60). **View ▸ Long Faders** (⌃⇧L) lengthens it; per inspector strip too |
| 15 | R / I (record, input monitor), Bnc on outputs | 15 | |
| 16 | M / S | 17 | |
| 17 | **Name bar** | 25 (1 line) / 41 (2 lines) | Track colour, white text |

**Configuring the strip.** *View ▸ Channel Strip Components* toggles every row: Audio Device
Controls, Setting, Gain Reduction, EQ Thumbnail, MIDI FX, Input, Audio FX, Sends, Output, Group,
Automation, Track Icon, Pan, VCA, Track Colour, Track Name, Track Number, Notes. The configuration
can be stored as a user default (⌥X).

**Widths.** Wide and Narrow buttons in the Mixer menu bar. I did not measure narrow.

**Views and filters.**

- **Single / Tracks / All** (⇧X cycles).
- Type filter buttons: Audio, Inst, Aux, Bus, Input, Output, Master/VCA, MIDI. ⌥-click solos one
  type.
- *View ▸* **Signal Flow Channel Strips** (show the aux or output a selected strip feeds),
  *Channels with Sends only*, *Folder Tracks*, *Follow Track Stacks*.

**Scrolling.**

- Horizontal scroll bar.
- **A vertical scroll bar when the desk is taller than the window.** The whole desk, legend
  included, scrolls. `logic-mixer-window.png` shows the desk scrolled so the Setting row is off
  the top.
- *View ▸ Autoscroll to Selection*.
- *View ▸ Scroll To ▸ \<type\>* jumps to the first aux, output, and so on.

**Selection.**

- ←/→ select strips; ⇧←/→ extend; ⌘-click for non-contiguous.
- Click an empty part of a strip to select it; drag across strips.
- ⇧A, ⇧S, ⇧F, ⇧O, ⇧E select strips by type; ⇧C selects same-coloured strips; ⇧M selects muted
  strips.
- The selected strip is **a whole grey step lighter** (`#737373` against `#575757`).

**"Sends on Faders".** The faders temporarily control one send bus. ⌃⌥⌘S cycles through the sends.

**Plug-in slot hover** (`logic-plugin-slot-bypass-hover.png`). Hovering a filled slot splits it
into three zones:

- **power (bypass)** on the left;
- **open window** in the middle (a slider icon);
- **⌃⌄ menu** on the right (replace, remove).

Drag slots to reorder them, ⌥-drag to copy, and drag to another strip to move.

### 4.2 Logic: showing the selected channel (the inspector)

See `logic-inspector-region-track-channelstrips.png`.

**Region and track sheets.** At the top are the **Region:** and **Track:** sheets. Each header is
"Region:" in grey plus the region's name in white, with a disclosure chevron, on a darker band.
Rows are a right-aligned form with a colon gutter. "More" discloses the rarer parameters.

**Two full channel strips side by side** (each ≈half the inspector) show the signal path:

- The **left strip is the selected track**.
- The **right strip is where it goes**: its output by default. ⇧-click a **send** slot to show
  that aux, ⇧-click the VCA slot to show the VCA master.
- For a track-stack subtrack, the right strip shows the stack's main track.
- Each can take Long Faders independently.

This beats a "form of properties" because it re-uses the mixer's visual vocabulary. The user
already knows how to read a strip, and seeing source and destination together answers "where does
this go?".

Smart Controls (`logic-smart-controls-pane.png`) is a second "selected-channel" surface: 8 macro
knobs mapped to the patch, with its own small inspector at the left. It is useful as a concept,
but its brushed-metal look is not something to copy.

### 4.3 Logic for iPad: the "selected channel" as tiles

**The Plug-ins area** (`logic-ipad-plugins-area.png`) is a bottom panel for the **selected
track**.

- **Header.** A small caption "Track" (grey) over the name "Dreamy Piano" (white), top-left. A
  centred segmented control **Track | Sends | Output**. At the right: *Automation* caption over
  "Read ⌃⌄" in green, an Edit (pencil) button and a "…" button.
- **Plug-in tiles.** Rounded cards (about 8 pt radius) laid **left to right in signal order**.
  Each has a header with a power button and the plug-in name, and **3–4 key parameters**: label
  above (grey), a small knob or value below, with the arc in accent blue. Instruments show their
  display, such as Alchemy's waveform. Bypassed tiles go grey. The row scrolls horizontally.

**The mixer** (`logic-ipad-mixer.png`):

- The selected strip is pinned in a column on the left; outputs are pinned on the right.
- A **meter bridge / number strip** above the strips acts as a scrubbable mini-map.
- *Setup* and *Mix* modes: edit the chain, or play with it.
- Plug-in names **wrap to two centred lines** ("Vintage / Electric Piano") in taller pills.
- Bypassed plug-ins show as grey text on dark.
- **Touch-and-hold a slot** opens a **mini-view popover** with 4 knobs
  (`logic-ipad-strip-mini-view.png`).

### 4.4 Studio One Console

**Anatomy of a Large, Normal strip** (`s1-console-large-main.png`, `s1-console-nav-column-wrench.png`),
top to bottom:

1. *(optional)* Input controls: gain knob "0.0 dB" and polarity Ø buttons.
2. **Inserts rack.**
   - Header: ⏻ Inserts ▼ +.
   - Rows: ⏻ name ▼.
   - Any insert can **expand in place into a micro view**: parameter rows with fill bars, or a
     transfer curve for compressors, a spectrum or EQ curve, as in `fsp8-micro-views.png`.
3. **A draggable divider.**
4. **Sends rack.**
   - Header: ⏻ Sends ▼ +.
   - Each send is a row (⏻ name ▼) with a **level bar** under it, plus a pre/post icon.
5. **A draggable divider.**
6. I/O: input and output selectors as full-width dark pills ("None", "Drums").
7. **Horizontal bar panner** (blue fill from centre) and its readout "<C>" / "L50".
8. M and S buttons with a record ● and monitor ◐ row under them, as a 2×2 block.
9. Fader value "-12.5" and the meter-mode icon.
10. **Meter at left, fader at right**, with a shared scale (10 / 6 / 0 / -6 … -72) and blue
    ticks.
11. Channel number, type icon, Channel Editor button (pack of cards), expand button.
12. Group or bus assignment chip, and automation mode ("Auto: Off" or a green "Read" chip).
13. **Name bar** in the channel colour.

**Widths.**

- Normal strip **≈86–88 pt** (173 px at 2x).
- **Narrow ≈34 pt** (68 px at 2x). In Narrow the Large console swaps the racks for **tall
  meters**, and the Small console overlays the fader handle on the meter, with M/S above.
- In Narrow, **double-click empty space in a strip to expand just that strip**.

**Small / Large.**

- *Small* hides the Insert and Send racks. An **Expand** button per strip opens its racks to the
  right, 105 pt wide in `s1-console-small.png`.
- ⇧F3 toggles Small/Large.
- The option **Auto-expand Selected Channel** makes the expanded strip follow the selection: the
  previous one collapses, and ⌥-click keeps both.

**Components.** The wrench menu toggles Audio device controls, Input controls, **Sends/Cue mix**,
I/O connections, VCA connections, Group assignment, Channel Notes and Channel Icons.

**Ordering.** *Keep FX / Bus / VCA channels to the right*. **Main Out is locked at the far right**
and never scrolls away.

**Navigation.**

- **Channel List** panel: number, show/hide dot, type icon, colour swatch, name, group column.
  Drag through the dots to hide many channels at once. A **Filter** field takes comma-separated
  terms ("bas, guit"). Type toggles sit at the bottom (`s1-console-channel-list.png`).
- **Scenes** store which channels are visible.
- ⌃⌥C opens a dialog to **find a channel by name or number**.
- ←/→ move across channels. While a pan pop-up is open, it follows the selection.
- **M / S keys** mute and solo the selected channels.
- ⌘C / ⌘V copy and paste channel settings (level, pan, inserts, sends) to one or many channels.

**Drag targets.**

- Drag an insert onto another channel to copy it (⌥ to move).
- Drag the **Inserts header** to copy the whole chain.
- Drag an effect onto the **Sends** rack to create an FX channel with that effect and a send to
  it.
- Dragging to the edge of the console **auto-scrolls** it.
- Removed inserts go to a **Trash Bin** you can restore from.

### 4.5 Studio One: showing the selected channel

**Channel Editor** (`fsp8-channel-editor.png`). The window is about 820 × 490 pt and tinted with
the channel colour.

- **Title bar.** ▼ + "Pop Lead Vox" … pin ✕.
- **Tab row.** View icons (Channel, Macros, Routing), then **one tab per insert** ("1 - Pro EQ",
  "2 - Compressor" …).
- **Left: the channel column, 155 pt.**
  - Bold centred name.
  - A 2×2 block: input gain "-7.0 dB" and polarity.
  - Input and output pills ("Input L+R", "Pop Leads").
  - A pan box: mode "Dual", a graphic, then "L60 66% R72".
  - A notes box and an icon box.
- **Then a fader column, 75 pt.** M/S, rec/monitor, value "-5.5", meter and fader, automation
  "None / Read", number and type icon.
- **Then three equal columns (about 175 pt each):** **Inserts**, **Sends**, **Cue mix**.
  - Each is a `#181a1e` panel whose header is ⏻ title ▼ +.
  - Rows are 20 pt `#51565b` with a 1-pt rule.
  - Inserts can show **micro-graphs inline** (compressor curve, EQ curve).
  - Sends show a level bar and a pre/post toggle.
- Gutters between columns are about 7 pt, and the window's tint shows through them.

**Channel Overview (v8)** (`fsp8-channel-overview.png`, `…-compact-views.png`,
`…-controls.png`). The same channel is laid out horizontally across the whole bottom panel.

- **Left block.** The same channel controls in about 150 pt, then a narrow fader column.
- **Insert chain bar.** ⏻ Inserts ▼ + at the left. In the centre, **pills for each insert in
  order** ("Compressor ▾ | Pro EQ | Beat Delay | Flanger"); drag them to reorder.
- **Plug-in cards, one per insert, left to right, in a horizontal scroll.**
  - Header: ⏻, automation icon, name, a **preset pill** (📄 "Drum Set*"), and an expand button.
  - Body: a **2 × 2 knob grid** of the key parameters, with the label above and the value below,
    plus the plug-in's display (transfer curve, EQ graph) as a live, editable control.
  - Third-party plug-ins show their first 8 parameters; *Setup Edit Parameters* lets you choose
    them.
  - A "+" card at the end adds an insert.
- **Channel tabs across the bottom.** Number, type icon and name chip in the channel colour,
  with options for icons, meters and I/O (`fsp8-channel-overview-tabs.png`).

**Inspector** (`fsp8-track-inspector.png`), about 204 pt wide.

- **Header row.** A 6-pt **colour swatch**, the number "17" and the name "Precession Bass" on
  `#1b1c1e`.
- **Rows at a 27-pt pitch.** Label left-aligned at a 9-pt inset; control in the right half, as
  93-pt drop-down fields or checkboxes aligned to the right edge.
- **Below the rows**, after a resize grip (a centred 20-pt bar), comes **the channel strip with
  its Inserts and Sends racks**, tinted with the track colour.

### 4.6 Side-by-side: what makes their "selected channel" panels readable

| | Logic inspector | Studio One Channel Editor / Overview | Cantus today (`views/inspector.rs`) |
|---|---|---|---|
| Structure | Collapsible sheets; source strip + destination strip | Strip column + equal columns (Inserts / Sends / Cue) or cards | Stacked `heading` + `label` + slider + readout |
| Label placement | Right-aligned colon gutter; legend-style strips | Row: label left, value right, bar below | Label above or beside the slider, varies per control |
| Section headers | Dark band, chevron, "Type: Name" | Dark band: ⏻ Title ▼ + | Gold text heading and UPPERCASE captions |
| Density | 22-pt rows, zebra | 20–21-pt rows, 1-pt rules | Uneven vertical rhythm |
| Signal flow | Visible (source → destination) | Visible (insert order, sends, routing view) | Not visible as a chain |
| Resize | Fixed width | Editor floats; Overview fixed (criticised) | — |

---

## 5. Plug-in and instrument windows

### 5.1 Logic Pro

**Frame.** A standard macOS window.

- **Title** = "Track: Setting" ("Anastacia: Set Adrift", "Audio 1"), centred, semibold.
- At the right of the title bar, a **header show/hide** button: the header can be hidden for a
  compact frame (`logic-plugin-window-header.png`).

**Header**, common to every plug-in, including third-party ones (`logic-plugin-header-compare.png`):

- A large round **power / bypass** button (blue when on).
- A **Setting** pop-up (preset) spanning the width, with **◀ ▶** to step presets.
- **Compare** (blue when it differs from the saved setting).
- Copy / Paste and Undo / Redo.
- A **View** pop-up: *Editor* or *Controls*, **50 %–200 %** in 25 % steps, "100 %"
  (`logic-plugin-window-size-menu.png`).
- **Link**. Off; *Single* (purple: one window follows every plug-in you open); *Multi* (yellow:
  one window per slot row, following the selected strip).
- A **Side Chain** pop-up with a search field.

**Resizing.** Drag any corner, or pick a size. Scaling is per plug-in.

**Footer.** Every Logic plug-in ends in a **dark footer bar** (`#262626`) with the **plug-in name
centred** (≈15 pt, light weight) and, at the left, a **disclosure arrow** that reveals "extended
parameters" (`apple-logic-chromaverb.jpg`, `apple-logic-quick-sampler.jpg`).

**Controls view.** For any plug-in, a generic list of horizontal sliders with numeric fields to
their left. This is the fallback for any UI.

**Interaction conventions**, the same in every plug-in:

- Vertical drag on knobs; ⌘-click in a knob's ring jumps to that value.
- ⌥-click resets to default; ⇧-drag for fine adjustment.
- Double-click a numeric field to type.

**House style of Logic's own plug-ins** (`apple-logic-quick-sampler.jpg`, ChromaVerb, Vintage EQ):

- **Body.** Dark blue-slate (Quick Sampler panel `#293740`, chips `#1e2730`, displays navy
  `#14335f`). ChromaVerb has a black display over a light grey control deck.
- **One accent colour per plug-in**, used for values, arcs and active chips: yellow `#ffd653` in
  Quick Sampler, orange in ChromaVerb.
- **Sections.** Centred UPPERCASE heading ("PITCH", "AMP"), or "⏻ FILTER" with a mode menu in
  accent at the right. Sections are separated by 1-px lines.
- **Knob grid.** 3 columns × 2 rows per section; label above in grey `#9ba5ae`; knob about 34 pt.
  Value readouts appear on the knob's ring or below it, in accent; toggles are dark chips.
- **Displays that are also controls.** The envelope graphs print **A: 46 ms, H: 10000, D: 10000,
  S: 0.00 %, R: 0 ms** at their corners, and their mode ("AHDSR ⌃⌄") is a menu *inside* the
  display. ChromaVerb's damping-EQ graph and ChromaVerb/Vintage EQ tab switches ("MAIN | DETAILS")
  sit in the same header row as the preset name pill ("Bloomy").
- **Instrument tabs.** "CLASSIC | ONE SHOT | SLICE | RECORDER" as a segmented bar; the active tab
  is filled with the accent colour.

### 5.2 Studio One / Fender Studio Pro

**Frame** (`fsp8-plugin-header-1.png`).

- **Title bar**: ▼ + channel name … pin ✕. It is tinted with the channel colour when *Colorize
  Plug-in Header* is on.
- **Tab bar** (`#111111`): Channel Editor, Macros and Routing icons, then **"1 - Compressor",
  "2 - Pro EQ ▾" …** One window serves the whole chain. ⌘PgUp/PgDn cycle through the inserts, and
  F11 opens the editor for the selected track.
- **Toolbar** (`#191b1e`): segmented groups on dark rounded boxes:
  - **⏻ activate** (blue), **bypass**, **automation mode**;
  - the **preset box**: 📄 menu, ◀ ▶, centred name "default*" ▼;
  - a **Sidechain** pill with a sources arrow;
  - at the right, a remote/device selector in yellow and ▽ **expand**, which reveals Compare,
    Copy / Paste and Focus Device, then Edit Automation and Recent Parameters.
- **Body.** Built-in plug-ins follow the Dark/Light/Colored appearance setting. Custom-UI
  plug-ins such as Ampire and Fat Channel keep their own look.

**Fat Channel XT as the layout model** (`fsp8-fat-channel-xt.png`).

- **Module bar.** Logo, a view toggle, then **⏻ HPF/Gate | ⏻ Compressor ▾ | ⇄ | ⏻ Equalizer ▾ |
  ⏻ Limiter**. The order can be swapped.
- **Stacked sections.** Each has a cyan ≈15-pt title with a power icon; backgrounds are
  `#2e2f31` / `#292b2c`.
- **Knob blocks.** Knob, then label over value at the right.
- **Button toggles** such as "Key Listen", "Expander" and "Soft/Auto": dark when off
  (`#3f4142`), **inverted light when on** (`#c3cad5`).
- **Graphs** on `#111217` with blue grid lines.
- **EQ band cards** under the graph: one card per band, headed with its short name ("LF", "LMF")
  in the band's colour, then a label/value table.

**Macro Controls** (`fsp8-channel-editor-macros.png`).

- 8 **ring-only arc knobs** (a thick coloured arc, no knob body), each with a coloured label and
  its value in a dark boxed field.
- 8 colour-block buttons and 2 XY pads.
- Any plug-in parameter maps to them, with curves.

---

## 6. Navigation and speed

**Logic Pro**

- Single-key toggles for areas: **X** Mixer, **I** Inspector. ⌘2 opens the Mixer window;
  ⌥⇧R opens the Region inspector as a window.
- The mixer: ←/→, type selection (⇧A/⇧S/⇧F/⇧O/⇧E/⇧C/⇧M), ⇧X for views, *Scroll To*,
  *Autoscroll to Selection*, ⌃⇧L Long Faders, ⌥X components, ⌃N new aux, **Sends on Faders**.
- Zoom: sliders; **Save / Recall Zoom 1–3** per area; ⌥-drag on the playhead's top to zoom;
  trackpad zoom centres on the pointer or the playhead.
- Hover: help tags that can show default values (*Show default values*); plug-in slots reveal
  three hot zones.
- Screensets, Link and Catch.
- Every numeric field is drag-editable and double-click-typable.

**Studio One**

- F3 Console, ⇧F3 Small/Large, F4 Inspector, F11 plug-in editor, ⌘PgUp/PgDn cycle plug-ins.
- ⌃⌥C find channel, ⌃⌥S recall scene.
- **M / S** on the selection.
- ←/→ across channels, with the pan pop-up following.
- Channel List with a filter and **drag-through show/hide**.
- Double-click empty strip space to expand a strip; auto-expand the selected channel.
- Copy/paste whole channel settings.
- Drag-and-drop everywhere: effects from the Browser onto tracks, channels or sends; inserts
  between channels; the inserts header copies the chain; the console auto-scrolls when you drag
  near its edge.
- v8: the Channel Overview's **channel tabs** give one-click access to any channel's details; the
  Arrangement Overview band gives the same for the session.

---

## 7. What Cantus should take (ranked)

The recommendations assume egui 0.36: custom painting is cheap, `SidePanel` and `TopBottomPanel`
are resizable, `egui_dock` provides tabs and docking, and native OS windows per panel are not an
option.

### 1. A grid discipline that ends "loose words". *(Highest impact; applies everywhere.)*

- **Every label belongs to exactly one row or one control.**
  - Panels use **label | value rows** at a fixed pitch (22 pt for forms, 20–21 pt for rack rows).
  - Put the label either **right-aligned to a gutter** (Logic form) or **left-aligned with the
    value right-aligned** (Studio One micro views). Pick one per surface and never mix them in
    one section.
- **The mixer gets a legend column** (Logic). Row names are printed once at the left, and strips
  carry only values. Remove per-strip captions such as "SENDS", "Mono", "Ø", "L/M/H".
- **Knob blocks** use one of two fixed templates: label above, knob, value below (compact
  cards); or knob, then label over value at the right (dense editors). **Units are 1–1.5 pt
  smaller and dimmer than numbers**, which needs a `LayoutJob` with two `TextFormat`s.
- **Three text sizes per surface, at most:**

  | Use | Size (egui pt) | Weight / style |
  |---|---|---|
  | Body / value | 12.5 | Plex Sans regular |
  | Label | 11.5 | Secondary colour |
  | Scale / caption | 9 | — |
  | Headers | 14 | Semibold |

  Logic goes down to 7 pt, but egui's grayscale anti-aliasing on Windows is softer than macOS, so
  keep **≥ 9 pt**.
- **Numbers** use a consistent format (dB to 1 decimal on faders, 2 decimals in plug-ins,
  explicit "+", "-∞") and are **right-aligned** or placed in fixed-width fields so they don't
  jitter. egui cannot turn on OpenType `tnum`: if Plex Sans digits turn out to be proportional,
  right-align them or use Plex Mono for digits only.
- **Truncation.** Implement **middle ellipsis** (egui only end-truncates): measure with
  `fonts.layout_no_wrap` and binary-search the cut. Wrap names onto **2 centred lines** in name
  bars (Logic 11). Show a tooltip with the full name whenever text is shortened. Give built-in
  plug-ins **short slot names**.

### 2. Rebuild the mixer side panel as a "Channel sheet". *(The user's main complaint.)*

Take Studio One's Channel Editor structure and Logic's inspector ideas. From top to bottom:

1. **Header band.** A 6-pt colour swatch, the number and name (14 pt semibold), and the type
   caption. Click the name to rename.
2. **Two columns: a strip and its chain.**
   - **Left, 70–80 pt: a compact strip.** I/O pills, pan, M/S/R block, value, then fader and meter.
   - **Right: stacked sections**, each with a **header band** (⏻ Title ▼ +):
     - **Inserts.** One 21-pt row per plug-in: ⏻ | name | ▼. A disclosure arrow **expands the
       row into a micro view** of 3–6 parameter rows (name left, value right, a 3-pt value bar
       along the bottom that is itself the drag target), or a thumbnail graph for EQ, compressor
       or spectrum.
     - **Sends.** A row plus a level bar and pre/post.
     - **Output / destination.** Logic's idea: show where this channel goes, and let the user
       click to jump to it.
3. **Sticky section headers** while the panel scrolls vertically. egui has no sticky headers:
   paint the header at `clip_rect.top()` when its section's top has scrolled off.
4. **Resizable** (`SidePanel::right(...).resizable(true)`) with snap widths of about 280, 360 and
   480. Wider widths switch the micro views to 2 columns.
5. **Optional wide mode: a bottom "Channel Overview" dock**, as in Studio One v8 and Logic iPad's
   Plug-ins area. Plug-in **cards** left to right in chain order, each a header (⏻, name, preset
   pill, open) over a 2×2 or 3×2 knob grid and its display. Channel tabs sit along the bottom.

**"Better than" means:**

- Resizable in both directions. Studio One's overview is fixed, and SOS criticised it.
- Unlike Logic's fixed-width inspector, the panel gets wider rather than only bigger text.
- The entire chain is readable without opening a single plug-in window.
- Labels align to at most 2 x-positions per section.

### 3. A mixer that scrolls and sizes predictably.

- **One row grid for the whole desk.** All insert wells are the same height (the longest chain,
  or a user-set slot count), all send wells too. **Draw empty slots as inset wells** a step darker
  than the strip.
- **Zones with draggable dividers** (Studio One): the racks area (inserts and sends) **scrolls
  vertically on the mouse wheel with no modifier**, and ⇧-wheel scrolls horizontally. The **fader
  block is pinned at the bottom** and resizable with a divider that shows a hover handle.
- Add a **Long faders** toggle (Logic).
- Alternatively, follow Logic: let the whole desk scroll in a `ScrollArea::both()` with the legend
  inside it. Either way, **a visible vertical scroll bar must exist** when content overflows.
- **Width presets:** Narrow about 36 pt (meter plus overlay fader, M/S above, 2-line names) ·
  Normal about 70 pt (Logic) · Wide about 88 pt (Studio One). **Double-click empty strip space**
  to expand a single strip.
- **Master / main pinned at the right** (Studio One, Logic iPad); optionally the selected strip
  pinned at the left (Logic iPad).
- **Overview / meter bridge** above the desk: a scrubbable strip of numbers and mini meters for
  jumping across large sessions (Logic iPad), plus *Scroll To ▸ type* and **Autoscroll to
  selection**.
- **Channel list** with show/hide dots, drag-through toggling and a comma-separated filter
  (Studio One).
- **Selection:** ←/→ move, ⇧ extends, **M / S / R keys act on the selection**; select by type or
  colour; ⌃⌥C-style "find channel".

### 4. Colour semantics and depth.

- A **5–6 step surface ladder**. Dark theme, adapted from Logic: canvas `#2e2e2e` · chrome
  `#474747` · desk `#4d4d4d` · strip `#575757` · well `#505050` · selected `#737373`. The cooler
  Studio One variant is `#202528` / `#2e343b` / `#393f44` / `#2e3033`.
- **Selection is a lighter surface**, not just an outline.
- **One "on" accent** (Studio One `#37a6fa`, Logic blue `#2772bf`/`#266fb7`) for power, arcs,
  active tabs and value bars.
- **Type colours for slots:** effect blue, instrument green.
- **Automation colours:** Read green text, Touch ochre, Latch orange, Write red.
- **Track colour** goes on the swatch and name bar. Tinting whole strips is an *option* (Studio
  One's *Colorize*), not the default.
- **Depth:** keep it flat. Use 1-pt lighter top edges on pills and darker inset wells; reserve
  "real" rendering (metal caps, shaded knobs) for faders and knobs only. Radius 3 pt on pills,
  4–6 pt on cards.
- Both themes must keep labels at **≥ 4.5 : 1** and values at **≥ 7 : 1**. Logic's `#c8c8c8`
  label on `#525252` is about 4.6 : 1; white on `#525252` is about 7.8 : 1.

### 5. One plug-in window frame for every effect and instrument.

- **Header row:** ⏻ | preset ◀ ▶ "name*" ▼ | Compare | Undo / Redo | Copy / Paste | size % |
  pin.
- **A tab row listing the channel's inserts**, so one window is reused (Studio One). **Pin**
  keeps a window. Optionally **tint the header with the channel colour**.
- **Footer** with the plug-in name and a disclosure for advanced parameters (Logic).
- **Resize** by corner drag, snapping to 75, 100, 125, 150 and 200 %: paint at scale inside an
  egui `Window` or viewport.
- A generic **"Controls" view** (a list of label | slider | value rows) as the fallback for every
  plug-in, including third-party ones.
- **Inside the body, one grammar** (section 1). Each plug-in gets **one accent colour**. Sections
  get UPPERCASE 10-pt captions with +5 % tracking (a `LayoutJob` with `extra_letter_spacing`), or
  ⏻ + title. **Displays are controls** and print their values at the corners (Logic envelopes,
  Studio One compressor curves).
- Toggles are **dark chips that invert when on** (Fat Channel) or take the accent colour (Logic).

### 6. Panels you can resize, detach and remember.

- Every split gets a **hover-highlighted 2-pt divider** (Logic `#83bffd`) with a resize cursor,
  plus a double-click to reset.
- **Each bottom-panel view remembers its own height** (Logic).
- Use `egui_dock` for the mixer, editors and channel sheet so they can be tabbed and
  split-docked. Detaching to separate OS windows requires egui multi-viewport support, which is
  possible but keep it for later.
- **Workspaces / screensets** on number keys (Logic): save the dock layout, bottom-panel choice
  and heights.
- **A key-focus frame** around the focused area (Logic), so arrow keys and M/S obviously go
  somewhere.

### 7. Smaller wins

- EQ thumbnail at the top of each strip: click opens the EQ.
- Gain-reduction mini meter.
- **Slot hover zones**: bypass on the left, open in the middle, menu on the right.
- Drag inserts between strips (⌥ copies); drag the rack header to copy the chain; drop an effect
  on Sends to create an FX return.
- A trash bin for removed inserts.
- Copy/paste whole channel settings.
- Help tags showing default values.

### What *not* to copy

- **Studio One's clipped narrow-mode names and edge-clipped tabs** ("Snare :", "Oveead").
  Cantus should never cut through a glyph: wrap, abbreviate or middle-truncate with a tooltip.
- **Logic's Smart Controls brushed metal and the vintage-hardware plug-in skins.** They are
  expensive to paint well in egui, clash with the flat house style, and hurt legibility.
- **Studio One v8's fixed-size Channel Overview.** Copy the idea, not the rigidity.
- **Logic's fixed-width inspector and hidden modifier-click functions** (⇧-click a send to show
  its aux, ⌥/⇧/⌘-click variants on the EQ thumbnail) as the *only* route. Offer visible buttons
  and menus too.
- **Many floating plug-in windows by default** (Logic without Link). Prefer Studio One's single
  tabbed window with an opt-in pin.
- **Tinting everything with the channel colour by default.** Rose-tinted panels
  (`fsp8-channel-editor.png`) lower contrast and fight with meters. Keep it an option.
- **7-pt scale digits** (Logic's meter scale). They are fine on Retina with macOS rendering;
  in egui on 100 % Windows, stay at 9 pt or more.

---

## Appendix: screenshot index

All files are in `research/ui/screenshots/logic-studio-one/`.

### Logic Pro: Apple Logic Pro User Guide (Mac)

Page URLs are `https://support.apple.com/guide/logicpro/<page>/mac`. Image assets are at
`https://help.apple.com/assets/697921A0B89A6DAE620D667A/697921A59331DF6F330EFA33/en_US/<asset>`.

| File | What it shows | Page / asset |
|---|---|---|
| `logic-main-window-overview.png` | Main window: control bar, toolbar, track headers, regions | `main-window-interface-lgcpe9cc403a` / `64a00fbdc18b63fd5ecc11895199c3d5.png` |
| `logic-inspector-region-track-channelstrips.png` | Inspector: Region/Track sheets, form rows, two inspector strips | `inspector-interface-lgcpe9cc3b1d` / `883d73d305c449cadffbfeaa578aeae3.png` |
| `logic-mixer-window.png` | Mixer with legend, filters, Single/Tracks/All, vertical scroll bar | `mixer-interface-lgcpe9cc43f6` / `04e60b8fd04f6f0bdf93dedd5c7e8da4.png` |
| `logic-channel-strip-anatomy.png` | Every strip row, with measurements | `channel-strip-controls-lgcpbc219210` / `ef13b0ae7f77f0aea1b7ae8edb845ee8.png` |
| `logic-mixer-channel-strip-components-menu.png` | Mixer View menu and Channel Strip Components | same page / `e50d07be83195c46af595c99ef0130af.png` |
| `logic-channel-strip-eq-thumbnail.png` | EQ thumbnail, slots, sends with knobs | `use-the-channel-eq-lgcpbc219e52` / `8bdadcea73efefbce1547aa54909e379.png` |
| `logic-plugin-slot-bypass-hover.png` | Plug-in slot hover zones (bypass / open / menu) | `add-remove-move-and-copy-plug-ins-lgcpbc218a22` / `2fc84bb5f37f083231e0a96b8bdeaa98.png` |
| `logic-plugin-header-compare.png` | Plug-in header: power, setting menu, ◀ ▶, Compare, size % | `work-with-plug-in-settings-lgcp4dcb0092` / `59ab443862dc297357a87d09e3b3d284.png` |
| `logic-plugin-window-header.png` | Plug-in window with the header hidden (Channel EQ) | `work-in-the-plug-in-window-lgcpbc21a1fd` / `6186e0f5dff74dde222ac18a394f8e80.png` |
| `logic-plugin-window-size-menu.png` | View menu: Controls / Editor, 50–200 % | same page / `fb4c77596550e50220707882c7010019.png` |
| `logic-resize-bar-loop-browser.png` | Resize bar highlighted blue with resize cursor | `move-and-resize-windows-lgcp5cbf1551` / `714eba4c69f6a7e3b9a06b0919160139.png` |
| `logic-settings-view-general.png` | Settings form: right-aligned labels, "Large inspectors" | `general-settings-lgcp9793a910` / `c8891c8c509eb896ec1373c1e1fb0ca1.png` |
| `logic-smart-controls-pane.png` | Smart Controls (skeuomorphic; counter-example) | `smart-controls-interface-lgcp91601243` / `189dd32365b75ba3f1fd209491b06f65.png` |

### Logic Pro: Apple Logic Pro User Guide (iPad)

Page URLs are `https://support.apple.com/guide/logicpro-ipad/<page>/ipados`. Image assets are at
`https://help.apple.com/assets/697908FC5CF7AEEA580EF573/697908FD5CF7AEEA580EF579/en_US/<asset>`.

| File | What it shows | Page / asset |
|---|---|---|
| `logic-ipad-plugins-area.png` | Plug-ins area: selected track's chain as tiles | `plug-ins-area-lpip8a07e838` / `b4d0124d35ade2b4c1677ea678015def.png` |
| `logic-ipad-mixer.png` | iPad mixer: pinned strips, meter bridge, 2-line slot names | `mixer-lpip2dc06e7c` / `f76d8f9d47c05fbb444ea05497d73b45.png` |
| `logic-ipad-strip-mini-view.png` | Touch-and-hold mini-view popover | `work-with-plug-ins-in-the-mixer-lpip806df382` / `5d1156343b56cf566dabd6d0beae16c9.png` |

### Logic Pro: apple.com product page

Page: `https://www.apple.com/logic-pro/`. Image URLs are
`https://www.apple.com/v/logic-pro/n/images/overview/<path>`.

| File | What it shows | Path |
|---|---|---|
| `apple-logic-mixing-hero.jpg` | Logic 11 mixer at about 1.08 px/pt: legend, 2-line names, pills | `mixing/mixing_hero_endframe__do9497nn8w8y_large_2x.jpg` |
| `apple-logic-chromaverb.jpg` | ChromaVerb: header-less frame, accent values, footer name | `sounds/modal/effects/chromaverb__fxn05v4tj5e2_large_2x.jpg` |
| `apple-logic-vintage-eq.jpg` | Vintage EQ windows (compact frames; skeuomorphic bodies) | `sounds/modal/effects/vintage_eq__cutn5wlxh4mu_large_2x.jpg` |
| `apple-logic-quick-sampler.jpg` | Quick Sampler: sections, knob grid, envelope displays | `sounds/modal/instruments/quick_sampler__ek63035rfl6q_large_2x.jpg` |

### Studio One / Fender Studio Pro: user manual

Image URLs are `https://fenderstudiopromanual.fender.com/en/Content/Resources/Images/<file>`.
Pages are under `https://fenderstudiopromanual.fender.com/en/Content/`.

| File | What it shows | Page / image |
|---|---|---|
| `s1-console-large-main.png` | Large Console, colourised, micro views in racks (Studio One 6/7) | `Mixing_Topics/The_Console.htm` / `SO60_mixing_the_console_main_883x337.png` |
| `s1-console-small.png` | Small Console with one channel expanded (Fat Channel micro view) | same / `SO60_ConsoleSection.png` |
| `s1-console-large-narrow.png` | Large Console in Narrow mode (tall meters; truncated names) | same / `SO60_mixing_the_console_narrow_.png` |
| `s1-console-nav-column-wrench.png` | Console navigation column and a normal strip | same / `Mixing_The-Console_Console-Options.png` |
| `s1-console-channel-list.png` | Channel List with show/hide, filter, type toggles | same / `SO60_mixing_console_chnlst.png` |
| `fsp8-channel-editor.png` | Channel Editor (v8): strip + Inserts / Sends / Cue columns | `Mixing_Topics/Channel Overview and Editor.htm` / `11_Mixing/Mixing_Channel-Editor_Overview_v8.png` |
| `fsp8-channel-editor-macros.png` | Macro Controls: arc knobs, colour buttons | same / `11_Mixing/Mixing_Channel-Editor_Macros_v8.png` |
| `fsp8-channel-overview.png` | Channel Overview (v8), full width | same / `11_Mixing/Mixing_Channel-Overview-and-Editor_Overview-Window_v8.png` |
| `fsp8-channel-overview-controls.png` | Channel Overview left column | same / `…_Channel-Overview-Controls_v8.png` |
| `fsp8-channel-overview-compact-views.png` | Plug-in cards with knob grids and insert pills | same / `…_Compact-View_v8.png` |
| `fsp8-channel-overview-tabs.png` | Channel tabs under the overview | same / `…_Channel-Overview-Options_v8.png` |
| `fsp8-micro-views.png` | Micro views: parameter rows with value bars | `Built-In_Effects_Topics/Effect_Micro.htm` / `17_Built-In Effects/Built-In-Effects_Micro-Views_Small_v8.png` |
| `fsp8-fat-channel-xt.png` | Fat Channel XT: sections, knob blocks, EQ band cards | `Built-In_Effects_Topics/Mixing.htm` / `17_Built-In Effects/Built-In-Effects_Mixing_Fat-Channel-XT_v8.png` |
| `fsp8-plugin-header-1.png` | Plug-in window: tinted title, insert tabs, toolbar | `Built-In_Effects_Topics/Chapter-Built_In_Effects.htm` / `17_Built-In Effects/Built-In-Effects_Chapter-Overview_Plug-in-Header_v8.png` |
| `fsp8-track-inspector.png` | Inspector: rows, swatch header, channel strip under a grip | `Editing_Topics/Track_and_Event_Inspectors.htm` / `7_Editing/Editing_Track-and-Event_Track-Inspector_v8.png` |
| `fsp8-session-page.png` | Whole Session page (Windows 1x): Browser cards, Overview dock | `Pages_Topics/Session.htm` / `5_Pages/Pages_Session_Session-Page_v8.png` |

### Other sources (text, no images kept)

- Apple Logic Pro guide pages:
  - [Change the Mixer view](https://support.apple.com/guide/logicpro/change-the-mixer-view-lgcp583779d6/mac)
  - [Navigate within the Mixer](https://support.apple.com/guide/logicpro/navigate-within-the-mixer-lgcp8e9563fe/mac)
  - [Channel strip types](https://support.apple.com/guide/logicpro/channel-strip-types-lgcpbc2192ea/mac)
  - [Mixer key commands](https://support.apple.com/guide/logicpro/mixer-lgcp2dc3f212/mac)
  - [Mixer View settings](https://support.apple.com/guide/logicpro/mixer-settings-lgcp50b91186/mac)
  - [Zoom windows](https://support.apple.com/guide/logicpro/zoom-windows-lgcp5cbf2096/mac)
  - [Intro to mixing (iPad)](https://support.apple.com/guide/logicpro-ipad/intro-to-mixing-lpip1454fa06/ipados)
- Fender Studio Pro manual pages:
  - [The Console](https://fenderstudiopromanual.fender.com/en/Content/Mixing_Topics/The_Console.htm)
  - [Effects Signal Routing](https://fenderstudiopromanual.fender.com/en/Content/Mixing_Topics/Effects_Signal_Routing.htm)
  - [General Options (Appearance)](https://fenderstudiopromanual.fender.com/en/Content/Setup_Topics/General_Options.htm)
- Sound On Sound:
  - [Studio Pro: The Channel Overview](https://www.soundonsound.com/techniques/studio-pro-channel-overview)
  - [Fender Studio Pro 8 review](https://www.soundonsound.com/reviews/fender-studio-pro-8)
  - [Using Studio One's Mixing Console](https://www.soundonsound.com/techniques/using-studio-ones-mixing-console)
