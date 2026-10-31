# Round 34 — transcribe: audio to MIDI

Builder **transcribe**, branch `r34/transcribe` (from `3af7ff92`, merged with
`r34/integration` at `8ec247b9` for stems' ML service, then with `r34/integration` at
`f07d79d5` and `r34/stems` at `35b994ba` and `13e609ef` for the second delivery). ADR 0086 holds the decisions and
every number; this is the summary, how it was tested, and the marked lines.

## What is done

1. **Polyphony — Basic Pitch** (Spotify, Apache-2.0): its `nmp.onnx` downloaded on demand
   from Spotify's repository at a pinned commit, hash checked, run as it is through stems'
   runtime (a new `prepare` kind `onnx`); `basic_pitch`'s windowing, joining and **note
   creation re-implemented in Rust** (`cantus-ml/src/transcribe*.rs`), decision for
   decision, NumPy's arithmetic where it decides a result. Bends kept or dropped by choice,
   kept as per-note bend curves (ADR 0080).
2. **Melody — Cantus's own pitch tracker** (the pitch editor's): measured better than
   Basic Pitch's notes on every monophonic set (0.805 vs 0.718 rendered, 0.769 vs 0.658
   TRIOS), so melody mode is it — in process, no download, its pitch curve as the bend.
3. **Drums — our own detector** (`cantus-dsp/src/onset_drums.rs`, MPL, semi-adaptive NMF
   with templates drawn from measured spectra, picked per group, named, leaks dropped)
   **and, optionally, Magenta's Onsets and Frames Drums** (Apache-2.0, a third party's
   ONNX conversion pinned on Hugging Face, 6 MB) for every class — its hi-hat output
   pointed at the wrong pitch in the conversion and is re-pointed at install (below); our
   detector says which hats were open. Mapped to GM keys on Pulsus II's studio kit, every
   key one the kit has. The dialog lets the user pick the classes; toms and cymbals
   are off by default without the model, and it says so.
4. **Convert to MIDI…** on an audio clip's menu: mode, sensitivity, shortest note, bends,
   drum classes, instrument (Unda or Exemplar; Pulsus for drums), quantise (off), the
   model's licence, size and state before it downloads, progress and *Stop* — into a new
   track under the clip with a MIDI clip aligned to it, through the clip's own time
   mapping (tempo, warp, slip, reverse, loops): **one undo step** (`cmd.transcribe`).
