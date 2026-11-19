# FL Studio 2026 — its native plug-ins, and Cantus's against them

*Research only, 2026-11-19, for BACKLOG §6l. FL Studio 2026 (26.1, released 2 July 2026,
announced 7 July; newest build 26.1.7.5, 12 Oct 2026). Every edition mark comes from
Image-Line's Compare Editions table, checked against the online manual's plug-in index and
its changelog (sources at the end). Cantus's side was read from the code at master
8adaf6b7.*

## 1. Editions, by Image-Line's own counts

| Edition | Price seen (list) | Total | Effects | Instruments | Audio editors | Video/visual |
|---|---|---|---|---|---|---|
| Fruity | $99 | 81 | 53 | 23 | 0 | 5 |
| Producer | $139 ($179) | 93 | 59 | 27 | 2 | 5 |
| Signature | $199 ($269) | 103 | 65 | 29 | 3 | 6 |
| All Plugins | $224 ($449) | 119 | 71 | 39 | 3 | 6 |

Prices are a snapshot of the store page (a sale, it seems). Fruity has no audio recording
and no stem separation, and holds 8 audio clips at most. FL Cloud (Free / Plus / Pro) is a
separate subscription: samples, AI-mastering presets, third-party plug-ins (its 2026 tiers
not verified).

## 2. Instruments (39 in All Plugins)

- **All Plugins only (10):** Drumaxx (physical-model drums), Harmor (additive/subtractive,
  image resynthesis), Kepler Exo (2024.1: Kepler with a second oscillator, sync, FM, a
  16-slot matrix), Morphine (additive, morphing, resynthesis), Ogun (metallic additive),
  PoiZone (subtractive), Sakura (string modelling), Sawer (vintage subtractive), Toxic
  Biohazard (FM/subtractive), Transistor Bass (303-style with a sequencer).
- **Signature and up (2):** DirectWave Full (sampler), Harmless (additive, subtractive
  workflow).
- **Producer and up (4):** Kepler (Oct 2023, Juno/Jupiter-style), Slicex, SoundFont Player,
  Sytrus (FM/RM/subtractive).
- **All editions (23):** 3x OSC, Autogun, BassDrum, BeepMap (image to sound), BooBass,
  Channel Sampler, DirectWave Player, Drumpad, FLEX (rebuilt 2026, "up to 50 % less CPU"),
  Fruity DrumSynth Live, Fruity DX10, Fruity Granulizer, Fruit Kick, FPC (2026: several
  samples per pad), Fruity Slicer, Fruity Slicer 2 (25.2, Nov 2025), GMS, MiniSynth,
  Plucked!, SimSynth, Speech Synthesizer, Wave Traveller, FL Keys.
- In the manual, not counted: Wasp/Wasp XT (discontinued), Groove Machine (GMS replaced
  it), Dashboard and SynthMaker (old projects), utilities (Layer, MIDI Out, ReWire, Buzz
  adapter, Fruity Dance, Fruity Vibrator), the Audio and Automation Clip generators.

## 3. Effects (71 in All Plugins; P Producer, S Signature, AP All Plugins)

- **Dynamics and mastering:** Emphasis [AP] (25.1), Emphasizer (25.2), Maximus [P], Fruity
  Compressor (sidechain in 25.2), Fruity Limiter, Fruity Multiband Compressor, Fruity Soft
  Clipper, Soundgoodizer, Transient Processor [AP], Transmitter [AP] (2026, transient and
  sustain apart), Low Lifter [S] (2024.1).
- **EQ and filter:** EQUO (morphing graphic EQ), Parametric EQ, Parametric EQ 2 (HQ 2024),
  Fruity Filter, Love Philter, Frequency Splitter, Delay Bank, Convolver.
- **Reverb and delay:** LuxeVerb [AP], Reeverb 2, Convolver, Delay 2, Delay 3, Multiband
  Delay [P].
- **Modulation:** Chorus, Flanger, Flangus, Phaser, Hyper Chorus [P], Vintage Chorus [S],
  Vintage Phaser [S], PanOMatic, Frequency Shifter [P].
- **Distortion:** Distructor, Blood Overdrive, Fast Dist, Squeeze, WaveShaper, Hardcore [S]
  (11 guitar effects as one).
