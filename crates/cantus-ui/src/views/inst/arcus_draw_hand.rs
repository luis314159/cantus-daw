//! r42:arcus-upper — the figure's left hand on the neck, drawn live (the owner,
//! 2026-11-24: with a musician or the mannequin there is no floating finger — its own hand
//! goes where the note is stopped). The hand sits in a *position* (its first finger's
//! semitone above the open string) and stays there while the notes fit under its four
//! fingers; a note outside it shifts the hand, gliding; the finger that plays lands on the
//! string at the stopped point, the others hover; an open string lifts them all. Vibrato
//! rocks the whole hand along the string. Also the stopped note's marker (a small chevron
//! in the theme's accent) when that is how the stop is shown.

use super::body::View;
use super::lit::{ball, tube, Lit};
use crate::kit::Brush;
use crate::scene::bowed::{Dims, Pose, Rig};
use crate::scene::bowed_hold::{elbow, Arm};
use crate::scene::geo::V3;
use egui::{Color32, Pos2};

/// The semitones a hand's four fingers reach from its position (first finger at 0): a
/// fourth under a chin, a minor third on a cello's or a bass's longer strings.
pub fn span(held: bool) -> f32 {
    if held {
        5.0
    } else {
        3.0
    }
}

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
pub fn target(at: f32, s: f32, span: f32) -> f32 {
    if s < 0.5 {
        return at;
    }
    // The note under the first finger at most, under the fourth at least.
    let lo = (s - span).max(1.0);
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
    /// What its four fingers reach ([`span`]).
    pub span: f32,
}

/// Where the finger that plays the note lands: on its string at the stopped point (rocked
/// by the vibrato); none on an open string.
pub fn playing_tip(l: &Left) -> Option<V3> {
    let up = (l.rig.strings[l.string][2] - l.rig.strings[l.string][1]).norm();
    (l.semis >= 0.5).then(|| on_string(l.rig, l.string, l.semis, 0.0) + up * l.rock)
}

/// The left arm's pose: its joints, the palm and the thumb (behind the neck), each
/// finger's knuckle (beside the neck, on its far side), middle joint and tip.
#[derive(Copy, Clone, Debug)]
pub struct LeftPose {
    pub elbow: V3,
    pub wrist: V3,
    pub palm: V3,
    pub thumb: (V3, V3),
    pub fingers: [(V3, V3, V3); 4],
    /// Which finger plays the note (none on an open string) and where its tip lands.
    pub playing: Option<usize>,
    pub tip: Option<V3>,
}

/// Where the left arm goes for `l`: the palm under the neck on the side away from the
/// fingerboard's face, the thumb behind it, the wrist straight in line with the forearm,
/// the elbow where `arm`'s pole puts it (out and down), the fingers curving over the neck
/// onto the strings.
pub fn left_pose(arm: &Arm, l: &Left) -> LeftPose {
    let rig = l.rig;
    let up = (rig.strings[l.string][2] - rig.strings[l.string][1]).norm();
    let nz = rig.pose.ez;
    let rock = up * l.rock;
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    let mid = (l.string as f32).clamp(1.0, 2.0) as usize;
    let first = on_string(rig, mid, l.at, 0.0) + rock;
    // The palm behind the neck (under it, toward the hand's side), not touching its back.
    let palm = first - nz * (2.9 * k) + l.side * (1.6 * k) - up * (1.2 * k);
    // The wrist straight: on the line from the elbow to the palm, a hand's length back.
    let mut wrist = palm - nz * 5.0 - up * 4.0 + l.side * 2.0;
    let mut el = elbow(arm.shoulder, wrist, arm.upper, arm.fore, arm.pole);
    for _ in 0..2 {
        wrist = palm + (el - palm).norm() * (7.5 * k);
        el = elbow(arm.shoulder, wrist, arm.upper, arm.fore, arm.pole);
    }
    let thumb = (
        palm - up * (1.0 * k) - l.side * (0.6 * k),
        first - nz * (2.4 * k) - l.side * (1.2 * k) + up * (0.8 * k),
    );
    let off = l.semis - l.at;
    let playing =
        (l.semis >= 0.5).then(|| ((off + 0.5) / (l.span / 4.0)).floor().clamp(0.0, 3.0) as usize);
    let tip = playing_tip(l);
    let mut fingers = [(V3::default(), V3::default(), V3::default()); 4];
    for (j, f) in fingers.iter_mut().enumerate() {
        let s = l.at + j as f32 * (l.span / 4.0) * 0.95;
        // The knuckle beside the neck on its far side, a little under the fingerboard.
        let knuckle = on_string(rig, mid, s, 0.0) + rock - nz * (1.3 * k) + l.side * (2.2 * k);
        let t = if playing == Some(j) {
            tip.unwrap_or(knuckle)
        } else {
            on_string(rig, l.string, s, 1.0) + rock
        };
        // The middle joint arched up over the fingerboard's edge.
        let knee = knuckle.lerp(t, 0.45) + nz * (1.7 * k) + l.side * (0.6 * k);
        *f = (knuckle, knee, t);
    }
    LeftPose {
        elbow: el,
        wrist,
        palm,
        thumb,
        fingers,
        playing,
        tip,
    }
}

