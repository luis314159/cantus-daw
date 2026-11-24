//! r42:arcus-upper — the figures' parts (`bowed_figure`): the musician (skin, concert
//! clothes, hair, a calm face of a few strokes) and the artist's wooden mannequin (an egg
//! head, ball joints), both on the same skeleton. The right arm is the live picture's; only
//! its shoulder is here.

use super::bowed_figure::{ball, limb, shell, sphere, Skel, HEAD};
use super::bowed_mesh::{Mesh, Rod, Role};
use super::geo::V3;
use std::f32::consts::{FRAC_PI_2, PI};

/// The musician: `skin` and `cloth` tones.
pub fn musician(m: &mut Mesh, s: &Skel, skin: u8, cloth: u8) {
    let (sk, cl) = (Role::Skin(skin), Role::Cloth(cloth));
    head(m, s, sk);
    // The neck, the collar.
    let chin_under = s.head - s.hu * (HEAD.1 * 0.55);
    limb(m, sk, s.neck + s.f * 0.5, chin_under, 4.2);
    // The torso: chest and waist in the shirt, the hips in the trousers.
    let (r, up) = (s.r, -s.down);
    ball(m, cl, s.chest, (r, up), (15.5, 16.5, 9.8));
    let waist = s.chest.lerp(s.pelvis, 0.55);
    ball(m, cl, waist, (r, up), (13.8, 13.0, 9.2));
    ball(m, Role::Trousers, s.pelvis, (r, up), (15.0, 8.5, 11.0));
    for sh in [s.ls, s.rs] {
        sphere(m, cl, sh, 6.2);
        m.rods.push((
            cl,
            Rod {
                a: sh,
                b: s.neck + (sh - s.neck) * 0.2 + s.down * 3.0,
                r: 6.0,
            },
        ));
    }
    // The left arm: the sleeve to the wrist, the hand round the neck.
    limb(m, cl, s.ls, s.le, 4.6);
    limb(m, cl, s.le, s.lw, 3.9);
    hand(m, s, sk, 1.0);
    // The legs.
    for i in 0..2 {
        limb(m, Role::Trousers, s.hips[i], s.knees[i], 7.0);
        limb(m, Role::Trousers, s.knees[i], s.ankles[i], 5.4);
        let toe = s.ankles[i] + s.f * 9.0;
        ball(
            m,
            Role::Shoes,
            s.ankles[i].lerp(toe, 0.5) + s.down * 4.0,
            (s.r, -s.down),
            (4.6, 3.4, 9.0),
        );
    }
}

/// The head: skin, hair over the crown and the back, ears, a small nose, the eyes closed,
/// brows, a smile, the cheeks' warmth.
fn head(m: &mut Mesh, s: &Skel, sk: Role) {
    let (a, b, c) = HEAD;
    let hr = s.hf.cross(s.hu).norm();
    let axes = (-hr, s.hu);
    ball(m, sk, s.head, axes, (a, b, c));
    // The face's points on the head (longitude toward the head's left, latitude up).
    let on = |th: f32, ph: f32, lift: f32| {
        s.head
            + (-hr) * ((a + lift) * ph.cos() * th.sin())
            + s.hu * ((b + lift) * ph.sin())
            + s.hf * ((c + lift) * ph.cos() * th.cos())
    };
    // Hair: the crown and the back, a soft fringe over the forehead.
    let g = 1.0;
    shell(
        m,
        Role::Hair,
        s.head + s.hu * 0.6,
        axes,
        (a + g, b + g, c + g),
        (-PI, PI),
        (0.32, FRAC_PI_2),
    );
    shell(
        m,
        Role::Hair,
        s.head + s.hu * 0.6,
        axes,
        (a + g, b + g, c + g),
        (1.15, TAU_M),
        (-0.55, 0.4),
    );
    // Ears.
    for sd in [1.0f32, -1.0] {
        let e = s.head + hr * (sd * (a - 0.4)) + s.hu * 0.2 - s.hf * 0.6;
        ball(m, sk, e, (s.hf, s.hu), (1.6, 2.6, 1.0));
    }
    // The nose.
    ball(m, sk, on(0.0, -0.12, 0.4), (hr, s.hu), (1.1, 1.5, 1.2));
    // The eyes closed (calm arcs, their middles lowest), the brows above, the smile.
    for sd in [1.0f32, -1.0] {
        arc(m, &on, sd * 0.36, 0.03, 0.16, -0.05, 0.24);
        arc(m, &on, sd * 0.36, 0.2, 0.18, 0.04, 0.2);
        ball(
            m,
            Role::Blush,
            on(sd * 0.55, -0.16, 0.15),
            (hr, s.hu),
            (1.7, 1.2, 0.35),
        );
    }
    arc(m, &on, 0.0, -0.38, 0.2, -0.06, 0.24);
}

