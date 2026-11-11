//! Round 39, *themes*, the way a user meets them — `CantusApp::assemble` on a data folder,
//! Settings ▸ Appearance, a click on a theme's picture, a restart:
//!
//! - **Cantus Night** (a theme of rounds 24–38, redrawn) chosen from its family's tile:
//!   both its lights set, its tokens, keys and face installed, its espresso room painted,
//!   its verdigris laid over the chosen page's row; kept across a restart.
//! - **Print** (new in round 39, light only) chosen from its tile: the light mode set and
//!   shown, its ink-and-paper room painted, its vermilion playhead in the arrangement; kept
//!   across a restart, with Cantus Night still the dark mode's.
//! - **An old setting naming a formerly retired id** (beta.13's `ultraviolet` and
//!   `parchment`) opens in those themes.
//! - **The page holds every theme**: a tile for each preset, none overlapping, each inside
//!   the page, every name laid out whole.
//!
//! A binary of its own: the installed tokens and the face are process-wide.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::appearance::{Mode, PRESETS};
use cantus_ui::kit::system::{self, Face, System};
use cantus_ui::kit::tokens;
use cantus_ui::theme_added as added;
use cantus_ui::theme_redrawn as redrawn;
use cantus_ui::views::workspace_settings::{open, tile_of, tiled_presets, SettingsPage};
use cantus_ui::CantusApp;
use egui::{Color32, Event, PointerButton, Pos2, Rect, Shape};
use std::path::Path;

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 720.0);

/// The tests share the process's installed theme: one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
    M.lock().unwrap_or_else(|e| e.into_inner())
}

fn data(appearance: Option<&str>) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    if let Some(a) = appearance {
        std::fs::write(d.path().join("appearance.json"), a).unwrap();
    }
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
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
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

    /// Wheel the page under `at` by `dy` points.
    fn wheel(&mut self, at: Pos2, dy: f32) {
        self.frame(vec![
            Event::PointerMoved(at),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, dy),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frames(3);
    }

    /// Tile `n`'s rectangle on Settings ▸ Appearance, wheeled into view.
    fn tile(&mut self, n: usize) -> Rect {
        let rect = |r: &Rig| {
            r.ctx
                .read_response(egui::Id::new(("look-family", n)))
                .map(|x| x.rect)
        };
        for _ in 0..40 {
            let r = rect(self).expect("the tile is laid out");
            let page = self.page();
            if r.top() >= page.top() && r.bottom() <= page.bottom() {
                return r;
            }
            let dy = if r.top() < page.top() { 60.0 } else { -60.0 };
            self.wheel(page.center(), dy);
        }
        panic!("tile {n} never came into view");
    }

    /// The settings page's body, where the tiles scroll.
    fn page(&self) -> Rect {
        let w = self
            .ctx
            .memory(|m| m.area_rect(egui::Id::new("cantus-settings")))
            .expect("Settings is open");
        let side = cantus_ui::views::workspace_settings::SIDEBAR_W;
        Rect::from_min_max(
            Pos2::new(w.left() + side + 24.0, w.top() + 52.0),
            Pos2::new(w.right() - 12.0, w.bottom() - 56.0),
        )
    }

    fn settings(&mut self) {
        self.app.workflow.start.close();
        self.frames(2);
        open(&mut self.app, SettingsPage::Appearance);
        self.frames(30);
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
fn a_restored_theme_and_a_new_one_chosen_in_settings_paint_and_survive_a_restart() {
    let _one = serial();
    let dir = data(None);
    let mut r = boot(dir.path());
    assert_eq!(r.app.appearance.dark.base, "preset:slate_night");
    let slate = cantus_ui::theme_slate::slate_night_tokens();

    // Cantus Night, redrawn: its family's tile sets both lights.
    r.settings();
    let n = tile_of("cantus_night").expect("Cantus Night has a tile");
    assert_eq!(tile_of("cantus_paper"), Some(n), "one tile for the family");
    let tile = r.tile(n);
    r.click(tile.center());
    assert_eq!(r.app.appearance.dark.base, "preset:cantus_night");
    assert_eq!(r.app.appearance.light.base, "preset:cantus_paper");
    assert_eq!(r.app.appearance.mode, Mode::Dark);
    let want = redrawn::cantus_night_tokens();
    let got = tokens::current();
    assert_eq!(
        (
            got.panel,
            got.toolbar,
            got.text,
            got.highlight,
            got.selection_tint
        ),
        (
            want.panel,
            want.toolbar,
            want.text,
            want.highlight,
            want.selection_tint
        )
    );
    assert_eq!(got.system, System::GRAPHITE);
    assert_eq!(system::face(), Face::Sofia);
    assert_ne!(got.panel, slate.panel);
    // It paints: the espresso room, and the chosen page's row through the verdigris.
    r.frames(40);
    let painted = r.frame(Vec::new());
    assert!(
        painted.contains(&want.toolbar),
        "the room is Cantus Night's"
    );
    let row = cantus_ui::kit::color::selected_fill(want.menu, want.selection_tint, 1.0);
    assert!(
        painted.contains(&row),
        "the chosen page's row is laid with the verdigris"
    );

    // Print, new and light only: its tile takes the light mode and shows it.
    let n = tile_of("print").expect("Print has a tile");
    let tile = r.tile(n);
    r.click(tile.center());
    assert_eq!(r.app.appearance.light.base, "preset:print");
    assert_eq!(r.app.appearance.mode, Mode::Light);
    assert_eq!(
        r.app.appearance.dark.base, "preset:cantus_night",
        "the dark mode kept"
    );
    let print = added::print_tokens();
    let got = tokens::current();
    assert_eq!(
        (got.panel, got.text, got.highlight),
        (print.panel, print.text, print.highlight)
    );
    assert_eq!(got.system.face, Face::PlexInk);
    r.frames(10);
    let painted = r.frame(Vec::new());
    assert!(painted.contains(&print.toolbar), "the room is Print's");
    // The arrangement plays with its vermilion playhead.
    r.app.show_settings = false;
    r.app
        .engine
        .seek(cantus_core::time::Ticks(cantus_core::time::PPQ * 8));
    r.app.engine.play();
    r.frames(2);
    let painted = r.frame(Vec::new());
    assert!(
        painted.contains(&print.playhead),
        "Print's playhead is vermilion"
    );

    // Kept: a restart opens in Print, Cantus Night still the dark mode's.
    assert!(dir.path().join("appearance.json").is_file());
    drop(r);
    let r = boot(dir.path());
    assert_eq!(r.app.appearance.mode, Mode::Light);
    assert_eq!(r.app.appearance.light.base, "preset:print");
    assert_eq!(r.app.appearance.dark.base, "preset:cantus_night");
    let back = tokens::current();
    assert_eq!((back.panel, back.highlight), (print.panel, print.highlight));
    assert_eq!(system::face(), Face::PlexInk);
}

#[test]
fn an_old_setting_naming_a_formerly_retired_theme_opens_in_it() {
    let _one = serial();
    // beta.13 wrote these; round 39 first moved them to Slate. They are back.
    let old = r#"{"version":2,"mode":"Dark","dark":{"base":"preset:ultraviolet"},
                  "light":{"base":"preset:parchment"},"themes":[]}"#;
    let dir = data(Some(old));
    let mut r = boot(dir.path());
    assert_eq!(r.app.appearance.dark.base, "preset:ultraviolet");
    assert_eq!(r.app.appearance.light.base, "preset:parchment");
    let uv = redrawn::ultraviolet_tokens();
    let got = tokens::current();
    assert_eq!(
        (got.panel, got.text, got.highlight),
        (uv.panel, uv.text, uv.highlight)
    );
    assert_eq!(got.system, System::SLATE);
    let painted = r.frame(Vec::new());
    assert!(
        painted.contains(&uv.toolbar),
        "Ultraviolet's room is painted"
    );
    // Its light mode is Parchment, redrawn.
    r.app.appearance.mode = Mode::Light;
    r.app.apply_appearance();
    r.frames(2);
    let parch = redrawn::parchment_tokens();
    assert_eq!(tokens::current().panel, parch.panel);
    assert_eq!(system::face(), Face::PlexInk);
    // And an old `theme.json` (before appearance.json) that is exactly round 24's Ink &
    // Gold opens in Ink & Gold, redrawn.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("theme.json"),
        serde_json::to_string(&cantus_ui::theme::ink_and_gold()).unwrap(),
    )
    .unwrap();
    drop(r);
    let _r = boot(dir.path());
    assert_eq!(
        tokens::current().panel,
        redrawn::ink_and_gold_tokens().panel
    );
}

