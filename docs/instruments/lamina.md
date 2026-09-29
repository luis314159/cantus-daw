# Lamina — the modelled electric piano

`InstrumentKind::ElectricPiano` · round 17 · ADR 0023 · research: `research/instruments/keys.md`
(§4.2, §8 "Lamina")

Lamina models the two classic electric pianos from their parts: a hammer, a vibrating
cantilever (a tine with its tone bar, or a reed with its solder tip), and the pickup that
hears the tip — electromagnetic or electrostatic, the nonlinearity that makes the bark. Every
geometric value is in millimetres, drawn at true scale in the editor.

| Layer | File |
|---|---|
| model | `crates/cantus-core/src/inst/epiano.rs` |
| DSP | `crates/cantus-dsp/src/lamina.rs` (+ `lamina_voice.rs`, `lamina_geometry.rs`, `lamina_tests.rs`) |
| engine | `crates/cantus-engine/src/inst/epiano.rs`, tests `crates/cantus-engine/tests/inst_epiano.rs` |
| editor | `crates/cantus-ui/src/views/inst/epiano.rs` (+ `epiano_pickup.rs`, `epiano_keys.rs`), tests `crates/cantus-ui/tests/inst_epiano.rs` |
| strings | `crates/cantus-ui/locales/inst/epiano.{en,es}.txt` |

## What was built

**Geometry in millimetres** (`lamina_geometry.rs`). The tine is a clamped-free steel
cantilever, 1.5 mm across: the length that sounds a pitch follows from
`f₁ = (β₁²/2πL²)·c·r/2` — 64 mm at C4, 197 mm at A0, 16 mm at C8. Its upper modes sit at the
beam's 6.267 and 17.547 times the fundamental; how strongly the hammer excites each comes from
the textbook mode shapes at the strike point (`strike`, a fraction of the length from the
clamp). The reed's modes sit at 7.1 and 20.4 (tip mass; our estimate), its length an empirical
18–60 mm fit.

**The voice** (`lamina_voice.rs`): up to five modes as complex one-pole resonators (so a decay
can change every sample without a click) — for the tine, its fundamental split between the tine
(a quick 35 % decay) and the tone bar that extends the sustain (`tine.couple`), its two upper
modes, and the tone bar's own 2.756× mode ringing in the attack (the "bell", level and decay
set by `tine.bell`, `tine.bell_decay`); for the reed, its fundamental and two overtones.

