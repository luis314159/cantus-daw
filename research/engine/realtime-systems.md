# Real-time systems knowledge: what the OS, the CPU and the language demand of ADR 0031

*Round 20, engine-research, 2026-10-06. Sources are **[fetched]** (read) or **[excerpt]**
(search excerpt or secondary summary only). Unreachable through the proxy: rossbencina.com
and timur.audio (socket errors / HTTP 500), www.1024cores.net (redirected to 127.0.0.1 —
Vyukov's text was read on its original Google Sites copy instead); nothing was bypassed.*

## 0. What Cantus does today — found in the code and its dependencies

These are not literature, but they decide how much of ADR 0031 §2.6 is real:

1. **The default callback thread is not real-time at all.** Cantus depends on `cpal = "0.18"`
   with no features except `asio` (`Cargo.toml`, `crates/cantus-engine/Cargo.toml`). cpal's
   WASAPI backend promotes its stream thread only under its **`realtime`** feature
   (`cpal-0.18.2/src/host/wasapi/stream.rs`: `#[cfg(feature = "realtime")] if let Err(err) =
   boost_current_thread_priority(…)`), which Cantus does not enable. So in the default
   driver mode (WASAPI shared, ADR 0025 §10) the thread that runs `Processor::process` is a
   **normal-priority thread**. ADR 0031 §2.6 says "the callback thread belongs to the driver
   (ASIO/WASAPI) and is never moved" — true for ASIO (the driver creates and prioritises its
   thread), false for WASAPI, where *we* own it through cpal. With the ADR's live workers at
   MMCSS High, the one thread every block waits on would be the lowest-priority thread of
   the engine: priority inversion by construction.
2. **Even with the feature on, cpal asks for the wrong class.** cpal's boost calls Mozilla's
   `audio_thread_priority`, which on Windows calls `AvSetMmThreadCharacteristicsW("Audio")`
   with no `AvSetMmThreadPriority` (`rt_win.rs`, **[fetched]**, github.com/mozilla/audio_thread_priority)
   — the browser's class, not "Pro Audio" at HIGH.
3. **cpal's WASAPI path is plain shared mode** (`IAudioClient::Initialize` with
   `AUDCLNT_SHAREMODE_SHARED`; no `IAudioClient3`, no exclusive mode). Its period is the
   audio engine's (typically 10 ms). The bench's `b64`/`b128` targets on Windows therefore
   mean **ASIO** (feature-gated) — or an `IAudioClient3` low-latency shared stream, which
   Microsoft documents for small periods (§4). ADR 0031 should say which.
4. The engine reads the clock (`Instant::now`, i.e. `QueryPerformanceCounter`) twice per
   block; that is fine — it does not block.

## 1. The rules (Bencina) and the backoff pattern (Doumler, Renn-Giles)

R. Bencina, "Real-time audio programming 101: time waits for nothing", rossbencina.com,
July 2011 (http://www.rossbencina.com/code/real-time-audio-programming-101-time-waits-for-nothing;
read through a verbatim reproduction on a UCSB MAT 240A course page,
w2.mat.ucsb.edu/240/A/2017/05/17/realtime.html **[fetched, secondary]**): *"If you don't know
how long it will take, don't do it"* — don't allocate, lock, do I/O or call OS functions that
may block; do pre-allocate in a non-time-critical thread; do use O(1) worst-case algorithms;
do amortise across samples. ADR 0005 is this list; ADR 0031 extends it to every rendering
thread, which is right.

