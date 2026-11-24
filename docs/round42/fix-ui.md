# Round 42 — `fix-ui`: the tuning UI's mid-round review, fixed

Fixer `fix-ui`, branch `r42/fix-ui` from `r42/integration` (433871a3e). The findings are
`F:\projects\daw-r42\review\verified-ui.md` (25: 24 confirmed, 1 partly) and the UI part of
`verified-reach.md` item 7. Every one is fixed; each row names what changed and the test that
holds it (written to fail first where the finding was testable).

## Decisions taken (the owner delegates by the excellence bar)

- **Ctrl+Shift+T** (⌘⇧T) opens the tuning, not Ctrl+Alt+T: GNOME, Ubuntu and most Linux
  desktops take Ctrl+Alt+T for the terminal before an application sees it. Ctrl+Shift+T is no
  system's shortcut on Windows, Linux desktops or macOS, and free in all five presets. Keymap
  version 20 (not released yet) brings it where it is free; a user's own Ctrl+Shift+T stays.
- **One name for the entry: *Options ▸ Tuning…*** (*Opciones ▸ Afinación…*) — the menu it is
  in — in the code's docs, ADR 0155/0156, the manuals and the checklist.
- **Notes are named with letters in Spanish too** (C D E … A4, `#`/`b`), as the app already
  names the song's key ("D menor"), the piano roll and the inspector (C4), and as Live, Logic,
  Cubase and FL name them in Spanish. The handful of fixed Spanish strings that used solfège
  ("La4" in Chorda, Fides, Bass, E-piano, Organ, Tonus, Saltus; "Do4", "Do1", "Do8", "La 415")
  now say "A4", "C4", "C1", "C8", "A415"; the Spanish tuning manual says the rule once
  (with the solfège names beside the letters) and uses letters throughout. ADR 0155 §6 records
  it. Reversible in the locale files alone if the owner prefers solfège.
- The Spanish tuning strings and manual now use **tú**, as the rest of the app and the manual do
  ("Cambia", "Haz clic", "Tus archivos"; "equipo", not "ordenador").

## The findings