#[test]
fn the_appearance_page_holds_every_theme_legibly() {
    let _one = serial();
    let dir = data(None);
    let mut r = boot(dir.path());
    r.settings();
    // A tile for every shipped theme, each preset on exactly one.
    let mut tiled = tiled_presets();
    tiled.sort_unstable();
    let mut ids: Vec<&str> = PRESETS.iter().map(|(id, _)| *id).collect();
    ids.sort_unstable();
    assert_eq!(tiled, ids);
    // Every tile laid out inside the page, none overlapping another.
    let page = r.page();
    // At most a tile a preset.
    let n = PRESETS.len();
    let mut rects = Vec::new();
    for i in 0..n {
        let Some(resp) = r.ctx.read_response(egui::Id::new(("look-family", i))) else {
            break;
        };
        rects.push(resp.rect);
    }
    for (id, _) in PRESETS {
        let n = tile_of(id).expect(id);
        assert!(n < rects.len(), "{id}'s tile {n} is laid out");
    }
    for (i, a) in rects.iter().enumerate() {
        assert!(
            a.width() >= 120.0 && a.height() >= 56.0,
            "tile {i} is {a:?}"
        );
        assert!(
            a.left() >= page.left() - 1.0 && a.right() <= page.right() + 1.0,
            "tile {i} {a:?} is cut by the page {page:?}"
        );
        for (j, b) in rects.iter().enumerate().skip(i + 1) {
            // Each tile's name sits 22 points under it: the next row starts below that.
            let named = Rect::from_min_max(a.min, a.max + egui::vec2(0.0, 22.0));
            assert!(!named.intersects(*b), "tiles {i} and {j} overlap");
        }
    }
    // The last tile scrolls into view.
    let last = r.tile(rects.len() - 1);
    assert!(last.bottom() <= r.page().bottom());
}
