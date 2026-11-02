//! The family's choreography (ADR 0090): every [`Move`] as numbers of a [`Body`] — where
//! the body is, how it leans, squashes and turns, where the arms (wings, paws) and feet
//! are, and what hangs on it (a tail, a crest, ears) — from the pose's phase alone.
//!
//! A character takes these with its own [`Style`] (how bouncy, how wide, how sharp) and
//! draws its body from them; it may replace any move with one of its own. **Phase 0 is the
//! beat**: a step lands, a clap meets, a kick is out on it; a side alternates by
//! `pose.cycle`. What hangs on the body follows late (overlapping action), arms and feet
//! move on arcs with easing, a landing squashes and a take-off stretches.
//!
//! Angles of an arm: 0 hangs down, 90 is straight out to its side, 180 straight up, a
//! negative angle crosses in front of the body. Index 0 is the arm or foot on the
//! viewer's left.

use super::*;
use cantus_core::inst::dancer::Move;

/// A character's way of dancing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Style {
    /// How high the body bobs on a beat (its units).
    pub bounce: f32,
    /// How much it squashes and stretches (0…0.3).
    pub squash: f32,
    /// How far arms swing (× the family's).
    pub arms: f32,
    /// How far it shifts side to side (its units).
    pub sway: f32,
    /// How far behind the beat what hangs on it follows (a share of a cycle).
    pub lag: f32,
    /// How high it jumps (its units).
    pub hop: f32,
    /// How far a foot steps out (its units).
    pub stride: f32,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            bounce: 0.06,
            squash: 0.12,
            arms: 1.0,
            sway: 0.06,
            lag: 0.12,
            hop: 0.2,
            stride: 0.13,
        }
    }
}

/// One moment of a body.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Body {
    /// Offset of the body from standing (its units; y negative is up).
    pub x: f32,
    pub y: f32,
    /// Degrees it leans about its feet (positive to the viewer's right).
    pub lean: f32,
    /// > 0 squashed, < 0 stretched.
    pub squash: f32,
    /// Degrees it has turned (the pseudo-3D ball).
    pub yaw: f32,
    /// Degrees the face looks up (+) or down (−).
    pub nod: f32,
    /// Degrees the face rolls (positive clockwise).
    pub tilt: f32,
    pub arm: [f32; 2],
    /// Curl of a wrist or wing tip, degrees (positive toward the body's middle).
    pub bend: [f32; 2],
    /// 0 closed … 1 fanned (fingers, feathers).
    pub spread: [f32; 2],
    /// Feet from where they stand (its units; y negative is lifted).
    pub foot: [P; 2],
    /// Degrees a tail swings and what is on top (a crest, ears) leans.
    pub tail: f32,
    pub top: f32,
    /// 0 closed … 1 wide open.
    pub mouth: f32,
    /// Where the eyes look (−1…1 each way; y positive is down).
    pub look: P,
    /// 0 open … 1 shut, per eye; and a smile's squint.
    pub wink: [f32; 2],
    pub squint: f32,
}

impl Body {
    /// Standing at rest.
    pub fn rest() -> Body {
        Body {
            arm: [10.0, 10.0],
            spread: [0.3, 0.3],
            ..Body::default()
        }
    }

    pub fn lerp(a: &Body, b: &Body, t: f32) -> Body {
        let l = |x: f32, y: f32| lerp(x, y, t);
        let l2 = |x: [f32; 2], y: [f32; 2]| [l(x[0], y[0]), l(x[1], y[1])];
        let lp = |x: P, y: P| lerp_p(x, y, t);
        // A turn blends the short way round.
        let mut dy = (b.yaw - a.yaw).rem_euclid(360.0);
        if dy > 180.0 {
            dy -= 360.0;
        }
        Body {
            x: l(a.x, b.x),
            y: l(a.y, b.y),
            lean: l(a.lean, b.lean),
            squash: l(a.squash, b.squash),
            yaw: a.yaw + dy * t,
            nod: l(a.nod, b.nod),
            tilt: l(a.tilt, b.tilt),
            arm: l2(a.arm, b.arm),
            bend: l2(a.bend, b.bend),
            spread: l2(a.spread, b.spread),
            foot: [lp(a.foot[0], b.foot[0]), lp(a.foot[1], b.foot[1])],
            tail: l(a.tail, b.tail),
            top: l(a.top, b.top),
            mouth: l(a.mouth, b.mouth),
            look: lp(a.look, b.look),
            wink: l2(a.wink, b.wink),
            squint: l(a.squint, b.squint),
        }
    }
}

