# Bass instruments: competitive research for the Cantus bass

*Researched 2026-09-24 for Cantus. Sources: the manufacturers' product pages and manuals (IK Multimedia,
Spectrasonics, Future Audio Workshop, Initial Audio, Image-Line, Ample Sound, Toontrack), press reviews (Sound on
Sound, MusicRadar/Computer Music, Rekkerd), one KVR forum thread, and the primary DSP literature (Karplus & Strong,
Jaffe & Smith, Karjalainen/Välimäki/Tolonen, Rank & Kubin, Paiva/Pakarinen/Välimäki, Bilbao & Torin, Werner/Abel/Smith,
Szabo). Everything is paraphrased. Where a claim comes only from a search-engine excerpt and not from a page I could
read, it is marked **[excerpt]**. Where a number is my own calculation, it says so. Gearspace and Sweetwater
returned HTTP 403 and were not retried or worked around. The session's web-search budget ran out part-way, so the
last third of the literature was read from known URLs directly; a few secondary citations below come from the
reference lists of papers I did read and are marked **[cited in]**.*

*Screenshots are the manufacturers' own images, saved in [`screenshots/`](screenshots/) (git-ignored) for internal
reference only.*

*Cantus context read first: `research/instruments/README.md`, `research/fabfilter/family.md` §0 and §7,
`research/fabfilter/pro-q.md` §0 and §11, `crates/cantus-dsp/src/firmus.rs`, the kick in
`crates/cantus-dsp/src/pulsus.rs`, `crates/cantus-core/src/instrument.rs` and `snapshot.rs`.*

---

## 0. The short version

1. **The market splits into three kinds of bass, and no product does all three well.** There are
   modelled or sampled *instruments* (MODO BASS 2, Trilian, Ample Bass, Scarbee, EZbass), *808/sub designers*
   (SubLab XL, 808 Studio) and *synth basses* (Transistor Bass for 303 lines; Serum-class synths for reese and wobble).
   Trilian comes closest to covering everything, with a 34 GB sample library from 2010 on the STEAM engine. Nobody
   combines a physical model, an 808 and a synth bass behind one voice and one output stage.
2. **MODO BASS 2 is the benchmark for the modelled bass.** IK describes it as modal synthesis **[excerpt]**, with the
   player's action modelled as a nonlinear contact. It exposes the physics as controls: 22 models, 4/5/6 strings,
   string type, gauge and age, action, scale, fretless, 32 pickup models that you can move, a piezo, passive or active
   circuits, finger/pick/slap with a velocity threshold for the pop, muting, detach and slide noise. It also has a
   *playing position* marker you drag on a photograph of the bass. Reviewers call it indistinguishable from the real
   thing (SOS, 2022). The main complaint is occasional stutter and note lag in fast playing (MusicRadar).
3. **The DSP for a first-class modelled bass is well documented and affordable.** Use a bidirectional digital
   waveguide per string, with:
   - a fractional delay for tuning;
   - a one-pole loss filter;
   - a dispersion allpass for stiffness;
   - pluck and pickup position as comb filters;
   - a nonlinear magnetic-pickup model;
   - *fret collisions as conditional reflections*, which is what makes slap sound like slap (Rank & Kubin 1997).

   Rank and Kubin needed about 3,200 operations per sample on the fingerboard, and placing the limiters only at the
   frets cut that by about 8× with no audible loss. For a mono bass that is trivial today.
4. **The 808 is a solved sound with unsolved ergonomics.** Every product offers the same building blocks: a sine,
   a pitch envelope, glide on overlapping notes, distortion, a clean sub path (SubLab's X-Sub bypasses the filter,
   envelopes and distortion) and a sidechain compressor. None of them knows the song's key or the kick. Every glide
   depends on the user drawing overlapping notes. FL Studio's manual says outright that its own portamento and
   overlapping-note slides are **not supported for VST/AU instruments**, so each plug-in reinvents them.
5. **The kick–bass relationship is handled today by a pile of separate plug-ins and routing**, all guessing at
   information the host already has:
   - volume shapers or sidechain compressors for ducking;
   - Trackspacer-style spectral ducking;
   - Sound Radix Pi for phase interaction.

   Cantus's snapshot already holds every track's sorted note events. The bass can therefore duck *before* the kick
   arrives, with zero latency, tune the kick and the 808 to each other in one undo step, and draw the kick's hits
   in its own display.
6. **Articulation control is the other shared weakness.** MODO's Control page is a 28-row table of keyswitches and
   CCs. Ample Bass shipped a whole piano roll *inside the plug-in* (Riffer 4) just to show which string each note is
   on. Cantus owns the piano roll: string lanes, slide notes and articulations can be note properties, with no
   keyswitches.
7. **Proposal: one instrument, *Gravis*, with three engines (String, 808, Synth).** All three share:
   - a mono/legato voice with a note stack and glide;
   - a clean, phase-locked sub path;
   - a fundamental-locked drive with key tracking;
   - kick-aware ducking;
   - a "mono below" crossover.

   It is deterministic and block-size independent, and it follows the Cantus family frame (family.md §7). Its P0 is
   parity with MODO, SubLab and Transistor Bass on sound. Its P1 is what only a host can do.

---

## 1. Products and versions

| Product | Latest version and date | Price (list) | Formats | Kind | Source |
|---|---|---|---|---|---|
| **IK Multimedia MODO BASS 2** | 2.x; MODO BASS 2 launched Sept 2022 (MusicRadar review dated 2022-09-05). The latest point release was not verified. | SE £/$149.99, full £/$299.99 at launch (SOS). IK's own page now lists different tiers, and a free CS edition with one model (the '60s P-Bass). IK discounts often. | VST2/VST3/AU/AAX + standalone (IK standard; formats not listed on the page I read) | Physical model (modal **[excerpt]**) | ikmultimedia.com/products/modobass2; SOS; MusicRadar |
| **Spectrasonics Trilian** | 1.6.x (the online manual is titled "Trilian 1.6"; the product pages still describe 1.5). Native Apple Silicon and VST3 through free updates. First released around 2009–10 (SOS review Jan 2010). | $299 (download or boxed); Trilogy upgrade $99 | VST3/VST2/AU/AAX, 64-bit, standalone | Sampled (34 GB core, 112,997 samples per SOS) + STEAM synth engine | spectrasonics.net; SOS 2010 |
| **Future Audio Workshop SubLab XL** | XL released 2022-07-30 (SOS news). Original SubLab reviewed July 2019. | $80 list, $50 intro. Sales of $25–39 seen in 2026 (Bedroom Producers Blog; FAW shop). | VST2/VST3/AU/AAX; macOS and Windows | 808/sub: Synth + Sampler + X-Sub engines | futureaudioworkshop.com; SOS news; Rekkerd |
| **Initial Audio 808 Studio** (2 and 3) | A "808 Studio 3" page exists, but its details could not be read. Features below are from the 808 Studio 2 page and its screenshot. | 808 Studio 2: $69 **[excerpt]** | VST/AU (Initial Audio standard; not verified) | 808 synth + sampler + sequencer | initialaudio.com |
| **Image-Line Transistor Bass** | Ships with FL Studio (introduced around FL 12.5, early 2017, per a blog post). Edition availability not verified. | Included with FL Studio | FL-native | TB-303-style mono synth + 16-step sequencer | Image-Line manual |
| **Ample Sound Ample Bass** (P, J, JF, Yinyang, TR6, AMR, U, A) | **v4.1, 2026-03-12** (chord system, auto-vibrato curve, swing, MIDI time-stretch); v4 on 2025-04-18 (Riffer 4, Tab Reader 4, "5K UI") | Per model; not verified. Free "P Lite" v4 edition. | VST2/VST3/AU/AAX + standalone; 20 GB+ | Sampled | KVR news (v4, v4.1); amplesound.net |
| **NI Scarbee Pre-Bass** (and MM-Bass, Jay-Bass, Rickenbacker) | Kontakt library, long established | not checked | Kontakt | Sampled: 4,000+ samples, 9 velocity layers **[excerpt]** | KVR, NI product pages **[excerpt]** |
| **Toontrack EZbass** | Reviewed by SOS in July 2020; the current version was not verified | not checked | VST/AU/AAX + standalone | Sampled + songwriting (grooves, Tap2Find, drum-MIDI drop zone, Audio Tracker) | SOS 2020; toontrack.com **[excerpt]** |
| **Xfer Serum 2 / Vital** (context only: the reese and wobble engines) | see `wavetable.md` | — | — | Wavetable synths used for reese, wobble and 808 | Output blog 2026 **[excerpt]** |

---

## 2. Screenshots saved

All files are in [`screenshots/`](screenshots/). Each one was checked as a real JPEG or PNG and viewed.

