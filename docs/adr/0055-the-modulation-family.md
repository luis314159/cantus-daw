# ADR 0055 — The modulation family: a clock on the song position, eight effects, and the host's transport in the exports

**Status:** Accepted · **Date:** 2026-10-14 · **Extends:** ADR 0017 (the plug-in family), ADR 0018 (the contract), ADR 0020 (the exported plug-ins), ADR 0031 (exact rest)

## Context

The owner asked (BACKLOG §3b) for the modulation effects a top DAW ships — chorus, flanger,
phaser, tremolo and auto-pan, vibrato, ring modulator, frequency shifter, an LFO Tool-style
volume shaper — each first-rank: Juno and Dimension chorus modes with a mono check,
through-zero flanging whose centre null holds to the bit, a phaser of 2–12 stages, synced
tremolo with swing, vibrato in cents with a rise, a band-limited ring modulator with no DC
leak, a single-sideband shifter with ≥ 60 dB rejection over 20 Hz–20 kHz, and a drawn volume
curve. Every one of them is built around an LFO, and three rules of the family collide with
a naive LFO:

- **Block-size independence at tolerance 0** — a phase accumulated per block rounds
  differently per split.
- **Offline = real time, and a render from bar 5 = bars 5… of a full render** — an LFO that
  starts when the unit is created is somewhere else in each.
- **Exact rest** (ADR 0031 §5.1) — "a free-running LFO is never at rest", so a unit with one
  could never be skipped.

The exported plug-ins also never told their units where the host's song was
(`cantus-plugins` never called `set_position`), so nothing synced could follow another DAW.

## Decisions

**A clock of integers** (`cantus_dsp::modfx_clock::Clock`). Two times: the **song sample**
(the transport's position in samples at the current tempo) and the **free sample** (samples
since playback started). The engine's `SongPosition` arrives once per pass; the clock takes
a new anchor only when the song **jumps** — a seek, a loop's wrap, a tempo change, the start
of playback — never because a pass split the stream (a disagreement of up to 2 samples is
the transport's rounding, not a jump). While stopped it keeps counting, so a synced chorus
does not freeze. Integers cannot round differently per split.

**An LFO that is a function of the clock** (`modfx_lfo::Lfo`). Synced: phase = song sample ×
cycles per sample + offset — a pure function of the song position, so bar 5 is bar 5 and
offline is live. Free: phase from the start of playback; a rate change re-anchors where the
wave is, so it never jumps. Six shapes, all starting at their middle rising; the stepped ones
(square, saws, random) have a **soft edge** so a tremolo never clicks; random steps are a
hash of a seed and the cycle's number; swing (0 … 100 %, an MPC's 50–75 %) on synced rates. Seventeen note
values (8/1 … 1/32, dotted and triplet) in one shared table.

