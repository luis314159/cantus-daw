//! r42:arcus-upper — the figure's left hand on the neck, drawn live (the owner,
//! 2026-11-24: with a musician or the mannequin there is no floating finger — its own hand
//! goes where the note is stopped). The hand sits in a *position* (its first finger's
//! semitone above the open string) and stays there while the notes fit under its four
//! fingers; a note outside it shifts the hand, gliding; the finger that plays lands on the
//! string at the stopped point, the others hover; an open string lifts them all. Vibrato
//! rocks the whole hand along the string. Also the stopped note's marker (a small chevron
//! in the theme's accent) when that is how the stop is shown.

use super::body::View;
use crate::kit::appearance::rgb;
use crate::kit::color::with_alpha;
use crate::kit::Brush;
use crate::scene::bowed::Rig;
use crate::scene::bowed_hold::{elbow, Arm};
use crate::scene::geo::V3;
use egui::{Color32, Pos2};

/// The semitones a hand's four fingers reach from its position (first finger at 0).
const SPAN: f32 = 5.0;

/// The semitones above the open string a stop at `st` (its share of the open length from
/// the bridge) plays.
pub fn semis(st: f32) -> f32 {
    if st >= 0.995 {
        0.0
    } else {
        -12.0 * st.max(1e-3).log2()
    }
}

/// The hand's position for a note `s` semitones up, from where it is (`at`): it stays
/// while the note lies under its fingers (an open string never moves it), else it shifts
/// so the note falls under its first or its last finger, whichever is nearer.
pub fn target(at: f32, s: f32) -> f32 {
    if s < 0.5 {
        return at;
    }
    // The note under the first finger at most, under the fourth at least.
    let lo = (s - SPAN).max(1.0);
    let hi = s.max(1.0);
    if at < lo - 0.25 {
        lo
    } else if at > hi + 0.25 {
        hi
    } else {
        at
    }
}

/// The hand glided toward `to` over `dt` seconds (a shift takes about a tenth of a second).
pub fn glide(at: f32, to: f32, dt: f32) -> f32 {
    let k = 1.0 - (-dt.clamp(0.0, 0.1) / 0.045).exp();
    let n = at + (to - at) * k;
    if (n - to).abs() < 0.01 {
        to
    } else {
        n
    }
}

/// The point `s` semitones up string `i`, `lift` cm off the string along its normal.
fn on_string(rig: &Rig, i: usize, s: f32, lift: f32) -> V3 {
    let [_, bridge, nut] = rig.strings[i];
    let st = (-s / 12.0).exp2();
    bridge + (nut - bridge) * st + rig.normal[i] * (rig.gauge[i] + lift)
}

/// What the left hand plays: the string, its stopped note (semitones; 0 open), the hand's
/// position, the vibrato's rock (cm along the string), the player's left (`side`).
pub struct Left<'a> {
    pub rig: &'a Rig,
    pub string: usize,
    pub semis: f32,
    pub at: f32,
    pub rock: f32,
    pub side: V3,
}

/// Where the finger that plays the note lands: on its string at the stopped point (rocked
/// by the vibrato); none on an open string.
pub fn playing_tip(l: &Left) -> Option<V3> {
    let up = (l.rig.strings[l.string][2] - l.rig.strings[l.string][1]).norm();
    (l.semis >= 0.5).then(|| on_string(l.rig, l.string, l.semis, 0.0) + up * l.rock)
}

