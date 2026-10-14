# Round 26 — builder *mixer*: round 25's small things

**Branch:** `r26/mixer` (base a3b6355, the contract), worktree `F:\projects\daw-r26\mixer`.
**ADRs amended:** 0024 (§15, the one text field), 0043 (§2 amended, §8–§10), 0044
(§11–§13), 0046 (base-theme swatches, the `text.selection` token). No schema change, no new `Command`, no golden hash touched,
no undo-fuzz arm needed (every mutation is an existing command: `SetStripSend`, the route
menu's existing batches).

## Done

| # | Item | How | Test that holds it (seen failing first) |
|---|---|---|---|
| 1 | An armed route lets go when the desk is hidden; a selected cable stays | The desk notes the egui pass it was drawn on (`MixerView::shown_pass`); a gap when it is next drawn is the clean "the desk was hidden" signal (its pane closed, any tab over it). `drag::desk_was_hidden` then lets go of an armed route, a press not yet a drag and anything carried; a button held through the hide is *stale* and no source takes it up again until every button is up. The selected cable stays | `mixer_routing::an_armed_route_lets_go_when_the_desk_is_hidden_and_a_selected_cable_stays` (failed: still armed); `what_the_desk_carried_lets_go_when_it_is_hidden` — cable, press, strip reorder (failed on the press: `cmd.add_send`) |
| 2 | Typing a level in the route menu | *Level ▸ Type a level…*: the menu becomes the kit's numeric entry where it was — the route's band, a lit well, the level prefilled and selected, a caption with range and keys. The field holds the keyboard while open (a click on its panel never hands keys to the app's shortcuts). Enter = one undo step; Enter on the untouched prefill = nothing. Only decibels read (a number, "dB" or nothing, decimal comma or point, either minus; "50%", "-6k", "-6 Hz" are no level); > 0 dB clamps to 0 dB; −∞ / ≤ −96 dB = silence; words keep the field (selected again, the caption in the danger colour); Esc or a click outside cancels. *Changed by the third review (ADR 0024 §15, ADR 0044 §12): Esc cancels; a click outside sets what was typed (words that are not a level set nothing).* `route_menu::open_level` for tools | `mixer_routing::a_level_typed_in_the_route_menu_sets_the_send_in_one_step` (failed: no row; then on the prefill at 0.3), `the_level_field_keeps_the_keyboard_while_it_is_open` (failed: Ctrl+Z removed the send), `route_menu::tests::a_typed_level_reads_as_decibels_clamped_to_unity` (failed: "50%" read as unity) |
| 3 | The inspector's *Add send* offers *New return ▸* | The empty Sends row and the band's + call `route_menu::open_add_send` under the inspector's owner — the desk's list, one code path; the inspector's own list removed | `mixer_routing::the_inspectors_add_send_offers_a_new_return` (failed: no menu) |
| 4 | Steady cable lanes | `views/mixer_lanes.rs` (new): the selection's lanes first, never moved by the pointer (the longest yield, faint, where the band holds no more lanes 0.6 pt apart); each hover cable at the depth nearest its span's, 1.5 pt from every overlapping lit run, 4 pt from every lit cable into its port, nested where the band has the depth (else only separated, else faint). Marks that would meet left out in priority order; each lit shown mark a grab area of its own, off the ports. The hit test takes the nearest; layers break ties only | `mixer_lanes::tests::the_placement_keeps_every_promise_on_random_desks` (2 000 random desks, every promise at once), `mixer_desk::hovering_a_strip_never_moves_the_selections_lanes` (failed: 5.0 → 10.375; a shared run), `the_hovers_cables_are_pointable_marked_apart_and_hung_by_their_span` (failed on the previous placement: marks 1.3 pt apart), `frame_budget::a_hovered_bus_with_many_inputs_costs_no_allocation` |
| 5 | The hint bar's ~17 allocations | Traced allocation by allocation (backtraces, a debug-symbol build): the hint bar costs **0**; the hover's allocations are all egui's — its hit test, its interaction lists and (debug assertions only) its `on_widget_under_pointer` plugin hook. The budget tests hold ours at 0 against controls | `frame_budget::the_hint_bar_costs_no_allocation` (against a frame with a plain shape instead of the bar; failed under a one-`format!` mutation: 17 vs 16) and `a_hovered_desk_control_costs_only_what_egui_charges_for_a_hover` (failed under a mutation: 17 vs 16) |
| 6 | Swatches in the Look tab's base-theme menus | Each row carries its theme's preview in that theme's tokens; the menu widens so no name runs under one | `tests/appearance_look.rs::the_base_theme_menus_carry_each_themes_swatch` (failed: no swatches) |
| 7 | Spanish SALIDA collision | The thick line is *salida* as everywhere in Spanish (options, hover, route menu); Wide's row, which shows where each strip goes, is *Destino* | `mixer_desk::the_legend_words_never_read_the_same` — every legend word distinct, the strip's words apart, each line's word inside its kind's name (failed on SALIDA, then on DESTINO) |
| 8 | The legend at the band's least | The band (ports, cables, legend cell) is **pinned to the desk's foot** while the desk is scrolled away from it, the strips scrolling under its edge — so the routing is on screen at 1280 × 720 in Compact and at 200 %. The pinned band stops at the scrollbar, takes the wheel as the band, lets nothing hidden be grabbed through it; its height handle is inert and dimmed (with a hint) while the desk holds the band at its least. *Changed by the third review (finding 6, ADR 0043 §8): the handle is never inert — a drag down acts at once, a drag up stops at what fits (with a hint).* Below 40 pt a compact key: three samples side by side under the band's handle, an output twice a send's weight, a key's dash and mark; ROUTING aligned with the column | `mixer_routing::the_cable_legend_shows_when_the_band_is_at_its_least` — whole window, Compact and Wide, 100/200 %, en/es, the cell inside the visible rows and the window (failed without the pin: cell 683–703 under a view ending at 687) |

## Review fixes (adversarial review, 13 findings)

1 press cleared and stale-button guard (item 1) · 2 the carried-drag and reorder cases
tested · 3 the level field keeps focus, Enter read only from the focused field · 4 dB only
· 5 Enter on the prefill sets nothing · 6–7 lanes redesigned (reserved selection lanes,
hover cables fitted, layers, sensors) · 8 the hint-bar test against a control · 9 a kit
`text.selection` token in all twelve themes (text ≥ 4.5:1, 7:1 high contrast; held by the
floor and the pairings) applied to egui's visuals — the typed value reads on Paper · 10 the
Spanish legend names the kind *salida*, the Wide row says *Destino* · 11 the band pinned to
the desk's foot · 12 the compact key a row of distinct samples · 13 ROUTING aligned.
Each with a test seen failing first (listed above and in the ADRs).

## Second review (15 findings, on the fixes above)

1–6 the placement rethought (`mixer_lanes`, above; the property over random desks holds
(a) the selection's lanes unmoved by the hover, (b) every lit cable picked on itself and a
faint one wherever nothing lies exactly on it, (c) marks 2 pt apart each with its own
grab area off the ports, (d) a lone hover cable at its span's depth (and its pill, on the
desk), (e) no shared run at any band height — what does not fit yields, faint, (f) no
needless crossing, and the share that yields; (g) a 32-input bus hovered: no allocation,
+0.06 ms release) · 7 the ratio handle never grabbed through the pinned band · 8 the wheel
reads the pinned band · 9 the band stops at the scrollbar · 10 the forced handle inert,
dimmed, with its reason · 11 Esc on the level field keeps the right-clicked cable · 12 the
four views' text fields with egui's frame are the kit's (lit well, frameless edit), so no
focus outline changes colour · 13 `theme::apply` hands egui its own tokens (2 000-apply race
test, 50 runs clean) · 14 the selection fill kept off the well under every knob (and a
pairing) · 15 the fill named and grouped as a surface, *Text selection (in a field)* /
*Selección de texto (en un campo)*. Each with a test seen failing first.

## Third review (12 findings)

Merged `r26/integration` first (twice: again after the sampler's round, for the release
builder's `font_coverage` note — the '▸' in `cables.hint.band_short` was already gone
here: "the desk's menu" / "el menú de la mesa"; `font_coverage` passes after the merge).

| # | Finding | Fix | Held by (seen failing / mutation) |
|---|---|---|---|
| 1 | The 2-pt mark gap smaller than the marks | Marks clear by each kind's glyph reach + 1 pt (dot 2.8, chevron 4.4, key 6.0); a mark slides down its corner to the nearest clear height | the property (glyph reach set to 1 pt: marks meet) |
| 2 | Nothing held the 4-pt same-port rule (M3) | The rule replaced: a hover cable's depth must leave its mark a place; the property asserts every non-yielded hover cable shows its mark | the property (M3's form here, a depth taken without a place for the mark: "hover … shows no mark"); `mixer_desk::two_short_cables_into_one_port_both_show_their_marks_apart` (fails under the same mutation) |
| 3 | The selection's marks lost without a bound | The selection's marks placed first (the selected one always); the property bounds those left out at 20 % — measured 7.7 % (681 of 8 804) | the property |
| 4 | No fan-in in the generator | A third of the desks have a bus fed by 8–32 strips, hovered; ADR 0043's table now gives bus and no-bus yields | the property's tallies |
| 5 | The selected cable's tail sensor over other grab areas | Every grab area (the tail's too) holds its mark, keeps clear of every other shown glyph and area; off the ports except the selected cable's | the property |
| 6 | The forced band handle traps the band | The fit keeps the most band that fits (`Drop::ShorterBand`); the handle is never inert: a drag starts from the band as shown, down acts at once, up stops at what fits; dots dimmed, hint `cables.hint.band_short` | `mixer_desk::the_band_handle_never_traps_the_band` (failed: inert, 20 pt), `mixer_layout::tests` (47 / 28 pt) |
| 7–12 | Text fields: the margin ignored under `Frame::NONE`, click-away not committing, Esc not cancelling, two focus rings, a DragValue, no select-all | **`kit::field::Field`** — one field for the whole app (ADR 0024 §15): the well with its inset as the frame's inner margin, one focus indication, select-all on open, Enter commits, Esc cancels and closes, a click away commits, closes and lets the keyboard go, `field::active` holds the app's shortcuts and the typing piano. Used by every text field: the layout name, track/clip/pad/strip renames, the level field, the inspector's name, the link picker and clip browser filters, the device picker and panel searches, the export and settings fields, an effect's typed value and text fields, a preset's name, the knob's exact entry (the DragValue gone). The lint refuses `TextEdit`/`DragValue` outside `kit/field.rs` | `kit::field::tests` (6), `tests/text_fields.rs` (Enter, Esc, click away, shortcuts held — track, clip, inspector name, pad, strip — and the layout dialog; all failing before the move), `appearance_look` (the lint; failing until the fields moved), `widgets::knob::tests::a_right_click_types_an_exact_value_in_the_kits_field` |

