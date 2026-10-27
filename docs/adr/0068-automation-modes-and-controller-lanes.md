# ADR 0068 — Automation modes (Write, Touch, Latch) and the piano roll's controller lanes

**Status:** accepted (round 31) · **Date:** 2026-10-25 · **Extends:** ADR 0027 (automation
clips: what a pass writes is ordinary clips), ADR 0057 (recording automation becomes the
Latch case), ADR 0062 (a controller's moves are written at their own time), ADR 0004 (a
pass is one undo step: `CommandBus::execute_folding`), ADR 0005 (the lanes on the audio
thread allocate nothing), ADR 0020 §4 (where controllers land: at their sample, not on the
16-sample grid)

## Context

BACKLOG §0.4 item 3, the daily gaps an FL or Ableton user hits first: *automation write /
touch / latch, and the piano roll's lanes for CC, pitch bend, velocity and release.* Until
now automation was drawn (ADR 0027) or recorded while a recording pass ran (ADR 0057 — every
control moved, latched until the end); a note had no release velocity (every note-off sent
0), and a clip carried no controllers at all: a MIDI file's bends and pedals were reported
and dropped, a keyboard's mod wheel was heard live and lost in the take.

The references: Pro Tools, Logic, Cubase and Studio One for the modes (Read, Write, Touch,
Latch; Pro Tools' *AutoMatch* glide; *after a write pass, switch to touch*), FL's *record
automation* and Ableton's *automation arm* for the simple case; FL's event editor,
Ableton's clip envelopes and Cubase's controller lanes (*controllers follow events*) for
the lanes.

## Decisions

### 1. The modes, per track and on the master (schema 42)

`Track::auto_mode` and `Master::auto_mode`: **Read** (every older project, the default) plays
the curves and a hand on a control writes nothing (FL's override, ADR 0027 §6); **Write**
overwrites every *armed* target of the track from the pass's start to its stop, each held
where it was heard when the pass began until the hand moves it; **Touch** writes while a
control is held, then glides back to the curve already there over the **touch glide** (a
setting, 250 ms by default — Pro Tools' AutoMatch time; 0…5 000 ms); **Latch** writes from
the first touch until the stop, holding the last value.

- *Whose mode governs a target:* its own track's (a track's level and pan, its instrument,
  its modulators' amounts); a strip's (fader, pan, width, sends, inserts) is the mode of the
  first track feeding it — the track its clips live under (`automation::host_track`); the
  master's level, pan and width and its inserts' parameters follow the master's mode. A
  strip fed by no track follows the selected track, as its clips do.
- *Armed targets in Write:* every target the track already automates (a lane of its) and
  every one the hand moves. Writing every parameter of every device (Pro Tools' "all
  enabled") would make a clip for hundreds of parameters nobody touched.
- *After a Write pass the track (or master) is in Touch*, in the same undo step: a second
  pass never overwrites by accident.

### 2. The arm, and recording automation as Latch

A **global arm** (the top bar's, after loop; Ableton's place) — off when the application
starts, never saved: opening a project never writes by surprise. Modes other than Read
write only while it is on. Its ▾ holds *All tracks* (every track's and the master's mode at
once, one undo step) and the touch glide. Two actions: `toggle-automation-arm` (no default
key, as in Live and Pro Tools) and `cycle-track-auto-mode` (Read → Touch → Latch → Write;
**Alt+Shift+W**, keymap version 12 — every arrangement action has a key).

Round 28's recording of automation (`RecordSettings::record_automation`, on by default) is
**the Latch case of the same writer**, not a second path: while a recording pass records
automation, every control on a track that is not armed in another mode writes as Latch,
inside the pass's window (the punch range, or from the record start).

### 3. A pass: a pure writer, the frame, one undo step

`cantus_core::automation::write::Writer` holds a pass as data — per target its runs of
`(tick, position)`, whether a hand holds it, whether it latched, its glide — and is driven
by the application: **values** from the commands the hand runs (`note_tweak`, before each
runs; a controller's at its message's own time), **frames** (the playhead; which controls
are held — the open gesture on top of the history: a drag, a plug-in window's gesture, a
controller still moving), **jumps** (a wrap or a seek: every run ends where the playhead
was; what goes on writing begins again where it lands) and the **stop**. A value is a touch:
held until a frame says the hand let go (a click, a typed value: let go at once).

- *Loops:* each pass writes over what the previous one wrote where it writes — the last pass
  wins (`write::overlay`; a run cut by a later one joins it where it was cut, so no sliver
  of the old curve shows).
- *Punch:* with punch on, a pass writes only inside the punch range — from the punch-in
  exactly, to the punch-out.
- *Commit* (`write_commit::commit`): each stretch becomes an automation clip over exactly the
  time it covers (`edit::cover` clears what played there, the bake's rule); the curve is the
  samples thinned to **1/500 of the control's travel** with every change a ramp (a hand's
  frames come at their own times, so nothing is guessed a jump); the clips around it play
  on — a touch that glided back meets the old curve where it was. The project's own value
  of every target the hand moved is put back to what it was when the pass began: the
  hand's moves live in the clip, and before the first clip plays what played before.
- *One undo step:* the commit runs through `CommandBus::execute_folding`, which folds into
  it every step the hand pushed during the pass (the parameter moves), provided nothing was
  undone or redone meanwhile and every one only sets parameters. Undo takes the whole pass
  back — the clips and the knobs.

**What the engine hears while it writes** (`write_commit::live_lanes`, applied to every
snapshot of the pass): a held control's lane drives nothing (the hand is heard through the
project's value, ADR 0062 §3); a latched or written one holds its value; a released touch
glides back along the old curve (a lane is added for the glide when none drove the target).
The snapshot is sent again when any of that changes.

### 4. The controller lanes (schema 42)

`Clip::lanes: Vec<ClipLane>` — **pitch bend**, **channel pressure** and **any controller
0…119** (120…127 are MIDI's channel mode messages — all notes off, reset all controllers — not
controllers; no lane sends them). A lane is a curve of `AutoPoint`s in the clip's **content
ticks**, exactly as its notes and an automation clip's curve are: values are positions 0…1
(the bend's centre 0.5), drawn with automation's shapes and tensions. So a clip's window,
slip, trim, loop, split, glue (the halves give back their lanes), duplicate, link group (the
lanes are shared content), the clipboard and the tempo map need no special case. One lane
per kind; `Command::SetClipLanes` sets them whole (a stroke merges into one step).

**The release velocity** is the note's own: `Note::release` (0 — every older note, every
note drawn — is *none given*: the note-off carries 0, exactly what it carried before). A
note-off carries it to native instruments (`note_off(…, release / 127)`: Lamina's damper,
Exemplar's and Modulus's `relvel` source already read it), CLAP and VST3.

### 5. The engine: events at their samples, chased, put to rest

The snapshot compiles each track's lanes into `TrackSnapshot::controls`, a sorted stream of
`CtlEvent { tick, kind, value }` (`clip_lanes::compile_track`): at the start of each window
(or loop repetition) the curve's value there; every point at its own tick; the moving parts
between points sampled on the timeline's **1/96-beat grid** (5.2 ms at 120 BPM — only values
that changed); and the lane's **rest value** where its clip ends — the bend's centre, no
pressure, pedals up, the General MIDI resets of volume (100), balance and pan (64) and
expression (127) — so a bend never outlives the clip that drew it. A rest that meets
another clip's value of its kind at its tick gives way (the halves of a split meet without
a dip).

The track node fires them exactly as it fires notes — it splits its pass at each — through
the instrument contract's `controller` (natives: the bend in semitones scaled to ±2, as plain
MIDI is; hosted instruments: their event lists, at the segment's first sample). **At one
sample: note-offs, then controllers, then note-ons**, so a bend set where a clip begins is in
place for its first note. Wherever the playhead lands (play, a seek, a loop's wrap, a new
snapshot) every kind the track's lanes drive is **chased**: sent the value its stream holds
there (the last event before, or its rest), so a render from any start point hears what
playing through would have. A stop puts every kind back to rest. Nothing allocates.

Controllers do not reach effects that hear notes (ADR 0022: notes only); note effects pass
them through untouched and keep each note's release with the note they produce (the midi
builder's seam). **Our exported plug-ins** are effects: they hear notes, not controllers, in
any host — their parameters are the host's automation.

### 6. Recording, files and the rest of the program

- **Live MIDI** recorded into a clip keeps its controllers: the engine reports each bend,
  pressure and CC 0…119 while it records (`EngineEvent::ControlInput`, at the heard
  position), the recorder tags them with their loop pass, and each take's clip gets them as
  steps (MIDI's own shape — a value holds until the next), the controller at rest before
  its first; an overdub writes them over the stretch played. A keyboard's note-off velocity
  reaches the instrument live and the recorded note (`NoteInput::release`).
- **MIDI files** carry them both ways: export writes the stream the engine plays (sampled
  curves, rests) as bend, pressure and CC events, at one tick after the note-offs and before
  the note-ons, and each note-off with its release velocity (64, the standard's default, for
  a note with none); import keeps bend, pressure and CCs 0…119 as the clip's lanes (as steps,
  at rest before their first) and each note-off's velocity — the report now says how many
  were kept and counts only what is dropped (channel mode messages, key pressure).
- **DAWproject** carries them too: a clip with lanes is written as a clip of lanes — its notes
  (each with its `rel`ease velocity when it has one), then a channel's `pitchBend`,
  `channelPressure` and `channelController` points in normalized values, its curves as
  straight and held pieces (a bent piece sampled every 1/16 of a beat) — valid against the
  official schema; reading one brings those points back as the clip's lanes (other points
  are counted as before).
- **Freeze, bounce, stems and export** render through the same processor and snapshot:
  they hear the lanes as playback does.
- **The channel rack's step sequencer** writes notes into the clip and leaves its lanes as
  they are (they play under the steps); a drawn step has no release velocity. **Pulsus**
  hears the CCs a lane sends (its hi-hat pedal, CC 4 by default); it reads no bend.
- **The clipboard:** copying notes carries the lanes over their stretch (*lanes follow the
  notes*), pasting writes them where the notes land; a cut takes the notes and leaves the
  lanes. **Moving notes** in time carries the lanes over their stretch with them.

### 7. The interface

- **The top bar:** the arm key (the automation glyph, lit while armed) and its ▾.
- **The track header:** a badge left of the record lamp — R, W, T or L in the mode's
  colour (Write the record red, Touch the meters' amber, Latch the modulators' hue), filled
  while the arm is on, an outline while it is off; a click opens the four modes. The
  inspector has an *Automation* row; the master's desk menu has the master's.
- **The piano roll:** under the notes, the velocity lane, the release velocity lane (shown
  from the **+** menu) and each lane the clip holds, sharing the area under the splitter
  (scrolled when they do not fit at 40 pt each). A lane's cell names it, has its **×** and
  its three tools — **pencil** (freehand, thinned to the bake's tolerance; a pedal's lane
  steps), **line**, **curve** (Alt bends it the other way); in the body, a point is dragged
  (with the selection it is in), Shift-drag selects, a right-click deletes a point, Delete
  deletes the selection. The **+** offers the release lane, *Lanes follow the notes*, pitch
  bend, pressure, the named controllers (mod wheel, breath, foot, portamento, volume,
  balance, pan, expression, the pedals, the sound controllers, reverb and chorus sends) and
  every other number in pages of twenty. The bend lane shows the instrument's range (a
  native's own setting, ±2 for one without; a hosted plug-in's is its own).