| id | what changed | test |
|---|---|---|
| ui#1 | The window is modal for keys: `global_shortcuts` and `dispatch_keys` hand a pass to `tuning_window_keys` while it is open — only Undo, Redo and `OpenTuning` run, the typing piano releases what it holds and plays nothing; Esc closes only the window. `other_project` (every New/Open/demo/recovery/undo of a replace) calls `tuning::project_replaced`: the window closes, its cadence's keys are released, the per-track Chorda keys and the last file words are forgotten. | `tests/tuning_fixes.rs`: `the_open_window_holds_the_keys_but_undo_redo_and_its_own`, `a_replaced_project_closes_the_window_and_its_cadence` |
| ui#2 | `tuning::choose_system`: choosing MTS-ESP for the song resigns Cantus as master first. `tuning::master_switch`: the switch is enabled whenever it is on. | `choosing_mts_esp_resigns_the_master_and_the_switch_always_turns_off` (the fake `LIBMTS`, built by the test) |
| ui#3 | *Follow the song's key* is enabled while ticked, whether or not the song has a key. | `follow_the_songs_key_can_always_be_unticked` (a click through the window) |
| ui#4 | `tuning::with_system`: any system but Scala drops the keyboard map. A `.kbm` alone over Equal, a temperament or MTS-ESP makes their twelve notes a Scala scale with it (where the map row shows and removes it). `twelve_rows` blanks the beats of an unmapped key (dot at the pure end); `beat_colour(NaN)` is the pure colour. | `equal_and_temperament_drop_a_keyboard_map`; `tuning_circle::an_unmapped_key_shows_no_nan` |
| ui#5, ui#25 | One spelling helper (`words::spell_above`, from the integrator's patch, plus `fifth_above`, `key_with_octave`): the card's thirds, the circle's hover (fifth and third), the keyboard-map row (Eb4, not D#4). | `tuning_words::a_major_third_is_spelt_as_a_third_over_every_root`, `a_fifth_is_spelt_as_a_fifth_over_every_root` (all 12 roots); `tuning_circle::the_hover_spells_the_fifth_and_the_third_from_the_note` |
| ui#6 | `tuning::audition_place`: for the project's tuning the selected track only when it follows the project, else the first instrument track that does; none → "Every instrument track plays a tuning of its own…". | `the_cadence_plays_what_the_window_edits_or_says_why_not` |
| ui#7 | `tuning::plays_twelve` (the pictures' rule, plus a period near the octave): the cadence is disabled with its reason for a scale not 12 to the octave, in the window and in Chorda's card; `start_audition` refuses it. | same test (31-EDO from the MIT `tuning-library` file) |
| ui#8 | `chorda::key_of` returns the key and whether the project sets it: following the project's temperament it is the resolved key (the song's when it follows the song's key), shown greyed and rung on the circle; `chorda_key` is a map per track. | `chordas_card_shows_the_resolved_key_and_keeps_one_per_track` |
| ui#9 | `circle::purest_and_roughest`: `None` on a twelve-way tie (to 0.01 c) → "All twelve the same size: none purer"; the roughest list runs roughest first; each list's dot is coloured by its first third's `third_beats / THIRD_TOP`, the window's measure. | `tuning_circle::purest_and_roughest_say_a_tie_and_run_from_the_extremes`; the card's words in `tuning_whole` |
| ui#10 | One Spanish convention (letters; see above) in `tuning.es.txt`, Chorda's and the other instruments' A4 strings, the Spanish manual. | `inst_chorda.rs` (the Spanish preview says "A4 440.0 Hz"); whole-label tests EN/ES |
| ui#11 | The warm-up's runner takes the theme's key (`picker_thumbs::theme_key`, the tokens: process-wide) when it starts and checks it after each capture; the worn language and faces must be the interface's; a mismatch is `Done::Stale` — neither saved nor shown, counted in `stats.stale`. A look change stops the runner before its next picture (`Gate::stop`) and the next frame starts one in the new look, wearing the look as it is then. | not deterministic to provoke (a theme switch inside a capture); the code path is the one `picker_warm.rs` runs |
| ui#12 + the integrator's idle note | The warm-up is **one runner thread** (`Thumbs::warm_start`) that makes the pictures one after another at background priority (`cantus_rt::thread::lower_background_thread()`), each only while its `Gate` is open. `picker_warm::frame` only sets the gate in frames the app paints anyway — open at the instant the start-up's delay and a quiet hand allow, closed while the song plays or records and while the picker is open — and **never asks for a frame**; the runner asks for one only while the picker is open (a tile waits). At rest nothing is painted and the runner goes on by itself; the next frame painted takes in what it finished. Pictures already on disk stay there (a tile reads its file back in a frame). | `tests/picker_warm.rs` (both, the second new: `the_warm_up_waits_while_the_song_plays`); `tests/idle_repaint_e2e.rs` unchanged, 2/2 |
| ui#13 | Ctrl+Shift+T (see above); keymap v20's migration brings it. | `keymap::tests::keymap_19_gains_the_tuning_key_where_it_is_free` (every preset; a user's binding kept; no preset uses Ctrl+Alt+T) |
| ui#14 | The kit's `MenuField::list(false)` / `well::value_field`: a value field with no ▾ and no "menu" in the hint bar. The Scale row is a value; the Keyboard map row opens a list (no map / import a `.kbm`); A4 (the window's and Chorda's own) is a value field with ‹ › and a click to type a frequency (`rows::parse_hz`, "442,5" accepted, range-checked); the followed Key (window and Chorda's card) is greyed and still. | `tuning_whole` (every state, EN/ES, 75–200 %) |
| ui#15 | The checklist's cadence items say the button plays the temperament's own key and ask for F# by hand (with its chords spelt). | — (a document) |
| ui#16 | `io::scala_why` and `check_why`: every `ScalaError` and every `tuning_cmd::check` refusal has its locale key; I/O errors say "Could not read/write {file}" in the interface's language (also *Save as mine*). The Spanish manual quotes the Spanish line. | `tuning_io::a_refusal_reads_in_spanish` |
| ui#17 | `transport_menus::fit_summary`: short of room the tuning's " · " parts go whole from the end (A415 first, then the temperament); the song's line stays. | `summary_tests::a_short_bar_drops_the_tunings_parts_whole` |
| ui#18 | `tuning_window::split`: side by side only while both halves keep their width (the controls 500 pt, the pictures 440 pt — the full window, unchanged); narrower, **one scrolling column**: the controls, then the circle, its legend and every degree at the column's width (`pictures_height`). The footer's buttons are as wide as their words; its summary drops whole parts (or says the short default) when the room is short. | `tuning_whole::the_tuning_window_adapts_to_a_narrow_screen` (640 × 720 and 900 × 640 rooms, every state, EN/ES: nothing elided, nothing overlapping; a row cut at the column's scrolling edge is the scroll's) |
| ui#19 | `rows::clients_words`: none / one / n programs; `tuning.scale.notes_one`. | whole-label tests |
| ui#20 | The wording list reworded (EN: "every song's default", "Plays on {track}.", "Concert, octaves stretched", "Your files", "Your tunings"; ES: "Se leyó/Se escribió", "Tus archivos", "Como se escribió", tú throughout, "equipo"); `degree_written` prints a ratio as a ratio ("2/1"). | `tuning_words::numbers_read_as_a_musician_writes_them` |
| ui#21 | "Young II (1799)", "Neidhardt III, große Stadt (1724)" in both languages and both manuals. | whole-label tests (the temperament list) |
| ui#22 | *Options ▸ Tuning…* everywhere (`keymap.rs`, `app.rs`, `views/tuning.rs`, `views/mod.rs`, ADR 0155 §5, ADR 0156, DEVELOPING §4, both manuals, the checklist). | — |
| ui#23 | The pictures re-shot from this branch's head (below), replacing the builder's. | looked at, every one |
| ui#24 | *Slightly out* is stored by the fixed description `chorda::SLIGHTLY_OUT_ID` ("Cantus slightly out") and named in the interface's language by `words::scale_name`. | `slightly_out_is_found_in_either_language` |
| reach#7 (UI) | The binding and the four "Project ▸ Tuning" places (above); the checklist line 1062 now says *Opciones ▸ Afinación…*. | `keymap_19_…`, `the_open_window_holds_the_keys_…` (presses Ctrl+Shift+T through the boot path's keymap) |


