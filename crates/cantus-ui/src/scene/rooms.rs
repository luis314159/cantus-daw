//! The rooms (port of `scene/rooms.py`): a booth, a studio's live room with the control
//! room's window, a hall with wooden fins, uplights and a polished stage, a church of
//! limestone with lancet windows — each with a choice of floor, walls and rug. Two walls
//! meet behind the instrument (on `Far`, drawn a touch soft), the floor runs toward us on
//! `Back`, the air's haze between on `Haze`. Every surface takes the key's spot; the pool
//! bounces warm onto the foot of the walls.

use super::display::{lin, lin_a, rad, Cap, Layer, Paint};
use super::geo::{v, V3};
use super::light::{add, lerp, scale};
use super::prims::Ctx;
use super::Room;
use crate::kit::skin::scene::{self as pal, ink, Laid, Mat, MatKind, Motif, Pattern, C3};
use cantus_core::inst::room_look::{Floor, RoomLook, Rug, Walls};
use egui::Pos2;

/// A room's size (cm) and its own look.
pub struct RoomDef {
    pub back: f32,
    pub side: f32,
    pub height: f32,
    pub floor: Floor,
    pub walls: Walls,
    pub rug: Rug,
}

pub fn def(kind: Room) -> RoomDef {
    let (back, side, height, floor, walls, rug) = match kind {
        Room::Stage => (
            480.0,
            760.0,
            720.0,
            Floor::StageWalnut,
            Walls::StageDark,
            Rug::Bare,
        ),
        Room::Booth => (
            170.0,
            230.0,
            250.0,
            Floor::HoneyOak,
            Walls::WarmPlaster,
            Rug::Red,
        ),
        Room::Studio => (
            270.0,
            380.0,
            340.0,
            Floor::HoneyOak,
            Walls::WarmWood,
            Rug::Red,
        ),
        Room::Hall => (
            560.0,
            720.0,
            820.0,
            Floor::StageWalnut,
            Walls::HallWood,
            Rug::Bare,
        ),
        Room::Church => (
            720.0,
            820.0,
            1100.0,
            Floor::Flagstone,
            Walls::Limestone,
            Rug::Bare,
        ),
    };
    RoomDef {
        back,
        side,
        height,
        floor,
        walls,
        rug,
    }
}

pub fn floor_paint(f: Floor) -> pal::FloorPaint {
    match f {
        Floor::LightOak => pal::FLOOR_LIGHT_OAK,
        Floor::HoneyOak => pal::FLOOR_HONEY_OAK,
        Floor::DarkWalnut => pal::FLOOR_DARK_WALNUT,
        Floor::StageWalnut => pal::FLOOR_STAGE_WALNUT,
        Floor::EbonyStage => pal::FLOOR_EBONY_STAGE,
        Floor::Concrete => pal::FLOOR_CONCRETE,
        Floor::Terrazzo => pal::FLOOR_TERRAZZO,
        Floor::Flagstone => pal::FLOOR_FLAGSTONE,
    }
}

pub fn walls_paint(w: Walls) -> pal::WallPaint {
    match w {
        Walls::WarmPlaster => pal::WALLS_WARM_PLASTER,
        Walls::WarmWood => pal::WALLS_WARM_WOOD,
        Walls::WalnutPanels => pal::WALLS_WALNUT_PANELS,
        Walls::Plaster => pal::WALLS_PLASTER,
        Walls::Charcoal => pal::WALLS_CHARCOAL,
        Walls::BlueFabric => pal::WALLS_BLUE_FABRIC,
        Walls::GreenFabric => pal::WALLS_GREEN_FABRIC,
        Walls::Brick => pal::WALLS_BRICK,
        Walls::HallWood => pal::WALLS_HALL_WOOD,
        Walls::Limestone => pal::WALLS_LIMESTONE,
        Walls::StageDark => pal::WALLS_STAGE_DARK,
    }
}

pub fn rug_paint(r: Rug) -> Option<pal::RugPaint> {
    match r {
        Rug::Red => Some(pal::RUG_RED),
        Rug::Blue => Some(pal::RUG_BLUE),
        Rug::Grey => Some(pal::RUG_GREY),
        Rug::Bare => None,
    }
}

/// What the room's build tells the scene: the back wall's z, the floor's gloss.
pub struct Info {
    pub zb: f32,
    pub gloss: f32,
}

type F = pal::FloorPaint;
type W = pal::WallPaint;

/// The warm light the floor's pool throws back onto the foot of a wall.
fn bounce(c: &Ctx, f: &F, x: f32, z: f32) -> C3 {
    scale(
        f.a,
        c.light.spot_r(v(x, 0.0, z), 340.0) * c.light.m.key * 0.42,
    )
}

fn up() -> V3 {
    v(0.0, 1.0, 0.0)
}

fn wall_strip(c: &mut Ctx, a: V3, b: V3, top: f32, n: V3, albedo: C3, f: &F) {
    let q = [
        v(a.x, 0.0, a.z),
        v(b.x, 0.0, b.z),
        v(b.x, top, b.z),
        v(a.x, top, a.z),
    ];
    let (mx, mz) = ((a.x + b.x) / 2.0, (a.z + b.z) / 2.0);
    let lo = add(
        c.light.lit(albedo, n, v(mx, 20.0, mz)),
        c.light
            .lit_s(bounce(c, f, mx, mz), up(), v(mx, 0.0, mz), false),
    );
    let mid = c.light.lit(albedo, n, v(mx, top * 0.35, mz));
    let hi = c.light.lit(albedo, n, v(mx, top, mz));
    let (p0, p1) = (c.sp(q[0]), c.sp(q[3]));
    let sp = c.sps(&q);
    c.p.poly(
        sp,
        lin(p0, p1, &[(0.0, lo), (0.35, mid), (1.0, scale(hi, 0.8))]),
        1.0,
        0.6,
    );
}

