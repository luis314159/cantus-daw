//! Caelum's sky (design sheet v2): the observatory's window on the tank and the shimmer.
//!
//! The last eight seconds run left to right, the now at the right. **The tail** is a band
//! of light on the horizon (the input's register), as thick and as bright as the tank's
//! energy, mirrored in the still water below it (the tank). **The shimmer's octaves** are
//! bands above it, a voice's interval apart, each lit by the share of the energy that has
//! climbed to it (`display::View::climb`: what is new enters at the horizon, a part of each
//! band climbs a step every pass, the measured shimmer share sets how much has left) — so
//! the light is seen rising band by band as the tail ages, and fading at the top; wisps
//! of light climb between the bands where the energy is passing. Behind: depth (a sky
//! darker toward the zenith, a glow on the horizon), the Milky Way, three magnitudes of
//! stars brightening with the tank. The frame's rails carry the scales: a tick each
//! second along the foot, a tick at each band along the left. Infinite writes ∞; a frozen
//! tank frosts the glass; the ducking's reduction lights a slot in the right rail.

use super::super::canvas::{self, Ink};
use super::super::frame::tr;
use super::art::{self, c, hash, Signed};
use super::display::{layer_of, Reading, View, KEPT, LAYERS};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::skin::observatory as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_dsp::math::sin;
use egui::{pos2, vec2, Align2, Rect, Vec2};
use std::f32::consts::{PI, TAU};

/// The pitch scale's range above (and below) the horizon, semitones.
fn range(spec: &EffectSpec) -> (f32, f32) {
    let (pa, pb) = (spec.get("pa"), spec.get("pb"));
    let up = (0..=4)
        .flat_map(|n| [pa * n as f32, pb * n as f32])
        .fold(0.0f32, f32::max)
        .clamp(24.0, 48.0);
    let down = pa.min(pb).clamp(-24.0, 0.0);
    (down, up)
}

/// Where everything in the glass goes.
struct Sky {
    /// The bands' area: the gutter for the bands' names at its left.
    plot: Rect,
    horizon: f32,
    lo: f32,
    hi: f32,
}

impl Sky {
    fn y(&self, st: f32) -> f32 {
        let top = self.plot.top();
        let bottom = self.horizon - 12.0;
        bottom - (st - self.lo) / (self.hi - self.lo) * (bottom - top)
    }

    fn x(&self, col: usize, n: usize) -> f32 {
        let w = self.plot.width() / (KEPT - 1) as f32;
        self.plot.right() - (n - 1 - col) as f32 * w
    }
}

/// The sky in the window: `fr` the frame (its rails take the scales), `g` the glass.
#[allow(clippy::too_many_arguments)]
pub fn show(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    fr: Rect,
    g: Rect,
    view: &View,
    now: &Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let horizon = g.bottom() - g.height() * 0.16;
    let (lo, hi) = range(spec);
    let sky = Sky {
        plot: Rect::from_min_max(
            pos2(g.left() + 44.0 * k, g.top() + 50.0 * k),
            pos2(g.right() - 8.0 * k, horizon),
        ),
        horizon,
        lo,
        hi,
    };
    let glow = ink.voice(ink.hue, false);
    let energy = super::display::light(now.energy_db);
    let frozen = (now
        .freeze
        .max(if spec.get("freeze") >= 0.5 { 1.0 } else { 0.0 }))
    .clamp(0.0, 1.0);
    let twinkle = if now.seen && frozen < 0.5 {
        view.phase
    } else {
        0.0
    };
    // Depth: the zenith darkest, the horizon glowing in the tail's light.
    let deep = shade(ink.ground, -0.02);
    b.rect(
        Rect::from_min_max(g.min, pos2(g.right(), horizon)),
        Fill::Stops(&[
            (0.0, deep),
            (0.55, mix(ink.ground, glow, 0.05)),
            (1.0, mix(ink.ground, glow, 0.16 + 0.10 * energy)),
        ]),
    );
    milky_way(b, k, g, horizon, energy);
    stars(b, k, g, horizon, energy, twinkle);
    b.radial(
        g,
        pos2(g.center().x, horizon),
        vec2(g.width() * 0.7, g.height() * 0.22),
        &[
            (0.0, with_alpha(glow, 0.10 + 0.14 * energy)),
            (1.0, with_alpha(glow, 0.0)),
        ],
        12,
    );
    water(b, k, g, horizon, ink, &sky, view, spec);
    rows(b, s, ink, &sky, spec, view);
    bands(b, k, &sky, view, spec, ink, false);
    wisps(b, k, &sky, view, spec, ink);
    centre(b, k, &sky, view, spec, ink);
    now_mark(b, k, &sky, now, ink);
    // The horizon itself, a fine line of light.
    b.hline(
        g.left(),
        g.right(),
        horizon,
        0.8 * k,
        with_alpha(c(pal::STAR), 0.22),
    );
    art::frost(b, g, k, frozen);
    words(b, s, ink, g, &sky, now, spec);
    art::glass(b, g, k);
    rails(b, s, fr, g, &sky, spec, view);
    ducking(b, s, ink, fr, g, now, spec);
}

