//! # cantus-ui
//!
//! The egui front end (ADR 0003). Holds a `CommandBus`, talks to the engine through an
//! `EngineHandle`, and never touches audio state directly.

#![forbid(unsafe_code)]

pub mod app;
pub mod audio_clips;
pub mod audio_files;
pub mod audio_setup;
pub mod autosave;
pub mod automation;
pub mod boot;
pub mod clips;
pub mod comping;
pub mod demos;
pub mod design;
pub mod hosted_inst;
pub mod i18n;
pub mod icons;
pub mod keyboard;
pub mod keymap;
pub mod kit;
pub mod layout;
pub mod perf_status;
pub mod picker;
pub mod picker_catalog;
pub mod picker_prefs;
pub mod picker_query;
pub mod raster;
pub mod record_pass;
pub mod record_session;
pub mod recording;
pub mod recovery;
pub mod recovery_restore;
pub mod recovery_session;
pub mod recovery_text;
pub mod samples;
pub mod settings;
pub mod startup_probe;
pub mod swap;
pub mod takes;
pub mod theme;
pub mod verse;
pub mod views;
pub mod waveform;
pub mod widgets;

pub use app::{starter_project, stress_project, welcome_project, CantusApp, APP_VERSION};

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
    /// Start in the channel rack, for the same reason as `--mixer`.
    pub rack: bool,
    /// Select this track (by index) at startup, so a view of a particular track can be
    /// looked at without driving the arrangement — synthetic input is unreliable here.
    pub track: Option<usize>,
    /// Open the shortcuts editor at startup (handy for a look without hunting the menu).
    pub shortcuts: bool,
    /// Open the settings dialog at startup, for the same reason.
    pub settings: bool,
    /// Put an effect with an editor (`eq` or `comp`) on the selected track's strip and open
    /// its editor at startup — a look at an editor without driving the inspector.
    pub editor: Option<String>,
    /// Start the transport at startup, so meters and analysers have something to show.
    pub play: bool,
    /// The splash's logo and the window's icon, baked into the executable at build time
    /// (`crates/cantus-app/build.rs`). Without them the splash renders the logo itself.
    pub art: Option<boot::Art>,
}

impl LaunchOptions {
    /// What the flags ask of the application once it is open.
    pub fn apply_flags(&self, app: &mut CantusApp) {
        if self.mixer {
            app.show_central(app::CentralView::Mixer);
        }
        if self.rack {
            app.show_central(app::CentralView::ChannelRack);
        }
        if let Some(id) = self
            .track
            .and_then(|i| app.project().tracks.get(i).map(|t| t.id))
        {
            app.select_track(id);
        }
        app.show_shortcuts = self.shortcuts;
        app.show_settings = self.settings;
        if let Some(kind) = self.editor.as_deref() {
            app.demo_editor(kind);
        }
        if self.play {
            app.toggle_play();
        }
    }
}

/// Run the desktop application until the window closes.
///
/// The window opens as the splash (ADR 0024 §12) — 520 × 320, no decorations, centred —
/// and becomes the main window once everything is in (`boot`).
pub fn run(options: LaunchOptions) -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    // An explicit `--locale` wins over the stored preference; the title is drawn in it.
    if let Some(l) = options.locale.as_deref().and_then(i18n::Locale::from_code) {
        i18n::set_locale(l);
    }
    // The verse at start (§6c): one small file read now, to know whether the splash is
    // tall enough for a verse, and in which language.
    let verse = verse::Launch::read(
        &app::config_dir(),
        options.locale.as_deref().and_then(i18n::Locale::from_code),
        verse::seed_now(),
    );
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(i18n::t("app.title"))
        .with_app_id("cantus")
        .with_inner_size(boot::size(verse.shown.is_some()))
        .with_decorations(false)
        .with_resizable(false);
    if let Some(icon) = boot::icon(options.art.as_ref()) {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let native = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Cantus",
        native,
        Box::new(move |cc| {
            let shell = boot::Shell::Booting(boot::Boot::new(cc, options).with_verse(verse));
            // The start-up benchmark's probe (round 20), only when asked for.
            Ok(match std::env::var_os(startup_probe::VAR) {
                Some(path) => Box::new(startup_probe::Probe::new(shell, started, path.into())),
                None => Box::new(shell) as Box<dyn eframe::App>,
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
