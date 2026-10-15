# Chorda — the modelled grand piano

`InstrumentKind::Chorda` · round 28 · ADR 0059 · BACKLOG §3e · the decision to build it:
`docs/round28/sound.md` §B

Chorda plays a grand piano from its physics: every key's one, two or three strings struck by
a felt hammer, meeting at a yielding bridge, ringing into a soundboard, the other keys'
strings ringing with them when their dampers let them. No sample excites anything; every
number comes from published measurements (ADR 0059 §1).

| Layer | File |
|---|---|
| model | `crates/cantus-core/src/inst/chorda.rs` |
| DSP | `crates/cantus-dsp/src/chorda.rs` (+ `chorda_tables.rs`, `chorda_design.rs`, `chorda_string.rs`, `chorda_voice.rs`, `chorda_board.rs`, `chorda_engine.rs`; tests `chorda_tests_*.rs`, `chorda_testkit.rs`) |
| engine | `crates/cantus-engine/src/inst/chorda.rs` (the unit, the design registry), tests `crates/cantus-engine/tests/inst_chorda.rs` |
| editor | `crates/cantus-ui/src/views/inst/chorda.rs` (+ `chorda_draw.rs`), tests `crates/cantus-ui/tests/inst_chorda.rs` |
| strings | `crates/cantus-ui/locales/inst/chorda.{en,es}.txt` |

## What was built

**The keyboard** (`chorda_tables.rs`): per key the stretched tuning (Rigaud, David & Daudet's
aural-tuning model: A0 −24 ct, C8 +35 ct), the inharmonicity `B`, one string to F♯1, two to
F2, three above, the speaking length, wave speed and impedance (700 N), the decay law
`σ_k = b₁ + b₂ (kπ/L)²` (Bensa et al. 2003 at C2, C4, C7), the hammer's mass, stiffness `K`,
exponent `p` and strike point (Chaigne & Askenfelt 1994), and its damper (none above E6).

**The strings** (`chorda_design.rs`, `chorda_string.rs`): waveguides split at the strike
point; stiffness as 1–22 second-order allpass sections (Abel, Välimäki & Smith 2010) refined by
Levenberg–Marquardt to put every partial below 5 kHz within a cent of `n F₀ √(1 + B n²)`; a
linear-phase loss filter that sets each partial's decay without moving its pitch; the
unisons detuned by a hashed spread and coupled at a resistive bridge (Weinreich's two-stage
decay); phantom partials from the tension's modulation.

**The hammer**: a mass on a hysteretic felt (Stulov: `ε` the felt's memory, `τ` 10 µs) solved
against the strings' impedance with four Newton steps a sample; *hardness* scales `K` by up to
4× either way and moves `p` by ±0.3; the velocity maps to 0.35–5.5 m/s (*dynamics* sets the
span, *velocity curve* its shape). The soft pedal shifts the next strikes onto one string
fewer and softer felt.

**Dampers and pedals** (`chorda_voice.rs`): a released key's damper grips as far as the
sustain pedal leaves it — `1 − smoothstep` over 15–65 % of the travel, the half-pedal zone —
gliding over 8 ms. Controller 64 is the sustain pedal (continuous), 67 the soft pedal.

**The post-voice stage** (`chorda_board.rs`): the keys' own strings ringing in sympathy (at
their speaking pitches, a reduced design each), and the soundboard — 40 modes at a quarter of
the rate, a four-line FDN for the high band, the direct radiation and a 20 dB radiation rise
between 120 Hz and 1.2 kHz.

**The instrument** (`chorda_engine.rs`): 88 voices, one per key; their lanes rendered across
the pool and summed in key order; every clock counted from the stream (ADR 0059 §5).

**The unit** (`inst/chorda.rs`, engine): the designs come from a lock-free registry, computed
on threads of their own — the reference keyboard at start-up and after a device change, a
project's tunings when its units are built; an offline render waits for its own.

