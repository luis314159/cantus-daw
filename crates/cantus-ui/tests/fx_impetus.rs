//! Impetus's editor (ADR 0056): what it paints, a band chosen in one step, the hit it draws
//! is the hit the DSP plays, the history keeps every column, and its presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::impetus;
use cantus_ui::CantusApp;
use support::*;

fn with_impetus() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Transient);
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

#[test]
fn the_editor_paints_its_cards() {
    let (mut a, s0, fx) = with_impetus();
    impetus::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Transient);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase();
    for want in [
        "input and gain",
        "a hit through it",
        "boost",
        "cut",
        "attack",
        "sustain",
        "speed",
        "length",
        "full",
        "highs",
        "lows",
        "+60%",
        "not a threshold",
        "no latency",
    ] {
        assert!(lower.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_zero_amounts_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_impetus();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

#[test]
fn a_click_on_a_band_chooses_it_as_one_step() {
    let (mut a, s0, _) = with_impetus();
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let keys = a.fx.impetus.band_keys.expect("the band keys were drawn");
    let at = egui::pos2(keys.left() + keys.width() * 0.5, keys.center().y);
    click(&mut a, &ctx, at);
    let band = a.project().strip(s0).unwrap().effects[0].spec.get("band");
    assert_eq!(band, 1.0, "highs chosen");
    a.undo();
    let band = a.project().strip(s0).unwrap().effects[0].spec.get("band");
    assert_eq!(band, 0.0, "one undo step");
}

/// The hit card is the DSP's: its output envelope is the hit run through Impetus, and the
/// attack moves it the way the knob says.
#[test]
fn the_hit_drawn_is_the_hit_played() {
    let mut spec = EffectSpec::new(EffectKind::Transient);
    spec.set("attack", 1.0);
    let s = cantus_engine::fx::impetus::settings(&spec);
    let (input, output) = impetus::hit_response(&s, 48_000.0);
    assert_eq!(input.len(), impetus::HIT_POINTS);
    // Run the DSP by hand on the same hit and compare a point in the onset.
    let mut d = cantus_dsp::impetus::Impetus::new(48_000.0);
    d.set(&s);
    d.reset();
    let n = (impetus::HIT_MS * 0.001 * 48_000.0) as usize;
    let x: Vec<f32> = (0..n).map(|i| impetus::hit_sample(i, 48_000.0)).collect();
    let (mut l, mut r) = (x.clone(), x.clone());
    d.process(&mut l, &mut r, None);
    let bin = n / impetus::HIT_POINTS;
    let at = (0.045 * 48_000.0) as usize / bin;
    let peak = l[at * bin..(at + 1) * bin]
        .iter()
        .fold(0f32, |m, v| m.max(v.abs()));
    assert_eq!(output[at], cantus_dsp::dynfx_follow::to_db(peak));
    assert!(output[at] > input[at] + 3.0, "the onset is lifted");
}

/// The history takes every column the audio made, however rarely the editor looks.
#[test]
fn the_history_keeps_every_column() {
    let mut h = cantus_ui::views::fx::dynfx::ColumnHistory::new(1_000);
    let mut d = cantus_dsp::impetus::Impetus::new(48_000.0);
    d.reset();
    let mut t = [0f32; 64];
    let (mut l, mut r) = (vec![0.1f32; 700], vec![0.1f32; 700]);
    for blk in 0..100 {
        d.process(&mut l, &mut r, None);
        if blk % 5 == 4 {
            d.telemetry(&mut t);
            h.update(&t);
        }
    }
    assert_eq!(h.len(), 100 * 700 / 512);
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in impetus::FACTORY_PRESETS {
        for (k, _) in *values {
            assert!(
                EffectKind::Transient.param(k).is_some(),
                "{label}: no parameter {k}"
            );
        }
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
        let mut spec = EffectSpec::new(EffectKind::Transient);
        for (k, v) in *values {
            spec.set(k, *v);
        }
        assert!(
            spec.get("attack") != 0.0 || spec.get("sustain") != 0.0,
            "{label} does something"
        );
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// No word is cut short, in either language, with the editor fed as when it plays.
#[test]
fn no_word_is_cut_in_either_language() {
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let (mut a, s0, fx) = with_impetus();
        impetus::demo_setup(&mut a, s0, fx);
        cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Transient);
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        assert!(!text.contains('\u{2026}'), "{lang:?}: {text}");
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Review 1: the key is chosen in Impetus's own editor, as in every keyed editor: the field
/// opens on the desk's strips, and choosing one routes the sidechain, as one undo step.
#[test]
fn the_key_is_chosen_in_the_editor() {
    let (mut a, s0, fx) = with_impetus();
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let field = a.fx.impetus.key_field.expect("the key's field was drawn");
    click(&mut a, &ctx, field.center());
    // The list: its own signal first, then the other strips; the second row is a strip.
    let mut chosen = None;
    for dy in [1.5f32, 2.5, 3.5] {
        let at = egui::pos2(field.left() + 30.0, field.bottom() + 24.0 * dy - 12.0);
        click(&mut a, &ctx, at);
        chosen = a.project().effect(s0, fx).and_then(|e| e.sidechain);
        if chosen.is_some() {
            break;
        }
        click(&mut a, &ctx, field.center());
    }
    let key = chosen.expect("a strip chosen from the list");
    assert_ne!(key, s0);
    a.undo();
    assert_eq!(
        a.project().effect(s0, fx).unwrap().sidechain,
        None,
        "one undo step"
    );
}
