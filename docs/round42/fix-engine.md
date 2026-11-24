# Round 42 — `fix-engine`: the mid-round review's engine, core and test findings

Branch `r42/fix-engine` from `r42/integration` (433871a3e). The findings are
`F:\projects\daw-r42\review\verified-correctness.md` (16) and `verified-reach.md` (13); every
CONFIRMED and PARTLY item that is not pure interface is fixed here. The interface's words, the
checklist's and menus' names and the Tuning window's key (reach 7's binding) are `fix-ui`'s.

## Correctness

| id | what changed | its test |
|---|---|---|
| 1 | A hosted plug-in is never tuned twice. `ExternalPlugin::tuning_by` (`TuningBy::MtsEsp`, the default and not written, or `NoteExpression`; schema 80's), `Command::SetInstrumentTuningBy` with its inverse, freeze-guard line and undo-fuzz arm, `cmd.tuning.by` and the inspector's *Tuning by* row (both locales; a minimal row for `fix-ui` to dress). The engine hands a hosted unit at *MTS-ESP* only its track's table **less** what MTS-ESP gives clients (`tuning::beyond`): the outside master's table, or the project's while Cantus is the master (`ExternalTuning::leads`, an atomic flag). It is exactly `0.0` where they agree (nothing is sent), and the difference for a track with a tuning of its own. Live, it is read each block (`TrackSlot::set_heard`); offline, it is `Snapshot::mts_heard`, filled by `fill_external`. ADR 0156 states it; there is a checklist item. | `tuning_mts_e2e::as_the_master_cantus_publishes_its_tuning_and_tunes_no_client_twice` (the CLAP test synth: whole table with no master, nothing on top of MTS-ESP as master, whole table at *Note expression*); `tuning.rs` unit tests `a_client_is_sent_only_what_mts_esp_does_not_give_it`, `a_compile_records_what_mts_esp_gives_its_clients` |
| 2, 7, 8 | The master's changes recompile. `views::tuning::master_moved`, run at each `sync_engine`, reads the master for a song that follows it anywhere (`follows_master`) and forces a fresh snapshot when the table changes or the master comes or goes. That snapshot feeds Chorda's design, the strips' and master's effects and offline copies. `install_mts` forces a resync (the boot's first snapshot came before the link). Live, the block refresh puts a track whose master left back on its compiled table (`TrackSlot::tuning_live`). | e2e `a_song_in_the_masters_tuning_follows_it…` (live snapshot's track and project tables follow the master; no recompile while it is still; equal when it goes); `tuning_instruments::a_track_whose_master_goes_plays_its_compiled_table_again` |
| 3 | `restart_audio`'s success arm is now `CantusApp::adopt_engine` (so tests reach it). It hands the new engine the link (`views::tuning::engine_replaced`). | e2e: the export after `adopt_engine` still plays the master's table, and the engine holds the link |
| 4 | The freeze key holds each region track's resolved tuning (`plays_in`) and the project's (`tuning`) where a pitched effect of the region follows it (Tonus, Saltus, Loquax, Nitor, a board with one). Both are written only when not the default, so no key moves and `SUBJECT_VERSION` stays. | `freeze::the_key_follows_the_tuning_its_tracks_play_in` |
| 5 | `replace_instrument` hands the new unit its slot's tuning. | `tuning_instruments::a_unit_swapped_in_plays_its_tracks_tuning` (untuned control, then tuned, to the bit) |
| 6 | Exemplar decides unmapped keys per zone (`TrackSlot::tuning_per_zone`). A keytrack-0 zone plays them; a pitched zone does not. ADR 0155 says what hosted drums need (the track's *Equal*). | `exemplar_tests4::an_unmapped_key_silences_pitched_zones_only` |
| 9 | The processor's and handle's first "no MTS-ESP" is one process-wide `Arc` (`tuning::no_external`), so the audio thread never frees it. `rt_memory` now sends the link after counting starts. | `rt_memory::the_mts_esp_refresh_and_a_tuning_never_allocate` |
| 10 | Exemplar gets its table before `configure` (which sizes a streamed zone's mip levels), and `configure` adds the tuning's reach. `Exemplar::set_tuning` republishes the starts when the reach changes. | `inst_stream_review2::a_tuning_that_raises_keys_keeps_the_levels_it_needs` |
| 11 | Calor, Unda and Modulus move the glide's origin (`last_pitch`) with a note still sounding when the tuning changes. | `calor_synth`/`unda_synth`/`modulus_synth` `a_retune_moves_the_glide_origin_of_a_sounding_note` |
| 12 | The mono string's offset rides in the glide's target (`note_of` for every engine). `note_tune` is gone, so the string's code is the contract's again. | `gravis_tests::a_string_slide_between_tuned_keys_starts_without_a_jump` |
| 13 | MTS SysEx: `mts_sent` is keyed by the table sent. An `External` track sends the master's table, and sends it again when it changes. | `tuning_mts_e2e::an_mts_instrument_is_sent_its_table…` |
| 14 | Loquax's carrier skips an unmapped key. The outboard note-out path skips one too. | `loquax_carrier::an_unmapped_key_starts_no_carrier`; `tuning_outboard::an_unmapped_key_sends_no_note_to_the_synth` |
| 15 | `TuningTable` is equal by bits (`PartialEq` and `Eq`), so an unmapped key's `NAN` equals itself. | `tuning::a_table_with_an_unmapped_key_equals_itself` |
| 16 | ADR 0155 states the MPE route's 14-bit bend over ±48 semitones: 0.29 c at most, clipped beyond. | (documentation) |

## Reach

| id | what changed | its test |
|---|---|---|
| 1 | `tuning_face_e2e`, `tuning_whole` and `shoot_tuning` read the committed `crates/cantus-core/tests/data/tuning-library/`. The e2e imports `12-shuffled.scl` (degrees written out of order) instead of `marvel12.scl`. Its table, checked honestly: A4 0 c, C4 −100 c, D4 −200 c. Chorda is measured near each key's tuned pitch (±2.5 %), so a key left where it was is not found. | the two tests themselves |
| 2 | `picker_warm`: the single-sample `worst` gate is gone. p99 stays the gate; `worst` is still reported. | — |
| 3 | See correctness 3. | e2e |
| 4 | See correctness 1. | e2e |
| 5 | See correctness 2. | e2e |
| 6 | `tuning_mts_e2e.rs` on `CantusApp::assemble` with the fake `LIBMTS`. It covers: export follows the master; the master's change recompiles; it survives an engine replacement; it falls back when the master goes; as master, Cantus publishes and republishes after an edit. The boot assertion of `tuning_face_e2e` checks that the link reached the engine (`EngineHandle::external_tuning`). | — |
| 7 | Keymap: a version-19 file gets the Tuning window's key where it is free, whichever key the preset gives it. A user's own binding of that key stays theirs, and a current-version file without the line stays unbound. The key itself, the menu texts and the app-reach tests are `fix-ui`'s. | `keymap::a_version_19_file_gets_the_tuning_key_where_it_is_free` |
| 8 | (a) Off, disabled or removed: 12-TET at A440 is sent once and the entry dropped, so on again sends afresh. `poll` sends tunings before routes, and the out thread writes SysEx before it looks at the routes again, so a removal's reset finds its port (best effort). (b) A route whose port is not open keeps its messages, bounded. (c) See correctness 13. `SyncService::use_pool` lets a test give the out thread test ports. | e2e `an_mts_instrument_is_sent_its_table_reset_when_off_and_follows_the_master` (through `external_service::poll`) |
| 9 | Scale Workshop's exporter test (MIT, `tests/data/scale-workshop/SOURCE.md`; values only, no code). Its scale's 128 frequencies, through our `encode_word`, hash to its SHA-256 of the words. Our dump (`write_key_dump_of`, new, f64 pitches), with its checksum convention, hashes to its dump's. Our reader names that convention. | `tuning_mts::scale_workshop_s_bulk_dump_hashes_are_ours` |
| 10 | `tuning_instruments` timed at opt-level 3 on this machine (see *Measured*). | — |
| 11 | `ExternalUnit::mts` and `tuning_by` are saved and read back, and not written at their defaults. | `cantus-project/tests/schema80_fields.rs` |
| 12 | `scala-archive.sh`: L6's `--retry 5 --retry-all-errors --retry-delay 5`, and a `shasum -a 256` fallback. ADR 0155 says the archive sweep is a hand-run tool, not CI. | `bash -n` |
| 13 | The video preview's snapshot gets the master's table (`preview_snapshot`, `fill_external` with the engine's link). | `video_export_job::preview_tuning::the_preview_plays_the_mts_esp_masters_table` |

## Measured

- **Each test failed first** (a red run on this machine: each fix switched off in place, the tests run, the fix restored): Loquax's, Calor's, Unda's, Modulus's and
  Exemplar's DSP tests; `rt_memory`'s MTS-ESP refresh (the audio thread freed the first
  `NoExternal`); the swapped unit and the leaving master in `tuning_instruments`; the outboard
  unmapped key. Others fail by construction: `TuningTable`'s derived `PartialEq` made `NAN ≠ NAN`;
  `12-shuffled` and the e2e/fixture tests use API this branch adds. Two are weaker. The streamed
  levels test calls the unit itself, so it holds `configure`'s use of the reach, not the
  processor's ordering. Gravis's slide was not run red (the old code is a larger revert); by its
  numbers it jumps 0.5 semitone at the slide's start.
- **Scale Workshop's hashes**: both SHA-256 match on the first run: the 384 words, and the
  408-byte dump with its checksum convention. Every one of the 128 words is byte for byte what
  its exporter writes, edges included (key 0 at 0.45 Hz → `00 00 00`, the top keys → `7F 7F 7E`).
- **`tuning_instruments`** (opt-level 3, Windows, two other builders compiling): 98 s for its
  11 tests before. Its coarse search took ~5.6·10⁹ `sin_cos`. That search now uses a window
  computed once and a rotating oscillator: 17 s for the 11 (the same load). The fine phase-slope measure is
  unchanged, so the results are the same to the printed digit.

- **The suites** (Windows, `--no-fail-fast`): core, dsp, project and host 2 701 passed, 0
  failed (75 ignored); engine 1 500 passed, 0 failed (26 ignored); ui 1 820 passed. The ui
  suite's first run had 12 failures, most of them the disk filling during the run (os error
  112; F: reached 0 bytes with four builders). Run again with room, the disk ones pass (inst_sampler, inst_wavetable, comparatio_e2e, hosted_view_isolated, hosted_view_isolated_busy). Four remain, and none is in code this branch touches. `idle_repaint_e2e` (2): the picker warm-up (`views/picker_warm.rs:96`) asks an idle window for frames. `inst_chorda` (2): Chorda's Temperament card elides "Import .scl / .kbm…" and "Export…" at 75 % and in Spanish. Both are `tuning-face`'s, for `fix-ui` and the integrator. Clippy `--all-targets` with `RUSTFLAGS=-D warnings` is clean on core, dsp, engine, project, host and ui (after two fixes: a test module moved to the end of `video_export_job.rs`, std's `powi` out of a test).

## Verified only in emulation

- MTS-ESP against the fake `LIBMTS`. "Not tuned twice" was shown with the CLAP test synth, which
  reads per-note tuning but not MTS-ESP. A real client (Surge XT) is the checklist's new item
  (*Sin afinar dos veces*).
- MTS SysEx through test ports; no synthesizer heard it.

## Left

- `fix-ui`: the inspector's *Tuning by* row's look and placement (a minimal row is there); the
  Tuning window's key and the app-reach tests of reach 7; the four "Project ▸ Tuning" texts; the
  two `tuning-face` failures above (`idle_repaint_e2e`, `inst_chorda`).
- The owner: a real MTS-ESP client (checklist *Sin afinar dos veces*), a synth that reads MTS.
- Not a test of its own: the processor hands Exemplar its table before `configure`. The streamed
  levels test holds what `configure` does with the reach.
