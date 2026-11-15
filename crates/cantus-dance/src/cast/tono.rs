//! **Tono**, the knob (`builtin:tono`, round 40, ADR 0122;
//! `docs/round40/design/dancer/tono.md`): Slate's parts with an attitude. A knob for a head
//! — its scale of dots lit by the level, a gold fin for a pointer swinging late — on a lean
//! robot body that is a key with a dark well of five vermilion bars (its voice: it has no
//! mouth), an **LED visor** for eyes, a vermilion patch cable slung round its neck with a
//! gold plug, high-top fader-cap sneakers. It dances with control: a hard nod with fist
//! hits, a mechanical glide with robot arms, and on the drop its signature, **Turn it up**:
//! it reaches up and turns its own knob, three clicks with an overshoot each, the dots
//! filling to the top.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{bars_into, circle_into, rrect_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Joints, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use tiny_skia::Color;

pub const ID: &str = "builtin:tono";

pub const PROPS: Props = Props {
    leg: 0.30,
    thigh: 0.158,
    shin: 0.152,
    torso: 0.25,
    neck: 0.035,
    head: 0.165,
    shoulder: 0.105,
    hipw: 0.052,
    stance: 0.068,
    upper: 0.16,
    fore: 0.155,
    foot: 0.11,
};

pub const STYLE: Style = Style {
    bounce: 0.10,
    sway: 0.06,
    step: 0.22,
    arms: 1.0,
    lag: 0.14,
    hop: 0.18,
    squash: 0.08,
    snap: 0.45,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: 0.02,
    lift_step: 0.02,
};

const CUT: f32 = 0.013;

/// Two beats: leaning back, knees bouncing, the head nodding hard on the beat; a fist drops
/// across the body on each beat, one hand then the other.
pub fn nod(b: f32, st: &Style) -> Fig {
    let b = snap_time(b, 0.5, 0.4, st.snap, 1.9);
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let k = (b.floor() as i64).rem_euclid(2);
    let down = beat_down(ph, 2.0);
    p.hip = (0.03 * cos1(b / 2.0), st.bounce * down);
    p.lean = -5.0 + 3.0 * cos1(b / 2.0);
    p.chest = (0.02 * cos1(b / 2.0), 0.04 * down);
    p.nod = 16.0 * beat_down(pmod(b - st.lag, 1.0), 2.0) - 6.0;
    p.tilt = 9.0 + 4.0 * cos1(b / 2.0);
    let hit = beat_down(ph, 3.0);
    p.hands = [
        lerp_p((-0.48, 0.95), (-0.10, 0.62), if k == 1 { hit } else { 0.0 }),
        lerp_p((0.48, 0.95), (0.10, 0.62), if k == 0 { hit } else { 0.0 }),
    ];
    p.toe = [0.0, 12.0 * beat_down(pmod(ph + 0.5, 1.0), 2.0)];
    p.eyes = Eyes::Open;
    p.accent = beat_down(ph, 8.0);
    p.level = 0.5 + 0.45 * down;
    p.point = 26.0 * cos1((b - 0.2) / 2.0);
    p
}

/// Four beats: the step-touch as a glide — the feet slide, barely lifted — with robot arms
/// (elbows held at a right angle, forearms pumping on the half beat, snapped).
pub fn glide(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, &Style { snap: 0.0, ..*st });
    let bs = snap_time(b, 0.5, 0.3, 0.8, 1.9);
    let up = [(-0.62, 0.30), (0.62, 0.62)];
    let dn = [(-0.62, 0.62), (0.62, 0.30)];
    p.hands = moves::hands_linear(&[(0.0, up), (0.5, dn), (1.0, up), (1.5, dn)], bs, 2.0);
    p.elbow = [1.0, 1.0];
    p.lean *= -0.6;
    let in4 = pmod(b, 4.0);
    p.tilt = 10.0 * math::sin(std::f32::consts::PI * in4 / 2.0) + 6.0;
    p.eyes = if (1.0..1.5).contains(&in4) || (3.0..3.5).contains(&in4) {
        Eyes::Cool
    } else {
        Eyes::Open
    };
    p.level = 0.55 + 0.35 * beat_down(pmod(b, 1.0), 3.0);
    p.point = 22.0 * math::sin(std::f32::consts::PI * (b - st.lag) / 2.0);
    p
}

