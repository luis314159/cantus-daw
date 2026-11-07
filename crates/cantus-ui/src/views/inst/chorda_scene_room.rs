//! The room (ADR 0104): two walls meeting in the corner behind the piano, the floor, and
//! what the room is made of — from the Room controls. Its size (`roomsize`) sets how far
//! the walls stand and how high they rise; its level (`room`, how live it is) thins a small
//! room's absorbers and polishes a big room's floor.
//!
//! | Size | The room |
//! |---|---|
//! | under 0.33 | a booth: plaster, fabric absorbers, a wooden diffuser, a lamp, a rug |
//! | under 0.66 | a studio's live room: wood slats, the control room's window, a rug |
//! | above | a hall or scoring stage: tall wooden fins, uplights, a polished stage |
//! | 0.95 and live past 0.85 | a church: limestone, tall windows, flagstones |

use super::geo::{Geo, M};
use super::mesh::{v3, V3};
use crate::kit::skin::scene::{self as pal, RoomPaint};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Booth,
    Studio,
    Hall,
    Church,
}

/// The room's plan: the back walls' planes, the height, what it is.
#[derive(Copy, Clone, Debug)]
pub struct Room {
    pub kind: Kind,
    pub size: f32,
    pub live: f32,
    /// The left wall's plane (x = `x0`) and the back wall's (z = `z0`).
    pub x0: f32,
    pub z0: f32,
    pub height: f32,
    pub paint: RoomPaint,
}

impl Room {
    /// The room for the controls, behind a piano whose tail is at z = `-l`.
    pub fn of(size: f32, live: f32, l: f32) -> Room {
        let size = size.clamp(0.0, 1.0);
        let live = live.clamp(0.0, 1.0);
        let kind = if size < 0.33 {
            Kind::Booth
        } else if size < 0.66 {
            Kind::Studio
        } else if size >= 0.95 && live >= 0.85 {
            Kind::Church
        } else {
            Kind::Hall
        };
        let paint = match kind {
            Kind::Booth => pal::BOOTH,
            Kind::Studio => pal::STUDIO,
            Kind::Hall => pal::HALL,
            Kind::Church => pal::CHURCH,
        };
        let s2 = size * size;
        Room {
            kind,
            size,
            live,
            x0: -(0.9 + 8.0 * s2),
            z0: -l - (0.9 + 10.0 * s2),
            height: 2.9 + 9.0 * s2,
            paint,
        }
    }

    /// How much of the piano the floor mirrors (0: none, a matte or covered floor).
    pub fn mirror(&self) -> f32 {
        if matches!(self.kind, Kind::Booth | Kind::Studio) {
            return 0.0;
        }
        self.paint.floor_gloss * (0.35 + 0.65 * self.live) * 0.5
    }

    /// Whether a rug lies under the piano.
    pub fn has_rug(&self) -> bool {
        matches!(self.kind, Kind::Booth | Kind::Studio)
    }
}

