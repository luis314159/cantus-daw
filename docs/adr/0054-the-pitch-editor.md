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

### 3. The curves and the time map (`cantus_core::pitch::curves`)

- Shift and formant by source time; each note its own value, **ramps inside the gaps**
  (40 ms at most beside an edited note) keeping a **margin** of the renderer's fade + 2
  frames from an untouched neighbour, so its 5 ms cross-fade never reaches it. Notes sung
  legato (a gap under two margins) share a 15 ms transition across the joint.
- Gain by output time, ramped inside the note.
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
correct pitch/drift, quantise, reset) that keep an edit valid — clamped, stopped at the
neighbours. MIDI out (`pitch::midi::notes`) and *Double* (`pitch::midi::double`, a
diatonic third or fifth in the key, a major third or a fifth when chromatic) are batches of
ordinary inserts. Fuzz arm: `undo_invariants.rs` `pitch_command` (additive), including the
refused paths.

### 5. The render, played in the source's place

`render_edit` = the base's `render` with the curves and the map, then each note's gain; at
the file's rate, then converted to the engine's like any source. `SampleCache::attach_audio`
(the one path every playing snapshot takes — live, export, stems) attaches, for a clip
whose edit is audible, **the render** keyed by (file, edit, rate): made at once in a manual
cache (tests, offline), queued on the service's worker in a live one. **While a render is
pending the clip plays the last render it had, or its source — never silence**, and
`SampleCache::loading()` is true, so export and stems wait (they already wait for
`loading()`, ADR 0050 §7). A render nothing plays any more is freed only when the cache is
its last holder (the audio thread only decrements). The same snapshot data serves real time
and the bounce: offline = real time and bounce = playback to the bit.

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
quantise grid, the take's status.

## Consequences

- The render is the whole take (0.76 s for a three-minute stereo take with every note
  corrected; analysis 2.3 s, once, then cached). An edit is heard a fraction of a second
  after it is made; the first open of a long take waits for its analysis.
- Monophonic only (one voice or line): chords are not split; ARA hosting and polyphonic
  editing stay BACKLOG §3c "later". A clip played backwards plays the edited take
  backwards; the editor's ruler, quantise (the note's source end, its start on the
  timeline) and MIDI out read it as it sounds (`ClipTime` mirrors the window as the engine
  does).
- The renders live in memory (a stereo three-minute render at 48 kHz is 69 MB); only the
  ones playing and each clip's last are kept.

## What the tests hold

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
- `cantus-ui` `pitch_editor.rs` (8), `quick_gestures.rs`, `rec_fixes.rs`: through the
  window — double-click opens it, a drag is one undo step undone exactly, split, Ctrl+J,
  the macro, *Original*, MIDI out (clicked and dragged onto a track), the double, the boot
  path's cache, the demo project plays its edits.
