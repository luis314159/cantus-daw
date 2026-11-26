//! r40:scene-px — shading, per sample: a material's surface at a point (its procedural
//! textures, the room's floor, walls, curtain and rug from their paints), then the light
//! leaving it toward the eye — a clear coat's Fresnel reflection of the panels and the
//! room over its diffuse colour, a metal's tinted reflection, a lathe's or brush's
//! anisotropic streaks, felt's and heads' soft sheen; the key with its soft shadow, the
//! panels' and lamps' fill, the ambient under the occlusion, the floor's warm bounce, a
//! cool rim.

use super::light::Rig;
use super::mat::{fbm, hash3, lin3, noise, smooth, wood_col, Kind, PMat, Tex};
use super::mesh::{ladd, lmix, lmul, lscale, v3, Lin, V3};
use super::trace::Bvh;
use crate::kit::skin::scene::{self as pal, Laid, Motif, Pattern};

/// A surface at a point, ready to be lit.
pub struct Surf {
    pub albedo: Lin,
    /// Reflectance at normal incidence (a coat's 0.04–0.05; a metal's tint).
    pub f0: Lin,
    pub coat: f32,
    pub rough: f32,
    /// A nudge to the normal (a curtain's folds, a cymbal's dents).
    pub dn: V3,
    pub sheen: f32,
    /// r42:arcus-polish — a figure's cloth: its fold here (−1 a crease … +1 a crest) and
    /// how near its edge it is (1 at the edge's roll, 0 a centimetre in).
    pub fold: f32,
    pub edge: f32,
}

fn s(albedo: Lin, coat: f32, rough: f32) -> Surf {
    Surf {
        albedo,
        f0: [0.045; 3],
        coat,
        rough,
        dn: V3::default(),
        sheen: 0.0,
        fold: 0.0,
        edge: 0.0,
    }
}

