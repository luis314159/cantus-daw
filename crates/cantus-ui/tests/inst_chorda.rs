//! Chorda's editor on the kit's shell (ADR 0024, ADR 0059): it opens in every view and
//! draws the piano from above — every key's strings at their length, where the hammers
//! strike, the dampers and the pedals — with the tuning's stretch, the felt's stiffness and
//! the strings' ring drawn from the model's own numbers; a cell turned is one undo step and
//! a modulation source dropped on it connects; no text is elided or overlaps at 75 % and
//! 200 % or in Spanish; presets set what they name; and the boot path starts designing the
//! keyboard before anyone asks for it.

mod inst_keys;
mod support;
use cantus_core::inst::modulation::connections;
use cantus_core::*;
use cantus_ui::views::inst::{chorda, preset_spec, View};
use cantus_ui::CantusApp;
use inst_keys::{audit, drag, drop_source, frame, joined, scroll, set_scale};
use support::*;

fn with_chorda() -> (CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda)),
    );
    a.open_inst_editor(track);
    (a, track)
}

fn spec(a: &CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("a native instrument")
}

fn settle(a: &mut CantusApp, ctx: &egui::Context) -> String {
    let mut text = String::new();
    for _ in 0..3 {
        text = joined(&frame(a, ctx, vec![]));
    }
    text
}

/// The whole Edit view's text, scrolled through to its end.
fn edit_text(a: &mut CantusApp, ctx: &egui::Context) -> String {
    a.inst.view = View::Edit;
    let mut text = settle(a, ctx);
    for _ in 0..16 {
        let area = a.inst.chorda.area.expect("the view laid out");
        let vis = a.inst.chorda.view.expect("the view laid out");
        if area.bottom() <= vis.bottom() + 1.0 {
            break;
        }
        scroll(a, ctx, vis.center(), -vis.height() * 0.7);
        text.push_str(&joined(&frame(a, ctx, vec![])));
    }
    text
}

#[test]
fn it_opens_in_every_view_and_draws_the_piano_from_above() {
    let (mut a, _) = with_chorda();
    let ctx = ctx();
    for view in [View::Play, View::Edit, View::Matrix] {
        a.inst.view = view;
        let text = settle(&mut a, &ctx);
        assert!(text.contains("CHORDA"), "{view:?}: {text}");
        if view != View::Matrix {
            assert!(text.contains("STRINGS"), "{view:?}: {text}");
            // Middle C until a key is played: three strings 63 cm long (Bensa et al.).
            assert!(text.contains("3 strings of 63 cm"), "{view:?}: {text}");
            assert!(text.contains("PEDALS"), "{view:?}: {text}");
        }
    }
    let text = edit_text(&mut a, &ctx);
    assert!(text.contains("A4 440.0 Hz"), "the tuning: {text}");
    assert!(text.contains("felt exponent 2.5"), "C4's hammer: {text}");
    // The strings' ring at C4, its first partial, as Bensa et al. measured: 6.2 s.
    assert!(text.contains("C4 rings 6.2 s"), "C4's ring: {text}");
    chorda::demo_feed(&mut a);
    a.inst.view = View::Play;
    let text = settle(&mut a, &ctx);
    assert!(text.contains("E5"), "the struck key is shown: {text}");
    assert!(text.contains("Sustain 40 %"), "the pedal: {text}");
}

#[test]
fn a_cell_turned_is_one_undo_step_and_a_dropped_source_connects() {
    let (mut a, track) = with_chorda();
    a.inst.view = View::Play;
    let ctx = ctx();
    settle(&mut a, &ctx);
    let cell = *a
        .inst
        .chorda
        .placed
        .get("hardness")
        .expect("the hardness cell");
    let before = spec(&a, track).get("hardness");
    let c = cell.center();
    drag(&mut a, &ctx, c, c - egui::vec2(0.0, 60.0));
    let after = spec(&a, track).get("hardness");
    assert!(
        after > before + 0.1,
        "hardness went from {before} to {after}"
    );
    a.undo();
    assert!(!spec(&a, track).is_stored("hardness"), "one undo step");
    drop_source(&mut a, &ctx, "vel", c);
    assert!(
        connections(&spec(&a, track)).any(|x| x.src == "vel" && x.dst == "hardness"),
        "velocity drives the hardness"
    );
    a.undo();
    assert_eq!(connections(&spec(&a, track)).count(), 0, "one undo step");
}

#[test]
fn the_tuning_curve_follows_the_stretch() {
    let (mut a, track) = with_chorda();
    let ctx = ctx();
    let stretched = edit_text(&mut a, &ctx);
    a.set_inst_value(track, "stretch", 0.0, false);
    let flat = edit_text(&mut a, &ctx);
    // With no stretch the octaves are only as wide as the strings' stiffness makes them.
    assert_ne!(stretched, flat);
    assert!(
        stretched.contains("C8 +"),
        "the top is tuned sharp: {stretched}"
    );
}

