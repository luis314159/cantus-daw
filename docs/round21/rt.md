# Round 21 — rt: `cantus-rt`, the engine's real-time ground

*Builder **rt**, branch `r21/rt` (base 49f02f3). ADR 0031 §2.6, §2.7, §5.2, §5.4 with the
review's amendments R1, R3, R4, R5, R9, R11; plan Phase 0 "rt" and its additions.*

## What was built

A new crate, `crates/cantus-rt` (MIT OR Apache-2.0, like the engine), the only home of the
engine's `unsafe` and OS code. `cantus-engine` stays `#![forbid(unsafe_code)]`.

| Module | What it does |
|---|---|
| `thread.rs` (+ `thread/{windows,macos,linux,other}.rs`) | `promote_callback(host, period)`: on **WASAPI** (and ALSA/other cpal-owned threads) MMCSS **"Pro Audio"** + `AvSetMmThreadPriority(HIGH)`; on **ASIO, CoreAudio, JACK** nothing (`Promotion::LeftAsIs` — the driver owns the thread). Cached per thread in a const thread-local: one system call per stream start, the same answer after. `promote_worker(role, period, waiting)` → a `!Send` guard reverted on drop, **safe for how the thread waits** (§5 below): a `Waiting::Spins` worker never enters a throttled real-time class (Windows TIME_CRITICAL / HIGHEST; Linux direct FIFO only with unlimited `RLIMIT_RTTIME`; macOS nothing), a `Waiting::Bounded` one under `realtime_share() − 0.15` gets Live "Pro Audio" HIGH, Ahead "Pro Audio" LOW, Background "Audio" (§2.6, R3); macOS `THREAD_TIME_CONSTRAINT_POLICY` (period = device period, computation ½, as Mozilla's `rt_mach.rs`), and `promote_worker_in(&Workgroup, …)` which sets the policy **before** `os_workgroup_join` (R4) and reports `EINVAL` (cancelled workgroup) as a refusal to re-fetch; `Workgroup::max_parallel_threads()`; Linux `SCHED_FIFO \| SCHED_RESET_ON_FORK` directly when `RLIMIT_RTPRIO` (or root) allows, else rtkit's SCHED_RR through `audio_thread_priority` (feature `rtkit`, default on, off for the plug-ins). `set_current_thread_cpus` (Windows CPU sets — soft, power-manager friendly; Linux affinity). `opt_out_of_power_throttling()` (EcoQoS off, timer resolution kept, R1). `AudioHost::from_driver_name` maps cpal's `HostId::name()`. Every result is a `Promotion`/`Refusal` with a `Display` that names the OS call and its error. |
| `topology.rs` (+ `topology/{windows,linux,macos,parse}.rs`) | Physical cores, P/E class, last-level-cache domains. Windows: `GetSystemCpuSetInformation` (ids for `SetThreadSelectedCpuSets`, `EfficiencyClass`, `LastLevelCacheIndex`, parked / reserved flags) with the cache's level and size from `GetLogicalProcessorInformationEx(RelationCache)`, and a second, independent topology from `GetLogicalProcessorInformationEx` alone for cross-checking. Linux: sysfs (`core_cpus_list`, `cpu_capacity`, `/sys/devices/cpu_atom/cpus`, highest `cache/index*`). macOS: `hw.perflevel*`. Fallback: `available_parallelism`. `default_live_workers()` = live-domain P-cores − 2, clamped 1..=14 (§2.1); `placement(w)` = live workers on P-cores of one cache domain skipping the first, one logical processor per core (R4, R5); ahead = spare P-cores then E-cores (R3); background = E-cores first. The records are parsed as bytes at `offset_of!` offsets: the only `unsafe` is the two calls. |
| `pad.rs` | `CachePadded<T>` at **128** bytes on every target (R11: Intel's spatial prefetcher pairs lines), `round_up`, `padded_len`. |
| `memory.rs` | `prefault(&mut [T])` (one volatile read-write per page, contents unchanged), `prefault_stack()` (64 KiB, top-down), `lock(&[T])` → `Lock::{Locked(LockedRegion), Refused(Refusal), NotApplicable}` — `VirtualLock` after raising the working-set minimum (soft limits), `mlock` on Unix; unlocked on drop. `page_size()`. |
| `fpenv.rs` | `disable_crt_fma3()` / `crt_fma3_enabled()` — the universal CRT's `_set_FMA3_enable(0)` on Windows x64 (R9 (2), interim until *math*); re-exports the denormal guard and `reassert()`. |
| `denormal.rs` | The guard moved from `cantus-plugins/src/ftz.rs` (now a re-export): FTZ+DAZ on x86-64, FZ on AArch64, saved and restored; `!Send`; `reassert()` on the guard, and a free `fpenv::reassert()` that acts only while a guard is alive on the thread (a const thread-local depth), so a hosted-plug-in wrapper can call it unconditionally; `flushing()`, `control_register()`. Inert under Miri. |
| `lib.rs` | `init_process()` — once, at start-up: the CRT FMA3 switch and the EcoQoS opt-out. |
| `examples/rt_probe.rs` | Prints the topology (both Windows sources), the placement, `init_process`, each promotion with the thread's priority before and after, the memory test and the FP control register. |

Outside the crate (small, listed below): `cantus-plugins/src/ftz.rs` re-exports the guard;
`cantus-host`'s hosted effect and instrument call `cantus_rt::fpenv::reassert()` after every
`process` and `reset` (R9: "re-asserted after every hosted plug-in call"); two measurement
examples in `cantus-bench`.

## Measurements

All on the round's machine (i7-12700H, Windows 11 23H2), **while the other wave-1 builders
compiled** (8–17 `rustc`/`cargo` processes for most of the session): absolute block times are
inflated, the ratios are the result.

### 1. The probe (`cargo run -p cantus-rt --example rt_probe --release`)

```text
topology: CpuSets, 20 logical, 14 physical cores (6 P + 8 E), hybrid
  P-cores: class 1, CPU set ids 256..267 (two threads each); E-cores: class 0, ids 268..275
  last-level cache 0x0: L3 24576 KiB, 14 cores
cross-check (GetLogicalProcessorInformationEx): 20 logical, 14 cores (6 P + 8 E), L3 24576 KiB — identical
default live workers: 4
placement: live [258, 260, 262, 264]   ahead [256, 266, 268..275]   background [268..275, 256..266]
CRT FMA3 path before: Some(true); after: Some(false)
init_process: CRT FMA3 path disabled; EcoQoS opted out, timer resolution kept
callback on Wasapi: MMCSS "Pro Audio" at High   [GetThreadPriority 0 -> 17]
callback on Asio:   driver-owned thread: real-time already, left alone   [0 -> 0]
worker Live: "Pro Audio" High [0 -> 17]; Ahead: "Pro Audio" Low [0 -> 15]; Background: "Audio" Normal [0 -> 13]; each reverted to 0 on drop
16 MiB: allocate 0.02 ms, first touch 4.22 ms (4097 pages), touched again 0.07 ms; lock: locked 16384 KiB
MXCSR 0x1fa0 -> 0x9fe0 in a guard (flushing: true); after the guard 0x1fa0
```

The first touch of a fresh 16 MiB slab costs **4.2 ms — 1.6 blocks at 128 frames** — which is
what R11's pre-fault on the allocating thread removes from the audio thread.

### 2. What promoting the callback buys (R1) — `cantus-bench/examples/promote_jitter.rs`

The bench's `Rig` playing **paced**: one `Processor::process` per 128-frame period (2 667 µs),
the thread sleeping to each period's start (std's high-resolution waitable timer), as a
WASAPI callback thread runs; *normal* = cpal's thread today, *promoted* =
`promote_callback(Wasapi)` on the first block (exactly the call requested of graph). *Loaded*
= 20 normal-priority busy threads (every logical processor) on top of the builders. *Late* =
how long after the period's start the thread ran; a **miss** = the block ended after the next
period began (a dropout on a device). 2 rounds × 10 s per configuration, 7 500 blocks each.

DAWbench DSP session (`--session dsp --tracks 8`: 8 tracks × 2 working compressors):

| config | late p50 | late p99 | late p99.9 | late max | block p50 | block p99 | block max | misses |
|---|---|---|---|---|---|---|---|---|
| normal / idle | 529 µs | 15 596 µs | 30 125 µs | 39 732 µs | 1 653 µs | 5 518 µs | 31 513 µs | 2 821 (38 %) |
| **promoted / idle** | 383 µs | **1 414 µs** | 2 190 µs | **2 842 µs** | 1 517 µs | 3 215 µs | 4 101 µs | **707 (9 %)** |
| normal / loaded | 16 569 µs | 54 731 µs | 76 688 µs | 103 263 µs | 1 372 µs | 19 638 µs | 81 466 µs | 7 250 (97 %) |
| **promoted / loaded** | 347 µs | **2 128 µs** | 2 504 µs | **3 266 µs** | 1 170 µs | 3 541 µs | 4 190 µs | **1 220 (16 %)** |

```json
[{"config":"normal / idle","late_us":{"p50":528.9,"p99":15595.5,"p999":30124.9,"max":39731.7},"block_us":{"p50":1652.7,"p99":5517.9,"p999":13175.5,"max":31513.3},"misses":2821,"blocks":7500},
 {"config":"promoted / idle","late_us":{"p50":382.6,"p99":1413.7,"p999":2190.3,"max":2842.1},"block_us":{"p50":1517.2,"p99":3215.2,"p999":3712.4,"max":4101.2},"misses":707,"blocks":7500},
 {"config":"normal / loaded","late_us":{"p50":16568.6,"p99":54731.1,"p999":76687.9,"max":103262.7},"block_us":{"p50":1371.5,"p99":19638.1,"p999":44565.3,"max":81466.4},"misses":7250,"blocks":7500},
 {"config":"promoted / loaded","late_us":{"p50":347.4,"p99":2128.3,"p999":2503.7,"max":3266.4},"block_us":{"p50":1169.5,"p99":3541.1,"p999":3799.2,"max":4190.0},"misses":1220,"blocks":7500}]
```

The scaled demo session (`--tracks 6`, the house desk with its mastering chain) costs more
than a period per block on this contended machine (block p50 3.4–8.5 ms), so every block
missed in both modes; its lateness tells the same story — p99 26 455 → 1 225 µs idle,
104 627 → 963 µs loaded, **max 1 037 838 µs (a whole second) → 2 738 µs** loaded:

```json
[{"config":"normal / idle","late_us":{"p50":631.0,"p99":26455.5,"p999":50334.1,"max":82049.1},"block_us":{"p50":4793.9,"p99":16878.3,"p999":32166.9,"max":48144.5},"misses":7499,"blocks":7500},
 {"config":"promoted / idle","late_us":{"p50":422.8,"p99":1225.0,"p999":1997.5,"max":2759.4},"block_us":{"p50":6158.5,"p99":10674.9,"p999":13551.5,"max":17727.7},"misses":7500,"blocks":7500},
 {"config":"normal / loaded","late_us":{"p50":24423.0,"p99":104626.8,"p999":309313.9,"max":1037837.9},"block_us":{"p50":8461.3,"p99":64963.2,"p999":136089.2,"max":525551.6},"misses":7500,"blocks":7500},
 {"config":"promoted / loaded","late_us":{"p50":388.6,"p99":963.3,"p999":1612.0,"max":2738.3},"block_us":{"p50":3429.3,"p99":12465.0,"p999":18153.9,"max":27694.1},"misses":7484,"blocks":7500}]
```

Reading: at normal priority the callback waits behind whatever else is runnable — tens of
milliseconds at p99 even with the machine "idle" (the builders), up to a second under load —
and its **block time** balloons too (p99 5.5 → 19.6 ms), because it is preempted *inside*
`process`. Promoted, the lateness floor (~0.35–0.5 ms p50) is the sleep timer's granularity,
p99 stays under one period (1.4 ms idle, 2.1 ms under full load), and the block's own time stops being
preempted (p99 3.2–3.5 ms in every case). The remaining misses are blocks whose cost plus the
timer's lateness exceed the period on a machine shared with eight compilers — a real WASAPI
event or ASIO callback wakes without the sleep timer's half millisecond. R1's case is made:
**the one thread every block waits on is today the one most likely to be late.**

### 3. The CRT's FMA3 path changes Cantus's output (R9) — `cantus-bench/examples/fma3_bits.rs`

```text
CRT FMA3 path at start: true
scaled(10), 7500 blocks of 128: FMA3 path on vs off: 1761451 of 1920000 samples differ
(first at sample 79143), peak difference 1.769e-4
```

**92 % of the samples** of the demo session differ between the CRT's FMA3 and non-FMA3
transcendentals (peak 1.8e-4, about −75 dBFS); `tests/crt_fma3.rs` sees the same on std's
`sin/cos/exp/ln_1p/powf/tanh/atan` alone (260 000 results, the digests differ). So R9 is not
theoretical: **every null-test hash recorded on this machine is an FMA3-path hash**, and a
machine without FMA3 (or with it disabled) renders other bits today. Consequence for wiring
`init_process()` — see the notes below.

### 4. `xtask perf`

Not re-run: this branch changes no code the engine runs (`processor*.rs`, `cantus-dsp` and
the engine's units are untouched; the guard only moved crates, and `reassert()` runs only after
hosted plug-in calls and is a no-op until graph adds the guard). On this contended machine
the perf suite's numbers would measure the builders, not the change.

### 5. Which threads may be promoted, and how — MMCSS and spinning (for wave 2)

*Asked by the coordinator after the sched builder's bench saw 2.5–17.6 s blocks with every pool
thread in MMCSS "Pro Audio" HIGH (155 ms worst with MMCSS off).*

**The mechanism (documented, and measured here).** MMCSS keeps `SystemResponsiveness` % of the
CPU (registry: **20**, both tasks at a 1 ms clock rate on this machine) for normal work: a
thread in an MMCSS task that is busy more than **80 %** of the time is dropped to priority 1–7
— *below* every normal thread — for the rest of the window. A pool worker that spins between
blocks is busy ~100 %. On a machine at 100 % (here: the builders), a demoted thread waits behind
every compiler. Linux and macOS police the same thing differently and more harshly: rtkit's
`RLIMIT_RTTIME` kills a real-time thread that runs ~200 ms without blocking, and macOS demotes
a time-constraint thread that keeps overrunning its computation.

**The experiment** (`crates/cantus-rt/examples/mmcss_spin.rs`): the pool's shape without the
engine — a callback paced at 128 frames (2 667 µs) that arms each block and does 300 µs, four
workers each doing 300 µs of every block and **spinning** between blocks (or spinning a
bounded fraction of a period, then parking), 20 normal-priority busy threads beside the
builders, every thread promoted and running before the first measured block (a start barrier
and 1 s of warm-up — without them the first block waited seconds for a worker that had not yet
been scheduled to promote itself: a real lesson for the pool too). 3 rounds × 15 s per row
(5 624 blocks each); ranges across the rounds:

| callback / workers | block p99 | block max | misses / 5 624 | longest gap seen by the callback |
|---|---|---|---|---|
| callback alone, **normal** priority | 2.1–3.5 ms | 49–103 ms | 5 469–5 578 | 49–103 ms |
| callback alone, **MMCSS "Pro Audio" HIGH** (R1) | **0.30 ms** | **0.32–0.45 ms** | **0–1** | 0.12–0.26 ms |
| callback alone, TIME_CRITICAL | 0.30 ms | 0.33–0.35 ms | 0–1 | 0.06–0.15 ms |
| MMCSS callback + **spinning MMCSS workers** (HIGH or CRITICAL) | **2.33–2.37 ms** | 2.9–3.9 ms | **80–133** | 2.8–3.5 ms |
| MMCSS callback + MMCSS workers spinning 0.7 / 0.85 / 1.0 period, then parking (81–100 % busy) | 2.26–2.42 ms | 3.0–3.5 ms | 91–152 | 2.7–3.5 ms |
| MMCSS callback + MMCSS workers spinning **0.5 period, then parking** (61 % busy) | **0.46–0.60 ms** | **2.5–3.1 ms** | **0–8** | 0.15–0.87 ms |
| MMCSS callback + **TIME_CRITICAL spinning** workers | **0.31–0.37 ms** | 0.9–31 ms | 0–4 | 0.1–0.7 ms |
| MMCSS callback + TIME_CRITICAL workers, 0.5 period then park | 0.64–1.45 ms | 3.4–39 ms | 1–32 | 0.1–0.9 ms |
| MMCSS callback, MMCSS spinners it **never waits on** (detached) | 2.23–2.30 ms | 3.2–3.4 ms | 59–116 | **2.9–3.3 ms** |
| MMCSS callback, *normal-priority* spinners it never waits on | 0.31–0.32 ms | 1.0–1.7 ms | 0–2 | 0.9–1.5 ms |

What it shows:

1. **The callback alone in MMCSS is safe and is the single biggest win** (R1 stands): from
   ~5 500 misses per 15 s at normal priority to 0–1.
2. **A spinning MMCSS thread is throttled, with the 80 % threshold exactly where
   `SystemResponsiveness` puts it**: 61 % busy runs clean, 81 % shows the throttle signature
   (a ~2.35 ms p99 — the demoted part of the window) and ~2 % misses.
3. **The throttle reaches every MMCSS thread of the process — the callback included.** In the
   *detached* rows the callback never waits for the workers and does 300 µs of its own, yet
   with MMCSS spinners beside it its own longest gap is 2.9–3.3 ms and it misses 59–116 blocks;
   with the same spinners at normal priority it misses 0–2. **One spinning MMCSS thread
   undoes R1.**
4. **TIME_CRITICAL** (`SetThreadPriority`, no MMCSS) is never throttled: the best p99 for
   spinners. Its residual risk is Windows' anti-starvation boost, which lifts starved normal
   threads to priority 15 — TIME_CRITICAL's own level — for a quantum: single blocks of
   8–31 ms on this saturated machine. MMCSS's band (16–31) is above that boost, which is why
   **bounded spin + park + MMCSS** has the best worst case (≤ 3.1 ms).
5. In the paced harness nothing reached seconds; the multi-second blocks of the sched bench
   need its two extra ingredients: its loop runs blocks **back to back** (the callback, also
   in MMCSS there, is ~100 % busy and throttled too) under `start /high /affinity 555` on a
   machine at 100 %, where a thread demoted below normal can wait for the ~4 s anti-starvation
   boost. Its own bench, run here once (1 round, W ≤ 2, launched without `/high /affinity`,
   binary built into my scratch dir from `r21/sched`): MMCSS on worst block 91 ms, off 82 ms —
   **not reproduced at that size**; the exact `start /high /affinity 555`, W ≤ 4, 5-round run
   was not repeated for time. The mechanism above does not depend on it.

**The rule, now in `cantus-rt`'s API** (`thread.rs`, safe by default):

| Thread | Windows | Linux | macOS |
|---|---|---|---|
| **Callback** (`promote_callback`) — blocks every period in the driver; its busy share is the engine's load | MMCSS "Pro Audio" HIGH (WASAPI); nothing on ASIO | rtkit / SCHED_FIFO (ALSA) | nothing (CoreAudio's thread) |
| **Spinning worker** — `promote_worker(role, period, Waiting::Spins)` | Live `THREAD_PRIORITY_TIME_CRITICAL`, Ahead `HIGHEST`, Background normal — **never MMCSS** | SCHED_FIFO only directly and only while `RLIMIT_RTTIME` is unlimited; never rtkit | left at normal priority |
| **Bounded worker** — `Waiting::Bounded { busy_share }`, granted when `busy_share ≤ realtime_share() − 0.15` (0.65 here), spin then **park** | MMCSS: Live "Pro Audio" HIGH, Ahead "Pro Audio" LOW, Background "Audio" | rtkit / SCHED_FIFO | time constraint, computation = busy share × period; `promote_worker_in` joins the device workgroup after it |

