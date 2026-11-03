//! The audio editor's window (ADR 0096), drawn with the kit: a large floating panel over a
//! dimmed room — title and the take, the toolbar of edits, the graphs on the left, the tools
//! on the right, and a foot that says the selection to the frame and what just happened.

use super::view_tools::{self, hz_text};
use super::view_wave as wave;
use super::{Act, Show};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::views::audio_parts as parts;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, Id::new("cantus-editor"))
}

/// The window, when a take is open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.editor.is_open() {
        super::view_clean::show(app, ctx);
        return;
    }
    let st = Style::current();
    let k = st.k;
    let screen = ctx.content_rect();
    egui::Area::new(Id::new("cantus-editor-veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            with_brush(ui.painter(), |b| {
                b.rect(
                    r,
                    Fill::Solid(crate::kit::color::with_alpha(st.t.shadow, 0.55)),
                )
            });
        });
    let size = Vec2::new(
        (screen.width() * 0.94).min(1560.0 * k),
        (screen.height() * 0.92).min(940.0 * k),
    );
    let rect = Rect::from_center_size(screen.center(), size);
    let mut acts: Vec<Act> = Vec::new();
    let mut tools = app.editor.tools.clone();
    let mut show_mode = app.editor.show;
    let has_clip = app.editor.clipboard.is_some();
    egui::Area::new(layer().id)
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            parts::panel(ui, rect, &st);
            let Some(mut s) = app.editor.session.take() else {
                return;
            };
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 34.0 * k));
            if title(ui, head, &st, &tf("editor.title", &[("name", &s.name)])) {
                acts.push(Act::Close);
            }
            let bar = Rect::from_min_size(
                Pos2::new(rect.left() + 14.0 * k, head.bottom() + 6.0 * k),
                Vec2::new(rect.width() - 28.0 * k, 30.0 * k),
            );
            toolbar(ui, bar, &s, &mut show_mode, has_clip, &mut acts);
            let foot = Rect::from_min_max(
                Pos2::new(rect.left() + 16.0 * k, rect.bottom() - 34.0 * k),
                Pos2::new(rect.right() - 16.0 * k, rect.bottom() - 8.0 * k),
            );
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 14.0 * k, bar.bottom() + 8.0 * k),
                Pos2::new(rect.right() - 14.0 * k, foot.top() - 6.0 * k),
            );
            let side_w = view_tools::WIDTH * k;
            let graphs = Rect::from_min_max(
                body.min,
                Pos2::new(body.right() - side_w - 10.0 * k, body.bottom()),
            );
            let side = Rect::from_min_max(Pos2::new(body.right() - side_w, body.top()), body.max);
            if s.buf.is_some() {
                wave::show(ui, graphs, rect, &mut s, show_mode);
                // (the picked rectangle is drawn by the graphs themselves)
            } else {
                busy_note(ui, graphs, &st, &s);
            }
            view_tools::show(ui, side, &mut tools, &s, &mut acts);
            footer(ui, foot, &st, &s);
            app.editor.session = Some(s);
        });
    ctx.move_to_top(layer());
    app.editor.tools = tools;
    app.editor.show = show_mode;
    for a in acts {
        crate::editor::act(app, a);
    }
    super::view_clean::show(app, ctx);
}

