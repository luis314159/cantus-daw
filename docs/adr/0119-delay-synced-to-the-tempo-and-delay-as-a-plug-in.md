# ADR 0119 — Delay synced to the tempo, and Delay as a CLAP/VST3 plug-in

**Status:** accepted (round 40, the `delay` builder) · **Date:** 2026-11-12 · **Schema:** none
(two parameters in the unit's own state) · **Extends:** ADR 0115 (Delay's window), ADR 0020
(our plug-ins outside Cantus), ADR 0022 (memory built off the audio thread), ADR 0060 (the
tempo track) · **Amends:** ADR 0115 §3 (*At the tempo* set the time once; it is the sync now)
· **Amended:** round 40's mid-round review (`fix-a`, 2026-11-12) — §3 *the sync switched on*,
§4 *a long line a stretch per call* and *an automated Delay's line*, the Consequences

## Context

The owner's answer 22 to round 39 (2026-11-12): *Delay's time synced to the tempo* and
*Delay as a CLAP/VST3 plug-in*. Round 39 had left both to him: Delay's time was milliseconds
only — *At the tempo* set it once from a note value and said it did not follow a tempo change
— and Delay had never been one of the exported plug-ins (`cantus_plugins::desc::ALL` had no
Delay; ADR 0115 §Consequences).

What had to hold: the sync off by default and **null at its default, to the bit**
(`DSP_VERSION` stays 6, no golden hash moves); block-size independence at tolerance 0; offline
equal to real time; the time following tempo changes and the tempo track with no zipper and no
click; the audio thread allocating nothing (ADR 0005); the plug-in bit-identical to the
built-in unit; **its ids forever**.

## Decision

### 1. Two parameters in the unit's state

`DELAY_PARAMS` gains `sync` (0/1, default 0) and `note` (0 … 26, default 10). A project saved
before reads them as their defaults (`EffectSpec::get`), so no schema bump and no migration:
an old song plays as it did, to the bit (`fx_delay_sync::the_sync_off_renders_as_a_delay_saved_before_round_40`).

**The note values are forever** (a project, an automation lane and a plug-in's state hold
the index): `note = 3 × base + feel`, the base from 1/64 (0) to 4/1 (8) — four whole notes,
four bars of 4/4 — and the feel straight (0), dotted (1) or triplet (2). In quarter notes,
`2^base / 16 × (1, 1.5, 2/3)` (`cantus_dsp::delay::sync::beats`). The default, a dotted
eighth, is 375 ms at 120 BPM: the free time's default, so turning the sync on at 120 BPM
moves nothing. Their names (`delay.note.*`) are the same words in both languages, as the
modulation family's (`1/8·`, `1/8T`). The choices are the plug-in's lists (`DELAY_CHOICES`:
`sync` reads *Free · Sync*, `note` the 27 names).

### 2. The sync off is the unit it was, instruction for instruction

`StereoDelay::process` runs the free time through the exact loop it always ran
(`process_free`); the synced time has its own (`process_synced`). The sample's pass through
the feedback path is one `#[inline(always)]` function both call, its float operations in
the order they were. `delay_tests::the_sync_off_is_the_unit_before_round_40_to_the_bit` keeps a
frozen copy of the round-39 unit and compares the two to the bit, through a time change and
with the tempo told and a far note value (neither may move a sample while the sync is off).
The engine's golden hashes and `null-hashes` stand.

### 3. The synced read head

The engine tells a unit that follows the song the tempo of each pass
(`SongPosition::tempo_bpm`; under a ramp one value per step of `RAMP_STEP` ticks, each
starting a pass — ADR 0060 §4), so the same samples hear the same tempo at every block size.
`EffectUnit::follows_song` is true for Delay **only while synced**, so with the sync off its
passes are cut exactly as before. The head (`sync::Head`, in `f64`: a long line's head moves
by fractions of a sample, never in steps) moves three ways:

- **inside a ramp** (`tempo_inc ≠ 0`), straight to each step's time over the step (a
  sixteenth of a beat at the step's tempo): the time is the ramp's, piece by piece, and the
  pitch bends as a tape's would whose speed follows the ramp — a few per cent on a musical
  ramp;
- **a small move** (a host's tempo creeping: a bend under 3 % over 50 ms), straight there
  over 50 ms;
- **a jump** (a tempo marker, a new note value, the sync turned on), a **crossfade** from
  the old head to the new over 50 ms. A glide there would race the head (a 14 000-sample move
  in 50 ms is the past played 28 times as fast: the first try did exactly that, and the test
  that holds the output's steepest step to the sine's caught it). A move that comes during a
  crossfade waits for its end.

While the lines hold only silence a new tempo is taken at once: nothing can be heard moving,
and a unit at rest stays at rest — the engine tells a resting strip's inserts the position
and skips them, and both paths (told then skipped, told then run on silence) do the same.

The synced read (`sync::read`) takes the whole and the fraction of the delay alone, so it
reads the same history the same whatever the line's length or where its write position is.

**The sync switched on** (amended, `fix-a` A-L1). With the sync off the unit does not follow
the song, so nobody tells it a tempo and the one it holds goes stale (120, or the last it
played synced). Turned on, the head therefore does not go to the note at that tempo: it waits
(`arm`) for the tempo of the next pass — told before the pass, as to every unit that follows
the song — and crossfades **once**, to the note at that tempo. A unit never told one (a host
with no transport) goes at its next pass on the tempo it has.

### 4. The line grows with the note

The free line is 2 s, as always. A synced note needs `note × 60 / 20 BPM` (the slowest tempo a
song can have, `MIN_BPM`), in whole seconds, between 2 and **48 s** (4/1 at 20 BPM; a dotted
4/1 under 30 BPM is held at the line's end, and the window says so). So no tempo change and
no tempo track ever finds the line short. It is built **off the audio thread** on ADR 0022's
path: `MemoryNeed::DelayLine(cells)` → `EffectMemory::DelayLine(Lines)`; `EffectUnit::new`
builds it in place (a unit made with the sync on starts on its line), a snapshot that turns
the sync on asks once (`EngineEvent::EffectNeed`) and the backend answers, an offline render
answers before its first block. When the line arrives the unit takes its history (the
newest of the old, as much as fits; nothing when the old held only zeros) and takes its
write position from the samples it has run, so **a line that grows mid-song changes no
sample** (`delay_tests::a_line_that_grows_mid_song_changes_no_sample`). Meanwhile the
synced time is held at the old line's end. Turned off, the unit asks for the free line back,
and once it has it and the old repeats have died away it is as good as new (at rest exactly:
ADR 0031 §5.1). A 4/1 line is 18 MB at 48 kHz — paid only by a Delay synced to a whole-bar
value.

**A long line a stretch per call** (amended, `fix-a` A-M3). The history was first copied in
one pass when the line arrived, and a reset zero-filled a line that held sound: on a 48 s line
that is 23 ms and 1.2 ms in one callback (measured), an xrun. Neither is done whole now
(`delay_line.rs`):

- a line offered while the old one holds sound waits beside it; before each call the unit
  copies the frames the call will process plus `MAINTAIN_CELLS` (32 768) more of the history,
  oldest first, in `memcpy` runs, and plays on the old line meanwhile. When the copy has
  caught up, the lines are swapped before that call runs (the new one holds what an
  immediate copy would), and the old one waits in the unit until the engine takes it after
  the block (`EffectUnit::take_spent`, `Processor::sweep_spent` while a unit is busy; the
  plug-in's builder takes it the same way) and frees it off the audio thread. A line that
  holds only silence is still swapped at once.
- a reset of a line that held sound leaves it as it is and marks the cells not written since
  (from the write position up): the reads of a call made while any are left read them as
  zeros (a masked copy of the loops; the unmasked loops are the instructions they were), and
  each call clears the frames it processes plus `MAINTAIN_CELLS` more from the top; skipped
  frames clear what they pass. The output is a cleared line's, to the bit.

A call's work is at most `2 × (frames + 32 768)` cells for each of the two
(`StereoDelay::work`); a 48 s line is moved or cleared in some 70 calls.

**A second reset** (amended, `fix-c` R2-A1). A reset of a loud line says the line is silent
(its reads see zeros) while the cells above the write position still hold the old sound
until cleared. A second reset before the clearing ends — freeze then unfreeze (a resting
strip does not run, so nothing is cleared meanwhile), two panics, a plug-in host's reset
followed by a value set before the first sample — took the line for silent and read the old
sound back unmasked. A reset now keeps a line stale while any of it is (`stale > 0`): what
follows is a new unit's, to the bit, with any number of calls between the resets.

**The free time on a long line** (amended, `fix-c` R2-A3). A line kept longer than the free
2 s one (a floor, below, with the sync off) read the free time as `write − delay` in `f32`,
whose step on a 48 s line is ¼–½ of a sample. On such a line the free time is read by the
delay's whole and fraction as the synced head is (`sync::read`, independent of the line's
length); on the free 2 s line it is the instructions it always was (the null, to the bit).

**An automated Delay's line** (amended, `fix-a` A-M2). Its `note` and `sync` can be driven by
a lane (automation, a modulator, a MIDI-learned control's lane), which configures the unit
between passes, never through a snapshot — so the line it needed was never asked for: the
time stayed held at the old line's end until an unrelated edit, offline as live, and the unit
never came to rest. Now:

- **the line is built for the lanes before the song gets there.** The automation's runtime,
  built off the audio thread with the snapshot, works out for each Delay whose `note` or
  `sync` a lane drives the note with the longest line the lanes can reach while synced
  (`FxCopy::line_note`: each lane's base and its pieces' ends — a curve stays between them —
  moved as far as the modulators driving it can move it: `Σ |depth| × amount` in the
  normalised domain, one side for a unipolar source; amended, `fix-c` R2-A3 — it was the
  lane's whole range, so a modulator of any depth on `note` reserved the 48 s line, 18 MB a
  side); installed, it is the unit's floor
  (`EffectUnit::set_line_floor`) and the line is asked for at once — an offline render
  answers before its first block. A note automated longer then never waits for its line, and
  **offline equals real time**. The floor holds with the sync off too, so switching the sync
  by a lane asks for nothing (the free time runs on that longer line meanwhile; a Delay
  nobody automates keeps the free line, the null).
- **after automation configures a unit** its memory need is asked for at once
  (`msg::ask_memory`), so anything the runtime could not foresee is asked for while the song
  plays; an offline render answers those too (taken at its next block), and so does a
  listen (`listen::Rig::answer`, amended `fix-c` R2-A2 — it dropped them, so agents, the
  mastering assistant and checkpoints heard another song than the export once an EQ's mode,
  an Aula response or a Delay's note beyond its floor was automated). A checkpoint kept
  while such an answer is on its way carries it, and a section resumed from it is sent it.
  Such an answer is taken at the next block, so where the old path ends depends on where
  the blocks fall: a listen's checkpoints therefore sit on the export's block boundaries
  (a whole number of blocks from the song's start, and on the control grid), and its
  blocks are the export's; a section resumed from a checkpoint trims the latency its
  lineage's render began with, as the export does. What this leaves, and was so before: a
  need the runtime could not foresee switches at the next block's start — offline at
  another block size, or in real time, a block or two apart — and an effect whose latency
  grows there (an EQ made linear phase) moves the export's later audio by the difference. A Delay's `note` and `sync` lanes are foreseen (the
  floor above), so for Delay offline still equals real time.
- **a unit that starts or stops following the song** (the sync switched by a lane) re-plans
  the passes its tempo steps cut (`scope_delays`): a Delay behind a latent insert hears each
  step of a ramp at its own lateness, the same at every block size.

### 5. *At the tempo* became the switch

Round 39's *At the tempo* set the time once from a note value and said it did not follow. It
**is the sync now**, not a second control beside it: two ways to put a note value on the time
that behave differently after a tempo change would be a trap, and the set-once is still there
as *Sync, then Free*. The time card's column is *Tempo*: the switch *Free · Sync* (the
modulation family's words and segmented control), the note value's field (each value with its
time at the song's tempo now), and one sentence — *375 ms at 120 BPM, following the tempo*,
or *In milliseconds; a note value follows the tempo*.

- **Sync** picks the note value nearest the time it was playing (by ratio), one step
  (`cmd.delay_sync_on`); choosing a note in the field turns the sync on with it.
- **Free** keeps the time it was playing, in milliseconds (`cmd.delay_sync_off`): the sound
  does not jump, and the old set-once is this.
- The **XL knob** turns the note value while synced (the modulation family's rate cell does
  the same), milliseconds while free.
- **The echoes** are drawn at the note's time at the song's tempo, and a drag across picks the
  note value nearest the pointer (one step, `cmd.delay_echoes`, as before).
- The inspector's strip names the note (`1/8· · 42%`).

The factory presets named by a note value — *Eighth echo*, *Dotted eighth*, *Quarter throw*,
*Trap vocal*, *Sixteenth bounce*, *Triplet* — follow the tempo now; their `time_ms` is the note
at 120 BPM, what the time is when the sync is turned off.

### 6. Delay as a CLAP/VST3 plug-in

Delay joins the exported set as the 25th (`desc::ALL`), with the feature `delay`, bundled with
its editor (`xtask`: `delay,editor`). Its window is the DAW's own on `FxHost` (ADR 0115 made it
ready): `editor::has_editor` is true for it now. Inside another host the synced time follows
that host's tempo (`Processor::position`, as the modulation family's units do), and the longer
line is built by the plug-in's own memory thread (`cantus_plugins::memory`, Aula's, now for
Delay too). **The ids, forever, by `desc.rs`' rule:**

| | |
|---|---|
| Name | `Cantus Delay` (slug `delay`) |
| CLAP id | `com.github.lalmazan.cantus.delay` |
| VST3 class id | `625C4F90AB5A380458F1F1D3D1DCA44C` (SHA-256 of `cantus/vst3/delay`, the first 16 bytes) |
| CLAP features | `audio-effect`, `delay`, `stereo` · VST3 `Fx|Delay` |
| Parameters (FNV-1a of the key, top bit clear) | in `crates/cantus-plugins/tests/golden/ids.txt` under `delay`: `time_ms`, `feedback`, `tone_hz`, `ping_pong`, `mix`, `sync`, `note`, `cantus.sidechain`, `cantus.bypass` |

## Consequences

- Delay is the 25th exported plug-in; the DAW's own Delay and the plug-in are one code path
  (`EffectUnit`), held bit-identical by the family's equivalence tests.
- `EffectUnit::set_position` reaches Delay (it reached only units behind the contract);
  `follows_song` is a function of its settings.
- Two parameters a song file writes for every Delay from now on (`sync`, `note`); an older
  Cantus reading a newer song ignores them (unknown keys are ignored) and plays the free time.
- A Delay synced to a long note costs its line's memory (a quarter 1.1 MB, a whole note
  4.6 MB, 4/1 18 MB at 48 kHz); the free line is the 768 KB it was.
- **Inside another host** the longer line comes from the plug-in's memory thread a moment
  after a long note is chosen; until then a synced time over 2 s is held at 2 s. A synced
  time the free line holds (a quarter above 30 BPM) is the same to the bit whenever it comes.
- ~~Automating the note to a longer value while a song plays asks for the line at the next
  snapshot: until then the time is held at the line's end.~~ (Amended, `fix-a`: the line is
  built for the lanes with the snapshot, before the song gets there, and asked for after
  any configure; §4.) A Delay whose note or sync is automated holds the line of the longest
  note its lanes reach for as long as they are in the song, the sync off included.
- No call moves or clears more than `2 × (frames + 32 768)` cells of a line; a long line
  arrives over some 70 calls, during which the synced time is held as while it was on its
  way.

## What the tests hold

- `cantus-dsp` `delay::tests` (13): the free time to the bit against the round-39 unit; the
  note values promised; the line each note needs; each note's echo at its time at the tempo;
  through a ramp and a jump the same bits at block sizes 1 … 4 096; no click, zipper or sweep
  through a ramp and a jump (the steepest step held to the sine's); the head straight inside a
  ramp's step; a line that grows mid-song changes no sample; held to the line until the longer
  one comes; at rest a new tempo is taken at once and skipping is exact; back to the free line
  it is as good as new.
- `cantus-engine` `tests/fx_delay_sync.rs`: through the tempo track's ramp and a jump every
  block size (1, 37, 64, 333, 512, 4 096) renders the same to the bit; offline equals real
  time; the echo is a quarter at 120 and at 90 BPM across a tempo change, the free time stays;
  a song saved without the keys renders as the sync off, and no note value moves a sample while
  it is off; a whole note turned on while playing asks for its 12 s line once and plays a whole
  note late once it has it.
- Amended (`fix-a`): `delay::tests` — the sync switched on crossfades once, to the tempo it is
  told (and, never told, at its next pass); a 48 s line's history moves in, and a reset
  clears it, at most `2 × (frames + 32 768)` cells a call, the output the same to the bit as
  the unit with the long line from the start and as a new unit. `fx_delay_sync.rs` — a note
  automated from a quarter to a whole note at 60 BPM echoes a whole note late (12 s, not the
  quarter's 3 s line's 11 s), real time equal to offline to the bit, the quarter's line given
  back; the sync switched on by a lane behind a 5 ms look-ahead, through the tempo track,
  the same at every block size. `rt_memory.rs` — `a_synced_delay_never_allocates`: a loud
  Delay synced by a snapshot (its line moved in), automated to 4/1, a panic, a seek, a stop.
- `cantus-plugins`: Delay in every family test (bit-identical through CLAP and VST3 to the DSP
  and to the engine's unit, any block size, automation on the grid, state both ways, the
  bypass, every sample rate, the ids' golden file, the editor opens and wears its face);
  `transport.rs`: a synced Delay follows the host's tempo bit for bit with the engine's unit,
  through a tempo change.
- `cantus-ui`: `delay_window_e2e` (through `CantusApp::assemble` and pointer events) — a
  quarter from the note list turns the sync on as one step; *Sync* picks the nearest note, the
  knob reads it, a tempo change moves what is said, *Free* keeps the time, a drag steps the
  note; the window in Spanish says *Libre · Al tempo*. `delay_sync_e2e` — the switch through
  the window's host on an assembled app, and the song rendered through the engine's compile:
  the echo follows a tempo change, the free time does not; undone, the render is as before.
- `fix-c` (the second review): `delay_tests::a_second_reset_never_brings_the_old_line_back`
  (a loud free 2 s and synced 48 s line reset twice, 0, 1 or 7 calls between: a new unit's
  output to the bit), `fx_delay_sync::two_panics_at_once_clear_a_delay_as_one_does` (free and
  synced, through the processor), `transport::a_host_reset_then_a_value_set_at_the_start_leaves_the_delay_empty`
  (the plug-in's host reset and a value before the first sample: a fresh processor's output);
  `delay_tests::the_free_time_on_a_long_line_keeps_its_fraction` (46 s along a 48 s line, an
  echo 4 800.3 samples late in the ratio 0.7 : 0.3, not the `f32`'s 0.75 : 0.25);
  `fx_delay_sync::a_modulated_note_reserves_only_the_line_its_modulator_reaches` (an LFO a step
  either side of a quarter: no line beyond the notes it reaches, offline equal to real time);
  `listen::memory_asked_for_mid_song_is_answered_as_the_export_does` (an EQ made linear phase
  and its resolution raised by lanes, one starting just before a checkpoint: the listen is
  the export to the bit, and each section from its checkpoint too).
