//! The clip browser drawn with the kit: the filter (the picker's one-well field), two bands,
//! the tiles in the clips' colours (title strip with the chain mark, name and ×N; the kind
//! and a miniature under it), only those in view laid out; and, while a tile is dragged,
//! its chip at the pointer and a ghost where the copy would land.

use super::{clip_and_colour, ClipBrowser, BAND, FIELD, PAD, TILE, TILE_GAP};
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::t;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use crate::links::{kind_key, PickRow};
use cantus_core::Clip;
use egui::{pos2, vec2, Align2, Color32, Context, Id, Pos2, Rect, Sense, Ui};

/// What a frame of the pane asked for.
#[derive(Default)]
pub(super) struct Picked {
    /// A tile clicked: it becomes the paint source (or stops being it).
    pub choose: Option<ClipRef>,
    /// A tile double-clicked: select its entry's clips.
    pub select: Option<ClipRef>,
}

pub(super) fn pane(st: &mut ClipBrowser, app: &CantusApp, ui: &mut Ui) -> Picked {
    let s = Style::current();
    let k = s.k;
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    with_brush(ui.painter(), |b| b.rect(full, Fill::Solid(s.t.panel)));
    let field = Rect::from_min_size(
        pos2(full.left() + PAD * k, full.top() + PAD * k),
        vec2(full.width() - 2.0 * PAD * k, s.d(FIELD)),
    );
    let id = Id::new("cantus-clip-browser-filter");
    let _ = super::super::link_picker::search_field(
        ui,
        &mut st.filter,
        id,
        "links.browser.filter",
        &s,
        field,
        false,
    );
    st.rebuild(app);
    let area = Rect::from_min_max(
        pos2(full.left() + PAD * k, field.bottom() + PAD * k),
        pos2(full.right() - PAD * k, full.bottom() - PAD * k),
    );
    let mut out = Picked::default();
    st.rects.clear();
    st.lane = None;
    if area.height() < TILE * k * 0.5 {
        return out;
    }
    if st.shown.is_empty() {
        let key = if st.filter.trim().is_empty() {
            "links.browser.empty"
        } else {
            "links.browser.no_match"
        };
        with_brush(ui.painter(), |b| {
            let t = Text::key(key, Role::Body, s.t.placeholder)
                .align(Align2::CENTER_TOP)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, area.shrink2(vec2(0.0, 8.0 * k)), &t, s.tk());
        });
        return out;
    }
    list(st, app, ui, &s, area, &mut out);
    out
}

