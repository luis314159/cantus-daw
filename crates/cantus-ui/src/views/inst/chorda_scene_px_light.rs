//! r40:scene-px — the light, designed as a product photographer lights a black grand: a
//! key softbox high over the treble front (its spot the stage's pool, its shadow soft —
//! the only light that casts one), long strip lights and a broad overhead diffuser out of
//! frame that black lacquer, chrome and bronze show as long crisp highlights, a warm
//! reflector board that the open lid's underside and the case pick up, a cool rim from
//! behind that separates black from black, the room's ambient (a dark theme's evening, a
//! light one's daylight) and its lamps. `env` is what a gloss sees in a direction: the
//! panels, the room's floor, walls and ceiling, and a grand's frame under its lid.

use super::mat::{lin3, smooth};
use super::mesh::{ladd, lmix, lscale, v3, Lin, V3};
use crate::kit::skin::scene::{self as pal, FloorPaint, RugPaint, WallPaint};

/// A rectangular light (or a bright board) out of frame: seen in reflections, and lighting
/// diffusely by `diffuse`.
#[derive(Copy, Clone, Debug)]
pub struct Panel {
    pub c: V3,
    /// Half sides.
    pub u: V3,
    pub v: V3,
    /// The side it shines from.
    pub n: V3,
    pub rad: Lin,
    pub diffuse: f32,
    /// How far in from its edge it reaches full brightness (0…1 of its half side): a
    /// strip's crisp edge, a big softbox's falloff toward its rim.
    pub soft: f32,
    /// Its face's brightness at its rim, of its middle's (1 an even face).
    pub core: f32,
    /// Whether a gloss sees it (a wash light above, out of every reflection, does not).
    pub seen: bool,
}

impl Panel {
    /// A panel at `c`, `w` × `h` metres, its face toward `at`, `up` its long side's world
    /// direction (a strip's length).
    pub fn toward(c: V3, at: V3, w: f32, h: f32, up: V3, rad: Lin, diffuse: f32) -> Panel {
        let n = (at - c).norm();
        let mut v = (up - n * up.dot(n)).norm();
        if v.len() < 0.5 {
            v = super::geo::perp(n);
        }
        let u = n.cross(v).norm();
        Panel {
            c,
            u: u * (w * 0.5),
            v: v * (h * 0.5),
            n,
            rad,
            diffuse,
            soft: 0.08,
            core: 1.0,
            seen: true,
        }
    }

    /// The same panel with its face falling to `core` of its middle's brightness at its rim.
    pub fn core(self, core: f32) -> Panel {
        Panel { core, ..self }
    }

    /// The same panel lighting only diffusely, never seen in a reflection.
    pub fn unseen(self) -> Panel {
        Panel {
            seen: false,
            ..self
        }
    }

    /// The same panel with a softer falloff from its middle to its edge.
    pub fn soft(self, soft: f32) -> Panel {
        Panel { soft, ..self }
    }

    /// Where the ray from `p` along `r` meets it: (distance, how far inside its edge 0…1).
    fn hit(&self, p: V3, r: V3) -> Option<(f32, f32)> {
        let den = r.dot(self.n);
        if den > -1e-4 {
            return None;
        }
        let t = (self.c - p).dot(self.n) / den;
        if t <= 1e-3 {
            return None;
        }
        let h = p + r * t - self.c;
        let (a, b) = (
            h.dot(self.u) / self.u.dot(self.u),
            h.dot(self.v) / self.v.dot(self.v),
        );
        if a.abs() >= 1.0 || b.abs() >= 1.0 {
            return None;
        }
        let edge = (1.0 - a.abs()).min((1.0 - b.abs()) * 0.6 + 0.4 * (1.0 - b.abs()).sqrt());
        // A softbox's face is brightest at its middle: `core` of that at its rim.
        let face = self.core + (1.0 - self.core) * ((1.0 - a * a) * (1.0 - b * b)).sqrt();
        Some((t, smooth(0.0, self.soft, edge) * face))
    }

    /// Its light falling on a surface at `p` facing `n` (no shadow).
    fn irradiance(&self, p: V3, n: V3) -> Lin {
        if self.diffuse <= 0.0 {
            return [0.0; 3];
        }
        let d = self.c - p;
        let d2 = d.dot(d).max(1e-4);
        let l = d * (1.0 / d2.sqrt());
        let (cs, cp) = (n.dot(l), -self.n.dot(l));
        if cs <= 0.0 || cp <= 0.0 {
            return [0.0; 3];
        }
        let area = 4.0 * self.u.len() * self.v.len();
        lscale(
            self.rad,
            self.diffuse * area * cs * cp / (d2 + area * 0.25) / std::f32::consts::PI,
        )
    }
}