D. Rowland, F. Renn-Giles, "Real-time 101 — Part I: investigating the real-time problem
space", ADC 2019 (adc19.sched.com/event/T1LX **[fetched, abstract]**): system calls,
allocations, priority inversion; the cost of mutexes, condition variables and atomics on
real-time threads. T. Doumler's "Using locks in real-time audio processing, safely"
(timur.audio, unreachable) survives as code in **crill** (T. Doumler, F. Renn-Giles,
https://github.com/crill-dev/crill, `progressive_backoff_wait.h` **[fetched]**): a spin with
*progressive backoff* — on Intel `<5, 10, 3000>`: five tries with no pause (~25 ns), ten with
one `_mm_pause` each (~400 ns), then up to 3 000 rounds of ten pauses with a
`yield` (~1 ms); on ARMv8 `<2, 750>` with `wfe`. Backoff "prevents wasting energy, and allows
other threads to progress by yielding … after approximately 1 ms".

**For the ADR.** §2.4's spin "for a time budget — 1.5 device periods — then yields, and
parks only after an idle grace" is crill's pattern at a coarser scale, and its insistence on
a *time* budget is confirmed by the hardware (§7: `PAUSE` went from ~10 to ~140 cycles on
Skylake, so iteration counts mean nothing across CPUs). Adopt crill's inner shape: a few
bare polls, then `spin_loop()` (PAUSE / `yield` on ARM; `wfe` where an event can wake it),
with the clock read every few hundred iterations, not every one.

## 2. Progress guarantees: the ready queue is not lock-free

- D. Vyukov, *Bounded MPMC queue* (1024cores; read on
  sites.google.com/site/1024cores/home/lock-free-algorithms/queues/bounded-mpmc-queue
  **[fetched]**): a "blocking producers and consumers queue … **not lockfree in the official
  meaning**, just implemented by means of atomic RMW operations w/o mutexes", one CAS per
  operation.
- Boost.Lockfree's `bounded_ticket_queue.hpp`, built on Vyukov's algorithm, states the
  consequence (**[fetched]**): "Multi-producer push is not lock-free. A concurrent producer
  that has claimed a slot via CAS but is then suspended can block all other producers once
  the ring buffer wraps around to that slot"; "Multi-consumer pop is not lock-free.
  Symmetric stall risk." In Vyukov's dequeue (and in the prototype's
  `docs/perf/proto/forkjoin/src/queue.rs`, line by line), a consumer that meets a cell
  claimed but not yet published returns **empty** — even if later cells are full.
- M. Herlihy, N. Shavit, "On the nature of progress", OPODIS 2011 (MIT DSpace 1721.1/73900
  **[excerpt]**): lock-free = *somebody* always progresses; wait-free = every operation
  finishes in a bounded number of its own steps.
- D. Chase, Y. Lev, "Dynamic circular work-stealing deque", SPAA 2005, pp. 21–28,
  doi:10.1145/1073970.1073974 **[fetched, Crossref]**; N. M. Lê, A. Pop, A. Cohen,
  F. Zappa Nardelli, "Correct and efficient work-stealing for weak memory models", PPoPP
  2013, pp. 69–80, doi:10.1145/2442516.2442524 **[fetched]** — "the first correctness proof of
  an optimized implementation of Chase and Lev's deque on top of the POWER and ARM
  architectures", with the C11 orderings needed.
- crossbeam-deque (`deque.rs` **[fetched]**): `MIN_CAP = 64`; `push` doubles the buffer when
  full, `pop` halves it at a quarter full; old buffers are freed through crossbeam-epoch,
  whose every 128th pin runs `collect()` and its destructors on whichever thread pinned
  (`internal.rs`, `PINNINGS_BETWEEN_COLLECT = 128` **[fetched]**). **ADR 0031 §2.3's
  rejection of crossbeam-deque is confirmed.**

**What this means for §2.2–2.3.** The ADR's guarantee is "the callback … waits for nodes
**already running** on other workers, never a worker waking up", and "the one way a worker
can hurt it is by being preempted mid-node". With a Vyukov queue there is a second way: a
worker preempted **between claiming a cell and publishing it** hides every node behind
that cell from every consumer, the callback included, which then spins on an "empty" queue
that is not empty. The window is a few instructions, so it is rare — but rare is exactly
what a p99.9 is made of, and it is the case the ADR's blame (§2.9) would misreport as
"waiting on a running node".

