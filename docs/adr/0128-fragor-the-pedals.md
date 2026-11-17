# ADR 0128 — Fragor, the pedals

**Status:** accepted (the owner's verdicts on the design, v3), pending integration (fx side round) · **Date:**
2026-11-13 · **Schema:** none beyond the contract's 72 (the kind's new parameters are read
with their defaults from older files) · **Relates to:** ADR 0004 (commands), ADR 0005 (the
audio thread), ADR 0018 (null defaults), ADR 0020 (our plug-ins outside Cantus), ADR 0026
(Fervor: saturation), ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert),
ADR 0074 (faces), ADR 0101 (presets by style and role)

## Context

Fervor (ADR 0026) is saturation and colour; it is not a guitarist's pedalboard. The
reference pedals — the Tube Screamer, the RAT, the DS-1, the Fuzz Face, the Big Muff — each
have a sound that comes from their circuit (clipping diodes in the feedback loop or to
ground, an op-amp too slow for its gain, a transistor fuzz's bias, a tone stack's mid
scoop), and the best plug-ins model those circuits rather than a generic curve. BACKLOG
§3b asks for distortion pedals and fuzz by name, with the dying-battery gating of a fuzz
as its *done when*.

## Decision

A new effect kind, `EffectKind::Pedal` (Latin *fragor*, a crash, a roar), `fx::fragor`:
**a board of up to three pedals in any order**, five circuit models, a noise gate before
them, input, mix, output, mono/stereo and 2×/4×/8× oversampling.

### The models: circuits, not curves

Each pedal is its schematic, transcribed component by component (the values are in
`cantus-dsp/src/fragor_circuits.rs` beside each element), discretised by the trapezoidal
rule and solved every oversampled sample by the **DK method** (Yeh, Abel & Smith,
*Automated physical modeling of nonlinear audio circuits*, IEEE TASLP 2010; Holters &
Zölzer, DAFx-11): nodal analysis reduces the circuit to its nonlinear ports (one per diode
pair, two per transistor, one per slewing op-amp) and Newton's method solves only those —
one to four unknowns — each sample, from a bounded quadratic prediction, with SPICE's
junction limiting (`pnjlim`). Capacitors are their trapezoidal companions; the state is one
history source per capacitor; the per-sample work is two passes of independent
multiply-adds over precomputed columns, rebuilt only when a knob moves a resistor. All in
`f64`, in a fixed order, with our own exponential: the same bits everywhere.

**Why the DK method**, not wave digital filters or the K-method's tables: a WDF handles one
nonlinearity per tree elegantly, but the Fuzz Face and the Big Muff's stages have two to
five coupled ones (R-type adaptors and an iterative solve — the same Newton in a harder
coordinate system); the K-method's tables grow with the nonlinear dimension and with the
knobs (a table per pot position, or interpolation across them). The DK formulation takes
the schematic as written, any number of coupled ports, pots that move continuously, and an
independent analysis to check it against (below). Its cost is a small Newton per sample.