/// The walls, the floor, the rug and the room's furnishing; `xr`, `zr`: how far the walls
/// run toward the camera's side (past the frame).
pub fn build(g: &mut Geo, room: &Room, rug: Option<((f32, f32), (f32, f32))>) {
    let eye = g.cam.eye;
    let (xr, zr) = (eye.x + 2.0, eye.z + 2.0);
    let h = room.height.max(eye.y + 6.0);
    let back = |p: V3| [p.x, p.y];
    let left = |p: V3| [p.z, p.y];
    g.quad(
        [
            v3(room.x0, 0.0, room.z0),
            v3(xr, 0.0, room.z0),
            v3(xr, h, room.z0),
            v3(room.x0, h, room.z0),
        ],
        v3(0.0, 0.0, 1.0),
        back,
        M::Wall,
        0,
    );
    g.quad(
        [
            v3(room.x0, 0.0, zr),
            v3(room.x0, 0.0, room.z0),
            v3(room.x0, h, room.z0),
            v3(room.x0, h, zr),
        ],
        v3(1.0, 0.0, 0.0),
        left,
        M::Wall2,
        0,
    );
    // A skirting board along both walls.
    g.boxed(
        v3(room.x0, 0.0, room.z0),
        v3(xr, 0.09, room.z0 + 0.02),
        0.004,
        M::WoodPanel,
        1,
    );
    g.boxed(
        v3(room.x0, 0.0, room.z0),
        v3(room.x0 + 0.02, 0.09, zr),
        0.004,
        M::WoodPanel,
        1,
    );
    // The floor, to a metre in front of the camera's feet.
    let (xf, zf) = (eye.x - 1.0, eye.z + 1.0);
    g.quad(
        [
            v3(room.x0, 0.0, room.z0),
            v3(room.x0, 0.0, zf),
            v3(xf, 0.0, zf),
            v3(xf, 0.0, room.z0),
        ],
        v3(0.0, 1.0, 0.0),
        |p| [p.x, p.z],
        M::Floor,
        0,
    );
    if let Some(((x0, z0), (x1, z1))) = rug {
        g.boxed(v3(x0, 0.0, z0), v3(x1, 0.008, z1), 0.004, M::Rug, 0);
    }
    match room.kind {
        Kind::Booth => {
            panels(g, room, 0.9, 2.1, 0.62, 0.95);
            diffuser(g, room);
            lamp(g, v3(room.x0 + 0.02, 2.0, room.z0 + 2.4));
        }
        Kind::Studio => {
            slats(g, room, xr);
            window(g, room);
            panels(g, room, 1.0, 2.3, 0.7, 1.3);
        }
        Kind::Hall => {
            fins(g, room, xr, zr);
            uplights(g, room, xr, zr);
        }
        Kind::Church => windows(g, room, xr, zr),
    }
}

fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9).wrapping_add(0x7F4A_7C15);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// Fabric absorbers on both walls between `y0` and `y1`, `pw` wide every `step` metres —
/// fewer the livelier the room.
fn panels(g: &mut Geo, room: &Room, y0: f32, y1: f32, pw: f32, step: f32) {
    let keep = 1.0 - 0.75 * room.live;
    for wall in 0..2u32 {
        let (start, len) = if wall == 0 {
            (room.x0 + 0.35, 7.0)
        } else {
            (room.z0 + 0.35, 6.0)
        };
        let n = (len / step) as u32;
        for i in 0..n {
            if hash(i * 17 + wall * 101 + 5) > keep {
                continue;
            }
            let a = start + i as f32 * step;
            let m = if (i + wall) % 2 == 0 {
                M::Fabric
            } else {
                M::Fabric2
            };
            if wall == 0 {
                g.boxed(
                    v3(a, y0, room.z0),
                    v3(a + pw, y1, room.z0 + 0.07),
                    0.018,
                    m,
                    0,
                );
            } else {
                g.boxed(
                    v3(room.x0, y0, a),
                    v3(room.x0 + 0.07, y1, a + pw),
                    0.018,
                    m,
                    0,
                );
            }
        }
    }
}

/// A quadratic-residue diffuser on the back wall: 7 × 7 wooden wells of seven depths.
fn diffuser(g: &mut Geo, room: &Room) {
    let cell = 0.11;
    let (x0, y0) = (0.15, 1.0);
    for j in 0..7 {
        for i in 0..7 {
            let d = ((i * i + j * j) % 7) as f32 * 0.024 + 0.012;
            let a = x0 + i as f32 * cell;
            let b = y0 + j as f32 * cell;
            g.boxed(
                v3(a + 0.004, b + 0.004, room.z0),
                v3(a + cell - 0.004, b + cell - 0.004, room.z0 + d),
                0.003,
                M::WoodPanel,
                0,
            );
        }
    }
    let s = cell * 7.0;
    for (lo, hi) in [
        (
            v3(x0 - 0.02, y0 - 0.02, room.z0),
            v3(x0 + s + 0.02, y0, room.z0 + 0.18),
        ),
        (
            v3(x0 - 0.02, y0 + s, room.z0),
            v3(x0 + s + 0.02, y0 + s + 0.02, room.z0 + 0.18),
        ),
        (v3(x0 - 0.02, y0, room.z0), v3(x0, y0 + s, room.z0 + 0.18)),
        (
            v3(x0 + s, y0, room.z0),
            v3(x0 + s + 0.02, y0 + s, room.z0 + 0.18),
        ),
    ] {
        g.boxed(lo, hi, 0.004, M::WoodPanel, 0);
    }
}