/// Where an extra is born, which the character turns into a point of its own.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Anchor {
    /// Beside the mouth or beak, on side −1 (viewer's left) or 1.
    Mouth(f32),
    /// Where the two hands meet in front.
    Hands,
    /// The tip of hand 0 or 1.
    Hand(usize),
    /// Above the head.
    Top,
    /// On the ground between the feet.
    Feet,
}

/// What the beat adds, born at an anchor; `t` its age 0…1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Fx {
    Sparkle { at: Anchor, t: f32 },
    Note { at: Anchor, t: f32, kind: u32 },
    Dust { t: f32 },
    Confetti { t: f32 },
}

/// How joyful the extras are: none at low energy, all at full (subtle, then joyful).
pub fn fx_strength(pose: &Pose) -> f32 {
    smooth((pose.energy - 0.3) / 0.6)
}

/// A bob on every beat: down on it (squashed), up between (stretched on the way).
fn bob(p: f32, e: f32, st: &Style, k: f32) -> (f32, f32) {
    let x = e * e;
    let y = -st.bounce * (0.35 + 0.85 * x) * k * arc(p);
    let sq = st.squash * (0.35 + 0.95 * x) * k * (pulse(p, 0.0, 0.3) - 0.55 * pulse(p, 0.32, 0.28));
    (y, sq)
}

/// How far a move goes to its extremes: the square of its vigour — about 0.12 at low
/// energy (subtle), 1 at full (the silhouette that tells the move as a thumbnail).
pub fn extreme(pose: &Pose) -> f32 {
    let e = vigour(pose);
    e * e
}

/// What hangs on the body follows the bob late: up when the body falls, down as it rises.
fn hang(p: f32, e: f32, st: &Style, k: f32) -> f32 {
    -k * e * wave_c(lag(p, st.lag))
}

/// Idle: a breath, a glance about every few seconds, a tail that flicks now and then — on
/// the song's seconds (or the wall clock, stopped).
pub fn idle(pose: &Pose, st: &Style) -> Body {
    let t = pose.seconds as f32;
    let breath = math::sin(t * TAU / 3.2);
    const GLANCE: f32 = 2.9;
    let n = (t / GLANCE).floor();
    let target = |k: f32| (hash(k as i64) - 0.5) * 1.3;
    let move_in = smooth((t - n * GLANCE) / 0.35);
    let look_x = lerp(target(n - 1.0), target(n), move_in);
    let flick = pulse((t / 4.3).rem_euclid(1.0), 0.5, 0.12);
    let mut b = Body::rest();
    b.squash = 0.025 * breath * (st.squash / 0.12);
    b.y = -0.004 * breath;
    b.arm = [9.0 + 3.0 * breath, 9.0 + 3.0 * breath];
    b.look = (look_x, 0.05);
    b.tilt = 3.0 * math::sin(t * TAU / 5.3) + look_x * 4.0;
    b.tail = 6.0 * math::sin(t * TAU / 2.1) + 14.0 * flick;
    b.top = 4.0 * math::sin(t * TAU / 2.6 + 1.0) - look_x * 5.0;
    b
}