/// The surface of `m` at `p` (uv, `fp` metres a pixel).
pub fn surf(rig: &Rig, m: &PMat, p: V3, uv: [f32; 2], fp: f32) -> Surf {
    let rm = &rig.room;
    match m.tex {
        Tex::Plain => match m.kind {
            Kind::Metal | Kind::Aniso => Surf {
                f0: m.base,
                ..s([0.0; 3], 1.0, m.rough)
            },
            Kind::Matte => Surf {
                sheen: 0.25,
                ..s(m.base, 0.0, 1.0)
            },
            _ => s(m.base, m.coat, m.rough),
        },
        Tex::Wood { seed, figure } => s(wood_col(m, uv, seed, figure, fp), m.coat, m.rough),
        Tex::Cast => {
            let cast = 0.82 + 0.18 * fbm(p.x * 30.0, p.z * 30.0, 9, 3);
            let fleck = 0.92 + 0.08 * noise(p.x * 900.0, p.z * 900.0, 13);
            Surf {
                f0: lscale(m.base, 0.35),
                ..s(lscale(m.base, cast * fleck), m.coat, m.rough)
            }
        }
        Tex::Spruce => {
            let t = super::mat::wood(uv[0] * 0.7 + uv[1] * 0.7, uv[1] * 0.7 - uv[0] * 0.7, 23, fp);
            s(
                lmix(m.base, lin3(pal::srgb(168, 124, 72)), t * 0.7),
                m.coat,
                m.rough,
            )
        }
        Tex::Fabric => {
            let weave = 0.9
                + 0.1 * noise(uv[0] * 600.0, uv[1] * 600.0, 51)
                + 0.05 * fbm(uv[0] * 8.0, uv[1] * 8.0, 52, 2);
            Surf {
                sheen: 0.4,
                ..s(lscale(m.base, weave), 0.0, 1.0)
            }
        }
        Tex::Grille => {
            let g = ((uv[0] * 1400.0).sin() * (uv[1] * 1400.0).sin()).abs();
            let open = if fp < 0.0008 {
                smooth(0.3, 0.7, g)
            } else {
                0.5
            };
            Surf {
                f0: lscale(m.base, 0.35 + 0.65 * open),
                ..s(lscale(m.base, 0.05), 1.0, 0.35)
            }
        }
        Tex::Fret => {
            let (fu, fv) = (uv[0] * 130.0, uv[1] * 130.0);
            let lat =
                ((fu.sin() * fv.sin()).abs() + 0.35 * (fu * 0.5 + fv * 0.5).sin().abs()).min(1.0);
            let cut = if fp < 0.0015 {
                smooth(0.62, 0.7, lat)
            } else {
                0.25
            };
            s(
                lscale(m.base, 1.0 - 0.9 * cut),
                m.coat * (1.0 - cut),
                m.rough,
            )
        }
        Tex::Head { c, axis, r, coated } => {
            let d = p - c;
            let rho = (d - axis * d.dot(axis)).len() / r.max(1e-3);
            if coated {
                // The coating's cloudy mottle (a few centimetres), its finer texture.
                let cloud = fbm(p.x * 38.0 + p.y * 11.0, p.z * 38.0 - p.y * 7.0, 63, 3);
                let mottle = (0.80 + 0.22 * cloud)
                    * (0.97 + 0.03 * fbm(p.x * 160.0 + p.y * 37.0, p.z * 160.0, 61, 3));
                // The coating's grain, only where a pixel is fine enough to hold it.
                let grain = 1.0 - smooth(0.0003, 0.0009, fp);
                let fleck = 1.0
                    - 0.06
                        * grain
                        * smooth(
                            0.8,
                            0.95,
                            noise(p.x * 2200.0, p.z * 2200.0 + p.y * 900.0, 62),
                        );
                let collar = 1.0 - 0.28 * smooth(0.90, 0.97, rho) + 0.06 * smooth(0.97, 0.99, rho);
                Surf {
                    sheen: 0.35,
                    ..s(lscale(m.base, mottle * fleck * collar), m.coat, m.rough)
                }
            } else {
                let ring = smooth(0.0, 0.012, (rho - 0.70).abs());
                let satin = 0.85 + 0.3 * fbm(p.x * 30.0 + p.y * 9.0, p.z * 30.0 - p.y * 5.0, 64, 3);
                let logo = lmix(lin3(pal::ink::KICK_RING), lscale(m.base, satin), ring);
                s(logo, m.coat, m.rough)
            }
        }
        Tex::Lathe { c, axis, r } => {
            let d = p - c;
            let radial = d - axis * d.dot(axis);
            let rho = radial.len() / r.max(1e-3);
            // The grooves: rings, their sheen alternating; the bell left plain. A groove
            // finer than the pixel is drawn an octave coarser at a time (a lathe's banding
            // as a photograph resolves it), never flattened away (round 40 cp3: the rings
            // were gone at every size the picture is seen at).
            let base_wl = 0.0026f32;
            let octave = (3.5 * fp / base_wl).max(1.0).log2().ceil();
            let wl = base_wl * octave.exp2();
            let groove = 0.84 + 0.16 * (rho * r * std::f32::consts::TAU / wl).sin();
            let bell = smooth(
                crate::scene::kit::BELL - 0.03,
                crate::scene::kit::BELL + 0.01,
                rho,
            );
            // The hammering: shallow dents that break the highlights up.
            // Their size and depth what a pixel can hold (finer, they were a grain).
            let (hx, hz) = (p.x * 70.0 + p.y * 27.0, p.z * 70.0 - p.y * 16.0);
            let dent = noise(hx, hz, 71) - 0.5;
            let tilt = radial.norm() * (dent * 0.06 * bell);
            Surf {
                f0: lscale(m.base, 1.0 + (groove - 1.0) * bell),
                dn: tilt,
                ..s(lscale(m.base, 0.06), 1.0, m.rough)
            }
        }
        Tex::Tonewood {
            seed,
            lines,
            flame,
            rosin,
        } => {
            let t = super::mat::tonewood(uv[0], uv[1], seed, lines, flame, fp);
            let c = lmix(m.light, m.dark, t);
            let c = lmix(c, m.base, 0.3);
            // r42:arcus-polish2 — the rosin's dust: thickest under the strings near where
            // the bow plays, thinning toward the fingerboard and either side, in a fine
            // uneven grain (faded where a pixel cannot hold it); it dulls the varnish's coat.
            let [y0, y1, w, k] = rosin;
            let r = if k > 0.0 {
                let along = smooth(y0 - 0.004, y0 + 0.006, uv[0])
                    * (1.0 - smooth(y0 + 0.01, y1, uv[0]) * 0.75)
                    * (1.0 - smooth(y1 - 0.01, y1 + 0.004, uv[0]));
                let across = 1.0 - smooth(w * 0.55, w, uv[1].abs());
                let grain = 1.0
                    + (noise(uv[0] * 2600.0, uv[1] * 2600.0, 241) - 0.5)
                        * 0.9
                        * (1.0 - smooth(0.0002, 0.0007, fp));
                (k * along * across * grain).clamp(0.0, 0.4)
            } else {
                0.0
            };
            s(
                lmix(c, ROSIN_DUST, r),
                m.coat * (1.0 - 1.5 * r).max(0.0),
                m.rough,
            )
        }
        // r42:arcus-upper — the realistic figures.
        Tex::Skin => {
            let (lum, red) = (uv[0].min(1.1), uv[1]);
            let a = m.base;
            let albedo = [
                (a[0] * lum * (1.0 + 0.22 * red)).max(0.0),
                (a[1] * lum * (1.0 - 0.12 * red)).max(0.0),
                (a[2] * lum * (1.0 - 0.18 * red)).max(0.0),
            ];
            s(albedo, m.coat, m.rough)
        }
        Tex::Velvet => {
            // Soft folds (sheet v7): the normal swung by slow, mostly vertical waves, so
            // the pile's sheen runs in the long soft streaks a hanging velvet shows.
            let a = noise(p.x * 14.0 + 0.4 * (p.y * 5.0).sin(), p.y * 3.0, 91) - 0.5;
            let b = noise(p.z * 14.0 + 0.4 * (p.y * 4.0).cos(), p.y * 3.0 + 7.0, 97) - 0.5;
            // (r42:arcus-polish: its folds and its edge's roll, `raster::place`.)
            Surf {
                sheen: 0.0,
                dn: v3(a * 0.55, 0.0, b * 0.55),
                fold: uv[1].clamp(-1.0, 1.0),
                // (r42:arcus-polish2: over the centimetre the mesh holds — narrower, the
                // distance's straight runs between corners drew the roll as teeth.)
                edge: (-(uv[0] / EDGE_ROLL_CM) * (uv[0] / EDGE_ROLL_CM)).exp() * 0.7,
                ..s(m.base, 0.0, 1.0)
            }
        }
        Tex::Card(k) => {
            // r42:arcus-upper — a card's strands: its texture's grey shades the base; a hair
            // card's `uv[0]` is already its strand's detail (r42:arcus-hair, `raster::place`).
            let f = if hair_card(m.tex) {
                uv[0]
            } else {
                crate::scene::figure::cards::card(k).map_or(1.0, |c| c.shade(uv))
            };
            Surf {
                sheen: 0.0,
                ..s(lscale(m.base, f), m.coat, m.rough)
            }
        }
        Tex::Eye => s(crate::scene::figure::cards::eye(uv), m.coat, m.rough),
        Tex::Wool => {
            // r42:arcus-polish — worsted: a heather of fibres a couple of millimetres
            // across (faded where a pixel cannot hold it) and a faint twill along the
            // diagonal; its edges' roll lighter (a lapel's, a pocket's, a hem's).
            let hold = 1.0 - smooth(0.0012, 0.004, fp);
            let heather = (noise(p.x * 420.0 + p.y * 130.0, p.z * 420.0 - p.y * 90.0, 131) - 0.5)
                * 0.16
                * hold;
            let twill =
                ((p.x + p.y + p.z) * 2400.0).sin() * 0.04 * (1.0 - smooth(0.0004, 0.0012, fp));
            // (r42:arcus-polish2: over the centimetre the mesh holds — at 0.4 cm the
            // distance's straight runs between corners drew the lapel's roll as a dotted
            // line, the stray-pixel gate's.)
            let roll = (-(uv[0] / EDGE_ROLL_CM) * (uv[0] / EDGE_ROLL_CM)).exp() * 0.6;
            Surf {
                sheen: 0.35,
                edge: roll,
                ..s(
                    lscale(m.base, 1.0 + heather + twill + 0.25 * roll),
                    0.0,
                    1.0,
                )
            }
        }
        Tex::Shirt => {
            // r42:arcus-polish2 — poplin: a fine weave a pixel cannot hold at these sizes
            // (a faint unevenness), the shade under the collar's points and the lapels, the
            // collar's rolled edge a little lighter.
            let lum = uv[0].clamp(0.3, 1.2);
            let roll = (-(uv[1] / EDGE_ROLL_CM) * (uv[1] / EDGE_ROLL_CM)).exp();
            let weave = (noise(p.x * 300.0 + p.y * 90.0, p.z * 300.0 - p.y * 70.0, 137) - 0.5)
                * 0.05
                * (1.0 - smooth(0.0012, 0.004, fp));
            Surf {
                sheen: 0.4,
                ..s(
                    lscale(m.base, lum * (1.0 + 0.06 * roll + weave)),
                    m.coat,
                    m.rough,
                )
            }
        }
        Tex::Scalp(_) => s(lscale(m.base, scalp_strands(uv, fp)), m.coat, m.rough),
        Tex::Hair => {
            // Strand bundles across the strands (`uv[0]`), the scalp's under the cards
            // (r42:arcus-hair: one head of hair — the chignon as the hair drawn into it); at
            // the hairline (`uv[1]` 0) the skin's colour (`m.dark`) shows through, as real
            // hair thins there. Its sheen along the strands is `strands`'.
            let across = uv[0];
            let streak = scalp_strands([across, 0.0], fp);
            // The hairline broken up as hair's own tips are (a smooth blend read as the
            // triangles' teeth).
            let tips = 0.45 * (noise(across * 41.0, 0.7, 73) - 0.5);
            let e = (uv[1] + tips * (1.0 - uv[1])).clamp(0.0, 1.0).powf(0.6);
            s(lmix(m.dark, lscale(m.base, streak), e), m.coat * e, m.rough)
        }
        Tex::Floor => floor(rig, p, fp),
        Tex::Rug => rug(rig, p, fp),
        Tex::Wall | Tex::Wall2 => wall(rig, m.tex == Tex::Wall2, p, uv, fp),
        Tex::Curtain => {
            // Velvet in deep folds: each fold its own width; the normal swings across it.
            let x = p.x * 3.4 + 0.6 * (p.x * 0.7).sin();
            let ph = x * std::f32::consts::TAU;
            let k = 0.5 + 0.5 * (p.x * 2.3).sin() * (p.x * 0.6).cos();
            let base = lmix(lin3(rm.walls.fabric), lin3(rm.walls.fabric2), k);
            let fold = 0.62 + 0.38 * ph.cos().abs().powf(0.6);
            Surf {
                dn: v3(ph.sin() * 0.85, 0.0, 0.0),
                sheen: 0.55,
                ..s(lscale(base, fold), 0.0, 1.0)
            }
        }
    }
}

