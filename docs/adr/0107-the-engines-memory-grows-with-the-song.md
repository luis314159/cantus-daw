# ADR 0107 — The engine's memory grows with the song

**Status:** accepted (round 39, the `memory` builder) · **Date:** 2026-11-09 · **Schema:**
none · **Amends:** ADR 0005 §1 (what is preallocated at construction), ADR 0018/0022 (the
delay-compensation pool's start-up lines), ADR 0031 §1.4 and §5.4 (memory comes like delay
lines), ADR 0069 (where a track's note-effect chain lives), ADR 0100 (where a Pulsus kit
lives) · **Relates to:** ADR 0106, `docs/round38/scorecard.md` §4

## Context

The scorecard's one lost row: Cantus held 237 MB on an empty song against REAPER's 62 and FL
Studio's 133. `crates/cantus-ui/tests/memory_e2e.rs` mapped the heap of a start: the processor
alone, before any song, held **95.8 MB**, about 60 MB of it capacity an empty song never
touches — the engine preallocated for the largest session it can hold (256 tracks, 256
strips) and every session paid for it:

- every one of the 256 track slots held a Pulsus drum machine (119.6 KB: its voice pool, its
  pad and layer tables) whether its track was a drum track or not — **31 MB**;
- every slot held a note-effect chain (50.8 KB) whether its track had note effects or not —
  **13 MB** (found by this round's map once the kits were gone);
- the engine's event queue held 4 096 events of 3 880 B, because an event is as large as its
  largest kind and the block's meters (256 track and 256 strip peaks, 256 gain reductions,
  two telemetry blocks) travelled in it — **15.9 MB**;
- the delay-compensation pool held 192 lines of 2 × 32 KB from the start — **12.4 MB** —
  though a project with no latency uses none;
- the pass buffers of all 256 track and 256 strip slots (outputs, sends, taps, keys, note
  outboxes) and 192 compensation edges — **17.4 MB** — and eight Speculum scope rings —
  **4.4 MB** — made whether a slot or a scope was ever used.

Each is an audio-thread contract (ADR 0005): nothing may be allocated there, so what a
snapshot may need was made in advance. The rule stays; what changes is *who* makes it and
*when*.

## Decision

