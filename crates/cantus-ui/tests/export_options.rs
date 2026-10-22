//! Every control of the export dialog, clicked as a musician clicks it (round 23,
//! BACKLOG §1): the gesture goes through egui's real input, the dialog publishes where
//! it drew each control (`export_dialog::geo`), and the test checks that the choice
//! landed in the dialog's settings. `export_files.rs` then checks that each choice
//! reaches the file.

use cantus_core::Ticks;
use cantus_engine::EngineHandle;
use cantus_export::{Channels, Depth, Dither, Format, Mp3Mode, Rate, Scope};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::views::{export, export_dialog};
use cantus_ui::{starter_project, CantusApp};
use egui::{Pos2, Rect};

static LANGUAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn language() -> std::sync::MutexGuard<'static, ()> {
    let guard = LANGUAGE.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_locale(Locale::En);
    guard
}

const SCREEN: egui::Vec2 = egui::vec2(1440.0, 900.0);

struct Ui {
    a: CantusApp,
    ctx: egui::Context,
    _dir: tempfile::TempDir,
}

impl Ui {
    fn open() -> Ui {
        let dir = tempfile::tempdir().unwrap();
        let mut a = CantusApp::from_parts(Some(starter_project()), EngineHandle::dummy(), false);
        a.settings_path = dir.path().join("settings.json");
        a.layouts_path = dir.path().join("layouts.json");
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        export::open(&mut a, None);
        let mut ui = Ui { a, ctx, _dir: dir };
        ui.run(vec![]);
        ui.run(vec![]);
        ui
    }

    fn run(&mut self, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..Default::default()
        };
        let a = &mut self.a;
        self.ctx
            .run_ui(input, |ui| a.frame(ui))
            .textures_delta
            .clear();
    }

    fn click_at(&mut self, at: Pos2) {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.run(vec![egui::Event::PointerMoved(at)]);
        self.run(vec![button(true)]);
        self.run(vec![button(false)]);
        // Two more frames: what the click opened is drawn, then drawn where it stays.
        self.run(vec![]);
        self.run(vec![]);
    }

    fn rect(&self, name: &str) -> Rect {
        export_dialog::geo(&self.ctx, name).unwrap_or_else(|| {
            panic!(
                "{name} was not drawn; drawn: {:?}",
                export_dialog::geo_names(&self.ctx)
            )
        })
    }

    fn click(&mut self, name: &str) {
        let r = self.rect(name);
        self.click_at(r.center());
    }

    /// Open the list of the menu field `field` and click its item `i`.
    fn choose(&mut self, field: &str, i: usize) {
        self.click(field);
        self.click(&format!("list#{i}"));
    }

    fn d(&self) -> &cantus_export::ExportSettings {
        self.a.export.dialog.as_ref().expect("the dialog is open")
    }

    fn d_mut(&mut self) -> &mut cantus_export::ExportSettings {
        self.a.export.dialog.as_mut().expect("the dialog is open")
    }
}

#[test]
fn the_format_segments_choose_the_format() {
    let _l = language();
    let mut u = Ui::open();
    for (i, f) in Format::ALL.into_iter().enumerate() {
        u.click(&format!("export_audio.row.format#{i}"));
        assert_eq!(u.d().format, f, "segment {i}");
    }
}

#[test]
fn the_tail_list_opens_over_the_dialog_and_chooses() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().tail_seconds = 2.0;
    u.run(vec![]);
    // 0, 0.5, 1, 2, 4, 8, 15 s: item 5 is 8 s.
    u.choose("export_audio.row.tail", 5);
    assert_eq!(u.d().tail_seconds, 8.0);
    u.choose("export_audio.row.tail", 0);
    assert_eq!(u.d().tail_seconds, 0.0);
}

#[test]
fn the_wav_depth_and_dither_segments_choose() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().format = Format::Wav;
    u.run(vec![]);
    for (i, depth) in [Depth::Int16, Depth::Int24, Depth::Float32]
        .into_iter()
        .enumerate()
    {
        u.click(&format!("export_audio.row.depth#{i}"));
        assert_eq!(u.d().depth, depth);
    }
    u.click("export_audio.row.depth#0");
    for (i, m) in [Dither::Off, Dither::Tpdf, Dither::Shaped]
        .into_iter()
        .enumerate()
    {
        u.click(&format!("export_audio.row.dither#{i}"));
        assert_eq!(u.d().dither, m);
    }
}

#[test]
fn the_flac_depth_segments_choose() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().format = Format::Flac;
    u.run(vec![]);
    u.click("export_audio.row.depth#0");
    assert_eq!(u.d().flac_depth(), Depth::Int16);
    u.click("export_audio.row.depth#1");
    assert_eq!(u.d().flac_depth(), Depth::Int24);
}

