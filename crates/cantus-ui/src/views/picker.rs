//! The device picker (round 20, ADR 0029): every instrument and effect Cantus can use —
//! its own and the hosted CLAP/VST3 plug-ins — on one screen, as a gallery of tiles with a
//! picture of each device. Bitwig's pop-up browser and Komplete Kontrol's tile grid in
//! FL Studio's look.
//!
//! It opens over the window for a [`PickPurpose`] (`CantusApp::open_picker`): the purpose
//! decides whether instruments or effects are offered and what choosing does, which is
//! `CantusApp::apply_pick` (the device-ux builder's). Left: the shelves (all, favourites,
//! recent, the categories, where devices come from) with their counts. Top: what is being
//! chosen, the search, the order, the tile size. Centre: the tiles, grouped under their
//! shelf in the curated order. Right: the chosen device's details — picture, what it is,
//! its vendor and format, a description, its presets. Bottom: the keys and the buttons.
//!
//! **Keyboard first** (Bitwig): typing searches; the arrows move between tiles (up and down
//! by what is on screen, across groups); Enter chooses; Ctrl+D stars; Esc clears the
//! search, then closes. Double-click a tile to choose it.
//!
//! The view's state is [`PickerView`] (`CantusApp::picker_view`); whether it shows is
//! `CantusApp::picker`. What it lays out each frame is published in [`Geometry`], so tests
//! click where a person would.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::picker::{PickChoice, PickPurpose};
use crate::picker_catalog::{self as catalog, Device, Role};
use crate::picker_prefs::Prefs;
use crate::picker_query::{Query, Shelf};
use egui::{Context, Id, Rect};
use std::path::PathBuf;

pub use super::picker_grid::Geometry;

/// The picker's state between frames.
#[derive(Default)]
pub struct PickerView {
    /// Every device, rebuilt when the language or the plug-in database changes.
    pub catalog: Vec<Device>,
    stamp: Option<(&'static str, u64)>,
    pub query: Query,
    /// The device the keyboard and the details pane are on, by key.
    pub selected: Option<String>,
    /// The device being replaced, by key (a `ReplaceInstrument` or `ReplaceEffect`).
    pub current: Option<String>,
    pub prefs: Prefs,
    /// Where the favourites and recents are kept; `None` keeps them for this session only.
    pub prefs_path: Option<PathBuf>,
    pub thumbs: super::picker_thumbs::Thumbs,
    /// The grid's scroll offset, in points from its top.
    pub scroll: f32,
    /// Bring the selected tile into view on the next frame.
    pub(super) reveal: bool,
    /// The search field was clicked into: ← → edit its text instead of moving.
    pub(super) editing: bool,
    /// What was laid out this frame (published for tests).
    pub geo: Geometry,
    /// Choices made through this view (for tests and the status line): the last one.
    pub last_choice: Option<(PickPurpose, PickChoice)>,
    /// The devices panel's own state (View ▸ Devices).
    pub panel: super::picker_panel::Panel,
    /// A plug-in scan is running (the empty CLAP/VST3 shelves say so; round 30).
    pub scanning: bool,
    /// The user's track templates the catalogue offers, and the workflow's count of their
    /// changes they were read at (round 31, ADR 0072).
    pub track_templates: Vec<crate::templates::TrackTemplate>,
    pub templates_seen: Option<u64>,
}

impl PickerView {
    /// The user's preferences from `path`, kept there from now on.
    pub fn load_prefs(&mut self, path: PathBuf) {
        self.prefs = Prefs::load(&path);
        self.prefs_path = Some(path);
    }

    pub(super) fn store_prefs(&self) {
        if let Some(p) = &self.prefs_path {
            if let Err(e) = self.prefs.save(p) {
                log::warn!("device picker: could not save {}: {e}", p.display());
            }
        }
    }

