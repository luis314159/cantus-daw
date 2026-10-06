//! The *Content* window (ADR 0030): every pack in the manifest, what it is, what it
//! weighs, its licence and who made it; install, update, remove, cancel, and install
//! from an archive already on disk.

use super::interop::form;
use super::samples_paint::{self as paint, Icon};
use super::workspace_settings::{frame, title};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::Glyph;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Style};
use cantus_content::{Pack, Phase, Source, Status};
use egui::{pos2, vec2, Align2, Context, Id, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};

/// How far an install has got, 0..1.
pub fn fraction(phase: Option<&Phase>) -> f32 {
    match phase {
        Some(Phase::Downloading { done, total }) => (*done as f64 / (*total).max(1) as f64) as f32 * 0.9,
        Some(Phase::Verifying) => 0.9,
        Some(Phase::Unpacking { done, total }) => 0.9 + 0.1 * (*done as f64 / (*total).max(1) as f64) as f32,
        _ => 0.0,
    }
}

/// What an install is doing, in words.
pub fn phase_text(phase: Option<&Phase>) -> String {
    let mb = |b: u64| format!("{:.1}", b as f64 / 1e6);
    match phase {
        None => t("samples.phase.starting"),
        Some(Phase::Downloading { done, total }) => tf("samples.phase.downloading", &[("done", &mb(*done)), ("total", &mb(*total))]),
        Some(Phase::Retrying { error, wait }) => tf("samples.phase.retrying", &[("why", error), ("s", &wait.as_secs())]),
        Some(Phase::Verifying) => t("samples.phase.verifying"),
        Some(Phase::Unpacking { .. }) => t("samples.phase.unpacking"),
    }
}

fn size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        format!("{:.0} MB", (bytes as f64 / 1e6).max(1.0))
    }
}

/// The window.
pub fn window(app: &mut CantusApp, ctx: &Context) {
    let s = Style::current();
    let screen = ctx.content_rect();
    let want = Vec2::new(780.0, 560.0) * s.k;
    let size = want.min(screen.size() - Vec2::new(40.0, 100.0));
    let mut open = true;
    egui::Window::new("cantus-content")
        .id(Id::new("cantus-content"))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_size(size)
        .min_size(Vec2::new(560.0, 360.0).min(size))
        .default_pos(Pos2::new(screen.center().x - size.x * 0.5, screen.top() + 70.0))
        .constrain(true)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            frame(ui, rect, &s);
            let header = Rect::from_min_size(rect.min, vec2(rect.width(), 30.0 * s.k));
            if title(ui, header, &s, Glyph::Browser, "samples.content.title") {
                open = false;
            }
            body(app, ui, &s, Rect::from_min_max(pos2(rect.left(), header.bottom()), rect.max));
        });
    if !open {
        app.samples.window.open = false;
    }
}

fn body(app: &mut CantusApp, ui: &mut Ui, s: &Style, area: Rect) {
    let k = s.k;
    let packs: Vec<Pack> = app.samples.content().manifest().packs.clone();
    if app.samples.window.selected.is_none() {
        app.samples.window.selected = packs.first().map(|p| p.id.clone());
    }
    let list_w = (area.width() * 0.42).clamp(240.0 * k, 360.0 * k);
    let list = Rect::from_min_max(area.min + vec2(8.0, 8.0) * k, pos2(area.left() + list_w, area.bottom() - 8.0 * k));
    let detail = Rect::from_min_max(pos2(list.right() + 12.0 * k, area.top() + 10.0 * k), area.max - vec2(14.0, 10.0) * k);
    with_brush(ui.painter(), |b| {
        crate::kit::well::paint(b, list, s, crate::kit::State::REST, None);
    });
    let lang = crate::i18n::locale().code();
    let mut child = ui.new_child(UiBuilder::new().max_rect(list.shrink(2.0 * k)));
    egui::ScrollArea::vertical().id_salt("content-list").auto_shrink([false, false]).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let width = ui.available_width();
        let mut last_cat = None;
        for p in &packs {
            if last_cat != Some(p.category) {
                last_cat = Some(p.category);
                let (r, _) = ui.allocate_exact_size(vec2(width, 24.0 * k), Sense::hover());
                with_brush(ui.painter(), |b| paint::section(b, r, s, p.category.key()));
            }
            let (r, resp) = ui.allocate_exact_size(vec2(width, 40.0 * k), Sense::click());
            let status = app.samples.content().status(&p.id);
            let selected = app.samples.window.selected.as_deref() == Some(p.id.as_str());
            with_brush(ui.painter(), |b| pack_row(b, r, s, p, &status, selected, resp.hovered(), lang));
            if resp.clicked() {
                app.samples.window.selected = Some(p.id.clone());
                app.samples.window.confirm_remove = None;
            }
        }
    });
    let Some(pack) = app.samples.window.selected.as_ref().and_then(|id| packs.iter().find(|p| &p.id == id)).cloned() else {
        return;
    };
    let mut child = ui.new_child(UiBuilder::new().max_rect(detail));
    egui::ScrollArea::vertical().id_salt("content-detail").auto_shrink([false, false]).show(&mut child, |ui| {
        ui.set_width(detail.width() - 12.0 * k);
        ui.spacing_mut().item_spacing = vec2(6.0 * k, 6.0 * k);
        details(app, ui, s, &pack, lang);
    });
}