fn floor(rig: &Rig, p: V3, fp: f32) -> Surf {
    let f = &rig.room.floor;
    let (a, b, j) = (lin3(f.a), lin3(f.b), lin3(f.joint));
    let w = f.w / 100.0;
    let rough = 0.06 + 0.25 * (1.0 - f.coat);
    let base = match f.laid {
        Laid::Boards => {
            let plank = (p.x / w).floor();
            let across = p.x / w - plank;
            let off = hash3(plank as i32, 0, 93) * 2.4;
            let len = 1.6 + hash3(plank as i32, 1, 93) * 1.0;
            let along = (p.z + off) / len;
            let piece = along.floor();
            let fz = along - piece;
            let id = (plank as i32).wrapping_mul(131).wrapping_add(piece as i32);
            let tone = 0.84 + 0.3 * hash3(id, 2, 94);
            let t = super::mat::wood(
                p.z + piece * 7.3,
                across * w + plank * 0.37,
                95 + (id as u32 & 7),
                fp,
            );
            let c = lscale(lmix(a, lmix(b, j, 0.35), t * 0.9), tone);
            // A joint finer than a pixel is spread over a pixel and paled by as much: a
            // faint even line, never a row of dots.
            let joint = |t: f32, size: f32| {
                let (j0, jf) = (0.0025 / size, 1.4 * fp / size);
                let jw = j0.max(jf);
                1.0 - (j0 / jw) * (1.0 - smooth(0.0, jw, t) * smooth(0.0, jw, 1.0 - t))
            };
            lmix(j, c, joint(across, w) * joint(fz, len))
        }
        Laid::Slabs | Laid::Stones | Laid::Terrazzo => {
            let stones = f.laid == Laid::Stones;
            let row = (p.z / w).floor();
            let off = if stones && (row as i32) % 2 != 0 {
                0.5
            } else {
                0.0
            };
            let (su, sv) = (p.x / w + off, p.z / w);
            let (iu, iv) = (su.floor(), sv.floor());
            let tone = if stones {
                0.78 + 0.3 * hash3(iu as i32, iv as i32, 91)
            } else {
                0.94 + 0.08 * hash3(iu as i32, iv as i32, 91)
            };
            let jw = (0.006 / w).max(fp / w);
            let (fu, fv) = (su - iu, sv - iv);
            let jn = smooth(0.0, jw, fu)
                * smooth(0.0, jw, 1.0 - fu)
                * smooth(0.0, jw, fv)
                * smooth(0.0, jw, 1.0 - fv);
            let wear = 0.88 + 0.12 * fbm(p.x * 2.0, p.z * 2.0, 92, 3);
            let mut c = lscale(lmix(a, b, hash3(iu as i32, iv as i32, 90)), tone * wear);
            if f.laid == Laid::Terrazzo && fp < 0.004 {
                let chip = noise(p.x * 160.0, p.z * 160.0, 97);
                c = lmix(c, lin3(pal::ink::CHIP_DARK), smooth(0.78, 0.82, chip) * 0.8);
                let chip2 = noise(p.x * 190.0 + 3.0, p.z * 190.0, 98);
                c = lmix(
                    c,
                    lin3(pal::ink::CHIP_LIGHT),
                    smooth(0.8, 0.84, chip2) * 0.6,
                );
            }
            lmix(j, c, jn)
        }
    };
    s(base, (0.25 + 0.75 * f.coat).min(1.0), rough)
}

fn rug(rig: &Rig, p: V3, fp: f32) -> Surf {
    let Some((rg, [x0, z0, x1, z1])) = rig.room.rug else {
        return s([0.1; 3], 0.0, 1.0);
    };
    let k = if rig.evening { 1.0 } else { rg.day };
    let field = lscale(lin3(rg.field), k * k);
    let (border, chain, line) = (lin3(rg.border), lin3(rg.chain), lin3(rg.line));
    let d = (p.x - x0).min(x1 - p.x).min(p.z - z0).min(z1 - p.z);
    let aa = fp.max(0.004);
    let mut b = border;
    let along = if (p.x - x0).min(x1 - p.x) < (p.z - z0).min(z1 - p.z) {
        p.z
    } else {
        p.x
    };
    if d < 0.2 {
        let band = smooth(0.08, 0.08 + aa, d) * (1.0 - smooth(0.15, 0.15 + aa, d));
        let mot = match rg.motif {
            Motif::Chain => smooth(
                0.5,
                0.6,
                ((along / 0.07) * std::f32::consts::TAU).sin() * 0.5 + 0.5,
            ),
            Motif::Diamonds => {
                let f = (along / 0.06).fract();
                smooth(
                    0.55,
                    0.65,
                    1.0 - ((f - 0.5).abs() * 2.0 + ((d - 0.115) / 0.035).abs()),
                )
            }
            Motif::Stripes => smooth(
                0.4,
                0.6,
                ((d / 0.02) * std::f32::consts::TAU).sin() * 0.5 + 0.5,
            ),
        };
        b = lmix(b, chain, band * mot * 0.85);
    }
    let l1 = smooth(0.2, 0.2 + aa, d) * (1.0 - smooth(0.225, 0.225 + aa, d));
    b = lmix(b, line, l1);
    if d > 0.225 {
        let (lu, lv) = ((p.x - x0) / 0.16, (p.z - z0) / 0.16);
        let (fu, fv) = (lu - lu.floor() - 0.5, lv - lv.floor() - 0.5);
        let loz = fu.abs() + fv.abs();
        let ring = smooth(0.30, 0.36, loz) * (1.0 - smooth(0.40, 0.46, loz));
        let detail = if fp < 0.01 { 1.0 } else { 0.3 };
        let mut c = lmix(field, border, 0.35 * ring * detail);
        c = lmix(c, line, 0.2 * (1.0 - smooth(0.0, 0.08, loz)) * detail);
        b = lscale(c, 0.93 + 0.07 * fbm(p.x * 0.6, p.z * 3.0, 101, 2));
    }
    let weave = 0.86 + 0.14 * noise(p.x * 500.0, p.z * 500.0, 103);
    Surf {
        sheen: 0.5,
        ..s(lscale(b, weave), 0.0, 1.0)
    }
}

/// How much more a wall takes by day (linear) — its share of the window's light.
const DAY_WALL: f32 = 1.45;

