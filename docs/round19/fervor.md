# Round 19 — Fervor (`EffectKind::Saturator`), the builder's notes

*Branch `r19/fervor`. What is described in full is in `docs/plugins/saturator.md`; this is
what the integrator folds into STATUS.*

## Done

- **Parameter model** (`cantus-core/src/fx/saturator.rs`, replacing the contract's stub):
  20 globals and 16 drawn-curve points × 4 fields (`p{n}.on/x/y/t`, Aequalis's banded-key
  pattern, numbers never reused), sparse; every default a null. The stub's three keys
  (`drive`, `mix`, `out`) keep their meaning; `drive`'s range (0 … 36 dB) is unchanged.
- **DSP** (`cantus-dsp/src/fervor.rs`, `fervor_shape.rs`, `fervor_os.rs`, tests in
  `fervor_tests.rs`): seven models (tape with pre-emphasis, tube, overdrive with the low end
  spared, fuzz, clip with a knee, fold, the drawn curve), a drive law that is exactly a wire
  at 0 dB with no step after it, bias with the DC taken out, oversampling 2×/4×/8× with
  linear-phase FIR half-bands (latency 47/55/57) or polyphase IIR half-bands (5/7/8),
  low cut, tilt, high cut, bits and rate reduction, mid/side, mix with a matched dry delay, a
  static level match at −18 dBFS RMS, glides and crossfades on the family's 16-sample grid.
- **Engine unit** (`cantus-engine/src/fx/saturator.rs`, replacing the wire): `settings(spec)`
  shared with the editor, latency reported for delay compensation, telemetry (layout in
  the file).
- **Editor** (`cantus-ui/src/views/fx/saturator{,_curve,_harm}.rs`) on the device frame,
  kit only: model keys with a picture of each curve and two lines of words; the transfer
  curve as heard with the signal as a cloud and a peak ball (drag to drive); the drawn
  curve's editor (add, move, bend, remove, ends, symmetric, eight shapes); a measured
  harmonics display (a test tone through the real DSP: even/odd bars, aliasing in red, THD,
  even/odd, alias level) with the oversampling chosen under it; drive (XL), character named
  per model, bias, ×100; tone; crush; output with the match's gain and L+R/Mid/Side.
  13 factory presets. Strings in English and Spanish.
- **CLAP/VST3**: `Cantus Fervor` in `cantus-plugins` (descriptor, `saturator` feature, the
  bundler's `fervor` entry with its editor), proven bit-identical through the family's tests
  (`tests/clap.rs`, `vst3.rs`, `hosted_in_cantus.rs`, `alloc.rs`, `ids.rs`, `text.rs`,
  `editor.rs`), which iterate every descriptor.
- `docs/plugins/saturator.md`: the references, every parameter, the DSP, the decisions.

## Tests

Every crate touched, in full, green: cantus-core 83, cantus-dsp 380, cantus-project 35,
cantus-engine 189, cantus-ui 430, cantus-plugins 35 (all targets, `--no-fail-fast`);
`cargo clippy --tests` on the six crates clean; the touched files `rustfmt`-clean.

Fervor's own (written before the wiring, as the house rule asks):

- **DSP** (`fervor_tests.rs`, 20): `at_its_defaults_it_is_a_wire_to_the_bit` (block sizes 1 …
  4096); `without_drive_every_analytic_model_is_a_wire_whatever_its_character_and_bias`;
  `the_output_gain_alone_is_a_plain_gain`;
  `at_no_mix_it_is_the_dry_signal_delayed_by_its_latency_to_the_bit` (1×, 2×, 4×, 8×,
  minimum phase); `drive_glided_back_to_zero_is_the_wire_again`;
  `the_output_does_not_depend_on_the_block_size` (every model, both filter kinds, crush,
  mid/side, ×100; blocks 1, 7, 64, 1000 against 4096, tolerance 0);
  `a_change_lands_on_the_grid_whatever_the_blocks`; determinism; `silence_stays_exactly_silent`
  (every model × every factor, bias, ×100); `a_full_scale_square_at_the_most_drive_stays_finite_and_bounded`;
  `no_denormals_in_the_state_after_a_signal_dies_away`; `the_latencies_are_whole_samples_and_measured`
  (47/55/57 linear, 5/7/8 minimum, the impulse's centre); `the_pass_band_is_flat_to_20k_for_both_filter_kinds`;
  `oversampling_keeps_the_aliasing_out`; `symmetric_models_make_odd_harmonics_and_bias_or_asymmetry_even_ones`;
  `more_drive_more_harmonics`; `the_level_match_keeps_a_mix_level_signal_at_its_level` (every
  model within 1 dB); the drawn curve, tension, and `a_change_of_model_or_drive_while_playing_does_not_click`.
- **Engine** (`tests/fx_saturator.rs`, 5): inserted at its defaults the render is the render
  without it, to the bit; without drive no analytic model touches a sample; block-size
  independence at 8× with crush and mix; a dry Fervor at 8× is the whole project delayed by
  57 samples, to the bit (delay compensation and the dry delay together); latency, settings
  and telemetry through the contract.
- **Editor** (`crates/cantus-ui/tests/fx_saturator.rs`, 9): it paints its cards and words; the
  character knob is named per model; the preview's line; the harmonics are measured through
  the real DSP (a wire has none and nothing between them, tape odd only, tube even, 8× aliases
  far less than 1×, more level more distortion); a click on a model is one step; the drawn
  curve takes a point by double-click, moves it by drag (one step), takes an end, lets a point
  go by right-click (the sparse map forgets it); dragging an analytic curve drives it; a shape
  is one step and a straight line clears the curve; every factory preset names real keys in
  both languages. `fx_frame` (every kind opens in the frame, Fervor included) and `fx_lint` pass
  over the three new files.
- **CLAP/VST3** (`crates/cantus-plugins/tests/`): the family's tests now cover Fervor — bit
  identical to the DSP through CLAP and VST3 at the test settings (tube, 18 dB, 2× linear, a
  drawn point), any block size, automation of `drive` on the grid, the latency, state both
  ways, the bypass, zero allocations under automation of all 84 parameters, ids (88 lines
  blessed), host text in both languages, the editor in the plug-in window, and the round trip
  through Cantus's own CLAP host (`hosted_in_cantus`).

## Files touched outside Fervor's own

One line or one arm each, in the shared registries:

- `crates/cantus-core/src/effect.rs` — `Saturator` out of `has_editor`'s exclusion.
- `crates/cantus-dsp/src/lib.rs` — `pub mod fervor;`.
- `crates/cantus-ui/src/views/fx/mod.rs` — the module, `FxEditors::saturator`, factory
  presets, `Editor::Saturator`, the window, preview, demo set-up and feed arms.
- `crates/cantus-ui/src/app.rs` — `demo_editor`: `"saturator" | "fervor"`.
- `crates/cantus-ui/examples/shoot.rs` — `saturator` in the default list.
- `crates/cantus-ui/tests/fx_lint.rs` — the three editor files in the lint's list.
- `crates/cantus-plugins/src/{desc.rs, params.rs, text.rs}`, `Cargo.toml`,
  `tests/common/mod.rs`, `tests/clap.rs`, `tests/golden/ids.txt` — the descriptor (the array
  grows from 7 to 8: **latitudo and culmen grow it too — the integrator sums the three**), the
  choices' and points' groups, the locale files, the feature, the test host's `Direct` arm and
  settings, the automated key, the blessed ids.
- `xtask/src/main.rs` — `"fervor" => "saturator,editor"`.
- `crates/cantus-plugins/tests/clap.rs`, `vst3.rs` — the factory counts read `desc::ALL.len()`
  instead of 7 (the same edit fits the other two effect builders).
- `crates/cantus-plugins/tests/hosted_in_cantus.rs` — **a fix the other effect builders need
  too**: a state with a new latency makes our CLAP plug-in ask for a restart from its first
  `process` (the standard lets latency change only in `activate`); the test now runs one block,
  restarts as a session does, and measures from a fresh start — and asserts that a plug-in
  whose latency changed did ask. Before, any effect whose test settings change its latency
  failed with "reports the native latency: 0".
- `crates/cantus-ui/examples/shoot.rs` — `saturator` in the default list and a
  `saturator-curve` variant (the drawn curve).
- `crates/cantus-project/src/dawproject/mod.rs` — `Saturator => "Fervor"` in device names.

## Screens

`cargo run -p cantus-ui --example shoot -- <dir> 1 saturator saturator-curve`. Looked at
beside Porta's: the same frame, card grammar and density; the curve card is square, the
harmonics card takes the rest of the row. One thing seen in every editor's shot, Porta's
included: the software rasteriser darkens the lower part of some displays — not Fervor's.

## Left for later

Multiband (Saturn's bands), dynamics on the drive, feedback, a tape hysteresis model; the
DAW's own bypass does not yet keep the latency (ADR 0018), as for every latent effect.

## For the integrator

- `desc::ALL` is `[Descriptor; 8]` here; with the other two effect builders it becomes 10.
- `tests/golden/ids.txt` gains Fervor's lines; the other two add theirs — keep all.
- The model list's `has_editor` exclusion line is edited by all three effect builders.