/// A wall lamp: a brass back plate, an opal glass shade glowing (its bloom is the baker's).
fn lamp(g: &mut Geo, at: V3) {
    g.disc(at, v3(1.0, 0.0, 0.0), 0.06, 20, M::Brass, 0);
    g.tube(at, at + v3(0.12, 0.0, 0.0), 0.008, 0.008, 8, M::Brass, 0);
    g.sphere(at + v3(0.14, -0.02, 0.0), 0.07, 8, M::Glow, 0);
}

/// Vertical wood slats over the back wall, their gaps dark between them.
fn slats(g: &mut Geo, room: &Room, xr: f32) {
    let top = room.height - 0.2;
    let x1 = xr.min(room.x0 + 8.0);
    let mut x = room.x0 + 0.25;
    let mut k = 0u8;
    while x + 0.07 < x1 {
        g.boxed(
            v3(x, 0.12, room.z0 + 0.01),
            v3(x + 0.07, top, room.z0 + 0.05),
            0.006,
            M::WoodPanel,
            k,
        );
        x += 0.105;
        k = k.wrapping_add(1);
    }
    g.boxed(
        v3(room.x0, 0.12, room.z0),
        v3(x1, top, room.z0 + 0.012),
        0.0,
        M::Fabric,
        0,
    );
}

/// The control room's window in the left wall: a wooden frame, dark glass, the console's
/// lights beyond it.
fn window(g: &mut Geo, room: &Room) {
    let (z0, z1, y0, y1) = (room.z0 + 0.8, room.z0 + 3.2, 0.95, 2.15);
    let x = room.x0;
    g.quad(
        [
            v3(x + 0.03, y0, z1),
            v3(x + 0.03, y0, z0),
            v3(x + 0.03, y1, z0),
            v3(x + 0.03, y1, z1),
        ],
        v3(1.0, 0.0, 0.0),
        |p| [p.z, p.y],
        M::Glass,
        0,
    );
    for (lo, hi) in [
        (v3(x, y0 - 0.08, z0 - 0.08), v3(x + 0.1, y0, z1 + 0.08)),
        (v3(x, y1, z0 - 0.08), v3(x + 0.1, y1 + 0.08, z1 + 0.08)),
        (v3(x, y0, z0 - 0.08), v3(x + 0.1, y1, z0)),
        (v3(x, y0, z1), v3(x + 0.1, y1, z1 + 0.08)),
        (v3(x, y0 - 0.1, z0 - 0.1), v3(x + 0.16, y0 - 0.06, z1 + 0.1)),
    ] {
        g.boxed(lo, hi, 0.01, M::WoodPanel, 3);
    }
    // The console's lights behind the glass, and a screen's glow.
    for i in 0..30u32 {
        let z = z0 + 0.2 + (z1 - z0 - 0.4) * (i as f32 / 29.0);
        let y = y0 + 0.16 + 0.05 * (hash(i * 3) - 0.5) + if i % 2 == 0 { 0.0 } else { 0.05 };
        g.disc(
            v3(x + 0.032, y, z),
            v3(1.0, 0.0, 0.0),
            0.006,
            8,
            M::Glow,
            2 + (hash(i * 7 + 1) * 3.99) as u8,
        );
    }
    let sc = v3(x + 0.031, y0 + 0.42, (z0 + z1) * 0.5);
    g.quad(
        [
            sc + v3(0.0, -0.12, 0.35),
            sc + v3(0.0, -0.12, -0.35),
            sc + v3(0.0, 0.12, -0.35),
            sc + v3(0.0, 0.12, 0.35),
        ],
        v3(1.0, 0.0, 0.0),
        |p| [p.z, p.y],
        M::Glow,
        6,
    );
}

