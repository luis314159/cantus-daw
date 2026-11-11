# Round 39 — `memory`: the engine's memory grows with the song (ADR 0107)

Branch `r39/memory`, worktree `F:\projects\daw-r39\memory`, base master `80d5f109` (beta.13).

## What changed

The processor before any song held 95.8 MB, nearly all of it capacity an empty song never
touches. Now it holds **8.0 MB**; a start on the demo holds **59.7 MB** where it held 145.5.

| Step of a start (`memory_e2e`, live heap) | Before | After |
|---|---:|---:|
| The processor | 95.8 MB | **8.0 MB** |
| Assembled on an empty song | +4.5 | +3.2 |
| Three frames | +6.6 | +8.2 |
| The demo opened, three frames | +38.5 | +40.2 |
| In all | 145.5 MB | **59.7 MB** |

Step by step, the processor alone (each measured with `--nocapture`):

| Change | Processor |
|---|---:|
| Round 38 | 95.8 MB |
| Pulsus kits off the slots, the meters out of the event queue, the compensation pool from 32 lines (A–C) | 41.0 MB |
| Note-effect chains off the slots too (found in the map once the kits were gone: 256 × 50.8 KB) | 28.3 MB |
| The slots grow with the song (16 track and 32 strip slots provisioned at first), a place's edge buffers come with its first line, scope rings made when first watched, pool scratch made for the slots a pool runs (the coordinator's "no leftovers") | **8.0 MB** |

1. **Slot parts** (`crate::parts`, `processor_parts.rs`, `pulsus_kit::SlotKit`): a track slot
   holds a Pulsus kit only while its track hosts Pulsus (−31 MB) and a note-effect chain only
   while its track has note effects (−13 MB). The backend builds and sends them before the
   snapshot that uses them (`EngineMessage::AddPart`); the processor keeps every one and moves
   them between slots, emptied, never allocating or freeing. A sender that sent none is asked
   once (`EngineEvent::PartsNeed`); offline paths and hand-driven tests build them in place
   (`Processor::hold_parts_for`).
2. **The block's report in a mailbox** (`crate::report`): meters, stats and modulator values
   go round in eight boxes over two `rtrb` rings of pointers; a late reader gets the missed
   blocks folded into the next report (highest peaks kept, the true peak reset only once a
   report got through). `EngineEvent` went from 3 880 B to ≤ 64 B (−15.6 MB). The
   `Meters`, `Stats` and `Modulation` events are gone; `Processor::meters()` and
   `take_reports()` replace them.
3. **The compensation pool grows from 32 lines** (`pdc::START_LINES`, 2 MB; was 192, 12.4 MB).
   The backend holds the engine to `pdc::lines_needed(snapshot)` before each snapshot (zero
   lines for a song nothing in which can have latency); long delays grow as before. A new edge
   never takes a kept edge's line (assignment deferred to `Compensation::finish`).

4. **The slots grow with the song** (`Processor::provision`, `parts::SlotCells`): all 256
   track and strip slots exist, only the first `ready` have pass buffers (16 and 32 at
   first). The backend sends what a snapshot needs before it (`EngineMessage::Cells`); a
   snapshot that needs more waits — the one before plays on, the queue keeps draining — asks
   once (`CellsNeed`) and installs when they come. A place's compensation edge buffers come
   with its first line (−1.5 MB).
5. **Scope rings when first watched** (−4.4 MB). The 8 × 552 KB I first called "the pool's
   scratch" (after round 38's scorecard) are the Speculum scope rings: an allocation count by
   thread and size proved it. The backend makes a ring the first time a scope is watched in
   its place (`EngineMessage::ScopeRing`).
6. **Pool scratch for the slots a pool runs**: a worker's scratch and profiler are made by
   `set_workers` for that pool's slots only, off the audio thread and touched there.

## How it is tested

- `tests/slot_parts.rs` (new): an engine before any song holds no part; a kit sent plays as a
  kit built in place, to the bit; a drum track without its kit is silent, asks once (and not
  again for the same shortfall) and plays from the block its kit arrives; kits move between
  slots as tracks change and are never asked for twice; the backend sends each kit once; a
  note-effect chain sent plays as one built in place and goes back spare when its track loses
  its note effects.
- `tests/rt_memory.rs`: `kits_arriving_and_moving_never_allocate` (two workers),
  `the_reports_mailbox_never_allocates` (drained every block, then a late reader folded),
  `compensation_lines_growing_from_few_never_allocate` (two workers; 22 tracks beside a latent
  strip grow the pool past its 32 lines, the insert taken out again) — zero heap events on the
  callback and the workers.
- `report.rs` unit tests (in-time reports as they were; a late reader's blocks folded), `pdc.rs`
  unit tests (a new pool holds only its start-up lines; a new edge never takes a kept edge's
  line; an empty pool asks and a short delay gets a start-up line; what a snapshot can need),
  `messages::size_tests::an_event_is_small`.
- The twins and null tests unchanged and green: the golden hashes, offline = real time,
  parallel = serial, block-size independence (`null_tests`, `inst_pulsus`, `padout`,
  `session`, `notefx`, `pdc`, `heard`, …), `resting_changes_no_sample_of_the_demo` (cantus-ui).
  Tests that drive a processor by hand now call `hold_parts_for` before installing; tests that
  read `EngineEvent::Meters` read `Processor::meters()`.
- `memory_e2e`'s bounds lowered: processor ≤ 9 MB, empty song ≤ 12.5, demo ≤ 44.5, all ≤ 66.
- Slots: `slot_parts.rs` — forty tracks and forty-six strips, their slots held in place or
  sent ahead, play the same bits; sent without them the snapshot waits, asks once and plays
  when they come; the backend provisions them before the snapshot. `rt_memory.rs`'s
  `slots_provisioned_while_playing_never_allocate` (two workers) and
  `a_scope_ring_arriving_never_allocates`.
- `cargo test -p cantus-engine` (every target), `CANTUS_WORKERS=3 cargo test -p cantus-engine
  --features fuzz`, `cargo test -p cantus-ui`, clippy `-D warnings` on the crates touched.

## Outside my files (minimal)

- `crates/cantus-bench/src/rig.rs`: the bench's processor holds its song's parts before the
  snapshot (otherwise its drum tracks would be silent and the bench would measure less work).
- `crates/cantus-bench/src/suites.rs`: the *load* row builds the song's parts with its units,
  as the application now does.
- `crates/cantus-ui/tests/demos.rs`: one hand-driven processor holds its parts.
- `crates/cantus-ui/tests/memory_e2e.rs`: the chain in the parts map; the new bounds.

## Verified only here

Everything is in-process: no device or driver is involved (the mailbox and the parts travel
the same rings on every host). The scorecard's start-up and private-memory rows are the
integrator's to measure on a quiet machine (`tools/scorecard/memmap.ps1`); not run here.

## What is left (nothing of this brief)

The two items left after the first delivery — the slots' pass buffers and the 4.4 MB then
called the pool's scratch — are done (items 4–6). What the processor's 8 MB holds now is
fixed working state, none of it scaled to the 256-slot ceiling (ADR 0107 §Consequences).
Slots, parts and lines are kept for the session once made (its high-water mark); giving them
back when a smaller song opens would need the backend to count what comes back, and is not
needed for the scorecard's rows.
