//! r40:scene-px — the rooms for the photographic renderer, from the scene module's own
//! (`scene::rooms`: each room's size, its floor, walls and rug, overridden by the look):
//! two walls meeting behind the instrument and the floor running toward us, as surfaces the
//! renderer textures per sample (boards, slabs, terrazzo, flagstones; plaster, panels,
//! acoustic tiles, brick, limestone, a velvet curtain in folds; the rug's weave and
//! border), and each room's furniture in 3-D — the stage's uplights and rig lamps, the
//! booth's fabric panels, diffuser and lamp, the studio's slats and control-room window,
//! the hall's fins and their washes, the church's lancet windows.

use super::geo::{cm, Geo};
use super::light::{Lamp, Panel, RoomEnv};
use super::mat::{lin3, PMat, Tex};
use super::mesh::{lscale, v3};
use crate::kit::skin::scene::{ink, Pattern};
use crate::scene::rooms::{def, floor_paint, rug_paint, walls_paint};
use crate::scene::Room;
use cantus_core::inst::room_look::RoomLook;

/// What a room adds to the light: its lamps, and its bright windows as panels.
pub struct Built {
    pub env: RoomEnv,
    pub lamps: Vec<Lamp>,
    pub panels: Vec<Panel>,
}

fn steps(a: f32, b: f32, s: f32) -> impl Iterator<Item = f32> {
    let n = ((b - a) / s).ceil().max(0.0) as usize;
    (0..n).map(move |k| a + k as f32 * s)
}

