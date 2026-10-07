# Round 21 — requests to the owners of files the rt builder does not own

*From the **rt** builder (`r21/rt`). Each request names the file, the exact call, and why.
`cantus-rt` is a workspace dependency (`cantus-rt.workspace = true`; enable `features =
["rtkit"]` in `cantus-engine` so Linux gets rtkit — the plug-ins leave it off so they never
link D-Bus).*

## To **graph** — `crates/cantus-engine/src/processor.rs` (R1, §5.2, R9)

### 1. Promote the callback on the first block after a stream start (R1)

A field and a setter on `Processor`, and two lines in the control phase:

```rust
// Processor fields
/// The driver API to promote the callback for, set by the stream that owns this
/// processor; taken on the first block (one system call per stream start).
promote: Option<cantus_rt::AudioHost>,

/// Asks the first block to promote the calling (callback) thread. Called by the device
/// code when it builds the output stream; never by the offline renderer, the stems, the
/// bench or the tests, which must not touch thread priority.
pub fn promote_on_first_block(&mut self, host: cantus_rt::AudioHost) {
    self.promote = Some(host);
}

// In `process`, first thing in the control phase (before `drain_messages`):
if let Some(host) = self.promote.take() {
    let period = std::time::Duration::from_secs_f64(frames as f64 / self.sample_rate);
    let p = cantus_rt::promote_callback(host, period);
    self.stats.promotion = Some(p);   // new `EngineStats` field, `Option<cantus_rt::Promotion>`
    self.stats.wakes += 1;            // new `EngineStats` counter, shared with Phase 1's idle → busy
                                      // wake: the system calls ADR 0005 rule 3 (as amended) permits
}
```

`promote_callback` allocates nothing (`rt_memory.rs` holds): on WASAPI it is
`AvSetMmThreadCharacteristicsW("Pro Audio")` + `AvSetMmThreadPriority(HIGH)`; on ASIO,
CoreAudio and JACK it returns `Promotion::LeftAsIs` with no system call; a second call on
the same thread returns the first answer from a thread-local. `Promotion` is `Copy` and
implements `Display` for the setup chip / readout ("MMCSS \"Pro Audio\" at High …" or
"refused: AvSetMmThreadCharacteristicsW failed: … (code …)").

### 2. The device code calls the setter — `devices.rs` (owned by **asio** this round)

Where the `OutputDriver` is built for a stream (`devices.rs` / `EngineHandle::start`):

```rust
processor.promote_on_first_block(cantus_rt::AudioHost::from_driver_name(host_id.name()));
```

`from_driver_name` maps cpal's `HostId::name()` ("ASIO", "WASAPI", "CoreAudio", "JACK",
"ALSA"; anything else → `Other`, promoted like WASAPI). If the integrator prefers it, the
input stream's callback (recording, ADR 0025) can call
`cantus_rt::promote_callback(host, period)` on its first callback too: it is the same
normal-priority cpal thread on WASAPI, and an input overrun is a lost take.

### 3. The denormal guard inside `process` (§5.2) — the first line of `Processor::process`

```rust
let _ftz = cantus_rt::DenormalGuard::new();
```

Inside `process` rather than in the callback, so the callback, the offline render, the stems,
the bench and every test run in the same floating-point mode. `cantus-host` already calls
`cantus_rt::fpenv::reassert()` after every audio-thread call into a hosted plug-in (done on
`r21/rt`: `unit.rs`, `inst_unit.rs`); it is a no-op until this guard exists, and from then
on puts FTZ/DAZ back if a plug-in cleared them. Pool threads (Phase 1) hold a guard for
their whole life and `reassert()` before each node.

The plan's `tests/ftz.rs` (graph's) should also render the null corpus with the guard off
and compare (plan, "Additions per phase", Phase 0 — rt): the guard's claim that it changes
no sample is empirical (RFC 3514), and that test is what makes it a fact.
`cantus_rt::denormal::{flushing, control_register}` give the test what it asserts on.

### 4. Pre-fault what arrives on the UI thread (R11)