/// The Milky Way: a soft band of light from the lower left to the upper right, thick with
/// faint stars.
fn milky_way(b: &mut Brush, k: f32, g: Rect, horizon: f32, energy: f32) {
    let h = horizon - g.top();
    let at = |u: f32| {
        pos2(
            g.left() + g.width() * u,
            horizon - h * (0.18 + 0.7 * u) + h * 0.12 * sin(u * PI),
        )
    };
    let haze = c(pal::STAR);
    for i in 0..16 {
        let u = (i as f32 + 0.5) / 16.0;
        let p = at(u);
        let rr = h * (0.10 + 0.06 * hash(i + 700));
        b.radial(
            g,
            p,
            Vec2::splat(rr),
            &[
                (0.0, with_alpha(haze, 0.05 + 0.03 * energy)),
                (1.0, with_alpha(haze, 0.0)),
            ],
            8,
        );
    }
    for i in 0..380u32 {
        let u = hash(i * 3 + 1000);
        let p = at(u);
        // Gathered toward the band's spine: the sum of three draws.
        let off = (hash(i * 3 + 1001) + hash(i * 3 + 1002) + hash(i * 5 + 1003) - 1.5) * h * 0.16;
        let q = pos2(p.x + off * 0.4, p.y + off);
        if q.y > horizon - 2.0 * k || q.y < g.top() || q.x < g.left() || q.x > g.right() {
            continue;
        }
        b.dot(q, 0.45 * k, with_alpha(haze, 0.10 + 0.22 * hash(i + 1500)));
    }
}

/// Three magnitudes of stars over the sky (none below the horizon): many faint points, a
/// few dozen with a halo, a handful of bright ones with their diffraction spikes; all
/// brighter while the tank holds energy, twinkling while it plays.
fn stars(b: &mut Brush, k: f32, g: Rect, horizon: f32, energy: f32, phase: f32) {
    let white = c(pal::STAR);
    let spot = |i: u32| {
        pos2(
            g.left() + 3.0 * k + (g.width() - 6.0 * k) * hash(i * 2 + 1),
            g.top()
                + 3.0 * k
                + (horizon - g.top() - 8.0 * k) * {
                    let v = hash(i * 2 + 2);
                    v * (0.7 + 0.3 * v)
                },
        )
    };
    let tw = |i: u32| 0.75 + 0.25 * sin(TAU * (phase * (2.0 + 3.0 * hash(i + 77)) + hash(i + 99)));
    for i in 0..220u32 {
        let p = spot(i);
        let a = (0.10 + 0.30 * hash(i + 3000)) * (0.7 + 0.5 * energy) * tw(i);
        b.dot(p, (0.45 + 0.35 * hash(i + 3100)) * k, with_alpha(white, a));
    }
    for i in 220..262u32 {
        let p = spot(i);
        let a = (0.35 + 0.35 * hash(i + 3200)) * (0.75 + 0.35 * energy) * tw(i);
        b.radial(
            g,
            p,
            Vec2::splat(4.0 * k),
            &[
                (0.0, with_alpha(white, 0.30 * a)),
                (1.0, with_alpha(white, 0.0)),
            ],
            4,
        );
        b.dot(p, 0.9 * k, with_alpha(white, a));
    }
    for i in 262..270u32 {
        let p = spot(i);
        let a = (0.6 + 0.3 * hash(i + 3300)) * (0.8 + 0.3 * energy) * tw(i);
        let reach = (5.0 + 5.0 * hash(i + 3400)) * k;
        b.radial(
            g,
            p,
            Vec2::splat(reach * 1.2),
            &[
                (0.0, with_alpha(white, 0.35 * a)),
                (0.35, with_alpha(white, 0.08 * a)),
                (1.0, with_alpha(white, 0.0)),
            ],
            6,
        );
        for d in [vec2(1.0, 0.0), vec2(0.0, 1.0)] {
            b.segment(
                p - d * reach,
                p + d * reach,
                0.5 * k,
                with_alpha(white, 0.35 * a),
                false,
            );
            b.segment(
                p - d * reach * 0.45,
                p + d * reach * 0.45,
                0.8 * k,
                with_alpha(white, 0.6 * a),
                false,
            );
        }
        b.dot(p, 1.3 * k, with_alpha(white, a));
    }
}

