//! Arca's and Rota's windows read whole (ADRs 0136 and 0137, provisional; the house rule:
//! nothing elided in English or Spanish at any scale) — and Phasis's circuit card with its
//! lamp vibe: each window drawn playing at 75 %, 100 %, 150 % and 200 % in both languages,
//! with the settings whose words are longest (the high-gain amp, the bass amp with its
//! condenser, the cabinet alone; the rotary's fast and brake), and the kit's text probe
//! finds no word cut anywhere in it. // fx3:arca

use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::text::probe;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

/// The words cut when `unit` is drawn with `kv` over its demo, playing.
fn cuts(
    unit: &'static str,
    kind: EffectKind,
    kv: &[(&str, f32)],
    lang: Locale,
    k: f32,
) -> Vec<String> {
    cantus_ui::i18n::set_locale(lang);
    let ctx = context();
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(unit);
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, unit).set_many(kv, "cmd.fx_preset");
    }
    views::fx::demo_feed(&mut app, kind);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    views::fx::frame::shell_of(kind).set_scale(&ctx, k);
    let size = views::fx::frame::designed_size(kind) * k;
    probe::start();
    cantus_ui::raster::shoot(&ctx, size, 1.0, 3, None, |root| {
        let ctx = root.ctx().clone();
        views::fx::window(&mut app, &ctx);
    });
    let mut out: Vec<String> = probe::take()
        .into_iter()
        .map(|c| format!("{:?} -> {:?}", c.full, c.shown))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// A unit, its kind, its settings over its demo.
type Case = (&'static str, EffectKind, &'static [(&'static str, f32)]);

#[test]
fn arca_rota_and_the_lamp_vibe_are_never_cut() {
    let cases: &[Case] = &[
        ("arca", EffectKind::Amp, &[("model", 4.0), ("cab", 4.0)]),
        (
            "arca",
            EffectKind::Amp,
            &[("model", 5.0), ("cab", 5.0), ("mic", 2.0)],
        ),
        ("arca", EffectKind::Amp, &[("model", 0.0), ("cab", 3.0)]),
        ("rota", EffectKind::Rotary, &[("speed", 2.0)]),
        ("rota", EffectKind::Rotary, &[("speed", 0.0)]),
        ("phasis", EffectKind::Phaser, &[("style", 1.0)]),
    ];
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        for k in [0.75f32, 1.0, 1.5, 2.0] {
            for (unit, kind, kv) in cases {
                for c in cuts(unit, *kind, kv, lang, k) {
                    bad.push(format!("{unit} {kv:?} {lang:?} at {k}: {c}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "words cut:\n{}", bad.join("\n"));
}
