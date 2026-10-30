# ADR 0080 — MPE end to end

**Status:** accepted (round 33) · **Date:** 2026-10-29 · **Extends:** ADR 0004 (commands),
ADR 0005 (real time), ADR 0023 (instruments' `mpe.*` sources), ADR 0028 (hosted note
expressions), ADR 0063 (the sandbox), ADR 0068 (lanes, release velocity) · schema 47 (the
integrator renumbers after session's 46), sandbox `PROTOCOL` 7

## Context

BACKLOG §0.2 asked for MPE as Bitwig and Logic have it: recorded, edited per note, played
to our instruments and to plug-ins, in MIDI files. ADR 0028 carried note expressions to
plug-ins, but nothing made them per note: `NoteOn.channel` was always 0, the processor
ignored the channel nibble, a clip had only channel-wide lanes, the MIDI import split a
controller's channels into tracks and dropped key pressure.

## Decision

1. **The model.** `Note::expr: Option<Box<NoteExpr>>` — three curves (bend, pressure,
   timbre), points `[at, value]` with `at` in ticks from the note's start; absent on an
   ordinary note (`skip_serializing_if`), so a note without expression serialises exactly
   as before. **Integers at MIDI 2.0's resolution** keep `Note: Eq`: the bend in 2⁻²⁴
   semitones (±127; exactly half a *Pitch 7.25* step), pressure and timbre as 0…`u32::MAX`
   (MIDI 2.0's 32-bit controllers). Linear between points, held outside. A point may lie
   before the start or past the end: a split and a trim keep the point that shapes the edge,
   so both halves sound what the whole did **to the integer** (tested), and shrinking a note
   is lossless. `normalised()` / `is_valid()`; a load repairs a hand-edited curve.
2. **Edits** are one command, `SetNoteExpr { track, clip, edit, notes: [(id, Option<…>)] }`,
   the inverse the old curves; `edit` (set, draw, scale, smooth, thin, flatten, clear) names
   it in the history (`cmd.mpe.*`). The work is pure functions (`note_expr::ops`: split,
   trim, stretch, reverse, join, scale, smooth — binomial ¼½¼ —, thin — Ramer–Douglas–
   Peucker at a cent / a thousandth —, draw, flatten, snap). It goes through `edit_shared`,
   so linked copies get it. `SetInstrumentExpression` sets a hosted instrument's route.
   Note operations: the curves are relative, so move, transpose, quantise, copy, paste, glue
   and linked clips keep them untouched; the knife (`slice_at`) splits them (see review fix
   10 below for the operations Cantus does not have yet).
3. **Playback** (`note_expr::compile`, the snapshot's `TrackSnapshot::expr`): exactly the
   notes `compile_events` plays (heard clips, windows, loops, grooves, muted notes and pads),
   each event named by the note-on's voice tag (`voice_tag` for a copy). Per curve: the
   value at the note-on, every point, the moving parts on the 1/96-beat grid (only changes),
   nothing at or after the note-off (the last value is held into the release). The track
   fires them last in each segment (after notes and controllers at one sample): a native
   unit splits its pass at each value's sample; a unit with `expression_offsets()` (a hosted
   plug-in) is handed the segment's values ahead with their **sample offsets**
   (`expression_at`), so a moving bend does not cut a plug-in's block every 5 ms.
4. **Our instruments.** Unda, Calor, Modulus and Gravis glide each value to its target at
   the control rate (`cantus_dsp::expr_glide`, τ = 4 ms): no zipper, the first value a note
   gets before it sounds taken at once (Unda re-runs its first control tick; Calor's first
   tick is lazy; Modulus's first ≤16 samples (0.33 ms) are at the start value — inaudible;
   Gravis per sample). The bend adds to the glided pitch (glide-safe). Measured by rendering:
   a +7 semitone curve is 700.0 ct on Unda, Modulus and Gravis and 698.2 ct on Calor (its
   default patch's analogue drift); the test holds it within 2 ct. Pressure and timbre are the `mpe.press`/`mpe.slide`
   sources; a **fresh** Unda, Calor or Modulus (the picker, the inspector) comes with two
   routes in matrix slots 31–32 (`note_expr::defaults`) — Unda: press → osc 1 level, slide →
   wavetable position; Calor: both → the VCF; Modulus: press → gain, slide → brightness —
   **null without expression** (tested to the bit) and only for new instances. Exemplar,
   Lamina, Chorda and the other instruments do not listen this round: Exemplar's pitch is a
   resample ratio per zone that a per-note bend would need to re-pitch per voice (its
   streaming voices, ADR 0050); Chorda's and Lamina's physics have no single "pressure"
   that sounds right without the owner's ears. They take their notes as before.
5. **Live input.** One ring merges every port, so a port's MPE-ness is decided in its MIDI
   callback (`inst::mpe::for_engine`): a port marked MPE passes its channels; any other port's
   channel messages go to channel 1 (the lower zone's master, or no zone); no port marked —
   nothing changes. The processor's `MpeLive` (fixed arrays, a child of its MIDI module):
   member channels (MPE 1.1 §3) become notes with ids of their own (`MPE_TAG_BASE − ch·128 −
   key`) and their channel; bend (at the zone's member range), channel/key pressure and CC 74
   are that note's; values before a note-on are held for it; the master channel goes the
   ordinary way; RPN 6 (the MCM) and RPN 0 (bend range) are read as they pass. While
   recording it reports `ExprInput { pitch, start, tick, kind, value }`, named by the
   `NoteInput` that started the note; the interface's `MpeTake` binds them and thins them.
   Settings ▸ MIDI's MPE band: per-port *MPE*, the zones, the ranges; published at
   `assemble`, an audio restart and a rescan (tested from `assemble`).
6. **Hosted plug-ins** (`ExternalPlugin::expression: Option<ExprRoute>`, `None` automatic):
   *note expressions* (CLAP `CLAP_EVENT_NOTE_EXPRESSION`, VST3 `NoteExpressionValueEvent`;
   VST3 tuning normalised `0.5 + st/240`, the SDK's ±120-semitone range), *MPE (MIDI)* — each
   note on a member channel, least recently used first, the MCM and RPN 0 = 48 announced once,
   bend/pressure/CC 74 on the note's channel, the oldest note giving its channel up when all
   15 sound — or *off*. Automatic follows what the plug-in declares (`ExprSupport`): CLAP's
   note-port dialects; a VST3's `INoteExpressionController` count and whether it maps a
   member channel's bend through `IMidiMapping`. That travels to the sandbox in
   `Described::expression`: **`PROTOCOL` 7**.
7. **Files.** MIDI import: a track is an MPE zone by its MCM, or guessed (≥ 2 member channels,
   each monophonic, one bent while sounding); members are recombined on the master channel,
   each note with its channel's curves (the value held at its note-on its first point); key
   pressure, on any track, is its note's pressure curve. Export: a track whose notes carry
   expression is a lower zone — MCM, RPN 0 = 48 per member, a member channel per note (LRU),
   each note's events as its channel's messages, a starting value before its note-on.
   DAWproject: a note's curves are `transpose` (semitones), `pressure`, `timbre` timelines,
   the lanes of the one timeline a note holds (schema-valid).
8. **The piano roll**: the bend as a line through each note (a semitone a row), pressure its
   weight, timbre its colour; rows *Bend / Pressure / Timbre (per note)* from the lane area's
   **+**, editing the selected notes (all when none): pencil (thinned), line, Shift snaps the
   bend to semitones, a right-click's smooth / thin / scale 50 % / 150 % / flatten / clear.

## MIDI 2.0 — measured

Built: the model at MIDI 2.0 resolution and the translation layer (`note_expr::midi2`):
the Min-Center-Max scaling of M2-115 §3 (7- and 14-bit up, tested on its fixed points), the
member bend ↔ curve (exact), per-note pitch bend at a range, *Pitch 7.25* (registered per-note
controller 3, exact), and UMP type-4 packets for note on (with *Pitch 7.9*), note off, poly
pressure, registered/assignable per-note controllers and per-note pitch bend — encoded and
decoded, and mapped onto the curves. Not built, with sizes for BACKLOG §0.2:

- **Transport** (≈ 2–3 weeks): midir has no UMP. Windows MIDI Services (the new
  `Microsoft.Windows.Devices.Midi2` WinRT API, Windows 11 24H2+) needs a WinRT binding in
  `cantus-rt` and its own endpoint enumeration; ALSA's UMP sequencer API (kernel 6.5+, alsa-lib
  1.2.10+) a new `cantus-rt` backend; CoreMIDI's `MIDIEventList` the macOS one. Each with
  MIDI-CI discovery to know a device speaks MIDI 2.0.
- **Plug-ins** (≈ 1 week): CLAP has `CLAP_EVENT_MIDI2` (raw UMP) — the translation above
  feeds it directly; VST3 has no MIDI 2.0 event, its note expressions already carry the 32-bit
  values (as doubles).
- **Per-note controllers beyond the three** (≈ 1 week, a schema bump): MIDI 2.0's per-note
  modulation, volume, pan, … would be more curves on `NoteExpr` and rows in the lane.

## Consequences

- Schema 47 (its migration writes nothing; `fixtures/v47.cantus` has a bent, pressed note and
  a hosted instrument sent MPE). An older build refuses a v47 file.
- Null tests hold: no expression, or expression only on muted notes / take lanes / muted
  clips, renders bit-identical; the golden hashes did not move (`DSP_VERSION` stays 5); a
  fresh instrument's routes are silent without expression; MPE off on a port changes nothing.
- `rt_memory`: expression playback through loops, snapshots and seeks, and a live MPE
  controller with MCM and RPNs while recording, allocate nothing.
- What the tests hold: `note_expr_tests.rs` (curves, splits exact, joins, MIDI 1/2, compile,
  the command's inverse, serialisation), `note_expr_zones.rs`, `note_expr_defaults.rs`,
  `cantus-engine/tests/mpe.rs` (null, block-size independence at 0, real time = offline,
  parallel = serial, values on their samples split or ahead, every instrument bends 7
  semitones by its sound, the routes, live MPE from a Seaboard-style stream, recording
  reports), `cantus-host` `inst_unit_tests.rs` / `inst_mpe.rs` (offsets, MPE out, off),
  `cantus-project/tests/midi_mpe.rs` and `dawproject_mpe.rs` (round trips), `format.rs`
  (schema 47), `cantus-ui/tests/mpe.rs` (note operations, linked copies, the boot path), the
  undo fuzz's round-33 stream (`round33_mpe_command`).
- Only a real controller and real MPE plug-ins can confirm the feel: `docs/beta-checklist.md`.

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
