//! The family's faces (ADR 0090): every [`Mood`] as numbers ([`Face`]), big glossy eyes
//! on a turning ball with lids (a blink, a wink, a sleepy droop, a smile's squint), cheeks,
//! sunglasses for *cool*, and a cartoon mouth that smiles, opens to sing and rounds in
//! surprise. A character gives its own [`FaceStyle`] (where and how big, its colours);
//! the songbird keeps its beak, the others use [`mouth`].
//!
//! Drawn layer by layer across both eyes — whites, pupils, irises, lights, lids, rings —
//! one fill per layer, no strokes.

use super::dance::Body;
use super::draw::{self, ellipse_into, ribbon_into, Ink, Look, Sink};
use super::{fx, Ball, OnBall, P, PI};
use crate::pose::Pose;
use crate::render::Canvas;
use cantus_core::inst::dancer::Mood;
use cantus_dsp::math;

/// What a mood does to a face.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Face {
    /// × the eyes' size.
    pub eye: f32,
    /// The pupil, a share of the eye.
    pub pupil: f32,
    /// How far the upper lids close (0…1) and the lower lids rise in a smile.
    pub upper: f32,
    pub lower: f32,
    /// Shut as happy arcs instead of eyes.
    pub arcs: bool,
    pub look: P,
    pub mouth: f32,
    /// How much the mouth smiles (0 flat … 1 a big grin).
    pub smile: f32,
    /// A round "o" mouth (surprise).
    pub round: bool,
    pub cheeks: f32,
    pub shades: bool,
    pub star: bool,
    /// The outer corners of the lids lower (sleepy, > 0) or rise.
    pub droop: f32,
    pub smirk: f32,
}

/// The face of a mood.
pub fn face_of(mood: Mood) -> Face {
    let f = Face {
        eye: 1.0,
        pupil: 0.74,
        upper: 0.08,
        lower: 0.0,
        arcs: false,
        look: (0.0, 0.0),
        mouth: 0.0,
        smile: 0.35,
        round: false,
        cheeks: 0.35,
        shades: false,
        star: false,
        droop: 0.0,
        smirk: 0.0,
    };
    match mood {
        Mood::Neutral => f,
        Mood::Happy => Face {
            upper: 0.02,
            lower: 0.22,
            mouth: 0.22,
            smile: 1.0,
            cheeks: 0.65,
            ..f
        },
        Mood::Excited => Face {
            eye: 1.1,
            pupil: 0.78,
            upper: 0.0,
            lower: 0.12,
            mouth: 0.6,
            smile: 1.0,
            cheeks: 0.8,
            star: true,
            ..f
        },
        Mood::Calm => Face {
            arcs: true,
            smile: 0.7,
            cheeks: 0.5,
            ..f
        },
        Mood::Surprised => Face {
            eye: 1.16,
            pupil: 0.46,
            upper: 0.0,
            mouth: 0.72,
            smile: 0.0,
            round: true,
            cheeks: 0.0,
            ..f
        },
        Mood::Sleepy => Face {
            upper: 0.58,
            look: (0.0, 0.35),
            smile: 0.15,
            cheeks: 0.3,
            droop: 0.5,
            ..f
        },
        Mood::Cool => Face {
            shades: true,
            cheeks: 0.15,
            smirk: 1.0,
            smile: 0.5,
            mouth: 0.05,
            ..f
        },
        Mood::Shy => Face {
            upper: 0.18,
            lower: 0.14,
            look: (0.55, 0.5),
            smile: 0.4,
            cheeks: 1.0,
            droop: -0.25,
            ..f
        },
    }
}

/// Where a character's eyes, cheeks and colours are.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FaceStyle {
    /// The eyes at ± this longitude and this latitude of the ball, this big.
    pub eye_lon: f32,
    pub eye_lat: f32,
    pub eye_rx: f32,
    pub eye_ry: f32,
    pub white: [u8; 4],
    pub pupil: [u8; 4],
    pub iris: [u8; 4],
    /// What closes over an eye (the skin or feathers around it; turned by the hue).
    pub lid: [u8; 4],
    pub ink: Ink,
    pub cheek_lon: f32,
    pub cheek_lat: f32,
    pub cheek_rx: f32,
    pub cheek: [u8; 4],
    /// What the cheeks sit on (a blush is mixed with it: opaque, no gradient).
    pub skin: [u8; 4],
}

