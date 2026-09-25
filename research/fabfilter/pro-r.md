# FabFilter Pro-R 2: research for the Cantus reverb

*Researched 2026-09-23. Goal: understand Pro-R 2 well enough to design an original reverb for
Cantus that matches or beats it in features, sound and feel. This is not a spec for a clone.*

Unless marked **(inference)**, facts come from FabFilter's own pages (the product page, the online
help and the PDF manual) or from the reviews and forum threads cited inline and listed at the end.
Everything below is paraphrased.

---

## 0. Screenshot index

All images come from fabfilter.com (the product page and the help pages) and are stored under
`research/fabfilter/screenshots/`. That folder is git-ignored (`.gitignore: /research/*/screenshots/`),
so the images stay local reference material and are never committed.

| File | What it shows | Source |
|---|---|---|
| `pro-r-01-main-interface.jpg` (2000×1456) | Whole plug-in in "Vintage Hall": top control row, decay EQ curve (blue), post EQ curve (yellow), analyzer, meters, bottom bar | product page, official screenshot |
| `pro-r-02-interface-overview-annotated.jpg` | Same view with every UI region labelled by FabFilter (decay-rate scale, EQ range button, meter, piano button, analyzer settings, resize) | help › Overview |
| `pro-r-03-main-controls-space-character-distance-width.jpg` | Top control strip: Predelay (sync), Style, Character, Thickness, Distance, Space + Decay Rate satellite knob, Brightness, Auto Gate, Width, Ducking, Freeze, Mix + Lock | help › Main controls |
| `pro-r-04-decay-rate-eq-display.jpg` | Decay Rate EQ with bell, notch and shelf bands; 12 %…200 % log scale; per-band ghost curves | help › Decay Rate EQ |
| `pro-r-05-decay-rate-eq-waterfall.jpg` | FabFilter's waterfall (level vs. frequency vs. time, 0–4000 ms) showing what a decay EQ curve does to the tail | help › Decay Rate EQ |
| `pro-r-06-post-eq.jpg` | Post EQ (yellow) on top of decay EQ (blue), ±18 dB range, low-cut and shelf/bell bands | help › Post EQ |
| `pro-r-07-analyzer-piano-and-meters.jpg` | Analyzer ("Reverb+Post"), piano keyboard strip with one dot per band, 5.1 output meters, Surround Settings in the bottom bar | help › Spectrum analyzer |
| `pro-r-08-band-value-display-speakers-menu.jpg` | Floating band value display (enable, shape, frequency, decay %, Q, delete) with the Speakers menu open | help › Surround |
| `pro-r-09-surround-effect-tilt-panel.jpg` | Surround panel: "Effect Tilt" front/rear sliders for 9 main controls, plus LFE, Center, Top and Cross Feed knobs | help › Surround |
| `pro-r-10-ir-import.jpg` | IR import in progress: the imported waveform drawn over the display while the plug-in converts it to settings | help › IR import |
| `pro-r-11-preset-browser.jpg` | Preset browser: folders by size, details panel with author, tags and description, favourite star, search | help › Loading presets |

---

## 1. Version, dates, what changed

| | |
|---|---|
| **Current product** | **Pro-R 2**. There is no Pro-R 3 as of Sep 2026. |
| **Current build** | **2.06**, 16 Apr 2026 (per the FabFilter download page) |
| **Pro-R 2 release** | **2 Nov 2023**; 2.01 followed the next day (3 Nov 2023) |
| **Pro-R 1 release** | 15 Nov 2016 |
| **Price** | EUR/USD 169, GBP 144; discounted upgrades for existing owners |
| **Formats** | VST2, VST3, CLAP (since Dec 2023), AU, AAX Native, AudioSuite on Win/macOS; AUv3 on iPad. Mono, stereo and immersive layouts up to 9.1.6 |