#[test]
fn the_mp3_mode_and_quality_choose() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().format = Format::Mp3;
    u.d_mut().mp3 = Mp3Mode::Cbr(320);
    u.run(vec![]);
    // 128, 160, 192, 224, 256, 320.
    u.choose("export_audio.row.quality", 2);
    assert_eq!(u.d().mp3, Mp3Mode::Cbr(192));
    u.click("export_audio.row.mode#1");
    assert!(matches!(u.d().mp3, Mp3Mode::Vbr(_)));
    // V0, V2, V4, V6.
    u.choose("export_audio.row.quality", 2);
    assert_eq!(u.d().mp3, Mp3Mode::Vbr(4));
    u.click("export_audio.row.mode#0");
    assert!(matches!(u.d().mp3, Mp3Mode::Cbr(_)));
}

#[test]
fn the_ogg_quality_list_chooses() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().format = Format::Ogg;
    u.run(vec![]);
    // 3, 4, 5, 6, 8, 10.
    u.choose("export_audio.row.quality", 4);
    assert_eq!(u.d().ogg_quality, 8);
}

#[test]
fn the_rate_list_chooses() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().format = Format::Wav;
    u.run(vec![]);
    // Session, 44.1, 48, 88.2, 96, 192.
    u.choose("export_audio.row.rate", 4);
    assert_eq!(u.d().rate, Rate::Hz(96_000));
    u.choose("export_audio.row.rate", 0);
    assert_eq!(u.d().rate, Rate::Session);
}

#[test]
fn the_channel_segments_choose() {
    let _l = language();
    let mut u = Ui::open();
    u.click("export_audio.row.channels#1");
    assert_eq!(u.d().channels, Channels::Mono);
    u.click("export_audio.row.channels#0");
    assert_eq!(u.d().channels, Channels::Stereo);
}

#[test]
fn the_loudness_target_ceiling_and_limit_choose() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().loudness.target_lufs = None;
    u.run(vec![]);
    // Off, −9, −11, −14, −16, −19, −23.
    u.choose("export_audio.row.target", 3);
    assert_eq!(u.d().loudness.target_lufs, Some(-14.0));
    // −0.1, −0.3, −1, −2.
    u.choose("export_audio.row.ceiling", 1);
    assert_eq!(u.d().loudness.ceiling_dbtp, -0.3);
    let limit = u.d().loudness.limit;
    u.click("loudness.limit");
    assert_eq!(u.d().loudness.limit, !limit);
    u.choose("export_audio.row.target", 0);
    assert_eq!(u.d().loudness.target_lufs, None);
}

#[test]
fn the_scope_and_stems_choices_choose() {
    let _l = language();
    let mut u = Ui::open();
    // Song, Selection, Loop, Markers (round 29: refused without markers), Stems.
    let before = u.d().scope;
    u.click("export_audio.row.what#3");
    // Review 2 (finding 23): refused — the scope stays, the note asks for markers.
    assert_eq!(u.d().scope, before, "no markers to export between");
    assert!(!matches!(u.d().scope, Scope::Markers { .. }));
    assert!(matches!(u.a.export.asked, Some(Scope::Markers { .. })));
    u.click("export_audio.row.what#4");
    assert_eq!(u.d().scope, Scope::Stems);
    u.click("export_audio.row.stems#1");
    assert!(u.d().stems.tracks);
    u.click("export_audio.row.stems#0");
    assert!(!u.d().stems.tracks);
    let (m, s) = (u.d().stems.master, u.d().stems.keep_silent);
    u.click("stems.master");
    assert_eq!(u.d().stems.master, !m);
    u.click("stems.keep_silent");
    assert_eq!(u.d().stems.keep_silent, !s);
    u.click("export_audio.row.what#0");
    assert_eq!(u.d().scope, Scope::Song);
}

#[test]
fn the_presets_apply() {
    let _l = language();
    let mut u = Ui::open();
    for (i, p) in cantus_export::Preset::ALL.into_iter().enumerate() {
        u.click(&format!("preset#{i}"));
        assert_eq!(u.d().preset(), Some(p), "preset {i}");
    }
}

#[test]
fn escape_closes_an_open_list_before_the_dialog() {
    let _l = language();
    let mut u = Ui::open();
    u.click("export_audio.row.tail");
    assert!(
        export_dialog::geo(&u.ctx, "list#0").is_some(),
        "the list is open"
    );
    u.run(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    u.run(vec![]);
    assert!(
        u.a.export.dialog.is_some(),
        "Esc closed the list, not the dialog"
    );
    assert!(
        export_dialog::geo(&u.ctx, "list#0").is_none(),
        "and the list is closed"
    );
    // The next Esc, with no list open, closes the dialog.
    u.run(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    u.run(vec![]);
    assert!(
        u.a.export.dialog.is_none(),
        "the second Esc closed the dialog"
    );
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }]
}

#[test]
fn a_range_the_project_lacks_stays_unchosen_and_says_why() {
    let _l = language();
    let mut u = Ui::open();
    u.a.time_selection = None;
    u.d_mut().scope = Scope::Song;
    u.run(vec![]);
    u.click("export_audio.row.what#1");
    assert_eq!(
        u.d().scope,
        Scope::Song,
        "no selection: Selection is not chosen"
    );
    assert_eq!(u.a.export.asked, Some(Scope::Selection));
    // The note stays on the following frames (it is not a one-frame flicker).
    for _ in 0..5 {
        u.run(vec![]);
    }
    assert_eq!(u.a.export.asked, Some(Scope::Selection));
    // With a selection, the same click chooses it.
    u.a.set_time_selection(cantus_core::Ticks::ZERO, cantus_core::Ticks::beats(4));
    u.click("export_audio.row.what#1");
    assert_eq!(u.d().scope, Scope::Selection);
    assert_eq!(u.a.export.asked, None);
}

