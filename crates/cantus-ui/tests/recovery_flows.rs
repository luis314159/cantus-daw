//! Recovery and refusals inside one process (round 22, *soak*): the journal's life (a
//! clean exit leaves nothing, a close with unsaved work offers it back, a running session
//! is never taken for a gone one), files that *Open* refuses with the session untouched,
//! a save that fails visibly, and the pieces the crash tests lean on (which file is whose
//! take, where a loop wrapped, a WAV header repaired).

mod harness;

use cantus_core::audio::TakeMode;
use cantus_core::{Command, Project, Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::recovery::{self, repair_wav, JournalPass, JournalTake};
use cantus_ui::recovery_restore::wraps;
use cantus_ui::recovery_session::match_takes;
use cantus_ui::CantusApp;
use std::fs;
use std::path::{Path, PathBuf};

fn app(data: &Path, project: Project) -> CantusApp {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let mut a = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
    harness::isolate(&mut a, data);
    a
}

fn song() -> Project {
    let mut p = Project::new("Song");
    let t = p.make_track("Keys");
    p.tracks.push(t);
    p
}

fn rename(a: &mut CantusApp, name: &str) {
    a.exec(Command::SetProjectName { name: name.into() });
    a.end_gesture();
}

#[test]
fn a_clean_exit_leaves_nothing_and_unsaved_work_is_offered_back() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    // Nothing unsaved: the journal goes with the session.
    let mut a = app(data, song());
    a.start_session(data);
    assert!(a.recovery.is_none());
    a.close_session();
    assert_eq!(recovery::orphans(&recovery::sessions_dir(data)), vec![]);
    let leftovers: Vec<_> = fs::read_dir(recovery::sessions_dir(data))
        .unwrap()
        .flatten()
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // Unsaved changes at close: autosaved on the way out, and offered next time.
    let mut a = app(data, song());
    a.start_session(data);
    rename(&mut a, "Unsaved idea");
    a.close_session();
    let mut b = app(data, Project::new("next"));
    b.start_session(data);
    let item = &b.recovery.as_ref().expect("offered").items[0];
    assert!(!item.crashed, "it closed; it did not crash");
    assert!(item.project_path.is_none());
    let lines = cantus_ui::views::recovery::lines(item, u64::MAX / 2);
    assert!(lines[0].contains("Unsaved idea"), "{lines:?}");
    // Round 31 (ADR 0072): the start page waits behind the offer, and the work restored
    // is what opens — no start page over it.
    b.workflow.start.open_at_launch();
    assert!(cantus_ui::workflow::start_page_waits(&b));
    assert!(b.restore_recovery(0));
    assert!(!b.workflow.start.is_open(), "restored: no start page");
    assert_eq!(b.project().name, "Unsaved idea");
    assert!(b.bus.is_dirty());
    assert!(b.recovery.is_none());
    b.end_session(false);

    // Discard forgets the journal and keeps the autosave on disk.
    let mut c = app(data, song());
    c.start_session(data);
    rename(&mut c, "Second idea");
    c.close_session();
    let mut d = app(data, song());
    d.start_session(data);
    let auto = d.recovery.as_ref().unwrap().items[0]
        .autosave
        .as_ref()
        .unwrap()
        .0
        .clone();
    d.discard_recovery();
    assert!(auto.exists(), "discarding deletes no file");
    d.end_session(false);
    let mut e = app(data, song());
    e.start_session(data);
    assert!(e.recovery.is_none());
}

#[test]
fn a_running_session_is_never_taken_for_a_gone_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = app(dir.path(), song());
    first.start_session(dir.path());
    rename(&mut first, "Busy");
    first.autosave_every = Some(std::time::Duration::ZERO);
    first.maybe_autosave();
    first.poll_autosave(true);
    first.journal_tick();
    // A second Cantus starting now sees the first one's journal locked.
    let mut second = app(dir.path(), song());
    second.start_session(dir.path());
    assert!(second.recovery.is_none());
    assert!(first.session.as_ref().unwrap().journal_path().exists());
    second.end_session(false);
    first.end_session(false);
}