| File | What it shows | Notes on the design |
|---|---|---|
| `bs-01-modobass2-overview.jpg` | Composite of three MODO BASS 2 screens (Upright amp page, Model page, Studio) | The model picker is a horizontal strip of photos. The spec sheet sits on the left. The whole family shares one frame. |
| `bs-02-modobass2-playstyle-slap.jpg` | **Play Style page**: Finger/Pick/Slap icon buttons; Muting 50%; Stroke AUTO; **Threshold 120** (the velocity that switches thumb to pop); Let Ring; Fingering "First Position"; Open String; Detach Noise; Slide Noise. The orange **position marker "9.00″"** sits on the body photo. | The value is printed *above* each knob and is always visible. The playing position is a draggable marker on the photo, labelled in inches. |
| `bs-03-modobass2-strings.jpg` | **String page**: Fingerboard (fretted/fretless), Strings (6), Drop, Action, Type (Round Wound), Gauge, Age, A4 reference | Every setup choice is a boxed button that cycles through a list. The spec sheet on the left mirrors them. |
| `bs-04-modobass2-electronics-fretless.jpg` | **Electronics page**: Bridge and Neck pickup model dropdowns (18 listed) with power toggles and volume, Piezo, Circuit (Passive), Tone, and Bass/Middle/Treble greyed out while passive. **Dimension lines drawn on the body show the pickup distances (2.36″, 6.25″)**. | This is the closest any product comes to "the drawing is the editor" for a physical model. Disabled controls are dimmed rather than hidden. |
| `bs-05-modobass2-control.jpg` | **Control page**: 28 rows of Type (KS/CC/PW) × Value × Latch. Pizzicato, Pick, Slap, Bend, Slide, Vibrato, Hammer-on/Pull-off, Legato Slide, Muting, Ghost, Harmonics, Let Ring, Index/Middle stroke, Left-hand position, Force C/G/D/A/E/B string, Pluck position, Chord mode, Stop on detach, DB slap noise, DB stop strings; Vibrato rate 3.0, Slide range 6 | The articulation system as a spreadsheet: MIDI-era plumbing exposed to the user. This is the gap §7 targets. |
| `bs-06-modobass2-mic-upright.jpg` | **Mics page (upright)**: Condenser 87, Volume, X, Y, Distance, Piezo, Room, Width, Mics, Stomp Out, Amp; a photo of a studio with the mics placed | The mic position is set with X/Y knobs even though the scene is drawn: the picture does not act as the editor here. |
| `bs-07-trilian-main.jpg` | Trilian 1.5+: patch browser (Attributes / Sound Match, Category/Type/Model/Genre); red-lacquer frame; **Controls page**: C-12 Mic, Pickup, Noises, Arpeggiator, Velocity, Compressor Amount, Humanize, 4-band EQ, Octave; tabs Main/A/B/FX/Arp; 8 parts + Multi | A skeuomorphic 2010 design with orange LED-ring knobs. The mix of mic, pickup and noise is the main tone control. |
| `bs-09-trilian-arp.jpg` | Trilian arpeggiator: step bars with per-step modifiers (Chd, ++, ---, +3, Hi, Lo, slide/ratchet icons, V1), Legato mode, Latch | Per-step *modifier chips* above the bars; a dense but readable design. |
| `bs-10-808studio2-main.png` | 808 Studio 2: preset list; Sequencer; Sampler (Follow Pitch, Loop, Retrig); Sub Osc (Octave, Drive, Gain, Fat); Osc 1/2; Filter; Master envelope + 2 mod envelopes + 2 LFOs; Filter HP/LP; Compressor; Overdrive/Distortion; 3-band EQ graph; Chorus (with a low-pass so only the top is widened); Velocity, **Portamento Time**, Pitchbend range; **Slide: Slide Time, Slide Octaves, Slide Curve**; **Sidechain: Threshold, Release, Reduction** | Dark charcoal with a yellow accent. Twenty small panels on one screen, with labels at about 8–9 px: dense and hard to read. |
| `bs-11-transistorbass-main.png` | Transistor Bass: Tuning, 303 pulse, Waveform, Cutoff, Reso, Env Mod, Decay, Accent, Volume; Tweaks (HP, Min Decay, Key Follow, LFO→PW, LFO rate, PW, VCA smooth); Distortion; Delay; Reverb; 16-step sequencer (pitch/gate/slide/accent) | Light brushed-metal skeuomorphism. The step sequencer is exemplary: gate, slide and accent are rows of toggles. |
| `bs-12-transistorbass-sequencer.png` | Transistor Bass sequencer in octave view: a 12-row pitch grid × 16 steps, plus Octave/Gate/Slide/Accent rows | Proof that "slide" and "accent" are best edited as per-step flags beside the notes. |
| `bs-13-sublabxl-macro.png` | SubLab XL **Macro page** inside Ableton: purple background, a 3-D blob, two yellow macro handles "A Drive" and "B Impact" on arcs, preset bar "N95 · My Subs" | A performance view with two big handles. High contrast, very little text. |
| `bs-14-sublabxl-fx-modules.png` | SubLab XL FX modules (Distortion "Grunge", Tape): **a two-handle range slider for the lo/hi-cut band**, Drive/Fizz/Gain knobs, chips at the foot for macro amount and mix % | The range-slider band is a strong idiom for "saturate only this band". |
| `bs-15-sublabxl-synth-superosc.png` | SubLab XL Sound section: tabs Synth / Sample / X-Sub; large waveform view; Sine/Triangle/Saw/Square/Super; Octave "0 oct", Tune "0 st"; the Super Oscillator popover (Layout 12 (2×), Unison 2 oct, Detune, Mix −3.0 dB, a Width wedge) | Very large numeric readouts with small units. The popover keeps rarely used options out of the main view. |
| `bs-16-sublabxl-mod-glide-tabs.png` | SubLab XL Mod section: tabs Vol / Pitch / Filter / LFO / **Glide**; Rate 0.4 Hz, BPM toggle, Fade In 182 ms; waveform chooser; Destination 1 "Mixer X-Sub 25%", Destination 2 "Filter Cutoff 35%" | Glide is treated as a *modulation page with a drawn curve*. Each section has its own hue (green for sound, pink for modulation). |
| `bs-17-808studio-v1-ignite.png` | The original 808 Studio (the Ignite VST era), a 3-D render: Kick, Sub Osc, Amp (Drive, Distortion), Compressor with Sidechain, Global (Level, **Glide**, High Cut), **Pitch Slide (Speed, Shape, Range)**, Chorus, EQ | For history: glide *shape* was a control from version 1. |
| `bs-18-amplebass4-banner.jpg` | Ample Bass v4.1 family banner: a bass photo strip with fret markers and a tooltip "String 3, Fret 6, D#2"; the Riffer piano roll with multicolour CC curves; a tab view | The played position is shown on the neck at all times. |
| `bs-19-amplebass-riffer-string-lanes.jpg` | **Ample Riffer**: a piano roll in which the notes on each string are joined by a coloured curve, string numbers 1–4 appear on the keyboard, and there are CC lanes (Bend, Mod, Velocity, Hold, Volume) and a toolbar | The best existing picture of *string assignment inside a note editor*. Cantus's piano roll can do this natively (§8). |
| `bs-20-amplebass-p-lite-main.jpg` | Ample Bass P Lite main page: the bass photo, "String 4, Fret 0, E1", faders DI/ST/Master/Pan/REL/FX/FSR with numeric values, a keyboard with coloured keyswitch zones | The faders show their values at rest; blue on black. |

---

## 3. Anatomy of each product

### 3.1 IK Multimedia MODO BASS 2 (the physical-model benchmark)

- **Engine.** IK describes the method as modal synthesis **[excerpt: KVR/gearnews announcement text]**:
  - each string is a *nonlinear resonator* whose behaviour comes from its physical parameters and from its
    interaction with the fretboard, body and pickups;
  - the player's action is a nonlinear physical interaction on an area of the string;
  - IK says the work took 8 years.
- **Models.** 22 in total:
  - 14 original models: '60s/'70s P, '70s/Modern J, Rick-style, Devil, Bass Man 5, Studio, Violin, Thunder, Japan,
    Flame, Imperial, Metal;
  - 8 new in v2: Fretless Jazz, Fretless Bass Man, Studio Upright, Rockabilly (upright, for old-school slap), Horn,
    Punk, Fusion J, Aluminum.

  Any model can be made fretless (not in CS).
- **Page structure** (7 tabs):
  - **Model**: the photo strip, the spec sheet and Reset to Default.
  - **Play Style**:
    - technique: Finger (variable force, alternate or index/middle, three touch weights per SOS), Pick (thickness and
      stroke direction), Slap (thumb and pop, with a velocity *Threshold*);
    - Muting, Let Ring, Fingering (e.g. First Position), Open String, Detach Noise, Slide Noise;
    - the *playing position* marker on the photo (e.g. 9.00″). Near the bridge is brighter; near the neck gives
      more lows and mids.
  - **String**:
    - Fingerboard, Strings 4/5/6, Drop tuning, Action;
    - Type (round, flat and others), Gauge, Age;
    - A4 reference;
    - scale length (per the product page).
  - **Electronics**:
    - up to two pickups chosen from 32 models, *freely positioned*, plus a piezo under the bridge;
    - Passive/Active circuit, Tone, and an active 3-band EQ.
  - **Studio**: 7 stomp effects (octaver, distortion, chorus, compressor, delay, envelope filter, graphic EQ), a
    tube 1×15 or solid-state 4×10 amp, and routing to AmpliTube 5. On uprights this page becomes **Mics**
    (condenser/ribbon, X/Y/Distance, piezo, room).
  - **Patterns**: more than 1,500 MIDI grooves with smart filters (genre, section, length, time signature),
    dragged into the DAW; pattern import is not in CS.
  - **Control**: remappable keyswitches/CCs (see bs-05), vibrato rate and slide range.
- **Performance.** Keyswitches or CCs for technique, bend, slide (by pitch wheel), vibrato, hammer-on/pull-off,
  legato slide, ghost notes, harmonics, forcing a note onto a string, and chord mode. The bottom strip is a fretboard
  that shows where each note is played, above a keyboard with the keyswitch zones coloured.

### 3.2 Spectrasonics Trilian (the sampled "total bass")

- **Content.**
  - About 24 electric basses (4/5/6/8-string, including Chapman Stick) played fingered, picked, fretless,
    slapped, tapped and muted;
  - a deep-sampled upright with 4 audio channels and more than 21,000 samples;
  - synth basses sampled from Minimoog, Prophet-5, Voyager, the Juno line and others;
  - 34 GB and 112,997 samples in the core library (SOS).
- **Articulations** (SOS 2010, read; plus a search excerpt of SOS review text **[excerpt]**): sustains and
  staccatos across up to six dynamics, slides up, down and up-and-down, glissandi, harmonics, squeaks, "X-notes",
  round robin on every note (**"up to 16×" [excerpt]**), and pitched release samples. The flagship "Studio Bass" is
  a Music Man 5-string Bongo **[excerpt]**.
- **Legato.** "Use Legato SS" switches to legato samples for intervals within a chosen range (1–24 semitones).
  The instrument turns *quasi-monophonic* inside that window, so a new note cuts the previous one, while staying
  polyphonic overall. Release noise is a separate layer with a level (about −3 dB recommended), a velocity/volume
  **mode by note length**, key tracking, and a *transition time* crossfade from the sustain.
- **Engine.** STEAM (shared with Omnisphere):
  - 2 layers (A/B) × 2 filters from 19 types;
  - 33 effects;
  - an arpeggiator with per-step modifiers and pitch slides;
  - it works as a satellite inside Omnisphere.
- **UI.** A per-patch Controls page (mic/pickup/noise mix, compressor, humanize, EQ, octave), with deeper pages
  behind it.
- **Criticisms** (SOS 2010): the browser covers the controls; no user sample import; it was plug-in only at the
  time (standalone came later).

### 3.3 Future Audio Workshop SubLab XL (the 808/sub designer)

- **Three sources.**
  - **Synth**: sine, triangle, saw, square and **Super** (supersaw with layouts, unison, detune, mix and width).
  - **Sampler**: 250+ kicks, kick transients and 808s from Roland, Jomox, Vermona, DSI, Korg, Waldorf, Intellijel
    and others. It follows pitch and has loop and crossfade controls; you can drop in your own samples.
  - **X-Sub**, the "psychoacoustic" sub oscillator:
    - Rekkerd describes it as lows up to about 65 Hz plus generated harmonics;
    - FAW says it is always *dry*: it bypasses the filter, envelopes, distortion and compressor so the sub stays
      consistent;
    - you "control the exact harmonics".
- **Processing.**
  - Distortion types: TubeA/TubeB (Heat), Orange (Dirt), Darkdrive (Fatness), Overdrive (Tone), Grunge (Fizz), with
    a visualiser, **lo/hi-cut around the distortion** and a mix for parallel distortion;
  - waveshaper, bitcrusher, tape;
  - compressor with sidechain and mix;
  - limiter/maximiser;
  - EQ;
  - a display that overlays the Synth, Sample and X-Sub frequencies.
- **Modulation.** Pages for Vol, Pitch, Filter, LFO and Glide. The LFO has two destinations from 18 targets.
  Glide has **editable curves and BPM-synced times**, from instant to a full bar, with a legato mode.
- **Tuning.** Root note, fine tune, global tuning and "tuning scale settings".
- **Macros.** Two macros (Drive, Impact) mapped for every preset; up to 10 parameters per macro, assigned by drag
  and drop.
- **Presets.** Artist "Bass Packs" (Richie Souf, Reese, Crushed, Analog Machines…).

### 3.4 Initial Audio 808 Studio 2 / 3

From the page and screenshot for version 2 (bs-10). The v3 details could not be read.

- **Sources.** Sampler (Follow Pitch, Loop, Retrig), Sub Osc (Octave, Drive, Gain, "Fat"), and Osc 1 and 2 with
  Octave and Gain. The filter has Drive, Cutoff and Resonance.
- **Modulation.** A master ADSR, 2 modulation envelopes and 2 LFOs.
- **Processing.**
  - HP and LP filters, a compressor and overdrive/distortion;
  - a 3-band EQ with a graph;
  - a chorus that only widens the top (it has a low-pass on its input), which keeps the sub mono **[excerpt]**;
  - a master section.
- **Glide.** *Portamento Time*, plus a **Slide section: Slide Time, Slide Octaves (range), Slide Curve**.
- **Sidechain.** Threshold, Release and Reduction.
- **Sequencer.** Overlapping notes produce glides; 80 presets **[excerpt]**.

### 3.5 Image-Line Transistor Bass (the 303)

- **Engine.** A mono subtractive synth modelled on a real TB-303:
  - Waveform blends square to saw; a "303 Pulse" mode uses the hardware's pulse shape (the hardware width is about
    66%);
  - Tuning ±1 octave;
  - Cutoff and Resonance (accent also modulates resonance);
  - Env Mod, Decay, *Min Decay* (which sets the accented decay);
  - Accent;
  - Tweaks: HP, Key Follow ±100, LFO→PW, LFO rate, PW, *VCA Smooth* (de-click);
  - distortion (pre-HP, drive, tone), delay and reverb.
- **Accent.** Triggered by internal steps, or by piano-roll velocity **≥ 86% (MIDI 100)**. It raises cutoff,
  resonance and level at once.
- **Slide.** Uses slide notes *or overlapping notes* from the piano roll, or the Slide row in the sequencer.
- **Sequencer.** 1–16 steps, shuffle, gate length (default 50%), per-step Octave/Gate/Slide/Accent (keys G, S, A),
  128 sequences and 128 patches, Auto Follow program changes.
- **The TB-303 accent circuit it emulates** (Robin Whittle, Devil Fish notes): the accent voltage passes through
  a diode–resistor–1 µF capacitor "accent sweep" network. The capacitor does not fully discharge between close
  accents, so **consecutive accents climb in pitch**, which is the "wow" of acid lines. Accented notes use the
  shortest decay setting.

