# Round 35 — dancer (ADR 0087, schema 49)

The instrument end to end: the model and its commands, the engine's silent unit and its
feed, the choreography, the live feed on the interface, the instrument's window and the
character picker, notes and the move map, "React to", MIDI learn, the catalogue and agents.

## First delivery (items 1–4 of the brief) — ready to merge

### 1. `InstrumentKind::Dancer`, schema 49

- `cantus-core`: the ninth kind, in every dispatch (`ALL`, the `dispatch!` macro).
  `automatable` refuses `view.mode`/`view.top` (set, never automated — so no modulator
  or lane reaches them either); `InstrumentKind::sounds()` is new (`false` for the dancer)
  for whatever checks a level or a tone.
- `inst/dancer.rs` (mine): the shared model (macros, the connection matrix) is the
  dancer's too, as every kind's; its `SOURCES` are the global ones (macros, mod wheel,
  expression, aftertouch, sustain) — a mod wheel can drive its energy. New helpers:
  `move_of`/`move_value` (0 = the routine to the beat, else `Move::ALL[v − 1]`),
  `speed_of`. `READY` stays `false` until the window and the picker's "Visual" group are
  done (the rest of the round): the kind exists everywhere, the picker does not offer it.
- `TrackIcon::Dance` (a dancing figure, `dance_live::paint_icon`) — the existing six said
  nothing right for a dancer; it is part of schema 49.
- `cantus-project`: `SCHEMA_VERSION` 49, `migrate_48_to_49` (writes nothing),
  `fixtures/v49.cantus` (a dancer before the last track: a character, three numbers moved,
  `move` automated to Clap over bar 3, a Jump note), `tests/schema49.rs` (it loads, round
  trips, no older fixture has a dancer).
