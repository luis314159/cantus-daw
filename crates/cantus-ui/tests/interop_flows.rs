//! Moving work between Cantus and other DAWs, through the application (ADR 0021): an
//! import is one undo step, stems are named the way the desk names its strips, the export
//! runs off the interface thread, every report line has words in both languages, and the
//! dialogs render.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_project::{dawproject, midi};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::views::{self, interop};
use cantus_ui::{starter_project, CantusApp};

/// The interface language is global and some tests here switch it: they take turns.
static LANGUAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn language() -> std::sync::MutexGuard<'static, ()> {
    let guard = LANGUAGE.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_thread_locale(Locale::En);
    guard
}

fn app(project: Project) -> CantusApp {
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// A song with a lead and a kit, as a MIDI file.
fn song_mid() -> Vec<u8> {
    let mut p = Project::new("Song");
    p.tempo_bpm = 100.0;
    let mut lead = p.make_track("Lead");
    let mut c = p.make_clip("A", Ticks::ZERO, Ticks::beats(4));
    for i in 0..4 {
        c.notes
            .push(p.make_note(Ticks::beats(i), Ticks::EIGHTH, 60 + i as u8, 90));
    }
    lead.clips.push(c);
    p.tracks.push(lead);
    let mut kit = p.make_track("Beat");
    kit.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let mut c = p.make_clip("B", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::SIXTEENTH, 36, 120));
    c.notes
        .push(p.make_note(Ticks::beats(1), Ticks::SIXTEENTH, 49, 100));
    kit.clips.push(c);
    p.tracks.push(kit);
    midi::export(&p, &midi::ExportOptions::default()).unwrap()
}

fn same_but_ids(a: &Project, b: &Project) -> bool {
    let mut b = b.clone();
    b.next_id = a.next_id;
    *a == b
}

#[test]
fn a_midi_import_lands_where_it_was_dropped_and_is_one_undo_step() {
    let _lang = language();
    let mut a = app(starter_project());
    let before = a.project().clone();
    let bytes = song_mid();
    assert!(interop::import_midi(
        &mut a,
        &bytes,
        "song.mid",
        false,
        false,
        Ticks::beats(8),
        Some(0)
    ));
    let p = a.project();
    assert_eq!(p.tracks.len(), 3);
    assert_eq!(p.tracks[0].name, "Lead");
    assert_eq!(p.tracks[1].name, "Beat");
    assert_eq!(
        p.tracks[2].name, before.tracks[0].name,
        "below the new ones"
    );
    assert_eq!(p.tracks[0].clips[0].start, Ticks::beats(8));
    assert_eq!(p.tempo_bpm, before.tempo_bpm, "the tempo was not asked for");
    let kit = p.tracks[1]
        .instrument
        .pulsus()
        .expect("channel 10 is a kit");
    assert!(kit.pads.iter().any(|pad| pad.pitch == 49));
    assert_eq!(a.selected_track, Some(a.project().tracks[0].id));
    assert!(a.status.text.contains("tracks: 2"), "{}", a.status.text);
    a.undo();
    assert!(
        same_but_ids(&before, a.project()),
        "one undo takes the whole import back"
    );
}

#[test]
fn a_dawproject_import_replaces_the_project_as_one_undo_step() {
    let _lang = language();
    let mut source = starter_project();
    source.name = "From elsewhere".into();
    source.tempo_bpm = 87.0;
    let file = dawproject::export(&source, "0.1.0");
    let mut a = app(Project::default());
    let before = a.project().clone();
    assert!(interop::import_dawproject(
        &mut a,
        &file.bytes,
        "x.dawproject"
    ));
    assert_eq!(a.project().name, "From elsewhere");
    assert_eq!(a.project().tempo_bpm, 87.0);
    assert_eq!(a.project().tracks.len(), source.tracks.len());
    assert!(a.path.is_none(), "an import is not the file it came from");
    assert!(a.interop.report.is_some(), "the report is shown");
    a.undo();
    assert_eq!(*a.project(), before);
}