### 3.6 Sampled comparisons: Ample Bass, Scarbee, EZbass

- **Ample Bass v4 / v4.1.**
  - 8 bass models.
  - 13 articulations: sustain, palm mute, slides, slap, pop, tap, dead notes and others.
  - A polyphonic legato and slide system.
  - **Riffer 4**: a MIDI editor with a piano-roll view and a guitar-tab view, a "String Visualization System" that
    prevents two notes sounding on one string and assigns fingering intelligently, and colour-coded CC lanes shown
    together.
  - Tab Reader imports Guitar Pro 3–8 with fingering and articulations.
  - v4.1 added a chord system for non-strummed instruments, one-click auto-vibrato curves, swing quantise and MIDI
    time-stretch.
  - Virtual amps (B15R, BA500, SVT-Pro) and cabinets (1×15, 2×10, 4×10, 8×10) with 4 mics.
- **Scarbee Pre-Bass** **[excerpt]**. A Kontakt instrument of 4,000+ samples, 9 velocity layers, all four
  strings. Articulations are played *without keyswitches*: legato triggers hammer-ons and pull-offs, and fast legato
  triggers grace notes. A fretboard overview shows the string, fret and an articulation symbol for each note as it
  plays.
- **EZbass** (SOS 2020).
  - Two 5-string basses (Vintage and Modern), fingered and picked, with pops, slaps, slides, ghosts and harmonics.
  - Songwriting tools:
    - a **drum-MIDI drop zone** and **Tap2Find** suggest matching grooves;
    - drop in drum or keyboard MIDI and a matching bassline is made **[excerpt]**;
    - an **Audio Tracker** turns mono bass or guitar audio into MIDI;
    - a Song Track with sections, chords and grooves;
    - a Grid Editor.
  - The review found no true fretless.

### 3.7 How producers build reese and wobble in Serum-class synths

Wavetable synths are covered in `wavetable.md`. For bass, the working recipe that reviews and tutorials describe
**[excerpt / general practice]** is:
- 2–4 saws detuned by a few to a few tens of cents, often with unison;
- a *separate clean sine sub* one octave down;
- the detuned layer high-passed above roughly 100–150 Hz so its beating does not make the fundamental pump;
- a low-pass filter moved by an LFO synced to 1/4, 1/8 or triplet values (the "wobble"), then distortion and OTT.

The physics behind this is in §4.4.

---

## 4. The DSP

### 4.1 Plucked-string physical modelling

**Karplus–Strong (KS).** Karplus & Strong (CMJ 7(2), 1983) fill a delay line of length N with noise and feed its
output back through a two-point average. The pitch is about fs/(N + ½). It is the special case of the "single
delay loop" (SDL) string model in which the loop filter is (1 + z⁻¹)/2 and the pluck is the initial state
(Karjalainen, Välimäki & Tolonen, CMJ 22(3), 1998; KVT98 below).

**Jaffe & Smith's extensions** (CMJ 7(2), 1983), as summarised by J. O. Smith, *Physical Audio Signal Processing*
(PASP), "Extended Karplus–Strong". There are six filters in the loop or at the input:

| Filter | Form | Purpose |
|---|---|---|
| Pick direction | H_p(z) = (1 − p)/(1 − p z⁻¹), 0 ≤ p < 1 | Softer or harder attack, or up/down stroke |
| Pick position | H_β(z) = 1 − z^−⌊βN + ½⌋, 0 < β < 1 | Comb notches at harmonics n where nβ is an integer |
| String damping | H_d(z), one or two poles/zeros, \|H_d\| ≤ 1 | Frequency-dependent decay |
| String stiffness | allpass H_s(z) | Dispersion, so the partials are stretched |
| Tuning | first-order allpass (−η + z⁻¹)/(1 − η z⁻¹); η from about −1/11 to 2/3 gives 1.2 to 0.2 samples of delay | Fractional part of the loop delay |
| Dynamic level | H_L(z) = (1 − R_L)/(1 − R_L z⁻¹), R_L = e^(−πLT) | Velocity → brightness |

J&S also give **decay stretching**, y[n] = (1 − S) y[n−N] + S y[n−N−1], which slows the decay of low notes.