**Rest is exact because the LFO is not state.** A unit's audio state (lines, filters,
feedback) is what `at_rest` checks; its clock is a counter that `skip` advances. Where a
unit's internal phase is a running sum (Anulus's and Translatio's carriers, Fluctus's level
glide), `skip` runs it sample by sample exactly as processing silence would, and the start
of playback (a new epoch of the clock) restarts it — the carriers at phase 0, Fluctus's
level on its curve — so live playback after any idle time is the offline render (review 1).
A filter a setting stops using (Tremor's crossover leaving Harmonic, Phasis's stages above
the count, Concentus's tone opened) is cleared when it stops, so it never holds the unit
out of rest. Small IIR states
are flushed to +0.0 after a run of 64 silent input samples, per sample (so where it happens
does not depend on the blocks); a product of silence by a negative carrier is −0.0, which is
normalised to +0.0. Every unit passes the twin tests of `fx_contract.rs` at its defaults,
heavy and random settings.

**Glides that land.** `modfx_line::Glide` lands on its target within a snap — or when its
step rounds away in `f32` (a delay of a hundred samples gliding by 1/2880 a sample stalls
long before 1e-4): without that a unit would never say it is at rest.

**Every default is a null, the amount control at 0.** Following the contract's rule and
Fervor's precedent, each unit is inserted doing nothing: Concentus, Flexus, Phasis, Anulus,
Translatio at *mix 0* (Flexus with through zero off, also its default: switched on at mix
0 it is the input 5 ms later, which delay compensation takes back); Tremor, Vibrans,
Fluctus at *depth 0*. Everything else defaults to a
musical value (Juno I, a slow triangle sweep, the classic pump), so one control — or a
factory preset — brings it in. Each editor says so on its display ("Mix is 0 %: the sound
passes untouched"). Secondary controls at their defaults add nothing (feedback 0, output
0 dB skipped exactly).

**The units** (each a DSP module, an engine unit, a kit editor, presets, a page in
`docs/plugins/`):

| Unit | Kind | The first-rank part |
|---|---|---|
| Concentus | Chorus | Juno I / II / I+II from the circuit's published sweeps (0.513 / 0.863 Hz over 1.66–5.35 ms; 9.75 Hz over 3.3–3.7 ms, the right channel's sweep inverted); Dimension 1–4 (our voicing: crossed, out-of-phase lines); 1–4 free voices; a BBD-dark wet path; a mono check (L/R correlation, the mono sum's loss) in the telemetry |
| Flexus | Flanger | through zero: the dry from a fixed 5 ms line, the wet sweeping either side; the plain flanger has no latency, so the reported latency moves with the switch — the engine re-plans delay compensation when an automated switch moves it (ADR 0056's `replan_if_latency_moved`; an exported plug-in asks its host to restart), and the defaults stay a wire to the bit; at the centre the wet is read at a whole sample, the same sample, so inverted at 50 % the output is exact zeros; LFO / envelope / manual sweeps; feedback ±95 % |
| Phasis | Phaser | 2–12 TPT first-order allpasses on one log-swept frequency, feedback ±95 %, the pedals as presets |
| Tremor | Tremolo | tremolo, auto-pan as a balance (unity at the centre and on the near side, the equal-power gain on the far side: no channel raised), harmonic tremolo (lows against highs) |
| Vibrans | Vibrato | depth in cents (the sweep follows rate and shape), a rise after each silence, delay up to 100 ms |
| Anulus | Ring mod | sine / triangle / square carriers band-limited by polyBLEP and polyBLAMP residuals, an LFO in semitones, a DC blocker on the product |
| Translatio | Freq shift | single sideband on a Hilbert pair designed per sample rate (below); up / down / both; feedback kept inside the pair's 90° band (25 Hz – 16 kHz) so 95 % stays bounded |
| Fluctus | Volume shaper | up to 16 points shaped by the automation editor's own `Shape::at`, a note-value cycle on the song position, a smoothing glide |

**The Hilbert pair is designed, not tabulated** (`modfx_hilbert`). The polyphase half-band
design of Valenzuela & Constantinides turned into a 90° splitter by `z → j·z`; coefficients
from the closed-form elliptic series for a transition of 0.8 × 20 Hz / fs, as many sections as
reach 66 dB over 20 Hz–20 kHz (or 0.45 fs). Measured: 14 sections, 69.5 dB designed at 44.1
kHz and 68.7 dB at 48 kHz; 69.2 dB worst through the running `f32` shifter at ten
frequencies from 20 Hz to 19.9 kHz at both rates. Our implementation of the published maths
(MPL-2.0 crate, no code copied).

**Shared code where it is the family's.** DSP in `cantus-dsp/src/modfx_{clock,lfo,line,
hilbert}.rs`; the engine's `SongPosition` → clock conversion and the LFO keys
(`sync/div/rate/shape/edge/swing/offset`) in `cantus-engine/src/fx/modfx.rs`; the editors'
LFO card, movement display (what the LFO does over two cycles, from the same settings the
DSP reads, with a playhead from the telemetry), mode card and preview strip in
`cantus-ui/src/views/fx/modfx.rs`. The family's words (`modfx.*`) live once, in Tremor's
locale files. Telemetry starts with a common header (`modfx_clock::tel`).

**Every picture is the DSP's arithmetic.** Concentus's delays, Flexus's comb (the Hermite
read and the feedback loop modelled), Phasis's response, Anulus's and Translatio's partials,
Vibrans's pitch and Fluctus's level are computed from the settings the DSP reads, and a test
per editor measures the running DSP against the drawing.

**The host's transport in the exports.** `cantus_plugins::instance::HostTransport` is the
format-free transport (playing, tempo, beats, bar, metre, loop — each only when the host says
it is valid). CLAP reads `clap_process.transport` (`clap_event_transport`, beat times in
`CLAP_BEATTIME_FACTOR`); VST3 reads `ProcessData.processContext` by its state flags and now
asks for it (`getProcessContextRequirements`: tempo, project time, transport state, bar
position, time signature, cycle). The processor turns it into the engine's `SongPosition`
at the start of every chunk it runs — the block's start in samples at the host's tempo plus
whole samples on, so the same integers whatever the host cuts — and calls `set_position`;
with no transport the units keep their own time. Review 1 completed it for real hosts: a
host with no beat timeline gives the song in seconds (CLAP's seconds timeline, VST3's
`projectTimeSamples`), read at its tempo; beats without a tempo give the tempo from how far
the beats moved since the last block; CLAP's `CLAP_EVENT_TRANSPORT` events inside a block
(up to 16) cut the chunks where they land; a loop that wraps inside a block cuts the chunk at
the wrap and goes on from the loop's start, as the engine never lets a pass cross one; and
each chunk's place is counted once. The editor's `transport()` (bar lines, playheads) reads
the host's position at the end of the block, the tempo as the units play it, and no playhead
when the host gives no timeline, from atomics the audio thread publishes. This closes the gap for every
synced unit, not only the family's.

## Consequences

- New: `cantus_dsp::{modfx_clock, modfx_lfo, modfx_line, modfx_hilbert}` and the eight units'
  modules; `cantus_engine::fx::modfx`; `cantus_ui::views::fx::modfx`; `HostTransport`,
  `Processor::set_transport`, `Shared::host_transport`; `examples/shoot_modfx.rs`.
- Shared files touched additively: the dispatch lines the contract left (heavy settings,
  `must_rest`, plug-in test settings and direct arms, automation keys, golden ids appended);
  `cantus_core::fx::readout` and `controls::write_readout` print values under 10 Hz with
  hundredths (an LFO's 0.25 Hz was "0 Hz"); `cantus_plugins::params::band_of` groups
  Fluctus's points; one `mod` line each in `cantus-dsp/src/lib.rs`, `fx/mod.rs` (engine)
  and `views/fx/mod.rs`.
- Anulus's carrier is set by frequency (shown as a note), not by a track's notes: the
  exported plug-ins' note input is Tonus's work this round (ADR 0053); following notes is a
  small step once it lands.
- No golden hash moved; `DSP_VERSION` unchanged (the new units are new sounds).

## What the tests hold

Per unit, `crates/cantus-engine/tests/fx_<stem>.rs` (null at defaults and at the amount 0,
block sizes 1–4 096 at tolerance 0, live = offline), the DSP's own tests (`<stem>_tests.rs`:
the measurements above), the editor's tests (`crates/cantus-ui/tests/fx_<stem>.rs`: words,
gestures as undo steps, the drawing against the DSP, presets in both languages), the family's
contract (`fx_contract.rs`) and the CLAP/VST3 export (bit-identical, automation on the grid).
The clock and LFO: split invariance, bar 5 = bar 5, free from playback, swing, shapes
continuous. The transport: `crates/cantus-plugins/tests/transport.rs` and the VST3 test — a
synced Tremor gate follows a host at 90 BPM from beat 2.5, bit-identical to the engine at
every block size, re-anchors on a jump back, and the editor reads the host's tempo.
