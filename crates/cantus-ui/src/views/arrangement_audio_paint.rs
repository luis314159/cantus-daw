//! The parts of an audio clip's drawing over its waveform: fades, handles, the gain line,
//! a missing file, and the fade-curve icon the menus and the inspector use.

use super::super::paint::ClipLook;
use super::{gain_y, missing_ground, Handle, Handles, Scratch, Shown};
use crate::audio_clips::Fade;
use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::{AudioRegion, FadeCurve};
use egui::{pos2, Align2, Color32, Pos2, Rect};

/// How many points a fade's curve is drawn with, at most.
const CURVE_POINTS: usize = 48;

/// Both fades over the waveform: the part a fade takes away shaded, and its curve.
#[allow(clippy::too_many_arguments)]
pub(super) fn fades(
    b: &mut Brush,
    shown: &Shown<'_>,
    body: Rect,
    inner: Rect,
    px_per_tick: f32,
    ink: Color32,
    scratch: &mut Scratch,
    s: &Style,
) {
    let a = shown.region;
    let w = body.width();
    // The engine shortens overlapping fades in proportion; draw what it plays.
    let (mut fi, mut fo) = (
        a.fade_in.0.max(0) as f32 * px_per_tick,
        a.fade_out.0.max(0) as f32 * px_per_tick,
    );
    if fi + fo > w && fi + fo > 0.0 {
        let k = w / (fi + fo);
        fi *= k;
        fo *= k;
    }
    let line = with_alpha(
        if s.t.dark {
            shade(ink, 0.25)
        } else {
            shade(ink, -0.25)
        },
        0.95,
    );
    // The ink token is the canvas: dark on Slate, white on Paper — the veil darkens or
    // washes out what the fade takes away, whichever the theme reads as "less".
    let veil = with_alpha(s.t.ink, if s.t.dark { 0.6 } else { 0.55 });
    for (fade, len) in [(Fade::In, fi), (Fade::Out, fo)] {
        if len < 1.0 {
            continue;
        }
        let curve = match fade {
            Fade::In => a.fade_in_curve,
            Fade::Out => a.fade_out_curve,
        };
        let (x0, x1) = match fade {
            Fade::In => (body.left(), body.left() + len),
            Fade::Out => (body.right() - len, body.right()),
        };
        let n = ((len / 2.0).ceil() as usize).clamp(2, CURVE_POINTS);
        scratch.pts.clear();
        for i in 0..=n {
            let u = i as f32 / n as f32;
            let x = x0 + (x1 - x0) * u;
            let g = match fade {
                Fade::In => curve.gain(u),
                Fade::Out => curve.gain(1.0 - u),
            };
            scratch
                .pts
                .push(pos2(x, inner.bottom() - g * inner.height()));
        }
        // The veil: from the top of the waveform down to the curve.
        scratch.tops.clear();
        let top = inner.top();
        scratch
            .tops
            .extend(scratch.pts.iter().map(|p| pos2(p.x, top)));
        b.between(&scratch.tops, &scratch.pts, veil);
        b.polyline(&scratch.pts, 1.25 * s.k, line);
    }
}

/// A missing (or unreadable) file: a dark body, a dashed outline in the danger colour,
/// and what is missing, written once.
pub(super) fn missing(
    b: &mut Brush,
    lanes: Rect,
    shown: &Shown<'_>,
    inner: Rect,
    look: &ClipLook,
    path: Option<&str>,
    s: &Style,
) {
    let _ = shown;
    let k = s.k;
    b.rect(inner, Fill::Solid(missing_ground(look, s)));
    let c = s.t.danger;
    let (dash, gap) = (4.0 * k, 3.0 * k);
    let r = b.snap_rect(inner.shrink(1.0));
    let mut x = r.left();
    while x < r.right() {
        let e = (x + dash).min(r.right());
        b.hline(x, e, r.top(), b.px() * 1.5, c);
        b.hline(x, e, r.bottom(), b.px() * 1.5, c);
        x += dash + gap;
    }
    let mut y = r.top();
    while y < r.bottom() {
        let e = (y + dash).min(r.bottom());
        b.vline(r.left(), y, e, b.px() * 1.5, c);
        b.vline(r.right(), y, e, b.px() * 1.5, c);
        y += dash + gap;
    }
    if inner.height() < 10.0 * k {
        return;
    }
    // Kept on screen while the clip's start scrolls off to the left.
    let left = inner.left().max(lanes.left()) + 6.0 * k;
    let right = inner.right().min(lanes.right()) - 6.0 * k;
    let line_h = (14.0 * k).min(inner.height());
    let row = Rect::from_min_max(
        pos2(left, inner.center().y - line_h * 0.5),
        pos2(right, inner.center().y + line_h * 0.5),
    );
    if row.width() < 24.0 * k {
        return;
    }
    let word = Text::key("audioclips.missing", Role::Micro, c)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let painted = text::paint(b, row, &word, s.tk());
    let after = painted
        .map(|p| p.rect.right() + 6.0 * k)
        .unwrap_or(row.left());
    if let Some(p) = path {
        let name = std::path::Path::new(p)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(p);
        let rest = Rect::from_min_max(pos2(after, row.top()), row.max);
        if rest.width() > 12.0 * k {
            let tx = Text::new(name, Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle));
            let _ = text::paint(b, rest, &tx, s.tk());
        }
    }
}

