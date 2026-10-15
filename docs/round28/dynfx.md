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
| `tonsor` | **Tonsor** — clipper | Hard / soft / analog shoulders with a knee; **oversampling 1–16×** of the residual only, so a sample the curve leaves alone is untouched at any factor (the null at 0 dBFS with no overs holds to the bit, delayed by the latency); a **true-peak ceiling** (a guard with 53 samples of look-ahead): a hot mix clipped plainly at −1 dB leaves **+1.88 dBTP**, with the guard **−0.045 dBTP** at 1× and **−0.072** at 4×/16× by a 32-point reference; an oversampled hard clip without the guard within **+0.034 dB** of its ceiling; aliasing of a hard-clipped 7 kHz sine **−15.9 dB** at 1×, **−84.7 dB** at 16×; gain match; a clip meter (how much, how often over the last second of audio, input overs, output true peak, a 5 s history) |
| `impetus` | **Impetus** — transient processor | Attack and sustain from **level differences** (a 10 ms running maximum, two pairs of the shared follower): quiet and loud hits shaped alike, to the bit for a 12 dB step; attack ±100 %: the onset **+14.0 / −12.4 dB** against the body; sustain ±100 %: the tail **+12.0 / −9.8 dB**; a steady 55 Hz tone reads < 0.05 dB (no wobble); split band (highs/lows, a Butterworth SVF 90° from the rest): a bass under hats moves **−22.5 dB** full band, **−43.4 dB** highs only; sidechain; no latency; the editor runs a hit through the DSP and keeps three seconds of input and gain column by column |
| `discrimen` | **Discrimen** — transient splitter | An **exact split**: transient + body = input **bit for bit** for any mask (Sterbenz), in both modes, through the whole engine; **zero latency** (75 % of an onset's first 0.25 ms in the transient part, 96 % of its first 2 ms) or a **2 ms look-ahead** (100 % and 100 %); per part gain, a one-knob compressor and a tilt; listen to either alone |
| `speculum` | **Speculum** — analyser | Overview, spectrum (FFT 1k–16k, averaging, slope, peak hold, mid/side/L/R), spectrogram, oscilloscope (free, triggered, **on the beat** by the stamped song position), vectorscope and correlation, loudness (M/S/I, LRA, true peak, target, 30 s history), peak and RMS; reads the **EBU Tech 3341** cases 1–5 within ±0.1 LU and **Tech 3342** ranges within ±1 LU; a pure pass-through (bit-identical at every setting); **docked** in the workspace (resizable, split, torn out) or floating, **several at once** — three in the real window lose **no frame** at 60 Hz, and the whole 1080p window with them builds and tessellates in **1.30 ms** median (95th percentile 1.84 ms, release) |

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

120 new tests (one timed, ignored). The whole of `cantus-dsp`, `cantus-core`, `cantus-project`,
`cantus-engine`, `cantus-bench`, `cantus-plugins` and `cantus-ui` pass; `cargo clippy --workspace
--all-targets -D warnings` and the plug-ins without default features are clean; `cargo fmt`.
The Linux check in WSL was not run (no `cfg(windows)` code was added).

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
- `crates/cantus-plugins/tests/clap.rs`: besides my automation keys, the two "it did
  something" checks expect Speculum to change nothing (it is a pass-through).
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

## Review 1

The review (5 lenses and 2 focus lenses, each finding checked by a refuter) confirmed 34
findings; with the duplicates across lenses merged, 23. All are fixed; none was judged
wrong. A test was written for each that can be tested and seen failing before the fix,
except where the test needs an interface the fix introduces (noted).

| # | Finding (severity) | Fixed how | Test |
|---|---|---|---|
| 1 | **Tonsor's oversampled residual added (F−1)/F of a sample early** — an oversampled clip passed its ceiling by +2.26 dB (HIGH) | The decimator's window ends at the newest sample's point 0 (`dynfx_os::Down::push`, its ring grew by a factor's worth): within **+0.034 dB** now; TP guard −0.072 dBTP; aliasing −84.7 dB at 16× | `the_decimator_is_aligned_with_point_zero` (phase against the declared delay, failed at 0.33), `an_oversampled_clip_holds_its_ceiling_without_the_guard` (failed at +2.26 dB) |
| 2 | One NaN/inf locked the detector (Impetus ±24 dB, Discrimen mask 1) and the units' filters | A non-finite level is silence in `Follower::run` and `Diff::run`; Impetus's filters and key, Discrimen's part sections take `dynfx_follow::sane` samples | `a_non_finite_level_locks_nothing` (detector), `a_non_finite_sample_locks_nothing` (Impetus, Discrimen) — all three failed |
| 3 | Switching between two factors above 1× emptied the dry line (40 samples of silence) | Same latency: only the decimator's history starts over | `switching_between_two_factors_keeps_the_dry_line` (failed) |
| 4 | Speculum's beat stamp ignored the compensation its slot waits | `Processor::scope_delays` (each plan and each watch change): a strip's arrival + the inserts before, the master's mix arrival + its inserts before; stamps moved back by it | `the_beat_stamp_counts_the_compensation_the_slot_waits` (new interface) |
| 5 | The "let go" test could not fail | Looks at the ring itself (`EngineHandle::scope_ring_len`), with a positive control | rewritten |
| 6 | Ring capacity overstated (chunks per pass), losses counted but never reported | Chunks filled across passes (65 536 frames whatever the passes, flushed at a jump); frames lost carried in the next chunk, returned by `drain_scope`, shown as *Gaps* under Speculum's loudness; the plug-in counts its tap's and its window queue's losses | `a_full_ring_says_what_it_lost` (exactly 65 536 frames held, the rest reported) |
| 7 | Exported Speculum's clock was the block's start used as the newest frame | The instance publishes the block's last frame's position | `the_exported_speculum_reads_its_own_stereo_frames_and_the_newest_frames_beat` |
| 8 | Docked Speculums survived opening another project (and attached to other effects with the same ids) | Closed when the project's epoch moves; kept only while the effect is a Speculum | `another_project_closes_the_docked_speculums` |
| 9 | A ninth Speculum (or no device) said "Play the song" forever | `View::stream` read: *No stream: no audio device, or eight Speculums are open already — close one* | `with_nothing_measured_it_says_why` (both cases) |
| 10 | Impetus's editor had no key picker | *Detect from* field in the band card (any strip; the sidechain switch in a plug-in), shared as `dynfx::key_source`; documented | `the_key_is_chosen_in_the_editor` (one undo step) |
| 11 | No test that a strip's ring gets every frame, bypassed too, or that pool workers allocate nothing | Strip and master rings compared frame for frame with the master's output (bypassed and not); allocations counted on the pool's workers too | `every_frame_of_a_watched_speculum_reaches_its_ring`, `the_copy_allocates_nothing` |
| 12 | The exported stereo tap had no test | Tested: left/right bit for bit, in order; another kind has no stream | as #7 |
| 13 | A reused ring could hand a block of the last Speculum's audio to the next | Each chunk carries its owner; `drain_scope` drops the others | `a_reused_ring_gives_only_its_new_owners_frames` (new interface) |
| 14 | Docked settings row overflowed with no scroll | The settings break into rows that fit any width (`settings_layout`); the display keeps room; the docked tab's least size 560 × 420 | `a_docked_tabs_settings_fit_its_width` |
| 15 | Peak/RMS bars without values, legend or scale | Titled *Peak and RMS*, a legend, each channel's two numbers, a dB scale where there is room | in `every_view_paints_what_it_measures_and_its_settings` |
| 16 | "L+R" / "I+D" for left and right | "L/R" / "I/D" | as #15 |
| 17 | Tonsor's ceiling tag could sit on the axis label or the numbers; "DBFS" | The tag kept clear of both; axis labels keep their case | visual — the shoots |
| 18 | Tonsor's oversampling explanation never shown | Shown on the keys' hover | `the_oversampling_keys_explain_themselves` |
| 19 | Spanish: "Impetus transitorios", "Un golpe a través", "Guarda" | "Impetus moldeador", "Un golpe procesado", "Protección TP" | the no-cut tests in Spanish |
| 20 | Discrimen's no-cut test never painted the zero-latency sentence | Both modes painted; both sentences shortened to fit | `no_word_is_cut_in_either_language` |
| 21 | **The automated re-plan emptied every re-timed line: the mix dropped out for each path's delay** | `DelayLine::retime` keeps a line's content and moves its read point (shorter skips ahead; longer waits only the difference) | `a_new_delay_keeps_what_the_line_holds` (unit, failed), `an_automated_latency_switch_costs_at_most_the_change` (93 silent frames before, ≤ 40 now) |
| 22 | Master Speculum got no frames with the master FX off or all bypassed | The wire path still feeds a watched master Speculum | `a_master_speculum_measures_with_the_chain_off_or_bypassed` (failed) |
| 23 | The latency signature folded the master chain as one sum | Each master insert folded | `opposite_moves_on_two_master_inserts_re_plan` (failed) |

Shoots re-taken (`F:\projects\daw-r28\shots\dynfx\`) and looked at: Tonsor, Impetus (the
*Detect from* field), Discrimen, Speculum's overview and loudness view.
