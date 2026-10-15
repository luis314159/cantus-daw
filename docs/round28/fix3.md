# Round 28 — fix3: leaders' second review, the integration review, sound's second review

Branch `r28/fix3` (worktree `F:\projects\daw-r28\fix2`), from `r28/integration` at
`4dab842`. Every confirmed finding of `reviews/{leaders-2, integration-1, sound-2}.json`,
deduplicated (the lenses overlap), with how it was closed and the test that holds it.
"Seen failing" means the test was run against the old code and failed (a test that could
not compile against the old code, because it names something new, says so).

**Schema:** none. **Golden hashes:** none moved (engine and bench suites green after every
group). **DSP_VERSION:** unchanged (nothing at defaults or in a golden render moved: the new
paths run only when a keyboard changes mid-song, a strike lands on old strings, or an insert
has latency before it).

## leaders-2 — modulators, hosted automation, comping

| Finding(s) | Fixed how | Test |
|---|---|---|
| 0, 8, 18 (HIGH) — an unstored hosted parameter drifted: a modulation sum of 0 released it, nothing was sent back, the next drive took the last modulated value as its base | `Known::release`: the base (`auto_base`) is taken once per drive and sent back on release — a sum of 0 (`HostedValue::is_released`) and a replan alike — where the project stores no value; then forgotten, so a change made in the plug-in while not driven is respected | `unit::tests::a_modulation_sum_of_zero_sends_an_unstored_parameter_back_to_its_base`, `…let_go_by_a_replan_returns_to_its_base_and_swings_there_again` — seen failing |
| 2 (HIGH), 1 — `Lane::sent` outlived a unit swapped in place (a plug-in's restart), and was stamped when the push found no room | `AutoQueue::push` returns whether it took the change; `hosted_at` and the control point stamp `sent` only then; `replace_effect`/`replace_instrument` (in place) call `auto_unit_swapped`: every lane's `sent` forgotten (stopped: a control point now) | `hosted_queue::a_unit_swapped_in_place_hears_the_held_value_within_a_grid_step`, `a_held_value_whose_change_found_no_room_reaches_the_unit_later` (200 automated parameters, a ramp ending 800 samples into a pass) — both seen failing |
| 3 — drops counted only from automated units | each insert's and each instrument's `take_dropped` is collected where it runs (`strip_run`, `master_run`, `graph/track`), automated or not | `hosted_queue::an_unautomated_hosted_units_losses_are_counted` (strip, master, instrument) — seen failing |
| 4, 10 — doc comments and `#[inline]` on the wrong functions | moved back (`automation_replanned`, `apply_seam`, `PluginHost::record`) | — |
| 5 — send levels not modulatable | `modulators::wired_value` (the shared wiring of a sheet value) for the inspector's send and receive rows; `DeskMod` on the desk's FL send knob, its send rows and the cable pills (drop, Alt-drag, rings, live mark); the route menu (every send control's right-click) gains *Automate the send's level* and *Modulate the send's level…* | `modulate_everywhere::sends_and_the_pinned_strip_offer_modulate` — seen failing; `modulators::a_handle_dropped_on_a_desk_send_knob_connects_the_send` — seen failing; `mixer_routing::a_sends_menu_automates_and_modulates_its_level` |
| 6 — the inspector's pinned strip offered no modulation | `PinMod` on its fader, pan knob and the three wells (strip and master targets): menu, drop, Alt-drag (the drag only), marks, live tick, drop outline | `sends_and_the_pinned_strip_offer_modulate` (channel and master) — seen failing; `modulators::a_handle_dropped_on_the_pinned_fader_connects_the_volume` — seen failing |
| 7, 13, 19 — hosted-instrument words cached too loosely | `Texts` keeps a stamp: `HostedInst::instance()` (plug-in key and id, the instance's address, its state generation) and every value the project stores for it; a new stamp forgets every word (another plug-in, a reload, a sync switch that rewords a rate); the row's id carries the instance | `modulator_hosted::tests::a_plugins_words_are_forgotten_when_the_stamp_changes` (unit: there is one test synth, so a swap of plug-ins cannot be driven in the app) |
| 9 — the nesting test never dropped | it carries the handle from away, drops it on the amount row (found by its widget id), and reads the amount's *Modulate…* list: the other modulator, never the LFO itself | `modulators::a_handle_dropped_on_an_amount_nests_one_modulator_in_another` (the code was right: passes on the old code too) |
| 11, 12 — the amount row cloned the modulator each frame; not dimmed when muted | the clone moved into the closure that runs on a change; `param_row` takes `dim` (the muted modulator's) | — (no allocation counter in UI paint; the dim is a colour) |
| 14 — the instrument arm passed when nothing was drawn | asserts each view drew a modulatable parameter | `every_native_instrument_editor_offers_modulate_on_every_control` |
| 15 — Reverb's pre-delay had no row | an editor-less effect lists every continuous parameter (`take(3)` only with an editor); the test checks all of them | `every_native_effect_editor_offers_modulate_on_every_control` — seen failing |
| 16 — any lane-0 copy of a take's audio counted as a comp window | `comping::shows_take`: a playing clip is a window onto a take when it plays its source and lies over it on the timeline (same origin: that take's; slipped or nudged: each take of the source it lies over); `comp_windows` and the take lanes' lighting use it | `comping::a_windows_copy_away_from_its_takes_is_not_the_comps` — seen failing. A copy pasted *within* its takes' stretch is indistinguishable from a nudged window without a per-window link (a schema change); it counts as the comp's |
| 17 — desk controls ignored double-click, arrows, Ctrl+wheel while mapping | `knob_input`/`fader_input` take `drags`: only the drag is the depth's; the sheet rows likewise | `modulators::a_desk_fader_double_clicked_while_a_modulator_maps_goes_to_unity` — seen failing |

## integration-1

| Finding(s) | Fixed how | Test |
|---|---|---|
| 9 (HIGH) — a split comp window made a fake seam (+3 dB) | `comp::is_seam` — one predicate for the snapshot, `SetCompSeam` and the arrangement — refuses one stretch of one file cut in two; a split, an unlinked paste and a copy (`copy_of`) leave the seam crossfade behind (a linked paste keeps it: a shared field) | engine `comp_seams::a_split_window_is_no_seam` (bit-identical to the unsplit render) — seen failing; core `comp::tests::a_split_window_is_no_seam` |
| 0 — synced modulation effects led the audio by the slot's latency | `EffectUnit::late` set for every insert by `scope_delays` (the strip's compensation plus the inserts before it; the master's likewise); each insert hears `SongPosition::earlier(late)` (folded into the loop; unchanged at 0) | `fx_tremor::a_synced_gate_behind_a_latent_insert_follows_the_beat_it_hears` — seen failing (8 869 samples off); `a_masters_synced_gate_follows_the_compensated_beat`; `fx::position_tests` |
| 1, 5, 6, 18 (and sound-2 6, 19) — Chorda's tuning knobs still took automation and modulation | one rule, `InstrumentKind::modulatable` (automatable, with a range): `frame::wire` gives such a key no clip value, no rings, no menu (`Param::offers` → plain); `wired` ignores it; the source-chip drop and highlight, `connect_modulation` (says why), the matrix's targets, `CantusApp::modulatable` (a handle, Alt-drag, *New modulator*, `connect`), the swap's carried connections | `modulate_everywhere::chordas_tuning_keys_take_no_automation_nor_modulation` — seen failing; `frame::tests::chordas_tuning_knob_ignores_an_inert_clip_and_a_swap_brings_it_no_source` |
| 8 — the top bar's snap spoke for the piano roll under the pitch and automation editors | `Editor::{Pitch, Automation}` from the selected clip: the pitch editor's quantise grid (no snap to switch, said in the hint), the arrangement's grid and snap for automation; their own glyph and name | `topbar::the_snap_speaks_for_the_pitch_and_the_automation_editor` |
| 10 — kept windows stayed in link groups; the switch back was refused | a window a comp keeps leaves its link group (`kept`) | `comping::a_linked_window_kept_by_a_comp_comes_back_after_its_group_changed` — seen failing |
| 11 — *Delete unused takes* removed a take a slipped or moved window plays | `in_comp` (what the delete keeps): a clip of the same source whose stretch of the source overlaps the take's, on lane 0 or kept | `comping::a_take_a_moved_window_still_plays_is_not_unused` — seen failing |
| 12 — a comp switch played edited windows without their edits | `Snapshot::kept_pitch` (the kept windows' edits and files); the sample cache keeps their renders | `pitch_editor::a_comp_switched_away_and_back_plays_its_edits_at_once` — seen failing |
| 4, 13 — the exported Speculum's newest frame a loop early | `publish_transport` takes the last frame's beat, read at `n − 1` | `transport::the_last_frame_of_a_block_ending_at_the_loop_end_is_before_the_wrap` — seen failing |
| 7 — the fx lint missed new files | walks `views/fx/`; `pitch_lane.rs` listed | `fx_lint` |
| 3 — dead `audioclips.piano_roll` | removed (EN, ES) | i18n |
| 2 — ADR 0051 §4 | annotated as amended by ADR 0054 (status, table, test summary) | — |
| 14 (+ the coordinator's note) — the manual's known issues | hosted parameters are automated, automation is recorded (no latch/touch yet), comp seams crossfade — EN and ES | — |
| 15, 16 — exemplar.md, ADR 0023 | schema 30 (`migrate_29_to_30`, its test), DRSKit's counts after review 1 (548 zones, 170 layers; the drum count is not restated: it cannot be checked without the archive) | — |
| 17 — 490 MB on the pack's page | about 470 MB (EN, ES); ADR 0042's table and sound.md mark 490 as the size before review 1 | — |
| 18 — chorda.md | every cell but the tuning's takes a source | — |
| 19 — Chorda's Spanish short labels | ESTIR., EXTINC., APAG., INCL., DIN. | — |

## sound-2 — Chorda, Exemplar, the drum pack

| Finding(s) | Fixed how | Test |
|---|---|---|
| 4 — the bench measured a silent Chorda | `Rig::with_workers` designs each Chorda track's keyboard before its units are made | `bench::the_rig_measures_a_chorda_that_sounds_its_voices` (8 voices) — seen failing (0) |
| 1, 5, 10 — `ensure` could spin forever | the design runs under `catch_unwind` (a failure is refused and the designer freed); `ensure` designs on its own thread when the designer could not start, the design was refused or failed, or after two minutes | `inst::chorda::tests::an_export_ends_when_the_designers_thread_cannot_start` — seen failing (a minute's timeout), `…when_a_design_fails_on_the_designers_thread` |
| 0, 8 — a keyboard never reached ringing strings while anything played | a sympathetic string waiting for a layout fades its output over 256 samples, then is silenced and takes it (woken again if driven); a key struck while it rings for an older keyboard fades its strings over 128 samples, then the strike lands on the new ones (a note-off meanwhile lands it released; all-notes-off too) — never a cut | dsp `a_keyboard_landing_while_the_piano_plays_reaches_the_ringing_strings` — seen failing; `a_new_keyboard_never_cuts_the_strings_ringing` still green |
| 15 — a fresh unit played A4 440 until its tuning arrived | a unit that has struck nothing drops the reference keyboard when its own tuning is not designed yet: silent, with the caption (ADR 0059 §6) | `inst::chorda::tests::a_fresh_unit_waits_silent_for_its_own_tuning` — seen failing |
| 2, 14, 16 — "designing the strings: 0 %" forever for a refused tuning | `chorda::refused`; the caption says the session keeps this tuning (`chorda.no_room`, EN/ES) | `views::inst::chorda::tests::a_tuning_this_session_cannot_design_says_so_never_zero_percent` (through `refuse_for_tests`) |
| 9 — the 1-cent test skipped the corners | A4 466 / 200 % / 150 % at 44.1 and 96 kHz, A4 415 / 0 % / 50 % at 44.1 kHz: worst 0.488, 0.497, 0.483 cents | `every_key_rings_within_a_cent_at_every_rate_and_tuning` |
| 3 — lanes hashed only the file | `RrLanes`' sound is the file, the generator and the start/end | core `generated_drums_and_slices_of_one_file_take_their_own_turns` — seen failing |
| 17 — live notes counted per key | in order, a live note's turn is counted per lane (its zones' sources and stretches, a fixed open-addressed table; per key past 128 lanes) | dsp `live_notes_on_one_drums_keys_take_turns_together` — seen failing |
| 7, 13, 20 — kit rows kept the old language | the rack rebuilds its axes and readouts when the language changes; the piano roll's axis key holds the language | `pack_drums::kit_rows_follow_a_language_switch_in_the_rack_and_the_piano_roll` — seen failing |
| 11 — the space label was cut | the details' label column is as wide as its longest label (110 pt at least) | `samples_content::space_tests::…` (with 21) |
| 12 — the no-space notice lost its figures | the figures first ("Needs 7.6 GB free, the disk has 3.1 GB: …"); the whole notice in the help line on hover | — (a hint, drawn) |
| 21 — the pack page's error in English bytes | `install_error_text`: the no-space error in the interface's language and in sizes, on the pack's page and the starter card | `the_space_label_fits_and_the_no_space_error_reads_in_words` |
| 18 — articulations on GM2/GS keys 82–87 | moved to 21–26 (two digits: the built SFZ keeps its pinned size); the test reserves GM2's 27–87; manual (EN/ES), ADR 0042, sound.md | `kit_build::the_bundled_kit_plays_general_midis_keys_with_their_own_drums_only` |

## Found on the way, not fixed (not this round's)

- The inspector with two modulators on a track reports two egui id clashes (`A1A6`,
  `C807`) — present on the base too (checked); not in the modulator section's ids. Left for
  the integrator to trace.
- A copy pasted inside its takes' stretch counts as a comp window (see finding 16 above).

## Files touched

`cantus-host` (`unit.rs`, `session.rs`, `session_inst.rs`), `cantus-engine` (automation,
modulator, processor_{auto,snap}, effects, graph/{strip_run,master_run,track}, fx/mod,
pitch_attach, inst/chorda, tests), `cantus-core` (comp, clipboard, snapshot, inst/{mod,
sampler}, tests), `cantus-dsp` (chorda_{board,voice,engine}, exemplar{,_engine}, tests),
`cantus-plugins` (instance, tests/transport), `cantus-bench` (rig, tests), `cantus-content`
(manifest, tests/kit_build), `cantus-ui` (many views; locales instruments, automation,
modulators, topbar, samples, audioclips, inst/chorda), docs (ADR 0023, 0042, 0051, 0057,
0058, 0059, manual EN/ES, exemplar.md, chorda.md, sound.md).