**From SDL to the bidirectional waveguide** (KVT98). An ideal string is two delay lines carrying waves in opposite
directions (d'Alembert), reflected with inversion and slight low-pass loss at each end.
- With a **bridge-force output**, the model reduces to: excitation-position comb → wave propagation → string loop
  S(z) = 1/(1 − H_loop(z)) → bridge filter.
- With a **velocity pickup** (magnetic), the output is the *sum* of the two travelling waves at the pickup point.
  That adds **a second comb filter** (pickup position) and a pickup filter P_o(s), "typically a second-order
  low-pass". An electric bass therefore has *two cascaded combs*, pluck and pickup, where an acoustic has one.

The SDL is the cheapest form. **KVT98 is explicit that nonlinear or time-varying string behaviour needs the
bidirectional form.** Fret collisions, slap and slides are exactly that, so the bass engine should be
bidirectional.

**Tuning numbers** (my calculation).

| Note | f0 | Loop length at 48 kHz |
|---|---|---|
| E1 (lowest 4-string note) | 41.203 Hz | 1,164.96 samples |
| B0 (5-string) | 30.868 Hz | 1,555.0 samples |
| G4 (24th fret, G string) | 392 Hz | 122.4 samples |
| C5 (6-string, 24th fret) | 523.25 Hz | 91.7 samples |

Rounding the delay by half a sample costs 0.74 cents at E1 but **9.4 cents at C5**, so a fractional delay is
mandatory.

The loop delay must also subtract the **phase delay at f0** of:
- the loss filter (a one-pole g(1 + a)/(1 + a z⁻¹) has a fraction of a sample);
- the dispersion allpass cascade, which can be tens of samples.

A stiff string's partials are sharp. The usual convention is to tune the *fundamental* to the note.

**Fractional delay** (Laakso, Välimäki, Karjalainen & Laine, "Splitting the unit delay", IEEE SPM 1996 **[cited in]**
KVT98):
- a **first-order Thiran allpass** is ideal for static pitch (flat magnitude);
- it produces transients when its coefficient changes quickly, i.e. during slides, vibrato and glides. **Lagrange
  3rd-order (FIR) interpolation** (or Hermite) behaves better under modulation, at the cost of slight high-frequency
  loss.

Recommendation: Lagrange-3 in the loop (it modulates cleanly), with the loss filter retuned to absorb its small
high-frequency droop.

**Loss filter.** The standard choice is a one-pole with a DC gain g and a cutoff coefficient a (J&S; Välimäki et
al., JAES 44(5), 1996 **[cited in]** KVT98). Loop gain must be ≤ 1 at every frequency for stability. The two
parameters map to "sustain" (T60 of the fundamental) and "brightness decay" (T60 of the high frequencies).

**Dispersion (stiffness).** Partial n of a stiff string is at f_n = n f0 √(1 + B n²), with inharmonicity
coefficient B = π³ E d⁴ / (64 T L²) (Fletcher's formula; standard textbook physics, stated here from knowledge
because the PASP page I read did not give it).

For a roundwound bass E, only the core wire gives stiffness. With a core of 0.9–1.1 mm, steel E = 200 GPa,
T ≈ 196 N and L = 0.864 m, B comes out at about **4×10⁻⁴ to 1×10⁻³** (my calculation; not measured). At B = 5×10⁻⁴:
- the 10th partial is about **+42 cents** sharp;
- the 20th is about **+157 cents** sharp.

That is the metallic "clank" of a fresh roundwound. Flatwounds, and dead strings, sound "thumpier" because they
lose high frequencies faster, which is a loss-filter effect as much as a dispersion one.

Dispersion is modelled by lumping an **allpass cascade** at the termination; it commutes with the delay line (PASP,
"Stiff string synthesis models"). Design methods:
- Van Duyne & Smith (ICMC 1994): cascaded identical first-order allpasses;
- Rauhala & Välimäki (IEEE SPL 2006): tunable dispersion filters;
- Abel, Välimäki & Smith (IEEE SPL 2010): robust allpass design for dispersive strings.

These three are cited from knowledge; they were not re-read this session. Piano tuning shows how much this
matters: PASP quotes a total stretch of about 35–60 cents across real pianos.

**Pluck position.** The comb 1 − z^(−βN) puts notches at harmonics n = k/β. KVT98 Fig. 6 shows this: plucking at
50% removes the even harmonics, and plucking at 22.5% removes harmonic 4.44 and its multiples.

On a 34″ (864 mm) bass, MODO's default of about 9″ (229 mm, β ≈ 0.265) puts the first notch near harmonic 3.8.
For E1 that is about 156 Hz (my calculation).

**Pickup position and aperture.** A pickup at distance d from the bridge notches harmonics at multiples of L/d.
Using the distances drawn on MODO's electronics page (bs-04):
- **2.36″ (60 mm)** puts the first notch at harmonic 14.4, about 594 Hz for E1: a bright, "bridge" tone;
- **6.25″ (159 mm)** puts it at harmonic 5.4, about 224 Hz: warm, "neck".

Rank & Kubin note that sensing a single point sounds unrealistically bright. A real magnetic field is wide, and a
**Gaussian-weighted tap** of the delay-line sum around the pickup position gives the right high-frequency damping.
That is the "aperture". A humbucker, with two coils, is two taps.

**Magnetic pickup model** (Paiva, Pakarinen & Välimäki, "Acoustics and modeling of pickups", JAES 60(10),
2012). The chain is:
1. a nonlinear map from string displacement to flux;
2. a time derivative, which gives the voltage;
3. the resonant low-pass formed by the coil's equivalent circuit.

Single-coil, humbucker and rail pickups produce measurably different distortion. The nonlinearity grows as the
string gets closer to the pole piece, so *pickup height* is a real parameter. Its effect is strongest on slap and
hard plucks. The resonance is loaded by the tone pot and cable, so passive "Tone" is a variable RC low-pass, and an
"Active" circuit buffers the pickup and adds an EQ.

**Fret collisions, buzz and slap** (Rank & Kubin, "A waveguide model for slapbass synthesis", ICASSP 1997;
paper read in full).
- **State variable.** Use *displacement* as the waveguide variable, so the string's position is simply the sum of
  the two waves at a point.
- **Limiting two-ports.** Put "limiting" two-ports along the fingerboard section, about 2/3 of the string. If the
  local displacement goes past the fret or fingerboard (y_fret(x) < 0), the excess is **reflected into the opposite
  delay line, inverted, with the fret distance added**. That is a *conditional reflection*.
- **Fingerboard geometry.** A linear y_fret(x), set by the string height at the body end and at the head end,
  i.e. the action.
- **Excitations.**
  - **Pop** (a strong pull by the index or middle finger): a triangular initial displacement with rounded corners.
  - **Thumb slap** (a knuckle strike): zero initial displacement plus a rounded velocity pulse at the strike point.
    They compare it to a piano hammer.
- **Terminations.** An ideal reflection at one end and a single-pole low-pass at the other. Dispersive reflection
  filters made no audible difference in their tests. The **limitation amplitude mattered far more**: the string has
  to hit at least the last fret during the first period. The limit must shrink towards the head end, but not to
  zero, or the note dies in a few periods.
- **Cost.** E1 at 44.1 kHz has about 800 unit delays over the fingerboard, which is about 3,200 operations per
  sample with a limiter at every delay. **Placing limiters only at the frets (spacing about 8) cut the cost by
  nearly 8× with no perceived loss.**
- **Pickup.** A (1 − z⁻¹) differentiator turns displacement into velocity, followed by the Gaussian aperture.
- **Result.** Realistic early reflections, periodic "fret noise" bursts in the first periods, and spectrograms
  close to recordings. The missing piece is the knuckle's own recoil, a secondary conditional reflection like a
  piano hammer's.

The general, energy-stable way to do this is finite differences with collision potentials: Bilbao & Torin,
"Numerical modeling and sound synthesis for articulated string/fretboard interactions", JAES 63(5), 2015. Their
paper argues that sample libraries become unwieldy for fully articulated fretted instruments, which is why a model
is attractive. Evangelista & Eckerholm (IEEE TASLP 2010) cover touch and collision in waveguides, and Trautmann &
Rabenstein cover fret–string collision in the functional-transformation (modal) domain (EURASIP 2004). Both are
cited from knowledge.

**Finger, pick and slap excitation**, in practice. Use an input-driven waveguide rather than initial conditions,
so that a pluck can arrive while the string is already ringing.
- **Finger.** A force pulse over a contact width of about 10–20 mm, low-passed by the flesh ("hardness"), then a
  release. Optionally, a small *detach* noise when the finger leaves, which is MODO's "Detach Noise".
- **Pick.** A narrower contact and a sharper corner. The pick-direction low-pass (J&S) differs between down- and
  up-strokes. A scrape noise can be added.
- **Slap thumb.** A velocity pulse, plus the *knuckle as a temporary conditional reflection* for 1–3 ms.
- **Pop.** A large displacement, i.e. a fret collision on release.

**Palm and finger muting** raise the loss near the bridge, cutting sustain from seconds to roughly 0.1–0.3 s
(a working estimate). **Harmonics** are a light touch at a node: a strong, short-lived damping point at L/n.

**Hammer-ons, pull-offs and slides.**
- A **hammer-on** shortens the vibrating length (the delay) and injects a small excitation at the fret.
- A **pull-off** lengthens it and injects a small pluck.
- A **fretted slide** steps the delay length fret by fret, with a click at each fret.
- A **fretless slide** or vibrato changes the delay continuously. The interpolator must be modulation-safe: use
  Lagrange, not allpass.

KVT98 lists all of these as future work. Commercial products do them with samples (Trilian, Ample) or keyswitched
model states (MODO).

**Sympathetic coupling** (KVT98). Naive feedback between strings can be unstable. KVT98's unconditionally stable
scheme runs each string as *two polarisations*:
- the output of the horizontal polarisation feeds the *vertical* polarisation of the other strings, through a
  coupling matrix C whose diagonal term g_c couples the string's own two polarisations;
- there is no feedback path, so C does not affect stability.

Slightly detuning the two polarisations gives natural beating; different loss filters give a two-stage decay
(KVT98 Fig. 10). On a bass this is a subtle P2, but the same structure provides open strings that *ring on*
(MODO's "Let Ring").

**Tension-modulation nonlinearity.** On a hard pluck the string is briefly sharp and the pitch settles as the
note decays (Tolonen, Välimäki & Karjalainen, IEEE TSAP 2000; cited from knowledge). It is audible on slapped low
strings. It is P2.

**Modal synthesis versus waveguide.** MODO is modal **[excerpt]**: a bank of damped resonators, one per partial.

Modal strengths:
- per-mode decay and inharmonicity are exact and free;
- pluck and pickup positions become per-mode gains, sin(nπβ)·sin(nπd/L), with no comb artefacts;
- the cost is manageable: E1 up to 10 kHz is about 240 modes × about 4 multiply-adds, roughly 1,000 per sample,
  fine for 4–6 strings.

Modal weaknesses:
- collisions happen in *space* and have to be mapped between the modal and spatial domains (Trautmann &
  Rabenstein's multirate approach);
- slides need every mode's coefficients recomputed.

**Recommendation for Cantus:**
- a **bidirectional waveguide** for the string, because slides, collisions, slap and pickup taps are all spatial;
- a **modal resonator bank** only for the *body* of the upright (P2), or a commuted body impulse response convolved
  into the excitation, which KVT98 and Smith 1993 describe as efficient and "non-physical but good".

### 4.2 The 808

**The original TR-808 bass drum** (Werner, Abel & Smith, "A physically-informed, circuit-bendable, digital model
of the Roland TR-808 bass drum circuit", DAFx-14; paper read in full):
- **Trigger.** A 1 ms trigger pulse (4–14 V, set by the accent level) goes through a nonlinear low-shelf *pulse
  shaper* into a **bridged-T band-pass network** whose ringing *is* the drum. It is a decaying pseudo-sinusoid.
- **Tuning.** The stock centre frequency is **≈ 49.5 Hz**; Roland's service notes give 56 Hz as "typical and
  variable".
- **Attack.** For about **6 ms** after the trigger, an envelope transistor raises the network's centre frequency
  and Q **by more than an octave**. That is too short to hear as pitch, but it makes the attack "punchier and
  crisper". A retriggering pulse then refills the resonance.
- **Pitch sigh.** A separate, slower drift comes from leakage through R161, a diode-like nonlinearity on the common
  node. The measured instantaneous frequency falls from about **56–58 Hz to about 49–50 Hz over about 300 ms**,
  roughly 2–2.5 semitones (my reading of their Fig. 11).
- **Decay.** VR6 controls how much a feedback buffer attenuates the signal, i.e. the Q of the loop.
- **No "machine-gun effect."** The filter states persist, so a retrigger sums with what is still ringing, and each
  hit differs slightly. The authors put unit-to-unit variation down to ±20% capacitors and ±5% resistors.

**What the trap "808" means today.** It is not the drum but a *sustained, pitched sub*. Pulsus already has the
drum-style kick, and the bass engine should cover the pitched sub. It has these parts:
1. A **sine** (sometimes with a slight triangle shape, or folded) at the note's pitch.
2. A **pitch envelope**: starts +7 to +24 semitones high and falls in roughly 10–80 ms (typical values; a design
   estimate). Pulsus's kick uses 15.5 st over 40 ms, landing at 45 Hz.
3. A **fast "knock" stage**, the TR-808's more-than-an-octave jump for about 6 ms. Pulsus implements it as a
   2.5×click extra pitch that decays in about 8 ms.
4. A **click or transient layer**: noise, a sampled kick transient (SubLab ships a folder of them), or the kick
   itself.
5. An **amplitude envelope** with a long sustain or release for "long 808s".
6. **Glide between overlapping notes**, the trap signature slide. SubLab and 808 Studio both offer a *curve* and a
   *range*, and SubLab offers BPM sync.
7. **Saturation** so the bass is heard on small speakers.

**Keeping the fundamental while adding harmonics.**
- **The problem.** A 45 Hz 808 has its fundamental where small speakers produce almost nothing; phone speakers
  reproduce little below roughly 200–500 Hz (an engineering rule of thumb, not measured here). Harmonics 2–5 sit at
  90–225 Hz, still low, so an 808 needs energy up to about the 10th harmonic (≈450 Hz) to be *heard* on a phone.
  The ear infers the missing fundamental from a series of harmonics spaced f0 apart. This is the principle behind
  psychoacoustic bass enhancers (Larsen & Aarts, "Reproducing low-pitched signals through small loudspeakers", JAES
  50(3), 2002; Waves MaxxBass). Both are cited from knowledge. The Waves page I read only says "psychoacoustic".
- **The trap.** Plain `tanh` drive on a sine *reduces the fundamental relative to the peak*. A symmetric curve
  gives odd harmonics only, which is hollow; an asymmetric one gives even ones, which is warmer. On a mixed signal it
  also intermodulates.
- **Solution A: Chebyshev harmonics** (exact and alias-free). For a *unit-amplitude* sine x = cos θ, the Chebyshev
  polynomial T_n(x) = cos nθ gives exactly the nth harmonic. So Σ a_n T_n(x), applied **before** the amplitude
  envelope, adds a chosen set of harmonics at fixed levels relative to the fundamental. The fundamental is kept by
  a₁ = 1. Nothing aliases while n·f0 < fs/2. This is the most controllable "harmonics" knob for a sine 808.
- **Solution B: fundamental-locked drive** (for any source).
  - Split the signal with a **pitch-tracked** crossover at about 1.5·f0 (the instrument knows f0 exactly; no
    detector is needed).
  - Saturate a copy, high-pass it above the crossover and add it back.
  - The fundamental comes only from the clean path, so its level is **unchanged by drive**. That is directly
    testable (§9).
  - SubLab's "lo/hi-cut around the distortion" (bs-14) is the manual version of this.
- **Key-tracked drive.** Hearing is least sensitive, and small speakers least capable, at the lowest notes (equal-
  loudness contours, ISO 226). So *more* harmonic drive on low notes and less on high ones keeps a bassline even
  across a phone and a club system: drive_dB = base + slope·(ref_note − note)/12.
- **Oversampling.** Harmonics of a 40–60 Hz sine rarely reach Nyquist, but the click and any reese layer do. Reuse
  Cantus's existing oversamplers (`comprimo_oversample.rs`, `terminus_os.rs`) for the drive and clipper stage at
  2–4×.

**Phase at note-on.**
- **Mono retrigger.** Starting the sine at 0° (rising) every time makes every hit identical, which is punchy and
  consistent against the kick. A hard retrigger mid-cycle is a click, so crossfade from the old voice over about
  1–2 ms. Pulsus's choke does the same over "a few milliseconds".
- **Legato.** Carry the phase through, with no reset, so a glide is continuous.
- **Integer phase accumulators** (u32/u64 fixed point) guarantee exact periodicity and bit-identical renders
  across platforms. A float phase is fine for short notes, but an 808 can sustain for bars.

**Glide.**
- **Shapes:**
  - *exponential*, an RC lag on a 1 V/oct control voltage, which is what the 303 and most analog monos do;
  - *linear in pitch*;
  - *S-curve*.
- **Modes:**
  - *constant time*: every glide takes T, which is best for 808 slides drawn in a piano roll;
  - *constant rate*: T is proportional to the interval, like analog portamento.
- **Triggers:** legato only (overlapping notes) or always.
- **Time.** The TB-303's slide is commonly quoted at about 60 ms. That is **unverified**: the Devil Fish page I
  read gave no time constant. SubLab offers glides from 0 up to a full bar with BPM sync.
- **Range.** 808 Studio adds *Slide Octaves*: a slide note sweeps a fixed range, independent of the next note.

### 4.3 Mono voice, note priority and legato

- **The note stack.** Keep every held note in order of arrival, up to 16. When the sounding note is released, return
  to the *previously held* note: glide back without retriggering if legato.
- **Priority.**
  - **last** (the modern default);
  - **low** (the Minimoog convention; good for bass played under chords);
  - **high**.
- **Legato versus retrigger.** In legato mode an overlapping note changes pitch without restarting the envelopes.
  The 303 slide keeps the gate open. In retrigger mode the envelopes restart but the pitch still glides.
- **Cantus-specific timing.** The snapshot sorts events at the same tick with **note-offs first**
  (`compile_events`). Notes that exactly *touch* are therefore released before the next one starts, i.e. not legato.
  A **legato window** (0–30 ms) that treats a gap that short as overlap is what piano-roll users expect. It must be
  measured in samples from the event times, never from block boundaries.
- **Poly for strings.** A modelled bass is not really mono: it is **one voice per string**. A new note on a string
  cuts that string's previous note. Notes on different strings ring together. This is what Ample's Riffer means by
  "no same-string polyphony" (bs-19).
- **Fingering.** Assign each incoming note a string and fret:
  - lowest position;
  - stay in a 4-fret position window;
  - prefer open strings;
  - or *as played* (MPE channel = string, as on MIDI guitar controllers).

### 4.4 Synth bass: reese, low-cutoff filters and the sub

**Supersaw measurements** (Szabo, "How to emulate the Super Saw", KTH thesis, 2010; read in full):
- **Detune.** Seven saws. At maximum detune the side oscillators are at about **1 ± 0.110 / 0.063 / 0.020**
  (for example 465.8, 490.4, 513.1 | 523.4 | 533.8, 555.9, 579.6 Hz at C5), i.e. roughly ±1.9 semitones at the
  outside. The detune knob follows a strongly nonlinear curve (an 11th-order polynomial fit): gentle for most of its
  travel, then steep above about 0.9.
- **Mix.** The centre oscillator falls *linearly* (about 1 → 0.445) while the six side oscillators rise along a
  parabola (about 0.04 → 0.6). They are equal at about 75% of the knob. This keeps the level from exploding.
- **Phase.** Randomised **at each note trigger** (free-running), and held for the note.
- **High-pass.** A pitch-tracked high-pass at the fundamental removes aliasing that folded below f0.

For bass, the random phase is the enemy: it makes every note's low end different. Cantus should offer **Reset**
(the default for bass), **Random** (seeded per note, as in §4.6) and **Free**.

**Why reese needs a separate sub** (my calculation). Two saws detuned by 20 cents at E1 beat at
41.2·(2^(20/1200) − 1) ≈ **0.48 Hz** at the fundamental and ≈ 4.8 Hz at the 10th harmonic.
- The upper harmonics give the characteristic movement.
- The **fundamentals drift in and out of phase every 2 s**, so the sub level swings between full and cancelled.

The fix is the recipe in §3.7: high-pass the detuned layer at about 100–150 Hz and put a clean, phase-locked sine
sub under it.

**Sub-oscillator phase alignment.** Derive the sub *from the main oscillator's phase*: a flip-flop that toggles on
each main-cycle wrap gives an exact octave down, and two stages give two octaves.
- The zero crossings then coincide; there is no drift and no beating.
- The combined waveform, and so the peak level, is the same on every note and during glides.
- A sub run as a separate oscillator at f/2 drifts whenever the pitch moves, because of rounding.

SubLab's X-Sub is always dry, bypassing the filter, envelopes and distortion, which is the same principle:
**the sub path stays clean**.

**Filters at low cutoff.**
- **Direct-form biquads** at, say, 30 Hz and 96 kHz have poles within about 0.002 of z = 1. In f32 their
  coefficients quantise badly (noise and a detuned cutoff). The **TPT/ZDF state-variable filter** (Zavalishin,
  *The Art of VA Filter Design*; the topology `cantus_dsp::filter::Svf` already uses) is well-conditioned there.
  For a mono bass the cost of **f64 filter state** is trivial, so use it.
- **Ladder resonance thins the bass.** In a 4-pole ladder with feedback k, the passband gain is 1/(1 + k). At
  self-oscillation (k = 4) the lows drop by about 14 dB. This is textbook (Stilson & Smith 1996; Huovilainen,
  DAFx 2004; cited from knowledge), and it is why "resonance kills the bass". Offer **bass compensation** (0–100%),
  which feeds back only the high-passed part, or adds (1 + k) gain below the cutoff.
- **The TB-303 filter** is a diode ladder with unequal poles, and its resonance and accent interact (Whittle). The
  accent sweep capacitor accumulates across successive accents (§3.5). Transistor Bass's slide and accent timing
  was criticised against ABL3 on KVR, which shows how much the *behaviour*, not the tone, defines a 303.

**Stereo and mono compatibility.** Keep everything below about 100–150 Hz **mono** ("mono below", width applied
only above a crossover), so bass does not cancel on mono systems and cuts cleanly on vinyl. 808 Studio's chorus
low-passes its *input* for the same reason.

### 4.5 The kick–bass relationship

- **Frequencies.** Kick bodies sit at about 40–80 Hz (Pulsus: 45 Hz × 2^(tune/12), falling from about 110 Hz).
  808s play at 30–60 Hz. They collide.
- **Ducking.** A volume shaper or sidechain compressor lowers the bass by 3–10 dB for the kick's body,
  typically about 50–150 ms (typical practice, not a measurement). The standard tools read the kick's *audio* and
  react after it. Some need lookahead, which means latency.
- **Phase.** Two low sines at nearby frequencies sum constructively or destructively depending on phase at the
  kick's onset.
  - An **808 that resets to 0° on the same tick as a kick that also starts at 0°** is constructive at the attack.
    It then drifts: at 50 Hz against 55 Hz, full cancellation comes after 1/(2·5 Hz) = 100 ms. By then the kick has
    mostly decayed.
  - **Sound Radix Pi** analyses the correlation of several tracks and applies a *dynamic, per-band phase rotation*
    to maximise it, through its own background process across plug-in instances. That shows the demand, and also
    how much machinery a plug-in needs to do what a host can do directly.
- **Tuning.** A kick that is a semitone away from the 808's root beats at about 2.7 Hz at 45 Hz, which reads as mud.
  Producers tune the kick to the key or to the 808. Pulsus's kick pitch is known *exactly* from its parameters.
- **Spectral ducking.** Trackspacer-style tools cut only the kick's band in the bass, only while the kick sounds.
  It was not researched in depth this session.

### 4.6 Articulations, round robin and determinism

- **Samplers need round robin** to avoid the machine-gun effect: Trilian goes up to 16×. A model gets variation
  for free, but **it must stay deterministic** under Cantus's null-test rules.
- **Recipe.** Derive the per-note variation from a hash of *(track id, note id)*:
  - pluck position ±2–3%;
  - excitation noise seed;
  - velocity-to-brightness ±1 dB;
  - finger choice, index or middle, alternating (MODO's "Stroke AUTO").
- Pulsus's noise-seeding comment spells out why: a **global counter** made one pad's sound depend on other pads'
  hits. Seeding per note keeps renders reproducible *and* isolated.
- **MPE** (MMA, ratified 28 January 2018):
  - zones, with a manager channel for common messages and member channels, one per note;
  - per-note pitch bend, pressure and CC74 (timbre);
  - by the specification's convention, member channels default to a ±48-semitone bend range and the manager
    channel to ±2. That is stated from knowledge; the MMA page I read confirmed the structure but not the numbers.

  CLAP note expressions carry the same per-note data natively (see `standards.md`). For a bass, bend gives slides
  and vibrato, pressure gives mute or "growl", and timbre gives pluck position or tone.

---

## 5. The interface: praise and criticism

**MODO BASS 2** (bs-01 to bs-06):
- **Frame.** A dark maroon-brown gradient with warm orange accents: the active tab, knob value arcs, and the
  selected button's outline. White and light-grey labels in tracked caps.
- **Values at rest.** Every knob prints its value *above* itself in orange, so values are visible without hovering.
  This matches family.md principle 3.
- **Hierarchy.** The instrument picture takes about 55–60% of the window. It is beautiful but mostly decorative:
  - it edits exactly one thing on Play Style (the orange position marker with an inch readout);
  - it shows the pickup geometry (dimension lines) on Electronics;
  - it is inert elsewhere;
  - the upright's mic *picture* is still edited through X/Y/Distance *knobs*.
- **Low-contrast states.** Inactive boxed buttons are a dark fill with a thin grey outline, about 1.5:1 against the
  background (estimated by eye from the JPEGs, not measured). Disabled knobs (the EQ when passive) are simply
  dimmed.
- **Where each note is played.** A fretboard strip with the frets drawn and dots on the played positions; a keyboard
  with the keyswitch zones in orange and out-of-range keys greyed.
- **The Control page is a spreadsheet** (bs-05), the least friendly part of the product.

**Trilian** (bs-07, bs-09):
- Skeuomorphic red lacquer with a gold serif logo and chrome and orange LED-ring knobs. It reads as 2010.
- The *Controls* page per patch is a good idea (8–10 curated knobs). The depth sits behind tabs: Main, A, B, FX, Arp.
- The arpeggiator (bs-09) has clear per-step modifier chips, a dense but legible design.
- SOS's criticism that the browser covers the controls still applies to its layout.

**SubLab XL** (bs-13 to bs-16):
- The most modern and legible of the group: flat surfaces, **large numeric readouts with small units** ("0.4 Hz",
  "182 ms", "0 oct"), and a distinct hue per section (green for sound, pink for modulation, yellow for FX and
  macros).
- Popovers for rarely used options (the Super Oscillator).
- A two-handle range slider sets the distortion's lo/hi-cut band.
- Two-macro performance page (bs-13): huge targets with near-zero text, which suits the "tweak a preset" user.
- The cost is that a lot of state is on other pages.

**808 Studio 2** (bs-10):
- About 20 small panels on one charcoal screen with a yellow accent, and labels of about 8–9 px: everything is
  visible and little of it is legible.
- *Slide Time / Slide Octaves / Slide Curve* and the *Sidechain* trio are the right *concepts*.

**Transistor Bass** (bs-11, bs-12):
- Light brushed metal; big knobs with weak pointer contrast.
- The **sequencer grid is the model to copy**: pitch, gate, slide and accent as aligned rows of toggles, with
  keyboard shortcuts G, S and A.

**Ample Bass** (bs-18 to bs-20):
- A photoreal bass across the top with a live **"String 3, Fret 6, D#2"** readout.
- **Riffer's string curves** join the notes on each string across the piano roll and number the strings 1–4 on the
  keys. It is the clearest visualisation of string assignment in any product.
- Faders show their values at rest.
- The price of all this is a second DAW inside the plug-in: its own transport, tools, CC lanes and quantise.

**What to take and what to avoid.**
- **Take:**
  - values printed at rest;
  - a position marker with a real unit (mm or in) on an instrument drawing;
  - pickup dimension lines;
  - fretboard and position readouts;
  - SubLab's big numerals, section hues and band range slider;
  - Transistor Bass's slide and accent rows;
  - Riffer's string curves, *in the host piano roll*.
- **Avoid:**
  - photoreal pictures that take the space without being editable;
  - keyswitch spreadsheets;
  - 8 px labels;
  - dark-only low-contrast buttons;
  - a second piano roll inside the instrument.

---

## 6. What reviewers and users say

*Caveat: reviews are from the press. Forum opinion is thin here: Gearspace and Sweetwater returned 403 and were
not retried. One KVR thread was read. Forum statements are individual opinions and are labelled as such.*

- **MODO BASS 2.**
  - *Sound on Sound* (Paul White, September 2022): the models sound so like the real basses that you forget they
    are digital. There were no substantial criticisms beyond fretless slides, bends and vibrato needing practice.
  - *Computer Music/MusicRadar* (September 2022): praises the 22 models, the depth and the legato options for fast
    lines. **Con: occasional stuttering and note lag during quick playing.**
  - Output's 2026 round-up says it "responds to playing dynamics like a real instrument" **[excerpt]**.
- **Trilian.**
  - *SOS* (Dave Stewart, January 2010): "an investment that won't let you down". Criticisms: the browser hides
    controls, there is no sample import, and it was plug-in only at the time.
  - Current round-ups still call it "the industry benchmark" for realistic bass **[excerpt]**.
  - The age of the product shows in the UI and in the 34 GB footprint. That is an inference, not a quoted review.
- **SubLab / XL.**
  - Rekkerd (2019): the visual feedback is "great" (a spectrum display and a note-to-frequency map) and the price is
    a steal; X-Sub is lows up to about 65 Hz plus harmonics.
  - Round-ups position XL as the "mix-ready 808s in seconds" workhorse **[excerpt]**.
- **Transistor Bass.**
  - A March 2026 blog review: authentic acid character and fast pattern workflow, but a narrow palette and "not the
    most detailed emulation".
  - **KVR forum** (user opinion): one user demonstrated that **slides prolong notes so that the following accents are
    missed**, where ABL3 behaves correctly. Image-Line replied that the design deliberately extends ranges beyond the
    hardware. Some users value accuracy (Reefius), others immediacy (Trancit).
  - The lesson: for a 303, the **timing semantics of slide and accent** are the product.
- **EZbass** (SOS 2020): the drum-MIDI drop zone and Tap2Find are loved. There is no true fretless, and the groove
  library was thin at launch.
- **Ample Bass** is often called the most realistic sampled bass in round-ups **[excerpt]**. Its v4 roadmap has
  gone into *editing* tools (chords, vibrato curves, swing), which signals that users want to author bass parts,
  not just play them.

---

## 7. Gaps none of them fills

1. **No product knows the kick.** Ducking needs a separate plug-in or an in-instrument compressor fed by a sidechain
   route. Phase interaction needs Pi. Tuning the kick to the 808 is done by ear in another plug-in. Everything
   reacts to *audio*, after the fact, or adds lookahead latency.
2. **No product knows the song's key.** SubLab has tuning and scale settings, and EZbass follows chords in its *own*
   song track. Nobody reads the host project's key or chords, because plug-in APIs do not expose them.
3. **Glide depends on each plug-in.** FL's manual says its own portamento and overlapping-note slides do not reach
   VST/AU instruments. Every 808 plug-in reimplements "overlap = glide". None offers a **slide note** as a first-
   class, visible note property with the curve drawn in the editor.
4. **Articulations are keyswitches.** MODO's 28-row Control table and Trilian's keyswitch zones are the norm.
   Scarbee's scripted legato inference is clever but invisible. Ample builds a second piano roll to show strings.
   A host can make string, technique and slide into *note properties* shown in its own piano roll.
5. **Physical models hide their physics.** Pickup position and pluck position are knobs or markers, but no product
   shows the *consequence*: the comb notches on the harmonic series of the current note. MODO's upright mic scene
   is a picture edited by knobs.
6. **No translation feedback.** X-Sub is "psychoacoustic" but opaque. Nobody shows how much of the bass survives
   on a phone or laptop, or offers a monitor-only small-speaker preview.
7. **No product spans model, 808 and synth with shared performance semantics.** Trilian samples; MODO models;
   SubLab and 808 Studio synthesise; Transistor Bass sequences.
8. **Determinism is never promised.** Random round robin, free-running phases (Szabo shows the supersaw
   randomises phase per note) and background analysis (Pi) mean offline renders may differ. None states otherwise.
9. **Legibility.** Everything except SubLab is dark and low-contrast. 808 Studio's labels are tiny. None is
   themeable or localised to Spanish, as far as the pages show.
10. **MPE.** None of the pages I read advertises MPE, even though bass slides and vibrato are the natural per-note
    gestures.

---

## 8. How Cantus beats them

Cantus has:
- Firmus (PolyBLEP oscillators, a TPT SVF, ADSR, 16 voices);
- Pulsus (a synthesised kick with pitch envelope, knock, click and peak-normalised `saturate`; choke over a few ms;
  per-pad noise seeding);
- oversamplers;
- a sidechain route model;
- the snapshot of sorted note events per track;
- the command bus with undo fuzz;
- EN/ES localisation;
- the family frame rules (family.md §7).

Nothing yet sits between a note and a sound in a mono, gliding way.

### P0: parity foundation (without these we are not credible)

1. **A mono voice engine done right**, shared by every engine:
   - a note stack of 16 with last/low/high priority, and return to held notes;
   - legato or retrigger, with de-click crossfades of about 1–2 ms;
   - glide in constant-time or constant-rate modes, with exponential, linear or S shapes, BPM sync, and "legato only"
     or "always";
   - a **legato window** for touching notes (§4.3);
   - pitch-bend range.

   All of it sample-accurate from event times, deterministic, and independent of block size.
2. **An 808 engine at SubLab and 808 Studio parity:**
   - an integer-phase sine with phase reset;
   - a pitch envelope plus a Pulsus-style **knock**, a click or transient layer, and an AHDSR;
   - **fundamental-locked drive** (a pitch-tracked split; §4.2) with Tube, Tape, Fold and Hard types and **key
     tracking**;
   - a **Chebyshev harmonics** control;
   - a subsonic high-pass;
   - an oversampled output clipper;
   - drive = 0 is **bit-exact** (the Pulsus `saturate` rule).
3. **A Synth engine at Transistor Bass parity, plus reese:**
   - PolyBLEP saw/square/pulse with 1–7 unison voices (linear or "JP" curve per Szabo) and Reset/Random/Free phase;
   - a **phase-locked sub** that bypasses the filter and drive;
   - TPT ladder, diode (303) and SVF filters with f64 state and **bass compensation**;
   - 303 accent with the *accumulating* sweep;
   - slides whose timing never swallows the next accent (the KVR complaint).
4. **A String engine at MODO parity for electric bass:**
   - a bidirectional waveguide per string, with a Lagrange-3 fractional delay, a one-pole loss filter and a
     dispersion allpass;
   - finger, pick, thumb and pop excitations;
   - **fret collisions at the fret positions** (Rank & Kubin), which slap needs;
   - two magnetic pickups with position taps, Gaussian aperture, nonlinearity, resonance, passive tone or active EQ,
     plus a piezo;
   - muting, harmonics, hammer-on/pull-off and slide by changing the delay length;
   - string and fret assignment;
   - detach, slide and fret noise;
   - fretless.
5. **Quality gates** (the Cantus habits):
   - null tests first;
   - pitch within 0.5 cent across the range;
   - block-size independence and determinism;
   - no allocation on the audio thread;
   - an ADR for the instrument;
   - a **schema bump plus a fixture** for `InstrumentSpec::Gravis`;
   - undo fuzz covering its commands;
   - EN and ES strings.
6. **The family frame** (family.md §7): header and footer, values at rest, AA contrast in the Paper, Parchment and
   Ink & Gold themes, a monospaced readout for every value, and every parameter automatable and undoable as a host
   command.

### P1: host-native, where only Cantus can do it

1. **The bass that knows where the kick is.**
   - A **Kick duck** whose default source is the project's kick. It finds the Pulsus pad whose voice is `Kick`, or a
     track the user picks.
   - It works from the kick's **note events in the snapshot**, not its audio. Because the host knows when the next
     kick will arrive, the duck can **start before it** ("pre", 0–10 ms) with **zero added latency**.
   - An audio-sidechain fallback uses the existing sidechain route.
   - The display draws the kick's hits and the duck curve as a lane.
2. **Kick and bass tuned as one.** The device shows the kick's fundamental, computed exactly from Pulsus's `tune`
   (45 Hz × 2^(tune/12)), on the same axis as the bass notes. **"Tune kick to root"** or **"tune 808 to kick"**
   changes the Pulsus kick's `tune` and/or the Gravis transpose as **one undo step** across two devices.
3. **Key awareness.** A project key, set by the user or estimated from the project's notes with a pitch-class key
   finder (Krumhansl–Schmuckler style). The piano roll flags out-of-key bass notes, and the key names the root the
   808 presets are voiced against. No plug-in can read this.
4. **Slide notes as a note property**, the FL idea that FL itself cannot give to VSTs:
   - the piano roll draws the glide curve between the notes;
   - the instrument receives an explicit "slide from previous" flag instead of guessing from overlap;
   - overlap-glide still works for imported MIDI;
   - in the rack, slide and accent are step rows, as in Transistor Bass.

   Model change: `Note { slide: bool, accent: bool }` or an articulation enum, with a schema bump.
5. **String and articulation lanes in the host piano roll.**
   - `InstrumentSpec::lanes()` already lets an instrument shape the piano roll. Gravis/String adds **string colouring
     and connecting curves** (Riffer's idea, bs-19) and a per-note **technique** property (finger/pick/thumb/pop/
     mute/harmonic) instead of keyswitches.
   - Keyswitches remain *input* bindings for live play and imported MIDI, converted into note properties when
     recorded.
6. **Show the physics.** The harmonic strip draws the current note's partials, with the comb notches from the pluck
   and pickup positions *moving as you drag*, and the inharmonic stretch as it happens.
7. **Translation preview.**
   - A monitor-only **Phone, Laptop or Club** listen mode (a band-limited speaker model at the device's output, never
     rendered, like the "Delta" listen of the family);
   - a **translation meter**: the proportion of bass energy that survives above 150 Hz, and the harmonic-to-
     fundamental ratio.
8. **The AI bridge** (`cantus-ai`), using the same commands a user would:
   - "make the 808 hit on phones" raises harmonics and key-tracked drive;
   - "tune the kick to the key";
   - "write a root-note bassline on the kick hits following these chords". This is a deterministic generator, not
     a sample library: roots on the kick onsets, approach notes, octave pops. It should rival EZbass's drum-drop
     workflow without a library.
9. **MPE and CLAP note expressions:** per-note bend for slides and vibrato, pressure for mute or growl, timbre for
   pluck position or tone. On MPE input, the member channel can be the string.

### P2: later

- **Upright acoustic bass:** a commuted body impulse response or a modal body; bow excitation later.
- **Physical refinements:** sympathetic coupling (KVT98's two-polarisation matrix), polarisation beating, and
  tension-modulation pitch glide on hard plucks.
- **A sampled 808/transient layer** with pitch tracking and loops, reusing Pulsus's sample pads (the SubLab and 808
  Studio sampler).
- **Effects:** amp and cab, octaver and envelope filter. These are family effects, used as inserts or a chain
  preset.
- **Authoring tools:** a pattern and groove library, audio-to-MIDI bass tracking (EZbass's Audio Tracker), and
  Guitar Pro tab import (Ample).
- **Export** as CLAP/VST3 (see `standards.md`).

### 8.1 The proposal: *Gravis*

**Name.** **Gravis**, Latin for *heavy, deep, low-pitched*. It is the root of "grave", and medieval theory used it
for the low notes. The alternative is **Fundus**, "bottom, foundation". Neither name appears in the repository.
(*Porta* is already taken by the gate, so "portamento" must stay a parameter name, not a device name.)

**One instrument, three engines.** The reasons:
- "Bass" is a *role* on a track. The kick awareness, the mono voice, the sub path, the mono-below crossover and the
  output stage belong to that role, whatever makes the tone.
- Presets can move between engines; switching engines keeps the voice and output settings.
- All three engines' parameters are **stored** in the spec, so switching is lossless and undoable, like A/B.
- The DSP stays modular in `cantus-dsp`, following the `aequalis_*` / `comprimo_*` pattern:
  - `gravis.rs`: the instrument and voice manager;
  - `gravis_voice.rs`: the note stack, glide and legato window;
  - `gravis_string.rs`: waveguide, excitation and collisions;
  - `gravis_pickup.rs`;
  - `gravis_808.rs`;
  - `gravis_synth.rs`, reusing `osc` and `filter`;
  - `gravis_out.rs`: sub, drive, duck, mono-below, clipper;
  - `gravis_tests.rs`.
- The core type is `InstrumentSpec::Gravis(GravisParams { engine, voice, string, e808, synth, sub, out })`, with
  `InstrumentParam::Gravis(GravisParam)`.

**Voice architecture:**

```
 note events (snapshot, sorted; slide/accent flags; MPE per-note bend/pressure/timbre)
        │
        ▼
 ┌──────────────── GravisVoice (mono / legato / strings) ─────────────────┐
 │ note stack(16) · priority · legato window · glide (time|rate, shape,    │
 │ sync) · pitch-bend · per-note seed = hash(track, note id)               │
 │ String mode: fingering → (string, fret) per note; one voice per string  │
 └──────┬───────────────────────────────┬──────────────────────────────────┘
        │ pitch(t), gate, velocity, accent, technique, seed
        ▼                               ▼
 ┌─ Engine (one of) ───────────────┐   ┌─ Sub path (optional, all engines) ─┐
 │ String: waveguide×strings →     │   │ phase-locked to engine f0          │
 │   pickups(pos, aperture, NL,    │   │ (flip-flop divider; ‑1/‑2 oct)     │
 │   resonance) → tone/active EQ   │   │ sine/tri/square, own level & LPF   │
 │ 808: int-phase sine → pitch env │   │ never filtered or driven           │
 │   + knock → Chebyshev harmonics │   └────────────────┬───────────────────┘
 │   → amp AHDSR (+ click layer)   │                    │
 │ Synth: PolyBLEP ×unison (+303)  │                    │
 │   → ladder/diode/SVF (f64,      │                    │
 │   bass comp) → amp ADSR         │                    │
 └──────────────┬──────────────────┘                    │
                ▼                                       │
 ┌─ Drive (fundamental-locked, key-tracked, 2–4× OS) ─┐ │
 │ split @ 1.5·f0 (pitch-tracked) → sat(high) + clean │ │
 └──────────────┬─────────────────────────────────────┘ │
                ▼                                        ▼
 ┌─ Output ───────────────────────────────────────────────────────────┐
 │ sum → subsonic HPF → kick duck (events look-ahead | audio key)     │
 │ → mono-below crossover + width → clipper (OS) → gain → track strip │
 └────────────────────────────────────────────────────────────────────┘
```

**Parameters.** Units are shown in the UI; every parameter is automatable. The defaults aim at "sounds good
first", but a control at its neutral value must not change a sample.

*Engine and voice (shared):*

| Parameter | Range | Default | Unit / notes |
|---|---|---|---|
| `engine` | String · 808 · Synth | 808 | stepped |
| `voice_mode` | Mono · Legato · Poly (Synth/808 up to 8) · Strings (String engine) | Legato (808/Synth), Strings (String) | stepped |
| `priority` | Last · Low · High | Last | stepped |
| `glide_mode` | Off · Legato only · Always | Legato only | stepped |
| `glide_time` | 0–2000 | 80 | ms, log; or synced |
| `glide_sync` | off · 1/64…1 bar (incl. dotted and triplet) | off | tempo |
| `glide_rate_mode` | Constant time · Constant rate | Constant time | stepped |
| `glide_shape` | Exponential · Linear · S-curve | Exponential | stepped |
| `slide_range` | 0–24 (for slide notes with no target) | 12 | st (808 Studio "Slide Octaves") |
| `legato_window` | 0–30 | 5 | ms |
| `retrigger_env_on_legato` | off/on | off | — |
| `phase_reset` | off/on | on | 808/Synth |
| `declick` | 0–5 | 1.5 | ms crossfade on retrigger/steal |
| `bend_range` | 0–24 | 2 | st; MPE member ±48 |
| `octave` / `transpose` / `fine` | −2…+2 / −12…+12 / ±50 | 0 / 0 / 0 | oct / st / cents |
| `a4_reference` | 415–466 | 440 | Hz (as in MODO) |
| `humanize` | 0–100 | 15 | % (seeded per note; 0 = identical notes) |

*String engine:*

| Parameter | Range | Default | Unit / notes |
|---|---|---|---|
| `strings` | 4 · 5 · 6 | 4 | — |
| `tuning` | Standard · Drop D · BEADG · Custom (MIDI note per string) | Standard (E1 A1 D2 G2 = 28 33 38 43) | — |
| `scale_length` | 762–914 | 864 | mm (30–36 in; unit toggle) |
| `frets` / `fretless` | 20–24 / off·on | 21 / off | — |
| `string_type` | Roundwound · Halfwound · Flatwound · Tape | Roundwound | sets loss and dispersion presets |
| `gauge` | Light · Medium · Heavy | Medium | scales tension and B |
| `stiffness` | 0–200 | 100 | % of the computed B |
| `sustain` | 0.3–20 | 6 | s (T60 of the fundamental) |
| `brightness_decay` | 0–100 | 45 | % (high-frequency T60 relative to the fundamental) |
| `age` | 0–100 | 20 | % (new → dead: faster high-frequency loss) |
| `action` | 1.0–4.0 | 2.2 | mm at the 12th fret (collision height; a typical setup, to be tuned by ear) |
| `buzz` | 0–100 | 30 | % (collision stiffness and extra noise) |
| `technique` | Finger · Pick · Slap (thumb/pop by threshold) · Mute · Harmonic | Finger | per note if set in the piano roll |
| `pluck_pos` | 20–400 | 180 | mm from the bridge (drag on the drawing) |
| `hardness` | 0–100 | 50 | % (finger flesh or pick thickness: excitation low-pass) |
| `pick_direction` | Down · Up · Alternate | Alternate | — |
| `pop_threshold` | 1–127 | 110 | velocity (MODO shows 120) |
| `mute` | 0–100 | 0 | % (bridge-side damping) |
| `finger_noise` / `slide_noise` / `fret_noise` | 0–100 | 30 / 40 / 30 | % |
| `let_ring` | off/on | off | open strings ring on |
| `fingering` | Lowest · Position (4-fret window) · Open strings · As played (MPE ch = string) | Position | — |
| `pickup1_type` / `pickup2_type` | Off · P split · J single · MM humbucker · Soapbar · … | J single / J single | aperture, resonance, nonlinearity presets |
| `pickup1_pos` / `pickup2_pos` | 20–300 | 60 / 159 | mm from the bridge (drag handles) |
| `pickup1_level` / `pickup2_level` | 0–100 | 100 / 100 | % |
| `pickup_height` | 1.5–5.0 | 3.0 | mm (nonlinearity and output) |
| `piezo` | 0–100 | 0 | % |
| `circuit` | Passive · Active | Passive | — |
| `tone` | 0–100 | 100 | % (passive RC low-pass; 100 = open) |
| `eq_bass` / `eq_mid` / `eq_treble` | ±12 | 0 | dB (active only) |
| `eq_mid_freq` | 250–2500 | 800 | Hz |
| `sympathetic` | 0–100 | 0 | % (P2) |

*808 engine:*

| Parameter | Range | Default | Unit / notes |
|---|---|---|---|
| `shape` | 0–100 (sine → soft triangle) | 0 | % |
| `pitch_env_amount` | 0–48 | 12 | st above the note |
| `pitch_env_time` | 1–500 | 45 | ms to −60 dB of the drop (the Pulsus convention) |
| `knock` | 0–100 | 20 | % (more than an octave for about 5–8 ms, as on the TR-808) |
| `click` / `click_tone` | 0–100 / 500–16000 | 15 / 3000 | % / Hz |
| `transient` | None · Pulsus kick · sample (P2) | None | layer |
| `attack` / `hold` / `decay` / `sustain` / `release` | 0–50 / 0–2000 / 10–10000 / 0–100 / 5–4000 | 1 / 0 / 1200 / 60 / 250 | ms / ms / ms / % / ms |
| `sigh` | 0–100 | 0 | % (TR-808-style slow fall of up to about 2.5 st over about 300 ms; off by default because it detunes) |
| `harmonics` | 0–100 | 20 | % (Chebyshev T2–T8 on the unit sine) |
| `harmonic_tilt` | −6…+6 | −3 | dB/harmonic-octave |
| `even_odd` | −100…+100 | +30 | % (even = warm, odd = hollow) |

*Synth engine:*

| Parameter | Range | Default | Unit / notes |
|---|---|---|---|
| `osc1_wave` / `osc2_wave` | Saw · Square · Pulse · Triangle · Sine | Saw / Saw | — |
| `osc2_semi` | −24…+24 | 0 | st |
| `detune` | 0–100 | 18 | cents (reese) |
| `unison` / `unison_curve` | 1–7 / Linear · JP | 2 / Linear | voices |
| `phase_mode` | Reset · Random (seeded) · Free | Reset | — |
| `pulse_width` | 5–95 | 50 | % |
| `filter_type` | Ladder 24 · Diode 303 · SVF LP12 · SVF BP | Ladder 24 | f64 state |
| `cutoff` | 20–20000 | 400 | Hz, log |
| `resonance` | 0–100 | 20 | % |
| `bass_comp` | 0–100 | 50 | % (restores the ladder's 1/(1+k) loss) |
| `filter_env` | −6…+6 | +2 | octaves |
| `key_track` | 0–100 | 50 | % |
| `filter_drive` | 0–100 | 0 | % |
| `accent` | 0–100 | 50 | % (velocity ≥ 100 or an accent note flag) |
| `accent_sweep` | off/on | on | the accumulating 303 behaviour |
| amp and filter ADSR | as Firmus | as Firmus | — |

*Sub and output (shared):*

| Parameter | Range | Default | Unit / notes |
|---|---|---|---|
| `sub_level` | 0–100 | 0 (808), 50 (Synth), 0 (String) | % |
| `sub_octave` / `sub_wave` | −1 · −2 / Sine · Triangle · Square | −1 / Sine | phase-locked |
| `drive` | 0–100 | 25 (808), 0 (others) | % — 0 is bit-exact |
| `drive_type` | Tube · Tape · Fold · Hard | Tube | — |
| `drive_keytrack` | −100…+100 | +50 | % (positive = more drive on low notes) |
| `drive_ref_note` | C0–C4 | C2 | pivot note for key tracking |
| `fundamental_lock` | off/on | on | pitch-tracked split at 1.5·f0 |
| `subsonic` | 10–40 | 20 | Hz high-pass |
| `duck_source` | None · Project kick (auto) · Track… (events) · Sidechain (audio) | None | never on by surprise |
| `duck_depth` / `duck_pre` / `duck_hold` / `duck_release` | 0–40 / 0–10 / 0–200 / 10–500 | 6 / 2 / 20 / 90 | dB / ms / ms / ms |
| `duck_shape` | Linear · Exponential · S | Exponential | — |
| `width` / `mono_below` | 0–200 / 40–300 | 100 / 120 | % / Hz |
| `clip` | off/on (4× OS) | on | output soft clip |
| `gain` | −24…+12 | −6 | dB |

**Editor layout.** It uses the family frame; the body changes with the engine. This sketch shows the String engine
at the "comfortable" size; the 808 and Synth bodies swap the centre panels.

```
┌─ Header ───────────────────────────────────────────────────────────────────────────────────────┐
│ [■ Bass · track colour] GRAVIS  ↶ ↷ │ A|B →Copy │ ‹ Punk Finger ●mod › │ Δ │ ⏻ │ ? │ ⋯ │ ⛶       │
│ Engine: [ String | 808 | Synth ]      Voice: [Strings ▾]  Priority [Last▾]  Glide ◠ 80 ms       │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ┌ Instrument (vector, top view; drag handles, mm readouts) ──────┐ ┌ Harmonics of current note ─┐ │
│ │  bridge║ ▮P1 ▮P2          pluck▼180 mm                          │ │ █ █ ▆ ▂ ▅ █ ▃ ▁ ▄ ▆ …      │ │
│ │ ═══════╫══╪═══╪══════════════╪════════╫═╫═╫═╫═╫═╫═╫═╫═╫═ nut   │ │ ↑ notches: pluck ▽ pickup ▼│ │
│ │ ═══════╫══╪═══╪══════════════╪════════╫═╫═╫═╫●╫═╫═╫═╫═╫═ (D2)  │ │ stretch +0…+42 ¢ (n≤10)    │ │
│ │  |←60 mm→|←159 mm→|                    string 3 · fret 5 · D2  │ │ f0 73.4 Hz  ·  B 5.0e-4    │ │
│ └────────────────────────────────────────────────────────────────┘ └────────────────────────────┘ │
│ ┌ Play ───────────────────────────────────┐ ┌ String ─────────────────┐ ┌ Electronics ─────────┐ │
│ │ [Finger|Pick|Slap|Mute|Harm]            │ │ Type [Round▾] Gauge [M▾]│ │ P1 [J single▾] 100 % │ │
│ │ Hardness 50%  Pop ≥ 110  Mute 0%        │ │ Sustain 6.0 s  Bright 45│ │ P2 [J single▾] 100 % │ │
│ │ Noise: finger 30 · slide 40 · fret 30   │ │ Action 2.2 mm  Buzz 30 %│ │ Height 3.0 mm Piezo 0│ │
│ │ Fingering [Position ▾]  Let ring ☐      │ │ Age 20 %  Stiff 100 %   │ │ [Passive|Active] Tone│ │
│ └─────────────────────────────────────────┘ └─────────────────────────┘ └──────────────────────┘ │
│ ┌ Low end: shared with the kick ─────────────────────────────────────────────────────────────┐  │
│ │ Kick (Drums ▸ Pulsus pad 1) ●   ●   ●   ●      duck ╲_╱‾‾ 6 dB · pre 2 ms · rel 90 ms      │  │
│ │ Bass  ▭▭▭▭▭▭▭╲▭▭▭▭  ▭▭▭▭▭                                                                 │  │
│ │ Axis: 40 Hz ┆ kick 45.0 Hz ◆   bass root E1 41.2 Hz ◇  (−1.5 st) [Tune kick to root]         │  │
│ │ Sub [−1 oct Sine] 0 %  Drive 0 % [Tube] key +50 %  Lock f0 ☑  Mono < 120 Hz  Width 100 %   │  │
│ └────────────────────────────────────────────────────────────────────────────────────────────┘  │
├─ Footer ───────────────────────────────────────────────────────────────────────────────────────┤
│ [Learn] · Voice Strings · Glide 80 ms · Duck Kick 6 dB · Mono<120 Hz · Listen [Full|Phone|Laptop] │
│ · latency 0 ms · In ▮ Out −6.0 dB ▮ Mix 100 %                                                     │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The other two bodies:
- **808 body.**
  - The top-left becomes a **pitch-over-time display**. The glide curve and the pitch envelope are drawn in
    semitones against ms, with handles for amount, time and knock; the amp envelope is drawn beneath.
  - The top-right becomes the **harmonics bar graph**: T1 is locked and labelled "fundamental", and T2–T8 bars can
    be dragged.
  - The drive and click panels take the "Play/String/Electronics" row.
- **Synth body.**
  - The top shows the oscillator waveform with the unison spread, and a **filter curve** with Pro-Q-style
    cutoff/resonance handles (family canvas).
  - The row holds the Osc, Filter and Envelope panels.
  - In 303 mode, the rack view adds step rows for slide and accent (bs-12).

**Interaction and visual rules** (family.md §7):
- The pickups and the pluck marker are **band-handle-style dots**. They follow family gestures: drag, Shift for fine,
  double-click to reset, right-click to type "60 mm" or "2.4 in". Their values are shown at rest.
- The harmonic strip marks notches with **triangles** as well as colour.
- The kick lane uses the kick track's colour.
- Every drag is **one host command**, coalesced into a single undo step. "Tune kick to root" is a single compound
  command.

---

## 9. Test plan hints

The tests follow the repository's habits: null tests first, the `nulltest` harness, sample rates 44.1, 48 and 96
kHz, and render block sizes {1, 7, 64, 480, 4096}.

1. **Waveguide pitch accuracy.**
   - For every MIDI note from B0 (23) to C5 (72) on every string that can reach it, with dispersion off, estimate f0
     over 1 s after a 100 ms settle (autocorrelation with parabolic interpolation, or a zero-crossing regression).
     Require **|error| ≤ 0.5 cent**.
   - With dispersion on, the fundamental stays within 1 cent, and partial n (n ≤ 10) is within 2 cents of
     n·f0·√(1 + B n²)/√(1 + B).
   - Check that a note at C5 fails the test if the fractional delay is disabled. That proves the test is sensitive.
2. **Comb correctness.** Linear mode, pluck at β, pickup at d: the harmonics nearest the predicted notches are at
   least 25 dB below their neighbours. Moving a pickup by X mm moves the notches as predicted.
3. **Stability and energy.** Fuzz every String parameter, collisions included. The output stays finite, the loop
   gain is ≤ 1 (check \|H_loop(e^jω)\| on a grid), and energy never grows after the excitation ends. Decay reaches
   −60 dB within about sustain × 1.2.
4. **Glide timing.**
   - E1 → E2, constant time, 100 ms. The instantaneous pitch (from zero crossings or a Hilbert transform) is within 1
     cent of the target by 100 ms (linear) or reaches 99% of the interval by 100 ms (exponential, as defined).
   - Constant-rate mode scales linearly with the interval.
   - BPM-synced times match tempo × division to the sample.
   - Identical results at every block size.
5. **Legato and priority.**
   - Hold A, press B, release B: the voice returns to A with no envelope restart in Legato mode, and with a restart
     in Mono mode.
   - Low priority ignores a higher note pressed second.
   - Notes touching within `legato_window` glide; a gap larger than the window does not.
   - A note-off at the same tick as a note-on (offs first, as in the snapshot) is handled as documented.
6. **Fundamental preserved under drive.** An 808 at A1 (55 Hz), `fundamental_lock` on:
   - The Goertzel amplitude at 55 Hz at drive 100% is within **0.1 dB** of drive 0%.
   - Harmonics 2–8 rise monotonically with drive.
   - DC stays below −80 dBFS.
   - With the lock off, the test *detects* the fundamental's loss, which proves it is sensitive.
7. **Neutral means identical.** Drive 0, harmonics 0, duck None, width 100% and mono-below at its minimum are each
   **bit-exact** against the path without them (the Pulsus `saturate` rule).
8. **Chebyshev exactness.** A unit sine through T_n gives energy only at n·f0 (other bins below −100 dB), in f64.
9. **Sub phase lock.** A sub zero crossing falls within ±1 sample of every second main-oscillator crossing,
   including throughout a 1-octave glide and a 200 ms vibrato.
10. **Determinism and isolation.**
    - Two renders are bit-identical.
    - Adding a note on *another track*, or another note on this track outside the tested one's lifetime, does not
      change the tested note's samples. The humanize seed comes from (track, note id): the Pulsus lesson.
11. **Kick duck.**
    - With kick events at known ticks, the gain reaches −depth ±0.1 dB at the kick tick.
    - The dip starts exactly `duck_pre` ms early.
    - Zero latency is reported to plug-in delay compensation.
    - The duck is identical when the kick track is muted (it is event-driven) unless the user chose to follow mute.
    - Audio-sidechain mode matches the Comprimo key path's conventions.
12. **Retrigger declick.** A retrigger in the middle of a cycle produces no sample-to-sample step larger than the
    signal's own maximum slope × 1.5.
13. **Idle.** After release and decay, `process_add` returns false within a bounded time, with no denormals (the
    Firmus convention).
14. **Project and undo.**
    - A schema bump with a fixture project containing Gravis in each engine: save → open → export null.
    - Undo fuzz over every Gravis command, including the cross-device "Tune kick to root".
    - EN/ES keys for every label.
15. **CPU budget.** 6-string String engine, polyphony 6, all features on: under X% of one core at 48 kHz with
    512-sample blocks (set X when the first profile exists). No allocation on the audio thread (the ADR 0005 test
    pattern).

---

## 10. Sources

**Products**
- IK Multimedia, MODO BASS 2 product page and GUI images: https://www.ikmultimedia.com/products/modobass2/
- MODO BASS (v1) announcement, modal-synthesis description **[excerpt]**: https://www.kvraudio.com/news/ik-multimedia-announces-modo-bass---physically-modeled-electric-bass-plug-in-34939 ; https://www.gearnews.com/modo-bass-physically-modelled-electric-bass-ik-multimedia/
- Sound on Sound, MODO Bass 2 review (Paul White, Sept 2022): https://www.soundonsound.com/reviews/ik-multimedia-modo-bass-2
- MusicRadar / Computer Music, MODO Bass 2 review (2022-09-05): https://www.musicradar.com/reviews/ik-multimedia-modo-bass-2
- Spectrasonics Trilian overview, library and buy pages: https://www.spectrasonics.net/products/trilian/overview.php ; https://www.spectrasonics.net/products/trilian/library.php ; https://www.spectrasonics.net/products/trilian/buytrilian.php
- Trilian manual, release noise and legato: https://support.spectrasonics.net/manual/Trilian/1.5/en/topic/soundsource-zoom-articulations ; Smart Update (1.6): https://support.spectrasonics.net/manual/Trilian/1.5/en/topic/smart-update-new
- Sound on Sound, Trilian review (Dave Stewart, Jan 2010): https://www.soundonsound.com/reviews/spectrasonics-trilian
- Spectrasonics Apple Silicon / VST3 update news **[excerpt]**: https://www.ilio.com/blog/spectrasonics-announces-native-apple-silicon-and-vst3-support-for-all-instruments/
- Future Audio Workshop, SubLab XL: https://futureaudioworkshop.com/sublab-xl/ ; product: https://futureaudioworkshop.com/product/sublab-xl/ ; FAQ: https://futureaudioworkshop.com/your-sublab-xl-faqs-answered/
- Sound on Sound news, SubLab XL (2022-07-30): https://www.soundonsound.com/news/sublab-xl-future-audio-workshop
- Rekkerd, SubLab review (2019-07-26): https://rekkerd.org/review-sublab-sub-bass-synthesizer-plugin-by-future-audio-workshop/
- Bedroom Producers Blog, SubLab deal (March 2026) **[excerpt]**: https://bedroomproducersblog.com/2026/03/13/future-audio-workshops-sublab/
- Initial Audio, 808 Studio 2 and 3: https://initialaudio.com/product/808-studio/ ; https://initialaudio.com/product/808-studio/808-studio-3/
- Image-Line, Transistor Bass manual: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Transistor%20Bass.htm
- Image-Line, Channel settings (Polyphony, Mono, Porta, Slide; not supported for VST/AU): https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/chansettings_misc.htm
- M. Musco, Transistor Bass review (March 2026): https://www.michaelmusco.com/2026/03/fl-studio-transistor-bass-review.html
- KVR forum, "ABL3 versus Image-Line Transistor Bass": https://www.kvraudio.com/forum/viewtopic.php?t=528345
- R. Whittle, Devil Fish / TB-303 notes (accent sweep circuit): https://www.firstpr.com.au/rwi/dfish/303-unique.html
- Ample Sound, Ample Bass P Lite: https://www.amplesound.net/en/pro-pd.asp?id=19 ; KVR news v4 (2025-04-18): https://www.kvraudio.com/news/ample-sound-releases-ample-bass-v4-with-special-promotion-63603 ; v4.1 (2026-03-12): https://www.kvraudio.com/news/ample-sound-updates-ample-bass-to-v4-1-66396
- NI Scarbee Pre-Bass **[excerpt]**: https://www.kvraudio.com/product/scarbee_pre_bass_by_native_instruments_ni
- Sound on Sound, Toontrack EZbass review (July 2020): https://www.soundonsound.com/reviews/toontrack-ezbass ; https://www.toontrack.com/product/ezbass/
- Output, "Best Bass VST Plugins" (2026-04-11) **[excerpt]**: https://output.com/blog/best-bass-vst-plugins
- Sound Radix Pi: https://www.soundradix.com/products/pi/
- Waves MaxxBass: https://www.waves.com/plugins/maxxbass
- MIDI Association, MPE: https://www.midi.org/midi-articles/midi-polyphonic-expression-mpe
- Mutable Instruments eurorack repository (STM32 code, including Rings, Elements and Plaits, is MIT; AVR code is GPL-3; hardware is CC-BY-SA): https://github.com/pichenettes/eurorack

**DSP literature**
- K. Karplus, A. Strong, "Digital synthesis of plucked-string and drum timbres", *Computer Music Journal* 7(2):43–55, 1983: https://users.soe.ucsc.edu/~karplus/papers/digitar.pdf
- D. A. Jaffe, J. O. Smith, "Extensions of the Karplus–Strong plucked-string algorithm", *CMJ* 7(2):76–87, 1983; summarised in J. O. Smith, *PASP*, "Extended Karplus-Strong Algorithm": https://ccrma.stanford.edu/~jos/pasp/Extended_Karplus_Strong_Algorithm.html
- M. Karjalainen, V. Välimäki, T. Tolonen, "Plucked-string models: from the Karplus–Strong algorithm to digital waveguides and beyond", *CMJ* 22(3):17–32, 1998: http://users.spa.aalto.fi/vpv/publications/cmj98.pdf
- J. O. Smith, *Physical Audio Signal Processing*, "Stiff string synthesis models" and "Stiff piano strings": https://ccrma.stanford.edu/~jos/pasp/Stiff_String_Synthesis_Models.html ; https://ccrma.stanford.edu/~jos/pasp/Stiff_Piano_Strings.html
- E. Rank, G. Kubin, "A waveguide model for slapbass synthesis", *Proc. IEEE ICASSP* 1997, pp. 443–446, doi:10.1109/ICASSP.1997.599670: https://www2.spsc.tugraz.at/people/franklyn/ICASSP97/pdf/author/ic970443.pdf
- R. C. D. Paiva, J. Pakarinen, V. Välimäki, "Acoustics and modeling of pickups", *JAES* 60(10):768–782, 2012: https://research.aalto.fi/en/publications/acoustics-and-modeling-of-pickups
- S. Bilbao, A. Torin, "Numerical modeling and sound synthesis for articulated string/fretboard interactions", *JAES* 63(5):336–347, 2015, doi:10.17743/jaes.2015.0023: https://www.research.ed.ac.uk/en/publications/numerical-modeling-and-sound-synthesis-for-articulated-stringfret/
- K. J. Werner, J. S. Abel, J. O. Smith, "A physically-informed, circuit-bendable, digital model of the Roland TR-808 bass drum circuit", *Proc. DAFx-14*, 2014: https://www.dafx.de/paper-archive/2014/dafx14_kurt_james_werner_a_physically_informed,_ci.pdf
- A. Szabo, "How to emulate the Super Saw", B.Sc. thesis, KTH, 2010: https://www.adamszabo.com/internet/adam_szabo_how_to_emulate_the_super_saw.pdf
- Cited from knowledge or from reference lists, not re-read this session:
  - T. I. Laakso et al., "Splitting the unit delay", *IEEE SPM* 13(1), 1996;
  - V. Välimäki et al., "Physical modeling of plucked string instruments…", *JAES* 44(5), 1996;
  - S. Van Duyne, J. O. Smith, ICMC 1994 (dispersion);
  - J. Rauhala, V. Välimäki, *IEEE SPL* 2006;
  - J. S. Abel, V. Välimäki, J. O. Smith, *IEEE SPL* 17(4), 2010;
  - T. Tolonen, V. Välimäki, M. Karjalainen, tension modulation, *IEEE TSAP* 2000;
  - G. Evangelista, F. Eckerholm, *IEEE TASLP* 2010;
  - L. Trautmann, R. Rabenstein, *EURASIP JASP* 2004;
  - E. Larsen, R. M. Aarts, "Reproducing low-pitched signals through small loudspeakers", *JAES* 50(3), 2002;
  - T. Stilson, J. O. Smith, "Analyzing the Moog VCF…", ICMC 1996;
  - A. Huovilainen, DAFx 2004;
  - V. Zavalishin, *The Art of VA Filter Design*.
