//! The sample browser's painters: a row of the tree, its small icons, the section
//! labels, and the chip that follows the pointer while a sample is dragged.

use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill, Style};
use crate::theme::tint as theme_tint;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// What a row shows at its left.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Icon {
    /// An installed pack: a box.
    Pack,
    /// A folder (open or closed).
    Folder { open: bool },
    /// An audio file: a little waveform.
    Wave,
}

/// How a row looks this frame.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct RowLook {
    pub hovered: bool,
    pub selected: bool,
    pub playing: bool,
    /// A folder row: draws the disclosure chevron.
    pub disclosure: Option<bool>,
    /// Dims the name (a hint line, a path in search results).
    pub dim: bool,
}

/// Paint one row: the band, the chevron, the icon, the name, and a play mark.
#[allow(clippy::too_many_arguments)]
pub fn row(b: &mut Brush, rect: Rect, s: &Style, depth: usize, icon: Icon, name: &str, detail: &str, look: RowLook) {
    let k = s.k;
    let r = b.snap_rect(rect);
    if look.selected {
        b.rect(r, Fill::Solid(with_alpha(s.t.selection, 0.55)));
        b.rect(
            Rect::from_min_size(r.min, vec2(2.0 * k, r.height())),
            Fill::Solid(s.t.highlight),
        );
    } else if look.hovered {
        b.rect(r, Fill::Solid(theme_tint(s.t.panel, s.t.text, 0.06)));
    }
    let indent = 8.0 * k + depth as f32 * 14.0 * k;
    let cy = r.center().y;
    let mut x = r.left() + indent;
    if let Some(open) = look.disclosure {
        let g = if open { Glyph::Down } else { Glyph::Next };
        glyph::paint(b, Rect::from_center_size(pos2(x + 5.0 * k, cy), vec2(9.0, 9.0) * k), g, s.t.text_dim);
    }
    x += 13.0 * k;
    let ir = Rect::from_center_size(pos2(x + 7.0 * k, cy), vec2(14.0, 12.0) * k);
    let ink = if look.selected || look.playing { s.t.highlight } else { s.t.text_dim };
    paint_icon(b, ir, s, icon, ink);
    x += 20.0 * k;
    let right = if look.playing { r.right() - 20.0 * k } else { r.right() - 6.0 * k };
    let ink = if look.dim { s.t.text_dim } else { s.t.text };
    let role = if matches!(icon, Icon::Pack) { Role::Control } else { Role::Body };
    let name_rect = Rect::from_min_max(pos2(x, r.top()), pos2(right, r.bottom()));
    let painted = text::paint(
        b,
        name_rect,
        &Text::new(name, role, ink)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
    if !detail.is_empty() {
        let used = painted.map(|p| p.rect.right()).unwrap_or(x) + 8.0 * k;
        if used < right - 24.0 * k {
            let _ = text::paint(
                b,
                Rect::from_min_max(pos2(used, r.top()), pos2(right, r.bottom())),
                &Text::new(detail, Role::Micro, s.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
        }
    }
    if look.playing {
        let pr = Rect::from_center_size(pos2(r.right() - 11.0 * k, cy), vec2(9.0, 9.0) * k);
        glyph::paint(b, pr, Glyph::Play, s.t.highlight);
    }
}

/// The small icons, drawn: a pack box, a folder, a waveform.
pub fn paint_icon(b: &mut Brush, r: Rect, s: &Style, icon: Icon, ink: Color32) {
    let k = s.k;
    match icon {
        Icon::Wave => {
            // Five bars of a waveform, tallest in the middle.
            let hs = [0.35, 0.7, 1.0, 0.6, 0.3];
            let w = r.width() / hs.len() as f32;
            for (i, h) in hs.iter().enumerate() {
                let x = r.left() + (i as f32 + 0.5) * w;
                let half = r.height() * 0.5 * h;
                b.segment(pos2(x, r.center().y - half), pos2(x, r.center().y + half), 1.4 * k, ink, true);
            }
        }
        Icon::Folder { open } => {
            let body = Rect::from_min_max(pos2(r.left(), r.top() + 2.5 * k), r.max);
            let tab = Rect::from_min_size(r.min, vec2(r.width() * 0.45, 3.5 * k));
            let fill = theme_tint(s.t.panel, s.hue, if open { 0.55 } else { 0.4 });
            b.rrect(tab, 1.0 * k, Fill::Solid(fill));
            b.rrect(body, 1.5 * k, Fill::Solid(fill));
            b.rrect_stroke(body, 1.5 * k, b.px(), shade(fill, -0.25));
            if open {
                b.hline(body.left() + 1.5 * k, body.right() - 1.5 * k, body.top() + 3.0 * k, b.px(), shade(fill, 0.25));
            }
        }
        Icon::Pack => {
            let fill = theme_tint(s.t.panel, s.t.brand, 0.5);
            b.rrect(r, 2.0 * k, Fill::Solid(fill));
            b.rrect_stroke(r, 2.0 * k, b.px(), shade(fill, -0.3));
            // The lid's seam and a label.
            b.hline(r.left(), r.right(), r.top() + r.height() * 0.38, b.px(), shade(fill, -0.3));
            let label = Rect::from_center_size(pos2(r.center().x, r.top() + r.height() * 0.66), vec2(r.width() * 0.4, 2.2 * k));
            b.rect(label, Fill::Solid(shade(fill, 0.35)));
        }
    }
}

/// A small-caps section label across the tree ("PACKS", "MY FOLDERS").
pub fn section(b: &mut Brush, rect: Rect, s: &Style, key: &str) {
    let r = b.snap_rect(rect);
    let _ = text::paint(
        b,
        r.shrink2(vec2(10.0 * s.k, 0.0)),
        &Text::key(key, Role::Micro, s.t.label)
            .align(Align2::LEFT_BOTTOM)
            .caps(true),
        s.tk(),
    );
}

/// The chip that follows the pointer while a sample is dragged: its waveform mark and
/// name on a raised pill, offset so it never hides what it is aimed at.
pub fn drag_chip(b: &mut Brush, at: Pos2, s: &Style, name: &str, over: bool) {
    let k = s.k;
    let t = Text::new(name, Role::Control, s.t.text).policy(Policy::Elide(Elide::End));
    let w = (text::measure(&b.ctx().clone(), &t, s.tk()).x + 36.0 * k).min(260.0 * k);
    let r = Rect::from_min_size(at + vec2(14.0, 10.0) * k, vec2(w, 24.0 * k));
    b.rrect_soft(r.translate(vec2(0.0, 3.0 * k)), 6.0 * k, Fill::Solid(with_alpha(s.t.shadow, 0.6)), 10.0 * k);
    let base = if over { s.t.highlight } else { s.t.panel_raised };
    b.rrect(r, 4.0 * k, Fill::Solid(base));
    b.rrect_stroke(r, 4.0 * k, b.px(), shade(base, -0.25));
    let ink: Color32 = if over { s.t.highlight_glyph } else { s.t.text };
    let ir = Rect::from_center_size(pos2(r.left() + 13.0 * k, r.center().y), vec2(12.0, 10.0) * k);
    paint_icon(b, ir, s, Icon::Wave, ink);
    let _ = text::paint(
        b,
        Rect::from_min_max(pos2(r.left() + 24.0 * k, r.top()), pos2(r.right() - 8.0 * k, r.bottom())),
        &Text::new(name, Role::Control, ink).align(Align2::LEFT_CENTER).policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
}

/// The outline a target wears while a sample hovers over it.
pub fn target_ring(b: &mut Brush, rect: Rect, s: &Style) {
    b.rrect(rect, 3.0 * s.k, Fill::Solid(with_alpha(s.t.highlight, 0.10)));
    b.rrect_stroke(rect.shrink(1.0), 3.0 * s.k, 2.0 * s.k, s.t.highlight);
}

/// Text in a rect, for the foot and the hints.
pub fn line(b: &mut Brush, rect: Rect, s: &Style, src: Src<'_>, role: Role, color: Color32, align: Align2) {
    let _ = text::paint(
        b,
        rect,
        &Text { src, role, caps: false, align, policy: Policy::Elide(Elide::End), color },
        s.tk(),
    );
}
