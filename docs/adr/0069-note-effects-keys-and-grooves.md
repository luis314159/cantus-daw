# ADR 0069 — Note effects, the song's key and groove templates

**Status:** Accepted · **Date:** 2026-10-25 · **Round:** 31 (midi) · **Extends:** ADR 0004
(commands), ADR 0005 (the audio thread), ADR 0022 (notes into effects), ADR 0027/0057
(automation and modulators), ADR 0037 (linked clips), ADR 0064 (freeze)

## Context

BACKLOG §0.4 item 3 lists the daily gaps an FL or Ableton user meets first. For MIDI they
are: an **arpeggiator** and **chords** on a channel (FL's channel *Arp* and *Chord*,
Ableton's MIDI effects, Bitwig's note FX), a **scale** that keeps notes in a key, the
piano roll's **scale highlighting** and snap (FL), *fold to scale* (Ableton 12), and
**groove templates** (Ableton's groove pool, the MPC's swing) for clips and for the channel
rack's patterns. Cantus had none of them: a track's notes went from its events straight to
its instrument (`graph/track.rs`), and a clip played its notes as written.

## Decisions

### 1. A chain on the track, before the instrument, in the engine

`Track::note_fx: Vec<NoteEffect>` (schema 43, empty by default) — at most eight, each an id
from `Project::alloc_id`, an *on* switch and a kind with its settings: `Arp`, `Chord`,
`ScaleFx` (`cantus_core::notefx`). The engine plays it (`cantus_engine::notefx`), on the
track node, for **every** note the track is given: the clips' events at their own samples,
live input in the control phase, and the events due on a loop's last sample. One path, not
two: there is no compiled copy of the effects' output in the snapshot, so a live key and a
clip's note go through the same code, and *Bake to notes* runs the same chain offline.

Each stage takes notes in and gives notes out; stages feed the next in the chain's order.
Chords and the scale answer at once and remember what each note sounded (its tag and pitch
→ the voices), so the note's off releases exactly that whatever the settings did in
between. The arpeggiator holds the notes it is given and **schedules** its steps and its
notes' ends; the track's segment loop splits at the chain's next scheduled time exactly as
it splits at its next event, so a step sounds on its own sample at every block size. At one
sample every event goes in before a step there fires (a chord change and the step on it
play the new chord), and an arpeggio's note-off before the next step at the same time.

**Time.** Rolling, every time is a tick: an input at its event's tick, a step at
`anchor + k × rate` (every second step later by the swing, MPC-style
`(swing − ½) × 2 × rate`; the swing and each note's gate length are rounded to whole ticks, so
every time is exact), turned into a sample offset by `Transport::samples_until` — the
arithmetic clip events use, through the tempo map. So the offline render, real time, the
pool and every block size give the same bits, and a freeze, a bounce, stems and an export
hear what playback hears (the tests hold each). Stopped, a live arpeggio runs on the
engine's **sample clock** at the tempo at the playhead; starting or stopping the transport
moves a running arpeggio over (what sounds ends, a clip's held notes are let go, the live
keys start again from there). The first step plays where the first note of a chord arrives
— on the grid for a clip's chord on the grid, at once for a key pressed live.

**Determinism.** *Random* draws step `k`'s note from a SplitMix64 hash of the seed and `k`;
nothing reads a clock or a global counter.

**Tags.** A chord's voice on the played key keeps the note's tag (a live key's pressure
still finds it); every other output gets `voice_tag(tag, …)` of the stage and the voice or
step (bits 30 and 63 set: never a note's own id, unique in the 31 bits a CLAP or VST3 id
keeps). A kit's hits keep their ids, ticks and ordinals (a keyless scale through a Pulsus
kit renders to the bit as no chain). An arpeggio's ordinal is its step.

