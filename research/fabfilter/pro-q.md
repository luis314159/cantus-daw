# FabFilter Pro-Q 4: competitive research for the Cantus EQ

*Researched 2026-09-23. Sources are FabFilter's product page, the official PDF manual (current edition; it
mentions Pro Tools 2025.12 and the March 2026 slope-menu change), the online help, FabFilter's news posts,
press reviews and user forums. Everything below is paraphrased. Where the manual is silent and a value comes
from another source or an official screenshot, the text says so. Screenshots are FabFilter's own and are in
`screenshots/`, used here for internal reference only.*

---

## 0. The short version

1. **Pro-Q 4 is the reference EQ, and most of its lead comes from the interaction design, not the DSP.** The
   display is the whole interface. Band controls float under the selection, where a band is created decides its shape,
   and the dynamic controls only appear once you use them. Reviewers praise the workflow more than the sound.
2. **Its newest features are its weakest spots.** Spectral dynamics always runs linear phase, which adds at least
   3072 samples (about 70 ms) and sometimes surprises users with project-wide delay. Character modes are
   described as too subtle, and EQ Sketch "needs practice". The instance list can't be reordered or grouped, and the
   plug-in depends on whatever the DAW is willing to report.
3. **Cantus is the host, which is our biggest structural advantage.** Pro-Q 4 guesses track order, colour and
   project membership through plug-in APIs that are often missing. We have the real mixer, so a mix-wide EQ view,
   cross-track masking and sidechains from any track cost us nothing.
4. **Dynamics in Pro-Q are deliberately opaque.** Attack and release are percentages where 50% means "auto".
   There is no ratio, no knee and no lookahead. Competitors (Kirchhoff-EQ, TDR Nova) expose all of these. We can
   offer Pro-Q's easy defaults *and* real units, and show the gain change over time on the display.
5. **Legibility is an open goal.** Pro-Q 4's darker redesign drew complaints that it put "form over function",
   with low-contrast knobs and hover-only panels. Users are also asking for dB readouts at the cursor. Cantus
   already has a light default theme and an editable palette, so we can compete on clarity.

---

## 1. Version, release date and what changed

