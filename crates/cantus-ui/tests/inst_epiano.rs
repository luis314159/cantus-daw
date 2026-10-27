//! Lamina's editor on the kit's shell (ADR 0024): it opens in every view and paints its
//! geometry in millimetres; dragging the pickup's pole piece, a key-curve anchor or a cell
//! is one undo step each; a modulation source dropped on a cell connects it; no text is
//! elided or overlaps at 75 % and 200 % or in Spanish; presets set what they name.

mod inst_keys;
mod support;
use cantus_core::inst::modulation::connections;
use cantus_core::*;
use cantus_ui::views::inst::{epiano, preset_spec, View};
use inst_keys::{audit, drag, drop_source, frame, joined, scroll, set_scale};
use support::*;

fn with_epiano() -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::ElectricPiano)),
    );
    a.open_inst_editor(track);
    (a, track)
}

fn spec(a: &cantus_ui::CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("a native instrument")
}

fn settle(a: &mut cantus_ui::CantusApp, ctx: &egui::Context) -> String {
    let mut text = String::new();
    for _ in 0..3 {
        text = joined(&frame(a, ctx, vec![]));
    }
    text
}

#[test]
fn it_opens_in_every_view_and_paints_its_geometry_in_millimetres() {
    let (mut a, _) = with_epiano();
    let ctx = ctx();
    for view in [View::Play, View::Edit, View::Matrix] {
        a.inst.view = view;
        let text = settle(&mut a, &ctx);
        assert!(text.contains("LAMINA"), "{view:?}: {text}");
        if view != View::Matrix {
            assert!(text.contains("gap 1.60 mm"), "{view:?}: {text}");
            assert!(
                text.contains("64 mm tine"),
                "{view:?}: C4's tine is 64 mm: {text}"
            );
            assert!(text.contains("PICKUP"), "{view:?}: {text}");
        }
    }
    epiano::demo_feed(&mut a);
    a.inst.view = View::Play;
    let text = settle(&mut a, &ctx);
    assert!(text.contains("swinging \u{b1}1.60 mm"), "{text}");
}

#[test]
fn dragging_the_pole_piece_voices_the_pickup_in_one_undo_step() {
    let (mut a, track) = with_epiano();
    a.inst.view = View::Play;
    let ctx = ctx();
    settle(&mut a, &ctx);
    let pole = a.inst.epiano.pole.expect("the pole piece was drawn");
    let from = pole.center();
    drag(&mut a, &ctx, from, from + egui::vec2(-60.0, 0.0));
    let align = spec(&a, track).get("tine.align");
    assert!(align < 0.4, "offset dragged to {align} mm");
    a.undo();
    assert_eq!(
        spec(&a, track).get("tine.align"),
        0.6,
        "one undo takes the drag back"
    );
}

#[test]
fn dragging_a_key_curve_anchor_is_one_undo_step() {
    let (mut a, track) = with_epiano();
    a.inst.view = View::Edit;
    let ctx = ctx();
    settle(&mut a, &ctx);
    // The key curves sit at the bottom of the Edit view: scroll down to them.
    for _ in 0..16 {
        if a.inst.epiano.anchor.is_some() {
            break;
        }
        let vis = a.inst.epiano.view.expect("the view laid out");
        scroll(&mut a, &ctx, vis.center(), -vis.height() * 0.5);
    }
    let anchor = a.inst.epiano.anchor.unwrap_or_else(|| {
        panic!(
            "C4's anchor came into view: view {:?}, area {:?}",
            a.inst.epiano.view, a.inst.epiano.area
        )
    });
    drag(
        &mut a,
        &ctx,
        anchor.center(),
        anchor.center() + egui::vec2(0.0, -30.0),
    );
    let v = spec(&a, track).get("curve.hard.4");
    assert!(v > 0.05, "C4's hardness offset rose to {v}");
    a.undo();
    assert!(!spec(&a, track).is_stored("curve.hard.4"));
}

#[test]
fn a_cell_turned_is_one_undo_step_and_a_dropped_source_connects() {
    let (mut a, track) = with_epiano();
    a.inst.view = View::Play;
    let ctx = ctx();
    settle(&mut a, &ctx);
    let cell = *a.inst.epiano.placed.get("drive").expect("the drive cell");
    let before = spec(&a, track).get("drive");
    let c = cell.center();
    drag(&mut a, &ctx, c, c - egui::vec2(0.0, 60.0));
    let after = spec(&a, track).get("drive");
    assert!(after > before + 0.1, "drive went from {before} to {after}");
    a.undo();
    assert!(!spec(&a, track).is_stored("drive"), "one undo step");
    drop_source(&mut a, &ctx, "vel", c);
    assert!(
        connections(&spec(&a, track)).any(|x| x.src == "vel" && x.dst == "drive"),
        "velocity drives the drive"
    );
    a.undo();
    assert_eq!(connections(&spec(&a, track)).count(), 0, "one undo step");
}

