//! Latitudo's editor: what it paints, what its gestures change (one undo step each), how
//! it takes the scope's points, and its presets.

mod support;
use cantus_core::*;
use cantus_dsp::latitudo::view::{pack_point, telemetry};
use cantus_ui::views::fx::stereo::{self, State};
use cantus_ui::CantusApp;
use support::*;

/// An app with Latitudo on the first strip, its editor open.
fn with_stereo() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Stereo);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
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

/// Press at `from`, move to `to` in steps, release.
fn drag(a: &mut CantusApp, from: egui::Pos2, to: egui::Pos2) {
    let ctx = ctx();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    frame(a, &ctx, vec![]);
}

fn geometry(a: &mut CantusApp) -> stereo::Geometry {
    let ctx = ctx();
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    a.fx.stereo.geometry.expect("the editor drew its displays")
}

#[test]
fn the_editor_paints_its_cards_meters_and_map() {
    let (mut a, s0, fx) = with_stereo();
    cantus_ui::views::fx::demo_setup(&mut a, s0, fx, EffectKind::Stereo);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Stereo);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase().replace(' ', "");
    for want in [
        "Latitudo",
        "Vectorscope",
        "CORR",
        "Width",
        "Tone",
        "Mono",
        "120Hz",
        "Mid tone",
        "Side tone",
        "Haas",
        "Stereo",
        "Safe",
        "No latency",
        "200%",
        "1k",
    ] {
        let want = want.replace(' ', "").to_lowercase();
        assert!(lower.contains(&want), "missing {want:?} in {text}");
    }
    assert!(
        a.fx.stereo.points.len() > 1_000,
        "the scope was fed: {}",
        a.fx.stereo.points.len()
    );
}

#[test]
fn the_preview_says_the_settings_in_one_line() {
    let mut s = EffectSpec::new(EffectKind::Stereo);
    s.set("width", 1.3);
    s.set("mono", 120.0);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(420.0, 120.0), 1, |ui| {
        cantus_ui::views::fx::preview(ui, &s);
    });
    assert!(text.contains("Width 130%"), "{text}");
    assert!(text.contains("120 Hz"), "{text}");
}

/// A telemetry block with `seq` points made so far, point `j` of the block at (j, -j)
/// thousandths.
fn telemetry_with(seq: u32, corr: f32) -> [f32; 64] {
    let mut t = [0.0f32; 64];
    t[telemetry::SEQ] = (seq % telemetry::WRAP) as f32;
    t[telemetry::SAMPLE_RATE] = 48_000.0;
    t[telemetry::CORR] = corr;
    for j in 0..cantus_dsp::latitudo::POINTS {
        let v = j as f32 * 0.001;
        t[telemetry::POINTS_AT + j] = pack_point(v, -v);
    }
    t
}

/// The scope takes exactly the points that are new: none twice, through the counter's
/// wrap, never more than it keeps; the lowest correlation is remembered.
#[test]
fn the_scope_takes_each_point_once() {
    let mut st = State::default();
    st.ingest(&telemetry_with(5, 0.5));
    assert_eq!(st.points.len(), 5);
    st.ingest(&telemetry_with(5, 0.5));
    assert_eq!(st.points.len(), 5, "the same block again brings nothing");
    st.ingest(&telemetry_with(8, -0.2));
    assert_eq!(st.points.len(), 8);
    let newest = st.points.back().unwrap().0;
    assert!(
        (newest - 0.047).abs() < 1e-3,
        "the newest is the block's last: {newest}"
    );
    assert_eq!(st.worst(), Some(-0.2));
    let mut st = State::default();
    st.ingest(&telemetry_with(telemetry::WRAP - 2, 1.0));
    let before = st.points.len();
    st.ingest(&telemetry_with(3, 1.0));
    assert_eq!(st.points.len() - before, 5);
    for i in 0..200 {
        st.ingest(&telemetry_with(100 + i * 48, 1.0));
    }
    assert_eq!(st.points.len(), stereo::SCOPE_POINTS);
}

/// Dragging the high band's bar up widens the high band, as one undo step.
#[test]
fn dragging_a_bands_bar_sets_its_width_in_one_step() {
    let (mut a, s0, _) = with_stereo();
    let g = geometry(&mut a);
    let x = g.x(9_000.0);
    let from = egui::pos2(x, g.width_y(1.0));
    let to = egui::pos2(x, g.width_y(1.5));
    drag(&mut a, from, to);
    let w = spec(&a, s0).get("w_hi");
    assert!((w - 1.5).abs() < 0.05, "w_hi {w}");
    a.undo();
    assert_eq!(spec(&a, s0).get("w_hi"), 1.0, "one undo, the whole drag");
}

/// The mono edge waits at the left end; pulled right it switches mono on at that
/// frequency; a shelf's dot moves its frequency and gain together.
#[test]
fn the_mono_edge_and_a_shelf_are_dragged_on_the_map() {
    let (mut a, s0, _) = with_stereo();
    let g = geometry(&mut a);
    let from = egui::pos2(g.width.left() + 2.0, g.width.bottom() - 10.0);
    let to = egui::pos2(g.x(150.0), g.width.bottom() - 10.0);
    drag(&mut a, from, to);
    let mono = spec(&a, s0).get("mono");
    assert!((mono / 150.0 - 1.0).abs() < 0.05, "mono {mono}");
    // The side's high shelf: from (8 kHz, 0 dB) to (6 kHz, +6 dB).
    let g = geometry(&mut a);
    let from = egui::pos2(g.x(8_000.0), g.tone_y(0.0));
    let to = egui::pos2(g.x(6_000.0), g.tone_y(6.0));
    drag(&mut a, from, to);
    let s = spec(&a, s0);
    // The mid's high shelf sits on the same spot: whichever is nearer wins, and both
    // start at 8 kHz, 0 dB — one of them moved.
    let (mf, mg, sf, sg) = (
        s.get("m_hi_f"),
        s.get("m_hi"),
        s.get("s_hi_f"),
        s.get("s_hi"),
    );
    let moved = if mg != 0.0 { (mf, mg) } else { (sf, sg) };
    assert!((moved.0 / 6_000.0 - 1.0).abs() < 0.05, "{moved:?}");
    assert!((moved.1 - 6.0).abs() < 0.3, "{moved:?}");
}

/// Every factory preset names keys of the table, inside their ranges.
#[test]
fn the_factory_presets_are_in_range() {
    assert!(stereo::FACTORY_PRESETS.len() >= 10);
    for (name, values) in stereo::FACTORY_PRESETS {
        assert!(cantus_ui::i18n::t(name) != *name, "{name} has no words");
        for (k, v) in *values {
            let info = EffectKind::Stereo
                .param(k)
                .unwrap_or_else(|| panic!("{name}: {k}"));
            assert!(*v >= info.min && *v <= info.max, "{name}: {k} = {v}");
        }
    }
}
