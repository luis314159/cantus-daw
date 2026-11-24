//! r42:arcus-upper — what holds an Arcus instrument in its picture (the owner, 2026-11-24):
//! **nothing** (the violin or the viola in the air at the playing angle, its soft shadow on
//! the wall behind; the cello or the bass on its endpin beside the empty chair, as before),
//! **a stand** (a violin's or a viola's instrument stand, upright; a cello's or a bass's floor
//! stand behind it), **a musician** (a stylised, warm player — eyes closed in the music,
//! never a realistic face), or **a mannequin** (an artist's wooden mannequin, faceless as
//! such a figure is). The same choice for the four instruments; the figure's tones (skin,
//! clothes, the mannequin's wood) are the viewer's to set.
//!
//! What is held is the picture only — never heard. The bow and the right arm that draws it
//! move with the sound, so they are drawn live (`views/inst/arcus_draw_body.rs`) from
//! [`arm`]; everything else is baked with the instrument.

use super::bowed::{Bowed, Dims, Pose};
use super::bowed_mesh::{Mesh, Rod, Role};
use super::geo::{v, V3};
use std::f32::consts::TAU;

/// What holds the instrument (stored by its word in the settings).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Hold {
    #[default]
    Air,
    Stand,
    Player,
    Mannequin,
}

impl Hold {
    pub const ALL: [Hold; 4] = [Hold::Air, Hold::Stand, Hold::Player, Hold::Mannequin];

    /// The stored word (forever once shipped).
    pub fn key(self) -> &'static str {
        match self {
            Hold::Air => "air",
            Hold::Stand => "stand",
            Hold::Player => "player",
            Hold::Mannequin => "mannequin",
        }
    }

    pub fn from_key(k: &str) -> Option<Hold> {
        Self::ALL.iter().copied().find(|h| h.key() == k)
    }

    /// Its name's string key.
    pub fn label_key(self) -> &'static str {
        match self {
            Hold::Air => "arcus_upper.hold.air",
            Hold::Stand => "arcus_upper.hold.stand",
            Hold::Player => "arcus_upper.hold.player",
            Hold::Mannequin => "arcus_upper.hold.mannequin",
        }
    }

    /// A figure holds it (a musician or the mannequin).
    pub fn figure(self) -> bool {
        matches!(self, Hold::Player | Hold::Mannequin)
    }
}

/// How the stopped note is shown on the string (a preference of this machine): the
/// fingertip (with a figure, its own left hand), a small marker in the theme's accent, or
/// nothing.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum StopShow {
    #[default]
    Finger,
    Marker,
    None,
}

impl StopShow {
    pub const ALL: [StopShow; 3] = [StopShow::Finger, StopShow::Marker, StopShow::None];

    /// Its name's string key.
    pub fn label_key(self) -> &'static str {
        match self {
            StopShow::Finger => "arcus_upper.stop.finger",
            StopShow::Marker => "arcus_upper.stop.marker",
            StopShow::None => "arcus_upper.stop.none",
        }
    }
}

/// What holds the instrument and how it looks: the musician's skin and clothes, the
/// mannequin's wood (indices into `kit::skin::scene::hold`).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(default)]
pub struct Holder {
    pub hold: Hold,
    pub skin: u8,
    pub cloth: u8,
    pub wood: u8,
}

impl Holder {
    pub fn of(hold: Hold) -> Holder {
        Holder {
            hold,
            ..Holder::default()
        }
    }
}

/// Where `b` stands when `h` holds it: a violin or a viola on its stand upright and leant
/// back a little (its end button ~70 cm over the floor); otherwise its own pose.
pub fn pose(b: Bowed, h: Holder) -> Pose {
    let d = b.dims();
    if d.held && h.hold == Hold::Stand {
        let rise = if b == Bowed::Viola { 66.0 } else { 70.0 };
        return standing(d, rise, 16.0, 10.0);
    }
    Pose::of(b)
}

/// The cello's pose made for a stand: `rise` from the floor to the lower edge along the
/// body, leant back `lean`°, rolled `roll`°.
fn standing(d: &Dims, rise: f32, lean: f32, roll: f32) -> Pose {
    let (sl, cl) = lean.to_radians().sin_cos();
    let (sr, cr) = roll.to_radians().sin_cos();
    let rot = |p: V3| v(p.x * cr + p.y * sr, -p.x * sr + p.y * cr, p.z);
    let ex = rot(v(1.0, 0.0, 0.0));
    let ey = rot(v(0.0, cl, -sl));
    let ez = rot(v(0.0, sl, cl));
    let o = ey * rise + ez * (d.rib_lo * 0.5);
    Pose { o, ex, ey, ez }
}

/// What holds `b` in `pose`, added to `m`: the stand, or the figure (its right arm is
/// the live picture's). Nothing for [`Hold::Air`].
pub fn build(m: &mut Mesh, b: Bowed, pose: &Pose, h: Holder) {
    let d = b.dims();
    match h.hold {
        Hold::Air => {}
        Hold::Stand if d.held => upper_stand(m, d, pose),
        Hold::Stand => floor_stand(m, d, pose),
        Hold::Player | Hold::Mannequin => super::bowed_figure::build(m, b, pose, h),
    }
}

