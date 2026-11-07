//! Volta's face (ADR 0105): light in its narrow V visor — slanted slits that hold every
//! mood fierce or focused: keen bars, a `^` for a confident grin, short tall slits of
//! surprise, thin low ones half asleep, a scan running the whole V for *cool*, slits
//! turned aside when it is shy — and an equalizer in its faceplate when it sings. Light is
//! drawn in fixed colours (never turned by the hue), in three layers: a halo, the glow, a
//! hot core; one fill each across both eyes.

use super::draw::{visor_pts, HeadView};
use super::pal;
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Body;
use crate::rig::draw::{self as d, ribbon_into, Sink};
use crate::rig::face;
use crate::rig::P;
use cantus_core::inst::dancer::Mood;
use cantus_dsp::math;

#[derive(Copy, Clone, Debug, PartialEq)]
enum Slit {
    /// A bar along the visor: its length, thickness, and how far its outer end drops
    /// below the visor's line (negative: rises).
    Bar(f32, f32, f32),
    /// A `^`: a confident grin.
    Chevron,
    /// Off: a thin flat line (a wink, a blink).
    Off,
}

/// One eye's slit: where it sits, the slit's direction (outward along the visor), its
/// upward normal.
#[derive(Copy, Clone)]
struct Eye {
    e: P,
    u: P,
    n: P,
    kind: Slit,
}

/// Each mood its own shape, readable at a small stage: keen bars (neutral), a grin
/// (happy), long thick bars rising hard at the ends (excited), flat lines drooping
/// (calm), short tall blocks (surprised), thin bars sagging at the ends (sleepy), short
/// bars glancing aside (shy); cool has none (a scan runs the visor).
fn slit_of(mood: Mood, b: &Body, pose: &Pose, i: usize) -> Slit {
    if b.wink[i] > 0.55 {
        return Slit::Off;
    }
    if b.squint > 0.45 && mood != Mood::Excited {
        return Slit::Chevron;
    }
    let base = match mood {
        Mood::Happy => return Slit::Chevron,
        Mood::Excited => Slit::Bar(0.066, 0.02, -0.012),
        Mood::Calm => Slit::Bar(0.05, 0.006, 0.012),
        Mood::Surprised => Slit::Bar(0.022, 0.026, 0.0),
        Mood::Sleepy => Slit::Bar(0.05, 0.006, 0.02),
        Mood::Shy => Slit::Bar(0.026, 0.012, 0.006),
        _ => Slit::Bar(0.056, 0.013, 0.0),
    };
    if pose.blink > 0.5 {
        return Slit::Off;
    }
    base
}

fn eye_shape(s: &mut dyn Sink, y: &Eye, q: f32, g: f32) {
    let (e, u, n) = (y.e, y.u, y.n);
    match y.kind {
        Slit::Bar(len, thick, drop) => {
            let (l, t) = (len * 0.5 * q + g, thick * 0.5 + g);
            // The outer end drops by `drop` (down the picture), the inner end rises.
            let (o, i) = (drop * 0.5, -drop * 0.5);
            let pts = [
                (e.0 - u.0 * l - n.0 * t, e.1 - u.1 * l - n.1 * t + i),
                (e.0 + u.0 * l - n.0 * t, e.1 + u.1 * l - n.1 * t + o),
                (e.0 + u.0 * l + n.0 * t, e.1 + u.1 * l + n.1 * t + o),
                (e.0 - u.0 * l + n.0 * t, e.1 - u.1 * l + n.1 * t + i),
            ];
            d::polygon_into(s, &pts);
        }
        Slit::Off => {
            let l = 0.024 * q;
            let pts = [(e.0 - l, e.1 + 0.002), (e.0 + l, e.1 + 0.002)];
            ribbon_into(s, &pts, 0.004 + 2.0 * g);
        }
        Slit::Chevron => {
            let l = 0.03 * q;
            let pts = [
                (e.0 - l, e.1 + 0.008),
                (e.0, e.1 - 0.016),
                (e.0 + l, e.1 + 0.008),
            ];
            ribbon_into(s, &pts, 0.011 + 2.0 * g);
        }
    }
}

fn mid(a: P, z: P) -> P {
    ((a.0 + z.0) * 0.5, (a.1 + z.1) * 0.5)
}

