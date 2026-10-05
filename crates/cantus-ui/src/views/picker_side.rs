//! The picker's shelves (ADR 0029 §2): the left column — the user's own (all,
//! favourites, recent), the categories of what is being chosen, and where devices come
//! from — each with its count, Ableton's sidebar with Bitwig's dim counts.

use super::picker::PickerView;
use crate::i18n::t;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style, Tok};
use crate::picker_query::{count, shelves, Shelf};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

const ROW: f32 = 27.0;
const BAND: f32 = 26.0;

/// The heading a shelf sits under.
fn band_of(shelf: Shelf) -> &'static str {
    match shelf {
        Shelf::All | Shelf::Favourites | Shelf::Recent => "picker.shelf.library",
        Shelf::Category(_) => "picker.shelf.categories",
        Shelf::Source(_) => "picker.shelf.sources",
    }
}

/// Draw the column in `r` and act on its clicks.
pub fn show(v: &mut PickerView, ui: &mut Ui, r: Rect, s: &Style) {
    let list = shelves(&v.catalog, v.query.role);
    let pad = s.s(10.0);
    let mut y = r.top() + s.s(6.0);
    let mut rows: Vec<(Shelf, Rect, Option<&'static str>)> = Vec::new();
    let mut last_band = "";
    for shelf in list {
        let band = band_of(shelf);
        let heading = if band != last_band {
            last_band = band;
            y += if rows.is_empty() { 0.0 } else { s.s(6.0) };
            let h = Some(band);
            y += s.s(BAND);
            h
        } else {
            None
        };
        let row = Rect::from_min_size(pos2(r.left() + pad * 0.6, y), vec2(r.width() - pad * 1.2, s.s(ROW)));
        rows.push((shelf, row, heading));
        y += s.s(ROW);
    }
    v.geo.shelves = rows.iter().map(|(sh, rr, _)| (*sh, *rr)).collect();
    let mut clicked = None;
    let mut hovered = None;
    for (shelf, row, _) in &rows {
        if row.bottom() > r.bottom() {
            break;
        }
        let resp = ui.interact(*row, Id::new(("picker-shelf", format!("{shelf:?}"))), Sense::click());
        if resp.clicked() {
            clicked = Some(*shelf);
        }
        if resp.hovered() {
            hovered = Some(*shelf);
        }
    }
    if let Some(sh) = clicked {
        v.set_shelf(sh);
    }
    let painter = ui.painter().with_clip_rect(r);
    with_brush(&painter, |b| {
        b.register(r, "picker-shelves", &[Tok::Strip, Tok::Text, Tok::TextDim, Tok::Selection], &[]);
        b.rect(r, Fill::Solid(s.t.strip));
        for (shelf, row, heading) in &rows {
            if let Some(key) = heading {
                let hr = Rect::from_min_size(pos2(row.left() + s.s(6.0), row.top() - s.s(BAND)), vec2(row.width(), s.s(BAND)));
                let label = t(key);
                let tt = Text::new(&label, Role::Section, s.t.label).align(Align2::LEFT_BOTTOM).policy(Policy::Elide(Elide::End));
                let _ = text::paint(b, hr.shrink2(vec2(0.0, s.s(4.0))), &tt, s.tk());
            }
            let n = count(&v.catalog, v.query.role, *shelf, &v.prefs);
            let on = v.query.shelf == *shelf;
            row_paint(b, *row, s, *shelf, n, on, hovered == Some(*shelf));
        }
    });
}

fn row_paint(b: &mut Brush, row: Rect, s: &Style, shelf: Shelf, n: usize, on: bool, hov: bool) {
    let hue = match shelf {
        Shelf::Category(c) => s.t.hue(c.hue()),
        Shelf::Favourites => s.t.brand_bright,
        _ => s.t.selection,
    };
    if on {
        b.rrect(row, s.r(4.0), Fill::Solid(mix(s.t.strip, s.t.selection, if s.t.dark { 0.2 } else { 0.24 })));
        let bar = Rect::from_min_size(row.min + vec2(0.0, s.s(5.0)), vec2(s.s(3.0), row.height() - s.s(10.0)));
        b.rrect(bar, s.r(1.5), Fill::Solid(s.t.selection));
    } else if hov {
        b.rrect(row, s.r(4.0), Fill::Solid(shade(s.t.strip, if s.t.dark { 0.04 } else { -0.03 })));
    }
    let g = Rect::from_center_size(pos2(row.left() + s.s(17.0), row.center().y), egui::Vec2::splat(s.s(14.0)));
    let gc = if on { hue } else { with_alpha(hue, 0.85) };
    match shelf {
        Shelf::All => glyph::paint(b, g, Glyph::Browser, gc),
        Shelf::Favourites => glyph::paint(b, g, Glyph::StarFilled, gc),
        Shelf::Recent => clock(b, g, gc, s.s(1.4)),
        Shelf::Category(c) => super::picker_cover::glyph(b, g, c, gc, s.s(1.4)),
        Shelf::Source(_) => glyph::paint(b, g, Glyph::Device, gc),
    }
    let name = t(shelf.key());
    let ink = if on { s.t.text } else if n == 0 { s.t.placeholder } else { mix(s.t.text, s.t.text_dim, 0.25) };
    let nr = Rect::from_min_max(pos2(row.left() + s.s(32.0), row.top()), pos2(row.right() - s.s(34.0), row.bottom()));
    let _ = text::paint(b, nr, &Text::new(&name, Role::Body, ink).align(Align2::LEFT_CENTER).policy(Policy::Elide(Elide::End)), s.tk());
    let cr = Rect::from_min_max(pos2(row.right() - s.s(34.0), row.top()), pos2(row.right() - s.s(8.0), row.bottom()));
    let count = n.to_string();
    let _ = text::paint(b, cr, &Text::new(&count, Role::Label, s.t.text_dim).align(Align2::RIGHT_CENTER), s.tk());
}

/// A clock face: the recent shelf.
fn clock(b: &mut Brush, r: Rect, c: egui::Color32, w: f32) {
    let ctr = r.center();
    let rad = r.width() * 0.45;
    b.circle_stroke(ctr, rad, w, c);
    b.segment(ctr, ctr + vec2(0.0, -rad * 0.62), w, c, true);
    b.segment(ctr, ctr + vec2(rad * 0.48, rad * 0.2), w, c, true);
}
