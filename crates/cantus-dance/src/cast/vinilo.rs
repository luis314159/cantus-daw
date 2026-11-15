//! **Vinilo**, the DJ (`builtin:vinilo`, round 40, ADR 0122;
//! `docs/round40/design/dancer/vinilo.md`): a turntable that plays itself. Its head is a
//! record — black vinyl with grooves and a vermilion label that carries its face, the gold
//! spindle its nose, a sheen and a gold marker that turn while the face stays upright — with
//! DJ headphones, one cup on, one pushed back. Its body is a graphite plinth with a lapis
//! light strip and a gold chain; its right arm is the **tonearm** (a gold pivot at the
//! shoulder, a pale rod, a headshell with a gold stylus for a hand), its left a sleeve with a
//! white glove that scratches the record's edge. No maker's look: no plinth layout, logo or
//! label of any brand. It grooves by **scratching**; on the drop, its signature,
//! **Backspin**: it brakes the record, spins it back in a blur, and lands with the record
//! running forward, arms up.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance;
use crate::rig::draw::{ellipse_into, ribbon_into};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{bars_into, capsule_into, circle_into, rrect_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use std::f32::consts::PI;
use tiny_skia::Color;

pub const ID: &str = "builtin:vinilo";

pub const PROPS: Props = Props {
    leg: 0.36,
    thigh: 0.186,
    shin: 0.180,
    torso: 0.24,
    neck: 0.02,
    head: 0.19,
    shoulder: 0.120,
    hipw: 0.050,
    stance: 0.070,
    upper: 0.150,
    fore: 0.150,
    foot: 0.11,
};

pub const STYLE: Style = Style {
    bounce: 0.08,
    sway: 0.06,
    step: 0.20,
    arms: 1.0,
    lag: 0.10,
    hop: 0.12,
    squash: 0.05,
    snap: 0.4,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: 0.02,
    lift_step: 0.07,
};

const CUT: f32 = 0.012;
/// Turns a beat (33⅓ at 125 BPM is about 0.27; ours is livelier).
const RPM: f32 = 0.55;
/// The glove on the record's left edge (neck-relative, A units).
const GRIP: P = (-0.62, -0.80);

/// Two beats: the glove on the record's edge rocks it back and forth on the sixteenths (the
/// record follows), the tonearm pumps up on the beat, the head nods.
pub fn scratch(b: f32, st: &Style) -> Fig {
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let rock = math::sin(2.0 * PI * 2.0 * b);
    p.shape = 0.10 * rock - RPM * b; // cancel the turn, rock it: a scratch
    p.hands = [
        (GRIP.0, GRIP.1 + 0.16 * rock),
        lerp_p((0.95, 0.10), (0.85, -0.55), beat_down(ph, 1.6)),
    ];
    p.hip = (0.03 * cos1(b / 2.0), st.bounce * beat_down(ph, 2.0));
    p.lean = 4.0 * cos1(b / 2.0);
    p.nod = 10.0 * beat_down(pmod(b - st.lag, 1.0), 2.0) - 3.0;
    p.tilt = 6.0 * cos1(b / 2.0);
    p.eyes = if ph < 0.15 { Eyes::Cool } else { Eyes::Open };
    p.accent = beat_down(ph, 8.0);
    p.level = 0.5 + 0.45 * beat_down(ph, 3.0);
    p
}

/// Four beats of step-touch, the glove reaching for the record on each beat.
pub fn step(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    let ph = pmod(b, 1.0);
    p.hands = [
        lerp_p((-0.55, 0.92), GRIP, beat_down(ph, 1.2)),
        lerp_p((0.95, 0.20), (0.90, -0.45), beat_down(ph, 2.0)),
    ];
    p.eyes = Eyes::Open;
    p.level = 0.5 + 0.4 * beat_down(ph, 3.0);
    p
}

/// Two beats: the glove brakes the record (it stops), then spins it back three turns in a
/// blur, leaning back; on beat 2 it lets go — the record runs forward, arms up.
pub fn backspin(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    if bb < 1.0 {
        let brake = smooth(bb / 0.25);
        let back = ease_in(clamp01((bb - 0.3) / 0.7));
        p.shape = -RPM * b * brake - 3.0 * back;
        p.hands = [lerp_p((-0.55, 0.92), GRIP, brake), (0.95, 0.15)];
        p.lean = -10.0 * back;
        p.hip = (0.03 * back, 0.05 * back);
        p.eyes = if bb > 0.3 { Eyes::Wide } else { Eyes::Open };
        p.sig = back; // the blur
    } else {
        let t = bb - 1.0;
        let up = ease_out_back(clamp01(t / 0.25), 2.0);
        p.shape = -RPM - 3.0 + 1.2 * t; // forward again, fast
        p.hands = [
            lerp_p(GRIP, (-0.90, -1.05), up),
            lerp_p((0.95, 0.15), (0.80, -1.10), up),
        ];
        p.hip = (0.0, 0.12 * up * (1.0 - smooth((t - 0.5) / 0.5)));
        p.lean = 6.0 * up * (1.0 - t);
        p.eyes = Eyes::Cool;
        p.sig = (0.6 - t * 2.0).max(0.0);
    }
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p.level = if bb >= 1.0 { 1.0 } else { 0.4 };
    p
}

mod pal {
    use crate::render::rgb;
    pub const VINYL: [u8; 4] = rgb(0x0D1014);
    pub const GROOVE: [u8; 4] = rgb(0x262D34);
    pub const SHEEN_NIGHT: [u8; 4] = rgb(0x59636E);
    pub const SHEEN_PAPER: [u8; 4] = rgb(0x47515B);
    pub const RIM: [u8; 4] = rgb(0x76818D);
    pub const LABEL: [u8; 4] = rgb(0xE0693B);
    pub const LABEL_S: [u8; 4] = rgb(0xB24E29);
    pub const FACE: [u8; 4] = rgb(0x14191E);
    pub const PLINTH: [u8; 4] = rgb(0x2B3239);
    pub const LED_NIGHT: [u8; 4] = rgb(0x8FAAF5);
    pub const LED_PAPER: [u8; 4] = rgb(0x5B79D6);
    pub const ARM: [u8; 4] = rgb(0xB4BDC6);
    pub const GLOVE: [u8; 4] = rgb(0xF3F5F7);
    pub const GLOVE_S: [u8; 4] = rgb(0xBAC3CC);
    pub const LEGS: [u8; 4] = rgb(0x59636E);
    pub const SHELL: [u8; 4] = rgb(0x38444F);
}

pub struct Vinilo {
    info: CharacterInfo,
    family: dance::Style,
}

impl Vinilo {
    pub fn new() -> Vinilo {
        Vinilo {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Vinilo".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            family: dance::Style {
                bounce: 0.05,
                squash: 0.05,
                arms: 1.0,
                sway: 0.06,
                lag: 0.10,
                hop: 0.14,
                stride: 0.12,
            },
        }
    }
}