/// The still water under the horizon (the tank): darker with depth, the tail's light
/// mirrored in it and broken by ripples, glints where the energy is.
#[allow(clippy::too_many_arguments)]
fn water(
    b: &mut Brush,
    k: f32,
    g: Rect,
    horizon: f32,
    ink: &Ink,
    sky: &Sky,
    view: &View,
    spec: &EffectSpec,
) {
    let glow = ink.voice(ink.hue, false);
    let w = Rect::from_min_max(pos2(g.left(), horizon), g.max);
    b.rect(
        w,
        Fill::V(mix(ink.ground, glow, 0.10), shade(ink.ground, -0.03)),
    );
    // The tail mirrored: the band's height turned over the horizon, fainter.
    bands(b, k, sky, view, spec, ink, true);
    // Ripples across it.
    let mut y = horizon + 2.0 * k;
    let mut i = 0u32;
    while y < g.bottom() - 1.0 * k {
        let x0 = g.left() + g.width() * hash(i + 5000) * 0.3;
        let x1 = g.right() - g.width() * hash(i + 5100) * 0.3;
        b.hline(
            x0,
            x1,
            y,
            0.6 * k,
            with_alpha(shade(ink.ground, -0.02), 0.35),
        );
        y += (2.2 + 1.2 * hash(i + 5200)) * k;
        i += 1;
    }
    // Glints: short dashes of light under the bright columns.
    let n = view.len();
    for j in 0..90u32 {
        if n < 2 {
            break;
        }
        let col = ((hash(j + 6000) * (n - 1) as f32) as usize).min(n - 1);
        let e = view.bright(col, 0);
        if e < 0.05 {
            continue;
        }
        let x = sky.x(col, n);
        let yy = horizon + 3.0 * k + (g.bottom() - horizon - 6.0 * k) * hash(j + 6100);
        let len = (3.0 + 9.0 * hash(j + 6200))
            * k
            * (1.0 - (yy - horizon) / (g.bottom() - horizon) * 0.5);
        b.hline(
            x - len * 0.5,
            x + len * 0.5,
            yy,
            0.6 * k,
            with_alpha(glow, 0.5 * e),
        );
    }
}

