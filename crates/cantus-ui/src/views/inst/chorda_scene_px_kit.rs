//! r40:scene-px — Tympanum's kit for the photographic renderer, from the scene module's
//! pieces (`scene::kit::Piece`: each piece's centre, head axis, radius and depth, a cymbal
//! stand's foot, a floor tom's legs — the designer's `kit-model.json` or a kit definition's
//! places): lacquered wood shells with their figure, triple-flanged chrome hoops (the kick's
//! in stained wood), lugs and tension rods, coated batter heads and the kick's black
//! resonant head with its port, lathed bronze cymbals with their bell and hammering, and
//! every stand, tripod, boom, pedal, holder and leg the designer drew.

use super::geo::{cm, of, perp, Geo};
use super::mat::{finish, lin3, parts, MatId, PMat, Tex};
use super::mesh::{v3, V3};
use crate::kit::skin::scene::Mat;
use crate::scene::kit::{prof, Kind, Piece, BELL};
use std::f32::consts::TAU;

/// The materials every kit shares.
pub struct KitMats {
    pub chrome: MatId,
    pub black: MatId,
    pub felt: MatId,
    pub rubber: MatId,
    pub shell: MatId,
    pub hoop_wood: MatId,
}

impl KitMats {
    pub fn new(g: &mut Geo, fin: &Mat) -> KitMats {
        let shell = finish(fin, 41);
        // The kick's wooden hoops in the shells' own finish (round 40, as the drawn kit).
        let hoop = finish(fin, 43);
        KitMats {
            chrome: g.mat(parts::chrome()),
            black: g.mat(parts::black_hw()),
            felt: g.mat(parts::felt()),
            rubber: g.mat(parts::rubber()),
            shell: g.mat(shell),
            hoop_wood: g.mat(hoop),
        }
    }
}

fn rod(g: &mut Geo, a: V3, b: V3, r_cm: f32, m: MatId) {
    g.rod(a, b, r_cm / 100.0, m);
}

/// A tripod's legs, braces and collars round `base` (cm).
pub fn tripod(
    g: &mut Geo,
    k: &KitMats,
    base: V3,
    collar_h: f32,
    spread: f32,
    rot: f32,
    braces: bool,
) {
    let (bx, bz) = (base.x * 100.0, base.z * 100.0);
    let top = cm(bx, collar_h, bz);
    let low = cm(bx, collar_h * 0.42, bz);
    for i in 0..3 {
        let a = (rot + 120.0 * i as f32 + 90.0).to_radians();
        let foot = cm(bx + spread * a.cos(), 1.0, bz + spread * a.sin());
        let knee = foot - (foot - top).norm() * 0.04;
        rod(g, top, knee, 0.75, k.chrome);
        rod(g, knee, foot, 1.05, k.rubber);
        if braces {
            rod(g, low, top + (foot - top) * 0.55, 0.45, k.chrome);
        }
    }
    rod(
        g,
        cm(bx, collar_h - 2.0, bz),
        cm(bx, collar_h + 2.5, bz),
        1.75,
        k.black,
    );
    rod(
        g,
        cm(bx, collar_h * 0.42 - 1.5, bz),
        cm(bx, collar_h * 0.42 + 1.5, bz),
        1.45,
        k.black,
    );
}

/// The cymbal's felt and wing nut on its bell's top (`bell` cm over the plane at `at`).
fn wing_nut(g: &mut Geo, k: &KitMats, at: V3, ax: V3, bell: f32) {
    let b = bell / 100.0;
    rod(g, at - ax * 0.025, at + ax * (b + 0.014), 0.42, k.chrome);
    g.tube(
        at + ax * (b + 0.0005),
        at + ax * (b + 0.0065),
        0.016,
        0.016,
        20,
        true,
        k.felt,
    );
    let top = at + ax * (b + 0.014);
    rod(g, top, top + ax * 0.012, 0.9, k.black);
    let side = ax.cross(v3(0.0, 0.0, 1.0)).norm();
    let m = top + ax * 0.008;
    rod(g, m - side * 0.022, m + side * 0.022, 0.45, k.black);
}

