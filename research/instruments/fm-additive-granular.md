# FM, additive, spectral and granular: the deep engines

*Researched 2026-09-24 for Cantus. Sources: manufacturer product pages, official manuals (the FM8 PDF manual, the
FL Studio online manual for Sytrus and Harmor, the Ableton Live 12 reference manual, the Omnisphere 3 online manual,
the Absynth 6 online manual, the Serum 2 user guide), the Dexed repository, the published DX7 measurements in the
Music Synthesizer for Android wiki (Raph Levien), Ken Shirriff's die-level analysis of the DX7 chips, the Tomisawa
feedback-FM patent, and the standard literature (Chowning 1973 onwards). Press reviews and KVR threads were used for
opinion. Gearspace refused automated reading (HTTP 403) and was skipped. Everything is paraphrased. Where a fact
comes only from a search-engine excerpt, or is my own estimate, the text says so. Dexed is GPL-3: this report
describes documented behaviour and published measurements only, never its code. Screenshots are the manufacturers'
own, kept in `screenshots/` (git-ignored) for internal reference.*

*Scope: NI FM8, Image-Line Sytrus and Harmor, Ableton Operator, Dexed, Spectrasonics Omnisphere 3, u-he Zebra 3,
NI Absynth 6. Serum 2 and Pigments 7 appear only as comparison points; the wavetable report covers them.*

---

## 0. The short version

1. **FM is a network, while additive and granular are sources.** An FM patch is a graph of 4 to 8 operators.
   Each operator has its own envelope, ratio and key scaling, and the timbre lives in the *connections*. Additive
   and granular engines produce a signal that a filter, an amplifier and the modulation then shape, like any other
   oscillator. The market is organised the same way. Every dedicated FM instrument (FM8, Sytrus, Operator, Dexed)
   is its own device with a matrix or algorithm view. Granular and additive have become **oscillator modes inside
   flagships**: Serum 2's granular and spectral modes, Pigments' harmonic engine, Zebra 3's additive renderer and
   Omnisphere's granular layer process. **Recommendation:** build **one dedicated FM instrument**, proposed name
   **Modulus**, and fold **additive/resynthesis (*Partes*) and granular (*Grana*) into Firmus II as oscillator
   engines**. All of them share DSP kernels in `cantus-dsp`. The argument is in §8.1.
2. **The FM market leader is stale, and that is our opening.** FM8's interface dates from 2006. Its recent updates
   only kept it compatible (Apple Silicon, VST3), and its price has fallen to $99. Its best ideas are still the
   benchmark: a free any-to-any matrix, the DX7 algorithm templates, an Easy page of meta-parameters, a four-corner
   Morph Square, and time/level breakpoint envelopes. Dexed is the faithful DX7, and it is as hard to program as a
   DX7. Operator is the most *legible*, but it stops at 4 operators and 11 algorithms. Sytrus is the most flexible,
   but its matrix knobs **cannot be automated** (its own manual says so).
