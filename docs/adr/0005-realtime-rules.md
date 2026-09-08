# ADR 0005 — Real-time rules for the audio thread

**Status:** Accepted · **Date:** 2026-09-07

1. **No allocation, no deallocation.** Voices, buffers and event queues are pre-allocated at
   engine construction with fixed capacities. `Arc<Snapshot>`s replaced on the audio thread are
   sent back over a *garbage* ring buffer so the UI thread drops them.
2. **No locks.** UI ⇄ engine communication is `rtrb` SPSC ring buffers only.
3. **No syscalls, no logging, no panics** in `process()`. Overflow of an event queue is
   counted (`EngineStats::dropped_events`) and reported, never panicked on.
4. **Sample accuracy.** Note on/off and transport changes are applied at the exact frame
   inside the block (the block is split at event boundaries).
5. **Deterministic.** Given the same snapshot, sample rate and start position, `process()` must
   produce bit-identical output. This is what the null-test harness verifies, and what makes
   the offline WAV render equal to what the user heard.
6. **Instrumented.** Callback duration and xruns are measured on every block and exposed as
   `EngineStats` (idea.md §12, "audio profiling tooling").
