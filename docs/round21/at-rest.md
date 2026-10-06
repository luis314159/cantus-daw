# Round 21 — at rest: the exact predicate, unit by unit

*Builder: at-rest (`r21/at-rest`). ADR 0031 §5.1 and risk 1; `docs/perf/plan.md` Phase 0.*

## What was asked, and what "at rest" means here

`FxUnit::at_rest`, `InstrumentUnit::at_rest` and `EffectUnit::at_rest` (default `false`),
implemented for every native unit by reading its real state, and the tests that make the
graph builder's per-node segmentation and suspension safe.

Reading the units showed that "the state equals a freshly reset unit's" is not one property
but two, and that a third method is needed for either to be exact. The contract, as written
on the traits (`crates/cantus-engine/src/fx/mod.rs`, `inst/mod.rs`):

1. **Silence in, silence out, still at rest.** With silent input (silent key, no note) the
   unit outputs exact zeros (+0.0) and stays at rest.
2. **Suspension is exact.** Not calling `process` for those samples — telling the unit with
   the new **`skip(frames)`** — changes no sample of what follows, *whatever input and
   whatever parameter changes come next*.
3. **As good as new.** It renders exactly what a freshly reset (or new) unit with the same
   configuration renders, once that unit is `skip`ped as far as this one has run — for any
   input, **as long as the parameters stay**.

`skip` exists because several units count samples and the count is in their output's
bits: the delay's write position (its fractional read head is worked out from the absolute
position — skipping 300 samples without moving it changed bits in the first twin test), the
grids that Latitudo, Fervor, Multiplex, Aequalis, Sibilus and the instruments update their
coefficients on, the linear-phase convolvers' block position (a partitioned convolution
rounds differently depending on where its blocks fall), Terminus's and Culmen's 10 ms
columns, Comprimo's 100 ms loudness steps, Lamina's tremolo and Calor's chorus (LFOs that
follow the sample clock, so they *can* rest while they turn). `skip` is O(1) for most and
loops per block for the linear-phase EQ (a design that was due is made where processing
would have made it).

Why (3) is limited to unchanged parameters: Terminus's and Culmen's Match monitor and
Comprimo's compare hold their level through silence; a new unit's is neutral. They reach
the audio only in the monitor modes that use them, so the unit renders like a new one until
a parameter switches that mode on — then the running unit (correctly) remembers. Suspension
(2) is still exact then, because the skipped unit remembers the same.

**Recommendation to the graph builder** (also in `requests.md`): suspend a node whose units
are all at rest and whose gathered input, key and notes are silent; while suspended call
`skip(pass_frames)` on each unit, once per pass; on wake call nothing (a `reset` is exact
too, but only while the parameters have not changed — prefer no reset).

**Zero means +0.0, bit for bit.** A state that holds −0.0 (the last input was −0.0) can
turn the next −0.0 into −0.0 where a new unit gives +0.0 — found by the twin test in
Terminus's DC blocker. Every check compares bits (`cantus_dsp::rest::{z, z64, all_zero}`).
The engine's gathered silence is +0.0 (ADR §1.3), so units that hear it come to rest.

**Cheap.** Long lines are never scanned when the engine asks: a `ZeroRun` beside a line
counts the zeros written into it in a row, updated after each block from the end of what
was written (one comparison while sound passes), or the unit counts zero input frames.
Shorter state (filters, envelopes, a 128-tap history) is compared directly, cheap checks
first. `rest_blocker()` (on `EffectUnit`, `FxUnit` and the big DSP types) names the first
condition that fails — for the tests and for a profiler that wants to say why a node runs.

## The units

(See the table in the report section below; it is filled from the twin tests' output.)

## Tests

- `tests/support/heavy.rs`: the bench's heavy settings (copied from
  `crates/cantus-bench/src/kernels.rs`), its signal, settings drawn at random over every
  parameter's range, a small RNG, and a test-only FTZ/DAZ guard (`Flush`).
- `tests/fx_contract.rs`, `tests/inst_contract.rs`, per kind (one test each, so they run in
  parallel):
  - **split invariance** of the unit alone: blocks of 1, 7, 4096 and two random split
    sequences (blocks 1…4096, weighted to short) against blocks of 512, bit for bit, at the
    defaults, the heavy settings and six random settings, keyed and not, with notes for the
    kinds that hear them; instruments with notes, chokes, controllers and per-note
    expressions at their samples, and the bench's 32-voice chord;
  - **the twin test**: feed (random length, random splits), run silent until `at_rest()`;
    then a twin that keeps processing zeros (exact +0.0 out, still at rest), a twin that is
    skipped (`skip`), a twin that is reset and a new one (both skipped as far as the first
    has run) must render the same bits for what comes next, each at its own random splits;
    a third trial moves the parameters at the wake (the skipped twin must still agree).
    Trials run without and with FTZ/DAZ (as the engine's `process` will run); rest is
    required, where the kind can rest, with FTZ;
  - the project-level test at the heavy settings at blocks 1, 64, 1000 and 4096.
- `cantus-dsp::rest`'s own tests: `ZeroRun` against a cell-by-cell reference at every block
  size, including blocks longer than the ring.

## Findings

(Filled in below.)