/// The tiles, virtualised: tiles have one height and the two bands another, so where tile
/// `i` sits is arithmetic and only those in view are sensed and painted.
fn list(
    st: &mut ClipBrowser,
    app: &CantusApp,
    ui: &mut Ui,
    s: &Style,
    area: Rect,
    out: &mut Picked,
) {
    let k = s.k;
    let n = st.shown.len();
    let groups = {
        let rows = st.list.rows();
        st.shown.partition_point(|&i| rows[i].group.count > 1)
    };
    let (step, band_h) = ((TILE + TILE_GAP) * k, BAND * k);
    let (first_band, second_band) = (groups > 0, n > groups);
    let y_of = |i: usize| {
        first_band as u8 as f32 * band_h
            + i as f32 * step
            + (i >= groups && second_band) as u8 as f32 * band_h
    };
    let total = (first_band as u8 + second_band as u8) as f32 * band_h + n as f32 * step;
    // The kit's scrollbar in a lane at the right (ADR 0024 §10), shown when it overflows.
    let lane = Rect::from_min_max(pos2(area.right() - 5.0 * k, area.top()), area.max);
    let rows_rect = Rect::from_min_max(area.min, pos2(lane.left() - 3.0 * k, area.bottom()));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rows_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rows_rect.intersect(ui.clip_rect()));
    let dragging = st.drag;
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("cantus-clip-browser-list")
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    // Where a drag on the scrollbar took the list last frame.
    if let Some(y) = st.scroll_to.take() {
        scroll = scroll.vertical_scroll_offset(y);
    }
    let shown = scroll.show_viewport(&mut child, |ui, view| {
        ui.set_height(total);
        let (top, left, w) = (
            ui.max_rect().top(),
            ui.max_rect().left(),
            ui.max_rect().width(),
        );
        let visible = |y: f32, h: f32| y + h > view.min.y && y < view.max.y;
        let at = |y: f32, h: f32| Rect::from_min_size(pos2(left, top + y), vec2(w, h));
        let from = (((view.min.y - 2.0 * band_h) / step).floor().max(0.0) as usize).min(n);
        let to = ((view.max.y / step).ceil() as usize + 1).min(n);
        let mut hovered = None;
        let mut started = None;
        for i in from..to {
            let y = y_of(i);
            if !visible(y, TILE * k) {
                continue;
            }
            let rect = at(y, TILE * k);
            let r = st.row(i);
            let target = r.group.first;
            let id = Id::new(("cantus-clip-tile", target.0 .0, target.1 .0));
            let resp = ui.interact(rect, id, Sense::click_and_drag());
            if resp.drag_started() {
                started = Some(target);
            }
            if resp.clicked() {
                out.choose = Some(target);
            }
            if resp.double_clicked() {
                out.select = Some(target);
            }
            if resp.hovered() && dragging.is_none() {
                hovered = Some(target);
                // The paint source says so; any other tile, how to drag it.
                let chosen = st.brush.is_some_and(|b| super::same_entry(app, b, target));
                let tip = t(if chosen {
                    "links.browser.chosen"
                } else {
                    "links.browser.drag_tip"
                });
                hint::set_text(ui.ctx(), &r.name, &tip, Caps::NONE);
            }
            st.rects.push((target, rect));
        }
        if started.is_some() {
            st.drag = started;
        }
        // The waveforms of the audio tiles in view, made once per clip and revision.
        let ctx = ui.ctx().clone();
        let cols = |rect: Rect| ((rect.width() * 0.5) / (2.0 * k)).round().max(8.0) as usize;
        for i in 0..st.rects.len() {
            let (target, rect) = st.rects[i];
            let audio =
                crate::clips::clip_of(app.project(), target).is_some_and(|c| c.audio.is_some());
            if audio {
                let _ = st.wave(app, &ctx, target, cols(rect));
            }
        }
        let brush = st.brush;
        let second_at = y_of(groups) - band_h;
        let painter = ui.painter().clone();
        with_brush(&painter, |b| {
            if first_band && visible(0.0, band_h) {
                band(b, s, at(0.0, band_h), "links.picker.groups");
            }
            if second_band && visible(second_at, band_h) {
                band(b, s, at(second_at, band_h), "links.picker.singles");
            }
            for &(target, rect) in &st.rects {
                let Some(row) = st.list.rows().iter().find(|r| r.group.first == target) else {
                    continue;
                };
                let hot = hovered == Some(target) || dragging == Some(target);
                let chosen = brush.is_some_and(|b| super::same_entry(app, b, target));
                let wave = st.waves.get(&target).map(|(_, _, w)| w.as_slice());
                tile(b, rect, s, app, row, hot, chosen, wave);
            }
        });
    });
    let content = shown.content_size.y.max(1.0);
    let len = (shown.inner_rect.height() / content).min(1.0);
    st.lane = None;
    if len < 1.0 {
        st.lane = Some(lane);
        let mut start = shown.state.offset.y / content;
        let id = Id::new("cantus-clip-browser-scroll");
        if crate::kit::widgets::scrollbar(ui, lane, id, &mut start, len).dragged() {
            st.scroll_to = Some(start * content);
        }
    }
}

/// A section band over the tiles: the words in capitals.
fn band(b: &mut Brush, s: &Style, r: Rect, key: &str) {
    let t = Text {
        caps: true,
        ..Text::key(key, Role::Section, s.t.label)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End))
    };
    let _ = text::paint(b, r.shrink2(vec2(2.0 * s.k, 0.0)), &t, s.tk());
}

