//! The *Link to…* panel, drawn with the kit: a floating window's frame and title, a line
//! saying what is being linked (the name cut, never the verb), the filter field (one well,
//! its focus on the well) beside the *Linked only* switch, the list in a well (a band over
//! the groups and one over the clips of their own; two-line rows; only the rows in view
//! laid out), and the keys' line.

use super::{LinkPicker, BAND, ROW, VISIBLE, WIDTH};
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{led, well, with_brush, Brush, Fill, State, Style};
use crate::links::{kind_key, refusal_key, PickRow};
use crate::views::workspace_settings as ws;
use egui::{pos2, vec2, Align2, Context, Id, Key, Rect, Sense, Ui};

/// What a frame of the panel decided.
pub(super) enum Out {
    Stay,
    Close,
    Choose(ClipRef),
}

/// The title row, the lead line, the field and the keys' line, with the gaps between.
const TITLE: f32 = 32.0;
const LEAD: f32 = 18.0;
const FIELD: f32 = 24.0;
const KEYS: f32 = 18.0;
const PAD: f32 = 12.0;
const GAP: f32 = 8.0;
/// The *Linked only* switch beside the field.
const SWITCH: f32 = 124.0;
/// The text's inset inside the filter's well.
const INSET: f32 = 8.0;

pub(super) fn show(st: &mut LinkPicker, app: &mut CantusApp, ctx: &Context) -> Out {
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let (up, down, enter, escape) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::Enter),
            i.key_pressed(Key::Escape),
        )
    });
    if escape {
        return Out::Close;
    }
    st.rebuild(app);
    let chrome = (TITLE + GAP + LEAD + GAP + s.t.density * FIELD + GAP * 2.0 + KEYS + PAD) * k;
    if st.list_h == 0.0 {
        let n = st.shown.len();
        let groups = st
            .shown
            .iter()
            .filter(|&&i| st.list.rows()[i].group.count > 1)
            .count();
        let bands = (groups > 0) as u8 + (n > groups) as u8;
        // The rows, the bands, and the well's hairline inset above and below them.
        st.list_h = (n.clamp(1, VISIBLE) as f32 * ROW + bands as f32 * BAND + 2.0) * k;
        // Where it was asked to hang, or the pointer; with neither (a key, no pointer
        // yet), centred near the top of the window, wherever the window is this frame.
        st.at = st.anchor.or_else(|| ctx.pointer_latest_pos());
    }
    let width = (WIDTH * k).min(screen.width() - 16.0);
    let list_h = st.list_h.min(screen.height() - 16.0 - chrome).max(ROW * k);
    let height = chrome + list_h;
    let want = st.at.unwrap_or(pos2(
        screen.center().x - width * 0.5,
        screen.top() + screen.height() * 0.2,
    ));
    let at = pos2(
        want.x
            .min(screen.right() - 8.0 - width)
            .max(screen.left() + 8.0),
        want.y
            .min(screen.bottom() - 8.0 - height)
            .max(screen.top() + 8.0),
    );
    let rect = Rect::from_min_size(at, vec2(width, height));
    st.panel = Some(rect);
    let mut out = Out::Stay;
    egui::Area::new(Id::new("cantus-link-picker"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ws::frame(ui, rect, &s);
            let title = Rect::from_min_size(rect.min, vec2(width, TITLE * k));
            if ws::title(ui, title, &s, Glyph::Link, "links.picker.title") {
                out = Out::Close;
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
            lead(&mut child, app, st, &s);
            field_row(&mut child, st, &s);
            st.rebuild(app);
            let stepped = up || down;
            if up {
                st.step(-1);
            }
            if down {
                st.step(1);
            }
            if let Some(target) = list(&mut child, st, &s, list_h, stepped) {
                out = Out::Choose(target);
            }
            keys_line(&mut child, &s);
        });
    if matches!(out, Out::Stay) && enter {
        if let Some(h) = st.highlight {
            out = Out::Choose(h);
        }
    }
    let (pressed, pointer) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    let outside = !pointer.is_some_and(|p| rect.contains(p));
    if matches!(out, Out::Stay) && pressed && !st.fresh && outside {
        out = Out::Close;
    }
    out
}

