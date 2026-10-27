//! **The command palette** (round 31, ADR 0072): VS Code's Ctrl+Shift+P, Bitwig's
//! *Commander*, Ableton 12's search — one field that finds everything Cantus can do.
//!
//! **Ctrl+Shift+P** (⌘⇧P on a Mac; *View ▸ Command palette…*, the top bar's menu) opens a
//! panel near the top of the window: the field, the rows under it, the keys' line. Typing
//! narrows the rows as one types (`palette_index`: fuzzy, in the interface's language and in
//! English, ranked by use); ↑ ↓ move, Enter runs, Esc or a click outside closes, a click on
//! a row runs it. The rows are every [`Action`] with its shortcut, the areas and windows,
//! every Settings page, every effect (inserted on the selected strip) and instrument (on a
//! new track after the selected one) of the device catalogue, the recent projects, the
//! project templates and the track templates.
//!
//! Cost: the entries are made once when it opens; a keystroke ranks them again (a few
//! hundred, folded once each), and only the rows in view are laid out and painted.

use crate::app::CantusApp;
use crate::i18n::{t, t_in, tf, Locale};
use crate::keymap::Action;
use crate::palette_index::{self as index, Entry, Kind, Target, Window};
use crate::picker::PickPurpose;
use crate::views::workspace::Pane;
use crate::views::workspace_settings::SettingsPage;
use egui::{Context, Rect};

/// The panel's width at 100 %.
pub const WIDTH: f32 = 560.0;
/// One row.
pub const ROW: f32 = 30.0;
/// Rows shown before the list scrolls.
pub const VISIBLE: usize = 12;

/// The palette's state while it is open.
#[derive(Default)]
pub struct Palette {
    open: bool,
    /// The words typed.
    pub query: String,
    entries: Vec<Entry>,
    /// The entries the words leave, best first (indices into `entries`).
    shown: Vec<usize>,
    shown_for: Option<String>,
    /// The row Enter runs: an index into `shown`.
    pub(super) highlight: usize,
    pub(super) offset: f32,
    pub(super) scroll_to: Option<f32>,
    pub(super) overflow: bool,
    /// The rows changed: bring the highlight into view.
    pub(super) reveal: bool,
    /// Where each row in view was drawn last frame (its index in `shown`).
    pub(super) rects: Vec<(usize, Rect)>,
    pub(super) panel: Option<Rect>,
    /// The frame it opened in: the key that opened it is no click outside.
    pub(super) fresh: bool,
}

impl Palette {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open over `entries`, nothing typed.
    pub fn open_with(&mut self, entries: Vec<Entry>) {
        *self = Palette {
            open: true,
            entries,
            fresh: true,
            ..Default::default()
        };
    }

    pub fn close(&mut self) {
        *self = Palette::default();
    }

    /// The rows shown now, best first.
    pub fn rows(&self) -> Vec<&Entry> {
        self.shown.iter().map(|&i| &self.entries[i]).collect()
    }

    /// Every entry the palette was opened with.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The row Enter would run.
    pub fn highlighted(&self) -> Option<&Entry> {
        self.shown.get(self.highlight).map(|&i| &self.entries[i])
    }

    /// Where the panel was drawn last frame.
    pub fn panel_rect(&self) -> Rect {
        self.panel.unwrap_or(Rect::NOTHING)
    }

    /// Where each row in view was drawn last frame, by its entry.
    pub fn row_rects(&self) -> Vec<(&Entry, Rect)> {
        self.rects
            .iter()
            .filter_map(|(i, r)| self.shown.get(*i).map(|&e| (&self.entries[e], *r)))
            .collect()
    }

    pub(super) fn shown_len(&self) -> usize {
        self.shown.len()
    }

    pub(super) fn shown_entry(&self, i: usize) -> &Entry {
        &self.entries[self.shown[i]]
    }

    /// Rank again when the words changed; the highlight goes back to the best row.
    pub(super) fn rerank(&mut self, uses: &index::Uses) {
        if self.shown_for.as_deref() == Some(self.query.as_str()) {
            return;
        }
        self.shown = index::rank(&self.entries, &self.query, uses);
        self.shown_for = Some(self.query.clone());
        self.highlight = 0;
        self.reveal = true;
    }

