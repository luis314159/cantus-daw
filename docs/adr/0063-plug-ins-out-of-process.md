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
heartbeat thread of its own says *alive* every 200 ms while that count moves; every frame
the helper sends counts as life too. *(Superseded by the isolation-5 amendment below: the
helper serves nothing from a nested loop.)* From a loop nested in a plug-in's code the pulse
also **serves Cantus's calls** (a save does not wait for a file dialog to close). Held for the
main loop — and said so, `Message::Deferred` — are only quit and the calls that end or
replace **the instance whose window runs the loop** (unload, drop audio, restart, close
editor), and the later calls on that same instance, in order; every other instance's
calls are served (review isolation-3 #4, #17, #36, #44). The helper says when it starts
each request (`Message::Started`), and a request's limit runs from then (#3, #37). A
plug-in window's messages run as calls into their instance (its root window maps to it),
so the main thread names that instance in the lobby while they run (`MAIN_AT`), as it
does in its calls and ticks; each processing thread names its job's. A crash handler —
the unhandled-exception filter (put back after each load, handing on to a plug-in's own),
the C runtime's abort signal, on Windows; the fatal signals, on a stack of their own, on
Unix — writes the faulting thread's instance (`CRASH_AT`), or *outside* for a thread
calling into none, as the process dies; and a crash in a window's timer callback is not
swallowed by Windows but ends the process like any other. A crash is **laid on the
plug-in that did it**: a culprit Cantus knows (its job or call hung) alone; else the
handler's word; else the single candidate among the main thread's instance and the
running jobs; several, or *outside*, are nobody's (`Blame::Unknown`), never an innocent
neighbour (#49; isolation-3 #0, #1, #19, #20, #38, #39, #45). It moves its own
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
thread with **no life for 5 s unless the request it is serving (the last it started) is
still within its own limit** (load 30 s, state 10 s, others 5 s; #41; isolation-3 #3,
#37). A helper that shows life is never hung, whatever waits on it (isolation-3 #36,
#44); a held request is never judged. Cantus's interface, though, never waits past a
call's own limit: a call not answered in it gives up with `HostError::Busy` — a save then
answers with the last-good state, a restart is picked up on a later frame — and the
helper is not killed for it (#4, #11). A pause of Cantus itself (a laptop asleep) longer
than the silence limit restarts the clocks instead of killing every helper (#49).
`Sandboxes::set_limits` shortens the limits for tests. Saving a plug-in that is itself in
trouble (its job unfinished, its own request overdue, its helper hung) answers at once
with its last-good state (#51); a merely busy neighbour does not make a save stale
(isolation-3 #2, #24). A helper that ends before its handshake is a load failure, never a
plug-in's crash, and a helper of another build answers the handshake with its own protocol
(#40). From the crashing block on, an **effect passes its input through,
delayed by the latency it reported** (as a bypassed slot plays, so nothing in parallel
moves against it); an **instrument is silent**; a block that only went **late** does the
same and is counted (`Status::late`, shown in the status bar as *Plug-ins late*, beside the
engine's own drops: the bridge knows it before the engine could, and the engine stays
unaware; it counts **jobs** that missed their deadline, one per late job, #35). What the
blocks that passed without the plug-in carried is **kept** in buffers made with the half —
the last value of each parameter, the note-offs and chokes, the last value of each
controller and expression, a reset — and goes in at the start of its next job, so no note
rings on and no value is out of step (#3, #10); a note-on late by a block is dropped; a
full buffer resets the plug-in (and drops what the reset supersedes). The resuming block's
own events come first in room, the carried note-offs next, the carried controllers last;
a carried note-off with no room resets the plug-in first, and what still finds none is
counted as lost (isolation-3 #8, #9, #16, #23, #41). What the late job itself reported —
its gestures and values — reaches the host with the next block (#12, #42, #46). The
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
once per helper death this session and not remembered (#25, #49); several instances of one
plug-in in one helper are counted once per death (isolation-3 #0, #20). It tells the
application `HostNotice::{PluginCrashed, InstrumentCrashed}` — also for a plug-in whose
helper died while it **loaded**, laid on whoever the helper's end is laid on, which is
reloadable and tried again when where it runs changes (#15; isolation-3 #7); a reload that
crashes again is counted again, its card comes back, and its reason is said in the
interface's words (isolation-3 #15, #31).
**Renders** (export, stems, freeze, bounce, a modulator's bake) run their isolated plug-ins
in helpers of their own (`render-<group>`): a crash there never takes the live ones down;
an offline wait is sliced, so a render's Stop ends it (`PluginHost::cancel_render`) and a
hang past the offline limit ends the helper; and a render that lost a plug-in — crashed,
hung, or not even loaded in its render helper — says so (`PluginHost::render_failures`)
instead of reporting success (#6, #39, #46, #47; isolation-3 #6, #47). One render at a
time: an export, a freeze or bounce, and a modulator's bake wait for each other (they share
the render instances; isolation-3 #5, #10, #13, #21, #48). A frame cut short by a dying
helper is the stream's end, not a broken protocol (isolation-3 #43).
Numbers that are not finite (a `-inf` dB minimum, a NaN value) cross the channel as the
nearest finite ones (`proto::Finite`, #4).

**Amended (review isolation-4).** *Its first four points — serving from a dialog inside a
call, the calls' waits and retries, `Limits::heavy`, the message hook — are superseded by
the isolation-5 amendment that follows them.*

- *Serving from a dialog inside a call.* The helper keeps each instance in a cell of its
  own and holds none across a call, so its pulse serves from **any** loop nested in a
  plug-in's code — a licence nag inside a load, a progress window inside a state — not
  only from a window's: a call on another instance is served there; a call on the
  instance whose call is under way waits behind it, and Cantus is told so
  (`Message::Behind`, protocol 5) and waits for it as for any call ahead of it; the calls
  that end or replace the looping instance stay held (`Deferred`). A request served past
  its own limit while its helper lives — a plug-in waiting on a window of its own — makes
  the calls queued behind it give up at once (told *behind*) or after a 2 s grace, and its
  slot shows a card: *X is waiting on a window of its own*, with **Show it** (Cantus lets
  the helper take the foreground, `AllowSetForegroundWindow`, before every call that may
  open a window, and `Call::Surface` brings the dialog forward) and **Stop it** (its
  helper ended, told as its own crash).
- *Calls.* A call's limit runs from when the helper starts it (from posting while it waits
  its turn). A call given up on (`HostError::Busy`, which says whether its own window held
  it) still has its answer go where it belongs: a state becomes the last-good one, a
  parameter list is taken and the host told again, a window asked for opens and is kept as
  open (`Notice::EditorOpened`). A restart is waited for 100 ms when asked, then only
  looked at, frame by frame; every wait looks whether the answer is in, or the call held,
  before it sleeps. A load the helper serves while it lives (its own dialog up) is waited
  for, up to `Limits::modal` (10 min); one given up on is tried again — after 2 s, doubling
  to a minute — and *Reload* is offered for any plug-in that could not be loaded. Saving
  answers at once with the last-good state when the plug-in's own request has run a second
  with its helper silent: its main thread is stuck in that plug-in's code.
- *Judging life.* Until its first frame a helper is judged by its handshake's own limit (a
  slow start is no hang). A load, a state or an unload that runs with the helper silent is
  judged by `Limits::heavy` (120 s), not its call's limit: a sampler reading its samples is
  slow, not hung. A pause of Cantus restarts each instance's late-job clock as well
  (`Sandbox::pauses`), and an offline wait whose slice took far longer than asked restarts
  its job's clock. A job not even taken by its deadline marks its helper **stalled** (a
  lobby word beside the processing threads' count of finished jobs): the helper's other
  instances pass dry at once until a job of it finishes — a stalled helper costs one
  deadline in a callback, not one per plug-in.
- *Whose an end is.* The crash handler names the instance whose **library** the faulting
  code is in — each instance's libraries, its own and those that came with it from its
  folder, are learnt at its load — else the faulting thread's, else *outside*. Every
  message the main thread retrieves, in any loop (a `WH_GETMESSAGE` hook), runs as a call
  into the instance it belongs to: its window's (by root owner), or the one whose library
  its window class or timer procedure comes from — so a JUCE timer, a VSTGUI thread timer,
  or a neighbour's window painted inside another plug-in's dialog is laid on the right
  plug-in. The handler's word stands for an instance still loaded or one **leaving** (its
  destructor runs as a call into it, and its libraries are forgotten only after), and over
  a hang Cantus decided while the crash was being reported. An end Cantus decides (a hang,
  a silence, a broken protocol, a stop) is laid on exactly what the main thread was doing,
  or on nobody — never on a request it had not started, nor on a job that happened to run.
  The only plug-in of a helper is laid any end. An end nothing lays on one plug-in is
  counted for none and offers none *Run individually*; a death is counted once per helper
  by a serial that is never reused (a process id may be); a plug-in that crashed as it left
  is counted after it left (`Sandboxes::take_departed_crashes`); a reload that fails says
  why now and whose. The chained crash filter is guarded against entering itself again (a
  reporter that hands back to ours), never called once its library is gone, forgotten when
  ours is on top anyway, put back after every call (a plug-in installs its reporter in a
  load, an editor or a restart), and a crash record it resumed is taken back; on Unix ours
  goes back only where the default was restored, so a plug-in's own fault handlers (a JIT,
  a VM) stay. Told to quit, a helper is given the call limit to end its plug-ins by itself.
- *Renders.* An offline instance's state and values are waited for as long as its helper
  lives — until Stop, or the offline limit, which ends it; a render instance that could not
  be made — not loaded, its state refused, not settled — is one the render never had
  (`RenderLoss::NeverLoaded`, said as such: *the file was made without it*); one begun from
  a busy plug-in's last-saved state is said, not failed (`RenderLoss::Stale`). One bake at
  a time.
- *The carry* keeps what dry blocks carried **in the order it came**: values folded to
  their last (the older removed, the newer appended, so the order of last values stands),
  poly pressure per key, a switch (sustain and the other pedals) its last lift and press,
  (N)RPN, data entry and channel-mode messages never folded; a reset supersedes only the
  voices' events, never the controllers; the reset a resuming block needs first is decided
  before anything is merged, and one that runs late leaves the carry whole; what finds no
  room is counted.
- *A late job's events* are taken by the audio half from its own record of the job,
  whatever the interface saw first.

**Amended (review isolation-5): four rules, and what they took away.** The fourth review
found the call handling not converging (53 → 50 → 67 → 74 findings): each round had added a
mechanism — calls served from loops nested in plug-in code, `Behind`/`Deferred`, a *heavy*
and a *modal* limit, a message hook for blame, load retries, synchronous waits bounded by
ever longer limits — and each opened edge cases of its own. The call handling was rebuilt
on four rules, and what they made unnecessary was removed.

1. **The interface thread never waits on a helper** beyond a frame's share
   (`bridge::main::FRAME_WAIT`, 30 ms, and only where the answer usually comes at once: a
   window opened, a restart). Every request is posted (`Sandbox::post`, its `Purpose`
   saying where the answer goes); the helper's reader thread lands the answer in the
   instance's inbox; `BridgeMain` takes it each frame. **Loading is one request** carrying
   the plug-in, its state and its values (protocol 6): the helper loads, gives the state
   and the values, settles a restart the state asks for, and only then puts the instance
   in its processing threads' table — its first job has its state. Until then its audio
   half passes dry; `idle` then says `Notice::Loaded`, and `MainThread::take_audio` hands
   the host the audio half made with its latency, which replaces the first (a half
   replaced by a newer one over the same plug-in tells the helper nothing as it goes — no
   drop, no stop: the test that found the plug-in playing dry after its load is
   `session_faults.rs`'s worker-pool test). A fresh insert's values join its insert's
   undo step when they come — while that step is on top and nothing was undone since,
   then sealed; otherwise they are not recorded (the plug-in holds them) and never make a
   step of their own nor clear a redo (`PluginHost::load_key`, `inst_load_key`, a key of
   each pick's own; isolation-6). A value set meanwhile stands. States (saved and given),
   reloads, moves, windows opened and closed, parameter lists: requests answered later.
   While the helper's main thread has been in a loop of the plug-in's own code for 2 s (a
   dialog: `Message::Nested`, `Unnested` — or of code nothing names, or of an instance that
   left: then each plug-in of that helper), the plug-in's **waiting card** shows; a request
   merely slow shows none — its slot, its editor and the inspector say *loading*
   (isolation-6) — *is waiting for
   you*, its whole name in the card's hint — with *Show it* (`Call::Surface`, served by
   the helper's reader thread whatever its main thread is doing: the topmost window of
   the main thread that is the dialog — modal, its owner disabled — else the topmost
   visible one; never the last one listed) and *Stop it* (its helper ended at the user's
   word: never counted as its crash; the plug-ins that share its process are told they
   went with it — *its process was ended to stop X* — and the card says beforehand how
   many will).
2. **No stale state is ever reported as fresh.** A save asks every plug-in for its state
   (`PluginHost::ask_states`), looks frame by frame (`states_in`), and is written once each
   has answered or passed its own limit **counted from when its helper started the
   request** (`MainThread::{ask_state_now, poll_state, state_waiting}`): a background
   refresh asked since the last change is the one waited for; a request held behind a
   plug-in's dialog waits with it (the card says so) — no grace cuts it short; a slow but
   healthy save is no trouble. One not to be had is saved as it was last said, and the
   save names it on the status line (`PluginHost::saved_stale`, `isolation.save.*`). New,
   Open and the close (`wait_for_save`) take what each said last at once, and name it;
   autosaves gather the same way. A render gathers the live states the same way before its
   instances load (`PluginHost::render_ready`, polled each frame by an export, a bake, a
   freeze or a bounce as they wait for samples); one not had in time is the render's
   warning (`RenderLoss::Stale`), said with its outcome — after a bake's or a bounce's own
   word, never under it. A move to another process waits for its fresh state the same way.
3. **Nothing that ends or replaces an instance is served from a nested loop.** The helper
   serves **nothing** from a loop nested in a plug-in's code; requests wait for its main
   loop, in their order — no instance is ever ended, replaced or called into under a
   plug-in's own code (no destructor runs with its code on the stack), and no backend's
   lock is entered again on the same thread. Rule 1 makes this free: nothing in Cantus
   waits for them.
4. **No unbounded retries.** A load past its limit from when its helper started it — its
   helper alive, no dialog of its holding it — is a **failed load** (`Notice::LoadFailed`:
   kept as one that could not be loaded, with *Reload*, worded as such — never *install
   it*); its late answer is not taken. A load held by its own dialog waits for it, with its
   card. A render's instance has no card: its load's limit holds inside a dialog too. The
   limits are the same live and offline. Nothing is tried again by itself; a helper that
   never started is a failed load too, nobody's crash.

**Removed, and why.** Serving calls from nested loops — `Here`'s re-entrant cells, the
pulse's serving, `Message::{Deferred, Behind}` (protocol 5 → 6): rule 3, and rule 1 makes it
unnecessary. `Limits::modal` (10 min), and the trouble checks that cut a save short
(`troubled`, `in_trouble`, `fresh_state`'s `Busy`, `held_up` and its 1 s grace): nothing
waits, so no limit needs stretching for a waiter, and a request is judged only by its own
limit from its start. (`Limits::heavy` went with them, and came back in isolation-6 with
one role: the helper's life, below.) The synchronous `Sandbox::call` on the
interface thread: rule 1. The load retry with its doubling wait (`Missing::retry_at`,
`PluginHost::wants_sync`, the busy failure): rule 4. The offline load wait inside the audio
half: a render's units are made once its plug-ins are in. The `WH_GETMESSAGE` hook and its
marks: its mark outlived the message it was set for (#2, #23, #33, #64). Blame is now the
faulting thread's own instance — the main thread's call, tick or the window message its own
loop dispatches, a processing thread's job — and, inside a plug-in's own loop (where any
window's message may run), the one instance whose library the faulting code is in, else
nobody; a library several instances share names none of them (#7, #32, #55); the Unix
module walk is Linux-only (`dl_iterate_phdr`, #9, #18, #25, #42, #58, #72); the crash filter
goes back on top after each call, the abort signal is ours only from the start, so a
plug-in's own reporter keeps it (#73). A stalled helper's mark is cleared by the first job
a processing thread finishes after it, read before the job's look, so it never stays (#8,
#71). Cantus, quitting, gives its helpers three seconds (`QUIT_GRACE`) to end their
plug-ins by themselves before the job object ends them (#24, #54).

**Amended (review isolation-6: 35 findings, from 74).** The rules held; their edges did not:

- *A slow load is no hang.* The waiter gives up at the request's own limit (a failed load,
  a state kept as it was last said), but the helper of a load, a state or an unload, silent
  — a sampler reading its samples without pumping — is judged hung only past
  `Limits::heavy` (120 s, four times the limit); a load past its limit is failed before its
  helper's life is judged in the same frame. Tested at the shipped ratio of silence to
  limit to heavy.
- *A render's preparation is its owner's and its content's* (`RenderFor`: an export, a
  bake, a freeze or bounce; keyed by every part's values and, for a freeze, its states):
  asked for other content it is made anew; another owner waits while it is in use — its
  owner asking each frame, or its render running — instead of taking it (the bake and the
  freeze took it from each other every frame); an owner that gives up drops it
  (`drop_render`: a cancelled freeze, a stopped export, a freeze with nothing to render);
  only its owner's look keeps it alive (`render_waiting`). A part whose instance was never
  made is the render's loss, said. A bake asks whether another render runs before it
  prepares; a freeze asks its plug-ins' states as they are now and waits for them, as a
  save does, and says which it kept as they were last said.
- *A state, never lost nor overwritten.* A state answer that came in is taken before a
  state is given (a save waited for ever otherwise); the state a load came back with never
  replaces one given while it loaded; a state taken (`poll_state`) ends its request, so an
  old answer never reads as fresh later. A plug-in let go — its slot removed, its chain
  frozen, every plug-in on a device change — asks its state as it is now and parks it once
  in (the slot loads again after: `take_resync`), else parks what it said last and says so
  (`HostNotice::StateKept`); A/B and a preset say when the state they took may lack a
  change.
- *A helper that greeted with another protocol* never started: a failed load, nobody's
  crash. *The autosave* waits for its plug-ins' states at most the state limit, then writes
  what each said last; it and a waiting export belong to the project they were asked for.

**Amended (the final review, final-B: six findings).** A plug-in *leaving* — let go, its
state as it is now on its way — is still the project's:

- *A render waits for it.* A heard plug-in that is leaving, or not synced yet, is a
  pending part of the preparation, made a part once it is loaded live again; only a frozen
  chain's resting plug-ins (as the last sync found them) are none of the render's. A
  render that begins without one it should have says so (`NeverLoaded`) — never an export
  without an insert reported done, nor a freeze failed as *missing* for the session.
- *It is waited for at most the state limit*, from when it was asked — started or not, so
  a helper held by a dialog never keeps it for ever: then it is parked as it was last said,
  said (`StateKept`), and loaded again. Meanwhile its card shows when its helper waits on a
  window, and *Show it* / *Stop it* reach it; a save waits for its state and saves it as it
  is now, or says so. An instrument picked again plays as soon as it is loaded: the one
  before it holds nothing up, and its state is nobody's.
- *A preparation asked anew* (a state that moves at every look, in process) keeps each
  instance whose part is the same — the same plug-in, where it runs, its values and state —
  so another plug-in's render instance is never loaded again for it: the freeze begins.
- *A bake given up* — its project replaced, refused, or needing no render after all —
  drops its preparation, as an export and a freeze do.

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
- **The interface never waits on a helper**: values come from a mirror the helper keeps
  current (after every call and whenever they change), texts are cached and fetched in the
  background, a flush the plug-in asked for arrives as news, and every other call is a
  request answered later (§3, isolation-5).
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

**After isolation-5** (the calls made asynchronous; the audio half reads two more flags
per block): run again, the same order — *Together* 0.6 µs hot and 14.0 µs paced at 64
frames (p99 43.2), 0.51 % of a core per instance; *Individually* 5.7 %; the 32-instance
session 1.16× within on that run (`docs/perf/isolation.md`, run 3).

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
- Review isolation-4 (the table in `docs/round29/isolation.md`, each fix seen failing
  first, by the test against the code before it or by putting the fault back):
  `bridge_faults4.rs` — a late job's events survive the interface seeing it finish first;
  a stalled helper costs one deadline; a pause while a job is late ends nothing, and a real
  silence after a pause still does; a save queued behind its own slow save waits from its
  start and is the state after the change; a restart is polled, never waited for, behind a
  dialog or a slow neighbour; a dialog inside a call never holds its neighbours (their
  states taken from inside it, the whole save within one limit); a slow state past its
  limit is no hang and its answer is kept; a helper slow to start is not hung; a parameter
  list and a window given up on are taken when they come; saving while its own save hangs
  answers at once with real limits. `bridge_blame.rs` — a crash in a destructor is the
  leaving plug-in's; a crash and a hang in a framework's timer, and a hang in its hidden window, are its plug-in's; a
  neighbour's window crashing inside a dialog is the neighbour's; a crash reporter that
  hands back changes nothing; a fail-fast with two jobs running is nobody's; the last
  plug-in's destructor runs to its end. `session_faults4.rs` — a load given up on while
  busy is tried again; an end nothing lays on one plug-in is counted for none; a reload
  that fails says why now and whose; a plug-in that crashes as it leaves is counted; a
  render waits for a state that takes its time (a state only the state carries), says so
  when it never comes, when it is refused, and when the plug-in crashes loading.
  Unit tests: the carry (order, folding, switches, poly pressure, resets, room, a late
  reset first), `culprits` split per finding, a long slice is a pause. `cantus-ui`: two
  bakes never overlap; render losses, reload failures and a busy window in the interface's
  words; *Reload* for a plug-in that never loaded; the waiting card.
- The final review, final-B (`session_faults7.rs`): a render waits for a plug-in leaving
  and never lacks one silently, and a save waits for it; a state that moves at every look
  never keeps a freeze's render from beginning; plug-ins let go during a dialog are bounded
  and show its card; an instrument picked again plays while the one before it leaves; the
  test gain's quiet tag (`TAG_QUIETLY`) is its own doing — and `session_faults6.rs`'s
  parking test now holds its claim with it. `cantus-ui`: a bake dropped for another project
  lets its preparation go.
- Review isolation-6 (`session_faults6.rs`): a slow silent load fails at its limit and its
  helper and neighbour live, and a slow silent state is no hang — at the shipped ratio; a
  preparation is its owner's and its content's; a render without its instance says so; a
  state given while it loads stands; a state answer is never lost; another build's helper
  is a failed load; a dialog nothing names shows every card of its helper; a slow load
  shows no card and says *loading*; a plug-in let go parks its state as it is now or says
  so, and a device change loads it again; a fresh insert's values make no stray step and
  lose no redo. `cantus-ui`: two instrument picks are two steps; stale reasons in their
  words, one neighbour said as one; a freeze asks its states first; an export waiting for
  another project is dropped; an autosave waits its while and belongs to its project.
- Review isolation-5 (the four rules; the table in `docs/round29/isolation.md`):
  `bridge_faults.rs` — a plug-in window's modal loop holds its calls and freezes nothing
  (every look under 50 ms, the save asked meanwhile waits and is then answered as it is,
  the card shows); a hang in a save gives what it said last, said as such, within the
  limit. `bridge_faults3.rs` — a 10 s dialog holds the helper, kills nothing, freezes
  nothing, and every held request runs after it in order; a slow neighbour's save never
  makes another's stale; a helper that never started is a failed load. `bridge_faults4.rs` —
  a dialog inside a call holds the helper: the slow one past its limit no longer holds the
  save, its neighbours answer after it as they are; a parameter list is said once it
  comes. `bridge_blame.rs` — a neighbour's window crashing inside a dialog is laid on the
  neighbour by its library. `session_faults4.rs` — a load held by its own dialog waits
  for it and loads; a load past its limit is a failed load, never retried, its late answer
  not taken, Reload loads it; a render whose instance's load passes its limit says it
  never had it. Through the whole engine every earlier test holds with loads answered
  later (`Rig::until_loaded`, `prepare_render`).
