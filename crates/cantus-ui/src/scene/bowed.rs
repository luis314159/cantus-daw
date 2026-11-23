//! r42:arcus-look — Arcus's cello and double bass for the scene engine (ADR 0126's third
//! and fourth subjects, after Chorda's pianos and Tympanum's kit): the instruments' measures
//! from luthiers' published figures, the outline drawn as a luthier draws it — the lower
//! bout, the C-bout and the upper bout as three curves meeting at the corners — the plates'
//! arching, the pose (standing on the endpin, leant back as a player holds it) and the
//! strings' places that the live picture (the sounding string, the finger, the bow) is
//! drawn on. `bowed_mesh` builds the surfaces from these, `bowed_draw` draws them as the
//! engine's projected vectors and `views/inst/chorda_scene_px_bowed.rs` bakes them per pixel.
//!
//! **Sources** (measures are facts; no drawing is copied): the cello's body, bouts and
//! string length are the Stradivari "Davidov" (1712, the B form) as its published copies
//! give them — 757 mm long, 340 / 232 / 436 mm across the upper, middle and lower bouts, a
//! 690 mm string; its low, flat arching (about 25 mm) and its purfling 4 mm from the edge
//! from R. Hargrave, "Stradivari's 'Davidov' cello", *The Strad*, December 2001. The double
//! bass is a 3/4 orchestral bass as makers publish it (body 1 090 – 1 110 mm; bouts about
//! 500 / 370 / 650 mm; ribs about 200 mm; a 1 060 mm string, Arcus's own table). The
//! fittings (bridge, fingerboard, neck, tailpiece, afterlength, scroll) are the standard
//! set-up figures of the trade. Local frame: x across (to the viewer's right with the top
//! toward them), y along the body from its lower edge (0) to its upper edge, z out of the
//! top (the ribs' upper edge at z = 0); centimetres.

use super::geo::{v, V3};

/// Which bowed instrument.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Bowed {
    Cello,
    Bass,
}

/// An instrument's measures (cm, degrees).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Dims {
    /// The body's length.
    pub body: f32,
    /// The ribs' height at the lower and the upper end.
    pub rib_lo: f32,
    pub rib_hi: f32,
    /// The top's and the back's arching over the edge.
    pub arch: f32,
    pub arch_back: f32,
    /// A double bass's flat back, bent toward the neck over its upper part.
    pub flat_back: bool,
    /// The top's upper edge to the bridge's line; the nut to the top's upper edge.
    pub stop: f32,
    pub neck: f32,
    /// The bridge's height over the top at its feet, the span of its feet.
    pub bridge_h: f32,
    pub bridge_w: f32,
    /// The fingerboard's length, its width at the nut and at its end.
    pub board: f32,
    pub board_w: (f32, f32),
    /// Between neighbouring strings at the nut and at the bridge.
    pub spacing: (f32, f32),
    /// The strings over the fingerboard's end (the lowest's clearance).
    pub clear: f32,
    /// The fingerboard's top over the top's edge where the neck meets the body (the
    /// overstand and the board): the neck's line runs from there up to the bridge, so the
    /// nut sits behind the top's plane, as on every cello and bass.
    pub overstand: f32,
    /// The bridge to the tailpiece's fret; the tailpiece's length and widths.
    pub after: f32,
    pub tail_len: f32,
    pub tail_w: (f32, f32),
    /// The pegbox's length, the scroll's outer radius, the head's width.
    pub pegbox: f32,
    pub scroll_r: f32,
    pub head_w: f32,
    /// The endpin out of the body.
    pub endpin: f32,
    /// Leant back (the top turned up) and sideways (the scroll toward the viewer's right).
    pub lean: f32,
    pub roll: f32,
    /// The strings' radii, lowest first (C G D A; E A D G).
    pub gauge: [f32; 4],
    /// The f-holes: their length, the upper and the lower eyes' distance from the middle.
    pub f_len: f32,
    pub f_upper: f32,
    pub f_lower: f32,
    /// Geared machines on the pegbox (the bass), not wooden pegs.
    pub machines: bool,
}

