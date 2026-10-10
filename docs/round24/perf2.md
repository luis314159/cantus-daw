# Round 24 — perf2: voices on several threads

Branch: **`r24/perf2`** (worktree `daw-r24/perf2`, base master cead440), **not committed**
(the integrator takes the diff). Decision record: **ADR 0038** (and R17 in ADR 0031 pointing
to it). The ask: performance wave 2 (round 22's *next session* item 2, `docs/perf/plan.md`,
ADR 0031) for `docs/BETA.md` blocker 2 — the startup song and every demo with zero
dropouts at 256 frames, under 70 % of the budget — whose obstacle round 22 measured: each
demo gated by **one instrument** (Calor, Unda, Exemplar taking 50–70 % of a period alone),
which the pool cannot split.

## 1. What was built

**An instrument forks its voices to the pool, with the bits of one thread.** Between two
events a voice reads the instance's settings and writes only its own state, so the voices
of one instrument can render at the same time on several cores. Each renders into **its
own lane**; the instance adds the lanes into its sum **in voice order**, which is exactly
the serial loop's sequence of additions — each lane starts at `−0.0`, the exact additive
identity, so this holds into any buffer (ADR 0038 §1 has the argument; the review found the
first version's `+0.0` lanes wrong on Exemplar for a buffer holding `−0.0`, §6).
Grouping voices into partial sums is not allowed — one lane per voice.

| Layer | What | Where |
|---|---|---|
| The contract | `Parallel` (width, run jobs), `Job` (a closure borrowing the caller's stack), `Serial`, `Reversed` (tests), `run_each` (no allocation, ≤ 64 jobs per call), `Lanes` (pre-faulted per-voice buffers), `add_into` | `cantus-dsp/src/par.rs` (new, safe, dependency-free) |
| Calor | per 64-frame chunk, voices and ghosts into 40 lanes at the oversampled rate | `calor_render.rs`, `calor_synth.rs` |
| Unda | per 64-frame span: the free LFOs ticked for each grid point up front, each voice runs all the span's control pieces into its lane, then piece by piece the serial loop's sum, decimator and allocator calls, in its order | `unda_par.rs` (new), `unda_synth.rs` |
| Exemplar | per 128-frame span, each sounding layer (≤ 64) into its lane; ended layers told to the allocator in layer order at the end; the resampler's scratch per call (it was the instance's) | `exemplar_render.rs` (render split into `Ctx` + `render_into` + `control`), `exemplar.rs`, `exemplar_engine.rs` |
| The fork | `Forks`: four slots, claim by `fetch_add`, helpers register before reading the state, the forker returns only when no helper can reach its jobs (also when unwinding); `present()`, `help()`, `any_open()`, counters | `cantus-rt/src/fork.rs` (new; the one new `unsafe`, `// SAFETY:` on it) |
| The pool | `Graph::help(slot) -> ns` (idle slots take forked jobs, timed into their busy share) and `Graph::presence(slot, inside)` around the drain loop | `sched/mod.rs`, `sched/worker.rs` |
| The engine | `Nodes::forks`, `PoolPar` (width = slots present in the drain loop; serial with the switch off); the track node calls `process_par`; `InstrumentUnit::{process_par, splits_voices}` (defaults: `process`, `false`); the three units override them | `graph/exec.rs`, `graph/track.rs`, `inst/{mod,analog,wavetable,sampler}.rs` |
| The serial hint | a plan with a track whose unit splits **and** that had two voices or more since the last clustering is *divisible*: §2.5's "one long chain → serial" no longer holds it to one thread; a one-voice line keeps the hint | `graph/cluster.rs` (`TaskTopo::divisible`), `graph/track.rs`, `processor_pool.rs` |
| Measuring | `Processor::set_voice_split` / `CANTUS_VOICE_SPLIT=0` (same bits either way; a change re-clusters at the next block), `Processor::plan_held_serial`, `EngineStats::{voice_forks, voice_jobs_helped}` | `processor.rs`, `processor_pass.rs`, `messages.rs`, `graph/ctx.rs` |
| Fuzz sites in the fork | `ForkSite` (opened, registered, claimed, closing) and `Forks::set_hook` behind cantus-rt's new `fuzz` feature (cantus-engine's `fuzz` turns it on); the engine routes them to its `Chaos` hook as `Site::Fork*` | `cantus-rt/src/fork.rs`, `cantus-rt/Cargo.toml`, `sched/{mod,fuzz}.rs`, `processor_pool.rs`, `cantus-engine/Cargo.toml` |
| Bench | the `demos` group **over the whole song** (`demo.<song>.b256`, `.p99`, `.load` = the worst window's median, `.load99`; in the quick suite, ~17 s, so `xtask perf` shows the beta's bar); `examples/voice_ab.rs` (paired A/B on one processor, the same music on both sides, the serial engine beside it) | `cantus-bench/src/demos.rs` (new), `lib.rs`, `main.rs`, `examples/voice_ab.rs` (new) |
| CI | `cargo test -p cantus-engine --features fuzz` with `CANTUS_WORKERS=3`, and `cargo test -p cantus-rt --features fuzz --test fork`, after the workspace tests | `.github/workflows/ci.yml` |

