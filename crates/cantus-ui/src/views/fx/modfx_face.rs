//! The modulation family's characteristic displays (round 32, ADR 0074): what each unit
//! draws over its display so its movement reads as its own — Concentus's bucket-brigade
//! beads, Flexus's zero crossings, the notches of Flexus's comb and Phasis's cascade,
//! Tremor's chart pen, Vibrans's tuner scale, Anulus's sideband rings, Translatio's shift
//! arrows, Fluctus's riding level. The marks are drawn in the display's own colours
//! ([`Ink`]) and the skin's second voice ([`second_voice`]); with no skin (the switch off,
//! a plain sheet) the second voice is the display's faint line, as before.

use super::canvas::Ink;
use crate::kit::color::with_alpha;
use crate::kit::{Brush, Style};
use egui::{pos2, Color32, Pos2, Rect};

/// The display's second voice: the look's second trace — the skin's second accent, which
/// the kit keeps 3:1 on the display's ground and apart from the main trace on every look.
/// `None` for a face with no second accent (the plain look: the switch off).
pub fn second_voice(s: &Style, ink: &Ink) -> Option<Color32> {
    s.skin.accent2.map(|_| ink.look.trace2)
}

/// Whether the style is a unit's own face (the switch on, a sheet with an accent): the
/// family's marks are drawn only then — with the switch off every picture is as before.
pub fn own_face(s: &Style) -> bool {
    s.skin.accent.is_some()
}

/// The right channel's line: the second voice in a face of its own, the faint line else.
pub fn right_line(s: &Style, ink: &Ink) -> (Color32, f32) {
    match second_voice(s, ink) {
        Some(c) => (c, 1.6),
        None => (ink.faint, 1.4),
    }
}

/// What a movement display draws over its curves (`modfx::Extras::marks`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum Marks {
    /// Nothing more.
    #[default]
    None,
    /// Concentus: beads along each voice's line, a bucket brigade's stages.
    Buckets,
    /// Flexus through zero: a flare where the wet's line crosses this value (−1…1).
    Crossings(f32),
    /// Tremor: a chart recorder's pen at the playhead and its time ticks along the top.
    Chart,
    /// Vibrans: a tuner's scale down the side and its pointer at the pitch now.
    Tuner,
}

/// Beads every `step` along `pts` (in `color`), the line under them thinner.
pub fn beads(b: &mut Brush, pts: &[Pos2], step: f32, r: f32, color: Color32) {
    let mut run = step * 0.5;
    for w in pts.windows(2) {
        let d = w[1] - w[0];
        let len = d.length();
        let mut t = 0.0;
        while run + (len - t) >= step {
            t += step - run;
            run = 0.0;
            b.dot(w[0] + d * (t / len.max(1e-6)), r, color);
        }
        run += len - t;
    }
}

/// Where the line through `pts` crosses the height `y`, rising or falling, once each — a
/// crossing that lands exactly on a point counts once (the side of the line is "above" or
/// "on or below", so a point on the line is where the side changes, not two crossings).
pub fn crossing_xs(pts: &[Pos2], y: f32, mut f: impl FnMut(f32)) {
    let mut last: Option<f32> = None;
    for w in pts.windows(2) {
        let (a, c) = (w[0].y - y, w[1].y - y);
        if (a > 0.0) == (c > 0.0) {
            continue;
        }
        let t = a / (a - c);
        let x = w[0].x + (w[1].x - w[0].x) * t;
        // A touch that leaves at once (on the line for one point) is one crossing.
        if last.is_none_or(|l| (x - l).abs() > 1e-3) {
            f(x);
        }
        last = Some(x);
    }
}

/// A flare at every place `pts` crosses the height `y`: a ring and a dot in `color`.
pub fn crossings(b: &mut Brush, pts: &[Pos2], y: f32, k: f32, color: Color32) {
    crossing_xs(pts, y, |x| {
        b.circle_stroke(pos2(x, y), 7.0 * k, 1.6 * k, with_alpha(color, 0.85));
        b.circle_stroke(pos2(x, y), 11.0 * k, 1.0 * k, with_alpha(color, 0.35));
        b.dot(pos2(x, y), 3.0 * k, color);
    });
}

/// A chart recorder's ticks along the top of `plot` (every eighth of a cycle over two
/// cycles, the cycle's quarters longer): `(x, top, bottom)` of each that keeps clear of the
/// display's corner words (`words`, with a margin).
pub fn chart_ticks(plot: Rect, k: f32, words: &[Rect], mut f: impl FnMut(f32, f32, f32)) {
    for i in 0..=16 {
        let x = plot.left() + plot.width() * i as f32 / 16.0;
        let h = if i % 4 == 0 { 5.0 } else { 2.5 } * k;
        let (top, bottom) = (plot.top() - 6.0 * k, plot.top() - 6.0 * k + h);
        let tick = Rect::from_x_y_ranges(x - 1.0 * k..=x + 1.0 * k, top..=bottom);
        if words.iter().any(|w| w.expand(2.0 * k).intersects(tick)) {
            continue;
        }
        f(x, top, bottom);
    }
}

