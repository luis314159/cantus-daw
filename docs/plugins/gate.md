# Porta — the gate and expander

*Effect kind `Gate`. Benchmark: FabFilter Pro-G 1.42 (`research/fabfilter/pro-g.md`).*

Porta is a gate, a downward expander, an upward expander and a ducker around one detector.
The key can be the gate's own input or any strip of the desk; it passes through a
high- and low-pass before the detector, and you can hear it, or hear exactly what the gate
takes away. The display is the control: the open and close thresholds are lines you drag
on the level history, the transfer curve has handles for the thresholds, the ratio and the
range, and the key's filters are triangles you drag on a spectrum.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/gate.rs` |
| DSP | `crates/cantus-dsp/src/porta.rs` (tests in `porta_tests.rs`) |
| Engine glue | `crates/cantus-engine/src/fx/gate.rs` (telemetry layout documented there) |
| Editor | `crates/cantus-ui/src/views/fx/gate.rs` |
| Strings | `crates/cantus-ui/locales/fx/gate.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_gate.rs`, `crates/cantus-ui/tests/fx_gate.rs` |

## Parameters (keys are forever)

| Key | Range | Default | Meaning |
|---|---|---|---|
| `thr` | −80 … 0 dB | −40 | where the gate **opens** |
| `hyst` | 0 … 24 dB | 4 | how far below `thr` it **closes**; the close threshold is `thr − hyst` |
| `ratio` | 1 … 100 (log) | 100 = ∞ | expansion below the threshold (Upward: above it); ∞ is a gate |
| `range` | 0 … 100 dB | 40 | the most it takes away; 100 is ∞ (a shut gate is exact silence). Upward: the most it adds, capped at 24 dB |
| `knee` | 0 … 30 dB | 0 | soft knee around the threshold |
| `att` | 0.01 … 1000 ms (log) | 1 | time to open fully |
| `hold` | 0 … 500 ms | 20 | time it stays fully open after the key falls below the close threshold |
| `rel` | 1 … 5000 ms (log) | 150 | time to close fully |
| `look` | 0 … 10 ms | 0 | lookahead; any value above 0 costs a fixed 10 ms of latency |
| `style` | 0 … 5 | Classic | Classic, Clean, Vocal, Guitar, Upward, Ducking |
| `hpf` / `lpf` | 20 … 10 000 / 100 … 20 000 Hz (log) | off / off | the key's filters; 20 Hz and 20 kHz are off |
| `key_slope` | 12 … 48 dB/oct | 24 | their slope (the editor offers 12, 24, 48) |
| `listen` | 0 … 2 | 0 | hear the gate, the key, or what the gate removes |

The hysteresis is stored **relative** to the threshold, so dragging the open line carries
the close line with it (the usual intent: move the gate, keep its hysteresis); Alt-drag
moves the open line alone, and the close line stays where it is.

## Decisions and why

**One state machine with four phases.** Closed → *open* when the key reaches `thr`; open →
*hold* when it falls below `thr − hyst`; hold → *release* after `hold` ms (back to open if
the key comes back above the close threshold); release → closed when the gain has landed.
An open gate measures its curve from the close threshold, a closed one from the open
threshold — that is what hysteresis is, and it is also what makes a ratio below ∞ behave
(an expander with hysteresis). The editor's lane draws these four phases.