`cantus-engine` stays `forbid(unsafe_code)`; `DSP_VERSION` stays 3 — no golden moved.
No new crate dependency, no schema change, no strings.

## 2. Decisions (ADR 0038)

1. **One lane per voice, summed in voice order by one thread**; the serial loop untouched
   when nobody can help (width 1): no lanes, no copies, no cost.
2. **The fork lives in `cantus-rt`**: lending a closure that borrows the forker's stack to a
   persistent worker is the one thing safe Rust cannot express. The protocol needs no
   `done` counter: a helper runs its jobs while registered in the slot, so "every index
   claimed and no helper registered" is "every job returned" (a first version had both; a
   mutation that removed the `done` wait passed every test, which is how it was found
   redundant and removed).
3. **Helpers are the pool's idle slots inside a block**; the fork never wakes a parked
   worker (no new system call). In a serial block nobody is present and nothing forks.
4. **The serial hint yields to a divisible node**: a song that *is* one heavy instrument
   is exactly what the other threads are for — while it has two voices or more to split
   (sticky until the next clustering reads it). A one-voice line keeps the hint. When the
   instrument is quiet, the adaptive rule (< 25 % for 32 blocks) still runs it serially.
5. **Grains**: Calor per 64-frame chunk (its render unit), Unda per 64-frame span, Exemplar
   per 128-frame span — a few forks per 256-frame block, each a few hundred nanoseconds
   against tens of microseconds of voice work. Spans are the block-size independence every
   unit already guarantees (`inst_contract.rs`).
6. **Not done this way**: summing in the order threads finish (not deterministic), a
   per-instance thread pool (oversubscribes the machine — the fork uses the engine's own
   pool), `std::thread::scope` (spawns: allocation and system calls on the audio thread).

## 3. Tests

Written after the first implementation, then **each one shown to fail against a deliberate
bug** (the "watch it fail" step, done by mutation):

- `cantus-rt/tests/fork.rs` (4, new): every job exactly once over 2 000 forks of 2–41 jobs
  with three helpers; nobody present → in order on the forker, nothing published; six
  forkers on four slots; a job panicking on the forker neither hangs the fork nor lets a
  helper's job outlive it, and the slot is free again (since the review the forker's job
  waits until a helper is inside one, so a helper is certainly mid-job). *Mutation: without
  the close's wait for helpers, all three that fork with helpers fail — the panic test
  among them, which the first version let pass.* With `--features fuzz` (+1,
  `stalls_inside_the_protocol_lose_or_repeat_no_job`): threads stalled at random at the
  four `ForkSite`s, five forkers on four slots and three helpers — every job exactly once,
  every site reached.
- `cantus-rt/tests/no_alloc.rs` (+1): 1 000 forks of 16 jobs with a helper — zero heap
  events on either thread, and the helper ran some jobs. *Mutation: a helper that never
  claims fails it (it passed before the review).*
- `cantus-engine/tests/inst_contract.rs`, `voices::*` (+7, every native kind):
  `process_par` with the jobs run backwards (`Reversed`) and on 3–4 real threads gives
  `process`'s bits — defaults, the bench's 32-voice chord, four random settings; blocks of
  512 / 256 and random splits — and (since the review) into a buffer that already holds
  `−0.0`, `+0.0` and small values, against `process` adding into the same buffer.
  *Mutations: Calor's or Unda's lanes summed in reverse voice order, Exemplar's in reverse
  layer order — each fails; lanes cleared to `+0.0` fail the dirty-buffer run on Exemplar
  (frame 16 909: `−0` against `+0`).*