## Also fixed on the way

- **Chorda's card, its files row** read cut at 75 % and in Spanish (`inst_chorda`'s two
  whole-label tests — failing on the integration base too). The row lost its caption ("Yours":
  the three buttons say what they do) and its buttons say *Import .scl* / *Export .scl*
  (`tuning.chorda.import/export`): the instruments' audit (`tests/inst_keys`) counts any
  painted "…" as elided, so a label ending in an ellipsis can never pass there.
- **The kit's `audio_parts::button` was suspected** of measuring less margin than the Slate
  dialog button it paints. It does not cut its words: the dialog button narrows its margin to
  fit (`kit::dialog::button`, `pad.min(...)`), and `tests/button_words.rs` holds every label
  the tuning views put on a button, EN/ES, Night/Paper, 75–200 %, nothing cut. What looked
  like a kit bug was the audit's "…" rule above; nothing in the kit changed.
- `views/inst/mod.rs`: rustfmt put `pub mod arcus;` in order (the integration tree failed
  `cargo fmt --check` there).

## Pictures

`docs/round42/tuning-face/` (80, replacing the builder's 24), made from this branch's head by
`cargo run -p cantus-ui --example shoot_tuning -- <dir>` and again with `--lang=es`: the
window (equal, werck, **narrow** — new: the same in a 760-point room, one scrolling column —
follow, scale, track), the inspector row, and Chorda's card (the project's, its own, plain),
Slate Lapis Night and Paper, 100 % and 200 %, English and Spanish. Looked at: one or more of
every kind in both themes, both languages and both zooms (werck, follow, scale, track,
equal, inspector, narrow ×2, Chorda's three cards ×5); the rest are the same views in the
other theme or zoom, and the whole-label tests read every one of their words.
