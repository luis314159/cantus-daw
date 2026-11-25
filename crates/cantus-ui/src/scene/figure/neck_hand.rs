//! r42:arcus-upper — a realistic figure's left hand on a violin's or a viola's neck, posed
//! from data (the owner, 2026-11-24: "the violin's left hand is a disaster"; ADR 0159).
//!
//! The poses are authored offline (`tools/arcus-figures/left_hand.py`): each one solved on
//! the figure's own MakeHuman hand — three phalanges a finger, the thumb's CMC, MCP and IP —
//! against the rules a violinist's hand keeps (the neck in the crook between the thumb and
//! the index finger's base with a gap under the palm; the thumb along the neck's side
//! opposite the first and second fingers, its tip about level with the fingerboard; each
//! finger rising from the neck's far side and arching high over the fingerboard, MCP
//! 45–70°, PIP 70–100°, DIP 20–50°, the tip coming down nearly upright; the others hovering
//! curved; the wrist straight; the palm toward the player), rendered beside the owner's
//! photographs and iterated there. What is stored is the result: the wrist (in the
//! instrument's frame) and every bone's turn, a row per figure, instrument, string, playing
//! finger (or none) and hand position. At run time the rows either side of the hand's
//! position are blended (turns by normalised quaternion), the bones carried down the chain
//! from the figure's rest skeleton (the same forward kinematics the tool runs), and the hand
//! moved by the few millimetres that put the playing finger's tip exactly on its stop.
//!
//! The table is `assets/figures/left_hand.bin` (little-endian): `CLHT`, a `u16` version, a
//! `u16` count of positions, the positions (`f32` semitones), then the rows in order
//! (figure: woman, man) × (violin, viola) × string (G … E) × finger (index … little, then
//! none) × position; a row is 70 `i16`: the wrist (0.01 cm, along the instrument's x, y,
//! z from its origin), the forearm's way from the wrist toward the elbow (a unit vector in
//! the instrument's frame × 32767: r42:arcus-hand, version 2 — the wrist turned a little
//! toward the little finger, as the photographs show it), the hand's turn in the
//! instrument's frame and each of the fifteen finger bones' turns against the hand's
//! (thumb, index … little; base to tip), each a quaternion (w, x, y, z) × 32767.

use super::rig::Rot;
use crate::scene::bowed::Pose;
use crate::scene::bowed_human::{self as hu, Figure, Who};
use crate::scene::geo::{v, V3};
use std::sync::OnceLock;

static TABLE: &[u8] = include_bytes!("../../../assets/figures/left_hand.bin");

/// A turn as a unit quaternion (w, x, y, z).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Quat(pub [f32; 4]);

impl Quat {
    /// The turn as a rotation.
    pub fn rot(&self) -> Rot {
        let [w, x, y, z] = self.0;
        Rot {
            x: v(
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y + w * z),
                2.0 * (x * z - w * y),
            ),
            y: v(
                2.0 * (x * y - w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z + w * x),
            ),
            z: v(
                2.0 * (x * z + w * y),
                2.0 * (y * z - w * x),
                1.0 - 2.0 * (x * x + y * y),
            ),
        }
    }

    /// `self` and `o` blended by `t` (normalised; the shorter way).
    pub fn nlerp(&self, o: &Quat, t: f32) -> Quat {
        let dot: f32 = self.0.iter().zip(o.0.iter()).map(|(a, b)| a * b).sum();
        let s = if dot < 0.0 { -1.0 } else { 1.0 };
        let mut q = [0.0f32; 4];
        for (i, x) in q.iter_mut().enumerate() {
            *x = self.0[i] * (1.0 - t) + o.0[i] * s * t;
        }
        let n = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
        Quat(q.map(|x| x / n))
    }
}

/// One stored pose: the wrist in the instrument's frame, the hand's turn in it, each
/// finger bone's turn against the hand's.
#[derive(Copy, Clone, Debug, Default)]
pub struct Row {
    pub wrist: V3,
    /// r42:arcus-hand — the forearm's way from the wrist (the instrument's frame).
    pub fore: V3,
    pub hand: Quat,
    pub bones: [Quat; 15],
}

/// The hand's bones' world turns (from the figure's rest): the hand's, then each finger
/// bone's (thumb, index … little; base to tip).
#[derive(Copy, Clone, Debug)]
pub struct HandRots {
    pub hand: Rot,
    pub bones: [Rot; 15],
}

/// The hand placed: the wrist, each finger's joints (its base, middle, last joint, tip),
/// the thumb's (its base, knuckle, joint, tip), and the bones' turns.
#[derive(Copy, Clone, Debug)]
pub struct HandPlace {
    pub wrist: V3,
    /// r42:arcus-hand — the forearm's way from the wrist toward the elbow (unit, world).
    pub fore: V3,
    pub fingers: [[V3; 4]; 4],
    pub thumb: [V3; 4],
    pub rots: HandRots,
}

impl HandPlace {
    /// Everything moved by `d`.
    pub fn shift(&mut self, d: V3) {
        self.wrist = self.wrist + d;
        for f in &mut self.fingers {
            for p in f {
                *p = *p + d;
            }
        }
        for p in &mut self.thumb {
            *p = *p + d;
        }
    }
}

const ROW: usize = 70;
const STRINGS: usize = 4;
const FINGERS: usize = 5;

struct Table {
    at: Vec<f32>,
    rows: Vec<Row>,
}

