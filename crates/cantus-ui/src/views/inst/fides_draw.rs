//! Fides's picture (a child of `views::inst::fides`): the instrument the settings make,
//! drawn as a luthier's plan — the resonator (a string between its bridge and nut, a bar
//! with its nodes, a tine from its clamp, a plate, a membrane, a tube, a bell, a glass, a
//! pan), the exciter where it plays it (a finger, a pick, a bow, a mallet, noise, a jack),
//! the microphone's point, and under it the partials it rings with: their ratios to the
//! fundamental on a log axis, each as tall as the strike excites it there (the comb a
//! position makes, the shapes of the modes), lit by what the newest voice sounds now.
//!
//! **The display is the control**: a drag across the resonator moves where it is played
//! (`exc.pos`), one undo step per drag; the partials follow as you drag.

use crate::app::CantusApp;
use crate::kit::color::with_alpha;
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{Brush, Style};
use cantus_core::TrackId;
use cantus_dsp::fides::exciter::ExciterKind;
use cantus_dsp::fides::tables::{partials, shape, Partial, ResKind, MAX_MODES};
use cantus_dsp::fides::{tel, FidesSettings};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// Where the resonator lies in the picture: its left end (pos 0) to its right end, and
/// the point at `pos` (0 … ½ from the left, mirrored past the middle for drawing).
fn along(obj: Rect, pos: f32) -> f32 {
    obj.left() + obj.width() * pos.clamp(0.0, 1.0)
}

/// The partials of A as (ratio, how much the strike excites it there, 0 … 1).
pub fn ladder(fs: &FidesSettings, out: &mut [(f32, f32); 24]) -> usize {
    let kind = fs.a.kind;
    let pos = fs.exc.pos as f64;
    if kind.is_guide() {
        // A waveguide's harmonics: a closed tube's odd ones; the comb |sin nπβ| of the
        // position, a pluck's 1/n.
        let odd = kind == ResKind::Tube;
        for (n, o) in out.iter_mut().enumerate() {
            let k = if odd { 2 * n + 1 } else { n + 1 } as f64;
            let comb = (k * std::f64::consts::PI * pos).sin().abs();
            *o = (k as f32, (comb / (1.0 + 0.15 * (k - 1.0))) as f32);
        }
        return out.len();
    }
    let mut table = [Partial {
        ratio: 0.0,
        m: 0,
        n: 0,
    }; MAX_MODES];
    let n = partials(kind, &mut table).min(fs.a.modes).min(out.len());
    let mut peak = 1e-9f64;
    let mut w = [0f64; 24];
    for k in 0..n {
        w[k] = (shape(kind, &table[k], pos) * shape(kind, &table[k], fs.a.listen as f64)).abs();
        peak = peak.max(w[k]);
    }
    let e = 1.0 + 0.12 * fs.a.inharm.clamp(-1.0, 1.0) as f64;
    for k in 0..n {
        out[k] = (table[k].ratio.powf(e) as f32, (w[k] / peak) as f32);
    }
    n
}

/// The ratio's place on the ladder's log axis (1 … `top`).
fn x_of(r: Rect, ratio: f32, top: f32) -> f32 {
    let u = (ratio.max(0.25).ln() - 0.5f32.ln()) / (top.ln() - 0.5f32.ln());
    r.left() + r.width() * u.clamp(0.0, 1.0)
}