fn wall(rig: &Rig, side: bool, p: V3, uv: [f32; 2], fp: f32) -> Surf {
    let w = &rig.room.walls;
    let (a, b) = if side {
        (lin3(w.wall2), lin3(w.wall))
    } else {
        (lin3(w.wall), lin3(w.wall2))
    };
    // By day the walls stand in the room's open light, not the key's pool: lit as a
    // daylit photograph shows them (round 40 cp3: the church's and the booth's read a
    // dull grey beside the drawn picture's).
    let (a, b) = if rig.evening {
        (a, b)
    } else {
        (lscale(a, DAY_WALL), lscale(b, DAY_WALL))
    };
    let u = if side { p.z } else { p.x };
    let y = p.y;
    let plaster = 0.94 + 0.06 * fbm(uv[0] * 3.0, uv[1] * 3.0, 41, 3);
    let c = match w.pattern {
        Pattern::Plain | Pattern::Curtain => lscale(a, plaster),
        Pattern::Panels => {
            let f = (u / 0.6).fract();
            let jw = (0.004f32 / 0.6).max(fp / 0.6);
            let j = smooth(0.0, jw, f) * smooth(0.0, jw, 1.0 - f);
            let t = super::mat::wood(y, u, 61 + (u / 0.6).floor() as u32, fp);
            lscale(
                lmix(lmix(a, lin3(w.wood), t * 0.5), lscale(a, 0.45), 1.0 - j),
                plaster,
            )
        }
        Pattern::Acoustic => {
            let (cu, cv) = (u / 0.7, (y - 0.2) / 0.7);
            let (iu, iv) = (cu.floor(), cv.floor());
            let (fu, fv) = (cu - iu, cv - iv);
            let jw = (0.06f32).max(fp / 0.7);
            let inside = smooth(0.0, jw, fu)
                * smooth(0.0, jw, 0.89 - fu)
                * smooth(0.0, jw, fv)
                * smooth(0.0, jw, 0.89 - fv);
            let alt = ((iu as i32 + iv as i32).rem_euclid(3)) == 0;
            let fab = lin3(if alt { w.fabric2 } else { w.fabric });
            let weave = 0.92 + 0.08 * noise(u * 400.0, y * 400.0, 44);
            lmix(
                lscale(a, plaster),
                lscale(fab, weave),
                if y > 0.2 && y < 3.0 { inside } else { 0.0 },
            )
        }
        Pattern::Brick => {
            let (bu, bv) = (u / 0.24, y / 0.08);
            let row = bv.floor();
            let cu = bu + if (row as i32) % 2 == 0 { 0.0 } else { 0.5 };
            let t = hash3(cu.floor() as i32, row as i32, 77);
            let jw = (0.012f32).max(fp / 0.08);
            let ju = smooth(0.0, jw * 0.33, cu.fract()) * smooth(0.0, jw * 0.33, 1.0 - cu.fract());
            let jv = smooth(0.0, jw, bv.fract()) * smooth(0.0, jw, 1.0 - bv.fract());
            let brick = lscale(lmix(a, b, t), 0.85 + 0.25 * noise(u * 40.0, y * 40.0, 78));
            lmix(lin3(pal::srgb(150, 140, 126)), brick, ju * jv)
        }
        Pattern::Stone => {
            let (bu, bv) = (u / 0.8, y / 0.42);
            let row = bv.floor();
            let cu = bu + if (row as i32) % 2 == 0 { 0.0 } else { 0.5 };
            let tone = 0.86 + 0.2 * hash3(cu.floor() as i32, row as i32, 77);
            let jw = (0.012f32 / 0.42).max(fp / 0.42);
            let ju = smooth(0.0, jw * 0.5, cu.fract()) * smooth(0.0, jw * 0.5, 1.0 - cu.fract());
            let jv = smooth(0.0, jw, bv.fract()) * smooth(0.0, jw, 1.0 - bv.fract());
            lscale(
                lmix(a, b, hash3(cu.floor() as i32, row as i32, 79)),
                tone * (0.62 + 0.38 * ju * jv) * plaster,
            )
        }
    };
    s(c, 0.0, 1.0)
}

/// What a reflection ray can meet: the instrument's hierarchy and its materials.
pub struct Trace<'a> {
    pub bvh: &'a Bvh<'a>,
    pub mats: &'a [PMat],
    /// The key's shadow map, looked up once (no penumbra search) where a reflection lands:
    /// a mirrored point's shadow edge is blurred by the gloss anyway.
    pub shadow: &'a super::raster::Shadow,
}

/// Reflections rougher than this see only the room.
const TRACE_ROUGH: f32 = 0.12;

/// The light the instrument sends back along `-d` from where the ray `p + t d` meets it:
/// lit once (the key and its shadow, the fill, the ambient; a gloss's view of the room).
fn lite(rig: &Rig, tr: &Trace, p0: V3, d: V3) -> Option<Lin> {
    let h = tr.bvh.hit(p0, d, 1e-3, 12.0)?;
    let t = tr.bvh.tri(h.tri);
    let m = &tr.mats[t.m as usize];
    if m.kind == Kind::Emit {
        return Some(m.emit);
    }
    let (p, n, uv, _) = super::trace::place(t, &h);
    let v = -d;
    let n = if n.dot(v) < 0.0 { -n } else { n };
    let sf = surf(rig, m, p, uv, 0.002 + h.t * 0.002);
    let ndv = n.dot(v).clamp(0.0, 1.0);
    let (kc, l) = rig.key.at(p);
    let kv = if kc[0] + kc[1] + kc[2] > 1e-4 {
        tr.shadow.hard(p, n)
    } else {
        0.0
    };
    let ndl = n.dot(l).max(0.0);
    let fill = rig.fill(p, n);
    let amb = rig.ambient(n);
    let e: Lin = std::array::from_fn(|i| kc[i] * ndl * kv + fill[i] + amb[i] * 0.8);
    let r = n * (2.0 * ndv) - v;
    Some(match m.kind {
        Kind::Metal | Kind::Aniso => {
            let f: Lin = std::array::from_fn(|i| schlick(sf.f0[i], ndv));
            ladd(lmul(rig.env(p, r, kv, true), f), lmul(sf.albedo, e))
        }
        Kind::Matte => lmul(sf.albedo, e),
        _ => {
            // A coat seen square on mirrors little: its view of the room only where it
            // tells (grazing), the dark above otherwise.
            let f = schlick(sf.f0[0], ndv) * sf.coat;
            let env = if f > 0.08 {
                rig.env(p, r, kv, true)
            } else {
                rig.void
            };
            ladd(lscale(lmul(sf.albedo, e), 1.0 - f), lscale(env, f))
        }
    })
}

/// Schlick's Fresnel.
#[inline]
fn schlick(f0: f32, ndv: f32) -> f32 {
    f0 + (1.0 - f0) * (1.0 - ndv).clamp(0.0, 1.0).powi(5)
}

/// The softboxes a filtered mirror at `p` sees round `r` (unit, `n` the surface): each
/// face taken as a Gaussian of its solid angle, the filter as one too — a box `half` wide
/// along `ea` and the gloss's blur `k` across both — so their overlap is a closed form, as
/// smooth as the filter is wide. The key's own face is dimmed by `kv`.
#[allow(clippy::too_many_arguments)]
fn panels_filtered(
    rig: &Rig,
    p: V3,
    r: V3,
    n: V3,
    ea: V3,
    eb: V3,
    half: f32,
    k: f32,
    kv: f32,
) -> Lin {
    let mut c = [0.0f32; 3];
    let (fa, fb) = (half * half / 3.0 + k * k, k * k + 1e-5);
    for (i, pn) in rig.panels.iter().enumerate().filter(|(_, pn)| pn.seen) {
        let to = pn.c - p;
        let dist = to.len().max(1e-3);
        let kd = to * (1.0 / dist);
        let facing = -kd.dot(pn.n);
        if facing <= 0.0 || kd.dot(n) <= 0.0 || kd.dot(r) <= 0.0 {
            continue;
        }
        let (lu, lv) = (pn.u.len(), pn.v.len());
        let (uh, vh) = (pn.u * (1.0 / lu.max(1e-6)), pn.v * (1.0 / lv.max(1e-6)));
        // Its solid angle, and the Gaussian of that area peaking at 1 on its face.
        let omega = 4.0 * lu * lv * facing / (dist * dist);
        let su = 0.8 * lu * (1.0 - kd.dot(uh).powi(2)).max(0.0).sqrt() / dist;
        let sv = 0.8 * lv * (1.0 - kd.dot(vh).powi(2)).max(0.0).sqrt() / dist;
        let (ua, ub) = (uh.dot(ea), uh.dot(eb));
        let (va, vb) = (vh.dot(ea), vh.dot(eb));
        // The combined covariance in the filter's plane (ea, eb).
        let saa = su * su * ua * ua + sv * sv * va * va + fa;
        let sbb = su * su * ub * ub + sv * sv * vb * vb + fb;
        let sab = su * su * ua * ub + sv * sv * va * vb;
        let det = (saa * sbb - sab * sab).max(1e-12);
        let dl = kd - r;
        let (da, db) = (dl.dot(ea), dl.dot(eb));
        let q = (sbb * da * da - 2.0 * sab * da * db + saa * db * db) / det;
        if q > 30.0 {
            continue;
        }
        let g = (-0.5 * q).exp() / (std::f32::consts::TAU * det.sqrt());
        let face = 0.5 + 0.5 * pn.core;
        let kk = if Some(i) == rig.key_panel { kv } else { 1.0 };
        c = ladd(c, lscale(pn.rad, omega * g * face * kk));
    }
    c
}

