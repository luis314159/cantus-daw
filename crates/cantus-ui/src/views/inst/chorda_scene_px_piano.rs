//! r40:scene-px — Chorda's pianos for the photographic renderer: the designer's model
//! (`scene::piano`: the grands' one outline scaled — concert 2.74 m, grand 2.00 m, parlour
//! 1.55 m — the rim from 64 to 94 cm, the lid hinged on the bass side at its angle, the
//! legs, lyre, keyboard, desk and bench where the designer put them; the upright's carcass,
//! panels, keybed and top lid) built as closed meshes with smooth normals, so the key's
//! shadow, the occlusion, the floor's mirror and the lacquer's reflections see a solid
//! instrument: a rounded rim edge and lid edge that carry long crisp highlights, a lid with
//! a real thickness, a cast frame with its holes, struts, pins and dampers, strung wires.

use super::geo::{cm, outline_normals, Geo};
use super::mat::{lin3, parts, MatId, PMat};
use super::mesh::{v3, V3};
use crate::kit::skin::scene::{Mat, FINISH_WALNUT};
use crate::scene::piano::{length, lid_point, outline, RIM_HI, RIM_LO};
use crate::scene::piano_geo::{
    desk_on, flap_outline, hinge_x, lid_normal, lid_outline, prop, LID_FRONT, LID_TH,
};
use cantus_core::inst::chorda_look::Body;

/// The materials a piano is built of.
pub struct Mats {
    pub case: MatId,
    pub under: MatId,
    pub inner: MatId,
    pub gold: MatId,
    pub spruce: MatId,
    pub steel: MatId,
    pub copper: MatId,
    pub nickel: MatId,
    pub brass: MatId,
    pub felt: MatId,
    pub red: MatId,
    pub ivory: MatId,
    pub ebony: MatId,
    pub keybed: MatId,
    pub leather: MatId,
    /// The bench's top: the case's finish, a touch satin (it is sat on, not polished).
    pub bench: MatId,
    pub desk: MatId,
    pub rubber: MatId,
    /// A sheet of music and its staves' ink.
    pub paper: MatId,
    pub ink: MatId,
}

impl Mats {
    pub fn new(g: &mut Geo, finish: &Mat) -> Mats {
        let case = super::mat::finish(finish, 17);
        let mut under = case;
        under.lid_under = true;
        // The underside mirrors the harp a little softened (its lacquer is never polished
        // as the top is): the strings read as a sheen, not a mesh.
        under.rough = under.rough.max(0.05);
        let inner = if finish.wood {
            case
        } else {
            super::mat::finish(&FINISH_WALNUT, 29)
        };
        Mats {
            case: g.mat(case),
            under: g.mat(under),
            inner: g.mat(inner),
            gold: g.mat(parts::gold()),
            spruce: g.mat(parts::spruce()),
            steel: g.mat(parts::steel()),
            copper: g.mat(parts::copper()),
            nickel: g.mat(parts::nickel()),
            brass: g.mat(parts::brass()),
            felt: g.mat(parts::damper()),
            red: g.mat(parts::felt_red()),
            ivory: g.mat(parts::ivory()),
            ebony: g.mat(parts::ebony()),
            keybed: g.mat(PMat::lacquer(lin3([0.05, 0.05, 0.055]), 0.3, 0.3)),
            leather: g.mat(PMat::lacquer(lin3([0.04, 0.036, 0.033]), 0.05, 0.6)),
            desk: g.mat(case),
            bench: g.mat(PMat {
                rough: case.rough.max(0.14),
                coat: case.coat * 0.45,
                ..case
            }),
            rubber: g.mat(parts::rubber()),
            paper: g.mat(super::mat::finish(&crate::kit::skin::scene::mat::PAPER, 5)),
            ink: g.mat(PMat::matte(lin3(crate::kit::skin::scene::mat::STAFF))),
        }
    }
}

/// A plan outline in centimetres to metres.
fn to_m(o: &[(f32, f32)]) -> Vec<(f32, f32)> {
    o.iter().map(|&(x, z)| (x / 100.0, z / 100.0)).collect()
}

/// The piano `body` in `finish`, its lid at `lid` degrees.
pub fn build(g: &mut Geo, body: Body, finish: &Mat, lid: f32) {
    let m = Mats::new(g, finish);
    if body == Body::Upright {
        super::upright::build(g, &m, lid);
    } else {
        grand(g, &m, body, lid);
    }
}

