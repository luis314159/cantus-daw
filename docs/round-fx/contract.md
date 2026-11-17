# The effects side round — the contract (`9fe58632`)

One commit on `master` (`dd4ad68b`): five effect kinds as exact pass-through stubs with one
parameter `out` (dB, default 0 — a null), wired into every list; schema 72 (62 until the final merge) and
`fixtures/v72.cantus` (the five kinds on strip 2, Cribrum's `out` at −3 dB); the ADR stubs
0127–0130. No golden hash moved, `DSP_VERSION` unchanged. About 5 723 tests green; clippy clean
(workspace, plug-ins with no default features).

| Variant | Stem | Unit | ADR |
|---|---|---|---|
| `AutoFilter` | `cribrum` | Cribrum | 0127 |
| `Pedal` | `fragor` | Fragor | 0128 |
| `TimeShaper` | `lapsus` | Lapsus | 0129 |
| `LoFi` | `vetus` | Vetus | 0130 |
| `Utility` | `libra` | Libra | 0130 |

## Files each builder owns

`crates/cantus-core/src/fx/<stem>.rs`, `crates/cantus-dsp/src/<stem>.rs`,
`crates/cantus-engine/src/fx/<stem>.rs`, `crates/cantus-ui/src/views/fx/<stem>.rs`,
`crates/cantus-ui/locales/fx/<stem>.{en,es}.txt` (also its `picker.dev.<stem>.*` keys and
`mixer.chain.<stem>`), `crates/cantus-ui/presets/effects/{autofilter,pedal,timeshaper,lofi,
utility}.toml` (empty), the ADR, and any new file under your stem's name.

## Anchors in shared files (each your variant's/stem's line, marked `// fx:contract`)

Line numbers are as of `9fe58632`; find your line by the marker.

- **Core:** `cantus-core/src/effect.rs` (variants, `ALL[26..31]`, the dispatch arms: name_key,
  params, sparse, takes_sidechain, takes_notes, choices); `fx/mod.rs`; `starting.rs`
  (`K::<V> => Start::Neutral(Flat)` — replace with your starting preset or your real reason).
- **DSP / engine:** `cantus-dsp/src/lib.rs`; `cantus-engine/src/fx/mod.rs` (mod and `make()`
  arm); `tests/support/heavy.rs` (your heavy settings, now `("out", 6.0)`);
  `tests/fx_contract.rs` (`must_rest` and the per-kind line; `ALL.len()` is 31).
- **Project:** `format.rs` (`SCHEMA_VERSION = 72`, `migrate_71_to_72`, the test
  `schema_62_keeps_the_fx_side_rounds_kinds`); `examples/write_fixture.rs` (the `upto >= 62`
  block — add your settings there if you need the fixture to carry them); `dawproject/mod.rs`.
- **UI:** `views/fx/mod.rs` (mods, `FxEditors` fields, `FACTORY_PRESETS`, the `Editor` enum,
  `of`, `show`, the window list, framed, preview, `demo_setup`, `demo_feed`); hue arms in
  `views/fx/frame.rs` and `views/inspector_mini.rs`; `views/mixer_desk.rs`;
  `views/picker_snap.rs`; `picker_catalog.rs` (Cribrum `[Eq, Modulation]`, Fragor and Vetus
  `[Distortion]`, Lapsus `[Modulation]`, Libra `[Utility]`); `preset_data.rs`; `i18n.rs`;
  `app.rs` (`--editor` names); `agent_units.rs` (the AI's description — now "being built");
  `kit/skin.rs` and `kit/skin/side.rs` (one placeholder face per stem borrowed from the tone
  family — **replace it with a face that differs from every other unit's** in hue or
  material; the skin tests check it).
- **UI tests:** `fx_lint.rs`; `demos.rs` (`the_showcase_plays_round_28` checks `ALL[12..26]` —
  widen it once your unit is in `demos/heart/desk.rs`); `preset_audit.rs` (your floor is 0 —
  delete your line when your 12 presets land).
- **AI:** `cantus-ai/src/tools_args.rs` (effect names and aliases: auto_filter, pedal,
  time_shaper, lofi, bitcrusher, utility); `tools_mixer.rs` (the effect-word description —
  changing it means `CANTUS_TOOLS_DUMP=1` for `cantus-ui/tests/golden/mcp-tools-list.json`).
- **Plug-ins:** `Cargo.toml` (the `all` list and the features); `build.rs` (`PLUGINS`);
  `src/desc.rs` (`ALL: [Descriptor; 29]` and your descriptor block); `src/text.rs`;
  `tests/clap.rs` (the automated key, now `"out"`); `tests/common/mod.rs` (settings, now
  `("out", -6.0)`; `Direct::Stub` and its arms — add your real `Direct` variant and drop your
  Stub arm); `tests/golden/ids.txt` (append-only; keep `out`).
- **Bench and xtask:** `cantus-bench/src/kernels.rs` (heavy arms, `kind_id`);
  `cantus-bench/tests/bench.rs`; `xtask/src/main.rs`.
- The undo fuzz draws the newer kinds from `EffectKind::ALL[12..]`: no change was needed; a
  new command of yours gets its own arm.

## Notes

- rustfmt re-aligned Aula's trailing comments in `effect.rs` and `dawproject/mod.rs`.
- The MCP tools golden was regenerated on purpose (the five new effect words and aliases only).
- The website (`web/src/units_data.rs`) is untouched; your unit card is yours to add.
- `cargo fmt --all` fails on this machine (os error 206): `cargo fmt -p <crate>`.
