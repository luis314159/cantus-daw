//! Review 6 of the tempo track's interface (ADR 0060): undo or redo of an imported song
//! waits while *any* pass records, and says so in its own words; a DAWproject read in the
//! background while a take records opens when the take ends; a MIDI file imported during
//! an audio take brings its notes at the song's tempo; the first tap during an audio take
//! says the hold; the ruler's editor is the *time signature* at its bar.

mod support;

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::{starter_project, views, CantusApp};
use support::app;

fn audio_rig(dir: &std::path::Path) -> (CantusApp, Processor, rtrb::Producer<f32>) {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let (h, mut proc) = EngineHandle::manual(48_000.0);
    let (ring, consumer) = rtrb::RingBuffer::<f32>::new(96_000);
    proc.attach_input(consumer, 1);
    let mut a = CantusApp::from_parts(Some(starter_project()), h, false);
    a.rec.demo_inputs = Some(vec![0.0]);
    a.path = Some(dir.join("song.cantus"));
    a.add_audio_track();
    a.sync_engine();
    (a, proc, ring)
}

fn elsewhere() -> Vec<u8> {
    let mut other = starter_project();
    other.name = "Elsewhere".into();
    other.tempo_bpm = 87.0;
    cantus_project::dawproject::export(&other, "0.1.0").bytes
}

/// Findings 1, 11, 8, 12: an imported song's undo and redo wait while a pass records —
/// notes only or audio — said in their own words, not the tempo's.
#[test]
fn undo_of_an_import_waits_while_any_pass_records() {
    // A notes-only pass.
    let mut a = app(starter_project());
    assert!(views::interop::import_dawproject(
        &mut a,
        &elsewhere(),
        "x.dawproject"
    ));
    a.selected_track = a.project().tracks.first().map(|t| t.id);
    a.start_recording();
    assert!(a.is_recording() && !a.audio_take_running(), "a notes pass");
    a.undo();
    assert_eq!(
        a.project().name,
        "Elsewhere",
        "the song stays under the pass"
    );
    assert!(a.status.text.contains("song"), "said: {}", a.status.text);
    assert!(!a.status.text.contains("tempo"), "not the tempo's words");
    a.stop_recording();
    a.undo();
    assert_ne!(a.project().name, "Elsewhere", "after the pass");
    // An audio take, and a redo.
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    assert!(views::interop::import_dawproject(
        &mut a,
        &elsewhere(),
        "x.dawproject"
    ));
    a.undo();
    let before = a.project().name.clone();
    a.start_recording();
    assert!(a.audio_take_running());
    a.redo();
    assert_eq!(a.project().name, before, "redo waits too");
    assert!(a.status.text.contains("song"), "said: {}", a.status.text);
}

/// Finding 7: a DAWproject read in the background while a take records opens when the
/// take ends — the take is not cut short.
#[test]
fn a_background_import_opens_when_the_take_ends() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    let path = dir.path().join("x.dawproject");
    std::fs::write(&path, elsewhere()).unwrap();
    a.start_recording();
    assert!(views::interop::import_dawproject_from(&mut a, &path));
    // The read finishes on its thread, then waits for the take (warp's review 3 holds an
    // import that is read while a pass records): it never lands while the take goes on.
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(3) {
        views::interop::poll_import(&mut a, None);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(a.audio_take_running(), "the take goes on");
    assert_ne!(a.project().name, "Elsewhere");
    a.stop_recording();
    views::interop::wait_import(&mut a);
    assert_eq!(a.project().name, "Elsewhere", "opened after the take");
}

/// Finding 13: a MIDI file imported during an audio take with the tempo asked for brings
/// its notes at the song's tempo (the tempo holds), not nothing.
#[test]
fn a_midi_import_during_an_audio_take_brings_its_notes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    let mut file = starter_project();
    file.tempo_bpm = 97.0;
    let bytes = cantus_project::midi::export(&file, &Default::default()).unwrap();
    let bpm = a.project().tempo_bpm;
    let tracks = a.project().tracks.len();
    a.start_recording();
    assert!(views::interop::import_midi(
        &mut a,
        &bytes,
        "groove.mid",
        false,
        true,
        Ticks::ZERO,
        None
    ));
    assert!(a.project().tracks.len() > tracks, "its notes came in");
    assert_eq!(a.project().tempo_bpm, bpm, "the tempo held");
}

/// Finding 16: the first tap of a run during an audio take says the tempo holds, not
/// "tap again".
#[test]
fn the_first_tap_during_an_audio_take_says_the_hold() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    a.start_recording();
    views::transport::tap_at(&mut a, 10.0);
    assert!(a.status.text.contains("audio"), "said: {}", a.status.text);
}

/// Finding 15: the ruler's editor is titled as the top bar's — the time signature — at
/// its bar.
#[test]
fn the_rulers_editor_is_the_time_signature_at_its_bar() {
    assert_eq!(
        cantus_ui::i18n::tf("tempo.metre_at_bar", &[("bar", &12)]),
        "Time signature at bar 12"
    );
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    let es = cantus_ui::i18n::tf("tempo.metre_at_bar", &[("bar", &12)]);
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert!(es.starts_with("Indicación de compás"), "{es}");
}

/// Findings 9, 14: a report line of one says it in the singular.
#[test]
fn one_kept_or_left_out_is_said_in_the_singular() {
    use cantus_project::report::{ReportItem, Severity};
    // English words, whatever the machine's language (round 31: it read the process's).
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for (key, want) in [
        ("tempo.report.tempo_kept", "One tempo change kept"),
        ("tempo.report.metres_kept", "One metre change kept"),
        ("tempo.report.markers_kept", "One marker kept"),
    ] {
        let item = ReportItem {
            severity: Severity::Info,
            key,
            args: vec![("n", "1".into())],
        };
        assert!(
            views::interop::report_line(&item).starts_with(want),
            "{key}"
        );
    }
    let far = ReportItem {
        severity: Severity::Dropped,
        key: "tempo.report.too_far",
        args: vec![("n", "1".into())],
    };
    assert!(!views::interop::report_line(&far).starts_with('1'));
}

/// Final review (finding A2): the ruler's editor and the top bar's are one editor with one
/// name, in both languages: the ruler's title is the top bar's, at its bar.
#[test]
fn the_rulers_editor_and_the_top_bars_share_their_name() {
    use cantus_ui::i18n::{set_thread_locale, t, tf, Locale};
    for lang in [Locale::En, Locale::Es] {
        set_thread_locale(lang);
        let top = t("topbar.metre");
        let ruler = tf("tempo.metre_at_bar", &[("bar", &12)]);
        assert!(ruler.starts_with(&top), "{ruler} / {top}");
    }
    set_thread_locale(Locale::En);
}
