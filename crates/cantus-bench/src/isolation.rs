//! What running a plug-in out of Cantus's process costs (round 29, ADR 0063): the group
//! `isolation`, on purpose only (`--only isolation`), in release, with the helper built
//! beside the bench (`cargo build --release -p cantus-sandbox -p cantus-bench`, or
//! `CANTUS_SANDBOX_EXE` pointing at one).
//!
//! - **Round trip** (`isolation.rt.<mode>.b<frames>[.paced]`): one call of the test gain
//!   (`cantus-test-clap`, which copies its input at 0 dB — almost no work of its own), µs
//!   per call, median and 99th percentile; back to back (the helper's thread still spinning
//!   from the call before, as for an instrument's next note segment) and *paced* (one
//!   call per device period, the helper parked between them, as a live callback sees it).
//! - **CPU** (`isolation.cpu.<mode>.b<frames>`): eight instances paced at real time for a
//!   few seconds; the share of a core Cantus's thread and the helper processes used,
//!   per instance, minus the same eight within Cantus — what isolation costs a session.
//! - **The DAWbench row** (`isolation.dsp32.<mode>.b128`): 32 tracks, each through one
//!   Comprimo exported as a CLAP plug-in (ADR 0020), hosted within, *Together* and
//!   *Individually*, against the native insert: µs per block of the whole session, back to
//!   back (ADR 0031 §8, phase 6: *Together* ≤ 1.2× within).

use crate::report::{Better, Metric};
use crate::stats::{median, quantile, Summary};
use crate::Plan;
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_host::bridge::Sandboxes;
use cantus_host::{Audio, AudioSetup, Backend, Format, Loaded, ParamOut, PluginHost, PluginInfo};
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SR: f64 = 48_000.0;
const MODES: [(&str, PluginIsolation); 3] = [
    ("within", PluginIsolation::Within),
    ("together", PluginIsolation::Together),
    ("individually", PluginIsolation::Individually),
];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root")
        .to_path_buf()
}

fn lib_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

/// A plug-in built in release into `target/bench-plugins`, copied to a `.clap` name.
fn build(package: &str, stem: &str, features: &[&str], name: &str) -> Option<PathBuf> {
    let target = workspace().join("target").join("bench-plugins");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Process::new(cargo);
    cmd.args(["build", "--release", "-p", package, "--target-dir"])
        .arg(&target)
        .args(features)
        .current_dir(workspace());
    let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        eprintln!("bench: isolation: {package} did not build");
        return None;
    }
    let built = target.join("release").join(lib_name(stem));
    let dir = target.join("clap");
    std::fs::create_dir_all(&dir).ok()?;
    let to = dir.join(name);
    std::fs::copy(&built, &to).ok()?;
    Some(to)
}

fn scan(path: &Path) -> Vec<PluginInfo> {
    cantus_host::backend_for(Format::Clap)
        .and_then(|b| b.scan(path).ok())
        .unwrap_or_default()
}

fn pool() -> Option<Arc<Sandboxes>> {
    let pool = Sandboxes::new(Sandboxes::default_exe());
    if !pool.available() {
        eprintln!(
            "bench: isolation: no helper at {} (build cantus-sandbox in release, or set {})",
            pool.exe.display(),
            cantus_host::bridge::sandbox::EXE_ENV
        );
        return None;
    }
    Some(pool)
}

fn load(pool: &Arc<Sandboxes>, info: &PluginInfo, mode: PluginIsolation, frames: usize) -> Loaded {
    let setup = AudioSetup {
        sample_rate: SR,
        max_block: frames.max(1024),
    };
    let group = cantus_host::bridge::group_key(mode, info);
    match group {
        None => cantus_host::backend_for(info.format)
            .expect("CLAP")
            .load(info, setup)
            .expect("loads"),
        Some(group) => cantus_host::bridge::BridgedBackend {
            format: info.format,
            group,
            pool: pool.clone(),
            offline: false,
        }
        .load(info, setup)
        .expect("loads in the helper"),
    }
}

fn call(
    audio: &mut dyn cantus_host::AudioThread,
    l: &mut [f32],
    r: &mut [f32],
    out: &mut ParamOut,
) {
    out.clear();
    audio.process(
        Audio {
            left: l,
            right: r,
            key: None,
            transport: None,
        },
        &[],
        &[],
        out,
    );
}

/// Sleeps (then spins the last stretch) until `at`: one device period between calls.
fn until(at: Instant) {
    let now = Instant::now();
    if at > now + Duration::from_micros(300) {
        std::thread::sleep(at - now - Duration::from_micros(250));
    }
    while Instant::now() < at {
        std::hint::spin_loop();
    }
}

