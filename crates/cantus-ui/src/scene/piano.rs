//! Chorda's pianos (port of `scene/piano.py`): a concert grand (2.74 m), a grand (2.00 m),
//! a parlour grand (1.55 m) — one outline scaled — and an upright, in any finish, the lid at
//! its angle (grands 0–36°: closed, the short stick near 20°, the full stick; the upright's
//! top 0–24°). Built in cm: the keys' front at z = +15, the case running back to −z, the
//! treble (the bentside) on +x, a grand's lid hinged on the bass side.

use super::display::{lin, Cap, Paint, Part};
use super::geo::{hull, ring, v, Camera, V3};
use super::light::{grain, lerp, mul, scale, scale_mat};
use super::piano_geo::{
    upright_leaf, LID_FRONT, LID_TH, UP_BENCH_DZ, UP_BLOCK, UP_D, UP_H, UP_LEAF_HINGE, UP_LEG,
    UP_LEG_X, UP_SHELF_Y, UP_SIDE, UP_TOE, UP_TOP_TH, UP_W,
};
use super::prims::{
    boxy, extrude, facet, facet0, facet_v, facet_v_bent, lacquer_band, tube, Ctx, OWN, OWN_BUTT,
};
use super::Room;
use crate::kit::skin::scene::{ink, mat, Mat, C3, FINISH_WALNUT};
use cantus_core::inst::chorda_look::Body;
use egui::Pos2;

pub const RIM_LO: f32 = 64.0;
pub const RIM_HI: f32 = 94.0;
const CTRL: [(f32, f32); 16] = [
    (-74.0, 0.0),
    (-74.0, -120.0),
    (-74.0, -226.0),
    (-66.0, -242.0),
    (-48.0, -248.0),
    (-26.0, -242.0),
    (-6.0, -228.0),
    (14.0, -206.0),
    (30.0, -178.0),
    (42.0, -148.0),
    (52.0, -116.0),
    (60.0, -86.0),
    (67.0, -56.0),
    (72.0, -28.0),
    (74.0, -8.0),
    (74.0, 0.0),
];

/// The full stick (deg) of a grand's lid, and an upright's top.
pub const GRAND_LID: f32 = 36.0;
pub const UPRIGHT_LID: f32 = 24.0;
/// The short stick (deg).
pub const SHORT_STICK: f32 = 20.0;

pub fn length(body: Body) -> f32 {
    match body {
        Body::Concert => 274.0,
        Body::Grand => 200.0,
        Body::Parlour | Body::Upright => 155.0,
    }
}

fn xs(body: Body) -> f32 {
    if body == Body::Parlour {
        0.96
    } else {
        1.0
    }
}

fn cr(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let f = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * (2.0 * b
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
            + (-a + 3.0 * b - 3.0 * c + d) * t * t * t)
    };
    (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
}

/// A grand's outline in plan (x, z), `n` points a span of its spline.
pub fn outline(body: Body, n: usize) -> Vec<(f32, f32)> {
    let zs = length(body) / 248.0;
    let xk = xs(body);
    let mut c = Vec::with_capacity(CTRL.len() + 2);
    c.push(CTRL[0]);
    c.extend_from_slice(&CTRL);
    c.push(CTRL[CTRL.len() - 1]);
    let mut pts = Vec::with_capacity((c.len() - 3) * n);
    for i in 1..c.len() - 2 {
        for k in 0..n {
            let (x, z) = cr(c[i - 1], c[i], c[i + 1], c[i + 2], k as f32 / n as f32);
            pts.push((x * xk, z * zs));
        }
    }
    pts
}

pub use super::piano_geo::{hinge_x, lid_point};

/// The points a camera frames: every grand as the concert grand is, so sizes read true.
pub fn points(body: Body, lid: f32) -> Vec<V3> {
    if body == Body::Upright {
        let mut p = Vec::new();
        // The same frame whatever the top's leaf does (it folds back within it).
        let _ = lid;
        for x in [-80.0, 80.0] {
            for y in [0.0, 154.0] {
                for z in [-62.0, 30.0 + UP_BENCH_DZ + 40.0] {
                    p.push(v(x, y, z));
                }
            }
        }
        return p;
    }
    let o = outline(Body::Concert, 2);
    let a = GRAND_LID.to_radians();
    let mut pts: Vec<V3> = Vec::new();
    for &(x, z) in &o {
        pts.push(v(x, 0.0, z));
        pts.push(v(x, RIM_HI, z));
    }
    pts.extend(o.iter().map(|&(x, z)| lid_point(x, z, a, Body::Concert)));
    pts.extend([v(-50.0, 0.0, 70.0), v(50.0, 0.0, 70.0), v(0.0, 0.0, 60.0)]);
    pts
}

/// The room's elevation: a bigger room is seen from lower down, its walls rising into the
/// frame.
pub fn elevation(room: Room) -> f32 {
    match room {
        Room::Stage => 15.0,
        Room::Booth => 26.0,
        Room::Studio => 24.0,
        Room::Hall => 15.0,
        Room::Church => 12.0,
    }
}

/// The calm three-quarter view from the audience's side.
pub fn camera(body: Body, room: Room) -> Camera {
    let el = elevation(room);
    if body == Body::Upright {
        return Camera::new(v(0.0, 72.0, -10.0), 34.0, (el - 8.0).max(10.0), 520.0);
    }
    let l = length(Body::Concert);
    Camera::new(v(0.0, 80.0, -0.42 * l), 50.0, el, 520.0 + l)
}

fn q(c: &mut Ctx, pts: &[V3], col: C3, seam: f32) {
    c.quad(pts, col, 1.0, seam);
}

fn leg(c: &mut Ctx, x: f32, z: f32, top: f32, fin: &Mat) {
    tube(c, v(x, 6.0, z), v(x, top * 0.55, z), 3.4, fin, OWN_BUTT);
    tube(c, v(x, top * 0.55, z), v(x, top, z), 4.2, fin, OWN_BUTT);
    tube(
        c,
        v(x, top * 0.55 - 1.0, z),
        v(x, top * 0.55 + 1.0, z),
        4.5,
        fin,
        OWN_BUTT,
    );
    tube(c, v(x, 4.5, z), v(x, 8.5, z), 3.5, &mat::BRASS, OWN_BUTT);
    tube(
        c,
        v(x - 1.5, 2.3, z),
        v(x + 1.5, 2.3, z),
        2.3,
        &mat::BLACK,
        OWN,
    );
    c.contacts.push((v(x, 0.0, z), 5.5));
}

/// The piano `body` in `fin`, its lid at `lid` degrees.
pub fn build(c: &mut Ctx, body: Body, fin: &Mat, white: bool, black: bool, lid: f32) {
    if body == Body::Upright {
        upright(c, fin, lid);
    } else {
        grand(c, body, fin, white, black, lid);
    }
}

