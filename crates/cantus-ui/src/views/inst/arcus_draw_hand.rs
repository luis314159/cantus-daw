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
/// r42:arcus-hands, pass 4 — how far (semitones) the fourth finger reaches past the hand's
/// position from the 12th semitone up.
const HIGH_REACH: f32 = 3.0;

pub fn target(at: f32, s: f32, span: f32) -> f32 {
    if s < 0.5 {
        return at;
    }
    // The note under the first finger at most, under the fourth at least.
    // (r42:arcus-hands, pass 4: high up a held instrument's hand shifts rather than reach —
    // from the 9th semitone the fourth finger's reach falls from the span to 3 by the 12th:
    // the little finger's long reach from the 7th position put the palm into the body's
    // shoulder, the coordinator: "let the hand shift or turn instead".)
    let reach = span - (span - HIGH_REACH).max(0.0) * ((s - 9.0) / 3.0).clamp(0.0, 1.0);
    let lo = (s - reach).max(1.0);
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

/// What the left hand plays: the instrument's measures, the string, its stopped note
/// (semitones; 0 open), the hand's position, the vibrato's rock (cm along the string), the
/// side its knuckles are on.
pub struct Left<'a> {
    pub rig: &'a Rig,
    pub d: &'a Dims,
    pub string: usize,
    pub semis: f32,
    pub at: f32,
    pub rock: f32,
    /// The side the knuckles are on and the fingers come over the fingerboard from: under a
    /// chin the E string's (the hall's side; the thumb is on the G string's), behind a cello
    /// or a bass the C string's or the E string's — the player's left (the thumb behind the
    /// neck). `+x` of the instrument's frame for all four.
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

/// The left arm's pose: its elbow and wrist, the back of the hand's middle, the thumb (its
/// base at the wrist, its knuckle, its joint, its tip) and each finger (its knuckle, middle
/// joint, last joint, tip), the finger that plays and where its tip lands.
#[derive(Copy, Clone, Debug)]
pub struct LeftPose {
    pub elbow: V3,
    pub wrist: V3,
    pub palm: V3,
    pub thumb: [V3; 4],
    pub fingers: [[V3; 4]; 4],
    /// Which finger plays the note (none on an open string) and where its tip lands.
    pub playing: Option<usize>,
    pub tip: Option<V3>,
    /// r42:arcus-upper — a realistic figure's hand bones turned from the authored table
    /// (none: the joints above are aimed at, as for the stylised figures).
    pub rots: Option<crate::scene::figure::neck_hand::HandRots>,
}

/// The phalanges' lengths, cm (an adult's hand: index, middle, ring, little finger), and
/// each finger's thickness.
const PHALANGES: [(f32, f32, f32); 4] = [
    (4.0, 2.4, 1.9),
    (4.5, 2.8, 2.0),
    (4.2, 2.7, 1.9),
    (3.4, 2.0, 1.7),
];
const FINGER_R: [f32; 4] = [0.8, 0.82, 0.78, 0.68];

/// The neck's depth under the fingerboard's top, cm (what the stamp and the hidden test
/// take it as).
pub fn neck_depth(d: &Dims) -> f32 {
    // (r42:arcus-polish2: the neck as drawn now — `bowed_head::NECK_DEEP`'s middle.)
    if d.held {
        let (a, b) = crate::scene::bowed_head::NECK_DEEP;
        0.5 * (a + b) * d.body / 35.6
    } else {
        0.06 * d.body + 0.6
    }
}

/// Where the left hand plays for `l`: the fingerboard's top under its first finger (`a`),
/// the way toward the bridge (`bw`), the knuckles' side (`kx`), the fingerboard's normal
/// (`z`), its half width (`hw`) and its top (`top`, in the instrument's frame) there; the
/// finger that plays and where it lands; each finger's tip (the playing one on its stop,
/// the others hovering over their notes); the hand's size (`k`).
#[derive(Copy, Clone, Debug)]
pub struct LeftAt {
    pub a: V3,
    pub bw: V3,
    pub kx: V3,
    pub z: V3,
    pub hw: f32,
    pub top: f32,
    pub playing: Option<usize>,
    pub tip: Option<V3>,
    pub tips: [V3; 4],
    pub k: f32,
}

/// [`LeftAt`] for `l`.
pub fn left_at(arm: &Arm, l: &Left) -> LeftAt {
    let rig = l.rig;
    let d = l.d;
    let up = (rig.strings[l.string][2] - rig.strings[l.string][1]).norm();
    let (bw, kx, z) = (-up, l.side, rig.pose.ez);
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    let rock = up * l.rock;
    // The fingerboard's top under the hand's first finger, and its half width there.
    let first = (on_string(rig, 1, l.at, 0.0) + on_string(rig, 2, l.at, 0.0)) * 0.5 + rock;
    let y_at = (first - rig.pose.o).dot(rig.pose.ey);
    let hw = crate::scene::bowed::board_w(d, y_at) * 0.5;
    let top = crate::scene::bowed::board_z(d, y_at);
    let a = first - z * ((first - rig.pose.o).dot(z) - top);
    let gap = l.span / 3.0;
    let off = l.semis - l.at;
    let playing = (l.semis >= 0.5).then(|| (off / gap).round().clamp(0.0, 3.0) as usize);
    let tip = playing_tip(l);
    // Each finger's tip: the playing one on its stop, the others hovering over their notes.
    let lift = if d.held { 0.9 } else { 1.4 } * k;
    let mut tips = [V3::default(); 4];
    for (j, t) in tips.iter_mut().enumerate() {
        *t = match (playing == Some(j), tip) {
            (true, Some(p)) => p,
            _ => on_string(rig, l.string, l.at + j as f32 * gap, lift) + rock,
        };
    }
    LeftAt {
        a,
        bw,
        kx,
        z,
        hw,
        top,
        playing,
        tip,
        tips,
        k,
    }
}

/// r42:arcus-upper — a realistic figure's left hand on a held neck from the authored table:
/// its bones turned as the table says for this string, this finger and this position (the
/// positions between its rows blended), the hand moved so the playing finger's tip lands on
/// its stop exactly (the vibrato's rock moves it with the hand), the elbow on the line of
/// the hand (the wrist straight). None when the table has no row for it.
fn left_pose_table(arm: &Arm, l: &Left, who: crate::scene::bowed_human::Who) -> Option<LeftPose> {
    use crate::scene::figure::neck_hand;
    let at = left_at(arm, l);
    let viola = l.d.body > crate::scene::bowed::Bowed::Violin.dims().body + 1.0;
    let row = neck_hand::row(who, viola, l.string, at.playing, l.at)?;
    let fig = crate::scene::bowed_human::figure(who);
    let mut hp = neck_hand::place(fig, &row, &l.rig.pose);
    // The playing finger's tip onto its stop; on an open string the hand rides the rock.
    let shift = match (at.playing, at.tip) {
        (Some(j), Some(t)) => t - hp.fingers[j][3],
        _ => {
            let up = (l.rig.strings[l.string][2] - l.rig.strings[l.string][1]).norm();
            up * l.rock
        }
    };
    hp.shift(shift);
    // The forearm on the table's way (r42:arcus-hand: the wrist straight but for a little
    // turn toward the little finger, as the photographs show it), its own length back.
    let mid = (hp.fingers[1][0] + hp.fingers[2][0]) * 0.5;
    let el = hp.wrist + hp.fore * arm.fore;
    Some(LeftPose {
        elbow: el,
        wrist: hp.wrist,
        palm: hp.wrist.lerp(mid, 0.55),
        thumb: hp.thumb,
        fingers: hp.fingers,
        playing: at.playing,
        tip: at.tip,
        rots: Some(hp.rots),
    })
}

/// Where the left arm goes for `l` (the owner's photographs, 2026-11-24, and the handoff
/// note's angles). **Under a chin**: the neck lies in the crook between the thumb and the
/// index finger's base — the index knuckle against the neck's E side just under the
/// fingerboard, the knuckles along the neck a little away from it toward the little finger,
/// the thumb up the G side opposite the first and second fingers, its tip about level with
/// the fingerboard, the palm clear of the neck; each finger rounded (its knuckles arched)
/// over the fingerboard onto its string from above. **Behind a cello or a bass**: the
/// knuckles beside the fingerboard's C side about level with its top, the fingers falling
/// from above onto the strings, the thumb behind the neck behind the second finger. The
/// wrist straight on the line from the elbow to the knuckles; the elbow where `arm`'s pole
/// puts it (under the violin; out to the cellist's side at about the hand's height).
pub fn left_pose(arm: &Arm, l: &Left) -> LeftPose {
    // r42:arcus-upper — a realistic figure's hand on a violin's or a viola's neck: its own
    // bones posed from the authored table (`figure::neck_hand`, made offline against the
    // owner's photographs by tools/arcus-figures/left_hand.py).
    if let (Some(who), true) = (arm.who, l.d.held) {
        if let Some(lp) = left_pose_table(arm, l, who) {
            return lp;
        }
    }
    let d = l.d;
    let LeftAt {
        a,
        bw,
        kx,
        z,
        hw,
        playing,
        tip,
        tips,
        k,
        ..
    } = left_at(arm, l);
    let along = |p: V3| (p - a).dot(bw);
    let depth = neck_depth(d);
    let mut kn = [V3::default(); 4];
    let thumb_tip;
    let thumb_joint;
    let thumb_knuckle;
    if d.held {
        // The index knuckle level with the fingerboard's top, against the E side, toward the
        // scroll from its tip (in first position by the nut: the photographs' hand is at
        // the scroll's end of the neck, its fingers reaching back toward the bridge).
        let base = a + bw * (along(tips[0]) - 1.6 * k) + kx * (hw + 1.5 * k);
        for (j, q) in kn.iter_mut().enumerate() {
            let j = j as f32;
            *q = base + bw * (1.6 * k * j) + kx * (0.45 * k * j) + z * (0.2 * k * j);
        }
        // Opposite the first and second fingers, up the G side.
        let b_t = along(tips[0]).max(0.0) * 0.6 + along(tips[1]) * 0.4;
        thumb_tip = a + bw * (b_t - 0.3 * k) - kx * (hw + 0.6 * k) + z * (0.2 * k);
        thumb_joint = a + bw * (b_t + 0.7 * k) - kx * (hw + 0.75 * k) - z * (depth * 0.7);
        thumb_knuckle = a + bw * (b_t + 2.0 * k) - kx * (0.1 * k) - z * (depth + 1.1 * k);
    } else {
        // Centred on the four notes, beside the C side, level with the fingerboard's top.
        let centre = (along(tips[0]) + along(tips[3])) * 0.5;
        let step = 2.0 * k;
        let base = a + bw * (centre - 1.5 * step) + kx * (hw + 2.0 * k) + z * (0.6 * k);
        for (j, q) in kn.iter_mut().enumerate() {
            let j = j as f32;
            *q = base + bw * (step * j) + kx * (0.2 * k * j) - z * (0.1 * k * j);
        }
        // Behind the second finger, its pad on the neck's back.
        let b_t = along(tips[1]);
        thumb_tip = a + bw * (b_t - 0.4 * k) + kx * (0.2 * k) - z * (depth + 0.7 * k);
        thumb_joint = a + bw * (b_t + 0.4 * k) + kx * (hw + 0.4 * k) - z * (depth + 0.9 * k);
        thumb_knuckle = a + bw * (b_t + 1.4 * k) + kx * (hw + 2.2 * k) - z * (depth * 0.5);
    }
    let mid = (kn[1] + kn[2]) * 0.5;
    // A realistic figure's own hand's length (its knuckles land where they are put).
    let palm_len = arm.reach.unwrap_or(7.2 * k);
    // The wrist straight: on the line from the elbow to the knuckles, a palm's length back.
    let mut wrist = mid - z * palm_len;
    let mut el = elbow(arm.shoulder, wrist, arm.upper, arm.fore, arm.pole);
    for _ in 0..4 {
        wrist = mid + (el - mid).norm() * palm_len;
        el = elbow(arm.shoulder, wrist, arm.upper, arm.fore, arm.pole);
    }
    let radial = (kn[0] - kn[3]).norm();
    let fwd = (mid - wrist).norm();
    let thumb = [
        wrist + radial * (1.0 * k) + fwd * (2.5 * k),
        thumb_knuckle,
        thumb_joint,
        thumb_tip,
    ];
    let mut fingers = [[V3::default(); 4]; 4];
    for (j, f) in fingers.iter_mut().enumerate() {
        let (p0, p1, p2) = PHALANGES[j];
        let (p0, p1, p2) = (p0 * k, p1 * k, p2 * k);
        let t = tips[j];
        // The arch: up off the fingerboard and out toward the knuckles' side.
        let pole = z * 1.0 + kx * 0.7;
        let chord = (p1 * p1 + p2 * p2).sqrt() * 0.98;
        let pip = elbow(kn[j], t, p0, chord, pole);
        let u = t - pip;
        let un = u.norm();
        let out = (pole - un * pole.dot(un)).norm();
        let dip = pip + u * (p1 / (p1 + p2)) + out * (0.3 * k);
        *f = [kn[j], pip, dip, t];
    }
    LeftPose {
        elbow: el,
        wrist,
        palm: wrist.lerp(mid, 0.55),
        thumb,
        fingers,
        playing,
        tip,
        rots: None,
    }
}

/// One lit piece of the left arm: a tube from `a` (`ra` thick) to `b` (`rb`) in `col`, or
/// a ball at `a` (`ra`) when `ball`.
#[derive(Copy, Clone, Debug, Default)]
pub struct Piece {
    pub a: V3,
    pub b: V3,
    pub ra: f32,
    pub rb: f32,
    pub col: [u8; 3],
    pub ball: bool,
}

/// The most pieces a left arm is drawn in.
pub const PIECES: usize = 28;

/// The left arm below the shoulder in pieces, in painting order (the forearm, the palm, the
/// thumb, each finger from its knuckle to its tip; the mannequin's joints): a fixed array,
/// so a frame allocates nothing for it.
pub fn left_pieces(arm: &Arm, lp: &LeftPose) -> ([Piece; PIECES], usize) {
    let mut out = [Piece::default(); PIECES];
    let mut n = 0;
    let mut put = |p: Piece| {
        if n < PIECES {
            out[n] = p;
            n += 1;
        }
    };
    let tube = |a: V3, b: V3, ra: f32, rb: f32, col: [u8; 3]| Piece {
        a,
        b,
        ra,
        rb,
        col,
        ball: false,
    };
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    let wr = arm.r.1 * 0.8;
    let skin = arm.hand;
    put(tube(lp.elbow, lp.wrist, arm.r.1 * 1.05, wr, arm.sleeve));
    if let Some(jc) = arm.joints {
        put(Piece {
            a: lp.wrist,
            b: lp.wrist,
            ra: wr * 1.05,
            rb: wr * 1.05,
            col: jc,
            ball: true,
        });
    }
    let [kn0, kn1, kn2, kn3] = [0, 1, 2, 3].map(|j| lp.fingers[j][0]);
    let mid = (kn1 + kn2) * 0.5;
    let radial = (kn0 - kn3).norm();
    // The thumb first: it lies on the palm's side, under the hand's back.
    put(tube(lp.thumb[0], lp.thumb[1], 1.25 * k, 1.0 * k, skin));
    put(tube(lp.thumb[1], lp.thumb[2], 1.0 * k, 0.9 * k, skin));
    put(tube(lp.thumb[2], lp.thumb[3], 0.9 * k, 0.78 * k, skin));
    // The palm: a fan of five rounded strips from the wrist to the knuckles (the back of
    // the hand reads broad and flat, not as separate bones), the thumb's ball inside it.
    let wrist_r = lp.wrist + radial * (0.3 * k);
    let wrist_u = lp.wrist - radial * (0.3 * k);
    for i in 0..5 {
        let t = i as f32 / 4.0;
        let kn = if t <= 0.5 {
            kn3.lerp(mid, t * 2.0)
        } else {
            mid.lerp(kn0, t * 2.0 - 1.0)
        };
        let w = wrist_u.lerp(wrist_r, t);
        let r0 = (1.0 - (t - 0.5).abs()) * 1.5 * k;
        put(tube(w, kn, r0.max(1.15 * k), 1.05 * k, skin));
    }
    for (j, f) in lp.fingers.iter().enumerate() {
        let r = FINGER_R[j] * k;
        put(tube(f[0], f[1], r * 1.08, r, skin));
        put(tube(f[1], f[2], r, r * 0.94, skin));
        put(tube(f[2], f[3], r * 0.94, r * 0.86, skin));
    }
    if let Some(jc) = arm.joints {
        let r = arm.r.0 * 1.05;
        put(Piece {
            a: lp.elbow,
            b: lp.elbow,
            ra: r,
            rb: r,
            col: jc,
            ball: true,
        });
    }
    (out, n)
}

/// Whether the neck (the fingerboard and the neck under it, between the body's edge and
/// the nut) hides `p` from `eye`. In the instrument's frame the neck's width and its top
/// change linearly along it, so every face is a linear bound on the ray's parameter: the
/// ray is hidden when the bounds leave a stretch of it (no march, a few dozen flops).
pub fn hidden_by_neck(p: V3, eye: V3, d: &Dims, pose: &Pose) -> bool {
    use crate::scene::bowed::{board_w, board_z, nut_y};
    let loc = |w: V3| {
        let q = w - pose.o;
        V3 {
            x: q.dot(pose.ex),
            y: q.dot(pose.ey),
            z: q.dot(pose.ez),
        }
    };
    let (a, e) = (loc(p), loc(eye));
    let len = (e - a).len();
    let dir = (e - a) * (1.0 / len.max(1e-6));
    let (y0, y1) = (d.body - 0.5, nut_y(d) + 0.4);
    // The half width and the top as lines in y.
    let lin = |f: &dyn Fn(f32) -> f32| {
        let (f0, f1) = (f(y0), f(y1));
        let k = (f1 - f0) / (y1 - y0);
        (f0 - k * y0, k)
    };
    let (w0, wk) = lin(&|y| board_w(d, y) * 0.5 + 0.1);
    let (t0, tk) = lin(&|y| board_z(d, y) + 0.05);
    let depth = neck_depth(d);
    let (mut lo, mut hi) = (0.2f32, len);
    // Each bound c0 + c1·s ≥ 0 narrows [lo, hi].
    let mut bound = |c0: f32, c1: f32| {
        if c1.abs() < 1e-7 {
            if c0 < 0.0 {
                hi = -1.0;
            }
        } else if c1 > 0.0 {
            lo = lo.max(-c0 / c1);
        } else {
            hi = hi.min(-c0 / c1);
        }
    };
    let (ya, yd) = (a.y, dir.y);
    bound(ya - y0, yd);
    bound(y1 - ya, -yd);
    // |x| ≤ w0 + wk·y
    bound(w0 + wk * ya - a.x, wk * yd - dir.x);
    bound(w0 + wk * ya + a.x, wk * yd + dir.x);
    // top - depth ≤ z ≤ top
    bound(t0 + tk * ya - a.z, tk * yd - dir.z);
    bound(a.z - (t0 + tk * ya - depth), dir.z - tk * yd);
    if lo > hi {
        return false;
    }
    // r42:arcus-hands, pass 5 — and through the neck as it is drawn, not its box: the
    // fingerboard's slab, then the neck's rounded back under it. Skin in the box's lower
    // corners (beside the rounded back) was taken for hidden, painted before the neck's stamp
    // and stamped over with the picture's background: the saw-toothed chips at the hand's
    // edge in third position.
    (0..NECK_STEPS).any(|k| {
        let t = lo + (hi - lo) * (k as f32 + 0.5) / NECK_STEPS as f32;
        let q = a + dir * t;
        let (w, top) = (w0 + wk * q.y, t0 + tk * q.y);
        let r = (q.x / w.max(1e-3)).clamp(-1.0, 1.0);
        q.z <= top && q.z >= top - NECK_SLAB.max(depth * (1.0 - r * r).sqrt())
    })
}

/// r42:arcus-hands, pass 5 — the fingerboard's own thickness (cm) over the neck's rounded
/// back, and the samples along a ray's way through the neck's box.
const NECK_SLAB: f32 = 0.55;
const NECK_STEPS: usize = 10;

/// r42:arcus-hands, pass 6 — whether the body hides `p` from `eye`: `p` deeper than the
/// back plate (`margin` cm under the ribs, over the body's length) and its ray to the eye
/// crossing the back's or the top's plane inside the body's outline. Skin under the back
/// whose ray passed beside the body was taken for hidden, painted before the body's stamp
/// and stamped over with the picture round the body's edge: the thin dark wedge in her palm
/// near the wrist in high position (pass 5's 400 % sheets).
pub fn hidden_by_body(
    p: V3,
    eye: V3,
    b: crate::scene::bowed::Bowed,
    pose: &Pose,
    margin: f32,
) -> bool {
    use crate::scene::bowed::{rib, Bowed};
    let d = b.dims();
    let loc = |w: V3| {
        let q = w - pose.o;
        V3 {
            x: q.dot(pose.ex),
            y: q.dot(pose.ey),
            z: q.dot(pose.ez),
        }
    };
    let a = loc(p);
    if !(0.0..=d.body).contains(&a.y) || a.z >= -rib(&d, a.y) - margin {
        return false;
    }
    let e = loc(eye);
    let dir = e - a;
    if dir.z <= 1e-6 {
        return false;
    }
    // The body's outline, once a kind (the app's own, `bowed::outline`).
    static OUTLINES: std::sync::OnceLock<[Vec<(f32, f32)>; 4]> = std::sync::OnceLock::new();
    let all = OUTLINES.get_or_init(|| {
        [Bowed::Cello, Bowed::Bass, Bowed::Violin, Bowed::Viola]
            .map(|k| crate::scene::bowed::outline(k, 8))
    });
    let o = &all[match b {
        Bowed::Cello => 0,
        Bowed::Bass => 1,
        Bowed::Violin => 2,
        Bowed::Viola => 3,
    }];
    let back = -rib(&d, a.y);
    [back, 0.0].iter().any(|&z| {
        let t = (z - a.z) / dir.z;
        inside(o, a.x + dir.x * t, a.y + dir.y * t)
    })
}

/// Whether (x, y) lies inside the closed polygon `o` (crossings of a ray along +x).
fn inside(o: &[(f32, f32)], x: f32, y: f32) -> bool {
    let n = o.len();
    let mut c = false;
    for i in 0..n {
        let (p, q) = (o[i], o[(i + n - 1) % n]);
        if (p.1 > y) != (q.1 > y) && x < p.0 + (q.0 - p.0) * (y - p.1) / (q.1 - p.1) {
            c = !c;
        }
    }
    c
}

/// Paint the left arm's pieces the neck hides from the eye (`hidden`: painted before the
/// neck is stamped back over them) or those it does not (after). A piece the neck hides in
/// part is split where it comes out from behind it.
#[allow(clippy::too_many_arguments)]
pub fn left_paint(
    b: &mut Brush,
    v: &View,
    lit: &Lit,
    arm: &Arm,
    lp: &LeftPose,
    l: &Left,
    hidden: bool,
) {
    let (pieces, n) = left_pieces(arm, lp);
    let eye = v.cam.eye();
    let hid = |p: V3| hidden_by_neck(p, eye, l.d, &l.rig.pose);
    for pc in &pieces[..n] {
        if pc.ball {
            if hid(pc.a) == hidden {
                ball(b, v, lit, pc.a, pc.ra, pc.col);
            }
            continue;
        }
        // Where along it the neck starts or stops hiding it (one crossing at most).
        let (h0, hm, h1) = (hid(pc.a), hid(pc.a.lerp(pc.b, 0.5)), hid(pc.b));
        let cut = |lo: f32, hi: f32, from: bool| {
            let (mut lo, mut hi) = (lo, hi);
            for _ in 0..6 {
                let m = 0.5 * (lo + hi);
                if hid(pc.a.lerp(pc.b, m)) == from {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            0.5 * (lo + hi)
        };
        let (t, first) = if h0 == h1 && h0 == hm {
            (1.0, h0)
        } else if h0 != hm {
            (cut(0.0, 0.5, h0), h0)
        } else {
            (cut(0.5, 1.0, h0), h0)
        };
        let r_at = |s: f32| pc.ra + (pc.rb - pc.ra) * s;
        let m = pc.a.lerp(pc.b, t);
        if first == hidden {
            tube(b, v, lit, pc.a, m, pc.ra, r_at(t), pc.col);
        }
        if t < 1.0 && first != hidden {
            tube(b, v, lit, m, pc.b, r_at(t), pc.rb, pc.col);
        }
    }
}

/// What of the left arm lies behind the instrument (painted before the neck and the body
/// are stamped back over it): the upper arm (under the violin, beside the cello), the
/// shoulder's joint, and what the neck hides of the rest.
pub fn left_behind(b: &mut Brush, v: &View, lit: &Lit, arm: &Arm, lp: &LeftPose, l: &Left) {
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
    if let Some(j) = arm.joints {
        ball(b, v, lit, arm.shoulder, arm.r.0 * 1.25, j);
    }
    left_paint(b, v, lit, arm, lp, l, true);
}

/// What of the left arm shows in front of the neck (painted after it).
pub fn left_front(b: &mut Brush, v: &View, lit: &Lit, arm: &Arm, lp: &LeftPose, l: &Left) {
    left_paint(b, v, lit, arm, lp, l, false);
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
    let uv = |p: Pos2| {
        Pos2::new(
            (p.x - shown.min.x) / shown.width().max(1.0),
            (p.y - shown.min.y) / shown.height().max(1.0),
        )
    };
    for p in &hull[..n] {
        m.vertices.push(egui::epaint::Vertex {
            pos: *p,
            uv: uv(*p),
            color: Color32::WHITE,
        });
    }
    for i in 1..n.saturating_sub(1) {
        m.indices.extend_from_slice(&[0, i as u32, i as u32 + 1]);
    }
    // r42:arcus-upper — a feathered rim (the owner's v7 review: the edge where the live
    // layer meets the picture read stair-stepped at 200 %): each corner again a little
    // outside, clear, so the stamp fades into what is under it over [`FEATHER`] points.
    let c = hull[..n]
        .iter()
        .fold(egui::Vec2::ZERO, |a, p| a + p.to_vec2())
        / n.max(1) as f32;
    for &p in &hull[..n] {
        let out = (p.to_vec2() - c).normalized() * FEATHER;
        let q = p + out;
        m.vertices.push(egui::epaint::Vertex {
            pos: q,
            uv: uv(q),
            color: Color32::TRANSPARENT,
        });
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (a, b, oa, ob) = (i as u32, j as u32, (n + i) as u32, (n + j) as u32);
        m.indices.extend_from_slice(&[a, oa, ob, a, ob, b]);
    }
    m
}

/// How far a stamp's edge fades into the live layer under it (points).
const FEATHER: f32 = 0.8;

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