fn cymbal_stand(g: &mut Geo, k: &KitMats, p: &Piece) {
    let (cc, ax) = (of(p.c), of(p.axis) * 100.0);
    let ax = ax.norm();
    let b = p.base.map(of).unwrap_or(v3(cc.x, 0.0, cc.z - 0.3));
    let under = cc - ax * 0.03;
    let top_h = (p.c.y - 24.0).max(60.0);
    let top = v3(b.x, top_h / 100.0, b.z);
    let h1 = (28.0 + (top_h - 28.0) * 0.55) / 100.0;
    tripod(g, k, b, 28.0, 31.0, b.x * 130.0, true);
    rod(g, v3(b.x, 0.30, b.z), v3(b.x, h1, b.z), 1.25, k.chrome);
    rod(
        g,
        v3(b.x, h1 - 0.01, b.z),
        v3(b.x, h1 + 0.025, b.z),
        1.7,
        k.black,
    );
    rod(g, v3(b.x, h1, b.z), top, 0.95, k.chrome);
    rod(
        g,
        top - v3(0.0, 0.015, 0.0),
        top + v3(0.0, 0.02, 0.0),
        1.55,
        k.black,
    );
    let dv = (under - top).norm();
    let tail = top - dv * 0.13;
    rod(g, tail, under, 0.8, k.chrome);
    rod(g, tail - dv * 0.015, tail + dv * 0.025, 1.9, k.black);
    rod(g, under - ax * 0.016, under + ax * 0.014, 1.25, k.black);
    wing_nut(g, k, cc, ax, prof(0.0, p.china));
}

fn pedal(g: &mut Geo, k: &KitMats, heel: V3, board_len: f32, width: f32, toward: (f32, f32)) {
    let (dx, dz) = toward;
    let h = v3(heel.x, 0.015, heel.z);
    let toe = h + v3(dx, 0.0, dz) * (board_len / 100.0) + v3(0.0, 0.055, 0.0);
    let side = v3(-dz, 0.0, dx).norm() * (width / 200.0);
    let along = toe - h;
    let nrm = along.cross(side).norm();
    let nrm = if nrm.y < 0.0 { -nrm } else { nrm };
    g.boxed(h - side, side * 2.0, along, nrm * 0.008, k.black);
    for (a, b) in [
        (h - side, h + side),
        (h + side, toe + side * 0.9),
        (toe + side * 0.9, toe - side * 0.9),
        (toe - side * 0.9, h - side),
    ] {
        rod(g, a, b, 0.5, k.chrome);
    }
}

fn hats_stand(g: &mut Geo, k: &KitMats, p: &Piece) {
    let c = p.c;
    let (x, z) = (c.x, c.z);
    tripod(g, k, cm(x, 0.0, z), 32.0, 29.0, 40.0, true);
    rod(g, cm(x, 33.0, z), cm(x, c.y - 12.0, z), 1.35, k.chrome);
    rod(g, cm(x, c.y - 13.0, z), cm(x, c.y - 9.5, z), 1.8, k.black);
    // The pull rod up through both bells, the clutch over the top one's bell.
    let bell = 2.4 + prof(0.0, false);
    rod(
        g,
        cm(x, c.y - 10.0, z),
        cm(x, c.y + bell + 4.0, z),
        0.45,
        k.chrome,
    );
    rod(
        g,
        cm(x, c.y + bell + 0.2, z),
        cm(x, c.y + bell + 3.1, z),
        1.3,
        k.black,
    );
    pedal(g, k, cm(x, 0.0, z - 6.0), 30.0, 9.0, (0.0, -1.0));
    rod(g, cm(x, 4.0, z - 7.0), cm(x, 33.0, z), 0.35, k.chrome);
}

fn snare_stand(g: &mut Geo, k: &KitMats, p: &Piece) {
    let (cc, ax, r, d) = (of(p.c), of(p.axis).norm(), p.r / 100.0, p.d / 100.0);
    let under = cc - ax * (d / 2.0 + 0.07);
    let b = v3(under.x, 0.0, under.z);
    tripod(g, k, b, 22.0, 27.0, 15.0, true);
    rod(g, v3(b.x, 0.24, b.z), under, 1.15, k.chrome);
    rod(
        g,
        under - v3(0.0, 0.02, 0.0),
        under + v3(0.0, 0.015, 0.0),
        1.7,
        k.black,
    );
    let e1 = perp(ax);
    let e2 = ax.cross(e1);
    for i in 0..3 {
        let a = 0.6 + TAU * i as f32 / 3.0;
        let q = cc - ax * (d / 2.0 + 0.015) + (e1 * a.cos() + e2 * a.sin()) * (r * 0.86);
        rod(g, under, q, 0.5, k.chrome);
        rod(g, q, q + ax * 0.022, 0.8, k.rubber);
    }
}