fn leg(g: &mut Geo, m: &Mats, x: f32, z: f32) {
    let top = RIM_LO;
    let mid = top * 0.55;
    g.tube(cm(x, mid, z), cm(x, top, z), 0.040, 0.046, 24, true, m.case);
    g.tube(
        cm(x, mid - 1.2, z),
        cm(x, mid + 1.2, z),
        0.049,
        0.049,
        24,
        true,
        m.case,
    );
    g.tube(cm(x, 8.5, z), cm(x, mid, z), 0.029, 0.037, 24, true, m.case);
    g.tube(
        cm(x, 4.2, z),
        cm(x, 8.5, z),
        0.032,
        0.031,
        24,
        true,
        m.brass,
    );
    g.tube(
        cm(x - 1.6, 2.3, z),
        cm(x + 1.6, 2.3, z),
        0.022,
        0.022,
        16,
        true,
        m.rubber,
    );
    g.tube(
        cm(x, 2.5, z),
        cm(x, 4.3, z),
        0.012,
        0.012,
        10,
        false,
        m.brass,
    );
}

fn grand(g: &mut Geo, m: &Mats, body: Body, lid: f32) {
    let ang = lid.to_radians();
    let o = outline(body, 14);
    let om = to_m(&o);
    let zmin = o.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    // Legs, lyre and pedals.
    for (lx, lz) in [(-62.0, -6.0), (62.0, -6.0), (-30.0, zmin + 30.0)] {
        leg(g, m, lx, lz);
    }
    for px in [-9.0, 9.0] {
        g.tube(
            cm(px, 12.0, -10.0),
            cm(px, RIM_LO, -10.0),
            0.017,
            0.019,
            14,
            true,
            m.case,
        );
    }
    g.aabb(cm(-16.0, 7.0, -13.5), cm(16.0, 13.0, -6.5), m.case);
    for px in [-7.0, 0.0, 7.0] {
        g.tube(
            cm(px, 8.4, -6.5),
            cm(px, 6.4, 7.0),
            0.012,
            0.010,
            12,
            true,
            m.brass,
        );
    }
    // The case: its outer skin, its rounded top edge, the rim's top, its belly.
    let er = 0.016;
    let yt = RIM_HI / 100.0;
    g.wall(&om, RIM_LO / 100.0, yt - er, true, m.case);
    g.round_edge(&om, yt, er, false, m.case);
    g.round_edge(&om, RIM_LO / 100.0 + 0.006, 0.006, true, m.case);
    let ns = outline_normals(&om, true);
    let inner_m: Vec<(f32, f32)> = o
        .iter()
        .map(|&(x, z)| (x * 0.97 / 100.0, z * 0.98 / 100.0))
        .collect();
    let outer_in: Vec<(V3, V3)> = om
        .iter()
        .zip(&ns)
        .map(|(&(x, z), n)| {
            let nn = (n.0 + n.1).norm();
            (v3(x - nn.x * er, yt, z - nn.z * er), v3(0.0, 1.0, 0.0))
        })
        .collect();
    let inner_r: Vec<(V3, V3)> = inner_m
        .iter()
        .map(|&(x, z)| (v3(x, yt, z), v3(0.0, 1.0, 0.0)))
        .collect();
    let mut a = outer_in.clone();
    a.push(outer_in[0]);
    let mut b = inner_r.clone();
    b.push(inner_r[0]);
    g.strip(&a, &b, 0.0, 0.02, m.case);
    g.plate(&om, RIM_LO / 100.0, false, m.case);
    // The inside: the rim's veneer, the soundboard, the frame, its holes and struts.
    let plate_y = RIM_HI - 7.0;
    g.wall(&inner_m, (plate_y - 4.0) / 100.0, yt, false, m.inner);
    g.plate(&inner_m, (plate_y - 4.0) / 100.0, true, m.spruce);
    let ins: Vec<(f32, f32)> = o
        .iter()
        .map(|&(x, z)| ((x * 0.94) / 100.0, (z * 0.97 - 2.0) / 100.0))
        .collect();
    g.plate(&ins, plate_y / 100.0, true, m.gold);
    g.wall(&ins, (plate_y - 1.5) / 100.0, plate_y / 100.0, true, m.gold);
    let zs = length(body) / 248.0;
    let up = v3(0.0, 1.0, 0.0);
    for (hx, hz, hr) in [
        (-40.0, -150.0, 13.0),
        (-10.0, -175.0, 11.0),
        (18.0, -140.0, 10.0),
        (-46.0, -70.0, 9.0),
        (30.0, -95.0, 8.0),
    ] {
        let r = hr * (zs + 0.2).min(1.0) / 100.0;
        let c = cm(hx, plate_y + 0.05, hz * zs);
        // The soundboard through the hole, its cast flange round it.
        g.lathe(c, up, &[(0.0, 0.001), (r, 0.001)], 40, m.spruce);
        g.lathe(
            c,
            up,
            &[
                (r, 0.001),
                (r + 0.004, 0.006),
                (r + 0.012, 0.004),
                (r + 0.016, 0.0),
            ],
            40,
            m.gold,
        );
    }
    for sx in [-30.0f32, 0.0, 30.0] {
        let (a, b) = (
            cm(sx, plate_y + 1.2, -40.0),
            cm(sx * 0.6 - 8.0, plate_y + 1.2, zmin * 0.85),
        );
        g.tube(a, b, 0.016, 0.014, 12, true, m.gold);
    }
    // The strings: wound copper in the bass, plain steel over them; the hitch pins.
    let back = |xq: f32| {
        let mm = o
            .iter()
            .filter(|&&(px, z)| (px - xq).abs() < 4.0 && z < -40.0)
            .map(|p| p.1)
            .fold(f32::MAX, f32::min);
        (if mm == f32::MAX { zmin } else { mm }) + 14.0
    };
    for i in 0..78 {
        let fr = i as f32 / 77.0;
        let (xa, xb) = if fr < 0.24 {
            let xa = -66.0 + 40.0 * (fr / 0.24);
            (xa, xa + 22.0)
        } else {
            let xa = -40.0 + 104.0 * ((fr - 0.24) / 0.76);
            (xa, xa * 0.92 - 4.0)
        };
        let bass = fr < 0.24;
        let hy = plate_y + if bass { 2.4 } else { 1.3 };
        let (mat, r) = if bass {
            (m.copper, 0.0022)
        } else {
            (m.steel, 0.0008)
        };
        for k in 0..if bass { 1 } else { 2 } {
            let dx = k as f32 * 0.45;
            g.line(
                cm(xa + dx, hy, -24.0),
                cm(xb + dx, hy, back(xb)),
                r,
                mat,
                if bass { 0.85 } else { 0.5 },
            );
        }
        g.tube(
            cm(xb, plate_y, back(xb) - 0.5),
            cm(xb, hy + 0.6, back(xb) - 0.5),
            0.0016,
            0.0016,
            6,
            true,
            m.nickel,
        );
    }
    // Tuning pins and the dampers' felt heads.
    for i in 0..60 {
        for j in 0..2 {
            let (x, z) = (
                -64.0 + i as f32 * 2.15,
                -12.0 - j as f32 * 3.0 - (i % 2) as f32,
            );
            g.tube(
                cm(x, plate_y, z),
                cm(x, plate_y + 2.4, z),
                0.0035,
                0.0035,
                8,
                true,
                m.nickel,
            );
        }
    }
    for i in 0..44 {
        let xa = -64.0 + 120.0 * i as f32 / 43.0;
        g.boxed(
            cm(xa, plate_y + 1.6, -26.0),
            cm(2.2, 0.0, 0.0),
            cm(0.0, 2.6, -1.0),
            cm(0.0, 0.6, -5.0),
            m.felt,
        );
    }
    keyboard(g, m);
    desk(g, m);
    // The main leaf behind the front flap's hinge, the flap folded back on it (round 40:
    // the desk stands in the open strip in front — `piano_geo`).
    lid_slab(g, m, body, &lid_outline(body, 14), ang, 0.0);
    lid_slab(g, m, body, &flap_outline(body, 14), ang, LID_TH);
    let ln = lid_normal(ang);
    let to_m = |p: crate::scene::geo::V3| cm(p.x, p.y, p.z);
    g.tube(
        to_m(lid_point(hinge_x(body) + 4.0, LID_FRONT, ang, body) + ln * (LID_TH * 0.5)),
        to_m(lid_point(-hinge_x(body) - 4.0, LID_FRONT, ang, body) + ln * (LID_TH * 0.5)),
        0.004,
        0.004,
        10,
        true,
        m.brass,
    );
    for hz in [LID_FRONT - 8.0, zmin * 0.5, zmin + 30.0] {
        let hx = hinge_x(body);
        g.aabb(
            cm(hx - 0.6, RIM_HI - 0.2, hz - 3.0),
            cm(hx + 1.6, RIM_HI + 1.8, hz + 3.0),
            m.brass,
        );
    }
    if let Some((foot, head)) = prop(body, lid) {
        g.tube(to_m(foot), to_m(head), 0.011, 0.012, 14, true, m.case);
    }
    bench(g, m);
}