/// The light in the visor of the head `v`.
pub(super) fn leds(c: &mut Canvas, v: &HeadView, b: &Body, pose: &Pose) {
    let f = face::face_of(pose.mood);
    let vis = visor_pts(v);
    let centre = mid(vis[1], vis[4]);
    let look = (b.look.0 * 0.8 + f.look.0).clamp(-1.0, 1.0);
    let q = v.sq;
    let eye = |i: usize| {
        let outer = if i == 0 {
            mid(vis[0], vis[5])
        } else {
            mid(vis[2], vis[3])
        };
        let (dx, dy) = (outer.0 - centre.0, outer.1 - centre.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-5);
        let u = (dx / len, dy / len);
        // The normal that points up.
        let n = if u.0 > 0.0 { (u.1, -u.0) } else { (-u.1, u.0) };
        let k = if pose.mood == Mood::Shy { 0.4 } else { 0.56 };
        let mut e = crate::rig::lerp_p(centre, outer, k);
        e.0 += look * 0.012 * q;
        if pose.mood == Mood::Sleepy {
            e.1 += 0.003;
        }
        Eye {
            e,
            u,
            n,
            kind: slit_of(pose.mood, b, pose, i),
        }
    };
    let eyes = [eye(0), eye(1)];
    let cool = f.shades;
    if cool || pose.mood == Mood::Calm {
        // The whole V glows low.
        let inner: [P; 6] = std::array::from_fn(|k| crate::rig::lerp_p(vis[k], centre, 0.12));
        d::fixed(c, &|s, _| d::polygon_into(s, &inner), pal::GLOW_HALO);
    }
    // The faceplate's equalizer when it sings.
    // Only singing lights the chin: the body's mouth, not the mood's.
    let open = b.mouth;
    let secs = pose.seconds as f32;
    let eq = |s: &mut dyn Sink, g: f32| {
        if open <= 0.12 {
            return;
        }
        let m = (v.c.0 + v.fx, v.c.1 + 0.064);
        for k in 0..5 {
            let wob = math::sin(secs * 9.0 + k as f32 * 1.7).abs();
            let mid = 1.0 - (k as f32 - 2.0).abs() * 0.2;
            let h = 0.004 + 0.018 * open * mid * (0.45 + 0.55 * wob);
            let x = m.0 + (k as f32 - 2.0) * 0.011 * q;
            d::rect_into(s, (x, m.1), 0.006 * q, h, 0.0, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        if !cool {
            for y in &eyes {
                eye_shape(s, y, q, g);
            }
        }
        eq(s, g);
    };
    let halo = if f.star { 0.014 } else { 0.006 };
    d::fixed(c, &|s, _| all(s, halo), pal::GLOW_HALO);
    d::fixed(c, &|s, _| all(s, 0.0), pal::GLOW);
    let hot = |s: &mut dyn Sink, _: f32| {
        if cool {
            return;
        }
        for y in &eyes {
            if let Slit::Bar(l, t, dr) = y.kind {
                if t > 0.006 {
                    let k = if f.star { 0.6 } else { 0.35 };
                    let core = Eye {
                        kind: Slit::Bar(l * 0.75, t * k, dr),
                        ..*y
                    };
                    eye_shape(s, &core, q, 0.0);
                }
            }
        }
    };
    d::fixed(c, &hot, pal::GLOW_HOT);
    if cool {
        // A scan runs the length of the V and back.
        let u = (secs * 0.7).rem_euclid(1.0);
        let t = if u < 0.5 { u * 2.0 } else { 2.0 - u * 2.0 };
        let (a, z) = (mid(vis[0], vis[5]), mid(vis[2], vis[3]));
        let p = if t < 0.5 {
            crate::rig::lerp_p(a, centre, t * 2.0)
        } else {
            crate::rig::lerp_p(centre, z, t * 2.0 - 1.0)
        };
        let scan = |s: &mut dyn Sink, g: f32| d::rect_into(s, p, 0.05 * q, 0.016, 0.0, g);
        d::fixed(c, &|s, _| scan(s, 0.01), pal::GLOW_HALO);
        d::fixed(c, &|s, _| scan(s, 0.004), pal::GLOW);
        d::fixed(c, &|s, _| scan(s, 0.0), pal::GLOW_HOT);
    }
    if f.star {
        // Excited: a flare off the outer end of each slit.
        let flare = |s: &mut dyn Sink, _: f32| {
            for y in &eyes {
                let o = (y.e.0 + y.u.0 * 0.03 * q, y.e.1 + y.u.1 * 0.03 * q);
                let z = (o.0 + y.u.0 * 0.02, o.1 + y.u.1 * 0.02 - 0.01);
                ribbon_into(s, &[o, z], 0.004);
            }
        };
        d::fixed(c, &flare, pal::GLOW_HOT);
    }
}