fn kick_hardware(g: &mut Geo, k: &KitMats, p: &Piece) {
    let (c, r, d) = (p.c, p.r, p.d);
    let (front, back) = (c.z + d / 2.0, c.z - d / 2.0);
    for sx in [-1.0f32, 1.0] {
        let a = cm(c.x + sx * (r + 1.2), c.y - 6.0, front - 9.0);
        let foot = cm(c.x + sx * (r + 13.0), 1.0, front + 3.0);
        let knee = foot - (foot - a).norm() * 0.035;
        rod(g, a, knee, 0.85, k.chrome);
        rod(g, knee, foot, 1.15, k.rubber);
        rod(g, a - cm(sx * 2.4, 0.0, 0.0), a, 1.5, k.chrome);
    }
    pedal(g, k, cm(c.x, 0.0, back - 44.0), 26.0, 9.0, (0.0, 1.0));
    for px in [-6.0, 6.0] {
        rod(
            g,
            cm(c.x + px, 2.0, back - 16.0),
            cm(c.x + px, 18.0, back - 12.0),
            0.9,
            k.chrome,
        );
    }
    rod(
        g,
        cm(c.x - 6.0, 18.0, back - 12.0),
        cm(c.x + 6.0, 18.0, back - 12.0),
        0.7,
        k.chrome,
    );
    rod(
        g,
        cm(c.x, 18.0, back - 12.0),
        cm(c.x, 31.0, back - 3.5),
        0.35,
        k.chrome,
    );
    g.tube(
        cm(c.x, 31.0, back - 4.8),
        cm(c.x, 31.0, back - 0.6),
        0.024,
        0.024,
        20,
        true,
        k.felt,
    );
}

fn tom_holder(g: &mut Geo, k: &KitMats, kick: &Piece, toms: &[Piece]) {
    let (c, r) = (kick.c, kick.r);
    let base = cm(c.x, c.y + r + 0.8, c.z + 4.0);
    let post_top = cm(c.x, c.y + r + 12.0, c.z + 3.0);
    rod(
        g,
        base - cm(3.0, 0.0, 0.0),
        base + cm(3.0, 0.0, 0.0),
        1.6,
        k.black,
    );
    rod(g, base, post_top, 1.25, k.chrome);
    rod(
        g,
        post_top - cm(0.0, 2.0, 0.0),
        post_top + cm(0.0, 1.5, 0.0),
        1.9,
        k.black,
    );
    for t in toms {
        let (tc, ta) = (of(t.c), of(t.axis).norm());
        // The bracket on the shell's quarter toward the post and the front (as the drawn
        // picture's): its arm shows between the toms.
        let toward =
            (v3(post_top.x - tc.x, 0.0, post_top.z - tc.z).norm() + v3(0.0, 0.0, 0.9)).norm();
        let brk = tc - ta * ((t.d / 2.0 - 7.0) / 100.0) + toward * ((t.r + 1.4) / 100.0);
        let elbow = v3(brk.x, post_top.y + 0.02, brk.z);
        rod(g, post_top, elbow, 0.85, k.chrome);
        rod(g, elbow, brk, 0.85, k.chrome);
        rod(g, brk - ta * 0.025, brk + ta * 0.025, 1.5, k.black);
    }
}

fn floor_legs(g: &mut Geo, k: &KitMats, p: &Piece, turn: f32) {
    let (cc, ax, r, d) = (of(p.c), of(p.axis).norm(), p.r / 100.0, p.d / 100.0);
    for i in 0..3 {
        let a = (turn + 60.0 + 120.0 * i as f32).to_radians();
        let rd = v3(a.cos(), 0.0, a.sin());
        let brk = cc + ax * (d * 0.18) + rd * (r + 0.016);
        let foot = v3(
            cc.x + (r + 0.09) * a.cos(),
            0.01,
            cc.z + (r + 0.09) * a.sin(),
        );
        let knee = foot - (foot - brk).norm() * 0.032;
        rod(g, brk - ax * 0.03, brk + ax * 0.03, 1.3, k.chrome);
        rod(g, brk + ax * 0.035, knee, 0.65, k.chrome);
        rod(g, knee, foot, 1.0, k.rubber);
    }
}

