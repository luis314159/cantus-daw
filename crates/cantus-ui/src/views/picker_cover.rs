//! Drawn covers (ADR 0029 §3): the card a device shows when it has no editor to picture —
//! a hosted plug-in, Firmus, the classic Delay and Reverb — and while a native device's
//! picture is being made. Komplete Kontrol's artwork tiles in the kit's vectors: a ground
//! in the device's hue with a quiet pattern, the category's glyph large at the right, the
//! vendor in small capitals, the name, and the format badge.
//!
//! Every shape is the brush's; colours come from the tokens and the device's hue (a
//! function hue for ours, one chosen from the vendor's name for a plug-in, so a vendor's
//! plug-ins share a colour).

use crate::kit::color::{mix, on_fill, with_alpha, Hsl};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Function, Style, Tok};
use crate::picker_catalog::{Category, Device, Source};
use egui::{pos2, Align2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::{PI, TAU};

/// The hue a device's card is drawn in.
pub fn hue(d: &Device, s: &Style) -> Color32 {
    if d.source == Source::Native {
        return s.t.hue(d.hue());
    }
    // A vendor's plug-ins share a hue: one of the six function hues, turned a little.
    let h = crate::kit::cache::key(crate::picker_catalog::fold(&d.vendor));
    let base = s.t.hue(FUNCTIONS[(h % 6) as usize]);
    let mut hsl = Hsl::of(base);
    hsl.h += ((h >> 8) % 41) as f32 - 20.0;
    hsl.rgb()
}

const FUNCTIONS: [Function; 6] = [
    Function::Source,
    Function::Tone,
    Function::Dynamics,
    Function::Mod,
    Function::Space,
    Function::Output,
];

/// The card's ground: the hue over the theme's deepest surface, lit from above.
fn ground(s: &Style, hue: Color32) -> (Color32, Color32) {
    if s.t.dark {
        (mix(s.t.ink, hue, 0.42), mix(s.t.ink, hue, 0.14))
    } else {
        (mix(s.t.panel_light, hue, 0.55), mix(s.t.panel, hue, 0.30))
    }
}

/// Paint `d`'s card in `rect` (corner radius `r`).
pub fn card(b: &mut Brush, rect: Rect, r: f32, s: &Style, d: &Device) {
    let hue = hue(d, s);
    let (top, bottom) = ground(s, hue);
    b.register(
        rect,
        "picker-cover",
        &[Tok::Ink, Tok::Panel, Tok::PanelLight],
        &[],
    );
    b.rrect(rect, r, Fill::V(top, bottom));
    let ink = on_fill(mix(top, bottom, 0.5));
    // A quiet pattern: arcs from the lower right, spaced by the device's key.
    let seed = crate::kit::cache::key(&d.key);
    let centre = pos2(
        rect.right() + rect.width() * 0.05,
        rect.bottom() + rect.height() * 0.1,
    );
    let step = rect.height() * (0.16 + (seed % 5) as f32 * 0.015);
    for i in 1..9 {
        let rad = step * i as f32;
        b.arc(
            centre,
            rad,
            PI,
            PI * 1.5,
            s.s(1.0),
            with_alpha(ink, 0.07),
            false,
        );
    }
    // The category's glyph, large, right of centre.
    let g = rect.height() * 0.46;
    let gr = Rect::from_center_size(
        pos2(
            rect.right() - g * 0.75,
            rect.center().y - rect.height() * 0.06,
        ),
        Vec2::splat(g),
    );
    glyph(
        b,
        gr,
        d.primary(),
        with_alpha(mix(hue, ink, 0.35), 0.85),
        s.s(2.4).max(1.5),
    );
    // Words: the vendor in small capitals, the name, the badge.
    let pad = s.s(10.0);
    let vendor = Rect::from_min_size(
        rect.min + Vec2::new(pad, pad * 0.8),
        Vec2::new(rect.width() * 0.62, s.s(12.0)),
    );
    let vt = Text::new(&d.vendor, Role::Micro, with_alpha(ink, 0.8))
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, vendor, &vt, s.tk());
    let name = Rect::from_min_max(
        pos2(rect.left() + pad, rect.bottom() - pad - s.s(20.0)),
        pos2(rect.right() - pad, rect.bottom() - pad * 0.6),
    );
    let nt = Text::new(&d.name, Role::Title, ink)
        .align(Align2::LEFT_BOTTOM)
        .policy(Policy::Elide(Elide::End));
    // The name set large: the title role, half again its size (a cover is artwork).
    let big = (rect.width() / s.s(200.0)).clamp(1.0, 1.6) * 1.35;
    let _ = text::paint(b, name, &nt, s.tk() * big);
    if let Some(badge) = d.source.badge() {
        self::badge(b, rect, s, badge, ink);
    }
}

