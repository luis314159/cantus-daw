//! **Metrónomo**, the serious one (`builtin:metronomo`, round 40, ADR 0122;
//! `docs/round40/design/dancer/metronomo.md`): a wooden metronome with a butler's dignity —
//! not Tic, the old family's robot: wood and manners. A tall pyramid in two values of wood
//! on a plinth, a gold finial, a gold tempo scale; its **pendulum** swings in front, the
//! gold weight on the rod, each extreme exactly on a beat (the song's beat, whatever the
//! move). A wing collar and a black bow tie, white gloves, a silver tray in one hand, a gold
//! monocle on a chain, half-lidded eyes, a raised brow, a curled moustache, white spats. It
//! barely moves — that is the joke. Its signature on the drop, **Doble tiempo**: the
//! pendulum runs at double, then quadruple time; he loses his composure, the monocle drops,
//! the arms flail; on the beat he straightens his tie and recomposes.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::draw::{ellipse_into, polygon_into};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{
    bars_into, capsule_into, circle_into, clip_convex, poly_round_into, Brush,
};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use std::f32::consts::PI;
use tiny_skia::Color;

pub const ID: &str = "builtin:metronomo";

pub const PROPS: Props = Props {
    leg: 0.14,
    thigh: 0.075,
    shin: 0.070,
    torso: 0.36,
    neck: 0.0,
    head: 0.10,
    shoulder: 0.100,
    hipw: 0.060,
    stance: 0.080,
    upper: 0.130,
    fore: 0.125,
    foot: 0.10,
};

pub const STYLE: Style = Style {
    bounce: 0.03,
    sway: 0.0,
    step: 0.18,
    arms: 1.0,
    lag: 0.0,
    hop: 0.06,
    squash: 0.03,
    snap: 1.0,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: 0.02,
    lift_step: 0.04,
};

const CUT: f32 = 0.012;
/// The pendulum's extreme, degrees.
const SWING: f32 = 30.0;
/// The tray hand (neck-relative, A units).
const TRAY: P = (-0.85, 0.20);

/// The pendulum's angle: its extreme exactly on each beat (cos at the beat), like a real
/// metronome; `mult` × the tempo.
pub fn pendulum(b: f32, mult: f32) -> f32 {
    SWING * math::cos(PI * b * mult)
}

/// Two beats, composed: a small bow on the one, the free hand conducting a tiny two (down on
/// the beat, up between), the tray steady.
pub fn keep_time(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let k = (b.floor() as i64).rem_euclid(2);
    p.shape = pendulum(b, 1.0);
    let bow = if k == 0 { beat_down(ph, 3.0) } else { 0.0 };
    p.nod = 8.0 * bow;
    p.lean = 2.0 * bow;
    p.hip = (0.0, 0.02 * bow);
    p.hands = [TRAY, (0.70, 0.30 + 0.25 * beat_down(ph, 2.0))];
    p.eyes = Eyes::Open;
    p.accent = beat_down(ph, 8.0);
    p
}

/// Four beats of exact side steps.
pub fn step(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    p.shape = pendulum(b, 1.0);
    p.hands = [TRAY, (0.70, 0.40)];
    p.lean = 0.0;
    p.eyes = Eyes::Open;
    p
}

