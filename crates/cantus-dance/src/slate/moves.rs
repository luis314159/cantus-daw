//! The Slate cast's shared choreography (the design rig's `moves.py`), as pure functions of
//! the beat: `b` is beats since the move started, beat 0 is the beat. Each takes a
//! [`Style`] so the same choreography is danced in each character's way.
//!
//! - [`groove`] (2 beats): a bounce on each beat, claps in front and high;
//! - [`steps`] (4 beats): step right, close left, step left, close right;
//! - [`jump`] (2 beats, the drop): recover, crouch, a star in the air, land on the beat;
//! - the open-silhouette moves (arms by angle and bend, nothing crossing the body):
//!   [`pump`], [`steps_open`], [`star`] — Compás's; [`steps_human`] and [`steps_wave`] for
//!   human and big-headed figures.

use super::fig::*;
use crate::rig::P;

/// A character's way of dancing the Slate choreography.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Style {
    /// The pelvis's dip on a beat (L units).
    pub bounce: f32,
    /// The hips side to side (L units).
    pub sway: f32,
    /// A step's travel (L units).
    pub step: f32,
    /// How far arms reach out (× the choreography's).
    pub arms: f32,
    /// The head and what hangs, behind the beat (beats).
    pub lag: f32,
    /// Jump height (units of height).
    pub hop: f32,
    /// A landing's squash.
    pub squash: f32,
    /// 0 fluid … 1 pose-to-pose holds on the half beat.
    pub snap: f32,
    pub lean: f32,
    /// The hands between claps (neck-relative, A units).
    pub open: P,
    /// The chest's sway against the hips (T units).
    pub chest: f32,
    /// How high a step lifts the foot (L units).
    pub lift_step: f32,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            bounce: 0.07,
            sway: 0.06,
            step: 0.16,
            arms: 1.0,
            lag: 0.10,
            hop: 0.20,
            squash: 0.10,
            snap: 0.0,
            lean: 1.0,
            open: (0.50, 0.84),
            chest: 0.02,
            lift_step: 0.07,
        }
    }
}

/// Keyed values at beats (sorted, within `period`): the two keys either side of `b` and how
/// far it is between them.
pub fn keyed<T: Copy>(keys: &[(f32, T)], b: f32, period: f32) -> (T, T, f32) {
    let bb = pmod(b, period);
    let n = keys.len();
    for i in 0..n {
        let (k, v) = keys[i];
        let (nk, nv) = keys[(i + 1) % n];
        let mut span = pmod(nk - k, period);
        if span == 0.0 {
            span = period;
        }
        let into = pmod(bb - k, period);
        if into < span {
            return (v, nv, into / span);
        }
    }
    (keys[0].1, keys[0].1, 0.0)
}

/// Hands from keyed positions, eased between.
fn hands(keys: &[(f32, [P; 2])], b: f32, period: f32, ease: fn(f32) -> f32) -> [P; 2] {
    let (a, z, t) = keyed(keys, b, period);
    lerp_pair(a, z, ease(t))
}

fn linear(t: f32) -> f32 {
    t
}

/// Arms by angle and bend from keyed positions, eased between.
pub fn fk_keys(keys: &[(f32, [(f32, f32); 2])], b: f32, period: f32) -> [(f32, f32); 2] {
    let (a, z, t) = keyed(keys, b, period);
    let t = ease_in_out(t);
    [
        (lerp(a[0].0, z[0].0, t), lerp(a[0].1, z[0].1, t)),
        (lerp(a[1].0, z[1].0, t), lerp(a[1].1, z[1].1, t)),
    ]
}

/// Two beats: a bounce down on each beat; a clap in front of the chest on beat 1 and high
/// over the right shoulder on beat 2; the hips sway toward the high clap.
pub fn groove(b: f32, st: &Style) -> Fig {
    let b = if st.snap != 0.0 { snap(b, st.snap) } else { b };
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let k = (b.floor() as i64).rem_euclid(2) as usize;
    let down = beat_down(ph, 2.2);
    let side = cos1((b - 1.0) / 2.0); // +1 at beat 2 (right), −1 at beat 1
    p.hip = (st.sway * side, st.bounce * down);
    p.lean = 4.0 * side * st.lean;
    p.chest = (st.chest * side, 0.03 * down);
    p.roll = 5.0 * cos1((b - 1.0 - st.lag) / 2.0);
    p.nod = 10.0 * beat_down(pmod(b - st.lag, 1.0), 2.0) - 3.0;
    p.tilt = 7.0 * cos1((b - 1.0 - st.lag) / 2.0);
    let a = st.arms;
    let meet = [
        [(-0.035, 0.50), (0.035, 0.50)],
        [(0.30, 0.22), (0.38, 0.22)],
    ];
    let open = [(-st.open.0 * a, st.open.1), (st.open.0 * a, st.open.1)];
    let (m0, m1) = (meet[k], meet[(k + 1) % 2]);
    p.hands = if ph < 0.5 {
        lerp_pair(m0, open, ease_out(ph * 2.0))
    } else {
        lerp_pair(open, m1, ease_in(ph * 2.0 - 1.0))
    };
    p.elbow = [1.0, 1.0];
    // The foot away from the sway taps on the off-beat.
    let tap = arc(clamp01((ph - 0.25) * 2.0)) * 0.05;
    p.feet = [
        (0.0, if side > 0.0 { -tap } else { 0.0 }),
        (0.0, if side < 0.0 { -tap } else { 0.0 }),
    ];
    p.claps = (1.0 - ph * 4.0).max(0.0);
    p.accent = beat_down(ph, 8.0) * if k == 0 { 1.0 } else { 0.7 };
    p.mouth = if k == 1 {
        0.7 * beat_down(ph, 3.0)
    } else {
        0.0
    };
    p.eyes = Eyes::Happy;
    p.point = 28.0 * side;
    p.level = 0.55 + 0.4 * beat_down(ph, 3.0);
    p
}

