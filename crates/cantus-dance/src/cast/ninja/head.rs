//! Fuga's head (ADR 0105), in the head's frame: the ponytail streaming behind, the face
//! hard-shaded with a violet rim, the hair round the back, narrow amber eyes with a heavy
//! winged lash line and a glint under thick angled brows, the fitted mask over the lower
//! face, and the spiky hair swept back with sharp bangs and side locks.

use super::pal;
use crate::cast::edge::{self, poly_into};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Body;
use crate::rig::draw::{self as d, ellipse_into, Sink};
use crate::rig::face::{self, Face};
use crate::rig::{self, Ball, P, PI};
use cantus_core::inst::dancer::Mood;
use cantus_dsp::math;

const EYE_LON: f32 = 33.0;
const EYE_LAT: f32 = -12.0;
/// An eye's half-width facing us.
const EYE_HW: f32 = 0.035;

/// Which way the wind takes what streams from it: −1 to the viewer's left (and whenever
/// it faces us), 1 to the right when it faces left.
pub(super) fn stream_dir(yaw: f32) -> f32 {
    let away = -rig::sin_deg(yaw);
    rig::lerp(-1.0, 1.0, rig::smooth((away - 0.2) / 0.5))
}

/// The visible part (view longitudes, −90…90) of the band of longitudes `lon0`…`lon0 +
/// span` on a ball turned by `yaw`.
fn visible_span(lon0: f32, span: f32, yaw: f32) -> Option<(f32, f32)> {
    for shift in [-360.0, 0.0, 360.0] {
        let a = lon0 + yaw + shift;
        let (lo, hi) = (a.max(-90.0), (a + span).min(90.0));
        if hi > lo + 1.0 {
            return Some((lo, hi));
        }
    }
    None
}

/// A point on the face clamped to the side that faces us.
fn on_face(head: &Ball, lon: f32, lat: f32) -> P {
    let l = (lon + head.yaw).clamp(-86.0, 86.0) - head.yaw;
    head.at(l, lat, 1.0).at
}

/// A point at view longitude `vl` (already turned) and latitude `lat`.
fn at_view(head: &Ball, vl: f32, lat: f32, out: f32) -> P {
    head.at(vl - head.yaw, lat, out).at
}

/// A strip round the head between latitudes `lo` and `hi`, its edges raised toward the
/// front by `−dip` degrees, on the side that faces us.
fn band_into(s: &mut dyn Sink, head: &Ball, lo: f32, hi: f32, dip: f32, out: f32, g: f32) {
    let mut pts = [(0.0, 0.0); 18];
    for k in 0..9 {
        let vl = -90.0 + 22.5 * k as f32;
        let dl = dip * rig::cos_deg(vl);
        let gl = g / head.ry * 57.3;
        pts[k] = at_view(head, vl, hi - dl + gl, out + g * 4.0);
        pts[17 - k] = at_view(head, vl, lo - dl - gl, out + g * 4.0);
    }
    d::polygon_into(s, &pts);
}

/// The hair round the sides and back of the head (longitudes 78…282), where it shows.
fn back_hair_into(s: &mut dyn Sink, head: &Ball) {
    let Some((lo, hi)) = visible_span(70.0, 220.0, head.yaw) else {
        return;
    };
    let mut pts = [(0.0, 0.0); 16];
    for k in 0..8 {
        let lat = -54.0 + 20.0 * k as f32;
        pts[k] = at_view(head, lo, lat, 1.01);
        pts[15 - k] = at_view(head, hi, lat, 1.01);
    }
    d::polygon_into(s, &pts);
}