/// A hoop round the head plane at `plane`, its flange reaching `out` along the axis (±1).
fn hoop(g: &mut Geo, plane: V3, ax: V3, r: f32, out: f32, wood: bool, m: MatId) {
    let prof: Vec<(f32, f32)> = if wood {
        vec![
            (r + 0.001, -0.016),
            (r + 0.016, -0.016),
            (r + 0.019, -0.010),
            (r + 0.019, 0.010),
            (r + 0.016, 0.014),
            (r + 0.001, 0.014),
            (r + 0.001, -0.016),
        ]
    } else {
        vec![
            (r + 0.001, -0.009),
            (r + 0.0065, -0.009),
            (r + 0.0065, 0.008),
            (r + 0.010, 0.0105),
            (r + 0.0085, 0.0125),
            (r + 0.004, 0.0115),
            (r + 0.002, 0.009),
            (r + 0.001, -0.009),
        ]
    };
    let prof: Vec<(f32, f32)> = prof.into_iter().map(|(a, h)| (a, h * out)).collect();
    g.lathe(plane, ax, &prof, 160, m);
}

fn drum(g: &mut Geo, k: &KitMats, p: &Piece) {
    let (cc, ax, r, d) = (of(p.c), of(p.axis).norm(), p.r / 100.0, p.d / 100.0);
    let kick = p.kind == Kind::Kick;
    let (top, bot) = (cc + ax * (d / 2.0), cc - ax * (d / 2.0));
    g.tube(bot, top, r, r, 160, false, k.shell);
    let coated = g.mat(
        PMat::lacquer(lin3([0.80, 0.79, 0.75]), 0.16, 0.4).with(Tex::Head {
            c: top,
            axis: ax,
            r,
            coated: true,
        }),
    );
    let reso = if kick {
        g.mat(
            PMat::lacquer(lin3([0.09, 0.09, 0.1]), 0.5, 0.3).with(Tex::Head {
                c: top,
                axis: ax,
                r,
                coated: false,
            }),
        )
    } else {
        g.mat(PMat::matte(lin3([0.88, 0.87, 0.84])).with(Tex::Head {
            c: bot,
            axis: ax,
            r,
            coated: true,
        }))
    };
    // Heads: the batter (the kick's is the back one), the resonant.
    let head = |g: &mut Geo, plane: V3, sgn: f32, m: MatId| {
        g.lathe(
            plane,
            ax * sgn,
            &[
                (0.0, 0.003),
                (r * 0.94, 0.003),
                (r * 0.99, 0.0),
                (r, -0.004),
            ],
            96,
            m,
        );
    };
    if kick {
        head(g, top, 1.0, reso);
        head(g, bot, -1.0, coated);
        let e1 = perp(ax);
        let e2 = ax.cross(e1);
        let pc = top + e1 * (-r * 0.42) + e2 * (-r * 0.40) + ax * 0.0035;
        let port = g.mat(PMat::matte(lin3([0.004, 0.004, 0.005])));
        g.lathe(pc, ax, &[(0.0, 0.0005), (r * 0.15, 0.0005)], 48, port);
        g.lathe(
            pc,
            ax,
            &[
                (r * 0.15, 0.0),
                (r * 0.15, 0.003),
                (r * 0.165, 0.003),
                (r * 0.165, 0.0),
            ],
            48,
            k.chrome,
        );
    } else {
        head(g, top, 1.0, coated);
        head(g, bot, -1.0, reso);
    }
    let (hm, wood) = if kick {
        (k.hoop_wood, true)
    } else {
        (k.chrome, false)
    };
    hoop(g, top, ax, r, 1.0, wood, hm);
    hoop(g, bot, ax, r, -1.0, wood, hm);
    // Lugs and tension rods.
    let e1 = perp(ax);
    let e2 = ax.cross(e1);
    let nl = if matches!(p.kind, Kind::Snare | Kind::Kick) {
        10
    } else if r > 0.16 {
        8
    } else {
        6
    };
    for i in 0..nl {
        let a = TAU * (i as f32 + 0.5) / nl as f32;
        let rm = e1 * a.cos() + e2 * a.sin();
        let spans: Vec<(f32, f32)> = match p.kind {
            Kind::Snare => vec![(0.30, 0.44), (-0.44, -0.30)],
            Kind::Kick => vec![(0.32, 0.45), (-0.45, -0.32)],
            _ => {
                let l = (d * 0.34).min(0.07) / d;
                vec![(-l / 2.0, l / 2.0)]
            }
        };
        for &(s0, s1) in &spans {
            let (a0, a1) = (
                cc + ax * (d * s0) + rm * (r + 0.011),
                cc + ax * (d * s1) + rm * (r + 0.011),
            );
            g.tube(a0, a1, 0.0085, 0.0085, 14, true, k.chrome);
            g.tube(
                a0 - rm * 0.008,
                a1 - rm * 0.008,
                0.006,
                0.006,
                10,
                false,
                k.chrome,
            );
        }
        let ends: Vec<f32> = spans.iter().flat_map(|&(a, b)| [a, b]).collect();
        for sgn in [1.0f32, -1.0] {
            let hp = cc + ax * (sgn * (d / 2.0 + 0.006)) + rm * (r + 0.017);
            let end = if sgn > 0.0 {
                ends.iter().copied().fold(f32::MIN, f32::max)
            } else {
                ends.iter().copied().fold(f32::MAX, f32::min)
            };
            rod(
                g,
                hp,
                cc + ax * (d * end) + rm * (r + 0.015),
                0.32,
                k.chrome,
            );
            rod(
                g,
                hp - ax * (sgn * 0.002),
                hp + ax * (sgn * 0.009),
                0.62,
                k.chrome,
            );
        }
    }
    if kick {
        for i in 0..10 {
            let a = TAU * (i as f32 + 0.5) / 10.0;
            let rm = e1 * a.cos() + e2 * a.sin();
            let p0 = top + ax * 0.006 + rm * (r + 0.007);
            rod(g, p0 - ax * 0.018, p0 + ax * 0.006, 0.8, k.chrome);
        }
    }
}