1. **Slot parts are built off the audio thread and sent in** (`crate::parts`,
   `processor_parts.rs`). A track slot holds a Pulsus kit (`pulsus_kit::SlotKit`) only while
   its track hosts Pulsus, and a note-effect chain only while its track has note effects.
   Before a snapshot that uses more of either than the engine holds, the backend builds them
   (`parts::parts_for`) and sends them (`EngineMessage::AddPart`), as it sends native
   instruments' units (ADR 0023). The processor keeps every part it is given — in a slot or
   in its spares, whose vectors have room for one of each per slot from construction — and
   at every install moves them: a slot whose track no longer uses a part gives it back
   (silenced and emptied: a kit `reset`, a chain configured to no stage and `reset`, so a
   reused part is a fresh one to the bit), and a track that uses one it lacks takes a spare,
   configured from the snapshot exactly as the install would have. Nothing is allocated or
   freed on the audio thread; parts are never freed at all (the engine's high-water mark of
   drum tracks and note-effect tracks, a few hundred KB).
   - **A sender that did not send them** (a test, a hand-built processor) is asked once per
     shortfall (`EngineEvent::PartsNeed`, counting what is already on its way); until the
     parts arrive a drum track is silent and a chain is bypassed (`EngineStats::kits_waiting`,
     `note_fx_waiting`), and they are placed in the block they arrive in. The backend answers
     it. Every offline path (render, stems, freeze, range renders, the modulators' bake) and
     every test that installs snapshots by hand builds them in place first
     (`Processor::hold_parts_for`), off any audio thread.
2. **The block's report travels in a mailbox of its own** (`crate::report`): meters, stats and
   the modulators' values leave the event queue. Eight boxes go round between the threads over
   two `rtrb` rings of pointers — the audio thread fills an empty box and hands it over full,
   the interface reads it and hands it back. A block whose report finds no empty box (a late
   reader) is **folded** into the one held: the latest readings, with those the reader takes
   the highest of (an analysed compressor's peaks and reduction, the inputs' peaks) kept at
   their highest; the master's true peak starts again only once a report got through
   (`MasterLoud::sent`, ADR 0052). A reader that drains every report and folds them — the
   backend does — sees what it saw when each block had its own event. An event is now at most
   64 B (`messages::size_tests::an_event_is_small`); the queue of 4 096 holds 256 KB.
   `Processor::meters()` gives a test the last block's meters; `Processor::take_reports()`
   gives the backend the reading end.
3. **The delay-compensation pool grows from 32 lines** (`pdc::START_LINES`, 2 MB, enough for a
   new project's sixteen strips with latency on one and a few tracks). Its 192 places are
   otherwise empty. Before a snapshot, the backend holds the engine to as many lines as the
   snapshot can need (`pdc::lines_needed`: one per edge that can be delayed — every track's
   and its own lateness, every strip's output, send and insert dry path, every master insert,
   the click and the recorder — and **none** when nothing in the song can have latency),
   sending start-up lines (`pdc::short_lines`) through the existing `EngineMessage::PdcLines`,
   which swaps them into empty places. The growth path for long delays (`PdcNeed`, ADR 0022)
   is unchanged and also serves a sender that sent nothing ahead.
   - With a pool sized to what the project needs there is no spare to make taking a line from
     an edge harmless, so **a new edge never takes a kept edge's line**: `Compensation::want`
     keeps the lines of edges that keep theirs and queues the rest, and `finish` gives the
     queued edges free lines only once every kept edge has kept its own. With enough lines
     this assigns exactly as before (a new line's ring starts empty either way); with too few,
     the newcomer waits for a line instead of a kept edge losing what its line holds.
   - A range render's checkpoint (ADR 0083) still leaves fresh lines out of its image, and a
     copy made from the image makes them again where the planner says a line is
     (`Compensation::len_of`), so a copy holds what the original did.
   - A place's **edge buffers** (a delayed send's output, ADR 0022) come with its first line
     (`DelayLine::out`, handed over by `Processor::edges_for_lines` when lines are supplied):
     a place without a line holds none (they were 192 pairs, 1.5 MB).
4. **The track and strip slots grow with the song** (ADR 0031 §1.4's "the ceiling costs
   nothing until it is used"). All 256 of each still exist — the plan, the atoms and every
   index stay as they were — but only the first `ready` are *provisioned*: the rest are bare,
   their pass buffers (a track's output, send and tap pairs and note outbox; a strip's
   post-chain, pre-fader, output and key pairs) empty. A processor starts with
   `parts::START_TRACKS` (16) and `START_STRIPS` (32). The install gives a track or strip only
   a provisioned slot, and since a slot whose track went is reused before a fresh one is,
   a snapshot of *n* tracks and *m* strips fits in *n* and *m* slots whatever came before
   (`parts::cells_needed`).
   - The backend sends the slots a snapshot needs beyond those (`parts::SlotCells`, built and
     touched off the audio thread, `EngineMessage::Cells`) before the snapshot; the processor
     swaps them into the next bare slots between passes and hands the bare ones back
     (`EngineEvent::GarbageCells`). Offline paths and hand-driven tests provision them in place
     (`hold_parts_for`).
   - A snapshot that needs more than are provisioned (a sender that sent none) **waits without
     stopping the queue**: the snapshot before it plays on, every other message is handled, a
     later snapshot takes its place, the slots are asked for once (`EngineEvent::CellsNeed`,
     `EngineStats::cells_waiting`), and it installs — on the automation's grid while playing,
     as any snapshot does — in the block its cells arrive. A range render does not fork while
     one waits (a passing refusal).
5. **Scope rings are made when a scope is first watched.** Eight rings of 1 024 chunks for
   the Speculum analyser (ADR 0056) were made with every engine handle: 4.4 MB whether a scope
   was ever opened or not. (Round 38's scorecard and this ADR's first version called these
   "the pool's scratch"; the per-allocation count proved them the scope rings.) The backend
   now makes a ring the first time a Speculum is watched in its place and sends its writing
   end (`EngineMessage::ScopeRing`) before the watch; each place is sent once and the ring
   stays for the session.
6. **A pool slot's working memory is made for the slots a pool runs.** The callback's slot is
   made with the processor; a worker's (its scratch buffers, its events, its profiler) when
   `Processor::set_workers` starts a pool with that slot — off the audio thread, touched there —
   instead of all fifteen at construction.

## Consequences

Measured by `memory_e2e` (this binary's own allocator; `--nocapture` prints the map):

| Step of a start | Round 38 | After 1–3 | After 4–6 (and the edge buffers) |
|---|---:|---:|---:|
| The processor, before any song | 95.8 MB | 28.3 MB | **8.0 MB** |
| The app on an empty song, its first frames | +11.1 | +11.1 | +11.4 |
| The demo opened, three frames | +38.5 | +40.3 | +40.2 (its kits, slots and lines counted here) |
| **A start on the demo, in all** | **145.5 MB** | 79.8 MB | **59.7 MB** |

The bounds are lowered to about a tenth above (processor ≤ 9, empty song ≤ 12.5, demo ≤
44.5, all ≤ 66 MB). The start-up rows of the scorecard are the integrator's to measure.

What the processor's 8 MB still holds is its fixed working state: the 32 start-up lines
(2 MB), the 16 track and 32 strip slots provisioned, the callback's scratch, the master
chain's buffers, the loudness meter's histograms, the plan and the profiler — none of it
scaled to the 256-slot ceiling any more.

**Bit-identity.** No sample changes: the golden hashes, `resting_changes_no_sample_of_the_demo`,
offline = real time, `CANTUS_WORKERS` parallel = serial and block-size independence all hold
with the parts and slots sent or built in place (`tests/slot_parts.rs` holds a kit, a chain
and forty tracks' and forty-six strips' slots sent ahead and built in place, to
the bit). `DSP_VERSION` stays 6.

**Real time.** `tests/rt_memory.rs` proves the new paths allocation-free on the callback and
on the pool's workers: kits asked for, arriving while the song plays and moving between
slots (`kits_arriving_and_moving_never_allocate`), the mailbox posting and folding
(`the_reports_mailbox_never_allocates`), compensation lines (and their places' edge buffers)
growing past the start-up pool while a latent insert comes and goes
(`compensation_lines_growing_from_few_never_allocate`), a wide song's slots asked for,
provisioned and installed while the demo plays (`slots_provisioned_while_playing_never_allocate`)
and a scope ring arriving (`a_scope_ring_arriving_never_allocates`). The engine fuzz
(`CANTUS_WORKERS=3 … --features fuzz`) passes.

**Costs.** Loading a song builds its parts and slots with its units (a kit ≈ 120 KB, a chain
≈ 51 KB, a track slot ≈ 30 KB, a strip slot 32 KB), on the interface thread — counted in the
bench's *load* row (`cantus-bench`, `suites.rs`). A reader that never takes the mailbox (an
offline render) costs the audio thread one fold per block, the same copy the event used to
be. Slots, parts and lines are kept for the session once made (the high-water mark of the
songs it opened).

## Addendum — the stack the engine is built on (round 39, `r39/stack`)

**What happened.** The integrated tree's release build overflowed Windows' 1 MB main thread
at start (`thread 'main' has overflowed its stack`, right after the EcoQoS line), with or
without a song; beta.13 started. The suite was green: tests run on 2 MB threads. The
processor had grown to 63.6 KB *inline* — the delay-compensation planner's arrays (13.9 KB,
this ADR), the recorder's live state (10.5 KB), the external MIDI state (8.5 KB), the slot
id, arrival and post tables (12 KB), the meters (3.9 KB) and the master meter (4.2 KB) — and
it is built on the thread that opens the device and moved by value: `Processor::with_cells`
(137 KB frame), `EngineHandle::start` (200 KB), then the output driver through cpal's
generic stream builders (64 + 128 + 192 + 128 KB), all under the boot's `Shell::ui`
(337 KB, the assembled application inlined in it). Measured from the release binary's
`__chkstk` sizes (`dumpbin /disasm`).

**Decision.** Large values live on the heap, built off the audio thread: those parts of the
processor are boxed (`Processor` is 10.3 KB inline), the output driver holds the processor
boxed (200 B), and the boot builds the application boxed in a frame of its own
(`Boot::assemble`, never inlined), so no stage carries it. The audio thread copies into the
boxes and never makes one (`tests/rt_memory.rs`). As an additional guard only, `cantus.exe`
is linked with an 8 MB main-thread stack on Windows (`crates/cantus-app/build.rs`), as Linux
and macOS give theirs; the frames are held without it. After the fix the release binary's frames on the
engine's start are `Shell::ui` 13 KB, `EngineHandle::start` 20 KB, `with_cells` 35 KB and
cpal's builders under 4 KB each; the assemble stage (`Boot::assemble` 132 KB,
`CantusApp::assemble` 128 KB, `from_parts_db` 220 KB) runs on its own, and the application
starts and stays up with a 1 MB main thread as with 8 MB.

**Tests.** `processor.rs` (`stack_size`) bounds `Processor` at 16 KB and `OutputDriver` at
1 KB inline — the device path no test can run; `cantus-engine/tests/stack_budget.rs` builds
the engine (`EngineHandle::manual`, `dummy`) on a 256 KB thread; `cantus-ui/tests/
stack_budget.rs` assembles the application with the start-up song as the boot does on a
768 KB thread (Windows' main thread less the window's own frames).