/// The key: a softbox spot. Its irradiance at the focus is `e0`, falling off with the
/// square of the distance and the cone (cos outer, cos inner); `size` its half side (the
/// shadows' penumbra).
#[derive(Copy, Clone, Debug)]
pub struct Key {
    pub pos: V3,
    pub aim: V3,
    pub focus: V3,
    pub cone: (f32, f32),
    pub e0: Lin,
    pub size: f32,
}

impl Key {
    /// Its light reaching `p` (before the shadow) and the unit vector toward it.
    pub fn at(&self, p: V3) -> (Lin, V3) {
        let d = self.pos - p;
        let d2 = d.dot(d).max(0.04);
        let l = d * (1.0 / d2.sqrt());
        let c = (-l).dot(self.aim);
        let cone = smooth(self.cone.0, self.cone.1, c);
        let f = (self.focus - self.pos).dot(self.focus - self.pos);
        (lscale(self.e0, cone * f / d2), l)
    }
}

/// The room as the environment sees it: its floor, two walls meeting behind the
/// instrument, its ceiling, their paints; a rug's rect.
#[derive(Copy, Clone, Debug)]
pub struct RoomEnv {
    pub kind: crate::scene::Room,
    /// The side wall's x, the right edge, the back wall's z, the front edge, the height (m).
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub h: f32,
    pub floor: FloorPaint,
    pub walls: WallPaint,
    pub rug: Option<(RugPaint, [f32; 4])>,
    /// How strongly the floor mirrors (0 … 1).
    pub gloss: f32,
}

/// A grand's frame under its open lid, which the lid's underside mirrors: its plane's
/// height and its outline (x, z, metres).
#[derive(Clone, Debug)]
pub struct Plate {
    pub y: f32,
    pub outline: Vec<(f32, f32)>,
}

/// A lamp in the room: where, its colour (irradiance at 1 m), how far it reaches.
#[derive(Copy, Clone, Debug)]
pub struct Lamp {
    pub p: V3,
    pub col: Lin,
    pub reach: f32,
}

/// Everything that lights the scene.
#[derive(Clone, Debug)]
pub struct Rig {
    pub key: Key,
    /// The key's own face (index into `panels`): seen in a gloss only where the key reaches.
    pub key_panel: Option<usize>,
    pub panels: Vec<Panel>,
    pub sky: Lin,
    pub ground: Lin,
    /// What a gloss sees where nothing of the room is: the dark above a stage, a room's
    /// ceiling in shadow.
    pub void: Lin,
    /// Directional rims: direction toward the light, colour.
    pub rims: Vec<(V3, Lin)>,
    pub lamps: Vec<Lamp>,
    pub room: RoomEnv,
    pub plate: Option<Plate>,
    pub haze: Lin,
    pub haze_k: f32,
    pub evening: bool,
    /// Exposure before the tone curve.
    pub exposure: f32,
    /// In focus up to this distance from the eye (m); softer beyond.
    pub focus_far: f32,
    pub eye: V3,
    /// r42:arcus-upper — the room behind softer (a realistic figure's picture reads as a
    /// photograph: a portrait lens's shallow focus); 0 for every other picture.
    pub soft_bg: f32,
}