**Release velocity** (automation's round-31 seam): every `Msg` carries `release`; a chord's
or scale's off passes the note's own, an arpeggio's note ends with the release of the key it
came from when that key was already up, else 64. The note-off call is marked for the merge
(`sound()` in `graph/track.rs`).

**Controllers** do not enter the chain: `TrackSlot::controller` is unchanged and reaches the
instrument as before.

**Inserts that hear the track** (ADR 0022) hear the chain's output — the notes the
instrument plays; a frozen track's chain still feeds them. Modulators retriggered by live
notes (`auto_live_note`) still hear the keys, and a modulator gated by a clip's notes
(`modulator_compile`) reads the track's compiled events — before the chain, grooves applied:
one opening per chord, not per arpeggio step (the manual says so).

**Recording** records the keys played, not the arpeggio (as Ableton and FL do).

**Real time.** `Chain::new` allocates its fixed tables once, with the track slot; configuring
from a snapshot copies settings in place, and a chain that changed **shape** (an effect
added, removed, moved, switched, of another kind) starts afresh, letting go of what it was
sounding on the instrument. Parameters change in place and are read from the next note or
step. `rt_memory` plays chords into an arpeggio through loops, a seek, a parameter and a
shape change, live keys while stopped, with automation and an LFO on its parameters: zero
allocations. An empty or switched-off chain is never consulted (`fx_active`): every track
before schema 43 takes the path it took, to the bit.

### 2. Parameters, automation and modulation

Each kind has a parameter table (`NoteFxKind::params`: key, label, range, default,
stepped, unit). `AutoTarget::NoteFx { track, effect, key }` names one; the engine resolves it
to `Dest::NoteFx { track, stage, param }` and writes it at the control grid. So every
parameter is automatable and modulatable like any knob: its inspector row has *Create
automation clip* and *Modulate…*, takes a modulator's handle, records automation
(`value_of_command`). `SetNoteFxParam` is its command (a drag merges into one step).

### 3. The song's key; no key per clip

`Project::key: Option<Key>` (a root pitch class and one of 19 scales, `cantus_core::scale`).
A clip has **no key of its own**: a song's key belongs to the song; a track that must
differ gives its chord or scale effect its **own key** (`KeyChoice::own`), which is where a
different key actually changes a sound. Key changes along the song are not modelled this
round (a key track would be the tempo track's sibling); `SetProjectKey` sets the one key.

The piano roll reads it: **Scale** lights the key's rows (outside a shade darker, the root
tinted) and marks the keyboard; **Fold** lays out only the key's rows (plus every row a note
of the clip is on, so nothing hides) — an `Axis` like a kit's, so moving a note a row moves
it a degree; **Snap** keeps drawn and painted notes on the key's nearest note (ties down),
dragged ones on the next note of the key the way they moved, and `Up`/`Down` transposes by
degrees (`Shift`: octaves). The three switches are view state (`NoteFxUi`), as the grid's
snap is; the key is in the project. A kit's rows are its pads: none of this applies to them.

### 4. Grooves: non-destructive, on the clip, from a pool

