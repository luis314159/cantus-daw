//! Round 39: **Slate Lapis**, the optional theme that goes with the logo's metal, chosen the
//! way a user chooses it — `CantusApp::assemble` on a fresh data folder, Settings ▸
//! Appearance, a click on its family's picture — installs Slate's structure in its
//! night-blue room with the lapis accent, lays the lapis over the selected page's row,
//! paints the arrangement's playhead and the ruler's bar numbers in the logo's gold
//! (`lapis2`), and is still there after a restart. Slate stays the default.
//!
//! A binary of its own: the installed tokens and the face are process-wide.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::system::{self, Face, System};
use cantus_ui::kit::tokens;
use cantus_ui::theme_slate_lapis as lapis;
use cantus_ui::views::workspace_settings::{open, SettingsPage};
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
fn slate_lapis_chosen_in_settings_paints_lapis_and_gold_and_survives_a_restart() {
    let dir = data();
    let mut r = boot(dir.path());

    // A fresh install is Slate Night, with Slate's azure playhead.
    let slate = cantus_ui::theme_slate::slate_night_tokens();
    assert_eq!(r.app.appearance.dark.base, "preset:slate_night");
    assert_eq!(tokens::current().highlight, slate.highlight);
    assert_eq!(tokens::current().playhead, slate.playhead);

    // The start page closed, as a user closes it; Settings ▸ Appearance, then a click on
    // Slate Lapis's picture (the family row's second).
    r.app.workflow.start.close();
    r.frames(2);
    open(&mut r.app, SettingsPage::Appearance);
    r.frames(3);
    let tile = r
        .ctx
        .read_response(egui::Id::new(("look-family", 1usize)))
        .expect("Slate Lapis's picture is drawn on Settings ▸ Appearance")
        .rect;
    r.click(tile.center());
    assert_eq!(r.app.appearance.dark.base, "preset:slate_lapis_night");
    assert_eq!(r.app.appearance.light.base, "preset:slate_lapis_paper");

    // Slate's structure, face and text in a night-blue room, the lapis accent and its
    // tint, the gold readouts and playhead — installed.
    let want = lapis::slate_lapis_night_tokens();
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
    // The page chosen in Settings' list wears the lapis: its row through the tint (once
    // the dialog's fade has run).
    r.frames(40);
    let painted = r.frame(Vec::new());
    let row = cantus_ui::kit::color::selected_fill(want.menu, want.selection_tint, 1.0);
    assert_ne!(row, want.menu);
    assert!(
        painted.contains(&row),
        "the selected page's row is laid with the lapis"
    );
    assert_eq!(got.highlight, Color32::from_rgb(0x7e, 0xa2, 0xf6));
    assert_eq!(got.selection, want.selection);
    assert_eq!(got.playhead, lapis::gold(true));
    assert_eq!(cantus_ui::theme::p().gold, want.highlight);

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

    // The choice is written, and a restart opens in it.
    assert!(dir.path().join("appearance.json").is_file());
    drop(r);
    let r = boot(dir.path());
    assert_eq!(r.app.appearance.dark.base, "preset:slate_lapis_night");
    let back = tokens::current();
    assert_eq!(back.highlight, want.highlight);
    assert_eq!(back.playhead, lapis::gold(true));
    assert_eq!(back.system, System::SLATE);

    // Slate is still the default, unchanged: none of the three is Slate's.
    assert_eq!(cantus_ui::theme::default_theme().name, "Slate Night");
    let def = cantus_ui::theme::default_theme().kit_tokens();
    assert_eq!(def.highlight, slate.highlight);
    assert_eq!(def.panel, slate.panel);
    for c in [
        def.selection_tint,
        def.readout,
        def.readout_soft,
        def.loop_edge,
    ] {
        assert_eq!(c, Color32::TRANSPARENT);
    }
}