/// A move as the family dances it (`p` the phase, 0 = on the beat).
pub fn dance(mv: Move, p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    let e = vigour(pose);
    let x = extreme(pose);
    let s = side(cycle);
    let mut b = Body::rest();
    match mv {
        Move::Idle => return idle(pose, st),
        Move::Bounce => {
            (b.y, b.squash) = bob(p, e, st, 1.0);
            let a = lag(p, 0.06);
            b.arm = [16.0 + (20.0 + 44.0 * x) * st.arms * arc(a); 2];
            b.bend = [-22.0 * arc(a); 2];
            b.spread = [0.3 + 0.6 * arc(a); 2];
            b.tilt = (4.0 + 9.0 * x) * s * arc(p);
            b.nod = -6.0 * pulse(p, 0.0, 0.3) + 4.0 * arc(p);
            b.top = 16.0 * hang(p, e, st, 1.0);
            b.tail = 12.0 * e * wave(lag(p, st.lag * 0.7));
        }
        Move::StepLeft | Move::StepRight => {
            let d = if mv == Move::StepLeft { -1.0 } else { 1.0 };
            let lead = if d < 0.0 { 0 } else { 1 };
            let out = |n: u32| if n.is_multiple_of(2) { 1.0 } else { 0.0 };
            let (from, to) = (out(cycle), out(cycle.wrapping_add(1)));
            let travel = ease_in_out(window(p, 0.5, 1.0));
            let w = lerp(from, to, travel);
            let lift = (0.04 + 0.08 * x) * arc(window(p, 0.5, 1.0));
            b.foot[lead] = (d * st.stride * e * w, -lift);
            let weight = lerp(from, to, smooth(window(p, 0.35, 1.0)));
            b.x = d * st.sway * e * weight;
            let (y, sq) = bob(p, e, st, 0.6);
            b.y = y;
            b.squash = sq;
            b.lean = d * (4.0 + 8.0 * x) * weight;
            // The head leads: the ball turns toward the step before the weight arrives.
            let lead_w = lerp(from, to, smooth(window(p, 0.2, 0.8)));
            b.yaw = d * (6.0 + 14.0 * x) * lead_w;
            b.arm[lead] = 18.0 + 42.0 * st.arms * e * weight;
            b.arm[1 - lead] = 14.0 + 10.0 * (1.0 - weight);
            b.bend[lead] = -18.0 * weight;
            b.spread[lead] = 0.3 + 0.6 * weight;
            b.look = (d * 0.65, 0.0);
            b.tilt = d * 6.0 * weight;
            b.top = -d * 12.0 * e * smooth(window(lag(p, st.lag), 0.5, 1.0)) * (to - from)
                + 10.0 * hang(p, e, st, 0.6);
            b.tail = d * 16.0 * e * (weight - 0.5);
        }
        Move::Sway => {
            let target = |n: u32| side(n);
            let k = lerp(target(cycle), target(cycle.wrapping_add(1)), ease_in_out(p));
            b.x = k * st.sway * (0.7 + 0.4 * x);
            b.lean = k * (5.0 + 9.0 * x);
            let ahead = lerp(
                target(cycle),
                target(cycle.wrapping_add(1)),
                ease_in_out((p + 0.1).min(1.0)),
            );
            b.yaw = ahead * (5.0 + 10.0 * x);
            b.y = -0.012 * arc(p);
            b.arm = [28.0 + 22.0 * k * st.arms, 28.0 - 22.0 * k * st.arms];
            b.bend = [-12.0 * k, 12.0 * k];
            b.spread = [0.5; 2];
            b.tilt = -k * 7.0;
            b.look = (k * 0.3, -0.1);
            let kl = lerp(
                target(cycle),
                target(cycle.wrapping_add(1)),
                ease_in_out(lag(p, st.lag)),
            );
            b.top = -kl * 12.0 * e;
            b.tail = kl * 16.0 * e;
            b.squint = 0.3;
        }
        Move::Clap => {
            let (open, shut) = ((52.0 + 36.0 * x) * st.arms.min(1.4), -70.0);
            let a = if p < 0.45 {
                lerp(shut, open, ease_out(p / 0.45))
            } else {
                lerp(open, shut, ease_in(window(p, 0.45, 1.0)))
            };
            b.arm = [a, a];
            let hit = pulse(p, 0.0, 0.22);
            b.bend = [26.0 * (1.0 - window(p, 0.0, 0.4)) + 10.0; 2];
            let openness = ((a - shut) / (open - shut)).clamp(0.0, 1.0);
            b.spread = [(0.2 + 0.8 * openness).max(hit); 2];
            (b.y, b.squash) = bob(p, e, st, 0.8);
            b.mouth = 0.45 * hit;
            b.squint = 0.6 * hit;
            b.tilt = 4.0 * s;
            b.top = 14.0 * hang(p, e, st, 1.0);
            b.tail = 10.0 * e * wave(lag(p, st.lag));
        }
        Move::Wave => {
            let w = if s > 0.0 { 1 } else { 0 };
            let ws = if w == 1 { 1.0 } else { -1.0 };
            let q = (p * 2.0).rem_euclid(1.0);
            let raise = ease_out_back(window(p, 0.0, 0.12));
            b.arm[w] = lerp(10.0, 148.0 + 16.0 * wave(p * 2.0), raise);
            b.bend[w] = 28.0 * wave(p * 2.0 - 0.14);
            b.spread[w] = 1.0;
            b.arm[1 - w] = -22.0;
            b.bend[1 - w] = 64.0;
            b.spread[1 - w] = 0.2;
            let (y, sq) = bob(q, e, st, 0.7);
            b.y = y;
            b.squash = sq;
            b.x = 0.02 * ws;
            b.lean = -(4.0 + 8.0 * x) * ws;
            b.tilt = (7.0 + 6.0 * x) * ws;
            b.yaw = (6.0 + 8.0 * x) * ws;
            b.wink[w] = pulse(p, 0.82, 0.22);
            b.mouth = 0.2;
            b.top = 10.0 * hang(q, e, st, 1.0) - 6.0 * ws;
            b.tail = 12.0 * e * wave(lag(q, st.lag));
            // Hands change sides every two beats: the wave comes down to meet the next.
            let mut rest = Body::rest();
            rest.arm = [24.0, 24.0];
            rest.spread = [0.5; 2];
            b = Body::lerp(&rest, &b, raise);
            b = Body::lerp(&b, &rest, ease_in_out(window(p, 0.88, 1.0)));
        }
        Move::Jump => return jump(p, pose, st),
        Move::Spin => return spin(p, pose, st),
        Move::Kick => return kick(p, cycle, pose, st),
        _ => return more(mv, p, cycle, pose, st),
    }
    b
}

