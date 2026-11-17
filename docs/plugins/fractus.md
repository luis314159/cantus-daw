# Fractus — glitch sequencer and grain cloud

*Effect kind `Glitch`. References: Sugar Bytes' Effectrix and Turnado, iZotope's Stutter
Edit, Ableton's Beat Repeat, Output's Portal, FL Studio's Gross Beat (Lapsus's job, not
this one's) — read about, nothing copied (ADR 0140, provisional number).*

*Fractus* is Latin for *broken*. Ten modules, each painted on its own lane of a step
sequencer that follows the song, and a grain cloud beside them; held notes can play the
lanes instead of (or as well as) the steps. Every choice is a function of the song's sample
and the seed, so a render from bar 5 is the whole render from there, and every join is
crossfaded (−62 … −71 dB of splatter against a hard cut's −3 … −14).

| Lane | Module | What an event does |
|---|---|---|
| 0 | Repeat | the first slice plays and is caught; each repeat replays it, `rpitch` higher and `rdecay` quieter, gated by `rduty`; `rramp` 1–3 halves each part (the stutter build) |
| 1 | Reverse | each slice plays the slice before it backwards (`vslice`: the step, or a note value) |
| 2 | Tape stop | the speed falls to still over `ttime` (or the event), or rises from still (`tkind`) |
| 3 | Shuffle | each slice plays one up to `srange` back, some backwards (`srev`) |
| 4 | Pitch jump | ±24 st, the classic four-head warble or clean (`pstyle`) |
| 5 | Crush | bits and a held rate |
| 6 | Filter sweep | low, high or band pass swept `ffrom` → `fto`, bent by `fcurve` |
| 7 | Gate | pulses of `grate` open for `gduty`, shut to `gdepth` (`grate` 0: shut for the event) |
| 8 | Delay throw | the event's sound into a synced ping-pong delay; the echoes ring after it |
| 9 | Cloud | the cloud replaces the chain for the event |

**The null**: nothing painted and the cloud at 0 — the output is the input, to the bit.
No latency.

| Layer | File |
|---|---|
| Parameters, lanes | `crates/cantus-core/src/fx/fractus.rs` (52 keys; `Pattern`, `Block`, `sanitize`, `paint`, `erase`; the lanes in a preset's flat map: `to_flat`, `pattern_of_flat`) |
| Command | `Command::SetFractusLane` (one lane replaced; its inverse the lane it replaced) |
| DSP | `crates/cantus-dsp/src/fractus*.rs`; tests `fractus_tests.rs`, `fractus_tests2.rs` |
| Engine glue | `crates/cantus-engine/src/fx/fractus.rs` (settings, the lanes written in place, `MemoryNeed::Fractus`: the 8.6 MB DSP built off the audio thread) |
| Editor | `crates/cantus-ui/src/views/fx/fractus.rs` (four pages of cards, the page keys), `fractus_display.rs` (the chassis and its windows), `fractus_paint.rs` (the pointer: paint, chance, erase, Frozen), `fractus_scope.rs` (the grain cloud), `fractus_art.rs` (pads, LEDs, digits, keys, finishes) |
| Face | `crates/cantus-ui/src/kit/skin/sequencers.rs` (the palette), `side_fxa.rs` (`FRACTUS` colourways) |
| Strings | `crates/cantus-ui/locales/fx/fractus.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/glitch.toml` (eighteen, their lanes among their values); starts on *Last beat stutter* |
| Tests | `crates/cantus-engine/tests/fx_fractus.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/fractus_e2e.rs`, `fractus_drag.rs`, `fxa_whole.rs`, `learn_everywhere.rs`; `crates/cantus-plugins/tests/fractus.rs`, `transport.rs`, `vst3.rs`, `notes.rs`, `clap.rs` |
| Exported | `Cantus Fractus` (CLAP and VST3) |

## The window

- **The lanes** (the big window, behind glass): ten rows of step pads, a pad lit from its
  floor to its block's amount, the block's start notched, its amount and chance in words when
  they are not whole; the playhead's column; the lane sounding now lit; a held note's key
  tagged on its lane; a LED per step and the seven-segment counter.
- **Painting** (`fractus_paint.rs`): press on a step and drag along the lane — a block from the
  press to the pointer, **its height the amount** (5 % steps); **Shift-drag** a stroke for
  chance over the blocks it passes; **right-click** erases a block. A gesture is one step of
  undo (`FxHost::set_fractus_lane`, merged by the gesture).
- **The grain cloud**: a scope's graticule, each grain drawn as its window, at its pitch,
  with its trail; the spread's band, the scale's degrees, the position and its spray;
  **Frozen** a lamp and a switch (a click freezes or thaws the cloud).
- **The keys that play the lanes**: ten keys from the first lane's, lit while held.
- **Four pages of cards** (*Sequencer*, *Slicing*, *Colour*, *Grain cloud*), one row each at
  1340 points in both languages; a lane's name or a page key opens a page (a view: nothing
  to undo).

## Presets carry their lanes

A preset is a flat map of numbers; a Fractus preset writes its blocks in it as
`"{lane}.{at}" = len` (`"repeat.12" = 4`), with `.amount` and `.chance` when they are not
whole. Loading one (the browser, the header's list, A/B, a user's own) sets the values and
the lanes in **one** step of undo; a map without blocks clears the lanes. The drift mark
(the preset's name in italics) compares the lanes too; the audition paints them; the data
check accepts lane keys and their ranges (`preset_data::check`).

## Exported

`Cantus Fractus` keeps its lanes as content in its state (`"fractus"`, the project's own
JSON), posts them to its processor as Lapsus's banks are posted (`ContentPost`: built on the
main thread, swapped in at the next block, the old ones handed back — nothing allocated or
freed on the audio thread), and starts a fresh instance on the starting stutter. Its DSP is
built at activation, whatever its settings, so a lane painted, the cloud raised or a note
sent later never builds anything on the audio thread. Its steps follow the host's transport
(CLAP's `clap_event_transport`, VST3's `ProcessContext`), bit-identical to the DSP given the
same song position at any block size; the notes the host sends (CLAP in either dialect, a
VST3 event bus) play the lanes at their sample. Ids blessed in
`crates/cantus-plugins/tests/golden/ids.txt`.

## Not a pedal (yet)

A Fractus pedal on Tabula would play only the lanes it was inserted with:
`Command::SetFractusLane` addresses an insert, not a pedal, so nothing could be painted on a
board; and a board hears no notes, so *Played by: Notes* and the presets built on it would
do nothing. It stays off the board (`pedal_allowed` false) until both are routed (ADR 0140,
phase B).

## Ours, better

- Repeats replayed to the bit, the stutter build, joins measured against hard cuts.
- Painted with the pointer as a sequencer is: height for amount, a stroke for chance.
- Steps, notes, or both; a grain cloud with octave copies so pitched grains do not alias.
- A render from any bar is the whole render from there.
