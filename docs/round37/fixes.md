# Round 37 — fixes (the integrated-tree review of drums and library)

Branch `r37/fixes`, base `r37/integration` dd9ef3a6. ADR 0100 §12 and ADR 0101 §7 (the
review's fixes) say what changed; no schema, no `DSP_VERSION` move, no golden render moved.
The MCP `tools/list` golden is regenerated (the effect lists, `audition_preset`'s and
`set_pad_chain`'s wording).

## Drums (ADR 0100)

| # | Fix | Where | Held by |
|---|---|---|---|
| 1 | A kit routed to another strip takes its pads' chains (and, split, its pad strips) with it, in the same batch, whoever routes it; `first_free_strip` never counts a strip a chain or pad strip plays into as free | `padout_chain.rs` `may_reroute`/`with_kit_strips_following`, `bus.rs` `execute_inner` (beside `may_orphan`), `model.rs` `first_free_strip` | `padout_tests.rs` `a_kit_routed_to_another_strip_takes_its_chains_and_pad_strips`, `a_strip_a_chain_plays_into_is_not_free`; undo fuzz kind 8 (a kit routed); `tests/padout_fixes.rs` `a_kit_routed_to_another_strip_takes_its_chained_pad_with_it` (through `assemble`: `route_track` 1 → 2, strip 2 muted → the engine plays silence, the kick included; freeze allowed, no Merge back; undo) |
| 2 | Track templates keep a kit's chains and pad strips; inserted, new ids (effects, modulators, sends, outputs remapped), after the track's strip; a pad output the template does not bring is cleared | `templates.rs` `track_template`, `kit_strips`, `insert_track` | `padout_fixes.rs` `a_kits_track_template_keeps_its_chains_in_the_new_projects_ids` (same project: own chains, never the original's; another project: no dangling id, freezes; undo exact) |
| 3 | Explode takes: the copies' pads get no outputs | `comping.rs` `explode_takes` | `padout_fixes.rs` `exploding_a_kits_takes_leaves_its_chains_with_it` (the chain's last insert then comes out) |
| 4 | Stems: a chain straight to the master is in the Direct stem; a kit's track stem holds its chains (post-chain, in place of the dry pad) | engine `graph/strip_run.rs`, `graph/strip.rs` (`chain_of`), `padout.rs` (`into_chain`), `graph/track.rs`, `processor_snap.rs`, `listen_fork.rs`; `views/interop.rs` `stem_specs` (Direct when such a chain exists) | `padout_fixes.rs` `a_chained_pad_of_a_kit_on_the_master_is_in_the_strips_stems` (Direct + strips into the master = master within 1e-5), `a_kits_track_stem_holds_its_pads_chains` (track stem = the master within 1e-5, and differs from the dry stem) |
| 5 | `carry_outputs`: by pitch, then voice only when not `Sample`, then name, then place | `padout.rs` | `padout_tests.rs` `sample_pads_carry_outputs_by_pitch_name_or_place_not_by_voice`; `padout_fixes.rs` `a_sample_kit_on_a_split_one_gives_each_strip_to_the_pad_of_its_name` (through `set_instrument`) |
| 6 | A split or a chain's first insert past 256 engine tracks (project + taps) or 256 strips is refused (`Refusal::NoRoom`), said in both languages and to agents | `padout.rs` `with_room`, `ENGINE_TRACKS`/`ENGINE_STRIPS` (a `const` assertion in `cantus-engine/src/messages.rs` holds them to the engine's), `padout_ui.rs`, `ai_tools_drums.rs`, `padout.no_room` EN/ES | `padout_fixes.rs` `a_split_the_engine_has_no_room_for_is_refused_and_said` (250 tracks, English and Spanish) |
| 7 | The code offers models 1 and 2 the upgrade (the manual said so); ADR 0100 §1 said model 2 got none — corrected, and two stale code comments | ADR 0100 §1, `pulsus_editor{,_panel}.rs` comments | — (documentation) |

## Library (ADR 0101)

| # | Fix | Where | Held by |
|---|---|---|---|
| 8 | `find_presets`/`audition_preset` answer in English through a scoped guard (`i18n::scoped_thread_locale`) that gives the thread back its language | `i18n.rs`, `ai_tools_library.rs` | `tests/library_fixes.rs` `the_audition_is_a_job_stop_stops_it_and_the_window_keeps_its_language` (booted Spanish, still Spanish after both) |
| 9 | `apply_preset`/`list_presets` read the kits from `preset_catalog::entries(Kit)` by their English names (*Default kit*, *Studio kit* kept as aliases) | `agent_tools.rs` (the Pulsus arm of `presets_of` only — the Native arm untouched) | `library_fixes.rs` `every_preset_found_loads_and_reverb_is_spatium` (every unit: every `find_presets` name is in `list_presets`; every kit and each unit's last loaded through `apply_preset`) |
| 10 | The retired Reverb offered nowhere new: `reverb` → Spatium in `args::effect_kind`, `effect_ids()`/`list_units` filter `offered()`, the pad chain's *Add insert ▾* too; descriptions fixed | `cantus-ai` `tools_args.rs`, `tools_mixer.rs`; `agent_units.rs`, `ai_tools_drums.rs`, `views/pulsus_editor_chain.rs` | the same test (`add_effect` and `set_pad_chain` with `reverb` make a Spatium; no schema says "reverb (simple)"; `list_units` has no `reverb`); `pulsus_editor_chain` unit test; `legacy_reverb.rs` unchanged |
| 11 | `audition_preset` is a `ToolRun::Job`: prepared on the UI thread, rendered and measured on the worker, handed when played to the browser's audition (`Audition::begin_elsewhere`: heard, until) — Stop, Esc (now also with the browser closed) and the next audition stop it; the frame polls it with the browser closed; stopped while rendering it answers `stopped` | `ai_tools_library.rs`, `preset_audition.rs` (`begin_elsewhere`, `work` made `pub(crate)` — `prepare` untouched), `app.rs` (the poll beside `poll_samples`, Esc) | the same test as 8: Stop during the render → `stopped`; played → the answer's loudness, the browser's `heard`/`until`, no pump over 500 ms, sound from the manual engine, then silence after Stop |
| 12 | Exemplar's count 26 → 32 in the README and the site; no generator exists (`units_data.rs` says it is kept by hand), so a test now holds every stated count | `README.md`, `web/src/units_data.rs` | `tests/stated_preset_counts.rs` |

## The review of the fix rounds (second pass)

| # | Fix | Held by |
|---|---|---|
| 1 | One free-strip rule: `Project::strip_is_free` / `Project::free_strips` (an audio strip — never a VCA, a chain or a pad's own strip — that no track plays into, through its output or a pad's, and no chain or pad strip plays into), used by `first_free_strip`, `audio_files::free_strips` (a sample on the rack, imported audio, *Convert to MIDI…*, *Double* and *MIDI out*) and the MIDI import's `FreeStrips` | `tests/free_strips_paths.rs`: a kit split with its pad strips before the desk's empty strips, a chain and a VCA; through `assemble`, a sample dropped on the rack, two files imported, a MIDI file imported, *Convert to MIDI…* and *Double* each make a track on no pad strip, chain or VCA; the rule is the one `first_free_strip` gives |
| 2 | A kit template's strips are inserted pads' own strips first, chains after, so the insert's undo takes a chain away before the strip it feeds (it was refused) | `padout_fixes.rs` `a_kits_template_with_a_chain_before_its_pad_strips_undoes_and_keeps_no_vca` (the chain made with the kit on the master, the kit routed to a later strip, split, template, insert, undo exact) |
| 3 | The retired Reverb answers to `reverb_legacy` (outputs, `get_unit`, `list_presets`, `find_presets`' unit); `reverb` stays Spatium for new inserts (`args::effect_kind` for new inserts, `args::any_effect_kind` for describing what is there) | `library_fixes.rs` `an_old_projects_reverb_answers_to_its_own_name` (the mixer says `reverb_legacy`, its unit lists the old keys, `set_effect` takes them, `reverb` is Spatium) |
| 4 | `audition_preset`: the agent's cancel is checked after the render too; a cancelled job answers Cancelled and is never played (its channel dropped unsent) | `tests/audition_cancel.rs` (its own binary): the job held right after its render by a test gate (`ai_tools::library::RENDERED_HOOK`), `cancel_job`, let go — `wait_job` says cancelled, no measurement, the browser's audition lets go, nothing handed to the player, the manual engine silent. Without the check it fails (the audition is played) |
| 5 | A template keeps no VCA ids (`vcas` cleared on save, and on insert for older templates) | the test of item 2 |

Second pass, Windows (r37/integration 65846ccc merged forward): workspace clippy
`-D warnings` clean; `cantus-core` 745 passed, `cantus-project` 181, `cantus-ai` 49 — 0
failed; `cantus-ui` whole: 2 438 passed, 1 failed, 43 ignored — the one `stage_budget::a_dancer_on_screen_stays_in_budget`
(the triangle count, not this branch's, as before). The `tools/list` golden did not change.

## Merged with piano's review fixes

`r37/integration` at 6f965af3 (piano's single preset path, `views::inst::preset_target`) is
merged in (`d47ac7d6`, no conflict). Both library items go through it: the agent's
`audition_preset` job calls `preset_audition::prepare` and `work`, which re-cut a Sectio
style on the worker through `preset_target` (the `restyle` field); `apply_preset`'s Native
arm is piano's, through `preset_target`, untouched here — item 9 changed only the Pulsus arm.

## Numbers (Windows, the merged tree)

- `cargo clippy --workspace --all-targets -- -D warnings`: clean (before and after the merge).
- `cantus-core`: **745 passed**, 0 failed, 1 ignored (`undo_invariants` included: 5 passed).
- `cantus-engine`: **1 102 passed**, 0 failed, 16 ignored (`rt_memory`, `padout` included; on
  the tree before the merge, which changed no engine file).
- `cantus-ai`: **49 passed**, 0 failed.
- `cantus-ui` whole (`--no-fail-fast`): **2 429 passed, 3 failed, 43 ignored**. Two were
  this branch's and are fixed and re-run green: `agent_tools`
  `the_catalogue_is_complete_and_generated_from_the_tables` (it expected the retired Reverb
  in `list_units`; now it expects it absent) and `topbar`
  `the_window_title_follows_the_project_through_the_boot_path` (it reads the first 3 000
  characters of `frame`; the audition's poll moved after the title's sync). The third,
  `frame_budget` `stage_budget::a_dancer_on_screen_stays_in_budget` (55 214 triangles against
  50 674 with a dancer on screen), is not this branch's — no drawing file is touched here;
  the library builder's report lists it failing in the integrated tree too.
- New tests: `padout_tests.rs` +3, `tests/padout_fixes.rs` 7, `tests/library_fixes.rs` 2,
  `tests/stated_preset_counts.rs` 1, `pulsus_editor_chain` unit 1; the undo fuzz's padout
  stream has a ninth kind.
- `tools/list` golden regenerated (`CANTUS_TOOLS_DUMP=1`): the effect `enum` now holds the
  plain names its description promised (the dispatcher refused `eq`, `reverb`… before), the
  retired `reverb (simple)` gone, `audition_preset`'s and `set_pad_chain`'s wording.
- Linux (WSL Ubuntu 24.04, `tools/wsl/check.sh r37/fixes clippy`, `~/ci-r37-fixes`, at
  d0eb63a7 — before the merge): workspace clippy exit 0, plug-ins without editor exit 0; the
  clone's `target/` deleted.

## Not done, and why

- **A latent chain in a track stem** is late by its own latency (the track stem is taken
  before compensation). Every Cantus insert a chain gets by default is zero-latency; making
  the track stem compensated per chain would need a line per chain in the stem tap. Said in
  ADR 0100 §12.
- **The agent's cancel during a render** stops the job at its end, not mid-render (the
  job's `cancelled` is not bridged to the render's `RenderProgress`); Stop and Esc do stop
  it mid-render. A render is at most ~9.5 s.
- **Website sounds and packs** not re-rendered (the integrator's, as asked).
- Verified only in emulation: nothing here needs hardware.