/// Spikes over the back of the head and a spiky nape (each a sharp lock rooted on the
/// ball, its tip swept back and out), the ones whose root shows; grown by `g`.
fn back_spikes_into(s: &mut dyn Sink, head: &Ball, dir: f32, g: f32) {
    // Locks hanging down from the crown, row under row, the lowest the nape.
    const ROWS: [(f32, f32, f32); 4] = [
        (70.0, 1.1, -34.0),
        (36.0, 1.12, -32.0),
        (2.0, 1.14, -30.0),
        (-30.0, 1.2, -30.0),
    ];
    for (row, &(lat, out, rise)) in ROWS.iter().enumerate() {
        for k in 0..7 {
            let lon = 90.0 + 30.0 * k as f32 + 15.0 * (row % 2) as f32;
            let root = head.at(lon, lat, 1.0);
            if root.z < 0.05 {
                continue;
            }
            let a = head.at(lon - 13.0, lat, 1.0 + g * 6.0).at;
            let z = head.at(lon + 13.0, lat, 1.0 + g * 6.0).at;
            let mut tip = head.at(lon, lat + rise, out + g * 6.0).at;
            tip.0 += dir * 0.025;
            poly_into(s, &[a, tip, z], 0.0);
        }
    }
}

/// The crown: the top of the head cut into sharp spikes swept with the wind, its lower
/// edge the hairline round the face.
fn crown_into(s: &mut dyn Sink, head: &Ball, dir: f32, g: f32) {
    let mut pts = [(0.0, 0.0); 24];
    let mut n = 0;
    for k in 0..17 {
        let a = PI * 1.06 + PI * 0.88 * k as f32 / 16.0;
        let (sn, cs) = math::sin_cos(a);
        let spike = k % 2 == 1;
        let r = if spike {
            1.4 - 0.12 * (k as f32 - 8.0).abs() / 8.0
        } else {
            1.02
        };
        let (x, y) = (cs * (head.rx + g) * r, sn * (head.ry + g) * r);
        let sweep = if spike { dir * 0.04 } else { 0.0 };
        pts[n] = (head.center.0 + x + sweep, head.center.1 + y);
        n += 1;
    }
    // The hairline, right to left, across the forehead.
    for k in 0..5 {
        let vl = 80.0 - 40.0 * k as f32;
        let lat = 30.0 + 6.0 * (1.0 - rig::cos_deg(vl).abs()) - g / head.ry * 57.3;
        pts[n] = at_view(head, vl, lat, 1.02);
        n += 1;
    }
    d::polygon_into(s, &pts[..n]);
}

/// Sharp bangs over the brow, swept with the wind.
fn bangs_into(s: &mut dyn Sink, head: &Ball, dir: f32, g: f32) {
    if head.at(0.0, 30.0, 1.0).z < 0.1 {
        return;
    }
    for (lon, len) in [
        (-58.0, 16.0),
        (-34.0, 22.0),
        (-10.0, 28.0),
        (14.0, 22.0),
        (38.0, 18.0),
        (60.0, 12.0),
    ] {
        let a = on_face(head, lon - 13.0, 40.0);
        let z = on_face(head, lon + 13.0, 40.0);
        let tip = on_face(head, lon + dir * 8.0, 34.0 - len);
        poly_into(s, &[a, z, tip], g);
    }
}

/// The locks framing the face: the ones on the side toward us (`near`) or away.
fn locks_into(s: &mut dyn Sink, head: &Ball, near: bool, g: f32) {
    for side in [-1.0f32, 1.0] {
        let mid = head.at(side * 92.0, 4.0, 1.07);
        if (mid.z > -0.05) != near {
            continue;
        }
        let a = head.at(side * 72.0, 34.0, 1.02).at;
        let z = head.at(side * 90.0, 30.0, 1.05).at;
        let tip = head.at(side * 84.0, -40.0, 1.04).at;
        poly_into(s, &[a, z, tip], g);
    }
}

/// One eye: its centre, half-width (foreshortened), how far the upper lid came down and
/// the lower one rose, its side (−1 the viewer's left).
#[derive(Copy, Clone)]
struct Eye {
    e: P,
    hw: f32,
    shut: f32,
    lower: f32,
    side: f32,
    squeeze: f32,
}