fn grand(c: &mut Ctx, body: Body, f: &Mat, white: bool, black: bool, lid: f32) {
    let cam = c.cam;
    let ang = lid.to_radians();
    let o = outline(body, 14);
    let zmin = o.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    c.piece = Part::Piano;
    // Legs, lyre and pedals.
    for (lx, lz) in [(-62.0, -6.0), (62.0, -6.0), (-30.0, zmin + 30.0)] {
        leg(c, lx, lz, RIM_LO, f);
    }
    for px in [-9.0, 9.0] {
        tube(
            c,
            v(px, 12.0, -10.0),
            v(px, RIM_LO, -10.0),
            1.7,
            f,
            OWN_BUTT,
        );
    }
    tube(c, v(-15.0, 10.0, -10.0), v(15.0, 10.0, -10.0), 3.4, f, OWN);
    for px in [-7.0, 0.0, 7.0] {
        tube(c, v(px, 7.5, -6.0), v(px, 5.8, 7.0), 1.2, &mat::BRASS, OWN);
    }
    // The case's outer skin.
    c.p.at(cam.depth(v(0.0, 80.0, zmin * 0.45)) + 140.0, Part::Piano);
    case_skin(c, &o, f);
    // The inside: the plate, its holes and struts, the strings, pins, dampers, inner rim.
    let inner_dep = cam.depth(v(0.0, RIM_HI, zmin * 0.45)) + 60.0;
    c.p.at(inner_dep, Part::Piano);
    let plate_y = RIM_HI - 7.0;
    let ins: Vec<(f32, f32)> = o.iter().map(|&(x, z)| (x * 0.94, z * 0.97 - 2.0)).collect();
    let up = v(0.0, 1.0, 0.0);
    let gc = v(0.0, plate_y, zmin / 2.0);
    let gold = c.shade(&mat::GOLD, up, gc);
    // Rich, never blown out.
    let k = (1.0 / (c.light.m.key * 0.78).max(1e-3)).min(1.0);
    let gold = [
        (gold[0] * k).min(1.0),
        (gold[1] * k).min(1.0),
        (gold[2] * k).min(1.0),
    ];
    let plate3: Vec<V3> = ins.iter().map(|&(x, z)| v(x, plate_y, z)).collect();
    c.keep_facet(&plate3, up, &mat::GOLD, 1.0);
    let pts: Vec<Pos2> = ins.iter().map(|&(x, z)| cam.p(v(x, plate_y, z))).collect();
    // A cast frame's gilt, not a flat yellow: a little less saturated, bright where the
    // key's pool falls near the keys and falling off toward the tail under the lid (round
    // 40 cp6: the inside read as one saturated orange-yellow sheet beside the photograph's).
    let gy = 0.2126 * gold[0] + 0.7152 * gold[1] + 0.0722 * gold[2];
    let gold = scale(lerp(gold, [gy; 3], 0.32), 0.92);
    let (front, tail) = (
        cam.p(v(0.0, plate_y, -12.0)),
        cam.p(v(0.0, plate_y, zmin * 0.95)),
    );
    c.p.poly(
        pts.clone(),
        lin(
            front,
            tail,
            &[
                (0.0, scale(gold, 1.08)),
                (0.45, scale(gold, 0.82)),
                (1.0, scale(gold, 0.50)),
            ],
        ),
        1.0,
        0.0,
    );
    // The gap between the frame and the inner rim: a dark line all round.
    c.p.polyline(pts, 2.6, Paint::Solid(ink::BLACK_EDGE), 0.5, true);
    let zs = length(body) / 248.0;
    for (hx, hz, hr) in [
        (-40.0, -150.0, 13.0),
        (-10.0, -175.0, 11.0),
        (18.0, -140.0, 10.0),
        (-46.0, -70.0, 9.0),
        (30.0, -95.0, 8.0),
    ] {
        let r = ring(
            v(hx, plate_y + 0.2, hz * zs),
            up,
            hr * (zs + 0.2).min(1.0),
            32,
            0.0,
        );
        let sp = c.sps(&r);
        c.p.poly(sp, Paint::Solid(scale(gold, 0.35)), 1.0, 0.0);
    }
    // The struts: the frame's bars toward the tail.
    for sx in [-30.0f32, 0.0, 30.0] {
        let (a, b) = (
            cam.p(v(sx, plate_y + 1.5, -40.0)),
            cam.p(v(sx * 0.6 - 8.0, plate_y + 1.5, zmin * 0.85)),
        );
        let w = (4.0 * cam.s / cam.depth(v(0.0, 90.0, -100.0))).max(2.0);
        c.p.line(a, b, w, Paint::Solid(scale(gold, 1.05)), Cap::Round, 1.0);
    }
    // The strings over the frame: each its shadow on the gilt, then the string lit by the
    // key's pool — the wound bass in copper crossing over the steel treble (round 40 cp7:
    // faint lines read as a plain gilt plate).
    for (si, (sa, sb, bass)) in strings(&o, zmin, plate_y).into_iter().enumerate() {
        // Thin golden-copper lines with the frame between them, as the photograph shows
        // them (round 40 cp8: drawn wide, side by side, they merged into an orange band):
        // every other wound string drawn, one in three of those a little wider.
        if bass && si % 2 == 1 {
            continue;
        }
        let lead = si % 6 == 0 || (!bass && si % 3 == 0);
        let col = if bass {
            ink::BASS_STRING
        } else {
            ink::STEEL_STRING
        };
        let lit = c.light.spot((sa + sb) * 0.5).clamp(0.0, 1.0);
        let col = if bass {
            let cu = mul(scale(col, 0.5 + 0.45 * lit), ink::COPPER_WARM);
            let y = 0.2126 * cu[0] + 0.7152 * cu[1] + 0.0722 * cu[2];
            lerp(cu, [y; 3], 0.3)
        } else {
            scale(col, 0.55 + 0.75 * lit)
        };
        // The wound bass twice the treble's width and more (round 40 cp8: copper lines
        // to read at a glance), the treble fine bright steel.
        let w = match (bass, lead) {
            (true, true) => 1.2,
            (true, false) => 1.0,
            _ => 0.42,
        };
        let drop = v(0.0, -(sa.y - plate_y) + 0.05, 1.2);
        let (sa2, sb2) = (cam.p(sa + drop), cam.p(sb + drop));
        c.p.line(
            sa2,
            sb2,
            w + 0.5,
            Paint::Solid(ink::BLACK_EDGE),
            Cap::Round,
            0.16,
        );
        let (a, b) = (cam.p(sa), cam.p(sb));
        c.p.line(
            a,
            b,
            w,
            Paint::Solid(col),
            Cap::Round,
            // Fine lines over the gilt, never a sheet of steel over it.
            match (bass, lead) {
                (true, true) => 0.8,
                (true, false) => 0.55,
                _ => 0.42,
            },
        );
        // The key's glint along the string where its pool falls: a bright stretch (the
        // wide bass strings and one treble string in four).
        if lit > 0.2 && lead && (bass || si % 4 == 0) {
            let (ga, gb) = (cam.p(sa + (sb - sa) * 0.30), cam.p(sa + (sb - sa) * 0.62));
            let gl = lerp(col, ink::GLINT, 0.55);
            c.p.line(
                ga,
                gb,
                if bass { 0.7 } else { 0.3 },
                Paint::Solid(gl),
                Cap::Round,
                (if bass { 0.7 } else { 0.35 }) * lit,
            );
        }
    }
    // Tuning pins: a field of small nickel dots near the front.
    for i in 0..60 {
        for j in 0..2 {
            let p = cam.p(v(
                -64.0 + i as f32 * 2.15,
                plate_y + 2.0,
                -12.0 - j as f32 * 3.0 - (i % 2) as f32,
            ));
            c.p.ellipse(p, 0.7, 0.45, Paint::Solid(ink::PIN), 0.9);
        }
    }
    // The dampers' row: black felted heads, each with its lit top edge.
    let dl = scale(ink::PIN, 0.55);
    for i in 0..60 {
        let xa = -64.0 + 120.0 * i as f32 / 59.0;
        let (ta, tb) = (
            cam.p(v(xa, plate_y + 3.6, -31.0)),
            cam.p(v(xa + 1.5, plate_y + 3.6, -31.0)),
        );
        facet0(
            c,
            &[
                v(xa, plate_y + 1.6, -26.0),
                v(xa + 1.6, plate_y + 1.6, -26.0),
                v(xa + 1.6, plate_y + 3.6, -31.0),
                v(xa, plate_y + 3.6, -31.0),
            ],
            &mat::BLACK,
            up,
        );
        c.p.line(ta, tb, 0.5, Paint::Solid(dl), Cap::Butt, 0.6);
    }
    let inner_o: Vec<(f32, f32)> = o.iter().map(|&(x, z)| (x * 0.97, z * 0.98)).collect();
    let inner_mat = if f.wood { *f } else { FINISH_WALNUT };
    extrude(c, &inner_o, plate_y, RIM_HI, &inner_mat, false, 2.1, 1.0);
    let top: Vec<Pos2> = o.iter().map(|&(x, z)| cam.p(v(x, RIM_HI, z))).collect();
    let rim = c.shade(f, up, v(0.0, RIM_HI, -100.0));
    c.p.polyline(top, 2.2, Paint::Solid(rim), 1.0, true);
    keyboard(c, f);
    // The music desk; the lid lies over the inside (the eye always above it), the desk in
    // front of the lid.
    let lid_dep = inner_dep - 5.0;
    c.p.at(
        (cam.depth(v(0.0, 108.0, -22.0)) + 20.0).min(lid_dep - 1.0),
        Part::Piano,
    );
    // The music desk as a grand's music rest: a board in the case's finish leaning back on
    // its ledge, fretwork slats either side, and a sheet of music open on the ledge (round
    // 40 phase 3: a dark tablet with a ring read as a screen, not a desk).
    let dn = v(0.0, 0.3, 1.0).norm();
    let dc = v(0.0, 108.0, -22.0);
    // A point on the desk's plane: x across, t from the ledge (0) to its top (1), `out`
    // toward the player.
    let on = super::piano_geo::desk_on;
    let dsk = [
        on(-40.0, 0.0, 0.0),
        on(40.0, 0.0, 0.0),
        on(40.0, 1.0, 0.0),
        on(-40.0, 1.0, 0.0),
    ];
    // The board in the case's own finish, shaded down its height like the case's faces
    // (round 40 cp6: a mid-grey board with white bars read as a cartoon).
    let dcol = c.shade(f, dn, dc);
    let board = if black { scale_mat(f, 0.45) } else { *f };
    facet_v(c, &dsk, &board, dn, 1.0, (0.85, 0.95), 0.10, 0.5);
    let dcol = if black { scale(dcol, 0.45) } else { dcol };
    // The fretwork either side of the sheet: cut through to the dark behind the desk —
    // a recess, then the board's slats left standing over it, a fine light edge on each.
    let slat = scale(dcol, if black { 1.0 } else { 0.95 });
    let edge = scale(dcol, if black { 1.8 } else { 1.15 });
    for side in [-1.0f32, 1.0] {
        let (xa, xb) = (side * 24.0, side * 36.5);
        let hole = [
            on(xa.min(xb), 0.1, 0.05),
            on(xa.max(xb), 0.1, 0.05),
            on(xa.max(xb), 0.9, 0.05),
            on(xa.min(xb), 0.9, 0.05),
        ];
        q(c, &hole, ink::BLACK_EDGE, 0.0);
        for k in 0..6 {
            let x = side * (24.6 + 2.3 * k as f32);
            let (a, b) = (cam.p(on(x, 0.1, 0.2)), cam.p(on(x, 0.9, 0.2)));
            c.p.line(a, b, 1.1, Paint::Solid(slat), Cap::Butt, 1.0);
            let (a2, b2) = (cam.p(on(x - 0.4, 0.1, 0.25)), cam.p(on(x - 0.4, 0.9, 0.25)));
            c.p.line(a2, b2, 0.35, Paint::Solid(edge), Cap::Butt, 0.5);
        }
        // A rail across the fretwork's middle.
        let (a, b) = (cam.p(on(xa, 0.5, 0.2)), cam.p(on(xb, 0.5, 0.2)));
        c.p.line(a, b, 1.0, Paint::Solid(slat), Cap::Butt, 1.0);
    }
    // The frame's moulding: a fine line, the finish's own sheen.
    let frm = [
        on(-37.0, 0.08, 0.2),
        on(37.0, 0.08, 0.2),
        on(37.0, 0.92, 0.2),
        on(-37.0, 0.92, 0.2),
    ];
    let sp = c.sps(&frm);
    c.p.polyline(sp, 0.6, Paint::Solid(edge), 0.45, true);
    // The ledge the music stands on: its top and its front edge.
    let ledge = [
        on(-41.0, 0.0, 0.0),
        on(41.0, 0.0, 0.0),
        on(41.0, 0.0, 3.5),
        on(-41.0, 0.0, 3.5),
    ];
    q(c, &ledge, scale(dcol, if black { 1.4 } else { 1.05 }), 0.5);
    let lip = [
        on(-41.0, 0.0, 3.5),
        on(41.0, 0.0, 3.5),
        v(41.0, 91.5, -14.5),
        v(-41.0, 91.5, -14.5),
    ];
    q(c, &lip, scale(dcol, 0.8), 0.5);
    // The sheet: two pages open on the ledge, a fold down the middle, staves across.
    let sheet = [
        on(-22.0, 0.03, 0.8),
        on(22.0, 0.03, 0.8),
        on(22.0, 0.94, 0.8),
        on(-22.0, 0.94, 0.8),
    ];
    // Paper a little warm under the stage's key (a page, not a lit screen).
    let pc = mul(c.shade(&mat::PAPER, dn, dc), ink::PAPER_WARM);
    q(c, &sheet, pc, 0.5);
    let ink = mat::STAFF;
    let fold = (cam.p(on(0.0, 0.03, 0.9)), cam.p(on(0.0, 0.94, 0.9)));
    c.p.line(
        fold.0,
        fold.1,
        0.6,
        Paint::Solid(scale(pc, 0.8)),
        Cap::Butt,
        0.8,
    );
    for page in [-1.0f32, 1.0] {
        let (x0, x1) = if page < 0.0 {
            (-20.0, -2.0)
        } else {
            (2.0, 20.0)
        };
        for system in 0..5 {
            let top = 0.86 - system as f32 * 0.16;
            for l in 0..5 {
                let t = top - l as f32 * 0.022;
                let a = cam.p(on(x0, t, 0.9));
                let b = cam.p(on(x1, t, 0.9));
                c.p.line(a, b, 0.35, Paint::Solid(ink), Cap::Butt, 0.45);
            }
        }
    }
    // The lid: closed on the rim, or on its stick with its underside toward us.
    // The main leaf behind the front flap's hinge (round 40: the flap folds back over it, so
    // the desk's strip is open and nothing of the lid reaches the desk — `piano_geo`).
    let lo = super::piano_geo::lid_outline(body, 14);
    let ln = super::piano_geo::lid_normal(ang);
    let lm0 = lid_point(0.0, zmin * 0.45, ang, body);
    let under = (-ln).dot(c.view(lm0)) > 0.0;
    // The face seen: the top, or the underside a lid's thickness below it.
    let off = if under {
        -ln * LID_TH
    } else {
        v(0.0, 0.0, 0.0)
    };
    let lpt = |x: f32, z: f32| lid_point(x, z, ang, body) + off;
    let lid_pts: Vec<V3> = lo.iter().map(|&(x, z)| lpt(x, z)).collect();
    let lp = c.sps(&lid_pts);
    let lm = lpt(0.0, zmin * 0.45);
    let face = if under { -ln } else { ln };
    let hx = hinge_x(body);
    c.p.at(lid_dep, Part::Lid);
    c.piece = Part::Lid;
    // Seen from below, the flap folded on the top lies behind the lid: its edge first.
    if under {
        flap(c, body, f, ang, false);
    }
    c.keep_facet(&lid_pts, face, f, 1.0);
    // A solid lacquered board: in strips along its length, each shaded across at nine
    // points, so what it reflects (the stage's pool of light, the strip lights' streaks)
    // moves over it as over a mirror; never a tinted glass. Seen from below, it mirrors
    // what it opens over — the gold frame, its holes and struts, the strings — drawn
    // mirrored in the lid's plane and cut to its outline (`lid_mirror`), as a photograph
    // of an open grand shows.
    let _ = white;
    // The board whole underneath (no hairline of what is behind between two strips), then
    // its strips — about three points each on screen, at most 72 — without seams: a seam
    // in each strip's own gradient drew a moiré of curved lines over a small closed lid.
    let base = c.shade_g(f, face, lm, 1.0);
    c.p.poly(lp.clone(), Paint::Solid(base), 1.0, 0.0);
    let run = cam.p(lpt(0.0, zmin)).distance(cam.p(lpt(0.0, LID_FRONT)));
    let n_strips = ((run / 3.0).round() as usize).clamp(12, 72);
    for i in 0..n_strips {
        let (za, zb) = (
            zmin + (LID_FRONT - zmin) * i as f32 / n_strips as f32,
            zmin + (LID_FRONT - zmin) * (i + 1) as f32 / n_strips as f32,
        );
        let band = slab(&lo, za - 0.3, zb + 0.3);
        if band.len() < 3 {
            continue;
        }
        let pts3: Vec<V3> = band.iter().map(|&(x, z)| lpt(x, z)).collect();
        let zm = (za + zb) / 2.0;
        let mut stops = [(0.0, [0.0; 3]); 13];
        for (k, st) in stops.iter_mut().enumerate() {
            let fr = k as f32 / 12.0;
            let pp = lpt(hx + 148.0 * fr, zm);
            *st = (fr, c.shade_g(f, face, pp, grain(fr * 3.0, 1.0, f.figure)));
        }
        let (a0, a1) = (cam.p(lpt(hx, zm)), cam.p(lpt(-hx, zm)));
        let sp = c.sps(&pts3);
        c.p.poly(sp, lin(a0, a1, &stops), 1.0, 0.0);
    }
    if under && lid > 3.0 {
        let plane = (lpt(0.0, 0.0), ln);
        // The mirror's mask: across the board from the edge the frame's reflection lies
        // along (where the mirror angle sends it), fading quickly.
        let zm = (zmin + LID_FRONT) / 2.0;
        let across = (cam.p(lpt(hx, zm)), cam.p(lpt(-hx, zm)));
        lid_mirror(c, &o, &ins, (zmin, zs), plate_y, gold, plane, &lp, across);
    }
    // The lid's thickness: its edge band where it faces us, and the bevel's thin light line.
    let th = if under { -ln * LID_TH } else { ln * LID_TH };
    let lc = lm;
    for k in 0..lid_pts.len() {
        let (a, b) = (lid_pts[k], lid_pts[(k + 1) % lid_pts.len()]);
        let mid = (a + b) * 0.5;
        let out_n = (mid - lc).norm();
        if !cam.facing(mid, out_n) {
            continue;
        }
        // Shaded along its run, the strip lights' streak a crisp band along it.
        facet_v(
            c,
            &[a, b, b - th, a - th],
            f,
            out_n,
            1.0,
            (1.0, 1.0),
            0.0,
            0.5,
        );
    }
    // Its edge: the board's dark thickness round it (a black lid reads as a solid board,
    // not a pane), then a thin light line, never brighter than a lacquer's sheen.
    if black {
        c.p.polyline(lp.clone(), 2.0, Paint::Solid(ink::BLACK_EDGE), 0.85, true);
    }
    let edge = c.shade(f, (face + v(0.0, 0.6, 0.0)).norm(), lm);
    let lum = edge[0].max(edge[1]).max(edge[2]).max(1e-3);
    // On black lacquer the rim's bevel is the lid's brightest line (the reference's).
    let edge = scale(edge, ((if black { 0.72 } else { 0.42 }) / lum).min(2.5));
    c.p.polyline(lp.clone(), 0.8, Paint::Solid(edge), 0.85, true);
    // Seen from above, the flap folded back lies on the lid.
    if !under {
        flap(c, body, f, ang, true);
    }
    // The prop stick from its cup on the bentside's rim to the lid's underside.
    if let Some((foot, head)) = super::piano_geo::prop(body, lid) {
        tube(c, foot, head, 1.1, f, OWN);
    }
    c.p.hit(
        Part::Lid,
        hull(&lp),
        cam.depth(v(0.0, 80.0, zmin / 2.0)) - 1.0,
    );
    // The bench.
    bench(c, f, 0.0);
    let mut sh: Vec<V3> = o.iter().step_by(3).map(|&(x, z)| v(x, RIM_HI, z)).collect();
    sh.extend(o.iter().step_by(3).map(|&(x, z)| v(x, RIM_LO, z)));
    c.shadow_polys.push((sh, 0.75));
    if lid > 1.0 {
        let lsh: Vec<V3> = lo
            .iter()
            .step_by(3)
            .map(|&(x, z)| lid_point(x, z, ang, body))
            .collect();
        c.shadow_polys.push((lsh, 0.25));
    }
    let mut hp: Vec<Pos2> = Vec::new();
    for &(x, z) in o.iter().step_by(2) {
        hp.push(cam.p(v(x, 0.0, z)));
        hp.push(cam.p(v(x, RIM_HI, z)));
    }
    hp.extend(lp);
    c.p.hit(Part::Piano, hull(&hp), cam.depth(v(0.0, 80.0, zmin / 2.0)));
}