    /// Move the highlight `by` rows, stopping at either end.
    pub(super) fn step(&mut self, by: i32) {
        if self.shown.is_empty() {
            return;
        }
        let last = self.shown.len() as i32 - 1;
        self.highlight = (self.highlight as i32 + by).clamp(0, last) as usize;
    }

    /// The entry at row `i`, taken out (the palette closes as it runs).
    pub(super) fn take(&mut self, i: usize) -> Option<Entry> {
        let e = self.shown.get(i).map(|&e| self.entries[e].clone());
        self.close();
        e
    }
}

/// The text of `key` in the interface's language and in English.
fn both(key: &str) -> (String, String) {
    (t(key), t_in(Locale::En, key))
}

/// The areas the palette shows that no action toggles (the rest are their actions).
pub const PANES: [Pane; 1] = [Pane::Devices];

/// Whether the palette reaches `pane`: an action shows it, or it is a view of its own.
pub fn reaches(pane: Pane) -> bool {
    PANES.contains(&pane)
        || Action::ALL.iter().any(|a| {
            *a != Action::CommandPalette && super::transport::menus::pane_of(*a) == Some(pane)
        })
}

/// Every entry, as the palette offers them now.
pub fn entries(app: &mut CantusApp) -> Vec<Entry> {
    let mut out = Vec::new();
    for a in Action::ALL {
        if a == Action::CommandPalette {
            continue;
        }
        let (name, en) = both(&a.label_key());
        let detail = app.keymap.display_for(a);
        out.push(Entry::new(
            Target::Action(a),
            Kind::Action,
            name,
            en,
            detail,
            Entry::action_key(a),
        ));
    }
    for p in PANES {
        let (name, en) = both(p.toggle_key());
        out.push(Entry::new(
            Target::Pane(p),
            Kind::View,
            name,
            en,
            String::new(),
            format!("pane:{p:?}"),
        ));
    }
    for w in Window::ALL {
        let (name, en) = both(w.key());
        out.push(Entry::new(
            Target::Window(w),
            Kind::Window,
            name,
            en,
            String::new(),
            Entry::window_key(w),
        ));
    }
    for page in SettingsPage::ALL {
        let word = |l: Locale| crate::i18n::t_in(l, page.key());
        let name = tf(
            "workflow.palette.settings",
            &[("page", &word(crate::i18n::locale()))],
        );
        let en = crate::i18n::tf_in(
            Locale::En,
            "workflow.palette.settings",
            &[("page", &word(Locale::En))],
        );
        out.push(Entry::new(
            Target::Settings(page),
            Kind::Settings,
            name,
            en,
            String::new(),
            Entry::settings_key(page),
        ));
    }
    crate::workflow::sync_templates(app);
    let sounds = app.samples.content().instruments();
    app.picker_view.refresh(&app.plugins.db, &sounds);
    for d in &app.picker_view.catalog {
        if matches!(d.choice, crate::picker::PickChoice::TrackTemplate(_)) {
            continue;
        }
        let (kind, verb) = match d.role {
            crate::picker_catalog::Role::Effect => (Kind::Effect, "workflow.palette.insert_effect"),
            crate::picker_catalog::Role::Instrument => {
                (Kind::Instrument, "workflow.palette.add_instrument")
            }
        };
        let name = tf(verb, &[("name", &d.name)]);
        let en = crate::i18n::tf_in(Locale::En, verb, &[("name", &d.name)]);
        // A plug-in installed as CLAP and VST3 is two rows: the format tells them apart.
        let detail = match d.source.badge() {
            Some(b) => format!("{} · {b}", d.what),
            None => d.what.clone(),
        };
        out.push(Entry::new(
            Target::Device(d.key.clone()),
            kind,
            name,
            en,
            detail,
            format!("device:{}", d.key),
        ));
    }
    for path in app.workflow.recent.projects.clone() {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let folder = path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        out.push(Entry::new(
            Target::Recent(path.clone()),
            Kind::Recent,
            name.clone(),
            name,
            folder,
            format!("recent:{}", path.display()),
        ));
    }
    for tpl in app.workflow.store.projects() {
        let name = tf("workflow.palette.new_from", &[("name", &tpl.name)]);
        let en = crate::i18n::tf_in(
            Locale::En,
            "workflow.palette.new_from",
            &[("name", &tpl.name)],
        );
        out.push(Entry::new(
            Target::Template(tpl.template.clone()),
            Kind::Template,
            name,
            en,
            tpl.what.clone(),
            tpl.template.key(),
        ));
    }
    for tpl in app.workflow.store.tracks() {
        let name = tf(
            "workflow.palette.insert_track_template",
            &[("name", &tpl.name)],
        );
        let en = crate::i18n::tf_in(
            Locale::En,
            "workflow.palette.insert_track_template",
            &[("name", &tpl.name)],
        );
        out.push(Entry::new(
            Target::TrackTemplate(tpl.path.clone()),
            Kind::TrackTemplate,
            name,
            en,
            String::new(),
            format!("track-template:{}", tpl.path.display()),
        ));
    }
    // Round 31 (ADR 0069): the note effects, added to the selected track's chain.
    out.extend(crate::notefx_ui::palette_entries(app));
    out
}