| Model (UI name) | Reference circuit and source | Solved | Ports |
|---|---|---|---|
| Green overdrive | Ibanez TS808 — ElectroSmash, *Tube Screamer analysis* | in one | 1 |
| Op-amp distortion | ProCo RAT — ElectroSmash, *ProCo RAT* | in one | 2 |
| Silicon distortion | Boss DS-1 (DS-1A) — ElectroSmash, *Boss DS-1 analysis* | booster ‖ gain stage (split at U1a's follower: exact) | 2 ‖ 1 |
| Germanium fuzz | Dallas Arbiter Fuzz Face, AC128 — ElectroSmash, *Fuzz Face*; Silicon Chip, Dec. 2024 | in one, battery included | 4 |
| Triangle muff | Electro-Harmonix Big Muff π V3 — ElectroSmash, *Big Muff Pi analysis* | three parts joined at R19 and R12 | 2 ‖ 3 ‖ 3 |

The ElectroSmash pages (CC BY-SA articles; only facts and component values are used) were
read from their archive at electrosmash.mas-effects.com — the original site did not
resolve from this machine; the schematics are the pages' figures. No code was read or
ported from any implementation.

**Component values.**

- *TS808:* the signal through C2 1 µF and RA 1 kΩ to the + input, R5 10 kΩ to 4.5 V; − input
  R4 4.7 kΩ + C3 47 nF to ground; feedback R6 51 kΩ + Drive 500 kΩ (A) ∥ C4 51 pF ∥ two
  silicon diodes (MA150, modelled with the 1N4148's Is = 2.52 nA, n = 1.752); JRC4558
  (3 MHz, 100 dB). Tone stage: R7 1 kΩ, C5 0.22 µF, R9 10 kΩ to 4.5 V on the + input; Tone
  20 kΩ between the + and − inputs, its wiper through C6 0.22 µF and R8 220 Ω to ground;
  R11 1 kΩ feedback; C7 1 µF, 1 kΩ, Level 100 kΩ (A). The input and output buffers are
  taken as followers.
- *RAT:* R1 1 MΩ, C1 22 nF, R2 1 MΩ to 4.5 V, R3 1 kΩ, C2 1 nF; feedback Distortion
  100 kΩ (A) ∥ C4 100 pF; − input R4 47 Ω + C5 2.2 µF and R5 560 Ω + C6 4.7 µF to ground;
  LM308 with C3 30 pF — slew rate 0.3 V/µs (ElectroSmash, from the datasheet), so its input
  pair's current saturates at 9 µA and its unity gain is at ~1 MHz (a tanh
  transconductance: the slew rate is a port of the circuit); C7 4.7 µF, R6 1 kΩ, two 1N914
  to ground; Filter 100 kΩ (A) + R7 1.5 kΩ, C8 3.3 nF; C9 22 nF, R8 1 MΩ (the JFET
  follower as unity).
- *DS-1:* R1 1 kΩ, C1 47 nF, R2 470 kΩ to 4.5 V, Q1 a follower (R3 10 kΩ); C2 0.47 µF,
  R4 100 kΩ, C3 47 nF; Q2 2SC2240 (β 400) with R5 100 kΩ, R7 470 kΩ ∥ C4 250 pF collector
  to base, R8 10 kΩ, R9 22 Ω; C5 68 nF, R10 100 kΩ. U1b: feedback R11 100 kΩ ∥ C7 100 pF;
  Dist 100 kΩ (B) + R13 4.7 kΩ + C8 0.47 µF to ground (gain 1 … 22.3, ElectroSmash's
  formula); NJM3404A-class (1.2 MHz; its 0.6 V/µs is never reached at 22.3 of gain — 4 V
  at 5 kHz asks 0.13 V/µs — so the op-amp is linear here, a port fewer); R14 2.2 kΩ, C9 0.47 µF, two 1N4148 to the
  4.5 V reference, C10 10 nF; tone C11 22 nF + R15 2.2 kΩ / R17 6.8 kΩ (high-pass,
  1 063 Hz) and R16 6.8 kΩ / C12 0.1 µF (low-pass, 234 Hz), Tone 20 kΩ (B) between them,
  Level 100 kΩ (B). D8 (the + input's protection) is left out: ElectroSmash shows it never
  touches the signal.
- *Fuzz Face:* C1 2.2 µF; R1 33 kΩ; Q1, Q2 AC128 (ElectroSmash's PSpice models: Is 85.8 nA
  β 85, and Is 120.8 nA β 120; βr 20); R4 100 kΩ from Q2's emitter to Q1's base; Fuzz 1 kΩ
  (B) from Q2's emitter to ground, its wiper to ground through C2 20 µF; R3 8.2 kΩ to Q2's
  collector, R2 470 Ω to the battery, the output from their junction (the divider that
  sets the Fuzz Face's modest output); C3 10 nF, Volume 500 kΩ (A). The PNP original on
  −9 V is this NPN circuit on +9 V with every voltage negated, so the signal is negated in
  and out. The guitar is a 4.7 kΩ source (its pickup's inductance is left out).
- *Big Muff π V3:* input booster R2 39 kΩ, C1 1 µF, R14 47 kΩ, R9 470 kΩ ∥ C10 470 pF,
  R13 10 kΩ, R22 100 Ω; C4 1 µF, Sustain 100 kΩ (B), R23 1 kΩ, C5 100 nF, R19 10 kΩ;
  clipping stages R20/R16 100 kΩ, R17/R15 470 kΩ ∥ C12/C11 470 pF, two 1N914 in series
  with C6/C7 1 µF from collector to base, R18/R11 10 kΩ, R21/R10 150 Ω, C13 100 nF,
  R12 10 kΩ; tone R8 39 kΩ / C8 10 nF and C9 4 nF / R5 22 kΩ, Tone 100 kΩ (B); output
  booster R7 430 kΩ, R3 100 kΩ, R6 15 kΩ, R4 3.3 kΩ, C2 100 nF, Volume 100 kΩ (B) — its
  3.3 kΩ of emitter degeneration keeps it linear well past the signal the tone stack hands
  it, so it is its exact small-signal linearisation at its operating point (rπ and gm from
  the DC solve), two ports fewer. All
  four transistors BC239 (β 300). The three parts are joined at R19 and R12: each drives
  the resistor against the next stage's base voltage of the previous sample and hands it
  the current that flows — a loop of gain about 0.2, so one oversampled sample of delay in
  it costs hundredths of a dB (treating the bases as virtual grounds was 6 dB off).

**The fuzz's Bias is its battery.** Fresh it is 9 V behind 2 Ω; turned down, the voltage
falls (squared, to 1.2 V) and the internal resistance climbs (to 1 kΩ) as a cell's does
at the end of its life. ElectroSmash notes the battery's resistance adds to R2 and changes
the sound; below about 3 V the starved transistors gate a decaying note — the sputtering
end of a note on a dead battery, from the circuit, not a gate added to it.

**Levels.** One volt at the jack is 1.0 in the signal. Each pedal's output is its volume
pot's share (its own taper) times a calibration (`CAL_DB`: +14.2, +6.4, +3.8, +15.9,
−1.7 dB) that makes the knobs at noon about as loud as bypass for a guitar's note at
−18 dBFS RMS (`fragor_tests::calibration`).

### The board

Three slots (`s1` … `s3`, each 0 = empty or a model 1 … 5) hold the chain in signal order;
a pedal is in at most one slot (a second mention is ignored). Each pedal has its knobs as
the pedal labels them and its footswitch (`od.on` …). A footswitch crossfades in 10 ms and
**never moves the latency**: the wet path (oversampled) runs while any pedal is on the
board, and with every footswitch off the output is the input delayed by the latency,
exactly. A new order dips to the dry signal for 10 ms each side of the change. The gate
(Porta's design reduced: linked peak follower, 6 dB hysteresis, 30 ms hold, 1 ms open,
60 ms close; closed is +0.0) runs before the pedals. Settings are read every 16 samples
from the stream's start and the knobs glide (20 ms): the output does not depend on the
block size.

**Rest.** A circuit never quite stops, so the board counts exact zeros: after a second of
silence into the chain (after the gate), every pedal returns exactly to its operating
point (a DC solve with the supplies stepped up, cached until a knob moves) and the
oversamplers are cleared — the state a new unit has — and silence then costs nothing.

### Defaults and the starting preset

Every default is a null: the board is empty and the gate off (0 dB gains). The knobs
default to noon (the fuzz's battery fresh). The starting preset (ADR 0071) is *Green
push* — the green overdrive on the board, drive 45 %, tone 55 %, level 55 % — so the
insert sounds at once. Fifteen factory presets (`presets/effects/pedal.toml`) cover guitar
(green push, blues edge, stacked lead, op-amp rhythm, silicon crunch, metal wall, vintage
fuzz, dying battery, muff sustain), bass (doom bass, bass grit), drums (drum smash), synths
(synth fuzz), voice (vocal grit) and a lo-fi radio, each brought to its role's level by
`preset_tool fit` (the pedals' calibration makes noon unity on a guitar's note; a preset
pushed harder is louder, so its output comes down). The guitar and bass presets play one
circuit (mono), as a pedal does: half the CPU. The unit's preset CPU budget is 0.25 of a
core (`preset_measure::budget`; 0.10 for most effects, Chorda's 0.20): circuits solved by
Newton every oversampled sample cost more than curves.

### The editor

Its face is palette A of the three the owner was shown (warm amber, a valve amp's
character); B and C are offered as its colourways (ADR 0127 §Colourways; the integration,
2026-11-14), **Cream** (cream and brass, on printed card — brushed metal near this brass is
Aula's and Calor's — with a bottle-green second voice) and **Steel** (steel with one signal
red), chosen in the window's ⋯ menu (`kit::skin::FRAGOR_COLOURWAYS`). Its panels are smoked glass,
as a valve's envelope and the screen's pane (near this amber the other materials are worn
by Phasis, Fervor, Chorda, Aula, Lapsus and Clipper, and siblings never look alike), and
its second voice a filament's white heat, nearly unsaturated, so the display's two voices
never share a hue. From the top:

- **The sound of the chain** — one cycle of a 200 Hz note in and out at the level playing,
  and its first twelve harmonics, odd solid and even hatched, the dominant ones marked, with
  the THD; an amber screen behind glass, dark in every theme. A copy of the board computes
  it **on its own thread** (a new computation when the settings change, at most four a
  second when only the level does; the first one at once): on the window's thread it cost
  most of a frame (34.6 ms mean with the song playing).
- **A valve amp's tube strip** behind a perforated grille: preamp and power tubes with
  incandescent filaments, plates with depth, micas, getters, glass and bakelite bases,
  glowing steadily at rest and brightening slowly with the signal; amber jewel lights and a
  brass name plate (`views/fx/valve_draw.rs`, reusable).
- **The board**: the pedals as original vector enclosures (`views/fx/pedal_draw.rs`,
  `pedal_art.rs`, `kit/skin/pedals.rs`) — each its finish (jade, graphite, copper, oxblood,
  steel), surface texture (metal flake, crinkle, hammertone, brushed), border, a large
  original illustration and its own lettering, which **never elides** (a smaller cut, then
  two lines); knobs with printed scales, footswitch, LED, top jacks, patch cables. **No
  product's colours, graphics, lettering or logos**; the UI names are generic descriptions,
  the products only here and in the manual's history note. Pedals come in five shapes —
  mini, standard, wide, double and round, data in the kit — and the board lays mixed widths
  out, scales them down to a legible floor (55 %) and then scrolls sideways (the kit's
  scrollbar, the wheel); each pedal's size (compact, standard, large) is a view preference
  of the editor's session. **Energy flows in the cables**: amber streaks travel in the
  signal's direction, as bright and dense as the level at that point of the chain (after the
  gate; after each pedal its output), still and dark at silence — the mixer's cable gleam
  (round 39) in the valves' amber; the window repaints only while it flows.
- **Interaction**: each slot has a numbered tag above it, the slot parameter's own control
  (its menu: automate, modulate, learn); drag a pedal onto another slot (they swap, one command, one undo step);
  click an empty slot for the pedals not on the board; right-click a pedal to replace or
  remove it or choose its size; every knob is its parameter (drag, double-click to reset,
  the parameter menu: automate, modulate, learn, reset); the footswitch is a stepped
  parameter. Beside the board: gate and input, output and mix, quality (2×/4×/8×, stereo or
  mono), and the levels.

## Consequences

- A distortion voice that is a circuit: the five pedals behave as their circuits do at
  every knob position, because the knobs move the circuit's resistors.
- CPU: a Newton solve per oversampled sample per pedal — more than a waveshaper; measured
  per pedal in `docs/round-fx/fragor.md` (to be measured again on a quiet machine: every
  measurement this round was taken with other builders holding the machine at full load).
- The editor's frame with the window open and the song playing, 1140 × 800 at 100 %: 7.6 ms
  mean, 6.7 median, 14.7 at the 95th percentile (dev profile, loaded machine;
  `FRAGOR_TIME=1 cargo run -p cantus-ui --example shoot_fragor`).
- Cantus's FLAC reader (claxon) does not read 32-bit integer FLAC: the CC0 guitar used for
  the listening set (Carbonthief) had to be decoded outside. For the backlog.
- The models are as good as their component values and device models: transistor β and
  diode parameters are typical values, not a measured unit's. Pedals vary unit to unit
  (germanium most of all); the owner's ears judge the result.

## What the tests hold

- `cantus-dsp` `fragor_spice_tests`: each model's small-signal response against an
  independent analysis — the whole schematic typed again, modified nodal analysis, the DC
  operating point by Newton with source stepping, complex AC at each frequency — at three
  or four knob settings each: TS808, RAT, DS-1 and Fuzz Face within **0.1 dB** from 80 Hz
  to 3 kHz and 0.25 dB at 8 kHz (the trapezoidal rule's warping at 192 kHz); the Big Muff
  within **0.08 dB** to 1 kHz and 0.5 dB at 8 kHz (its coupling's one-sample delay).
- Published operating points reproduced: the Fuzz Face's Q2 collector at 4.7 V and Q1's at
  0.7 V (ElectroSmash's targets: 4.5 V and 0.5 – 0.7 V); the Big Muff's input booster
  collector at 7.0 V (ElectroSmash: 7 V).
- `fragor_tests`: the defaults and an empty board are a wire to the bit; footswitches off
  are the input delayed by the latency, to the bit; every pedal sounds at its minimum;
  block sizes 1, 37, 64, 512 identical with knobs, a footswitch, a reorder, mono and the
  gate moving; rest after silence as good as new; silence stays silent; the order is
  heard; mono is one circuit; the dry path aligned at 2×, 4× and 8×; a footswitch does not
  click; the gate closes on hiss to exact silence; **the dying battery gates a decaying
  note** (fresh, the output holds within 1 dB while the input falls 24 dB; at Bias 10 % it
  falls 20 dB more than the input within 0.4 s); aliasing; calibration.
- **Aliasing** (a 2.5 kHz sine at 300 mV through the op-amp distortion at 80 %, 48 kHz):
  what is not a harmonic is **−30.0 dB** under the harmonics at 2×, **−53.6 dB** at 4×
  (the default), **−63.1 dB** at 8×.
- `cantus-engine` `fx_fragor`: the null in a project render, an empty board with knobs
  turned, block-size independence, footswitches off as the project delayed by the latency,
  latency and telemetry through the contract; `fx_contract`'s split invariance and exact
  rest.
- `cantus-ui` `fragor_e2e`: on the boot path (`CantusApp::assemble`, the real engine
  handle), Fragor inserted on a playing strip the way the picker does sounds its starting
  preset in the master (brighter than without it), a knob turned through the editor's host
  is one command, a pedal moved to another slot is one step, and undo returns the render
  to the bit. The showcase demo plays Fragor on its guitar
  (`demos::the_showcase_plays_round_28`).
- `cantus-plugins`: Fragor exported as CLAP and VST3 renders bit-identical to the engine's
  unit (two pedals on, `tests/common`'s settings); its parameter ids are appended to the
  golden list.