    /// The catalogue, rebuilt if the language, the plug-ins or the installed sound packs'
    /// instruments changed.
    pub fn refresh(
        &mut self,
        db: &cantus_host::PluginDb,
        sounds: &[cantus_content::InstalledInstrument],
    ) {
        let plugins = db.plugins();
        let h = crate::kit::cache::key((
            plugins
                .iter()
                .map(|p| (p.format, p.id.as_str(), p.name.as_str(), p.vendor.as_str()))
                .collect::<Vec<_>>(),
            sounds.iter().map(|s| s.key()).collect::<Vec<_>>(),
            self.track_templates
                .iter()
                .map(|t| t.path.as_path())
                .collect::<Vec<_>>(),
            // A template saved over under its name is the same path: the workflow's count
            // of changes tells it (review 10).
            self.templates_seen,
        ));
        let stamp = (crate::i18n::locale().code(), h);
        if self.stamp != Some(stamp) {
            self.catalog = catalog::build_with(plugins, sounds);
            let first = self.catalog.len();
            self.catalog
                .extend(catalog::track_templates(first, &self.track_templates));
            self.stamp = Some(stamp);
        }
    }

    /// The device with this key.
    pub fn device(&self, key: &str) -> Option<&Device> {
        self.catalog.iter().find(|d| d.key == key)
    }

    /// The selected device, if it is in the catalogue.
    pub fn selection(&self) -> Option<&Device> {
        self.selected.as_deref().and_then(|k| self.device(k))
    }

    /// What the gallery shows now, as catalogue indices in order.
    pub fn visible(&self) -> Vec<usize> {
        crate::picker_query::run(&self.catalog, &self.query, &self.prefs)
    }

    /// Star or unstar a device, and keep it.
    pub fn toggle_favourite(&mut self, key: &str) {
        self.prefs.toggle_favourite(key);
        self.store_prefs();
    }

    /// Choose another shelf.
    pub fn set_shelf(&mut self, shelf: Shelf) {
        self.query.shelf = shelf;
        self.scroll = 0.0;
        self.keep_selection_visible();
    }

    /// Change the words searched for.
    pub fn set_text(&mut self, text: &str) {
        self.query.text = text.to_string();
        self.scroll = 0.0;
        self.keep_selection_visible();
    }

