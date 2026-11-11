# Round 39 — `abort-blame`: a shared helper's crash blamed on no one, the cause that stayed

Branch `r39/abort-blame`, from master `5466982a` (round 39 squashed). It blocked beta.14:
`cantus-sandbox/tests/session_faults9.rs`,
`a_hundred_aborts_in_a_shared_helper_are_each_the_aborting_plugins_alone`, failed on
GitHub's Windows runners in both runs at the tag — "the gain not blamed in 2 of 100" (CI)
and "6 of 100" (release) — and once on this machine too, in round 39's integration
(`integfix-ws9.log`, the workspace suite beside three builds: 1 of 100).

## The cause

A CLAP gain and a VST3 gain share one helper; the gain aborts in its `process`. On Windows
that is a fail-fast: no crash handler runs, so the end is laid on the one candidate among
the instance the helper's main thread names in the lobby (`MAIN_AT`) and the jobs a worker
took and never finished (ADR 0063 §3, `culprits`). Two candidates are nobody's.

The main thread named an instance for far more than its plug-in's code:

- **A request, whole.** The test sets the neighbour's value (`V_GAIN`) just before
  `settle()`: a request (`Flush` on instance 2) the helper's main thread answers. The
  settle is fast — 64 blocks in about 14 ms here — and the fault follows at once. The mark
  covered the plug-in's call and also the helper's own work after it: the mirroring, the
  answer serialised and written to the pipe. A main thread lagging 14 ms behind (put off
  the processor, or waiting on the pipe) was still answering the neighbour when the gain's
  job aborted.
- **A tick, whole.** Every 15 ms the main thread mirrors each instance (`Server::tick`: its
  flush, its requests, its values). The mark covered the whole mirroring, the bookkeeping
  between the plug-in's calls included; a main thread put off the processor there, in the
  neighbour's turn, left the neighbour named.

Either way: `MAIN_AT = 2` and the gain's job running — two candidates, nobody blamed.
Never the neighbour, always nobody: what CI saw. A debug print of every end showed both:
`end=Observed crash_at=0 main_at=2 running=[gain] laid={}`, serving the neighbour's
`Flush` (with the lag knob below), and serving nothing — a tick — in the loaded runs.

**What it was not** (each tried and measured before the cause showed):

- *The process held for seconds by the system's error reporting.* A fail-fast freezes the
  whole helper here within a millisecond (a timing program: no tick of another thread after
  the abort) and ends it about 150 ms later. A Cantus-side hook that saw the end up to 12 s
  late — the helper silent, the gain's job unfinished, the neighbour's job never taken —
  still blamed the gain in every case: below 5 s the end is observed with one job running;
  above it the block's 5 s wait is taken for a pause (`rebased_after_a_pause`), every clock
  starts again, and the gain's slot passes its hang limit before the silence limit.
- *The other threads running on while the aborting one is held.* The gain sleeping up to 8 s
  before its abort, the rest of the helper live: the gain each time.
