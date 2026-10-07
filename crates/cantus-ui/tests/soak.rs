//! The soak (round 22, *soak*; BETA.md blocker 3): a long session of the real application,
//! headless — whole frames of the window at 60 Hz, the demo songs looped, random edits
//! through the commands the interface uses, recording passes against a fake input, device
//! windows opened and closed, undo/redo storms, saves and opens — while the process's
//! working set, private bytes, handles and threads, the frame time and the callback's
//! overruns are sampled. It fails on a panic, a hang (a frame or the callback stalled),
//! a save/open that does not round-trip, or growth beyond the bounds below.
//!
//! ```text
//! CANTUS_SOAK_MINUTES=30 cargo test -p cantus-ui --release --test soak -- --ignored --nocapture
//! ```
//!
//! `CANTUS_SOAK_SEED` replays a session; `CANTUS_SOAK_OUT` is where `soak.csv` (one row
//! per sample) goes (default: a temporary folder, printed). `soak_smoke` runs the same
//! script for a few hundred frames in the normal suite.

mod harness;
mod soak_actions;

use soak_actions::{Kind, Rng};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What the operating system says about this process.
#[derive(Copy, Clone, Debug, Default)]
struct Proc {
    working_set: u64,
    private: u64,
    handles: u64,
    threads: u64,
}

/// Ask PowerShell (Windows) or `/proc` (elsewhere): no unsafe code in a test.
fn sample_process() -> Option<Proc> {
    let pid = std::process::id();
    if cfg!(windows) {
        let cmd = format!(
            "$p = Get-Process -Id {pid}; \"$($p.WorkingSet64) $($p.PrivateMemorySize64) $($p.HandleCount) $($p.Threads.Count)\""
        );
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let v: Vec<u64> = s.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        (v.len() == 4).then(|| Proc {
            working_set: v[0],
            private: v[1],
            handles: v[2],
            threads: v[3],
        })
    } else {
        let s = std::fs::read_to_string("/proc/self/status").ok()?;
        let kb = |k: &str| {
            s.lines()
                .find(|l| l.starts_with(k))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
                * 1024
        };
        let fds = std::fs::read_dir("/proc/self/fd").map(|d| d.count()).unwrap_or(0) as u64;
        Some(Proc {
            working_set: kb("VmRSS:"),
            private: kb("RssAnon:"),
            handles: fds,
            threads: kb("Threads:") / 1024,
        })
    }
}