**The editor**: *Play* — the piano from above (every key's strings at their length, the bridge,
the strike line, the dampers, the keyboard; the last key played lit, its hammer shifted by the
soft pedal) beside the pedals (the half-pedal law with the pedal on it), and the hammer,
resonance and output cells. *Edit* — the strings, the tuning's stretch in cents, the felt's
force against compression (as measured and as set), the strings' ring key by key (free and
damped), the pedals, resonance and output. Every cell takes a modulation source by drag and
drop and is one undo step.

## Parameters

Shares are fractions shown in %; 100 % is the model as measured.

| Key | Range (default) | What it does |
|---|---|---|
| `model` | 1 | The model a song was made with; a later one never changes it |
| `a4` | 415–466 Hz (440) | A4's first partial; designs the strings again |
| `stretch` | 0–200 % (100) | The octaves' widening an aural tuner gives; designs again |
| `stiffness` | 50–150 % (100) | The strings' inharmonicity `B`; designs again |
| `hardness` | −100–100 % (0) | The felt, softer or harder |
| `hysteresis` | 0–98 % (90) | The felt's memory `ε` |
| `dynamics` | 50–150 % (100) | The span of hammer speeds the velocities reach |
| `vel.curve` | 0.4–2.5 (1) | The velocity's curve |
| `decay` | 25–400 % (100) | Every partial's ring against the measured |
| `unison` | 0–8 ct (1) | The unisons' spread |
| `coupling` | 0–90 % (50) | The bridge's share of the first partial's loss (the two-stage decay) |
| `sympathy` | 0–200 % (100) | How strongly the other strings ring along |
| `phantom` | 0–300 % (100) | The phantom partials' level |
| `damper` | 0–100 % (100) | How firmly the dampers grip |
| `una` | 0–100 % (100) | How far the soft pedal shifts the hammers |
| `body` | 0–200 % (100) | The soundboard's ring |
| `tilt` | ±12 dB (0) | The top raised or lowered |
| `width` | 0–100 % (70) | The keyboard's spread across the stereo field |
| `gain` | −48–12 dB (0) | The output |

The modulation matrix, the eight macros and the sources (velocity, key, mod wheel,
aftertouch, expression, sustain) work as on every native instrument; the three tuning keys
are never modulated.

**Presets** (each with macros Hardness, Resonance, Body, Decay): Concert Grand, Studio Bright,
Warm Ballad, Jazz Club, Felt Piano, Honky-Tonk, Baroque Pitch (A 415), Pedal Wash, Pop
Forward, Old Grand.

## What the tests hold

ADR 0059's table: tuning within a cent (designed and measured on the sound), T60 against
Bensa et al., the continuous half pedal and re-pedalling, the two-stage decay, una corda,
sympathy, the cost of 64 pedal-down voices, the family's rules through the contract, offline
equal to real time, headroom, and the editor's views, gestures and texts.

## Measured

| What | Value |
|---|---|
| Worst partial against the stretched target, designed (88 keys) | 0.47 cents at 44.1 kHz, 0.49 at 48 kHz |
| Worst partial measured on the rendered sound (A0 … C8, sampled keys) | 0.40 cents |
| T60 against Bensa et al. (C2, C4, C7, first partials) | 0.96–1.01 of the measured |
| Prompt sound / aftersound, middle C held | 24.6 / 8.7 dB/s |
| Half pedal: decay after release, pedal 0 → 100 % (dampers alone) | 241 → 41 dB/s, monotonic |
| Sympathy with the pedal / without (energy against the dry note) | −17 dB / −32 dB |
| 64 pedal-down voices, 256 frames, one thread, release | 18–19 % of a core |
| Both hands fortissimo with the pedal / mezzo-forte middle C | −3.3 / −22.7 dBFS |
| A keyboard's design (88 + 88 reduced), eight threads | about two seconds |

## Not done (the owner's)

- The blind listening test (≥ 20 listeners, Pianoteq 9 and a sampled grand).
- The legal review of the structure against the patents ADR 0059 names.
- The presets' last word by ear.
