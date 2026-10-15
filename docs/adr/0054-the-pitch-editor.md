# ADR 0054 — The pitch editor: notes stored with the clip, a render played in the source's place

**Status:** Accepted · **Date:** 2026-10-15 · **Round:** 28 (pitch) · **Builds on:** ADR 0053
(the pitch base) · **Extends:** ADR 0004 (commands), ADR 0023 (analysis stored), ADR 0025
(audio clips), ADR 0031 (offline = real time) · **Amends:** ADR 0051 §4 (an audio clip
opens the pitch editor) · **Serves:** BACKLOG §3c

> Measurements and tests: `docs/round28/pitch.md`. The manual: §5, *The pitch editor*.

## Context

BACKLOG §3c asks for a Melodyne-class editor for recordings: notes found in a take (each
with its centre, drift and vibrato apart) and its key; per-note pitch, centre, drift,
vibrato, formant, amplitude, timing, split and join; macros; A/B; MIDI out; a double;
non-destructive, every edit undoable, the result bounced bit-identical to playback.
Round 28's *voice* builder left the base (ADR 0053 §4): the offline tracker with epochs
and the offline formant-preserving TD-PSOLA renderer with ratio and formant curves and a
time map, the source bit for bit wherever nothing is asked.

## Decision

### 1. Analysis (`cantus_engine::pitch`, off the audio and the UI threads)

- **The take** is analysed at its file's own rate (the edit is the file's, whatever the
  device's rate): the base's `analyse`, then **notes**: voiced runs (single-frame dropouts
  bridged) cut where the level dips 9 dB for a new syllable, then a least-squares cut of
  the pitch smoothed over a vibrato cycle (180 ms) into steps, steps nearer than half a
  semitone joined (a drift is not a new note), pieces shorter than 150 ms joined to the
  neighbour they are nearer (a scoop or a fall belongs to its note), each cut moved to
  where the pitch moves fastest. A note's **sung centre** is the mean of its voiced
  frames, a tenth trimmed at each end. **Key**: Krumhansl–Schmuckler over the notes'
  durations (major and minor on every tonic); fewer than three notes or no fit:
  chromatic.
- **Drift and vibrato kept apart** (`cantus_core::pitch::curves::decompose`): the drift is
  the note's pitch less its centre through two moving averages of 200 and 167 ms (zeros at
  5 and 6 Hz, where vibrato is); the vibrato is what is left. Sung pitch = centre + drift
  + vibrato, to the float.
- **Sibilants and breaths** are the tracker's unvoiced frames, told apart by its sibilance
  share; the editor marks them.
- **The service** (`PitchService`, one worker thread): analyses and renders queued, renders
  first, a newer render for a clip replacing a waiting one (a drag asks every frame; only
  the newest is made). Analyses are kept in memory and **on disk** in
  `<data>/cache/pitch/<key>.pitch` — the key FNV-1a over the file's *name*, size, time,
  the rate and `ANALYSER` (so a take that moves into the project's folder on its first
  save is still found; a better analyser analyses again). The cache dir is set in
  `CantusApp::assemble` (the boot path). A recorded take is analysed as it lands.

### 2. The model (schema 27): the segmentation is stored, edits refer to it

`AudioRegion::pitch: Option<Box<PitchEdit>>` — `rate`, `frames`, `analyser`, `key`,
`notes`, `bypass` (A/B). Each `PitchNote` stores **the segmentation** (`src_start`,
`src_end`, `sung`, `level_db`) and **the edits** (`pitch`, `centre`, `drift`, `vibrato`,
`formant`, `gain_db`, and the timing `start`/`end`). ADR 0023's principle: the analysis is
run offline and its result that the edits refer to is stored, so an improved analyser
never moves an old edit; the per-frame pitch and epochs (megabytes) stay in the cache and
are recomputed, deterministically, when missing. Nothing is stored until the first edit;
defaults are not written. Positions are **source frames**, so a split clip, a slip or a
second clip over the file read the edited take where they read the file; linked clips
share the edit (it is audio content, ADR 0037).

A note's shift (semitones), with `home = round(sung)`:
`(pitch − home) + centre·(home − sung) + (drift − 1)·d(t) + (vibrato − 1)·v(t)` — exactly
0 at the defaults. Moving a note keeps its deviation; *centre* 1 puts it on its note.
*Correct pitch* by an amount `a` sets `pitch = home + a·(target − home)` and `centre = a`,
so the note sounds at `sung + a·(target − sung)`: nothing at 0, its target at 1, never past
it, and run again at another amount it is as if run once (review 1). Which pitch is the
macro's is **stored on the note** (`PitchNote::corrected`, schema 30, review 2): set by
*Correct pitch*, cleared by a hand move — so a run after a key change re-targets the
macro's notes (they were left between semitones) and a hand move onto the key's note is
kept (it was taken back); an older file's notes are all the hand's. A step of the key
(↑/↓, a scale-snapped drag) from a note off the key lands on the key's next note, its cents
kept (`PitchKey::step`, `steps_to`).

