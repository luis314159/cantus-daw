# ADR 0038 — Voices on several threads: an instrument forks its voices to the pool, with the bits of one thread

**Status:** Accepted · **Date:** 2026-10-09 · **Round:** 24 (perf2) · **Extends:** ADR 0031
(the performance architecture: §2 the pool, §2.5 the serial hint, §2.8 the CLAP
`thread-pool` extension), ADR 0005 (real-time rules), ADR 0023 (native instruments)

## Context

Round 22 put the engine's graph on a real-time pool (ADR 0031 §2). A node is the unit the
pool schedules, and the pool cannot split one: a block ends no sooner than its heaviest node
plus the strip and the master chain after it. The paced runs at 256 frames
(`docs/round22/pool.md`) found every demo gated by **one instrument**: Calor on the startup
song's *Chords* (2.8 ms of a 5.33 ms period alone, serial), Unda on the ballad's *Pad*
(70 % of its block), Exemplar on the trap song's *Pad*, Calor in lofi, Unda in ambient.
`docs/BETA.md`'s blocker 2 — the demos with zero dropouts at 256 frames, under 70 % of the
budget — cannot be met by scheduling nodes better.

Inside those nodes the work is wide: a chord of polyphonic voices, each a few oscillators,
a filter and envelopes at 2× oversampling, and each **independent of the others between two
events** — a voice reads the instance's settings and writes only its own state. The serial
loop adds voice after voice into one buffer. Surge XT (`QuadFilterChain`) and Vital
(`poly_float`) vectorise across voices; Bitwig and Cubase run instruments' voices on
several cores. CLAP's `thread-pool` extension is the standard's version of the same idea:
the plug-in hands the host `n` tasks and the host's pool runs them (ADR 0031 §2.8).

Two constraints shape the design:

- **The bits may not move.** Every null test, golden hash and the parallel = serial
  property of ADR 0031 §1.3 hold to the bit; floating-point addition is not associative, so
  summing voices in groups — or in whatever order threads finish — changes the output.
- **The real-time rules** (ADR 0005): no allocation, lock, block or I/O on the audio thread
  or a worker; `cantus-engine` stays `forbid(unsafe_code)`.

## Decisions

### 1. One lane per voice, summed in voice order by one thread

A voice renders into **its own lane** (a buffer filled with **`−0.0`** first, one addition
per sample), and the instance then adds the lanes into its sum **in voice order**. This is
the serial loop's own sequence of additions, into any buffer: `−0.0` is the exact additive
identity (`−0 + x == x` for every `x`, `−0` and `+0` included), so a lane holds exactly
`vₖ`, and a sample its voice never wrote (the voice ended mid-lane) holds `−0`, which leaves
the sum as the serial loop leaves it. `+0.0` would not do: `+0 + (−0) = +0`, so a lane
starting at `+0` turns a voice's `−0` into `+0`, and adding that to a buffer holding `−0`
gives `+0` where the serial loop gives `−0`. (Calor and Unda sum into a scratch buffer they
zero themselves, where that cannot show; Exemplar adds its lanes straight into the caller's
buffer, and the review of round 24 found the difference there, on a buffer pre-filled with
`−0.0`.) Grouping voices into a partial sum first is **not** allowed — one lane per voice,
always.

When nobody can help ([`Parallel::width`] is 1) the instrument runs its serial loop
untouched: no lanes, no copies, no cost.

### 2. The contract: `cantus_dsp::par`

`cantus-dsp` stays dependency-free and safe. It defines:

- `trait Parallel: Sync { fn width(&self) -> usize; fn run(&self, jobs: &mut [Job]) }` —
  run every job once, possibly on other threads, return when all have returned; never
  allocate, lock or block (it may wait only for jobs already running). `Job<'a>` is
  `Option<&'a mut (dyn FnMut() + Send + 'a)>`: a closure borrowing the caller's stack.
- `Serial` (the caller alone), `Reversed` (tests: the jobs last-first), `run_each` (items
  to jobs on the stack, ≤ 64 per call, no allocation), `Lanes` (per-voice stereo buffers,
  allocated and pre-faulted with the instrument, ADR 0031 R11), `add_into`.
