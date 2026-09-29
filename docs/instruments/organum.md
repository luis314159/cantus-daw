# Organum — the tonewheel organ and its rotary speaker

`InstrumentKind::Organ` · round 17 · ADR 0023 · research: `research/instruments/keys.md` (§4.3,
§8 "Rota" and "Vertigo")

Organum is a tonewheel organ built the way the instrument is: 91 wheels that never stop, keys
that close contacts onto them, drawbars that set how much of each bus bar reaches the
output, a single-trigger percussion, a scanner vibrato, a preamp, and a rotary speaker with
a horn and a drum. It is not voice-based: its cost does not depend on how many keys are
held.

| Layer | File |
|---|---|
| model | `crates/cantus-core/src/inst/organ.rs` |
| DSP | `crates/cantus-dsp/src/organum.rs` (+ `organum_wheels.rs`, `organum_scanner.rs`, `organum_rotary.rs`, `organum_filters.rs`, `organum_tests.rs`) |
| engine | `crates/cantus-engine/src/inst/organ.rs`, tests `crates/cantus-engine/tests/inst_organ.rs` |
| editor | `crates/cantus-ui/src/views/inst/organ.rs` (+ `organ_drawbars.rs`, `organ_cabinet.rs`), tests `crates/cantus-ui/tests/inst_organ.rs` |
| strings | `crates/cantus-ui/locales/inst/organ.{en,es}.txt` |

## What was built

