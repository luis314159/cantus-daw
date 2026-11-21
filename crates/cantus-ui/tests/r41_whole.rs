//! Round 41's new windows read whole (the house rule, ADR 0109: nothing elided, dropped,
//! abbreviated or painted past its card or the window, in English or Spanish): Nitor's
//! editor in each of its design states, Tabula with each of Fragor's five pedals on its
//! board and chosen, and Fides's instrument window in its Play, Edit and Matrix views —
//! each drawn playing at its designed size in the default theme (Slate Lapis) and its light
//! twin, at 100 % and at 75, 150 and 200 %, in both languages, with the kit's clip-aware
//! probe on (cantus-6f found Nitor's Spanish *Low end* card cut at 100 %: "Armónicos del…",
//! "Realce en la tó…", "Banda grave h…"). // r41:no-elision

use cantus_core::fx::tabula::{Pedal, PedalId};
use cantus_core::{Command, EffectKind, EffectSpec, InstrumentKind};
use cantus_engine::EngineHandle;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::shell::Shell;
use cantus_ui::kit::text::probe;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::default_theme()
    } else {
        cantus_ui::theme::default_light_theme()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

/// What the probe saw, one line per word not read whole.
fn seen() -> Vec<String> {
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

/// Draw `app`'s open effect window of `kind` at `k`: every word not read whole.
fn probe_fx(app: &mut CantusApp, kind: EffectKind, k: f32, dark: bool) -> Vec<String> {
    let ctx = context(dark);
    views::fx::frame::shell_of(kind).set_scale(&ctx, k);
    let size = views::fx::frame::designed_size(kind) * k;
    probe::start_clipped();
    cantus_ui::raster::shoot(&ctx, size, 1.0, 3, None, |root| {
        let ctx = root.ctx().clone();
        views::fx::window(app, &ctx);
    });
    seen()
}

/// The scales a window is drawn at: its designed 100 % in both themes, the others dark.
const SCALES: [(f32, bool); 5] = [
    (1.0, true),
    (1.0, false),
    (0.75, true),
    (1.5, true),
    (2.0, true),
];

/// Every pass over both languages, each a list of "where: cut".
fn in_both(mut each: impl FnMut(Locale, &mut Vec<String>)) -> Vec<String> {
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        i18n::set_thread_locale(lang);
        each(lang, &mut bad);
    }
    i18n::set_thread_locale(Locale::En);
    bad
}

#[test]
fn nitors_window_reads_whole_in_both_languages() {
    let _theme_turn = theme_turn();
    let bad = in_both(|lang, bad| {
        for (state, kv) in views::fx::nitor::DESIGN_STATES {
            for (k, dark) in SCALES {
                // The other scales in the starting state only: the layout is the same.
                if k != 1.0 && *state != "start" {
                    continue;
                }
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.demo_editor("nitor");
                let (strip, effect, _) = app.fx.open.expect("Nitor is open");
                if !kv.is_empty() {
                    views::fx::host::DawHost::new(&mut app, strip, effect, "nitor")
                        .set_many(kv, "cmd.fx_preset");
                }
                views::fx::demo_feed(&mut app, EffectKind::Enhancer);
                app.fx.pos = Some(egui::pos2(0.0, 0.0));
                for c in probe_fx(&mut app, EffectKind::Enhancer, k, dark) {
                    bad.push(format!("nitor {state} {lang:?} {k} dark {dark}: {c}"));
                }
            }
        }
    });
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}

#[test]
fn tabula_with_each_of_fragors_pedals_reads_whole_in_both_languages() {
    let _theme_turn = theme_turn();
    let bad = in_both(|lang, bad| {
        for kind in cantus_core::fx::fragor_pedal::KINDS {
            for (k, dark) in SCALES {
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.demo_editor("tabula");
                let (strip, effect, _) = app.fx.open.expect("Tabula is open");
                let spec = EffectSpec::new(kind);
                app.exec(Command::InsertPedal {
                    strip,
                    effect,
                    index: 0,
                    pedal: Box::new(Pedal::new(PedalId(60), spec)),
                    maps: Vec::new(),
                });
                app.fx.tabula.selected = Some(PedalId(60));
                views::fx::demo_feed(&mut app, EffectKind::Board);
                app.fx.pos = Some(egui::pos2(0.0, 0.0));
                for c in probe_fx(&mut app, EffectKind::Board, k, dark) {
                    bad.push(format!("tabula + {kind:?} {lang:?} {k} dark {dark}: {c}"));
                }
            }
        }
    });
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}

#[test]
#[ignore = "Fides's labels are fixed on r41/fides-all; un-ignore once merged"]
fn fides_window_reads_whole_in_every_view_and_both_languages() {
    use cantus_ui::views::inst::View;
    let _theme_turn = theme_turn();
    let kind = InstrumentKind::Resonator;
    let bad = in_both(|lang, bad| {
        for view in [View::Play, View::Edit, View::Matrix] {
            for (k, dark) in SCALES {
                let ctx = context(dark);
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.demo_editor("fides");
                views::inst::demo_feed(&mut app, kind);
                let track = app.inst.open.expect("Fides is open");
                app.inst.view = view;
                Shell::new(("inst", track.0)).set_scale(&ctx, k);
                let size = views::inst::editor_size(kind) * k;
                probe::start_clipped();
                cantus_ui::raster::shoot(&ctx, size, 1.0, 3, None, |ui| {
                    views::inst::window_in(&mut app, ui);
                });
                for c in seen() {
                    bad.push(format!("fides {view:?} {lang:?} {k} dark {dark}: {c}"));
                }
            }
        }
    });
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}

/// Every text painted in a frame of `app`'s open effect window at its designed size, with
/// its colour (each section's, its fallback's or its override's) and where it lies.
fn texts_of(
    app: &mut CantusApp,
    ctx: &egui::Context,
    size: egui::Vec2,
) -> Vec<(String, egui::Color32, egui::Rect)> {
    let mut v = Vec::new();
    for frame in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            views::fx::window(app, &ctx);
        });
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Color32, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => {
                    let rect = t.galley.rect.translate(t.pos.to_vec2());
                    let text = t.galley.text();
                    for sec in &t.galley.job.sections {
                        let c = t.override_text_color.unwrap_or(
                            if sec.format.color == egui::Color32::PLACEHOLDER {
                                t.fallback_color
                            } else {
                                sec.format.color
                            },
                        );
                        let r = sec.byte_range.start.0..sec.byte_range.end.0;
                        let words = text.get(r).unwrap_or("").trim();
                        if !words.is_empty() && t.opacity_factor > 0.99 && c.a() > 0 {
                            out.push((words.to_string(), c, rect));
                        }
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        v.clear();
        for c in &out.shapes {
            walk(&c.shape, &mut v);
        }
    }
    v
}

