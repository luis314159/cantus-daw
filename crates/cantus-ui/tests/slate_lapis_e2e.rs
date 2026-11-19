//! **Slate Lapis**, the default since 2026-11-18 (ADR 0111, amended), the way a user meets
//! it — `CantusApp::assemble` on a fresh data folder installs Slate's structure in its
//! night-blue room with the lapis accent, lays the lapis over the selected page's row on
//! Settings ▸ Appearance (its picture the first), paints the arrangement's playhead and the
//! ruler's bar numbers in the logo's gold (`lapis2`). Slate, round 39's default, chosen on
//! its picture, is still there after a restart; a beta.14–15 file that never chose a theme
//! opens in Slate Lapis.
//!
//! A binary of its own: the installed tokens and the face are process-wide.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::system::{self, Face, System};
use cantus_ui::kit::tokens;
use cantus_ui::theme_slate_lapis as lapis;
use cantus_ui::views::workspace_settings::{open, tile_of, SettingsPage};
use cantus_ui::CantusApp;
use egui::{Color32, Event, PointerButton, Pos2, Shape};
use std::path::Path;

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 720.0);

fn data() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    d
}

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
}

fn boot(dir: &Path) -> Rig {
    std::env::set_var("CANTUS_DATA_DIR", dir);
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::demos::startup_project()),
        engine,
        None,
        None,
        None,
        false,
    );
    let mut r = Rig { app, ctx, t: 0.0 };
    r.frames(3);
    r
}

impl Rig {
    /// One frame with these events; every colour it painted.
    fn frame(&mut self, events: Vec<Event>) -> Vec<Color32> {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        let mut colours = Vec::new();
        for c in &out.shapes {
            colours_of(&c.shape, &mut colours);
        }
        colours
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        self.frames(2);
    }
}

/// The colours a shape paints with.
fn colours_of(shape: &Shape, out: &mut Vec<Color32>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| colours_of(s, out)),
        Shape::Mesh(m) => out.extend(m.vertices.iter().map(|v| v.color)),
        Shape::Rect(r) => out.extend([r.fill, r.stroke.color]),
        Shape::LineSegment { stroke, .. } => out.push(stroke.color),
        Shape::Path(p) => {
            out.push(p.fill);
            if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                out.push(c);
            }
        }
        Shape::Circle(c) => out.extend([c.fill, c.stroke.color]),
        Shape::Text(t) => {
            out.push(t.fallback_color);
            out.push(t.override_text_color.unwrap_or(t.fallback_color));
            out.extend(t.galley.job.sections.iter().map(|s| s.format.color));
        }
        _ => {}
    }
}

