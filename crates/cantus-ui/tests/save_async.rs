//! *Save* off the interface's thread (round 24, `save_job`): a save returns at once and the
//! file lands whole; an edit made while it is written keeps the project dirty; a failed
//! save says so (in both languages) and keeps it dirty; a second save waits its turn; New,
//! Open, the *Save changes?* question and the close wait for a save in flight.
//!
//! A test holds a save's thread before it writes (`Saving::hold`), so "in flight" is a
//! state the test is in, not a race it hopes to win.

mod harness;

use cantus_core::{Command, Project};
use cantus_engine::EngineHandle;
use cantus_ui::views::unsaved::{Answer, Then};
use cantus_ui::CantusApp;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

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

/// Hold the next save's thread until the returned sender sends (or drops).
fn hold(a: &mut CantusApp) -> mpsc::Sender<()> {
    let (tx, rx) = mpsc::channel();
    a.saving.hold = Some(rx);
    tx
}

fn saved_name(path: &Path) -> String {
    cantus_project::load(path, true).unwrap().project.name
}

#[test]
fn a_save_returns_at_once_and_the_file_lands_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.cantus");
    let mut a = app(dir.path(), song());
    rename(&mut a, "Whole");
    let gate = hold(&mut a);
    let t0 = Instant::now();
    assert!(a.start_save(&path));
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "the frame did not write"
    );
    assert!(a.save_in_flight());
    assert!(
        !path.exists(),
        "nothing is written while the thread is held"
    );
    assert!(a.status.text.contains("Saving"), "{}", a.status.text);
    assert!(a.bus.is_dirty(), "not saved until it has landed");
    // Frames go on while it is written.
    a.poll_save(false);
    assert!(a.save_in_flight());
    gate.send(()).unwrap();
    a.wait_for_save();
    assert!(!a.save_in_flight());
    assert!(!a.bus.is_dirty());
    assert_eq!(a.path.as_deref(), Some(path.as_path()));
    assert!(a.status.text.contains("Saved"), "{}", a.status.text);
    let loaded = cantus_project::load(&path, true).unwrap();
    assert!(loaded.integrity_verified, "the file is whole");
    assert_eq!(&loaded.project, a.project());
}

#[test]
fn an_edit_made_during_a_save_keeps_the_project_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.cantus");
    let mut a = app(dir.path(), song());
    rename(&mut a, "Before");
    let gate = hold(&mut a);
    a.start_save(&path);
    rename(&mut a, "During");
    drop(gate);
    a.wait_for_save();
    assert_eq!(
        saved_name(&path),
        "Before",
        "the file holds what was asked to be saved"
    );
    assert!(
        a.bus.is_dirty(),
        "the edit made during the save is not in the file"
    );
    assert_eq!(a.project().name, "During");
    // Saving again takes it.
    a.save_to_path(&path);
    assert!(!a.bus.is_dirty());
    assert_eq!(saved_name(&path), "During");
}

