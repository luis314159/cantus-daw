//! *Settings ▸ Models* (ADR 0085): every model Cantus can download — what it does, what it
//! weighs, its licence and the inference runtime's, who made it and from what (its
//! provenance card) — with *Accept the licence and download*, its progress and *Cancel*,
//! and *Remove* (the helper ended first, so nothing holds its files). The same summary and
//! actions sit in *Separate stems…* before a model's first download.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::text::Role;
use crate::kit::Style;
use crate::views::interop::form;
use crate::views::samples_content::size;
use crate::views::workspace_settings as ws;
use cantus_content::models::{Model, ModelStatus};

fn model_of(app: &CantusApp, id: &str) -> Option<Model> {
    app.ml.models().manifest().model(id).cloned()
}

/// The model's name and what it does.
pub fn header(app: &CantusApp, ui: &mut egui::Ui, s: &Style, id: &str) {
    let Some(m) = model_of(app, id) else {
        return;
    };
    let lang = crate::i18n::locale().code();
    let w = ui.available_width();
    ws::wrapped(ui, m.name.get(lang), Role::Control, s.t.text, w);
    ws::wrapped(ui, m.description.get(lang), Role::Body, s.t.text_dim, w);
}

/// The model's sizes, licence and provenance, as label and value rows.
pub fn summary(app: &CantusApp, ui: &mut egui::Ui, s: &Style, id: &str) {
    let Some(m) = model_of(app, id) else {
        return;
    };
    let w = ui.available_width();
    let lang = crate::i18n::locale().code();
    let runtime = app
        .ml
        .models()
        .manifest()
        .runtime(&m.runtime, &cantus_content::models::platform())
        .cloned();
    let rt_size = runtime.as_ref().map_or(0, |r| match &r.source {
        cantus_content::Source::Zip(z) | cantus_content::Source::TarGz(z) => z.size,
        cantus_content::Source::Files(f) => f.iter().map(|x| x.size).sum(),
    });
    let rows = [
        (
            t("stems.detail.download"),
            tf(
                "stems.detail.download_value",
                &[
                    ("model", &size(m.download_size())),
                    ("runtime", &size(rt_size)),
                ],
            ),
        ),
        (
            t("stems.detail.disk"),
            size(m.installed_size + runtime.as_ref().map_or(0, |r| r.installed_size)),
        ),
        (t("stems.detail.sources"), {
            let parts: Vec<String> = m.sources.iter().map(|p| super::run::part_name(p)).collect();
            parts.join(", ")
        }),
        (
            t("stems.detail.licence"),
            tf("stems.detail.licence_value", &[("licence", &m.licence)]),
        ),
        (
            t("stems.detail.runtime"),
            runtime.as_ref().map_or_else(
                || t("stems.detail.no_runtime"),
                |r| {
                    tf(
                        "stems.detail.runtime_value",
                        &[("version", &r.version), ("licence", &r.licence)],
                    )
                },
            ),
        ),
        (t("stems.detail.authors"), m.provenance.authors.clone()),
        (
            t("stems.detail.training"),
            m.provenance.training.get(lang).to_owned(),
        ),
        (
            t("stems.detail.quality"),
            m.provenance.quality.get(lang).to_owned(),
        ),
        (
            t("stems.detail.weights"),
            m.provenance.weights.get(lang).to_owned(),
        ),
        (
            t("stems.detail.conversion"),
            m.provenance.conversion.get(lang).to_owned(),
        ),
        (t("stems.detail.checked"), m.checked.clone()),
    ];
    // Label and value side by side, the value wrapping in its column.
    let lw = (120.0 * s.k).min(w * 0.35);
    for (label, value) in rows {
        ui.horizontal_top(|ui| {
            // A value's lines close together; the rows apart.
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.vertical(|ui| {
                ui.set_width(lw);
                ws::wrapped(ui, &label, Role::Label, s.t.text_dim, lw);
            });
            ui.vertical(|ui| {
                ws::wrapped(
                    ui,
                    &value,
                    Role::Body,
                    s.t.text,
                    (w - lw - 8.0 * s.k).max(40.0),
                );
            });
        });
    }
    if let Ok(link) = url_ok(&m.licence_url) {
        if form::button(ui, &t("stems.detail.read_licence"), false, true) {
            ui.ctx().open_url(egui::OpenUrl::new_tab(link));
        }
    }
}

fn url_ok(u: &str) -> Result<&str, ()> {
    if u.starts_with("https://") {
        Ok(u)
    } else {
        Err(())
    }
}