impl Default for Vinilo {
    fn default() -> Self {
        Vinilo::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Vinilo> {
    slate::Slate(Vinilo::new())
}

impl Figure for Vinilo {
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
            Move::Bounce => scratch(b, &STYLE),
            Move::StepLeft | Move::StepRight => step(b, &STYLE),
            Move::Jump => backspin(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Its eighth mood is spinning (Shy's place): the record a blur, a happy face.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Shy => {
                f.eyes = Eyes::Happy;
                f.sig = 1.0;
            }
            Mood::Excited => f.sig = f.sig.max(0.6),
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.56,
            top: -1.18,
            right: 0.56,
            bottom: 0.07,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        draw(f, pose, c);
    }
}

/// Points on an arc about `c` (degrees, 0 = right, 90 = down).
fn arc_pts<const N: usize>(c: P, r: f32, a0: f32, a1: f32) -> [P; N] {
    std::array::from_fn(|i| {
        let a = a0 + (a1 - a0) * i as f32 / (N - 1) as f32;
        let (s, co) = math::sin_cos(crate::rig::rad(a));
        add(c, (r * co, r * s))
    })
}

fn draw(f: &Fig, pose: &Pose, c: &mut Canvas) {
    let j = solve(f, &PROPS);
    let p = |q: P| place(q, &j);
    let mut br = Brush::new(c, CUT);
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let dark = br.ink.dark;
    let gold = br.hued(br.ink.gold);
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    slate::on_ground(br.c, pose, |c| {
        Brush::new(c, CUT).shadow((gx, 0.010), 0.20, 0.028, 0.5);
    });
    // Legs, sneakers with gold soles.
    let (legs, shoe) = (col(&br, pal::LEGS), col(&br, pal::GLOVE));
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        let ankle = add(foot, (0.0, -0.04));
        br.piece(
            &|sk, _| {
                capsule_into(sk, hip, knee, 0.072, 0.062, 0.0);
                capsule_into(sk, knee, ankle, 0.062, 0.058, 0.0);
            },
            legs,
            false,
        );
        let ang = -s * j.toes[i] * 0.6;
        let base = add(foot, (s * 0.020, -0.016));
        br.rrect(
            (base.0, base.1 + 0.006),
            0.130,
            0.022,
            0.010,
            ang,
            gold,
            false,
        );
        br.rrect(
            (base.0, base.1 - 0.018),
            0.120,
            0.046,
            0.020,
            ang,
            shoe,
            false,
        );
    }
    // The plinth: rim, body, a lapis light strip.
    let (top, bot) = (p(j.neck), add(p(j.pelvis), (0.0, 0.04)));
    let mid = mul(add(top, bot), 0.5);
    let h = length(sub(top, bot)) + 0.02;
    let deg = j.torso_deg;
    let (rim, plinth) = (col(&br, pal::RIM), col(&br, pal::PLINTH));
    br.rrect(mid, 0.27, h + 0.010, 0.045, deg, rim, true);
    br.rrect(mid, 0.26, h, 0.045, deg, plinth, false);
    let l = |x: f32, y: f32| add(mid, rot0((x, y), deg));
    let led = 0.4 + 0.6 * f.level;
    let strip = col(&br, if dark { pal::LED_NIGHT } else { pal::LED_PAPER });
    br.bars(
        &[
            l(-0.095, h / 2.0 - 0.040),
            l(-0.095 + 0.19 * led, h / 2.0 - 0.040),
        ],
        0.012,
        strip,
        false,
    );
    // DJ bling: a gold chain with a tiny record for a pendant, swinging a little late.
    let sw = 0.02 * math::sin(crate::rig::rad(f.lean * 3.0 + f.tilt * 2.0));
    let chain = [
        l(-0.085, -h / 2.0 + 0.010),
        l(-0.040 + sw, -h / 2.0 + 0.080),
        l(sw * 1.4, -h / 2.0 + 0.098),
        l(0.040 + sw, -h / 2.0 + 0.080),
        l(0.085, -h / 2.0 + 0.010),
    ];
    let pend = l(sw * 1.6, -h / 2.0 + 0.122);
    br.piece(
        &|s, _| {
            ribbon_into(s, &chain, 0.010);
            circle_into(s, pend, 0.024, 0.0);
        },
        gold,
        false,
    );
    let vinyl = col(&br, pal::VINYL);
    br.circle(pend, 0.016, vinyl, false);
    let label = br.hued(pal::LABEL);
    br.circle(pend, 0.006, label, false);
    record(&mut br, f, pose, &j, gold, label);
    arms(&mut br, &j, gold);
}

/// The record: its rim, the grooves, a sheen and a marker that turn, the label's face, the
/// headphones.
fn record(br: &mut Brush, f: &Fig, pose: &Pose, j: &slate::Joints, gold: Color, label: Color) {
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let hc = place(j.head, j);
    let r = PROPS.head;
    let turn = 360.0 * (RPM * pose.beats as f32 + f.shape);
    br.circle(hc, r + 0.007, col(br, pal::RIM), true);
    br.circle(hc, r, col(br, pal::VINYL), false);
    let groove = col(br, pal::GROOVE);
    // The grooves: four rings, one even-odd fill.
    br.piece_even_odd(
        &|s| {
            for gr in [0.172f32, 0.152, 0.132, 0.114] {
                ellipse_into(s, hc, gr + 0.002, gr + 0.002, 0.0);
                ellipse_into(s, hc, gr - 0.002, gr - 0.002, 0.0);
            }
        },
        groove,
    );
    let sheen = col(
        br,
        if br.ink.dark {
            pal::SHEEN_NIGHT
        } else {
            pal::SHEEN_PAPER
        },
    );
    let blur = 1 + (3.0 * f.sig) as usize;
    br.piece(
        &|s, _| {
            for k in 0..blur.min(4) {
                for base in [turn, turn + 180.0] {
                    let a = base - k as f32 * 22.0;
                    let pts: [P; 9] = arc_pts(hc, 0.150, a - 18.0, a + 18.0);
                    ribbon_into(s, &pts, 0.022 - 0.004 * k as f32);
                }
            }
        },
        sheen,
        false,
    );
    let (sm, cm) = math::sin_cos(crate::rig::rad(turn));
    br.circle(add(hc, (0.176 * cm, 0.176 * sm)), 0.010, gold, false);
    let hd = j.head_deg;
    let h = |x: f32, y: f32| add(hc, rot0((x, y), hd));
    let lr = 0.098;
    br.circle(hc, lr, br.hued(pal::LABEL_S), false);
    br.circle(add(hc, (-0.005, -0.005)), lr - 0.006, label, false);
    face(br, f, &h);
    br.circle(h(0.0, 0.010), 0.011, gold, false);
    // DJ headphones: the band over the record, one cup on, one pushed back.
    let band: [P; 11] = arc_pts(hc, r + 0.022, 200.0, 330.0);
    br.curve(&band, 0.016, col(br, pal::SHELL), true);
    br.rrect(h(-r - 0.010, 0.010), 0.042, 0.080, 0.016, hd, gold, true);
    br.rrect(
        h(r - 0.010, -0.150),
        0.040,
        0.070,
        0.016,
        hd + 50.0,
        gold,
        true,
    );
}

/// The label's face: eyes, brows, a DJ's half smile.
fn face(br: &mut Brush, f: &Fig, h: &dyn Fn(f32, f32) -> P) {
    let k = br.fixed(pal::FACE);
    let e = f.eyes;
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                let x = side * 0.036;
                let c = h(x, -0.026);
                match e {
                    Eyes::Happy => {
                        let pts = [h(x - 0.016, -0.020), h(x, -0.034), h(x + 0.016, -0.020)];
                        bars_into(s, &pts, 0.008, 0.0);
                    }
                    Eyes::Closed | Eyes::Sleepy => {
                        bars_into(s, &[h(x - 0.016, -0.026), h(x + 0.016, -0.024)], 0.008, 0.0);
                    }
                    Eyes::Wink if side > 0.0 => {
                        bars_into(s, &[h(x - 0.016, -0.026), h(x + 0.016, -0.024)], 0.008, 0.0);
                    }
                    Eyes::Cool => rrect_into(s, c, 0.044, 0.022, 0.008, 0.0, 0.0),
                    _ => {
                        let big = if e == Eyes::Wide { 1.3 } else { 1.0 };
                        ellipse_into(s, c, 0.012 * big, 0.016 * big, 0.0);
                    }
                }
                let lift = if e == Eyes::Cool { 0.006 } else { 0.0 };
                bars_into(
                    s,
                    &[h(side * 0.022, -0.052), h(side * 0.050, -0.056 + lift)],
                    0.007,
                    0.0,
                );
            }
            match e {
                Eyes::Wide => ellipse_into(s, h(0.0, 0.050), 0.014, 0.014, 0.0),
                Eyes::Happy => ribbon_into(
                    s,
                    &[h(-0.030, 0.040), h(0.0, 0.058), h(0.030, 0.040)],
                    0.008,
                ),
                _ => ribbon_into(
                    s,
                    &[h(-0.026, 0.050), h(0.004, 0.054), h(0.030, 0.042)],
                    0.008,
                ),
            }
        },
        k,
        false,
    );
}