/// The lid of an eye (centre `e`, radii `rx`, `ry`): the part of the eye above the line
/// `amount` of the way down (`upper`), or below the curve `amount` of the way up.
fn lid_into(s: &mut dyn Sink, e: P, rx: f32, ry: f32, amount: f32, upper: bool, slant: f32) {
    let a = amount.clamp(0.0, 1.0);
    let edge: [P; 9] = std::array::from_fn(|k| {
        let th = PI * k as f32 / 8.0;
        let (sn, cs) = math::sin_cos(th);
        let y = if upper { -sn } else { sn };
        (e.0 - cs * rx * 1.04, e.1 + y * ry * 1.04)
    });
    let line_y = if upper {
        e.1 - ry + 2.0 * ry * a
    } else {
        e.1 + ry - 2.0 * ry * a
    };
    let bow = if upper { -0.14 } else { -0.4 } * ry * (1.0 - a * 0.5);
    let right = (e.0 + rx * 1.04, line_y - slant * ry);
    let left = (e.0 - rx * 1.04, line_y + slant * ry);
    // Wound the rig's way: the edge then the lid's line back; the polygon helper is not
    // used because the edge is a curve.
    let mut pts = [(0.0, 0.0); 12];
    pts[..9].copy_from_slice(&edge);
    pts[9] = right;
    pts[10] = (e.0, line_y + bow * 2.0);
    pts[11] = left;
    let area: f32 = (0..12)
        .map(|i| {
            let (p, q) = (pts[i], pts[(i + 1) % 12]);
            p.0 * q.1 - q.0 * p.1
        })
        .sum();
    if area < 0.0 {
        pts.reverse();
    }
    draw::spline_into(s, &pts, true);
    s.close();
}

/// The lid's edge (the lash line) as points across the eye.
fn lid_edge(e: P, rx: f32, ry: f32, amount: f32, upper: bool, slant: f32) -> [P; 5] {
    let a = amount.clamp(0.0, 1.0);
    let line_y = if upper {
        e.1 - ry + 2.0 * ry * a
    } else {
        e.1 + ry - 2.0 * ry * a
    };
    let bow = if upper { -0.14 } else { -0.4 } * ry * (1.0 - a * 0.5);
    let yy = ((line_y - e.1) / ry).clamp(-1.0, 1.0);
    let half = rx * (1.0 - yy * yy).max(0.0).sqrt() * 0.98;
    std::array::from_fn(|k| {
        let u = k as f32 / 4.0 * 2.0 - 1.0;
        (
            e.0 + u * half,
            line_y - slant * ry * u * half / rx + bow * 0.8 * (1.0 - u * u),
        )
    })
}

