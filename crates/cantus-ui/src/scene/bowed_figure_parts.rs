//! r42:arcus-upper — the figures' parts (`bowed_figure`): the musician (skin, concert
//! clothes, hair, a calm face of a few strokes) and the artist's wooden mannequin (an egg
//! head, ball joints), both on the same skeleton. Both arms are the live picture's (the
//! hands follow the playing); only the shoulders are here.

use super::bowed_figure::{ball, limb, shell, sphere, taper, Skel, HEAD};
use super::bowed_mesh::{Mesh, Rod, Role};
use super::geo::V3;
use std::f32::consts::{FRAC_PI_2, PI};

/// The musician: `skin` and `cloth` tones.
pub fn musician(m: &mut Mesh, s: &Skel, skin: u8, cloth: u8) {
    let (sk, cl) = (Role::Skin(skin), Role::Cloth(cloth));
    head(m, s, sk);
    // The neck (short, as a person's).
    let chin_under = s.head - s.hu * (HEAD.1 * 0.62);
    let neck_role = match sk {
        Role::Skin(i) => Role::SkinShade(i),
        r => r,
    };
    taper(m, neck_role, s.neck + s.f * 0.5, chin_under, 4.6, 4.2);
    // The torso: chest and waist in the shirt, the hips in the trousers.
    let (r, up) = (s.r, -s.down);
    ball(m, cl, s.chest + up * 1.0, (r, up), (14.5, 15.0, 9.4));
    let waist = s.chest.lerp(s.pelvis, 0.55);
    ball(m, cl, waist, (r, up), (12.8, 12.5, 8.8));
    ball(m, Role::Trousers, s.pelvis, (r, up), (14.0, 8.0, 10.5));
    // The shoulders sloping from the collar.
    for sh in [s.ls, s.rs] {
        taper(
            m,
            cl,
            s.neck + (sh - s.neck) * 0.15 + s.down * 1.5,
            sh,
            5.2,
            5.4,
        );
    }
    // Both arms are the live picture's: the left hand goes where the note is stopped,
    // the right one draws the bow.
    // The legs.
    for i in 0..2 {
        taper(m, Role::Trousers, s.hips[i], s.knees[i], 6.6, 5.2);
        taper(m, Role::Trousers, s.knees[i], s.ankles[i], 4.9, 3.8);
        let toe = s.ankles[i] + s.f * 9.0;
        ball(
            m,
            Role::Shoes,
            s.ankles[i].lerp(toe, 0.5) + s.down * 3.4,
            (s.r, -s.down),
            (4.2, 3.0, 8.6),
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
        (0.85, TAU_M),
        (-0.45, 0.42),
    );
    // A fringe a little lower and fuller over the forehead, so the hairline steps down
    // softly instead of cutting straight across.
    shell(
        m,
        Role::Hair,
        s.head + s.hu * 0.3,
        axes,
        (a + g * 1.3, b + g * 1.1, c + g * 1.4),
        (-0.95, 0.95),
        (0.2, 0.5),
    );
    // The jaw and the cheeks: the lower face narrower and rounded, not a block.
    ball(
        m,
        sk,
        s.head - s.hu * (b * 0.48) + s.hf * (c * 0.26),
        axes,
        (a * 0.74, b * 0.42, c * 0.64),
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

/// Longitudes past the back: from the right temple round the back to the left (radians).
const TAU_M: f32 = 2.0 * PI - 0.85;

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

/// The artist's mannequin in `wood`: an egg head, a chest and hips joined by a ball, ball
/// joints at the shoulders, hips, knees and ankles (its arms are the live picture's).
pub fn mannequin(m: &mut Mesh, s: &Skel, wood: u8) {
    let (w, j) = (Role::Wood(wood), Role::Joint(wood));
    let (r, up) = (s.r, -s.down);
    let hr = s.hf.cross(s.hu).norm();
    ball(m, w, s.head + s.hu * 0.5, (-hr, s.hu), (6.0, 9.2, 7.6));
    limb(m, w, s.neck, s.head - s.hu * 6.5, 2.4);
    ball(m, w, s.chest, (r, up), (12.8, 14.2, 7.8));
    let waist = s.chest.lerp(s.pelvis, 0.55);
    sphere(m, j, waist, 5.0);
    ball(m, w, s.pelvis, (r, up), (12.0, 7.6, 8.6));
    for sh in [s.ls, s.rs] {
        sphere(m, j, sh, 3.7);
    }
    for i in 0..2 {
        sphere(m, j, s.hips[i], 4.3);
        taper(m, w, s.hips[i], s.knees[i], 4.6, 3.6);
        sphere(m, j, s.knees[i], 3.9);
        taper(m, w, s.knees[i], s.ankles[i], 3.4, 2.6);
        sphere(m, j, s.ankles[i], 2.5);
        let toe = s.ankles[i] + s.f * 8.0;
        ball(
            m,
            w,
            s.ankles[i].lerp(toe, 0.5) + s.down * 3.0,
            (s.r, up),
            (3.3, 2.5, 7.4),
        );
    }
}