/// The left arm from its shoulder to the hand on the neck; returns where the playing
/// fingertip is (none on an open string).
pub fn left_arm(b: &mut Brush, v: &View, arm: &Arm, l: &Left) -> Option<V3> {
    let rig = l.rig;
    let up = (rig.strings[l.string][2] - rig.strings[l.string][1]).norm();
    let nz = rig.pose.ez;
    let rock = up * l.rock;
    // The hand's middle string line at its first finger, the palm under the neck there.
    let mid = (l.string as f32).clamp(1.0, 2.0) as usize;
    let first = on_string(rig, mid, l.at, 0.0) + rock;
    let palm = first - nz * 2.6 + l.side * 1.8 - up * 1.2;
    let wrist = palm - nz * 3.2 + l.side * 1.2 - up * 2.4;
    let el = elbow(arm.shoulder, wrist, arm.upper, arm.fore, arm.pole);
    let sleeve = rgb(arm.sleeve);
    let shade = rgb(arm.sleeve_shade);
    let (ps, pe, pw) = (v.p(arm.shoulder), v.p(el), v.p(wrist));
    let wu = 2.0 * arm.r.0 * v.scale(el);
    let wf = 2.0 * arm.r.1 * v.scale(wrist);
    b.segment(ps, pe, wu, shade, true);
    b.segment(ps, pe, wu * 0.82, sleeve, true);
    b.segment(pe, pw, wf, shade, true);
    b.segment(pe, pw, wf * 0.82, sleeve, true);
    let skin = rgb(arm.hand);
    let skin_shade = rgb(arm.hand_shade);
    let wh = 2.0 * 3.4 * v.scale(palm);
    b.segment(pw, v.p(palm), wh * 1.04, skin_shade, true);
    b.segment(pw, v.p(palm), wh * 0.84, skin, true);
    // The four fingers: the one that plays the note lands on its string; the others hover
    // over theirs, a little off the board.
    let off = l.semis - l.at;
    let playing = if l.semis < 0.5 {
        None
    } else {
        Some(((off + 0.5) / (SPAN / 4.0)).floor().clamp(0.0, 3.0) as usize)
    };
    let mut tip_at = None;
    let wfi = 1.7 * v.scale(palm);
    for j in 0..4 {
        let s = l.at + j as f32 * (SPAN / 4.0) * 0.95;
        let knuckle = on_string(rig, mid, s, 0.0) + rock - nz * 1.2 + l.side * 2.4;
        let (tip, lifted) = if playing == Some(j) {
            (playing_tip(l).unwrap_or(knuckle), false)
        } else {
            (on_string(rig, l.string, s, 1.1) + rock, true)
        };
        let knee = knuckle.lerp(tip, 0.5) + nz * 1.6 + l.side * 0.6;
        let col = if lifted { with_alpha(skin, 0.92) } else { skin };
        for (a, e) in [(knuckle, knee), (knee, tip)] {
            b.segment(v.p(a), v.p(e), wfi * 1.12, skin_shade, true);
            b.segment(v.p(a), v.p(e), wfi * 0.84, col, true);
        }
        if !lifted {
            tip_at = Some(tip);
        }
    }
    // The thumb under the neck, on its far side.
    let t0 = palm - up * 1.0;
    let t1 = first - nz * 2.2 - l.side * 1.6 + up * 0.6;
    b.segment(v.p(t0), v.p(t1), wfi * 1.2, skin_shade, true);
    b.segment(v.p(t0), v.p(t1), wfi * 0.92, skin, true);
    tip_at
}

/// The stopped note's marker: a small chevron pointing at the string at `at`, from the
/// side `toward` (screen), in `accent`, outlined dark so it reads on light wood and on the
/// ebony alike.
pub fn marker(b: &mut Brush, v: &View, at: V3, size: f32, accent: Color32, edge: Color32) {
    let p = v.p(at);
    let s = size;
    let q = |x: f32, y: f32| Pos2::new(p.x + s * x, p.y - s * y);
    // Two halves, each convex: the outline, then the accent inside it.
    for (tip, l, notch, r) in [
        (q(0.0, 0.0), q(-0.62, 1.05), q(0.0, 0.72), q(0.62, 1.05)),
        (q(0.0, 0.17), q(-0.45, 0.94), q(0.0, 0.69), q(0.45, 0.94)),
    ] {
        let col = if tip == p { edge } else { accent };
        b.polygon(&[tip, l, notch], col);
        b.polygon(&[tip, notch, r], col);
    }
}
