# Lapsus — the time shaper

*Effect kind `TimeShaper`. References: FL Studio's Gross Beat (the function), Ableton's Beat
Repeat, Sugar Bytes' Effectrix, a turntable and a tape machine (the physics). ADR 0132
(provisional).*

Lapsus bends **time** and **level** to the beat: tape stops and starts, half time, stutters
and rolls, reverse, scratches, gates and pumps. As Gross Beat does, it holds two banks of
thirty-six patterns — **time** patterns and **volume** patterns — chosen apart and played
together, each by its knob (automatable), by a note of its own range, or by a click on its
pad. Each pattern is a length (¼ to 4 bars) and a curve drawn as Fluctus's and the
automation clips are (points, tension, the six segment shapes), on a cycle that follows the
**song position**.

| Layer | File |
|---|---|
| Parameters and banks | `crates/cantus-core/src/fx/lapsus.rs`, the factory banks in `lapsus_bank.rs` |
| DSP | `crates/cantus-dsp/src/lapsus.rs` (the unit), `lapsus_run.rs` (each sample), `lapsus_read.rs` (the history and its kernels), `lapsus_curve.rs`; tests in `lapsus_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/lapsus.rs` (the banks loaded slot by slot; the history by the memory path) |
| Editor | `crates/cantus-ui/src/views/fx/lapsus.rs`, the timeline `lapsus_grid.rs`, its gestures `lapsus_edit.rs`, the banks `lapsus_bank.rs` |
| Command | `Command::SetLapsusPattern` (one slot of one bank; `None` the factory's) |
| Strings | `crates/cantus-ui/locales/fx/lapsus.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_lapsus.rs`, `crates/cantus-ui/tests/fx_lapsus.rs`, `lapsus_e2e.rs`, the family's contract, `rt_memory`, the CLAP/VST3 tests and `notes.rs` |
| Exported | `Cantus Lapsus` (CLAP `com.github.lalmazan.cantus.lapsus`), its banks in its state |

## What the references do well, and what people complain about

- **Gross Beat** is the standard: a time envelope and a volume envelope per slot, 36 of each,
  switched by keys; the time envelope's vertical axis is how far back in the buffer it reads.
  Complaints: clicks where the envelope jumps unless smoothed by hand; a tape stop that holds
  a buzzing sample at its end; the text-only slot list (you read names, not shapes).
- **Beat Repeat** repeats slices by chance, with a pitch decay; **Effectrix** sequences
  loopers, scratches and reverses on a grid. Both are excellent at performance, neither lets
  you draw the time itself.

Lapsus keeps Gross Beat's model and draws the slots as their shapes; every jump of the time
curve is crossfaded (2 ms by default, measured: thirty times smoother than a hard join); a
tape stop's level falls with its speed (the *Tape stop* presets pair it with its fade); and
it is exact — a render from any bar is the whole render there, to the bit.

## Parameters (keys are forever)

| Key | What | Range | Default |
|---|---|---|---|
| `tslot` | the time pattern playing | 1 … 36 | 1 (*Through*) |
| `vslot` | the volume pattern playing | 1 … 36 | 1 (*Through*) |
| `mix` | the shaped signal against the dry | 0 … 100 % | 100 % |
| `xfade` | the crossfade where the time curve jumps | 0 … 20 ms | 2 ms |
| `smooth` | the level's glide | 0 … 20 ms | 1 ms |
| `notes` | what a note does: off, switch, hold | | hold |
| `retrig` | a note starts its pattern there | off / on | off |
| `tkey`, `vkey` | the notes of pattern 1 of each bank | 0 … 92 | C4, C1 |
| `ahead` | lookahead: the time curve may read the future (its latency) | 0 … 250 ms | 0 |
| `out` | output | ±24 dB | 0 dB |

Every default leaves the signal untouched, to the bit. An insert starts on *Half time*
(time 8, half time every half bar).

## How it works

- **The time curve's value is the delay** behind the live signal in pattern lengths: flat at
  the top is the music as it arrives; a line down at the diagonal is the tape stopped; half
  that slope is half speed; steeper is reverse; a held step is a repeat. A whole-sample delay
  reads that sample exactly (the null).
- **Nothing before the cycle's start is read** — the display shades that region, *Before the
  cycle: silence* — so a cycle never depends on what played before it.
- **The interpolator** chooses by how far behind now it reads (the future is not written
  yet): a line, a four-point Hermite, a 32-tap Kaiser sinc (−95 dB at 1 kHz), crossfaded by
  distance; read faster than real time, the sinc's cut-off follows the speed (a 15 kHz tone
  read at 2× is −93 dB of what would fold).
- **Joins**: inside a cycle the old read carries on and fades out; at the cycle's end the
  fade is placed before the line, so the next bar starts on its line at full weight.
- **The history** is one bar of the time bank's longest pattern at 60 BPM, built off the
  audio thread.

## The window

The **timeline** draws both patterns over the longer cycle: the time curve above (over the
cycle's input, faint), the level below, and while it plays the playhead and **the slip** — the
line from where the song is to where Lapsus is reading it, marked on the ruler. Drag a point
(Shift: off the grid), double-click to add or remove one, drag a line to bend it; the
**pencil** paints steps a grid cell at a time. The **banks** card shows one bank at a time
(*Time* / *Volume*, each naming what it plays), its pads drawn as their curves; a click plays
a pad, a right-click gives it the factory's pattern back; the keyboard under it shows both
note ranges. Every edit is one step of the project's undo.
