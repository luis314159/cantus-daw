//! Comprimo's editor, driven the way a user drives it: through the displays' published
//! geometry, with one undo step per gesture, in both languages — and the two places it
//! reaches the desk: the strip's gain-reduction meter and "Duck this strip from…".

mod support;
use cantus_core::*;
use cantus_ui::views::comp_editor::{self, Speed};
use cantus_ui::CantusApp;
use support::*;

const SIZE: egui::Vec2 = egui::vec2(1500.0, 950.0);

/// The language is global: the tests here take turns, so the Spanish one cannot change
/// what an English one paints.
static LOCALE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    LOCALE.lock().unwrap_or_else(|e| e.into_inner())
}

/// An app with a compressor on the first track's strip, its editor open.
fn with_comp() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Compressor);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId, fx: EffectId) -> EffectSpec {
    a.project()
        .strip(s)
        .unwrap()
        .effect(fx)
        .unwrap()
        .spec
        .clone()
}

/// One frame of the compressor's window with these input events.
fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>, time: f64) {
    frame_held(a, ctx, events, time, egui::Modifiers::NONE);
}

/// One frame with these keys held.
fn frame_held(
    a: &mut CantusApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    time: f64,
    modifiers: egui::Modifiers,
) {
    let mut all = vec![egui::Event::ModifiersChanged(modifiers)];
    all.extend(events);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(time),
        events: all,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| comp_editor::window(a, ui.ctx()))
        .textures_delta
        .clear();
}

/// Hover, press, move in steps, release — a drag as egui sees one.
fn drag(
    a: &mut CantusApp,
    ctx: &egui::Context,
    from: egui::Pos2,
    to: egui::Pos2,
    modifiers: egui::Modifiers,
) {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    // egui's clock only runs forward, across drags too.
    let mut time = ctx.input(|i| i.time) + 1.0;
    let mut step = |a: &mut CantusApp, events| {
        time += 0.05;
        frame_held(a, ctx, events, time, modifiers);
    };
    step(a, vec![]);
    step(a, vec![egui::Event::PointerMoved(from)]);
    step(a, vec![egui::Event::PointerMoved(from), button(from, true)]);
    for i in 1..=5 {
        let at = from + (to - from) * (i as f32 / 5.0);
        step(a, vec![egui::Event::PointerMoved(at)]);
    }
    step(a, vec![egui::Event::PointerMoved(to), button(to, false)]);
    step(a, vec![]);
}

/// The geometry once the layout has settled (the rows under the displays are measured on
/// the first frame).
fn geometry(a: &mut CantusApp, ctx: &egui::Context) -> comp_editor::CompGeometry {
    for i in 0..3 {
        frame(a, ctx, vec![], i as f64 * 0.05);
    }
    a.comp_editor.geometry()
}

#[test]
fn the_editor_paints_every_part_with_a_demo_running() {
    let _turn = turn();
    let (mut a, s0, fx) = with_comp();
    comp_editor::demo_setup(&mut a, s0, fx);
    a.open_effect_editor(s0, fx);
    let text = paint_editors(&mut a, SIZE);
    // Labels and section names are set in capitals, and a label wider than its cell takes
    // its short form (the full name is on the help line); a value's unit is set a hair
    // apart in a smaller face (no space in the painted text).
    let lower = text.to_lowercase().replace(' ', "");
    let second_stage = cantus_ui::i18n::t("fx.comp.det_dual.short");
    let lookahead = cantus_ui::i18n::t("fx.comp.look.short");
    // The header's preset well, where the presets menu used to be.
    let presets = cantus_ui::i18n::t("shell.preset.none");
    for want in [
        "Comprimo compressor",
        "Punch",
        "Mastering",
        "Custom (from Punch)",
        "Engine",
        &second_stage,
        "Hear",
        "Wet",
        "Delta",
        "Compare",
        "Latency 2.0 ms",
        "Oversampling",
        "Loudness out",
        "GR · 10 s",
        "Crest dB",
        "Response to a burst",
        "Key EQ",
        "Band 2",
        "Dynamics",
        &lookahead,
        "AUTO",
        "Whole take",
        &presets,
    ] {
        assert!(
            lower.contains(&want.to_lowercase().replace(' ', "")),
            "never painted {want:?}: {text}"
        );
    }
    // The measured numbers are there: a loudness, and the response's attack.
    assert!(text.contains("attack "), "{text}");
    assert!(
        !text.contains("comp."),
        "a raw string key was painted: {text}"
    );
}

#[test]
fn it_speaks_spanish_everywhere() {
    let _turn = turn();
    let (mut a, s0, fx) = with_comp();
    comp_editor::demo_setup(&mut a, s0, fx);
    a.open_effect_editor(s0, fx);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let text = paint_editors(&mut a, SIZE);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let lower = text.to_lowercase();
    for want in [
        "Motor",
        "Oír",
        "Comparar",
        "Respuesta a una ráfaga",
        "EQ de la clave",
    ] {
        assert!(
            lower.contains(&want.to_lowercase()),
            "never painted {want:?}: {text}"
        );
    }
    assert!(
        !text.contains("comp."),
        "a raw string key was painted: {text}"
    );
}

