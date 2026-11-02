# ADR 0087 — The dancer: a silent instrument, a feed of frames, a pose that is a function of the song

**Status:** accepted (round 35, dancer builder) · **Date:** 2026-10-31 ·
**Extends:** ADR 0023 (native instruments), ADR 0057 (modulators), ADR 0068 (MIDI learn),
ADR 0022 (what is heard), ADR 0027 (automation), ADR 0082 (the unit catalogue) ·
schema 49 (`InstrumentKind::Dancer`, `TrackIcon::Dance`)

## Context

The owner asked for a character that dances on screen (BACKLOG §6i; FL Studio's Fruity
Dance is the reference for the idea, never its character or art): driven by automation
and notes, dancing to the beat with no automation at all, reacting to a track's level,
drawn the same on the screen, in the companion window and in a video. Three things had to
be decided: where its numbers live, how what the engine knows reaches the screen without
touching the audio thread's rules, and how a pose is made so that the screen at any rate
and a video agree.

## Decision

### 1. A native instrument that makes no sound

`cantus_core::inst::dancer` is a ninth `InstrumentKind`. A track hosts it, so its numbers
are `AutoTarget::Instrument { track, key }`: automation clips, modulators, MIDI learn, the
unit catalogue and agents reach them with no new path. Its keys, `Move`'s and `Mood`'s
orders and `BASE_KEY` (48) are forever. `view.mode`/`view.top` are set, never automated
(`InstrumentKind::automatable`), so no lane or modulator reaches them. The shared model
(macros, the connection matrix) is the dancer's too; its sources are the global ones
(macros, mod wheel, expression, aftertouch, sustain). `InstrumentKind::sounds()` is
`false` for it: what measures an instrument's level passes it by.

### 2. The engine publishes; the interface reads

The unit (`cantus-engine/src/inst/dancer.rs`, MPL like the engine) writes no sample and
is always at rest. At each segment's end it **publishes** a `DanceFrame` — the song's
place (beats, seconds, tempo, bar), its numbers after automation and modulation, the last
note of its move map with where it started and was let go, the note before it, where
`move` last changed and from what, the keys held — into its `DanceFeed`: a sequence lock
over atomics, the writer never waiting. Offline the feed also keeps every frame. Notes,
releases and changes of `move` are stamped with the position of the segment they start
(the processor splits segments at events and at the control grid), so they are exact to
the sample and the same at any block size.

The feed is made with the unit, off the audio thread, and handed over by
`InstrumentUnit::dance_feed()` before the unit is sent: `EngineHandle::dance_feed(track)`
for the live engine (kept by the unit's serial), `RenderProgress::dance` for a render
(`timelines()` afterwards), `dance::feeds_of` for units made by hand. A fork publishes
into a feed of its own.

### 3. The pose is a pure function of a frame

`cantus_dance::pose::pose_at(frame, character, wall)` keeps no state. Its rules
(`choreo.rs`, `choreo_motion.rs`):

- **Which move**: a map note from its start (held it goes on; let go it finishes its
  cycle; a tap plays one); then a map key still held, from the end of the latest note;
  then `move` ≥ 1, its phase from the bar line; then the character's routine, one move a
  bar.
- **Blends** only where the start of the current move is known — a bar line, a note's
  start or end, a change of `move` — from what played just before (its phase running on),
  over a quarter beat, eased.
- **The body**: steps land on the beat; a walk turns round at the stage's edges, facing
  the way it goes; a Jump crouches and lands on the next beat; a Spin pirouettes in
  `turn`; energy is the knob, the level heard by `react`, and a note's velocity; the
  beat's accent (downbeat strongest) is there for beat-reactive extras.
- Stopped, it breathes (Idle) on the wall clock where the playhead stands.

A moment is taken from a frame by `DanceFrame::at_seconds`/`at_beats` and from a
timeline by `dance::sample`: the frame of the segment that holds the moment (the first
published after it), moved to it. So the screen at 30 or 144 frames a second and a
video's frames agree wherever their times meet.

### 4. The screen shows what is heard

`cantus_ui::dance_live` reads each feed once a screen frame into a short history and poses
each dancer at the heard position (ADR 0022), the frame of the segment that held it; the
shown position runs on at most 60 ms between the engine's reports and never steps back a
hair. A dancer with no unit yet dances from the transport and its own numbers. Its
character is resolved from `character` (`builtin:<id>`, `file:<path>`), cached by id; one
not found draws the default and says so.

### 5. The window, the ways in, and what it hears

- Saltator's window (`views/inst/dancer*.rs`) is a small theatre: the live stage, the
  character picker with dancing previews, the move map as pads, every number in the unit a
  person reads (named fields for move, travel and mood). It is on the picker's **Visual**
  shelf: a dancer makes no sound and is not filed with the instruments that do.
- **React to ‹a track›** is one command (`dancer::react_command`, a `Batch`): the follower
  modulator on the dancer's track routed to `level`, made, retuned or removed, and `react`
  set. There is no second level path.
- A preset or A/B changes the dance, never who dances or where: `character`, the place,
  the size, `visible` and `view.*` are kept.
- **Imported characters load off the interface's thread** (`dance_live_load.rs`): a worker
  asks import's sandboxed loader (`import::load`), the default dances meanwhile, and a
  loaded import is asked for again every half second (the loader's cache checks the
  files), so an edit of its moves reaches the screen and nothing is kept for good.
- `Pose::from_cycle` gives the outgoing move's cycle where a blend starts, so a
  side-alternating move blends from the side it was on.

## Consequences

- A project with a dancer renders bit-identical to the project without it; golden hashes
  and `DSP_VERSION` do not move.
- A dancer is never put to rest by a freeze (it is watched, not heard).
- The video export samples the render's timeline at its frame times through the same
  `pose_at` and renderer: what is shown is what is exported.
- `Pose` grew `playing`, `bar`, `beat_in_bar`, `beats_in_bar` and `accent`, with
  `Default` and `Pose::of`: code building a literal adds `..Pose::default()`.

## What the tests hold

- `cantus-engine/tests/dancer.rs`: bit-identical with and without a dancer (blocks 1, 64,
  256, 1000, real time), the pool equals serial, the frames block-size independent, what
  it publishes follows automation, notes and modulators; `rt_memory`: no allocation;
  `inst_contract`: the twins.
- `cantus-dance` `pose_tests.rs`, `choreo_motion`: every rule as a pure function;
  `tests/choreo_song.rs`: rendered songs sampled at 30 fps — the routine to the beat, an
  automation clip over bars 3–4 changing the move exactly from frame 120 to 239, notes,
  30 = 144 fps.
- `cantus-ui/tests/dance_live.rs`: on `assemble`'s path, the frame poses the dancer from
  the engine's feed at the heard position; feeds dropped on a project switch and when the
  track stops dancing.
- `cantus-project/tests/schema49.rs`, `fixtures/v49.cantus`.