- `cantus-engine/tests/pool.rs` (+2): chords on Calor, Unda and Exemplar (three tracks; and
  Calor alone, the divisible serial hint) at W 1, 3 (parallel and adaptive), 7 against the
  serial engine, to the bit, with forks published and voices rendered by other threads
  (e.g. W 3: 6 033 forks, 16 122 voices on helpers in 3 s of audio), and the same with the
  split off (no fork); with `--features fuzz`, the PCT stalls over forked voices at W 1, 3, 7
  (some now land at the `Fork*` sites, asserted) and a `Stall` at each of the four `Fork*`
  sites in every block at W 3 and 7 (108–121 stalls over ~2 080 forks each).
  `a_single_voice_line_keeps_the_serial_hint` (one note at a time on each of the three:
  the plan stays held to one thread, nothing forks) and
  `switching_the_split_reclusters_at_the_next_block` — both failed before the review's
  fixes.
- `cantus-engine/tests/rt_memory.rs` (+1), `forked_voices_never_allocate`: 300 blocks of
  chords on three workers, no heap event on the callback or a worker.

Results at the close: see *Final runs* below.

## 4. Measurements

**The machine.** This round ran on an **AMD Ryzen 7 7735HS** (8 cores / 16 threads; the
bench's class `windows-amd-ryzen-7-7735hs-with-radeon-graphics` — not the i7-12700H of
round 22, so no number here compares with round 22's), `Workers::auto()` = 6 workers.
**Every number below was taken while the machine was loaded** by two other builders
compiling (rustc/cargo processes running throughout) and a 2-hour soak in another
worktree; the CPU read 58 % busy before the first run. Hence pairs and interleaving
everywhere; single runs of the same configuration varied by up to 2× in their tails.

### The demos at 256 frames over the whole song (`cantus-bench --full --runs 2 --only demos`)

Three interleaved rounds (off, on / on, off / off, on), the same binary with the split off
(`CANTUS_VOICE_SPLIT=0`, the old path) and on; each side 2 runs × 16 windows of 1 s spread
over the song (32 points per song per side). **The machine was loaded** (CPU 56–100 % busy
from the other builders and the soak). `.load` is the worst window's median block as a
share of the 5.33 ms budget — what blocker 2 reads; one value per round:

| Song | Forks per side | `.load` off | **`.load` on** | p50 all blocks, µs, off → on | On/off p50 (median of rounds) |
|---|---:|---|---|---|---:|
| startup | ~124 000 | 52 / 50 / 55 % | **23 / 22 / 24 %** | 2 233–2 536 → 1 006–1 123 | ×0.42 |
| house | ~80 000 | 47 / 48 / 49 % | **23 / 25 / 25 %** | 2 132–2 310 → 1 017–1 063 | ×0.47 |
| ballad | ~73 000 | 50 / 61 / 59 % | **22 / 21 / 23 %** | 2 087–2 498 → 923–988 | ×0.39 |
| ambient | ~72 000 | 27 / 28 / 25 % | 26 / 24 / 27 % | 1 006–1 067 → 833–945 | ×0.87 |
| trap | ~27 000 | 22 / 31 / 28 % | 32 / 30 / 30 % | 1 039–1 120 → 920–1 118 | ~1 |
| lofi | ~7 000 | 23 / 26 / 24 % | 22 / 31 / 23 % | 801–1 094 → 805–975 | ~1 |
| funk | **0** | 18 / 28 / 27 % | 21 / 29 / 28 % | 874–1 147 → 924–1 178 | ~1 |

**With the split on, every song's worst window is at most 32 % of the budget** (the bar is
70 %); off, the worst were ballad (61 %) and startup (55 %). The split halves the block on
the three songs a splitting instrument dominates (startup, house, ballad), takes ~13 % off
ambient's median, and is lost in the noise on trap and lofi (few forks). **Funk never
forks**, over the whole song: its Calor solo plays one note at a time. The 99th
percentiles of all timed blocks are dominated by the machine's other load (off 63–279 %,
on 28–129 %; one round of each side's startup had a tail above the budget), so no claim is
made from them.

*What this replaces.* The first version of this section timed the first 1–7 s of each
song only (the review's finding): funk, lofi and house had **no fork at all** in that
window, so house's "29 → 13 %" there was machine noise, not the split. Over the whole song
house does split (above, and in the paired runs below).

### Paired A/B on the same music (`examples/voice_ab`, 256 frames, 2 s runs, 6 pairs)

Each pair seeks the split-on side, the split-off side and the serial engine to the same
point of the song (pair *k* at *k*/6 of it), half a second of warm-up, then times it.
µs per block (median of runs), and the median of the per-pair ratios. Rerun after the
review's fix to `set_voice_split` (it now re-clusters at the next block, so the "off" side
can no longer run with the serial hint still disabled); the machine at 100 % CPU when it
started (the other builders compiling):

