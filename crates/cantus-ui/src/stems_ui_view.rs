//! The *Separate stems…* panel: what it separates, into how many parts (the 4- or 6-stem
//! model), fast or best, the graphics processor if it can; the model's size, licence and
//! maker before its first download, its download and its preparing; the separation's
//! progress with *Cancel*. A floating panel as *Quantise audio…* is: Esc closes it, and a
//! separation goes on with it closed.

use super::run::{self, Phase};
use super::Target;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::Role;
use crate::kit::Style;
use crate::views::interop::form;
use crate::views::workspace_settings as ws;
use cantus_content::models::ModelStatus;
use egui::{pos2, vec2, Context, Id, Key, Rect, UiBuilder};

pub const WIDTH: f32 = 480.0;
const HEIGHT: f32 = 560.0;

/// The models the panel offers, in order: id, and the key of its words.
pub const MODELS: [(&str, &str); 2] = [
    ("htdemucs", "stems.model.four"),
    ("htdemucs-6s", "stems.model.six"),
];

/// The run of `target`'s clips, if one is under way (or just ended).
fn run_of(app: &CantusApp, target: &Target) -> Option<usize> {
    app.stems.runs.iter().rposition(|r| &r.target == target)
}

pub fn dialog(app: &mut CantusApp, ctx: &Context) {
    let Some(target) = app.stems.dialog.clone() else {
        return;
    };
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.stems.dialog = None;
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    // As tall as its rows were last frame (a run's progress is short), up to HEIGHT.
    let fit = Id::new("cantus-stems-dialog-fit");
    let want = ctx.data(|d| d.get_temp::<f32>(fit)).unwrap_or(HEIGHT * k);
    let size = vec2(
        (WIDTH * k).min(screen.width() - 16.0),
        want.min(HEIGHT * k).min(screen.height() - 40.0),
    );
    let at = pos2(
        screen.center().x - size.x * 0.5,
        screen.top() + screen.height() * 0.15,
    );
    let rect = Rect::from_min_size(at, size);
    let mut close = false;
    egui::Area::new(Id::new("cantus-stems-dialog"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ws::frame(ui, rect, &s);
            let title = Rect::from_min_size(rect.min, vec2(size.x, 30.0 * k));
            if ws::title(ui, title, &s, Glyph::Mixer, "stems.title") {
                close = true;
            }
            let inner = Rect::from_min_max(
                pos2(rect.left() + 14.0 * k, title.bottom() + 8.0 * k),
                pos2(rect.right() - 14.0 * k, rect.bottom() - 10.0 * k),
            );
            let mut child = ui.new_child(UiBuilder::new().max_rect(inner));
            let shown = egui::ScrollArea::vertical()
                .id_salt("stems-dialog")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(inner.width() - 8.0 * k);
                    ui.spacing_mut().item_spacing = vec2(6.0 * k, 6.0 * k);
                    if body(app, ui, &s, &target) {
                        close = true;
                    }
                });
            let rows = (shown.content_size.y + (inner.top() - rect.top()) + 10.0 * k)
                .min(HEIGHT * k)
                .min(screen.height() - 40.0);
            if (rows - size.y).abs() > 0.5 {
                ctx.data_mut(|d| d.insert_temp(fit, rows));
                ctx.request_repaint();
            }
        });
    if close {
        app.stems.dialog = None;
    }
}

/// The panel's rows; true to close it.
fn body(app: &mut CantusApp, ui: &mut egui::Ui, s: &Style, target: &Target) -> bool {
    let words = target.words(app.project());
    ws::wrapped(ui, &words, Role::Title, s.t.text, ui.available_width());
    // A separation of these clips under way: its progress and Cancel, nothing else.
    if let Some(i) = run_of(app, target) {
        let r = &app.stems.runs[i];
        match r.phase.clone() {
            Phase::Done => {
                ws::wrapped(
                    ui,
                    &t("stems.done"),
                    Role::Body,
                    s.t.text_dim,
                    ui.available_width(),
                );
                return form::buttons(ui, |ui| form::button(ui, &t("stems.close"), true, true));
            }
            Phase::Failed(why) => {
                ws::wrapped(
                    ui,
                    &tf("stems.failed", &[("why", &why)]),
                    Role::Body,
                    s.t.danger,
                    ui.available_width(),
                );
            }
            Phase::Cancelled => {}
            phase => {
                let stage = match phase {
                    Phase::Preparing(_) => "stems.stage.prepare",
                    Phase::Separating(_) => "stems.stage.separate",
                    _ => "stems.stage.write",
                };
                form::band(ui, "stems.progress");
                form::progress(ui, r.done);
                ws::wrapped(
                    ui,
                    &t(stage),
                    Role::Micro,
                    s.t.text_dim,
                    ui.available_width(),
                );
                let close = form::buttons(ui, |ui| {
                    if form::button(ui, &t("stems.cancel"), false, true) {
                        run::cancel(app, i);
                    }
                    form::button(ui, &t("stems.hide"), true, true)
                });
                return close;
            }
        }
    }
    // The choices.
    form::band(ui, "stems.choose");
    let mut model = app.settings.stems.model.clone();
    for (id, key) in MODELS {
        if form::choice(ui, model == id, true, &t(key)) {
            model = id.to_string();
        }
    }
    if model != app.settings.stems.model {
        app.settings.stems.model = model.clone();
        app.store_settings();
    }
    let best = app.settings.stems.best;
    if form::choice(ui, !best, true, &t("stems.fast")) && best {
        app.settings.stems.best = false;
        app.store_settings();
    }
    if form::choice(
        ui,
        best,
        true,
        &tf("stems.best", &[("n", &super::BEST_SHIFTS)]),
    ) && !best
    {
        app.settings.stems.best = true;
        app.store_settings();
    }
    // Offered only where the runtime this system downloads carries a graphics provider
    // (CoreML on macOS); elsewhere said, not shown as a switch that does nothing.
    if super::gpu_provider(app, &model).is_some() {
        let gpu = app.settings.stems.gpu;
        if form::choice(ui, gpu, true, &t("stems.gpu")) {
            app.settings.stems.gpu = !gpu;
            app.store_settings();
        }
    } else {
        let w = ui.available_width();
        ws::wrapped(ui, &t("stems.gpu_unavailable"), Role::Body, s.t.text_dim, w);
    }
    // The model: ready, or what it takes to have it.
    form::band(ui, "stems.model");
    let status = app.ml.models().status(&model);
    let mut close = false;
    super::models::header(app, ui, s, &model);
    match status {
        ModelStatus::Ready => {
            close = form::buttons(ui, |ui| {
                let cancel = form::button(ui, &t("stems.close"), false, true);
                let can = super::models::ready(app, &model).is_some();
                if form::button(ui, &t("stems.separate"), true, can) {
                    match super::models::ready(app, &model) {
                        Some(ready) => {
                            if let Err(e) = run::start(app, target.clone(), ready) {
                                app.set_error(e);
                            }
                        }
                        None => app.set_error(t("stems.removing")),
                    }
                }
                cancel
            });
        }
        other => {
            if super::models::actions(app, ui, s, &model, &other) {
                close = true;
            }
        }
    }
    // What the model is: its card under the buttons.
    super::models::summary(app, ui, s, &model);
    close
}