**New in 2.0** (release post and help "New in version 2"):
- Two new styles alongside the original, now called *Modern*: **Vintage** (early digital units from the '80s and
  '90s: bright, shimmering, best for big spaces) and **Plate** (fast build-up, metallic).
- **Immersive/Atmos** up to 9.1.6, with cross-feed between surround channels, per-band **Speakers** routing for
  both EQs, and a surround panel with front/rear **Effect Tilt**.
- **IR import** (WAV/AIFF, FLAC from Dec 2023). The file is analysed and turned into *algorithmic* settings;
  no convolution happens.
- **Thickness** (density plus saturation/dynamics), **Ducking**, **Auto Gate** (tempo-syncable hold) and
  **Freeze**.
- New **preset browser** (tags, favourites, search) and a new factory library.
- **Mid/side and L/R** placement per Post EQ band.
- **Piano scale** on the display, with note snapping.
- **Output level meters**.
- **Decay Rate** range widened from 50–200 % to **25–400 %**. The help's "other key features" list still says
  50–200 %: a stale line in the docs.
- Predelay moved from the bottom bar into the main control row.

**Point releases since:**
- Dec 2023: CLAP; AAX upmix (e.g. stereo to 7.1.2); FLAC IR import; IR drag-and-drop, which users had called
  out as missing at launch; meter scale fix.
- Feb 2024: VST2 to VST3 migration.
- Jun 2024: VST3 5.1.2 layout.
- Mar 2026: sizing and scaling fixes; drag-and-drop of preset files; Copy/Paste in the preset right-click menu.
- Apr 2026: pasting settings from older versions; preset browser hover fix.

Since launch the product has had maintenance only; its feature set has been stable since 2023.

---

## 2. Complete feature reference

FabFilter documents behaviour, not numeric ranges, for most knobs. "n/d" means not documented. Values
marked *(obs.)* are read off the official screenshots.

### 2.1 Main controls (top row, left to right in the UI)

| Control | Range / values | Default | Behaviour |
|---|---|---|---|
| **Predelay** | 0–500 ms | n/d | Gap between the dry signal and the onset of the reverb. |
| ↳ Predelay **Sync** | Off, 1/4, 1/8, 1/16, 1/32 | Off | When sync is on, the knob becomes **Predelay Offset** (50–200 % of the synced time), which covers dotted (150 %) and triplet (67 %) values. |
| **Style** (button above Character/Thickness) | Modern / Vintage / Plate | n/d | Chooses the algorithm. IR import always selects Modern. |
| **Character** | 0–100 % | n/d | 0 % is as clean and predictable as possible. Towards 50 % it adds some modulation, more pronounced early reflections and subtle late echoes. Above 50 % modulation keeps rising, reaching a chorus-like effect at 100 %. Sound On Sound (v1): one knob moves modulation, diffusion and early-reflection scatter together. |
| **Thickness** | n/d (%) | n/d | Density plus saturation. Up gives a denser tail with subtle non-linearity and dynamics; down gives an airy, sparse one. FabFilter warns the result depends heavily on the source material. |
| **Distance** | 0–100 % | n/d | Listener position in the room. At 0 % the source is close, with prominent, bright early reflections. Higher values give a longer build-up and a more diffuse tail. SOS: it sets both the early-to-late balance and the character of the reflections. |
| **Space** (large centre knob) | about 0.2 s (tiny ambience) to about 10 s (cathedral) | n/d | Stepless. Morphs through "over a dozen" room models (reviews count 12) and sets a matching decay time automatically, without clicks. A yellow label shows the resulting decay in seconds. About six room icons around the knob jump straight to model positions *(obs.)*. The models are not named anywhere. |
| **Decay Rate** (small "%" satellite knob on Space) | 25–400 % of the decay Space implies | 100 % | Changes the decay length without changing the room model. The Space label shows the final time, plus the original time and the rate when the rate is not 100 %. Effective decay is therefore about 0.05–40 s **(inference)**. |
| **Brightness** | n/d | n/d | Moves the high/low balance **and** how fast highs decay. It stands for high-frequency absorption; FabFilter says a slightly lower setting usually sounds more natural. |
| **Stereo Width** | 0–120 % | n/d | 0 %: mono. 50 %: maximum internal cross-feed; a sound in one input reverberates equally in both outputs. 100 %: channels fully independent (multi-mono). 101–120 %: side signal boosted. Width is internal linking plus cross-feed, not just an output M/S mix. (Range from MusicRadar's v1 review.) |
| **Ducking** | one knob sets the ducking range | n/d | "Intelligent" ducking keyed from the plug-in's own input. **No threshold, attack, release or external sidechain.** |
| **Auto Gate** | on/off, hold time (drag on the button), tempo sync (same divisions as predelay) | off | Keyed from the input. Threshold, attack and release are automatic, tuned for single drum hits. |
| **Freeze** (snowflake above Mix) | toggle, or momentary while the mouse button is held | off | Endless decay that recirculates what is already in the tank. Input-side controls (Distance, Thickness) stop having any effect; Character, Brightness, Ducking and Auto Gate still work. |
| **Mix** | 0–100 % | n/d | Dry/wet. FabFilter tells users to set 100 % by hand when the plug-in sits on a send. |
| ↳ **Lock Mix** (padlock) | on/off | remembered globally | Loading a preset leaves Mix alone. Automation and the saved session still apply. New instances inherit the lock state. |
| **Input level/pan**, **Output level/pan** | n/d (dB, L/R) | 0 dB | Hover the output button in the bottom bar to show them. |
| **Global bypass** | on/off | off | Soft (click-free) bypass; the analyzer freezes while bypassed. |

Plus: **Smart Parameter Interpolation** (every parameter change is smoothed) and sample-accurate automation
of all parameters.

### 2.2 Decay Rate EQ (blue curve; FabFilter calls it an industry first)

- **What it is.** A curve of *decay-time multiplier against frequency*, not of gain. Classic reverbs offer a
  low/high crossover with two multipliers. Pro-R gives a free parametric curve instead (see waterfall
  `pro-r-05`).
- **Bands:** up to **6**. **Shapes:** Bell, Low Shelf, High Shelf, **Notch**.
- **Per band:** frequency, decay rate (%), Q, enable, shape, and Speakers routing in multichannel instances.
  The value display shows e.g. "177.19 Hz / 156.3 % / Q 0.578" *(obs.)*.
- **Scale:** 12 %…200 % on a log axis, with 100 % at about the upper third *(obs.)*. The numeric limits of a band
  are not documented. A v1-era forum bug report (confirmed by FabFilter support) found that *raising* high-frequency decay with a
  steep high shelf failed while lowering it worked, which suggests a ceiling on loop gain at high frequencies
  **(inference)**.
- **Creating bands:** drag the curve itself. Dragging near the left or right edge makes a shelf. Double-click
  or Ctrl-click low in the display makes a notch.

### 2.3 Post EQ (yellow curve)

- Up to **6 bands**. Shapes: Bell, Low Shelf, High Shelf, **Low Cut, High Cut** (cuts up to **96 dB/oct**).
- **Per band:** frequency, gain, Q, enable, shape, slope (cuts only), **stereo placement**
  (Stereo / Left / Right / Mid / Side) and Speakers in multichannel.
- **Display range:** ±30, ±18 or ±9 dB (a button at the top of the right-hand scale).
- **Automatic gain compensation:** reverb level is corrected as you EQ, so the mix and send level do not need
  touching.
- **Creating bands:** drag the curve; double-clicking the background at the far left or right makes a cut.

### 2.4 Shared band editing (both curves; the Pro-Q interaction model)

- Click a dot or its coloured area to select; rubber-band select (spans **both** curves); Ctrl-click to add to
  the selection; Shift-click for a range.
- Dragging moves the whole selection. Several selected decay bands scale *relative* to one another.
- Wheel, or Ctrl+vertical drag, changes Q; Shift fine-tunes; Alt locks to one axis.
- Alt-click bypasses a band; Ctrl+Alt-click cycles its shape; Alt+Shift-click cycles the cut slope; double-click
  types exact values (Tab steps through the fields); right-click opens a band menu.
- A floating **parameter value display** follows the band under the mouse (enable, shape, Speakers or
  placement, the values, an X to delete). If that band is part of a selection, edits apply to the whole
  selection.
- The left scale (blue, decay) or right scale (yellow, post) lights up to show which curve you are editing.

### 2.5 Analyzer and meters

- Modes: **Reverb+Post** (default), **Pre+Post**, **Off**.
- A thin white line shows the real-time output spectrum. Behind it, **soft yellow "ghost" curves fall and fade
  at the reverb time of each frequency**: a stylised picture of the tail per frequency, not a measurement
  **(inference)**.
- **Output meter** on each channel (all speakers in surround), with a peak read-out; it can be hidden from the
  Help menu.
- **Piano strip** (88 keys) under the display, with one dot per band. Click a dot to snap the band to the nearest
  note; drag it to move in semitones. With the strip on, frequencies also show as note plus cents.

### 2.6 Surround / immersive (multichannel instances only)

- Layouts up to 9.1.6 Atmos. The UI adapts to the layout.
- Speakers menu per band: All, All excl. LFE (the default), All Tops, L/C/R, LFE, Center, L/R, Lss/Rss, Lsr/Rsr,
  Lts/Rts…
- **Surround Settings** panel:
  - **Effect Tilt:** a front↔rear bias for each of 9 main controls (Predelay, Character, Thickness, Distance,
    Decay Rate, Brightness, Width, Ducking, Mix).
  - Wet-level knobs for **LFE** (fully dry by default), **Center** and **Top**. Top trades a surround feel for an
    immersive one.
  - **Cross Feed** between speaker pairs (default 100 %).
- AAX can upmix, e.g. stereo to 7.1.2.

### 2.7 IR import

- Load WAV, AIFF or FLAC from the preset browser or by drag-and-drop. The display animates the analysis while
  the IR is converted into **main-control settings, a Decay Rate EQ curve and a Post EQ curve**. Style is always
  Modern. Save the result as an ordinary preset.
- Impulse responses only: **sine sweeps are rejected** with a warning.
- FabFilter's staff describe the result as the space's "character", not a replica. Stereo cross-feed is not
  reproduced exactly, and FabFilter has not published how the analysis works.

---

## 3. The display in detail

Layout (see `pro-r-01`, `pro-r-02`): the **top 25 %** is a dark control "deck" whose lower edge curves down
around the Space knob, which bulges into the display. The **bottom 75 %** is one large interactive frequency
display (20 Hz–20 kHz, log). It has:

1. **Left scale (blue): decay rate %** (12/25/50/100/200) for the blue curve.
2. **Right scale (yellow): Post EQ dB** (±30/18/9), with a **second dB scale for the analyzer** beside it
   (0…−90) and the **output meter** at the far right edge.
3. **Blue Decay Rate EQ curve**, with translucent blue fill between the curve and 100 %, plus a thin line for
   each band's own contribution.
4. **Yellow Post EQ curve**, with amber fill between the curve and 0 dB, and a thin line for each band.
5. **Band dots**, coloured per band (cyan, blue, green, orange…). The selected dot gets a white ring.
6. **Analyzer**: a white live line plus warm, blurred decaying layers. It is visually striking and doubles as
   brand identity.
7. **Piano strip** (optional) along the bottom.

**What you can drag:** either curve (which creates a band), the band dots, and the piano dots. You can also
drag a rectangle to select, and use the wheel for Q. Every knob works by vertical drag (speed-sensitive), mouse
wheel, or double-click to type. Typed values accept shortcuts: "1k", "A4", "C#3+13", "2x" (= +6 dB), "50%".

**What the display does *not* show** (these are our openings):
- no **time axis** anywhere: no envelope, early-reflection pattern, predelay marker or build-up;
- no **absolute RT60 in seconds per frequency**, only relative %;
- no picture of **room model, distance or width**: Space is just a number and a few icons;
- no **measured** decay: the ghost curves are drawn from the model's settings;
- no **gain-reduction view** for Ducking or Auto Gate, although those are dynamic processes;
- no **per-curve bypass**. MusicRadar and a Gearspace user both asked for one, to A/B the decay EQ quickly.

---

## 4. Workflow

- **Presets:**
  - Library is organised by size: Ambience / Small / Medium / Large / Very Large / Tempo Synced / Basic.
  - Each preset carries an author, tags and a description, and can be starred as a favourite; type-to-search
    finds presets by name, folder or tag.
  - Browsing: previous/next arrows; `[` and `]` while the browser is open; arrow keys and Enter.
  - A preset loads as soon as it is clicked, and the browser stays open for auditioning.
  - MIDI Program Change / Bank Select can load presets.
  - "Save As Default" overwrites the default preset; the preset name dims once the preset has been edited.
  - Files are `.ffp` and cross-platform. Pro-R 1 presets load and sound about 99 % the same; **Pro-R 1
    automation does not carry over**.
- **Undo/redo:** every UI change becomes an undo step. **Automation or MIDI changes do not.**
- **A/B + Copy:** two states; Copy duplicates the active state into the other and then greys out until they
  differ again.
- **Copy/Paste of settings** from the right-click menu on the preset name (all plug-ins, since Mar 2026).
- **MIDI Learn:** a mode that dims the UI and labels each parameter with its CC. Click a parameter, move a
  controller, done. The menu offers Enable MIDI, Clear (per mapping or all), Revert and Save.
- **Resizing:** Medium (default) / Large / Extra Large, **Full Screen** (Esc leaves), scaling 100–300 %
  remembered per monitor type, free edge-drag resizing in VST3.
- **Help:** interactive help hints (a tooltip bubble per control), a full online and PDF manual, and the Help menu.
- **Keyboard/modifiers:** Ctrl-click resets; Shift fine-tunes; Alt drags linked knobs together. Pro Tools gets
  its own modifier conventions.
- **iPad:** touch equivalents (a second finger for fine control, double-tap to reset, long-press to type).

---

## 5. The DSP concept: what is public, and what we can infer

FabFilter publishes **no** algorithm details. Its stated goals, from the About page and the Pro-R 1 press
release: a *natural* sound, *enough density*, *no colouration or phase problems*, so the reverb sits in a mix;
**each musical control drives several internal parameters at once**; and room models that morph without clicks.
Staff have said on the forum that early reflections, modulation and other advanced options were limited *on
purpose*, "because it is too easy to make it sound unnatural".

Our best reconstruction **(inference, for designing our own engine; not a claim about FabFilter's code):**

| Pro-R control | Likely mechanism | How we would build it |
|---|---|---|
| **Space** | A table of hand-tuned "room model" parameter vectors (delay-line lengths, diffusion, ER pattern, base T60(f), density), interpolated continuously, with a base decay tied to room size | Room models as data (a `RoomModel` struct). Morph by interpolating the vectors. Change delay lengths click-free by slow modulated interpolation (limited Doppler) or by a short equal-power crossfade between two read taps. |
| **Decay Rate + Decay Rate EQ** | Frequency-dependent **absorption filters** in the feedback loop of an FDN-type late reverb. For delay line *i* of length *mᵢ* samples, the attenuation per pass is **Aᵢ(f) [dB] = −60 · mᵢ / (fs · T60(f))**. Here T60(f) = T60_space(f) × DecayRate × r(f), and r(f) is the blue curve converted from % to a factor. | Sample the target T60(f) on a ⅓-octave grid and fit each line's attenuation filter to its own target Aᵢ(f). Use an accurate cascaded graphic EQ, as in Prawda, Välimäki & Schlecht (DAFx 2019) and the two-stage attenuation filter (IEEE SPL 2024). **Clamp every line's gain below 0 dB** (that is why "infinite" decay needs Freeze). Show the *achieved* T60(f) against the target. |
| **Brightness** | High-frequency absorption inside the loop (HF decay) plus an output tilt | The same attenuation filters (HF T60 scaling) plus a gentle output shelf. |
| **Character** | LFO/random modulation of delay taps (depth and rate grow with the knob), less input diffusion (so discrete echoes survive), a stronger ER scatter pattern | Time-varying FDN: modulated delays and/or a slowly rotating feedback matrix (Schlecht & Habets). Expose rate and depth in an advanced drawer. |
| **Distance** | ER/late balance, ER spectrum (brightness), and build-up time (input diffusion length and ER density) | A direct-to-reverberant model. Distance moves ER gain and colour, the late onset (build-up) and diffusion. |
| **Thickness** | Echo density (diffuser count or feedback) plus soft saturation and gentle compression on the tank input (FabFilter says it acts on the input side: it has no effect in Freeze) | A diffusion stage with variable density, then a soft-clip/compander before the tank. |
| **Width** | An input cross-feed matrix (independent L/R tanks at 100 %, fully shared at 50 %, mono at 0 %) plus an M/S side boost above 100 % | The same topology: it is easy to explain and to show visually. |
| **Vintage style** | Emulates '80s digital units: lower internal bandwidth or sample rate, strongly modulated allpass loops (a Lexicon-style "spin/wander"), a brighter shimmering tail, a grainy build-up | Run the tank at a reduced internal rate with anti-alias filters, a figure-eight allpass loop and random modulation. |
| **Plate style** | A dense, fast build-up with metallic modes: classic plate tank topology (Dattorro, JAES 1997) | Dattorro-like tank with high early density and modest modulation. |
| **Post EQ gain compensation** | Integrate the post-EQ magnitude over a pink-ish reference spectrum and apply the inverse gain | The same; also show the compensation amount. |
| **Freeze** | Loop gain set to 1 and damping disabled or kept; input muted; the input-side stages (ER, Thickness) bypassed | The same, with a ramped transition and optional slow filtering of the frozen tail. |
| **IR import** | Find the onset (gives predelay). Run octave-band **Schroeder backward integration** to get EDC → T60(f); fit Space and Decay Rate to the broadband T60 and fit the residual as up to 6 decay bands. Fit the long-term spectrum (or early energy) as up to 6 post-EQ bands. Estimate width from inter-channel coherence. Sweeps are rejected because they are not impulses. | We can do better (see §8): show the fit error, keep the ER pattern, and support true-stereo IRs. |
| **Analyzer ghosts** | The output spectrum plus decaying copies whose fade time per frequency comes from the model's T60(f) | Replace with a *measured* decay view (see §8). |

**Key references for our engine:**
- Jot & Chaigne (1991): FDN with absorption filters.
- Schlecht & Habets: time-varying FDN matrices; accurate T60 control (DAFx 2017).
- Prawda, Välimäki & Schlecht: "Improved Reverberation Time Control for FDNs" (DAFx 2019).
- Välimäki, Prawda & Schlecht: "Two-Stage Attenuation Filter for Artificial Reverberation" (IEEE SPL 2024).
- Dattorro: "Effect Design Part 1" (JAES 1997).
- Schroeder: backward integration (1965).
- Dal Santo et al.'s curated list of delay-network reverb papers: https://github.com/gdalsanto/delay-network-reverbs

---

## 6. Reception

### Praise (consistent across sources)
- **It sits in a mix:** smooth, uncoloured, easy to fit.
  - MusicTech 9/10 (Nov 2023): "high-quality algorithms that sit well in the mix".
  - WaveInformer (Dec 2023): calls it perhaps the most versatile algorithmic reverb available.
  - SOS (v1): "smooth, rich, sophisticated and classy".
- **Speed and clarity of the UI:** musical macro controls with no reverb jargon.
  - Tape Op (Larry Crane, Jan/Feb 2024) found it far easier than its density suggested.
  - Computer Music gave v1 9/10.
  - The KVR launch thread called the redesign "cleaner, less cluttered, more readable".
- **Decay Rate EQ** is the signature feature, praised as more musical than crossovers.
- **Distance** and **Width**, singled out by Tape Op for shaping depth and placement.
- **Ducking and Auto Gate** "work great". Gated '80s drums became easy.
- **IR import** as a preset generator ("close enough to *feel* right").
- **Atmos support** is best in class for an all-rounder.

### Complaints and gaps
1. **Too little deep control.** No separate early-reflection section and no modulation rate/depth (FabFilter
   forum threads; MusicTech: "extended modulation options" missing). FabFilter defends limiting them.
2. **Ducking is a single knob.** No threshold, attack, release, frequency focus or external sidechain.
   Gearspace users say the ducking "didn't sound very good".
3. **No bypass per curve** for the decay and post EQ, so A/B-ing the curves is awkward (MusicRadar v1;
   Gearspace v2).
4. **IR import is hit and miss.** Some results are "quite far off", especially low-mids and width (MusicTech);
   one KVR user called it "completely useless" compared with real convolution. No true-stereo (4-channel) IRs.
5. **No spring algorithm**, and no non-linear or reverse options beyond Auto Gate (MusicTech, WaveInformer).
6. **Hard to read.** A FabFilter forum thread measured Pro-R 2's UI as much darker and lower in contrast than
   Pro-Q 3 or Pro-C 2 and asked for a brightness/gamma option. FabFilter did not reply.
7. **Metallic or bright highs** (a v1 KVR thread): users often have to cut highs. Defenders say it is fine at mix
   levels. The same "synthetic highs" remark shows up in Valhalla comparisons.
8. **Price.** The upgrade and full price seem steep next to Valhalla's USD 50. Some said v2 "looks new but most
   buttons do the same as v1".
9. **Other:** no saving of decay-EQ curves as partial presets (SOS v1); no comb/special effects; no
   separate-dry/wet level mode; no tempo sync for decay; no reverse/envelope; no snapshots beyond A/B (all
   requested on the FabFilter forum).

---

## 7. Competitors: what they have that Pro-R lacks

| Product | Model / price | What Pro-R 2 lacks that it has |
|---|---|---|
| **Valhalla VintageVerb** (v4.0.5) | Algorithmic, USD 50 | **22 modes**, including exotic ones (Nonlin, Chaotic Hall/Chamber/Neutral, Random/Chorus Space, Chamber1979, Hall1984), and **3 era "Color" modes** (1970s/1980s/Now) that change bandwidth and grain. **Explicit controls** for attack, size, early/late diffusion, mod rate/depth and bass multiplier/crossover. Tiny CPU and a tiny price. Widely called the most-used reverb. (No decay curve, analyzer, ducking or surround.) |
| **ValhallaRoom** (v2.0.5) | Algorithmic, USD 50 | **Separate Early and Late sections**, each with its own modulation; late decay **0.1–100 s**; 12 modes. |
| **LiquidSonics Seventh Heaven (Pro)** (v1.5.8) | "Fusion-IR" modulated convolution of the **Bricasti M7**; USD 69 / 299 | The realism of a revered hardware unit, **236 curated programs**, early/late level, low-frequency level, a separate delay, a 5-band EQ, up to 9.1.6 in Pro. |
| **LiquidSonics Cinematic Rooms (Pro)** (v1.3.9) | Algorithmic surround, USD 199 / 399 | A dedicated **reflection engine**; a **full dynamics section** (ducking, compression, gating) with **sidechain triggers**; "surround editing planes"; propagation modes (surround, true stereo, multi-mono); decay up to infinity. |
| **Relab LX480 Complete** (V4) | Lexicon 480L recreation; about USD 179–349 | **Dual-engine routing** (two 480L algorithms at once, in 4 engine configurations); faithful 480L programs with their classic deep parameters (e.g. spin/wander, shape/spread); 400+ presets including most original banks. |

The pattern: **Pro-R wins on approachability, the graphical decay curve, and polish. It loses on depth,
character variety, dynamics, and realism.** No one product combines both.

---

## 8. How we beat it

The plan is to keep FabFilter's best idea (a few musical macros over a large interactive display) and add
**depth that stays hidden until asked for**, **honest visualisation** and **DAW-native integration** that a
third-party plug-in cannot have. Priorities: **P0** = parity needed at launch, **P1** = where we win, **P2** =
later.

### P0: parity (the new engine replaces today's Freeverb in `crates/cantus-dsp/src/reverb.rs`)
1. **A proper late engine:**
   - FDN with 8 or 16 lines, a Householder or Hadamard matrix, and per-line attenuation filters fitted to a
     target T60(f) on a ⅓-octave grid;
   - loop gain always below 1;
   - deterministic, sample-rate independent (scale delay lengths with fs, unlike today's fixed comb tunings),
     allocation-free and null-testable.
2. **Space macro:** continuous morph across about 12 named room models, with a matching base decay and
   click-free transitions. **Decay** multiplier 25–400 % *and* an absolute read-out in seconds.
3. **Decay curve editor:** at least 6 bands (bell, shelves, notch).
4. **Post EQ:** at least 6 bands (bell, shelves, cuts up to 96 dB/oct), placement per band (Stereo/L/R/M/S),
   auto gain compensation. Reuse `cantus-dsp/src/eq.rs` biquads and add cuts and notches.
5. **Macros:** Predelay with tempo sync and offset, Character, Distance, Brightness, Thickness, Width
   0–120 %, Mix with Lock.
6. **Freeze** (toggle and momentary), **Ducking** and **Gate**, with better versions in P1.
7. **Styles at launch:** Natural (FDN), Vintage (reduced-rate modulated loop) and Plate (Dattorro-style tank).
   Keep today's Freeverb as a **"Classic" legacy style**, so old projects render bit-identically. Record this in
   an ADR with a schema bump and a fixture, per the Cantus workflow.
8. **Workflow:**
   - Cantus's own undo/redo, with undo steps for automation edits too (Pro-R records none);
   - A/B;
   - presets with tags and search;
   - MIDI learn through Cantus's mapping;
   - resizable, scale-aware egui panel;
   - every string in `locales/en.txt` and `locales/es.txt`.

### P1: where we clearly win
1. **A time axis: the "Tail view".** Add a second display mode (or a split view) that plots energy (dB)
   against time, with:
   - draggable **predelay**, **build-up/attack**, **early-reflection** window (with visible ER taps) and **decay**
     handles;
   - a live **measured** envelope of the wet signal drawn over it.
   Pro-R has no time view at all; this makes Distance, Character and Predelay *visible*.
2. **An honest decay display:**
   - the Y axis switches between **% and seconds (RT60)**;
   - draw the *achieved* T60(f) next to the target, so the user sees where the engine clamps (the bug FabFilter
     users ran into);
   - replace the decorative ghosts with a **measured spectro-decay (waterfall) of the real output** after a
     transient.
3. **Progressive disclosure: "Detail" drawers under the macros.**
   - **Early:** pattern and model, size, ER level, ER/late balance.
   - **Modulation:** rate, depth, random/sine, spin.
   - **Diffusion:** input and late.
   - **Dynamics:** see the next item.

   The macros stay the default surface, so we keep FabFilter's approachability while answering the most
   common complaint. Each macro visibly moves its internal parameters, so the macros teach reverb design
   instead of hiding it.
4. **Real dynamics:**
   - Ducking with threshold, attack, release and range, a frequency focus (a band-limited detector), and a
     **sidechain from any Cantus track or bus** (native routing, no host tricks);
   - Gate with threshold, hold (synced), release and shape;
   - a **gain-reduction trace** on the tail view.
   Keep a one-knob "Auto" mode for speed.
5. **Bypass and solo per curve and per band:**
   - one-click bypass for the decay curve and the post curve (requested twice);
   - solo a band to hear the frequency range it affects in the tail (a Pro-Q idiom that Pro-R lacks);
   - **partial presets**, so the decay curve, post EQ or macros can be loaded or saved on their own
     ("load only the decay curve").
6. **Better IR import ("Match"):**
   - multiband Schroeder T60(f) fit **plus ER extraction** (the first ~80 ms become our ER pattern),
     true-stereo (4-channel) and sweep deconvolution (accept sweeps: Pro-R rejects them);
   - **show the fit**: an overlay of IR against result, per-band T60 error, and a "match quality" score;
   - also "match to a region of a Cantus clip": capture a room from a recording in the project.
7. **Legibility first:**
   - honour Cantus's light default theme and its user-editable palette;
   - WCAG-level contrast for labels and scales, a colour-blind-safe pair for the two curves (not only blue vs.
     yellow: add line style and fill pattern), and adjustable UI scale.
   This is a documented Pro-R 2 complaint, and we can win it cheaply.
8. **More character without clutter:** styles for **Spring**, **Nonlinear/Gated**, **Reverse/Swell** (an envelope
   on the wet signal) and **Shimmer** (a pitch-shifted feedback loop). Add an era **Color** switch (Clean / '80s /
   '70s) that changes internal bandwidth and grain. This answers the missing-spring and missing-effects gaps and
   takes VintageVerb's best idea.
9. **Tempo-aware decay:** decay locks to note values (1 bar, 2 bars…) and the gate hold syncs; the Space read-out
   can show beats.
10. **DAW-native advantages no plug-in can match:**
    - on a **return bus**, default to Mix 100 % and Lock automatically (FabFilter asks the user to do this);
    - show the tail's RT60 and width on the mixer strip;
    - **snapshots (4–8) with morphing**, not just A/B, recorded as ordinary automation;
    - preset auditioning that follows the transport;
    - tails that render correctly on export, because Cantus knows the tail length.
11. **Width you can see:** a small goniometer or correlation meter on the output, and a **mono-compatibility
    warning** when width is above 100 %.
12. **Separate Dry / Wet levels** as an optional alternative to Mix (requested on FabFilter's forum).

### P2: later
1. **Immersive:** an N-channel engine (design the FDN outputs and the input cross-feed for N from day one),
   per-speaker band routing, front/rear **tilt** per macro, and LFE/Center/Top levels.
2. **Dual-engine routing** (serial or parallel, as in LX480), e.g. a Plate into a Hall.
3. **A 2-D "room map" pad:** X = Space (model and size), Y = Distance, dot size = Width. A direct, visual
   alternative to the three knobs.
4. **Quality modes** (Eco/Normal/High: line count and oversampling of the modulated taps) and **SIMD** kernels.
   Publish CPU figures; reviewers never mention Pro-R's CPU, but Valhalla's low CPU is a selling point.
5. **A "Freeze pad" mode:** a frozen tail with a slow filter sweep and crossfade for sound design.

### Design guardrails (what not to copy or break)
- Keep **one big hero control** (Space) and **no more than 10 knobs** on the default surface. Tape Op's first
  reaction to Pro-R's density was hesitation.
- Every macro must **sound natural across its whole range**: FabFilter's reason for limiting options is valid.
  Advanced drawers may make things strange; macros must not.
- **Tame the highs by default** (a darker default Brightness, gentle HF absorption), because "metallic highs" is
  the one long-running sound complaint about Pro-R.
- Follow the Cantus workflow:
  - null tests for bypass, dry-only and legacy "Classic" equivalence;
  - an ADR for the engine, one for the parameter schema and one for the UI;
  - a schema bump with a fixture;
  - undo fuzz over every new parameter;
  - both locales.

---

## 9. Sources

**FabFilter (official)**
- Product page: https://www.fabfilter.com/products/pro-r-2-reverb-plug-in
- Online help (index): https://www.fabfilter.com/help/pro-r
  - Main controls: https://www.fabfilter.com/help/pro-r/using/maincontrols
  - Decay Rate EQ: https://www.fabfilter.com/help/pro-r/using/decayrateEQ
  - Post EQ: https://www.fabfilter.com/help/pro-r/using/postEQ
  - Surround: https://www.fabfilter.com/help/pro-r/using/surround
  - IR import: https://www.fabfilter.com/help/pro-r/using/irimport
  - Spectrum analyzer: https://www.fabfilter.com/help/pro-r/using/spectrumanalyzer
  - Knobs: https://www.fabfilter.com/help/pro-r/using/knobs
  - Full screen / resize: https://www.fabfilter.com/help/pro-r/using/fullscreenandresize
  - I/O options: https://www.fabfilter.com/help/pro-r/using/outputoptions
  - MIDI learn: https://www.fabfilter.com/help/pro-r/using/midilearn
  - Undo / A-B: https://www.fabfilter.com/help/pro-r/using/undoredo
  - Presets: https://www.fabfilter.com/help/pro-r/presets/loadingpresets
  - Upgrading from v1: https://www.fabfilter.com/help/pro-r/support/upgrading
- PDF manuals: https://www.fabfilter.com/downloads/pdf/help/ffpror2-manual.pdf · (v1) https://www.fabfilter.com/downloads/pdf/help/ffpror-manual.pdf
- Release news (2 Nov 2023): https://www.fabfilter.com/news/1698915600/fabfilter-releases-pro-r-2-reverb-plug-in
- 2.01: https://www.fabfilter.com/news/1699002000/fabfilter-pro-r-201-update-released
- Dec 2023 update: https://www.fabfilter.com/news/1702983600/all-plug-ins-updated-with-clap-support-bug-fixes-and-more
- Jun 2024 update: https://www.fabfilter.com/news/1718618400/all-plug-ins-updated-with-bug-fixes
- Mar 2026 update: https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes
- Apr 2026 update: https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins
- Download page (2.06, 16 Apr 2026): https://www.fabfilter.com/download
- Pro-R 1 press release (15 Nov 2016): https://www.fabfilter.com/press/1479196800/fabfilter-releases-fabfilter-pro-r-reverb-plug-in

**Reviews**
- MusicTech (9/10, Nov 2023): https://musictech.com/reviews/plug-ins/fabfilter-pro-r-2-review/
- Tape Op #159: https://tapeop.com/reviews/gear/159/pro-r-2-reverb-plug-in
- WaveInformer: https://waveinformer.com/2023/12/01/software-review-fabfilter-pro-r-2/
- Sound On Sound (v1): https://www.soundonsound.com/reviews/fabfilter-pro-r
- MusicRadar (v1): https://www.musicradar.com/reviews/tech/fabfilter-pro-r-646592
- Gearspace review (v2, could not be fetched; seen only through search snippets): https://gearspace.com/board/reviews/1416749-fabfilter-pro-r-2-a.html

**Forums**
- KVR launch thread: https://www.kvraudio.com/forum/viewtopic.php?t=603159&start=30 · &start=90 · &start=105
- KVR "Is Pro-R really so metallic?": https://www.kvraudio.com/forum/viewtopic.php?t=542803
- FabFilter forum, early reflections (staff on limiting options): https://prod.fabfilter.com/forum/topic/6499/pror-early-reflections
- FabFilter forum, feature ideas (ducking/gating, staff reply): https://prod.fabfilter.com/forum/topic/7317/feature-ideas-for-pro-r2
- FabFilter forum, ideas for Pro-R 2: https://prod.fabfilter.com/forum/topic/7187/ideas-pro-r2
- FabFilter forum, true-stereo IR import: https://prod.fabfilter.com/forum/topic/7477/r2-true-stereo-ir-import
- FabFilter forum, decay-rate high-shelf bug: https://prod.fabfilter.com/forum/topic/6363/pro-r-bug-decay-rate-high-shelf-not-working-as-expected
- FabFilter forum, R2 GUI legibility: https://prod.fabfilter.com/forum/topic/7637

**Competitors**
- Valhalla VintageVerb: https://valhalladsp.com/shop/reverb/valhalla-vintage-verb/
- ValhallaRoom: https://valhalladsp.com/shop/reverb/valhalla-room/
- LiquidSonics Seventh Heaven: https://www.liquidsonics.com/software/seventh-heaven/
- LiquidSonics Cinematic Rooms: https://www.liquidsonics.com/software/cinematic-rooms/
- Relab LX480: https://relabdevelopment.com/lx480-dual-engine-reverb/

**DSP literature**
- Prawda, Välimäki, Schlecht, "Improved Reverberation Time Control for FDNs" (DAFx 2019): https://research.aalto.fi/en/publications/improved-reverberation-time-control-for-feedback-delay-networks/
- Delay-network reverb paper and code list: https://github.com/gdalsanto/delay-network-reverbs

*Method note:* Gearspace, Reddit and the main FabFilter forum host were behind bot protection or a corporate
login proxy. The Gearspace and Reddit sentiment above therefore comes from search-result excerpts, and the
FabFilter forum threads were read through its `prod.` mirror.
