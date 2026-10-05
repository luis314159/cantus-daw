# Round 19 — ui-rec: recording, as the best recording DAWs do it

Builder: **ui-rec** (`r19/ui-rec`). ADR 0025 is the decision; this is what the UI side of it
became, and what the integrator and the other builders need to know.

## What exists

**A pass, start to end** (`src/record_session.rs`, `src/record_pass.rs`):

- The record button records **every armed track at once**: audio from its input on a track
  that has one, the notes played on one that does not. With nothing armed it is the quick
  record every earlier version had (notes into the selected clip, merged), so nothing that
  worked before stops working.
- On record: the recordings folder (`<project folder>/Audio`, or a session scratch folder
  `…/Cantus/recordings/session <stamp>` before the first save), `begin_recording(dir)`, the
  **count-in** (seek back N bars, `set_record_start(Some(playhead))`), play with recording on.
  No input device: the status says so and the settings open; nothing starts.
- While it runs: `poll_recording` once a frame (`LivePeaks` into per-track buffers), the
  loop's wraps seen from the playhead (the live take restarts at the loop's start, and the
  MIDI recorder tags notes with their pass, splitting a frame's events at the loop's middle).
- On stop: an audio pass stops the transport, then `finish_recording`; the takes and the notes
  become **one undo step** (`cmd.record_take`) built by `record_pass::build`:
  - each take is an `AudioSource` (`recorded: true`, named after the file) and one clip **per
    loop pass** — the last on lane 0, the earlier ones on new take lanes, oldest first; a last
    pass stopped in its first quarter is dropped when a full one precedes it;
  - windows: pass 0 from the record start, later passes from the loop start, cut to the punch
    range; content offsets from `start_tick` and, for later passes, from `wraps` and
    `record_latency().total()` — so the **count-in and the tail are handles**;
  - **Stack** moves what the new take covers on lane 0 to its own take lane (split where it
    straddles), **Replace** cuts it away, **Overdub** = Stack for audio;
  - **MIDI** gets the same windows, count-in, punch (notes outside it dropped), loop takes and
    modes; Overdub (and loop recording with loop takes off: cycle overdub) merges into the clip
    under the record start; new MIDI clips run bar line to bar line.
  - Dropped frames (`FinishedTake::dropped`) are reported in the status (ms lost) and a warning
    glyph stays in the clock until the next pass.

**Track header** (`views/arrangement_rec.rs`, hooks in `arrangement.rs`): a round red **arm**
lamp left of mute on every track (lit when armed, pulsing while recording; Shift+click arms
that track alone). On a tall row of a track with an input, a second line: the **input chip**
("IN 1", "IN 3-4"; a click lists *No input*, every mono input and stereo pair of the device,
and the monitor modes — or *No input device* and *Audio settings…*), the **monitor key** (a
speaker: struck out Off, one wave Auto, two waves and lit On; click cycles) and, while armed,
an **input meter** in place of the song map. A **disclosure** at the foot of the colour cell
opens the take lanes. The track menu gains a Recording part (add audio track, arm, show takes,
delete unused, explode) and the input list; the `+` key's right-click and the timeline's menu
offer *Add audio track*. Geometry is published: `views::arrangement::record_controls(app, id)`.

**Take lanes and comping** (`views/arrangement_takes.rs`, `src/comping.rs`): lanes under the
track (`TrackRow::takes` is their height; rows, scroll, content height and hit tests count
it), each a row of 0.75 × the base row with its take's clips drawn **by the arrangement's own
clip painter** (`clips()` places a lane-k clip in its lane and never hit-tests it — so
ui-audio's waveform body appears there unchanged). **Swipe** across a lane (grid-snapped, Ctrl
for free) → that range of lane 0 becomes a window on the take, one undo step
(`cmd.comp_swipe`); a stroke next to an existing window of the same take grows it. Click a
take clip → it becomes the comp over its range; click a lane's name in the header → the whole
lane. Comped ranges are lit on the take lanes (lime wash and foot bar) and a lamp on the lane
header. Lane menu: use this take, delete unused takes (also removes sources nothing plays and
closes lane gaps), explode takes to new tracks (same strip, clips muted so nothing heard
changes), hide takes. A comp is identified by `ContentKey` (the source, or the first note id,
plus `start - offset`): MIDI comp windows share note ids with their take, as a split does.

**Transport** (`views/record_options.rs`, hooks in `transport.rs`): a narrow ▾ beside the
record key (or right-click on the key) opens the **record options** popover in the inspector's
row grammar: count-in (Off / 1 / 2 / 4 bars), punch (on/off, *from selection*), loop takes,
take mode (Stack / Replace / Overdub), click (Always / Recording / Count-in) and click level.
While recording, the clock's metre slot shows a pulsing lamp and the take's elapsed time
(counting down the beats of the count-in).

**Punch** (`arrangement_rec.rs`): while punch is on, a punch lane appears between the loop
lane and the bar numbers (`TimelineGeometry::punch`) with the range as a red brace: drag an
end to move it, drag elsewhere for a new range; right-click: *set punch from selection*,
*punch off*. Red lines mark punch in/out down the lanes.

**Live takes**: each recording track shows its take growing in the record colour on lane 0
(the count-in part shaded as a handle), with the live waveform through
`waveform::paint_peaks` (folded to the columns on screen first), and the notes as they are
played on instrument tracks.