/// A jump over two beats: settle from the landing, crouch, spring, fly, land on the beat.
pub fn jump(p: f32, pose: &Pose, st: &Style) -> Body {
    let e = vigour(pose);
    let x = extreme(pose);
    let mut b = Body::rest();
    let u = window(p, 0.55, 1.0);
    let air = if p >= 0.55 { arc(u) } else { 0.0 };
    b.y = -st.hop * (0.45 + 0.55 * x) * air;
    let land = if p < 0.2 {
        1.0 - ease_out(p / 0.2)
    } else {
        0.0
    };
    let crouch = smooth(window(p, 0.36, 0.52)) * (1.0 - window(p, 0.52, 0.58));
    let take_off = if (0.52..0.75).contains(&p) {
        pulse(p, 0.6, 0.2)
    } else {
        0.0
    };
    b.squash = st.squash * (0.4 + 0.9 * x) * (1.4 * land + 1.3 * crouch - 1.1 * take_off);
    b.y += (0.01 + 0.03 * x) * crouch;
    let up = ease_out(window(p, 0.5, 0.68));
    let down = ease_in(window(p, 0.86, 1.0));
    let settle = ease_out(window(p, 0.0, 0.3));
    let arms = if p < 0.5 {
        lerp(lerp(48.0, 14.0, settle), -8.0, crouch)
    } else {
        lerp(lerp(-8.0, 165.0, up), 48.0, down)
    };
    b.arm = [arms; 2];
    b.spread = [0.3 + 0.7 * air; 2];
    b.bend = [-20.0 * air; 2];
    b.foot = [(-0.03 * air, -0.08 * air), (0.03 * air, -0.08 * air)];
    b.mouth = 0.55 * air;
    b.squint = 0.5 * crouch + 0.4 * land;
    b.look = (0.0, -0.4 * air);
    b.nod = 10.0 * air - 8.0 * crouch;
    // What hangs on it trails the motion: down on the way up, up on the way down.
    let v = if p >= 0.55 { 1.0 - 2.0 * u } else { 0.0 };
    b.top = -26.0 * v * e + 18.0 * land;
    b.tail = 20.0 * v * e - 10.0 * crouch;
    b
}

