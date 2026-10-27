//! *Save as template…* and *Save track as template…* (round 31, ADR 0072): a small panel
//! that asks for the template's name — the project's or the track's, ready to keep or type
//! over — says when a template of that name is there already (saving replaces it), and saves
//! with Enter or *Save*; Esc or *Cancel* closes it.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Style};
use crate::views::interop::form;
use crate::views::workspace_settings as ws;
use cantus_core::TrackId;
use egui::{pos2, vec2, Align2, Context, Id, Key, Rect, Sense};

/// What is being kept.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Keep {
    Project,
    Track(TrackId),
}

/// The panel's state while it asks.
#[derive(Clone, Debug, PartialEq)]
pub struct Naming {
    pub keep: Keep,
    pub name: String,
    fresh: bool,
    /// Whether a template of the name typed is there, looked at once per name (review 5).
    exists_for: Option<(String, bool)>,
}

/// Ask for the name of a template of the project.
pub fn ask_project(app: &mut CantusApp) {
    let name = app.project().name.clone();
    app.workflow.naming = Some(Naming {
        keep: Keep::Project,
        name,
        fresh: true,
        exists_for: None,
    });
}

/// Ask for the name of a template of `track` (the selected one when `None`); nothing to
/// keep without a track, said.
pub fn ask_track(app: &mut CantusApp, track: Option<TrackId>) {
    let Some(track) = track.or(app.selected_track) else {
        app.set_status(t("workflow.status.no_track"));
        return;
    };
    let Some(name) = app.project().track(track).map(|t| t.name.clone()) else {
        return;
    };
    app.workflow.naming = Some(Naming {
        keep: Keep::Track(track),
        name,
        fresh: true,
        exists_for: None,
    });
}

const W: f32 = 420.0;
const TITLE: f32 = 32.0;
const PAD: f32 = 14.0;
const GAP: f32 = 8.0;
const FIELD: f32 = 26.0;
const LINE: f32 = 18.0;

/// Draw the panel while it asks, and save when told. Once a frame.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    let Some(mut st) = app.workflow.naming.take() else {
        return;
    };
    let s = Style::current();
    let k = s.k;
    let (enter, escape) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if escape {
        return;
    }
    let exists = match &st.exists_for {
        Some((n, e)) if *n == st.name => *e,
        _ => {
            let e = match st.keep {
                Keep::Project => app.workflow.store.project_exists(&st.name),
                Keep::Track(_) => app.workflow.store.track_exists(&st.name),
            };
            st.exists_for = Some((st.name.clone(), e));
            e
        }
    };
    let screen = ctx.content_rect();
    let h = (TITLE + GAP + LINE + GAP + FIELD + GAP + LINE + GAP + 26.0 + PAD) * k;
    let size = vec2((W * k).min(screen.width() - 16.0), h);
    let at = pos2(
        screen.center().x - size.x * 0.5,
        screen.top() + screen.height() * 0.22,
    );
    let rect = Rect::from_min_size(at, size);
    let (title_key, lead_key) = match st.keep {
        Keep::Project => ("workflow.name.project_title", "workflow.name.project_lead"),
        Keep::Track(_) => ("workflow.name.track_title", "workflow.name.track_lead"),
    };
    let mut save = false;
    let mut close = false;
    egui::Area::new(Id::new("cantus-template-name"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ws::frame(ui, rect, &s);
            let title = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE * k));
            if ws::title(ui, title, &s, Glyph::Plus, title_key) {
                close = true;
            }
            let inner = Rect::from_min_max(
                pos2(rect.left() + PAD * k, title.bottom() + GAP * k),
                pos2(rect.right() - PAD * k, rect.bottom() - PAD * k),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.spacing_mut().item_spacing = vec2(0.0, GAP * k);
            let (lead, _) =
                child.allocate_exact_size(vec2(child.available_width(), LINE * k), Sense::hover());
            let (field, _) = child
                .allocate_exact_size(vec2(child.available_width(), s.d(FIELD)), Sense::hover());
            let _ = crate::views::link_picker::search_field(
                &mut child,
                &mut st.name,
                Id::new("cantus-template-name-field"),
                "workflow.name.hint",
                &s,
                field,
                st.fresh,
            );
            let (note, _) =
                child.allocate_exact_size(vec2(child.available_width(), LINE * k), Sense::hover());
            with_brush(child.painter(), |b| {
                let l = Text::key(lead_key, Role::Body, s.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = text::paint(b, lead, &l, s.tk());
                if exists {
                    let words = t("workflow.name.replaces");
                    let n = Text::new(&words, Role::Label, s.t.led)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End));
                    let _ = text::paint(b, note, &n, s.tk());
                }
            });
            let empty = st.name.trim().is_empty();
            form::buttons(&mut child, |ui| {
                if form::button(ui, &t("workflow.name.save"), true, !empty) {
                    save = true;
                }
                if form::button(ui, &t("workflow.name.cancel"), false, true) {
                    close = true;
                }
            });
            if enter && !empty {
                save = true;
            }
        });
    st.fresh = false;
    if close {
        return;
    }
    if !save {
        app.workflow.naming = Some(st);
        return;
    }
    begin(app, st.keep, st.name.trim().to_string());
}

/// A template being written: its plug-ins asked for their state as it is now, and waited
/// for frame by frame as a save waits (ADR 0072, review 2: a patch shaped in a plug-in's
/// editor was lost when the template took the project's last copy).
#[derive(Clone, Debug, PartialEq)]
pub struct Gathering {
    pub keep: Keep,
    pub name: String,
    started: std::time::Instant,
}

/// Save a template of `keep` under `name`: the hosted plug-ins are asked for their state;
/// [`poll`] writes it once they have answered.
pub fn begin(app: &mut CantusApp, keep: Keep, name: String) {
    app.plugins.ask_states();
    app.workflow.gathering = Some(Gathering {
        keep,
        name,
        started: std::time::Instant::now(),
    });
    poll(app, false);
}

/// Write the template whose states are in — or, `now`, with each plug-in as it was last
/// said, which the status names. Each frame; `true` once written (or failed).
pub fn poll(app: &mut CantusApp, now: bool) -> bool {
    if app.workflow.gathering.is_none() {
        return false;
    }
    if !now && !app.plugins.states_in() {
        return false;
    }
    let Some(g) = app.workflow.gathering.take() else {
        return false;
    };
    let _ = g.started;
    let project = app.plugins.project_for_save(app.bus.project());
    let stale = crate::views::plugin_notice::saved_stale(app.plugins.saved_stale());
    let name = g.name;
    let result = match g.keep {
        Keep::Project => {
            let base = app.audio_base().map(|p| p.to_path_buf());
            app.workflow
                .store
                .save_project(&project, base.as_deref(), &name)
        }
        Keep::Track(track) => app.workflow.store.save_track(&project, track, &name),
    };
    match result {
        Ok(_) => {
            app.set_status(tf("workflow.status.saved", &[("name", &name)]));
            if let Some(said) = stale {
                app.set_error(said);
            }
            app.workflow.templates_changed();
        }
        Err(e) => app.set_error(tf("workflow.status.save_failed", &[("error", &e)])),
    }
    true
}