/// A foot's (x, y) from its landings `(land_beat, from_x, to_x)`, each travelled in the
/// half beat before it lands, lifted on an arc.
fn foot_track(b: f32, moves: &[(f32, f32, f32)], period: f32, lift: f32) -> P {
    let bb = pmod(b, period);
    for &(land, x0, x1) in moves {
        let t = pmod(bb - (land - 0.5), period);
        if t < 0.5 {
            let u = ease_in_out(t / 0.5);
            return (lerp(x0, x1, u), -lift * arc(t / 0.5));
        }
    }
    // Holding: the last landing before now.
    let mut best: Option<(f32, f32)> = None;
    for &(land, _, x1) in moves {
        let since = pmod(bb - land, period);
        if best.is_none_or(|(s, _)| since < s) {
            best = Some((since, x1));
        }
    }
    (best.map_or(0.0, |b| b.1), 0.0)
}

/// Four beats of step-touch: step right (1), close left (2), step left (3), close right
/// (4); the arms swing low on a step and snap to the side on a close.
pub fn steps(b: f32, st: &Style) -> Fig {
    let b = if st.snap != 0.0 { snap(b, st.snap) } else { b };
    let d = st.step;
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let lm = [(1.0, -d, d), (2.0, d, -d)];
    let rm = [(0.0, -d, d), (3.0, d, -d)];
    let left = foot_track(b, &lm, 4.0, st.lift_step);
    let right = foot_track(b, &rm, 4.0, st.lift_step);
    p.feet = [left, right];
    let hx = (left.0 + right.0) / 2.0;
    let ahead =
        (foot_track(b + 0.15, &lm, 4.0, 0.07).0 + foot_track(b + 0.15, &rm, 4.0, 0.07).0) / 2.0;
    p.hip = (hx * 1.05, st.bounce * beat_down(ph, 2.0) + 0.02);
    p.lean = (ahead - hx) * 40.0 * st.lean;
    let a = st.arms;
    let keys = [
        (0.0, [(-0.50 * a, 0.70), (0.50 * a, 0.70)]), // step right: arms low and open
        (1.0, [(0.12, 0.30), (0.78 * a, -0.02)]),     // close: a snap to the right
        (2.0, [(-0.50 * a, 0.70), (0.50 * a, 0.70)]),
        (3.0, [(-0.78 * a, -0.02), (-0.12, 0.30)]), // a snap to the left
    ];
    p.hands = hands(&keys, b, 4.0, ease_in_out);
    let sidek = pmod(b, 4.0).floor() as i64;
    let in4 = pmod(b, 4.0);
    let snapside = if (0.5..2.5).contains(&in4) { 1.0 } else { -1.0 };
    p.tilt = 9.0
        * cantus_dsp::math::sin(
            std::f32::consts::PI * pmod(b - st.lag, 4.0) / 2.0 - std::f32::consts::PI / 4.0,
        );
    p.roll = 6.0 * snapside * smooth(1.0 - (pmod(b, 2.0) - 1.0).abs());
    p.nod = 8.0 * beat_down(pmod(b - st.lag, 1.0), 2.0) - 2.0;
    p.chest = (0.03 * snapside, 0.02 * beat_down(ph, 2.0));
    let on_step = sidek == 0 || sidek == 2;
    p.accent = beat_down(ph, 8.0) * if on_step { 1.0 } else { 0.7 };
    p.eyes = if on_step { Eyes::Open } else { Eyes::Happy };
    p.mouth = 0.0;
    p.point = 25.0 * snapside;
    p.level = 0.5 + 0.4 * beat_down(ph, 3.0);
    p
}

