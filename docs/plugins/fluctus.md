# Fluctus — the volume shaper

*Effect kind `VolumeShaper`. References: Xfer LFO Tool, Nicky Romero Kickstart, Cableguys
VolumeShaper, FL Studio's Fruity Peak Controller and Gross Beat's volume envelopes
(ADR 0055).*

Fluctus makes the level follow a **drawn curve** that repeats in time with the song — the
sidechain "pump" without a key, gates, swells, chops. The curve is up to **sixteen points**
over one cycle, and each segment is shaped exactly as the automation editor shapes a clip's
(its six shapes — curve, S-curve, hold, stairs, pulse, wave — and their tension), so a curve
drawn here and an automation clip bend the same way. The cycle is a note value from 4/1 to
1/16, a function of the **song position**; a **smoothing** glide makes sure a step never
clicks.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/fluctus.rs` (globals and sixteen points, sparse) |
| DSP | `crates/cantus-dsp/src/fluctus.rs` (tests in `fluctus_tests.rs`) |
| Engine glue | `crates/cantus-engine/src/fx/fluctus.rs` (`shape_at`: `cantus_core::automation::Shape::at`) |
| Editor | `crates/cantus-ui/src/views/fx/fluctus.rs`, the curve editor in `fluctus_curve.rs` |
| Strings | `crates/cantus-ui/locales/fx/fluctus.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_fluctus.rs`, `crates/cantus-ui/tests/fx_fluctus.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Fluctus` (CLAP `com.github.lalmazan.cantus.fluctus`); its points grouped as *Point 1 … 16* for hosts |

## What the references do well, and what people complain about

- **LFO Tool** is the standard: draw a curve, pick a length, done. Complaints: its curve
  editor has its own shapes, unlike the DAW's automation; steps click without care.
- **Kickstart** is one knob and four shapes — fast, but you cannot draw.
- What users ask: the pump **on the song's beats** whatever the transport does (a loop, a
  jump), presets that sound right at once, and no clicks.

Fluctus draws like the DAW's own automation, follows the song position (a render from any bar
is the full render there, through the family's clock — and in another DAW, the host's
transport), smooths every step (2 ms by default), and ships the classic pumps as presets.

## Parameters (keys are forever)

The depth starts at 0, so Fluctus inserted and left alone returns its input to the bit. The
default curve is the classic pump on the quarters: nothing on the beat, rising fast to full
by half the beat, held.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `depth` | 0 … 100 % | 0 | the level is `1 − depth·(1 − curve)` |
| `div` | 4/1 … 1/16 | 1/4 | the cycle's length, as a note value |
| `offset` | 0 … 100 % | 0 | where in the song's cycle the curve starts (nudge the pump onto the kick) |
| `smooth` | 0 … 50 ms | 2 ms | the level's glide |
| `out` | −24 … +24 dB | 0 | output gain |
| `p{n}.on` | used / unused | p0, p1 used | point `n` (0 … 15) is on the curve |
| `p{n}.x` | 0 … 100 % | 0 (p1: 50 %) | where in the cycle |
| `p{n}.y` | 0 … 100 % | 100 % (p0: 0) | the level there |
| `p{n}.t` | −1 … 1 | 0 (p0: −0.6) | the tension of the segment that starts there |
| `p{n}.s` | 0 … 5 | curve (p1: hold) | the segment's shape: curve, S, hold, stairs, pulse, wave |

A point's number never changes once it exists: removing one frees its number.

## The editor

- **Drag a point** to move it (it lands on the cycle's sixteenths; hold Shift to place it
  freely); **double-click** an empty place to add a point, a point to remove it; **drag a
  segment** up or down to bend it; the **Point** card sets the selected point's segment
  shape and tension. Each gesture is one undo step.
- The line is the curve you edit; the filled area is what is heard (the depth in it); the
  playhead is the song's; the corner says the length and the tempo ("1/4 · 128 BPM").

## How it works

- The phase is the family's synced LFO phase (song sample × cycles per sample + offset); the
  curve is read between the points sorted by place, wrapping from the last point to the first
  one a cycle on.
- At the start of playback (a new song epoch) the level is set on the curve where the song
  is, not glided there: a render or a live play from mid-cycle starts exactly where the full
  render is at that point, to the bit (review 1).
- The level never depends on the audio, so silence in gives silence out; `skip` runs the
  level's glide sample by sample as processing would, and the unit is at rest whenever its
  depth and gain glides have landed.

## Factory presets

Pump on the quarters · Pump, hard · Pump, soft · Pump on the eighths · Half-time pump ·
Swell over a bar · Gate, sixteenths · Off-beats only · Triplet chop · Slow wave.

## What the tests hold

- At its defaults and at depth 0 with any curve, the render is the render without it, to
  the bit; at full depth the lead is silent on every beat of the song and untouched (to the
  bit) through the held half, from the start and from beat 3; the same bits at blocks of 1,
  7, 64, 1 000 and 4 096; live equal to offline (`fx_fluctus.rs`).
- The DSP: the curve read between points and wrapped; the pump on every beat; 2 ms of
  smoothing takes the wrap's step from 1 to under 0.02 per sample; a render from bar 5 mid-cycle
  equal to the full render there to the bit, from its first sample; split invariance;
  `skip` exact.
- The engine: a segment bends as an automation clip does (`Shape::at`, to the bit).
- The editor: double-click adds and removes a point, a drag moves one onto the grid — one
  undo step each; the level drawn is the level played (within 1e-4); every preset in both
  languages.
- The family's contract and the CLAP/VST3 export, bit-identical, depth automated.
