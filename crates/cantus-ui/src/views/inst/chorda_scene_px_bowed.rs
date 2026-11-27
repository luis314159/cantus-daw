//! r42:arcus-look — Arcus's cello and double bass for the photographic renderer: the scene
//! module's surfaces (`scene::bowed_mesh`, the same the Drawn picture is built from) as
//! smooth triangles — each grid cell two, every corner its own normal, so the arching and
//! the scroll's turns shade round — in their materials: the spruce top's quartered grain
//! and the maple's flame (`Tex::Tonewood`) under the varnish's clear coat, ebony, the
//! bridge's pale maple, nickel, brass, the strings' steel and the silks; the f-holes and
//! the bridge's cut-outs dark, the strings and rods round.

use super::geo::{ear_clip, of, perp, vt, Geo, Vert};
use super::mat::{finish, lin3, parts, MatId, PMat, Tex};
use super::mesh::V3;
use crate::kit::skin::scene::bowed as pal;
use crate::kit::skin::scene::hold as hp; // r42:arcus-upper
use crate::scene::bowed::Bowed;
use crate::scene::bowed_mesh::{mesh_for, Role};
use std::collections::HashMap;

/// r42:arcus-polish — her blonde seen per pixel, a little toward ash (the owner,
/// 2026-11-25: "platinum reads yellow-flat"), linear.
fn fair() -> super::mesh::Lin {
    let c = lin3(hp::HAIR_FAIR);
    [c[0] * 0.93, c[1] * 0.96, c[2] * 1.08]
}

/// r42:arcus-polish2 — a played top's rosin (the owner's list: "very faint rosin dust under
/// the strings"): the bow sheds it where it crosses the strings, so it lies on the top from
/// just past the bridge toward the fingerboard's end, under the strings' span — a faint pale
/// haze over the varnish, as every played violin in the photographs shows.
fn with_rosin(b: Bowed, mut m: PMat) -> PMat {
    let d = b.dims();
    let yb = crate::scene::bowed::bridge_y(d);
    let ye = crate::scene::bowed::nut_y(d) - d.board;
    let y1 = yb + 0.75 * (ye - yb).max(1.0);
    let w = d.spacing.1 * 1.5 + 0.35 * d.body / 35.5;
    if let Tex::Tonewood { rosin, .. } = &mut m.tex {
        *rosin = [(yb + 0.4) / 100.0, y1 / 100.0, w / 100.0, ROSIN];
    }
    m
}

/// How much of the rosin's colour lies over the varnish at its thickest.
const ROSIN: f32 = 0.2;

/// r42:arcus-polish4 — his loose tips' albedo over his dark hair's (lighter: a lone strand at
/// the outline catches the back lights over its whole width).
/// r42:arcus-polish6 — 1.6 (2.4 before): with the tips thinner, the ones lying on his crown
/// read as faint strands, not bright scratches; the outline stays broken.
const TIP_ALBEDO: f32 = 1.6;