The track keeps whether its **pitch lane** is shown (`Track::pitch_open`, schema 27,
`Command::SetTrackPitchOpen`).

### 3. The curves and the time map (`cantus_core::pitch::curves`)

- Shift and formant by source time; each note its own value, **ramps inside the gaps**
  (40 ms at most beside an edited note) keeping a **margin** of the renderer's fade + 2
  frames from an untouched neighbour, so its 5 ms cross-fade never reaches it. Notes sung
  legato (a gap under two margins) share a 15 ms transition across the joint.
- Gain by output time, carried across a note's joints the same way (ramps in the gaps,
  margins kept from untouched neighbours, a shared transition at a legato joint) — never a
  step, so a gain-only edit never clicks (review 1).
- **The time map**: every note's edges are anchors; inside a note it is stretched, in a gap
  the two sides' offsets are bridged between margins kept next to each side; a note moved
  by whole frames is read sample for sample. The take keeps its length. A source gap may
  shrink but never vanish (`is_valid`), so the map is a function.

### 4. Every edit a `Command`

`Command::SetClipPitch { track, clip, pitch, op }` replaces the whole edit; the inverse is
the edit as it was; `op` (`PitchOp`) names it for the undo menu (`cmd.pitch.*`) and decides
whether a run merges (values and drags do; splits, joins and macros do not). Refused on a
clip that is not audio and for an edit that is not valid. The edits themselves are pure
functions (`cantus_core::pitch::edit`: set a value, set pitch, timing, shift, split, join,
correct pitch/drift, quantise to a grid or to onsets — another track's notes —, reset)
that keep an edit valid — clamped, stopped at the neighbours. MIDI out (`pitch::midi::notes`) and *Double* (`pitch::midi::double`, a
diatonic third or fifth in the key, a major third or a fifth when chromatic) are batches of
ordinary inserts. Fuzz arm: `undo_invariants.rs` `pitch_command` (additive), including the
refused paths.

### 5. The render, played in the source's place

`render_edit` = the base's `render` with the curves and the map, then each note's gain; at
the file's rate, then converted to the engine's like any source. `SampleCache::attach_audio`
(the one path every playing snapshot takes — live, export, stems) attaches, for a clip
whose edit is audible, **the render** keyed by (file, edit, rate): made at once only in a
manual cache that asks for it (tests, one-off renders); `EngineHandle::compile` always
queues it on the service's worker — with a device or without (review 1: the app without a
device rendered on the UI thread at every drag frame). **While a render is pending the clip
plays the newest render it had *at the engine's rate*, or its source — never silence, never
a render made at another rate** — and `SampleCache::loading()` is true;
`EngineHandle::compile_for_render` (export, stems) waits for every pending render before its
second compile, so a file holds the edits made the same frame. Kept: each clip's last
three renders (undo and redo play theirs at once) and, while the clip is compared with its
original (*Original*), its edits' render (switching back plays it at once). A render
nothing keeps is freed only when the cache is its last holder (the audio thread only
decrements). A file that failed is tried again when the sample cache retries it (its
analysis and renders forgotten); an idle worker is woken by every new job. The same
snapshot data serves real time and the bounce: offline = real time and bounce = playback to
the bit.

### 6. The editor (`views/pitch*.rs`), and how it opens