/// The front flap folded back on the lid (`piano_geo::flap_outline`): its edge band where it
/// faces us — a doubled edge at the lid's front — and, seen from above, its top with the
/// lid's sheen and the hinge's brass line along its fold.
fn flap(c: &mut Ctx, body: Body, f: &Mat, ang: f32, top: bool) {
    let cam = c.cam;
    let ln = super::piano_geo::lid_normal(ang);
    let fo = super::piano_geo::flap_outline(body, 14);
    let at = |x: f32, z: f32| lid_point(x, z, ang, body) + ln * LID_TH;
    let pts: Vec<V3> = fo.iter().map(|&(x, z)| at(x, z)).collect();
    if pts.len() < 3 {
        return;
    }
    let ctr = super::geo::centre(&pts);
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        let mid = (a + b) * 0.5;
        let out_n = v(mid.x - ctr.x, mid.y - ctr.y, mid.z - ctr.z).norm();
        if !cam.facing(mid, out_n) {
            continue;
        }
        facet_v(
            c,
            &[a - ln * LID_TH, b - ln * LID_TH, b, a],
            f,
            out_n,
            1.0,
            (0.9, 1.0),
            0.0,
            0.5,
        );
    }
    if top {
        c.keep_facet(&pts, ln, f, 1.0);
        let sp = c.sps(&pts);
        let (a0, a1) = (
            cam.p(at(hinge_x(body), LID_FRONT * 1.5)),
            cam.p(at(-hinge_x(body), LID_FRONT * 1.5)),
        );
        let mut stops = [(0.0, [0.0; 3]); 9];
        for (k, st) in stops.iter_mut().enumerate() {
            let fr = k as f32 / 8.0;
            let p = at(hinge_x(body) + 148.0 * fr, LID_FRONT * 1.5);
            *st = (fr, c.shade_g(f, ln, p, grain(fr * 3.0, 2.0, f.figure)));
        }
        c.p.poly(sp, lin(a0, a1, &stops), 1.0, 0.0);
        let hc = c.shade(&mat::BRASS, ln, at(0.0, LID_FRONT));
        let (ha, hb) = (
            cam.p(at(hinge_x(body) + 4.0, LID_FRONT)),
            cam.p(at(-hinge_x(body) - 4.0, LID_FRONT)),
        );
        c.p.line(ha, hb, 0.8, Paint::Solid(hc), Cap::Butt, 0.9);
    }
}

