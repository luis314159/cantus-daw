//! The Settings window scrolls to its last section, *Plug-ins* (the owner, 2026-10-22: the
//! window would not scroll, so *Scan for plug-ins* could not be reached).

use cantus_engine::EngineHandle;
use cantus_ui::{welcome_project, CantusApp};
use egui::{pos2, vec2, Event, MouseWheelUnit, Pos2, Rect};

struct Win {
    app: CantusApp,
    ctx: egui::Context,
    frame: u64,
    size: egui::Vec2,
}

impl Win {
    fn new(size: egui::Vec2) -> Win {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        let (engine, _processor) = EngineHandle::manual(48_000.0);
        let app = CantusApp::from_parts(Some(welcome_project()), engine, false);
        Win {
            app,
            ctx,
            frame: 0,
            size,
        }
    }

    /// One frame of the whole window; the texts drawn, with where they are.
    fn step(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |root| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(app, root, &mut frame);
        });
        out.textures_delta.clear();
        let mut texts = Vec::new();
        for c in &out.shapes {
            collect(&c.shape, c.clip_rect, &mut texts);
        }
        texts
    }
}

fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect)>) {
    match shape {
        egui::Shape::Text(t) => {
            let r = t.visual_bounding_rect();
            if clip.intersects(r) {
                out.push((t.galley.text().to_string(), r));
            }
        }
        egui::Shape::Vec(v) => {
            for s in v {
                collect(s, clip, out);
            }
        }
        _ => {}
    }
}

fn find<'a>(texts: &'a [(String, Rect)], word: &str) -> Option<&'a Rect> {
    texts.iter().find(|(t, _)| t == word).map(|(_, r)| r)
}

#[test]
fn the_settings_window_scrolls_to_the_plug_ins() {
    // A laptop's screen (the owner's window showed about 880 points of height).
    let mut w = Win::new(vec2(1366.0, 900.0));
    for _ in 0..30 {
        w.step(Vec::new());
    }
    w.app.show_settings = true;
    let mut texts = Vec::new();
    for _ in 0..10 {
        texts = w.step(Vec::new());
    }
    let title = *find(&texts, "Settings").expect("the window is open");
    assert!(
        find(&texts, "Scan for plug-ins").is_none(),
        "the plug-ins start below the window's fold"
    );
    // The wheel, over the window's body (below its title, left of its scroll bar).
    let at = pos2(title.left() + 120.0, title.bottom() + 200.0);
    for _ in 0..200 {
        texts = w.step(vec![
            Event::PointerMoved(at),
            Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: vec2(0.0, -3.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ]);
        if find(&texts, "Scan for plug-ins").is_some() {
            break;
        }
    }
    for _ in 0..20 {
        texts = w.step(vec![Event::PointerMoved(at)]);
    }
    assert!(
        find(&texts, "Scan for plug-ins").is_some(),
        "the wheel reaches Scan for plug-ins"
    );
}

/// Every window drawn by the kit claims the area it paints — or egui gives its wheel to
/// whatever lies under it, and its lists cannot scroll: Settings, Appearance, Sound packs,
/// Updates, Channels & routing.
#[test]
fn every_kit_window_claims_what_it_paints() {
    let cases: [(&str, fn(&mut CantusApp)); 5] = [
        ("cantus-settings", |a| a.show_settings = true),
        ("cantus-appearance", |a| a.show_theme_editor = true),
        ("cantus-content", |a| a.samples.window.open = true),
        ("cantus-update", |a| a.update.open = true),
        ("cantus-audio-routing", |a| a.audio_setup.routing = true),
    ];
    for (id, open) in cases {
        let mut w = Win::new(vec2(1366.0, 900.0));
        for _ in 0..5 {
            w.step(Vec::new());
        }
        open(&mut w.app);
        for _ in 0..10 {
            w.step(Vec::new());
        }
        let r = w
            .ctx
            .memory(|m| m.area_rect(egui::Id::new(id)))
            .unwrap_or_else(|| panic!("{id}: open"));
        assert!(
            r.width() > 300.0 && r.height() > 200.0,
            "{id}: its area is what it paints, not {r:?}"
        );
        let inside = r.center();
        assert_eq!(
            w.ctx.layer_id_at(inside).map(|l| l.id),
            Some(egui::Id::new(id)),
            "{id}: the pointer over it is over it"
        );
    }
}