**Times in real units, the same in every style.** The gain moves along a ramp in dB whose
speed is set so that a full swing (the whole range) comes within 1 % of its end in exactly
the attack or release time, then through a one-pole that rounds the corners. The share of
the time spent rounding is the style's `shape`; the ramp is made faster by exactly as much
as the rounding adds (`ramp_share` solves `c + f·ln(100 (f/c)(1 − e^{−c/f})) = 1`), so the
time you set is the time it takes. A smaller change takes proportionally less (the ramp is
a speed, like the benchmark's "time to reach full cut").

**Styles are spelled out** (`porta::Style::spec`), and the editor prints the words:

| Style | Mode | Detector | Motion |
|---|---|---|---|
| Classic | lowers below | peak, lets go over 10 ms | straight lines in dB (shape 0.02): fast, console-like |
| Clean | lowers below | peak, lets go over 30 ms (less flutter on low notes) | a rounded S (0.15) |
| Vocal | lowers below | RMS over 10 ms | a rounded S (0.18) |
| Guitar | lowers below | RMS over 20 ms | 0.08, and **closes slowly while the key is still near the close threshold** (a tenth of the speed just under it, full speed 12 dB under), so a note fading through the threshold is let down, not cut |
| Upward | raises above | peak, 10 ms | 0.1 |
| Ducking | lowers while the key is above | RMS over 5 ms | 0.12 |

RMS detectors are sine-calibrated (a sine reads its peak level), so a threshold means the
same dB in every style.

*Tried and dropped*: a Guitar that "never closes faster than the note fades" (the release
limited to the detector's own fall rate). Measured, it followed the RMS detector's own
decay (≈ 100 dB/s after a 20 dB/s note had dropped under the threshold), not the note —
the detector's smoothing *is* a fade. The level-dependent release is what does the job.
Also tried: Clean moving in amplitude instead of dB — it splattered no less at the opening
and was dropped.

**Lookahead that does not move the latency.** With any lookahead at all, the audio is
delayed by a fixed 10 ms and the key by `10 − look` ms, so the detector hears `look` ms
early and the latency the host compensates does not change while the knob turns (the
benchmark does the same). Off costs nothing.

**A gate starts open.** It holds for its hold time plus the lookahead's fill plus 1 ms
before it may close, so a signal above the threshold from the first sample passes to the
bit, and the start of a render is never cut by a detector that has not yet heard anything.

**A hit is an event, not a phase.** A hit is counted when the key rises through the open
threshold after having been below the close threshold — whether or not the gate had
closed in between — and stamped where the key crossed, on the *input's* timeline (the
lookahead is taken out), with the key's peak over the next 10 ms. So a fast re-trigger
during hold is a hit, and a hit's time is where the drum was struck.

**Telemetry that loses nothing.** The unit reports its last 24 columns of 5 ms (packed as
whole numbers below 2²⁴, which an `f32` holds exactly) with a counter; the editor takes
the columns that are new since its last frame, so it keeps a gap-free four-second history
even though it only sees the last block's telemetry. Hits travel the same way (last four,
with a counter).

## What the tests hold (numbers)

DSP (`porta_tests.rs`, run in release):

- **Wire, to the bit**: a signal that never falls below the threshold, in Classic, Clean,
  Vocal and Guitar with both key filters on; Upward with the signal below its threshold;
  Ducking with a quiet key. With lookahead, the output is the input delayed by exactly 480
  samples (48 kHz; 441 at 44.1 kHz) for look = 0.5, 3 and 10 ms. At range 0, every style
  whatever the signal.
- **Block-size independence at tolerance 0**: every style with filters (48 dB/oct),
  lookahead, hold, knee and a 4:1 ratio, at blocks 1, 7, 64, 1000, 4096 — samples and
  telemetry identical.
- **Silence**: after a burst, 10 s of silence gives exact zeros and every ringing state
  (filters, detector) at exactly zero; no NaN.
- **Hysteresis**: 1 kHz hovering ±1.5 dB around the threshold for 2 s — with 0 dB of
  hysteresis the gate chatters (≥ 20 openings); with 3 dB it opens once and never closes.
- **Hold**: 5, 50 and 180 ms of hold are exactly 240, 2 400 and 8 640 samples of the hold
  phase, fully open to the bit.
- **Timing**: attack 10 ms / release 100 ms and attack 2 ms / release 400 ms reach within
  1 % of the swing in the time set, ±3 %, in Classic, Clean, Vocal, Upward and Ducking.
- **Guitar**: a note fading at 20 dB/s through the threshold is let down at under a fifth of
  Classic's speed (Classic > 500 dB/s); a note that stops is shut within twice the release.
- **Click-free opening** (key steps up at a crest of a 200 Hz tone; peak of what a 4 kHz,
  48 dB/oct high-pass finds, relative to the tone): Clean with a 2 ms attack −70.5 dB,
  Classic −42.2 dB, a hard switch (0.01 ms) −7.5 dB.
- **Lookahead keeps the transient**: with 5 ms of lookahead and a 1 ms attack, a hit that
  starts at its peak comes out bit-identical (delayed); without lookahead its first sample
  is cut by more than 20 dB.
- **Key filters**: an HP at 1 kHz ignores a 100 Hz key at −10 dB that a 3 kHz key at the
  same level opens; the response drawn is −3.01 dB at the corner and −24 dB an octave out.
- **Delta**: gated + removed = input (delayed) within 10⁻⁶. **Key listen** plays the key.
- **Curve**: 2:1 at 10 dB under → −10 dB; an expander at −30 dBFS settles at −40 ± 0.5;
  Upward 2:1, 6 dB over → +6 dB (measured −18 ± 0.3 dBFS out); Ducking takes exactly the
  range and returns to exactly 0 dB.