/// [`chart_ticks`] and the chart's pen: a triangle at `at` pointing down the curve, in
/// `color`.
#[allow(clippy::too_many_arguments)]
pub fn chart(
    b: &mut Brush,
    plot: Rect,
    at: Option<Pos2>,
    k: f32,
    ink: &Ink,
    color: Color32,
    words: &[Rect],
) {
    chart_ticks(plot, k, words, |x, top, bottom| {
        b.vline(x, top, bottom, b.px(), ink.dim);
    });
    if let Some(p) = at {
        let (w, h) = (5.0 * k, 9.0 * k);
        b.polygon(
            &[
                pos2(p.x, p.y - 1.0 * k),
                pos2(p.x - w, p.y - h),
                pos2(p.x + w, p.y - h),
            ],
            color,
        );
        b.dot(p, 2.0 * k, color);
    }
}

/// A tuner's scale: down the right of `plot`, from under the display's corner readout
/// (whose bottom is `below`) to the plot's foot — the scale's x and the height of `v`
/// (−1…1, +1 at the top).
pub fn tuner_scale(plot: Rect, below: f32, k: f32) -> (f32, impl Fn(f32) -> f32) {
    let top = plot.top().max(below + 6.0 * k);
    let bottom = plot.bottom();
    let mid = (top + bottom) * 0.5;
    let half = ((bottom - top) * 0.5).max(0.0);
    (plot.right() + 3.0 * k, move |v: f32| {
        mid - v.clamp(-1.0, 1.0) * half
    })
}

/// The tuner's pointer at `v`: the triangle's corners.
pub fn tuner_pointer(plot: Rect, below: f32, v: f32, k: f32) -> [Pos2; 3] {
    let (x, y_of) = tuner_scale(plot, below, k);
    let y = y_of(v);
    let (w, h) = (6.0 * k, 4.5 * k);
    [pos2(x - w - 1.0 * k, y), pos2(x, y - h), pos2(x, y + h)]
}

/// A tuner's scale down the right of `plot`, under the readout (ticks at every eighth of
/// the depth, the middle and the ends long), and its pointer at `v` in `color`.
#[allow(clippy::too_many_arguments)]
pub fn tuner(
    b: &mut Brush,
    plot: Rect,
    below: f32,
    v: Option<f32>,
    k: f32,
    ink: &Ink,
    color: Color32,
) {
    let (x, y_of) = tuner_scale(plot, below, k);
    for i in -4..=4 {
        let len = match i {
            0 => 7.0,
            -4 | 4 => 5.0,
            _ => 3.0,
        } * k;
        let c = if i == 0 { ink.text } else { ink.dim };
        b.hline(x - len, x, y_of(i as f32 / 4.0), b.px().max(1.0 * k), c);
    }
    if let Some(v) = v {
        b.polygon(&tuner_pointer(plot, below, v, k), color);
    }
}

/// The notches of a response drawn through `pts` (screen points, the top louder): a mark
/// under every dip at least `depth` points deep below its neighbours' crests, on `base`.
pub fn notches(b: &mut Brush, pts: &[Pos2], base: f32, depth: f32, k: f32, color: Color32) {
    let n = pts.len();
    let mut i = 1;
    while i + 1 < n {
        let y = pts[i].y;
        if y >= pts[i - 1].y && y > pts[i + 1].y {
            // A dip (screen y grows downwards): how far it falls under the crests either side.
            let crest = |range: &mut dyn Iterator<Item = usize>| {
                let mut best = y;
                for j in range {
                    if pts[j].y > y {
                        break;
                    }
                    best = best.min(pts[j].y);
                }
                best
            };
            let left = crest(&mut (0..i).rev());
            let right = crest(&mut (i + 1..n));
            if y - left.max(right) >= depth {
                let x = pts[i].x;
                let (w, h) = (3.5 * k, 5.0 * k);
                b.polygon(
                    &[pos2(x, base - h), pos2(x - w, base), pos2(x + w, base)],
                    color,
                );
            }
        }
        i += 1;
    }
}