    /// When the selection is filtered out, select the first device shown.
    pub(super) fn keep_selection_visible(&mut self) {
        let vis = self.visible();
        let still = self
            .selected
            .as_deref()
            .is_some_and(|k| vis.iter().any(|&i| self.catalog[i].key == k));
        if !still {
            self.selected = vis.first().map(|&i| self.catalog[i].key.clone());
        }
        self.reveal = true;
    }
}

/// The key of the device `purpose` would replace, if any.
fn current_of(app: &CantusApp, purpose: &PickPurpose) -> Option<String> {
    use cantus_core::{EffectKind, InstrumentSpec};
    let choice = match purpose {
        PickPurpose::ReplaceInstrument { track } => {
            match &app.project().track(*track)?.instrument {
                InstrumentSpec::Firmus(_) => PickChoice::Firmus,
                InstrumentSpec::Pulsus(_) => PickChoice::Pulsus,
                InstrumentSpec::Native(n) => PickChoice::Instrument(n.kind),
                #[allow(unreachable_patterns)]
                _ => return None,
            }
        }
        PickPurpose::ReplaceEffect { strip, effect } => {
            let slot = app.project().effect(*strip, *effect)?;
            if slot.spec.kind == EffectKind::External {
                let ext = slot.spec.external.as_ref()?;
                let prefix = match ext.format {
                    cantus_core::PluginFormat::Clap => "clap",
                    cantus_core::PluginFormat::Vst3 => "vst3",
                };
                return Some(format!("{prefix}:{}", ext.id));
            }
            PickChoice::Effect(slot.spec.kind)
        }
        _ => return None,
    };
    catalog::key_of(&choice)
}

/// Open the picker for `purpose` (what `CantusApp::open_picker` does): instruments or
/// effects, the search empty, the device being replaced selected — or the last one chosen.
pub fn open(app: &mut CantusApp, purpose: PickPurpose) {
    let current = current_of(app, &purpose);
    let sounds = app.samples.content().instruments();
    app.picker_view.refresh(&app.plugins.db, &sounds);
    app.picker_view.scanning = app.plugins.scanning();
    let v = &mut app.picker_view;
    let role = Role::of(&purpose);
    v.query = Query {
        role: Some(role),
        shelf: Shelf::All,
        text: String::new(),
        sort: v.prefs.sort,
    };
    v.current = current.clone();
    v.scroll = 0.0;
    v.editing = false;
    let recent = v
        .prefs
        .recent
        .iter()
        .find(|k| v.catalog.iter().any(|d| &d.key == *k && d.role == role))
        .cloned();
    v.selected = current.or(recent);
    v.keep_selection_visible();
    app.picker = Some(purpose);
}

/// Choose `key` for the open purpose: remember it, and apply it (`apply_pick`, one undo
/// step, which also closes the picker).
pub fn choose(app: &mut CantusApp, key: &str) {
    let Some(purpose) = app.picker.clone() else {
        return;
    };
    let Some(choice) = app.picker_view.device(key).map(|d| d.choice.clone()) else {
        return;
    };
    app.picker_view.prefs.used(key);
    app.picker_view.store_prefs();
    app.picker_view.last_choice = Some((purpose.clone(), choice.clone()));
    app.apply_pick(purpose, choice);
    app.picker = None;
}

/// Close without choosing.
pub fn close(app: &mut CantusApp) {
    app.picker = None;
    app.picker_view.editing = false;
}

/// The window's title for `purpose`, and what it acts on.
pub fn title(app: &CantusApp, purpose: &PickPurpose) -> (String, String) {
    match purpose {
        PickPurpose::AddInstrument { .. } => (
            t("picker.title.add_instrument"),
            t("picker.context.new_track"),
        ),
        PickPurpose::ReplaceInstrument { track } => {
            let name = app
                .project()
                .track(*track)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            (
                t("picker.title.replace_instrument"),
                tf("picker.context.track", &[("name", &name)]),
            )
        }
        PickPurpose::AddEffect { strip } | PickPurpose::ReplaceEffect { strip, .. } => {
            let title = if matches!(purpose, PickPurpose::AddEffect { .. }) {
                t("picker.title.add_effect")
            } else {
                t("picker.title.replace_effect")
            };
            let context = if *strip == cantus_core::StripId::MASTER {
                t("picker.context.master")
            } else {
                let name = app
                    .project()
                    .strip(*strip)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                tf("picker.context.strip", &[("name", &name)])
            };
            (title, context)
        }
    }
}

/// The word on the choose button.
pub fn verb(purpose: &PickPurpose) -> String {
    match purpose {
        PickPurpose::ReplaceInstrument { .. } | PickPurpose::ReplaceEffect { .. } => {
            t("picker.choose.replace")
        }
        PickPurpose::AddEffect { .. } => t("picker.choose.insert"),
        PickPurpose::AddInstrument { .. } => t("picker.choose.add"),
    }
}

/// The overlay's id.
pub fn id() -> Id {
    Id::new("device-picker")
}

/// Draw the picker when it is open (called once a frame, after the workspace).
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    let Some(purpose) = app.picker.clone() else {
        return;
    };
    let sounds = app.samples.content().instruments();
    app.picker_view.refresh(&app.plugins.db, &sounds);
    app.picker_view.thumbs.pump(ctx);
    let screen = ctx.content_rect();
    let mut action = super::picker_keys::Action::None;
    egui::Area::new(id())
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .constrain(false)
        .show(ctx, |ui| {
            action = super::picker_layout::show(app, ui, screen, &purpose);
        });
    super::picker_keys::act(app, action);
}

/// The panel's rect inside `screen`: large, centred, never smaller than it can be used at.
pub fn panel_rect(screen: Rect) -> Rect {
    let w = (screen.width() - 48.0).clamp(720.0_f32.min(screen.width()), 1360.0);
    let h = (screen.height() - 48.0).clamp(480.0_f32.min(screen.height()), 860.0);
    Rect::from_center_size(screen.center(), egui::vec2(w, h))
}
