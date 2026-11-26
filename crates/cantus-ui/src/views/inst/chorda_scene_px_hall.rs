//! r42:arcus-polish3 — the concert hall behind a realistic figure on the stage, for the
//! photographic renderer (the owner, 2026-11-25: "compara con las imágenes reales"; the
//! photographs he chose, ref-04, ref-06, ref-10, show a soloist before an orchestra in a dark
//! warm hall, out of focus). Ours is procedural and anonymous: the stage's floor, a back wall
//! of warm wood panels in pools of light, the orchestra on its risers — seated players as
//! dark forms with violins and bows, cellos, music stands with their small warm lamps —
//! and the hall's lights high up; all of it far enough behind to fall out of focus (the
//! portrait's blur, `raster::post`), so it reads as a hall and never as anyone.

use super::geo::{cm, Geo};
use super::light::{Lamp, Panel, RoomEnv};
use super::mat::{hash3, lin3, MatId, PMat, Tex};
use super::mesh::{lscale, v3, V3};
use super::room::Built;
use crate::scene::figure::concert::Concert;
use crate::scene::rooms::{floor_paint, walls_paint};
use crate::scene::Room;
use cantus_core::inst::room_look::{Floor, Walls};

/// How far behind the subject the back wall stands (cm).
pub const BACK: f32 = 1300.0;

/// The hall's materials.
struct Mats {
    cloth: MatId,
    shirt: MatId,
    skins: [MatId; 3],
    hairs: [MatId; 3],
    varnish: MatId,
    bow: MatId,
    stand: MatId,
    lamp: MatId,
}

/// A place in the hall: `r` to the camera's right of the subject, `y` up from the floor,
/// `d` behind it (cm), as world metres.
struct Frame {
    mid: (f32, f32),
    toward: (f32, f32),
    right: (f32, f32),
}

impl Frame {
    fn at(&self, r: f32, y: f32, d: f32) -> V3 {
        cm(
            self.mid.0 + self.right.0 * r - self.toward.0 * d,
            y,
            self.mid.1 + self.right.1 * r - self.toward.1 * d,
        )
    }

    /// A unit way in the frame (right, up, behind).
    fn way(&self, r: f32, y: f32, d: f32) -> V3 {
        v3(
            self.right.0 * r - self.toward.0 * d,
            y,
            self.right.1 * r - self.toward.1 * d,
        )
        .norm()
    }
}

/// A deterministic 0 … 1 for `i`, `j`.
fn h(i: i32, j: i32, s: u32) -> f32 {
    hash3(i, j, s)
}

/// A ball round `c`, `r` metres.
fn ball(g: &mut Geo, c: V3, r: f32, m: MatId) {
    let prof: Vec<(f32, f32)> = (0..=8)
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 8.0;
            (r * a.cos(), r * a.sin())
        })
        .collect();
    g.lathe(c, v3(0.0, 1.0, 0.0), &prof, 14, m);
}

/// One seated player at (`r`, `d`) on a riser `rise` cm high: its body, head and hair, and
/// what it plays (a violin and its bow, or a cello).
#[allow(clippy::too_many_arguments)]
fn player(g: &mut Geo, f: &Frame, m: &Mats, r: f32, d: f32, rise: f32, k: (i32, i32), cello: bool) {
    let seat = rise + 48.0;
    let tall = 1.0 + 0.08 * (h(k.0, k.1, 3) - 0.5);
    let lean = (h(k.0, k.1, 5) - 0.5) * 8.0;
    let hip = f.at(r, seat, d);
    let neck = f.at(r + lean, seat + 58.0 * tall, d + 2.0);
    g.tube(hip, neck, 0.17, 0.19, 16, true, m.cloth);
    // The shoulders' line.
    g.tube(
        f.at(r - 21.0 + lean, seat + 52.0 * tall, d + 2.0),
        f.at(r + 21.0 + lean, seat + 52.0 * tall, d + 2.0),
        0.075,
        0.075,
        12,
        true,
        m.cloth,
    );
    // Half of them in a white shirt under the black (the photographs' orchestra).
    if h(k.0, k.1, 9) > 0.5 {
        g.tube(
            f.at(r + lean, seat + 36.0 * tall, d - 15.0),
            f.at(r + lean, seat + 56.0 * tall, d - 13.0),
            0.045,
            0.06,
            10,
            true,
            m.shirt,
        );
    }
    let i = ((h(k.0, k.1, 7) * 3.0) as usize).min(2);
    let head = f.at(r + lean, seat + 74.0 * tall, d);
    ball(g, head, 0.105, m.skins[i]);
    ball(g, head + f.way(0.0, 0.35, 1.0) * 0.03, 0.108, m.hairs[i]);
    if cello {
        // A cello between the knees, its neck up past the left shoulder.
        let foot = f.at(r + 4.0, rise + 2.0, d - 30.0);
        let top = f.at(r - 6.0, seat + 66.0, d - 18.0);
        let ax = (top - foot).norm();
        let side = f.way(1.0, 0.0, 0.0);
        let face = ax.cross(side).norm();
        let c = foot + ax * 0.12;
        g.boxed(
            c - side * 0.22 - face * 0.12,
            side * 0.44,
            ax * 0.75,
            face * 0.24,
            m.varnish,
        );
        g.rod(c + ax * 0.75, top + ax * 0.3, 0.018, m.bow);
        let (a, b) = (
            f.at(r - 40.0, seat + 28.0, d - 40.0),
            f.at(r + 30.0, seat + 18.0, d - 46.0),
        );
        g.rod(a, b, 0.004, m.bow);
    } else {
        // A violin under the chin, toward the player's left (the camera's right), and its
        // bow rising from the right hand.
        let chin = f.at(r + 6.0 + lean, seat + 60.0 * tall, d - 12.0);
        let along = f.way(0.75, -0.12, -0.65);
        let up = f.way(0.0, 1.0, 0.0);
        let across = along.cross(up).norm();
        g.boxed(
            chin - across * 0.10,
            along * 0.36,
            across * 0.20,
            v3(0.0, 0.045, 0.0),
            m.varnish,
        );
        g.rod(chin + along * 0.36, chin + along * 0.6, 0.012, m.bow);
        let tilt = (h(k.0, k.1, 11) - 0.5) * 30.0;
        let hand = f.at(r - 18.0, seat + 30.0, d - 30.0);
        let tip = f.at(r - 50.0 + tilt, seat + 100.0 + tilt * 0.6, d - 40.0);
        g.rod(hand, tip, 0.0045, m.bow);
    }
}