/// The strings, plan and height: from the hitch near the keys to the case's back, the bass
/// crossing over the treble (`(front, back, bass)`).
fn strings(o: &[(f32, f32)], zmin: f32, plate_y: f32) -> Vec<(V3, V3, bool)> {
    let back = |xq: f32| {
        let m = o
            .iter()
            .filter(|&&(px, z)| (px - xq).abs() < 4.0 && z < -40.0)
            .map(|p| p.1)
            .fold(f32::MAX, f32::min);
        (if m == f32::MAX { zmin } else { m }) + 14.0
    };
    (0..110)
        .map(|i| {
            let fr = i as f32 / 109.0;
            let bass = fr < 0.24;
            let (xa, xb) = if bass {
                let xa = -66.0 + 40.0 * (fr / 0.24);
                (xa, xa + 22.0)
            } else {
                let xa = -40.0 + 104.0 * ((fr - 0.24) / 0.76);
                (xa, xa * 0.92 - 4.0)
            };
            let hy = plate_y + if bass { 2.4 } else { 1.3 };
            (v(xa, hy, -24.0), v(xb, hy, back(xb)), bass)
        })
        .collect()
}

/// The open lid's underside as a mirror: the gold frame, its holes and struts and the
/// strings, reflected in the lid's plane (`plane`: a point of it and its normal) and cut to
/// its outline `clip` (screen) — black lacquer shows them dimmer and warmer.
#[allow(clippy::too_many_arguments)]
fn lid_mirror(
    c: &mut Ctx,
    o: &[(f32, f32)],
    ins: &[(f32, f32)],
    (zmin, zs): (f32, f32),
    plate_y: f32,
    gold: C3,
    plane: (V3, V3),
    clip: &[Pos2],
    (ha, fa): (Pos2, Pos2),
) {
    let (l0, n) = plane;
    // How far across the board from the hinge side a point on screen lies (0 … 1): the
    // mirror's angle sends the frame's reflection into a band near the hinge, the rest of
    // the board black (round 40 cp5: an even reflection over the whole lid read as a pane
    // one sees through).
    let span = fa - ha;
    let s2 = span.length_sq().max(1.0);
    let across = move |p: Pos2| ((p - ha).dot(span) / s2).clamp(0.0, 1.0);
    let fade = move |p: Pos2| {
        let t = ((across(p) - 0.08) / 0.30).clamp(0.0, 1.0);
        1.0 - t * t * (3.0 - 2.0 * t)
    };
    let cam = c.cam;
    let m = |p: V3| cam.p(p - n * (2.0 * (p - l0).dot(n)));
    // Black lacquer's reflectance: a warm copy over the black, dimming toward the tail
    // where the mirror looks into the dark (round 40 cp4: at 0.62 over the whole frame the
    // lid read as a tinted glass one saw through, not a black mirror).
    let k = 0.42;
    let plate: Vec<Pos2> = ins.iter().map(|&(x, z)| m(v(x, plate_y, z))).collect();
    c.p.poly_clip(
        plate,
        lin(
            ha,
            fa,
            &[
                (0.0, scale(gold, 0.80)),
                (0.14, scale(gold, 0.50)),
                (0.30, scale(gold, 0.10)),
                (0.42, ink::BLACK_EDGE),
                (1.0, ink::BLACK_EDGE),
            ],
        ),
        k,
        clip.to_vec(),
    );
    for (hx, hz, hr) in [
        (-40.0, -150.0, 13.0),
        (-10.0, -175.0, 11.0),
        (18.0, -140.0, 10.0),
        (-46.0, -70.0, 9.0),
        (30.0, -95.0, 8.0),
    ] {
        let r: Vec<Pos2> = ring(
            v(hx, plate_y + 0.2, hz * zs),
            v(0.0, 1.0, 0.0),
            hr * (zs + 0.2).min(1.0),
            20,
            0.0,
        )
        .into_iter()
        .map(m)
        .collect();
        let fh = r.first().map_or(0.0, |&p| fade(p));
        if fh > 0.02 {
            c.p.poly_clip(
                r,
                Paint::Solid(scale(gold, 0.16)),
                k * 0.5 * fh,
                clip.to_vec(),
            );
        }
    }
    // The strings: each cut to the lid's outline on screen (sampled along its length).
    let inside = |p: Pos2| super::geo::inside(clip, p);
    for (sa, sb, bass) in strings(o, zmin, plate_y) {
        let (a, b) = (m(sa), m(sb));
        let n = 24;
        let mut run: Option<(f32, f32)> = None;
        let mut best: Option<(f32, f32)> = None;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            if inside(a + (b - a) * t) {
                run = Some(run.map_or((t, t), |r| (r.0, t)));
            } else if let Some(r) = run.take() {
                best = Some(best.map_or(r, |b| if r.1 - r.0 > b.1 - b.0 { r } else { b }));
            }
        }
        if let Some(r) = run {
            best = Some(best.map_or(r, |b| if r.1 - r.0 > b.1 - b.0 { r } else { b }));
        }
        let Some((t0, t1)) = best else { continue };
        if t1 - t0 < 0.05 {
            continue;
        }
        let fs = fade(a + (b - a) * ((t0 + t1) * 0.5));
        if fs < 0.02 {
            continue;
        }
        let col = if bass {
            ink::BASS_STRING
        } else {
            ink::STEEL_STRING
        };
        c.p.line(
            a + (b - a) * t0,
            a + (b - a) * t1,
            if bass { 0.7 } else { 0.45 },
            Paint::Solid(col),
            Cap::Butt,
            if bass { 0.5 } else { 0.38 } * fs,
        );
    }
}

