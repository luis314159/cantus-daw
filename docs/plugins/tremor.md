# Tremor — tremolo, auto-pan and the harmonic tremolo

*Effect kind `Tremolo`. References: FL Studio's Fruity Panomatic and its tremolo presets,
Soundtoys Tremolator, Logic's Tremolo, Kilohearts Trance Gate, the Fender "brown" amps'
harmonic vibrato (ADR 0055).*

Tremor moves a sound's **level** (tremolo), its **place between the speakers** (auto-pan,
a balance under the equal-power law of ADR 0012), or — in the harmonic mode — its **lows against its
highs**, in time with the song. Its LFO is the modulation family's: synced to the song
position (a note value from 8/1 to 1/32, dotted and triplet values between, with swing) or
free in Hz from the start of playback; six shapes, a soft edge on the stepped ones so a
square never clicks; the right channel's phase apart from the left's for stereo movement.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/tremor.rs` |
| DSP | `crates/cantus-dsp/src/tremor.rs` (tests in `tremor_tests.rs`); the family's clock and LFO in `modfx_clock.rs`, `modfx_lfo.rs` |
| Engine glue | `crates/cantus-engine/src/fx/tremor.rs`, the family's in `fx/modfx.rs` |
| Editor | `crates/cantus-ui/src/views/fx/tremor.rs`, the family's parts in `modfx.rs` |
| Strings | `crates/cantus-ui/locales/fx/tremor.{en,es}.txt` (also the family's shared words) |
| Tests | `crates/cantus-engine/tests/fx_tremor.rs`, `crates/cantus-ui/tests/fx_tremor.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Tremor` (CLAP `com.github.lalmazan.cantus.tremor`, VST3 `673BE03AACAD1CEDD8FFE2250D2C4581`) |

## What the references do well, and what people complain about

- **Panomatic** is FL's quick auto-pan and volume LFO: shape, speed, volume and pan
  amounts. It runs in tempo, shows nothing of the movement, and its square clicks.
- **Tremolator** is the favourite for its groove (swing, a step pattern), its "dynamics"
  and its scope; people complain of a crowded panel.
- **Trance Gate** is the gating standard: steps on a grid with a resolution and swing.
- What users ask of all of them: **tempo sync that survives a jump in the song**, no
  **clicks** on a square, a **stereo** spread, **harmonic** tremolo (rare outside amp
  simulators), and a picture of what will happen.

Tremor answers each: the LFO is a function of the song position (a render from bar 5 hears
what bar 5 of the full render hears; a loop that wraps puts it back), every stepped wave has
a soft edge, the stereo phase is a knob, the harmonic mode is one click, and the editor draws
the next two cycles of the level, pan or halves with a playhead where the unit is now.

## Parameters (keys are forever)

The depth starts at 0, so Tremor inserted and left alone returns its input to the bit;
everything else is set to a musical value, waiting for the depth.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `mode` | 0 … 2 | Tremolo | Tremolo, Auto-pan, Harmonic |
| `depth` | 0 … 100 % | 0 | how far the level (or the pan, or the two halves) moves |
| `sync` | Free / Sync | Sync | the rate follows the song (`div`) or runs free (`rate`) |
| `div` | 8/1 … 1/32 | 1/8 | the synced cycle as a note value (· dotted, T triplet) |
| `rate` | 0.05 … 20 Hz | 4 Hz | the free rate, from the start of playback |
| `shape` | 0 … 5 | Sine | Sine, Triangle, Square, Saw up, Saw down, Random |
| `edge` | 0 … 100 % | 30 % | the soft edge of the square, the saws and the random steps |
| `swing` | 0 … 100 % | 0 % (straight) | every second synced cycle starts late, as the drum machines swing: 0 … 100 % here is an MPC's 50 … 75 % (67 % here is the triplet feel, an MPC's 66 %) |
| `spread` | 0 … 180° | 0 | the right channel's LFO ahead of the left |
| `offset` | 0 … 360° | 0 | where in its cycle the LFO starts |
| `xover` | 100 … 4 000 Hz | 700 Hz | the harmonic mode's crossover |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

- **Tremolo**: gain `1 − depth·(½ − ½·m)` for the LFO's value `m` in −1 … 1: full at the
  top, `1 − depth` at the bottom.
- **Auto-pan**: the pan position is `depth·m`; it is a **balance** (`balance_gains`): the
  near channel stays at unity and the far one takes the equal-power law's gain (`√2·cos θ`
  or `√2·sin θ`, capped at 1), so no channel is ever raised — a stereo source never gains
  the +3 dB a pan law would give its near side (review 1) — and a centred moment is the
  input itself.
- **Harmonic**: a Butterworth low-pass at the crossover and its complement; the lows take
  the tremolo gain, the highs the opposite one.
- **The clock** (`modfx_clock`): the song sample at the current tempo while playing — the
  engine's `SongPosition`, once per pass, re-anchored only on a jump (a seek, a loop's
  wrap, a tempo change, the start of playback), never because a block was cut. A free LFO
  counts from the start of playback; a rate change re-anchors it where it is, so it never
  jumps. In another DAW the exported plug-in reads the host's transport (CLAP
  `clap_event_transport`, VST3 `ProcessContext`): a synced Tremor follows that DAW's tempo.
- **Exact rest**: Tremor holds no audio but the crossover's two states, flushed after a run
  of silence; its clock is advanced by `skip`.

## Factory presets

Surf amp · Slow swell · Helicopter · Trance gate · Stereo shimmer · Random steps · Pan on
the quarters · Slow wide pan · Harmonic, brown amp.

## What the tests hold

- At its defaults and at depth 0 in every mode, the render is the render without it, to
  the bit (`fx_tremor.rs`).
- Working (every mode, a square with swing and spread): the same bits at blocks of 1, 7, 64,
  1 000 and 4 096; live playback equal to the offline render, synced and free.
- A full square on the quarters passes the lead untouched through the first half of every
  beat and silences the second — from the song's start and from beat 3; at 90 BPM the
  halves follow 90 BPM.
- The DSP: split invariance with the transport given per call; a synced render from bar 5
  equal to bars 5… of a full render; a free LFO deterministic from the start of playback;
  auto-pan keeps a mono source's power (`L² + R²` within 1e-5); the harmonic mode moves the
  lows against the highs; rest and `skip` exact.
- The family's contract (`fx_contract.rs`): split invariance at defaults, heavy and random
  settings; the at-rest twins.
- The export: CLAP and VST3 bit-identical to the DSP, automation on the 16-sample grid at
  any block size; a synced Tremor follows the host's tempo and position
  (`crates/cantus-plugins/tests/{clap,vst3}.rs`).
- The editor: its cards and words, a mode chosen in one undo step, the movement drawn is
  the movement played (to 1e-4), every preset in both languages.