| Song | Serial p50 | Off p50 | On p50 | On/off p50 | Off p99 | On p99 | On/off p99 | Voices on helpers /s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| startup | 2 504 | 2 073 | 1 050 | **0.50** | 3 027 | 1 397 | **0.47** | 11 952 |
| house | 2 394 | 1 961 | 923 | **0.48** | 2 941 | 1 180 | **0.45** | 10 532 |
| ballad | 2 070 | 1 967 | 904 | **0.46** | 5 003 | 1 571 | **0.41** | 3 539 |
| funk | 904 | 897 | 870 | 1.00 | 1 508 | 1 435 | 0.95 | 0 |
| lofi | 894 | 860 | 834 | 1.00 | 1 347 | 1 291 | 0.97 | 1 041 |
| ambient | 1 538 | 1 106 | 928 | **0.78** | 2 005 | 1 922 | 0.91 | 1 891 |
| trap | 1 080 | 1 094 | 1 059 | 0.99 | 1 880 | 1 995 | 1.05 | 1 067 |

The same picture as before the fix (startup 0.51, house 0.48, ballad 0.46, ambient 0.75;
trap 0.91 then, 0.99 now — within the ±10 % the no-fork songs show). Funk (no fork at all)
gives the noise floor. (A first version of this A/B ran its pairs at successive points of
the song, so the two sides played different music — house came out 1.27× "worse"; seeking
both sides to the same point fixed the method.)

### Base build against this branch (`examples/song_profile`, `CANTUS_WORKERS=auto`)

The base binary (built from cead440 before any edit) and this branch's, split off and on,
interleaved, three rounds, the first 6 s of each song, block p50 / p99 µs:

| Song | Base | Off | On |
|---|---|---|---|
| startup | 2 306 / 6 679 · 2 033 / 3 031 · 2 342 / 3 925 | 2 539 / 12 171 · 2 042 / 3 128 · 2 362 / 4 048 | **1 249 / 2 566 · 1 005 / 1 456 · 1 213 / 1 720** |
| ballad | 1 691 / 2 857 · 1 772 / 2 924 · 1 748 / 2 889 | 1 700 / 2 754 · 1 678 / 2 721 · 2 324 / 6 941 | **881 / 1 843 · 902 / 2 400 · 1 092 / 3 151** |
| trap | 1 131 / 2 026 · 1 256 / 2 632 · 1 206 / 2 264 | 1 161 / 2 330 · 1 209 / 2 388 · 1 135 / 2 419 | **968 / 1 898 · 937 / 1 793 · 920 / 1 976** |
| ambient | 1 000 / 1 653 · 1 107 / 1 830 · 1 048 / 1 788 | 990 / 1 546 · 1 128 / 1 806 · 1 320 / 2 123 | 995 / 1 594 · **797 / 1 248** · 1 112 / 1 818 |
| house | 774 / 1 264 · 682 / 1 202 · 776 / 1 270 | 740 / 1 213 · 675 / 1 000 · 909 / 1 801 | 793 / 1 310 · 681 / 962 · 762 / 1 254 |

Off is the base within the noise (the presence counters and the idle check cost nothing
measurable); house's first 6 s have no chords (no fork), ambient's first 6 s few.

### Paced at the device period (`cantus-bench paced --frames 256 --seconds 20`)

The live check: the base binary and this branch, interleaved, two rounds (the second in
the opposite order), the application's pool (6 promoted workers, MMCSS), 3 750 blocks per
song per run. Misses (a block that ended after its period) / block p50 / p99 µs:

| Song | Base r1 | **Branch r1** | Base r2 | **Branch r2** |
|---|---|---|---|---|
| startup | 1 / 2 286 / 3 364 | **0 / 1 185 / 1 596** | 0 / 2 472 / 3 679 | **0 / 1 389 / 1 837** |
| house | 1 / 1 116 / 3 212 | **0 / 1 151 / 1 660** | 0 / 1 391 / 3 575 | 0 / 1 377 / 3 586 |
| ballad | 0 / 2 390 / 3 165 | 2 / 1 148 / 2 666 | 1 / 2 712 / 3 892 | **0 / 1 586 / 3 801** |
| funk | 0 / 1 171 / 1 758 | 0 / 1 469 / 1 836 | 0 / 876 / 1 232 | 0 / 1 035 / 1 823 |
| lofi | 0 / 992 / 1 557 | 0 / 1 119 / 1 759 | 0 / 999 / 1 608 | 0 / 1 027 / 1 547 |
| ambient | 0 / 1 382 / 2 254 | **0 / 1 023 / 1 650** | 1 / 1 481 / 2 514 | **0 / 943 / 1 448** |
| trap | 0 / 1 386 / 2 445 | **0 / 1 134 / 1 750** | 0 / 1 511 / 2 670 | **0 / 1 252 / 1 815** |

Misses: base 4 in 52 500 blocks, branch 2 — both of the branch's in one ballad run whose
worst block was **33.4 ms** (the base's startup run had a 33.4 ms block too): a stall of
the whole machine, not a voice (the blame named a 0.3 ms node). The heaviest node's
average fell: Calor on Chords 1 644–1 820 → 592–734 µs, Unda on Pad (ballad) 1 617–1 769 →
585–1 125, Exemplar on Pad (trap) 780–819 → 456–471, Unda on Pad (ambient) 804–848 →
329–385. Two 20 s runs per song are not the 10-minute zero-miss proof BETA asks for; on
this loaded machine neither binary missed for a reason of its own. These runs start at bar
1 and cover each song's first 20 s only (the review's point about the demos group applies
here too; the whole-song sweep above covers the rest, back to back rather than paced).

## 5. Left

- **The 10-minute live run with 0 overruns** (BETA blocker 2's second half) — on a quiet
  machine, on the owner's laptop, with a real device. The numbers above say the engine
  has the headroom; the proof is a device run.
- **A quiet-machine `xtask perf --full --save`** for this machine class: never quiet this
  round. No baseline exists for the Ryzen class.
- **The CLAP `thread-pool` extension** (ADR 0031 §2.8): the same `Forks` can serve
  `request_exec(n)`; not wired.
- **Other instruments**: Modulus, Lamina, Organum, Gravis, Pulsus do not split (they did not
  gate a demo). The pattern is `par.rs` + a lane per voice.
- **A fast `log10` for the meters**: not done. The per-sample `20·log10` calls are in
  detectors that feed the audio (Terminus's `reduction_db(db(level))`, Porta's level), so
  a faster one is a `DSP_VERSION` bump; the meter-only ones (telemetry) run once per block
  and cost nothing measurable.
- **Help time accounting**: a helper adds its fork time to its slot's busy counter after
  `help` returns, which can land in the next block's work (the adaptive rule's load only).
- **`loom`/Miri for `fork.rs`**: not run (no nightly or `loom` on this machine). Since the
  review the protocol has stall sites (`ForkSite`) driven by its own stress test and the
  engine's fuzz, but they do not reach every window: **removing the helper's state
  re-check still passes every test** (run three times with stalls), because its one unsafe
  window — a helper reading a slot a new forker is filling, between two adjacent `Relaxed`
  stores — has no site, and a weakened memory ordering would likely pass on x86 anyway. A
  loom model of `Slot`/`Close` behind `cfg(loom)` is the tool for it.