#[allow(clippy::too_many_arguments)]
fn pack_row(b: &mut crate::kit::Brush, r: Rect, s: &Style, p: &Pack, status: &Status, selected: bool, hovered: bool, lang: &str) {
    let k = s.k;
    if selected {
        b.rect(r, Fill::Solid(with_alpha(s.t.selection, 0.55)));
        b.rect(Rect::from_min_size(r.min, vec2(2.0 * k, r.height())), Fill::Solid(s.t.highlight));
    } else if hovered {
        b.rect(r, Fill::Solid(crate::theme::tint(s.t.well, s.t.text, 0.06)));
    }
    let ir = Rect::from_center_size(pos2(r.left() + 20.0 * k, r.center().y), vec2(18.0, 15.0) * k);
    paint::paint_icon(b, ir, s, Icon::Pack, s.t.text_dim);
    let x = ir.right() + 10.0 * k;
    let badge_w = 74.0 * k;
    let name_r = Rect::from_min_max(pos2(x, r.top() + 4.0 * k), pos2(r.right() - badge_w - 6.0 * k, r.center().y + 2.0 * k));
    paint::line(b, name_r, s, Src::Str(p.name.get(lang)), Role::Control, s.t.text, Align2::LEFT_CENTER);
    let meta = format!("{} · {}", size(p.download_size()), p.licence);
    let meta_r = Rect::from_min_max(pos2(x, r.center().y), pos2(r.right() - badge_w - 6.0 * k, r.bottom() - 4.0 * k));
    paint::line(b, meta_r, s, Src::Str(&meta), Role::Micro, s.t.text_dim, Align2::LEFT_CENTER);
    // The state, as a chip at the right.
    let (key, tone) = match status {
        Status::NotInstalled => (None, s.t.text_dim),
        Status::Installed { .. } => (Some("samples.state.installed"), s.t.led),
        Status::Update { .. } => (Some("samples.state.update"), s.t.highlight),
        Status::Busy(_) => (Some("samples.state.busy"), s.t.highlight),
    };
    if let Some(key) = key {
        let chip = Rect::from_center_size(pos2(r.right() - badge_w * 0.5 - 6.0 * k, r.center().y), vec2(badge_w, 18.0 * k));
        b.rrect(chip, 9.0 * k, Fill::Solid(crate::theme::tint(s.t.well, tone, 0.22)));
        b.rrect_stroke(chip, 9.0 * k, b.px(), with_alpha(tone, 0.6));
        paint::line(b, chip, s, Src::Key(key), Role::Micro, s.t.text, Align2::CENTER_CENTER);
    }
    b.hline(r.left() + 8.0 * k, r.right() - 8.0 * k, r.bottom() - b.px() * 0.5, b.px(), shade(s.t.well, -0.08));
}

/// Text that wraps to the width it is given (descriptions, credits, errors).
fn text_block(ui: &mut Ui, s: &Style, src: Src<'_>, role: Role, color: egui::Color32, _lines: f32) {
    let text = match src {
        Src::Str(t) => t.to_string(),
        Src::Key(k) => t(k),
    };
    let width = ui.available_width();
    let galley = ui.painter().layout(text, role.font(s.tk()), color, width);
    let (r, _) = ui.allocate_exact_size(vec2(width, galley.size().y), Sense::hover());
    ui.painter().galley(r.min, galley, color);
}

