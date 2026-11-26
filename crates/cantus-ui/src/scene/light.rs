//! The light and the materials (port of `scene/light.py` and `scene/materials.py`): a key
//! light — a soft box above and in front of the instrument whose spot falls off round its
//! focus — an ambient fill, a floor bounce, and the environment every polished surface
//! reflects (floor, walls, ceiling, the soft box, two tall strip lights out of frame). One
//! `shade` turns a material record, a normal, the eye and the light into a colour.

use super::geo::{v, V3};
use crate::kit::skin::scene::{self as pal, Mat, MatKind, C3};

/// A mood placed in a scene: its data, the soft box's place, where its spot is aimed.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Light {
    pub m: pal::Mood,
    pub evening: bool,
    pub key_pos: V3,
    pub focus: V3,
    /// The floor's colour, as a gloss sees the pool of light on it.
    pub floor: C3,
    /// Leave the strip lights' crisp streaks out of a lacquer's shade: a broad flat face
    /// shaded once would light whole; it draws them as a band across itself instead
    /// ([`Light::streak`], `prims::facet_v`).
    pub no_streak: bool,
    /// Leave the photographer's fill out (the floor: it lit a stage's floor edge to edge,
    /// where the photograph's is a pool of light falling off to the dark).
    pub no_fill: bool,
    /// r42:arcus-polish3 — the concert stage's own graphic light (a realistic figure on the
    /// stage, `figure::concert`): a spot with a clean terminator, the hall's dark, one clean
    /// highlight on a varnish, the back lights' rim (`Light::rim`).
    pub concert: Option<crate::scene::figure::concert::Concert>,
    /// The concert's back lights (ways toward them, strengths) and its subject's middle.
    pub backs: [(V3, f32); 2],
}

pub fn mul(a: C3, b: C3) -> C3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

