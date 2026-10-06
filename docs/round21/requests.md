# Round 21 — requests between builders

## From at-rest to graph (suspension, ADR 0031 §5.1 — Phase 2's, but the API lands now)

No change of `processor*.rs` or `graph/` is needed for this branch to merge: `at_rest`
defaults to `false` and nothing calls it yet. When the graph suspends nodes:

1. **Suspend** a node whose units all say `at_rest()`, whose gathered input and key are
   all-zero this pass (ADR §1.3 already knows: every contribution was skipped), with no note
   or controller arriving, and whose own delay lines are empty.
2. **While suspended, call `skip(pass_frames)` on every unit of the node, once per pass**
   (`EffectUnit::skip`, `InstrumentUnit::skip`). Several units count samples and the count is
   in their bits — the delay's fractional read head, the coefficient grids, the linear-phase
   convolvers' block position, LFOs that run on the sample clock. `skip` is cheap (O(1) for
   most; a linear-phase EQ walks its blocks) and never allocates.
3. **On wake, call nothing.** A suspended unit that was skipped is exactly a unit that
   processed silence, whatever parameter changes come with the wake. `reset` + nothing is
   exact too, but only while the parameters have not changed (a monitor's memory and the
   counters a reset zeroes) — prefer no reset.
4. Ask once per block at most: `at_rest` is a few comparisons for most units, a few hundred
   for the ones with short histories, never a scan of a long line.
5. `EffectUnit::rest_blocker()` (and `FxUnit::rest_blocker`) names why a unit is not at
   rest — for the profiler's blame, if useful.
6. The track slot's built-in instruments: `cantus_dsp::Firmus::at_rest()` exists (nothing to
   skip). Pulsus has none — its live hits and its hi-hat chick are seeded by a hit counter a
   new kit starts at zero (see `at-rest.md`).
7. FTZ/DAZ inside `process` (yours) is what lets some units reach rest at all: Aequalis's
   detector envelopes and a few f64 loudness integrators decay by multiplication alone and
   stop in the subnormals without it. They are exact either way; they only rest with it.