/// What a mood does to the brows: the inner end rises, the outer end rises, the arch
/// (each × 0.01). Low and angled in: intent.
fn brows_of(mood: Mood, i: usize) -> (f32, f32, f32) {
    match mood {
        Mood::Neutral => (-1.4, 0.6, 0.0),
        Mood::Happy => (-0.5, 0.7, 0.3),
        Mood::Excited => (-0.2, 1.1, 0.3),
        Mood::Calm => (-0.4, 0.0, 0.2),
        Mood::Surprised => (1.3, 1.2, 0.6),
        Mood::Sleepy => (-0.8, -0.8, 0.0),
        Mood::Cool if i == 0 => (-1.6, 0.1, 0.0),
        Mood::Cool => (0.2, 1.3, 0.4),
        Mood::Shy => (0.8, -0.6, 0.0),
    }
}

/// The almond of an eye: its inner corner, the upper lid (higher toward the raised outer
/// corner), the outer corner, the lower lid back; 10 points.
fn almond(y: &Eye, f: &Face, grow: f32) -> [P; 10] {
    let s = y.side;
    let hw = y.hw + grow;
    let inner = (y.e.0 - s * hw, y.e.1 + 0.004);
    let outer = (y.e.0 + s * hw, y.e.1 - 0.01);
    let up = (0.02 * f.eye * (1.0 - 0.9 * y.shut)).max(0.002) + grow;
    let lo = 0.008 * f.eye * (1.0 - 1.8 * y.lower) + grow;
    let along = |t: f32, bump: f32| {
        let p = rig::lerp_p(inner, outer, t);
        let k = math::sin(PI * math::powf(t, 0.75));
        (p.0, p.1 + bump * k)
    };
    [
        inner,
        along(0.18, -up),
        along(0.42, -up),
        along(0.66, -up),
        along(0.86, -up * 0.75),
        outer,
        along(0.8, lo),
        along(0.56, lo),
        along(0.32, lo),
        along(0.12, lo * 0.7),
    ]
}