fn title(ui: &mut Ui, rect: Rect, s: &Style, name: &str) -> bool {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        b.hline(
            r.left() + 1.0,
            r.right() - 1.0,
            r.bottom() - b.px() * 0.5,
            b.px(),
            crate::kit::color::shade(s.t.panel, -0.16),
        );
        let gr = Rect::from_center_size(
            Pos2::new(r.left() + 18.0 * k, r.center().y),
            Vec2::splat(14.0 * k),
        );
        crate::kit::glyph::paint(b, gr, Glyph::Zoom, s.t.text_dim);
        let tr = Rect::from_min_max(
            Pos2::new(gr.right() + 8.0 * k, r.top()),
            Pos2::new(r.right() - 40.0 * k, r.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &Text::new(name, Role::Title, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    let keys = Rect::from_min_max(
        Pos2::new(rect.right() - 36.0 * k, rect.top() + 4.0 * k),
        Pos2::new(rect.right() - 8.0 * k, rect.bottom() - 4.0 * k),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(keys)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    crate::views::workspace_widgets::glyph_key(
        &mut child,
        Glyph::Close,
        false,
        22.0,
        &t("editor.close"),
    )
    .clicked()
}

fn toolbar(
    ui: &mut Ui,
    bar: Rect,
    s: &super::Session,
    show_mode: &mut Show,
    has_clip: bool,
    acts: &mut Vec<Act>,
) {
    let st = Style::current();
    let k = st.k;
    let busy = s.busy.is_some() || s.buf.is_none();
    let sel = s.sel.is_some_and(|x| !x.is_empty());
    // The edits on the left, scrolled sideways when a language makes them wider than the
    // window; the view switch fixed on the right.
    let seg_w = 250.0;
    let left = Rect::from_min_max(
        bar.min,
        Pos2::new(bar.right() - (seg_w + 12.0) * k, bar.bottom()),
    );
    let right = Rect::from_min_max(Pos2::new(bar.right() - seg_w * k, bar.top()), bar.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(left));
    egui::ScrollArea::horizontal()
        .id_salt("cantus-editor-toolbar")
        .auto_shrink([false, true])
        .show(&mut child, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(5.0 * k, 0.0);
                let b = |ui: &mut Ui, key: &str, on: bool, primary: bool| -> bool {
                    parts::button(ui, &t(key), primary, on, 40.0).clicked() && on
                };
                if s.play.is_some() {
                    if b(ui, "editor.stop", true, true) {
                        acts.push(Act::Stop);
                    }
                } else if b(ui, "editor.play", s.buf.is_some(), true) {
                    acts.push(Act::Play);
                }
                ui.add_space(8.0 * k);
                if b(ui, "editor.cut", !busy && sel, false) {
                    acts.push(Act::Cut);
                }
                if b(ui, "editor.copy", sel, false) {
                    acts.push(Act::Copy);
                }
                if b(ui, "editor.paste", !busy && has_clip, false) {
                    acts.push(Act::Paste);
                }
                if b(ui, "editor.delete", !busy && sel, false) {
                    acts.push(Act::Edit(cantus_core::audio_edit::Edit::Delete));
                }
                if b(ui, "editor.trim", !busy && sel, false) {
                    acts.push(Act::Edit(cantus_core::audio_edit::Edit::Trim));
                }
                if b(ui, "editor.silence", !busy && sel, false) {
                    acts.push(Act::Edit(cantus_core::audio_edit::Edit::Silence));
                }
                ui.add_space(8.0 * k);
                if b(ui, "editor.invert", !busy, false) {
                    acts.push(Act::Edit(cantus_core::audio_edit::Edit::Invert));
                }
                if b(ui, "editor.reverse", !busy, false) {
                    acts.push(Act::Edit(cantus_core::audio_edit::Edit::Reverse));
                }
                ui.add_space(8.0 * k);
                if b(ui, "editor.undo", !busy, false) {
                    acts.push(Act::Undo);
                }
                if b(ui, "editor.redo", !busy, false) {
                    acts.push(Act::Redo);
                }
                ui.add_space(8.0 * k);
                if b(ui, "editor.marker", s.buf.is_some(), false) {
                    acts.push(Act::AddMarker);
                }
                if b(ui, "editor.region", sel, false) {
                    acts.push(Act::AddRegion);
                }
                if b(ui, "editor.loop", sel, false) {
                    acts.push(Act::SetLoop);
                }
                if b(ui, "editor.marker_clear", !s.marks.is_empty(), false) {
                    acts.push(Act::ClearMarks);
                }
                if b(ui, "editor.fit", s.buf.is_some(), false) {
                    acts.push(Act::FitAll);
                }
                if b(ui, "editor.fit_sel", sel, false) {
                    acts.push(Act::FitSelection);
                }
                ui.add_space(8.0 * k);
                if b(ui, "editor.to_clip", !busy && sel, false) {
                    acts.push(Act::ToNewClip);
                }
                if b(ui, "editor.open_file", true, false) {
                    acts.push(Act::OpenFile);
                }
                if b(ui, "editor.paste_file", !busy, false) {
                    acts.push(Act::PasteFile);
                }
                if b(ui, "editor.cleanup", true, false) {
                    acts.push(Act::Cleanup);
                }
            });
        });
    let mut rc = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(right)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    {
        let ui = &mut rc;
        let mut i = match *show_mode {
            Show::Wave => 0,
            Show::Both => 1,
            Show::Spectral => 2,
        };
        crate::kit::widgets::segmented(
            ui,
            &mut i,
            &[
                Src::Key("editor.show.wave"),
                Src::Key("editor.show.both"),
                Src::Key("editor.show.spectral"),
            ],
            &t("editor.show"),
            seg_w,
        );
        *show_mode = [Show::Wave, Show::Both, Show::Spectral][i];
    }
}

fn busy_note(ui: &mut Ui, rect: Rect, st: &Style, s: &super::Session) {
    let msg = match (s.busy, &s.status) {
        (Some(key), _) => t(key),
        (None, Some((m, _))) => m.clone(),
        _ => String::new(),
    };
    with_brush(ui.painter(), |b| {
        b.rect(rect, Fill::Solid(st.t.row_black));
        let _ = text::paint(
            b,
            rect,
            &Text::new(&msg, Role::Body, st.t.text_dim),
            st.tk(),
        );
    });
}

/// The selection to the frame, the band, the length; the thread's work or the last result.
fn footer(ui: &mut Ui, rect: Rect, st: &Style, s: &super::Session) {
    let rate = s.buf.as_ref().map_or(48_000, |b| b.rate);
    let frames = s.buf.as_ref().map_or(0, |b| b.frames());
    let sel = match s.sel {
        Some(x) if !x.is_empty() => {
            let mut m = tf(
                "editor.foot.sel",
                &[
                    ("start", &wave::exact_text(x.start, rate)),
                    ("end", &wave::exact_text(x.end, rate)),
                    ("len", &wave::time_text(x.len() as f64, rate)),
                ],
            );
            if let Some(b) = s.band {
                m.push_str(&format!("   {} - {}", hz_text(b.lo_hz), hz_text(b.hi_hz)));
            }
            m
        }
        Some(x) => tf(
            "editor.foot.cursor",
            &[("at", &wave::exact_text(x.start, rate))],
        ),
        None => tf(
            "editor.foot.take",
            &[
                ("len", &wave::time_text(frames as f64, rate)),
                ("rate", &rate.to_string()),
                ("ch", &s.buf.as_ref().map_or(0, |b| b.channels).to_string()),
            ],
        ),
    };
    let right = match (s.busy, &s.status) {
        (Some(key), _) => (t(key), false),
        (None, Some((m, warn))) => (m.clone(), *warn),
        _ => (String::new(), false),
    };
    let half = rect.width() * 0.5;
    let l = Rect::from_min_size(rect.min, Vec2::new(half - 8.0 * st.k, rect.height()));
    let r = Rect::from_min_max(Pos2::new(rect.left() + half, rect.top()), rect.max);
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            l,
            &Text::new(&sel, Role::Body, st.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
            st.tk(),
        );
        let _ = text::paint(
            b,
            r,
            &Text::new(
                &right.0,
                Role::Body,
                if right.1 { st.t.danger } else { st.t.text },
            )
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::Middle)),
            st.tk(),
        );
    });
}