/// Build the room `kind` round `centre` with its look; `evening` the mood.
pub fn build(c: &mut Ctx, kind: Room, centre: V3, look: RoomLook) -> Info {
    let r = def(kind);
    let f = floor_paint(look.floor.unwrap_or(r.floor));
    let w = walls_paint(look.walls.unwrap_or(r.walls));
    let w = if c.light.evening { w } else { pal::daylit(w) };
    let rug = rug_paint(look.rug.unwrap_or(r.rug));
    let (cx, cz) = (centre.x, centre.z);
    let zb = cz - r.back;
    let xs = cx - r.side;
    let xr = cx + r.side * 1.6;
    let zf = (cz + r.back * 2.4).min(c.cam.pos.z - 60.0);
    let h = r.height;
    c.p.on(Layer::Far);
    c.quad(
        &[
            v(xs, h, zb),
            v(xr, h, zb),
            v(xr, h * 3.0, zb),
            v(xs, h * 3.0, zb),
        ],
        scale(w.wall2, 0.25),
        1.0,
        0.0,
    );
    let step = if matches!(kind, Room::Booth | Room::Studio) {
        40.0
    } else {
        80.0
    };
    let mut x = xs;
    while x < xr {
        wall_strip(
            c,
            v(x, 0.0, zb),
            v(xr.min(x + step), 0.0, zb),
            h,
            v(0.0, 0.0, 1.0),
            w.wall,
            &f,
        );
        x += step;
    }
    let mut z = zb;
    while z < zf {
        wall_strip(
            c,
            v(xs, 0.0, z),
            v(xs, 0.0, zf.min(z + step)),
            h,
            v(1.0, 0.0, 0.0),
            w.wall2,
            &f,
        );
        z += step;
    }
    let (a, b) = (c.sp(v(xs, 0.0, zb)), c.sp(v(xs, h, zb)));
    c.p.line(a, b, 6.0, Paint::Solid(ink::SHADE), Cap::Round, 0.18);
    surface(c, &w, xs, xr, zb, zf, h);
    match kind {
        Room::Stage => stage(c, &w, cx, xs, xr, zb, h),
        Room::Booth => booth(c, &w, cx, cz, xs, zb),
        Room::Studio => studio(c, &w, &f, cx, xs, xr, zb, h),
        Room::Hall => hall(c, &w, xs, xr, zb, h),
        Room::Church => church(c, cx, zb),
    }
    let sk = scale(w.wood, 0.55);
    let col = c.light.lit(sk, v(0.0, 0.0, 1.0), v(cx, 5.0, zb));
    c.quad(
        &[
            v(xs, 0.0, zb),
            v(xr, 0.0, zb),
            v(xr, 9.0, zb + 0.5),
            v(xs, 9.0, zb + 0.5),
        ],
        col,
        1.0,
        0.5,
    );
    let col = c.light.lit(sk, v(1.0, 0.0, 0.0), v(xs, 5.0, cz));
    c.quad(
        &[
            v(xs, 0.0, zb),
            v(xs, 0.0, zf),
            v(xs + 0.5, 9.0, zf),
            v(xs + 0.5, 9.0, zb),
        ],
        col,
        1.0,
        0.5,
    );
    // The air between us and the far walls: a haze that thickens with the room.
    c.p.on(Layer::Haze);
    let (a, b) = (c.sp(v(cx, h, zb)), c.sp(v(cx, 0.0, zb)));
    let hz = match kind {
        Room::Stage => 0.20,
        Room::Booth => 0.10,
        Room::Studio => 0.16,
        Room::Hall => 0.30,
        Room::Church => 0.34,
    };
    let hc = c.light.m.haze;
    // Daylight's haze is pale: a thin veil, never a grey wash over the room (round 40).
    let hz = hz * if hc[0] > 0.5 { 0.4 } else { 1.0 };
    for q in [
        [v(xs, 0.0, zb), v(xr, 0.0, zb), v(xr, h, zb), v(xs, h, zb)],
        [v(xs, 0.0, zb), v(xs, 0.0, zf), v(xs, h, zf), v(xs, h, zb)],
    ] {
        let sp = c.sps_near(&q);
        c.p.poly(
            sp,
            lin_a(a, b, &[(0.0, hc, hz * 1.4), (1.0, hc, hz * 0.3)]),
            1.0,
            0.0,
        );
    }
    c.p.on(Layer::Back);
    floor(c, &f, xs, xr, zb, zf, cx, cz);
    if kind == Room::Hall {
        uplight_pools(c, xs, xr, zb);
    } else if kind == Room::Stage {
        let op = 0.30 * c.light.m.glow + 0.04;
        for ux in steps((cx - 470.0) as i32, xr as i32, 330) {
            let p = c.sp(v(ux as f32, 0.3, zb + 30.0));
            let g = rad(
                p,
                40.0,
                9.0,
                &[(0.0, ink::UPLIGHT, op), (1.0, ink::UPLIGHT, 0.0)],
            );
            c.p.ellipse(p, 40.0, 9.0, g, 1.0);
        }
    }
    if let Some(rg) = rug {
        c.p.on(Layer::Rug);
        rug_at(c, cx, cz, kind, &rg);
    }
    c.p.on(Layer::Objects);
    Info {
        zb,
        gloss: f.coat
            * match kind {
                Room::Hall => 0.85,
                Room::Stage => 0.95,
                _ => 0.75,
            },
    }
}

/// Integer steps from `a` up to (not including) `b` (Python's `range` over ints).
fn steps(a: i32, b: i32, s: i32) -> impl Iterator<Item = i32> {
    (a..b).step_by(s.max(1) as usize)
}

