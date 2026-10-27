//! The command palette drawn with the kit: a floating window's frame (no title: the field is
//! the title, as in VS Code), the field in one well, the rows in a well — each a glyph of its
//! kind, the name, what it is at its right (a shortcut in the value face, a folder, a
//! device's kind) and its kind's word — and the keys' line. Only the rows in view are laid
//! out, sensed and painted.

use super::{Palette, ROW, VISIBLE, WIDTH};
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{well, with_brush, Brush, Fill, State, Style};
use crate::palette_index::{Entry, Kind, Target, Uses};
use crate::views::workspace_settings as ws;
use egui::{pos2, vec2, Align2, Context, Id, Key, Rect, Sense, Ui};

/// What a frame of the panel decided.
pub(super) enum Out {
    Stay,
    Close,
    /// Run the row at this index of the rows shown.
    Run(usize),
}

const FIELD: f32 = 30.0;
const KEYS: f32 = 18.0;
const PAD: f32 = 12.0;
const GAP: f32 = 8.0;
/// The kind's word at a row's right.
const KIND_W: f32 = 84.0;

/// The glyph a row of `e` shows.
fn glyph_of(e: &Entry) -> Glyph {
    match &e.target {
        Target::Pane(p) => p.glyph(),
        Target::Action(a) => match crate::views::transport::menus::pane_of(*a) {
            Some(p) => p.glyph(),
            None => match e.kind {
                _ if *a == crate::keymap::Action::ViewHistory => Glyph::Undo,
                _ => Glyph::Key,
            },
        },
        _ => match e.kind {
            Kind::Action => Glyph::Key,
            Kind::View => Glyph::Inspector,
            Kind::Window => Glyph::More,
            Kind::Settings => Glyph::Gear,
            Kind::Effect => Glyph::Device,
            Kind::Instrument => Glyph::Keyboard,
            Kind::Recent => Glyph::Arrangement,
            Kind::Template => Glyph::Plus,
            Kind::TrackTemplate => Glyph::Plus,
            Kind::NoteEffect => Glyph::Keyboard,
        },
    }
}

pub(super) fn show(st: &mut Palette, uses: &Uses, ctx: &Context) -> Out {
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let (up, down, page_up, page_down, enter, escape) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::PageUp),
            i.key_pressed(Key::PageDown),
            i.key_pressed(Key::Enter),
            i.key_pressed(Key::Escape),
        )
    });
    if escape {
        return Out::Close;
    }
    st.rerank(uses);
    let chrome = (PAD + s.t.density * FIELD + GAP + GAP + KEYS + PAD) * k;
    let width = (WIDTH * k).min(screen.width() - 16.0);
    // As tall as its rows (at most VISIBLE): a short answer is a short panel.
    let rows = st.shown_len().clamp(1, VISIBLE);
    let list_h = (rows as f32 * ROW * k + 2.0 * k)
        .min(screen.height() - 32.0 - chrome)
        .max(ROW * k);
    let height = chrome + list_h;
    // Near the top of the window, centred, wherever the window is this frame.
    let at = pos2(
        screen.center().x - width * 0.5,
        screen.top() + (screen.height() * 0.12).min(96.0 * k),
    );
    let rect = Rect::from_min_size(at, vec2(width, height));
    st.panel = Some(rect);
    let mut out = Out::Stay;
    egui::Area::new(Id::new("cantus-palette"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ws::frame(ui, rect, &s);
            let inner = rect.shrink(PAD * k);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.spacing_mut().item_spacing = vec2(0.0, GAP * k);
            let (field, _) = child
                .allocate_exact_size(vec2(child.available_width(), s.d(FIELD)), Sense::hover());
            let _ = crate::views::link_picker::search_field(
                &mut child,
                &mut st.query,
                Id::new("cantus-palette-field"),
                "workflow.palette.hint",
                &s,
                field,
                true,
            );
            st.rerank(uses);
            let stepped = up || down || page_up || page_down;
            if up {
                st.step(-1);
            }
            if down {
                st.step(1);
            }
            if page_up {
                st.step(-(VISIBLE as i32));
            }
            if page_down {
                st.step(VISIBLE as i32);
            }
            if let Some(i) = list(&mut child, st, &s, list_h, stepped) {
                out = Out::Run(i);
            }
            keys_line(&mut child, &s, st);
        });
    if matches!(out, Out::Stay) && enter && st.shown_len() > 0 {
        out = Out::Run(st.highlight);
    }
    let (pressed, pointer) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    let outside = !pointer.is_some_and(|p| rect.contains(p));
    if matches!(out, Out::Stay) && pressed && !st.fresh && outside {
        out = Out::Close;
    }
    out
}