/// Narrow amber eyes and the brows on `head` for the face `f`.
fn eyes(c: &mut Canvas, head: &Ball, f: &Face, b: &Body, pose: &Pose) {
    let lat = EYE_LAT + b.nod * 0.5;
    let look = (
        (b.look.0 * 0.8 + f.look.0).clamp(-1.0, 1.0),
        (b.look.1 * 0.8 + f.look.1).clamp(-1.0, 1.0),
    );
    // Intent: excitement narrows the eyes rather than sparkling them.
    let upper = if f.star { f.upper.max(0.12) } else { f.upper };
    let mut eye: [Option<Eye>; 2] = [None; 2];
    for (i, o) in eye.iter_mut().enumerate() {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let on = head.at(side * EYE_LON, lat, 1.0);
        if on.z <= 0.1 {
            continue;
        }
        let sq = 0.35 + 0.65 * on.squeeze;
        *o = Some(Eye {
            e: on.at,
            hw: EYE_HW * f.eye.min(1.1) * sq,
            shut: (upper + pose.blink * (1.0 - upper))
                .max(b.wink[i])
                .clamp(0.0, 1.0),
            lower: (f.lower + b.squint * 0.28).min(0.5),
            side,
            squeeze: on.squeeze,
        });
    }
    let closed = |y: &Eye| f.arcs || y.shut > 0.92;
    let each_open = |s: &mut dyn Sink, f2: &dyn Fn(&mut dyn Sink, &Eye)| {
        for y in eye.iter().flatten() {
            if !closed(y) {
                f2(s, y);
            }
        }
    };
    let iris = |y: &Eye| {
        let k = (f.pupil / 0.74).clamp(0.6, 1.1);
        let irx = 0.014 * f.eye * k * (0.35 + 0.65 * y.squeeze);
        let iry = 0.017 * f.eye * k;
        let pc = (y.e.0 + look.0 * y.hw * 0.45, y.e.1 + look.1 * 0.005 - 0.003);
        (pc, irx, iry)
    };
    let white = |s: &mut dyn Sink, y: &Eye| d::blob_into(s, &almond(y, f, 0.0));
    d::fixed(c, &|s, _| each_open(s, &white), pal::EYE_WHITE);
    let dark = |s: &mut dyn Sink, y: &Eye| {
        let (pc, irx, iry) = iris(y);
        ellipse_into(s, pc, irx, iry, 0.0);
    };
    d::fixed(c, &|s, _| each_open(s, &dark), pal::IRIS_DARK);
    let lit = |s: &mut dyn Sink, y: &Eye| {
        let (pc, irx, iry) = iris(y);
        ellipse_into(s, (pc.0, pc.1 + iry * 0.32), irx * 0.8, iry * 0.58, 0.0);
    };
    d::fixed(c, &|s, _| each_open(s, &lit), pal::IRIS);
    let pupil = |s: &mut dyn Sink, y: &Eye| {
        let (pc, irx, iry) = iris(y);
        ellipse_into(s, (pc.0, pc.1 + iry * 0.06), irx * 0.32, iry * 0.62, 0.0);
    };
    d::fixed(c, &|s, _| each_open(s, &pupil), pal::PUPIL);
    // A glint: one sharp diamond high in the iris.
    let glint = |s: &mut dyn Sink, y: &Eye| {
        let (pc, irx, iry) = iris(y);
        let (x, yy) = (pc.0 - irx * 0.35, pc.1 - iry * 0.4);
        let r = irx * if f.star { 0.75 } else { 0.5 };
        d::polygon_into(
            s,
            &[
                (x, yy - r),
                (x + r * 0.5, yy),
                (x, yy + r),
                (x - r * 0.5, yy),
            ],
        );
    };
    d::fixed(c, &|s, _| each_open(s, &glint), pal::EYE_WHITE);
    // The lids clip the iris: skin round each almond (an oval less the almond).
    let lids = |s: &mut dyn Sink, y: &Eye| {
        ellipse_into(s, y.e, y.hw * 1.3, 0.03, 0.0);
        d::blob_into(s, &almond(y, f, 0.0));
    };
    d::flat_even_odd(c, &|s, _| each_open(s, &lids), pal::SKIN.base);
    let winking = b.wink.iter().any(|w| *w > 0.9) && pose.blink < 0.5;
    d::fixed(c, &|s, _| lashes_into(s, &eye, f, winking), pal::LASH);
    // The brows: thick at the inner end, tapering out, low over the eyes.
    let brows = |s: &mut dyn Sink, _: f32| {
        for (i, y) in eye.iter().enumerate() {
            let Some(y) = y else { continue };
            let (inner, outer, arch) = brows_of(pose.mood, i);
            let u = 0.01;
            let base = y.e.1 - 0.032;
            let w = EYE_HW * 1.2 * (0.35 + 0.65 * y.squeeze);
            let a = (y.e.0 - y.side * w * 0.95, base - inner * u + 0.004);
            let z = (y.e.0 + y.side * w * 1.15, base - outer * u - 0.006);
            let m = ((a.0 + z.0) * 0.5, (a.1 + z.1) * 0.5 - arch * u - 0.003);
            d::taper_into(s, &[a, m, z], 0.024, 0.008);
        }
    };
    d::flat(c, &brows, pal::HAIR.base);
}

/// The lash lines: heavy along the upper lid and winged past the outer corner, fine under
/// the outer half; for shut eyes a hard line, a wink's arc, calm's closed curve.
fn lashes_into(s: &mut dyn Sink, eye: &[Option<Eye>; 2], f: &Face, winking: bool) {
    for y in eye.iter().flatten() {
        let (e, hw, side) = (y.e, y.hw, y.side);
        let inner = (e.0 - side * hw, e.1 + 0.004);
        let outer = (e.0 + side * hw, e.1 - 0.01);
        if f.arcs || y.shut > 0.92 {
            let k = if f.arcs {
                0.008
            } else if winking {
                -0.01
            } else {
                0.004
            };
            let m = ((inner.0 + outer.0) * 0.5, (inner.1 + outer.1) * 0.5 + k);
            let wing = (outer.0 + side * 0.014, outer.1 - 0.006);
            d::taper_into(s, &[inner, m, outer, wing], 0.01, 0.005);
            continue;
        }
        let a = almond(y, f, 0.0);
        let wing = (outer.0 + side * 0.02, outer.1 - 0.012);
        d::taper_into(s, &[a[0], a[1], a[2], a[3], a[4], a[5]], 0.008, 0.015);
        d::taper_into(s, &[a[5], wing], 0.015, 0.004);
        d::taper_into(s, &[a[7], a[6], a[5]], 0.003, 0.006);
    }
}