#[test]
fn the_selection_names_its_last_bar_not_the_next() {
    let _l = language();
    let mut u = Ui::open();
    // Bars 1–4 exactly (the end is exclusive: tick 0 of bar 5).
    let bar = cantus_core::Ticks::beats(4);
    u.a.set_time_selection(cantus_core::Ticks::ZERO, cantus_core::Ticks(bar.0 * 4));
    u.d_mut().scope = Scope::Selection;
    u.run(vec![]);
    let text = cantus_ui::i18n::tf(
        "export_audio.selection",
        &[("from", &1), ("to", &4), ("seconds", &"8.0")],
    );
    assert_eq!(
        cantus_ui::views::export_rows::range_note(&u.a, Scope::Selection),
        Some(text)
    );
}

#[test]
fn the_mp3_modes_keep_their_own_choice() {
    let _l = language();
    let mut u = Ui::open();
    u.d_mut().format = Format::Mp3;
    u.d_mut().mp3 = Mp3Mode::Cbr(192);
    u.run(vec![]);
    u.click("export_audio.row.mode#1");
    u.choose("export_audio.row.quality", 2);
    assert_eq!(u.d().mp3, Mp3Mode::Vbr(4));
    u.click("export_audio.row.mode#0");
    assert_eq!(u.d().mp3, Mp3Mode::Cbr(192), "constant comes back at 192");
    u.click("export_audio.row.mode#1");
    assert_eq!(u.d().mp3, Mp3Mode::Vbr(4), "variable comes back at V4");
}

#[test]
fn the_window_shortcuts_stay_out_while_the_dialog_is_open() {
    let _l = language();
    let mut u = Ui::open();
    // Esc closes the dialog and nothing else: the time selection stays.
    let sel = (cantus_core::Ticks::ZERO, cantus_core::Ticks::beats(4));
    u.a.set_time_selection(sel.0, sel.1);
    u.run(key(egui::Key::Escape, Default::default()));
    u.run(vec![]);
    assert!(u.a.export.dialog.is_none(), "Esc closed the dialog");
    assert_eq!(u.a.time_selection, Some(sel), "and kept the time selection");
    // Ctrl+E while it is open keeps the choices being made.
    u.a.export_dialog();
    u.d_mut().format = Format::Flac;
    u.run(key(egui::Key::E, egui::Modifiers::COMMAND));
    u.run(vec![]);
    assert_eq!(
        u.d().format,
        Format::Flac,
        "Ctrl+E did not reset the dialog"
    );
    // Delete and undo do not reach the project behind it — each checked on its own, and
    // the same Delete shown to work with the dialog closed, so the test can fail.
    let (t, c) = (
        u.a.project().tracks[0].id,
        u.a.project().tracks[0].clips[0].id,
    );
    u.a.select_clip(t, c);
    u.a.focus_arrangement();
    let revision = u.a.bus.revision();
    u.run(key(egui::Key::Delete, Default::default()));
    u.run(vec![]);
    assert_eq!(u.a.bus.revision(), revision, "Delete changed nothing");
    u.run(key(egui::Key::Z, egui::Modifiers::COMMAND));
    u.run(vec![]);
    assert_eq!(u.a.bus.revision(), revision, "Ctrl+Z changed nothing");
    assert!(u.a.export.dialog.is_some());
    export::close(&mut u.a);
    u.a.select_clip(t, c);
    u.a.focus_arrangement();
    u.run(key(egui::Key::Delete, Default::default()));
    u.run(vec![]);
    assert_ne!(
        u.a.bus.revision(),
        revision,
        "with the dialog closed, Delete deletes"
    );
}

#[test]
fn enter_in_a_tag_field_confirms_the_tag_and_does_not_export() {
    let _l = language();
    let mut u = Ui::open();
    u.click("export_audio.tag.artist");
    u.run(vec![egui::Event::Text("Coro".into())]);
    u.run(key(egui::Key::Enter, Default::default()));
    u.run(vec![]);
    assert_eq!(u.d().tags.artist, "Coro");
    assert_eq!(
        u.a.export.enter_pressed, 0,
        "the Enter that confirmed the tag did not export"
    );
    assert!(u.a.export.dialog.is_some());
}

/// Review 2 (finding 23): with two markers the segment chooses *between markers*, the
/// markers around the playhead.
#[test]
fn the_markers_scope_takes_the_markers_around_the_playhead() {
    let _l = language();
    let mut u = Ui::open();
    let a = u.a.add_marker(Ticks::beats(4)).unwrap();
    let b = u.a.add_marker(Ticks::beats(12)).unwrap();
    u.a.seek(Ticks::beats(6));
    u.click("export_audio.row.what#3");
    assert_eq!(u.d().scope, Scope::Markers { from: a.0, to: b.0 });
    assert_eq!(u.a.export.asked, None);
}