/// The rows in their well; returns the row clicked.
fn list(ui: &mut Ui, st: &mut Palette, s: &Style, h: f32, stepped: bool) -> Option<usize> {
    let k = s.k;
    let (well_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    with_brush(ui.painter(), |b| {
        well::paint(b, well_rect, s, State::REST, None)
    });
    st.rects.clear();
    let inner = well_rect.shrink(k);
    let n = st.shown_len();
    if n == 0 {
        let words = tf("workflow.palette.no_match", &[("text", &st.query.trim())]);
        with_brush(ui.painter(), |b| {
            let t = Text::new(&words, Role::Body, s.t.placeholder)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, well_rect.shrink(8.0 * k), &t, s.tk());
        });
        return None;
    }
    let row_h = ROW * k;
    let total = n as f32 * row_h;
    let lane = Rect::from_min_max(
        pos2(inner.right() - 6.0 * k, inner.top() + 2.0 * k),
        pos2(inner.right() - 1.0 * k, inner.bottom() - 2.0 * k),
    );
    let rows_rect = if st.overflow {
        Rect::from_min_max(inner.min, pos2(lane.left() - 2.0 * k, inner.bottom()))
    } else {
        inner
    };
    if stepped || st.reveal {
        st.reveal = false;
        let (y, view) = (st.highlight as f32 * row_h, rows_rect.height());
        if y < st.offset {
            st.scroll_to = Some(y);
        } else if y + row_h > st.offset + view {
            st.scroll_to = Some(y + row_h - view);
        } else if st.highlight == 0 {
            st.scroll_to = Some(0.0);
        }
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rows_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rows_rect.intersect(ui.clip_rect()));
    let moved = child.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("cantus-palette-list")
        .auto_shrink([false, false])
        .max_height(rows_rect.height())
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(y) = st.scroll_to.take() {
        scroll = scroll.vertical_scroll_offset(y);
    }
    let mut chosen = None;
    let out = scroll.show_viewport(&mut child, |ui, view| {
        ui.set_height(total);
        let (top, left, w) = (
            ui.max_rect().top(),
            ui.max_rect().left(),
            ui.max_rect().width(),
        );
        let from = ((view.min.y / row_h).floor().max(0.0) as usize).min(n);
        let to = ((view.max.y / row_h).ceil() as usize + 1).min(n);
        let mut painted = Vec::with_capacity(to - from);
        for i in from..to {
            let rect = Rect::from_min_size(pos2(left, top + i as f32 * row_h), vec2(w, row_h));
            let resp = ui.interact(rect, Id::new(("cantus-palette-row", i)), Sense::click());
            if resp.hovered() && moved {
                st.highlight = i;
            }
            if resp.clicked() {
                chosen = Some(i);
            }
            painted.push((i, rect, resp.hovered()));
            st.rects.push((i, rect));
        }
        let painter = ui.painter().clone();
        with_brush(&painter, |b| {
            for &(i, rect, hover) in &painted {
                paint_row(b, rect, s, st.shown_entry(i), i == st.highlight, hover);
            }
        });
    });
    st.offset = out.state.offset.y;
    let content = out.content_size.y.max(1.0);
    let len = (out.inner_rect.height() / content).min(1.0);
    st.overflow = len < 1.0;
    if st.overflow {
        let mut start = out.state.offset.y / content;
        let id = Id::new("cantus-palette-scroll");
        if crate::kit::widgets::scrollbar(ui, lane, id, &mut start, len).dragged() {
            st.scroll_to = Some(start * content);
        }
    }
    chosen
}

