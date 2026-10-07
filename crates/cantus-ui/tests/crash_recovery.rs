//! Crash recovery, proved by killing a real session (round 22, *soak*; BETA.md blocker 3).
//!
//! Each test spawns this test binary again as a **child** (the ignored test `child`, told
//! what to do by `CANTUS_CRASH_CHILD`) that runs the application headless — whole frames,
//! the session journal, autosave every second — and prints what it did as it does it. The
//! parent kills it (`TerminateProcess` on Windows: no destructor runs, nothing is flushed)
//! and then starts a session over the same data folder, as the next launch would:
//!
//! - **mid-edit**: the autosave restores the project to an edit made no more than the
//!   autosave interval (and a frame) before the kill; *Restore* puts it in place as one
//!   undo step over the last save.
//! - **mid-recording**: the take's WAV reads back up to its last header flush (≤ 0.5 s
//!   before the kill, ADR 0025 §8), every frame the input that was played, and the
//!   recovery repairs the header to the last whole frame on disk and offers the take,
//!   which *Restore* lays on its track.

mod harness;

use cantus_core::audio::TrackInput;
use cantus_core::{Command, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::recovery::{self, repair_wav};
use cantus_ui::recovery_restore;
use cantus_ui::CantusApp;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as Proc, Stdio};
use std::time::{Duration, Instant};

const AUTOSAVE: Duration = Duration::from_secs(1);
const SIZE: egui::Vec2 = egui::Vec2::new(1280.0, 800.0);

fn base_project(record: bool) -> Project {
    let mut p = Project::new("base");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    if record {
        t.input = Some(TrackInput::mono(0));
        t.armed = true;
    } else {
        let c = p.make_clip("c", Ticks::ZERO, Ticks::beats(16));
        t.clips.push(c);
    }
    p.tracks.push(t);
    p
}

fn say(line: String) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// The child: a session that edits (or records) until it is killed.
#[test]
#[ignore = "run by the crash tests as a child process"]
fn child() {
    let Ok(mode) = std::env::var("CANTUS_CRASH_CHILD") else {
        return;
    };
    let data = PathBuf::from(std::env::var("CANTUS_DATA_DIR").unwrap());
    let path = PathBuf::from(std::env::var("CANTUS_CRASH_PROJECT").unwrap());
    let project = cantus_project::load(&path, true).unwrap().project;
    let (engine, live) = if mode == "record" {
        let (h, l) = harness::LiveEngine::start(256);
        (h, Some(l))
    } else {
        (EngineHandle::dummy(), None)
    };
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    harness::isolate(&mut app, &data);
    app.path = Some(path.clone());
    app.autosave_every = Some(AUTOSAVE);
    app.start_session(&data);
    let ctx = harness::ctx();
    let t0 = Instant::now();
    let ms = || t0.elapsed().as_millis();
    if mode == "record" {
        app.rec.demo_inputs = Some(vec![0.0]);
        harness::frame(&mut app, &ctx, SIZE, 0.0);
        app.start_recording();
        let audio = app.rec.pass.as_ref().is_some_and(|p| p.audio);
        say(format!("RECORDING {} {}", u8::from(audio), ms()));
    }
    let mut i = 0u64;
    loop {
        if mode == "edit" {
            i += 1;
            app.exec(Command::SetProjectName {
                name: format!("edit {i}"),
            });
            app.end_gesture();
            say(format!("EDIT {i} {}", ms()));
        } else if let Some(l) = &live {
            let n = l.stats.input_frames.load(std::sync::atomic::Ordering::Relaxed);
            say(format!("INPUT {n} {}", ms()));
        }
        let dt = harness::frame(&mut app, &ctx, SIZE, t0.elapsed().as_secs_f64());
        say(format!("FRAME {} {}", dt.as_millis(), ms()));
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn spawn(mode: &str, data: &Path, project: &Path) -> Child {
    Proc::new(std::env::current_exe().unwrap())
        .args(["child", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env("CANTUS_CRASH_CHILD", mode)
        .env("CANTUS_DATA_DIR", data)
        .env("CANTUS_CRASH_PROJECT", project)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Read the child's lines until `done` says so (or a minute passes); kill it there.
fn run_until(mut child: Child, mut done: impl FnMut(&[String]) -> bool) -> Vec<String> {
    let out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut lines = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(120) {
        if let Ok(l) = rx.recv_timeout(Duration::from_millis(50)) {
            lines.push(l);
            if done(&lines) {
                break;
            }
        }
    }
    child.kill().unwrap();
    child.wait().unwrap();
    // What it printed before it died.
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(200)) {
        lines.push(l);
    }
    lines
}

/// The numbers on lines that start with `tag`.
fn tagged(lines: &[String], tag: &str) -> Vec<(u64, u64)> {
    lines
        .iter()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            // The test harness may print `test child ... ` before the first line.
            w.find(|x| *x == tag)?;
            Some((w.next()?.parse().ok()?, w.next()?.parse().ok()?))
        })
        .collect()
}

fn next_launch(data: &Path) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(Project::new("fresh")), EngineHandle::dummy(), false);
    harness::isolate(&mut app, data);
    app.start_session(data);
    app
}