fn parse(b: &[u8]) -> Option<Table> {
    if b.len() < 8 || &b[..4] != b"CLHT" {
        return None;
    }
    let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    if u16_at(4) != 2 {
        return None;
    }
    let n = u16_at(6) as usize;
    let mut o = 8;
    let mut at = Vec::with_capacity(n);
    for _ in 0..n {
        at.push(f32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?));
        o += 4;
    }
    let count = 2 * 2 * STRINGS * FINGERS * n;
    if b.len() < o + count * ROW * 2 || n == 0 {
        return None;
    }
    let i16_at = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]) as f32;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let q = |k: usize| {
            let p = o + 2 * (6 + 4 * k);
            Quat([0, 1, 2, 3].map(|i| i16_at(p + 2 * i) / 32767.0))
        };
        let mut bones = [Quat::default(); 15];
        for (k, bq) in bones.iter_mut().enumerate() {
            *bq = q(k + 1);
        }
        rows.push(Row {
            wrist: v(i16_at(o), i16_at(o + 2), i16_at(o + 4)) * 0.01,
            fore: v(i16_at(o + 6), i16_at(o + 8), i16_at(o + 10)) * (1.0 / 32767.0),
            hand: q(0),
            bones,
        });
        o += ROW * 2;
    }
    Some(Table { at, rows })
}

fn table() -> Option<&'static Table> {
    static T: OnceLock<Option<Table>> = OnceLock::new();
    // (r42:arcus-hands: `CANTUS_FIGURE_DIR`'s `left_hand.bin` while the table is being
    // solved, as `bowed_human` reads the figures from there — DEVELOPING §4.)
    T.get_or_init(|| {
        let dir = std::env::var_os("CANTUS_FIGURE_DIR")
            .and_then(|d| std::fs::read(std::path::Path::new(&d).join("left_hand.bin")).ok());
        parse(dir.as_deref().unwrap_or(TABLE))
    })
    .as_ref()
}

/// The pose for `who` on a violin (or a viola), `string` (G 0 … E 3), the finger that
/// plays (none: an open string), the hand's position `at` (semitones; the rows either side
/// blended, clamped at the table's ends). None without a table.
pub fn row(who: Who, viola: bool, string: usize, playing: Option<usize>, at: f32) -> Option<Row> {
    let t = table()?;
    let n = t.at.len();
    let w = match who {
        Who::Woman => 0,
        Who::Man => 1,
    };
    let f = playing.map_or(4, |p| p.min(3));
    let base = (((w * 2 + viola as usize) * STRINGS + string.min(3)) * FINGERS + f) * n;
    let (i, k) = match t.at.iter().position(|&a| a > at) {
        None => (n - 1, 0.0),
        Some(0) => (0, 0.0),
        Some(j) => (j - 1, (at - t.at[j - 1]) / (t.at[j] - t.at[j - 1])),
    };
    let a = &t.rows[base + i];
    if k <= 0.0 {
        return Some(*a);
    }
    let b = &t.rows[base + i + 1];
    let mut bones = [Quat::default(); 15];
    for (j, q) in bones.iter_mut().enumerate() {
        *q = a.bones[j].nlerp(&b.bones[j], k);
    }
    Some(Row {
        wrist: a.wrist.lerp(b.wrist, k),
        fore: a.fore.lerp(b.fore, k).norm(),
        hand: a.hand.nlerp(&b.hand, k),
        bones,
    })
}

/// `row` placed on `fig`'s left hand in the instrument's frame `pose`: the bones carried
/// down each finger from the rest skeleton (each bone's head where its parent's turn takes
/// it), so the joints are the skinned hand's own.
pub fn place(fig: &Figure, row: &Row, pose: &Pose) -> HandPlace {
    let o = hu::side(0);
    let frame = Rot {
        x: pose.ex,
        y: pose.ey,
        z: pose.ez,
    };
    let rh = frame.mul(&row.hand.rot());
    let wrist = pose.o + frame.apply(row.wrist);
    let hb = &fig.bones[o + hu::HAND];
    let mut bones = [Rot::I; 15];
    let mut chains = [[V3::default(); 4]; 5];
    for f in 0..5 {
        let mut at = wrist + rh.apply(fig.bones[o + hu::finger(f, 0)].head - hb.head);
        chains[f][0] = at;
        for k in 0..3 {
            let b = &fig.bones[o + hu::finger(f, k)];
            let r = rh.mul(&row.bones[f * 3 + k].rot());
            bones[f * 3 + k] = r;
            let next = if k < 2 {
                fig.bones[o + hu::finger(f, k + 1)].head
            } else {
                b.tail
            };
            at = at + r.apply(next - b.head);
            chains[f][k + 1] = at;
        }
    }
    HandPlace {
        wrist,
        fore: frame.apply(row.fore).norm(),
        fingers: [chains[1], chains[2], chains[3], chains[4]],
        thumb: chains[0],
        rots: HandRots { hand: rh, bones },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quaternion_turns_as_its_rotation() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        // 90° about z: x goes to y.
        let r = Quat([h, 0.0, 0.0, h]).rot();
        assert!((r.apply(v(1.0, 0.0, 0.0)) - v(0.0, 1.0, 0.0)).len() < 1e-5);
        let id = Quat([1.0, 0.0, 0.0, 0.0]);
        let half = id.nlerp(&Quat([h, 0.0, 0.0, h]), 0.5).rot();
        let p = half.apply(v(1.0, 0.0, 0.0));
        assert!((p.x - p.y).abs() < 1e-4 && p.x > 0.0);
    }
}