/// Longitudes past the back: from the right ear round the back to the left (radians).
const TAU_M: f32 = 2.0 * PI - 1.15;

/// A stroke on the face: centred at longitude `th`, latitude `ph`, `w` wide (radians),
/// its middle `sag` lower than its ends, `r` thick.
fn arc(m: &mut Mesh, on: &dyn Fn(f32, f32, f32) -> V3, th: f32, ph: f32, w: f32, sag: f32, r: f32) {
    let n = 6;
    let pt = |i: usize| {
        let u = i as f32 / n as f32 * 2.0 - 1.0;
        on(th + u * w * 0.5, ph + sag * (1.0 - u * u), 0.12)
    };
    for i in 0..n {
        m.rods.push((
            Role::Feature,
            Rod {
                a: pt(i),
                b: pt(i + 1),
                r,
            },
        ));
    }
}

/// The left hand round the neck: the palm from the wrist to the grip, the thumb under it;
/// `scale` 1 for a person, smaller for the mannequin's mitten.
fn hand(m: &mut Mesh, s: &Skel, role: Role, scale: f32) {
    let along = (s.grip - s.lw).norm();
    let side = along.cross(s.f).norm();
    let palm = s.lw.lerp(s.grip, 0.5);
    ball(
        m,
        role,
        palm,
        (along, side),
        (4.4 * scale, 3.8 * scale, 1.8 * scale),
    );
    let thumb = palm + side * (2.8 * scale);
    limb(m, role, thumb, thumb + along * (3.6 * scale), 0.9 * scale);
    let fingers = s.grip + along * (1.2 * scale);
    ball(
        m,
        role,
        fingers,
        (along, side),
        (2.4 * scale, 3.6 * scale, 1.6 * scale),
    );
}

/// The artist's mannequin in `wood`: an egg head, a chest and hips joined by a ball, ball
/// joints at the shoulders, elbows, wrists, hips, knees and ankles, mitten hands.
pub fn mannequin(m: &mut Mesh, s: &Skel, wood: u8) {
    let (w, j) = (Role::Wood(wood), Role::Joint(wood));
    let (r, up) = (s.r, -s.down);
    let hr = s.hf.cross(s.hu).norm();
    ball(m, w, s.head + s.hu * 0.5, (-hr, s.hu), (6.8, 10.2, 8.4));
    limb(m, w, s.neck, s.head - s.hu * 7.0, 2.6);
    ball(m, w, s.chest, (r, up), (13.5, 15.0, 8.2));
    let waist = s.chest.lerp(s.pelvis, 0.55);
    sphere(m, j, waist, 5.2);
    ball(m, w, s.pelvis, (r, up), (12.5, 8.0, 9.0));
    for sh in [s.ls, s.rs] {
        sphere(m, j, sh, 3.9);
    }
    m.rods.push((
        w,
        Rod {
            a: s.ls,
            b: s.le,
            r: 2.7,
        },
    ));
    sphere(m, j, s.le, 2.9);
    m.rods.push((
        w,
        Rod {
            a: s.le,
            b: s.lw,
            r: 2.2,
        },
    ));
    sphere(m, j, s.lw, 1.9);
    hand(m, s, w, 0.85);
    for i in 0..2 {
        sphere(m, j, s.hips[i], 4.4);
        m.rods.push((
            w,
            Rod {
                a: s.hips[i],
                b: s.knees[i],
                r: 4.4,
            },
        ));
        sphere(m, j, s.knees[i], 4.0);
        m.rods.push((
            w,
            Rod {
                a: s.knees[i],
                b: s.ankles[i],
                r: 3.3,
            },
        ));
        sphere(m, j, s.ankles[i], 2.6);
        let toe = s.ankles[i] + s.f * 8.0;
        ball(
            m,
            w,
            s.ankles[i].lerp(toe, 0.5) + s.down * 3.0,
            (s.r, up),
            (3.4, 2.6, 7.6),
        );
    }
}