/// Paint the picture in `d` and take the drag that moves the position. `tele` is the
/// newest voice's telemetry (zeros with no engine).
#[allow(clippy::too_many_arguments)]
pub fn picture(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    fs: &FidesSettings,
    tele: &cantus_engine::Telemetry,
    d: Rect,
    s: &Style,
) {
    let pad = s.s(10.0);
    let inner = d.shrink(pad);
    let split = inner.top() + inner.height() * 0.6;
    let obj = Rect::from_min_max(inner.min, pos2(inner.right(), split - s.s(8.0)));
    let lad = Rect::from_min_max(pos2(inner.left(), split + s.s(4.0)), inner.max);
    let live = tele[tel::ALIVE] > 0.5 && tele[tel::KEY] >= 0.0;
    let pos = if live { tele[tel::POS] } else { fs.exc.pos };
    let level = if live { tele[tel::LEVEL].clamp(0.0, 1.0) } else { 0.0 };
    // The drag: where it is played, along the resonator (the left half is the pos range).
    let id = egui::Id::new(("fides-pos", track.0));
    let resp = ui.interact(obj, id, Sense::click_and_drag());
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let u = ((p.x - obj.left()) / obj.width()).clamp(0.0, 1.0);
            let v = if u > 0.5 { 1.0 - u } else { u };
            app.set_inst_value(track, "exc.pos", v.clamp(0.01, 0.5), true);
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        app.end_gesture();
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    let ink = s.t.text;
    let dim = s.t.text_dim;
    let hue = super::face::accent_on(s, s.hue);
    let ring = super::face::voice2(s, s.hue);
    crate::kit::with_brush(&ui.painter_at(d), |b| {
        object(b, obj, fs, pos, level, ink, dim, hue, s);
        exciter(b, obj, fs, pos, hue, ink, s);
        if !fs.a.kind.is_guide() {
            let x = along(obj, fs.a.listen);
            let y = obj.center().y + obj.height() * 0.34;
            b.circle_stroke(pos2(x, y), s.s(4.0), s.s(1.2), ring);
            b.vline(x, y - s.s(10.0), y - s.s(4.0), s.s(1.0), ring);
        }
        partial_ladder(b, lad, fs, tele, live, hue, ring, dim, s);
    });
}

