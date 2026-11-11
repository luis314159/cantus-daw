//! Round 39 (ADR 0111): the program boots in **Slate** the way a user starts it —
//! `CantusApp::assemble` on a fresh data folder — with Slate Night's tokens, its recessed
//! keys and its face (Barlow Semi Condensed) installed; a setting written before round 39
//! that names a theme of rounds 24–38 opens in that theme, redrawn; and switching themes from the Appearance
//! page's path (`apply_appearance`) carries each family's colours, keys and face, and
//! comes back to Slate exactly.
//!
//! A binary of its own: the installed tokens and the face are process-wide.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::design::role::Role;
use cantus_ui::kit::appearance::{Mode, PRESETS};
use cantus_ui::kit::system::{self, Face, System};
use cantus_ui::kit::tokens;
use cantus_ui::CantusApp;
use std::path::Path;

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
    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.t += 1.0 / 60.0;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 720.0),
                )),
                time: Some(self.t),
                predicted_dt: 1.0 / 60.0,
                ..Default::default()
            };
            let app = &mut self.app;
            self.ctx
                .run_ui(input, |ui| app.frame(ui))
                .textures_delta
                .clear();
        }
    }

    /// The width a role lays `text` out at, in points, in the fonts installed now.
    fn width(&self, text: &str, role: Role) -> f32 {
        let font = role.font(1.0);
        self.ctx.fonts_mut(|f| {
            f.layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE)
                .size()
                .x
        })
    }

    fn switch(&mut self, id: &str) {
        let make = PRESETS.iter().find(|(k, _)| *k == id).unwrap().1;
        let mode = if make().dark { Mode::Dark } else { Mode::Light };
        self.app.appearance.mode = mode;
        self.app.appearance.assign(mode, format!("preset:{id}"));
        self.app.apply_appearance();
        self.frames(2);
    }
}

#[test]
fn a_fresh_install_boots_in_slate_and_every_theme_switches_and_comes_back() {
    let _one = serial();
    let dir = data(None);
    let mut r = boot(dir.path());

    // Slate Night, as designed, installed for every painter.
    let slate = cantus_ui::theme_slate::slate_night_tokens();
    let t = tokens::current();
    assert_eq!(t.system, System::SLATE);
    assert_eq!(
        (t.panel, t.toolbar, t.text),
        (slate.panel, slate.toolbar, slate.text)
    );
    assert_eq!(system::face(), Face::Barlow);
    assert_eq!(r.app.appearance.dark.base, "preset:slate_night");

    // Its face is really there: tabular figures (Barlow as `build_faces.py` made it), and
    // words narrower than Plex Sans Condensed sets them.
    let (ones, zeros) = (r.width("1111", Role::Value), r.width("0000", Role::Value));
    assert!(
        (ones - zeros).abs() < 0.01,
        "figures are tabular: {ones} vs {zeros}"
    );
    let slate_word = r.width("Percussion", Role::Body);

    // Every shipped theme through the Appearance page's path: its own tokens, keys and
    // face, then back to Slate, exactly as it booted.
    for (id, make) in PRESETS {
        r.switch(id);
        let want = make().kit_tokens();
        let got = tokens::current();
        assert_eq!(got.system, want.system, "{id}");
        assert_eq!(
            (got.panel, got.text, got.highlight),
            (want.panel, want.text, want.highlight),
            "{id}"
        );
        assert_eq!(system::face(), want.system.face, "{id}");
        let word = r.width("Percussion", Role::Body);
        match want.system.face {
            Face::Barlow => assert!((word - slate_word).abs() < 0.01, "{id}"),
            _ => assert!(
                (word - slate_word).abs() > 0.5,
                "{id}: {word} vs Slate's {slate_word}"
            ),
        }
    }
    r.switch("slate_night");
    let back = tokens::current();
    assert_eq!(back.system, System::SLATE);
    assert_eq!((back.panel, back.text), (slate.panel, slate.text));
    assert_eq!(system::face(), Face::Barlow);
    assert!((r.width("Percussion", Role::Body) - slate_word).abs() < 0.01);
}

#[test]
fn a_setting_of_beta_13_opens_in_the_theme_it_names() {
    let _one = serial();
    // An appearance file of beta.13: dark on Cantus Night, light on Daylight. Round 39
    // first retired both and showed Slate; they are back (the owner, 2026-11-10), so the
    // file opens in Daylight, redrawn in round 39's system, and the dark mode keeps Cantus
    // Night.
    let old = r#"{"version":2,"mode":"Light","dark":{"base":"preset:cantus_night"},
                  "light":{"base":"preset:daylight"}}"#;
    let dir = data(Some(old));
    let r = boot(dir.path());
    assert_eq!(r.app.appearance.dark.base, "preset:cantus_night");
    assert_eq!(r.app.appearance.light.base, "preset:daylight");
    let t = tokens::current();
    let day = cantus_ui::theme_redrawn::daylight_tokens();
    assert_eq!(
        (t.panel, t.text, t.highlight),
        (day.panel, day.text, day.highlight)
    );
    assert_eq!(t.system, System::GRAPHITE);
    assert_eq!(system::face(), Face::Sofia);
}
