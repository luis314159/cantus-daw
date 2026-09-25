# ADR 0018 — Delay compensation, and a contract every plug-in is built to

**Status:** Accepted · **Date:** 2026-09-24 · **Extends:** ADR 0005 (the audio thread), ADR 0017 (the plug-in family)

## Context

The user asked for every plug-in to be finished "to a quality equal to or better than
FabFilter", with one agent per plug-in working in parallel. Two things had to exist first:
a way for seven people's worth of work not to collide, and delay compensation — the
limiter's lookahead, a linear-phase EQ and a compressor's lookahead all delay their strip,
and without compensation a delayed strip smears against the rest of the mix and combs
against anything in parallel.

## Decisions

**A plug-in is owned by its files.** Each kind has one module per layer:
`cantus_core::fx::<kind>` (the parameter model: `params()`, `SPARSE`, `TAKES_SIDECHAIN`,
`NAME_KEY`), a DSP module in `cantus-dsp`, an `FxUnit` in `cantus_engine::fx::<kind>`, an
editor in `cantus_ui::views::fx::<kind>` and a locale file pair in `locales/fx/`.
`EffectKind` dispatches to them; nothing else learns a plug-in's name. Adding a plug-in is
a new module in each layer and one line in each dispatch.

**`FxUnit` is the engine's plug-in contract**: `configure(spec)` on the audio thread
without allocating, `process(l, r, key)`, `reset`, `latency()` in samples, and
`telemetry()` — 64 numbers for the editor, whose meaning is the plug-in's own and which
travel in `Meters::fx` for the effect whose editor is open. Aequalis and Comprimo predate
the trait and keep their own arms in `EffectUnit`, with the same methods.

**The frame is shared.** One window per open editor, and a header — the strip it sits on,
presets, A/B, bypass — written once in `views::fx`. A/B is kept by the host: switching
applies the other side's parameters as one command, so it is one step of the project's
undo (`research/fabfilter/family.md`, rule 3). Presets: each plug-in's factory list, and
the user's own as JSON per kind under the app's data folder.

**Compensation is per edge.** A source sent to the master and to a latent return needs its
master edge delayed and its send edge not; one delay per strip cannot express that. Once
per snapshot, on the audio thread, walking the strips in processing order: a strip's input
arrival `A` is the latest of what reaches it, it leaves at `P = A + own latency` (the
latency of its active inserts), and each edge from `src` into `dst` is delayed by
`A(dst) − P(src)`; tracks leave at zero; the master's arrival is the total latency, which
the engine reports (`EngineStats::latency_samples`). A slot mix around a latent effect
delays its dry path by the effect's latency, or the blend would comb.

**Lines from a pool, kept while unchanged.** 192 stereo lines of 8192 samples (170 ms at
48 kHz), allocated at start-up. An edge keeps its line across snapshots while its delay is
the same, so an edit elsewhere does not click; a line that still holds signal keeps its
track or strip running until it has drained. A line's ring runs at exactly its delay, so
its length is a ceiling and the memory it touches is what the delay needs.

**The pool grows on request, off the audio thread.** The EQ's linear phase reports up to
16 896 samples at 48 kHz (67 584 at 192 kHz), and one such strip makes every path in
parallel with it need a line that long; a fixed pool sized for that would hold a hundred
megabytes nobody uses. So when no free line is long enough, the edge is left
uncompensated for the moment and the audio thread asks (`EngineEvent::PdcNeed { count,
delay }`, sent once per shortfall); the backend allocates lines rounded up to a power of
two (`pdc::lines_for`) and sends them (`EngineMessage::PdcLines`); the audio thread swaps
them in for the shortest free lines, plans again, and hands the short ones back
(`EngineEvent::GarbageLines`) to be freed on the UI thread. Live, the gap is the few
milliseconds between the edit and the answer; an offline render settles it
(`Processor::settle`) before its first block, so a bounce is aligned from its first
sample. The ceiling is 2¹⁹ samples. An edge that still finds no line — every line held —
is left uncompensated and counted (`EngineStats::pdc_unserved`), and the status bar shows
it in red.

**Keys are not compensated.** A sidechain key is read from the key strip's post-fader
buffer as it is; delaying it would need a line per key and would only shift the detector
by the key strip's path latency. Written down so it is a decision and not a surprise.

**Bypass changes the latency.** A bypassed latent insert stops counting, so compensation
changes with it. Keeping the latency through a bypass (a "soft" bypass that delays the dry
signal) is the better behaviour and is left for when a plug-in with large latency ships.

## What the tests hold (`crates/cantus-engine/tests/pdc.rs`)

With a test unit that is a pure 100-sample delay: a track and its inverted twin, the twin
on a latent strip, cancel to **exact silence**; the latent strip alone and the
compensated twin alone are bit-identical; a slot mix at 50 % around the latent unit is
bit-identical to 100 % (the dry path waits); a latent return fed by a send lines up with
the dry signal to better than −60 dB; the engine reports 100 samples; and a project with
no latency renders exactly as before compensation existed. With a delay of 16 884 samples,
longer than every start-up line, the twins cancel exactly from the first sample when
settled offline, and exactly after one latency live, once the lines have arrived.

## Consequences

- New: `cantus_engine::pdc`, `EffectUnit::{latency, from_fx, telemetry}`,
  `cantus_engine::fx::{FxUnit, Telemetry}`, `EngineStats::{latency_samples, pdc_unserved}`,
  `views::fx` (frame, A/B, presets, `param_knob`), five kinds (`Limiter`, `Space`,
  `Multiband`, `DeEsser`, `Gate`), schema 13.
- The playhead is not moved by the total latency: the output lags the displayed position
  by `latency_samples`. Compensating the display (and recording) is follow-up work.
