//! **Síncopa**, noir (`builtin:sincopa`, round 40, ADR 0122;
//! `docs/round40/design/dancer/sincopa.md`, the owner's pick S3): a human drawn as a
//! silhouette, mysterious and stylish. One dark ink for the whole figure; a **gold rim
//! light** down its lit side and a cool rim down the other — what draws it on Night's dark
//! room, while on Paper it is a sharp silhouette. A hood with gold headphones over it, a long
//! open coat whose tails swing late, slim trousers, sneakers with **gold soles**. The face is
//! barely there: two gold glints in the hood's dark. It moves liquid, behind the beat; its
//! signature on the drop, **Arm wave**: the arms rise and a wave runs from one hand through
//! the shoulders and out of the other; then it turns its head and its eyes light.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::draw::{blob_into, Sink};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{capsule_into, circle_into, rrect_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Props};
use cantus_core::inst::dancer::Move;
use cantus_dsp::math;
use std::f32::consts::PI;
use tiny_skia::{Color, Transform};

pub const ID: &str = "builtin:sincopa";

pub const PROPS: Props = Props {
    leg: 0.50,
    thigh: 0.262,
    shin: 0.248,
    torso: 0.27,
    neck: 0.040,
    head: 0.082,
    shoulder: 0.094,
    hipw: 0.050,
    stance: 0.068,
    upper: 0.158,
    fore: 0.152,
    foot: 0.10,
};

pub const STYLE: Style = Style {
    bounce: 0.05,
    sway: 0.10,
    step: 0.17,
    arms: 1.0,
    lag: 0.22,
    hop: 0.10,
    squash: 0.03,
    snap: 0.0,
    lean: 1.2,
    open: (0.50, 0.84),
    chest: -0.08,
    lift_step: 0.02,
};

/// How far the rims stand out (the character's units).
const RIM: f32 = 0.011;

/// Two beats, smooth: the hips sway on a sine (no hits), the chest answers, the shoulders
/// roll; the right hand draws a slow circle at the chest, the left hangs.
pub fn liquid(b: f32, st: &Style) -> Fig {
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let s = math::sin(PI * b);
    p.hip = (st.sway * s, st.bounce * beat_down(ph, 1.5));
    p.chest = (st.chest * s, 0.02 * beat_down(ph, 1.5));
    p.roll = 8.0 * math::sin(PI * (b - st.lag));
    p.lean = -2.0 - 3.0 * s;
    p.tilt = 8.0 * math::sin(PI * (b - st.lag * 1.5));
    p.nod = 6.0 * beat_down(pmod(b - st.lag, 1.0), 2.0) - 3.0;
    let (sa, ca) = math::sin_cos(PI * b);
    p.hands = [(-0.52, 0.96), (0.30 + 0.18 * ca, 0.50 + 0.14 * sa)];
    p.eyes = Eyes::Open;
    p.point = 40.0 * math::sin(PI * (b - st.lag * 2.0));
    p.accent = beat_down(ph, 8.0);
    p
}

/// Four beats: the step-touch as a glide, the hands low and loose.
pub fn glide(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    p.hands = [(-0.55, 0.95), (0.55, 0.95)];
    p.point = 50.0 * math::sin(PI * (b - 0.4) / 2.0);
    p.eyes = Eyes::Open;
    p.tilt = 6.0 * math::sin(PI * b / 2.0);
    p
}

fn bump(u: f32, c: f32) -> f32 {
    let x = (u - c) / 0.13;
    math::exp(-x * x)
}

