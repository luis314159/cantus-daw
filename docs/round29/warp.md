# Round 29 — warp: warping, time-stretch and pitch-shift of audio

Item 5 of round 29 (BACKLOG §3 *Audio editing*). Part A (the stretcher, the bench and the
listening set, the analysis, the model at schema 34, the editor and the inspector) and
part B (playback under the tempo map through `audio_time`, DAWproject `<Audio>`/`<Warps>`,
the tests). Decisions: ADR 0061. Branch `r29/warp`, merged with `r29/integration` at
`0933835` (tempo's 33, controllers' placeholder 35, isolation's 36 around my 34).

## Done

1. **The stretcher** (`cantus_dsp::stretch`, MPL-2.0, our own implementation from papers;
   licences reviewed in ADR 0061 §1 before any code — no GPL/LGPL code read for code, no
   crate added, `THIRD-PARTY-NOTICES.md` unchanged; ADR 0061 corrects
   `research/engine/dsp-quality.md`'s "Rubber Band is usable" note, which predates ADR 0047).
   One entry point, `stretch::render(channels, rate, &Map, &Params) -> Vec<Vec<f32>>`,
   offline and deterministic, exactly the requested length, and four modes:
   - **Complex**: a phase vocoder (170 ms frames) with Laroche–Dolson identity phase
     locking, the instantaneous frequency from two frames one synthesis hop apart,
     zero-phase frames; transposition by moving whole peak regions (the level held by a
     match to the source's power) with the formants kept by a true envelope
     (Röbel–Rodet) on a pooled 512-point grid; percussive energy split off by a
     Driedger HPSS soft mask (separation factor 2) and carried by short OLA grains;
     transients held at their own speed (`Map::hold`) with the risen regions reset on
     them; the right channel's phases locked to the left's image where their difference
     holds still, so the stereo image holds.
   - **Rhythm**: a slice at every transient and marker, each played as the source from
     where the map puts its transient (resampled by the transposition); the gap after it,
     when slowed, filled by its tail — *Fade*, *Loop* or *Ping-pong*.
   - **Melodic**: TD-PSOLA on the pitch base's epochs (ADR 0053), the Complex render
     cross-faded in where the line is unvoiced or more than one voice; formants kept by
     the grains.
   - **Re-pitch**: tape — read at the map's speed through Exemplar's windowed-sinc kernel
     on the mip level that does not alias; its pitch is its speed.
   - **Identity**: a map that is the identity, untransposed, returns the source to the bit.
2. **The bench and the listening set**: `cantus-bench --full --only warp` (eleven
   measurements; excluded from `--quick`, it takes ~40 s) and `cantus-bench listen <dir>`
   writing the A/B set to `F:\projects\daw-r29\shots\warp\listen\` with `README.txt`
   (five materials × four modes × ×0.5/×0.8/×1.25/×2, transpositions, the brief's
   ×1.25 −3 st case, each source beside them).
3. **The analysis at import**: SuperFlux onsets (within a millisecond, biased early); autocorrelation + comb
   tempo with an octave rule into 70–180 BPM; Ellis DP beats; the downbeat from low
   onsets; loop detection; the WAV `acid` chunk; on `WarpService` (the engine's worker,
   beside the pitch service, never on the interface thread), cached in memory and in
   `<data>/cache/warp/` (folder set in `CantusApp::assemble`, the boot path). **The
   import rule**: a loop is warped at once in the import's own undo step; long files and
   recordings play at natural speed; a paste keeps what it copied.
4. **The model, schema 34**: `AudioRegion::warp: Option<Box<Warp>>` (markers as source
   frame + content tick; mode, cents, formants, tail, follow, the detected tempo);
   `Command::SetClipWarp` with its exact inverse and `cmd.warp.*` in both languages;
   undo-fuzz arm 44 with its `warped` counter; `migrate_33_to_34` (writes nothing),
   the fixture block, `fixtures/v34.cantus`, `schema_34_keeps_the_warp`. Split, trim,
   slip, glue, loops, links, the clipboard and comping keep every marker valid; a warped
   window takes no comp seam.
5. **The interface**: the waveform drawn through the warp; transients along a band and
   markers as gold flags on the clip (double-click/drag the band, drag a marker — snapped
   through `views/bars.rs`, `Ctrl` free, `Alt` slides the audio under it — double-click,
   `Del` or the menu to remove); **Alt-drag of the right edge stretches the clip**; the
   clip editor's **Pitch | Warp** tabs (the warp editor with its bar); the inspector's
   **Warp** section; the clip menu's warp list (modes, *Follow the project tempo*,
   *Quantise audio…*, *Detect tempo*, *Warp from here*, *Set the project tempo from this
   clip*, *Edit warp*, *Bounce warped audio*, *Reset warp*, *Remove warp marker*); the
   *Quantise audio* dialog; *Bounce warped audio* on its own thread; `Glyph::Warp`;
   `shoot_warp` (PNGs in `F:\projects\daw-r29\shots\warp\`). On a warped clip the fade and
   gain squares sit below the marker band, so no flag is covered.
6. **Playback under the tempo map**: `audio_time::{frame_through, tick_through,
   clip_frame_at, clip_tick_at_frame, warped_length}` — one mapping for the engine and
   every interface site; each warped window rendered (`warp_render::Plan`) with frame 0
   on the window's start (the clip's `slip` = −offset seconds, so `Transport::clip_zero`
   places it unchanged); a key with the tempo map's `span_key`; the bridge (varispeed of
   the same map, or a WSOLA sketch at the clip's transposition) while a render is on its
   way; renders and bridges kept by recency within 512 MB; `compile_for_render` waits for pitch then warp renders. The identity
   warp under one tempo is dropped by the snapshot: such a clip plays the same bits as
   before this round.
7. **DAWproject**: audio clips written as `<Clip>` → `<Warps contentTimeUnit="seconds">`
   → `<Audio>` + `<File>` in the archive and `<Warp time contentTime>` points (algorithm
   complex/beats/tones/repitch, fades), XSD-valid; read back from any application (points
   on the natural line come back unwarped with their fine offset; the files written under
   the data folder); a transposition, pitch edits and a reversed clip reported.
8. **Finish**: ADR 0061; the manual's *Warping and stretching audio* (§5, after the pitch
   editor) in English and Spanish; this file.

## Measurements

`cargo run -p cantus-bench --release -- --full --only warp`, after the review (the full bench
report rows `warp.*`; the earlier figures are in ADR 0061's history).

After the review (§10); one core, stereo, 48 kHz.

| What | Target | Measured |
|---|---|---|
| Identity (every mode; also through the render path) | the source to the bit | to the bit (tests) |
| Output length | exact | exact (every mode × 0.25–4 × transpositions) |
| A hit's place through markers (every mode; the source's own rise subtracted) | ≤ 12 samples | 0 (bench); ≤ 12 every mode (test) |
| A marker's frame on its tick (engine; every mode; 44.1/48/96 kHz files at 44.1/48/96 kHz; under a ramp) | ±1 sample | ±1 |
| Attack (10–90 %), ×0.5 … ×2; under a tempo ramp | ≤ the source + 1 ms | the source's (92 frames median); 0 added under a ramp |
| Pre-echo before a hit (Complex): ×0.5 / 0.8 / 1.25 / 2; under a ramp | ≤ −30 dB | −31 / −35 / −36 / −35 dB median; −36 … −37 dB |
| Pre-echo before a lone drum hit, detected onsets (the source's own −30 dB) | the source's | −29.6 dB |
| A held tone's level before each hit (×0.8, ×1.25) | ≤ 1 dB | 0.03 dB |
| A transposed render's level (−12/−5/+7/+12 st, tone and dense noise) | ≤ 0.5 dB | 0.03 dB, 0.02 dB |
| Pitch of a shifted tone (−12 … +12 st, ×0.8 … ×1.25) | ≤ 3 cents | ≤ 0.002 cents |
| A held chord's voices, each, both channels: added level wander | ≤ 1 dB | 0.44 dB (×0.8), 0.52 dB (×1.25) |
| A voice +5 st, formants kept (envelope distance) | Complex ≤ 4 dB, Melodic ≤ 3 dB | 2.7 dB, 0.9 dB (not kept: 9.2, 9.2) |
| A delayed stereo pair through ×0.8 / ×1.5, +4 st | ±0.05 rad | within |
| Aliasing reading faster (Rhythm, grains, Re-pitch; up to +24 st) | ≤ −60 dB | below (test) |
| Tempo within ±1 BPM, metronomic (clicks, drums, swing; 70–180 BPM) | ≥ 97 % | 100 % (69/69); the 18 outside 70–180 all their octave inside it |
| Tempo within ±1 BPM, played (jitter, drift, a pad, a pick-up, 3/4, 6/8) | ≥ 90 % | 90.6 % (87/96) in the test; every miss an octave; 92.8 % in the bench's variant |
| CPU per second of audio (×1.25 −3 st) | — | Complex ≈ 0.11 s, Melodic ≈ 0.13 s, Re-pitch ≈ 5 ms, Rhythm ≈ 1 ms |
| CPU per second of a 2 s window of a 1-minute file (Complex) | the window's | ≈ 0.09 s |

No golden hash moved; `DSP_VERSION` unchanged (nothing plays differently unless a clip is
warped). The A/B listening set is in `F:\projects\daw-r29\shots\warp\listen\` (111 WAVs and
`README.txt`); the screenshots (arrangement, editor, inspector, quantise dialog, both
themes and languages, 75/100/200 %) in `F:\projects\daw-r29\shots\warp\`.

## Tests and what each holds

- **cantus-dsp** `stretch_tests.rs` (12): identity to the bit in every mode; exact length;
  a hit lands where its marker puts it; Rhythm's slices on their frame exactly; a tone's
  pitch and level through a stretch; Re-pitch moves pitch with speed; transposition by the
  semitones; a voice's formants kept; attacks sharp with no pre-echo; the stereo image
  kept; determinism; the map inverts and holds keep its breakpoints. `onset_tests.rs` (5):
  every click on its sample; a swung drum loop hit by hit; a vibrato is not a note;
  silence and one hit; determinism. `beat_tests.rs` (5): a click track's tempo and
  beats; a swung loop's tempo and downbeat; whole-bar loops told and made exact; silence;
  the corpus within 1 BPM.
- **cantus-core** `warp_tests.rs` (10): the map through the markers and its inverse; no
  markers = the source's tempo; identity told and nothing else; invalid warps refused;
  every edit keeps the warp valid; quantise by strength; a grid warps a loop bar by bar;
  one function for a clip's time; a clip never warped writes nothing new; a stretched
  warp plays its window longer. `audio_time` unit tests (one tempo = the old rule to the
  bit; natural speed from its start). `tests/warp.rs` (5): set and undone exactly;
  refusals; a marker drag is one undo step; the snapshot drops a warp that plays as the
  source; a warped window takes no comp seam. Undo fuzz arm 44.
- **cantus-project**: `schema_34_keeps_the_warp`, every fixture loads;
  `tests/dawproject_audio.rs` (2): written XSD-valid with the files carried; read back
  with markers on their frames, the mode, fades, natural-speed clips unwarped with their
  fine offset, nothing reported dropped.
- **cantus-engine** `tests/warp.rs` (9): the analysis finds a loop's tempo and hits and
  keeps them on disk; the `acid` chunk; an identity warp plays the source to the bit; a
  92 BPM loop plays in time at 128 BPM; a marker on its tick to the sample; offline =
  real time at block sizes 1/37/64/512 and mixed; while a render is on its way the last
  render plays; an export waits for the warp renders; the render and the interface read
  one mapping (to the bit, under a ramp).
- **cantus-ui** `tests/warp.rs` (7): a loop imported warps itself in the import's undo
  step; a file that is not a loop stays at natural speed; a marker added, moved and
  deleted by `Del` (not the clip); quantise pulls a loose take onto the grid; the
  arrangement draws markers to grab, with the fade and gain squares below the band;
  bounce leaves the clip on a file of its own; Alt-drag of the right edge stretches.
  `tests/warp_boot.rs`: the boot path keeps analyses in the data folder. `fx_lint.rs`,
  `kit_lint.rs`, `i18n.rs` cover the new views and strings.

## Files outside my area (additive)

- `cantus-core`: `audio.rs` (the `warp` field), `command.rs` (one `SetClipWarp` block),
  `snapshot.rs` (the warp, the source rate, the identity dropped), `comp.rs` (a warped
  window takes no seam), `lib.rs` (`mod warp`), `audio_time.rs` (tempo's file, extended
  for warped clips as the brief says: `frame_through`, `tick_through`, `clip_frame_at`,
  `clip_tick_at_frame`, `warped_length`), `tests/undo_invariants.rs` (arm 44, `warped`).
- `cantus-dsp/src/lib.rs` (`mod beat, onset, stretch`).
- `cantus-engine`: `lib.rs` (`mod warp`), `samples.rs` (the warp attach after the pitch
  attach, `loading()` counting both), `pitch_attach.rs` (`render_of`), `backend.rs`
  (`compile_for_render` waits for the warp renders after the pitch renders).
- `cantus-project`: `format.rs` (`migrate_33_to_34`, the test), `examples/write_fixture.rs`
  (the 34 block), `fixtures/v34.cantus` and the rewritten `v36.cantus`,
  `dawproject/{mod, read, read_timeline, write}.rs` (audio clips; `audio.rs` new),
  `report.rs` (the new keys, scanning `audio.rs`).
- `cantus-ui`: `app.rs` (the `warp` state, the service poll, the import hook, the
  pre-hooks), `lib.rs` (`mod warp…`), `views/mod.rs`, `views/arrangement.rs` (the warp
  handles routed in begin/update/reset/hint/click/menu), `views/arrangement_audio.rs`
  and `_edit.rs` (the waveform through the warp, the markers, the squares below the band,
  Alt-drag of the right edge), `views/inspector.rs` (the section, `Act::Warp`),
  `views/piano_roll.rs` (the Warp tab opens the warp editor), `views/pitch.rs` (the tabs
  line), `pitch.rs` (`ClipTime` through `audio_time` and the warp), `pitch_demo.rs` (a
  path), `audio_files.rs` and `audio_clips.rs` (the import rule, glue only with the same
  warp), `clips.rs` (the undo label), `kit/glyph.rs` (`Glyph::Warp`), `views/interop.rs`
  (`export_with`, the imported media placed), `locales/areas/interop.{en,es}.txt`
  (`d_audio_clips` no longer says audio clips are dropped), `tests/fx_lint.rs` (the new
  views scanned).
- `cantus-bench` (isolation's crate for its suite): `src/warp.rs` new, `lib.rs` (the
  `warp` group, kept out of `--quick`), `main.rs` (`listen`), `Cargo.toml` (`hound`,
  already in the workspace: no new notice); `Cargo.lock`.
- No keymap action was added (`Del` reuses `Action::Delete`; Alt-drag is a modifier), so
  nothing was appended to `NEW_IN_V9`.

- **Review 1's fixes** (all additive or local to warp paths): `cantus-core/src/clipboard.rs`
  (`paste_clips`: the tempo scale skipped for a following warp — folded into tempo's
  relocation block, not duplicated), `command.rs` (`validate_clip` and `SetClipAudio`
  refuse an invalid warp), `comp.rs` (`seam_window` takes a natural warp),
  `snapshot.rs` (a natural warp dropped under any map), `tests/undo_invariants.rs` (refused
  ways in, every warp checked valid after every command, marker adds counted only when
  real); `cantus-project/src/format.rs` and `store.rs` (`warps_repaired` on load),
  `report.rs` (new keys; `mod.rs` scanned), `dawproject/{mod, read, read_timeline, write,
  zip}.rs` (lanes, content types, natural audio, ZIP64, streaming, `import_from`);
  `cantus-engine/src/backend.rs` (`compile_for_render` loops, `renders_late`),
  `tests/pitch_editor.rs` (one test appended); `cantus-ui/src/app.rs` and `boot.rs` (the
  load-repair status), `views/arrangement.rs` (the right-clicked tick for *Warp from
  here*; an empty click lets a marker go), `views/arrangement_audio.rs` (the band offered
  only where it may be taken), `audio_files.rs` (`import_audio_as`),
  `recovery_session.rs` (recovered takes as recordings), `pitch.rs` (`pitch_editing` off
  in the Warp tab), `views/modulator_bake.rs` (waits while loading), `views/interop.rs`
  (export on a worker, `import_dawproject_in`, unwritten media reported),
  `kit/widgets.rs` (`led_button_if`, additive).
- **New test files**: `cantus-dsp/src/stretch_review_tests.rs`,
  `cantus-engine/tests/warp_review.rs`, `cantus-ui/tests/warp_review.rs`,
  `cantus-ui/tests/warp_bounce.rs` (its own binary: it points the data folder elsewhere;
  the old bounce test moved here from `tests/warp.rs`).


## Review 1 (`reviews/warp-1.json`): 76 findings, all fixed

Each with a test seen failing first (on the code before the fix, or — where the fix adds
the API the test needs — with the fix undone in place). Tests named by file: `dsp/rv` is
`cantus-dsp/src/stretch_review_tests.rs`, `eng/rv` is `cantus-engine/tests/warp_review.rs`,
`ui/rv` is `cantus-ui/tests/warp_review.rs`, `ui/bounce` is `cantus-ui/tests/warp_bounce.rs`,
`core/warp` is `cantus-core/tests/warp.rs`, `proj/audio` is
`cantus-project/tests/dawproject_audio.rs`.

| # | Sev. | Finding | Fix | Test |
|---|---|---|---|---|
| 1 | H | A pasted warped clip rescaled as if at natural speed | `paste_clips`: a following warp skips the tempo scale (and tempo's `relocate` leaves it) | core/warp `a_pasted_warped_clip_keeps_its_ticks_at_any_tempo` |
| 2 | H | The first warp edit started from a straight line at the first tempo, following | `Warp::natural_of` (markers on the unwarped rule through the map); `warp_to_edit` starts not following (transpose, formants, tail, mode), `warp_to_warp` following (markers, stretch, Quantise, tempo, Detect, From here, Follow) | warp_tests `the_natural_warp_plays_as_the_clip_plays_unwarped`; ui/rv `a_first_warp_edit_leaves_what_plays_alone` |
| 3 | H | Detect tempo slid a trimmed window's audio | `commit_warp_keeping`: a `SetClipOffset` keeps the window's first frame (Detect, From here, the clip's tempo) | ui/rv `detect_tempo_keeps_a_trimmed_window_on_its_audio` |
| 4 | M | Loop length in the project's metre, analysis in 4/4 | the length is four beats a bar | ui/rv `a_loop_in_three_four_keeps_its_four_four_bars` |
| 5 | M | Only the first loop of an import joined its undo step | the step is closed when the import's last file settles | ui/rv `a_many_file_import_is_one_undo_step` |
| 6 | M | A reversed clip's band placed markers mirrored wrong | `Hits` mirrors its ticks as the markers are drawn | arrangement_warp `a_reversed_clips_band_is_read_mirrored` |
| 7 | M | Invalid warps entered through other commands and files | `validate_clip` and `SetClipAudio` refuse them; `warp::repair` on load, said | core/warp `an_invalid_warp_is_refused_on_every_way_in`; format `a_file_with_an_invalid_warp_opens_without_it`; fuzz checks every warp valid |
| 8 | M | An Alt-stretch (and a bounce) of a linked copy changed its siblings' content | the copy is made unique in the same step | ui/rv `a_linked_copy_stretches_on_its_own` |
| 9 | M | Quantise changed the speed after the last pulled hit | the range's ends (and a guard beyond) pinned first | warp_tests `quantise_leaves_the_audio_beyond_the_range_alone` |
| 10 | M | Recovered takes went through the import rule | `import_audio_as(.., recorded)`; `auto_warp` refuses a recorded source | ui/rv `a_recovered_take_is_never_warped_by_itself` |
| 11 | L | Any warp removed a comp seam's crossfade | `Warp::natural` (not following, not transposing) keeps the seam | core/warp `a_warp_that_plays_the_source_keeps_the_comp_seam` |
| 12 | L | Only the first 64 handles could be grabbed | the 64 nearest the pointer | arrangement_warp `the_handles_offered_are_the_nearest` |
| 13 | L | *Set the project tempo* set only tick 0's | `set_tempo_at` the clip's start | ui/rv `the_project_tempo_is_set_where_the_clip_is` |
| 14 | H | Rhythm cut a slice at every ramp sample (a buzz) | soft map points (tempo samples) never cut | dsp/rv `rhythm_rides_through_a_tempo_ramp_without_chopping` |
| 15 | H | Complex's holds shrank under any tempo change | holds bounded by hard points only; soft points beside them give way | dsp/rv `holds_keep_their_size_under_a_tempo_ramp`, `attacks_stay_sharp_under_a_tempo_ramp` |
| 16 | M | Transposition changed the level (+1.8 / −4 dB) | whole peak regions moved (true frequency), magnitudes read by windowed sinc, a level match against the source's power | dsp/rv `a_transposed_render_keeps_its_level` (±0.5 dB, −24…+24 st, tone and noise) |
| 17 | M | Every bin reset at a transient (sustained partials dipped) | only the regions whose peak rose are reset | dsp/rv `a_held_tone_does_not_dip_under_hits` |
| 18 | M | Re-pitch lost its top octave just over speed 1 | a band-limited reader: mip level + a cut-off scaled by the local speed | dsp/rv `repitch_just_over_speed_one_keeps_its_top_octave` |
| 19 | M | Rhythm and the grains aliased when transposed up | the same reader above speed 1 | dsp/rv `reading_faster_folds_nothing_back` |
| 20 | M | Every render analysed the whole file | Complex/Melodic render `stretch::reach`'s span (hop-aligned, with context); `Request::origin` | dsp/rv `a_window_of_a_long_file_renders_as_the_whole_file_would` |
| 21 | L | DC locked to the first peak (a slow wobble) | DC, its lobe and Nyquist keep their analysed phase | dsp/rv `a_dc_offset_stays_constant_through_a_stretch` |
| 22 | L | The bench's metrics could not fail | detected onsets on drums and a mix, a held tone under hits, every metric under a ramp, level after transposition, each voice in both channels, a window of a long file, the played corpus | the bench rows below |
| 23 | L | A short hold missed its reset frame | a hold is passed only once its nearest frame is behind | dsp/rv `a_short_hold_still_gets_its_reset_frame` |
| 24 | H | A loop's pieces cancelled each other's renders | jobs are dropped only when nothing plays their key | eng/rv `every_piece_of_looped_warped_clips_plays_its_own_render` |
| 25 | H | Renders evicted whenever a clip left the snapshot | kept by recency within 512 MB, whatever the snapshot | eng/rv `a_render_is_kept_while_its_clip_is_out_of_the_snapshot` |
| 26 | M | A drag never played a bridge | each piece plays its newest render or bridge, kept even for a superseded key | eng/rv `a_drag_plays_the_newest_bridge` |
| 27 | M | Renders and bridges copied the whole file | only the span read is taken from the input; the pitch analysis kept per input | eng/rv (cost: the bench's window row); dsp/rv window test |
| 28 | M | Bouncing an identity warp wrote the whole file | the bounce always renders the window | ui/bounce `a_trimmed_window_bounces_to_its_own_audio` |
| 29 | L | Exports waited for renders of deleted clips | a key nothing plays is not pending; `renders_late` counted and logged | eng/rv `a_deleted_clips_render_is_not_waited_for` |
| 30 | L | Tempo points were not render-map breakpoints | every tempo point inside the window is one | eng/rv `a_tempo_step_inside_a_window_is_on_the_render_map` |
| 31 | L | A natural warp under a ramp was rendered | the snapshot drops a natural warp under any tempo | core/warp `a_warp_that_does_not_follow_is_no_warp_under_any_tempo`; eng/rv `a_natural_warp_under_a_ramp_plays_the_unwarped_bits` |
| 32 | L | Replaced bridges bypassed the graveyard | every replaced render or bridge goes to the graveyard | (by construction: `put_done`, `put_bridge`) |
| 33 | L | The modulator bake blocked the frame while renders were made | it waits beside the window while loading, then runs | ui/rv `a_bake_waits_for_renders_on_their_way` |
| 34 | H | The bridge dropped the transposition | a transposed bridge is a WSOLA sketch at the transposition, anchored on markers and transients | eng/rv `the_bridge_plays_at_the_right_place_and_pitch_until_the_render_comes` |
| 35 | M | Loop pieces shared one history | history per piece (clip, start) | as 24 |
| 36 | L | Same as 32 | as 32 | as 32 |
| 37 | L | A bounce decoded and rendered the whole project | the snapshot is cut to the one clip | ui/bounce (all three) |
| 38 | H | DAWproject wrote take lanes as playing clips | only lane 0, the takes reported | proj/audio `take_lanes_are_left_out_and_said` |
| 39 | H | Audio deflated, all in RAM, no ZIP64 | stored, read once each, streamed to the file on a worker; ZIP64 both ways | zip `zip64_records_are_written_and_read_back`; proj/audio `audio_is_stored_once_and_streams_to_a_file` |
| 40 | M | Natural clips written as tempo-following warps | bare `<Audio>` in seconds; warps kept under a tempo map | proj/audio `audio_clips_keep_what_they_play_under_a_tempo_map` |
| 41 | M | As 2 | as 2 | as 2 |
| 42 | M | A stale selected marker took Del | Del goes to a marker only while its clip is what is edited; an empty click lets go | ui/rv `a_stale_marker_selection_does_not_take_del` |
| 43 | M | The pitch editor's keys acted in the Warp tab | `pitch_editing()` is off there; Q opens *Quantise audio* | ui/rv `the_warp_tab_takes_the_pitch_editors_keys_away` |
| 44 | M | Audio tracks written as notes | `contentType` audio (or both); the input said, and given back on import | proj/audio `audio_clips_are_written_valid_with_their_files`, `..._come_back_with_their_warps` |
| 45 | L | A re-import wrote over the first one's audio | a folder of its own per import; failed writes reported | ui/bounce `imported_media_never_writes_over_an_earlier_import` |
| 46 | L | `external` ignored; wrong relative folder | `import_from(folder)` resolves against the archive's folder, missing files said; `./` alike; absolute paths written | proj/audio `an_external_file_is_found_next_to_the_archive` |
| 47 | L | Gain, fade shapes, tail, formants, markers dropped silently | each reported | (report keys registry test) |
| 48 | L | As 4 | as 4 | as 4 |
| 49 | L | As 10; the take-analysis hook unused | recordings are analysed by `analyse_file_soon` as they land; the unused hook removed | as 10 |
| 50 | H | Only Rhythm tested to the sample | every mode, five rate pairs, with and without a ramp, the source's own place subtracted; dsp marker test over every mode, 12 samples for all | eng/rv `every_mode_puts_a_marker_on_its_tick`; stretch_tests `a_hit_lands_where_its_marker_puts_it` |
| 51 | M | The identity test never reached the render path | a tempo change elsewhere keeps the warp; still the bits | eng/rv `an_identity_warp_through_the_render_path_is_the_source_to_the_bit` |
| 52 | M | As 31 | as 31 | as 31 |
| 53 | M | The tempo corpus was metronomic and asserted 69 of 87 | a played corpus (jitter, drift, pad, pick-up, 3/4, 6/8); every file counted, every miss asserted to be an octave | beat_tests `the_corpus_is_within_one_bpm` |
| 54 | M | Offline = real time could pass on silence | the render is asserted present, sounding, and not the source | engine warp `offline_equals_real_time_at_any_block_size` |
| 55 | M | The on-its-way test was a tautology | asserted exactly; the bridge tested on its own | engine warp `while_a_render_is_on_its_way…`; eng/rv bridge tests |
| 56 | M | No test of split, loops, paste, reverse, pitch→warp; fuzz never checked validity | tests for each; the fuzz checks every warp after every command and counts only real marker adds | core/warp `a_split_warped_clip_plays_what_the_whole_did`; pitch_editor `a_pitch_edit_and_a_warp_chain_into_the_export`; fuzz |
| 57 | M | As 10 | as 10 | as 10 |
| 58 | L | No parallel = serial | added | eng/rv `parallel_equals_serial_with_warped_clips` |
| 59 | L | Loop and Ping-pong tails untested | added | dsp/rv `rhythm_tails_loop_cleanly_and_stay_in_their_slice` |
| 60 | L | Rhythm untested on detected onsets | added; the detector held to −48…+24 frames (early, never late enough to cut an attack); "to the sample" corrected in the docs | dsp/rv `rhythm_on_detected_onsets_keeps_the_attacks` |
| 61 | L | The bounce test never checked what was written | the file equals what played (transposed, trimmed, looped); its own binary | ui/bounce |
| 62 | H | The band of any clip took a press | only a warped clip's (and the editor's) | ui/rv `the_band_of_an_unwarped_clip_takes_no_press`; arrangement_warp `a_band_not_offered_takes_no_press` |
| 63 | M | Markers past 64 ungrabbable; no zoom | nearest 64; the editor zooms (Ctrl+wheel) and scrolls (wheel, a bar) | as 12 |
| 64 | M | As 42 | as 42 | as 42 |
| 65 | M | *Warp from here* used the playhead | the menu uses where it was right-clicked; outside the clip it is said | ui/rv `warp_from_here_outside_the_clip_is_said` |
| 66 | M | Quantise reached one clip | the menu and the inspector pass the selection | ui/rv `quantise_audio_over_several_clips_is_one_step` |
| 67 | M | The bar drew dead controls live | Formants, Transpose, Reset drawn disabled as the inspector does | shots |
| 68 | M | The inspector's tempo clamped to 40–240 | 20–999 (log), reset to the detected tempo | (by construction; shots) |
| 69 | L | Quantise said "0 clips" over "analysing" | said only when something moved; the dialog stays open while analysing | (by construction) |
| 70 | L | "1 bars", "(s)" | singular keys, `tf_count` | i18n test |
| 71 | L | An unlabelled "100 %" | its own row: *Sure* | shots |
| 72 | L | Slice ends cut off; two Spanish words for a fade | Fade/Loop/Mirror, Fundir/Bucle/Espejo | shots (EN k200, ES k100) |
| 73 | L | *usted* in Spanish | *tú* | — |
| 74 | L | Editor drags snapped to the arrangement's grid | the editor's own grid chip (1/4…1/32, off) | shots |
| 75 | L | Menu and inspector disagreed (Follow, Bounce) | Follow on any audio clip; one bounce test, `can_bounce` | — |
| 76 | L | Modes split over two labelled rows | one label, the second row its continuation | shots |

## Left

Nothing of mine. Owner-only: **listen** to `F:\projects\daw-r29\shots\warp\listen\`
(`README.txt` says what each file is; written again after the review, the vocoder now at
170 ms) and say which mode or ratio sounds worse than élastique — the numbers above are
met, the ear is the owner's.

## Notes for the integrator

- **Known-issue lines this work retires** (the integrator edits them; tempo retires the
  rest of the same line): the manual's §14 "**No time-stretch**: audio clips keep their
  speed when the tempo changes" (the *one tempo and one time signature* half is tempo's)
  and "warping" in *Not in this beta*, in English and Spanish; the release notes
  `docs/releases/v0.1.0-beta.1.md` (the same two lines in each language). Search for
  "No time-stretch", "Sin estiramiento de tiempo" and "warping": line numbers moved with
  the merge.
- **Schema**: mine is 34; the chain on this branch is 33 tempo, 34 warp, 35 controllers,
  36 isolation, 37 freeze.
- **Merge hot spots**: `clipboard.rs::paste_clips` (my guard sits in tempo's new block:
  `scale.filter(|_| !follows)`; tempo's `relocate` already leaves a following warp),
  `views/arrangement.rs`, `app.rs`, `samples.rs` / `backend.rs`, `views/interop.rs`, the
  undo fuzz (arm 44's refusals now draw from the shared generator).
- `crates/cantus-engine/tests/inst_stream.rs::the_suite_leaves_no_cache_folder_behind`
  failed once in the full engine run here (its child run under load) and passed alone
  twice and in the fuzz run: not warp's, but worth a look if it recurs.
- `research/engine/dsp-quality.md` §2 and `research/engine/README.md` said Rubber Band
  was "usable since Cantus is GPL" (written before ADR 0047); both now point at ADR 0061 §1.
- On a warped clip the fade and gain squares sit below the marker band, so no flag is
  covered.
