# Round 33 — mpe: MPE end to end (ADR 0080, schema 47, sandbox protocol 7)

Branch `r33/mpe` from the contract `e0853ec1`. The decision and its reasons are ADR 0080;
the user's pages are the manual's *MPE: per-note expression* (EN/ES, §4). Shots in
`docs/round33/mpe/` (`cargo run -p cantus-ui --example shoot_mpe -- <dir>`).

## What was done

- **Model (schema 47).** `Note::expr` — bend, pressure, timbre curves, integers at MIDI 2.0
  resolution (`Note` keeps `Eq`), absent on ordinary notes (serialised exactly as before).
  Ops: split/trim/stretch/reverse/join (exact to the integer), scale, smooth, thin, draw,
  flatten, snap. `SetNoteExpr` (named per gesture: `cmd.mpe.*`) and
  `SetInstrumentExpression`, exact inverses, through linked copies; the knife splits the
  curves, every other note operation carries them. A load repairs a hand-edited curve.
  `fixtures/v47.cantus`.
- **Playback.** `TrackSnapshot::expr`, compiled beside the notes (same notes, windows,
  loops, grooves, voice tags); fired last at each sample — split for native units, handed
  ahead with sample offsets to hosted ones.
- **Instruments.** Unda, Calor, Modulus, Gravis glide to each value (τ 4 ms, no zipper,
  first value taken at once, glide-safe); fresh Unda/Calor/Modulus get pressure and timbre
  routes (null without expression). Not this round, with reasons (ADR 0080 §4): Exemplar,
  Chorda, Lamina, the others.
- **Live and recording.** Ports marked MPE in Settings ▸ MIDI (others moved to channel 1,
  nothing changes when none is marked); zones and bend ranges, MCM and RPN 0 read from the
  controller; member notes with their own ids, channels and values to the live track and its
  echoes; while recording the values are bound to their notes and thinned.
- **Hosted.** Routes *automatic / note expressions / MPE (MIDI) / off* (inspector ▸ *Note
  expression*); automatic from what the plug-in declares (CLAP dialects, VST3
  `INoteExpressionController`, member bend mapping) — carried to the sandbox (`PROTOCOL` 7).
- **Files.** MIDI: MPE zones (MCM or guessed) recombined into one track with per-note curves,
  key pressure kept; MPE export as a lower zone (MCM, RPN 0, LRU member channels).
  DAWproject: per-note `transpose`/`pressure`/`timbre` timelines, schema-valid.
- **Piano roll.** Curves on the notes (bend line, pressure weight, timbre colour); three rows
  editing the selected notes (pencil, line, Shift-snap; smooth, thin, scale, flatten, clear).
- **MIDI 2.0.** The model's resolution and the translation layer (Min-Center-Max scaling,
  per-note pitch bend, Pitch 7.25, UMP per-note packets) built; the transport measured and
  sized in ADR 0080 and BACKLOG §0.2.

## How it was tested — the numbers

- Null: no expression / expression only where unheard renders bit-identical; golden hashes
  unchanged, `DSP_VERSION` 5; fresh routes silent without expression; MPE off changes nothing.
- Block sizes 1, 37, 64, 256, 1024: identical to the bit; real time (uneven blocks) = offline;
  parallel (3 workers) = serial.
