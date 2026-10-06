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
name); counted in `EngineStats::wakes`.

## Measurements

MEASUREMENTS

## Tests

TESTS

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
  largest graph at `PASS_FRAMES`, so nothing asks for memory yet; pre-faulting (R11) comes
  with the slab.
- Critical-path ranks, clustering and the serial fallback (§2.3, §2.5): Phase 1.
- The stems tap's `direct` sum is still added by the track nodes in track order — serial
  only; Phase 1 moves it into the master's gather.

## Notes for the integrator

NOTES
