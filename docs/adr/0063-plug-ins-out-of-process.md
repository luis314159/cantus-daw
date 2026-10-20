# ADR 0063 — Third-party plug-ins out of process

**Status:** accepted (round 29) · **Date:** 2026-10-16 · **Amends:** ADR 0005 (rule 3, for
this one case), ADR 0019 (hosting: "not built: crash isolation"), ADR 0031 (§2.7's line
budget, §6 and R15) · **Extends:** ADR 0028 (hosted instruments), ADR 0048 §9 (signed
files)

## Context

The owner's ask (2026-10-16): *a plug-in that crashes must not take the session down; the
user is told, can reload, and the project loses nothing; latency and CPU measured against
in-process hosting, with an option per plug-in.* Until now a CLAP or VST3 plug-in that
crashed in `process`, in its state, its activation or its editor took Cantus with it
(ADR 0019 isolated only the scan; BETA.md listed it as a known limitation).

ADR 0031 §6 designed the sandbox as an *external task* of the real-time pool (no host
thread ever waits on another process; the scheduler polls completions), R15 added a
machine-wide spinner budget, PipeWire-style direct wakes and an *asynchronous* option (the
previous block's output, one block of latency). Its phase-6 target: a crashing plug-in in
*Together* leaves audio running, and a round trip ≤ 5 % of a 64-sample budget per group —
or the default stays *Within Cantus*.

The seam was already there: `cantus-host`'s `Backend` splits a plug-in into a `MainThread`
and an `AudioThread`; everything above — `HostedUnit`, `HostedInstrument`, `PluginHost`,
the engine, the interface — knows no format.

## Decisions

### 1. Lock-step, at the backend seam — the engine stays unaware

An isolated plug-in is loaded by `BridgedBackend`, one more `Backend` whose halves are
proxies (`cantus_host::bridge::{BridgeMain, BridgeAudio}`); a helper process
(`cantus-sandbox`, a new GPL crate) loads the real halves with the very same CLAP and VST3
backends. No engine trait changed; no file of the engine changed.

Three designs were weighed by measurement (§5):

- **ADR 0031 §6's external task** needs the scheduler to treat a node as remote — the pool's
  files (`graph/`, `sched/`), owned by nobody this round and hot for every later one — and
  gains only while other nodes are ready to run meanwhile. Not taken: the cost it hides is
  what §5 shows is already small.
- **The asynchronous option** (R15 (3)) takes the round trip off the critical path at the
  price of a fixed latency for every isolated plug-in. The engine calls a unit with passes
  of varying length (split at events, up to 1 024 frames), so the latency would have to be
  the largest pass — 21 ms at 48 kHz on every isolated insert — and the output would no
  longer be the in-process output at the same place. Not taken.
- **Lock-step** (taken): the audio half copies the block into the instance's shared slot,
  publishes the job, spins for a bounded time, then sleeps on its own bell until a
  **deadline**; the plug-in's output comes back in the same call. No added latency; the
  bits are the in-process bits (§4).

**This amends ADR 0005 rule 3 for this one case.** The audio thread may wait for an
isolated plug-in: the wait *is* the plug-in's own processing time, moved to another process
— in process the audio thread would be inside the plug-in for the same span. It is bounded:
a spin of at most 200 µs (`bridge::audio::SPIN`), then a kernel wait on the caller's bell
until the deadline — twice the block's duration, at least 5 ms. A plug-in that takes longer
was going to make the device drop out in process too; isolated, the block goes on without
it (§3). The only other system call is the non-blocking ring (`SetEvent`, a futex wake)
that wakes a parked helper thread. Nothing allocates (`tests/rt_alloc.rs`). An **offline**
render (export, stems, freeze, bounce) has no deadline: it waits for every block while the
helper lives.

### 2. Shared memory and the lobby (`cantus-rt`, MPL)

`cantus_rt::shm` maps named shared memory (Windows `CreateFileMappingW`/`MapViewOfFile` in
`Local\`, Linux `shm_open`/`mmap`, its name unlinked by its creator as soon as the helper
has mapped it — after the handshake for the lobby, after the load for a slot — so a killed
Cantus leaves nothing in `/dev/shm`, review #43); `cantus_rt::ipc` is the
protocol, generic and audio-agnostic:

- one **`Exchange`** per instance (job / taken / done sequence numbers, the caller's
  *waiting* flag and bell, *closed*), beside the instance's job area in its slot
  (`bridge::slot`: the stereo in/out, the sidechain, 2 048 parameter changes, 512 notes,
  the transport, 256 reported events — fixed at load, about 180 KB at 8 192 frames);
- one **`Lobby`** per helper process: the count of its threads *scanning* and the lobby's
  bell. A helper runs a small pool of processing threads (as many as its instances, at most
  the cores beside Cantus's own and 8); each serves every instance (`ipc::serve`): it takes
  a pending job by compare-and-swap, runs it, keeps scanning for **50 µs**, then announces
  it stops, scans once more and parks. Cantus rings the lobby only when no thread scans.
  So within a block the second and later isolated plug-ins of a helper — and an
  instrument's further note segments — are taken a cache-line hand-off after they are
  published, and only the first job after a quiet spell pays a wake-up. One short spin per
  burst, per helper, not per plug-in: R15's spinner budget, held by construction.
- Lost wake-ups are excluded by the order of the operations (each side announces before its
  last look, publishes before it reads the other's announcement — Dekker's pattern under
  sequential consistency; bells are eventcounts). `cantus-rt/tests/ipc_model.rs` explores
  every interleaving of two callers and two workers over both bell semantics (a Windows
  auto-reset event, a Linux futex) and finds none; it also proves it would catch the
  classic mistake. `ipc_process.rs` runs the protocol between two real processes. (loom is
  not a dependency of the workspace; an exhaustive model of our own was preferred to a new
  crate; Miri runs the model, which makes no OS call.)

ADR 0031 §2.7 limited `cantus-rt` to 2 000 lines; with the IPC modules (≈ 900 lines with
their documentation) it is past that: the limit is amended to *small modules, each with
one job, nothing audio-specific* — the rule that mattered.

### 3. The helper, and what a crash does

`cantus-sandbox --serve <protocol> --crt-fma3 on|off --group <key> --lobby <name>`, started
per group, found beside the program (`CANTUS_SANDBOX_EXE` for tests and development), its
own version resource. Its main thread loads the plug-ins (`mark_main_thread`), answers
Cantus's calls on its standard pipes (JSON frames with a binary blob for states,
`bridge::proto`, protocol-versioned), pumps the plug-ins' windows with
`MsgWaitForMultipleObjects`, has COM (`OleInitialize`) for VST3 editors, and forwards the
plug-ins' requests and changed values. **Life** (amended, review isolation-2 #0/#48): the
main thread bumps a progress count each time round its loop and each time a **pulse** runs
— a hidden message-only window whose 100 ms timer and wake-ups are dispatched by whatever
message loop the thread is in, a plug-in's modal dialog or a dragged window included; a
heartbeat thread of its own says *alive* every 200 ms while that count moves. From a loop
nested in a plug-in's code the pulse also **serves Cantus's calls** (a save does not wait
for a file dialog to close), except those that end or replace an instance — unload, drop
audio, restart, close editor, quit — which, with every call after them, wait for the main
loop. Before calling into an instance the main thread writes its number into the lobby
(`MAIN_AT`), and each processing thread knows its job's; a crash handler (the
unhandled-exception filter on Windows, the fatal signals on Unix) writes the faulting
thread's instance (`CRASH_AT`) as the process dies, so a crash is **laid on the plug-in
that did it** (#49). It moves its own
standard handles to NUL before loading anything (a plug-in that prints cannot break a
frame), has no crash dialog (`SetErrorMode`), is started without a console window
(`CREATE_NO_WINDOW`), and ends with Cantus: Windows puts every helper in one job object
with *kill on job close*; Linux asks `PR_SET_PDEATHSIG`; both end when their input closes.
It mirrors Cantus's floating-point world: the CRT FMA3 switch as Cantus has it, and each
job's denormal flushing as the calling thread had it.

**Groups**: *Together* (one helper for every plug-in set to it), *By vendor* (one per
maker — cheap, so built), *Individually* (one per instance), *Within Cantus*.

**A crash** (the helper ends) is seen at once — the reader thread's pipe closes, the
watchdog's `try_wait` names how it ended (the Windows exception, the Unix signal) — and
every instance's exchange is **closed**, which wakes an audio thread waiting on it. **A
hang** kills the helper, and is then a crash: a real-time job unfinished for 2 s — judged
by the exchange, so a late job that finished while nothing called again (a bypassed slot,
a restart) is no hang (#2, #38); an offline job unfinished for 30 s (#1, #11); or a main
thread with **no life for 5 s while no request — waited for or not, background saves and
unloads included — is under way within its own limit** (load 30 s, state 10 s, others
5 s; #41). A request past its limit while the heartbeat still comes is waited for, up to
six times its limit (a modal dialog during a load). `Sandboxes::set_limits` shortens them
for tests. Saving a plug-in Cantus knows to be in trouble (a job unfinished, no heartbeat
for a second) answers at once with its last-good state (#51). From the crashing block on, an **effect passes its input through,
delayed by the latency it reported** (as a bypassed slot plays, so nothing in parallel
moves against it); an **instrument is silent**; a block that only went **late** does the
same and is counted (`Status::late`, shown in the status bar as *Plug-ins late*, beside the
engine's own drops: the bridge knows it before the engine could, and the engine stays
unaware; it counts **jobs** that missed their deadline, one per late job, #35). What the
blocks that passed without the plug-in carried is **kept** in buffers made with the half —
the last value of each parameter, the note-offs, chokes, expressions and controllers, a
reset — and goes in at the start of its next job, so no note rings on and no value is out of
step (#3, #10); a note-on late by a block is dropped; a full buffer resets the plug-in. The
crashing block itself is the one glitch: if the system takes longer to end the process than
the block's deadline, it waits for the deadline — **to the microsecond**: the caller's
sleep ends on a high-resolution waitable timer (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`)
beside its event, not on the 15.6 ms system tick (#12; a late wait measured within 2 ms of
a 5 ms deadline) — and every block after it passes at once. A block the engine sends an
instance after its slot left passes at once too: its exchange is closed before the helper
is told to unload it (#5).
`MainThread::idle` says `Notice::Crashed { reason, blame }` once, and
`MainThread::stopped` says it whenever asked (a crash while a state loaded that no `idle`
reported, #45); `PluginHost` marks the slot crashed and counts it by its **blame**: the
plug-in that did it this session and in the plug-in database for good
(`PluginDb::crashes`); a neighbour in the same helper not at all; when nothing says whose,
once per helper death this session and not remembered (#25, #49). It tells the application
`HostNotice::{PluginCrashed, InstrumentCrashed}` — also for a plug-in whose helper died
while it **loaded**, which is reloadable and tried again when where it runs changes (#15).
**Renders** (export, stems, freeze, bounce, a modulator's bake) run their isolated plug-ins
in helpers of their own (`render-<group>`): a crash there never takes the live ones down;
an offline wait is sliced, so a render's Stop ends it (`PluginHost::cancel_render`) and a
hang past the offline limit ends the helper; and a render that lost a plug-in says so
(`PluginHost::render_failures`) instead of reporting success (#6, #39, #46, #47).
Numbers that are not finite (a `-inf` dB minimum, a NaN value) cross the channel as the
nearest finite ones (`proto::Finite`, #4).

### 4. Nothing lost, nothing different

- **The same bits.** The helper runs the same backends on the same block splits, with the
  caller's FTZ/DAZ and CRT setting: an isolated plug-in's output equals the in-process
  output to the bit — tested at block sizes 1, 37, 64 and 512 for CLAP and VST3 effects and
  instruments, through the whole engine at 64/256/1 000 frames, and for offline renders.
- **The last-good state.** `BridgeMain` keeps the plug-in's state: the one it started with
  (sent with the load), every state loaded into it — set before the call, so a helper that
  dies loading it, or is dead already, keeps the state the project asked for (#9, #40,
  #44) — and a background refresh half a second
  after it says its state changed, at the end of every gesture
  (`MainThread::checkpoint`, called by `PluginHost::record`), and every ten seconds while
  its values move; a background answer older than a state loaded or saved since is
  dropped, and a refused one asked again two seconds later (#7, #8, #50). Once the helper
  is gone, `save_state` answers with it — so
  `project_for_save`, parking a slot and reloading all keep what the user had; the
  project's own values (kept by commands) are flushed on top at a reload.
- **The interface never waits in steady state**: values come from a mirror the helper
  keeps current (after every call and whenever they change), texts are cached and fetched
  in the background, a flush the plug-in asked for arrives as news. Calls that must answer
  are user actions (load, a state, an editor, a restart) and wait at most their time limit,
  once — the helper is then killed.
- **Editors** are top-level windows the helper makes and owns: an owned or child window
  across processes would join the two processes' input queues, and a hung helper would
  freeze Cantus's window (Raymond Chen, *Is it legal to have a cross-process parent/child or
  owner/owned window relationship?*). `editor_is_floating` is true; bringing it forward
  (`MainThread::focus_editor`) and closing are proxied; its close box comes back as
  `Notice::EditorClosed`. The cost: the window is not kept above Cantus's.
- **Hosted instruments keep their per-segment calls** (ADR 0019 left whole-block hosted
  units open): measured, a further segment in the same block costs a hot round trip
  (§5, ≈ 1 µs), so the engine's loop is not restructured.

### 5. Measured (`cantus-bench --only isolation`, i7-12700H, release, the machine shared with two builds)

The test gain (`cantus-test-clap`, which copies its input: almost no work of its own) for
the round trip; Cantus's own Comprimo exported as a CLAP plug-in (ADR 0020) for a real
plug-in's session. *Hot*: calls back to back (a helper thread still scanning: the further
plug-ins and note segments of a block). *Paced*: one call per device period, the helper
parked between them (the first isolated job of each block). Each figure is one run of
10 000 calls (`docs/perf/isolation.md` has both runs).

| Round trip, µs per call (median / p99) | 64 frames | 128 frames | 256 frames |
|---|---|---|---|
| Within Cantus, hot | 0.3 / 0.3 | 0.5 / 0.6 | 0.8 / 1.0 |
| Together, hot | 0.6 / 1.1 | 0.7 / 1.2 | 1.2 / 1.7 |
| Individually, hot | 0.6 / 1.1 | 0.7 / 0.8 | 1.2 / 1.7 |
| Within Cantus, paced | 1.6 / 6.5 | 3.1 / 10.3 | 5.5 / 13.6 |
| Together, paced | 11.7 / 54.8 | 36.5 / 52.3 | 34.0 / 276 |
| Individually, paced | 14.2 / 44.2 | 26.0 / 262 | 41.6 / 61.8 |

| Helpers' CPU, % of a core per instance (8 instances paced at real time) | 64 | 128 | 256 |
|---|---|---|---|
| Together | 0.55 | 0.35 | 0.20 |
| Individually | 5.8 | 2.4 | 1.5 |

| 32 tracks × one Comprimo, µs per block at 128 frames (serial engine, back to back) | |
|---|---|
| native Comprimo | 563 |
| Comprimo CLAP within Cantus | 593 (1.05× native) |
| Comprimo CLAP *Together* | 773 (**1.30× within**) |
| Comprimo CLAP *Individually* | 1 125 (1.90× within) |

**The design moved by the numbers.** The first build gave each instance its own helper
thread that spun 300 µs after every job: the hot round trip was the same, but the helpers
burned **23.7 % of a core per instance** at 64 frames (the spin, every period) and the
32-instance session cost **1.75×** — every plug-in of a block woke a parked thread. The
lobby (§2) — a pool per helper, a 50 µs spin per burst — brought that to **0.55 %** and
**1.30×**: the first job of a block wakes a thread, the other 31 are taken while it scans.

**Against ADR 0031's phase-6 targets.** *A round trip ≤ 5 % of a 64-sample budget per
group* (67 µs of 1 333): met — *Together* 11.7 µs median (0.9 %), 54.8 µs at the 99th
percentile (4.1 %), and only the first isolated job of a block pays it. *Together with 32
instances ≤ 1.2× in process on the DSP session*: **1.30×, not met** in the serial engine
with every block back to back — 5.6 µs per instance per block of handing the audio to
another core and back (two cache-line round trips, the buffers' transfer, the plug-in's
state on the helper's core), which no wake-up policy removes. In a live session the pool
runs strips in parallel and the helper's threads with it, so the 180 µs spread over cores;
a session of a few third-party plug-ins pays a few microseconds each.

### 6. The choice, and the default

Each hosted plug-in records where it runs — `ExternalPlugin::isolation: Option<
PluginIsolation>` (schema 36; absent while it follows the machine's default, so an old
project saves the same bytes), changed by `Command::{SetPluginIsolation,
SetInstrumentIsolation}` with exact inverses (undo fuzz arm 46). The machine's default is in
Settings' plug-in manager (`Settings::plugin_isolation`), applied on the boot path
(`CantusApp::assemble`). A change moves the plug-in at the next sync with its state; the
replaced instance is kept a second so its helper never unloads what the engine still
calls. Our own effects and instruments are never hosted, so they always run within.

**The default is *Together*.** By the owner's goal — *a plug-in that crashes must not take the session down* —
weighed against the numbers: the round trip is within ADR 0031's budget per group, the
helpers cost under 1 % of a core per plug-in, the bits are the in-process bits, and the
price that remains is 1.30× on a session built to exaggerate it (32 isolated plug-ins, one
per track, in a serial engine). *Individually* costs more (a wake per plug-in per block,
5.8 % of a core each at 64 frames) and is offered where it pays: a plug-in that keeps
crashing. *Within Cantus* stays one click away, per plug-in or for the machine, for a
session where every microsecond counts and the plug-ins are trusted. ADR 0031 said the
default stays *Within* unless the per-group round trip is under 5 % of a 64-sample budget;
it is, so *Together* is the default for a new installation.

### 7. The interface

The crash card (`views::plugin_notice`) in the one corner stack it shares with the audio
notices (`views::audio_notice`, rebuilt as a stack: each card above the last, buttons
wrapping to a second row rather than past the edge): *"<plug-in> stopped working. Cantus
kept playing without it."* — **Reload** / **Keep it off**, *Reload all* for several,
*Run individually* for a plug-in that crashed three times this session; the reasons in its
hint. A stopped slot shows *stopped* in the danger colour (the inspector's insert list), a
stopped instrument a sign on its track icon and an outlined channel button. The slot menu
and a hosted instrument's *Runs* row choose where it runs and offer *Reload plug-in*; the
generic editor has a *Runs* card and the stopped card with Reload; the slot's hint gives
where it runs and its CPU (BACKLOG §3's per-plug-in meter: every hosted unit times its
calls, the helper times the plug-in itself; `PluginHost::cpu`). The manager has *Where
third-party plug-ins run* and the plug-ins that ever stopped here (*Forget*). Both
languages (`locales/areas/isolation.*`).

## Consequences

- New: `cantus_rt::{shm, ipc}`, `cantus_host::bridge::{slot, proto, sandbox, audio, main}`,
  `session_iso.rs`, the `cantus-sandbox` crate and program, `PluginIsolation`, schema 36,
  the two commands, `Notice::Crashed`, `HostError::Crashed`, `HostNotice::{PluginCrashed,
  InstrumentCrashed, IsolationUnavailable}`, `MainThread::{focus_editor, checkpoint,
  own_time, isolated_in, late_blocks}` (defaults, so a backend that does not implement them
  still builds — ADR 0019's rule for the contract), `CpuMeter`, `PluginDb::crashes`,
  `PluginHost::{reload, reload_instrument, reload_all_crashed, crashed, cpu, …}`, the
  test plug-ins' hidden *Fault* and read-only *Process* parameters, the bench's
  `isolation` group.
- Packaging: `cantus-sandbox.exe` built at the lowest level, staged, checked for its
  version resource, installed and waited for by the installer, on the signing list, in the
  notices' roots; the Windows release workflow builds it; DEVELOPING says how to build it.
- Linux: the same code — `shm_open`, futex bells, `PR_SET_PDEATHSIG` — builds and runs; the
  editors are unsupported there as in process. macOS builds with a sleep-loop bell.
- Left to later rounds by design, not by omission: the async option for live nodes, and an
  ahead path that hides the round trip entirely (ADR 0031 §3); a window kept above Cantus's
  while it is the active application.

## What the tests hold

- `cantus-rt`: `ipc_model.rs` (every interleaving; the model catches a late announcement),
  `ipc_process.rs` (round trips between two processes carry their data; two callers served
  together; a dead worker process closes the exchange and wakes a waiting caller; a late
  job finishes later and is seen).
- `cantus-sandbox/tests/bridge.rs` (real helpers): CLAP and VST3 gain isolated equal in
  process at 1/37/64/512 frames and report the helper's pid; the CLAP synth and the VST3
  pulse play their notes at their samples as in process; an abort in `process` leaves the
  block dry, every later block dry at once, the crash said once, the last-good state
  saved; a hang never holds the audio thread past the deadline nor the frame, and is
  ended; aborts in `save_state` (the last-good state answers), `load_state`, `activate` and
  the editor are errors and crashes; *Together* shares one process and one crash takes
  both, *Individually* plays on; a missing helper is an error.
- `session.rs` (through the engine): bit-identical playback isolated vs within; a crash
  mid-song keeps the song playing (the crashing block on equals the bypassed reference to
  the bit), is told once, and Reload brings a new helper with the values set, to the bit;
  the project saved after a crash keeps the state the plug-in had (a gesture's last
  value); an instrument goes silent and comes back; changing where it runs moves it and
  keeps its sound, undo moves it back, the machine's default moves the ones that follow
  it; an offline render through isolated plug-ins equals the in-process one; a hang never
  blocks the frames; every crash counted and remembered.
- `rt_alloc.rs`: the audio half allocates nothing — normal blocks, a latency line, a
  reset, a stop, dry blocks after the helper died.
- `cantus-core/tests/plugin_isolation.rs`, the undo fuzz's arm 46; `format.rs`'s
  `schema_36_keeps_where_a_hosted_plugin_runs`; `cantus-ui/tests/plugin_isolation.rs` (the
  boot path applies the default; one card, its answers; the card and two audio notices
  never overlap, in English and Spanish; the menu's choice is one undo step; both
  languages); `xtask`'s package tests (the helper staged, zipped, waited for).
- Review isolation-2 (round 29's fix round; `docs/round29/isolation.md` has the table):
  `ipc_process.rs` — a late wait ends within 2 ms of its deadline; an unlinked region keeps
  sharing. `bridge_faults.rs` — hangs in a save, a state load, the editor and a background
  save end within their limits, the save answering with the last-good state and saving
  meanwhile not waiting; a modal loop in a save or in the plug-in's window is not a hang,
  and calls are served from inside it; a late block's dry blocks keep their notes and
  values, pass at once, and count one late job; a late job that finished is no hang; an
  offline render through a hung plug-in ends, and a cancelled one stops waiting; a block
  after its instance left passes at once; a state asked for is kept when the helper dies
  loading it; a refused background save is asked again; an older background state is
  dropped. `bridge.rs` — instruments at 1/37/64/512 with notes inside blocks; the
  sidechain to the bit; a crash with latency plays the delayed input to the bit.
  `session_faults.rs` — the worker pool equals the serial engine and in process; an
  instrument to the bit through the engine; a crash in a shared helper counted once, for
  the one that did it, by abort and by access violation; the missing helper said once;
  the processes each choice makes; a crash while loading reloadable and retried where it
  runs; a render that loses a plug-in says so, and a hung one ends. `rt_alloc.rs` adds the
  late path, the crashing block and a live stop; `worker.rs` the retired snapshots;
  `proto.rs` non-finite numbers; `bridge/mod.rs` the groups; `session_iso.rs` a scan
  keeps crash records; `cantus-ui` the card's pruning, answers, three-button layout at
  420×600, *Run {name} individually*, a stopped plug-in's window, the reasons' details,
  and `plugin_isolation_boot.rs` — a start-up project's plug-in first loads where this
  computer says.