#[test]
fn open_refuses_a_bad_file_and_leaves_everything_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.cantus");
    cantus_project::save(&song(), &good, "t").unwrap();
    let text = fs::read_to_string(&good).unwrap();
    let mut newer: serde_json::Value = serde_json::from_str(&text).unwrap();
    newer["schema_version"] = serde_json::Value::from(cantus_project::SCHEMA_VERSION + 1);
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "cut.cantus",
            text.as_bytes()[..text.len() / 3].to_vec(),
            "incomplete",
        ),
        (
            "bad.cantus",
            b"{\"format\": \"cantus-project\", \"schema_version\": 20, \"project\": 7}".to_vec(),
            "damaged",
        ),
        (
            "newer.cantus",
            serde_json::to_vec_pretty(&newer).unwrap(),
            "newer Cantus",
        ),
        (
            "other.cantus",
            b"{\"hello\": 1}".to_vec(),
            "not a Cantus project",
        ),
        ("empty.cantus", Vec::new(), "incomplete"),
    ];
    let mut a = app(dir.path(), song());
    let here = dir.path().join("here.cantus");
    a.save_to_path(&here);
    rename(&mut a, "Work in progress");
    let before = a.project().clone();
    for (name, bytes, words) in cases {
        let p = dir.path().join(name);
        fs::write(&p, &bytes).unwrap();
        a.open(&p);
        assert!(a.status.error, "{name}: {}", a.status.text);
        assert!(a.status.text.contains(words), "{name}: {}", a.status.text);
        assert!(a.status.text.contains(name), "{name}: {}", a.status.text);
        assert!(
            a.status.text.contains("unchanged"),
            "{name}: {}",
            a.status.text
        );
        assert_eq!(
            a.project(),
            &before,
            "{name}: the open project is untouched"
        );
        assert_eq!(a.path.as_deref(), Some(here.as_path()));
        assert!(a.bus.is_dirty(), "{name}: still unsaved");
        assert_eq!(
            fs::read(&p).unwrap(),
            bytes,
            "{name}: the file is untouched"
        );
    }
    // A damaged file with a good backup beside it says where the good copy is.
    cantus_project::save(&song(), &good, "t").unwrap();
    fs::write(&good, b"{").unwrap();
    a.open(&good);
    assert!(
        a.status.text.contains("good.cantus.bak"),
        "{}",
        a.status.text
    );

    // And in Spanish.
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    a.open(&dir.path().join("newer.cantus"));
    assert!(a.status.text.contains("más nuevo"), "{}", a.status.text);
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn a_save_that_cannot_be_written_fails_visibly_and_loses_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path(), song());
    rename(&mut a, "Precious");
    let before = a.project().clone();
    // The folder is a file: nothing can be written there.
    let blocked = dir.path().join("Blocked");
    fs::write(&blocked, b"a file where a folder should be").unwrap();
    let path = blocked.join("precious.cantus");
    a.save_to_path(&path);
    assert!(a.status.error);
    assert!(
        a.status.text.contains("Could not save"),
        "{}",
        a.status.text
    );
    assert!(
        a.status.text.contains("Nothing was lost"),
        "{}",
        a.status.text
    );
    assert_eq!(a.project(), &before);
    assert!(a.bus.is_dirty());
    assert_eq!(
        a.path, None,
        "the session is not pointed at a file it could not write"
    );

    // A project whose folder stops being writable autosaves to the fallback and says so.
    let ok = dir.path().join("Song").join("song.cantus");
    a.save_to_path(&ok);
    assert!(!a.status.error, "{}", a.status.text);
    fs::remove_dir_all(ok.parent().unwrap()).unwrap();
    fs::write(ok.parent().unwrap(), b"gone").unwrap();
    rename(&mut a, "Precious 2");
    a.autosave_every = Some(std::time::Duration::ZERO);
    a.maybe_autosave();
    a.poll_autosave(true);
    assert!(a.status.error);
    assert!(a.status.text.contains("now goes to"), "{}", a.status.text);
    let fallback = cantus_project::fallback_autosave_path(&a.autosave_dir, "song");
    assert_eq!(
        cantus_project::load(&fallback, true).unwrap().project.name,
        "Precious 2"
    );
}