/// What a gloss reflects along `r` from `p`, blurred by `rough` (seven rays in a cone),
/// never below the surface `n`.
#[allow(clippy::too_many_arguments)]
fn env_rough(
    rig: &Rig,
    p: V3,
    r: V3,
    n: V3,
    rough: f32,
    kv: f32,
    lid: bool,
    panels: bool,
    tr: Option<&Trace>,
    dr: V3,
) -> Lin {
    let tr = tr.filter(|_| rough < TRACE_ROUGH && panels);
    let one = |d: V3| {
        let d = if d.dot(n) < 0.02 {
            (d - n * (d.dot(n) - 0.02)).norm()
        } else {
            d
        };
        if let Some(tr) = tr {
            if let Some(c) = lite(rig, tr, p + n * 0.002, d) {
                return c;
            }
        }
        if lid && tr.is_none() {
            if let Some(c) = rig.plate_seen(p, d) {
                return c;
            }
        }
        rig.env(p, d, kv, panels)
    };
    // A surface whose normal turns fast under the pixel (a thin tube's half-round in two
    // or three samples): the reflection spread over what the sample's footprint sees —
    // rays along the turn, the gloss's blur across it — so a tube reads as one smooth
    // highlight down its length, never a bead per sample row.
    let k = rough * 0.75;
    let wide = dr.len();
    if wide > (0.6 * k).max(0.03) {
        let b = r.cross(dr);
        let b = if b.len() > 1e-6 {
            b.norm()
        } else {
            super::geo::perp(r)
        };
        const S: [f32; 6] = [-0.85, -0.51, -0.17, 0.17, 0.51, 0.85];
        // The room by rays (it is broad and dim), the lights in closed form: a softbox's
        // small, very bright face caught by one ray of eighteen in a pixel and missed by
        // its neighbour's salted a tube white and black (round 40 cp3).
        let room = |d: V3| {
            let d = if d.dot(n) < 0.02 {
                (d - n * (d.dot(n) - 0.02)).norm()
            } else {
                d
            };
            if let Some(tr) = tr {
                if let Some(c) = lite(rig, tr, p + n * 0.002, d) {
                    return c;
                }
            }
            rig.env(p, d, kv, false)
        };
        let mut acc = [0.0f32; 3];
        let mut cnt = 0.0f32;
        for s in S {
            let d = r + dr * (s * 0.8);
            acc = ladd(acc, room(d.norm()));
            cnt += 1.0;
            if rough >= 0.035 {
                for side in [-1.0f32, 1.0] {
                    acc = ladd(acc, room((d + b * (side * k * d.len())).norm()));
                    cnt += 1.0;
                }
            }
        }
        let mut c = lscale(acc, 1.0 / cnt);
        if panels {
            let ea = dr - r * dr.dot(r);
            let ea = if ea.len() > 1e-6 {
                ea.norm()
            } else {
                r.cross(b)
            };
            let half = 0.68 * wide;
            c = ladd(c, panels_filtered(rig, p, r, n, ea, b, half, k, kv));
        }
        return c;
    }
    if rough < 0.035 {
        return one(r);
    }
    let a = super::geo::perp(r);
    let b = r.cross(a);
    // The middle (twice) and four rays round it on a fixed cross: a soft, even sheen with
    // no grain (a turned pattern per sample would need many samples to average out).
    let mut acc = lscale(one(r), 2.0);
    for (c, sn) in [(1.0f32, 0.0f32), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
        acc = ladd(acc, one((r + a * (c * k) + b * (sn * k)).norm()));
    }
    lscale(acc, 1.0 / 6.0)
}

/// r42:arcus-hair — whether `t` is a hair card's (its maps carry the strands' detail and
/// way; a brow's or a lash's card is grey and coverage).
fn hair_card(t: Tex) -> bool {
    matches!(t, Tex::Card(k) if k % 3 == 0
        && crate::scene::figure::cards::card(k).is_some_and(|c| c.hair))
}

/// r42:arcus-hair — the strands combed back over the scalp under the cards (where the
/// cards end short of the hairline, it is what shows): bundles across the head (`uv[0]`,
/// about a centimetre a unit) drifting slowly along it (`uv[1]`, degrees in from the
/// hairline), each its own light, as the cards' detail is; the finer bundles fade where a
/// pixel (`fp` metres) cannot hold them.
fn scalp_strands(uv: [f32; 2], fp: f32) -> f32 {
    let (a, l) = (uv[0], uv[1]);
    let keep = |cycles_cm: f32| smooth(1.5, 3.0, 0.01 / cycles_cm / fp.max(1e-6));
    let coarse = noise(a * 5.0 + 0.11 * l, l * 0.05, 101) - 0.5;
    let fine = noise(a * 11.0 + 0.07 * l, l * 0.09 + 3.0, 103) - 0.5;
    (1.0 + 0.36 * coarse * keep(5.0) + 0.3 * fine * keep(11.0)).clamp(0.6, 1.4)
}

/// What lights a strand: the normal (the head of hair's, smooth), the eye's and the key's
/// way, the way the strand runs, the key's colour and how much of it reaches.
struct Strands {
    n: V3,
    v: V3,
    l: V3,
    along: V3,
    kc: Lin,
    kv: f32,
    /// Whose hair (0 hers, 1 his).
    who: u8,
    /// r42:arcus-polish — where (m): the tonal streaks along the strands.
    p: V3,
    /// r42:arcus-polish2 — how far in from the hairline (degrees; far: 90): the roots.
    line: f32,
}