/// Two beats, its signature: the arms rise to a line; a wave runs from the left hand through
/// the left elbow and shoulder, the chest, the right shoulder and elbow, out of the right
/// hand; then the arms fall and it turns its head to look — the eyes light.
pub fn arm_wave(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    let rise = smooth(bb / 0.3);
    let fall = smooth((bb - 1.55) / 0.3);
    let u = clamp01((bb - 0.3) / 1.2);
    let l_bend = 70.0 * bump(u, 0.06);
    let l_up = 26.0 * bump(u, 0.24);
    let r_up = 26.0 * bump(u, 0.74);
    let r_bend = 70.0 * bump(u, 0.92);
    let out = 92.0 * rise * (1.0 - fall) + 8.0 * (1.0 - rise * (1.0 - fall));
    let k = 1.0 - fall;
    p.fk = Some([(out + l_up * k, l_bend * k), (out + r_up * k, r_bend * k)]);
    p.chest = (0.0, -0.04 * bump(u, 0.5));
    p.roll = 12.0 * (bump(u, 0.40) - bump(u, 0.60));
    p.hip = (0.0, 0.03);
    p.tilt = 10.0 * fall;
    p.nod = -6.0 * fall;
    p.eyes = if bb > 1.6 { Eyes::Wide } else { Eyes::Open };
    p.point = 30.0 * bump(u, 0.5);
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

/// The ink, the hood's opening and the glints (the same in both rooms); the rims by room.
const INK: [u8; 4] = rgb(0x0E1216);
const OPENING: [u8; 4] = rgb(0x050709);
const GLINT: [u8; 4] = rgb(0xF2C766);
const GOLD_NIGHT: [u8; 4] = rgb(0xE2B85A);
const GOLD_PAPER: [u8; 4] = rgb(0xC99A3A);
const COOL_NIGHT: [u8; 4] = rgb(0x8A96A2);
const COOL_PAPER: [u8; 4] = rgb(0x7A8692);

pub struct Sincopa {
    info: CharacterInfo,
    family: dance::Style,
}

impl Sincopa {
    pub fn new() -> Sincopa {
        Sincopa {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Síncopa".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            // Liquid, behind the beat: a soft bob, a wide sway, a long lag.
            family: dance::Style {
                bounce: 0.04,
                squash: 0.03,
                arms: 1.0,
                sway: 0.09,
                lag: 0.22,
                hop: 0.12,
                stride: 0.13,
            },
        }
    }
}

impl Default for Sincopa {
    fn default() -> Self {
        Sincopa::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Sincopa> {
    slate::Slate(Sincopa::new())
}

impl Figure for Sincopa {
    fn info(&self) -> &CharacterInfo {
        &self.info
    }

    fn props(&self) -> &Props {
        &PROPS
    }

    fn family(&self) -> &dance::Style {
        &self.family
    }

    fn own(&self, mv: Move, b: f32) -> Option<Fig> {
        Some(match mv {
            Move::Bounce => liquid(b, &STYLE),
            Move::StepLeft | Move::StepRight => glide(b, &STYLE),
            Move::Jump => arm_wave(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.56,
            top: -1.2,
            right: 0.56,
            bottom: 0.07,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        draw(f, pose, c);
    }
}

/// The rims and the ink of the lit pieces.
struct Lit {
    gold: Color,
    cool: Color,
    ink: Color,
}

/// A piece of the silhouette: its gold rim moved toward the light (left, up), its cool rim
/// the other way, then the ink — the rims survive only at the piece's edges.
fn lit(br: &mut Brush, l: &Lit, r: f32, shape: &dyn Fn(&mut dyn Sink)) {
    let t = br.c.transform;
    for (dx, dy, col) in [
        (-r, -r * 0.35, l.gold),
        (r * 0.6, r * 0.1, l.cool),
        (0.0, 0.0, l.ink),
    ] {
        br.c.transform = t.pre_concat(Transform::from_translate(dx, dy));
        br.piece(&|s, _| shape(s), col, false);
    }
    br.c.transform = t;
}

/// A tapered tube through three points (round ends and join).
fn tube3_into(s: &mut dyn Sink, p: [P; 3], w: [f32; 3]) {
    capsule_into(s, p[0], p[1], w[0], w[1], 0.0);
    capsule_into(s, p[1], p[2], w[1], w[2], 0.0);
}

fn draw(f: &Fig, pose: &Pose, c: &mut Canvas) {
    let j = solve(f, &PROPS);
    let p = |q: P| place(q, &j);
    let mut br = Brush::new(c, 0.0);
    let dark = br.ink.dark;
    let l = Lit {
        gold: br.hued(if dark { GOLD_NIGHT } else { GOLD_PAPER }),
        cool: br.fixed(if dark { COOL_NIGHT } else { COOL_PAPER }),
        ink: br.fixed(INK),
    };
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    let lift = f.lift;
    slate::on_ground(br.c, pose, |c| {
        let k = (1.0 - lift * 3.0).max(0.0);
        Brush::new(c, 0.0).shadow((gx, 0.008), 0.17 * k + 0.05, 0.024 * k + 0.008, 0.55);
    });
    let t = PROPS.torso;
    let neck = p(j.neck);
    let at = |x: f32, y: f32| add(neck, rot0((x, y), j.torso_deg));
    let sw = 0.0011 * f.point.clamp(-50.0, 50.0) - 0.0015 * f.lean;
    // The coat's tails, behind the legs: they swing late.
    for s in [-1.0f32, 1.0] {
        let tail = [
            at(s * 0.070, t - 0.03),
            at(s * 0.114, t - 0.03),
            at(s * 0.150 + sw * 1.2, t + 0.21),
            at(s * 0.088 + sw * 1.6, t + 0.23),
        ];
        lit(&mut br, &l, RIM, &|s| blob_into(s, &tail));
    }
    // Legs: slim trousers; sneakers with gold soles.
    let pelvis = p(j.pelvis);
    for i in 0..2 {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        let top = add(add(hip, mul(sub(pelvis, hip), 0.3)), (0.0, -0.03));
        let pts = [top, knee, add(foot, (0.0, -0.040))];
        lit(&mut br, &l, RIM, &|s| {
            tube3_into(s, pts, [0.080, 0.066, 0.062])
        });
    }
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let foot = p(j.feet[i]);
        let ang = -s * j.toes[i] * 0.6;
        let base = add(foot, (s * 0.022, -0.016));
        br.rrect(
            (base.0, base.1 + 0.008),
            0.132,
            0.020,
            0.010,
            ang,
            l.gold,
            false,
        );
        let up: [P; 6] = [
            (-0.056, 0.0),
            (-0.052, -0.046),
            (-0.020, -0.066),
            (0.030, -0.056),
            (0.064, -0.020),
            (0.064, 0.0),
        ]
        .map(|(x, y)| add(base, rot0((x * s, y), ang)));
        lit(&mut br, &l, RIM * 0.7, &|s| blob_into(s, &up));
    }
    // The coat's body (open), its two seams.
    let coat = [
        at(-0.036, -0.004),
        at(-0.102, 0.008),
        at(-0.128, 0.052),
        at(-0.122, 0.150),
        at(-0.112, t),
        at(0.112, t),
        at(0.122, 0.150),
        at(0.128, 0.052),
        at(0.102, 0.008),
        at(0.036, -0.004),
    ];
    lit(&mut br, &l, RIM, &|s| blob_into(s, &coat));
    br.piece(
        &|s, _| {
            crate::slate::paint::bars_into(
                s,
                &[at(-0.020, 0.030), at(-0.010, t - 0.01)],
                0.004,
                0.0,
            );
            crate::slate::paint::bars_into(s, &[at(0.020, 0.030), at(0.010, t - 0.01)], 0.004, 0.0);
        },
        l.cool,
        false,
    );
    // The hood round the head, the face's dark opening, the glints.
    let hc = p(j.head);
    let hd = j.head_deg;
    let h = |x: f32, y: f32| add(hc, rot0((x, y), hd));
    let hood = [
        h(-0.092, 0.060),
        h(-0.104, -0.010),
        h(-0.090, -0.080),
        h(-0.040, -0.118),
        h(0.020, -0.124),
        h(0.074, -0.098),
        h(0.100, -0.040),
        h(0.098, 0.040),
        h(0.070, 0.090),
        h(0.0, 0.104),
        h(-0.070, 0.092),
    ];
    lit(&mut br, &l, RIM * 1.2, &|s| blob_into(s, &hood));
    let opening = [
        h(-0.060, -0.010),
        h(-0.040, -0.062),
        h(0.0, -0.072),
        h(0.044, -0.060),
        h(0.062, -0.006),
        h(0.046, 0.062),
        h(0.0, 0.078),
        h(-0.046, 0.062),
    ];
    let dark_in = br.fixed(OPENING);
    br.piece(&|s, _| blob_into(s, &opening), dark_in, false);
    eyes(&mut br, f, &h, l.gold);
    // Gold headphones over the hood.
    let band: [P; 11] = std::array::from_fn(|k| {
        let (sa, ca) = math::sin_cos(crate::rig::rad(195.0 + 15.0 * k as f32));
        h(0.100 * ca, -0.012 + 0.110 * sa)
    });
    br.curve(&band, 0.012, l.gold, false);
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                rrect_into(s, h(side * 0.100, 0.006), 0.030, 0.052, 0.012, 0.0, 0.0);
            }
        },
        l.gold,
        false,
    );
    // Arms: coat sleeves, gloved hands, each with its rim so it reads in front of the coat.
    for i in 0..2 {
        let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
        let sh = add(sh, mul(sub(neck, sh), 0.12));
        lit(&mut br, &l, RIM, &|s| {
            tube3_into(s, [sh, el, ha], [0.074, 0.058, 0.048])
        });
        let fd = sub(ha, el);
        let fl = length(fd).max(1e-6);
        let hand = add(ha, mul(fd, 0.028 / fl));
        br.circle((hand.0 - RIM, hand.1 - RIM * 0.35), 0.031, l.gold, false);
        br.circle(hand, 0.031, l.ink, false);
    }
}

/// Two gold glints in the hood's dark: a mood is how they glint.
fn eyes(br: &mut Brush, f: &Fig, h: &dyn Fn(f32, f32) -> P, gold: Color) {
    let g = br.fixed(GLINT);
    use crate::slate::paint::bars_into;
    match f.eyes {
        Eyes::Closed => {}
        Eyes::Cool => br.bars(&[h(-0.046, 0.0), h(0.046, 0.0)], 0.008, g, false),
        Eyes::Wide => br.piece(
            &|s, _| {
                circle_into(s, h(-0.026, 0.0), 0.010, 0.0);
                circle_into(s, h(0.026, 0.0), 0.010, 0.0);
            },
            g,
            false,
        ),
        Eyes::Sleepy => br.piece(
            &|s, _| {
                for x in [-0.026f32, 0.026] {
                    bars_into(s, &[h(x - 0.010, 0.004), h(x + 0.010, 0.004)], 0.004, 0.0);
                }
            },
            gold,
            false,
        ),
        e => br.piece(
            &|s, _| {
                for side in [-1.0f32, 1.0] {
                    let x = side * 0.026;
                    if e == Eyes::Happy {
                        let pts = [h(x - 0.012, 0.006), h(x, -0.004), h(x + 0.012, 0.006)];
                        bars_into(s, &pts, 0.006, 0.0);
                    } else if e == Eyes::Wink && side > 0.0 {
                        // The right one shut.
                    } else {
                        let pts = [h(x - 0.012 * side, 0.002), h(x + 0.010 * side, -0.004)];
                        bars_into(s, &pts, 0.007, 0.0);
                    }
                }
            },
            g,
            false,
        ),
    }
}
