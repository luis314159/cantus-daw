//! Prose in a box (ADR 0024 §3, for the few texts longer than a label: the verse at
//! start). [`text`](super::text) paints one line and shortens what does not fit; prose is
//! broken into lines at spaces, each measured with the role's own face, and painted one
//! line per box with [`text::paint`](super::text::paint) — so it keeps the kit's rules:
//! measured before it is painted, never outside its box.

use super::brush::Brush;
use super::text::{self, Elide, Policy, Role, Text};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, Context, Id, Pos2, Rect, Vec2};
use std::sync::Arc;

/// A paragraph broken into lines at a width.
#[derive(Clone, Debug, PartialEq)]
pub struct Lines {
    pub lines: Vec<String>,
    /// The distance between baselines, in points.
    pub line_h: f32,
    /// The widest line's width, in points.
    pub widest: f32,
}

impl Lines {
    /// The block's height: every line at its pitch.
    pub fn height(&self) -> f32 {
        self.lines.len() as f32 * self.line_h
    }
}

/// Leading: the pitch of the lines over the face's own line height.
const LEADING: f32 = 1.12;

/// Break `s` into lines no wider than `width` points, in `role` at scale `k`. A word wider
/// than the box keeps a line of its own (and is elided when painted). Cached per text,
/// role, scale and width.
pub fn wrap(ctx: &Context, s: &str, role: Role, k: f32, width: f32) -> Arc<Lines> {
    let id = Id::new(("cantus-prose", s, role, k.to_bits(), width.to_bits()))
        .with(ctx.pixels_per_point().to_bits());
    if let Some(l) = ctx.data(|d| d.get_temp::<Arc<Lines>>(id)) {
        return l;
    }
    let lines = Arc::new(wrap_uncached(ctx, s, role, k, width));
    ctx.data_mut(|d| d.insert_temp(id, lines.clone()));
    lines
}

fn wrap_uncached(ctx: &Context, s: &str, role: Role, k: f32, width: f32) -> Lines {
    let mut job = LayoutJob::default();
    job.append(
        s,
        0.0,
        TextFormat {
            font_id: role.font(k),
            extra_letter_spacing: role.tracking() * k,
            color: Color32::PLACEHOLDER,
            valign: Align::BOTTOM,
            ..Default::default()
        },
    );
    let galley = ctx.fonts_mut(|f| f.layout_job(job));
    let line_h = galley.size().y * LEADING;
    let chars: Vec<char> = s.chars().collect();
    let Some(row) = galley.rows.first() else {
        return Lines {
            lines: Vec::new(),
            line_h,
            widest: 0.0,
        };
    };
    let glyphs = &row.row.glyphs;
    if glyphs.len() != chars.len() {
        // Not one glyph per char (should not happen without newlines): one line.
        return Lines {
            lines: vec![s.to_string()],
            line_h,
            widest: galley.size().x,
        };
    }
    let left = |i: usize| glyphs[i].pos.x;
    let right = |i: usize| glyphs[i].pos.x + glyphs[i].advance_width;
    let mut lines = Vec::new();
    let mut widest = 0.0f32;
    let mut start = 0usize;
    while start < chars.len() {
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
        if start >= chars.len() {
            break;
        }
        let x0 = left(start);
        let mut end = start;
        let mut last_break = None;
        while end < chars.len() {
            if chars[end] == ' ' {
                last_break = Some(end);
            } else if right(end) - x0 > width + 0.01 {
                break;
            }
            end += 1;
        }
        let stop = if end >= chars.len() {
            chars.len()
        } else {
            match last_break {
                Some(b) if b > start => b,
                // One word wider than the box: up to the next space.
                _ => (end..chars.len())
                    .find(|i| chars[*i] == ' ')
                    .unwrap_or(chars.len()),
            }
        };
        let line: String = chars[start..stop].iter().collect::<String>().trim_end().to_string();
        let mut last = stop;
        while last > start && chars[last - 1] == ' ' {
            last -= 1;
        }
        if last > start {
            widest = widest.max(right(last - 1) - x0);
        }
        lines.push(line);
        start = stop;
    }
    Lines {
        lines,
        line_h,
        widest,
    }
}

/// Paint `lines` from the top of `rect`, each in its own row aligned by `align`'s x.
/// Returns the rect the block occupies and whether any line had to be shortened.
pub fn paint(
    b: &mut Brush,
    rect: Rect,
    lines: &Lines,
    role: Role,
    k: f32,
    color: Color32,
    align: Align2,
) -> (Rect, bool) {
    let mut used = Rect::NOTHING;
    let mut shortened = false;
    for (i, line) in lines.lines.iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + i as f32 * lines.line_h),
            Vec2::new(rect.width(), lines.line_h),
        );
        let a = Align2([align.x(), Align::Center]);
        if let Some(p) = text::paint(
            b,
            row,
            &Text::new(line, role, color)
                .caps(false)
                .align(a)
                .policy(Policy::Elide(Elide::End)),
            k,
        ) {
            used = used.union(p.rect);
            shortened |= p.shortened;
        } else {
            shortened = true;
        }
    }
    (used, shortened)
}