The fix the literature offers fits Cantus unusually well: **a Chase–Lev deque per worker
with a fixed capacity equal to the plan's task count**. Each task is pushed at most once per
block, so the buffer can never overflow, never grows, never shrinks — the two properties
that disqualified crossbeam-deque (allocation, epoch reclamation) disappear because the
buffer is never replaced. Owner `push`/`pop` are wait-free; a thief that is preempted
mid-steal blocks nobody (its CAS simply fails later); a published task is always stealable.
The orderings are Lê et al.'s, and the structure is ~150 lines of atomics on `u32`
indices — safe Rust (one `AtomicU32` per slot), or a small `cantus-rt` type with loom
tests. The continuation slot of §2.3 *is* the owner's LIFO end of such a deque, and the
critical-path priority becomes "a worker pushes its critical successors last, so it pops
them first; thieves steal oldest-first". See `scheduling-papers.md` §5 for locality.

## 3. Apple: audio workgroups

WWDC 2020 session 10224, *Meet Audio Workgroups*
(https://developer.apple.com/videos/play/wwdc2020/10224/ **[fetched, transcript]**):
"Modern CPUs have performance controllers … The performance controller can also dynamically
change the clock frequency of the CPUs … we've given it a new input into the performance
controller: the new workgroup APIs." On sizing: "there is a new API which will recommend how
many threads to create: and that's **os_workgroup_max_parallel_threads**" — "create as many
threads as there are CPU cores, but make a render-time decision to use only the recommended
number of threads". Join once, leave before the thread ends. The reference
(https://developer.apple.com/documentation/os/os_workgroup_max_parallel_threads **[fetched]**):
"the theoretical maximum … doesn't take into account the current system load". *Adding
parallel real-time threads to audio workgroups* (Apple docs **[fetched]**): joining "informs
the system that these threads are rendering to the same deadline"; handle `EALREADY` and
`EINVAL` (workgroup cancelled — e.g. the device changed). *Understanding Audio Workgroups*
**[fetched]**: the system "observe[s] CPU usage of real-time threads so that it can better
balance … power consumption and performance".

Practice: a thread must already hold `THREAD_TIME_CONSTRAINT_POLICY` before
`os_workgroup_join`, or the join fails with `EINVAL` (JUCE forum, forum.juce.com/t/…/54240
**[fetched, community]**); Mozilla's `rt_mach.rs` uses period = constraint = buffer duration
and computation = half of it, capped at 50 ms "per macOS limits" **[fetched]**; Mozilla's
crate has **no workgroup code** (the ADR already noted this).

**For the ADR.** §2.6 (macOS) is right. Two amendments: on macOS, **W comes from
`os_workgroup_max_parallel_threads(device_workgroup) − 1`** (the callback is one), not from
"physical P-cores − 2" — Apple's own number is the one its performance controller plans for;
and the pool creates one thread per core at start and *uses* W at render time, so a device
change that returns a different recommendation needs no thread creation. Re-join on
`EINVAL` after a device change (the control phase can see the new workgroup).

## 4. Windows: MMCSS, the Real-Time Work Queue, QoS and power throttling

- **MMCSS** (https://learn.microsoft.com/windows/win32/procthread/multimedia-class-scheduler-service
  **[fetched]**): `SystemResponsiveness` reserves a share (default 20 %) for low-priority
  work; the High category (priorities 23–26) is "designed for Pro Audio tasks"; threads that
  have used their quota drop to 1–7 ("they can continue to run if no low-priority threads
  are ready") — **the throttling a spinning pool can trigger**. `AvSetMmThreadPriority`
  takes LOW, NORMAL, HIGH, CRITICAL **[fetched]**.
- **Low Latency Audio** (https://learn.microsoft.com/windows-hardware/drivers/audio/low-latency-audio,
  2024-12 **[fetched]**): in the small-buffer mode Windows protects the audio engine thread,
  the driver's registered threads and interrupts, and, "if the application uses WASAPI, then
  **only** the work items that were submitted to the Real-Time Work Queue API … and were
  tagged as 'Audio' or 'ProAudio'". "Microsoft recommends for applications that use WASAPI
  to also use the Real-Time Work Queue API … **instead of their own threads**"
  (`RtwqLockSharedWorkQueue(L"Audio", …)`). The page says nothing about ASIO. It also
  documents `IAudioClient3` for small shared-mode periods.
- **Quality of Service** (https://learn.microsoft.com/windows/win32/procthread/quality-of-service
  **[fetched]**): levels High, Medium, Low, Utility, Eco, **Media** and **Deadline**. Deadline:
  "threads explicitly tagged by MMCSS to denote that audio threads require performance to
  meet deadlines … High performance" (since Windows 10 2004). "Processes which are
  determined to be playing audio are HighQoS"; a minimised or fully occluded window's
  process gets Low QoS.
- **Power throttling / EcoQoS** (`SetProcessInformation`, `PROCESS_POWER_THROTTLING_STATE`
  **[fetched]**): with `EXECUTION_SPEED` on, the process is "classified as EcoQoS … reducing
  CPU frequency or using more power efficient cores"; left unset, "the system will use its
  own heuristics". `ControlMask = EXECUTION_SPEED, StateMask = 0` forces HighQoS.
  `IGNORE_TIMER_RESOLUTION`: Windows 11 may ignore a fully occluded, minimised,
  non-audible process's timer-resolution request.
- **Steinberg's knowledge base**, *Dropouts on Intel hybrid-architecture CPUs with
  Cubase/Nuendo running in background* (steinberg.help **[fetched]**): with the window in the
  background, "efficiency cores … handle prefetch threads … the dropouts may occur"; the fix
  is `powercfg /powerthrottling disable /path "…Cubase15.exe"`. **This is ADR 0031's exact
  design — ahead workers on E-cores — failing in the market's reference implementation.**
- **CPU sets** (https://learn.microsoft.com/windows/win32/procthread/cpu-sets **[fetched]**):
  "soft" affinity compatible with power management; a restrictive affinity mask wins over
  CPU sets; sets allocated exclusively to another process are ignored;
  `EfficiencyClass` higher = faster; `CoreIndex` identifies SMT siblings.
- **Intel Thread Director** (Intel support 000091284, 2022 **[fetched]**): hardware feedback
  "optimized for Windows 11 Task Scheduler … also works with Windows 10 … but it is not
  optimized for it". Spin-wait (`PAUSE`-heavy) code is its own class, judged suitable for
  E-cores (IntervalZero's hybrid notes **[fetched]**: class 2; AnandTech's Architecture Day
  2021 summary: "spin loop wait … from the P-core to the E-core" **[excerpt]**).
- `WaitOnAddress`/`WakeByAddressSingle`: same process only; spurious wake-ups allowed
  **[fetched]** — the ADR already knows both.

**For the ADR.**
1. **Promote the callback ourselves** (§0): on its first block after a stream start, the
   control phase calls `cantus_rt::promote_callback()` — `AvSetMmThreadCharacteristicsW("Pro
   Audio")` + `AvSetMmThreadPriority(HIGH)` on WASAPI, nothing on ASIO (the driver owns its
   thread). One system call per stream start, inside ADR 0005's amended rule 3 like the
   idle → busy wake, and counted.
2. **Opt the whole process out of EcoQoS** at start (`ControlMask = EXECUTION_SPEED,
   StateMask = 0`, and keep the timer resolution) — what Steinberg tells its users to do by
   hand with `powercfg`. Cantus should not need a knowledge-base article.
3. **Ahead workers are not "E-cores first".** They go wherever live workers are not, with
   P-cores preferred while the transport plays; E-cores take background freeze and export.
   The ahead path's lead is a deadline too, just a longer one.
4. **Spinning live workers stay on P-core CPU sets** — that is also what keeps Thread
   Director's "spin-wait suits an E-core" verdict from moving them.
5. **RTWQ is an open measurement, not a rejection.** Microsoft protects only RTWQ work items
   for WASAPI applications in the small-buffer mode. RTWQ is a pool of callbacks, so a
   spinning worker cannot *be* a work item forever — but a work item can run a block's
   share of the graph. Phase 1 should measure MMCSS threads against RTWQ items under the
   WASAPI low-latency mode before choosing; under ASIO, MMCSS "Pro Audio" HIGH as written.
6. **Say which driver path the targets assume**: ASIO, or `IAudioClient3` (cpal does not do
   it today; the audio-setup builder would).

## 5. Linux

- PREEMPT_RT is **mainline since 6.12** (L. Torvalds, "Linux 6.12-rc1", LWN 992184,
  2024-09-29: "PREEMPT_RT is now mainlined and enabled as a config option" **[fetched]**).
- rtkit (README, sources.debian.org/src/rtkit/0.14-1/README **[fetched]**): only clients with
  `RLIMIT_RTTIME` set get RT; requires `SCHED_RESET_ON_FORK`; a canary watchdog "demotes all
  real-time threads to SCHED_OTHER should the system overload"; try `sched_setscheduler()`
  first, D-Bus on `EPERM`. It grants **`SCHED_RR`** **[excerpt]**, not FIFO — the ADR's
  "SCHED_FIFO via rtkit" should read "SCHED_RR via rtkit, FIFO directly".
- `sched_rt_runtime_us` = 950 000 per second by default (docs.kernel.org, sched-rt-group
  **[fetched]**); 6.12's fair "dl_server" reserves 50 ms/s for normal tasks **[excerpt]** —
  a pool that spins on every core for more than 95 % of a second gets throttled, one more
  reason for §2.1's free core.
- `SCHED_DEADLINE` cannot fork, needs root-domain-wide affinity and admission control, and
  rtkit does not grant it (docs.kernel.org sched-deadline **[fetched]**): not for a desktop
  DAW.
- **Frequency**: RT tasks default to `uclamp_min = 1024`, "boosted to run at the maximum
  performance point" (docs.kernel.org sched-util-clamp **[fetched]**). On Linux with
  schedutil, spinning to hold the clock up is unnecessary for FIFO/RR threads.

## 6. Frequency scaling, C-states and hybrid cores

Speed Shift (HWP) ramp to top frequency: legacy OS-driven P-states "more than 50 ms",
Skylake ~12–17 ms, Kaby Lake ~5.3 ms (R. Shrout, *PC Perspective*, 2016-11-22 **[fetched]**)
— longer than a 1.3–2.7 ms audio period. Lynx Studio's KB: turn off EIST and C1E/C3/C6,
because waking "takes quite a bit of time" **[fetched]**; Native Instruments: minimum and
maximum processor state 100 % **[excerpt]**. No primary source states that spinning holds
the frequency up; it follows from the ramp times and Apple's statement that the
performance controller needs an input (§3), and **it should be measured** (the bench's paced
mode, `verification.md` §3).

**For the ADR.** The case for spinning is stronger than the prototype's 2.1 µs vs 55 µs:
a parked worker also wakes on a core that may have clocked down for milliseconds. Keep
§2.4; on Linux RT threads are boosted anyway, so the *Eco* setting (park between blocks)
costs less there.

## 7. Caches, false sharing and `PAUSE`

crossbeam-utils `cache_padded.rs` (**[fetched]**): 128 bytes on x86-64, aarch64 and
powerpc64 — "Starting from Intel's Sandy Bridge, spatial prefetcher is now pulling pairs of
64-byte cache lines at a time, so we have to align to 128 bytes rather than 64", citing
Intel's optimisation manual (the spatial prefetcher completes each line "to a 128-byte
aligned chunk" **[excerpt]**). Apple M1 reports `hw.cachelinesize: 128` **[excerpt]**.
`PAUSE`: "about 10 cycles" before Skylake, "as many as 140 cycles" on it (Intel manual,
quoted by N. Piggin on LKML, 2017-04-07 **[fetched]**).

**For the ADR.** §5.4 pads hot scalars to 128 bytes on Apple silicon and implies 64
elsewhere; **use 128 on x86-64 too**. Node buffers written concurrently by two workers
should not share a 128-byte pair either: round each buffer up to 128 bytes.

## 8. Page faults and locked memory

mlock(2) (man7.org **[fetched]**): "paging is one major cause of unexpected program
execution delays"; reserve locked stack pages before the time-critical section.
`VirtualLock` (**[fetched]**): the lockable amount is "intentionally small" — raise the
minimum working set with `SetProcessWorkingSetSize` first; locked pages are never written
to the page file. Large pages need `SeLockMemoryPrivilege` and are hard to obtain once the
system has run a while (**[fetched]**) — impractical for a desktop application.

**For the ADR.** §1.4's slab and §3.3's rings (up to 36 MiB at High) are allocated on the UI
thread — but the OS maps freshly allocated pages **on first touch**, so the first block that
writes each page takes a soft page fault (and the kernel zeroes the page) *on the audio
thread*. Amend §5.4: every slab, ring and page pool is **pre-faulted** (written once) on the
thread that allocates it, and locked (`VirtualLock` after raising the working-set minimum;
`mlock` on Linux and macOS) when the OS allows, best-effort and reported. The same for each
worker's stack (touch a few pages at start) and ADR 0018's delay-line pool.

## 9. Denormals, and what Rust says about changing MXCSR

Intel oneMKL, *Operating on denormals* (**[fetched]**): denormal operations go through a
microcode assist; FTZ and DAZ are MXCSR bits. Arm FPCR (SysReg XML 2026-03 **[fetched]**):
`FZ` flushes denormal inputs **and** outputs (with `AH = 0`); with FEAT_AFP and `AH = 1` it
flushes outputs only and `FIZ` flushes inputs; **there is no separate DAZ bit**. JUCE's
`ScopedNoDenormals` saves and restores MXCSR because a plug-in that does not "can collide
with the denormal settings of the DAW and/or other plugins" (JUCE forum **[excerpt]**).
Rust's RFC 3514 (https://rust-lang.github.io/rfcs/3514-float-semantics.html **[fetched]**):
code that changes the floating-point control bits and does not restore them before its asm
block ends is **undefined behaviour**; `_mm_setcsr` is deprecated since Rust 1.75 for that
reason.

**For the ADR.** §5.2 is the industry's practice and should stay, but it should say what it
rests on: running Rust code under FTZ is outside the language's model, so the guarantee is
empirical — Cantus's DSP flushes in software and never needs a subnormal, so the flags
change no sample, which the null corpus checks with the guard on and off on every target
(x86-64 FTZ+DAZ, aarch64 FZ). Re-assert after each **hosted** plug-in call, not only before
each node, so a plug-in that leaves MXCSR changed cannot affect the next native unit inside
the same node.

## 10. The Rust crates the ADR names

| Crate | What it really does (source **[fetched]**) | ADR consequence |
|---|---|---|
| `audio_thread_priority` (Mozilla) | Windows: MMCSS "Audio", no `AvSetMmThreadPriority`; macOS: time constraint only, no workgroups; Linux: rtkit over D-Bus or `pthread_setschedparam`, sets `RLIMIT_RTTIME` to rtkit's maximum "to prevent getting SIGKILL" | Use it for **Linux only**, as the ADR says; Windows and macOS are `cantus-rt`'s |
| `atomic_refcell` | RefCell semantics across threads; **panics** on a conflicting borrow | §2.7 must use `try_borrow`/`try_borrow_mut` only — the ADR does; add a clippy ban on `borrow()`/`borrow_mut()` in `graph/` and `sched/` |
| `rtrb` | wait-free SPSC ring | already ADR 0005's channel |
| `basedrop` | deferred drop on another thread | an alternative to the garbage rings; not needed |
| `assert_no_alloc` | allocation only; **does nothing in release** with its default feature | `rt_memory.rs` is the stronger gate; keep it |
| `crossbeam-deque` / `-epoch` | grows/shrinks; epoch GC frees on any pinning thread | rejected, correctly (§2) |
