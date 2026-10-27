//! Sibilus's editor, headless: it paints its state in real units, the threshold is dragged
//! on the history and the band on the spectrum — one undo step each — Learn sets the
//! threshold from a passage, and the presets and the preview do what they say.

mod support;
use cantus_core::*;
use cantus_dsp::sibilus::Column;
use cantus_ui::views::fx::deesser::{self, event_peaks, learn_threshold};
use cantus_ui::CantusApp;
use support::*;

const SIZE: egui::Vec2 = egui::vec2(1400.0, 900.0);

/// Painted text as the tests read it: labels are set in capitals, a value's unit is set
/// apart from its number without a space, and minus signs are real ones.
fn norm(s: &str) -> String {
    s.to_lowercase().replace(' ', "").replace('\u{2212}', "-")
}

/// The locale is global: these tests take turns, so one painting in Spanish never meets
/// another reading English.
fn turn() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// An app with a de-esser on the first track's strip, its editor open and fed.
fn open() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::DeEsser);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    deesser::demo_setup(&mut a, s0, fx);
    a.open_effect_editor(s0, fx);
    deesser::demo_feed(&mut a);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

/// One frame of the editor window with these input events.
fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::fx::window(a, &ctx);
    })
    .textures_delta
    .clear();
}

/// Let the window find its size, and check it has stopped growing: an editor that asks
/// for a little more than it is given makes a resizable window creep taller every frame.
fn settle(a: &mut CantusApp, ctx: &egui::Context) -> deesser::Geometry {
    for _ in 0..4 {
        frame(a, ctx, vec![]);
    }
    let g = a.fx.deesser.geo;
    frame(a, ctx, vec![]);
    frame(a, ctx, vec![]);
    let now = a.fx.deesser.geo;
    assert_eq!(now.detector, g.detector, "the window keeps growing");
    assert_eq!(now.spectrum, g.spectrum, "the window keeps growing");
    g
}