/// Open the palette (Ctrl+Shift+P); again closes it.
pub fn toggle(app: &mut CantusApp) {
    if app.workflow.palette.is_open() {
        app.workflow.palette.close();
        return;
    }
    let entries = entries(app);
    app.workflow.palette.open_with(entries);
    let uses = std::mem::take(&mut app.workflow.uses);
    app.workflow.palette.rerank(&uses);
    app.workflow.uses = uses;
}

/// The strip an effect from the palette goes on: the selected strip, else the selected
/// track's, else the master.
fn effect_strip(app: &CantusApp) -> cantus_core::StripId {
    app.selected_strip
        .or_else(|| {
            app.selected_track
                .and_then(|t| app.project().track(t))
                .and_then(|t| t.strip)
        })
        .unwrap_or(cantus_core::StripId::MASTER)
}

/// Where a track from the palette goes: after the selected one, else at the end.
pub fn track_index(app: &CantusApp) -> usize {
    app.selected_track
        .and_then(|t| app.project().track_index(t))
        .map_or(app.project().tracks.len(), |i| i + 1)
}

/// Do what an entry says; its use is counted.
pub fn run(app: &mut CantusApp, entry: &Entry) {
    app.workflow.uses.used(&entry.key);
    app.workflow.store_uses();
    match &entry.target {
        Target::Action(a) => app.run_action(*a),
        Target::Pane(p) => app.show_pane(*p),
        Target::Window(w) => match w {
            Window::Shortcuts => app.show_shortcuts = true,
            Window::Appearance => app.show_theme_editor = true,
            Window::TopBar => crate::views::transport::open_customise(app),
            Window::Start => app.workflow.start.open(),
            Window::About => app.show_about = true,
        },
        Target::Settings(page) => crate::views::workspace_settings::open(app, *page),
        Target::Device(key) => {
            let Some(d) = app.picker_view.device(key).cloned() else {
                return;
            };
            let purpose = match d.role {
                crate::picker_catalog::Role::Effect => PickPurpose::AddEffect {
                    strip: effect_strip(app),
                },
                crate::picker_catalog::Role::Instrument => PickPurpose::AddInstrument {
                    index: Some(track_index(app)),
                },
            };
            app.apply_pick(purpose, d.choice);
        }
        Target::Recent(path) => app.request_open_path(path.clone()),
        Target::Template(tpl) => app.request_template(tpl.clone()),
        Target::NoteFx(track, kind) => {
            app.add_note_fx(*track, *kind);
        }
        Target::TrackTemplate(path) => {
            let path = path.clone();
            app.insert_track_template(&path, track_index(app));
        }
    }
}

#[path = "palette_paint.rs"]
mod paint;

/// Draw the palette when it is open, and run what is chosen. Once a frame.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    if !app.workflow.palette.is_open() {
        return;
    }
    // Its own key closes it, even while its field has the keys (review 11) — not in the
    // frame it opened in, where that key opened it.
    let again = !app.workflow.palette.fresh
        && app
            .keymap
            .pressed(ctx, false, false)
            .contains(&Action::CommandPalette);
    if again {
        app.workflow.palette.close();
        return;
    }
    let mut st = std::mem::take(&mut app.workflow.palette);
    let uses = std::mem::take(&mut app.workflow.uses);
    let out = paint::show(&mut st, &uses, ctx);
    app.workflow.uses = uses;
    st.fresh = false;
    app.workflow.palette = st;
    match out {
        paint::Out::Stay => {}
        paint::Out::Close => app.workflow.palette.close(),
        paint::Out::Run(i) => {
            if let Some(e) = app.workflow.palette.take(i) {
                run(app, &e);
            }
        }
    }
}
