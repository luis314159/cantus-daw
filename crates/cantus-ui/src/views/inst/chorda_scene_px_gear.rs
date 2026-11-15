//! r40:scene-px — the microphones and their stands for the photographic renderer, where the
//! scene module puts them (`scene::gear`): the piano's pair on its stereo bar placed by
//! Perspective and Width (its tally lit), the room pair of large-diaphragm condensers far
//! back, the drummer's dynamics on their rim clips, pencils, the kick's condenser and sub,
//! the overhead pair and the room pair — brushed bodies, nickel grilles, black stands with
//! chrome masts, and cables running over the floor out of the frame.

use super::geo::{cm, of, Geo};
use super::kit::{tripod, KitMats};
use super::mat::{parts, MatId};
use super::mesh::{v3, V3};
use crate::scene::gear::{piano_pair_place, KitMic, MicKind};

/// The gear's materials.
pub struct GearMats {
    pub k: KitMats,
    pub body: MatId,
    pub grille: MatId,
    pub nickel: MatId,
    pub dark: MatId,
    pub tally: MatId,
}

impl GearMats {
    pub fn new(g: &mut Geo, k: KitMats) -> GearMats {
        GearMats {
            k,
            body: g.mat(parts::mic_body()),
            grille: g.mat(parts::grille()),
            nickel: g.mat(parts::nickel()),
            dark: g.mat(parts::black_hw()),
            tally: g.mat(parts::tally()),
        }
    }
}

fn rod(g: &mut Geo, a: V3, b: V3, r_cm: f32, m: MatId) {
    g.rod(a, b, r_cm / 100.0, m);
}

/// A cable from a stand's foot over the floor, away out of the frame.
fn cable(g: &mut Geo, m: &GearMats, base: V3, away: V3) {
    let mut prev = v3(base.x, 0.004, base.z);
    for i in 1..16 {
        let f = i as f32 / 15.0;
        let x = base.x + (away.x - base.x) * f + 0.14 * (f * 3.1).sin();
        let z = base.z + (away.z - base.z) * f;
        let p = v3(x, 0.004, z);
        g.tube(prev, p, 0.0035, 0.0035, 8, false, m.k.rubber);
        prev = p;
    }
}

/// A tripod, a mast, a boom to `end` with its counterweight behind the mast.
fn boom_stand(g: &mut Geo, m: &GearMats, base: V3, mast_h: f32, end: V3, cable_to: Option<V3>) {
    tripod(g, &m.k, base, 26.0, 26.0, base.x * 70.0, true);
    let top = v3(base.x, mast_h / 100.0, base.z);
    // Masts and booms a size lighter than a cymbal stand's (as the drawn picture's).
    rod(g, v3(base.x, 0.26, base.z), top, 0.75, m.k.chrome);
    rod(
        g,
        top - v3(0.0, 0.015, 0.0),
        top + v3(0.0, 0.015, 0.0),
        1.4,
        m.k.black,
    );
    let d = (end - top).norm();
    let tail = top - d * 0.22;
    rod(g, tail, end, 0.55, m.k.chrome);
    g.tube(
        tail - d * 0.02,
        tail + d * 0.03,
        0.02,
        0.02,
        20,
        true,
        m.k.black,
    );
    if let Some(to) = cable_to {
        cable(g, m, base, to);
    }
}

/// A small-diaphragm pencil, its head at `head` aimed along `aim`.
#[allow(clippy::too_many_arguments)]
fn pencil(
    g: &mut Geo,
    m: &GearMats,
    head: V3,
    aim: V3,
    length: f32,
    r: f32,
    scale: f32,
    tally: bool,
) {
    let (len, rr) = (length * scale / 100.0, r * scale / 100.0);
    let tail = head - aim * len;
    let grille = head - aim * (len * 0.25);
    g.tube(tail, grille, rr, rr, 20, true, m.body);
    g.tube(grille, head, rr * 1.04, rr * 1.04, 20, true, m.grille);
    if tally {
        let at = head - aim * (len * 0.3);
        g.tube(
            at - aim * 0.006,
            at + aim * 0.006,
            rr * 1.1,
            rr * 1.1,
            20,
            false,
            m.tally,
        );
    }
}

/// A large-diaphragm condenser in its shock mount.
fn ldc(g: &mut Geo, m: &GearMats, pos: V3, scale: f32) {
    let s = scale / 100.0;
    let up = v3(0.0, 1.0, 0.0);
    g.tube(
        pos - up * (9.0 * s),
        pos,
        2.6 * s,
        2.6 * s,
        24,
        true,
        m.nickel,
    );
    g.tube(
        pos,
        pos + up * (6.0 * s),
        2.8 * s,
        2.8 * s,
        24,
        false,
        m.grille,
    );
    g.lathe(
        pos + up * (6.0 * s),
        up,
        &[(2.8 * s, 0.0), (2.4 * s, 1.0 * s), (0.0, 1.4 * s)],
        24,
        m.grille,
    );
    for y in [-6.0, 0.0] {
        let p = pos + v3(0.0, y * s, 0.0);
        g.lathe(
            p,
            up,
            &[(4.4 * s, -0.25 * s), (4.6 * s, 0.0), (4.4 * s, 0.25 * s)],
            32,
            m.dark,
        );
    }
}

