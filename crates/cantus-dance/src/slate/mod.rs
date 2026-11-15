//! The Slate cast's rig (round 40, ADR 0122): the twelve characters the owner approved for
//! the Slate look (`docs/round40/design/dancer/`), drawn from the design rig ported number
//! for number, so the app draws what he approved.
//!
//! - [`fig`]: a figure's pose ([`Fig`]), the design's easing, two-bone IK, snap time;
//! - [`body`]: proportions ([`Props`]) and the joints of a pose, with the ground rule;
//! - [`moves`]: the shared choreography (groove, step-and-touch, the jump on the drop, and
//!   the open-silhouette moves);
//! - [`paint`]: the pieces — straight bars, rounded rectangles, the room-coloured cut;
//! - this file: a [`Figure`] (what a Slate character says about itself) made a
//!   [`Character`] by [`Slate`]: the pose of the song turned into a figure — its own moves
//!   for the groove (`Bounce`), the step-and-touch (`StepLeft`/`StepRight`) and its
//!   signature on the drop (`Jump`), the family's choreography (`rig::dance`) for every
//!   other move, adapted — blended between moves, sized by the energy, with the mood's face.
//!
//! The theme's light is the canvas's (`render::StageInk`): ink, gold, the room. Everything
//! is a pure function of the pose (ADR 0087 §2) and calls `cantus_dsp::math` (ADR 0031 R9).

pub mod body;
pub mod fig;
pub mod moves;
pub mod paint;

pub use body::{place, solve, Joints, Props};
pub use fig::{Eyes, Fig};

use crate::character::{Bounds, Character, CharacterInfo};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{self, Body};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use tiny_skia::Transform;

/// What a Slate character says about itself; [`Slate`] makes it a [`Character`].
pub trait Figure: Send + Sync {
    fn info(&self) -> &CharacterInfo;
    fn props(&self) -> &Props;
    /// The family's style for the moves it takes from the family's choreography.
    fn family(&self) -> &dance::Style;
    /// Whether its arms go by angle and bend (Compás's straight bars) rather than by hand
    /// targets.
    fn arms_by_angle(&self) -> bool {
        false
    }
    /// Its own move `mv`, `b` beats into it (`Bounce` 2 beats, the steps 4, `Jump` 2):
    /// `None` takes the family's.
    fn own(&self, mv: Move, b: f32) -> Option<Fig>;
    /// The routine it dances when nothing chooses a move (one move a bar).
    fn routine(&self) -> &[Move];
    fn bounds(&self) -> Bounds;
    /// Draw `fig` (already the whole moment) onto `c`.
    fn draw(&self, fig: &Fig, pose: &Pose, c: &mut Canvas);
    /// What a mood adds beyond the eyes ([`mood_eyes`]): its own eighth mood (Bajo's blast,
    /// Metrónomo's monocle off…), a level, a posture. Its words are the locale files'
    /// `cast.<id>.mood.<mood>` where it names a mood its own way.
    fn mood(&self, _mood: Mood, _f: &mut Fig) {}

    /// Whether it shows the master's spectrum (`Pose::bands`).
    fn hears_spectrum(&self) -> bool {
        false
    }

    /// How many earlier moments it draws behind it (Compás's motion marks) and how far
    /// apart they are, in beats.
    fn trails(&self) -> (usize, f32) {
        (0, 0.0)
    }
}

/// A Slate character.
pub struct Slate<F: Figure>(pub F);

/// A move's natural cycle in quarter notes for the Slate cast: the groove two beats, the
/// step-and-touch four, the drop four — its signature in the first two, landing on the
/// third, then the groove (the design's bar on the drop).
pub fn move_beats(m: Move) -> f32 {
    match m {
        Move::Bounce => 2.0,
        Move::StepLeft | Move::StepRight | Move::Jump => 4.0,
        Move::Spin | Move::Bow | Move::Twist | Move::Wave => 2.0,
        Move::Freeze | Move::Idle => 4.0,
        _ => 1.0,
    }
}

/// How far down the pelvis sits in the family's moves: their bob rises this much before it
/// lifts the feet.
const BASE_DIP: f32 = 0.03;

/// How long a change of move blends inside a move (the drop into the groove), in beats.
const BLEND: f32 = 0.25;

/// The family's body as a figure: the moves a Slate character takes from `rig::dance`.
pub fn from_body(b: &Body, pr: &Props, by_angle: bool) -> Fig {
    let l = pr.leg;
    let y = b.y + BASE_DIP;
    let mut f = Fig {
        hip: (b.x / l, y.max(0.0) / l),
        lift: (-y).max(0.0),
        lean: b.lean,
        squash: b.squash * 0.7,
        turn: crate::rig::sin_deg(b.yaw) * 0.8,
        nod: -b.nod,
        tilt: b.tilt,
        feet: [
            (b.foot[0].0 / l, b.foot[0].1 / l),
            (b.foot[1].0 / l, b.foot[1].1 / l),
        ],
        mouth: b.mouth,
        point: b.top,
        ..Fig::default()
    };
    f.eyes = if b.wink[0] > 0.5 || b.wink[1] > 0.5 {
        Eyes::Wink
    } else if b.squint > 0.4 {
        Eyes::Happy
    } else {
        Eyes::Open
    };
    let fk = [(b.arm[0], -b.bend[0]), (b.arm[1], -b.bend[1])];
    if by_angle {
        f.fk = Some(fk);
    } else {
        f.hands = std::array::from_fn(|i| hand_of(fk[i], i, pr));
    }
    f
}