/// One row of the curves.
#[derive(Clone, Debug, Default)]
struct Row {
    t_s: f64,
    proc_: Proc,
    frames: usize,
    frame_p50_ms: f64,
    frame_p99_ms: f64,
    frame_max_ms: f64,
    overruns: u64,
    callback_max_ms: f64,
    engine_overruns: u64,
    /// Messages the engine dropped or units it found missing (`EngineStats::dropped_events`).
    engine_dropped: u64,
    /// Effect units the handle believes the engine holds.
    known_effects: usize,
    history: usize,
    tracks: usize,
    recorded_s: f64,
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn mb(b: u64) -> f64 {
    b as f64 / 1_048_576.0
}

struct Config {
    duration: Duration,
    seed: u64,
    out: PathBuf,
    /// Frames a second (0: as fast as they come — the smoke).
    fps: f64,
    sample_every: Duration,
    /// Scales the intervals between recordings, demos and saves (1: as written).
    pace: f64,
}

fn run(cfg: Config) -> Vec<Row> {
    let work = cfg.out.join("work");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    // Nothing the session writes reaches the user's own folder.
    std::env::set_var("CANTUS_DATA_DIR", work.join("data"));
    let song = work.join("Song").join("soak.cantus");

    let (engine, mut live) = harness::LiveEngine::start(256);
    let mut rng = Rng::new(cfg.seed);
    let demos = &cantus_ui::demos::ALL;
    let mut app = cantus_ui::CantusApp::from_parts(Some((demos[0].build)()), engine, false);
    harness::isolate(&mut app, &work.join("data"));
    app.autosave_every = Some(Duration::from_secs(10));
    app.rec.demo_inputs = Some(vec![0.0, 0.0]);
    app.start_session(&work.join("data"));
    app.save_to_path(&song);
    assert!(!app.status.error, "{}", app.status.text);
    app.toggle_play();
    let ctx = harness::ctx();
    let size = egui::Vec2::new(1600.0, 900.0);

    let sampled: Arc<Mutex<Option<Proc>>> = Arc::new(Mutex::new(None));
    let start = Instant::now();
    let mut rows = Vec::new();
    let mut window: Vec<f64> = Vec::new();
    let mut counts: BTreeMap<Kind, usize> = BTreeMap::new();
    let mut next_sample = Instant::now();
    let secs = |s: u64| Duration::from_secs_f64(s as f64 * cfg.pace);
    let mut next_record = start + secs(45);
    let mut next_demo = start + secs(180);
    let mut next_save = start + secs(60);
    let mut recording_until: Option<Instant> = None;
    let mut recorded_frames = 0u64;
    let mut pass_frames = 0u64;
    let mut demo = 0usize;
    let mut frames = 0usize;
    let mut slowest = Duration::ZERO;
    let mut last = String::from("start");
    let mut slow: Vec<(f64, Duration, String)> = Vec::new();
    let period = (cfg.fps > 0.0).then(|| Duration::from_secs_f64(1.0 / cfg.fps));
    let mut sampler: Option<std::thread::JoinHandle<()>> = None;
    loop {
        let now = Instant::now();
        let t = now - start;
        let done = t >= cfg.duration;

        // The curves: one row per sample period, the process figures from a thread.
        if now >= next_sample || done {
            next_sample = now + cfg.sample_every;
            // A sample still under way (PowerShell is slow on a busy machine) is left to
            // finish: the loop never waits for it.
            if sampler.as_ref().is_none_or(|h| h.is_finished()) {
                let s = sampled.clone();
                sampler = Some(std::thread::spawn(move || {
                    let p = sample_process();
                    if p.is_some() {
                        *s.lock().unwrap() = p;
                    }
                }));
            }
            let st = &live.stats;
            let proc_ = sampled.lock().unwrap().unwrap_or_default();
            let row = Row {
                t_s: t.as_secs_f64(),
                proc_,
                frames: window.len(),
                frame_p50_ms: pct(&mut window, 0.5),
                frame_p99_ms: pct(&mut window, 0.99),
                frame_max_ms: window.iter().copied().fold(0.0, f64::max),
                overruns: st.overruns.load(Ordering::Relaxed),
                callback_max_ms: st.max_us.load(Ordering::Relaxed) as f64 / 1000.0,
                engine_overruns: app.engine.stats().overruns,
                engine_dropped: app.engine.stats().dropped_events,
                known_effects: app.engine.known_effect_count(),
                history: app.bus.history().len(),
                tracks: app.project().tracks.len(),
                recorded_s: (recorded_frames + pass_frames) as f64 / f64::from(harness::RATE),
            };
            println!(
                "soak {:>7.1} s  ws {:>7.1} MB  priv {:>7.1} MB  handles {:>5}  threads {:>3}  \
                 frame p50 {:>5.1} p99 {:>5.1} max {:>6.1} ms  overruns {:>4}  dropped {:>4}  fx {:>4}  history {:>5}  tracks {:>3}",
                row.t_s,
                mb(row.proc_.working_set),
                mb(row.proc_.private),
                row.proc_.handles,
                row.proc_.threads,
                row.frame_p50_ms,
                row.frame_p99_ms,
                row.frame_max_ms,
                row.overruns,
                row.engine_dropped,
                row.known_effects,
                row.history,
                row.tracks
            );
            rows.push(row);
            window.clear();
            // The callback must keep running: a stall is a hang.
            let last_block = Duration::from_micros(live.stats.last_block_at_us.load(Ordering::Relaxed));
            let since = live.started.elapsed().saturating_sub(last_block);
            assert!(since < Duration::from_secs(5), "the callback stalled for {since:?}");
        }
        if done {
            break;
        }

        // A recording pass: start, run a few seconds, stop.
        if let Some(until) = recording_until {
            if now >= until {
                app.stop_recording();
                recorded_frames += std::mem::take(&mut pass_frames);
                recording_until = None;
                *counts.entry(Kind::Record).or_default() += 1;
                last = "RecordStop".into();
            } else if let Some(p) = &app.rec.pass {
                pass_frames = p.takes.iter().map(|t| t.frames).sum();
            }
        } else if now >= next_record {
            next_record = now + secs(90 + rng.below(60) as u64);
            if soak_actions::arm_audio_track(&mut app).is_some() {
                harness::frame(&mut app, &ctx, size, t.as_secs_f64());
                app.start_recording();
                last = "RecordStart".into();
                if app.rec.pass.as_ref().is_some_and(|p| p.audio) {
                    recording_until = Some(now + Duration::from_secs(2 + rng.below(4) as u64));
                }
            }
        } else if now >= next_demo {
            next_demo = now + secs(180);
            demo = (demo + 1) % demos.len();
            cantus_ui::views::demos::open(&mut app, demo);
            app.save_to_path(&song);
            app.toggle_play();
            *counts.entry(Kind::Demo).or_default() += 1;
            last = "Demo".into();
        } else if now >= next_save {
            next_save = now + secs(45 + rng.below(45) as u64);
            if let Err(e) = soak_actions::save_and_open(&mut app, &song) {
                panic!("save/open at {:.0} s: {e}", t.as_secs_f64());
            }
            app.toggle_play();
            *counts.entry(Kind::SaveOpen).or_default() += 1;
            last = "SaveOpen".into();
        } else if frames.is_multiple_of(6) {
            let kinds = [
                (Kind::Notes, 30),
                (Kind::Clips, 10),
                (Kind::Automation, 3),
                (Kind::Mixer, 25),
                (Kind::Effects, 10),
                (Kind::Windows, 8),
                (Kind::UndoStorm, 2),
                (Kind::Transport, 6),
            ];
            let total: usize = kinds.iter().map(|k| k.1).sum();
            let mut r = rng.below(total);
            let kind = kinds
                .iter()
                .find(|k| {
                    if r < k.1 {
                        true
                    } else {
                        r -= k.1;
                        false
                    }
                })
                .map(|k| k.0)
                .unwrap_or(Kind::Notes);
            soak_actions::edit(&mut app, &mut rng, kind);
            last = format!("{kind:?}");
            *counts.entry(kind).or_default() += 1;
        }

        let dt = harness::frame(&mut app, &ctx, size, t.as_secs_f64());
        if dt > Duration::from_millis(250) {
            println!("soak: slow frame {dt:?} at {:.1} s after {last}", t.as_secs_f64());
            slow.push((t.as_secs_f64(), dt, last.clone()));
        }
        last.clear();
        slowest = slowest.max(dt);
        assert!(dt < Duration::from_secs(10), "a frame took {dt:?}: a hang");
        window.push(dt.as_secs_f64() * 1000.0);
        frames += 1;
        if let Some(p) = period {
            if let Some(rest) = p.checked_sub(dt) {
                std::thread::sleep(rest);
            }
        }
    }
    if let Some(h) = sampler.take() {
        let _ = h.join();
    }
    app.close_session();
    live.stop();
    println!("soak: {frames} frames, slowest {slowest:?}; actions {counts:?}");
    println!("soak: {} frames over 250 ms: {slow:?}", slow.len());
    rows
}

fn write_csv(rows: &[Row], path: &Path) {
    let mut f = std::fs::File::create(path).unwrap();
    let _ = writeln!(
        f,
        "t_s,working_set_mb,private_mb,handles,threads,frames,frame_p50_ms,frame_p99_ms,frame_max_ms,callback_overruns,callback_max_ms,engine_overruns,engine_dropped,known_effects,history,tracks,recorded_s"
    );
    for r in rows {
        let _ = writeln!(
            f,
            "{:.1},{:.2},{:.2},{},{},{},{:.2},{:.2},{:.2},{},{:.3},{},{},{},{},{},{:.1}",
            r.t_s,
            mb(r.proc_.working_set),
            mb(r.proc_.private),
            r.proc_.handles,
            r.proc_.threads,
            r.frames,
            r.frame_p50_ms,
            r.frame_p99_ms,
            r.frame_max_ms,
            r.overruns,
            r.callback_max_ms,
            r.engine_overruns,
            r.engine_dropped,
            r.known_effects,
            r.history,
            r.tracks,
            r.recorded_s
        );
    }
}

/// Growth after warm-up: the last fifth's median against the median of the samples
/// between minutes 3 and 6 (the demos, the effects and the caches all met once).
fn growth(rows: &[Row], f: impl Fn(&Row) -> f64) -> (f64, f64) {
    let mut warm: Vec<f64> = rows
        .iter()
        .filter(|r| (180.0..=360.0).contains(&r.t_s))
        .map(&f)
        .collect();
    let n = rows.len();
    let mut tail: Vec<f64> = rows[n - (n / 5).max(1)..].iter().map(&f).collect();
    (pct(&mut warm, 0.5), pct(&mut tail, 0.5))
}

#[test]
#[ignore = "long: CANTUS_SOAK_MINUTES=30 cargo test -p cantus-ui --release --test soak -- --ignored --nocapture"]
fn soak() {
    let minutes: f64 = std::env::var("CANTUS_SOAK_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0);
    let seed = std::env::var("CANTUS_SOAK_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(22);
    let out = std::env::var_os("CANTUS_SOAK_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("cantus-soak"));
    std::fs::create_dir_all(&out).unwrap();
    println!("soak: {minutes} min, seed {seed}, out {}", out.display());
    let rows = run(Config {
        duration: Duration::from_secs_f64(minutes * 60.0),
        seed,
        out: out.clone(),
        fps: 60.0,
        sample_every: Duration::from_secs(10),
        pace: 1.0,
    });
    write_csv(&rows, &out.join("soak.csv"));
    let ws = growth(&rows, |r| mb(r.proc_.working_set));
    let private = growth(&rows, |r| mb(r.proc_.private));
    let handles = growth(&rows, |r| r.proc_.handles as f64);
    let threads = growth(&rows, |r| r.proc_.threads as f64);
    let recorded = rows.last().map(|r| r.recorded_s).unwrap_or(0.0);
    println!(
        "growth after warm-up: working set {:.1} → {:.1} MB, private {:.1} → {:.1} MB, \
         handles {:.0} → {:.0}, threads {:.0} → {:.0}; {recorded:.0} s recorded",
        ws.0, ws.1, private.0, private.1, handles.0, handles.1, threads.0, threads.1
    );
    if minutes >= 10.0 {
        // Recorded audio stays in memory by design (ADR 0025 §9: decoded once, f32,
        // never evicted while the session lives): that much is allowed on top.
        let audio_mb = recorded * f64::from(harness::RATE) * 4.0 * 2.0 / 1_048_576.0;
        let allowance = 96.0 + audio_mb;
        assert!(
            private.1 - private.0 <= allowance,
            "private bytes grew {:.1} MB (allowed {allowance:.1})",
            private.1 - private.0
        );
        assert!(handles.1 - handles.0 <= 200.0, "handles grew {:?}", handles);
        assert!(threads.1 - threads.0 <= 8.0, "threads grew {:?}", threads);
    }
}

#[test]
fn soak_smoke() {
    let out = tempfile::tempdir().unwrap();
    let rows = run(Config {
        duration: Duration::from_secs(20),
        seed: 7,
        out: out.path().to_path_buf(),
        fps: 0.0,
        sample_every: Duration::from_secs(5),
        pace: 0.08,
    });
    assert!(rows.len() >= 2);
}