/// A format badge (`CLAP`, `VST3`) in the top-right corner of `rect`.
pub fn badge(b: &mut Brush, rect: Rect, s: &Style, word: &str, ink: Color32) {
    let t = Text::new(word, Role::Micro, ink).caps(true);
    let w = text::measure(b.ctx(), &t, s.tk()).x + s.s(10.0);
    let r = Rect::from_min_size(
        pos2(rect.right() - w - s.s(7.0), rect.top() + s.s(7.0)),
        Vec2::new(w, s.s(15.0)),
    );
    let r = b.snap_rect(r);
    b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(s.t.ink, 0.55)));
    b.rrect_stroke(r, s.r(3.0), s.s(1.0), with_alpha(ink, 0.35));
    let _ = text::paint(b, r, &t, s.tk());
}

/// A category's symbol, drawn in lines inside the square `rect`.
pub fn glyph(b: &mut Brush, rect: Rect, cat: Category, c: Color32, w: f32) {
    let r = rect.shrink(rect.width() * 0.08);
    let (x0, y0, ww, hh) = (r.left(), r.top(), r.width(), r.height());
    let at = |u: f32, v: f32| pos2(x0 + u * ww, y0 + v * hh);
    let wave = |b: &mut Brush, f: &dyn Fn(f32) -> f32, amp: f32| {
        let pts: Vec<Pos2> = (0..=48)
            .map(|i| {
                let u = i as f32 / 48.0;
                at(u, 0.5 - f(u) * amp)
            })
            .collect();
        b.polyline(&pts, w, c);
    };
    match cat {
        Category::Synths => {
            // A saw, then a sine: what a synth starts from.
            wave(
                b,
                &|u| {
                    if u < 0.5 {
                        1.0 - 4.0 * (u % 0.25) * 2.0
                    } else {
                        (u * TAU * 2.0).sin()
                    }
                },
                0.32,
            );
        }
        Category::Keys => {
            let kw = ww / 4.0;
            for i in 0..4 {
                let k = Rect::from_min_size(at(i as f32 * 0.25, 0.08), Vec2::new(kw, hh * 0.84));
                b.rrect_stroke(k.shrink(w * 0.5), w, w * 0.8, c);
            }
            for i in [1.0, 2.0] {
                let k = Rect::from_min_size(
                    pos2(x0 + i * kw - kw * 0.3, y0 + hh * 0.08),
                    Vec2::new(kw * 0.6, hh * 0.5),
                );
                b.rrect(k, w, Fill::Solid(c));
            }
        }
        Category::Bass => {
            wave(b, &|u| (u * TAU).sin(), 0.36);
            b.hline(x0, x0 + ww, y0 + hh * 0.5, w * 0.4, with_alpha(c, 0.5));
        }
        Category::Drums => {
            let ctr = at(0.5, 0.58);
            b.circle_stroke(ctr, ww * 0.34, w, c);
            b.circle_stroke(ctr, ww * 0.12, w * 0.7, with_alpha(c, 0.6));
            b.segment(at(0.12, 0.08), at(0.42, 0.42), w, c, true);
            b.segment(at(0.88, 0.08), at(0.58, 0.42), w, c, true);
        }
        Category::Samplers => {
            let n = 13;
            for i in 0..n {
                let u = (i as f32 + 0.5) / n as f32;
                let a = ((u * 9.0).sin() * 0.5 + 0.5) * (1.0 - (u - 0.3).abs()) * 0.42 + 0.04;
                b.segment(at(u, 0.5 - a), at(u, 0.5 + a), w * 0.8, c, true);
            }
        }
        Category::OtherInstruments => {
            b.circle(at(0.36, 0.76), ww * 0.14, Fill::Solid(c));
            b.segment(at(0.49, 0.76), at(0.49, 0.12), w, c, true);
            b.segment(at(0.49, 0.12), at(0.72, 0.3), w, c, true);
        }
        Category::Eq => {
            b.hline(x0, x0 + ww, y0 + hh * 0.62, w * 0.4, with_alpha(c, 0.5));
            wave(
                b,
                &|u| {
                    let bell = (-(u - 0.42).powi(2) / 0.012).exp() * 0.9;
                    let cut = if u < 0.12 { -(0.12 - u) * 6.0 } else { 0.0 };
                    bell + cut - 0.24
                },
                0.38,
            );
        }
        Category::Dynamics => {
            let pts = [at(0.06, 0.94), at(0.5, 0.5), at(0.94, 0.3)];
            b.segment(
                at(0.06, 0.94),
                at(0.94, 0.06),
                w * 0.5,
                with_alpha(c, 0.4),
                true,
            );
            b.polyline(&pts, w, c);
            b.circle(pts[1], w * 1.4, Fill::Solid(c));
        }
        Category::Space => {
            let o = at(0.12, 0.88);
            for i in 0..4 {
                let rad = ww * (0.25 + i as f32 * 0.22);
                let a = 1.0 - i as f32 * 0.22;
                b.arc(o, rad, -PI * 0.5, 0.0, w, with_alpha(c, a), true);
            }
        }
        Category::Distortion => {
            b.hline(x0, x0 + ww, y0 + hh * 0.18, w * 0.4, with_alpha(c, 0.5));
            b.hline(x0, x0 + ww, y0 + hh * 0.82, w * 0.4, with_alpha(c, 0.5));
            wave(b, &|u| ((u * TAU * 1.5).sin() * 1.8).clamp(-1.0, 1.0), 0.32);
        }
        Category::Modulation => {
            wave(b, &|u| (u * TAU * 2.0).sin() * (0.35 + 0.65 * u), 0.4);
        }
        Category::Pitch => {
            // A voice's wavering pitch, and the steps of the scale it is tuned to.
            wave(b, &|u| (u * TAU * 2.5).sin() * 0.25 + (u - 0.5) * 1.1, 0.4);
            for i in 0..4 {
                let u = i as f32 / 4.0;
                let v = 0.5 - ((u + 0.125) - 0.5) * 1.1 * 0.4;
                b.segment(at(u, v), at(u + 0.25, v), w, with_alpha(c, 0.55), true);
            }
        }
        Category::Stereo => {
            b.circle_stroke(at(0.36, 0.5), ww * 0.27, w, c);
            b.circle_stroke(at(0.64, 0.5), ww * 0.27, w, with_alpha(c, 0.7));
        }
        Category::Mastering => {
            b.hline(x0, x0 + ww, y0 + hh * 0.16, w, c);
            let n = 11;
            for i in 0..n {
                let u = (i as f32 + 0.5) / n as f32;
                let a = 0.2 + 0.62 * ((u * 7.3).sin() * 0.5 + 0.5).powf(0.6);
                b.segment(
                    at(u, 0.94),
                    at(u, 0.94 - a.min(0.74)),
                    w * 0.9,
                    with_alpha(c, 0.8),
                    true,
                );
            }
        }
        Category::Utility => {
            let ctr = at(0.5, 0.74);
            b.arc(ctr, ww * 0.44, PI, TAU, w, c, true);
            b.segment(ctr, at(0.72, 0.36), w, c, true);
            b.circle(ctr, w * 1.6, Fill::Solid(c));
        }
    }
}