/// The two eyes on `ball` (both drawn layer by layer: one fill per layer).
pub fn eyes(c: &mut Canvas, ball: &Ball, st: &FaceStyle, f: &Face, b: &Body, pose: &Pose) {
    let lat = st.eye_lat + b.nod * 0.5;
    let ons = [
        ball.at(-st.eye_lon, lat, 1.0),
        ball.at(st.eye_lon, lat, 1.0),
    ];
    if f.shades {
        return shades(c, &ons, st);
    }
    let look = (
        (b.look.0 * 0.8 + f.look.0).clamp(-1.0, 1.0),
        (b.look.1 * 0.8 + f.look.1).clamp(-1.0, 1.0),
    );
    let w = st.ink.width * 0.62;
    // Each eye: centre, radii, how shut, how far the lower lid rose, its slant.
    let mut eye = [None; 2];
    for (i, on) in ons.iter().enumerate() {
        if on.z <= 0.08 {
            continue;
        }
        let sq = 0.35 + 0.65 * on.squeeze;
        let rx = st.eye_rx * f.eye * sq;
        let ry = st.eye_ry * f.eye;
        let shut = (f.upper + pose.blink * (1.0 - f.upper))
            .max(b.wink[i])
            .clamp(0.0, 1.0);
        let lower = (f.lower + b.squint * 0.28).min(0.5);
        let outer: f32 = if i == 0 { -1.0 } else { 1.0 };
        eye[i] = Some((
            on.at,
            rx,
            ry,
            shut,
            lower,
            f.droop * outer * 0.35,
            on.squeeze,
        ));
    }
    let open =
        |e: &Option<(P, f32, f32, f32, f32, f32, f32)>| e.filter(|e| !(f.arcs || e.3 > 0.94));
    // Shut eyes: a happy arc when content, a soft lash line in a blink.
    let shut_lines = |s: &mut dyn Sink, _: f32| {
        for (e, rx, _, shut, ..) in eye.iter().flatten() {
            if f.arcs || *shut > 0.94 {
                let k = if f.arcs { -0.045 } else { 0.022 } * st.eye_ry / 0.1;
                ribbon_into(
                    s,
                    &[
                        (e.0 - rx, e.1 + 0.01),
                        (e.0, e.1 + k),
                        (e.0 + rx, e.1 + 0.01),
                    ],
                    0.024,
                );
            }
        }
    };
    draw::fixed(c, &shut_lines, st.ink.color);
    let pupil_of = |e: P, rx: f32, ry: f32| {
        let pr = rx.min(ry) * f.pupil / f.eye.max(0.5);
        let pc = (
            e.0 + look.0 * (rx - pr) * 0.9,
            e.1 + look.1 * (ry - pr) * 0.8 + ry * 0.1,
        );
        (pc, pr)
    };
    draw::fixed(
        c,
        &|s, _| {
            for (e, rx, ry, ..) in eye.iter().filter_map(open) {
                ellipse_into(s, e, rx, ry, 0.0);
            }
        },
        st.white,
    );
    draw::fixed(
        c,
        &|s, _| {
            for (e, rx, ry, .., sq) in eye.iter().filter_map(open) {
                let (pc, pr) = pupil_of(e, rx, ry);
                ellipse_into(s, pc, pr * (0.55 + 0.45 * sq), pr * 1.08, 0.0);
            }
        },
        st.pupil,
    );
    draw::fixed(
        c,
        &|s, _| {
            for (e, rx, ry, .., sq) in eye.iter().filter_map(open) {
                let (pc, pr) = pupil_of(e, rx, ry);
                let prx = pr * (0.55 + 0.45 * sq);
                ellipse_into(s, (pc.0, pc.1 + pr * 0.52), prx * 0.62, pr * 0.42, 0.0);
            }
        },
        st.iris,
    );
    draw::fixed(
        c,
        &|s, _| {
            for (e, rx, ry, ..) in eye.iter().filter_map(open) {
                let (pc, pr) = pupil_of(e, rx, ry);
                if !f.star {
                    ellipse_into(
                        s,
                        (pc.0 - pr * 0.32, pc.1 - pr * 0.36),
                        pr * 0.34,
                        pr * 0.4,
                        -20.0,
                    );
                }
                ellipse_into(
                    s,
                    (pc.0 + pr * 0.38, pc.1 + pr * 0.36),
                    pr * 0.15,
                    pr * 0.15,
                    0.0,
                );
            }
        },
        st.white,
    );
    if f.star {
        for (e, rx, ry, ..) in eye.iter().filter_map(open) {
            let (pc, pr) = pupil_of(e, rx, ry);
            let hl = (pc.0 - pr * 0.32, pc.1 - pr * 0.36);
            fx::sparkle(
                c,
                hl,
                pr * 1.3,
                0.5,
                st.white,
                &Ink {
                    width: 0.0,
                    ..st.ink
                },
            );
        }
    }
    // Lids: the skin over the white, a lash line along their edge.
    draw::flat(
        c,
        &|s, _| {
            for (e, rx, ry, shut, _, slant, _) in eye.iter().filter_map(open) {
                if shut > 0.14 {
                    lid_into(s, e, rx, ry, shut, true, slant);
                }
            }
        },
        st.lid,
    );
    draw::fixed(
        c,
        &|s, _| {
            for (e, rx, ry, shut, _, slant, _) in eye.iter().filter_map(open) {
                if shut > 0.14 {
                    ribbon_into(s, &lid_edge(e, rx, ry, shut, true, slant), 0.02);
                }
            }
        },
        st.ink.color,
    );
    draw::flat(
        c,
        &|s, _| {
            for (e, rx, ry, _, lower, ..) in eye.iter().filter_map(open) {
                if lower > 0.02 {
                    lid_into(s, e, rx, ry, lower, false, 0.0);
                }
            }
        },
        st.lid,
    );
    draw::fixed(
        c,
        &|s, _| {
            for (e, rx, ry, _, lower, ..) in eye.iter().filter_map(open) {
                if lower > 0.02 {
                    ribbon_into(s, &lid_edge(e, rx, ry, lower, false, 0.0), 0.014);
                }
            }
        },
        draw::mix(st.ink.color, st.lid, 0.3),
    );
    // The ring: each eye grown by the ink and nudged down-right, less the eye.
    draw::fixed_even_odd(
        c,
        &|s, _| {
            for (e, rx, ry, ..) in eye.iter().filter_map(open) {
                ellipse_into(
                    s,
                    (e.0 + 0.08 * w, e.1 + 0.18 * w),
                    rx + 0.6 * w,
                    ry + 0.6 * w,
                    0.0,
                );
                ellipse_into(s, e, rx, ry, 0.0);
            }
        },
        st.ink.color,
    );
}