#[test]
fn killed_mid_edit_the_autosave_brings_back_all_but_the_last_interval() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let proj = dir.path().join("Song").join("song.cantus");
    cantus_project::save(&base_project(false), &proj, "t").unwrap();

    // Edit for five seconds (five autosaves), then die without warning.
    let lines = run_until(spawn("edit", &data, &proj), |l| {
        tagged(l, "EDIT").last().is_some_and(|&(_, t)| t > 5_000)
    });
    let edits = tagged(&lines, "EDIT");
    let frames = tagged(&lines, "FRAME");
    let (last_i, last_t) = *edits.last().unwrap();
    let slowest = frames.iter().map(|f| f.0).max().unwrap_or(0);

    // The journal says it crashed with unsaved changes, and where they were autosaved.
    let orphans = recovery::orphans(&recovery::sessions_dir(&data));
    assert_eq!(orphans.len(), 1, "one session is gone");
    let j = &orphans[0].journal;
    assert!(j.dirty && !j.closed, "{j:?}");
    assert_eq!(j.project_path.as_deref(), Some(proj.as_path()));
    let auto = j.autosave.clone().expect("an autosave was written");
    assert_eq!(auto, cantus_project::autosave_path(&proj));
    let saved = cantus_project::load(&auto, true).unwrap().project;
    let k: u64 = saved.name["edit ".len()..].parse().unwrap();
    let t_k = edits.iter().find(|e| e.0 == k).unwrap().1;
    let lost = last_t - t_k;
    println!(
        "{} edits, killed after edit {last_i} at {last_t} ms; the autosave holds edit {k} \
         ({lost} ms earlier; slowest frame {slowest} ms)",
        edits.len()
    );
    assert!(
        lost <= AUTOSAVE.as_millis() as u64 + slowest + 300,
        "the autosave is {lost} ms behind the last edit"
    );
    assert_eq!(
        cantus_project::load(&proj, true).unwrap().project.name,
        "base",
        "the project file itself is untouched"
    );

    // The next launch offers it; Restore puts it in place over the last save.
    let mut app = next_launch(&data);
    let item = &app.recovery.as_ref().expect("offered").items[0];
    assert!(item.crashed);
    assert!(item.autosave.is_some());
    assert!(app.restore_recovery(0));
    assert_eq!(app.project().name, format!("edit {k}"));
    assert_eq!(app.path.as_deref(), Some(proj.as_path()));
    assert!(app.bus.is_dirty(), "restored work is unsaved work");
    app.undo();
    assert_eq!(app.project().name, "base", "undo goes back to the last save");
    app.redo();
    app.end_session(false);
    // Offered once.
    let again = next_launch(&data);
    assert!(again.recovery.is_none());
}