- Values land on their samples (tick × 25 at 120 BPM/48 kHz) split or ahead, any block size.
- A +7 semitone curve measured from the sound: Unda 700.0 ct, Modulus 700.0, Gravis 700.0,
  Calor 698.2 (its patch's drift). Default routes move the sound by up to 0.315 (Unda), 0.155
  (Calor), 0.111 (Modulus). The test holds the bend within 2 ct of 700.
- A Seaboard-style stream (MCM, two overlapping member notes, bend before note-on, pressure
  after, RPN 0 = 24): each note its own voice, +12/−24 semitones at ±48, +6 at ±24.
- MIDI file round trip within a 14-bit step of bend (0.006 st at ±48) and 1/127 of pressure;
  DAWproject round trip to 1e-6.
- Dense piano roll (96 notes, 300-point vibratos, three rows): 37 088 vertices against 16 976
  without expression, 2.8 ms a frame in a debug build.
- `rt_memory`: expression playback and live MPE allocate nothing.
- Undo fuzz: the round-33 stream applies and refuses expression edits and routes.
- Windows (`RUSTFLAGS=-D warnings`), the crates touched (core, dsp, engine, host, sandbox,
  project, ui): 4 522 passed before the last two fixes (a boot test moved to its own
  binary, `device_ux`'s fresh-instrument expectation now with its MPE routes), those
  binaries then green; workspace clippy clean. Linux (WSL, `check.sh r33/mpe`): clippy
  clean, the workspace **4 900 passed, 0 failed**, 59 ignored.

## Marked lines in files mpe does not own (`// r33:mpe`)

- `cantus-core/src/snapshot.rs`: `TrackSnapshot::expr` (field, doc) and its compile line.
- `cantus-engine/src/graph/track.rs`: `TrackSlot::next_expr` (field, init); the expression
  call before `track_segment` (3 lines under a marker comment).
- `cantus-engine/src/processor_snap.rs`: `next_expr` set on a seek (1 line + marker).
- `cantus-engine/src/processor.rs`: `Processor::mpe` (field, init).
- `cantus-engine/src/processor_msg.rs`: `mod mpe_live` (`#[path]`), `SetMpe` dispatch, the
  `mpe_midi` call at the top of `midi()` (3 lines under a marker).
- `cantus-engine/src/messages.rs`: `EngineMessage::SetMpe`, `EngineEvent::ExprInput`.
- `cantus-engine/src/midi.rs`: `route` reads `eng` (the MPE port filter) for the engine's
  three uses.
- `cantus-engine/src/backend.rs`: `expr_inputs` (field, two inits, the event arm,
  `take_expr_inputs`).
- `cantus-project/src/format.rs`: `SCHEMA_VERSION` 47, the 45→46 placeholder (session's 46
  takes it at merge) and 46→47 migrations with their docs, the repair call, the schema-47
  test.
- `cantus-ui/src/recording.rs`: `RecordedNote::expr`, `Recorder::mpe`, its clear on start,
  `expr: None` in three literals (one in a test).
- `cantus-ui/src/record_session.rs`: the MPE take fed and its notes added (5 lines), and at
  stop (2).
- `cantus-ui/src/record_pass.rs`: `expr: n.expr.clone()` in `new_note`.
- `cantus-ui/tests/rec_pass.rs`: `expr: None` in a literal.
- `cantus-ui/tests/device_ux.rs`: a swapped-in fresh Modulus now equals `with_routes(..)`.
- `cantus-ui/src/settings.rs`: `mpe_ports`, `mpe_zones` (additive, `serde(default)`).
- `cantus-ui/src/lib.rs`: `pub mod mpe`.
- `cantus-ui/src/app.rs`: `publish` at `assemble`, after an audio restart and a rescan; the
  knife's split (`slice_at`).
- `cantus-ui/src/views/workspace_settings.rs`: the MPE band at the end of the MIDI page
  (sync rebuilds it as a per-port table: `mpe::input::port_cell` is the column).
- `cantus-ui/src/swap.rs`, `views/inspector_menus.rs`: a new instrument gets its MPE routes.
- `cantus-core/tests/undo_invariants.rs`: the round-33 stream (`mrng`, `mpe_done`, its
  assert, `round33_mpe_command`) and `expr: None` in one literal.
- Unmarked additive edits in shared files: `command.rs` (two variants, labels, arms),
  `freeze_guard.rs`, `external.rs` (`ExprRoute`, the field), `ui/clips.rs`,
  `views/inspector_track.rs` (`..x.clone()`), `views/inspector_hosted.rs` (the route row).

## What needs the owner

- A real MPE controller (or a phone MPE app) and an MPE plug-in: the feel of bend, pressure
  and timbre (`docs/beta-checklist.md`).

## Review fixes (round 33, after the first hand-back)

1. A port marked MPE but not open publishes **no** layout: every open port is ordinary.
2. MIDI import guesses a zone only on strong evidence — three or more member channels from
   channel 2 with no gap, each one note at a time, none with a program of its own, notes
   sounding together across them, at least half bent — so an ordinary file (a piano, a bass
   and a bent lead; a GM file with drums) keeps its parts.
3. MIDI export: the first expressive track is the lower zone (manager channel 1, members
   2–9), the second the upper (manager 16, members 15–11); channel 10 stays the drums'; the
   other melodic tracks take channels no zone uses (with two zones there are none left and
   they share); a third expressive track is written plain. Lanes go on the zone's manager;
   RPN 0 goes on the zone's members only.
4. A reused member channel goes back to rest first (export: only what the last note left
   elsewhere; hosted MPE out the same, CC 74 included); import reads a note's values only
   from when its channel was let go by the note before.
5. Unda, Calor and Modulus forget the released note's expression when a mono voice glides
   to another note (as Gravis did).
6. Import thins each curve as recording does and slices the samples per note by time: a
   20 s note of 10 000 bends keeps its end.
7. MPE notes are recorded by member channel, key and start (the engine reports their on,
   off and values as `ExprInput`), so two notes of one key on two channels stay two.
8. The manager bend range is kept as the controller says it but not applied — the manager's
   bend goes through the instrument's own range, like any keyboard's; its Settings knob is
   gone.
9. A panic (CC 120/123) on a member channel releases its note; elsewhere it releases every
   MPE note. The MCM and RPN 0 are read once per selection, so a later data entry on channel
   1 (another port's CC 6, which arrives there) is not a new MCM. RPN state is still per
   channel, not per port: the ports share one ring and the engine never sees which port a
   message came from.
10. `join` holds each side over its own part. Only the knife (`slice_at`) is wired among
    the operations that cut a note's time: Cantus has no note-level left trim, time stretch,
    reverse or note join today (clip trims move windows, not notes); `trim_start`, `stretch`,
    `reverse` and `join` are ready for them.

Fix round, tested: Windows (`-D warnings`), the touched crates — 4 533 passed and the boot
test updated for fix 1 (then green); workspace clippy clean. Linux (WSL): clippy clean,
**4 902 passed, 0 failed**, 59 ignored.

## Review round 2

- **D1** Hosted MPE out: after a reset (seek, panic, new route) every member channel's state
  is unknown, so its first note puts bend, pressure and CC 74 back to rest.
- **D2** MPE notes recorded in a frame where the playhead wrapped go to the next pass event
  by event, as the plain notes do.
- **D3** A take starts with the engine's MPE events from before it drained.
- **D4/D5** MIDI export: an MPE zone is sized to the notes its track sounds at once (the
  most polyphonic track gets the lower zone, up to members 2–9; the next the upper, up to
  15–11). The plain melodic tracks need channels of their own: the upper zone shrinks
  first, then the lower; if that is not enough the second expressive track is written
  plain (so is a third). When every member sounds, the oldest note is cut where the new
  one takes its channel, its later values dropped.
- **D6** Without an MCM, MPE is recognised by its signature: at least 2 channels, each
  playing one note at a time, none with a program of its own, at least 4 notes, and three
  in four of them with their channel's own bend, pressure or CC 74 within a 64th of the
  note-on. A rotation may start anywhere and skip channels, and a one-note line counts. An
  ordinary file's parts, which bend now and then, are not merged.
- **D7** While a port is marked MPE, other ports' RPN/NRPN selections and data entry
  (CC 6, 38, 98–101) are dropped in their callbacks; their CC 120/123 arrive tagged
  (`PLAIN_PANIC`) and release only the plain live notes.
- **D8** A recorded value is not reported when fewer than 64 slots of the engine's event
  ring are free, so a note's on and off always fit.
- **Nit** On import, a value on the very tick a channel was let go belongs to the note let
  go, unless the next note starts on that tick.
- Timbre's rest is now MIDI 2.0's centre (`1 << 31`, where CC 74 at 64 lands), and a
  pressure of 0 held at a note-on is no curve. `Zone::master_bend` is no longer written.
- No shot showed the master-bend knob: the round's shots are of the piano roll only (the
  Settings ▸ MIDI band was never shot), so there is nothing to re-shoot.

Round 2, tested: Windows (`-D warnings`), the touched crates — **4 543 passed, 0 failed**;
workspace clippy clean. Linux (WSL): clippy clean, **4 919 passed, 0 failed**, 59 ignored.

Integration review: expression edits (`command_for`) and the load repair reach session clips (`Track::clip`, slots) — tests `an_expression_edit_reaches_a_session_clip`, `repair_reaches_session_clips`, `mpe_session.rs`; the undo fuzz draws slot-clip notes too. cantus-core + cantus-project: 743 passed, 0 failed.