#[test]
fn each_new_file_is_matched_to_its_armed_track() {
    let t = |n: u64| TrackId(n);
    let dir = PathBuf::from("C:/rec");
    let files = vec![
        dir.join("Gtr 2026-10-06 101500.wav"),
        dir.join("Gtr 2026-10-06 101500 2.wav"),
        dir.join("Vox_ L 2026-10-06 101500.wav"),
        dir.join("Take 2026-10-06 101500.wav"),
    ];
    let tracks = vec![
        (t(1), "Gtr".to_string()),
        (t(2), "Gtr".to_string()),
        (t(3), "Vox: L".to_string()),
        (t(4), " . ".to_string()),
    ];
    let m = match_takes(&tracks, &files);
    let want = |id: u64, i: usize| JournalTake {
        track: t(id),
        path: files[i].clone(),
    };
    assert_eq!(m, vec![want(1, 0), want(2, 1), want(3, 2), want(4, 3)]);
}

#[test]
fn a_recovered_loop_pass_wraps_where_the_engine_would_have() {
    // 120 BPM at 48 kHz: 25 frames a tick; the loop is beats 1–3 (48 000 frames).
    let pass = JournalPass {
        record_start: Ticks::beats(1),
        transport_start: Ticks::beats(1),
        punch: None,
        loop_region: Some((Ticks::beats(1), Ticks::beats(3))),
        loop_takes: true,
        mode: TakeMode::Stack,
        tempo_bpm: 120.0,
        tempo_points: vec![],
        sample_rate: 48_000,
        latency_frames: 0,
        marked_wraps: Vec::new(),
        takes: vec![],
        finished: false,
    };
    assert_eq!(wraps(&pass, 130_000), vec![48_000, 96_000]);
    // Under a tempo track (ADR 0060): 60 BPM from beat 2, so the loop (beats 1–3) is a
    // half second at 120 and a second at 60 — 72 000 frames — and its first end is 72 000
    // frames after beat 1.
    let mapped = JournalPass {
        tempo_points: vec![
            cantus_core::TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: false,
            },
            cantus_core::TempoPoint {
                tick: Ticks::beats(2),
                bpm: 60.0,
                ramp: false,
            },
        ],
        ..pass.clone()
    };
    assert_eq!(wraps(&mapped, 200_000), vec![72_000, 144_000]);
    // With a bar of count-in (4 beats before), the first wrap comes a bar later.
    let counted = JournalPass {
        transport_start: Ticks::beats(1) - Ticks::beats(4),
        ..pass.clone()
    };
    assert_eq!(wraps(&counted, 200_000), vec![144_000, 192_000]);
    let no_loop = JournalPass {
        loop_region: None,
        ..pass
    };
    assert!(wraps(&no_loop, 1_000_000).is_empty());
}

#[test]
fn a_wav_cut_off_by_a_crash_is_repaired_to_its_last_whole_frame() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..1_000 {
        w.write_sample(i as f32).unwrap();
        w.write_sample(-(i as f32)).unwrap();
    }
    w.flush().unwrap(); // the header says 1 000 frames
    for i in 1_000..1_500 {
        w.write_sample(i as f32).unwrap();
        w.write_sample(-(i as f32)).unwrap();
    }
    // The process dies here: what the buffer held reaches the disk, the header does not.
    std::mem::forget(w);
    // (`forget` keeps hound's buffer in memory: write the frames as the OS would have.)
    let mut bytes = fs::read(&path).unwrap();
    let data = bytes.windows(4).position(|w| w == b"data").unwrap() + 8;
    let have = (bytes.len() - data) / 8;
    for i in have..1_500 {
        bytes.extend_from_slice(&(i as f32).to_le_bytes());
        bytes.extend_from_slice(&(-(i as f32)).to_le_bytes());
    }
    bytes.extend_from_slice(&[1, 2, 3]); // half a frame
    fs::write(&path, &bytes).unwrap();
    assert_eq!(hound::WavReader::open(&path).unwrap().duration(), 1_000);
    assert_eq!(repair_wav(&path).unwrap(), 1_500);
    let r = hound::WavReader::open(&path).unwrap();
    assert_eq!(r.duration(), 1_500);
    let s: Vec<f32> = r.into_samples::<f32>().map(|s| s.unwrap()).collect();
    assert_eq!(s[2_998], 1_499.0);
    assert_eq!(s[2_999], -1_499.0);
    // Repairing again changes nothing; a file that is not a WAV is left alone.
    assert_eq!(repair_wav(&path).unwrap(), 1_500);
    let not = dir.path().join("x.wav");
    fs::write(&not, b"hello").unwrap();
    assert!(repair_wav(&not).is_err());
    assert_eq!(fs::read(&not).unwrap(), b"hello");
}

