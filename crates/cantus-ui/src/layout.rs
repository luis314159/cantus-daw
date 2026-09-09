//! Named window layouts: how big each panel is, and which of them are showing.
//!
//! A DAW is used in phases — arranging, editing notes, mixing — and each wants a different
//! shape of window. Rather than making the user drag the same splitters back and forth,
//! Cantus lets them name a shape and return to it.
//!
//! While the app runs, egui remembers a resized panel's size in its own memory, keyed by
//! the panel's [`Id`]. That is the right default: a splitter the user drags should stay
//! where they put it. This module leaves it alone except at two moments:
//!
//! - **capture**: read the sizes back out of egui ([`PanelState::load`]) to save a layout.
//! - **apply**: impose a layout's sizes for exactly one frame.
//!
//! Applying is the awkward half. egui 0.36 exposes no setter for [`PanelState`], and
//! `Panel::default_size` only applies when the panel has no stored size. `Panel::exact_size`
//! does work unconditionally — it pins the size range to a single point — so
//! [`Layouts::forced`] hands the app an exact size for one frame, egui records it, and from
//! the next frame the panel is resizable again from there.
//!
//! Note for anyone extending the tests below: a bare `egui::Context` does not keep panel
//! sizes between frames the way eframe's does, so the tests assert what a forced frame
//! produces and leave "a dragged size stays put" to be checked by running the app.
//!
//! eframe is built here without its `persistence` feature, so egui does not carry panel
//! sizes across restarts. The layouts file is what makes them survive: the app captures the
//! current layout on exit and forces it on startup.

use crate::design::space;
use egui::containers::panel::PanelState;
use egui::{Context, Id};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The resizable panels a layout describes. The `Id` is the one `app` gives the panel, and
/// also the key egui files its remembered size under.
pub const INSPECTOR: &str = "inspector";
pub const ARRANGEMENT: &str = "arrangement";

pub fn panel_id(name: &str) -> Id {
    Id::new(name)
}

/// Which view fills the centre of the window.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Central {
    PianoRoll,
    Mixer,
}

/// One named arrangement of the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    /// Width of the inspector panel, in points.
    pub inspector: f32,
    /// Height of the arrangement panel, in points.
    pub arrangement: f32,
    pub show_inspector: bool,
    pub show_arrangement: bool,
    pub central: Central,
}

impl Layout {
    /// The layout a fresh install opens with, and what *Reset layout* returns to.
    pub fn default_layout() -> Layout {
        Layout {
            // The key i18n string is resolved at display time, so the stored name stays
            // stable when the user switches language.
            name: DEFAULT_NAME.to_string(),
            inspector: space::GRID * 76.0,
            arrangement: space::GRID * 65.0,
            show_inspector: true,
            show_arrangement: true,
            central: Central::PianoRoll,
        }
    }

    /// Read the live panel sizes out of egui, keeping everything else from `self`.
    ///
    /// A panel that is hidden has no state to read, so its stored size is kept rather than
    /// collapsing to zero — otherwise hiding a panel and saving would lose its width.
    pub fn capture(&self, ctx: &Context) -> Layout {
        let size = |name: &str, shown: bool, current: f32| -> f32 {
            if !shown {
                return current;
            }
            PanelState::load(ctx, panel_id(name))
                .map(|s| s.size())
                .map(|v| if name == INSPECTOR { v.x } else { v.y })
                .filter(|v| *v > 0.0)
                .unwrap_or(current)
        };
        Layout {
            inspector: size(INSPECTOR, self.show_inspector, self.inspector),
            arrangement: size(ARRANGEMENT, self.show_arrangement, self.arrangement),
            ..self.clone()
        }
    }

    /// This layout's size for a panel, or `None` for a panel it does not describe.
    pub fn size_of(&self, panel: &str) -> Option<f32> {
        match panel {
            INSPECTOR => Some(self.inspector),
            ARRANGEMENT => Some(self.arrangement),
            _ => None,
        }
    }

    pub fn shows(&self, panel: &str) -> bool {
        match panel {
            INSPECTOR => self.show_inspector,
            ARRANGEMENT => self.show_arrangement,
            _ => true,
        }
    }

    /// The label to show: built-in layouts are translated, user-named ones are not.
    pub fn display_name(&self) -> String {
        if self.name == DEFAULT_NAME {
            crate::i18n::t("layout.default")
        } else {
            self.name.clone()
        }
    }
}

/// The stored name of the built-in layout. Never shown raw — see [`Layout::display_name`].
pub const DEFAULT_NAME: &str = "@default";