pub fn inside(o: &[(f32, f32)], x: f32, z: f32) -> bool {
    let mut c = false;
    let n = o.len();
    for i in 0..n {
        let (a, b) = (o[i], o[(i + 1) % n]);
        if (a.1 > z) != (b.1 > z) && x < (b.0 - a.0) * (z - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

impl Rig {
    /// The ambient light on a surface facing `n`: the sky above, the floor's bounce below.
    pub fn ambient(&self, n: V3) -> Lin {
        lmix(self.ground, self.sky, n.y * 0.5 + 0.5)
    }

    /// Every unshadowed light on a surface at `p` facing `n`: the panels, the lamps.
    pub fn fill(&self, p: V3, n: V3) -> Lin {
        let mut e = [0.0f32; 3];
        for pn in &self.panels {
            e = ladd(e, pn.irradiance(p, n));
        }
        for l in &self.lamps {
            let d = l.p - p;
            let d2 = d.dot(d).max(0.01);
            let cs = n.dot(d * (1.0 / d2.sqrt())).max(0.0);
            let k = cs / d2 * (-(d2.sqrt() / l.reach).powi(2)).exp();
            e = ladd(e, lscale(l.col, k));
        }
        e
    }

    /// The light the room's own surfaces send back, for what a gloss sees of them: the key
    /// where it falls (its pool on the floor), the fill, the ambient.
    fn lit(&self, albedo: Lin, p: V3, n: V3) -> Lin {
        let (k, l) = self.key.at(p);
        let kd = n.dot(l).max(0.0);
        let e = ladd(ladd(lscale(k, kd), self.fill(p, n)), self.ambient(n));
        [albedo[0] * e[0], albedo[1] * e[1], albedo[2] * e[2]]
    }

    /// What a gloss at `p` sees along `r` (unit): `key_vis` how much of the key reaches `p`;
    /// `panels` false for the floor (the lights are flagged off it, as a photographer flags them)
    /// (its face is not seen in a shadowed gloss).
    pub fn env(&self, p: V3, r: V3, key_vis: f32, panels: bool) -> Lin {
        let mut best = f32::MAX;
        // The dark above and around: a little lighter toward the horizon.
        let mut c = lmix(self.void, lscale(self.void, 0.5), r.y.clamp(0.0, 1.0));
        for (i, pn) in self
            .panels
            .iter()
            .enumerate()
            .filter(|(_, pn)| panels && pn.seen)
        {
            if let Some((t, edge)) = pn.hit(p, r) {
                if t < best {
                    best = t;
                    let k = if Some(i) == self.key_panel {
                        key_vis
                    } else {
                        1.0
                    };
                    c = lscale(pn.rad, edge * k);
                }
            }
        }
        let rm = &self.room;
        // The floor (and its rug).
        if r.y < -1e-4 {
            let t = -p.y / r.y;
            if t > 1e-3 && t < best {
                let h = p + r * t;
                if h.x > rm.x0 && h.z > rm.z0 && h.z < rm.z1 + 6.0 {
                    best = t;
                    let alb = match rm.rug {
                        Some((rg, [x0, z0, x1, z1]))
                            if h.x > x0 && h.x < x1 && h.z > z0 && h.z < z1 =>
                        {
                            lin3(rg.field)
                        }
                        _ => lin3(rm.floor.a),
                    };
                    c = self.lit(alb, h, v3(0.0, 1.0, 0.0));
                }
            }
        }
        // The back wall and the side wall, the ceiling.
        let walls = [(p.z, r.z, rm.z0, false), (p.x, r.x, rm.x0, true)];
        for (o, d, plane, side) in walls {
            if d < -1e-4 {
                let t = (plane - o) / d;
                if t > 1e-3 && t < best {
                    let h = p + r * t;
                    if h.y < rm.h && h.y > 0.0 {
                        best = t;
                        let n = if side {
                            v3(1.0, 0.0, 0.0)
                        } else {
                            v3(0.0, 0.0, 1.0)
                        };
                        let alb = if side {
                            lin3(rm.walls.wall2)
                        } else {
                            lin3(rm.walls.wall)
                        };
                        c = self.lit(alb, h, n);
                    }
                }
            }
        }
        if r.y > 1e-4 {
            let t = (rm.h - p.y) / r.y;
            if t > 1e-3 && t < best {
                c = lscale(self.ambient(v3(0.0, -1.0, 0.0)), 0.25);
            }
        }
        c
    }

    /// The frame a grand's lid mirrors along `r` from `p`, when the ray meets it before
    /// anything else: its colour.
    pub fn plate_seen(&self, p: V3, r: V3) -> Option<Lin> {
        let pl = self.plate.as_ref()?;
        if r.y >= -1e-4 {
            return None;
        }
        let t = (pl.y - p.y) / r.y;
        if t <= 1e-3 {
            return None;
        }
        let h = p + r * t;
        if !inside(&pl.outline, h.x, h.z) {
            return None;
        }
        // Bronze-painted iron under the strings: the strings' sheen as fine stripes.
        let gold = lin3(pal::srgb(184, 146, 78));
        let strings = (std::f32::consts::PI * h.x / 0.0135).cos().abs().powi(6);
        let s = (0.45 + 0.9 * strings * 0.3) * 0.6;
        let e = self.lit(gold, h, v3(0.0, 1.0, 0.0));
        Some(lscale(e, s))
    }

    /// The haze between the eye and `p`.
    pub fn hazed(&self, p: V3, c: Lin) -> Lin {
        let d = (p - self.eye).len();
        let t = 1.0 - (-(d - 6.0).max(0.0) * self.haze_k).exp();
        lmix(c, self.haze, t.min(0.8))
    }
}