pub const CELLO: Dims = Dims {
    body: 75.7,
    rib_lo: 12.2,
    rib_hi: 11.4,
    arch: 2.5,
    arch_back: 2.5,
    flat_back: false,
    stop: 40.0,
    neck: 28.5,
    bridge_h: 8.8,
    bridge_w: 9.0,
    board: 58.0,
    board_w: (2.4, 4.3),
    spacing: (0.72, 1.7),
    clear: 0.75,
    overstand: 3.0,
    after: 11.5,
    tail_len: 21.5,
    tail_w: (2.6, 5.4),
    pegbox: 11.5,
    scroll_r: 4.0,
    head_w: 5.4,
    endpin: 30.0,
    lean: 24.0,
    roll: 16.0,
    gauge: [0.08, 0.066, 0.052, 0.042],
    f_len: 12.8,
    f_upper: 4.7,
    f_lower: 10.2,
    machines: false,
};

pub const BASS: Dims = Dims {
    body: 110.0,
    rib_lo: 20.0,
    rib_hi: 18.0,
    arch: 3.0,
    arch_back: 0.6,
    flat_back: true,
    stop: 60.0,
    neck: 45.0,
    bridge_h: 15.5,
    bridge_w: 14.0,
    board: 87.0,
    board_w: (4.4, 8.4),
    spacing: (1.2, 2.9),
    clear: 1.1,
    overstand: 4.6,
    after: 17.0,
    tail_len: 30.0,
    tail_w: (4.0, 8.6),
    pegbox: 19.0,
    scroll_r: 5.6,
    head_w: 7.2,
    endpin: 22.0,
    lean: 12.0,
    roll: 10.0,
    gauge: [0.15, 0.125, 0.105, 0.088],
    f_len: 19.5,
    f_upper: 7.2,
    f_lower: 14.8,
    machines: true,
};

impl Bowed {
    pub fn dims(self) -> &'static Dims {
        match self {
            Bowed::Cello => &CELLO,
            Bowed::Bass => &BASS,
        }
    }
}