The piano roll's place shows the pitch editor for an audio clip (as it shows the automation
editor for an automation clip). **ADR 0051 §4 amended:** an audio clip now has an editor of
its own — `ClipEditor::Pitch`, still derived from the content, nothing stored; its gain,
fades and file stay in the inspector. Also *Edit pitch* in the clip's menu and in the
inspector (with the A/B switch when the clip plays edits). Blobs on a piano-roll grid over
the waveform (read through the time map), as thick as the voice is loud; the pitch curves
(sung, dim; sounding, bright); sibilant and breath marks; the bars and the playhead; scroll
bars that zoom. Tools: *Edit* (select, pitch drag snapped to the scale, semitones or free —
Alt for cents —, time drag, stretch at the ends, double-click centre) and *Split*. The note
card and the macro card under it; the bar: snap, key, *Original*, join, reset, play from
the selection, MIDI (drag onto an arrangement track, or click for a new one), *Double*, the
quantise target (a grid, or another track's notes), the take's status. The bar is measured
with the widths it draws (review 2: two dividers and the … key were under-counted, so …
or the quantise field could vanish): what it has no room for (the status, then the
quantise target, play/MIDI/Double, join and reset, and on a very narrow bar *Original*, the
key and the snap) folds into a … menu that offers it — the melody onto an existing track
too, the MIDI chip's drag. The note panel keeps its knobs whole and
scrolls sideways when narrower than its cards. The editor builds its curves and time map
once per edit, not per frame. The key reads in the notes' letters, its mode in the
language's own words ("A minor", "A menor"). The clip's menu also makes MIDI and a double
without opening the editor.

### 7. The pitch lane (review 1; BACKLOG §3c item 4)

Under an audio track, shown from the track's menu (*Pitch lane*): its clips' notes (stored
edit, or the analysed notes) as bars where they play, at the pitch they sound, the track's
range fitted to the lane. Drag one up or down (the editor's snap; Alt for cents), or
double-click it to pull its centre onto its note — the editor's `SetClipPitch`, one undo
step a gesture — for small fixes without opening the editor. `TrackRow::pitch` is the
lane's height in the arrangement's layout, after the take and automation lanes. Review 2:
the lane takes presses only where it is seen (its part under the ruler, the scroll bar or a
panel takes nothing) and only through its own response (a window or menu above keeps its
presses); a gesture moves the pitch only once it goes up or down (a sideways slip changes
nothing, and a drag never commits an edit equal to the one it started from); the range of
the press holds through the drag, so the note stays under the pointer; the lanes are laid
out once per edit and view, not twice a frame.

### 8. Quantise to another track (review 1; BACKLOG §3c item 2)

*Quantise to* on the bar is a grid or a track: with a track, each note's first edge on the
timeline moves onto the nearest onset of that track within an eighth note — a note clip's
notes as they play (loops repeated), or an audio clip's pitch notes — its length kept, its
neighbours untouched (`edit::quantise_to`, which the editor runs; review 2). Any audio track
whose take is analysed is offered, edited or not (the lead a double is lined up with); a
target whose track is gone is no target — the bar names the grid, and the grid is used.

## Consequences

- The render is the whole take (0.76 s for a three-minute stereo take with every note
  corrected; analysis 2.3 s, once, then cached). An edit is heard a fraction of a second
  after it is made; the first open of a long take waits for its analysis.
- Monophonic only (one voice or line): chords are not split; ARA hosting and polyphonic
  editing stay BACKLOG §3c "later". A clip played backwards plays the edited take
  backwards; the editor's ruler, quantise (the note's source end, its start on the
  timeline) and MIDI out read it as it sounds (`ClipTime` mirrors the window as the engine
  does).
- The renders live in memory (a stereo three-minute render at 48 kHz is 69 MB); the ones
  playing, each clip's last three and a compared clip's are kept.
- Glue refuses audio clips whose pitch edits differ (one half's corrections would be lost);
  the comp swipe keeping a comp clip's pitch edits is the comping builder's (`comping.rs`).
- MIDI out gives the notes the clip plays: its window (one period of a loop, repeated to
  its end), a note across an edge cut there.

## What the tests hold

Review 1 added (each seen failing first where it could be): `cantus-core` gain never steps
at a legato joint or a short gap; correct pitch below 100 % never overshoots, 0 % leaves a
note alone, 100 % then 50 % is 50 %; a key step from an off-key note; quantise to onsets; a
valid edit refused on a note clip (and the fuzz arm sending one); `SetTrackPitchOpen` in the
fuzz; the fixture moves a note, stores A/B and a shown lane. `cantus-engine`: switching back
from *Original* plays the kept render with nothing pending; undo and redo play their own
renders at once; a render at another rate is never the fallback; without a device renders
go to the worker and `compile_for_render` waits; a failed analysis is analysed again once
the file is back (this found the idle worker never woken); the sample cache's retry forgets
it. `cantus-ui`: key steps from an off-key note; Alt+arrows with the pointer away; a
reversed slice fits, rules and plays from its window; MIDI out windowed; glue refuses
different edits; the panel fits any width; a narrow bar folds into … with every action in
its menu; quantise onto another track's notes; the clip menu's MIDI and Double; the
inspector's tips; an idle editor builds nothing; the pitch lane (shown in one step, a drag
and a double-click, undone); the demo's double plays its own render; the boot test in its own
binary (`pitch_boot.rs`).


- `cantus-core` `pitch_tests.rs` (21): defaults ask for nothing, to the bit; a moved note
  shifts only itself and never its neighbours or their margins; centre and moves; the
  decomposition (drift within 5 cents, vibrato within 6, their sum the sung pitch); drift
  and vibrato scales; formant and gain inside their note; the time map maps only the edited
  note and its gaps; timing stops at neighbours; split/join round trip; correct pitch into
  the key; quantise; reset; the key's snap and degrees; MIDI out and the double; serde; the
  validity of 1 200 random edits through every function. `undo_invariants.rs`: the pitch
  arm (set and cleared both applied).
- `cantus-project`: `schema_27_keeps_a_takes_pitch_edits`, every fixture loads.
- `cantus-engine` `pitch_editor.rs` (12): notes and key found; corrected within 5 cents;
  formants at ±3 st; a timing edit leaves its neighbours bit for bit; nothing asked = the
  take; gain; the engine plays the render, offline = real time; A/B and untouched play the
  file; the original while pending, `loading()` true; the disk cache; a legato move does not
  click; the three-minute timing.
- `cantus-ui` `pitch_editor.rs` (10), `quick_gestures.rs`, `rec_fixes.rs`: through the
  window — double-click opens it, a drag is one undo step undone exactly, split, the edit
  keys through the keymap (`CantusApp::pitch_action`), Ctrl+J,
  the macro, *Original*, MIDI out (clicked and dragged onto a track), the double, the boot
  path's cache, the demo project plays its edits.
