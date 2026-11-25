//! r42:arcus-upper — the stylised musician's and the wooden mannequin's poser on the
//! **PlayRig** contract (ADR 0159; the owner, 2026-11-24: all four figures one module).
//!
//! The realistic figures are posed bone by bone from a MakeHuman skeleton (`play::pose`);
//! the stylised ones are a handful of joints their shells hang from (`bowed_figure_parts`).
//! Both read the same contract: the facing, the head (a chin rest's point or where the eyes
//! go, and its turn), the seat (hung from the head, a chair, standing), the legs and the
//! left hand's rest. What is the instrument's and not the figure's — where its neck is, the
//! chair's place — comes beside the rig ([`Beside`]).
//!
//! The stylised figure's own proportions (a little larger head; a seated adult's trunk) are
//! here, the numbers its pictures were drawn with: those pictures are unchanged.

use super::play::{Hand, HeadAt, Legs, PlayRig, Seat};
use crate::scene::bowed_figure::{Skel, HEAD};
use crate::scene::geo::{v, V3};

const UP: V3 = V3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// What the instrument says beside the rig: where the left hand holds its neck, the neck's
/// way, the fingerboard's up, the side the hand comes from, the fingerboard's top there,
/// and whether a chair stands under the figure (none: the instrument's own seat).
#[derive(Copy, Clone, Debug)]
pub struct Beside {
    pub grip: V3,
    pub nd: V3,
    pub nz: V3,
    pub nx: V3,
    pub top: V3,
    pub chair: bool,
}

/// The stylised figure's joints for `rig`.
///
/// - **Hung from the head** (a chin rest, `HeadAt::Jaw`): the head over the jaw, the neck,
///   the shoulders, the chest and the pelvis below it, the legs forward from a chair.
/// - **On a chair** (`Seat::Chair`): the pelvis over its place, the chest, the neck and the
///   head above it (a cellist's back against the instrument), the legs to the rig's knees
///   and ankles.
///
/// The left arm reaches the rest's wrist, its elbow toward the rest's pole (a violinist's
/// upper arm 28 cm and forearm 25, a cellist's 29 and 27: the pictures' own).
pub fn skel(rig: &PlayRig, at: &Beside) -> Skel {
    let (f, r) = (rig.facing, rig.right);
    let down = -UP;
    let (hf, hu) = (rig.head.face, rig.head.up);
    let (lw, pole) = match rig.hands[0] {
        Hand::Rest { wrist, pole } => (wrist, pole),
        Hand::Aim(a) => (a.wrist, down),
        Hand::Grip(_, pole) => (at.grip, pole),
    };
    let (head, neck, ls, rs, chest, pelvis, chin, lens) = match (rig.head.at, rig.seat) {
        (HeadAt::Jaw(chin), _) => {
            let hr = hf.cross(hu).norm();
            let (a, bb, c) = HEAD;
            let head = chin + hu * (bb * 0.78) - hf * (c * 0.15) + hr * (a * 0.5);
            let neck = head + down * 15.5 - f * 3.0 + r * 4.0;
            let ls = neck - r * 16.0 + down * 3.5;
            let rs = neck + r * 16.0 + down * 3.5;
            let chest = neck + down * 15.0 - f * 2.0;
            let pelvis = chest + down * 27.0 - f * 2.0;
            (head, neck, ls, rs, chest, pelvis, chin, (28.0, 25.0))
        }
        (_, Seat::Chair { at: pelvis, .. }) | (_, Seat::Standing { at: pelvis, .. }) => {
            let chest = pelvis + UP * 27.0 + f * 3.0;
            let neck = chest + UP * 15.0 - f * 1.0;
            let head = neck + UP * 16.5 + r * 5.0 + f * 1.0;
            // The shoulders' joints a hand's breadth behind the chest's front, as a person's
            // are.
            let ls = neck - r * 16.0 + down * 3.5 - f * 4.0;
            let rs = neck + r * 16.0 + down * 3.5 - f * 4.0;
            (
                head,
                neck,
                ls,
                rs,
                chest,
                pelvis,
                head - hu * 9.0,
                (29.0, 27.0),
            )
        }
        (HeadAt::Eyes(eyes), Seat::FromHead { .. }) => {
            // The eyes' place without a chin rest: the trunk straight under it.
            let head = eyes;
            let neck = head + down * 16.5 - r * 5.0 - f * 1.0;
            let chest = neck + down * 15.0 + f * 1.0;
            let pelvis = chest + down * 27.0 - f * 3.0;
            let ls = neck - r * 16.0 + down * 3.5 - f * 4.0;
            let rs = neck + r * 16.0 + down * 3.5 - f * 4.0;
            (
                head,
                neck,
                ls,
                rs,
                chest,
                pelvis,
                head - hu * 9.0,
                (29.0, 27.0),
            )
        }
    };
    let le = super::rig::two_bone(ls, lw, lens.0, lens.1, pole);
    let hips = [pelvis - r * 9.5, pelvis + r * 9.5];
    let (knees, ankles) = match rig.legs {
        Legs::To { knees, ankles } => (knees, ankles),
        Legs::Forward => {
            let knees = [
                hips[0] + f * 42.0 - r * 3.0 + v(0.0, -2.0, 0.0),
                hips[1] + f * 42.0 + r * 3.0 + v(0.0, -2.0, 0.0),
            ];
            let ankles = [
                v(knees[0].x, 9.0, knees[0].z) + f * 5.0,
                v(knees[1].x, 9.0, knees[1].z) + f * 5.0,
            ];
            (knees, ankles)
        }
        Legs::Straight => {
            let ankles = hips.map(|h| v(h.x, 9.0, h.z));
            (hips.map(|h| h.lerp(v(h.x, 9.0, h.z), 0.5)), ankles)
        }
    };
    Skel {
        f,
        r,
        down,
        head,
        hf,
        hu,
        neck,
        chest,
        pelvis,
        ls,
        rs,
        le,
        lw,
        grip: at.grip,
        hips,
        knees,
        ankles,
        // The chair a little behind the pelvis, turned with the figure.
        seat: at
            .chair
            .then(|| (v(pelvis.x, 0.0, pelvis.z) - f * 6.0, f.x.atan2(f.z))),
        nd: at.nd,
        nz: at.nz,
        nx: at.nx,
        top: at.top,
        chin,
    }
}
