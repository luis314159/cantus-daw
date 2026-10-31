# Round 34 — tools: what an agent can do and know (ADR 0082)

Builder **tools**, branch `r34/tools` (from 2b5f1790, mcp's first delivery; merged
`r34/integration` at 6af6430f and 8be124c9 on the way). ADR 0082. No schema change, no new
`Command` (so no new undo-fuzz arm in `undo_invariants.rs`: the tools plan only existing
commands; their own fuzz is `crates/cantus-ai/tests/tools_fuzz.rs`).

## What was done

1. **The project's tools** — `cantus_ai::tools_project::TOOLS`, 37 tools (36, and
   `replace_automation` since review 1) in `tools_*.rs`
   (args, json, read, song, tracks, notes, notes2, mixer, mod, music, session, remove, res):
   - read: `get_project`, `get_track`, `get_clip_notes`, `get_mixer`, `get_device`,
     `get_history`, `lint_music`;
   - the song: `set_tempo`, `set_tempo_point`, `set_song` (name, metre and metre changes,
     key, loop, metronome), `set_marker`;
   - tracks: `add_track` (a strip of its own, made when all are taken), `set_track`,
     `set_instrument` (destructive), `set_instrument_param` (native keys, Firmus keys, Pulsus
     `pad<note>.<field>` / `kit.<field>`, hosted ids; a sample path only inside the allowed
     folders, opened as `resolve_touch` gives it);
   - clips and notes: `add_clip`, `add_notes`, `replace_notes` (destructive), `edit_notes`
     (transpose, move, quantise, humanise, velocity, length, legato, mute), `set_clip`,
     `duplicate_clip`, `write_drum_pattern`;
   - the session: `set_session` (scenes, slots, launch, follow);
   - the mixer: `add_strip`, `set_strip` (and the master), `set_send`, `add_effect` (starting
     preset unless `neutral`), `set_effect` (params, reset, bypass, mix, sidechain, move,
     hosted ids), `replace_effect` (destructive);
   - movement: `automate` (points in the parameter's units, FL's shapes; draws into the clip
     under the first point or makes one), `add_modulator`, `set_modulator`;
   - music: `write_part`, `voice_chords`, `pedal_from_chart`;
   - `remove_items` (destructive): every removal in one tool, one step.
2. **The application's tools** — `cantus_ui::ai_tools::project_app::TOOLS` (10, in
   `agent_tools.rs`, `agent_units.rs`, `agent_export.rs`): `transport` (Transport),
   `get_selection` (selection, transport, export state), `list_units`, `get_unit`,
   `list_presets`, `apply_preset`, `comp_takes`, `tidy_takes` (destructive), `export_audio`
   (Files, the dialog's export), `export_project` (Files job: MIDI, DAWproject).
3. **The legacy bridge** moved into the table: `bridge.rs` is now the panel's door
   (`tool_specs` from the table, `to_command` through a `BareHost`); old names and arguments
   that live on: `set_tempo`, `set_tempo_point`, `add_track`, `add_notes`, `add_effect`.
4. **The unit catalogue**, generated from the tables with English names, groups, choices by
   number and name, presets and what they set, starting preset, latency at 48 kHz, uses —
   `list_units`, `get_unit`, `cantus://units`, `cantus://units/{id}`.
5. **The music linter and harmony** in `cantus-core/src/harmony*.rs` (MPL, ours): chords,
   charts, the linter, voicings, parts.
6. **Resources** `cantus://project`, `mixer`, `history`, `tracks/{id}`, `clips/{id}/notes`,
   `units`, `units/{id}`; **prompts** *Explain my mix*, *Write a part*, *Arrange an intro*,
   *Tidy the project*, *Teach me this unit* (both locales).
7. Every sentence names its target (`add 32 notes to “Bass” from 9:1`); names quoted as data.
   Strings under `# tools` in `locales/areas/ai.{en,es}.txt` (35 sentences, 19 step names,
   19 prompt strings). Manual: `docs/manual/{en,es}/ai-tools.md`, linked from each README.
8. Found by the tools' fuzz and fixed: two notes at one place could swap order across an undo
   — `InsertNotes`/`SetNotes` now sort by (start, pitch, id).

## How it was tested

- `cantus-core` `harmony::tests` (9): chord symbols (35 qualities, slash basses, errors),
  pitch names round trip, charts (shares, holds, repeats, 3/4, errors), **the demo's Fm7 held
  under E♭/G found** (A♭ an error against G; F and C warnings; E♭ common and fine; the fixed
  part clean), clashes/doubles/scale/range, voicings (essentials kept, smooth, deterministic,
  a too-narrow range refused), parts never ringing into the next chord over four patterns ×
  two rhythms with swing and humanisation (it caught my own writer doubling notes under
  humanised timing — fixed), common-tone ties and step rhythms.
- `cantus-ai` `tests/tools.rs` (5) and `tools_music.rs` (7): every tool through a bare host;
  **each mutating call checked to be one undo step whose undo gives back the project exactly
  and whose redo gives the edit again**; errors (range, unknown keys, not found, refusals
  that change nothing); classes; a song composed only through tools, linted clean, undone to
  the empty project; resources and prompts. `tools_fuzz.rs`: 64 seeds × 80 random calls over
  22 kinds of call, all undone and redone exactly. The bridge's 8 tests, updated.
- `cantus-ui` `tests/agent_tools.rs` (6), through `TestClient` with `CantusApp` as host: the
  catalogue complete (every `EffectKind` and `InstrumentKind`, every parameter with an English
  name and its choices, presets naming only real keys); every sentence key in English and
  Spanish; **a song composed through the server, rendered offline (sounds, finite, under full
  scale) and taken back by the user's Undo exactly**; comping (swipe, duplicate, rename,
  switch, delete) undone exactly; export refused without *Allow files*, outside the allowed
  folders and over an existing file, MIDI written (`MThd`), a WAV through the dialog's export;
  every one of the 47 tools called once, each edit one undo step.
