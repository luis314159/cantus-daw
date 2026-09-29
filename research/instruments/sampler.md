# Samplers and sample-instrument formats: competitive research for the Cantus sampler

*Researched 2026-09-24 for Cantus. Sources: the vendors' manuals (Kontakt 7.6 PDF and the Kontakt 8 online
manual, the Falcon 2026 PDF manual, the Ableton Live 12 manual and release notes, the Logic Pro 12.3 user guide,
the FL Studio online manual), the format specifications (sfzformat.com's opcode database, the SoundFont 2.04
specification, the Decent Sampler developer guide), open-source code read for design facts only (sfizz, BSD-2;
Signalsmith Stretch, MIT; ConvertWithMoss documentation, LGPL-3), the DSP literature, press reviews and user
forums. Everything is paraphrased; no code was copied. The session's web-search quota ran out part-way, so some
facts come from a single search excerpt; the text says so wherever that is the case, and says "from memory" where
a fact could not be re-checked today. Screenshots are the manufacturers' own and live in the git-ignored
[`screenshots/`](screenshots/) folder, for internal reference only.*

*House rules this report builds on: [`../fabfilter/family.md`](../fabfilter/family.md) §7 (device frame, knob and
display rules, host-owned undo and modulation) and the report shape of [`../fabfilter/pro-q.md`](../fabfilter/pro-q.md).
Cantus today: a Pulsus pad can play one WAV (`crates/cantus-core/src/pads.rs`, `crates/cantus-engine/src/samples.rs`),
decoded whole into `f32`, cached for the session and read with **linear interpolation**
(`crates/cantus-dsp/src/pulsus.rs`, `SampleRef::at`).*

---

## 0. The short version

1. **Kontakt wins on ecosystem, not on engine.** Its moat is the library catalogue and the paid Kontakt Player
   licence that commercial developers buy into. The engine underneath (groups, zones, group start conditions, voice
   groups, eight loops per zone, the Time Machine family, KSP scripting) has changed little in a decade, and
   reviewers of Kontakt 8 say so. Its default interpolation ("Standard") is, according to a Kontakt script developer
   on KVR, linear; better quality is opt-in per group and its cost grows with the transposition. In 2026 the vendor
   went through preliminary insolvency (27 Jan) and was sold to inMusic (agreement announced 8 May). **Cantus
   cannot load encrypted Kontakt libraries and must never try.** It can offer everything else.
2. **The open formats are enough to bring most free and indie libraries over.** SFZ (text, documented opcode by
   opcode at sfzformat.com, 433 opcodes across v1, v2 and ARIA, about 60 of which matter for real libraries),
   Decent Sampler's `.dspreset` (XML, documented, zipped as `.dslibrary`), SoundFont 2 (binary RIFF, a 2006 spec)
   and plain WAV/AIFF with their `smpl`/`inst` chunks. The open CC0 libraries (VSCO 2 CE, VCSL, the Karoryfer
   catalogue) ship as SFZ. **Import order: WAV folders, SFZ, Decent Sampler, SF2/SF3; export SFZ.**
