# FabFilter as a family: the shared interface language and the bundles

*Researched 2026-09-23 for Cantus. Sources are FabFilter's own pages: product pages, the online help, the PDF
manuals, the download page, news posts and press releases, and the shop, read through a fetch service because
the shop is blocked for direct download here. Press reviews and user forums were used for criticism. Everything
is paraphrased. Colour values were **measured from FabFilter's official screenshots** (JPEG, so ±2 per channel).
Where the documentation is silent, the text says so.*

*This report covers the family as a whole. Product detail lives in the sibling reports:
[pro-q.md](pro-q.md), [pro-c.md](pro-c.md), [pro-l.md](pro-l.md) and [pro-r.md](pro-r.md), plus Pro-MB, Pro-DS and
Pro-G where they exist. The screenshots are FabFilter's and are kept in [`screenshots/`](screenshots/) for internal
reference only.*

---

## 0. The short version

1. **The family is held together by a frame, not by a look.** Every FabFilter plug-in has the same top bar
   (logo tab, undo/redo, A/B + Copy, a preset field with ‹ ›, Help, full screen) and the same bottom bar (MIDI
   Learn at the left, global options in the middle, bypass, output and resize at the right). The same
   knob gestures, text-entry shortcuts, preset files, MIDI Learn and resize menu work everywhere. The *look*
   between these fixed points spans three visual generations inside one bundle
   ([family-01](screenshots/family-01-lineup-side-by-side.jpg), [family-02](screenshots/family-02-top-and-bottom-bars.png)).
2. **Each product has its own colour.** Pro-Q and Pro-C are yellow, Pro-L red on lilac, Pro-R amber on navy,
   Pro-DS lime, Pro-G blue, Saturn crimson and Timeless green. Every product still sits on near-black neutrals
   with white text.
3. **Its best ideas are about direct manipulation:** the display *is* the editor; a value is shown when you hover;
   dragging from a source onto a knob creates modulation; modulation is drawn live on the knobs.
4. **Its weak spots are legibility and hidden state.** The UI is dark-only. Knob bodies measure **1.1–1.2:1** against
   their background (WCAG asks 3:1 for control boundaries). Secondary labels measure about **2.5:1**. Important
   panels only appear on hover. Users have complained about text size (partly fixed by scaling in 2023) and about
   darker redesigns ("camouflage" knobs in Pro-Q 4; Pro-R 2 "too dark").
5. **FabFilter keeps hitting the plug-in wall.** The Pro-Q 4 instance list has to guess track order, names and
   colours through format APIs that are often missing. MIDI Learn needs per-host MIDI routing to an effect. Each
   plug-in keeps its own undo history, separate from the host's. **Cantus is the host, so none of this applies
   to us.**
6. **Saturn, Timeless, Volcano and Twin share one modulation system:** drag a source onto any knob; slots have a
   level, an invert and a bypass; slots can modulate slots; modulation is drawn around the knobs. It is the right
   model for our instruments. Its gaps: no global matrix view, colour by source *type* rather than by instance,
   and depth that is only readable on hover.
7. **Our family language should keep FabFilter's frame and gestures, and fix its legibility.** Light and dark
   themes from the app palette, AA contrast on every control, values readable without hovering, monospaced
   numbers, the host's undo, automation and controller mapping, and one analyser shared across strips.
   Section 7 has the component list and the rules.

---

## 1. Screenshots saved

All in [`screenshots/`](screenshots/). Checked: correct JPEG or PNG signature, all larger than 12 KB, all viewed.

| File | What it shows |
|---|---|
| `family-01-lineup-side-by-side.jpg` | **Composite** of nine official main screens (Pro-Q 4, Pro-C 3, Pro-L 2, Pro-R 2, Pro-MB, Pro-DS, Pro-G, Saturn 2, Timeless 3), window chrome cropped. The colour identities and the three visual generations side by side. |
| `family-02-top-and-bottom-bars.png` | **Composite**: the top bar and the bottom bar of eight products, cropped from official screenshots and scaled to one width. It shows how fixed the frame is. |
| `family-03-saturn2-main.jpg` | Saturn 2 main screen (1920 px): multiband display, floating band panel, modulation strip, source flow glow. |
| `family-04-timeless3-main.jpg` | Timeless 3 main screen (2160 px): delay taps display, filter display, effect knobs, modulation strip. |
| `family-05-knob-classic-anatomy-saturn2.jpg` | Official annotated diagram of the classic knob: the "arrows" on the rim and the "center area". |
| `family-06-knob-modern-proc3.jpg` | The modern knob (Pro-C 3): dark disc, accent arc and tip dot, AUTO label state, auto-threshold ring, wave mini-sliders. |
| `family-07-knob-modulation-ring-saturn2.jpg` | Modulation on a knob: the red L/R ring, the yellow target-level arc and the source-colour dots next to the label. |
| `family-08-modulation-slot-panel-saturn2.jpg` | The floating modulation slot panel, including a slot that modulates another slot (indented). |
| `family-09-preset-browser-proc3.jpg` | The new preset browser: search, gear, favourites star, folder column, details panel, Copy/Paste, Save As. |
| `family-10-output-options-popup-proc3.jpg` | The output panel that appears on hover in the bottom bar: in and out level/pan knobs, mix bar, values. |
| `family-11-resize-menu-proc3.jpg` | The resize menu (Compact/Small/Medium/Large plus a Scaling submenu). It is an OS-native light menu. |
| `family-12-midi-learn-saturn2.jpg` | MIDI Learn mode: the UI dims, a banner names the target, a red square marks it and balloons show CC numbers. |
| `family-13-help-hint-timeless3.jpg` | An "interactive help hint": a large light speech bubble anchored to the Help button, explaining the hovered control. |

The sibling reports already hold the Pro-Q 4 parameter popup (`pro-q-03`), the Pro-Q preset browser with tags
(`pro-q-15`) and the instance list (`pro-q-08`, `pro-c-12`).

---

## 2. What is in the bundles (September 2026)

### 2.1 Bundles

Read from `fabfilter.com/shop/<bundle>` (EUR prices include VAT, as shown to a European visitor). The shop's
bundle menu lists **Total, FX, Pro, Mixing, Mastering, Essentials, Creative** and a **Custom** bundle (build your
own from your account). There is no "Producer" bundle any more.

| Bundle | Contents | Price (EUR) | Stated value |
|---|---|---|---|
| **Pro** | Pro-Q 4, Pro-C 3, Pro-L 2, Pro-R 2, Pro-MB, Pro-DS, Pro-G | 659 | 1 183 |
| **Mixing** | Pro-Q 4, Pro-C 3, Pro-R 2, Pro-DS, Pro-G, Saturn 2, Timeless 3 | 619 | 1 103 |
| **Mastering** | Pro-Q 4, Pro-C 3, Pro-L 2, Pro-MB | 439 | 676 |
| **Essentials** | Pro-Q 4, Pro-C 3, Pro-R 2 | 359 | 507 |
| **FX** | everything in Pro, plus Saturn 2, Timeless 3, Volcano 3 (10 plug-ins) | 799 | 1 570 |
| **Creative** | Saturn 2, Timeless 3, Volcano 3, Twin 3 | 339 | 516 |
| **Total** | all 14: the seven Pro plug-ins, Saturn 2, Timeless 3, Volcano 3, Twin 3, One, Simplon, Micro | 899 | 1 826 |

Single prices: EUR 169 for each Pro plug-in, 129 for Saturn/Timeless/Volcano/Twin, 49 for One and Simplon, 29 for
Micro. Some resellers still list an older Mixing bundle with Pro-C 2 in it; FabFilter's own shop lists Pro-C 3.