- **New tests: 33** (cantus-core 9, cantus-ai 13 + the bridge's 8 rewritten, cantus-ui 6;
  plus the fuzz's 64 × 80 calls inside one).
- Windows (`RUSTFLAGS=-D warnings`): `cargo test -p cantus-core -p cantus-ai -p cantus-mcp`
  **712 passed**, 2 ignored, 0 failed; `cargo test -p cantus-ui --lib --test agent_tools
  --test ai_mcp_boot --test ai_mcp_pump` **391 passed**, 2 ignored, 0 failed;
  `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean for my crates.
- Linux (WSL Ubuntu 24.04, `F:/projects/daw-r34/tools-wsl.sh`, headless): clippy for the
  workspace and the plug-ins without their editor clean; `cantus-core`, `cantus-ai`,
  `cantus-mcp` **713 passed**; `agent_tools`, `ai_mcp_boot`, `ai_mcp_pump` 12 passed; the
  `ai_`/`i18n`/`history` lib tests 8 passed. The clone's `target/` deleted.

## Marked lines (`// r34:tools`)

- `crates/cantus-core/src/lib.rs`: `pub mod harmony;`.
- `crates/cantus-core/src/command.rs`: the two note sorts (`InsertNotes`, `SetNotes`) by
  (start, pitch, id).
- `crates/cantus-ui/src/ai_tools.rs`: `#[path = "agent_tools.rs"]`, `pub mod project_app;`,
  `project_app::TOOLS,` in `registry()`.
- `crates/cantus-ui/src/ai_catalog.rs`: `project_app::RESOURCES` and
  `project_app::RESOURCE_TEMPLATES` in `resources()`/`templates()`.
- `docs/manual/{en,es}/README.md`: one line each linking `ai-tools.md`.

## For the integrator

- The golden `tools/list` is not on `r34/integration` yet; regenerate it with
  `CANTUS_TOOLS_DUMP=1` after this merge (46 tools of mine, after `cantus-ai`'s, before
  listen's).
- `export_audio` is a `Host` tool of class Files (the dialog's export needs the application's
  plug-in render instances and its wait for samples, so it cannot be a pure job): its card
  shows its title, not a sentence. If mcp gives Host tools a sentence, it should say
  `export the {format} file {file}`.

## For the owner

- **Ears**: what `write_part`, `voice_chords` and `pedal_from_chart` write — the voicings'
  spacing, the arpeggios and bass lines, the pedal's lift (a sixteenth) — on Chorda, Lamina
  and Gravis.

## Review 1

Eighteen findings; mcp took #4's generic part and #5's generic check. Each fixed with a test
that fails without it (`crates/cantus-ai/tests/tools_review.rs`, `harmony_tests.rs`,
`bridge.rs`'s tests, `crates/cantus-ui/tests/agent_tools.rs`).

| # | Fix | Test |
|---|---|---|
| 1 | `apply_preset` is Destructive (it replaces a device's settings) | `a_preset_is_destructive` |
| 2 | `automate` and `pedal_from_chart` refuse a span holding points; `replace_automation` (Destructive) redraws, a point at the span's end replaced, not doubled | `automation_over_points_is_refused_and_redrawn_only_when_asked` |
| 3 | stems: each output name checked against `overwrite` | `exports_name_their_file_and_never_overwrite_unasked` |
| 4 | `export_audio` is a `HostSaid`: "export the WAV file “…”", "…, replacing it"; `export_project`'s sentence likewise | the same |
| — | mcp's fourth review: every output opened through `open_touch` (new unless `overwrite`) — on the worker for MIDI and DAWproject (`cantus_project::dawproject::export_into`), before the export starts for audio and stems | the same: a file that appears between the plan and the write is refused and left as it was |
| 5 | drums' bars, velocity, rows, steps and the session slot's notes refused, not clamped; every schema closed to the bottom | `drum_and_session_limits_are_refused_not_clamped`, `every_schema_is_closed_to_the_bottom`, `the_application_tools_schemas_are_closed` |
| 6 | `write_part`/`voice_chords` write a clip of their own; refused where the track has notes unless `clip_id` names one | `writers_make_their_own_clip` |
| 7 | the bass line's fifth is the chord's own, from its root | `the_bass_lines_fifth_is_the_chords_own` |
| 8 | a clash is a semitone *above* a chord tone; a dominant's ♭9/♭13 are tensions | `the_clash_rule_is_directional` (a ii–V–I on 9ths and 13ths lints clean) |
| 9 | step rhythms run on from the chart's start | `a_step_pattern_longer_than_a_bar_runs_across_it` |
| 10 | `chart_start_bar` 1…1 000 000, the chart's end inside the horizon | `a_chart_past_the_horizon_is_refused` |
| 11 | `transport` refused while Cantus records | `the_transport_is_the_users_while_recording` |
| 12 | samples only from the project's folder | `a_sample_comes_only_from_the_project_folder` |
| 13 | `replace_notes` within a loop's first pass; `edit_notes` refuses the hidden window and past the loop, grows a clip that does not loop | `notes_stay_where_they_are_heard` |
| 14 | the linter prepares each chord once and bisects (24 000 notes / 1 600 chords: under 40 ms, was 160); the loop expansion capped at 200 000 (`notes_capped`) | `the_linter_is_fast_on_a_big_song`, `the_lint_caps_a_loops_repetitions` |
| 15 | a swap renames only `^{instrument}( \d+)?$` | `a_user_named_track_keeps_its_name_on_a_swap` |
| 16 | modulators named by kind and number (`LFO 2`), `set_modulator` says which, the `host` text, `automation_points_per_call` | `words_say_what_they_mean` |
| 17 | names cleaned of control characters, one line | the same, and `sentences_are_worded_whole_in_spanish` |
| 18 | `bridge::to_command` returns `Planned { command, class, said }` | `a_planned_call_carries_its_class` |

**Localised phrases** (mcp's nested `Said`): every phrase a sentence holds — a clip and its
bars, a strip or the master, an effect, a parameter, a modulator, a list of fields — is a
`Said` of its own (`crates/cantus-ai/src/tools_words.rs`, `ai.phrase.*` and `ai.field.*` in
both locales), the user's names quoted by the host. `sentences_are_worded_whole_in_spanish`
holds it; `every_sentence_is_in_both_languages` checks the new keys.

## Review 2 (the fix round)

Eleven findings, each fixed with a test that fails without it.

| # | Fix | Test |
|---|---|---|
| 1 (high) | `edit_notes` works on the notes the window shows (unless `note_ids` names others) and checks or grows only for notes the call moved or lengthened that were heard | `an_edit_of_a_split_half_touches_only_what_it_shows` (both halves of a split, a trimmed tail) |
| 2 (high) | the agent's audio export creates and truncates nothing before the exporter writes: outputs checked where they are (no link; absent unless `overwrite`), and without leave the exporter places each file by a hard link that fails if anything appeared (`ExportSettings::no_replace`, `cantus-export`'s `Part`) | `an_audio_export_loses_nothing_when_it_does_not_finish` (start refused, stopped, a file appearing mid-render; with and without overwrite) |
| 3 | MIDI and DAWproject written whole: a `CreateNew` `.part` through `open_touch`, synced, then renamed (overwrite) or linked | `agent_export::tests::a_failed_write_leaves_the_old_file_whole` |
| 4 | the clash rule also flags a semitone below a major 3rd (a dominant's ♯9 excepted), a major 7th and a natural 13th; the ♭13 is a tension only over a perfect fifth | `the_clash_rule_is_directional` (B♭/Cmaj7, E♭/C, A/C7♯5, G/C7♭5, A♭/C13 flagged; the ii–V–I clean) |
| 5 | the lint counts a loop's repeats one by one up to its cap; `loop_beats` ≥ 1/64 beat | `a_tiny_loop_neither_floods_the_lint_nor_is_accepted` (a one-tick loop over 100 000 bars) |
| 6 | a list chart is held to the horizon | `a_list_chart_past_the_horizon_is_refused` |
| 7 | `automate` refuses where any clip of the parameter has points in the span the new curve takes; `replace_automation` clears them | `automation_never_overrides_another_clips_points` |
| 8 | `set_modulator` adds routes (`add_routes`), never replaces a modulator's; `remove_items` removes one by target; `write_drum_pattern` writes a clip of its own; an empty clip of the user's counts | `routes_are_added_and_drums_write_their_own_clip` |
| 9 | numbers and positions worded by the host (`116,5`, `9:2,5`), counts with their nouns (`1 punto`), `del máster`, `el modulador LFO 1 de «…»`, `la cantidad del modulador …`, an unknown effect in words | `sentences_are_worded_whole_in_spanish` (whole sentences asserted) |
| 10 | a Files card shows the whole path | `a_files_card_shows_the_whole_path` |
| 11 | `place_notes`' loop rule from the clip's offset | `add_notes_follows_the_loop_from_its_offset` |

Not mine, seen in the runs: `cantus-ui` `views::transport::layout::tests::short_groups_share_a_column_two_high`
fails on the integrated tree (11 columns, not 10: the top bar's agent chip, mcp's).

## Review 3

Ten findings, each fixed minimally with a test that fails without it.

| # | Fix | Test |
|---|---|---|
| 1 (high) | `views::export::remember` clears `no_replace` (a marked line): an agent's "never replace" never reaches the user's settings | `an_agents_no_replace_stays_out_of_the_users_settings` |
| 2 | `cantus_export::job::place_new`: a hard link, or — where the file system has none — a copy into a file made with `create_new`, synced, removed again if the copy fails; used by the exporter's parts and by `write_whole` | `without_hard_links_the_part_is_copied_into_a_new_file` |
| 3 | `remove_items`' `modulator_routes` edits each modulator once with every route it removes; a route named twice is refused | `two_routes_of_one_modulator_go_together` |
| 4 | `automate`'s overlap check reads every track's clips of the target and clears each on its own track | `automation_checks_every_tracks_lanes` |
| 5 | the lint starts a loop at its first repeat in range and reports `notes_capped` when it stopped at the cap | `a_late_range_on_a_long_loop_checks_its_own_repeats` |
| 6 | `write_whole`'s part is named per process and call, so a stale one never blocks | `a_failed_write_leaves_the_old_file_whole` (a stale part, then a write) |
| 7 | removing the part after it is placed is best effort | `a_placed_output_succeeds_even_if_its_part_stays` |
| 8 | the exporter opens its part with `create_new` (`new_part`), never through a link; a stale plain part is removed first | `a_part_is_new_and_never_a_link` |
| 9 | `add_routes` together with `routes` is refused | `add_routes_and_routes_together_are_refused` |
| 10 | the commit's refusal held without a race at the exporter (`never_replacing_refuses_a_file_that_appeared_and_leaves_it_whole`); the UI test now asserts the job failed | both |

## The fonts' glyphs (after review 3)

`font_coverage` found the music flat, sharp and triangle (`♭ ♯ Δ`) in `harmony_chord.rs`'s
parsing tables, where the test reads a string list and a char array as drawable text; IBM
Plex has none of them. The parser still reads them (and `∆`) in what an agent writes, now
through `match` patterns (`triangle`, `accidental`). What Cantus says of
a chord is spelled by `harmony::spell` (`b`, `#`, `maj7` / `maj` before a degree): a
`ChordError`'s quote and reason, and the chords named in `lint_music`'s messages. No
exception was added to the test. The manual pages keep the signs: they open on GitHub, in
the browser's fonts. The core tests that need the signs moved to `cantus-core/tests/harmony_signs.rs` (the test reads
`src/`). Tests: `a_chord_is_shown_in_the_fonts_glyphs_and_read_in_any` (core),
`a_chart_in_music_signs_is_read_and_spoken_in_plain_letters` (ai), and `font_coverage`.

## Review 4

| # | Fix | Test |
|---|---|---|
| 1 | the UI test no longer races a running export: a file there before the call is refused and left whole; the refusal of one that appears mid-render is the exporter's deterministic test | `an_audio_export_loses_nothing_when_it_does_not_finish` |
| 2 | a link where the part goes, to a file elsewhere, is refused and that file left whole (Unix; Windows where file links can be made, else skipped) | `a_link_where_the_part_goes_is_refused_and_its_file_untouched` |
| 3 | without hard links, the part is moved with the system's no-replace rename (`cantus_rt::place::rename_no_replace`: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`, `renameat2(RENAME_NOREPLACE)`, macOS `renamex_np(RENAME_EXCL)`); the copy only where that is missing | `without_hard_links_the_part_is_renamed_without_replacing`, `place::tests::moves_and_never_replaces` |
| 4 | `write_whole` passes over part names that are taken (a reused process id) and never asks for `overwrite` about a part | `parts_left_under_this_process_id_never_block_a_write` |