When the plan's slab (and ADR 0018's delay-line pool) is allocated on the UI thread, before
it is sent: `cantus_rt::memory::prefault(&mut slab)` and, best-effort,
`cantus_rt::memory::lock(&slab)` (keep the `LockedRegion` beside the slab and drop it when
the slab comes back as garbage). Measured on this machine: first touch of 16 MiB costs
**4.2 ms** — 1.6 blocks of 128 frames — if it happens on the audio thread.

## To **sched** and **graph** (Phase 1's pool) — how pool threads are promoted

Round 21 measured why the sched bench's MMCSS runs had 2.5–17.6 s blocks
(`docs/round21/rt.md` §5): **MMCSS throttles a thread that is busy more than
`100 − SystemResponsiveness` % (80 % by default) of each period, and the throttle reaches every
MMCSS thread of the process — the callback included, even when the callback never waits on the
spinner.** A spinning pool thread is busy ~100 % of the time. Hence, in `cantus-rt`:

```rust
// A worker that spins between blocks (the pool's 1.5-period spin budget):
let _p = cantus_rt::thread::promote_worker(WorkerRole::Live, period, Waiting::Spins);
//  -> Windows THREAD_PRIORITY_TIME_CRITICAL (no MMCSS); Linux SCHED_FIFO only when
//     RLIMIT_RTTIME is unlimited (never rtkit: it would kill a spinner); macOS: left alone.

// A worker that spins at most `busy_share` of a period, then parks until woken:
let _p = promote_worker(WorkerRole::Live, period, Waiting::Bounded { busy_share: 0.5 });
//  -> MMCSS "Pro Audio" HIGH / time constraint (+ workgroup) / rtkit, when
//     busy_share <= realtime_share() - SHARE_MARGIN (0.65 on a default Windows).
```

- The sched bench's `mmcss::promote()` in its `on_start` hook should become
  `promote_worker(WorkerRole::Live, cfg.period, Waiting::Spins)` — the pool spins.
- A pool that wants the real-time class (macOS workgroups require it; Apple's guidance is not
  to spin at all) must bound its spin: spin ≤ ~½ period after its last task, then park. The
  measurement favours **bounded spin + park** on every count except wake-up latency
  (`rt.md` §5); the `park_after` grace of ADR 0031 §2.4 then becomes "half a period", with the
  idle → busy wake on every block — which amends ADR 0005 rule 3's "none in steady state".
  That trade is the pool's to measure in wave 2 with these two promotions.
- The bench's back-to-back `run_block()` loop keeps even the *callback* busy ~100 % of the
  time: with MMCSS on it measures the throttle, not the scheduler. Pace it (sleep to each
  period) or leave the callback unpromoted in that bench.

## To the **integrator** — `crates/cantus-app/src/main.rs` (R1, R9)

**Now** — at start-up, before the engine starts (no effect on any sample):

```rust
let eco = cantus_rt::thread::opt_out_of_power_throttling();
log::info!("real-time: {eco}");
```

It opts the process out of EcoQoS (`SetProcessInformation(ProcessPowerThrottling)`, execution
speed controlled and off, timer resolution kept) — what Steinberg tells Cubase users to do by
hand with `powercfg` after prefetch threads dropped out on E-cores in the background.

**Not yet: the CRT FMA3 switch** (`fpenv::disable_crt_fma3`, the other half of
`cantus_rt::init_process()`). Measured on this machine (`cantus-bench/examples/fma3_bits.rs`,
`docs/round21/rt.md` §3): the switch changes **92 % of the samples** of the scaled demo session
(peak 1.8e-4). Every null-test hash was recorded on the FMA3 path; calling the switch in the
app alone would make the app render other bits than the tests pin, and calling it in the
tests would move the `v5.cantus` hash — a bit change this wave reserves for *math*. So:

