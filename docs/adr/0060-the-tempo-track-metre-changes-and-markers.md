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
insert that hears the song late (delay compensation), at its own delayed step too
(review 1). The modulation family's clock re-anchors at each step from the beats (the
`samples` it is told are the beats at the step's tempo), so its synced phase is the
song's beats; at one tempo nothing changes, to the bit. A step reached by the engine's own
rounding counts as reached (`fx::step_of`). **Per-pass tempo with CLAP's `tempo_inc`**
was chosen over finer splitting: inside a ramp hosted plug-ins are told the exact tempo
at the block's first sample and its slope per sample (CLAP `tempo` + `tempo_inc`; VST3
`tempo`), the song's real seconds (`song_pos_seconds`, `projectTimeSamples`), the bar's
start and number through the metre changes. *Measured:* see §8.

**Unwarped audio clips** (`cantus_core::audio_time`, the one mapping the engine and every
interface site use): a clip plays at its natural speed **anchored at its start**, its
`offset` read at the tempo in force where it starts (Ableton's unwarped clip); with one
tempo this is exactly the old rule. Fades and comp-seam crossfades are measured through the
map. A recorded take is moved back by its latency's *seconds* through the map, so a take
played across a ramp lands where it was played. Pasting an audio clip where the tempo
under it differs keeps its seconds (the clipboard remembers where it was copied).

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
the loop lane while the song has markers: click to jump, click on the name's letters to
rename (ADR 0051's gesture), drag, recolour from the track palette; Alt+M adds one at the
playhead, Alt+, and Alt+. step between them, Alt+Shift+L loops between the markers around
the playhead; the ruler's menu lists them to jump by name; *Export between markers* is a
scope of the export dialog (`Scope::Markers`), the two markers picked by name.

### 6. Interchange (amends ADR 0021)

See §7 of `docs/round29/tempo.md` for what each format carries.

### 7. Licences

All of it is our own code in the MPL crates; the closed forms are textbook calculus.

### 8. Measurements

Filled from `tempo_map::measure_the_cost_of_ramp_steps` and the tests (see
`docs/round29/tempo.md`).

## Consequences

- Every old project plays, renders and exports exactly as before (no golden moved,
  `DSP_VERSION` unchanged); a project with a tempo track plays the same at every block
  size, offline as live, in parallel as serially.
- The warp builder extends `audio_time` for warped clips and keys renders on `span_key`;
  freeze measures lengths with the map; controllers set the tempo with `set_tempo_at`.
- Not built (owner's or later): Ableton Link and MIDI clock sync (BACKLOG §0.2) — the map's
  `tick_at_seconds`/`bpm_at` are what they will need.