/// A grand's rim: each strip of the side shaded along its height (rounded a little, darker
/// at its foot), then the rounded top edge as a narrow bevel catching the light as one crisp
/// line.
fn case_skin(c: &mut Ctx, o: &[(f32, f32)], f: &Mat) {
    let n = o.len();
    // Each strip's normal, and at each corner the mean of the two that meet there.
    let seg_n: Vec<V3> = (0..n)
        .map(|k| {
            let (x0, z0) = o[k];
            let (x1, z1) = o[(k + 1) % n];
            v(z1 - z0, 0.0, -(x1 - x0)).norm()
        })
        .collect();
    let corner = |k: usize| (seg_n[(k + n - 1) % n] + seg_n[k]).norm();
    let mut bands = Vec::new();
    for k in 0..n {
        let (x0, z0) = o[k];
        let (x1, z1) = o[(k + 1) % n];
        let (nx, nz) = (z1 - z0, -(x1 - x0));
        let l = nx.hypot(nz);
        let l = if l > 0.0 { l } else { 1.0 };
        let nrm = v(nx / l, 0.0, nz / l);
        let pm = v((x0 + x1) / 2.0, (RIM_LO + RIM_HI) / 2.0, (z0 + z1) / 2.0);
        if !c.cam.facing(pm, nrm) {
            continue;
        }
        let g = grain(k as f32 * 0.05, 1.3, f.figure);
        // Each strip reaches a little into its neighbours, so no hairline of what is
        // behind shows between two antialiased edges.
        let (ex, ez) = ((x1 - x0) / l * 0.35, (z1 - z0) / l * 0.35);
        let (x0e, z0e, x1e, z1e) = (x0 - ex, z0 - ez, x1 + ex, z1 + ez);
        let side = [
            v(x0e, RIM_LO, z0e),
            v(x1e, RIM_LO, z1e),
            v(x1e, RIM_HI - 1.4, z1e),
            v(x0e, RIM_HI - 1.4, z0e),
        ];
        let ends = (corner(k), corner((k + 1) % n));
        let exact = [
            v(x0, RIM_LO, z0),
            v(x1, RIM_LO, z1),
            v(x1, RIM_HI - 1.4, z1),
            v(x0, RIM_HI - 1.4, z0),
        ];
        facet_v_bent(c, &side, f, nrm, ends, true, g, (0.45, 0.95), 0.22, 0.6);
        bands.push((exact, ends));
        let bev = v(nrm.x, 0.9, nrm.z).norm();
        let top = [
            v(x0, RIM_HI - 1.4, z0),
            v(x1, RIM_HI - 1.4, z1),
            v(x1 - nrm.x * 1.2, RIM_HI, z1 - nrm.z * 1.2),
            v(x0 - nrm.x * 1.2, RIM_HI, z0 - nrm.z * 1.2),
        ];
        facet(c, &top, f, bev, g, None, 0.4, 1.0);
    }
    // The light's band over every strip, after them all: one long highlight.
    for (q, ends) in bands {
        lacquer_band(c, &q, f, ends);
    }
}

/// A grand's keyboard: the key slip, the fallboard, the keys, the cheeks.
fn keyboard(c: &mut Ctx, f: &Mat) {
    let cam = c.cam;
    c.p.at(cam.depth(v(0.0, 74.0, 8.0)) - 30.0, Part::Keys);
    // The key slip and the fallboard: lacquer shaded along their height. The slip faces
    // the camera square on: the strip lights' band there filled it whole with a silver
    // sheen (round 40 cp7), so it keeps its finish with one lit line along its top edge.
    let slip = [
        v(-74.0, RIM_LO, 15.0),
        v(74.0, RIM_LO, 15.0),
        v(74.0, 72.0, 15.0),
        v(-74.0, 72.0, 15.0),
    ];
    let zn = v(0.0, 0.0, 1.0);
    facet_v_bent(c, &slip, f, zn, (zn, zn), true, 1.0, (0.8, 0.95), 0.12, 0.5);
    let top_lit = c.shade(f, v(0.0, 1.0, 0.6).norm(), v(0.0, 72.0, 15.0));
    let (a, b) = (cam.p(slip[3]), cam.p(slip[2]));
    c.p.line(a, b, 0.8, Paint::Solid(top_lit), Cap::Butt, 0.8);
    facet_v(
        c,
        &[
            v(-62.0, 74.0, -1.5),
            v(62.0, 74.0, -1.5),
            v(62.0, 81.0, -3.5),
            v(-62.0, 81.0, -3.5),
        ],
        &super::light::scale_mat(f, 0.55),
        v(0.0, 0.4, 1.0).norm(),
        1.0,
        (0.7, 0.9),
        0.1,
        0.5,
    );
    let em = cam.p(v(0.0, 79.5, -2.8));
    c.p.ellipse(em, 5.0, 1.2, Paint::Solid(ink::EMBLEM), 0.85);
    let (a, b) = (cam.p(v(-62.0, 74.6, -0.6)), cam.p(v(62.0, 74.6, -0.6)));
    c.p.line(a, b, 1.2, Paint::Solid(ink::KEY_FELT), Cap::Round, 1.0);
    q(
        c,
        &[
            v(-62.0, 72.0, 15.0),
            v(62.0, 72.0, 15.0),
            v(62.0, 72.0, 0.0),
            v(-62.0, 72.0, 0.0),
        ],
        ink::KEYBED,
        0.5,
    );
    let w = 124.0 / 52.0;
    let up = v(0.0, 1.0, 0.0);
    let mut hp = Vec::new();
    // The white keys: one ivory run (each key shaded where it stands), the gaps between
    // them fine soft lines — the keybed showing through every gap hatched the keys grey
    // (round 40 cp7).
    for i in 0..52 {
        let x = -62.0 + i as f32 * w;
        let col = c.shade(&mat::IVORY, up, v(x, 74.0, 8.0));
        q(
            c,
            &[
                v(x, 74.0, 15.0),
                v(x + w, 74.0, 15.0),
                v(x + w, 74.0, 0.0),
                v(x, 74.0, 0.0),
            ],
            col,
            0.3,
        );
        q(
            c,
            &[
                v(x, 72.6, 15.05),
                v(x + w, 72.6, 15.05),
                v(x + w, 74.0, 15.05),
                v(x, 74.0, 15.05),
            ],
            scale(col, 0.75),
            0.3,
        );
        if i > 0 {
            let (ga, gb) = (cam.p(v(x, 74.02, 15.0)), cam.p(v(x, 74.02, 0.0)));
            c.p.line(
                ga,
                gb,
                0.35,
                Paint::Solid(scale(col, 0.55)),
                Cap::Butt,
                0.55,
            );
        }
    }
    let pat = [true, false, true, true, false, true, true];
    for i in 0..51 {
        if !pat[i % 7] {
            continue;
        }
        let x = -62.0 + (i + 1) as f32 * w - w * 0.3;
        // A black key two thirds the white's length (6 cm read as stubs), its front
        // edge catching the light.
        let bc = c.shade(&mat::EBONY, up, v(x, 75.0, 4.0));
        q(
            c,
            &[
                v(x, 75.3, 9.5),
                v(x + w * 0.58, 75.3, 9.5),
                v(x + w * 0.58, 75.3, 0.0),
                v(x, 75.3, 0.0),
            ],
            bc,
            0.0,
        );
        q(
            c,
            &[
                v(x, 74.0, 9.5),
                v(x + w * 0.58, 74.0, 9.5),
                v(x + w * 0.58, 75.3, 9.5),
                v(x, 75.3, 9.5),
            ],
            ink::KEY_SIDE,
            0.0,
        );
        let (ea, eb) = (cam.p(v(x, 75.32, 9.4)), cam.p(v(x + w * 0.58, 75.32, 9.4)));
        c.p.line(ea, eb, 0.4, Paint::Solid(ink::PIN), Cap::Butt, 0.45);
    }
    for cx in [-68.5, 68.5] {
        boxy(
            c,
            v(cx - 5.5, 70.0, -2.0),
            v(11.0, 0.0, 0.0),
            v(0.0, 9.0, 0.0),
            v(0.0, 0.0, 17.0),
            f,
            None,
        );
    }
    for p in [
        v(-62.0, 72.6, 15.0),
        v(62.0, 72.6, 15.0),
        v(62.0, 75.3, 0.0),
        v(-62.0, 75.3, 0.0),
    ] {
        hp.push(cam.p(p));
    }
    c.p.hit(Part::Keys, hull(&hp), cam.depth(v(0.0, 74.0, 8.0)) - 30.0);
}