#[test]
fn the_engine_switch_turns_the_tine_into_a_reed_in_one_step() {
    let (mut a, track) = with_epiano();
    a.inst.view = View::Play;
    let ctx = ctx();
    settle(&mut a, &ctx);
    let sw = *a
        .inst
        .epiano
        .placed
        .get("engine")
        .expect("the engine switch");
    let at = egui::pos2(sw.right() - sw.width() / 4.0, sw.center().y);
    drag(&mut a, &ctx, at, at);
    assert_eq!(spec(&a, track).get("engine"), 1.0, "Reed");
    let text = settle(&mut a, &ctx);
    assert!(text.contains("mm reed"), "the reed drawn to scale: {text}");
    a.undo();
    assert!(!spec(&a, track).is_stored("engine"), "one undo step");
}

/// Every problem of the painted text in Play and Edit (Edit scrolled through to its end).
fn problems(a: &mut cantus_ui::CantusApp, ctx: &egui::Context) -> Vec<String> {
    let mut all = Vec::new();
    for view in [View::Play, View::Edit] {
        a.inst.view = view;
        let mut texts = Vec::new();
        for _ in 0..3 {
            texts = frame(a, ctx, vec![]);
        }
        for _ in 0..16 {
            let area = a.inst.epiano.area.expect("the view laid out");
            let vis = a.inst.epiano.view.expect("the view laid out");
            for p in audit(&texts, vis.intersect(area)) {
                all.push(format!("{view:?}: {p}"));
            }
            if area.bottom() <= vis.bottom() + 1.0 {
                break;
            }
            scroll(a, ctx, vis.center(), -vis.height() * 0.7);
            texts = frame(a, ctx, vec![]);
        }
    }
    all
}

fn demo() -> (cantus_ui::CantusApp, TrackId) {
    let (mut a, track) = with_epiano();
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::ElectricPiano);
    cantus_ui::views::inst::demo_feed(&mut a, InstrumentKind::ElectricPiano);
    (a, track)
}

#[test]
fn no_text_is_elided_or_overlaps_at_75_100_and_200_percent() {
    for k in [0.75, 1.0, 2.0] {
        let (mut a, track) = demo();
        let ctx = ctx();
        set_scale(&ctx, track, k);
        let p = problems(&mut a, &ctx);
        assert!(p.is_empty(), "at {k}: {p:#?}");
    }
}

#[test]
fn no_text_is_elided_or_overlaps_in_spanish() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    let mut found = Vec::new();
    for k in [0.75, 1.0] {
        let (mut a, track) = demo();
        let ctx = ctx();
        set_scale(&ctx, track, k);
        found.extend(
            problems(&mut a, &ctx)
                .into_iter()
                .map(|p| format!("{k}: {p}")),
        );
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn presets_set_what_they_name() {
    let (mut a, track) = with_epiano();
    assert!(epiano::FACTORY_PRESETS.len() >= 8);
    let reed = epiano::FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "epiano.preset.reed")
        .unwrap();
    a.apply_inst_spec(
        track,
        &preset_spec(InstrumentKind::ElectricPiano, reed),
        "cmd.fx_preset",
    );
    let s = spec(&a, track);
    assert_eq!(s.get("engine"), 1.0);
    assert_eq!(s.text("mod1.dst"), Some("reed.gap"));
    for p in epiano::FACTORY_PRESETS {
        let s = preset_spec(InstrumentKind::ElectricPiano, p);
        assert_eq!(
            s.params.len(),
            p.values.len(),
            "{}: a key it names is not a parameter",
            p.label_key
        );
        assert!(s.text("macro1.name").is_some(), "{}", p.label_key);
        assert_ne!(
            cantus_ui::i18n::t(p.label_key),
            p.label_key,
            "{} has no name",
            p.label_key
        );
    }
    a.undo();
    assert!(spec(&a, track).params.is_empty());
}

#[test]
fn screenshots_for_the_design_review() {
    inst_keys::shots(InstrumentKind::ElectricPiano, "lamina");
}

#[test]
fn the_editor_uses_kit_parts_and_tokens_only() {
    let found = inst_keys::lint(&["epiano.rs", "epiano_keys.rs", "epiano_pickup.rs"]);
    assert!(found.is_empty(), "{found:#?}");
}
