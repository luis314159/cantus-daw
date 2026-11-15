# Tabula, the pedalboard — the builder's report (fx side round, wave 3)

Branch `fx/tabula` from `fx/integration` (`d3b51a09`, the wave-3 contract). ADR 0134
(provisional number). Phase A (commands, engine, design sheet) and, after the owner's verdict
(2026-11-15: a finer, subtler vinyl; bigger pedals filling the board, wrapping then
scrolling; a face of its own without the vents), phase B.

## Done

- **Model and commands** (`cantus-core`): the board commands' bodies with exact inverses
  (`command_board.rs`: insert and remove with their macro maps, move onto a lane,
  footswitch, shape, macro maps and names, a pedal's response and Lapsus bank), the
  canonical board (`None` when empty), refusals (no board, an id it holds, a kind that may
  not be a pedal, a nested board, the seventeenth, a bad map); `apply_to_spec` (the commands
  on a board outside a project) and `referenced_ids` (fresh ids skip what lanes still name);
  `fx/tabula_random.rs` (Randomise, seeded, musical ranges); `fx/tabula_presets.rs` (a board
  as a flat map of numbers, fourteen factory boards); `put_flat_params` and `prepared` (the
  snapshot's macros); the undo fuzz's board arm.
- **Engine** (`fx/tabula.rs`): pedals as `EffectUnit`s along Pre → (A ∥ B) → Post, fades,
  the soft bypass, the reorder's dip, lines carved from `MemoryNeed::Tabula`, adopt and
  retire, notes/key/position fan-out, exact rest, telemetry; `KnownPedals` (the handle sends
  `AddPedal`), `Boards` and `drain_retired` (the processor hands back faded pedals), the
  macros in the snapshot and over automation, `EffectUnit::hear_position` at every insert.
- **Window** (`views/fx/tabula*.rs`): the board drawn with Fragor's kit (every unit in its
  own paint, seven new motifs, cables with energy, the split's two boxes, wrapping rows),
  the macro rail with Learn and Randomise, the chosen pedal's controls, its place (size,
  lane, footswitch, remove), the board's levels, the picker as a shelf of small pedals, a
  pedal's full editor in place (`PedalHost`), every gesture one command (`tabula_edit.rs`),
  the keyboard and the screen reader; its face (`TABULA`, the new `FineTolex` material).
- **Presets**: fourteen boards for guitar, bass, synths, drums and voice; loaded whole as one
  `ReplaceEffect` from every preset path; user presets keep the board.
- **Exported plug-in**: the board in the state (`save_spec_extras`), a fresh instance on the
  guitar rig, boards posted through rings with their new pedals built off the audio thread
  (`board_post.rs`), the memory builder routing pedals' needs, the editor's pedal edits and
  board commands applied to the instance's board.
- **Docs**: `docs/plugins/tabula.md`, the manual (EN and ES), ADR 0134 finished, the site's
  card, the AI's unit description and effect words.

## Measured

- **CPU**: see the last section (`cantus-engine/examples/tabula_overhead.rs`).
- **Sounds** (`C:\projects\daw-fx\compartir\tabula\sound\`): on the CC0 DI guitar, each
  level-matched to the dry file within 0.01 dB: the rig (out −4.5 dB), a parallel split
  (−4.0 dB), a macro sweep (−4.9 dB).
- **Pictures**: the design sheet (`compartir\tabula\design\`) and its v2 after the verdict
  (`design\v2\`): the rig, a full board wrapped into two rows, the split, the picker, a
  pedal's full editor — Slate EN and Paper ES at 100 %, the rig at 75/150/200 %.

## Tests and what each holds

See ADR 0134 *What the tests hold*. Files: `crates/cantus-core/src/command_board_tests.rs`,
`fx/tabula.rs`, `fx/tabula_presets.rs`, `fx/tabula_random.rs` (unit tests),
`crates/cantus-core/tests/undo_invariants.rs` (the board's arm),
`crates/cantus-engine/tests/fx_tabula.rs`, `fx_contract.rs` (`a_board_of_one_is_the_unit`,
`split::tabula`, `at_rest::tabula`), `rt_memory.rs` (`tabula_never_allocates`),
`crates/cantus-ui/tests/tabula_e2e.rs`, `crates/cantus-plugins/tests/tabula_board.rs`.

## Shared files touched (each line marked `fx3:tabula`)

- Core: `command.rs` (none), `mix_snap.rs` (exact undo through `put_flat_params`),
  `snapshot.rs` (`prepared`), `lib.rs` (`command_board` public), `fx/mod.rs` (two modules).
- Engine: `effects.rs` (`pedal_mut` of a boxed pedal, `hear_position`), `fx/mod.rs` (the
  doc of `set_position_late`), `graph/strip_run.rs`, `graph/strip.rs`,
  `graph/master_run.rs` (`hear_position`), `automation.rs` (`ensure_macro_keys`),
  `processor_auto.rs` (`apply_macros`), `backend.rs` (`KnownPedals`, `AddPedal`),
  `processor.rs` and `listen_fork.rs` (`boards`), `processor_msg.rs` (`Boards`,
  `drain_retired`); tests `fx_contract.rs`, `support/heavy.rs`, `rt_memory.rs`;
  `cantus-bench/src/kernels.rs` (the heavy board).
- UI: `views/fx/mod.rs` (modules, the code presets, `apply_fx_params` loads a board),
  `host.rs` (`board_commands`, a board's user preset), `frame.rs` (designed size),
  `pedal_draw.rs` (a knob's word takes its row's room, set smaller before eliding — Fragor's
  words, which fitted, are drawn as they were), `pedal_art.rs` (seven motifs, the smallest
  cut for a long one-word name), `kit/skin.rs`, `skin/materials.rs`, `skin/textures.rs`,
  `kit/gallery_skin.rs` (the `FineTolex` material and its tile), `skin/side.rs` (`TABULA`),
  `agent_units.rs`, `tests/preset_audit.rs` (the floor line removed).
- Plug-ins: `lib.rs`, `state.rs` (`save_spec_extras`, `load_board`), `instance.rs`,
  `memory.rs` (needs by pedal), `editor.rs`; tests `common/mod.rs` (`board_spec`).
- AI: `tools_mixer.rs` (the effect words).
- Web: `web/src/units_data.rs` (the card; `EFFECTS` 31).

## Verified only in emulation

- No real host was run with the exported Tabula: its state, posts and editor are checked
  through the instance's own API and the family's CLAP/VST3 harness.
- The keyboard and the screen reader's nodes were exercised only by building them; no
  screen reader was run.
- The sounds were rendered offline; nobody listened on monitors yet (the owner's ears).

## Left

- **Owner-only:** listening to the sounds; whether Tabula goes on the showcase song's
  guitar (it changes that song's sound); the website's pictures and preset excerpts are
  generated by the site's tools at the merge (the card is in).