/// Two beats: from the half beat the pendulum runs at ×2, then ×4 (its extremes still on the
/// beats); he loses his composure — wide eyes, the monocle drops, arms flailing, the tray
/// wobbling — and on beat 2 he freezes, straightens his tie, the monocle goes back.
pub fn double_time(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    if bb < 1.0 {
        let mult = if bb < 0.25 {
            1.0
        } else if bb < 0.6 {
            2.0
        } else {
            4.0
        };
        p.shape = pendulum(b, mult);
        let panic = smooth((bb - 0.2) / 0.4);
        let f = math::sin(bb * PI * 8.0);
        p.hands = [
            (TRAY.0 - 0.1 * f * panic, TRAY.1 - 0.6 * panic * f.abs()),
            (0.85, 0.30 - 1.2 * panic * math::sin(bb * PI * 6.0).abs()),
        ];
        p.lean = 8.0 * panic * math::sin(bb * PI * 10.0);
        p.hip = (0.03 * panic * math::sin(bb * PI * 12.0), 0.0);
        p.eyes = if panic > 0.3 { Eyes::Wide } else { Eyes::Open };
        p.sig = panic; // the monocle falls
        p.mouth = 0.8 * panic;
    } else {
        let t = bb - 1.0;
        p.shape = pendulum(b, 1.0);
        let settle = smooth(t / 0.8);
        let tie = math::sin(PI * clamp01(t / 0.6));
        p.hands = [TRAY, (lerp(0.85, 0.12, tie), lerp(0.30, 0.20, tie))];
        p.lean = 0.0;
        p.nod = -6.0 * tie;
        p.eyes = if t < 0.6 { Eyes::Closed } else { Eyes::Open };
        p.sig = 1.0 - settle;
        p.mouth = 0.0;
    }
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

/// Wood and manners: the same in both rooms but the pyramid's rim.
mod pal {
    use crate::render::rgb;
    pub const WOOD: [u8; 4] = rgb(0x9A6A44);
    pub const WOOD_S: [u8; 4] = rgb(0x734B2E);
    pub const GRAIN: [u8; 4] = rgb(0xB07E54);
    pub const PANEL: [u8; 4] = rgb(0x5E3D25);
    pub const RIM_NIGHT: [u8; 4] = rgb(0xC9A27A);
    pub const RIM_PAPER: [u8; 4] = rgb(0x5E3D25);
    pub const WHITE: [u8; 4] = rgb(0xF3F5F7);
    pub const WHITE_S: [u8; 4] = rgb(0xBAC3CC);
    pub const INK: [u8; 4] = rgb(0x14191E);
    pub const STEEL: [u8; 4] = rgb(0xC3CBD3);
}

/// The logo's gold of the weight, the scale and the chain (both rooms).
pub const GOLD_L: [u8; 4] = rgb(0xE2B85A);

pub struct Metronomo {
    info: CharacterInfo,
    family: dance::Style,
}

impl Metronomo {
    pub fn new() -> Metronomo {
        Metronomo {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Metrónomo".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            // Composure: the family's moves, small.
            family: dance::Style {
                bounce: 0.02,
                squash: 0.03,
                arms: 0.7,
                sway: 0.02,
                lag: 0.0,
                hop: 0.06,
                stride: 0.08,
            },
        }
    }
}

impl Default for Metronomo {
    fn default() -> Self {
        Metronomo::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Metronomo> {
    slate::Slate(Metronomo::new())
}

impl Figure for Metronomo {
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
            Move::Bounce => keep_time(b, &STYLE),
            Move::StepLeft | Move::StepRight => step(b, &STYLE),
            Move::Jump => double_time(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// The butler's moods: composed, pleased, alarmed (the mouth open, the monocle slipping),
    /// unamused, and the eighth (Shy's place) the monocle off.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Excited => {
                f.eyes = Eyes::Wide;
                f.mouth = f.mouth.max(0.8);
                f.sig = f.sig.max(0.6);
            }
            Mood::Surprised => f.mouth = f.mouth.max(0.5),
            Mood::Cool => {
                f.eyes = Eyes::Open;
                f.tilt -= 6.0;
            }
            Mood::Shy => {
                f.eyes = Eyes::Wide;
                f.sig = 1.0;
            }
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.52,
            top: -1.04,
            right: 0.52,
            bottom: 0.07,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        // The pendulum keeps the song's beat whatever the move; only Doble tiempo runs away.
        let swing = if pose.mv == Move::Jump {
            f.shape
        } else {
            pendulum(pose.beats as f32, 1.0)
        };
        draw(f, swing, pose, c);
    }
}

fn draw(f: &Fig, swing: f32, pose: &Pose, c: &mut Canvas) {
    let j = solve(f, &PROPS);
    let p = |q: P| place(q, &j);
    let mut br = Brush::new(c, CUT);
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let (ink, white, white_s) = (
        col(&br, pal::INK),
        col(&br, pal::WHITE),
        col(&br, pal::WHITE_S),
    );
    let (wood, wood_s) = (br.hued(pal::WOOD), br.hued(pal::WOOD_S));
    let gold = br.hued(br.ink.gold);
    let gold_l = br.hued(GOLD_L);
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    slate::on_ground(br.c, pose, |c| {
        Brush::new(c, CUT).shadow((gx, 0.010), 0.22, 0.030, 0.5);
    });
    // Short legs: white spats, black shoes.
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        let ankle = add(foot, (0.0, -0.03));
        br.piece(
            &|sk, _| {
                capsule_into(sk, hip, knee, 0.060, 0.056, 0.0);
                capsule_into(sk, knee, ankle, 0.056, 0.064, 0.0);
            },
            ink,
            false,
        );
        br.rrect(
            (foot.0 + s * 0.010, foot.1 - 0.040),
            0.070,
            0.034,
            0.010,
            0.0,
            white,
            false,
        );
        br.rrect(
            (foot.0 + s * 0.024, foot.1 - 0.012),
            0.110,
            0.030,
            0.014,
            0.0,
            ink,
            false,
        );
    }
    let deg = j.torso_deg;
    let pelvis = p(j.pelvis);
    let l = |x: f32, y: f32| add(pelvis, rot0((x, y), deg));
    // The plinth, in two values (the light moved up-left, kept inside it).
    let pc = l(0.0, -0.010);
    br.rrect(pc, 0.38, 0.075, 0.016, deg, wood_s, true);
    br.rrect(
        (pc.0 - 0.005, pc.1 - 0.003),
        0.37,
        0.069,
        0.014,
        deg,
        wood,
        false,
    );
    // The wooden pyramid: its rim, its two values, the grain; a gold finial.
    let pyr = [
        l(-0.160, -0.045),
        l(0.160, -0.045),
        l(0.080, -0.700),
        l(-0.080, -0.700),
    ];
    let room = br.room;
    br.piece(&|s, _| poly_round_into(s, &pyr, CUT), room, false);
    let rim = col(
        &br,
        if br.ink.dark {
            pal::RIM_NIGHT
        } else {
            pal::RIM_PAPER
        },
    );
    br.piece(&|s, _| poly_round_into(s, &pyr, 0.012), rim, false);
    br.piece(&|s, _| polygon_into(s, &pyr), wood_s, false);
    let lit = pyr.map(|q| add(q, (-0.020, -0.006)));
    let mut out = [(0.0, 0.0); 32];
    let n = clip_convex(&lit, &pyr, &mut out);
    br.piece(&|s, _| polygon_into(s, &out[..n]), wood, false);
    let grain = br.hued(pal::GRAIN);
    br.piece(
        &|s, _| {
            for x in [-0.110f32, 0.095] {
                bars_into(s, &[l(x, -0.08), l(x * 0.55, -0.66)], 0.006, 0.0);
            }
        },
        grain,
        false,
    );
    br.circle(l(0.0, -0.730), 0.026, gold, true);
    // The tempo scale: a dark panel with gold ticks.
    let panel = br.hued(pal::PANEL);
    let scale = [
        l(-0.055, -0.105),
        l(0.055, -0.105),
        l(0.034, -0.460),
        l(-0.034, -0.460),
    ];
    br.piece(&|s, _| polygon_into(s, &scale), panel, false);
    br.piece(
        &|s, _| {
            for k in 0..9 {
                let y = -0.13 - k as f32 * 0.038;
                let w = 0.030 - k as f32 * 0.002;
                bars_into(s, &[l(-w, y), l(w, y)], 0.004, 0.0);
            }
        },
        gold_l,
        false,
    );
    // The wing collar and the bow tie.
    br.piece(
        &|s, _| {
            polygon_into(
                s,
                &[l(-0.090, -0.505), l(-0.006, -0.470), l(-0.050, -0.450)],
            );
            polygon_into(s, &[l(0.090, -0.505), l(0.006, -0.470), l(0.050, -0.450)]);
        },
        white,
        false,
    );
    br.piece(
        &|s, _| {
            polygon_into(s, &[l(-0.040, -0.490), l(0.0, -0.476), l(-0.040, -0.462)]);
            polygon_into(s, &[l(0.040, -0.490), l(0.0, -0.476), l(0.040, -0.462)]);
            circle_into(s, l(0.0, -0.476), 0.010, 0.0);
        },
        ink,
        false,
    );
    face(&mut br, f, &l, ink, white, gold, gold_l);
    // The pendulum: pivot, steel rod, gold weight — exact on the beat.
    let piv = l(0.0, -0.110);
    let a = swing + deg;
    let (sa, ca) = math::sin_cos(crate::rig::rad(a));
    let tip = add(piv, (0.37 * sa, -0.37 * ca));
    let steel = col(&br, pal::STEEL);
    br.bars(&[piv, tip], 0.013, steel, true);
    let wt = add(piv, (0.25 * sa, -0.25 * ca));
    br.rrect(wt, 0.050, 0.040, 0.008, a, gold_l, true);
    br.circle(piv, 0.018, gold_l, false);
    // Arms: black tail-coat sleeves, white gloves; a silver tray.
    for i in 0..2 {
        let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
        br.piece(
            &|s, g| {
                capsule_into(s, sh, el, 0.050 + 2.0 * g, 0.044 + 2.0 * g, 0.0);
                capsule_into(s, el, ha, 0.044 + 2.0 * g, 0.040 + 2.0 * g, 0.0);
            },
            ink,
            true,
        );
        br.piece(
            &|s, g| ellipse_into(s, ha, 0.032 + g, 0.030 + g, 0.0),
            white_s,
            true,
        );
        br.piece(
            &|s, _| ellipse_into(s, add(ha, (-0.003, -0.003)), 0.028, 0.026, 0.0),
            white,
            false,
        );
    }
    let tray = add(p(j.hands[0]), (0.0, -0.040));
    let tilt = 10.0 * math::sin(f.t * 20.0) * f.sig;
    br.piece(
        &|s, g| ellipse_into(s, tray, 0.075 + g, 0.014 + g, tilt),
        white_s,
        true,
    );
    br.piece(
        &|s, _| ellipse_into(s, add(tray, (0.0, -0.002)), 0.070, 0.010, tilt),
        steel,
        false,
    );
    let cup = add(tray, rot0((0.010, -0.024), tilt));
    br.rrect(cup, 0.030, 0.030, 0.006, tilt, white, true);
}

/// The butler's face on the pyramid: half-lidded eyes, a raised brow, the monocle on its
/// chain (it falls with the signature), the curled moustache.
#[allow(clippy::too_many_arguments)]
fn face(
    br: &mut Brush,
    f: &Fig,
    l: &dyn Fn(f32, f32) -> P,
    ink: Color,
    white: Color,
    gold: Color,
    gold_l: Color,
) {
    let e = f.eyes;
    let ey = -0.590;
    let wood_s = br.hued(pal::WOOD_S);
    if e == Eyes::Wide {
        br.piece(
            &|s, _| {
                for x in [-0.036f32, 0.036] {
                    circle_into(s, l(x, ey), 0.016, 0.0);
                }
            },
            white,
            false,
        );
    }
    let lid_open = |s: f32| match e {
        Eyes::Wink => s > 0.0,
        Eyes::Closed | Eyes::Sleepy | Eyes::Happy | Eyes::Wide => false,
        _ => true,
    };
    if !matches!(e, Eyes::Wide | Eyes::Closed | Eyes::Sleepy | Eyes::Happy) {
        // Composed: half-lidded, the lid's line in the wood's shade.
        br.piece(
            &|s, _| {
                for side in [-1.0f32, 1.0] {
                    if lid_open(side) {
                        let x = side * 0.036;
                        bars_into(
                            s,
                            &[l(x - 0.016, ey - 0.004), l(x + 0.016, ey - 0.004)],
                            0.006,
                            0.0,
                        );
                    }
                }
            },
            wood_s,
            false,
        );
    }
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                let x = side * 0.036;
                if e == Eyes::Wide {
                    circle_into(s, l(x, ey), 0.007, 0.0);
                } else if lid_open(side) {
                    ellipse_into(s, l(x, ey + 0.002), 0.012, 0.008, 0.0);
                } else if e == Eyes::Happy {
                    let pts = [
                        l(x - 0.016, ey + 0.004),
                        l(x, ey - 0.008),
                        l(x + 0.016, ey + 0.004),
                    ];
                    crate::rig::draw::ribbon_into(s, &pts, 0.006);
                } else {
                    let pts = [l(x - 0.016, ey), l(x, ey + 0.006), l(x + 0.016, ey)];
                    crate::rig::draw::ribbon_into(s, &pts, 0.006);
                }
                let brow = if side > 0.0 && e != Eyes::Wide {
                    -0.020
                } else {
                    -0.014
                };
                bars_into(
                    s,
                    &[
                        l(x - 0.018, ey + brow),
                        l(x + 0.016, ey + brow - 0.004 * side),
                    ],
                    0.006,
                    0.0,
                );
                // The moustache, curled.
                let m = [
                    l(0.0, -0.548),
                    l(side * 0.030, -0.552),
                    l(side * 0.054, -0.546),
                    l(side * 0.060, -0.558),
                ];
                crate::rig::draw::ribbon_into(s, &m, 0.010);
            }
            if f.mouth > 0.3 {
                ellipse_into(s, l(0.0, -0.526), 0.012, 0.010 * f.mouth + 0.004, 0.0);
            }
        },
        ink,
        false,
    );
    // The monocle on the right eye and its chain; it falls with the signature.
    let eye = l(0.036, ey);
    let hang = l(0.060, -0.470);
    let mc = lerp_p(eye, hang, clamp01(f.sig));
    br.bars(&[l(0.075, -0.495), mc], 0.003, gold_l, false);
    br.ring(mc, 0.022, 0.022, 0.006, gold);
}
