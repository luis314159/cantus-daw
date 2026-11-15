//! **Compás**, the pictogram (`builtin:compas`, round 40, ADR 0122) — the Slate cast's
//! default dancer (`docs/round40/design/dancer/compas.md`): a sign figure come alive. One ink
//! (the theme's ink 1), solid pieces of one weight with round ends — a disc head, a capsule
//! torso, capsule limbs — each piece in front **cut** from the one behind it by a hairline
//! of the room's colour. One gold accent: the headphones, and a beat ring at its feet. No
//! face: a mood is a posture (*Cool* cuts a visor into the disc; *Shy* is its own, *Cueing*:
//! one cup lifted off the ear). It dances **pose to
//! pose**: it snaps into a key on the beat and holds it, short motion marks in ink 3 along
//! the path a hand or a foot has just travelled.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance;
use crate::rig::draw::mix;
use crate::rig::P;
use crate::slate::fig::{add, length, mul, rot0, sub};
use crate::slate::moves::{self, Style};
use crate::slate::paint::Brush;
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Joints, Props};
use cantus_core::inst::dancer::Move;
use cantus_dsp::math;

pub const ID: &str = "builtin:compas";

pub const PROPS: Props = Props {
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

pub const STYLE: Style = Style {
    bounce: 0.06,
    sway: 0.05,
    step: 0.15,
    arms: 1.0,
    lag: 0.0,
    hop: 0.20,
    squash: 0.08,
    snap: 1.0,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: 0.02,
    lift_step: 0.07,
};

const W_ARM: f32 = 0.066;
const W_LEG: f32 = 0.078;
const W_TORSO_TOP: f32 = 0.162;
const W_TORSO_BOT: f32 = 0.120;
const CUT: f32 = 0.017;
/// The motion marks: how many earlier moments, how far apart (beats).
const TRAIL_N: usize = 5;
const TRAIL_STEP: f32 = 0.03;

pub struct Compas {
    info: CharacterInfo,
    family: dance::Style,
}

impl Compas {
    pub fn new() -> Compas {
        Compas {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Compás".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            // The family's moves danced crisply: little squash, a firm bob.
            family: dance::Style {
                bounce: 0.05,
                squash: 0.06,
                arms: 1.1,
                sway: 0.06,
                lag: 0.04,
                hop: 0.22,
                stride: 0.12,
            },
        }
    }
}

impl Default for Compas {
    fn default() -> Self {
        Compas::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Compas> {
    slate::Slate(Compas::new())
}

impl Figure for Compas {
    fn info(&self) -> &CharacterInfo {
        &self.info
    }

    fn props(&self) -> &Props {
        &PROPS
    }

    fn family(&self) -> &dance::Style {
        &self.family
    }

    fn arms_by_angle(&self) -> bool {
        true
    }

    fn own(&self, mv: Move, b: f32) -> Option<Fig> {
        Some(match mv {
            Move::Bounce => moves::pump(b, &STYLE),
            Move::StepLeft | Move::StepRight => moves::steps_open(b, &STYLE),
            Move::Jump => moves::star(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.52,
            top: -1.28,
            right: 0.52,
            bottom: 0.08,
        }
    }

    fn trails(&self) -> (usize, f32) {
        (TRAIL_N, TRAIL_STEP)
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        let j = solve(f, &PROPS);
        let p = |q: P| place(q, &j);
        let mut br = Brush::new(c, CUT);
        let ink = br.fixed(br.ink.ink);
        let room = br.room;
        let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
        // The contact shadow and the beat ring (gold, on the beat), on the floor.
        let accent = f.accent;
        let lift = f.lift;
        let gold = br.hued(br.ink.gold);
        // The ring's opacity as a mix toward the room: one opaque fill.
        let ring = br.hued(mix(br.ink.gold, br.ink.room, 1.0 - (accent * 1.4).min(1.0)));
        slate::on_ground(br.c, pose, |c| {
            let mut br = Brush::new(c, CUT);
            let k = (1.0 - lift * 3.5).max(0.0);
            br.shadow((gx, 0.012), 0.17 * k + 0.04, 0.026 * k + 0.006, 0.55);
            if accent > 0.04 {
                let r = 0.23 + 0.07 * (1.0 - accent);
                br.ring((gx, 0.012), r, r * 0.16, 0.012 * (0.4 + 0.6 * accent), ring);
            }
        });
        // Motion marks: two short arcs along the path a hand or a foot just travelled.
        if pose.playing {
            trails(self, &mut br, f, pose, &j);
        }
        // Legs and torso: one silhouette (their cuts first, then their fills).
        let legs: [[P; 3]; 2] =
            std::array::from_fn(|i| [p(j.hips[i]), p(j.knees[i]), p(j.feet[i])]);
        let top = p(add(j.neck, rot0((0.0, W_TORSO_TOP * 0.42), j.torso_deg)));
        let bot = p(j.pelvis);
        for leg in &legs {
            br.bars(leg, W_LEG + 2.0 * CUT, room, false);
        }
        br.tube(
            top,
            bot,
            W_TORSO_TOP + 2.0 * CUT,
            W_TORSO_BOT + 2.0 * CUT,
            room,
            false,
        );
        for leg in &legs {
            br.bars(leg, W_LEG, ink, false);
        }
        br.tube(top, bot, W_TORSO_TOP, W_TORSO_BOT, ink, false);
        // Arms: in front, cut from the torso where they cross it, joined at the shoulder.
        for i in 0..2 {
            let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
            let free = add(sh, mul(sub(el, sh), 0.55));
            br.bars(&[free, el, ha], W_ARM + 2.0 * CUT, room, false);
            br.bars(&[sh, el, ha], W_ARM, ink, false);
        }
        // The head, and the gold headphones.
        let hc = p(j.head);
        let r = PROPS.head;
        br.circle(hc, r, ink, true);
        let deg = j.head_deg;
        if f.eyes == Eyes::Cool {
            // A visor band: sunglasses as negative space.
            let a = add(hc, rot0((-r * 0.78, -r * 0.05), deg));
            let b = add(hc, rot0((r * 0.78, -r * 0.05), deg));
            let mid = mul(add(a, b), 0.5);
            br.rrect(mid, length(sub(b, a)), r * 0.30, 0.0, deg, room, false);
        }
        let band_r = r + 0.022;
        let arc: [P; 15] = std::array::from_fn(|k| {
            let a = crate::rig::rad(200.0 + 10.0 * k as f32);
            let (s, co) = math::sin_cos(a);
            add(hc, rot0((band_r * co, band_r * s), deg))
        });
        br.curve(&arc, 0.017, gold, true);
        // Shy, its own way (review 2 of round 40, R2-B8): *Cueing* — a DJ's cup lifted off
        // one ear, up and out, to hear the next record.
        let cueing = f.eyes == Eyes::Wink;
        for s in [-1.0f32, 1.0] {
            if cueing && s > 0.0 {
                let cup = add(hc, rot0((r + 0.040, -0.040), deg));
                br.rrect(cup, 0.040, 0.066, 0.016, deg + 30.0, gold, true);
                continue;
            }
            let cup = add(hc, rot0((s * (r + 0.006), 0.012), deg));
            br.rrect(cup, 0.040, 0.066, 0.016, deg, gold, true);
        }
    }
}

/// The motion marks (the design's `trails`): the hands and feet `TRAIL_N` earlier moments
/// back; where one travelled far enough (a snap), two short arcs in ink 3 behind it.
fn trails(ch: &Compas, br: &mut Brush, f: &Fig, pose: &Pose, j: &Joints) {
    let mut past: [Option<Joints>; TRAIL_N] = [None; TRAIL_N];
    for (k, o) in past.iter_mut().enumerate() {
        let fb = slate::fig_before(ch, pose, (k + 1) as f32 * TRAIL_STEP);
        *o = Some(solve(&Fig { eyes: f.eyes, ..fb }, &PROPS));
    }
    let col = br.fixed(br.ink.ink3);
    for idx in 0..2 {
        for (hand, w) in [(true, W_ARM), (false, W_LEG)] {
            let at = |jj: &Joints| place(if hand { jj.hands[idx] } else { jj.feet[idx] }, jj);
            let now = at(j);
            let mut path = [now; TRAIL_N + 1];
            for (k, pj) in past.iter().enumerate() {
                if let Some(pj) = pj {
                    path[k + 1] = at(pj);
                }
            }
            let travel: f32 = path.windows(2).map(|w| length(sub(w[0], w[1]))).sum();
            if travel < 0.10 {
                continue;
            }
            let mut keep = [now; TRAIL_N + 1];
            let mut n = 0;
            for q in path {
                if length(sub(q, now)) > w * 0.5 + 0.035 {
                    keep[n] = q;
                    n += 1;
                }
            }
            if n < 2 {
                continue;
            }
            let (a, z) = (keep[0], keep[n - 1]);
            let dv = sub(a, z);
            let ln = length(dv).max(1e-6);
            let nrm = (-dv.1 / ln, dv.0 / ln);
            let m = n.min(3);
            for o in [-0.42f32, 0.42] {
                let mut seg = [now; 3];
                for (s, q) in seg.iter_mut().zip(keep.iter()).take(m) {
                    *s = add(*q, mul(nrm, o * w));
                }
                br.curve(&seg[..m], 0.010, col, false);
            }
        }
    }
}