/// r42:arcus-hair — a head of hair (the owner's review of sheet v8: her ponytail's cards
/// read as camouflage): every card and the scalp under them lit as one — the diffuse from
/// the head of hair's smooth normal (`e`, wrapped), the colour one base with each strand's
/// own detail, and the light along the strands as Kajiya and Kay have it, from the way the
/// strands run in the card's texture: a narrow, near white highlight where the strands
/// stand square to the half-way vector and a broader one tinted by the hair — together a
/// soft sheen band across the head, as a tied-back head of hair shows. `k` how much of the
/// surface is hair (the scalp's edge is skin); `detail` the strand's own light.
fn strands(m: &PMat, sf: &Surf, s: &Strands, e: Lin, rim: Lin, detail: f32, k: f32) -> Lin {
    let look = crate::scene::figure::cards::hair_look(s.who);
    let n = s.n;
    // r42:arcus-polish — his short hair's own way runs every way (its curls): combed back
    // and up from the face instead, so a sheen band can cross his head as it crosses a
    // groomed man's (the owner, 2026-11-25: "a specular band, no helmet").
    let way = if s.who == 1 {
        let away = v3(-s.v.x, 0.0, -s.v.z).norm();
        (away + v3(0.0, 0.55, 0.0)).norm()
    } else {
        s.along
    };
    let along = way - n * way.dot(n);
    let along = if along.len() < 0.2 {
        super::geo::perp(n)
    } else {
        along.norm()
    };
    // r42:arcus-polish — tonal streaks along the strands (locks a little darker and warmer
    // or lighter and cooler, half a centimetre to two across, long along them): a head of
    // hair, not one tone (the owner: "yellow-flat, helmet-like").
    let across = along.cross(n);
    let (xa, xl) = (s.p.dot(across), s.p.dot(along));
    // (r42:arcus-polish2: and broad sections a few centimetres across, each its own shade —
    // a blonde's lowlights and lighter lengths, the owner's "tonal bands".)
    let band = 0.6 * (super::mat::noise(xa * 190.0, xl * 22.0, 211) - 0.5)
        + 0.4 * (super::mat::noise(xa * 55.0, xl * 7.0, 223) - 0.5)
        + look.sections * (super::mat::noise(xa * 26.0, xl * 3.0, 227) - 0.5);
    let streak = (band * look.streaks).clamp(-0.5, 0.5);
    let tone: Lin = if streak < 0.0 {
        let k = -streak * 2.0;
        // (r42:arcus-polish2: a fair head's lowlights toward honey-brown, deeper.)
        let w = look.lowlight;
        [1.0 - 0.36 * w * k, 1.0 - 0.42 * w * k, 1.0 - 0.5 * w * k]
    } else {
        let k = streak * 2.0;
        [1.0 + 0.2 * k, 1.0 + 0.22 * k, 1.0 + 0.26 * k]
    };
    // r42:arcus-polish2 — the underlayer: hair turned down and in (under the crown's
    // fall, at the nape, round the bun) lies in its own shade and reads deeper, as the
    // roots do (the owner: "darker roots and underlayer").
    let under = 1.0 - look.under * smooth(0.25, -0.65, n.y);
    // r42:arcus-polish2 — the roots a little deeper for the first centimetres in from the
    // hairline, the lengths drawn back lighter toward the crown and the bun (a fair head of
    // hair is never one tone from root to end; the coordinator: along the strands, no blobs).
    let roots = (1.0 - look.roots * (1.0 - smooth(0.0, 14.0, s.line)))
        * (1.0 + look.ends * smooth(20.0, 70.0, s.line));
    let under = under * roots;
    let d = lscale(
        lmul(lmul(sf.albedo, tone), ladd(e, lscale(rim, look.rim))),
        under,
    );
    let h = (s.l + s.v).norm();
    // The highlight fades over the terminator (a strand's back is not lit).
    let lit = (n.dot(s.l) * 2.0 + 0.6).clamp(0.0, 1.0) * s.kv;
    // r42:arcus-polish — Kajiya–Kay's two lobes shifted along the strand as Marschner's are
    // (the cuticle's tilt): the white one toward the root, the tinted one toward the tip, so
    // they read as two bands, not one smear.
    let kk = |t: V3| {
        let th = t.norm().dot(h);
        (1.0 - th * th).max(0.0).sqrt()
    };
    let s1 = kk(along + n * 0.12).powf(look.shine.0) * look.shine.1;
    let s2 = kk(along - n * 0.18).powf(look.sheen.0) * look.sheen.1;
    let tint = lmul(lscale(lmul(m.base, tone), 1.5), look.warm);
    // (A bright strand catches more of it; the gaps' deeper hair less.)
    // r42:arcus-polish2 — the band broken along the strands into clumps (a groomed head's
    // sheen is a broken band of light, never a sheet: the owner on sheet v11).
    let clumps = smooth(0.3, 0.72, super::mat::noise(xa * 300.0, xl * 36.0, 239));
    let g = detail * detail * k * (1.0 + streak) * (1.0 - look.broken * (1.0 - clumps)) * under;
    // (His primary tinted too: a white one over dark brown read as a grey sheet, the owner's
    // review of sheet v11.)
    let white = if s.who == 1 { 0.0 } else { 1.0 };
    let spec: Lin = std::array::from_fn(|i| {
        s.kc[i] * lit * g * (s1 * (white + (1.0 - white) * tint[i]) + s2 * tint[i])
    });
    ladd(d, spec)
}

/// r42:arcus-polish2 — rosin's dust (linear): a pale, slightly warm grey.
const ROSIN_DUST: Lin = [0.62, 0.58, 0.5];

/// r42:arcus-polish2 — how wide a cloth's edge roll is (cm from the edge): what the
/// figure's mesh, a vertex a centimetre or so, can draw smoothly.
const EDGE_ROLL_CM: f32 = 1.0;

/// r42:arcus-polish — how much the back light lifts a velvet's turned-away edge (its rim
/// over the dark room: the arm, the shoulder told from what is behind them).
const VELVET_BACK: f32 = 1.8;

/// r42:arcus-polish — skin's sheen from the key alone (the room's softboxes left out of its
/// reflection): a broad lobe and a tighter one (the forehead's, the nose's, the lips'
/// light), Schlick's Fresnel at the half-way vector, in the key's shadow none.
fn skin_sheen(n: V3, v: V3, l: V3, kc: Lin, kv: f32, ndl: f32) -> Lin {
    let h = (l + v).norm();
    let ndh = n.dot(h).max(0.0);
    let f = schlick(0.028, l.dot(h).max(0.0));
    let lobe = 0.8 * ndh.powi(14) + 3.0 * ndh.powi(70);
    let k = f * lobe * ndl.min(1.0) * kv;
    std::array::from_fn(|i| kc[i] * k)
}

/// r42:arcus-polish3 — black cloth's broad sheen from the concert's key (`k` its strength):
/// a soft lobe round the half-way vector, brighter toward grazing, in the key's shadow none.
#[allow(clippy::too_many_arguments)]
fn cloth_spec(
    c: &crate::scene::figure::concert::Concert,
    k: f32,
    n: V3,
    v: V3,
    l: V3,
    kc: Lin,
    kv: f32,
) -> Lin {
    let h = (l + v).norm();
    let ndh = n.dot(h).max(0.0);
    let ndl = n.dot(l).max(0.0);
    let graze = 0.6 + 0.4 * (1.0 - n.dot(v).clamp(0.0, 1.0));
    let s = k * ndh.powf(c.cloth_pow) * ndl * kv * graze;
    std::array::from_fn(|i| kc[i] * s)
}

/// r42:arcus-polish3 — the back lights' line along a black cloth's edge (the photographs'
/// dark sleeves and shoulders told from the dark hall by it): the cloth's own sheen seen at
/// grazing toward a light behind, not its black albedo's share.
fn cloth_edge(rig: &Rig, c: &crate::scene::figure::concert::Concert, n: V3, ndv: f32) -> Lin {
    let g = (1.0 - ndv).powi(3);
    let mut e = [0.0f32; 3];
    for &(d, col) in &rig.rims {
        e = ladd(e, lscale(col, n.dot(d).max(0.0) * g * c.cloth_rim));
    }
    e
}