#[test]
fn a_failed_save_says_so_in_both_languages_and_keeps_the_project_dirty() {
    for (locale, words) in [
        (cantus_ui::i18n::Locale::En, "Could not save"),
        (cantus_ui::i18n::Locale::Es, "No se pudo guardar"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut a = app(dir.path(), song());
        cantus_ui::i18n::set_thread_locale(locale);
        rename(&mut a, "Precious");
        let blocked = dir.path().join("Blocked");
        fs::write(&blocked, b"a file where a folder should be").unwrap();
        let gate = hold(&mut a);
        a.start_save(&blocked.join("precious.cantus"));
        assert!(!a.status.error, "nothing has failed yet");
        drop(gate);
        a.wait_for_save();
        assert!(a.status.error);
        assert!(a.status.text.contains(words), "{}", a.status.text);
        assert!(a.bus.is_dirty());
        assert_eq!(a.path, None, "not pointed at a file it could not write");
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn a_second_save_waits_its_turn_and_takes_the_later_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.cantus");
    let mut a = app(dir.path(), song());
    rename(&mut a, "First");
    let gate = hold(&mut a);
    assert!(a.start_save(&path));
    rename(&mut a, "Second");
    assert!(!a.start_save(&path), "queued behind the one in flight");
    drop(gate);
    a.wait_for_save();
    assert_eq!(saved_name(&path), "Second");
    assert!(!a.bus.is_dirty());
}

#[test]
fn new_open_and_the_close_wait_for_a_save_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    // New: the save lands first, then the project is replaced.
    let path = dir.path().join("a.cantus");
    let mut a = app(dir.path(), song());
    rename(&mut a, "Kept");
    let gate = hold(&mut a);
    a.start_save(&path);
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        drop(gate);
    });
    a.request_new();
    release.join().unwrap();
    assert!(!a.save_in_flight());
    assert_eq!(saved_name(&path), "Kept");
    assert_ne!(a.project().name, "Kept", "New went ahead after the save");
    assert!(
        a.unsaved_prompt.is_none(),
        "the save landed, so nothing was unsaved"
    );

    // Open.
    let other = dir.path().join("other.cantus");
    cantus_project::save(&Project::new("Other"), &other, "t").unwrap();
    let path = dir.path().join("b.cantus");
    let mut a = app(dir.path(), song());
    rename(&mut a, "Mine");
    let gate = hold(&mut a);
    a.start_save(&path);
    drop(gate);
    a.request_open_path(other.clone());
    assert_eq!(saved_name(&path), "Mine");
    assert_eq!(a.project().name, "Other");

    // The close: the session's journal and autosave wait for it; nothing is unsaved.
    let data = dir.path().join("data");
    let path = dir.path().join("c.cantus");
    let mut a = app(&data, song());
    a.start_session(&data);
    rename(&mut a, "Closing");
    let gate = hold(&mut a);
    a.start_save(&path);
    drop(gate);
    a.close_session();
    assert!(!a.save_in_flight());
    assert_eq!(saved_name(&path), "Closing");
    assert_eq!(
        cantus_ui::recovery::orphans(&cantus_ui::recovery::sessions_dir(&data)),
        vec![],
        "a landed save leaves nothing to recover"
    );
}

#[test]
fn save_changes_carries_out_its_errand_only_once_the_save_has_landed() {
    let dir = tempfile::tempdir().unwrap();
    let mine = dir.path().join("mine.cantus");
    let other = dir.path().join("other.cantus");
    cantus_project::save(&Project::new("Other"), &other, "t").unwrap();
    let mut a = app(dir.path(), song());
    a.save_to_path(&mine);
    rename(&mut a, "Mine, edited");
    a.request_open_path(other.clone());
    assert!(matches!(a.unsaved_prompt, Some(Then::OpenPath(_))));
    let gate = hold(&mut a);
    a.answer_unsaved(Answer::Save);
    assert_eq!(
        a.project().name,
        "Mine, edited",
        "not replaced while it is written"
    );
    assert!(a.save_in_flight());
    drop(gate);
    a.wait_for_save();
    assert_eq!(saved_name(&mine), "Mine, edited");
    assert_eq!(a.project().name, "Other");

    // A failed save goes no further.
    let mut a = app(dir.path(), song());
    let blocked = dir.path().join("Blocked");
    fs::write(&blocked, b"not a folder").unwrap();
    a.save_to_path(&blocked.join("x.cantus"));
    assert!(a.status.error);
    a.path = Some(blocked.join("x.cantus"));
    rename(&mut a, "Song, edited");
    a.request_open_path(other);
    assert!(a.unsaved_prompt.is_some());
    a.answer_unsaved(Answer::Save);
    a.wait_for_save();
    assert!(a.status.error);
    assert_eq!(
        a.project().name,
        "Song, edited",
        "the failed save kept the project"
    );
    assert!(a.bus.is_dirty());
}

