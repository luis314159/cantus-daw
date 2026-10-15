# Round 28 — dynfx: dynamics and analysis

Builder: **dynfx** (branch `r28/dynfx`, from `r28/integration` at `935c273`). ADR **0056**.
BACKLOG §3b (*Clipper*, *Transient processor*, *Transient splitter*, *Analysis and
visualisation*). No schema change.

## Done

Four effects, each complete — DSP, engine unit, kit editor, factory presets, both
languages, a page in `docs/plugins/`, the CLAP/VST3 export bit-identical, its own tests on
top of the family's contract — and the pieces they share:

| Stem | Unit | What makes it first-rank (measured) |
|---|---|---|
| `tonsor` | **Tonsor** — clipper | Hard / soft / analog shoulders with a knee; **oversampling 1–16×** of the residual only, so a sample the curve leaves alone is untouched at any factor (the null at 0 dBFS with no overs holds to the bit, delayed by the latency); a **true-peak ceiling** (a guard with 53 samples of look-ahead): a hot mix clipped plainly at −1 dB leaves **+1.88 dBTP**, with the guard **−0.045 dBTP** at 1× and **−0.09** at 4×/16× by a 32-point reference; aliasing of a hard-clipped 7 kHz sine **−15.9 dB** at 1×, **−87 dB** at 16×; gain match; a clip meter (how much, how often over the last second of audio, input overs, output true peak, a 5 s history) |
| `impetus` | **Impetus** — transient processor | Attack and sustain from **level differences** (a 10 ms running maximum, two pairs of the shared follower): quiet and loud hits shaped alike, to the bit for a 12 dB step; attack ±100 %: the onset **+14.0 / −12.4 dB** against the body; sustain ±100 %: the tail **+12.0 / −9.8 dB**; a steady 55 Hz tone reads < 0.05 dB (no wobble); split band (highs/lows, a Butterworth SVF 90° from the rest): a bass under hats moves **−22.5 dB** full band, **−43.4 dB** highs only; sidechain; no latency; the editor runs a hit through the DSP and keeps three seconds of input and gain column by column |
| `discrimen` | **Discrimen** — transient splitter | An **exact split**: transient + body = input **bit for bit** for any mask (Sterbenz), in both modes, through the whole engine; **zero latency** (75 % of an onset's first 0.25 ms in the transient part, 96 % of its first 2 ms) or a **2 ms look-ahead** (100 % and 100 %); per part gain, a one-knob compressor and a tilt; listen to either alone |
| `speculum` | **Speculum** — analyser | Overview, spectrum (FFT 1k–16k, averaging, slope, peak hold, mid/side/L/R), spectrogram, oscilloscope (free, triggered, **on the beat** by the stamped song position), vectorscope and correlation, loudness (M/S/I, LRA, true peak, target, 30 s history), peak and RMS; reads the **EBU Tech 3341** cases 1–5 within ±0.1 LU and **Tech 3342** ranges within ±1 LU; a pure pass-through (bit-identical at every setting); **docked** in the workspace (resizable, split, torn out) or floating, **several at once** — three in the real window lose **no frame** at 60 Hz |

**Shared** (`crates/cantus-dsp/src/dynfx_*.rs`): the standalone **envelope follower**
(`dynfx_follow`: peak/RMS, time constants, homogeneous, exact rest — for the leaders'
follower modulator), the residual oversampler (`dynfx_os`), the level-difference detector
and the SVF split (`dynfx_transient`), the meter pieces that lose nothing between frames
(`dynfx_meter`: 50 ms holds, loss-free columns); in the UI `views/fx/dynfx.rs` (rates from
counters over the last second of audio, column history, labelled cells).

**Latencies that move under automation re-plan compensation** (ADR 0056): a signature of
every insert's latency is checked before each pass; Tonsor switched 1× → 16× → 1× by
automation keeps the whole mix in line to the bit. This also closes the gap for Fervor's
oversampling and Sibilus's look-ahead.

**Speculum's data path**: eight lock-free scope rings, chunks of 64 stereo frames stamped
with the song's position, copied after the insert on strips and master (no allocation —
tested), `EngineHandle::watch_scope / drain_scope / release_unwatched_scopes`;
`FxHost::take_stereo` in Cantus and in the exported plug-in (whose tap carries Speculum's
frames). Measurements on the editor's thread; integrated loudness every 200 ms of audio.