/// The keys, their slip and bed, the fallboard with its emblem, the cheeks, the felt.
fn keyboard(g: &mut Geo, m: &Mats) {
    g.aabb(cm(-74.0, RIM_LO, 13.0), cm(74.0, 72.0, 15.6), m.case);
    g.aabb(cm(-62.0, 70.6, 0.0), cm(62.0, 72.6, 13.5), m.keybed);
    g.boxed(
        cm(-62.0, 74.0, -1.0),
        cm(124.0, 0.0, 0.0),
        cm(0.0, 7.0, -2.5),
        cm(0.0, 0.0, -1.8),
        m.case,
    );
    g.lathe(
        cm(0.0, 79.0, -2.6),
        v3(0.0, 0.34, 0.94),
        &[(0.0, 0.0012), (0.036, 0.0012), (0.04, 0.0)],
        32,
        m.brass,
    );
    g.tube(
        cm(-62.0, 74.5, -0.4),
        cm(62.0, 74.5, -0.4),
        0.004,
        0.004,
        8,
        false,
        m.red,
    );
    let w = 124.0 / 52.0;
    for i in 0..52 {
        let x = -62.0 + i as f32 * w;
        g.aabb(
            cm(x + 0.12, 72.6, 0.0),
            cm(x + w - 0.12, 74.0, 15.0),
            m.ivory,
        );
    }
    let pat = [true, false, true, true, false, true, true];
    for i in 0..51 {
        if !pat[i % 7] {
            continue;
        }
        let x = -62.0 + (i + 1) as f32 * w - w * 0.3;
        g.aabb(cm(x, 74.0, 0.0), cm(x + w * 0.6, 75.4, 6.0), m.ebony);
        g.boxed(
            cm(x + 0.1, 75.4, 6.0),
            cm(w * 0.6 - 0.2, 0.0, 0.0),
            cm(0.0, -1.4, 0.4),
            cm(0.0, 0.0, -0.6),
            m.ebony,
        );
    }
    for cx in [-68.5, 68.5] {
        g.aabb(cm(cx - 5.5, 70.0, -2.0), cm(cx + 5.5, 79.0, 15.0), m.case);
    }
}