fn cymbal(g: &mut Geo, p: &Piece, centre: V3, china: bool) {
    let (ax, r) = (of(p.axis).norm(), p.r / 100.0);
    let m = g.mat(PMat::aniso(
        lin3([0.80, 0.63, 0.36]),
        0.18,
        Tex::Lathe {
            c: centre,
            axis: ax,
            r,
        },
    ));
    let n = 30;
    let mut rhos: Vec<f32> = (0..=n)
        .map(|i| 0.03 + 0.97 * (i as f32 / n as f32).powf(0.85))
        .collect();
    // The bell's shoulder on a ring of its own (a crisp line where it leaves the bow).
    rhos.extend([BELL - 0.012, BELL, BELL + 0.012]);
    rhos.sort_by(|a, b| a.total_cmp(b));
    rhos.dedup_by(|a, b| (*a - *b).abs() < 0.004);
    let top: Vec<(f32, f32)> = rhos
        .iter()
        .map(|&rho| (rho * r, prof(rho, china) / 100.0))
        .collect();
    let mut pr = top.clone();
    // The edge: a rounded lip 3 mm thick (a turned edge that catches the light), the
    // metal thickening toward the bell.
    let (er, eh) = (r, prof(1.0, china) / 100.0);
    pr.pop();
    pr.extend([
        (er - 0.0012, eh + 0.0004),
        (er, eh - 0.0012),
        (er - 0.0006, eh - 0.0028),
    ]);
    for &(a, h) in top.iter().rev().skip(1) {
        pr.push((a, h - 0.003 - 0.0025 * (1.0 - a / r)));
    }
    pr.push(top[0]);
    g.lathe(centre, ax, &pr, 128, m);
}

/// The whole kit: each piece with its stand.
pub fn build(g: &mut Geo, pieces: &[Piece], fin: &Mat) {
    let k = KitMats::new(g, fin);
    let toms: Vec<Piece> = pieces
        .iter()
        .filter(|p| p.kind == Kind::Drum && p.legs.is_none())
        .copied()
        .collect();
    for p in pieces {
        match p.kind {
            Kind::Cymbal => cymbal_stand(g, &k, p),
            Kind::Hats => hats_stand(g, &k, p),
            Kind::Snare => snare_stand(g, &k, p),
            Kind::Kick => {
                kick_hardware(g, &k, p);
                tom_holder(g, &k, p, &toms);
            }
            Kind::Drum => {
                if let Some(turn) = p.legs {
                    floor_legs(g, &k, p, turn);
                }
            }
        }
        match p.kind {
            Kind::Drum | Kind::Snare | Kind::Kick => drum(g, &k, p),
            Kind::Hats => {
                cymbal(g, p, of(p.c), false);
                cymbal(g, p, of(p.c) + of(p.axis).norm() * 0.024, false);
            }
            Kind::Cymbal => cymbal(g, p, of(p.c), p.china),
        }
    }
}