/// What the walls are made of: brick courses, walnut panels with their joints, a grid of
/// acoustic panels, or limestone courses.
fn surface(c: &mut Ctx, w: &W, xs: f32, xr: f32, zb: f32, zf: f32, h: f32) {
    let n = v(0.0, 0.0, 1.0);
    match w.pattern {
        Pattern::Brick => {
            for (row, y) in steps(0, h.min(420.0) as i32, 8).enumerate() {
                let off = (row % 2) as i32 * 12;
                for (i, x) in steps(xs as i32 - 12 + off, xr as i32, 24).enumerate() {
                    let (fi, fr, xf, yf) = (i as f32, row as f32, x as f32, y as f32);
                    let t = 0.5 + 0.5 * (fi * 2.9 + fr * 1.7).sin() * (fi * 0.7 + fr).cos();
                    let col = c
                        .light
                        .lit(lerp(w.wall, w.wall2, t), n, v(xf + 12.0, yf + 4.0, zb));
                    let z = zb + 0.2;
                    c.quad(
                        &[
                            v(xf + 0.6, yf + 0.6, z),
                            v(xf + 23.4, yf + 0.6, z),
                            v(xf + 23.4, yf + 7.4, z),
                            v(xf + 0.6, yf + 7.4, z),
                        ],
                        col,
                        1.0,
                        0.0,
                    );
                }
            }
        }
        Pattern::Panels => {
            for x in steps(xs as i32, xr as i32, 60) {
                let x = x as f32;
                let col = scale(c.light.lit(w.wall, n, v(x, 100.0, zb)), 0.5);
                let (a, b) = (c.sp(v(x, 0.0, zb + 0.3)), c.sp(v(x, h, zb + 0.3)));
                c.p.line(a, b, 1.2, Paint::Solid(col), Cap::Round, 1.0);
            }
            for z in steps(zb as i32, zf as i32, 60) {
                let z = z as f32;
                let col = scale(c.light.lit(w.wall2, v(1.0, 0.0, 0.0), v(xs, 100.0, z)), 0.5);
                let (a, b) = (c.sp(v(xs + 0.3, 0.0, z)), c.sp(v(xs + 0.3, h, z)));
                c.p.line(a, b, 1.2, Paint::Solid(col), Cap::Round, 1.0);
            }
        }
        Pattern::Acoustic => {
            for x in steps(xs as i32 + 10, xr as i32, 70) {
                for y in steps(20, h.min(300.0) as i32 - 60, 70) {
                    let alt = (x.div_euclid(70) + y.div_euclid(70)).rem_euclid(3) == 0;
                    let (x, y) = (x as f32, y as f32);
                    let alb = if alt { w.fabric2 } else { w.fabric };
                    let col = c.light.lit(alb, n, v(x + 30.0, y + 30.0, zb));
                    let z = zb + 3.0;
                    c.quad(
                        &[
                            v(x, y, z),
                            v(x + 62.0, y, z),
                            v(x + 62.0, y + 62.0, z),
                            v(x, y + 62.0, z),
                        ],
                        col,
                        1.0,
                        0.0,
                    );
                }
            }
        }
        Pattern::Stone => {
            for (row, y) in steps(0, (h * 0.9) as i32, 42).enumerate() {
                let off = (row % 2) as i32 * 40;
                for (i, x) in steps(xs as i32 - 40 + off, xr as i32, 80).enumerate() {
                    let (fi, fr, xf, yf) = (i as f32, row as f32, x as f32, y as f32);
                    let t = 0.5 + 0.5 * (fi * 1.9 + fr * 2.3).sin();
                    let col = c
                        .light
                        .lit(lerp(w.wall, w.wall2, t), n, v(xf + 40.0, yf + 21.0, zb));
                    let z = zb + 0.3;
                    c.quad(
                        &[
                            v(xf + 1.0, yf + 1.0, z),
                            v(xf + 79.0, yf + 1.0, z),
                            v(xf + 79.0, yf + 41.0, z),
                            v(xf + 1.0, yf + 41.0, z),
                        ],
                        col,
                        1.0,
                        0.0,
                    );
                }
            }
        }
        Pattern::Plain | Pattern::Curtain => {}
    }
}

/// The floor: its joints' ground, then each board (shaded as a finish at four points along
/// it: the soft box's long sheen, two fine grain lines near the eye) or each slab or stone.
#[allow(clippy::too_many_arguments)]
fn floor(c: &mut Ctx, f: &F, xs: f32, xr: f32, zb: f32, zf: f32, cx: f32, cz: f32) {
    // The floor takes the room's light, not the photographer's fill on the instrument
    // (round 40 cp5: a stage lit edge to edge; the photograph's is a pool).
    c.light.no_fill = true;
    floor_in(c, f, xs, xr, zb, zf, cx, cz);
    c.light.no_fill = false;
}

