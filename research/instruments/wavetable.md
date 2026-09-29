# Wavetable and hybrid flagship synths: competitive research for Firmus II

*Researched 2026-09-24 for Cantus. Primary sources are the manufacturers' own documents: the **Serum 2 User Guide**
(Xfer, v2.0.18, manual 1.0.3, 354 pages, served publicly as a PDF) and the *What's New in Serum 2* PDF; the
**Pigments 7 manual** (Arturia, product version 7.0.0, 15 Dec 2025); Kilohearts' online docs; u-he's Hive pages and
release notes; NI's Massive X page; the **Surge XT manual** and its repository documents; Vital's site and a community
user guide. For Vital and Surge (GPLv3) the source was **read to describe behaviour and file formats only; no code
is reproduced**, and nothing here should be copied into Cantus's MIT/Apache crates. Reviews and forums were used for
opinion and are marked as such. The web-search budget ran out part-way through; later facts come from direct fetches
of known pages, and anything I could not re-verify says so.*

*Screenshots are the manufacturers' images, saved in [`screenshots/`](screenshots/) (git-ignored) for internal
reference only. Colour and contrast figures are estimates read from those PNG/JPEG files (±5 % luminance).*

---

## 0. The short version

1. **The whole category has settled on one table format: 2048-sample single cycles, up to 256 frames.** Serum,
   Pigments, Phase Plant and Vital all use it, and Serum's WAV-with-`clm `-chunk is the lingua franca. Surge goes
   further (power-of-two frames up to 4096 samples, up to 512 frames, its own `.wt`, and it reads the `clm `, `uhWT`,
   `srge`, `smpl` and `cue ` chunks). **If Firmus II reads WAV + `clm ` and `.wt` on day one, it can play almost every
   table in circulation.** Massive X *still* cannot import tables after six years, and reviewers keep saying so.
2. **Serum 2 (March 2025, 2.1.5 in July 2026) is the benchmark again.** It has three oscillators, each with five
   engines (wavetable, multisample/SFZ, sample, granular with up to 256 grains, spectral). Each oscillator has
   **two warp slots**, chosen from about 60 warp modes in six families. It adds two filters (with Moog-, 303- and
   EMS-style circuit models), 4 envelopes, up to 10 LFOs (including Lorenz/Rössler chaos and an XY "Path" mode),
   8 macros, a mixer with two FX buses, a clip sequencer and an arpeggiator. It oversamples at 1×/2×/4× and has
   sample-accurate automation. The cost: reviewers call it **"more CPU-hungry than ever"**, and much of it is hidden
   in right-click menus.
3. **The competition is about engines, warps and modulation UX, not about sound quality.** All the leaders are
   clean. What differs is:
   - how a table is **warped**: Serum's phase and distortion warps, Vital's **spectral** warps on the harmonics,
     Pigments' phase transform and wavefolding;
   - how **unison** is shaped: detune curves, stacks and per-voice spread of frame and warp;
   - how modulation is **made and shown**: drag from a tile onto a knob, rings, a matrix, live source graphics.
4. **Antialiasing is solved three ways, and we should combine two of them.**
   - **Precomputed mip-mapped tables** (Surge: octave levels made by half-band decimation, read through a
     windowed-sinc step convolution).
   - **Per-block inverse FFT with harmonic truncation** (Vital: each frame is stored as a spectrum, and each voice
     rebuilds a band-limited cycle every block, which is also what makes its spectral warps possible).
   - **Oversampling** on top of either (Serum 1/2/4×, Vital 1–8×). Warps, sync and FM create harmonics that no
     table can pre-limit, so the oversampling is needed as well.

   The combination for us: octave mips plus 2× internal oversampling is alias-free *and* flat to 20 kHz at 48 kHz.
5. **Nobody is host-native, and it shows.** Every product keeps its own undo, its own preset silo and its own
   sequencer (Serum 2 now ships a "mini DAW" clip sequencer). Their modulation stops at the plug-in boundary.
   Massive X exposed only 16 macros to host automation until **March 2026**. Phase Plant is criticised for giving no
   visual feedback on routing. **Cantus can fix all of these structurally:**
   - one undo history, covering wavetable edits too;
   - modulation that reaches the mixer, and host sources that reach the synth;
   - per-note lanes drawn in the piano roll;
   - resampling any clip or track into a table, at exactly 2048 samples per cycle;
   - an AI bridge that edits patches with the same commands the user's hands produce.
6. **Legibility varies a lot; Serum 2 is the one to learn from.**
   - Serum 2 puts dark knobs on light blue-grey panels (≈5:1) and green readouts on near-black (≈9:1).
   - Vital's flat dark style has knob bodies at ≈1.3:1 against the panel, FabFilter's failure again, rescued only
     by saturated arcs.
   - Pigments hides state behind hover (rings appear only when you hover a source).
   - Surge XT is the only one that documents **screen-reader support**.
7. **Proposal: a new instrument variant (working title Firmus II, product name *Tabula*) rather than rewriting
   Firmus.** Existing projects must keep rendering bit-identically, the same rule ADR 0017 applied to the channel EQ.
   - **P0 (parity):** 3 wavetable oscillators + sub + noise, 16-voice unison, dual warps, two filters,
     4 envelopes, 8 LFOs, 8 macros, a 64-slot matrix with drag-to-modulate, a wavetable editor, WAV/`clm `/`.wt`
     import and export, all deterministic and block-size independent.
   - **P1 (only a host can do these):** the host-native features in finding 5.
   - Sections 8 and 9 have the parameter list, the voice architecture, the file formats and the tests.

---

## 1. Products and versions