- Every other dispatch: `cantus-engine` (`make`, the listen budget), `cantus-ai` (`dancer`
  in the instruments list, its playable range = the move map), `cantus-bench`
  (`inst.saltator.*`), `cantus-ui` (the frame's dispatch, the face `SALTATOR`, the icon,
  `--editor dancer`). Its product name is **Saltator** (Latin for dancer, like the
  family's names) in both locales.

### 2. The engine's unit and its feed (`cantus-engine/src/inst/dancer.rs`, `dance.rs`)

- Silent: `process` writes nothing and returns `false`; `at_rest` always (it has nothing
  to suspend); no voices. `set_position` gives it the song at each segment; at each
  segment's end it publishes a `DanceFrame` (atomics; offline also the timeline). `skip`
  publishes too, should the engine ever skip it.
- Notes: only keys in the move map become the frame's `note`, stamped exactly with the
  segment they start (the next `set_position`); `released` (new) is where it was let go
  (a stop or loop wrap lets it go where the song is); `prev_note` (new) is the note it
  took over from; every key is in `held`. A seek/reset forgets them.
- `move_change` (new): where `move` last changed and from what — an automated change of
  move blends from the move it left. The first configure is not a change.
- `InstrumentUnit::dance_feed()` (default `None`). `EngineHandle::dance_feed(track)`:
  the feed of the unit the engine holds for a track (kept by serial in `DanceFeeds`,
  pruned with the units). `RenderProgress::dance` keeps a render's feeds:
  `progress.dance.timelines()` after the render — what video export samples, with
  `dance::sample(timeline, seconds)` (the frame of the segment that holds the moment,
  moved to it: `DanceFrame::at_seconds`/`at_beats`). `dance::feeds_of(units)` for anyone
  making units by hand.
- Its fork (a range render's checkpoint) publishes into a feed of its own.

**Null tests** (`cantus-engine/tests/dancer.rs`): the demo with a dancer — map notes, a
key outside the map, `move` automated, an LFO on `hue`, a follower on `level` — renders
bit-identical to the demo without it at blocks 1, 64, 256, 1000 and in real time with
irregular blocks; the pool (W = 1, 3, shuffled) gives the serial bits and the same last
frame; the frames that end on the same sample at blocks 64 and 1000 agree to the bit
(numbers, notes, bar; beats within 1e-9); what it publishes follows automation, notes,
releases and modulators; stopped it still publishes. `rt_memory::a_dancer_never_allocates`
(serial and W = 3: notes, automation, modulators, a live note, a seek, a loop, a stop):
zero heap events. `inst_contract` checks it with every kind (split, voices, at rest).

### 3. The live feed on the interface (`cantus-ui/src/dance_live.rs`)

`DanceLive::dancers(&app) -> Vec<LiveDancer { track, character, pose, frame, missing,
view_mode, on_top, live }>`, refreshed once a frame from `CantusApp::frame` (the shipped
path). Each feed is read once a frame into a history of 128 frames; the pose is at the
**heard** position (ADR 0022): the frame of the segment that held that moment, moved to
it, the shown position running on smoothly between the engine's reports (at most 60 ms)
and never stepping back a hair. With no unit yet (no device, the first frame) a dancer
dances from the transport and its own numbers. The character comes from `character`
(`builtin:` from the cast, `file:` from the project's folder — through
`cantus_dance::import::import` until import's `load` is merged), cached by id; one not
found draws the default and says so (`missing`). A project switch and a track that stops
hosting a dancer drop their feeds. Tests: `cantus-ui/tests/dance_live.rs` (on
`assemble` + `EngineHandle::manual`).

### 4. The choreography (`cantus-dance/src/pose.rs`, `choreo.rs`, `choreo_motion.rs`)

`pose_at` stays the pure function of the contract, now built from:

- `choreo.rs` — which move and from when: a map note plays its move from its start;
  held it goes on, let go it finishes the cycle it is in (a tap: one whole cycle); a note
  ahead of the song (after a loop wrap) has not happened; a key still held takes over when
  the latest note ends; else `move` ≥ 1, its phase from the bar line; else the routine,
  one move a bar. **Blends** only where the start is known — a bar line (from the bar
  before; none when the same move runs on seamlessly), a note (from the note it took over
  from, or the bar's move), a note's end, an automated change — over a quarter beat,
  eased, the old move's phase still running.
- `choreo_motion.rs` — travel (`step`: steps land on the beat; left/right walk the stage
  and turn round at its edges, facing the way they walk through a front-on turn; wander
  stays about `x`; counted from the song so stopping never jumps), a Jump's arc (crouch a
  quarter, spring, land on the next beat; higher with energy) on top of `jump`, a Spin's
  pirouette in `turn` (through the front and the back), energy (knob, `react`/`level`,
  a note's velocity), the beat's accent.
- `Pose` gained `playing`, `bar`, `beat_in_bar`, `beats_in_bar` and `accent` (for the
  cast's beat-reactive extras and phrase ends), `Default` and `Pose::of(mv, phase)` (a
  still pose for sheets). **Code that builds a `Pose` literal needs `..Pose::default()`.**

Tests: `pose_tests.rs` (18 rules as pure functions against a stub character, plus the
stand-in), `choreo_motion` (8), and `cantus-dance/tests/choreo_song.rs` end to end: the
engine renders a song, the timeline sampled at 30 fps — the routine to the beat with no
automation; an automation clip of `move` over bars 3–4 claps from frame 120 (4.0 s) to
frame 239 exactly; notes play their moves; 30 and 144 fps agree.

### Marked lines (files I do not own)

- `cantus-core/src/model.rs`: `TrackIcon::Dance`, `ALL`, its label key.
- `cantus-engine/src/backend.rs`: the `dance_feeds` field and its two inits, two lines in
  `send_snapshot` (take the feed, keep it), two pruning it, `dance_feed(track)`.
- `cantus-engine/src/render.rs`: `RenderProgress::dance`, one line keeping the feeds.
- `cantus-engine/src/listen_fork.rs`: the dancer's image size.
- `cantus-engine/tests/inst_contract.rs`: `dancer` in `per_kind!`, nine kinds.
- `cantus-engine/tests/rt_memory.rs`: `a_dancer_never_allocates`.
- `cantus-project/src/format.rs`, `examples/write_fixture.rs`: the migration and the v49
  block.
- `cantus-ui/src/lib.rs`: `pub mod dance_live`. `app.rs`: the `dance_live` field, its
  init, the frame call. `icons.rs`: the `Dance` arm. `views/inst/mod.rs`: `pub mod
  dancer`, `InstEditors::dancer`, `--editor dancer`. `tests/inst_faces.rs`,
  `tests/inst_shell.rs`: the kind's name.

### How it was tested (Windows, first delivery)

- `cantus-engine`: the whole crate before the feed grew (104 binaries, 997 tests, all
  pass); after: `--lib dance` 11, `tests/dancer.rs` 5, `inst_contract` 34 (the dancer's
  three twins included), `rt_memory` 26 (`a_dancer_never_allocates` serial and W = 3).
- `cantus-dance`: 29 unit tests (18 pose rules, 8 motion, the contract's) + `choreo_song` 4.
- `cantus-core`, `cantus-ai`, `cantus-bench`, `cantus-project` (with `schema49` 3, every
  old fixture loading): all pass (39 binaries); `freeze` 35 with the dancer's new test.
- `cantus-ui`: `--lib` 412, `dance_live` 6, and every test binary that walks every kind
  (`agent_tools`, `controllers_review4`, `gain_staging`, `id_clash`, `inst_frame`,
  `inst_shell`, `inst_faces`, `learn_everywhere`, `modulate_everywhere`, `picker`,
  `demos`, `fx_lint`, `kit_lint`): all pass.
- `cargo clippy --all-targets` with `-D warnings` on the seven crates touched: clean.
  `cargo fmt` on each.

### For the others when they merge

- `Pose` literals: add `..Pose::default()` (five new fields). `DanceNote` gained
  `released`, `DanceFrame` gained `prev_note` and `move_change`: literals add
  `..Default::default()`.
- The stage: `DanceLive::dancers(&app)` (refreshed once a frame by the app), each with
  its `view_mode`, `on_top`, `pose` and `character`.
- Video: `RenderProgress::default()` → render → `progress.dance.timelines()`; a frame at
  time t is `cantus_engine::dance::sample(&timeline, t)`, posed by `pose_at`.
- Import: `dance_live::load` calls `cantus_dance::import::import(dir.join(rel))` until
  `import::load(project_dir, rel)` exists; then I switch to it.

### Linux (WSL Ubuntu 24.04, `tools/wsl/check.sh r35/dancer clippy`, at `bac4519c`)

Workspace clippy with `-D warnings` and the plug-ins without their editor: clean. Tests,
headless: `cantus-core`, `cantus-dance`, `cantus-project` all pass; `cantus-engine`
`--lib` 165, `dancer` 5, `inst_contract` 34, `rt_memory` 26; `cantus-ui` `--lib` 421,
`dance_live` 6, `inst_shell` 3, `kit_lint` 4. The clone's `target/` was deleted.

## The rest of the section

### The window (`views/inst/dancer*.rs`)

**Play** is a small theatre:
- **Stage**: the character dancing live at the heard position, on a kit-drawn stage
  (back wall, floor, a spotlight that follows it and swells with the beat's accent,
  footlights glowing on strong beats), with a monitor's words (bar · beat, the move
  playing, who dances, or *loading* / why a character could not be found). It is drawn by
  the one renderer at the display's physical pixels, and only when the pose changed.
- **Character**: every included character as a tile dancing its own routine (previews
  at 30 fps at most), every character imported into the project
  (`dance_import::imported_in_project`), *Import…* (`dance_import::open`); under them the
  chosen one's name, its line (two lines, wrapped) and its licence; *Edit moves…* for an
  imported one (`dance_import::edit_moves`). A click is one undo step.
- **Moves**: the move map as sixteen pads (C3…D#4, key and move named, black keys a shade
  deeper, the move dancing lit). A pad held plays its key live on the track.
- **Dance** (Move and Travel as named fields with ‹ ›, Speed ¼×…4×, Energy), **Body**
  (Lift %, Facing left/front/right, Spin °, Size ×), **Look** (Mood by name, Colour °),
  **Ears** (*Reacts to* ‹Nothing / The whole song / each channel› and *Reacts* %).

**Edit**: every number as a card, *Where it shows* (Floating · Docked · Hidden, Always on
top) and the picker.

### Ways in and out
- The picker's new **Visual** shelf with Saltator (its own cover art); `READY = true`.
  A new dancer is named *Saltator*, its icon the dancing figure, its `character` the
  cast's first (`builtin:songbird`).
- **Presets** (To the beat, Party, Chill, Cool) and A/B never change who dances or where:
  `character`, `x`, `y`, `size`, `visible`, `view.*` are kept (`keep_through_preset`).
- **React to** (`dancer::react_command`): one `Batch` — a follower on the dancer's track,
  listening to a strip or the master, routed to `level` (made, retuned, or removed) and
  `react` set (`cmd.dance_react`, `cmd.dance_react_off`).
- **Imports** load off the interface's thread (`dance_live_load.rs`: a worker asks
  `import::load`, a panic inside it is an answer; the default dances meanwhile, *loading*
  said); a loaded import is asked for again every 0.5 s (the loader answers from its
  stamp-checked cache), so *Edit moves…* reaches the screen; a key leaving the project
  (`file:../../x.json`, an absolute path) is refused by the sandbox and said.
- `Pose::from_cycle` (cast's ask): the outgoing move's cycle where a blend starts.
- **Agents**: `get_unit dancer` adds the move map (key, note, move), the moves and moods by
  number and name, the characters and how it dances; stepped keys read their names (the
  choice table); `add_track {"kind": "dancer"}` makes one. The tools/list golden is
  regenerated (the instruments' list names the dancer).
- **MIDI learn** and the choice table (`inst_choices`: move, travel, speed, mood, visible,
  view.top, view.mode): a controller on `move` goes through the moves by name.
- The **manual** (`docs/manual/{en,es}/dancer.md`, linked from both indexes).

### Tests added
- `cantus-core`: `react_to_is_one_step…`; the undo fuzz's own sequences
  (`a_dancers_commands_undo_and_redo_among_the_others`: 60 sequences, 6 kinds).
- `cantus-engine/tests/dancer.rs`: a step sequencer on `move`.
- `cantus-dance`: `a_blend_knows_the_cycle_the_move_it_left_was_in`; `choreo_song` holds
  for any character (each move's own `move_beats`).
- `cantus-ui/tests/dancer_window.rs` (Visual shelf and a new dancer, both views in both
  themes with the character drawn, pads, React to in one step, the picker in one step and
  presets keeping who and where, MIDI learn and words, redraws only of what moved, an
  ignored timing test), `dancer_agents.rs` (the catalogue, an agent making a dancer and
  automating its move), `dance_live.rs` (imports off the thread, *Edit moves* reaching
  the screen, a key leaving the project refused).
- Shots: `examples/shoot_dancer.rs` (Play and Edit, Slate and Paper, 100/150 %, Spanish, a
  sequence through two bars with a Jump note); the best in `docs/round35/dancer/`.
- `Pose::beats` (cast's ask, for a pendulum across metre changes): the song's position in
  quarter notes (the playhead's, stopped).

### The numbers
- The window, Play view with the character dancing (a new pose every frame), release, this
  machine: **median 3.67 ms, p95 4.04 ms a frame** (`dancer_window::the_window_timing`).
  The stage draws again only when the pose changed; the previews at most 30 a second.
- The tools/list golden: the instruments' list gains `dancer` (Saltator).

### Shots
`docs/round35/dancer/`: Play and Edit in Slate and Paper, Play in Spanish and at 150 %,
two frames of the stage through bars 2–3 (a Jump note: crouch, spring). Every character of
the family dances its own preview in the picker.

### Seen while testing, not mine
`cantus-ui --lib`: `stage::geom::tests::a_box_draws_what_the_whole_area_draws` and
`the_body_stays_put_while_the_dancer_jumps_and_spins` fail on this branch after the cast's
characters arrived (they pose the default character with `pose_at` and check pixels and
bounds against stage's assumptions); stage's file, for stage or the integrator.

### Final checks (at `a3ee9283`)
- Windows: `cantus-core`, `cantus-dance`, `cantus-project`, `cantus-ai` 939 passed;
  `cantus-engine` (lib, dancer, rt_memory, inst_contract) 231; `cantus-ui` (lib and the
  dancer, learn, shell, frame, picker, lint, agent and MCP binaries) 518 passed, 2 failed
  (stage's two `stage::geom` tests above). Clippy `-D warnings` clean on the seven crates.
- Linux (WSL, `tools/wsl/check.sh r35/dancer clippy` + tests): workspace clippy and the
  plug-ins without editor clean; 1 573 passed, the same 2 failed; the clone's `target/`
  deleted.

## Review 1 of the full section

Each finding fixed with a test that fails without it:

1. **A jump of the song forgets the past** (`engine/inst/dancer.rs`): when a segment does
   not start where the last ended (a seek, a loop wrap), the notes and the last change of
   `move` are forgotten; a note given on the sample the song lands on is kept.
   `a_jump_of_the_song_forgets_the_notes_and_the_change_of_move`.
2. **The heard frame comes from this pass of a loop** (`dance_live::heard_frame`): the
   history is cut where the song went back; the newest run that holds the heard moment is
   searched (right after a wrap that is still the pass before).
   `the_heard_frame_comes_from_this_pass_of_a_loop`.
3. **A timeline only when a video asks** (`RenderFeeds::want_timelines`, set by the video
   job; `DanceFeed::keep_timeline`): `set_offline` no longer keeps frames; a WAV export,
   stems, a freeze, a listen keep none. `a_render_keeps_no_timeline_unless_a_video_asks_for_it`.
4. **Imports re-checked by their files** (`dance_live_load.rs`): the worker compares the
   character file's and `character.json`'s size and modification time and reads again only
   when they changed (the same `Arc` stays); misses are retried. Checked in
   `an_imported_character_dances_and_its_edited_moves_reach_the_screen` (no read while the
   files are unchanged).
5. **A dancer's own track is not frozen** (`core/freeze.rs` `CannotFreeze::Dancer`, said in
   words). `a_dancers_own_track_is_not_frozen`.
6. **Two map notes on one sample** leave the older note's release alone.
   `two_notes_on_one_sample_leave_the_older_ones_release_alone`.
7. **Audition while stopped**: a held pad or live key plays its move on the wall clock.
   `stopped_a_held_pad_plays_its_move_on_the_wall_clock`.
8. **Agents**: `add_track` names a new dancer's character, its description lists the
   dancer, the choice text has no runs of spaces; the golden regenerated.

**A hidden dancer costs nothing**: the cost was the inspector (the new track selected): a
native instrument's strip — a separate flush, an egui frame and button, four macro knobs.
The dancer now has label | value rows on the inspector's own sheet (who dances, the move,
what it reacts to, where it shows; a click on the character opens the window). And
`DanceLive::update` allocates nothing in a warm frame (the specs and the tempo map read
again only when the project changes, buffers reused, the default character kept, the
project folder asked only for an import). `dancer_cost.rs`:
`a_hidden_dancer_track_costs_the_window_what_a_plain_track_does`,
`a_warm_frame_of_the_dancers_allocates_nothing`.

**The window**: no Macros, Modulation or keyboard bands for the dancer (the stage takes the
room); the footer says the move and bar · beat instead of voices and an output meter;
*Import* under its tile like the characters' names; the tiles and the chosen one's words as
one block centred in the card. Re-shot (Slate, Paper, Spanish, 150 %).