#[allow(clippy::too_many_arguments)]
fn object(
    b: &mut Brush,
    obj: Rect,
    fs: &FidesSettings,
    pos: f32,
    level: f32,
    ink: Color32,
    dim: Color32,
    hue: Color32,
    s: &Style,
) {
    let w = s.s(1.4);
    let c = obj.center();
    let fill = with_alpha(hue, 0.10);
    match fs.a.kind {
        ResKind::String => {
            // Bridge (left), nut (right), the string, displaced as a pluck leaves it.
            let (x0, x1) = (obj.left() + s.s(6.0), obj.right() - s.s(6.0));
            b.rrect(
                Rect::from_center_size(pos2(x0, c.y), vec2(s.s(6.0), s.s(22.0))),
                s.r(1.5),
                crate::kit::Fill::Solid(dim),
            );
            b.rrect(
                Rect::from_center_size(pos2(x1, c.y), vec2(s.s(4.0), s.s(18.0))),
                s.r(1.0),
                crate::kit::Fill::Solid(dim),
            );
            let px = x0 + (x1 - x0) * pos;
            let lift = obj.height() * 0.3 * level.sqrt();
            b.polyline(&[pos2(x0, c.y), pos2(px, c.y - lift), pos2(x1, c.y)], w, ink);
        }
        ResKind::Tube | ResKind::Pipe => {
            let r = Rect::from_center_size(c, vec2(obj.width() * 0.94, obj.height() * 0.34));
            b.rrect(r, r.height() * 0.5, crate::kit::Fill::Solid(fill));
            b.rrect_stroke(r, r.height() * 0.5, w, ink);
            if fs.a.kind == ResKind::Tube {
                // The closed end, on the right.
                let cap = Rect::from_min_max(pos2(r.right() - r.height() * 0.5, r.top()), r.max);
                b.rrect(cap, r.height() * 0.5, crate::kit::Fill::Solid(dim));
            }
        }
        ResKind::Bar | ResKind::Marimba => {
            let r = Rect::from_center_size(c, vec2(obj.width() * 0.94, obj.height() * 0.32));
            b.rrect(r, s.r(3.0), crate::kit::Fill::Solid(fill));
            b.rrect_stroke(r, s.r(3.0), w, ink);
            if fs.a.kind == ResKind::Marimba {
                // The undercut that tunes it.
                let u = Rect::from_center_size(
                    pos2(c.x, r.bottom() - r.height() * 0.2),
                    vec2(r.width() * 0.5, r.height() * 0.4),
                );
                b.rrect_stroke(u, u.height() * 0.5, s.s(1.0), dim);
            }
            // The fundamental's nodes, 0.224 of the length from each end.
            for n in [0.224f32, 0.776] {
                let x = r.left() + r.width() * n;
                b.dashed(
                    &[pos2(x, r.top() - s.s(6.0)), pos2(x, r.bottom() + s.s(6.0))],
                    s.s(1.0),
                    dim,
                    s.s(3.0),
                    s.s(3.0),
                );
            }
        }
        ResKind::Tine => {
            let clamp = Rect::from_min_max(
                pos2(obj.right() - obj.width() * 0.12, obj.top() + obj.height() * 0.2),
                pos2(obj.right(), obj.bottom() - obj.height() * 0.2),
            );
            b.rrect(clamp, s.r(2.0), crate::kit::Fill::Solid(dim));
            let y = c.y;
            let tip = obj.left() + s.s(6.0);
            let bend = obj.height() * 0.25 * level.sqrt();
            b.polyline(
                &[
                    pos2(clamp.left(), y),
                    pos2((clamp.left() + tip) * 0.5, y - bend * 0.3),
                    pos2(tip, y - bend),
                ],
                s.s(4.0),
                ink,
            );
        }
        ResKind::Plate => {
            let r = Rect::from_center_size(c, vec2(obj.width() * 0.94, obj.height() * 0.8));
            b.rrect(r, s.r(2.0), crate::kit::Fill::Solid(fill));
            b.rrect_stroke(r, s.r(2.0), w, ink);
            b.dashed(
                &[
                    pos2(r.left(), r.top() + r.height() * 0.31),
                    pos2(r.left() + r.width() * 0.5, r.top() + r.height() * 0.575),
                ],
                s.s(1.0),
                dim,
                s.s(3.0),
                s.s(3.0),
            );
        }
        ResKind::Membrane | ResKind::Drumhead | ResKind::Pan => {
            let rx = obj.width().min(obj.height() * 2.6) * 0.47;
            let ry = obj.height() * 0.38;
            let pts: Vec<Pos2> = (0..=64)
                .map(|i| {
                    let a = i as f32 / 64.0 * std::f32::consts::TAU;
                    pos2(c.x + rx * a.cos(), c.y + ry * a.sin())
                })
                .collect();
            b.polygon(&pts, fill);
            b.polyline(&pts, w, ink);
            // The radius it is struck along, from the rim (left) to the centre.
            b.dashed(&[pos2(c.x - rx, c.y), c], s.s(1.0), dim, s.s(3.0), s.s(3.0));
            if fs.a.kind == ResKind::Pan {
                let note = Rect::from_center_size(
                    pos2(c.x - rx * 0.45, c.y),
                    vec2(rx * 0.55, ry * 0.7),
                );
                b.rrect_stroke(note, note.height() * 0.5, s.s(1.0), dim);
            }
        }
        ResKind::Bell | ResKind::Glass => bell_or_glass(b, obj, fs.a.kind, ink, fill, s),
    }
}