pub fn scale(c: C3, k: f32) -> C3 {
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn add(a: C3, b: C3) -> C3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn lerp(a: C3, b: C3, t: f32) -> C3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn maxc(c: C3) -> f32 {
    c[0].max(c[1]).max(c[2])
}

/// A light case finish (white, maple, a pale wood): bright, and a wood or near neutral —
/// not a gilt or a coloured trim, which keep the ordinary lacquer's look.
pub fn light_finish(m: &Mat) -> bool {
    let (hi, lo) = (
        maxc(m.albedo),
        m.albedo[0].min(m.albedo[1]).min(m.albedo[2]),
    );
    hi > 0.45 && (m.wood || lo > 0.75 * hi)
}

/// `i` reflected about `n`.
pub fn reflect(i: V3, n: V3) -> V3 {
    i - n * (2.0 * i.dot(n))
}

/// The two strip lights' directions (constant).
fn strips() -> (V3, V3) {
    (v(-1.0, -0.12, 0.62).norm(), v(1.0, -0.05, 0.40).norm())
}

impl Light {
    /// The mood of a dark theme (`evening`) or a light one, on the stage or in a room.
    pub fn new(evening: bool, stage: bool, key_pos: V3, focus: V3) -> Light {
        Light {
            m: match (evening, stage) {
                (true, false) => pal::MOOD_EVENING,
                (false, false) => pal::MOOD_DAYLIGHT,
                (true, true) => pal::MOOD_STAGE_EVENING,
                (false, true) => pal::MOOD_STAGE_DAYLIGHT,
            },
            evening,
            key_pos,
            focus,
            floor: [0.34, 0.24, 0.16],
            no_streak: false,
            no_fill: false,
            concert: None,
            backs: [(v(0.0, 1.0, 0.0), 0.0); 2],
        }
    }

    /// r42:arcus-polish3 — the concert stage's light round a subject at `aim` seen from
    /// `eye` (cm): the stage's evening mood, its key the concert's spot aimed at the subject,
    /// its pool the concert's.
    pub fn concert(c: &crate::scene::figure::concert::Concert, aim: V3, eye: V3) -> Light {
        let mut l = Light::new(true, true, c.key_pos(aim, eye), aim);
        l.m.spot_r = c.key_r * 1.6;
        l.m.spot_k = 3.0;
        l.m.amb = c.d_amb;
        l.m.key = c.d_key;
        l.m.key_col = crate::scene::figure::concert::warmth(c.key_warm);
        l.m.falloff = 0.5;
        l.concert = Some(*c);
        let b = c.backs(aim, eye);
        let lum = |x: [f32; 3]| 0.2126 * x[0] + 0.7152 * x[1] + 0.0722 * x[2];
        l.backs = [(b[0].0, lum(b[0].1)), (b[1].0, lum(b[1].1))];
        l
    }

    /// r42:arcus-polish3 — the concert's back lights on a surface facing `n` seen along `vv`
    /// (0 off the concert stage): a clean line along the edge that turns toward them.
    pub fn rim(&self, n: V3, vv: V3) -> f32 {
        let Some(c) = &self.concert else {
            return 0.0;
        };
        let edge = smooth(0.55, 0.85, 1.0 - n.dot(vv).clamp(0.0, 1.0));
        let k: f32 = self.backs.iter().map(|&(d, e)| n.dot(d).max(0.0) * e).sum();
        edge * k * c.d_rim
    }

    /// What a broad flat face of `mat` draws as its band at `p` facing `n` (seen along
    /// `vv`, toward the eye): the tall lights' streaks, the key's tight highlight and the
    /// soft box mirrored — the sharp reflections a face shaded once could not hold — as an
    /// opacity of a warm white over the face.
    pub fn band(&self, mat: &Mat, n: V3, vv: V3, p: V3) -> f32 {
        let m = &self.m;
        if self.concert.is_some() {
            // r42:arcus-polish3 — the stage's spot: one clean highlight, no strip lights.
            let nh = n.dot((self.ldir(p) + vv).norm()).max(0.0);
            return (nh.powi(16) * 0.12 * mat.coat).min(0.2);
        }
        let d = reflect(-vv, n);
        // Black lacquer is a mirror first: the tall lights' bar down its bentside as bright
        // as the photograph's (round 40 cp3: at a light finish's strength it read grey).
        let dark = maxc(mat.albedo) < 0.1;
        // A light finish's bands are soft: at a dark one's strength a white case read as
        // polished metal (round 40 cp6).
        let coat = mat.coat
            * if dark {
                0.4
            } else if light_finish(mat) {
                0.07
            } else {
                0.15
            };
        let mut streak = self.streak_p(d, 1800) * coat * 2.4;
        if dark {
            // A tall light straight behind the instrument: the second bar the reference
            // shows where a black bentside turns away toward its tail (round 40 cp4) —
            // on black lacquer's faces only (the floor's gloss would wash out under it).
            let h = |a: V3| v(a.x, 0.0, a.z).norm();
            let st = v(-0.1, 0.0, -1.0).norm();
            let tall = smooth(-0.6, -0.3, d.y) * (1.0 - smooth(0.55, 0.8, d.y));
            streak += h(d).dot(st).max(0.0).powi(900) * tall * m.strip * coat * 2.4;
        }
        let l = self.ldir(p);
        let nh = n.dot((l + vv).norm()).max(0.0);
        let spec = mat.spec * nh.powf(mat.shine) * 0.9 * m.key;
        let bx = d.dot(l).max(0.0).powi(8) * m.env_box * mat.coat * Light::lacquer_gloss(mat);
        if light_finish(mat) {
            // A light finish shows no strip lights' bars (a white bentside's bar read as a
            // chrome streak at its bend, round 40 cp7): the key's own soft sheen only.
            return (spec * 0.3).min(0.35);
        }
        (streak + spec + bx).min(0.92)
    }

    /// The two tall strip lights' crisp streak a polish shows along `d` (sharp across, long
    /// along: they are tall), in units of the environment's tint.
    pub fn streak(&self, d: V3) -> f32 {
        self.streak_p(d, 600)
    }

    /// [`Light::streak`] as sharp across as `pow` says (a flat face's band: sharper).
    pub fn streak_p(&self, d: V3, pow: i32) -> f32 {
        let (sl, sr) = strips();
        let h = |a: V3| v(a.x, 0.0, a.z).norm();
        let dh = h(d);
        let tall = smooth(-0.55, -0.35, d.y) * (1.0 - smooth(0.55, 0.8, d.y));
        // A third tall light behind and to the right: the long bright band a grand's
        // bentside and a kick's shell show toward the back (the photographs' favourite).
        let sb = v(0.6, -0.05, -0.8).norm();
        let s = dh.dot(h(sl)).max(0.0).powi(pow) * 2.4
            + dh.dot(h(sr)).max(0.0).powi(pow) * 1.6
            + dh.dot(h(sb)).max(0.0).powi(pow) * 2.0;
        s * tall * self.m.strip
    }

    /// How much of the environment a lacquer adds before Fresnel: black lacquer is a
    /// mirror first.
    pub fn lacquer_gloss(mat: &Mat) -> f32 {
        if maxc(mat.albedo) < 0.1 {
            0.15
        } else {
            0.035
        }
    }

    /// Toward the key from `p`.
    pub fn ldir(&self, p: V3) -> V3 {
        (self.key_pos - p).norm()
    }

    /// How much of the key's spot reaches `p`: a soft cone round the focus.
    pub fn spot_r(&self, p: V3, radius: f32) -> f32 {
        let d = (p.x - self.focus.x).hypot(p.z - self.focus.z);
        1.0 / (1.0 + (d / radius).powf(self.m.spot_k))
    }

    /// The spot at `p` with the mood's own radius (a stage's tight cone).
    pub fn spot(&self, p: V3) -> f32 {
        self.spot_r(p, self.m.spot_r)
    }

    /// What a mirror sees in direction `d` from `at`.
    pub fn env(&self, d: V3, at: V3) -> C3 {
        let m = &self.m;
        let y = d.y;
        let l = self.ldir(at);
        // The soft box, mirrored (on a broad flat face, `no_streak`, it is the band's too).
        let bx = if self.no_streak {
            0.0
        } else {
            d.dot(l).max(0.0).powi(8) * m.env_box
        };
        let mut k = if y < -0.04 {
            m.env_floor * (0.75 + 0.25 * (1.0 + y).max(0.0))
        } else if y < 0.12 {
            m.env_wall * 0.70
        } else if y < 0.55 {
            m.env_wall * (0.85 + 0.4 * (y - 0.12))
        } else {
            m.env_ceiling
        };
        let (sl, sr) = strips();
        // On a broad flat face (its streak drawn as a band, `no_streak`) the strip lights
        // are left to the band: a flat mirror shows a light where it lines up, not a grey
        // veil over the whole face.
        let (a, b) = if self.no_streak {
            // All of the strip lights on such a face is its band, drawn across it: shaded
            // once per strip, any of their glow here would step from strip to strip.
            (0.0, 0.0)
        } else {
            (d.dot(sl).max(0.0).powi(7), d.dot(sr).max(0.0).powi(7))
        };
        k += bx + m.strip * a + m.strip * 0.6 * b;
        scale(m.env_tint, k)
    }

    /// What a polished case sees in direction `d` from `at`: the environment, the floor's
    /// pool of light where `d` meets the floor (so a black lid's underside shows the stage
    /// it stands on), and the two tall strip lights as crisp streaks — sharp across, long
    /// along (they are tall), what a lacquered piano shows in a photograph.
    pub fn env_gloss(&self, d: V3, at: V3) -> C3 {
        let m = &self.m;
        let mut c = self.env(d, at);
        if d.y < -0.04 && at.y > 0.0 {
            let hit = at + d * (at.y / -d.y);
            let pool = self.spot(hit) * m.key * 0.55;
            c = add(c, scale(self.floor, pool));
        }
        if self.no_streak {
            return c;
        }
        add(c, scale(m.env_tint, self.streak(d)))
    }

    /// A matte surface: the ambient fill and the key, the spot falling off away from it
    /// (`spot` false: the key at full), the floor's bounce on what faces down.
    pub fn lit_s(&self, albedo: C3, n: V3, p: V3, spot: bool) -> C3 {
        let m = &self.m;
        let l = self.ldir(p);
        let ndl = match &self.concert {
            // r42:arcus-polish3 — the stage's clean terminator: the lit side nearly even,
            // the shadow side dark, a narrow soft step between (a drawn spot's light).
            Some(c) => smooth(-0.05, c.d_term, n.dot(l)) * (0.6 + 0.4 * n.dot(l).max(0.0)),
            None => n.dot(l).max(0.0),
        };
        let mut d = ndl * if spot { self.spot(p) } else { 1.0 };
        d += m.fill_floor * 2.2 * (-n.y).max(0.0);
        std::array::from_fn(|i| albedo[i] * (m.amb * m.amb_col[i] + m.key * m.key_col[i] * d))
    }

    pub fn lit(&self, albedo: C3, n: V3, p: V3) -> C3 {
        self.lit_s(albedo, n, p, true)
    }

    /// The one shading function: `mat` at `p` with normal `n`, seen along `vv` (toward the
    /// eye); `tangent` a bronze's circumferential direction; `g` a wood's or bronze's grain.
    pub fn shade(&self, mat: &Mat, n: V3, vv: V3, p: V3, tangent: Option<V3>, g: f32) -> C3 {
        let m = &self.m;
        match mat.kind {
            MatKind::Lacquer => {
                let mut base = mat.albedo;
                if mat.wood {
                    let t = (0.5 + (g - 1.0) / (2.0 * mat.figure) * 0.9).clamp(0.0, 1.0);
                    if t != 0.5 {
                        base = lerp(mat.dark, mat.light, t);
                    }
                    base = lerp(base, mat.albedo, 0.45);
                }
                let mut c = self.lit(base, n, p);
                // A photographer's fill on the instrument (the photographic rig's front panel,
                // overhead box and strips): on a dark stage a white or a maple case keeps its
                // colour away from the key; black lacquer, all coat, is untouched by it.
                let fill = if self.no_fill {
                    0.0
                } else {
                    // A light finish takes a little more: the photograph's soft cream or peach
                    // case, never a grey one.
                    // From the camera's side, as the photograph's front softbox: a light
                    // case's faces toward us bright, turning softly darker as they turn
                    // away (round 40 cp6: lit by the ambient alone a white rim read grey).
                    let k = if light_finish(mat) {
                        8.0 * (0.5 + 0.5 * n.dot(vv).max(0.0))
                    } else {
                        1.8
                    };
                    (0.30 - m.amb).max(0.0) * k * (0.65 + 0.35 * n.y.max(-1.0))
                };
                // A light finish's fill is the softbox's white, not the key's warm tint (on
                // a dark stage a maple went yellow-orange, round 40 cp8).
                let fc = if light_finish(mat) {
                    lerp(m.key_col, [1.0; 3], 0.6)
                } else {
                    m.key_col
                };
                c = add(c, std::array::from_fn(|i| base[i] * fill * fc[i]));
                if light_finish(mat) {
                    // Held under white with its hue (clipped channel by channel a maple went
                    // yellow — red first, then green — beside the photograph's peach).
                    let mx = maxc(c);
                    if mx > 0.82 {
                        c = scale(c, (0.82 + (mx - 0.82) * 0.25) / mx);
                    }
                }
                let l = self.ldir(p);
                let h = (l + vv).norm();
                let nh = n.dot(h).max(0.0);
                if self.concert.is_some() {
                    // r42:arcus-polish3 — under the stage's spot a varnish shows one clean
                    // highlight (a shape, not the strip lights' blotches), the dark hall
                    // round it, its edge's rim from the back lights.
                    let spot = nh.powi(12) * 0.12;
                    let c = add(c, scale(m.key_col, spot * mat.spec.max(0.4) * m.key));
                    return add(c, scale([1.0, 0.9, 0.78], self.rim(n, vv) * 0.6));
                }
                // The key's tight highlight is a broad flat face's band too (`no_streak`):
                // shaded once per strip it would light strips whole, one after another.
                let tight = if self.no_streak {
                    0.0
                } else {
                    nh.powf(mat.shine) * 0.9
                };
                let spec = mat.spec * (tight + nh.powi(12) * 0.06) * m.key;
                c = add(c, scale(m.key_col, spec));
                let nv = n.dot(vv).max(0.0);
                let e = self.env_gloss(reflect(-vv, n), p);
                let fres = 0.04 + 0.96 * (1.0 - nv).powi(5);
                let gloss = Light::lacquer_gloss(mat);
                c = add(c, scale(e, mat.coat * gloss));
                // A light finish keeps its colour toward grazing (white or maple lacquer
                // seen along the rim is still white or maple, its mirror over it).
                // Its mirror over it adds light; it never darkens a light finish toward the
                // dark room it sees (round 40 cp6: a white rim read as chrome, a maple one
                // as grey).
                let lightness = maxc(base).min(1.0);
                let cap = if light_finish(mat) {
                    0.6 * (1.0 - lightness).powi(2)
                } else {
                    0.6 * (1.0 - 0.75 * lightness)
                };
                lerp(c, e, (fres * 0.75 * mat.coat).min(cap))
            }
            MatKind::Chrome => {
                // A stand's chrome as a photograph shows it (round 40 cp4: on the dark
                // stage the drawn stands were thin black lines): the room's colours in it
                // half washed out, a silver fill from the photographer's cards, the key's
                // one long highlight, the edges dark where the tube turns away.
                let e = self.env(reflect(-vv, n), p);
                let y = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
                let e = lerp(e, [y; 3], 0.5);
                let l = self.ldir(p);
                let nh = n.dot((l + vv).norm()).max(0.0);
                let nv = n.dot(vv).max(0.0).sqrt();
                let glint = (nh.powi(60) * 1.1 + nh.powi(10) * 0.10) * m.key;
                let fill = 0.40 * nv;
                let body = scale(add(e, [fill; 3]), 0.35 + 0.65 * nv);
                mul(add(body, [glint; 3]), mat.tint)
            }
            MatKind::Black => {
                let l = self.ldir(p);
                let d = n.dot(l).max(0.0);
                let h = (l + vv).norm();
                let s = n.dot(h).max(0.0).powi(30) * 0.35;
                let kk = 0.05 + 0.10 * d + s;
                [kk, kk * 1.02, kk * 1.06]
            }
            MatKind::Matte => self.lit(mat.albedo, n, p),
            MatKind::Bronze => {
                let l = self.ldir(p);
                let d = n.dot(l).max(0.0);
                // A photographer's fill keeps the bronze's colour on a dark stage (round 40
                // cp4: the drawn cymbals were dark slivers beside the photograph's gold).
                let fill = (0.30 - m.amb).max(0.0) * 1.4;
                let mut c = scale(mat.albedo, g * (m.amb * 0.55 + m.key * 0.55 * d + fill));
                let h = (l + vv).norm();
                let th = tangent.map_or(0.0, |t| t.dot(h));
                let an = (1.0 - th * th).max(0.0).sqrt();
                // Across the grain the lathe spreads the light a few tens of degrees, not
                // all of it: a cymbal facing the key half-way to the eye no longer lights
                // white over its whole face — the spoke is where the half-vector lies.
                let across = tangent.map_or(1.0, |t| {
                    let bh = n.cross(t).dot(h) / 0.3;
                    1.0 / (1.0 + bh * bh * bh * bh)
                });
                // The bright streak a light draws across lathed bronze, and its soft halo.
                let streak =
                    (an.powi(90) * 1.4 + an.powi(18) * 0.35 + an.powi(4) * 0.08) * across * m.key;
                c = add(c, scale(pal::ink::BRONZE_GLINT, streak));
                let e = self.env(reflect(-vv, n), p);
                lerp(c, [e[0], e[1] * 0.85, e[2] * 0.6], 0.18)
            }
        }
    }
}

/// Wood figure along an angle or a length: slow bands and fine stripes, deterministic.
pub fn grain(a: f32, seed: f32, f: f32) -> f32 {
    let mut g = 1.0
        + f * (a * 7.0 + seed).sin()
        + f * 0.7 * (a * 23.0 + 2.1 * (a * 3.0 + seed).sin()).sin();
    g += f * 0.5 * (a * 61.0 + seed * 3.0).sin();
    g
}

/// A material with its colours scaled by `k` (a darker plinth).
pub fn scale_mat(m: &Mat, k: f32) -> Mat {
    Mat {
        albedo: scale(m.albedo, k),
        dark: scale(m.dark, k),
        light: scale(m.light, k),
        ..*m
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
