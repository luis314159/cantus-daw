//! A drum kit's stands as real hardware (round 40 polish, the owner's "púlelo de una": the
//! stands read thin and flat beside the detailed drums): double-braced tripods — each leg
//! and each brace a pair of tubes — on rubber feet with their contact shadows; height tubes
//! in sections, each joint a clamp with its memory lock and wing bolt; ratchet tilters; the
//! cymbal's felts and cup washer; the hi-hat's seat, clutch, spring housings and heel plate.
//! Proportioned as a heavy-duty stand is (a 25.4 mm base tube, 22 and 19 mm sections, 16 mm
//! leg pairs), every chrome tube with the key's streak along it.
//!
//! Only the kit's stands use these; a microphone's tripod stays the single-braced one
//! (`kit::tripod`), as a mic stand is.

use super::geo::{v, Camera, V3};
use super::kit::glint;
use super::prims::{tube, Ctx, TubeOpts, OWN, OWN_BUTT, OWN_NS, OWN_NS_BUTT};
use crate::kit::skin::scene::mat;

/// How much thicker a stand's tubes are drawn than they are: a picture a few hundred
/// points wide shows a 25 mm tube four pixels across, too thin to read as round chrome
/// beside the drums (an illustrator's licence, as a product shot's lens gives it).
pub const THICK: f32 = 1.45;

/// A chrome tube from `a` to `b`, radius `r`, with the key's streak along it.
pub fn chrome(c: &mut Ctx, a: V3, b: V3, r: f32, o: TubeOpts) {
    tube(
        c,
        a,
        b,
        r * THICK,
        &mat::CHROME,
        TubeOpts { bands: true, ..o },
    );
    if !c.mirror {
        glint(c, a, b, r);
    }
}

/// A black part (a clamp, a foot, a knob) from `a` to `b`, radius `r`.
pub fn black(c: &mut Ctx, a: V3, b: V3, r: f32, o: TubeOpts) {
    tube(c, a, b, r, &mat::BLACK, o);
}

/// The horizontal direction from `at` toward the eye turned `turn` degrees (where a wing
/// bolt points: toward the camera, a little aside, as a drummer sets them).
pub fn toward_eye(c: &Ctx, at: V3, turn: f32) -> V3 {
    let e = c.cam.eye() - at;
    let h = v(e.x, 0.0, e.z).norm();
    let (s, co) = turn.to_radians().sin_cos();
    v(h.x * co - h.z * s, 0.0, h.x * s + h.z * co).norm()
}

/// A wing bolt (a T-screw) out of a clamp at `at`: its threaded stem along `out`, its
/// butterfly across `up`, `k` its size (1 a stand's).
pub fn wing_bolt(c: &mut Ctx, at: V3, out: V3, up: V3, k: f32) {
    let end = at + out * (1.5 * k);
    chrome(c, at, end, 0.28 * k, OWN_NS);
    black(
        c,
        end - out * (0.1 * k),
        end + out * (0.7 * k),
        0.55 * k,
        OWN_NS_BUTT,
    );
    let m = end + out * (0.35 * k);
    black(
        c,
        m - up * (1.45 * k),
        m + up * (1.45 * k),
        0.38 * k,
        OWN_NS,
    );
}

/// A clamp round a tube along `ax` at `at` (its middle), `r` the tube's radius: a black
/// collar, a chrome lip at its top, the memory lock under it, its wing bolt toward `out`.
pub fn clamp(c: &mut Ctx, at: V3, ax: V3, r: f32, out: V3) {
    let r = r * THICK;
    let h = 3.2;
    black(
        c,
        at - ax * (h * 0.5),
        at + ax * (h * 0.5),
        r + 0.55,
        OWN_BUTT,
    );
    chrome(
        c,
        at + ax * (h * 0.5 - 0.1),
        at + ax * (h * 0.5 + 0.25),
        (r + 0.35) / THICK,
        OWN_NS_BUTT,
    );
    // The memory lock: a ring that keeps the height when the stand is folded.
    let m = at - ax * (h * 0.5 + 0.9);
    black(c, m - ax * 0.55, m + ax * 0.55, r + 0.42, OWN_NS_BUTT);
    chrome(c, m + out * (r + 0.3), m + out * (r + 1.0), 0.22, OWN_NS);
    let side = ax.cross(out).norm();
    let up = if side.y.abs() > 0.5 { side } else { ax };
    wing_bolt(c, at + out * (r + 0.45), out, up, 1.25);
}

