//! Tremor's editor (ADR 0055): what it paints, a mode chosen in one step, the movement it
//! draws is the movement the DSP plays, and its presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::tremor;
use cantus_ui::CantusApp;
use support::*;

fn with_tremor() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Tremolo);
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
fn the_editor_paints_its_cards_and_the_rate_in_words() {
    let (mut a, s0, fx) = with_tremor();
    tremor::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Tremolo);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase();
    for want in [
        "tremolo",
        "auto-pan",
        "harmonic",
        "level over two cycles",
        "128 bpm",
        "lfo",
        "free",
        "sync",
        "depth",
        "swing",
        "no latency",
    ] {
        assert!(lower.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn at_depth_zero_the_display_says_the_sound_passes() {
    let (mut a, _, _) = with_tremor();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the sound passes untouched"), "{text}");
}

#[test]
fn a_click_on_a_mode_chooses_it_as_one_step() {
    let (mut a, s0, _) = with_tremor();
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let display = a.fx.tremor.display.expect("the display was drawn");
    // The modes' column is left of the display: the second key is Auto-pan.
    let at = egui::pos2(display.left() - 100.0, display.top() + 26.0 + 13.0 + 3.0);
    click(&mut a, &ctx, at);
    let mode = a.project().strip(s0).unwrap().effects[0].spec.get("mode");
    assert_eq!(mode, 1.0, "auto-pan chosen");
    a.undo();
    let mode = a.project().strip(s0).unwrap().effects[0].spec.get("mode");
    assert_eq!(mode, 0.0, "one undo step");
}

/// The display draws what the DSP plays, the editor's own `movement_at` against the DSP
/// at a free rate, in every mode: tremolo's gains (constant 1s in); auto-pan's places in
/// the balance (constant 1s in, each channel's gain the balance's); harmonic's two halves
/// (a constant is all low half; a signal at half the sample rate all high half).
#[test]
fn the_movement_drawn_is_the_movement_played() {
    let n = 16_000;
    let nyquist: Vec<f32> = (0..n)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    for mode in [0.0f32, 1.0, 2.0] {
        let mut spec = EffectSpec::new(EffectKind::Tremolo);
        for (k, v) in [
            ("mode", mode),
            ("depth", 0.8),
            ("sync", 0.0),
            ("rate", 3.0),
            ("shape", 2.0),
            ("edge", 0.4),
            ("spread", 60.0),
        ] {
            spec.set(k, v);
        }
        let ts = cantus_engine::fx::tremor::settings(&spec);
        let run = |input: &[f32]| {
            let mut d = cantus_dsp::tremor::Tremor::new(48_000.0);
            d.set(&ts);
            d.reset();
            let (mut l, mut r) = (input.to_vec(), input.to_vec());
            d.process(&mut l, &mut r);
            (l, r)
        };
        let (l, r) = run(&vec![1.0f32; n]);
        let (hl, _) = run(&nyquist);
        // Past the crossover's settling (harmonic); every sample counts in the others.
        let from = if mode == 2.0 { 4_800 } else { 0 };
        for i in (from..n).step_by(37) {
            let c = 3.0 * i as f64 / 48_000.0;
            let (a, b) = tremor::movement_at(&ts, c);
            let (want_l, want_r) = match mode as u8 {
                0 => ((a + 1.0) / 2.0, (b + 1.0) / 2.0),
                1 => (
                    cantus_dsp::tremor::balance_gains(a).0,
                    cantus_dsp::tremor::balance_gains(b).1,
                ),
                _ => ((a + 1.0) / 2.0, (b + 1.0) / 2.0),
            };
            let (got_l, got_r) = match mode as u8 {
                2 => (l[i], hl[i].abs()),
                _ => (l[i], r[i]),
            };
            assert!(
                (got_l - want_l).abs() < 1e-3,
                "mode {mode}, sample {i}: {got_l} vs {want_l}"
            );
            assert!(
                (got_r - want_r).abs() < 1e-3,
                "mode {mode}, sample {i}: {got_r} vs {want_r}"
            );
        }
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in tremor::FACTORY_PRESETS {
        for (k, _) in *values {
            assert!(
                EffectKind::Tremolo.param(k).is_some(),
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

#[test]
fn every_preset_moves_the_sound() {
    for (label, values) in tremor::FACTORY_PRESETS {
        let mut spec = EffectSpec::new(EffectKind::Tremolo);
        for (k, v) in *values {
            spec.set(k, *v);
        }
        assert!(spec.get("depth") > 0.0, "{label} does something");
    }
}

/// Swing reads the same in the editor as everywhere else (the exported plug-ins, a typed
/// value, an automation lane): 0 … 100 % of the parameter, one number for one setting.
#[test]
fn swing_reads_in_the_editor_as_the_host_reads_it() {
    let (mut a, s0, fx) = with_tremor();
    {
        let mut host = cantus_ui::views::fx::host::DawHost::new(&mut a, s0, fx, "tremor");
        cantus_ui::views::fx::host::FxHost::set_many(
            &mut host,
            &[("sync", 1.0), ("swing", 0.37)],
            "cmd.fx_preset",
        );
    }
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let host_reads = cantus_core::fx::readout::readout(0.37, "%");
    assert_eq!(host_reads, "37%");
    assert!(text.contains(&host_reads), "{text}");
    assert!(
        !text.contains("59%"),
        "not the drum machines' 50 … 75 %: {text}"
    );
}