/// The bands' pitch rows: a fine dotted line at each lit band and its interval written in
/// the gutter (the second voice's where there is room).
fn rows(b: &mut Brush, s: &Style, ink: &Ink, sky: &Sky, spec: &EffectSpec, view: &View) {
    let k = s.k;
    let mut placed: [f32; LAYERS] = [f32::NAN; LAYERS];
    for l in 0..LAYERS {
        let Some((st, second)) = layer_of(spec, l) else {
            continue;
        };
        let y = sky.y(st);
        if y < sky.plot.top() - 1.0 || y > sky.horizon {
            continue;
        }
        let lit = view.peak(l);
        if l > 0 && lit < 0.02 {
            continue;
        }
        let col = if second {
            ink.voice(ink.hue, true)
        } else {
            ink.voice(ink.hue, false)
        };
        canvas::dashed(
            b,
            pos2(sky.plot.left(), y),
            pos2(sky.plot.right(), y),
            0.6 * k,
            with_alpha(ink.grid, 0.55),
            2.0 * k,
            5.0 * k,
        );
        if placed.iter().any(|p| (p - y).abs() < 12.0 * k) {
            continue;
        }
        placed[l] = y;
        canvas::putf(
            b,
            pos2(sky.plot.left() - 6.0 * k, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            mix(col, ink.text, 0.25),
            k,
            None,
            format_args!("{} st", Signed(st)),
        );
    }
}

/// The bands of light: the tail on the horizon and the shimmer's bands above it (or, with
/// `mirror`, the tail turned over the horizon into the water).
fn bands(
    b: &mut Brush,
    k: f32,
    sky: &Sky,
    view: &View,
    spec: &EffectSpec,
    ink: &Ink,
    mirror: bool,
) {
    let n = view.len();
    if n < 2 {
        return;
    }
    let ls: &[usize] = if mirror {
        &[0]
    } else {
        &[8, 7, 6, 5, 4, 3, 2, 1, 0]
    };
    for &l in ls {
        let (st, second) = if mirror {
            (0.0, false)
        } else {
            match layer_of(spec, l) {
                Some(v) => v,
                None => continue,
            }
        };
        let mut y = sky.y(st);
        if mirror {
            y = sky.horizon + (sky.horizon - sky.y(0.0)) + 2.0 * k;
        } else if y < sky.plot.top() - 1.0 || y > sky.horizon {
            continue;
        }
        if view.peak(l) < 0.01 {
            continue;
        }
        let base = if second {
            ink.voice(ink.hue, true)
        } else {
            ink.voice(ink.hue, false)
        };
        // Each band a little whiter the higher it has climbed.
        let rung = if l == 0 {
            0.0
        } else {
            ((l - 1) % 4 + 1) as f32
        };
        let col = mix(base, c(pal::STAR), 0.06 * rung);
        let tail = l == 0;
        let thick = if tail { 26.0 } else { 18.0 } * k;
        let fade = |j: usize| ((j as f32) / 14.0).min(1.0);
        let dim = if mirror { 0.4 } else { 1.0 };
        let shape = |j: usize, wide: f32, hot: f32| {
            let e = view.bright(j, l) * fade(j);
            let h = ((if tail { 3.0 } else { 2.0 }) * k * (e * 5.0).min(1.0) + thick * e) * wide;
            let colr = mix(col, c(pal::STAR), hot * e);
            (pos2(sky.x(j, n), y), h, with_alpha(colr, dim * e.sqrt()))
        };
        // The halo, the body, the hot core.
        b.ribbon(
            n,
            |j| shape(j, 2.4, 0.0),
            &[(-1.0, 0.0), (0.0, 0.16), (1.0, 0.0)],
        );
        b.ribbon(
            n,
            |j| shape(j, 1.0, 0.1),
            &[
                (-1.0, 0.0),
                (-0.5, 0.45),
                (0.0, 0.8),
                (0.5, 0.45),
                (1.0, 0.0),
            ],
        );
        if !mirror {
            b.ribbon(
                n,
                |j| shape(j, 0.22, 0.55),
                &[(-1.0, 0.0), (0.0, 0.9), (1.0, 0.0)],
            );
        }
    }
}

/// Wisps of light climbing from each band to the next where the energy is passing: up and
/// toward the now, as a sound rises while it ages.
fn wisps(b: &mut Brush, k: f32, sky: &Sky, view: &View, spec: &EffectSpec, ink: &Ink) {
    let n = view.len();
    if n < 8 {
        return;
    }
    let shim = spec.get("shim").clamp(0.0, 1.0);
    if shim <= 0.0 || spec.get("freeze") >= 0.5 {
        return;
    }
    for l in 1..LAYERS {
        let Some((st, second)) = layer_of(spec, l) else {
            continue;
        };
        let from = if l == 1 || l == 5 { 0 } else { l - 1 };
        let Some((st0, _)) = layer_of(spec, from) else {
            continue;
        };
        let (y0, y1) = (sky.y(st0), sky.y(st));
        if y1 < sky.plot.top() - 1.0 || y1 > sky.horizon || (y0 - y1).abs() < 6.0 * k {
            continue;
        }
        let col = if second {
            ink.voice(ink.hue, true)
        } else {
            ink.voice(ink.hue, false)
        };
        let dx = sky.plot.width() * 0.035;
        for j in 0..14u32 {
            let seed = l as u32 * 97 + j;
            let at = ((0.05 + 0.9 * hash(seed + 7000)) * (n - 1) as f32) as usize;
            let e = view
                .bright(at, from)
                .min(view.bright(at.min(n - 1), l) + 0.3);
            if e < 0.08 {
                continue;
            }
            let u = (view.phase * (0.8 + 0.6 * hash(seed + 7100)) + hash(seed + 7200)).fract();
            let x0 = sky.x(at, n);
            let path = |t: f32| {
                let ease = t * t * (3.0 - 2.0 * t);
                pos2(x0 + dx * t, y0 + (y1 - y0) * ease)
            };
            let life = sin(PI * u);
            let a = e * shim.sqrt() * life * 0.8;
            let tail = 0.3;
            for q in 0..6 {
                let t0 = (u - tail * (q as f32 + 1.0) / 6.0).max(0.0);
                let t1 = (u - tail * q as f32 / 6.0).max(0.0);
                let fade = 1.0 - q as f32 / 6.0;
                b.segment(
                    path(t0),
                    path(t1),
                    1.3 * k,
                    with_alpha(col, a * fade * 0.8),
                    false,
                );
            }
            let head = path(u);
            b.radial(
                Rect::from_center_size(head, Vec2::splat(10.0 * k)),
                head,
                Vec2::splat(4.0 * k),
                &[(0.0, with_alpha(col, 0.5 * a)), (1.0, with_alpha(col, 0.0))],
                4,
            );
            b.dot(head, 0.9 * k, with_alpha(mix(col, c(pal::STAR), 0.5), a));
        }
    }
}

/// Where the light is on average (its pitch weighted by each band's light): a dotted line
/// that rises across the sky as the tail ages — the shimmer climbing, at a glance.
fn centre(b: &mut Brush, k: f32, sky: &Sky, view: &View, spec: &EffectSpec, ink: &Ink) {
    let n = view.len();
    if n < 2 || (1..LAYERS).all(|l| view.peak(l) < 0.02) {
        return;
    }
    let col = mix(ink.voice(ink.hue, false), c(pal::STAR), 0.45);
    let mut last: Option<egui::Pos2> = None;
    for j in (0..n).step_by(3) {
        // Averaged over a fifth of a second either side, so the line is the trend.
        let (mut w, mut sum, mut cnt) = (0.0f32, 0.0f32, 0.0f32);
        for jj in j.saturating_sub(6)..(j + 7).min(n) {
            cnt += 1.0;
            for l in 0..LAYERS {
                if let Some((st, _)) = layer_of(spec, l) {
                    let e = view.bright(jj, l);
                    w += e;
                    sum += e * st;
                }
            }
        }
        if w < 0.05 * cnt {
            last = None;
            continue;
        }
        let p = pos2(sky.x(j, n), sky.y(sum / w));
        if let Some(q) = last {
            if (j / 3) % 2 == 0 {
                b.segment(
                    q,
                    p,
                    1.0 * k,
                    with_alpha(col, 0.55 * (w / cnt).min(1.0)),
                    false,
                );
            }
        }
        last = Some(p);
    }
}

/// The now: a faint line up the sky, the input's peak a short bar of light from the
/// horizon.
fn now_mark(b: &mut Brush, k: f32, sky: &Sky, now: &Reading, ink: &Ink) {
    let x = sky.plot.right();
    canvas::dashed(
        b,
        pos2(x, sky.plot.top() - 8.0 * k),
        pos2(x, sky.horizon),
        0.6 * k,
        with_alpha(ink.text, 0.25),
        2.0 * k,
        4.0 * k,
    );
    let peak = (1.0 + cantus_dsp::math::log10(now.input.max(1e-6)) / 4.0).clamp(0.0, 1.0);
    if peak > 0.0 {
        b.vline(
            x,
            sky.horizon - 22.0 * k * peak,
            sky.horizon,
            2.0 * k,
            with_alpha(ink.text, 0.9),
        );
    }
}

/// The words in the glass: its title and the space in figures (top left), the endless
/// states (top right), the time's two ends (along the water).
fn words(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    g: Rect,
    sky: &Sky,
    now: &Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let left = g.left() + 10.0 * k;
    // On frosted glass the words sit on a plate.
    let frosted = spec.get("freeze") >= 0.5 || now.freeze > 0.01;
    let plate = frosted.then(|| with_alpha(ink.ground, 0.85));
    canvas::put(
        b,
        pos2(left, g.top() + 8.0 * k),
        Align2::LEFT_TOP,
        &Text::key("caelum.d.sky", Role::Micro, ink.dim),
        k,
        plate,
    );
    let inf = spec.get("inf") >= 0.5;
    let decay = if inf {
        tr("caelum.d.infinite").to_string()
    } else {
        format!("{:.1} s", spec.get("decay"))
    };
    let at = pos2(left, g.top() + 24.0 * k);
    if now.seen {
        canvas::putf(
            b,
            at,
            Align2::LEFT_TOP,
            Role::Micro,
            ink.text,
            k,
            plate,
            format_args!(
                "{} {decay} \u{b7} {} {:.0} ms \u{b7} {} {:.0} ms",
                tr("caelum.l.decay"),
                tr("caelum.l.pre"),
                now.predelay_ms,
                tr("caelum.d.lines"),
                now.line_ms,
            ),
        );
    } else {
        // At rest the engine says nothing: the settings as they stand.
        canvas::putf(
            b,
            at,
            Align2::LEFT_TOP,
            Role::Micro,
            ink.text,
            k,
            plate,
            format_args!(
                "{} {decay} \u{b7} {} {:.0} ms",
                tr("caelum.l.decay"),
                tr("caelum.l.pre"),
                spec.get("pre"),
            ),
        );
    }
    let mut x = g.right() - 10.0 * k;
    let spark = ink.voice(ink.hue, true);
    if spec.get("freeze") >= 0.5 || now.freeze > 0.01 {
        let ice = c(pal::ICE);
        let r = canvas::put(
            b,
            pos2(x, g.top() + 8.0 * k),
            Align2::RIGHT_TOP,
            &Text::key("caelum.d.frozen", Role::Micro, ice),
            k,
            Some(with_alpha(ink.ground, 0.8)),
        );
        art::crystal(
            b,
            pos2(r.left() - 10.0 * k, r.center().y),
            5.0 * k,
            0.8 * k,
            ice,
        );
        x = r.left() - 24.0 * k;
    }
    if inf {
        let r = canvas::put(
            b,
            pos2(x, g.top() + 8.0 * k),
            Align2::RIGHT_TOP,
            &Text::key("caelum.d.infinite", Role::Micro, spark),
            k,
            None,
        );
        art::infinity(
            b,
            pos2(r.left() - 13.0 * k, r.center().y),
            16.0 * k,
            1.2 * k,
            spark,
        );
    }
    let foot = g.bottom() - 4.0 * k;
    for (x, align, key) in [
        (sky.plot.left(), Align2::LEFT_BOTTOM, "caelum.d.ago"),
        (sky.plot.right(), Align2::RIGHT_BOTTOM, "caelum.d.now"),
    ] {
        canvas::put(
            b,
            pos2(x, foot),
            align,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            plate,
        );
    }
}

/// The scales engraved on the frame: a tick each second along the foot (longer at the
/// ends and the middle), a tick at each band along the left rail.
fn rails(b: &mut Brush, s: &Style, fr: Rect, g: Rect, sky: &Sky, spec: &EffectSpec, view: &View) {
    let k = s.k;
    let foot = (g.bottom() + fr.bottom()) * 0.5;
    for sec in 0..=8 {
        let x = sky.plot.right() - sky.plot.width() * sec as f32 / 8.0;
        let long = sec % 4 == 0;
        let h = if long { 4.0 } else { 2.5 } * k;
        art::engrave(b, s, pos2(x, foot - h), pos2(x, foot + h), 0.8 * k);
    }
    let rail = (g.left() + fr.left()) * 0.5;
    for l in 0..LAYERS {
        let Some((st, _)) = layer_of(spec, l) else {
            continue;
        };
        if l > 0 && view.peak(l) < 0.02 {
            continue;
        }
        let y = sky.y(st);
        if y < g.top() || y > sky.horizon {
            continue;
        }
        let h = if l == 0 { 4.0 } else { 2.5 } * k;
        art::engrave(b, s, pos2(rail - h, y), pos2(rail + h, y), 0.8 * k);
    }
}

/// The ducking: a slot in the right rail lit red from the top as far as the reduction
/// reaches, its figure in the glass beside it.
fn ducking(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    fr: Rect,
    g: Rect,
    now: &Reading,
    spec: &EffectSpec,
) {
    let range = spec.get("duck");
    if range <= 0.0 {
        return;
    }
    let k = s.k;
    let x = (g.right() + fr.right()) * 0.5;
    let slot = Rect::from_min_max(
        pos2(x - 2.0 * k, g.top() + 30.0 * k),
        pos2(x + 2.0 * k, g.top() + g.height() * 0.55),
    );
    b.rrect(slot.expand(0.8 * k), 2.0 * k, Fill::Solid(c(pal::GASKET)));
    let d = (now.duck_db / range.max(1.0)).clamp(0.0, 1.0);
    if d > 0.0 {
        let lit = Rect::from_min_max(slot.min, pos2(slot.right(), slot.top() + slot.height() * d));
        b.rrect(
            lit,
            1.5 * k,
            Fill::V(ink.danger, with_alpha(ink.danger, 0.7)),
        );
    }
    canvas::tag(
        b,
        pos2(g.right() - 8.0 * k, slot.top()),
        Align2::RIGHT_TOP,
        tr("caelum.d.ducking"),
        ink.danger,
        k,
        Some(with_alpha(ink.ground, 0.85)),
        format_args!("\u{2212}{:.1} dB", now.duck_db),
    );
}