/// Round trips of one instance: (back-to-back µs, paced µs).
fn round_trips(
    audio: &mut dyn cantus_host::AudioThread,
    frames: usize,
    calls: usize,
) -> (Vec<f64>, Vec<f64>) {
    let (mut l, mut r) = (vec![0.25f32; frames], vec![-0.25f32; frames]);
    let mut out = ParamOut::new();
    for _ in 0..200 {
        call(audio, &mut l, &mut r, &mut out);
    }
    let mut hot = Vec::with_capacity(calls);
    for _ in 0..calls {
        let t = Instant::now();
        call(audio, &mut l, &mut r, &mut out);
        hot.push(t.elapsed().as_secs_f64() * 1e6);
    }
    let period = Duration::from_secs_f64(frames as f64 / SR);
    let mut paced = Vec::with_capacity(calls);
    let mut next = Instant::now() + period;
    for _ in 0..calls {
        until(next);
        next += period;
        let t = Instant::now();
        call(audio, &mut l, &mut r, &mut out);
        paced.push(t.elapsed().as_secs_f64() * 1e6);
    }
    (hot, paced)
}

/// CPU seconds a process has used, when the OS says.
fn process_seconds(pid: u32) -> Option<f64> {
    os::process_seconds(pid)
}

#[cfg(windows)]
mod os {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn CloseHandle(h: isize) -> i32;
        fn GetProcessTimes(
            p: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    pub fn process_seconds(pid: u32) -> Option<f64> {
        // SAFETY: a query-only handle to a process by id, closed below; the four
        // out-pointers are live FILETIMEs on this frame.
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h == 0 {
                return None;
            }
            let (mut c, mut e, mut k, mut u) = Default::default();
            let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
            CloseHandle(h);
            let t = |f: &FileTime| ((f.high as u64) << 32 | f.low as u64) as f64 * 100e-9;
            (ok != 0).then(|| t(&k) + t(&u))
        }
    }
}

#[cfg(not(windows))]
mod os {
    pub fn process_seconds(pid: u32) -> Option<f64> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let rest = &stat[stat.rfind(')')? + 2..];
        let f: Vec<&str> = rest.split_whitespace().collect();
        let ticks: f64 = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
        Some(ticks / 100.0)
    }
}

/// Eight instances of `info` run `mode`, paced at real time for `seconds`: the share of a
/// core per instance (Cantus's thread plus the helpers).
fn cpu_share(
    pool: &Arc<Sandboxes>,
    info: &PluginInfo,
    mode: PluginIsolation,
    frames: usize,
    seconds: f64,
) -> f64 {
    let mut halves: Vec<Loaded> = (0..8).map(|_| load(pool, info, mode, frames)).collect();
    let pids: Vec<u32> = {
        let mut p: Vec<u32> = halves.iter().filter_map(|h| h.main.isolated_in()).collect();
        p.sort();
        p.dedup();
        p
    };
    let (mut l, mut r) = (vec![0.25f32; frames], vec![-0.25f32; frames]);
    let mut out = ParamOut::new();
    let period = Duration::from_secs_f64(frames as f64 / SR);
    let blocks = (seconds / period.as_secs_f64()) as usize;
    let kids0: f64 = pids.iter().filter_map(|p| process_seconds(*p)).sum();
    let start = Instant::now();
    let mut next = start + period;
    for _ in 0..blocks {
        until(next);
        next += period;
        for h in halves.iter_mut() {
            call(h.audio.as_mut(), &mut l, &mut r, &mut out);
        }
    }
    let wall = start.elapsed().as_secs_f64();
    // The bench's own thread spins between blocks (`until`), so its CPU says nothing; what
    // the calls cost it is the round trip, measured apart. The helpers' CPU is what
    // isolation adds: the plug-ins' work, their waits, their wakes.
    let kids: f64 = pids.iter().filter_map(|p| process_seconds(*p)).sum::<f64>() - kids0;
    drop(halves);
    kids / wall / 8.0
}