fn details(app: &mut CantusApp, ui: &mut Ui, s: &Style, pack: &Pack, lang: &str) {
    let status = app.samples.content().status(&pack.id);
    let error = app.samples.content().error(&pack.id).map(|e| e.to_string());
    text_block(ui, s, Src::Str(pack.name.get(lang)), Role::Title, s.t.text, 1.4);
    text_block(ui, s, Src::Str(pack.description.get(lang)), Role::Body, s.t.text_dim, 3.0);
    let files = match &pack.source {
        Source::Files(f) => f.len().to_string(),
        Source::Zip(_) => "—".into(),
    };
    for (label, value) in [
        (t("samples.detail.download"), size(pack.download_size())),
        (t("samples.detail.on_disk"), size(pack.installed_size)),
        (t("samples.detail.files"), files),
        (t("samples.detail.licence"), format!("{} — {}", pack.licence, t("samples.licence.cc0"))),
        (t("samples.detail.checked"), pack.checked.clone()),
    ] {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0 * s.k), Sense::hover());
        with_brush(ui.painter(), |b| {
            let lw = 110.0 * s.k;
            paint::line(b, Rect::from_min_max(r.min, pos2(r.left() + lw, r.bottom())), s, Src::Str(&label), Role::Body, s.t.text_dim, Align2::LEFT_CENTER);
            paint::line(b, Rect::from_min_max(pos2(r.left() + lw, r.top()), r.max), s, Src::Str(&value), Role::Value, s.t.text, Align2::LEFT_CENTER);
        });
    }
    form::band(ui, "samples.detail.credits");
    for c in &pack.credits {
        text_block(ui, s, Src::Str(&c.name), Role::Body, s.t.text, 1.0);
        text_block(ui, s, Src::Str(&c.url), Role::Micro, s.t.text_dim, 1.0);
    }
    text_block(ui, s, Src::Str(&tf("samples.detail.licence_kept", &[("file", &pack.licence_file)])), Role::Micro, s.t.text_dim, 2.0);
    form::band(ui, "samples.detail.actions");
    match &status {
        Status::Busy(phase) => {
            form::progress(ui, fraction(phase.as_ref()));
            text_block(ui, s, Src::Str(&phase_text(phase.as_ref())), Role::Micro, s.t.text_dim, 1.0);
            if form::button(ui, &t("samples.cancel"), false, true) {
                app.samples.content().cancel(&pack.id);
            }
        }
        _ => {
            let installed = !matches!(status, Status::NotInstalled);
            ui.horizontal_wrapped(|ui| {
                let label = match status {
                    Status::Update { .. } => t("samples.update"),
                    Status::Installed { .. } => t("samples.reinstall"),
                    _ => t("samples.install"),
                };
                if form::button(ui, &label, !installed || matches!(status, Status::Update { .. }), true) {
                    app.samples.content().install(&pack.id);
                }
                if installed {
                    if app.samples.window.confirm_remove.as_deref() == Some(pack.id.as_str()) {
                        if form::button(ui, &t("samples.remove_confirm"), true, true) {
                            let r = app.samples.content().remove(&pack.id);
                            app.samples.window.confirm_remove = None;
                            app.samples.notice = Some(match r {
                                Ok(()) => tf("samples.notice.removed", &[("pack", &pack.name.get(lang))]),
                                Err(e) => e.to_string(),
                            });
                            app.samples.refresh();
                        }
                    } else if form::button(ui, &t("samples.remove"), false, true) {
                        app.samples.window.confirm_remove = Some(pack.id.clone());
                    }
                    if form::button(ui, &t("samples.show_folder"), false, true) {
                        let dir = app.samples.content().pack_dir(&pack.id);
                        let _ = crate::app::reveal(&dir);
                    }
                }
                if matches!(pack.source, Source::Zip(_)) && form::button(ui, &t("samples.from_file"), false, true) {
                    if let Some(f) = rfd::FileDialog::new().add_filter("zip", &["zip"]).pick_file() {
                        app.samples.content().install_from_file(&pack.id, &f);
                    }
                }
            });
            if let Some(e) = error {
                text_block(ui, s, Src::Str(&e), Role::Micro, s.t.danger, 2.0);
            }
        }
    }
}