/// "“Beat B” becomes the clip you choose" — what is being linked. A long name is cut so
/// that the words saying what will happen always show; made once per width and language.
fn lead(ui: &mut Ui, app: &CantusApp, st: &mut LinkPicker, s: &Style) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), LEAD * s.k), Sense::hover());
    let key = (
        r.width().to_bits(),
        crate::i18n::locale() as u8,
        app.bus.revision(),
    );
    if st.lead_for != Some(key) {
        st.lead = lead_text(ui.ctx(), app, &st.sel, s, r.width());
        st.lead_for = Some(key);
    }
    with_brush(ui.painter(), |b| {
        let t = Text::new(&st.lead, Role::Body, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, r, &t, s.tk());
    });
}

fn lead_text(ctx: &Context, app: &CantusApp, sel: &[ClipRef], s: &Style, width: f32) -> String {
    let [one] = sel else {
        return tf("links.picker.lead", &[("n", &sel.len())]);
    };
    let name = crate::clips::clip_of(app.project(), *one)
        .map(|c| c.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| t("links.picker.unnamed"));
    let template = t("links.picker.lead_one");
    let Some((before, after)) = template.split_once("{name}") else {
        return tf("links.picker.lead_one", &[("name", &name)]);
    };
    let wide = |x: &str| text::measure(ctx, &Text::new(x, Role::Body, s.t.text), s.tk()).x;
    let room = width - wide(&format!("{before}{after}")) - 2.0 * s.k;
    if wide(&name) <= room {
        return format!("{before}{name}{after}");
    }
    // The longest start of the name that fits with its ellipsis.
    let chars: Vec<char> = name.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cut: String = chars[..mid]
            .iter()
            .collect::<String>()
            .trim_end()
            .to_string()
            + "…";
        if wide(&cut) <= room {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let cut = chars[..lo]
        .iter()
        .collect::<String>()
        .trim_end()
        .to_string();
    format!("{before}{cut}…{after}")
}

/// The filter field and the *Linked only* switch, side by side.
fn field_row(ui: &mut Ui, st: &mut LinkPicker, s: &Style) {
    let k = s.k;
    let h = s.d(FIELD);
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let switch = Rect::from_min_max(
        pos2(row.right() - SWITCH * k, row.center().y - s.d(10.0)),
        pos2(row.right(), row.center().y + s.d(10.0)),
    );
    let field = Rect::from_min_max(row.min, pos2(switch.left() - GAP * k, row.bottom()));
    let id = Id::new("cantus-link-picker-filter");
    let _ = search_field(
        ui,
        &mut st.filter,
        id,
        "links.picker.filter",
        s,
        field,
        true,
    );
    let r = ui.interact(
        switch,
        Id::new("cantus-link-picker-groups-only"),
        Sense::click(),
    );
    if r.clicked() {
        st.groups_only = !st.groups_only;
    }
    if r.hovered() {
        let name = t("links.picker.groups_only_tip");
        hint::set_text(ui.ctx(), &name, "", Caps::NONE);
    }
    with_brush(ui.painter(), |b| {
        led::button(
            b,
            switch,
            s,
            State::of(&r, st.groups_only),
            Src::Key("links.picker.groups_only"),
        )
    });
}

/// One well that is the field: the text edits over the whole of it, inset by the kit's
/// margin, so the focus ring outlines the well and nothing is framed twice; the
/// placeholder is the kit's own text, painted while the field is empty.
/// A search field that is one well: the text edits over the whole of it, inset by the
/// kit's margin, so the focus ring outlines the well and nothing is framed twice; the
/// placeholder (`hint_key`) is the kit's own text, painted while the field is empty. Kept
/// focused when `keep_focus` (the picker types into it at once). Shared with the clip
/// browser.
pub(crate) fn search_field(
    ui: &mut Ui,
    text_: &mut String,
    id: Id,
    hint_key: &str,
    s: &Style,
    rect: Rect,
    keep_focus: bool,
) -> egui::Response {
    let _ = s;
    // The kit's field (round 26): the hint and the typed words start at one inset (the
    // frame's own margin), so nothing jumps on the first key.
    let edit = crate::kit::field::edit_id(id);
    if keep_focus && !ui.memory(|m| m.has_focus(edit)) {
        ui.memory_mut(|m| m.request_focus(edit));
    }
    let hint = crate::i18n::t(hint_key);
    let resp = crate::kit::field::Field::new(id)
        .hint(&hint)
        .pad(INSET - 6.0, 0.0)
        .show(ui, rect, text_)
        .resp;
    // The arrows stay with the field (a list under it reads them): egui would otherwise
    // move the focus to the next widget, and the letter typed next would be lost.
    if resp.has_focus() {
        let filter = egui::EventFilter {
            vertical_arrows: true,
            horizontal_arrows: true,
            ..Default::default()
        };
        ui.memory_mut(|m| m.set_focus_lock_filter(edit, filter));
    }
    resp
}

/// The list in its well; returns the row clicked when it links. Only the rows in view are
/// laid out, painted and sensed: the rows have one height and the two bands another, so
/// where row `i` sits is arithmetic.
fn list(ui: &mut Ui, st: &mut LinkPicker, s: &Style, h: f32, stepped: bool) -> Option<ClipRef> {
    let k = s.k;
    let (well_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    with_brush(ui.painter(), |b| {
        well::paint(b, well_rect, s, State::REST, None)
    });
    st.rects.clear();
    let inner = well_rect.shrink(k);
    if st.shown.is_empty() {
        let text = if st.filter.trim().is_empty() {
            t("links.picker.empty")
        } else {
            tf("links.picker.no_match", &[("text", &st.filter.trim())])
        };
        with_brush(ui.painter(), |b| {
            let t = Text::new(&text, Role::Body, s.t.placeholder)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, well_rect.shrink(8.0 * k), &t, s.tk());
        });
        return None;
    }
    // Groups come first in the list: the rows before `groups` sit under the first band.
    let n = st.shown.len();
    let groups = {
        let rows = st.list.rows();
        st.shown.partition_point(|&i| rows[i].group.count > 1)
    };
    let (row_h, band_h) = (ROW * k, BAND * k);
    let first_band = groups > 0;
    let second_band = n > groups;
    let y_of = |i: usize| {
        first_band as u8 as f32 * band_h
            + i as f32 * row_h
            + (i >= groups && second_band) as u8 as f32 * band_h
    };
    let total = (first_band as u8 + second_band as u8) as f32 * band_h + n as f32 * row_h;
    // A list longer than the well scrolls under the kit's scrollbar (ADR 0024 §10), in a
    // lane the rows leave free only while they overflow.
    let lane = Rect::from_min_max(
        pos2(inner.right() - 6.0 * k, inner.top() + 2.0 * k),
        pos2(inner.right() - 1.0 * k, inner.bottom() - 2.0 * k),
    );
    let rows_rect = if st.overflow {
        Rect::from_min_max(inner.min, pos2(lane.left() - 2.0 * k, inner.bottom()))
    } else {
        inner
    };
    // The keys keep the highlighted row in view, and so does a change of the rows shown (a
    // filter, the switch). Going up to the first row under a band brings the band too.
    if stepped || st.reveal {
        st.reveal = false;
        let hot = st
            .highlight
            .and_then(|hl| (0..n).find(|&i| st.row(i).group.first == hl));
        if let Some(i) = hot {
            let (y, view) = (y_of(i), rows_rect.height());
            let under_band = (i == 0 && first_band) || (i == groups && second_band);
            let top = if under_band { (y - band_h).max(0.0) } else { y };
            if top < st.offset {
                st.scroll_to = Some(top);
            } else if y + row_h > st.offset + view {
                st.scroll_to = Some(y + row_h - view);
            }
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
        .id_salt("cantus-link-picker-list")
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
        let shows = |y: f32, h: f32| y + h > view.min.y && y < view.max.y;
        let at = |y: f32, h: f32| Rect::from_min_size(pos2(left, top + y), vec2(w, h));
        // Sensed first, then painted with one brush: the cost of a frame does not grow
        // with the rows in view, nor at all with the rows out of it.
        let from = (((view.min.y - 2.0 * band_h) / row_h).floor().max(0.0) as usize).min(n);
        let to = ((view.max.y / row_h).ceil() as usize + 1).min(n);
        st.painted.clear();
        for i in from..to {
            let y = y_of(i);
            if !shows(y, row_h) {
                continue;
            }
            let rect = at(y, row_h);
            let r = st.row(i);
            let target = r.group.first;
            let id = Id::new(("cantus-link-pick", target.0 .0, target.1 .0));
            let resp = ui.interact(rect, id, Sense::click());
            let links = r.refusal.is_none();
            if resp.hovered() {
                hover_hint(ui.ctx(), r);
            }
            if links && resp.hovered() && moved {
                st.highlight = Some(target);
            }
            if resp.clicked() && links {
                chosen = Some(target);
            }
            st.painted.push((i, resp.hovered()));
            st.rects.push((target, rect));
        }
        let second_at = y_of(groups) - band_h;
        let painter = ui.painter().clone();
        with_brush(&painter, |b| {
            if first_band && shows(0.0, band_h) {
                band(b, s, at(0.0, band_h), "links.picker.groups");
            }
            if second_band && shows(second_at, band_h) {
                band(b, s, at(second_at, band_h), "links.picker.singles");
            }
            for (&(i, hover), &(target, rect)) in st.painted.iter().zip(&st.rects) {
                let hot = st.highlight == Some(target);
                paint_row(b, rect, s, st.row(i), hot, hover);
            }
        });
    });
    st.offset = out.state.offset.y;
    let content = out.content_size.y.max(1.0);
    let len = (out.inner_rect.height() / content).min(1.0);
    st.overflow = len < 1.0;
    if st.overflow {
        let mut start = out.state.offset.y / content;
        let id = Id::new("cantus-link-picker-scroll");
        if crate::kit::widgets::scrollbar(ui, lane, id, &mut start, len).dragged() {
            st.scroll_to = Some(start * content);
        }
    }
    chosen
}

/// A section band inside the list: the words in capitals on a slightly lifted ground.
fn band(b: &mut Brush, s: &Style, r: Rect, key: &str) {
    b.rect(
        r,
        Fill::Solid(shade(s.t.well, if s.t.dark { 0.04 } else { -0.03 })),
    );
    let t = Text {
        caps: true,
        ..Text::key(key, Role::Section, s.t.label)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End))
    };
    let _ = text::paint(b, r.shrink2(vec2(10.0 * s.k, 0.0)), &t, s.tk());
}