5. **`audio_to_midi`** for agents: class Edit, a job ending in `JobDone::Then`; its words
   name the clip and track ("Convert “Vox.wav” on “Lead” to MIDI (melody) on a new
   track"); a missing model is refused, never downloaded.
6. **The F-measures** (mir_eval's rules, our own `cantus_dsp::onset::eval`): notes on
   material rendered from known MIDI through seven of our instruments and on TRIOS; drums
   per class on MDB Drums (tuning tracks and held-out tracks apart) and on Pulsus's kits;
   Cantus's Basic Pitch against Spotify's.
7. ADR 0086, the manual (`docs/manual/{en,es}/audio-to-midi.md`), strings in
   `transcribe.{en,es}.txt`, screenshots in `docs/round34/transcribe/`.

## Numbers

| What | Result |
|---|---|
| Basic Pitch in Cantus vs Spotify's `basic_pitch` 0.4.0, 19 TRIOS files | posteriorgrams bit-identical; **3 286 of 3 286 notes identical**; 47–73× real time |
| Note creation on the CI fixture, 6 settings | every note equal to Basic Pitch's (frames, pitch, mean to the bit, bends) |
| Notes, rendered through our instruments (1 225 notes) | polyphonic Fno 0.609 (F 0.399); melodies: own tracker 0.805, Basic Pitch 0.746 |
| Notes, TRIOS (3 068 notes) | polyphonic Fno 0.703 (F 0.333); solo stems: own tracker 0.769, Basic Pitch 0.699 |
| Drums, MDB held out, detector alone | kick 0.961 · snare 0.849 · toms 0.311 · hi-hat 0.647 · cymbals 0.115 (pooled 0.691) |
| Drums, MDB held out, with the drum model | kick 0.963 · snare 0.885 · toms 0.442 · **hi-hat 0.738** · cymbals 0.640 (pooled 0.811) |
| Drums, Pulsus's 7 kits, detector (CI, floors held) | kick 0.986 · snare 0.833 · toms 0.286 · hi-hat 0.847 · cymbals 0.250 |
| Drums, Pulsus's 7 kits, with the drum model | kick 0.918 · snare 0.972 · toms 0.620 · hi-hat 0.824 · cymbals 0.102 (pooled 0.774) |
| Drums, MDB held-out full mixes: as they are / drums separated first (with the model) | pooled 0.611 / **0.795** |
| An hour of take (peak working set) | the interface's reader 15 MB (was 0.97–1.27 GB); the helper 1.49 GB (notes and chords), 1.61 GB private (drums with the model); the hour converts in 1.4 min (notes) and 3 min (drums) in a debug build |
| Published, MDB 5 classes pooled (ADTOF, NC-licensed) | 0.87–0.89 |

Toms (0.44) stay under 0.6 on the held-out stems even with the model: the held-out
tracks have few toms and the model fires on low kicks. Hi-hats went from 0.647 to **0.738**
with the model's own hats (below). Cymbals reach 0.64 only with the model. On full mixes
everything falls (pooled 0.611); *Separate the drums first* (HT-Demucs, when installed)
brings them back to 0.795. Organum (tonewheel organ) does not transcribe (0.06–0.21).

## The review's findings (second delivery)

Each fixed with a test that fails without it (the test in parentheses).

1. **Melody ignored Sensitivity and Shortest note.** The shortest note now filters the
   tracker's notes (`melody_keeps_only_notes_as_long_as_the_shortest_asked`); sensitivity
   is the models' and is hidden in melody mode (the dialog and the agent tool's schema say
   so).
2. **The drum model's dead hi-hat.** Magenta's 8-hit map folds every hat into pitch 46;
   the conversion's output `Gather` took 42. Exporting all 88 outputs proved 46 carries the
   hats; the content manager now patches that one index in a copy at install (`prepare`
   kind `onnx` with a `patch`: exact bytes, found once, the copy's sha256 checked), the
   card, ADR 0086 and `transcribe_drums.rs` say so, and what is not verified: no comparison
   with Magenta's TensorFlow inference (the converter's scripts answer 404). Hats with the
   model, MDB held out: **0.738** (the detector's were 0.647; tuning stems 0.826, Pulsus's
   kits 0.824). (`the_drum_models_patch_points_the_hat_at_pitch_46`,
   `a_patched_onnx_is_edited_in_a_copy_and_its_hash_held`.)
3. **A reversed loop transcribed the wrong part** and 4. **loop repeats were shifted by
   ticks.** The clip is cut into `Snapshot::compile`'s pieces, each with its own
   `ClipTime` (reversed in its own window, the last piece short), the frames they cover
   read, and each piece places what its frames hold through its own mapping
   (`transcribe_place`: tempo change, short last piece, reverse + loop, offset + slip,
   warp — ticks worked out from the engine's rules, not the code).
5. **Ride bell/cowbell on key 53, no pad.** They play the ride (51); `KEYS` lists every key
   written (`every_key_the_network_names_is_one_the_kit_has`, `…_detector_…`,
   `every_drum_key_a_conversion_writes_has_a_pad_in_its_kit`).
6. **Exemplar made a silent track.** It loads `FACTORY_PRESETS[0]`
   (`exemplar_gets_a_sound_not_an_empty_sampler`).
7. **Spotify's NOTICE.** A hand-written section in `xtask/notices/head.md`, "Basic Pitch's
   note creation, ported to Rust", with the NOTICE text; THIRD-PARTY-NOTICES.md
   regenerated (`spotifys_notice_is_in_the_notices`).
8. **Stop while reading left input.wav.** The reader takes a cancel flag (asked between
   blocks, a `.part` file renamed only when whole); a conversion's folder is held by a
   `held.lock` file lock and removed with its last holder; folders whose lock nobody holds
   are swept at the next start (`transcribe_workdir`).
9. **Long files.** Only the clip's frames are read, a block at a time, mixed to mono
   without per-channel buffers, converted by a block-wise resampler bit-identical to the
   whole-file one (`blocks_convert_to_the_bit_as_the_whole`, seven rate pairs) and written
   as they come; Basic Pitch's windows are joined as they come and let go; the cancel
   reaches the NMF, asked each iteration (`a_cancel_stops_the_factorisation`). Measuring an
   hour (`transcribe_long`, ignored) found a real bug: the detector ran silent for longer
   than the client waits and the job failed as **Hung**; each factorisation step now says
   it is alive (`a_long_drum_take_keeps_the_helper_alive`, which fails without it). The
   hour's peaks are in the table above.
10. **The job table leaked.** The router updates only jobs it still has (`get_mut`); every
    starter forgets its job when it ends (the dialog on every outcome and stop, the agent
    tool's `follow`, separation first); and the table keeps at most `KEPT_ENDED` (32) ended
    jobs, which bounds stems' separation too (`ml_service_jobs`,
    `the_agents_drum_job_leaves_nothing_in_the_job_table`).
11. **Bends of cut or reversed notes.** A bend is rescaled to the kept span, mirrored when
    reversed; drum hits are anchored at their onset
    (`a_bend_keeps_the_part_of_the_note_that_is_kept` and the placement tests).

After merging stems' review 2 (`13e609ef`): a conversion stops when another project opens
and its dialog closes, nothing made
(`another_project_open_stops_the_conversion_and_makes_nothing`); its workers wake the
window; it counts as the helper in use for model removal; the ONNX models' hashes come from
the build's manifest (`map_of`), and *prepared* is marker, version and size.

## The integrated review (r34/integration 381020aa)

1. **An agent's `audio_to_midi` held no in-use counter**: between its helper jobs *Remove*
   was accepted. It now holds stems' agent counter (`stems_ui::run::Held` on
   `app.stems.agents`, the name stems is renaming to a shared one) from `prepare` until
   the job ends (`remove_is_refused_while_an_agents_conversion_reads_its_take`).
2. **A session clip was accepted** (`Track::clip` finds them too): both the dialog's path
   (`transcribe_ui::source`, string `transcribe.err.session` in both languages) and the
   tool look in the arrangement's clips only and refuse a session clip saying why; the
   tool's description says so (`a_session_clip_is_refused_with_words_that_say_why`).
3. **The drum kit kept English pad names on a Spanish screen**: `instrument` names the
   studio kit's pads through `app::localized_pad_name`
   (`a_drum_conversions_kit_speaks_the_interfaces_language`).

## How it was tested

**Second delivery** (the review's fixes, both stems merges): **23 new tests** and one more
ignored measurement — cantus-ml 4 (`transcribe_keys` 2, `transcribe_notice` 1,
`transcribe_helper`'s long drum take), cantus-content 2 (`models_onnx`), cantus-dsp 1 (the
factorisation's cancel), cantus-ui 16 (`transcribe_place` 7, `transcribe_workdir` 4,
`ml_service_jobs` 2, `transcribe_read` 1, `transcribe_convert` 2); ignored:
`transcribe_long` (an hour). On Windows with `-D warnings`: cantus-ml 42, cantus-content 93,
cantus-project 156, cantus-dsp's onset 25, cantus-engine's `transcribe_drums` 1, cantus-ui's
`transcribe_*`, `ml_service_*`, `stems_feature`, `stems_review2` and `ai_mcp_pump` 50 — all
pass; cantus-ui's library 389 of 390: `views::transport::layout::tests::short_groups_share_a_column_two_high`
fails since the merge of `r34/integration` (mcp's top-bar agent chip adds a group: 11
columns, the test expects 10) — not this branch's file, left to its owner. Workspace
clippy `--all-targets` clean. Linux (WSL Ubuntu 24.04, `tools/wsl/check.sh` in its own
clone, headless): workspace clippy and the plug-ins without their editor clean; cantus-ml
and cantus-content, cantus-project, cantus-dsp's onset tests, cantus-engine's
`transcribe_drums`, and the same cantus-ui test binaries (50) all pass; cantus-ui's
library 398 of 399, the same transport-layout test failing; the clone's target removed.
The first Linux run caught `the_agents_drum_job_leaves_nothing_in_the_job_table` counting
the dialog tests' folders running beside it in the shared cache; it now counts only the
agents' folders and waits for them.

First delivery:

See ADR 0086 "What the tests hold". **35 new tests** pass on Windows (`-D warnings`):
cantus-ml 10 (`transcribe_reference` 4, `transcribe_helper` 6), cantus-dsp 13 (the measures
4, the detector 9), cantus-engine 1 (Pulsus's kits, floors per class), cantus-content 3
(`models_onnx`), cantus-ui 8 (`transcribe_convert`); and the crates' existing tests still
pass (cantus-ml, cantus-content, cantus-dsp onset, cantus-ui i18n). Workspace clippy with
`--all-targets` is clean. Linux (WSL Ubuntu 24.04, `tools/wsl/check.sh` in its own clone,
headless): workspace clippy clean (and the plug-ins without their editor); cantus-ml,
cantus-content, cantus-dsp's onset tests, cantus-engine's `transcribe_drums`, cantus-ui's
`transcribe_convert`, `ai_mcp_pump`, `ml_service_boot` and its library tests all pass; the
clone's target removed. **7 ignored** measurements, with what they need:

```text
CANTUS_ORT_LIBRARY=…/onnxruntime.dll CANTUS_BASIC_PITCH=…/nmp.onnx CANTUS_BP_REFERENCE=…/bpref \
  cargo test -p cantus-ml --release --test transcribe_real -- --ignored --nocapture
CANTUS_ORT_LIBRARY=… CANTUS_OAF_DRUMS=…/oaf_drums.onnx CANTUS_MDB_DRUMS=".../MDB Drums" \
  CANTUS_DRUMS_EXTRA=…/pulsus-renders cargo test -p cantus-ml --release --test transcribe_drums_real -- --ignored --nocapture
# Run its two tests one at a time (name each: `drums_with_the_model_per_class`, then
# `drums_on_mixes_after_separation` with CANTUS_HTDEMUCS set): ONNX Runtime allows one
# environment per process, and each test makes its own (the helper itself keeps one for all its jobs).
CANTUS_ORT_LIBRARY=… CANTUS_BASIC_PITCH=… CANTUS_TRANSCRIBE_SET=…/bpref \
  cargo test -p cantus-ui --release --test transcribe_accuracy -- --ignored --nocapture
CANTUS_MDB_DRUMS=… cargo test -p cantus-dsp --release --test onset_drums_mdb -- --ignored --nocapture
```

References: `crates/cantus-ml/tools/reference_basic_pitch.py` (Spotify's `basic_pitch`
0.4.0 in a Python environment under `F:\projects\daw-r34\models\pyenv-bp`, layered on
stems' `pyenv`). Datasets stay local (`F:\projects\daw-r34\models\datasets`): MDB Drums
(CC BY-NC-SA 4.0), TRIOS (CC BY-NC-SA 2.0 UK) — measuring only, never committed.

## Marked lines (files others own)

- `crates/cantus-ml/src/lib.rs`: `pub mod transcribe;` — `// r34:transcribe`.
- `crates/cantus-ml/src/jobs.rs`: `("transcribe", crate::transcribe::job)` in `JOBS`.
- `crates/cantus-ml/Cargo.toml`: dev-dependency `flate2` (the fixture is gzipped).
- `crates/cantus-content/src/models.rs`: `Prepare::Onnx { file, patch }`, its validation
  arm (the patch checked), the `prepared` weights exception, `can_prepare`'s and
  `start_prepare`'s matches, `ready`'s file list, `prepare`'s arm, and
  `#[path = "models_onnx.rs"] mod onnx; pub use onnx::{apply as onnx_apply, OnnxPatch};`
  — each `// r34:transcribe`; the new file `models_onnx.rs` is mine.
- `crates/cantus-content/manifest/models.json`: the entries `basic-pitch` and `oaf-drums`
  (JSON has no comments).
- `crates/cantus-ui/src/ml_service.rs` (stems' `MlStarter` kept, mine removed in the
  merge): `Inner::ended` and `KEPT_ENDED`, `MlStarter::forget`/`kept`,
  `MlService::forget` through the starter and `kept`, and the router's updates through
  `get_mut` with the ended list bounded — each `// r34:transcribe`.
- `crates/cantus-content/src/models.rs` after stems' review 2: `map_of`'s ONNX arm (the
  hashes from the manifest the build carries), `prepared`'s size check on the graph itself
  for an ONNX model, `ready`'s file list without a weights file — `// r34:transcribe`; my
  `models_onnx.rs` gained `map_of`.
- `crates/cantus-ui/src/stems_ui_models.rs`: `in_use` counts a running conversion
  (`// r34:transcribe`).
- `xtask/notices/head.md`: the section "Basic Pitch's note creation, ported to Rust"
  (`<!-- r34:transcribe -->`); `THIRD-PARTY-NOTICES.md` regenerated.
- `crates/cantus-ui/src/lib.rs`: `pub mod transcribe_ui;`.
- `crates/cantus-ui/src/app.rs`: the field `transcribe`, its default, the frame's
  `transcribe_ui::run::frame`, the dialog's `transcribe_ui::view::show`.
- `crates/cantus-ui/src/views/arrangement_audio_menu.rs`: the *Convert to MIDI…* item and
  its arm.
- `crates/cantus-dsp/src/onset.rs`: `#[path] pub mod drums; pub mod eval;`.
- `docs/manual/{en,es}/README.md`: a link to the new page.

## For the owner

- **Ears and eyes**: convert a few of your own recordings (a sung line, a piano, a drum
  loop) and say whether the notes feel right.
- **The legal review** (already listed) gains two model sources: Spotify's Basic Pitch
  (Apache-2.0, in its repository) and the third-party ONNX conversion of Magenta's E-GMD
  checkpoint (Apache-2.0 per its card; Magenta publishes the checkpoint in its Apache-2.0
  repository without a separate licence line).
