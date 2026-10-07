# Round 21 — graph (Phase 0: the processor as a graph, still on one thread)

*Builder: **graph**, branch `r21/graph` (base `49f02f3`). ADR 0031 §1, §5.2, R1;
`docs/perf/plan.md` Phase 0.*

## What changed

**The processor is a graph.** `processor.rs` is now the control phase and the glue —
messages, MIDI, the input ring, recording marks, the transport, snapshots, delay
compensation — and each block is cut into **passes** only where everything must stop at
once: a loop wrap, an automation control point, and every `PASS_FRAMES` (1 024) frames.
A live block is one pass. In each pass every node runs once, in the plan's order:

| Node | File | Runs |
|---|---|---|
| Track | `graph/track.rs` | splits its pass **only at its own events**; instrument, clips and monitored input, level and balance, its line; writes its output buffer, its peak, its stem, its notes into its **outbox** |
| Strip | `graph/strip.rs`, `graph/strip_run.rs` | **whole pass**: gathers its inputs in canonical order, then polarity → swap → inserts → EQ → width → pan → fader; leaves its post-chain pair and gains, its output edge, each delayed send's line, its key, its take |
| Master | `graph/master.rs`, `graph/master_run.rs` | **whole pass**: gathers what feeds the master, its chain, its strip, the take; the click splits itself at its beats |

The serial engine cut *every* node at *every* event of *every* track ("global segments");
a strip's chain ran once per segment. Now each strip chain runs once per block, and each
instrument's calls are cut only at its own notes.

**The plan** (`graph/plan.rs`, `plan_compile.rs`): nodes, the order, each node's
**canonical input list**, successor lists and predecessor counts (audio, key and note
edges), the counts refused past the limits, and a `topology_key` — a hash of exactly what
the plan reads (ids, routes, send targets, keys, `midi_from`, the order), never a level.
`EngineHandle::set_snapshot` compiles it on the UI thread **only when the key differs
from the last snapshot sent** and sends `EngineMessage::SetSnapshotPlanned(snapshot,
plan)`; otherwise the plain `SetSnapshot`. The old plan comes back as
`EngineEvent::GarbagePlan`. A snapshot that arrives without its plan (every existing test
sends them by hand, and a full queue can drop one) is compiled **in place** into the
plan the processor holds — same code, allocation-free, counted in
`EngineStats::plans_compiled`. `render.rs` and `stems.rs` send the plan.

**The canonical gather** (`graph/gather.rs`): an accumulator that starts at +0.0 and
takes its contributions in the plan's order — the tracks that feed it, then for each strip
in the order its output edge and its sends `k = 0, 1, …`; on a mono device the master adds
left then right. That is the order the serial engine scattered in, so the sums are the
same bits; who runs which node and when cannot change a sample (the next phase's promise).
Every serial-engine routing rule is kept, including the ones only a corrupt file meets (a
route past the limit goes to the master, a send into itself or past the sixteenth is
dropped, a contribution into a strip already mixed is lost).

**Note outboxes** (`graph/notes.rs`): a track writes the notes it plays (and the control
phase the live notes and all-notes-off) into its own ring, stamped with the absolute
sample; each listening insert keeps a cursor and its delay and takes the notes due inside
its call at `due − pass start`. Same delivery as the shared queue (order, lateness,
bypassed inserts, past-due pruning); no shared writer. A re-plan keeps each listener's
cursor.

**FTZ/DAZ inside `process`** (§5.2): the guard is set at entry and the caller's register
restored at exit, through `graph/rt_shim.rs` (see "Notes for the integrator").

**Limits 64 → 256** tracks and strips. A snapshot with more plays 256 and reports the
rest in `EngineStats::refused_tracks` / `refused_strips` — nothing is cut silently any
more. Node buffers are `PASS_FRAMES` long (not `MAX_BLOCK_FRAMES`), so 256 strips cost
less memory than 64 did. `cantus-project`'s mirrored limits (DAWproject import, MIDI
import default) follow.

**R1**: the first block after a stream start calls `promote_callback(host, period)`
(`Processor::promote_on_first_block`, armed in `EngineHandle::start` with the driver's
name); counted in `EngineStats::wakes`. **R11**: every node buffer is touched once in
`Processor::new`, on the thread that builds it, so the audio thread takes no first fault
on them.

## Measurements