`realtime_share()` reads `SystemResponsiveness` (Windows), `sched_rt_runtime_us` (Linux).
Declaring `Bounded` and then spinning is the one way left to hurt the callback; the
documentation says so where the promise is made. **Recommendation for wave 2's pool**: spin
at most about half a period after the last task, then park, and take `Waiting::Bounded`
(MMCSS): it has the best worst case measured. If the pool keeps the 1.5-period spin budget,
`Waiting::Spins` (TIME_CRITICAL) is safe and has the best p99, at the risk of rare 10–30 ms
blocks under saturation. Either way, a worker must count as available only after it has
promoted itself.

## Tests

`cargo test -p cantus-rt`: **26 tests, all pass** on this machine (after the §5 change too).

- `tests/model.rs` (13) — the pure types on synthetic machines: an i7-12700H as CPU sets
  describe it (6 P with SMT + 8 E, one L3 → 4 live workers on P-cores 1–4, ahead = the two
  spare P-cores then the E-cores, background E-first); a dual-CCD part (the live pool stays in
  one L3 domain); parked and reserved processors never placed (a core's usable thread is
  placed instead); uniform CPUs and the 1..=14 clamp; 128-byte padding and `round_up`;
  `prefault` keeps contents and touches every page; nested guards and `reassert` outside a
  guard; the sysfs parsers; `AudioHost::from_driver_name`.
  **Miri** (`cargo +nightly miri test -p cantus-rt --no-default-features --lib --test model`,
  nightly 2026-10-05): these 13 and the 2 unit tests pass.
