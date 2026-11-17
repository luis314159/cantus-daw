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
        // fxa:loquax — the two other objects (phase B): the talk-box's tube, the desk mic.
        ("loquax", EffectKind::Vocoder, &[("obj", 1.0)]),
        ("loquax", EffectKind::Vocoder, &[("obj", 2.0), ("src", 0.0)]),
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
            &[("inf", 1.0), ("isus", 0.4)],
        ),
        (
            "caelum",
            EffectKind::Shimmer,
            &[("freeze", 1.0), ("duck", 12.0), ("pblvl", 0.6)],
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

/// fxa:loquax — the owner's v2 verdict: in Spanish (four rows of cards) the display does
/// not shrink; the window grows instead. In both languages at 75, 100 and 200 % the cards
/// take the rows `loquax::card_rows` says (so the designed size has room for them) and the
/// display — the microphone, the bank and the meters — is the same size, scaled.
#[test]
fn loquax_s_display_keeps_its_size_in_every_language_and_scale() {
    let mut heights = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for k in [0.75f32, 1.0, 2.0] {
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.demo_editor("loquax");
            views::fx::demo_feed(&mut app, EffectKind::Vocoder);
            app.fx.pos = Some(egui::pos2(0.0, 0.0));
            assert!(probe_window(&mut app, EffectKind::Vocoder, k, true).is_empty());
            let st = &app.fx.loquax;
            assert_eq!(
                st.rows,
                views::fx::loquax::card_rows(lang),
                "{lang:?} at {k}: the rows the designed size makes room for"
            );
            let d = st.display.expect("the display is drawn");
            heights.push((lang, k, d.width() / k, d.height() / k));
        }
    }
    let (_, _, w0, h0) = heights[0];
    for &(lang, k, w, h) in &heights {
        assert!(
            (w - w0).abs() <= 1.0 && (h - h0).abs() <= 1.0,
            "{lang:?} at {k}: the display {w:.1} x {h:.1}, in English at 75 % {w0:.1} x {h0:.1}"
        );
    }
    assert!(h0 >= 300.0, "the display as approved, not squeezed: {h0}");
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
                    let mut app = CantusApp::from_parts(
                        Some(welcome_project()),
                        EngineHandle::dummy(),
                        false,
                    );
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
                        bad.push(format!(
                            "fractus page {page} {lang:?} dark {dark} at {k}: {c}"
                        ));
                    }
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}

/// fxa:fractus — every page of Fractus's cards is one row at its designed size, in English
/// and Spanish (design sheet v2): a card grown past it takes a second row and the display
/// gives up its height on every page.
#[test]
fn fractus_s_pages_are_one_row_each() {
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        app.demo_editor("fractus");
        app.fx.pos = Some(egui::pos2(0.0, 0.0));
        let _ = probe_window(&mut app, EffectKind::Glitch, 1.0, true);
        let (rows, width) = app.fx.fractus.card_rows;
        assert_eq!(
            rows, [1; 4],
            "{lang:?}: every page one row in {width:.0} points"
        );
    }
}

// Caelum's sill answers the pointer as its picture promises (design sheet v2, ADR 0141
// provisional), driven frame by frame with real pointer events through the editor the app
// draws: a click on Infinite's lit pushbutton turns the tank infinite and a second click
// lets it go; Freeze freezes it; a click on the Sustain gauge puts its rider there (ADR
// 0141's amendment); each click is one undo step. // fxa:caelum

use cantus_ui::raster::shoot_events;
use egui::{Event, PointerButton, Pos2};

fn sill_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

fn sill_frames(app: &mut CantusApp, ctx: &egui::Context, size: egui::Vec2, at: Option<Pos2>) {
    shoot_events(
        ctx,
        size,
        1.0,
        6,
        None,
        |f, _| match (f, at) {
            (0, Some(p)) => vec![Event::PointerMoved(p)],
            (1, Some(p)) => vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            (2, Some(p)) => vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            _ => Vec::new(),
        },
        |ui| {
            let c = ui.ctx().clone();
            views::fx::window(app, &c)
        },
    );
}

fn sill_value(app: &CantusApp, key: &str) -> f32 {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project().effect(strip, effect).unwrap().spec.get(key)
}

#[test]
fn caelum_s_sill_buttons_throw_the_endless_settings() {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("caelum");
    views::fx::demo_feed(&mut app, EffectKind::Shimmer);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let kind = EffectKind::Shimmer;
    let size = views::fx::frame::designed_size(kind);
    let click = |app: &mut CantusApp, i: usize| {
        let c = sill_ctx();
        views::fx::frame::shell_of(kind).set_scale(&c, 1.0);
        sill_frames(app, &c, size, None);
        let at = app.fx.caelum.endless_rects()[i].center();
        assert!(at.x > 0.0 && at.y > 0.0, "button {i} is drawn");
        sill_frames(app, &c, size, Some(at));
    };
    // Infinite means infinite: the Sustain starts at 100 % (ADR 0141's amendment).
    assert_eq!(sill_value(&app, "inf"), 0.0);
    assert_eq!(sill_value(&app, "isus"), 1.0);

    click(&mut app, 0);
    assert_eq!(sill_value(&app, "inf"), 1.0, "Infinite lit");
    click(&mut app, 0);
    assert_eq!(sill_value(&app, "inf"), 0.0, "and let go");
    app.undo();
    assert_eq!(sill_value(&app, "inf"), 1.0, "one click, one undo step");

    click(&mut app, 1);
    assert_eq!(sill_value(&app, "freeze"), 1.0, "Freeze lit");

    // The Sustain gauge: a click puts the rider there, one undo step.
    let c = sill_ctx();
    views::fx::frame::shell_of(kind).set_scale(&c, 1.0);
    sill_frames(&mut app, &c, size, None);
    let (gauge, x) = app.fx.caelum.sustain_gauge(0.25).expect("the gauge is drawn");
    assert!(gauge.width() >= 150.0, "the gauge is wide enough to aim");
    sill_frames(&mut app, &c, size, Some(Pos2::new(x, gauge.center().y)));
    assert!(
        (sill_value(&app, "isus") - 0.25).abs() < 0.011,
        "the rider at a quarter: {}",
        sill_value(&app, "isus")
    );
    app.undo();
    assert_eq!(sill_value(&app, "isus"), 1.0, "one click, one undo step");
}