/// The left arm, a sleeve and a white glove; the right, the tonearm — pivot,
/// counterweight, rod, headshell, stylus.
fn arms(br: &mut Brush, j: &slate::Joints, gold: Color) {
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let p = |q: P| place(q, j);
    let (sh, el, ha) = (p(j.shoulders[0]), p(j.elbows[0]), p(j.hands[0]));
    br.piece(
        &|s, g| {
            capsule_into(s, sh, el, 0.064 + 2.0 * g, 0.056 + 2.0 * g, 0.0);
            capsule_into(s, el, ha, 0.056 + 2.0 * g, 0.050 + 2.0 * g, 0.0);
        },
        col(br, pal::PLINTH),
        true,
    );
    br.piece(
        &|s, g| ellipse_into(s, ha, 0.040 + g, 0.036 + g, 0.0),
        col(br, pal::GLOVE_S),
        true,
    );
    br.piece(
        &|s, _| ellipse_into(s, add(ha, (-0.004, -0.004)), 0.035, 0.031, 0.0),
        col(br, pal::GLOVE),
        false,
    );
    let (sh, el, ha) = (p(j.shoulders[1]), p(j.elbows[1]), p(j.hands[1]));
    let up = sub(el, sh);
    let ln = length(up).max(1e-6);
    let cw = sub(sh, mul(up, 0.06 / ln));
    let shell = col(br, pal::SHELL);
    // The counterweight: a block behind the pivot (square ends).
    let mid = mul(add(sh, cw), 0.5);
    let ang = math::atan2(cw.1 - sh.1, cw.0 - sh.0) * (180.0 / PI);
    br.rrect(mid, length(sub(cw, sh)), 0.050, 0.0, ang, shell, true);
    br.bars(&[sh, el, ha], 0.020, col(br, pal::ARM), true);
    br.circle(sh, 0.034, gold, false);
    br.circle(sh, 0.014, col(br, pal::PLINTH), false);
    let fd = sub(ha, el);
    let fl = length(fd).max(1e-6);
    let ang = math::atan2(fd.1, fd.0) * (180.0 / PI);
    let head = add(ha, mul(fd, 0.020 / fl));
    br.rrect(head, 0.060, 0.034, 0.008, ang, shell, true);
    br.circle(add(head, rot0((0.022, 0.016), ang)), 0.009, gold, false);
}