/// Drag the slope of the transfer curve: the ratio follows the pointer, lands on a click
/// stop, and the whole drag is one step of undo.
#[test]
fn dragging_the_curve_sets_the_ratio_on_its_click_stops() {
    let _turn = turn();
    let (mut a, s0, fx) = with_comp();
    a.set_fx_param(s0, fx, "ratio", 2.0, false);
    a.set_fx_param(s0, fx, "knee", 0.0, false);
    let ctx = ctx();
    let g = geometry(&mut a, &ctx);
    let thr = -18.0;
    // Grab the curve 12 dB over the threshold (6 dB over at 2:1), and pull it down to
    // where 5.8:1 would put it: 12 / 5.8 = 2.07 dB over.
    let from = g.transfer_point(thr + 12.0, thr + 6.0).unwrap();
    let to = g.transfer_point(thr + 12.0, thr + 12.0 / 5.8).unwrap();
    drag(&mut a, &ctx, from, to, egui::Modifiers::NONE);
    assert_eq!(
        spec(&a, s0, fx).get("ratio"),
        6.0,
        "snapped to the 6:1 stop"
    );
    a.undo();
    assert_eq!(
        spec(&a, s0, fx).get("ratio"),
        2.0,
        "one undo step for the drag"
    );
    // With Shift the ratio is free.
    drag(&mut a, &ctx, from, to, egui::Modifiers::SHIFT);
    let r = spec(&a, s0, fx).get("ratio");
    assert!(r != 6.0 && (r - 5.8).abs() < 0.6, "free ratio {r}");
}

/// The threshold is a line on the history: drag it, and it is where you left it.
#[test]
fn dragging_the_threshold_line_on_the_history() {
    let _turn = turn();
    let (mut a, s0, fx) = with_comp();
    let ctx = ctx();
    let g = geometry(&mut a, &ctx);
    let h = g.history.unwrap();
    let y = |d: f32| h.top() + -d / g.range_db * h.height();
    let from = egui::pos2(h.center().x, y(-18.0));
    let to = egui::pos2(h.center().x, y(-30.0));
    drag(&mut a, &ctx, from, to, egui::Modifiers::NONE);
    let thr = spec(&a, s0, fx).get("thr");
    assert!((thr - -30.0).abs() < 0.5, "threshold {thr}");
    a.undo();
    assert_eq!(spec(&a, s0, fx).get("thr"), -18.0);
}

/// The engine drawer: the first turn makes the style custom, starting from the style's
/// own numbers, in one undo step; picking a style goes back to its own detector.
#[test]
fn the_engine_drawer_makes_a_style_its_own() {
    let _turn = turn();
    let (mut a, s0, fx) = with_comp();
    a.set_comp_style(s0, fx, cantus_dsp::Style::Opto);
    let before = spec(&a, s0, fx);
    a.set_comp_engine(s0, fx, "det_fb", 0.5, true);
    a.set_comp_engine(s0, fx, "det_fb", 0.6, true);
    a.end_gesture();
    let s = cantus_engine::comp_settings(&spec(&a, s0, fx));
    let d = s.engine.expect("custom now");
    let opto = cantus_dsp::Style::Opto.spec();
    assert_eq!(d.feedback, 0.6);
    assert_eq!(d.rms_ms, opto.rms_ms, "the rest start from Opto's own");
    assert_eq!(d.program_release, opto.program_release);
    assert_eq!(comp_editor::style_name(&s), "Custom (from Opto)");
    a.undo();
    assert_eq!(
        spec(&a, s0, fx),
        before,
        "one undo step for the whole gesture"
    );
    a.redo();
    a.set_comp_style(s0, fx, cantus_dsp::Style::Bus);
    let s = cantus_engine::comp_settings(&spec(&a, s0, fx));
    assert!(s.engine.is_none());
    assert_eq!(s.style, cantus_dsp::Style::Bus);
}

/// A key-EQ band made by a double-click where you clicked, and removed without a trace
/// (the parameters are sparse, so "removed" means "not stored").
#[test]
fn a_key_band_is_made_where_you_double_click_and_leaves_no_trace() {
    let _turn = turn();
    let (mut a, s0, fx) = with_comp();
    let ctx = ctx();
    let g = geometry(&mut a, &ctx);
    let at = g.key_eq_point(1_000.0, 6.0).unwrap();
    let click = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut a, &ctx, vec![egui::Event::PointerMoved(at)], 1.0);
    frame(&mut a, &ctx, vec![click(true), click(false)], 1.05);
    frame(&mut a, &ctx, vec![click(true), click(false)], 1.1);
    frame(&mut a, &ctx, vec![], 1.2);
    let s = spec(&a, s0, fx);
    assert_eq!(s.get("sc0.used"), 1.0);
    assert_eq!(s.get("sc0.shape"), 0.0, "a bell away from the edges");
    assert!(
        (s.get("sc0.freq") - 1_000.0).abs() < 60.0,
        "{}",
        s.get("sc0.freq")
    );
    assert!(
        (s.get("sc0.gain") - 6.0).abs() < 1.0,
        "{}",
        s.get("sc0.gain")
    );
    let settings = cantus_engine::comp_settings(&s);
    assert!(settings.key_bands[0].active);
    a.remove_key_band(s0, fx, 0);
    let s = spec(&a, s0, fx);
    assert!(
        !s.params.keys().any(|k| k.starts_with("sc0.")),
        "{:?}",
        s.params
    );
}