3. **Nobody makes aliasing-free transposition the default.** Kontakt defaults to Standard; sfizz and sforzando default
   to Hermite; Ableton's Sampler defaults to Normal with Good/Best as "significant CPU"; FL DirectWave offers a 16-point
   sinc that its own manual says "may produce more aliasing". Polynomial interpolators on non-oversampled audio are
   poor: Niemitalo measured 19 dB for linear and 23.5 dB for 4-point Hermite even on 2x-oversampled input. A
   windowed sinc with a fixed kernel (sfizz's 8 to 72 points) still aliases when pitching *up*, because the kernel's
   cutoff stays at the source Nyquist. **De Soras's octave mip-map + oversampled interpolator + half-band decimator
   gives a bounded cost at any ratio; Cantus should ship that as the default.**
4. **The best ideas are scattered across products.** Kontakt's loop-edit seam view; TX16Wx's loop overlay and its
   grid-plus-table mapping with fade ranges drawn inside zones; Falcon's `OFF+E`/`OFF+V` release triggers that start
   at the level the note had reached; Ableton's round robin with a *reset every bar*; Logic's "convert regions to
   sampler track"; Live's similar-sample swapping; Life's colour-coded slices clustered by sound. **None of them has
   all of these, and none of them owns the arrangement.**
5. **The host-native advantages are real and large.** Logic and Ableton convert audio to a sampler instrument once,
   one way. Cantus can keep the link: a zone that *is* a clip or a take, slices that *are* a pattern in the channel
   rack, one undo history, render identical to playback (including round-robin and random choices), and the AI
   bridge driving the same commands.
6. **Time-stretch licensing decides the architecture.** Cantus's engine crates are MIT OR Apache-2.0 (ADR 0001).
   Rubber Band is GPL-2.0 or commercial and élastique is commercial, so neither can go into `cantus-dsp` for free.
   **Signalsmith Stretch is MIT** (STFT-based, polyphonic, formant control, 120 ms default block; Rust wrappers
   exist). Bungee is MPL-2.0. Use Signalsmith for tonal stretch, slicing for beats, and write our own WSOLA for
   cheap monophonic cases.
7. **Content: bundle only CC0 unless the licence ADR changes.** ADR 0001 says bundled samples must be original or
   CC0. VSCO 2 CE (3 GB), VCSL (5 GB), the Karoryfer/Versilian SFZ libraries and Osiris Piano are CC0. Salamander
   Grand Piano v3 (the obvious free piano) is **CC-BY 3.0**, so it needs an attribution file and an ADR amendment.
   Sonatina Symphonic Orchestra (Sampling Plus 1.0, unclear provenance) and 99Sounds (redistribution forbidden)
   must not be bundled.
8. **Proposed name: Exemplar** (Latin: a copy, a model to copy from; Spanish *ejemplar*). The slicing mode can be
   called **Sectio** (a cutting). §9 has the P0/P1/P2 plan, the data model, the voice and streaming architecture and
   the editor layout.

---

## 1. Products and versions

| Product | Latest version and date | Price (Sept 2026) | Formats / platforms | Source |
|---|---|---|---|---|
| **NI Kontakt 8** | **8.13.0, 3 Sep 2026** (8.10.0 on 29 Apr 2026 added an Aftertouch tool; 8.9.0 on 25 Feb 2026 replaced instrument pages with a scrolling list). Kontakt 8 launched autumn 2024 (reviews Dec 2024–Feb 2025). *Version history from a search excerpt of the VI-Control update thread; the thread itself returned 403.* | USD 299 full; upgrades from GBP 89 (SOS). Kontakt Player free. | VST2/VST3/AU/AAX, standalone; Win/macOS | [NI product page](https://www.native-instruments.com/products/kontakt), [SOS review](https://www.soundonsound.com/reviews/native-instruments-kontakt-8), VI-Control thread (search excerpt) |
| **UVI Falcon** | **Falcon 2026** (KVR lists 26.0.7). Falcon 3 was 19 Oct 2023; 3.1 added a tag browser with audio previews. | USD/EUR 299 (was 349), with a 100 voucher; also in SonicPass (USD 24/month) | VST/VST3/AU/AAX, standalone; up to 10.2 channels | [uvi.net/falcon](https://www.uvi.net/falcon), [manual](https://cdn.uvi.net/UVIFC_Falcon/manuals/Falcon_manual_en.pdf), [KVR](https://www.kvraudio.com/product/falcon-by-uvi) |
| **Steinberg HALion 7** | **7.5, Aug 2026** (free update: MPE, Spectral Snapshots, real-time Decompose, auto-gain compressor, better MediaBay). 7.0 was spring 2023; 7.1 reworked the spectral engine. | EUR 349 (60 % off to 24 Aug 2026) | VST3/AU/AAX; Win/macOS | [Synth Anatomy](https://synthanatomy.com/2026/08/steinberg-halion-7-flagship-sampler-plugin-gets-an-fm-and-spectral-makeover.html), [SOS review](https://www.soundonsound.com/reviews/steinberg-halion-7) |
| **Ableton Sampler / Simpler / Drum Sampler** | Live **12.4.6** is the newest release note. Live 12 added native round robin to Sampler; Drum Sampler is new in Live 12. | Sampler in Suite; Simpler and Drum Sampler in all editions | Inside Live | [Live 12 manual](https://www.ableton.com/en/manual/live-instrument-reference/), [release notes](https://www.ableton.com/en/release-notes/live-12/) |
| **Logic Sampler / Quick Sampler / Auto Sampler** | Logic Pro for Mac **12.3** is the newest guide edition | In Logic Pro | Inside Logic (AU) | [Logic Pro guide](https://support.apple.com/guide/logicpro/sampler-mapping-and-zone-pane-overview-lgsifc8617c2/mac) |
| **FL Studio DirectWave, Slicex, FLEX** | Current online manual (FL Studio 2025/2026) | In FL Studio (DirectWave full in Producer and up, from memory) | Inside FL; DirectWave programs also play in FL Mobile | [DirectWave manual](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/DirectWave.htm) |
| **Decent Sampler** | **1.33.0, 22 Sep 2026.** A burst of releases since August: 1.25 sample-editing tools (loop tuner, auto loop detection), 1.27 typed values, 1.30 oscillators, FM and DX7 patch import in the preset editor. The last version for Windows 7 (next moves to JUCE 8). | Free (closed source); a library store | VST/VST3/AU/AAX, AUv3 on iOS; Win/macOS/Linux | [version list](https://store.decentsamples.com/downloads/decent-sampler/versions), [developer guide](https://decentsampler-developers-guide.readthedocs.io/en/latest/) |
| **TX16Wx** | **3.8.0** | Free; Professional EUR 89 (EUR 29 reduced) | VST/VST3/AU/AAX/**CLAP**; Win/macOS/Linux | [KVR](https://www.kvraudio.com/product/tx16wx-software-sampler-by-cwitec), [tx16wx.com](https://www.tx16wx.com/) |
| **Plogue sforzando** | Version not verified today | Free | VST3/AU/**CLAP**/AAX, standalone | [plogue.com](https://www.plogue.com/products/sforzando.html) |
| **sfizz** (open SFZ engine) | **1.2.3, 14 Jan 2024**; last push Mar 2025. A 2026 fork, *sfizioso*, keeps the libraries BSD-2 and puts its app under AGPL-3. | Free, **BSD-2-Clause** | LV2/VST3/AU, library | [GitHub](https://github.com/sfztools/sfizz), [sfizioso](https://github.com/rullopat/sfizioso-player) |
| **Serato Sample** | Not verified | One-off purchase or Producer Suite subscription | VST/AU/AAX | [serato.com/sample](https://serato.com/sample) |
| **XLN Life** | **1.2** (adds Duck sidechain) | EUR 149, or EUR 9.99/month rent-to-own | VST/VST3/AU/AAX, standalone, Field Recorder app | [xlnaudio.com/products/life](https://www.xlnaudio.com/products/life) |
| **ConvertWithMoss** (converter, reference only) | **20.3.0, 19 Sep 2026** | Free, **LGPL-3.0** (Java) | Standalone | [GitHub](https://github.com/git-moss/ConvertWithMoss) |

**The vendor behind the standard is in flux.** Native Instruments GmbH entered preliminary insolvency on
27 Jan 2026 and inMusic (Akai, Moog, Denon DJ) signed a definitive agreement to buy it on 8 May 2026; the CEO's
statements promised business as usual and continued Kontakt Player licensing
([gearnews](https://www.gearnews.com/native-instruments-insolvency/), [MusicTech](https://musictech.com/news/industry/native-instruments-acquired-by-inmusic-akai-pro-moog-denon/)).
A widely shared "Kontakt AI replaces Kontakt" leak was gearnews's April Fools' piece, not news.

---

## 2. Screenshots saved

All in [`screenshots/`](screenshots/) (git-ignored). Each was downloaded with `curl -L`, checked for a real
PNG/JPEG signature and viewed. WebP originals were converted to PNG locally.

| File | What it shows |
|---|---|
| `smp-01-kontakt-mapping-editor.png` | Kontakt's Mapping Editor (NI manual): a key × velocity grid with a two-row control strip of small icon buttons (Auto, Root, Solo Zone, Selected Groups only, AutoSel. Grp), a status line (K.Range, Velocity, Root, Volume, Pan, Tune) and a keyboard. Three zones, each split in two velocity layers, are translucent teal boxes; sample names are written **rotated 90°**, one letter per line. |
| `smp-02-kontakt-mapping-crossfades.png` | Zones in yellow with crossfades drawn as gradients. Where zones overlap, their rotated names collide ("Boes…" printed twice over itself). |
| `smp-03-kontakt-wave-editor-loop-tab.png` | The Wave Editor: toolbar, a status row (Sample, S.Start, S.Mod, S.End, format 48000/24, length), the waveform with the loop in amber, and four tabs (Sample Loop, Sync/Slice, Zone Envelopes, Sample Editor). The Sample Loop tab has an **eight-slot 2×4 loop grid**, Loop Start/End, X-Fade, Tune, Count, a loop-mode picture and "until end". A Grid panel (Fix/Auto, BPM) sits on the right. |
| `smp-04-kontakt-loop-edit-view.png` | **The Loop Edit view**: the pane is split at the seam; left is the audio just before the loop end, right is the audio after the loop start, and the audio that *would* have followed the end is drawn in dim grey behind it. The best loop-point diagnostic in the survey. |
| `smp-05-kontakt-group-start-options.png` | Group Start Options as a rule list: "on ctrl. If CC# 1 is between 0 and 64 **and** rd. robin position 1 **and** always". A readable rule builder, three rows. |
| `smp-06-kontakt-edit-rack-all-editors.png` | Kontakt's Instrument Edit rack with every editor open: Group Editor, Mapping Editor, Wave Editor, Source (DFD, HQI standard, Release Trigger), group insert FX, Amplifier, buses, insert and send FX, modulation. A single column about 2,400 px tall: everything is reachable, nothing is visible at once. |
| `smp-07-kontakt-sync-slice.png` | A drum loop sliced in the Wave Editor, some slices selected (blue). |
| `smp-08-falcon-2026-overview.png` | Falcon 2026 marketing collage: dark module panels (Harmonic Resonators, Euclidean Drum, Slice Remixer) and colourful macro front panels for the factory presets. |
| `smp-09-falcon-2026-edit-view.png` | Falcon's Edit view: **one collapsible column per level** (Program → Layer → Keygroup → Oscillator), each level with its own FX and EVENT chains inline; the oscillator panel (Wavetable with FM, unison); a **mapping strip docked at the bottom** (Root, Key, Vel, Vol, Pan, Fine Tune) with two keygroups as coloured blocks; the module browser on the right (Sampling: Grains, IRCAM Granular, IRCAM Multi Granular, IRCAM Scrub, IRCAM Stretch, Sample, Slice, Stretch). |
| `smp-10-falcon-slice-oscillator.png` | The Slice oscillator: waveform with slice lines; Playback (Start, Speed, Auto Play, Sync tempo), Detection (Mode "User markers", Sensitivity), Extension (Auto, Amount, Time), an AHS envelope and a drag-to-Export button. |
| `smp-11-ableton-sampler-key-zone-editor.png` | Live 12 Sampler's Zone tab: **a list of sample layers on the left, each with a horizontal range bar** on a keyboard ruler (C-2…C8); top-left toggles Pow/Lin fade mode, RR, "Forward", Reset "None", and Key/Vel/Sel to switch which axis the bars show. Light grey with green bars; very readable. |
| `smp-12-ableton-simpler-slice-mode.png` | Simpler in Slice mode: a large amber waveform with cyan slice lines; Gain, Trigger/Gate, Slice By "Region", Regions 35, Playback Mono, Preserve Transients, Warp "Beats", "as 4 Bars", :2 / *2. Large, high-contrast type. |
| `smp-13-ableton-sampler-round-robin.png` | Sampler's round robin: four cello layers spanning the keyboard, RR on, mode Forward, **Reset "1 Bar"**, fade mode Lin. |
| `smp-14-xln-life-edit.png` | Life's Edit page: the source recording as a strip of **colour-coded slices** (same colour = same sound cluster, dark = unused), four sound rows (Volume, Pitch, Pan, Tone, Cut, Rev, Del, Env) each with A/B 16-step patterns, and Density, Syncopation, Symmetry, Sound and Pattern Variation macros. |
| `smp-15-fl-directwave-main.png` | DirectWave: a **zone spreadsheet** (Zone, Name, Size, Rate, Mute, Root, Semi, Fine, K-Trk, Ticks, SY, SL, L-Key, H-Key, L-Vel, H-Vel, Lk, VC) over a key × velocity grid whose narrow zones carry rotated names; parameter tabs below (Main, Time, Trigger; Filter 1/2; Amp ADSR as faders; LFO; a four-slot mod matrix); a folder browser on the right; 16 program tabs on top. |
| `smp-16-fl-directwave-sample-tab.png` | DirectWave's sample view: the waveform over a **spectrogram background**, Start and Loop markers, a context menu (File, Format, Edit, Tools, View, Snap, Select, Zoom) and **knobs** for Start, Loop Start and Loop End. |
| `smp-17-decent-sampler-player.jpg` | Decent Sampler's player (vendor image): a library-defined front panel (photo background, Tone and Reverb knobs), the mapped key range tinted blue on the keyboard, Browse / preset / Volume / File. |
| `smp-18-tx16wx-keymapper.png` | TX16Wx's key mapper: a grid where **each zone's fade range is drawn as a gradient inside the zone**, plus a synchronized table (Lo K, Hi K, Lo V, Hi V, Lo FK, Hi FK, Lo FV, Hi FV, Root, Fine, Sample, **Mode RAM/DFD per zone**, Rev, Sus. Loop, Rel. Loop, Att, Pan). |
| `smp-19-tx16wx-loop-overlay.png` | TX16Wx's **loop overlay**: the audio after the loop end is drawn translucently over the loop start, so a mismatch at the seam is visible at a glance. |
| `smp-20-logic-sampler-mapping-zone.png` | Logic's Sampler: tabs Synth / Mod Matrix / Modulators / Mapping / Zone. Mapping pane: groups list left, zones as labelled blue columns over the keyboard (selected one yellow), group and zone parameter strips as "Label: value". Zone pane: blue waveform with the loop in yellow, start/end/fade handles, One Shot, Reverse, Follow Tempo, and rows for Sample Start/End/Length, Fade In/Out, Loop Start/End/Length, Mode, Crossfade. |

Not kept: HALion (the Steinberg help site redirected every deep link to its index) and Serato Sample (images only
behind a CDN listing). Both are described from reviews and vendor text.

---

## 3. Anatomy of each product

### 3.1 Kontakt 8 (the engine is Kontakt's classic engine)

- **Hierarchy.** Multi → Instrument → Groups → Zones. A zone is one sample with a key range, velocity range, root,
  and volume/pan/tune offsets (tune ±36 semitones). Everything that processes sound lives on the **group**: the
  Source module, group insert FX, amplifier, modulators. Groups are how articulations, velocity layers,
  round-robins and mic positions are organised ([manual ch. 19–20](https://docs.native-instruments.com/ni-tech-manuals/kontakt-manual/en/classic-view)).
- **Source module modes, per group** (7.6 manual ch. 23): **Sampler** (RAM), **DFD** (direct from disk: sample
  heads preloaded, the rest streamed), **Wavetable** (every zone read as 2048-sample cycles, four quality levels),
  **Tone Machine** (granular, pitch and formant independent of speed), **Time Machine**, **Time Machine II**
  (transient-preserving, for drums), **Time Machine Pro** (highest quality, CPU- and RAM-heavy, stereo only, its own
  voice limits; widely reported to be zplane élastique, from memory), **Beat Machine** (plays slices in time with the
  host), and two "vintage" machines (**S1200** at about 12-bit/26 kHz, **MP60**).
- **Interpolation (HQI):** Standard, High, Perfect. Standard is "the algorithm of old KONTAKT versions", with a
  constant cost; High and Perfect "eliminate virtually all audible artifacts (especially when transposing upwards)"
  and **cost more the further the sample is transposed**. A global *Offline Interpolation Quality* can force Perfect
  for bounces. A Kontakt developer on KVR says most libraries leave it at Standard, which he calls linear
  (forum, Dec 2020).
- **Round robin, keyswitches, pedal layers** are *Group Start Options*: a rule list of conditions (always, start on
  key, start on controller, cycle round robin, cycle random, slice trigger) joined with and/or
  ([smp-05](screenshots/smp-05-kontakt-group-start-options.png)).
- **Voice groups:** 128 per instrument, each with a voice limit, a stealing mode (kill any, oldest, newest,
  highest, lowest), *prefer released*, a fade time in ms, and one of 16 **exclusive groups** (choke).
- **Loops:** up to **eight loop regions per zone**, each with start, end, crossfade in ms, a tune applied after the
  first pass, a repeat count (0 = forever) and a mode: until end, until end ping-pong, until release, until release
  ping-pong, one shot. Snap-to-zero-crossing for loops and for sample start/end. The **Loop Edit view**
  ([smp-04](screenshots/smp-04-kontakt-loop-edit-view.png)) shows the seam.
- **Release triggers** on the Source module, with a counter that measures how long the note was held and becomes a
  modulation source (to lower release-sample volume after long notes), and a "release trigger only once" option.
- **Mapping:** drag samples onto the grid; **auto-mapping from filename tokens** (split the name into tokens and say
  which is root, low key, high key, velocity); batch key and velocity crossfades; root-key helpers
  ([smp-01](screenshots/smp-01-kontakt-mapping-editor.png), [smp-02](screenshots/smp-02-kontakt-mapping-crossfades.png)).
- **Slicing:** the Wave Editor's Grid (Fix or Auto by transient) feeds Beat Machine, Time Machine, or auto-mapping
  slices to keys with a "drag MIDI to host" button ([smp-07](screenshots/smp-07-kontakt-sync-slice.png)).
- **Storage:** NCW, NI's lossless codec (the manual quotes 30–50 % smaller; reverse-engineered as DPCM with bit
  truncation in 512-sample blocks, optional mid/side), and NKX monoliths. Encrypted NKI/NKX for Player libraries.
- **Scripting:** KSP (Kontakt Script Processor) on every instrument; Kontakt 8 adds the Komplete UI framework
  ("Komplete Script") for high-DPI library front ends.
- **Kontakt 8 additions:** the Tools framework (Chords, Phrases, Patterns, Chord Builder, Velocity Curve, Humanizer,
  Scale Lock, Arpeggiator, Aftertouch), **Leap** (a 16-slot loop and one-shot player), Conflux (sample + PPG
  wavetable hybrid), Piano Uno, a redesigned default view with Classic view kept, and 900+ factory instruments.
- **Foreign formats were dropped.** The 5.7 manual documented importing EXS24, HALion ≤ 2, GigaStudio, Akai, Kurzweil,
  Reason NN-XT, VSampler and more; the 7.6 manual's file-format table lists only NI formats, Battery, REX, Apple
  Loops/ACID and WAV/AIFF. Kontakt no longer helps users leave *or arrive*.

**Why it is the standard.** Scripted instruments with custom front ends since Kontakt 2; a free Player that runs
licensed libraries, so the customer of a library need not own Kontakt; NI's NKS browser integration; and two decades
of third-party catalogue. The Player licence costs the developer a fee that indie developers often decline, so their
libraries only run in full Kontakt (search excerpt from a buyer's guide; the fee is not public).

### 3.2 UVI Falcon 2026

- **Hierarchy:** Multi → Part → Program → **Layer** → **Keygroup** → **Oscillator(s)**
  ([smp-09](screenshots/smp-09-falcon-2026-edit-view.png)). Effects, event processors (scripts) and modulators can
  be placed at program, layer or keygroup level; keygroups can hold several oscillators of any type.
- **24 oscillators, 8 of them sample-based:** Sample (with crop, normalise, silence, reverse, fades and "apply an
  effect in place"), **Slice** (transient or embedded REX markers, export slices as a Falcon patch, WAVs or a MIDI
  sequence), **Stretch** (real-time, low-CPU pitch shift up to ±2 octaves, with a mono/solo algorithm),
  **IRCAM Granular**, **IRCAM Multi Granular**, **IRCAM Stretch** (a phase vocoder with transient and envelope
  preservation), **IRCAM Scrub**, and **Grains** (new in 2026: Classic, Cloud and Pitch scheduling, a parametric grain
  envelope and per-grain filtering).
- **Quality:** the Sample oscillator's *Interpolation Mode* is Lo-fi, Standard or Best; a preference forces Best at
  load. **Streaming** is a per-program toggle with a per-sample "allow streaming" override; the preferences set a
  hard-drive type and cache size.
- **Keygroup trigger modes:** ON, OFF, **OFF + E** (release sample starts at the amplitude the envelope had reached),
  **OFF + V** (uses the note-off velocity), **OFF + VE**; trigger sync Immediate, Next Beat, Next Bar; 32 exclusive
  groups per layer; Latch.
- **Mapping editor:** blocks on key × velocity; Ctrl-drag an edge to make a fade (drawn dashed); "highlight played
  keygroups"; import with a dialog that parses names; *Apply Keygroup Template*; Extend commands; Layer Rules.
- **Scripting:** Lua for event processors and scripted front ends. Imports SFZ and EXS (the change log is full of
  "SFZ handling fixes").

### 3.3 HALion 7.5

- A sampler plus seven zone types: Sample, Virtual Analogue synth, Wavetable, Granular, Organ, FM (8 operators) and
  Spectral; layered freely in one program ([SOS](https://www.soundonsound.com/reviews/steinberg-halion-7)).
- Sample editing includes a spectral warp mode and **Decompose** (split a sample into tonal and noise parts; 7.5 does
  it in real time and adds transient). 7.5 adds **MPE** and spectral snapshots.
- For developers: Library Creator (packaged `.vstsound` libraries), Macro Page Designer (front ends), Lua scripting
  (from memory), a free player (HALion Sonic, from memory).
- Reviewers: "a bit of a beast", a serious learning curve, a ~700-page manual.

### 3.4 Ableton Sampler, Simpler and Drum Sampler (Live 12)

- **Sampler** tabs: Zone, Sample, Pitch/Osc, Filter/Global, Modulation, MIDI. The Zone editor lists sample layers
  and shows **one axis at a time** — Key, Velocity, or **Sample Select** (a third, user-driven axis for articulations
  or mic mixes) ([smp-11](screenshots/smp-11-ableton-sampler-key-zone-editor.png)). Zone fades are Lin or Pow.
- **Round robin** (new in 12): a toggle, a mode (Forward and others) and a **Reset** interval such as 1 Bar
  ([smp-13](screenshots/smp-13-ableton-sampler-round-robin.png)).
- Global **Interpol** Normal / Good / Best ("significant demands on your CPU" above Normal), and a **RAM mode** that
  loads the whole multisample (the default streams). Loops use **constant-power crossfades by default**. *Snap* moves
  start/end points to zero crossings **of the left channel only**, so the manual advises a small crossfade too.
- **Simpler:** Classic (polyphonic, looping, ADSR), One-Shot (monophonic, no loop), **Slice** (by Transient with a
  sensitivity up to 64 slices, Beat, Region or Manual; Trigger/Gate; Mono/Poly), with Live's warp modes available in
  all three ([smp-12](screenshots/smp-12-ableton-simpler-slice-mode.png)). Warped clips dropped into Simpler keep
  their warp markers.
- **Drum Sampler** (Live 12): one-shot playback with nine playback effects (Stretch, Loop, Pitch Env, Punch,
  8-Bit, FM, Ring Mod, Sub Osc, Noise).
- **Host features:** *Slice to New MIDI Track* and *Slice to Drum Rack*; **similar-sample swapping** (buttons that swap
  the loaded sample for the next most similar one in the library).

### 3.5 Logic Sampler, Quick Sampler and Auto Sampler (Logic Pro 12.3)

- **Sampler**: Mapping pane with three editors — the Key Mapping Editor (zones over a keyboard, velocity editable
  graphically), Group view and Zone view (column tables) — and a Zone pane with the waveform, start/end, fades, loop
  and crossfade ([smp-20](screenshots/smp-20-logic-sampler-mapping-zone.png)).
- **Import:** drop SoundFont2, DLS or Gigasampler files into the Sampler Instruments folder and Logic converts them
  (SF2 banks become a `.bk` folder of instruments). **Streaming** ("virtual memory") keeps only the attacks in RAM.
- **Quick Sampler**: one file, modes Classic, One Shot, Slice, Recorder; **Flex** with *Follow Tempo* for tempo-tagged
  audio; can be swapped for Sampler with its content carried over.
- **Convert Regions to New Sampler Track**: selected audio regions become zones, either one per region or one per
  **transient-marker section**; the target is Sampler, Drum Machine Designer or Alchemy; options for "1Shot" zones,
  a trigger-note range and even spreading; Logic creates **MIDI regions with ascending trigger notes** on the new
  track and mutes the originals. It is a one-time conversion.
- **Auto Sampler** samples hardware or software instruments into a Sampler instrument.

### 3.6 FL Studio: DirectWave, Slicex, FLEX

- **DirectWave**: a bank of up to 16 programs; each program's zones shown both as a spreadsheet and as a grid
  ([smp-15](screenshots/smp-15-fl-directwave-main.png)); five loop modes (Disabled, Forward, One Shot, Sustained,
  Bounce); mute groups; **trigger groups** with Cycle, Random and **Avoid Previous**; a *Channel Sampler* that
  auto-samples any plug-in with key zones, velocity layers and cycle layers. It **imports more formats than any other
  commercial sampler here**: Akai AKP, Battery 1–3, EXS24, Giga, Kontakt 1/2, Kurzweil, MPC (disk), Reason NN-XT,
  ReCycle, SoundFont2 and SFZ — none if encrypted; SF2 import drops filter, chorus and reverb settings. Monolithic
  `.dwp`/`.dwb` files embed FLAC. Interpolation: a 16-point sinc option that the manual says "uses less CPU but may
  produce more aliasing". No disk streaming is documented.
- **Slicex**: beat-detection slicing (uses embedded slice data first), per-slice and global stretching tuned for drum
  loops, layered decks, articulators; slices are played from the piano roll (dumping the slice score to the piano roll
  is a long-standing feature, from memory).
- **FLEX**: a preset player (subtractive, wavetable, multisample, FM/AM) with eight macros and `.flexpack` content;
  users cannot build their own instruments in it.

### 3.7 Decent Sampler

- A free player whose instruments are a folder of audio plus one XML `.dspreset`; `.dslibrary` is the same zipped,
  `.dsbundle` an unzipped folder. **Commercial libraries are encrypted** and only the plug-in can decrypt them.
- The library author defines the whole front end (`<ui>`: images, knobs, menus, keyboard colours) and bindings from
  controls to engine parameters ([smp-17](screenshots/smp-17-decent-sampler-player.jpg)). Since August 2026 it also
  has an in-app preset editor with loop tools, oscillators, a 6-operator FM engine and DX7 patch import.
- Format details in §5.3.

### 3.8 TX16Wx

- Free, with a Professional tier that adds time-stretch/pitch-shift modes, effects routing and trigger switching.
  Reads WAV, AIFF, Ogg, FLAC and Yamaha waves; loads SF2, SFZ, Akai and EXS.
- 128 voices, **RAM or DFD chosen per zone** (a column in the mapping table,
  [smp-18](screenshots/smp-18-tx16wx-keymapper.png)); two LFOs, two envelopes and three step sequencers per voice into
  a matrix; interactive slicer with automatic keyboard layout; wave editor with non-destructive real-time crossfade,
  **loop overlay** ([smp-19](screenshots/smp-19-tx16wx-loop-overlay.png)) and automatic loop detection.
- Its sound files are a **fully documented XML format** (`.txprog`, `.txbank`, `.txperf`).

### 3.9 sforzando and sfizz (the SFZ players)

- **sforzando** (Plogue): "no fancy UI, effects or mixers — every tweak has to be done at the SFZ file level"; highly
  SFZ 2.0 compliant with ARIA extensions; converts SF2, DLS and acidised WAV to SFZ. ARIA's `bank.xml` front ends are
  proprietary, which is why few SFZ libraries have a GUI (forum, 2022).
- **sfizz** (BSD-2): the reference open engine. Interpolators: nearest, linear, **Hermite (default, "same as
  sforzando")**, cubic B-spline, and Kaiser-windowed sinc with 8, 12, 16, 24, 36, 48, 60 or 72 points
  (`sample_quality` 1–10) — **a fixed kernel whose cutoff does not follow the pitch ratio**. Engine constants: 64 voices
  by default, 256 maximum, **8,192-frame preload**, 1,024-frame file chunks, voices stolen by age and by power, a 1.5×
  pool of *overflow* voices so stolen voices can fade out, an S-shaped loop-crossfade curve, and a 10 ms fast release
  ([Config.h](https://github.com/sfztools/sfizz/blob/develop/src/sfizz/Config.h)).

### 3.10 Fast chopping: Serato Sample and XLN Life

- **Serato Sample**: "Find Samples" auto-cue placement, up to 32 cue pads, Pitch 'n Time stretch and key shift,
  real-time **stem separation**, key and BPM detection, keyboard mode, Hold/Trigger and Mono/Poly per pad.
- **XLN Life**: record or drop audio, and it **slices, clusters similar hits into sounds, and generates grooves**
  with Density, Syncopation and Symmetry; slices are coloured by cluster
  ([smp-14](screenshots/smp-14-xln-life-edit.png)). Field Recorder phone app; a 30-second DAW record buffer.

---

## 4. The DSP and the engine

### 4.1 Interpolation: what transposition costs, and why most samplers alias

A sampler plays a recording at a rate ratio `r = 2^(semitones/12) × fs_sample / fs_out`. Two errors appear:
**imaging/aliasing from the interpolator** (it is a poor low-pass filter), and, when `r > 1`, **aliasing of the
source's own high band** (content between `fs_out/(2r)` and `fs_sample/2` folds back below Nyquist). The second one
is the one users hear as "metallic" top octaves on pitched-up samples.

| Method | Quality numbers | Cost | Who uses it |
|---|---|---|---|
| Drop-sample / linear | Linear: 19.1 dB modified SNR on 2×-oversampled input, 33.8 dB on 4×, 47.0 dB on 8× (Niemitalo 2001) | 1–2 MACs | Kontakt "Standard" (per the KVR developer), Cantus pads today |
| 4-point 3rd-order Hermite | 23.5 dB on 2×, 44.2 dB on 4×, 64.0 dB on 8× (Niemitalo) | ~8 ops | sfizz and sforzando default |
| 4-point 3rd-order "optimal" (Niemitalo) | 65.9 dB on 2×, 89.0 dB on 4× | ~same as Hermite | — (needs oversampled input) |
| 6-point 5th-order optimal | 111.4 dB on 2×, 149.3 dB on 4× | ~20 ops | — (needs oversampled input) |
| Windowed sinc, fixed kernel | Good passband; **no protection when `r > 1`** | N taps (8–72) | sfizz Sinc 8–72, DirectWave 16-point |
| Windowed sinc, kernel scaled by `1/r` (Smith's bandlimited interpolation) | Alias-free at any ratio | **N × r taps**: cost grows with the transposition | Consistent with Kontakt High/Perfect's documented behaviour |
| **Mip-map + oversampled polynomial/polyphase + half-band decimator** (de Soras 2005) | Alias suppression limited by the mip-map and half-band filters (80–120 dB achievable) | Bounded at any ratio | The target for Cantus |

Two lessons from the literature:
- **Niemitalo's SNR figures assume oversampled input.** On a 44.1 kHz recording that is *not* oversampled, a
  polynomial interpolator alone is poor. His recommended hybrid is to oversample by an integer factor with a proper
  FIR (a polyphase filter) and then interpolate that with a low-order polynomial; the polynomial then performs as in
  his tables ([deip.pdf](http://yehar.com/blog/wp-content/uploads/2009/08/deip.pdf)).
- **De Soras's resampler** adds octave **mip-mapping**: each sample is stored at levels 0, 1, 2… where level *k* is
  low-passed and decimated by 2^k. For a ratio `r`, pick the level that brings the local ratio into [0.5, 1), run the
  interpolator at **twice the output rate**, and decimate with a half-band filter (his HIIR polyphase IIR). Cost is
  independent of `r`; memory grows by at most 2× (1 + ½ + ¼ …)
  ([The Quest For The Perfect Resampler, 2005](http://ldesoras.free.fr/doc/articles/resampler-en.pdf)). Cantus
  already has a 4× polyphase interpolator and decimator in `cantus-dsp::terminus_os` and half-band machinery in
  Comprimo's oversampler; the pieces exist.
- **Pitching down** (`r < 1`) needs no anti-alias filter but benefits from a flat passband; the same interpolator
  running at 2× covers it.
- **Unity must be exact.** When `r == 1` and the phase is integral, the output should be the input samples, bit for
  bit, the same rule Aequalis has for 0 dB.

### 4.2 Voices, stealing and polyphony

- **Voice pool.** Pre-allocated (ADR 0005). sfizz uses 64 by default, 256 max, and keeps a **1.5× overflow pool**
  so that stolen voices can finish a short fade instead of being cut. Kontakt allows the count to "temporarily exceed
  the maximum" while the fade (ms, per voice group) runs.
- **Who is stolen.** Kontakt: any, oldest, newest, highest, lowest, with *prefer released*. sfizz: age- and
  power-weighted (thresholds at 50 % of the oldest age and 50 % of summed power). SFZ exposes `polyphony` (per group or
  region), `note_polyphony` with `note_selfmask` (a repeated note may only steal quieter copies of itself), and ARIA's
  `polyphony_stealing`. Salamander's SFZ uses `note_selfmask` to keep piano repetitions cheap.
- **Choke / exclusive groups.** SFZ `group` + `off_by` + `off_mode` (`fast`, `normal` = enter release, `time` with
  ARIA's `off_time`, default 6 ms); SF2 `exclusiveClass`; Kontakt exclusive groups (16); Falcon (32 per layer); Decent
  Sampler `tags` + `silencedByTags` with `silencingMode` fast/normal and `silencingDecay`. Pulsus already has choke
  groups (1–8).
- **Recommendation:** steal released voices first, then the quietest (tracked envelope level, as sfizz's power
  follower does), then the oldest; always a 5 ms minimum fade; per-group and per-note limits; stealing is
  deterministic (ties broken by voice index), so renders repeat.

### 4.3 Release triggers, legato and true legato

- **Release samples:** SFZ `trigger=release` (also on sustain-pedal up) and `release_key` (on key-up even if the
  pedal is down); `rt_decay` attenuates the release sample by *n* dB per second the note was held; `rt_dead` decides
  whether a release plays after its sustain sample has ended. Kontakt has a release trigger with a held-time counter.
  **Falcon's OFF + E** starts the release at the amplitude the attack voice had reached; that is the most natural
  default and none of the formats encodes it directly.
- **Legato:** SFZ `trigger=first` (no other note held) and `trigger=legato` (another note held) plus `sw_previous`
  (condition on the previous note). Decent Sampler adds `legatoInterval` (exact semitone distance to the previous
  note) and `previousNotes`, and its guide documents a three-group recipe (first-note sustains, transitions, and
  sustains that follow transitions, with tags silencing each other). **True legato** = a recorded transition sample
  per interval, crossfaded into the destination sustain at an offset; it is data-heavy and mostly script-driven in
  Kontakt.
- **Glide/portamento** (Decent `glideTime`/`glideMode`, SFZ v2 bend smoothing) is the cheap alternative.

### 4.4 Velocity layers, crossfades and dynamics

- **Velocity and key crossfades:** SFZ `xfin_lovel/hivel`, `xfout_lovel/hivel`, `xfin/xfout_lokey/hikey` and CC
  crossfades `xfin_loccN…`, with `xf_velcurve`/`xf_keycurve`/`xf_cccurve` = `gain` (linear, "best for phase-aligned
  material") or `power` (equal-power, "better to mix very different material", the default). Ableton Lin/Pow; Kontakt
  gradient crossfades; TX16Wx fade columns.
- **CC crossfades are how orchestral "dynamics on the mod wheel" work** (the same note crossfades p → mf → f while
  held). This needs every layer's voice running at once, so it costs polyphony.
- **Velocity curve:** SFZ `amp_veltrack` (default 100 %) and `amp_velcurve_N` points; SF2's default modulator maps
  velocity to 960 cB (96 dB) of attenuation with a concave curve; Kontakt maps velocity to volume as 60·log10(x) dB,
  i.e. amplitude ∝ velocity³ (ConvertWithMoss's measurement). **Importers must carry the curve**, or a library's
  dynamics change on import.

### 4.5 Round robin and random

- SFZ: `seq_length` + `seq_position` (per key), `lorand`/`hirand` (one random number per note, shared by all
  regions, so layered randoms stay coherent). Decent Sampler: `seqMode` = `round_robin`, `random`, `true_random`,
  `always`. DirectWave: Cycle, Random, **Avoid Previous**. Kontakt: cycle round robin / cycle random group conditions.
  Live 12: modes plus a **Reset** interval.
- **For Cantus the hard requirement is determinism** (ADR 0005 rule 5: offline render equals what the user heard).
  Round-robin counters and random generators must be part of the voice-allocator state, seeded per instrument, and
  **reset at a defined point** (transport start, or every bar like Live), so a bounce, a freeze and a live playback
  pick the same samples. "Random without immediate repeat" should be the default random mode.

### 4.6 Loops: points, crossfades, zero crossings

- **Where loop points come from:** the WAV `smpl` chunk (loop type forward/alternating/backward, start, end, play
  count, plus MIDI unity note and pitch fraction), AIFF `INST` + `MARK`, SF2 sample headers, SFZ `loop_start/end`
  (override the file), Decent `loopStart/loopEnd` (the file's markers win if present).
- **Modes to support:** forward, ping-pong (alternate), backward; *sustain loop* (loop while held, then play the tail:
  SFZ `loop_sustain`, Kontakt "until release", DirectWave "Sustained", SF2 sampleModes 3) and *continuous* (keep
  looping through the release); one-shot; a loop count (SFZ `count`, Kontakt Count). Kontakt's per-loop tune and
  multiple loops per zone are rarely used; support one sustain loop and one release loop (as TX16Wx does).
- **Crossfade:** SFZ v2 `loop_crossfade` (seconds), Decent `loopCrossfade` (frames) with `linear`/`equal_power`,
  Kontakt X-Fade (ms), Live constant-power by default, sfizz an S-curve. Equal-power suits uncorrelated material;
  linear suits a loop whose two ends are already phase-aligned (a crossfade between correlated signals should sum
  to constant *amplitude*).
- **Zero crossings help less than people think.** Snapping start and end to zero crossings removes the level jump but
  not the slope jump, and on stereo files the channels cross zero at different places (Live snaps on the left channel
  only). Better: search near the requested point for the offset that minimises the error over a short window
  (normalised cross-correlation of the audio around the end with the audio around the start), then apply a short
  crossfade. SF2 even asks authors to make the eight points around both loop points identical.
- **Display:** Kontakt's split seam view and TX16Wx's overlay are the two tools that work; show both, and print the
  seam error in dB.

### 4.7 Disk streaming and memory

- **The scheme everyone uses:** keep the first *N* frames of every sample in RAM (the preload), start the voice from
  it instantly, and have a background thread fill a per-voice ring buffer with the rest. Kontakt DFD (preload slider;
  an override in the Memory options), Falcon (per-program toggle, drive type and cache size), Logic (virtual memory:
  "only the initial attacks … in RAM"), Decent (`playbackMode` = memory / disk_streaming / auto), TX16Wx (RAM/DFD per
  zone), Live (streams unless RAM mode). sfizz: 8,192 frames preloaded, 1,024-frame chunks, four background threads.
- **The preload must scale with the pitch ratio.** A voice pitched up by +24 semitones consumes source frames four
  times as fast. Preload needed = `ceil(r_max × T_budget × fs_sample) + interpolator margin`, where `T_budget` is the
  worst-case time to fetch the next chunk. With `T_budget` = 100 ms at 48 kHz: 4,800 frames at unity, **19,200 at
  +24 semitones**. A fixed 8,192-frame preload covers about 43 ms at +24 semitones. Compute `r_max` per zone from its
  key range above the root (plus pitch-bend range and pitch modulation depth).
- **Loops stay resident.** A looped region is re-read forever; keep it in RAM, not in the stream.
- **Memory arithmetic:** one minute of stereo 48 kHz audio is 11.0 MiB as `i16`, 16.5 MiB as `i24`, 22.0 MiB as
  `f32`. Cantus today decodes to `f32` and never evicts (fine for a drum kit, a 2× penalty for a 16-bit library).
  Store samples at native depth and convert inside the voice; offer lossless compression (NCW is DPCM with bit
  truncation; FLAC is the open equivalent) for RAM, not just disk.
- **Real-time rules:** no file I/O, locks or allocation on the audio thread (ADR 0005). **No memory-mapped sample
  reads on the audio thread** either: a page fault is a blocking syscall. Voices request chunks through an SPSC queue;
  the loader answers into pre-allocated ring buffers; a voice that underruns outputs silence for the missing frames,
  counts a `stream_miss` in `EngineStats`, and never blocks.
- **Offline render must wait for the disk.** The render loop is not real-time, so it should *pull*: before each
  block, block the render thread until every active voice has the frames it needs. Then the offline render equals a
  real-time run that had no misses, bit for bit.
- **Background loading:** Kontakt lets notes play while a library is still loading ("allow instant playback"),
  accepting glitches. Cantus should load preloads first (they are small), mark the instrument "streaming ready", then
  warm caches.

### 4.8 Time-stretch and pitch-shift: algorithms and licences

| Library | Licence (checked) | Algorithm | Fit for `cantus-dsp` (MIT OR Apache-2.0) |
|---|---|---|---|
| **Signalsmith Stretch** | **MIT** (GitHub mirror; header-only C++11; Rust wrappers `signalsmith-stretch` 0.1.3 and `ssstretch`, both MIT) | STFT-based, polyphonic; default 120 ms block with 30 ms interval, "cheaper" 100/40 ms; formant shift and compensation with an f0 hint; `seek()` and `outputSeek()` to pre-roll so a fixed-length sound stretches exactly; latency reported as input and output halves | **Yes.** Needs a C++ toolchain, or a Rust port (the licence allows it) |
| **Bungee** | **MPL-2.0** (Bungee Pro is commercial) | Frequency-domain, real-time, for time-stretch and pitch | Usable: file-level copyleft, compatible with MIT/Apache linking if its files stay MPL |
| **Rubber Band** | **GPL-2.0-or-later or a commercial licence** (no royalties, no expiry) | Phase vocoder with transient handling; the R3 "finer" engine is its high-quality mode (from memory) | **No** for the engine crates unless bought; GPL would only fit the GPL app crates |
| **zplane élastique** | Commercial | — | Only with a paid licence (the Kontakt Time Machine Pro and many DAWs use it, from memory) |
| **SoundTouch** | LGPL-2.1 (from memory) | WSOLA | Awkward for static Rust linking |

The literature to implement from: phase vocoder with phase locking (Laroche & Dolson 1999), WSOLA for
monophonic/percussive material (Verhelst & Roelands 1993), the review of time-scale modification by Driedger & Müller
(2016), phase-gradient heap integration (Průša & Holighaus 2017), and Signalsmith's ADC 2022 talk "Four Ways To Write
A Pitch-Shifter". **Recommendation:** Signalsmith Stretch for tonal and polyphonic zones and for tempo-following
loops; slice playback (Beat-Machine style) for drum loops, which is transient-perfect by construction; a small in-house
WSOLA for cheap monophonic voice shifting. Stretching needs a look-ahead of about one block, so a stretched zone must
pre-roll from its preload with `seek()`; budget that in the preload formula.

### 4.9 Slicing by transient, root-key detection, loop finding

- **Onset detection:** spectral flux on log-compressed magnitudes with adaptive-threshold peak picking (Bello et al.,
  "A Tutorial on Onset Detection in Music Signals", IEEE TSAP 2005; Dixon, "Onset Detection Revisited", DAFx 2006);
  **SuperFlux** (Böck & Widmer, DAFx 2013) adds a maximum filter across frequency to ignore vibrato, which matters on
  sustained instruments. Expose one *sensitivity* control (Simpler and Falcon do), keep manual slices when sensitivity
  changes (Simpler does, with Alt-click to toggle manual/auto), and read embedded slice markers first (WAV `cue ` and
  `LIST/adtl`, ACID, REX via its SDK; Slicex and Falcon do this).
- **Slice boundaries:** start each slice at the onset minus a few ms, then snap to the preceding zero crossing or
  apply a 1–2 ms fade (Kontakt's Beat Machine crossfades slices with an attack control; Slicex offers declicking).
- **Root-key detection** for dropped files: YIN (de Cheveigné & Kawahara, JASA 2002) or pYIN (Mauch & Dixon, ICASSP
  2014) on the sustain portion; fall back to note names in file names (ConvertWithMoss and Kontakt's auto-mapper both
  parse names; DirectWave has its own tokens such as `+KA3-F#4+V64-95`), then WAV `smpl`/`inst` metadata, which
  should win over both.
- **Clustering hits into "sounds"** (Life): MFCC or spectral-centroid features per slice, then k-means or
  agglomerative clustering; colour slices by cluster.

---

## 5. The file formats

### 5.1 The interop map

| Format | Nature | Openness | Cantus plan |
|---|---|---|---|
| **WAV / AIFF / FLAC / Ogg** + metadata | Audio with `smpl` (loops, unity note, fine tune), `inst` (root, key and velocity range, gain), `acid` (tempo, beats, root), `cue ` + `LIST/adtl` (markers, slices); AIFF `INST`/`MARK` | Open | **P0 import**; P0 export of edited zones as WAV with `smpl`/`inst` written back |
| **SFZ** (`.sfz` + samples) | Plain text, hierarchical headers and opcodes | Open, documented at sfzformat.com | **P0 import, P0 export** (our native interchange) |
| **Decent Sampler** (`.dspreset`, `.dslibrary` = zip, `.dsbundle` = folder) | XML | Open, documented; commercial libraries encrypted | **P0 import** of unencrypted; P1 export |
| **SoundFont 2 / SF3** (`.sf2`, `.sf3`) | Binary RIFF, samples embedded (16-bit + optional `sm24`; SF3 = Ogg Vorbis samples, a MuseScore extension) | Open spec (2.04, 2006) | **P0/P1 import**; no export at first |
| **Bitwig `.multisample`** | Zip of WAVs + XML; key/velocity crossfades, groups, metadata; read by Bitwig and Studio One | Open | P1 import and export |
| **TX16Wx** (`.txprog`, `.txbank`) | XML | Documented by its author | P2 import |
| **Logic EXS24** (`.exs`) | Binary, **absolute sample paths**, sometimes one consolidated audio file with zones as ranges | Proprietary, reverse-engineered | P2 import (search for samples near the file) |
| **Ableton** `.adv`/`.adg` | Gzipped XML | Proprietary but readable; **Ableton's own library AIFFs are encrypted** | P2 import of user presets |
| **FL DirectWave** `.dwp` | Binary; monolithic variant embeds FLAC | Proprietary; structure documented by ConvertWithMoss | P2 import (FL users are Cantus's core audience) |
| **Akai MPC** `.xpm`, Korg, Elektron, 1010music… | Various | Mixed | P2 export for hardware users |
| **Kontakt NKI/NKM**, unencrypted, K1–K7 | Binary, many versions; NCW audio | Proprietary, reverse-engineered (ConvertWithMoss; the `ncw` Rust crate, MIT OR Apache-2.0) | P2 at most, after legal review |
| **Kontakt Player libraries (encrypted NKI/NKX, NICNT)**, encrypted Decent libraries, HALion `.vstsound`, UVI `.ufs`, Ableton pack AIFFs, NN-XT Refills | Encrypted | Closed | **Never.** Circumventing protection is illegal in most jurisdictions and would destroy trust with developers. Offer "record the plug-in into a sampler" instead (§9 P1). |

ConvertWithMoss (LGPL-3, Java) already converts between about 60 formats and documents every one; it is the best
reference for parameter mapping and edge cases. Its code cannot be linked into the MIT/Apache crates, but its
documentation and conventions can guide ours (for example: one-shot and choke are stored per zone; a format that
cannot express random falls back to round robin, never to "play all").

### 5.2 SFZ: what to implement, in priority order

**Syntax.** Headers `<control>`, `<global>`, `<master>` (ARIA), `<group>`, `<region>`, `<curve>`, `<effect>`,
`<midi>`, `<sample>`. Opcodes are `name=value` separated by whitespace; the `sample` value may contain spaces and
runs to the next `name=`; comments are `//` (and `/* */` in ARIA/sfizz); `#define $NAME value` and `#include "file"`.
Opcodes inherit downward: a region gets `<global>` → `<master>` → `<group>` → its own values, the most specific
winning; a new `<group>` resets group-level values. Paths are relative to the `.sfz` file (or `default_path`), with
`/` or `\`. Notes may be numbers or names (`c4` = 60; `note_offset` and `octave_offset` exist for libraries that
disagree). SFZ v1 audio is WAV and Ogg; v2 adds AIFF and FLAC. `sample=*sine` (also `*saw`, `*square`, `*triangle`,
`*noise`, `*silence`) are built-in generators.

**sfzformat.com's database lists 433 opcodes: 128 SFZ v1, 215 SFZ v2, 87 ARIA, one each for LinuxSampler, sfizz and
Calfbox.** A useful importer needs about sixty.

| Tier | Opcodes | Why |
|---|---|---|
| **1: mapping** (P0) | `sample`, `default_path`, `key`, `lokey`, `hikey`, `pitch_keycenter`, `lovel`, `hivel`, `note_offset`, `octave_offset`, `#define`, `#include` | Without these nothing loads |
| **1: playback** | `offset`, `end`, `loop_mode` (`no_loop`, `one_shot`, `loop_continuous`, `loop_sustain`), `loop_start`, `loop_end`, `direction`, `count`, `delay` | Plays the right part of the file |
| **1: level and pitch** | `volume` (dB), `amplitude` (ARIA, %), `pan`, `width`, `position`, `tune` (cents), `transpose`, `pitch_keytrack`, `amp_veltrack`, `amp_velcurve_N`, `bend_up`, `bend_down` | Keeps the library's balance and dynamics |
| **1: envelope** | `ampeg_delay/attack/hold/decay/sustain/release/start`, ARIA `ampeg_attack_shape/decay_shape/release_shape` | Every library sets release times |
| **1: triggers and articulation** | `trigger` (`attack`, `release`, `first`, `legato`, `release_key`), `rt_decay`, `seq_length`, `seq_position`, `lorand`, `hirand`, `sw_lokey`, `sw_hikey`, `sw_last`, `sw_default`, `sw_label`, `loccN`/`hiccN` | Round robin, release samples, keyswitches, pedal layers |
| **1: voice control** | `group`, `off_by`, `off_mode`, `off_time`, `polyphony`, `note_polyphony`, `note_selfmask` | Chokes and repetitions (Salamander depends on self-masking) |
| **1: crossfades** | `xfin_lovel`, `xfin_hivel`, `xfout_lovel`, `xfout_hivel`, `xfin/xfout_lokey/hikey`, `xf_velcurve`, `xf_keycurve` | Velocity and key crossfades |
| **2: filter** (P1) | `cutoff`, `resonance`, `fil_type` (map the 2-pole and 4-pole LP/HP/BP/BR types to Cantus's SVF), `fil_keytrack`, `fil_keycenter`, `fil_veltrack`, `fileg_*` | Tone shaping in many libraries |
| **2: CC modulation** | `_onccN` suffixes (`volume_onccN`, `amplitude_onccN`, `pan_onccN`, `cutoff_onccN`, `pitch_onccN`/`tune_onccN`, `offset_onccN`), `_curveccN`, `_smoothccN`, `_stepccN`, `<curve>` headers, `xfin_loccN`/`xfout_loccN` + `xf_cccurve`, `set_ccN`, `label_ccN` | Mod-wheel dynamics and CC-driven controls in orchestral and scripted SFZ libraries |
| **2: v2 modulators** | `lfoN_freq/…/pitch/volume/cutoff`, `egN_points`, `egN_timeX`, `egN_levelX`, `egN_shapeX`, `egN_sustain`, `pitchlfo_*`, `amplfo_*`, `fillfo_*` (v1) | Vibrato, tremolo, flexible envelopes |
| **2: misc** | `loop_crossfade`, `sw_previous`, `sustain_sw`, `sostenuto_sw`, `sustain_cc`, `rt_dead`, `eqN_freq/gain/bw`, `hint_*` (ignore), `sample_quality` (map to our quality tiers), `region_label`/`group_label`/`master_label` | Legato scripts, pedals, labels in the editor |
| **3: later** | `<effect>` buses (reverb, delay, etc.), `oscillator*`, `set_hdccN`, `varNN_*`, `image`, ARIA GUI `bank.xml` | Mostly replaced by Cantus's own effects and front end |

Importer rules: log unsupported opcodes per file (ConvertWithMoss has this option and it is how users report gaps);
treat `amp_velcurve_N` and ARIA shapes as curves, not approximations; honour embedded WAV loops unless the SFZ
overrides them; keep the original text in the project so a round trip does not lose opcodes we do not model.
Test against sfizz renders of the same file (sfizz is BSD-2, so it can be used as a reference in tests).

### 5.3 Decent Sampler `.dspreset`

**Structure.** Root `<DecentSampler minVersion="…">` with children `<ui>`, `<groups>`, `<effects>`, `<midi>`,
`<modulators>`, `<tags>`, `<noteSequences>`, `<arpeggiator>`, `<buses>`; a library folder may carry
`DSLibraryInfo.xml` ([developer guide](https://decentsampler-developers-guide.readthedocs.io/en/latest/)).

**`<groups>` / `<group>` / `<sample>` attributes that matter** (paraphrased from the guide):
- `<groups>`: `volume`/`globalVolume`, `globalPan`, `globalTuning`, `glideTime`, `glideMode` (`always`, `legato`, `off`).
- `<group>`: `enabled`, `volume`/`groupVolume`, `groupPan`, `groupTuning`, `ampVelTrack` (0–1), `pitchKeyTrack`,
  `ampEnvEnabled` (false = one-shot), `attack`, `decay`, `sustain`, `release` with `attackCurve`, `decayCurve`,
  `releaseCurve` (−100…100), `seqMode` (`round_robin`, `random`, `true_random`, `always`), `seqLength`, `loCCN`/`hiCCN`,
  `loopCrossfade`, `loopCrossfadeMode` (`linear`, `equal_power`), `tags`, `silencedByTags`.
- `<sample>`: **`path`**, **`rootNote`**, `loNote`, `hiNote`, `loVel`, `hiVel`, `start`, `end`, `tuning`, `volume`,
  `pan`, `trigger` (`attack`, `release`, `first`, `legato`, `continuous`), `releaseTriggerDecay`, `loopStart`,
  `loopEnd`, `loopEnabled`, `loopCrossfade`, `loopCrossfadeMode`, `seqPosition`, `onLoCCN`/`onHiCCN`, `tags`,
  `silencedByTags`, `silencingMode` (`fast`, `normal`), `silencingDecay`, `previousNotes`, `legatoInterval`, `delay` +
  `delayUnit`, `playbackMode` (`memory`, `disk_streaming`, `auto`), `output1Target…output8Target`.
- **Inheritance quirk:** most attributes inherit downward (the most specific wins), but **volume multiplies** across
  the three levels, and `globalPan`/`groupPan` **add** to `pan`. Get this wrong and a library's balance shifts.
- Formats: WAV, AIFF, FLAC. Embedded WAV loop markers are used unless overridden.

**Modulators and bindings:** `<lfo>` (shape, frequency in Hz or musical time, `modAmount`, `scope` global/voice),
`<envelope>`, `<midiCC>`, `<midiVelocity>`, `<mpeTimbre>`, `<mpePressure>`, `<random>`, each with `<binding>` children
that target `type` (`amp`, `effect`, `modulator`), `level` (`instrument`, `group`, `voice`) and a `parameter` name
(`AMP_VOLUME`, `GROUP_TUNING`, `FX_FILTER_FREQUENCY`…). UI controls bind the same way. Import strategy: map
modulators to Cantus's modulation service (family.md §7.6), turn `<ui>` knobs into macros, keep the background image
as the preset's artwork, drop what we cannot map with a log line.

### 5.4 SoundFont 2 (and SF3)

**File layout** (spec 2.04): a RIFF `sfbk` with three lists: `INFO` (version, name, copyright…), `sdta` (`smpl` =
16-bit little-endian samples for the whole bank; optional `sm24` = the low byte of each sample for 24-bit), and
`pdta` = the nine "hydra" sub-chunks (`phdr`, `pbag`, `pmod`, `pgen`, `inst`, `ibag`, `imod`, `igen`, `shdr`).

- **Hierarchy:** preset (bank, program) → preset zones → instrument → instrument zones → sample header. Each zone has
  generators (fixed settings) and modulators (MIDI-driven rules). A zone with no instrument/sample link is a *global
  zone* for its list.
- **Precedence:** instrument-level generators are **absolute**; preset-level generators are **added** to them (e.g. a
  preset attack of 2400 timecents on an instrument attack of 1200 gives 3600 = 8 s). Some generators are
  instrument-only (sample offsets, `keynum`, `velocity`, `sampleModes`, `exclusiveClass`, `overridingRootKey`) and must
  be ignored at the preset level. Key and velocity ranges intersect.
- **Units:** time in **timecents** (seconds = 2^(tc/1200); −12000 ≈ 1 ms is the default), attenuation in
  **centibels** (0.1 dB), filter cutoff in absolute cents (8.176 Hz × 2^(c/1200); 13500 ≈ 20 kHz = open), filter Q in
  cB, pan in 0.1 % (−500…500), sends in 0.1 %, mod-envelope sustain in 0.1 % decrease, volume-envelope sustain in cB of
  attenuation.
- **Generators to support first:** key/velocity ranges (43, 44), sample offsets (0–4, 12, 45, 50), `sampleModes` (54:
  0 no loop, 1 continuous, 3 loop while held then play to the end), `overridingRootKey` (58), `coarseTune`/`fineTune`
  (51, 52), `scaleTuning` (56), `initialAttenuation` (48), `pan` (17), the volume envelope (33–38 + key scaling 39–40),
  `exclusiveClass` (57), filter (8, 9), the modulation envelope and LFOs (5–7, 10–11, 13, 21–32), `keynum`/`velocity`
  overrides (46, 47).
- **Default modulators are implicit and must be applied:** velocity → attenuation (960 cB, concave), velocity → filter
  cutoff (−2400 cents), channel pressure and CC1 → vibrato depth, CC7 and CC11 → attenuation (concave), CC10 → pan,
  CC91 → reverb send, CC93 → chorus send, pitch wheel × RPN sensitivity → pitch.
- **Samples:** mono, left, right or linked (`sfSampleType`, with ROM variants to reject); every sample is followed by
  at least 46 zero samples; loops need eight valid points around each loop point. Combine linked left/right mono
  samples into stereo zones, but only when rate and length agree (ConvertWithMoss found commercial E-mu banks with
  bogus stereo links).
- **Known pitfalls:** hardware-era banks were tuned by ear on EMU chips, so players disagree on attenuation and filter
  scaling; FluidSynth follows the hardware on several quirks (from memory: an attenuation scaling factor). Validate
  against FluidSynth (LGPL, as a test reference only) or `rustysynth` (MIT) renders. **SF3** replaces sample data
  with Ogg Vorbis blocks (a MuseScore extension; `lewton` decodes Vorbis under MIT/Apache).

### 5.5 Rust building blocks (licences checked on crates.io)

| Need | Crate | Licence |
|---|---|---|
| WAV read/write | `hound` 3.5 (already used) | Apache-2.0 — but it does **not** parse `smpl`/`inst`/`cue `; add a small RIFF chunk reader |
| FLAC | `claxon` 0.4 | Apache-2.0 |
| Ogg Vorbis (SF3, SFZ) | `lewton` 0.10 | MIT OR Apache-2.0 |
| AIFF, CAF, MP3, many | `symphonia` 0.6 | MPL-2.0 (file-level copyleft; acceptable, note it in the ADR) |
| SF2 parse / reference synth | `soundfont` 0.1 (MIT), `rustysynth` 1.3 (MIT); `oxisynth` is LGPL-2.1 (avoid) | — |
| SFZ parse | `sofiza` 0.3 (MIT/Apache), small and old: write our own parser | — |
| Sample-rate conversion (offline, for mip-maps and import) | `rubato` 5.0 | MIT OR Apache-2.0 |
| Time-stretch | `signalsmith-stretch` 0.1.3 (MIT), `bungee-rs` (MPL-2.0) | — |
| NCW (if ever) | `ncw` 0.3 | MIT OR Apache-2.0 |

### 5.6 What to support first

1. **A folder of WAV/AIFF/FLAC files**, mapped from metadata (`smpl`, `inst`), then file-name note and velocity
   tokens, then pitch detection. This is what a musician who recorded their own instrument has.
2. **SFZ tier 1 + tier 2 filter**, because the free and open library world (VSCO 2 CE, VCSL, Karoryfer, Salamander,
   sfzinstruments.github.io) lives there. **Export SFZ** from day one: it makes Cantus a sampler
   authoring tool whose work outlives Cantus.
3. **Decent Sampler**, the second-largest free library ecosystem (Pianobook's free libraries are largely in this format, from memory); unencrypted only.
4. **SoundFont 2 / SF3**, for General MIDI banks, retro and game music.
5. Then Bitwig multisample, DirectWave, EXS24, Ableton presets, TX16Wx.

---

## 6. The interface: mapping, waveform editing, browsing

### 6.1 How zones are shown on key × velocity

| Product | Model | Strength | Weakness |
|---|---|---|---|
| **Kontakt** ([smp-01](screenshots/smp-01-kontakt-mapping-editor.png), [smp-02](screenshots/smp-02-kontakt-mapping-crossfades.png)) | A true 2D grid (keys across, velocity up), a status line for the selected zone, and a keyboard below that marks played notes with a red tick whose height is the velocity | Key and velocity visible at once; drag edges, Ctrl-drag for crossfades, rubber-band select, Alt-drag zoom; auto-map from name tokens; "Select zone via MIDI" | Names rotated 90° and unreadable in narrow zones; overlapping zones (round robins, mic layers) stack invisibly; groups are filtered by a toggle rather than shown; an icon-only control strip ("Auto", "Root", ⟷ icons); small bitmap type, grey on grey |
| **TX16Wx** ([smp-18](screenshots/smp-18-tx16wx-keymapper.png)) | 2D grid **plus a synchronized table**; fade ranges are drawn *inside* each zone as a gradient | The best of both: shape in the grid, exact numbers in the table (Lo/Hi key, Lo/Hi vel, fade key and fade velocity, root, fine, sample, RAM/DFD, loops, level, pan) | Dark, saturated greens; names still vertical in narrow zones |
| **DirectWave** ([smp-15](screenshots/smp-15-fl-directwave-main.png)) | A spreadsheet of zones above a grid | Sortable and exact; FL users know it | The grid is an afterthought: tiny cells, rotated labels, and the two views compete for height |
| **Ableton Sampler** ([smp-11](screenshots/smp-11-ableton-sampler-key-zone-editor.png), [smp-13](screenshots/smp-13-ableton-sampler-round-robin.png)) | A list of layers, **one row per sample**, each a bar on a keyboard ruler; Key, Vel and Sel are three separate views | Always legible (names never rotate); scales to hundreds of layers; the Sample Select axis is a clean way to handle articulations and mic sets | You never see key and velocity together; a velocity-layered piano becomes a long list |
| **Logic Sampler** ([smp-20](screenshots/smp-20-logic-sampler-mapping-zone.png)) | Groups list + zones as labelled columns over a keyboard, plus Group and Zone table views | Values visible at rest as "Label: value" strips; the selected zone in a contrasting colour; the waveform pane directly below | Velocity is secondary in the key view; blue on navy is low contrast for unselected zones |
| **Falcon** ([smp-09](screenshots/smp-09-falcon-2026-edit-view.png)) | A mapping strip docked at the bottom of an editor that shows the Program/Layer/Keygroup/Oscillator hierarchy as stacked panels | The hierarchy is always visible, with FX and events at each level; "highlight played keygroups" | The mapping view is small by default; MusicTech found the muted colours make sections hard to tell apart |

**Conclusion for Cantus:** a **2D key × velocity grid with a synchronized table** (TX16Wx's idea), **horizontal zone
labels that elide instead of rotating**, overlapping zones fanned out as a stack you can expand (the round-robin and
mic "layers" become a third axis, like Live's Sample Select, shown as tabs or a depth selector), and a keyboard strip
that shows **which zone each sounding voice is playing** and the round-robin position.

### 6.2 Waveform and loop editing

- **Kontakt's Wave Editor** ([smp-03](screenshots/smp-03-kontakt-wave-editor-loop-tab.png)): toolbar, a numeric status
  row, the waveform, tabs for Loop / Sync-Slice / Zone Envelopes / Sample Editor, a 2×4 grid of loop slots, and the
  **Loop Edit seam view** ([smp-04](screenshots/smp-04-kontakt-loop-edit-view.png)). The seam view is the single most
  useful loop tool in the survey; the eight-slot grid is rarely needed.
- **TX16Wx's loop overlay** ([smp-19](screenshots/smp-19-tx16wx-loop-overlay.png)) draws the post-end audio over the
  loop start. Cheaper to read than the seam view for long loops.
- **DirectWave** ([smp-16](screenshots/smp-16-fl-directwave-sample-tab.png)) draws a **spectrogram behind the waveform**
  (useful for spotting noise and tails) but sets loop points with **knobs**, which is the wrong control for a sample
  position.
- **Logic's Zone pane** shows start, end, fade-in, fade-out and loop handles directly on the waveform with the numbers
  below: the clearest layout.
- **Simpler** ([smp-12](screenshots/smp-12-ableton-simpler-slice-mode.png)): a big waveform, slice lines in a contrasting
  colour, and every mode as a readable word ("Slice By Region", "Warp Beats"). Its type size is what a modern sampler
  should look like.
- **Falcon's Slice oscillator** ([smp-10](screenshots/smp-10-falcon-slice-oscillator.png)) and **Life**
  ([smp-14](screenshots/smp-14-xln-life-edit.png)) show slicing as a *performance* surface: a detection mode and a
  sensitivity knob, then export or pattern generation. Life's colour-by-cluster is the best at-a-glance summary of
  what a slice set contains.

### 6.3 Browsing

- Kontakt 8's browser: library tiles, six tabs (Instrument, Combined, Tools, Leap, Loops, One-shots), NKS tags and a
  side pane; previews through Native Access-installed libraries only.
- Falcon 3.1+: tags with AND/OR, **audio previews**, favourites.
- Live 12: **similar-sample swapping** in Simpler and Drum Sampler, and similarity search in the browser; it answers
  "give me another snare like this one" without a folder hunt.
- DirectWave: a plain folder tree with a preview button.
- Decent Sampler: a library browser and an in-app store (1.28–1.33 reworked it).

### 6.4 Praise and criticism of the interfaces

- **Praised:** direct drag-and-drop mapping (all), Logic's always-visible values, Simpler's readability, Kontakt's
  seam view, Life's speed ("record a sound, get a beat").
- **Criticised:** Kontakt's Classic view is dated and dense (the new default view hides editing; the edit rack is a
  2,400-px column, [smp-06](screenshots/smp-06-kontakt-edit-rack-all-editors.png)); Kontakt 8 removed the option to hide
  instrument interfaces when stacking several libraries (MusicTech); Falcon's depth and muted palette (MusicTech);
  HALion's learning curve (SOS); sforzando has no editor at all.

---

## 7. What reviewers and users say

*Caveat: forum posts are single users' opinions and are marked as such; press reviews are dated.*

**Kontakt 8**
- MusicRadar (Dec 2024): Kontakt becomes "a performance tool and ideas generator", but it is pricey, Leap is "not
  essential" because a DAW can do the same, and the core sampling engine is largely unchanged.
- MusicTech (Feb 2025, 8/10): Chords and Phrases presets are strong, Leap is fun but limiting with longer use, its
  performance effects are global and it is stereo-only, and users lost the ability to hide instrument UIs.
- Sound On Sound (Dec 2024): Conflux, Chords, Phrases and Leap add creative options; not every user needs them;
  Phrases cannot create phrases manually; no MIDI Learn on Conflux macros; no full chord MIDI export.
- KVR forum (Dec 2020, a well-known Kontakt script developer): most libraries leave HQI at Standard, which he calls
  linear; users hearing a dull or "tube-like" sound at 96 kHz should try High or Perfect, or set Perfect for offline
  renders.
- Buyer's guides (search excerpt): the Player licence fee pushes indie developers to ship "full Kontakt only"
  libraries, a recurring source of buyer confusion.
- 2026 context: NI's insolvency and sale to inMusic raised questions about the long-term licence server and Native
  Access; statements so far promise continuity.

**Falcon** — MusicTech (Jan 2024, 10/10 for Falcon 3): huge flexibility, effects at any level, but "easy to become
lost in Falcon's limitless depths", and the muted colours make sections hard to tell apart.

**HALion 7** — Sound On Sound (May 2023): extensive sound design, a strong FM zone, drag-and-drop modulation,
dongle-free licensing, but "a bit of a beast" with a steep learning curve.

**SFZ players** — KVR forum (Feb 2022): sfizz's selling point is "the resampling quality" (its sinc modes);
sforzando "development pretty much stopped"; sforzando's GUI layer is proprietary, so few SFZ libraries have a front
end; sfizz lacked Ogg at the time. (sfizz itself has had no release since Jan 2024.)

**Decent Sampler** — press coverage (Synth Anatomy, Jan 2026) frames it as the free alternative to Kontakt, now with
oscillators; the release cadence of Aug–Sep 2026 (eight releases in a month) shows active development.

**Ableton and Logic** — the manuals themselves are candid about costs: Live says Good/Best interpolation place
"significant demands on your CPU" and its zero-crossing snap only looks at the left channel; Logic's convert-to-sampler
is one-way and needs an asset setting to copy samples into the project.

---

## 8. Gaps none of them fills

1. **Alias-free transposition by default at a bounded cost.** Everyone defaults to linear or Hermite; the good modes
   are opt-in and scale with the ratio. No product shows the user that a zone is aliasing.
2. **Render = playback, including randomness.** No sampler documents that round robin and random choices repeat
   exactly between a live take, a freeze and a bounce (Live's RR reset comes closest).
3. **A living link between the arrangement and the instrument.** Logic converts regions once; Live slices to a new
   MIDI track once. Move a slice marker later and the pattern does not follow; edit the source clip and the
   instrument does not update.
4. **Interop both ways.** Kontakt stopped importing foreign formats; DirectWave imports many but exports only its own;
   sforzando and Decent read one format each. Only an external Java tool (ConvertWithMoss) converts many-to-many.
5. **Honest loop tooling.** Seam view (Kontakt) and overlay (TX16Wx) exist separately; nobody measures the seam error,
   proposes the best nearby points and crossfade length, and shows the result numerically.
6. **Voice-level visibility.** Kontakt and Falcon highlight the played key or keygroup; none shows, per sounding
   voice, *which* sample, which round-robin slot, which loop pass, its stream-buffer health and why it was stolen.
7. **A mapping editor that is legible and keyboard-accessible.** Rotated names, low-contrast grids, no keyboard
   editing of zones, no screen-reader names; no localisation beyond English and a few languages.
8. **Streaming that adapts to pitch.** Preloads are fixed sizes (a slider in Kontakt, 8,192 frames in sfizz); none
   sizes them from the zone's actual transposition range.
9. **One undo for everything.** Plug-in samplers keep their own undo (or none) outside the host's history; zone edits
   and arrangement edits cannot be undone in one timeline.
10. **Sampling anything, deterministically.** Logic's Auto Sampler and DirectWave's Channel Sampler record in real
    time; a host that renders offline could sample its own instruments and plug-ins faster than real time and
    bit-exactly.

---

## 9. How Cantus beats them

The plan assumes what Cantus has today: deterministic, null-tested DSP (ADR 0005); a command bus with undo fuzz
(ADR 0004); the device frame and interaction rules of family.md §7; a channel rack and piano roll whose lanes come
from the instrument (`Lanes`, which already anticipates "a sampler with named zones"); a `SampleCache` that decodes
whole WAVs to `f32`; Pulsus pads that read samples with linear interpolation; and a GPL app on top of MIT/Apache
engine crates (ADR 0001). **Build the sample store, streaming, resampler, stretch, waveform peaks and slicing once, in
`cantus-dsp`/`cantus-engine`: audio clips (STATUS calls them "the single biggest hole") need exactly the same parts.**

### P0: parity foundation

1. **The zone engine.** Instrument → groups → zones (the data model below), with: key and velocity ranges; root and
   fine tune; gain, pan; start/end; a sustain loop and a release loop (forward, ping-pong, backward; sustain or
   continuous; crossfade with a linear or equal-power curve); one-shot; reverse; triggers (attack, release with
   `rt_decay`-style decay and **start-at-the-level-reached** like Falcon's OFF + E, first, legato, release-key);
   selectors (always, round robin, random without repeat, keyswitch, CC range, layer); choke groups; per-group and
   per-note polyphony; velocity and key crossfades with gain or power curves; velocity curves. Sample-accurate events,
   **block-size independent, deterministic**, null-tested.
2. **An alias-free resampler as the default.** De Soras's scheme (octave mip-maps + interpolation at 2× the output rate
   + half-band decimation), in four tiers: *Draft* (linear, explicit, for sketches and lo-fi), *Standard* (optimal
   polynomial on 2×-oversampled data), **High (default)** and *Best* (longer polyphase kernels; **always used for
   offline render and freeze**, like Kontakt's offline override but without asking). Unity pitch is bit-exact. Replace
   Pulsus's linear reader with it too; a pad tuned +24 semitones aliases today.
3. **Sample store and streaming.** Native-depth storage (`i16`/`i24`/`f32`) with optional in-RAM FLAC; preloads sized
   from each zone's maximum pitch ratio (§4.7); a loader thread feeding per-voice ring buffers through SPSC queues;
   looped regions resident; **pull-mode offline render** so bounces equal playback; stream misses counted in
   `EngineStats`; a missing file = exact silence + a relink prompt, never a project that fails to open; content hashes
   so moved libraries are found again; "collect samples into project".
4. **Import and export.** WAV/AIFF/FLAC folders with `smpl`/`inst`/`acid`/`cue ` metadata and name-token mapping; SFZ
   tier 1 (+ tier 2 filter) import and **SFZ export**; Decent Sampler import (unencrypted); SF2/SF3 import at the end
   of P0. Every importer logs what it could not map.
5. **The editor** (sketch below): the family device frame; a key × velocity grid with a synchronized zone table;
   horizontal elided labels; stacked layers for round robin and mics; a waveform editor with start/end/fade/loop
   handles, **seam view and overlay**, a measured seam error, "find best loop" and zero-crossing snap on *both*
   channels; drag files and folders from the OS or the browser onto keys; auto-map (metadata → name → pitch
   detection); batch crossfades; full keyboard control; every edit one host command (drags coalesced); EN/ES strings.
6. **Sectio, basic slicing.** Transient detection (spectral flux; SuperFlux for sustained sources) with one sensitivity
   control, manual slices that survive re-detection, embedded markers first, one slice per key, 1–2 ms edge fades,
   "export slices as MIDI" to the piano roll.

### P1: host-native advantages (where Cantus is clearly better)

1. **Drag a clip, a take or a time selection straight onto the keys.** The zone *references* the project audio
   (`SampleSource::Clip`), non-destructively. Trim or fade the clip and the instrument follows; comp a new take and the
   zone can follow the comp. Logic and Live copy once; Cantus keeps the link.
2. **Slicing writes a pattern into the channel rack, and stays linked.** Sectio slices a loop and writes a clip whose
   notes point at **slice IDs**, not raw pitches, with the slice names as piano-roll lanes (a `Lanes::Zones` variant).
   Move a marker or change the sensitivity and the pattern still plays the same material; re-order slices in the piano
   roll to re-sequence the beat. Life's clustering can colour and group the lanes.
3. **One undo history.** Zone edits, imports, slicing and arrangement edits interleave in one timeline, covered by the
   undo fuzz; A/B snapshots are host state (family.md §7.5).
4. **Render equals playback, including randomness.** Round robin and random choices are seeded per instrument and reset
   at transport start (or every bar, user choice); stealing is deterministic; streaming is pulled offline. A freeze, a
   bounce and a live take are identical, which no plug-in sampler can promise.
5. **Auto-sample anything, faster than real time and bit-exact.** Point Exemplar at any Cantus instrument or hosted
   plug-in; render notes × velocities × round robins offline; build the instrument with loops found automatically.
   This is also the honest answer to encrypted libraries: *record what you are licensed to play*.
6. **Record into a zone.** Arm Exemplar, play or sing, and the take lands on the key under the cursor with its root
   detected.
7. **The AI bridge drives the same commands.** Tools such as `import_instrument`, `map_samples(folder, strategy)`,
   `slice_clip(clip, sensitivity) → pattern`, `make_round_robin(takes)`, `describe_instrument` go through the command
   bus with the same undo and audit as a mouse click (ADR 0004). An agent can turn "map this folder of violin notes,
   three velocity layers, loop the sustains" into ordinary, undoable commands.
8. **Voice inspector and honest meters.** Per voice: zone, sample, RR slot, loop pass, envelope stage, stream-buffer
   fill, and why it was stolen; a warning badge on zones transposed beyond what the chosen quality tier handles
   cleanly; the footer shows voices, RAM, disk health and latency.
9. **Tempo and key follow.** Stretch-enabled zones follow the project tempo (Signalsmith Stretch, pre-rolled with
   `seek()`), loops keep their key when the project key changes, and Live-style **similar-sample swap** uses the same
   analysis features as slicing.
10. **One browser and one modulation service.** Preview in project tempo and key; tags shared with every device;
    Exemplar parameters take modulation from the track-level sources of family.md §7.6 (LFO, envelope, MPE, macros);
    a macros-only performance page is the default for presets.

### P2: later

- Scripting for library builders (sandboxed; Rhai or WASM rather than a KSP clone), plus a true-legato authoring
  wizard (transition samples per interval, crossfade offsets).
- Multi-mic mixing with purge-unused-samples; per-zone insert effects.
- Granular, spectral and stretch zones beyond Signalsmith (an in-house WSOLA for cheap monophonic shifting).
- Per-note MPE pitch, pressure and timbre on sample voices.
- Import EXS24, DirectWave, Ableton presets, TX16Wx, Bitwig; export Decent Sampler, Bitwig, MPC `.xpm`; unencrypted NKI
  only after legal review.
- Stem separation into zones; Life-style pattern generation (density, syncopation) from a slice set.
- A General MIDI "player" mode for SF2 banks and MIDI-file playback.

### 9.1 Data model (in `cantus-core`, no I/O)

```text
ExemplarParams                       // an Instrument variant, serialised in the project (schema 13 -> 14 + fixture)
  global: ExemplarGlobal             // voices, steal policy, glide, bend range, velocity curve,
                                     // quality tier, streaming mode, rr_seed, rr_reset (Never | Transport | Bar)
  samples: Vec<SampleSlot>           // the pool, addressed by stable SampleId
  groups:  Vec<ZoneGroup>            // ordered, stable GroupId
  macros:  Vec<Macro>                // performance page
  provenance: Option<Import>         // source format, unmapped opcodes kept verbatim for round trips

SampleSlot { id, source: SampleSource, hash, rate, channels, frames, detected_root, file_loops }
SampleSource = File { path } | Clip { clip_id, take_id, range } | ProjectAsset { asset_id }

ZoneGroup {
  id, name, colour, enabled,
  trigger:  Attack | Release { decay_db_per_s, from_reached_level } | First | Legato | ReleaseKey
  selector: Always | RoundRobin { length } | Random { avoid_repeat } | Keyswitch { key }
          | CcRange { cc, lo, hi } | Layer { axis, index }        // Live's "sample select" as a named axis
  choke: u8, polyphony: Option<u16>, note_polyphony: Option<u16>, self_mask: bool,
  amp_env: Dahdsr + curves, filter: FilterSpec, pitch_env, output: BusRef,
  zones: Vec<Zone>
}

Zone {
  id, sample: SampleId, label,
  root: Pitch, root_cents: f32, keytrack: f32,
  keys: (Pitch, Pitch), vels: (u8, u8),
  key_fade: Fade { in_width, out_width, curve: Gain | Power },
  vel_fade: Fade { .. },
  rr_slot: Option<u8>,
  start, end: FramePos, direction: Forward | Reverse, one_shot: bool,
  sustain_loop: Option<Loop>, release_loop: Option<Loop>,
  gain_db, pan, tune_cents,
  stretch: Option<StretchSpec { mode: Off | Tonal | Beats, follow_tempo, formant }>
}
Loop { start, end, mode: Forward | PingPong | Backward, crossfade_frames, curve, count: Option<u32> }
```

Rules carried over from Cantus: every addressable field is an enum-addressed parameter (`PadField` style) that the
bus can set and undo; IDs never renumber (automation and slice patterns depend on them); the model stores what the
user set, clamped, and nothing derived (the undo-fuzz lesson recorded in `pads.rs`).

### 9.2 Parameter list (automatable, with units)

| Level | Parameters |
|---|---|
| **Instrument** | Volume (dB), Pan, Tune (±24 st, cents), Transpose, Voices (1–256), Steal (released → quietest → oldest), Glide (ms, mode), Bend range (st), Velocity curve (amount, shape), Quality (Draft/Standard/High/Best), Streaming (Auto/RAM/Disk), RR reset (never/transport/bar), Macros 1–8 |
| **Group** | Level, Pan, Tune, Amp DAHDSR (ms) with curve per stage, Velocity → amp (%), Filter type (LP/HP/BP/BR 12/24), Cutoff (Hz), Resonance, Key track, Velocity → cutoff, Filter env amount, Pitch env amount, Release decay (dB/s), Choke, Polyphony |
| **Zone** | Gain, Pan, Tune (cents), Start, End, Loop start, Loop end, Crossfade (ms), Fade widths (keys, velocity steps) |

### 9.3 Voice architecture

- **Pool** allocated when the snapshot is compiled: `voices × 1.5` (sfizz's overflow idea) so stolen voices fade.
- **Note-on:** find candidate zones (keys, velocities, selector state: keyswitch, CC range, RR counter modulo length,
  seeded random with no immediate repeat); compute crossfade gains; allocate one voice per zone; apply choke; enforce
  polyphony with the steal order in §4.2 and a 5 ms minimum fade.
- **Per voice:** a `SampleReader` (RAM slice or stream ring), the resampler state (mip level, phase, history), amp and
  filter envelopes, a Cantus SVF, parameter smoothers, loop state (pass count, direction), and the stretch state when
  enabled. Modulation updates every 32 frames; audio per frame.
- **Note-off:** release the voice; start release-trigger zones at `decay_db_per_s × held_time` below, multiplied by
  the level the attack voice had reached when `from_reached_level` is set.
- **Determinism:** RR counters and RNG live in the allocator state inside the engine and reset at the defined point;
  voice order and tie-breaks are by index; no wall-clock anything.
- **Cost guide (rough):** High quality at 48 kHz is on the order of 30–50 multiply-adds per output sample per channel;
  128 stereo voices ≈ 0.35–0.6 GMAC/s, fine with SIMD; Draft exists for giant sketches.

### 9.4 Streaming

- **Preload per sample** = `max(8192, ceil(r_max × T_budget × fs) + margin)` with `T_budget` 100 ms (SSD) or 250 ms
  (HDD, user setting), `r_max` from the zone's highest key above its root plus bend and modulation range; plus the
  whole of any loop region; plus the stretch pre-roll when stretching is on.
- **Per-voice ring:** three chunks of 4,096–16,384 frames, refilled by a loader thread that receives requests over an
  SPSC queue and decodes (WAV/FLAC) off the audio thread; an LRU cache of decoded chunks lives only on the loader side.
- **Real time:** a voice that runs dry outputs silence for the missing frames and increments `stream_miss`; nothing
  blocks. **Offline:** the render thread waits for fills before each block.
- **Memory release:** buffers the audio thread might still reference are only freed after the snapshot that used them
  comes back over the garbage ring (ADR 0005 rule 1); the current never-evict `SampleCache` rule becomes "evict when no
  live snapshot references it".
- **Loading UX:** preloads first (small), then the instrument is playable; a progress bar in the footer; the voice
  inspector shows which zones are still cold.

### 9.5 The editor layout

```text
+-- header: [chip Violin * track colour] EXEMPLAR  undo redo  A/B  < Solo Violin mf >  bypass  ?  []  ---------+
| tabs: [Play] [Map] [Wave] [Mod] [Sectio]      High quality | voices 34/128 | RAM 412 MB | disk ok | 0.0 ms      |
+--------------+---------------------------------------------------------------+------------------------------+
| GROUPS       | MAP   keys ->                     layer: [RR 1][2][3][4] [mic]| ZONE  Violin_A3_mf_rr2       |
| [x] Sus mf   | vel ^  +--------++--------++--------+                         |  Root   A3      +3 ct        |
| [x] Sus f    | 127    | A2 f   || C3 f   || D#3 f  |   labels stay level,    |  Keys   G#3 .. B3            |
| [ ] Release  |  90    +--------++--------++--------+   elided with "..."     |  Vel    64 .. 100  fade 8/8  |
| [x] Legato   |        | A2 mf  || C3 mf  || D#3 mf |   fades = gradients      |  Gain  -1.5 dB   Pan 0       |
| + group      |   1    +--------++--------++--------+                         |  RR slot 2 of 4              |
|              | [keyboard: sounding voices, zone hit, RR slot, velocity tick] |  Loop  sustain, fwd, 12 ms   |
|              | [table view: Lo K | Hi K | Lo V | Hi V | fades | root | ...]  |  [Find best loop] [Snap 0x]  |
+--------------+---------------------------------------------------------------+------------------------------+
| WAVE  S>----------[ loop >.................< ]-----------<E   [spectrogram] [overlay]  seam error -48 dB     |
|       +--- seam view: before loop end --+--- after loop start ---+   start 0  loop 15 579..32 266  xf 12 ms |
+-------------------------------------------------------------------------------------------------------------+
| footer: Learn | Quality: High | Streaming: Auto | RR reset: transport | help: "Drop files on the keys to map" |
+-------------------------------------------------------------------------------------------------------------+
```

- **Play** is the macros-only performance page (family.md §7.6); **Map** and **Wave** share the screen as above;
  **Mod** is the modulation matrix; **Sectio** is the slicing view (big waveform, slice lines, sensitivity, clusters
  coloured, "Write pattern" button).
- Docked in the inspector at compact density the Map tab shows only the grid and zone panel; floating, everything.
- Keyboard: arrows move the selected zones by a key or a velocity step; Shift-arrows resize; Tab cycles zones; Enter
  edits the focused field; `L` toggles the loop; `S` snaps; `[` `]` step the RR layer.
- Colours from theme tokens only; zone fills carry a text label and a pattern for layers, never colour alone; every
  boundary at 3:1 (family.md §7.4).

### 9.6 Bundled content

ADR 0001 says bundled samples must be **original or CC0**. idea.md §8 says "verified permissive/CC0". The two need
reconciling in an ADR before the first content ships.

| Library | Licence (verified) | Size | Quality and fit | Plan |
|---|---|---|---|---|
| **VCSL** (Versilian Community Sample Library) | **CC0** ("even make commercial software… no credit") | ~5 GB, 4,000+ samples, ~150 instruments; SFZ | Curated for "lightweight sample library creation", mostly whole-tone sampled | **Ship a curated subset** (orchestral percussion, mallets, plucked, a few winds) |
| **VSCO 2 Community Edition** | **CC0** | ~3 GB, 3,000 WAVs, 44.1 kHz, 16/24-bit; SFZ | A chamber orchestra; "natural" and a little loose | **Ship strings/brass/woodwind essentials**; full set as a download |
| **Karoryfer / Versilian libraries** on sfzinstruments (Growlybass, Meatbass, Pastabass, Emilyguitar, Shinyguitar, Black and Blue Basses, Big Rusty/Swirly/Unruly Drums, Frankensnare, Virtuosity Drums…) | **CC0** each | Repos from ~14 MB to 1.2 GB (git sizes) | Characterful, well-scripted SFZ with round robins | **Download packs**; Virtuosity Drums (six mics, CC0) is the best free acoustic kit found |
| **Osiris Piano** (Versilian + Karoryfer, KVRDC 2026) | **CC0** | ~450 MB repo, 16-bit FLAC | A quiet Yamaha C2 at half-stick, soft pedal; two velocity layers, diatonic, three mics; pedal-up and pedal-down sets | A lovely *character* piano, not a general one |
| **Salamander Grand Piano v3** | **CC-BY 3.0** (attribution) | ~730 MB repo (FLAC, 48 kHz/24-bit), 16 velocity layers in minor thirds, release and pedal noises | The standard free concert grand; the SFZ port relies on v2 + ARIA opcodes | **The default piano if the ADR admits CC-BY with an attribution file**; otherwise a one-click download |
| Headroom Piano, DrumGizmo DRS/Muldjord kits, Naked Drums, MTG Solo Sax | CC-BY-4.0 | 0.1–1.25 GB | Good | Downloads with attribution |
| **Sonatina Symphonic Orchestra** | Creative Commons **Sampling Plus 1.0**; its author states some sources' licensing is unknown | — | Popular | **Do not bundle** |
| **99Sounds** | Royalty-free for music, but **"cannot sell or redistribute (even for free)"** the files | — | Good one-shots | **Do not bundle** |
| **GeneralUser GS** (SF2) | Custom permissive licence; its author says he "cannot be 100 % sure" where every sample came from | 261 presets + 13 kits | A widely used free GM/GS bank | Let users load it; do not bundle |
| **Freesound** | Per sound: CC0, CC-BY, CC-BY-NC, legacy Sampling+ | — | Variable | Only CC0 sounds, logged one by one |
| University of Iowa MIS, Philharmonia samples | Iowa: free to use (from memory); Philharmonia: redistribution as samples not allowed (from memory) | — | Raw recordings | Iowa only as source material for our own instruments, after checking terms |

**Recommendation:** ship ~500 MB in the installer (FLAC): a VCSL/VSCO subset, one Karoryfer bass, one guitar, a
CC0 kit, and Pulsus's synthesized drums; offer the rest (full VSCO 2 CE, VCSL, Virtuosity Drums, Salamander if
allowed) as in-app downloads with licence text shown. Longer term, record a Cantus-original piano and a core set.

---

## 10. Test plan hints

Written in the style of the existing null-test harness (`cantus-dsp::nulltest`) and the undo fuzz.

**Resampler**
- **Unity is identity:** ratio 1.0 at an integral phase returns the input bit for bit, in every quality tier except
  Draft's documented behaviour (and Draft too, if it is linear at integral phase).
- **Aliasing at +12 and +24 semitones:** a 44.1 kHz sample holding sines at 5, 10, 15 and 19 kHz, played at +12 and
  +24 semitones into 48 kHz. Every component whose pitched frequency exceeds 24 kHz must vanish; measure the largest
  spectral line that is not an expected partial. Targets: **High ≤ −80 dBFS, Best ≤ −100 dBFS**; record Standard and
  Draft figures as documentation, not as passes. Repeat with a full-band noise burst and compare the energy above the
  shifted Nyquist image.
- **Pitch-down passband:** −12 and −24 semitones: magnitude within ±0.1 dB up to 90 % of the output band.
- **Sweep continuity:** a continuous pitch sweep across a mip-map level change produces no step larger than the
  interpolator's error (no click at level switches; de Soras crossfades between levels).
- **Cost is bounded:** High at +36 semitones costs no more than 1.2× High at +1 semitone (a benchmark, not a
  correctness test).

**Engine**
- **Block-size independence:** render the same MIDI with blocks of 1, 7, 64, 512 and 4,096 frames: bit-identical,
  including note-ons inside blocks, loops, crossfades and release triggers.
- **Determinism of round robin and random:** same seed → same zone sequence; transport restart resets it; offline
  render equals a real-time run; "avoid repeat" never repeats with ≥ 2 candidates.
- **Crossfade laws:** across a velocity crossfade, the sum of the two layers' gains is constant for *gain* curves and
  the sum of squares is constant for *power* curves (±0.01 dB).
- **Voice stealing:** at the polyphony limit, the stolen voice fades over ≥ 5 ms (bound the maximum sample-to-sample
  step), released voices go first, per-group limits hold, choke groups silence only their members.
- **Release triggers:** a note held for *t* seconds gives a release sample `decay × t` dB lower (±0.05 dB);
  `from_reached_level` starts at the attack voice's envelope level.
- **Exact silence for a missing file:** the zone renders zeros bit-exactly, the other zones are unaffected, the project
  loads, the status reports the path, and relinking restores the sound without reloading the project (the Pulsus
  sample-pad test already covers the first part).

**Loops**
- **Seam continuity:** a sine whose loop points sit an exact number of periods apart loops with no discontinuity: the
  largest step at the seam is no larger than the largest step inside the loop. With a deliberately bad loop and a
  crossfade of *n* ms, the step shrinks and the crossfade region matches the chosen curve.
- **Ping-pong turn-around** repeats no sample and skips none; backward loops mirror forward loops exactly.
- **Loop-finder:** on recorded sustains (VCSL/VSCO), the proposed points reduce seam error below −40 dB in most files;
  report the distribution.

**Streaming**
- **Under load:** 128 voices streaming from a mock disk with injected latency (0–200 ms, jittered) and preloads from
  the formula: zero `stream_miss` below the budget, graceful silence above it, audio thread never blocks (assert no
  locks with a debug hook).
- **Pitch-scaled preload:** a zone mapped two octaves above its root survives the budget where a fixed 8,192-frame
  preload would not.
- **Offline equals ideal real time:** offline render of a streaming instrument is bit-identical to an all-in-RAM render.
- **Memory:** RAM use of a 16-bit library stays within 1.1× its native size (not 2× from `f32`).

**Import and export**
- **SFZ corpus:** VSCO 2 CE, VCSL, Salamander, a few Karoryfer libraries and sfzformat's test files parse with an
  unsupported-opcode log; renders match sfizz (BSD-2) references within a tolerance on level (±0.5 dB) and timing
  (sample-exact starts).
- **Round trip:** SFZ → model → SFZ → model is equal; unknown opcodes survive verbatim.
- **Decent Sampler:** volume multiplies across levels and pans add; embedded loop markers win unless overridden.
- **SF2 units:** table tests for timecents, centibels, absolute cents, preset-level additive generators, instrument-only
  generators ignored at preset level, default modulators applied, linked stereo pairs merged only when compatible.
- **Undo fuzz:** every Exemplar command (add/remove/move zones, set fields, loops, slicing, imports) inverts exactly;
  schema 14 fixture opens in the next build.

**Slicing and analysis**
- Onset detection on a labelled drum-loop set: F-measure reported per sensitivity; manual slices survive sensitivity
  changes; a slice pattern still plays the same audio after its markers move.
- Root-key detection within ±5 cents on pitched VCSL/VSCO samples; falls back to names and metadata in the right order.

---

## 11. Sources

**Kontakt / Native Instruments**
- Kontakt product page: https://www.native-instruments.com/products/kontakt
- Kontakt 8 online manual (Classic view: Group Editor, Mapping Editor, Wave Editor, Instrument Options):
  https://docs.native-instruments.com/ni-tech-manuals/kontakt-manual/en/classic-view
- What's new in Kontakt 8: https://docs.native-instruments.com/ni-tech-manuals/kontakt-manual/en/new-in-kontakt-8
- Kontakt 7.6 manual (PDF): https://www.native-instruments.com/fileadmin/ni_media/downloads/manuals/kontakt/KONTAKT_7_6_Manual_en.pdf
- Kontakt 5.7 manual (PDF, foreign-format import): https://www.native-instruments.com/fileadmin/ni_media/downloads/manuals/kontakt/KONTAKT_5_7_Manual_English.pdf
- Kontakt 8 update thread (search excerpt only; page returned 403): https://vi-control.net/community/threads/kontakt-8-updates-current-version-8-13-0.155892/
- Reviews: Sound On Sound https://www.soundonsound.com/reviews/native-instruments-kontakt-8 ;
  MusicRadar https://www.musicradar.com/music-tech/plugins/native-instruments-kontakt-8-review ;
  MusicTech https://musictech.com/reviews/software-instruments/native-instruments-kontakt-8-review/
- KVR forum on HQI (Dec 2020): https://www.kvraudio.com/forum/viewtopic.php?t=558389
- Kontakt Player licensing (search excerpt): https://plugg-supply.net/articles/kontakt-vs-kontakt-player-difference
- NI insolvency and sale: https://www.gearnews.com/native-instruments-insolvency/ ;
  https://musictech.com/news/industry/native-instruments-acquired-by-inmusic-akai-pro-moog-denon/ ;
  https://www.digitalmusicnews.com/2026/05/11/inmusic-acquires-bankrupt-native-instruments/ ;
  April Fools' piece: https://www.gearnews.com/kontakt-ai-native-instruments-tech/
- NCW decoder (Rust, MIT OR Apache-2.0): https://github.com/monomadic/ncw

**UVI Falcon**
- Product page: https://www.uvi.net/falcon ; manual (Falcon 2026): https://cdn.uvi.net/UVIFC_Falcon/manuals/Falcon_manual_en.pdf
- KVR product page: https://www.kvraudio.com/product/falcon-by-uvi
- Falcon 3 release: https://sonicstate.com/news/2023/10/20/uvi-releases-falcon-3-new-expansions/
- MusicTech review: https://musictech.com/reviews/software-instruments/uvi-falcon-3-review/

**Steinberg HALion**
- HALion 7.5: https://synthanatomy.com/2026/08/steinberg-halion-7-flagship-sampler-plugin-gets-an-fm-and-spectral-makeover.html ;
  https://sonicstate.com/news/2026/08/04/halion-7-5-free-update-marks-vst-30th-and-halion-25th/
- Sound On Sound review: https://www.soundonsound.com/reviews/steinberg-halion-7

**Ableton**
- Live 12 instrument reference (Sampler, Simpler, Drum Sampler): https://www.ableton.com/en/manual/live-instrument-reference/
- Live 12 release notes: https://www.ableton.com/en/release-notes/live-12/

**Apple Logic Pro**
- Sampler Mapping and Zone panes: https://support.apple.com/guide/logicpro/sampler-mapping-and-zone-pane-overview-lgsifc8617c2/mac
- SoundFont2, DLS and Gigasampler import: https://support.apple.com/guide/logicpro/add-soundfont2-dls-and-gigasampler-files-lgsifc8653a8/mac
- Sampler memory management: https://support.apple.com/guide/logicpro/sampler-memory-management-lgsidfa7eea2/mac
- Quick Sampler: https://support.apple.com/guide/logicpro/quick-sampler-overview-lgcp5af33756/mac ;
  Flex: https://support.apple.com/guide/logicpro/use-flex-in-quick-sampler-lgcpe3d7978d/mac
- Convert regions to a sampler track: https://support.apple.com/guide/logicpro/convert-audio-regions-samples-a-sampler-lgcpf7c0bf4d/mac

**Image-Line FL Studio**
- DirectWave: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/DirectWave.htm
- Slicex: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Slicex.htm
- FLEX: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/FLEX.htm

**Decent Sampler**
- Versions: https://store.decentsamples.com/downloads/decent-sampler/versions
- Developer guide: https://decentsampler-developers-guide.readthedocs.io/en/latest/ (pages: `the-groups-element`,
  `the-modulators-element`, `introduction`, `topic-how-to-implement-true-legato`)
- Encryption of commercial libraries: https://www.decentsamples.com/2024/01/24/q-is-it-possible-to-extract-the-samples-contained-within-decent-sampler-instruments/
- Synth Anatomy (oscillators, Jan 2026): https://synthanatomy.com/2026/01/decent-sampler-the-popular-free-sampler-plugin-is-now-also-a-decent-synthesizer.html

**TX16Wx, sforzando, sfizz**
- TX16Wx: https://www.tx16wx.com/ ; KVR: https://www.kvraudio.com/product/tx16wx-software-sampler-by-cwitec
- sforzando: https://www.plogue.com/products/sforzando.html
- sfizz: https://github.com/sfztools/sfizz (Config.h, Interpolators.h/.hpp read for design facts); https://sfz.tools/sfizz/
- sfizioso fork: https://github.com/rullopat/sfizioso-player
- KVR sfizz thread (Feb 2022): https://www.kvraudio.com/forum/viewtopic.php?t=560082&start=60

**Serato and XLN**
- Serato Sample: https://serato.com/sample
- XLN Life: https://www.xlnaudio.com/products/life

**Formats**
- SFZ opcodes and headers: https://sfzformat.com/opcodes/ , https://sfzformat.com/headers/ , https://sfzformat.com/versions/ ;
  opcode database: https://github.com/sfz/sfz.github.io/blob/source/data/sfz/syntax.yml ;
  `sample` opcode: https://sfzformat.com/opcodes/sample/
- SoundFont 2.04 specification: https://musescore.org/sites/musescore.org/files/2023-01/sfspec24.pdf
- ConvertWithMoss (formats and conventions): https://github.com/git-moss/ConvertWithMoss ,
  documentation/README-FORMATS.md in that repository
- Multisample format notes (2026, unreviewed community docs): https://github.com/JefroB/Multisample-format-docs

**DSP literature and libraries**
- O. Niemitalo, *Polynomial Interpolators for High-Quality Resampling of Oversampled Audio* (2001):
  http://yehar.com/blog/wp-content/uploads/2009/08/deip.pdf
- L. de Soras, *The Quest For The Perfect Resampler* (2005): http://ldesoras.free.fr/doc/articles/resampler-en.pdf
- J. O. Smith, *Digital Audio Resampling Home Page* (bandlimited interpolation): https://ccrma.stanford.edu/~jos/resample/
- J. Laroche and M. Dolson, "Improved phase vocoder time-scale modification of audio", IEEE TSAP 7(3), 1999.
- W. Verhelst and M. Roelands, "An overlap-add technique based on waveform similarity (WSOLA)…", ICASSP 1993.
- J. Driedger and M. Müller, "A Review of Time-Scale Modification of Music Signals", Applied Sciences 6(2), 2016.
- Z. Průša and N. Holighaus, "Phase vocoder done right", EUSIPCO 2017.
- J. P. Bello et al., "A Tutorial on Onset Detection in Music Signals", IEEE TSAP 13(5), 2005.
- S. Dixon, "Onset Detection Revisited", DAFx 2006.
- S. Böck and G. Widmer, "Maximum Filter Vibrato Suppression for Onset Detection", DAFx 2013.
- A. de Cheveigné and H. Kawahara, "YIN, a fundamental frequency estimator…", JASA 111(4), 2002; M. Mauch and
  S. Dixon, "pYIN", ICASSP 2014.
- Signalsmith Stretch (MIT): https://signalsmith-audio.co.uk/code/stretch/ , https://github.com/Signalsmith-Audio/signalsmith-stretch
- Bungee (MPL-2.0): https://github.com/bungee-audio-stretch/bungee
- Rubber Band licensing: https://breakfastquay.com/rubberband/license.html ; mirror https://github.com/breakfastquay/rubberband (GPL-2.0)
- r8brain-free-src (MIT): https://github.com/avaneev/r8brain-free-src ; rubato (MIT OR Apache-2.0): https://github.com/HEnquist/rubato
- FluidSynth (LGPL-2.1, reference only): https://github.com/FluidSynth/fluidsynth
- crates.io records for `hound`, `claxon`, `lewton`, `symphonia`, `soundfont`, `rustysynth`, `oxisynth`, `sofiza`,
  `rubato`, `signalsmith-stretch`, `ssstretch`, `bungee-rs` (licences as listed in §5.5)

**Content**
- VSCO 2 CE: https://github.com/sgossner/VSCO-2-CE , https://versilian-studios.com/vsco-community.html
- VCSL: https://github.com/sgossner/VCSL , https://versilian-studios.com/vcsl/
- SFZ instruments (Karoryfer, Salamander, Osiris and more): https://github.com/sfzinstruments ;
  Salamander README and LICENSE: https://github.com/sfzinstruments/SalamanderGrandPiano ;
  Osiris Piano: https://github.com/sfzinstruments/Osiris_Piano
- Sonatina Symphonic Orchestra: https://github.com/peastman/sso
- 99Sounds licence: https://99sounds.org/license/
- Freesound licences: https://freesound.org/help/faq/
- GeneralUser GS licence: https://github.com/mrbumpy409/GeneralUser-GS/blob/main/documentation/LICENSE.txt
