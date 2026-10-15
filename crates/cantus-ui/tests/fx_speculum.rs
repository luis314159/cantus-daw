//! Speculum's editor (ADR 0056): every view paints its measurements and its own settings, a
//! view chosen in one step, docking, and its presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::speculum;
use cantus_ui::CantusApp;
use support::*;

fn with_speculum() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Analyzer);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| cantus_ui::views::fx::window(a, ui.ctx()))
        .textures_delta
        .clear();
}

fn click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    use egui::PointerButton::Primary;
    let b = |pressed| egui::Event::PointerButton {
        pos: at,
        button: Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(a, ctx, vec![b(true)]);
    frame(a, ctx, vec![b(false)]);
    frame(a, ctx, vec![]);
}

fn set_view(a: &mut CantusApp, s0: StripId, fx: EffectId, view: f32) {
    a.set_fx_param(s0, fx, "view", view, false);
}

/// Every view paints its measurements and only the settings it reads.
#[test]
fn every_view_paints_what_it_measures_and_its_settings() {
    let (mut a, s0, fx) = with_speculum();
    speculum::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Analyzer);
    let cases: [(f32, &[&str], &[&str]); 6] = [
        (
            0.0,
            &[
                "overview",
                "spectrum",
                "oscilloscope",
                "vectorscope",
                "loudness",
                "momentary",
                "correlation",
                "1k",
                "on the beat",
            ],
            &["fft", "average", "slope/oct"],
        ),
        (
            1.0,
            &["spectrum", "100", "10k", "average", "held"],
            &["fft", "channel", "floor", "16k", "l/r"],
        ),
        (2.0, &["spectrogram", "1k"], &["fft"]),
        (
            3.0,
            &["oscilloscope", "on the beat"],
            &["sync", "trigger", "length", "8 beats", "window"],
        ),
        (4.0, &["vectorscope", "correlation", "left", "right"], &[]),
        (
            5.0,
            &[
                "momentary",
                "short-term",
                "integrated",
                "range",
                "true peak max",
                "lufs",
                "lu",
                "dbtp",
                "short-term, last 30 s",
                "peak",
            ],
            &["target", "reset"],
        ),
    ];
    for (view, shows, settings) in cases {
        set_view(&mut a, s0, fx, view);
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
        for want in shows.iter().chain(settings.iter()) {
            assert!(
                text.contains(want),
                "view {view}: missing {want:?} in {text}"
            );
        }
        assert!(
            !text.contains('\u{2026}'),
            "view {view}: a word cut short in {text}"
        );
    }
}

#[test]
fn a_click_on_a_tab_chooses_the_view_as_one_step() {
    let (mut a, s0, _) = with_speculum();
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let tabs = a.fx.speculum.tabs.expect("the tabs were drawn");
    // Six tabs: the fifth is the vectorscope.
    click(
        &mut a,
        &ctx,
        egui::pos2(tabs.left() + tabs.width() * 4.5 / 6.0, tabs.center().y),
    );
    let view = a.project().strip(s0).unwrap().effects[0].spec.get("view");
    assert_eq!(view, 4.0);
    a.undo();
    let view = a.project().strip(s0).unwrap().effects[0].spec.get("view");
    assert_eq!(view, 0.0, "one undo step");
}

/// With a stream but nothing in it yet, the editor says to play; with no stream at all (no
/// audio device, or eight Speculums watched already), it says that instead (review 1).
#[test]
fn with_nothing_measured_it_says_why() {
    // No device: the dummy engine has no rings.
    let (mut a, _, _) = with_speculum();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("No stream"), "{text}");
    assert!(!text.contains("Play the song"), "{text}");
    // A device, nothing played yet.
    let (engine, _proc) = cantus_engine::EngineHandle::manual(48_000.0);
    let mut a = CantusApp::from_parts(Some(cantus_ui::starter_project()), engine, false);
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Analyzer);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("Play the song"), "{text}");
}

/// Docking: the Speculum leaves the floating editor for a tab of the workspace, with its
/// measurements.
#[test]
fn docking_moves_it_to_a_tab_with_what_it_measured() {
    let (mut a, s0, fx) = with_speculum();
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Analyzer);
    let before = speculum::measured(&a, fx).unwrap();
    assert!(before > 0);
    speculum::dock(&mut a, s0, fx);
    a.close_fx_editor();
    assert_eq!(speculum::measured(&a, fx), Some(before));
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    let key = egui::Id::new(("speculum-dock", fx.0)).value();
    let mut title = String::new();
    assert!(cantus_ui::views::workspace_device::title(
        &ctx, key, &mut title
    ));
    assert!(title.starts_with("Speculum"), "{title}");
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in speculum::FACTORY_PRESETS {
        for (k, _) in *values {
            assert!(
                EffectKind::Analyzer.param(k).is_some(),
                "{label}: no parameter {k}"
            );
        }
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// No word is cut short in Spanish either, in any view.
#[test]
fn no_word_is_cut_in_spanish() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let (mut a, s0, fx) = with_speculum();
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Analyzer);
    for view in 0..6 {
        set_view(&mut a, s0, fx, view as f32);
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        assert!(!text.contains('\u{2026}'), "view {view}: {text}");
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Opening a Speculum that is docked brings its tab forward instead of a second view of
/// the same stream.
#[test]
fn opening_a_docked_speculum_brings_its_tab_forward() {
    let (mut a, s0, fx) = with_speculum();
    speculum::dock(&mut a, s0, fx);
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    assert!(a.fx.open.is_none(), "the floating editor closed");
    assert!(speculum::is_docked(&a, fx));
}

/// Review 1: docked Speculums belong to their project: opening another closes them.
#[test]
fn another_project_closes_the_docked_speculums() {
    let (mut a, s0, fx) = with_speculum();
    speculum::dock(&mut a, s0, fx);
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    assert!(speculum::is_docked(&a, fx));
    a.replace_project(cantus_ui::starter_project());
    frame(&mut a, &ctx, vec![]);
    assert!(!speculum::is_docked(&a, fx));
}

/// Review 1: in a docked tab of any width the settings break into rows that fit — never a
/// row wider than the tab — and at the docked tab's smallest and default sizes every
/// setting is painted whole, in both languages.
#[test]
fn a_docked_tabs_settings_fit_its_width() {
    for which in [0usize, 3, 5] {
        for width in (300..=1_000).step_by(25) {
            let rows = speculum::settings_layout(which, width as f32);
            assert!(rows.len() <= 3, "view {which} at {width}: {} rows", rows.len());
        }
    }
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        for size in [egui::vec2(560.0, 420.0), egui::vec2(760.0, 460.0)] {
            let (mut a, s0, fx) = with_speculum();
            cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Analyzer);
            let mut view = speculum::View::new(48_000.0);
            let ctx = ctx();
            let text = paint(&ctx, size, 2, |ui| {
                let mut host = cantus_ui::views::fx::host::DawHost::new(&mut a, s0, fx, "speculum");
                let set = cantus_engine::fx::speculum::settings(
                    cantus_ui::views::fx::host::FxHost::spec(&host),
                );
                let s = cantus_ui::kit::Style::current();
                speculum::body_ui(&mut host, &mut view, ui, &s, ui.max_rect(), &set);
            });
            assert!(!text.contains('\u{2026}'), "{lang:?} {size:?}: {text}");
            let held = if lang == cantus_ui::i18n::Locale::En { "HELD" } else { "RETENIDO" };
            assert!(text.contains(held), "{lang:?} {size:?}: the last setting, in {text}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}