/// What can be done with the model as it stands; true to close a panel it sits in.
pub fn actions(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    s: &Style,
    id: &str,
    status: &ModelStatus,
) -> bool {
    let w = ui.available_width();
    match status {
        ModelStatus::Unavailable => {
            ws::wrapped(ui, &t("stems.state.unavailable"), Role::Body, s.t.danger, w);
            false
        }
        ModelStatus::Absent { accepted } => {
            if !accepted {
                ws::wrapped(ui, &t("stems.accept_note"), Role::Body, s.t.text_dim, w);
            }
            let label = if *accepted {
                t("stems.download")
            } else {
                t("stems.accept_download")
            };
            form::buttons(ui, |ui| {
                if form::button(ui, &label, true, true) {
                    let ms = app.ml.models_mut();
                    let r = (if *accepted {
                        Ok(())
                    } else {
                        ms.accept(id).map_err(|e| e.to_string())
                    })
                    .and_then(|_| ms.install(id));
                    if let Err(e) = r {
                        app.set_error(e);
                    }
                }
            });
            false
        }
        ModelStatus::Downloading { done, total } => {
            form::progress(
                ui,
                if *total == 0 {
                    0.0
                } else {
                    *done as f32 / *total as f32
                },
            );
            ws::wrapped(
                ui,
                &tf(
                    "stems.state.downloading",
                    &[("done", &size(*done)), ("total", &size(*total))],
                ),
                Role::Micro,
                s.t.text_dim,
                w,
            );
            if form::button(ui, &t("stems.cancel"), false, true) {
                app.ml.models_mut().cancel(id);
            }
            false
        }
        ModelStatus::Retrying { error } => {
            ws::wrapped(
                ui,
                &tf("stems.state.retrying", &[("why", error)]),
                Role::Micro,
                s.t.text_dim,
                w,
            );
            if form::button(ui, &t("stems.cancel"), false, true) {
                app.ml.models_mut().cancel(id);
            }
            false
        }
        ModelStatus::Preparing => {
            ws::wrapped(ui, &t("stems.state.preparing"), Role::Body, s.t.text_dim, w);
            false
        }
        ModelStatus::Failed(e) => {
            ws::wrapped(
                ui,
                &tf("stems.state.failed", &[("why", e)]),
                Role::Body,
                s.t.danger,
                w,
            );
            if form::button(ui, &t("stems.retry"), true, true) {
                if let Err(e) = app.ml.models_mut().install(id) {
                    app.set_error(e);
                }
            }
            false
        }
        ModelStatus::Ready => false,
    }
}

/// Whether the helper is in use — a separation under way, a conversion to MIDI (r34:transcribe),
/// or any job: a model is not removed under it (review 2 #10).
pub fn in_use(app: &CantusApp) -> bool {
    app.ml.busy()
        || app.stems.runs.iter().any(|r| r.running())
        || app.ml.ml_agents() > 0
        || app.transcribe.busy() // r34:transcribe: a conversion reads or listens
}

/// Whether `id` is being removed (its removal waits for the helper to let its files go).
pub fn removing(app: &CantusApp, id: &str) -> bool {
    app.stems.removing.as_ref().is_some_and(|(r, _)| r == id)
}

/// `id`, ready to start: installed, and not being removed — a removal waiting for the
/// helper still reads ready on disk (integration review #3). Every start asks this.
pub fn ready(app: &CantusApp, id: &str) -> Option<cantus_content::models::ReadyModel> {
    if removing(app, id) {
        return None;
    }
    app.ml.models().ready(id)
}

/// Ask to remove a model: refused while the helper is in use; else the helper is ended
/// (its library and the model's files are free then, on Windows too) and the removal is
/// tried a frame at a time by [`tend`] — the window never waits for it (review 2 #10).
pub fn request_remove(app: &mut CantusApp, id: &str) -> Result<(), String> {
    if in_use(app) {
        return Err(t("stems.remove_in_use"));
    }
    app.ml.release_helper();
    app.stems.removing = Some((id.to_string(), std::time::Instant::now()));
    Ok(())
}

/// A removal asked for, tried again: done, still waiting for the helper to let its files
/// go (up to 4 s), or failed.
pub fn tend(app: &mut CantusApp) {
    let Some((id, t0)) = app.stems.removing.clone() else {
        return;
    };
    // Something took the helper meanwhile (another tool's job): not removed under it.
    if in_use(app) {
        app.stems.removing = None;
        app.set_error(t("stems.remove_in_use"));
        return;
    }
    match app.ml.models_mut().remove(&id) {
        Ok(()) => {
            app.stems.removing = None;
            app.set_status(t("stems.status.removed"));
        }
        Err(_) if t0.elapsed() < std::time::Duration::from_secs(4) => {
            if let Some(ctx) = &app.stems.repaint {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        Err(e) => {
            app.stems.removing = None;
            app.set_error(e.to_string());
        }
    }
}

/// The Settings page.
pub fn page(app: &mut CantusApp, ui: &mut egui::Ui) {
    let s = Style::current();
    ws::wrapped(
        ui,
        &t("stems.models.intro"),
        Role::Body,
        s.t.text_dim,
        ui.available_width(),
    );
    let ids: Vec<String> = app
        .ml
        .models()
        .manifest()
        .models
        .iter()
        .map(|m| m.id.clone())
        .collect();
    for id in ids {
        form::band(ui, "stems.models.band");
        header(app, ui, &s, &id);
        let status = app.ml.models().status(&id);
        let w = ui.available_width();
        let state = match &status {
            ModelStatus::Ready => t("stems.state.ready"),
            ModelStatus::Absent { .. } => t("stems.state.absent"),
            _ => String::new(),
        };
        if !state.is_empty() {
            ws::wrapped(ui, &state, Role::Label, s.t.text, w);
        }
        if status == ModelStatus::Ready {
            // Not while the helper uses it, nor twice.
            let free = !in_use(app) && app.stems.removing.is_none();
            if in_use(app) {
                ws::wrapped(ui, &t("stems.remove_in_use"), Role::Body, s.t.text_dim, w);
            }
            if app.stems.confirm_remove.as_deref() == Some(id.as_str()) {
                if form::button(ui, &t("stems.remove_confirm"), true, free) {
                    app.stems.confirm_remove = None;
                    if let Err(e) = request_remove(app, &id) {
                        app.set_error(e);
                    }
                }
            } else if form::button(ui, &t("stems.remove"), false, free) {
                app.stems.confirm_remove = Some(id.clone());
            }
        } else {
            actions(app, ui, &s, &id, &status);
        }
        summary(app, ui, &s, &id);
    }
}