/// A spin over two beats: wind up, whip round (the ball turns, its face goes round the
/// back), then a little hop and a ta-da that lands on the next beat.
pub fn spin(p: f32, pose: &Pose, st: &Style) -> Body {
    let e = vigour(pose);
    let mut b = Body::rest();
    let wind = ease_out(window(p, 0.0, 0.16));
    let u = window(p, 0.16, 0.78);
    let turn = ease_in_out(u);
    let yaw = if p < 0.16 {
        -30.0 * wind
    } else {
        lerp(-30.0, 360.0, turn)
    };
    b.yaw = (yaw + 180.0).rem_euclid(360.0) - 180.0;
    let speed = if (0.16..0.78).contains(&p) {
        6.0 * u * (1.0 - u)
    } else {
        0.0
    };
    let tada = ease_out_back(window(p, 0.78, 0.9));
    b.arm = if p < 0.16 {
        [lerp(10.0, -10.0, wind); 2]
    } else if p < 0.8 {
        [lerp(-10.0, 88.0, smooth(window(p, 0.16, 0.3))); 2]
    } else {
        [lerp(lerp(88.0, 140.0, tada), 10.0, smooth(window(p, 0.9, 1.0))); 2]
    };
    let down = smooth(window(p, 0.9, 1.0));
    b.spread = [lerp(0.4 + 0.6 * tada, 0.3, down); 2];
    b.squash = st.squash * e * (0.8 * wind * (1.0 - u) - 0.5 * speed / 1.5);
    let hop = arc(window(p, 0.8, 1.0));
    b.y = -st.hop * 0.45 * e * hop - 0.02 * speed;
    b.top = -30.0 * speed / 1.5 * e;
    b.tail = 34.0 * speed / 1.5 * e;
    b.mouth = 0.25 * speed / 1.5 + 0.4 * tada * (1.0 - window(p, 0.97, 1.0));
    b.squint = 0.5 * tada * (1.0 - down);
    b
}

/// A kick to the side on every beat, legs taking turns: the kicking foot is out on the
/// beat, comes back, and the other one chambers and kicks on the next.
pub fn kick(p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    let e = vigour(pose);
    let x = extreme(pose);
    let mut b = Body::rest();
    let now = (cycle % 2) as usize;
    let next = 1 - now;
    let dir = |i: usize| if i == 0 { -1.0 } else { 1.0 };
    let out = |i: usize| (dir(i) * st.stride * (1.1 + 0.7 * x), -(0.08 + 0.12 * x));
    let chamber = |i: usize| (dir(i) * 0.03, -(0.07 + 0.08 * x));
    let back = ease_in_out(window(p, 0.0, 0.42));
    let o = out(now);
    b.foot[now] = (o.0 * (1.0 - back), o.1 * (1.0 - back));
    let ch = ease_out(window(p, 0.45, 0.78));
    let ext = ease_in(window(p, 0.78, 1.0));
    let (c, o2) = (chamber(next), out(next));
    b.foot[next] = lerp_p(lerp_p((0.0, 0.0), c, ch), o2, ext);
    let w = smooth(window(p, 0.42, 1.0));
    let lean_of = |i: usize| -dir(i) * (4.0 + 8.0 * x);
    b.lean = lerp(lean_of(now), lean_of(next), w);
    b.x = lerp(-dir(now) * 0.02, -dir(next) * 0.02, w);
    // A hop on the standing foot, between the kicks.
    b.y = -(0.02 + 0.07 * x) * arc(p);
    b.squash = st.squash * (0.3 + 0.6 * x) * (pulse(p, 0.0, 0.25) - 0.5 * pulse(p, 0.5, 0.3));
    b.yaw = lerp(dir(now), dir(next), w) * (4.0 + 10.0 * x);
    b.arm[now] = lerp(70.0 * st.arms, 20.0, back);
    b.arm[next] = lerp(20.0, 70.0 * st.arms, w);
    b.bend = [-15.0; 2];
    b.spread = [0.6; 2];
    b.mouth = 0.35 * pulse(p, 0.0, 0.22);
    b.tilt = lerp(dir(now), dir(next), w) * 5.0;
    b.top = 14.0 * hang(p, e, st, 1.0);
    b.tail = lerp(dir(now), dir(next), w) * 14.0;
    b
}