/// Where a hand by angle and bend lands, as a target from the neck (A units).
fn hand_of((a, bend): (f32, f32), i: usize, pr: &Props) -> crate::rig::P {
    let side = if i == 0 { -1.0 } else { 1.0 };
    let dir = |deg: f32| {
        let (s, c) = math::sin_cos(crate::rig::rad(deg));
        (side * s, c)
    };
    let (d1, d2) = (dir(a), dir(a + bend));
    let x = side * pr.shoulder + d1.0 * pr.upper + d2.0 * pr.fore;
    let y = d1.1 * pr.upper + d2.1 * pr.fore;
    (x / pr.arm(), y / pr.arm())
}

impl<F: Figure> Slate<F> {
    /// The whole moment ([`fig_of`]).
    pub fn fig(&self, pose: &Pose) -> Fig {
        fig_of(&self.0, pose)
    }

    /// The current move `beats_ago` earlier ([`fig_before`]).
    pub fn fig_before(&self, pose: &Pose, beats_ago: f32) -> Fig {
        fig_before(&self.0, pose, beats_ago)
    }
}

/// The figure of move `mv` at `phase` of its cycle (`cycle` whole cycles in).
pub fn move_fig<F: Figure + ?Sized>(ch: &F, mv: Move, phase: f32, cycle: u32, pose: &Pose) -> Fig {
    let mut b = phase * move_beats(mv);
    if mv == Move::StepLeft {
        // The step-and-touch from its other half: a step to the left first.
        b += 2.0;
    }
    let mut f = if mv == Move::Jump && b >= 2.0 {
        // Landed: the groove takes over in a quarter beat (the design's blend).
        let groove = own_or_family(ch, Move::Bounce, b - 2.0, pose);
        if b < 2.0 + BLEND {
            let landed = own_or_family(ch, Move::Jump, b, pose);
            Fig::blend(&landed, &groove, fig::smooth((b - 2.0) / BLEND))
        } else {
            groove
        }
    } else {
        match ch.own(mv, b) {
            Some(f) => f,
            None => {
                let body = dance::dance(mv, phase, cycle, pose, ch.family());
                from_body(&body, ch.props(), ch.arms_by_angle())
            }
        }
    };
    f.t = b;
    f
}

/// Its own move at `b` beats, or the family's at the same moment.
fn own_or_family<F: Figure + ?Sized>(ch: &F, mv: Move, b: f32, pose: &Pose) -> Fig {
    ch.own(mv, b).unwrap_or_else(|| {
        let beats = move_beats(mv);
        let (phase, cycle) = ((b / beats).rem_euclid(1.0), (b / beats).floor() as u32);
        let body = dance::dance(mv, phase, cycle, pose, ch.family());
        from_body(&body, ch.props(), ch.arms_by_angle())
    })
}

/// The whole moment: the move (blended from the one it leaves), sized by the energy,
/// turned by the knob, with the mood's face.
pub fn fig_of<F: Figure + ?Sized>(ch: &F, pose: &Pose) -> Fig {
    let now = move_fig(ch, pose.mv, pose.phase, pose.cycle, pose);
    let mut f = match pose.from {
        Some((from, phase)) if pose.blend < 1.0 => {
            let was = move_fig(ch, from, phase, pose.from_cycle, pose);
            Fig::blend(&was, &now, pose.blend)
        }
        _ => now,
    };
    finish(ch, &mut f, pose);
    f
}

/// The moment `beats_ago` earlier (a motion mark): the moves (and the one it leaves) that
/// much further back, the blend as it is now — as [`fig_of`].
pub fn fig_before<F: Figure + ?Sized>(ch: &F, pose: &Pose, beats_ago: f32) -> Fig {
    let back = |mv: Move, phase: f32, cycle: u32| {
        let mut phase = phase - beats_ago / move_beats(mv);
        let mut cycle = cycle;
        while phase < 0.0 {
            phase += 1.0;
            cycle = cycle.wrapping_sub(1);
        }
        (phase, cycle)
    };
    let (phase, cycle) = back(pose.mv, pose.phase, pose.cycle);
    let from = pose.from.map(|(m, p)| {
        let (p, c) = back(m, p, pose.from_cycle);
        (m, p, c)
    });
    let earlier = Pose {
        phase,
        cycle,
        from: from.map(|(m, p, _)| (m, p)),
        from_cycle: from.map_or(pose.from_cycle, |(_, _, c)| c),
        ..*pose
    };
    fig_of(ch, &earlier)
}