/// The light leaving the surface of `m` at `p` toward `eye`. `n` is the shading normal,
/// `turn` how far it turns over one sample (a thin tube's: its mirror is spread over it),
/// `t` the grain (`dP/du`), `vis` how much of the key reaches (its shadow), `ao` the
/// occlusion.
#[allow(clippy::too_many_arguments)]
pub fn shade(
    rig: &Rig,
    m: &PMat,
    eye: V3,
    p: V3,
    n: V3,
    turn: V3,
    t: V3,
    uv: [f32; 2],
    fp: f32,
    ao: f32,
    vis: &dyn Fn(V3, V3) -> f32,
    tr: Option<&Trace>,
    given: Option<Lin>,
) -> Lin {
    if m.kind == Kind::Emit {
        return m.emit;
    }
    let mut sf = surf(rig, m, p, uv, fp);
    // r42:arcus-polish3 — under the concert light, skin graded warmer (its spots' tungsten
    // on it, the photographs' saturated skin).
    let cc = rig.concert.as_ref();
    if let (Some(c), Tex::Skin) = (cc, m.tex) {
        sf.albedo = std::array::from_fn(|i| sf.albedo[i] * c.skin_gain[i]);
    }
    // A traced reflection already found (at half resolution, `raster.rs`) stands in for
    // tracing it here.
    let traced = |rough: f32, panels: bool, f: &dyn Fn() -> Lin| match given {
        Some(c) if tr.is_some() && rough < TRACE_ROUGH && panels => c,
        _ => f(),
    };
    let v = (eye - p).norm();
    let figure = matches!(
        m.tex,
        Tex::Skin | Tex::Hair | Tex::Velvet | Tex::Card(_) | Tex::Scalp(_) | Tex::Eye
    );
    // r42:arcus-hair — one head of hair: the hair cards and the scalp under them (how much
    // of the scalp is hair, not the skin at its edge).
    let hairy = match m.tex {
        Tex::Scalp(_) | Tex::Hair => Some(1.0),
        t if hair_card(t) => Some(1.0),
        _ => None,
    };
    // (The screen's occlusion is not a head of hair's: its cards lie in layers a centimetre
    // apart, and the depths between them darkened each lower layer — his crown read as
    // lighter and darker patches.)
    let ao = if hairy.is_some() { 1.0 } else { ao };
    let mut n = if figure && n.dot(v) < 0.0 && n.dot(v) > -0.35 {
        // r42:arcus-upper — at a figure's silhouette the smooth normal turns just past the
        // eye: bent back to graze it, not flipped (flipped it faced away from every light:
        // a dark outline round a face, a neck, a sleeve).
        (n - v * (n.dot(v) - 0.02)).norm()
    } else if n.dot(v) < 0.0 {
        -n
    } else {
        n
    };
    if sf.dn.dot(sf.dn) > 0.0 {
        let nn = (n + sf.dn).norm();
        if nn.dot(v) > 0.02 {
            n = nn;
        }
    }
    let ndv = n.dot(v).clamp(0.0, 1.0);
    let (kc, l) = rig.key.at(p);
    let lit = kc[0] + kc[1] + kc[2] > 1e-4;
    let kv = if lit { vis(p, n) } else { 0.0 };
    let ndl = if m.tex == Tex::Skin {
        // r42:arcus-upper — light through skin: the terminator wrapped wide and soft (the
        // owner, 2026-11-24: skin, not paint). (r42:arcus-polish3: narrower under the
        // concert's spot — the face's shadow side.)
        let w = cc.map_or(0.2, |c| c.skin_wrap);
        ((n.dot(l) + w) / (1.0 + w)).max(0.0)
    } else if hairy.is_some() {
        // r42:arcus-hair — light through hair: a head of hair's terminator is softer still.
        let w = cc.map_or(0.35, |c| c.hair_wrap);
        ((n.dot(l) + w) / (1.0 + w)).max(0.0)
    } else {
        n.dot(l).max(0.0)
    };
    // What a polished metal sees of the key's face: its shadow looked up a little toward
    // the key, so a thin rod never shadows its own glint (off along the normal, its far
    // side's look-ups fell in its own shadow in steps down its length).
    let kvs = if lit && matches!(m.kind, Kind::Metal | Kind::Aniso) {
        vis(p + l * 0.03, l)
    } else {
        kv
    };
    // Diffuse light: the key, the fill, the ambient, the floor's bounce on what faces down,
    // a cool rim at grazing angles.
    let (kf, _) = rig.key.at(v3(p.x, 0.0, p.z));
    let below = (-n.y).max(0.0);
    let floor_alb = lin3(rig.room.floor.a);
    let fill = rig.fill(p, n);
    let amb = rig.ambient(n);
    let mut e = [0.0f32; 3];
    // (r42:arcus-upper: the screen-space occlusion stays in the ambient only — its
    // dithered samples, carried into the key or the fill, showed on a face as fine
    // diagonal stripes at 200 %.)
    for i in 0..3 {
        e[i] = kc[i] * ndl * kv + fill[i] + amb[i] * ao + kf[i] * floor_alb[i] * 0.55 * below * ao;
    }
    if m.tex == Tex::Skin {
        // r42:arcus-upper — skin's scattered light: a warm share where the key falls off
        // (stronger: the light under skin is red), a little red in the shade, and a soft
        // high-key fill from the camera's side on whatever faces it (a portrait's light).
        let t = (1.0 - n.dot(l).max(0.0)) * ndl;
        let sss = cc.map_or(0.18, |c| c.skin_sss);
        e[0] += kc[0] * kv * t * sss + amb[0] * ao * 0.06;
        e[1] += kc[1] * kv * t * sss * 0.22 + amb[1] * ao * 0.015;
        let front = ndv.powf(2.0) * cc.map_or(0.07, |c| c.skin_front);
        for (i, ei) in e.iter_mut().enumerate() {
            *ei += (kc[i] + amb[i]) * front;
        }
    }
    let mut rim = [0.0f32; 3];
    // The rim separates the instrument from the dark; the room takes none.
    let room = matches!(
        m.tex,
        Tex::Floor | Tex::Rug | Tex::Wall | Tex::Wall2 | Tex::Curtain
    );
    // Skin takes a little of it (r42:arcus-upper: a full rim read as a white halo).
    let rim_k = match (m.tex, cc) {
        (Tex::Skin, Some(c)) => c.skin_rim,
        (Tex::Skin, None) => 0.3,
        (_, Some(c)) if hairy.is_some() => c.hair_rim,
        _ => 1.0,
    };
    for &(d, c) in rig.rims.iter().filter(|_| !room) {
        let k = n.dot(d).max(0.0) * (1.0 - ndv).powi(2) * rim_k;
        rim = ladd(rim, lscale(c, k));
    }
    let r = n * (2.0 * ndv) - v;
    if let Some(k) = hairy {
        // The scalp's `t` is across its strands (its `u`); a card's runs along them.
        let along = if hair_card(m.tex) { t } else { n.cross(t) };
        let detail = if hair_card(m.tex) {
            uv[0]
        } else if m.tex == Tex::Hair {
            scalp_strands([uv[0], 0.0], fp)
        } else {
            scalp_strands(uv, fp)
        };
        let who = match m.tex {
            Tex::Card(k) => k / 3,
            Tex::Scalp(w) => w,
            _ => 0,
        };
        let lit = Strands {
            n,
            v,
            l,
            along,
            kc,
            kv,
            who,
            p,
            line: if matches!(m.tex, Tex::Card(_) | Tex::Scalp(_)) {
                uv[1]
            } else {
                90.0
            },
        };
        return rig.hazed(p, strands(m, &sf, &lit, e, rim, detail, k));
    }
    let c = match m.kind {
        Kind::Metal => {
            let f: Lin = std::array::from_fn(|i| schlick(sf.f0[i], ndv));
            let dr = if turn.dot(turn) > 1e-8 {
                let nn = (n + turn).norm();
                (nn * (2.0 * nn.dot(v).max(0.0)) - v) - r
            } else {
                V3::default()
            };
            let spread = dr.len() > (0.45 * sf.rough).max(0.03);
            let env = if spread {
                env_rough(rig, p, r, n, sf.rough, kvs, false, true, tr, dr)
            } else {
                traced(sf.rough, true, &|| {
                    env_rough(rig, p, r, n, sf.rough, kvs, false, true, tr, dr)
                })
            };
            // A stand's chrome reads silver, as a photograph's does: the room's colours in
            // it (a red rug under every mast) half washed out — at full strength each
            // stand read as a copper or a red-and-white pole (round 40 cp3).
            let env = if sf.rough >= 0.1 {
                let y = 0.2126 * env[0] + 0.7152 * env[1] + 0.0722 * env[2];
                lmix(env, [y; 3], 0.55)
            } else {
                env
            };
            ladd(lmul(env, f), lmul(sf.albedo, e))
        }
        Kind::Aniso => aniso(rig, &sf, p, n, v, t, kc, kvs, ndv, e),
        Kind::Matte if m.tex == Tex::Velvet => {
            // r42:arcus-upper — velvet: dark where it faces the eye, its pile's pale sheen
            // where it turns away (a light of its own, not the black's).
            // r42:arcus-polish — and a bright rim where it turns fully away (the pile seen
            // along its length), the back light's along its edge too; its folds' crests
            // catch it and their creases go dark; an edge's roll catches it as well.
            let g = 1.0 - ndv;
            let fold = sf.fold;
            let pile = (g * g * 0.28 + g.powi(5) * 0.42 + sf.edge * 0.22)
                * (1.0 + 0.85 * fold)
                * cc.map_or(1.0, |c| c.velvet_pile);
            let lift = 1.0 + 0.35 * fold.min(0.0) + 0.2 * fold.max(0.0);
            let back: f32 = rig
                .rims
                .iter()
                .map(|&(dd, _)| n.dot(dd).max(0.0) * g.powi(3))
                .sum::<f32>();
            let d = lmul(sf.albedo, ladd(e, lscale(rim, 0.8)));
            let sheen = lmul(m.light, ladd(lscale(kc, kv.max(0.35)), amb));
            let rimc = rig.rims.first().map_or([0.0; 3], |r| r.1);
            // r42:arcus-polish3 — under the concert's spot, the pile's broad sheen where the
            // fold's crest turns toward it (the photographs' black dress lit along its folds).
            let (vb, spec) = match cc {
                Some(c) => (
                    c.velvet_back,
                    ladd(
                        cloth_spec(c, c.velvet_spec, n, v, l, kc, kv),
                        cloth_edge(rig, c, n, ndv),
                    ),
                ),
                None => (VELVET_BACK, [0.0; 3]),
            };
            lscale(
                ladd(
                    ladd(ladd(d, lscale(sheen, pile.max(0.0))), lmul(m.light, spec)),
                    lscale(lmul(m.light, rimc), back * vb),
                ),
                lift,
            )
        }
        Kind::Matte => {
            let sheen = sf.sheen
                * (1.0 - ndv).powi(3)
                * match (cc, m.tex) {
                    (Some(c), Tex::Wool) => c.wool_sheen,
                    _ => 1.0,
                };
            let d = lmul(sf.albedo, ladd(e, lscale(rim, 0.8)));
            let c = ladd(
                d,
                lscale(lmul(sf.albedo, ladd(kc, amb)), sheen * kv.max(0.3)),
            );
            // r42:arcus-polish3 — wool's soft sheen along its folds where the spot hits.
            match (cc, m.tex) {
                (Some(cn), Tex::Wool) => ladd(
                    ladd(c, cloth_spec(cn, cn.wool_spec, n, v, l, kc, kv)),
                    cloth_edge(rig, cn, n, ndv),
                ),
                _ => c,
            }
        }
        _ => {
            let f = schlick(sf.f0[0], ndv) * sf.coat;
            // r42:arcus-polish — skin mirrors no softbox: the room's tall strips (the
            // pianos' long crisp bars) read on a face and a neck as pale vertical streaks
            // (the owner's 200 % sheet, her cheek, his forehead); its sheen is the key's own,
            // below.
            let panels = m.tex != Tex::Floor && m.tex != Tex::Skin;
            let env = traced(sf.rough, panels, &|| {
                env_rough(
                    rig,
                    p,
                    r,
                    n,
                    sf.rough,
                    kv,
                    m.lid_under,
                    panels,
                    tr,
                    V3::default(),
                )
            });
            let d = lmul(sf.albedo, ladd(e, lscale(rim, 0.5)));
            let c = ladd(lscale(d, 1.0 - f), lscale(env, f));
            if m.tex == Tex::Skin {
                ladd(c, skin_sheen(n, v, l, kc, kv, ndl))
            } else {
                c
            }
        }
    };
    let c = if m.kind == Kind::Lacquer || m.kind == Kind::Metal {
        ladd(c, lscale(rim, 0.25))
    } else {
        c
    };
    rig.hazed(p, c)
}