/// Every word on Nitor's corona — the one knob's label and value among them — reads 4.5:1
/// on the glass in every theme preset, with the units' faces on and off (cantus-6f: in
/// Slate Lapis Paper the sun knob's "AMOUNT" and "35 %" were the theme's dark card ink on
/// the dark glass).
#[test]
fn every_word_on_nitors_corona_reads_on_the_glass_in_every_theme() {
    use cantus_ui::kit::color::contrast;
    use cantus_ui::kit::{skin, Style, Tokens};
    let _theme_turn = theme_turn();
    i18n::set_thread_locale(Locale::En);
    let mut bad = Vec::new();
    for faces in [true, false] {
        skin::set_enabled(faces);
        for (name, make) in cantus_ui::theme::PRESETS {
            let theme = make();
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&theme, &ctx);
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.demo_editor("nitor");
            views::fx::demo_feed(&mut app, EffectKind::Enhancer);
            app.fx.pos = Some(egui::pos2(0.0, 0.0));
            let kind = EffectKind::Enhancer;
            views::fx::frame::shell_of(kind).set_scale(&ctx, 1.0);
            let texts = texts_of(&mut app, &ctx, views::fx::frame::designed_size(kind));
            let glass = app.fx.nitor.corona.expect("the corona is drawn");
            let worn =
                Style::new(Tokens::for_theme(&theme)).skinned(skin::of(skin::Unit::Effect(kind)));
            let ground = skin::display_ink(&worn).ground;
            let on_glass: Vec<_> = texts
                .iter()
                .filter(|(_, _, r)| glass.contains(r.center()))
                .collect();
            // The sun knob's label and value are among them (in a face's capitals or not).
            let amount = i18n::t("fx.nitor.amount").to_uppercase();
            assert!(
                on_glass.iter().any(|(t, _, _)| t.to_uppercase() == amount)
                    && on_glass.iter().any(|(t, _, _)| t.starts_with("35")),
                "{name} faces {faces}: the sun knob's label and value are on the glass: {:?}",
                on_glass.iter().map(|(t, _, _)| t).collect::<Vec<_>>()
            );
            for (t, c, _) in on_glass {
                let x = contrast(*c, ground);
                if x < 4.5 {
                    bad.push(format!("{name} faces {faces}: {t:?} {x:.2}:1"));
                }
            }
        }
    }
    skin::set_enabled(true);
    bad.sort();
    bad.dedup();
    assert!(
        bad.is_empty(),
        "words under 4.5:1 on the glass:
{}",
        bad.join(
            "
"
        )
    );
}

/// This binary's tests take turns (docs/RELEASING.md L5): `theme::apply` installs its palette
/// and the kit's tokens for the whole process, so a test applying a theme repainted another's
/// words mid-measure (beta.16's rehearsal, macOS: fx_limiter's dark demo cut "GAIN REDUCTION").
/// The app has one interface thread; only tests run several.
fn theme_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    // Every test starts in the app's own look, whatever the test before it applied.
    cantus_ui::theme::apply(
        &cantus_ui::theme::default_theme(),
        &egui::Context::default(),
    );
    turn
}
