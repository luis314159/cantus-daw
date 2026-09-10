//! # cantus-ui
//!
//! The egui front end (ADR 0003). Holds a `CommandBus`, talks to the engine through an
//! `EngineHandle`, and never touches audio state directly.

#![forbid(unsafe_code)]

pub mod app;
pub mod clips;
pub mod design;
pub mod i18n;
pub mod icons;
pub mod keyboard;
pub mod keymap;
pub mod layout;
pub mod recording;
pub mod settings;
pub mod theme;
pub mod views;
pub mod widgets;

pub use app::{starter_project, stress_project, CantusApp, APP_VERSION};

use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct LaunchOptions {
    /// Project file to open at startup.
    pub project: Option<PathBuf>,
    /// Load the 20 000-note stress project (ADR 0003 guardrail).
    pub stress: bool,
    /// Force a locale (`en`, `es`).
    pub locale: Option<String>,
    /// Open at this window size instead of maximized (also handy for screenshots).
    pub window: Option<(f32, f32)>,
    /// Start in the mixer instead of the piano roll.
    pub mixer: bool,
    /// Open the shortcuts editor at startup (handy for a look without hunting the menu).
    pub shortcuts: bool,
    /// Open the settings dialog at startup, for the same reason.
    pub settings: bool,
}

/// Run the desktop application until the window closes.
pub fn run(options: LaunchOptions) -> anyhow::Result<()> {
    // An explicit `--locale` wins over the stored preference; the app applies whichever
    // it is given.
    let locale_from_flag = options.locale.as_deref().and_then(i18n::Locale::from_code);
    if let Some(l) = locale_from_flag {
        i18n::set_locale(l);
    }
    let project = match &options.project {
        Some(path) => match cantus_project::load(path, true) {
            Ok(l) => Some(l.project),
            Err(e) => {
                log::error!("could not open {}: {e}", path.display());
                None
            }
        },
        None => None,
    };
    let stress = options.stress;
    let mixer = options.mixer;
    let shortcuts = options.shortcuts;
    let settings = options.settings;
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(i18n::t("app.title"))
        .with_app_id("cantus")
        .with_min_inner_size([960.0, 600.0]);
    viewport = match options.window {
        Some((w, h)) => viewport.with_inner_size([w, h]),
        None => viewport
            .with_inner_size([1280.0, 800.0])
            .with_maximized(true),
    };
    let native = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Cantus",
        native,
        Box::new(move |cc| {
            let mut app = CantusApp::new(cc, project, stress, locale_from_flag);
            if mixer {
                app.central = app::CentralView::Mixer;
            }
            app.show_shortcuts = shortcuts;
            app.show_settings = settings;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