#[test]
fn stems_are_named_as_the_desk_names_them() {
    let _lang = language();
    let mut p = Project::default();
    let lead = p.make_track("Lead");
    p.tracks.push(lead);
    let bass = p.make_track("Bass");
    let strip = bass.strip.unwrap();
    p.tracks.push(bass);
    p.strip_mut(strip).unwrap().name = "Low end".into();
    let mut pad = p.make_track("Pad");
    pad.strip = None;
    p.tracks.push(pad);
    let names = |specs: Vec<cantus_engine::StemSpec>| -> Vec<String> {
        specs.into_iter().map(|s| s.file_name).collect()
    };
    let strips = names(interop::stem_specs(&p, false, true));
    assert_eq!(
        strips[..5],
        [
            "00 Master.wav",
            "00 Direct to master.wav",
            "01 Lead.wav",
            "02 Low end.wav",
            "03.wav"
        ]
    );
    assert_eq!(strips.len(), 2 + DEFAULT_STRIPS);
    assert_eq!(
        names(interop::stem_specs(&p, true, false)),
        ["01 Lead.wav", "02 Bass.wav", "03 Pad.wav"]
    );
    i18n::set_thread_locale(Locale::Es);
    assert_eq!(
        names(interop::stem_specs(&p, false, true))[..2],
        ["00 Máster.wav", "00 Directo al máster.wav"]
    );
    i18n::set_thread_locale(Locale::En);
}

#[test]
fn every_report_line_has_words_in_both_languages() {
    let _lang = language();
    for locale in Locale::ALL {
        i18n::set_thread_locale(locale);
        for key in cantus_project::report::KEYS {
            assert_ne!(i18n::t(key), *key, "{locale:?} has no words for {key}");
        }
    }
    i18n::set_thread_locale(Locale::En);
    let mut report = cantus_project::Report::default();
    report.dropped(
        "interop.report.d_plugin",
        vec![("name", "Serum".into()), ("track", "Lead".into())],
    );
    assert_eq!(
        interop::report_line(&report.items[0]),
        "The plug-in Serum on Lead: plug-ins are not hosted yet"
    );
}

#[test]
fn the_dialogs_and_the_report_render() {
    let _lang = language();
    let mut a = app(starter_project());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.mid");
    std::fs::write(&path, song_mid()).unwrap();
    interop::open_midi_import(&mut a, &path);
    assert!(a.interop.midi_import.is_some());
    a.interop.midi_export = Some(interop::MidiExportDialog {
        scope: interop::MidiScope::Clip,
        ppq: 480,
        played: true,
    });
    let imported = dawproject::import(&dawproject::export(&starter_project(), "0").bytes).unwrap();
    a.interop.report = Some(interop::ReportWindow {
        title: "Report".into(),
        report: imported.report,
    });
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    for locale in Locale::ALL {
        i18n::set_thread_locale(locale);
        for _ in 0..2 {
            let mut out = ctx.run_ui(input.clone(), |ui| {
                egui::Panel::top(egui::Id::new("t"))
                    .show(ui, |ui| views::transport::show(&mut a, ui));
                egui::CentralPanel::default().show(ui, |ui| views::arrangement::show(&mut a, ui));
                interop::frame(&mut a, ui.ctx());
            });
            out.textures_delta.clear();
        }
    }
    i18n::set_thread_locale(Locale::En);
    // Nothing was clicked, so everything is still open.
    assert!(a.interop.midi_import.is_some() && a.interop.midi_export.is_some());
}

#[test]
fn a_file_dropped_on_the_arrangement_lands_under_the_pointer() {
    let _lang = language();
    let mut a = app(starter_project());
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    // One frame so the arrangement knows where it is.
    let mut out = ctx.run_ui(input.clone(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::arrangement::show(&mut a, ui));
    });
    out.textures_delta.clear();
    let g = a.arrangement.geometry().expect("drawn");
    let x = g.x(9.0);
    let (at, index) = interop::drop_target(&a, Some(egui::pos2(x, g.row_y(0))));
    assert_eq!(index, Some(1), "below the row it was dropped on");
    if a.arrangement.snap {
        let grid = a.arrangement.grid;
        assert_eq!(at.0 % grid.0, 0, "on the grid");
        assert!(at <= Ticks::beats(9) && Ticks::beats(9) - at < grid);
    } else {
        assert_eq!(at, Ticks::beats(9));
    }
    // Off the arrangement: the playhead's bar, after the last track.
    let (_, index) = interop::drop_target(&a, None);
    assert_eq!(index, None);
}