Found by looking at the pictures: an effect editor's typed value opened *under* its cell
(the cell's face is painted after it; so it was before the kit field too), at 100 % in a
200 % editor. It now opens in a foreground area, and `Field::scale` takes a device
editor's own scale — `views::fx::controls::tests::a_typed_value_opens_over_its_cell_at_the_editors_scale`
(failed: the cell's "125ms" painted last; with the scale dropped, rows [16, 16]).

## Final review (the whole integration: fields 1–6, visual 1, 2, 4, 5, 6, 8, reach 2, the chosen tile)

Merged `r26/integration` before and during (clean both times).

| Finding | Fix | Held by (each seen failing first) |
|---|---|---|
| fields 3 (high) | `workspace_widgets::text_field` takes the field's id; every caller salts it (a routing row's `(input, channel)`, an export tag's key, the theme search/name, the samples search, the proxy). `kit::field` refuses two fields with one id in a pass (debug builds) | `workspace_widgets::tests::sibling_rows_each_have_a_field_of_their_own` (["Monitors", "Out 2Monitors"]), `kit::field::tests::two_fields_with_one_id_are_refused` |
| fields 1, 2, 6 | `field::Out::edited` (the words differ from those the session opened with, spaces aside) and `Out::enter`; an untouched knob entry, effect value or pad rename sets nothing; the pad's name is kept trimmed | `knob::tests::an_untouched_exact_entry_sets_nothing` (1200), `controls::tests::an_untouched_typed_value_sets_nothing` (Some(1200)), `text_fields::an_untouched_rename_runs_no_command_and_names_are_trimmed` (pad: `cmd.set_pad_name`) |
| fields 4 | The preset name box: only Enter on a name saves (a preset is a file); a click elsewhere, Esc or Enter on nothing closes it, and a save closes it whatever its outcome | `frame::tests::the_preset_name_box_closes_and_never_saves_an_empty_name` (stayed open), `only_enter_on_a_name_saves_a_preset` |
| fields 5 | The export dialog's Esc waits while a tag is typed | `text_fields::esc_in_an_export_tag_cancels_the_tag_and_keeps_the_dialog` (dialog closed) |
| reach 2 | `Shell::Running` runs `CantusApp`'s own `eframe::App::ui` — one path, the taskbar's poll included | `boot_shell::the_running_shell_runs_the_apps_whole_frame` (never read) |
| visual 1 (high) | Where the master, the left dock, the right dock and one strip do not fit side by side, the right dock scrolls with the middle (after its last strip) instead of lying over the master | `mixer_desk::the_right_dock_never_covers_the_master` (300–800 pt × every width; failed at 300 pt, Extra large) |
| visual 2 | The clip menu's colour row is part of the menu: its panel, the items' inset, the sections meeting | `final_look::the_clip_menus_colour_row_is_inside_the_menu` |
| visual 4 | The layout's name dialog is the kit's dialog, centred: panel and title bar (`layout.dialog_title`, no "…"), the kit's buttons, Save lit | `final_look::the_layout_name_dialog_is_a_kit_dialog_in_the_middle` |
| visual 5 | A hint with no value wider than its line takes both lines of the hint bar, wrapped (the project's name gives way while it shows); `cables.hint.band_short` says its advice first | `final_look::a_long_hint_reads_whole_at_1280_by_720` (en, es) |
| visual 6 | A strip rename opens with the name the strip shows (its track's), selected, as a framed popover under the number row; untouched, the strip keeps following its track | `mixer_desk::a_strip_rename_opens_with_the_name_it_shows_clear_of_the_numbers` |
| visual 8 | The Look tab's names column is as wide as the longest name in the language; one pair a row where two do not fit | `final_look::the_colours_with_a_meaning_are_named_whole` ("LED de encendido /…") |
| last gap: every hint whole | The idle hints (desk, arrangement, rack, each piano-roll tool) show their essentials (`<key>.short`, both languages) on one line under the project's name, the whole as the hint panel's tooltip; a hover hint wider than its line — name, or name and value — wraps over both lines; the clip's hover says its essentials (`arrangement.clip_hint`), the legend's and the Spanish device chip's words shortened. ADR 0024 §9 | `final_look::every_hint_reads_whole_at_1280_by_720` — 1280 × 720 at 100 % and 200 %, en/es, every idle hint and a pointer sweep (failed: 1 503 cut, 12 distinct) |
| last pass 1–4 | The piano roll's grid says its tool's essentials (`Tool::short_hint_key`), the whole in the idle hint and its tooltip; the capability icons are painted for every hint, wrapped ones too (registered as `hint_icon`); the pinned strip's captions stop short of their values (`VOL` where `VOLUMEN` does not fit); `rack.hint.short` in Spanish: "Clic en un paso: encender · Clic derecho: apagar" | `every_hint_reads_whole_at_1280_by_720` now sweeps once per piano-roll tool, the rack and the arrangement painting, and asserts the grid was pointed at (failed: the whole Draw list cut); `a_wrapped_hint_keeps_its_capability_icons` (0 icons); `the_pinned_strip_never_runs_a_caption_into_its_value` (1 160–1 420 pt, en/es; failed: "+0.0dB" into "VOLUME") |
| the chosen tile | The clip browser's paint source says `links.browser.chosen` when pointed at | `final_look::the_chosen_tile_says_it_is_the_paint_source` |

Pictures (`F:\projects\daw-r26\shots\mixer\final`, `…\fields`): the Extra-large desk at
200 % in 1280 × 720 (Night, Slate, Paper — the master whole, the returns after the middle),
the clip menu (100 %, 200 % Paper es), the layout dialog, the strip rename, the band and
chosen-tile hints (`shoot_fields`, 75/100/200 %, en/es, Night/Paper), the Look tab in
Spanish — looked at.

## Measurements

Hovered desk control, worst of 30 frames, 64 strips, Wide (`frame_budget`):

| Build | Desk at rest | Fader hovered | Bare egui frame, same shape | Cable hovered |
|---|---|---|---|---|
| debug assertions (`cargo test`) | 20 | 36 (+16) | 20 → 36 (+16) | 37 (+1: the band is one more widget under the pointer) |
| release | 20 | **27 (+7)** | 20 → 27 (+7) | **27** |

Hint bar alone (worst frame, release and debug): a plain shape in its place 16, the idle
bar 16, a hovered hint 16; button held 16, a live value with its pill 17 (egui regrowing
its frame's shape list for the pill's shapes, `GraphicLayers::drain`, traced). What is
ours: **0 → 0** (round 25's "~17 for the hint bar" was egui's own hover price).
Release timing on a quiet machine: mixer64 desk p50 1.32–1.44 ms, wired *All* p50
1.62–1.69 ms (round 25: 1.68), mixer16 window 1.05 ms; a 32-input bus hovered 0.83 ms
against 0.78 ms for a plain strip (fastest frames). Allocations hovering that bus: 27 =
27 (release). Third review: the bus hovered costs +0.17–0.19 ms over a plain strip on a
quiet machine (0.80 / 0.63 ms, 0.87 / 0.69 ms; the search for a depth that leaves the
mark a place — 5 inputs lit with marks, 27 related), under the test's 0.3 ms bound;
still 27 = 27 allocations. Lanes over 2 000 random desks (hover cables yielded / nesting
given up): no bus, band < 40: 26 % / 10 %; ≥ 40: 0.2 % / 9 %; a bus hovered, < 40: 87 % /
5 %; ≥ 40: 55 % / 26 %.

## Tests

Final review, after merging `r26/integration` again: **1 238 passed, 0 failed, 24
ignored**; clippy `-D warnings` and `fmt --check` clean; `frame_budget` in release: 15
passed, and the bus timing (with two other builders compiling, every frame ~1.5–2× slower)
went over its 0.3 ms bound once (+0.41 ms), then passed on three reruns at +0.16 ms.

`cargo test -p cantus-core -p cantus-project -p cantus-ui --no-fail-fast` (third review,
after merging `r26/integration`): **1 216 passed, 0 failed, 24 ignored**; `font_coverage`
3 passed. Second review: 1 101 passed, 0 failed, 22 ignored. `cargo test --release -p cantus-ui --test frame_budget --
--include-ignored`: 16 passed. `cargo clippy --workspace --all-targets -- -D warnings`:
clean. `cargo fmt --all`: applied.

Changed: `mixer_desk::esc_on_the_desk_cancels_its_own_things_and_clears_nothing_else`
re-selects strip 2 before clicking its cable (the strip drag it had just cancelled had
selected strip 3; the cable is the selection's again, in its lane); the `frame_budget`
doc on the cable's "+1".

## Pictures

`cargo run --release -p cantus-ui --example shoot_mixer -- F:/projects/daw-r26/shots/mixer <filter>`,
new variants `-level` (`-es`, `-75`, `-200`), `-insp-send` (`-es`), `-look-menu` (`-es`,
`-75`, `-200`), `-clip-rename` (`-200`), `-layout-name` (`-es`) — the other views' text
fields, the kit's now. The pinned band at 200 % stops at the scrollbar, its forced handle
dimmed. Looked at (Cantus Night and Paper, 1280 × 720 and 1920 × 1080, en/es,
75/100/200 %): the default Compact desk at 1280 × 720 now shows ROUTING, its key and the
cables pinned at the foot (round 25's picture ended at FROM SELECTED); at 200 % the band
sits under NAME, pinned; the compact key reads as three different lines (thick, thin, key
mark), clear of the handle; the typed value reads on Paper and Night (the kit's selection
fill); the base-theme swatches read at 3×; the inspector's list opens with *New return ▸*.
Found on the way: shoot_mixer's Paper pictures were Cantus Night (the window applies its
saved appearance) — it now names the picture's theme in the appearance.

Third review pictures: `cargo run --release -p cantus-ui --example shoot_fields --
F:/projects/daw-r26/shots/mixer/fields` (new) — every kind of field open, its words
selected: track, clip, the inspector's name, pad, strip, the layout's name, the knob's
exact entry, an effect's typed value, and the band handle's hint while the desk holds the
band; Night and Paper, en/es, 75/100/200 % (108 PNGs). Plus `shoot_links kind=picker-filter`
(100 % Night, 200 % Paper es, 75 % Night es), `shoot_picker search panel`, `shoot_export`
(200 % es, 75 % en). Looked at: the wells lit once, the words inset and selected (the
selection fill reads on Paper and Night), nothing clipped but a clip rename running past
its pane's edge with its clip; the typed effect value was hidden — fixed above. The band
hint's words are elided in the hint bar at 1280 × 720, as the bar's other long hints are.

## Files outside my area

- `views/inspector_menus.rs`: removed `MenuId::AddSend`, `Choice::AddSend` and their arms
  (dead once the inspector uses the route menu's list).
- `kit/tokens.rs`, `theme_family.rs`, `theme.rs`: the `surface.text_selection` token (a
  field per theme, the derivation for old theme files, the knob shift, the floor, two
  pairings) and egui's selection visuals from it (review findings, asked by the
  coordinator). Its names are in `locales/areas/look.*` (mine).
- `app.rs` (the layout's name dialog), `views/arrangement.rs` (a track's and a clip's
  rename), `views/channel_rack.rs` (a channel's rename): each field the kit's — a lit well
  under a frameless edit (second review, 12). A few lines each.
- `kit/cable.rs` (mine) and `kit/gallery.rs`: `CableLook::mark` (a band may leave a mark
  out); the gallery passes `true`.
- `locales/en.txt` / `es.txt` keep `mixer.send_none`, now unused (not mine to edit).
- New test file `tests/appearance_look.rs` (my area's `appearance*.rs`).
- Third review: `kit/field.rs` (new), `design.rs` (`paint_focus_ring` leaves text edits
  alone), and every view with a text field — `app.rs` (the layout's name, the shortcuts
  gate), `arrangement.rs`, `channel_rack.rs`, `inspector_clip_link.rs`,
  `link_picker_paint.rs`, `picker_layout.rs`, `picker_panel.rs`, `workspace_widgets.rs`,
  `export_dialog.rs` (its typing gate), `fx/controls.rs`, `fx/frame.rs`,
  `widgets/knob.rs` (the DragValue replaced); new `tests/text_fields.rs`,
  `examples/shoot_fields.rs`.

## Left

- Upstream only: egui's own price for a hover (7 allocations in release: its hit test's
  and interaction's fresh `Vec`s each frame) — measured equal to a bare egui frame of the
  same shape by `a_hovered_desk_control_costs_only_what_egui_charges_for_a_hover`.

## Notes for the integrator

- New public API: `route_menu::{open_level, typed_level, level_panel}`,
  `MixerView::{cable_key, rows_seen}` / `CableKey` (with `handle`), `Tok::TextSelect` /
  `Tokens::text_select` (key `surface.text_selection`); fields
  `Drawn::{laned, mark, yielded, relaxed}`, `Geo::{strips, pinned, inner_right}`,
  `CableLook::mark`. A test that builds `Drawn`, `Tokens` or `CableLook` by hand needs the
  new fields. New module `views/mixer_lanes.rs` (declared in `mixer.rs`).
- A theme file written by this build carries `text.selection`; an older build ignores it,
  an older file derives it.
- Third review API: `kit::field::{Field, Event, Out, active, edit_id}` (`Field::scale`),
  `ArrangementView::{begin_rename_track, renaming_track, renaming_clip}`,
  `ChannelRackView::{begin_rename_pad, renaming_pad}`, `MixerView::renaming`,
  `Drawn::mark_at`, `CableLook::mark: Option<Pos2>` and `CableLook::related`; the locale
  key `cables.hint.band_short`.
- Shared files I edited: `inspector_menus.rs`, the three theme files, `app.rs`,
  `arrangement.rs`, `channel_rack.rs` (the text fields), `kit/gallery.rs` (one field).
