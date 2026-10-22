# Analog modelling and virtual-analog polysynths: competitive research for Cantus's VA instrument

*Researched 2026-09-24 for Cantus. The sources are the manufacturers' product pages, the Diva and Repro-5 PDF user
guides (both dated 7 August 2025), changelogs, the Surge XT manual, KVR user reviews, and the published DSP
literature (section 10). Everything is paraphrased. Contrast ratios were **measured from the official screenshots**
(JPEG, so ±2 per channel) with a WCAG relative-luminance script. Blocked or refused sources, which I did not work
around: the Reveal Sound knowledge base (HTTP 403/504), discodsp.com (HTTP 999), Gearspace (403), Sound On Sound
(410), and Softube's model pages (404). The session's web-search budget ran out partway through. Claims that come only
from search excerpts or forums are marked, and so are those given from memory.*

*The screenshots are the manufacturers' images. They are kept in `screenshots/` (git-ignored) for
internal reference only.*

*Licence note: OB-Xd is GPL v3 only **up to version 2.11**. TAL-U-NO-LX is **commercial and closed**. This report
describes documented behaviour and the literature, and quotes no code from either.*

---

## 0. The short version

1. **"Alive" comes from six behaviours, not from how many components are modelled:**
   1. A **nonlinear filter whose tone depends on the input level.**
   2. **Resonance that stays pitched and clean** when it sweeps, because the filter is zero-delay feedback (ZDF).
   3. **Self-oscillation that tracks the keyboard.**
   4. **Bass that survives high resonance.**
   5. **Free-running, drifting oscillators that differ slightly from voice to voice.** They are allocated round-robin,
      as on hardware.
   6. **RC-curve envelopes** and **low aliasing.**

   u-he Diva does all six, and pays for it: 2× oversampling in its cheap modes, 4× in its good ones, and an iterative
   filter solver that usually converges in 1–2 passes but can take up to 15 (Diva guide p. 6).
2. **Every serious VA exposes accuracy as a CPU dial.** Diva has Draft/Fast/Great/Divine plus a separate offline
   setting, Repro has HQ, Mercury-8 and TAL have oversampling, and Spire has HQ.
   - Diva's Great and Divine share 4× oversampling. They differ only in how precisely the filter/VCA feedback is
     solved: **16-bit versus 20-bit** (Urs Heckmann on KVR).
   - They sound identical at low resonance and differ only at high resonance and with filter FM.
3. **Voice-to-voice variance is now table stakes.** Examples:
   - Diva's Trimmers page (detune per voice and oscillator, slop on cutoff, envelopes, pulse width (PW) and glide,
     global drift);
   - Repro's Voice Detune and its "reallocate" jumper;
   - Arturia's Vintage knob and Voice Dispersion;
   - Cherry Audio's Condition display;
   - a hidden `oscdrift` setting in Sylenth1 3.072;
   - Surge's Osc Drift.

   **Nobody makes it reproducible (seeded), visible as voice cards, or independent of where playback starts.** A host
   can.
4. **A fixed architecture with a readable panel wins.** Repro, Mini V and Sylenth1 sell on it. **Diva's best idea is
   swapping models *inside* a fixed layout**: oscillator, high-pass/feedback, filter and envelope models each change in
   place. That is the model for us.
5. **Their weak spots are plug-in-wall problems and legibility.**
   - Diva **won't expose its switch parameters to host automation**, because of CPU spikes and noise bursts.
   - Its multicore mode can *slow down* Apple Silicon and fights the host's own threading.
   - Offline quality depends on the host announcing that it is rendering.
   - Users call its modulation "tedious" (KVR, 2023).
   - Replica panels measure **1.3–1.8:1** knob-to-panel contrast, and Jup-8 V's white-on-red headers measure 3.35:1.
6. **Trade dress splits the market.**
   - u-he uses generic names ("Ladder", "Cascade", "a specific piece of monophonic hardware").
   - Arturia and Cherry Audio replicate whole hardware panels.

   Cantus should name models by **topology**, show **no replica panels**, and use the family frame.
7. **Decision: a separate instrument, working name *Calor*,** built on a shared `cantus-dsp::va` library. The same
   library provides Tabula's analog filters and the bass instrument's ladder and diode filters. Firmus v1 stays
   bit-identical. Section 8 argues this and gives the parameter list, voice architecture, filter order, layout and
   tests.

---

## 1. Products and versions