#[test]
fn killed_mid_recording_the_take_survives_and_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let proj = dir.path().join("Song").join("song.cantus");
    cantus_project::save(&base_project(true), &proj, "t").unwrap();

    let lines = run_until(spawn("record", &data, &proj), |l| {
        let rec = tagged(l, "RECORDING");
        rec.first().is_some_and(|r| r.0 == 1)
            && tagged(l, "INPUT")
                .last()
                .is_some_and(|&(_, t)| t > rec[0].1 + 3_000)
    });
    assert!(
        lines.iter().any(|l| l.contains("RECORDING 1 ")),
        "the pass recorded audio: {lines:?}"
    );
    let inputs = tagged(&lines, "INPUT");
    let &(_, last_t) = inputs.last().unwrap();
    // What had come in a second before the kill (the header is flushed every 0.5 s).
    let floor = inputs
        .iter()
        .rev()
        .find(|i| i.1 + 1_000 <= last_t)
        .map(|i| i.0)
        .unwrap();

    let audio = proj.with_file_name("Audio");
    let wavs = cantus_ui::recovery_session::wavs_in(&audio);
    assert_eq!(wavs.len(), 1, "{wavs:?}");
    let wav = &wavs[0];
    let read = |p: &Path| -> Vec<f32> {
        let mut r = hound::WavReader::open(p).expect("the take reads as a WAV");
        assert_eq!(r.spec().sample_rate, harness::RATE);
        assert_eq!(r.spec().channels, 1);
        r.samples::<f32>().map(|s| s.unwrap()).collect()
    };
    let flushed = read(wav);
    assert!(!flushed.is_empty(), "the header was flushed before the kill");
    let s0 = harness::which(flushed[0]).expect("the take starts on the input");
    for (j, v) in flushed.iter().enumerate() {
        assert_eq!(harness::which(*v), Some(s0 + j as u64), "frame {j}");
    }
    let header = flushed.len() as u64;
    println!(
        "take: {header} frames in the header ({:.2} s); input {floor} frames a second \
         before the kill, the take began at input frame {s0}",
        header as f64 / f64::from(harness::RATE)
    );
    assert!(
        s0 + header >= floor,
        "the header is more than a second behind: {} < {floor}",
        s0 + header
    );

    // The recovery repairs the header to every whole frame on disk.
    let repaired = repair_wav(wav).unwrap();
    assert!(repaired >= header);
    let all = read(wav);
    assert_eq!(all.len() as u64, repaired);
    for (j, v) in all.iter().enumerate() {
        assert_eq!(harness::which(*v), Some(s0 + j as u64), "repaired frame {j}");
    }
    println!("repaired: {repaired} frames (+{} past the last header flush)", repaired - header);

    // The next launch offers the take and lays it on its track.
    let mut app = next_launch(&data);
    let offer = app.recovery.as_ref().expect("the take is offered");
    assert_eq!(offer.items[0].take_count(), 1);
    assert!(offer.items[0].autosave.is_none(), "nothing else was unsaved");
    assert!(app.restore_recovery(0));
    let p = app.project();
    let voice = p.tracks.iter().find(|t| t.name == "Voice").unwrap();
    let clip = voice
        .clips
        .iter()
        .find(|c| c.audio.is_some())
        .expect("the take is a clip on its track");
    let source = clip.audio.as_ref().unwrap().source;
    let src = p.audio_pool.iter().find(|s| s.id == source).unwrap();
    let on_disk = |s: &str| s.replace('\\', "/").to_lowercase();
    assert!(
        on_disk(&app.audio_file(src).to_string_lossy()) == on_disk(&wav.to_string_lossy()),
        "{} vs {}",
        src.path,
        wav.display()
    );
    assert!(app.bus.is_dirty());
    app.end_session(false);
    // The take is in the project now: a crash before saving would offer it once more,
    // but a fresh start after this clean one offers nothing.
    assert!(next_launch(&data).recovery.is_none());
    let _ = recovery_restore::offer(Vec::new());
}