/// A hall's wooden fins: tall boards standing off both walls.
fn fins(g: &mut Geo, room: &Room, xr: f32, zr: f32) {
    let top = room.height;
    for wall in 0..2u32 {
        let (s0, s1) = if wall == 0 {
            (room.x0, xr.min(room.x0 + 24.0))
        } else {
            (room.z0, zr.min(room.z0 + 24.0))
        };
        let n = (((s1 - s0) / 0.9) as usize).min(40);
        for i in 0..n {
            let s = s0 + 0.4 + i as f32 * 0.9;
            if wall == 0 {
                g.boxed(
                    v3(s, 0.3, room.z0 + 0.04),
                    v3(s + 0.3, top, room.z0 + 0.16),
                    0.01,
                    M::WoodPanel,
                    i as u8,
                );
            } else {
                g.boxed(
                    v3(room.x0 + 0.04, 0.3, s),
                    v3(room.x0 + 0.16, top, s + 0.3),
                    0.01,
                    M::WoodPanel,
                    i as u8,
                );
            }
        }
    }
}

/// Warm uplights at the foot of a hall's walls (their washes are the bloom's).
fn uplights(g: &mut Geo, room: &Room, xr: f32, zr: f32) {
    for i in 0..10u32 {
        let p = if i < 6 {
            v3(room.x0 + 1.0 + i as f32 * 2.6, 0.0, room.z0 + 0.25)
        } else {
            v3(room.x0 + 0.25, 0.0, room.z0 + 1.6 + (i - 6) as f32 * 2.6)
        };
        if p.x > xr || p.z > zr {
            continue;
        }
        g.tube(p, p + v3(0.0, 0.08, 0.0), 0.06, 0.05, 12, M::Stand, 0);
        g.disc(
            p + v3(0.0, 0.081, 0.0),
            v3(0.0, 1.0, 0.0),
            0.045,
            12,
            M::Glow,
            0,
        );
    }
}

/// A church's tall windows on both walls: lancets of light in stone.
fn windows(g: &mut Geo, room: &Room, xr: f32, zr: f32) {
    let _ = (xr, zr);
    for i in 0..4u32 {
        let on_back = i < 2;
        let s = if on_back {
            room.x0 + 2.2 + i as f32 * 3.4
        } else {
            room.z0 + 2.4 + (i - 2) as f32 * 3.6
        };
        let (y0, y1, half) = (1.1, 3.9, 0.5);
        let at = |s: f32, y: f32, d: f32| {
            if on_back {
                v3(s, y, room.z0 + d)
            } else {
                v3(room.x0 + d, y, s)
            }
        };
        let n = if on_back {
            v3(0.0, 0.0, 1.0)
        } else {
            v3(1.0, 0.0, 0.0)
        };
        // The light: the straight part and the pointed arch, in strips.
        let steps = 8;
        for k in 0..steps {
            let a0 = k as f32 / steps as f32;
            let a1 = (k + 1) as f32 / steps as f32;
            let w = |a: f32| {
                half * (1.0 - (a * std::f32::consts::FRAC_PI_2).sin())
                    .max(0.0)
                    .sqrt()
            };
            let ya = y1 + half * 1.6 * a0;
            let yb = y1 + half * 1.6 * a1;
            g.quad(
                [
                    at(s - w(a0), ya, 0.01),
                    at(s + w(a0), ya, 0.01),
                    at(s + w(a1), yb, 0.01),
                    at(s - w(a1), yb, 0.01),
                ],
                n,
                |p| [p.x, p.y],
                M::Glow,
                1,
            );
        }
        g.quad(
            [
                at(s - half, y0, 0.01),
                at(s + half, y0, 0.01),
                at(s + half, y1, 0.01),
                at(s - half, y1, 0.01),
            ],
            n,
            |p| [p.x, p.y],
            M::Glow,
            1,
        );
        // The mullion and the sill, in stone.
        let (lo, hi) = if on_back {
            (
                v3(s - 0.03, y0, room.z0),
                v3(s + 0.03, y1 + 0.7, room.z0 + 0.05),
            )
        } else {
            (
                v3(room.x0, y0, s - 0.03),
                v3(room.x0 + 0.05, y1 + 0.7, s + 0.03),
            )
        };
        g.boxed(lo, hi, 0.005, M::Wall, 0);
        let (lo, hi) = if on_back {
            (
                v3(s - half - 0.1, y0 - 0.12, room.z0),
                v3(s + half + 0.1, y0, room.z0 + 0.18),
            )
        } else {
            (
                v3(room.x0, y0 - 0.12, s - half - 0.1),
                v3(room.x0 + 0.18, y0, s + half + 0.1),
            )
        };
        g.boxed(lo, hi, 0.01, M::Wall, 0);
    }
}