## Consequences

- New: `cantus_core::clip_lanes` (`ClipLane`, `LaneKind`, `CtlEvent`, `compile_track`,
  `edit::{overwrite, line, pencil, span, spans, paste, shift, glue, steps,
  lanes_from_messages}`), `automation::write::{AutoMode, Writer, Live, overlay}`,
  `automation::write_commit::{commit, restore, heard_pos, live_lanes}`,
  `Command::{SetClipLanes, SetTrackAutoMode, SetMasterAutoMode}`,
  `CommandBus::{mark, execute_folding}`, `clipboard::{copy_notes_with_lanes, paste_lanes}`;
  `NoteEvent::release`, `TrackSnapshot::{controls, control_kinds}`; the engine's
  `graph/track_controls.rs`, `EngineEvent::ControlInput`, `NoteInput::release`,
  `TrackSlot::note_off(…, release)`; the UI's `auto_write.rs`, `views::auto_modes`,
  `views::piano_roll::lanes`; `Settings::touch_glide_ms`; schema **42** (fixture `v42.cantus`:
  a Touch track, a Latch master, a bend and a mod wheel lane, releases).
- `views::modulator_record` is now the entry points of the writer; `ModUi::rec` is gone
  (`CantusApp::written_points` reads what a pass has written).
- What the tests hold: a clip with no lanes (or empty, muted, take-lane ones) renders to the
  bit as none; lanes are heard, block-size independent (1…8 192) and real time equals the
  offline render to the bit; each event lands on its sample and the release on its note-off;
  a start inside a lane chases its value; nothing allocates through wraps, a new snapshot, a
  seek and a stop; Read with no gesture changes nothing; a written curve reproduces the
  gesture within 0.2 % of the travel (measured: 0.18 %, 0.11 dB on the fader around 0 dB);
  touch glides back and the old curve plays on; latch holds and puts the knob back; Write
  holds and switches to Touch; the last loop pass wins; punch bounds the writing; MIDI files
  round-trip releases and lanes within MIDI's resolution; the undo fuzz sets lanes and modes
  (valid and refused) through link groups.