/// The traced reflection `shade` would find for `m` at `p` (seen from `eye`), when it
/// traces one there: a metal or a coat smoother than the tracing's limit, never the floor.
#[allow(clippy::too_many_arguments)]
pub fn reflection(
    rig: &Rig,
    m: &PMat,
    eye: V3,
    p: V3,
    n: V3,
    uv: [f32; 2],
    fp: f32,
    vis: &dyn Fn(V3, V3) -> f32,
    tr: &Trace,
) -> Option<Lin> {
    if !matches!(m.kind, Kind::Metal | Kind::Lacquer) || m.tex == Tex::Floor {
        return None;
    }
    let sf = surf(rig, m, p, uv, fp);
    if sf.rough >= TRACE_ROUGH {
        return None;
    }
    let v = (eye - p).norm();
    let mut n = if n.dot(v) < 0.0 { -n } else { n };
    if sf.dn.dot(sf.dn) > 0.0 {
        let nn = (n + sf.dn).norm();
        if nn.dot(v) > 0.02 {
            n = nn;
        }
    }
    let ndv = n.dot(v).clamp(0.0, 1.0);
    let (kc, l) = rig.key.at(p);
    let lit = kc[0] + kc[1] + kc[2] > 1e-4;
    let r = n * (2.0 * ndv) - v;
    let flat = V3::default();
    Some(if m.kind == Kind::Metal {
        let kvs = if lit { vis(p + l * 0.03, l) } else { 0.0 };
        env_rough(rig, p, r, n, sf.rough, kvs, false, true, Some(tr), flat)
    } else {
        let kv = if lit { vis(p, n) } else { 0.0 };
        env_rough(
            rig,
            p,
            r,
            n,
            sf.rough,
            kv,
            m.lid_under,
            true,
            Some(tr),
            flat,
        )
    })
}

/// A grained metal: the environment seen through normals spread across the grain (the
/// bright spoke a light draws on a cymbal, the streak along a brushed tube), and the key's
/// own anisotropic highlight.
#[allow(clippy::too_many_arguments)]
fn aniso(
    rig: &Rig,
    sf: &Surf,
    p: V3,
    n: V3,
    v: V3,
    t: V3,
    kc: Lin,
    kv: f32,
    ndv: f32,
    e: Lin,
) -> Lin {
    let t = (t - n * t.dot(n)).norm();
    let t = if t.len() < 0.5 {
        super::geo::perp(n)
    } else {
        t
    };
    let b = n.cross(t);
    const S: [f32; 7] = [-0.9, -0.55, -0.25, 0.0, 0.25, 0.55, 0.9];
    const ENV_CAP: f32 = 1.1;
    let spread = 0.25 + sf.rough * 1.4;
    let mut acc = [0.0f32; 3];
    for s in S {
        let nn = (n + b * (s * spread)).norm();
        let ndv2 = nn.dot(v);
        if ndv2 <= 0.0 {
            continue;
        }
        let r = nn * (2.0 * ndv2) - v;
        let r = if r.dot(n) < 0.02 {
            (r - n * (r.dot(n) - 0.02)).norm()
        } else {
            r
        };
        // Each look at the room held under a bright bronze's level: the key's face
        // mirrored whole washed a cymbal's top out to white, its lathe and its colour
        // gone (round 40 cp3); the key's own streak below draws the spoke.
        let c = rig.env(p, r, kv, true);
        let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        acc = ladd(
            acc,
            if l > ENV_CAP {
                lscale(c, ENV_CAP / l)
            } else {
                c
            },
        );
    }
    let env = lscale(acc, 1.0 / S.len() as f32);
    let (_, l) = rig.key.at(p);
    let h = (l + v).norm();
    let th = t.dot(h);
    let an = (1.0 - th * th).max(0.0).sqrt();
    // Across the grain the lathe's spread is a few tens of degrees, not all of it: a
    // cymbal facing the key half-way to the eye flashed white over its whole face (round
    // 40 cp3, the cymbals set flatter); the spoke is where the half-vector lies within it.
    let bh = b.dot(h) / (0.22 + sf.rough);
    let across = 1.0 / (1.0 + bh * bh * bh * bh);
    let streak = (an.powi(120) * 1.6 + an.powi(24) * 0.25) * across * kv * n.dot(l).max(0.0).sqrt();
    // Bronze keeps its colour toward grazing (a cymbal is seen nearly edge on, and
    // Schlick's rise to white turned every cymbal a pale grey): a quarter of the way.
    let g5 = (1.0 - ndv).clamp(0.0, 1.0).powi(5);
    let f: Lin = std::array::from_fn(|i| sf.f0[i] + (0.25 - 0.25 * sf.f0[i]) * g5);
    let spec = lscale(lmul(kc, sf.f0), streak);
    ladd(ladd(lmul(env, f), spec), lmul(sf.albedo, e))
}