**Implication for Cantus:** the *Pro* set (EQ, compressor, limiter, reverb, multiband, de-esser, gate) is what
FabFilter sells as a coherent family. Saturn and Timeless join it in the Mixing and FX bundles and bring the
modulation system with them. Our planned list (EQ, compressor, limiter, reverb, multiband, de-esser, gate,
saturation, delay) is exactly the **FX bundle minus Volcano**.

### 2.2 Current versions (download page, 2026-09-23)

| Product | Version | Date | Visual generation |
|---|---|---|---|
| Pro-Q 4 | 4.13 | 2026-06-25 | modern (Dec 2024 redesign) |
| Pro-C 3 | 3.02 | 2026-04-16 | modern (Jan 2026) |
| Pro-R 2 | 2.06 | 2026-04-16 | modern-ish (Nov 2023; navy, icon knobs) |
| Pro-L 2 | 2.26 | 2026-04-16 | classic (arrow knobs, lilac-blue) |
| Saturn 2 | 2.13 | 2026-04-16 | classic |
| Timeless 3 | 3.10 | 2026-04-16 | classic |
| Volcano 3 | 3.09 | 2026-04-16 | classic |
| Twin 3 | 3.07 | 2026-04-16 | classic, new browser |
| Pro-MB | 1.33 | 2026-04-16 | oldest (bevelled, white pointers) |
| Pro-DS | 1.32 | 2026-04-16 | oldest (yellow textured knobs) |
| Pro-G | 1.42 | 2026-04-16 | oldest (lavender textured knobs) |
| One / Simplon / Micro | 3.52 / 1.42 / 1.32 | 2026-04-16 | "fresh modern look" in May 2025 |

Recent family-wide changes, from the news posts:
- **March 2023:** interface *scaling* was added to all plug-ins, separate from resizing.
- **January 2026:** with Pro-Q 4.10, the instance list gained Pro-C 3, Pro-DS and Pro-G.
- **March 2026:** in every plug-in, you can drop preset files onto the interface, and right-clicking the preset
  button gives Copy/Paste. Sizing bugs were fixed, mostly in Logic.
- **April 2026:** preset browser hover behaviour was improved in Pro-Q 4, Pro-C 3, Pro-R 2 and Twin 3.

---

## 3. The shared interface language

### 3.1 Layout grid

Every product has four horizontal zones, in the same order
([family-01](screenshots/family-01-lineup-side-by-side.jpg)):

1. **Top bar.** About 36–40 pt high, estimated from @2x marketing screenshots.
2. **Display.** The large, interactive, animated area. It takes 50–85 % of the height and usually runs the full
   width, with a meter column at the right edge.
3. **Control area.** Knobs and buttons.
   - In the display products (Pro-Q, Pro-MB, Saturn), it is a *floating panel* that sits over the lower display,
     under the selected band.
   - In the dynamics products, it is a fixed row on top of the display's lower edge.
   - In the modulation products, a **modulation strip** sits below it.
4. **Bottom bar.** About 25 pt.

Proportions follow the product's aspect ratio, not a unit grid. Medium sizes are about 840 × 690 pt (Pro-C 3) and
about 1150 × 690 pt (Pro-Q 4). The logo tab's lower-right edge sweeps down in a curve into the display, and that
curve is the family's signature shape. In Pro-R 2 the same curve runs along the bottom of its control deck.

### 3.2 Top bar

The same order, left to right, in all nine products ([family-02](screenshots/family-02-top-and-bottom-bars.png)):

| Slot | Content | Notes |
|---|---|---|
| Logo tab | "fabfilter / software instruments" in grey, then the **product wordmark in the product colour** | Pro-Q/Pro-C/Pro-MB/Pro-DS/Pro-G use the geometric "Pro·X" wordmark with a superscript version number. Saturn and Timeless use a script wordmark. |
| Undo, Redo | icon buttons (curved arrows) | disabled (dim) when there is nothing to undo |
| A/B, Copy | the letter of the active state is bright and the inactive one dim ("A/**B**") | Copy disables itself when A and B are equal |
| ‹ preset › | previous and next arrows around a **wide, centred preset name** | the name dims when the preset has been edited. The modern products use thin chevrons, the classic ones filled triangles. |
| Help | text button, opens the Help menu | the menu also holds display preferences (3.7) |
| Full screen | corner-bracket icon, far right | Esc leaves full screen |

Product extras go between Help and the preset field. Pro-L 2, for example, adds a meter-and-lock icon.

### 3.3 Bottom bar

| Zone | Content | Behaviour |
|---|---|---|
| Left | **MIDI Learn** and a ▾ menu (Enable MIDI, Clear, Revert, Save) | click to enter learn mode (4.3) |
| Middle | product globals, written as **"Label: Value"** (dim label, bright value): Oversampling: 4x, Lookahead: On (20 ms), Channel Mode: Left/Right, Dither: 16 Bits. Toggles are written as plain words: Linear Phase, High Quality, True Peak Limiting, Auto Mute Self-Osc. | Click to open a menu or to toggle. **A thin coloured line on the top edge of a segment means "on"**: green for High Quality or True Peak, blue for Freeze or phase invert, yellow for Auto Gain. |
| Middle (Pro-Q 4) | the **instance list button**: a coloured dot plus the track name | 4.6 |
| Right | **bypass** icon, then the output summary (e.g. `100%  0.0 dB  0.0 dB` = mix, in, out; or `In: 0.0 dB  Out: 0.0 dB`), then a **resize** icon in the corner | hovering the output text opens the output panel; clicking makes the panel sticky |

Output panel ([family-10](screenshots/family-10-output-options-popup-proc3.jpg)): large input and output level knobs,
each with a pan ring, a mix bar that goes to 200 % in Pro-C, and numeric values underneath. You can drag the
bottom-bar text vertically, use the mouse wheel on it, or double-click it to type, without opening the panel.
Alt-drag moves input and output in opposite directions.

Global bypass is FabFilter's own. They document that it applies latency compensation and a soft (click-free)
bypass. While bypassed, **the display dims and a red line appears on top of the bypass button**.

### 3.4 Knobs: two generations

**Classic** (Saturn 2, Timeless 3, Volcano 3, Twin 3, Pro-L 2, Pro-G, Pro-DS, Pro-MB;
[family-05](screenshots/family-05-knob-classic-anatomy-saturn2.jpg),
[family-07](screenshots/family-07-knob-modulation-ring-saturn2.jpg)):
- **Body:** a mid-grey disc with a soft top-light gradient. The product tints it: warm grey in Saturn, lilac in
  Pro-L and Pro-G, yellow in Pro-DS.
- **Pointer:** a **light "arrow" wedge**, a lighter segment of the rim with a short tick. It is also a handle:
  grab it for *rotate mode*.
- **Range:** min and max labels in small grey type at the two ends of the arc ("0 ms … 10 s", "−60 dB … 0 dB").
  Classic knobs have no value arc.
- **Second parameter:** an **outer ring** carries a second parameter. In Saturn it is a red L/R ring with a white
  wedge for stereo drive/level balance, labelled L and R. In Pro-G the ring shows gain and pan.
- **Modulation:** a thin coloured arc just outside the ring shows the live range being applied, drawn from the
  knob's own position. Next to the label, coloured dots name the source types (see 5).