**Mixer** (`mixer_strip.rs::fx_rec`, `mixer_menu.rs`): on a strip whose tracks have an input,
the record dot **arms** them (FL); the strip menu gains Recording: *Input ▸* (the strip's
recording track — its first track with an input), *Arm*, and *Record the strip's output*
(the round-15 mix take moved here for those strips; strips without an input keep the dot as
the mix take).

**Inspector** (`views/inspector_record.rs`): a *Recording* section on the channel sheet under
the input row: the device input (a list; the level along the well's foot while armed),
monitor (Off | Auto | On), trim (−24…+24 dB, drag), arm. Dimmed, never hidden, without an
input.

**Keys** (all presets, Alt+letter so the typing piano never takes them): Alt+R arm the
selected track, Alt+C count-in, Alt+P punch, Alt+T show takes, Ctrl+Shift+A add audio track;
R still records.

## Tests

- `tests/rec_pass.rs` (16): count-in kept as a handle; latency moves content not window; a
  clip per loop pass, the last on lane 0; a short last pass dropped; later passes lined up by
  the latency; loop takes off; punch window with both handles; a take missing the punch leaves
  nothing (no source); Stack splits and stacks; Replace cuts and undoes in one step; old
  material stacks before the passes; multitrack; MIDI loop passes, MIDI overdub, MIDI punch,
  quick record. Each asserts one undo step.
- `tests/rec_comp.rs` (6): a swipe makes a window and undoes in one step, a second stroke
  grows it, comped ranges reported; promote; delete unused (lanes closed, pool emptied); a
  partly comped take is kept; explode (muted tracks, same strip, nothing heard changes); MIDI
  comps share note ids and compile to the right notes.
- `tests/rec_flows.rs` (9): audio track shape and undo; arm/monitor/input/trim round trip and
  undo (Shift = exclusive in one step); the header's published controls clicked with real
  pointer events (arm, Shift-arm, monitor); disclosure + a swipe across a take lane as pointer
  events (one undo step); the punch lane drag; count-in key and punch from selection; no
  device → nothing starts, status and settings; an armed instrument track records a take on
  its lane (Stack); a finished pass with dropped frames.
- `recording.rs`: a wrap splits a frame at the loop's middle, held notes keep their pass.
- `cantus-core/tests/undo_invariants.rs`: two new arms — arm, input (incl. a NaN trim that is
  refused), takes open, clip lane (incl. a missing clip), audio source insert, an audio clip on
  a take lane, and the record settings (incl. an infinite click level that is refused).
- `examples/shoot_rec.rs`: the arrangement and the whole window with armed tracks, inputs,
  monitor On, meters, take lanes open with a three-take comp, the punch brace, a guitar take
  being recorded with its waveform, the record options open, in Slate and Paper and Spanish.

## Files touched outside my own

`app.rs` (the recording methods moved to `record_session.rs`: the old section deleted, a
`rec` field, five `run_action` arms), `arrangement.rs` (two `#[path]` modules, `TrackRow::takes`,
`TimelineGeometry::punch`, `Frame` punch height, lane-k clips placed in their lane and not
hit, marquee on lane 0, the headers' calls, the menus), `arrangement_paint.rs` (`HeaderRects::arm`,
bar numbers below the punch lane), `transport.rs` (the ▾ key, the clock's elapsed slot),
`mixer_strip.rs` (the dot arms input strips), `mixer_menu.rs` (Recording entries, `Sub::Input`,
three `Act`s and their helpers), `inspector.rs` + `inspector_channel.rs` (one module, one call),
`keymap.rs` (five actions), `views/mod.rs`, `lib.rs`, `recording.rs` (pass tags),
`cantus-core/tests/undo_invariants.rs` (`rng.below(25)`, arms 23 and 24).

## For the other builders and the integrator

- **engine-rec**: the UI calls `set_recording(false)` and, for an audio pass, `stop()` before
  `finish_recording()`; it calls `set_record_start(None)` when not counting in and after a
  pass. `record_latency().total()` is read at `finish_pass` time and used for passes ≥ 1
  (`origin = loop.start − (wraps[k−1] + latency)/sr`); pass 0 trusts `start_tick`. `wraps`
  are taken as *rendered* frame counts at the loop's start. There is no live overflow API — the
  UI reports `FinishedTake::dropped` at the end; an `EngineHandle::record_dropped()` (or overflow
  in `LivePeaks`) would let the clock warn while recording. MIDI from the keyboard is recorded
  into every armed instrument track; only the live track sounds.
- **ui-audio**: take-lane clips go through your clip body painter unchanged; please keep
  `clips()`'s first lines (the `target` row for lane > 0, the `clip.lane > 0` early-out in the
  hit test) when merging. `waveform::paint_peaks` is used for live takes with pre-folded
  columns. The saving step should move recorded sources out of the scratch folder.
- **undo fuzz**: if another builder also bumped `rng.below(23)`, renumber: my arms are 23
  (recording, track-level) and 24 (record settings), and arm 23 already inserts audio sources
  and audio clips on take lanes.
- **devices**: `CantusApp::input_channels()` is the device's input count (or
  `rec.demo_inputs` for shots/tests), `input_level()` reads `Meters::inputs`.

## Left

- Live dropped-frame warning (needs the engine API above).
- Take lanes for a track whose lanes are shown but empty (max lane 0) show nothing; renaming a
  take lane; colouring takes; quick-swipe across several lanes in one stroke (Logic).
- Count-in before bar 1 is clipped at 0 (no negative timeline).
- The input chip and monitor key need a tall row (≥ 34 pt); short rows have arm only (the
  menu and the inspector have the rest).
