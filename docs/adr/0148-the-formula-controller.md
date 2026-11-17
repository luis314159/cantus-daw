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

## Consequences

- Schema 67 (shared with ADR 0147): `ModSource::Formula`; nothing older has one. The v67
  fixture: FL's `a*Sin(SongTime*Pi*b)+c` on the first strip's fader, knob b automated.
- The formula's face (the display the formula is typed on, its live plot, the knobs, the
  targets) is step 3, after the owner approves the design sheet
  (`compartir/formula/design/v1`).
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