/// The DAWbench row: 32 tracks, each through one Comprimo — native, or hosted `mode`.
fn dsp32(
    pool: &Arc<Sandboxes>,
    comprimo: Option<&PluginInfo>,
    mode: Option<PluginIsolation>,
    frames: usize,
    seconds: f64,
) -> f64 {
    let mut p = crate::sessions::dsp(32, 0);
    let strips: Vec<StripId> = p.tracks.iter().filter_map(|t| t.strip).collect();
    let mut host = PluginHost::new(None, SR);
    host.set_sandboxes(pool.clone());
    let mut bus = CommandBus::new(p.clone());
    for s in &strips {
        let slot = match (comprimo, mode) {
            (Some(info), Some(m)) => host
                .make_slot_isolated(&mut bus, *s, info, Some(m))
                .expect("Comprimo loads"),
            _ => {
                let mut c = p.make_effect(EffectKind::Compressor);
                c.spec.set("thr", -30.0);
                c.spec.set("ratio", 4.0);
                c
            }
        };
        bus.execute(
            Command::InsertEffect {
                strip: *s,
                index: 0,
                slot: Box::new(slot),
            },
            Origin::User,
        )
        .expect("insert");
    }
    p = bus.project().clone();
    let (mut engine, mut processor): (EngineHandle, Processor) = EngineHandle::manual(SR);
    host.sync(&p, &mut engine);
    let snapshot = engine.compile(&p, 1);
    engine.set_snapshot(snapshot);
    let mut buf = vec![0f32; frames * 2];
    for _ in 0..64 {
        processor.process(&mut buf, 2);
        engine.poll();
    }
    engine.seek(Ticks::ZERO);
    engine.play();
    for _ in 0..32 {
        processor.process(&mut buf, 2);
        engine.poll();
    }
    let blocks = (seconds * SR / frames as f64) as usize;
    let mut times = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        let t = Instant::now();
        processor.process(&mut buf, 2);
        times.push(t.elapsed().as_secs_f64() * 1e6);
        engine.poll();
    }
    engine.stop();
    drop(host);
    median(&times)
}

/// The group.
pub fn suite(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    let Some(pool) = pool() else {
        return out;
    };
    let Some(gain_path) = build(
        "cantus-test-clap",
        "cantus_test_clap",
        &[],
        "bench-gain.clap",
    ) else {
        return out;
    };
    let Some(gain) = scan(&gain_path).into_iter().next() else {
        eprintln!("bench: isolation: the test gain did not scan");
        return out;
    };
    let calls = if plan.quick { 2_000 } else { 10_000 };
    for frames in [64usize, 128, 256] {
        for (name, mode) in MODES {
            let mut loaded = load(&pool, &gain, mode, frames);
            let (hot, paced) = round_trips(loaded.audio.as_mut(), frames, calls);
            for (suffix, v) in [("", &hot), (".paced", &paced)] {
                let id = format!("isolation.rt.{name}.b{frames}{suffix}");
                out.push(
                    Metric::new(&id, "µs/call", Better::Lower, Summary::exact(median(v)))
                        .ungated()
                        .note(format!("p99 {:.1} µs", quantile(v, 0.99))),
                );
            }
        }
    }
    let seconds = if plan.quick { 2.0 } else { 5.0 };
    for frames in [64usize, 128, 256] {
        for (name, mode) in &MODES[1..] {
            let share = cpu_share(&pool, &gain, *mode, frames, seconds);
            out.push(
                Metric::new(
                    format!("isolation.cpu.{name}.b{frames}"),
                    "% of a core per instance",
                    Better::Lower,
                    Summary::exact(share * 100.0),
                )
                .ungated()
                .note("the helpers' CPU, paced at real time, 8 instances"),
            );
        }
    }
    let comprimo = build(
        "cantus-plugins",
        "cantus_plugins",
        &["--no-default-features", "--features", "all"],
        "bench-cantus.clap",
    )
    .and_then(|p| scan(&p).into_iter().find(|i| i.name.contains("Comprimo")));
    let native = dsp32(&pool, None, None, 128, seconds);
    out.push(
        Metric::new(
            "isolation.dsp32.native.b128",
            "µs/block",
            Better::Lower,
            Summary::exact(native),
        )
        .ungated()
        .note("32 tracks, one native Comprimo each"),
    );
    if let Some(c) = comprimo.as_ref() {
        let mut within = 0.0;
        for (name, mode) in MODES {
            let v = dsp32(&pool, Some(c), Some(mode), 128, seconds);
            if mode == PluginIsolation::Within {
                within = v;
            }
            let ratio = if within > 0.0 { v / within } else { 0.0 };
            out.push(
                Metric::new(
                    format!("isolation.dsp32.{name}.b128"),
                    "µs/block",
                    Better::Lower,
                    Summary::exact(v),
                )
                .ungated()
                .note(format!(
                    "32 Comprimo CLAP instances; {ratio:.2}× within, {:.2}× native",
                    v / native.max(1e-9)
                )),
            );
        }
    }
    out
}
