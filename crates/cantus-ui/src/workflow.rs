//! Round 31's workflow (ADR 0072): the command palette, the undo history, the start page,
//! project and track templates, and the recent projects — their state on the application,
//! the actions they answer, the start-up wiring and the frame's drawing, in one place so
//! the shared files carry one line each.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::keymap::Action;
use crate::palette_index::Uses;
use crate::recent::Recent;
use crate::templates::{Store, TemplateRef};
use crate::views;
use std::path::{Path, PathBuf};

/// Everything the workflow keeps while Cantus runs.
#[derive(Default)]
pub struct WorkflowUi {
    pub palette: views::palette::Palette,
    pub history: views::history::HistoryView,
    pub start: views::start_page::StartPage,
    pub naming: Option<views::template_name::Naming>,
    /// A template waiting for its plug-ins' states (review 2).
    pub gathering: Option<views::template_name::Gathering>,
    pub recent: Recent,
    pub uses: Uses,
    uses_path: Option<PathBuf>,
    pub store: Store,
    /// Bumped when a template is saved or deleted: the picker's catalogue is made again.
    pub templates_rev: u64,
}

impl WorkflowUi {
    /// Keep the palette's uses (where the start-up wiring said).
    pub fn store_uses(&self) {
        if let Some(p) = &self.uses_path {
            if let Err(e) = self.uses.save(p) {
                log::warn!("command palette: could not save {}: {e}", p.display());
            }
        }
    }

    /// A template was saved or deleted.
    pub fn templates_changed(&mut self) {
        self.templates_rev += 1;
    }

    /// Point the workflow at its files: the templates in `data`, the recent projects and the
    /// palette's uses beside the settings in `config`.
    pub fn wire_at(&mut self, config: &Path, data: &Path) {
        self.store = Store::at(data.join("templates"));
        self.recent = Recent::load(crate::recent::path(config));
        let uses = crate::palette_index::uses_path(config);
        self.uses = Uses::load(&uses);
        self.uses_path = Some(uses);
    }
}

/// The start-up wiring (`CantusApp::assemble`, the path the shipped app takes).
pub fn wire(app: &mut CantusApp) {
    app.workflow
        .wire_at(&crate::app::config_dir(), &crate::app::data_dir());
}

/// The start page at launch (the owner's decision, 2026-10-26): open unless Cantus was
/// given a project to open or *Show at start* is off. Called by `CantusApp::assemble`, the
/// boot's path.
pub fn open_at_start(app: &mut CantusApp, opening_a_file: bool) {
    if app.settings.start_page_at_start && !opening_a_file {
        app.workflow.start.open_at_launch();
    }
}

/// Whether something that comes first at start is showing — the crash-recovery offer, the
/// audio first-run setup — so the start page waits behind it (it opens after; ADR 0072).
pub fn start_page_waits(app: &CantusApp) -> bool {
    app.recovery.is_some() || app.audio_setup.first_run.is_some()
}

/// The actions of round 31; `true` when `action` was one.
pub fn action(app: &mut CantusApp, action: Action) -> bool {
    match action {
        Action::CommandPalette => views::palette::toggle(app),
        Action::ViewHistory => app.toggle_pane(views::workspace::Pane::History),
        Action::NewFromTemplate => app.workflow.start.open(),
        Action::SaveAsTemplate => views::template_name::ask_project(app),
        Action::SaveTrackTemplate => views::template_name::ask_track(app, None),
        _ => return false,
    }
    true
}

/// The device picker's track templates, read again when one was saved or deleted (a
/// compare per frame; the folder is read only then).
pub fn sync_templates(app: &mut CantusApp) {
    let rev = app.workflow.templates_rev;
    if app.picker_view.templates_seen != Some(rev) {
        app.picker_view.track_templates = app.workflow.store.tracks();
        app.picker_view.templates_seen = Some(rev);
    }
}

/// The workflow's panels, over the window. Once a frame.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    sync_templates(app);
    views::template_name::poll(app, false);
    if !start_page_waits(app) {
        views::start_page::frame(app, ctx);
    }
    views::template_name::frame(app, ctx);
    views::palette::frame(app, ctx);
}

impl CantusApp {
    /// *New from template*: a new, untitled project from `template` — at once when nothing
    /// is unsaved, else after the question *New* asks.
    pub fn request_template(&mut self, template: TemplateRef) {
        self.wait_for_save();
        // As *New* and *Open* do: a pass in progress ends into this project first.
        self.end_passes();
        if self.bus.is_dirty() {
            self.unsaved_prompt = Some(views::unsaved::Then::Template(template));
        } else {
            self.new_from_template(&template);
        }
    }

    /// Put a project made from `template` in place of the open one (no question asked).
    pub fn new_from_template(&mut self, template: &TemplateRef) {
        match crate::templates::open_template(template) {
            Ok(p) => {
                self.replace_project(p);
                let name = match template {
                    TemplateRef::Starter(s) => t(s.name_key()),
                    TemplateRef::User(p) => p
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                };
                self.set_status(tf("workflow.status.new_from", &[("name", &name)]));
            }
            Err(e) => self.set_error(tf("workflow.status.open_failed", &[("error", &e)])),
        }
    }

    /// Insert the track template at `path` as track `index`: one undo step, the new track
    /// selected; what it reached that this project lacks is said.
    pub fn insert_track_template(&mut self, path: &Path, index: usize) -> bool {
        let template = match crate::templates::load_track_template(path) {
            Ok(p) => p,
            Err(e) => {
                self.set_error(tf("workflow.status.open_failed", &[("error", &e)]));
                return false;
            }
        };
        let mut next = self.project().next_id;
        let insert =
            match crate::templates::insert_track(self.project(), &template, index, &mut next) {
                Ok(i) => i,
                Err(e) => {
                    self.set_error(e);
                    return false;
                }
            };
        self.bus.reserve_ids(next);
        if !self.exec(insert.command) {
            return false;
        }
        self.selected_track = Some(insert.track);
        let name = template
            .tracks
            .first()
            .map(|t| t.name.clone())
            .unwrap_or_default();
        if insert.missing.is_empty() {
            self.set_status(tf("workflow.status.inserted", &[("name", &name)]));
        } else {
            let missing = insert.missing.join(", ");
            self.set_status(tf(
                "workflow.status.inserted_missing",
                &[("name", &name), ("missing", &missing)],
            ));
        }
        true
    }
}