**Modern** (Pro-Q 4, Pro-C 3, and in part Pro-R 2; [family-06](screenshots/family-06-knob-modern-proc3.jpg)):
- **Body:** a near-black disc, almost the same colour as the background, with a thin lighter rim.
- **Value:** a **thick accent-coloured arc inside the rim**, from the minimum (about 7 o'clock) clockwise to the value,
  ending in a **small dot at the tip**. There is no pointer line, and there are no min/max labels.
- **Label:** ALL CAPS with wide letter-spacing, under the knob. A secondary word lights up to show state: "AUTO
  THRESHOLD" and "AUTO RELEASE" have a dim "AUTO" that turns bright or coloured when the feature is on.
- **Extra rings:** outside the main one, for example Pro-C 3's green auto-threshold arc.
- **Small parameters:** Pro-C 3 draws Knee, Range, Lookahead and Hold as short **wave-shaped mini-sliders** (a thin
  curved track with a short thick yellow segment). Pro-R 2 puts a **glyph inside each knob** that shows what it
  does, and shows the value *inside* its big Space knob ("3.20 s").
- **Measured contrast:** the knob body is **1.08:1** against Pro-C 3's display (#322d33 on #343339) and **1.21:1**
  against Pro-Q 4's panel (#29252c on #171518). The arc carries all the information. This is the "camouflage"
  complaint (section 6).

**Common to both:** knobs **"light up" on hover**, and a **value popup** shows the parameter's name and value while
you hover or drag. Linked pairs (for example in/out or dry/wet) are shown by a connecting line between the labels,
as in Saturn's FEEDBACK — FREQ.

### 3.5 Dragging and modifiers

From the knob help pages. The Pro Tools alternatives are there because FabFilter follows each host's
conventions.

| Gesture | Effect | In Pro Tools |
|---|---|---|
| Drag up or down on the knob | **vertical mode**, *speed-sensitive*: slow movement gives finer steps | — |
| Drag the arrow wedge around the knob (classic only) | **rotate mode**; moving further from the centre is finer | — |
| Mouse wheel over a knob, pan ring or bottom-bar value | step. On Windows you may need to click into the plug-in first. | — |
| Shift + drag or wheel | fine adjustment | Ctrl/Cmd + drag |
| **Ctrl/Cmd + click** | reset to default | Alt + click |
| **Double-click** | type a value | — |
| Alt + drag | move *linked* knobs together, e.g. in against out in opposite directions | Shift + drag |
| XY pad: Shift | lock to one axis | — |
| Mod slot level: Alt | adjust every slot of the same source | — |
| EQ display: wheel / Ctrl-drag / Alt-drag / Alt-click | Q / Q / constrain the axis / bypass a band | see pro-q.md 5.4 |

**Keyboard:** there are no documented keyboard controls for knobs. Tab only steps through fields while typing
EQ values. Esc closes full screen, the preset browser and learn mode.

### 3.6 Typing values

Double-click a knob, a band dot or a value in a popup, and a text field opens in place. The parser accepts the same
things in every product:
- `1k` = 1000 Hz;
- note names `A4` = 440 Hz, including cents: `C#3+13`;
- `2x` = +6 dB (a gain ratio);
- `50%` = the middle of the knob's travel, for any parameter.

Tab moves to the next field in the Pro-Q parameter display.

### 3.7 Hover, value popups and help

- **Value popup:** hovering over a knob shows its name and value. In Pro-Q 4 the band's popup doubles as a mini
  editor: you can drag, wheel or double-click the values, and it holds bypass, solo, delete and shape buttons.
- **Hover panels:** the output and analyser settings open on hover and stay open ("sticky") if you click.
- **Interactive help hints** ([family-13](screenshots/family-13-help-hint-timeless3.jpg)): every product page
  advertises "an extensive help file with interactive help hints". The official image shows a **large, light-grey
  rounded speech bubble with dark body text**, whose tail points at the **Help** button. It has two short
  paragraphs: what the control does, then how to operate it ("click the centre to enable, drag to change the
  amount"). *The manuals don't say exactly what triggers a hint (hover delay, or hovering the Help button); check
  this in a trial.*
- **The Help menu** is also where **display preferences** live:
  - Pro-Q 4: Show EQ Parameter Display, Auto-EQ Sketch, Auto-Adjust Display Range, Show Frequency On Hover,
    **Use Accessible Colors**;
  - Pro-C 3: Show Input Level Meter;
  - Timeless 2: Show Component Displays.

  The same menu holds the help file, About (version) and Enter License.

### 3.8 Colour and typography

**Principle:** near-black neutral surfaces; white for primary text; mid-grey for labels; *one product hue* for the
wordmark, the value arcs and the main curve; saturated colours for band identity; red for clipping, gain
reduction and bypass.

Measured from the official screenshots:

| Element | Colour | Contrast |
|---|---|---|
| Pro-Q 4 display background | #18151a (top) to #1f1b20 (bottom): a subtle vertical gradient, warm near-black | — |
| Pro-Q 4 top bar / bottom bar | #141217 / #1a181d | — |
| Primary text (preset name, bottom-bar values) | #fffeff | 17.5:1 on #1a181d |
| Pro-C 3 knob label | #c5c5c7 on #201e21 | 9.6:1 |
| Pro-C 3 dim label ("AUTO", disabled "LOOKAHEAD") | #5e5c5f on #201e22 | **2.5:1** (fails AA) |
| Pro-Q 4 analyser dB scale | #656368 on #18161b | **3.0:1** (fails AA for text) |
| Pro-Q 4 EQ scale (yellow) | #ffec36 | 14.8:1 |
| Pro-C 3 value arc (yellow) | #ffd844 on the knob | 9.7:1 |
| Saturn 2 display / lower panel | #2d1417 (burgundy) / #161616 | — |
| Saturn 2 drive ring | #da272e | 3.7:1 on #161616 |
| Timeless 3 display | #182015 (green-black) | — |
| Pro-R 2 display / accent | #050507 / amber #ffd766 on navy | — |
| Pro-L 2 panel / waveform | #3f3d48 / lilac #8a96bc | — |

**Band and source colours** are a fixed rotating set of saturated hues, used as translucent fills plus solid dots:
yellow-green, red, blue, magenta, violet, teal, and so on. Collisions in Pro-Q 4 are a dark red glow, and *Use
Accessible Colors* switches that one colour to a brighter one.

**Typography:**
- a humanist sans-serif for all UI text; the wordmarks are custom;
- value text is mixed case with units ("0.0 dB", "20 ms");
- knob labels in the modern products are ALL CAPS and tracked;
- the bottom bar is mixed case.

We could not verify whether FabFilter uses tabular figures.

### 3.9 Animation and motion

- **Hover:** knobs brighten. Band dots and curves highlight.
- **Panels:** the Pro-Q/Saturn band panel **follows the selected band** horizontally. Output and analyser panels pop
  up from the bar. Modulation sources expand in place and the others auto-collapse (an option).
- **Displays run continuously:**
  - scrolling level histories: Pro-C, Pro-L, Pro-DS, Pro-G;
  - spectra with a configurable release speed, and a Freeze that builds a maximum hold;
  - a stack of decaying translucent traces in Pro-R 2;
  - delay taps pulsing in Timeless.
- **Modulation:**
  - target arcs move in real time;
  - a **"source flow"** diffuse coloured glow drifts from the sources up towards what they modulate (it can be
    turned off);
  - source tiles show their live waveform or level.
- **Audio side:** "Smart Parameter Interpolation" smooths parameter jumps and automation is sample-accurate.
  Drawing is GPU-accelerated.
- Durations and easing are not documented.

### 3.10 How the analysers are drawn

| Plug-in | Rendering |
|---|---|
| Pro-Q 4 | Pre-EQ spectrum as a **filled, darker translucent grey area**. Post-EQ as a **light grey line** over a faint fill. The external/sidechain spectrum has a light red outline. Each band's **shape is a translucent fill in the band's colour with a gradient**, plus a thin outline. The **overall curve is a thick yellow line**. Spectrum Grab dims the bands. The spectrum is tilted 4.5 dB/oct by default so it "looks natural". Two scales: yellow for EQ gain, grey for the analyser. |
| Pro-MB | The same spectrum style. Each band's frequency range is a **vertical translucent column** in the band's colour. Gain change is drawn as a filled region around the 0 dB line, with the band dot on it. |
| Saturn 2 | A single-line spectrum (light, thin) with a **gradient fill in the product hue**. The **selected band's range is lit brighter** across the full height. Crossovers are **dashed vertical lines** with a handle icon. Band levels are **pill-shaped grey handles**. |
| Pro-C 3 / Pro-G / Pro-DS | Scrolling history: input as a **filled grey area**, output as a lighter line or fill, **gain reduction as a red line from the top**. Pro-DS paints only the detected sibilance in lime. The knee/transfer curve sits on the same dB scale. |
| Pro-L 2 | A lilac filled waveform history; gain reduction as red spikes from 0 dB with **yellow value tags** ("−1.6 dB"). Large segmented meters with a loudness column. |
| Pro-R 2 | Several **translucent amber traces** stacked to show decay over time, with Decay Rate EQ and Post EQ curves in blue and yellow. |
| Timeless 3 | Delay taps as thin vertical bars coloured by channel (green/orange); a filter display with the analyser behind it. |

The common traits:
- fills are translucent and additive on near-black;
- curves are 1–3 px, with the "result" curve thickest and in the accent colour;
- nothing is fully opaque except the dots.

### 3.11 Interactive displays: hover and selection

- **Hover:**
  - in Pro-Q 4, a *curve preview* shows the band you would create at that spot (dashed when a click would
    deselect instead);
  - the frequency (or piano key) under the pointer shows on the axis;
  - hovering a dot highlights its shape;
  - a spectrum held still under the pointer enters Spectrum Grab.
- **Selection:**
  - a selected band's fill becomes more opaque and its dot gets a white ring;
  - rectangle-drag selects several bands;
  - the floating panel shows "(multiple)" when more than one is selected.
- **Handles:**
  - EQ bands: coloured dots (numbered in Timeless and Twin);
  - dynamic range: small ▲▼ markers;
  - crossovers: an icon at the top of a dashed line;
  - levels: pills.
- **Scales:** you can zoom the frequency axis by dragging it vertically and pan it horizontally; double-click
  resets. The dB range comes from a menu (±3/6/12/30 dB) and grows automatically while you drag.

### 3.12 Resizing, full screen, scaling and HiDPI

- **Full screen** is a top-bar button, and Esc leaves it. The plug-in chooses a larger scaling automatically in
  full screen.
- **Resize menu** in the bottom-right corner ([family-11](screenshots/family-11-resize-menu-proc3.jpg)):
  - fixed sizes: Small / **Medium (default)** / Large / Extra Large; Pro-C 3 has Compact / Small / Medium / Large,
    where Compact hides the displays and turns the meters horizontal;
  - the chosen size becomes the default for new instances;
  - sizes the current screen cannot fit are greyed out.
- **Scaling** is a submenu, separate from size (since March 2023): for example 125 % and 150 %, or 150 %/300 % on
  a Retina display that defaults to 200 %. It is remembered **separately for normal and full-screen mode and per
  monitor type** (HiDPI or regular).
- **Free resize** by dragging the window edge works in VST3 (and CLAP in Pro-C 3).
- All art is vector or @2x, and drawing is GPU-accelerated.

---

## 4. Shared workflow features

### 4.1 Presets

There are two generations:

- **Menu** (Pro-L 2, Pro-MB, Pro-DS, Pro-G, Saturn 2, Timeless 3, Volcano 3):
  - a hierarchical menu of folders;
  - ‹ › step through presets one at a time;
  - an Options submenu: Save As Default, Open Other Preset, Change Preset Folder, Restore Factory Presets, Enable
    MIDI Program Changes.
- **Browser** (Pro-Q 4, Pro-C 3, Pro-R 2, Twin 3; [family-09](screenshots/family-09-preset-browser-proc3.jpg)):
  - a dark panel that drops from the preset field;
  - **search field** at the top, with a gear for Options and a **star** that filters to favourites;
  - **folder column** on the left: hover a folder to open it, click a preset to load it; the current preset is
    highlighted in blue;
  - **details panel** on the right: name, author, **tags** (pills; "+" adds, double-click renames, × removes), a
    description, and a star;
  - **Copy / Paste / Save As…** at the bottom.
  - **Keyboard:** type to search, filtering by folder, name and tag. The arrow keys navigate, Enter loads and
    closes, → loads without closing, Esc cancels, and `[` `]` step presets while the browser is open.
  - Moving the pointer out of the browser after loading closes it.

**Common to both:**
- **the name dims when the preset has been edited**;
- right-clicking the preset field gives Save (asks to confirm), Save As, Favorite and Copy/Paste;
- preset files are dropped onto the UI to load them;
- **MIDI Program Change / Bank Select** loads presets, numbered automatically in folder order;
- "Default Setting" is loaded at start-up and can be overwritten.

Files are `.ffp`, stored in `Documents/FabFilter/Presets/<Product>`, identical on Windows and macOS. **Section
presets** (Saturn, Timeless, Volcano, Twin) save one band or one modulation source, with its own Save As Default.
They are stored in the app-data folder, which you cannot move.

### 4.2 A/B, copy, undo/redo

- **Undo:** every UI change (a knob drag, a preset load) adds a state. Changes made through automation or MIDI are
  **not** recorded. The buttons disable themselves at either end. **The depth of the history is not documented.**
- **A/B:** switching saves the current state first, so pressing twice brings you back. **Copy** copies the active
  state into the inactive one and then disables itself while the two are equal.
- Undo and A/B are **per instance**, separate from the host's undo.

### 4.3 MIDI Learn

- Click **MIDI Learn**. The interface **dims**, every learnable control is highlighted, and each shows a **small
  balloon with its CC number**.
- Click a control: a **red square** marks it, and a **banner** across the top says "Move a slider/knob on your MIDI
  controller to associate it to: <parameter>", with a Close button.
- Move a controller, and it is mapped. Remapping replaces the old mapping without warning.
- The ▾ menu has Enable MIDI (globally), Clear (per mapping or all), Revert and Save. The mapping is saved
  automatically when the plug-in closes.
- The help spends a page on **how to get MIDI into an effect in each host**: in Logic this means loading it as a
  MIDI-controlled effect, in Live only the first plug-in on a track receives MIDI. This is the plug-in wall
  (see 7.5).

([family-12](screenshots/family-12-midi-learn-saturn2.jpg))

### 4.4 Automation mapping

- Every parameter is exposed to the host and automation is sample-accurate.
- Smart Parameter Interpolation smooths the steps.
- Avid/EUCON control surfaces are supported (Pro Tools).
- The plug-in doesn't show automation state anywhere in its own UI.

### 4.5 Help system

- A full HTML/PDF manual per product, with the same chapter structure everywhere: Introduction, Using, Presets,
  Purchase, Support.
- Hint bubbles (3.7). Links from the help into FabFilter's "Learn" tutorials.
- The Help menu (help, preferences, About, licence).

### 4.6 The instance list: FabFilter's cross-product surface

Since Pro-Q 4.10 (January 2026), clicking the track-name button in Pro-Q 4's bottom bar replaces the UI with a
**zoomable list of every Pro-Q 4, Pro-C 3, Pro-DS and Pro-G instance in the session, grouped by track**. It is
FabFilter's first "family view" and effectively a multi-track channel strip.
- **Zoom:** a slider controls how much each instance shows, from a thumbnail spectrum or Threshold and Gain only,
  up to almost the full UI.
- **Navigation:** auto-zoom on hover, filter by name, Quick Jump, pinning (including "pin similar colours"), and a
  minimap.
- **Per instance:** bypass, an emphasize button (a momentary level boost), and a menu with the instance's presets
  and copy/paste.
- **Masking:** collision detection against a reference track.

**It depends on the host telling the plug-in the track order, name and colour:**
- Pro Tools only reports track position since 2025.12 and never reports colour;
- Audio Units reports neither, so tracks are sorted alphabetically;
- FL Studio's CLAP reports no names, and the order can be wrong when some instances add latency;
- stopping and starting playback re-sorts the list.

---

## 5. The modulation system (Saturn 2, Timeless 3, Volcano 3, Twin 3)

This is the model for our instruments later. It is the same system in all four products.

### 5.1 Sources

Added with the **"+"** in the modulation strip at the bottom. Every source type has its own colour family:
cyan for the XLFO, blue for the EG, violet for the EF, purple for the MIDI source and green for sliders and XY.

| Source | What it does |
|---|---|
| **XLFO** | LFO and 16-step sequencer. Each step has a value, a glide, a curve (linear, sqr, sqrt or sine) and an optional *random* flag. Global controls: free rate (0.02–500 Hz) or tempo sync (16 bars to 1/64) with an offset from ×0.5 to ×2 and dotted/triplet presets; a **balance ring** on the rate knob that skews the first half of the cycle against the second; global glide (−1…1); a draggable phase offset; MIDI retrigger or legato; **Snap**, which turns it into an arpeggiator over two octaves with a mini keyboard. Rate, balance, phase and glide can all be modulated. |
| **Envelope generator (EG)** | Delay, attack, decay, sustain, hold and release, edited by dragging points on a graph. Each segment has a **slope dot** that bends the curve. It is triggered by the main input, the sidechain, a band's input or MIDI notes. A threshold knob has a **circular input meter around it**, and there is an audition button. The range is either 0…1 or "neutral sustain", where the sustain stage outputs 0. |
| **Envelope follower (EF)** | Attack and release are dragged on a graph. It follows the main input, the sidechain or a band, in **Envelope** or **Transient** mode (a transient detector). |
| **MIDI source** | Velocity, pitch bend, mod wheel, key tracking or any CC number, through a response curve (linear, exp, log, square, sqrt or sine). Twin 3 adds a range setting: negative, centred or positive. |
| **XY controller / slider** | Up to 6. Either a 2-D pad with separate X and Y outputs, or a vertical **slider**, which works as a macro. Bipolar or unipolar. Shift locks the axis. |

Twin 3 adds a **Source Level** (−200…200 %, which can itself be modulated) to every source.

### 5.2 Source tiles

- **At rest:** each source is a **tile showing a live mini-graphic** of its settings (the XLFO waveform, the EG
  shape, the EF curve, a MIDI icon). A **coloured ring dot above the tile is its drag handle**; a filled centre in
  that dot means the source is in use. The name is under the tile, with a ▾ for section presets.
- **Direct adjustment:** dragging the tile itself adjusts the source's main parameter, for example the XLFO rate,
  with a value popup.
- **Expanding:** click a tile to open its full editor in place. Other sources auto-collapse (optional), and clicking
  the background collapses everything.
- **Naming:** double-click a name to rename the source. Names are saved in presets, A/B and undo.
- **Show/hide:** ">>" and "<<" show or hide every source **except sliders and XY pads**. That leaves a tidy
  performance view of macros only, which is how the factory presets are meant to be used.

### 5.3 Connecting: drag and drop

- **Grabbing** a source's drag handle dims the whole interface and **highlights every valid target**.
- **While dragging**, a line follows the pointer and **snaps to targets**. Hovering a band or a tab while dragging
  selects it (spring-loaded), so targets in hidden panels can be reached.
- **Dropping** on a knob, a point on a graph or a crossover creates a **slot**, and the **slot panel** pops up next
  to the target.
- There is no menu or matrix to navigate. FabFilter's own help presents this as the point of the design.

### 5.4 Slots ([family-08](screenshots/family-08-modulation-slot-panel-saturn2.jpg))

- **Opening the panel:** click a target, its indicator dots or a source's drag handle. You see every slot that
  involves it, for that target or for that source.
- **Each slot row has:**
  - a **bipolar level slider** drawn in the source's colour (Shift = fine, Alt = every slot of the same source,
    Ctrl/Cmd-click = reset);
  - a **+/− invert** button;
  - an on/off button that appears on hover;
  - a ▾ menu to change the source or target;
  - a **×** to remove it.
- **Slot-on-slot:** a slot's level can itself be a target. The modulating slot is **shown indented under it** with
  a connecting line.
- **Panel menu:** delete, bypass or invert all slots.
- **Limits:** 50 slots in Saturn 2 and Timeless 3, 100 in Twin 3.

### 5.5 Visualization

([family-07](screenshots/family-07-knob-modulation-ring-saturn2.jpg), [family-03](screenshots/family-03-saturn2-main.jpg))
- **Target level indicator:** an arc around the knob or slider, starting at the knob's own position and showing the
  range the modulation is sweeping, in real time. It takes the source type's colour, or **yellow when several
  source types mix**.
- **Modulation indicator dots:** small dots next to a target's label, one per source *type*, in that type's colour.
  Clicking them opens the slot panel. A dot on a display or on an EG means some parameter inside it is modulated.
- **Source flow:** a soft coloured glow in the band above the sources, drifting towards what they modulate. The
  help admits it is only an overall impression, not an exact map. It can be switched off.

### 5.6 What to take and what to fix

**Take:**
- drag from source to target;
- slots that open next to the target;
- the knob arc showing the live range;
- slot-on-slot modulation;
- tiles that show live mini-graphics;
- renamable sources;
- per-section presets;
- the "show only the macros" view.

**Fix:**
1. **There is no overview.** You only see the slots for one target or one source at a time, so a preset with 30
   slots can't be read. Add a matrix or list view that can be sorted and filtered.
2. **Colour is by type, not by instance.** Two XLFOs share a colour and a dot, so you can't tell which one is
   modulating a knob.
3. **Depth is a bare slider.** Give it units ("±6.0 dB", "±1.5 oct") and draw the range on the knob even while
   idle.
4. **The source flow is decorative.** Replace it with an optional exact "show connections" overlay.
5. **Modulation lives inside one plug-in.** In our host, modulators can be a host service that can reach any
   parameter of any device on the track (7.6).

---

## 6. Accessibility and known criticisms

| Issue | Evidence |
|---|---|
| **Dark only; no light theme, gamma or brightness setting** | No theme option in any product. A forum user measured Pro-R 2's brightest orange at about 46 % luminance and asked for a gamma setting; there was no staff reply (FabFilter forum, 2024). |
| **Control boundaries are nearly invisible** | Measured knob body vs background: 1.08:1 (Pro-C 3) and 1.21:1 (Pro-Q 4), against WCAG 1.4.11's 3:1. KVR users on Pro-Q 4: the knob looks like it's "wearing camouflage"; "who needs contrast?". |
| **Secondary text is under AA** | Dim labels 2.5:1, analyser scale 3.0:1 (measured). |
| **Darker with each redesign** | Sound On Sound calls Pro-Q 4 "noticeably more sombre" than version 3 (while praising the usability). KVR: the darker UI is "hard for my eyes". A forum user ranks Pro-R 2 as the darkest and Pro-C 2 as the easiest to read. |
| **Small text** | Before March 2023, resizing only spread the controls apart without enlarging text. Timeless 2 was singled out for its "dark, low contrast GUI with tiny buttons". Scaling fixed most of this, and Sound On Sound notes it rescues Pro-Q 4's "minuscule" expert controls. |
| **Hidden state** | Output, analyser and character settings live in hover panels. The meaning of a coloured line on the bottom bar has to be learnt. Value popups only appear on hover. |
| **Colour as the only cue** | Band identity, modulation source type, collisions (dark red; the fix is *Use Accessible Colors*) and states are all colour-coded. |
| **No keyboard or screen-reader access** | No keyboard control of knobs is documented, and nothing about screen readers. Custom GPU-drawn UIs like this usually expose nothing to accessibility APIs. |
| **Inconsistent generations** | One bundle contains three visual styles: textured bevels (Pro-MB, Pro-DS, Pro-G), arrow knobs (Pro-L 2, Saturn, Timeless) and flat dark (Pro-Q 4, Pro-C 3). Only four products have the new preset browser. |
| **Two undo histories** | Plug-in undo is separate from the host's, and automation and MIDI changes aren't in it. |
| **Preferences in the Help menu** | Display options sit under "Help", where nobody looks. |
| **Plug-in wall** | The instance list and MIDI Learn both depend on what each host exposes (4.3, 4.6). |

**Accessibility features that do exist:**
- scaling (per monitor type, and separately for full screen);
- *Use Accessible Colors* (Pro-Q 4, for the collision colour only);
- Show Frequency On Hover;
- the options to hide distracting overlays (parameter display, source flow, curve previews);
- the typed-value parser, which is a good accessibility aid for precise values.

---

## 7. A family language that beats it

The target: **FabFilter's frame and gestures, with fixes for its legibility, its hidden state and its plug-in
wall.** This builds on what Cantus already has:
- `design.rs`: the 4-px grid, IBM Plex type scale, monospaced numbers and focus rings;
- `theme.rs`: light default, an editable palette and WCAG luminance helpers;
- `widgets::{Knob, Fader, Bar}`: one interaction set, with the arc drawn from the default;
- the command model with undo fuzz;
- the planned *plug-in view* contract and analyser tap in STATUS §2.

### 7.1 Principles

1. **One frame, fixed positions.** Every effect and instrument has the same header and footer, in the same order,
   docked in the inspector or floating. Only the body differs.
2. **The thing you drag is the parameter.** Every value has a direct handle, and the display is the editor.
3. **State is visible at rest.** No important value or mode may exist only on hover.
4. **Legible in both themes.** Every pairing of colour tokens passes AA, checked by code.
5. **The host owns state.** Undo, A/B, presets, automation, controller mapping and modulation are host services
   that every device shares.

### 7.2 Component list

| Component | Anatomy | Rules | FabFilter reference |
|---|---|---|---|
| **Device frame** | header (32 px = 8 grid units), body, footer (28 px = 7 units); 12 px outer padding | docks into the inspector at "compact" density or opens as a window; continuous resize with a minimum size; full screen | 3.1 |
| **Header / preset bar** | [channel chip: track colour swatch + name] [device name in the device hue] · [undo] [redo] · [A/B toggle] [→ Copy] · [‹ preset name ●modified ›] · [compare Δ] [bypass] [?] [⋯] [⛶] | the preset name never dims to show an edit; a **dot plus italic** shows "modified" (a shape cue, not only colour); ‹ › and `[` `]` step presets; right-click gives Save, Save As, Copy, Paste and Favourite | 3.2, 4.1 |
| **Footer / global strip** | [Learn] · global options as **chips reading "Label value"** (always visible, never hover-only): Quality 4×, Lookahead 5 ms, Mode M/S … · **latency badge** ("11.6 ms") · [In] [Out] [Mix] as editable readouts · [size] | a chip that is "on" gets a **filled background and an icon**, not only a coloured line; In, Out and Mix are real Bars, so there is no hover panel | 3.3 |
| **Knob** | 270° arc with the gap at the bottom; a **track** in `grid_strong`; a **value arc from the default** (bipolar-aware) in the device hue; a body with a **3:1 edge** against the surface; a **pointer tick** (kept so it reads in greyscale); a label under it; the value **replaces the label while hovering or dragging** | sizes S 28 / M 36 / L 56 / XL 80 px (on the grid); optional min and max ticks; an **outer 3 px ring for modulation or automation** (7.6); a second ring for pan or balance only where the parameter really is two-dimensional | 3.4 |
| **Bar** (uni/bipolar) and **Fader** | as in `widgets::bar` and `widgets::fader` | the same gestures as the knob | Pro-C's wave mini-sliders |
| **Segmented control / toggle** | text + icon; the selected segment is filled | used for style, mode and channel mode; ≥ 3:1 edge | Pro-G/DS segmented buttons |
| **Value readout** | IBM Plex Mono, tabular digits, **width reserved for the widest value** ("−100.0 dB") | drag, wheel and type, like a knob | bottom-bar text |
| **Value popup** | a small plate *above* the control (never covering it or the pointer), mono digits, units | shows at once on hover and while dragging; also shows the automation value when it differs from the manual value | 3.7 |
| **Display canvas** | a shared frame: frequency axis (log; piano option), dB axis with a range menu (±3/6/12/30), a grid in `grid`, the analyser layer, curve layers, a handle layer | analyser fills adapt to the theme (7.4); zoom and pan the axes by dragging them, double-click resets; the frequency and level under the pointer show on the axes | 3.10, 3.11 |
| **Band handle** | a dot of 12 px **with its number inside**, in the band colour | four states: idle (outline), hover (ring plus shape highlight), selected (filled with a white ring and a more opaque shape), dragging (plus crosshair values); disabled bands show hollow and dashed | 3.11 |
| **Band popover** | next to the selected handle: Freq / Gain / Q as editable readouts, a shape picker, bypass, solo, delete | Tab moves between fields; never covers the handle it belongs to | pro-q.md 5 |
| **Meter** | shared ballistics (peak, RMS, LUFS short-term), a clip latch and a scale, plus a GR meter drawn from the top | colours from `meter_low/high/clip`, **plus scale ticks**, so colour is not the only cue | Pro-L/Pro-C meters |
| **Preset browser** | a panel: search + filters (tags, author, favourites, device, *this project*) · results list · details (tags, description, author) · Copy, Paste, Save As | fully keyboard-driven (type to search, arrows, Enter, → to preview, Esc); **one library across devices**; changing the selection only *previews* until you confirm | 4.1 |
| **Help strip** | one line at the bottom of the frame: *what the hovered control does · its gestures · its automation or modulation state*, EN/ES | appears after 500 ms of hover; F1 opens the full page. It never covers the UI, unlike FabFilter's bubble. | 3.7 |
| **Modulation** | source tile (live mini-graphic, drag handle, name), connection slot row, target ring, matrix view | see 7.6 | 5 |
| **Empty-state hint** | a centred, dim sentence in an empty display ("Drag the line or double-click to add a band") | disappears after the first edit | Pro-Q's default preset text |

### 7.3 Interaction rules (one set for every control)

These extend the rules already written in `widgets/knob.rs`. Where Cantus deliberately differs from FabFilter,
the table says so.

| Action | Cantus rule | vs FabFilter |
|---|---|---|
| Drag | up or right increases; **200 px for the full range, linear (no acceleration)** | FabFilter varies the rate with speed; ours is predictable and repeatable |
| Fine | **Shift** = 1/5 of the rate, for drag, wheel and arrows | same key |
| Reset | **double-click** (the existing Cantus rule and the common plug-in convention), **plus Ctrl/Cmd-click as an alias** for FabFilter muscle memory | FabFilter uses double-click to type |
| Type a value | **right-click opens a menu whose first line is an entry field that already has focus**, so typing still sets the value at once (the existing rule), and the menu below offers Reset, Automate, Learn, Modulate, Copy and Paste. Also **Enter on a focused control**, or **start typing digits while hovering**. | FabFilter uses double-click |
| Parser | `1k`, `2.5k`, `A4`, `C#3+13`, `2x`/`0.5x` (gain ratio), `50%` (travel), `-inf`, `ms`/`s`, `1/8d`, `1/4t`, and simple arithmetic `+3` or `-1.5` relative to the current value | a superset of FabFilter's |
| Wheel | one notch = 1 % of travel (Shift 0.2 %). Always active in a device window; in a scrolling inspector, only while Ctrl is held or after the control has been clicked | FabFilter always adjusts, with a click to focus on Windows |
| Alt + drag | moves the **linked** partner too (in/out, wet/dry, threshold/make-up) | same |
| Alt + click | **bypass** the element (band, slot, section, source) | Pro-Q uses this for bands |
| Keyboard | Tab/Shift-Tab through the controls in reading order; arrows ±1 % (the existing rule); PageUp/PageDown ±10 %; Home/End = min/max; Delete = reset; **Esc during a drag cancels and restores the value** | FabFilter has no keyboard control for knobs |
| Undo | **one gesture = one host command**; a drag is coalesced into a single step, and so is a wheel burst within 500 ms | FabFilter keeps its own history per instance |
| Hover | the control brightens by one tone step at once; the value replaces the label at once; the help strip after 500 ms | FabFilter pops up a value balloon |
| Multi-edit | selecting several bands, slots or strips and dragging one moves all of them relatively; Alt moves them absolutely | Pro-Q does this for bands |
| Display gestures | click on an empty area = add, drag = move, wheel = Q/width, Ctrl-drag = Q, Alt-drag = constrain the axis, right-click = menu, rectangle = multi-select | Pro-Q's set, kept |

### 7.4 Visual rules

1. **Colours come only from theme tokens** (`theme::Palette`). Every device works in **Paper, Parchment and Ink &
   Gold**, and in any palette the user edits.
2. **Contrast contract**, extended from the theme editor's check to every device token pairing:
   - text ≥ 4.5:1;
   - large numbers (18 px or more) ≥ 3:1;
   - **every non-text boundary ≥ 3:1** against what is next to it: knob edge, arc track, value arc, curves, band
     handles, meter segments, the focus ring;
   - **disabled text is still ≥ 3:1**, and its state is shown by an icon or style too.

   FabFilter fails the boundary rule by a wide margin.
3. **Colour is never the only cue.** Band handles carry numbers. States have icons and fills. Meters have scales.
   Collision and masking areas are **hatched** as well as tinted. Every modulation source *instance* has a letter
   or number as well as a colour.
4. **Each device has one hue**, used for its name, value arcs and main curve, and chosen from a validated set that
   passes 3:1 on both light and dark surfaces. All surfaces are shared neutrals, so a chain of devices reads as one
   product. The band/source palette is a separate, colour-blind-checked rotation.
5. **Analyser rendering depends on the theme.**
   - Dark theme: translucent fills and a thicker accent curve, as FabFilter does.
   - Light theme: **darker lines, fills at low alpha (≤ 20 %), no glow**. A bright fill on white washes out, so
     the analyser has to draw differently there.
   - In both themes, the pre spectrum is a fill, the post spectrum a line, other strips a dashed line, and the
     result curve is the thickest element.
6. **Numbers are monospaced with tabular digits** (`design::mono`), and the width is reserved, so readouts never
   jitter.
7. **Type:** IBM Plex. Labels are small caps or tracked caps at ≥ 11 px at 100 %; values ≥ 12 px; axis ticks ≥ 10 px
   in `text_dim`, which must still pass 4.5:1. Scaling follows the app's UI scale (Ctrl +/−) and each monitor's DPI
   through egui `pixels_per_point`, so there is no second scaling menu per device.
8. **Everything lands on the 4-px grid.** Control sizes S/M/L/XL are 28/36/56/80 px.
9. **Motion:**
   - displays at 60 fps; meters with real ballistics;
   - UI transitions ≤ 120 ms, ease-out;
   - **the OS's reduced-motion setting** turns off decorative motion (flow, glows, panel slides) but keeps the
     meters.
10. **Bypass** dims the body to 50 % and puts a "Bypassed" badge in the header. The badge is text, not only a red
    line.

### 7.5 What we can do that FabFilter cannot, because the devices are native

| # | Native advantage | FabFilter's workaround or gap |
|---|---|---|
| 1 | **The host's undo.** Every gesture is a `cantus-core` command in the project history, with redo, fuzz-tested invariants and undo across devices ("undo the EQ move I made before the compressor change"). The device's own undo and redo buttons show *the host history filtered to this device*. | A separate history per instance that ignores automation; two undo stacks to reason about |
| 2 | **Automation lanes one click away.** Right-click → "Show automation" opens the lane; knobs that have automation show an **automation ring** in the lane's colour, with the current automated value and a latch/override state; "Arm" writes from the device. | Invisible in the plug-in |
| 3 | **Knowing the channel.** The header chip shows the **track's name and colour** live. Device lists are in the real track and insert order, with no guessing and no stop/start re-sort. | The instance list guesses from partial host APIs (no colour in Pro Tools, no order in AU) |
| 4 | **One analyser shared across strips.** The per-strip analyser tap (STATUS §2b) feeds every device: overlay **any other strip's spectrum** in the EQ, and mark masking between the vocal and the guitars across the project, with **no routing**. The multiband, de-esser and gate all use the same tap and the same axes. | EQ-to-EQ only, and only between instances that have registered with each other |
| 5 | **A family view for free.** The mixer and inspector *are* the instance list: a "channel strip" view of all devices on a track, and an "all EQs / all compressors in the project" view, built on the same device views at compact density. | A special mode inside Pro-Q 4, limited to four products |
| 6 | **Sidechain from any track** through the existing send model (a send into an insert's second input), chosen from the device's header. | Depends on the host's sidechain routing |
| 7 | **Host-level controller mapping.** One Learn mode for the whole app. It reaches every device because MIDI never has to be *routed to an effect*. Mappings live in the project or in global templates. | A page of per-host routing instructions; mappings per plug-in |
| 8 | **One preset library across devices.** Tags, search and favourites shared; "presets used in this project"; chain presets (EQ + comp + de-esser) as one item; a preset diff ("what changed from the factory setting"). | Separate `.ffp` folders per product |
| 9 | **PDC and latency are visible.** The footer badge shows each device's latency, and the header or mixer shows the chain's total. | Latency is only mentioned in the help text |
| 10 | **A/B at the chain level.** A/B one device, or the whole insert chain, as an undoable host state; "compare to bypass at matched loudness". | Per plug-in only |
| 11 | **Themes, i18n and accessibility for free.** The app palette (light by default), EN/ES strings, keyboard focus rings, and AccessKit names and values for every parameter through egui. | Dark only, English only, not exposed to accessibility APIs |
| 12 | **Help inside the product.** The help strip and F1 open the device's page in the app's own help, localised. The AI bridge (`cantus-ai`) can explain or set a device with the same commands a user would use. | An external HTML/PDF manual |
| 13 | **Modulation as a host service** (7.6): the same modulators can drive *any* parameter of any device on the track, or the mixer. | Modulation is locked inside Saturn, Timeless, Volcano and Twin |
| 14 | **No version drift.** One widget library means every device updates together. | Three visual generations inside one bundle |

### 7.6 Modulation model for the family and the instruments

- **Sources are track-level objects:**
  - LFO/step sequencer (the XLFO feature set);
  - envelope generator (DAHDSR with slope handles, triggered by notes, the input, the sidechain or a band);
  - envelope follower (envelope or transient mode);
  - MIDI or MPE source with a response curve;
  - macro slider / XY pad.

  Each instance gets a **colour and a short tag** (LFO1, EG2), not just a colour for its type.
- **Connect by drag and drop, like Saturn.** Drag the source's handle, the valid targets light up, the line snaps,
  drop. Right-click → "Modulate by…" gives the same result from the keyboard.
- **The target ring** is drawn outside the knob:
  - a static band shows the **range** (always visible, not only while it moves);
  - a moving dot shows the **live value**;
  - the ring takes the colour of the instance, and becomes **striped** when there are several sources;
  - automation uses the same ring style in the lane's colour, so "something drives this knob" always looks the
    same.
- **Slot rows**, in a popover at the target and in the **matrix view**:
  - source → target;
  - depth **in the target's units**;
  - invert, bypass, curve;
  - an optional "via" (another source scales the depth, as in Saturn's slot-on-slot).
- **The matrix view** is a sortable table of every connection on the track, filtered by source, target or device.
  It is FabFilter's missing overview.
- **Show connections** is an optional overlay that draws the exact lines, replacing the decorative source flow.
- **Macros-only view:** hide everything but sliders and XY pads. This is Saturn's performance view, and the
  default for instrument presets.

### 7.7 Suggested order and open questions

1. The **device frame** (header, footer, help strip) and the **plug-in view contract**. Port the channel EQ and the
   existing Delay into it.
2. The **display canvas** and the **analyser tap**, then Aequalis (EQ) on them.
3. The **meter** component and the compressor. The sidechain arrives with it.
4. The **preset library**, and A/B as host state.
5. The **modulation service** with Saturn-style saturation as its first user, then the instruments.

**Still to verify in a trial:**
- what triggers FabFilter's help bubble;
- the undo depth;
- whether its digits are tabular;
- the exact animation timings.

---

## Sources

**FabFilter: products, shop, downloads, news**
- Products index: https://www.fabfilter.com/products
- Download page (current versions): https://www.fabfilter.com/download
- Shop bundles (fetched through a fetch service; EUR, VAT included):
  - https://www.fabfilter.com/shop/?currency=eur&vat=1
  - https://www.fabfilter.com/shop/pro-bundle
  - https://www.fabfilter.com/shop/mixing-bundle
  - https://www.fabfilter.com/shop/mastering-bundle
  - https://www.fabfilter.com/shop/fx-bundle
  - https://www.fabfilter.com/shop/essentials-bundle
  - https://www.fabfilter.com/shop/creative-bundle
- Product pages:
  - https://www.fabfilter.com/products/saturn-2-multiband-distortion-saturation-plug-in
  - https://www.fabfilter.com/products/timeless-3-delay-plug-in
  - https://www.fabfilter.com/products/twin-3-synthesizer-plug-in
  - https://www.fabfilter.com/products/pro-c-3-compressor-plug-in
- News:
  - Pro-Q 4.13: https://www.fabfilter.com/news/1782806400/pro-q-413-released
  - April 2026 fixes: https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins
  - March 2026 update: https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes
  - Pro-C 3 release: https://www.fabfilter.com/news/1768435200/fabfilter-releases-pro-c-3-compressor-plug-in
- Press releases:
  - Pro-C 3 and Pro-Q 4.10: https://www.fabfilter.com/press/1768467600/fabfilter-releases-fabfilter-pro-c-3-compressor-plugin-and-updates-fabfilter-pro-q-4
  - Pro-Q 4: https://www.fabfilter.com/press/1733994000/fabfilter-releases-fabfilter-pro-q-4-equalizer-plug-in
  - Pro-R 2: https://www.fabfilter.com/press/1698915600/fabfilter-releases-fabfilter-pro-r-2-reverb-plug-in

**FabFilter online help**
- Saturn 2:
  - [overview](https://www.fabfilter.com/help/saturn/using/overview)
  - [knobs](https://www.fabfilter.com/help/saturn/using/knobs)
  - [MIDI Learn](https://www.fabfilter.com/help/saturn/using/midilearn)
  - [undo, redo, A/B](https://www.fabfilter.com/help/saturn/using/undoredo)
  - [full screen, resizing, scaling](https://www.fabfilter.com/help/saturn/using/fullscreenandresize)
  - [modulation](https://www.fabfilter.com/help/saturn/using/modulation)
  - [modulation slots](https://www.fabfilter.com/help/saturn/using/modulationslots)
  - [modulation visualization](https://www.fabfilter.com/help/saturn/using/modulationvisualization)
  - [XLFO](https://www.fabfilter.com/help/saturn/using/xlfo)
  - [EG](https://www.fabfilter.com/help/saturn/using/eg)
  - [EF](https://www.fabfilter.com/help/saturn/using/ef)
  - [MIDI source](https://www.fabfilter.com/help/saturn/using/midisource)
  - [XY controller](https://www.fabfilter.com/help/saturn/using/xycontroller)
  - [loading presets](https://www.fabfilter.com/help/saturn/presets/loadingpresets)
  - [section presets](https://www.fabfilter.com/help/saturn/presets/sectionpresets)
  - [how presets are stored](https://www.fabfilter.com/help/saturn/presets/howpresetsarestored)
- Pro-C 3:
  - [intro](https://www.fabfilter.com/help/pro-c/intro/fabfilterproc3)
  - [overview](https://www.fabfilter.com/help/pro-c/using/overview)
  - [knobs](https://www.fabfilter.com/help/pro-c/using/knobs)
  - [loading presets](https://www.fabfilter.com/help/pro-c/presets/loadingpresets)
  - [saving presets](https://www.fabfilter.com/help/pro-c/presets/savingpresets)
  - [output options](https://www.fabfilter.com/help/pro-c/using/outputoptions)
  - [instance list](https://www.fabfilter.com/help/pro-c/using/instancelist)
  - [full screen, resizing, scaling](https://www.fabfilter.com/help/pro-c/using/fullscreenandresize)
- Pro-Q 4:
  - [intro](https://www.fabfilter.com/help/pro-q/intro/fabfilterproq)
  - [overview](https://www.fabfilter.com/help/pro-q/using/overview)
  - [knobs](https://www.fabfilter.com/help/pro-q/using/knobs)
  - [display and workflow](https://www.fabfilter.com/help/pro-q/using/eqdisplay)
  - [analyzer](https://www.fabfilter.com/help/pro-q/using/analyzer)
  - [output](https://www.fabfilter.com/help/pro-q/using/output)
  - [instance list](https://www.fabfilter.com/help/pro-q/using/instance-list)
- Pro-L 2 [knobs](https://www.fabfilter.com/help/pro-l/using/knobs)
- Timeless 3 [intro](https://www.fabfilter.com/help/timeless/intro/fabfiltertimeless) and [overview](https://www.fabfilter.com/help/timeless/using/overview)
- Twin 3 [intro](https://www.fabfilter.com/help/twin/intro/about)
- PDF manuals:
  - Pro-Q 4: https://www.fabfilter.com/downloads/pdf/help/ffproq4-manual.pdf
  - Pro-C 3: https://www.fabfilter.com/downloads/pdf/help/ffproc3-manual.pdf
  - Saturn 2: https://www.fabfilter.com/downloads/pdf/help/ffsaturn2-manual.pdf
  - Timeless 3: https://www.fabfilter.com/downloads/pdf/help/fftimeless3-manual.pdf
  - Timeless 2: https://www.fabfilter.com/help/fftimeless2-manual.pdf
  - Pro-L 2: https://www.fabfilter.com/downloads/pdf/help/ffprol2-manual.pdf
  - Micro: https://www.fabfilter.com/downloads/pdf/help/ffmicro-manual.pdf

**Reviews and forums**
- Sound On Sound, Pro-Q 4 review: https://www.soundonsound.com/reviews/fabfilter-pro-q-4
- KVR, Pro-Q 4 thread (UI darkness, "camouflage"): https://www.kvraudio.com/forum/viewtopic.php?p=9012835
- KVR, 2023 scaling update thread (text size before scaling): https://www.kvraudio.com/forum/viewtopic.php?t=594909
- FabFilter forum, Pro-R 2 GUI too dark (2024): https://prod.fabfilter.com/forum/topic/7637/r2-gui-everything-is-more-difficult-to-see-than-with-any-other-ff-gui-any-adjustment-available
- Reseller cross-check for the Pro bundle contents: https://www.audiosteps.io/en/product/fabfilter-pro-bundle/

**Measurements:** colour samples and contrast ratios in 3.8 and 6 were computed with Pillow from the official
screenshots listed in section 1, using the WCAG 2.x relative-luminance formula.