/// Press at `from`, move to `to` in steps, release.
fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for k in 1..=6 {
        let at = from + (to - from) * (k as f32 / 6.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
}

/// Open, fed by the real DSP over a synthetic voice: the editor says what it is doing, in
/// dB, Hz and ms, with every state visible at rest.
#[test]
fn the_editor_paints_its_state_in_real_units() {
    let _turn = turn();
    let (mut a, _, _) = open();
    assert!(
        a.fx.deesser.history().len() > 400,
        "the demo filled the history"
    );
    assert!(
        a.fx.deesser.history().iter().any(|c| c.gr_db > 5.0),
        "the demo's sibilants were cut"
    );
    let text = paint_editors(&mut a, SIZE);
    for needle in [
        "Sibilus de-esser",
        "Threshold",
        "-38.0 dB",
        "Single vocal",
        "Split band",
        "Latency 7.6 ms",
        "4.5 kHz",
        "11.0 kHz",
        "SIGNAL",
        "CUT",
        "DETECTOR",
        "sibilance",
        "events",
        "centre",
        "Live",
    ] {
        assert!(
            norm(&text).contains(&norm(needle)),
            "missing {needle:?} in: {text}"
        );
    }
}

/// The threshold is a line on the detector lane: dragged, it follows the pointer in dB,
/// and the whole drag is one undo step.
#[test]
fn the_threshold_is_dragged_on_the_history() {
    let _turn = turn();
    let (mut a, s0, _) = open();
    let ctx = ctx();
    let g = settle(&mut a, &ctx);
    assert!(g.detector.width() > 100.0, "the lanes were laid out: {g:?}");
    let before = spec(&a, s0).get("thr");
    let x = g.detector.center().x;
    drag(
        &mut a,
        &ctx,
        egui::pos2(x, g.detector_y(before)),
        egui::pos2(x, g.detector_y(-20.0)),
    );
    let after = spec(&a, s0).get("thr");
    assert!((after - -20.0).abs() < 0.6, "threshold went to {after}");
    a.undo();
    assert_eq!(spec(&a, s0).get("thr"), before, "one undo step");
}

/// The band is dragged on the spectrum: an edge moves one side; the middle moves both at
/// the same ratio. Each drag is one undo step.
#[test]
fn the_band_is_dragged_on_the_spectrum() {
    let _turn = turn();
    let (mut a, s0, _) = open();
    let ctx = ctx();
    let g = settle(&mut a, &ctx);
    let y = g.spectrum.center().y;
    let (lo, hi) = (spec(&a, s0).get("lo"), spec(&a, s0).get("hi"));
    drag(
        &mut a,
        &ctx,
        egui::pos2(g.spectrum_x(lo), y),
        egui::pos2(g.spectrum_x(3_000.0), y),
    );
    let s = spec(&a, s0);
    assert!(
        (s.get("lo") / 3_000.0 - 1.0).abs() < 0.03,
        "low edge {}",
        s.get("lo")
    );
    assert_eq!(s.get("hi"), hi, "the other edge stays");
    // The middle: both edges, one ratio.
    let mid = (3_000.0f32 * hi).sqrt();
    drag(
        &mut a,
        &ctx,
        egui::pos2(g.spectrum_x(mid), y),
        egui::pos2(g.spectrum_x(mid * 1.5), y),
    );
    let s = spec(&a, s0);
    let (r_lo, r_hi) = (s.get("lo") / 3_000.0, s.get("hi") / hi);
    assert!(
        (r_lo - 1.5).abs() < 0.05 && (r_hi - r_lo).abs() < 0.01,
        "{r_lo} {r_hi}"
    );
    a.undo();
    let s = spec(&a, s0);
    assert!((s.get("lo") / 3_000.0 - 1.0).abs() < 0.03 && s.get("hi") == hi);
    a.undo();
    assert_eq!(spec(&a, s0).get("lo"), lo);
}

fn col(det_db: f32, conf: f32) -> Column {
    Column {
        input: 0.3,
        output: 0.3,
        det_db,
        gr_db: 0.0,
        conf,
        hz: 0.0,
    }
}

/// Learn: the median sibilant gets half the Range. In Single vocal the classifier marks the
/// events; three is the least it learns from.
#[test]
fn learn_sets_the_threshold_from_a_passage() {
    let _turn = turn();
    let mut cols = Vec::new();
    for peak in [-20.0, -12.0, -16.0] {
        cols.extend([
            col(-40.0, 0.0),
            col(peak - 3.0, 0.9),
            col(peak, 1.0),
            col(-45.0, 0.1),
        ]);
    }
    assert_eq!(event_peaks(&cols, true), vec![-20.0, -12.0, -16.0]);
    assert_eq!(learn_threshold(&cols, true, 6.0), Some(-19.0));
    assert_eq!(
        learn_threshold(&cols[..8], true, 6.0),
        None,
        "two events are too few"
    );

    // Through the editor: start, hear a passage, stop — one undo step.
    let (mut a, s0, fx) = open();
    let before = spec(&a, s0).get("thr");
    a.deesser_learn(s0, fx);
    assert!(a.fx.deesser.learning);
    for c in &cols {
        a.fx.deesser.push(*c);
    }
    a.deesser_learn(s0, fx);
    assert!(!a.fx.deesser.learning);
    assert_eq!(spec(&a, s0).get("thr"), -19.0);
    assert_eq!(a.fx.deesser.learned, Some(-19.0));
    let text = paint_editors(&mut a, SIZE);
    assert!(
        norm(&text).contains("learned"),
        "the learned threshold is marked: {text}"
    );
    a.undo();
    assert_eq!(spec(&a, s0).get("thr"), before);
}

/// Every factory preset names only real parameters, has its label in both languages, and
/// sets what it names.
#[test]
fn the_presets_are_complete() {
    let _turn = turn();
    let (mut a, s0, fx) = open();
    for (label, params) in deesser::FACTORY_PRESETS {
        for locale in cantus_ui::i18n::Locale::ALL {
            cantus_ui::i18n::set_thread_locale(locale);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {locale:?}");
        }
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let map = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        a.apply_fx_params(s0, fx, &map, "cmd.fx_preset");
        let s = spec(&a, s0);
        for (k, v) in *params {
            assert!(EffectKind::DeEsser.param(k).is_some(), "{label}: {k}");
            assert_eq!(s.get(k), *v, "{label}: {k}");
        }
    }
    assert!(deesser::FACTORY_PRESETS.len() >= 4);
}

/// The inspector's picture: the band and the settings, in words.
#[test]
fn the_preview_says_what_it_is_set_to() {
    let _turn = turn();
    let (a, s0, _) = open();
    let spec = spec(&a, s0);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(320.0, 120.0), 1, |ui| {
        deesser::preview(ui, &spec);
    });
    assert!(text.contains("-38 dB"), "{text}");
    assert!(
        text.contains("Single vocal") && text.contains("Split band"),
        "{text}"
    );
    assert!(
        text.contains("4.5 kHz") && text.contains("11.0 kHz"),
        "{text}"
    );
}

/// In Spanish, the editor speaks Spanish.
#[test]
fn the_editor_speaks_spanish() {
    let _turn = turn();
    let (mut a, _, _) = open();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    let text = paint_editors(&mut a, SIZE);
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for needle in ["Umbral", "Voz sola", "Dividir banda", "SEÑAL", "DETECTOR"] {
        assert!(
            norm(&text).contains(&norm(needle)),
            "missing {needle:?}: {text}"
        );
    }
}
