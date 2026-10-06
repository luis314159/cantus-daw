//! The picker's gallery (ADR 0029 §2, §3): tiles in rows under their shelf's heading,
//! laid out by a pure function ([`lay_out`]) and drawn in three passes so a screenful of
//! tiles costs a handful of draw calls whatever it holds: the ground, headings and drawn
//! covers through one brush; every picture of the native devices as **one mesh** on the
//! thumbnails' atlas; then names, stars and the selection through a second brush. Only
//! the tiles inside the viewport are drawn or sensed.

use super::picker::PickerView;
use super::picker_keys::Action;
use super::picker_snap::{Subject, ASPECT};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{color::with_alpha, glyph::Glyph, Brush, Fill, Style, Tok};
use crate::picker::PickChoice;
use crate::picker_catalog::{Category, Device};
use crate::picker_query::{Group, Shelf};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui, Vec2};

/// What the picker laid out this frame, in screen points: what tests click and what the
/// keyboard moves over.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub panel: Rect,
    pub search: Rect,
    pub close: Rect,
    pub choose: Rect,
    pub cancel: Rect,
    /// The details pane's star.
    pub star: Rect,
    pub sort: Rect,
    pub size: Rect,
    pub details_toggle: Rect,
    pub shelves: Vec<(Shelf, Rect)>,
    /// Every tile in display order: its device's key and its rect in content coordinates
    /// (from the gallery's top, unscrolled).
    pub tiles: Vec<(String, Rect)>,
    /// The tiles drawn this frame, on screen.
    pub shown: Vec<(String, Rect)>,
    /// The gallery's viewport on screen.
    pub grid: Rect,
    pub content_h: f32,
    /// A screenful, for Page Up / Page Down.
    pub page: f32,
}

impl Default for Geometry {
    fn default() -> Self {
        let r = Rect::NOTHING;
        Geometry {
            panel: r,
            search: r,
            close: r,
            choose: r,
            cancel: r,
            star: r,
            sort: r,
            size: r,
            details_toggle: r,
            shelves: Vec::new(),
            tiles: Vec::new(),
            shown: Vec::new(),
            grid: r,
            content_h: 0.0,
            page: 0.0,
        }
    }
}

/// One laid-out element of the gallery, in content coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Heading(Category, usize, Rect),
    /// A catalogue index and its tile.
    Tile(usize, Rect),
}

pub const PAD: f32 = 16.0;
pub const GAP: f32 = 14.0;
pub const HEADING: f32 = 30.0;
/// The words under a picture: the name and what it is.
pub const CAPTION: f32 = 42.0;

/// Lay `groups` out in rows across `width` points, tiles about `tile_w` wide (stretched to
/// fill the row). Returns the items and the content's height.
pub fn lay_out(groups: &[Group], width: f32, tile_w: f32, k: f32) -> (Vec<Item>, f32) {
    let (pad, gap) = (PAD * k, GAP * k);
    let inner = (width - 2.0 * pad).max(tile_w * k);
    let cols = (((inner + gap) / (tile_w * k + gap)).floor() as usize).max(1);
    let tw = (inner - gap * (cols - 1) as f32) / cols as f32;
    let th = tw / ASPECT + CAPTION * k;
    let mut out = Vec::new();
    let mut y = pad * 0.5;
    for g in groups {
        if let Some(c) = g.title {
            out.push(Item::Heading(
                c,
                g.items.len(),
                Rect::from_min_size(pos2(pad, y), vec2(inner, HEADING * k)),
            ));
            y += HEADING * k + 2.0 * k;
        }
        for (n, &i) in g.items.iter().enumerate() {
            let (col, row) = (n % cols, n / cols);
            let r = Rect::from_min_size(
                pos2(pad + col as f32 * (tw + gap), y + row as f32 * (th + gap)),
                vec2(tw, th),
            );
            out.push(Item::Tile(i, r));
        }
        let rows = g.items.len().div_ceil(cols);
        y += rows as f32 * (th + gap) + if g.title.is_some() { 6.0 * k } else { 0.0 };
    }
    (out, y + pad * 0.5)
}

/// The picture of a native device with an editor, if it has one.
pub fn subject(d: &Device) -> Option<Subject> {
    match d.choice {
        PickChoice::Instrument(k) => Some(Subject::Instrument(k)),
        PickChoice::Pulsus => Some(Subject::Pulsus),
        PickChoice::Effect(k) => Subject::of_effect(k),
        _ => None,
    }
}