- **Creative, time, pitch:** Effector (12 as one), Gross Beat [S], Transporter [AP]
  (2024.2), Scratcher, Vocoder, Vocodex [P], Pitcher [S], Pitch Shifter [AP], Tuner.
- **Stereo and routing:** Spreader [P] (2024.1), Stereo Enhancer, Stereo Shaper, Balance,
  Send, LSD.
- **Control, modular, notes:** Patcher, Control Surface, Formula Controller, Peak
  Controller, X-Y and X-Y-Z Controllers, the VFX modules (Color Mapper, Envelope, Key Mapper,
  Keyboard Splitter, Level Scaler, Sequencer, VFX Script 25.1), Razer Chroma, the NoteBooks.
- **Visual (6):** Wave Candy, Spectroman, dB Meter, Big Clock, ZGameEditor Visualizer,
  Video Player [S].

About 45–48 of the 71 process audio; the rest are controllers, modules, notebooks.

## 4. Features that act as instruments or effects

Edison [P] (wave editor; separation, Deverb, declipper, denoiser), Newtime [P], Newtone
[S]; stem separation [P] (21.2; per-stem mixer tracks 25.2; "Remix a song" 2026); FL Cloud
mastering; Gopher (AI assistant, 25.1; acts on FL in 2026); Loop Starter (25.1); chord
progression generator (2024) and Chord Stamp (2026); Note Repeat (2024); Audio Logger
(2026); the FL Studio Mobile Rack (25.1: 8 instruments, about 30 effects, 5 note effects
incl. an arpeggiator in 2026 — counted as one plug-in).

## 5. Cantus against it (2026-11-19)

- **Instruments, 12:** Firmus (subtractive), Calor (analog polysynth), Unda (wavetable,
  hybrid), Modulus (FM and additive), Exemplar (SFZ/SF2/Decent sampler, Sectio slicer),
  Gravis (808, modelled electric, synth bass), Lamina (modelled electric piano), Organum
  (tonewheel organ and rotary), Chorda (modelled grand), Pulsus (synthesised kit, per-pad
  chains), Tympanum (modelled and recorded multi-mic drums), the dancer (no sound).
- **Effects, about 48** (`cantus-core/src/effect.rs`): EQ, compressor, limiter, two reverbs,
  multiband, de-esser, gate, saturator, stereo, maximizer, chorus, flanger, phaser,
  tremolo, vibrato, ring mod, frequency shifter, volume shaper, clipper, transient
  processor and splitter, analyser, auto-tuner, convolution, auto filter, pedals, time
  shaper, lo-fi, utility, pedalboard, pitch shifter, rotary, amp, vocoder, glitch, shimmer,
  bus compressor, resonance suppressor, ducker, repair, reference comparator.
- **Note effects, 3:** arpeggiator, chord, scale.
- **FL has, Cantus has no answer to (found by reading the code):** Harmor-class
  resynthesis, string modelling, a 303 with a sequencer, a large-preset ROMpler (FLEX), a
  dedicated granular/scratch instrument, speech; multiband delay, Soundgoodizer, Low
  Lifter, EQUO, an exciter, Scratcher; Note Repeat, a progression generator and Chord
  Stamp, Loop Starter. All in BACKLOG §6l.
- **Cantus has, FL does not:** a modelled grand, multi-mic acoustic drums, a modelled organ
  and electric piano, a de-esser, a resonance suppressor, real-time repair, a reference
  comparator, a three-model bus compressor, an MCP agent interface.

## 6. Not verified

The 2026 Audio Logger's edition; FL Cloud's 2026 tiers and prices; whether each legacy
plug-in still shows in the default picker. Image-Line's table has small slips (MiniSynth's
description is Kepler Exo's; "Fruity Kick" for Fruit Kick; Speech Synthesizer marked
Windows-only though 2026 mentions macOS).

## Sources

- https://www.image-line.com/fl-studio/compare-editions
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/Index_Frame_Left.html
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/effects_plugins.htm
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/generator_plugins.htm
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_new.htm
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_new_25.htm
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/WhatsNew.htm
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/FL%20Studio%20Mobile%20Rack.htm
- https://image-line.com/blog/fl-studio-2024-is-here
- https://www.soundonsound.com/news/fl-studio-212-now-available
- https://alternativeto.net/news/2026/7/fl-studio-2026-brings-rebuilt-flex-fl-cloud-project-storage-and-new-transmitter-plugin/
