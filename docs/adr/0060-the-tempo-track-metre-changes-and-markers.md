# ADR 0060 — The tempo track, metre changes and markers

**Status:** Accepted · **Date:** 2026-10-16 · **Extends:** ADR 0006 (ticks, never seconds),
ADR 0021 (MIDI and DAWproject — amends its dropped tempo/metre/marker items), ADR 0025 §1
(audio plays at its natural speed), ADR 0027 (automation is a function of the tick),
ADR 0055 (the modulation family's clock), ADR 0057 (modulators)

## Context

Until round 29 a project had one tempo (`tempo_bpm`) and one metre (`time_signature`):
the engine's transport counted `origin + samples · ticks-per-sample`, every bar line was a
multiple of one bar from tick 0, and a tempo edit was a *jump* (the transport re-anchored).
ADR 0027 said a tempo track "changes the tick↔sample map the whole engine is built on".
The owner asked (2026-10-16, item 4) for a tempo track with ramps and metre changes along
the song and named markers (jump, loop, export between them), with the whole engine,
automation, modulators, synced effects and hosted plug-ins following the map, and MIDI and
DAWproject carrying it. FL Studio (tempo automation, time-signature markers, playlist
markers), Ableton (tempo and signature changes, locators), Cubase and Studio One (tempo
track with ramps, signature track, marker track) and Logic (tempo curves) were compared.

## Decisions

### 1. The model (schema 33)

`Project` gains `tempo_points: Vec<TempoPoint { tick, bpm, ramp }>`, `metre_changes:
Vec<MetreChange { tick, signature }>` and `markers: Vec<Marker { id, tick, name, color }>`,
each `#[serde(default, skip_serializing_if = "Vec::is_empty")]`: a project without them
saves the same bytes as before. With a tempo track its first point is at tick 0 and *is*
`tempo_bpm` (both kept in step by every command); likewise the first metre change and
`time_signature`. Positions stay in ticks (ADR 0006): nothing in the song moves when the
tempo changes. Markers are kept sorted by tick then id and named by a stable `MarkerId`.

Commands, each with its exact inverse (ADR 0004) and a `cmd.tempo.*` label:
`SetTempoTrack` (the whole track and the tempo at 0), `InsertTempoPoint`,
`RemoveTempoPoint`, `SetTempoPoint` (a point moved, its tempo or its ramp — kept between
its neighbours; merged per point in a drag), `SetMetreTrack` (the whole list),
`SetMetreAt` (a change added at, or edited on, a bar line), `RemoveMetreAt`, `MoveMetre`,
`AddMarker`, `RemoveMarker`, `SetMarker` (moved, renamed, recoloured; merged per marker).
**Metre edits keep the music where it is** (review 1): every change before the edited
one keeps its tick, the edited one lands exactly where asked (a move counted along the
bar lines of the metre before it), and the later changes move so that each keeps its bar
number and any short bar before it. `SetTimeSignature` on a project with metre changes is
`SetMetreAt` at tick 0. A change that falls off the bar lines of the metre before it (an
imported file) is valid: the bar before it is short, and the ruler shows it so.

### 2. What the top bar's tempo edits; tap tempo

The tempo field shows the tempo **at the playhead** (inside a ramp, the ramp's value
there); the metre field the metre in force there. Editing either sets the point (or the
metre change) that **governs the playhead** — the last at or before it; for a ramp, the
value it starts from — and without a tempo track the song's one tempo, as before
(`tempo::edit::set_tempo_at`, `set_metre_at`). Tap tempo does the same, one undo step per
run. This is Studio One's and Logic's behaviour (Cubase only greys the field when its tempo
track is on); a controller mapped to the tempo (round 29's controllers builder) uses the
same function.

### 3. A ramp is linear in ticks

A ramp moves the tempo linearly **in beats** from one point's value to the next's, as
Cubase, Studio One and Bitwig draw it — and as the timeline, linear in beats, shows it: a
straight line. The time it takes integrates in closed form: over `Δt` ticks from `b₀` at
`k` BPM per tick, `seconds = 60 / (PPQ·k) · ln(1 + k·Δt / b₀)` and back
`Δt = b₀ / k · (e^{s·PPQ·k/60} − 1)`, with `cantus_dsp::math::{ln_1p, exp_m1}` (deterministic
on every machine). Before tick 0 (a count-in, a pre-roll) the first tempo and the first
metre hold backwards. `TempoMap` precomputes one segment per point (start tick, start
second, start tempo, slope), finds them by binary search and never allocates on a lookup;
the metre side counts bars through every change (bar lines, beats, BBT, bar/beat snaps).
Over a stretch where one tempo holds the old constant arithmetic is used unchanged —
so **a project without a tempo track renders bit-identically** (every golden, the null-hash
corpus) and `seconds_between` inside a held stretch is `(to − from).to_seconds(bpm)` to
the bit. `span_key(from, to)` is the tempo function over a span measured from its start
(a warped clip's or a frozen strip's render key).

### 4. The engine follows the map

**The transport counts song samples through the map.** The position at a sample is the
map's tick `samples / rate` seconds after the origin; an event's sample is the map's
seconds to its tick, rounded up as before. A tempo point or a ramp is therefore **not a
jump**: only a seek, a loop's wrap or an edit of the tempo re-anchors (and a metre edit
does not). `position_back` is exact across points; a loop over a tempo change wraps after
exactly the map's loop length (the late inserts' `Wraps` too, no longer "at the tempo
now"). The offline renders measure their length and stop sample through the same map.

**Units hear the tempo in steps.** Notes, clips, automation and the click are exact (they
are functions of the tick). A unit that follows the tempo (the modulation family, Speculum,
hosted plug-ins) is told one tempo per pass. A pass therefore ends at every tempo point
and, inside a ramp, every `RAMP_STEP` (60 ticks, a 64th note) counted from the ramp's
start — tick positions, so the same at every block size, offline as live — and, for every
insert that follows the song and hears it late (delay compensation), at its own delayed
step too (review 1), folded through a loop's wraps as the position it is told is
(`fx::heard_tick`, one rule for both: review 2). Only units that read the position
(`FxUnit::follows_song`: the modulation family, a watched Speculum, hosted plug-ins) are
told it or cut passes for it — an equaliser behind a look-ahead costs nothing here
(review 2). The modulation family's clock re-anchors at each step from the beats (the
`samples` it is told are the beats at the step's tempo), so its synced phase is the
song's beats; at one tempo nothing changes, to the bit. A step reached by the engine's own
rounding of a cut — a millionth of a *sample* — counts as reached (`fx::step_of`).
**Per-pass tempo with CLAP's `tempo_inc`** was chosen over finer splitting: inside a ramp
hosted plug-ins are told the exact tempo at the block's first sample and its slope per
sample (CLAP `tempo` + `tempo_inc`; VST3 `tempo`), the song's real seconds
(`song_pos_seconds`) and samples (`projectTimeSamples`, rounded once by the engine), the
bar's start and number through the metre changes; at one tempo both are the expressions of
round 28, to the bit (review 2). *Measured:* see §8.

**Unwarped audio clips** (`cantus_core::audio_time`, the one mapping the engine and every
interface site use). Two invariants hold, and the owner holds us to both: **(i) a tempo
edit never changes which source frame plays at an unwarped clip's start** — a take
recorded from bar 17 after playing from bar 1, its loop passes, its comp windows, a
reversed clip, a punch-in, all keep their audio where it was played when bar 1's tempo is
changed; and **(ii) every window edit is exact** — split, trims, comp swipes and seams,
glue, loop passes, pastes, moves of several clips — so the windows of one take keep
agreeing with each other. The final rule (review 3) has two parts:

1. *The reading rule* — a clip plays at its natural speed from its **content origin**
   (`start − offset`, ADR 0025's origin), laid on the timeline through the map; with one
   tempo this is exactly the old rule. Every window of one take shares its origin, so the
   tick arithmetic of every window edit is exact through any map (invariant ii). Review 2
   chose this over "anchored at its start, the offset read at the tempo there", with which
   every window edit needed its offset re-derived in seconds and the windows of a take
   stopped agreeing.
2. *Tempo edits re-anchor* — alone, the reading rule broke invariant (i): a tempo edit
   inside a clip's hidden part (a take's pre-roll, the passes before a loop's last, a
   count-in) slid what it played (review 3, findings 0, 13, 37). So the command bus
   (`CommandBus::apply_keeping_audio`, `tempo::keep`) wraps **every command that changes
   the map** — `SetTempo`, `SetTempoTrack`, `Insert/Remove/SetTempoPoint`, or a batch
   holding one: it notes the source second at the start of every unwarped window (clips
   and the alternate comps' windows; warped clips that follow the tempo are the map's by
   definition and are left alone), applies the command, and re-anchors each window whose
   second moved (`audio_time::anchor`: the offset in whole ticks, the remainder in
   `slip_frames`), composed into the same undo step with its exact inverse; if a
   re-anchor were refused, the whole command is rolled back. A window the command itself
   moved or re-cut is left as the command left it. Windows of one take are re-anchored to
   the seconds they played, so they still meet and glue (within the sub-tick rounding a
   slip absorbs); a linked copy cannot hold its own slip (it shares its group's region,
   ADR 0037), so it is re-anchored by its offset alone, within half a tick. The undo fuzz
   checks after every tempo command that each window's frame at its start is kept and
   that undo restores the project to the bit.

Design (a) — storing the left edge in source frames — was weighed and rejected: it moves
every window edit off tick arithmetic onto seconds (the review-2 problem back) and needs a
schema change for every clip, while (b) touches only the commands that move the map and
keeps both invariants with one inverse. Its cost: a tempo edit is a batch that also writes
the clips' offsets (one undo step all the same). It holds at one tempo too: the top bar's
tempo changed in a song without a tempo track keeps each trimmed clip's frame at its start
(round 28 slid it by the change in its offset's seconds); playback of every project as
saved is unchanged, to the bit — only an edit of the tempo now re-anchors.

Moving and copying go through the map too: **one block** — a selection moved, nudged,
drag-copied or pasted — moves its earliest clip by the delta and lays every unwarped
window the same *seconds* after it (`audio_time::block_start`), so windows that met keep
meeting and a take keeps one origin; a single window lands at the same second at its new
start with its length and fades in seconds (`audio_time::relocate`: a drag, a drag-copy, a
duplicate and a paste give the same clip; a looping clip and a span of one tempo keep
their ticks; a linked copy keeps its group's fades). The drag draws what it will drop
(`clips::drag_preview` is the commit's own commands). A **slip** moves the source under
the window's start by the seconds the pointer's travel lasts there
(`audio_time::slipped`), not across the hidden part. A **reversed** window reads back from
its end in seconds — its frame at tick T is its start frame plus the seconds from T to its
end — the engine, the pitch editor and the waveform alike (`reanchor_reversed` at its
trims, splits and glue). Fades and comp-seam crossfades are measured through the map,
where they lie. A recorded take is moved back by its latency's *seconds* through the map;
its loop passes' fine offsets put each pass's recorded frame at its window. A warped clip
that keeps its natural speed (a warp that does not follow) reads its hidden part too, so
its render key must hash the map from its origin (the warp builder's `warp_render::key`).

**Modulators** read time through the map: `ModTime` carries it while it moves, an
envelope's note gates are computed in song seconds, a retriggered LFO in Hz counts the
seconds since its note through it; *Bake to automation* does the same.

**Spatium's synced pre-delay** stays what it is: a time set from a note value at the
tempo where the user set it (its editor's *resync* hint compares it with the tempo at the
playhead). A reverb's pre-delay changing under a ramp would pitch its tail.

### 5. The interface

One helper (`views/bars.rs`) counts every ruler, grid, BBT readout and bar/beat snap
through the map (with one metre, the old arithmetic); the grid menu's *Bar* means the bar
of the metre in force. The **tempo track** is a lane under the ruler (shown from the View
menu, the ruler's menu or its key; by default while the project has one): points on a bpm
axis, steps held flat, ramps straight; double-click adds, drag moves (Shift fine, Ctrl off
the grid), Alt+click ramps or holds, double-click types a value, the menu removes. **Metre
changes** are chips on the ruler before the bar's number: drag along the bar lines, click
to edit (the top bar's metre editor), remove from the menu. **Markers** are a lane under
the loop lane while the song has markers: a click anywhere on a tab jumps (review 2: the
name's letters filled the tab, and a click there renamed), a double-click or the menu
renames, drag, recolour from the track palette; markers on one tick share the room before the next marker, and a run too close behind
another starts after it (review 3); a tab or a metre chip drags by the pointer's travel,
never by where it was grabbed;
Alt+M adds one at the playhead, Alt+, and Alt+. step between them, Alt+Shift+O loops
between the markers around the playhead — a letter, on its key on every layout (review 3:
Alt+Shift+, cannot be typed on Spanish, German or French keyboards, where the comma's
shifted key is another sign; Alt+Shift+L was Select linked's in every preset);
the ruler's and the empty timeline's menu (one menu) add a marker or a metre change where
it was opened, the toolbar's ▸ menu at the playhead; *Add a metre change* at bar 1 opens
the song's own metre; the lane's menu lists the markers to jump by name; *Export between
markers* is a scope of the export dialog (`Scope::Markers`), the two markers picked by name
— a pair that makes no range is refused with a word, never exported as the whole song.
The tempo lane's scale holds while a point is dragged, and the top bar's wheel (a notch,
one BPM) and drag move the governing point by their own amount, a ramp's start from its
own value (review 2), the wheel counting a touchpad's fractions into whole notches and
leaving Ctrl/Shift+wheel to the view (review 3); a point dragged past the lane's range
slides the range with it, the scale held. The ruler's menu adds on the grid when snap is
on; *Add a metre change* opens its editor where it is drawn, in view. The lanes' state
belongs to one project.

### 6. Interchange (amends ADR 0021)

ADR 0021 dropped, with a report, a MIDI file's tempo changes (kept by beats or by seconds)
and a DAWproject's `TempoAutomation`, `TimeSignatureAutomation` and `Markers`. Now:

- **MIDI export** writes the whole conductor track: the metre in force and every metre
  change, every tempo point — a ramp as a tempo event every sixteenth note
  (`RAMP_EXPORT_STEP`), each the *average* tempo of its sixteenth, so every sixteenth and
  the whole ramp last exactly as long as they play — and every marker as a marker meta
  event (0x06); a clip or a range starts the map at its own start.
- **MIDI import** has a new default mode, *keep the tempo map* (`TempoMode::Map`): notes by
  beats, and — when the import sets the song's tempo — the file's tempo changes in the tempo
  track, its metre changes and its markers; the report says how many of each were kept.
  Dropped at the song's start the file's map **replaces** the song's tempo track and metre
  changes (the song's markers stay, the file's join them); dropped at a later bar it starts
  at the bar line nearest the drop, where the notes go too: the song keeps its own tempo
  and metre before it, a ramp into it ending where it stood (review 2). A signature Cantus
  cannot hold is dropped and counted, two at one tick keep the later, a first one written
  a few ticks in (within a sixteenth) is the song's, as is a first tempo (review 3: a whole
  bar moved a pickup's 3/4 to tick 0); a merged map left with one tempo clears the song's
  later points; an SMPTE file keeps no map; markers kept are no texts dropped. `Beats` and
  `Seconds` stay for importing into a song whose tempo stays; in `Seconds` (and every SMPTE
  file) into a song with a tempo track the notes are placed through the song's map, at the
  seconds they were counted at (review 3). The export writes the map and the metre changes
  past the last clip, so a song's tempo map alone exports.
- **DAWproject** writes the arrangement's `Markers` (name, colour), `TempoAutomation`
  (`RealPoint`s on the transport's tempo parameter; a ramp `linear`, a held point `hold`)
  and `TimeSignatureAutomation` (`TimeSignaturePoint`s), valid against the official schema,
  and reads them back; an audio clip whose content is in seconds plays its `playStart`
  second at its start through the map (`audio_anchor`, the rule above; review 3) — the
  unwarped audio's own `<Warps>` line is the warp builder's (`dawproject/audio.rs`).
  Times in seconds are placed through the file's own map — its tempo
  points one after another, then every clip, note, marker and metre (review 2) — a ramp
  written in seconds becomes one linear in beats (reported); two points at one time are a
  jump (the later wins); the lane's `unit` is read (`normalized` through the transport's
  tempo range, another dropped with a report). Markers inside lanes or clips are still
  counted as dropped (nowhere to go). **MIDI export** keeps the markers at and after the
  music's end.
- **The AI bridge** sees every tempo point, metre change and marker in its context and has
  a `set_tempo_point` tool (an ordinary, undoable command).

### 7. Licences

All of it is our own code in the MPL crates; the closed forms are textbook calculus.

### 8. Measurements

- **The cost of the ramp's steps** (`tempo_map::measure_the_cost_of_ramp_steps`, best of
  five, 48 kHz, 256-sample blocks, the demo song with a synced tremolo): cutting a pass at
  every 60-tick step adds about 16 % more passes at 90–140 BPM; the same one-tempo song
  rendered in 16 % more passes costs **1.8 %** more. The ramped song as a whole costs 9–15 %
  more per song second than at one tempo, most of it the denser notes of its faster tempo.
  Splitting at the automation grid (16 samples) instead would multiply the passes by ~16:
  rejected — the per-pass tempo with `tempo_inc` is exact for the engine's own timing and
  within a sixtieth of a beat's tempo change for the units.
- **Latent inserts in a ramp** (review 2,
  `tempo_review2::measure_the_passes_of_latent_inserts_in_a_ramp`, the demo over a
  90→140 ramp, every strip behind a look-ahead limiter and an equaliser, 512-sample
  blocks, ten seconds): one tempo 938 passes; the
  ramp with nothing late 1 193; with the latent inserts and no unit that follows the song
  **1 193** (1 703 before review 2 cut only for followers); with a synced tremolo too
  1 448 (1 703 before). At the measured 1.8 % per 16 % more passes, a mix of latent
  plug-ins and no synced unit costs nothing more in a ramp.
- **Accuracy**: seconds ↔ ticks inverse within 1e-9 s over 300 random maps of up to 8
  points (tempos 20–999); a ramp's closed form within 1e-9 s of Simpson's rule; an event
  after a ramp due at the analytic sample, to the sample; a loop over a tempo change
  wraps within 1e-3 sample of the map's length over 40 passes; a MIDI round trip of a ramp
  within 1e-4 s at every sixteenth.
- **Real time**: `rt_memory::a_tempo_map_never_allocates` (serial and with the pool): a
  ramp, steps, metre changes, a loop over the tempo change, a tempo edit while playing, a
  count-in, a stop — no allocation, no free on the audio thread or a worker.

## Consequences

- Every old project plays, renders and exports exactly as before (no golden moved,
  `DSP_VERSION` unchanged); a project with a tempo track plays the same at every block
  size, offline as live, in parallel as serially.
- The warp builder extends `audio_time` for warped clips and keys renders on `span_key`;
  freeze measures lengths with the map; controllers set the tempo with `set_tempo_at`.
- Not built (owner's or later): Ableton Link and MIDI clock sync (BACKLOG §0.2) — the map's
  `tick_at_seconds`/`bpm_at` are what they will need.
