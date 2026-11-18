//! fxb — wave B's windows (Vinculum, Lenis, Cedo) never cut a word, in English and
//! Spanish at 75 … 200 %, in Slate and Paper, every colourway, in every state its module
//! names (`DESIGN_STATES`): elided, dropped, a designed short form, or **clipped** — whole
//! but painted past its card or the window (the probe's `start_clipped`).

use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::skin;
use cantus_ui::kit::text::probe;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

/// Draw `app`'s open window of `kind` at `k` and say every word not read whole.
fn probe_window(app: &mut CantusApp, kind: EffectKind, k: f32, dark: bool) -> Vec<String> {
    let ctx = context(dark);
    views::fx::frame::shell_of(kind).set_scale(&ctx, k);
    let size = views::fx::frame::designed_size(kind) * k;
    probe::start_clipped();
    cantus_ui::raster::shoot(&ctx, size, 1.0, 3, None, |root| {
        let ctx = root.ctx().clone();
        views::fx::window(app, &ctx);
    });
    let shorts = probe::take_shorts();
    let mut out: Vec<String> = probe::take()
        .into_iter()
        .chain(shorts)
        .map(|c| format!("{:?} -> {:?} ({})", c.full, c.shown, c.how))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The words not read whole when `unit` is drawn with `kv` over its demo, playing.
fn cuts(
    unit: &'static str,
    kind: EffectKind,
    kv: &[(&str, f32)],
    k: f32,
    dark: bool,
) -> Vec<String> {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(unit);
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, unit).set_many(kv, "cmd.fx_preset");
    }
    views::fx::demo_feed(&mut app, kind);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    probe_window(&mut app, kind, k, dark)
}

type States = &'static [(&'static str, &'static [(&'static str, f32)])];

fn units() -> [(&'static str, EffectKind, States); 3] {
    [
        (
            "vinculum",
            EffectKind::BusCompressor,
            views::fx::vinculum::DESIGN_STATES,
        ),
        (
            "lenis",
            EffectKind::ResonanceSuppressor,
            views::fx::lenis::DESIGN_STATES,
        ),
        ("cedo", EffectKind::Ducker, views::fx::cedo::DESIGN_STATES),
    ]
}

fn check(unit: &'static str, kind: EffectKind, states: States) {
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for dark in [true, false] {
            for k in [0.75f32, 1.0, 2.0] {
                let u = skin::Unit::Effect(kind);
                let ways: Vec<&str> = skin::colourways(u).iter().map(|w| w.id).collect();
                for (state, kv) in states {
                    for w in &ways {
                        skin::set_colourway(skin::colourway_unit(u).expect("a unit"), w);
                        for c in cuts(unit, kind, kv, k, dark) {
                            bad.push(format!(
                                "{unit} {state} {w} {lang:?} dark {dark} at {k}: {c}"
                            ));
                        }
                    }
                    skin::set_colourways(std::iter::empty());
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}

#[test]
fn vinculum_s_window_never_cuts_or_clips_a_word() {
    let [(u, k, s), _, _] = units();
    check(u, k, s);
}

#[test]
fn lenis_s_window_never_cuts_or_clips_a_word() {
    let [_, (u, k, s), _] = units();
    check(u, k, s);
}

#[test]
fn cedo_s_window_never_cuts_or_clips_a_word() {
    let [_, _, (u, k, s)] = units();
    check(u, k, s);
}

/// Each unit's states are named once, the first its start.
#[test]
fn each_unit_names_its_design_states() {
    for (unit, _, states) in units() {
        assert_eq!(states[0].0, "start", "{unit}");
        for (i, (a, _)) in states.iter().enumerate() {
            assert!(states[i + 1..].iter().all(|(b, _)| a != b), "{unit}: {a}");
        }
    }
}
