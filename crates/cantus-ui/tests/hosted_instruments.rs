//! Hosted instruments in the application (ADR 0028): the picker's `Hosted` choice puts a
//! CLAP instrument on a track in one undo step, the inspector shows its card, the track
//! opens the plug-in's own window, and a project whose instrument is missing opens, says
//! so and saves its record back untouched.

mod support;
use cantus_core::*;
use cantus_host::{Backend, DbEntry, PluginInfo, ScanStatus};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use support::*;

/// The test instrument, built from source where the host's tests build it.
fn test_synth() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let file = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap_synth.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap_synth.dylib"
    } else {
        "libcantus_test_clap_synth.so"
    });
    // As the host's tests: with CANTUS_TEST_PLUGINS_PREBUILT, a built one is used as it is.
    if std::env::var_os("CANTUS_TEST_PLUGINS_PREBUILT").is_some() && file.is_file() {
        return file;
    }
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap-synth", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    file
}

/// The app's plug-in database knows the test instrument, as after a scan.
fn knows_synth(a: &mut CantusApp) -> PluginInfo {
    a.plugins.db_path = None;
    let dir = tempfile::tempdir().unwrap().keep();
    let file = dir.join("Cantus Test Synth.clap");
    std::fs::copy(test_synth(), &file).unwrap();
    let plugins = cantus_host::clap::ClapBackend.scan(&file).unwrap();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap();
    a.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    let list = a.hosted_instruments();
    assert_eq!(list.len(), 1, "one instrument scanned: {list:?}");
    list[0].clone()
}

fn inspector_text(a: &mut CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(420.0, 4000.0), 3, |ui| {
        cantus_ui::views::inspector::show(a, ui)
    })
}

#[test]
fn the_picker_puts_a_hosted_instrument_on_a_track_in_one_step() {
    let mut a = app(cantus_ui::starter_project());
    let info = knows_synth(&mut a);
    let track = a.project().tracks[0].id;
    let before = a.bus.history().len();
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info.clone())),
    );
    assert_eq!(a.bus.history().len(), before + 1, "one undo step");
    let plugin = a
        .project()
        .track(track)
        .unwrap()
        .instrument
        .external()
        .cloned();
    let plugin = plugin.expect("the track hosts it");
    assert_eq!(plugin.name, "Cantus Test Synth");
    assert!(!plugin.state.is_empty());
    assert!(
        a.plugins.hosted_instrument(track).is_some(),
        "loaded once, kept"
    );
    a.select_track(track);
    let text = inspector_text(&mut a);
    assert!(text.contains("Cantus Test Synth"), "{text}");
    assert!(text.contains("CLAP"), "{text}");
    assert!(
        text.contains(&cantus_ui::i18n::t("hosted.status_ok")),
        "{text}"
    );
    // Undo gives the track its old instrument back, and the plug-in goes.
    a.undo();
    assert!(a
        .project()
        .track(track)
        .unwrap()
        .instrument
        .firmus()
        .is_some());
    a.sync_engine();
    assert!(a.plugins.hosted_instrument(track).is_none());
    // `AddInstrument`: a new track that plays it.
    let tracks = a.project().tracks.len();
    a.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Hosted(Box::new(info)),
    );
    assert_eq!(a.project().tracks.len(), tracks + 1);
    let last = a.project().tracks.last().unwrap();
    assert!(last.instrument.external().is_some());
}

#[cfg(windows)]
#[test]
fn opening_its_editor_from_the_track_opens_the_plugin_window() {
    let mut a = app(cantus_ui::starter_project());
    let info = knows_synth(&mut a);
    let track = a.project().tracks[0].id;
    assert!(a.set_hosted_instrument(track, &info));
    a.sync_engine();
    a.open_inst_editor(track);
    assert!(
        a.plugins.instrument_window_open(track),
        "the plug-in's own window ({})",
        a.status.text
    );
    assert!(a.inst.open.is_none(), "not the native frame");
    // F8 closes it when it is in front (round 27, ADR 0051). Fix6: a test is not the
    // program in the foreground, and Windows lets no background program raise a window
    // over the user's (the foreground lock) — so wherever the desktop had another
    // program's window over the plug-in window's middle, F8 rightly kept bringing it
    // forward, never closing it, however long the test waited (fix5). In the app the
    // click that asks for the editor is on Cantus, which has the foreground. Here the
    // window is put always on top — over every program's ordinary windows, and an
    // always-on-top window over it does not count as covering — and this thread's
    // messages are handled as the app's frame loop handles them, so "in front" is the
    // window's own state, not the desktop's. Covered and brought forward is tested where
    // the test owns the window over it (`clap_window`, `quick_gestures`).
    on_top_and_pumped(&a, track);
    assert!(
        a.instrument_window_in_front(track),
        "in front once nothing ordinary can cover it"
    );
    a.toggle_instrument_window_of(track);
    assert!(
        !a.plugins.instrument_window_open(track),
        "F8 closes the plug-in's window in front"
    );
    assert!(a.inst.open.is_none(), "nothing else opened");
}