- *Load alone, on master.* 300 rounds at 14 spinning threads, 150 on four cores with eight, 100 on two
  cores with six, 400 of the whole binary, 300 more with threads that also churn memory:
  0 failures, `main_at` 0 every time. The lag needed (14 ms, the settle's length) is rarer
  here than on a 4-vCPU runner.
- The pipe breaking mid-frame (a short frame is the end, not a broken protocol — review
  #43), a job taken before the exit is seen (the take is in shared memory before the abort),
  the silence and hang judgements racing the exit (the first decides; each names the gain or
  is never reached here).

## The fix (`crates/cantus-sandbox/src/serve.rs`)

The main thread names a plug-in only while it runs that plug-in's code:

- `marked(instance, f)` runs one call into the plug-in naming it. The mirroring
  (`Instance::news`, `Instance::after`) and the tick mark each call into the plug-in's
  main half or window (`idle`, `param_value`, `params`, `latency`, `flush`,
  `resize_editor`, the window's fit and close) and none of the bookkeeping between them.
- `Server::handle` keeps the request's mark over the request's own calls (the match: a
  load, a save, a flush…) and drops it before the mirroring after it (which marks its own
  calls) and the answer.
- `Server::send` writes every frame under `blame::calling(0)` — the helper's own code, in
  whatever call it is made (the news a save says first, a load's frames) — and restores the
  mark after.

A request's calls, a unload, a window's messages (their own hook) are marked as before, so
a hang or a crash in a plug-in's call is laid as before. A crash in the helper's own writing is no plug-in's (a
handler writes *outside*). A main thread truly inside a neighbour's code at the instant
another plug-in fail-fasts is still two candidates and stays nobody's: nothing in the
process says which thread called the fail-fast (`bridge_blame.rs`'s two-jobs test and
`culprits`' unit test hold that rule). Recorded in ADR 0063 §9.

Nothing behind `#[cfg(windows)]` was touched: `blame::calling` is the same on every system,
so the Linux build has no new dead code.

## The test knob and the tests

`CANTUS_TEST_SANDBOX_LAG=<instance>:<ms>` (read once by each helper, capped at 1 s like the
timers' knob): the helper's own work about that instance — each answer to a request about
it, each tick's mirroring of it — takes that much longer, after the plug-in's calls: a
loaded runner's lagging main thread, on any machine.

- `session_faults9.rs`,
  `an_abort_while_the_neighbours_answer_is_written_is_still_the_gains`: the hundred-abort
  scenario seven times (each pre-block offset) with a 60 ms lag on the neighbour; the gain
  alone each round. **Before the fix: nobody in 7 of 7; after: 0.**
- `serve.rs`'s unit test `the_lag_knob_names_an_instance_and_is_capped`.
- The hundred-abort test is unchanged.

## Rates (the hundred-abort test, `CANTUS_ABORTS`, this machine: i7-12700H, 16 threads)

"Before" is master `5466982a`; "after" the final fix. Neighbour blamed: 0 in every run.

| Condition | Before | After |
|---|---|---|
| lag 60 ms on the neighbour | 100 of 100 nobody | 0 of 500 |
| lag 15 ms | 28 of 100 nobody | 0 of 500 |
| lag 5 ms (shorter than the settle) | 0 of 100 | — |
| 14 spinning threads beside the test, no lag | 0 of 1 000 | 0 of 1 000 |
| 14 spinning threads, lag 15 ms | — | 0 of 500 |
| the workspace suite beside three builds (integration, 2026-11-09) | 1 of 100 | — |
| GitHub's Windows runners (beta.14) | 2 and 6 of 100 | the next CI and release runs |

Load alone never showed it here on the master code (0 of 1 000 at 14 spinning threads, and
0 in about 2 300 more rounds earlier: four cores with eight threads, two with six, threads
churning memory, the whole binary) — the 14 ms lag it needs is rare on this machine. A first
fix that unmarked only the answer (its frames written naming no plug-in) still failed 4 of
1 500 at 14 threads, each end `main_at=2` serving no request — the neighbour's tick. Marking
each call into the plug-in instead of the whole tick closed that window too (0 of 1 500,
lag or none).

## Verified

- `cargo test -p cantus-sandbox -p cantus-host --no-fail-fast`: 282 passed, 0 failed,
  2 ignored (38 binaries).
- `RUSTFLAGS="-D warnings" cargo clippy -p cantus-sandbox -p cantus-host --all-targets`:
  clean.
- `cargo fmt -p cantus-sandbox -- --check` and `-p cantus-host -- --check`: clean.
- Linux: not built here. Nothing behind `#[cfg(windows)]` changed and the new code
  (`marked`, `lag`, `Server::send`'s mark) is the same on every system, so no new
  Linux-only dead code is expected; the WSL check is the integrator's.

## Left

- Only GitHub's runners can confirm the CI rate: the next Windows CI and release runs.
- The residual, by design: a fail-fast while the main thread is genuinely inside a
  neighbour's code is nobody's. Telling the faulting thread apart would take the system's
  error reporting (a WER runtime-exception module) or a debugger attached to the helper —
  noted, not done.
