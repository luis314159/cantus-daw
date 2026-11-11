# Delay — the stereo delay

*Effect kind `Delay`. References: FL Studio's Fruity Delay 3, Ableton Live's Echo (its
tunnel), FabFilter Timeless, Soundtoys EchoBoy, the tape echoes (Roland RE-201, Echoplex)
— ADR 0115.*

Delay is Cantus's plain stereo echo: a time up to two seconds, feedback, a tone filter inside
the feedback path so each repeat is darker than the one before, and ping-pong between the
sides. Its window draws **the echoes in time**, and the drawing is the control.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/effect.rs` (`DELAY_PARAMS`) |
| DSP | `crates/cantus-dsp/src/delay.rs` (`StereoDelay`) |
| Engine glue | `crates/cantus-engine/src/effects.rs` (`Dsp::Delay`) |
| Editor | `crates/cantus-ui/src/views/fx/delay.rs`, the echoes in `delay_view.rs` |
| Face | `crates/cantus-ui/src/kit/skin/tone.rs` (`DELAY`) |
| Strings | `crates/cantus-ui/locales/fx/delay.{en,es}.txt` (the parameters' in `en.txt`, `es.txt`) |
| Presets | `crates/cantus-ui/presets/effects/delay.toml` (13, ADR 0101) |
| Tests | `delay_view.rs`'s unit tests (the drawing against the real unit), `crates/cantus-ui/tests/delay_window_e2e.rs` |
| Exported | not a CLAP/VST3 plug-in (yet: ADR 0115, Consequences) |

## What the references do well, and what people complain about

- Most delays show knobs only; the user hears the repeats but never sees where they fall,
  how fast they die or which side they are on. Echo's tunnel and Timeless's display show
  them; the tunnel is a picture, not a measure.
- A synced time is what most people want on a vocal; a millisecond readout with no note
  beside it means reaching for a calculator.
- A tone knob in the feedback path darkens each repeat a little more, which is the sound of
  tape — but no display shows that the fourth repeat has lost most of its air.

Delay answers: every repeat drawn at its time, at its level, on its side, in a colour that
cools as the tone takes its highs (the filter's own loss at 8 kHz, once per pass); a drag
on the drawing sets the time and the feedback; the note value at the song's tempo beside
the time; and *What is heard* in numbers — the repeats to −60 dB, the tail, the loss per
repeat.

## Parameters (keys are forever)

| Key | Range | Default | Meaning |
|---|---|---|---|
| `time_ms` | 1 … 2 000 ms | 375 ms | the time between repeats (a change glides: a sweep bends the pitch) |
| `feedback` | 0 … 95 % | 40 % | each repeat's level against the one before |
| `tone_hz` | 200 Hz … 20 kHz | 6 kHz | the one-pole low-pass in the feedback path |
| `ping_pong` | Off / On | On | the sides' sum into the left, the feedback crossed: repeats alternate |
| `mix` | 0 … 100 % | 30 % | the repeats against the dry sound |

The time has no sync: *At the tempo* sets it once from a note value and says so (ADR 0115 §3).

## The window

- **Echoes** (the display): the left side above the centre, the right below; the sound at 0;
  each repeat a stem at its time, its level relative to the first drawn as its square root;
  dashed, the decay through the heads and the feedback's level in each lane. A drag across
  moves the first repeat (the time), up and down moves the level lines (the feedback) — one
  step of undo; Shift is fine, a double-click the defaults. While sound plays each repeat
  glows with what the input was its time ago; at rest nothing moves and the window asks for
  no frame.
- **What is heard**: repeats to −60 dB, the tail, each repeat's loss at 8 kHz, the first
  repeat's side, and how the sides repeat.
- **Time** (the XL knob and *At the tempo*), **Repeats** (feedback and tone), **Output**
  (ping-pong and the mix).
- The face: a tape echo at night — oxblood vinyl, the echoes on a phosphor screen in coral
  cooling to rust, skirted knobs, a rocker, an engraved name, screws (ADR 0115 §5).
