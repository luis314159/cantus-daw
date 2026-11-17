# ADR 0148 — The formula controller

**Status:** proposed (fx side round wave D, provisional number — renumbered at merge) ·
**Date:** 2026-11-17 · **Schema:** 67 (provisional) · **Relates to:** ADR 0005 (the audio
thread), ADR 0031 R9 (deterministic maths), ADR 0057 (modulators on any parameter), ADR
0062 (MIDI learn), ADR 0147 (the X-Y pad: the same architecture, links and inputs)

## Context

BACKLOG §3b's *Automation controller* row left one role open after round 28 (LFO,
envelope, follower) and Fluctus (LFO Tool): **Fruity Formula Controller** — a formula of
the song's time and three knobs a, b, c driving any parameter; one controller, several
targets, recorded as automation if asked. FL's manual (the *Fruity Formula Controller*
page of the online manual, read 2026-11-17) publishes its syntax, its variables, operators
and functions, and some thirty example formulas with what each draws.

## Decision

### 1. A modulator source, as the pad

**`ModSource::Formula(FormulaSource { text, inputs: [a, b, c], bipolar, links })`**,
`ModKind::Formula`, a seventh source of ADR 0057's modulators (ADR 0147 §1 says why a
source and not an effect kind; it is even clearer here: the formula *is* a modulator).
One formula drives any number of targets, each route with its depth and, through
`links`, its own range and bend (ADR 0147 §2). The knobs are `AutoTarget::ModInput`
(inputs 0, 1, 2) set by `Command::SetModulatorInput`: automated by a clip, learnt by a
MIDI controller, driven by another modulator, recorded while a pass records automation.
*Recorded as automation if asked* is ADR 0057 §5's *Bake to automation* (the formula is a
pure function of time and its knobs, so a bake needs no trace) and, for the knobs, the
recording of their moves.

### 2. The language: FL's syntax, safe and deterministic

`cantus_core::modulator::formula`: one expression, case-insensitive, parsed by recursive
descent and compiled **off the audio thread** to a flat postfix program (`Program`,
`Vec<Op>`), which the audio thread runs **without allocating** on a fixed stack. There
are no assignments, loops or recursion; a program is at most 256 steps run once each
(the only loop inside a step is a factorial's, at most 170 multiplications), at most 32
deep (proved while compiling, never checked while running), nested at most 48 deep (the
parser refuses deeper text before its own stack could overflow), and at most 512
characters. Every transcendental is `cantus_dsp::math`'s; the value is `f64` (a song's
time keeps its precision for hours) and the output is clamped to 0…1 as FL's (anything
not finite is 0), or −1…1 when bipolar.

**Supported** (FL's manual, its spellings): the knobs `a`, `b`, `c`; `SongTime` (song
position in quarters, also `beat`), `Pi`, `GetTempo` (also `tempo`), `Rand`; a name may be
written as a call of nothing (`SongTime()`); `+ - * / ^`, the suffixes `%` and `!`,
`= in <> < <= > >=` (1 or 0), `div mod fmod shl shr`, `not and or xor` (above 0.5 is
true); `Sin Cos Tan Tg Sec CoSec Ctg ArcSin ArcCos ArcTg ATan2 Sinh Cosh Tanh ArcCosh
ArcTanh DegToRad RadToDeg Int Trunc Floor Ceil Round Frac Sqrt Sqr Exp Ln Log10 Log2 LogN
Power Pow IntPower Max Min Sum Abs Neg Case If IfE IfG IfGE IfL IfLE Inter InterNoSwap Map
Fold SeededRand`. Delphi's definitions where FL is a Delphi program: `Round` to even,
`Int`/`Trunc` toward zero, `Frac` keeps the sign, `div`/`mod` on the whole parts, `LogN(base,
x)`, `Sqr` the square.