/// What of the left arm lies behind the neck (painted before the neck is stamped back over
/// it): the sleeve, the palm, the thumb, each finger's first segment, the mannequin's
/// joints.
pub fn left_behind(b: &mut Brush, v: &View, lit: &Lit, arm: &Arm, lp: &LeftPose) {
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    let wr = arm.r.1 * 0.8;
    tube(
        b,
        v,
        lit,
        arm.shoulder,
        lp.elbow,
        arm.r.0 * 1.05,
        arm.r.0,
        arm.sleeve,
    );
    tube(
        b,
        v,
        lit,
        lp.elbow,
        lp.wrist,
        arm.r.1 * 1.05,
        wr,
        arm.sleeve,
    );
    let skin = arm.hand;
    tube(b, v, lit, lp.wrist, lp.palm, wr * 0.92, 2.7 * k, skin);
    tube(b, v, lit, lp.thumb.0, lp.thumb.1, 1.05 * k, 0.85 * k, skin);
    for (j, &(knuckle, knee, _)) in lp.fingers.iter().enumerate() {
        let r = if j == 3 { 0.64 } else { 0.78 } * k;
        tube(
            b,
            v,
            lit,
            lp.palm.lerp(knuckle, 0.3),
            knuckle,
            1.1 * k,
            r * 1.08,
            skin,
        );
        tube(b, v, lit, knuckle, knee, r * 1.06, r, skin);
    }
    if let Some(j) = arm.joints {
        ball(b, v, lit, arm.shoulder, arm.r.0 * 1.25, j);
        ball(b, v, lit, lp.elbow, arm.r.0 * 1.05, j);
        ball(b, v, lit, lp.wrist, arm.r.1 * 1.15, j);
    }
}

/// What of the left hand shows over the fingerboard (painted after the neck): each
/// finger's arch from its middle joint down to its tip.
pub fn left_front(b: &mut Brush, v: &View, lit: &Lit, arm: &Arm, lp: &LeftPose) {
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    for (j, &(_, knee, tip)) in lp.fingers.iter().enumerate() {
        let r = if j == 3 { 0.64 } else { 0.78 } * k;
        tube(b, v, lit, knee, tip, r, r * 0.86, arm.hand);
    }
}