/// "Duck this strip from…": one undo step adds a compressor keyed from the chosen strip,
/// set to duck with a release of an eighth note at the tempo; one undo takes it all away.
#[test]
fn ducking_from_a_strip_is_one_step() {
    let _turn = turn();
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    let s1 = a
        .project()
        .strips
        .iter()
        .map(|s| s.id)
        .find(|id| *id != s0)
        .unwrap();
    let before = a.project().strip(s0).unwrap().effects.len();
    let fx = a.duck_strip_from(s0, s1).expect("added");
    let slot = a.project().strip(s0).unwrap().effect(fx).unwrap().clone();
    assert_eq!(slot.sidechain, Some(s1));
    assert_eq!(slot.spec.kind, EffectKind::Compressor);
    let eighth = 30_000.0 / a.project().tempo_bpm as f32;
    assert!((slot.spec.get("rel") - eighth).abs() < 0.01);
    assert_eq!(slot.spec.get("range"), 12.0);
    a.undo();
    assert_eq!(a.project().strip(s0).unwrap().effects.len(), before);
    // A key that would close a loop is refused, and leaves nothing behind.
    a.exec(Command::SetStripOutput {
        id: s1,
        output: Some(s0),
    });
    let n = a.project().strip(s1).unwrap().effects.len();
    assert!(a.duck_strip_from(s1, s0).is_none());
    assert_eq!(a.project().strip(s1).unwrap().effects.len(), n);
}

/// A strip with a compressor shows a gain-reduction meter on the desk.
#[test]
fn the_desk_shows_gain_reduction_on_a_compressor_strip() {
    let _turn = turn();
    let (mut a, s0, _fx) = with_comp();
    let ctx = ctx();
    // How many gain-reduction lanes the desk shows filling.
    let red_bars = |a: &mut CantusApp| {
        let mut n = 0;
        for i in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
                time: Some(i as f64 * 0.25),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| cantus_ui::views::mixer::show(a, ui));
            });
            out.textures_delta.clear();
            n = a.mixer.gr_lanes_filling();
        }
        n
    };
    assert_eq!(
        a.comp_gr_on_strip(s0),
        Some(None),
        "no engine: a lane, with nothing reported"
    );
    assert_eq!(red_bars(&mut a), 0);
    // With telemetry reporting (a demo stands in for the engine), the lane fills.
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    comp_editor::demo_setup(&mut a, s0, fx);
    a.open_effect_editor(s0, fx);
    paint_editors(&mut a, SIZE);
    let gr = a.comp_gr_on_strip(s0).flatten().expect("reported");
    assert!(gr > 0.5, "the demo is compressing: {gr}");
    assert_eq!(red_bars(&mut a), 1, "one strip, one lane filling");
    // A strip without one shows none.
    let s1 = a
        .project()
        .strips
        .iter()
        .map(|s| s.id)
        .find(|id| *id != s0)
        .unwrap();
    assert_eq!(a.comp_gr_on_strip(s1), None);
}

/// Every factory preset sets what it names — and only real parameters.
#[test]
fn the_factory_presets_are_real() {
    let _turn = turn();
    let (mut a, s0, fx) = with_comp();
    assert!(comp_editor::FACTORY_PRESETS.len() >= 6);
    for (label, values) in comp_editor::FACTORY_PRESETS {
        assert_ne!(cantus_ui::i18n::t(label), *label, "{label} has no words");
        let map = values.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        a.apply_fx_params(s0, fx, &map, "cmd.fx_preset");
        let s = spec(&a, s0, fx);
        for (k, v) in *values {
            assert!(EffectKind::Compressor.param(k).is_some(), "{label}: {k}");
            assert_eq!(s.get(k), *v, "{label}: {k}");
        }
    }
}

/// The Whole speed keeps the whole take: when it fills, pairs of columns fold into one
/// and each column holds twice as many frames.
#[test]
fn the_whole_take_folds_instead_of_forgetting() {
    let _turn = turn();
    let (mut a, _s0, _fx) = with_comp();
    a.comp_editor.set_speed(Speed::Whole);
    for i in 0..5_000 {
        a.comp_editor.feed(0.5, 0.25, (i % 7) as f32);
    }
    let (cols, frames) = a.comp_editor.columns();
    assert!(cols <= 4_096 && cols > 2_000, "{cols}");
    assert_eq!(frames, 2);
    a.comp_editor.set_speed(Speed::Fast);
    for _ in 0..5_000 {
        a.comp_editor.feed(0.5, 0.25, 1.0);
    }
    assert_eq!(a.comp_editor.columns().0, 4_096, "Fast forgets the oldest");
}
