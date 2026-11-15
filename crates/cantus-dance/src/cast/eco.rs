//! **Eco**, the delay's ghost (`builtin:eco`, round 40, ADR 0122;
//! `docs/round40/design/dancer/eco.md`): a small floating ghost — a dome, a scalloped hem that
//! ripples, two arm nubs, little gold headphones, cut eyes — in the theme's ink, pale at
//! night and graphite on paper. Behind it **three echoes** repeat each move a quarter beat
//! late (Delay's taps): the same pose at an earlier time, each a value further into the
//! room's colour, opaque. It floats and grooves lightly; its signature on the drop,
//! **Feedback**: the echoes multiply in a widening spiral until they fill the frame — each
//! copy one fill, its silhouette alone — then all go out at once on the beat and the ghost
//! pops.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::draw::{blob_into, ellipse_into, mix, ribbon_into, Sink};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{bars_into, cel_blob, rrect_into, Brush};
use crate::slate::{self, Eyes, Fig, Figure, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use std::f32::consts::PI;
use tiny_skia::Color;

pub const ID: &str = "builtin:eco";

/// Unused by its drawing (a ghost, not a skeleton): the family's moves read it.
pub const PROPS: Props = Props {
    leg: 0.47,
    thigh: 0.245,
    shin: 0.235,
    torso: 0.30,
    neck: 0.035,
    head: 0.075,
    shoulder: 0.075,
    hipw: 0.045,
    stance: 0.06,
    upper: 0.165,
    fore: 0.16,
    foot: 0.06,
};

pub const STYLE: Style = Style {
    bounce: 0.04,
    sway: 0.10,
    step: 0.22,
    arms: 1.0,
    lag: 0.10,
    hop: 0.10,
    squash: 0.10,
    snap: 0.0,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: 0.02,
    lift_step: 0.07,
};

const CUT: f32 = 0.012;
/// The echoes: how many, how far behind (beats).
const ECHO_N: usize = 3;
const ECHO_STEP: f32 = 0.25;
/// Feedback's copies at the most.
const COPIES: usize = 13;

/// Two beats: floating, leaning side to side, a nub waving up on each beat.
pub fn float(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    p.lift = 0.03 * math::sin(PI * b) + 0.02 * beat_down(ph, 2.0);
    p.lean = 9.0 * cos1(b / 2.0);
    p.hip = (0.04 * cos1(b / 2.0), 0.0);
    let k = (b.floor() as i64).rem_euclid(2);
    let up = beat_down(ph, 1.5);
    p.hands = [
        (-1.0, if k == 0 { 0.2 - up } else { 0.3 }),
        (1.0, if k == 1 { 0.2 - up } else { 0.3 }),
    ];
    p.squash = 0.06 * beat_down(ph, 4.0);
    p.eyes = if ph < 0.2 { Eyes::Happy } else { Eyes::Open };
    p.mouth = 0.6 * beat_down(ph, 3.0);
    p.accent = beat_down(ph, 8.0);
    p
}

/// Four beats: the step-and-touch as a drift, leaning against it.
pub fn drift(b: f32, st: &Style) -> Fig {
    let q = moves::steps(b, st);
    let mut p = float(b, st);
    p.hip = (q.hip.0 * 1.6, 0.0);
    p.lean = -q.lean * 1.6;
    p.hands = [(-1.0, 0.6), (1.0, 0.6)];
    p
}

/// Two beats: the echoes multiply (sig 0 → 1) in a widening spiral while the ghost spins its
/// arms; on beat 2 every echo goes out at once and the ghost pops.
pub fn feedback(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    if bb < 1.0 {
        p.sig = ease_in(bb);
        p.lift = 0.04 + 0.04 * p.sig;
        p.lean = 12.0 * math::sin(bb * PI * 4.0);
        let (sa, ca) = math::sin_cos(bb * PI * 6.0);
        p.hands = [(-ca, sa), (ca, -sa)];
        p.eyes = if bb < 0.6 { Eyes::Happy } else { Eyes::Wide };
        p.mouth = 0.3 + 0.5 * p.sig;
    } else {
        let t = bb - 1.0;
        p.sig = 0.0;
        p.squash = -0.18 * (1.0 - ease_out(clamp01(t / 0.2)))
            + 0.08 * math::sin(clamp01(t / 0.6) * PI) * (1.0 - t);
        p.lift = 0.10 * (1.0 - ease_out(clamp01(t / 0.5)));
        p.hands = if t < 0.5 {
            [(-1.0, -1.0), (1.0, -1.0)]
        } else {
            [(-1.0, 0.3), (1.0, 0.3)]
        };
        p.eyes = if t < 0.4 { Eyes::Wide } else { Eyes::Happy };
        p.mouth = if t < 0.4 { 0.9 } else { 0.2 };
        p.landed = (1.0 - t * 2.5).max(0.0); // the flash ring
    }
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

const BODY_NIGHT: [u8; 4] = rgb(0xDCE3EA);
const SHADE_NIGHT: [u8; 4] = rgb(0xAEB8C2);
const BODY_PAPER: [u8; 4] = rgb(0x38444F);
const SHADE_PAPER: [u8; 4] = rgb(0x1F2730);

pub struct Eco {
    info: CharacterInfo,
    family: dance::Style,
}

impl Eco {
    pub fn new() -> Eco {
        Eco {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Eco".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            family: dance::Style {
                bounce: 0.05,
                squash: 0.10,
                arms: 1.0,
                sway: 0.08,
                lag: 0.10,
                hop: 0.14,
                stride: 0.12,
            },
        }
    }
}

impl Default for Eco {
    fn default() -> Self {
        Eco::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Eco> {
    slate::Slate(Eco::new())
}

impl Figure for Eco {
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
            Move::Bounce => float(b, &STYLE),
            Move::StepLeft | Move::StepRight => drift(b, &STYLE),
            Move::Jump => feedback(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Its eighth mood is a boo (Shy's place): eyes wide, the mouth open.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Shy => {
                f.eyes = Eyes::Wide;
                f.mouth = 1.0;
            }
            Mood::Excited => f.mouth = f.mouth.max(0.8),
            Mood::Surprised => f.mouth = f.mouth.max(0.5),
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin, Feedback's spiral at its widest;
    /// `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.72,
            top: -1.12,
            right: 0.62,
            bottom: 0.07,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        let mut echoes = [Fig::default(); ECHO_N];
        for (k, e) in echoes.iter_mut().enumerate() {
            *e = slate::fig_before(self, pose, (k + 1) as f32 * ECHO_STEP);
        }
        draw(f, &echoes, pose, c);
    }
}

/// Where the ghost floats: its centre, from the figure's pelvis offset and lift.
fn centre(f: &Fig) -> P {
    (f.hip.0 * 0.47, -0.50 - f.lift)
}

/// The ghost's outline (24 points): a dome, the sides, a rippling scalloped hem.
fn outline(f: &Fig, at: P, scale: f32, t: f32) -> [P; 24] {
    let sq = f.squash;
    let (sx, sy) = ((1.0 + 0.6 * sq) * scale, (1.0 - sq) * scale);
    let tr = |x: f32, y: f32| add(at, rot0((x * sx, y * sy), f.lean));
    let r = 0.17;
    let mut pts = [(0.0, 0.0); 24];
    for (k, q) in pts.iter_mut().take(13).enumerate() {
        let (s, c) = math::sin_cos(crate::rig::rad(180.0 + 15.0 * k as f32));
        *q = tr(r * c, -0.05 + r * s);
    }
    pts[13] = tr(r, 0.10);
    pts[14] = tr(r * 0.98, 0.15);
    for n in 0..7 {
        let x = r * (1.0 - 2.0 * (n as f32 + 0.5) / 7.0);
        let bump = if n % 2 == 1 { 0.025 } else { -0.005 };
        let y = 0.16 + 0.03 * math::sin(n as f32 * 2.2 + t * 6.0) + bump;
        pts[15 + n] = tr(x, y);
    }
    pts[22] = tr(-r * 0.98, 0.15);
    pts[23] = tr(-r, 0.10);
    pts
}

/// The ghost's colours, `tint` of the way into the room.
struct Tones {
    body: Color,
    shade: Color,
    gold: Color,
    cut: Color,
}

fn tones(br: &Brush, tint: f32) -> Tones {
    let dark = br.ink.dark;
    let (body, shade) = if dark {
        (BODY_NIGHT, SHADE_NIGHT)
    } else {
        (BODY_PAPER, SHADE_PAPER)
    };
    let room = br.ink.room;
    let cut = if tint == 0.0 {
        br.room
    } else {
        br.fixed(mix(room, mix(body, room, tint), 0.4))
    };
    Tones {
        body: br.fixed(mix(body, room, tint)),
        shade: br.fixed(mix(shade, room, tint)),
        gold: br.hued(mix(br.ink.gold, room, tint)),
        cut,
    }
}

fn draw(f: &Fig, echoes: &[Fig; ECHO_N], pose: &Pose, c: &mut Canvas) {
    let mut br = Brush::new(c, CUT);
    let at = centre(f);
    let (lift, landed) = (f.lift, f.landed);
    let ring = br.hued(mix(br.ink.gold, br.ink.room, 1.0 - landed));
    slate::on_ground(br.c, pose, |c| {
        let mut b = Brush::new(c, CUT);
        let k = (1.0 - (lift + 0.1) * 2.0).max(0.25);
        b.shadow((at.0, 0.006), 0.13 * k, 0.020 * k, 0.45);
    });
    if landed > 0.05 {
        // The flash ring as everything goes out.
        let r = 0.20 + 0.45 * (1.0 - landed);
        br.ring(at, r, r, 0.012 * landed + 0.002, ring);
    }
    // Feedback: the echoes multiplying in a widening spiral, one fill each, behind it all.
    if f.sig > 0.0 {
        let n = ((3.0 + 10.0 * f.sig) as usize).min(COPIES);
        for k in (1..=n).rev() {
            let a = crate::rig::rad(k as f32 * 47.0 + f.t * 90.0);
            let r = 0.10 + 0.045 * k as f32 * (0.5 + f.sig);
            let (sa, ca) = math::sin_cos(a);
            let copy = (at.0 + r * ca, at.1 + r * sa * 0.8);
            let tint = (0.30 + 0.05 * k as f32).min(0.85);
            let body = tones(&br, tint).body;
            let pts = outline(f, copy, 0.85, f.t);
            br.piece(&|s, _| blob_into(s, &pts), body, false);
        }
    }
    // The echoes: the same dance a quarter beat late each, trailing behind.
    for k in (1..=ECHO_N).rev() {
        let e = &echoes[k - 1];
        let ea = (
            e.hip.0 * 0.47 - 0.085 * k as f32,
            -0.50 - e.lift + 0.012 * k as f32,
        );
        let tint = 0.28 + 0.20 * (k - 1) as f32;
        ghost(
            &mut br,
            e,
            ea,
            tint,
            1.0 - 0.06 * k as f32,
            pose.beats as f32,
        );
    }
    ghost(&mut br, f, at, 0.0, 1.0, pose.beats as f32);
}

/// One ghost at `at`, `tint` toward the room (0 itself), at `scale`.
fn ghost(br: &mut Brush, f: &Fig, at: P, tint: f32, scale: f32, t: f32) {
    let tn = tones(br, tint);
    let pts = outline(f, at, scale, t);
    if tint == 0.0 {
        let room = br.room;
        br.piece(
            &|s, _| crate::rig::draw::blob_grown_into(s, &pts, CUT),
            room,
            false,
        );
    }
    cel_blob(br, &pts, tn.body, tn.shade, (-0.016, -0.012));
    let sq = f.squash;
    let (sx, sy) = ((1.0 + 0.6 * sq) * scale, (1.0 - sq) * scale);
    let tr = |x: f32, y: f32| add(at, rot0((x * sx, y * sy), f.lean));
    // Arm nubs toward the hands; little gold headphones.
    br.piece(
        &|s, _| {
            for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                let (hx, hy) = f.hands[i];
                let ang = math::atan2(hy, hx * side);
                let (sa, ca) = math::sin_cos(ang);
                let base = tr(side * 0.150, 0.03);
                let tip = add(
                    base,
                    rot0((side * 0.10 * ca * scale, 0.10 * sa * scale), f.lean),
                );
                bars_into(s, &[base, tip], 0.050 * scale, 0.0);
            }
        },
        tn.body,
        false,
    );
    let band: [P; 10] = std::array::from_fn(|k| {
        let (s, c) = math::sin_cos(crate::rig::rad(205.0 + 15.0 * k as f32));
        tr(0.15 * c, -0.05 + 0.15 * s - 0.02)
    });
    br.piece(
        &|s, _| {
            ribbon_into(s, &band, 0.016 * scale);
            for side in [-1.0f32, 1.0] {
                rrect_into(
                    s,
                    tr(side * 0.150, -0.075),
                    0.034 * scale,
                    0.056 * scale,
                    0.014 * scale,
                    f.lean,
                    0.0,
                );
            }
        },
        tn.gold,
        false,
    );
    face(br, f, &tr, tn.cut);
}

/// The cut eyes and mouth (negative space).
fn face(br: &mut Brush, f: &Fig, tr: &dyn Fn(f32, f32) -> P, cut: Color) {
    let e = f.eyes;
    br.piece(
        &|s: &mut dyn Sink, _| {
            for side in [-1.0f32, 1.0] {
                let x = side * 0.056;
                match e {
                    Eyes::Happy => {
                        let pts = [tr(x - 0.024, -0.040), tr(x, -0.064), tr(x + 0.024, -0.040)];
                        bars_into(s, &pts, 0.016, 0.0);
                    }
                    Eyes::Closed | Eyes::Sleepy => {
                        let pts = [tr(x - 0.024, -0.050), tr(x, -0.040), tr(x + 0.024, -0.050)];
                        bars_into(s, &pts, 0.014, 0.0);
                    }
                    Eyes::Wink if side > 0.0 => {
                        let pts = [tr(x - 0.024, -0.050), tr(x, -0.040), tr(x + 0.024, -0.050)];
                        bars_into(s, &pts, 0.014, 0.0);
                    }
                    Eyes::Cool => {
                        if side < 0.0 {
                            let (a, z) = (tr(-0.090, -0.052), tr(0.090, -0.052));
                            let ang = math::atan2(z.1 - a.1, z.0 - a.0) * (180.0 / PI);
                            rrect_into(
                                s,
                                mul(add(a, z), 0.5),
                                length(sub(z, a)),
                                0.034,
                                0.0,
                                ang,
                                0.0,
                            );
                        }
                    }
                    _ => {
                        let big = if e == Eyes::Wide { 1.25 } else { 1.0 };
                        ellipse_into(s, tr(x, -0.050), 0.024 * big, 0.034 * big, f.lean);
                    }
                }
            }
            if f.mouth > 0.2 {
                ellipse_into(s, tr(0.0, 0.020), 0.020, 0.010 + 0.022 * f.mouth, f.lean);
            } else {
                ribbon_into(
                    s,
                    &[tr(-0.022, 0.012), tr(0.0, 0.024), tr(0.022, 0.012)],
                    0.012,
                );
            }
        },
        cut,
        false,
    );
}
