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
bar), kept in `Settings::topbar`. Narrow windows **fold groups whole into ⋯** by
importance (menus fold into ≡ before the transport); ⋯ scrolls inside the window; its dot
turns the chip's colour when the device is folded and unwell.

## Measurements

- Layout property test: **11 172 layouts** (800–3 840 px × 7 zooms × 6 group sets × EN/ES),
  no overlap, no clipping, owner's order kept.
- Bar warm-frame allocations: **1** (was 12; ceiling 400). Whole-window draw calls in the
  64-strip scenario: **87** (ceiling 96) — the bar's words are drawn after its shapes.
- Master loudness cost: **121 ns per stereo frame, 0.58 % of one core at 48 kHz** (release,
  `loud_cost`). −20 dBFS 1 kHz stereo sine → −20.0 LUFS ±0.1 (M, S, I).
- Goldens unchanged (metering only reads); no `DSP_VERSION` bump.
- Group widths at 100 % (EN): mode 59, transport 164, tempo ≈180, metronome 156,
  position 249, snap 148, undo 58, windows 178, performance ≈185, meter 151–280, inputs
  137, device ≈177, tools ≈125; left block ≥ 334 (hint panel ≥ 300 pt).

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
(`F:\projects\daw-r28\shots\topbar\`, 144 PNGs) and choosing which groups he wants visible
at 1920 px (the default folds inputs and tools — and in Spanish the windows keys — into ⋯).

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
