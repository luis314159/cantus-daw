# Round 28 — modfx: the modulation family, and the host's transport in the exports

Builder: **modfx** (branch `r28/modfx`, from the contract `ca5b48e`). ADR **0055**. BACKLOG §3b
(*Modulation*). No schema change.

## Done

Eight effects, each complete — DSP, engine unit, kit editor, factory presets, both
languages, a page in `docs/plugins/`, the CLAP/VST3 export bit-identical, and its own tests
on top of the family's contract:

| Stem | Unit | What makes it first-rank (measured) |
|---|---|---|
| `concentus` | Concentus — chorus | Juno I / II / I+II from the circuit's published sweeps (measured through the unit: 1.66–5.35 ms at 0.513 and 0.863 Hz, 3.3–3.7 ms at 9.75 Hz, within 0.01 ms and 1 %; the right channel opposite); Dimension 1–4 (crossed out-of-phase lines: wider than Juno I, < 3 dB mono loss); 1–4 free voices on the family's LFO; BBD tone; a **mono check** (correlation, mono-sum loss, a verdict) |
| `flexus` | Flexus — flanger | **through zero** (5 ms centre, reported latency 240 samples at 48 kHz): the centre null is **exact zeros** in the DSP, through the whole engine and in the editor's comb; feedback ±95 %, LFO / envelope / manual sweeps; *the comb now* drawn with the unit's own arithmetic (sines measured within 0.6 dB) |
| `phasis` | Phasis — phaser | 2–12 TPT allpass stages: a notch < −30 dB at every frequency where the prewarped phase says, +6 dB peaks; feedback ±95 % bounded; Phase 90 (script/block) and Small Stone presets; the response drawn within 0.3 dB of the DSP |
| `tremor` | Tremor — tremolo / auto-pan | tremolo, **equal-power** auto-pan (L²+R² constant within 1e-5, unity at centre), harmonic tremolo; six shapes with a soft edge, swing, stereo phase; a full square gate lands on the song's beats to the bit |
| `vibrans` | Vibrans — vibrato | depth **in cents** (measured ±8 % at 50 ct/5 Hz, 25 ct/3 Hz triangle, 100 ct/0.5 Hz), a **rise** after each silence, stereo phase, mix |
| `anulus` | Anulus — ring modulator | band-limited carriers (polyBLEP/BLAMP: the folded 9th harmonic of a 5 kHz square ≥ 10 dB under naive), the input gone (< −80 dB), **no DC** (< 1e-3 with a partial on the carrier and an input offset), an LFO in semitones; the carrier shown as a note |
| `translatio` | Translatio — frequency shifter | Hilbert pair designed per sample rate: **69.2 dB** worst sideband rejection measured from 20 Hz to 19.9 kHz at 44.1 and 48 kHz (designed 69.5 / 68.7 dB, 14 sections); up / down / both; feedback inside the 90° band, bounded at 95 % for any shift |
| `fluctus` | Fluctus — volume shaper | a **drawn curve** of up to 16 points shaped by the automation editor's own `Shape::at`, 4/1 … 1/16 on the song position, smoothing (the wrap's step from 1 to < 0.02/sample at 2 ms), a kit curve editor (drag, Shift for free, double-click add/remove, bend, segment shapes — one undo step each), ten pumping/gate presets |

**The family's base** (ADR 0055): a clock of integers on the song position
(`modfx_clock`), an LFO that is a function of it (`modfx_lfo`: synced = song position, free
= from playback, six shapes with a soft edge, swing, seeded random steps), a modulated line
with an exact whole-sample read (`modfx_line`, also the TPT filters and a glide that lands),
the Hilbert design (`modfx_hilbert`). Synced LFOs: a render from bar 5 equals bars 5… of the
full render; offline = live (tested per unit through the processor, irregular blocks); exact
rest for all eight (the twin tests at default, heavy and random settings).

**The host's transport in the exported plug-ins**: CLAP `clap_event_transport` and VST3
`ProcessContext` (now requested) feed `SongPosition` to the unit per chunk; the editor reads
it. A synced Tremor gate follows a host at 90 BPM from beat 2.5 bit-identical to the engine at
blocks 1 … 4096 (CLAP and VST3), re-anchors on a jump; tests seen failing without the feed.
This closes the gap for every synced unit.