/// The bench in front of the keys.
fn bench(c: &mut Ctx, f: &Mat, dz: f32) {
    let cam = c.cam;
    let by = 50.0;
    let dep = cam.depth(v(0.0, by, 54.0 + dz));
    c.p.at(dep, Part::Bench);
    boxy(
        c,
        v(-40.0, by - 6.0, 38.0 + dz),
        v(80.0, 0.0, 0.0),
        v(0.0, 6.0, 0.0),
        v(0.0, 0.0, 32.0),
        f,
        None,
    );
    c.piece = Part::Bench;
    for (bx, bz) in [
        (-36.0, 66.0 + dz),
        (36.0, 66.0 + dz),
        (-36.0, 42.0 + dz),
        (36.0, 42.0 + dz),
    ] {
        tube(c, v(bx, 0.0, bz), v(bx, by - 6.0, bz), 1.8, f, OWN_BUTT);
        c.contacts.push((v(bx, 0.0, bz), 3.0));
    }
    c.piece = Part::Piano;
    c.shadow_polys.push((
        vec![
            v(-40.0, by, 70.0 + dz),
            v(40.0, by, 70.0 + dz),
            v(40.0, by, 38.0 + dz),
            v(-40.0, by, 38.0 + dz),
        ],
        0.6,
    ));
    let mut hp = Vec::new();
    for x in [-40.0, 40.0] {
        for y in [0.0, by] {
            for z in [38.0 + dz, 70.0 + dz] {
                hp.push(cam.p(v(x, y, z)));
            }
        }
    }
    c.p.hit(Part::Bench, hull(&hp), dep);
}

/// A face of `f` drawn when it faces the eye, shaded along its height (`q`: bottom edge
/// q0–q1, top edge q2–q3).
fn face_v(c: &mut Ctx, q: [V3; 4], f: &Mat, n: V3) {
    if c.cam.facing(super::geo::centre(&q), n) {
        facet_v(c, &q, f, n, 1.0, (0.86, 0.95), 0.06, 0.5);
    }
}

/// A flat face of `f` drawn when it faces the eye.
fn face(c: &mut Ctx, pts: &[V3], f: &Mat, n: V3) {
    if c.cam.facing(super::geo::centre(pts), n) {
        facet(c, pts, f, n, 1.0, None, 0.5, 1.0);
    }
}