/// Every layout the user has, and which one is in use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layouts {
    pub current: Layout,
    #[serde(default)]
    pub saved: Vec<Layout>,
    /// Frames left in which the panel sizes are imposed rather than read. One is enough;
    /// the field exists so `apply` can be called from a menu and take effect on the next
    /// frame, whenever that is.
    #[serde(skip)]
    forcing: u8,
}

impl Default for Layouts {
    fn default() -> Self {
        Layouts {
            current: Layout::default_layout(),
            saved: Vec::new(),
            forcing: 0,
        }
    }
}

impl Layouts {
    /// Save the current shape under `name`, replacing a layout of the same name.
    pub fn save_as(&mut self, name: &str, ctx: &Context) {
        let mut layout = self.current.capture(ctx);
        layout.name = name.to_string();
        match self.saved.iter_mut().find(|l| l.name == name) {
            Some(existing) => *existing = layout.clone(),
            None => self.saved.push(layout.clone()),
        }
        self.current = layout;
    }

    /// Switch to a saved layout by name. Returns false if there is no such layout.
    pub fn apply_named(&mut self, name: &str) -> bool {
        let Some(layout) = self.saved.iter().find(|l| l.name == name).cloned() else {
            return false;
        };
        self.current = layout;
        self.force();
        true
    }

    /// Impose the current layout's sizes on the next frame.
    pub fn force(&mut self) {
        self.forcing = 1;
    }

    /// The size the app must pin `panel` to this frame, if a layout is being applied.
    /// `None` means "leave it to egui and the user".
    pub fn forced(&self, panel: &str) -> Option<f32> {
        (self.forcing > 0)
            .then(|| self.current.size_of(panel))
            .flatten()
    }

    /// Call once per frame, after the panels are laid out.
    pub fn end_frame(&mut self) {
        self.forcing = self.forcing.saturating_sub(1);
    }

    pub fn delete(&mut self, name: &str) {
        self.saved.retain(|l| l.name != name);
        if self.current.name == name {
            self.current.name = DEFAULT_NAME.to_string();
        }
    }

    /// Back to the built-in shape, keeping the saved layouts.
    pub fn reset(&mut self) {
        self.current = Layout::default_layout();
        self.force();
    }

    /// Fold the live panel sizes into `current`, so what gets written to disk is what the
    /// user is actually looking at.
    pub fn sync(&mut self, ctx: &Context) {
        self.current = self.current.capture(ctx);
    }