/// The neck and the fingerboard as the eye sees them between the body's edge and the nut
/// (the corners of their silhouette): where the neck is stamped back over what lies
/// behind it.
pub fn neck_outline(v: &View, d: &Dims, pose: &Pose) -> [Pos2; 8] {
    use crate::scene::bowed::{board_w, board_z, nut_y};
    let depth = 0.06 * d.body + 0.6;
    let mut out = [Pos2::ZERO; 8];
    let mut i = 0;
    for y in [d.body - 0.5, nut_y(d) + 0.4] {
        let half = board_w(d, y) * 0.5 + 0.15;
        let top = board_z(d, y) + 0.5;
        for x in [-half, half] {
            out[i] = v.p(pose.at(x, y, top));
            out[i + 1] = v.p(pose.at(x, y, top - depth));
            i += 2;
        }
    }
    out
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

/// The picture's own pixels over the convex hull of `corners`: a textured fan whose
/// coordinates are the points' place in `shown` (the picture's rect).
pub fn stamp(corners: &[Pos2; 8], shown: egui::Rect, tex: egui::TextureId) -> egui::Mesh {
    let (hull, n) = hull8(corners);
    let mut m = egui::Mesh::with_texture(tex);
    for p in &hull[..n] {
        let uv = Pos2::new(
            (p.x - shown.min.x) / shown.width().max(1.0),
            (p.y - shown.min.y) / shown.height().max(1.0),
        );
        m.vertices.push(egui::epaint::Vertex {
            pos: *p,
            uv,
            color: Color32::WHITE,
        });
    }
    for i in 1..n.saturating_sub(1) {
        m.indices.extend_from_slice(&[0, i as u32, i as u32 + 1]);
    }
    m
}

/// The convex hull of eight points, counter-clockwise, without allocating (Andrew's
/// monotone chain on a fixed array).
pub fn hull8(p: &[Pos2; 8]) -> ([Pos2; 16], usize) {
    let mut s = *p;
    s.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut h = [Pos2::ZERO; 16];
    let mut k = 0;
    for &q in &s {
        while k >= 2 && cross(h[k - 2], h[k - 1], q) <= 0.0 {
            k -= 1;
        }
        h[k] = q;
        k += 1;
    }
    let lower = k + 1;
    for &q in s.iter().rev().skip(1) {
        while k >= lower && cross(h[k - 2], h[k - 1], q) <= 0.0 {
            k -= 1;
        }
        h[k] = q;
        k += 1;
    }
    (h, k.saturating_sub(1))
}

/// The instrument's body as the eye sees it (its top's outline and its back's), as two
/// fans from its middle (the outline is star-shaped from there): stamped back with the
/// neck over the figure's arm where it passes behind the body.
pub fn stamp_body(
    v: &View,
    b: crate::scene::bowed::Bowed,
    pose: &Pose,
    shown: egui::Rect,
    tex: egui::TextureId,
) -> [egui::Mesh; 2] {
    let d = b.dims();
    let o = outline_of(b);
    let mid = (d.body * 0.5, 0.0f32);
    let uv = |p: Pos2| {
        Pos2::new(
            (p.x - shown.min.x) / shown.width().max(1.0),
            (p.y - shown.min.y) / shown.height().max(1.0),
        )
    };
    let fan = |z: &dyn Fn(f32) -> f32| {
        let mut m = egui::Mesh::with_texture(tex);
        m.vertices.reserve(o.len() + 1);
        let c = v.p(pose.at(mid.1, mid.0, z(mid.0)));
        m.vertices.push(egui::epaint::Vertex {
            pos: c,
            uv: uv(c),
            color: Color32::WHITE,
        });
        for &(x, y) in o.iter() {
            let p = v.p(pose.at(x, y, z(y)));
            m.vertices.push(egui::epaint::Vertex {
                pos: p,
                uv: uv(p),
                color: Color32::WHITE,
            });
        }
        let n = o.len() as u32;
        m.indices.reserve(3 * n as usize);
        for i in 0..n {
            m.indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % n]);
        }
        m
    };
    [fan(&|_| d.arch), fan(&|y| -crate::scene::bowed::rib(d, y))]
}

/// `b`'s outline (local cm), worked out once and kept.
fn outline_of(b: crate::scene::bowed::Bowed) -> std::sync::Arc<Vec<(f32, f32)>> {
    use std::sync::{Arc, Mutex};
    type Kept = Vec<(crate::scene::bowed::Bowed, Arc<Vec<(f32, f32)>>)>;
    static KEPT: Mutex<Kept> = Mutex::new(Vec::new());
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, o)) = kept.iter().find(|(k, _)| *k == b) {
        return o.clone();
    }
    let o = Arc::new(crate::scene::bowed::outline(b, 2));
    kept.push((b, o.clone()));
    o
}
