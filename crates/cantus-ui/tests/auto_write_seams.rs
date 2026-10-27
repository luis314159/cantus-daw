//! Round 31's fix-round review (ADR 0068): a write pass belongs to the project it rode —
//! by every path another project takes its place (Open, a recovery), and through the
//! unsaved question's *Save*.

mod harness;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::views::unsaved::Answer;
use cantus_ui::CantusApp;
use std::path::Path;

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

fn bar(n: i64) -> Ticks {
    Ticks::beats(4 * n)
}

/// One frame at `pos`, playing.
fn frame(a: &mut CantusApp, pos: Ticks) {
    a.engine.set_heard_for_tests(pos, true);
    a.auto_write_frame();
    a.auto_write_settle();
}

/// The master in Latch, armed, its fader ridden while playing: a pass open on its gain.
fn ride(a: &mut CantusApp) {
    assert!(a.exec(Command::SetMasterAutoMode {
        mode: AutoMode::Latch
    }));
    a.set_auto_armed(true);
    frame(a, bar(1));
    a.exec_merging(
        Command::SetMasterGain { gain_db: -9.0 },
        MergeKey::of(&"master-drag"),
    );
    a.end_gesture();
    frame(a, bar(2));
    assert!(a.write_pass_open());
}

/// The frames after another project is in place, stopped.
fn settle_stopped(a: &mut CantusApp) {
    a.engine.set_heard_for_tests(Ticks::ZERO, false);
    a.auto_write_frame();
    a.auto_write_settle();
    a.auto_write_frame();
}

fn automation_clips(p: &Project) -> usize {
    p.tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| c.automation.is_some())
        .count()
}

/// #1: *Open* in the middle of a pass writes nothing into the file opened.
#[test]
fn opening_a_file_ends_the_pass_with_the_old_project() {
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("other.cantus");
    let mut saved = song();
    saved.name = "Other".into();
    saved.master.gain_db = 1.5;
    cantus_project::save(&saved, &other, "t").unwrap();
    let mut a = app(dir.path(), song());
    ride(&mut a);
    a.open(&other);
    settle_stopped(&mut a);
    assert_eq!(a.project().name, "Other");
    assert_eq!(automation_clips(a.project()), 0, "nothing written into it");
    assert_eq!(a.project().master.gain_db, 1.5, "its own gain");
    assert!(!a.bus.is_dirty(), "the file as it was opened");
}

/// #1: restoring a recovered session in the middle of a pass writes nothing into it, nor
/// into the base it is restored over.
#[test]
fn restoring_a_recovery_ends_the_pass_with_the_old_project() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let mut a = app(data, song());
    a.start_session(data);
    a.exec(Command::SetProjectName {
        name: "Unsaved idea".into(),
    });
    a.end_gesture();
    a.close_session();
    let mut b = app(data, song());
    b.start_session(data);
    assert!(b.recovery.is_some(), "offered");
    // The fader starts away from where a new project has it.
    b.exec(Command::SetMasterGain { gain_db: -3.0 });
    b.end_gesture();
    ride(&mut b);
    assert!(b.restore_recovery(0));
    settle_stopped(&mut b);
    assert_eq!(b.project().name, "Unsaved idea");
    assert_eq!(automation_clips(b.project()), 0, "nothing written into it");
    assert!(
        b.bus
            .history()
            .iter()
            .all(|e| e.label_key != "automodes.cmd.write_pass"),
        "no pass committed into the restored history"
    );
    // Nor into the base it was restored over (what undo goes back to).
    b.undo();
    assert_eq!(
        automation_clips(b.project()),
        0,
        "nothing written into the base"
    );
    assert_eq!(b.project().master.gain_db, 0.0, "the base's own gain");
    b.end_session(false);
}

/// #3: *New* at the unsaved question, answered *Save*: the file holds what the pass wrote.
#[test]
fn saving_at_the_question_keeps_the_written_automation() {
    let dir = tempfile::tempdir().unwrap();
    let mine = dir.path().join("mine.cantus");
    let mut a = app(dir.path(), song());
    a.save_to_path(&mine);
    a.wait_for_save();
    ride(&mut a);
    a.request_new();
    assert!(a.unsaved_prompt.is_some(), "the ride is unsaved");
    a.answer_unsaved(Answer::Save);
    a.wait_for_save();
    let saved = cantus_project::load(&mine, false).unwrap().project;
    assert_eq!(automation_clips(&saved), 1, "the ride was saved");
}

/// #1, the other arm: a recovery with no autosave (recorded takes over a saved project)
/// opens the project's file — the pass is not committed into it.
#[test]
fn restoring_takes_over_a_saved_project_ends_the_pass_with_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let file = data.join("saved.cantus");
    let mut saved = song();
    saved.name = "Saved".into();
    cantus_project::save(&saved, &file, "t").unwrap();
    let mut a = app(data, song());
    a.start_session(data);
    a.exec(Command::SetProjectName {
        name: "Unsaved idea".into(),
    });
    a.end_gesture();
    a.close_session();
    let mut b = app(data, song());
    b.start_session(data);
    {
        let item = &mut b.recovery.as_mut().expect("offered").items[0];
        item.autosave = None;
        item.project_path = Some(file.clone());
    }
    ride(&mut b);
    assert!(b.restore_recovery(0));
    settle_stopped(&mut b);
    assert_eq!(b.project().name, "Saved");
    assert_eq!(automation_clips(b.project()), 0, "nothing written into it");
    b.end_session(false);
}
