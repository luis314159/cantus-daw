# Round 28 — pitch: the pitch editor for recordings (Melodyne-class)

*Builder: pitch · branch `r28/pitch` (from `r28/integration` at `42fe33b`, voice merged) ·
ADR 0054 (amends ADR 0051 §4) · schema 27 · BACKLOG §3c.*

## Done

- **Analysis** (`cantus-engine/src/pitch_take.rs`, on the pitch service's worker): the
  base's offline tracker at the file's own rate, then **notes** (voiced runs cut at level
  dips and at pitch steps of the vibrato-free pitch; drifts and scoops stay inside their
  note), each note's **sung centre**, its **drift and vibrato kept apart**
  (`cantus-core` `pitch::curves::decompose`), **sibilants and breaths**, and the **key**
  (Krumhansl–Schmuckler). Cached in memory and in `<data>/cache/pitch/` (keyed by file name,
  size, time, rate, analyser version); the cache folder is set on the boot path; a take is
  analysed as it lands (recording) and on demand (opening the editor).
- **The model, schema 27**: `AudioRegion::pitch` (`cantus-core/src/pitch.rs`): the stored
  segmentation and every per-note edit (pitch, centre, drift, vibrato, formant, gain,
  timing), the key, A/B. Migration `26 → 27` (writes nothing), `fixtures/v27.cantus`.
- **Every edit a `Command`**: `SetClipPitch` (whole edit, exact inverse, `PitchOp` label
  per edit, a drag merges), the edits as pure functions (`pitch::edit`), a fuzz arm.
- **The editor** (`cantus-ui/src/views/pitch*.rs`, `src/pitch.rs`), in the piano roll's
  place for an audio clip: blobs as loud as the voice over the waveform, the sung and
  sounding pitch curves, sibilant/breath marks, bars and playhead, scroll bars that zoom,
  the wheel as in the piano roll. Per note: pitch (snap to the scale, semitones or free,
  Alt for cents), centre (also by double-click), drift, vibrato, formant, gain, timing
  (move, stretch either end — neighbours untouched), split, join. Macros: correct pitch,
  correct drift, quantise to the grid. Key (detected, editable), *Original* (A/B), reset,
  play from the selection, **MIDI out** (drag the chip onto an instrument track, or click
  for a new one), **Double** (a third or a fifth above, in the key, on its own track).
  Opened by a **double-click** on an audio clip (`ClipEditor::Pitch`, ADR 0051 §4 amended),
  *Edit pitch* in the clip's menu and in the inspector (with an A/B switch). A clip that
  plays edits carries a *Tuned* badge in the arrangement.
- **Resynthesis** (`pitch_render.rs`): the base's TD-PSOLA with the edits' ratio, formant
  and time curves, ramps inside the gaps (margins keep untouched neighbours bit for bit),
  legato joints crossed in 15 ms. **Non-destructive**: the file is never touched; the
  sample cache attaches the render in the source's place (keyed by file + edit + rate), made
  off the audio thread; **while pending, the last render or the source plays — never
  silence** — and `SampleCache::loading()` holds export and stems until it arrives. Renders
  nothing plays are freed only when the cache is their last holder.
- **Demo project** (`src/pitch_demo.rs`, `examples/pitch_demo.rs`), **shots**
  (`examples/shoot_pitch.rs`), **manual** EN/ES §5 *The pitch editor*, **ADR 0054**.

## Measurements

| What | Measured |
|---|---|
| Correct pitch 100 % on a synthetic take sung +35, −28, +23 (vibrato ±30), −40 cents | **+0.26, +0.18, −0.69, +0.02 cents** from the targets (bar: 5) |
| Formants kept, envelope distance (dB RMS, harmonics 300 Hz–4 kHz) at +3 / −3 st | 110 Hz man: 0.57 / 0.34 · 180 Hz: 1.26 / 1.09 · 220 Hz woman: 1.42 / 1.12 · 300 Hz: 2.67 / 1.23 — formants moved with the pitch: 6.7 – 9.6 |
| A note moved 25 ms later and stretched 10 % | its three neighbours **bit for bit** |
| Nothing asked, *Original*, untouched | the file, bit for bit |
| Bounce vs playback | the bounce is the render sample for sample; real time at blocks 17…1024 = offline, tolerance 0 |
| Legato note moved a major third | second difference at the joint 0.0070, steady voice 0.0063 (no click) |
| Detection on the take | 4/4 notes, edges within 30 ms, centres within 3 cents; key C major / A minor |
| Three-minute take (228 notes) | analysis **2.31 s** (once, then cached); stereo render of every note corrected **0.76 s** |
| Undo | every edit one step, undone exactly (fuzz over 40 seeds; a window drag undone to the bit-equal project) |

## Tests and what each holds

- `cantus-core` `src/pitch_tests.rs` (21): the defaults ask for nothing to the bit; a moved
  note never touches its neighbours or their margins; centre/move arithmetic; drift and
  vibrato apart (5/6 cents) and summing to the sung pitch; each scale removes its own part;
  formant and gain stay inside their note; the time map; timing clamps; split ↔ join;
  correct pitch into the key; quantise; reset; key snap and degrees; MIDI out; the double;
  serde (defaults not written); validity; 1 200 random edits stay valid.
  `tests/undo_invariants.rs`: `pitch_command` arm (+ set/cleared both applied).
- `cantus-project` `format.rs`: `schema_27_keeps_a_takes_pitch_edits`; every fixture loads.
- `cantus-engine` `tests/pitch_editor.rs` (12): the figures above, the engine playing the
  render (offline = real time), A/B and untouched play the file, the original while
  pending and `loading()` true, the disk cache read back equal, no click, the timing.
- `cantus-ui` `tests/pitch_editor.rs` (10): double-click opens it fitted; a drag is one undo
  step undone exactly; split tool, Ctrl+J, the macro key, *Original*; the edit keys (Ctrl+A,
  ↑ a step of the key, Shift+↑ an octave, Alt+↓ ten cents, Esc keeps the clip); MIDI out (click and
  drag onto a track); a reversed clip's MIDI in sounding order; the double (one step); the
  boot path's cache folder; the demo opens and plays its edits. `fx_lint.rs` covers the
  five view files. `quick_gestures.rs` / `rec_fixes.rs` updated for ADR 0051 §4's amendment.

## Files outside my area (additive)

- `cantus-core`: `audio.rs` (`AudioRegion::pitch`), `command.rs` (`SetClipPitch`: variant,
  label, supersedes, apply), `lib.rs` (`pub mod pitch`), `snapshot.rs`
  (`AudioClipSnapshot::pitch`), `tests/undo_invariants.rs` (the arm, `below(41)`, a counter).
- `cantus-project`: `format.rs` (27, `migrate_25_to_26` placeholder, `migrate_26_to_27`,
  the test), `examples/write_fixture.rs`, `fixtures/v27.cantus`.
- `cantus-engine`: `lib.rs` (`pub mod pitch`), `samples.rs` (`decode_audio` pub(crate), the
  renders field, a block in `attach_audio`, one line each in `poll_loaded`/`loading`,
  `pitch()`, `pitch_pending()`, `pitch_held()`).
- `cantus-ui`: `lib.rs`, `views/mod.rs` (one line each), `app.rs` (the `pitch` field, the
  boot path's cache folder, and one hook at the top of `run_action`: the keymap's edit
  actions go to the pitch editor while it has the focus), `clips.rs` (`ClipEditor::Pitch`, `changes_content`),
  `gestures.rs`, `views/piano_roll.rs` (an audio clip → the pitch editor),
  `views/arrangement_audio_menu.rs` (*Edit pitch*), `views/arrangement_audio.rs` (badge),
  `views/inspector_audio.rs` (button, A/B), `record_session.rs` (a landed take analysed),
  tests `fx_lint.rs`, `quick_gestures.rs`, `rec_fixes.rs`; `docs/manual/{en,es}/README.md`.

## Left

- **Owner only:** judging corrections by ear on a real recorded voice (the demo's *Your
  voice* track: record, double-click the take), and the blind comparison against Melodyne
  / Newtone of BACKLOG §3c.

## Notes for the integrator

- **Schema:** topbar's step is 26. Here `migrate_25_to_26` is an empty placeholder — keep
  topbar's in its place and this branch's `migrate_26_to_27`; regenerate `v27.cantus` after
  the chain (`write_fixture`), and later schemas (leaders 28/29) chain after 27.
- `record_session.rs` is leaders' file: my hook is five lines after `self.rec.dropped =
  dropped;` in `finish_pass_at`.
- The key `audioclips.piano_roll` is no longer drawn (the piano roll shows the pitch editor
  for an audio clip); harmless, its area's owner may drop it.
- `inst_stream_review::a_late_start_two_or_three_octaves_up_is_kept_ready_at_its_level`
  failed once under load (a 200 ms wait while two other builders compiled) and passed twice
  on its own: not touched here.
- Kept in BACKLOG §3c as "later": polyphonic editing and ARA hosting. (The *Pitch* lane
  and quantise to another track were built in review 1.)
- The demo for the owner: `cargo run -p cantus-ui --example pitch_demo -- <folder>`; one is
  written at `F:\projects\daw-r28\shots\pitch\demo\pitch-demo.cantus`.

## Review 1

31 confirmed findings (`reviews/pitch-1.json`), 25 after duplicates, plus §3c's two items
the brief had left out. Each fix has a test seen failing first where it could; none was
judged wrong. Commits `…` after `448f48f` on `r28/pitch`.

| # | Finding | Fix | Test |
|---|---|---|---|
| 0 | Note gain stepped at a legato joint / short gap (click) | gain follows the shift's rule: ramps in the gaps, shared at a joint (`Curves::gain_db_at`) | `a_notes_gain_never_steps_at_a_legato_joint_or_a_short_gap` (6 dB step before; < 0.05 dB now) |
| 1 | Scale drag and ↑/↓ from an off-key note landed off the key | `PitchKey::step` / `steps_to` (cents kept, the next note of the key) in the drag, the keys and the lane | `a_scale_step_from_an_off_key_note_lands_on_the_key`, UI `a_key_step_from_an_off_key_note_lands_on_the_key` |
| 2, 30 | Reversed clip opened fitted to the whole take | frames ordered before clamping | `a_reversed_slice_opens_on_its_slice_with_its_bars` |
| 3 | Reversed play-from-selection started near the note's end | the earliest edge on the timeline (`pitch_play_from_selection`) | same test |
| 4 | Pending fallback could play a render at another rate | fallback only at the engine's rate; a clip's history kept while its source loads | `a_render_at_another_rate_is_never_the_fallback` (mutation-checked) |
| 5, 6, 21, 29 | A/B: switching back played the original for a re-render | a compared clip keeps its edits' render | `switching_back_from_the_original_plays_the_edits_at_once` |
| 7 | Alt+↑/↓ only with the pointer over the editor | whenever the editor has the edit focus | `ten_cents_with_the_pointer_away_from_the_notes` |
| 8, 27 | Glue lost one half's pitch edits | glue refuses halves whose edits differ | `glue_keeps_pitch_edits_or_refuses` |
| 9 | Demo test never checked the double | it does, and that it is its own render | `the_demo_project_opens_and_plays_its_edits` |
| 10 | Boot test raced others on `CANTUS_DATA_DIR` | its own binary, compares with its folder | `tests/pitch_boot.rs` |
| 11 | Fuzz never sent a valid edit to a non-audio clip | the refusal arm does; and a unit test | `a_pitch_edit_on_a_note_clip_is_refused`, the fuzz |
| 12 | Fixture held neither a moved note nor A/B | note 0 → G4, `bypass` on, a lane shown | `schema_27_keeps_a_takes_pitch_edits` (failed before regenerating) |
| 13 | Reversed clip: no bar lines, no bar numbers | tick range ordered in the ladder and the ruler | `a_reversed_slice_opens…` (bar numbers drawn) |
| 14 | Note panel overlapped the macros under ~815 pt | cards keep their size; narrower, the panel scrolls sideways (bar always shown) | `the_note_panel_fits_its_width` (1 600, 1 000, 640 pt) |
| 15 | Bar dropped Grid/Double/MIDI/Play silently | measured groups fold into a … menu that offers them; MIDI and Double also in the clip's menu | `a_narrow_bar_folds_into_its_more_menu`, `the_clip_menu_makes_midi_and_a_double`; shots `pitch-narrow-*` |
| 16 | Curves rebuilt and edits cloned every frame | built once per edit (`PitchView::cache`), bar by reference, blobs not copied | `an_idle_editor_builds_its_curves_once` |
| 17 | Inspector tips never shown; Edit pitch live with a missing file | tips on the help line; disabled when missing | `the_inspectors_pitch_button_explains_itself` |
| 18 | 'La Menor', solfège beside letters | the key in the notes' letters, the mode in the language's words ("A menor") | `the_key_reads_as_the_notes_do` |
| 19 | Spanish knob 'Tono' | 'Afinación', the macro knobs a size up so it fits | same test; shots |
| 20 | No device: renders on the UI thread per drag frame | `EngineHandle::compile` always queues on the worker | `without_a_device_renders_go_to_the_worker_and_export_waits` |
| 22 | Undo/redo played the undone edit's render | each clip's last three renders kept | `undo_and_redo_play_their_own_render_at_once` |
| 23 | Export before the frame's sync missed the latest edit | `compile_for_render` waits for pitch renders | same test as 20 |
| 24 | A failed analysis cached for the session | the sample cache's retry forgets it (and failed renders); **the idle worker was never woken by a new job** — found by this test, fixed | `a_failed_analysis_is_tried_again_when_the_file_is_back`, `the_sample_caches_retry_forgets_a_failed_analysis` |
| 25 | Correct pitch < 100 % overshot; 0 % moved an off-key note | `pitch = home + a·(target − home)`: never past, nothing at 0, re-runs are as if once | `correct_pitch_below_full_never_overshoots` |
| 26 | MIDI out gave every note of the take | windowed to the clip, loops repeated, edges cut | `midi_out_gives_the_notes_the_clip_plays` |
| 28 | Comp swipe drops a comp clip's pitch edits | **not mine** (`comping.rs`, leaders): left to the leaders builder as instructed | — |

**§3c, built (owner's no-gaps rule):**

- **The Pitch lane** (`views/pitch_lane.rs`): right-click an audio track ▸ *Pitch lane*; its
  clips' notes in a lane under it, dragged in pitch (the editor's snap) and centred by a
  double-click, with the editor's `SetClipPitch`. `Track::pitch_open` (schema 27),
  `Command::SetTrackPitchOpen` (fuzz arm), `TrackRow::pitch`. Test
  `the_pitch_lane_edits_notes_in_the_arrangement`; shot `pitch-arrangement-*`.
- **Quantise to another track**: *Quantise to* on the bar lists the grids and every other
  track with notes; each note's first edge onto the nearest onset within an eighth
  (`edit::quantise_to`, `quantise_by`). Tests `quantise_to_onsets_moves_notes_onto_another_tracks_notes`,
  `quantise_onto_another_tracks_notes`.

Files outside my area touched in review 1 (additive): `cantus-core` `model.rs`
(`Track::pitch_open`), `command.rs` (`SetTrackPitchOpen`); `cantus-project`
`midi/import.rs` (the new field); `cantus-engine` `backend.rs` (pitch renders always on the
worker, `compile_for_render` waits), `samples.rs` (`set_pitch_background`, `wait_pitch`,
retry forgets); `cantus-ui` `views/arrangement.rs` (`TrackRow::pitch`, the lane's paint,
header, input and menu hooks), `views/arrangement_auto.rs` and `views/arrangement_takes.rs`
(`pitch: 0.0` in their lane rows — the latter is leaders'), `audio_clips.rs` (glue),
`views/inspector_audio_parts.rs` (`button_with`).