    pub fn load(path: &Path) -> Layouts {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

/// Where the layouts file lives, next to the theme.
pub fn layouts_path(dir: &Path) -> PathBuf {
    dir.join("layouts.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        let ctx = Context::default();
        crate::design::install_style(&ctx);
        ctx
    }

    fn input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 800.0),
            )),
            ..Default::default()
        }
    }

    /// One frame laid out the way `app` lays the real window out: each panel pinned when
    /// `layouts` says so, resizable otherwise.
    fn frame(ctx: &Context, layouts: &mut Layouts) {
        let forced_inspector = layouts.forced(INSPECTOR);
        let forced_arrangement = layouts.forced(ARRANGEMENT);
        let (show_i, show_a) = (
            layouts.current.show_inspector,
            layouts.current.show_arrangement,
        );
        ctx.run_ui(input(), |ui| {
            if show_i {
                let p = egui::Panel::right(panel_id(INSPECTOR));
                match forced_inspector {
                    Some(w) => p.exact_size(w),
                    None => p.resizable(true).min_size(240.0),
                }
                .show(ui, |ui| {
                    ui.label("i");
                });
            }
            if show_a {
                let p = egui::Panel::top(panel_id(ARRANGEMENT));
                match forced_arrangement {
                    Some(h) => p.exact_size(h),
                    None => p.resizable(true).min_size(120.0),
                }
                .show(ui, |ui| {
                    ui.label("a");
                });
            }
            egui::CentralPanel::default().show(ui, |ui| {
                ui.label("c");
            });
        })
        .textures_delta
        .clear();
        layouts.end_frame();
    }

    /// The size egui actually laid the panel out at, straight from its own state — not
    /// through [`Layout::capture`], which falls back to the layout's own value.
    fn actual(ctx: &Context, panel: &str) -> Option<f32> {
        PanelState::load(ctx, panel_id(panel)).map(|s| {
            if panel == INSPECTOR {
                s.size().x
            } else {
                s.size().y
            }
        })
    }

    #[test]
    fn a_forced_frame_lays_the_panels_out_at_the_layouts_sizes() {
        let ctx = ctx();
        let mut layouts = Layouts::default();
        layouts.current.inspector = 320.0;
        layouts.current.arrangement = 260.0;
        layouts.force();
        frame(&ctx, &mut layouts);

        assert_eq!(actual(&ctx, INSPECTOR), Some(320.0));
        assert_eq!(actual(&ctx, ARRANGEMENT), Some(260.0));
        // And the force is spent, so the user can drag again.
        assert_eq!(layouts.forced(INSPECTOR), None);
    }

    #[test]
    fn capture_reports_what_egui_laid_out_not_what_was_asked_for() {
        let ctx = ctx();
        let mut layouts = Layouts::default();
        layouts.current.inspector = 288.0;
        layouts.current.arrangement = 200.0;
        layouts.force();
        frame(&ctx, &mut layouts);

        let captured = layouts.current.capture(&ctx);
        assert_eq!(Some(captured.inspector), actual(&ctx, INSPECTOR));
        assert_eq!(Some(captured.arrangement), actual(&ctx, ARRANGEMENT));
    }

    #[test]
    fn hidden_panels_keep_their_stored_size() {
        let ctx = ctx();
        let mut layouts = Layouts::default();
        layouts.current.inspector = 320.0;
        layouts.force();
        frame(&ctx, &mut layouts);

        // Hide the inspector: it is not on screen, so its width must come from the layout
        // rather than from whatever egui last laid out.
        layouts.current.show_inspector = false;
        layouts.current.inspector = 288.0;
        frame(&ctx, &mut layouts);
        assert_eq!(layouts.current.capture(&ctx).inspector, 288.0);
    }

    #[test]
    fn layouts_are_named_saved_switched_and_reset() {
        let ctx = ctx();
        let mut layouts = Layouts::default();
        layouts.current.inspector = 320.0;
        layouts.force();
        frame(&ctx, &mut layouts);

        layouts.save_as("Mixing", &ctx);
        assert_eq!(layouts.saved.len(), 1);
        assert_eq!(layouts.current.name, "Mixing");
        assert_eq!(layouts.saved[0].inspector, 320.0);

        // Saving the same name again replaces rather than duplicates.
        layouts.save_as("Mixing", &ctx);
        assert_eq!(layouts.saved.len(), 1);

        assert!(!layouts.apply_named("Nope"));
        assert!(layouts.apply_named("Mixing"));
        assert_eq!(
            layouts.forced(INSPECTOR),
            Some(320.0),
            "switching layouts must impose the switched-to sizes"
        );

        layouts.reset();
        assert_eq!(layouts.current.name, DEFAULT_NAME);
        assert_eq!(
            layouts.current.inspector,
            Layout::default_layout().inspector
        );
        assert_eq!(layouts.saved.len(), 1, "reset keeps the saved layouts");
        assert!(
            layouts.forced(INSPECTOR).is_some(),
            "reset must impose the default sizes"
        );

        layouts.delete("Mixing");
        assert!(layouts.saved.is_empty());
    }

    #[test]
    fn deleting_the_layout_in_use_falls_back_to_the_default_name() {
        let ctx = ctx();
        let mut layouts = Layouts::default();
        frame(&ctx, &mut layouts);
        layouts.save_as("Mixing", &ctx);
        layouts.delete("Mixing");
        assert_eq!(layouts.current.name, DEFAULT_NAME);
    }

    #[test]
    fn layouts_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = layouts_path(dir.path());
        // A missing file is not an error: it is a fresh install.
        assert_eq!(Layouts::load(&path), Layouts::default());

        let mut layouts = Layouts::default();
        layouts.current.central = Central::Mixer;
        layouts.current.show_arrangement = false;
        layouts.saved.push(Layout {
            name: "Tracking".into(),
            ..Layout::default_layout()
        });
        layouts.save(&path).unwrap();
        assert_eq!(Layouts::load(&path), layouts);

        // A corrupt file must not stop the app from opening.
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Layouts::load(&path), Layouts::default());
    }

    #[test]
    fn the_built_in_layout_shows_a_translated_name() {
        crate::i18n::set_locale(crate::i18n::Locale::En);
        let d = Layout::default_layout();
        assert_ne!(d.display_name(), DEFAULT_NAME);
        let custom = Layout {
            name: "Mixing".into(),
            ..d
        };
        assert_eq!(custom.display_name(), "Mixing");
    }
}
