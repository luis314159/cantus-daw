//! Arca's and Rota's windows read whole (ADRs 0136 and 0137, provisional; the house rule:
//! nothing elided or abbreviated in English or Spanish at any scale) — and Phasis's window
//! with its lamp vibe, and every pedal face on Tabula's board: each window drawn playing at
//! 75 %, 100 %, 150 % and 200 % in both languages, with the settings whose words are longest
//! (the high-gain amp, the bass amp with its condenser, the cabinet alone; the rotary's fast
//! and brake), and the kit's text probe finds no word cut, and no short form painted in
//! place of a word, anywhere in it. // fx3:arca

use cantus_core::fx::tabula::{Pedal, PedalId};
use cantus_core::{Command, EffectKind, EffectSpec};
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

/// Draw `app`'s open window of `kind` at `k` and say every word not read whole.
fn probe_window(app: &mut CantusApp, kind: EffectKind, k: f32) -> Vec<String> {
    let ctx = context();
    views::fx::frame::shell_of(kind).set_scale(&ctx, k);
    let size = views::fx::frame::designed_size(kind) * k;
    probe::start();
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
fn cuts(unit: &'static str, kind: EffectKind, kv: &[(&str, f32)], k: f32) -> Vec<String> {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(unit);
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, unit).set_many(kv, "cmd.fx_preset");
    }
    views::fx::demo_feed(&mut app, kind);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    probe_window(&mut app, kind, k)
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
        ("phasis", EffectKind::Phaser, &[("style", 0.0)]),
    ];
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for k in [0.75f32, 1.0, 1.5, 2.0] {
            for (unit, kind, kv) in cases {
                for c in cuts(unit, *kind, kv, k) {
                    bad.push(format!("{unit} {kv:?} {lang:?} at {k}: {c}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}

/// Every pedal face on a board — each kind Tabula's faces table has, and Fragor's five
/// models — four to a board, in English and Spanish at 75 % and 100 %.
#[test]
fn every_pedal_face_on_the_board_reads_whole() {
    let mut specs: Vec<EffectSpec> = Vec::new();
    for f in views::fx::tabula_faces::FACES {
        if f.kind == EffectKind::Pedal {
            for m in 1..=5 {
                let mut s = EffectSpec::new(f.kind);
                s.set(cantus_core::fx::fragor::SLOTS[0], m as f32);
                specs.push(s);
            }
        } else {
            specs.push(EffectSpec::new(f.kind));
        }
    }
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for k in [0.75f32, 1.0] {
            for chunk in specs.chunks(4) {
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.demo_editor("tabula");
                let (strip, effect, _) = app.fx.open.expect("Tabula is open");
                for (i, spec) in chunk.iter().enumerate() {
                    app.exec(Command::InsertPedal {
                        strip,
                        effect,
                        index: 0,
                        pedal: Box::new(Pedal::new(PedalId(60 + i as u16), spec.clone())),
                        maps: Vec::new(),
                    });
                }
                app.fx.tabula.selected = Some(PedalId(60));
                app.fx.pos = Some(egui::pos2(0.0, 0.0));
                let kinds: Vec<String> = chunk.iter().map(|s| format!("{:?}", s.kind)).collect();
                for c in probe_window(&mut app, EffectKind::Board, k) {
                    bad.push(format!("{kinds:?} {lang:?} at {k}: {c}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}