- `tests/machine.rs` (9) — ask the OS: the topology is whole (every core in one cache
  domain, logical count = `available_parallelism`, Windows sizes its L3); **CPUID's hybrid
  bit (leaf 7, EDX 15) ⇒ the topology is hybrid**, has E-cores, E-cores are not the
  hyper-threaded ones, and no live worker lands on an E-core; **CPU sets and
  `GetLogicalProcessorInformationEx` agree** on cores, classes, threads per core, cache levels
  and sizes; the callback is promoted to MMCSS "Pro Audio" HIGH on Windows (else the refusal
  names the call and the error), once per thread; ASIO/CoreAudio/JACK are left alone; each
  worker role is promoted and reverted on drop (priority back to 0); a thread is placed on a
  live core; the process leaves EcoQoS; a 1 MiB pre-faulted slab locks.
- `tests/crt_fma3.rs` (1) — the switch turns the CRT's FMA3 path off and reports whether
  std's transcendentals differ (they do here).
- `tests/no_alloc.rs` (1) — `promote_callback` (first call and 1 000 later ones), the guard
  and both `reassert`s allocate nothing (a counting global allocator, the thread watched).
- `src/denormal.rs` (2) — `reassert` restores flags a "plug-in" cleared, only inside a guard.

Also: `cargo clippy -p cantus-rt --all-targets` clean; `cargo check -p cantus-rt
--all-targets --no-default-features` clean for **x86_64-unknown-linux-musl,
aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin** (the rtkit feature
cannot be cross-checked here — `libdbus-sys` needs the target's pkg-config — so its three
calls were checked against `audio_thread_priority` 0.38's source); `cargo test -p
cantus-plugins --test clap` (the guard through its re-export, FTZ on and off bit-identical):
**16 pass**; `cargo check -p cantus-host --all-targets`: clean. The engine's tests are untouched by this branch (no engine
file changed).

## Files outside my area

- `Cargo.toml` (root): member, default member, workspace dependency (`default-features =
  false`; the engine should enable `rtkit`).
- `crates/cantus-plugins/{Cargo.toml, src/ftz.rs}`: the move (re-export; paths unchanged).
- `crates/cantus-host/{Cargo.toml, src/unit.rs, src/inst_unit.rs}`: `reassert()` after the
  hosted plug-in's `process` and `reset` — 2 lines each, no-ops until graph adds the guard.
- `crates/cantus-bench/{Cargo.toml (dev-dependency), examples/promote_jitter.rs,
  examples/fma3_bits.rs}`.
- `Cargo.lock`: `cantus-rt`, `audio_thread_priority` 0.38 and its Linux/macOS deps (`dbus`,
  `libdbus-sys`, `mach2`) — none built on Windows.
- `docs/round21/requests.md`: for graph (`processor.rs`), asio (`devices.rs`), the integrator
  (`cantus-app/src/main.rs`) and CI.

## Left

- **Run `rt_probe` on macOS and Linux** (the plan asks for it by hand): the macOS code
  (time constraint, workgroup join/leave/max-parallel via `dlsym`, `hw.perflevel*`) and the
  Linux code (FIFO, rtkit, sysfs) compile for both targets but have not run.
- **The CoreAudio side of R4**: fetching `kAudioDevicePropertyIOThreadOSWorkgroup` from the
  device (CoreAudio FFI) and handing it to `Workgroup::from_raw` — the pool's device code,
  Phase 1.
- **`loom` models**: the plan asks for them on types with interior unsafety; nothing in this
  crate has any yet (no shared-memory types: the rings and `shm.rs` come in Phases 2/6).
- **The sched bench's exact multi-second case** (`start /high /affinity 555`, W ≤ 4, 5 rounds)
  was not re-run; one shorter run did not reproduce it (§5). A quiet-machine run of
  `mmcss_spin` and of that case would settle the magnitude; the rule does not depend on it.
- **RTWQ vs MMCSS** under WASAPI low latency (R3's measurement) — Phase 1, with a real
  `IAudioClient3` stream.
- **Crate size**: 2 563 lines with comments and blank lines (1 900 of code) against ADR
  0031 §2.7's "under 2 000". The four platforms' backends and the documentation of every
  OS call are most of it; if the integrator reads the rule as total lines, `topology/macos.rs`
  and `thread/macos.rs` (309 lines) are the candidates to move behind a `cantus-rt-apple`
  file split, or the Display impls could go.

## Notes for the integrator

1. **Merge order**: anywhere (the plan: "rt merges any time"). Merging changes no sample:
   `ftz.rs` re-exports the same guard; `cantus-host`'s `reassert()` is a no-op until graph's
   guard exists.
2. **Workspace dependency**: `cantus-rt = { path = …, default-features = false }`. The engine
   should depend with `features = ["rtkit"]` (Linux rtkit); the plug-ins keep it off.
3. **FMA3**: do **not** wire `init_process()` (which includes the CRT switch) into the app this
   wave — it would make the app render other bits than every test pins (92 % of samples
   differ here). Wire `thread::opt_out_of_power_throttling()` now; decide the FMA3 switch
   after *math* with `fma3_bits` (`requests.md`).
4. **Pool threads (wave 2)**: never MMCSS for a thread that spins — the throttle reaches the
   callback too (§5). `promote_worker` now takes `Waiting` and is safe by default; the sched
   bench's `on_start` hook should call `promote_worker(Live, period, Waiting::Spins)`, or the
   pool should adopt bounded spin + park and declare `Waiting::Bounded` (`requests.md`).
5. **Graph's R1 call** and the guard in `process`: `requests.md` has the exact code; the
   jitter table above is the reason to do it in Phase 0 rather than wait for the pool.
6. **Linux CI** needs `libdbus-1-dev` once the engine enables `rtkit` (`requests.md`).
7. Measurements were taken with the other builders compiling; re-run `promote_jitter`
   (`--session dsp --tracks 8`) and `rt_probe` on a quiet machine for the published numbers.
