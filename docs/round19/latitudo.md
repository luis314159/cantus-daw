# Round 19 — Latitudo (stereo control and mid/side)

*Builder: latitudo · branch `r19/latitudo` · ADR 0026 · the effect's own page is
`docs/plugins/stereo.md`.*

## Done

`EffectKind::Stereo` end to end, in the way the family is built (ADR 0017/0018/0020):

- **Parameter model** (`cantus-core/src/fx/stereo.rs`): 29 keys, sparse, every default a
  null; the contract's `width`, `mid`, `side` kept their ranges and meaning. Keys:
  `width mid side bal rot swap inv_l inv_r listen mono mono_slope x_lo x_hi w_lo w_mid
  w_hi m_lo_f m_lo m_hi_f m_hi s_lo_f s_lo s_hi_f s_hi tone_slope haas haas_mode haas_amt
  out`. `has_editor` now true for `Stereo`.
- **DSP** (`cantus-dsp/src/latitudo{,_run,_view,_tests}.rs`): polarity/swap → classic
  Haas → M/S → mono-safe Haas → M/S shelves → width by band (LR4, mid all-passed) → mono
  below (LR 6/12/24/48, mid all-passed) → one output matrix (gains, width, listen,
  rotation, balance, out). With no M/S stage working the whole effect is one 2×2 matrix
  computed in f64; at the identity the block is untouched. Glides on the 16-sample
  stream grid with per-sample linear ramps; stages fade over 10 ms; structural changes
  (slopes, Haas mode) wait for silence. No latency, no allocation after `new`, `reset`
  allocation-free. Reuses Multiplex's TPT crossovers and Butterworth shelves (and their
  exact responses for the displays). Meters: 48 vectorscope points a block (packed
  12+12 bits), correlation/balance of input and output, correlation in three bands, side
  vs mid, levels, peaks.
- **Engine unit** (`cantus-engine/src/fx/stereo.rs`): `settings(spec)`, the `FxUnit`,
  telemetry; latency 0.
- **Editor** (`cantus-ui/src/views/fx/stereo{,_scope,_map}.rs`) on the device frame, kit
  only: vectorscope (age-faded dots, polar outline, level rings, auto-zoom) with
  correlation (input mark, lowest-in-3-s triangle) and balance meters; a frequency map
  (effective width with draggable band bars, crossovers and mono edge, per-band
  correlation; mid/side tone curves with draggable shelf dots and the slope switch);
  cards Image, Width by band, Output (listen, swap, polarities), Mid tone, Side tone, Mono
  bass, Haas (with the mono comb warning / mono-safe note). 13 factory presets, preview
  for the inspector, demo set-up and feed for `shoot`.
- **CLAP/VST3**: descriptor `Cantus Latitudo` (CLAP `com.github.lalmazan.cantus.latitudo`,
  VST3 `EC13BB3A-4A5F71E7-1468741C-D2C29ACE`), feature `stereo` (in `all`), bundle
  `latitudo` with its editor, choices named (`st.listen`, `st.slope`, `st.haas_mode`),
  the plug-in suite's settings and `Direct` arm. Ids blessed into `tests/golden/ids.txt`.
- **Docs**: `docs/plugins/stereo.md` (references, every parameter, the DSP, decisions).

## Tests (all green)

- **DSP** (`cantus-dsp`, `latitudo::tests`, 21): at the defaults the output is the input
  to the bit (signed zeros, the smallest normal, full scale; blocks 1…4096); neutral
  values of every stage are wires; every control put back is a wire again; block size
  does not matter to the bit (blocks 1, 7, 16, 64, 1000, 4096; telemetry too, peaks
  apart); deterministic, and reset starts over; silence stays silent and the state
  flushes to exact zero; a full-scale square at the extremes stays finite; width 0/2,
  swap/polarity/balance/listen, rotation, gains; mono below removes the low side per
  slope and keeps the mid, **and keeps the image above it** (−62 dB leak at 24 dB/oct);
  width by band and tone follow the responses the editor draws (±0.2 / ±0.1 dB); classic
  Haas delays one channel exactly and cancels at the comb frequency; mono-safe Haas keeps
  the mono sum; meters (correlation, balance, per-band correlation, points); eight switches
  thrown on a low tone make no click. Whole crate: 381 passed.