#[allow(clippy::too_many_arguments)]
fn floor_in(c: &mut Ctx, f: &F, xs: f32, xr: f32, zb: f32, zf: f32, cx: f32, cz: f32) {
    let fin = Mat {
        kind: MatKind::Lacquer,
        albedo: f.a,
        dark: f.a,
        light: f.a,
        coat: f.coat,
        spec: 0.5 + f.coat,
        shine: 40.0,
        wood: false,
        figure: 0.07,
        tint: [1.0; 3],
    };
    // By evening the floor is the key's pool, falling off toward the dark beyond it (the
    // photograph's stage): what the ambient lights far from the instrument held at a
    // fraction of it.
    let evening = c.light.evening;
    let pool = move |c: &Ctx, p: V3| {
        if evening {
            0.45 + 0.55 * c.light.spot_r(p, c.light.m.spot_r * 1.6)
        } else {
            1.0
        }
    };
    let col =
        |c: &Ctx, alb: C3, p: V3| scale(c.shade(&Mat { albedo: alb, ..fin }, up(), p), pool(c, p));
    let ground = scale(
        c.light.lit(f.joint, up(), v(cx, 0.0, cz)),
        if evening { 0.6 } else { 1.0 },
    );
    c.quad(
        &[
            v(xs, 0.0, zb),
            v(xr, 0.0, zb),
            v(xr, 0.0, zf),
            v(xs, 0.0, zf),
        ],
        ground,
        1.0,
        0.0,
    );
    if f.laid != Laid::Boards {
        let s = f.w as i32;
        let sf = f.w;
        for (i, x) in steps(xs as i32, xr as i32, s).enumerate() {
            for (j, z) in steps(zb as i32, zf as i32, s).enumerate() {
                let (fi, fj, x, z) = (i as f32, j as f32, x as f32, z as f32);
                let off = if f.laid == Laid::Stones {
                    (j % 2) as f32 * sf / 2.0
                } else {
                    0.0
                };
                let amp = if f.laid == Laid::Stones { 1.0 } else { 0.4 };
                let t = 0.5 + 0.5 * (fi * 2.7 + fj * 1.3).sin() * amp;
                let q = [
                    v(x + 0.8 + off, 0.0, z + 0.8),
                    v(x + sf - 0.8 + off, 0.0, z + 0.8),
                    v(x + sf - 0.8 + off, 0.0, z + sf - 0.8),
                    v(x + 0.8 + off, 0.0, z + sf - 0.8),
                ];
                let (a, b) = (v(x + off, 0.0, z), v(x + sf + off, 0.0, z + sf));
                let alb = lerp(f.a, f.b, t);
                let g = lin(
                    c.sp(a),
                    c.sp(b),
                    &[(0.0, col(c, alb, a)), (1.0, col(c, alb, b))],
                );
                let sp = c.sps_near(&q);
                if sp.len() >= 3 {
                    c.p.poly(sp, g, 1.0, 0.0);
                }
                if f.laid == Laid::Terrazzo {
                    for k in 0..5 {
                        let px = x + off + sf * (((i * 7 + j * 3 + k * 5) % 11) as f32 / 11.0);
                        let pz = z + sf * (((i * 5 + j * 11 + k * 3) % 13) as f32 / 13.0);
                        let pp = c.sp(v(px, 0.2, pz));
                        let ch = if k % 2 == 1 {
                            ink::CHIP_DARK
                        } else {
                            ink::CHIP_LIGHT
                        };
                        c.p.ellipse(pp, 1.0, 0.5, Paint::Solid(ch), 0.55);
                    }
                } else if f.laid == Laid::Slabs {
                    for k in 0..3 {
                        let px = x + sf * (0.2 + 0.3 * k as f32);
                        let pz = z + sf * (0.3 + 0.2 * ((i + k) % 3) as f32);
                        let pp = c.sp(v(px, 0.2, pz));
                        c.p.ellipse(pp, 14.0, 5.0, Paint::Solid(scale(alb, 0.94)), 0.12);
                    }
                }
            }
        }
        return;
    }
    let w = f.w;
    let mut k = 0usize;
    let mut x = xs;
    while x < xr {
        let mut z0 = zb;
        let mut j = 0usize;
        while z0 < zf {
            let mut ln = 150 + ((k * 53 + j * 29) % 110) as i32;
            if j == 0 {
                ln -= ((k * 37) % 120) as i32;
            }
            let z1 = zf.min(z0 + (ln.max(40)) as f32);
            let (fk, fj) = (k as f32, j as f32);
            let t = 0.5 + 0.5 * (fk * 2.1 + fj * 1.7).sin() * (fk * 0.7).cos();
            let alb = lerp(f.a, f.b, t);
            let stops: Vec<(f32, C3)> = [0.0, 0.33, 0.66, 1.0]
                .iter()
                .map(|&q| (q, col(c, alb, v(x + w / 2.0, 0.0, z0 + (z1 - z0) * q))))
                .collect();
            let (sa, sb) = (c.sp(v(x + w / 2.0, 0.0, z0)), c.sp(v(x + w / 2.0, 0.0, z1)));
            let q = [
                v(x + 0.35, 0.0, z0 + 0.35),
                v(x + w - 0.35, 0.0, z0 + 0.35),
                v(x + w - 0.35, 0.0, z1 - 0.35),
                v(x + 0.35, 0.0, z1 - 0.35),
            ];
            let sp = c.sps_near(&q);
            if sp.len() >= 3 {
                c.p.poly(sp, lin(sa, sb, &stops), 1.0, 0.0);
            }
            if c.cam.s / c.cam.depth(v(x, 0.0, z0)).max(1.0) > 0.35 {
                for g in [0.3f32, 0.7] {
                    let pts: Vec<Pos2> = (0..7)
                        .map(|i| {
                            let fi = i as f32;
                            c.sp(v(
                                x + w * g + 0.8 * (fi * 0.9 + fk + g * 5.0).sin(),
                                0.1,
                                z0 + (z1 - z0) * fi / 6.0,
                            ))
                        })
                        .collect();
                    c.p.polyline(pts, 0.5, Paint::Solid(scale(alb, 0.72)), 0.35, false);
                }
            }
            z0 = z1;
            j += 1;
        }
        x += w;
        k += 1;
    }
}

/// The hall's uplights: a warm pool on the stage at the foot of every other fin.
fn uplight_pools(c: &mut Ctx, xs: f32, xr: f32, zb: f32) {
    let glow = c.light.m.glow;
    for (k, x) in steps(xs as i32 + 60, xr as i32, 110).enumerate() {
        if k % 2 == 1 {
            continue;
        }
        let p = c.sp(v(x as f32 + 50.0, 0.3, zb + 40.0));
        let g = rad(
            p,
            46.0,
            12.0,
            &[
                (0.0, ink::UPLIGHT, 0.35 * glow + 0.05),
                (1.0, ink::UPLIGHT, 0.0),
            ],
        );
        c.p.ellipse(p, 46.0, 12.0, g, 1.0);
    }
}

/// A rug: its border, its field lit across by the spot, a quiet lattice, the inner line
/// and the border's motif.
/// A rug's pile: its top above the floor (cm).
const PILE: f32 = 1.2;

