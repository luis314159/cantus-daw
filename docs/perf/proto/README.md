# Prototypes for ADR 0031

Throwaway code that checks a design choice of `docs/adr/0031-performance-architecture.md`
before anyone builds it into the engine. Not workspace members (each has an empty
`[workspace]` table); std only, so they build without the package cache.

## `forkjoin/` — the scheduler of ADR 0031 §2

A static graph (tracks → buses → master) run by spinning or parked workers through a
Vyukov MPMC queue of node indices, in safe Rust; the "callback" (the main thread) arms
each block, helps, and spins only on nodes already running. Nodes gather their inputs in
canonical order into a +0.0 accumulator and run biquads.

Build and run (cargo works too; `rustc` avoids waiting on the package-cache lock when
other builders are compiling):

```
rustc -C opt-level=3 --edition 2021 src/main.rs -o fj.exe
start "" /high /affinity 555 /wait /b fj.exe          # one thread of each P-core (12700H)
set FJ_MAXW=4 & set FJ_ONLY=6 & fj.exe                # scheduling cost alone
```

`FJ_MAXW` caps the worker counts tried; `FJ_ONLY=6` runs only the bare-node test.

### Results (2026-10-05, i7-12700H, 6 P + 8 E cores, Windows 11)

**The machine was not quiet**: other round-20 builders were compiling (a dozen cargo and
rustc processes). A biquad sample cost about 14 ns, against about 4 ns on a quiet core, so
absolute times are roughly 3× too high and the tails include the neighbours. The ratios
are what the ADR uses. Raw output in `forkjoin/results/`:

- `run1-unpinned-loaded.txt` — no affinity, normal priority, count-based spin budget (the
  first version; its §4 shows why a spin budget must be a *time*).
- `run2-pcores-high.txt` — `/high /affinity 555` (logical CPUs 0, 2, 4, 6, 8, 10: one per
  P-core), time-based spin budget, at most 5 workers.
- `run3-bare-pcores-high.txt` — the same placement, nodes that do nothing.

| Measure | Result |
|---|---|
| Heavy graph, median speed-up over serial | 2 threads 1.98×, 3: 3.09×, 4: 3.52×, **5: 4.73×**, 6 (all pinned cores): 3.90× |
| Heavy graph, p99 per block | serial 6.7 ms; 5 threads 2.05 ms; 6 threads on 6 cores **97 ms** |
| Parallel = serial to the bit (500 blocks) | identical, 1/3/7/13 workers |
| Last worker sees a new block (median / p99) | spin 2.1 µs / 761 µs; park 55 µs / 315 µs (3 workers); spin with 5 workers + callback on 6 cores **17.7 ms** |
| Scheduling cost per node (nothing to do) | serial 51 ns; 1 worker 310 ns; 4 workers 527 ns |
| Copy through relaxed `AtomicU32` vs plain | 2.5–2.8 vs 0.6–0.9 ns per sample (write + read) |

### What it decided

1. **Parallel equals serial** with a gather in canonical order — no surprise, but now a
   test, not an argument.
2. **Spinners never outnumber their cores**: W = P-cores − 2 by default (ADR §2.1).
3. **Spin for a time, park only after silence**: a parked worker costs ~55 µs to wake,
   a spinning one ~2 µs (§2.4).
4. **Cluster light nodes** into tasks of ≥ 20 µs: contended dispatch costs ~0.3–0.5 µs
   per task (§2.5).
5. **Rings are not per-sample atomics**: 3–4× the cost of a copy; the single-writer
   ring belongs in `cantus-rt` with a Miri/loom-tested unsafe slice copy (§3.4).
6. **Seed every node without a predecessor**: the first version seeded only the tracks
   and hung on a bus nothing fed.

Not measured here (needs the real engine): MMCSS instead of a High process class, the
driver's callback thread instead of the main thread, real plug-in loads. Phase 1 of
`docs/perf/plan.md` measures those with `cantus-bench`.