**Changed, so a render repeats:** `Time` is milliseconds since playback started (FL's is
the system clock); `Rand` is a hash of the song position (FL's is a running generator);
`SeededRand(n)` is Cantus's own hash of `n` (FL's sequence is not published).

**Refused, with a message where it is in the text:** `MouseX`, `MouseY`, `Date` (they
would make a render depend on the moment it was made); `Tension` and `Warp` (FL's page
does not say what they compute).

**Cantus's own additions:** `t` (seconds since playback started), `e`, and cycle waves
0…1 over one cycle of their argument — `sine`, `saw`, `tri`, `square`, `pulse(x, width)` —
and `clamp(x, lo, hi)`. Number literals take exponents (`1e3`).

### 3. Errors never reach audio

A `SetModulator` whose formula does not compile is **refused** (`FormulaSource::is_valid`
compiles it), so the project, the snapshot and the engine only ever hold formulas that
compile; the snapshot compiles each one once into its plan (`ModPlan::program`). The
editor keeps the typed draft apart from the project, shows the error under the display —
`ErrorKind::key()` (`fxd.formula.err.*`, in both languages) with the character where it
is — and commits only text that compiles; the controller keeps playing its last good
formula meanwhile.

### 4. The engine

A formula is a pure function of the song position, the time since playback started, the
tempo and its knobs: no state, so its block-size independence, real-time equals offline and
render-from-bar-N follow from the control grid (ADR 0057 §4). Its knobs are read from
their lanes at each control point and offset by the modulators that drive them (one level);
the program runs twice per control point (now and at the next grid point), the gains ramp
between.

## Phase B (step 3, 2026-11-17): the window, typing, presets, bake, the agents

The owner approved design sheet v1, the name **Calculus** (Latin *reckoning stone*) and §2's
rules for a render that repeats (`Time` since play, `Rand` of the song position, no
`MouseX`, `MouseY` or `Date`). Shots: `compartir/formula/design/v2-step3/`. The window,
its frame and *What it drives* are the pad's (ADR 0147 Phase B); what is the formula's own:

- **The face** (`views/fx/fxd_formula.rs`): the formula on a vacuum-fluorescent line, a
  plotting screen of its value over the last four beats and the next four at the knobs as
  they are (*Now* a cursor, the value a lit bead), the keypad with the dials a, b, c, the
  cards *Knobs* (a, b, c) and *Output* (Polarity *One way*/*Both ways*, Amount).
- **Typing** (`fxd_edit.rs`): a click on the display starts a session with a caret; text,
  Backspace, Delete, the arrows, Home and End edit at it, and the keypad's keys insert at
  it. The session keeps a **draft** apart from the project: each edit whose text compiles
  and differs from what plays is committed as a `SetModulator` merged under the session's
  key — **one undo step a session**, heard as it is typed; a draft that does not compile
  never reaches the project or the audio and shows its error under the display at its
  character (§3), the last good formula playing meanwhile. Enter, `Esc` or a press outside
  the display, the keypad and the dials ends the session; `Esc` drops a draft that does not
  compile (the last good text shows again). While a session types, the display holds the
  keyboard: the app's shortcuts wait and `Esc` is the session's, not the window's. A
  formula longer than the line shrinks its letters to 60 %, then shows a window of the
  text that keeps the caret in sight.
- **The keypad** (`fxd_formula::KEYS`, 24 keys; a click types its word at the caret):
  `Sin(` `Cos(` `tri(` `saw(` `Frac(` `Round(` `Int(` `Rand` · `a` `b` `c` `Pi` `SongTime`
  (the *beat* key) `t` `(` `)` · `+` `-` `*` `/` `^` `If(` `Max(` `Min(`. **The dials** a,
  b, c on the keypad: a vertical drag turns one (one undo step a drag), a double-click puts
  it back at 0.5; the *Knobs* card moves the same inputs (`SetModulatorInput`).
- **Presets** (`cantus_core::modulator::presets`, `fxd.preset.formula.*`), nine in FL's
  syntax on **one plan for the knobs**: `a` the rate — `2^(Round(a·6)−4)` cycles a beat,
  from one in four bars to four a beat (0.34 a bar, 0.67 a beat); `b` the shape (the
  waves' phase, the pulse's and the gate's width, the random's sequence, the envelope's
  decay, the climb's bend); `c` the depth. *Sine*, *Triangle*, *Saw*, *Square* (bipolar,
  a bar a cycle), *Pulse* (bipolar, a beat, width b 0.25), *Random each beat* (bipolar,
  `SeededRand` stepped per beat, b another sequence), *Gate on beats* (b its length, soft
  edges), *Envelope each bar* (a the attack, b the decay), *Climb* (over 1…64 bars by a,
  b the bend). The exact texts are `modulator_presets.rs`' constants; a test compiles each
  and samples its shape. A formula preset has no links: applied, every route reads the
  formula whole. `matching` ignores the knobs, so the header keeps the name while they
  are played.
- **Bake** (`modulator_bake.rs`): the formula compiled once, its knobs from their lanes,
  the route's range × amount at each control point — a pure function, no trace; a formula
  whose knobs another modulator drives is traced (`trace_route`, ADR 0147 Phase B).
- **The agents**: `add_modulator` kind `formula` with `formula` (its text), `a`, `b`, `c`,
  `bipolar`; routes take `range {from, to, curve}`; `set_modulator` takes all of it, its
  `a`/`b`/`c` become `SetModulatorInput`; a formula that does not compile is refused naming
  its character; a knob is a target `{modulator_id, param: "a"|"b"|"c"}` that reads
  "Formula 1 · knob b".
- **What a user may write**, as the window and the manual list it (§2 in a reader's
  order; names are case-insensitive, and a name may be written as a call of nothing):
  - *values*: `a` `b` `c` (0…1), `SongTime` (also `beat`, `beats`: the song position in
    quarter notes), `t` (also `seconds`) and `Time` (milliseconds) since playback started,
    `GetTempo` (also `tempo`, BPM), `Pi`, `e`, `Rand` (0…1, a hash of the song position);
    numbers with an exponent (`1e3`);
  - *operators*, loosest first: `= in <> < <= > >=` (1 or 0); `+ - or xor`; `* / div mod
    fmod shl shr and`; the signs `-`, `+` and `not`; `^` (right to left: `2^3^2` = 2⁹,
    `2^-1`); the suffixes `%` (a hundredth) and `!` (factorial of the whole part, up to
    170); above 0.5 is true;
  - *functions of one value*: `Sin Cos Tan`/`Tg Sec CoSec Ctg`/`Cot ArcSin ArcCos ArcTg`/
    `ArcTan Sinh Cosh Tanh ArcCosh ArcTanh DegToRad RadToDeg Int`/`Trunc` (toward zero)
    `Floor Ceil Round` (to even) `Frac` (keeps the sign) `Sqrt Sqr` (the square) `Exp Ln
    Log10 Log2 Abs Neg Fold` (reflected into 0…1) `SeededRand`;
  - *of two*: `ATan2(y, x) LogN(base, x) Power`/`Pow(x, y) IntPower(x, n) Max Min Sum
    IfE IfG IfGE IfL IfLE` (1 or 0) `pulse(x, width)`;
  - *of three*: `If(test, then, else)`, `Case(a, b, c)` (b when a is exactly 1, else c),
    `Inter(x, lo, hi)` and `InterNoSwap` (1 inside), `Map(x, lo, hi)` (lo + x·(hi − lo)),
    `clamp(x, lo, hi)`;
  - *Cantus's cycle waves*, 0…1 over each whole number of their argument: `sine` (from 0,
    `0.5 − 0.5·cos`), `saw`, `tri`, `square`;
  - *changed*: `Time`, `Rand`, `SeededRand` (above); *refused, at their character*:
    `MouseX`, `MouseY`, `Date` (a render would depend on the moment), `Tension`, `Warp`
    (FL's page does not define them).

## Consequences

- Schema 67 (shared with ADR 0147): `ModSource::Formula`; nothing older has one. The v67
  fixture: FL's `a*Sin(SongTime*Pi*b)+c` on the first strip's fader, knob b automated.
  Phase B changed no schema.
- Step 1's deferral is done: the face (the display typed on, its live plot, the keypad and
  dials, the targets) is Phase B.
- The pre-existing findings of the e2e (a native effect's start from its stored value on a
  play or seek; Firmus's smoothers at their defaults on a render that begins on a note) are
  ADR 0147's Consequences; the formula's e2e proves render-from-bar-N on the strip's fader.
- The three colourways (Bakelite, Ivory, Ebony) are chosen in the window's ⋯ menu (ADR 0147).
- Not here: a formula of other parameters' values or of audio (FL's controller has none;
  a follower covers audio), user-defined variables, multi-line programs.

## What the tests hold

- `modulator::formula::tests`, against FL's manual as the outside source: **every example
  formula the page publishes** (its 24 waves and shapes; the sample and hold apart), verbatim, sampled over two beats
  at three knob settings against a reference written from the page's description with the
  platform's libm (to 1e-6); the basic waves have the shapes the page names (a sine of two
  beats, a triangle reaching 0.319·π, a square); the page's boolean table for 1/0 inputs;
  the sample and hold holds and repeats; precedence and every FL word (`div`, `mod`,
  `shl`, `%`, `!`, `in`, `<>`, Delphi's rounding); what is refused and at which character
  (empty, unclosed, trailing, unknown, the moment-dependent names, the undefined ones,
  wrong argument counts, bad numbers, too long, too deep, too many steps, a stack too
  deep); the output is always in 0…1 (NaN, infinities, huge values); `reads()` says what
  moves it; evaluation repeats to the bit.
- `modulator::fxd_tests`: a formula that does not compile is refused by `SetModulator`; the
  default is FL's first example; a formula at zero outputs exactly 0; the plan holds the
  compiled program.
- `cantus-engine` `tests/fxd_controllers.rs` (with the pad): a formula of value 0 and one
  whose knobs are 0 are bit-exact with none; an automated knob and an LFO on another, with
  FL's stepped-frequency sine, are block-size independent and live equals offline; a
  synced formula from bar 5 equals the full render there; `rt_memory`: no allocation.

Phase B (step 3):

- `cantus-core` `modulator::presets::tests`: every formula preset compiles; the waves have
  their shapes at their rate; the random holds each beat and the gate opens on each; the
  envelope strikes each bar and the climb rises over its length; both languages name each.
  `modulator_bake`: a formula bakes with its automated knob and its compiled program; a
  driven controller is traced.
- `cantus-engine` `tests/fxd_bake.rs`: a formula with an automated knob bakes what it
  played; one whose knob an LFO drives bakes from its trace (within 0.002 at the control
  points, the render within 1 % of peak).
- `cantus-ui` `tests/fxd_drag.rs` (the formula's five of 17): the formula typed on its
  display is one step a session; the keypad types at the caret and `Esc` drops a broken
  draft; the dials turn a, b, c one step a drag; a dial double-clicked goes back to the
  middle; a formula longer than the line keeps the caret in sight (measured as painted).
- `cantus-ui` `tests/fxd_e2e.rs`: `formula.square` live = offline to the bit over 6 bars,
  its ±15 dB swing followed within 0.3 dB at every beat, from bar 5 the full render's; an
  edit to the formula heard (difference RMS 0.25) and undone to the bit, knob c halving the
  crest (+14.80 → +7.46 dB); an unlinked formula and a depth-0 route the dry master to the
  bit.
- `cantus-ai` `tests/tools_fxd.rs`: an agent makes a formula, and one that does not compile
  is refused at its character; knobs move as the hand moves them; a knob is a target that
  reads as itself and round-trips.