/// A source still on the engine's loader thread (ADR 0025 §9): it plays silence until it
/// arrives, so the body is veiled and says so.
pub(super) fn loading(b: &mut Brush, lanes: Rect, inner: Rect, look: &ClipLook, s: &Style) {
    let k = s.k;
    b.rect(inner, Fill::Solid(with_alpha(missing_ground(look, s), 0.6)));
    if inner.height() < 10.0 * k {
        return;
    }
    let left = inner.left().max(lanes.left()) + 6.0 * k;
    let right = inner.right().min(lanes.right()) - 6.0 * k;
    let line_h = (14.0 * k).min(inner.height());
    let row = Rect::from_min_max(
        pos2(left, inner.center().y - line_h * 0.5),
        pos2(right, inner.center().y + line_h * 0.5),
    );
    if row.width() < 24.0 * k {
        return;
    }
    let word = Text::key("audioclips.loading", Role::Micro, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, row, &word, s.tk());
}

/// The handles: the fades' squares at the top corners and the gain's diamond on its
/// line, the one under the pointer (or being dragged) lit in the highlight colour.
pub(super) fn paint_handles(
    b: &mut Brush,
    h: &Handles,
    a: &AudioRegion,
    hot: Option<Handle>,
    ink: Color32,
    s: &Style,
) {
    let k = s.k;
    let rest = if s.t.dark { shade(ink, 0.2) } else { ink };
    let rim = with_alpha(s.t.shadow, 0.85);
    // The gain line, when it is not at the top (or the gain is being touched).
    let y = gain_y(h.wave, a.gain_db);
    if a.gain_db < -0.05 || hot == Some(Handle::Gain) {
        let c = if hot == Some(Handle::Gain) {
            s.t.highlight
        } else {
            with_alpha(rest, 0.8)
        };
        b.hline(h.wave.left(), h.wave.right(), y, b.px(), c);
    }
    for (which, r) in [(Handle::FadeIn, h.fade_in), (Handle::FadeOut, h.fade_out)] {
        let fill = if hot == Some(which) {
            s.t.highlight
        } else {
            rest
        };
        let r = b.snap_rect(r);
        b.rrect(r.expand(b.px()), 1.5 * k, Fill::Solid(rim));
        b.rrect(r, 1.0 * k, Fill::V(shade(fill, 0.08), shade(fill, -0.08)));
    }
    let g = h.gain;
    let fill = if hot == Some(Handle::Gain) {
        s.t.highlight
    } else {
        rest
    };
    let c = g.center();
    let r = g.width() * 0.62;
    let diamond = |r: f32| {
        [
            pos2(c.x, c.y - r),
            pos2(c.x + r, c.y),
            pos2(c.x, c.y + r),
            pos2(c.x - r, c.y),
        ]
    };
    b.polygon(&diamond(r + b.px()), rim);
    b.polygon(&diamond(r), fill);
}

/// A fade curve as an icon in `rect`: the curve of `fade` (rising for a fade in), lit or
/// at rest.
pub fn curve_icon(b: &mut Brush, rect: Rect, curve: FadeCurve, fade: Fade, color: Color32, k: f32) {
    let r = rect.shrink(3.0 * k);
    let n = 16;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let u = i as f32 / n as f32;
            let g = match fade {
                Fade::In => curve.gain(u),
                Fade::Out => curve.gain(1.0 - u),
            };
            pos2(r.left() + u * r.width(), r.bottom() - g * r.height())
        })
        .collect();
    b.area(&pts, r.bottom(), with_alpha(color, 0.25));
    b.polyline(&pts, 1.5 * k, color);
}
