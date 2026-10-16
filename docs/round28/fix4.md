# Round 28 — fix4: what fix3 left, and the review of fix3

Branch `r28/fix4` (worktree `F:\projects\daw-r28\fix2`), from `r28/integration` at
`9031e49`. The three items fix3 left open, then every confirmed finding of
`reviews/fix3-1.json` (25, deduplicated: several lenses found the same Chorda and drum-key
defects). "Seen failing" means the test was run against the old code and failed; a test
that names something new and could not compile against the old code says so.

**Schema:** 32 (`Clip::comp`). **Golden hashes / DSP_VERSION:** none moved (no golden render
loops behind a latent insert; every path at defaults is untouched). **Pinned size:** the
DRSKit pack is 470 879 297 bytes installed (was 470 879 054: three-digit keys).

## The three items fix3 left

### 1. A copy pasted inside its takes' stretch counted as a comp window

**What.** Comp windows were recognised by lying over their takes, so a copy pasted at its
take's place was taken for a window (a switch moved it, a new comp copied it).

**How.** A comp window says it is one: `Clip::comp` (schema 32), each placement's own (a
link group does not share it). Set by a swipe and a promote, and by a recording laid over
takes (the loop's last pass, a stacked take — which also makes alternate comps work after a
plain stacked recording: before, the new take stayed in every comp); kept by a split, a
seam move, trims, slips, nudges, comp switches and copies (`SetTrackComps` refuses a kept
window without it), and by a glue that takes a window in; cleared by every copy and paste
(linked or not), a move to another track, a recording stacking a window onto a take lane,
and flatten. `Command::SetClipComp` (exact inverse, `cmd.set_clip_comp` EN/ES).
`comp_windows` is the flagged playing-lane clips; which take a window shows (its number,
the lanes' lighting) is the take whose stretch of the same content it plays — a loop's
last pass no longer lights its earlier passes (fix3 lit them all). `migrate_31_to_32` (raw
JSON) flags exactly what schema 31 counted, and every kept window. ADR 0058 amended,
manual EN/ES (*What belongs to the comp*).

**Tests.** `comping::a_copy_pasted_over_its_take_is_not_the_comps` (the real copy and
paste, a switch away and back, the lighting) — seen failing; `edits_keep_a_comp_window_said_right`
(split, glue, Alt-drag copy and move to another track, flatten, all undone exactly);
`rec_pass::a_pass_laid_over_takes_is_the_comps_window` (loop, first recording, stack; comp
A comes back with the stacked take whole) — names the new field; core
`comp::tests::{a_kept_window_is_a_comp_window, a_split_keeps_a_window_in_its_comp_and_a_copy_or_paste_is_none}`;
the undo fuzz's comp arm grows two cases (flag set/cleared/refused; a window split, copied
and linked-copied, checking the half is the comp's and the copies are not — additive,
coverage asserted); project `schema_32_flags_the_windows_schema_31_counted` (hand-built)
and `schema_32_keeps_old_comps_exactly_and_a_pasted_copy_out` (v28–v31 equal schema 31's
rule clip by clip; v32 holds a pasted copy over a third take); `fixtures/v32.cantus`.

### 2. egui id clashes in the inspector (A1A6, C807)

**What / how.** Traced by drawing the inspector headless and reading egui's clash reports:
a modulator's switches (*Sync*, *Restart*, *Polarity*, *Smooth*) used
`("insp-led", key, button)` — the same id for every modulator. `Sheet::led_row_of` salts
it by its owner; the modulator's switches pass the modulator's id.

