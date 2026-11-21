//! **The look follows the system on a fresh install** (ADR 0111, amended 2026-11-20 — the
//! AppImage catalogue's review asked for a light default with dark only on a dark system,
//! and the owner accepted it): `CantusApp::assemble` on an empty data folder shows Slate
//! Lapis Paper on a light system and Slate Lapis Night on a dark one, and follows the
//! system when it changes while the program runs. The user's own mode, picked on Settings
//! ▸ Appearance, survives a restart on the other system; a file of beta.15 that only ever
//! showed the old default (Dark, no theme chosen) follows the system from now on, and one
//! whose user chose Light stays Light.
//!
//! A binary of its own: the installed tokens and the face are process-wide.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::appearance::{Appearance, Mode};
use cantus_ui::kit::tokens;
use cantus_ui::theme_slate_lapis as lapis;
use cantus_ui::views::workspace_settings::{open, SettingsPage};
use cantus_ui::CantusApp;
use egui::{Event, PointerButton, Pos2, Shape};
use std::path::Path;

const SCREEN: egui::Vec2 = egui::vec2(1600.0, 1200.0);

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
    /// What the operating system says: `None` is a system that says nothing.
    system: Option<egui::Theme>,
}

fn boot(dir: &Path, system: Option<egui::Theme>) -> Rig {
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
    let mut r = Rig {
        app,
        ctx,
        t: 0.0,
        system,
    };
    r.frames(3);
    r
}

impl Rig {
    /// One frame with these events; every text it painted where it can be seen (inside its
    /// clip: a scrolled page paints rows it hides).
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, egui::Rect)> {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            system_theme: self.system,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        let mut texts = Vec::new();
        for c in &out.shapes {
            let mut here = Vec::new();
            texts_of(&c.shape, &mut here);
            texts.extend(
                here.into_iter()
                    .filter(|(_, r)| c.clip_rect.contains(r.center())),
            );
        }
        texts
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

fn texts_of(shape: &Shape, out: &mut Vec<(String, egui::Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts_of(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
        _ => {}
    }
}

/// The installed look is Slate Lapis in this light: its room and its accent.
fn shows(night: bool) -> bool {
    let want = if night {
        lapis::slate_lapis_night_tokens()
    } else {
        lapis::slate_lapis_paper_tokens()
    };
    let got = tokens::current();
    (got.panel, got.highlight, got.text) == (want.panel, want.highlight, want.text)
}

#[test]
fn a_fresh_install_follows_the_system_and_a_mode_the_user_picks_stays() {
    // The two lights really differ, so `shows` tells them apart.
    assert_ne!(
        lapis::slate_lapis_night_tokens().panel,
        lapis::slate_lapis_paper_tokens().panel
    );

    // A fresh install on a light system: Slate Lapis Paper, the mode following the system.
    let dir = data();
    let mut r = boot(dir.path(), Some(egui::Theme::Light));
    assert_eq!(r.app.appearance.mode, Mode::System);
    assert!(shows(false), "a light system opens in Slate Lapis Paper");
    assert!(!cantus_ui::theme::p().dark);

    // The system goes dark while the program runs: Slate Lapis Night, within a frame or two.
    r.system = Some(egui::Theme::Dark);
    r.frames(2);
    assert!(shows(true), "the look follows the system to the dark");
    r.system = Some(egui::Theme::Light);
    r.frames(2);
    assert!(shows(false), "and back to the light");
    assert!(
        !dir.path().join("appearance.json").exists(),
        "following the system is no choice: nothing is written"
    );
    drop(r);

    // A fresh install on a dark system: Slate Lapis Night.
    let dir = data();
    let r = boot(dir.path(), Some(egui::Theme::Dark));
    assert!(shows(true), "a dark system opens in Slate Lapis Night");
    drop(r);

    // A system that says nothing counts as light.
    let dir = data();
    let r = boot(dir.path(), None);
    assert!(shows(false), "an unknown system opens in the light");
    drop(r);

    // On a light system the user picks Night on Settings ▸ Appearance, as a user does: the
    // choice is written, and a restart on the light system opens in the night.
    let dir = data();
    let mut r = boot(dir.path(), Some(egui::Theme::Light));
    r.app.workflow.start.close();
    r.frames(2);
    open(&mut r.app, SettingsPage::Appearance);
    r.frames(40);
    // Below the themes' pictures: scrolled into view, as a user scrolls.
    let mut painted = r.frame(Vec::new());
    for _ in 0..40 {
        if painted.iter().any(|(t, _)| t == "Follow the system") {
            break;
        }
        // Over Settings' page, where it scrolls.
        let at = r
            .ctx
            .memory(|m| m.area_rect(egui::Id::new("cantus-settings")))
            .expect("Settings is open")
            .center();
        r.frame(vec![Event::PointerMoved(at)]);
        r.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -3.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        }]);
        r.frames(12);
        painted = r.frame(Vec::new());
    }
    // The mode's row: the Night beside its *Follow the system* (the families' pictures name
    // their Night too).
    let system = painted
        .iter()
        .find(|(t, _)| t == "Follow the system")
        .map(|(_, rect)| rect.center())
        .unwrap_or_else(|| {
            let seen: Vec<&str> = painted.iter().map(|(t, _)| t.as_str()).collect();
            panic!("Settings ▸ Appearance shows the mode's row: {seen:?}")
        });
    let night = painted
        .iter()
        .filter(|(t, rect)| t == "Night" && (rect.center().y - system.y).abs() < 4.0)
        .map(|(_, rect)| rect.center())
        .next()
        .expect("the mode's row has its Night");
    r.click(night);
    assert_eq!(r.app.appearance.mode, Mode::Dark);
    assert!(shows(true), "Night picked: the night, on a light system");
    assert!(dir.path().join("appearance.json").is_file());
    drop(r);
    let r = boot(dir.path(), Some(egui::Theme::Light));
    assert_eq!(r.app.appearance.mode, Mode::Dark);
    assert!(
        shows(true),
        "the user's Night survives a restart on a light system"
    );
    drop(r);

    // A beta.15 file (version 3) that only ever showed the old default — Dark, neither mode's
    // theme chosen — is no choice: it follows the system now, and is read, not rewritten.
    let beta15 = |mode: &str| {
        let mut v = serde_json::to_value(Appearance::default()).unwrap();
        v["version"] = 3.into();
        v["mode"] = mode.into();
        v.to_string()
    };
    let dir = data();
    let file = dir.path().join("appearance.json");
    std::fs::write(&file, beta15("Dark")).unwrap();
    let r = boot(dir.path(), Some(egui::Theme::Light));
    assert_eq!(r.app.appearance.mode, Mode::System);
    assert!(
        shows(false),
        "beta.15's untouched Dark follows a light system"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), beta15("Dark"));
    drop(r);

    // A beta.15 user who chose Light keeps it on a dark system.
    let dir = data();
    std::fs::write(dir.path().join("appearance.json"), beta15("Light")).unwrap();
    let r = boot(dir.path(), Some(egui::Theme::Dark));
    assert_eq!(r.app.appearance.mode, Mode::Light);
    assert!(shows(false), "a chosen Light stays Light");
}
