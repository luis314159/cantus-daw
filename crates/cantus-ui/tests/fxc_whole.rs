//! fxc — wave C's windows (Purus, Comparatio) never cut or clip a word, in English and
//! Spanish at 75 … 200 %, in Slate and Paper, every colourway and every design state (the
//! probe's `start_clipped`); the cards take the rows each editor's `card_rows` says, and the
//! display keeps its size in both languages (the window grows instead); the controls drawn
//! on the objects answer a real click, each one undo step.

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

fn open(unit: &'static str, kind: EffectKind, kv: &[(&str, f32)]) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(unit);
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, unit).set_many(kv, "cmd.fx_preset");
    }
    views::fx::demo_feed(&mut app, kind);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn states(unit: &str) -> &'static [(&'static str, &'static [(&'static str, f32)])] {
    match unit {
        "purus" => views::fx::purus::DESIGN_STATES,
        _ => views::fx::comparatio::DESIGN_STATES,
    }
}

#[test]
fn wave_c_s_windows_never_cut_or_clip_a_word() {
    let _theme_turn = theme_turn();
    let mut bad = Vec::new();
    for (unit, kind) in [
        ("purus", EffectKind::Repair),
        ("comparatio", EffectKind::Reference),
    ] {
        for lang in [Locale::En, Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            for dark in [true, false] {
                for k in [0.75f32, 1.0, 2.0] {
                    let u = skin::Unit::Effect(kind);
                    let ways: Vec<&str> = skin::colourways(u).iter().map(|w| w.id).collect();
                    for (state, kv) in states(unit) {
                        // Every colourway at 100 %, the face alone at the other scales.
                        for w in ways.iter().take(if k == 1.0 { 3 } else { 1 }) {
                            skin::set_colourway(skin::colourway_unit(u).expect("a unit"), w);
                            let mut app = open(unit, kind, kv);
                            for c in probe_window(&mut app, kind, k, dark) {
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
    }
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "words not read whole:\n{}", bad.join("\n"));
}

/// The owner's rule: in Spanish the display does not shrink; the window grows. In both
/// languages at 75, 100 and 200 % the cards take the rows `card_rows` says and the display
/// is the same size, scaled.
#[test]
fn the_displays_keep_their_size_in_every_language_and_scale() {
    let _theme_turn = theme_turn();
    for (unit, kind) in [
        ("purus", EffectKind::Repair),
        ("comparatio", EffectKind::Reference),
    ] {
        let mut sizes = Vec::new();
        for lang in [Locale::En, Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            for k in [0.75f32, 1.0, 2.0] {
                let mut app = open(unit, kind, &[]);
                let _ = probe_window(&mut app, kind, k, true);
                let (rows, want, d) = match unit {
                    "purus" => (
                        app.fx.purus.rows,
                        views::fx::purus::card_rows(lang),
                        app.fx.purus.display,
                    ),
                    _ => (
                        app.fx.comparatio.rows,
                        views::fx::comparatio::card_rows(lang),
                        app.fx.comparatio.display,
                    ),
                };
                assert_eq!(
                    rows, want,
                    "{unit} {lang:?} at {k}: the rows the designed size makes room for"
                );
                let d = d.expect("the display is drawn");
                sizes.push((lang, k, d.width() / k, d.height() / k));
            }
        }
        let (_, _, w0, h0) = sizes[0];
        for &(lang, k, w, h) in &sizes {
            assert!(
                (w - w0).abs() <= 1.0 && (h - h0).abs() <= 1.0,
                "{unit} {lang:?} at {k}: the display {w:.1} x {h:.1}, English at 75 % {w0:.1} x {h0:.1}"
            );
        }
    }
}

/// Frames of `app`'s window at 100 %, the pointer pressed and released at `at` (or no
/// pointer).
fn frames(app: &mut CantusApp, kind: EffectKind, at: Option<egui::Pos2>) {
    use egui::{Event, PointerButton};
    let ctx = context(true);
    views::fx::frame::shell_of(kind).set_scale(&ctx, 1.0);
    let size = views::fx::frame::designed_size(kind);
    cantus_ui::raster::shoot_events(
        &ctx,
        size,
        1.0,
        10,
        None,
        |f, _| match (f, at) {
            (4, Some(p)) => vec![Event::PointerMoved(p)],
            (5, Some(p)) | (6, Some(p)) => vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed: f == 5,
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

fn value(app: &CantusApp, key: &str) -> f32 {
    let (strip, effect, _) = app.fx.open.expect("open");
    app.project()
        .effect(strip, effect)
        .map(|e| e.spec.get(key))
        .unwrap_or(f32::NAN)
}

/// Purus's bench: each cartridge's toggle switches its module, a key to hear what it takes
/// out, the mains keys and the decay's keys — each press one undo step.
#[test]
fn purus_s_bench_answers_the_pointer() {
    let _theme_turn = theme_turn();
    let mut app = open("purus", EffectKind::Repair, &[]);
    frames(&mut app, EffectKind::Repair, None);
    let bench = app.fx.purus.bench.clone().expect("the bench is laid out");
    for sp in &bench.spots {
        let key = match sp.act {
            views::fx::purus::Act::Toggle(k) | views::fx::purus::Act::Set(k, _) => k,
        };
        let before = value(&app, key);
        frames(&mut app, EffectKind::Repair, Some(sp.rect.center()));
        let after = value(&app, key);
        match sp.act {
            views::fx::purus::Act::Toggle(_) => {
                assert_ne!(before >= 0.5, after >= 0.5, "{key}: switched")
            }
            views::fx::purus::Act::Set(_, v) => assert_eq!(after, v, "{key}: set"),
        }
        app.undo();
        assert_eq!(value(&app, key), before, "{key}: undone");
    }
}

/// Comparatio's console: the selector swaps between the mix and the reference, the keys
/// A, B and X choose what is heard, an answer counts and moves to the next trial.
#[test]
fn comparatio_s_console_answers_the_pointer() {
    let _theme_turn = theme_turn();
    use views::fx::comparatio::Act;
    let mut app = open("comparatio", EffectKind::Reference, &[]);
    frames(&mut app, EffectKind::Reference, None);
    let spots = app.fx.comparatio.spots.clone();
    let at = |a: Act| {
        spots
            .iter()
            .find(|s| s.act == a)
            .expect("a spot")
            .rect
            .center()
    };
    frames(&mut app, EffectKind::Reference, Some(at(Act::Swap)));
    assert_eq!(
        value(&app, "listen"),
        1.0,
        "the selector turns to the reference"
    );
    frames(&mut app, EffectKind::Reference, Some(at(Act::Swap)));
    assert_eq!(value(&app, "listen"), 0.0, "and back to the mix");
    frames(&mut app, EffectKind::Reference, Some(at(Act::Listen(2.0))));
    assert_eq!(value(&app, "listen"), 2.0, "X");
    let trial = value(&app, "trial");
    frames(&mut app, EffectKind::Reference, Some(at(Act::Answer(true))));
    assert_eq!(
        value(&app, "trial"),
        trial + 1.0,
        "an answer moves to the next trial"
    );
    assert_eq!(app.fx.comparatio.blind.answers.len(), 1);
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