- `InstrumentUnit::process_par(left, right, &dyn Parallel)` (default: `process`) and
  `splits_voices()` (default `false`) on the engine's instrument contract.

### 3. The three heavy instruments split, each at its own grain

- **Calor**: per 64-frame chunk (its existing render unit), the sounding voices and ghosts
  render into 40 lanes of 256 oversampled samples (80 KiB).
- **Unda**: its serial loop walks the 16-sample control grid (the free LFOs and every
  voice's control at a grid point, then every voice's piece into one sum, then the
  decimator). The parallel render takes spans of 64 frames: the free LFOs are ticked for
  each grid point up front (they depend on nothing a voice does), each voice runs all the
  span's pieces into its lane, then piece by piece the lanes are summed exactly as the serial
  loop adds them, the decimator runs, and the allocator hears of the voices that ended in the
  order the serial loop told it (32 lanes × 256 samples, 64 KiB).
- **Exemplar**: per 128-frame span, each sounding layer (up to 64; with more it renders
  serially) into its lane; the layers that ended are told to the allocator in layer order at
  the end of the call, as the serial loop does. Its resampler's coefficient scratch, written
  before it is read, became per call (it was shared by the instance).

Splitting a call into spans is the block-size independence every unit already guarantees
(`inst_contract.rs`), so the spans change nothing either.

### 4. The fork: `cantus_rt::fork`, the one new `unsafe`

Lending a closure that borrows the forker's stack to a persistent worker thread is the one
thing safe Rust cannot say, so it lives in `cantus-rt` (ADR 0031 §2.7). `Forks` holds four
slots; a forker takes a free one by compare-and-swap, publishes the jobs' address and count,
claims jobs itself (one `fetch_add` per index: every index is claimed by exactly one thread,
which is what makes its `&mut` exclusive), then closes the slot and waits until no helper is
registered in it. A helper registers *before* it reads the slot's state (both `SeqCst`), so
one that arrives after the close sees it closed. **The forker does not return — not even by
unwinding — while anyone can reach its jobs**: a forker that panics first claims every job
nobody took, without running it, then waits; a helper that unwinds leaves its slot (guards).
With no free slot, one job, or nobody present, the forker runs the jobs itself, in order.

The protocol has four named windows (`cantus_rt::fork::ForkSite`): the forker between
opening its slot and claiming, a helper between registering and re-reading the state, any
thread between claiming a job and running it, the forker between marking the slot closing
and waiting. With cantus-rt's `fuzz` feature (which cantus-engine's `fuzz` turns on)
`Forks::set_hook` installs a callback called at each; the engine routes it to the schedule
fuzz's `Chaos` hook as `Site::Fork{Opened, Registered, Claimed, Closing}`, so the PCT stalls
and `Stall` land inside the protocol too. Without the feature the sites compile to nothing.

### 5. Who helps: the pool's idle threads, inside a block

