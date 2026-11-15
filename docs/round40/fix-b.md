# Round 40 — `fix-b`: the mid-round review's model, keys, interface and words

Branch `r40/fix-b` from `r40/integration` 91339a2b. The findings are `tmp/review-mid.md` § fix-b.
Each was reproduced first — a test run against the code without the fix, or a shot — or
refuted with its reason; each fix is at the root, every mutation a `Command` with its exact
inverse.

## The findings

| # | Reproduced? | Cause | Fix | Test / shot |
|---|---|---|---|---|
| B-1 | Yes — the three new core tests failed without the fix (cues left where they were) | `ripple_marks::shift_marks` and the section drag's `carry` / `clear_bars` / `paste_marks` knew markers and sections, not `project.mix.cues` | `ripple_marks::cues`: a cue at or after the insert moves (`SetCue`, which keeps its place in the list), one inside deleted time goes (`RemoveCue`); `Carried::cues`: cleared where bars land, pasted with them (a copy gets fresh ids, appended); *Every track* ripple moves them with the marks | `ripple_tests::the_mixers_cues_move_with_the_time_and_go_with_deleted_time`, `ripple_bars_tests::the_bars_carry_their_cues_and_replace_the_ones_where_they_land`, `…::every_tracks_ripple_moves_the_cues_after_the_edit`; `ripple_props.rs`' 400 random songs now hold up to three cues (insert-then-delete identity; undo/redo of every kind) |
| B-2 | Yes — `merge_back_keeps_a_strip_the_user_chose…` failed (strip and insert gone) | `padout::merge` removed any unused strip a pad was heard on | removal gated on `pad_made` | that test (core) and `pad_user_strip_e2e.rs` through `assemble` |
| B-3 | Yes — same test: `kit_of_strip` named the kit for a user's strip | `kit_of_strip` matched any pad output; the strip bounce's "reaching" tracks ignored pads | `kit_of_strip` answers only for `pad_made` strips (no Split/Merge in a user strip's menu, Bounce no longer refuses it); `bounce::strip_plan` counts a kit whose pad plays into the strip | `pad_user_strip_e2e.rs`: no kit on the strip, its *Bounce* runs and the song afterwards is bit-identical, *Merge back* keeps the strip and its insert |
| B-4 | Yes — `ripple_e2e` held the processor as `_proc` | — | the rig keeps the processor: a take imported at the song's start, a bar inserted with the key, the real processor played in blocks of 256–1024 frames — the bar silent, the song after it bit-identical to the export's render of the song before; undo fuzz kinds 6 (*This track* delete) and 7 (*Every track* move) | `ripple_e2e::an_inserted_bar_moves_the_sound_by_its_length_to_the_sample`; `undo_invariants::time_edits_and_section_drags_undo_and_redo_among_the_others` |
| C-1 | Drawn as reported (`11 12 13 + 17…24 │ 14 15 │ 16`) — **refuted as a defect, for the owner to judge** | — | none. The numbers are the strips' own, as FL's insert numbers are: docked strips keep theirs (FL's dock shows 14 15 16 at the edge and the middle runs 13, 17, 18…); a ghost is where the next strip will be made and the number it will get — a click on "17" makes strip 17 there (`mixer_desk::the_desk_fills_its_width_with_the_next_empty_strips`). Numbering them "in order" would show numbers the strips will not get. | — |
| C-2 | Yes (the shots, zoomed): stubs above and below "−3.5 dB" read as a line through it; "+6.0 dB" crossed | the gap left 2.5-px stubs touching the figures | no edge line under the number and its unit (the fill's own edge shows the value there); a send's level is a value bar under the default (*Slim* keeps round 39's) | `look/inspector-slate_*-{100,150}.png`, `look/topbar-slate_*-100-es.png`; goldens below |
| C-3 | Yes — `ripple_e2e::the_time_keys_act_only_in_the_arrangement` failed without the fix (Insert over the mixer inserted a bar) | `dispatch_keys` ran `InsertTime` / `DeleteTime` wherever the pointer was | `CantusApp::time_keys_here`: the arrangement the keys' focus and on screen, the pointer over neither a layer above the workspace (an editor, a menu) nor another dock area; the menus and the palette as before | same test: Insert over the mixer or with the mixer focused inserts nothing; Ctrl+Del over an EQ's band takes the band only |
| C-4 | Yes: "1 compás insertados", "2.50 compases" | participles agreeing with a plural; `{bars:.2}` | Spanish lines reworded ("Tiempo insertado: 1 compás en 5.1, en todas las pistas"; "Estribillo: se movió a 9.1 (4 compases)"); `ripple_ui::decimal_words` (≤ 2 decimals, no trailing zeros, decimal comma in Spanish) | `ripple_ui::tests::lengths_read_as_prose_and_the_spanish_lines_agree` |
| C-5 | Yes (the strings) | — | the record tap's row in Spanish **Se graba: Sin FX · Con FX** (it was *Toma de: Antes de FX · Después de FX*; the integrated `ux_whole_words` found "Después de…" cut in 20 places, 67 pt a segment; English keeps *Take from: Before FX · After FX*); **Lit mute button / Botón M encendido**; Delay's held time "is more than the delay holds" / "supera el máximo del retardo"; the ripple key's tip names its keys first, then what it does when on; manual es *dos flechas* | `pads/insp-record-tap-*-{100,150}-es.png`; `ux_whole_words` green |
| C-6 | Yes (`settings-*`) | — | **Layout and controls / Disposición y controles**, a note under it as its neighbours have, one width (440) for the five choices; the Spanish top-bar extras were cut ("Armar automatizac…", "Teclado del ordena…") and read *Automatización*, *Teclado* | `look/settings-slate_*-100{,-es}.png` |
| C-7 | Yes | — | manual en/es: the window toolkit (egui-winit) reads Ctrl+Insert as Copy, not Windows | — |
| C-8 | Yes — the e2e's "each strip once" failed without the fix ("The kit's strip (1)" and "1 · Drums"; "First free strip (3)" and "3 · Strip 3") | the list added every strip that can carry a pad | the kit's strip and the first free one only in their own entries | `pad_user_strip_e2e.rs` |
| C-9 | Missing pictures; and the note list's shot showed a defect | an fx list is one widget, so the help line's fallback (`views::hint::menu_words`) read every row run together — cut, and over the status line | `fx::frame::popup_list` says the hovered row on the help line | `delay_window_e2e::the_note_list_reads_whole_in_both_languages` (every value whole, rows apart, nothing cut, en and es); `delay/delay-notes-{en,es}.png`; `pads/insp-{record-tap,hear-key}-{night,paper}-{100,150}-es.png` (`shoot_pads --es --zoom`) |

## Also

- **Slate's window goldens re-blessed** (`slate_window_golden`, `CANTUS_BLESS=1`): they failed on
  the base already — `look`'s answers were merged but never blessed (the docked mixer's returns
  at the edge with the next strips before them, the inspector's Velocity as a value bar) — plus
  this branch's send bar. Diffed against the images before: mixer — only the Sends row
  (x 1114–1262, y 561–581); piano roll — only the inspector's Velocity and Sends rows; the
  arrangement — those and the docked mixer (x 504–1262). Nothing else moved.
- `settings.rs`: one comment's alignment, as `cargo fmt` wants it (the integration left it).
- A finding for the engine's owner, not chased: the first try at B-4's test played a Pulsus
  groove, and the kit after a silent bar differed from the kit at the start by ~1e-8 from the
  first samples — an instrument shifted in time is not bit-identical to itself. The test
  plays an audio take (as `cantus-engine/tests/ripple_render.rs` does).

ADRs amended: 0116 (cues; the time keys; Ctrl+Ins; the engine test), 0117 (merge, the strip's
kit, the well's list), 0110 (*Layout and controls*, value bars, the sends). Manual en/es:
`arrangement.md` (cues, the keys' scope, Ctrl+Insert, *flechas*), `README.md` (the band's,
Mute's and the record tap's new names). `beta-checklist.md`: the record tap's Spanish words.

## Runs (Windows; no WSL)

- `cantus-core`: the whole suite green (528 unit + every integration binary, `ripple_props`
  and `undo_invariants` included).
- `cantus-ui`: the whole suite run once (294 binaries green); its three failures then —
  `slate_window_golden` (above), `ux_whole_words` (C-5), `delay_window_e2e`'s new test (C-9) —
  fixed and run again green, with `ripple_e2e`, `pad_user_strip_e2e`, `owner_look_e2e`.
- `clippy --all-targets`, `RUSTFLAGS=-D warnings`, `cantus-core` and `cantus-ui`: clean.
  `cargo fmt -p cantus-core -p cantus-ui`.

Verified only on the CPU renderer; nothing here needs hardware.

## For the owner to look at

`docs/round40/look/settings-slate_paper-100-es.png` (*Disposición y controles*),
`look/inspector-slate_paper-100.png` (value bars, no line through "−3.5 dB"; the send as a value
bar), `pads/insp-record-tap-paper-100-es.png` (*Se graba: Sin FX · Con FX*),
`delay/delay-notes-es.png` (the note list open), and C-1's numbering (FL's — his call).