fn rug_at(c: &mut Ctx, cx: f32, cz: f32, kind: Room, rg: &pal::RugPaint) {
    let (w, d) = if kind == Room::Studio {
        (300.0, 230.0)
    } else {
        (250.0, 200.0)
    };
    let (x0, x1, z0, z1) = (cx - w / 2.0, cx + w / 2.0, cz - d / 2.0, cz + d / 2.0);
    let q = |a: f32, b: f32, cc: f32, dd: f32| {
        [
            v(a, PILE, cc),
            v(b, PILE, cc),
            v(b, PILE, dd),
            v(a, PILE, dd),
        ]
    };
    let n = up();
    let border = c.light.lit(rg.border, n, v(cx, 0.0, cz));
    // Its pile's edges where they face us (a rug has a thickness), the fringe at its ends.
    for (e0, e1, en) in [
        (v(x0, 0.0, z1), v(x1, 0.0, z1), v(0.0, 0.0, 1.0)),
        (v(x1, 0.0, z0), v(x1, 0.0, z1), v(1.0, 0.0, 0.0)),
        (v(x0, 0.0, z0), v(x0, 0.0, z1), v(-1.0, 0.0, 0.0)),
    ] {
        if c.cam.facing((e0 + e1) * 0.5, en) {
            let col = scale(c.light.lit(rg.border, en, (e0 + e1) * 0.5), 0.6);
            c.quad(&[e0, e1, e1 + n * PILE, e0 + n * PILE], col, 1.0, 0.5);
        }
    }
    let fr = scale(c.light.lit(rg.line, n, v(cx, 0.0, cz)), 0.85);
    for z in [z0, z1] {
        let out = if z == z1 { 6.0 } else { -6.0 };
        for k in 0..40 {
            let x = x0 + 4.0 + (x1 - x0 - 8.0) * k as f32 / 39.0;
            let (pa, pb) = (c.sp(v(x, 0.4, z)), c.sp(v(x + 0.6, 0.2, z + out)));
            c.p.line(pa, pb, 0.6, Paint::Solid(fr), Cap::Round, 0.75);
        }
    }
    c.quad(&q(x0, x1, z0, z1), border, 1.0, 0.5);
    let bw = 22.0;
    let pc = c.sp(v(cx, PILE, cz));
    let (a, b) = (c.sp(v(x0, 0.0, cz)), c.sp(v(x1, 0.0, cz)));
    let fa = if c.light.evening {
        rg.field
    } else {
        scale(rg.field, rg.day)
    };
    let f0 = c.light.lit(fa, n, v(cx, 0.0, cz));
    let f1 = c.light.lit(fa, n, v(x1, 0.0, z1));
    let span = (b.x - a.x).abs();
    let sp = c.sps(&q(x0 + bw, x1 - bw, z0 + bw, z1 - bw));
    c.p.poly(
        sp,
        rad(
            pc,
            span * 0.6,
            span * 0.32,
            &[(0.0, f0, 1.0), (1.0, f1, 1.0)],
        ),
        1.0,
        0.0,
    );
    for i in 1..12 {
        for sgn in [1.0f32, -1.0] {
            let xa = x0 + bw + (x1 - x0 - 2.0 * bw) * i as f32 / 12.0;
            let (pa, pb) = (
                c.sp(v(xa, PILE + 0.05, z0 + bw)),
                c.sp(v(
                    xa + sgn * (z1 - z0 - 2.0 * bw) * PILE + 0.05,
                    PILE + 0.05,
                    z1 - bw,
                )),
            );
            c.p.line(pa, pb, 0.6, Paint::Solid(scale(f0, 0.7)), Cap::Round, 0.35);
        }
    }
    // The pile's sheen falling off toward the border: the field reads as wool, not paint.
    let fsp = c.sps(&q(x0 + bw, x1 - bw, z0 + bw, z1 - bw));
    c.p.poly(
        fsp,
        rad(
            pc,
            span * 0.42,
            span * 0.22,
            &[
                (0.0, ink::SHADE, 0.0),
                (0.7, ink::SHADE, 0.08),
                (1.0, ink::SHADE, 0.28),
            ],
        ),
        1.0,
        0.0,
    );
    let md = c.light.lit(rg.chain, n, v(cx, 0.0, cz));
    for (rx, rz, w, op) in [
        (46.0, 34.0, 1.6, 0.7),
        (40.0, 29.0, 0.8, 0.5),
        (14.0, 10.0, 1.2, 0.7),
    ] {
        let pts: Vec<Pos2> = (0..48)
            .map(|k| {
                let t = std::f32::consts::TAU * k as f32 / 48.0;
                c.sp(v(cx + rx * t.cos(), PILE + 0.05, cz + rz * t.sin()))
            })
            .collect();
        c.p.polyline(pts, w, Paint::Solid(md), op, true);
    }
    let ln = c.light.lit(rg.line, n, v(cx, 0.0, cz));
    let sp = c.sps(&q(
        x0 + bw - 3.0,
        x1 - bw + 3.0,
        z0 + bw - 3.0,
        z1 - bw + 3.0,
    ));
    c.p.polyline(sp, 1.2, Paint::Solid(ln), 1.0, true);
    let ch = c.light.lit(rg.chain, n, v(cx, 0.0, cz));
    if rg.motif == Motif::Stripes {
        for k in [5.0, 9.0, 13.0, 17.0] {
            let sp = c.sps(&q(x0 + k, x1 - k, z0 + k, z1 - k));
            c.p.polyline(sp, 0.7, Paint::Solid(ch), 0.6, true);
        }
        return;
    }
    let mut per = Vec::with_capacity(140);
    for t in 0..40 {
        let f = t as f32 / 40.0;
        per.push((x0 + 11.0 + (x1 - x0 - 22.0) * f, z0 + 11.0));
        per.push((x0 + 11.0 + (x1 - x0 - 22.0) * f, z1 - 11.0));
    }
    for t in 0..30 {
        let f = t as f32 / 30.0;
        per.push((x0 + 11.0, z0 + 11.0 + (z1 - z0 - 22.0) * f));
        per.push((x1 - 11.0, z0 + 11.0 + (z1 - z0 - 22.0) * f));
    }
    for (px, pz) in per {
        if rg.motif == Motif::Diamonds {
            let pts: Vec<Pos2> = [(-2.6, 0.0), (0.0, -2.6), (2.6, 0.0), (0.0, 2.6)]
                .iter()
                .map(|&(dx, dz)| c.sp(v(px + dx, PILE + 0.05, pz + dz)))
                .collect();
            c.p.poly(pts, Paint::Solid(ch), 0.85, 0.0);
        } else {
            let p = c.sp(v(px, PILE + 0.05, pz));
            c.p.ellipse(p, 1.6, 0.8, Paint::Solid(ch), 0.85);
        }
    }
}

