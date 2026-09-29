# Keys: piano, electric piano and organ — competitive research for Cantus

*Researched 2026-09-24 for Cantus. Sources: the makers' product pages, manuals and store pages (Modartt,
Spectrasonics, Arturia, AAS, Native Instruments, XLN, Image-Line, Garritan), the SFZ Instruments page for
Salamander, Google Patents, press reviews (Sound On Sound, MusicTech, Bedroom Producers Blog, CDM), two user
forums (KVR, Modartt), and the research literature (JASA, IEEE, DAFx, EURASIP, JOS/CCRMA). Everything is
paraphrased. Colour and contrast values were **measured from the official screenshots** (JPEG/PNG, ±2 per
channel; the WebP files could not be measured). Where a figure is our own calculation or estimate, the text says
so. Screenshots are the makers' own, kept in [`screenshots/`](screenshots/) (git-ignored) for internal
reference only.*

*Limits of this round: the shared web-search quota ran out halfway through, so the later sources were found by
fetching known pages directly. VI-Control and Gearspace returned 403, and the Sound On Sound reviews of the
Arturia keys and of Lounge Lizard EP-5 returned 410. Those opinions are missing rather than guessed. The GSi VB3-II
and IK B-3X organ pages could not be read, so they appear only as names. Papers marked "not re-read" are cited
from the literature record, not re-checked this round.*

---

## 0. The short version

1. **The market has split into two camps, and physical modelling won on everything except the attack.**
   - The modelled camp is small and fully playable: Pianoteq 9 is about 50 MB, Lounge Lizard EP-5 under 80 MB,
     and Arturia's keys about 3 GB with the shared runtime.
   - The sampled camp is huge and plays back recordings: Keyscape 77–80 GB, Noire 16 GB. It cannot do true
     half-pedalling or sympathetic resonance. Keyscape's "Realism" switch just loads a second, pedal-down
     sample set.
   - Reviewers still single out the modelled camp's attack and release. SOS heard "surprisingly stiff-sounding"
     bass on Pianoteq 9's flagship Steinway D. CDM rated Lounge Lizard's key-release behaviour below its rivals.
2. **Physical modelling is the only path open to Cantus, and it is well documented.**
   - Cantus's licence rules (ADR 0001: bundled content must be original or CC0) rule out bundling anyone's
     samples. Even Salamander, which is CC-BY 3.0, would need an ADR amendment.
   - The published literature (Bank, Välimäki, Smith, Chaigne & Askenfelt, Stulov, Weinreich, Zambon) gives
     concrete, costed building blocks: hammer constants per register, dispersion allpass orders of 20 or less,
     a multirate soundboard at about 100 operations per sample, and a sustain-pedal model of 1000–1500
     resonators.
   - Modartt's own patent (US 7,915,515) uses offline finite-element modal analysis. It **expires 2027-10-20**.
     We should stay on the published waveguide and modal path, and get a legal read before Chorda ships.
3. **Organs are the cheapest big win.** A tonewheel organ is 91 always-running sine oscillators, 9 bus bars,
   a scanner delay line, a percussion envelope and a rotary speaker. It costs well under 1 % of one core (our
   estimate). Every parameter is documented: gear ratios, crosstalk levels, click burst lengths, rotor rpm and
   spin-up times. Its hardest part, the rotary speaker, is also a **standalone effect** that any Cantus track
   can use.