/// One row: the name and, at its right, the kind and the count; under them where the
/// first member sits, or why the row links nothing. Every string was made with the list.
fn paint_row(b: &mut Brush, rect: Rect, s: &Style, r: &PickRow, hot: bool, hover: bool) {
    let k = s.k;
    let links = r.refusal.is_none();
    if hot {
        b.rect(rect, Fill::Solid(mix(s.t.well, s.t.selection, 0.22)));
        let bar = Rect::from_min_size(rect.min, vec2(2.0 * k, rect.height()));
        b.rect(bar, Fill::Solid(s.t.selection));
    } else if hover && links {
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
    let a = if links { 1.0 } else { 0.45 };
    let body = rect.shrink2(vec2(10.0 * k, 4.0 * k));
    let top = Rect::from_min_max(body.min, pos2(body.right(), body.center().y));
    let bottom = Rect::from_min_max(pos2(body.left(), body.center().y), body.max);
    // The right of the first line: "Notes  ×3".
    let mut right = top.right();
    if !r.count.is_empty() {
        let t =
            Text::new(&r.count, Role::Value, with_alpha(s.t.text, a)).align(Align2::RIGHT_CENTER);
        let w = text::measure(b.ctx(), &t, s.tk()).x;
        let _ = text::paint(b, top, &t, s.tk());
        right -= w + 8.0 * k;
    }
    let kind = Text::key(
        kind_key(r.group.kind),
        Role::Label,
        with_alpha(s.t.text_dim, a),
    )
    .align(Align2::RIGHT_CENTER);
    let kw = text::measure(b.ctx(), &kind, s.tk()).x;
    let kr = Rect::from_min_max(pos2(right - kw, top.top()), pos2(right, top.bottom()));
    let _ = text::paint(b, kr, &kind, s.tk());
    right = kr.left() - 10.0 * k;
    let nt = Text::new(&r.name, Role::Body, with_alpha(s.t.text, a))
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let nr = Rect::from_min_max(top.min, pos2(right.max(top.left()), top.bottom()));
    let _ = text::paint(b, nr, &nt, s.tk());
    let under_color = with_alpha(s.t.text_dim, a.max(0.7));
    let ut = match r.refusal {
        Some(why) => Text::key(refusal_key(why), Role::Label, under_color),
        None => Text::new(&r.hint, Role::Label, under_color),
    }
    .align(Align2::LEFT_CENTER)
    .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, bottom, &ut, s.tk());
}

/// The hint bar names the row ("Untitled clip" for one with no name), and says where it
/// sits or why it does not link.
fn hover_hint(ctx: &Context, r: &PickRow) {
    let under = match r.refusal {
        Some(why) => t(refusal_key(why)),
        None => r.hint.clone(),
    };
    hint::set_text(ctx, &r.name, &under, Caps::NONE);
}

/// "Arrows choose · Enter links · Esc closes".
fn keys_line(ui: &mut Ui, s: &Style) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), KEYS * s.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let t = Text::key("links.picker.keys", Role::Label, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, r, &t, s.tk());
    });
}