/// The music rest (the drawn picture's): a board in the case's finish leaning back on its
/// ledge, fretwork either side (slats over a dark recess), the ledge and its lip, a sheet of
/// music open on it.
fn desk(g: &mut Geo, m: &Mats) {
    let on = |x: f32, t: f32, out: f32| {
        let p = desk_on(x, t, out);
        cm(p.x, p.y, p.z)
    };
    let a = on(-40.0, 0.0, 0.0);
    let (across, upv) = (on(40.0, 0.0, 0.0) - a, on(-40.0, 1.0, 0.0) - a);
    let fwd = cm(0.0, 0.0, 1.2);
    g.boxed(a, across, upv, fwd, m.desk);
    for side in [-1.0f32, 1.0] {
        let (xa, xb) = if side < 0.0 {
            (-36.0, -24.0)
        } else {
            (24.0, 36.0)
        };
        let r0 = on(xa, 0.1, 1.2);
        g.boxed(
            r0,
            on(xb, 0.1, 1.2) - r0,
            on(xa, 0.9, 1.2) - r0,
            cm(0.0, 0.0, 0.05),
            m.keybed,
        );
        for k in 0..6 {
            let x = xa + 0.6 + k as f32 * 2.2;
            let s0 = on(x, 0.1, 1.25);
            g.boxed(
                s0,
                cm(0.9, 0.0, 0.0),
                on(x, 0.9, 1.25) - s0,
                cm(0.0, 0.0, 0.45),
                m.desk,
            );
        }
        // The frame round the fretwork.
        for (t0, t1) in [(0.06, 0.1), (0.9, 0.94)] {
            let f0 = on(xa - 0.6, t0, 1.2);
            g.boxed(
                f0,
                on(xb + 0.6, t0, 1.2) - f0,
                on(xa - 0.6, t1, 1.2) - f0,
                cm(0.0, 0.0, 0.5),
                m.desk,
            );
        }
    }
    // The ledge and its lip.
    g.aabb(
        cm(-41.0, RIM_HI - 2.5, -18.0),
        cm(41.0, RIM_HI + 0.4, -14.5),
        m.case,
    );
    g.aabb(
        cm(-41.0, RIM_HI + 0.4, -15.3),
        cm(41.0, RIM_HI + 1.6, -14.5),
        m.case,
    );
    // The sheet, two pages, on the ledge against the board.
    let s0 = on(-22.0, 0.03, 1.3);
    super::upright::sheet(
        g,
        m,
        s0,
        on(22.0, 0.03, 1.3) - s0,
        on(-22.0, 0.94, 1.3) - s0,
    );
}

