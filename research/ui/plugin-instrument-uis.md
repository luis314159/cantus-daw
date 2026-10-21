# Plug-in and instrument interfaces: what the first rank does, and a style guide for Cantus

*Researched 2026-09-25 for Cantus. Research only: nothing in the code was changed. Products studied:
**Xfer Serum 2**, **Vital**, **Arturia** (Pigments 7 and the V Collection), **Native Instruments**
(Massive X, Kontakt 8 instruments), **u-he** (Diva, Zebra 3), **FabFilter Pro-Q 4** and **Soundtoys 5.5**.
Colours, sizes and contrast were **measured from the vendors' own screenshots** with Pillow and the WCAG
2.x formula (scripts in the session scratchpad; JPEG and video frames are ±2 per channel). Sizes are
given in the screenshot's own pixels unless marked "@1x"; where a product image is a @2x render, the
@1x value is half. Cantus was measured from `cargo run -p cantus-ui --example shoot -- <dir> 1 eq comp
limiter space wavetable analog fm sampler bass organ epiano pulsus` (1500 × 940 at scale 1, so 1 px = 1
logical px).*

*This report sits beside [`cantus-audit.md`](cantus-audit.md), which catalogues Cantus's defects. It
does not repeat them. It builds on [`../fabfilter/family.md`](../fabfilter/family.md) §7 (the effect
family's rules) and [`../instruments/family.md`](../instruments/family.md) §4.14 and §6 (measured
legibility of instruments, and the instrument frame rules V1–V7). Where this report changes one of
those rules, it says so.*

---

## 0. The short version