/// Two beats, landing on the beat: recover, crouch with the arms low, spring into a star —
/// arms up in a V, legs apart — and land on the next beat.
pub fn jump(b: f32, st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    let h = st.hop;
    let a = st.arms;
    let rest = [(-0.50 * a, 0.92), (0.50 * a, 0.92)];
    let low = [(-0.62 * a, 0.86), (0.62 * a, 0.86)];
    let down = [(-0.95 * a, 0.32), (0.95 * a, 0.32)];
    if bb < 0.5 {
        // Landed: squash, recover.
        let u = bb / 0.5;
        p.hip = (0.0, lerp(0.22, 0.02, ease_out(u)));
        p.squash = st.squash * (1.0 - ease_out(u * 1.4));
        p.hands = lerp_pair(down, rest, ease_out(u));
        p.landed = (1.0 - bb * 3.0).max(0.0);
        p.eyes = Eyes::Happy;
        p.nod = lerp(12.0, 0.0, ease_out(u));
    } else if bb < 1.0 {
        // The crouch: the anticipation.
        let u = (bb - 0.5) / 0.5;
        p.hip = (0.0, lerp(0.02, 0.30, ease_in_out(u)));
        p.hands = lerp_pair(rest, low, ease_in_out(u));
        p.lean = 0.0;
        p.nod = lerp(0.0, 14.0, u);
        p.eyes = if u > 0.6 { Eyes::Closed } else { Eyes::Open };
        p.squash = 0.04 * ease_in(u);
    } else {
        // In the air: a star, then land.
        let u = bb - 1.0;
        p.lift = h * cantus_dsp::math::powf(arc(u), 0.85);
        let up = ease_out(clamp01(u / 0.22));
        p.hip = (
            0.0,
            if u < 0.22 {
                lerp(0.30, -0.02, up)
            } else {
                lerp(-0.02, 0.06, smooth((u - 0.6) / 0.4))
            },
        );
        let star = cantus_dsp::math::sin(std::f32::consts::PI * clamp01((u - 0.08) / 0.8));
        p.feet = [(-0.16 * star, -0.10 * star), (0.16 * star, -0.10 * star)];
        let vee = [(-0.62 * a, -0.90), (0.62 * a, -0.90)];
        p.hands = if u < 0.25 {
            lerp_pair(low, vee, ease_out(u / 0.25))
        } else if u < 0.65 {
            vee
        } else {
            lerp_pair(vee, down, ease_in_out((u - 0.65) / 0.35))
        };
        p.squash = -0.08 * cantus_dsp::math::sin(std::f32::consts::PI * clamp01(u / 0.3))
            - 0.04 * smooth((u - 0.7) / 0.3);
        p.nod = if u < 0.5 {
            lerp(14.0, -10.0, up)
        } else {
            lerp(-10.0, 6.0, (u - 0.5) / 0.5)
        };
        p.eyes = Eyes::Wide;
        p.mouth = 0.8 * cantus_dsp::math::sin(std::f32::consts::PI * clamp01((u - 0.1) / 0.7));
    }
    p.accent = beat_down(pmod(b, 1.0), 8.0) * if bb < 1.0 { 1.0 } else { 0.6 };
    p.level = if bb >= 1.0 { 0.9 } else { 0.5 };
    p.point = if bb >= 1.0 {
        40.0 * cantus_dsp::math::sin(std::f32::consts::PI * clamp01(bb - 1.0))
    } else {
        -10.0 * pmod(bb, 1.0)
    };
    p
}

/// Arm keys of the open-silhouette moves: (angle, bend).
pub const UP: (f32, f32) = (148.0, 0.0);
pub const DOWN: (f32, f32) = (38.0, 0.0);
pub const OUT: (f32, f32) = (92.0, 0.0);
pub const W: (f32, f32) = (100.0, 82.0);
pub const HIP: (f32, f32) = (34.0, -76.0);

/// Two beats: one arm punches up on the diagonal and the other down, swapping on each beat;
/// between, both arms flex in a W; the foot under the low arm flicks out.
pub fn pump(b: f32, st: &Style) -> Fig {
    let b = if st.snap != 0.0 { snap(b, st.snap) } else { b };
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let k = (b.floor() as i64).rem_euclid(2);
    p.fk = Some(fk_keys(
        &[
            (0.0, [DOWN, UP]),
            (0.5, [W, W]),
            (1.0, [UP, DOWN]),
            (1.5, [W, W]),
        ],
        b,
        2.0,
    ));
    let sw = cos1(b / 2.0); // +1 on beat 1 (right arm up), −1 on beat 2
    p.hip = (-st.sway * sw, st.bounce * beat_down(ph, 2.0));
    p.lean = 5.0 * sw * st.lean;
    p.tilt = 12.0 * sw;
    p.nod = 6.0 * beat_down(ph, 2.0) - 4.0;
    let flick = beat_down(ph, 1.5);
    p.feet = if sw > 0.0 {
        [(-0.12 * flick, -0.12 * flick), (0.0, 0.0)]
    } else {
        [(0.0, 0.0), (0.12 * flick, -0.12 * flick)]
    };
    p.accent = beat_down(ph, 8.0) * if k == 0 { 1.0 } else { 0.7 };
    p.eyes = Eyes::Open;
    p.point = 30.0 * sw;
    p.level = 0.55 + 0.4 * beat_down(ph, 3.0);
    p
}