/// The piano's microphones: the pair Perspective places, the room pair when `room` is above
/// 3 % (`back_z` the back wall's z, cm).
pub fn piano(
    g: &mut Geo,
    m: &GearMats,
    persp: f32,
    width: f32,
    room: f32,
    upright: bool,
    back_z: f32,
) {
    let scale = 1.6;
    let (centre, aim, spacing, sd, base) = piano_pair_place(persp, width, upright);
    let (c, aim, sd, base) = (of(centre), of(aim).norm(), of(sd).norm(), of(base));
    boom_stand(
        g,
        m,
        base,
        centre.y + 18.0,
        c + v3(0.0, 0.06, 0.0),
        Some(v3(base.x - 2.0, 0.0, base.z + 3.0)),
    );
    // Round 40 (visual): the pair hangs from the boom's end on a short black clip, a black
    // stereo bar across, the two pencils from its ends along the aim, toed out a little —
    // one clean T (a chrome drop rod and splayed pencils read as a zig-zag).
    rod(
        g,
        c + v3(0.0, 0.06, 0.0),
        c + v3(0.0, 0.012, 0.0),
        1.0,
        m.k.black,
    );
    let (a, b) = (c - sd * (spacing / 200.0), c + sd * (spacing / 200.0));
    rod(g, a, b, 0.75, m.k.black);
    for (p, sgn) in [(a, -1.0f32), (b, 1.0)] {
        let toe = (aim + sd * (sgn * 0.12)).norm();
        rod(
            g,
            p - v3(0.0, 0.012, 0.0),
            p + v3(0.0, 0.012, 0.0),
            1.1,
            m.k.black,
        );
        pencil(g, m, p + toe * (0.15 * scale), toe, 15.0, 1.0, scale, true);
    }
    if room > 0.03 {
        for sx in [-1.0f32, 1.0] {
            let b = cm(-40.0 + sx * 150.0, 0.0, back_z + 50.0);
            tripod(g, &m.k, b, 30.0, 32.0, 10.0, true);
            rod(g, v3(b.x, 0.30, b.z), v3(b.x, 1.96, b.z), 0.75, m.k.chrome);
            ldc(g, m, v3(b.x, 2.10, b.z + 0.04), scale * 0.9);
            cable(g, m, b, b + v3(sx * 3.0, 0.0, -2.0));
        }
    }
}

fn mic_body(g: &mut Geo, m: &GearMats, p: V3, aim: V3, kind: MicKind, scale: f32) {
    let s = scale / 100.0;
    match kind {
        MicKind::Dynamic => {
            let tail = p - aim * (13.0 * s * 0.7);
            g.tube(
                tail,
                p - aim * (3.0 * s),
                1.4 * s * 0.7,
                1.4 * s * 0.7,
                20,
                true,
                m.dark,
            );
            g.tube(
                p - aim * (3.0 * s),
                p,
                1.9 * s * 0.7,
                1.9 * s * 0.7,
                20,
                true,
                m.grille,
            );
        }
        MicKind::Pencil => pencil(g, m, p, aim, 15.0, 1.0, scale * 0.8, false),
        MicKind::Ribbon => pencil(g, m, p, aim, 12.0, 2.2, scale * 0.8, false),
        MicKind::Ldc => ldc(g, m, p, scale * 0.6),
        MicKind::Sub => {
            g.tube(p - aim * 0.10, p, 0.08, 0.08, 40, true, m.dark);
            g.tube(p - aim * 0.006, p, 0.074, 0.074, 40, true, m.grille);
        }
        MicKind::Pair | MicKind::LdcPair => {}
    }
}

/// The drummer's mics.
pub fn kit(g: &mut Geo, m: &GearMats, mics: &[KitMic]) {
    let scale = 1.5;
    for k in mics {
        let (p, aim) = (of(k.p), of(k.aim).norm());
        match k.kind {
            MicKind::Pair => {
                for sx in [-1.0f32, 1.0] {
                    let base = cm(sx * 70.0, 0.0, -120.0);
                    let q = p + v3(sx * k.spread / 200.0, 0.0, 0.0);
                    boom_stand(g, m, base, 176.0, q + v3(0.0, 0.06, 0.0), None);
                    pencil(g, m, q, aim, 15.0, 1.0, scale * 0.8, false);
                }
            }
            MicKind::LdcPair => {
                for sx in [-1.0f32, 1.0] {
                    let q = p + v3(sx * k.spread / 200.0, 0.0, 0.0);
                    let base = v3(q.x, 0.0, q.z + 0.1);
                    tripod(g, &m.k, base, 30.0, 30.0, 0.0, true);
                    rod(
                        g,
                        v3(base.x, 0.30, base.z),
                        v3(q.x, q.y - 0.1, q.z),
                        0.75,
                        m.k.chrome,
                    );
                    ldc(g, m, q, scale * 0.7);
                    cable(g, m, base, base + v3(sx * 2.0, 0.0, 2.0));
                }
            }
            kind => {
                if let Some(st) = k.stand {
                    let st = of(st);
                    boom_stand(
                        g,
                        m,
                        st,
                        k.p.y + 14.0,
                        p + v3(0.0, 0.04, 0.0),
                        Some(st + v3(1.2, 0.0, 2.6)),
                    );
                }
                if k.clip {
                    let a = p - aim * 0.06;
                    rod(g, a, a + v3(0.0, -0.07, 0.0), 0.5, m.k.black);
                }
                mic_body(g, m, p, aim, kind, scale);
            }
        }
    }
}