- **Hits**: three hits in a second stamped at their exact sample, with and without
  lookahead; peak −6.02 dB ± 0.2. 200 columns in a second.
- **Learning**: hits over bleed at −45 ± 3 dB close between −42.5 and −37 dB and open at
  least 4 dB above, below every hit; noise alone closes 3 dB above its loudest 1 %.

Engine (`fx_gate.rs`): range 0 renders exactly as no gate; a gate that stays open renders
exactly as no gate over the riff; a ducker keyed from the bass strip is bit-identical at
blocks 1–4096, leaves the render untouched before the bass enters and moves it after; a
shut gate with ∞ range is exact silence; the latency through the contract is 480 samples
with lookahead and 0 without; telemetry counts 200 columns and four hits a second.

UI (`crates/cantus-ui/tests/fx_gate.rs`): the editor paints the styles' words, the
thresholds, the legend and the knob groups; the history takes each column once (and
across the counter's wrap); a captured hit lands at the right tick; dragging the open
line moves the threshold with the close line following, in one undo step; dragging the
close line sets the hysteresis, Alt-drag moves the open line alone; dragging the floor
sets the range; Learn sets both thresholds in one step; hits become a drum track with a
clip (merged, velocities from the peaks) and one undo removes it; presets name real
parameters and exist in both languages.

## Feature matrix against Pro-G

| Pro-G has | Porta | Verdict |
|---|---|---|
| Threshold −60…0, ratio 1…∞, range 0…100 | −80…0, 1…∞, 0…∞ (exact silence) | **have** |
| Hysteresis fixed per style | a parameter, drawn as a band and dragged as a line | **better** |
| Attack 0–1 s, release 0–5 s, hold 0–250 ms, knee 0–30 | 0.01 ms–1 s, 1 ms–5 s, 0–500 ms, 0–30; times hold in every style (tested) | **better** (hold range, stated timing) |
| Lookahead 0–10 ms, fixed 10 ms latency | the same, reported for PDC, shown in the editor | **have** |
| Six styles, described in one line in the manual | six styles, each spelled out as detector + motion in the editor | **better** |
| Expert SC filters HP/LP 48 dB/oct | HP/LP 12/24/48 dB/oct, matched design, dragged on a spectrum | **better** |
| Audition (key) | hear the key | **have** |
| — (cannot hear what is removed) | hear what the gate removes (Upward: what it adds) | **better** |
| External sidechain (host routing) | any strip of the desk, chosen in the editor | **better** |
| Read-only level display with transfer curve | history with open/close lines you drag, a phase lane, gain curve, bar lines, hover readout; transfer curve with handles | **better** |
| Ducking style | Ducking, keyed from any strip | **have** |
| Upward style | Upward, capped at +24 dB | **have** |
| MIDI triggering (open while a note is held) | — | **missing**: the engine does not route MIDI to effects yet |
| Wet/dry levels and pans | the host's slot mix (dry/wet on every slot) | **have, differently** |
| SC gain/mix matrix per channel, M/S | linked detection (the louder side decides) | **missing**: planned as a Detect switch (Linked/Mid/Side/per channel) |
| Oversampling 2×/4× | — | **missing**: the gain is smoothed so aliasing from the gain curve is not the problem it is for a compressor at 0 ms; left for the shared oversampling utility |
| Presets | nine: kick, snare, toms, hi-hat, vocal, guitar, noise floor, duck, upward punch | **have** |
| — | **Learn** the thresholds from a passage (hits over bleed, or noise alone) | **better** (host) |
| — | **Hits → MIDI clip** on a new drum track, velocities from peaks, one undo step | **better** (host; Oxford Drum Gate sells this) |

## Not done, and why

- **MIDI gating** (open or close while a note is held): needs the engine to route a MIDI
  track's notes to an effect — shared code (`processor.rs`, `messages.rs`).
- **Per-channel / Mid / Side detection**: one more parameter and a second state machine;
  left for a round of its own rather than half-built.
- **Timeline overlay and "commit as clip edits"** (P1-9 of the report): needs audio clips.
- **A capture that is sample-exact across a loop wrap**: a hit in the last block before the
  transport wraps is stamped from the position after the wrap. Hits closer than 15 ms are
  merged, so a loop played twice does not double its notes.