/// The fitted mask over the nose and the lower face, its top edge raised over the bridge
/// of the nose.
fn draw_mask(c: &mut Canvas, head: &Ball, lat: f32) {
    if head.at(0.0, -30.0, 1.0).z < -0.3 {
        return;
    }
    let chin = on_face(head, 0.0, -80.0);
    let mask = |s: &mut dyn Sink, g: f32| {
        band_into(s, head, -92.0, -24.0 + lat, -9.0, 1.02, g);
        let jaw = [
            on_face(head, -84.0, -30.0),
            on_face(head, -50.0, -68.0),
            (chin.0, head.center.1 + head.ry * 1.1),
            on_face(head, 50.0, -68.0),
            on_face(head, 84.0, -30.0),
            on_face(head, 0.0, -34.0),
        ];
        d::blob_grown_into(s, &jaw, g);
    };
    edge::part(c, &mask, &pal::MASK, &pal::OUTLINE, true);
}

/// The head, in the head's frame.
pub(super) fn draw_head(c: &mut Canvas, head: &Ball, b: &Body, pose: &Pose) {
    let dir = stream_dir(head.yaw);

    let far = |s: &mut dyn Sink, g: f32| locks_into(s, head, false, g);
    edge::part(c, &far, &pal::HAIR, &pal::OUTLINE, false);
    // The skull and a tapering jaw to a pointed chin (it turns with the face).
    let skull = |s: &mut dyn Sink, g: f32| {
        ellipse_into(s, head.center, head.rx + g, head.ry + g, 0.0);
        if head.at(0.0, 0.0, 1.0).z > -0.5 {
            let chin = on_face(head, 0.0, -80.0);
            let jaw = [
                on_face(head, -80.0, -18.0),
                on_face(head, -48.0, -64.0),
                (chin.0, head.center.1 + head.ry * 1.1),
                on_face(head, 48.0, -64.0),
                on_face(head, 80.0, -18.0),
                (head.center.0, head.center.1 - head.ry * 0.2),
            ];
            d::blob_grown_into(s, &jaw, g);
        }
    };
    edge::part(c, &skull, &pal::SKIN, &pal::OUTLINE, true);
    d::flat(c, &|s, _| back_hair_into(s, head), pal::HAIR.shade);
    let spikes = |s: &mut dyn Sink, g: f32| back_spikes_into(s, head, dir, g);
    d::outline(c, &spikes, &pal::OUTLINE.thin(0.7));
    d::flat(c, &spikes, pal::HAIR.base);
    let mut f = face::face_of(pose.mood);
    f.shades = false;
    if pose.mood == Mood::Cool {
        f.upper = 0.4;
    }
    if head.at(0.0, 0.0, 1.0).z > -0.6 {
        let lat = b.nod * 0.5;
        eyes(c, head, &f, b, pose);
        draw_mask(c, head, lat);
    }
    // The hair: the crown, the bangs, the near locks, bouncing late.
    let bob = tiny_skia::Transform::from_translate(b.top * 0.0005, -b.top.abs() * 0.0004);
    let saved = c.transform;
    c.transform = saved.pre_concat(bob);
    let piece = |k: usize, s: &mut dyn Sink, g: f32| match k {
        0 => crown_into(s, head, dir, g),
        1 => bangs_into(s, head, dir, g),
        _ => locks_into(s, head, true, g),
    };
    let hair = |s: &mut dyn Sink, g: f32| {
        for k in 0..3 {
            piece(k, s, g);
        }
    };
    d::outline(c, &hair, &pal::OUTLINE);
    edge::hard(c, &|s, g| piece(0, s, g), &pal::HAIR, true);
    edge::hard(c, &|s, g| piece(1, s, g), &pal::HAIR, false);
    edge::hard(c, &|s, g| piece(2, s, g), &pal::HAIR, false);
    c.transform = saved;
}
