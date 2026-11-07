//! The scene's materials and light, per pixel (ADR 0104): procedural textures computed from
//! the surface's position and texture coordinates (wood with its figure and pores, the
//! floor's boards, the rug's weave and border, fabric, the frame's cast gold, the grille's
//! mesh), a key spot with its shadow, a sky-and-floor fill darkened by ambient occlusion, a
//! rim light, and what a gloss reflects — traced against the room itself (the floor, the
//! walls, a softbox, the frame under a grand's lid), so the lacquer shows the room it is in.

use super::geo::M;
use super::mesh::{lin, lmix, lscale, v3, Camera, Lin, V3};
use super::piano::Piano;
use super::room::Room;
use crate::kit::skin::scene::{self as pal, Paint};

// ---- noise --------------------------------------------------------------------------------

#[inline]
fn hash3(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ s.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// Value noise in 0…1, smooth.
pub fn noise(x: f32, y: f32, s: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let (tx, ty) = (x - fx, y - fy);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = hash3(ix, iy, s);
    let b = hash3(ix + 1, iy, s);
    let c = hash3(ix, iy + 1, s);
    let d = hash3(ix + 1, iy + 1, s);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sy
}

/// Fractal noise, `oct` octaves, in 0…1.
pub fn fbm(x: f32, y: f32, s: u32, oct: u32) -> f32 {
    let (mut sum, mut amp, mut f, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..oct {
        sum += amp * noise(x * f, y * f, s.wrapping_add(o * 977));
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Wood's figure at `u` (along the grain) and `v` (across), metres: 0 the early wood's pale
/// … 1 the late wood's dark, with its cathedral figure and, fine enough, its pores. `fp`:
/// metres a pixel covers (finer detail fades out instead of aliasing).
pub fn wood(u: f32, v: f32, seed: u32, fp: f32) -> f32 {
    // A board's figure: broad bands bent by a slow warp (the cathedral arches of flat-sawn
    // veneer), fine streaks along the grain, the rings soft-edged.
    let warp = fbm(u * 0.6, v * 3.0, seed, 3) * 2.0 + fbm(u * 0.12, v * 0.9, seed + 7, 2) * 3.5;
    let rings = v * 13.0 + warp * 2.2 + (u * 1.7 + v * 2.0).sin() * 0.35;
    let f = rings - rings.floor();
    let ring = smooth(0.45, 0.85, f) * (1.0 - smooth(0.88, 1.0, f));
    let streak = fbm(u * 1.5, v * 90.0, seed + 3, 3);
    let mut t = 0.45 * ring + 0.55 * streak;
    let pore_w = (0.0012 / fp.max(1e-5)).min(1.0);
    if pore_w > 0.2 {
        let pores = noise(u * 260.0, v * 1400.0, seed + 11);
        t += 0.25 * pore_w * smooth(0.7, 0.95, pores);
    }
    t.clamp(0.0, 1.0)
}

// ---- the light --------------------------------------------------------------------------

/// How the scene is lit, in linear light (from the theme's `pal::Light`).
pub struct Stage {
    pub cam: Camera,
    pub key_pos: V3,
    pub key_aim: V3,
    pub key: Lin,
    pub cone: (f32, f32),
    pub sky: Lin,
    pub ground: Lin,
    pub rim_dir: V3,
    pub rim: Lin,
    /// The softbox: its centre, its half sides and its radiance.
    pub soft: (V3, V3, V3, Lin),
    pub haze: Lin,
    pub haze_k: f32,
    pub glow: Lin,
    pub piano: Piano,
    pub room: Room,
    pub finish: Paint,
    /// The rug's rect in the plan, when there is one.
    pub rug: Option<((f32, f32), (f32, f32))>,
}

/// A surface's response.
#[derive(Copy, Clone, Debug)]
pub struct Surf {
    pub base: Lin,
    pub coat: f32,
    pub spec: f32,
    pub shine: f32,
    pub metal: bool,
    pub glow: Option<Lin>,
}

impl Surf {
    fn matte(base: Lin) -> Surf {
        Surf {
            base,
            coat: 0.0,
            spec: 0.08,
            shine: 10.0,
            metal: false,
            glow: None,
        }
    }
    fn coated(base: Lin, coat: f32, shine: f32) -> Surf {
        Surf {
            base,
            coat,
            spec: 1.0,
            shine,
            metal: false,
            glow: None,
        }
    }
    fn metal(base: Lin, shine: f32) -> Surf {
        Surf {
            base,
            coat: 1.0,
            spec: 1.0,
            shine,
            metal: true,
            glow: None,
        }
    }
}

/// The plate's height under a grand's lid.
pub const PLATE_Y: f32 = 0.87;

/// The glows' colours (a lamp, a window, the console's LEDs, a screen).
pub fn glow_of(k: u8, stage: &Stage) -> Lin {
    match k {
        0 => lscale(stage.glow, 6.0),
        1 => lscale(lmix(stage.glow, pal::gear::SKYLIGHT, 0.6), 3.5),
        2..=5 => lscale(lin(pal::gear::LEDS[(k - 2) as usize]), 3.0),
        6 => lscale(lin(pal::gear::SCREEN), 0.35),
        _ => lscale(lin(pal::gear::TALLY), 4.0),
    }
}

impl Stage {
    /// The surface of `mat` at `p` (uv, normal `n`), `fp` metres a pixel.
    pub fn surf(&self, mat: M, k: u8, p: V3, uv: [f32; 2], fp: f32) -> Surf {
        use pal::part;
        let f = &self.finish;
        match mat {
            M::Case | M::Inner => {
                let mut base = lin(f.base);
                if f.wood {
                    let t = wood(uv[0], uv[1], 17, fp);
                    base = lmix(lin(f.grain_light), lin(f.grain_dark), t);
                    base = lmix(base, lin(f.base), 0.25);
                }
                if mat == M::Inner {
                    base = lscale(base, 0.75);
                }
                // The desk's fretwork: a lattice of dark piercings over its panel.
                if k == 1 {
                    let (fu, fv) = (uv[0] * 130.0, uv[1] * 130.0);
                    let lat = ((fu.sin() * fv.sin()).abs()
                        + 0.35 * (fu * 0.5 + fv * 0.5).sin().abs())
                    .min(1.0);
                    let cut = if fp < 0.0012 {
                        smooth(0.62, 0.7, lat)
                    } else {
                        0.25
                    };
                    base = lscale(base, 1.0 - 0.85 * cut);
                }
                // The lid's underside: a deep gloss facing the floor, which it mirrors only
                // dimly (a lacquered board, not smoked glass).
                let coat = if k == 2 { f.coat * 0.22 } else { f.coat * 0.6 };
                Surf {
                    base,
                    coat,
                    spec: f.spec,
                    shine: f.shine * 4.0,
                    metal: false,
                    glow: None,
                }
            }
            M::Ivory => {
                let n = 0.94 + 0.06 * fbm(uv[0] * 40.0, uv[1] * 40.0, 5, 2);
                Surf::coated(lscale(lin(part::IVORY), n), 0.35, 60.0)
            }
            M::Ebony => Surf::coated(lin(part::EBONY), 0.6, 120.0),
            M::Gold => {
                // Bronze paint on cast iron: a warm, satin metallic finish.
                let cast = 0.85 + 0.15 * fbm(p.x * 30.0, p.z * 30.0, 9, 3);
                let mut s = Surf::coated(lscale(lin(part::GOLD), cast * 1.15), 0.2, 30.0);
                s.spec = 0.8;
                s
            }
            M::Spruce => {
                let t = wood(uv[0] * 0.7 + uv[1] * 0.7, uv[1] * 0.7 - uv[0] * 0.7, 23, fp);
                Surf::coated(
                    lmix(lin(part::SPRUCE), lin(part::SPRUCE_GRAIN), t * 0.7),
                    0.15,
                    20.0,
                )
            }
            M::Felt => Surf::matte(lscale(
                lin(part::DAMPER),
                0.9 + 0.2 * noise(p.x * 400.0, p.z * 400.0, 3),
            )),
            M::HammerFelt => Surf::matte(lin(part::HAMMER)),
            M::RedFelt => Surf::matte(lin(part::FELT_RED)),
            M::Steel => Surf::metal(lin(part::STEEL), 80.0),
            M::Copper => Surf::metal(lin(part::COPPER), 40.0),
            M::Nickel => Surf::metal(lin(part::NICKEL), 120.0),
            M::Brass => Surf::metal(lin(part::BRASS), 60.0),
            M::Chrome => Surf::metal(lin(pal::gear::CHROME), 200.0),
            M::Stand => Surf::coated(lin(pal::gear::STAND), 0.25, 30.0),
            M::Rubber => Surf::coated(lin(pal::gear::CABLE), 0.12, 15.0),
            M::MicBody => {
                // Brushed: fine streaks along the body.
                let b = 0.9 + 0.1 * noise(uv[0] * 400.0, uv[1] * 8.0, 31);
                Surf::metal(lscale(lin(pal::gear::MIC_BODY), b), 50.0)
            }
            M::Grille => {
                let m = ((uv[0] * 900.0).sin() * (uv[1] * 900.0).sin()).abs();
                let open = if fp < 0.0015 {
                    smooth(0.3, 0.7, m)
                } else {
                    0.5
                };
                Surf::metal(lscale(lin(pal::gear::GRILLE), 0.55 + 0.45 * open), 30.0)
            }
            M::MicDark => Surf::coated(lin(pal::gear::MIC_DARK), 0.4, 60.0),
            M::Floor => self.floor(p, fp),
            M::Rug => self.rug(p, fp),
            M::Wall | M::Wall2 => {
                let base = if mat == M::Wall {
                    self.room.paint.wall
                } else {
                    self.room.paint.wall2
                };
                let plaster = 0.93 + 0.07 * fbm(uv[0] * 3.0, uv[1] * 3.0, 41, 3);
                let mut b = lscale(lin(base), plaster);
                if matches!(self.room.kind, super::room::Kind::Church) {
                    // Limestone courses: blocks, their joints, each block its own tone.
                    let (bu, bv) = (uv[0] / 0.6, uv[1] / 0.45);
                    let row = bv.floor();
                    let cu = bu + if (row as i32) % 2 == 0 { 0.0 } else { 0.5 };
                    let tone = 0.88 + 0.18 * hash3(cu.floor() as i32, row as i32, 77);
                    let jw = (0.012f32 / 0.45).max(fp / 0.45);
                    let ju =
                        smooth(0.0, jw, cu - cu.floor()) * smooth(0.0, jw, 1.0 - (cu - cu.floor()));
                    let jv = smooth(0.0, jw, bv - row) * smooth(0.0, jw, 1.0 - (bv - row));
                    b = lscale(b, tone * (0.65 + 0.35 * ju * jv));
                }
                Surf::matte(b)
            }
            M::Fabric | M::Fabric2 => {
                let base = if mat == M::Fabric {
                    self.room.paint.fabric
                } else {
                    self.room.paint.fabric2
                };
                let weave = 0.9
                    + 0.1 * noise(uv[0] * 300.0, uv[1] * 300.0, 51)
                    + 0.05 * fbm(uv[0] * 8.0, uv[1] * 8.0, 52, 2);
                Surf::matte(lscale(lin(base), weave))
            }
            M::WoodPanel => {
                let t = wood(uv[1], uv[0], 61 + k as u32, fp);
                let base = lmix(
                    lin(self.room.paint.wood),
                    lscale(lin(self.room.paint.wood), 0.6),
                    t * 0.8,
                );
                Surf::coated(base, 0.2, 25.0)
            }
            M::Glass => {
                let mut s = Surf::coated(lin(pal::gear::GLASS), 1.0, 300.0);
                s.spec = 1.5;
                s
            }
            M::Glow => Surf {
                glow: Some(glow_of(k, self)),
                ..Surf::matte([0.0; 3])
            },
        }
    }

    fn floor(&self, p: V3, fp: f32) -> Surf {
        let paint = &self.room.paint;
        let church = matches!(self.room.kind, super::room::Kind::Church);
        if church {
            // Flagstones.
            let (su, sv) = (p.x / 0.9, p.z / 0.9);
            let (iu, iv) = (su.floor(), sv.floor());
            let tone = 0.82 + 0.25 * hash3(iu as i32, iv as i32, 91);
            let jw = (0.01f32 / 0.9).max(fp / 0.9);
            let fu = su - iu;
            let fv = sv - iv;
            let j = smooth(0.0, jw, fu)
                * smooth(0.0, jw, 1.0 - fu)
                * smooth(0.0, jw, fv)
                * smooth(0.0, jw, 1.0 - fv);
            let wear = 0.9 + 0.1 * fbm(p.x * 2.0, p.z * 2.0, 92, 3);
            let b = lscale(lmix(lin(paint.joint), lin(paint.floor), j), tone * wear);
            return Surf::coated(b, paint.floor_gloss * (0.4 + 0.6 * self.room.live), 80.0);
        }
        let pw = 0.16;
        let plank = (p.x / pw).floor();
        let across = p.x / pw - plank;
        let offset = hash3(plank as i32, 0, 93) * 2.4;
        let len = 1.8 + hash3(plank as i32, 1, 93) * 0.8;
        let along = (p.z + offset) / len;
        let piece = along.floor();
        let fz = along - piece;
        let id = (plank as i32).wrapping_mul(131).wrapping_add(piece as i32);
        let tone = 0.82 + 0.3 * hash3(id, 2, 94);
        let t = wood(
            p.z * 1.0 + piece * 7.3,
            across * pw + plank * 0.37,
            95 + (id as u32 & 7),
            fp,
        );
        let mut b = lmix(lin(paint.floor), lin(paint.floor2), t * 0.9);
        b = lscale(b, tone);
        // The joints: along the boards and at their ends.
        let jw = (0.003 / pw).max(0.8 * fp / pw);
        let side = smooth(0.0, jw, across) * smooth(0.0, jw, 1.0 - across);
        let jl = (0.003 / len).max(0.8 * fp / len);
        let end = smooth(0.0, jl, fz) * smooth(0.0, jl, 1.0 - fz);
        b = lmix(lin(paint.joint), b, side * end);
        Surf::coated(b, paint.floor_gloss * (0.4 + 0.6 * self.room.live), 60.0)
    }

    fn rug(&self, p: V3, fp: f32) -> Surf {
        let Some(((x0, z0), (x1, z1))) = self.rug else {
            return Surf::matte(lin(pal::RUG_FIELD));
        };
        let d = (p.x - x0).min(x1 - p.x).min(p.z - z0).min(z1 - p.z);
        let field = lin(pal::RUG_FIELD);
        let border = lin(pal::RUG_BORDER);
        let line = lin(pal::RUG_LINE);
        let aa = fp.max(0.004);
        let mut b = border;
        // The border's own pattern: a running chain of lozenges.
        let along = if (p.x - x0).min(x1 - p.x) < (p.z - z0).min(z1 - p.z) {
            p.z
        } else {
            p.x
        };
        let chain = ((along / 0.07) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        if d < 0.3 {
            let band = smooth(0.06, 0.06 + aa, d) * (1.0 - smooth(0.24, 0.24 + aa, d));
            let lozenge = smooth(
                0.55,
                0.6,
                chain * (1.0 - ((d - 0.15).abs() / 0.09).min(1.0)),
            );
            b = lmix(b, line, 0.14 * band * lozenge);
        }
        let l1 = smooth(0.3, 0.3 + aa, d) * (1.0 - smooth(0.33, 0.33 + aa, d));
        b = lmix(b, line, l1);
        if d > 0.33 {
            // The field: an all-over pattern of small lozenges in a lattice (a Persian
            // field's repeat, quiet at this distance) with a soft abrash — the wool's
            // dye lots, in bands across the warp.
            let (lu, lv) = ((p.x - x0) / 0.16, (p.z - z0) / 0.16);
            let (fu, fv) = (lu - lu.floor() - 0.5, lv - lv.floor() - 0.5);
            let loz = fu.abs() + fv.abs();
            let ring = smooth(0.30, 0.36, loz) * (1.0 - smooth(0.40, 0.46, loz));
            let detail = if fp < 0.01 { 1.0 } else { 0.3 };
            let mut c = lmix(field, border, 0.35 * ring * detail);
            c = lmix(c, line, 0.25 * (1.0 - smooth(0.0, 0.08, loz)) * detail);
            let abrash = 0.93 + 0.07 * fbm(p.x * 0.6, p.z * 3.0, 101, 2);
            c = lscale(c, abrash);
            b = c;
        }
        let weave = 0.88 + 0.12 * noise(p.x * 500.0, p.z * 500.0, 103);
        Surf::matte(lscale(b, weave))
    }

    /// The key spot's light reaching `p` (before its shadow): its colour, and the unit
    /// vector toward it.
    pub fn key_at(&self, p: V3) -> (Lin, V3) {
        let d = self.key_pos - p;
        let d2 = d.dot(d).max(0.25);
        let l = d * (1.0 / d2.sqrt());
        let c = (-l).dot(self.key_aim);
        let spot = smooth(self.cone.0, self.cone.1, c);
        let fall = spot * 16.0 / d2;
        (lscale(self.key, fall), l)
    }

    /// The fill from the room: sky above, the floor's bounce below.
    pub fn fill(&self, n: V3) -> Lin {
        lmix(self.ground, self.sky, n.y * 0.5 + 0.5)
    }

    /// What a gloss at `p` reflects in direction `r` (unit), traced against the room.
    pub fn env(&self, p: V3, r: V3) -> Lin {
        let mut best = f32::MAX;
        let mut c = lmix(lscale(self.sky, 0.6), self.sky, r.y.max(0.0));
        // The softbox: a bright rectangle.
        let (sc, su, sv, rad) = self.soft;
        let sn = su.cross(sv).norm();
        let den = r.dot(sn);
        if den.abs() > 1e-4 {
            let t = (sc - p).dot(sn) / den;
            if t > 0.0 {
                let h = p + r * t - sc;
                let (a, b) = (h.dot(su) / su.dot(su), h.dot(sv) / sv.dot(sv));
                if a.abs() < 1.0 && b.abs() < 1.0 {
                    let edge = (1.0 - a.abs()).min(1.0 - b.abs());
                    best = t;
                    c = lscale(rad, smooth(0.0, 0.15, edge));
                }
            }
        }
        // The room's floor and walls.
        let room = &self.room;
        let planes: [(f32, f32, f32, M); 3] = [
            (p.y, r.y, 0.0, M::Floor),
            (p.z, r.z, room.z0, M::Wall),
            (p.x, r.x, room.x0, M::Wall2),
        ];
        for (o, d, plane, m) in planes {
            if d < -1e-4 {
                let t = (plane - o) / d;
                if t > 1e-3 && t < best {
                    let h = p + r * t;
                    if m == M::Floor && (h.x < room.x0 || h.z < room.z0) {
                        continue;
                    }
                    best = t;
                    let s = if m == M::Floor {
                        match self.rug {
                            Some(((x0, z0), (x1, z1)))
                                if h.x > x0 && h.x < x1 && h.z > z0 && h.z < z1 =>
                            {
                                Surf::matte(lin(pal::RUG_FIELD))
                            }
                            _ => Surf::matte(lin(room.paint.floor)),
                        }
                    } else {
                        Surf::matte(lin(if m == M::Wall {
                            room.paint.wall
                        } else {
                            room.paint.wall2
                        }))
                    };
                    let (kc, _) = self.key_at(h);
                    let n = if m == M::Floor {
                        v3(0.0, 1.0, 0.0)
                    } else {
                        v3(0.0, 0.0, 1.0)
                    };
                    let lit = super::mesh::ladd(self.fill(n), lscale(kc, 0.5));
                    // A diffuse room seen in a gloss is dimmer than the gloss's own lights.
                    c = lscale(
                        [s.base[0] * lit[0], s.base[1] * lit[1], s.base[2] * lit[2]],
                        0.5,
                    );
                }
            }
        }
        c
    }

    /// The haze between the camera and `p` (big rooms), and its colour.
    pub fn hazed(&self, p: V3, c: Lin) -> Lin {
        let d = (p - self.cam.eye).len();
        let t = 1.0 - (-(d - 6.0).max(0.0) * self.haze_k).exp();
        lmix(c, self.haze, t.min(0.8))
    }
}

/// Schlick's Fresnel for a clear coat (index 1.5).
#[inline]
pub fn fresnel(ndv: f32) -> f32 {
    0.04 + 0.96 * (1.0 - ndv).clamp(0.0, 1.0).powi(5)
}