/// The lid: a slab 2.4 cm thick on its hinge, its top and underside, a rounded band round
/// its edge, its hinges.
fn lid_slab(g: &mut Geo, m: &Mats, body: Body, o: &[(f32, f32)], ang: f32, lift: f32) {
    let th = LID_TH / 100.0;
    let bev = 0.007;
    let ln = v3(-ang.sin(), ang.cos(), 0.0);
    let lift = ln * (lift / 100.0);
    let rot = |n: V3| v3(n.x * ang.cos(), n.x * ang.sin(), n.z);
    let om = to_m(o);
    let ns = outline_normals(&om, true);
    let at = |x: f32, z: f32| {
        let p = lid_point(x * 100.0, z * 100.0, ang, body);
        cm(p.x, p.y, p.z) + lift
    };
    let inset: Vec<(f32, f32)> = om
        .iter()
        .zip(&ns)
        .map(|(&(x, z), n)| {
            let nn = (n.0 + n.1).norm();
            (x - nn.x * bev, z - nn.z * bev)
        })
        .collect();
    g.plate_map(&inset, |x, z| at(x, z) + ln * 0.0, ln, m.case);
    g.plate_map(&inset, |x, z| at(x, z) - ln * th, -ln, m.under);
    // The band: top bevel, side, bottom bevel.
    let n = om.len();
    let mut rows: [Vec<(V3, V3)>; 4] = Default::default();
    for i in 0..=n {
        let i = i % n;
        let (x, z) = om[i];
        let (xi, zi) = inset[i];
        let out = rot((ns[i].0 + ns[i].1).norm());
        rows[0].push((at(xi, zi), ln));
        rows[1].push((at(x, z) - ln * bev, (out + ln * 0.35).norm()));
        rows[2].push((at(x, z) - ln * (th - bev), (out - ln * 0.35).norm()));
        rows[3].push((at(xi, zi) - ln * th, -ln));
    }
    g.strip(&rows[0], &rows[1], 0.0, bev, m.case);
    g.strip(&rows[1], &rows[2], bev, th - bev, m.case);
    g.strip(&rows[2], &rows[3], th - bev, th, m.under);
}

/// The bench in the piano's finish: a top with a rounded edge on its frame, four legs.
pub fn bench(g: &mut Geo, m: &Mats) {
    let by = 50.0;
    g.aabb(cm(-40.0, by - 6.0, 38.0), cm(40.0, by - 2.5, 70.0), m.case);
    let top = [
        (-0.395, 0.385),
        (0.395, 0.385),
        (0.395, 0.695),
        (-0.395, 0.695),
    ];
    let ry = by / 100.0;
    g.wall(&top, ry - 0.025, ry - 0.008, true, m.bench);
    g.round_edge(&top, ry, 0.008, false, m.bench);
    let inner = [
        (-0.387, 0.393),
        (0.387, 0.393),
        (0.387, 0.687),
        (-0.387, 0.687),
    ];
    g.plate(&inner, ry, true, m.bench);
    for (bx, bz) in [(-36.0, 66.0), (36.0, 66.0), (-36.0, 42.0), (36.0, 42.0)] {
        g.tube(
            cm(bx, 0.5, bz),
            cm(bx, by - 6.0, bz),
            0.016,
            0.019,
            16,
            true,
            m.case,
        );
        g.tube(
            cm(bx, 0.0, bz),
            cm(bx, 0.8, bz),
            0.017,
            0.017,
            16,
            true,
            m.rubber,
        );
    }
}