- **Engine** (`tests/fx_stereo.rs`, 6, + 2 unit tests): defaults and neutral stages render
  exactly as no insert; two swaps render as none; every stage on is block-size independent
  (1…4096); real time (uneven blocks, units sent as the backend does) equals offline to the
  bit; latency 0 and telemetry through the contract. `null_tests`, `fx_contract`,
  `fx_gate` still green.
- **UI** (`tests/fx_stereo.rs`, 6): the editor paints its cards, meters and map; the
  preview's line; the scope takes each point once (through the counter's wrap, capped);
  a band's bar drag is one undo step; the mono edge and a shelf dot are dragged on the
  map; the factory presets are in range. `fx_frame` (every kind with an editor opens in
  the frame), `fx_lint`, `kit_lint`, `fx_gate`, `inspector` and the lib's 125 (i18n:
  every key in both languages) green.
- **Plug-ins** (`cantus-plugins`, all targets): Latitudo's CLAP and VST3 bit-identical to
  the DSP and to the engine's unit, any block size, automation on the grid, state,
  latency, bypass, zero allocations in `process`, words in both languages, the editor in
  the plug-in window, and **hosted by Cantus's own CLAP host it sounds as the native
  insert to the bit** (`hosted_in_cantus.rs`). Ids blessed (33 new lines).
- `cargo clippy` clean on the touched crates for my code; `cargo fmt` clean.
- Screenshots: `cargo run -p cantus-ui --example shoot -- <dir> 1 stereo` writes
  `stereo-dark.png` / `stereo-light.png`; reviewed next to Porta's.

## Files touched outside my area (all small, local hunks)

- `crates/cantus-core/src/effect.rs` — `Stereo` out of the `has_editor` exclusion.
- `crates/cantus-core/src/fx/readout.rs`, `crates/cantus-ui/src/views/fx/controls.rs` —
  a `°` unit in the shared readout/parser (rotation prints `+12.5°`), and its tests.
- `crates/cantus-dsp/src/lib.rs` — `pub mod latitudo;`.
- `crates/cantus-ui/src/views/fx/mod.rs` — the registry lines (module, state field,
  presets, `Editor`, window, preview, demo set-up and feed).
- `crates/cantus-ui/src/views/fx/frame.rs` — `hue_of(Stereo)` = Space (the kit's words:
  width is space; the inspector's mini view already said Space).
- `crates/cantus-ui/src/app.rs` — `demo_editor("stereo")`.
- `crates/cantus-ui/examples/shoot.rs` — `stereo` in the default list.
- `crates/cantus-ui/tests/fx_lint.rs` — the three editor files on the kit list.
- `crates/cantus-plugins/{Cargo.toml, src/desc.rs, src/params.rs, src/text.rs,
  tests/common/mod.rs, tests/golden/ids.txt}`, `xtask/src/main.rs` — the export.
  `desc::ALL` became `[Descriptor; 8]`: Fervor and Culmen will each bump it — the merge is
  the array length and the appended entries.
- `crates/cantus-project/src/dawproject/mod.rs` — the device name "Latitudo".

## Notes for the integrator

- No new commands, no schema change.
- `desc::ALL`'s length, the `all` feature list, `text.rs`'s include list and
  `tests/golden/ids.txt` will conflict trivially with Fervor and Culmen: keep every
  entry; re-bless ids with `CANTUS_BLESS_IDS=1` after the merge if the golden file's order
  conflicts.
- On the master (ADR 0026) Latitudo works as any insert; it has no latency, so it never
  moves the heard position.

## What is left

- A linear-phase option for the crossovers (latency; only if a reason shows up — the
  minimum-phase ones keep the image intact, see `docs/plugins/stereo.md`).
- A decorrelation ("stereoize" by all-pass diffusion) as a fourth Haas mode, and Waves'
  asymmetry control — both considered, left out for this round.
- The scope draws the output only; an input ghost would need more telemetry room (the
  numbers of the input are there: correlation, balance, side level).
- The editor was reviewed in English at 100 %; Spanish at 75 % and 200 % is covered by the
  kit's eliding text policy but was not shot.