/// Sunglasses over both eyes (cool): dark glossy lenses, a bridge, a glint.
fn shades(c: &mut Canvas, ons: &[OnBall; 2], st: &FaceStyle) {
    let (l, r) = (ons[0], ons[1]);
    let sx = st.eye_rx / 0.082;
    let sy = st.eye_ry / 0.1;
    let lenses = |s: &mut dyn Sink, g: f32| {
        for on in [l, r] {
            if on.z > 0.05 {
                let sq = (0.35 + 0.65 * on.squeeze) * sx;
                draw::blob_grown_into(
                    s,
                    &[
                        (on.at.0 - 0.1 * sq, on.at.1 - 0.06 * sy),
                        (on.at.0 + 0.1 * sq, on.at.1 - 0.065 * sy),
                        (on.at.0 + 0.085 * sq, on.at.1 + 0.05 * sy),
                        (on.at.0, on.at.1 + 0.075 * sy),
                        (on.at.0 - 0.085 * sq, on.at.1 + 0.05 * sy),
                    ],
                    g,
                );
            }
        }
        if l.z > 0.05 && r.z > 0.05 {
            let a = (l.at.0 + 0.09 * sx, l.at.1 - 0.045 * sy);
            let z = (r.at.0 - 0.09 * sx, r.at.1 - 0.045 * sy);
            let m = ((a.0 + z.0) * 0.5, a.1.min(z.1) - 0.02);
            ribbon_into(s, &[a, m, z], 0.03 + 2.0 * g);
        }
    };
    let glass = Look {
        base: [44, 40, 64, 255],
        shade: [22, 18, 34, 255],
        light: [104, 98, 140, 255],
        rim: [0, 0, 0, 0],
    };
    draw::outline(c, &lenses, &st.ink);
    // Glass is never turned by the hue.
    let saved = c.hue;
    c.hue = 0.0;
    draw::bands(c, &lenses, &glass, 3);
    c.hue = saved;
    draw::fixed(
        c,
        &|s, _| {
            for on in [l, r] {
                if on.z > 0.05 {
                    let g0 = (on.at.0 - 0.05 * sx, on.at.1 + 0.02 * sy);
                    let g1 = (on.at.0, on.at.1 - 0.045 * sy);
                    ribbon_into(s, &[g0, g1], 0.018);
                }
            }
        },
        [190, 188, 214, 255],
    );
}