/// The picture's rect inside a tile.
pub fn picture(tile: Rect) -> Rect {
    Rect::from_min_size(tile.min, vec2(tile.width(), tile.width() / ASPECT))
}

/// Draw the gallery in `view` and sense it. Returns what a double-click asked.
pub fn show(v: &mut PickerView, ui: &mut Ui, view: Rect, s: &Style) -> Action {
    let groups = crate::picker_query::groups(&v.catalog, &v.query, &v.prefs);
    let bar = s.s(10.0);
    let width = view.width() - bar;
    let (items, content_h) = lay_out(&groups, width, v.prefs.tile.width(), s.k);
    v.geo.grid = view;
    v.geo.content_h = content_h;
    v.geo.page = view.height() * 0.8;
    v.geo.tiles = items
        .iter()
        .filter_map(|it| match it {
            Item::Tile(i, r) => Some((v.catalog[*i].key.clone(), *r)),
            _ => None,
        })
        .collect();
    // Scrolling: the wheel, the bar, and the selection brought into view.
    let max = (content_h - view.height()).max(0.0);
    let hovered = ui.rect_contains_pointer(view);
    if hovered {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        v.scroll -= dy;
    }
    if std::mem::take(&mut v.reveal) {
        if let Some((_, r)) = v.geo.tiles.iter().find(|t| Some(&t.0) == v.selected.as_ref()) {
            let margin = s.s(PAD);
            // The tops of the rows and headings: a scroll that starts on one shows whole
            // tiles at the top of the gallery, never a cut row.
            let tops: Vec<f32> = items
                .iter()
                .map(|it| match it {
                    Item::Heading(_, _, h) | Item::Tile(_, h) => h.top() - margin * 0.5,
                })
                .collect();
            if r.top() - margin < v.scroll {
                // Upward: start at the selection's row, or at its heading just above.
                let want = r.top() - margin * 0.5;
                v.scroll = tops
                    .iter()
                    .copied()
                    .filter(|&t| t <= want && t >= want - s.s(HEADING) - margin)
                    .fold(want, f32::min);
            } else if r.bottom() + margin > v.scroll + view.height() {
                // Downward: the first row start that still shows the selection whole.
                let need = r.bottom() + margin - view.height();
                v.scroll = tops
                    .iter()
                    .copied()
                    .filter(|&t| t >= need)
                    .fold(f32::MAX, f32::min)
                    .min(r.top() - margin * 0.5);
            }
        }
    }
    v.scroll = v.scroll.clamp(0.0, max);
    let scroll = v.scroll;
    let at = move |r: Rect| r.translate(view.min.to_vec2() - vec2(0.0, scroll));
    let painter = ui.painter().with_clip_rect(view);
    let mut action = Action::None;
    let mut hovered_key: Option<String> = None;
    let mut star_click: Option<String> = None;
    v.geo.shown.clear();
    // Sense first, so the passes below know what is hovered.
    for it in &items {
        let Item::Tile(i, r) = it else { continue };
        let r = at(*r);
        if !r.intersects(view) {
            continue;
        }
        let key = v.catalog[*i].key.clone();
        let resp = ui.interact(r.intersect(view), Id::new(("picker-tile", &key)), Sense::click());
        let star = star_rect(r, s);
        let star_resp = ui.interact(star.intersect(view), Id::new(("picker-star", &key)), Sense::click());
        if resp.hovered() || star_resp.hovered() {
            hovered_key = Some(key.clone());
            let d = &v.catalog[*i];
            crate::kit::hint::set_text(
                ui.ctx(),
                &d.name,
                &crate::i18n::tf("picker.hint.tile", &[("name", &d.name)]),
                crate::kit::hint::Caps::NONE,
            );
        }
        if star_resp.clicked() {
            star_click = Some(key.clone());
        } else if resp.double_clicked() {
            v.selected = Some(key.clone());
            action = Action::Choose(key.clone());
        } else if resp.clicked() {
            v.selected = Some(key.clone());
            v.editing = false;
        }
        v.geo.shown.push((key, r));
    }
    if let Some(k) = star_click {
        v.toggle_favourite(&k);
    }
    // Pass 1: the ground, the headings, hover and selection faces, drawn covers.
    crate::kit::with_brush(&painter, |b| {
        b.register(view, "picker-grid", &[Tok::Ink], &[]);
        b.rect(view, Fill::Solid(s.t.ink));
        for it in &items {
            match it {
                Item::Heading(c, n, r) => {
                    let r = at(*r);
                    if r.intersects(view) {
                        heading(b, r, *c, *n, s);
                    }
                }
                Item::Tile(i, r) => {
                    let r = at(*r);
                    if !r.intersects(view) {
                        continue;
                    }
                    let d = &v.catalog[*i];
                    let sel = v.selected.as_deref() == Some(d.key.as_str());
                    let hov = hovered_key.as_deref() == Some(d.key.as_str());
                    face(b, r, s, sel, hov);
                    let pic = picture(r);
                    let has = subject(d).and_then(|x| v.thumbs.get(x)).is_some();
                    if !has {
                        super::picker_cover::card(b, pic, s.r(6.0), s, d);
                    } else {
                        b.rrect(pic, s.r(6.0), Fill::Solid(s.t.well));
                    }
                }
            }
        }
    });
    // Pass 2: every picture as one mesh on the atlas.
    let mut mesh = egui::Mesh::default();
    for it in &items {
        let Item::Tile(i, r) = it else { continue };
        let r = at(*r);
        if !r.intersects(view) {
            continue;
        }
        let d = &v.catalog[*i];
        if let Some((tex, uv)) = subject(d).and_then(|x| v.thumbs.get(x)) {
            mesh.texture_id = tex;
            let hov = hovered_key.as_deref() == Some(d.key.as_str());
            // The picture as it is; a touch dimmer at rest, so hovering lights it.
            let tint = egui::Color32::WHITE.gamma_multiply(if hov { 1.0 } else { 0.93 });
            mesh.add_rect_with_uv(picture(r), uv, tint);
        }
    }
    if !mesh.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
    // Pass 3: the words, the stars, the badges, the selection ring.
    crate::kit::with_brush(&painter, |b| {
        for it in &items {
            let Item::Tile(i, r) = it else { continue };
            let r = at(*r);
            if !r.intersects(view) {
                continue;
            }
            let d = &v.catalog[*i];
            let sel = v.selected.as_deref() == Some(d.key.as_str());
            let hov = hovered_key.as_deref() == Some(d.key.as_str());
            let fav = v.prefs.is_favourite(&d.key);
            let current = v.current.as_deref() == Some(d.key.as_str());
            overlay(b, r, s, d, sel, hov, fav, current);
        }
    });
    if v.geo.tiles.is_empty() {
        empty(v, ui, view, s);
    }
    // The scrollbar, always visible when there is more than a screenful.
    if max > 0.0 {
        let bar_rect = Rect::from_min_max(
            pos2(view.right() - bar + s.s(2.0), view.top() + s.s(4.0)),
            pos2(view.right() - s.s(2.0), view.bottom() - s.s(4.0)),
        );
        let mut start = v.scroll / content_h;
        let len = view.height() / content_h;
        crate::kit::widgets::scrollbar(ui, bar_rect, Id::new("picker-scroll"), &mut start, len);
        v.scroll = (start * content_h).clamp(0.0, max);
    }
    action
}