1. After *math* lands (std's transcendentals gone from `cantus-dsp`), re-run
   `cargo run -p cantus-bench --example fma3_bits --release`.
2. If samples still differ (engine paths *math* lists as unreachable), call
   `cantus_rt::fpenv::disable_crt_fma3()` in **one place every render path shares** —
   `Processor::new` (graph's file), so the app, the offline render, the stems, the bench and
   every test take the same path — in the same commit as *math*'s re-blessed hashes, and
   switch the app's start-up call above to `cantus_rt::init_process()`.
3. If nothing differs, the switch is not needed: R9 (1) did its job.

## To **CI** — `.github/workflows/ci.yml`

The Linux job builds `cantus-rt` with `rtkit` (through `cantus-engine`), which links
`libdbus-1`: add `libdbus-1-dev` to the `apt-get install` line (`libgtk-3-dev` very likely
pulls it in already; naming it keeps the build honest).

---

# From at-rest

# Round 21 — requests between builders

## From at-rest to graph (suspension, ADR 0031 §5.1 — Phase 2's, but the API lands now)

No change of `processor*.rs` or `graph/` is needed for this branch to merge: `at_rest`
defaults to `false` and nothing calls it yet. When the graph suspends nodes:

1. **Suspend** a node whose units all say `at_rest()`, whose gathered input and key are
   all-zero this pass (ADR §1.3 already knows: every contribution was skipped), with no note
   or controller arriving, and whose own delay lines are empty.
2. **While suspended, call `skip(pass_frames)` on every unit of the node, once per pass**
   (`EffectUnit::skip`, `InstrumentUnit::skip`). Several units count samples and the count is
   in their bits — the delay's fractional read head, the coefficient grids, the linear-phase
   convolvers' block position, LFOs that run on the sample clock. `skip` is cheap (O(1) for
   most; a linear-phase EQ walks its blocks) and never allocates.
3. **On wake, call nothing.** A suspended unit that was skipped is exactly a unit that
   processed silence, whatever parameter changes come with the wake. `reset` + nothing is
   exact too, but only while the parameters have not changed (a monitor's memory and the
   counters a reset zeroes) — prefer no reset.
4. Ask once per block at most: `at_rest` is a few comparisons for most units, a few hundred
   for the ones with short histories, never a scan of a long line.
5. `EffectUnit::rest_blocker()` (and `FxUnit::rest_blocker`) names why a unit is not at
   rest — for the profiler's blame, if useful.
6. The track slot's built-in instruments: `cantus_dsp::Firmus::at_rest()` exists (nothing to
   skip). Pulsus has none — its live hits and its hi-hat chick are seeded by a hit counter a
   new kit starts at zero (see `at-rest.md`).
7. FTZ/DAZ inside `process` (yours) is what lets some units reach rest at all: Aequalis's
   detector envelopes and a few f64 loudness integrators decay by multiplication alone and
   stop in the subnormals without it. They are exact either way; they only rest with it.

---

# From math

# Round 21 — requests between builders

## From *math* to *graph*: route the last std transcendentals in your files

ADR 0031 R9: every transcendental on a rendered path goes through `cantus_dsp::math`
(the pinned pure-Rust `libm`), the same bits on every machine. `crates/cantus-engine/clippy.toml`
now bans std's (`f32::powf`, `f64::log2`…) in the whole engine crate; these call sites are in
files *graph* owns this round, so *math* left them, and `cargo clippy` warns on them until
they move:

| File | Line (at 49f02f3) | Today | Becomes |
|---|---|---|---|
| `src/processor_rec.rs` | 319 | `10f32.powf(i.trim_db / 20.0)` | `math::powf(10f32, i.trim_db / 20.0)` |
| `src/processor_rec.rs` | 332 | `10f32.powf(db / 20.0)` | `math::powf(10f32, db / 20.0)` |
| `src/profile.rs` | 51 | `us.log2()` | `math::log2(us)` |
| `src/profile.rs` | 59, 65 | `2f64.powf(x)` | `math::powf(2f64, x)` |
| `src/profile.rs` | 371, 374 (tests) | `2f64.powf(0.25)` | `math::powf(2f64, 0.25)` |

with `use cantus_dsp::math;` at the top. `processor_rec.rs`'s two are the input trim of a
recording (rendered bits); `profile.rs`'s are the bench's histogram buckets (no audio, done
only so the lint is clean). After the math merge the backend is libm, so the trim's gain can
differ from std's by an ulp — no golden covers a recording's trim; nothing to re-bless.

Any new DSP code in `graph/`: `cantus_dsp::math::*`, never std's `sin`/`exp`/`powf`/….