**The hammer** is a mass on a power-law spring (`p = 2.5`) against the near-rigid point by the
clamp: its force pulse has one shape (computed once by RK4), its length set by the tip's
hardness (3.2 ms soft to 0.5 ms hard at full velocity) and lengthened for a slower hammer by
the spring's own scaling `v^(−(p−1)/(p+1))` — so brightness rises with velocity by itself. A
tine moving back faster than the hammer throws it off: no contact outlasts half a period.
Loudness is set exactly (the fundamental swings `swing_ff · 10^(−dynamics·(1−v)/20)` mm, the
pulse's own response at the fundamental measured at the strike to scale it), so `dynamics` is
the range in dB; the physics decides the colour.

**The pickups** read the tip's displacement `x` against a pole (or slot) `gap` mm away and
`offset` mm off the rest line: flux `(1 + ((x − a)/d)²)^(−3/2)` for the tine, capacitance
`(1 + ((x − a)/d)²)^(−1/2)` for the reed; the output is the curve's rate of change, computed as
a difference in double precision (a quiet note's swing would drown in single precision's
rounding). Centred, the curve is even and the note jumps an octave; off centre it is mostly the
fundamental, with an asymmetric distortion that grows as a hard strike swings the tip into the
steep part — the bark. Past the curve's inflection (half the gap off centre) the even harmonics
return, as on the instrument.

**Dampers and the pedal**: releasing a key moves its damper onto the tine over 8 ms; each mode's
decay moves between free and damped (`damper.time`, upper modes faster with `damper.tone`). The
sustain pedal is continuous: the shared allocator (`cantus_dsp::voices`) keeps the bookkeeping,
and every released voice's damper rests at `engagement(pedal)` — fully on below the lift point's
zone, fully off above it, partly on across the 20 % of travel around `pedal.lift`:
half-pedalling. The felt landing on a moving tine thumps (`release.noise`, growing with the
swing it stops and the release velocity); lifting it off brushes (`damper.noise`); the hammer
knocks (`hammer.noise`). All seeded from the note's id.

**Voices**: 8–64 from the shared allocator; a stolen voice fades out over 3 ms before its new
strike; a key struck again while it rings is struck **on the moving tine** (the new voice takes
the old one's state). Per-key spread (cents), upper-mode detune, decay and pickup misalignment
come from the key, scaled by `age`, so the instrument is the same instrument every time.

**Keyboard curves**: hardness (±), decay (×) and level (dB) each have eight anchors, C1…C8,
interpolated per key.

**Global**: bass and treble shelves, the preamp (the same ADAA clipper as Organum's), an optional
small-speaker voicing (`output`), the tremolo — stereo pan for the tine, amplitude for the reed,
or either — and the output gain. Pitch bend bends every ringing tine; expression is a volume
pedal before the preamp.

**The editor**: the pickup's transfer curve across ±3 mm of tip travel with the stretch the tip
actually swings through lit from the engine's report, and the geometry below at true scale —
drag the pole piece sideways for the offset, up and down for the gap; the tine (or reed) from
the side, to scale, for the key last played, with the strike point, the tone bar and the pickup
and a 10 mm scale; the pedal's travel with the half-pedal zone shaded from the actual lift point
(draggable) and where the pedal is now; the key-curve strip over an 88-key keyboard with the
instrument's range lit, anchors dragged. Eleven factory presets (eight tine, three reed), each
naming its four macros (Bark, Drive, Tremolo, Hardness).

On the kit's device shell (ADR 0024, round 18) every section is a card in its function hue —
pickup and tine source, hammer and dampers dynamics, preamp tone, tremolo and key curves
modulation, playing output — with value-pill cells and segmented switches in the card titles
(engine Tine · Reed, output, tremolo mode, keyboard range, which key curve). Play: the pickup
beside the tine and the pedal, and seven cells; Edit: every section, the tremolo's second drawn,
the key curves at the bottom.

## Parameters

| Key | What | Range | Default | Unit |
|---|---|---|---|---|
| `engine` | tine / reed | | tine | |
| `range` | 73 (E1–E7) / 64 (A1–C7) / 88 keys | | 73 | |
| `a4` | reference | 430–450 | 440 | Hz |
| `spread` | per-key detune (seeded from the key) | 0–10 | 2 | ct |
| `age` | spread, noise and misalignment key by key | 0–100 | 20 | % |
| `hard` | hammer tip hardness | 0–100 | 50 | % |
| `hard.vel` | velocity's share of it | 0–100 | 60 | % |
| `force` | level | −24…+6 | 0 | dB |
| `strike` | strike point from the clamp | 2–20 | 8 | % of length |
| `hammer.noise` | | −60…0 | −24 | dB |
| `tine.decay` | fundamental decay at C4 | 0.5–30 | 8 | s |
| `tine.couple` | tone bar's share of the fundamental | 0–100 | 50 | % |
| `tine.bell` | upper modes against the physics | −40…0 | −12 | dB |
| `tine.bell_decay` | | 0.02–2 | 0.25 | s |
| `tine.gap` `tine.align` | pickup gap, offset | 0.3–5, −2…+2 | 1.6, 0.6 | mm |
| `reed.decay` | at C4 | 0.3–10 | 3 | s |
| `reed.overtones` | | −40…0 | −10 | dB |
| `reed.gap` `reed.align` | | 0.3–5, −2…+2 | 1.0, 0.6 | mm |
| `pickup.level` | | −24…+12 | 0 | dB |
| `damper.time` | | 20–1000 | 120 | ms |
| `damper.tone` | upper modes damp faster | 0–100 | 50 | % |
| `damper.noise` `release.noise` | | −60…0 | −30, −36 | dB |
| `pedal.lift` | where the pedal lifts the dampers | 10–90 | 35 | % of travel |
| `drive` | preamp | 0–100 | 20 | % |
| `bass` `treble` | shelves at 150 Hz, 3 kHz | −12…+12 | 0 | dB |
| `output` | direct / speaker | | direct | |
| `trem.mode` | the engine's / pan / amplitude | | the engine's | |
| `trem.rate` | | 0.5–12 | 5.5 | Hz |
| `trem.depth` | | 0–100 | 0 | % |
| `dynamics` | level range across velocity | 20–60 | 40 | dB |
| `vel.curve` | softer ↔ harder to play loud | −100…+100 | 0 | % |
| `polyphony` | | 8–64 | 32 | voices |
| `curve.hard.1…8` | hardness offset at C1…C8 | −50…+50 | 0 | % |
| `curve.decay.1…8` | decay multiplier at C1…C8 | 0.25–4 | 1 | × |
| `curve.level.1…8` | level at C1…C8 | −12…+12 | 0 | dB |
| `gain` | output | −48…+12 | 0 | dB |

## What the tests hold

DSP (`lamina_tests.rs`, `lamina_geometry.rs`): a C4 tine is 64 mm (±2) and lengths fall with
pitch; the mode shapes are zero at the clamp and ±2 at the tip; a centred pickup is even and an
offset one is not; every key sounds its equal-tempered pitch (both engines, within 0.5 ct);
a centred pickup gives a 2nd harmonic more than 20 dB over the fundamental and moving the pole
off centre lowers it monotonically up to the curve's inflection; the bark (harmonics 2–8
against the fundamental) and the level rise with velocity (both engines, three keys, eight
velocities); a faster hammer reaches further up the modes; half-pedalling moves a released
note's decay monotonically between the damper's (0.12 s ±15 %) and the free one; a key struck
while it rings is one tine struck twice; a stolen voice fades instead of clicking; a scenario
with a pedal ramp, a steal, a bend and a re-strike is identical at every block size; exact
silence; the pedal holds released notes and lifting it damps them.

Engine (`crates/cantus-engine/tests/inst_epiano.rs`, plus `inst_contract.rs`): block-size
independence and determinism at tolerance 0 for both engines; every value at its default
changes no sample; pitch across the keyboard; the alias floor of a high note at full velocity
through the pickup and a driven preamp; velocity routed to a parameter moves only the notes it
should; the sustain pedal as a controller, with half-pedalling between up and down; CPU per
voice.

Editor (`crates/cantus-ui/tests/inst_epiano.rs`): opens in Play, Edit and Matrix and paints its
name and its geometry in millimetres; the swing it prints is the engine's; dragging the pole
piece, a key-curve anchor or a cell, and switching the engine, are one undo step each; a
modulation source dropped on a cell connects it; no text is elided or overlaps at 75 %, 100 %,
200 % or in Spanish; every preset sets only real keys,
names its macros, and is one undo step.

## Measured

Measured on this machine (shared by about a dozen builders; dev profile, the DSP and
engine crates at opt-level 3), 48 kHz:

| What | Measured |
|---|---|
| Pitch, A1…A6, both engines | worst 0.010 ct from equal temperament |
| Tine length at C4 | 64 mm (197 mm at A0, 16 mm at C8) |
| Centred pickup | 2nd harmonic more than 20 dB over the fundamental; falls monotonically to 0.8 mm off centre |
| Alias floor, B6 at full velocity, drive 60 % | tine −77.2 dB, reed −78.4 dB against the fundamental |
| Released note, pedal up | T60 0.12 s (±15 %), the damper's time |
| Released note, pedal down | T60 above 3 s; monotonic in between (half-pedalling) |
| Dynamics | the fundamental's swing spans exactly `dynamics` dB across velocity (40 dB tine, 50 dB reed presets) |
| CPU, 10 s of audio, 256-frame blocks, pedal down (best of five 2 s runs) | **20 voices 1.055 s (10.6 % of one core)** · 32 voices 1.606 s |

## Not done

- **Tremolo synced to the tempo** (and a phase locked to the bar): the instrument contract gives
  a unit no tempo or transport; the tremolo's phase is already counted from the start of the
  stream, so it is ready to be locked.
- **2× oversampling of the pickup and preamp**: the alias floor of a B6 at full velocity is
  about −77 dB (see Measured), against the research's −90 dB hint. The voice can evaluate its
  tip at half samples exactly (one more phasor turn per mode) for a shared half-band decimator;
  not built this round.
- **A dedicated pedal lane under the piano roll** and the damper state drawn under notes (P1):
  host features outside this instrument's files.
- The reed's mode ratios and length fit, the tone bar's mode, the swing at full velocity and the
  noises' spectra are estimates calibrated by ear and by the documented behaviour, not
  measurements of an instrument.
- MPE / poly aftertouch reach the unit as note expressions, which Lamina does not read yet.