/// The photographic material of a role (r42:arcus-polish3: under the concert light `cc`, a
/// violin's or a viola's varnish red-amber and glossier, as the photographs' are under the
/// spots).
fn pmat(b: Bowed, r: Role, cc: Option<&crate::scene::figure::concert::Concert>) -> PMat {
    // The varnish seen per pixel: its colour deeper than the Drawn picture's flat facets
    // need (the clear coat over it adds its own light).
    let tone = |m: &crate::kit::skin::scene::Mat, lines: f32, flame: f32, seed: u32| {
        let f = finish(m, seed);
        // Deeper and more saturated (red kept, blue taken): under the clear coat's white
        // reflection a varnish's orange must still read orange, not brown or salmon.
        // A violin's or a viola's oil varnish kept nearer its amber-brown, less of the blue
        // taken (the Photographic picture's orange read as plastic at a violin's size).
        let held = b.dims().held; // r42:arcus-upper
        let k = |c: [f32; 3]| {
            if let (true, Some(cc)) = (held, cc) {
                let v = cc.varnish;
                [c[0] * v[0], c[1] * v[1], c[2] * v[2]]
            } else if held {
                [c[0] * 0.40, c[1] * 0.31, c[2] * 0.23] // r42:arcus-upper
            } else {
                [c[0] * 0.56, c[1] * 0.40, c[2] * 0.26]
            }
        };
        PMat {
            tex: Tex::Tonewood {
                seed,
                lines,
                flame,
                rosin: [0.0; 4],
            },
            base: k(f.base),
            dark: k(f.dark),
            light: k(f.light),
            coat: match (held, cc) {
                (true, Some(cc)) => cc.varnish_coat,
                _ => f.coat * 0.5,
            },
            rough: match (held, cc) {
                (true, Some(cc)) => cc.varnish_rough,
                _ => (f.rough + 0.08).min(0.4),
            },
            ..f
        }
    };
    let (top, maple) = match b {
        Bowed::Cello => (pal::CELLO_TOP, pal::CELLO_MAPLE),
        Bowed::Bass => (pal::BASS_TOP, pal::BASS_MAPLE),
        Bowed::Violin => (pal::VIOLIN_TOP, pal::VIOLIN_MAPLE), // r42:arcus-upper
        Bowed::Viola => (pal::VIOLA_TOP, pal::VIOLA_MAPLE),    // r42:arcus-upper
    };
    let held = b.dims().held; // r42:arcus-upper
    match r {
        // r42:arcus-upper — a violin's or a viola's: its spruce's lines finer than a pixel
        // at the picture's sizes (they fade to the wood's slow streaks, no moiré), its
        // flame half as strong (stripes, not bars).
        Role::Top if held => with_rosin(b, tone(&top, 1100.0, 0.0, 71)),
        Role::Maple if held => tone(&maple, 260.0, 0.45, 73),
        Role::Top => with_rosin(b, tone(&top, 520.0, 0.0, 71)),
        Role::Maple => tone(&maple, 260.0, 1.0, 73),
        Role::Inside => {
            let mut m = tone(&maple, 260.0, 1.0, 73);
            m.base = m.base.map(|c| c * 0.4);
            m.dark = m.dark.map(|c| c * 0.4);
            m.light = m.light.map(|c| c * 0.4);
            m
        }
        Role::Ebony | Role::Purfling => PMat::lacquer(lin3(pal::EBONY.albedo), 0.75, 0.10),
        Role::Bridge => PMat {
            tex: Tex::Tonewood {
                seed: 77,
                lines: 380.0,
                flame: 0.3,
                rosin: [0.0; 4],
            },
            ..finish(&pal::BRIDGE, 77)
        },
        Role::BridgeCut => PMat::matte(lin3(top.dark).map(|c| c * 0.35)),
        Role::Nickel => parts::nickel(),
        Role::Brass => parts::brass(),
        Role::Rubber => parts::rubber(),
        Role::Hole => PMat::matte(lin3(pal::HOLE)),
        Role::Cut => PMat::matte(lin3(pal::CUT)),
        Role::Silk(i) => PMat::matte(lin3(pal::SILK[(i as usize).min(3)])).with(Tex::Fabric),
        Role::Steel => parts::steel(),
        Role::Wound => PMat::metal(lin3(pal::TUNGSTEN.tint), 0.22),
        Role::Chair => finish(&crate::kit::skin::scene::FINISH_WALNUT, 91),
        Role::Seat => PMat::matte(lin3(pal::SEAT.albedo)).with(Tex::Fabric),
        // r42:arcus-upper — what holds it: the Drawn picture's materials, seen per pixel.
        Role::Stand => PMat::lacquer(lin3(hp::STAND.albedo), 0.35, 0.25),
        Role::Pad => PMat::matte(lin3(hp::PAD.albedo)).with(Tex::Fabric),
        Role::Skin(i) => PMat::matte(lin3(hp::skin(i).albedo)),
        Role::SkinShade(i) => PMat::matte(lin3(hp::skin_shade(i).albedo)),
        Role::Cloth(i) => PMat::matte(lin3(hp::cloth(i).albedo)).with(Tex::Fabric),
        Role::Trousers => PMat::matte(lin3(hp::TROUSERS)).with(Tex::Fabric),
        Role::Shoes => PMat::lacquer(lin3(hp::SHOES), 0.4, 0.2),
        Role::Hair => PMat::matte(lin3(hp::HAIR)),
        Role::Feature => PMat::matte(lin3(hp::FEATURE)),
        Role::Blush => PMat::matte(lin3(hp::BLUSH)),
        Role::Wood(i) => PMat::lacquer(lin3(hp::wood(i, false).albedo), 0.3, 0.3),
        Role::Joint(i) => PMat::lacquer(lin3(hp::wood(i, true).albedo), 0.3, 0.3),
        // r42:arcus-upper — the realistic figures.
        Role::Velvet => PMat {
            light: lin3(hp::VELVET_SHEEN),
            ..PMat::matte(lin3(hp::VELVET)).with(Tex::Velvet)
        },
        Role::Wool => PMat::matte(lin3(hp::WOOL)).with(Tex::Wool), // r42:arcus-polish
        Role::Shirt => PMat::matte(lin3(hp::SHIRT)).with(Tex::Fabric),
        Role::Tie => PMat::lacquer(lin3(hp::TIE), 0.12, 0.35),
        Role::HairFair => PMat::lacquer(fair(), 0.1, 0.42).with(Tex::Hair),
        // r42:arcus-polish2 — a loose hair: her blonde, a strand's gloss.
        Role::Wisp => PMat::lacquer(
            fair().map(|c| c * crate::scene::figure::cards::hair_look(0).albedo * 1.15),
            0.2,
            0.3,
        ),
        // r42:arcus-polish4 — his loose tips: his dark brown, a strand's gloss.
        // (Lighter than the hair's albedo: a lone strand at the outline catches the back
        // lights over its whole width, and at the hair's own value it vanished.)
        // (r42:arcus-polish6: `CANTUS_TUNE`'s `tip_albedo` while studied.)
        Role::WispDark => PMat::lacquer(
            lin3(hp::HAIR_DARK)
                .map(|c| c * crate::scene::figure::tune::tune("tip_albedo", TIP_ALBEDO)),
            0.2,
            0.3,
        ),
        Role::Eye => PMat::lacquer(lin3(hp::EYE_WHITE), 0.35, 0.08),
        Role::Iris => PMat::lacquer(lin3(hp::IRIS), 0.35, 0.08),
        // (r42:arcus-hair: a little lighter than the Drawn picture's — near black, each lash
        // a sample wide read as a black blob over the eye at 200 %.)
        Role::Lash => PMat::matte(lin3(hp::LASH).map(|c| c * 1.8)),
        Role::Nail(i) => {
            PMat::lacquer(lin3(hp::nail(i).albedo).map(|c| c * 0.5), 0.18, 0.22).with(Tex::Skin)
        }
        // r42:arcus-upper — the cards: her hair fair, his dark; brows a shade darker than
        // the hair; lashes near black.
        Role::Card(k) => {
            let woman = k < 3;
            let base = match (k % 3, woman) {
                (0, true) => hp::HAIR_FAIR,
                (0, false) => hp::HAIR_DARK, // r42:arcus-polish4
                (1, true) => hp::BROW_FAIR,
                (1, false) => hp::HAIR_DARK,
                _ => hp::LASH,
            };
            let base = match k % 3 {
                2 => lin3(base).map(|c| c * 1.8),
                0 if woman => fair().map(|c| c * crate::scene::figure::cards::hair_look(0).albedo),
                0 => lin3(base).map(|c| c * crate::scene::figure::cards::hair_look(k / 3).albedo),
                _ => lin3(base),
            };
            PMat::lacquer(base, 0.06, 0.45).with(Tex::Card(k))
        }
        Role::Shell(0) | Role::Scalp(0) => PMat::lacquer(fair(), 0.1, 0.42).with(Tex::Hair),
        Role::Shell(_) | Role::Scalp(_) => PMat::matte(lin3(hp::HAIR_DARK)),
    }
}