/// A bell's or a wine glass's outline, mouth down (a bell) or up (a glass).
fn bell_or_glass(b: &mut Brush, obj: Rect, kind: ResKind, ink: Color32, fill: Color32, s: &Style) {
    let c = obj.center();
    let h = obj.height() * 0.9;
    let wmax = (obj.width() * 0.2).min(h * 0.6);
    let (top, bot) = (c.y - h * 0.5, c.y + h * 0.5);
    let mut left = Vec::with_capacity(24);
    for i in 0..=20 {
        let u = i as f32 / 20.0;
        let (y, half) = if kind == ResKind::Bell {
            // The crown narrow, the waist, the lip flaring.
            let half = wmax * (0.42 + 0.22 * u + 0.36 * u.powi(4));
            (top + u * h, half)
        } else {
            // The bowl over the upper two thirds, then the stem.
            let bowl = 0.66;
            if u <= bowl {
                let v = u / bowl;
                (top + u * h, wmax * (1.0 - 0.15 * v * v) * (1.0 - v).max(0.0).sqrt().max(0.12))
            } else {
                (top + u * h, wmax * 0.08)
            }
        };
        left.push(pos2(c.x - half, y));
    }
    let mut pts = left.clone();
    for p in left.iter().rev() {
        pts.push(pos2(2.0 * c.x - p.x, p.y));
    }
    b.polygon(&pts, fill);
    b.polyline(&pts, s.s(1.4), ink);
    if kind == ResKind::Glass {
        b.hline(c.x - wmax * 0.6, c.x + wmax * 0.6, bot, s.s(1.6), ink);
    }
}

/// Where the exciter meets the resonator, for `pos` (0 … ½ from the end the controls
/// name: a string's bridge, a bar's end, a tine's tip, a membrane's rim, a bell's lip).
fn strike_point(obj: Rect, kind: ResKind, pos: f32, s: &Style) -> Pos2 {
    let c = obj.center();
    let p = pos.clamp(0.0, 0.5);
    match kind {
        ResKind::String => {
            let (x0, x1) = (obj.left() + s.s(6.0), obj.right() - s.s(6.0));
            pos2(x0 + (x1 - x0) * p, c.y)
        }
        ResKind::Tine => {
            let tip = obj.left() + s.s(6.0);
            let clamp = obj.right() - obj.width() * 0.12;
            pos2(tip + (clamp - tip) * p, c.y)
        }
        ResKind::Plate => {
            let r = Rect::from_center_size(c, vec2(obj.width() * 0.94, obj.height() * 0.8));
            pos2(r.left() + r.width() * p, r.top() + r.height() * (0.31 + 0.53 * p))
        }
        ResKind::Membrane | ResKind::Drumhead | ResKind::Pan => {
            let rx = obj.width().min(obj.height() * 2.6) * 0.47;
            pos2(c.x - rx + 2.0 * p * rx, c.y)
        }
        ResKind::Bell | ResKind::Glass => {
            let h = obj.height() * 0.9;
            let (top, bot) = (c.y - h * 0.5, c.y + h * 0.5);
            let y = if kind == ResKind::Bell {
                bot - 2.0 * p * h * 0.9
            } else {
                top + 2.0 * p * h * 0.6
            };
            let half = (obj.width() * 0.2).min(h * 0.6);
            pos2(c.x - half * 0.95, y)
        }
        _ => pos2(along(obj, p * 0.94 + 0.03), c.y),
    }
}