/// A ratchet tilter at `at`: a toothed disc on axis `side`, its wing bolt out along `side`.
pub fn ratchet(c: &mut Ctx, at: V3, side: V3, r: f32) {
    black(c, at - side * 1.1, at + side * 1.1, r, OWN_BUTT);
    chrome(c, at + side * 1.0, at + side * 1.35, r * 0.82, OWN_NS_BUTT);
    let up = if side.y.abs() < 0.9 {
        v(0.0, 1.0, 0.0)
    } else {
        v(1.0, 0.0, 0.0)
    };
    wing_bolt(c, at + side * 1.35, side, up, 0.95);
}

/// A double-braced tripod round `base`: its collar `collar_h` up, its feet `spread` out,
/// turned `rot` degrees. Each leg a pair of 16 mm tubes ending in a rubber foot; each brace
/// a pair from the lower collar to the leg's middle, riveted to it; both collars with their
/// wing bolts.
pub fn tripod(c: &mut Ctx, base: V3, collar_h: f32, spread: f32, rot: f32) {
    let (bx, bz) = (base.x, base.z);
    let top = v(bx, collar_h, bz);
    let low = v(bx, collar_h * 0.40, bz);
    let t = THICK;
    for k in 0..3 {
        let a = (rot + 120.0 * k as f32 + 90.0).to_radians();
        let (sa, ca) = a.sin_cos();
        let radial = v(ca, 0.0, sa);
        let side = v(-sa, 0.0, ca);
        let foot = v(bx + spread * ca, 1.4, bz + spread * sa);
        let attach = top + radial * (1.7 * t) - v(0.0, 0.6, 0.0);
        let dir = (foot - attach).norm();
        let knee = foot - dir * 5.0;
        for sg in [-1.0f32, 1.0] {
            let o = side * (0.95 * THICK * sg);
            chrome(c, attach + o, knee + o, 0.6, OWN);
        }
        // The rubber foot: wide and flat, the pair's ends in it.
        black(c, knee - dir * 0.8, foot, 1.45 * t, OWN);
        black(c, foot - dir * 0.6, foot + dir * 0.4, 1.7 * t, OWN_NS_BUTT);
        c.contacts.push((foot, 3.4 * t));
        // The double brace, riveted to the leg's middle.
        let mid = attach + (foot - attach) * 0.56;
        let from = low + radial * (1.5 * t);
        for sg in [-1.0f32, 1.0] {
            let o = side * (0.8 * THICK * sg);
            chrome(c, from + o, mid + o, 0.42, OWN);
        }
        black(
            c,
            mid - side * (1.25 * t),
            mid + side * (1.25 * t),
            0.55 * t,
            OWN_NS,
        );
        // The legs' hinge on the top collar.
        black(
            c,
            attach - side * (1.3 * t),
            attach + side * (1.3 * t),
            0.75 * t,
            OWN_NS,
        );
    }
    let up = v(0.0, 1.0, 0.0);
    let out = toward_eye(c, top, 35.0);
    black(c, top - up * 2.4, top + up * 2.4, 2.05 * t, OWN_BUTT);
    chrome(c, top + up * 2.3, top + up * 2.7, 1.75, OWN_NS_BUTT);
    wing_bolt(c, top + out * (2.05 * t), out, up, 1.2);
    black(c, low - up * 1.6, low + up * 1.6, 1.75 * t, OWN_BUTT);
    wing_bolt(c, low + out * (1.75 * t), out, up, 0.95);
}

/// Felts round a cymbal on its sleeve (`at` the cymbal's centre, `ax` its axis): the cup
/// washer and the felt under it (the felt and wing nut over it are `kit::wing_nut`'s).
pub fn under_felt(c: &mut Ctx, at: V3, ax: V3) {
    tube(
        c,
        at - ax * 1.05,
        at - ax * 0.2,
        1.75,
        &mat::FELT,
        OWN_NS_BUTT,
    );
    chrome(c, at - ax * 1.45, at - ax * 1.05, 2.0, OWN_NS_BUTT);
    black(c, at - ax * 3.6, at - ax * 1.4, 0.75, OWN_NS_BUTT);
}

// ───────────────────────────────────────────── untangling (round 40 polish, 2nd pass)

/// Three legs round a stand, as the planner sees them: the centre on the floor, the height
/// and radius their tops leave the stand at, their feet's reach, the first leg's angle
/// (degrees in the floor's x–z plane), and how tall the stand's upright is (0: none).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Legs {
    pub base: V3,
    pub collar: f32,
    pub inner: f32,
    pub spread: f32,
    pub angle: f32,
    pub upright: f32,
    /// A boom stand may stand a little aside (its boom still reaches its cymbal): the
    /// planner may move its centre up to 22 cm.
    pub movable: bool,
}

