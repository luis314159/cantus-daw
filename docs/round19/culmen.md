# Round 19 — culmen: the mastering maximizer

Builder **culmen**, branch `r19/culmen`, on the contract commit 6cba3f7. Owner of
`EffectKind::Maximizer` end to end (ADR 0026 §4). The full write-up — what the references do,
the signal path, every parameter, the editor, the decisions, the numbers — is
[`docs/plugins/maximizer.md`](../plugins/maximizer.md).

## For STATUS

**Culmen** (`EffectKind::Maximizer`): FL Maximus's idea to the family's bar, with Ozone's
Maximizer and Pro-L 2 / Pro-MB in mind. Up to three bands (Dynamic, minimum-phase or
linear-phase crossovers, 6–48 dB/oct) plus the full band, each with a two-sided
compressor/expander curve (above `thr` and below `thr_lo`, ratio > 1 squeezes, < 1
stretches, range, knee, attack/release in ms, band gain, stereo/mid/side), into a drive and
Terminus's look-ahead true-peak brickwall (five characters, per-stage stereo link, 4× clipper,
DC filter); delta of everything Culmen changes, 1:1 and loudness-matched monitoring, dither.
Built from Multiplex and Terminus — the same code, not copies. The editor: gain over time with
a lane per band, all loudness readings at once with PLR and the input's, delivery targets
(Streaming −14, Apple −16, CD −9, Club −7, Broadcast −23) with a verdict in words and shape,
the bands dragged on the spectrum, the selected band's curve dragged by its points and slopes,
**Learn drive** as a closed loop (secant search on the measured output, within 0.3 LU, one
undo step), ten factory presets. Exported as *Cantus Culmen* (CLAP, VST3), bit-identical to
the engine's unit.

## Files

Mine (new or replaced):

- `crates/cantus-core/src/fx/maximizer.rs` — the model (replaced the stub; 73 parameters,
  sparse, `lo.`/`mid.`/`hi.`/`all.` band keys).
- `crates/cantus-dsp/src/culmen.rs`, `culmen_engine.rs`, `culmen_telemetry.rs`,
  `culmen_tests.rs`, `culmen_tests_stages.rs`.
- `crates/cantus-engine/src/fx/maximizer.rs` (replaced the wire),
  `crates/cantus-engine/tests/fx_maximizer.rs`.
- `crates/cantus-ui/src/views/fx/maximizer.rs` and `maximizer_{display,meters,bands,curve,cards,learn,demo}.rs`,
  `crates/cantus-ui/tests/fx_maximizer.rs`, `crates/cantus-ui/locales/fx/maximizer.{en,es}.txt`.
- `docs/plugins/maximizer.md`, this file.

Touched outside my area (each a line or a few, for the registries):

- `crates/cantus-core/src/effect.rs` — `Maximizer` taken out of `has_editor`'s exclusion
  (rustfmt then put the remaining four on one line).
- `crates/cantus-dsp/src/lib.rs` — `pub mod culmen;`.
- `crates/cantus-dsp/src/terminus.rs` — `Ditherer`, its `new` and `process` made `pub` (no
  behaviour change), so Culmen dithers with the same code.
- `crates/cantus-dsp/src/multiplex.rs` — `Multiplex::snap_next()` (three lines): the next
  `set` is taken at once, as the first one is. Nothing calls it but Culmen; Multiplex is
  unchanged otherwise (its 25 tests pass).
- `crates/cantus-ui/tests/fx_lint.rs` — Culmen's eight editor files added to the kit lint.
- `crates/cantus-plugins/src/params.rs` — `table()`'s cache sized by `EffectKind::ALL.len()`
  (it was 9: any of the three new kinds panicked there — fervor and latitudo will meet the
  same, and this line is the fix for all three).
- `crates/cantus-ui/src/views/fx/mod.rs` — `pub mod maximizer`, the `FxEditors::maximizer`
  state, and one arm in each dispatch (factory presets, `Editor`, `window`, `preview`,
  `demo_setup`, `demo_feed`).
- `crates/cantus-ui/src/views/fx/frame.rs` — `designed_size`: Culmen is 1180 × 720.
- `crates/cantus-ui/src/app.rs` — `demo_editor`: `"maximizer" | "culmen"`.
- `crates/cantus-ui/examples/shoot.rs` — `maximizer` in the default list.
- `crates/cantus-plugins/`: `src/desc.rs` (the descriptor; `ALL` is now `[Descriptor; 8]`),
  `Cargo.toml` (`maximizer` feature, in `all`), `src/params.rs` (Culmen's named band groups in
  `band_of`; its choice lists; dither's names), `src/text.rs` (the locale files),
  `tests/common/mod.rs` (settings that make it work; the `Direct` arm), `tests/clap.rs`
  (automation key; the factory count now reads `desc::ALL.len()`), `tests/vst3.rs` (the same
  count), `tests/golden/ids.txt` (Culmen's ids appended with `CANTUS_BLESS_IDS=1`).
- `xtask/src/main.rs` — `"culmen" => "maximizer,editor"`.

## Notes for the integrator

- **Merging with fervor and latitudo**: the three of us add a variant to the same lines —
  `has_editor`'s list, `desc::ALL` (its length: 8 here; with all three it is 10), the
  `all` feature, the xtask map, `views/fx/mod.rs`'s dispatches, `designed_size`,
  `demo_editor`, the shoot list, `params.rs`'s choice table, `text.rs`'s includes,
  `tests/common`. All additive. The factory-count tests now read `desc::ALL.len()`, so they
  need no edit for the others. `golden/ids.txt`: keep every appended block.
- **The master's GR meter** (`Meters::master_gr`, the master builder's): Culmen's telemetry
  slot 9 (`cantus_dsp::culmen::telemetry::GR_NOW`) is the brickwall's reduction now, dB, the
  deeper channel; the band gains are in each column. There is no generic `FxUnit` method for
  "gain reduction" in the contract; if the master builder adds one, Culmen's answer is
  `Culmen::reduction_now_db()`.
- **Latency**: `latency()` = the brickwall's (look-ahead + the true-peak detectors, 186
  samples at the defaults, 48 kHz) + 2304 samples with linear-phase bands. It changes with
  `look`, `tp`, `clip` and `xmode`.
- **Null at the defaults**: a new Culmen (nothing stored) is the input delayed by its latency,
  to the bit, while the signal stays under the −1 dBTP ceiling — Terminus's null rule. A
  bypassed Culmen is the host's bypass.
- **Demos**: `demo_setup` is a good "on the master" setting (classic three bands, drive 7 dB,
  Punch, −14 target). Presets: *Streaming −14* is the safe default for a demo song's master.
- A test finding worth keeping: a true-peak measurement of a buffer cut off at full level
  reads an overshoot at the cut (0.9 dB on our material) that is not the limiter's. Any test
  that checks a ceiling by the 32× reference should let the signal end in silence.
- **For Terminus's owner**: with its clipper on, Terminus overshot its true-peak ceiling by
  0.032 dB (32× reference) on band-limited drums and hats driven 18 dB through a
  minimum-phase band split — over its documented −0.019 dB. Culmen asks for the ceiling less
  0.05 dB (`cantus_dsp::culmen::TP_MARGIN_DB`); Terminus alone may want the same look.
