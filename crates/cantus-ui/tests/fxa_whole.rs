//! fxa — wave A's windows (Loquax, Fractus, Caelum) never cut a word, in English and
//! Spanish at 75 … 200 %, in Slate and Paper, every colourway: elided, dropped, a designed
//! short form, or **clipped** — whole but painted past its card or the window (the probe's
//! `start_clipped`, added after the integrator found Paper Spanish's first row of Loquax's
//! cards running out of the window while the plain probe saw nothing).

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

/// A unit, its kind, its settings over its demo.
type Case = (&'static str, EffectKind, &'static [(&'static str, f32)]);

#[test]
fn wave_a_s_windows_never_cut_or_clip_a_word() {
    let cases: &[Case] = &[
        ("loquax", EffectKind::Vocoder, &[]),
        (
            "loquax",
            EffectKind::Vocoder,
            &[("robot", 0.0), ("bands", 64.0)],
        ),
        ("fractus", EffectKind::Glitch, &[]),
        (
            "fractus",
            EffectKind::Glitch,
            &[("clevel", 0.8), ("cfreeze", 1.0)],
        ),
        ("caelum", EffectKind::Shimmer, &[]),
        (
            "caelum",
            EffectKind::Shimmer,
            &[("inf", 1.0), ("ishim", 1.0)],
        ),
    ];
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for dark in [true, false] {
            for k in [0.75f32, 1.0, 2.0] {
                for (unit, kind, kv) in cases {
                    let u = skin::Unit::Effect(*kind);
                    let ways: Vec<&str> = skin::colourways(u).iter().map(|w| w.id).collect();
                    for w in ways {
                        skin::set_colourway(skin::colourway_unit(u).expect("a unit"), w);
                        for c in cuts(unit, *kind, kv, k, dark) {
                            bad.push(format!(
                                "{unit} {w} {kv:?} {lang:?} dark {dark} at {k}: {c}"
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

/// Fractus's pages of cards (design sheet v2): each page's cards and the display over them
/// read whole in English and Spanish at 75 … 200 %, in Slate and Paper — the page with the
/// keys held and the frozen cloud's among them.
#[test]
fn fractus_s_pages_never_cut_or_clip_a_word() {
    let pages = views::fx::fractus::PAGES.len();
    assert_eq!(pages, 4);
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for dark in [true, false] {
            for k in [0.75f32, 1.0, 2.0] {
                for page in 1..pages {
                    let mut app =
                        CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                    app.demo_editor("fractus");
                    let (strip, effect, _) = app.fx.open.expect("the unit is open");
                    views::fx::host::DawHost::new(&mut app, strip, effect, "fractus").set_many(
                        &[("trig", 2.0), ("cfreeze", 1.0), ("cwin", 3.0)],
                        "cmd.fx_preset",
                    );
                    views::fx::demo_feed(&mut app, EffectKind::Glitch);
                    views::fx::fractus::show_page(&mut app, page);
                    app.fx.pos = Some(egui::pos2(0.0, 0.0));
                    for c in probe_window(&mut app, EffectKind::Glitch, k, dark) {
                        bad.push(format!("fractus page {page} {lang:?} dark {dark} at {k}: {c}"));
                    }
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}