/// Where a tile's star sits: the picture's top-right corner.
pub fn star_rect(tile: Rect, s: &Style) -> Rect {
    let z = s.s(22.0);
    Rect::from_min_size(pos2(tile.right() - z - s.s(5.0), tile.top() + s.s(5.0)), Vec2::splat(z))
}

/// A shelf's heading: its name, its count, a hairline to the edge.
fn heading(b: &mut Brush, r: Rect, c: Category, n: usize, s: &Style) {
    let name = crate::i18n::t(c.key());
    let t = Text::new(&name, Role::Section, s.t.label).align(Align2::LEFT_CENTER);
    let w = text::measure(b.ctx(), &t, s.tk()).x;
    let gr = Rect::from_center_size(pos2(r.left() + s.s(7.0), r.center().y), Vec2::splat(s.s(14.0)));
    super::picker_cover::glyph(b, gr, c, s.t.hue(c.hue()), s.s(1.4));
    let tr = Rect::from_min_max(pos2(r.left() + s.s(20.0), r.top()), r.max);
    let _ = text::paint(b, tr, &t, s.tk());
    let count = n.to_string();
    let ct = Text::new(&count, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER);
    let cr = Rect::from_min_max(pos2(tr.left() + w + s.s(8.0), r.top()), r.max);
    let _ = text::paint(b, cr, &ct, s.tk());
    let x0 = cr.left() + s.s(26.0);
    b.hline(x0, r.right(), b.snap_mid(r.center().y), b.px(), with_alpha(s.t.edge, 0.35));
}

