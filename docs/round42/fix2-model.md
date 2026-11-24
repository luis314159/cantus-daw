# Round 42 — `fix2-model`: the mid-round review's findings on the model side

Branch `r42/fix2-model` (worktree `F:\projects\daw-r42\fix2-model`), from `r42/integration` at
1da2e8df6. The core, project, file-format, UI, AI and rt side of the integrated review; Arcus's
and Exemplar's DSP are `fix2-arcus`'s. Each finding: its cause, the fix, the test that failed
before it.

| # | Finding | Cause | Fix | Test |
|---|---|---|---|---|
| 1 | `tie_legato` cut a repeated pitch | the tie moved the previous note's off past the legato note's on whatever its pitch: `on 60 @1000`, `off 60 @1001` | no tie when the note before has the same pitch, or the same `NoteId` (a looping clip's repetition) | `note_art::tests::a_repeated_pitch_is_never_tied` (same pitch; a loop's repetition; same id on another pitch; another pitch still tied) |
| 2 | a `.scl` with no `.kbm` anchored unlike Scala and Surge | `KeyboardMap::scala_default` put key 69 at 440 (31-EDO's key 60 at 359.8 Hz) | a Scala scale with no map has degree 0 on key 60 at C4 of 12-TET at the diapason (`C4_DEFAULT_HZ`, exactly −9 semitones: the 12-TET null stays bit-exact); a temperament keeps A on the diapason (`KeyboardMap::a4_anchored`); Ableton's `! @ABL REFERENCE_PITCH` read as the file's map (`tuning_scala::read_ascl_map`, as Surge reads it) | `tuning_offsets::a_scale_with_no_map_sounds_as_surge_s_tests_say` (Surge's expected values from its `alltests.cpp`: key 60 at `MIDI_0_FREQ*32` for every file, 12-intune A440, 31-EDO steps, 6-exact's known values, ED3-17, Carlos Alpha, 12-shuffled's order); `tuning_scala::ableton_s_reference_pitch_is_the_file_s_reference` (rast.ascl as Surge's test holds it; 31-edo.ascl gives the `31-edo.kbm` Live wrote; the bad file refused); the 31-EDO test and the tuning-face e2e moved to the new anchor |
| 3 | Scala export lost the diapason | the map's `reference_hz` written unscaled; no `.kbm` for a tuning without a map (a diapason, a temperament's A anchor) | `tuning_io::map_for_export`: the map scaled by the diapason, or a linear map with degree 0 on key 60 at what key 60 sounds, whenever the `.scl` alone would sound elsewhere | `fix2_model::a_scala_export_reads_back_as_the_same_pitches_here_and_in_surge` (six tunings, 0.01 cent, and a reader following Surge's documented `KeyboardMapping` rules) |
| 4 | a UTF-8 BOM refused | `decode` read `EF BB BF` as Latin-1 text on the first line | a leading BOM is dropped and says UTF-8 | `tuning_scala::a_utf8_byte_order_mark_is_dropped` (Surge's own files' bytes with a BOM: `.scl`, `.kbm`, `.ascl`) |
| 5 | a description starting with `!` broke the file | written as is, it read as a comment | a blank written before it, dropped on read | `tuning_scala::a_description_starting_with_a_bang_round_trips`; the e2e round trip's Ptolemy |
| 6 | articulation dropped by Slice, Duplicate, Bake | `slice_at`'s tail and `duplicate_selection`'s copies were new notes without `art` (the copies without `expr`, `release` too); `BakedNote` had no `art` | each keeps it (the tail its release; the copies all three); `BakedNote::art` carried through the chain | `fix2_model::{slicing_keeps…, duplicating_keeps…, baking_note_effects_keeps…}`; `notefx::tests::a_baked_note_keeps_its_articulation` |
| 7 | exports dropped tuning and articulation in silence | no report lines; MusicXML wrote no marks; MIDI had no tuning | DAWproject reports `x_tuning`, `x_track_tuning`, `x_note_art`; MusicXML writes/reads `<staccato/>`, `<spiccato/>`, `<tenuto/>`, `<up-bow/>`, `<down-bow/>`, `<tremolo>` and `pizzicato="yes"` (MusicXML has no pizzicato mark: checked on w3.org's 4.0 reference); MIDI's *Write the tuning* (off by default): a bulk dump, plus scale/octave or the program selected, never both — the export dialog's switch (kit, both locales), the agent's `export_project` `tuning` | `dawproject::the_tuning_and_the_notes_articulation_are_reported…`; `musicxml_roundtrip::articulations_and_bows_are_written_as_musicxml_names_them…` (and a hand-written file read); `midi_tuning.rs` (bytes by the MMA's layout, computed in the test); `fix2_model::{musicxml_from_the_app…, the_midi_export_writes_the_tuning_only_when_asked}` |
| 8 | undo-fuzz gaps | no `KeyboardMap`, no key past B, no non-finite diapason, no merged path, not in the main mix | keyboard maps with `x` and every case `check` refuses; keys 12–255; NaN/±∞ diapasons; half the tuning sets as merged gestures (one undo step each, checked); `random_command` arm 49 | `undo_invariants::tuning_and_articulation_undo_and_redo_among_the_others` (floors: merged > 40, maps > 20, with `x` > 5) |
| 9 | MCP | `note_json` had no `art`; `replace_notes` wrote notes without it; no Arcus row; no tuning tools | `art` read and written (schema, parse, `to_clip_notes`); the `arcus` row; `get_tuning`, `set_tuning` through the commands and `tuning_cmd::check` | `cantus-ai/tests/tools_tuning.rs` (five tests, each edit's undo/redo exact); the MCP golden list rewritten |
| 10 | a frozen External-tuned track did not go stale | the content key is the song's alone; the master's table is another program's | `freeze::follows_external`; when the master's table moves (and once a session), `cantus_ui::freeze::master_moved` sets those renders aside and removes their own copies — they are made again, playing live meanwhile | `freeze::a_target_follows_the_master_only_where_its_region_plays_external`; `tuning_mts_e2e::a_frozen_track_in_the_masters_tuning_follows_the_master` (fake `LIBMTS`) |
| 11 | Chorda's diapason readout ignored the master | `effective_a4(spec, resolved)` evaluated the tuning without the master's table | `views::tuning::chorda::shown_a4`, from the card's `a4_of` (which reads the master's A4 under `External`); the Edit view calls it | `tuning_mts_e2e::chordas_readout_and_its_tuning_card_agree_under_the_master` |
| 12 | a quick double-click on empty grid opened the menu | the first click drew a note; the second, a double-click, hit it | the grid remembers the note its last click drew (kept through the double-click's second click); the menu skips it | `fix2_model::a_quick_double_click_on_empty_grid_draws_a_note_and_opens_no_menu` (and a later double-click on it opens the menu) |
| 13 | `Mts::read` made a `&[f64]` of shared memory | `from_raw_parts` over a table another process writes | `ptr::read_volatile` per element | the `cantus-rt` MTS tests (no behaviour a test can tell apart: the change removes undefined behaviour, which no test observes) |
| 14 | docs | ADR 0158 named `SetNoteArt { clip, notes, art }`; schema 80's comments left out two fields; the fixture had no keyboard map | ADR 0158 says `{ track, clip, arts }`; `migrate_79_to_80` and `SCHEMA_VERSION` name `ExternalUnit::mts` and `ExternalPlugin::tuning_by`; v82's project plays Ptolemy on the white keys at A415 (five `x` keys) — v82 is unreleased, re-written | `format::tests::schema_82_keeps…` (the map, its `x` keys, an `x` key silent) |

**After the review of this delivery (the coordinator):** Chorda's two presets built from a Scala
scale (*Slightly out*, *Saloon*) now carry the A4-anchored map they were designed with
(`KeyboardMap::a4_anchored`), so no shipped sound moves — `fix2_model::
chordas_scala_presets_sound_as_they_were_designed` holds their 128 offsets bit-identical to the
old path's in all twelve keys, and the engine's `null_tests`, `inst_chorda`, `tuning_null` and
`tuning_instruments` pass unchanged. The e2e round trip also found a stale-file bug: an export
needing no `.kbm` left an earlier export's `.kbm` of the same name beside the new `.scl` (read
with it, here and in Surge); it is removed now, and the test checks the pair on disk.

ADRs 0155, 0156 and 0158 carry amendments (anchors, `.ascl`, export, what each format
carries; the frozen render under the master; the look-ahead and the edits). The manual's
tuning page says the anchor, the `.kbm` on export and the tuning in other files, in both
languages.

## Gates (Windows, `CARGO_TARGET_DIR=C:/cantus-r42-target/fix2-model`, merged with
`r42/integration` 7c1d543a0)

- `cargo test -p cantus-core`: every target green (40 result lines), the undo fuzz included.
- `-p cantus-project -p cantus-score -p cantus-ai -p cantus-rt`: green (after v82 was re-written
  by `write_fixture` and the kit test moved to A440, where scale/octave can hold the tuning).
- `-p cantus-engine --lib --test null_tests --test notefx --test tuning_instruments
  --test tuning_null --test inst_chorda`: green — no golden moved.
- `-p cantus-ui --lib` (620, the i18n test among them) and `fix2_model`, `tuning_mts_e2e`,
  `tuning_face_e2e`, `tuning_fixes`, `interop_flows`, `notefx_flows`, `mpe`,
  `chorda_tune_e2e`, `tuning_engine_e2e`, `tuning_whole`, `freeze`, `strings_rec_e2e`,
  `app_flows`, `locale_isolation`: green. `ai_mcp_boot`'s golden list re-written
  (`CANTUS_TOOLS_DUMP=1`: `get_tuning`, `set_tuning`, the `arcus` kind, the notes' `art`,
  `export_project`'s `tuning`) and green.
- `cargo clippy --workspace --all-targets --keep-going -- -D warnings`: clean.
- **The fuzz's wider mix** (`random_command` over 50 arms, the tuning arm 49): each older arm
  is drawn 1/50 instead of 1/49 (−2 %). Measured on the default 120 seeds against the same
  test at 49 arms: commands applied per seed, mean 103.6 (was 103.0), least 86 (was 77; floor
  75); mix commands, mean 10.95 (was 11.03), least 3 (was 3; floor 3); every per-kind floor
  (clipboard cases, routing batches, warps, tempo gestures) passes.

## Verified only in emulation

MTS-ESP with the fake `LIBMTS` of `cantus-rt`'s tests, not a real master; the MIDI file's
tuning messages by the MMA's layout and our MTS reader, not by a hardware synthesiser.
