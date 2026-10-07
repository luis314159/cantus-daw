//! What the beta manual's author found while writing against the real app (round 22,
//! fixed by *soak*): New and Open ask before unsaved work goes; a new audio track comes
//! armed and monitoring; a count-in clicks whatever the metronome switch says. (The fourth
//! — a recovery prompt at start-up — is `crash_recovery.rs` and `recovery_flows.rs`.)

use cantus_core::audio::MonitorMode;
use cantus_core::{Command, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::keymap::Action;
use cantus_ui::views::unsaved::{Answer, Then};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

fn song() -> Project {
    let mut p = Project::new("Song");
    let t = p.make_track("Keys");
    p.tracks.push(t);
    p
}

fn edit(a: &mut CantusApp, name: &str) {
    a.exec(Command::SetProjectName { name: name.into() });
    a.end_gesture();
}

#[test]
fn new_and_open_ask_before_unsaved_work_goes() {
    // Nothing unsaved: New is immediate.
    let mut a = app(song());
    a.run_action(Action::NewProject);
    assert!(a.unsaved_prompt.is_none());
    assert_ne!(a.project().name, "Song");

    // Unsaved: New asks; Cancel keeps everything.
    let mut a = app(song());
    edit(&mut a, "Precious");
    a.run_action(Action::NewProject);
    assert_eq!(a.unsaved_prompt, Some(Then::New));
    assert_eq!(a.project().name, "Precious", "nothing replaced while asking");
    a.answer_unsaved(Answer::Cancel);
    assert!(a.unsaved_prompt.is_none());
    assert_eq!(a.project().name, "Precious");
    assert!(a.bus.is_dirty());
    // Don't save: replaced.
    a.request_new();
    a.answer_unsaved(Answer::DontSave);
    assert_ne!(a.project().name, "Precious");

    // Open a file over unsaved work: Save writes it first, then opens.
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("other.cantus");
    let mut o = Project::new("Other");
    o.tempo_bpm = 99.0;
    cantus_project::save(&o, &other, "t").unwrap();
    let mine = dir.path().join("mine.cantus");
    let mut a = app(song());
    a.save_to_path(&mine);
    edit(&mut a, "Mine, edited");
    a.request_open_path(other.clone());
    assert!(matches!(a.unsaved_prompt, Some(Then::OpenPath(_))));
    a.answer_unsaved(Answer::Save);
    assert_eq!(
        cantus_project::load(&mine, true).unwrap().project.name,
        "Mine, edited",
        "saved before it was closed"
    );
    assert_eq!(a.project().name, "Other");
    assert_eq!(a.path.as_deref(), Some(other.as_path()));

    // In Spanish too.
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    assert_eq!(cantus_ui::i18n::t("unsaved.dont_save"), "No guardar");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn a_new_audio_track_is_armed_and_monitoring() {
    let mut a = app(song());
    a.rec.demo_inputs = Some(vec![0.0]);
    a.add_audio_track();
    let id = a.selected_track.unwrap();
    let t = a.project().track(id).unwrap();
    assert!(t.armed, "ready to record");
    assert_eq!(t.input.as_ref().unwrap().monitor, MonitorMode::Auto);
    // One undo step takes it away.
    a.undo();
    assert!(a.project().track(id).is_none());
}

/// The peak of `seconds` of output, rendered in blocks as a callback would.
fn peak(proc: &mut cantus_engine::Processor, a: &mut CantusApp, seconds: f64) -> f32 {
    let mut out = vec![0f32; 480 * 2];
    let mut p = 0f32;
    for _ in 0..(seconds * 48_000.0 / 480.0) as usize {
        proc.process(&mut out, 2);
        a.engine.poll();
        a.sync_engine();
        p = out.iter().fold(p, |m, v| m.max(v.abs()));
    }
    p
}

#[test]
fn a_count_in_clicks_with_the_metronome_off() {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    // One silent instrument track (no clips), the metronome off, a bar of count-in.
    let mut p = song();
    p.tempo_bpm = 120.0;
    p.metronome = false;
    p.recording.count_in_bars = 1;
    let (h, mut proc) = EngineHandle::manual(48_000.0);
    let mut a = CantusApp::from_parts(Some(p), h, false);
    a.select_track(a.project().tracks[0].id);
    a.seek(Ticks::beats(4));
    a.sync_engine();
    peak(&mut proc, &mut a, 0.05);
    a.start_recording();
    // The bar of 4/4 at 120 BPM is 2 s: it clicks, then the take is silent.
    let counting = peak(&mut proc, &mut a, 1.9);
    let after = peak(&mut proc, &mut a, 1.5);
    assert!(counting > 0.01, "the count-in clicks ({counting})");
    assert!(after < 1e-4, "and stops when the take begins ({after})");
    a.stop_recording();
    assert!(!a.project().metronome, "the project's switch is untouched");
    assert_eq!(
        a.project().recording.click,
        cantus_core::ClickMode::Always,
        "and so is its click mode"
    );
    assert!(!a.rec.count_in_click);
}