- A preempted helper inside its few-instruction registration window delays its forker's
  close by a scheduler quantum (the same class as ADR 0031 R2's note on the queues).

## 6. The review (two adversarial reviewers, eight findings)

Each finding was checked against the code; where it was real a test was written first and
**watched fail**, then the code fixed.

| # | Finding | Verdict | What was done |
|---|---|---|---|
| 1 | Lanes start at `+0.0`, so `process_par` equals `process` only into a buffer with no `−0.0`; Exemplar adds its lanes straight into the caller's buffer | **Real** (minor: the engine zeroes with `+0.0`, so unreachable today) | `par::clear_lane` fills lanes with `−0.0`, the exact additive identity, in all three instruments; `inst_contract` `voices::*` now also plays into a dirty buffer — it failed on Exemplar (`−0` against `+0` at frame 16 909) before the fix; ADR 0038 §1 and `par.rs` rewritten |
| 2 | The schedule fuzz never stalls inside the fork protocol | **Real** | `ForkSite` + `Forks::set_hook` (cantus-rt `fuzz` feature, compiled out otherwise), routed to the engine's `Chaos` as `Site::Fork*`; the pool fuzz test asserts PCT points land there and stalls at each site every block (W 3, 7); a cantus-rt stress test with random stalls; CI runs it. Not caught still: removing the helper's re-check (see §5) — loom is the remaining step |
| 3 | The demos metric measures only the first seconds of each song | **Real** (major) | The group sweeps the whole song (windows spread from the first bar to the last, one-bar pre-roll); `.load` is the worst window's median; the note names its bar and the forks. Quick suite 17 s |
| 4 | The before/after table credits the split for house and ambient where that window had no forks | **Real** (major) | Re-measured whole-song, three interleaved rounds, forks per row (§4); the old table removed with a note on why it was wrong. Over the whole song house does split (×0.47); ambient ×0.87 |
| 5 | Any Calor/Unda/Exemplar track disables the 80 % serial hint, even with one voice | **Real** | The per-track flag is set only with two voices or more, sticky until the next clustering reads and clears it; `a_single_voice_line_keeps_the_serial_hint` failed before (on Calor), passes now |
| 6 | `set_voice_split` does not re-cluster | **Real** | It marks the map dirty on a change; `switching_the_split_reclusters_at_the_next_block` failed before; `voice_ab` rerun (§4) |
| 7 | The no-alloc fork test never checks that the helper ran a job | **Real** | Jobs spin briefly and the test asserts `counts().0 > 0`; a helper that never claims now fails it (it passed before) |
| 8 | The panic test can pass with no helper mid-job | **Real** | The forker's job waits until a helper is inside one, the helper works 5 ms; asserts a helper entered and finished before `run` unwound. Without the close's wait it now fails (it passed before) |

No finding was rejected.

## Final runs

All on the final code (after the review's fixes), the machine loaded as above:

- `cargo test -p cantus-rt --no-fail-fast`: **34 passed**; with `--features fuzz`: **35
  passed** (the stall stress test). `cargo test -p cantus-dsp`: **434 passed**, 6 ignored.
  `cargo test -p cantus-bench`: 7 passed.
- `cargo test -p cantus-engine --no-fail-fast` (W = 0): **387 passed, 0 failed**, 6 ignored.
- `CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz --no-fail-fast`: **391
  passed, 0 failed**, 6 ignored — the null tests and goldens (`DSP_VERSION` 3, unchanged),
  `pool.rs` with the fork-site stalls, `rt_memory.rs`, `inst_contract.rs` included.
- `cargo test --workspace --no-fail-fast`: **1 913 passed, 0 failed, 28 ignored**, plus the
  four binaries Windows will not start under their names (`audio_setup`,
  `audio_setup_first`, `audio_setup_flows`, `cantus_update`: os error 740), run copied to
  neutral names from their crate folders: **24 passed** — **1 937 passed, 0 failed** (round
  23 closed at 1 921; this round adds 16). The first close's one failure
  (`crash_recovery::killed_mid_recording…`, a child-process timing test) passed this time.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean;
  `cargo clippy -p cantus-engine -p cantus-rt --all-targets --features cantus-engine/fuzz
  -- -D warnings`: clean.
- `rustfmt` on the files changed here only (checked first with `--check` which files each
  would reach; none of the base's).

## Files outside my area

- `crates/cantus-rt/src/{fork.rs (new), lib.rs}`, `crates/cantus-rt/tests/{fork.rs (new),
  no_alloc.rs}` — the fork (the round's only `unsafe`); `crates/cantus-rt/Cargo.toml` — the
  `fuzz` feature (the fork's stall sites); `crates/cantus-engine/Cargo.toml` — its `fuzz`
  turns on cantus-rt's.
- `crates/cantus-engine/src/messages.rs` — two `EngineStats` counters (additive).
- `crates/cantus-bench/` — the `demos` group and `voice_ab`.
- `.github/workflows/ci.yml` — the fuzz line.
- `docs/perf/benchmark.md` (§2.2b), `docs/adr/0031-performance-architecture.md` (R17).

## Notes for the integrator

- **ADR 0038** (0037 left free for the other builders; renumber if they collided).
- Merge anywhere: the voice loops of Calor, Unda and Exemplar changed shape in
  `calor_render.rs`, `unda_synth.rs` (fields + `mod par`), `unda_par.rs`,
  `exemplar_render.rs` (rewritten around `Ctx`) — if *beta-rest* changed the instruments'
  output gains, those are in settings/output stages, not these lines; check
  `exemplar_render.rs` by hand if it touched Exemplar's render.
- The inst_contract `voices::*` tests take ~11 s; `pool.rs`'s new test ~5 s.
