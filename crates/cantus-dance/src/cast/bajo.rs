//! **Bajo**, the speaker (`builtin:bajo`, round 40, ADR 0122;
//! `docs/round40/design/dancer/bajo.md`): a studio monitor with a heavy, calm groove — the
//! Slate cast's first character that is not a skeleton with a head. A tall cabinet is its
//! body and head, rigid, turning about the pelvis: two tweeters near the top are its eyes (a
//! gold dome for each pupil, lids of the baffle for its moods; its neutral is a heavy,
//! unimpressed half-lid), the woofer below is its chest and pumps with the music, the bass
//! port is its belt. Short thick legs on isolation pads, short arms with big mitts. Graphite
//! in both themes, held at 3:1 on Night by the knob's rim. It moves like weight: a slow
//! heavy nod, a stomp, and on the drop its signature, **Drop the bass**: it winds up, then
//! the woofer blasts — gold shockwave rings, the cabinet jolts and shakes.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::draw::mix;
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{circle_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use tiny_skia::Color;

pub const ID: &str = "builtin:bajo";

pub const PROPS: Props = Props {
    leg: 0.20,
    thigh: 0.108,
    shin: 0.100,
    torso: 0.36,
    neck: 0.02,
    head: 0.10,
    shoulder: 0.235,
    hipw: 0.10,
    stance: 0.11,
    upper: 0.13,
    fore: 0.13,
    foot: 0.14,
};

pub const STYLE: Style = Style {
    bounce: 0.16,
    sway: 0.10,
    step: 0.30,
    arms: 1.0,
    lag: 0.16,
    hop: 0.08,
    squash: 0.10,
    snap: 0.25,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: 0.02,
    lift_step: 0.16,
};

const CUT: f32 = 0.014;
/// The cabinet: its width, how far it reaches above the neck and below the pelvis.
const CAB_W: f32 = 0.46;
const CAB_TOP: f32 = 0.40;
const CAB_BOT: f32 = 0.04;

/// Two beats: the cabinet rocks side to side and squashes on each beat, the knees take the
/// weight, the woofer pumps; the mitts swing low and slow.
pub fn heavy(b: f32, st: &Style) -> Fig {
    let b = snap_time(b, 0.5, 0.5, st.snap, 1.9);
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let down = beat_down(ph, 1.6);
    let side = cos1(b / 2.0);
    p.hip = (0.03 * side, st.bounce * down);
    p.lean = 5.0 * side;
    p.squash = 0.06 * beat_down(ph, 3.0);
    p.hands = [(-0.62 - 0.12 * side, 0.92), (0.62 - 0.12 * side, 0.92)];
    p.eyes = Eyes::Open;
    p.accent = beat_down(ph, 8.0);
    p.level = 0.45 + 0.55 * beat_down(ph, 2.5);
    p.nod = 6.0 * beat_down(pmod(b - st.lag, 1.0), 2.0);
    p.point = 20.0 * side;
    p
}

/// Four beats: the step-touch as stomps — each foot lifts high and lands hard, the cabinet
/// squashing on landing, the arms out for balance.
pub fn stomp(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    let ph = pmod(b, 1.0);
    p.squash = 0.09 * beat_down(ph, 5.0);
    p.landed = (1.0 - ph * 4.0).max(0.0);
    p.hands = [(-0.95, 0.55), (0.95, 0.55)];
    p.lean *= 1.4;
    p.eyes = Eyes::Open;
    p.level = 0.5 + 0.45 * beat_down(ph, 3.0);
    p
}

/// Two beats, its signature: it crouches and winds up (mitts raised, squashed), holds a
/// breath — then on beat 2 the woofer blasts: rings, a jolt up, a shake, and it settles.
pub fn drop_the_bass(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    let wind = smooth(bb / 0.8);
    let blast = clamp01((bb - 1.0) / 1.0);
    let hit = if bb >= 1.0 { 1.0 } else { 0.0 };
    let settle = smooth((bb - 1.5) / 0.5);
    p.squash = 0.14 * wind * (1.0 - hit) - 0.10 * hit * (1.0 - smooth(blast * 3.0))
        + 0.02 * hit * smooth(blast * 3.0);
    p.hip = (0.0, 0.14 * wind * (1.0 - hit) + 0.02 * hit);
    p.lift = 0.06 * hit * arc(clamp01(blast * 2.5));
    // The shake: a sine at a fixed rate under an envelope (a pure function of the phase).
    let shake = 0.018 * hit * (1.0 - settle) * math::sin(bb * 60.0);
    p.hip.0 += shake;
    let raised = [(-0.95, -0.30), (0.95, -0.30)];
    let flung = [(-1.05, 0.40), (1.05, 0.40)];
    let rest = [(-0.62, 0.92), (0.62, 0.92)];
    let h = lerp_pair(rest, raised, wind);
    let h = lerp_pair(h, flung, ease_out_back(clamp01(blast * 4.0), 2.0) * hit);
    p.hands = lerp_pair(h, rest, settle);
    p.feet = [(-0.10, 0.0), (0.10, 0.0)];
    p.sig = blast * hit;
    p.eyes = if 0.6 < bb && bb < 1.0 {
        Eyes::Closed
    } else if hit > 0.0 && blast < 0.35 {
        Eyes::Wide
    } else {
        Eyes::Open
    };
    p.level = (hit * (1.0 - settle * 0.5)).max(0.3);
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

/// Graphite in both themes (a monitor is black).
mod pal {
    use crate::render::rgb;
    pub const CAB: [u8; 4] = rgb(0x262D34);
    pub const CAB_LIP: [u8; 4] = rgb(0x3A434C);
    pub const BAFFLE: [u8; 4] = rgb(0x2F373F);
    pub const RIM: [u8; 4] = rgb(0x76818D);
    pub const EDGE: [u8; 4] = rgb(0x0D1014);
    pub const SURROUND: [u8; 4] = rgb(0x14191E);
    pub const CONE: [u8; 4] = rgb(0x4C5661);
    pub const CONE_S: [u8; 4] = rgb(0x3A434C);
    pub const PORT: [u8; 4] = rgb(0x0D1014);
    pub const LIMB: [u8; 4] = rgb(0x6A7682);
    pub const MITT: [u8; 4] = rgb(0xB4BDC6);
    pub const PAD: [u8; 4] = rgb(0x14191E);
}

/// The domes' gold (the logo's, in both themes) and their glint.
pub const DOME: [u8; 4] = rgb(0xE2B85A);
pub const GLINT: [u8; 4] = rgb(0xFFF1C9);

pub struct Bajo {
    info: CharacterInfo,
    family: dance::Style,
}

impl Bajo {
    pub fn new() -> Bajo {
        Bajo {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Bajo".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            // Weight: a deep bob, a slow sway, short steps.
            family: dance::Style {
                bounce: 0.04,
                squash: 0.10,
                arms: 0.9,
                sway: 0.05,
                lag: 0.16,
                hop: 0.10,
                stride: 0.08,
            },
        }
    }
}

impl Default for Bajo {
    fn default() -> Self {
        Bajo::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Bajo> {
    slate::Slate(Bajo::new())
}

impl Figure for Bajo {
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
            Move::Bounce => heavy(b, &STYLE),
            Move::StepLeft | Move::StepRight => stomp(b, &STYLE),
            Move::Jump => drop_the_bass(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Its eighth mood is the blast (Shy's place): the woofer at full, the first rings out.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Shy => {
                f.eyes = Eyes::Wide;
                f.level = 1.0;
                f.sig = f.sig.max(0.35);
            }
            Mood::Excited => f.accent = f.accent.max(0.6),
            Mood::Sleepy => f.level = f.level.min(0.15),
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin, the rings at their largest
    /// (0.73 about the woofer); `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.78,
            top: -1.22,
            right: 0.78,
            bottom: 0.22,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        draw(f, pose, c);
    }
}

fn draw(f: &Fig, pose: &Pose, c: &mut Canvas) {
    let j = solve(f, &PROPS);
    let p = |q: P| place(q, &j);
    let mut br = Brush::new(c, CUT);
    let s = j.squash;
    let (sx, sy) = (1.0 + 0.6 * s, 1.0 - s);
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    let (lift, landed) = (f.lift, f.landed);
    let feet = [p(j.feet[0]), p(j.feet[1])];
    let dust = br.ink.ink4;
    slate::on_ground(br.c, pose, |c| {
        let mut br = Brush::new(c, CUT);
        let k = (1.0 - lift * 4.0).max(0.0);
        br.shadow((gx, 0.01), 0.26 * k + 0.05, 0.032 * k + 0.008, 0.6);
        if landed > 0.05 {
            // Dust where a stomp lands.
            let r = 0.03 + 0.05 * (1.0 - landed);
            for (n, sd) in [-1.0f32, 1.0].into_iter().enumerate() {
                br.blot((feet[n].0 + sd * 0.09, -0.012), r, r * 0.45, dust, landed);
            }
        }
    });
    // Legs on isolation pads.
    let limb = br.fixed(pal::LIMB);
    for (i, sd) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        br.bars(&[hip, knee, add(foot, (0.0, -0.03))], 0.072, limb, false);
        let ang = -sd * j.toes[i] * 0.4;
        let rim = br.fixed(pal::RIM);
        br.rrect(
            (foot.0 + sd * 0.012, foot.1 - 0.024),
            0.16,
            0.052,
            0.018,
            ang,
            rim,
            true,
        );
        let pad = br.fixed(pal::PAD);
        br.rrect(
            (foot.0 + sd * 0.012, foot.1 - 0.026),
            0.148,
            0.040,
            0.014,
            ang,
            pad,
            false,
        );
    }
    // The cabinet, rigid, turned with the torso about the pelvis.
    let deg = j.torso_deg;
    let t = PROPS.torso;
    let pelvis = p(j.pelvis);
    let cab = |x: f32, y: f32| add(pelvis, rot0((x * sx, y * sy), deg));
    let cy = (-(t + CAB_TOP) + CAB_BOT) / 2.0;
    let h = t + CAB_TOP + CAB_BOT;
    let mid = cab(0.0, cy);
    let rim = br.fixed(pal::RIM);
    br.rrect(mid, CAB_W * sx, h * sy, 0.06, deg, rim, true);
    let lip = br.fixed(pal::CAB_LIP);
    br.rrect(
        mid,
        (CAB_W - 0.010) * sx,
        (h - 0.010) * sy,
        0.06,
        deg,
        lip,
        false,
    );
    let inner = cab(0.0, cy + 0.006);
    let body = br.fixed(pal::CAB);
    br.rrect(
        inner,
        (CAB_W - 0.016) * sx,
        (h - 0.024) * sy,
        0.052,
        deg,
        body,
        false,
    );
    let baffle = br.fixed(pal::BAFFLE);
    br.rrect(
        inner,
        (CAB_W - 0.060) * sx,
        (h - 0.070) * sy,
        0.036,
        deg,
        baffle,
        false,
    );
    // The woofer: surround, cone (two values), gold dust cap; it pumps with the level.
    let wc = cab(0.0, -0.30);
    let pump = 1.0 + 0.07 * f.level + 0.05 * f.accent;
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    br.circle(wc, 0.168, col(&br, pal::SURROUND), false);
    br.circle(wc, 0.142 * pump, col(&br, pal::CONE_S), false);
    br.circle(
        add(wc, (-0.008, -0.010)),
        0.126 * pump,
        col(&br, pal::CONE),
        false,
    );
    let dome = br.hued(DOME);
    br.circle(wc, 0.050 * pump, dome, false);
    if f.sig > 0.0 {
        // The blast: three gold rings out from the woofer, fading into the room.
        let (gold, room) = (br.ink.gold, br.ink.room);
        for n in 0..3 {
            let t = clamp01(f.sig * 1.6 - n as f32 * 0.22);
            if 0.0 < t && t < 1.0 {
                let r = 0.18 + 0.55 * t;
                let c = br.hued(mix(gold, room, t));
                br.ring(wc, r, r * 0.92, 0.016 * (1.0 - t) + 0.003, c);
            }
        }
    }
    // The port: its belt.
    let port = br.fixed(pal::PORT);
    br.rrect(
        cab(0.0, -0.03),
        0.20 * sx,
        0.034 * sy,
        0.017,
        deg,
        port,
        false,
    );
    eyes(&mut br, f, &cab, deg);
    // Arms from the cabinet's sides, big mitts.
    let (edge, mitt) = (br.fixed(pal::EDGE), br.fixed(pal::MITT));
    for i in 0..2 {
        let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
        br.bars(&[sh, el, ha], 0.060, limb, true);
        br.circle(ha, 0.054, edge, false);
        br.circle(ha, 0.047, mitt, false);
    }
}

/// Two tweeters: a dark ring, a gold dome for a pupil; moods are lids of the baffle.
fn eyes(br: &mut Brush, f: &Fig, cab: &dyn Fn(f32, f32) -> P, deg: f32) {
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let (edge, baffle) = (col(br, pal::EDGE), col(br, pal::BAFFLE));
    if f.eyes == Eyes::Cool {
        br.bars(&[cab(-0.16, -0.62), cab(0.16, -0.62)], 0.110, edge, false);
        let gold = br.hued(br.ink.gold);
        br.bars(
            &[cab(-0.13, -0.645), cab(-0.07, -0.645)],
            0.010,
            gold,
            false,
        );
        return;
    }
    let big = if f.eyes == Eyes::Wide { 1.15 } else { 1.0 };
    let (rim, sur, dome, glint) = (
        col(br, pal::RIM),
        col(br, pal::SURROUND),
        br.hued(DOME),
        col(br, GLINT),
    );
    // Both tweeters in one fill a layer.
    let both = |br: &mut Brush, off: P, r: f32, c: Color| {
        br.piece(
            &|s, _| {
                for sd in [-1.0f32, 1.0] {
                    circle_into(s, add(cab(sd * 0.095, -0.62), off), r, 0.0);
                }
            },
            c,
            false,
        );
    };
    both(br, (0.0, 0.0), 0.060 * big, edge);
    both(br, (0.0, 0.0), 0.050 * big, rim);
    both(br, (0.0, 0.0), 0.040 * big, sur);
    both(br, (0.004, 0.004), 0.020 * big, dome);
    both(br, (-0.006, -0.008), 0.006, glint);
    for sd in [-1.0f32, 1.0] {
        let x = sd * 0.095;
        // Shy is a wink: the left lid shut, the right one heavy.
        let e = match f.eyes {
            Eyes::Wink if sd < 0.0 => Eyes::Closed,
            Eyes::Wink => Eyes::Open,
            e => e,
        };
        match e {
            Eyes::Open | Eyes::Sleepy => {
                // A heavy lid: calm and sure.
                let drop = if e == Eyes::Open { 0.030 } else { 0.052 };
                br.rrect(
                    cab(x, -0.62 - 0.064 + drop),
                    0.15,
                    0.064,
                    0.01,
                    deg,
                    baffle,
                    false,
                );
                let a = cab(x - 0.062, -0.62 - 0.032 + drop);
                let z = cab(x + 0.062, -0.62 - 0.032 + drop - sd * 0.004);
                br.bars(&[a, z], 0.010, edge, false);
            }
            Eyes::Happy | Eyes::Closed => {
                let happy = e == Eyes::Happy;
                let (ly, lh) = if happy {
                    (-0.62 + 0.040, 0.070)
                } else {
                    (-0.62, 0.13)
                };
                br.rrect(cab(x, ly), 0.15, lh, 0.03, deg, baffle, false);
                let y = -0.62 + 0.006;
                let pts = if happy {
                    [
                        cab(x - 0.05, y + 0.012),
                        cab(x, y - 0.012),
                        cab(x + 0.05, y + 0.012),
                    ]
                } else {
                    [cab(x - 0.05, y), cab(x, y + 0.012), cab(x + 0.05, y)]
                };
                br.curve(&pts, 0.014, dome, false);
            }
            _ => {}
        }
    }
}