`Clip::groove: Option<ClipGroove>` names a source — a built-in swing (`Swing { grid: 8ths |
16ths, percent: 50…75 }`, never stored) or a groove of `Project::grooves` — and amounts for
**timing** (0–150 %), **velocity** and **length** (0–100 %). A `Groove` is a period and one
point per grid step: how late (or early) a note there is played, and its velocity and
length against the groove's average. The snapshot applies it where it compiles the
clip's events: each note **heard** at tick `t` (its timeline position, so swing lines up
with the bars whatever the clip's offset and every loop repetition is swung alike) takes the
point of its nearest step and moves by `timing × shift` (rounded to a tick), keeping its own
distance from the grid — a groove adds a feel, it does not quantise. A grooved note never
starts before its clip. *Commit* writes the groove into the notes (the content positions of
each note's first hearing) and clears it — one `Batch`.

*Extract groove* takes a bar (the metre at the song's start) on a 16th grid from a MIDI
clip's notes, or from an audio clip's transients — round 29's SuperFlux onsets from the warp
analysis, placed through the clip's warp (or the unwarped rule) — each step's mean lateness,
and strength and length against the whole's.

The groove is **content** (ADR 0037): a link group shares it, copies and splits keep it; the
channel rack's *Groove* field sets it on every clip of the current pattern in one step
(*patterns take a groove too*).

### 5. Bake to notes replaces the clip (and keeps what rings past it)

What playback plays past the clip's end is baked too: an arpeggio's note rings to its gate (up to
200 %), a swung last step sounds, and a latched arpeggio (*Hold*) steps on until the track's
next note-on (where it would be replaced; else the song's end), then rings out
(`bake_until`); the baked clip is as long as what it holds. A *Commit* on a looping clip
whose loop is not a whole number of the groove's periods is refused (each pass has another
feel): *Bake to notes* writes every pass.

*Bake to notes* runs the chain over the clip's events (its groove applied) in ticks —
`notefx::bake`, the engine's own chain — and **replaces** the clip with a new one holding
those notes, one undo step. A second chain pass would play an arpeggio of an arpeggio, so
the new clip carries `Clip::skip_note_fx` (schema 43, content): its events reach the
instrument past the chain (`TrackSnapshot::direct`, empty when no clip has it). The source
stays one undo away; we chose not to keep a muted duplicate on the lane, which would leave
two overlapping clips the user did not ask for. The flag is visible and switchable on the
clip (*Note effects ▸ Bypass*), so a baked clip can be sent through the chain again.

### 6. MIDI export says what it writes

The export dialog asks **As played** (the default: grooves applied and each track's note
effects written out note by note, through `export_with`'s hook into the engine's bake) or
**As written** (the clips' notes, no groove, no effects), and says so in a line under the
choice. A clip's export now carries the pool and the key it reads.

**DAWproject** has no note effects, grooves or key: its export writes the clips' notes as
they hold them and the report says what was left out (`x_note_fx` per track, `x_grooves`,
`x_key`), each pointing at *Bake to notes* or *Commit* to keep it (ADR 0021's rule).

### 7. Freeze and the edit lock

The freeze subject (ADR 0064) already holds the track (its chain) and its clips (their
grooves); it now also holds the **key** when a frozen region's chain reads it
(`notefx::reads_project_key`) and the **pooled grooves** its clips play — written only when
present, so every older frozen target keeps its key. `SetTrackNoteFx`, `SetNoteFxParam` and
a clip's groove or bypass touch the track's source; `SetProjectKey`, `RemoveGroove` and
`SetGroove` are global (applied, then the frozen keys checked); `InsertGroove` touches
nothing.

## What the tests hold

- `cantus-core`: the scales (masks, snap ties down, stepping, diatonic chords), the chord
  voicer (inversions, spreads, range, key shapes, own key), the scale effect, every
  parameter's round trip and clamping, chain validity; the grooves (the MPC arithmetic: 66 %
  of 16ths puts the off-step 77 ticks late; half the timing half the swing; a played part's
  extracted groove gives it back to the tick and the velocity; a hit across the period is
  early for the downbeat; invalid grooves refused). The undo fuzz's round-31 arm: chains,
  parameters, the key, grooves, the pool, commit and bake, each applied and refused, exact
  inverses (120 sequences).
- `cantus-project`: schema 43's fixture keeps the chain, the key, the pool and both clips'
  grooves and the bypass, and saves back equal; v41 has none.
- `cantus-engine` (`notefx` unit tests through `bake`): up, down, up-down over two octaves,
  down-up, as played, chord mode with fixed velocity, random repeatable by seed, swing and
  gate, hold latching until a new chord, chords and their releases, chords from the key, a
  scale snapping and dropping, chains in either order, a baked clip passing the chain.
- `tests/notefx.rs`: an empty chain, a switched-off one and a keyless scale (also through a
  kit) render as no chain **to the bit**; an arpeggio renders as the notes it bakes to, to
  the bit, at blocks of 1, 7, 64, 1000 and 4096; chords into a swung random arpeggio into a
  scale is block-size independent; chords and a scale render as their bake; the processor
  driven by irregular blocks, serially and on a 3-thread pool, equals the offline render to
  the bit; a track's stem equals its bake's; a frozen track with an arpeggio plays to the bit
  what it played live; an automation clip holding *Octaves* at 3 renders as the chain set to
  3, a step from 1 to 4 is heard and block-independent; an LFO on *Gate* is heard and
  block-independent; live keys through a chord equal the chord played by hand; a stopped
  arpeggio steps every 6 000 samples at 120 BPM, as notes played by hand there.
- `rt_memory::note_effects_never_allocate`.

## Consequences

- New: `cantus_core::{notefx, scale, groove}`, `Track::note_fx`, `Clip::{groove,
  skip_note_fx}`, `Project::{key, grooves}`, `ClipContent::{groove, skip_note_fx}`,
  `TrackSnapshot::{note_fx, direct}`, `Snapshot::key`, `AutoTarget::NoteFx`, the commands
  `SetTrackNoteFx`, `SetNoteFxParam`, `SetProjectKey`, `SetClipGroove`, `SetClipSkipNoteFx`,
  `InsertGroove`, `RemoveGroove`, `SetGroove`; `cantus_engine::notefx::{Chain, bake}`,
  `Dest::NoteFx`; `midi::{Notes, export_with, NoteFxPlay}`; schema 43; the `notefx` strings;
  `shoot_notefx`.
- A track with an arpeggiator splits its pass at the arpeggio's steps and note-offs (as at
  its events): a 1/64 arpeggio at 999 BPM is a split every 180 samples — still bounded.
- Decided out of this design (§3): key changes along the song — the song has one key, a
  track that differs gives its effects their own. Per-note expression is not multiplied by
  the chain: a live key's pressure reaches the chord voice on that key (its tag), not the
  voices built over it, as a hardware chord memory does.