**Test.** `id_clash.rs` fails on any egui clash report: the inspector with four track and
two strip modulators (and the master's), a channel with three modulated sends (its
inspector, each return's, the desk), and every native instrument with four modulated
knobs (each editor view and the inspector) — the first seen failing (A1A6, C807, C45E).

### 3. The real-kit install

`CANTUS_DRSKIT_ARCHIVE=…\DRSKit2_1.zip`: the ignored real-archive test installs through the
bundled manifest (SHA-256 verified, 496 files, peak −4.87 dBFS); after fix3's keys it
matched the pinned 470 879 054 bytes. After this round's key move (below) it measures
470 879 297 — pinned in `content.json`, ADR 0042 and sound.md; `pack_drums`' real-kit groove
test passes on it (−26.0 LUFS, −8.2 dBTP, 117 MiB of heads). Installed under
`F:\projects\daw-r28\tmp` and removed; the zip and `drsmix` kept.

## The review of fix3 (`reviews/fix3-1.json`)

| Finding(s) | Fixed how | Test |
|---|---|---|
| 0, 3, 7, 23 — a fresh Chorda whose tuning is refused stayed silent for the session while the caption said "keeping this one" | `configure` drops the reference keyboard only when the tuning `can_arrive` (lock-free: flags for a full registry and a designer that could not start, a fixed list of failed designs, written under the designer's lock); `follow_design` takes the reference keyboard back when it will not come | `inst::chorda::tests::a_fresh_unit_whose_tuning_is_refused_keeps_playing` (refused before, refused after, no designer) — seen failing |
| 6, 13 — designer fault tests shared global flags | the designer tests run one at a time behind a mutex, a drop guard resets every fault, the spawn-failure test waits for an idle designer and asserts the failed-spawn count rose; the design-failure test asserts the refusal happened on the designer | the tests themselves |
| 14 — the retune and sympathetic fade paths had no block-size test | `Ev::Retune` in the DSP test performer | `a_keyboard_landing_mid_phrase_gives_the_same_bits_at_any_split` (blocks 1, 37, 64, 512, threads; asserts the paths ran) — passes (no defect) |
| 2 — a late insert heard the loop wrap up to a pass late, block-size dependent | the pass is cut at each insert's own wrap (distinct latenesses kept by `scope_delays` in a preallocated slice, sorted in place) | `fx_tremor::a_late_synced_gate_wraps_with_its_audio_at_any_block_size` (loop of 4⅓ beats, from the start and entered from before, four block patterns, bit equality) — seen failing |
| 4 — `earlier` folded on the first entry, once only | `Transport::since_wrap`, `SongPosition::wrap`: folds only after a wrap, as many passes back as it reaches, in whole samples | `position_tests::earlier_folds_only_after_a_wrap_and_never_moves_a_stopped_playhead` |
| 5 — a stopped position was moved back | `earlier` returns the position unchanged while stopped | same test (parked on bar 5 and at 0) |
| 15 — the swap-in-place resend untested stopped and for instruments | — (the code was right) | `hosted_queue::a_unit_swapped_in_place_while_stopped_hears_the_held_value`, `a_hosted_instrument_swapped_in_place_hears_the_held_value_playing_and_stopped` — each seen failing with its resend line removed |
| 1, 10, 24 — articulations on 21–26, which XG and GS use | moved to 101–106; the test refuses 13–87 for anything but the key's own GM drum; ADR 0042, manual EN/ES, sound.md say only what holds | `kit_build::the_bundled_kit_plays_general_midis_keys_with_their_own_drums_only` — seen failing on 21; the real archive re-pinned |
| 8, 18 — the top bar's pitch grid ignored *Quantise to <track>* | `set_grid` lets the track target go (as the pitch bar's menu); the field names the target track and checks no grid meanwhile | `topbar::the_pitch_editors_track_target_shows_and_a_grid_picked_lets_it_go` — seen failing |
| 9 — cable pills took send modulation without showing it | outline in the modulation hue, a live tick on the foot | `mixer_desk::a_modulated_sends_pill_shows_its_modulation` — seen failing |
| 11 — the manual's *Everywhere* bullet read as "the amount is not modulated" | the Chorda sentence after the nesting one, EN and ES | — |
| 12 — the id clashes | item 2 above | — |
| 16 — the space-label test was tautological | `DETAIL_LABELS` and `detail_column` shared by the table and the test; every label fits the drawn column; the Spanish space label is wider than the old fixed column | `samples_content::…::the_space_label_fits_and_the_no_space_error_reads_in_words` |
| 17 — the hosted words were all re-asked every frame of a drag | the stamp holds the instance and the stepped stored values only | `modulator_hosted::tests::a_dragged_value_keeps_the_other_rows_words` (names a new helper) |
| 19 — hosted rows dropped double-click and typing while a modulator maps | built on `wired_value` (only the drag is the depth's; the live tick) | `hosted_instruments::a_hosted_row_double_clicked_while_a_modulator_maps_resets` (the test synth) — seen failing |
| 20 — the piano roll's note count kept the old language | its cache key holds the language | `pack_drums::kit_rows_follow_a_language_switch_in_the_rack_and_the_piano_roll` extended |
| 21 — the pinned fader's live mark was off the cap; the pan knob had no rings | the mark at `cap_rect`'s centre (the desk's rule); the knob takes the rings and the live dot | `inspector::pinned::tests::the_pinned_controls_show_their_modulation_as_the_desk_does` |
| 22 — Delay's ping-pong was reachable nowhere | an editor-less effect's 0/1 switches are LED rows in the inspector (`fx.switch_on`, EN/ES) | `modulate_everywhere::every_native_effect_editor_offers_modulate_on_every_control` now requires every stepped row |

ADRs 0057, 0058, 0059 and 0042 amended; manual EN/ES (comping, drum keys, the
*Everywhere* bullet).

**Found on the way, fixed:** fix3's wider label column (the Spanish *Espacio libre para
instalar*) cut the licence's words in the pack details ("…con crédito al…") —
`samples_browser::a_cc_by_pack_shows_its_credit_and_the_plain_words_before_and_after_download`
failed on the base too. The licence's words now wrap in the value column.

## Left

Nothing that is not the owner's. (A limit once listed here — a loop shorter than an
insert's lateness folded by whole passes at the current pass length — was closed by fix5:
the wraps played, at their real lengths; what stays is measured in ADR 0057.)

## The runs

`cargo fmt --all` (`--check` clean); `cargo clippy --workspace --all-targets -D warnings`
and `cargo clippy -p cantus-plugins --no-default-features --all-targets -D warnings` clean.
Tests, all passing: cantus-core 260, -project 64, -content 68 (+ the ignored real-archive
install), -dsp 689, -engine 673, -host 34 (`clap_window`'s topmost-window test failed once
in the run while another builder compiled — a desktop z-order race — and passes alone,
twice), -plugins 74, -bench 8, -ui 1 280 (+ the ignored real-kit groove test).