#[test]
fn slate_lapis_is_the_default_paints_lapis_and_gold_and_a_chosen_slate_survives_a_restart() {
    let dir = data();
    let mut r = boot(dir.path());

    // A fresh install is Slate Lapis Night (ADR 0111, amended 2026-11-18): Slate's
    // structure, face and text in a night-blue room, the lapis accent and its tint, the
    // gold readouts and playhead — installed, the mode following the default.
    let slate = cantus_ui::theme_slate::slate_night_tokens();
    let want = lapis::slate_lapis_night_tokens();
    assert_eq!(r.app.appearance.dark.base, "preset:slate_lapis_night");
    assert_eq!(r.app.appearance.light.base, "preset:slate_lapis_paper");
    assert!(r.app.appearance.dark.follows_default);
    let got = tokens::current();
    assert_eq!(got.system, System::SLATE);
    assert_eq!(system::face(), Face::Barlow);
    assert_eq!(
        (got.panel, got.toolbar, got.well, got.strip),
        (want.panel, want.toolbar, want.well, want.strip)
    );
    assert_ne!(
        got.panel, slate.panel,
        "the room is night blue, not Slate's grey"
    );
    assert_eq!((got.text, got.text_dim), (slate.text, slate.text_dim));
    assert_eq!(got.selection_tint, want.selection_tint);
    assert!(got.selection_tint.a() > 0);
    assert_eq!((got.readout, got.loop_edge), (want.readout, want.loop_edge));
    assert_eq!(got.highlight, Color32::from_rgb(0x7e, 0xa2, 0xf6));
    assert_eq!(got.selection, want.selection);
    assert_eq!(got.playhead, lapis::gold(true));
    assert_eq!(cantus_ui::theme::p().gold, want.highlight);

    // The start page closed, as a user closes it; Settings ▸ Appearance: the default's
    // picture comes first, and the page chosen in Settings' list wears the lapis — its row
    // through the tint (once the dialog's fade has run).
    r.app.workflow.start.close();
    r.frames(2);
    open(&mut r.app, SettingsPage::Appearance);
    r.frames(3);
    assert_eq!(tile_of("slate_lapis_night"), Some(0));
    assert_eq!(tile_of("slate_night"), Some(1));
    r.frames(40);
    let painted = r.frame(Vec::new());
    let row = cantus_ui::kit::color::selected_fill(want.menu, want.selection_tint, 1.0);
    assert_ne!(row, want.menu);
    assert!(
        painted.contains(&row),
        "the selected page's row is laid with the lapis"
    );

    // The arrangement paints its playhead in the gold: settings closed, the song two bars
    // in, playing.
    r.app.show_settings = false;
    r.app
        .engine
        .seek(cantus_core::time::Ticks(cantus_core::time::PPQ * 8));
    r.app.engine.play();
    r.frames(2);
    let painted = r.frame(Vec::new());
    assert!(
        painted.contains(&lapis::gold(true)),
        "the playhead is drawn in the logo's gold"
    );
    assert!(
        !painted.contains(&slate.playhead),
        "nothing is left in Slate's azure playhead"
    );
    assert!(
        painted.contains(&want.readout_soft),
        "the ruler's bar numbers are read in the second gold"
    );
    r.app.engine.stop();
    r.frames(2);

    // Slate, round 39's default, chosen on its picture: Slate Night installed, azure and
    // grey, none of Lapis's three.
    open(&mut r.app, SettingsPage::Appearance);
    r.frames(3);
    let tile = r
        .ctx
        .read_response(egui::Id::new((
            "look-family",
            tile_of("slate_night").unwrap(),
        )))
        .expect("Slate's picture is drawn on Settings ▸ Appearance")
        .rect;
    r.click(tile.center());
    assert_eq!(r.app.appearance.dark.base, "preset:slate_night");
    assert_eq!(r.app.appearance.light.base, "preset:slate_paper");
    assert!(!r.app.appearance.dark.follows_default);
    let got = tokens::current();
    assert_eq!((got.highlight, got.panel), (slate.highlight, slate.panel));
    assert_eq!(got.selection_tint, Color32::TRANSPARENT);

    // The choice is written, and a restart opens in it: a deliberate Slate stays Slate.
    assert!(dir.path().join("appearance.json").is_file());
    drop(r);
    let r = boot(dir.path());
    assert_eq!(r.app.appearance.dark.base, "preset:slate_night");
    let back = tokens::current();
    assert_eq!((back.highlight, back.panel), (slate.highlight, slate.panel));
    assert_eq!(back.playhead, slate.playhead);
    drop(r);

    // A file beta.14–15 wrote for a user who only switched the mode to the system's and
    // back (version 2, untouched Slate in both modes): no theme was chosen, so the next
    // start opens in the default, Slate Lapis, and writes nothing until the look changes.
    let dir = data();
    let mut v2 = serde_json::to_value(cantus_ui::kit::appearance::Appearance::default()).unwrap();
    v2["version"] = 2.into();
    for (mode, id) in [("dark", "slate_night"), ("light", "slate_paper")] {
        let slot = v2[mode].as_object_mut().unwrap();
        slot.remove("follows_default");
        slot.insert("base".into(), format!("preset:{id}").into());
    }
    let file = dir.path().join("appearance.json");
    std::fs::write(&file, v2.to_string()).unwrap();
    let r = boot(dir.path());
    assert_eq!(r.app.appearance.dark.base, "preset:slate_lapis_night");
    assert_eq!(r.app.appearance.light.base, "preset:slate_lapis_paper");
    let got = tokens::current();
    assert_eq!((got.highlight, got.panel), (want.highlight, want.panel));
    assert_eq!(got.playhead, lapis::gold(true));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        v2.to_string(),
        "the file is the user's: read, not rewritten"
    );

    // The default itself, the theme a fresh install and any unreadable file get.
    assert_eq!(cantus_ui::theme::default_theme().name, "Slate Lapis Night");
    assert_eq!(
        cantus_ui::theme::default_theme().kit_tokens().highlight,
        want.highlight
    );
    assert_eq!(
        cantus_ui::theme::default_light_theme().name,
        "Slate Lapis Paper"
    );
}