/// More points for the camera to fit: the figure's head and shoulders.
pub fn fit(b: Bowed, h: Holder) -> Vec<V3> {
    if !h.hold.figure() {
        return Vec::new();
    }
    super::bowed_figure::fit(b, &pose(b, h))
}

/// The right arm the live picture draws to the bow's frog: the shoulder, the upper arm's
/// and the forearm's lengths, the way the elbow falls, its colours (sleeve, its shade;
/// the hand, its shade).
#[derive(Copy, Clone, Debug)]
pub struct Arm {
    pub shoulder: V3,
    pub upper: f32,
    pub fore: f32,
    pub pole: V3,
    pub sleeve: [u8; 3],
    pub sleeve_shade: [u8; 3],
    pub hand: [u8; 3],
    pub hand_shade: [u8; 3],
    /// The upper arm's and the forearm's thickness, cm.
    pub r: (f32, f32),
    /// The mannequin's ball joints' colour (none for a person).
    pub joints: Option<[u8; 3]>,
    /// The figure's frame: facing, its right, down (the hands are posed in it).
    pub f: V3,
    pub right: V3,
    pub down: V3,
}

/// The figure's skeleton for `b` held by `h`, worked out once and kept (the live picture
/// asks for it every frame; at rest that must allocate nothing).
fn skel(b: Bowed, h: Holder) -> super::bowed_figure::Skel {
    use std::sync::Mutex;
    static KEPT: Mutex<Vec<((Bowed, Holder), super::bowed_figure::Skel)>> = Mutex::new(Vec::new());
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, s)) = kept.iter().find(|(k, _)| *k == (b, h)) {
        return *s;
    }
    let s = super::bowed_figure::skeleton(b, &pose(b, h));
    if kept.len() >= 16 {
        kept.remove(0);
    }
    kept.push(((b, h), s));
    s
}

/// `b`'s player's left arm when a figure holds it (its hand on the neck, live).
pub fn left_arm(b: Bowed, h: Holder) -> Option<Arm> {
    let a = arm(b, h)?;
    let sk = skel(b, h);
    // Where the elbow goes from the shoulder (the owner's photographs, 2026-11-24): a
    // violinist's down under the violin's body, toward the chest's middle line (its forearm
    // and the neck a narrow V); a cellist's out to the side at about the hand's height, a
    // little under it.
    let pole = if b.dims().held {
        sk.down + sk.r * 0.45 + sk.f * 0.15
    } else {
        -sk.r + sk.down * 0.4
    };
    Some(Arm {
        // From inside the shoulder, so the arm grows out of the body.
        shoulder: sk.ls + sk.r * 3.0 + sk.down * 1.0,
        upper: 28.0,
        fore: 26.0,
        pole,
        ..a
    })
}

/// `b`'s player's right arm when a figure holds it.
pub fn arm(b: Bowed, h: Holder) -> Option<Arm> {
    use crate::kit::skin::scene::hold as hp;
    if !h.hold.figure() {
        return None;
    }
    let sk = skel(b, h);
    let (sleeve, sleeve_shade, hand, hand_shade) = if h.hold == Hold::Player {
        let (c, s) = ((h.cloth as usize).min(3), (h.skin as usize).min(4));
        (
            hp::CLOTHS[c],
            hp::CLOTH_SHADES[c],
            hp::SKINS[s],
            hp::SKIN_SHADES[s],
        )
    } else {
        let w = (h.wood as usize).min(2);
        (
            hp::WOODS[w],
            hp::WOOD_JOINTS[w],
            hp::WOODS[w],
            hp::WOOD_JOINTS[w],
        )
    };
    Some(Arm {
        // From inside the shoulder; the elbow out to the player's right and down, at a
        // bowing arm's height.
        shoulder: sk.rs - sk.r * 3.0 + sk.down * 1.0,
        upper: 29.0,
        fore: 27.0,
        pole: if b.dims().held {
            // A violinist's bow arm: the elbow out to the right, a little under the shoulder.
            sk.down * 12.0 + sk.r * 18.0 + sk.f * 8.0
        } else {
            // A cellist's: the upper arm hanging out to the side, the elbow a little behind
            // and about level with the bow, the forearm forward and in to the frog.
            sk.r + sk.down * 0.5 - sk.f * 0.3
        },
        sleeve,
        sleeve_shade,
        hand,
        hand_shade,
        r: match (h.hold, b.dims().held) {
            (Hold::Player, true) => (3.3, 2.6),
            (Hold::Player, false) => (3.9, 3.1),
            _ => (2.8, 2.2),
        },
        joints: (h.hold == Hold::Mannequin).then(|| hp::WOOD_JOINTS[(h.wood as usize).min(2)]),
        f: sk.f,
        right: sk.r,
        down: sk.down,
    })
}