#[test]
fn a_queued_save_never_writes_a_project_that_replaced_the_one_it_was_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.cantus");
    let mut a = app(dir.path(), song());
    rename(&mut a, "Mine");
    a.save_to_path(&path);
    rename(&mut a, "Mine, edited");
    let gate = hold(&mut a);
    assert!(a.start_save(&path));
    assert!(!a.start_save(&path), "queued behind the one in flight");
    // A DAWproject import replaces the project (its file dialog held the frame while the
    // save was written).
    let bytes = cantus_project::dawproject::export(&Project::new("Imported"), "t").bytes;
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        drop(gate);
    });
    assert!(cantus_ui::views::interop::import_dawproject(
        &mut a,
        &bytes,
        "imported.dawproject"
    ));
    release.join().unwrap();
    a.wait_for_save();
    assert_eq!(a.project().name, "Imported");
    assert_eq!(
        saved_name(&path),
        "Mine, edited",
        "the user's file holds the user's project"
    );
    assert_eq!(a.path, None, "the import is a new, unsaved session");
    assert!(a.bus.is_dirty(), "the import is in no file");

    // Undoing the import replaces the project without waiting: a save queued for the
    // project before is dropped, not written with the one that came back.
    let imp = dir.path().join("imported.cantus");
    a.save_to_path(&imp);
    let gate = hold(&mut a);
    assert!(a.start_save(&imp));
    assert!(!a.start_save(&imp));
    a.undo();
    assert_eq!(a.project().name, "Mine, edited");
    drop(gate);
    a.wait_for_save();
    assert_eq!(
        saved_name(&imp),
        "Imported",
        "the queued save was for the import"
    );
}

#[test]
fn a_second_save_of_an_untitled_project_goes_where_its_first_save_went() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("first.cantus");
    let mut a = app(dir.path(), song());
    rename(&mut a, "Untitled");
    assert_eq!(a.save_target(), None, "nothing chosen yet: Save As asks");
    let gate = hold(&mut a);
    assert!(a.start_save(&path));
    assert_eq!(a.path, None, "bound to the file only once it has landed");
    assert_eq!(
        a.save_target().as_deref(),
        Some(path.as_path()),
        "Save goes where the save in flight goes, without asking again"
    );
    rename(&mut a, "Later");
    a.save();
    drop(gate);
    a.wait_for_save();
    assert_eq!(saved_name(&path), "Later");
    assert!(!a.bus.is_dirty());
    assert_eq!(a.path.as_deref(), Some(path.as_path()));
}

#[test]
fn the_demo_pickers_save_first_closes_it_and_opens_the_demo_once_saved() {
    use cantus_ui::views::demos as picker;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mine.cantus");
    let demo = cantus_ui::i18n::t(cantus_ui::demos::ALL[0].name_key);
    let mut a = app(dir.path(), song());
    a.save_to_path(&path);
    rename(&mut a, "Mine, edited");
    picker::show(&mut a);
    picker::request(&mut a, 0);
    assert!(a.demos.confirm);
    let gate = hold(&mut a);
    picker::save_first(&mut a);
    assert!(a.save_in_flight());
    assert!(
        !a.demos.open,
        "closed: nothing left in it to cancel while the save is written"
    );
    picker::save_first(&mut a);
    assert!(
        a.status.text.contains("Saving"),
        "a second press asks nothing more: {}",
        a.status.text
    );
    assert_eq!(a.project().name, "Mine, edited");
    drop(gate);
    a.wait_for_save();
    assert_eq!(saved_name(&path), "Mine, edited");
    assert_eq!(
        a.project().name,
        demo,
        "the demo opened once the save landed"
    );

    // An edit made while the save is written: the question is asked again rather than
    // the errand dropped without a word (or the edit thrown away).
    let mut a = app(dir.path(), song());
    a.save_to_path(&path);
    rename(&mut a, "Mine, edited");
    picker::show(&mut a);
    picker::request(&mut a, 0);
    let gate = hold(&mut a);
    picker::save_first(&mut a);
    rename(&mut a, "Mine, edited again");
    drop(gate);
    a.wait_for_save();
    assert_eq!(a.project().name, "Mine, edited again");
    assert_eq!(a.unsaved_prompt, Some(Then::Demo(0)), "asked again");
}

