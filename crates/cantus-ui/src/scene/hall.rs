//! r42:arcus-polish3 — the concert hall behind a realistic figure on the stage, in the
//! Drawn picture's own style: the photographs' warm dark hall (ref-04, ref-06, ref-10)
//! drawn as clean soft shapes out of focus — the wood wall's warm dark with its pools of
//! light, the orchestra as soft dark forms with a warm head and an instrument's glow, the
//! stands' and the hall's lights as bokeh discs. Anonymous, procedural, no photograph.
//! (The Photographic picture's is `chorda_scene_px_hall`, the same layout in 3-D.)

use super::display::{lin, rad, Layer, Paint};
use super::figure::concert::Concert;
use super::geo::{v, V3};
use super::prims::Ctx;
use super::rooms::Info;
use crate::kit::skin::scene::C3;
use egui::Pos2;

/// How far behind the subject the back wall stands (cm; the Photographic hall's).
const BACK: f32 = 1300.0;

/// The orchestra's rows: how far behind the subject (cm) and their risers' height, front
/// row first (both pictures').
pub const ROWS: [(f32, f32); 4] = [(330.0, 0.0), (460.0, 25.0), (600.0, 50.0), (750.0, 75.0)];

/// A deterministic 0 … 1 for `i`, `j` (a small integer hash).
fn h(i: i32, j: i32, s: u32) -> f32 {
    let mut x = (i as u32)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add((j as u32).wrapping_mul(0x85EB_CA77))
        .wrapping_add(s.wrapping_mul(0xC2B2_AE3D));
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// The subject's frame: right of it, up, behind it (cm), as the camera sees it.
struct Frame {
    mid: V3,
    toward: V3,
    right: V3,
}

impl Frame {
    fn at(&self, r: f32, y: f32, d: f32) -> V3 {
        v(
            self.mid.x + self.right.x * r - self.toward.x * d,
            y,
            self.mid.z + self.right.z * r - self.toward.z * d,
        )
    }
}

/// A soft disc of `col` round `p` (`r` cm across in the world), its edge feathered over
/// `soft` of its radius, `op` opaque at its middle; `ring` brightens its rim (a lens's
/// bokeh).
#[allow(clippy::too_many_arguments)]
fn disc(c: &mut Ctx, f: &Frame, at: V3, r: f32, col: C3, op: f32, soft: f32, ring: f32) {
    let p = c.sp(at);
    let e = c.sp(at + f.right * r);
    let rr = (e.x - p.x).hypot(e.y - p.y).max(0.5);
    let inner = (1.0 - soft).clamp(0.0, 0.98);
    let g = rad(
        p,
        rr,
        rr,
        &[
            (0.0, col, op),
            (inner * 0.85, col, op * (1.0 + ring * 0.2)),
            (inner, col, op * (1.0 + ring)),
            (1.0, col, 0.0),
        ],
    );
    c.p.ellipse(p, rr, rr, g, 1.0);
}

/// A soft vertical form (a seated player's body) from `a` to `b` (its middle line), `w`
/// cm wide.
fn form(c: &mut Ctx, f: &Frame, a: V3, b: V3, w: f32, col: C3, op: f32) {
    let (pa, pb) = (c.sp(a), c.sp(b));
    let e = c.sp(a + f.right * w);
    let rx = (e.x - pa.x).hypot(e.y - pa.y).max(0.5);
    let ry = ((pb.y - pa.y).abs() * 0.5).max(1.0);
    let ctr = Pos2::new((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5);
    let g = rad(
        ctr,
        rx,
        ry,
        &[(0.0, col, op), (0.6, col, op), (1.0, col, 0.0)],
    );
    c.p.ellipse(ctr, rx, ry, g, 1.0);
}

/// Draw the hall round the subject at `aim` seen from `eye` (cm) in `cc`'s light.
pub fn concert(c: &mut Ctx, aim: V3, eye: V3, cc: &Concert) -> Info {
    let mut t = v(eye.x - aim.x, 0.0, eye.z - aim.z);
    if t.len() < 1.0 {
        t = v(0.0, 0.0, 1.0);
    }
    let toward = t.norm();
    let f = Frame {
        mid: aim,
        toward,
        right: v(toward.z, 0.0, -toward.x),
    };
    let k = cc.d_hall.max(0.0);
    let warm = |col: C3, s: f32| -> C3 { col.map(|x| (x * s * k).min(1.0)) };
    c.p.on(Layer::Far);
    // The wall: a warm dark, a little lighter toward the floor, in panels.
    let (w0, w1) = (-2200.0, 2200.0);
    let q = [
        f.at(w0, -200.0, BACK),
        f.at(w1, -200.0, BACK),
        f.at(w1, 1600.0, BACK),
        f.at(w0, 1600.0, BACK),
    ];
    let sp = c.sps_near(&q);
    let (top, bottom) = (c.sp(f.at(0.0, 900.0, BACK)), c.sp(f.at(0.0, 0.0, BACK)));
    c.p.poly(
        sp,
        lin(
            top,
            bottom,
            &[
                (0.0, warm([0.035, 0.020, 0.013], 1.0)),
                (0.6, warm([0.075, 0.040, 0.022], 1.0)),
                (1.0, warm([0.055, 0.030, 0.018], 1.0)),
            ],
        ),
        1.0,
        0.0,
    );
    let mut x = w0;
    let mut i = 0i32;
    while x < w1 {
        let wd = 70.0 + 30.0 * h(i, 1, 13);
        let s = 0.75 + 0.5 * h(i, 2, 17);
        let q = [
            f.at(x, 0.0, BACK - 2.0),
            f.at(x + wd - 4.0, 0.0, BACK - 2.0),
            f.at(x + wd - 4.0, 1100.0, BACK - 2.0),
            f.at(x, 1100.0, BACK - 2.0),
        ];
        let sp = c.sps_near(&q);
        c.p.poly(
            sp,
            lin(
                top,
                bottom,
                &[
                    (0.0, warm([0.03, 0.017, 0.011], s)),
                    (0.6, warm([0.085, 0.045, 0.024], s)),
                    (1.0, warm([0.06, 0.032, 0.019], s)),
                ],
            ),
            0.55,
            0.0,
        );
        x += wd;
        i += 1;
    }
    // The wall's pools of light.
    let mut wx = -1400.0;
    while wx < 1450.0 {
        disc(
            c,
            &f,
            f.at(wx, 420.0, BACK - 4.0),
            260.0,
            warm([0.55, 0.30, 0.14], 1.0),
            0.16,
            0.9,
            0.0,
        );
        wx += 420.0;
    }
    // The hall's lights high up: bokeh.
    for q in 0..14 {
        let (u, w) = (h(q, 41, 43), h(q, 47, 53));
        let at = f.at(-1500.0 + 3000.0 * u, 450.0 + 400.0 * w, BACK - 15.0);
        let r = 22.0 + 22.0 * h(q, 59, 61);
        disc(
            c,
            &f,
            at,
            r,
            warm([0.80, 0.55, 0.30], 1.0),
            0.22 + 0.25 * h(q, 67, 71),
            0.25,
            0.4,
        );
    }
    // The orchestra, back row first: soft dark forms, a warm head, an instrument's glow,
    // the stands' lamps.
    for (row, &(d, rise)) in ROWS.iter().rev().enumerate() {
        let ri = 3 - row as i32;
        let gap = 105.0 + 10.0 * ri as f32;
        let mut r = -950.0 + 40.0 * h(ri, 0, 23);
        let mut j = 0i32;
        let lit = 0.6 + 0.12 * ri as f32;
        while r < 950.0 {
            let jr = r + (h(ri, j, 29) - 0.5) * 30.0;
            if ri > 0 || jr.abs() > 70.0 {
                let dd = d + 20.0 * h(ri, j, 31);
                let seat = rise + 48.0;
                form(
                    c,
                    &f,
                    f.at(jr, seat, dd),
                    f.at(jr, seat + 62.0, dd),
                    24.0,
                    warm([0.02, 0.016, 0.014], 1.0),
                    0.95,
                );
                let head = f.at(jr, seat + 74.0, dd);
                disc(
                    c,
                    &f,
                    head,
                    12.0,
                    warm([0.36, 0.22, 0.15], lit),
                    0.8,
                    0.45,
                    0.0,
                );
                let cello = ri < 2 && jr > 250.0;
                let (ia, ir) = if cello {
                    (f.at(jr, seat + 20.0, dd - 30.0), 22.0)
                } else {
                    (f.at(jr + 18.0, seat + 56.0, dd - 14.0), 13.0)
                };
                disc(c, &f, ia, ir, warm([0.50, 0.22, 0.08], lit), 0.55, 0.5, 0.0);
                if j % 3 == 0 {
                    let sr = jr + gap * 0.5;
                    let top = rise + 98.0;
                    form(
                        c,
                        &f,
                        f.at(sr, top - 5.0, d - 70.0),
                        f.at(sr, top + 32.0, d - 70.0),
                        27.0,
                        warm([0.012, 0.011, 0.011], 1.0),
                        0.9,
                    );
                    disc(
                        c,
                        &f,
                        f.at(sr, top + 34.0, d - 66.0),
                        9.0,
                        warm([0.85, 0.62, 0.36], 1.0),
                        0.75,
                        0.3,
                        0.5,
                    );
                }
            }
            r += gap;
            j += 1;
        }
    }
    // The stage's floor in front of the orchestra: the dark it sits in.
    let q = [
        f.at(w0, 0.0, -300.0),
        f.at(w1, 0.0, -300.0),
        f.at(w1, 0.0, 200.0),
        f.at(w0, 0.0, 200.0),
    ];
    let sp = c.sps_near(&q);
    c.p.poly(sp, Paint::Solid(warm([0.03, 0.018, 0.012], 1.0)), 1.0, 0.0);
    // The stage's floor in front of the orchestra: warm pools off to the sides.
    for x in [-760.0f32, -430.0, 430.0, 760.0] {
        disc(
            c,
            &f,
            f.at(x, 2.0, 200.0),
            170.0,
            warm([0.30, 0.17, 0.09], 1.0),
            0.5,
            0.9,
            0.0,
        );
    }
    c.p.on(Layer::Objects);
    Info {
        zb: aim.z - BACK,
        gloss: 0.0,
    }
}