/// Two beats, its signature on the drop: the right hand reaches up to its own pointer; it
/// turns the knob in three clicks (each with an overshoot), leaning back, the dots filling
/// to the top and the visor scanning; then it lets go.
pub fn turn_up(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    let reach = smooth(bb / 0.45);
    let letgo = smooth((bb - 1.6) / 0.4);
    let clicks: f32 = (0..3)
        .map(|n| ease_out_back(clamp01((bb - (0.5 + n as f32 * 0.34)) / 0.16), 2.4) / 3.0)
        .sum();
    p.sig = clicks * (1.0 - letgo);
    p.lean = -4.0 - 8.0 * p.sig;
    let bump = if 0.5 < bb && bb < 1.6 {
        beat_down(pmod(bb * 3.0, 1.0), 4.0)
    } else {
        0.0
    };
    p.hip = (0.04 * p.sig, 0.05 + 0.05 * bump);
    p.feet = [(-0.06, 0.0), (0.12, 0.0)];
    p.toe = [0.0, 18.0 * p.sig];
    let rest_r = (0.48, 0.95);
    let (a_len, r) = (PROPS.arm(), PROPS.head + 0.03);
    // The grip travels round the rim as it turns.
    let (sa, ca) = math::sin_cos(crate::rig::rad(-55.0 + 70.0 * p.sig));
    let grip = (
        r * ca / a_len,
        (-(PROPS.neck + PROPS.head) + r * sa) / a_len,
    );
    let hand_r = lerp_p(lerp_p(rest_r, grip, reach), rest_r, letgo);
    p.hands = [
        lerp_p((-0.48, 0.95), (-0.95, 0.30), reach * (1.0 - letgo)),
        hand_r,
    ];
    p.tilt = -6.0 - 6.0 * p.sig;
    p.nod = -8.0 * p.sig;
    p.eyes = if p.sig > 0.2 { Eyes::Cool } else { Eyes::Open };
    p.level = p.sig.max(0.4);
    p.point = 0.0;
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

/// Its colours in a dark room and a light one.
struct Pal {
    knob: [u8; 4],
    knob_lip: [u8; 4],
    rim: [u8; 4],
    body: [u8; 4],
    body_lip: [u8; 4],
    /// `None`: the room's own colour (Paper).
    edge: Option<[u8; 4]>,
    well: [u8; 4],
    slot: [u8; 4],
    limb: [u8; 4],
    joint: [u8; 4],
    shoe: [u8; 4],
    sole: [u8; 4],
    dot_off: [u8; 4],
}

const NIGHT: Pal = Pal {
    knob: rgb(0x2F373F),
    knob_lip: rgb(0x3B444D),
    rim: rgb(0x76818D),
    body: rgb(0x2B3239),
    body_lip: rgb(0x3A434C),
    edge: Some(rgb(0x0F1317)),
    well: rgb(0x14191E),
    slot: rgb(0x0D1014),
    limb: rgb(0x7D8996),
    joint: rgb(0x59636E),
    shoe: rgb(0x434C56),
    sole: rgb(0x0D1014),
    dot_off: rgb(0x3A434C),
};

const PAPER: Pal = Pal {
    knob: rgb(0xFBFCFD),
    knob_lip: rgb(0xFFFFFF),
    rim: rgb(0x7A8692),
    body: rgb(0xE4E9EE),
    body_lip: rgb(0xFBFCFD),
    edge: None,
    well: rgb(0x2B3239),
    slot: rgb(0x1D2329),
    limb: rgb(0x6A7682),
    joint: rgb(0x556170),
    shoe: rgb(0xFBFCFD),
    sole: rgb(0x38444F),
    dot_off: rgb(0xC5CDD5),
};

/// The visor's LED gold and the scan's highlight (never turned by the hue).
pub const LED: [u8; 4] = rgb(0xF2C766);
pub const SCAN: [u8; 4] = rgb(0xFFF1C9);

pub struct Tono {
    info: CharacterInfo,
    family: dance::Style,
}

impl Tono {
    pub fn new() -> Tono {
        Tono {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Tono".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            // Control, not bounce: a firm bob, little squash, arms that hit.
            family: dance::Style {
                bounce: 0.05,
                squash: 0.05,
                arms: 1.0,
                sway: 0.05,
                lag: 0.14,
                hop: 0.18,
                stride: 0.12,
            },
        }
    }
}

impl Default for Tono {
    fn default() -> Self {
        Tono::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Tono> {
    slate::Slate(Tono::new())
}

impl Figure for Tono {
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
            Move::Bounce => nod(b, &STYLE),
            Move::StepLeft | Move::StepRight => glide(b, &STYLE),
            Move::Jump => turn_up(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Excited fills the dots; sleepy dims them.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Excited => f.level = 1.0,
            Mood::Sleepy => f.level = f.level.min(0.1),
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.54,
            top: -1.10,
            right: 0.54,
            bottom: 0.07,
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
    let pal = if br.ink.dark { &NIGHT } else { &PAPER };
    let col = |br: &Brush, rgba: [u8; 4]| br.fixed(rgba);
    let edge = pal.edge.map_or(br.room, |e| br.fixed(e));
    let gold = br.hued(br.ink.gold);
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    let lift = f.lift;
    slate::on_ground(br.c, pose, |c| {
        let k = (1.0 - lift * 3.0).max(0.0);
        Brush::new(c, CUT).shadow((gx, 0.01), 0.19 * k + 0.05, 0.026 * k + 0.008, 0.55);
    });
    // Legs: straight robot segments with knee caps.
    let (limb, joint) = (col(&br, pal.limb), col(&br, pal.joint));
    for i in 0..2 {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        br.bars(&[hip, knee, add(foot, (0.0, -0.05))], 0.048, limb, false);
        br.circle(knee, 0.030, joint, false);
    }
    // High-top fader-cap sneakers: a sole, the cap, a gold stripe.
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let foot = p(j.feet[i]);
        let ang = -s * j.toes[i] * 0.6;
        let c = (foot.0 + s * 0.016, foot.1 - 0.040);
        br.rrect(
            (c.0, c.1 + 0.028),
            0.136,
            0.026,
            0.012,
            ang,
            col(&br, pal.sole),
            true,
        );
        br.rrect(
            (c.0, c.1 - 0.006),
            0.120,
            0.072,
            0.022,
            ang,
            col(&br, pal.shoe),
            false,
        );
        let a = add(c, rot0((-0.040 * s, 0.012), ang));
        let b = add(c, rot0((0.020 * s, -0.022), ang));
        br.bars(&[a, b], 0.011, gold, false);
    }
    body(&mut br, f, &j, pal, edge);
    head(&mut br, f, &j, pal, edge, gold);
    cable(&mut br, f, &j, gold);
    // Arms: straight segments, elbow joints, fists; cut where they cross the body.
    let fist = br.fixed(br.ink.ink2);
    let room = br.room;
    for i in 0..2 {
        let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
        let free = add(sh, mul(sub(el, sh), 0.5));
        br.bars(&[free, el, ha], 0.046 + 2.0 * CUT, room, false);
        br.bars(&[sh, el, ha], 0.046, limb, false);
        br.piece(
            &|s, _| {
                circle_into(s, el, 0.027, 0.0);
                circle_into(s, sh, 0.030, 0.0);
            },
            joint,
            false,
        );
        br.circle(ha, 0.040, edge, false);
        br.circle(ha, 0.034, fist, false);
    }
}

/// The body: a key — cut, rim, lip, surface — and a dark well with the five bars.
fn body(br: &mut Brush, f: &Fig, j: &Joints, pal: &Pal, edge: Color) {
    let p = |q: P| place(q, j);
    let (top, bot) = (p(j.neck), p(j.pelvis));
    let mid = mul(add(top, bot), 0.5);
    let h = length(sub(top, bot)) + 0.05;
    let deg = j.torso_deg;
    let w = 0.235 * (1.0 + j.squash * 0.25);
    br.rrect(mid, w + 0.020, h + 0.020, 0.060, deg, edge, true);
    let rim = br.fixed(pal.rim);
    br.rrect(mid, w + 0.008, h + 0.008, 0.054, deg, rim, false);
    let lip = br.fixed(pal.body_lip);
    br.rrect(mid, w, h, 0.046, deg, lip, false);
    let surface = br.fixed(pal.body);
    br.rrect(
        (mid.0, mid.1 + 0.006),
        w - 0.004,
        h - 0.012,
        0.043,
        deg,
        surface,
        false,
    );
    let well_c = add(mid, rot0((0.0, 0.006), deg));
    let well = br.fixed(pal.well);
    br.rrect(well_c, 0.160, 0.090, 0.018, deg, well, false);
    let level = f.level;
    let verm = br.fixed(br.ink.verm);
    br.piece(
        &|s, _| {
            for (n, x) in [-0.052f32, -0.026, 0.0, 0.026, 0.052]
                .into_iter()
                .enumerate()
            {
                let shape = [0.4, 0.7, 1.0, 0.7, 0.4][n];
                let lvl =
                    0.25 + 0.75 * level * (0.7 + 0.3 * math::sin(n as f32 * 1.7 + level * 9.0));
                let bh = 0.064 * shape * lvl + 0.010;
                let c = add(well_c, rot0((x, 0.0), deg));
                rrect_into(s, c, 0.014, bh, 0.007, deg, 0.0);
            }
        },
        verm,
        false,
    );
}

/// A jack cable slung round the neck: across the collar, over the right shoulder, its gold
/// plug hanging and swinging late.
fn cable(br: &mut Brush, f: &Fig, j: &Joints, gold: Color) {
    let neck = place(j.neck, j);
    let l = |x: f32, y: f32| add(neck, rot0((x, y), j.torso_deg));
    let sw = 0.0012 * f.point.clamp(-40.0, 40.0) - 0.02 * math::sin(crate::rig::rad(f.lean));
    let pts = [
        l(-0.075, 0.012),
        l(-0.030, 0.052),
        l(0.030, 0.050),
        l(0.085, 0.010),
        l(0.120 + sw * 0.4, 0.075),
        l(0.128 + sw, 0.150),
        l(0.122 + sw * 1.6, 0.205),
    ];
    let verm = br.fixed(br.ink.verm);
    br.curve(&pts, 0.020, verm, true);
    let (a, z) = (pts[5], pts[6]);
    let ang = math::atan2(z.1 - a.1, z.0 - a.0) * (180.0 / std::f32::consts::PI) - 90.0;
    let tip = add(z, rot0((0.0, 0.028), ang));
    br.rrect(tip, 0.030, 0.052, 0.008, ang, gold, true);
    br.bars(
        &[
            add(z, rot0((0.0, 0.050), ang)),
            add(z, rot0((0.0, 0.066), ang)),
        ],
        0.010,
        gold,
        false,
    );
}

/// The knob: the scale of dots, the rim and its lip, the pointer fin, the visor.
fn head(br: &mut Brush, f: &Fig, j: &Joints, pal: &Pal, edge: Color, gold: Color) {
    let hc = place(j.head, j);
    let r = PROPS.head;
    let hd = j.head_deg;
    let lit = (clamp01(f.level) * 11.0).round() as usize;
    let dot = |n: usize| {
        let (s, c) = math::sin_cos(crate::rig::rad(135.0 + n as f32 * 27.0));
        add(hc, rot0((c * (r + 0.042), s * (r + 0.042)), hd))
    };
    br.piece(
        &|s, _| (0..lit.min(11)).for_each(|n| circle_into(s, dot(n), 0.012, 0.0)),
        gold,
        false,
    );
    let off = br.fixed(pal.dot_off);
    br.piece(
        &|s, _| (lit.min(11)..11).for_each(|n| circle_into(s, dot(n), 0.010, 0.0)),
        off,
        false,
    );
    br.circle(hc, r + 0.010, edge, true);
    let rim = br.fixed(pal.rim);
    br.circle(hc, r, rim, false);
    let lip = br.fixed(pal.knob_lip);
    br.circle(hc, r - 0.011, lip, false);
    let knob = br.fixed(pal.knob);
    br.circle(add(hc, (0.004, 0.006)), r - 0.015, knob, false);
    // The pointer: a gold fin through the rim, swinging late (turned by Turn it up: fully
    // turned, 5 o'clock).
    let pa = -90.0 + f.point * 0.5 + hd + f.sig * 135.0;
    let (s, c) = math::sin_cos(crate::rig::rad(pa));
    let u = (c, s);
    br.bars(
        &[add(hc, mul(u, r - 0.07)), add(hc, mul(u, r + 0.022))],
        0.026,
        gold,
        true,
    );
    visor(br, f, hc, r, hd, pal, gold);
}

/// The LED visor: a dark slot across the knob; gold segments are its eyes.
fn visor(br: &mut Brush, f: &Fig, hc: P, r: f32, hd: f32, pal: &Pal, gold: Color) {
    let h = |x: f32, y: f32| add(hc, rot0((x * r, y * r), hd));
    let slot = br.fixed(pal.slot);
    br.rrect(h(0.0, 0.12), r * 1.42, r * 0.40, r * 0.20, hd, slot, false);
    let led = br.fixed(LED);
    match f.eyes {
        Eyes::Cool => {
            // One bar the whole width, a bright scan running along it.
            br.bars(&[h(-0.55, 0.12), h(0.55, 0.12)], r * 0.12, gold, false);
            let x = -0.45 + 0.9 * pmod(f.t * 0.5, 1.0);
            let scan = br.fixed(SCAN);
            br.bars(
                &[h(x - 0.08, 0.12), h(x + 0.08, 0.12)],
                r * 0.14,
                scan,
                false,
            );
        }
        Eyes::Wide => br.piece(
            &|s, _| {
                for side in [-1.0f32, 1.0] {
                    rrect_into(
                        s,
                        h(side * 0.30, 0.12),
                        r * 0.24,
                        r * 0.24,
                        r * 0.05,
                        hd,
                        0.0,
                    );
                }
            },
            led,
            false,
        ),
        Eyes::Closed | Eyes::Sleepy => {
            let c = if f.eyes == Eyes::Sleepy {
                br.hued(br.ink.gold_deep)
            } else {
                led
            };
            br.piece(
                &|s, _| {
                    for side in [-1.0f32, 1.0] {
                        let cx = side * 0.30;
                        bars_into(s, &[h(cx - 0.16, 0.16), h(cx + 0.16, 0.16)], r * 0.06, 0.0);
                    }
                },
                c,
                false,
            );
        }
        e => br.piece(
            &|s, _| {
                for side in [-1.0f32, 1.0] {
                    let cx = side * 0.30;
                    if e == Eyes::Happy {
                        let pts = [h(cx - 0.16, 0.19), h(cx, 0.05), h(cx + 0.16, 0.19)];
                        bars_into(s, &pts, r * 0.10, 0.0);
                    } else if e == Eyes::Wink && side < 0.0 {
                        bars_into(s, &[h(cx - 0.16, 0.16), h(cx + 0.16, 0.16)], r * 0.06, 0.0);
                    } else {
                        // A keen bar, its inner end a little higher: confident, not cute.
                        let pts = [h(cx - 0.17 * side, 0.15), h(cx + 0.15 * side, 0.10)];
                        bars_into(s, &pts, r * 0.12, 0.0);
                    }
                }
            },
            led,
            false,
        ),
    }
}