fn finish<F: Figure + ?Sized>(ch: &F, f: &mut Fig, pose: &Pose) {
    // The energy sizes the motion (the design's at the default, 0.7; calmer below).
    let k = fig::lerp(0.55, 1.0, fig::smooth(pose.energy / 0.7));
    if k < 1.0 {
        let keep = *f;
        let rest = Fig {
            fk: f.fk.map(|_| [(8.0, 4.0), (8.0, 4.0)]),
            ..Fig::default()
        };
        *f = Fig::blend(&rest, &keep, k);
        f.eyes = keep.eyes;
        (f.accent, f.level, f.sig, f.shape, f.t) =
            (keep.accent, keep.level, keep.sig, keep.shape, keep.t);
    }
    f.turn = (f.turn + pose.turn * 0.6).clamp(-1.0, 1.0);
    if !pose.playing {
        f.accent = 0.0;
    }
    let e = (pose.energy / 0.7).clamp(0.0, 1.3);
    f.level = (f.level * (0.45 + 0.55 * e)).clamp(0.0, 1.0);
    f.eyes = mood_eyes(f.eyes, pose.mood);
    if pose.mood == Mood::Excited {
        f.level = (f.level + 0.3).min(1.0);
    }
    ch.mood(pose.mood, f);
    if pose.blink > 0.5 && matches!(f.eyes, Eyes::Open | Eyes::Happy | Eyes::Wide) {
        f.eyes = Eyes::Closed;
    }
}

/// The eyes a mood gives a move's own: Happy (the default) keeps the move's, Neutral and
/// Excited lean them, the others are the mood's.
pub fn mood_eyes(of_move: Eyes, mood: Mood) -> Eyes {
    match mood {
        Mood::Happy => of_move,
        Mood::Neutral => match of_move {
            Eyes::Happy => Eyes::Open,
            e => e,
        },
        Mood::Excited => match of_move {
            Eyes::Open => Eyes::Happy,
            e => e,
        },
        Mood::Calm => Eyes::Closed,
        Mood::Surprised => Eyes::Wide,
        Mood::Sleepy => Eyes::Sleepy,
        Mood::Cool => Eyes::Cool,
        Mood::Shy => Eyes::Wink,
    }
}

/// Draw `f` with the canvas on the ground under the character (its shadow, a ring, dust):
/// the renderer's lift and spin undone.
pub fn on_ground(c: &mut Canvas, pose: &Pose, f: impl FnOnce(&mut Canvas)) {
    let saved = c.transform;
    c.transform = crate::rig::ground(c, pose, false);
    f(c);
    c.transform = saved;
}

/// A transform's point (for a piece placed in another frame).
pub fn map(t: &Transform, p: crate::rig::P) -> crate::rig::P {
    (
        t.sx * p.0 + t.kx * p.1 + t.tx,
        t.ky * p.0 + t.sy * p.1 + t.ty,
    )
}

impl<F: Figure> Character for Slate<F> {
    fn info(&self) -> &CharacterInfo {
        self.0.info()
    }

    fn has_move(&self, _m: Move) -> bool {
        true
    }

    fn move_beats(&self, m: Move) -> f32 {
        move_beats(m)
    }

    fn routine(&self) -> &[Move] {
        self.0.routine()
    }

    fn bounds(&self) -> Bounds {
        self.0.bounds()
    }

    fn jumps_itself(&self) -> bool {
        true
    }

    fn hears_spectrum(&self) -> bool {
        self.0.hears_spectrum()
    }

    fn draw(&self, pose: &Pose, canvas: &mut Canvas) {
        let f = self.fig(pose);
        self.0.draw(&f, pose, canvas);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_family_arm_lands_where_its_angle_says() {
        let pr = Props {
            leg: 0.43,
            thigh: 0.222,
            shin: 0.222,
            torso: 0.275,
            neck: 0.030,
            head: 0.092,
            shoulder: 0.066,
            hipw: 0.040,
            stance: 0.056,
            upper: 0.148,
            fore: 0.150,
            foot: 0.05,
        };
        // Hanging straight down: under the shoulder, a whole arm below the neck.
        let h = hand_of((0.0, 0.0), 1, &pr);
        assert!((h.0 - pr.shoulder / pr.arm()).abs() < 1e-5 && (h.1 - 1.0).abs() < 1e-5);
        // Straight up on the left.
        let up = hand_of((180.0, 0.0), 0, &pr);
        assert!(up.1 < -0.99 && up.0 < 0.0);
    }

    #[test]
    fn a_mood_keeps_the_moves_face_at_the_default() {
        assert_eq!(mood_eyes(Eyes::Wide, Mood::Happy), Eyes::Wide);
        assert_eq!(mood_eyes(Eyes::Happy, Mood::Neutral), Eyes::Open);
        assert_eq!(mood_eyes(Eyes::Open, Mood::Cool), Eyes::Cool);
    }
}