/// The rest of the moves: shuffle, point, arms up, twist, bow, freeze.
fn more(mv: Move, p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    let e = vigour(pose);
    let x = extreme(pose);
    let s = side(cycle);
    let mut b = Body::rest();
    match mv {
        Move::Shuffle => {
            // Double time: a foot taps out on every half beat, arms pump like running.
            let q = (p * 2.0).rem_euclid(1.0);
            let half = cycle.wrapping_mul(2).wrapping_add(u32::from(p >= 0.5));
            let f = (half % 2) as usize;
            let fd = if f == 0 { -1.0 } else { 1.0 };
            b.foot[f] = (fd * (0.04 + 0.04 * x) * arc(q), -(0.04 + 0.07 * x) * arc(q));
            b.foot[1 - f] = (-fd * 0.015 * e * arc(q), 0.0);
            b.y = -(0.02 + 0.06 * x) * arc(q);
            b.squash =
                st.squash * (0.3 + 0.7 * x) * (pulse(q, 0.0, 0.3) - 0.4 * pulse(q, 0.5, 0.3));
            let pump = wave(p);
            b.arm = [40.0 + 28.0 * pump * st.arms, 40.0 - 28.0 * pump * st.arms];
            b.bend = [55.0; 2];
            b.spread = [0.3; 2];
            b.x = 0.018 * e * pump;
            b.tilt = 4.0 * wave(p);
            b.top = 12.0 * hang(q, e, st, 1.0);
            b.tail = 14.0 * e * wave(lag(p, st.lag));
            b.squint = 0.25;
        }
        Move::Point => {
            // Up to the right on one beat, down across on the next; it snaps there and
            // winks once a bar.
            let target = |n: u32| if n.is_multiple_of(2) { 142.0 } else { -38.0 };
            let go = ease_in_out(window(p, 0.62, 1.0));
            let u = window(p, 0.0, 0.45);
            let snap = 10.0 * math::exp(-4.5 * u) * math::sin(u * 1.5 * TAU);
            let up_now = cycle.is_multiple_of(2);
            b.arm[1] = lerp(target(cycle), target(cycle.wrapping_add(1)), go)
                + snap * if up_now { 1.0 } else { -1.0 };
            b.bend[1] = -8.0;
            b.spread[1] = 0.15;
            b.arm[0] = -24.0;
            b.bend[0] = 66.0;
            b.spread[0] = 0.2;
            let k = lerp(s, -s, go);
            b.lean = -k * (4.0 + 8.0 * x);
            b.x = k * (0.02 + 0.03 * x);
            // The head turns to where the hand points.
            b.yaw = k * (6.0 + 14.0 * x);
            let (y, sq) = bob(p, e, st, 0.7);
            b.y = y;
            b.squash = sq;
            b.look = (
                lerp(0.55, -0.35, (1.0 - k) * 0.5),
                lerp(-0.6, 0.5, (1.0 - k) * 0.5),
            );
            b.tilt = k * 8.0;
            if cycle % 4 == 2 {
                // A cheeky look at the camera, and a wink.
                b.look = lerp_p(b.look, (0.0, 0.0), pulse(p, 0.3, 0.58));
                b.wink[1] = pulse(p, 0.22, 0.26);
            }
            b.mouth = 0.12;
            b.top = 10.0 * hang(p, e, st, 1.0);
            b.tail = -k * 12.0 * e;
        }
        Move::ArmsUp => {
            // Hands in the air, pushed up on the beat and swaying bar-wide, singing.
            let push = pulse(p, 0.0, 0.32);
            let sway = lerp(s, -s, ease_in_out(p));
            let lift = 118.0 + 20.0 * x;
            b.arm = [
                lift + 16.0 * push + 14.0 * sway,
                lift + 16.0 * push - 14.0 * sway,
            ];
            b.bend = [-10.0 - 12.0 * push; 2];
            b.spread = [1.0; 2];
            (b.y, b.squash) = bob(p, e, st, 1.1);
            b.x = sway * (0.015 + 0.02 * x);
            b.lean = sway * (3.0 + 6.0 * x);
            b.tilt = sway * 7.0;
            b.mouth = 0.5 + 0.3 * push;
            b.squint = 0.45;
            b.nod = 8.0;
            b.top = 14.0 * hang(p, e, st, 1.0);
            b.tail = sway * 14.0 * e;
        }
        Move::Twist => {
            // Two beats: the body turns to each side on each beat and sinks low and back.
            let c = wave_c(p);
            b.yaw = (24.0 + 26.0 * x) * c;
            b.lean = c * (3.0 + 5.0 * x);
            b.arm = [46.0 - 26.0 * c * st.arms, 46.0 + 26.0 * c * st.arms];
            b.bend = [42.0; 2];
            b.spread = [0.4; 2];
            let low = arc(p);
            b.y = (0.02 + 0.05 * x) * low;
            b.squash = st.squash * (0.4 + 0.9 * x) * low;
            b.foot = [(-0.022 * c, 0.0), (0.022 * c, 0.0)];
            b.tilt = -6.0 * c;
            b.look = (c * 0.4, 0.0);
            b.top = -14.0 * e * wave_c(lag(p, st.lag));
            b.tail = 18.0 * e * wave_c(lag(p, st.lag));
            b.squint = 0.3;
        }
        Move::Bow => {
            let down = ease_in_out(window(p, 0.08, 0.42));
            let up = ease_out_back(window(p, 0.7, 0.98));
            let k = down * (1.0 - up);
            // A deep bow: the face goes down toward the floor, the body folds.
            b.nod = -(28.0 + 36.0 * x) * k;
            b.squash = (0.08 + 0.12 * x) * k;
            b.y = (0.02 + 0.04 * x) * k;
            b.arm = [lerp(10.0, 68.0 + 26.0 * x, k), lerp(10.0, -58.0, k)];
            b.bend = [lerp(0.0, -22.0, k), lerp(0.0, 46.0, k)];
            b.spread = [lerp(0.3, 1.0, k), 0.2];
            b.wink = [0.92 * k; 2];
            b.top = 24.0 * smooth(window(lag(p, st.lag), 0.1, 0.45)) * (1.0 - up);
            b.tail = -10.0 * k;
            b.mouth = 0.25 * up * (1.0 - window(p, 0.95, 1.0));
            b.squint = 0.4 * up;
        }
        Move::Freeze => {
            // Strike a pose on the beat and hold it, breathing.
            let hit = if cycle == 0 {
                ease_out_back(window(p, 0.0, 0.05))
            } else {
                1.0
            };
            let t = pose.seconds as f32;
            let breath = 0.012 * math::sin(t * TAU / 2.4);
            let mut f = Body::rest();
            f.arm = [-30.0, 156.0];
            f.bend = [70.0, -10.0];
            f.spread = [0.2, 1.0];
            f.lean = 6.0 + 8.0 * x;
            f.x = 0.03;
            f.yaw = 10.0 * x;
            f.foot = [(-0.06, 0.0), (0.04, -0.02)];
            f.tilt = -9.0;
            f.squint = 0.35;
            f.squash = breath;
            f.top = -10.0;
            f.tail = 12.0;
            f.mouth = 0.1;
            b = Body::lerp(&Body::rest(), &f, hit);
        }
        _ => {}
    }
    b
}