| Product | Latest version and date | Price | Formats | Source |
|---|---|---|---|---|
| **Xfer Serum 2** | **2.1.5, 6 Jul 2026** (release notes as quoted in search excerpts; the 2.0.x history to 2.0.24 of 13 Nov 2025 is in a changelog gist compiled from Xfer's update API). Launched as 2.0.16 on **17 Mar 2025**. A **Linux beta** was announced in summer 2026: VST3, x86-64 with AVX2, glibc ≥ 2.38. | **$249**, with lifetime free updates; free for Serum 1 owners; $189 at launch | **VST3, AU, AAX** (64-bit). No VST2, **no CLAP**. Win 10+, macOS 10.13+ | [product page](https://xferrecords.com/products/serum-2), [User Guide PDF](https://www.xferrecords.com/manual/serum-2/docs), [changelog gist](https://gist.github.com/0xdevalias/a537a59d1389d5aed3bc63b544c70c8d), [Linux beta](https://synthanatomy.com/2026/08/xfer-records-serum-2-super-popular-wavetable-synth-gets-massive-free-update.html) |
| **Arturia Pigments 7** | **7.0.0, 15/16 Dec 2025.** The manual was updated for 7.0.1 in July 2026. **7 is current.** | **$199**; $99 at launch; free for Pigments owners | VST, VST3, AU, AAX, standalone, NKS. CLAP not listed | [press release](https://www.arturia.com/company/press/press-releases/arturia-announces-pigments-7), [What's new FAQ](https://support.arturia.com/hc/en-us/articles/23729034214940-Pigments-New-in-Pigments-7), [manual 7.0.0](https://dl.arturia.net/products/pigments/manual/pigments_Manual_7_0_0_EN.pdf) |
| **Kilohearts Phase Plant** | **2.4.6, 10 Mar 2026.** **v3 was teased in Dec 2025 and previewed at Superbooth 2026, but is not released.** Confirmed for v3: audio input, a vocoder snap-in, metering and a new browser. | **$199** | VST, VST3, AU, AAX. No CLAP in the changelog | [changelog](https://kilohearts.com/changelog), [docs](https://kilohearts.com/docs/phase_plant), [v3 tease (KVR)](https://www.kvraudio.com/forum/viewtopic.php?t=626488) |
| **NI Massive X** | **1.7, 3 Mar 2026.** It finally exposes *all* parameters to host automation. 1.6 (Jul 2025) added a Play view, a Morpher (XY) and an Animator. | **$199** | VST, VST3, AU, AAX. No CLAP. NI is in preliminary insolvency proceedings (per Synth Anatomy) | [product page](https://www.native-instruments.com/en/products/komplete/synths/massive-x/), [1.7 news](https://synthanatomy.com/2026/03/native-instruments-massive-x-out-now.html) |
| **u-he Hive 2** | **2.1.2, 27 Aug 2024.** It added CLAP, MPE improvements and **export of wavetables in Serum format**. | **€149** | **CLAP**, VST3, AU, AAX. Windows, macOS, **Linux** | [product page](https://u-he.com/products/hive/), [release notes](https://u-he.com/products/hive/releasenotes.html) |
| **Vital** (open) | 1.5.x. **Not verified this session:** the site has no public changelog, and the GitHub repo accepts no PRs. | Free (Basic) / Plus / **Pro $80** / $5 a month. Paid tiers add presets, tables and text-to-wavetable quota | VST, VST3, AU, LV2, standalone (from the download page, not re-fetched). **GPLv3** source at `mtytel/vital` | [site](https://vital.audio/), [repo](https://github.com/mtytel/vital) |
| **Surge XT** (open) | **1.3.4, 11 Aug 2024.** The 1.4 milestone was 92 % complete on 14 Sep 2026; Lua features are the long pole. | Free, **GPLv3** | VST3, AU, **CLAP** (with note expressions and polyphonic modulation, CLAP only), LV2, standalone. Windows, macOS, Linux | [releases](https://github.com/surge-synthesizer/releases-xt/releases), [changelog](https://surge-synthesizer.github.io/changelog/), [manual](https://surge-synthesizer.github.io/manual-xt/) |

**Observation:** of the seven products, only Hive and Surge ship CLAP. The two market leaders, Serum 2 and Pigments,
still don't in September 2026. That matters for how Cantus exports its own instruments (see `standards.md`), not for
competing with them inside Cantus.

---

## 2. Screenshots saved

All in [`screenshots/`](screenshots/), prefix `wt-`. Every file was checked: PNG signature `89504E47` or JPEG
`FFD8FF`, and every one was opened and viewed. The `fam-*` files were saved by the family agent and are referenced
here read-only.

| File | What it shows |
|---|---|
| `wt-01-serum2-main.png` | **Serum 2 main page** (2380×1480, product page). Tab bar: OSC · MIX · FX · MATRIX · GLOBAL, with a green dot on the active tab. Module row: SUB · OSC A (wavetable, 3D view) · OSC B (wavetable) · OSC C (granular) · NOISE · FILTER 1 · FILTER 2. Each module has a power LED, a type drop-down, a routing badge (F1/F2) and a menu. Under that: macros 1–8 with counts, ENV 1–4, LFO 1–10 tabs with **destination-count badges**, VELO and NOTE; an LFO in **Path** (XY) mode; voicing (POLY 8, "4 / 272" voices); CLIP, ARP, the keyboard and the key, scale and swing settings. |
| `wt-02-serum2-wavetable-osc.png` | One wavetable oscillator. The 3D table view is teal with green frames, and the **current frame is yellow**. Around it: OCT/SEM/FIN/CRS readouts, phase 180°, RAND 100, then WT POS, UNISON (field plus gear for its settings), DETUNE, BLEND, **WARP 1/WARP 2 slots** with ‹ › steppers, PAN, LEVEL. |
| `wt-03-serum2-spectral-osc.png` | The spectral oscillator: a green spectrogram with a blue region selection, a loop mode (FWD/REV LOOP), SCAN, and CUT/FILTER/MIX with a small drawn filter curve. |
| `wt-04-serum2-granular-osc.png` | The granular oscillator. **Live grains are drawn as white vertical lines** over the waveform, with a window-shape icon. Controls: SCAN, DENS, LENGTH and their RAND knobs. The small hook icons on knobs mark modulated controls. |
| `wt-05-serum2-lorenz-lfo.png` | An LFO of type *Chaos: Lorenz* drawn as a point-cloud attractor. FREE/RETRIG/ENVELOPE modes, BPM/HZ at 1000 Hz, RISE, DELAY, MONO. |
| `wt-06-pigments-engines.jpg` | Pigments' Modal engine (a crop): cyan partial bars in 3D, light-grey knobs on near-black with blue value arcs. |
| `wt-07-pigments-modulation.jpg` | A Pigments Function editor (crop): a green multi-point curve with round handles, a one-shot mode and a retrigger source. |
| `wt-08-pigments-banner.png` | Pigments product banner: the Synth page over the Play page. **The centre modulation strip holds 24 source tiles with live mini-graphics, coloured by group** (pink MIDI, orange envelopes, amber LFOs, green functions, purple random, magenta combinate, blue macros). The tabbed source editor sits below, with 4 macros on the right. |
| `wt-09-hive2-fullui.jpg` | Hive 2 full UI (default light-grey skin). Symmetric left and right halves (OSC/SUB, FILTER, AMP/MOD/LFO) around a **hexagonal centre panel** showing a wavetable (frames, position, interpolator = crossfade). A CLEAN/NORMAL/DIRTY engine switch, shape sequencers, function generators, MATRIX A/B, XY1–4. |
| `wt-10-phaseplant-hero.jpg` | Phase Plant marketing render (perspective): the generator stack (analog, sample, envelope, wavetable), lanes of snapins, macros. Use `fam-03` for analysis instead. |
| `wt-11-surge-signal-flow.png` | **Official Surge signal-flow diagram** (from `doc/`): two scenes; per voice, 3 osc + noise, then the mixer (FM and ring mod between oscillators), then the filter block F1 → waveshaper → F2 with feedback, then VCA and HPF; per-scene FX1/FX2, sends, sum. |
| `wt-12-surgext-main.png` | Surge XT main window (Classic skin, from the manual). A grey-blue panel with **sliders instead of knobs**, orange for the selected state, the blue routing bar of modulation sources, an orange LFO editor with a waveform display and DAHDSR. |
| `wt-13-surgext-modulation-routing.png` | Surge's routing bar (macros, MIDI sources, voice and scene LFOs, EGs) above the LFO 1 editor. |
| `wt-14-surgext-mod-depth-sliders.png` | **How Surge draws modulation on a slider**: the modulated slider gets a second (blue) handle for depth and a light-green bar for the range; a filter-type chip above it. |
| *(sibling)* `fam-02-vital-main-frame.png` | Vital main page. Flat dark grey, **purple oscillators, orange filters, teal envelopes and LFOs**. Each oscillator shows its unison (voices, detune %), spectral morph ("INHARMONIC", "HARM STRETCH", "TIME SKEW") and distortion ("FM OSC 2") pickers. |
| *(sibling)* `fam-04-vital-drag-to-modulate-frames.png` | Vital drag-to-modulate in three frames: dragging LFO 1 turns every valid target teal; after the drop, a **half-filled depth disc** and a teal bar on the slider show the amount. |
| *(sibling)* `fam-03-phaseplant-overview.png` | Phase Plant annotated overview: macro bar with coloured source dots, a generator stack with Group, three effect lanes, a horizontally scrolling modulator strip, and **a latency readout (16.33 ms) bottom-right**. |
| *(sibling)* `fam-07/08/09-pigments-*.jpg` | Pigments Play view in dark and **light** variants, and the Synth combi view. |

Not kept: Vital's and Massive X's page art (a logo and an abstract render, with no interface in them). NI's gallery
images are lazy-loaded and I did not script around that.

---

## 3. Anatomy of each product

### 3.1 Serum 2 ([User Guide](https://www.xferrecords.com/manual/serum-2/docs), [What's New PDF](https://static.xferrecords.com/Serum%202%20What's%20New.pdf))

- **Signal flow.**
  - Sources: SUB + OSC A/B/C + NOISE.
  - Each source is routed to Filter 1 and/or Filter 2, or direct, from a **mixer page**. The mixer has per-channel
    level and pan and sends to **two internal FX buses** plus Main.
  - The filters run in series or parallel. After them come the FX rack (main plus bus 1 and bus 2) and the output.
  - The filters are **per voice**; the FX are per instance.
- **Oscillators.** Three, each switchable between five engines.
  - **Wavetable:**
    - up to 256 frames of 2048 samples; the manual points out that 2048 × 256 × 32 bit is exactly 2 MB;
    - a "Smooth Interpolation" option makes WT position continuous without baked morph frames;
    - phase and phase randomisation, and **phase memory** (all voices / contiguous / per voice);
    - unison up to 16, and **two warp slots**.
  - **Multisample:** loads SFZ (not SF2), with its own envelope, velocity tracking, a "timbre" shift and unison.
  - **Sample:** loop modes, crossfade, slicing, start/end and loop points that can be modulated.
  - **Granular:** up to **256 grains**, window amount, density, length, scan, pitch, random per parameter, and an
    X|Y control.
  - **Spectral:** analysis and resynthesis with transient detection, scan rate, cut, a drawn spectral filter and a
    mix; a "Pitch Shift" spectral warp arrived in 2.0.19.
  - Plus a **sub** (6 shapes) and a **noise** oscillator (sample-based, one-shot or looping, keytrack).
- **Warp modes** (manual pp. 49–56, grouped by menu category):
  - *Sync*, with a soft/hard variable;
  - *Alt Warp*: Bend ±, Asym ±, PWM, Flip, Mirror, Remap 1–4 (a drawn transfer curve; Remap 2 mirrored, 3
    sinusoidal, 4 applied four times), Quantize, Odd/Even;
  - *Filter*: LPF, HPF;
  - *Distortion*: Tube, Soft Clip, Hard Clip, Diode 1/2, Linear Fold, Sine Fold, Zero-Square, Asym, Rectify, Sine
    Shaper, Stomp Box, Tape Sat., Soft Sat.;
  - *FM* from B, C, noise, sub or either filter, in **thru-zero, exponential and linear** variants;
  - *PD* from another oscillator, a filter or **self**;
  - *AM* and *RM* from the same sources;
  - "Swap Warps".

  The 2D view **draws the effect of the warp live** for the Sync, Alt Warp and Distortion families.
- **Unison** (pp. 45–48):
  - **Modes:** Linear (evenly spaced in frequency), Super, Exp, Inv, Random.
  - **Stack:** Off, 12 (1–3×), 12+7 (1–3×), Center −12, Center −24.
  - **Width**, **Range**, and per-voice **spread of WT POS, WARP 1 and WARP 2** around the knob position.
  - **Detune** and **Blend**. Blend is a level offset between the centre voice(s) and the others; at 75 % all
    voices are equal.
  - An automatic level law keeps loudness roughly constant as voices are added.
  - The unison field **changes colour** as the count rises, as a CPU warning.
  - The manual calls 7 "the classic magic number" and recommends 3–7.
- **Filters.** Two modules, each with its own oscillator routing (S, A, B, C, N) and serial or parallel routing.
  The type list includes:
  - MG Low 6/12/18/24 (ladder);
  - SVF low, high, band, peak and notch;
  - dual SVFs (LH/LB/…/NN);
  - morphing SVFs (LBH, LPH, LNH, BPN);
  - comb, flanger and phaser in L/H/HL variants;
  - Low/Band/High EQ;
  - ring mod ×1/×2;
  - sample-and-hold;
  - allpass combs, "reverb";
  - French LP, German LP, Add Bass;
  - Formant I–III;
  - Bandreject;
  - distortion combs;
  - Scream;
  - Wsp (Wasp circuit model), DJ Mixer;
  - **new in 2:** Diffusor, **MG Ladder, Acid Ladder (diode ladder), EMS Ladder, MG Dirty**, **PZ SVF (drawable)**,
    Comb 2, Exp MM, Exp BPF.

  Every type has a context-dependent **VAR** knob, labelled FAT, MORPH, SMOOTH, PAIN (the lighter-under-the-board
  joke), DAMP and so on. **Drive has a "Clean mode"** (−24 dB in, +24 dB out). **Pan offsets the cutoff between L
  and R.** The display shows the frequency response, response plus FFT, or phase plus FFT, and cutoff and resonance
  can be dragged on the graph.
- **Modulation.**
  - **Sources:**
    - 4 envelopes (ENV 1 is amp), each with hold, drawn curves, BPM mode and **legato invert**;
    - **up to 10 LFOs** (LFO 7–10 appear only after LFO 6 is used), of type Normal, Path (XY), Chaos Lorenz,
      Chaos Rössler or S&H, in Free, Retrig or Envelope mode (with a loopback point), with rates to **1000 Hz**
      (×10 mode), rise, delay, smooth, phase, direction, swing and an LFO editor window;
    - 8 macros (each also a destination), velocity and note;
    - in the matrix only: active voices, alternate note-on, note-on randoms, oscillators and filters as sources,
      release velocity, voice index, voice mod 1/2, **MPE/Note Expression X/Y/Z**, wheels, aftertouch and poly
      aftertouch.
  - **Assigning:** **drag a source tab onto a knob**. A blue halo shows the depth, and a small handle at the top-left
    of the knob adjusts it; Alt-drag on the knob does the same. A grey halo marks modulation from an unselected
    source. The source tab shows **how many destinations** it has.
  - **Matrix:** source; **editable curve with rise/fall slew**; amount; uni- or bipolar; destination; an output
    graph; an **aux source (multiplier) with its own curve and invert**; output scale; bypass; remove. Rows can be
    reordered by drag. An expanded view.
- **Voicing and global.**
  - Mono, legato (paraphonic when mono is off), poly count, and portamento with curve, always and scaled.
  - A **voice control sequencer**: up to 8 steps of per-voice values, plus randomisation of pan, detune, cutoff and
    envelopes per voice.
  - **Quality:** Draft 1×, High 2×, Ultra 4×, lockable; an S1-compatibility mode; "disable smoothing" for sample
    accuracy.
  - Tuning files and **MTS-ESP**.
- **FX.** 13 effect types plus **3 splitters** (L/H, L/M/H, M/S). The effects are Bode, Chorus, Compressor,
  Convolve, Distortion, EQ, Filter, Flanger, Hyper/Dimension, Phaser, Reverb (adding Vintage, Nitrous and Basin),
  Utility and Delay (an HQ mode is now the default). Modules can be dragged to reorder, there are rack presets,
  multiple instances are allowed, and the displays are directly editable.
- **Performance.** A **clip sequencer**: 12 slots per bank, a piano roll, record (overdub or extend), automation
  lanes, **MIDI out**. An **arpeggiator**: 12 slots, a pattern editor, retrigger, velocity, MIDI out. A keyboard
  page with transpose, **key and scale applied to input and to clip and arp output**, swing, and an **oscillator
  mapping editor** (note and velocity ranges per oscillator).
- **Presets.** A tree browser with packs, categories and tags, ratings, previews that **play custom clips**,
  macros adjustable while previewing, and hybridise. Imported audio is embedded in the preset, at 8 KB to 4 MB per
  table.

### 3.2 Arturia Pigments 7 ([manual](https://dl.arturia.net/products/pigments/manual/pigments_Manual_7_0_0_EN.pdf))

- **Signal flow.** Two main engine slots plus a **Utility engine** (a 5-shape VA oscillator, 2 noise engines,
  external audio input). They feed two filters with **continuously variable series/parallel routing**, then a VCA,
  then FX buses A and B plus an aux send bus (3 slots each).
- **Engines.**
  - **Virtual Analog** (3 oscillators, unison);
  - **Wavetable**:
    - 256 positions × 2048 samples;
    - a Morph on/off toggle;
    - 2D and 3D views;
    - FM (linear, exponential or ring) from a **modulation oscillator**;
    - **phase modulation** with a retrigger source (Key, Random, Self, Mod Osc);
    - **Phase Transform** (7 phase-distortion "types": Pulse Width, Skew, Round, Tri/Pulse, Octave Plus,
      Pseudo PW, Fractalize);
    - **Wavefolding** (3 folder shapes);
    - unison up to 8 in Unison, Chord or Super modes;
  - **Sample**: 6 samples per engine, with a granular mode;
  - **Harmonic** (additive);
  - **Modal** (physical modelling, since v6).
- **Custom tables.** Folders of WAV files. The rule is plain: **the first 2048 samples are position 1, the next
  2048 position 2, …, up to 524,288 samples**. The manual gives no chunk support; Serum `clm ` tables load simply
  because they are already multiples of 2048.
- **Filters.** **19 types, 68 modes**: Classic, Cluster, Phaser, Formant, Surgeon, **Ripple, Reverb, Rage (new in
  7)**, LoFi, Comb, Mini, MS-20, Matrix 12, Jup-8, SEM, LowPass Gate. 7.0 adds audio-rate FM on the Classic filter.
- **Modulation.**
  - **24 sources** in the centre strip: MIDI group (keyboard, wheels, expression), 3 envelopes, 3 LFOs,
    **3 Functions** (drawn multi-segment), 3 Random (Turing, S&H, Binary), **2 Combinates** (combine two sources),
    4 macros.
  - **Colour by group.** Hovering a source draws a **collar (mod ring)** on its destinations and puts dots on
    engine tabs that hide destinations. Hovering a destination shows small pie knobs for its sources.
  - Every route can have a **sidechain** (a second source that scales it).
  - Three ways to assign: drag and drop (since v4), the **Mod Source view** (sliders −1…+1 for every source on one
    target), and the **Mod Target view** (every target of one source).
  - Quick Edit v3 (7.0) prints "{source} ± amount" with the min and max range.
- **Voices.** Polyphony from mono up to **32**, with voice allocation modes (Rotate, …).
- **FX.** 20 algorithms, including shimmer reverb, a multiband compressor and a vocoder, plus Corroder (new in 7).
- **Sequencer.** A generative, polymetric sequencer and arpeggiator with scale charts.
- **Presets and Play view.** A redesigned **audio-reactive Play page** (7.0) with circular macros and a visualiser
  of the sound's structure; a light and a dark variant. In-app tutorials and sound-design tips.

### 3.3 Kilohearts Phase Plant ([docs](https://kilohearts.com/docs/phase_plant), [modulation](https://kilohearts.com/docs/modulation))

- **Signal flow.** Semi-modular:
  - a **generator stack of up to 32 modules**: analog, wavetable, sample, granular and noise generators, plus
    distortion, filter, non-linear filter, mix and aux routing, and groups;
  - each stack **sends to one of 3 effect lanes** of *snapins* (Kilohearts' effect modules; the Essentials are
    free, the rest are paid), and the lanes chain into the master;
  - lanes can be poly (per voice).
- **Wavetable.** 256 frames × 2048 samples. The editor converts samples, keyframe-animates effects across frames,
  and shows spectrum and waveform views with morph, pen, brush, harmonic and filter tools.
- **Unison.**
  - Modes: Hard, Smooth, Synthetic, Frequency Stack, Pitch Stack, **Shepard**, and chord modes.
  - Parameters: voices, detune, spread, blend, **bias**.
  - Available per generator and globally.
- **Audio-rate modulation** between generators: classic (phase) FM, linear FM, ring mod, exponential FM.
- **Modulators.** **Up to 32**, of about 18 types: envelope, LFO, **LFO Table** (a wavetable as an LFO), curve,
  random, audio follower, **pitch tracker**, note, velocity, pressure, pitch wheel, note gate, MIDI CC, MPE timbre,
  remap, scale, lower and upper limit, sample and hold.
  - Assigning: click "+", pick a target, and an orange depth knob appears next to it.
  - **Each modulator draws the newest voice in blue and older voices in grey.**
  - **Control rate is about every 64 samples, which caps control modulators near 100 Hz.** Generator-to-generator
    modulation runs per sample: orange marks control rate, green marks audio rate.
- **Criticism (review, [ModWheel](https://modwheel.net/reviews/kilohearts-phase-plant-review)):** the blank canvas
  intimidates, effects beyond Essentials cost extra, and **"no meaningful visual feedback on signal flow"**.

### 3.4 Native Instruments Massive X ([product page](https://www.native-instruments.com/en/products/komplete/synths/massive-x/))

- **Oscillators.** Two dual wavetable oscillators (170+ tables) with **10 oscillator modes** (the Gorilla family,
  Bend, Hardsync, Formant, …), 2 phase-modulation oscillators and 2 noise sources.
- **Filters.** New designs (Asimov, Creak, Groian, comb, …) in series or parallel.
- **Routing and FX.** Up to 3 insert FX, whose slots can also hold a sine/saw/pulse oscillator, a ring modulator,
  S&H or a PM oscillator. **"Any output to any input"** on a routing page (the only product here with a patch-cable
  view). Stereo FX follow.
- **Modulation.** 9 slots for envelopes and LFOs (switcher LFOs, a random LFO, an exciter envelope), **3
  Performers** (8-bar drawn modulation), Remote Octave, **Trackers** (note-dependent curves), a voice randomiser,
  and 16 macros.
- **2025–26 changes.** 1.6 added the Morpher XY and the Animator. **1.7 finally exposes all parameters to host
  automation** (before that, only the 16 macros were exposed).
- **Still missing:** **wavetable import**. Synth Anatomy (Mar 2026): it "doesn't work even after 6 years".

### 3.5 u-he Hive 2 ([product page](https://u-he.com/products/hive/), [release notes](https://u-he.com/products/hive/releasenotes.html))

- **Oscillators.** Two oscillator + sub pairs.
  - The oscillators run either classic waveforms or **wavetables**, with **"Multi-Table" 2D tables of up to 16
    parts** and **4 frame interpolators: switch, crossfade, spectral, zero-phase**.
  - Tables are **`.uhm` scripts**, u-he's own text scripting language, and since 2.1.2 they can be exported in
    Serum format.
- **Engine character.** **Clean / Normal / Dirty**, a global CPU-versus-character choice.
- **Filters.** Two multimode filters (LP, HP, BP, BR, peak, comb, reverb, dissonant, sideband).
- **Modulation.**
  - 2 LFOs, 2 ADSR pairs, **3 function generators** (envelope, gate, LFO or slew);
  - a **12×2 matrix** with modifiers (curve, rectify, quantise, S&H, slew);
  - a 16-step sequencer plus an 8-step **shape sequencer**;
  - XY pads.
- **Voices and FX.** Up to 16 voices, unison up to 16 per oscillator, 7 FX.
- **Display.** A scope. It needs at least 1000×600 on screen.

### 3.6 Vital ([site](https://vital.audio/), community [user guide](https://davidmvogel.com/docs/Vital/UserGuide/Oscillators-and-Sampler); counts confirmed from the source's constants)

- **Voice resources:**
  - **3 wavetable oscillators + 1 sampler**, **2 filters**;
  - **6 envelopes**, **8 LFOs**, **4 random sources** (Perlin, S&H, sine-interpolated, **Lorenz**), **4 macros**;
  - up to **32 voices**;
  - **64 modulation connections** at most.
- **Wavetables.** 2048-sample frames on **257 positions** (0–256).
- **Per oscillator: two effects.**
  - **Spectral morph (11):** Vocode, Formant Scale, Harmonic Stretch, Inharmonic Stretch, Smear, Random
    Amplitudes, Low Pass, High Pass, Phase Disperse, **Shepard Tone**, Spectral Time Skew.
  - **Wave morph / distortion (12):** Sync, Formant, Quantize, Bend, Squeeze, Pulse Width, FM←A/B/sampler,
    RM←A/B/sampler.
- **Unison** up to **16** voices.
  - Detune (%), **detune power −5…+5** (the shape of the spread), detune range 0–48 st.
  - Blend, stereo spread.
  - **11 stack styles:** Unison, Center Drop 12/24, Octave, 2× Octave, Power Chord, 2× Power Chord, Major, Minor,
    Harmonics, Odd Harmonics.
  - **Table spread** (±128 frames), **spectral spread**, **distortion spread**.
- **Filters.** Analog, Dirty, Ladder, Digital (each 12/24 dB with notch/B-P-N blends), Diode (low shelf/low cut
  morph), Formant (AOIE/AIUO vowels, X/Y), Comb (comb, flange, band-spread variants), Phaser (±).
- **Quality.** **1× (draft), 2× (recommended), 4×, 8×**.
- **Wavetable editor.** Non-destructive and keyframed.
  - A table is a list of **groups**, each a stack of **components**.
  - Sources: Wave Source, Line Source, Audio File Source, Shepard Tone.
  - Modifiers: Phase Shift, Wave Window, Frequency Filter, Slew Limiter, Wave Folder, Wave Warp.
  - **Keyframes** sit at positions across the 257 frames, with interpolation between them.
  - It can import audio by pitch-splice or vocode, and has an online **text-to-wavetable** (5 a day free).
- **Modulation UX.** Drag a source onto a target, **audition by hovering while playing**, a remap curve per
  connection, and stereo modulation. MPE.
- **Customisation.** A skin editor.
- **Development.** Paid tiers only add content. The GPLv3 source is public but **the author takes no pull
  requests**, and the name, trademark and web services are reserved.

### 3.7 Surge XT ([manual](https://surge-synthesizer.github.io/manual-xt/), [`doc/Wavetables.md`](https://github.com/surge-synthesizer/surge/blob/main/doc/Wavetables.md), [WT format](https://github.com/surge-synthesizer/surge/blob/main/resources/data/wavetables/WT%20fileformat.txt))

- **Structure.** **Two scenes** (A/B, split or layered), each with:
  - 3 oscillators + noise → mixer (FM 2→1, 3→2→1 and similar; ring mod) → filter block (F1 → waveshaper → F2,
    8 configurations, feedback) → VCA → HPF;
  - 6 voice LFOs + 6 scene LFOs per scene: LFO, step sequencer, **MSEG**, **formula (Lua)**;
  - filter EG and amp EG.

  Across the instrument: 8 macros, 16 FX slots, and sends 1 and 2 ([wt-11](screenshots/wt-11-surge-signal-flow.png)).
- **Oscillators.** 12 algorithms: Classic, Modern, Wavetable, Window, Sine, FM2, FM3, S&H Noise, Audio Input,
  String, Twist, Alias.
- **Wavetables.**
  - `.wt` files, and `.wav` with the `clm `, `uhWT`, `srge`/`srgo`, `smpl` or `cue ` chunks.
  - **Frame sizes are powers of two up to 4096, with up to 512 frames.**
  - A wavetable **script evaluator** generates tables.
  - Since 1.3.4 it reads **full-range 16-bit `.wt`**, which makes Bitwig's factory tables load: **Bitwig uses the
    same `.wt` container**.
- **Modulation UX.**
  - Select a source in the routing bar; each modulated slider shows a **second handle for depth** and a coloured
    range bar ([wt-14](screenshots/wt-14-surgext-mod-depth-sliders.png)).
  - A modulation list view.
  - In CLAP only: **non-destructive polyphonic modulation and note expressions**.
- **Accessibility.** An Accessibility submenu (1.2.0), and screen-reader and keyboard work across releases. **The
  only product here that documents it.**
- **Skins and scaling.** Skins can include fonts; zoom steps of 10 %.

---

## 4. The DSP

### 4.1 The table and why 2048 × 256 won

- **Size.** A frame of 2048 samples holds harmonics 1–1024. At 44.1 kHz its fundamental is 21.5 Hz (Serum's
  manual works through this: "F1 −22 cents"). At 96 kHz it is 46.9 Hz. Harmonic 1024 of a 20 Hz note is 20.5 kHz,
  so **2048 is the smallest power of two that keeps a full-bandwidth spectrum for the lowest musical notes**.
- **Memory.** 256 frames × 2048 × f32 = **2 MiB per raw table**.
- **Morphing.** Serum and Hive store only the key frames and compute the in-between frames at load time. Serum
  stores the *interpolation type* in the file, not the interpolated data; that is the `clm ` flag digit. There are
  two kinds of morph:
  - **crossfade** (sample-wise linear mix);
  - **spectral** (interpolate magnitude and phase per harmonic, as additive synths do). Variants zero the
    fundamental's phase or all phases, which gives the smoothest morph and changes the timbre
    ([Serum manual pp. 289, 347](https://www.xferrecords.com/manual/serum-2/docs)).
- **Frame position** is continuous: read two adjacent frames and crossfade by the fractional position. That is
  Serum's "smooth interpolation" and Vital's 257-position space. Discrete ("switch") positions are an option,
  useful for stepped effects.
- **Literature:**
  - Bristow-Johnson, *Wavetable Synthesis 101: A Fundamental Perspective*, AES 101st Convention, 1996: band-limited
    tables and the interpolation error;
  - Massie, "Wavetable sampling synthesis", in Kahrs & Brandenburg (eds.), *Applications of DSP to Audio and
    Acoustics*, Kluwer 1998;
  - Horner, Beauchamp & Haken, "Methods for multiple wavetable synthesis of musical instrument tones", *JAES*
    41(5), 1993: tables as a basis for spectra;
  - Moore, "Table lookup noise for sinusoidal digital oscillators", *Computer Music Journal* 1(2), 1977:
    interpolation noise against table size.

### 4.2 Antialiasing: the three methods in the field

| Method | Who | How it works | Cost | Weakness |
|---|---|---|---|---|
| **Mip-mapped tables** | Surge (read in its GPL source; described, not copied); most wavetable synths | At load time, derive levels that each halve the bandwidth. Surge halves the table length per level with a 63-tap half-band filter, up to 16 levels. At play time, choose a level from the phase increment, and read it with a **windowed-sinc step convolution** (the same machinery as its BLIT Classic oscillator). | About 2× the memory; very cheap to play | With octave levels, the worst-case bandwidth is ½ Nyquist just before a switch (≈12 kHz at 48 kHz). Switching can click unless levels are crossfaded. |
| **Per-block spectral rebuild** | Vital (source structure: every frame is stored as amplitudes, normalised frequencies and phases) | For each voice and oscillator, once per block: take the frame's spectrum, **truncate the harmonics above what the current pitch allows**, apply the *spectral morph*, inverse-FFT into a buffer, then read it with cubic (Catmull-Rom) interpolation, crossfading from the previous block's buffer. | One 2048-point IFFT per oscillator per voice per block: about 10⁵ flops, or ≈40 Mflop/s per voice-oscillator at 48 kHz with 128-sample blocks | CPU scales with voices × oscillators. Buffers change once per block (the crossfade hides it). |
| **Oversampling** | Serum (1/2/4×), Vital (1/2/4/8×) | Run the oscillators, warps and nonlinear filters at k·fs, then decimate. | ≈k× the voice cost, plus a decimator | Latency from the decimator. It does not fix the table's own band limit, but it makes octave mips *sufficient*. |

**Why the combination is right.**
- With **octave mips plus 2× internal oversampling**, the chosen level always has its highest harmonic below the
  internal Nyquist (48 kHz at a 48 kHz base). In the worst case the kept band ends at 24 kHz, which is above
  hearing. Everything between 24 and 48 kHz is removed by the decimator, not folded.
- **Phase-domain warps, sync, FM and PD create new harmonics** that no precomputed table contains. Oversampling is
  what keeps those out of the audio band. That is why Serum's manual says its alias-free playback costs CPU "at
  load and run time", and why Vital recommends 2×.
- **Hard sync** additionally wants a band-limited step at the reset instant: Brandt's minBLEP, or Surge-style
  step convolution. Oversampling alone leaves audible aliasing on sync sweeps at 2×.

**Literature:**
- Stilson & Smith, "Alias-free digital synthesis of classic analog waveforms", ICMC 1996 (BLIT);
- Brandt, "Hard sync without aliasing", ICMC 2001 (minBLEP);
- Välimäki & Huovilainen, "Antialiasing oscillators in subtractive synthesis", *IEEE Signal Processing Magazine*
  24(2), 2007;
- Välimäki, Nam, Smith & Abel, "Alias-suppressed oscillators based on differentiated polynomial waveforms",
  *IEEE TASLP* 18(4), 2010;
- Välimäki, Pekonen & Nam, "Perceptually informed synthesis of bandlimited classical waveforms using integrated
  polynomial interpolation", *JASA* 131(1), 2012;
- Franck & Välimäki, "Higher-order integrated wavetable and sampling synthesis", *JAES* 61(9), 2013 (an
  alternative to mips);
- Laakso, Välimäki, Karjalainen & Laine, "Splitting the unit delay", *IEEE SPM* 13(1), 1996 (fractional-delay
  interpolation);
- Valenzuela & Constantinides, "Digital signal processing schemes for efficient interpolation and decimation",
  *IEE Proc. G* 130(6), 1983 (the polyphase IIR half-bands behind low-latency oversamplers).

**Evidence from the credits.** Serum 2's manual thanks **Andrew Simper** (Cytomic, the trapezoidal SVF papers) and
**Laurent de Soras** (author of the HIIR polyphase half-band oversampling library). That is circumstantial but
consistent with TPT filters and polyphase IIR oversampling.

### 4.3 Reading the table

- **Phase accumulator.** Use a **u32 fixed-point phase** (it wraps for free, is exact and bit-deterministic, and
  gives fs/2³² ≈ 11 µHz of resolution). Firmus v1 accumulates an `f32` phase in [0, 1). Its rounding (half an ulp,
  ≈3·10⁻⁸, per step) is a systematic pitch error of up to ≈0.1 ¢ for a 20 Hz note, depending on where the phase
  sits. It is not drift, but it is not exact either. Vital uses integer phase in SIMD lanes.
- **Interpolation and level length.** A level with H harmonics must be stored oversampled so that a 4-point cubic
  read is accurate: interpolation error falls steeply with the table-oversampling ratio (Niemitalo, *Polynomial
  Interpolators for High-Quality Resampling of Oversampled Audio*, 2001).
  - Level ℓ keeps harmonics ≤ H_ℓ = 1024/2ˡ and is stored at **L_ℓ = clamp(16·H_ℓ, 256, 2048)** samples. Levels
    0–3 stay at 2048, level 4 is 1024, level 5 is 512, and levels 6–10 are 256.
  - Every level from ℓ = 3 up is ≥ 16× oversampled. Levels 0–2 are used only below ≈190 Hz (at 2× and 48 kHz), and
    their less-oversampled top harmonics lie above 24 kHz, which the decimator removes.
  - Memory is ≈5.4× the raw table: **≈10.8 MiB for a full 256-frame table in f32**. Most tables have far fewer
    frames. Mip sets are **shared across instances by content hash**, so the cost is per unique table, not per
    oscillator.
  - Surge instead reads halved levels through its sinc step table; Vital keeps 2048-sample buffers at every
    bandwidth and reads them with Catmull-Rom.
  - If the aliasing test (section 9) fails at the lowest levels, switch those levels to a 6-point 5th-order
    interpolator from Niemitalo's tables. Don't lengthen the tables.
- **Worst-case reads.** Two frames (the position crossfade) × two mip levels (the switch crossfade) × 4 cubic taps
  = 16 reads per unison voice per sample. With 16 unison voices, that is 256 reads per oscillator per sample.
  **This is why every product uses SIMD across unison voices** (Vital and Surge process voices four at a time).

### 4.4 Warps: what they are mathematically

- **Phase-shaping warps** (Bend, Asym, PWM, Mirror, Flip, Remap, Squeeze, Pigments' "Phase Transform") are a
  transfer function φ' = w(φ) applied to the read phase before the lookup. That is Casio-style phase distortion,
  generalised. They are band-unlimited, so they need oversampling.
  - Literature: Lazzarini & Timoney, "New perspectives on distortion synthesis for virtual analog oscillators",
    *CMJ* 34(1), 2010; Kleimola, Lazzarini, Timoney & Välimäki, "Vector phaseshaping synthesis", DAFx 2011.
- **Sync** restarts the read phase at the master's cycle: hard, or soft with a window (Serum's "Var").
- **Waveshaping warps** (clip, fold, tube, diode, tape) are a static nonlinearity y = f(x) on the oscillator
  output. Use **antiderivative antialiasing (ADAA)** plus modest oversampling.
  - Parker, Zavalishin & Le Bivic, "Reducing the aliasing of nonlinear waveshaping using continuous-time
    convolution", DAFx 2016;
  - Bilbao, Esqueda, Parker & Välimäki, "Antiderivative antialiasing for memoryless nonlinearities", *IEEE SPL*
    24(7), 2017;
  - folders: Esqueda, Pöntynen, Parker & Bilbao, "Virtual analog models of the Lockhart and Serge wavefolders",
    *Applied Sciences* 7(12), 2017.
- **FM/PM/AM/RM from another oscillator** (per sample):
  - linear FM **through zero**: the carrier frequency may go negative, which is Serum's default;
  - exponential FM (the pitch drifts with depth);
  - PM (Chowning's "FM"; the pitch is stable);
  - AM and RM.
  - Literature: Chowning, "The synthesis of complex audio spectra by means of frequency modulation", *JAES* 21(7),
    1973.
- **Spectral warps** (Vital) operate on the harmonic array before the IFFT:
  - **stretch:** harmonic *h* moves to h·s (harmonic stretch) or to h + (h−1)·s (inharmonic);
  - **formant scale:** resample the envelope, keep the partial positions;
  - **smear:** a running average across bins;
  - **random amplitudes:** seeded;
  - **spectral LP/HP:** a brick-wall on bins;
  - **phase disperse:** a phase proportional to h²;
  - **skew:** a phase offset proportional to *h*, time-skewing the harmonics;
  - **Shepard:** an octave-looped crossfade.

  They are **alias-free by construction**, because the truncation happens after the warp. They cost one IFFT per
  block, and are the most "expensive-sounding" warps for pads.
- **The neutral point.** Serum's Bend ± and Asym ± are neutral at **50 %**, and Odd/Even at 50 %. That breaks the
  rule "a control at its default changes nothing" unless the default is 50 %. **For Cantus: bipolar warps take an
  amount of −1…+1 with 0 as the identity; unipolar warps take 0…1 with 0 as the identity.**

### 4.5 Unison: the maths behind "wide and expensive"

- **Detune law.** For N voices at detune d and range R semitones, voice *i* gets an offset of
  `R · d · c(uᵢ)` semitones, where `uᵢ ∈ [−1, 1]` is evenly spaced and `c` is the curve:
  - linear: `c(u) = u`;
  - exponential or "power": `c(u) = sign(u)·|u|^p` (Vital's detune power −5…5 maps to p);
  - inverse;
  - random (seeded per note);
  - **Super**: the supersaw law.
- **Super.** The Roland JP-8000 supersaw was measured by Adam Szabó (*How to Emulate the Super Saw*, KTH thesis,
  2010).
  - For 7 voices the offsets are **not** evenly spaced: roughly ±0.11, ±0.063 and ±0.020 of the maximum detune
    ratio around the centre, and the detune knob follows a steep polynomial.
  - The centre and side levels follow separate curves against "mix". Low detune settings stay subtle, and the top
    of the knob gets wide fast.
  - That law, plus random free-running phases and a high-pass at the fundamental, *is* the sound.
- **Level compensation.** With random phases, N uncorrelated voices add in power, so scale by **1/√N**. Serum
  does this automatically and Pigments/Vital similarly. **Blend** then trades the centre voice against the sides:
  define 0 = centre only, 0.5 = all equal, 1 = sides only, with equal-power interpolation.
- **Stereo width.** Alternate voices left and right with equal-power pan at `±width·|uᵢ|`. **Check mono
  compatibility**: the L+R sum must not comb badly. Random phases help, but also keep the centre voice centred.
- **Phase.** If all voices start in phase, the result is the "laser zap" sweep, which Serum's manual explains and
  advises against. Randomise per voice at note-on. **In Cantus the random values must be seeded** (section 8.3),
  or renders stop being repeatable.
- **Spread of timbre across voices.** Offset each voice's WT position or warp amount by `spread·uᵢ`: Serum's
  WT POS/WARP 1/WARP 2 spreads, Vital's table, spectral and distortion spreads. This costs nothing extra and makes
  a stack sound like an ensemble rather than a chorus.
- **Stacks.** Replace uᵢ with interval sets (octaves, fifths, chords, harmonic series) instead of detune: Vital's
  11 styles, Serum's 5.
- **CPU.** Unison multiplies the oscillator cost by N and nothing else, as long as the filter comes after the sum.
  Serum's manual advises 3–7 voices and a chorus on an FX bus instead of more.

### 4.6 Filters

- **The base: the TPT/ZDF state-variable filter** (Zavalishin, *The Art of VA Filter Design*, rev. 2.1.2, 2020;
  Simper, "Linear trapezoidal integrated SVF", Cytomic technical paper, 2013). **Cantus already has it**
  (`cantus-dsp/src/filter.rs`).
  - Two limits today: its resonance stops at Q≈10, so it does not self-oscillate, and it clamps the cutoff at
    0.45·fs.
  - Firmus v1 recomputes `tan` **per sample per voice**. Compute the coefficients at control rate, or use a `tan`
    approximation.
- **Ladder filters** are the characterful ones:
  - Moog-style: Huovilainen, "Non-linear digital implementation of the Moog ladder filter", DAFx 2004;
    D'Angelo & Välimäki, "An improved virtual analog model of the Moog ladder filter", ICASSP 2013, and
    "Generalized Moog ladder filter" parts I/II, *IEEE/ACM TASLP* 22(12), 2014;
  - the **diode ladder** ("acid") and **Sallen-Key/MS-20** types are derived in Zavalishin's book.

  The nonlinearities sit inside feedback loops, so ADAA does not apply directly. **Oversample** (the global
  quality setting) and use a per-stage `tanh` approximation.
- **Comb, flanger, phaser and formant** are cheap and add much variety.
  - Comb: a fractional delay line with feedback, where the delay is 1/f_cutoff.
  - Formant: 2–5 parallel band-passes at vowel formants (Peterson & Barney 1952 data), morphed by one parameter.
- **What sounds expensive in a synth filter:**
  - the cutoff is **smoothed in the log domain**;
  - resonance is **gain-compensated**, so the passband doesn't collapse as Q rises;
  - drive with **"clean mode" gain staging**: Serum 2's −24/+24 dB trick keeps the model in its linear region;
  - self-oscillation that tracks the keyboard to within a few cents;
  - **stereo cutoff offset** (Serum's pan).

### 4.7 Envelopes, LFOs, chaos

- **Envelopes.** Use **curved segments**: an exponential approach with a shape parameter. Also:
  - DAHDSR (delay, attack, hold, decay, sustain, release), a BPM mode, and legato invert;
  - Pigments 7 advertises **S-shaped attack and release curves** "for smoother audio quality", which reduces clicks
    at the segment ends;
  - timing measured in samples from note-on, never from a block edge.
- **LFOs.**
  - Drawn multi-segment shapes (points plus per-segment curvature; Serum's LFOTool `.XferShape`, Pigments'
    Functions, Vital's shapes) with grid snap, step drawing and smoothing;
  - Free, Retrigger, Envelope (one-shot with a loop-back point) and Sustain-loop modes;
  - rise and delay; per-voice (poly) or global (mono); up to audio rate (Serum's ×10 mode reaches 1 kHz).
- **Chaos sources.**
  - Lorenz: σ = 10, ρ = 28, β = 8/3 (Lorenz, *J. Atmos. Sci.* 20, 1963).
  - Rössler: a = b = 0.2, c = 5.7 (Rössler, *Phys. Lett. A* 57, 1976).
  - Integrate with RK4 at control rate from a **fixed, seeded initial state**, normalise, and let the rate scale
    the time step. They are cheap and deterministic if seeded, and look spectacular drawn as the attractor
    ([wt-05](screenshots/wt-05-serum2-lorenz-lfo.png)).

### 4.8 Spectral and granular engines (Serum 2, Pigments)

- **Spectral.** STFT analysis and resynthesis: a phase vocoder with scan (time) independent of pitch, a spectral
  filter, and **transient preservation**.
  - Dolson, "The phase vocoder: a tutorial", *CMJ* 10(4), 1986;
  - Laroche & Dolson, "Improved phase vocoder time-scale modification of audio", *IEEE TSAP* 7(3), 1999;
  - Röbel, "A new approach to transient processing in the phase vocoder", DAFx 2003;
  - Serra & Smith, "Spectral modeling synthesis", *CMJ* 14(4), 1990.
- **Granular.** Overlapping windowed grains with jittered position, pitch and pan (Roads, *Microsound*, MIT Press
  2001; Truax, "Real-time granular synthesis with a DSP", *CMJ* 12(2), 1988). Serum caps it at 256 grains.
- Both belong in **P2** for Cantus. They overlap with the sampler report's scope and need a streaming sample
  engine first.

### 4.9 Numbers at a glance

| Quantity | Serum 2 | Pigments 7 | Phase Plant | Vital | Surge XT | Hive 2 |
|---|---|---|---|---|---|---|
| Frame size (samples) | 2048 | 2048 | 2048 | 2048 | power of two, ≤ 4096 | 2048 in the `uhWT`/Serum exports |
| Frames | ≤ 256 | ≤ 256 | 256 | 257 positions | ≤ 512 | multi-tables of up to 16 parts |
| Oscillators | 3 + sub + noise | 2 engines + utility | ≤ 32 generators | 3 + sampler | 3 + noise per scene, 2 scenes | 2 + 2 subs |
| Unison max | 16 | 8 | per generator, plus global | 16 | per oscillator (algorithm-dependent) | 16 |
| Oversampling | 1/2/4× | not documented | not documented | 1/2/4/8× | per-filter/character options | Clean/Normal/Dirty |
| Polyphony | user-set, "8 enough, 16 a lot" | ≤ 32 | user-set | ≤ 32 | user-set (Poly) | ≤ 16 |
| Envelopes / LFOs / macros | 4 / ≤10 / 8 | 3 / 3 (+3 functions, 3 random, 2 combinate) / 4 | ≤ 32 modulators / 8 macros | 6 / 8 (+4 random) / 4 | 2 EG per scene / 12 per scene / 8 | 4 / 2 (+3 functions) / — |
| Mod slots | unbounded matrix | per route | per modulator | 64 | unbounded | 12×2 |
| Control rate | sample-accurate automation option | — | ≈64 samples (≈100 Hz cap) | per block, SIMD | per block (32 samples) | — |

---

## 5. The interface

### 5.1 Layout and visual language

- **Serum 2** ([wt-01](screenshots/wt-01-serum2-main.png)).
  - **Grid:** a single fixed page grid with five top tabs (OSC, MIX, FX, MATRIX, GLOBAL).
  - **Rows:** a horizontal row of equal-width module columns (SUB, A, B, C, NOISE, F1, F2). Under it the modulation
    band: a macro column, then source tabs with the selected source's editor (ENV, LFO or VELO/NOTE curve), then
    voicing. At the bottom, CLIP, ARP and the keyboard.
  - **Hierarchy by value:** module headers and displays are near-black; the **controls sit on light blue-grey
    panels**.
  - **Colours:**
    - knob body ≈ #23292F on panel ≈ #8E9CAB, **≈ 5:1** (FabFilter's knobs are 1.1–1.2:1);
    - the blue value tick on the dark body ≈ 5:1;
    - dark navy labels on the panel ≈ 5:1;
    - green numeric readouts on near-black **≈ 9:1**;
    - yellow for "current" (frame, grain, phase line).
  - **Type:** a condensed geometric sans in caps; the numbers are not visibly tabular.
  - **Density:** high, but every section has a title.
  - **Resizing:** drag the corner or use a % menu from the logo, and set a default size.
- **Pigments 7** ([wt-08](screenshots/wt-08-pigments-banner.png)).
  - **Rows:** a three-band page: engines and filters on top; **the modulation strip as the spine**; the source
    editor and macros below.
  - **Colours:**
    - dark neutrals, with knobs as light-grey discs on near-black (**≈ 5–9:1**) and blue value arcs;
    - secondary labels ≈ 4.3:1;
    - inactive tile text ≈ 2.5:1, which fails;
    - **colour carries the source group everywhere** (tile, ring, pie).
  - **Themes:** a light Play-view variant exists (`fam-08`).
  - **Hover dependence:** state is often revealed only on hover (the rings appear when you hover a source).
- **Vital** (`fam-02`).
  - Flat, dark, large type, generous displays.
  - **Colour identifies the section:** purple oscillators, orange filters, teal modulation.
  - Knob bodies ≈ #2B2D31 on panels ≈ #3B3E43, **≈ 1.3:1**. The saturated arc (≈ 5:1) carries the value.
  - A skin editor lets users recolour it, a precedent for Cantus's editable palette.
- **Surge XT** ([wt-12](screenshots/wt-12-surgext-main.png)).
  - The densest of the seven, with **sliders rather than knobs** (handle ≈ 4.4:1 against the grey-blue panel) and
    orange for the selected state (≈ 9:1 with black text).
  - Surge's own documentation asks for labels everywhere, and gets them.
  - Skins and zoom. Documented accessibility.
- **Hive 2** ([wt-09](screenshots/wt-09-hive2-fullui.jpg)).
  - Mirrored left and right halves around a hexagonal display. Two skins.
  - Knobs are silver on mid-grey (≈ 4:1).
  - **Small text**, about 9–10 px at 100 %.
  - Faders for ADSR, and good "everything on one page" density.
- **Phase Plant** (`fam-03`).
  - A module stack plus lanes plus a scrolling modulator strip.
  - Blue arcs, **orange for control-rate and green for audio-rate modulation**.
  - Readable, but the routing is only implied.
- **Massive X.** Multiple pages (Synth, Routing, Performer, Play). **The only visual patch view.** Reviewers call
  the UI "problematic" (Synth Anatomy, 2026).

### 5.2 Interaction patterns worth keeping

| Pattern | Best example | Keep / change |
|---|---|---|
| **Drag a source tile onto a control; valid targets light up while dragging** | Vital (teal targets, `fam-04`), Serum, Pigments | **Keep.** Add a keyboard path ("Modulate by…"), as family.md 7.6 says. |
| **Depth drawn as a ring or bar on the target, adjustable in place** | Serum (halo plus corner handle, Alt-drag), Surge (second handle) | **Keep, and make it visible at rest** (Serum shows a grey halo for unselected sources; Pigments hides it). |
| **Count badges on source tabs** (how many destinations) | Serum | **Keep.** Clicking the badge lists and selects the targets. |
| **Live mini-graphics of every source** | Pigments' strip | **Keep.** It is the overview FabFilter lacks. |
| **Per-voice display: newest voice bright, others grey** | Phase Plant | **Keep** for LFO and envelope displays and the target ring dot. |
| **Aux/"via" source and a curve per slot, with slew** | Serum matrix, Pigments sidechain | **Keep.** It is the family.md "via". |
| **2D view shows the warp live; 3D view shows every frame, with the current one highlighted** | Serum ([wt-02](screenshots/wt-02-serum2-wavetable-osc.png)), Pigments | **Keep.** Draw interpolated frames dimmer (Serum: grey for interpolated, green for real, yellow for current). |
| **Grains and play positions drawn live** | Serum granular ([wt-04](screenshots/wt-04-serum2-granular-osc.png)) | Keep for P2. |
| **Filter graph editable by dragging** (cutoff × resonance) | Serum, Vital | Keep. It matches our Aequalis display grammar. |
| **Unison field changes colour with CPU cost** | Serum | Replace with a **real measured CPU and voice readout** from the host. |
| **Hidden right-click menus for major features** (phase legato, smooth interpolation, clean drive, ×10 rate) | Serum | **Don't copy.** Reviewers note that much of Serum 2 lives in right-click menus. Surface them as visible chips. |

### 5.3 Praise and criticism of the interfaces (press and users)

- Serum 2 is **"intuitive"**, but many advanced features are **hidden in right-click menus** (MusicTech). Its
  350-page manual signals the depth (CFA-Sound).
- Vital's drag-and-drop modulation is **"buttery smooth"**, and the layout is "clean, functional and fun"
  (pluginoise).
- Phase Plant: the blank canvas **"actively works against you"** if you want to load and play; there is no visual
  signal flow (ModWheel).
- Pigments 7: the Play page is "cleaner and more like a virtual instrument" (Synth Anatomy).
- Massive X: the UI remains a complaint, along with the lack of wavetable import (Synth Anatomy).

---

## 6. What reviewers and users say

*Forum material is anecdotal and self-selected. Where only a search excerpt or a fetch summary was available, the
line says so.*

- **Serum 2**
  - **Praised:** MusicTech (8/10) calls the spectral oscillator "mind-blowing". The first-day KVR thread praises
    the spectral oscillator, the dispersion/diffusor filter, audio-rate modulation, FM/RM/PM, convolution and the
    piano-roll clip sequencer.
  - **Criticised:**
    - it is **"more CPU-hungry than ever"**; MusicTech advises 16 GB of RAM;
    - the price for newcomers, and "so many powerful competitors";
    - the multisample mode is somewhat gimmicky (all MusicTech);
    - KVR users found the new filters weak on "Moog zaps and snappy analog percussion", thought the filters respond
      poorly to fast envelopes, reported artefacts when the spectral scan changes direction, and had to re-import
      thousands of Serum 1 tables.
  - An Xfer developer said on KVR that the cycle took 4.5 years and that more filters are planned.
  - A more positive CPU view (CFA-Sound): the basic elements are *lighter* than Serum 1.
- **Pigments 7** (Synth Anatomy review):
  - **Praised:** character-focused filters (Rage, Ripple, Reverb), FM on Classic filters, smoother S-curve
    envelopes, CPU optimisation (though the gains were "not immediately apparent").
  - **Criticised:** the factory presets have an "overdose" of effects, and there is **still no multisample
    oscillator**.
- **Vital** (pluginoise, 2026):
  - **Praised:** spectral warping, drag-and-drop, a nearly complete free tier, **low CPU relative to competitors**,
    and 8× oversampling for export (2× advised while producing).
  - **Criticised:** a thinner preset library, an overwhelming modulation depth, and harsh extreme warps.
- **Phase Plant** (ModWheel): unlimited routing and the best audio-rate cross-modulation, but intimidating, paid
  effects, no visual flow, and CPU that scales with the patch.
- **Massive X** (Synth Anatomy, 1.7 news): development continues despite NI's insolvency proceedings. Automation
  was fixed in 1.7, but there is still no table import and the UI is still an issue.
- **Hive 2:** its reputation is **"CPU-light workhorse"**. u-he's own marketing calls it "sleek, streamlined,
  supercharged", and the Clean/Normal/Dirty switch is the CPU/character trade-off made explicit. *(No 2025–26
  review was fetched; the reputation is from general knowledge.)*
- **Surge XT:** it is free and deep, and its CLAP polyphonic modulation and accessibility work are unique. It
  looks technical and is slow to reach 1.4 (the milestone notes the Lua work "took 2 years longer than expected").

---

## 7. Gaps none of them fills

1. **Undo is local to the plug-in and separate from the project.** Serum 2 advertises "comprehensive undo and redo"
   inside itself, and so do the others, but none can undo "the table edit I made before I moved the mixer fader".
2. **Wavetable edits are mostly destructive and unversioned.** Serum rewrites frames. Phase Plant 3's wishlist on
   KVR asks for non-destructive editing. **Vital is the exception** (keyframed components). None keeps a *history*
   of a table.
3. **Modulation stops at the plug-in boundary.** An LFO in Serum cannot duck a send in the host mixer, and a host
   envelope follower on the kick cannot reach Serum's filter without sidechain routing plus an in-synth follower.
   Surge's CLAP polyphonic modulation is the only partial bridge, and only in hosts that support it.
4. **No product shows the routing and the modulation as one picture.** Serum and Pigments have lists and strips.
   Massive X has cables but no modulation. Phase Plant is criticised for having none.
5. **Aliasing is invisible.** No product shows when a warp, sync or FM setting is aliasing at the current quality
   factor. Users find out by ear, or not at all.
6. **Renders are not guaranteed repeatable.** Random phases and free-running LFOs differ run to run unless the user
   finds options like Serum's HOST switch. Nobody documents bit-identical bounces.
7. **Per-note expression is limited to MPE's three axes and the host's goodwill.** Serum lists MPE X/Y/Z; Surge
   has CLAP note expressions. Nobody lets you **draw per-note curves for any parameter** in a piano roll, because
   none of them owns the piano roll.
8. **Resampling into a table is manual.** Serum's manual walks through tuning another synth to "F1 −22 cents" and
   rendering so that each cycle is 2048 samples. The host could do that automatically.
9. **Duplicated sequencers.** Serum 2's clip sequencer and arpeggiator and Pigments' sequencer rebuild what a DAW
   already has, because the plug-in cannot rely on the host.
10. **Accessibility and localisation.** None but Surge documents screen-reader support. None ships a Spanish UI.
    Hive's text is tiny.
11. **CPU transparency.** Only indirect cues exist: Serum's unison colour, Phase Plant's latency readout. None
    shows measured CPU per voice or per patch.
12. **Exchange of tables in both directions.** Vital's `.vitaltable` is read by nobody else. Massive X imports
    nothing. Hive only recently exports Serum format.

---

## 8. How Cantus beats them

**Starting point:**
- Firmus v1: 2 PolyBLEP oscillators with an `f32` phase, the TPT SVF, 2 ADSRs, 16 voices, deterministic phase
  reset, voice stealing "free → oldest releasing → oldest";
- the half-band oversampler from Comprimo (63 + 19 taps, whole-sample latency);
- the Aequalis conventions: sparse `key → f32` parameters, stable numbering, a 16-sample modulation grid counted
  from the stream start;
- the command bus with undo fuzz;
- the family language (research/fabfilter/family.md §7).

**Decision to record first (ADR):** Firmus II is a **new `InstrumentSpec` variant**, and Firmus v1 stays
bit-identical, because changing its DSP would move renders of existing projects. A "Convert to Firmus II" command
maps v1 parameters into a Firmus II patch (section 8.5). Both locales get strings.

**Name:** *Tabula* (Latin for "tablet" or "table", as in wavetable; it fits Firmus, Pulsus, Aequalis and Spatium),
with "Firmus II" as the working title if the owner prefers continuity.

### P0: parity (without these we are not credible)

1. **An alias-free wavetable core.**
   - 2048-sample frames, up to 256 frames. Accept any power of two from 256 to 4096 on import and resample to 2048.
   - Octave mip levels are built off the audio thread by FFT truncation and stored at the lengths in 4.3. Crossfade
     across the top quarter-octave of each level so switching never clicks.
   - **u32 phase**, cubic Hermite reads, continuous frame position with crossfade. A spectral-morph option is baked
     at load time.
   - **Quality 1×/2×/4×, default 2×, with one decimator per instance** (after the voice sum; decimation is linear,
     so summing first is exact). Draft 1× uses half-octave mips so the band stays ≥ 17 kHz.
2. **Unison up to 16 per oscillator.**
   - Detune with a curve: Linear, **Super (Szabó law)**, Power, Inverse, Seeded Random.
   - Range, equal-power blend, width, stack (Off, Octave, Octave+5th, Centre −12/−24, Power, Major, Minor, Harmonics,
     Odd).
   - Frame and warp spread across the voices, 1/√N level compensation, **seeded** random phases. SIMD across voices.
3. **Two warp slots per oscillator, where amount 0 is the identity.**
   - Sync (hard or soft, with a band-limited step at the reset), Bend, Asym, PWM, Flip, Mirror, Quantize, Odd/Even.
   - Remap (a drawn curve), Sine Fold and Soft/Hard Clip (with ADAA).
   - FM (linear through-zero, or exponential), PM, AM and RM from another oscillator, the sub, the noise or itself.
   - The 2D view draws the warped cycle live.
4. **Two filters, serial↔parallel as one continuous control.**
   - The existing TPT SVF in LP/BP/HP/Notch/Peak at 12 and 24 dB, and a morph LP→BP→HP.
   - A **ZDF nonlinear ladder** at 24/12 dB, with drive and "clean" gain staging.
   - **Comb ±** (feedback sign).
   - Keytrack, stereo cutoff offset, mix. Coefficients at control rate; resonance that reaches self-oscillation.
5. **The modulation system, with every edit a host command.**
   - Sources: 4 envelopes (ENV1 = amp), 8 LFOs (drawn shapes, Free/Retrig/Env/Loop, poly or mono, sync, rise,
     delay, smooth, Lorenz, Rössler, S&H), 8 macros, velocity, release velocity, note, wheel, bend, channel and
     poly aftertouch, MPE/CLAP note expressions, seeded note-on randoms, alternate, voice index.
   - **64 slots with stable ids.**
   - Drag-to-modulate; rings visible at rest; count badges; a matrix view with a via source, a curve and slew.
6. **A wavetable editor.**
   - 3D/2D view, a frame strip (select, reorder, insert, remove, copy), draw tools, and an **FFT bar editor with
     magnitude and phase**.
   - Morph (crossfade, spectral, spectral with zero phase), process (normalise, DC, fades, brick-wall filter).
   - Import modes (below). **Non-destructive by storing the recipe** (8.4).
7. **Table import and export on day one.**
   - Import WAV (with `clm `, `uhWT`, `srge`/`srgo`, `smpl`, `cue ` or no chunk), `.wt`, and folders of single
     cycles.
   - Export WAV with a `clm ` chunk (Serum, Vital, Surge, Pigments and Phase Plant can all read it) and `.wt`.
8. **Voices.** Polyphony 1–32 (default 16), mono and legato, glide (always or legato, with a curve), the v1
   stealing policy, and a same-note policy. Note ids are carried through for per-note expression.
9. **Presets.** Presets live in the shared Cantus library (family.md 4.1), with **instrument + insert-chain presets**
   (so a "Serum-style" patch with reverb is one item), macros playable during preview, and tags.
10. **Invariants that are already Cantus's culture:**
    - deterministic output;
    - block-size independence;
    - null tests;
    - "a control at its default changes nothing";
    - no allocation on the audio thread (tables are swapped in by pointer);
    - undo-fuzz coverage;
    - a schema bump plus a fixture;
    - EN/ES strings.

### P1: differentiators (only a host-native instrument can do most of these)

1. **One undo history, including tables.**
   - A knob drag, a matrix edit, **a stroke in the wavetable editor** and a mixer move are all commands in the same
     project history, fuzz-tested.
   - The device header's undo and redo show the project history **filtered to this instrument**.
   - A table's recipe history can be scrubbed ("go back to before I added the fold").
2. **Modulation that crosses the plug-in boundary** (the family.md 7.6 service).
   - **Mono sources out:** a global LFO, a macro or an envelope follower in Firmus II can drive *any* parameter on
     the track or in the mixer: a send level, an Aequalis band, the strip pan.
   - **Host sources in:** another track's envelope follower (a kick ducking the pad's cutoff, with no sidechain
     routing), transport and bar phase, arrangement section markers, and any automation lane as a modulation
     source.
   - **Rule:** poly sources never leave the voice. Only mono sources cross, and the UI says so.
3. **The piano roll is the clip sequencer.**
   - **Per-note expression lanes for any Firmus II parameter** (WT position, warp, cutoff) are drawn under each note.
     That is CLAP note expressions without the three-axis limit, because we own both ends.
   - **Channel-rack parameter locks per step** (Elektron-style: step 5 sets `flt.1.cutoff`), stored as note
     expressions.
   - The arpeggiator is a **host MIDI device** usable in front of any instrument, including third-party ones. It
     follows the project key and scale (Serum applies key and scale to its arp output; we apply the project's).
4. **Resample anything into a table.** Select a clip, a track or a time range and choose "Make wavetable".
   - The engine **renders offline and deterministically**. For an instrument source it renders the note at exactly
     fs/2048 Hz, so each cycle is 2048 samples with no resampling. That automates Serum's manual "F1 −22 ¢"
     procedure.
   - Import modes: fixed cycle length, pitch-average, pitch-follow, FFT (256–2048), note name.
5. **An aliasing and quality indicator.**
   - The engine measures **energy above the base-rate Nyquist before decimation**, per oscillator, cheaply (a
     one-pole high-pass energy estimate at the oversampled rate).
   - A chip lights up ("Warp 1 aliasing at 2×: try 4×"). No competitor does this.
6. **CPU and voices you can read.** The host already times each strip. The header shows **voices in use and
   measured CPU for this instrument**, with a hint ("unison 16×3 = 48 oscillators per voice"), instead of
   Serum's colour cue.
7. **The AI command bridge.**
   - "Warmer", "more movement", "make it a supersaw", "explain this patch" become plain `SetInstrumentParam`,
     `AddModSlot` and `SetWavetableRecipe` commands, previewable and undoable.
   - The AI reads the matrix as structured data.
   - **Local table generation from a description** (harmonic recipes and formulas) competes with Vital's metered
     cloud text-to-wavetable.
8. **Repeatable renders by contract.** Every random value is derived from `(instance seed, note id, voice lane,
   purpose)`, and free LFOs derive their phase from the transport sample position. A bounce from bar 33 equals the
   same bars of a full bounce. This is documented and tested.
9. **Themes, accessibility, i18n.** The editable palette, AA contrast on knob edges (Serum's ≈5:1 is the
   benchmark; Vital's 1.3:1 is the trap), AccessKit names and values for every parameter, EN/ES, and help text in
   the help strip.
10. **Per-voice FX inside the instrument** (a per-voice distortion or comb before the voice sum). This is the one
    kind of FX a host chain cannot provide. The instance FX stay in the host chain.

### P2: later

- **Spectral warps** (Vital-style, per-block IFFT only when active): formant scale, harmonic and inharmonic
  stretch, smear, spectral LP/HP, phase disperse, skew, Shepard.
- **Spectral and granular oscillators** (after the sampler engine), **multisample/SFZ** (shared with `sampler.md`),
  image-to-table, a formula parser, `.vitaltable` modifiers, `.uhm` scripts.
- Diode ladder, formant, phaser and MS-20 filters; Path (XY) LFOs; a voice-control sequencer; per-voice analog
  drift presets; MTS-ESP.
- Importing Vital `.vital` presets (open JSON) as a migration path, approximated and with a report of what did not
  map.

### 8.1 Parameter list (P0)

Keys follow Aequalis: sparse `key → f32` (ADR 0017), with **stable names and numbers**. Non-scalar state (table
recipes, LFO shapes, remap curves) has its own commands and is stored as content-hashed project assets. Bipolar
amounts are always 0 = identity. Unless marked, a parameter can be automated and modulated.

**Global and voice**

| Key | Range | Default | Unit / notes |
|---|---|---|---|
| `gain` | −36 … +12 | −6 | dB (the v1 range and default) |
| `quality` | 1× / 2× / 4× | 2× | stepped; not modulatable |
| `voice.poly` | 1 … 32 | 16 | voices |
| `voice.mode` | Poly / Mono / Legato | Poly | stepped |
| `voice.glide` | 0 … 10 | 0 | s (log) |
| `voice.glide_mode` | Always / Legato | Legato | stepped |
| `voice.glide_curve` | −1 … +1 | 0 | 0 = constant-time linear in pitch |
| `voice.same_note` | New voice / Retrigger | New voice | v1 behaviour |
| `voice.bend_up`, `voice.bend_down` | 0 … 48 | 2 | st |
| `voice.vel_amp` | 0 … 1 | 1 | depth of velocity → amp (v1 uses a v^1.5 curve) |
| `voice.drift` | 0 … 1 | 0 | per-voice seeded pitch (±5 ¢ at 1) and cutoff (±⅙ oct) drift |

**Oscillators** `osc.a`, `osc.b`, `osc.c` (the table itself is set with the `SetWavetable` command)

| Key | Range | Default | Unit / notes |
|---|---|---|---|
| `on` | 0/1 | A = 1, B = C = 0 | |
| `pos` | 0 … 1 | 0 | table position (displayed as frame 1…N) |
| `pos_mode` | Switch / Crossfade | Crossfade | stepped; spectral morph is baked into the table |
| `oct` | −4 … +4 | 0 | oct, stepped |
| `semi` | −12 … +12 | 0 | st, stepped |
| `fine` | −100 … +100 | 0 | ¢ |
| `coarse` | −64 … +64 | 0 | st, continuous (the pitch-modulation target) |
| `keytrack` | 0/1 | 1 | off = fixed at C3 |
| `phase` | 0 … 360 | 0 | ° |
| `phase_rand` | 0 … 100 | 100 | %, **seeded** (8.3) |
| `level` | −∞ … +6 | 0 | dB |
| `pan` | −1 … +1 | 0 | equal-power |
| `route` | F1 / F2 / F1+F2 / Direct | F1 | stepped; an off filter is a wire |
| `uni.voices` | 1 … 16 | 1 | at 1, every `uni.*` below is inert (tested) |
| `uni.detune` | 0 … 1 | 0.25 | fraction of `uni.range` |
| `uni.range` | 0 … 12 | 2 | st at detune = 1 |
| `uni.curve` | Linear / Super / Power / Inverse / Random | Super | stepped |
| `uni.power` | −5 … +5 | 0 | only for Power; 0 = linear |
| `uni.blend` | 0 … 1 | 0.5 | 0 = centre only, 0.5 = equal, 1 = sides only (equal-power) |
| `uni.width` | 0 … 1 | 1 | stereo spread of the voices |
| `uni.stack` | Off / Oct / Oct+5 / C−12 / C−24 / Power / Major / Minor / Harm / Odd | Off | stepped |
| `uni.pos_spread` | −1 … +1 | 0 | fraction of the table |
| `uni.warp_spread` | −1 … +1 | 0 | fraction of each warp's amount range |
| `warp1.mode`, `warp2.mode` | Off, Sync, Bend, Asym, PWM, Flip, Mirror, Quantize, OddEven, Remap, SineFold, SoftClip, HardClip, FM, FMexp, PM, AM, RM | Off | stepped |
| `warp1.amt`, `warp2.amt` | −1 … +1 (bipolar modes) or 0 … 1 | 0 | 0 = identity in every mode |
| `warp1.src`, `warp2.src` | A / B / C / Sub / Noise / Self | the next oscillator | stepped; FM/PM/AM/RM only |
| `warp1.var`, `warp2.var` | 0 … 1 | 0 | mode-specific: sync softness, quantize steps, fold symmetry |

**Sub and noise**

| Key | Range | Default | Notes |
|---|---|---|---|
| `sub.on` | 0/1 | 0 | |
| `sub.wave` | Sine / Tri / Saw / Square / Pulse25 | Sine | PolyBLEP, at the oversampled rate |
| `sub.oct` | −3 … 0 | −1 | oct |
| `sub.level`, `sub.pan` | as for the oscillators | 0 dB, 0 | |
| `sub.route` | F1 / F2 / F1+F2 / Direct | Direct | the sub usually bypasses the filter |
| `noise.on` | 0/1 | 0 | |
| `noise.color` | −1 … +1 | 0 | tilt: −1 brown, 0 white, +1 blue |
| `noise.stereo` | 0 … 1 | 1 | decorrelated L/R |
| `noise.level`, `noise.pan`, `noise.route` | | −12 dB, 0, F1 | |

**Filters** `flt.1`, `flt.2`, plus `flt.routing`

| Key | Range | Default | Notes |
|---|---|---|---|
| `on` | 0/1 | 0 | off = a bit-exact wire |
| `type` | SVF LP12, LP24, BP, HP12, HP24, Notch, Peak, Morph; Ladder LP24, LP12, HP24; Comb+, Comb− | SVF LP12 | stepped (v1 is SVF LP12) |
| `cutoff` | 16 … 20 000 | 2 200 | Hz (log); modulation depth in **octaves** |
| `res` | 0 … 1 | 0 | reaches self-oscillation near 1 on SVF and Ladder |
| `drive` | 0 … 36 | 0 | dB, with a clean gain-staging option |
| `keytrack` | −1 … +2 | 0 | octaves of cutoff per octave of note, around C4 (v1's `key_track`) |
| `var` | 0 … 1 | 0 | morph position (Morph), damping (Comb), "fat" (Ladder) |
| `pan` | −1 … +1 | 0 | stereo cutoff offset, ±1 oct at the ends |
| `mix` | 0 … 1 | 1 | |
| `level` | −∞ … +12 | 0 | dB |
| `flt.routing` | 0 … 1 | 0 | 0 = serial F1→F2, 1 = parallel, continuous |

**Envelopes** `env.1` (amp) … `env.4`

| Key | Range | Default (env.1 / others) | Notes |
|---|---|---|---|
| `delay` | 0 … 10 | 0 | s |
| `attack` | 0 … 20 | 0.005 / 0.01 | s (log) |
| `hold` | 0 … 10 | 0 | s |
| `decay` | 0.001 … 20 | 0.15 / 0.3 | s |
| `sustain` | 0 … 1 | 0.8 / 0.3 | linear level (shown in dB as well) |
| `release` | 0.001 … 30 | 0.25 / 0.3 | s |
| `a_curve`, `d_curve`, `r_curve` | −1 … +1 | 0.3, −0.5, −0.5 | segment curvature; 0 = linear |
| `sync` | 0/1 | 0 | times in note values |
| `legato_invert` | 0/1 | 0 | always retrigger |

**LFOs** `lfo.1` … `lfo.8` (the shape is an asset set with `SetLfoShape`)

| Key | Range | Default | Notes |
|---|---|---|---|
| `type` | Shape / S&H / Lorenz / Rössler | Shape | stepped |
| `mode` | Free / Retrig / Envelope / Loop | Retrig | Free derives its phase from the transport |
| `poly` | 0/1 | 1 | per voice or global (only global can leave the instrument) |
| `sync` | 0/1 | 1 | |
| `rate` | 0.01 … 100 (×10 → 1 kHz) | 1 | Hz when `sync` = 0 |
| `div` | 8/1 … 1/64, plus T and D | 1/4 | when `sync` = 1 |
| `phase` | 0 … 1 | 0 | |
| `rise`, `delay` | 0 … 10 | 0 | s |
| `smooth` | 0 … 1 | 0 | one-pole on the output |
| `dir` | Fwd / Rev / PingPong | Fwd | |
| `bipolar` | 0/1 | 1 | the output range the matrix sees |

**Macros** `macro.1` … `macro.8`: 0 … 1, default 0, with a label and a bipolar flag. Each is also a destination.

**Modulation slots** `mod.N` (stable N, never renumbered, up to 64)

| Field | Values | Default |
|---|---|---|
| `src` | any source (8.3) | — |
| `dst` | any parameter key of this instrument (P1: any key on the track or in the mixer, mono sources only) | — |
| `depth` | **in target units**: oct for cutoff, st for pitch, dB for levels, fraction for positions | 0 (bit-identical to no slot) |
| `bipolar` | 0/1 | chosen by the target at drag time: a centred target gets bipolar (Serum's rule) |
| `curve` | −1 … +1, or a drawn curve | 0 |
| `via`, `via_invert` | source, 0/1 | none |
| `slew_up`, `slew_down` | 0 … 2 s | 0 |
| `bypass` | 0/1 | 0 |

### 8.2 Voice architecture

```
note on (note id, pitch, velocity) ─► allocator (free → oldest releasing → oldest; mono / legato / glide)
                                         │
 per voice (fixed arrays, allocated once; SIMD over unison lanes; runs at quality × fs)
  ┌─────────────────────────────────────────────────────────────────────────────────────────┐
  │ mod eval every 16 base samples on a stream-anchored grid, linear ramps per sample        │
  │   poly sources: ENV1-4, poly LFOs, vel, note, MPE/note expr, seeded randoms              │
  │   mono sources: from the instrument's global block, or the host modulation service (P1)  │
  │                                                                                          │
  │ OSC A/B/C: u32 phase ×16 lanes → warp1 → warp2 → mip select (+crossfade)                 │
  │            → 2-frame cubic read → unison pan/level → route                               │
  │ SUB (PolyBLEP)   NOISE (seeded, colour tilt)                                             │
  │        │ F1 in / F2 in / direct                                                          │
  │ FILTER 1 ──(serial ↔ parallel, flt.routing)── FILTER 2   (TPT SVF / ZDF ladder / comb)   │
  │        ▼                                                                                 │
  │ VCA (ENV1 × velocity law) ─► [P1: per-voice FX] ─► voice stereo out                      │
  └─────────────────────────────────────────────────────────────────────────────────────────┘
        Σ voices (at the oversampled rate)
        ─► ONE half-band decimator per instance (Comprimo's filters; latency reported for PDC)
        ─► gain ─► host insert chain (Aequalis, Comprimo, Spatium …)
```

- **Control rate.** Every 16 base-rate samples, **counted from the start of the stream**: the Aequalis rule, which
  makes the output independent of block size. Values ramp linearly per sample, in the log domain for cutoff and
  pitch. Audio-rate paths (FM/PM/AM/RM, and later the oscillator as a modulation source) run per sample. This beats
  Phase Plant's ≈64-sample control rate.
- **Real time.**
  - Tables and mips are built on a worker thread and handed over by pointer swap. The old table is returned
    through a garbage queue; the audio thread never frees (ADR 0005).
  - Voices and buffers are allocated at construction.
  - Denormals are flushed per sample, as the SVF already does.
- **Oversampling latency.** Comprimo's 2× adds 31 base samples (≈0.65 ms at 48 kHz). That is acceptable for a live
  instrument and is reported to PDC. A **polyphase IIR half-band** option (Valenzuela–Constantinides) is the
  low-latency alternative, to decide in the ADR.
- **CPU targets** (to be measured by the benchmark in 9; these are targets, not claims):
  - init voice (1 osc, 1 unison, SVF, 2×): ≤ 0.05 % of a core at 48 kHz;
  - supersaw voice (1 osc × 7 unison, ladder, 2×): ≤ 0.3 %;
  - worst case (3 osc × 16 unison, 2 ladders, 4×): ≤ 3 % per voice.
  - With 16 voices of a typical pad (2 osc × 7 unison) the instrument should stay **under 10 % of one core**.

### 8.3 Modulation model

- **It follows family.md 7.6 exactly, plus the poly/mono split.**
  - *Poly sources* (per-voice envelopes and LFOs, velocity, note, per-note expression, seeded randoms) live inside
    the voice loop and can only target this instrument's parameters.
  - *Mono sources* (macros, global LFOs, host sources) are track-level objects in the host modulation service, and
    can target anything on the track or in the mixer. The UI marks each source "voice" or "track".
- **Seeded randomness.** Every random draw is a counter-based hash (PCG or SplitMix) of
  `(instance_seed, note_id, lane, purpose)`. `instance_seed` is stored in the project; `note_id` comes from the
  sequencer (v1 already carries tags). Live input uses a monotonic counter since the transport start. Free LFOs
  compute their phase from the transport sample position, so starting playback anywhere gives the same audio.
- **The target ring** (outside the knob) is always visible:
  - a band shows the range in the source's colour, **plus a letter/number tag** (L3, E2, M5), so colour is not the
    only cue;
  - a dot per sounding voice, **the newest bright and the others dim** (Phase Plant's idea);
  - several sources give a striped ring;
  - Alt-drag on the knob edits the depth of the selected source (Serum).
- **Source tiles.** A strip of tiles with **live mini-graphics** (Pigments), a count badge (Serum) and a drag
  handle. Dragging highlights every valid target (Vital), and releasing creates a slot with bipolar/unipolar chosen
  from the target (Serum's rule).
- **Matrix view.** A sortable, filterable table of all slots: source, curve thumbnail, depth in units, via, slew,
  bypass, target, and a live output graph. Rows keep their numbers when reordered.
- **Commands:** `AddModSlot`, `SetModField`, `RemoveModSlot`, `SetLfoShape`. They are all undoable and part of the
  undo fuzz walk.

### 8.4 Wavetable data model and file-format import

**In the project.** A table is a **recipe** (Vital's idea, done our way): an ordered list of *keyframes* at frame
positions. Each keyframe is either:
- inline frame data (2048 f32, content-hashed asset), or
- a generator: a harmonic list, a formula, or "from asset X, frame k".

After the keyframes comes a list of *processes* (normalise, remove DC, fade edges, filter, morph mode). Rendering
the recipe gives 256 frames, then the mips. Consequences:
- undo is a recipe diff;
- the project stores small recipes plus the imported source data once;
- export flattens the recipe.

**Import: WAV** (mono; if the file is stereo, use the left channel and offer the right one for oscillator B, as
Serum does):

1. **`clm ` chunk (Serum).** ASCII like `<!>2048 01000000 wavetable (www.xferrecords.com)`.
   - After `<!>` comes the **samples per frame**.
   - Then a flag string. **Digit 1 is the interpolation mode**: 0 none, 1 linear crossfade, 2–4 spectral variants.
     **Digit 2 is "factory"**: 1 means "don't embed in presets"; third parties should write 0.
   - Other digits are reserved; developers may ask Xfer for values. (Steve Duda,
     [KVR, 26 Dec 2018](https://www.kvraudio.com/forum/viewtopic.php?p=7266103).)
   - Surge writes `<!>N 10000000 wavetable Surge-XT` when exporting for Serum.
   - **Map digit 1 to `pos_mode`/the morph process.** Ignore digit 2.
2. **`uhWT` chunk (u-he Hive):** 2048 per frame (Surge treats it like `clm `).
3. **`srge` / `srgo` chunk (Surge):** two little-endian i32: version and frame size. `srgo` means "one-shot sample,
   not a table".
4. **`smpl` chunk:** frame size = the length of the first loop (end − start + 1). A `smpl` with zero loops is taken
   as 2048 (Surge's handling of RAPID files).
5. **`cue ` chunk (NI style):** frame size = the spacing between the cue points.
6. **No chunk:** if the length is a multiple of 2048 and ≤ 256 × 2048, take 2048 per frame (Pigments' documented
   rule, and Serum's default). Otherwise open the **import dialog**: fixed size (typed samples or a note name), pitch
   average, pitch follow, FFT 256/512/1024/2048, or "use as sample".
7. **Bit depths:** 16-, 24- and 32-bit PCM and 32-bit float. Resample non-2048 frame sizes to 2048 per frame with a
   band-limited FFT resize (so a 256-sample WaveEdit frame gains no aliasing).
8. **Text overrides (Serum):** `sound.txt` or `FolderInfo.txt` beside the file holds `[N]` (32–9999 samples per
   cycle) and `[interp]`/`[no interp]`. Cheap to support for Serum-pack compatibility.

**Import: Surge `.wt`** (and Bitwig's tables, which use the same container), from Surge's published format note:

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `vawt` (the only big-endian part; the rest is little-endian) |
| 4 | 4 | `wave_size`: samples per frame, a power of two, 2 … 4096 |
| 8 | 2 | `wave_count`: frames, 1 … 512 |
| 10 | 2 | flags: `0x01` is a sample, not a table · `0x02` looped sample · `0x04` int16 data (else float32) · `0x08` int16 uses full range (else a peak of 2¹⁴, i.e. −6 dBFS) · `0x10` a null-terminated XML `<wtmeta>` block follows the data |
| 12 | 4·size·count or 2·size·count | the frames, back to back |

Frames beyond 256 are decimated with a warning, or kept (the internal limit is a constant; 512 costs 4 MiB per
table).

**Import: Vital `.vitaltable`** (P1). JSON, as observed from the file structure (implement from the schema, and
copy no GPL code):
- top level: `name`, `author`, `version`, `remove_all_dc`, `full_normalize`, and `groups[]`;
- each group has `components[]`, each with a `type`:
  - sources: `"Wave Source"`, `"Line Source"`, `"Audio File Source"`, `"Shepard Tone Source"`;
  - modifiers: `"Phase Shift"`, `"Wave Window"`, `"Frequency Filter"`, `"Slew Limiter"`, `"Wave Folder"`,
    `"Wave Warp"`;
- sources have `keyframes[]` with a `position` on the 0–256 scale;
- a Wave Source keyframe carries `wave_data`, which is **base64 of 2048 little-endian f32**;
- there are also `interpolation_style`;
- an Audio File Source embeds `audio_file` (base64), `audio_sample_rate`, `window_size`, `fade_style` and
  `phase_style`.

P1 imports Wave Source keyframes with their interpolation and warns about unsupported modifiers. P2 implements the
modifiers. Vital presets (`.vital`, also JSON) embed the same table structure.

**Import: folders of single cycles** (AKWF and similar). Sort the files naturally; each file becomes a frame.
Dropping several files at once builds one table (Serum's behaviour).

**Export.** WAV, mono, **32-bit float, 2048 per frame**, with a `clm ` chunk `<!>2048 10000000 wavetable Cantus`
(digit 1 = the table's morph mode), plus an optional private `cnts` chunk holding the recipe as JSON, which others
ignore. Also `.wt` in float32. **Round trip:** export then import is bit-identical (tested).

**Licensing note (for the ADR).** File formats are interoperability data, and we describe them from documents
(Surge's format note, the KVR post) and from observed structure. The implementation must be clean-room in
`cantus-dsp` or `cantus-project`. The factory tables of Serum, Vital and the rest are **not** redistributable; the
user imports their own.

### 8.5 Migration from Firmus v1

"Convert to Firmus II" builds this patch (the v1 project stays untouched until the user runs it):
- `osc.a` = a table holding the v1 waveform (Sine/Tri/Saw/Square as single-frame tables), and `osc.b` likewise;
- `osc.b.semi` / `fine` = `osc2_semitones` / `osc2_detune_cents`;
- the oscillator levels come from `osc_mix` (equal-power is *not* v1's law: v1 is linear 1−m / m, so convert
  literally);
- `flt.1` = SVF LP12 on, with `cutoff`, `res`, and `keytrack` = v1 `key_track` (both are octaves per octave around
  C4);
- `env.1` = the amp ADSR; `env.2` = the filter ADSR;
- `mod.1` = ENV2 → `flt.1.cutoff` with depth `filter_env_octaves` (in oct);
- `gain` = `gain_db`; `phase_rand` = 0 (v1 resets the phase to 0).

The conversion is not bit-identical, because the oscillator changes from PolyBLEP to wavetable. It is
**perceptually matched** and says so. Tested: the spectra match within 1 dB below 10 kHz for C2–C6.

### 8.6 Editor layout (family frame: header · body · footer)

```
┌ Header ────────────────────────────────────────────────────────────────────────────────────────┐
│ ▣ Track 3 "Pad"  TABULA · ↶ ↷ · A|B → · ‹ Glass Choir ●mod › · Δ · ⏻ · ? · ⋯ · ⛶                │
├ Tabs: [Synth] [Table] [Matrix] [Voice]           voices 12/16 · CPU 3.1 % · Q 2× · ⚠ alias W1  │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│ OSC A ⏻ Wavetable▾   │ OSC B ⏻            │ OSC C ⏻            │ SUB ⏻ │ FILTER 1 ⏻ Ladder24▾     │
│ ‹ Basic Shapes ›  ✎  │ ‹ …            ›   │ ‹ …            ›   │ sine  │ ┌ response + FFT ─────┐ │
│ ┌ 3D / 2D table ──┐  │ ┌──────────────┐   │ ┌──────────────┐   │ −1 oct│ │ drag = cut × res    │ │
│ │ current frame ▬ │  │ │              │   │ │              │   │ ◯ lvl │ └─────────────────────┘ │
│ └─────────────────┘  │ └──────────────┘   │ └──────────────┘   │ NOISE │ ◯cut ◯res ◯drv ◯key ◯mix │
│ oct 0 semi 0 fine 0  │ …                  │ …                  │ ⏻ wht ├─ serial ━━━●━━ parallel ─┤
│ ◯POS ◯LVL ◯PAN →F1▾  │                    │                    │ ◯ lvl │ FILTER 2 ⏻ SVF LP12▾     │
│ UNI [7]▴▾ Super▾ ◯DET ◯BLD ◯WID  stack Off▾ │                    │       │ ◯cut ◯res ◯drv ◯key ◯mix │
│ W1 [Bend ▾] ◯   W2 [FM←B ▾] ◯   phase 0° rand 100% │           │       │                          │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Sources: [E1 ②][E2 ①][E3][E4] [L1 ③][L2][L3]…[L8] [M1…M8] [Vel][Note][MW][AT][MPE][Rnd] ◀ drag │
│ ┌ selected source editor (envelope with curves / drawn LFO with grid) ──┐ ┌ Macros ─────────────┐ │
│ │                                                                       │ │ ◯M1 ◯M2 ◯M3 ◯M4    │ │
│ └ rate 1/4 · Retrig · poly · rise 0 · delay 0 · smooth 0 ───────────────┘ │ ◯M5 ◯M6 ◯M7 ◯M8    │ │
├ Footer ─────────────────────────────────────────────────────────────────────────────────────────┤
│ Learn · Poly 16 · Legato off · Glide 0 ms · Bend ±2 · Quality 2× · Seed #3F2A · Latency 0.65 ms │
│ In —  Out ◯ −6 dB   · help strip: "Warp 1 · Bend: pinches the cycle toward its centre · Alt-drag…" │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- The **Table** tab is the full editor, with the same frame:
  - the 3D stack of frames with the current frame highlighted (real frames solid, interpolated frames dim);
  - the 2D cycle with draw tools and a grid;
  - an **FFT bar editor** (magnitude above, phase below; Serum's layout);
  - a frame strip (drag to reorder, Shift for a range);
  - the recipe list (keyframes and processes, each with bypass, like a device chain);
  - an import button and an "Import from project…" (resample) button.
- **Compact density** (docked in the inspector): only the header, the four oscillator/filter summaries (a table
  thumbnail plus the three most-used knobs each), the macros and the source strip. Double-click opens the window.
- **Every value is visible at rest** (family.md 7.1-3): the unison count, warp names and quality are chips, not
  right-click options. Serum's hidden options (phase legato, smooth interpolation, clean drive, ×10 LFO rate) become
  visible toggles in the relevant panel.

---

## 9. Test plan hints

Each of these is a test in `cantus-dsp` (unit), `cantus-engine/tests` (null and render tests) or `cantus-core`
(undo fuzz). Numbers are proposed pass criteria.

1. **Aliasing floor.** Render a full saw table (1024 harmonics) for 1 s at 48 kHz at **C6 (1046.5 Hz), C7
   (2093 Hz), C8 (4186 Hz) and 10 kHz**, at 1×, 2× and 4×. Take a long Blackman-Harris FFT. Sum the energy in the
   bins that are not harmonics of f0, below 20 kHz.
   - Criteria: **≤ −96 dB relative to the fundamental at 2×**, ≤ −80 dB at 1×.
   - Add a **low-note case: A1 (55 Hz) and E1 (41 Hz), ≤ −90 dB at 2×**. It exercises the least-oversampled mip
     levels, and is the case that decides the interpolator.
   - Report Firmus v1's PolyBLEP as the baseline (it will be tens of dB worse at C8).
2. **Sweep test.** An exponential pitch sweep of 20 Hz → 18 kHz over 10 s. The spectrogram is kept as a test
   artefact, and the maximum inharmonic component is asserted per quality level.
3. **Full bandwidth.** At a note just below a mip switch, harmonics under 20 kHz are within **0.5 dB** of the
   ideal at 2×. With draft 1× and half-octave mips: ≥ 17 kHz.
4. **Mip and frame continuity.** Glide the pitch across every mip boundary, and sweep `pos` 0 → 1 over 1 s. The
   high-passed residual against a reference (an ideal additive render) shows no step above **−90 dB**.
5. **Warp aliasing.** For each warp at amount 1, the alias floor at 2× and 4× is recorded, with sync, FM and PWM
   asserted (≤ −70 dB at 4× for C6). This also calibrates the P1 aliasing indicator.
6. **Unison.**
   - The RMS with random phases stays within **±1 dB for N = 1…16** at blend 0.5.
   - The L+R sum at width 1 loses ≤ 3 dB against the stereo power (mono compatibility).
   - At `uni.voices` = 1, every `uni.*` parameter is inert (bit-identical).
7. **Determinism.** Two renders of the same project are bit-identical, including random phases, S&H, chaos LFOs
   and drift. **A bounce starting at bar 33 equals bars 33+ of the full bounce** (the seed-per-note-id rule).
8. **Block-size independence.** Render with block sizes 1, 7, 64, 256, 1000 and 4096: bit-identical (the
   16-sample grid anchored to the stream).
9. **A control at its default changes nothing.**
   - Every parameter set explicitly to its default equals the init render, bit for bit.
   - Warp amount 0, a modulation slot at depth 0 and a bypassed slot are each bit-identical to their absence.
   - A filter that is off is a wire. Enabling filter `mix` = 0 is a wire.
10. **Sample-rate consistency.** At 44.1/48/96 kHz the magnitude spectra agree within 0.5 dB below 15 kHz, and
    envelope times agree within 1 sample.
11. **CPU per voice.** A bench harness reports ns per sample per voice for init, supersaw-7, 3×16 at 4×, and a
    16-voice pad. It fails on regressions of more than 10 % against a stored baseline, and checks the 8.2 targets.
12. **Real time.** No allocation in `process` (checked with a counting allocator in tests). Loading a table on the
    worker thread while notes play gives no glitch and no free on the audio thread.
13. **Import and export.**
    - Export → import round-trips bit-identically.
    - Hand-built fixtures (made in the test; nothing copied from vendors): `clm ` 2048 with flag digits 0/1/2;
      `uhWT`; `srge` 1024; `smpl` loop 512; `cue ` spacing; a plain 16×2048 file; a 256-sample frame file
      (resampled without aliasing); `.wt` float32, int16 at 15-bit and full range, with a metadata flag; a
      `.vitaltable` with two base64 keyframes.
    - Malformed files (short chunks, zero frames, over 512 frames) fail with a readable, localised error and never
      panic.
14. **Robustness.** Resonance 1 with drive 36 dB on every filter for 60 s gives no NaN or Inf, and denormals are
    flushed. Long releases decay to exact zero, and `active_voices` returns to 0 (v1's test, extended).
15. **Undo and project.**
    - Firmus II parameters, mod-slot commands, `SetWavetable`, recipe edits and `SetLfoShape` join the **undo fuzz
      random walk**, and undo restores the sparse map and the asset references exactly.
    - Schema bump plus a fixture project; loading an old project leaves Firmus v1 renders bit-identical (a null
      test against a stored render).
16. **Migration.** "Convert to Firmus II" on the v1 default patch matches the spectra within 1 dB below 10 kHz for
    C2–C6.
17. **UI contract.**
    - Every token pair used by the editor passes the family contrast rules: knob edge ≥ 3:1, text ≥ 4.5:1.
    - Every parameter has EN and ES label keys and an AccessKit name and value.
    - Ring, count badge and chip state are visible without hover (snapshot tests of the widget state).

---

## 10. Sources

**Xfer Records (Serum 2)**
- Product page: https://xferrecords.com/products/serum-2
- Serum 2 User Guide (v2.0.18, manual 1.0.3, 27 Apr 2025, PDF): https://www.xferrecords.com/manual/serum-2/docs
- *What's New in Serum 2* (PDF, 17 Mar 2025): https://static.xferrecords.com/Serum%202%20What's%20New.pdf
- Web manual: https://xferrecords.com/web-manual/serum-2/welcome
- Support, converting samples to wavetables: https://support.xferrecords.com/article/59-converting-samples-to-wavetables
- Changelog compiled from Xfer's API (to 2.0.24): https://gist.github.com/0xdevalias/a537a59d1389d5aed3bc63b544c70c8d
- `clm ` chunk described by Steve Duda (26 Dec 2018): https://www.kvraudio.com/forum/viewtopic.php?p=7266103
- Linux beta: https://synthanatomy.com/2026/08/xfer-records-serum-2-super-popular-wavetable-synth-gets-massive-free-update.html ; https://www.gearnews.com/serum-2-xfer-records-synths/
- Reviews and threads: https://musictech.com/reviews/plug-ins/xfer-records-serum-2-review/ ; https://www.cfa-sound.com/xfer-serum-2-review-and-insights/ ; https://www.kvraudio.com/forum/viewtopic.php?t=619315 ; https://modeaudio.com/magazine/an-introduction-to-serum-2

**Arturia (Pigments 7)**
- Press release: https://www.arturia.com/company/press/press-releases/arturia-announces-pigments-7
- What's new in 7: https://support.arturia.com/hc/en-us/articles/23729034214940-Pigments-New-in-Pigments-7
- Manual 7.0.0 (PDF): https://dl.arturia.net/products/pigments/manual/pigments_Manual_7_0_0_EN.pdf
- Product page: https://www.arturia.com/products/software-instruments/pigments/overview
- Review: https://synthanatomy.com/2025/12/arturia-pigments-7-review-a-character-upgrade-for-the-flagship-synthesizer-plugin.html ; https://www.synthtopia.com/content/2025/12/16/arturia-pigments-7-a-free-upgrade-adds-new-effects-more/

**Kilohearts (Phase Plant)**
- Docs: https://kilohearts.com/docs/phase_plant ; https://kilohearts.com/docs/modulation ; https://kilohearts.com/docs/wavetables
- Product page: https://kilohearts.com/products/phase_plant
- Changelog: https://kilohearts.com/changelog
- v3 tease: https://www.kvraudio.com/forum/viewtopic.php?t=626488 ; Superbooth 2026 preview: https://www.studio.se/nyheter/nyhet/kilohearts-phase-plant-3-preview-p%C3%A5-superbooth-2026-r1149/
- Review: https://modwheel.net/reviews/kilohearts-phase-plant-review

**Native Instruments (Massive X)**
- Product page: https://www.native-instruments.com/en/products/komplete/synths/massive-x/
- 1.7 news: https://synthanatomy.com/2026/03/native-instruments-massive-x-out-now.html ; 1.6: https://www.kvraudio.com/news/native-instruments-updates-massive-x-to-v1-6-0-64450

**u-he (Hive 2)**
- Product page: https://u-he.com/products/hive/
- Release notes: https://u-he.com/products/hive/releasenotes.html

**Vital**
- Site: https://vital.audio/
- Source (GPLv3; read for structure, not copied): https://github.com/mtytel/vital. Files consulted:
  `src/common/synth_constants.h` (counts), `src/synthesis/producers/synth_oscillator.h/.cpp` (morph and
  distortion enums, spectral-buffer rendering), `src/synthesis/lookups/wavetable.h`, `wave_frame.h`,
  `src/common/wavetable/*` (the table JSON structure)
- Community user guide (David Vogel): https://davidmvogel.com/docs/Vital/UserGuide/Oscillators-and-Sampler ; …/Advanced ; …/Modulation ; …/Filters
- Review: https://pluginoise.com/vital-synth-review/

**Surge XT**
- Manual: https://surge-synthesizer.github.io/manual-xt/
- Changelogs: https://surge-synthesizer.github.io/changelog/ ; https://surge-synthesizer.github.io/old-changelogs/
- Releases: https://github.com/surge-synthesizer/releases-xt/releases ; 1.4 milestone: https://github.com/surge-synthesizer/surge/milestone/36
- `.wt` format: https://github.com/surge-synthesizer/surge/blob/main/resources/data/wavetables/WT%20fileformat.txt
- Wavetables doc: https://github.com/surge-synthesizer/surge/blob/main/doc/Wavetables.md
- Source read for behaviour (GPLv3; not copied): `src/common/dsp/Wavetable.cpp` (mip building),
  `src/common/WAVFileSupport.cpp` (chunk detection), `src/common/dsp/oscillators/WavetableOscillator.cpp` (mip
  selection and sinc step convolution)

**DSP literature** (cited in section 4)
- Bristow-Johnson, "Wavetable Synthesis 101: A Fundamental Perspective", AES 101st Conv., 1996.
- Massie, "Wavetable sampling synthesis", in Kahrs & Brandenburg (eds.), *Applications of DSP to Audio and Acoustics*, Kluwer, 1998.
- Horner, Beauchamp & Haken, "Methods for multiple wavetable synthesis of musical instrument tones", *JAES* 41(5), 1993.
- Moore, "Table lookup noise for sinusoidal digital oscillators", *CMJ* 1(2), 1977.
- Stilson & Smith, "Alias-free digital synthesis of classic analog waveforms", ICMC 1996.
- Brandt, "Hard sync without aliasing", ICMC 2001.
- Välimäki & Huovilainen, "Antialiasing oscillators in subtractive synthesis", *IEEE SPM* 24(2), 2007.
- Välimäki, Nam, Smith & Abel, "Alias-suppressed oscillators based on differentiated polynomial waveforms", *IEEE TASLP* 18(4), 2010.
- Välimäki, Pekonen & Nam, "Perceptually informed synthesis of bandlimited classical waveforms using integrated polynomial interpolation", *JASA* 131(1), 2012.
- Franck & Välimäki, "Higher-order integrated wavetable and sampling synthesis", *JAES* 61(9), 2013.
- Laakso, Välimäki, Karjalainen & Laine, "Splitting the unit delay", *IEEE SPM* 13(1), 1996.
- Niemitalo, *Polynomial Interpolators for High-Quality Resampling of Oversampled Audio*, 2001 (technical report; the interpolator/oversampling trade-off tables).
- Valenzuela & Constantinides, "Digital signal processing schemes for efficient interpolation and decimation", *IEE Proc. G* 130(6), 1983.
- Lazzarini & Timoney, "New perspectives on distortion synthesis for virtual analog oscillators", *CMJ* 34(1), 2010.
- Kleimola, Lazzarini, Timoney & Välimäki, "Vector phaseshaping synthesis", DAFx 2011.
- Chowning, "The synthesis of complex audio spectra by means of frequency modulation", *JAES* 21(7), 1973.
- Parker, Zavalishin & Le Bivic, "Reducing the aliasing of nonlinear waveshaping using continuous-time convolution", DAFx 2016.
- Bilbao, Esqueda, Parker & Välimäki, "Antiderivative antialiasing for memoryless nonlinearities", *IEEE SPL* 24(7), 2017.
- Esqueda, Pöntynen, Parker & Bilbao, "Virtual analog models of the Lockhart and Serge wavefolders", *Applied Sciences* 7(12), 2017.
- Zavalishin, *The Art of VA Filter Design*, rev. 2.1.2, Native Instruments, 2020.
- Simper, "Linear trapezoidal integrated SVF", Cytomic technical paper, 2013.
- Huovilainen, "Non-linear digital implementation of the Moog ladder filter", DAFx 2004.
- D'Angelo & Välimäki, "An improved virtual analog model of the Moog ladder filter", ICASSP 2013; "Generalized Moog ladder filter, parts I and II", *IEEE/ACM TASLP* 22(12), 2014.
- Szabó, *How to Emulate the Super Saw*, KTH thesis, 2010.
- Dolson, "The phase vocoder: a tutorial", *CMJ* 10(4), 1986; Laroche & Dolson, *IEEE TSAP* 7(3), 1999; Röbel, DAFx 2003; Serra & Smith, *CMJ* 14(4), 1990.
- Roads, *Microsound*, MIT Press, 2001; Truax, *CMJ* 12(2), 1988.
- Lorenz, "Deterministic nonperiodic flow", *J. Atmos. Sci.* 20, 1963; Rössler, "An equation for continuous chaos", *Phys. Lett. A* 57, 1976.
- Peterson & Barney, "Control methods used in a study of the vowels", *JASA* 24(2), 1952.
