# Tonus — real-time pitch correction

*Effect kind `AutoTune`, stem `tonus` (round 28, ADR 0053). References: Antares Auto-Tune
(Auto mode), Waves Tune Real-Time, Logic's Pitch Correction, FL Studio's Pitcher,
MeldaProduction MAutoPitch and Auburn Graillon.*

Tonus hears a monophonic voice, decides which note it is singing, and moves it to a target
note — chosen from a **key and scale**, from **any set of notes** on a one-octave keyboard,
or from the **notes played on MIDI** — with a retune speed from the hard, stepped effect
(0 ms) to a slow, natural pull. The voice keeps its **formants** (its vowels and its
timbre) whatever the shift; a **throat** control moves them on purpose. Its latency is
**128 samples at 48 kHz (2.7 ms), the same at every setting**, reported to delay
compensation, so a singer can monitor through it while recording.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/tonus.rs` |
| The pitch base | `crates/cantus-dsp/src/pitch*.rs` (tracker), `crates/cantus-dsp/src/psola*.rs` (shifter) |
| DSP | `crates/cantus-dsp/src/tonus.rs`, `tonus_scale.rs` (tests in `tonus_tests.rs`) |
| Engine glue | `crates/cantus-engine/src/fx/tonus.rs` (telemetry layout documented there) |
| Editor | `crates/cantus-ui/src/views/fx/tonus.rs`, `tonus_graph.rs`, `tonus_keys.rs` |
| Demo project | `crates/cantus-ui/src/views/fx/tonus_demo.rs`, `examples/tonus_demo.rs` |
| Strings | `crates/cantus-ui/locales/fx/tonus.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_tonus.rs`, `crates/cantus-ui/tests/fx_tonus.rs`, `crates/cantus-plugins/tests/notes.rs` |

## Parameters (keys are forever)

| Key | Range | Default | Meaning |
|---|---|---|---|
| `src` | 0 … 2 | Scale | where the targets come from: **Scale** (the key, the scale and the keyboard), **MIDI notes** (the pitch classes held on the slot's MIDI source, any octave: the nearest is the target), **MIDI melody** (the newest note held, exactly: the voice sings the keyboard's melody) |
| `key` | C … B | C | the key of the scale (the project has no key of its own) |
| `scale` | 0 … 12 | Chromatic | chromatic, major, minor, harmonic minor, melodic minor, dorian, phrygian, lydian, mixolydian, locrian, major pentatonic, minor pentatonic, blues |
| `speed` | 0 … 400 ms | 20 | the retune speed: the time to cover 95 % of a correction; 0 is the hard effect |
| `amount` | 0 … 100 % | 100 | how much of the correction is applied |
| `human` | 0 … 100 % | 0 | humanise: a note held past 120 ms slows its speed by up to 400 ms × this over the next 300 ms — short notes snap, long notes keep their movement |
| `flex` | 0 … 100 cents | 0 | tolerance: a voice nearer its target than half this is left alone, one beyond it is corrected fully, a smooth step between |
| `vib` | −100 … +100 % | 0 | the singer's own vibrato, as much of it as the speed lets through: −100 % removes it, +100 % doubles it |
| `vdepth`, `vrate` | 0 … 100 cents, 2 … 10 Hz | 0, 5 Hz | a created vibrato, starting at each note and fading in over 250 ms |
| `formant` | Follow / Keep | Keep | keep the formants where they are, or let them move with the pitch (the "chipmunk" sound) |
| `throat` | −12 … +12 st | 0 | the formant shift: up is a shorter throat (brighter, younger), down a longer one |
| `trans` | −24 … +24 st | 0 | a transposition, in whole semitones |
| `a4` | 415 … 466 Hz | 440 | the reference pitch |
| `n0` … `n11` | 0 … 3 | As the scale | each pitch class C … B: as the scale says, **on** (a target), **removed** (never a target), **bypass** (a voice nearest it is left alone) |

## How it works

1. **Hearing the voice** — the pitch base's streaming tracker (`cantus_dsp::pitch::
   PitchTracker`): YIN's cumulative mean normalised difference on the mono sum decimated
   to about 12 kHz, refined at the full rate to a fraction of a cent, every 2.7 ms. A
   frame is voiced when it is periodic, loud enough and not sibilant (the de-esser's cue:
   the share of energy above the voice's band), with hysteresis, so breaths and "s"
   sounds never flip it. The lowest f0 tracked is 55 Hz (A1).
2. **Choosing the target** — the allowed note nearest the sung pitch; a new note must be
   0.12 semitones nearer than the current one before the target changes (no flapping on
   a boundary). A MIDI note is applied **at its own sample**, whatever the block.
3. **The correction** — `T − p` (zero inside the flex tolerance), followed by a one-pole
   glide at the retune speed (humanise slows it on long notes); plus the vibrato terms and
   the transposition; glided over 2 ms so the tracker's 2.7 ms steps never show.
4. **Moving the voice** — the pitch base's streaming shifter (`cantus_dsp::psola::
   Shifter`): pitch-synchronous overlap-add whose synthesis marks are spaced by the
   **target period exactly**, so at speed 0 the output is the target, with no warble,
   whatever the voice does. Each grain is chosen on a lattice one period apart and aligned
   to the period before it by a causal cross-correlation, so overlapping grains are always
   on the same phase. Grains read faster or slower move the formants (*throat*) without
   touching the pitch.
5. **Where nothing is corrected** (unvoiced, a bypass note, no MIDI note held, amount 0)
   the output is the input delayed by the latency, **bit for bit**; engaging and releasing
   cross-fade over 5 ms.

## The editor

- **The pitch display**: the last three seconds of what was sung (dim) and what comes out
  (the hue), over note lanes — targets tinted, the current target lit, bypassed notes
  dashed, removed ones plain. The view follows the voice.
- **The detected note**: the sung note in large type, its distance from the nearest note
  in cents (red beyond 15), the target, the shift applied and a meter centred at zero
  (±100 cents).
- **The front row**: the target source (Scale / MIDI notes / MIDI melody); with a scale,
  the key and the scale as menus; with MIDI, in Cantus, the track whose notes it hears
  (its clips and what is played on it live); the speed (labelled *Hard* at 0) and the
  amount.
- **The keyboard**: a dot for a target, a cross for a removed note, a ring for a bypassed
  one. Click: target ↔ removed; right-click: bypass on/off; *Reset notes* back to the
  scale. With MIDI it lights the notes held.
- **More**: humanise, flex, the singer's vibrato, the created vibrato, the formants, the
  throat, the transposition and the reference pitch.

Every knob turn and click is one undo step. Factory presets: *Natural*, *Tight*, *Hard
tune*, *Hard tune with a created vibrato*, *Throat up*, *Throat down*, *MIDI lead*,
*Major key, soft touch*, *An octave down, formants kept*.

## Measured (round 28)

| What | Measured | Target |
|---|---|---|
| Latency at 48 kHz | 128 samples (2.67 ms), constant | ≤ 144 (3 ms) |
| Hard tune (speed 0) on steady notes 30–35 cents off | within 0.1 cent of the target | no warble |
| Hard tune on a note with ±40 cents of vibrato | mean 2.3, worst 3.8 cents from the target | below 5 cents (an ear's threshold for a wobble) |
| Natural speed (60 ms) | the note's centre within 5 cents after 0.3 s; vibrato kept (> 40 cents swing) | natural |
| A legato note change (the target jumps a whole tone while the voice sounds) | the output's largest second difference around the change 1.02 × a steady voice's at 60 ms, 0.98 × at 0 ms | no click |
| MIDI targets | the target changes on the note's own sample at block sizes 1, 37, 64, 512, 4096; the output is the same bits at every block size | sample-accurate |
| Monitoring | the monitored voice is late by exactly 128 samples when nothing else in the project has latency | Tonus's latency only |
| Formants kept at ±3–5 semitones | 0.9–1.6 dB envelope distance (8–10 dB when they follow the pitch) | preserved |

## Inside the exported plug-ins

`Cantus Tonus` (CLAP `com.github.lalmazan.cantus.tonus`, VST3 *Fx|Pitch Shift*) is the same
DSP, bit for bit. It has a **note input** — a CLAP note port (CLAP and MIDI dialects) and a
VST3 event bus — so another DAW's MIDI track drives *MIDI notes* and *MIDI melody* exactly
as Cantus does, at the note's sample. Porta's MIDI trigger has the same input.

## Monitoring through Tonus

A singer monitoring through Tonus hears their voice 2.7 ms after the audio interface
delivers it (plus the interface's own round trip). Delay compensation aligns every strip
to the largest latency in the project, so a long-latency plug-in elsewhere (a
linear-phase EQ: 96 ms) delays the monitored voice too; for recording, keep such plug-ins
off, or use the recording mode of BACKLOG §4c when it lands.