/// The extras a move adds at this moment (for the character to draw at its anchors).
pub fn extras(mv: Move, p: f32, cycle: u32, pose: &Pose, out: &mut dyn FnMut(Fx)) {
    if fx_strength(pose) <= 0.0 {
        return;
    }
    match mv {
        Move::Clap if p < 0.35 => out(Fx::Sparkle {
            at: Anchor::Hands,
            t: p / 0.35,
        }),
        Move::ArmsUp => out(Fx::Note {
            at: Anchor::Mouth(side(cycle)),
            t: p,
            kind: cycle,
        }),
        Move::Jump => {
            if p < 0.3 {
                out(Fx::Dust { t: p / 0.3 });
            }
            if p >= 0.66 && pose.energy > 0.6 {
                out(Fx::Confetti {
                    t: window(p, 0.66, 1.0),
                });
            }
        }
        Move::Spin if p >= 0.8 => {
            let t = window(p, 0.8, 1.0);
            out(Fx::Sparkle {
                at: Anchor::Hand(0),
                t,
            });
            out(Fx::Sparkle {
                at: Anchor::Hand(1),
                t: (t - 0.15).max(0.0),
            });
        }
        Move::Freeze if p < 0.12 && cycle == 0 => out(Fx::Sparkle {
            at: Anchor::Hand(1),
            t: p / 0.12,
        }),
        Move::Idle | Move::Freeze | Move::Bow => {}
        _ => {
            // A little note on the bar's first beat, from alternate sides bar by bar.
            if pose.beat_in_bar == 0 && pose.energy > 0.5 {
                let side = if pose.bar.rem_euclid(2) == 0 {
                    1.0
                } else {
                    -1.0
                };
                out(Fx::Note {
                    at: Anchor::Mouth(side),
                    t: pose.beat_phase,
                    kind: pose.bar.rem_euclid(8) as u32,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cast, pose_at, DanceFrame};

    fn pose() -> Pose {
        let ch = cast::default_character();
        pose_at(
            &DanceFrame {
                playing: true,
                ..DanceFrame::default()
            },
            ch.as_ref(),
            0.0,
        )
    }

    #[test]
    fn every_move_is_continuous_across_its_cycle() {
        // The end of a cycle meets the start of the next (no pop on the beat).
        let st = Style::default();
        let po = pose();
        for mv in Move::ALL {
            if mv == Move::Idle {
                continue;
            }
            for cycle in 0..4 {
                let a = dance(mv, 0.9999, cycle, &po, &st);
                let b = dance(mv, 0.0, cycle.wrapping_add(1), &po, &st);
                let d = |x: f32, y: f32| (x - y).abs();
                let worst = [
                    d(a.x, b.x) * 100.0,
                    d(a.y, b.y) * 100.0,
                    d(a.arm[0], b.arm[0]),
                    d(a.arm[1], b.arm[1]),
                    d(a.foot[0].0, b.foot[0].0) * 100.0,
                    d(a.foot[1].0, b.foot[1].0) * 100.0,
                    d(a.foot[0].1, b.foot[0].1) * 100.0,
                    d(a.foot[1].1, b.foot[1].1) * 100.0,
                    d(a.lean, b.lean),
                    (a.yaw - b.yaw)
                        .rem_euclid(360.0)
                        .min((b.yaw - a.yaw).rem_euclid(360.0)),
                ]
                .into_iter()
                .fold(0.0f32, f32::max);
                assert!(
                    worst < 2.5,
                    "{mv:?} jumps at the end of cycle {cycle}: {a:?} vs {b:?}"
                );
            }
        }
    }

    #[test]
    fn a_long_song_never_overflows_the_cycle() {
        // A note held for ever, a song played for days: the cycle count reaches its end and
        // wraps; no move panics, and the last cycles still meet.
        let st = Style::default();
        let po = pose();
        for mv in Move::ALL {
            for cycle in [u32::MAX - 1, u32::MAX] {
                for k in 0..8 {
                    let _ = dance(mv, k as f32 / 8.0, cycle, &po, &st);
                }
            }
            if mv != Move::Idle {
                let a = dance(mv, 0.9999, u32::MAX - 1, &po, &st);
                let b = dance(mv, 0.0, u32::MAX, &po, &st);
                assert!(
                    (a.arm[0] - b.arm[0]).abs() < 2.5,
                    "{mv:?} at the end of the count"
                );
            }
            let mut out = |_f: Fx| {};
            let late = Pose {
                bar: i32::MAX,
                ..po
            };
            extras(mv, 0.5, u32::MAX, &late, &mut out);
        }
    }

    #[test]
    fn steps_land_on_the_beat() {
        // On the beat the stepping foot is down; half way it is in the air.
        let st = Style::default();
        let po = pose();
        let b = dance(Move::StepLeft, 0.0, 0, &po, &st);
        assert_eq!(b.foot[0].1, 0.0);
        let mid = dance(Move::StepLeft, 0.75, 0, &po, &st);
        assert!(mid.foot[0].1 < -0.01, "in the air between beats");
        let k = kick(0.0, 0, &po, &st);
        assert!(k.foot[0].1 < -0.05, "a kick is out on the beat");
    }
}
