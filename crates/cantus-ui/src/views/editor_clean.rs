//! *Remove unused audio…* (ADR 0096 §7), drawn with the kit: what will go — the pool's
//! entries no clip plays and the editor's files nothing refers to, each with its size — and
//! the total, before anything is removed. One undo step for the pool; the files of entries
//! it removes stay until the history no longer holds them.

use crate::app::CantusApp;
use crate::editor::commit::{self, bytes_text};
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::views::audio_parts as parts;
use egui::{Align2, Id, Pos2, Rect, Sense, Vec2};

/// The dialog, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(plan) = app.editor.cleanup.clone() else {
        return;
    };
    let st = Style::current();
    let k = st.k;
    let screen = ctx.content_rect();
    let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("cantus-editor-clean"));
    egui::Area::new(Id::new("cantus-editor-clean-veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            with_brush(ui.painter(), |b| {
                b.rect(
                    r,
                    Fill::Solid(crate::kit::color::with_alpha(st.t.shadow, 0.4)),
                )
            });
        });
    let size = (Vec2::new(560.0, 480.0) * k).min(screen.size() - Vec2::splat(24.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let (mut close, mut go) = (false, false);
    egui::Area::new(layer.id)
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            parts::panel(ui, rect, &st);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 34.0 * k));
            close |= crate::kit::dialog::head(ui, head, &st, Glyph::Warning, "editor.clean.title");
            let foot = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, rect.bottom() - 48.0 * k),
                Pos2::new(rect.right() - 18.0 * k, rect.bottom() - 14.0 * k),
            );
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, head.bottom() + 10.0 * k),
                Pos2::new(rect.right() - 18.0 * k, foot.top() - 8.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .id_salt("cantus-editor-clean-list")
                .auto_shrink([false, false])
                .max_height(body.height())
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 12.0 * k);
                    let w = ui.available_width();
                    let note = if plan.sources.is_empty() && plan.files.is_empty() {
                        t("editor.clean.nothing")
                    } else {
                        tf(
                            "editor.clean.summary",
                            &[
                                ("n", &plan.sources.len().to_string()),
                                ("f", &plan.files.len().to_string()),
                                ("size", &bytes_text(plan.bytes())),
                            ],
                        )
                    };
                    crate::views::workspace_settings::wrapped(ui, &note, Role::Body, st.t.text, w);
                    for (_, name, size) in &plan.sources {
                        row(ui, &st, name, &bytes_text(*size));
                    }
                    for (path, size) in &plan.files {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        row(ui, &st, &name, &bytes_text(*size));
                    }
                    crate::views::workspace_settings::wrapped(
                        ui,
                        &t("editor.clean.kept"),
                        Role::Body,
                        st.t.text_dim,
                        w,
                    );
                });
            let mut c = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(foot)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            let any = !plan.sources.is_empty() || !plan.files.is_empty();
            go = parts::button(&mut c, &t("editor.clean.go"), true, any, 110.0).clicked() && any;
            c.add_space(8.0 * k);
            close |= parts::button(&mut c, &t("editor.clean.cancel"), false, true, 90.0).clicked();
        });
    ctx.move_to_top(layer);
    if go {
        let (entries, files) = commit::run_cleanup(app);
        app.set_status(tf(
            "editor.clean.done",
            &[("n", &entries.to_string()), ("f", &files.to_string())],
        ));
        app.editor.cleanup = None;
    } else if close {
        app.editor.cleanup = None;
    }
}

/// One thing that will go: its name and its size.
fn row(ui: &mut egui::Ui, st: &Style, name: &str, size: &str) {
    let (r, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), st.d(20.0)), Sense::hover());
    let split = r.right() - 90.0 * st.k;
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            Rect::from_min_max(r.min, Pos2::new(split - 8.0, r.bottom())),
            &Text::new(name, Role::Body, st.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
            st.tk(),
        );
        let _ = text::paint(
            b,
            Rect::from_min_max(Pos2::new(split, r.top()), r.max),
            &Text::new(size, Role::Value, st.t.text_dim)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            st.tk(),
        );
    });
}