4. **A modelled electric piano is the second-fastest win.**
   - The physics is small: a cantilever tine or reed (a few modes), a tone bar coupled to it, and a *nonlinear
     pickup*. The pickup is where the "bark" comes from.
   - Arturia, AAS and Pianoteq all expose nearly the same model controls: hammer hardness, pickup distance and
     alignment (or symmetry), damper time, tone-bar resonance, and noises.
   - What they do badly is the release and damper behaviour, **unitless controls** (Arturia shows "Pickup
     Distance 0.444"), and hidden keyboard tracking.
5. **Every product runs into the same plug-in wall, and Cantus doesn't.**
   - The pedal is just CC64 inside a generic controller lane.
   - A rotary speaker can't schedule a speed change so that it *arrives* at full speed on the downbeat, because
     the plug-in can't see the future. The host can.
   - Each plug-in keeps its own undo (Arturia and Pianoteq both have one).
   - Voicing across the keyboard is paywalled: Pianoteq gates note-by-note editing to PRO at $399.
   - None of them lists CLAP among its formats.
6. **A determinism gap we can own.** Pianoteq's "CPU overload detection" skips parts of the computation to keep
   audio running, so a live performance can differ from the offline render. Cantus's ADR 0005 forbids that.
   Our keys will be bit-identical across block sizes and between realtime and render, and we will show a
   budget badge instead of quietly degrading.
7. **Build order:**
   1. **Rota** (tonewheel organ) with **Vertigo** (rotary speaker, also an insert effect).
   2. **Lamina** (modelled EP with a tine engine and a reed engine).
   3. **Chorda** (modelled grand) in two stages.

   Section 8 gives the argument, the parameter lists, the voice architecture and the editor sketches.

---

## 1. Products and versions

| Product | Latest version and date | Price (Sept 2026) | Formats | Size on disk | Source |
|---|---|---|---|---|---|
| **Modartt Pianoteq 9** (modelled) | 9.x; v9 released Oct 2025 | Stage $139, Standard $269, Pro $399; instrument packs extra; upgrade reported at €29 | VST3, AU, AAX, NKS, standalone; Win/macOS/Linux/iOS | **≈50 MB** (SOS: ≈54 MB) | modartt.com; SOS review; KVR news |
| **Spectrasonics Keyscape** (sampled) | 1.5 (Mar 2023, added Double Felt Grand); manual edition 1.3 | $399 (SOS); $379 / €329 quoted in 2023 | VST2, VST3, AU, AAX, standalone | **77–80 GB** full (over 200 GB uncompressed), 30 GB lite; 8 GB RAM | spectrasonics.net; support manual; SOS; Synth Anatomy |
| **Arturia Piano V3** (modelled, "Phi") | current in V Collection 11 (Apr 2025) | $199 | VST3, AU, AAX, NKS, standalone | 3 GB free disk required; 4 GB RAM; 4 cores at 3.4 GHz minimum | arturia.com product and store pages |
| **Arturia Stage-73 V2** (modelled Rhodes) | V Collection 11 | $99 | as above | 3 GB | arturia.com |
| **Arturia Wurli V3** (modelled Wurlitzer 200A) | V Collection 11 | $99 | as above | 3 GB | arturia.com |
| **Arturia B-3 V2** (modelled tonewheel, "TAE") | V Collection 11 | $99 | as above | 3 GB | arturia.com |
| **AAS Lounge Lizard EP-5** (modelled, tine and reed engines) | 5.x, June 2025 | $199 ($149 at launch; upgrades $79 / $49) | VST, AU, AAX, standalone; MPE listed on the product page today | **61–80 MB** download; under 256 MB RAM per instance | applied-acoustics.com; SOS news; BPB |
| **NI Noire** (sampled Yamaha CFX, Nils Frahm) | Kontakt library (2018, from memory) | $149 | Kontakt 7.6+ or the free Player | 16 GB (NI) | native-instruments.com |
| **NI Vintage Organs** (sampled tones, modelled rotator) | 1.5.x | $99 | Kontakt or Player | 1.4 GB download (1.8 GB of samples) | native-instruments.com |
| **XLN Addictive Keys** (sampled) | 1.x; four instruments | €99 each, €249 for all four | VST, AU, AAX, standalone | not stated | xlnaudio.com; SOS (2013) |
| **Image-Line FL Keys** (small sampler) | ships with FL Studio | included | FL native only | its Grand Piano preset reads "3 MB" in the UI | IL online manual |
| **Garritan Abbey Road Studios CFX** (sampled) | ARIA player | $199.95; there is also a Lite edition | AAX, VST, AU (RTAS still listed) | not stated on the pages fetched | garritan.com |
| **Salamander Grand Piano** (sampled Yamaha C5) | V3, 9 Dec 2016 | free, **CC-BY 3.0** | SFZ (any SFZ player) | 1.18 GB (48 kHz/24-bit) or 394 MB (44.1/16) | sfzinstruments.github.io |

**Formats:** not one of these lists CLAP, which matters given Cantus's CLAP-first ADR.

**Benchmarks named but not read this round:** GSi VB3-II and IK B-3X (modelled organs), Rhodes' own plug-ins,
Modartt Organteq (a pipe organ, out of scope).

---

## 2. Screenshots saved

All are in [`screenshots/`](screenshots/). Each was checked for a valid image signature and viewed.

| File | What it shows |
|---|---|
| `ky-01-pianoteq9-pro-ipad-tuning-voicing.webp` | Pianoteq 9 PRO (the official photo, on an iPad). The cream "wood" panel holds Tuning (440 Hz, equal temperament, Unison width, Direct sound duration, an envelope graph), Voicing (hammer hardness pp/mezzo/ff sliders, Spectrum profile bars 1–8, Hammer noise, Hammer tone, Soft pedal) and Design. A "+0.20" value bubble appears while dragging. |
| `ky-02-pianoteq9-design-panel.webp` | The Design panel. A decay-vs-frequency graph with a 2-D control point (soundboard impedance, cutoff, slope) sits above sliders for String length, Strike point, symp/duplex Resonance and Blooming energy/inertia. |
| `ky-03-pianoteq9-mics-mixer.webp` | The Mics & Mix panel: 8 mic channel strips, each named by mic model (DPA4007, U87 Fig 8, C414 Omni, R84…), with M/S, fader, meter, L/R delay, and a mixed output. |
| `ky-04-pianoteq9-ep-design-panel.webp` | The Design panel for the electric pianos. It adds a pair of Tine noise sliders (hammer, damper) to the same decay graph. |
| `ky-05-piano-v3-main.jpg` | Arturia Piano V3 main view. A photo-real piano in a room fills about 55 % of the window. The macro row is Model ‹›, Brightness, Timbre, Dynamics, St. Width, Reverb, Volume and Limiter, above the keyboard. |
| `ky-06-piano-v3-model-advanced.jpg` | Piano V3 "Advanced → Model", with six cards: Action (Hammer Hardness, Hammer Position, Dynamics, Release Time), Acoustics (Lid Position, Soundboard, Sympathetic and Duplex Resonance), Mics (Pop / Stereo Width / Ambient), Strings (Tuning, Unison Detune, Stretch Tuning, Age), Noises (Hammer, Key Release, Sustain Pedal) and Keyboard (Velocity Curve). |
| `ky-07-keyscape-rhodes-main.png` | Keyscape, Rhodes Classic. A browser column sits beside a photo of the instrument. The MAIN tab holds Mix (Direct, Mechanical), Tremolo (Depth, Auto-Pan, Speed) and Performance (Rel Noise with a "T" transition button, Pedal Noise, Dynamics). |
| `ky-08-keyscape-c7-tweak.png` | Keyscape, LA Custom C7, TWEAK tab: Image Mono, Feel (Attack Natural/Gentle/Tight, Rel Transition, Rel Time), Sustain Realism, Pedal Effects, and Damper "No Damper Above F6". |
| `ky-09-fl-keys.png` | FL Keys: a 722×406 panel with 16 grey knobs. Environment has Decay, Release, Pan and Stereo; Misc has Overdrive, LPD, Treble and Stretch; Velocity has →Muffle, →Hardness and Sens; Tuning has Tune and Detune. The preset reads "Grand Piano (3 MB)". |
| `ky-10-stage73-v2-advanced.jpg` | Arturia Stage-73 V2 Advanced, showing a velocity curve editor and a settings table. Instrument: Model Mark V B, Tuning 440 Hz, Hammer Hardness 0.446. Mechanics: Pickup Distance 0.380, Pickup Alignment 0.600, Damper Duration 0.034. Noises: Pickup −64 dB, Hammer 0.107, Tine −6 dB, Damper −7.62 dB. Acoustics: Dynamics 40 dB, Tonebar Resonance 0.319. |
| `ky-11-wurli-v3-advanced.jpg` | Wurli V3 Advanced: Model 200a, Hammer Hardness 0.400, Age 0.352, Reeds Noise −6 dB, Tonebar Resonance 0.300, Output Mode Direct Stereo, Vibrato Speed 6.0 Hz, Pickup Distance 0.444, Pickup Alignment 0.342, Damper Duration 0.250, Dynamics 50 dB. |
| `ky-12-b3-v2-advanced-drawbar-mod.jpg` | B-3 V2 Mod page. The Drawbar Modulator (LFO / envelope / step sequencer moving individual drawbars) sits beside Advanced Settings: Tonewheel Profile 1963 B-3, Tonewheel Leakage 0.012, Drawbar Leakage 0.104, Brilliance, Key Click, Percussion Soft 3 dB / Normal 7 dB, Slow 400 ms / Fast 140 ms, Vol Drop, Disable 1', Polyphonic, and envelopes. |
| `ky-13-b3-v2-rotary-cabinet-panel.png` | B-3 V2 top view with the rotary speaker's control box: Balance, Stereo Width, Horn Accel, Drum Accel, Mix, Open/Closed model, Slow Speed, Fast Speed, Reverb, Dry/Wet. Half-moon Slow/Fast and Stop/Run switches, pedals and the swell pedal are also visible. |
| `ky-14-lounge-lizard-ep5-reed-and-tine.webp` | Lounge Lizard EP-5, both engines in Expert view. The Reed engine has Hammer (Stiffness, Force, Noise, each with KEY and VEL mini-knobs, plus Pitch), Reed (Overtones, Fundamental, Decays, Release), Damper, EQ, Pickup (In, Tone, Key, Sym, Level) and Tremolo. The Tine engine has "Fork" (Tine, Color, Tone Bar, Decays) and a Pickup with Distance. The FX strip has Character, EQ, Compressor, Phaser/Chorus, Guitar Amp and Reverb. |
| `ky-15-pianoteq9-note-edit.webp` | Pianoteq PRO Note Edit: one yellow bar per key on a grey keyboard grid, with blue anchor handles and interpolation between them. This is the per-key curve editor. |

---

## 3. Anatomy of each product

### 3.1 Pianoteq 9 (Modartt)

- **Model.**
  - Modartt's patent describes the method. Strings are elastic beams and the soundboard is a shell finite-element
    model, coupled at the bridge. A nonlinear eigenvalue problem gives each partial *n* a frequency *fₙ* and a
    damping *dₙ*. Those "timbre coefficients" are then **interpolated in real time** from user parameters such as
    impedance and detuning.
  - It is, in effect, **modal synthesis whose coefficients come from offline FE analysis**. That explains the
    PRO feature "thousands of editable overtones".
  - The patent's priority date is 2006-07-28 and it expires on 2027-10-20.
- **Panels** (manual, ky-01, ky-02):
  - **Tuning:** diapason, temperament (7 built in, plus Scala), unison width, unison balance, octave stretching
    (1 = natural, derived from inharmonicity), direct sound duration. Retuning offers "String tension" (timbre
    changes, because inharmonicity rises as tension falls) or "Full rebuild".
  - **Voicing:** hammer hardness, global or at three velocities (~41, ~70, ~98); Spectrum profile (the first 8
    overtones); hammer noise; hammer tone (new in v9); soft-pedal smoothing. The electric pianos also have
    **Pickup symmetry** (centring the pickup makes the note jump an octave) and **Pickup distance** (closer
    means more distortion and more soft/loud timbre change).
  - **Design:** soundboard impedance, cutoff and Q as one 2-D point on a decay graph; string length (up to a
    "10-metre piano", which has almost no inharmonicity); strike point with humanise; sympathetic and duplex
    resonance; blooming energy and inertia; tine noise for hammer and damper on the EPs.
  - **Action:** damper position (which overtones decay on release, most audible in the bass with partial
    pedals); damping duration; last damper (MIDI note); mute; key release noise; sustain-pedal "whoosh".
  - **Pedals:** twelve assignments. The sustain pedal is progressive ("partial pedals"). There are also una
    corda, sostenuto, harmonic pedal, super-sostenuto, and pinch-harmonic, rattle, buff, celeste, glissando,
    Mozart rail and (new in v9) thunder pedals.
  - **Condition:** from "mint" to worn out, with a random seed.
  - **Mics & Mix:** up to 8 virtual mics (ky-03). You choose model and polar pattern (omni, cardioid,
    figure-8), use AB/XY/ORTF/MS layouts, or a binaural head. Per mic: volume, pan, delay, polarity. There is
    also proximity compensation, lid position, and "sound speed".
  - **Note Edit** (PRO only, ky-15): 30 parameters per key.
  - **Morphing and layering:** morphing happens *in the physics*.
  - **Effects:** 5-band EQ, tremolo, wah, chorus/flanger/phaser, fuzz, amp, compressor, delay, and a
    convolution reverb with a low-cut.
- **Performance.**
  - Options → Perf offers multicore rendering, **CPU overload detection** (which skips tasks), internal sample
    rate (Stage/Standard up to 48 kHz, PRO up to 192 kHz), and polyphony (Auto pessimistic/optimistic).
  - Polyphony counts "individual sounds (notes, sympathetic resonances…)", and bass notes cost more.
- **CPU figures from users** (forum-grade):
  - Raspberry Pi 4: 48-voice polyphony ≈ 30 % audio load. Another user reached 98 voices at 1.75 GHz with a
    real-time kernel, with a reduced internal rate.
  - A Ryzen 1600X reached a performance index of 125 at 60 voices. An i7-4790K sustained over 250 voices.
  - The Perf panel screenshot shows an Apple M3 Max at index 75–99.

### 3.2 Keyscape (Spectrasonics)

- **Content.** 36 keyboards, 465 patches and 185 soundsources in 11 categories: acoustic pianos (LA Custom
  C7, Double Felt Grand, Wing Tack/Upright), electric pianos (Rhodes variants, Wurlitzer, Vintage Vibe, Pianet,
  Weltmeister, CP70), Clavinet, belltone keyboards, toy pianos, mini pianos, plucked keyboards, wind keyboards,
  vintage digital (JD-800, MK-80, MKS-20) and "Duo" hybrids.
- **Sampling.** MusicTech reports 32 velocity layers plus round robins, with releases and mechanical noises
  recorded as separate layers.
- **Controls** (ky-07, ky-08): each model gets its own macro page (MAIN, TONE, EFFECTS, SETTINGS, INFO).
  - **Rhodes:** Mix (Direct, Mechanical), Tremolo (Depth, Auto-Pan, Speed), Performance (Rel Noise with a "T"
    button that lengthens the sustain→release transition, Pedal Noise, Dynamics). The Tone tab has Signal
    (Direct↔Speaker), Bass/Mid/Treble, De-noise, Color Shift and Tape.
  - **C7:** Feel (Attack Natural/Gentle/Tight, Rel Transition, Rel Time), Sustain "Realism" (loads *both*
    pedal-up and pedal-down layers; off loads only pedal-up), Pedal Effects, and "No Damper Above" (F6 by
    default).
  - **Clavinet:** pickup selection A, B, A+B and A−B (out of phase).
- **Deep editing** needs Omnisphere 2.

### 3.3 Arturia Piano V3, Stage-73 V2, Wurli V3, B-3 V2

- **Shared frame** (ky-05, ky-10, ky-11, ky-12):
  - Header: menu, product name in the product's colour, library, favourite, preset field with ‹ ›, Advanced
    and FX tabs, settings.
  - Body: a large photo-real rendering of the instrument.
  - Footer: undo, history, redo, CPU %, and **four macro knobs** (Brightness, Timbre, Time, Movement; the B-3
    has Brightness, Drive, Time, Vibrato).
  - The FX page (Stage-73) has 4 pedal slots, an Amp (Twin, with On-Axis/Bright, Drive, EQ, and the amp's
    tremolo) and a Room.
- **Piano V3.** 12 models, from American/German/Japanese grands and uprights to Plucked, Tack, Glass and Metal
  grands. The Model page is ky-06. Mics and a convolution room are included.
- **Stage-73 V2.** "Reverse-engineered… component by component". It covers Stage and Suitcase, '73 and '74,
  Mark I and Mark V, and High Tines. Advanced settings are in ky-10. The Suitcase view shows the preamp
  (Volume, Treble, Bass, Vibrato Intensity/Speed).
- **Wurli V3.** Models the 200A: reeds, pickup, preamp and output circuit. The "vibrato" is really tremolo.
  Advanced settings are in ky-11, including Output Mode Direct/Speaker and Vibrato Sync.
- **B-3 V2.**
  - Upper and lower manuals and pedal drawbars, V1–V3 and C1–C3 with separate upper/lower switches,
    percussion, preamp drive, half-moon Slow/Fast and Stop/Run switches.
  - Tonewheel profiles by vintage, adjustable **tonewheel and drawbar leakage**, key click, background noise.
  - Percussion volume and decay in dB and ms, "Vol. Drop", "Disable 1'" (percussion steals the 1' drawbar),
    and a Polyphonic percussion mode.
  - The **Drawbar Modulator**: up to ten LFO, envelope or step-sequencer sources moving drawbars, with
    positive and negative amounts.
  - The rotary cabinet: Balance, Stereo Width, Horn/Drum Accel, Mix, Open/Closed, Slow/Fast speed (ky-13).
  - It can also run through a Twin Amp instead of the rotary.

### 3.4 Lounge Lizard EP-5 (AAS)

- **Two engines** (ky-14).
  - **Tine:** Hammer (Stiffness, Force, Noise, Pitch), "Fork" (Tine, Tone Bar, Color, separate Decays,
    Release), Damper (Amount, Tone, Attack-Release), EQ, Pickup (In, Distance, Symmetry, Level) and Tremolo
    (Depth, Shape, Rate, Stereo, Sync).
  - **Reed:** Hammer, Reed (Overtones, Fundamental, Decays, Release) and a Pickup with In/Tone/Sym.
- **Keyboard tracking and velocity per parameter.** Nearly every model parameter has **KEY and VEL mini-knobs**
  beside it, so how it changes up the keyboard and with velocity is visible.
- **Views.** Simple/Expert toggle; resizable GUI.
- **FX strip.** A Character block (Luxy, Groovy, Phat, Crystal, Amp) followed by EQ, Compressor, Chorus or
  Phaser, Guitar Amp and Reverb.
- **Modelling.** AAS says the modelling "takes into account the different parts of the fork and their
  coupling", which gives more complex attacks.
- **Also:** Scala tuning, under 256 MB of RAM, and over 150 factory presets. MPE: CDM said there was none at
  launch; the product page now lists it.

### 3.5 The sampled rest

- **Noire.** Yamaha CFX, Pure and Felt variants, and a "Particles" engine that generates algorithmic
  accompanying figures.
- **Vintage Organs.** B-3, C-3, M-3, Vox Continental II and Farfisa Compact, chromatically sampled. The
  "Rotator", 8 cabinets and reverb are modelled in real time on top of the samples.
- **Addictive Keys.** Studio Grand (Steinway D), Modern Upright (U3), Mark One (Rhodes) and Electric Grand
  (CP-80).
  - Up to 7 mic perspectives, 3 loaded at once, through a 6-channel mixer. SOS (2013) notes its sends are
    pre-fader only.
  - An Explore page (presets on a map) and an Edit page (pitch envelope, filters, per-channel chains).
- **FL Keys** (ky-09). A tiny sampler with Decay, Release, Pan (auto-pan↔tremolo), Stereo, Overdrive, LPD,
  Treble, **Stretch (±50 cents)**, Muffle and Hardness (each velocity-mapped), Sensitivity, Tune (±100 cents)
  and Detune. Grand, Rhodes and "Roto Organ" presets. It is the FL-parity baseline: cheap and immediate, but
  with no pedal realism at all.
- **Garritan CFX.** Separate pedal-up, pedal-down and soft-pedal sample sets, "partial pedaling", 3 mic
  perspectives (Classic, Contemporary, Player), convolution reverb, ARIA player.
- **Salamander.** Yamaha C5 sampled every **minor third** at **16 velocity layers**, two AKG C414s in AB about
  12 cm above the strings, hammer-noise releases in one layer, and string-resonance releases in minor thirds in
  3 layers.

---

## 4. The DSP

### 4.1 Acoustic piano

**String stiffness and inharmonicity** (Fletcher 1964):
- Partial frequencies follow *fₙ = n f₀ √(1 + B n²)*. B grows with wire diameter to the fourth power and falls
  with length squared and tension, so short, thick treble strings are the most inharmonic.
- Roughly, B is of order 10⁻⁴ through the bass and middle and approaches 10⁻² in the top octave. That is an
  approximate literature range; we must calibrate per register.
- Two consequences:
  1. Tuners stretch the octaves. A **natural stretch** comes from matching the 2:1 (or 4:2) partial
     coincidences, and grows to tens of cents at both ends: the Railsback curve.
  2. Inharmonicity is *heard more easily in the bass*, even though B is smaller there, because listeners use
     beating in the higher partials (Järveläinen et al., cited in Bank et al. 2003).
- Pianoteq's manual says its tuning "takes into account the inharmonicity… the same way a piano tuner does",
  and that an octave-stretching value of 1 means natural stretch.

**Strings as digital waveguides** (Smith; Bank et al. 2003; Bensa, Bilbao, Kronland-Martinet & Smith 2003):
- A delay line holds one period. A loss filter sets the per-partial decay, typically *σ(f) = b₁ + b₃f²*.
- A cascade of allpass filters adds dispersion, and a fractional-delay filter (Thiran or Lagrange) fine-tunes
  the pitch.
- **Dispersion design is the hard part.**
  - Rocchesso & Scalcon showed that several tens of partials can be placed correctly on any key with an allpass
    order of **20 or less**. The Bank et al. example uses order 18 for C2.
  - Rauhala & Välimäki (2006) gave a *tunable* design from Thiran's closed-form group delay. Abel, Välimäki &
    Smith (2010) gave a robust closed form for very high orders.
- Perception gives slack. Deviations between **−25 % and +40 %** in a note's overall decay time are not heard
  (Tolonen & Järveläinen, cited in Bank et al. 2003).

**Strings as modal banks** (Bank, Zambon & Fontana 2010):
- Each string is a set of parallel second-order resonators placed *exactly* at the inharmonic partials. The
  hammer interacts with the modal displacement at the strike point.
- Advantages: exact tuning and stretch; per-partial decay; SIMD-friendly; trivially independent of block size.
- Cost grows with the number of partials, so the low bass is the hot spot. This is also the family Pianoteq's
  "editable overtones" belongs to.

**The hammer** (Chaigne & Askenfelt 1994; Stulov 1995):
- It is a mass on a nonlinear spring: *F = k·max(0, y_h − y_s)ᵖ*, plus hysteresis in Stulov's felt model.
- Constants per register (Chaigne & Askenfelt's values, as tabulated in Bank et al. 2003):

  | Note | p | k | hammer mass |
  |---|---|---|---|
  | C2 | 2.3 | 4.0×10⁸ | 4.9 g |
  | C4 | 2.5 | 4.5×10⁹ | 2.97 g |
  | C6 | 3.0 | 1.0×10¹² | 2.2 g |

- Contact lasts **about 2 ms**, and reflections from the string show up as pulses in the force.
- Because the spring hardens as it compresses, **brightness rises with velocity by itself**. That is the
  "continuous velocity, not 127 sample layers" argument.
- The coupled hammer and string form a delay-free loop, which needs a predictor or an implicit step. Bank's
  formulation stays stable up to about **10×** the impact velocity at which the straightforward method fails.

**Commuted synthesis** (Smith & Van Duyne 1995; JOS, *PASP*):
- Because the string and soundboard are linear and time-invariant, they can swap places. The soundboard's
  impulse response then becomes the excitation table, and the hammer becomes one to three velocity-dependent
  low-pass "force pulses".
- It is very cheap. The catch: it cannot reproduce re-striking a moving string, or sympathetic coupling, which
  needs a shared bridge.

**Double decay and beating** (Weinreich 1977):
- Two or three unison strings, slightly mistuned and coupled through the bridge, give a fast "prompt sound" and
  a slow "aftersound". Pianoteq's *Unison width* and *Direct sound duration* control exactly this.
- It can be built with two coupled waveguides, or more cheaply with **parallel second-order resonators added to
  one string**: the average cost is **under 5 multiplications per sample per note** with multirate (Bank).

**Soundboard** (Bank et al. 2003):
- High modal density. As a black box it needs about a **2000-tap FIR at 44.1 kHz**, and IIR filters do not
  shorten that much.
- The multirate model splits at 2.2 kHz: the low band is decimated by 8 into a high-order filter, and the high
  band goes through a 50-tap minimum-phase filter. It costs **about 100 operations per sample** for a similar
  spectrum, though high notes get a slightly sharper attack.
- Feedback delay networks (FDNs) are the other option. They double as the coupling medium for the
  sustain-pedal effect (Borin et al.).
- Pianoteq 9's headline change is "an improved model of the soundboard vibration" for stereo and spatial
  perception. Its mics measure pressure and pressure gradient at any point around the modelled sound field.

**Sustain pedal, sympathetic resonance, half-pedalling:**
- Lehtonen et al. (2007) measured what the pedal does: it *raises* partial decay rates (especially mid-range),
  changes the beating, and increases the residual energy.
- Zambon, Lehtonen & Bank (DAFx-08) calibrate **1000–1500 parallel resonators** (4 multiply-adds each) from
  measured soundboard and string-register impulse responses. Multirate and SIMD make that affordable. In 2008
  they listed **half-pedalling as an open problem**.
- The commercial state of the art:
  - Pianoteq: progressive pedal, damper position, re-pedalling, harmonic pedal.
  - Garritan CFX: "partial pedaling" across separately sampled sets.
  - Keyscape: a switch between pedal-up and pedal-down sample sets.

**Also in a good piano:**
- Dampers: position, time, the last damper at about F6, mute pressure.
- Una corda: the action shifts so the hammer meets fewer strings, with softer felt.
- Duplex-scale resonance.
- Longitudinal "phantom" partials (Bank & Sujbert 2005): the metallic bass clang.
- Key-release, damper and pedal noises. Pianoteq notes some of its noises are *sampled*, some modelled.
- The hammer re-striking a string that is already moving.

**A full FE model** (Chabassier, Chaigne & Joly 2013) is the physical reference, far from real time. Bank &
Chabassier (IEEE SPM 2019) review the spectrum from there to real-time models.

**What is hard, in practice:**
1. The **attack**: knock, shank and key-bottom noise, and the first 20 ms, where "stiff" and "synthetic"
   complaints come from. Bank et al. call attack noise "the most important issue" for radiation.
2. **Bass realism**: many partials, phantom partials, beating.
3. **Tuning the dispersion** to the stretch target.
4. **Pedal-down cost** at full polyphony.
5. **Stability** of the hammer–string loop at ff.
6. **Denormals** in long decays.

### 4.2 Electric piano

**Rhodes**
- A hammer (neoprene tip) strikes a **tine**, a stiff spring-steel cantilever whose pitch is set by a tuning
  spring. The tine is bolted to a brass **tone bar**, forming an asymmetric tuning fork.
- Muenster, Pfeifle, Weinrich & Keil (JASA 2014) measured the coupling:
  - The nearly linear, lightly damped tine is the *generator*. It enslaves the tone bar in phase or antiphase,
    even though the bar's own frequencies are very different.
  - The tone bar adds the glockenspiel-like component to the attack and **extends the sustain**.
- Cantilever physics gives the tine's upper modes at about **6.27× and 17.5×** the fundamental (textbook
  clamped-free beam ratios; the tuning spring shifts them). These inharmonic modes decay fast and are part of
  the "bell".
- **Pickup.** An electromagnetic coil. Its output follows the change in magnetic flux as the tine tip swings
  past the pole piece.
  - The flux is a nonlinear function of the tip's position. **Centred** (symmetric), the pickup doubles the
    frequency; Pianoteq calls this the "octave jump".
  - **Off-centre**, the output is mostly the fundamental with *asymmetric* distortion. Hard strikes drive the
    tip into the steep part of the curve, and that is **the bark**.
  - Pfeifle & Münster (2016/17) confirm with high-speed video and FEM that the timbre difference between Rhodes
    and Wurlitzer comes mainly from their pickups.
  - Falaize & Hélie (JSV 2017) give a passive, energy-conserving (port-Hamiltonian) simulation of a Rhodes that
    includes the nonlinear pickup.
- **Suitcase models** add a preamp and a stereo "vibrato", which is really auto-pan.

**Wurlitzer 200A**
- A felt hammer strikes a **steel reed** with solder at the tip for tuning. The reed sits in the slot of a
  comb-shaped plate held at a high DC voltage: an **electrostatic** pickup.
- The moving reed changes the capacitance, so the output current is *V·dC/dt*. As the reed enters the slot,
  *C(x)* grows steeply, which gives the growl at high velocity (Pfeifle & Münster).
- The tremolo acts inside the preamp. Arturia's default rate is 6.0 Hz.
- The small built-in speakers are an output option ("Direct / Speaker" in Arturia and Keyscape).

**What the market exposes** (a strong hint at the right parameter set):
- Hammer hardness (with key and velocity tracking).
- Pickup distance and alignment (symmetry).
- Damper duration, amount and tone.
- Tone-bar resonance.
- Hammer, tine/reed, damper and pickup noises.
- Dynamics range in dB: Arturia uses 40 dB for the Rhodes and 50 dB for the Wurli.
- Age/condition.
- Output direct or speaker.

**What is hard:**
1. The release: damper felt landing, the pitch dip and noise. CDM singled out Lounge Lizard's release as weaker
   than its rivals'.
2. The attack "bell" balance.
3. **Aliasing from the pickup and preamp nonlinearities**. Use antiderivative anti-aliasing (ADAA) or 2× local
   oversampling.
4. Consistent velocity response. SOS criticised Addictive Keys' sampled Rhodes for velocity-switching gaps,
   which a model avoids by construction.

### 4.3 Tonewheel organ and rotary speaker

**Tone generator**
- A synchronous motor drives 91 tonewheels (1200 rpm on 60 Hz mains). They spin **continuously**, so phases
  are never retriggered.
- Pitches come from 12 gear-ratio pairs (the ratios as commonly published and used by setBfree). Worked for the
  lowest octave (2-tooth wheels, 20 rev/s), our calculation gives every note within **0.71 cents** of equal
  temperament, and A exact:

  | Note | Ratio | Error |
  |---|---|---|
  | C | 85/104 | −0.58 ct |
  | C♯ | 71/82 | −0.68 ct |
  | D | 67/73 | +0.20 ct |
  | D♯ | 105/108 | −0.09 ct |
  | E | 103/100 | −0.14 ct |
  | F | 84/77 | −0.68 ct |
  | F♯ | 74/64 | +0.03 ct |
  | G | 98/80 | +0.02 ct |
  | G♯ | 96/74 | −0.71 ct |
  | A | 88/64 | 0.00 ct |
  | A♯ | 67/46 | −0.29 ct |
  | B | 108/70 | −0.59 ct |

- That tiny, fixed mistuning is part of the sound, so it should be a temperament option ("Gear 60 Hz"), like
  setBfree's `gear60`.

**Drawbars and keys**
- Each key closes **9 contacts** onto 9 bus bars: 16′, 5⅓′, 8′, 4′, 2⅔′, 2′, 1⅗′, 1⅓′, 1′. Relative to 8′ these
  are harmonics ½, 1½, 1, 2, 3, 4, 5, 6 and 8.
- There are 91 wheels for 61 keys × 9 bars, so the ranges **fold back** at the ends.
- Drawbars have 9 positions (0–8). Each step is commonly given as about 3 dB (not verified this round).

**Leakage and crosstalk**
- setBfree models four sources: tone-generator compartment, transformer, terminal strip and wiring. Its
  defaults are around **0.01 linear (≈ −40 dB)**.
- B-3 V2 exposes "Tonewheel Leakage" and "Drawbar Leakage" separately.

**Key click**
- Contact bounce as each of the 9 contacts closes at a slightly different moment. setBfree's default model is
  "several random contact bounces", with bursts of **0.1–0.5 units of about 5.87 ms** and a smaller release
  click.
- Real, loved, and **must be deterministic** in Cantus (seeded).

**Percussion**
- A decaying copy of the 2nd (4′) or 3rd (2⅔′) harmonic bus, upper manual only, with fast/slow decay and
  normal/soft level.
- It is **single-trigger**: it only re-arms when all keys are up, so legato does not retrigger.
- On the hardware it **steals the 1′ drawbar**, and normal percussion lowers the manual's volume.
- Defaults disagree across references, because each defines decay differently:
  - setBfree: fast 1.0 s, slow 4.0 s; soft = 0.5 × normal.
  - B-3 V2: Slow 400 ms, Fast 140 ms; Soft 3 dB, Normal 7 dB.

  We must define our unit (for example, time to −60 dB) and calibrate against a real instrument.

**Vibrato and chorus**
- A lumped LC delay line is scanned by a rotating capacitive scanner at about **7 Hz** (setBfree default).
- V1, V2 and V3 are increasing depths (setBfree's relative depths are 1.0, 2.5 and 5.0). C1, C2 and C3 mix the
  scanned signal with the dry signal.
- The sound comes from *stepped* tap interpolation along the line, not from a smooth delay LFO.

**Preamp and overdrive**
- The tube overdrive, including the Leslie 122's amplifier, is an asymmetric soft clip after the swell pedal.
  It needs oversampling or ADAA.

**Rotary speaker** (Smith, Serafin, Abel & Berners, DAFx-02)
- **Doppler.** Simulated with *interpolating and de-interpolating delay lines*. Multiple read pointers model
  the direct and reflected paths, and two fixed listeners (ears or mics) give stereo.
- **Directivity.** Measured horn impulse responses calibrate angle-dependent filters. They had a large
  angle-independent component, separated from the angle-dependent one by a relaxation algorithm.
- **Drum.** The bass rotor is a rotating baffle in front of a fixed woofer, so it is mostly amplitude and filter
  modulation, not Doppler.
- Herrera, Hanson & Abel (AES 2009, not re-read) and Pekonen, Pihlajamäki & Välimäki (DAFx-11, "Computationally
  efficient Hammond organ synthesis", not re-read) are the other standard references.
- **setBfree's defaults** (GPL-2 project; we can read its documentation, never copy its code):

  | Setting | Horn | Drum |
  |---|---|---|
  | Slow | 40.32 rpm (0.67 Hz) | 36 rpm (0.60 Hz) |
  | Fast | 423.36 rpm (7.06 Hz) | 357.3 rpm (5.96 Hz) |
  | Spin-up time constant | 0.161 s | 4.127 s |
  | Spin-down time constant | 0.321 s | 1.371 s |
  | Radius | 17 cm | 22 cm |
  | Other | horn leak 0.15 | drum filter centred near 812 Hz |

- **Our calculation from these:**
  - At fast speed the horn tip moves at about 7.5 m/s, a Doppler swing of about **±37.6 cents** and a delay
    swing of about **±0.5 ms** (±22 samples at 44.1 kHz).
  - At slow speed the swing is only about ±3.6 cents, so the chorale sound is mostly amplitude modulation and
    directivity.
- The drum's slow spin-up (a time constant of seconds) is *the* gesture players perform. It is why
  **scheduling** matters (section 8).

**Cost** (our estimate): 91 oscillators, bus-bar sums, a scanner of about 20 taps, two rotors with a
modulated delay and a few biquads each, and an oversampled waveshaper. That is **under 1,500 operations per
sample**, a fraction of 1 % of a modern core, **independent of how many keys are held**.

### 4.4 CPU numbers in one place

| Item | Figure | Source |
|---|---|---|
| Pianoteq on Raspberry Pi 4 | 48 voices → ≈30 % audio load; 98 voices with an overclock, a real-time kernel and reduced internal rate | Modartt forum (user reports) |
| Pianoteq on desktop CPUs | Ryzen 1600X: index 125 at 60 voices; i7-4790K: over 250 voices sustained | Modartt forum |
| Pianoteq polyphony units | notes *and* resonances count; bass notes cost more | manual |
| Lounge Lizard EP-5 | under 256 MB RAM per instance | AAS |
| Keyscape | 8 GB RAM recommended; SSD for streaming | Spectrasonics; MusicTech |
| Arturia keys | 4 GB RAM minimum; 4 cores at 3.4 GHz recommended | Arturia |
| Beating / two-stage decay | under 5 multiplications per sample per note (multirate) | Bank et al. 2003 |
| Soundboard | 2000-tap FIR equivalent in about 100 operations per sample (multirate) | Bank et al. 2003 |
| Sustain-pedal resonator bank | 1000–1500 resonators × 4 multiply-adds per sample (before multirate) | Zambon, Lehtonen & Bank 2008 |
| Dispersion | allpass order ≤ 20 per string | Rocchesso & Scalcon, via Bank et al. |

---

## 5. The interface: praise and criticism

**Pianoteq** (ky-01 to ky-04, ky-15)
- **Praise:**
  - It is the rare *light* skeuomorphic UI: a cream "wood" panel with dark text.
  - Every physical parameter has a small graph: a direct/remanent envelope under Tuning, spectrum-profile bars,
    and a Design graph where one draggable point sets impedance, cutoff and slope together.
  - A value bubble ("+0.20") appears while dragging.
  - The Note Edit bar-per-key editor with anchors and interpolation is the best keyboard-wide editing idea in
    the category.
  - The mixer shows mic *models* as photos.
- **Criticism:**
  - Controls are sliders in relative units, not physical units (hardness shows as "+0.20").
  - Dense, small type.
  - Per-note editing is locked behind the $399 PRO edition.
  - The window was fixed-size until v9.
  - The "Condition" slider's randomness is hidden behind a right-click.

**Keyscape** (ky-07, ky-08)
- **Praise:**
  - The Omnisphere shell (browser column, a photo of the instrument, a tabbed macro page) is clear.
  - Labels are legible: measured #BBBBBB on #202024 ≈ **8.5:1**.
  - Knobs are bright chrome on dark (≈7:1).
  - Model-specific controls use the instrument's own vocabulary ("No Damper Above F6").
- **Criticism:**
  - Knobs show no values at rest.
  - Shallow editing; the deep layer needs Omnisphere.
  - Only two mic sets (MusicTech).
  - "The sheer… number of creative options can cause dizziness" (SOS).

**Arturia** (ky-05, ky-06, ky-10 to ky-13)
- **Praise:**
  - One family frame across all V instruments: the same header, footer, undo/history and four macros. This is
    the FabFilter lesson again.
  - The Advanced pages are **tables of label + value** in high contrast: Stage-73 labels #CACACA on #303030 ≈
    8.1:1, values ≈ 9:1; Wurli and B-3 white on #202020 ≈ 16:1. They are dense and scannable.
  - B-3's **Drawbar Modulator** is a genuinely new idea.
  - The rotary cabinet panel (ky-13) puts every Leslie control on one face.
- **Criticism:**
  - **Unitless values** everywhere: Pickup Distance 0.444, Alignment 0.342, Tonewheel Leakage 0.012, even
    Damper Duration 0.250. Only a few have units (dB, Hz, ms).
  - The photo-real instrument takes over half the window as decoration.
  - Control boundaries fail WCAG 3:1. Measured: dark knob bodies on Piano V ≈ **1.95:1**, the Stage-73 dropdown
    ≈ **1.20:1**, the B-3 value boxes and buttons ≈ 2.6–2.7:1.
  - Section headers are dim: #8D8E90 on #28282C ≈ 4.5:1, and B-3's "UPPER MANUAL" ≈ **3.8:1**, which fails AA
    for small text.

**Lounge Lizard EP-5** (ky-14)
- **Praise:**
  - A **colour identity per engine**: a red chassis for Reed, black for Tine.
  - The Simple/Expert toggle.
  - **KEY and VEL mini-knobs beside every model parameter.** That is the right pattern for physical models,
    where almost everything should track the keyboard and velocity.
- **Criticism:**
  - Knob scales read 1–10 with no units.
  - Very small labels on brushed metal.
  - No velocity-curve editor (CDM).
  - The Simple view hides the model.

**FL Keys** (ky-09)
- **Praise:** it fits in 722×406, and Stretch, Hardness and Muffle are one knob each.
- **Criticism:** labels are #717173 on #1C1C1C ≈ **3.5:1** and headers ≈ 3.8:1 (both fail AA); knobs have no
  values; there is no pedal, damper or resonance model.

**Keep:**
- Keyboard-wide curves (Pianoteq Note Edit).
- A physical graph per section.
- A label + value table for advanced settings (Arturia).
- Per-parameter KEY and VEL (AAS).
- A consistent family frame and macros (Arturia).
- The instrument's own vocabulary (Keyscape).

**Avoid:**
- Unitless numbers.
- Decorative photos at half the window.
- Features behind a paywall or a right-click.
- Values only while dragging.
- Sub-3:1 control edges.

---

## 6. What reviewers and users say

*Forum caveat: forum posts are individual opinions, not measurements, and forums skew toward enthusiasts. They
are paraphrased and attributed to the forum, not to named people. VI-Control and Gearspace were unreadable this
round (403).*

- **Pianoteq 9** (SOS review):
  - Sound: the new soundboard model brings "a noticeable improvement in the warmth and fullness", and the
    pedal resonance and decay quality are "exceptional".
  - Criticism: "fleeting unnaturalness" in some presets, and "surprisingly stiff-sounding bass notes" on the
    flagship Steinway D. The new Sombre presets don't have the problem.
  - Verdict: "a really welcome release".
- **Pianoteq on the KVR forum.** Every version changes the physics, so each upgrade divides people. Some prefer
  v7 and call v8 "EQ'd". Others answer that this means preferring "less accurate physics". The €29 upgrade is
  praised.
  - *Lesson for us:* a model update changes existing projects' sound. Cantus needs **model versioning in the
    project file**, so old projects keep sounding the same.
- **Keyscape.**
  - SOS: "magnificent, authentic, definitive"; but the release samples are "often intrusive", and the options
    are dizzying.
  - MusicTech: the C7 is "up there with the best"; editing is limited without Omnisphere; only two mic sets;
    heavy RAM and SSD demands.
- **Lounge Lizard EP-5.**
  - BPB: "puts AAS back into the top tier of modeled electric pianos"; both engines authentic; tiny footprint
    compared with Keyscape's 80 GB; the experimental library is a "mixed bag".
  - CDM: the attack articulations are outstanding; velocity sensitivity is "outstanding"; key-release
    characteristics face "fierce competition"; no MPE or poly aftertouch at launch.
- **Addictive Keys** (SOS, 2013): fast loading and good recordings; the Rhodes had velocity-switching
  inconsistencies; installation required going online.
- **Arturia.** No independent reviews were retrievable this round. The only quotes found are the vendor's
  curated ones ("no annoying 60 cycle hum").
- **General pattern** (a widely repeated view that we did not re-source this round): modelled pianos are
  praised for playability, dynamics and pedal behaviour, and criticised for attack realism and a "synthetic"
  top end. Sampled pianos are the reverse.

---

## 7. Gaps none of them fills

1. **The pedal is a second-class citizen.** Every product gets the sustain pedal as MIDI CC64 through a generic
   controller lane. None of them can show, in the editor, where the dampers actually *are* (lifted, half,
   down), or draw half-pedal as a shape aligned with the notes.
2. **Instruments are blind to the arrangement.** A Leslie can't pre-start its spin-up so the drum *arrives* at
   speed on bar 17. A tremolo can sync its rate but not its phase to a section. Nothing can say "this pedal
   change lands 30 ms before the downbeat".
3. **Two undo histories.** Arturia and Pianoteq each keep their own, separate from the host's.
4. **Unitless or relative controls.** Arturia shows 0.444; Pianoteq shows +0.20; Lounge Lizard uses 1–10. Real
   units (mm, cents, ms, dB, Hz, m) would make presets explainable and the AI bridge exact.
5. **Keyboard-wide voicing is paywalled or missing.** Pianoteq has it only in PRO; Lounge Lizard only as KEY
   knobs; the rest not at all.
6. **Non-determinism under load.** Pianoteq's overload detection skips tasks, so realtime can differ from the
   render. The samplers depend on disk streaming.
7. **Model updates change old projects silently** (the Pianoteq v7/v8 argument).
8. **Tuning is per plug-in.** Scala import happens in each instrument separately. Nobody takes the project's
   temperament and key from the host, so a Rota, a Chorda and a Firmus could share one tuning.
9. **None visualise the physics they simulate** beyond small static graphs: the live partials, the pickup
   transfer curve, the rotor state.
10. **Accessibility.** All are dark or skeuomorphic, with values that are hard to read, and none expose their
    parameters to screen readers.
11. **No CLAP listed by any of them**, and no per-note expression beyond MIDI, except Lounge Lizard's later MPE.

---

## 8. How Cantus beats them

### P0: parity foundation (without these we are not credible)

1. **Rota: a complete tonewheel organ.**
   - Upper, lower and pedal drawbars; gear-ratio tuning; leakage.
   - Seeded key click; single-trigger percussion with the 1′ steal; V1–V3 and C1–C3 scanner; preamp drive.
2. **Vertigo: a rotary speaker, both inside Rota and as an insert effect.**
   - Horn and drum with the documented speeds and spin times.
   - Doppler through modulated delay; amplitude modulation and directivity filters; drum baffle; cabinet;
     two mics; brake; Open/Closed.
3. **Lamina: a modelled electric piano with two engines.** Dens (tine + tone bar) and Lingua (reed), with a
   nonlinear pickup, damper, noises, preamp and tremolo, over 73 or 88 keys.
4. **A continuous sustain pedal and real dampers in every keys instrument.**
   - Half-pedalling, damper position and time, release velocity.
   - Sostenuto and una corda where the instrument has them.
5. **Physical units and keyboard-wide curves at rest, for everyone.**
   - Every model parameter shows a real unit.
   - Every "tracked" parameter has a KEY curve and a VEL amount.
   - Curves are edited on a keyboard-aligned display, as in Pianoteq's Note Edit, in every tier.
6. **Determinism, block-size independence and CPU budgets as tests** (section 9).
   - No task skipping. If the budget is exceeded, show a badge and offer freeze or render; never degrade
     silently.
7. **Chorda I: a modelled grand.**
   - Strings with exact stretch; nonlinear hammer; double decay; dampers with half-pedal; sympathetic and duplex
     resonance; a shared soundboard; one stereo perspective.
   - It ships after Rota and Lamina.
8. **Model versioning.** Each instrument stores `model_version` in the project. An update keeps old projects on
   the old model until the user opts in. This follows the "schema bump + fixture" habit.

### P1: host-native differentiators (only a host can do these)

1. **One undo history.** Every knob gesture, curve edit and preset load is a `cantus-core` command, with a drag
   coalesced into one step. The device's undo and redo buttons show the host history filtered to that device.
   The existing undo fuzz covers it.
2. **The arrangement's tempo and timeline drive Vertigo.**
   - **"Arrive on the beat":** a speed change placed at bar 17 starts its ramp early, using the known spin-up
     constants, so the drum reaches about 90 % of the target speed at the downbeat. The engine can see the
     automation ahead, so this is exact and deterministic.
   - A speed lane with Slow, Fast and Brake values.
   - Optional **tempo-locked rotor rates** as a creative mode, for example horn fast = one turn per 16th note.
3. **The pedal is an automation lane.**
   - A dedicated **Pedal lane** under the piano roll, drawn as a continuous shape, with the half-pedal zone
     shaded from the instrument's *actual* lift point.
   - Sostenuto and una corda lanes too.
   - Recording from a continuous pedal keeps its resolution. Quantising notes moves pedal events with them
     ("pedal follows the harmony").
   - Displayed per note: the damper state (up, half, down) as a thin band under each note.
4. **The AI bridge voices an instrument from a description.**
   - `cantus-ai` maps "a warm, intimate felt upright for a lo-fi ballad" to named parameter changes. For
     example: Chorda Size 1.3 m, hardness pp/mf/ff 20/30/45 %, una corda 0.3, hammer noise −12 dB, lid 0 %,
     close perspective.
   - Those changes are applied as **one grouped, undoable command**, and the bridge can *explain* the current
     settings back in words.
   - Real units make both directions exact. No competitor can do this, because their parameters are unitless.
5. **Project-wide tuning.** One temperament or Scala table and one A4 reference for the project. Rota's gear
   temperament and Chorda's stretch derive from it, so the organ, piano and synth agree.
6. **Host-native effects chain.** Lamina doesn't ship a pedalboard inside the instrument. Its "rig" is a
   **chain preset** of Cantus devices (Aequalis EQ, Comprimo compressor, the delay, Vertigo), so every effect
   gets the host's undo, automation and analyser.
7. **Live physics views.**
   - Chorda: the partials of the last note against their stretch targets.
   - Lamina: the pickup transfer curve with the tine's live swing drawn across it.
   - Vertigo: the rotor angles and speeds, including the ramp.
   - Rota: the scanner position.
   - All are drawn on the family display canvas; the display is the editor.
8. **CLAP export** of all four, per ADR 0001, so they are usable in other DAWs (a later round).

### P2: later

- Chorda II: movable mics (a top-down view with draggable mics); lid angle; morphing between models in the
  physics.
- A partials editor (Pianoteq PRO's "thousands of overtones"), and phantom partials.
- Historical and "impossible" pianos: a 10 m grand, glass or metal frames.
- Clavinet (with Keyscape's A/B/A+B/A−B pickups) and Pianet as Lamina engines.
- Vox and Farfisa (transistor) models for Rota.
- A Drawbar Modulator (B-3 V2's idea) as a host modulation service targeting the drawbars.
- MPE and poly aftertouch: aftertouch → tine bend or damper pressure.
- Small **original** recorded noise layers (key release, pedal thump), if synthesis does not convince in blind
  tests. They would be recorded by us, so licence-clean.

### 8.1 Build order: Rota → Lamina → Chorda

1. **Rota + Vertigo first.**
   - **Lowest risk:** tonewheels are near-pure sines, and every other behaviour is documented to the parameter
     (section 4.3). There is no "uncanny valley" in the core tone; the character lives in the Leslie and the
     overdrive, which are ordinary, testable DSP.
   - **Highest reuse:** Vertigo is an effect every track can use, and it exercises the timeline-aware
     scheduling that is our strongest P1 story.
   - **Constant cost:** it is not a voice-based instrument, so its CPU does not depend on how many keys are
     held.
   - **FL parity:** FL ships an organ preset in FL Keys and a rotary effect. This matches and beats them.
2. **Lamina second.**
   - A few modes per voice plus a static nonlinearity: moderate risk, and a large payoff, because EPs are
     everywhere in modern R&B, lo-fi, house and pop.
   - It builds the pieces Chorda needs: nonlinear hammer contact, dampers with half-pedal, keyboard-tracked
     curves, a velocity curve widget, and the pedal lane.
3. **Chorda last, in two stages.**
   - It is the hardest instrument (section 4.1). It is judged against Pianoteq, the best-engineered product in
     the category.
   - By then the pedal lane, dampers, curves, determinism tests and model versioning all exist.
   - Chorda I is P0; Chorda II is P2.
   - The timing also puts its release near or after the **2027-10-20** expiry of Modartt's patent. Even so, our
     method (waveguide or modal from closed-form physics and measurements, no offline FE eigen-analysis of a
     coupled string–soundboard) should be reviewed before release.

If the owner wants an EP first for marketing reasons, swap steps 1 and 2. The risk difference is small; the
reuse argument favours the organ.

### 8.2 Voice architecture (all three follow ADR 0005)

**Common rules**
- Parameters live in `cantus-core` as `InstrumentSpec::{Rota, Lamina, Chorda}` with `…Params` structs and
  `…Param` enums whose `info()` gives label key, range, default, unit, log and stepped. This is the
  `FirmusParam` pattern, and each is reached through `InstrumentParam::{Rota(..), Lamina(..), Chorda(..)}`.
- The DSP lives in `cantus-dsp` (MIT/Apache) and must not contain GPL code: setBfree is GPL-2 and may be read
  for documentation only.
- One schema bump and fixture per instrument (v14, v15, …), with EN/ES strings for every label.
- Pools are allocated at construction. There is no allocation in `process()`.
- Events are split at sample positions: note on and off (with release velocity), CC64, CC66, CC67, CC11,
  poly aftertouch.
- Every random element (click bounce, detune spread, humanise) comes from a **seeded PRNG keyed by
  (instrument seed, note id, event index)**, never from time.
- Denormals: flush-to-zero plus a per-voice silence cull below −100 dBFS for 50 ms.

**Rota** (not voice-based)
- 91 phase-continuous tonewheel oscillators, with an optional complex-waveform table for the lowest wheels.
- A static crosstalk matrix (sparse, about −40 dB).
- Key-contact state for 3 × 61 keys × 9 bars, and a gain per wheel per bus, recomputed at events and ramped
  over 1–5 ms.
- A pool of 64 click generators (short seeded bursts).
- Percussion: one shared single-trigger envelope.
- Per manual: a vibrato scanner, a tapped LC-line emulation with an interpolated scan.
- Swell (with the AO-28-style low-volume bass tilt), then a preamp waveshaper with ADAA, then Vertigo.
- Estimated cost: under 1,500 operations per sample, constant.

**Vertigo**
- A crossover (about 800 Hz, 4th order) splits the signal into horn and drum paths.
- **Horn path:** one modulated fractional-delay write/read per path, plus 1–2 reflection paths per mic. Each
  path has angle-indexed directivity filters (biquads interpolated by angle) and amplitude modulation.
- **Drum path:** amplitude modulation and a moving-baffle filter, plus a small Doppler.
- Rotor speed is a first-order lag toward its target, with separate accelerate and decelerate constants. The
  rotor angle is the integral of speed. **Brake** stops at a chosen angle.
- A cabinet filter, then two mics.
- Every state variable is deterministic, and the scheduling reads the timeline ahead.

**Lamina** (voice-based, 32 voices by default, 64 at most)
- Voices are stolen only in overflow, oldest released first, with a 3 ms fade.
- **Exciter:** a nonlinear hammer contact (the power-law spring) against the resonator's displacement at the
  strike point. Its hardness, force and noise follow KEY curves and VEL amounts.
- **Resonator (Dens):** the tine as 3–4 modal resonators (fundamental, about 6.27×, about 17.5×, plus a spring
  mode), coupled to a tone bar of 2–3 modes through a coupling coefficient (tine as generator, bar enslaved).
- **Resonator (Lingua):** the reed as 3–5 modes with the tip-mass shift.
- **Pickup:**
  - Dens: electromagnetic, with the output taken from the change in a displacement-dependent flux curve set
    by distance and alignment.
  - Lingua: electrostatic, where *C(x)* is set by the gap and the reed's centring and the output is
    *V·dC/dt*.
  - Both nonlinearities use ADAA, with 2× oversampling as an option.
- **Damper:** a mode-damping ramp on note-off (or pedal-up), shaped by damper time and damper tone, plus
  seeded damper and release noise. The pedal scales damper engagement continuously, which gives half-pedal.
- **Global:** a preamp (the Suitcase/Peterson or 200A curve), then tremolo (stereo pan for Dens, amplitude for
  Lingua), then an optional speaker model.
- Estimated cost: about 80–120 operations per voice per sample, so 32 voices ≈ 3–4k operations per sample.

**Chorda** (one string group per key, not a voice pool)
- **97 fixed string groups** (A0–C8 plus the extended range), each with 1–3 strings. A key re-strike hits the
  *same* string group while it is still moving, which is physically correct: no voice stealing, and no
  "retrigger" artefact.
- **Strings.** The recommendation is to **start with modal strings**: exact inharmonicity and stretch,
  per-partial decay laws, SIMD, and trivially block-size independent.
  - Partials are capped at min(Nyquist, 8–10 kHz, the quality setting). Each partial carries two slightly
    detuned resonators for double decay and beating, which covers unison width.
  - Above the cap, the attack's high band comes from a short filtered-noise "knock" excitation.
  - **Decide from a prototype benchmark:** if the lowest octave (100+ partials) breaks the budget, switch the
    bass to waveguides with a dispersion allpass of order ≤ 20 (Rauhala–Välimäki or Abel–Välimäki–Smith
    design).
- **Hammer.** A mass and power-law spring with hysteresis (Stulov), constants interpolated from the C2/C4/C6
  table and scaled by the hardness curves. It is integrated with a stable predictor or implicit step, and
  contact takes about 2 ms.
- **Bridge and soundboard.**
  - All string forces are summed into **one shared soundboard**: the multirate FIR/IIR (about 100 operations
    per sample) or an FDN, with a stereo pair of radiation filters and a lid shelf.
  - Because the soundboard is shared, its cost is fixed.
- **Sympathetic, duplex and pedal.**
  - A shared **sympathetic bank**: about 8–12 low partials for each of 88 notes, about 900–1,100 resonators
    (Zambon et al.'s scale), driven by the bridge signal.
  - Each note's damping in that bank is set by key state, sustain-pedal depth through the lift-point curve,
    sostenuto capture, and "last damper".
  - Duplex is a small fixed resonator set per register.
- **Budget target** (our estimate, to be measured): 88 notes at ff with the pedal down at 48 kHz, at the
  Standard quality setting, ≤ 25 % of one core on the reference machine. That is roughly 15–25k
  multiply-adds per sample with AVX2.

### 8.3 Parameter lists (names, ranges, defaults, units)

*Defaults marked † are starting points from the cited references. They must be calibrated by ear and by
measurement before release.*

**Rota**

| Group | Parameter | Range | Default | Unit |
|---|---|---|---|---|
| Upper | Drawbars 16′ … 1′ (9) | 0–8 (stepped) | 8 8 8 0 0 0 0 0 0 | step |
| Lower | Drawbars 16′ … 1′ (9) | 0–8 | 0 0 8 8 0 0 0 0 0 | step |
| Pedal | Drawbars 16′, 8′ | 0–8 | 8, 4 | step |
| Percussion | On | off/on | off | — |
| | Harmonic | 2nd / 3rd | 3rd | — |
| | Decay | Fast / Slow | Fast | — |
| | Fast time, Slow time (to −60 dB) | 50–3000, 200–8000 | 1000†, 4000† | ms |
| | Volume | Normal / Soft | Normal | — |
| | Soft level | −12 … 0 | −6† | dB |
| | Steals 1′ | off/on | on | — |
| | Normal volume drop on manual | −6 … 0 | −3† | dB |
| Vibrato | Mode | V1 V2 V3 C1 C2 C3 | C3 | — |
| | Upper on, Lower on | off/on | on, off | — |
| | Scanner rate | 4.0–9.0 | 7.0† | Hz |
| Tone | Key click (attack) | −60 … +6 | −6 | dB |
| | Release click | −60 … +6 | −12 | dB |
| | Leakage (crosstalk) | −80 … −20 | −40† | dB |
| | Brilliance (tilt) | −6 … +6 | 0 | dB |
| | Temperament | Gear 60 Hz / Gear 50 Hz / Equal / Project | Gear 60 Hz | — |
| | A4 | 415–466 | 440 (or the project's) | Hz |
| Amp | Swell (expression) | 0–100 | 100 | % |
| | Drive | 0–100 | 20 | % |
| Mix | Upper, Lower, Pedal level | −40 … +6 | 0, −3, −3 | dB |
| Out | Output | Vertigo / Direct | Vertigo | — |

**Vertigo** (inside Rota, or as an insert)

| Parameter | Range | Default | Unit |
|---|---|---|---|
| Speed | Stop / Slow / Fast | Slow | — |
| Horn slow, Horn fast | 20–80, 300–480 | 40, 420† | rpm |
| Drum slow, Drum fast | 20–80, 250–450 | 36, 357† | rpm |
| Horn accel / decel (time constant) | 0.05–2.0 / 0.05–3.0 | 0.16† / 0.32† | s |
| Drum accel / decel | 0.5–10 / 0.5–6 | 4.1† / 1.4† | s |
| Horn / drum balance | −12 … +12 | 0 | dB |
| Crossover | 500–1200 | 800† | Hz |
| Cabinet | Closed / Open | Closed | — |
| Mic distance | 10–200 | 60 | cm |
| Mic angle (stereo spread) | 0–180 | 120 | ° |
| Drive (amplifier) | 0–100 | 10 | % |
| Brake angle | 0–359 | 90 | ° |
| Arrive on beat | off/on | on | — |
| Tempo-lock rotors (P1 creative) | off / note value | off | — |
| Mix (as insert) | 0–100 | 100 | % |

**Lamina** (shared across engines unless noted)

| Group | Parameter | Range | Default | Unit |
|---|---|---|---|---|
| Model | Engine | Dens (tine) / Lingua (reed) | Dens | — |
| | Range | 73 keys (E1–E7) / 64 keys (A1–C7, Lingua) / 88 keys | 73 | — |
| Tuning | A4 | 430–450 | 440 | Hz |
| | Per-key spread (seeded) | 0–10 | 2 | cents |
| Hammer | Hardness | 0–100, + KEY curve, + VEL amount 0–100 % | 50, VEL 60 | % |
| | Force (level) | −24 … +6 | 0 | dB |
| | Strike position (from clamp) | 2–20 | 8 | % of length |
| | Hammer noise | −60 … 0 | −24 | dB |
| Resonator (Dens) | Tine decay at C4 | 0.5–30, + KEY | 8 | s |
| | Tone-bar coupling | 0–100 | 50 | % |
| | Bell (upper tine and tone-bar modes) | −40 … 0 | −12 | dB |
| | Bell decay | 0.02–2 | 0.25 | s |
| Resonator (Lingua) | Reed decay at C4 | 0.3–10, + KEY | 3 | s |
| | Overtones | −40 … 0 | −10 | dB |
| Pickup | Distance (gap) | 0.3–5.0 | 1.6† (Dens), 1.0† (Lingua) | mm |
| | Alignment (offset from centre) | −2.0 … +2.0 | 0.6† | mm |
| | Pickup level | −24 … +12 | 0 | dB |
| Damper | Damper time | 20–1000, + KEY | 120 | ms |
| | Damper tone (highs die faster) | 0–100 | 50 | % |
| | Damper noise, Release noise | −60 … 0 | −30, −36 | dB |
| | Pedal lift point | 0–100 | 35 | % of pedal travel |
| Preamp | Drive | 0–100 | 20 | % |
| | Bass, Treble | −12 … +12 | 0, 0 | dB |
| | Output | Direct / Speaker | Direct | — |
| Tremolo | Mode | Pan (Dens) / Amplitude (Lingua) | per engine | — |
| | Rate | 0.5–12 Hz or a synced note value | 5.5 | Hz |
| | Depth | 0–100 | 0 | % |
| Performance | Dynamics range | 20–60 | 40 (Dens), 50 (Lingua) | dB |
| | Velocity curve | the shared curve widget | linear | — |
| | Polyphony | 8–64 | 32 | voices |
| | Age | 0–100 | 20 | % |

**Chorda** (Chorda I)

| Group | Parameter | Range | Default | Unit |
|---|---|---|---|---|
| Model | Size (longest-string scale) | 1.1–10 | 2.7 (concert grand) | m |
| | Type | Grand / Upright | Grand | — |
| Tuning | A4 | 415–466 | 440 (or the project's) | Hz |
| | Temperament | Equal / Werckmeister III / Vallotti / Project (Scala) | Equal | — |
| | Stretch | 0–2 (1 = natural 2:1 matching) | 1.0 | × |
| | Inharmonicity trim | 25–400 | 100 | % of derived B |
| | Unison width | 0–10 | 0.6† | cents |
| | Unison balance | −1 … +1 | 0 | — |
| Voicing | Hardness at pp / mf / ff | 0–100 each | 35 / 50 / 70 | % |
| | Hardness curve | a keyboard curve (±50 %) | flat | % |
| | Strike point | 1/12–1/5 | 1/8 | of length |
| | Hammer knock | −60 … 0 | −20 | dB |
| | Una corda (soft) | 0–1 (pedal) | 0 | — |
| Strings | Prompt-sound decay | 25–400 | 100 | % |
| | Aftersound decay | 25–400 | 100 | % |
| Soundboard | Sustain (impedance) | 0–100 | 50 | % |
| | Brightness cutoff | 1–12 | 5† | kHz |
| | High-frequency decay slope | 0–100 | 50 | % |
| Resonance | Sympathetic | −40 … +6 | 0 | dB |
| | Duplex | −40 … +6 | −6 | dB |
| | Last damper | a note, C6–C8 | F6 | note |
| Dampers | Damping time | 10–500, + KEY | 80† | ms |
| | Damper position | 0–100 | 50 | % |
| | Pedal lift point | 0–100 | 35 | % of travel |
| | Pedal noise, Key-release noise | −60 … 0 | −30, −36 | dB |
| Output | Lid | 0–100 | 100 | % open |
| | Perspective | Player / Close / Room | Player | — |
| | Stereo width | 0–150 | 100 | % |
| | Dynamics range | 20–70 | 45 | dB |
| Quality | Partials cap | 16–128 | 64 | per string |
| | Mode | Eco / Standard / High (deterministic) | Standard | — |

### 8.4 Editor layouts

All three use the family frame from `family.md` §7:
- The header: channel chip, the device name in the device hue, undo/redo from the host history, A/B, preset
  ‹ › with a modified dot, bypass, help.
- The footer: Learn, the global chips, a latency badge, In/Out.
- Values are always visible in real units. Knobs have a 3:1 edge.
- Keyboard-wide curves are drawn on a strip aligned to the keyboard at the bottom.

**Rota**

```
┌ ▌Organ 1 │ ROTA ················· ↶ ↷ │ A│B → │ ‹ Jimmy's Blues ● › │ ⏻ ? ⋯ ⛶ ┐
│ UPPER                         LOWER                         PEDAL          │
│ ▮▮▮▯▯▯▯▯▯ 888 000 000         ▯▯▮▮▯▯▯▯▯ 008 800 000          ▮▮ 8 4       │
│ 16 5⅓ 8 4 2⅔ 2 1⅗ 1⅓ 1       16 5⅓ 8 4 2⅔ 2 1⅗ 1⅓ 1        16  8        │
│ (drawbars = vertical Bars, 0–8, number under each, drag/scroll/type)       │
├──────────────────┬───────────────────┬───────────────────┬────────────────┤
│ PERCUSSION       │ VIBRATO / CHORUS  │ TONE              │ VERTIGO        │
│ [On] [3rd|2nd]   │ [V1 V2 V3 C1 C2 C3]│ Click   −6.0 dB   │  ◐ horn 0.67 Hz│
│ [Fast|Slow]      │ Upper [on] Lower[ ]│ Leak   −40.0 dB   │  ◑ drum 0.60 Hz│
│ [Norm|Soft]      │ Rate   7.0 Hz     │ Drive    20 %     │ [Stop|Slow|Fast]│
│ Fast  1000 ms    │ ▭ scanner ●───    │ Swell   100 %     │ ramp ▁▂▅▇ 4.1 s│
│ Steal 1′ [on]    │                   │ Temper. Gear 60   │ Arrive on beat ✓│
├──────────────────┴───────────────────┴───────────────────┴────────────────┤
│ Learn · Quality Std · Tuning: project A4 440 · 0.0 ms · In −∞ · Out −3.2 dB │
└──────────────────────────────────────────────────────────────────────────┘
```

The rotor glyphs turn at the real rotor speed, or stay still under reduced motion with the speed shown as
text. The ramp mini-graph shows the next scheduled change from the timeline.

**Lamina**

```
┌ ▌Keys │ LAMINA ··· ↶ ↷ │ A│B │ ‹ Suitcase '74 ● › │ [Dens|Lingua] │ ⏻ ? ⋯ ┐
│ HAMMER            RESONATOR (tine + bar)      PICKUP                       │
│ (Hard)  50 %      (Decay) 8.0 s  (Couple) 50% ┌──────────────────────────┐ │
│  KEY ╱ VEL 60 %   (Bell) −12 dB (BellDk) .25s │  flux ╭──╮   tine swing  │ │
│ (Force)  0.0 dB   Strike 8 % of length        │ ─────╯   ╰───  ◄═══►     │ │
│ (Noise) −24 dB                                │ gap 1.6 mm · offset 0.6 mm│ │
│                                               │ (drag the pole = offset)  │ │
├─────────────── DAMPER ──────────┬── PREAMP ───┴──┬──── TREMOLO ─────────────┤
│ Time 120 ms  Tone 50 %          │ Drive 20 %     │ [Pan|Amp] Rate 5.5 Hz ♪  │
│ Noise −30 dB Release −36 dB     │ Bass 0  Treb 0 │ Depth 0 %  [sync ♩=]     │
│ Pedal lift 35 % ▁▁▁▅▇█          │ [Direct|Spkr]  │                          │
├─────────────────────────────────┴────────────────┴──────────────────────────┤
│ KEY CURVE: [Hardness▾]  ▁▂▂▃▃▄▄▅▅▆▆▇ (bars per key, anchors, interpolate)   │
│ ║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌  (live: struck keys light by velocity)│
├───────────────────────────────────────────────────────────────────────────┤
│ Learn · Voices 32 · Range 73 · Dyn 40 dB · Vel curve ▸ · 0.0 ms · Out −6 dB │
└───────────────────────────────────────────────────────────────────────────┘
```

The pickup display is the editor. It plots the flux (or capacitance) transfer curve for the current gap and
offset, with the tine's live excursion drawn over it. Dragging the pole piece sideways sets the alignment;
dragging it vertically sets the gap.

**Chorda**

```
┌ ▌Piano │ CHORDA ··· ↶ ↷ │ A│B │ ‹ Concert 2.7 m ● › │ ⏻ ? ⋯ ⛶ ┐
│ TUNING                          │ VOICING                                 │
│ A4 440 Hz · Equal · Stretch 1.0 │ Hardness pp 35 · mf 50 · ff 70 %         │
│ ┌ stretch (cents) vs key ─────┐ │ ┌ hardness vs velocity ─────┐ Strike 1/8 │
│ │+30 ╭──────╯  (Railsback,    │ │ │        ╭───●ff           │ Knock −20dB│
│ │  0 ─────╯     drag anchors) │ │ │   ●mf──╯                 │ Una corda  │
│ │−30╯                          │ │ │●pp                      │  0.00 ▭    │
│ └──────────────────────────────┘ │ └──────────────────────────┘           │
│ Unison 0.6 ct · Balance 0 · B ×100 %                                      │
├──────────────── SOUNDBOARD ─────┬──── RESONANCE ─────┬──── DAMPERS / PEDAL ─┤
│ Sustain 50 % · Bright 5.0 kHz   │ Sympathetic 0 dB   │ Sustain ▕████▌   ▏ 62 %│
│ HF slope 50 % · Lid 100 %       │ Duplex −6 dB       │  half-pedal zone ░░   │
│ Perspective [Player|Close|Room] │ Last damper F6     │ Damping 80 ms · Pos 50│
├─────────────────────────────────┴────────────────────┴──────────────────────┤
│ LAST NOTE: partials vs targets  | | || ||| |||| (cents error colour + glyph)│
│ KEY CURVE [Decay▾] ▁▁▂▂▃▃▄▅▆▇ │ ║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌║▌▌║▌ │
├────────────────────────────────────────────────────────────────────────────┤
│ Learn · Quality Standard · Partials 64 · Budget 18 % ✓ · 0.0 ms · Out −4 dB │
└────────────────────────────────────────────────────────────────────────────┘
```

The pedal meter shows the live pedal depth, with the half-pedal zone shaded from the actual lift point. The
same shading appears in the piano roll's Pedal lane.

---

## 9. Test plan hints

In the house order: null tests first, then an ADR per decision, schema bump and fixture, undo fuzz, and both
locales.

1. **Tuning accuracy with stretch.**
   - For each key, render a note, estimate *f₁* by interpolated FFT peak (or read the modal frequencies
     directly as a white-box check), and compare with the tuning table produced by the stretch algorithm.
     Tolerance: **±0.5 cent** for A1–C8, **±1 cent** below A1.
   - Octave check at stretch = 1: partial 2 of A4 equals partial 1 of A5 within ±0.3 cent. Likewise 4:2 for
     the bass octaves.
2. **Inharmonicity per note.**
   - Fit *fₙ²/n² = f₀² + f₀²B·n²* by least squares over the first 20 partials (or up to Nyquist).
   - The fitted B must match the model's B within **±5 %** for every key, and **B must rise monotonically**
     from the tenor break to C8 at the default scale.
3. **Velocity → brightness is monotonic.**
   - Take the spectral centroid of the first 100 ms at velocities 8, 16, … 127 for 12 test keys across the
     range. It must be **strictly increasing**, and the peak level non-decreasing.
   - The same for Lamina (the bark ratio rises with velocity) and Rota's click level.
4. **Block-size independence.** Render the same project at blocks of 1, 7, 64, 256, 512 and 4096 frames. The
   results must be **bit-identical** (the existing null-test harness). Include a pedal ramp, a Vertigo speed
   change and a Rota percussion retrigger inside the blocks.
5. **Determinism.**
   - Two renders with the same seed are bit-identical.
   - Realtime and offline render are bit-identical.
   - A different instrument seed changes only the seeded elements (click, spread, humanise). A test diffs the
     spectra to prove it.
6. **CPU at 88 notes with the pedal down.**
   - Chorda: all 88 keys at ff, sustain at 100 %, held 10 s, at 48 kHz with 256-frame blocks. Measure the
     realtime factor. The budget is ≤ 25 % of one core on the reference machine (a CI-tracked benchmark).
   - Also a pedal-down chromatic glissando at 16 notes per second, and a **denormal soak**: the decay tail to
     silence must not raise the per-block time by more than 10 %.
   - Lamina: 64 voices at ff. Rota: all 61 × 3 keys held (it must be flat, because its cost is constant).
7. **Half-pedal behaviour.**
   - At pedal 0 %, a released note's decay matches damper time within 10 %.
   - At 100 %, it matches the undamped decay.
   - In between, the T60 is monotonic in pedal depth.
   - Sympathetic energy on silently held keys is clearly above the damped case. Start with a threshold of
     ≥ 6 dB and tighten it after calibration.
8. **Rota specifics.**
   - The gear-ratio frequency table matches the published ratios within 0.01 cent.
   - Drawbar steps are consistent in dB.
   - Percussion is single-trigger: it does not retrigger in legato and does retrigger after all keys are
     released.
   - The 1′ steal.
   - Crosstalk measured at the chosen dB.
   - The scanner rate is visible in the modulation spectrum.
9. **Vertigo.**
   - The measured AM period matches the rpm setting (±1 %).
   - Spin-up follows the time constant (63 % at τ ±5 %).
   - The "arrive on beat" ramp reaches 90 % of the target speed within **±10 ms** of the scheduled bar line at
     any tempo.
   - Brake stops within ±2° of the chosen angle.
10. **Lamina.**
    - Pickup offset 0 gives a 2nd-harmonic to fundamental ratio above +20 dB (the octave jump). Increasing the
      offset lowers it monotonically.
    - Tremolo synced to tempo drifts less than one sample per bar over 64 bars.
11. **No clicks.** On voice steal, note-off at any phase, parameter jumps and preset loads, the largest
    sample-to-sample step stays below a threshold. Check parameter smoothing with an automation ramp render.
12. **Aliasing.** A 2 kHz note at ff through the pickup and preamp must keep aliases below −90 dB relative to
    the fundamental (FFT of the full render with a known Nyquist fold).
13. **Undo and model versioning.**
    - Every parameter is set through one command, and the undo fuzz runs over all three instruments.
    - A project saved with `model_version = 1` renders identically after a DSP change that bumped the model to
      v2: a fixture plus a golden render.

---

## 10. Sources

**Products (vendor)**
- Pianoteq 9 overview: https://www.modartt.com/pianoteq · manual:
  https://www.modartt.com/user_manual?product=pianoteq&lang=en · what's new:
  https://www.modartt.com/pianoteq_whats_new · images: https://www.modartt.com/images/pianoteq9/
- Modartt patent US 7,915,515 B2: https://patents.google.com/patent/US7915515B2/en
- Keyscape: https://www.spectrasonics.net/products/keyscape/ · library:
  https://www.spectrasonics.net/products/keyscape/library.php · reference guide:
  https://support.spectrasonics.net/manual/Keyscape/11/en/topic/getting-started (topics: system-recommendations,
  rhodes-classic-mark-i, performance, pickups, damper, sustain, feel, concepts, controls)
- Arturia: https://www.arturia.com/products/software-instruments/piano-v/overview ·
  …/stage-73-v/overview · …/wurli-v/overview · …/b-3-v/overview · store pages
  https://www.arturia.com/store/piano-v (and stage-73-v, wurli-v, b-3-v)
- AAS Lounge Lizard EP-5: https://www.applied-acoustics.com/lounge-lizard-ep-5/
- NI Noire: https://www.native-instruments.com/en/products/komplete/keys/noire/ · Vintage Organs:
  https://www.native-instruments.com/en/products/komplete/keys/vintage-organs/
- XLN Addictive Keys: https://www.xlnaudio.com/products/addictive_keys
- FL Keys manual: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/FL%20Keys.htm
- Garritan CFX: https://www.garritan.com/products/cfx-concert-grand-virtual-piano/ · comparison:
  …/compare-cfx-lite/
- Salamander Grand: https://sfzinstruments.github.io/pianos/salamander
- setBfree (GPL-2; documentation and config defaults only): https://github.com/pantherb/setBfree (README,
  cfg/default.cfg)

**Press and forums**
- SOS, Pianoteq 9 review: https://www.soundonsound.com/reviews/modartt-pianoteq-9 · news:
  https://www.soundonsound.com/news/modartt-release-pianoteq-9
- KVR news, Pianoteq 9:
  https://www.kvraudio.com/news/modartt-releases-pianoteq-9-with-enhanced-physical-modeling-and-new-instruments-65046
- Synth Anatomy, Pianoteq 9:
  https://synthanatomy.com/2025/10/modartt-pianoteq-9-the-king-of-physical-modeling-piano-plugins.html
- KVR forum, Pianoteq 9 thread: https://www.kvraudio.com/forum/viewtopic.php?t=624413&start=75
- Modartt forum, benchmarks: https://forum.modartt.com/viewtopic.php?id=4149 · Raspberry Pi 4:
  https://forum.modartt.com/viewtopic.php?id=6638 and https://forum.modartt.com/viewtopic.php?id=6843
- SOS, Keyscape: https://www.soundonsound.com/reviews/spectrasonics-keyscape · MusicTech:
  https://musictech.com/reviews/spectrasonics-keyscape-review/ · Synth Anatomy, Keyscape 1.5:
  https://synthanatomy.com/2023/03/spectrasonics-keyscape-1-5-free-update-adds-new-double-felt-grand-piano.html
- BPB, Lounge Lizard EP-5: https://bedroomproducersblog.com/2025/07/03/lounge-lizard-ep-5/ · CDM:
  https://cdm.link/lounge-lizard-ep-5/ · SOS news: https://www.soundonsound.com/news/aas-unveil-lounge-lizard-ep-5
  · Synth Anatomy:
  https://synthanatomy.com/2025/06/aas-lounge-lizard-ep-5-physical-modeling-e-piano-plugin-gets-a-major-dual-engine-upgrade.html
- SOS, Addictive Keys (2013): https://www.soundonsound.com/reviews/xln-audio-addictive-keys

**Literature**
- H. Fletcher, "Normal vibration frequencies of a stiff piano string," JASA 36 (1964). doi:10.1121/1.1918933
- A. Chaigne, A. Askenfelt, "Numerical simulations of piano strings I," JASA 95 (1994). doi:10.1121/1.408459
- A. Stulov, "Hysteretic model of the grand piano hammer felt," JASA 97 (1995). doi:10.1121/1.411912
- G. Weinreich, "Coupled piano strings," JASA 62 (1977); see also Sci. Am. 1979. doi:10.1038/scientificamerican0179-118
- J. O. Smith, S. Van Duyne, "Commuted piano synthesis," ICMC 1995; JOS, *Physical Audio Signal Processing*:
  https://ccrma.stanford.edu/~jos/pasp/Commuted_Piano_Synthesis.html and
  https://ccrma.stanford.edu/~jos/pasp/Dispersion_Filter_Design.html
- B. Bank, F. Avanzini, G. Borin, G. De Poli, F. Fontana, D. Rocchesso, "Physically informed signal-processing
  methods for piano sound synthesis: a research overview," EURASIP JASP 2003 (the source of the hammer table,
  dispersion order, beating and soundboard costs): http://home.mit.bme.hu/~bank/publist/jasp03.pdf
- J. Bensa, S. Bilbao, R. Kronland-Martinet, J. O. Smith, "The simulation of piano string vibration: from
  physical models to finite difference schemes and digital waveguides," JASA 114 (2003). doi:10.1121/1.1587146
- J. Rauhala, V. Välimäki, "Tunable dispersion filter design for piano synthesis," IEEE SPL 13 (2006).
  doi:10.1109/lsp.2006.870376
- J. Abel, V. Välimäki, J. O. Smith, "Robust, efficient design of allpass filters for dispersive string sound
  synthesis," IEEE SPL 17 (2010). doi:10.1109/lsp.2010.2040924
- B. Bank, L. Sujbert, "Generation of longitudinal vibrations in piano strings," JASA 117 (2005).
  doi:10.1121/1.1868212
- H.-M. Lehtonen, H. Penttinen, J. Rauhala, V. Välimäki, "Analysis and modeling of piano sustain-pedal
  effects," JASA 122 (2007). doi:10.1121/1.2756172
- S. Zambon, H.-M. Lehtonen, B. Bank, "Simulation of piano sustain-pedal effect by parallel second-order
  filters," DAFx-08: http://home.mit.bme.hu/~bank/publist/dafx08ped.pdf
- B. Bank, S. Zambon, F. Fontana, "A modal-based real-time piano synthesizer," IEEE TASLP 18 (2010).
  doi:10.1109/tasl.2010.2040524
- J. Chabassier, A. Chaigne, P. Joly, "Modeling and simulation of a grand piano," JASA 134 (2013).
  doi:10.1121/1.4809649
- B. Bank, J. Chabassier, "Model-based digital pianos: from physics to sound synthesis," IEEE SPM 36 (2019).
  doi:10.1109/msp.2018.2872349
- M. Muenster, F. Pfeifle, T. Weinrich, M. Keil, "Nonlinearities and self-organization in the sound production
  of the Rhodes piano," JASA 136 (2014). doi:10.1121/1.4899833
- F. Pfeifle, M. Münster, "Tone production of the Wurlitzer and Rhodes e-pianos," in *Studies in Musical
  Acoustics and Psychoacoustics*, Springer 2017 (Current Research in Systematic Musicology 4).
  doi:10.1007/978-3-319-47292-8_3
- A. Falaize, T. Hélie, "Passive simulation of the nonlinear port-Hamiltonian modeling of a Rhodes piano,"
  J. Sound Vib. 390 (2017). doi:10.1016/j.jsv.2016.11.008
- J. O. Smith, S. Serafin, J. Abel, D. Berners, "Doppler simulation and the Leslie," DAFx-02:
  https://ccrma.stanford.edu/~jos/doppler/
- J. Pekonen, T. Pihlajamäki, V. Välimäki, "Computationally efficient Hammond organ synthesis," DAFx-11
  (not re-read).
- J. Herrera, C. Hanson, J. Abel, "Discrete time emulation of the Leslie speaker," AES 127th Convention, 2009
  (not re-read).