/// Every problem of the painted text in Play and Edit (Edit scrolled through to its end).
fn problems(a: &mut CantusApp, ctx: &egui::Context) -> Vec<String> {
    let mut all = Vec::new();
    for view in [View::Play, View::Edit] {
        a.inst.view = view;
        let mut texts = Vec::new();
        for _ in 0..3 {
            texts = frame(a, ctx, vec![]);
        }
        for _ in 0..16 {
            let area = a.inst.chorda.area.expect("the view laid out");
            let vis = a.inst.chorda.view.expect("the view laid out");
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

fn demo() -> (CantusApp, TrackId) {
    let (mut a, track) = with_chorda();
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::Chorda);
    cantus_ui::views::inst::demo_feed(&mut a, InstrumentKind::Chorda);
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
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
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
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn presets_set_what_they_name() {
    let (mut a, track) = with_chorda();
    assert!(chorda::FACTORY_PRESETS.len() >= 8);
    let felt = chorda::FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.felt")
        .expect("the felt piano");
    a.apply_inst_spec(
        track,
        &preset_spec(InstrumentKind::Chorda, felt),
        "cmd.fx_preset",
    );
    let s = spec(&a, track);
    assert!(s.get("hardness") < -0.5, "soft felt");
    assert_eq!(s.text("mod1.dst"), Some("hardness"));
    for p in chorda::FACTORY_PRESETS {
        let s = preset_spec(InstrumentKind::Chorda, p);
        assert_eq!(
            s.params.len(),
            p.values.len() + InstrumentKind::Chorda.stored_versions().len(),
            "{}: a key it names is not a parameter",
            p.label_key
        );
        assert_eq!(s.params.get("model"), Some(&1.0), "the model is stored");
        for (k, v) in p.values {
            let info = InstrumentKind::Chorda.param(k).expect("a parameter");
            assert!(
                (info.min..=info.max).contains(v),
                "{}: {k} = {v} is outside its range",
                p.label_key
            );
        }
        assert!(s.text("macro1.name").is_some(), "{}", p.label_key);
        assert_ne!(
            cantus_ui::i18n::t(p.label_key),
            p.label_key,
            "{} has no name",
            p.label_key
        );
    }
    a.undo();
    assert_eq!(spec(&a, track), NativeSpec::new(InstrumentKind::Chorda));
}

#[test]
fn the_inspector_reads_the_tuning_as_the_knobs_do() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut spec = NativeSpec::new(InstrumentKind::Chorda);
    assert!(
        chorda::preview_text(&spec).contains("A4 440.0 Hz · Stretch 100 %"),
        "{}",
        chorda::preview_text(&spec)
    );
    spec.set("stretch", 0.7).unwrap();
    assert!(chorda::preview_text(&spec).contains("70 %"));
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let es = chorda::preview_text(&spec);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(es.contains("La4 440.0 Hz"), "{es}");
}

#[test]
fn while_the_strings_are_designed_the_editor_says_so() {
    let (mut a, _) = with_chorda();
    let ctx = ctx();
    let mut t = [0.0; cantus_engine::TELEMETRY_LEN];
    t[cantus_engine::inst::chorda::tel::ALIVE] = 1.0;
    a.inst.injected = Some(t);
    for view in [View::Play, View::Edit] {
        a.inst.view = view;
        let text = settle(&mut a, &ctx);
        assert!(text.contains("designing the strings: "), "{view:?}: {text}");
    }
    t[cantus_engine::inst::chorda::tel::DESIGNED] = 1.0;
    a.inst.injected = Some(t);
    a.inst.view = View::Play;
    assert!(!settle(&mut a, &ctx).contains("designing"));
}

#[test]
fn the_tuning_is_set_never_automated() {
    let (mut a, track) = with_chorda();
    let clips = |a: &CantusApp| {
        a.project()
            .tracks
            .iter()
            .map(|t| t.clips.len())
            .sum::<usize>()
    };
    let before = clips(&a);
    for key in ["a4", "stretch", "stiffness"] {
        let made = a.create_automation_clip(AutoTarget::Instrument {
            track,
            key: key.into(),
        });
        assert!(!made, "{key}");
    }
    assert_eq!(clips(&a), before, "no clip");
    assert!(a.create_automation_clip(AutoTarget::Instrument {
        track,
        key: "hardness".into(),
    }));
}

#[test]
fn screenshots_for_the_design_review() {
    inst_keys::shots(InstrumentKind::Chorda, "chorda");
}

#[test]
fn the_editor_uses_kit_parts_and_tokens_only() {
    let found = inst_keys::lint(&["chorda.rs", "chorda_draw.rs"]);
    assert!(found.is_empty(), "{found:#?}");
}
