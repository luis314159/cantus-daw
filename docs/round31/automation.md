# Round 31 — automation (item 1): automation modes and the piano roll's lanes

Branch `r31/automation`; ADR 0068; schema 42; keymap version 12 (Alt+Shift+W cycles the selected
track's mode). What was asked (BUILDING.md, "automation — item 1"), what was done, how it
was proved, the numbers, and what only the owner can do.

## What was done

| Area | Done |
|---|---|
| **Modes** | `Track::auto_mode`, `Master::auto_mode` (Read, Write, Touch, Latch; Read the default and every older project). Write overwrites every armed target (what the track automates, and what the hand moves) from start to stop, holding each where it was; Touch writes while held and glides back over the touch glide (setting, 250 ms); Latch writes from the first touch until the stop. After a Write pass the track is in Touch. Master inserts follow the master's mode; a strip's targets the first track feeding it. |
| **Arm** | The top bar's automation arm (after loop) and its ▾: *All tracks* (every mode at once, one undo step) and the touch glide. Off at start, never saved. Actions `toggle-automation-arm`, `cycle-track-auto-mode` (in the palette; the second on Alt+Shift+W). |
| **One writer** | `cantus_core::automation::write::Writer` (pure: values, frames, jumps, stop) and `write_commit::{commit, live_lanes}`; the application's `auto_write.rs` drives it each frame. Round 28's recording of automation is now the Latch case of this writer (`views::modulator_record` is its entry points). |
| **One undo step** | `CommandBus::{mark, execute_folding}`: the commit folds in every parameter step the hand pushed during the pass; undo takes back the clips and the knobs. |
| **While writing** | A held control is heard (its lane drives nothing), a latched one holds, a released touch glides back along the old curve; the snapshot is resent when that changes. |
| **Loops, punch** | The last loop pass wins where it writes (a cut run joins the new one); with punch on only the punch range is written. |
| **Release velocity** | `Note::release` (0 = none, what every note-off carried). `TrackSlot::note_off(…, release)` → natives (`/127`), CLAP and VST3; live note-offs carry the keyboard's. |
| **Lanes** | `Clip::lanes`: pitch bend, channel pressure, CC 0…119, curves of `AutoPoint`s in content ticks; shared by link groups; through split, glue, duplicate, the clipboard and moved notes (*lanes follow the notes*). `Command::SetClipLanes`. |
| **Engine** | `TrackSnapshot::controls` (points, moving parts on the 1/96-beat grid, a rest where the clip ends); fired at their samples by the track node (offs, then controllers, then ons), chased wherever the playhead lands, put to rest at a stop. `graph/track_controls.rs`. |
| **Recording** | `EngineEvent::ControlInput` while recording; the recorder tags each with its loop pass; take clips get them as steps; overdub writes over the stretch played. |
| **Files** | MIDI export writes the controllers and each note-off's release velocity; import keeps bend, pressure and CCs 0…119 in lanes and the release velocities (the report says how many were kept). DAWproject writes a clip of lanes (notes with `rel`, `pitchBend` / `channelPressure` / `channelController` points) and reads it back. |
| **Interface** | The header badge (R/W/T/L in the mode's colour, filled while armed; a click opens the modes), the inspector's *Automation* row, the master's desk menu; the piano roll's lane rows (velocity, release, each lane) with their cells (name, ×, pencil/line/curve), the **+** menu (release lane, *Lanes follow the notes*, bend, pressure, the named controllers, every CC in pages of twenty), point drag, Shift-marquee, right-click delete, Delete. Glyphs `LanePencil`, `LaneLine`, `LaneCurve`. |
| **Words, docs** | `automodes` area in both locales (the interop report's dropped-message lines reworded); ADR 0068; the manual EN/ES (MIDI recording, the piano roll's lanes, writing automation). |

## How it was tested — and the numbers

Tests first: the engine's nulls before its wiring, the writer as pure data before the
interface drove it.

| Where | What it holds |
|---|---|
| `cantus-engine/tests/clip_lanes.rs` (5) | Empty lanes, a muted clip's and a take lane's lanes, notes with no release: **bit-identical** to none. Lanes heard; **block sizes 1, 7, 64, 333, 1 024, 8 192 bit-identical**; **real time in uneven blocks = offline, to the bit**; every bend event on its sample (tick × 25 at 120 BPM / 48 kHz) at any block size, the rest where the clip ends, each release velocity on its note-off; a start inside a bend **chases** its value. |
| `rt_memory.rs::controller_lanes_never_allocate` | Lanes through loop wraps (each chases), a new snapshot, a seek, a stop: **0 allocations**. |
| `automation/write_tests.rs` (8) | Read writes nothing (null); a written curve reproduces the gesture within **0.2 % of the travel — measured 0.175 %, 0.105 dB on the fader around 0 dB**; Touch glides back and the old curve plays on; Latch holds and puts the knob back; Write holds and turns Touch; the last loop pass wins; punch bounds the writing; the engine hears the hand, the hold, the glide. |
| `clip_lanes_tests.rs` (13), `bus.rs` (1) | Windows, loops, split halves meeting without a rest, validation, MIDI values both ways, overwrite/line/pencil/span/paste/shift/glue/steps; the bus folds a pass's steps (and not after an undo, not over a non-parameter step). |
| `undo_invariants.rs` | Lanes (valid and refused, through link groups) and modes, drawn from a side stream so every other arm's draws are unchanged; counted (all applied). |
| `cantus-project`: `midi_lanes.rs` (2), `dawproject.rs` (1), `midi_file.rs` | MIDI and DAWproject round-trip releases and lanes within MIDI's resolution; the DAWproject export valid against the official schema; the import report counts only what is dropped. |
| `cantus-ui/tests/auto_write.rs` (7) | Read, disarmed, Touch (one undo step with the drag folded in; undo restores clip and knob), Latch, Write → Touch, the held control heard, *All tracks*. The round-28 recording tests (controllers reviews 1–4, modulators) pass against the writer — their step counts now one folded step. |

**Windows**, `RUSTFLAGS=-D warnings`: `cargo clippy --workspace --all-targets` clean;
`cargo test --workspace --no-fail-fast`: **4 491 passed, 18 failed** — 2 then fixed and
passing (`frame_budget::mixer64_window_stays_in_budget`, its ceiling raised from 120 000 to
121 000 triangles for the arm, the badges and the inspector row: 120 108 measured;
`sched::the_serial_fallback_has_hysteresis_chase_lev`, timing under three builders' load,
passes on rerun) and **16 that fail on this machine whatever the branch**: every test that
builds a hosted plug-in session (`hosted_instruments` 4, `hosted_plugins` 2,
`controllers_review*` 7, `frame_budget` 2, `modulators` 1) — the app loads this PC's own
plug-in database, which now holds Odin2, Surge XT and Cantus's exported CLAPs from
`C:\Program Files\Common Files` ("one instrument scanned: 5"); nothing of this round
touches the scan. **Linux (WSL, `tools/wsl/check.sh`, CI's flags):** clippy clean; `cargo test
--workspace --no-fail-fast` exits 0 — every test green, the hosted ones included.

Shots (`cargo run -p cantus-ui --example shoot -- <dir> 1 automodes`, both themes, EN and
ES): `automodes-pianoroll-slate-en.png`, `automodes-pianoroll-paper-es.png`,
`automodes-headers-armed-slate-en.png`, `automodes-headers-off-paper-en.png`,
`automodes-topbar-slate-en.png` in this folder.

## Lines changed in shared files

- `cantus-engine/src/graph/track.rs`: `TrackSlot::next_control` (field, init);
  **`note_off(pitch, tag, release)`** (natives get `release / 127`); `run_track`: a
  `ctl_off` closure, controllers due fired before a note-on due at the same sample, the next
  controller bounding the segment after the notes loop. midi's note entry points otherwise
  untouched.
- `processor.rs` (`mod track_controls`, `fire` passes `ev.release`), `processor_msg.rs`
  (`live_note`/`sound_live` gained `release`; 0x80 split from a note-on at 0;
  `ControlInput` pushed while recording), `processor_snap.rs` (reseek: `next_control` and
  `chase`; `release_all`: `rest`), `messages.rs`, `backend.rs`, `lib.rs` (exports).
- `cantus-core`: `command.rs` (`SetClipLanes`, `SetTrackAutoMode`, `SetMasterAutoMode`
  after `SetTrackAutomationOpen`: enum, label, supersedes, apply), `model.rs`
  (`Note::release`, `Clip::lanes`, `Track/Master::auto_mode`), `snapshot.rs`
  (`NoteEvent::release`, `TrackSnapshot::{controls, control_kinds}`), `bus.rs`
  (`HistoryMark`, `mark`, `execute_folding`), `links.rs`, `freeze_guard.rs`,
  `clipboard.rs` (lanes in `Payload::Notes`, split/copy carry lanes), `lib.rs`; every
  `Clip`/`Note` literal got `lanes`/`release`.
- `cantus-project`: `format.rs` (schema 42, `migrate_41_to_42`), `examples/write_fixture.rs`
  (`v42.cantus`), MIDI import/export, DAWproject read/write, `report.rs`.
- `cantus-ui`: `app.rs` (one line `self.auto_write_frame()` after `note_playhead()`; the two
  actions in `run_action`; Delete takes selected lane points first), `keymap.rs` (two
  actions, `ALL` 98, ids, categories, Alt+Shift+W, **`FILE_VERSION` 12** with
  `NEW_IN_V12`), `settings.rs` (`touch_glide_ms`), `controllers_input.rs` (`latched`,
  `latch_recorded` delegate to the writer; the arm's lamp), `record_session.rs`,
  `record_pass.rs`, `recording.rs`, `clips.rs` (glue), `clipboard.rs`,
  `views/arrangement.rs` (badge layout, paint, input: three spots), `views/transport_keys.rs`
  (width and the arm keys), `views/inspector_track.rs` and `inspector_menus.rs` (the mode
  row), `views/mixer_menu.rs` (the master's modes), `views/modulator.rs` (`ModUi::rec`
  removed), `kit/glyph.rs` (three glyphs), `tests/frame_budget.rs` (the window's ceiling);
  locales `automodes` (and `interop`'s dropped-message lines).

## The merge with r31/integration (midi and workflow)

- Schema: 42 is automation's (`migrate_41_to_42` keeps its body), 43 midi's; `v43.cantus`
  rewritten by the merged writer so it holds both (lanes, releases and modes as well as
  note effects and grooves); `v42.cantus` unchanged.
- Keymap: version 11 is the workflow builder's (palette, history), 12 ours; `ALL` 103; an
  older file fills in V11 then V12.
- Release velocity through the note effects (ADR 0069): `Msg::release` now uses our
  convention (0 = none given, not 64). A clip note's off carries `NoteEvent::release` into
  the chain (`play::msg_of`, `bake`), and a live key carries its own. A chord gives every
  note it builds the key's release, and an arpeggio latched past the key gives its later
  steps that release. `Track::sound` passes `m.release` to `note_off`. `BakedNote::release`
  carries it into *Bake to notes* (`Note::release`) and into the MIDI export hook
  (`NoteFxPlay` is now 5-tuples; the played-file branch keeps the track's lanes too).
  Held by `notefx::tests::a_notes_release_rides_to_the_notes_it_makes`.
- `CommandBus`: workflow's step ids (`fresh_step`) and our `HistoryMark` /
  `execute_folding` both kept; a folded entry keeps its step.

## The integrated-tree review (seams with the other builders)

1. **A write pass outlived a project replacement** and was committed into the new one (a
   "Master · Gain" clip and the old gain in a project made from a template). Now
   `end_write_pass_here` ends an open pass into the project it rode before
   `replace_project` resets the bus (New, a template, a demo) and before a
   `Command::ReplaceProject` runs (an import, a recovery); undo or redo of another song
   waits while a pass writes (`automodes.status.song_held`). Three tests in
   `crates/cantus-ui/tests/auto_write_review.rs`.
2. ***Bake to notes* kept the lanes in the source's content frame** (a trimmed clip's
   bends landed `offset` late; a loop's played once). `clip_lanes::edit::heard(clip, a, b)`
   lays out what the lanes play over `[a, b)`, each repetition in turn, then the rest;
   the baked clip takes `heard(c, c.start, c.end())` (one line in `notefx_ui.rs`). Test:
   `bake_to_notes_keeps_the_controllers_as_heard`.
3. **A time-range cut or copy of a looping note clip misplaced its lanes** (the piece
   written out at offset 0, the tail re-inserted the same way). `loop_pieces`' note branch
   takes `heard(clip, a, b)`. Test: `crates/cantus-core/tests/lanes_review.rs` — the piece
   plays the range as heard, and what stays either side plays what it played.

## The fix-round review

1. **Open (and a recovery) still let a pass finish into the next song.** Every
   replacement now goes through `CantusApp::reset_bus_to` (`end_passes`: a recording and
   a write pass end into the project they rode, the transport stops, then `bus.reset`):
   `replace_project`, `open`, and both arms of `restore_recovery`. Tests in
   `crates/cantus-ui/tests/auto_write_seams.rs` (Open; a recovery over an autosave; takes
   over a saved project).
2. **`span` lost the shape where a stretch begins** (a held pedal became a ramp after
   Bake or a crop). `edit::span_on(points, a, b, shift)` keeps a point at `a`, a held or
   flat segment as a hold, a straight one as a line, and writes any other shape cut
   mid-segment as holds on the engine's grid (`timeline = content + shift`), so the piece
   plays the same samples; a span that ends on a point ends at that point's value, and
   `overwrite` keeps a point at `a` as the target of the segment before it (a pulse or
   stairs do not arrive there). `heard` and `glue` pass the grid. Tests: three cases in
   `crates/cantus-core/tests/lanes_review.rs` (lines within rounding; a held pedal and
   every curved shape to the bit on the grid) and a held pedal in the Bake test.
3. **Save at the unsaved question lost a pass**: `ask_or` (and *New from template*)
   ends the passes before asking. Test: New → Save keeps the written automation.

## What needs the owner

- **Ears and hands:** a Touch and a Latch pass on a fader and on a hosted plug-in's own
  window; a keyboard's bend, mod wheel, sustain and release recorded into a take and played
  back on Lamina (its damper reads the release velocity) and on Surge XT.