/// A music stand at (`r`, `d`) on a riser `rise` high, its desk toward the players behind
/// it, its small lamp lit.
fn stand(g: &mut Geo, f: &Frame, m: &Mats, r: f32, d: f32, rise: f32) {
    let top = rise + STAND_TOP;
    g.rod(f.at(r, rise, d), f.at(r, top, d), 0.01, m.stand);
    let o = f.at(r - 23.0, top, d);
    let back = f.way(0.0, 0.0, 1.0);
    let tilt = (f.way(0.0, 1.0, 0.0) * 0.85 + back * 0.5).norm();
    g.boxed(
        o,
        f.way(1.0, 0.0, 0.0) * 0.46,
        tilt * 0.30,
        back * 0.012,
        m.stand,
    );
    let lamp = f.at(r, top + 28.0, d + 4.0);
    g.lathe(
        lamp,
        f.way(0.0, 0.0, -1.0),
        &[(0.0, 0.0), (0.025, 0.0)],
        12,
        m.lamp,
    );
}

/// The stands' desks' height over their riser (cm).
const STAND_TOP: f32 = 98.0;

/// Build the hall round the subject at `centre` (cm, x and z) as the camera at `eye` (cm)
/// sees it, in `c`'s light.
pub fn build(g: &mut Geo, centre: (f32, f32), eye: (f32, f32), c: &Concert) -> Built {
    let (tx, tz) = {
        let (x, z) = (eye.0 - centre.0, eye.1 - centre.1);
        let l = x.hypot(z).max(1.0);
        (x / l, z / l)
    };
    let f = Frame {
        mid: centre,
        toward: (tx, tz),
        right: (tz, -tx),
    };
    let warm = |col: [f32; 3], k: f32| lscale(lin3(col), k);
    let k = c.hall.max(0.0);
    let floor_m = g.mat(PMat::lacquer(warm([0.20, 0.11, 0.06], 1.0), 0.3, 0.35));
    let wall = |g: &mut Geo, t: f32| {
        g.mat(
            PMat::lacquer(lin3([0.34, 0.16, 0.08]).map(|x| x * t), 0.25, 0.35).with(Tex::Wood {
                seed: 83,
                figure: 0.12,
            }),
        )
    };
    let walls = [wall(g, 1.0), wall(g, 0.8), wall(g, 0.65)];
    let mats = Mats {
        cloth: g.mat(PMat::matte(lin3([0.018, 0.016, 0.016]))),
        shirt: g.mat(PMat::matte(lin3([0.80, 0.78, 0.74]))),
        skins: [
            g.mat(PMat::matte(lin3([0.62, 0.45, 0.36]))),
            g.mat(PMat::matte(lin3([0.48, 0.32, 0.24]))),
            g.mat(PMat::matte(lin3([0.70, 0.52, 0.42]))),
        ],
        hairs: [
            g.mat(PMat::matte(lin3([0.10, 0.07, 0.05]))),
            g.mat(PMat::matte(lin3([0.35, 0.33, 0.31]))),
            g.mat(PMat::matte(lin3([0.25, 0.15, 0.08]))),
        ],
        varnish: g.mat(PMat::lacquer(lin3([0.55, 0.25, 0.09]), 0.5, 0.2)),
        bow: g.mat(PMat::matte(lin3([0.30, 0.16, 0.08]))),
        stand: g.mat(PMat::matte(lin3([0.02, 0.02, 0.022]))),
        lamp: g.mat(PMat::emit(warm([1.0, 0.78, 0.50], 1.6 * c.bokeh))),
    };
    // The stage's floor, from in front of the subject to the back wall.
    let (w0, w1) = (-1600.0, 1600.0);
    g.quad_uv(
        [
            f.at(w0, 0.0, -300.0),
            f.at(w1, 0.0, -300.0),
            f.at(w1, 0.0, BACK),
            f.at(w0, 0.0, BACK),
        ],
        v3(0.0, 1.0, 0.0),
        floor_m,
    );
    // The back wall: tall panels of warm wood, a little apart, each its own shade.
    let n_w = f.way(0.0, 0.0, -1.0);
    let mut x = w0;
    let mut i = 0i32;
    while x < w1 {
        let wd = 70.0 + 30.0 * h(i, 1, 13);
        let m = walls[((h(i, 2, 17) * 3.0) as usize).min(2)];
        let dd = BACK + 6.0 * h(i, 3, 19);
        g.quad_uv(
            [
                f.at(x, 0.0, dd),
                f.at(x + wd - 3.0, 0.0, dd),
                f.at(x + wd - 3.0, 1100.0, dd),
                f.at(x, 1100.0, dd),
            ],
            n_w,
            m,
        );
        x += wd;
        i += 1;
    }
    // The orchestra on its risers: rows of players, a stand to each pair, cellos on the
    // camera's right in the first rows.
    let mut lamps = Vec::new();
    for (row, &(d, rise)) in crate::scene::hall::ROWS.iter().enumerate() {
        let ri = row as i32;
        let gap = 105.0 + 10.0 * ri as f32;
        let mut r = -950.0 + 40.0 * h(ri, 0, 23);
        let mut j = 0i32;
        while r < 950.0 {
            let jr = r + (h(ri, j, 29) - 0.5) * 30.0;
            // None in the subject's own place on the first row (the soloist stands before
            // them).
            if row > 0 || jr.abs() > 70.0 {
                let cello = row < 2 && jr > 250.0;
                player(
                    g,
                    &f,
                    &mats,
                    jr,
                    d + 20.0 * h(ri, j, 31),
                    rise,
                    (ri, j),
                    cello,
                );
                if j % 3 == 0 {
                    stand(g, &f, &mats, jr + gap * 0.5, d - 70.0, rise);
                }
            }
            r += gap;
            j += 1;
        }
        // A wash from in front of the row: the players' faces, instruments and shoulders
        // warm out of the dark (the stage's general light, short of the soloist).
        let mut lx = -900.0;
        while lx < 950.0 {
            lamps.push(Lamp {
                p: f.at(lx, rise + 250.0, d - 110.0),
                col: warm([1.0, 0.82, 0.60], 4.0 * k * c.rows),
                reach: 2.6,
            });
            lx += 260.0;
        }
    }
    // The stage's floor between the soloist and the orchestra in warm pools, off to the
    // sides (none straight behind the figure): the picture's lower half a lit floor, not a
    // void (the coordinator on the first sheet: the dark clothes melted into it).
    for x in [-760.0f32, -430.0, 430.0, 760.0] {
        lamps.push(Lamp {
            p: f.at(x, 130.0, 170.0),
            col: warm([1.0, 0.80, 0.56], 2.5 * k * c.rows),
            reach: 2.2,
        });
    }
    // The wall's pools of light, low behind the players, and the hall's lights high up
    // (its bokeh).
    let mut wx = -1400.0;
    while wx < 1450.0 {
        lamps.push(Lamp {
            p: f.at(wx, 300.0, BACK - 80.0),
            col: warm([1.0, 0.75, 0.48], 2.0 * k),
            reach: 4.5,
        });
        wx += 380.0;
    }
    let glow: Vec<MatId> = [1.5f32, 2.2, 3.0]
        .iter()
        .map(|&e| g.mat(PMat::emit(warm([1.0, 0.72, 0.42], e * c.bokeh))))
        .collect();
    for q in 0..14 {
        let (u, w) = (h(q, 41, 43), h(q, 47, 53));
        let at = f.at(-1500.0 + 3000.0 * u, 450.0 + 400.0 * w, BACK - 15.0);
        let r = 0.025 + 0.02 * h(q, 59, 61);
        let m = glow[((h(q, 67, 71) * 3.0) as usize).min(2)];
        g.lathe(at, n_w, &[(0.0, 0.0), (r, 0.0)], 16, m);
    }
    let fp = floor_paint(Floor::StageWalnut);
    let wp = walls_paint(Walls::HallWood);
    let corners = [f.at(w0, 0.0, -300.0), f.at(w1, 0.0, BACK)];
    Built {
        env: RoomEnv {
            kind: Room::Stage,
            x0: corners[0].x.min(corners[1].x),
            x1: corners[0].x.max(corners[1].x),
            z0: corners[0].z.min(corners[1].z),
            z1: corners[0].z.max(corners[1].z),
            h: 11.0,
            floor: fp,
            walls: wp,
            rug: None,
            gloss: 0.0,
        },
        lamps,
        panels: Vec::<Panel>::new(),
    }
}