**The editors**: every one draws what the DSP will do (the movement over two cycles with a
playhead, the comb, the response, the partials, the curve) from the settings the DSP reads,
and a test per editor holds the drawing against the running DSP. Shared parts in
`views/fx/modfx.rs` (LFO card with shape keys, movement display, mode card, preview strip,
idle note). Shoots: `F:\projects\daw-r28\shots\modfx\` — every editor in Cantus Night and
Paper, English and Spanish, at 75 / 100 / 200 % (`cargo run -p cantus-ui --example
shoot_modfx -- <dir> [stem…] [--quick] [--preview]`), looked at; long Spanish labels got
`.short` forms, displays got margins so no word sits on a curve.

**The manual** (EN/ES §10, *The modulation effects*), **ADR 0055**, eight pages in
`docs/plugins/`.

## Tests and what each holds

- `crates/cantus-dsp/src/modfx_*` and `<stem>_tests.rs` (80): the clock never re-anchors on
  a split, anchors on a seek/tempo/play; the LFO is the same at every split, bar 5 = bar 5,
  free from playback, rate changes do not jump, shapes bounded and continuous, swing; the
  Hilbert design ≥ 60 dB at 44.1/48 kHz and the running pair in quadrature; each unit's
  measurements above; split invariance; rest and `skip` exact.
- `crates/cantus-engine/tests/fx_<stem>.rs` (eight files, 36 tests): null at defaults and at
  the amount 0 in every mode; block sizes 1, 7, 64, 1 000, 4 096 at tolerance 0; live =
  offline; Tremor's and Fluctus's gates on the song's beats from the start and from beat 3;
  the project's tempo heard; Flexus's latency compensated and its centre silence.
- `crates/cantus-engine/src/fx/<stem>.rs` unit tests (20): the table's defaults are the DSP's,
  every parameter read.
- `crates/cantus-ui/tests/fx_<stem>.rs` (eight files, 39 tests): words painted, gestures as
  single undo steps (modes, curve points), the drawing against the DSP, presets valid and in
  both languages.
- `crates/cantus-plugins/tests/transport.rs` (5) and the VST3 transport test; the family's
  rows in `clap.rs`/`vst3.rs` (bit-identical, automation on the grid), ids blessed (append
  only).
- `fx_contract.rs`: split and at-rest for all eight (`must_rest` (true, true) each).

The whole of `cantus-dsp`, `cantus-core`, `cantus-project`, `cantus-engine`,
`cantus-plugins`, `cantus-bench` and `cantus-ui` pass; `cargo clippy --workspace
--all-targets -D warnings` and the plug-ins without default features are clean; `cargo fmt`.

## Files outside my area (additive)

- `crates/cantus-dsp/src/lib.rs`: four `pub mod modfx_*;` lines.
- `crates/cantus-engine/src/fx/mod.rs`: `pub mod modfx;`.
- `crates/cantus-ui/src/views/fx/mod.rs`: `pub mod modfx;` (with its doc line).
- `crates/cantus-core/src/fx/readout.rs` and `views/fx/controls.rs::write_readout`: a value in
  Hz below 10 prints with hundredths (`0.25 Hz`, was `0 Hz`) — the plug-ins' value text
  reads it back; the existing readout tests pass.
- `crates/cantus-plugins/src/params.rs::band_of`: Fluctus's points grouped as *Point n*.
- `crates/cantus-plugins/src/instance.rs`: `HostTransport`, `Processor::set_transport`, the
  per-chunk `set_position`, `Shared::host_transport` (the format-free half of the transport).
- `crates/cantus-plugins/src/editor.rs::transport`: the host's transport for the editor.
- `crates/cantus-plugins/tests/{common/mod.rs, vst3.rs}`: the test hosts can send a
  transport (a field each); my settings/direct/automation lines; `golden/ids.txt` appended.
- `crates/cantus-bench/src/kernels.rs`, `crates/cantus-engine/tests/support/heavy.rs`,
  `fx_contract.rs`: my lines. New: `tests/support/modfx.rs` (shared by my engine tests).
- `docs/manual/{en,es}/README.md`: the family's subsection in §10.

## Left

Nothing that is mine to do. Owner-only: **listening** — the Juno and Dimension modes against
the owner's references, the presets' musical judgement, and how the editors feel in the hand
(the shoots are in `F:\projects\daw-r28\shots\modfx\`).

## Notes for the integrator

- **Defaults are nulls** with the amount control at 0 (the contract's rule; Fervor's
  precedent), so a freshly inserted unit is silent until its *Mix*/*Depth* is raised or a
  preset is chosen; each editor's display says so. If the owner prefers sounding defaults
  (Spatium's way), it is one default per table — and the null tests move to "at mix 0".
- Anulus does **not** take notes (`TAKES_NOTES = false`): following a track's notes needs the
  exported plug-ins' note input, which voice builds for Tonus; once merged, it is a small,
  separate step (the brief made it optional).
- The voice builder may also edit `clap.rs`/`vst3/plugin.rs` (note ports): my changes there
  are one line in each `process` and the transport functions appended at the end of each
  file, plus `getProcessContextRequirements` — merge additively.
- Golden ids were blessed per unit (appended at the end of `ids.txt`); the eight stems keep
  their `out` lines.
- The shoot example `examples/shoot_modfx.rs` is mine (the generic `shoot_effects` maps
  unknown kinds to *External*'s size).