/// A tile's face: nothing at rest (the picture stands on the ground), a raised card on
/// hover, the selection's card when selected.
fn face(b: &mut Brush, r: Rect, s: &Style, sel: bool, hov: bool) {
    if !sel && !hov {
        return;
    }
    let card = r.expand(s.s(6.0));
    // Dark: the strip's grey, a breath of the selection when selected. Light: the raised
    // tile's white, which the selection ring alone marks (a green tint turns khaki there).
    let base = match (s.t.dark, sel) {
        (true, true) => crate::kit::color::mix(s.t.strip, s.t.selection, 0.16),
        (true, false) => s.t.strip,
        (false, _) => s.t.panel_raised,
    };
    crate::kit::light::raised(
        b,
        card,
        s.r(9.0),
        base,
        crate::kit::light::RaisedOpts {
            drop: true,
            k: s.k,
            alpha: 1.0,
            depth: s.t.depth,
            ..Default::default()
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn overlay(b: &mut Brush, r: Rect, s: &Style, d: &Device, sel: bool, hov: bool, fav: bool, current: bool) {
    let pic = picture(r);
    b.rrect_stroke(pic, s.r(6.0), b.px(), with_alpha(s.t.dark_shade, 0.5));
    // The name, and what it is.
    let name_r = Rect::from_min_size(pos2(r.left() + s.s(2.0), pic.bottom() + s.s(6.0)), vec2(r.width() - s.s(4.0), s.s(17.0)));
    let what_r = name_r.translate(vec2(0.0, s.s(16.0)));
    let ink = if sel { s.t.text } else { crate::kit::color::mix(s.t.text, s.t.text_dim, 0.15) };
    let nt = Text::new(&d.name, Role::Title, ink).align(Align2::LEFT_CENTER).policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, name_r, &nt, s.tk());
    let what = match d.source.badge() {
        Some(_) => format!("{} · {}", d.vendor, d.what),
        None => d.what.clone(),
    };
    let wt = Text::new(&what, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER).policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, what_r, &wt, s.tk());
    // The star: always when set, on hover otherwise.
    if fav || hov {
        let z = star_rect(r, s);
        b.circle(z.center(), z.width() * 0.5, Fill::Solid(with_alpha(s.t.ink, 0.62)));
        let (g, c) = if fav { (Glyph::StarFilled, s.t.brand_bright) } else { (Glyph::Star, s.t.text_dim) };
        crate::kit::glyph::paint(b, z.shrink(s.s(4.0)), g, c);
    }
    if current {
        let label = crate::i18n::t("picker.current");
        let t = Text::new(&label, Role::Micro, s.t.highlight_glyph).caps(true);
        let w = text::measure(b.ctx(), &t, s.tk()).x + s.s(10.0);
        let cr = Rect::from_min_size(pic.left_top() + vec2(s.s(6.0), s.s(6.0)), vec2(w, s.s(16.0)));
        b.rrect(cr, s.r(3.0), Fill::Solid(s.t.highlight));
        let _ = text::paint(b, cr, &t, s.tk());
    }
    if sel {
        let ring = r.expand(s.s(6.0));
        b.rrect_stroke(ring, s.r(9.0), s.s(2.0), s.t.selection);
    }
}

/// What the gallery says when it shows nothing: why, and what to do.
fn empty(v: &PickerView, ui: &Ui, view: Rect, s: &Style) {
    use crate::i18n::{t, tf};
    let (line, hint) = if !v.query.text.trim().is_empty() {
        (tf("picker.empty", &[("q", &v.query.text.trim())]), t("picker.empty.hint"))
    } else {
        match v.query.shelf {
            Shelf::Favourites => (t("picker.empty.favourites"), String::new()),
            Shelf::Recent => (t("picker.empty.recent"), String::new()),
            _ => (t("picker.empty.shelf"), String::new()),
        }
    };
    let c = view.center() - vec2(0.0, view.height() * 0.12);
    crate::kit::with_brush(ui.painter(), |b| {
        let g = Rect::from_center_size(c - vec2(0.0, s.s(40.0)), Vec2::splat(s.s(36.0)));
        super::picker_layout::magnifier(b, g, with_alpha(s.t.text_dim, 0.6), s.s(2.4));
        let r = Rect::from_center_size(c, vec2(view.width() - s.s(80.0), s.s(24.0)));
        let _ = text::paint(b, r, &Text::new(&line, Role::Title, s.t.text).policy(Policy::Elide(Elide::End)), s.tk());
        if !hint.is_empty() {
            let r = r.translate(vec2(0.0, s.s(24.0)));
            let _ = text::paint(b, r, &Text::new(&hint, Role::Body, s.t.text_dim).policy(Policy::Elide(Elide::End)), s.tk());
        }
    });
}
