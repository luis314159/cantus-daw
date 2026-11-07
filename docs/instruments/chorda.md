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
their speaking pitches — two inversions in each string's loop, as in a voice — a reduced
design each, their dampers gripping as the *dampers* setting says, silenced while their
key's voice plays them), and the soundboard — 40 modes at a quarter of the rate, a
four-line FDN for the high band, the direct radiation and a 20 dB radiation rise between
120 Hz and 1.2 kHz.

**The instrument** (`chorda_engine.rs`): 88 voices, one per key; their lanes rendered across
the pool and summed in key order; every clock counted from the stream (ADR 0059 §5).

**The unit** (`inst/chorda.rs`, engine): the designs come from a lock-free registry that one
designer thread fills — the reference keyboard at start-up and after a device change, a
track's tuning once it has settled (never one per frame of a knob drag). A unit never
waits: made before its keyboard it is silent and its editor shows the design's progress;
an offline render waits for its own. The tuning keys are not automatable.

**The editor**: *Play* — the piano from above (every key's strings at their length, the bridge,
the strike line, the dampers, the keyboard; the last key played lit, its hammer shifted by the
soft pedal) beside the pedals (the half-pedal law with the pedal on it), and the hammer,
resonance and output cells. *Edit* — the strings, the tuning's stretch in cents, the felt's
force against compression (as measured and as set), the strings' ring key by key (free and
damped), the pedals, resonance and output. Every cell but the tuning's (A4, stretch,
stiffness — set, never automated or modulated) takes a modulation source by drag and drop;
each change is one undo step.

## Model 2 (round 37, ADR 0098)

A new Chorda plays its **second model**; every song saved before keeps the first (schema
54 writes `model = 1` into it), to the bit. Model 2 was voiced by comparison with a
recorded grand — the Salamander Grand Piano (Yamaha C5, CC BY 3.0), installed through
Cantus's own pack code — with `cantus-ui/examples/chorda_compare`, because model 1, right in
every physical number, measured thin, bright, dry and almost mono against it (the owner:
*"sintético… sin cuerpo, muy fino"*).

| Layer | File |
|---|---|
| the body: two radiation responses (two microphones over the strings), convolved | `chorda_body.rs`, `chorda_conv.rs` |
| the room: early reflections and an eight-line FDN | `chorda_room.rs` |
| the action: the hammer's blow, the keybed, the dampers landing | `chorda_noise.rs`, `chorda_voice.rs` |
| the voicing (every number the comparison set) | `chorda_voicing.rs` |
| the perspective: mics, pair width, lid, room size — gliding gains and shelves | `chorda_stage.rs`, `chorda_tests_stage.rs` |
| tests: the family's rules for model 2, model 1's pinned hash | `chorda_tests_model2.rs`, `chorda_tests_hash.rs` |
| the comparison and the owner's ABX kit | `cantus-ui/examples/chorda_compare`, `chorda_abx`, `chorda_kit/` |

Model 2 adds five settings, read by it alone: **`room`** (0–100 %, default 35 %: the room's
level), **`noise`** (0–200 %, default 100 %: the action's noises), **`persp`** (0 over the
hammers … 35 % the player … 100 % the audience), **`lid`** (0 closed … 100 % open) and
**`roomsize`** (0–100 %, 40 %); `width` is model 2's pair width too (a mid/side gain). Voicing
pass two (after the owner's first ABX) is in ADR 0098's second amendment. `model` (1–2) is set,
never automated or modulated; loading any preset on an old song's Chorda moves it to model 2.

## Parameters

Shares are fractions shown in %; 100 % is the model as measured.

| Key | Range (default) | What it does |
|---|---|---|
| `model` | 1–2 (2) | The model a song was made with, stored with every instance; schema 54 writes 1 into every older one |
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
| `pedal` | 0–100 % (0) | The sustain pedal as a parameter (round 28, the demo builder): what an automation lane or a modulator drives, half-pedalling included; the higher of it and controller 64 is the pedal the strings hear |
| `damper` | 0–100 % (100) | How firmly the dampers grip |
| `una` | 0–100 % (100) | How far the soft pedal shifts the hammers |
| `body` | 0–200 % (100) | The soundboard's ring |
| `tilt` | ±12 dB (0) | The top raised or lowered |
| `width` | 0–100 % (70) | The keyboard's spread across the stereo field; model 2: the pair's width |
| `gain` | −48–12 dB (0) | The output |
| `room` | 0–100 % (35) | Model 2: the room's level |
| `noise` | 0–200 % (100) | Model 2: the hammer's blow, the keybed and the dampers landing |
| `persp` | 0–100 % (35) | Model 2: the microphones, over the hammers (0) … the audience (100 %) |
| `lid` | 0–100 % (100) | Model 2: the lid, closed (0) … open |
| `roomsize` | 0–100 % (40) | Model 2: the room's size (its decay ×0.44 … ×3.5) |

The modulation matrix, the eight macros and the sources (velocity, key, mod wheel,
aftertouch, expression, sustain) work as on every native instrument; the three tuning keys
are never modulated.

**Presets** (round 37: twenty-four, voiced for model 2, tagged with the preset library's
styles and roles, ADR 0101; macros Hardness, Resonance, Body, Decay): film — Concert Grand,
Hall Grand, Game Theme, Baroque Pitch (A 415), Cinematic Soft; pop — Studio Bright, Pop
Forward, Close and Intimate, Dry Studio; worship — Warm Ballad, Gospel Grand; jazz — Jazz
Club; R&B — R&B Keys; latin — Montuno, Reggaeton Keys; rock — Rock Piano, Honky-Tonk; trap —
Trap Keys; house — House Piano; lo-fi — Felt Piano, Lo-fi Grand, Old Grand; ambient (pads) —
Pedal Wash, Ambient Halo.

## What the tests hold

ADR 0059's table: tuning within a cent (designed and measured on the sound), T60 against
Bensa et al., the continuous half pedal and re-pedalling, the two-stage decay, una corda,
sympathy, the cost of 64 pedal-down voices, the family's rules through the contract, offline
equal to real time, headroom, and the editor's views, gestures and texts.

## Measured

| What | Value |
|---|---|
| Worst partial on the rendered sound, all 88 keys | 0.47 cents at 44.1 kHz, 0.49 at 48 and 96 kHz, 0.48 at A4 415 Hz / 70 % stretch, 0.50 at 150 % stiffness |
| T60 against Bensa et al. (C2, C4, C7, first partials) | 0.96–1.01 of the measured |
| Prompt sound / aftersound, middle C held | 24.6 / 8.7 dB/s |
| Half pedal: decay after release, pedal 0 → 100 % (dampers alone) | 241 → 41 dB/s, monotonic |
| Sympathy with the pedal / without (energy against the dry note) | −16 dB / −35 dB |
| 64 pedal-down voices, 256 frames, one thread, release | 18–19.5 % of a core |
| Both hands fortissimo with the pedal / mezzo-forte middle C | −3.3 / −22.7 dBFS |
| A keyboard's design (88 + 88 reduced), eight threads | about two seconds |

## Not done (the owner's)

- The blind listening test (≥ 20 listeners, Pianoteq 9 and a sampled grand).
- The legal review of the structure against the patents ADR 0059 names.
- The presets' last word by ear.