/// A flat panel on the back wall (or the side wall, `side`), `off` out from it.
#[allow(clippy::too_many_arguments)]
fn panel(
    c: &mut Ctx,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    z: f32,
    albedo: C3,
    n: V3,
    off: f32,
    side: bool,
) {
    let q = if side {
        [
            v(z + off, y0, x0),
            v(z + off, y0, x1),
            v(z + off, y1, x1),
            v(z + off, y1, x0),
        ]
    } else {
        [
            v(x0, y0, z + off),
            v(x1, y0, z + off),
            v(x1, y1, z + off),
            v(x0, y1, z + off),
        ]
    };
    let ctr = v(
        (q[0].x + q[2].x) / 2.0,
        (y0 + y1) / 2.0,
        (q[0].z + q[2].z) / 2.0,
    );
    let col = c.light.lit(albedo, n, ctr);
    c.quad(&q, col, 1.0, 0.0);
}

fn glow(c: &mut Ctx, p: V3, rx: f32, ry: f32, col: C3, op: f32) {
    let s = c.sp(p);
    c.p.ellipse(
        s,
        rx,
        ry,
        rad(s, rx, ry, &[(0.0, col, op), (1.0, col, 0.0)]),
        1.0,
    );
}

/// An uplight's wash on the wall: a warm cone widening upward.
fn wash(c: &mut Ctx, x: f32, zb: f32, w: f32, h: f32, op: f32) {
    let pts = c.sps(&[
        v(x - 8.0, 0.0, zb + 1.0),
        v(x + 8.0, 0.0, zb + 1.0),
        v(x + w / 2.0, h, zb + 1.0),
        v(x - w / 2.0, h, zb + 1.0),
    ]);
    let (a, b) = (c.sp(v(x, 0.0, zb)), c.sp(v(x, h, zb)));
    c.p.poly(
        pts,
        lin_a(a, b, &[(0.0, ink::WASH, op), (1.0, ink::WASH, 0.0)]),
        1.0,
        0.0,
    );
}

fn booth(c: &mut Ctx, w: &W, cx: f32, cz: f32, xs: f32, zb: f32) {
    if !matches!(w.pattern, Pattern::Acoustic | Pattern::Brick) {
        for (i, x) in steps((cx - 170.0) as i32, (cx + 200.0) as i32, 95).enumerate() {
            let alb = if i % 3 != 1 { w.fabric } else { w.fabric2 };
            let x = x as f32;
            panel(
                c,
                x,
                x + 70.0,
                70.0,
                210.0,
                zb,
                alb,
                v(0.0, 0.0, 1.0),
                4.0,
                false,
            );
        }
    }
    let (z0, y0, s) = (cz - 60.0, 90.0, 13.0);
    for i in 0..7 {
        for j in 0..7 {
            let dpt = ((i * i + j * j) % 7) as f32 / 6.0;
            let (fi, fj) = (i as f32, j as f32);
            let col = c.light.lit(
                scale(w.wood, 0.55 + 0.45 * dpt),
                v(1.0, 0.0, 0.0),
                v(xs, y0 + fj * s, z0 + fi * s),
            );
            let xx = xs + 2.0 + dpt * 6.0;
            let q = [
                v(xx, y0 + fj * s, z0 + fi * s),
                v(xx, y0 + fj * s, z0 + (fi + 1.0) * s - 1.5),
                v(xx, y0 + (fj + 1.0) * s - 1.5, z0 + (fi + 1.0) * s - 1.5),
                v(xx, y0 + (fj + 1.0) * s - 1.5, z0 + fi * s),
            ];
            c.quad(&q, col, 1.0, 0.0);
        }
    }
    let lp = v(cx + 150.0, 200.0, zb + 6.0);
    let g = 0.45 * c.light.m.glow;
    glow(c, lp, 130.0, 100.0, ink::LAMP, g);
    let s = c.sp(lp);
    let bulb = if c.light.evening {
        ink::BULB_EVENING
    } else {
        ink::BULB_DAY
    };
    c.p.ellipse(s, 4.0, 3.0, Paint::Solid(bulb), 1.0);
}