/// A ring joining each pair of sidebands round the input: an arch from `a` to `c` (the
/// partials' tops), its crown `lift` above the higher of the two, in `color`.
pub fn ring(b: &mut Brush, a: Pos2, c: Pos2, lift: f32, w: f32, color: Color32) {
    const N: usize = 25;
    let top = a.y.min(c.y) - lift;
    let mut pts = [Pos2::ZERO; N];
    for (i, p) in pts.iter_mut().enumerate() {
        let t = i as f32 / (N - 1) as f32;
        // A half sine from one top to the other.
        let s = cantus_dsp::math::sin(std::f32::consts::PI * t);
        let x = a.x + (c.x - a.x) * t;
        let y0 = a.y + (c.y - a.y) * t;
        *p = pos2(x, y0 + (top - y0) * s);
    }
    b.polyline(&pts, w, color);
}

/// An arrow from `a` to `c` (a harmonic carried across by the shift), `w` wide.
pub fn arrow(b: &mut Brush, a: Pos2, c: Pos2, w: f32, k: f32, color: Color32) {
    let d = c - a;
    let len = d.length();
    if len < 4.0 * k {
        return;
    }
    let u = d / len;
    let head = 5.0 * k;
    let back = c - u * head;
    let side = egui::vec2(-u.y, u.x) * head * 0.55;
    b.segment(a, back, w, color, false);
    b.polygon(&[c, back + side, back - side], color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::color::contrast;
    use crate::kit::skin::{self, Unit};
    use crate::kit::Tokens;
    use cantus_core::EffectKind as K;

    const FAMILY: [K; 8] = [
        K::Chorus,
        K::Flanger,
        K::Phaser,
        K::Tremolo,
        K::Vibrato,
        K::RingMod,
        K::FreqShift,
        K::VolumeShaper,
    ];

    /// Every unit of the family draws its other channel in a second voice of its own on its
    /// display, in every theme preset: 3:1 on the display's ground, apart from the trace.
    #[test]
    fn every_face_has_a_second_voice_on_its_display_in_every_theme() {
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for kind in FAMILY {
                let s = Style::new(base).skinned(skin::of(Unit::Effect(kind)));
                let ink = Ink::of(&s);
                match second_voice(&s, &ink) {
                    Some(c) if contrast(c, ink.ground) + 1e-3 >= 3.0 => {}
                    other => bad.push(format!("{name} {kind:?}: {other:?}")),
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// A unit's header (its nameplate, stripe, lamps) and its body paint in its accent,
    /// never in the theme's modulation hue, which a skinned style keeps for `Mod` cues.
    #[test]
    fn every_face_s_header_and_body_paint_in_its_accent() {
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for kind in FAMILY {
                let skin = skin::of(Unit::Effect(kind));
                let header = Style::new(base)
                    .function(super::super::frame::hue_of(kind))
                    .skinned(skin);
                let body = super::super::body(
                    &Style::new(base)
                        .function(crate::kit::Function::Mod)
                        .skinned(skin),
                );
                for (what, s) in [("header", header), ("body", body)] {
                    if s.hue != s.t.hue_tone || s.hue == s.t.hue_mod {
                        bad.push(format!("{name} {kind:?}: the {what} is not in the accent"));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{}",
            bad.join(
                "
"
            )
        );
    }

    /// Inside every unit's display, in every preset, the words read on every fill they are
    /// drawn on — the look's own ground and the plate under a label (never a theme fill:
    /// an LCD is light in Slate, a CRT dark in Paper) — and the traces show on it; the
    /// ink's polarity is the display's, not the theme's.
    #[test]
    fn every_display_s_words_and_traces_read_on_its_own_ground_in_every_theme() {
        use crate::kit::color::{luminance, over};
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for kind in FAMILY {
                let skin = skin::of(Unit::Effect(kind));
                let s = super::super::body(
                    &Style::new(base)
                        .function(super::super::frame::hue_of(kind))
                        .skinned(skin),
                );
                let ink = Ink::of(&s);
                let d = crate::kit::skin::display_ink(&s);
                if ink.ground != d.ground {
                    bad.push(format!("{name} {kind:?}: not the look's ground"));
                }
                if ink.dark != (luminance(ink.ground) < 0.18) {
                    bad.push(format!(
                        "{name} {kind:?}: the ink's polarity is not the display's"
                    ));
                }
                for (fill, bg) in [
                    ("ground", ink.ground),
                    ("plate", over(ink.plate, ink.ground)),
                ] {
                    for (what, fg, need) in [
                        ("text", ink.text, 4.5),
                        ("dim", ink.dim, 3.0),
                        ("trace", ink.hue, 3.0),
                    ] {
                        let c = contrast(fg, bg);
                        if c + 1e-3 < need {
                            bad.push(format!("{name} {kind:?}: {what} on {fill} {c:.2}"));
                        }
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{}",
            bad.join(
                "
"
            )
        );
    }

    /// Every crossing of the line, rising and falling, once — also those that land exactly
    /// on a point (the movement display's samples put the ¼ and ¾ crossings on points).
    #[test]
    fn every_crossing_is_found_once_rising_or_falling() {
        let pts = [
            (0.0, 10.0),
            (1.0, 0.0),
            (2.0, -10.0),
            (3.0, 0.0),
            (4.0, 10.0),
            (5.0, 0.0),
        ]
        .map(|(x, y)| pos2(x, y));
        let mut xs = Vec::new();
        crossing_xs(&pts, 0.0, |x| xs.push(x));
        assert_eq!(xs, vec![1.0, 3.0, 5.0]);
        // Between points, both ways.
        let mut xs = Vec::new();
        crossing_xs(
            &[pos2(0.0, 5.0), pos2(2.0, -5.0), pos2(4.0, 5.0)],
            0.0,
            |x| xs.push(x),
        );
        assert_eq!(xs, vec![1.0, 3.0]);
        // A touch of the line and back is one crossing, not two.
        let mut xs = Vec::new();
        crossing_xs(
            &[pos2(0.0, 5.0), pos2(1.0, 0.0), pos2(2.0, 5.0)],
            0.0,
            |x| xs.push(x),
        );
        assert_eq!(xs, vec![1.0]);
    }

    /// The display's corner words as the movement display places them, at `k`: the top
    /// name at the left, the rate on its plate at the right.
    fn corner_words(ctx: &egui::Context, rect: Rect, top: &str, rate: &str, k: f32) -> [Rect; 2] {
        use crate::kit::text::{measure, Role, Text};
        let c = Color32::WHITE;
        let left = measure(ctx, &Text::new(top, Role::Micro, c).caps(true), k);
        let right = measure(ctx, &Text::new(rate, Role::Value, c).caps(false), k);
        [
            egui::Align2::LEFT_TOP
                .anchor_size(pos2(rect.left() + 6.0 * k, rect.top() + 3.0 * k), left),
            egui::Align2::RIGHT_TOP
                .anchor_size(pos2(rect.right() - 6.0 * k, rect.top() + 3.0 * k), right)
                .expand2(egui::vec2(3.0 * k, 1.0 * k)),
        ]
    }

    /// Nothing overlaps: Vibrans's tuner pointer at full depth and Tremor's chart ticks
    /// keep clear of the display's corner words, at 75, 100 and 200 %.
    #[test]
    fn the_tuner_and_the_chart_keep_clear_of_the_corner_words() {
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        for k in [0.75f32, 1.0, 2.0] {
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(690.0, 340.0) * k);
            let plot = rect.shrink2(egui::vec2(10.0 * k, 14.0 * k));
            let words = corner_words(&ctx, rect, "+35 ct", "5.50 Hz", k);
            for v in [1.0f32, 0.0, -1.0] {
                let p = tuner_pointer(plot, words[1].bottom(), v, k);
                let r = Rect::from_points(&p);
                assert!(
                    !r.intersects(words[1]),
                    "pointer at {v}, {k}: {r:?} {:?}",
                    words[1]
                );
            }
            let words = corner_words(&ctx, rect, "Full", "6.20 Hz", k);
            let mut n = 0;
            chart_ticks(plot, k, &words, |x, top, bottom| {
                n += 1;
                let tick = Rect::from_x_y_ranges(x..=x, top..=bottom);
                assert!(
                    !words.iter().any(|w| w.intersects(tick)),
                    "tick at {x}, {k}"
                );
            });
            assert!(n >= 12, "{n} ticks at {k}");
        }
    }

    /// With the faces off (the plain look) the displays keep their faint right channel.
    #[test]
    fn the_plain_look_keeps_the_faint_line() {
        // With the switch off nothing of the faces reaches a picture: no marks, no second
        // voice, the family's own hue in the header and the body, the plain display ink.
        skin::set_enabled(false);
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for kind in FAMILY {
                let sheet = skin::of(Unit::Effect(kind));
                let f = super::super::frame::hue_of(kind);
                let s = Style::new(base).function(f).skinned(sheet);
                let ink = Ink::of(&s);
                let plain = Style::new(base).function(super::super::frame::hue_of(kind));
                let ok = !own_face(&s)
                    && second_voice(&s, &ink).is_none()
                    && right_line(&s, &ink) == (ink.faint, 1.4)
                    && f == super::super::frame::hue_of(kind)
                    && super::super::body(&s) == s
                    && s.hue == plain.hue
                    && ink.ground == base.ink
                    && ink.text == base.text;
                if !ok {
                    bad.push(format!("{name} {kind:?}"));
                }
            }
        }
        skin::set_enabled(true);
        assert!(
            bad.is_empty(),
            "{}",
            bad.join(
                "
"
            )
        );
    }
}