/// Build the room `kind` round `centre` (cm) with its look; the camera's z (cm) bounds the
/// floor toward us; `glow` the mood's lamps (1 by evening).
pub fn build(
    g: &mut Geo,
    kind: Room,
    centre: (f32, f32),
    look: RoomLook,
    cam_z: f32,
    evening: bool,
    glow: f32,
) -> Built {
    let r = def(kind);
    let f = floor_paint(look.floor.unwrap_or(r.floor));
    let w = walls_paint(look.walls.unwrap_or(r.walls));
    let w = if evening {
        w
    } else {
        crate::kit::skin::scene::daylit(w)
    };
    let rug = rug_paint(look.rug.unwrap_or(r.rug));
    let (cx, cz) = centre;
    let zb = cz - r.back;
    let xs = cx - r.side;
    let xr = cx + r.side * 1.6;
    let zf = (cz + r.back * 2.4).min(cam_z - 60.0);
    let h = r.height;
    let curtain = w.pattern == Pattern::Curtain;
    let floor = g.mat(PMat::lacquer(lin3(f.a), f.coat, 0.2).with(Tex::Floor));
    let (wall, wall2) = if curtain {
        let c = g.mat(PMat::matte(lin3(w.fabric)).with(Tex::Curtain));
        (c, c)
    } else {
        (
            g.mat(PMat::matte(lin3(w.wall)).with(Tex::Wall)),
            g.mat(PMat::matte(lin3(w.wall2)).with(Tex::Wall2)),
        )
    };
    let dark = g.mat(PMat::matte(lscale(lin3(w.wall2), 0.25)));
    g.quad_uv(
        [
            cm(xs, 0.0, zf),
            cm(xr, 0.0, zf),
            cm(xr, 0.0, zb),
            cm(xs, 0.0, zb),
        ],
        v3(0.0, 1.0, 0.0),
        floor,
    );
    g.quad_uv(
        [
            cm(xs, 0.0, zb),
            cm(xr, 0.0, zb),
            cm(xr, h, zb),
            cm(xs, h, zb),
        ],
        v3(0.0, 0.0, 1.0),
        wall,
    );
    g.quad_uv(
        [
            cm(xs, 0.0, zf),
            cm(xs, 0.0, zb),
            cm(xs, h, zb),
            cm(xs, h, zf),
        ],
        v3(1.0, 0.0, 0.0),
        wall2,
    );
    g.quad_uv(
        [
            cm(xs, h, zb),
            cm(xr, h, zb),
            cm(xr, h * 3.0, zb),
            cm(xs, h * 3.0, zb),
        ],
        v3(0.0, 0.0, 1.0),
        dark,
    );
    g.quad_uv(
        [
            cm(xs, h, zf),
            cm(xs, h, zb),
            cm(xs, h * 3.0, zb),
            cm(xs, h * 3.0, zf),
        ],
        v3(1.0, 0.0, 0.0),
        dark,
    );
    let wood = g.mat(PMat::lacquer(lin3(w.wood), 0.4, 0.25).with(Tex::Wood {
        seed: 61,
        figure: 0.08,
    }));
    let skirt = g.mat(PMat::lacquer(lscale(lin3(w.wood), 0.3), 0.4, 0.2));
    if !curtain {
        g.aabb(cm(xs, 0.0, zb), cm(xr, 9.0, zb + 1.2), skirt);
        g.aabb(cm(xs, 0.0, zb), cm(xs + 1.2, 9.0, zf), skirt);
    }
    let mut lamps = Vec::new();
    let mut panels = Vec::new();
    let warm = |c: [f32; 3], k: f32| lscale(lin3(c), k);
    match kind {
        Room::Stage => {
            let foot = g.mat(PMat::emit(warm(ink::STAGE_FOOT, (6.0 * glow + 0.5) * 0.4)));
            let can = g.mat(PMat::lacquer(lin3([0.02, 0.02, 0.022]), 0.4, 0.2));
            for ux in steps(cx - 470.0, xr, 330.0) {
                g.tube(
                    cm(ux, 0.0, zb + 22.0),
                    cm(ux, 7.0, zb + 18.0),
                    0.06,
                    0.055,
                    18,
                    true,
                    can,
                );
                g.lathe(
                    cm(ux, 7.1, zb + 18.0),
                    v3(0.0, 0.5, -0.86),
                    &[(0.0, 0.0), (0.045, 0.0)],
                    18,
                    foot,
                );
                lamps.push(Lamp {
                    p: cm(ux, 10.0, zb + 45.0),
                    col: warm(ink::CURTAIN_WARM, 2.6 * glow + 0.15),
                    reach: 3.2,
                });
            }
            let rigl = g.mat(PMat::emit(warm(ink::RIG_LAMP, (9.0 * glow + 1.0) * 0.4)));
            for i in 0..8 {
                let lx = cx - 460.0 + i as f32 * 140.0;
                g.lathe(
                    cm(lx, 300.0, zb + 40.0),
                    v3(0.0, -0.3, 1.0),
                    &[(0.0, 0.01), (0.07, 0.0)],
                    20,
                    rigl,
                );
            }
        }
        Room::Booth => {
            if !matches!(w.pattern, Pattern::Acoustic | Pattern::Brick) {
                let (fa, fb) = (
                    g.mat(PMat::matte(lin3(w.fabric)).with(Tex::Fabric)),
                    g.mat(PMat::matte(lin3(w.fabric2)).with(Tex::Fabric)),
                );
                for (i, x) in steps(cx - 170.0, cx + 200.0, 95.0).enumerate() {
                    g.aabb(
                        cm(x, 70.0, zb),
                        cm(x + 70.0, 210.0, zb + 4.0),
                        if i % 3 != 1 { fa } else { fb },
                    );
                }
            }
            let (z0, y0, s) = (cz - 60.0, 90.0, 13.0);
            for i in 0..7 {
                for j in 0..7 {
                    let dpt = ((i * i + j * j) % 7) as f32 / 6.0;
                    let (fi, fj) = (i as f32, j as f32);
                    g.aabb(
                        cm(xs, y0 + fj * s, z0 + fi * s),
                        cm(
                            xs + 2.0 + dpt * 6.0,
                            y0 + (fj + 1.0) * s - 1.5,
                            z0 + (fi + 1.0) * s - 1.5,
                        ),
                        wood,
                    );
                }
            }
            let bulb = g.mat(PMat::emit(warm(
                if evening {
                    ink::BULB_EVENING
                } else {
                    ink::BULB_DAY
                },
                8.0 * glow + 1.0,
            )));
            g.lathe(
                cm(cx + 150.0, 200.0, zb + 8.0),
                v3(0.0, 0.0, 1.0),
                &[(0.0, 0.0), (0.05, 0.0), (0.045, 0.03), (0.0, 0.05)],
                20,
                bulb,
            );
            lamps.push(Lamp {
                p: cm(cx + 150.0, 200.0, zb + 14.0),
                col: warm(ink::LAMP, 1.4 * glow + 0.1),
                reach: 2.4,
            });
        }
        Room::Studio => {
            if !matches!(w.pattern, Pattern::Acoustic | Pattern::Brick) {
                let mut x = cx - 40.0;
                let mut k = 0usize;
                let wood2 = g.mat(PMat::lacquer(lscale(lin3(w.wood), 0.74), 0.4, 0.25).with(
                    Tex::Wood {
                        seed: 67,
                        figure: 0.08,
                    },
                ));
                while x < xr - 10.0 {
                    let d = [6.0, 2.0, 10.0, 4.0, 8.0, 3.0, 12.0][k % 7];
                    g.aabb(
                        cm(x, 12.0, zb),
                        cm(x + 7.0, h - 20.0, zb + d),
                        if k % 2 == 1 { wood } else { wood2 },
                    );
                    x += 9.5;
                    k += 1;
                }
            }
            let (wx0, wx1, wy0, wy1) = (cx - 260.0, cx - 70.0, 110.0, 230.0);
            g.aabb(
                cm(wx0 - 8.0, wy0 - 8.0, zb),
                cm(wx1 + 8.0, wy1 + 8.0, zb + 2.0),
                wood,
            );
            let glass = g.mat(PMat::lacquer(
                lin3(if evening {
                    ink::GLASS_EVENING
                } else {
                    ink::GLASS_DAY
                }),
                1.0,
                0.02,
            ));
            g.quad_uv(
                [
                    cm(wx0, wy0, zb + 2.5),
                    cm(wx1, wy0, zb + 2.5),
                    cm(wx1, wy1, zb + 2.5),
                    cm(wx0, wy1, zb + 2.5),
                ],
                v3(0.0, 0.0, 1.0),
                glass,
            );
            let screen = g.mat(PMat::emit(lscale(lin3(ink::SCREEN), 1.2)));
            let sc = cm((wx0 + wx1) / 2.0 + 30.0, wy0 + 45.0, zb + 2.0);
            g.quad_uv(
                [
                    sc + cm(-14.0, -6.0, 0.0),
                    sc + cm(14.0, -6.0, 0.0),
                    sc + cm(14.0, 6.0, 0.0),
                    sc + cm(-14.0, 6.0, 0.0),
                ],
                v3(0.0, 0.0, 1.0),
                screen,
            );
            let leds: Vec<_> = ink::LEDS
                .iter()
                .map(|&c| g.mat(PMat::emit(lscale(lin3(c), 3.0))))
                .collect();
            for i in 0..26 {
                let p = cm(
                    wx0 + 14.0 + i as f32 * 6.6,
                    wy0 + 14.0 + (i % 3) as f32 * 3.0,
                    zb + 2.0,
                );
                g.aabb(
                    p - cm(0.5, 0.5, 0.0),
                    p + cm(0.5, 0.5, 0.2),
                    leds[(i * 7) % 4],
                );
            }
            if w.pattern != Pattern::Acoustic {
                let fa = g.mat(PMat::matte(lin3(w.fabric)).with(Tex::Fabric));
                for z in steps(zb + 80.0, zb + 420.0, 130.0) {
                    g.aabb(cm(xs, 80.0, z), cm(xs + 4.0, 240.0, z + 90.0), fa);
                }
            }
            lamps.push(Lamp {
                p: cm(cx + 120.0, h - 30.0, zb + 60.0),
                col: warm(ink::LAMP, 0.9 * glow + 0.05),
                reach: 3.5,
            });
        }
        Room::Hall => {
            let foot = g.mat(PMat::emit(warm(
                ink::UPLIGHT_FOOT,
                (5.0 * glow + 0.5) * 0.4,
            )));
            let mut x = xs + 60.0;
            let mut k = 0usize;
            while x < xr {
                g.aabb(cm(x, 0.0, zb), cm(x + 16.0, h * 0.85, zb + 18.0), wood);
                if k.is_multiple_of(2) {
                    g.aabb(
                        cm(x + 46.0, 0.0, zb + 6.0),
                        cm(x + 54.0, 3.0, zb + 14.0),
                        foot,
                    );
                    lamps.push(Lamp {
                        p: cm(x + 50.0, 6.0, zb + 16.0),
                        col: warm(ink::UPLIGHT, (2.4 * glow + 0.2) * 0.35),
                        reach: 4.0,
                    });
                }
                x += 110.0;
                k += 1;
            }
        }
        Room::Church => {
            let col = lin3(if evening {
                ink::WINDOW_EVENING
            } else {
                ink::WINDOW_DAY
            });
            let win = g.mat(PMat::emit(lscale(col, if evening { 2.2 } else { 4.0 })));
            for wx in [cx - 300.0, cx - 60.0, cx + 180.0, cx + 420.0] {
                // The lancet's outline, no point twice (the clipper drops a doubled one).
                let mut pts = vec![(wx, 90.0), (wx + 70.0, 90.0), (wx + 70.0, 330.0)];
                for k in 1..8 {
                    let a = std::f32::consts::PI * k as f32 / 8.0;
                    pts.push((
                        wx + 35.0 + 35.0 * a.cos(),
                        330.0 + 80.0 * a.sin().max(0.0).powf(0.8),
                    ));
                }
                pts.push((wx, 330.0));
                // Its stone reveal round it, a shade darker than the wall.
                let reveal = g.mat(PMat::matte(lscale(lin3(w.wall2), 0.6)));
                g.aabb(
                    cm(wx - 6.0, 84.0, zb),
                    cm(wx + 76.0, 90.0, zb + 2.0),
                    reveal,
                );
                let pm: Vec<(f32, f32)> =
                    pts.iter().map(|&(x, y)| (x / 100.0, y / 100.0)).collect();
                let z = zb / 100.0 + 0.01;
                g.plate_map(&pm, |x, y| v3(x, y, z), v3(0.0, 0.0, 1.0), win);
                panels.push(Panel::toward(
                    cm(wx + 35.0, 250.0, zb + 1.0),
                    cm(wx + 35.0, 250.0, zb + 400.0),
                    0.7,
                    3.2,
                    v3(0.0, 1.0, 0.0),
                    lscale(col, 3.0),
                    0.25,
                ));
            }
        }
    }
    let rug_rect = rug.map(|rg| {
        let (rw, rd) = if kind == Room::Studio {
            (300.0, 230.0)
        } else {
            (250.0, 200.0)
        };
        let rect = [
            (cx - rw / 2.0) / 100.0,
            (cz - rd / 2.0) / 100.0,
            (cx + rw / 2.0) / 100.0,
            (cz + rd / 2.0) / 100.0,
        ];
        let m = g.mat(PMat::matte(lin3(rg.field)).with(Tex::Rug));
        g.aabb(v3(rect[0], 0.0, rect[1]), v3(rect[2], 0.004, rect[3]), m);
        (rg, rect)
    });
    let gloss = f.coat
        * match kind {
            Room::Hall => 0.85,
            Room::Stage => 0.95,
            _ => 0.75,
        };
    Built {
        env: RoomEnv {
            kind,
            x0: xs / 100.0,
            x1: xr / 100.0,
            z0: zb / 100.0,
            z1: zf / 100.0,
            h: h / 100.0,
            floor: f,
            walls: w,
            rug: rug_rect,
            gloss,
        },
        lamps,
        panels,
    }
}
