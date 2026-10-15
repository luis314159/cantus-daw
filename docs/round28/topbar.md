# Round 28 — topbar: the top bar à la FL (BACKLOG §6e)

Branch `r28/topbar` from the contract `ca5b48e`. ADR 0052. Schema 26. Keymap file 8.

## Done

The whole top bar of §6e, reachable from `cantus_ui::run` → `boot` → `assemble` (the bar is
`CantusApp::frame`'s toolbar panel; the title is set in `boot::become_main_window`):

1. **Transport**: to start, play, stop, record (+ ▾ options), loop — through `run_action`.
2. **Tempo block**: BPM well (drag/wheel/double-click); **TAP** (`Shift+T`; mean of the
   last 8 taps, a pause ends the run, one undo step per run); the **time signature**
   well opening an editor (beats ‹ ›, note value, usual metres) → `SetTimeSignature`.
3. **Metronome**: switch, **count-in**, **level** on the bar; ▾ with **sound** (Classic /
   Woodblock / Beep), **accent**, when, level, count-in. Schema 26 (`click_sound`,
   `click_accent`, migration writes nothing; old projects bit-identical). The *Click*
   rows left the record options.
4. **PAT / SONG** (`L`): `Project::pattern_mode` + `Command::SetPatternMode`;
   `cantus_core::pattern` (the rack's pattern region, `audition`); the app compiles the
   audition project (additive branch in `sync_engine`), moves the playhead in and back,
   follows the edited clip; recording refused with a hint; export renders the song.
5. **Snap / grid** of the focused editor (arrangement, piano roll, rack).
6. **Position**: bars.beats.ticks and time; pattern-relative in pattern mode.
7. **CPU / RAM / voices / drops** — moved from the status bar; RAM from
   `cantus_rt::usage::working_set_bytes` (Windows working set, Linux `VmRSS`, `None`
   elsewhere), read ~1/s on the UI thread; a new dropout said on the status line.
8. **Master meter** PEAK / LUFS (kept in settings): `Meters::master_loudness` (M, S, I since
   play, block true peak) from the engine's master, after the strip, before the click.
9. **Project name** renamed in place in the hint panel; **window title** "Name • — Cantus"
   from the boot path and every frame after.
10. **Inputs** (typing keyboard, MIDI lamp from `MidiSender::received`, armed tracks), the
    **audio chip** (moved from the status bar), **tools** (picker, appearance, zoom menu).

Groups are shown/hidden/reordered in **Customise top bar…** (⋯, View, right-click on the
bar), kept in `Settings::topbar`. The bar has FL's two rows (since review 1): tall groups
take its height, one-row groups stack two to a column; at 1920 px / 100 % every group is on
the bar in EN and ES (*review 2, fix2: with a running device's chip too — the chip now
takes 150 pt at least and grows into spare room; see `docs/round28/fix2.md`*). Narrow windows **fold groups whole into ⋯** by importance, and bring
back any that fits again; the menus never fold (they share the hint panel's width). ⋯
keeps its own open state, so the folded groups' menus and popovers work inside it; it
scrolls inside the window; its dot turns the chip's colour when the device is folded and
unwell.

## Measurements

- Layout property test: 800–3 840 px × 7 zooms × 6 group sets × EN/ES (two-row overlap
  check), no overlap, no clipping, owner's order kept; every group at 1920 px in EN and ES.
- Whole-window 64-strip scenario: **87 draw calls** (ceiling 96) and **119 346 triangles**
  (ceiling 120 000; the bar alone 5 424 at 1920 px, its row keys on one cap a strip) — the bar's words
  are drawn after its shapes. A custom group order allocates nothing more than the default.
- Master loudness cost (review 1): **2.5 µs a 32-frame callback, 5.1 µs at 64, 20.4 µs at
  256 — 80 ns per stereo frame, 0.38 % of one core at 48 kHz** (release, `loud_cost`).
  −20 dBFS 1 kHz stereo sine → −20.0 LUFS ±0.1 (M, S, I); mono → −23.0.
- Goldens unchanged (metering only reads); no `DSP_VERSION` bump.
- Group widths at 100 % (EN / ES): mode 59/78 (tall), transport 134, tempo 172,
  metronome 156/147 (tall), position 235 (tall), snap 166, undo 46, windows 142, inputs
  ≈160/≈178, performance 184/187 (tall), meter 151–280 (tall), device 177/≈150, tools 109;
  left block ≥ 334 (hint panel ≥ 300 pt). At 1280 px: inputs, CPU/RAM, the meter, the
  snap, undo and the tools fold; at 1600 px: undo, inputs, CPU/RAM.

## Tests (each seen failing first, or by mutation)

- `cantus-core`: `pattern::tests` (region, audition, the command and its serde),
  `audio::tests::the_metronome_choices_are_written_only_when_chosen`; undo fuzz arms for
  `SetPatternMode` and the new click fields (from existing draws — the seeds' cases do not
  move).
- `cantus-project`: `schema_26_keeps_the_metronome_voice_and_the_pattern_mode`, `v26`.
- `cantus-dsp`: `click::tests` — classic accented click equal **to the bit** to the old
  code; no accent → downbeat = beat for every voice; voices bounded, short, distinct.
- `cantus-engine`: `tests/topbar.rs` — voice/accent through the engine; master loudness in
  `Meters` (true peak ≥ sample peak, −∞ in silence); **pattern mode = the pattern laid end
  to end, to the bit**; `master_run::loud_tests` (BS.1770 value, block true peak);
  `midi::tests::the_sender_counts_what_comes_in`.
- `cantus-rt`: `usage::tests` (parser; the working set grows by what is touched).
- `cantus-ui`: `tests/topbar_layout.rs` (the property test, real frames inside the bar,
  every group inside its own width with no "…"), `tests/topbar.rs` (13 flows: tap and its
  undo, the TAP key, the metre editor, pattern mode + refused recording + undo, the title
  through the boot path + rename in place, snap follows the focus, metronome voice and
  accent, overflow usable at 900 px, customise hide/drag/reset saved, LUFS mode saved,
  MIDI lamp, performance moved); unit tests for layout, tap, perf texts, title, settings.

## Files outside my area (additive)

- `cantus-core`: `model.rs` (`pattern_mode` field + init), `command.rs` (`SetPatternMode`:
  variant, label, supersedes, apply), `lib.rs` (`pub mod pattern`, `ClickSound` export),
  `pattern.rs` (new), `tests/undo_invariants.rs` (two arms extended).
- `cantus-project`: `format.rs` (schema 26, `migrate_25_to_26`, test),
  `examples/write_fixture.rs` (section), `fixtures/v26.cantus`.
- `cantus-engine`: `processor.rs` (one field + init), `processor_pass.rs` (one line),
  `processor_rec.rs` (click voice/accent after the click level — not `add_clips`),
  `midi.rs` (`received` counter), `lib.rs` (`MasterLoudness` export), `tests/topbar.rs`.
- `cantus-rt`: `lib.rs` (`pub mod usage`), `Cargo.toml` (windows-sys feature
  `Win32_System_ProcessStatus`; no new crate).
- `cantus-ui`: `Cargo.toml` (`cantus-rt` dependency — our own crate), `app.rs` (`topbar`
  field + init; `run_action` arms `Record` guard, `TapTempo`, `TogglePatternMode`;
  `sync_engine` pattern branch; `title::sync` in `frame`), `boot.rs` (title on
  `become_main_window`), `keymap.rs` (two actions, file version 8, `NEW_IN_V8`),
  `tests/app_flows.rs` (expects `Ctrl+M` instead of `Ctrl+Shift+S`: two new transport
  actions pushed Save As below the shortcuts list's fold), `tests/audio_setup.rs`
  (`SCREEN` 1600 × 900: at 1280 the chip folds into ⋯).

## Left

Nothing that is not the owner's: judging the bar's look against FL's strip on the shots
(`F:projectsdaw-r28shots	opbar`, 144 PNGs).

## Notes for the integrator

- Schema **26**, ADR **0052**, keymap file version **8** (other builders may also add
  actions: chain `NEW_IN_V*` and the `Action::ALL` count).
- `Action::ALL` grew by two (82): merge conflicts there are additive.
- The status bar no longer shows the engine readout, the MIDI count or the chip; tests or
  docs elsewhere that look for them there should look at the top bar.
- Linux: `usage.rs` uses `/proc/self/status` (no `unsafe`); `parse_vm_rss` is
  `cfg(any(target_os = "linux", test))` so Windows builds have no dead code. Not run in WSL
  here — please include it in the Linux check.
- Shots: `cargo run --release -p cantus-ui --example shoot_topbar -- <dir> [filter]`.

## Review 1

The review (`F:\projects\daw-r28\reviews\topbar-1.json`) confirmed 44 findings, many the
same one seen by several lenses: 2 high, the rest medium and low. Each is fixed here, with
a test that failed first (or a mutation that makes it fail) wherever one can be written:

| Finding | Fix | Test |
|---|---|---|
| **High**: a folded group's own menus and popovers closed ⋯ and themselves (egui keeps one popup) | ⋯ keeps its open state on the bar and is an area under its popovers; closes on Esc (when none of them is open), a click outside, or ⋯ | `a_narrow_window_folds_groups_that_still_work_in_the_overflow` (the grid list sets the grid, the metronome's options open, ⋯ stays open, a click outside closes it) |
| **High**: at 1920 px five groups folded | FL's two rows: one-row groups (22-pt keys, FL's button strips: one cap a strip) stack two to a column; tighter gaps; the clock's side 70 pt; per-lamp widths; the chip capped at 200 pt (ES "Sin audio — configurar"); folded groups come back when room frees; Windows outranks Snap and Undo | `the_default_bar_shows_every_group_at_1920` (EN and ES), `short_groups_share_a_column_two_high`, `a_folded_group_comes_back_when_room_frees`, the property test's two-row overlap check |
| Pattern mode: playhead before or outside the pattern played silence | `follow` keeps the playhead in the pattern every frame | `pattern_mode_keeps_the_playhead_in_the_pattern` |
| Pattern mode switched on mid-take | `toggle` refuses with a hint; an undo or redo that brings it back ends the take first | `pattern_mode_waits_for_the_take_to_end` |
| Pattern state survived another project | keyed on the app's project epoch | `another_song_starts_without_the_last_ones_pattern_state` |
| Selecting a track or an audio clip dropped the pattern | the last note clip chosen stays the pattern | `selecting_something_else_keeps_the_pattern` |
| The app test did not look at what the engine got | it checks the sent snapshot: a far clip's pitch is out, the edited clip's in | `pattern_mode_plays_the_edited_clip_alone` |
| TP showed the last callback's peak | the backend keeps the highest of the callbacks drained; the meter holds the highest since Play until clicked | engine `the_true_peak_read_is_the_highest_of_the_blocks_drained` (mutation-checked), `the_true_peak_is_held_from_play_until_clicked` |
| Integrated loudness rescanned every callback; cost measured at 256 frames only | worked out per gating block while playing; `loud_cost` at 32/64/256 | `the_integrated_reading_is_worked_out_only_when_it_can_change` |
| "Integrated since play" moved after Stop | held after Stop, restarted on Play | `the_integrated_reading_holds_while_stopped` |
| One NaN left LUFS at −∞ for good | non-finite samples metered as silence | `a_nan_does_not_poison_the_meter` |
| Histograms first touched on the audio thread | `MasterLoud::prefault` from `Processor::prefault`; cleared at Play only when written | — (ADR 0031 R11; by inspection) |
| Mono output read 3 dB high | `process_mono` | `a_mono_output_is_weighed_as_one_channel` |
| The menus never folded into ≡ | the ≡ key is gone: it saved no width while the hint panel needs its 300 pt (final review rule); docs say the menus never fold | the property test (no compact block) |
| Text over the rename field | the field is drawn after the words are reordered; the panel leaves the name and the idle hint out while renaming | — (shot `topbar-rename-*`) |
| Clock elided digits in X/2 and ≥ 10 beats | its width follows the metre (cache keyed on it); tick digits from the beat's length | `the_clock_reads_whole_in_every_metre` |
| Drop line one row off downwards | drawn under the target when moving down | — (by inspection) |
| A custom order allocated every frame | resolved into a fixed array | `the_owners_order_allocates_nothing_more` |
| RAM "1000 MB" | the unit is decided on the rounded value; the cache keyed on it | `the_memory_reads_in_megabytes_then_gigabytes` |
| Fine Shift-drag of the click level lost in rounding | the drag is summed unrounded while it lasts | `a_slow_fine_drag_moves_the_click_level` |
| The rack's grid set from the bar skipped its refit | sets `scroll_steps = 0`, `fit = true`, as the rack's own menu | — (by inspection) |
| First-run dialog: the chip "at the bottom right"; unused `devices.status.engine` | points at the top bar (EN/ES); the string removed | — |
| Manual: MIDI count and dropouts in the status bar | point at the top bar (EN/ES) | — |
| The status-bar "moved" test proved nothing about the chip | it asserts the chip's words are gone from the status bar and on the top bar | `the_performance_moved_from_the_status_bar_to_the_top_bar` |
| No keymap 7 → 8 upgrade test | added, with a user's own `L` kept | `a_version_7_file_gets_tap_and_pattern_keys` (mutation-checked) |
| Spanish wording | "panel de ayuda", "reiniciarlo", "tras una pausa, vuelve a empezar", count-in "Claqueta" (the app's own term in the record options) | i18n test |

Judged otherwise, with the reason:

- *"Tap" on the Spanish bar*: kept. It is the word musicians use in Spanish too, the
  reviewer marked it optional, and "Marcar" would widen the tempo group.
- The three refuted findings (the refuter's reasons hold).
