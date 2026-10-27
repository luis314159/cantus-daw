//! Text after a theme change (round 23, BACKLOG §1: "sometimes the labels look strange
//! after changing the theme"). The same view drawn before and after a round trip of
//! themes — and after switching *into* a theme, compared with starting in it — must be the
//! same pixels; with a zoom change and a language change in between as well.
//!
//! The cause it pins down: a dark↔light switch changes egui's text options (its colour
//! transfer function), so egui rebuilds its font atlas and every glyph moves; the kit's
//! cache of laid-out text must notice and lay its text out again, or labels keep pointing
//! at where the glyphs used to be. The atlas here is 16 384 texels wide, as on a desktop
//! GPU, so a whole session's glyphs fit in one row — the case the old guess (a drop in
//! the atlas's fill ratio) could not see.
//!
//! One test binary of its own: a theme is process-wide state.

use cantus_engine::EngineHandle;
use cantus_ui::kit::text::{self, Elide, Policy, Role, Text};
use cantus_ui::kit::{self, appearance::Mode};
use cantus_ui::raster::{Canvas, Textures};
use cantus_ui::{design, i18n, theme, views, welcome_project, CantusApp};
use egui::{Pos2, Rect, Vec2};

/// The font atlas's width on a desktop GPU (glow's `GL_MAX_TEXTURE_SIZE`).
const SIDE: usize = 16 * 1024;

/// One context drawn frame after frame, its textures kept across runs (egui sends only
/// what changed), as a window's are.
struct Shots {
    ctx: egui::Context,
    tex: Textures,
    t: f64,
}

impl Shots {
    fn new() -> Shots {
        let ctx = egui::Context::default();
        design::install_fonts(&ctx);
        design::install_style(&ctx);
        Shots {
            ctx,
            tex: Textures::default(),
            t: 0.0,
        }
    }

    fn run(&mut self, size: Vec2, frames: usize, ui: &mut dyn FnMut(&mut egui::Ui)) -> Canvas {
        let mut last = None;
        for _ in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                time: Some(self.t),
                predicted_dt: 0.25,
                max_texture_side: Some(SIDE),
                ..Default::default()
            };
            self.t += 0.25;
            let mut out = self.ctx.run_ui(input, |root| ui(root));
            self.tex.update(&out.textures_delta);
            out.textures_delta.clear();
            last = Some(out);
        }
        let out = last.expect("a frame");
        let ppp = out.pixels_per_point;
        let prims = self.ctx.tessellate(out.shapes, ppp);
        let bg = self.ctx.global_style().visuals.panel_fill;
        let mut c = Canvas::new(
            (size.x * ppp).round() as usize,
            (size.y * ppp).round() as usize,
            bg,
        );
        c.paint(&prims, &self.tex, ppp);
        c
    }
}

/// Pixels that differ between two pictures of the same size.
fn diff(a: &Canvas, b: &Canvas) -> usize {
    assert_eq!((a.width, a.height), (b.width, b.height), "the same size");
    (0..a.height)
        .flat_map(|y| (0..a.width).map(move |x| (x, y)))
        .filter(|&(x, y)| a.pixel(x, y) != b.pixel(x, y))
        .count()
}

fn save(name: &str, c: &Canvas) {
    if let Some(dir) = std::env::var_os("CANTUS_SHOTS") {
        let _ = c.write_png(&std::path::Path::new(&dir).join(format!("theme-{name}.png")));
    }
}

const KIT: Vec2 = Vec2::new(420.0, 300.0);