/// A folder the user may not write to (an ACL that denies it, as a synced or managed folder
/// can): the save says *permission denied* and nothing is lost.
#[cfg(windows)]
#[test]
fn a_read_only_folder_refuses_the_save_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("Locked");
    fs::create_dir_all(&locked).unwrap();
    let icacls = |args: &[&str]| {
        std::process::Command::new("icacls")
            .arg(&locked)
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    if !icacls(&["/deny", "*S-1-1-0:(OI)(CI)(W)"]) {
        eprintln!("icacls is not available: skipped");
        return;
    }
    let mut a = app(dir.path(), song());
    rename(&mut a, "Locked out");
    let before = a.project().clone();
    a.save_to_path(&locked.join("song.cantus"));
    let text = a.status.text.clone();
    let error = a.status.error;
    icacls(&["/remove:d", "*S-1-1-0"]);
    assert!(error, "{text}");
    assert!(text.contains("permission denied"), "{text}");
    assert!(text.contains("Nothing was lost"), "{text}");
    assert_eq!(a.project(), &before);
    assert!(a.bus.is_dirty());
    assert_eq!(a.path, None);
}

// ---- round 29, freeze's third review ---------------------------------------------------

#[test]
fn the_journal_keeps_the_figures_the_engine_marked_for_the_pass() {
    // A crashed pass is placed by the latency the engine marked at its start (not the
    // callback's before it) and by the wraps it marked (review 3).
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let mut p = Project::new("Rec");
    let mut t = p.make_track("Voice");
    t.input = Some(cantus_core::audio::TrackInput::mono(0));
    t.armed = true;
    p.tracks.push(t);
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(48_000);
    proc.attach_input(consumer, 1);
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let mut a = CantusApp::from_parts(Some(p), engine, false);
    harness::isolate(&mut a, &data);
    a.start_session(&data);
    a.sync_engine();
    // A pass journaled as recording starts: with a figure that is not the engine's.
    if let Some(s) = a.session.as_mut() {
        s.journal.passes.push(JournalPass {
            record_start: Ticks::ZERO,
            transport_start: Ticks::ZERO,
            punch: None,
            loop_region: None,
            loop_takes: false,
            mode: TakeMode::Stack,
            tempo_bpm: 120.0,
            tempo_points: vec![],
            sample_rate: 48_000,
            latency_frames: 987_654,
            marked_wraps: Vec::new(),
            takes: vec![],
            finished: false,
        });
    }
    a.engine.begin_recording(&dir.path().join("takes")).unwrap();
    a.engine.set_recording(true);
    a.engine.play();
    let mut buf = vec![0f32; 512];
    for _ in 0..8 {
        for _ in 0..256 {
            let _ = ring.push(0.0);
        }
        proc.process(&mut buf, 2);
        a.engine.poll();
    }
    a.journal_tick();
    let (figure, _) = a.engine.current_pass().expect("the pass started");
    let got = a
        .session
        .as_ref()
        .unwrap()
        .journal
        .passes
        .last()
        .unwrap()
        .latency_frames;
    assert_eq!(got, figure.total());
}

#[test]
fn a_recovered_loop_pass_is_laid_by_the_wraps_the_engine_marked() {
    // The engine marked the first wrap 300 frames later in the file (the heard latency
    // moved: silence written): the recovery lays every later pass by it.
    let mut pass = JournalPass {
        record_start: Ticks::beats(1),
        transport_start: Ticks::beats(1),
        punch: None,
        loop_region: Some((Ticks::beats(1), Ticks::beats(3))),
        loop_takes: true,
        mode: TakeMode::Stack,
        tempo_bpm: 120.0,
        tempo_points: vec![],
        sample_rate: 48_000,
        latency_frames: 0,
        marked_wraps: Vec::new(),
        takes: vec![],
        finished: false,
    };
    assert_eq!(wraps(&pass, 150_000), vec![48_000, 96_000, 144_000]);
    pass.marked_wraps = vec![(48_300, -300)];
    assert_eq!(wraps(&pass, 150_000), vec![48_300, 96_300, 144_300]);
}