/// Step-touch with the arms in clear lines: out in a T on a step, a disco point to the side
/// it closes toward, the other hand on the hip.
pub fn steps_open(b: f32, st: &Style) -> Fig {
    let mut p = steps(b, st);
    let b = if st.snap != 0.0 { snap(b, st.snap) } else { b };
    p.fk = Some(fk_keys(
        &[
            (0.0, [OUT, OUT]),
            (1.0, [HIP, UP]),
            (2.0, [OUT, OUT]),
            (3.0, [UP, HIP]),
        ],
        b,
        4.0,
    ));
    p
}

/// The jump with straight arms: down and out on landing, low behind in the crouch, a V in
/// the air (the star jump).
pub fn star(b: f32, st: &Style) -> Fig {
    let mut p = jump(b, st);
    let bb = pmod(b, 2.0);
    let (land, low, v) = (70.0, 22.0, 150.0);
    let a = if bb < 0.5 {
        lerp(land, 12.0, ease_out(bb / 0.5))
    } else if bb < 1.0 {
        lerp(12.0, low, ease_in_out((bb - 0.5) / 0.5))
    } else {
        let u = bb - 1.0;
        if u < 0.22 {
            lerp(low, v, ease_out(u / 0.22))
        } else if u < 0.65 {
            v
        } else {
            lerp(v, land, ease_in_out((u - 0.65) / 0.35))
        }
    };
    p.fk = Some([(a, 0.0), (a, 0.0)]);
    p
}

/// Step-touch for a human figure: the arms swing low and loose on a step; on a close the
/// near arm reaches out and up on the diagonal (a snap) and the far hand comes to the chest.
pub fn steps_human(b: f32, st: &Style) -> Fig {
    let mut p = steps(b, st);
    let keys = [
        (0.0, [(-0.55, 0.95), (0.55, 0.95)]),
        (1.0, [(0.10, 0.42), (1.00, -0.62)]),
        (2.0, [(-0.55, 0.95), (0.55, 0.95)]),
        (3.0, [(-1.00, -0.62), (-0.10, 0.42)]),
    ];
    p.hands = hands(&keys, b, 4.0, ease_in_out);
    let in4 = pmod(b, 4.0);
    let snapside = if (0.5..2.5).contains(&in4) { 1.0 } else { -1.0 };
    p.chest = (-0.05 * snapside, p.chest.1);
    p
}

/// Step-touch for a big-headed character: nothing crosses the body — on a close the near
/// arm waves up and out, the far one swings out low.
pub fn steps_wave(b: f32, st: &Style) -> Fig {
    let mut p = steps(b, st);
    let a = st.arms;
    let keys = [
        (0.0, [(-0.62 * a, 0.80), (0.62 * a, 0.80)]),
        (1.0, [(-0.70 * a, 0.70), (0.80 * a, -0.62)]),
        (2.0, [(-0.62 * a, 0.80), (0.62 * a, 0.80)]),
        (3.0, [(-0.80 * a, -0.62), (0.70 * a, 0.70)]),
    ];
    p.hands = hands(&keys, b, 4.0, ease_in_out);
    p
}

/// Hands from keys, linearly between (Tono's robot arms).
pub fn hands_linear(keys: &[(f32, [P; 2])], b: f32, period: f32) -> [P; 2] {
    hands(keys, b, period, linear)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_lands_on_the_beat_and_lifts_between() {
        let st = Style::default();
        let on = steps(0.0, &st);
        assert!(on.feet[1].1.abs() < 1e-6, "the right foot lands on beat 0");
        let mid = steps(3.75, &st);
        assert!(mid.feet[1].1 < -0.01, "in the air half a beat before");
    }

    #[test]
    fn the_jump_lands_on_the_beat() {
        let st = Style::default();
        assert!(jump(0.0, &st).lift.abs() < 1e-6);
        assert!(jump(1.5, &st).lift > 0.1);
        assert!(jump(1.999, &st).lift < 0.02);
    }

    #[test]
    fn keyed_values_wrap_round_the_period() {
        let keys = [(0.0, 0.0f32), (1.0, 10.0), (3.0, 30.0)];
        let (a, z, t) = keyed(&keys, 3.5, 4.0);
        assert_eq!((a, z), (30.0, 0.0));
        assert!((t - 0.5).abs() < 1e-6);
    }
}