## Tests and what each holds

- `cantus-dsp` (`dynfx_*`, `tonsor_tests`, `impetus_tests`, `discrimen_tests`,
  `speculum_tests`): the figures above, each null, block-size independence, latencies where
  an impulse comes out, exact rest and as-new after `skip`.
- `cantus-engine/tests/fx_{tonsor,impetus,discrimen,speculum}.rs`: nulls through the whole
  engine, latency as the same render later, block sizes 1 … 4 096 at tolerance 0, live =
  offline, a key, the automated latency, Speculum's stream (every frame once, several at
  once, released, no allocation on the audio thread). Unit tests in `fx/<stem>.rs`: the
  table's defaults are the DSP's, every parameter read.
- `cantus-ui/tests/fx_{tonsor,impetus,discrimen,speculum}.rs`: words painted, one undo step
  per choice, the drawing against the DSP (Tonsor's curve within 0.05 dB of a sine through
  the DSP; Impetus's hit is the DSP's), the history keeps every column, no word cut short in
  either language, presets valid in both languages, docking. `speculum_budget.rs`: three
  analysers at once in the real window lose no frame; timed in release (ignored).
- `fx_contract.rs` (split and at-rest for all four, `must_rest` (true, true) each), the
  CLAP/VST3 export tests (bit-identical, automation on the grid), ids blessed.

## Files outside my area (additive)

- `crates/cantus-dsp/src/lib.rs`: four `pub mod dynfx_*;` lines.
- `crates/cantus-engine/src/{processor.rs, processor_snap.rs, processor_pass.rs}`: the
  latency signature (a field, `latency_signature`, `replan_if_latency_moved`, one call before
  each pass); `Processor::attach_scopes` and the `scoped` table.
- `crates/cantus-engine/src/{messages.rs, processor_msg.rs}`: `EngineMessage::WatchScopes`.
- `crates/cantus-engine/src/graph/{exec.rs, ctx.rs, strip_run.rs, master_run.rs}`: the scope
  cells, the context's `scoped`, `strip_run::scope` and its calls after an insert.
- `crates/cantus-engine/src/backend.rs`: the rings, `watch_scope`, `drain_scope`,
  `release_unwatched_scopes`, `watched_scopes`.
- `crates/cantus-ui/src/views/fx/host.rs`: `FxHost::take_stereo`, `StereoTake`, DawHost's.
- `crates/cantus-ui/src/views/fx/mod.rs`: `pub mod dynfx;` and one line in `window` drawing the
  docked Speculums.
- `crates/cantus-plugins/src/{instance.rs, editor.rs}`: Speculum's tap carries left/right;
  PluginHost's `take_stereo`.
- Shared lines: `heavy.rs`, `kernels.rs`, `fx_contract.rs` (unchanged), plug-in tests
  `common/mod.rs` (settings, direct variants; the stub variant became `Wire` for Speculum),
  `clap.rs` (automation keys), `golden/ids.txt` (appended; Speculum's `out` line removed).
- `docs/manual/{en,es}/README.md`: the *Dynamics and analysis* subsection of §10.
- New: `crates/cantus-ui/examples/shoot_dynfx.rs`.

## Left

Nothing that is mine to do. Owner-only: **listening** — the three shoulders, the true-peak
guard's sound when driven hard, Impetus and Discrimen on real drums — and how the editors
feel in the hand (shoots in `F:\projects\daw-r28\shots\dynfx\`).

## Notes for the integrator

- `Processor::replan_if_latency_moved` runs before every pass: if leaders' modulators also
  re-plan, keep one of the two; the test is `fx_tonsor::an_automated_latency_keeps_the_mix_in_line`.
- `EngineMessage` gained a variant; `Meters` was not touched.
- A Speculum docked and opened again brings its tab forward (one stream, one view).