/// Every text role of the kit, a key and a plain string each: text only, so nothing but
/// the kit's laid-out text is on the screen.
fn kit_scene(ui: &mut egui::Ui) {
    let s = kit::Style::current();
    let roles = [
        Role::Micro,
        Role::Label,
        Role::Body,
        Role::Control,
        Role::Title,
        Role::Section,
        Role::Value,
    ];
    kit::with_brush(ui.painter(), |b| {
        for (i, role) in roles.into_iter().enumerate() {
            let y = 8.0 + i as f32 * 40.0;
            let r = Rect::from_min_size(Pos2::new(8.0, y), Vec2::new(200.0, 18.0));
            let _ = text::paint(
                b,
                r,
                &Text::key("export_audio.title", role, s.t.text).policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let r = Rect::from_min_size(Pos2::new(210.0, y), Vec2::new(200.0, 18.0));
            let _ = text::paint(
                b,
                r,
                &Text::new("Grand Piano 120.00", role, s.t.text),
                s.tk(),
            );
        }
    });
}

#[test]
fn a_round_trip_of_themes_leaves_the_text_as_it_was() {
    i18n::set_thread_locale(i18n::Locale::En);
    let (dark, light, gold, parchment) = (
        theme::slate(),
        theme::paper(),
        theme::ink_and_gold(),
        theme::parchment(),
    );
    let mut s = Shots::new();
    theme::apply(&dark, &s.ctx);
    let a0 = s.run(KIT, 4, &mut kit_scene);
    let again = s.run(KIT, 2, &mut kit_scene);
    assert_eq!(
        diff(&a0, &again),
        0,
        "the scene is the same frame after frame"
    );

    // Into a light theme: what starting in it gives.
    theme::apply(&light, &s.ctx);
    let b1 = s.run(KIT, 3, &mut kit_scene);
    let mut fresh = Shots::new();
    theme::apply(&light, &fresh.ctx);
    let b0 = fresh.run(KIT, 4, &mut kit_scene);
    save("kit-light-switched", &b1);
    save("kit-light-fresh", &b0);
    assert_eq!(diff(&b1, &b0), 0, "switched into Paper is Paper");

    // And back: what it was.
    theme::apply(&dark, &s.ctx);
    let a1 = s.run(KIT, 3, &mut kit_scene);
    save("kit-dark-back", &a1);
    assert_eq!(diff(&a1, &a0), 0, "Slate → Paper → Slate is Slate");

    // A zoom in between.
    theme::apply(&light, &s.ctx);
    s.ctx.set_zoom_factor(1.25);
    s.run(KIT, 3, &mut kit_scene);
    s.ctx.set_zoom_factor(1.0);
    theme::apply(&dark, &s.ctx);
    let a2 = s.run(KIT, 3, &mut kit_scene);
    assert_eq!(diff(&a2, &a0), 0, "with a zoom in between");

    // A language in between.
    i18n::set_thread_locale(i18n::Locale::Es);
    theme::apply(&parchment, &s.ctx);
    s.run(KIT, 3, &mut kit_scene);
    i18n::set_thread_locale(i18n::Locale::En);
    theme::apply(&gold, &s.ctx);
    s.run(KIT, 3, &mut kit_scene);
    theme::apply(&dark, &s.ctx);
    let a3 = s.run(KIT, 3, &mut kit_scene);
    assert_eq!(
        diff(&a3, &a0),
        0,
        "with a language and every theme in between"
    );
    // Each dark↔light switch rebuilt egui's atlas, and the kit saw every one.
    let rebuilds = kit::cache::with_raw(&s.ctx, |c| c.stats.font_rebuilds);
    assert!(rebuilds >= 6, "{rebuilds} rebuilds noticed");
    // A theme switch keeps the kit's fader cap.
    assert_eq!(
        s.ctx.global_style().visuals.handle_shape,
        egui::style::HandleShape::Rect { aspect_ratio: 0.38 }
    );

    views_round_trip();
    every_theme_round_trip();
    every_theme_in_the_views();
}

/// Round 25 (ADR 0046): every shipped theme, switched into from the default and back,
/// is what starting in it gives, and leaves the default as it was.
fn every_theme_round_trip() {
    i18n::set_thread_locale(i18n::Locale::En);
    let home = theme::default_theme();
    let mut s = Shots::new();
    theme::apply(&home, &s.ctx);
    let a0 = s.run(KIT, 4, &mut kit_scene);
    for (key, make) in theme::PRESETS {
        let t = make();
        theme::apply(&t, &s.ctx);
        let switched = s.run(KIT, 3, &mut kit_scene);
        let mut fresh = Shots::new();
        theme::apply(&t, &fresh.ctx);
        let started = fresh.run(KIT, 4, &mut kit_scene);
        let name = key.trim_start_matches("theme.");
        save(&format!("kit-{name}-switched"), &switched);
        assert_eq!(
            diff(&switched, &started),
            0,
            "{key}: switched into it is what starting in it gives"
        );
        theme::apply(&home, &s.ctx);
        let back = s.run(KIT, 3, &mut kit_scene);
        assert_eq!(
            diff(&back, &a0),
            0,
            "Cantus Night → {key} → Cantus Night is Cantus Night"
        );
    }
}

/// The real views through every shipped theme, each given to the light mode in turn and
/// shown through the Appearance page's path; dark (Cantus Night) after each is the same.
fn every_theme_in_the_views() {
    const SIZE: Vec2 = Vec2::new(1024.0, 600.0);
    let mut s = Shots::new();
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let dir = tempfile::tempdir().unwrap();
    app.settings_path = dir.path().join("settings.json");
    app.appearance_path = dir.path().join("appearance.json");
    app.attach(&s.ctx);
    let mut mode: Option<Mode> = None;
    app.appearance.mode = Mode::Dark;
    app.apply_appearance();
    let a0 = s.run(SIZE, 6, &mut scene(&mut app, &mut mode));
    for (id, _) in kit::appearance::PRESETS {
        app.appearance.assign(Mode::Light, format!("preset:{id}"));
        mode = Some(Mode::Light);
        s.run(SIZE, 4, &mut scene(&mut app, &mut mode));
        mode = Some(Mode::Dark);
        let back = s.run(SIZE, 4, &mut scene(&mut app, &mut mode));
        assert_eq!(diff(&back, &a0), 0, "Dark → {id} → Dark is Dark");
    }
}

/// The transport and the arrangement; a mode waiting in `mode` is applied first, in the
/// middle of the frame, as the Appearance page does.
fn scene<'a>(app: &'a mut CantusApp, mode: &'a mut Option<Mode>) -> impl FnMut(&mut egui::Ui) + 'a {
    move |ui: &mut egui::Ui| {
        if let Some(m) = mode.take() {
            app.appearance.mode = m;
            app.apply_appearance();
        }
        egui::Panel::top(egui::Id::new("t")).show(ui, |ui| views::transport::show(app, ui));
        egui::CentralPanel::default().show(ui, |ui| views::arrangement::show(app, ui));
    }
}

/// The real views, switched the way the Appearance page switches them (mid-frame, through
/// `apply_appearance`).
fn views_round_trip() {
    const SIZE: Vec2 = Vec2::new(1280.0, 720.0);
    let mut s = Shots::new();
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let dir = tempfile::tempdir().unwrap();
    app.settings_path = dir.path().join("settings.json");
    app.appearance_path = dir.path().join("appearance.json");
    app.attach(&s.ctx);
    let mut mode: Option<Mode> = None;
    app.appearance.mode = Mode::Dark;
    app.apply_appearance();
    let a0 = s.run(SIZE, 6, &mut scene(&mut app, &mut mode));
    let again = s.run(SIZE, 2, &mut scene(&mut app, &mut mode));
    assert_eq!(
        diff(&a0, &again),
        0,
        "the views are the same frame after frame"
    );
    for round in 0..3 {
        mode = Some(Mode::Light);
        s.run(SIZE, 4, &mut scene(&mut app, &mut mode));
        if round == 1 {
            app.set_locale(i18n::Locale::Es);
            s.run(SIZE, 2, &mut scene(&mut app, &mut mode));
            app.set_locale(i18n::Locale::En);
        }
        mode = Some(Mode::Dark);
        let back = s.run(SIZE, 4, &mut scene(&mut app, &mut mode));
        save(&format!("views-dark-round{round}"), &back);
        assert_eq!(
            diff(&back, &a0),
            0,
            "round {round}: Dark → Light → Dark is Dark"
        );
    }
}