| Version | Date | What changed | Source |
|---|---|---|---|
| **4.00** | **12 Dec 2024** | Launch. See the list below. Priced at EUR 169 / USD 179 / GBP 144. | [release news](https://www.fabfilter.com/news/1733994000/fabfilter-releases-pro-q-4-equalizer-plug-in) |
| 4.01 | 17 Dec 2024 | Crash fixes (e.g. EQ Sketch while bypassed), a Cubase latency-report fix for *Make Spectral*, multi-band copy/paste fixed, analyzer defaults corrected. | [4.01 news](https://www.fabfilter.com/news/1734440400/fabfilter-pro-q-401-update-released) |
| 4.02 | 27 Feb 2025 | **Per-band Spectral Tilt** (3 dB/oct tilt on the detector). Character-mode oversampling becomes linear phase in LP mode. Pasted bands get a small frequency offset. Natural sorting of track names. The instance list is limited to the current project. **All Pass shows as bypassed in linear-phase mode.** Many fixes. AUv3 iPad version released. Pro-Q 3.27 adds copy so settings can be pasted into v4. | [4.02 news](https://www.fabfilter.com/news/1740654000/fabfilter-pro-q-402-update-released) |
| **4.10** | **15 Jan 2026** | The **instance list becomes multi-plug-in**: it now controls Pro-C 3, Pro-DS and Pro-G instances too, like a "multi-track channel strip". A spectral band now always adds its latency, even at 0 dB range. | [4.10 news](https://www.fabfilter.com/news/1768467600/fabfilter-introduces-multi-plugin-instance-list-in-pro-q-410) |
| (4.1x) | 19 Mar 2026 | Clicking the slope in the value popup now opens the slope menu. EQ Match fixes, including one where matching a signal against itself gave about 1 dB of difference instead of flat. Latency reporting fixed for spectral bands at Very High/Maximum. Drag-and-drop presets and preset copy/paste across all FabFilter plug-ins. | [Mar 2026 news](https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes) |
| (4.1x) | 16 Apr 2026 | Crash fix when a spectral band's range is set to 0. Better diagonal mouse movement in the preset browser's folders. | [Apr 2026 news](https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins) |
| **4.13** | **30 Jun 2026** | **Latest version as of Sep 2026.** Fixes rare invalid output when slowly moving a band's dynamic range around 0 dB in zero-latency mode. | [4.13 news](https://www.fabfilter.com/news/1782806400/pro-q-413-released) |

**New in 4.0 compared with Pro-Q 3** ([manual p.3](https://www.fabfilter.com/downloads/pdf/help/ffproq4-manual.pdf), [release news](https://www.fabfilter.com/news/1733994000/fabfilter-releases-pro-q-4-equalizer-plug-in)):
- **Spectral dynamics**: a per-band mode that compresses or expands only the frequencies inside the band that cross
  the threshold.
- **Instance list**: a zoomable overview of every instance in the session. It is also where EQ Match and collision
  detection now live.
- **EQ Sketch**: draw a curve in one gesture and bands are created along it.
- **Character modes**: Clean, Subtle and Warm (saturation). The product page calls them "Gentle and Warm"; the
  manual and news posts say *Subtle*.
- Dynamic EQ gains **attack and release** knobs and **free sidechain filtering** (custom trigger band). Its processing
  is also cleaner, with less distortion.
- **Fractional slopes**, e.g. a 3.5 dB/oct cut or a 14.2 dB/oct bell.
- **All Pass** shape.
- **Copy and paste of bands and presets**, including between instances.
- Values in the popup can be **dragged or scrolled** directly.
- Better linear-phase precision and better analog matching in the Zero Latency and Natural Phase modes.
- **Preset browser** with tags, favourites, search and filtering.
- A new look.

**Formats and platforms**: VST, VST3, AU, CLAP, AAX Native, AudioSuite, and AUv3 on iPad. Windows 7 to 11 (32 and 64-bit)
and macOS 10.13+ ([product page](https://www.fabfilter.com/products/pro-q-4-equalizer-plug-in)). Pro-Q 4
installs alongside v1 to v3 and reads their presets, but **it can't read automation written by older versions**
([upgrading](https://www.fabfilter.com/help/pro-q/support/upgrading)).

**Inconsistencies in the documentation.** These show where the spec is fuzzy, even for FabFilter:
- The character mode is called "Gentle" on the product page and "Subtle" in the manual.
- The Band controls page says the dynamic range works on Bell and Shelving shapes. The Dynamic EQ page says Bell,
  Shelving *and Flat Tilt*.

---

## 2. Screenshots saved

All are official FabFilter images, downloaded with `curl` from the product page and the online help. Every file
was checked to be a real JPEG with an `FFD8FF` signature and larger than 30 KB.

| File | What it shows |
|---|---|
| `pro-q-01-main.jpg` | The full interface (2298×1380, from the product page). Six bands of several shapes. A purple dynamic bell shows its range band with ▲▼ handles, and a spectral bell shows a jagged yellow result curve. Pre and post spectra are drawn behind. The floating band panel shows "(multiple)" for a multi-selection. On the right are the yellow EQ dB scale (±12) next to the grey analyzer scale and a stereo output meter. The bottom bar shows MIDI Learn, *Natural Phase + High*, the instance button ("Guitar"), *Analyzer: Pre+Post+SC*, character, and 100% / 0.0 dB. |
| `pro-q-02-overview-annotated.jpg` | The same view with FabFilter's labels: undo/redo, A/B, presets, help, full screen, output meter, resize, output options, character modes, analyzer, instance list, processing mode, band controls, MIDI learn, piano and EQ Sketch buttons. |
| `pro-q-03-eq-display-parameter-popup.jpg` | The hover popup next to a band: frequency, gain and Q (with a "Cmd" hint for Q-drag), power, solo (headphones) and delete, plus a row of 10 shape icons. |
| `pro-q-04-band-controls.jpg` | The floating band panel: bypass, shape (Bell) with its slope (12 dB/oct), FREQ, GAIN (with a range ring), the gain–Q interaction icon, Q (default 1), ‹ 2 › band stepper, delete, stereo placement (Stereo) and split (scissors). |
| `pro-q-05-dynamic-eq.jpg` | Several dynamic bands. Each has a filled region between its static gain and gain plus range, a ▲▼ handle to drag the range, and a yellow curve showing the *current* dynamic response. |
| `pro-q-06-dynamic-band-controls-expanded.jpg` | The expanded dynamics panel. The range ring is red and shows the live gain change in yellow. It also shows the threshold slider (A = auto) with a level meter, sidechain and audition buttons, the Free trigger with draggable low/high handles drawn as a bar above the panel, and A and R knobs. |
| `pro-q-07-spectral-dynamics.jpg` | A spectral band, green, 1–20 kHz. The result curve cuts only individual peaks. The panel adds a Spectral Density slider and a Spectral Tilt icon. |
| `pro-q-08-instance-list.jpg` | The instance list at mixed zoom. Tracks run as rows with colour tags and pins. Mini Pro-Q, Pro-C, Pro-DS and Pro-G panels sit side by side, and one Pro-Q is fully editable. A coloured minimap lists the tracks on the right. The toolbar has zoom, search, options and a pin filter. |
| `pro-q-09-eq-sketch.jpg` | EQ Sketch while drawing: a freehand yellow curve with a glowing pen cursor. The sketch button (bottom left) is lit. |
| `pro-q-10-eq-match-reference.jpg` | The EQ Match panel in the instance list, with Input and Reference record buttons. The reference menu offers Input, Sidechain, External (other instances by track name), Load File, and Save Input As Reference Spectrum. |
| `pro-q-11-eq-match-result.jpg` | The match result: a white difference curve, fitted bands (8) in yellow, a NUMBER OF BANDS slider, and ‹ Analyze / Finish. |
| `pro-q-12-analyzer-settings.jpg` | The analyzer panel: Pre, Post and SC toggles, with a source menu listing the sidechain and other instances. Range 120 dB, Resolution Maximum, Speed Medium, Tilt 4.5 dB/oct, plus Spectrum Grab, Collisions and Freeze buttons. |
| `pro-q-13-spectrum-grab.jpg` | Spectrum Grab: a frozen spectrum tinted purple, peaks labelled (64 Hz, 281 Hz, 692 Hz, 1.5 kHz, 5.7 kHz), and the cursor frequency readout on the axis. |
| `pro-q-14-stereo-lr-mid-side.jpg` | Per-band placement. Band dots carry M, L, R and S badges, and result curves are grouped per channel (white L, red R, green M, blue S). |
| `pro-q-15-preset-browser.jpg` | The preset browser: options gear, search, favourites star, folder list, and a details pane with name, author, tags (+) and description. |
| `pro-q-16-piano-display.jpg` | The piano strip replaces the frequency axis. There is one dot per band on the keys, and a tooltip reads "Band 3 Frequency 392.00 Hz G4 +00". |
| `pro-q-17-surround-atmos.jpg` | The surround speaker picker (All, Center, L/R, Lss/Rss, Lsr/Rsr, LFE, plus Stereo placement). The dot is labelled "Lss/Rss" and the meters are labelled per speaker. |
| `pro-q-18-band-solo.jpg` | Band solo: a red headphone button, with the audible region shaded between two vertical lines. |

Other official images were downloaded to scratch but not kept: EQ Match access menu, output options, knobs, MIDI
learn, resize/scaling menus, processing mode menu, magnitude and phase plots, sidechain routing per DAW. They are
all at `https://www.fabfilter.com/help_pages/pro-q/images/<name>@2x.jpg` if needed later.

---

## 3. Anatomy of the interface

([overview](https://www.fabfilter.com/help/pro-q/using/overview), [manual p.6–7](https://www.fabfilter.com/downloads/pdf/help/ffproq4-manual.pdf))

- **Top bar:** logo; Undo and Redo; A/B and Copy; ‹ preset name › (the name is dimmed when modified); Help menu;
  Full Screen (top right).
- **EQ display:** fills the window. There are two dB scales on the right: a yellow one for the EQ curves and a grey
  one for the analyzer and meter. The display-range dropdown sits at the top of the yellow scale. Frequency runs from
  **10 Hz to 30 kHz**; the display extends past 20 kHz on purpose, so filters can sit there and still shape the
  audible range.
- **Floating band panel:** appears only when bands are selected, positioned under the selection. Deselecting hides it.
- **Bottom-left corner:** EQ Sketch button and piano display button.
- **Bottom bar:** MIDI Learn and its menu; Processing Mode, plus resolution when LP or spectral is in use; the
  **Instance list button**, which shows the track name; Analyzer, with a panel that pops up on hover; Character;
  global Bypass; Output options (hover panel, showing gain scale % and output dB); Resize.
- **Right edge:** output level meter with peak readouts. It can be hidden.

---

## 4. Feature reference: parameters, ranges, defaults

### 4.1 Bands and shapes

- **Up to 24 bands.** Bands are numbered 1, 2, 3… when created and are **never renumbered** after a delete, so that
  host automation keeps targeting the same band
  ([band controls](https://www.fabfilter.com/help/pro-q/using/bandcontrols)).

| # | Shape | Gain control | Q control | Min slope | Dynamic / spectral | Notes |
|---|---|---|---|---|---|---|
| 1 | Bell | yes | yes | 12 dB/oct | yes | Optional gain–Q interaction (bell only) |
| 2 | Low Shelf | yes | yes (internal mapping) | 6 | yes | |
| 3 | Low Cut | no | yes (resonance) | **0** | no | Brickwall option |
| 4 | High Shelf | yes | yes | 6 | yes | |
| 5 | High Cut | no | yes | **0** | no | Brickwall option |
| 6 | Notch | no | yes | 12 | no | |
| 7 | Band Pass | no | yes | **0** | no | |
| 8 | Tilt Shelf | yes | yes | 6 | (doc unclear) | Tilts around a frequency |
| 9 | Flat Tilt | yes | – | 6 | yes (per Dynamic EQ page) | Tilts the whole audible range |
| 10 | All Pass | no | yes | 6 | no | Phase only. **Has no effect in linear-phase mode** and is shown as bypassed there (4.02) |

Sources: [band controls](https://www.fabfilter.com/help/pro-q/using/bandcontrols), [dynamic EQ](https://www.fabfilter.com/help/pro-q/using/dynamic-eq), [4.02 news](https://www.fabfilter.com/news/1740654000/fabfilter-pro-q-402-update-released).

### 4.2 Band parameters

| Parameter | Range / values | Unit | Default | Notes |
|---|---|---|---|---|
| Frequency | 10 Hz – 30 kHz | Hz | set by where you create the band | Accepts typed values like `100`, `2k`, `A4`, `C#2+13` (note plus cents) |
| Gain | −30 … +30 | dB | 0 | Bell, shelf, tilt only. The display range auto-expands while dragging |
| Q | 0.025 – 40 *(Pro-Q 3 figure from [MusicManta](https://www.musicmanta.com/reviews/fabfilter-pro-q-3-in-depth-review); the v4 manual gives no range)* | – | **1**, defined as "the default bandwidth" | FabFilter warns that its Q doesn't match other EQs, and shelf Q uses an internal mapping. Not adjustable at 6 dB/oct |
| Slope | 0/6/12 … 96, **continuous and fractional**, plus **Brickwall** for LC/HC | dB/oct | Bell shows 12 in official shots | The mouse wheel steps through the classic values. Shift (Ctrl/Cmd in Pro Tools) allows any fractional value. Clicking opens a menu where you pick a value or drag an indicator |
| Gain–Q interaction | on/off | – | remembers the last choice for new instances | Analog-style: Q narrows as gain rises, and a little gain is added at very narrow Q |
| Stereo placement | Stereo, Left, Right, Mid, Side (surround: speaker sets) | – | Stereo | **Split** (scissors) duplicates the band into an L+R or M+S pair |
| Bypass | on/off | – | on | Alt-click the dot to toggle |

### 4.3 Dynamic EQ

([dynamic EQ](https://www.fabfilter.com/help/pro-q/using/dynamic-eq))

- **Design principle: progressive disclosure.** The controls only appear once a band becomes dynamic. The default
  "auto" behaviour depends on the programme material: attack, release and knee follow the audio, the band's frequency
  and the range.
- **Making a band dynamic:**
  - turn the range ring away from 0;
  - Alt+wheel over a band;
  - choose *Make Dynamic* from the band menu;
  - create the band with Alt held (Alt+drag the curve, Alt+double-click, or Alt+Ctrl-click).

| Control | Range / values | Notes |
|---|---|---|
| Dynamic range ring | −30 … +30 dB (may be limited by the gain limits) | Negative values compress, positive values expand. Red shows the range and yellow shows the live gain change. The range can also be dragged with the ▲▼ indicator in the display |
| Expand `>>` | auto / custom | Collapsing it reverts everything to auto |
| Threshold slider | top position = **Auto** ("A"); otherwise manual (no range given) | Shows the trigger level inside the slider. The soft knee means it can start acting slightly below the threshold |
| External sidechain | internal / external | The sidechain is band-limited the same way as the input |
| Attack, Release | knobs, **50% = the auto behaviour**; lower is faster, higher is slower | **No millisecond values are documented** |
| Trigger | **Band** (default: follows the band) / **Free** | Free exposes low-cut and high-cut trigger filters, drawn as a draggable bar in the display |
| Audition | hold to listen | Plays the trigger signal |
| Spectral toggle | on/off | See 4.4 |
| Bypass dynamics / Clear dynamics | – | Clear resets the range to 0 |

- **Linear phase:** dynamics work only up to the **High** resolution. At Very High or Maximum a warning icon appears
  and dynamics stop working. Attack and release also behave slightly differently in LP.
- **Missing compared with competitors:** ratio, knee control, lookahead, dual thresholds (above and below), a
  relative or absolute detector choice, and a separate sidechain source per band (there is only one external
  sidechain).

### 4.4 Spectral dynamics (new in v4)

([spectral dynamics](https://www.fabfilter.com/help/pro-q/using/spectral-dynamics))

- **What it does:** within the band, only the individual frequencies that exceed the threshold are processed.
  Tape Op compares it to "a bunch of tiny dynamic EQ bands".
- **Availability:** Bell and shelf bands with a non-zero range.
- **Enabling:** the Spectral icon on the gain/range ring, Alt+Shift+click in the display, or *Make Spectral* in
  the band menu.
- **Auto mode** sets the threshold, attack and release automatically. Expanding the panel gives the same manual
  controls as dynamic EQ, plus:
  - **Spectral Density**: selectivity. Low values act on wide regions; high values act on narrow, specific peaks.
  - **Spectral Tilt**: a +3 dB/oct tilt on the detector, on by default for new spectral bands since 4.02. It
    makes treble trigger more easily, which suits pink-ish full mixes.
- **Always linear phase, per band.** The other bands keep the global processing mode. The resolution control
  appears in the bottom bar with a spectral icon. Very High and Maximum aren't allowed. Low or Medium is recommended
  above 1 kHz.
- **Latency:** Low adds 3072 samples at 44.1 kHz (about 70 ms), and more at higher resolutions. Since 4.10 the
  latency is added whenever a spectral band exists, even at 0 dB range.

### 4.5 Stereo, mono and surround

([stereo](https://www.fabfilter.com/help/pro-q/using/stereo), [surround](https://www.fabfilter.com/help/pro-q/using/surround))

- **Per-band placement:** Stereo, Left, Right, Mid or Side. Result curves are grouped per channel: white for L,
  red for R, green for M, blue for S. Bands affecting all channels use the usual yellow.
- **Mono and surround:** the plug-in adapts to the track's channel layout. On mono tracks, placement and pan are
  hidden.
- **Surround up to 9.1.6 Atmos:**
  - a speaker picker with an All toggle (LFE excluded by default), row selection, individual speakers, and
    combined L/C/R;
  - a per-speaker L/R/M/S placement on top of the speaker selection;
  - multichannel meters.
  - Output pan isn't available in surround.
- **Incompatible presets:** bands that don't fit the current layout are shown disabled until you use *Reset
  Placement/Speakers*.

### 4.6 Processing modes and latency

([processing mode](https://www.fabfilter.com/help/pro-q/using/processingmode))

| Mode | Latency @ 44.1 kHz | Characteristics |
|---|---|---|
| **Zero Latency** | 0 | Matches the analog *magnitude* response. FabFilter's plot shows a small deviation above 18 kHz on a 15 kHz bell. The most efficient mode |
| **Natural Phase** | small; not documented (community tests of v2/v3 report about 330 samples, [gearspace](https://gearspace.com/board/music-computers/1048715-pro-q-2-zero-latency-vs-natural-phase.html), unverified) | Matches analog magnitude *and* phase closely, without noticeable pre-ring |
| Linear Phase, Low | 3072 samples (≈70 ms) | Low-Q or mid/high-frequency work only |
| Linear Phase, Medium | 5120 (≈116 ms) | The recommended general LP setting |
| Linear Phase, High | 9216 (≈209 ms) | High-Q low end. The **maximum for dynamic and spectral bands** |
| Linear Phase, Very High | 17408 (≈395 ms) | |
| Linear Phase, Maximum | 66560 (≈1509 ms) | Risk of pre-echo |

- **Doubled latency:** using L/R-specific and M/S-specific bands together in LP runs two stages, which doubles the
  latency.
- **Sample-rate scaling:** the latency in samples scales with the sample rate, so the latency in ms stays roughly
  constant.
- **No zipper noise:** LP frequency sweeps are smooth.
- **Low CPU:** usage stays low across the modes and doesn't change much between LP resolutions.

### 4.7 Character modes

([character mode](https://www.fabfilter.com/help/pro-q/using/character-mode))

- **Clean** is the default and transparent.
- **Subtle** is gentle, programme- and frequency-dependent saturation, described as transformer-style.
- **Warm** is more obvious, tube-style saturation.
- It is **global, with no amount control**. In LP mode its oversampling is linear phase too (4.02).

### 4.8 Output section

([output](https://www.fabfilter.com/help/pro-q/using/output))

- **Global Bypass:** latency-compensated with a soft (click-free) bypass. It also disables the analyzer.
- **Output options** (a hover panel that can be made "sticky" with a click):
  - **Phase Invert.**
  - **Auto Gain:** an *estimate* derived from the EQ settings. FabFilter says it doesn't measure the signal.
  - **Meter show/hide.**
  - **Gain Scale:** a horizontal slider shown in % (100% in the screenshots). It scales every gain-bearing band,
    handy for automating the overall amount; the range isn't documented.
  - **Output Gain:** −∞ … +36 dB.
  - **Output Pan:** a ring, in L/R or M/S mode.
- **Shortcut:** drag the output button vertically to change the gain or the gain scale directly.
- **Headroom:** unlimited internally; the meter warns only about clipping further down the chain.

### 4.9 Solo

([solo](https://www.fabfilter.com/help/pro-q/using/solo))

- **Entering solo:** click and hold the headphone icon in the band's popup.
- **Dragging while held:** horizontal moves the band's frequency, vertical sets the listening level, and Ctrl-drag
  changes Q.
- **What you hear:** the part of the spectrum the band affects. On LC/HC bands you hear what is being *removed*.

### 4.10 Piano display

([piano display](https://www.fabfilter.com/help/pro-q/using/pianodisplay))

- **Layout:** a toggle replaces the frequency axis with an 88-key keyboard (A0 27.5 Hz to C8 4186 Hz). Each band
  has a dot on the keys.
- **Quantizing:** click a dot to snap the band to the note; drag to move it in note steps.
- **Readouts:** show the note and cents. C4 is middle C.

### 4.11 Automation and smoothing

- **Automation:** sample-accurate for all parameters.
- **Smart Parameter Interpolation:** smooth transitions with no zipper noise.
- **Band numbers:** stable (see 4.1).
- **Undo:** automation and MIDI changes aren't recorded in the undo history
  ([undo](https://www.fabfilter.com/help/pro-q/using/undoredo)).

---

## 5. The interactive display in detail

([display and workflow](https://www.fabfilter.com/help/pro-q/using/eqdisplay), [manual p.9–12](https://www.fabfilter.com/downloads/pdf/help/ffproq4-manual.pdf))

### 5.1 What is drawn, back to front

1. **Grid:** decade lines and dB lines. Two scales: the yellow EQ scale and the grey analyzer/meter scale.
2. **Pre-EQ spectrum:** a dim grey filled area.
3. **Post-EQ spectrum:** a white or light line.
4. **External spectrum:** from the sidechain or any other Pro-Q 4 instance, with a light red outline.
5. **Collision glow:** red highlight where the post spectrum and the external spectrum overlap. There is an
   *accessible colours* option.
6. **Per-band curves:** thin lines, each in the band's own colour. The **selected** band's area is filled with its
   colour.
7. **Dynamic bands:** a filled region between static gain and gain plus range, a ▲▼ range handle, and the **live**
   dynamic response.
8. **Overall result curve:** thick yellow. For per-channel bands it is split into white (L), red (R), green (M) and
   blue (S).
9. **Band dots:** one colour each, with an M/L/R/S badge when placed. Dimmed when bypassed. For Free triggers, the
   dot has a vertical stem to a draggable trigger-range bar.
10. **Hover affordances:**
    - a ghost preview of the band that a click would create (dashed when bands are already selected);
    - the frequency under the cursor, shown on the axis;
    - the parameter popup near the band.
11. **Transient overlays:**
    - solo region shading;
    - Spectrum Grab tint (purple) with peak labels (Hz or note);
    - permanent-grab tint (blue);
    - the EQ Sketch pen glow.

### 5.2 Creating bands

**Ways to create a band:**
- Drag the yellow result curve up or down; a small preview of the shape appears as you start.
- Click or double-click the background, where a subtle preview shows exactly what will be created. When bands are
  already selected, a single click only deselects, so use double-click or Ctrl/Cmd-click instead.
- **Modifiers:** hold Alt to create a *dynamic* band, or Alt+Shift for a *spectral* band.

**Where you click decides the shape:**
- double-click at the very bottom of the display → **Notch**;
- double-click at the far left or far right → **Low Cut / High Cut**;
- drag the curve near the left or right end → **shelf**;
- anywhere else → **Bell**.

### 5.3 Selecting

- Click a dot or its coloured area.
- Drag a rectangle over the background (rubber band).
- Ctrl/Cmd-click adds a band to the selection; Shift-click selects a consecutive range.
- Click the background to deselect all.

### 5.4 Editing gestures (complete list from the manual)

| Gesture | Effect |
|---|---|
| Drag a selected dot | Frequency and gain for **all selected** bands. With several bands selected, gains scale *relative to each other* |
| Drag the ▲▼ range indicator | Dynamic range |
| Wheel over a curve, or while dragging | Q. For LC/HC it changes the slope in steps; Shift gives fractional values |
| Ctrl/Cmd + vertical drag | Q |
| Alt + wheel | Dynamic range |
| Ctrl/Cmd + wheel | Gain |
| Alt + Ctrl/Cmd + wheel | Trade gain for dynamic range (reverse-linked) |
| Shift + drag or wheel | Fine adjustment |
| Alt + drag | Lock to one axis: horizontal for frequency, vertical for gain (or Q with Ctrl) |
| Alt-click a dot | Toggle band bypass |
| Ctrl/Cmd + Alt-click a dot | Cycle the shape |
| Alt + Shift-click a dot | Cycle the slope |
| Double-click a dot | Type values; Tab moves through Freq → Gain → Q |
| Double-click a value in the popup | Type that value |
| Drag or wheel on a value in the popup | Change it directly (new in v4) |
| Right-click a dot | Band menu: settings, Copy, Make Dynamic, Make Spectral, Reset Placement/Speakers… |
| Right-click the background | Paste bands, or Copy all bands |
| Drag the frequency axis vertically | Zoom in or out around the clicked frequency |
| Drag the frequency axis horizontally (while zoomed) | Pan |
| Double-click the frequency axis | Reset the zoom |
| Range dropdown (top right) | ±3, 6, 12 or 30 dB. The range auto-expands when a drag goes past it; this can be turned off |

**The parameter popup** shows exact values. It also has:
- bypass, solo, delete and shape buttons;
- a slope menu (LC/HC only; since March 2026 clicking it opens the menu);
- a band-menu triangle.

It can be hidden with *Help → Show EQ Parameter Display*.

### 5.5 EQ Sketch

([EQ Sketch](https://www.fabfilter.com/help/pro-q/using/eq-sketch))

- **Starting a sketch:** in an empty instance, just press and draw left to right. Once bands exist, use the Sketch
  button (bottom left).
- **What gets created:**
  - A new band starts when the pen leaves the 0 dB line far enough. Its type depends on the position.
  - The **steepness of the stroke** sets the slope (for cuts) or the Q (other shapes).
  - Returning close to 0 dB finalises that band.
- **Correcting a sketch:** moving *back* (right to left) during the same stroke removes the bands just added. Undo
  removes the whole sketch.
- **Scope:** it is meant for a rough starting curve, not for precision.
- **Preferences:** both the hover previews and auto-sketch can be disabled with *Help → Auto-EQ Sketch*.

### 5.6 Spectrum Grab

([Spectrum Grab](https://www.fabfilter.com/help/pro-q/using/spectrumgrab))

- **Entering:** hover over the spectrum for a few seconds. The spectrum freezes and the bands dim.
- **Grabbing:** the main peaks get Hz or note labels. Grab one and drag it to create a **Bell** with an automatically
  chosen Q.
- **Permanent grab:** click and hold until the tint turns blue, then grab several peaks. Click the background to
  exit.
- **Best with the post spectrum.**
- **Turning it off:** it can be disabled in the analyzer panel.

### 5.7 Knobs

([knobs](https://www.fabfilter.com/help/pro-q/using/knobs))

- **Adjusting:**
  - vertical drag, *speed-sensitive*;
  - mouse wheel;
  - double-click to type.
- **Modifiers:**
  - Ctrl/Cmd-click resets (Alt-click in Pro Tools);
  - Shift for fine adjustment (Ctrl/Cmd in Pro Tools);
  - Alt-drag moves linked knobs, e.g. trading gain against range.
- **Typing shortcuts:**
  - `1k`
  - `A4` or `C#3+13`
  - `2x` for +6 dB
  - `50%` for the middle of the knob's range
- **Hover:** the knob lights up and shows a value balloon.

### 5.8 Keyboard

The keyboard is thin. What exists:
- Tab while typing values;
- Esc to leave full screen, the instance list or the preset browser;
- arrows and Enter in the preset browser, with the right arrow loading without closing;
- `[` and `]` for the previous and next preset;
- type-to-search in presets;
- type-to-filter or type-to-quick-jump in the instance list.

There are no documented shortcuts for band selection, nudging, delete, solo or bypass.

### 5.9 Touch (iPad AUv3)

([iOS](https://www.fabfilter.com/help/pro-q/using/ios))

- **Knobs:** drag. Hold a second finger for fine adjustment. Double-tap resets and long-press types a value.
- **Display:**
  - drag the curve or double-tap to create a band;
  - drag dots to move them, and drag a rectangle to select several;
  - double-tap and drag changes Q;
  - long-press a dot for the band menu;
  - long-press a value to type it;
  - long-press the spectrum for Spectrum Grab.
- **Size:** set by the host; there is no resizing in the plug-in.

### 5.10 Display preferences (Help menu)

- Show EQ Parameter Display
- Auto-EQ Sketch (the curve previews)
- Auto-Adjust Display Range
- Show Frequency On Hover
- Use Accessible Colors

---

## 6. Analyzer and metering

([analyzer](https://www.fabfilter.com/help/pro-q/using/analyzer))

| Setting | Values | Notes |
|---|---|---|
| Sources | **Pre**, **Post**, **SC/Ext** | Ext can be the sidechain *or the post spectrum of any other Pro-Q 4 instance*, picked from a list of track names |
| Range | 60 / **90 (default)** / 120 dB | |
| Resolution | Low 1024, Medium 2048, High 4096, Maximum 8192 points (FFT) | Higher resolution gives a slower update and attack. Use High or Maximum for low-frequency peaks and collisions |
| Speed | release speed, from fast to slow (named steps; "Medium" in screenshots) | Full list not documented |
| Tilt | dB/oct around 1 kHz, **default 4.5** | Meant to look like perceived loudness |
| Freeze | toggle; *hold* to freeze temporarily | Builds a maximum over time. A blue line on the Analyzer button shows it is active |
| Spectrum Grab | toggle | See 5.6 |
| Show Collisions | toggle | A red glow where the spectrum collides with the selected external spectrum. FabFilter calls it an indication, "not exact science" |

- **Settings persistence:** analyzer settings are saved in the song but aren't changed by presets.
- **Panel behaviour:** the panel opens on hover and can be made sticky.
- **Output meter:** a large meter with peak readouts, labelled per speaker in surround. There is **no loudness
  (LUFS) metering, no correlation or phase meter, no spectrogram and no phase-response display**.

---

## 7. Workflow features

**Presets** ([loading](https://www.fabfilter.com/help/pro-q/presets/loadingpresets), [saving](https://www.fabfilter.com/help/pro-q/presets/savingpresets), [storage](https://www.fabfilter.com/help/pro-q/presets/howpresetsarestored))
- **Files:** `.ffp` files in `Documents/FabFilter/Presets/Pro-Q 4`, with folders as categories.
- **Browsing:**
  - the browser opens folders on hover;
  - metadata: author, tags, description;
  - favourites and type-to-search;
  - ‹ › stepping without opening the browser.
- **Preset management:**
  - MIDI program change loading (bank/program numbers are assigned automatically);
  - *Save As Default* sets the start-up state;
  - Open Other, Change Folder, Restore Factory, Refresh;
  - drag and drop of preset files (March 2026).
- **Right-click the preset button** for Favourite, Save, Save As, Copy and Paste.
- **Control surfaces:** a "Flat 7 Bands" preset exists for Pro Tools control surfaces, because they need a fixed
  band count.

**Undo, redo, A/B** ([undo](https://www.fabfilter.com/help/pro-q/using/undoredo))
- **Undo:** a linear history of UI changes. There is no visible history list and no undo for automation or MIDI
  changes.
- **A/B:** two states, plus Copy (A→B), which greys out when both are equal.

**Copy and paste**
- **Bands:** copy and paste works between any instances, through the band menu or the background menu.
- **Full state:** copy and paste of the whole state through the preset menu.

**MIDI Learn** ([MIDI learn](https://www.fabfilter.com/help/pro-q/using/midilearn))
- **Learning:** click a parameter, then move a controller.
- **Target modes:** bind to a *specific band* (e.g. Band 3 Gain), or to the **Active Band**, so a small controller
  follows whichever band is selected. The Active Band mode needs the UI to be open.
- **Buttons:** Prev/Next/Delete band can be triggered by CC value 127.
- **Menu:** Enable MIDI, Clear (per mapping or all), Revert, Save.

**Resize and full screen** ([full screen](https://www.fabfilter.com/help/pro-q/using/fullscreenandresize))
- **Sizes:** Mini, Small, **Medium (default)**, Large, Extra Large. A Scaling submenu offers, for example, 150% or
  300% on Retina displays.
- **Full screen:** scales the controls up automatically.
- **Memory:** the scaling is remembered separately for normal and full-screen mode, and per monitor DPI class.
- **Free resize:** in VST3 only.
- **Rendering:** GPU-accelerated.

**Instance list** ([instance list](https://www.fabfilter.com/help/pro-q/using/instance-list))
- **Scope:** every Pro-Q 4 instance in the session, plus Pro-C 3, Pro-DS and Pro-G since 4.10, grouped by track in
  DAW order.
- **Navigation:**
  - a zoom slider, from spectra only up to nearly full editing, including Sketch;
  - **auto-zoom** on hover;
  - a type-to-filter field and a **Quick Jump** search;
  - **pin** tracks (Shift-click for a range, Alt-click to pin only one);
  - *Filter Pinned*;
  - saved **pinned sets**;
  - a **minimap**.
- **Per instance:**
  - bypass, output settings, and an **Emphasize** button that raises the level for a moment;
  - maximize;
  - a menu with presets, copy/paste and EQ Match;
  - *Pin Similar Colors*;
  - rename the track label.
- **Collision reference:** any track can be made the collision reference.
- **Drag and drop:** drop a preset file to load it, or drop an audio file to start EQ Match with it as the reference.
- **Caveats depend on the DAW:**
  - AU gets no track order or colour, so tracks are sorted by name;
  - Pro Tools reports positions only from 2025.12, and colours not at all;
  - FL Studio may misorder instances when some of them add latency;
  - the order is refreshed on stop and start.
- **Editing limits:** there is no band panel in the list, so advanced dynamics, such as the sidechain setup, can't
  be edited there ([pluginoise](https://pluginoise.com/fabfilter-pro-q-4-review/)).

**EQ Match** ([EQ Match](https://www.fabfilter.com/help/pro-q/using/eqmatch))
- **Access:** only through an instance's menu in the instance list.
- **Reference source:**
  - a saved reference spectrum;
  - the input recorded at a different time, with record and pause for each side;
  - the output of another Pro-Q 4 instance;
  - the sidechain;
  - an audio file (Load File or drag and drop).
- **Analysis:** averages over time, usually in under 30 seconds, and shows a thick white difference curve.
- **Match:** a **number-of-bands** slider sets the detail (the default is chosen automatically). *Finish* commits
  the bands.
- **Resolution:** the same as the analyzer.
- **Missing:** amount/percentage, smoothing and boost/cut limits.

**External sidechain** ([side-chaining](https://www.fabfilter.com/help/pro-q/support/externalsidechaining))
- **One sidechain input:** used by dynamic and spectral triggers, EQ Match and the analyzer.

---

## 8. What reviewers and users say

**Praise**
- *Sound on Sound* ([review](https://www.soundonsound.com/reviews/fabfilter-pro-q-4)):
  - spectral EQ is the reviewer's favourite addition, one he "never want[s] to return" from;
  - scaling and the instance list make a great UI better;
  - verdict: he "can't fault it".
- *MusicTech*, 8/10 ([review](https://musictech.com/reviews/plug-ins/fabfilter-pro-q-4-review/)): big workflow
  gains, per-band spectral processing, better dynamics controls, convincing analog modelling, and a gentle learning
  curve.
- *Production Expert* gave it its Gold award ([review](https://www.production-expert.com/production-expert-1/fabfilter-pro-q-4-expert-review)):
  the CPU is still very light (hundreds of instances), and attack and release on dynamics are welcome.
- *Tape Op* ([review](https://tapeop.com/reviews/gear/166/fabfilter-pro-q-4-plug-in)): spectral mode adapts to
  resonances that shift across chords, character modes "add up nicely" across a mix, and continuous slopes are
  praised.
- **KVR users** rate it 5/5 ([reviews](https://www.kvraudio.com/product/pro-q-4-by-fabfilter/reviews)). Collision
  detection is called "the ultimate weapon", along with the analyzer, M/S and L/R, the GUI, and transparency.
- **Across all sources:** the interface itself is the main reason people choose it.

**Complaints**
- **Spectral latency and CPU.** Spectral bands force linear-phase latency even in "Zero Latency" mode.
  - Users report 5120 samples and about 90–128 ms per instance, and the UI doesn't make this obvious
    ([FabFilter forum](https://prod.fabfilter.com/forum/topic/7873/anyone-getting-super-long-latency-pro-q-4)).
  - Host delay compensation then **delays the whole project**, including unrelated MIDI tracks
    ([FabFilter forum](https://prod.fabfilter.com/forum/topic/7940/project-wide-latency-caused-by-pro-q-4)).
  - Users measured about 4% CPU in Low and 12% in High per spectral instance, and consider it heavier than soothe2
    ([KVR forum](https://www.kvraudio.com/forum/viewtopic.php?t=616754&start=270)). SOS also notes "significant
    latency".
- **Spectral isn't as deep as the dedicated tools.**
  - SOS would still use soothe on full mixes, and MusicTech says it won't replace soothe2 or Gullfoss.
  - Some describe the processing as tied statically to the band, rather than following the material.
- **Redesign legibility.**
  - Production Expert: the knobs show "less visual feedback on their position" than v3, and the design "favoured
    form a little too much over function".
  - KVR users: the UI is darker, the gain knob looks like it is "wearing camouflage", and the multi-view doesn't
    use the space well.
- **Instance list gaps.**
  - It can't be reordered (SOS).
  - There is no grouped or linked control of several instances; FabFilter says it has "considered this"
    ([forum](https://prod.fabfilter.com/forum/topic/7911/pro-q4-instance-list-missing-feature)).
  - Pinned sets aren't saved per project, and advanced dynamics can't be edited in the list
    ([Gearspace thread](https://gearspace.com/board/new-product-alert/1439778-fabfilter-releases-pro-q-4-eq-plug-13.html),
    via search summary because the page returns 403 to fetchers; [pluginoise](https://pluginoise.com/fabfilter-pro-q-4-review/)).
- **Character modes** are barely audible alone and offer no control. SOS would rather use a dedicated saturator and
  wants a way to switch all instances at once.
- **EQ Sketch** takes practice and is rarely faster than normal band creation (SOS,
  [pluginoise](https://pluginoise.com/fabfilter-pro-q-4-review/)).
- **Feature requests seen:**
  - a **dB readout at the cursor or peak** (Gearspace);
  - **frequency sweep with auto-listen** (Gearspace);
  - **MIDI note follow** ([forum topic](https://www.fabfilter.com/forum/topic/8123/pro-q-4-feature-request-midi-note-follow));
  - **inverse linking of two nodes** and **"follow EQ" that tracks a resonance** like iZotope's
    ([Ideas for Pro-Q 4](https://prod.fabfilter.com/forum/add-comment?id=6536), via search summary).
- **Price** ($179, $89 upgrade) is called steep for hobbyists (MusicTech).

---

## 9. Competitors: what they have that Pro-Q 4 lacks

| Product | Features Pro-Q 4 lacks |
|---|---|
| **Three-Body Technology Kirchhoff-EQ** ([product](https://www.threebodytech.com/en/products/kirchhoffeq)) | **32 bands.** 15 continuously variable shapes, including **Flat Top** and a resonance-extracting **"Sword"** filter. **32 vintage EQ curve models** from 9 hardware units. **Nyquist-matched** filters with no HF cramping, and 64-bit processing with optional **117-bit "double-double"** precision. **Mixed-phase** mode (zero latency in the lows, linear in the highs) and an Analogue mode (63 samples). **Dynamics:** dual thresholds (above and below), relative detection, onset (transient) detection, **lookahead**, upward and downward modes. Per-band detector frequency and Q. **2× oversampling.** Global width control. |
| **oeksound soothe3** (19 May 2026) ([product](https://oeksound.com/plugins/soothe3), [SOS news](https://www.soundonsound.com/news/oeksound-introduce-soothe3)) | The benchmark for spectral work. **A zero-latency mode** (0 samples at base rates, about 1 ms at high rates), where Pro-Q's spectral mode needs at least 70 ms. Soft (adaptive threshold) and Hard modes. A single **Detail** control. Tilt and **Max Cut** limits. Sidechain between tracks, a linear-phase option, 9.1.6. |
| **sonible smart:EQ 4** ([product](https://www.sonible.com/smarteq4/)) | **Cross-track unmasking:** up to 10 instances in a group talk to each other, with Track, Group or Track+Group roles and a drag-and-drop hierarchy. AI **target profiles** (genre or reference track). Remote control of the whole group. |
| **Eventide SplitEQ** ([product](https://www.eventideaudio.com/plug-ins/spliteq/)) | **Transient and tonal split per band**, each with its own level, solo and mute. Per-band L/R pan or M/S width. An analyzer that shows the transient and tonal streams separately. |
| **DMG Audio EQuilibrium** ([product](https://dmgaudio.com/products_equilibrium.php), [SOS](https://www.soundonsound.com/reviews/dmg-audio-equilibrium)) | User-designed **phase response**: minimum, analogue, zero-latency analogue, linear or "free", with FIR **window shape** (12 types) and **impulse length**. A library of classic-console curves. |
| **TDR Nova / Nova GE** ([Nova GE manual](https://docs.tokyodawn.net/nova-ge-manual/)) | Real **threshold and ratio** per band. The **sidechain band can be unlinked** with its own frequency and Q. Equal-loudness workflow and frequency-dependent ratio (GE). Nova is free, so it sets the bar for "good enough". |
| **MeldaProduction MAutoDynamicEq** ([product](https://www.meldaproduction.com/MAutoDynamicEq)) | **Pitch tracking** (bands follow the detected fundamental or harmonics), MIDI note control, and LFO, envelope or random modulators on any parameter. |

---

## 10. Gaps in Pro-Q 4, in one list

- **Spectral latency:** spectral bands always run linear phase, with at least 3072 samples. There is no low-latency
  spectral option, and the latency is easy to miss in the UI.
- **Dynamics:** a relative-% attack and release with no ms values, no ratio, knee or lookahead, one threshold
  direction, and a single external sidechain shared by all bands.
- **Dynamics in LP:** they stop working above the High LP resolution. L/R and M/S bands together double the LP
  latency.
- **Auto Gain** is a curve-based estimate, not a measured loudness match.
- **Undo:** there is no history list. Automation and MIDI changes aren't undoable. There are only two A/B slots
  and no morph between them.
- **Band linking:** there is no linking or grouping of bands within an instance (inverse linking) or across
  instances.
- **Musical awareness:** there is no MIDI note or pitch follow, and no resonance tracking.
- **Analyzer:** FFT only, with a speed–resolution trade-off. There is no spectrogram or waterfall, no numeric dB
  readout at the cursor or peaks, no phase or group-delay view, and no LUFS or correlation metering.
- **Collision detection** compares against *one* reference, and is only an indication.
- **EQ Match** is hidden inside the instance list, and has no amount, smoothing or boost/cut limits.
- **Instance list:** it depends on the DAW (order, colour and project membership often unknown). It can't be
  reordered, pinned sets aren't saved per project, and there is no band panel.
- **Character** is global, with no amount and no per-band option.
- **Keyboard:** almost no keyboard workflow.
- **Hidden state:** hover-only panels (analyzer, output) hide their state, and important preferences live in the
  Help menu.
- **Dynamic shapes:** dynamics are available only on the bell, shelf and tilt shapes. There is no dynamic cut
  frequency.

---

## 11. How we beat it

The priorities are ordered by impact per effort, given where Cantus is today. Cantus has:
- a 3-band RBJ channel EQ (`cantus_dsp::eq`);
- deterministic, null-tested DSP;
- project-level undo with fuzz tests;
- an egui UI with a light default theme and a user-editable palette;
- en and es locales.

### P0: parity foundation (without these we are not credible)

1. **Filters matched to analog up to Nyquist.**
   - RBJ bells and shelves cramp near Nyquist; FabFilter's own plot shows plain digital EQ going wrong above
     about 15 kHz. Use matched designs instead: Vicanek-style magnitude-matched biquads, or Orfanidis prescribed
     Nyquist gain.
   - Cuts: cascaded sections with **continuous, fractional slope** from 0 to 96 dB/oct, plus a brickwall option.
   - Keep the rules Cantus already has: 0 dB is bit-exact identity, results don't depend on block size, and there
     are null tests.
   - *Beat it:* publish the magnitude and phase error against the analog prototype as a test artefact, not a
     marketing plot.
2. **All ten Pro-Q shapes** (Bell, Low and High Shelf, Low and High Cut, Notch, Band Pass, Tilt Shelf, Flat Tilt,
   All Pass), plus **Flat Top** (Kirchhoff). At least 24 bands; aim for 32. **Stable band IDs** that never renumber,
   because automation depends on them. Pro-Q gets this right, so copy it.
3. **The display grammar Pro-Q users expect:**
   - drag the curve to create a band, with the shape chosen by position;
   - a ghost preview on hover;
   - rubber-band selection;
   - relative multi-band drag;
   - wheel for Q, Alt for axis lock, Shift for fine;
   - double-click to type, accepting `2k`, `A4`, `C#2+13` and `2x`;
   - drag the axis to zoom;
   - an auto-expanding dB range;
   - a floating band panel under the selection;
   - dynamic controls that appear only when used.

   Users' muscle memory is an asset; don't fight it.
4. **Dynamic EQ with per-band L/R/M/S** and a band-limited detector (Band or Free).
5. **An analyzer** with pre, post and external sources; freeze; tilt; Spectrum Grab.
6. **Undo in the Cantus project history.**
   - Every gesture is one command: a drag commits once, and wheel bursts are merged.
   - Covered by the existing undo-fuzz tests.
   - A and B snapshots.

### P1: differentiators, where we are clearly better

1. **The mixer *is* the instance list, because we are the host.**
   - Every channel EQ in one zoomable view, in real track order, colours and groups. No DAW-API caveats.
   - **Reorderable**, with **pinned sets saved in the project**.
   - The full band panel available in the list.
   - **Linked editing across tracks**: select bands on three vocals and drag them together, with *inverse linking*
     as an option. FabFilter admits it hasn't built this.
   - Tie it to Cantus's mixer strips and docks (ADR 0014 and 0016), so opening it doesn't require opening plug-in
     windows.
2. **A masking matrix instead of one collision reference.**
   - Since every track's post spectrum is available for free, show *who masks whom*: a heat strip per track and a
     click-through pair view.
   - Offer an **unmask** action that proposes complementary cut and boost bands on the two tracks (smart:EQ's group
     idea, with the result shown as ordinary editable bands, not a black box).
3. **Spectral dynamics that are honest about latency.**
   - **Low-latency spectral mode:** a dense, minimum-phase dynamic filterbank of 1/6 to 1/12 octave bells driven by
     band-limited detectors, with zero or near-zero latency. soothe3 proved in 2026 that users want exactly this.
   - **Linear-phase STFT mode** for mastering, as a second option.
   - Show a **latency badge on the band** and a total in the bottom bar. Warn *before* enabling something that
     changes the project's delay compensation.
   - Add **Max Cut** and **Detail/Density** controls, and a **delta listen** that plays only what is removed.
4. **Dynamics you can read and set.**
   - Keep Pro-Q-style **Auto**, but expose **attack and release in ms**, **ratio**, **knee**, **lookahead**, and
     **both directions** (above and below a threshold, as Kirchhoff does).
   - Draw the **threshold on the band** and a scrolling **gain-change history ribbon** on the display, so the user
     *sees* what the band did over the last few seconds.
   - Allow a **per-band sidechain source from any track** without routing; only a host can do this cheaply.
   - Offer dynamics on **cut filters** too, as a dynamic HPF frequency.
5. **Legibility first.**
   - Values always readable: an optional label on each dot; arcs with strong contrast in both themes.
   - A **cursor crosshair readout** showing frequency, note and cents, *spectrum level in dB*, and *curve gain in
     dB* (a Gearspace request).
   - **Peak labels in dB** as well as Hz.
   - A colourblind-safe L/R/M/S palette that also marks placement with **glyphs**, not colour alone.
   - Everything follows Cantus's editable theme.
   - No hover-only panels: state lives in visible toggles.
6. **One "fit" engine behind Sketch, Match and Grab.**
   - The user draws, imports or grabs a *target curve*. The engine fits bands live under a **band-budget slider**
     (Pro-Q already has this for Match but not for Sketch).
   - The target stays **editable** after drawing, so there is no "draw it again".
   - **Match from any track, clip, bus or file** in the project, **one click from the main view**, with amount %,
     smoothing, max boost and max cut, and the **residual error** shown.
   - This turns Sketch, which reviewers call hard, into something forgiving.
7. **Keyboard-first editing.**
   - Number keys select bands; arrows nudge frequency (semitones with Shift) and gain.
   - `[` and `]` change Q; single keys toggle dynamic, spectral, solo and bypass, and delete the band; Tab cycles
     bands; a key sets L/R/M/S placement.
   - A searchable command list, with the shortcuts shown in tooltips.
   - Localised en and es like the rest of Cantus.
8. **Fair comparisons.**
   - **Measured loudness-matched bypass and A/B** (short-term LUFS, gated), instead of a curve estimate.
   - **A–D snapshots with a morph slider** that can be automated.
   - Enforce level matching in *listen* comparisons so "louder = better" can't mislead.
9. **Musical awareness from the host.**
   - A band's **frequency can follow the track's MIDI notes**: its own piano-roll clips or another track, with
     fundamental or n-th harmonic.
   - A **scale-aware quantise** that uses the project key.
   - A **"sweep and listen"** helper for hunting resonances, with a narrow boosted band and auto-solo.
   - Pro-Q users have asked for these; we have the data natively.

### P2: later, once the core wins

- **Spectrogram and waterfall** view toggles. A **multi-resolution (constant-Q) analyzer**, so the low end gets
  resolution without Pro-Q's 8192-point sluggishness. A **phase and group-delay** overlay.
- **Per-band saturation or character** with an amount control, plus a global *mix-into-it* console mode that a
  single command switches on for all tracks (SOS's request).
- **Transient and tonal split per band** (SplitEQ's idea).
- **Mixed-phase** mode (Kirchhoff): minimum phase in the lows and linear in the highs.
- **Oversampling** option; vintage curve models; surround up to 9.1.6, once Cantus's buses support it.

### UX principles to keep from Pro-Q, and what not to copy

- **Keep:**
  - the display as the whole instrument;
  - a panel that follows the selection;
  - progressive disclosure;
  - drag or wheel on any readout;
  - note-name entry;
  - stable band numbers;
  - smooth parameter interpolation;
  - unlimited headroom;
  - Esc always backs out.
- **Avoid:**
  - hover-only panels that hide state;
  - latency that appears silently;
  - "50% = auto" style units;
  - key features buried in the Help menu or inside another view (EQ Match);
  - a dark, low-contrast control styling that sacrifices readability for mood.

---

## 12. Sources

- **FabFilter, official**
  - Product page: https://www.fabfilter.com/products/pro-q-4-equalizer-plug-in
  - PDF manual: https://www.fabfilter.com/downloads/pdf/help/ffproq4-manual.pdf
  - Online help index: https://www.fabfilter.com/help/pro-q. Pages used:
    - `using/`: `overview`, `eqdisplay`, `bandcontrols`, `dynamic-eq`, `spectral-dynamics`, `character-mode`,
      `processingmode`, `analyzer`, `spectrumgrab`, `eqmatch`, `eq-sketch`, `instance-list`, `pianodisplay`,
      `solo`, `stereo`, `surround`, `output`, `fullscreenandresize`, `knobs`, `midilearn`, `undoredo`, `ios`
    - `presets/`: `loadingpresets`, `savingpresets`, `howpresetsarestored`
    - `support/`: `upgrading`, `externalsidechaining`
- **FabFilter news**
  - Release: https://www.fabfilter.com/news/1733994000/fabfilter-releases-pro-q-4-equalizer-plug-in
  - 4.01: https://www.fabfilter.com/news/1734440400/fabfilter-pro-q-401-update-released
  - 4.02: https://www.fabfilter.com/news/1740654000/fabfilter-pro-q-402-update-released
  - 4.10: https://www.fabfilter.com/news/1768467600/fabfilter-introduces-multi-plugin-instance-list-in-pro-q-410
  - Mar 2026: https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes
  - Apr 2026: https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins
  - 4.13: https://www.fabfilter.com/news/1782806400/pro-q-413-released
- **Reviews**
  - Sound on Sound: https://www.soundonsound.com/reviews/fabfilter-pro-q-4
  - MusicTech: https://musictech.com/reviews/plug-ins/fabfilter-pro-q-4-review/
  - Production Expert: https://www.production-expert.com/production-expert-1/fabfilter-pro-q-4-expert-review
  - Tape Op: https://tapeop.com/reviews/gear/166/fabfilter-pro-q-4-plug-in
  - pluginoise: https://pluginoise.com/fabfilter-pro-q-4-review/
  - KVR user reviews: https://www.kvraudio.com/product/pro-q-4-by-fabfilter/reviews
- **Forums**
  - KVR: https://www.kvraudio.com/forum/viewtopic.php?t=616754&start=270
  - Gearspace: https://gearspace.com/board/new-product-alert/1439778-fabfilter-releases-pro-q-4-eq-plug-13.html
  - FabFilter forum, latency: https://prod.fabfilter.com/forum/topic/7873/anyone-getting-super-long-latency-pro-q-4
  - FabFilter forum, project-wide latency: https://prod.fabfilter.com/forum/topic/7940/project-wide-latency-caused-by-pro-q-4
  - FabFilter forum, instance-list grouping: https://prod.fabfilter.com/forum/topic/7911/pro-q4-instance-list-missing-feature
  - FabFilter forum, MIDI note follow: https://www.fabfilter.com/forum/topic/8123/pro-q-4-feature-request-midi-note-follow
  - FabFilter forum, Ideas for Pro-Q 4: https://prod.fabfilter.com/forum/add-comment?id=6536
  - Natural Phase latency (community test): https://gearspace.com/board/music-computers/1048715-pro-q-2-zero-latency-vs-natural-phase.html
  - Q range (Pro-Q 3 review): https://www.musicmanta.com/reviews/fabfilter-pro-q-3-in-depth-review
- **Competitors**
  - Kirchhoff-EQ: https://www.threebodytech.com/en/products/kirchhoffeq
  - soothe3: https://oeksound.com/plugins/soothe3 and https://www.soundonsound.com/news/oeksound-introduce-soothe3
  - smart:EQ 4: https://www.sonible.com/smarteq4/
  - SplitEQ: https://www.eventideaudio.com/plug-ins/spliteq/
  - EQuilibrium: https://dmgaudio.com/products_equilibrium.php and https://www.soundonsound.com/reviews/dmg-audio-equilibrium
  - TDR Nova GE: https://docs.tokyodawn.net/nova-ge-manual/
  - MAutoDynamicEq: https://www.meldaproduction.com/MAutoDynamicEq