#[allow(clippy::too_many_arguments)]
fn studio(c: &mut Ctx, w: &W, f: &F, cx: f32, xs: f32, xr: f32, zb: f32, h: f32) {
    let n = v(0.0, 0.0, 1.0);
    if !matches!(w.pattern, Pattern::Acoustic | Pattern::Brick) {
        let mut x = cx - 40.0;
        let mut k = 0usize;
        while x < xr - 10.0 {
            let d = [6.0, 2.0, 10.0, 4.0, 8.0, 3.0, 12.0][k % 7];
            let alb = if k % 2 == 1 {
                w.wood
            } else {
                scale(w.wood, 0.86)
            };
            let col = c.light.lit(alb, n, v(x, h * 0.4, zb));
            let lo = add(
                c.light.lit(alb, n, v(x, 30.0, zb)),
                c.light
                    .lit_s(bounce(c, f, x, zb), up(), v(x, 0.0, zb), false),
            );
            let q = [
                v(x, 12.0, zb + d),
                v(x + 7.0, 12.0, zb + d),
                v(x + 7.0, h - 20.0, zb + d),
                v(x, h - 20.0, zb + d),
            ];
            let (p0, p1) = (c.sp(q[0]), c.sp(q[3]));
            let sp = c.sps(&q);
            c.p.poly(
                sp,
                lin(p0, p1, &[(0.0, lo), (0.4, col), (1.0, scale(col, 0.7))]),
                1.0,
                0.0,
            );
            x += 9.5;
            k += 1;
        }
    }
    let (wx0, wx1, wy0, wy1) = (cx - 260.0, cx - 70.0, 110.0, 230.0);
    let fr = c.light.lit(w.wood, n, v((wx0 + wx1) / 2.0, 170.0, zb));
    let z = zb + 2.0;
    c.quad(
        &[
            v(wx0 - 8.0, wy0 - 8.0, z),
            v(wx1 + 8.0, wy0 - 8.0, z),
            v(wx1 + 8.0, wy1 + 8.0, z),
            v(wx0 - 8.0, wy1 + 8.0, z),
        ],
        fr,
        1.0,
        0.0,
    );
    let glass = if c.light.evening {
        ink::GLASS_EVENING
    } else {
        ink::GLASS_DAY
    };
    let z = zb + 3.0;
    c.quad(
        &[
            v(wx0, wy0, z),
            v(wx1, wy0, z),
            v(wx1, wy1, z),
            v(wx0, wy1, z),
        ],
        glass,
        1.0,
        0.0,
    );
    let sc = c.sp(v((wx0 + wx1) / 2.0 + 30.0, wy0 + 45.0, z));
    c.p.ellipse(sc, 14.0, 6.0, Paint::Solid(ink::SCREEN), 0.9);
    for i in 0..26 {
        let p = c.sp(v(
            wx0 + 14.0 + i as f32 * 6.6,
            wy0 + 14.0 + (i % 3) as f32 * 3.0,
            zb + 3.2,
        ));
        c.p.ellipse(p, 0.9, 0.9, Paint::Solid(ink::LEDS[(i * 7) % 4]), 0.95);
    }
    let (a, b) = (
        c.sp(v(wx0 + 20.0, wy1 - 6.0, zb + 3.4)),
        c.sp(v(wx0 + 90.0, wy0 + 6.0, zb + 3.4)),
    );
    let op = if c.light.evening { 0.06 } else { 0.12 };
    c.p.line(a, b, 10.0, Paint::Solid(ink::GLINT), Cap::Round, op);
    if w.pattern != Pattern::Acoustic {
        for z in steps((zb + 80.0) as i32, (zb + 420.0) as i32, 130) {
            let z = z as f32;
            panel(
                c,
                z,
                z + 90.0,
                80.0,
                240.0,
                xs,
                w.fabric,
                v(1.0, 0.0, 0.0),
                4.0,
                true,
            );
        }
    }
}

fn hall(c: &mut Ctx, w: &W, xs: f32, xr: f32, zb: f32, h: f32) {
    let mut x = xs + 60.0;
    let mut k = 0usize;
    while x < xr {
        let col = c.light.lit(w.wood, v(0.0, 0.0, 1.0), v(x, 200.0, zb));
        let q = [
            v(x, 0.0, zb + 18.0),
            v(x + 16.0, 0.0, zb + 18.0),
            v(x + 16.0, h * 0.85, zb + 18.0),
            v(x, h * 0.85, zb + 18.0),
        ];
        let (p0, p1) = (c.sp(q[0]), c.sp(q[3]));
        let sp = c.sps(&q);
        c.p.poly(
            sp,
            lin(p0, p1, &[(0.0, col), (1.0, scale(col, 0.45))]),
            1.0,
            0.0,
        );
        let side = c
            .light
            .lit(scale(w.wood, 0.6), v(1.0, 0.0, 0.0), v(x + 16.0, 200.0, zb));
        c.quad(
            &[
                v(x + 16.0, 0.0, zb),
                v(x + 16.0, 0.0, zb + 18.0),
                v(x + 16.0, h * 0.85, zb + 18.0),
                v(x + 16.0, h * 0.85, zb),
            ],
            side,
            1.0,
            0.0,
        );
        if k.is_multiple_of(2) {
            let op = 0.42 * c.light.m.glow + 0.06;
            wash(c, x + 50.0, zb, 120.0, 420.0, op);
            let f = c.sp(v(x + 50.0, 2.0, zb + 10.0));
            c.p.ellipse(f, 4.0, 2.0, Paint::Solid(ink::UPLIGHT_FOOT), 0.9);
        }
        x += 110.0;
        k += 1;
    }
}

fn church(c: &mut Ctx, cx: f32, zb: f32) {
    let light = if c.light.evening {
        ink::WINDOW_EVENING
    } else {
        ink::WINDOW_DAY
    };
    for wx in [cx - 300.0, cx - 60.0, cx + 180.0, cx + 420.0] {
        let z = zb + 1.0;
        let mut pts = vec![
            v(wx, 90.0, z),
            v(wx + 70.0, 90.0, z),
            v(wx + 70.0, 330.0, z),
        ];
        for k in 0..9 {
            let a = std::f32::consts::PI * k as f32 / 8.0;
            pts.push(v(
                wx + 35.0 + 35.0 * a.cos(),
                330.0 + 80.0 * a.sin().max(0.0).powf(0.8),
                z,
            ));
        }
        pts.push(v(wx, 330.0, z));
        let sp = c.sps(&pts);
        c.p.poly(sp, Paint::Solid(light), 0.95, 0.0);
        let cc = c.sp(v(wx + 35.0, 250.0, z));
        c.p.ellipse(
            cc,
            90.0,
            160.0,
            rad(cc, 90.0, 160.0, &[(0.0, light, 0.35), (1.0, light, 0.0)]),
            1.0,
        );
        if !c.light.evening {
            let f = c.sps(&[
                v(wx, 0.4, zb + 40.0),
                v(wx + 70.0, 0.4, zb + 40.0),
                v(wx + 160.0, 0.4, zb + 420.0),
                v(wx + 60.0, 0.4, zb + 420.0),
            ]);
            c.p.poly(f, Paint::Solid(ink::WINDOW_DAY), 0.12, 0.0);
        }
    }
}