impl Legs {
    /// Each leg's top and foot.
    pub fn legs(&self) -> [(V3, V3); 3] {
        std::array::from_fn(|k| {
            let (s, co) = (self.angle + 120.0 * k as f32).to_radians().sin_cos();
            let d = v(co, 0.0, s);
            (
                v(self.base.x, self.collar, self.base.z) + d * self.inner,
                v(self.base.x, 1.0, self.base.z) + d * self.spread,
            )
        })
    }
}

/// Whether the segments `a–b` and `c–d` cross (their ends excepted).
fn cross2(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> bool {
    let o = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let (d1, d2) = (o(c, d, a), o(c, d, b));
    let (d3, d4) = (o(a, b, c), o(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// The shortest distance from `p` to the segment `a–b` (2-D).
fn dist2(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (x, y) = (a.0 + dx * t - p.0, a.1 + dy * t - p.1);
    (x * x + y * y).sqrt()
}

/// Turn and reach each stand's legs so they read as a set-up kit does in a photograph: no
/// leg through another stand's on the floor, as few crossing another's (or its upright) in
/// the picture as can be, no foot on another's — each stand kept as near its own turn and
/// full reach as that allows. `fixed`: what is already on the floor (the kick's spurs, the
/// pedals), as segments. `stands` are placed in order and changed in place.
pub fn untangle(cam: &Camera, stands: &mut [Legs], fixed: &[(V3, V3)]) {
    let flat = |p: V3| (p.x, p.z);
    let scr = |p: V3| {
        let q = cam.p(p);
        (q.x, q.y)
    };
    // Placed so far: segments on the floor and on the screen, feet.
    let mut floor: Vec<((f32, f32), (f32, f32))> =
        fixed.iter().map(|(a, b)| (flat(*a), flat(*b))).collect();
    let mut screen: Vec<((f32, f32), (f32, f32))> =
        fixed.iter().map(|(a, b)| (scr(*a), scr(*b))).collect();
    let mut feet: Vec<(f32, f32)> = Vec::new();
    for st in stands.iter_mut() {
        let home = *st;
        let mut best = (f32::MAX, home);
        let shifts: Vec<(f32, f32)> = if home.movable {
            std::iter::once((0.0, 0.0))
                .chain([12.0f32, 22.0].into_iter().flat_map(|d| {
                    (0..8).map(move |k| {
                        let (s, c) = (k as f32 * 45.0).to_radians().sin_cos();
                        (c * d, s * d)
                    })
                }))
                .collect()
        } else {
            vec![(0.0, 0.0)]
        };
        for &(dx, dz) in &shifts {
            for step in -10..=10 {
                for reach in [1.0f32, 0.85, 0.7, 0.55] {
                    let cand = Legs {
                        base: v(home.base.x + dx, home.base.y, home.base.z + dz),
                        angle: home.angle + 6.0 * step as f32,
                        spread: home.inner + (home.spread - home.inner) * reach,
                        ..home
                    };
                    let mut score = (step as f32).abs() * 0.1
                        + (1.0 - reach) * 6.0
                        + (dx * dx + dz * dz).sqrt() * 0.12;
                    for (top, foot) in cand.legs() {
                        let (ft, ff) = (flat(top), flat(foot));
                        let (st_, sf) = (scr(top), scr(foot));
                        for (a, b) in &floor {
                            if cross2(ft, ff, *a, *b) || dist2(ff, *a, *b) < 4.0 {
                                score += 60.0;
                            }
                        }
                        for (a, b) in &screen {
                            if cross2(st_, sf, *a, *b) {
                                score += 3.0;
                            }
                        }
                        for f in &feet {
                            let d = ((ff.0 - f.0).powi(2) + (ff.1 - f.1).powi(2)).sqrt();
                            if d < 10.0 {
                                score += 8.0;
                            }
                        }
                    }
                    if score < best.0 {
                        best = (score, cand);
                    }
                }
            }
        }
        *st = best.1;
        for (top, foot) in st.legs() {
            floor.push((flat(top), flat(foot)));
            screen.push((scr(top), scr(foot)));
            feet.push(flat(foot));
        }
        if st.upright > 0.0 {
            let b = st.base;
            screen.push((scr(v(b.x, st.collar, b.z)), scr(v(b.x, st.upright, b.z))));
        }
    }
}