#[test]
fn a_recording_pass_that_ends_during_a_save_stays_in_the_journal() {
    use cantus_core::audio::TrackInput;
    const SIZE: egui::Vec2 = egui::Vec2::new(1280.0, 800.0);
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let path = dir.path().join("take.cantus");
    let mut p = Project::new("Take");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    p.tracks.push(t);
    let (engine, mut live) = harness::LiveEngine::start(256);
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let mut a = CantusApp::from_parts(Some(p), engine, false);
    harness::isolate(&mut a, &data);
    a.start_session(&data);
    a.save_to_path(&path);
    a.rec.demo_inputs = Some(vec![0.0]);
    let ctx = harness::ctx();
    let t0 = Instant::now();
    harness::frame(&mut a, &ctx, SIZE, 0.0);
    a.start_recording();
    assert!(
        a.rec.pass.as_ref().is_some_and(|p| p.audio),
        "an audio pass"
    );
    // Until the engine has written some of the take (the frames carry its progress).
    let recorded = |a: &CantusApp| {
        a.rec
            .pass
            .as_ref()
            .and_then(|p| p.takes.first())
            .map_or(0, |t| t.frames)
    };
    while recorded(&a) < 12_000 && t0.elapsed() < Duration::from_secs(30) {
        harness::frame(&mut a, &ctx, SIZE, t0.elapsed().as_secs_f64());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        recorded(&a) >= 12_000,
        "the engine recorded the take: {} frames, {} in, status {:?}, pass {}",
        recorded(&a),
        live.stats
            .input_frames
            .load(std::sync::atomic::Ordering::Relaxed),
        a.status.text,
        a.rec.pass.is_some()
    );
    let passes = |a: &CantusApp| a.session.as_ref().unwrap().journal.passes.clone();
    // Saved while recording: the copy has no take yet.
    let gate = hold(&mut a);
    assert!(a.start_save(&path));
    a.stop_recording();
    let j = passes(&a);
    assert_eq!(j.len(), 1);
    assert!(j[0].finished, "its takes are clips now, in no file");
    drop(gate);
    a.wait_for_save();
    assert!(a.bus.is_dirty(), "the take is not in the file just written");
    assert_eq!(
        passes(&a).len(),
        1,
        "the journal keeps the pass until a file holds its takes"
    );
    let on_disk: cantus_ui::recovery::Journal = serde_json::from_str(
        &fs::read_to_string(a.session.as_ref().unwrap().journal_path()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        on_disk.passes.len(),
        1,
        "and so does the file a crash leaves"
    );
    // The next save holds the take: the journal lets the pass go.
    a.save_to_path(&path);
    assert!(passes(&a).is_empty());
    live.stop();
    a.end_session(false);
}

/// What the frame takes against what the thread does, on the heaviest demo:
/// `cargo test --release -p cantus-ui --test save_async -- --ignored --nocapture`.
#[test]
#[ignore]
fn measure_the_copy_against_the_write() {
    let dir = tempfile::tempdir().unwrap();
    let mut cases: Vec<(String, Project)> = cantus_ui::demos::ALL
        .iter()
        .map(|d| (d.id.to_string(), (d.build)()))
        .collect();
    // A big session: 64 tracks of 4 000 notes each.
    let mut big = Project::new("Big");
    for t in 0..64 {
        let mut track = big.make_track(format!("T{t}"));
        let mut c = big.make_clip(
            "c",
            cantus_core::Ticks::ZERO,
            cantus_core::Ticks::beats(4_000),
        );
        for i in 0..4_000i64 {
            let n = big.make_note(
                cantus_core::Ticks::beats(i),
                cantus_core::Ticks::QUARTER,
                36 + (i % 48) as u8,
                90,
            );
            c.notes.push(n);
        }
        track.clips.push(c);
        big.tracks.push(track);
    }
    cases.push(("big".into(), big));
    for (id, p) in cases {
        let mut a = app(dir.path(), p);
        let path = dir.path().join(format!("{id}.cantus"));
        let n = 5;
        let (mut copy, mut ser, mut all) = (f64::MAX, f64::MAX, f64::MAX);
        for _ in 0..n {
            let t = Instant::now();
            let c = a.plugins.project_for_save(a.bus.project());
            copy = copy.min(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            let text = cantus_project::ProjectFile::wrap(c, "t")
                .unwrap()
                .to_json()
                .unwrap();
            ser = ser.min(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            cantus_project::save(a.project(), &path, "t").unwrap();
            all = all.min(t.elapsed().as_secs_f64() * 1e3);
            assert!(!text.is_empty());
        }
        a.start_save(&path);
        a.wait_for_save();
        println!(
            "{:<8} {:>7} KiB  copy {copy:>7.3} ms  serialise {ser:>7.3} ms  whole save {all:>7.2} ms  frame {:>7.3} ms",
            id,
            fs::metadata(&path).unwrap().len() / 1024,
            a.saving.last_frame_ms,
        );
    }
}