/// Put `track`'s plug-in window always on top, shown, without taking the focus, and handle
/// this thread's window messages (the window is this thread's). Shown: a process's first
/// `ShowWindow` takes the show command its launcher passed (a runner that starts tests
/// hidden or minimised hides it) — in the app the first window is Cantus's own, here it
/// is the plug-in's.
#[cfg(windows)]
fn on_top_and_pumped(a: &CantusApp, track: TrackId) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, SetWindowPos, ShowWindow, TranslateMessage, HWND_TOPMOST,
        MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOWNOACTIVATE,
    };
    let h = a
        .plugins
        .hosted_instrument(track)
        .and_then(|h| h.window_handle())
        .expect("its window") as HWND;
    // SAFETY: our own window, on the thread that made it; `m` is a live MSG.
    unsafe {
        ShowWindow(h, SW_SHOWNOACTIVATE);
        SetWindowPos(
            h,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let mut m: MSG = std::mem::zeroed();
        while PeekMessageW(&mut m, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    }
}

#[test]
fn a_missing_hosted_instrument_opens_says_so_and_saves_back_untouched() {
    let mut p = cantus_ui::starter_project();
    let mut plugin = ExternalPlugin::new(
        PluginFormat::Vst3,
        "0123456789ABCDEF0123456789ABCDEF",
        "Far Synth",
    );
    plugin.vendor = "Elsewhere Audio".into();
    plugin.state = PluginState::new(vec![9, 8, 7, 6]);
    plugin.params.insert(3, -1.5);
    p.tracks[0].instrument = InstrumentSpec::External(plugin.clone());
    let track = p.tracks[0].id;
    let mut a = app(p);
    a.sync_engine();
    assert!(a.plugins.instrument_missing(track).is_some());
    a.select_track(track);
    let text = inspector_text(&mut a);
    assert!(text.contains("Far Synth"), "{text}");
    assert!(text.contains("VST3"), "{text}");
    assert!(text.contains("Elsewhere Audio"), "{text}");
    assert!(
        text.contains(&cantus_ui::i18n::t("hosted.status_missing")),
        "{text}"
    );
    // Its editor cannot open; the status line says why instead.
    a.open_inst_editor(track);
    assert!(!a.plugins.instrument_window_open(track));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.cantus");
    a.save_to_path(&path);
    let back = cantus_project::load(&path, true).unwrap().project;
    assert_eq!(back.tracks[0].instrument, InstrumentSpec::External(plugin));
}

/// One inspector frame with `events`; every painted text with where it lies.
fn inspector_frame(
    ctx: &egui::Context,
    a: &mut CantusApp,
    events: Vec<egui::Event>,
    frame: &mut f64,
) -> Vec<(String, egui::Rect)> {
    *frame += 1.0;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(420.0, 4000.0),
        )),
        time: Some(*frame / 60.0),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| cantus_ui::views::inspector::show(a, ui));
    });
    out.textures_delta.clear();
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_string(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut v);
    }
    v
}

/// Review of fix3: a hosted instrument's parameter row works as the native rows while a
/// modulator maps — only the drag is the depth's; a double-click still resets the value.
#[test]
fn a_hosted_row_double_clicked_while_a_modulator_maps_resets() {
    use cantus_core::modulator::{ModHost, ModKind};
    let mut a = app(cantus_ui::starter_project());
    let info = knows_synth(&mut a);
    let track = a.project().tracks[0].id;
    assert!(a.set_hosted_instrument(track, &info));
    a.sync_engine();
    let p = a
        .plugins
        .hosted_instrument(track)
        .expect("loaded")
        .params
        .iter()
        .find(|p| !p.hidden && p.automatable && p.max > p.min && !p.stepped)
        .cloned()
        .expect("a continuous parameter");
    let away = if p.default > (p.min + p.max) * 0.5 {
        p.min
    } else {
        p.max
    };
    a.exec(Command::SetInstrumentPluginParam {
        track,
        id: p.id,
        value: Some(away),
    });
    let id = a
        .add_modulator(ModHost::Track(track), ModKind::Lfo, None)
        .unwrap();
    a.toggle_mapping(id);
    a.select_track(track);
    let ctx = ctx();
    let mut frame = 0.0;
    inspector_frame(&ctx, &mut a, vec![], &mut frame);
    let texts = inspector_frame(&ctx, &mut a, vec![], &mut frame);
    let label = texts
        .iter()
        .find(|(t, _)| *t == p.name)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("the row of {}: {texts:?}", p.name));
    let at = egui::pos2(label.right() + 60.0, label.center().y);
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    inspector_frame(
        &ctx,
        &mut a,
        vec![egui::Event::PointerMoved(at)],
        &mut frame,
    );
    for _ in 0..2 {
        inspector_frame(&ctx, &mut a, vec![press(true)], &mut frame);
        inspector_frame(&ctx, &mut a, vec![press(false)], &mut frame);
    }
    inspector_frame(&ctx, &mut a, vec![], &mut frame);
    let now = a
        .project()
        .track(track)
        .unwrap()
        .instrument
        .external()
        .and_then(|e| e.params.get(&p.id).copied());
    assert_eq!(
        now,
        Some(p.default),
        "{}: reset by the double-click",
        p.name
    );
}

/// Two hosted instruments picked one after the other on one track are two undo steps: the
/// first can be had back (review isolation-6 #8, #16 — a key per track merged them).
#[test]
fn two_picks_on_one_track_are_two_steps() {
    let mut a = app(cantus_ui::starter_project());
    let info = knows_synth(&mut a);
    let track = a.project().tracks[0].id;
    let before = a.bus.history().len();
    assert!(a.set_hosted_instrument(track, &info));
    assert!(a.set_hosted_instrument(track, &info));
    assert_eq!(a.bus.history().len(), before + 2, "two steps");
}