`sched::Graph` gained two default methods: `help(slot) -> ns` — a slot with nothing ready
while the block is not over takes forked jobs (timed into its busy share) — and
`presence(slot, inside)` around the drain loop. `Forks::present()` is how many slots are in
the drain loop now; the engine's `PoolPar` reports it as the width. In a serial block
(`W = 0`, the adaptive rule's quiet blocks, a stem render) nobody is present and the
instruments run serially. A worker that parked between blocks is woken by the block's arm
as before; the fork never wakes anyone (no new system call, ADR 0005's amended rule 3
unchanged). The forker waits only for jobs that are running — the callback's rule.

### 6. The serial hint yields to a divisible node

ADR 0031 §2.5 runs the block alone when its critical path is over 80 % of the work (one
long chain): exactly the case of a song that *is* one heavy instrument. A track whose unit
`splits_voices()` **and** that sounded two voices or more at the end of some pass since the
last clustering marks the plan divisible (`TaskTopo::divisible`), and the hint does not
hold. The per-track flag is sticky until the next clustering reads it and clears it (every
half second of audio, or at a plan change), so a chord between two clusterings counts and a
track back to one voice lets the hint hold again. A heavy monophonic line keeps the hint —
there is nothing to split, so waking the pool would only cost (round 24's review: the first
version set the flag for every split-capable track, whatever it played). A chord that starts
while the hint holds runs serially until the next clustering (at most half a second; the
behaviour before this ADR). When the instrument is quiet the adaptive rule (under 25 % load
for 32 blocks) still makes the block serial.

### 7. A switch for measuring

`Processor::set_voice_split(bool)` (default on; `CANTUS_VOICE_SPLIT=0` turns it off) —
the same bits either way, so paired A/B runs can alternate on one processor
(`cantus-bench --example voice_ab`). A change re-clusters at the next block, so the "off"
side is the engine without the split, serial hint included, from its first block. `EngineStats::{voice_forks, voice_jobs_helped}` count
the forks and the voices other threads rendered.

## What the tests hold

- `cantus-rt/tests/fork.rs`: every job runs exactly once with three helpers over 2 000
  forks of 2–41 jobs; with nobody present the forker runs them in order and publishes
  nothing; six forkers at once on four slots each get their own jobs back; a job that
  panics on the forker neither hangs the fork nor lets a helper's job outlive it, and the
  slot is free again — the forker's job waits until a helper is inside one, so a helper is
  certainly mid-job when the forker unwinds, and `run` must have waited for it. Without the
  close's wait for helpers, all three tests that fork with helpers fail. With `--features
  fuzz`, threads stalled at random at the four `ForkSite`s, five forkers on four slots and
  three helpers, still run every job exactly once, and every site is reached.
  `tests/no_alloc.rs`: 1 000 forks of 16 jobs allocate nothing on the forker or a helper,
  and the helper ran some of them (a helper that never claims fails it).
- `cantus-engine/tests/inst_contract.rs`, `voices::*` for all seven native kinds:
  `process_par` with the jobs run backwards, and on 3–4 real threads, gives `process`'s
  bits at the defaults, under the bench's 32-voice chord and at four random settings, at
  fixed and random splits — and into a buffer that already holds `−0.0`, `+0.0` and music,
  against `process` adding into the same buffer (lanes starting at `+0.0` fail it, on
  Exemplar). Summing Calor's or Unda's lanes in reverse voice order, or Exemplar's in
  reverse layer order, fails it.
- `tests/pool.rs`: chords on Calor, Unda and Exemplar (three tracks, and Calor alone — the
  divisible serial hint) at W 1, 3 (parallel and adaptive) and 7 give the serial engine's
  bits, with forks published and voices rendered by other threads, and the same with the
  split off; with `--features fuzz`, the PCT stalls of `sched::fuzz` over forked voices
  (some landing at the `Fork*` sites) and a `Stall` at each of the four `Fork*` sites in
  every block, at W 3 and 7. A heavy one-voice line on each of the three keeps the serial
  hint and never forks; `set_voice_split` re-clusters at the next block, both ways.
- `tests/rt_memory.rs`, `forked_voices_never_allocate`: 300 blocks of chords on three
  workers, not one heap event on the callback or a worker.
- Every existing test, the null tests and goldens unchanged (`DSP_VERSION` stays 3).

## Consequences

- The demos' heaviest nodes now take a fraction of a period on a laptop's cores (measured in
  `docs/round24/perf2.md`); the block's floor is the strip and the master chain after the
  instrument, which are serial by nature.
- An instrument with two voices pays for a fork (a few hundred nanoseconds); one with one
  voice, or no helper present, pays nothing.
- More memory per instance: 80 KiB (Calor), 64 KiB (Unda), 64 KiB (Exemplar) of lanes.
- `cantus-rt` grew by one module (~250 lines; the crate stays under its 2 000-line rule).
- The fork protocol is tested by stress and stalls, not model-checked (no loom model, no
  Miri run yet). The stall sites widen the windows the safety argument names, but not all of
  them: removing the helper's state re-check still passes every test, because its only
  unsafe window — a helper reading a slot a new forker is filling, between two adjacent
  stores — has no site; and a weakened memory ordering would most likely pass on x86. A loom
  model of `Slot`/`Close` behind `cfg(loom)` is the next step for it.
- The same `Forks` can serve the CLAP `thread-pool` extension (§2.8): a hosted plug-in's
  `request_exec(n)` becomes `n` jobs. Not wired yet.
- Next: the other instruments (Modulus, Lamina, Organum) take the same pattern if the
  profiler shows them gating a song; SIMD across voices (ADR 0031 §5.3) composes with it.