**The machine was not quiet.** Five other builders compiled and tested on the same laptop
the whole afternoon (cargo's package-cache lock queued every build), so a pinned P-core
(logical CPU 2, `ProcessorAffinity = 4`, High priority) still shared its hyperthread
sibling and its clock with them: the same binary measured the 64-track session anywhere
from 15 to 61 ms per block. Whole-suite runs taken an hour apart are therefore not
comparable, and the numbers below are **paired**: the base (`49f02f3`) and this branch
built from the same `cantus-bench` code, run alternately, back to back, on the same core.

*Paired A/B* (`cantus_bench::rig::Rig`, `sessions::scaled(n)`, one second of warm-up,
then 3–4 s of audio per run; a small driver built twice, not committed; median block µs):

| Session | Pairs | Base median of runs | Graph median of runs | Graph faster in | Ratio of medians | Ratio of best runs |
|---|---:|---:|---:|---:|---:|---:|
| `engine.s64.b128` | 8 | 30 851 | 26 560 | 7 of 8 | **0.86×** | **0.86×** (13 709 / 15 926) |
| `engine.s32.b128` | 5 | 15 545 | 9 991 | 4 of 5 | **0.64×** | 0.68× (8 089 / 11 911) |
| `engine.s64.b64` | 5 | 21 383 | 8 942 | 3 of 5 | (noise) | 1.35× (6 877 / 5 078) |

Per pair, s64.b128: 0.74, 1.75, 0.73, 0.87, 0.86, 0.79, 0.91, 0.55 (median 0.83). The
target, `engine.s64.b128` ≤ 0.9×, is met on every summary of the paired runs; the one
pair the other way is the base's best run against one of the graph's worst. At 64 frames
blocks are already short (fewer events fall inside one), the gain is smaller and the
noise swamps it: no claim.

*Whole engine suite* (`cantus-bench --full --only engine --runs 3`, pinned; three rounds
each, alternating; `engine.<…>` median / `.cpu`, µs per block):

| Round | s10.b128 | s32.b128 | s64.b128 | s100.b128 | s64.b64 |
|---|---|---|---|---|---|
| base 1 | 3 698 / 3 719 | 7 772 / 7 729 | 41 115 / 40 333 | 42 947 / 34 302 *(64 of 100 tracks)* | 10 465 / 14 677 |
| graph 1 | 3 245 / 3 167 | 4 589 / 4 646 | 33 238 / 30 719 | 22 271 / 33 229 *(all 100)* | 20 489 / 20 505 |
| base 2 | 3 389 / 3 438 | 16 403 / 16 469 | 17 670 / 20 885 | 37 203 / 37 292 | 8 613 / 9 859 |
| graph 2 | 3 438 / 4 875 | 15 639 / 16 146 | 20 409 / 25 604 | 63 306 / 54 188 | 29 410 / 26 521 |
| base 3 | 5 852 / 6 083 | 12 517 / 13 562 | 30 391 / 29 854 | 31 658 / 31 281 | 16 257 / 15 766 |
| graph 3 | 3 553 / 3 615 | 9 919 / 10 125 | 28 481 / 28 562 | 24 532 / 31 438 | 14 640 / 14 427 |

The JSON reports are in `target/bench-base/base{1,2,3}.json` and
`target/bench-new/new{1,2,3}.json` of this worktree (not committed). Read with the
spread: the run-to-run p10–p90 of a single suite run is often wider than the difference.
Re-measure on a quiet machine with `cargo run -p xtask -- perf --full --pin 2` before
moving the gate's baseline.

**`engine.s100.*` plays all 100 tracks**: 274 voices at the end of the run against the
base's 194 (it played the first 64), and the report no longer says "plays 64 of 100".

## Tests

`cargo test -p cantus-engine`: **every existing test passes unchanged** — 304 passed, 0
failed, 5 ignored (the hardware ones) across 47 test binaries — including `null_tests.rs`
with the `v5.cantus` SHA-256 **unchanged, with FTZ/DAZ on**, `pdc.rs` (twins at exact
silence), `midi_fx.rs` (notes now by outbox), `heard.rs`, `stems.rs`, `master_chain.rs`,
`recording*.rs`, `automation.rs`, the `fx_contract`/`inst_contract` contracts, and
`rt_memory.rs` (zero allocations — the in-place plan compile included, since every test
sends bare snapshots). `cargo check --workspace --all-targets`: clean, no warnings.

New:

- `tests/graph_order.rs` — 3 240 random topologies (tracks, strips, outputs, sends —
  including into itself, past the limit, past the sixteenth, into an earlier strip —,
  keys, `midi_from`, mutes, delayed edges, strips that rest, orders that leave strips out,
  mono and multichannel devices, values including ±0, subnormals and 3e30): every strip's
  gather and the master's equal a rule-for-rule reference of the serial engine's scatter,
  **to the bit** (`to_bits`). Also the plan's DAG (forward edges, predecessor counts, every
  input a dependency), and that compiling in place equals compiling afresh and the key
  follows routes, not levels. Seeded and reproducible (a failure prints its seed) rather
  than `proptest`, to add no dependency to the lock file this wave.
- `tests/limits.rs` — 256 tracks on 256 strips all sound; a 257th track and a 257th strip
  are refused and counted (`refused_tracks`, `refused_strips`).
- `tests/ftz.rs` — inside `process` FTZ and DAZ are set (a probe insert reads MXCSR/FPCR)
  and the caller's mode is restored, whatever it was; the output does not depend on the
  caller's mode; and a corpus with every native effect kind on strips and the master,
  eight seconds into its tails, renders **bit-identical with the guard on and off**.

## Deliberate differences (no test sees them; none changes a sample of a sane project)