3. **None of them treats aliasing honestly.** FM8's anti-aliasing "High resolution" mode is global and **is not
   saved with the sound**. Sytrus has separate draft and render quality, and its manual warns that oversampling can
   change how FM feedback sounds, so **the bounce can differ from what you heard**. Operator offers an opaque
   "Tone" knob whose effect "cannot generally be predicted". Nobody shows the user *where the sidebands land*
   relative to Nyquist. Cantus can:
   - measure the bandwidth (Carson's rule) per voice and draw it;
   - save the quality setting with the patch;
   - guarantee that render equals playback, bit for bit.
4. **The approachability tools are all *navigation*, not *understanding*.** Omnisphere's Orb, its Adaptive Global
   Controls and Patch Mutations, FM8's Easy/Morph, and Absynth 6's AI Preset Explorer and Mutator all help a user
   find a sound *without* learning FM. None of them explains *why* a patch sounds the way it does, or turns a
   description into a patch. Cantus's AI command bridge can **program an FM patch from a description** as ordinary
   undoable commands, and **explain an existing one** in words. No competitor can do either.
5. **The DSP is well documented and small.** DX-style FM is phase modulation of sine tables, driven by envelopes
   that run in the log domain. Feedback goes through Tomisawa's two-sample "anti-hunting" average. Yamaha's patent
   and later measurements (the MSFA wiki, Shirriff's die analysis) pin down the envelope rates, level tables and
   feedback behaviour with numbers. **An 8-operator engine costs a few hundred flops per voice-sample.** The hard
   part is alias control at high indices, and a patch model that stays editable.
6. **Additive cost scales with pitch, not with the partial-count knob.** A voice only needs the partials below
   Nyquist. At 48 kHz that is 872 partials at A0, 109 at A3 and 54 at A4. Harmor (up to 516) and Zebra 3 (up to
   1024) quote the ceiling. Two ways to render:
   - an oscillator bank: exact and modulatable per sample, with cost linear in the number of partials;
   - inverse-FFT synthesis: roughly 100× cheaper per partial, but parameters update only once per hop.

   Cantus should use the bank, culling above Nyquist and computed across several lanes at once with SIMD. The
   inverse FFT belongs in the analysis path.
7. **Granular engines differ mainly in scheduling and randomness, and none documents determinism.**
   - Omnisphere exposes unitless 0–1 controls.
   - Absynth gives density 1–32, grains up to 500 ms and jitter 0–100.
   - Serum 2 offers three density modes, 10 or more window shapes and per-grain random offset, direction, pitch,
     length, pan and level.

   Cantus's rule of block-size independence and bit-identical renders requires two things:
   - grains are scheduled on the sample clock from the start of the stream;
   - each voice has its own seeded random number generator.

   No competitor promises either.

---

## 1. Products and versions (September 2026)

| Product | Latest version, date | Price | Formats | Source |
|---|---|---|---|---|
| **NI FM8** | 1.4.6 (KVR lists 1.4.6; 1.4.5 added Apple Silicon and VST3; see note) | **$99** (NI shop, Sep 2026) | VST, VST3, AU, AAX (Win/macOS). **No CLAP** | [NI](https://www.native-instruments.com/en/products/komplete/synths/fm8/), [KVR](https://www.kvraudio.com/product/fm8-by-native-instruments) |
| **Image-Line Sytrus** | 2.6.x inside FL Studio 2025 (KVR: 2.6.73; no date) | In FL Studio Producer and above; KVR lists $149 separately (possibly stale) | FL-native; VST (KVR); AU through Image-Line's plug-in installer (Splice). CLAP **not confirmed** | [FL manual](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Sytrus.htm), [KVR](https://www.kvraudio.com/product/sytrus-by-image-line) |
| **Image-Line Harmor** | 1.3.8 (KVR) | $119 (KVR; 2022 price history) | VST, AU (Splice). CLAP not confirmed | [FL manual](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Harmor.htm), [KVR](https://www.kvraudio.com/product/harmor-by-image-line) |
| **Ableton Operator** | Live 12 (the current manual) | Bundled with Live; the edition split was not re-checked | Live only | [Live 12 manual §31.9](https://www.ableton.com/en/manual/live-instrument-reference/) |
| **Dexed** | 1.0.1 (29 Nov; the page shows no year, but it follows 0.9.9 of 16 Nov, so probably 2025); nightlies to Jul 2026 | Free, **GPL-3** (its MSFA core is Apache-2.0) | VST3, AU, CLAP, standalone; Win/macOS/Linux | [GitHub](https://github.com/asb2m10/dexed), [releases](https://github.com/asb2m10/dexed/releases) |
| **Spectrasonics Omnisphere 3** | 3.0, **21 Oct 2025** (point release not published on the fetched pages) | **$499**; upgrade $199 | VST2, VST3, AU, AAX, standalone. **No CLAP** | [SOS news](https://www.soundonsound.com/news/spectrasonics-omnisphere-3-arrives), [product page](https://www.spectrasonics.net/products/omnisphere/) |
| **u-he Zebra 3** | **3.0.2 (rev. 22175), 15 Jul 2026**. 3.0 shipped 20 Apr 2026 after a public beta from Dec 2025 (search excerpt) | **€249** | **CLAP**, VST3, AU, AAX; Win/macOS/**Linux** | [u-he](https://u-he.com/products/plug-ins/zebra3/) |
| **NI Absynth 6** | 6.0 on **9 Dec 2025**; 6.1 around Apr 2026 (audio modulator, LFO retrigger, undo/redo) | **$199**; upgrade from 2–5 $99 | VST3, AU, AAX. No CLAP | [NI](https://www.native-instruments.com/en/products/komplete/synths/absynth-6/), [Sonicstate](https://sonicstate.com/news/2025/12/09/absynth-returns/), [gearnews](https://www.gearnews.com/absynth-6-native-instruments-synths/) |
| *Xfer Serum 2* (comparison) | 2.0.18 (manual 1.0.3, 27 Apr 2025) | — | VST3, AU, AAX (manual cover) | Serum 2 User Guide (support.xferrecords.com) |
| *Arturia Pigments 7* (comparison) | 7 | $199 | VST, VST3, AU, AAX; CLAP not mentioned | [Arturia](https://www.arturia.com/products/software-instruments/pigments/overview) |

**Notes.**
- The claim that FM8 1.4.5 added Apple Silicon and VST3 comes from a search excerpt of a rekkerd.org news item.
  The KVR page confirms 1.4.6 and the formats.
- The Absynth 6 release date was confirmed from the published metadata of the SOS and Sonicstate news items
  (2025-12-09). The 6.1 contents come from gearnews (2026-04-20).
- Only Zebra 3 and Dexed ship CLAP, which is Cantus's primary plug-in format (ADR 0001).

---

## 2. Screenshots saved

All files are official images downloaded with `curl` into `screenshots/`: NI product galleries, the
Ableton manual, the FL Studio online manual, the u-he product page, the Spectrasonics interface tour and NI's
Absynth tour. Every file was opened and checked. The u-he WebP images were converted to PNG. Background-only
decoration images were discarded.

| File | What it shows, and what to learn from it |
|---|---|
| `fm-01-fm8-matrix.png` | FM8 Expert › Env with the **FM Matrix** at the right. Operators A–F, X and Z sit on a diagonal of pale tiles on a slate grid. A connection is drawn as an elbow line with a **white number box** (modulation amount). The bottom rows are the output level and the pan (shown as `‹-81›` chips). At the centre, eight stacked **multi-breakpoint envelopes** (A–F, X, Z, plus P for pitch), one of them expanded with red sustain/loop markers and a parameter strip (BP#, Mode, Abs T, Rel T, Level, Slope). |
| `fm-02-fm8-expert-ops.png` | Operator Z (the filter) page: filter curve, two filters (Cutoff, Reso, Mode, Spread), routing Mix/Par-Ser, an amp-mod source grid, and a dense matrix with nearly every cell used. The matrix reads well because the numbers are large and white on dark. The knobs and envelope around it are small and grey. |
| `fm-03-fm8-master.png` | Master page: output and input faders, polyphony 8, unison 3 with Dynamic, master tune, transpose −8, portamento, arpeggiator BPM/step, and **Quality: Analog 14 / Digital 0** sliders. A MIDI CC assignment list. |
| `fm-04-fm8-easy-morph.png` | **Easy/Morph page**. Meta-knobs: LFO (rate, vibrato, timbre, tremolo); Timbre (Harmonic, Detune, Brightness, Env Amt, Velocity); Timbre and Amplitude ADSR as **bipolar offset faders**; an effects on/off list. The **Morph Square** has four corner names (Full Range, Bandpass, Hipass, Double Bandpass), a red handle, a **dotted random cloud**, Rnd Amt, "Normalize Timbres", Random Seed and Apply/Reset. |
| `fm-05-operator-main.png` | Ableton Operator. The **shell** at the left has four oscillator rows (Coarse, Fine, Fixed, Level), each with a **colour tile** (D orange, C violet, B teal, A yellow). The dark central display shows the selected oscillator's envelope (Attack/Decay/Release/Initial/Peak/Sustain, Loop) and oscillator (Wave User, Feedback, Repeat, Phase R, Osc<Vel Q). The right shell holds LFO, Filter (12/24, Clean), Pitch Env/Spread/Transpose, and Time/**Tone**/Volume. |
| `fm-06-operator-algorithms.png` | The global display. The **11 algorithms are drawn as tiny stacks of the same coloured squares**; the selected one is highlighted. Below it, a MIDI routing table (Velocity, Key, Aftertouch, Pitch Bend, Mod Wheel → two destinations each), then Voices, Retrigger, **Interpolation, Antialias** and Pan/Key/Rnd. |
| `fm-07-operator-partials.png` | The harmonics editor: cyan **bars for harmonics** and a **16/32/64** switch. The selected shell row is outlined, and the display follows the selection. |
| `fm-08-sytrus-main.png` | Sytrus main page. The **9×9 knob matrix** (rows 1–6 and F1–F3; columns 1–6, PAN, FX, OUT; orange knobs are active; a mute glyph marks OUT; FM/RM tabs) at the right. At the left: master Vol/LFO/Pitch, Vol and Filter ADSR offsets, unison (ORD, PN, VL, PT, SB, PH, EV), an **X/Y modulation pad**, a 3-band EQ, **Quality with DRAFT/RENDER columns** (HQ envelopes, oversampling), and a patch comment box that explains the X/Y assignments. |
| `fm-09-sytrus-operator.png` | Operator page: a waveform thumbnail, shape sliders (SH, TN, SK, SN, FL, NS), CENTER/DECLICK/**BAND LIMIT**/PLUCK, Hz offset and ratio readouts, PHASE with GLOBAL, VOL, and articulation target tabs (PAN VOL MOD PITCH PHASE DAMP **OSC**). The harmonic editor has amplitude bars above and phase bars below. |
| `fm-10-sytrus-envelopes.png` | The articulation editor, annotated by Image-Line: target tabs, source tabs (ENV LFO KEY M VEL M MOD X MOD Y RAND UNI), a spline envelope with control points, **tension handles** and D/S section markers, ATT/DEC/SUS/REL scaling knobs, and TEMPO/GLOBAL. |
| `fm-11-harmor-main.png` | Harmor. A dense maroon panel: Timbre (two thumbnails with a mix mode), Blur, Tremolo, Prism, Harmonizer, Unison, Pitch/Vibrato/Legato, then a vertical "FILTERING SECTION" with two filters (adapt, kb track, width, FREQ, RES), Pluck, Phaser and EQ. Below: Global, **A/B part** selector, an X/Y/Z pad, and a large envelope editor with ENV/IMG/FX/ADV tabs. At the right, a **3-D partial visualiser** in magenta lines. |
| `fm-12-harmor-image-resynth.png` | The IMG tab, annotated by Image-Line. A **spectrogram-like image of partial gain over time**, with start time, playhead, loop start/end, crossfade and loop gain handles. At the right: smooth, time, speed, sharp, level/freq scale and formant. |
| `fm-13-zebra3-spline-editor.png` | Close-up of Zebra 3's **unified spline editor**: a 16×4 grid, snap, guides, a pen/brush/transform tool palette, and one spline curve that shapes both waveforms and MSEGs. |
| `fm-14-zebra3-generator-rack.png` | Official composite of the generator modules. **FMO** draws its own signal flow *inside the module*: Input → FM Index, Carrier, Ratio and Feedback, with arrows between the knobs. Also Osc (Renderer: Wavetable, Resolution 800 Hz, Curve Math, Unison), Noise, Exciter, Modal (16 modes, a partials plot, A/B profile CSVs), Comb, EQ, Filter (with a drawn response and a model picker), 4in1 vector pad, 4in4 scan, Folder, Distortion and Ring. |
| `fm-15-zebra3-main.png` | Zebra 3 main window. **Modules appear only when added**, outlined in their type colour (Noise/Osc in violet, Filter in yellow-green, Env in blue). The four-lane grid (Noise 1 → Osc 1 → Filter 1 → Comb 1 → Filter 2) sits in the centre with Main/Pan/Envelope/Volume per lane. At the bottom: tabs for editor, Perform, FX, Matrix, OSC 1–4, MSEG 1–4 and Mapper 1–4; the oscillator's spline with a curve-morph strip; osc FX slots (Tone Works, Sync); matrix slot cards; and a **source strip** (PW MW PRS VEL KEY … E1–E4 L1–L4 MA1–4). |
| `fm-16-omnisphere3-layer-main.jpg` | Omnisphere 3 layer A page. Pitch, a Modulation quick slot, **Synth Osc** (a waveform in a circle, "SawSquare Fat"), filter, LFOs 1–8 and envelopes. There is a **grid of eight process buttons** (Wave, Unison, FM, Ring Mod, Freq Shift, Waveshaper, Harmonia, Granular); **a cyan underline marks the ones in use**. Quadzone, the layer A–D mix faders at the bottom, and the patch browser with Mutate at the left. |
| `fm-17-omnisphere3-granular-zoom.jpg` | Oscillator Zoom › Granular. A waveform with **grain positions drawn as coloured vertical lines** (the colour encodes the grain's pitch), a position slider, Speed/Position mode, and knobs for Grain Depth, Intensity, Smoothing, Spread, Detuning, Pitch Grains, Interval and Gliding, plus Direction and Mode. |
| `fm-18-omnisphere3-orb.jpg` | **The Orb**: a large circle with concentric rings, a draggable glowing puck, REC / Song Pos / Motion / Clear, **DICE**, Depth, Inertia and Attractor. |
| `fm-19-absynth6-explorer-mutator.png` | Absynth 6 browser. The **AI Preset Explorer** is a map of thousands of dots coloured by type (Bass, Bowed Strings, …), clustered by similarity. The **Mutator** strip has Mutation Amount, Random Amount, Mutate and Retry, and names the result ("Vintage Lead M 4"). Eight macro knobs sit below. |
| `fm-20-absynth6-envelopes.png` | Absynth 6 envelope page: a list of named envelopes with **thumbnail curves**, "Master Envelope Macros" 1–4, and three stacked large envelopes (a smooth looping amp shape, a noisy "sample jump" curve, a sawtooth mix envelope) with a pink selected breakpoint, a Grid of 1/16, and Sync, LFO and S&H toggles. |

**Measured label contrast** (WCAG, from the screenshots; ±0.2 because of JPEG/PNG scaling):

| Label | Background → text | Ratio | Verdict for 12 px text |
|---|---|---|---|
| FM8 knob label "Cutoff" | #c5ced2 → #313334 | 7.9:1 | pass |
| FM8 knob **value** under the label | #c5ced2 → #7d8385 | **2.4:1** | **fail** |
| FM8 matrix amount box | #ffffff → #242933 | 14.6:1 | pass |
| Operator shell label | #9b9fa9 → #10121a | 7.1:1 | pass |
| Operator display value (cyan on dark) | #1f232d → #00e5f6 | 10.1:1 | pass |
| Sytrus label "VOL" | #2e3234 → #a8afb4 | 5.8:1 | pass |
| Harmor label "start" | #573a41 → #f7f5f4 | 9.3:1 | pass |
| Omnisphere label "CUTOFF" | #505058 → #dddee2 | 5.9:1 | pass |
| **Zebra 3 label "Tune"** | #474546 → #a3a1a0 | **3.7:1** | **fail** |
| Absynth 6 list item | #17222b → #7a8683 | **4.3:1** | borderline fail |
| Absynth 6 macro label | #121a20 → #abb6ae | 8.4:1 | pass |

The legibility problems are the same as the FabFilter family's: values set dimmer than labels (FM8), and low-contrast
grey-on-grey labels in the new flat dark style (Zebra 3, Absynth 6).

---

## 3. Anatomy of each product

### 3.1 NI FM8 ([manual](https://storage.googleapis.com/pdf-guides/FM8-book_EN_ebook.pdf))

- **Operators.**
  - Six waveform operators, A–F. Each has **32 waveforms**, where the DX7 had sine only.
  - Two special operators:
    - **X** adds noise, then runs a saturator (asymmetry and limit) on noise plus input, then applies the envelope.
    - **Z** is two 2-pole multimode filters with cutoff spread, continuous series/parallel and output mix.
  - The **audio input** is also a node in the matrix, so FM8 can frequency-modulate external audio.
- **FM Matrix.**
  - Any operator can modulate any other, including itself and "backwards", so feedback loops of any length are
    possible.
  - To connect two operators, you drag at the intersection of two imaginary lines; the value box shows the amount,
    and double-clicking removes the connection.
  - The bottom row holds pan, from −99 to 100.
  - There are templates for every DX7 algorithm, plus up to **64 user templates**.
- **Frequency.**
  - Ratio is a four-decimal multiple of the note (1.0030 beats against the other operators, and the beat speeds
    up with pitch).
  - Offset is in Hz, so the beating stays constant across the keyboard.
  - With Ratio = Offset = 0, the operator becomes a static waveshaper whose phase is set by a negative offset.
  - Each operator has Key Sync or Free Run, a pitch-envelope toggle and an Invert Waveform switch (which matters
    most with feedback).
- **Envelopes.**
  - **Time/level**, not the DX7's rate/level. The manual explains why: the same *rate* takes different times for
    different distances, which confused users.
  - Unlimited breakpoints (added by right-click), a slope handle per segment, and sustain markers.
  - **Extra breakpoints between the sustain markers make a loop.**
  - Key scaling and velocity scaling of all times, tempo-sync snapping, and Sustain/Release mode switches.
  - A retriggered envelope restarts from its current level, so it doesn't click.
- **Key scaling.** A breakpoint curve of level against note for each operator (0 dB at the top, lowered where it
  should fall off). The same page has 12-note microtuning (−99 to +100 cents) and an octave stretch.
- **Master page.**
  - Polyphony, unison (voices, detune, pan, dynamic) and portamento.
  - **Analog**: random variation between voices.
  - **Digital**: bit depth down to and below the DX7's 12 bits.
  - **High resolution** "doubles the resolution of internal computations" to reduce aliasing in FM, the X
    saturator and the drive effects. It is **global and not saved with the sound**.
- **Easy/Morph page.**
  - Bipolar *meta-parameters*: Harmonic (shifts the ratios), Detune, Brightness (scales the matrix amounts),
    Envelope Amount, Velocity, Timbre and Amplitude ADSR offsets, LFO and output.
  - **Apply** bakes the offsets into the patch and re-centres the sliders.
  - The **Morph Square** interpolates between four timbres: the matrix and operator settings are morphed, but the
    envelopes and modulation are not, and the effects morph only by their amount.
  - A random cloud with its own seed.
- **Other:** 12 effects in a rack, an arpeggiator with 48 templates, a Spect page (overtone display), and DX/TX
  SysEx import.

### 3.2 Image-Line Sytrus ([FL manual](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Sytrus.htm))

- **Six operators and three filters, routed by a 9×9 knob matrix.**
  - Rows are targets and columns are sources; the diagonal is self-feedback.
  - A negative value inverts the modulation phase.
  - Extra columns set **PAN, FX send and OUT** for every operator and filter, and filter rows route operators into
    filters.
  - There are separate **FM and RM** (ring/amplitude) pages.
  - **Matrix knobs cannot be automated.** The manual's workaround is to automate the operator's VOL instead.
  - Blocks in the processing diagram are evaluated "at the same time". My reading: a cycle between two operators
    is resolved with a one-sample delay.
- **Operator.**
  - Shape morph from sine through triangle and pulse to saw, plus tension, skew, sine-shaper, pre-filter and noise.
  - DC centre, declick, **band limit**, and **plucked** mode with damping.
  - Hz offset plus ratio.
  - A **harmonic editor of up to 128 harmonics** with amplitude and phase, and Gibbs attenuation or removal.
- **Articulation: one uniform model for every target.**
  - Targets: PAN, VOL, MOD, PITCH, PHASE, DAMP and OSC.
  - Each target can use ENV, LFO, **KEY M** and **VEL M** mapping curves, MOD X/Y, **RAND** (a random distribution
    curve) and **UNI** (a value for each unison voice).
  - Spline envelopes have unlimited points, five segment types, and sustain/loop/decay markers.
- **Unison.** Order, pan, volume, pitch, sub-octave, phase and envelope variation, each shaped per voice by
  mapping curves.
- **Filters.** A multimode SVF, eight RBJ types, a "Mango" low-pass and a phaser (order 1–10); a waveshaper per
  filter.
- **Effects and quality.**
  - Chorus, three delays and a reverb; a 3-band EQ.
  - **Quality has separate columns for draft (real time) and render**: HQ envelopes and an oversampling factor.
  - The manual's patch tips:
    - prefer *band-limit* to oversampling, because 4× oversampling costs 4× the CPU;
    - reduce modulator levels on high notes with key mapping to avoid aliasing;
    - keep at least 3 operators as pure sines, because they are shared (cheaper).

### 3.3 Image-Line Harmor ([FL manual](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Harmor.htm))

- **An additive engine that works on "a table of frequency and amplitude data"** instead of audio. It has **up to
  516 sine partials per note, per unison voice**, adjustable from 12 to 516.
  - Precision modes: Average, High, Perfect.
  - Resynthesis or image precision: Generic, 255 partials, or High, 516.
  - The manual doesn't say whether rendering uses a sine bank or an inverse FFT. A KVR developer *speculated* that
    it uses time-domain sines with recurrences; that comes from a search excerpt only and is unverified.
- **Two identical parts, A and B**, with a mix between them (for a stereo sample, the A/B mix works as mid/side).
- **Subtractive metaphors applied to partials:**
  - Timbre 1 and 2, drawn or analysed from audio, mixed with Fade, Subtract, Multiply, Max, Min or Pluck.
  - Two filters with resonance *shapes*, adaptive width and self-oscillation.
  - **Harmonizer**, which clones harmonics at octave or harmonic offsets.
  - **Prism**, which moves partials away from integer ratios, making them inharmonic.
  - **Pluck**, a decay time that depends on frequency.
  - A **phaser on the partial table** rather than in the time domain.
  - **Blur**, which smears partials in time and in frequency.
  - Tremolo, and unison of 1–9 voices.
- **Resynthesis and image synthesis.**
  - You drop a WAV, MP3, OGG, REX or an image onto it.
  - In images, vertical is partial index (one pixel per partial; 516 px high plays all of them) and horizontal is
    time.
  - Two data planes, **gain and pitch**, which can be edited in any image editor.
- **Articulators, as in Sytrus, plus *harmonic mappings*:** functions of partial index for level, phase,
  protection and clipping, and the filter, resonance and pluck shapes. There are also held-index (chord note
  order), unison-index and random mappings.
- **Time-domain effects after synthesis:** distortion, chorus, delay, reverb and a Maximus-based compressor.

### 3.4 Ableton Operator ([Live 12 manual §31.9](https://www.ableton.com/en/manual/live-instrument-reference/))

- **Four oscillators A–D and 11 fixed algorithms**, chosen by clicking icons. The algorithm is an ordinary
  parameter: it can be automated and modulated.
- **Waves.**
  - Sine, Sine 4 Bit, Sine 8 Bit, Saw D and Square D.
  - Saw, square and triangle are **resynthesised with a stated number of harmonics** ("Square 6"). Fewer harmonics
    alias less at high pitch.
  - Two noises.
  - **User** waves are drawn as bars over **16, 32 or 64 harmonics**. **Repeat** extends the drawn pattern upward
    with a fade, and Normalize keeps the level constant.
- **Frequency.**
  - Coarse (integer ratio) and Fine.
  - **Fixed** mode uses Freq × Multi and goes down to 0.1 Hz.
  - **Osc < Vel** shifts the frequency with velocity, optionally quantised to whole ratios.
- **Feedback** exists only on an oscillator that no other oscillator modulates.
- **Seven envelopes** (four oscillators, filter, pitch, LFO).
  - Initial, Peak and Sustain levels with Attack, Decay and Release times.
  - Oscillator attack is linear; decay and release are exponential.
  - Loop modes: **Loop, Beat, Sync** (quantised to the 16th) and **Trigger** (ignores note-off).
  - A global **Time** scales every envelope, plus Time < Key.
- **Other sections.**
  - An LFO that runs at audio rate, "a fifth oscillator".
  - Filters co-developed with Cytomic, in five circuit models plus a Morph filter, followed by a shaper.
  - Up to 32 voices.
  - **Spread** uses two detuned voices panned left and right.
- **Aliasing:** an **Antialias** mode, on by default, with the algorithm undocumented. **Interpolation** can be
  turned off to save CPU. **Tone** controls high-frequency content; its effect "depends on the sound and cannot
  generally be predicted".

### 3.5 Dexed ([repository](https://github.com/asb2m10/dexed))

- **A DX7 model on the MSFA engine** (Raph Levien's Music Synthesizer for Android, Apache-2.0), wrapped in a GPL-3
  plug-in.
- All **144 DX7 parameters**, a **cartridge manager** with .syx import and export, and several engine modes,
  including "Mark I" and OPL-series behaviour.
- MIDI learn, MTS-ESP/SCL/KBM microtuning, MPE, portamento (0.9.9), operator mute switches, and UI scaling.
- It is the reference for *fidelity*. The UX is the DX7 panel: six operator strips and an algorithm display.

### 3.6 Spectrasonics Omnisphere 3 ([online manual](https://support.spectrasonics.net/manual/Omnisphere3/30/en/), [product page](https://www.spectrasonics.net/products/omnisphere/))

- **Structure.**
  - **8 parts** in a multi; **4 layers per patch** (A–D).
  - Each layer is a synth oscillator (638 DSP waveforms, **each one a morphing wavetable**) or a sample soundsource
    (including the user's own audio).
  - **Up to 20 oscillators per patch**, because Harmonia adds 4 oscillators per layer.
  - 8 LFOs and 12 envelopes; a Flex-Mod matrix with 48 slots; 69 filter types; 93 effects.
- **Per-layer processes** (the eight buttons in fm-16):
  - **FM**: the modulator can be any of 500+ wavetables. Frequency is a 0–1 slider with notches at .5, 1:1, 1:1.5
    and 1:2; the lowest settings give LFO-like vibrato. Depth has Normal and **Boost** ranges; there is also
    Shape, Symmetry and FM Sync. It is **a single modulator, not an operator network.**
  - Ring mod, **polyphonic dual frequency shifter**, waveshaper, **Harmonia** (4 extra oscillators per layer) and
    unison.
  - **Granular**:
    - Mix; Gain from −24 to +10 dB.
    - **Grain Depth** (number of grains, which costs CPU) and **Intensity** (grain size: higher means shorter
      grains and faster transitions).
    - Smoothing (crossfade between grains), Spread (stereo) and Detuning.
    - **Pitch Grains** (share of transposed grains), **Interval** 1–12 semitones, Gliding, and Direction (Up, Down,
      Up *or* Down, Up *and* Down).
    - Mode Normal or **Wild**, and a **Legacy** algorithm for version-1 patches.
    - **Speed** mode (auto-scan forward or backward) or **Position** mode (choose the region, such as "only the
      pluck").
    - A matrix-only granular trigger.
    - **Every control is a unitless 0.000–1.000 value.**
- **New in version 3:**
  - **Quadzone** crossfades or switches layers by a fader, by note or by velocity.
  - Oscillator Drift, and new glide modes.
  - **Adaptive Global Controls**: Tone, Ambience, Filter, Envelope, Vibrato and Unison, which "analyse any
    patch".
  - **Patch Mutations**, from Subtle to Extreme, each saved automatically.
  - MPE, **300+ hardware profiles** and Adaptive MIDI Scaling.
- **The Orb** ([manual](https://support.spectrasonics.net/manual/Omnisphere3/30/en/topic/orb-index)).
  - The **angle picks which "scene" of parameter changes applies; the radius sets how strong it is** (the centre
    is the original patch).
  - **Dice** generates new sets of changes; **Depth** scales them; **Inertia** lets the puck roll on after release;
    **Attractor** swings it like a non-linear pendulum.
  - Recording synced to the host, automation, and use as a mod-matrix source. There is one Orb per part.

### 3.7 u-he Zebra 3 ([product page](https://u-he.com/products/plug-ins/zebra3/))

- **A grid of four lanes, with an adaptive interface.** Modules appear only when added and are arranged by drag and
  drop.
- **Four main oscillators** drawn as **splines**:
  - rendered either as a wavetable (up to 16× unison), or as **additive (up to 1024 partials, which can be
    inharmonic)**;
  - 20 oscillator effects, such as spectral decay, sync and phase remapping.
  - The screenshot shows Renderer, *Resolution 800 Hz* and *Curve Math: Fast* fields. Their exact meaning is
    undocumented on the product page.
- **Four FM oscillators, each a modulator/carrier pair.** Each accepts any rack signal as a third operator, so
  algorithms are built freely.
  - Dubspot describes it as through-zero FM.
  - The FMO panel carries "DX" labels next to Input, Index and Feedback; their meaning is unverified.
- **Other generators:** noise, exciters, **modal resonators** (profile CSVs) and combs, which enable physical
  modelling.
- **Processing:** 13 filter models with 105+ responses, vector (XY) and scan mixers, a wavefolder, ring modulator
  and distortion.
- **Modulation:** 4 ADSR, 4 LFO, **4 MSEG** (up to eight morphable curves each), 4 Mappers (step sequencers or
  transfer curves) and 4 Mod Math modules. **Pitch, gate and trigger are first-class signals.**
- **Effects:** an FX grid with per-voice sends, including *TextureVerb*, a granular effect.
- **Format and library:** CLAP, VST3, AU and AAX on macOS, Windows and Linux; MPE, MTS-ESP; 1,200+ presets.

### 3.8 NI Absynth 6 ([manual](https://docs.native-instruments.com/ni-tech-manuals/absynth-6-user-guide/en/patch-page), [product page](https://www.native-instruments.com/en/products/komplete/synths/absynth-6/))

- **Channels.** Three oscillator channels, A–C. Each has a source module and two inserts. The Main channel has two
  inserts and an effect.
- **Source modules:**
  - Single, Double.
  - **FM**: Main and Mod oscillators, an FM Index, and frequency modes Transpose, Ratio, Hz or Note.
  - Ringmod.
  - **Fractalize**: the waveform is copied into itself; Iterations 2–7, Warp 0–1, Amount.
  - **Sync Granular**: density 3–8, diffusion, balance; formant-like.
  - **Granular Engine**: density **1–32** grains, grain size **up to 500 ms**, time, frequency and amplitude
    jitter 0–100, and Time 50–200 % independent of pitch.
  - Sample.
- **Envelopes:** a 68-point system with loops, sync, S&H and a per-envelope LFO.
- **Browsing and variation:**
  - Master Envelope Macros; 16 macros.
  - The **AI Preset Explorer**, a similarity map.
  - The **Mutator** (mutation amount, random amount, a timeline of results).
- **Effects:** the Aetherizer and Cloud Filter (granular textures).
- **Output and expression:** surround up to 8 channels; MPE and polyphonic aftertouch; MTS-ESP.
- **6.1** added an audio modulator, LFO retrigger and **undo/redo**.

### 3.9 Comparison points: Serum 2 and Pigments 7

- **Serum 2 granular** (user guide, pp. 85–103):
  - **Scan** rate of ±200, 400 or 800 %, key-tracked or tempo-synced; 0 freezes and negative values play backwards.
  - **Density** in three modes: rate in Hz, tempo-synced, or a fixed *number of grains*. Options for a "quick
    start" (full density at note-on) and a **max-grains CPU cap** that skips spawns.
  - **Length** in ms, synced, or relative.
  - **10+ window shapes**: Hann, Gaussian, Sinc, Tukey, ExpDec, ExpDec Rev, triangle, trapezoid variants and others.
    Each has an amount and a skew, both randomisable per grain.
  - **Per-grain randomisation** of offset, direction, pitch, length, pan and level.
  - Unison voices spawn together or offset in time.
  - A loop-grain mode, and an X/Y pad whose Y axis can target any grain parameter.
- **Pigments 7:** lists granular and a "harmonic" additive engine among six engine types; the product page gives no
  numbers.
- These are the parity bar for *Grana* in Firmus II.

---

## 4. The DSP

### 4.1 FM versus phase modulation

- **Chowning's formulation.** The paper ([Chowning 1973, JAES 21(7)](https://yamahasynth.com/wp-content/uploads/images/fm_synthesispaper-2.pdf);
  US patent 4,018,121) writes the output as
  `y(t) = A(t)·sin(2π f_c t + I(t)·sin(2π f_m t))`.
  - The sidebands sit at `f_c ± k·f_m`, with amplitudes given by the Bessel functions `J_k(I)`.
  - Written this way, the formula is *phase* modulation. For a sinusoidal modulator it has the same spectrum as
    true FM with peak deviation `Δf = I·f_m`; only the sideband phases differ.
- **Why every DX-style engine uses PM** (Yamaha's hardware adds the modulator output to the sine table's phase
  index; [Shirriff, part 4](http://www.righto.com/2021/12/yamaha-dx7-chip-reverse-engineering.html)):
  1. **No pitch drift.** A modulator with DC (feedback, a non-sine wave, an envelope's asymmetry) *detunes* the
     carrier under true FM, because the integral of the DC accumulates. Under PM it only shifts the phase.
  2. **Timbre is constant across the keyboard.** In PM the index is the parameter. In FM with a fixed deviation,
     the index falls as `f_m` rises, so high notes get duller unless the deviation tracks the key.
  3. **Stacks and feedback are just sums into a phase.**
- **True linear FM** (through zero) is still useful: modulating a fixed-frequency operator, or audio-rate FM of
  non-sine sources, as in Zebra's "through-zero FM" or modular-style patches.
  - In a digital phase accumulator, going through zero is free: the increment can go negative.
  - The cost is DC-dependent drift, so a **DC blocker** belongs on the modulation input.
  - **Cantus: default PM; offer FM (DC-blocked), RM and AM as per-input modes.**
- **Modified FM (ModFM; Lazzarini & Timoney, JAES 2010).** It multiplies the carrier by `exp(k·cos ω_m t − k)`.
  - The sideband amplitudes become modified Bessel functions `I_n(k)` that fall monotonically, so there are no
    spectral holes and no negative-frequency reflections.
  - It is much easier to band-limit, because `k` bounds the bandwidth.
  - It is a cheap operator mode that FM8, Sytrus and Operator don't have. Proposed for P1.

### 4.2 Operators, algorithms and evaluation order

- **The DX7 hardware** ([MSFA wiki: DX7 hardware](https://github.com/google/music-synthesizer-for-android/blob/master/wiki/Dx7Hardware.wiki); [Shirriff](http://www.righto.com/2021/12/yamaha-dx7-chip-reverse-engineering.html)):
  - one operator circuit, time-shared across **96 slots** (6 operators × 16 voices) at **49,096 Hz**;
  - a **32-algorithm ROM** (96 nine-bit entries);
  - operators evaluated in reverse order (6 → 1), with modulation assembled in two buffers (M and F).
- **Frequency** is a log value: 4 bits of octave plus 10 fractional bits, or about 1.17 cents. Within an octave it is
  quantised linearly to 1/4096.
- **Gain** is a 12-bit log value in steps of about 0.0235 dB (256 steps per doubling). Operator output = `exp(logsin(phase) + gain)`, so **there is no multiplier**: the envelope *adds* in the log domain.
- **Frequency controls** (common DX7 documentation):
  - ratio coarse of 0.5 or 1–31, times (1 + fine/100);
  - detune of ±7 small steps;
  - **fixed mode** of 1, 10, 100 or 1000 Hz, times an exponential fine.
- **Free matrices** (FM8, Sytrus) allow cycles. The usual way to evaluate them:
  - pick a fixed operator order;
  - edges that go "forward" in that order use the current sample;
  - edges that go "backward" (cycles) use the previous sample (`z⁻¹`).

  Sytrus's manual says items in one block of its diagram are "processed at the same time". The order must be
  fixed and documented, because it changes the sound. **Cantus: order by operator letter, and use `z⁻¹` on back
  edges** (deterministic, and shown in the graph as a small clock glyph on the edge).

### 4.3 Feedback and the anti-hunting filter

- **Tomisawa's feedback FM** ([US 4,249,447](https://patents.google.com/patent/US4249447), Yamaha) feeds an
  operator's output back into its own phase: `y[n] = sin(φ[n] + β·ȳ[n])`.
- **As β rises, the spectrum fills in toward a sawtooth-like 1/n series** whose bandwidth grows with β. Beyond a
  point the one-sample loop **"hunts"**: it falls into a limit cycle at or near Nyquist, and then into noise.
- **The DX7's fix**, measured in the MSFA wiki and seen on the die by Shirriff:
  - **average the last two outputs**: `ȳ = (y[n−1] + y[n−2]) / 2`;
  - the feedback level 0–7 is a **power-of-two shift**, so no multiply is needed.
- **Why it works:** a two-tap average has a **zero at fs/2**, so it removes exactly the Nyquist mode that the loop
  wants to lock into. The cost is half a sample of extra delay, which changes the timbre slightly.
- **Consequences for us:**
  - the averaging is **part of the DX sound**, so it must be on by default and be a documented switch;
  - feedback depends on the sample rate, because the loop delay is one sample, so FB at 96 kHz is brighter and
    more stable than at 44.1 kHz.
  - With oversampling (§4.5), feedback runs at the oversampled rate. That is why Sytrus warns that oversampling
    changes the feedback sound. **Cantus must run feedback at a fixed internal rate** (below) so the patch sounds
    the same whatever the quality setting.
- Schoffhauzer (2007) analyses feedback FM as a generator of quasi-band-limited sawtooth waves. That is a useful
  second oscillator mode ("FB saw"): cheap, and its alias content is controlled by β.

### 4.4 The DX7 envelope, as measured

Sources: the MSFA wiki's [DX7 envelope](https://github.com/google/music-synthesizer-for-android/blob/master/wiki/Dx7Envelope.wiki)
page, a nearly bit-accurate model, and [DX7 hardware](https://github.com/google/music-synthesizer-for-android/blob/master/wiki/Dx7Hardware.wiki).
The numbers are paraphrased from those measurements.

- **Parameters.** Four rates R1–R4 and four levels L1–L4, each 0–99. The envelope runs L4 → L1 → L2 → L3, holds
  L3, and moves to L4 on release.
- **Levels.**
  - L 0–5 map to 2·L; 5–16 to 5 + L; 17–20 to 4 + L; 20–99 to 14 + L/2.
  - Output level has twice the precision: 28 + L above 20, and a table below.
  - Total level = 64·level + 32·output level, in steps of about 0.0235 dB.
  - The **floor clips at about 3824 steps below full scale**, roughly 90 dB. Velocity can push past full scale.
- **Rate.**
  - `qrate = rate·41/64` (0–63).
  - **Decay is linear in dB**: about `0.2819 · 2^(qrate/4) · (1 + 0.25·(qrate mod 4))` dB/s.
  - At qrate 0 the level moves one step every 4096 samples. Each +4 doubles the speed. Above 48, the step size
    grows instead.
- **Attack.** The step is multiplied by a factor that grows with the distance still to go: `2 + (full − current)/256`.
  This gives the characteristic "concave in dB" DX attack. The level also **jumps at once to about 40 dB above the
  floor**, for a crisp start.
- **Timing is quantised** into fixed bit patterns for fractional qrates. A bit-exact "Mark I" mode (Dexed has one)
  reproduces the resulting stair-stepping.
- **Retrigger** starts the attack from the current level. FM8 and Operator do the same, to avoid clicks.
- **Keyboard scaling.** Rate scaling is 0–7. Level scaling has a break point, left and right depths of 0–99, and
  four curves (−LIN, −EXP, +EXP, +LIN). This is how DX patches tame bright modulators on high notes, which is also
  **the DX era's anti-aliasing tool**.
- **FM8 replaced rate/level with time/level.** It is easier to read, but a DX import must convert rates to times,
  and that conversion depends on the distance each segment travels.
- **Cantus should offer both:**
  - a DX mode driven by the published formulas, used for SysEx import and for "that sound";
  - a Time mode (breakpoints, curves, loops), which is the default for new patches.

### 4.5 Aliasing in FM: numbers, and the options

- **Carson's rule.** Most of the energy of a PM/FM spectrum lies within `f_c ± (I + 1)·f_m`. The upper edge is
  `f_c + (I + 1)·f_m`. Examples at fs = 48 kHz (Nyquist 24 kHz):

| Note (carrier), ratio | Index I | Upper edge | Result |
|---|---|---|---|
| A4 440 Hz, 1:1 | 5 | 440 + 6·440 = **3.1 kHz** | clean |
| C7 2093 Hz, 1:1 | 5 | 2093 + 6·2093 = **14.7 kHz** | clean |
| C7 2093 Hz, 1:1 | 10 | 2093 + 11·2093 = **25.1 kHz** | **aliases** |
| C7 2093 Hz, 1:3 (f_m 6.3 kHz) | 3 | 2093 + 4·6279 = **27.2 kHz** | **aliases** |
| A6 1760 Hz, 14:1 "tine" modulator | any | the modulator itself is **24.6 kHz** | **above Nyquist before any FM** |

- **Negative frequencies are not aliasing.** Sidebands below 0 Hz reflect (with inverted phase for sine carriers);
  that is part of the FM sound and must be kept.
- **Non-sine operators** (FM8's 32 waves, Sytrus shapes, Omnisphere's wavetable modulators) have infinite
  bandwidth, so they alias at any index unless band-limited.
- **The options the products use:**
  1. **Oversampling.**
     - Sytrus: 2–4× or more, for render; its manual notes the CPU cost scales with the factor.
     - FM8: "doubling the resolution", global and not saved.
     - Operator: an undocumented Antialias mode.

     Decimation with a cascade of polyphase half-band filters is standard. *Linear-phase FIR* half-bands add
     latency (with ~31 taps, a few samples per stage). *IIR allpass polyphase* half-bands (Valenzuela and
     Constantinides 1983; the approach of the public-domain HIIR library) have near-zero latency and a phase error
     only near Nyquist.
  2. **Index key-scaling:** the DX level-scaling curves; Sytrus's advice to map modulator level to key.
  3. **Band-limited waves:** Sytrus's *band limit*; Operator's waves with a stated harmonic count (Saw 32 and so on).
  4. **A brightness macro** (Operator's Tone, FM8's Brightness). It changes the sound, not only the alias content.
- **What nobody does: bandwidth-aware control.** The upper edge can be *computed* for each voice and each control
  block from the current ratios, indices and envelope levels. **Cantus proposes an "alias guard" with three
  settings:**
  - **Show**: draw each voice's Carson edge on the spectrum, and turn a badge amber when it passes 0.45·fs.
  - **Tame**: soft-limit each modulator's index, per voice, so the edge stays under 0.45·fs. This is a
    deterministic DX-style key scaling, done automatically.
  - **Oversample Auto**: choose each voice's factor (1, 2, 4 or 8) **once, at note-on**, from the pitch and the
    maximum reachable index (envelope peaks plus modulation range). Change it only at note boundaries, so there are
    no clicks and the result is deterministic.
  - **Fixed internal rate:** all FM and feedback run at `fs_int = 2 · 48 kHz` (or at the host rate times N). The
    patch sounds the same at 44.1, 48 and 96 kHz, and render equals playback.
- **Cost of oversampling (my estimate).**
  - A 4× oversampled voice costs about 4× the operator work, plus about 8–12 multiply-adds per output sample per
    half-band stage.
  - At 8 operators per voice (§4.6) that is roughly 1,000 flops per output sample per voice.
  - It is affordable for 16 voices on one core at 48 kHz (about 0.8 GFLOP/s), but not for 64 voices with unison.
    Hence per-voice *Auto*, not a global 8×.

### 4.6 FM cost per voice (my estimate)

- **Per operator per sample:**
  - phase accumulator: an add;
  - sine: a 4096-entry table with linear interpolation, whose worst-case error is about −130 dB, or an odd
    polynomial;
  - envelope: an interpolated control value every 16 samples;
  - the modulation sum: up to 8 multiply-adds for a full row;
  - output routing.

  That is about 25–30 flops.
- **8 operators:** about 240 flops per sample per voice, or about 11.5 MFLOP/s per voice at 48 kHz.
- **With SIMD across voices** (8 voices in one 8-lane AVX2 register), a 16-voice patch at 1× costs well under 5 % of
  one modern core.
- **Determinism detail:** don't call libm `sin`, whose output differs between Windows, macOS and Linux. Use our own
  table or polynomial, so a render is bit-identical on every OS.

### 4.7 Additive: oscillator bank versus inverse FFT

| | Oscillator bank | Inverse-FFT (FFT⁻¹) synthesis (Rodet & Depalle 1992) |
|---|---|---|
| Method | one oscillator per partial (phase accumulator + sine, or a recursive complex rotator `z ← z·w`) | "splat" each partial into K bins of a spectral frame using the window's transform, run one IFFT per hop, overlap-add |
| Per-partial cost | about 8 flops per sample (rotator + amplitude ramp + accumulate) | about K complex multiply-adds per **hop**; with K ≈ 9 and hop 256, about 0.035 per sample, **roughly 200× less** |
| Shared cost | none | one IFFT per hop per output channel, **shared by all voices** (for N = 1024, hop 256: about 20 butterflies per sample) |
| Parameter rate | per sample (FM of partials, fast glides, exact attacks) | per hop (5.3 ms at 256/48 k); fast changes smear |
| Low partials | exact at any frequency | need N ≥ 4096 when partials are closer than a few bins (resolution is fs/N: 47 Hz at N = 1024) |
| Noise and residual | needs a separate noise path | natural: add noise bins |
| Aliasing | **zero**: simply don't render partials above Nyquist, with a short taper so a pitch bend doesn't click | zero, likewise |
| Determinism | trivial; recursive rotators need a per-block renormalisation, done deterministically | trivial |

- **How many partials a voice needs:** `floor(0.5·fs / f0)`. At 48 kHz: A0 → 872, A1 → 436, A2 → 218, A3 → 109,
  A4 → 54, A5 → 27, A6 → 13.
  - The product ceilings (Harmor 516, Zebra 3 1024) only matter below about 47 Hz and 23 Hz respectively.
  - **Real cost scales with the note, not the knob**, so a bank that culls above Nyquist is cheaper than its
    maximum suggests.
  - Estimate: a bank at about 1 cycle per partial-sample (8-wide SIMD) needs about 25–50 M cycles/s for 512
    partials per voice. That is 1–2 % of a 3 GHz core per voice at the bass extreme, and a fraction of that at A3.
- **A third way, used by the wavetable flagships:** render the *harmonic* spectrum into a single-cycle table with an
  IFFT whenever it changes (per block), then play it with mip-mapping. It is very cheap, but it is **harmonic
  only**. Inharmonic effects (Harmor's Prism, Zebra's inharmonic additive) need a bank or a full IFFT.
- **Recommendation for *Partes*:** a SIMD bank with Nyquist culling. Offer a "table" fast path when the spectrum is
  harmonic and static during a block. Use the IFFT only in *analysis*.

### 4.8 Resynthesis: sinusoidal modelling and the phase vocoder

- **Sinusoidal modelling** (McAulay & Quatieri, IEEE TASSP 1986; **SMS**, Serra & Smith, CMJ 14(4) 1990):
  1. an STFT (Blackman-Harris or Hann; N = 4096 at 48 kHz gives 11.7 Hz bins; zero-pad ×2);
  2. **peak picking with parabolic interpolation** on the log magnitude (typically ±0.1 bin);
  3. **partial tracking** (births, deaths, continuation by nearest frequency);
  4. optionally **harmonic assignment** from an f0 track (for example YIN; de Cheveigné & Kawahara 2002), which is
     Harmor's model of "partial index × time";
  5. the **residual** (input minus the sinusoids) modelled as time-varying filtered noise, in 24–32 ERB/Bark bands
     per frame.
  6. Transients need a separate model (Verma & Meng's *transient modeling synthesis*, 2000), or keeping the original
     attack and crossfading into the resynthesis (proposed below).
- **Phase vocoder** (Flanagan & Golden 1966; tutorial by Dolson, CMJ 1986; **phase-locked** vocoder by Laroche &
  Dolson, IEEE TSAP 1999): time-stretch and pitch-shift in the STFT domain. It is better for *processing* audio
  (Serum 2's spectral mode is this family) and worse for *synthesis editing*, because there are no partials to
  grab.
- **Harmor's representation** (gain and pitch planes indexed by partial) is the *editable* one. It reads like a
  picture, can be drawn, and plays back with the same engine as the synth. It is the right target for Cantus's
  resynthesis: analysis produces **harmonic frames** (per hop, per partial: amplitude and frequency deviation) plus
  a noise-envelope track.
- **What is hard:**
  - **inharmonic sources** (bells, pianos with stretched partials) break harmonic assignment, so keep free tracks
    as an option;
  - **noisy sources** push most of the energy into the residual;
  - **low notes** need long windows, which smear transients.
  - Honest defaults: harmonic mode with an inharmonicity tolerance; FFT size 4096, hop 256; a residual level
    control; "keep attack" of 30 ms.

### 4.9 Granular: scheduling, windows, randomness

- **Background.** The grain is Gabor's acoustic quantum (Nature, 1947). The canonical texts are Roads (CMJ 1988;
  *Microsound*, MIT Press 2001) and Truax (real-time granular synthesis, CMJ 12(2), 1988). Bencina (2001) is the
  standard implementation note.
- **Grains** are windowed segments of a source, typically 1–100 ms, or up to 500 ms in Absynth.
  - Short grains spread energy in frequency (the uncertainty relation), so a 5 ms Hann grain sounds like a click
    with a colour.
- **Scheduling:**
  - **Synchronous:** a fixed inter-onset interval (IOI). At IOIs under about 20 ms the grain rate is heard as a
    *pitch*, which is the basis of pulsar, FOF (Rodet 1984) and PSOLA (Moulines & Charpentier 1990). Absynth's
    Sync Granular is this.
  - **Quasi-synchronous:** a fixed IOI with jitter.
  - **Asynchronous:** Poisson onsets at density λ grains/s. The mean overlap is `λ·L`. The probability that a given
    instant is *uncovered* is `e^(−λL)`: at λ = 50/s and L = 20 ms that is 37 %, which sounds "cloudy"; at L = 80 ms
    it is 2 %, which sounds continuous.
  - **Overlap-add check:** synchronous Hann grains at hop = L/2 sum to a constant (COLA), so a frozen position
    sounds smooth. Other windows need their own hop.
- **Controls across products:**
  - read position and scan rate (freeze at 0; reverse below 0);
  - **spray**, the position jitter;
  - pitch (transposition by resampling each grain), and pitch randomness: in cents, or in *intervals* (Omnisphere's
    Interval 1–12 with a direction);
  - pan spread; level randomness; **reverse probability** (Serum's Dir);
  - window choice, amount and skew; density; length; max grains.
- **Pitch-shifting grains aliases when transposing up.** Reading at 2× folds everything above fs/4. Two fixes:
  - a **per-octave, pre-filtered mip-map** of the source, built when the sample loads (the same trick as a
    wavetable), with 4-point cubic reading;
  - a windowed-sinc reader (16–32 taps) for the HQ setting.
- **Cost (my estimate):** per active grain per sample, one interpolated read (about 7 flops), a window lookup and a
  pan, about 15 flops. 64 grains per voice at 48 kHz is about 3 M grain-samples/s, or 0.1–0.3 % of a core per
  voice.
- **Determinism, which nobody documents:**
  1. **Schedule on the sample clock** counted from the start of the voice, never from the block. A grain onset is a
     sample index plus a fraction (the fraction becomes the grain's starting phase), so any block split gives the
     same onsets.
  2. **One seeded PRNG per voice** (PCG or xorshift), seeded from (patch seed, note ID, voice index). Advance it once
     per *grain event*, not per block.
  3. **A fixed grain pool** allocated when the synth is built: 128 per voice. When full, **skip** the spawn
     (Serum's policy) rather than steal. Skipping is deterministic; stealing needs a fade.

---

## 5. The interface: how each makes a deep engine playable

### 5.1 Six strategies the market uses

| Strategy | Who | Praise | Criticism |
|---|---|---|---|
| **Show the topology** | Operator (algorithm icons of coloured squares; the colours match the shell rows); FM8/Sytrus (matrix); Zebra 3 FMO (signal flow drawn inside the module); Dexed (DX panel and algorithm display) | Operator's **shell** always shows Coarse, Fine, Fixed and Level for all four operators, the most important FM numbers, at rest. The consistent colour per oscillator links icon, row and display. The FM8 matrix is compact and free-form; its number boxes are the most legible element on screen (14.6:1). | A matrix hides *direction and flow*: you read coordinates, not a signal path. Sytrus's knob matrix hides values (knob angle only) and can't be automated. Operator has only 11 algorithms. |
| **Hide the topology behind meta-controls** | FM8 Easy page (Harmonic, Brightness, Detune, envelope offsets, **Apply**); Omnisphere Adaptive Global Controls (Tone, Ambience, Filter, Envelope, Vibrato, Unison, "analyse any patch"); Absynth/Omnisphere macros; Operator's Time and Tone | Musicians get useful results at once. FM8's centred bipolar sliders make "0 = the original" obvious. | The mapping is opaque: you can't see *what* Brightness moved. FM8's Apply destroys the offset layer (it becomes a new baseline). Omnisphere doesn't document what its adaptive controls change. |
| **Navigate a space instead of editing** | FM8 **Morph Square** (4 corners + random cloud); Omnisphere **Orb** (angle = scene, radius = amount, Dice, Inertia, Attractor, record); **Mutations** (Omnisphere, Absynth); Absynth **AI Explorer** map; Omnisphere Sound Match | Performable and automatable (Orb recording, Morph as a parameter). Mutation histories encourage exploration. The Explorer map makes a 2,000-preset library browsable "by feel". | Variation without understanding. The Orb's scenes are random sets, so they can't be *designed*. Histories live inside the plug-in, outside the DAW's undo, and are lost on a project revert. |
| **Draw it** | Operator's harmonic bars (16/32/64 + Repeat); Sytrus's amplitude + phase harmonic editor (128); Harmor timbres and **images**; Zebra 3's unified **spline editor** (waves and MSEGs, geometric morph) | Direct manipulation, matching the house rule (§7.1 of family.md). A spline editor shared between waveforms and envelopes is a strong unifying idea. | Drawing is only half the job: you draw a *spectrum* but hear a *timbre*, and no product fits parameters to a target the user draws or loads. |
| **One articulation model for every parameter** | Sytrus/Harmor: every target has ENV, LFO, KEY M, VEL M, X/Y, RAND and UNI graphs; FM8 stacked envelopes plus key-scaling curves | Extremely expressive: per-unison-voice curves, per-partial curves ("harmonic mappings"). | Deep and invisible. Each target hides seven tabs. Nothing shows at rest which targets have articulation. |
| **Hardware** | Omnisphere **hardware profiles** (300+) that turn a hardware synth's panel into Omnisphere's controller; Adaptive MIDI Scaling | Muscle memory transfers from the hardware. | Specific to Omnisphere. A host can do this *for every device* (family.md §7.5 #7). |

### 5.2 Layout and hierarchy, product by product

- **FM8** (fm-01 … fm-04).
  - A light grey Komplete-era chassis: a slate navigator at the left, a dark matrix at the right, and a keyboard
    across the bottom.
  - The hierarchy is *navigator → page*. The matrix repeats on every Expert page, which is a good anchor.
  - Low points: tiny grey text; values at 2.4:1; eight envelopes stacked as 20-px strips; the design dates from
    2006.
- **Operator** (fm-05 … fm-07).
  - A three-column device on one strip: a shell of the most important numbers left and right, and a dark detail
    display in the centre that follows the selection.
  - It is the best example of **progressive disclosure without hiding state**. Its limit is its size: 4 operators
    and 11 algorithms.
- **Sytrus** (fm-08 … fm-10).
  - Tabs across the top (MAIN, OP1–6, FILTER1–3, FX) and a matrix that is always present on the main page.
  - Orange marks active knobs, a clear state cue.
  - The patch comment box explaining the X/Y pad is a nice convention, a human "macro legend".
- **Harmor** (fm-11, fm-12).
  - Maximum density: dozens of small knobs on maroon, and a vertical watermark reading "FILTERING SECTION".
  - Beginners are rescued by the subtractive vocabulary (filter, phaser, unison) and by drag-and-drop
    resynthesis. The visualiser at the right is decorative but motivating.
- **Omnisphere 3** (fm-16 … fm-18).
  - Tabs at three levels: ORB / MAIN / A–D / FX / ARP, then the layer page, then *Zoom* pages.
  - The process grid with **underlines marking active processes** shows state at rest.
  - The granular display colours grains by pitch.
  - The Orb gives a whole tab to one circle, which is generous but clear.
- **Zebra 3** (fm-13 … fm-15).
  - The adaptive modular layout shows only what exists.
  - Colour codes module types. A source strip along the bottom works as a palette of modulators to drag from.
  - Labels measure 3.7:1, which fails for small text.
- **Absynth 6** (fm-19, fm-20).
  - Top-level icon tabs (Browser, Patch, Effects, Waves, Envelope, LFO, Assign).
  - Envelopes are listed with **thumbnails** and edited in a stack.
  - The Explorer map and Mutator dominate the browser. It is modern and calm, with some low-contrast list text.

---

## 6. What reviewers and users say

*Caveat: forum posts are anecdotal, and the threads are old (KVR 2020 and 2022). Gearspace was not readable (403).
Reviews are single-author blogs or magazines.*

- **Omnisphere 3** ([BangerLoops review](https://bangerloops.com/articles/spectrasonics-omnisphere-3-review/)).
  - Praise:
    - the Adaptive Global Controls as a major workflow gain;
    - Mutations as a way out of creative block, with every mutation kept;
    - a cleaner browser;
    - thousands of new patches in about the same disk footprint.
  - Criticism:
    - a **premium price** ($499);
    - the **GUI looks largely unchanged**;
    - **heavy CPU on complex patches** (the advice is to cut unison or bounce);
    - a **steep curve for deep design**.
- **Zebra 3** ([Dubspot, 9.0/10](https://blog.dubspot.com/u-he-zebra-3)).
  - Praise: a real rebuild; spline waves; additive up to 1,024 partials; through-zero FM with an audio input; the
    adaptive interface cuts clutter.
  - Criticism: the price; the learning time ("not for preset-driven workflows"); a minimum display of **1300 × 910**.
- **Absynth 6** ([gearnews on 6.1](https://www.gearnews.com/absynth-6-native-instruments-synths/),
  [Sonicstate](https://sonicstate.com/news/2025/12/09/absynth-returns/)).
  - Welcomed back after its 2022 retirement.
  - The AI Explorer is framed as widening discovery, not automating it.
  - The learning curve is "still demanding".
  - Telling detail: **undo/redo arrived only in 6.1**.
- **Harmor and additive in general** (KVR [2020](https://www.kvraudio.com/forum/viewtopic.php?t=553024),
  [2022](https://www.kvraudio.com/forum/viewtopic.php?t=585257)).
  - Harmor is loved for pads and resynthesis quality, but gives "not much control over the harmonic structure"
    compared with per-partial editors. **"Editing partials is not intuitive."**
  - Users distinguish *true additive* (per-partial envelopes: Alchemy, MSoundFactory, Vertigo) from *wavetable synths
    with additive editors*.
  - Per-partial control is called "ridiculously powerful" and "time-consuming".
  - The recurring tension: **control against usability**.
- **FM8.** It is praised as the modern DX7 with a free matrix and DX import. The evidence of decline is factual, not
  opinion: the price has fallen to $99, and the only updates were for compatibility.
- **Sytrus / Operator.** No fresh reviews were found in reach. The manuals themselves concede the difficulty:
  Sytrus's tutorial says FM "can seem intimidating" because sidebands are hard to picture; FM8's manual opens its
  Easy chapter by saying FM "was never easy to program".

---

## 7. Gaps none of them fills

1. **The instrument's undo is not the project's undo.**
   - FM8, Sytrus and Harmor have their own histories, or none.
   - Absynth gained undo only in 6.1.
   - Omnisphere's and Absynth's mutation histories live inside the plug-in.
   - Cantus: every edit, morph, mutation and AI action is a host command in one history, with branches.
2. **No product shows where the sidebands land.** Aliasing is left to opaque controls: Tone, High resolution, and
   separate draft and render quality.
3. **Render doesn't equal playback.** Sytrus's draft and render quality differ. FM8's quality isn't saved with the
   patch. None claims determinism or block-size independence.
4. **No product programs a sound from a description or explains one.** Absynth's AI only *browses*.
5. **No inverse design.** You can't draw or load a target spectrum and get operator ratios and indices fitted to
   it, although matching FM to a target is a solved research problem (Horner, Beauchamp & Haken, *genetic
   algorithms and FM matching synthesis*, CMJ 17(4), 1993).
6. **Macros and morphs stay inside the instrument.** The Orb can't open a send or move an EQ band on another track.
7. **Matrix cells are second-class.** Sytrus matrix knobs can't be automated. FM8 automates them but shows no
   modulation on them.
8. **State hidden behind tabs.** Sytrus articulation tabs and Harmor mappings don't show at rest which targets are
   modulated.
9. **Resynthesis is file-based.** Harmor wants a dropped file. Nobody resynthesises *a clip already in the project*
   while keeping a link to it.
10. **Accessibility.** Matrix grids are exposed to no accessibility API. Contrast fails in the newest designs.
11. **Plug-in walls.** Only Zebra 3 and Dexed ship CLAP. No instrument knows its track's name, colour or key.

---

## 8. How Cantus beats them

### 8.1 Decision: one dedicated FM instrument, plus additive and granular as Firmus II engines

**Options considered:**
- **A. One "deep engine" instrument** holding FM, additive and granular, in the style of Absynth, Omnisphere or
  Zebra 3.
- **B. Fold everything into Firmus II** as oscillator types, in the style of Serum 2, Pigments or Vital.
- **C. A hybrid (recommended):**
  - FM gets its own instrument;
  - additive/resynthesis and granular become Firmus II oscillator engines;
  - all three are built on shared `cantus-dsp` kernels (`fm`, `partials`, `grains`).

**Why C:**
- **FM's timbre is its topology.** A credible FM instrument needs, per operator, an envelope, key scaling, a
  ratio/fixed/offset choice and a velocity response, plus a free matrix, the DX algorithm templates and SysEx
  import. As an oscillator type inside Firmus II, it collapses to 2–3 operators with shared envelopes (Serum's "FM
  from B", Omnisphere's single FM modulator, Zebra's two-operator FMO). That is exactly the limit the market's FM
  specialists exist to escape.
- **FM's UX problem is the product.** The graph editor, the Carson display, the explain and describe features and
  the inverse fit are FM-specific. They need a whole editor surface, not a panel inside another synth.
- **Additive and granular are *sources*.** They want the flagship's filters, modulation, unison, effects and preset
  browser. Duplicating those in a second instrument (option A) splits users' attention and doubles the UI work.
  The market agrees: Serum 2, Pigments 7 and Zebra 3 all present granular and additive as oscillator modes.
- **The shared kernels keep the options open.**
  - Firmus II can still offer an "FM pair" oscillator from the `fm` kernel (Zebra's FMO approach).
  - Modulus can later accept a *Partes* spectrum or a *Grana* source as an operator wave (P2), without duplicate
    DSP.
- **Scope fits the plan.** STATUS already schedules Firmus II as the Serum/Vital-class flagship. This adds two
  engines to it and one new instrument whose DSP is small (§4.6), where most of the work is interface work.

**Names.** In Latin, like Firmus, Pulsus, Aequalis and Comprimo:
- **Modulus** for the FM instrument. *Modulus* means a measure, a rhythm, a melody; it is the root of "modulation".
  Collision risk: a British hardware synth sold as "Modulus.002" in the 2010s. Alternatives: *Textus* (a woven
  network) or *Rota*.
- **Partes** for the additive/resynthesis engine ("the parts": partials).
- **Grana** for the granular engine ("grains").

### 8.2 P0: parity (without these we are not credible)

1. **An 8-operator phase-modulation engine with a free matrix.**
   - Any operator can modulate any other, including itself.
   - Back edges use `z⁻¹`, and the evaluation order is fixed.
   - **All 32 DX7 topologies as templates**: the topologies are facts, so no code is copied.
   - Output and pan per operator; ratio, fixed and offset frequency; key sync or free run; per-operator pitch-envelope
     enable; invert.
2. **Feedback with Tomisawa's anti-hunting average**: on by default, documented as a switch.
3. **Envelopes.**
   - A DX mode (4R/4L, qrate, log-domain attack and decay, the published level tables).
   - A Time mode: up to 16 breakpoints, curves, sustain and loop markers, tempo sync, time key-scaling and velocity
     time-scaling.
   - Restart from the current level.
   - **Level key scaling**: a DX break point with depths and four curves, or a breakpoint curve.
4. **DX7 SysEx import.**
   - A 32-voice cartridge (4104 bytes: header, 4096 bytes of packed voices, checksum) and single voices.
   - Parameters mapped through the published scalings.
   - Written from the documented format, never from Dexed.
5. **Alias control that is saved with the patch.**
   - Oversampling of 1, 2, 4 or 8× with polyphase IIR half-bands (zero latency), plus *Auto*, chosen per voice at
     note-on.
   - Band-limited non-sine waves (harmonic tables per octave).
   - A **fixed internal FM rate**.
   - **Render equals playback, bit-identical.**
6. **An approachable layer at FM8's level.**
   - Macros: Brightness (scales every modulator's index), Harmonic, Detune, and Attack/Decay/Sustain/Release offsets.
     They are *non-destructive*: kept as offsets, with an explicit "Bake" command.
   - A **4-corner morph pad** (matrix and operator settings), automatable.
   - Presets, with the family's preset browser.
7. **Expression:** velocity, aftertouch, **MPE** (per-note index and pitch), glide, unison 1–8, microtuning
   (Scala/MTS-ESP later with the standards work).
8. **Host guarantees:**
   - every parameter is a host parameter: automatable, modulatable and undoable, *including every matrix cell*;
   - null-tested at defaults;
   - block-size independent;
   - deterministic.
9. **For Firmus II (coordinate with wavetable.md):**
   - ***Grana*** at Serum 2 parity: scan, density in Hz/sync/count, length, 8+ windows with amount and skew,
     per-grain random offset, direction, pitch, length, pan and level, a max-grains cap, and mip-mapped
     transposition.
   - ***Partes*** at Harmor-lite level: up to 1024 partials culled at Nyquist; spectral filter, tilt, odd/even,
     stretch/prism, blur and pluck; and **resynthesis from a file or a project clip** into harmonic frames plus a
     noise track.

### 8.3 P1: differentiators (what a host-native instrument can do)

1. **Program by description (the AI bridge).**
   - A `design_fm_patch` tool takes a structured spec: operators, ratios, indices in radians, envelope shapes,
     algorithm edges and macros.
   - `cantus-ai` validates it and compiles it into **one `Command::Batch` of ordinary `SetInstrumentParam`
     commands**: one undo step, visible in the history, identical to a user's edit.
   - `describe_instrument` returns the patch **in words**, for example: "two stacks: B(14:1, index 1.2, fast
     decay) → A(1:1), the tine; D(1:1, feedback 0.8) → C(1:1), the body; brightness follows velocity". That lets
     the model edit incrementally ("more bell" means a non-integer ratio near 3.5 and a longer modulator decay) and
     lets it *teach* FM.
   - Nobody else can do either; Absynth's AI only arranges presets.
2. **Inverse design.**
   - **Fit to target**: draw a harmonic spectrum, grab one from a clip, or pick a Partes spectrum.
   - A deterministic, seeded optimiser (a genetic algorithm or CMA-ES, after Horner, Beauchamp & Haken 1993)
     proposes **N candidate patches** within a user-chosen topology.
   - Each candidate is shown with its spectral error, and auditioning is one click.
   - Paired with Partes, this becomes a "sample → FM" workflow.
3. **The spectrum *is* the FM display.**
   - A live output spectrum, with the **Carson edge of each voice** as a marker, the Nyquist line, and an amber
     badge when the alias guard is working.
   - Hovering an edge in the graph highlights that edge's contribution (the sideband spacing `f_m`).
   - This teaches why ratio 1:1 gives all harmonics and 1:2 gives odd ones.
4. **Mutations and morphs in the project history.** "Mutate" (seeded; Subtle to Wild), the morph pad and macro sweeps
   are host commands. Branches can be compared A/B at matched loudness (family.md §7.5 #10). No lost timelines.
5. **Modulation reaching beyond the instrument.**
   - Modulus's envelopes, LFOs and macros are **track-level modulation sources** (family.md §7.6). A macro can also
     open a reverb send, or move an Aequalis band on the vocal bus.
   - Host sources (a sidechain envelope follower, a clip envelope) can modulate any matrix cell.
   - A mono sum or the last note stands in for per-voice sources outside the instrument, documented.
6. **Aware of the arrangement.**
   - The morph pad as a clip envelope.
   - "Freeze variant to clip".
   - Optional index scaling by the project's key or scale (for example, fixed-frequency operators snapped to scale
     notes).
   - Resynthesis of **any clip in the project** with a live link: re-analyse when the clip changes.
7. **A matrix that shows state at rest.** Every cell shows its value as a number (not a knob angle), a ring when it
   is modulated or automated, and a striped ring for several sources.
8. **Keyboard-first graph editing and accessibility.**
   - Letters select operators; arrows move the ratio (Shift: fine, in cents); `[` and `]` change the index.
   - Tab walks the edges; Enter types a value (with the parser: `3.5`, `14:1`, `A4`, `1k`).
   - AccessKit names every node and edge ("B to A, index 1.2 radians").
9. **ModFM and FB-saw operator modes** (§4.1, §4.3): cheap, well-behaved spectra that the competitors lack.

### 8.4 P2: later

- Operators whose wave is a *Partes* spectrum or a *Grana* source (the Absynth and Zebra hybrid).
- Sytrus-style per-target mapping graphs (key, velocity, random, per unison voice) for every parameter.
- A bit-exact "DX Mark I" colour: a 49,096 Hz internal rate, 12-bit log envelopes, stair-stepped rates.
- A post filter as a node (FM8's Z) with drive.
- A formant or physical operator (Karplus pluck like Sytrus's).
- Surround panning (Absynth).
- Hardware profiles as host-level "surface maps" for all devices (Omnisphere's idea, generalised).
- An FM8-style audio input as a matrix node (FM the vocal).

### 8.5 Concrete proposal: Modulus

**Voice architecture** (ADR 0005: fixed pool, no allocation after construction):

```
Voice (pool of 64, stealing as in Firmus: free → oldest releasing → oldest)
 ├─ note state: pitch, velocity, MPE (pressure, timbre, bend), note ID, seeded PRNG (drift/mutate)
 ├─ oversampling factor N ∈ {1,2,4,8}, fixed at note-on (Auto) or by patch
 ├─ operators A..H: phase u32 (exact periodicity), incr, env state, last two outputs (feedback average)
 ├─ control grid: every 16 samples counted from stream start (the Aequalis rule), linear interpolation between
 ├─ evaluation: fixed order H→A; forward edges use this sample, back edges z⁻¹
 ├─ output: Σ carriers × out level → per-operator pan → stereo → (optional) post filter
 └─ decimation: polyphase IIR half-band cascade (N/2 … 1), per voice, state reset at note-on
Engine: unison (1–8 copies per note, detune/spread), glide, pitch envelope, 2 LFOs per voice
        + host modulation service; gain smoother; idle = exact silence (the Firmus rule)
```

**Parameter model.** It uses the generic `key → f32` model (ADR 0017), stored **sparse**; a new patch stores only what
moved. Operators are A–H and **never renumbered**. The keys:

- `op.<X>.<field>` for operator fields;
- `m.<S>.<T>` for the matrix (source S → target T); `fb.<X>` for feedback;
- `out.<X>` and `pan.<X>` for outputs;
- `env.<X>.<field>` for envelopes.

`InstrumentParam::Modulus(key)` is a new variant, as STATUS says a third instrument needs.

| Key | Name | Range | Default | Unit / notes |
|---|---|---|---|---|
| `voices` | Voices | 1–64 | 16 | |
| `mode` | Mode | Poly / Mono / Legato | Poly | |
| `glide` | Glide | 0–5000 | 0 | ms |
| `unison`, `uni.detune`, `uni.spread` | Unison | 1–8; 0–100; 0–100 | 1; 10; 50 | voices; cents; % |
| `transpose`, `tune` | Transpose, Tune | ±48; ±100 | 0; 0 | st; cents |
| `pb_range` | Bend range | 0–48 | 2 | st |
| `quality` | Quality | Eco 1× / Auto / 2× / 4× / 8× | Auto | **saved with the patch** |
| `alias_guard` | Alias guard | Off / Show / Tame | Show | Tame limits indices to keep the Carson edge < 0.45·fs_int |
| `fb_average` | Anti-hunt | Off / On | On | the Tomisawa two-sample mean |
| `drift` | Analog drift | 0–100 | 0 | %, seeded, deterministic |
| `vel_curve` | Velocity curve | −1…+1 | 0 | 0 = linear |
| `gain` | Output | −60…+12 | −6 | dB (Firmus's default gain) |
| `op.X.on` | Operator on | 0/1 | A on, others off | |
| `op.X.wave` | Wave | Sine / Harmonic / Noise / FB-saw / ModFM | Sine | Harmonic = drawn 16/32/64 partials, band-limited per octave |
| `op.X.freq_mode` | Frequency | Ratio / Fixed | Ratio | |
| `op.X.ratio` | Ratio | 0.0625–64.0000 | 1.0000 | shown as coarse + fine; DX coarse 0.5, 1–31 |
| `op.X.detune` | Detune | ±100 | 0 | cents (DX ±7 mapped) |
| `op.X.offset` | Offset | ±100 | 0 | Hz, constant beating (FM8) |
| `op.X.fixed` | Fixed | 0.1–20,000 | 440 | Hz, log taper |
| `op.X.level` | Level | 0–100 | 100 | %, scales the operator's whole column (FM8 ratiometric Level) |
| `op.X.vel` | Velocity → level | 0–100 | 0 | % (on a modulator this is velocity → brightness) |
| `op.X.phase`, `op.X.key_sync` | Phase, Key sync | 0–360; 0/1 | 0; 1 | degrees; reset at note-on |
| `op.X.pitch_env` | Pitch env | 0/1 | 1 | |
| `op.X.input` | Input mode | PM / FM / RM / AM | PM | FM is DC-blocked linear FM |
| `m.S.T` | Mod S → T | −16…+16 | 0 | **radians of peak phase deviation** (the index), per unit of source level; negative inverts; S ≠ T |
| `fb.X` | Feedback | 0–4.0 | 0 | radians; DX 0–7 mapped by power-of-two steps |
| `out.X`, `pan.X` | Out, Pan | −∞…+6; ±100 | A 0 dB, others −∞; 0 | dB; % |
| `env.X.mode` | Envelope mode | Time / DX | Time | DX = R1–R4, L1–L4 0–99 |
| `env.X.p<n>.t`, `.lvl`, `.curve` | Breakpoint n (≤16) | 0–30,000; 0–1; −1…+1 | an ADSR shape: 5 ms / 1.0; 300 ms / 0.7; 250 ms release | ms; linear gain; bend |
| `env.X.sus`, `env.X.loop` | Sustain, loop start | point index / none | point 2 / none | |
| `env.X.key_time`, `env.X.vel_time` | Time scaling | 0–100; ±100 | 0; 0 | % per octave above C4; % |
| `ks.X.break`, `ks.X.l_depth`, `ks.X.r_depth`, `ks.X.l_curve`, `ks.X.r_curve` | Level scaling | MIDI 0–127; 0–48; 0–48; ±lin/±exp | 60; 0; 0; −lin | dB at ±2 octaves |
| `penv.*` | Pitch envelope | 4 points, ±48 st | flat | |
| `lfo1.*`, `lfo2.*` | LFOs | rate 0.01–50 Hz or sync; sine / tri / saw / square / S&H / noise; delay; key-sync | 5 Hz sine | per voice |
| `macro.bright` | Brightness | −100…+100 | 0 | %; every `m.*` × 2^(bright/50) |
| `macro.harm` | Harmonic | −100…+100 | 0 | %; moves the modulators' ratios toward the next integer or half-integer |
| `macro.detune` | Detune | −100…+100 | 0 | %; scales `op.*.detune` and `op.*.offset` |
| `macro.a`, `.d`, `.s`, `.r` | Envelope offsets | −100…+100 | 0 | %; applied to every operator envelope |
| `morph.x`, `morph.y` | Morph pad | 0–1 | 0, 0 | 4 corner snapshots of the `op.*`, `m.*`, `fb.*` and `out.*` values |
| `seed` | Seed | 0–2³²−1 | 1 | drift, mutations, fit |

**Editor layout** (the family frame from family.md §7.2: header 32 px, footer 28 px, 12 px padding; about 1180 × 740
at 100 %):

```
┌─ ● Keys 2 │ MODULUS │ ↶ ↷ │ A/B → │ ‹ Tine Piano ● › │ Δ │ ✎ Describe… │ ⏻ │ ? │ ⋯ │ ⛶ ─────────────────┐
├─ ALGORITHM ─────────── [Graph | Matrix | DX 1–32 ▾] ┬─ OPERATORS ──────────────────────────┬─ SOUND ─────────┤
│                                                     │ op wave   ratio     fixed  lvl env   │ spectrum (live) │
│   (F)──1.1──►(E)                 (B)◄─┐ fb 0.8      │ A  sine   1.0000      –    100 ▇▅▂  │ ▁▂▅▇▆▄▃▂▁   ┆Nyq│
│      ╲          ╲                 │ ╰─┘            │ B  sine  14.0000      –     62 ▇▂_  │ Carson ═════╪   │
│      2.4        0.6               1.2               │ C  sine   1.0000      –    100 ▆▅▃  │ guard ● Show    │
│        ╲          ▼                ▼                │ D  sine   1.0000      –     80 ▇▄▂  │ quality Auto ×2 │
│         ►(D)─0.9─►(C)             (A)               │ E  harm   3.5000      –     40 ▅▂_  │                 │
│            carrier ▼               carrier ▼        │ F  sine   0.5000      –     30 ▃▂_  │ voices ▮▮▮▯ 3/16│
│  ═══════════════ OUT (C −3 dB, A 0 dB) ════════════ │ G  –                                │ CPU 2.1 %       │
│  edge thickness = index; ⏱ = one-sample back edge   │ H  –                                │                 │
├─ OPERATOR B ─ [Envelope | Key scaling | Wave] ──────┴──────────────────────────────────────┴─────────────────┤
│  envelope (Time mode): ●───●╲___●─ ─ ─ ●  S▲ L▲  1/16 grid │ ratio 14.0000  offset 0 Hz  detune 0 ct  PM ▾ │
│  t: 1 ms · 180 ms · 1.2 s · rel 300 ms   curve −0.4        │ vel→lvl 70 %  key-time 20 %/oct  sync ✓      │
├─ MACROS ─────────────────────────────────────────────────────────────────────────────────────────────────────┤
│  (Bright +20) (Harmonic 0) (Detune 0) (Atk 0) (Dec −10) (Sus 0) (Rel 0)      ┌ morph ┐  [Mutate ▾] [Fit… ]   │
│                                                                              │ ·   ● │                       │
├─ KEYBOARD: active notes glow, bar per voice = current max index ─────────────└───────┘───────────────────────┤
└─ [Learn] · Quality: Auto · Unison 1 · MPE on · Bend 2 st · latency 0 samples · [Out −6.0 dB] · [size] ────────┘
```

- **The graph is the default view.**
  - Nodes carry their letter and ratio; edges carry their index as a number (never only a thickness).
  - Drag from node to node to connect; drag an edge's number to change it; Alt-click bypasses an edge.
  - Carriers sit on the bottom row by default.
- **Matrix** is a toggle for power users: an FM8-style grid with numbers and rings.
- **DX 1–32** applies a template, and **reports which cells it will overwrite before it applies them**.
- **The Operators column is Operator's shell**, extended to 8 rows: the numbers that matter, always visible.
- **The Sound column** holds the spectrum, the alias state, voices and CPU. State at rest; nothing hover-only.
- **✎ Describe…** in the header opens the AI prompt. Results arrive as one undo step and a diff ("changed 14
  parameters on B, D, A"), and can be A/B-compared.

**Firmus II engines, in brief** (the full parameter list belongs in the wavetable report's Firmus II proposal):
- ***Grana*:**

  | Parameter | Range | Default |
  |---|---|---|
  | source | sample / project clip | — |
  | position | 0–100 % | — |
  | scan | ±800 % (0 = freeze), key-track, sync | — |
  | density | 1–500 grains/s, sync, or count 1–64 | 40 /s |
  | length | 1–1000 ms, sync, or overlap × 1/density | 60 ms |
  | window | Hann, Gaussian, Tukey (width), Blackman-Harris, Expodec, Rexpodec, Trapezoid, Sinc; amount; skew | Hann |
  | spray | 0–1000 ms | — |
  | pitch | ±48 st | — |
  | pitch random | ±1200 ct, or intervals from the project scale | — |
  | reverse probability | 0–100 % | — |
  | pan spread | 0–100 % | — |
  | level random | 0–24 dB | — |
  | mode | Sync / Quasi / Async | — |
  | max grains | 1–128 | 64 |
  | seed | — | — |

- ***Partes*:**

  | Parameter | Range | Default |
  |---|---|---|
  | partials | 16–1024 | 256; culled at Nyquist with a 2 kHz taper |
  | source | drawn / wavetable frame / resynthesis | — |
  | stretch (inharmonicity) | `f_k = f0·k^(1+s)`, s ∈ ±0.1 | — |
  | tilt | ±12 dB/oct | — |
  | odd/even | ±100 % | — |
  | spectral filter on the partials | LP / BP / comb, with resonance shapes | — |
  | pluck | frequency-dependent decay | — |
  | blur | time and frequency | — |
  | phase | aligned / random / drawn | — |
  | resynthesis: FFT size | 2048 / 4096 / 8192 | 4096 |
  | resynthesis: hop | — | 256 |
  | resynthesis: mode | harmonic / free tracks | — |
  | resynthesis: residual noise level | — | — |
  | resynthesis: keep attack | 0–100 ms | — |
  | resynthesis: scan, formant shift, freeze | — | — |

---

## 9. Test plan hints

- **FM aliasing at high indices.**
  - Render a sine carrier at 1:1 while stepping the index from 0 to 12 rad, at notes C4, C6, C7 and C8, at 44.1, 48
    and 96 kHz.
  - Pick exactly periodic frequencies, `f0 = fs·cycles/N`, the trick in `osc.rs`'s saw test, so that **every bin
    that is not a harmonic is alias**.
  - Proposed budgets:
    - Auto quality with guard Show: alias ≤ −60 dB relative to the total up to index 8 at C7;
    - guard Tame: ≤ −80 dB at every note;
    - Eco 1×: no budget, but measured and published as a test artefact, as Aequalis does for its curves.
- **Carson monitor correctness.** The predicted edge must be ≥ the measured 99 %-energy frequency for a sweep of
  ratios and indices (a unit test of the guard's estimator).
- **Feedback stability.**
  - For β from 0 to 4 in steps of 0.1: output finite, `|y| ≤ 1` per operator, no DC runaway.
  - With the average on, the energy in the top 1 % of the band stays below the average-off case by at least 20 dB at
    high β (it proves the anti-hunt zero at fs/2 works).
- **DX envelope fidelity.**
  - Decay slopes for qrate 0–63 within 1 % of the published formula.
  - Attack jump of about 40 dB.
  - The floor at 3824 steps.
  - Level tables exact.
- **SysEx round trip.** Parse a cartridge, export it, and require byte-identical output with a correct checksum.
  Out-of-range bytes are clamped and reported.
- **Block-size independence.** Render the same MIDI with block sizes 1, 7, 64, 256 and 1024, and require
  **bit-identical** output. This covers control-grid glides (counted from the stream start), LFOs, envelopes,
  oversampling decimator state and **grain onsets**. Same harness as the existing null tests.
- **Determinism.**
  - Same patch and seed gives an identical render across runs and **across OSes** (no libm `sin` in the audio
    path).
  - Different note IDs get different grain and drift streams.
  - Changing the seed changes them.
- **Render equals playback.** An offline render and a real-time render through the engine give identical buffers
  at every quality setting. There are no draft/render differences.
- **Quality switching.** Changing `quality` mid-note affects only notes that start afterwards. No discontinuity in
  held notes (the maximum of the first difference stays under a threshold).
- **Sample-rate independence of timbre.** The spectral centroid of a fixed patch at 44.1, 48 and 96 kHz agrees within
  1 % below 16 kHz. This holds because indices are in radians and FM runs at a fixed internal rate.
- **CPU per voice (bench, not a unit test).** Publish a table:
  - FM: 8 operators at 1×, 2×, 4× and 8×;
  - Partes: 64, 256 and 1024 partials at A1, A3 and A5, showing that culling makes the cost follow pitch;
  - Grana: 16, 64 and 128 grains.

  Set a budget, for example 16 Modulus voices at Auto under 5 % of a reference core, and fail CI on a regression of
  more than 20 %.
- **Granular scheduling.**
  - The onset count over T seconds is `round(density·T) ± 1` in Sync mode, and Poisson-consistent in Async mode
    (chi-square test with a fixed seed).
  - Freeze (scan 0) with synchronous Hann grains at hop L/2 gives a constant envelope within 0.1 dB (COLA).
  - The max-grains cap skips spawns deterministically, without clicks.
- **Additive.**
  - A pitch bend sweeping partials past Nyquist produces no energy above the taper and no clicks.
  - Resynthesising a synthetic harmonic signal (known amplitudes) and subtracting gives a residual under −40 dB.
  - A recursive-oscillator bank drifts less than 0.01 dB in amplitude over 10 minutes (renormalisation works).
- **Undo fuzz.** Add Modulus parameters (matrix cells, envelope points, mode switches, morph, AI batches) to the
  40-seed undo/redo invariant walk. A sparse parameter that was never stored must undo to *absent*
  (`ClearEffectParam`-style), as for Aequalis.
- **Idle silence.** No notes gives exact zeros, and the smoothers snap (Firmus's rule), so idle time is also
  independent of block size.

---

## 10. Sources

**Manufacturers and manuals**
- Native Instruments, FM8 product page: https://www.native-instruments.com/en/products/komplete/synths/fm8/
- FM8 Operation Manual (PDF, linked from the product page): https://storage.googleapis.com/pdf-guides/FM8-book_EN_ebook.pdf
- KVR, FM8: https://www.kvraudio.com/product/fm8-by-native-instruments
- Image-Line, Sytrus (FL Studio online manual): https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Sytrus.htm
- Image-Line, Harmor (FL Studio online manual): https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Harmor.htm
- KVR, Sytrus: https://www.kvraudio.com/product/sytrus-by-image-line · Harmor: https://www.kvraudio.com/product/harmor-by-image-line
- Ableton, Live 12 Reference Manual, Live Instrument Reference (§31.9 Operator; manual images): https://www.ableton.com/en/manual/live-instrument-reference/
- Dexed repository and releases: https://github.com/asb2m10/dexed · https://github.com/asb2m10/dexed/releases
- Spectrasonics, Omnisphere product page: https://www.spectrasonics.net/products/omnisphere/
- Omnisphere 3 online manual, index: https://support.spectrasonics.net/manual/Omnisphere3/30/en/. Pages used (prefix `…/topic/`): `layer-page-oscillator-page29`, `…29a`, `…29b`, `…29c` (granular), `layer-page-oscillator-page15` (FM), `…25a` (Harmonia), `orb-index`, `layers`, `quadzone`, `hardware-integration`, `global-controls`
- u-he, Zebra 3: https://u-he.com/products/plug-ins/zebra3/
- Native Instruments, Absynth 6: https://www.native-instruments.com/en/products/komplete/synths/absynth-6/
- Absynth 6 User Guide, Patch page: https://docs.native-instruments.com/ni-tech-manuals/absynth-6-user-guide/en/patch-page
- Xfer Records, Serum 2 User Guide (manual 1.0.3, Serum 2.0.18, 27 Apr 2025), granular chapter pp. 85–103; support.xferrecords.com
- Arturia, Pigments: https://www.arturia.com/products/software-instruments/pigments/overview

**News and reviews**
- Sound On Sound, "Spectrasonics Omnisphere 3 arrives": https://www.soundonsound.com/news/spectrasonics-omnisphere-3-arrives
- MusicRadar, Omnisphere 3 announcement (search excerpt): https://www.musicradar.com/music-tech/spectrasonics-announces-omnisphere-3-the-ultimate-virtual-instrument-thats-been-ten-years-in-the-making
- BangerLoops, Omnisphere 3 review: https://bangerloops.com/articles/spectrasonics-omnisphere-3-review/
- Dubspot, u-he Zebra 3 review: https://blog.dubspot.com/u-he-zebra-3
- Synth Anatomy and gearnews on Zebra 3.0.1 (search excerpts): https://synthanatomy.com/2026/07/u-he-zebra-3-modular-synthesizer-plugin.html · https://www.gearnews.com/u-he-zebra-3-synthesizer/
- Sonicstate, "Absynth returns" (2025-12-09): https://sonicstate.com/news/2025/12/09/absynth-returns/
- Sound On Sound, "Native Instruments launch Absynth 6" (date from page metadata): https://www.soundonsound.com/news/native-instruments-launch-absynth-6
- gearnews, Absynth 6.1 (2026-04-20): https://www.gearnews.com/absynth-6-native-instruments-synths/
- rekkerd.org, FM8 Apple Silicon/VST3 update (search excerpt): https://rekkerd.org/native-instruments-updates-fm8-synthesizer-with-apple-silicon-and-vst3-support/

**Forums** (opinion; see the caveat in §6)
- KVR, "Is Harmor the best additive synth available?" (2020): https://www.kvraudio.com/forum/viewtopic.php?t=553024
- KVR, "Additive Synth VST/AU thread" (2022): https://www.kvraudio.com/forum/viewtopic.php?t=585257
- Gearspace, "u-he Zebra 3 released": https://gearspace.com/threads/u-he-zebra-3-released.1463514/ (HTTP 403, not read)

**DX7 hardware and literature**
- R. Levien et al., Music Synthesizer for Android wiki: Dx7Hardware and Dx7Envelope: https://github.com/google/music-synthesizer-for-android/tree/master/wiki
- K. Shirriff, "Yamaha DX7 chip reverse-engineering, part 4: how algorithms are implemented" (2021): http://www.righto.com/2021/12/yamaha-dx7-chip-reverse-engineering.html · part 5, output circuitry: https://www.righto.com/2022/02/yamaha-dx7-chip-reverse-engineering.html
- N. Tomisawa (Yamaha), US 4,249,447, "Tone production method for an electronic musical instrument" (feedback FM): https://patents.google.com/patent/US4249447
- J. Chowning, "The Synthesis of Complex Audio Spectra by Means of Frequency Modulation", JAES 21(7), 1973: https://yamahasynth.com/wp-content/uploads/images/fm_synthesispaper-2.pdf ; US 4,018,121
- **Cited from the standard literature. These papers were not re-read for this report; the citations are for the
  builder to follow:**
  - V. Lazzarini, J. Timoney, "Theory and Practice of Modified Frequency Modulation Synthesis", JAES 58(6), 2010
  - J. Schoffhauzer, "Synthesis of Quasi-Bandlimited Analog Waveforms Using Frequency Modulation" (2007)
  - A. Horner, J. Beauchamp, L. Haken, "Machine Tongues XVI: Genetic Algorithms and Their Application to FM
    Matching Synthesis", CMJ 17(4), 1993
  - X. Rodet, P. Depalle, "Spectral Envelopes and Inverse FFT Synthesis", AES 93rd Convention, 1992
  - R. McAulay, T. Quatieri, "Speech Analysis/Synthesis Based on a Sinusoidal Representation", IEEE TASSP 34(4),
    1986
  - X. Serra, J. O. Smith, "Spectral Modeling Synthesis", CMJ 14(4), 1990
  - T. Verma, T. Meng, transient modeling synthesis (2000)
  - J. Flanagan, R. Golden, "Phase Vocoder", Bell System Technical Journal, 1966
  - M. Dolson, "The Phase Vocoder: A Tutorial", CMJ 10(4), 1986
  - J. Laroche, M. Dolson, "Improved Phase Vocoder Time-Scale Modification of Audio", IEEE TSAP 7(3), 1999
  - A. de Cheveigné, H. Kawahara, "YIN", JASA 111(4), 2002
  - D. Gabor, "Acoustical Quanta and the Theory of Hearing", Nature 159, 1947
  - C. Roads, "Introduction to Granular Synthesis", CMJ 12(2), 1988, and *Microsound*, MIT Press, 2001
  - B. Truax, "Real-Time Granular Synthesis with a Digital Signal Processor", CMJ 12(2), 1988
  - R. Bencina, "Implementing Real-Time Granular Synthesis", *Audio Anecdotes*, 2001
  - X. Rodet, "Time-Domain Formant-Wave-Function Synthesis" (FOF), CMJ 8(3), 1984
  - E. Moulines, F. Charpentier, PSOLA, Speech Communication, 1990
  - P. Valenzuela, A. Constantinides, "Digital Signal Processing Schemes for Efficient Interpolation and
    Decimation", IEE Proc. G, 1983
  - J. Carson, 1922 (FM bandwidth rule)