/// A profile in (z, y) extruded along x from `xa` to `xb`: its side faces that face the
/// eye; its end faces are the caller's (they are large and want a gradient).
fn prism_sides(c: &mut Ctx, prof: &[(f32, f32)], xa: f32, xb: f32, f: &Mat) {
    let area: f32 = (0..prof.len())
        .map(|i| {
            let (a, b) = (prof[i], prof[(i + 1) % prof.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    let s = if area > 0.0 { 1.0 } else { -1.0 };
    for i in 0..prof.len() {
        let (a, b) = (prof[i], prof[(i + 1) % prof.len()]);
        let n = v(0.0, -(b.0 - a.0) * s, (b.1 - a.1) * s).norm();
        face(
            c,
            &[
                v(xa, a.1, a.0),
                v(xb, a.1, a.0),
                v(xb, b.1, b.0),
                v(xa, b.1, b.0),
            ],
            f,
            n,
        );
    }
}

/// A veneer's figure over a wooden face `q` (bottom edge q0–q1, top edge q2–q3): faint
/// wavering lines along it, the way the drums' shells show theirs (a flat brown board
/// otherwise).
fn veneer(c: &mut Ctx, q: [V3; 4], f: &Mat, n: V3, seed: f32, lines: usize) {
    if !f.wood || !c.cam.facing(super::geo::centre(&q), n) {
        return;
    }
    let col = scale(f.dark, 0.6);
    for i in 0..lines {
        let t = (i as f32 * 0.618_034 + seed).fract();
        let pts: Vec<Pos2> = (0..9)
            .map(|j| {
                let s = j as f32 / 8.0;
                let w = 0.012 * (s * 6.0 + i as f32 * 1.7 + seed).sin();
                let tt = (t + w).clamp(0.0, 1.0);
                c.sp(q[0].lerp(q[1], tt).lerp(q[3].lerp(q[2], tt), s) + n * 0.05)
            })
            .collect();
        let w = 0.45 + 0.4 * ((i * 7) % 3) as f32 / 2.0;
        let op = 0.10 + 0.12 * ((i * 5) % 3) as f32 / 2.0;
        c.p.polyline(pts, w, Paint::Solid(col), op, false);
    }
}

/// A profile's end face at `x` facing `nx` (±1), shaded along its height in one gradient
/// (a little toward the front: the room's fill reaches a side the key does not).
fn side_face(c: &mut Ctx, prof: &[(f32, f32)], x: f32, nx: f32, f: &Mat) {
    let n = v(nx, 0.0, 0.45).norm();
    let pts: Vec<V3> = prof.iter().map(|&(z, y)| v(x, y, z)).collect();
    let ctr = super::geo::centre(&pts);
    if !c.cam.facing(ctr, n) {
        return;
    }
    c.keep_facet(&pts, v(nx, 0.0, 0.0), f, 1.0);
    let (y0, y1) = prof
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let (bot, top) = (v(x, y0, ctr.z), v(x, y1, ctr.z));
    let mut stops = [(0.0, [0.0; 3]); 4];
    for (st, (t, k)) in stops
        .iter_mut()
        .zip([(0.0, 0.7), (0.15, 0.9), (0.7, 1.0), (1.0, 0.92)])
    {
        let p = bot.lerp(top, t);
        let col = c.shade_g(f, n, p, grain(t * 2.0 + x * 0.01, 3.0, f.figure));
        *st = (t, scale(col, k));
    }
    let (a, b) = (c.sp(bot), c.sp(top));
    let sp = c.sps(&pts);
    c.p.poly(sp, lin(a, b, &stops), 1.0, 0.5);
}

/// A raised field on a front at `z`: its flat middle and four bevels, each its own light.
#[allow(clippy::too_many_arguments)]
fn raised(c: &mut Ctx, x0: f32, x1: f32, y0: f32, y1: f32, z: f32, bev: f32, rise: f32, f: &Mat) {
    let (ix0, ix1, iy0, iy1) = (x0 + bev, x1 - bev, y0 + bev, y1 - bev);
    let o = |x: f32, y: f32| v(x, y, z);
    let i = |x: f32, y: f32| v(x, y, z + rise);
    let k = rise / bev;
    for (q, n) in [
        (
            [o(x0, y0), o(x1, y0), i(ix1, iy0), i(ix0, iy0)],
            v(0.0, -k, 1.0),
        ),
        (
            [o(x1, y0), o(x1, y1), i(ix1, iy1), i(ix1, iy0)],
            v(k, 0.0, 1.0),
        ),
        (
            [i(ix0, iy1), i(ix1, iy1), o(x1, y1), o(x0, y1)],
            v(0.0, k, 1.0),
        ),
        (
            [o(x0, y0), i(ix0, iy0), i(ix0, iy1), o(x0, y1)],
            v(-k, 0.0, 1.0),
        ),
    ] {
        face(c, &q, f, n.norm());
    }
    face_v(
        c,
        [i(ix0, iy0), i(ix1, iy0), i(ix1, iy1), i(ix0, iy1)],
        f,
        v(0.0, 0.0, 1.0),
    );
}

/// A sheet of music from `o` along `across` and `up`: two pages, the fold, five systems.
fn music(c: &mut Ctx, o: V3, across: V3, up: V3, n: V3) {
    let pc = c.shade(&mat::PAPER, n, o + across * 0.5 + up * 0.5);
    q(c, &[o, o + across, o + across + up, o + up], pc, 0.5);
    let at = |a: f32, t: f32| c.cam.p(o + across * a + up * t + n * 0.1);
    let (f0, f1) = (at(0.5, 0.02), at(0.5, 0.98));
    c.p.line(f0, f1, 0.6, Paint::Solid(scale(pc, 0.8)), Cap::Butt, 0.8);
    for (a0, a1) in [(0.05, 0.45), (0.55, 0.95)] {
        for s in 0..5 {
            let top = 0.9 - s as f32 * 0.17;
            for l in 0..5 {
                let t = top - l as f32 * 0.022;
                let (pa, pb) = (at(a0, t), at(a1, t));
                c.p.line(pa, pb, 0.35, Paint::Solid(mat::STAFF), Cap::Butt, 0.45);
            }
        }
    }
}

/// A modern studio upright (`piano_geo`'s): side panels stepped as a real one's, the key
/// blocks, the key slip and keys, the fallboard open over the key backs, the music shelf
/// with a sheet on it against the upper panel's raised field, the bottom board's field and
/// the pedals, the plinth; the top in two leaves — the front one folds back over the rest,
/// showing the frame's top bar, its pins and the strings' tops in the well.
fn upright(c: &mut Ctx, f: &Mat, lid: f32) {
    let cam = c.cam;
    c.piece = Part::Piano;
    let (x0, x1, zb) = (-UP_W / 2.0, UP_W / 2.0, -UP_D);
    let (ix0, ix1) = (x0 + 3.0, x1 - 3.0);
    let well = UP_H - 22.0;
    let up = v(0.0, 1.0, 0.0);
    let ez = v(0.0, 0.0, 1.0);
    let dark = scale_mat(f, 0.35);
    let body_dep = cam.depth(v(0.0, 60.0, -30.0)) + 20.0;
    c.p.at(body_dep, Part::Piano);
    // Far first: the bass panel's inner face (seen in the knee space), its front edges.
    side_face(c, &UP_SIDE, ix0, 1.0, f);
    prism_sides(c, &UP_SIDE, x0, ix0, f);
    // The well (seen when the front leaf is up): its floor, the frame's top bar, the pins,
    // the strings.
    let open = lid > 0.2;
    if open {
        let wf = c.shade(&dark, up, v(0.0, well, -12.0));
        q(
            c,
            &[
                v(ix0, well, UP_LEAF_HINGE),
                v(ix1, well, UP_LEAF_HINGE),
                v(ix1, well, -2.0),
                v(ix0, well, -2.0),
            ],
            scale(wf, 0.5),
            0.5,
        );
        let zw = UP_LEAF_HINGE + 2.5;
        let bar = [
            v(ix0 + 5.0, UP_H - 13.0, zw),
            v(ix1 - 5.0, UP_H - 13.0, zw),
            v(ix1 - 5.0, UP_H - 1.0, zw),
            v(ix0 + 5.0, UP_H - 1.0, zw),
        ];
        face_v(c, bar, &mat::GOLD, ez);
        for i in 0..64 {
            let x = ix0 + 9.0 + i as f32 * ((ix1 - ix0 - 18.0) / 63.0);
            let col = if i < 16 {
                ink::BASS_STRING
            } else {
                ink::STEEL_STRING
            };
            let (a, b) = (
                cam.p(v(x, UP_H - 9.5, zw + 0.5)),
                cam.p(v(x, well, zw + 0.9)),
            );
            c.p.line(
                a,
                b,
                if i < 16 { 0.7 } else { 0.4 },
                Paint::Solid(col),
                Cap::Butt,
                0.8,
            );
            for (row, y) in [(0, UP_H - 4.0), (1, UP_H - 7.5)] {
                let dx = if row == 1 { 0.9 } else { 0.0 };
                let p = cam.p(v(x + dx, y, zw + 2.0));
                c.p.ellipse(p, 0.7, 0.55, Paint::Solid(ink::PIN_UPRIGHT), 1.0);
            }
        }
    }
    // The bottom board and its field, the plinth, the pedals.
    face_v(
        c,
        [
            v(ix0, 7.0, 3.0),
            v(ix1, 7.0, 3.0),
            v(ix1, 64.0, 3.0),
            v(ix0, 64.0, 3.0),
        ],
        f,
        ez,
    );
    raised(c, -54.0, 54.0, 14.0, 58.0, 3.0, 2.6, 0.6, f);
    veneer(
        c,
        [
            v(ix0, 7.0, 3.7),
            v(ix1, 7.0, 3.7),
            v(ix1, 64.0, 3.7),
            v(ix0, 64.0, 3.7),
        ],
        f,
        ez,
        0.5,
        30,
    );
    face_v(
        c,
        [
            v(ix0, 0.0, 5.0),
            v(ix1, 0.0, 5.0),
            v(ix1, 7.0, 5.0),
            v(ix0, 7.0, 5.0),
        ],
        &scale_mat(f, 0.8),
        ez,
    );
    face(
        c,
        &[
            v(ix0, 7.0, 5.0),
            v(ix1, 7.0, 5.0),
            v(ix1, 7.0, 3.0),
            v(ix0, 7.0, 3.0),
        ],
        f,
        up,
    );
    q(
        c,
        &[
            v(-13.0, 2.6, 3.6),
            v(13.0, 2.6, 3.6),
            v(13.0, 7.4, 3.6),
            v(-13.0, 7.4, 3.6),
        ],
        scale(c.shade(&dark, ez, v(0.0, 5.0, 3.6)), 0.6),
        0.5,
    );
    for px in [-8.0, 0.0, 8.0] {
        tube(c, v(px, 5.0, 3.4), v(px, 4.2, 12.5), 1.1, &mat::BRASS, OWN);
    }
    // The upper panel and its raised field; its top edge where the leaf is up.
    face_v(
        c,
        [
            v(ix0, 84.0, 0.0),
            v(ix1, 84.0, 0.0),
            v(ix1, UP_H, 0.0),
            v(ix0, UP_H, 0.0),
        ],
        f,
        ez,
    );
    raised(c, -58.0, 58.0, 91.0, UP_H - 7.0, 0.0, 3.0, 0.7, f);
    veneer(
        c,
        [
            v(ix0, 84.0, 0.8),
            v(ix1, 84.0, 0.8),
            v(ix1, UP_H, 0.8),
            v(ix0, UP_H, 0.8),
        ],
        f,
        ez,
        0.2,
        34,
    );
    if open {
        face(
            c,
            &[
                v(ix0, UP_H, 0.0),
                v(ix1, UP_H, 0.0),
                v(ix1, UP_H, -2.0),
                v(ix0, UP_H, -2.0),
            ],
            f,
            up,
        );
    }
    // The keybed's front under the slip, the key slip.
    face_v(
        c,
        [
            v(ix0, 64.0, 23.4),
            v(ix1, 64.0, 23.4),
            v(ix1, 70.6, 23.4),
            v(ix0, 70.6, 23.4),
        ],
        &scale_mat(f, 0.8),
        ez,
    );
    boxy(
        c,
        v(x0 + 15.0, 65.6, 22.6),
        v(x1 - x0 - 30.0, 0.0, 0.0),
        v(0.0, 6.8, 0.0),
        v(0.0, 0.0, 1.6),
        f,
        Some(&[1, 3]),
    );
    // The music shelf, its lip, the sheet leaning on the panel.
    boxy(
        c,
        v(-50.0, 84.0, 0.0),
        v(100.0, 0.0, 0.0),
        v(0.0, UP_SHELF_Y - 84.0, 0.0),
        v(0.0, 0.0, 5.5),
        f,
        Some(&[1, 3, 5]),
    );
    let (sb, st) = (v(-22.0, UP_SHELF_Y + 0.1, 4.3), v(-22.0, 113.0, 1.0));
    let sn = v(0.0, 3.3, 26.9).norm();
    music(c, sb, v(44.0, 0.0, 0.0), st - sb, sn);
    boxy(
        c,
        v(-50.0, UP_SHELF_Y, 4.6),
        v(100.0, 0.0, 0.0),
        v(0.0, 1.5, 0.0),
        v(0.0, 0.0, 0.9),
        f,
        Some(&[1, 3, 5]),
    );
    // The fallboard open over the key backs: its face, its rounded lip, the nameplate.
    let (fa, fb) = (x0 + 15.0, x1 - 15.0);
    let fbn = v(0.0, 3.8, 10.6).norm();
    // In a gentle curve: steeper at its lip, leaning back toward the shelf.
    face_v(
        c,
        [
            v(fa, 73.4, 6.0),
            v(fb, 73.4, 6.0),
            v(fb, 78.8, 5.1),
            v(fa, 78.8, 5.1),
        ],
        f,
        v(0.0, 0.9, 5.4).norm(),
    );
    face_v(
        c,
        [
            v(fa, 78.8, 5.1),
            v(fb, 78.8, 5.1),
            v(fb, 84.0, 2.2),
            v(fa, 84.0, 2.2),
        ],
        f,
        v(0.0, 2.9, 5.2).norm(),
    );
    tube(c, v(fa, 73.6, 5.6), v(fb, 73.6, 5.6), 0.95, f, OWN_BUTT);
    let plate = [
        v(-5.0, 77.6, 5.32),
        v(5.0, 77.6, 5.32),
        v(5.0, 78.5, 5.17),
        v(-5.0, 78.5, 5.17),
    ];
    let pcol = c.shade(&mat::BRASS, fbn, v(0.0, 78.7, 4.3));
    q(c, &plate, pcol, 0.0);
    // The front legs, tapering to their toe blocks on casters.
    for (la, lb) in UP_LEG_X {
        side_face(c, &UP_TOE, lb + 1.0, 1.0, f);
        prism_sides(c, &UP_TOE, la - 1.0, lb + 1.0, f);
        tube(
            c,
            v(la + 0.4, 1.2, 20.0),
            v(lb - 0.4, 1.2, 20.0),
            1.2,
            &mat::BLACK,
            OWN,
        );
        side_face(c, &UP_LEG, lb, 1.0, f);
        prism_sides(c, &UP_LEG, la, lb, f);
        c.contacts.push((v((la + lb) * 0.5, 0.0, 14.0), 6.0));
    }
    // The keys.
    c.p.at(body_dep, Part::Keys);
    let wk = (fb - fa) / 52.0;
    for i in 0..52 {
        let x = fa + i as f32 * wk;
        let col = c.shade(&mat::IVORY, up, v(x, 72.6, 14.0));
        q(
            c,
            &[
                v(x + 0.12, 72.6, 22.2),
                v(x + wk - 0.12, 72.6, 22.2),
                v(x + wk - 0.12, 72.6, 6.0),
                v(x + 0.12, 72.6, 6.0),
            ],
            col,
            0.0,
        );
        q(
            c,
            &[
                v(x + 0.12, 70.8, 22.25),
                v(x + wk - 0.12, 70.8, 22.25),
                v(x + wk - 0.12, 72.6, 22.25),
                v(x + 0.12, 72.6, 22.25),
            ],
            scale(col, 0.75),
            0.0,
        );
    }
    let pat = [true, false, true, true, false, true, true];
    for i in 0..51 {
        if !pat[i % 7] {
            continue;
        }
        let x = fa + (i + 1) as f32 * wk - wk * 0.3;
        let bc = c.shade(&mat::EBONY, up, v(x, 73.9, 10.0));
        q(
            c,
            &[
                v(x, 73.9, 14.6),
                v(x + wk * 0.6, 73.9, 14.6),
                v(x + wk * 0.6, 73.9, 6.0),
                v(x, 73.9, 6.0),
            ],
            bc,
            0.0,
        );
        q(
            c,
            &[
                v(x, 72.6, 15.0),
                v(x + wk * 0.6, 72.6, 15.0),
                v(x + wk * 0.6, 73.9, 14.6),
                v(x, 73.9, 14.6),
            ],
            ink::KEY_SIDE,
            0.0,
        );
    }
    let (a, b) = (cam.p(v(fa, 72.9, 5.8)), cam.p(v(fb, 72.9, 5.8)));
    c.p.line(a, b, 1.0, Paint::Solid(ink::KEY_FELT), Cap::Round, 1.0);
    let khp: Vec<Pos2> = c.sps(&[
        v(fa, 70.8, 22.25),
        v(fb, 70.8, 22.25),
        v(fb, 73.9, 6.0),
        v(fa, 73.9, 6.0),
    ]);
    c.p.hit(Part::Keys, hull(&khp), body_dep - 1.0);
    // The key blocks either side of the keys.
    c.p.at(body_dep, Part::Piano);
    for (xa, xb) in [(ix0, fa), (fb, ix1)] {
        side_block(c, xa, xb, f);
    }
    // The top: the back leaf, the piano hinge, the front leaf at its fold.
    boxy(
        c,
        v(x0 - 1.0, UP_H, zb - 0.5),
        v(UP_W + 2.0, 0.0, 0.0),
        v(0.0, UP_TOP_TH, 0.0),
        v(0.0, 0.0, UP_LEAF_HINGE - zb + 0.5),
        f,
        None,
    );
    let hc = c.shade(&mat::BRASS, up, v(0.0, UP_H + UP_TOP_TH, UP_LEAF_HINGE));
    let (ha, hb) = (
        cam.p(v(x0 + 2.0, UP_H + UP_TOP_TH + 0.1, UP_LEAF_HINGE)),
        cam.p(v(x1 - 2.0, UP_H + UP_TOP_TH + 0.1, UP_LEAF_HINGE)),
    );
    c.p.line(ha, hb, 0.7, Paint::Solid(hc), Cap::Butt, 0.9);
    c.p.at(body_dep, Part::Lid);
    c.piece = Part::Lid;
    let lc = upright_leaf(lid);
    boxy(
        c,
        lc[0],
        lc[1] - lc[0],
        lc[3] - lc[0],
        lc[4] - lc[0],
        f,
        None,
    );
    tube(
        c,
        (lc[3] + lc[7]) * 0.5,
        (lc[2] + lc[6]) * 0.5,
        UP_TOP_TH * 0.5,
        f,
        OWN_BUTT,
    );
    let lid_hp: Vec<Pos2> = lc.iter().map(|&p| cam.p(p)).collect();
    c.p.hit(
        Part::Lid,
        hull(&lid_hp),
        cam.depth(v(0.0, 60.0, -30.0)) - 1.0,
    );
    // The bench in the piano's finish.
    bench(c, f, UP_BENCH_DZ);
    c.piece = Part::Piano;
    c.p.at(body_dep, Part::Piano);
    // Near last: the treble panel's outer face and its front edges.
    c.p.at(body_dep, Part::Piano);
    c.piece = Part::Piano;
    prism_sides(c, &UP_SIDE, ix1, x1, f);
    side_face(c, &UP_SIDE, x1, 1.0, f);
    veneer(
        c,
        [
            v(x1, 0.0, zb),
            v(x1, 0.0, 1.5),
            v(x1, UP_H, 1.5),
            v(x1, UP_H, zb),
        ],
        f,
        v(1.0, 0.0, 0.0),
        0.8,
        24,
    );
    let mut sh = Vec::new();
    let mut hp = Vec::new();
    for x in [x0, x1] {
        for y in [0.0, UP_H] {
            for z in [zb, 24.0] {
                sh.push(v(x, y, z));
            }
        }
        for y in [0.0, UP_H + 4.0] {
            for z in [zb, 24.0] {
                hp.push(cam.p(v(x, y, z)));
            }
        }
    }
    for x in [x0 + 2.0, x1 - 2.0] {
        for z in [zb + 3.0, 22.0] {
            c.contacts.push((v(x, 0.0, z), 5.0));
        }
    }
    c.shadow_polys.push((sh, 0.9));
    c.p.hit(Part::Piano, hull(&hp), cam.depth(v(0.0, 60.0, -30.0)));
}

/// A key block between `xa` and `xb`: its end face toward the keys (the bass one's), its
/// top and front round its rounded corner.
fn side_block(c: &mut Ctx, xa: f32, xb: f32, f: &Mat) {
    let n = v(1.0, 0.0, 0.0);
    let cap: Vec<V3> = UP_BLOCK.iter().map(|&(z, y)| v(xb, y, z)).collect();
    face(c, &cap, f, n);
    prism_sides(c, &UP_BLOCK, xa, xb, f);
}

/// The part of a plan outline between depths `z0` and `z1` (Sutherland–Hodgman against the
/// two half-planes): a band of the lid.
pub fn slab(o: &[(f32, f32)], z0: f32, z1: f32) -> Vec<(f32, f32)> {
    let clip = |pts: Vec<(f32, f32)>, keep: &dyn Fn(f32) -> bool, at: f32| {
        let mut out = Vec::with_capacity(pts.len() + 4);
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            let (ia, ib) = (keep(a.1), keep(b.1));
            if ia {
                out.push(a);
            }
            if ia != ib {
                let t = (at - a.1) / (b.1 - a.1);
                out.push((a.0 + (b.0 - a.0) * t, at));
            }
        }
        out
    };
    let a = clip(o.to_vec(), &|z| z >= z0, z0);
    if a.len() < 3 {
        return a;
    }
    clip(a, &|z| z <= z1, z1)
}