/// Cheeks: cel blushes (two opaque ovals mixed with the skin), and lines when shy.
pub fn cheeks(c: &mut Canvas, ball: &Ball, st: &FaceStyle, f: &Face, b: &Body, pose: &Pose) {
    if f.cheeks <= 0.0 {
        return;
    }
    let k = f.cheeks.min(1.0);
    let lat = st.cheek_lat + b.nod * 0.5;
    let ons = [
        ball.at(-st.cheek_lon, lat, 1.0),
        ball.at(st.cheek_lon, lat, 1.0),
    ];
    let pink = [st.cheek[0], st.cheek[1], st.cheek[2], 255];
    let rx = |on: &OnBall| st.cheek_rx * (0.4 + 0.6 * on.squeeze);
    let ry = st.cheek_rx * 0.6;
    draw::flat(
        c,
        &|s, _| {
            for on in ons.iter().filter(|o| o.z > 0.12) {
                ellipse_into(s, on.at, rx(on), ry, 0.0);
            }
        },
        draw::mix(st.skin, pink, 0.3 + 0.25 * k),
    );
    draw::flat(
        c,
        &|s, _| {
            for on in ons.iter().filter(|o| o.z > 0.12) {
                ellipse_into(
                    s,
                    (on.at.0 - rx(on) * 0.12, on.at.1 - ry * 0.14),
                    rx(on) * 0.62,
                    ry * 0.62,
                    0.0,
                );
            }
        },
        draw::mix(st.skin, pink, 0.5 + 0.35 * k),
    );
    if pose.mood == Mood::Shy {
        draw::flat(
            c,
            &|s, _| {
                for on in ons.iter().filter(|o| o.z > 0.12) {
                    for kx in [-1.0, 0.0, 1.0] {
                        let a = (on.at.0 + kx * 0.022 - 0.008, on.at.1 + 0.012);
                        let z = (on.at.0 + kx * 0.022 + 0.008, on.at.1 - 0.012);
                        ribbon_into(s, &[a, z], 0.008);
                    }
                }
            },
            draw::mix(st.skin, st.ink.color, 0.5),
        );
    }
}

/// What a mouth looks like inside.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MouthStyle {
    pub width: f32,
    pub inside: [u8; 4],
    pub tongue: [u8; 4],
    pub ink: Ink,
}

/// A cartoon mouth at `on` (the ball's point there): a smile line when closed, a D-shaped
/// open mouth with a tongue when singing, a round "o" in surprise, a smirk when cool.
pub fn mouth(c: &mut Canvas, on: &OnBall, f: &Face, open: f32, ms: &MouthStyle) {
    if on.z <= 0.05 {
        return;
    }
    let o = open.max(f.mouth).clamp(0.0, 1.0);
    let sq = 0.45 + 0.55 * on.squeeze;
    let w = ms.width * sq;
    let at = on.at;
    let up = 0.6 * ms.width * f.smile;
    let tilt = 0.25 * ms.width * f.smirk;
    if o < 0.08 {
        // Closed: a smile line, its corners up, one higher in a smirk.
        let pts = [
            (at.0 - w, at.1 - up - tilt),
            (at.0 - w * 0.45, at.1 + up * 0.2),
            (at.0, at.1 + up * 0.35),
            (at.0 + w * 0.45, at.1 + up * 0.2),
            (at.0 + w, at.1 - up + tilt),
        ];
        draw::fixed(
            c,
            &|s, _| ribbon_into(s, &pts, ms.ink.width * 0.8),
            ms.ink.color,
        );
        return;
    }
    let h = ms.width * (0.25 + 0.9 * o);
    let shape = |s: &mut dyn Sink, g: f32| {
        if f.round {
            ellipse_into(s, (at.0, at.1 + h * 0.3), w * 0.45 + g, h * 0.55 + g, 0.0);
        } else {
            draw::blob_grown_into(
                s,
                &[
                    (at.0 - w, at.1 - up * 0.5 - tilt),
                    (at.0, at.1 - up * 0.1),
                    (at.0 + w, at.1 - up * 0.5 + tilt),
                    (at.0 + w * 0.55, at.1 + h * 0.7),
                    (at.0, at.1 + h),
                    (at.0 - w * 0.55, at.1 + h * 0.7),
                ],
                g,
            );
        }
    };
    draw::outline(c, &shape, &ms.ink.thin(0.8));
    draw::fixed(c, &shape, ms.inside);
    let tc = (at.0, at.1 + h * if f.round { 0.62 } else { 0.72 });
    let tw = w * if f.round { 0.28 } else { 0.5 };
    draw::fixed(c, &|s, _| ellipse_into(s, tc, tw, h * 0.22, 0.0), ms.tongue);
}