/// Where the elbow of a two-bone arm from `s` to `w` falls (lengths `a`, `b`), bent toward
/// `pole`.
pub fn elbow(s: V3, w: V3, a: f32, b: f32, pole: V3) -> V3 {
    let dv = w - s;
    let dist = dv.len().clamp(1.0, a + b - 0.5);
    let u = dv.norm();
    let x = (a * a - b * b + dist * dist) / (2.0 * dist);
    let hh = (a * a - x * x).max(0.0).sqrt();
    let q = (pole - u * pole.dot(u)).norm();
    s + u * x + q * hh
}

fn rod(m: &mut Mesh, role: Role, a: V3, b: V3, r: f32) {
    m.rods.push((role, Rod { a, b, r }));
}

/// A violin's or a viola's stand: three legs, a column, a padded cradle the lower bout sits
/// in, a padded fork behind the neck's root.
fn upper_stand(m: &mut Mesh, d: &Dims, pose: &Pose) {
    let back = -(d.rib_lo + d.arch_back) - 0.9;
    let width = d.body * 0.29;
    let c = pose.at(0.0, -0.9, -d.rib_lo * 0.5);
    let side = pose.ex * (width * 0.72);
    let up = pose.ey * (d.body * 0.13);
    rod(m, Role::Pad, c - side, c + side, 0.75);
    for s in [-1.0f32, 1.0] {
        let a = c + side * s;
        rod(m, Role::Pad, a, a + up - pose.ez * 0.3, 0.7);
    }
    let col_top = pose.at(0.0, -2.4, back * 0.55);
    let hub = v(col_top.x, 14.0, col_top.z);
    rod(m, Role::Stand, c, col_top, 0.55);
    rod(m, Role::Stand, col_top, hub, 0.85);
    let mut floor = legs(m, hub, 27.0, 0.6);
    let y_fork = d.body + d.neck * 0.18;
    let fork = pose.at(0.0, y_fork, back);
    let low = pose.at(0.0, d.body * 0.2, back);
    rod(m, Role::Stand, col_top, low, 0.45);
    rod(m, Role::Stand, low, fork, 0.45);
    let neck_w = d.board_w.1 * 0.62;
    for s in [-1.0f32, 1.0] {
        let tip = pose.at(s * neck_w, y_fork, d.overstand * 0.4);
        let el = pose.at(s * neck_w, y_fork, back);
        rod(m, Role::Stand, fork, el, 0.4);
        rod(m, Role::Pad, el, tip, 0.55);
    }
    m.foot = v(hub.x, 0.0, hub.z);
    floor.push(m.foot);
    m.shadows.push(floor);
}

/// A cello's or a bass's floor stand behind it: legs, a column up behind the back to a
/// padded yoke under the neck's heel, a padded arm against the lower bout's back. The
/// endpin stays on the floor in its rubber cup.
fn floor_stand(m: &mut Mesh, d: &Dims, pose: &Pose) {
    let k = d.body / 75.7;
    let back = -(d.rib_lo + d.arch_back) - 1.2 * k;
    let yoke = pose.at(0.0, d.body + d.neck * 0.12, back);
    let base = v(yoke.x, 0.0, yoke.z - 6.0 * k);
    let hub = v(base.x, 10.0 * k, base.z);
    rod(m, Role::Stand, hub, yoke, 1.0 * k);
    let mut floor = legs(m, hub, 34.0 * k, 0.8 * k);
    let neck_w = d.board_w.1 * 0.7;
    for s in [-1.0f32, 1.0] {
        let tip = pose.at(s * neck_w, d.body + d.neck * 0.12, d.overstand * 0.3);
        let el = pose.at(s * neck_w, d.body + d.neck * 0.12, back);
        rod(m, Role::Stand, yoke, el, 0.7 * k);
        rod(m, Role::Pad, el, tip, 1.0 * k);
    }
    // The lower arm: from the column to a padded bar across the lower bout's back.
    let t = 0.42;
    let on_col = hub + (yoke - hub) * t;
    let pad = pose.at(0.0, d.body * 0.3, back);
    rod(m, Role::Stand, on_col, pad, 0.7 * k);
    let across = pose.ex * (d.body * 0.16);
    rod(m, Role::Pad, pad - across, pad + across, 1.1 * k);
    floor.push(base);
    m.shadows.push(floor);
}

/// Three splayed legs from `hub` to the floor, rubber feet; their feet on the floor.
fn legs(m: &mut Mesh, hub: V3, reach: f32, r: f32) -> Vec<V3> {
    let mut floor = Vec::new();
    for k in 0..3 {
        let a = TAU * (k as f32 + 0.25) / 3.0;
        let f = v(hub.x + reach * a.cos(), 1.0, hub.z + reach * a.sin());
        rod(m, Role::Stand, hub, f, r);
        rod(m, Role::Pad, f, f - v(0.0, 1.0, 0.0), r * 1.6);
        floor.push(v(f.x, 0.0, f.z));
    }
    floor
}
