//! Organum's editor on the kit's shell (ADR 0024): it opens in every view and paints what it
//! is; a drawbar dragged and a cell turned are one undo step each; a modulation source
//! dropped on a cell connects it; no text is elided or overlaps at 75 % and 200 % or in
//! Spanish; presets set what they name; the rotary prints the speeds the engine reports.

mod inst_keys;
mod support;
use cantus_core::inst::modulation::connections;
use cantus_core::*;
use cantus_ui::views::inst::{organ, preset_spec, View};
use inst_keys::{audit, drag, drop_source, frame, joined, scroll, set_scale};
use support::*;

fn with_organ() -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Organ)),
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
fn it_opens_in_every_view_and_paints_its_drawbars_and_rotors() {
    let (mut a, _) = with_organ();
    let ctx = ctx();
    for view in [View::Play, View::Edit, View::Matrix] {
        a.inst.view = view;
        let text = settle(&mut a, &ctx);
        assert!(text.contains("ORGANUM"), "{view:?}: {text}");
        if view != View::Matrix {
            // The registration is written with spaces that keep it one readout.
            assert!(
                text.contains("16\u{2032}") && text.contains("88\u{a0}8000\u{a0}000"),
                "{view:?}: {text}"
            );
            assert!(text.contains("UPPER MANUAL"), "{view:?}: {text}");
        }
    }
    // The rotor at rest reads its target where the cabinet is shown (Play; Edit's rotary
    // card is further down the scrolled view).
    a.inst.view = View::Play;
    let text = settle(&mut a, &ctx);
    assert!(text.contains("Horn 40 rpm"), "{text}");
    // With an engine reporting, the rotary prints what it reports.
    organ::demo_feed(&mut a);
    let text = settle(&mut a, &ctx);
    assert!(text.contains("Horn 318 rpm"), "{text}");
}

#[test]
fn a_drawbar_dragged_is_one_undo_step() {
    let (mut a, track) = with_organ();
    a.inst.view = View::Edit;
    let ctx = ctx();
    settle(&mut a, &ctx);
    let bar = *a
        .inst
        .organ
        .bars
        .get("u4")
        .expect("the 4′ drawbar was drawn");
    assert_eq!(spec(&a, track).get("u4"), 0.0);
    drag(
        &mut a,
        &ctx,
        egui::pos2(bar.center().x, bar.top() + 4.0),
        egui::pos2(bar.center().x, bar.bottom() - 10.0),
    );
    let pulled = spec(&a, track).get("u4");
    assert!(pulled >= 6.0, "the 4′ was pulled out to {pulled}");
    a.undo();
    assert_eq!(
        spec(&a, track).get("u4"),
        0.0,
        "one undo takes the whole drag back"
    );
}

#[test]
fn a_cell_turned_is_one_undo_step_and_a_dropped_source_connects() {
    let (mut a, track) = with_organ();
    a.inst.view = View::Play;
    let ctx = ctx();
    settle(&mut a, &ctx);
    let cell = *a.inst.organ.placed.get("drive").expect("the drive cell");
    let before = spec(&a, track).get("drive");
    let c = cell.center();
    drag(&mut a, &ctx, c, c - egui::vec2(0.0, 60.0));
    let after = spec(&a, track).get("drive");
    assert!(after > before + 0.1, "drive went from {before} to {after}");
    a.undo();
    assert!(
        !spec(&a, track).is_stored("drive"),
        "one undo takes the whole turn back"
    );
    // A source dragged from its chip and dropped on the cell connects it.
    drop_source(&mut a, &ctx, "mw", c);
    assert!(
        connections(&spec(&a, track)).any(|x| x.src == "mw" && x.dst == "drive"),
        "the wheel drives the drive"
    );
    a.undo();
    assert_eq!(connections(&spec(&a, track)).count(), 0, "one undo step");
}

#[test]
fn the_switches_and_lamps_set_what_they_name_in_one_step() {
    let (mut a, track) = with_organ();
    a.inst.view = View::Play;
    let ctx = ctx();
    let text = settle(&mut a, &ctx);
    for word in ["Stop", "Slow", "Fast", "2nd", "3rd", "V1", "C3"] {
        assert!(text.contains(word), "{word}: {text}");
    }
    let click = |a: &mut cantus_ui::CantusApp, at: egui::Pos2| {
        drag(a, &ctx, at, at);
    };
    // Fast: the speed switch's third segment.
    let speed = *a
        .inst
        .organ
        .placed
        .get("rot.speed")
        .expect("the speed switch");
    assert_eq!(spec(&a, track).get("rot.speed"), 1.0);
    click(
        &mut a,
        egui::pos2(speed.right() - speed.width() / 6.0, speed.center().y),
    );
    assert_eq!(spec(&a, track).get("rot.speed"), 2.0, "Fast");
    a.undo();
    assert!(!spec(&a, track).is_stored("rot.speed"), "one undo step");
    // The lower manual through the scanner: its lamp.
    let lower = *a.inst.organ.placed.get("vib.lower").expect("the lamp");
    assert_eq!(spec(&a, track).get("vib.lower"), 0.0);
    click(&mut a, lower.center());
    assert_eq!(spec(&a, track).get("vib.lower"), 1.0, "lit");
    a.undo();
    assert!(!spec(&a, track).is_stored("vib.lower"), "one undo step");
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
            let area = a.inst.organ.area.expect("the view laid out");
            let vis = a.inst.organ.view.expect("the view laid out");
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
    let (mut a, track) = with_organ();
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::Organ);
    cantus_ui::views::inst::demo_feed(&mut a, InstrumentKind::Organ);
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
fn edit_scrolls_to_the_rotary_speaker() {
    let (mut a, _) = with_organ();
    a.inst.view = View::Edit;
    let ctx = ctx();
    settle(&mut a, &ctx);
    let vis = a.inst.organ.view.unwrap();
    for _ in 0..8 {
        scroll(&mut a, &ctx, vis.center(), -400.0);
    }
    let text = joined(&frame(&mut a, &ctx, vec![]));
    assert!(text.contains("ROTARY SPEAKER"), "{text}");
    assert!(text.contains("rpm over 8 s"), "the spin-up: {text}");
}

#[test]
fn presets_set_what_they_name() {
    let (mut a, track) = with_organ();
    assert!(organ::FACTORY_PRESETS.len() >= 8);
    let full = organ::FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "organ.preset.full")
        .unwrap();
    a.apply_inst_spec(
        track,
        &preset_spec(InstrumentKind::Organ, full),
        "cmd.fx_preset",
    );
    let s = spec(&a, track);
    for k in cantus_core::inst::organ::UPPER {
        assert_eq!(s.get(k), 8.0, "{k}");
    }
    assert_eq!(s.text("macro1.name"), Some("Drive"));
    assert_eq!(s.text("mod1.dst"), Some("drive"));
    // Every preset names its macros and only real keys.
    for p in organ::FACTORY_PRESETS {
        let s = preset_spec(InstrumentKind::Organ, p);
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
    assert!(spec(&a, track).params.is_empty(), "one undo step");
}

#[test]
fn screenshots_for_the_design_review() {
    inst_keys::shots(InstrumentKind::Organ, "organum");
}

#[test]
fn the_editor_uses_kit_parts_and_tokens_only() {
    let found = inst_keys::lint(&[
        "organ.rs",
        "organ_cabinet.rs",
        "organ_drawbars.rs",
        "organ_parts.rs",
    ]);
    assert!(found.is_empty(), "{found:#?}");
}