/// The exciter's glyph at its point.
#[allow(clippy::too_many_arguments)]
fn exciter(
    b: &mut Brush,
    obj: Rect,
    fs: &FidesSettings,
    pos: f32,
    hue: Color32,
    ink: Color32,
    s: &Style,
) {
    let p = strike_point(obj, fs.a.kind, pos, s);
    let w = s.s(1.6);
    match fs.exc.kind {
        ExciterKind::Pluck => {
            // A fingertip: an arc under the string, pulling.
            b.arc(p + vec2(0.0, s.s(8.0)), s.s(6.0), 3.6, 5.8, s.s(2.4), hue, true);
        }
        ExciterKind::Pick => {
            let t = [
                p + vec2(-s.s(6.0), s.s(14.0)),
                p + vec2(s.s(6.0), s.s(14.0)),
                p + vec2(0.0, s.s(2.0)),
            ];
            b.polygon(&t, hue);
        }
        ExciterKind::Bow => {
            // The bow's hair across the point, its stick above, the frog at the right.
            let a = p + vec2(-s.s(34.0), s.s(10.0));
            let z = p + vec2(s.s(34.0), -s.s(10.0));
            b.segment(a, z, w, hue, true);
            b.segment(a + vec2(0.0, -s.s(5.0)), z + vec2(0.0, -s.s(5.0)), s.s(2.4), ink, true);
            b.rrect(
                Rect::from_center_size(z + vec2(s.s(2.0), -s.s(2.5)), vec2(s.s(6.0), s.s(8.0))),
                s.r(1.5),
                crate::kit::Fill::Solid(hue),
            );
        }
        ExciterKind::Mallet => {
            let head = p + vec2(s.s(6.0), -s.s(9.0));
            b.segment(head, head + vec2(s.s(26.0), -s.s(18.0)), s.s(2.0), ink, true);
            b.circle(head, s.s(5.5), crate::kit::Fill::Solid(hue));
        }
        ExciterKind::Noise => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let u = i as f32 / 24.0;
                    let j = ((i * 7919) % 13) as f32 / 13.0 - 0.5;
                    p + vec2(-s.s(16.0) + u * s.s(32.0), -s.s(12.0) + j * s.s(8.0))
                })
                .collect();
            b.polyline(&pts, s.s(1.2), hue);
        }
        ExciterKind::Input => {
            b.circle_stroke(p + vec2(0.0, -s.s(12.0)), s.s(5.0), s.s(1.6), hue);
            b.segment(p + vec2(0.0, -s.s(7.0)), p, s.s(1.6), hue, true);
        }
    }
    b.circle(p, s.s(2.2), crate::kit::Fill::Solid(hue));
}

/// The partials: each a line at its ratio, as tall as the strike excites it, lit by what
/// sounds; the axis under them in ×1, ×2, ×4 ….
#[allow(clippy::too_many_arguments)]
fn partial_ladder(
    b: &mut Brush,
    r: Rect,
    fs: &FidesSettings,
    tele: &cantus_engine::Telemetry,
    live: bool,
    hue: Color32,
    ring: Color32,
    dim: Color32,
    s: &Style,
) {
    let mut rungs = [(0f32, 0f32); 24];
    let n = ladder(fs, &mut rungs);
    let top = rungs[..n]
        .iter()
        .fold(4.0f32, |m, (ratio, _)| m.max(*ratio))
        .min(64.0)
        * 1.15;
    let axis = r.bottom() - s.s(12.0);
    b.hline(r.left(), r.right(), axis, s.s(1.0), dim);
    let mut mark = 1.0f32;
    while mark <= top {
        let x = x_of(r, mark, top);
        b.vline(x, axis, axis + s.s(3.0), s.s(1.0), dim);
        let txt = format!("×{mark:.0}");
        let lr = Rect::from_center_size(pos2(x, axis + s.s(8.0)), vec2(s.s(30.0), s.s(10.0)));
        let _ = ktext::paint(b, lr, &Text::new(&txt, Role::Micro, dim).align(Align2::CENTER_CENTER), s.tk());
        mark *= 2.0;
    }
    let height = axis - r.top() - s.s(2.0);
    for (k, (ratio, w)) in rungs[..n].iter().enumerate() {
        if *ratio > top {
            continue;
        }
        let x = x_of(r, *ratio, top);
        let h = height * w.clamp(0.04, 1.0);
        b.vline(x, axis - h, axis, s.s(2.0), with_alpha(hue, 0.55));
        if live && k < tel::MODE_COUNT && !fs.a.kind.is_guide() {
            let lit = (tele[tel::MODES + k] * 40.0).clamp(0.0, 1.0);
            if lit > 0.02 {
                b.vline(x, axis - h * lit, axis, s.s(2.0), ring);
            }
        }
    }
}

/// The inspector's thumbnail: the resonator and the exciter, small, without the partials.
pub fn thumbnail(b: &mut Brush, r: Rect, fs: &FidesSettings, s: &Style) {
    let hue = super::face::accent_on(s, s.hue);
    object(b, r, fs, fs.exc.pos, 0.0, s.t.text, s.t.text_dim, hue, s);
    exciter(b, r, fs, fs.exc.pos, hue, s.t.text, s);
}