/// One tile, FL's picker tile: a title strip in the clip's colour with the chain mark (a
/// group), the name and ×N; under it, on the colour's darker ground, the kind and a
/// miniature of what it holds. The paint source (`chosen`) wears the selection's ring.
#[allow(clippy::too_many_arguments)]
fn tile(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    app: &CantusApp,
    r: &PickRow,
    hot: bool,
    chosen: bool,
    wave: Option<&[(f32, f32)]>,
) {
    let k = s.k;
    let Some((clip, colour)) = clip_and_colour(app, r.group.first) else {
        return;
    };
    let radius = s.r(3.0);
    let strip_h = 18.0 * k;
    let ground = mix(s.t.well, colour, if s.t.dark { 0.28 } else { 0.22 });
    let strip = mix(s.t.well, colour, if s.t.dark { 0.62 } else { 0.5 });
    b.rrect(rect, radius, Fill::Solid(ground));
    let strip_rect = Rect::from_min_size(rect.min, vec2(rect.width(), strip_h));
    b.rrect(strip_rect, radius, Fill::Solid(strip));
    b.rect(
        Rect::from_min_max(pos2(rect.left(), strip_rect.center().y), strip_rect.max),
        Fill::Solid(strip),
    );
    let (edge, width) = match (chosen, hot) {
        (true, _) => (s.t.selection, 2.0),
        (false, true) => (s.t.focus, 2.0),
        (false, false) => (shade(colour, -0.25), 1.0),
    };
    b.rrect_stroke(
        b.snap_rect(rect).shrink(b.px() * width * 0.5),
        radius,
        b.px() * width,
        edge,
    );
    // The title strip: ⛓ name … ×N.
    let mut left = strip_rect.left() + 6.0 * k;
    if r.group.count > 1 {
        let g = Rect::from_center_size(
            pos2(left + 5.0 * k, strip_rect.center().y),
            vec2(10.0 * k, 10.0 * k),
        );
        glyph::paint(b, g, Glyph::Link, s.t.text);
        left = g.right() + 4.0 * k;
    }
    let mut right = strip_rect.right() - 6.0 * k;
    if !r.count.is_empty() {
        let t = Text::new(&r.count, Role::Value, s.t.text).align(Align2::RIGHT_CENTER);
        let w = text::measure(b.ctx(), &t, s.tk()).x;
        let _ = text::paint(
            b,
            Rect::from_min_max(
                pos2(right - w, strip_rect.top()),
                pos2(right, strip_rect.bottom()),
            ),
            &t,
            s.tk(),
        );
        right -= w + 6.0 * k;
    }
    let nt = Text::new(&r.name, Role::Body, s.t.text)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let nr = Rect::from_min_max(
        pos2(left, strip_rect.top()),
        pos2(right.max(left), strip_rect.bottom()),
    );
    let _ = text::paint(b, nr, &nt, s.tk());
    // The body: the kind at the left, the miniature at the right.
    let body = Rect::from_min_max(
        pos2(rect.left() + 6.0 * k, strip_rect.bottom() + 3.0 * k),
        pos2(rect.right() - 6.0 * k, rect.bottom() - 4.0 * k),
    );
    let kind = Text::key(kind_key(r.group.kind), Role::Label, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let kw = text::measure(b.ctx(), &kind, s.tk())
        .x
        .min(body.width() * 0.45);
    let kr = Rect::from_min_size(body.min, vec2(kw + 1.0, body.height()));
    let _ = text::paint(b, kr, &kind, s.tk());
    let mini = Rect::from_min_max(pos2(kr.right() + 8.0 * k, body.top()), body.max);
    if mini.width() > 8.0 * k {
        let ink = mix(colour, s.t.text, 0.35);
        match wave {
            Some(w) => waveform(b, mini, s, w, ink),
            None => miniature(b, mini, s, clip, ink),
        }
    }
}

/// What the clip holds, small: its notes as dashes over its length (at most 128), or its
/// curve as a line; nothing for audio until its waveform is ready.
fn miniature(b: &mut Brush, r: Rect, s: &Style, clip: &Clip, ink: Color32) {
    let k = s.k;
    if let Some(a) = &clip.automation {
        let span = clip.content_length.unwrap_or(clip.length).0.max(1) as f32;
        let x = |t: cantus_core::Ticks| r.left() + (t.0 as f32 / span).clamp(0.0, 1.0) * r.width();
        let y = |v: f32| r.bottom() - v.clamp(0.0, 1.0) * r.height();
        let mut last: Option<Pos2> = None;
        for p in a.points.iter().take(256) {
            let q = pos2(x(p.tick), y(p.value));
            if let Some(l) = last {
                b.segment(l, q, 1.2 * k, ink, false);
            }
            last = Some(q);
        }
        if let Some(l) = last {
            b.segment(
                l,
                pos2(r.right(), l.y),
                1.2 * k,
                with_alpha(ink, 0.6),
                false,
            );
        }
        return;
    }
    if clip.audio.is_some() || clip.notes.is_empty() {
        return;
    }
    let notes = clip.notes.iter().take(128);
    let (lo, hi) = notes.clone().fold((u8::MAX, u8::MIN), |(lo, hi), n| {
        (lo.min(n.pitch), hi.max(n.pitch))
    });
    let end = notes
        .clone()
        .map(|n| (n.start + n.length).0)
        .max()
        .unwrap_or(1)
        .max(clip.content_length.unwrap_or(clip.length).0)
        .max(1) as f32;
    let rows = (hi.saturating_sub(lo) as f32 + 1.0).max(1.0);
    let h = (r.height() / rows).clamp(1.5 * k, 4.0 * k);
    for n in notes {
        let x0 = r.left() + (n.start.0 as f32 / end) * r.width();
        let x1 = r.left() + ((n.start + n.length).0 as f32 / end) * r.width();
        let yc = r.bottom() - ((n.pitch - lo) as f32 + 0.5) / rows * r.height();
        let nr = Rect::from_min_max(
            pos2(x0, yc - h * 0.5),
            pos2(x1.max(x0 + 1.5 * k), yc + h * 0.5),
        );
        b.rect(nr, Fill::Solid(ink));
    }
}

/// While a tile is dragged: a chip with its name at the pointer and, over the arrangement,
/// a ghost where the copy would land (or the chip dimmed where nothing would).
pub(super) fn drag_feedback(
    ctx: &Context,
    app: &CantusApp,
    clip: &Clip,
    at: Pos2,
    ghost: Option<Rect>,
) {
    let s = Style::current();
    let k = s.k;
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("cantus-clip-browser-drag"),
    ));
    let colour = app
        .project()
        .tracks
        .iter()
        .find(|t| t.clip(clip.id).is_some())
        .map(|t| crate::theme::track_color(clip.color.unwrap_or(t.color)))
        .unwrap_or(s.t.selection);
    with_brush(&painter, |b| {
        if let Some(g) = ghost {
            b.rrect(g, s.r(3.0), Fill::Solid(with_alpha(colour, 0.35)));
            b.rrect_stroke(g, s.r(3.0), 1.5 * k, s.t.focus);
        }
        let name = if clip.name.is_empty() {
            t("links.picker.unnamed")
        } else {
            clip.name.clone()
        };
        let label = Text::new(&name, Role::Body, s.t.text);
        let w = text::measure(b.ctx(), &label, s.tk()).x.min(220.0 * k) + 34.0 * k;
        let chip = Rect::from_min_size(at + vec2(12.0 * k, 10.0 * k), vec2(w, 22.0 * k));
        let a = if ghost.is_some() { 1.0 } else { 0.6 };
        b.rrect(
            chip,
            s.r(4.0),
            Fill::Solid(with_alpha(mix(s.t.panel_raised, colour, 0.35), a)),
        );
        b.rrect_stroke(chip, s.r(4.0), b.px(), with_alpha(colour, a));
        let g = Rect::from_center_size(
            pos2(chip.left() + 11.0 * k, chip.center().y),
            vec2(10.0 * k, 10.0 * k),
        );
        glyph::paint(b, g, Glyph::Link, with_alpha(s.t.text, a));
        let tr = Rect::from_min_max(
            pos2(g.right() + 5.0 * k, chip.top()),
            pos2(chip.right() - 6.0 * k, chip.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &label
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

/// An audio tile's waveform: its min/max columns spread over `r`, centred, each at least
/// a hairline.
fn waveform(b: &mut Brush, r: Rect, s: &Style, w: &[(f32, f32)], ink: Color32) {
    if w.is_empty() {
        return;
    }
    let k = s.k;
    let step = r.width() / w.len() as f32;
    let mid = r.center().y;
    let half = r.height() * 0.5;
    for (i, &(lo, hi)) in w.iter().enumerate() {
        let x = r.left() + (i as f32 + 0.5) * step;
        let y0 = mid - hi.clamp(-1.0, 1.0) * half;
        let y1 = mid - lo.clamp(-1.0, 1.0) * half;
        let col = Rect::from_min_max(
            pos2(x - (step * 0.35).max(0.5 * k), y0.min(mid - 0.5 * k)),
            pos2(x + (step * 0.35).max(0.5 * k), y1.max(mid + 0.5 * k)),
        );
        b.rect(col, Fill::Solid(ink));
    }
}