/// A dark stage: a velvet curtain in deep folds, barely touched by the key; warm uplights at
/// its foot light it in soft tall pools (the folds still showing through); the rig's lamps
/// glow far up, out of focus.
#[allow(clippy::too_many_arguments)]
fn stage(c: &mut Ctx, w: &W, cx: f32, xs: f32, xr: f32, zb: f32, h: f32) {
    let ups: Vec<f32> = steps((cx - 470.0) as i32, xr as i32, 330)
        .map(|u| u as f32)
        .collect();
    let glow = c.light.m.glow;
    let n = v(0.0, 0.0, 1.0);
    let z = zb + 8.0;
    let mut x = xs;
    let mut k = 0usize;
    while x < xr {
        let kf = k as f32;
        let fw = 22.0 + 10.0 * (0.5 + 0.5 * (kf * 2.3).sin());
        let t = 0.5 + 0.5 * (kf * 1.7).sin() * (kf * 0.6).cos();
        let base = lerp(w.fabric, w.fabric2, t);
        let lo = c.light.lit(base, n, v(x + fw / 2.0, 40.0, zb));
        let q = [v(x, 0.0, z), v(x + fw, 0.0, z), v(x + fw, h, z), v(x, h, z)];
        let (a0, a1) = (c.sp(v(x, h * 0.4, z)), c.sp(v(x + fw, h * 0.4, z)));
        let sp = c.sps(&q);
        let g = lin(
            a0,
            a1,
            // Velvet: a soft crest, deep troughs — dark folds, not lit bars (round 40 cp5).
            &[
                (0.0, scale(lo, 0.30)),
                (0.42, scale(lo, 0.95)),
                (0.58, scale(lo, 0.85)),
                (1.0, scale(lo, 0.28)),
            ],
        );
        c.p.poly(sp.clone(), g, 1.0, 0.6);
        // The uplights' warmth on this fold: strong at its foot, fading up.
        let wk: f32 = ups
            .iter()
            .map(|u| (-((x + fw / 2.0 - u) / 52.0).powi(2)).exp())
            .sum();
        if wk > 0.02 {
            let (g0, g1) = (
                c.sp(v(x + fw / 2.0, 0.0, z)),
                c.sp(v(x + fw / 2.0, 420.0, z)),
            );
            let amp = wk.min(1.0) * (0.30 * glow + 0.04);
            let warm = ink::CURTAIN_WARM;
            c.p.poly(
                sp,
                lin_a(
                    g0,
                    g1,
                    &[(0.0, warm, amp), (0.35, warm, amp * 0.55), (1.0, warm, 0.0)],
                ),
                1.0,
                0.0,
            );
            let crest = c.sps(&[
                v(x + fw * 0.4, 0.0, z + 0.2),
                v(x + fw * 0.6, 0.0, z + 0.2),
                v(x + fw * 0.6, 380.0, z + 0.2),
                v(x + fw * 0.4, 380.0, z + 0.2),
            ]);
            let cc = ink::CURTAIN_CREST;
            c.p.poly(
                crest,
                lin_a(g0, g1, &[(0.0, cc, amp * 0.35), (1.0, cc, 0.0)]),
                1.0,
                0.0,
            );
        }
        x += fw;
        k += 1;
    }
    // Each uplight's pool on the folds: warm and bright at its foot, gone a man's height up
    // (what a stage's photograph shows behind the instrument).
    for &ux in &ups {
        let (pa, pb) = (c.sp(v(ux - 70.0, 60.0, z)), c.sp(v(ux + 70.0, 60.0, z)));
        let rx = (pb.x - pa.x).abs() * 0.5;
        let ry = (c.sp(v(ux, 0.0, z)).y - c.sp(v(ux, 170.0, z)).y).abs() * 0.5;
        let pc = c.sp(v(ux, 55.0, z));
        let warm = ink::CURTAIN_WARM;
        let amp = 0.55 * glow + 0.05;
        c.p.ellipse(
            pc,
            rx,
            ry,
            rad(
                pc,
                rx,
                ry,
                &[(0.0, warm, amp), (0.45, warm, amp * 0.45), (1.0, warm, 0.0)],
            ),
            1.0,
        );
    }
    for &ux in &ups {
        let f = c.sp(v(ux, 3.0, zb + 18.0));
        c.p.ellipse(f, 6.0, 2.6, Paint::Solid(ink::STAGE_FOOT), 0.95);
        // The lamp itself, lit: a warm point and its small halo.
        let l = c.sp(v(ux, 7.0, zb + 18.0));
        let u = ink::UPLIGHT;
        c.p.ellipse(
            l,
            9.0,
            6.0,
            rad(l, 9.0, 6.0, &[(0.0, u, 0.55 * glow), (1.0, u, 0.0)]),
            1.0,
        );
        c.p.ellipse(l, 2.2, 1.5, Paint::Solid(u), 0.9 * glow + 0.1);
    }
    for i in 0..8 {
        let lx = cx - 460.0 + i as f32 * 140.0;
        let p = c.sp(v(lx, 300.0, zb + 40.0));
        let l = ink::RIG_LAMP;
        let g = rad(
            p,
            11.0,
            11.0,
            &[(0.0, l, 0.50 * glow + 0.08), (0.5, l, 0.18), (1.0, l, 0.0)],
        );
        c.p.ellipse(p, 11.0, 11.0, g, 1.0);
    }
}