| Product | Latest version and date | Price | Formats | Source |
|---|---|---|---|---|
| **u-he Diva** | **1.4.8** (rev. 16519), released **27 Aug 2024**; user guide dated 7 Aug 2025. MTS-ESP since 1.4.5, **MPE since 1.4.8**. | €179 | **CLAP**, VST3, AUv2, AAX; Win, macOS, Linux (beta) | [product page](https://u-he.com/products/diva/), [user guide](https://u-he.com/downloads/manuals/plugins/diva/Diva-user-guide.pdf) |
| **u-he Repro (Repro-1 + Repro-5)** | **1.1.3** (rev. 16797), **10 Dec 2024**. 1.1.3 doubled the modulation slots to 4, made the FX order match the signal flow, and added MPE to Repro-5. | €149 (both synths) | CLAP, VST3, AUv2, AAX | [product page](https://u-he.com/products/repro/), [Repro-5 guide](https://dl.u-he.com/manuals/plugins/repro/Repro-5-user-guide.pdf) |
| **Arturia V Collection 11** | Released **24 Apr 2025** with 7 new instruments (JUP-8000 V, Synthx V, SEM V, MiniBrute V and others); 45 instruments in total (search excerpts). **V Collection 12** is rolling out one instrument at a time, starting with **Memory V** (Memorymoog), **May 2026**, $149. | Pro $699; Intro $199 (search excerpts) | VST2, VST3, AU, AAX, standalone. **No CLAP** found (an open KVR request; excerpt only). | [VC11 press release](https://www.arturia.com/company/press/press-releases/arturia-announces-v-collection-11), [Sonicstate](https://sonicstate.com/news/2025/04/24/arturia-v-collection-11-7-new-instruments/), [Memory V review](https://synthanatomy.com/2026/05/arturia-memory-v-review-the-memorymoog-taken-to-your-daw-with-extras.html) |
| — Mini V (4th generation) | "Mini V4" in the official screenshots. The date was not re-verified. | single instruments ≈ $149 (Memory V's price) | as above | [Mini V page](https://www.arturia.com/products/software-instruments/mini-v/overview) |
| — Jup-8 V (4th generation) | "Jup-8 V4" in the screenshot title | — | as above | [Jup-8 V page](https://www.arturia.com/products/software-instruments/jup-8-v/overview) |
| — Prophet-5 V | Current. Models Rev 2 with SSM chips, and bundles Prophet-VS. | — | as above, plus NKS | [Prophet-5 V page](https://www.arturia.com/products/software-instruments/prophet-5-v/overview) |
| **LennarDigital Sylenth1** | **3.075** Windows / **3.073** macOS (downloads page, Sep 2026). **3.072 (6 Dec 2022)** added VST3, native ARM, and "oscillator drift emulation" (`oscdrift` in Settings.xml). | €139 ex VAT, or €9.95/month installments | VST2, VST3, AU, AAX. **No CLAP.** | [overview](https://www.lennardigital.com/sylenth1/), [specs](https://www.lennardigital.com/sylenth1/specs.php), [store](https://www.lennardigital.com/store/), [3.072 notes on KVR](https://www.kvraudio.com/forum/viewtopic.php?t=590403) |
| **Reveal Sound Spire** | **1.5.22** (build 5409), **17 Sep 2026**. VST3 arrived in Mar 2023; the RS "AI preset generation" service in 1.5.17 (Sep 2024). | $189 alone; $289 bundle (the page also shows $115 next to it, probably a sale price) | VST2, VST3, AU, AAX, **CLAP** (separate download) | [home/download](https://www.reveal-sound.com/), [changelog](https://www.reveal-sound.com/content/changelog.php?spire) |
| **Cherry Audio Mercury-8** (Jupiter-8 class) | 2025 (its FX are shared with Cherry releases "from October 2025 onward") | $69 | VST, VST3, AU, AAX, standalone | [product page](https://cherryaudio.com/products/mercury-8) |
| — the rest of Cherry Audio's line | Eight Voice, Mercury-4/-6, Miniverse, PS-3300, GX-80, Memorymode, DCO-106, Octave Cat, Polymode, SH-Max and others (product index) | $39–$69 typical | same | [products](https://cherryaudio.com/products) |
| **discoDSP OB-Xd 2** (open reference) | **2.20, Aug 2026** (binaries). **GPL v3 source only up to 2.11.** OB-Xd 3 is a commercial rewrite by discoDSP and the original author, 2DaT. | free | VST2, VST3, AU, AAX, LV2 | [GitHub](https://github.com/reales/OB-Xd), [original](https://github.com/2DaT/Obxd) |
| **TAL-U-NO-LX** (Juno-60 class) | **5.1.2 (3 Nov 2025)**; 5.1.3 is listed as the latest. MPE options added in 5.1.1. | not shown on the page (~$60 from memory) | VST2, VST3, AU, AAX, **CLAP**; Win, macOS, Linux | [product page](https://tal-software.com/products/tal-u-no-lx) (read through a fetch service) |
| **Surge XT** (open reference, GPL) | Manual current in Sep 2026 | free | CLAP, VST3, AU, LV2 | [manual](https://surge-synthesizer.github.io/manual-xt/) |
| Softube Model series (72/80/84) | **Not verified**: the product URLs returned 404 this round | — | — | — |

**Corrections to the brief:**
- TAL-U-NO-LX is not GPL. The GPL code from TAL is its older free synths (TAL-U-No-62, TAL-NoiseMaker); this is from
  memory and not re-verified this round.
- OB-Xd's current binaries are freeware; only versions up to 2.11 are GPL.
- Firmus's filter is not a Chamberlin SVF. It is Simper's **trapezoidal (TPT/ZDF) SVF** (`filter.rs`), which is
  already the right linear core.

---

## 2. Screenshots saved

All files are in `screenshots/`. Each was checked to be a real JPEG or PNG and was viewed. Contrast
is measured as the WCAG luminance ratio between the median panel colour and the 5th-percentile dark or
99th-percentile bright pixels in a region around the control.

| File | What it shows | Visual analysis |
|---|---|---|
| `va-01-diva-main-layout.jpg` (1000×558) | Diva, main page: Triple VCO, Mixer, VCF Ladder, two ADS envelopes; bottom row LFO 1/2, Tuning, Amplifier/Pan, voice panel (Mode legato, Accuracy great, Offline Acc same, Voices 2, Stack 1), clock/arp, Effect 1 (Plate) and Effect 2 (Delay); tabs Main / Modifications / Trimmers / Scope / Presets | **Skeuomorphic**: wood end cheeks, a Moog-like black upper panel and an oxblood lower panel, chrome-capped black knobs with printed scales. Knob skirt vs panel **1.5–1.8:1**; the chrome cap's highlight reaches **6–9:1**, and that is what carries legibility. White labels **9.8:1**. The dense lower row uses small dark "LCD" fields for mod sources, which are hard to read at 100 %. |
| `va-02-diva-trimmers.jpg` | The Trimmers page: an 8×3 grid of per-voice oscillator detune knobs with voice LEDs, 8 VoiceMap knobs, Detune Amt, Voice Drift, Variance (Cutoff/Env/PW/Glide) each with a randomise button, Reset Phase per oscillator, Transient Mode ("analog"), Stack Tune 1–6, LED colour, Bipolar Noise | **The best analog-variance UI in the category**, but it hides on a tab. Each voice card is a column; there are 8 knob columns for 16 voices (voices 9–16 reuse them). |
| `va-03-diva-filter-ladder.jpg` | VCF Ladder close-up: Cutoff (0–10) and Emphasis, 12/24 dB switch, two bipolar mod knobs with source labels (LAG, ADD), KYBD, and FM (OSC1) | Bipolar knobs mark ± with red and white ticks, and the source name sits above the knob as a clickable label. That is a good pattern. |
| `va-04-diva-modifications.jpg` | The Modifications page: VCO/Filter/Feedback mod-depth sources, and the processors Rectify, Invert, Quantize, Lag, Multiply (a × b), Add (a + b) | A mod processor is a *source* made from other sources. A red "M" marks a modulated parameter on the main page. |
| `va-05-diva-preset-browser.jpg` | Preset browser: a directory tree (Local, User, search history, banks, 7 coloured favourites), a two-column list, and a preset info pane (Usage: "MW = deep vibrato, AT = tone") | The "Usage" field, telling the player which controls do what, is worth copying. |
| `va-06-repro5-main.jpg` | Repro-5 main: Voice Mod (the poly-mod), LFO, Wheel Mod, Osc A/B, Mixer, Filter, Amp, a 2-slot Matrix, Unison, Glide, Voice Detune; the top bar has a Voices 7-segment display, a red LED preset display, Undo/Redo, Save, MCORE, HQ, Tune and Output; a keyboard with wheels | Photo-real, brown-black panel. Knob vs panel **1.3:1**; cream labels **4.7:1**; red LED text **4.5:1**. Readable but dim. |
| `va-07-repro5-tweaks.jpg` | The Tweaks page: the synth drawn as a green circuit board with jumpers (LFO DC, INV, S&H/Noise, Reallocate, FB/Noise, Key/Key+PB), model selectors (P5 Old, Poly, Analog), voice panning trimmers 1–8, pitch range on 7-segment displays, microtuning; below it the FX chain (Distortion, Velvet, Lyrebird, ResQ, Drench, Sonic Conditioner) as stomp boxes | "Eye-candy" by u-he's own description. It turns rare switches into physical metaphors. Our equivalent should be a plain **Behaviours** list. |
| `va-08-repro1-signal-flow.png` (1800×1200) | Repro-1 block diagram: waveshape generation → combination → timbre → loudness; the feedback path from the final VCA back into the mixer; trigger and gate to both envelopes | The teaching idea to keep: **the panel should read as the block diagram.** |
| `va-09-arturia-mini-v4-main.jpg` (2000×1309) | Mini V4 main: a wood-cased replica panel (Controllers, Oscillator Bank, Mixer, Modifiers, plus Vintage and Tune), keyboard, wheels; top bar (menu, name, library, favourite, preset, Advanced, cog); bottom bar (Poly 6, undo/history/redo, **CPU 5 %**, Macro 1–4) | Knob skirt **1.5:1**; labels **8.9:1**; top-bar text **14:1**; macro labels **7.4:1**. Arturia's frame is flat and modern around a skeuomorphic replica. |
| `va-10-arturia-mini-v4-advanced.jpg` | The Advanced panel slides in above the replica: Mod/Arp, Keyboard and Effects tabs; LFO/Function/Arp editors with drawn curves, destination pills and bipolar knobs | A flat dark drawer with an orange accent. Modern features stay off the replica, and that separation works. |
| `va-11-arturia-jup8v-main.jpg` | Jup-8 V: slider banks under red section headers (LFO, VCO Modulator, VCO-1/2, HPF, VCF, VCA, ENV-1/2), Dispersion 1–3/Custom, Voice Assign, arpeggiator, an LED display; the bottom bar shows Unison / Classic and CPU 12 % | White on red headers **3.35:1** (fails 4.5:1 for small text); small labels **7.1:1**. Sliders show value by position, which reads well. |
| `va-12-arturia-prophet5v-main.jpg` | Prophet-5 V: Poly-Mod, LFO, Wheel-Mod, Osc 1/2, Mixer, Filter, Amplifier, arpeggiator; the bottom bar reads Poly 5 and CPU 8 % | Knob **1.8:1**, labels **11:1**. A wood frame and red square LED switches. |
| `va-13-sylenth1-main.jpg` (908×591) | Sylenth1 v3: Osc A1/A2, Amp Env A, Filter A, Filter Control, a central LCD (arp/FX pages), Mixer with meters, Mod Env 1/2, LFO 1/2, Misc 1/2 source slots, keyboard, Mono Legato, Portamento | A dense fixed grid; brown and grey plates with wood cheeks. Knob **1.6:1**; labels **7.2:1**. The LCD holds FX and arp as pages. |
| `va-14-spire-main.jpg` (2340×1070) | Spire: OSC1–4 tabs, wave selector (HardFM), Unison (9 voices), Mix with ANA/INV/KEY toggles, Filter 1 & 2 (Infecto, LP2, PAR/LINK), FX tabs (SHP, PHS, CHR, DEL, REV), a step/LFO editor, arp step grid, X-Comp/EQ, **DRIFT** button, mode poly, voices 16 | The only **light-panel** product here. Dark labels on light grey **9.2:1**; knobs are dark on light. Its blue pixel-font "LCD" readouts measure **3.3:1**, which fails. |
| `va-15-cherry-mercury8-condition.jpg` (1400×694) | Mercury-8: a Jupiter-style replica with an orange-header slider bank; the LED display shows **Condition: VCO drift "Brand new", VCF drift "In studio", VCA drift "Road worn", ENV slop "44 years!"**; the FX chain row is at the bottom; the top bar has Tempo, Focus and Hide Extras | Black on orange **4.7:1**; display text **8.1:1**; slider body vs panel **1.3:1**. **Named condition stops** are a friendly way to present slop. "Hide Extras" shows only the original panel. |

Not kept:
- Diva's other layout skins (u-he has five layouts, all the same red and black idiom);
- Repro-1's pages;
- Mercury-8's key-mode view (redundant);
- Arturia's Vintage knob close-up (redundant with va-09).

Not obtainable: the OB-Xd and TAL-U-NO-LX UI images (discodsp.com refused; TAL's image links were broken).

---

## 3. Anatomy of each product

### 3.1 u-he Diva ("Dinosaur Impersonating Virtual Analogue")

- **Architecture.** Swappable modules in fixed slots: oscillator model → HPF/feedback model → VCF model → VCA, with two
  envelope slots whose models are independent (guide pp. 23–38).
  - The guide claims Diva is the first native synth to apply **industrial circuit-simulator methods (PSpice-like)
    in real time**.
  - Its ZDF solver predicts, measures the deviation and corrects. It usually needs 1–2 passes, and **up to 15**
    at very high resonance or with noise input (p. 6).
- **Oscillators (5 models):**
  - **Triple VCO**: continuous waveform morph from ramp through triangle, saw and square to narrow pulse; FM 1→2/3;
    sync 2 and 3 to 1; per-oscillator tune-mod and shape-mod switches; a "5-turn" detune pot that flips octaves;
    pink or white noise; post-filter **feedback** into the mixer, which gives bass boost and, higher, subharmonics
    and howl.
  - **Dual VCO**: multiple waves at once, PWM, sync, cross-mod, 1/Both/2/Split pitch-mod targets, and three
    "hardware revisions" (ideal, analog1, analog2) that change the triangle most. The waveforms change with pitch,
    as on hardware.
  - **DCO**: pulse, saw, six sub shapes (−1 or −2 octaves) and noise, summed.
  - **Dual VCO Eco**: cheap; ring mod; a quirky triangle.
  - **Digital**: Multisaw (7 saws), TriWrap, resonant noise, a feedback comb, spiky pulses, harmonics; deliberately
    aliasing, with an optional "High Quality" switch.
- **High-pass section (for any oscillator model except Triple):** feedback only; HPF Post (with a **Boost** setting
  that adds bass after the filter); HPF Pre (continuous); HPF Bite (resonant, a Sallen-Key type, the most CPU-hungry).
- **Filters (5 models):**
  - **Ladder**: 24 dB, 12 dB option, bipolar filter FM from osc 1;
  - **Cascade**: cleaner, with a Rough/Clean switch that changes the top-end resonance;
  - **Multimode**: LP4, LP2, HP, BP;
  - **Bite**: Sallen-Key, Rev1/Rev2, very level-dependent ("try 10 % oscillator level to hear the resonance");
  - **Uhbie**: 2-pole SVF crossfading LP → notch/BP → HP.

  Each has two assignable cutoff-mod knobs, key follow and FM.
- **Envelopes (3 models):**
  - **ADS**: decay doubles as release; switchable;
  - **Analogue**: ADSR;
  - **Digital**: ADSR with **Q** (a quantised, steppy output, like late-80s digital control) and **C** (an S-shaped
    curve).

  Velocity and keyboard scaling of the times. The guide notes that amp and filter envelopes in the *same* hardware
  often had different curves.
- **LFOs.** Two, with 8 shapes. Restart modes: sync (never; all voices in phase), gate, single, random. Phase, delay up
  to about 20 s, rate modulation, depth modulation, unipolar switch.
- **Modulation.**
  - Mostly **fixed routings with a source selector per knob**, plus the Modifications page (resonance mod, filter-FM
  mod, feedback mod).
  - **Modulation processors** (Rectify, Invert, Quantize, Lag, Multiply, Add) create derived sources.
  - Esoteric sources: Alternate (flip-flop per voice), Random per note, **StackIndex** (−1…+1 across a unison stack),
    **VoiceMap** (a value per voice from the Trimmers).
- **Voice modes.** Poly, mono, legato, duo (osc 2 follows the highest note), and **poly2**, which steals releasing
  voices first. Note priority last/lowest/highest. Voices 2–16; **Stack** (unison) with Stack Tune per stacked voice.
- **Glide.** Time or rate mode, **Glide2** (a per-oscillator offset) and **Range** (a u-he idea: lower values start the
  slur closer to the target, which gives sloppy intonation).
- **Trimmers.**
  - per-voice detune for each oscillator (8 columns; voices 9–16 reuse them);
  - **Variance** (random offsets to cutoff, envelope times, PW and glide, re-rolled by a button);
  - Voice Drift;
  - **Transient Mode** (analog free-running / dc reset / vcf reset / osc reset with per-oscillator reset phase);
  - Bipolar Noise.
- **FX.** Two serial stereo slots: chorus (Classic, Dramatic, Ensemble), phaser, plate, a tempo delay with wow, rotary
  with drive.
- **Global.** Accuracy (draft/fast/great/divine), OfflineAcc (same/best), Multicore, and an arpeggiator (played, up,
  down, up+dn ×2, random; serial/round/leap/repeat progressions).
- **MIDI.** CLAP note expressions and parameter modulation (CLAP brightness is fixed to Control A). Poly aftertouch.
  Multichannel MIDI. **MPE** (1.4.8, pitch bend up to ±48, CC74 via Control A). .tun files, MTS-ESP, Key Control
  (typed entry, experimental).
- **Presets.** 1,200+ factory presets. The .h2p text format is cross-platform. Tags, smart folders, 7 favourite
  colours, and a "Usage" field.
- **Known limits (documented).**
  - Switch parameters are **not exposed to host automation**: "CPU spikes, crashes, loud noise bursts".
  - Multicore can hurt older CPUs and Apple Silicon, and conflicts with host multicore.
  - Offline quality needs the host to signal offline rendering.

### 3.2 u-he Repro-1 and Repro-5

- **Repro-1** is a "component-level" model of a famous Curtis-chip monosynth with a sequencer. **Repro-5** is its
  polyphonic sibling, modelled on the famous 1978 five-voice polysynth from the same company, with up to 8 voices.
- **Voice (Repro-5).**
  - Osc A (saw/pulse, sync) and Osc B (saw/tri/pulse, Lo Freq, Kbd).
  - Mixer (Osc A, Osc B, noise; a jumper swaps noise for **feedback** from after the amp).
  - A 4-pole lowpass with cutoff, resonance, envelope amount, key amount and its own ADSR; the amp ADSR.
  - **Poly-mod**: filter envelope and Osc B → Osc A frequency, Osc A PW, filter.
  - Wheel-mod (LFO/noise mix → A and B frequency, A and B PW, filter).
  - 4 matrix slots, Unison with Glide (glide only in unison), Voice Detune, per-voice pan trimmers.
  - Audio-rate modulation of Osc A frequency, PW and cutoff.
- **Tweaks page (documented behaviour).**
  - Oscillator models **P5 / P1 / ideal / P5 Old** (P5 Old is "highly detuned"). In P5 the pulse is inverted relative
    to P1.
  - Filter models:
    - **Crispy** and **Rounded**, modelled from the two individual units u-he bought; Rounded's cutoff sits a few
      semitones lower and its resonance differs;
    - **Driven**;
    - **Poly**, which approximates the polysynth's filter.
  - Envelope models:
    - **Ideal**, and **Analog**, which has "more imperfections";
    - **High Sustain**: a hardware fault where the attack peak doesn't clear the sustain level;
    - **One Shot**;
    - **Piano 1/2**, derived from an E&MM article on the CEM3310 envelope chip, with a second, true release if the key
      is lifted during release.
  - Jumpers: LFO DC/no-DC, saw inversion, S&H vs noise for the wheel, note priority, **Reallocate** (same note → same
    voice, or round-robin), filter key vs key+pitch bend.
- **FX.** A **per-voice** distortion (Soft/Hard clip, Foldback, Corrode) before stereo stomp boxes: Velvet (Repro-5),
  Lyrebird delay/chorus, ResQ EQ/resonator, Drench plate reverb, Sonic Conditioner saturator/transient shaper.
- **CPU.** MCORE, and a global HQ switch that "doubles the requirement for little or no difference" except with
  extreme FM or pitches. Repro-1 sleeps when idle.
- **Presets.** 500+ for Repro-1 and 950+ for Repro-5.

### 3.3 Arturia (Mini V, Jup-8 V, Prophet-5 V; the TAE approach)

- **TAE ("True Analog Emulation")**, as described by Arturia (TAE page, search excerpt only):
  - **free-running oscillators computed dynamically**, not sampled or wavetable, and not restarted from phase 0 at
    each note;
  - **direct filter circuit modelling**, whose self-oscillation shows harmonics at multiples of the resonant
    frequency;
  - **soft clipping**.

  The current product pages add "component-level modeling" and "circuit decay".
- **Mini V (V4).**
  - 3 oscillators, mixer with Drive, Feedback (the headphone-into-input trick) and noise.
  - The ladder filter with **Filter Bass Compensation**, which restores the low end lost at high resonance.
  - A **Vintage knob**, a continuous "calibration from supreme precision to 40-year-old attic find".
  - Polyphony up to 6.
  - **Advanced panel**: 1 arp, 2 modulators (LFO and Function), 3 FX slots from 17 effects, 4 keyboard expression
    modes (velocity, aftertouch, key tracking, mod wheel curves, up to 3 destinations each).
  - 150+ presets.
- **Jup-8 V (V4).**
  - 2 VCOs with cross-mod, sync and 3 PWM modes; HPF plus a resonant LPF (12/24 dB); 2 ADSRs; an LFO.
  - Unison Detune.
  - **Voice Dispersion (3 settings + Custom)**, emulating "voltage variations".
  - Voice assign Solo / Unison / Poly 1 / Poly 2; the arpeggiator.
  - Advanced: 2 more complex LFOs, a modulation mixer, **2 × 32-step sequencers** (notes and modulation), keyboard
    curves, 3 FX slots (series or parallel) from 11 effects.
  - Component-level detail "from oscillator sync behaviour to the varying pan spread of each voice".
- **Prophet-5 V.**
  - Rev 2 modelled on SSM VCOs and the SSM2040 filter; poly-mod; a 3-shape LFO with quick destinations.
  - **Voice Dispersion**: "the tiny analog inconsistencies between every voice and parameter".
  - An added arpeggiator, an extra LFO and a Function Generator, 16 FX in 3 slots.
- **The Memory V example (V Collection 12, May 2026).**
  - Up to 12 voices (the original had 6), a 24 dB ladder with an added 12 dB mode and selectable **bass compensation**.
  - 3 mod slots, 4 macros, MPE, and 4 FX.
  - Criticised for thin MPE mapping in presets and for having no NKS at launch.
- **Shared frame.**
  - Top: menu, name, library, favourite, preset ‹›, Advanced, settings.
  - Bottom: voice mode, undo/history/redo, a **live CPU %**, and four macros (Brightness, Timbre, Time, Movement).
  - Resizable; in-app tutorials.

### 3.4 LennarDigital Sylenth1

- 4 "alias-free, analog-shaped" oscillators (two per part, A and B), each with **up to 8 stereo unison voices**. 16
  notes of polyphony, so 512 oscillator voices at most. The oscillators stay sharp from 0.01 Hz up to fs/2.
- 2 filter sections, **each 4 stages with nonlinear saturation**. LP, BP and HP at 12 or 24 dB. Drive per stage;
  resonance "way beyond self-oscillation".
- 4 ADSRs (2 amp, 2 mod), 2 LFOs with 11 waves, 16 mod slots (24 sources and destinations).
- FX: arpeggiator/step sequencer, distortion (4× oversampled), phaser (2×), chorus/flanger (2×), EQ, delay, reverb,
  compressor. SSE-optimised; unused parts switch themselves off.
- 2,500+ presets, 7 skins.
- **Missing, and asked for for 15 years (KVR users):** PWM, sync and FM.
- Drift was added in 3.072, but **only as a hidden XML setting**.

### 3.5 Reveal Sound Spire

- 4 "polymorphing" oscillators with modes Classic, Noise, FM, AMSync, SawPWM, HardFM and Vowel. **9 unison voices per
  oscillator**, spread by chords or octaves (supersaw/hypersaw-type stacks).
- 2 multimode filters named Perfecto, Infecto, Acido, Scorpio, Combo and Shaper. Their hardware lineage is not
  documented in anything I could read: the knowledge base refused automated reading.
- FX: shaper/decimator, phaser/vowel, chorus/flanger, delay, reverb, an X-Comp multiband compressor, and EQ.
- 4 macros, 4 envelopes, 4 LFOs, 15 matrix slots (2 sources and 4 targets each), 2 steppers, an arpeggiator.
- The UI has **ANA** (analog) and **DRIFT** buttons. Their exact behaviour is undocumented here (not verified).
- 1,000+ presets. The business now centres on preset packs and an "AI preset generation" service (1.5.17, 2024).

### 3.6 Cherry Audio Mercury-8 (representative of the Cherry line)

- A "circuit and behaviour modelled" JP-8 with a **dual layer**: 16 voices per layer, split or stacked.
- 2 VCOs with "accurate exponential FM cross-mod" and sync; HPF plus a resonant LPF (12/24 dB); 2 ADSRs; an LFO per
  layer.
- Assign modes Solo, Unison, Poly-1, Poly-2, Multi and Chord.
- **Analog Drift and Condition** controls (VCO, VCF and VCA drift, envelope slop) from "pristine new to unstable
  vintage".
- **Multi-voice "mini synth trimmers"**: per-voice pitch, pan, filter and envelope offsets.
- A 4-slot matrix (25 sources, 43 destinations, a "Via" control), an arpeggiator with Chance and Feel, and a 16×4
  polyphonic step sequencer.
- 3 FX chains from 20 effects, with a per-effect modulator.
- **Imports and exports original JP-8 patch SysEx.**
- **User-adjustable oversampling.** "Focus" zoom, resize by dragging, and "Hide Extras".

### 3.7 OB-Xd 2 (open reference: OB-X/OB-Xa class)

- 8 voices "with per-voice parameter control", "micro random detuning", saw/square/triangle/noise, 2 ADSRs, an LFO, and
  a mod matrix (README).
- The filter is its signature:
  - a **12 dB mode that blends continuously HP → notch/BP → LP**;
  - a **24 dB mode that morphs from 4 poles to 1**.

  Surge XT ported these filters with discoDSP's permission. The Surge manual describes a "Pushed" subtype whose
  boosted nonlinearities drive the filter harder into self-oscillation, and pole taps at 6/12/18/24 dB.

### 3.8 TAL-U-NO-LX (Juno-60 class)

- The product page says every oscillator, filter and envelope of the Juno-60 was "painstakingly modelled".
- An arpeggiator (as played, random, round-robin), delay and reverb, and the famous chorus (modes not listed on the
  page I could read).
- A "Service Control" dialog for voice detune and "vintage noise"; oversampling; MTS microtuning; 14-bit CC; MPE with
  compatibility options (5.1.1).
- The Juno architecture itself (a well-known hardware fact, not from TAL's page): one DCO per voice with saw, pulse/PWM,
  a sub and noise mixed together; a non-resonant 4-position HPF; a 4-pole OTA lowpass; one ADSR shared by VCF and VCA;
  one LFO; a BBD chorus.

### 3.9 Surge XT (open reference, the VA parts)

- **Osc Drift** per scene, with an option to **randomise the initial drift phase**.
- Filters:
  - **Legacy Ladder** (stable self-oscillation, cheap);
  - **Vintage Ladder**: Type 1 integrates a Moog ODE with Runge–Kutta; Type 2 follows Stilson–Smith and Huovilainen;
    both come in **gain-compensated** variants;
  - **K35** (MS-20-inspired, from Odin 2) with 5 saturation levels;
  - **Diode Ladder** (from Odin 2, pole taps, won't self-oscillate without feedback);
  - **OB-Xd 12/24**;
  - **Cutoff Warp / Resonance Warp** (nonlinear biquads after Chowdhury, DAFx 2020);
  - **Tri-Pole** (after Ian Fritz's Threeler).

  Surge credits a public repository of Moog-filter models as its reference. It is the best open catalogue of *which*
  VA filters matter.

---

## 4. The DSP

### 4.1 What makes it sound expensive (and what Firmus lacks today)

| Behaviour | Why it matters | Who does it | Firmus v1 today |
|---|---|---|---|
| **Level-dependent nonlinear filter** | The tone changes with oscillator level, drive and resonance, so the mixer becomes a timbre control. Diva's guide says oscillator levels "significantly" change the filter's tone. | Diva (all models), Repro "Driven", Sylenth1 (per-stage saturation), Arturia TAE, OB-Xd "Pushed" | Linear SVF |
| **ZDF resonance** | With no unit delay in the loop, the resonance frequency matches the cutoff and doesn't smear at high Q (Diva guide p. 6; Huovilainen §5.2 shows the delay's phase error) | Diva, u-he, TAL, Surge | Yes (TPT SVF) |
| **Self-oscillation that tracks** | A resonant filter becomes a playable sine-like voice; its pitch follows the keys | all the ladders | No: k ≥ 0.1, so Q ≤ 10 |
| **Bass kept at high resonance** | A 4-pole feedback filter loses 14 dB of bass at the threshold (4.2) | Arturia's **Bass Compensation**, Diva's HPF Boost and feedback, Surge's "compensated" types | SVF LP has no loss, but no resonance either |
| **Free-running, drifting oscillators** | Each note starts at a different phase, so stacked saws never "phase-slap" identically | TAE "free-running", Diva transient "analog", Surge drift | Phase reset to 0 on every note |
| **Per-voice variance and round-robin** | Chords breathe because each voice card is slightly different, and repeated notes rotate cards | Diva Trimmers, Repro Reallocate, Dispersion, Condition | Identical voices; lowest free voice first |
| **RC envelopes, retriggered from the current level** | Snappy attacks, and no clicks when a note is re-struck or stolen | Diva's 3 models, Repro's 6, Mercury-8 slop | RC-style curves already; but **resets to 0** on note-on, so a steal clicks |
| **Low aliasing** | High notes, sync sweeps and FM stay harmonic | all (BLEP-type oscillators + 2–4× OS) | 2-point PolyBLEP at 1× |
| **Smooth modulation** | Log-domain cutoff, per-sample modulation, audio-rate filter FM | Diva (filter FM), Repro (audio-rate voice mod) | Per-sample log cutoff, with `tan` every sample |
| **Waveform imperfection** | Finite saw retrace, AC coupling (pulse tilt), triangle rounding that changes with pitch | Diva Dual VCO revisions; Repro P5/P1/P5 Old | Ideal shapes |

### 4.2 Filters

**The core technique is TPT/ZDF** (Zavalishin, *The Art of VA Filter Design*, rev. 2.1.2).
- Each integrator is trapezoidal, with the cutoff pre-warped (g = tan(π·fc/fs)), and it is written in
  "instantaneous response" form: y = G·x + S.
- A feedback loop is then **solved** for the current sample instead of being delayed by one.
- Urs Heckmann's short explanation (silvrback, 2016) is the plainest:
  - in the linear case the implicit equation is rearranged algebraically;
  - in the nonlinear case it is solved iteratively (bisection or Newton–Raphson) until the residual is about 1e-6 of
    full scale.

**The linear 4-pole ladder or OTA cascade.** Four equal one-pole sections with negative feedback k:

  H(s) = 1 / ((1 + s/ωc)⁴ + k)

- **Self-oscillation at k = 4, exactly at ωc.** Each pole gives −45° there, so four give −180°, and |1/(1+j)|⁴ = 1/4.
- With the pre-warped TPT form this holds up to Nyquist. A naive design with a delay in the feedback loop detunes and
  loses resonance (Huovilainen §5.2; Stilson & Smith 1996).
- **Bass loss:** the DC gain is 1/(1 + k), which is **−14 dB at k = 4**. This is why resonant ladder and CEM/SSM-type
  cascades go thin.
  - Compensation adds c·k·x to the input. c = 1 restores the DC gain fully but makes the peak louder, so a 0–100 %
    "Bass comp" control is the honest exposure. Arturia's Mini V4 and Memory V ship exactly this.
- **By contrast, a 2-pole SVF's lowpass has DC gain 1 at any Q.** SEM-type, Uhbie-type and OB-type filters therefore
  keep their bass. This is a real character difference to preserve, not a bug to fix.

**Nonlinear ladder, from cheap to expensive:**
1. **Huovilainen (DAFx 2004).** A per-stage tanh derived from the transistor differential pair, Euler integration,
   **oversampling required**, five tanh evaluations per sample, and a tuning correction for the unit delay in the
   feedback. Well known and cheap, but not zero-delay.
2. **"Cheap" non-linear ZDF** (Teemu Voipio, KVR "Cheap non-linear zero-delay filters").
   - Linearise each nonlinearity around its operating point from the previous sample, then solve the linear ZDF
     system once.
   - One tanh per stage, no iteration. It is accurate enough at 2× or more.
3. **Newton–Raphson on the whole loop** (Zavalishin ch. 6; this is Diva's approach). Iterate to a tolerance.
   - Diva's quality modes are this dial: 16-bit tolerance at *great*, 20-bit at *divine*, and "not zero delay" at
     *draft*.
   - The cost is unbounded unless the iterations are capped (Diva: up to 15).
4. **The D'Angelo & Välimäki explicit delay-free-loop method** (ICASSP 2013; *IEEE/ACM TASLP* 22(12), 2014, parts
   I/II). Accurate and **non-iterative**, so the CPU cost is predictable. That makes it attractive for real-time
   determinism.

**Recommendation.** Ship (2) as *Good* and (3) capped at 8 iterations as *Best*. Keep (4) as a research option. Use
**our own** rational tanh and exp approximations, not libm (4.8).

**The other topologies we need** (the chip and hardware lineage is well-known public engineering, for internal docs
only; the UI uses topology names):

| Model (Cantus name) | Topology | Hardware family (docs only) | Character notes | Literature |
|---|---|---|---|---|
| **Ladder** | 4 transistor stages, global feedback, tanh at each differential pair, taps at 6/12/18/24 dB | transistor ladder monos and polys | Warm, "creamy"; bass loss; clean self-oscillation | Huovilainen 2004; Stinchcombe 2008; D'Angelo–Välimäki 2013/14 |
| **Cascade** | 4 OTA integrators (tanh at each OTA input), VCA feedback, 12/24 dB | CEM3320, SSM2040 and IR3109-type polysynth filters | Brighter and "tighter" than the ladder; same bass loss; resonance a little less smooth | Zavalishin (OTA ladder); Pirkle 2021 |
| **State-variable** | 2-pole SVF, LP→notch/BP→HP morph, soft nonlinearity in the integrators or feedback | SEM, OB-class | Full bass at high Q; smooth; blendable modes | Simper 2013; Zavalishin ch. 4 |
| **Diode** | Coupled diode ladder (the stages are not buffered, so they interact) | acid bass box, some European synths | Rubbery, squelchy, resonance interacts with level; unequal poles in the bass-box variant (bass.md) | Zavalishin (diode ladder); Pirkle 2021 |
| **Sallen-Key** | 2-pole SK with nonlinear (diode) limiting in the feedback | MS-20 class (Korg35 / OTA revisions) | Aggressive "screaming" resonance; strongly level-dependent | Stinchcombe, *A Study of the Korg MS10 & MS20 Filters* (2006); Zavalishin |

**Filter FM** means an oscillator driving the cutoff at audio rate (Diva's "FM (OSC1)"; Repro's audio-rate voice mod).
- The TPT structures handle a per-sample cutoff without energy blow-ups.
- The cost is **g = tan(·) every sample**: use a polynomial or rational `tan` (≤1e-5 relative error up to 0.49·fs_os).
- This is the setting where Diva's Great and Divine differ audibly.

**The Minimoog-style feedback loop** (post-VCA back into the mixer; Diva, Repro and Mini V all have it) is a
delay-free loop *across modules*. At 2× or 4× a one-sample delay in that path is 10 or 5 µs, which is inaudible.
Document it; don't solve it jointly.

### 4.3 Oscillators and antialiasing

**The methods, in order of cost:**
- **BLIT** (Stilson & Smith, ICMC 1996);
- **minBLEP** (Brandt, ICMC 2001, which also solves **hard sync without aliasing**);
- **PolyBLEP** (Välimäki & Huovilainen, *IEEE SPM* 24(2), 2007): 2-point, cheap, and what Firmus uses;
- **DPW** (Välimäki, *IEEE SPL* 12(3), 2005; higher orders in Välimäki, Nam, Smith & Abel, *TASLP* 18(4), 2010);
- **PolyBLAMP** for slope discontinuities: triangles, synced triangles, clipped signals (Esqueda, Välimäki & Bilbao,
  DAFx-16).

**What each case needs:**
- **Saw and pulse:** PolyBLEP at 2× is enough for a Good quality level; a 4-point BLEP or minBLEP table for Best.
- **Triangle:** generate it **directly with PolyBLAMP** at the corners. Firmus's leaky-integrated square (leak 0.999)
  changes shape and level at low frequencies.
- **Hard sync:** at the master's reset, compute the **fractional** position, then apply a BLEP of the slave's value
  jump plus a BLAMP of its slope change.
- **Cross-mod / exponential FM:** the phase increment varies every sample. BLEP still works if dt is taken per
  sample; the residual error is covered by the oversampling.
- **Pulse-width modulation:** two BLEPs per period with the current PW; clamp PW to [dt, 1 − dt].
- **The wavetable report's plan applies here too:** run the whole voice at 2× or 4×, **sum the voices, then decimate
  once per instance**. Decimation is linear, so summing first is exact. Comprimo's half-band decimator (63 + 19 taps,
  whole-sample latency) can be reused as it is.
- **Analog waveform details** worth modelling, as parameters of a "shape revision":
  - a saw with a finite retrace (a short linear fall rather than an ideal step, which also lowers aliasing);
  - an **AC-coupled** output (a high-pass at a few Hz, which tilts the tops of pulses);
  - a triangle made from the saw by a slightly soft waveshaper (the "sharkfin" and rounded peaks);
  - small DC offsets that differ between voice cards.

  Diva explicitly notes that its waveforms differ at different pitches, as on hardware.
- **Phase:**
  - **free-running**, the analog default;
  - **reset** (per-oscillator phases, for consistent attacks; Diva's "osc reset");
  - **DC reset** (Diva: removes the DC component at note start, the "punchy vs clicky" difference).

  Free-running must still be deterministic; see 4.4.

**Oversampling factors in the field:**

| Product | Setting | Factor |
|---|---|---|
| Diva | Draft, Fast | ≈2× ("typically") |
| Diva | Great, Divine | 4× |
| Repro | HQ | "doubles" the CPU |
| Sylenth1 | distortion | 4× |
| Sylenth1 | phaser, chorus | 2× |
| Mercury-8, TAL | oversampling | user-adjustable (factors not stated) |
| Huovilainen's ladder | — | requires oversampling (the paper uses 2×) |

### 4.4 Drift, slop and per-voice variance

**A model that covers what the products expose.** All of it is scaled by one "Condition" macro, and each part can be
overridden.
1. **Static card offsets** (calibration error), per voice card: oscillator pitch in cents for each oscillator (Diva's
   8×3 grid), cutoff in semitones, envelope time scale, PW offset, glide-time scale, pan, and a small DC offset.
2. **Slow drift:** band-limited random motion of pitch (and, more weakly, cutoff), 0.02–2 Hz, a few cents.
   - Build it from two or three one-pole-filtered noise sources for a 1/f-like spectrum.
   - Make it **partly common** to all voices (temperature) and partly per card.
3. **Key-tracking error:** a small V/oct scale error per card, in cents per octave from middle C. This is what makes
   wide unison and octave doubles beat differently across the keyboard.
4. **Note-on variation:** a random initial drift phase (Surge) and a free-running oscillator phase.
5. **Noise floor:** an inaudible (≈ −100 dBFS) seeded noise injected at the filter input. A self-oscillating filter
   then starts on its own, and nothing is ever perfectly static.

**Determinism, the part nobody else offers:**
- Use a **counter-based RNG**: hash(instance seed, card index, stream sample index / 16).
- Drift is then a function of **absolute time since transport zero**. It does not depend on block size, on seeks, or on
  how many notes were played before.
- A sequenced note's **card and phase can come from a hash of its stable note ID** (Firmus already carries note tags).
  Playing bar 9 alone then sounds exactly like bar 9 in a full render.
- Live input falls back to runtime rotation.

### 4.5 Envelopes

- **An analog ADSR** charges and discharges a capacitor toward targets:
  - the attack aims *above* the peak (the overshoot trick Firmus already uses, `ATTACK_RATIO = 0.3`), which gives a
    convex attack;
  - decay and release are exponential toward the sustain level or zero.
- **Model choices worth offering:**
  - **RC** (the analog default);
  - **ADS** (decay doubles as release; Diva);
  - **Digital** (linear attack, optional stepped output and S-curve; Diva's Q/C);
  - **One-shot** (Repro).

  Offer velocity scaling of the level and **keyboard scaling of the times** (Diva KYBD).
- **Retrigger behaviour is a character choice:**
  - hardware envelopes restart the attack **from the current level**;
  - "reset" (Firmus v1) jumps to 0 and clicks;
  - legato doesn't retrigger.
- **A stolen voice** must fade over 1–3 ms (declick) or continue from its current level. It must never jump.

### 4.6 Performance: allocation, modes, glide, unison and MPE

**Allocation:**
- **Rotate** (round-robin to the next free card: hardware cyclic assignment; Repro "Reallocate off"; Diva's LEDs
  show it);
- **Reuse** (the same pitch gets the same card: Repro "Reallocate on");
- **Lowest free** (what Firmus does).

**Stealing:**
- **oldest releasing first** (Diva poly2, Firmus);
- else the oldest held;
- else the quietest.

**Modes:**
- **Poly**;
- **Mono / Legato**, with last, low or high note priority (the documented US vs Japanese conventions in Diva and
  Repro);
- **Unison** (stack N voices per note, with detune and pan spread and StackIndex as a mod source; poly-unison allowed,
  like Diva's Stack);
- Duo (P2).

**Glide:**
- **time vs rate** mode (Diva);
- **RC (exponential lag of the pitch CV)** vs linear-in-pitch. RC is what analog monos do; bass.md covers the 303
  slide;
- a per-oscillator offset (Glide2);
- poly glide from the *card's previous note* (hardware), or from the *last played note*.

**MPE:**
- per-voice pitch bend up to ±48 semitones, pressure, and timbre (CC74);
- freeze a note's expression when it is released, so its channel can be reused while the tail is still sounding (Diva
  guide);
- CLAP note expressions (volume, pan, tuning, brightness) map onto the same per-voice inputs.

Diva added MPE only in 2024, and Memory V is criticised for weak MPE mapping in its presets. Build per-voice
expression into the engine from day one.

**Microtuning:** .tun, Scala .scl/.kbm and MTS-ESP appear everywhere (Diva, Repro, TAL, Mercury-8). In Cantus the
**project tuning** should feed every instrument (P1).

### 4.7 CPU at each accuracy level

**Documented:**
- Diva: Draft uses no ZDF and ~2×; Fast uses low-accuracy ZDF and ~2×; Great and Divine use 4× with 16- or 20-bit
  feedback precision.
  - Typically 1–2 solver passes, up to 15.
  - u-he: "rather demanding in divine mode", and Divine is "seldom necessary". Urs: any 4-core i7 makes it a
    non-issue.
- Repro: HQ doubles the CPU.
- Sylenth1: SSE-optimised, idle parts switched off.
- Arturia shows a live CPU % in every instrument (5–12 % in the official screenshots, for one instance).

**Users (forum-grade):**
- Diva: "instant 100 % CPU … 3 instances" (KVR review, 2022).
- Sylenth1: "10 instances … 50 %" (KVR review).
- The musicproductionwiki review (June 2026) scores Diva's CPU efficiency at 6.4/10. It also misnames the accuracy modes,
  so treat it as low-grade.

**Cantus estimates (mine, to be replaced by the benchmark in 9.6).** Per voice, at 48 kHz, one modern core at about 4
GHz. Three PolyBLEP oscillators cost ≈15 ns per sample at 1×, the linear ladder ≈8 ns, and one nonlinear iteration
with 5 fast tanh ≈20 ns.

| Quality | What runs | ≈ns per base sample per voice | ≈% of a core per voice | 16 voices |
|---|---|---|---|---|
| **Draft** | 1×, linear ZDF filter, static output saturation | 35 | 0.17 % | ≈3 % |
| **Good** (default) | 2×, cheap non-linear ZDF plus 1 Newton step | 130 | 0.6 % | ≈10 % |
| **Best** | 4×, Newton to 1e-6 (mean ≈3 iterations, cap 8) | 340 (worst ≈900) | 1.6 % (worst ≈4 %) | ≈26 % |

Unison multiplies the cost. At 16 voices × stack 2 × Best, one instance needs about half a core, which is why the
footer shows a **per-voice CPU badge** (8.4).

### 4.8 What is hard

- **Bounded cost with iterative solvers.** Cap the iterations and log a histogram. Noise at maximum resonance is the
  worst case (Diva: 15 passes).
- **Cross-platform bit-identity.** `f32::tan`, `exp2` and `powf` can come from the system libm and differ between
  operating systems. For renders that null across machines, use our own polynomial or rational `tan`, `tanh` and
  `exp2`, and a fixed evaluation order. Also: no fused multiply-add unless forced on every target.
- **Hard sync, cross-mod and filter FM together.** Each is fine alone. Combined, they need the oversampling.
- **Low cutoffs in f32.** Integrator states near 20 Hz at 4× lose precision. Keep a scalar reference in f64 and allow a
  SIMD f32 path only if it passes the same tests. The bass report already asks for f64 filter state.
- **Automating switches without clicks.** This is why Diva forbids it (section 8, P1.2).
- **Seek invariance with round-robin allocation.** Solved by allocating cards by note-ID hash (4.4).
- **Honest defaults.**
  - Slop must never be on by surprise in a *mix* preset. Diva's own guide (Troubleshooting, "Out of tune?") tells users to turn down Detune Amt and Voice
    Drift "for good tuning".
  - The Condition default should be modest (25 %). An "Init" patch should be *calibrated* (0 %).

### 4.9 Firmus v1 against this bar (from the code)

| Area | Firmus v1 (`firmus.rs`, `osc.rs`, `filter.rs`, `env.rs`) | Gap |
|---|---|---|
| Oscillators | 2 × PolyBLEP (2-point), `f32` phase; triangle is a leaky-integrated square; phase reset to 0 on every note | No sync, FM, PWM, sub, noise, drift or free-running phase |
| Filter | Simper TPT SVF, LP only; k from √2 down to 0.1 (Q ≤ 10); cutoff clamped to 0.45·fs; `tan` per sample per voice | No self-oscillation, nonlinearity, ladder, drive or bass compensation |
| Envelopes | 2 × ADSR with RC overshoot curves; reset to 0 on note-on | Clicks on steal and retrigger; one model only; no velocity or key scaling of the times |
| Voices | 16; free → oldest releasing → oldest | No mono, legato, glide, unison, allocation modes or declick |
| Modulation | Filter-envelope amount, key track | No LFO, wheel, pitch bend, velocity routing or matrix |
| Quality | 1×; deterministic; block-size independent (a smoother snaps when idle) | No oversampling or quality levels |

**Keep:** determinism, the TPT core, the RC envelope maths, the no-allocation voice pool, note-tag release.

---

## 5. The interface

### 5.1 Per product

- **Diva.**
  - Layout: five layout skins of one idiom; a top bar (Save, logo, MIDI activity, the preset display with ‹ › and a
    parameter readout while dragging, undo/redo, Multicore, Output, u-he badge, cog in the wood cheek); modules in
    the upper half, LFO/Tuning/Amp/Voice/Arp/FX in the lower; tabs at the bottom.
  - Resizable **70–200 %**; skins in the data folder.
  - Praised as "slick" and "accessible" (MusicRadar quote on the product page) and "simple, logical, intuitive" (KVR
    users).
  - Criticised as looking its age, with tedious modulation and a messy, limited matrix (KVR 2023).
  - Legibility: labels 9.8:1, but knob skirts 1.5–1.8:1 and the red lower panel's small dark fields at about 1.7:1.
  - Hidden state: Trimmers, Modifications and the Scope are tabs; only the "M" marker hints at modulation.
- **Repro.**
  - The most photo-real: a keyboard, a 7-segment voice count, a red LED preset display, the Tweaks circuit board,
    stomp boxes you can drag to reorder.
  - Labels about 4.7:1 and knobs about 1.3:1: charming, and dim.
- **Arturia.**
  - A **modern flat frame around a replica**. The Advanced drawer pushes the replica down, rather than hiding features
    in tiny replica switches. Macros are always in the bottom bar. A live CPU % is shown.
  - The Jup-8 V's white-on-red section headers measure 3.35:1. Replica knobs are about 1.5–1.8:1, but the labels are
    8.9–11:1.
- **Sylenth1.**
  - One dense fixed screen; the central LCD pages hold FX and arp; skins.
  - Loved for speed ("clean", "fast"); called dated by KVR users from 2007 to 2021.
- **Spire.**
  - A light grey panel with dark knobs: the most legible body (dark on light, about 9:1).
  - But its blue pixel-font readouts are 3.3:1.
  - Reviewers ask for a visual overhaul (Mar 2026) and more size options (2017). Its envelope graphics don't update
    with the parameters (2020).
- **Mercury-8.**
  - The replica plus a modern top bar (preset name with an **EDITED** badge, undo/redo, zoom, tempo, **Focus**, **Hide
    Extras**) and a persistent FX tray.
  - The LED display is a clickable menu for condition, drift, voices and priorities. That is a clever re-use of a
    replica element, but it is modal.

### 5.2 Contrast summary (measured)

| Product | Knob body vs panel | Labels | Worst text |
|---|---|---|---|
| Diva | 1.5–1.8:1 (cap highlight 6–9:1) | 9.8:1 | small LCD fields ≈1.7:1 |
| Repro-5 | 1.3:1 | 4.7:1 | LED display 4.5:1 |
| Mini V4 | 1.5:1 | 8.9:1 | macro labels 7.4:1 |
| Jup-8 V | sliders 1.9:1 | 7.1:1 | headers 3.35:1 |
| Prophet-5 V | 1.8:1 | 11:1 | — |
| Sylenth1 | 1.6:1 | 7.2:1 | — |
| Spire | dark on light (≈9:1) | 9.2:1 | blue readouts 3.3:1 |
| Mercury-8 | 1.3:1 | display 8.1:1 | headers 4.7:1 (black on orange) |

The replica knobs all fail the 3:1 non-text rule, as FabFilter's do (1.1–1.2:1, family.md). They stay readable only
through a bright **pointer or cap**. In a flat Cantus knob, the **value arc and the pointer tick** have to carry that
job, with a ≥3:1 edge.

### 5.3 What to take

- Diva's **model swap in place**.
- Diva's **per-voice grid with LEDs**.
- Diva's **Usage** preset field.
- Arturia's **drawer, which keeps extras off the main panel**, and its **always-visible macros and CPU**.
- Cherry's **named condition stops** and **Hide Extras**.
- Repro's **block diagram as panel**.
- Spire's **light panel legibility**.
- Jup-8 V's **sliders for envelopes**: position reads at a glance.

---

## 6. What reviewers and users say

*Caveat: KVR and forum reviews are self-selected, and many are years old. They are marked with dates where known.
Magazine quotes come from the manufacturers' own pages, which choose them.*

- **Diva.**
  - Praise: "fattest, thickest, most out-of-the-box analog"; the mix-and-match modules; the presets; a "simple,
    logical" UI (KVR).
  - Magazines on u-he's page: "sounds and handles like a luxury instrument" (Resident Advisor 2011); "Startlingly
    good" (SOS 2012).
  - **CPU dominates the complaints from 2012 to 2023**: "uses the MOST CPU of any plugin", "3 instances → 100 %".
  - 2023: modulation "FAR more tedious" than in other synths; FX not good enough.
- **Sylenth1.** 4.53/5 from 30 KVR reviews.
  - Praise: analog sound, **tiny CPU**, fast UI, presets.
  - Complaints: **no PWM, sync or FM** (KVR 2022: "in 15 years"), a dated GUI, slow development.
- **Spire.**
  - Praise: sound, filters, "hit-ready" presets.
  - Complaints: the UI needs an overhaul (Mar 2026); envelope graphics don't follow the parameters; limited sizes;
    early CPU problems (fixed by 2016); no layers or sample oscillators (2022).
- **Arturia.**
  - Quotes on the product pages praise the accuracy and the new features, and one mentions a "very low CPU load".
  - Memory V (2026 review): "absolutely convincing"; MPE mapped in too few presets; no NKS at launch.
  - I found no user-review corpus I could read this round.
- **Mercury-8.** "Could easily be the next-gen Jupiter in software" (MusicRadar, 5 stars, quoted by Cherry Audio).
- **Repro.** Awards on u-he's page; the Tweaks page is admitted by u-he itself to be eye-candy.

**The pattern:** users forgive CPU for sound (Diva), and forgive missing features for speed and sound (Sylenth1). They
do **not** forgive a dated, hard-to-read UI for long (Sylenth1, Spire).

---

## 7. Gaps none of them fills

1. **Reproducible analog.** No product seeds its drift and slop, so no render can be repeated bit for bit. None offers
   "same sound wherever playback starts" or "re-roll this voice card".
2. **Switch automation.** Diva refuses it outright. The others automate switches, but with no click-free crossfade
   and no lookahead.
3. **Variance you can see.** Diva's Trimmers are a hidden tab. Arturia's Dispersion is a 3-position switch. Cherry's is
   a modal display. Nobody shows **live per-voice cards** (which card played which note and how it is trimmed) on the
   main view.
4. **Honest units.** Diva's cutoff runs 0–10 and its variance has no units. Arturia and Cherry scale everything 0–10.
   Nobody shows cutoff in Hz and notes, resonance with a marked self-oscillation threshold, drift in cents, or card
   offsets in cents and semitones.
5. **One undo, A/B and preset library shared with the project.** Every product has its own history and its own preset
   silo.
6. **Modulation that leaves the plug-in.** A synth's LFO can't reach the mixer, and the mixer's sidechain can't reach
   the synth, without routing.
7. **Knowing the render is offline.** Diva needs the host to say so. Some hosts don't, and then Diva renders at
   realtime quality.
8. **Threading.** Diva's Multicore fights host multicore and can slow down Apple Silicon.
9. **Accessible, themed and localised UI.** All are dark or photo-real, English-only, and at 1.3–1.8:1 knob contrast.
10. **An explanation of the sound.** No product tells you *why* the bass got thin (resonance loss) or *which* control
    causes the grit (mixer level into the nonlinear filter). An AI-assisted host can.

---

## 8. How Cantus beats them

### 8.0 Decision: a separate instrument, not a voice model inside Tabula

**Proposal:** **Calor** (Latin: warmth; also the Spanish word, which fits the EN/ES locales), a new `InstrumentSpec`
variant. It is built on a new **`cantus-dsp::va`** module shared with Tabula (wavetable.md) and the bass instrument
(bass.md). Firmus v1 stays **bit-identical**, the same rule ADR 0017 applied to the channel EQ.

**For a separate instrument:**
1. **A VA's product is a fixed, learnable signal path.** Repro, Mini V, Sylenth1 and Jup-8 V win because the panel
   *is* the synth: a player learns it in a minute and remembers it for years.
   - Tabula is a generator stack with a 64-slot matrix, optimised for flexibility.
   - As one "voice model" among many inside it, the VA would inherit a flagship's density and lose its panel.
2. **Different presets, different defaults, different users.** "Init" in a VA is a playable saw bass through a ladder.
   In a wavetable flagship it is a blank oscillator. The preset library, the AI descriptions and the help pages all
   stay simpler when the parameter set is small (≈120 keys, most of them sparse).
3. **Voice cards are an instrument-level idea.** Seeded, per-card trims and round-robin allocation define the VA's
   behaviour and the tests. In a stack synth they would be one more option most patches ignore.
4. **Model swapping (Diva's best idea) fits a fixed layout.** Oscillator, filter and envelope *models* change in place
   inside the same panel positions. That is a "voice model" *inside Calor*, not Calor inside Tabula.

**Where the reuse happens instead:**
- **At the library level.** `va::filters` (ladder, cascade, SVF, diode, Sallen-Key), `va::osc` (BLEP/BLAMP analog
  oscillators with sync, FM and shape revisions), `va::drift` (the counter-based RNG and card model) and `va::env`
  (the RC models).
  - Tabula takes its analog filters and an "Analog" generator from here, which delivers its own P2 list ("diode
    ladder, MS-20 filters, per-voice analog drift").
  - The bass instrument takes its ladder, diode and bass compensation from here.
- **At the host level.** One modulation service, one family frame, one preset library (family.md §7).

**The cost:** two instruments to maintain. It is mitigated because both are thin shells over the shared library and
shared host services.

### 8.1 P0: parity (without these we are not credible)

1. **The nonlinear ZDF filter family, with self-oscillation.** Ladder (6/12/18/24 dB taps), Cascade (12/24), and
   State-variable (LP→notch/BP→HP morph).
   - Resonance runs 0–1.2, where **1.0 is the marked self-oscillation threshold**.
   - Drive, **Bass comp 0–100 %**, key tracking where 100 % = exactly 1 V/oct, envelope ±, velocity, LFO, and
     audio-rate **filter FM**.
   - A non-resonant HPF (pre or post) with a Boost position.
2. **Analog oscillators ×3, plus sub and noise.**
   - Simultaneous saw, pulse (PW 5–95 %, PWM) and triangle levels (PolyBLEP/BLAMP).
   - **Hard sync** (2→1, 3→1), **cross-mod** (exponential FM 2→1), **poly-mod** (filter envelope and osc 3 → osc 1
     frequency, osc 1 PW, cutoff).
   - Osc 3 low-frequency mode (a mod source) and keyboard off.
   - Mixer levels drive the filter, with a visible input meter; a feedback loop; shape revisions (Ideal, Vintage A,
     Vintage B).
3. **Quality levels:** Draft 1×, **Good 2× (default)**, Best 4×.
   - One decimator per instance, after the voice sum.
   - **Render quality defaults to "same as live"**, so what you hear is what you render. "Best when rendering" is an
     opt-in, shown as a badge.
4. **Performance.**
   - Poly (1–16), Mono and Legato (last/low/high priority), **Unison stack 1–8** (poly-unison allowed).
   - Allocation Rotate / Reuse / Lowest; steal oldest-releasing first, with a **declick**.
   - Glide: time or rate, RC or linear, legato-only.
   - Pitch bend up and down separately, the mod wheel, velocity, aftertouch.
   - **MPE and per-note expression in the engine** from the first version.
5. **Envelopes and LFOs.**
   - 2 envelopes with models RC / ADS / Digital / One-shot, velocity and key scaling of the times, retrigger reset /
     legato / from-current.
   - 2 LFOs: 7 shapes, delay, restart free/gate/random, phase, host sync, unipolar option.
   - Hardware-style fixed routings (wheel mod, poly-mod) and **4 matrix slots** whose amounts are in target units.
6. **Analog variance, seeded.**
   - The Condition macro, plus expert drift, detune spread and cutoff/envelope/PW/glide variance.
   - Free / reset / DC-reset phase modes.
   - 16 voice cards with seeded trims; the seed is a parameter.
7. **Chorus in the output stage** (BBD-style I / II / I+II, plus mix). Other FX come from the host insert chain,
   saved as a **chain preset** together with the patch.
8. **Parity plumbing.**
   - Every gesture is one undoable command.
   - Sparse `key → f32` parameters with stable keys (ADR 0017), a schema bump plus a fixture, **undo fuzz including
     "re-roll cards"**, EN/ES strings, null tests first.

### 8.2 P1: differentiators (what a host-native instrument can do)

1. **Reproducible analog.**
   - Counter-based drift; a card and phase from the note-ID hash for sequenced notes.
   - **Bit-identical renders, independent of block size and of where playback starts.**
   - A per-track seed derived from the project seed and the track ID, so duplicating a track doesn't produce two
     identical synths that flange. "Keep seed" is an option.
   - "Re-roll card 5" and "Re-roll all" are undoable commands.
2. **Automatable switches, click-free.**
   - A model, slope or mode change crossfades each voice over 5 ms between the old and new model, running both only
     for those 5 ms.
   - Because **the host sees the automation lane ahead of time**, it can pre-warm the new model's state one block
     early. Diva can't, so it forbids switch automation.
   - Option: "apply on the next note".
3. **Voice cards on the main view.**
   - 16 LEDs show which card is sounding. Click one to expand its trims (cents, semitones, time ×, pan).
   - Seed chip, Calibrate (zero all), Re-roll.
   - The piano roll can colour notes by the card that played them.
4. **One undo, A/B and preset library with the project.**
   - "Usage" notes on presets (Diva's idea).
   - Presets carry their insert-chain preset.
   - Tags shared across devices; "presets used in this project".
5. **Modulation across the boundary** (family.md §7.6):
   - Calor's envelopes and LFOs become **track-level sources** that can drive Aequalis gains, send levels or pan on the
     same track.
   - Host sources can drive Calor: a sidechain envelope follower from the kick track can **duck the cutoff** with no
     routing.
   - Arrangement macro lanes can slowly raise **Condition across the song**, so the synth "ages" through the track.
6. **Channel rack integration.**
   - Per-step parameter locks (cutoff, resonance, accent, slide, card) stored as per-note expression; this shares the
     303 accent and slide work in bass.md.
   - The rack row shows the card LEDs and the Condition.
7. **Per-note expression lanes in the piano roll**, for MPE and CLAP-style note expressions (pitch, pressure, timbre,
   brightness, pan).
8. **Project tuning** (Scala .scl/.kbm) feeding every instrument. MTS-ESP only matters for third-party plug-ins.
9. **The AI command bridge.**
   - The AI programs patches with the same commands a user's hands produce.
   - It **explains the sound** from the units: "the bass thinned because resonance 0.95 costs 12 dB at DC on the
     Ladder; raise Bass comp or switch to State-variable".
   - It translates "warmer", "more vintage" or "tighter" into documented parameter moves.
   - It describes presets in EN/ES.
10. **Rendering is known.** The engine *is* the renderer, so "Best when rendering" never depends on a host flag.
    Freezing and bouncing use it and show it.
11. **Honest units everywhere.**
    - Cutoff in Hz and as a note.
    - Resonance with the self-oscillation mark.
    - Drift in cents, card offsets in cents and semitones, envelope times in ms and s.
    - Condition with named stops: **Calibrated · Settled · Worn · Weathered · Unruly**. These are our own names, not
      Cherry's.

### 8.3 P2: later

- Diode and Sallen-Key filters, if the bass work hasn't already brought them.
- **Circuit-level "Divine" models** for one or two filters: nodal DK-method or WDF solvers (Yeh, Abel & Smith 2010;
  Holters & Zölzer 2015), used offline or for Best.
- Duo and paraphonic modes; chord memory; dual layers (the host can already do this with two rack channels and key
  ranges).
- A Behaviours list (Repro's jumpers, but plain): LFO DC, filter key + pitch bend, and similar.
- A voice-parallel render inside the host's worker pool. The rule: **no second thread pool fighting the host**, unlike
  Diva's Multicore.
- Hardware patch import (public SysEx formats) through the "DAW interop later" track. This needs trademark care.
- Exporting Calor as a CLAP plug-in (standards.md).

### 8.4 Voice architecture

```
            note on (pitch, vel, note-id) ──► allocator: Rotate | Reuse | Lowest ─► card k (seeded trims)
                                                  │ steal: oldest-releasing → oldest → quietest (2 ms declick)
 per voice, at 1×/2×/4×:
  ┌──────── OSC 1 ─┐ sync◄─┐ xmod◄─┐
  │ saw/pls/tri    │       │       │      ┌─ MIXER ──────────┐   ┌─ HPF ─┐   ┌─ VCF (model) ────────┐   ┌─ VCA ─────┐
  ├──────── OSC 2 ─┤───────┴───────┴─────►│ o1 o2 o3 sub noise├──►│pre/off├──►│ Ladder│Cascade│SVF   ├──►│ env/gate  ├─► pan(card)
  ├──────── OSC 3 ─┤ (low-freq / kbd off)  │ + feedback ◄──────┼───┼───────┼───┼─ drive, bass comp,   │   │ drive     │     │
  └────────────────┘                       │ drive ► [meter]   │   │ boost │   │  reso 0..1.2, FM◄osc3│   └───────────┘     │
   ▲ pitch: key + glide + bend + drift(card, t) + card offset + vibrato + poly-mod + MPE bend                              │
   ▲ ENV1 (filter) ENV2 (amp)  LFO1 LFO2  wheel  matrix×4 (target units)                                                    │
                                                                                                                            ▼
 instance:  Σ voices ─► decimate (Comprimo half-band, whole-sample latency) ─► chorus (BBD I/II) ─► out gain ─► host inserts
```

- **Grid:** modulation updates every 16 samples, counted from the **stream start** (Aequalis rule). Envelope→cutoff and
  filter FM run **per sample** at the oversampled rate.
- **State:** u32 phase accumulators; the filter state is f64 in the reference implementation; no allocation after
  construction; a per-sample denormal flush (as in `filter.rs`).
- **Voice budget:** voices × stack ≤ 32, and the footer shows the measured %/voice.

### 8.5 Filter models to build first

1. **Linear TPT ladder with 4 taps and bass compensation.** It is the test oracle: its self-oscillation must land
   exactly on the cutoff.
2. **The nonlinear ladder, "Good":** the cheap ZDF plus 1 Newton step, per-stage tanh on our own approximation.
   **"Best":** Newton to 1e-6 with a cap of 8.
3. **Cascade (OTA):** the same code with the nonlinearity at each OTA input and a feedback VCA. The character is a
   brighter top and a slightly different resonance.
4. **State-variable (nonlinear):** extend Firmus's Simper SVF with soft saturation in the integrator feedback, the
   LP→notch/BP→HP morph, and self-oscillation at zero damping.
5. **Diode** (shared with the bass instrument), then **Sallen-Key**.

Each ships with its measured linear response and its self-oscillation tracking curve as test artefacts.

### 8.6 Parameter list (P0)

Keys are sparse `key → f32`: an enum is stored as its index and a bool as 0/1. Log-scaled parameters take typed units
through the family parser (`2k`, `A4`, `150ms`). Everything can be automated and modulated unless marked **(n)**.

| Key | Range | Default | Unit / notes |
|---|---|---|---|
| `voice.mode` | poly · mono · legato | poly | (n) switch, automatable with crossfade (P1) |
| `voice.count` | 1–16 | 8 | voices (n) |
| `voice.stack` | 1–8 | 1 | unison voices per note (n) |
| `voice.stack_detune` | 0–50 | 10 | cents, spread across the stack |
| `voice.stack_spread` | 0–100 | 50 | % pan spread |
| `voice.assign` | rotate · reuse · lowest | rotate | (n) |
| `voice.priority` | last · low · high | last | mono and legato |
| `voice.steal` | released-first · oldest · quietest | released-first | (n) |
| `glide.time` | 0–10 | 0 | s, log |
| `glide.mode` | time · rate | time | (n) |
| `glide.curve` | rc · linear | rc | |
| `glide.legato` | off/on | off | glide only when notes overlap |
| `tune.fine` / `tune.transpose` | ±100 / ±24 | 0 / 0 | cents / semitones |
| `bend.up` / `bend.down` | 0–48 | 2 / 2 | semitones |
| `osc{1,2,3}.on` | off/on | 1, 1, 0 | |
| `osc{n}.octave` | −2…+2 | 0 | octaves (32′…2′) |
| `osc{n}.semi` | ±12 | 0 | semitones |
| `osc{n}.fine` | ±50 | 0, +6, −4 | cents |
| `osc{n}.saw` / `.pulse` / `.tri` | 0–1 | 1/0/0 | level of each simultaneous wave |
| `osc{n}.pw` | 5–95 | 50 | % |
| `osc{n}.pwm` | 0–100 | 0 | % of LFO 2 depth |
| `osc{n}.rev` | ideal · vintage A · vintage B | vintage A | waveform revision |
| `osc{2,3}.sync` | off/on | off | hard sync to osc 1 |
| `osc3.low` / `osc3.kbd` | off/on | off / on | LFO range; key tracking |
| `xmod.amount` | 0–100 | 0 | % (exponential FM, osc 2 → osc 1) |
| `polymod.env` / `polymod.osc3` | ±100 / 0–100 | 0 / 0 | % |
| `polymod.to_freq` / `.to_pw` / `.to_vcf` | off/on | on / off / off | destinations |
| `sub.level` / `sub.shape` | 0–1 / sq−1 · sq−2 · pulse−1 | 0 / sq−1 | follows osc 1 |
| `noise.level` / `noise.color` | 0–1 / white · pink | 0 / white | |
| `mix.osc{n}` | 0–1 | 0.8, 0.8, 0 | level into the filter (drives it) |
| `mix.feedback` | 0–1 | 0 | post-VCA → mixer |
| `mix.drive` | −12…+24 | 0 | dB; the input meter shows the level reaching the filter |
| `hpf.cutoff` | 10–2 000 | 10 | Hz, log (10 = off) |
| `hpf.pos` | pre · post · boost | pre | boost = +6 dB low shelf, post-filter |
| `vcf.model` | ladder · cascade · svf (· diode · sk, P1) | ladder | (n) crossfaded switch |
| `vcf.slope` | 24 · 18 · 12 · 6 | 24 | dB/oct (ladder and cascade taps) |
| `vcf.morph` | 0–1 | 0 | SVF: LP → notch/BP → HP |
| `vcf.notch_bp` | notch · bp | notch | SVF centre position |
| `vcf.cutoff` | 20–20 000 | 2 000 | Hz, log; also shown as a note |
| `vcf.reso` | 0–1.2 | 0.2 | **1.0 = self-oscillation threshold** |
| `vcf.drive` | −12…+24 | 0 | dB (the model's input gain) |
| `vcf.bass_comp` | 0–100 | 50 | % of the 1/(1+k) loss restored |
| `vcf.key` | 0–120 | 50 | % (100 = 1 V/oct, pivot C4) |
| `vcf.env` | ±8 | +2 | octaves from ENV 1 |
| `vcf.vel` | 0–100 | 30 | % velocity → env amount |
| `vcf.lfo` | ±4 | 0 | octaves from LFO 2 |
| `vcf.fm` | 0–100 | 0 | % audio-rate FM from osc 3 |
| `vcf.rev` | clean · rough · pushed | clean | the model's revision |
| `vca.source` | env2 · gate | env2 | |
| `vca.level` | −∞…+6 | 0 | dB |
| `vca.vel` | 0–100 | 70 | % |
| `vca.drive` | 0–100 | 0 | % soft saturation |
| `vca.pan` | ±100 | 0 | % |
| `env{1,2}.a` | 0.5 ms–20 s | 5 ms / 2 ms | log |
| `env{e}.d` | 1 ms–30 s | 300 / 200 ms | log |
| `env{e}.s` | 0–100 | 30 / 80 | % |
| `env{e}.r` | 1 ms–30 s | 300 / 250 ms | log |
| `env{e}.model` | rc · ads · digital · oneshot | rc | (n) |
| `env{e}.kbd` | 0–100 | 0 | % key scaling of the times |
| `env{e}.vel` | 0–100 | 0 / 0 | % velocity → level |
| `env{e}.retrig` | from-current · reset · legato | from-current | |
| `lfo{1,2}.rate` | 0.01–50 Hz or a sync division | 5 Hz / 0.8 Hz | log |
| `lfo{l}.shape` | tri · sine · saw↑ · saw↓ · square · s&h · smooth | tri | |
| `lfo{l}.delay` | 0–10 | 0 | s (fade-in) |
| `lfo{l}.restart` | free · gate · random | free / gate | free = all voices in phase |
| `lfo{l}.phase` | 0–360 | 0 | ° |
| `lfo{l}.unipolar` | off/on | off | |
| `wheel.vibrato` | 0–100 | 30 | cents at full wheel (LFO 1) |
| `wheel.mix` | 0–1 | 0 | LFO 1 ↔ noise (hardware "wheel mod") |
| `wheel.to_pw` / `.to_vcf` | 0–100 / ±4 | 0 / 0 | % / octaves |
| `mx{1..4}.src` | velocity · key · wheel · pressure · timbre · bend · env1 · env2 · lfo1 · lfo2 · random · card · stack | — | (n) |
| `mx{i}.dst` | any continuous key | — | (n) |
| `mx{i}.amt` | ± the target's range | 0 | **in the target's units** |
| `mx{i}.via` | a source or none | none | scales the amount |
| `analog.condition` | 0–100 | 25 (Init: 0) | % macro: Calibrated · Settled · Worn · Weathered · Unruly |
| `analog.seed` | 0–65 535 | derived | from project seed × track id |
| `analog.drift` | 0–20 | 3 | cents (scaled by Condition) |
| `analog.drift_rate` | 0.02–2 | 0.2 | Hz |
| `analog.detune` | 0–20 | 4 | cents, the static card spread |
| `analog.var_cutoff` | 0–1 | 0.3 | semitones, card spread |
| `analog.var_env` | 0–20 | 5 | % time spread |
| `analog.var_pw` / `var_glide` | 0–5 / 0–20 | 1 / 5 | % |
| `analog.tracking` | 0–10 | 0.5 | cents per octave, card scale error |
| `analog.phase` | free · reset · dc-reset | free | |
| `analog.reset_phase{n}` | 0–360 | 0 | ° (reset mode) |
| `card{1..16}.osc{n}` / `.cutoff` / `.env` / `.pan` | ±50 ¢ / ±1 st / ±20 % / ±100 % | *generated from the seed unless stored* | sparse; the Re-roll and Calibrate commands |
| `quality.live` | draft · good · best | good | (n) |
| `quality.render` | same · best | same | (n) |
| `chorus.mode` | off · I · II · I+II | off | |
| `chorus.mix` | 0–100 | 50 | % |
| `out.gain` | −∞…+12 | −6 | dB |
| `mpe.on` / `mpe.bend` | off/on / 1–96 | off / 48 | semitones |

### 8.7 Editor layout (family frame; flat; light theme by default)

```
┌─ header ────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ [■ Bass 1]  CALOR   ↶ ↷ │ A|B →Copy │ ‹ Warm Brass ● › │ Δ │ ⏻ │ ? │ ⋯ │ ⛶                              │
├─ OSCILLATORS ───────────────────┬─ MIXER ─────────┬─ FILTER  [Ladder|Cascade|SVF] [24|18|12|6] ─┬─ AMP ──────┤
│ 1  oct◯ semi◯ fine◯  saw▮ pls▮ tri▮ PW◯ │ 1 ▮ 2 ▮ 3 ▮     │ ┌ response + live cutoff dot, self-osc ┐ │ Level◯     │
│ 2  oct◯ semi◯ fine◯  saw▮ pls▮ tri▮ PW◯ │ sub▮ noise▮     │ │ mark on the Reso arc; input meter    │ │ Vel◯       │
│    [sync] xmod◯                         │ feedback▮       │ └──────────────────────────────────────┘ │ Drive◯     │
│ 3  oct◯ semi◯ fine◯  … [sync][low][kbd] │ drive◯ ▯▯▯ in   │ CUTOFF◯(L) RESO◯(L) Drive◯ BassComp◯     │ Pan◯       │
│ POLY-MOD env◯ osc3◯ → [A freq][A pw][vcf] │  HPF◯ [pre|post|boost] │ Key◯ Env◯ Vel◯ LFO◯ FM◯     │ [Env2|Gate]│
├─ ENV 1 · filter ──┬─ ENV 2 · amp ────┬─ LFO 1 ───────┬─ LFO 2 ───────┬─ MOD ─────────────────────────────────┤
│ curve (drawn, live│ curve            │ shape + phase │ shape + phase │ wheel: vib◯ mix◯ →pw◯ →vcf◯           │
│ playhead per card)│ A D S R sliders  │ rate◯ delay◯  │ rate◯ delay◯  │ 1 pressure → cutoff  +1.0 oct  [via –]│
│ A D S R sliders   │ [RC|ADS|Dig|1sh] │ [free|gate|rnd]│              │ 2 timbre  → vcf.morph +0.30  …        │
├─ VOICES ─────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ [Poly|Mono|Legato] 8 voices · stack 1 · [Rotate|Reuse|Lowest] · glide◯ [time|rate] [RC|lin]                  │
│ CARDS ● ● ◐ ○ ○ ○ ○ ○  (click = trims ▾)    CONDITION ━━━●━━━━━ Settled   seed 4127 ↻   phase [free|reset]  │
├─ footer ─────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Learn · Quality [Good 2×] · Render [same] · CPU 0.6 %/voice · 4.1 % · MPE ✓ · Tuning 12-TET · In◯ Out◯ Mix◯ · ⤡│
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Reading order is the signal flow**: left to right, then the modulators under the stages they drive. The section
  headers are joined by thin flow connectors, so the panel teaches the block diagram (va-08).
- **Knob sizes** (family grid): Cutoff and Resonance are L (56 px); everything else M (36) or S (28). Envelopes are
  **sliders**, whose position reads at a glance (Jup-8 V's strength), with the RC curve drawn above them.
- **State is visible at rest:**
  - the Reso arc carries a **tick at 1.0**, labelled "self-osc";
  - Bass comp shows its dB restored;
  - the mixer's input meter shows how hard the filter is driven;
  - the Condition shows its named stop;
  - the card LEDs are always visible;
  - modulated knobs get the family's outer ring (family.md §7.6).
- **The card drawer** expands under CARDS: a 16-column grid (cents per oscillator, cutoff in semitones, env ×, pan)
  with Re-roll, Calibrate and Copy card. It replaces Diva's hidden Trimmers tab.
- **A "Panel only" view** (Cherry's Hide Extras) hides the matrix and the card drawer. **Macros-only** is the default
  view for presets (family.md §7.6).
- **Visual rules:**
  - dark knob bodies with a ≥3:1 edge on the light panel (Spire and Serum-2 legibility);
  - the device hue is a warm copper, validated to 3:1 on the light and dark surfaces;
  - no wood, leatherette, chrome caps, LCD pixel fonts or replica switches;
  - IBM Plex, with monospaced values.

### 8.8 Trade dress and naming rules

- **Name by topology**: Ladder, Cascade, State-variable, Diode, Sallen-Key; revisions Clean, Rough, Pushed; oscillator
  revisions Ideal, Vintage A, Vintage B.
  - No hardware, brand, chip or artist names in the UI, the preset names or the tags.
  - Chip lineage may appear in developer docs only, as descriptive engineering.
  - This is u-he's approach ("a specific piece of monophonic hardware"). Arturia's and Cherry Audio's replica panels
    and model names are the approach **not** to follow.
- **No replica layouts:**
  - no row of slider banks under coloured header strips in a known synth's order;
  - no three-section "Controllers / Oscillator Bank / Mixer / Modifiers" layout;
  - no wood strip above the keys;
  - no on-screen keyboard as the main visual.

  Our layout is the family frame plus the signal flow.
- **Condition names are our own.** Cherry uses "Brand new / In studio / Road worn / 44 years!", and Arturia's copy
  speaks of an "attic find". Avoid both.
- Factory presets are named for the sound ("Round Brass", "Glass Pluck"), never after songs or artists. Arturia's and
  Cherry's preset lists name songs and artists.

---

## 9. Test plan hints (null tests first)

1. **Self-oscillation pitch tracking.**
   - Setup: `vcf.reso` = 1.05, key 100 %, no oscillators, a seeded noise floor to start the oscillation.
   - Measure the f0 at C1…C8 at 44.1, 48 and 96 kHz, for each quality level and model.
   - Linear ladder: within **±3 cents** from 30 Hz to 12 kHz at 2×.
   - Nonlinear models: publish the tracking-error curve, and hold **±10 cents** from 50 Hz to 5 kHz as the target.
2. **Resonance vs level.**
   - Sweep resonance 0 → 1 at 1 kHz and measure the gain at 100 Hz. With `bass_comp` = 0, the linear ladder must
     follow 1/(1+k) (−14 dB at threshold, ±0.3 dB). At 100 % the loss must be ≤ 0.5 dB.
   - The SVF must show no loss.
   - Sweep a sine at the cutoff from −40 to +12 dBFS: THD must rise monotonically with drive, and the
     self-oscillation amplitude must stay bounded.
3. **Aliasing floor.**
   - Oscillators alone: saw, 10 % pulse, triangle, a sync sweep and cross-mod, with f0 swept from 20 Hz to 8 kHz.
   - Reuse the periodic-DFT method in `osc.rs`.
   - Report the largest non-harmonic bin in the 20 Hz–20 kHz band relative to the fundamental. Targets: **Draft ≤
     −60 dB, Good ≤ −80 dB, Best ≤ −100 dB**.
   - Filter: a 1 kHz saw at +12 dB drive and reso 0.9, compared with a 16× reference render.
4. **Small-signal response.** Each model's linear magnitude vs its analog prototype: within ≤ 0.1 dB up to 0.4·fs
   (2×). This is the Aequalis-style artefact.
5. **Modulation robustness.**
   - A per-sample random cutoff and resonance fuzz (resonance ≤ 1.2, 10⁶ samples): the output stays finite and below
     +24 dBFS.
   - A 1 s sweep from 20 Hz to 20 kHz: no step larger than a bound.
   - Filter FM at 100 % stays stable.
6. **CPU per voice.**
   - A benchmark renders 1 voice for 10 s for each quality × model, reporting ns per sample, % of a core, and a Newton
     iteration histogram (cap respected).
   - Worst case: noise input at maximum resonance.
   - Budgets as in 4.7. Fail the build if a quality level regresses by more than 15 %.
7. **Block-size independence.** Render the same MIDI with blocks of 1, 7, 64, 256 and 4096 and with random splits:
   the results must be **bit-identical** (`nulltest.rs`). Include drift, LFOs, glide, mid-block parameter changes and
   note events at sample offsets.
8. **Deterministic drift.**
   - The same seed gives bit-identical renders across runs; a different seed gives a different render.
   - **Seek invariance:** rendering bars 9–12 alone equals bars 9–12 of the full render (in a region with no earlier
     tails).
   - Golden hashes in CI on Windows, macOS and Linux, which proves our own `tan`, `tanh` and `exp2` are used.
9. **Allocation.** Rotate cycles the cards; Reuse maps the same pitch to the same card; steal order; stack count;
   legato doesn't retrigger; glide time vs rate; note-tag release (existing).
10. **Envelopes.**
    - The attack reaches its peak in the set time ±5 %.
    - The RC curve matches its analytic form.
    - **Retrigger and steal show no step greater than −60 dBFS** (declick).
    - Velocity and key scaling of the times.
11. **Switch automation.** Change the filter model every 100 ms over a held chord: no click above the threshold, and a
    bounded CPU spike.
12. **Sample-rate independence.** The same patch at 44.1, 48 and 96 kHz gives the same cutoff response (within
    tolerance), the same drift rate in Hz and the same envelope times.
13. **MPE.** A per-note bend reaches only its own voice; the expression is frozen after note-off; the channel is reused
    safely.
14. **Idle and denormals.** The output is exactly 0 after release, and there is no CPU while idle (as in the existing
    tests).
15. **Project plumbing.**
    - Undo fuzz with Calor's parameters, **Re-roll cards** and model switches in the random walk.
    - A schema bump and a fixture.
    - A "Firmus v1 unchanged" null test against the stored render.
    - EN/ES strings for every key.

---

## 10. Sources

**Product pages, manuals and changelogs**
- u-he Diva: https://u-he.com/products/diva/ · user guide v1.4.8 (7 Aug 2025): https://u-he.com/downloads/manuals/plugins/diva/Diva-user-guide.pdf
- Urs Heckmann on Diva's accuracy modes (KVR u-he forum): https://www.kvraudio.com/forum/viewtopic.php?t=394742
- Urs Heckmann, "Zero Delay Feedback" (4 Feb 2016): https://urs.silvrback.com/zero-delay-feedback
- u-he Repro: https://u-he.com/products/repro/ · Repro-5 guide: https://dl.u-he.com/manuals/plugins/repro/Repro-5-user-guide.pdf · Repro-1 guide (listed, not read): https://dl.u-he.com/manuals/plugins/repro/Repro-1-user-guide.pdf
- Arturia Mini V: https://www.arturia.com/products/software-instruments/mini-v/overview · Jup-8 V: https://www.arturia.com/products/software-instruments/jup-8-v/overview · Prophet-5 V: https://www.arturia.com/products/software-instruments/prophet-5-v/overview
- Arturia TAE (search excerpt only): https://www.arturia.com/products/technology/tae%C2%AE
- V Collection 11 (search excerpts): https://www.arturia.com/company/press/press-releases/arturia-announces-v-collection-11 · https://sonicstate.com/news/2025/04/24/arturia-v-collection-11-7-new-instruments/ · https://musictech.com/news/gear/arturia-v-collection-11-new-plugins-price-intro-everything-you-need-to-know/
- Memory V review (Synth Anatomy, May 2026): https://synthanatomy.com/2026/05/arturia-memory-v-review-the-memorymoog-taken-to-your-daw-with-extras.html
- Arturia CLAP request (KVR, excerpt only): https://www.kvraudio.com/forum/viewtopic.php?t=583806
- LennarDigital Sylenth1: https://www.lennardigital.com/sylenth1/ · specs: https://www.lennardigital.com/sylenth1/specs.php · downloads: https://www.lennardigital.com/downloads/ · store: https://www.lennardigital.com/store/ · 3.072 notes: https://www.kvraudio.com/forum/viewtopic.php?t=590403
- Reveal Sound Spire: https://www.reveal-sound.com/ · features: https://www.reveal-sound.com/plug-ins/spire · changelog: https://www.reveal-sound.com/content/changelog.php?spire
- Cherry Audio Mercury-8: https://cherryaudio.com/products/mercury-8 · products: https://cherryaudio.com/products
- OB-Xd: https://github.com/reales/OB-Xd · original: https://github.com/2DaT/Obxd
- TAL-U-NO-LX: https://tal-software.com/products/tal-u-no-lx
- Surge XT manual: https://surge-synthesizer.github.io/manual-xt/

**Reviews (forum-grade unless stated)**
- KVR Diva reviews: https://www.kvraudio.com/product/diva-by-u-he/reviews
- KVR Sylenth1 reviews: https://www.kvraudio.com/product/sylenth1-by-lennardigital/reviews
- KVR Spire reviews: https://www.kvraudio.com/product/spire-by-reveal-sound/reviews
- musicproductionwiki Diva review (June 2026; low-grade): https://musicproductionwiki.com/articles/u-he-diva-review

**DSP literature** († = PDF retrieved and checked this round; the others are standard citations from memory, so check
page numbers before quoting)
- V. Zavalishin, *The Art of VA Filter Design*, rev. 2.1.2, Native Instruments (announcement thread: https://www.kvraudio.com/forum/viewtopic.php?t=350246; NI PDF: https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.2.pdf)
- A. Simper, "Linear Trapezoidal Integrated SVF", Cytomic, 2013 †: https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf
- A. Huovilainen, "Non-linear digital implementation of the Moog ladder filter", DAFx-04 †: https://www.dafx.de/paper-archive/2004/P_061.PDF
- T. Stilson, J. Smith, "Analyzing the Moog VCF with considerations for digital implementation", ICMC 1996 †: https://ccrma.stanford.edu/~stilti/papers/moogvcf.pdf
- T. Stilson, J. Smith, "Alias-free digital synthesis of classic analog waveforms", ICMC 1996 †: https://ccrma.stanford.edu/~stilti/papers/blit.pdf
- E. Brandt, "Hard sync without aliasing", ICMC 2001 †: http://www.cs.cmu.edu/~eli/papers/icmc01-hardsync.pdf
- T. Stinchcombe, "Analysis of the Moog transistor ladder and derivative filters", 2008 †: http://www.timstinchcombe.co.uk/synth/Moog_ladder_tf.pdf
- T. Stinchcombe, "A study of the Korg MS10 & MS20 filters", 2006 †: http://www.timstinchcombe.co.uk/synth/MS20_study.pdf
- T. Voipio (mystran), "Cheap non-linear zero-delay filters", KVR DSP forum †: https://www.kvraudio.com/forum/viewtopic.php?t=349859
- S. D'Angelo, V. Välimäki, "An improved virtual analog model of the Moog ladder filter", ICASSP 2013; "Generalized Moog ladder filter", parts I and II, *IEEE/ACM TASLP* 22(12), 2014
- V. Välimäki, A. Huovilainen, "Oscillator and filter algorithms for virtual analog synthesis", *Computer Music Journal* 30(2), 2006; "Antialiasing oscillators in subtractive synthesis", *IEEE Signal Processing Magazine* 24(2), 2007
- V. Välimäki, "Discrete-time synthesis of the sawtooth waveform with reduced aliasing", *IEEE SPL* 12(3), 2005; V. Välimäki, J. Nam, J. O. Smith, J. Abel, "Alias-suppressed oscillators based on differentiated polynomial waveforms", *IEEE TASLP* 18(4), 2010
- F. Esqueda, V. Välimäki, S. Bilbao, "Rounding corners with BLAMP", DAFx-16 †: https://dafx.de/paper-archive/2016/dafxpapers/18-DAFx-16_paper_33-PN.pdf
- J. Parker, V. Zavalishin, E. Le Bivic, "Reducing the aliasing of nonlinear waveshaping using continuous-time convolution", DAFx-16 † (extends to feedback systems): https://dafx.de/paper-archive/2016/dafxpapers/20-DAFx-16_paper_41-PN.pdf
- M. Holters, "Antiderivative antialiasing for stateful systems", DAFx-19 †: https://www.dafx.de/paper-archive/2019/DAFx2019_paper_4.pdf
- S. Bilbao, F. Esqueda, J. Parker, V. Välimäki, "Antiderivative antialiasing for memoryless nonlinearities", *IEEE SPL* 24(7), 2017
- D. Yeh, J. Abel, J. O. Smith, "Automated physical modeling of nonlinear audio circuits for real-time audio effects — Part I", *IEEE TASLP* 18(4), 2010 (the DK method); M. Holters, U. Zölzer, "A generalized method for the derivation of non-linear state-space models from circuit schematics", EUSIPCO 2015
- W. Pirkle, *Designing Software Synthesizer Plug-ins in C++*, 2nd ed., Focal Press/Routledge, 2021 (VA ladder, diode ladder, Korg35, oscillators)
- J. Pekonen, V. Välimäki, "The brief history of virtual analog synthesis", Forum Acusticum 2011

**Cantus context read:** `research/instruments/README.md`, `research/fabfilter/family.md` §0 and §7,
`research/fabfilter/pro-q.md` §0 and §11, `research/instruments/wavetable.md` §0 and §8 (Tabula),
`research/instruments/bass.md` (ladder, diode and bass compensation), `crates/cantus-dsp/src/{firmus,osc,filter,env}.rs`,
`crates/cantus-core/src/instrument.rs`, `docs/adr/0017-plug-in-family.md`.