/// One row: the glyph, the name, what it is (a shortcut in the value face), its kind.
fn paint_row(b: &mut Brush, rect: Rect, s: &Style, e: &Entry, hot: bool, hover: bool) {
    let k = s.k;
    if hot {
        b.rect(rect, Fill::Solid(mix(s.t.well, s.t.selection, 0.22)));
        let bar = Rect::from_min_size(rect.min, vec2(2.0 * k, rect.height()));
        b.rect(bar, Fill::Solid(s.t.selection));
    } else if hover {
        b.rect(
            rect,
            Fill::Solid(shade(s.t.well, if s.t.dark { 0.05 } else { -0.04 })),
        );
    }
    b.hline(
        rect.left(),
        rect.right(),
        rect.bottom() - b.px() * 0.5,
        b.px(),
        shade(s.t.well, if s.t.dark { -0.06 } else { -0.08 }),
    );
    let body = rect.shrink2(vec2(10.0 * k, 0.0));
    let icon = Rect::from_center_size(
        pos2(body.left() + 8.0 * k, body.center().y),
        vec2(14.0 * k, 14.0 * k),
    );
    let ink = if hot { s.t.text } else { s.t.text_dim };
    glyph::paint(b, icon, glyph_of(e), ink);
    // The kind's word in its column at the right, then what it is, then the name.
    let kind_r = Rect::from_min_max(pos2(body.right() - KIND_W * k, body.top()), body.max);
    let kind = Text {
        caps: true,
        ..Text::key(e.kind.key(), Role::Section, with_alpha(s.t.label, 0.85))
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::End))
    };
    let _ = text::paint(b, kind_r, &kind, s.tk());
    let mut right = kind_r.left() - 10.0 * k;
    if !e.detail.is_empty() {
        let role = if e.kind == Kind::Action {
            Role::Value
        } else {
            Role::Label
        };
        let d = Text::new(&e.detail, role, s.t.text_dim).align(Align2::RIGHT_CENTER);
        // At most 40 % of the row: a long folder is cut in its middle, the name keeps room.
        let room = (body.width() * 0.4)
            .min(right - body.left() - 120.0 * k)
            .max(0.0);
        let dw = text::measure(b.ctx(), &d, s.tk()).x.min(room);
        if dw > 0.0 {
            let dr = Rect::from_min_max(pos2(right - dw, body.top()), pos2(right, body.bottom()));
            let d = d.policy(Policy::Elide(Elide::Middle));
            let _ = text::paint(b, dr, &d, s.tk());
            right = dr.left() - 10.0 * k;
        }
    }
    let name_r = Rect::from_min_max(
        pos2(icon.right() + 8.0 * k, body.top()),
        pos2(right.max(icon.right() + 8.0 * k), body.bottom()),
    );
    let nt = Text::new(&e.name, Role::Body, s.t.text)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, name_r, &nt, s.tk());
}

/// "↑ ↓ choose · Enter runs · Esc closes", and how many rows match.
fn keys_line(ui: &mut Ui, s: &Style, st: &Palette) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), KEYS * s.k), Sense::hover());
    let count = tf("workflow.palette.count", &[("n", &st.shown_len())]);
    with_brush(ui.painter(), |b| {
        let c = Text::new(&count, Role::Label, s.t.text_dim).align(Align2::RIGHT_CENTER);
        let cw = text::measure(b.ctx(), &c, s.tk()).x;
        let _ = text::paint(b, r, &c, s.tk());
        let left = Rect::from_min_max(r.min, pos2(r.right() - cw - 12.0 * s.k, r.bottom()));
        let words = t("workflow.palette.keys");
        let keys = Text::new(&words, Role::Label, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, left, &keys, s.tk());
    });
}