/// The half-outline (the viewer's right half, x ≥ 0), mm from the lower edge, as three
/// curves — the lower bout to its corner, the C-bout between the corners, the upper bout
/// from its corner to the neck — each run through its points (centripetal Catmull–Rom),
/// meeting at the corners' tips with the corners' own angle.
type Half = [&'static [(f32, f32)]; 3];

/// The cello (757 mm; 436 / 232 / 340 mm across the bouts).
const CELLO_HALF: Half = [
    &[
        (0.0, 0.0),
        (70.0, 5.0),
        (130.0, 22.0),
        (178.0, 56.0),
        (207.0, 102.0),
        (218.0, 162.0),
        (214.0, 216.0),
        (201.0, 258.0),
        (186.0, 285.0),
        (179.0, 297.0),
        (186.0, 307.0),
    ],
    &[
        (186.0, 307.0),
        (167.0, 314.0),
        (146.0, 329.0),
        (127.0, 353.0),
        (118.0, 377.0),
        (116.0, 396.0),
        (118.0, 417.0),
        (126.0, 439.0),
        (140.0, 461.0),
        (154.0, 477.0),
        (161.0, 486.0),
    ],
    &[
        (161.0, 486.0),
        (154.0, 494.0),
        (154.0, 507.0),
        (160.0, 535.0),
        (168.0, 570.0),
        (170.0, 600.0),
        (166.0, 637.0),
        (153.0, 674.0),
        (130.0, 707.0),
        (97.0, 734.0),
        (55.0, 751.0),
        (0.0, 757.0),
    ],
];

/// The 3/4 double bass (1 100 mm; 650 / 370 / 490 mm across the bouts), its shoulders
/// sloping straight down from the neck's block to the upper bout, as a bass's (the viol's
/// line it keeps).
const BASS_HALF: Half = [
    &[
        (0.0, 0.0),
        (100.0, 7.0),
        (192.0, 31.0),
        (260.0, 77.0),
        (306.0, 142.0),
        (325.0, 232.0),
        (320.0, 312.0),
        (301.0, 372.0),
        (281.0, 412.0),
        (270.0, 430.0),
        (279.0, 444.0),
    ],
    &[
        (279.0, 444.0),
        (254.0, 454.0),
        (225.0, 474.0),
        (201.0, 506.0),
        (189.0, 536.0),
        (185.0, 563.0),
        (188.0, 591.0),
        (198.0, 621.0),
        (213.0, 655.0),
        (223.0, 683.0),
        (229.0, 699.0),
    ],
    &[
        (229.0, 699.0),
        (217.0, 711.0),
        (215.0, 731.0),
        (226.0, 776.0),
        (240.0, 818.0),
        (245.0, 846.0),
        (236.0, 884.0),
        (210.0, 930.0),
        (176.0, 976.0),
        (140.0, 1022.0),
        (102.0, 1061.0),
        (70.0, 1087.0),
        (46.0, 1099.0),
        (0.0, 1100.0),
    ],
];

/// A centripetal Catmull–Rom curve through `p`, `per` points a span (ends included once).
fn spline(p: &[(f32, f32)], per: usize) -> Vec<(f32, f32)> {
    let n = p.len();
    let at = |i: isize| -> (f32, f32) {
        if i < 0 {
            let (a, b) = (p[0], p[1]);
            (2.0 * a.0 - b.0, 2.0 * a.1 - b.1)
        } else if i as usize >= n {
            let (a, b) = (p[n - 1], p[n - 2]);
            (2.0 * a.0 - b.0, 2.0 * a.1 - b.1)
        } else {
            p[i as usize]
        }
    };
    let mut out = Vec::with_capacity(n * per);
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (
            at(i as isize - 1),
            at(i as isize),
            at(i as isize + 1),
            at(i as isize + 2),
        );
        let d = |a: (f32, f32), b: (f32, f32)| ((b.0 - a.0).hypot(b.1 - a.1)).sqrt().max(1e-4);
        let t1 = d(p0, p1);
        let t2 = t1 + d(p1, p2);
        let t3 = t2 + d(p2, p3);
        let lerp = |a: (f32, f32), b: (f32, f32), ta: f32, tb: f32, t: f32| {
            let k = (t - ta) / (tb - ta);
            (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
        };
        for j in 0..per {
            let t = t1 + (t2 - t1) * j as f32 / per as f32;
            let a1 = lerp(p0, p1, 0.0, t1, t);
            let a2 = lerp(p1, p2, t1, t2, t);
            let a3 = lerp(p2, p3, t2, t3, t);
            let b1 = lerp(a1, a2, 0.0, t2, t);
            let b2 = lerp(a2, a3, t1, t3, t);
            out.push(lerp(b1, b2, t1, t2, t));
        }
    }
    out.push(p[n - 1]);
    out
}

/// The body's whole outline (cm), counter-clockwise seen from the top: from the lower
/// edge's middle up the right side to the neck, down the left side; `per` points a span.
pub fn outline(b: Bowed, per: usize) -> Vec<(f32, f32)> {
    let half = match b {
        Bowed::Cello => &CELLO_HALF,
        Bowed::Bass => &BASS_HALF,
    };
    let mut right: Vec<(f32, f32)> = Vec::new();
    for (k, seg) in half.iter().enumerate() {
        let mut s = spline(seg, per);
        if k > 0 {
            s.remove(0);
        }
        right.extend(s);
    }
    let right: Vec<(f32, f32)> = right.iter().map(|&(x, y)| (x / 10.0, y / 10.0)).collect();
    let mut all = right.clone();
    // Down the left side, the two middles not repeated.
    all.extend(
        right[1..right.len() - 1]
            .iter()
            .rev()
            .map(|&(x, y)| (-x, y)),
    );
    all
}

/// The distance from (x, y) to the closed polygon `o`.
pub fn dist(o: &[(f32, f32)], x: f32, y: f32) -> f32 {
    let n = o.len();
    let mut best = f32::MAX;
    for i in 0..n {
        let (a, b) = (o[i], o[(i + 1) % n]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = (dx * dx + dy * dy).max(1e-9);
        let t = (((x - a.0) * dx + (y - a.1) * dy) / l2).clamp(0.0, 1.0);
        let (px, py) = (a.0 + dx * t - x, a.1 + dy * t - y);
        best = best.min(px * px + py * py);
    }
    best.sqrt()
}

/// The plates' arching (cm over the edge plane) at a distance `d` (cm) in from the edge:
/// the edge's thickness, the channel's shallow hollow just inside the purfling, then the
/// arch rising to its plateau about `reach` cm in (the 'Davidov' is low and full).
pub fn arch(height: f32, d: f32, reach: f32) -> f32 {
    let edge = 0.42;
    let t = (d / reach).clamp(0.0, 1.0);
    let rise = (t * std::f32::consts::FRAC_PI_2).sin().powf(1.7);
    let channel = 0.10 * (-((d - 0.95) / 0.45).powi(2)).exp();
    edge + (height - edge * 0.5) * rise - channel
}

/// Where the instrument stands: its local frame in the world (cm) — the endpin's tip on the
/// floor at the origin, the body leant back by `lean` (its top turned up toward the light)
/// and its scroll tipped `roll` toward the viewer's right, as a player holds it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pose {
    pub o: V3,
    pub ex: V3,
    pub ey: V3,
    pub ez: V3,
}

impl Pose {
    pub fn of(b: Bowed) -> Pose {
        let d = b.dims();
        let (sl, cl) = d.lean.to_radians().sin_cos();
        let (sr, cr) = d.roll.to_radians().sin_cos();
        let rot = |p: V3| v(p.x * cr + p.y * sr, -p.x * sr + p.y * cr, p.z);
        let ex = rot(v(1.0, 0.0, 0.0));
        let ey = rot(v(0.0, cl, -sl));
        let ez = rot(v(0.0, sl, cl));
        // The endpin leaves the lower block at the ribs' middle.
        let o = ey * d.endpin + ez * (d.rib_lo * 0.5);
        Pose { o, ex, ey, ez }
    }

    /// A local point in the world.
    pub fn at(&self, x: f32, y: f32, z: f32) -> V3 {
        self.o + self.ex * x + self.ey * y + self.ez * z
    }

    /// A local direction in the world.
    pub fn dir(&self, d: V3) -> V3 {
        self.ex * d.x + self.ey * d.y + self.ez * d.z
    }
}

/// The ribs' height at `y` (a bass's back bends toward the neck over its upper fifth).
pub fn rib(d: &Dims, y: f32) -> f32 {
    let t = (y / d.body).clamp(0.0, 1.0);
    let h = d.rib_lo + (d.rib_hi - d.rib_lo) * t;
    if d.flat_back && t > 0.8 {
        let k = (t - 0.8) / 0.2;
        h * (1.0 - 0.42 * k * k)
    } else {
        h
    }
}

/// How far in from the edge the arch reaches its plateau.
pub fn reach(d: &Dims) -> f32 {
    d.body * 0.12
}

/// The bridge's line (y), the nut's (y), the strings' height over the edge plane at the
/// nut (their middle).
pub fn bridge_y(d: &Dims) -> f32 {
    d.body - d.stop
}

pub fn nut_y(d: &Dims) -> f32 {
    d.body + d.neck
}

fn nut_z(d: &Dims) -> f32 {
    board_z(d, nut_y(d)) + 0.08
}

/// The bridge's top arc: its radius and the strings' heights over the edge plane there.
pub fn bridge_r(d: &Dims) -> f32 {
    d.spacing.1 * 2.5
}

/// String `i`'s place across at the bridge and the nut (lowest at the viewer's right).
pub fn string_x(d: &Dims, i: usize) -> (f32, f32) {
    let k = 1.5 - i as f32;
    (k * d.spacing.1, k * d.spacing.0)
}

/// String `i`'s three points in local cm: on the tailpiece's fret, on the bridge, on the
/// nut.
pub fn string_local(d: &Dims, i: usize) -> [V3; 3] {
    let (xb, xn) = string_x(d, i);
    let r = bridge_r(d);
    let top = d.arch + d.bridge_h;
    let zb = top - (r - (r * r - xb * xb).max(0.0).sqrt()) + d.gauge[i];
    let zn = nut_z(d) + d.gauge[i] - xn * xn / (2.0 * board_r(d));
    let yb = bridge_y(d);
    let yt = yb - d.after;
    let xt = xb * 0.62;
    let zt = tail_z(d, yt) + 0.75;
    [v(xt, yt, zt), v(xb, yb, zb), v(xn, nut_y(d), zn)]
}

/// The tailpiece's underside height over the edge plane along y (its fret end high, its
/// lower end low near the saddle).
pub fn tail_z(d: &Dims, y: f32) -> f32 {
    let yb = bridge_y(d);
    let (y1, y0) = (yb - d.after, yb - d.after - d.tail_len);
    let t = ((y - y0) / (y1 - y0)).clamp(0.0, 1.0);
    let lo = d.arch * 0.35 + 0.5;
    let hi = d.arch + d.bridge_h * 0.32;
    lo + (hi - lo) * t
}

/// The fingerboard's top along y, at its middle (cm over the edge plane): a straight line
/// from the overstand at the neck's joint to just under the strings at the bridge.
pub fn board_z(d: &Dims, y: f32) -> f32 {
    let yb = bridge_y(d);
    let at_bridge = d.arch + d.bridge_h - d.clear * 1.15;
    d.overstand + (at_bridge - d.overstand) * (d.body - y) / (d.body - yb)
}

/// The fingerboard's cross radius (its top is an arc across, flatter than the bridge's).
pub fn board_r(d: &Dims) -> f32 {
    d.board_w.1 * 1.15
}

/// The fingerboard's width at y.
pub fn board_w(d: &Dims, y: f32) -> f32 {
    let (yn, end) = (nut_y(d), nut_y(d) - d.board);
    let t = ((yn - y) / (yn - end)).clamp(0.0, 1.0);
    d.board_w.0 + (d.board_w.1 - d.board_w.0) * t
}

/// What the live picture draws on, in the world: each string's points (fret, bridge,
/// nut), the bridge's arc normal at each string (the bow's tilt to reach it alone), the
/// instrument's frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rig {
    pub strings: [[V3; 3]; 4],
    pub normal: [V3; 4],
    pub gauge: [f32; 4],
    pub pose: Pose,
    /// The open strings' length, cm.
    pub length: f32,
}

pub fn rig(b: Bowed) -> Rig {
    let d = b.dims();
    let pose = Pose::of(b);
    let r = bridge_r(d);
    let top = d.arch + d.bridge_h;
    let mut strings = [[V3::default(); 3]; 4];
    let mut normal = [V3::default(); 4];
    for (i, (s, n)) in strings.iter_mut().zip(normal.iter_mut()).enumerate() {
        let l = string_local(d, i);
        *s = [
            pose.at(l[0].x, l[0].y, l[0].z),
            pose.at(l[1].x, l[1].y, l[1].z),
            pose.at(l[2].x, l[2].y, l[2].z),
        ];
        // From the arc's centre through the string.
        let c = v(0.0, 0.0, top - r);
        let dir = v(l[1].x - c.x, 0.0, l[1].z - c.z).norm();
        *n = pose.dir(dir).norm();
    }
    let l = string_local(d, 2);
    Rig {
        strings,
        normal,
        gauge: d.gauge,
        pose,
        length: (l[2] - l[1]).len(),
    }
}
