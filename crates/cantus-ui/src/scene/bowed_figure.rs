//! r42:arcus-upper — the figure that holds an Arcus instrument (`bowed_hold`): a stylised
//! musician or an artist's wooden mannequin, seated, posed from the instrument's own pose.
//!
//! **Not uncanny** (the owner, 2026-11-24): the musician is drawn as our dancers are — warm
//! and friendly, a little larger head, eyes closed in the music, a small smile, cheeks
//! warm — never a realistic face or open, empty eyes. The mannequin is the artist's wooden
//! figure every drawing class knows: an egg for a head, no face, ball joints.
//!
//! **The pose**: a violinist or a violist on a chair, the instrument's end on the left
//! collarbone, the jaw's left side on the chinrest, the face turned along the strings, the
//! left hand round the neck in second position; a cellist or a bassist behind the
//! instrument, its back against the chest, the knees either side of the lower bout, the
//! left hand on the neck. The right arm (the bow's) is the live picture's.

use super::bowed::{board_z, nut_y, outline, Bowed, Pose};
use super::bowed_head::grid;
use super::bowed_hold::{elbow, Hold, Holder};
use super::bowed_mesh::{Mesh, Role};
use super::geo::{v, V3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The figure's joints (world cm) and its frame: facing `f`, its right `r`, `down`; the
/// head's frame (`hf` the face's way, `hu` the crown's).
#[derive(Copy, Clone, Debug)]
pub struct Skel {
    pub f: V3,
    pub r: V3,
    pub down: V3,
    pub head: V3,
    pub hf: V3,
    pub hu: V3,
    pub neck: V3,
    pub chest: V3,
    pub pelvis: V3,
    pub ls: V3,
    pub rs: V3,
    pub le: V3,
    pub lw: V3,
    /// Where the left hand holds the neck.
    pub grip: V3,
    pub hips: [V3; 2],
    pub knees: [V3; 2],
    pub ankles: [V3; 2],
    /// The chair's place and its yaw (radians), or none (a cello's or a bass's own).
    pub seat: Option<(V3, f32)>,
    /// The instrument's neck: its way toward the scroll, the fingerboard's up, the side the
    /// left hand comes from; the fingerboard's top over the grip.
    pub nd: V3,
    pub nz: V3,
    pub nx: V3,
    pub top: V3,
}

/// The head's half-sizes: across, up, front to back (a stylised adult's, a touch large).
pub const HEAD: (f32, f32, f32) = (6.7, 9.3, 8.0);

const UP: V3 = V3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// The figure posed for `b` in `pose`.
pub fn skeleton(b: Bowed, pose: &Pose) -> Skel {
    let d = b.dims();
    let down = -UP;
    if d.held {
        let k = d.body / 35.6;
        let t = v(pose.ey.x, 0.0, pose.ey.z).norm();
        let rt = t.cross(UP);
        let f = (t + rt).norm();
        let r = f.cross(UP).norm();
        let chin = pose.at(-4.4 * k, 3.3 * k, d.arch * 0.55 + 2.1 * k);
        let hf = (t * 0.55 + f * 0.45 + down * 0.28).norm();
        let hu = (UP - r * 0.24).norm();
        let hu = (hu - hf * hu.dot(hf)).norm();
        let hr = hf.cross(hu).norm();
        let (a, bb, c) = HEAD;
        let head = chin + hu * (bb * 0.78) - hf * (c * 0.15) + hr * (a * 0.5);
        let neck = head + down * 17.5 - f * 3.5 + r * 4.0;
        let ls = neck - r * 16.0 + down * 3.5;
        let rs = neck + r * 16.0 + down * 3.5;
        let chest = neck + down * 15.0 - f * 2.0;
        let pelvis = chest + down * 27.0 - f * 2.0;
        let grip = pose.at(0.0, nut_y(d) - 5.5 * k, board_z(d, nut_y(d)) - 1.1 * k);
        let lw = grip - pose.ez * (3.8 * k) - pose.ey * (3.0 * k) - pose.ex * (1.2 * k);
        let le = elbow(ls, lw, 28.0, 25.0, down + f * 0.3 + r * 0.35);
        let (hips, knees, ankles) = legs(pelvis, f, r, pelvis.y - 9.0);
        let seat = v(pelvis.x, 0.0, pelvis.z) - f * 6.0;
        return Skel {
            f,
            r,
            down,
            head,
            hf,
            hu,
            neck,
            chest,
            pelvis,
            ls,
            rs,
            le,
            lw,
            grip,
            hips,
            knees,
            ankles,
            seat: Some((seat, f.x.atan2(f.z))),
            nd: pose.ey,
            nz: pose.ez,
            nx: -pose.ex,
            top: grip + pose.ez * (1.35 * k),
        };
    }
    // A cellist or a bassist behind the instrument, facing the hall.
    let k = d.body / 75.7;
    let f = v(0.0, 0.0, 1.0);
    let r = f.cross(UP).norm();
    let seat_h = if b == Bowed::Bass { 74.0 } else { 46.0 };
    let pelvis_h = seat_h + 10.0;
    let chest_h = pelvis_h + 27.0;
    let back = -(d.rib_hi + d.arch_back);
    let base = pose.at(0.0, 0.0, back);
    let y_c = ((chest_h - base.y) / pose.ey.y.max(0.2)).clamp(d.body * 0.45, d.body * 0.97);
    let contact = pose.at(0.0, y_c, back);
    let chest = contact - f * 10.0 + r * 1.0;
    let pelvis = chest + down * 27.0 - f * 3.0;
    let neck = chest + UP * 15.0 - f * 1.0;
    let head = neck + UP * 18.5 + r * 5.0 + f * 1.0;
    let hf = (f + down * 0.22 - r * 0.22).norm();
    let hu = (UP - r * 0.16).norm();
    let hu = (hu - hf * hu.dot(hf)).norm();
    let ls = neck - r * 16.0 + down * 3.5;
    let rs = neck + r * 16.0 + down * 3.5;
    let grip = pose.at(0.0, nut_y(d) - 10.0 * k, board_z(d, nut_y(d)) - 2.2 * k);
    let lw = grip + pose.ex * (4.0 * k) - pose.ez * (3.0 * k) - pose.ey * (4.0 * k);
    let le = elbow(ls, lw, 29.0, 27.0, -r + down * 0.6);
    // The knees either side of the lower bout.
    let half = outline(b, 2)
        .iter()
        .filter(|&&(_, y)| y < d.body * 0.45)
        .fold(0.0f32, |m, &(x, _)| m.max(x.abs()));
    let mut knees = [V3::default(); 2];
    for (i, s) in [1.0f32, -1.0].into_iter().enumerate() {
        knees[i] = pose.at(s * (half + 3.0 * k), d.body * 0.22, -d.rib_lo * 0.4);
    }
    let hips = [pelvis - r * 9.5, pelvis + r * 9.5];
    let ankles = [
        v(knees[0].x - r.x * 4.0, 9.0, knees[0].z + 8.0),
        v(knees[1].x + r.x * 4.0, 9.0, knees[1].z + 8.0),
    ];
    Skel {
        f,
        r,
        down,
        head,
        hf,
        hu,
        neck,
        chest,
        pelvis,
        ls,
        rs,
        le,
        lw,
        grip,
        hips,
        knees,
        ankles,
        seat: None,
        nd: pose.ey,
        nz: pose.ez,
        nx: pose.ex,
        top: grip + pose.ez * (2.6 * k),
    }
}

/// A seated figure's legs from `pelvis`: hips, knees forward, ankles on the floor.
fn legs(pelvis: V3, f: V3, r: V3, _knee_h: f32) -> ([V3; 2], [V3; 2], [V3; 2]) {
    let hips = [pelvis - r * 9.5, pelvis + r * 9.5];
    let knees = [
        hips[0] + f * 42.0 - r * 3.0 + v(0.0, -2.0, 0.0),
        hips[1] + f * 42.0 + r * 3.0 + v(0.0, -2.0, 0.0),
    ];
    let ankles = [
        v(knees[0].x, 9.0, knees[0].z) + f * 5.0,
        v(knees[1].x, 9.0, knees[1].z) + f * 5.0,
    ];
    (hips, knees, ankles)
}

/// The points the camera fits when a figure holds `b`: its head, its shoulders.
pub fn fit(b: Bowed, pose: &Pose) -> Vec<V3> {
    let s = skeleton(b, pose);
    let (a, bb, _) = HEAD;
    let mut p = vec![
        s.head + s.hu * (bb + 4.0),
        s.head - s.hf.cross(s.hu) * (a + 3.0),
        s.head + s.hf.cross(s.hu) * (a + 3.0),
        s.ls - s.r * 8.0,
        s.rs + s.r * 8.0,
    ];
    if b.dims().held {
        // A violinist from the waist up, a little room round them.
        p.extend([
            s.pelvis + s.r * 14.0,
            s.pelvis - s.r * 14.0,
            s.le + s.down * 6.0,
        ]);
    }
    p
}

/// The figure `h` names, posed for `b` in `pose`, added to `m`.
pub fn build(m: &mut Mesh, b: Bowed, pose: &Pose, h: Holder) {
    let s = skeleton(b, pose);
    if let Some((at, yaw)) = s.seat {
        super::bowed_props::chair(m, at, yaw);
    }
    if h.hold == Hold::Mannequin {
        super::bowed_figure_parts::mannequin(m, &s, h.wood);
    } else {
        super::bowed_figure_parts::musician(m, &s, h.skin, h.cloth);
    }
}

/// An ellipsoid round `c` with half-sizes `a` along `e1`, `b` along `e2`, `t` along their
/// normal; or only the part of it between longitudes `lon` and latitudes `lat` (radians;
/// longitude 0 is `e3`'s way, toward `e1` positive).
#[allow(clippy::too_many_arguments)]
pub fn shell(
    m: &mut Mesh,
    role: Role,
    c: V3,
    (e1, e2): (V3, V3),
    (a, b, t): (f32, f32, f32),
    lon: (f32, f32),
    lat: (f32, f32),
) {
    let e3 = e1.cross(e2).norm();
    let full = (lon.1 - lon.0) >= TAU - 1e-3;
    let cols = if full { 28 } else { 18 };
    let rows = 16;
    let at = move |ph: f32, th: f32| {
        c + e1 * (a * ph.cos() * th.sin()) + e2 * (b * ph.sin()) + e3 * (t * ph.cos() * th.cos())
    };
    let p = grid(
        rows,
        cols,
        full,
        |r, k| {
            let ph = lat.0 + (lat.1 - lat.0) * r as f32 / (rows - 1) as f32;
            let den = if full { cols } else { cols - 1 };
            let th = lon.0 + (lon.1 - lon.0) * k as f32 / den as f32;
            at(ph, th)
        },
        move |q| q - c,
        |r, k| [k as f32, r as f32],
    );
    m.patches.push((role, p));
}

/// A whole ellipsoid.
pub fn ball(m: &mut Mesh, role: Role, c: V3, axes: (V3, V3), size: (f32, f32, f32)) {
    shell(m, role, c, axes, size, (-PI, PI), (-FRAC_PI_2, FRAC_PI_2));
}

/// A sphere.
pub fn sphere(m: &mut Mesh, role: Role, c: V3, r: f32) {
    ball(m, role, c, (v(1.0, 0.0, 0.0), UP), (r, r, r));
}

/// A limb from `a` to `b`, `r` thick, rounded at both ends.
pub fn limb(m: &mut Mesh, role: Role, a: V3, b: V3, r: f32) {
    taper(m, role, a, b, r, r);
}

/// A limb from `a` (`ra` thick) to `b` (`rb`), rounded at both ends: a smooth tube (its
/// normals round its axis, so neither picture shows facets), a ball at each end.
pub fn taper(m: &mut Mesh, role: Role, a: V3, b: V3, ra: f32, rb: f32) {
    let ax = b - a;
    let len = ax.len().max(1e-3);
    let u = ax * (1.0 / len);
    let side = if u.y.abs() < 0.9 {
        UP
    } else {
        v(1.0, 0.0, 0.0)
    };
    let p = u.cross(side).norm();
    let q = u.cross(p).norm();
    let rows = 6;
    let cols = 28;
    let at = move |r: usize, c: usize| {
        let t = r as f32 / (rows - 1) as f32;
        let th = TAU * c as f32 / cols as f32;
        let rr = ra + (rb - ra) * t;
        a + u * (len * t) + (p * th.cos() + q * th.sin()) * rr
    };
    let tube = grid(
        rows,
        cols,
        true,
        at,
        move |x| {
            let t = (x - a).dot(u).clamp(0.0, len);
            x - (a + u * t)
        },
        |r, c| [c as f32, r as f32],
    );
    m.patches.push((role, tube));
    sphere(m, role, a, ra);
    sphere(m, role, b, rb);
}