**The tone generator** (`organum_wheels.rs`). 91 wheels, each a 32-bit phase accumulator
reading a 2048-point sine table (interpolation error below −118 dB). A wheel's pitch is the
motor's 20 turns a second × its gear ratio × its teeth: the twelve published gear pairs, teeth
doubling each octave, and the top seven wheels (192 teeth) borrowing the gears of F…B. So the
organ is within 0.71 cent of equal temperament below C8 and up to +2 cents in the top seven —
the instrument's own small, fixed mistuning (`tuning` = Gear 60 Hz), or exact equal temperament
(`tuning` = Equal). `a4` scales either. The lowest octave's wheels carry a third harmonic at
−22 dB (our estimate: the instrument's are not pure sines).

**Keys and contacts** (`organum.rs`). Each key closes nine contacts (16′ 5⅓′ 8′ 4′ 2⅔′ 2′ 1⅗′
1⅓′ 1′ — harmonics ½, 1½, 1, 2, 3, 4, 5, 6, 8), folding back an octave at either end of the
generator. What sounds is a gain per wheel per output (upper, lower, pedal, percussion bus):
`91 × 4` multiply-adds a sample, rebuilt from the key state when a key settles or a drawbar
moves (drawbar moves glide over 5 ms). Pedal drawbars are mixtures (16′ with a little quint,
8′ with a little 4′ and 2⅔′; our voicing).

**Key click** is modelled, not sampled: each of a key's contacts touches at its own moment
(spread over 1.5 ms) and bounces 0–3 times (0.05–0.5 ms closed, 0.03–0.3 ms open), all seeded
from the note's id, key and division. The click is the running sines switched on that way;
`click` scales between a clean smoothstep contact (−60 dB) and the raw bouncing one (0 dB, up
to +6 dB exaggerated). The release has its own, gentler bounce (`click.rel`).

**Leakage**: a contact on a wheel also picks up the wheels of its compartment (same pitch class
two, four and six octaves away, at 1, ½ and ¼ of the `leak` level — our model of the
compartment crosstalk), and a drawbar pushed in still passes `leak.bar`.

**Percussion**: the 2nd (4′) or 3rd (2⅔′) harmonic of the upper manual's keys through one
envelope — 1 ms attack, exponential decay to −60 dB in `perc.fast` or `perc.slow` —
**single-trigger** (it re-arms only when every upper key is up, so legato does not retrigger);
it takes the upper 1′ drawbar (`perc.steal`), and at normal volume drops the upper manual by
`perc.drop`.

**Vibrato and chorus** (`organum_scanner.rs`): a delay line tapped every 0.11 ms, a scanner
sweeping the first 3, 6 or 10 taps (V1 0.22 ms, V2 0.55 ms, V3 0.99 ms — the documented
1 : 2.5 : 4.5) up and back once a revolution, crossfading between adjacent taps (the stepped
reading that is the instrument's sound), a low-pass closing with depth for the LC line; C1–C3
mix it with the dry signal at equal power. Upper and lower each switch through it.

**Swell and preamp**: the expression pedal from −36 dB to unity with a low-shelf bass
compensation growing to +9 dB at 150 Hz as it closes; then an asymmetric tube-like clipper
(`tanh(x + 0.2) − tanh(0.2)`) with first-order antiderivative anti-aliasing, driven up to
+30 dB.

**The rotary speaker** (`organum_rotary.rs`, standalone so it can become an insert):
Linkwitz–Riley 4th-order crossover; the horn a Doppler read of a delay line at the travel time
to each of two microphones (4-point Hermite), with an angle-dependent gain (−15 dB facing away)
and low-pass (12 → 3 kHz), plus its image in the back wall and a 15 % leak; the drum a rotating
baffle — amplitude and filter modulation with a small Doppler; speeds as first-order lags with
their own up and down time constants per rotor, angles integrated in f64 per sample; brake
parks the rotors at `rot.brake`; closed or open cabinet. Model after Smith, Serafin, Abel &
Berners (DAFx-02), from the paper; defaults from setBfree's documentation (never its code).

**Splits on one track**: `split.pedal` and `split.upper` divide the keyboard into pedal,
lower and upper zones; MIDI channel 2 plays the lower manual and channel 3 the pedals
whatever the split.

**Controllers**: the mod wheel past half way turns the rotors fast; expression (CC 11) is the
swell. Macros, mod wheel, expression, aftertouch and sustain are modulation sources; any
parameter can be a target (the fixture's `macro1 → gain` included: macro 0.5 × depth 0.25 × the
gain's 60 dB range = +7.5 dB, tested to 0.01 dB).

**The editor**: drawbars you drag (footage under each, the registration written out as
`88 8000 000`, the step's dB on hover, the organist's colour code); the cabinet from above with
the horn and drum at the angles the engine reports (they turn at their real speed), the
microphones draggable (spread and distance) and the brake angle draggable; the spin-up curve for
the change under way with τ marked and where the rotors are now; the scanner's taps with the
pickup moving; the percussion decay with its envelope now; the split map with draggable
boundaries. On the kit's device shell (ADR 0024, round 18) every section is a card in its
function hue — drawbars and splits tone, percussion dynamics, vibrato modulation, the rotary
space, levels output — with value-pill cells, segmented switches (rotor speed, 2nd/3rd,
Fast/Slow, Normal/Soft, V1–C3, tuning, cabinet), LED buttons (Upper, Lower, Takes the 1′) and
power LEDs in the percussion and rotary titles. Play: the upper drawbars beside the cabinet,
percussion and vibrato with their displays, and nine cells; Edit: all three drawbar sets,
splits, percussion, vibrato, tone, preamp and the rotary speaker with cabinet and spin-up.
Eleven factory presets, each naming its four macros (Drive, Fast, Click, Brilliance).

## Parameters

| Key | What | Range | Default | Unit |
|---|---|---|---|---|
| `u1`…`u9` | upper drawbars 16′…1′ | 0–8 | 8 8 8 0 0 0 0 0 0 | step (3 dB) |
| `l1`…`l9` | lower drawbars | 0–8 | 0 0 8 8 0 0 0 0 0 | step |
| `p1` `p2` | pedal 16′, 8′ | 0–8 | 8, 4 | step |
| `split.upper` | first key of the upper manual (0 none) | 0–127 | 0 | key |
| `split.pedal` | first key above the pedals (0 none) | 0–127 | 0 | key |
| `perc` | percussion | off/on | off | |
| `perc.harm` | 2nd / 3rd | | 3rd | |
| `perc.decay` | fast / slow | | fast | |
| `perc.vol` | normal / soft | | normal | |
| `perc.fast` `perc.slow` | decay to −60 dB | 50–3000, 200–8000 | 1000, 4000 | ms |
| `perc.soft` | soft level | −12…0 | −6 | dB |
| `perc.drop` | normal-volume drop on the upper manual | −6…0 | −3 | dB |
| `perc.steal` | takes the 1′ | off/on | on | |
| `vib` | V1 V2 V3 C1 C2 C3 | | C3 | |
| `vib.upper` `vib.lower` | through the scanner | off/on | on, off | |
| `vib.rate` | scanner | 4–9 | 7.0 | Hz |
| `click` `click.rel` | key click, release click | −60…+6 | −6, −12 | dB |
| `leak` | wheel crosstalk | −80…−20 | −40 | dB |
| `leak.bar` | drawbar leak | −80…−20 | −60 | dB |
| `bright` | brilliance: dB at the top wheel vs 523 Hz | −6…+6 | 0 | dB |
| `tuning` | Gear 60 Hz / Equal | | Gear | |
| `a4` | reference | 415–466 | 440 | Hz |
| `swell` | expression | 0–100 | 100 | % |
| `drive` | preamp | 0–100 | 20 | % |
| `lvl.upper` `lvl.lower` `lvl.pedal` | division levels | −40…+6 | 0, −3, −3 | dB |
| `rot.out` | rotary / direct | | rotary | |
| `rot.speed` | stop / slow / fast | | slow | |
| `rot.horn_slow` `rot.horn_fast` | horn speeds | 20–80, 300–480 | 40, 420 | rpm |
| `rot.drum_slow` `rot.drum_fast` | drum speeds | 20–80, 250–450 | 36, 357 | rpm |
| `rot.horn_up` `rot.horn_down` | horn time constants | 0.05–2, 0.05–3 | 0.16, 0.32 | s |
| `rot.drum_up` `rot.drum_down` | drum time constants | 0.5–10, 0.5–6 | 4.1, 1.4 | s |
| `rot.balance` | horn against drum | −12…+12 | 0 | dB |
| `rot.xover` | crossover | 500–1200 | 800 | Hz |
| `rot.cab` | closed / open | | closed | |
| `rot.dist` | microphones from the cabinet | 10–200 | 60 | cm |
| `rot.spread` | angle between them | 0–180 | 120 | ° |
| `rot.brake` | where the brake parks the rotors | 0–359 | 90 | ° |
| `gain` | output | −48…+12 | 0 | dB |

## What the tests hold

DSP (`organum_tests.rs`, `organum_wheels.rs`, `organum_filters.rs`):

- the gear table lands on the published errors (every class of the lowest octave within
  0.006 ct of the table; A exact; wheels 85 and 91 at 4189.09 and 5924.57 Hz), and the phase
  increments keep every wheel within 0.01 ct of its gear pitch at 44.1, 48 and 96 kHz;
- rendered pitch, gear and equal, on ten keys: within 0.05 ct;
- drawbar steps are 3.0 dB (±0.1); crosstalk measures at the leakage setting (±0.5 dB, at −40
  and −30 dB);
- percussion is single-trigger (legato does not retrigger, a new key after all are up does)
  and takes the 1′;
- the scanner's pitch modulation peaks at its rate (7 Hz ±0.15) with a V3 depth of tens of
  cents;
- the rotors turn at their rpm, reach 63.2 % (±5 %) of a speed change after one time
  constant, and the brake parks both within 2° of the angle (0°, 90°, 237°);
- the horn's amplitude modulation has the rotor's period within 1 % (fast and slow);
- a performance with splits, percussion, legato, a speed change and a drawbar move is
  identical at blocks of 1, 7, 64, 256, 512 and 4096; exact silence once the notes are over;
  the key click is seeded (same note, same click; another note, another) and scales with its
  level; splits route keys and channels.

Engine (`crates/cantus-engine/tests/inst_organ.rs`, plus the family's `inst_contract.rs`):
block-size independence and determinism at tolerance 0 through the renderer; every value at
its default changes no sample; pitch across the keyboard; the preamp's alias floor; the
fixture's macro connection; the mod wheel spinning the rotors up and expression as the
swell; headroom of a full organ at full drive; CPU flat in the number of keys held.

Editor (`crates/cantus-ui/tests/inst_organ.rs`): opens in Play, Edit and Matrix and paints its
name, footages, registration and rotor speeds; the rotary prints the speeds the engine reports;
a drawbar dragged, a cell turned, a switch and a lamp clicked are one undo step each; a
modulation source dropped on a cell connects it; no text is elided or overlaps at 75 %, 100 %,
200 % or in Spanish; the files use kit parts and tokens only; every preset sets only real keys, names its macros, and is
one undo step.

## Measured

Measured on this machine (shared by about a dozen builders; dev profile, the DSP and
engine crates at opt-level 3), 48 kHz:

| What | Measured |
|---|---|
| Gear tuning against equal temperament | within 0.71 ct below C8 (G♯ −0.71, C♯ and F −0.68, C −0.58); the top seven wheels +1.2…+2.0 ct |
| Phase increments against the gear table | every wheel within 0.01 ct at 44.1, 48 and 96 kHz |
| Rendered pitch through the engine, C2…C7 | worst 0.0003 ct from the gear pitch |
| Drawbar step | 3.0 dB (±0.1) |
| Crosstalk | at the leakage setting ±0.5 dB (−40 and −30 dB) |
| Rotor speeds, steady | horn 40 / 420 rpm, drum 36 / 357 rpm (within 0.001 rpm) |
| Spin-up after one time constant | 63.21 % of the way (horn τ 0.16 s, drum τ 4.1 s) |
| Horn amplitude-modulation period | 0.1428 s for 0.1429 s at fast (−0.03 %), 1.4999 s for 1.5000 s at slow |
| Brake | parks both rotors within 0.001° of 0°, 90° and 237° |
| Mod wheel to fast, one second later | horn above 400 rpm, drum 90–200 rpm (on its 4.1 s way) |
| CPU, 10 s of audio, 256-frame blocks (best of five 2 s runs) | 1 key 0.541 s · **20 keys 0.504 s (5.0 % of one core)** · all 154 keys 0.557 s — flat |
| Preamp alias floor, C7 at full drive | −73.6 dB against the fundamental |
| Headroom: ten keys, all nine drawbars, full drive, percussion, upper +6 dB | peak −10.9 dBFS |
| The fixture's `macro1 → gain` (0.5 × 0.25 × 60 dB) | +7.50 dB (±0.01) |

## Not done

- **Arrive on the beat** (a speed change scheduled so the drum reaches speed on the bar line)
  and **tempo-locked rotors** need the timeline; the instrument contract gives a unit no
  transport or look-ahead. The rotor model is ready for it (the spin-up is a closed-form lag).
- **Vertigo as an insert effect**: the rotary is a standalone module with a stereo `process`
  and a mix, but registering it as an `EffectKind` touches the shared effect dispatch — left for
  the coordinator.
- **Gear 50 Hz** and **project-wide tuning** (a temperament from the project): the 50 Hz gear
  table is not documented in the sources we could read; project tuning waits for the host.
- The pedal drawbars' mixtures, the lowest wheels' harmonic, the compartment crosstalk pattern
  and the scanner's line are our models, calibrated to the documented numbers where there are
  numbers, not measured on an instrument.
- A Drawbar Modulator (B-3 V2's idea) exists only as the general matrix: macros and controllers
  can move any drawbar; there are no LFOs in the organ.