- **Sign of zero, three corners.** Where the serial engine *skipped* a strip for part of
  a block (no input, no inserts, a quiet EQ, lines empty) and the graph runs it for the
  whole pass, a phase-inverted strip's silent stretch can leave as −0.0 instead of not at
  all: only a capture take or a stem of that strip, or a key read from it, can see it.
  The click's "still active?" is decided per click stretch instead of per global segment,
  which can turn a −0.0 of an inverted, silent master into +0.0. Neither can reach a sum
  (`x + ±0.0 == x`), and the serial engine itself was not block-size-independent there.
- **The click after a trigger** looks for the next beat in the same stretch: the serial
  engine missed a beat that fell inside the segment that triggered the previous one (beats
  closer than a block — 300 BPM in 16ths at 8 192 frames).
- **Notes in flight across a re-plan** keep their listener's cursor but take the listener's
  new delay; a listener whose `midi_from` changed loses the notes of its old track still
  on their way (the shared queue delivered them by effect id).
- **Hosted units' song position** is the pass's (a strip's inserts, the master's) or its own
  segment's (an instrument), not every global segment's — what ADR 0031 §1.5 asks.
- **`EngineStats::active_voices`** counts the slots a track holds; a freed slot's frozen
  voices no longer count.

## Left for later

- The slab and `EngineEvent::GraphNeed` (§1.4): every node buffer is preallocated for the
  largest graph at `PASS_FRAMES`, so nothing asks for memory yet. The delay-line pool
  (`pdc.rs`, ≈ 12 MiB of zeroed lines) is not pre-faulted yet: R11 for it, and locking,
  come with the slab (`cantus_rt::memory::{prefault, lock}`).
- Critical-path ranks, clustering and the serial fallback (§2.3, §2.5): Phase 1.
- The stems tap's `direct` sum is still added by the track nodes in track order — serial
  only; Phase 1 moves it into the master's gather.

## Notes for the integrator

- **`cantus-rt` (r21/rt) — a body swap.** `graph/rt_shim.rs` copies `cantus-rt`'s API:
  `AudioHost` (same variants, `from_driver_name`), `promote_callback(host, period)`, and the
  denormal guard (a byte-for-byte copy of `cantus-plugins/src/ftz.rs`). At the merge:
  add `cantus-rt = { workspace = true, features = ["rtkit"] }` to the engine, make the
  shim `pub use cantus_rt::{AudioHost, DenormalGuard};` and
  `pub fn promote_callback(h, p) -> bool { !matches!(cantus_rt::promote_callback(h, p),
  cantus_rt::Promotion::LeftAsIs(_)) }` (or store the `Promotion` in a new
  `EngineStats::promotion` as rt's request suggests), drop the shim's `unsafe`, and put
  `lib.rs` back to `#![forbid(unsafe_code)]` (it says `deny` only for the shim). Do **not**
  call `cantus_rt::init_process()`'s FMA3 switch (rt's note: it moves 92 % of samples).
- **`EngineHandle::start`** (asio's file): one line added after `attach_analyser` —
  `processor.promote_on_first_block(AudioHost::from_driver_name(host.id().name()))`. If
  asio moved the processor's construction, put that line wherever the processor is built
  for a stream.
- **`EngineStats::wakes`** counts the permitted system calls (the promotion now, the pool's
  idle → busy wakes in Phase 1): the sched builder's name, so wave 2 needs no rename.
- **`EngineMessage::SetSnapshot(Arc<Snapshot>)` is unchanged** (every test sends it); the
  plan travels in the new `SetSnapshotPlanned(Arc<Snapshot>, Box<GraphPlan>)`. Anything
  that matches `EngineEvent` exhaustively needs an arm for `GarbagePlan` (only
  `backend.rs` does today).
- **at-rest** (r21/at-rest): nothing in this branch depends on it, and its requests need no
  change here to merge. Suspension (calling `at_rest`/`skip`) is Phase 2's. Its extended
  contract tests should be run on the merged tree: this branch's split-invariance rests
  on them (ADR 0031 risk 1); every contract test of the base passes here.
- **sched**: `GraphPlan` gives what `sched::Graph` needs — `nodes()`, `successors(n)`,
  `predecessors(n)`, `master()`, and the canonical `inputs(n)`; node indices are the serial
  order (tracks, strips in order, master), every edge runs forward.
- **Memory.** `Meters` grew to 256 tracks and strips (≈ 3.7 KiB): the event ring of 4 096
  slots is now ≈ 15 MiB in the application (it was ≈ 5). If that matters, a smaller
  `EVENT_QUEUE` or meters by reference are wave-2 choices. The node buffers are
  `PASS_FRAMES` long, so the processor itself is about as large as it was at 64.
- **Outside `processor*`/`graph/`**: `messages.rs`, `backend.rs` (plan sending + the one
  promotion line), `lib.rs`, `render.rs`, `stems.rs` (send the plan), `processor_rec.rs`
  (the click at a sample), and `cantus-project`'s two mirrored limits
  (`dawproject/read.rs`, `midi/import.rs`: 64 → 256). `cantus-bench` follows
  `MAX_TRACKS` by itself. `crates/cantus-bench/examples/graph_ab.rs` is a local measuring
  aid, not committed.