1. **The best plug-ins never show a loose word.** Every label belongs to a *unit*: a knob with its
   name, its value or range legends, and its tick scale, all on one grid cell (Soundtoys, Serum 2,
   Zebra 3, Diva, Arturia's hardware panels). Every group of units sits in a **framed section with a
   title** (Serum's title rows, Vital's rounded panels, Pigments' header rows, Zebra's module cards,
   Soundtoys' "pods", Little AlterBoy's colour blocks). Every choice among options is a **container**
   (segmented buttons, LED buttons, a menu field), never words in a row. Cantus has none of the three:
   its labels, values and section titles are **all the same 10-px grey at 4.65:1**
   (`#626977` on `#e8ecf0`, measured on the compressor), its sections have no frame, and its
   choices ("Clean Punch Opto Bus…", "Ladder Cascade Diode…", "Stop Slow Fast") are bare words.
2. **Hierarchy comes from weight, case and position, not from size alone.** In the products studied,
   a knob's label text is about **¼ of the knob's diameter** (ring : label height 3.8–5.1 in Pigments,
   Zebra, Vital and Serum), set in **caps or a bold face**, and in a **different place and colour
   from the value**. Pro-Q 4 and Pigments print labels near-white (12–13:1); Zebra and Massive X dim
   the *label* and brighten the *value*. Cantus prints both at 10 px in the one dim grey, one under
   the other, in two different typefaces: that is what reads as "words thrown down".
3. **A knob is an object that stands off its panel.** The ones that read best put a **dark body on a
   light deck** (Serum 2's knob rim `#1a1e20` on the `#8193a1` deck, **5.3:1**; Pigments' light theme
   5.5:1; Arturia's hardware) or give the body a **light rim**. Pro-Q 4's body is 1.15:1 against its
   panel and Vital's 1.7:1; they survive on the value arc alone. Cantus's knob is an outline circle on
   a track that measures **1.2:1** in both themes.
4. **Colour means function, per section; surfaces stay neutral.** Serum (oscillators green, filters
   blue), Vital (oscillators violet `#a490f8`, filters orange, modulation teal), Zebra 3 (a title colour
   per module type), Massive X (modulators by family: performers orange, envelopes blue, LFOs green,
   trackers purple) and Pigments (modulation families) all do it. Cantus uses gold for everything and a
   24-chip rainbow for its modulation sources (Unda).
5. **One frame, fixed positions, across the whole family.** Arturia (Pigments and every V Collection
   instrument), Soundtoys (every plug-in) and FabFilter put the **same header and footer** on every
   product: the preset field centred and wide, ‹ › beside it, undo/redo, the view tabs, and a footer
   with a help line, CPU and a **resize grip**. Everything is **scalable** (Pigments 50–200 % with a grip
   that snaps; Diva 70–200 %; Serum 2 and Soundtoys by corner drag or a size menu). Cantus's editors
   spend **75 px on chrome** (a 45-px egui title bar plus a 30-px header), repeat the device name twice,
   and **do not show the preset's name at all**.
6. **Light themes exist and can be first-class**: Pigments 7 (dark and light), Massive X (Default, Dark,
   Light and three Flat variants). Both keep **dark knob bodies** in the light theme and swap the accent
   per theme to hold contrast. Everyone else is dark-only.

The style guide in **§7** turns this into rules Cantus can paint in egui: a **param cell** (label above,
knob, value well below), a **section card** (24-px title bar with an LED, a title in tracked caps and a
type selector), **four knob sizes** with exact radii, **five type roles**, **six function hues** per
theme (all measured ≥ 4.5:1), and the header, footer and resize behaviour.

---

## 1. Screenshots

New files are in `screenshots/plugin-instrument-uis/` (git-ignored,
local only). Many more of the products studied were already saved by the instrument and FabFilter
research; they are cited from their folders rather than downloaded again.

### 1.1 Saved for this report

| File | What it shows | Source |
|---|---|---|
| `mx-01-massive-x-light-skin-main.png` | Massive X, light skin: 16 numbered macro knobs in a strip under the header, oscillator section with the waveform drawn *inside* the big knob, modulator family tags (P1–P3, E1–E7, L4–L9, T1–T4, VR) in family colours, dropdown values underlined | native-instruments.com (Massive X product page image) |
| `mx-02-massive-x-player-page.png` | Massive X Play page: Play / Edit / Browse tabs, centred preset name with ▲▼ and save, 8 macro knobs with Title-case labels, preset list, Animator XY | native-instruments.com (gallery) |
| `mx-03-massive-x-dark-and-light-skins.png` | The Dark and Light skins side by side, same layout | native-instruments.com (gallery "skins") |
| `mx-04-massive-x-filter-highlight-light.png` | Light skin with the filter section lit and the rest dimmed: one section's frame and surface emphasised | native-instruments.com (gallery "filters") |
| `k8-01-kontakt8-conflux-play-and-edit.png` | NI Conflux (Kontakt 8 factory): Play view (large animated visual, 6 knobs) and Edit view (tiles titled in tiny caps: SOURCE, AUDIO MOD, FILTER, LFO 1/2, ENVELOPES) | native-instruments.com (Kontakt 8 page) |
| `k8-02-kontakt8-play-series-hybrid-keys.png` | NI Play Series *Hybrid Keys*: a full-bleed colour panel, A/B preset pickers, one row of 8 knobs | native-instruments.com (Kontakt 8 included sounds) |
| `k8-03-kontakt8-factory-synths.png` | Kontakt 8 factory *Synths*: art panel above, one row of controls below (A/B, Sound, Cutoff, Glide, Monophony, Chord, Vol Envelope), line-ring knobs | native-instruments.com (Kontakt 8 included sounds) |
| `k8-04-kontakt8-shell-phrases-tool.png` | The Kontakt 8 shell: header (KONTAKT · FILE · LIBRARY · VIEW · SHOP · voices · **CPU / Disk**), side browser, the Phrases tool, an instrument panel and a coloured keyboard | native-instruments.com (Kontakt 8 Tools page) |
| `pg-01-pigments-value-popup-mod-chip.jpg` | Pigments detail: a **dark value plate** ("0.288") beside the knob while dragging, a "+" drag-to-modulate handle, the modulation source shown as a cyan chip **in place of the label** ("env 3"), modulator tiles | arturia.com (Pigments page) |
| `pg-02-pigments-shape-group-and-rules.jpg` | Pigments detail: engine header ("ENGINE 2 : Sample ▾"), a display that bleeds to the panel edges, a **group title "SHAPE"** and **hairline rules** tying pairs of knobs ("Tilt Offset — Tilt", "Depth — Section", "— A —") | arturia.com (Pigments page) |
| `st-01-decapitator.png` | Soundtoys Decapitator: the shared Soundtoys bar (‹ › red LED preset field, save, revert, menu, BYPASS), caps labels **above** knobs, range legends at the arc ends ("DARK / BRIGHT", "20 Hz / 1 kHz"), dot tick scales, A–E–N–T–P style buttons joined by a line | soundtoys.com |
| `st-02-echoboy.png` | EchoBoy: three rounded "pods" grouping controls, LED readouts, the pattern display in a bezel | soundtoys.com |
| `st-03-crystallizer.png` | Crystallizer: a pod, red LED value fields with ▲▼ steppers under three knobs | soundtoys.com |
| `st-04-little-alterboy.png` | Little AlterBoy: **three colour-blocked sections** (mustard PITCH/FORMANT, teal MODE, red DRIVE/MIX), a LINK button joined to both knobs by rules | soundtoys.com |
| `st-05-effect-rack.png` | Effect Rack: many Soundtoys devices stacked, each keeping its own face inside one shell | soundtoys.com |
| `st-06-filterfreak.png` | FilterFreak: pods per filter, a green scope display, legends on every knob | soundtoys.com |
| `st-07-superplate.png` | SuperPlate: a large hero Decay knob with a numeric scale and a red arc, "— FILTER —" group title with rules, min/max hooks ("0 ⌐ ¬ 250") | soundtoys.com |
| `st-08-sie-q.png` | Sie-Q: a light metal panel; every knob surrounded by its dB scale, "dB" unit under the knob, "— MID —" group title | soundtoys.com |
| `uhe-01-zebra3-modulation-rack.png` | Zebra 3 modulation rack: module cards titled in **large coloured Title case** (Env 1 blue, LFO 1 green, MSEG 1 yellow, Map 1 purple), labels above knobs, values in underlined fields | u-he.com (Zebra 3 page, converted from WebP) |

### 1.2 Reused from the other research folders

| File | What it shows | Source |
|---|---|---|
| `fam-01-serum2-main.png` | Serum 2 main page (2380 px) | xferrecords.com |
| `wt-02-serum2-wavetable-osc.png` | One Serum 2 oscillator section, close up | xferrecords.com |
| `fam-02-vital-main-frame.png` | Vital main page (a frame of the official video, 1600 px) | vital.audio |
| `fam-07`, `fam-08`, `fam-09` | Pigments 7: Play view dark and light, Synth page | arturia.com |
| `va-09-arturia-mini-v4-main.jpg`, `va-11`, `ky-10-stage73-v2-advanced.jpg`, `ky-12-b3-v2-advanced-drawbar-mod.jpg` | Arturia V Collection: the shared shell, hardware panels, and the flat "Advanced" panels | arturia.com |
| `fam-21-kontakt8-factory-instrument.png` | A Kontakt 8 factory instrument (Orchestral Tools) | native-instruments.com |
| `va-01-diva-main-layout.jpg` | Diva main page (1000 px) | u-he.com |
| `fm-15-zebra3-main.png`, `fm-14-zebra3-generator-rack.png` | Zebra 3 main page and generator modules | u-he.com |
| `pro-q-01-main.jpg`, `family-06-knob-modern-proc3.jpg`, `family-02-top-and-bottom-bars.png` | Pro-Q 4, the modern FabFilter knob, the shared bars | fabfilter.com |

---

## 2. Where Cantus's editors stand, measured

Details and code locations are in [`cantus-audit.md`](cantus-audit.md) §4.11–4.12. The numbers that
matter for this report:

| Element | Cantus today (Paper / Ink & Gold) | What the studied products do |
|---|---|---|
| Window chrome | egui title bar **45 px** (`#dcdcdc` / `#2d2d2d`, neutral greys outside the palette) with the device and track name at 16 px, then a 30-px header row that repeats the name. **The preset name is not shown**; there is a "Presets" button. | One 32–40 px @1x header with the preset name centred (§4.6) |
| Knob | body Ø30 outlined 1 px in `text_dim` (4.7:1), a 3-px track at Ø38 in `#d2d7df` / `#101218` (**1.22:1 / 1.19:1**), gap 1 px, value arc only from the default | dark body or light rim ≥ 3:1; track or arc ≥ 3:1 (§4.2) |
| Knob label | 10 px Plex Sans Regular, cap height **7 px**, `#626977` on `#e8ecf0` (**4.65:1**) / `#8e929d` on `#1c202c` (5.2:1), centred below | 11–12 px @1x caps or bold, 6.5–13:1 (or dim label + bright value) |
| Knob value | 10 px Plex Mono, cap ~6 px, **the same colour** as the label, on the next line | hidden at rest (Pro-Q, Serum, Vital, Pigments), or in a field or LED (Zebra, Soundtoys, Massive X) |
| Section title | "Dynamics", "Time", "Output": 10.5–11 px Regular (text height 10 px with the descender), sentence case, **the same colour as the labels**, no rule, no frame | a title bar, a card, a coloured title or a rule (§4.3) |
| Choices | text toggles with no container ("Clean Punch Opto…", "Room Plate Vintage Spring", "V1 V2 V3 C1 C2 C3") | segmented buttons, LED buttons, rotary selectors, menu fields |
| Colour | gold for arcs, selection, toggles, headings; up to 24 differently coloured modulation chips (Unda) | neutral surfaces; one hue per section function (§4.4) |
| EQ display share | 447 of 824 px of window height (**54 %**) under two toolbar rows | Pro-Q 4: the display is the window (≈ 90 %) |
| Bottom band (instruments) | ~230 of ~626 content px (37 %) in every instrument, whatever it holds | Serum 2: keyboard band ≈ 12 %; Pigments Synth: modulation ≈ 45 %, because modulation is that page's subject (§5) |

The Play views (Unda's 3-D wavetable, Calor's filter and voice cards, Organum's drawbars and rotor) are
good pictures. What makes the editors look plain is everything around the pictures: no frame, no
hierarchy, no objects.

---

## 3. Product by product

### 3.1 Xfer Serum 2 (fam-01, wt-02)

- **Structure.** Five page tabs (OSC · MIX · FX · MATRIX · GLOBAL) as tall square buttons with an LED
  dot, in the header. The OSC page is a row of sections (SUB · OSC A · OSC B · OSC C · NOISE · FILTER 1
  · FILTER 2), then modulation (MACROS, ENV 1–4, LFO 1–10 tabs with target counts, VELO, NOTE), then
  the keyboard band.
- **Framing.** Sections are **light blue-grey decks (`#8193a1`) separated by 4-px black gutters**
  (@2x, so 2 px @1x). The deck is light, the displays inside it are dark (`#283434`), the knobs are
  dark. This "light deck, dark objects" scheme is why Serum reads as hardware without any texture.
- **Section title row.** 32 px @2x, on the deck: a **green LED square** (`#5cd800`, the section's
  on/off), the name in **bold condensed caps** ("OSC A", `#22272b`, 4.6:1), the type as a centred menu
  ("WAVETABLE ▾"), and a small tag at the right ("F1" = routed to filter 1).
- **Knob.** Measured at DETUNE: a **60-px dark body** (rim `#1a1e20`, face `#47525c`) inside a **72-px
  soft halo** that carries the value mark in blue; two **min/max dots** at the ends of the travel; a
  white pointer line; a small **drag-to-modulate tab** at the top-left of the halo. Label: **bold
  condensed caps, cap height 14 px**, 8–10 px under the body. No value at rest.
- **Numbers.** Values live in their own **dark fields**: "OCT 0 · SEM 0 · FIN 0 · CRS –" (a caps
  label and a coloured value in one box), "UNISON 1 ▴▾", the envelope's "0.5 ms 0.0 ms 1.00 s…" row
  under its graph.
- **Colour.** Oscillator displays green/yellow, filter display cyan (`#00b4ff`, 5.7:1 on its display),
  preset name cyan (`#33c4ff`, 8.6:1), macros neutral, arpeggiator purple.
- **Header.** Logo tile (a menu: sizes and the default size), the five page tabs, the preset field
  (save icon, name in cyan with "*" when modified, ▼ browser, ‹ ›), a second row with ARTIST and DESC,
  a browser button, MENU, undo/redo, a MAIN volume knob and a stereo meter.
- **Resize.** Drag the lower-right corner, or pick a size from the logo menu and "Set x % as Default"
  (Xfer's manual page "Resizing the UI").

**What to take:** the light-deck / dark-object contrast, the LED-plus-caps section title row, labels in
bold caps, values in their own fields. **What not to:** the density of tiny fields and the 3.8:1 grey
metadata ("ARTIST:").

### 3.2 Vital (fam-02)

- **Framing.** Every section is a **rounded panel** (`#44484c`, radius ≈ 4 px at 1600 px) on a darker
  ground (`#202424`), with **3–4 px gutters**. Inside, sub-groups sit on slightly darker **rounded
  plates** ("PITCH −12 0", "UNISON 8v 7 %").
- **Section title.** The oscillator title is **rotated 90°** in a narrow spine at the panel's left
  ("OSC 1"), under a **violet power dot**. Filters use a normal title row with a power dot ("FILTER 1
  ‹ Ladder : Notch Spread ›").
- **Knob.** A dark body, a faint full track, a **coloured arc** in the section's colour, a **white
  pointer** from the rim inward. Diameter ≈ 42–44 px at 1600 px, 2.7 % of the width (Serum's is 2.5 %).
- **Label.** Caps, ≈ 10-px cap height, `#c2c6c8` (6.5:1), **each on its own darker pill-shaped
  plate** under the knob, the plate as wide as the cell. That plate is the single most effective device
  against "loose words": each label is visibly a part of one cell, and the plates align into a row.
- **Colour by section.** Oscillators violet (`#a490f8`), filters orange (fill `#94643c`), envelopes
  and LFOs teal (fill `#186870`). The skin system has about 60 tokens with **per-section overrides**
  (see `instruments/family.md` §4.8).
- **Displays** are **dark rounded insets** with the selector ("‹ Creepy Solar ›") **inside** the display
  at the top and the mode ("2D") inside at the bottom-left. The filter display carries its own blend
  slider along the top edge.
- **Resize.** Continuous, vector drawing.

**Take:** label plates, section hue on arcs and curves, selectors inside the display. **Not:** the rotated
titles (hard to read), knob bodies at 1.7:1.

### 3.3 Arturia: Pigments 7 and the V Collection (fam-07, fam-08, fam-09, pg-01, pg-02, va-09, ky-10, ky-12)

- **One shell for every product.** Pigments, Mini V, Jup-8 V, B-3 V and Stage-73 V have the same top
  toolbar (☰ · product name in its accent · a **centred preset pill**: library icon │ ♥ │ name with "*"
  │ ▲▼ · view tabs · settings) and the same footer (help line · play mode · undo / history / redo · CPU %
  · 4 macros in the V Collection · **resize grip**). A user who knows one knows all.
- **Measured (Pigments Synth page, 2000 px).** Toolbar 67 px; footer 46 px; panels `#2c2c2e` on
  **5–6-px near-black gutters** (`#070709`); preset name 20-px text height, `#fefeff` (17.6:1).
- **Section header row.** Power icon + **"FILTER 1 :"** in semibold caps (`#ffffff`, 14:1) + the type
  as a ‹ MS20 LP12 › selector. Inner groups get **dim caps sub-titles** ("UNISON", "GRANULAR",
  "MODULATOR") with their own ‹ › and power toggles, and **hairline rules** join related knobs ("Tilt
  Offset — Tilt"; "— A —" over a pair; pg-02).
- **Knob.** Resonance: a **42-px mid-grey body** (`#4a4a4c`) inside a **50-px thin track ring**,
  arc in cyan, a small dark pointer. Label **Title case**, 12-px ascender height, **near-white
  (13.4:1)**, 6–8 px under the ring. No value at rest; while dragging, a **dark value plate** appears
  beside the knob ("0.288", pg-01). A modulated knob shows its **source as a coloured chip in the
  label's place** ("env 3", "kbd", "velocity").
- **Colour by modulation family** (measured on fam-09): keyboard pink `#d46bb2`, envelopes orange
  `#d28e76`, LFOs yellow `#e2d08f`, functions green `#408448`, random purple `#7256bd` (**2.8:1, fails**),
  macros cyan `#46b3cf`. The bottom tabs carry the family colour as a 3-px top bar.
- **Displays** bleed to the panel edges (filter curves, sample waveform), no frame of their own.
- **V Collection hardware views** (Mini V4) are photographic, but the typography is disciplined: silk-
  screened caps labels above each knob, scale numbers around it, **section titles at the bottom in
  large caps** ("OSCILLATOR BANK", "MIXER", "MODIFIERS") with vertical rules between sections. The
  **Advanced panels** (Stage-73 V, B-3 V) are flat: dim caps group titles ("ORGAN MODEL", "PERCUSSION"),
  then **label … value rows** with values in the accent (`0.446`, `-64.0 dB`) in dark fields.
- **Resize** (Pigments 7 manual §3.3.1.7 and §3.8.5): **50 % to 200 %** from a menu; the corner grip
  resizes freely and **snaps to the nearest menu size on release**; a Maximize View button re-centres a
  window pushed off-screen.
- **Themes** (§3.3.1.8): "classic (dark)" and "light". In the light theme the knobs stay dark (5.5:1,
  `instruments/family.md` §4.14) and the accent changes per theme (the tips icon is yellow in dark mode
  and **purple in light mode**, manual §18.1). Notably, the **toolbar and footer stay dark** in the light
  theme (fam-08).

**Take:** the one shell, the section header row, near-white labels, the value plate while dragging, the
source chip in the label's place, hairline pair rules, the snapping resize, the per-theme accent. **Not:**
the decorative visualiser (Pigments itself ships "Disable Visualizer"), the failing idle and family
colours.

### 3.4 Native Instruments Massive X (mx-01–mx-04)

- **Structure.** A header row (MASSIVE X ▾ · search · bank · preset name centred · ▲▼ · save · output
  meter · NI), then a **macro strip** (PB, M, AT and **16 numbered macro knobs** with names under the
  first eight), the sound sections (Pitch · Oscillators · Noise · Filter · Insert FX · Amp · Stereo FX),
  a **modulator tag strip** (P1–P3, E1–E7, L4–L9, T1–T4, VR, each with a ✥ drag handle), then three
  modulator panels and a routing/tabs footer.
- **Framing.** Flat panels separated by 1-px lines; the **light skin** is `#c0c0c0` with a `#383838`
  header. mx-04 shows **focus by section**: the filter panel lit to `#e8e8e8`, the rest dimmed.
- **Knob.** White faces with a dark pointer on the light skin (dark faces for the Amp section), inside a
  **separate thin track ring with a wide gap**; modulation rings are **arcs in the modulator's family
  colour**, and the knob face carries the **modulation slot number in red** ("1", "5"). The big
  oscillator knobs **draw the wavetable shape inside the knob**: the knob is the display.
- **Label vs value.** Labels are **dim** (`#646365` on `#c0c0c0`, **3.3:1** light; `#8a9089` on
  `#303030`, 4.0:1 dark; both fail), values and menu choices are **bright** (`#202128`, 8.8:1;
  `#f1f8ee`, 12.2:1). Numbers use a **large integer and small decimals** ("0.000", "0.050").
- **Menus as text.** Every dropdown value is **underlined** ("Switcher LFO", "Gorilla", "Free").
- **Colour by family.** Performers orange, envelopes blue, LFOs green, trackers purple, voice random
  magenta; the tags and the rings use the same colours.
- **Skins and size.** Default, Dark, Light, and a Flat version of each (MusicTech, gearnews on 1.1);
  resizable window (NI product page).

**Take:** family colours shared by tag and ring, the knob-as-display for the main parameter, the
section focus highlight, light and dark skins of one layout. **Not:** the failing dim labels, the
underline-only menus (they look like links and have no boundary).

### 3.5 Native Instruments Kontakt 8 instruments (k8-01–k8-04, fam-21)

- **The pattern.** An instrument is a **picture** (library art, a 3-D render, a visual) taking 60–70 %
  of the panel, over **one row of 6–8 controls**: line-ring knobs (a 2-px ring with a pointer, Ø ≈ 30 px
  in k8-03), Title-case labels under them (10-px text height, `#bec18a` on `#2c3438`, 6.8:1), and pill
  menus ("‹› On", "‹› Off") beside them. Conflux adds an Edit view whose tiles are titled in **tiny
  caps centred at the top** with a power dot at left.
- **The shell** (k8-04) has its own header with **voices, memory, CPU and Disk** at the top right,
  a browser on the left and a coloured keyboard (key ranges tinted) at the bottom.
- **Resize.** Kontakt 7.4 added zoom levels, reported by users as raster scaling of the old assets
  **[forum]**; the browser is HiDPI.

**Take:** the Play-page grammar (one picture, one row of big controls) for Cantus's Play views.
**Not:** 7–8-px labels at typical widths (`instruments/family.md` §4.14), library-specific looks.

### 3.6 u-he Diva and Zebra 3 (va-01, fm-15, fm-14, uhe-01)

- **Diva** is two decks: a dark upper deck (`#3c3c38`, the sound path: TRIPLE VCO · MIXER · VCF |
  LADDER · ENVELOPES) and a **red lower deck** (`#843434`, LFOs · TUNING · AMPLIFIER | PAN · EFFECTS ·
  ARP), inside a wooden frame. Upper-deck section titles are **bold caps at the bottom of each
  section** with a ▼ that opens the model menu; lower-deck titles are **centred caps at the top**
  (`#f4f1ea` on red, 7.4:1). Labels **above** knobs in bold caps, **scale numbers around** each knob,
  settings in **small dark value fields** labelled beneath ("sine / Waveform", "gate / Restart").
  Bottom tabs: MAIN · MODIFICATIONS · TRIMMERS · SCOPE · PRESETS. Resizable **70 % to 200 %** (u-he
  product page).
- **Zebra 3** uses **module cards** (radius ≈ 8 px) on a dark ground. Each card's title is **large
  coloured Title case** (text height 17 px at 2000 px) in the **module type's colour**: Env blue
  `#8dc9e6`, LFO green `#c4db8c`, Filter yellow-green `#e5eecc`, MSEG yellow `#f5df98`, Mapper purple
  `#b885cc`; the **selected module gets a 2-px border** in the same colour. Parameters are a regular
  4–6-column grid: **label above** (grey, `#a09d9d` on `#444444`, **3.6:1**, fails), knob or **value
  field** below. The value field is **bright text on a slightly lighter bar with a 2-px underline**
  ("Pitch 1", "Wavetable", "200Hz": `#e4e2e3`, 7.6:1). Knobs are small bodies (Ø28) in a large track
  ring (Ø46).
- **Zebra 3's "adaptive interface"**: modules appear only when added to the patch (u-he product page),
  so a simple patch has a simple face.

**Take:** module title in the function colour, the selected-card border, a strict parameter grid,
value fields with a clear boundary, scale-around-the-knob for stepped values (Diva). **Not:** Zebra's dim
labels, Diva's texture.

### 3.7 FabFilter Pro-Q 4 (pro-q-01)

Covered in depth in [`../fabfilter/family.md`](../fabfilter/family.md) §3 and
[`../fabfilter/pro-q.md`](../fabfilter/pro-q.md). What matters here is **proportion**, which Cantus's
editors copied only in outline:

- the **display is the window** (about 90 % of the height, measured on pro-q-01), near-black with a
  warm tint (`#18151a` → `#1f1b20`);
- the controls float in **one small panel with a curved top edge** over the display, only for the
  selected band;
- the knobs in that panel are **large** (FREQ: 110-px outer ring at 2298 px ≈ 55 px @1x), with a thick
  yellow arc (4 px @2x) inside a lighter rim (`#585566`, 2.5:1) and a **label in near-white caps**
  (`#d6d5d8`, 12.1:1; 16-px text height @2x ≈ 8 px @1x), 17 px @2x under the ring;
- **one hue** (yellow) for arcs, the EQ scale and the result curve; band colours only for bands.

The body itself is 1.15:1 against its panel; the arc and rim carry the knob.

### 3.8 Soundtoys 5.5 (st-01–st-08)

- **One bar for every plug-in** (44 px at 1200 px): ‹ › · preset name in a **red LED field**
  (`#ca0000` on `#100000`, **3.4:1**, fails AA) · save · revert · Soundtoys menu · list · BYPASS.
- **Typography is the whole design.** Labels are **caps with wide tracking** (or small caps, "Freq",
  "Res"), **above** the knob; **range legends** at the two ends of the travel ("DRY / WET", "MIN / MAX",
  "20 Hz / 20 kHz", "−24 / +24"); a **tick or dot scale** around each knob; on SuperPlate, **hooks**
  that tie the legend to the end of the arc ("0 ⌐ ¬ 250"). A label, its knob, its scale and its legends
  are one printed unit; nothing floats.
- **Grouping** by rounded **pods** (EchoBoy, FilterFreak, Crystallizer), **colour blocks** (Little
  AlterBoy: mustard, teal, red), **group titles with rules** ("— FILTER —", "— MID —"), and **link
  buttons joined to both knobs by lines** (LINK between PITCH and FORMANT; the A–E–N–T–P style chain).
- **Readouts**: 7-segment LED fields under the knobs that need an exact value (Crystallizer's "1200
  cents", PrimalTap's "0.25 beats"), with ▲▼ steppers.
- **Resize** (Soundtoys 5.5): "Interface Size" in the menu, or drag the lower-right corner.

**Take:** legends and scales that make a knob self-explanatory at rest; group titles with rules; link
lines; LED-style readouts for the few values that need exactness. **Not:** the wood, the screws, the
photoreal knobs (raster art that does not theme), the red-on-black LED preset field.

---

## 4. Across the products, topic by topic

### 4.1 How panels are framed and grouped

| Device | Who | Measured |
|---|---|---|
| **Cards on a darker ground, with gutters** | Serum 2, Vital, Pigments, Zebra 3, Massive X | gutters 2 px @1x (Serum), 3–4 px (Vital), 5–6 px @2x (Pigments); card vs ground: Pigments `#2c2c2e` / `#070709`, Vital `#44484c` / `#202424` |
| **Rounded corners** | Vital (≈ 4 px), Zebra (≈ 8 px), Pigments (small, ≈ 2–4 px), Soundtoys pods (large, pill-shaped) | — |
| **Light deck, dark objects** | Serum 2 (deck `#8193a1`, knobs and displays dark), Arturia hardware, Diva, Massive X light | knob rim on deck 5.3:1 (Serum) |
| **Sub-plates inside a card** | Vital ("PITCH", "UNISON" plates), Serum (value fields), Pigments (dark chips) | — |
| **Colour blocks per section** | Little AlterBoy, Diva's red lower deck | — |
| **Group rules** (a title between two hairlines, or a line joining two controls) | Soundtoys ("— FILTER —", "— MID —", LINK), Pigments ("— A —", "Tilt Offset — Tilt") | — |
| **Focus/selection of one section** | Massive X (the selected section lit, the rest dimmed), Zebra 3 (a 2-px border in the module colour) | — |

**The common rule:** a section is visibly a *thing* (a card, a pod, a block) before you read anything in
it; its title is attached to its top edge (or bottom, on hardware panels); and the gaps *between* sections
are larger and darker than the gaps *inside* them.

### 4.2 Knobs: sizes, arcs, readouts and labels

**Measured anatomy** (native pixels of each screenshot; "ring" = outer diameter including the track):

| Product | Image width | Ring Ø | Body Ø | Arc | Label text height | Ring ÷ label | Label colour, contrast | Value at rest |
|---|---|---|---|---|---|---|---|---|
| Pro-Q 4 (FREQ) | 2298 | 110 | ≈ 100 | 4 px, inside the rim, yellow | 16 (caps) | 6.9 | `#d6d5d8` on `#18181c`, **12.1:1** | no (popup) |
| Serum 2 (DETUNE) | 2380 | 72 (halo) | 60 | a blue mark in the halo | 14 (caps, bold condensed) | 5.1 | `#22272b` on `#8193a1`, 4.6:1 | no; separate value fields |
| Vital (env) | 1600 | ≈ 44 | ≈ 34 | section colour | ≈ 10 (caps) | 4.4 | `#c2c6c8` on its plate `#383c3c`, 6.5:1 | no |
| Pigments (Resonance) | 2000 | 50 | 42 | cyan, on the ring | 12 (Title case) | 4.2 | `#fafafb` on `#2c2c2c`, **13.4:1** | no; plate while dragging |
| Zebra 3 (Detune) | 2000 | 46 | 28 | on the ring | 12 (Title case) | 3.8 | `#a09d9d` on `#444444`, **3.6:1** ✗ | fields for menus |
| Massive X (Bend, light) | 1920 | ≈ 67 | ≈ 33 | modulation arcs | ≈ 12 | ≈ 5.4 | `#646365` on `#c0c0c0`, **3.3:1** ✗ | numbers for pitch |
| Kontakt 8 *Synths* | 1040 | 30 | (line ring) | none | 10 | 3.0 | `#bec18a` on `#2c3438`, 6.8:1 | no |
| Soundtoys (TONE) | 1200 | 105 (+ dot scale 150) | 105 | none; ticks | 10 (tracked caps) | 10.5 | `#c7c7c7` on `#202024`, 9.6:1 | legends; LEDs for some |
| **Cantus (Ratio)** | 1500 @1x | **38** | **30** | 3 px from default | **7** | **5.4** | `#626977` on `#e8ecf0`, **4.65:1** | yes, 10-px mono, same grey |

**Reading it.**
- **Size ratio.** A ring about **4× the label's text height** is the norm (3.0–5.4 across the six
  dense instruments, median ≈ 4.3; big "hero" knobs go to 7–10). Cantus's 5.4 sits at the small-label
  end, with Massive X. At a 36-px knob the median is an 8-px text height, i.e. an **11–12 px font** in
  IBM Plex (cap height 0.698 em), not 10.
- **Weight and case.** Caps (Pro-Q, Serum, Vital, Soundtoys, Diva) or a medium weight (Pigments,
  Massive X). Only Zebra 3 and Massive X use dim mixed-case labels (3.3–3.6:1, failing), and both put a
  **bright** value (7.6–12.2:1) right under them. Nobody leaves label and value in the same dim tone, as
  Cantus does.
- **Where the value goes.** Two schools:
  1. **hidden at rest**, shown on hover or drag, in a popup or plate (Pro-Q, Serum, Vital, Pigments);
  2. **always shown, in its own container** (Zebra's underlined fields, Serum's value boxes, Soundtoys'
     LEDs, Arturia's Advanced panels, Ableton's devices in `instruments/family.md` §4.1).

  Nobody prints a bare value line under the label in the same colour. Cantus's principle "state visible
  at rest" (`fabfilter/family.md` §7.1) points to school 2, and school 2 needs a **container** for the
  value.
- **Arcs.** Every modern knob has one: on the track ring (Pigments, Vital, Zebra), inside the rim
  (Pro-Q), or a mark in a halo (Serum). Colour = the section's or product's hue.
- **Bodies.** Dark body on a light deck (Serum, Arturia, Massive X Amp, Pigments light) gives 4.7–5.5:1
  and reads at a glance. Dark-on-dark bodies (Pro-Q 1.15:1, Pigments dark 1.55:1, Zebra 1.8:1, Vital 1.7:1)
  rely on the arc and a lighter rim.
- **Extras that pay:** min/max dots at the ends of travel (Serum), a tick scale for stepped or
  calibrated knobs (Soundtoys, Diva, Arturia), a drag-to-modulate tab on the knob (Serum, Pigments "+"),
  the modulation source chip in the label's place (Pigments), a **knob-as-display** for the one main
  parameter (Massive X's wavetable knobs, Pro-R 2's glyph inside the knob).

### 4.3 How sections are titled

| Style | Who | Details |
|---|---|---|
| **Title row on the card's top edge**: power LED/icon + TITLE in bold caps + type selector | Serum 2, Pigments, Vital (filters) | Serum: 32 px @2x, LED `#5cd800`; Pigments: "FILTER 1 :" semibold caps, ‹ type › centred |
| **Large coloured title, top-left of a card** | Zebra 3 | Title case, colour = module type, ≈ 1.4× the label size |
| **Tiny centred caps at the top of a tile** | Kontakt 8 Conflux, Arturia Advanced ("ORGAN MODEL") | dim caps; works only with a card behind it |
| **Caps title at the bottom of a section, hardware style** | Diva (upper deck), Arturia Mini V ("OSCILLATOR BANK") | with a disclosure ▼ for the model menu (Diva) |
| **Group title between rules** | Soundtoys ("— FILTER —"), Pigments ("— A —") | for a sub-group of 2–3 controls inside a section |
| **Rotated spine** | Vital (OSC 1) | saves width; hard to read |
| **Name + underlined type menu** | Massive X ("Modulator 4 · Switcher LFO") | dim small name at left, type centred |

**The common rule:** the title is a *label of a container*, attached to the container's edge, in a
style no parameter label uses (bigger, bolder, coloured, or on a bar). Cantus's "Dynamics" is the same
size, weight and colour as "Threshold" beneath it, with nothing to attach to.

### 4.4 Colour accents per section

| Product | Rule | Colours measured |
|---|---|---|
| Serum 2 | by function | oscillators green (LED `#5cd800`, displays green/yellow), filters cyan `#00b4ff`, brand cyan `#33c4ff` for the preset |
| Vital | by function, skinnable per section | oscillators violet `#a490f8`, filters orange, modulation teal |
| Zebra 3 | by module type, on the title and the selection border | Env `#8dc9e6`, LFO `#c4db8c`, Filter `#e5eecc`, MSEG `#f5df98`, Mapper `#b885cc` |
| Massive X | by modulator family, on tags and rings | P orange, E blue, L green, T purple, VR magenta |
| Pigments | by modulator family; engines and filters in the brand cyan | KBD `#d46bb2`, ENV `#d28e76`, LFO `#e2d08f`, FUNC `#408448`, RAND `#7256bd`, MACRO `#46b3cf` |
| Pro-Q 4 / FabFilter | one hue per product; a rotation for bands | Pro-Q yellow `#ffec36` / `#ffd844` |
| Soundtoys | one look per product; colour blocks per section in Little AlterBoy | — |
| Diva | two deck colours (dark sound path, red modifiers) | `#3c3c38`, `#843434` |

**The common rule:** surfaces are neutral; **colour is spent on the few marks that carry meaning**:
LEDs, value arcs, curves, the title or the tag. A colour means the *same function* everywhere it
appears (Massive X's green is "LFO" on the tag, the ring and the modulator panel). Pigments' family
colours fail for "random" (2.8:1): colour sets must be checked against the surface they sit on.

### 4.5 How displays are integrated

| Pattern | Who |
|---|---|
| **The display is the section's face**, directly under the title row, bleeding to the card's left and right edges; the section's controls in rows below | Serum 2 (oscillators, filter), Pigments (engines, filters, modulators), Zebra 3 (filter card's left third) |
| **A dark rounded inset** with the selector and mode *inside* it | Vital ("‹ Creepy Solar ›" at the top, "2D" at the bottom-left) |
| **The display is the window**, with one floating control panel | Pro-Q 4, and the FabFilter family |
| **The knob is the display** | Massive X (waveform inside the oscillator knob), Pro-R 2 (glyph in the knob) |
| **Hardware bezels** (VU meter, CRT scope) | Soundtoys, Arturia |
| **Tiles as mini-displays** (each modulator draws its live shape) | Pigments (25 tiles), Serum (LFO tabs), Zebra (MSEG preview) |
| **A picture above one row of controls** | Kontakt 8 instruments, Pigments Play view, Massive X Play |

Displays are always **dark in dark products** and **dark inside Serum's light deck**. Pigments' and
Massive X's light themes keep light display backgrounds with darker curves.

### 4.6 Header and preset bars

| Product | Left | Centre | Right | Height |
|---|---|---|---|---|
| Serum 2 | logo (menu: sizes), 5 page tabs with LEDs | save · **preset name** (cyan, `*` when modified) · ▼ · ‹ › ; ARTIST / DESC row | browser, MENU, undo/redo, MAIN knob + meter | ≈ 65 px @1x (two rows) |
| Vital | logo, 4 page tabs (underline on the active one) | ‹ **preset** › · save · menu | output bar, oscilloscope | ≈ 55 px at 1600 |
| Pigments / V Collection | ☰, product name in accent | pill: library │ ♥ │ **name** + `*` │ ▲▼ | PLAY · SYNTH · FX · SEQ, tips, master knob + meter, ⚙ | 67 px @2x ≈ 34 @1x |
| Massive X | logo ▾, Play · Edit · Browse | search · bank · **preset** · ▲▼ · save | output meter, NI logo | ≈ 42 px in 1454, + a 52-px macro row |
| Kontakt 8 | KONTAKT · FILE · LIBRARY · VIEW | — | SHOP · voices · memory · **CPU · Disk** · NI | instrument panels add their own title |
| Diva | Save, logo, MIDI activity | ‹ **preset** › with its category ("OSC Model") | undo/redo, Multicore, Output knob, ⚙ | ≈ 55 at 1000 |
| Zebra 3 | logo, Synthesis · Pitches · Presets | the focused module ("LFO 1 ◂ ▸") | save, undo/redo, volume, version, ⚙ | — |
| Pro-Q 4 | logo tab | undo/redo · A/B Copy · ‹ **preset** › | Help, full screen | 36–40 pt |
| Soundtoys | ‹ › | red LED **preset** field · save · revert | menu (sizes), list, BYPASS | 44 at 1200 |

**Seven of the nine show the preset name, centred or near-centre, as the largest text in the bar,
with ‹ › or ▲▼ next to it and (where visible) a modified marker**; the Kontakt shell leaves it to each
instrument, and Zebra 3's centre slot names the focused module. Pigments' footer adds the **help line** (the
hovered control's name and purpose, 16.7:1), play mode, undo history, CPU % and the grip.

### 4.7 Scalable, resizable windows

| Product | How | Range | Notes |
|---|---|---|---|
| Pigments / Arturia | menu **and** corner grip that **snaps to the nearest menu size** | 50–200 % | Maximize View button; per-product default |
| Serum 2 | corner drag, or the logo menu; "Set x % as Default" | — | skins from the same menu (Serum 1 did; the Serum 2 page read documents sizes only) |
| Vital | continuous corner drag | — | vector (OpenGL) drawing |
| Massive X | resizable window | — | 6 skins |
| Diva (u-he) | size menu | 70–200 % | — |
| Soundtoys 5.5 | "Interface Size" menu, or drag the lower-right corner | — | added to every plug-in in 5.5 |
| FabFilter | size presets + a separate scaling submenu; free resize in VST3/CLAP; full screen | — | remembered per monitor type (`fabfilter/family.md` §3.12) |
| Kontakt 8 | zoom levels (7.4) | — | raster upscaling of old assets, per user reports **[forum]** |

**The common rule:** the layout is designed at one size and **scaled uniformly** (text and controls
together); free-aspect resizing is reserved for display-centric products (Pro-Q gives extra width to
the display). A snap to a few sizes keeps text crisp and gives users a size to return to.

### 4.8 Light and dark variants

- **Pigments** (dark and light): same layout; light panels `#e8dcdc`-ish, **knobs stay dark** (`#565656`,
  5.5:1), labels `#3e3735` (8.6:1); the accent is re-chosen per theme; the top and bottom chrome stay
  dark.
- **Massive X** (Default mid-grey, Dark `#303030`, Light `#c0c0c0`, plus Flat variants without
  shading). The same colour families on both; labels fail in both.
- **Serum 2** is a hybrid: dark chrome and displays, **light control decks**. It shows that a light deck
  is not a "light theme" problem but a contrast tool.
- **Vital** and **Surge XT** (see `instruments/family.md` §4.8) have token-based skins.
- **Dark only:** FabFilter, Zebra 3, Diva (textured), Kontakt shells, Soundtoys (each product has its
  own fixed face; Sie-Q is a light metal panel).

**The common rule:** a light theme is a second palette for the *same* layout, with **darker saturated
accents** (a colour that glows on black is unreadable on white) and **dark control bodies**.

### 4.9 Typography: how they avoid "loose words"

What the studied products do, and Cantus does not:

1. **Two or three roles, each unmistakable.** Section title (bar, colour or size) ≠ parameter label
   (caps or bold) ≠ value (mono or bright, in a field). Cantus: one role.
2. **Every text has a container or an anchor**: a label plate (Vital), a value field (Zebra, Serum),
   a pod or block (Soundtoys), a title bar (Serum, Pigments), a legend hooked to the arc (Soundtoys).
3. **A grid of equal cells.** Knob rows are equal-width columns (Zebra's 4–6 columns, Serum's 4-knob
   rows, Vital's 6 envelope knobs on plates). Labels are centred on the *cell*, and the cell is wider
   than the widest label: Serum and Vital **abbreviate** systematically (WT POS, ATK, DEC, REL, KEY TRK,
   RES, FIN, SEM, OCT).
4. **Caps with tracking** for labels (Soundtoys' wide tracking, Pro-Q's tracked caps), so a word reads
   as a label, not as prose.
5. **Numbers are set as numbers**: tabular, often monospaced or LED-like, with units smaller or
   dimmer (Massive X's "0.000" with small decimals; Arturia's "−64.0 dB" in the accent).
6. **No sentences on the panel.** Descriptions go to the help line (Pigments' footer) or a tooltip.

### 4.10 Moving around fast inside a device (and what does not apply)

- **One page or pages.** One-page instruments (Vital, Massive X's main page) are praised for speed;
  paged ones (Serum's five tabs, Diva's MAIN / MODIFICATIONS / TRIMMERS) for depth
  (`instruments/family.md` §4.1). Zebra 3 shows a third way: modules appear only when the patch uses
  them, so the page is as long as the sound needs.
- **Hover tells you what you are on:** knobs brighten (FabFilter), a value plate appears (Pigments),
  the footer help line names the control and its purpose (Pigments; Bitwig names the gesture).
- **Every number can be typed** (FabFilter's parser, Serum's and Vital's fields); Cantus's right-click
  entry and parser already match this (`fabfilter/family.md` §7.3).
- **Presets step without opening anything:** ‹ › or ▲▼ beside the name in every product.
- **Drag handles are visible objects:** Serum's tab on the knob halo, Pigments' "+", Massive X's ✥ on
  every modulator tag.
- **Keyboard control of knobs is rare** (FabFilter documents none; Surge XT is the exception); Cantus's
  arrow, Page and Home/End rules are already ahead.
- **Windows.** Every plug-in studied is a floating window owned by its host; docking is the host's job
  (Live's and Bitwig's device panels). Cantus's plan (inspector strip + editor window + a dock button)
  is in `instruments/family.md` §6.2 and is not changed here.
- **The mixer** questions of the brief (strips, sends, scrolling) do not apply to this topic; they are
  covered by the DAW reports in this folder.

---

## 5. Proportions: where the space goes

| Product | Display(s) | Controls | Modulation | Keyboard | Chrome (header + footer) |
|---|---|---|---|---|---|
| Serum 2 OSC page | ≈ 20 % | ≈ 23 % | ≈ 35 % | ≈ 12 % | ≈ 9 % |
| Pigments Synth | ≈ 17 % | ≈ 25 % | ≈ 45 % (tiles + editor) | on Play only | ≈ 9 % |
| Pro-Q 4 | ≈ 90 % (one panel floats over its lower part) | (floating) | — | — | ≈ 9 % |
| Cantus EQ (Aequalis) | 54 % | ≈ 16 % | — | — | **≈ 20 %** (title 47 + header ≈ 33 + two toolbar rows) |
| Cantus instruments | varies | varies | ≈ 37 % fixed band | (in the band) | ≈ 11 % (title + header) |

(Percentages of window height, estimated from the screenshots' section boundaries.)

---

## 6. What Cantus should take, ranked, and what not to copy

### 6.1 Take (in order of impact on "plain and ugly")

1. **Section cards with title bars** (Serum, Pigments, Zebra) on a darker ground with 4-px gutters.
   One change turns every editor from "a sheet with words" into "a panel of modules".
2. **The param cell** (label + knob + value as one fixed-width unit on a grid), with labels in
   **tracked semibold caps** at 11 px and values in a **recessed value well** (Zebra, Serum, Vital's
   plates). This is the direct cure for "loose words" and for labels running into each other.
3. **A knob that is an object**: a dark body on light decks (a lighter rim on dark decks), a ≥ 3:1
   track, the value arc in the section hue, a white pointer (Serum, Pigments light, Arturia).
4. **Segmented controls, LED buttons and menu fields** for every choice (Serum's page tabs and fields,
   Diva's value fields, Soundtoys' LED buttons). No bare words.
5. **Function hues per section** (Serum, Vital, Zebra, Massive X), checked for contrast per theme;
   neutral surfaces; gold kept for Cantus's own focus and brand.
6. **One shell**: a 32-px header with the preset name centred and ‹ ›, a 28-px footer with the help
   line, chips, meter and a resize grip; the egui title bar removed (Arturia, Soundtoys, FabFilter).
7. **Displays as the section's face**, flush under the title bar, with their selectors inside them
   (Serum, Pigments, Vital); the EQ and other display-centric effects give the display ≥ 70 % of the
   height (Pro-Q).
8. **Scaling**: uniform scale with snap sizes 75–200 % and a corner grip (Pigments), remembered per
   device kind.
9. **Legends and scales** where they explain more than a number: range legends at the arc ends for
   bipolar and unit-less knobs, tick scales for stepped knobs (Soundtoys, Diva), hairline rules for
   linked pairs (Pigments, Little AlterBoy).
10. **Play pages** built as one picture plus one row of large controls (Kontakt 8, Pigments Play,
    Massive X Play) — Cantus's Play views already have the picture.

### 6.2 Do not copy

| Pattern | Why not |
|---|---|
| Wood, screws, photographic knobs, brushed metal (Soundtoys, Arturia hardware, Diva) | raster art that does not scale or theme, and fights an editable palette. Take their *typography and grouping*, not their textures. |
| Dim labels (Zebra 3.6:1, Massive X 3.3–4.0:1), dim idle states (Pigments 2.8–3.3:1), red LED preset text (Soundtoys 3.4:1) | fail WCAG; Cantus's theme test would reject them |
| Near-invisible knob bodies (Pro-Q 1.15:1, Vital 1.7:1) | the knob disappears at rest (`fabfilter/family.md` §6 "camouflage") |
| Rotated section titles (Vital) | slow to read; Cantus's localisation (ES) makes titles longer |
| Underline-only menus (Massive X) | read as links, no control boundary |
| Decorative visualisers (Pigments' Play view) | cost attention and CPU; Pigments ships an off switch |
| Library-specific looks per instrument (Kontakt) | the family would stop looking like one product |
| Tiny text to fit a fixed panel (Kontakt libraries at 7–8 px, Serum's metadata) | Cantus's floor is 11 px for labels, 10 px for legends |
| A rainbow of per-instance colours with no family grouping (Cantus's current 24 chips in Unda) | colour stops meaning anything; use family hue + tag (Massive X, Pigments) |

### 6.3 What "better than" would mean, measurably

- **Every token pairing passes** in all three shipped themes and in user palettes (text 4.5:1, knob
  boundary and track 3:1, arcs 3:1), checked by the existing theme test. Of the products studied, only
  Pigments' light theme comes close.
- **No label is ever clipped or overlaps** at any scale and in both locales (EN, ES), checked by a test
  that measures every param cell's galleys.
- **Every value is visible at rest and editable in place** (drag, wheel, type) — which Pro-Q, Serum,
  Vital and Pigments do not offer at rest.
- **One frame for 15+ devices**, effects and instruments alike, with the same header, footer and help
  line (as Arturia and Soundtoys manage across their catalogues), in light and dark.
- **Scales cleanly 75–200 %** with crisp text (vector painting; Kontakt's raster zoom is the counter-
  example).

---

## 7. The Cantus device style guide

For **all effect and instrument editors**, their inspector strips and the generic hosted-plug-in frame.
Everything below is **custom painting in egui 0.36** with the existing `design` and `theme` modules;
nothing needs a new dependency. Sizes are logical px at 100 %; every size is multiplied by the editor's
scale `k` (§7.11).

### 7.1 Principles

1. **Cells, cards, shell.** Every control is a *cell*; cells sit in *cards*; cards sit in the *shell*
   (header, body, footer). Nothing is painted outside a cell or a card.
2. **Three text roles you can tell apart without reading**: title (on a bar), label (tracked caps),
   value (mono, in a well).
3. **Objects on neutral decks.** Knob bodies, wells and LEDs contrast with the deck; decks contrast
   with the ground.
4. **Colour is function.** Six function hues, one per section kind; surfaces are neutral; gold is
   Cantus's own accent (focus, brand), not a state colour.
5. **Measured, not hoped.** Every size comes from the tables below and every colour pairing is in the
   theme test.

### 7.2 Palette roles (add to `theme::Palette`)

New roles, with values measured against their neighbours (script in the scratchpad; all pairs listed
were checked):

| Role | Paper | Parchment | Ink & Gold | Used for | Checks |
|---|---|---|---|---|---|
| `device_ground` | `#dde1e7` | `#d9d2c3` | `#0b0d12` | the body behind the cards (gutters) | deck vs ground 1.24 / 1.31 / 1.18 |
| `deck` | `#f7f8fa` | `#f3efe6` | `#1b1f28` | card face | existing `text` on it: 17.0 / 15.0 / 12.8:1 |
| `deck_title` | `#eceff3` | `#e8e2d5` | `#232834` | card title bar, value wells, segmented background | one visible step from `deck` |
| `well` | `#ffffff` | `#fbf8f0` | `#12151c` | displays, text entry | — |
| `well_edge` | `#858d9a` | `#8a8170` | `#6b7385` | display and field outlines, dividers that bound a control | 3.15 / 3.35 / 3.47:1 vs deck |
| `label` | `#3b414c` | `#3f3a31` | `#c9ccd3` | parameter labels, section titles | 9.7 / 9.8 / 10.3:1 on deck; ≥ 7.5:1 on ground; ≥ 8.7:1 on `deck_title` |
| `text_dim` (existing) | `#5b6270` | `#666158` | `#8f939e` | legends, axis ticks, help line | 5.8 / 5.4 / 5.4:1 on deck; ≥ 4.7 on `deck_title` |
| `knob_body` | `#353b46` | `#3a342b` | `#2c313c` | knob body, selected segment (light themes) | **10.6 / 10.7:1** vs deck; dark: 1.27 (the rim carries it) |
| `knob_rim` | — (none) | — (none) | `#8f97a8` | 1-px rim on dark bodies | **5.6:1** vs deck |
| `knob_track` | `#858d9a` | `#8f8674` | `#6b7385` | the 270° track | **3.15 / 3.14 / 3.47:1** vs deck |
| `knob_pointer` | `#ffffff` | `#fbf8f0` | `#ffffff` | pointer line | 11.3 / 11.6 / 13.0:1 vs body |

**Function hues** (`hue_*`). Every one is **≥ 4.5:1 on `deck`, `deck_title` and `well`** in its theme,
so it can colour text as well as arcs:

| Function | Paper | Parchment | Ink & Gold | Sections that use it |
|---|---|---|---|---|
| `hue_source` (blue) | `#2f62c8` | `#2c5bb8` | `#74a7ff` | oscillators, generators, samples, sample zones, operators, drawbars |
| `hue_tone` (teal) | `#0d736d` | `#0e7069` | `#3fc2b6` | filters, EQ bands' panel, formant, de-esser detection |
| `hue_dynamics` (amber) | `#8f5c00` | `#875c08` | `#e0ae52` | amp envelope, gain, compressor, gate, limiter, drive |
| `hue_mod` (violet) | `#7a4cc2` | `#7045b3` | `#a98bfa` | LFOs, envelopes as sources, macros, modulation matrix |
| `hue_space` (rose) | `#b0457a` | `#a13d6e` | `#f07ab8` | delay, reverb, chorus, rotary, stereo, width |
| `hue_output` (olive) | `#4f6b2c` | `#4a6428` | `#9ccf6a` | voice, global, output, routing, utility |

Measured minimums: Paper 4.58:1 (space on `deck_title`), Parchment 4.56:1 (dynamics on
`deck_title`), Ink & Gold 5.45:1 (mod on `deck_title`). The hues were chosen as six clearly separate
hue families, but they have **not** been run through a colour-blindness simulation; every use pairs them
with a title or tag, so hue is never the only cue. The **device hue** (the one per device of
`instruments/family.md` rule V4) is chosen *from this set* by the device's main function (Aequalis tone,
Comprimo dynamics, Spatium space, Unda source…), so the family stays coherent. **Modulation source
instances** keep their colour-blind-checked rotation (`fabfilter/family.md` §7.4.3) but are grouped by
family: the chip carries the family hue as a bar and the instance tag as text (§7.8).

**Rule of use:** a function hue may colour a card's LED, its value arcs, its display curve, a selected
segment's underline, and its title *text* only if the title bar is `deck_title` (all pass). It never
fills a surface larger than a chip.

### 7.3 Type roles

IBM Plex, as bundled, plus **Plex Sans Medium** (same OFL family; add
`assets/fonts/IBMPlexSans-Medium.ttf`). Optional later: **Plex Sans Condensed SemiBold** for
labels in the densest editors (it keeps caps inside S-size cells; verify widths with the §7.13 test).

| Role | Size | Face | Case, tracking | Colour | Where |
|---|---|---|---|---|---|
| `dev.name` | 13 | Plex Sans SemiBold | CAPS, +1.0 px | device hue | header wordmark |
| `dev.preset` | 14 | Plex Sans Medium | as typed | `text` (never dimmed; italic + dot when modified) | preset field |
| `dev.control` | 12 | Plex Sans Medium | Title case | `text` | buttons, segmented controls, menu fields, tabs |
| `sec.title` | 11 | Plex Sans SemiBold | CAPS, +0.8 px | `label` | card title bars, group titles |
| `param.label` | 11 | Plex Sans SemiBold | CAPS, +0.5 px | `label` | above every knob and fader |
| `param.value` | 12 | Plex Mono Medium | as formatted; unit after a thin space | `text`; hue while dragging | value wells, fields |
| `param.legend` | 10 | Plex Sans Medium | CAPS, +0.3 px | `text_dim` | range legends, tick numbers, units, axis ticks |
| `readout.hero` | 18 | Plex Mono Medium | — | `text` | XL knob centre, hero values (Space "2.40 s") |
| `help` | 11 | Plex Sans Regular | sentence | `text_dim` | footer help line, empty states |

- Five sizes (10, 11, 12, 14, 18). The 10 is used **only** for legends and ticks, which differ from
  labels in weight, colour *and* position, so 10 beside 11 does not read as a mistake (the audit's
  complaint about 10 beside 10.5).
- **Tracking** in egui: `egui::text::TextFormat { extra_letter_spacing: 0.5, .. }` in a `LayoutJob`;
  caps are produced with `to_uppercase()` (locale-aware: `"resonancia".to_uppercase()`).
- **Abbreviations** live in i18n next to the full names (EN and ES), for S and M cells: `ATK DEC SUS
  REL`, `RES`, `FREQ`, `THRESH`, `DET`, `POS`, `KEY TRK`… The full name is always in the help line and
  tooltip.
- **Numbers:** mono, reserved width for the widest value of the parameter (`"−100.0 dB"`), units in
  `text_dim` when the value is in a well ≥ 64 px wide.
- A test bans `FontId::new` outside `design.rs` (as the audit proposes) and asserts that every role
  above exists.

### 7.4 Spacing and the grid

All on the 4-px grid (`design::space`):

| Token | Value | Where |
|---|---|---|
| Header height | 32 | device header (shell) |
| Footer height | 28 | device footer (shell) |
| Body padding | 8 | around the card grid |
| Card gutter | 4 | between cards (shows `device_ground`) |
| Card radius | 6 | cards; displays and wells inside use 4, chips 3 |
| Card title bar | 24 | inside the card, top |
| Card padding | 12 left/right, 10 below the title bar (or below the display), 12 bottom | inside the card |
| Cell gap | 0 horizontal (cells include their own side space), 12 between rows | knob rows |
| Group rule | 16 tall | "— FILTER —" style sub-group title |
| Control heights | 24 (M: buttons, segmented, menu fields), 20 (S), 18 (value well M) | — |

### 7.5 The shell

```
┌─ header 32 ──────────────────────────────────────────────────────────────────────────────────────┐
│ ▲ CALOR  ■ Keys │ ‹ [☆ Glass Pluck ●  ▾] › 💾 │ [A|B] → │ ↶ ↷ │ [Play|Edit|Matrix] ···· ⏻ ? ⋯ ⤢ │
├─ body: cards on device_ground, 8 padding, 4 gutters ─────────────────────────────────────────────┤
│ ┌■ OSC 1 ─ WAVETABLE ‹ Vowels › ⋯┐┌■ FILTER ─ ‹ Ladder 24 › ⋯┐┌■ AMP ENV ──────────────┐          │
│ │ (display, bleeds to the edges) ││ (curve)                  ││  ATTACK  DECAY  SUS …  │          │
│ │  POS   DETUNE  UNISON  LEVEL   ││  CUTOFF   RES   DRIVE    ││   ◯      ◯      ◯      │          │
│ │   ◯      ◯       ◯      ◯      ││    ◯       ◯      ◯      ││ [4 ms] [200ms] [80 %]  │          │
│ │ [8/25] [+18ct]  [ 4 ]  [-4 dB] ││ [2.4k]  [30 %] [+3 dB]   │└────────────────────────┘          │
│ └────────────────────────────────┘└──────────────────────────┘                                    │
├─ footer 28 ──────────────────────────────────────────────────────────────────────────────────────┤
│ ? Cutoff · the filter's corner frequency · drag, Shift fine, dbl-click reset │ Voices 5/8 · CPU 3.1 % · Out ▮▮▮▯ −6.2 dB ◢ │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **No egui title bar.** `egui::Window::title_bar(false)`; the header *is* the title bar. The window
  moves when its header background is dragged; ✕ closes it (in ⋯ and as Esc). This recovers 45 px and
  removes the grey `#dcdcdc` / `#2d2d2d` bar that is outside the palette.
- **Header** (fill `panel`, 1-px bottom hairline `grid_strong`; groups separated by 1 × 16-px dividers
  in `well_edge`):
  1. device glyph (16 px) + `dev.name` in the device hue; click → device menu (help page, sizes, about);
  2. track chip: 10 × 10 swatch in the track colour + name in `dev.control`, max 120 px, elided;
  3. **preset group, centred in the free width**: ‹ · a **280-px preset field** (`well`, radius 4,
     1-px `well_edge`; ☆ favourite, the name in `dev.preset`, a gold **●** + italic when modified, ▾ to
     open the browser) · › · save;
  4. A|B segmented (48 px) and → copy;
  5. undo / redo (the host history filtered to this device);
  6. instruments: Play | Edit | Matrix segmented;
  7. right: bypass ⏻ (lit in the device hue when active; bypassed = the "Bypassed" badge of
     `fabfilter/family.md` §7.4.10), ? help, ⋯ menu, ⤢ size menu.
- **Footer** (fill `panel`, 1-px top hairline): the **help line** (`help` role, flexible width, the
  hovered control's name · purpose · gestures · modulation) · **chips** "Label value" (`deck_title`
  fill, radius 3: Oversampling 2×, Latency 2.0 ms, Voices 5/8, CPU 3.1 %) · an output meter (60 × 6) and
  its readout · a **resize grip** (12 × 12, three diagonal lines in `text_dim`).
- **Instruments** keep the macro bar, modulation strip and keyboard of `instruments/family.md` §6.3,
  each as a **card** of its own (title bars "MACROS", "MODULATION"), so they obey the same rules; the
  keyboard is a card with no title bar.

### 7.6 The card (section frame)

```
   ┌──────────────────────────────────────────────────────┐  radius 6, fill deck
   │ ■ FILTER 1        ‹ Ladder 24 dB ›               ⋯   │  title bar 24, fill deck_title
   ├──────────────────────────────────────────────────────┤  1 px device_ground @ 50 %
   │▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒ display (optional) ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒│  bleeds left/right; min 96 px
   │                                                      │  10
   │   CUTOFF        RES          DRIVE        KEY TRK    │  param cells (§7.7)
   │    (◔)          (◔)          (◔)          (◔)        │
   │  [2.40 kHz]   [ 30 % ]     [+3.0 dB]    [ 50 % ]     │
   │                                                      │  12
   └──────────────────────────────────────────────────────┘
```

- **Title bar**, left to right: 8 px · **LED** 10 × 10, radius 2 (on: fill the card's function hue with
  a 1-px darker edge; off: 1.5-px `knob_track` outline, no fill; absent if the section cannot be
  switched off, replaced by a 3 × 12-px hue tab) · 6 px · **TITLE** in `sec.title` · a type selector
  (‹ name ›, `dev.control`, chevrons 10 px) either right after the title or centred · `⋯` menu (section
  presets, copy/paste, reset) at 8 px from the right.
- **Depth.** Light themes: an outer shadow `Shadow { offset: [0, 1], blur: 2, spread: 0, color:
  black @ 8 % }` and a 1-px `#ffffff` @ 70 % highlight along the top edge. Ink & Gold: no shadow (the
  ground is darker), a 1-px `#ffffff` @ 5 % highlight. This is the "controlled vocabulary of depth" the
  audit asks for (§5.1.4).
- **States.** Section off: body at 45 % opacity, LED off, title in `text_dim`. Selected (the card that
  owns keyboard focus or the modulation target being edited): 1.5-px border in the function hue
  (Zebra 3). Hovered during a modulation drag: border in the source's colour.
- **Displays** sit directly under the title bar and **bleed to the card's left and right edges**, with a
  1-px `well_edge` line above and below (Serum, Pigments). Selectors and mode switches that belong to
  the display go **inside** it, top-left (Vital); value tags top-right; axis labels on 85 % plates
  (the audit's `plot_label`). Minimum height 96 px; 40–55 % of the card's height in instruments; ≥ 70 %
  of the window in display-centric effects (EQ, multiband, analyser).
- **Group rule** inside a card: a `sec.title`-styled word centred between two 1-px `well_edge` lines
  that span exactly the grouped cells ("— SHAPE —", "— A —"). **Link line**: a 1-px `well_edge` line at
  knob-centre height joining two cells, with a link toggle in the middle when the link is switchable
  (Little AlterBoy, Pigments).

### 7.7 The param cell and the knob

**Cell anatomy (M):**

```
 y  0 ┌───────────── 64 ─────────────┐
      │           CUTOFF             │  param.label, centred, row 14 (baseline 11)
   14 │                              │  4
   18 │            ╭───╮             │
      │           ( ◔   )            │  knob, ring Ø 36
   54 │            ╰───╯             │  4
   58 │      ┌────────────────┐      │  value well 18 × 56, radius 3, fill deck_title
      │      │    2.40 kHz    │      │  param.value, centred
   76 └──────┴────────────────┴──────┘  row pitch 76 + 12 = 88
```

**Sizes** (the ring Ø matches `fabfilter/family.md` §7.2 and `instruments/family.md` V5):

| Size | Ring Ø | Track stroke | Gap track→body | Body Ø | Pointer | Cell width | Label | Value | Typical use |
|---|---|---|---|---|---|---|---|---|---|
| **S** | 28 | 2.5 | 2 | 19 | 1.5 px, 0.35→0.95 r | 48 | `param.legend` size, abbreviated | **replaces the label while hovered or dragged**; no well | inspector strip, dense rows, modulation depths |
| **M** | 36 | 3 | 2 | 26 | 2 px | 64 | `param.label` | well 18 × 56 | the default |
| **L** | 56 | 4 | 3 | 42 | 2.5 px | 80 | `param.label` | well 20 × 72 | macros, the 2–4 main parameters of a section |
| **XL** | 80 | 5 | 4 | 62 | 3 px | 112 | `sec.title` size | **inside the body**, `readout.hero` (Pro-R 2's Space knob) | one hero knob per device at most |

Body Ø = ring Ø − 2 × (track stroke + gap). Cell width is a floor: a cell grows to its label's galley
width + 8 when a label cannot be abbreviated, and the row re-flows by whole cells, never by overlap.

**This changes one family rule.** `fabfilter/family.md` §7.2 put the label *under* the knob and let the
value *replace* it while hovering. That rule came from Pro-Q, which hides values at rest. Cantus's own
principle is that state is visible at rest (§7.1 there), so the value needs a slot of its own; putting
the label **above** (as Soundtoys, Zebra 3, Diva, Massive X and Arturia's hardware panels do) gives
each cell a fixed reading order, name → control → value, and keeps the changing number away from the
fixed word. The S size keeps the old rule, because dense rows have no room for a well.

**Knob layers, back to front:**

1. **Modulation / automation rings** (existing ADR 0023 behaviour): 2-px arcs outside the ring at
   r + 3, r + 6…, in the source colour; per-voice dots per `instruments/family.md` V7.
2. **Track**: the 270° arc (gap at the bottom, from 135° to 405°), `knob_track`, the track stroke width.
   A 2-px notch at the **default** position when the default is not an end (bipolar knobs).
3. **Value arc**: on the track, same width, from the default to the value, in the card's function hue
   (dragging: `tint` 12 % brighter in dark themes, darker in light). **Round ends**: egui strokes have
   butt ends, so paint `circle_filled(end, stroke / 2, hue)` at both ends.
4. **Min/max dots** (optional, M and up): 2-px dots in `text_dim` just outside both ends (Serum).
5. **Body**: `circle_filled(c, body_r, knob_body)`. Bevel without a mesh: a 1-px arc from 200° to 340°
   in `#ffffff` @ 14 % (top light) and a 1-px arc from 20° to 160° in `#000000` @ 18 % (bottom shade).
   Ink & Gold adds the full 1-px `knob_rim` circle. Optional soft shadow under the body in light themes:
   `circle_filled(c + (0, 1), body_r + 1, black @ 10 %)` painted first.
6. **Pointer**: a line from 0.35 r to 0.95 r of the body at the value angle, `knob_pointer`, pointer
   width, plus a `circle_filled` cap at the outer end.
7. **Hover**: body lightened one step (`tint`), the well gets a 1-px `well_edge` outline, cursor
   `CursorIcon::ResizeVertical`; the help line fills after 500 ms.
8. **Focus**: the existing `paint_focus_ring`, drawn at ring r + 3.
9. **Disabled / section off**: everything at 45 % opacity; no value arc.

**Value well**: fill `deck_title`, radius 3, no border at rest (the knob identifies the control; the well
is part of it), `param.value` centred; it takes **drag, wheel and typing** like the knob (a click
focuses it as a text field; Enter commits; Esc restores). While the knob is dragged, the value text
turns the function hue.

**Legends instead of values** (Soundtoys): for knobs whose number means little to the user (Character,
Tone, Mix on simple devices), the cell may show two `param.legend` words at the ends of the arc
("DARK · BRIGHT", "DRY · WET") in place of the well; the number is still in the help line and on hover.

**A sketch of the knob paint in egui** (the load-bearing parts only):

```rust
// c: centre, r: ring radius, s: track stroke, g: gap, pw: pointer width, k: editor scale,
// hue: the card's function hue, pv / pd: value and default as positions 0…1
let ang = |p: f32| START + SWEEP * p;                    // START = 0.75π, SWEEP = 1.5π
let arc = |a0: f32, a1: f32, rad: f32| -> Vec<Pos2> {
    let n = (((a1 - a0).abs() / SWEEP) * 48.0).ceil().max(2.0) as usize;
    (0..=n).map(|i| { let a = a0 + (a1 - a0) * i as f32 / n as f32;
                      c + rad * Vec2::angled(a) }).collect()
};
let tr = r - s * 0.5;                                    // track centre-line radius
p.add(Shape::line(arc(ang(0.0), ang(1.0), tr), Stroke::new(s, pal.knob_track)));
if (pv - pd).abs() > 0.002 {
    let (a0, a1) = (ang(pd.min(pv)), ang(pd.max(pv)));
    p.add(Shape::line(arc(a0, a1, tr), Stroke::new(s, hue)));
    for a in [a0, a1] { p.circle_filled(c + tr * Vec2::angled(a), s * 0.5, hue); } // round ends
}
let br = r - s - g;                                      // body radius
p.circle_filled(c, br, pal.knob_body);
p.add(Shape::line(arc(200f32.to_radians(), 340f32.to_radians(), br - 0.5),
                  Stroke::new(1.0 * k, Color32::from_white_alpha(36))));   // top light
if let Some(rim) = pal.knob_rim { p.circle_stroke(c, br, Stroke::new(1.0 * k, rim)); }
let a = ang(pv);
p.line_segment([c + 0.35 * br * Vec2::angled(a), c + 0.95 * br * Vec2::angled(a)],
               Stroke::new(pw, pal.knob_pointer));
```

The label and value are laid out with `LayoutJob` so the cell can **measure before it paints**:
`let g = ui.painter().layout_job(job); if g.size().x > cell_w - 8.0 { /* abbreviation, else elide
with "…" and tooltip */ }` then `painter.galley(pos, g, pal.label)`. This replaces the current
`painter.text` calls in `widgets/knob.rs:331-353`, which paint without measuring and are the root of the
collisions in `cantus-audit.md` §4.1.

### 7.8 The other controls

| Control | Anatomy | States |
|---|---|---|
| **Segmented** (styles, modes, Play · Edit · Matrix, A · B, 6/12/18/24 dB, Draft/Good/Best) | a container 24 px high (S: 20), fill `deck_title`, radius 4, 1-px `well_edge` outline; segments of equal width (or text + 12), 1-px dividers `well_edge` @ 50 %; `dev.control` text | selected: light themes **fill `knob_body`, text `knob_pointer`** (the same ink as the knobs); Ink & Gold: fill `grid_bar` (`#3b4152`), text `text`, and a 2-px underline in the function hue; hover: fill one `tint` step |
| **LED button** (on/off: Freeze, Auto, Key EQ, Sync, Hold) | 20 px high; an LED 8 × 8 (radius 2) + caps text (11 px Plex Sans Medium, +0.3 px), on `deck_title`, radius 3 | on: LED in the function hue, text `text`; off: LED hollow `knob_track`, text `label` |
| **Menu field** (type pickers, key strip, algorithms) | 24 px, fill `deck_title`, radius 3, value in `dev.control` + ▾ 8 px at right; optional ‹ › steppers outside | hover: 1-px `well_edge` outline; open: outline in the function hue. **Never** underline-only |
| **Value field** (typed numbers not tied to a knob) | the knob's value well at its own width, with a `param.label` above | as the value well |
| **Fader / bar** | the existing `widgets::fader` and `widgets::bar`; groove in `knob_track` (≥ 3:1), cap in `knob_body`, label above and value well below exactly as a knob cell | as the knob |
| **Modulation chip** (source tiles, macro targets) | 20 px, fill `deck_title`, radius 3; a **3-px bar at the left in the family hue** (`hue_mod` for LFO/ENV/macros, `hue_source` for key/velocity/MPE…), the instance **tag** in `param.label` style ("LFO 2"), a count badge at the right | dragging: raised with the overlay shadow; target highlight as `instruments/family.md` §6.4 |
| **Icon button** | 24 × 24, icon 14 px in `label`; hover: fill `deck_title` | disabled: `text_dim` @ 60 % |
| **Property row** (settings changed once per sound: tuning, noise levels, velocity curve points, engine options) | 24-px rows in a card titled "ADVANCED": name at the left in `dev.control` / `label`, value right-aligned in a 20-px well (`param.value`), menus as menu fields (Arturia's Stage-73 V and B-3 V Advanced panels) | as the value well; this replaces long "knob walls" for parameters nobody performs with |

### 7.9 Colour in practice

- **Per card**: the LED, the value arcs, the display's main curve and the selected-segment underline use
  the card's function hue. Nothing else in the card is coloured.
- **Per device**: the header wordmark, the bypass lamp and the inspector chip use the device hue.
- **Gold** (`gold`, `gold_bright`, `focus`): keyboard focus, the modified-preset dot, and app-level
  selection. It no longer means "on".
- **Danger / clip**: `danger` only for clipping, gain reduction lines and destructive actions.
- **Modulation sources**: family hue on the chip bar, per-instance colour on rings and per-voice dots
  (the existing rotation), and always the tag text.

### 7.10 Displays inside cards

Follow `fabfilter/family.md` §7.4 rule 5 (theme-aware analysers). In addition:

- the display's background is `well`; in Paper this is white, one step lighter than the deck (Pigments
  light, Massive X light), with the 1-px `well_edge` lines above and below;
- the main curve is 2 px in the function hue, fills at ≤ 20 % alpha of the hue in light themes (≤ 35 %
  in dark), secondary curves 1 px in `text_dim`;
- grid lines in `grid` (a *decoration*, allowed below 3:1), axis ticks in `param.legend` on plates;
- the display may be the control (curve handles, wavetable position, sample start): its handles follow
  the band-handle rules of `fabfilter/family.md` §7.2;
- in the header row of a display, selectors sit **inside** it at the top-left; the value tag of what is
  under the pointer at the top-right.

### 7.11 Size and scale

- **Designed sizes at 100 %** (on the 4-px grid): effects 880 × 560, display-centric effects
  1040 × 640, instruments 1040 × 680, inspector strips at their panel width. The generic hosted plug-in
  frame fits the plug-in's own editor plus the 32 + 28 px shell.
- **Scale `k`** snaps to **75, 90, 100, 125, 150, 175, 200 %**. The ⤢ menu lists them with "Set as
  default for new windows" (Serum 2, Pigments, FabFilter). The **corner grip** resizes freely while
  dragging and **snaps to the nearest step on release** (Pigments). Snapping also bounds the number of
  font sizes egui has to rasterise (each `FontId` size is its own set of glyphs in the atlas).
- **Display-centric effects** may also grow in width beyond the aspect ratio; the extra width goes to
  the display, the controls stay at `k` (Pro-Q).
- **Implementation:** egui's zoom (`Context::set_zoom_factor`) is global, so per-window scaling is done
  in Cantus's own component layer: a `DeviceStyle { k, pal, hue }` passed to every cell, card and
  control, and every size and `FontId` computed as `base × k`. Strokes are snapped to whole physical
  pixels (`(w * ppp).round() / ppp`) so 1-px hairlines stay sharp at 125 % and 175 %.
- The chosen size is stored per device kind (and per window in the project), and the whole app's UI
  scale (Ctrl + / −, `pixels_per_point`) still multiplies on top (`fabfilter/family.md` §7.4.7).

### 7.12 Light and dark

- The three shipped palettes get the roles of §7.2. The colour editor exposes them, and **the theme test
  asserts every pairing** in the "Checks" column plus every function hue on `deck`, `deck_title` and
  `well` (≥ 4.5:1), `well_edge` and `knob_track` on `deck` (≥ 3:1), and the boundary of the knob
  (`knob_body` or `knob_rim`) on `deck` (≥ 3:1).
- Light themes: dark knob bodies, dark selected segments, light displays, darker saturated hues.
  Dark theme: dark bodies with a light rim, lighter hues, lighter selected segments.
- No theme may use colour alone: LEDs have a filled/hollow shape difference; segmented selection has a
  fill difference; modulation chips carry tags.

### 7.13 Tests and checks that keep the look

1. **Contrast**: extend the existing WCAG theme test to the new roles (above).
2. **No overlap, no clipping**: render every editor at 75 % and 200 %, in EN and ES, in all three
   themes, through `raster::shoot`, and assert for every param cell that its label and value galleys
   fit the cell (the layout records galley rects in a debug list).
3. **One font table**: no `FontId::new` outside `design.rs`; every text uses a role.
4. **No bare text toggles** in device views: a lint over `views/fx` and `views/inst` for
   `selectable_label` and `ui.label(` inside rows of controls, as the audit counts (145 call sites).
5. **Screenshot review**: keep `examples/shoot.rs` as the design review; add the new sizes and
   the ES locale to its matrix.

### 7.14 Order of work

1. Palette roles and type roles (§7.2, §7.3), with their tests.
2. The knob and the param cell (§7.7): measuring, abbreviations, the value well. Every editor improves
   at once, because every editor uses `widgets::Knob`.
3. The card (§7.6) and the segmented, LED and menu controls (§7.8); replace text toggles.
4. The shell (§7.5): header with the preset field, footer with the help line and grip; remove the egui
   title bar; scale `k` and snapping (§7.11).
5. Per-editor passes: EQ and compressor get a display-first layout (≥ 70 %); instruments get their
   cards (OSC, FILTER, AMP, MOD, FX, VOICE) with function hues; modulation chips grouped by family.

---

## 8. Sources

**Vendor pages and manuals (read for this report)**
- Native Instruments, Massive X product page and gallery:
  https://www.native-instruments.com/en/products/komplete/synths/massive-x/
- Native Instruments, Kontakt 8 product page, included sounds and Tools pages:
  https://www.native-instruments.com/en/products/komplete/samplers/kontakt-8/
- Arturia, Pigments product page: https://www.arturia.com/products/software-instruments/pigments/overview
- Arturia, *Pigments 7 User Manual* (§3.3.1.7 Resize Window, §3.3.1.8 Theme, §3.8.5 Resize handle,
  §3.8.6 Maximize View, §18.1 Sound Design Tips):
  https://dl.arturia.net/products/pigments/manual/pigments_Manual_7_0_0_EN.pdf
- Xfer Records, Serum 2 web manual, "Resizing the UI":
  https://xferrecords.com/web-manual/serum-2/resizing-the-ui
- u-he, Diva product page ("Resizable UI from 70% to 200%"): https://u-he.com/products/diva/
- u-he, Zebra 3 product page ("Adaptive interface: modules only appear when added to the patch"):
  https://u-he.com/products/plug-ins/zebra3/
- Soundtoys, Decapitator product page and product images: https://www.soundtoys.com/product/decapitator/
- Soundtoys, "Resizable UIs Are Here!": https://www.soundtoys.com/resizable-uis-are-here/
- Soundtoys Knowledge Base, "How Do I Resize the Plug-in GUI?":
  https://support.soundtoys.com/article/125-how-do-i-resize-the-plug-in-gui
- FabFilter, via [`../fabfilter/family.md`](../fabfilter/family.md) and its sources.

**Press and forums**
- MusicTech, "Massive X 1.1 comes with interface updates and new presets":
  https://musictech.com/news/massive-x-1-1/
- gearnews, "Massive X 1.1 update fixes the envelope visualisation and adds new skins":
  https://www.gearnews.com/massive-x-1-1-update-fixes-the-envelope-visualisation-and-adds-new-skins/
- rekkerd.org, "Massive X updated with new skins…":
  https://rekkerd.org/massive-x-updated-with-new-skins-dynamic-envelopes-and-switcher-lfo-60-new-presets-more/
- VI-CONTROL, "Kontakt 7.4.0 – re-sizeable interface" **[forum]**:
  https://vi-control.net/community/threads/kontakt-7-4-0-re-sizeable-interface.140989/
- NI Community, "UI Zoom on Kontakt!" **[forum]**:
  https://community.native-instruments.com/discussion/15281/ui-zoom-on-kontakt

**Image sources of the files saved for this report**
- `mx-01`: https://www.native-instruments.com/cdn/shop/files/a21771a55148-massive-x-image-01-new.png?v=1786053898&width=3840
- `mx-02`: https://www.native-instruments.com/cdn/shop/files/massive-x-product-page-03-gallery-player-page.png?v=1772642601&width=3840
- `mx-03`: https://www.native-instruments.com/cdn/shop/files/massive-x-product-page-03-gallery-a-skins.png?v=1772642608&width=3840
- `mx-04`: https://www.native-instruments.com/cdn/shop/files/massive-x-product-page-03-gallery-c-filters.png?v=1772642604&width=3840
- `k8-01`: https://www.native-instruments.com/cdn/shop/files/Kontakt-8-product-page-07-conflux-seamless.png?v=1772642274&width=3840
- `k8-02`: https://www.native-instruments.com/cdn/shop/files/Kontakt-8-included-sounds-23-hybrid-keys.png?v=1779815937&width=3840
- `k8-03`: https://www.native-instruments.com/cdn/shop/files/Kontakt-8-included-sounds-08-synths.png?v=1781959244&width=3840
- `k8-04`: https://www.native-instruments.com/cdn/shop/files/Kontakt-8-Tools-subpage-01-hero_01.png?v=1786624630&width=3840
- `pg-01`: https://medias.arturia.net/images/products/pigments/clear-color-coded-design.jpg
- `pg-02`: https://medias.arturia.net/images/products/pigments/animation-that-informs.jpg
- `st-01`: https://www.soundtoys.com/wp-content/uploads/Soundtoys_5.5_Decapitator_MassiveCrunch.png
- `st-02`: https://www.soundtoys.com/wp-content/uploads/Soundtoys_5.5_EchoBoy_ComplexPedalPattern-1.png
- `st-03`: https://www.soundtoys.com/wp-content/uploads/Soundtoys_5.5_Crystallizer_Master.png
- `st-04`: https://www.soundtoys.com/wp-content/uploads/Soundtoys_5.5_LittleAlterBoy_ModernDouble.png
- `st-05`: https://www.soundtoys.com/wp-content/uploads/5.5-Effectrack-Screencap-Oceanside-Groover.png
- `st-06`: https://www.soundtoys.com/wp-content/uploads/Soundtoys_5.5_Filterfreak_DistortedStaccato.png
- `st-07`: https://www.soundtoys.com/wp-content/uploads/Soundtoys_5.5_SuperPlate_LongClearSynthspace.png
- `st-08`: https://www.soundtoys.com/wp-content/uploads/Soundtoys_5.5_Sie-Q_Clarity.png
- `uhe-01`: https://u-he.com/products/plug-ins/zebra3/features/modulation-rack/img/section.webp

**Cantus**
- Renders: `cargo run -p cantus-ui --example shoot -- <dir> 1 eq comp limiter space wavetable analog fm
  sampler bass organ epiano pulsus` (light and dark for each).
- Code read: `crates/cantus-ui/src/design.rs`, `theme.rs`, `widgets/knob.rs`, `views/inst/frame.rs`,
  `views/fx/mod.rs`.
- Companion reports: [`cantus-audit.md`](cantus-audit.md), [`../fabfilter/family.md`](../fabfilter/family.md),
  [`../instruments/family.md`](../instruments/family.md).