/// r42:arcus-upper — the middle of hair card `k`'s head of hair (its vertices' mean).
fn middle(tris: &[(Role, crate::scene::bowed_mesh::Tris)], k: u8) -> V3 {
    let (mut sum, mut n) = (V3::default(), 0usize);
    for (_, t) in tris.iter().filter(|(r, _)| *r == Role::Card(k)) {
        for &p in &t.pts {
            sum = sum + of(p);
            n += 1;
        }
    }
    sum * (1.0 / n.max(1) as f32)
}

/// `b` added to `g` (its triangles after the room's).
pub fn build(g: &mut Geo, b: Bowed, h: crate::scene::bowed_hold::Holder) {
    build_lit(g, b, h, None);
}

/// [`build`] under the concert light `cc` (r42:arcus-polish3: its varnish).
pub fn build_lit(
    g: &mut Geo,
    b: Bowed,
    h: crate::scene::bowed_hold::Holder,
    cc: Option<&crate::scene::figure::concert::Concert>,
) {
    let m = mesh_for(b, h); // r42:arcus-upper
    let mut ids: HashMap<Role, MatId> = HashMap::new();
    let mut id = |g: &mut Geo, r: Role| *ids.entry(r).or_insert_with(|| g.mat(pmat(b, r, cc)));
    let uvm = |uv: [f32; 2]| [uv[0] / 100.0, uv[1] / 100.0];
    // (r42:arcus-polish2: the top's edge band's coordinates are its outline's columns, not
    // the top's place: it takes no rosin.)
    let mut top_edge = None;
    for (role, p) in &m.patches {
        let mi = if *role == Role::Top && p.rows == 2 {
            *top_edge.get_or_insert_with(|| {
                let mut t = pmat(b, Role::Top, cc);
                if let Tex::Tonewood { rosin, .. } = &mut t.tex {
                    *rosin = [0.0; 4];
                }
                g.mat(t)
            })
        } else {
            id(g, *role)
        };
        for cell in p.cells() {
            let v: [_; 4] = std::array::from_fn(|k| {
                vt(of(p.pts[cell[k]]), of(p.nrm[cell[k]]), uvm(p.uv[cell[k]]))
            });
            let area = |a: V3, b: V3, c: V3| (b - a).cross(c - a).len();
            if area(v[0].p, v[1].p, v[2].p) > 1e-10 {
                g.tri(v[0], v[1], v[2], mi);
            }
            if area(v[0].p, v[2].p, v[3].p) > 1e-10 {
                g.tri(v[0], v[2], v[3], mi);
            }
        }
    }
    // r42:arcus-upper — the realistic figures' triangles; their corners' detail (the skin's
    // albedo, the hair's strands) carried as the texture coordinates.
    let scalp = m
        .tris
        .iter()
        .find_map(|(r, _)| match r {
            Role::Skin(i) => Some(lin3(hp::real_skin(*i).albedo).map(|c| c * 0.48)),
            _ => None,
        })
        .unwrap_or([0.4, 0.3, 0.25]);
    for (role, t) in &m.tris {
        let mi = match role {
            Role::Skin(i) => {
                // Seen per pixel under the key a skin's albedo reads lighter than drawn:
                // a little less of it, so a fair face is not chalk.
                let a = lin3(hp::real_skin(*i).albedo).map(|c| c * 0.48);
                // (r42:arcus-upper: a soft sheen on the nose and the forehead, as skin has —
                // the owner's review: porcelain.)
                g.mat(PMat::lacquer(a, 0.09, 0.38).with(Tex::Skin))
            }
            Role::Hair => g.mat(PMat {
                dark: scalp,
                ..PMat::lacquer(lin3(hp::HAIR), 0.1, 0.42).with(Tex::Hair)
            }),
            Role::HairFair => g.mat(PMat {
                dark: scalp,
                ..PMat::lacquer(
                    fair().map(|c| c * crate::scene::figure::cards::hair_look(0).albedo),
                    0.1,
                    0.42,
                )
                .with(Tex::Hair)
            }),
            // The scalp under the cards in the cards' own finish (no streaks of its own:
            // where it showed between them it read as patches of another hair).
            // (r42:arcus-hair: the hair's colour, lit as the cards over it are, thinning out
            // at its edge by the hairline.)
            Role::Scalp(w) => {
                let hair = if *w == 0 { fair() } else { lin3(hp::HAIR_DARK) };
                let deep = crate::scene::figure::cards::hair_look(*w).scalp;
                // (r42:arcus-polish4: the skin under it, `dark`, through his short sides.)
                g.mat(PMat {
                    dark: scalp,
                    ..PMat::lacquer(hair.map(|c| c * deep), 0.06, 0.45).with(Tex::Scalp(*w))
                })
            }
            // (r42:arcus-hair: the eyes from their texture — the iris, its pupil, the white —
            // wet: a catch-light from the key.)
            Role::Eye | Role::Iris => g.mat(PMat::lacquer([1.0; 3], 0.6, 0.04).with(Tex::Eye)),
            // The thick shell is the Drawn picture's (here the cards over the scalp).
            Role::Shell(_) => continue,
            // r42:arcus-polish2 — his shirt and tie with their shade under the collar; the
            // tie's silk softer (its tight highlights read as wet plastic at 200 %).
            Role::Shirt => g.mat(PMat::matte(lin3(hp::SHIRT)).with(Tex::Shirt)),
            // (r42:arcus-polish4: silk's faint sheen, so the knot's faces read by value.)
            Role::Tie => g.mat(PMat::lacquer(lin3(hp::TIE), 0.08, 0.35).with(Tex::Shirt)),
            // r42:arcus-polish2 — her earrings polished gold, a little rough, so the key's
            // glint on the bead spreads over a pixel or two (the owner's list: "the
            // earring's glint"; as brass fittings they read as a dull brown dot).
            Role::Brass => g.mat(PMat::metal([0.95, 0.72, 0.38], 0.16)),
            r => id(g, *r),
        };
        // r42:arcus-upper — a hair card is lit as the head of hair it belongs to (each card's
        // own normal showed as light and dark patches, the owner's review): its normal the
        // way out from the hair's middle, one smooth light over every card.
        // (r42:arcus-hair: the scalp under them lit from the same middle — by its own normal
        // its strip along the hairline, where the cards end short, faced the key more and
        // read as a pale band.)
        let radial = match role {
            Role::Card(k) if k % 3 == 0 => Some(middle(&m.tris, *k)),
            Role::Scalp(w) => Some(middle(&m.tris, *w * 3)),
            _ => None,
        };
        for tri in &t.idx {
            let [a, b2, c] = tri.map(|i| i as usize);
            let w = [a, b2, c].map(|i| {
                let p = of(t.pts[i]);
                let n = radial.map_or(of(t.nrm[i]), |m| (p - m).norm());
                Vert {
                    uv2: t.uv.get(i).copied().unwrap_or([-1.0, -1.0]),
                    ..vt(p, n, t.detail[i])
                }
            });
            let fnrm = (w[1].p - w[0].p).cross(w[2].p - w[0].p);
            if fnrm.len() > 1e-10 {
                g.tri(w[0], w[1], w[2], mi);
            }
        }
    }
    for (role, poly) in &m.polys {
        let mi = id(g, *role);
        let n = of(poly.n).norm();
        let a = perp(n);
        let c = n.cross(a).norm();
        let pts: Vec<V3> = poly.pts.iter().map(|&q| of(q)).collect();
        let flat: Vec<(f32, f32)> = pts.iter().map(|q| (q.dot(a), q.dot(c))).collect();
        for t in ear_clip(&flat) {
            let w = t.map(|i| vt(pts[i], n, [flat[i].0, flat[i].1]));
            g.tri(w[0], w[1], w[2], mi);
        }
    }
    for (role, r) in &m.rods {
        let mi = id(g, *role);
        g.rod(of(r.a), of(r.b), r.r / 100.0, mi);
        // r42:arcus-polish2 — the strings (dashed white and black down the fingerboard
        // at 100 %, the owner's review).
        if matches!(
            role,
            Role::Steel | Role::Wound | Role::Wisp | Role::WispDark
        ) && !g.wires.contains(&mi)
        {
            g.wires.push(mi);
        }
    }
}
