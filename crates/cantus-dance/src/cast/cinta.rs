//! **Cinta**, the cassette (`builtin:cinta`, round 40, ADR 0122;
//! `docs/round40/design/dancer/cinta.md`): an 80s cassette that breakdances. Its body and
//! head are a cassette — a graphite shell with the knob's rim, a cream label with two retro
//! stripes (vermilion and lapis; no brand, no logo), two **reels for eyes** whose hubs turn
//! as it dances (lids of the label make the moods), the tape opening below its mouth. A
//! **backwards lapis cap** with a gold button, thin graphite limbs, white gloves, track
//! pants, chunky sneakers with gold laces; its **tape slips out** and swirls with every move,
//! following the body late. It grooves with a breakdancer's **toprock**; on the drop, its
//! signature, **Rewind**: the dance runs backwards at eight times the speed — the reels spin
//! back, the tape whips, a gold ◀◀ flashes — then it hits a freeze on the beat.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance;
use crate::rig::draw::{self as d, blob_into, ellipse_into, polygon_into, ribbon_into, Look};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, keyed, Style};
use crate::slate::paint::{bars_into, capsule_into, circle_into, rrect_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use std::f32::consts::PI;
use tiny_skia::Color;

pub const ID: &str = "builtin:cinta";

pub const PROPS: Props = Props {
    leg: 0.40,
    thigh: 0.208,
    shin: 0.198,
    torso: 0.24,
    neck: 0.02,
    head: 0.10,
    shoulder: 0.150,
    hipw: 0.050,
    stance: 0.070,
    upper: 0.150,
    fore: 0.145,
    foot: 0.12,
};

pub const STYLE: Style = Style {
    bounce: 0.10,
    sway: 0.06,
    step: 0.20,
    arms: 1.0,
    lag: 0.12,
    hop: 0.12,
    squash: 0.06,
    snap: 0.35,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: 0.02,
    lift_step: 0.10,
};

const CUT: f32 = 0.012;
/// The cassette: its width and height.
const W: f32 = 0.42;
const HC: f32 = 0.27;

/// Two beats of toprock: on each beat a foot crosses in front of the other with the arms
/// flung open; on the off-beat it steps back, the fists in at the chest.
pub fn toprock(b: f32, st: &Style) -> Fig {
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let k = (b.floor() as i64).rem_euclid(2);
    let cross = math::powf(math::sin(PI * ph).max(0.0), 0.8);
    let s = if k == 0 { 1.0 } else { -1.0 };
    let (neg, pos) = (
        if s < 0.0 { 1.0 } else { 0.0 },
        if s > 0.0 { 1.0 } else { 0.0 },
    );
    p.feet = [
        (0.18 * cross * neg, -0.04 * cross * neg),
        (-0.18 * cross * pos, -0.04 * cross * pos),
    ];
    p.hands = lerp_pair(
        [(-0.52, 0.78), (0.52, 0.78)],
        [(-0.95, 0.10), (0.95, 0.10)],
        beat_down(ph, 1.6),
    );
    p.hip = (-0.05 * s * cross, st.bounce * beat_down(ph, 2.0));
    p.lean = -7.0 * s * cross;
    p.nod = 10.0 * beat_down(pmod(b - st.lag, 1.0), 2.0) - 3.0;
    p.tilt = 8.0 * s;
    p.eyes = Eyes::Open;
    p.point = 50.0 * math::sin(PI * (b - st.lag));
    p.accent = beat_down(ph, 8.0);
    p.level = 0.5 + 0.4 * beat_down(ph, 3.0);
    p
}

/// Four beats of step-touch with the arms thrown out wide and up on the closes.
pub fn shuffle(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    let ph = pmod(b, 1.0);
    let keys = [
        (0.0, [(-0.90, 0.40), (0.90, 0.40)]),
        (1.0, [(-0.25, 0.45), (0.85, -0.70)]),
        (2.0, [(-0.90, 0.40), (0.90, 0.40)]),
        (3.0, [(-0.85, -0.70), (0.25, 0.45)]),
    ];
    let (a, z, t) = keyed(&keys, b, 4.0);
    p.hands = lerp_pair(a, z, ease_in_out(t));
    p.point = 50.0 * math::sin(PI * (b - st.lag) / 2.0);
    p.eyes = if pmod(b, 2.0) > 0.8 {
        Eyes::Happy
    } else {
        Eyes::Open
    };
    p.level = 0.5 + 0.4 * beat_down(ph, 3.0);
    p
}

/// Two beats: the toprock runs backwards at ×8 for a beat (reels back, tape whipping, a gold
/// ◀◀); on beat 2 a freeze — a hand on the hip, a fist up, a leg out.
pub fn rewind(b: f32, st: &Style) -> Fig {
    let bb = pmod(b, 2.0);
    let mut p = if bb < 1.0 {
        let mut p = toprock(8.0 - bb * 8.0, st);
        p.shape = -3.0 * bb; // the reels spin back, fast
        p.sig = 1.0;
        p.point = 80.0 * math::sin(bb * 30.0);
        p.eyes = Eyes::Wide;
        p
    } else {
        let t = bb - 1.0;
        let hit = ease_out_back(clamp01(t / 0.2), 2.4);
        Fig {
            hands: lerp_pair(
                [(-0.52, 0.78), (0.52, 0.78)],
                [(-0.42, 0.78), (0.70, -1.0)],
                hit,
            ),
            feet: [(0.0, 0.0), (0.40 * hit, -0.10 * hit)],
            hip: (-0.06 * hit, 0.10 * hit),
            lean: -12.0 * hit,
            tilt: -10.0 * hit,
            eyes: Eyes::Cool,
            shape: -3.0,
            sig: 0.0,
            point: -40.0 * hit,
            ..Fig::default()
        }
    };
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

mod pal {
    use crate::render::rgb;
    pub const SHELL: [u8; 4] = rgb(0x2B3239);
    pub const RIM: [u8; 4] = rgb(0x76818D);
    pub const LABEL: [u8; 4] = rgb(0xEDE6DA);
    pub const VERM: [u8; 4] = rgb(0xE0693B);
    pub const LAPIS: [u8; 4] = rgb(0x4664C0);
    pub const WINDOW: [u8; 4] = rgb(0x0D1014);
    pub const HUB: [u8; 4] = rgb(0xF3F5F7);
    pub const LIMB: [u8; 4] = rgb(0x59636E);
    pub const GLOVE: [u8; 4] = rgb(0xF3F5F7);
    pub const GLOVE_S: [u8; 4] = rgb(0xBAC3CC);
    pub const CAP_S: [u8; 4] = rgb(0x2E4794);
    pub const TAPE: [u8; 4] = rgb(0x8A5A32);
    pub const TAPE_L: [u8; 4] = rgb(0xC08A52);
    pub const SOLE: [u8; 4] = rgb(0x2B3239);
}

pub struct Cinta {
    info: CharacterInfo,
    family: dance::Style,
}

impl Cinta {
    pub fn new() -> Cinta {
        Cinta {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Cinta".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            family: dance::Style {
                bounce: 0.06,
                squash: 0.06,
                arms: 1.2,
                sway: 0.07,
                lag: 0.12,
                hop: 0.18,
                stride: 0.15,
            },
        }
    }
}

impl Default for Cinta {
    fn default() -> Self {
        Cinta::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Cinta> {
    slate::Slate(Cinta::new())
}

impl Figure for Cinta {
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
            Move::Bounce => toprock(b, &STYLE),
            Move::StepLeft | Move::StepRight => shuffle(b, &STYLE),
            Move::Jump => rewind(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Its eighth mood is rewinding (Shy's place): ◀◀ over its head, the tape whipping.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Shy => {
                f.eyes = Eyes::Wide;
                f.sig = 1.0;
            }
            Mood::Excited | Mood::Surprised => f.mouth = f.mouth.max(0.8),
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.58,
            top: -1.1,
            right: 0.58,
            bottom: 0.18,
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
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let gold = br.hued(br.ink.gold);
    let lapis = br.hued(pal::LAPIS);
    let verm = br.hued(pal::VERM);
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    slate::on_ground(br.c, pose, |c| {
        Brush::new(c, CUT).shadow((gx, 0.010), 0.20, 0.028, 0.5);
    });
    let deg = j.torso_deg;
    let neck = p(j.neck);
    let l = |x: f32, y: f32| add(neck, rot0((x, y), deg));
    let beats = pose.beats as f32;
    // The tape slipping out of the bottom, swirling late (behind the legs).
    let whip = 1.0 + 1.5 * f.sig;
    let tape: [P; 16] = std::array::from_fn(|i| {
        let s = i as f32 / 15.0;
        let a = 2.0 * PI * (1.1 * s) + beats * 2.2 + crate::rig::rad(f.point) * 0.8;
        let w = 6.0 * PI * s + beats * 3.0;
        let x = 0.02 + 0.16 * s * math::sin(a) * whip + 0.05 * s * math::sin(w);
        let y = HC / 2.0 + 0.01 + 0.40 * s + 0.04 * s * math::cos(w) * whip;
        l(x, y)
    });
    br.curve(&tape, 0.018, br.hued(pal::TAPE), true);
    br.curve(&tape, 0.005, br.hued(pal::TAPE_L), false);
    // Track pants (lapis, a vermilion stripe), sneakers with gold laces.
    let pelvis = p(j.pelvis);
    let (shoe, sole) = (col(&br, pal::GLOVE), col(&br, pal::SOLE));
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        let top = add(add(hip, mul(sub(pelvis, hip), 0.3)), (0.0, -0.03));
        let pts = [top, knee, add(foot, (0.0, -0.040))];
        br.piece(
            &|sk, _| {
                capsule_into(sk, pts[0], pts[1], 0.084, 0.070, 0.0);
                capsule_into(sk, pts[1], pts[2], 0.070, 0.074, 0.0);
            },
            lapis,
            false,
        );
        let stripe = pts.map(|q| add(q, (s * 0.030, 0.0)));
        br.bars(&stripe, 0.010, verm, false);
        let ang = -s * j.toes[i] * 0.6;
        let base = add(foot, (s * 0.024, -0.016));
        br.rrect(
            (base.0, base.1 + 0.008),
            0.150,
            0.024,
            0.012,
            ang,
            sole,
            false,
        );
        br.rrect(
            (base.0, base.1 - 0.020),
            0.136,
            0.056,
            0.024,
            ang,
            shoe,
            false,
        );
        let a = add(base, rot0((-0.020 * s, -0.036), ang));
        let z = add(base, rot0((0.020 * s, -0.026), ang));
        br.bars(&[a, z], 0.010, gold, false);
    }
    br.rrect(
        (pelvis.0, pelvis.1 - 0.04),
        0.17,
        0.10,
        0.03,
        deg,
        lapis,
        false,
    );
    cassette(&mut br, f, pose, &l, deg, gold, lapis, verm);
    // Arms: thin limbs from the cassette's sides, white gloves.
    let (limb, glove_s, glove) = (
        col(&br, pal::LIMB),
        col(&br, pal::GLOVE_S),
        col(&br, pal::GLOVE),
    );
    for i in 0..2 {
        let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
        br.bars(&[sh, el, ha], 0.040, limb, true);
        br.piece(
            &|s, g| ellipse_into(s, ha, 0.040 + g, 0.036 + g, 0.0),
            glove_s,
            true,
        );
        br.piece(
            &|s, _| ellipse_into(s, add(ha, (-0.004, -0.004)), 0.035, 0.031, 0.0),
            glove,
            false,
        );
    }
}

/// The cassette: rim, shell, screws, the label with its stripes, the window with the reels,
/// the tape opening; the backwards cap; ◀◀ while it rewinds.
#[allow(clippy::too_many_arguments)]
fn cassette(
    br: &mut Brush,
    f: &Fig,
    pose: &Pose,
    l: &dyn Fn(f32, f32) -> P,
    deg: f32,
    gold: Color,
    lapis: Color,
    verm: Color,
) {
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let c = l(0.0, -0.03);
    br.rrect(
        c,
        W + 0.012,
        HC + 0.012,
        0.030,
        deg,
        col(br, pal::RIM),
        true,
    );
    br.rrect(c, W, HC, 0.030, deg, col(br, pal::SHELL), false);
    br.piece(
        &|s, _| {
            for sx in [-1.0f32, 1.0] {
                for sy in [-1.0f32, 1.0] {
                    circle_into(
                        s,
                        l(sx * (W / 2.0 - 0.022), -0.03 + sy * (HC / 2.0 - 0.020)),
                        0.008,
                        0.0,
                    );
                }
            }
        },
        col(br, pal::RIM),
        false,
    );
    let label = col(br, pal::LABEL);
    br.rrect(
        l(0.0, -0.06),
        W - 0.060,
        HC * 0.62,
        0.016,
        deg,
        label,
        false,
    );
    br.rrect(l(0.0, -0.135), W - 0.060, 0.022, 0.002, deg, verm, false);
    br.rrect(l(0.0, -0.108), W - 0.060, 0.014, 0.002, deg, lapis, false);
    let window = col(br, pal::WINDOW);
    br.rrect(l(0.0, -0.040), 0.27, 0.090, 0.040, deg, window, false);
    let spin = 360.0 * (0.8 * pose.beats as f32 + f.shape);
    reels(br, f, l, spin, deg, label, window);
    // The tape opening below: its mouth.
    let m = if f.mouth > 0.0 {
        0.4 + 0.6 * f.mouth
    } else {
        0.4
    };
    let mouth = [
        l(-0.070, 0.090),
        l(0.070, 0.090),
        l(0.085, 0.090 + 0.035 * m),
        l(-0.085, 0.090 + 0.035 * m),
    ];
    br.piece(&|s, _| polygon_into(s, &mouth), window, false);
    // The backwards cap (two values), its brim behind, a gold button.
    let top = -0.03 - HC / 2.0;
    let mut cap = [(0.0, 0.0); 15];
    for (k, q) in cap.iter_mut().take(13).enumerate() {
        let (sa, ca) = math::sin_cos(crate::rig::rad(180.0 + 15.0 * k as f32));
        *q = l(-0.120 + 0.13 * ca + 0.12, top + 0.075 * sa);
    }
    cap[13] = l(0.12, top + 0.006);
    cap[14] = l(-0.12, top + 0.006);
    let room = br.room;
    br.piece(&|s, _| d::blob_grown_into(s, &cap, CUT), room, false);
    let look = Look {
        base: pal::LAPIS,
        shade: pal::CAP_S,
        light: pal::LAPIS,
        rim: [0, 0, 0, 0],
    };
    d::bands(br.c, &|s, _| blob_into(s, &cap), &look, 2);
    let brim = l(0.120, top - 0.004);
    br.rrect(
        (brim.0 + 0.035, brim.1 - 0.010),
        0.10,
        0.022,
        0.010,
        deg - 18.0,
        br.hued(pal::CAP_S),
        false,
    );
    br.circle(l(0.0, top - 0.074), 0.012, gold, false);
    if f.sig > 0.0 {
        // ◀◀ over its head while it rewinds.
        br.piece(
            &|s, g| {
                for k in [0.0f32, 1.0] {
                    let x0 = -0.02 + k * 0.07;
                    let tri = [
                        l(x0 - 0.045, -0.36),
                        l(x0 + 0.020, -0.40),
                        l(x0 + 0.020, -0.32),
                    ];
                    crate::slate::paint::poly_round_into(s, &tri, g);
                }
            },
            gold,
            true,
        );
    }
}

/// Two reels — the eyes: white hubs with six teeth, turning; lids of the label.
fn reels(
    br: &mut Brush,
    f: &Fig,
    l: &dyn Fn(f32, f32) -> P,
    spin: f32,
    deg: f32,
    label: Color,
    window: Color,
) {
    let e = f.eyes;
    let hub = br.fixed(pal::HUB);
    let shut = |side: f32| {
        matches!(e, Eyes::Closed | Eyes::Sleepy | Eyes::Happy) || (e == Eyes::Wink && side > 0.0)
    };
    let r = 0.036 * if e == Eyes::Wide { 1.15 } else { 1.0 };
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                let x = side * 0.082;
                if shut(side) {
                    let pts = if e == Eyes::Happy {
                        [l(x - 0.030, -0.034), l(x, -0.058), l(x + 0.030, -0.034)]
                    } else {
                        [l(x - 0.030, -0.040), l(x, -0.028), l(x + 0.030, -0.040)]
                    };
                    ribbon_into(s, &pts, 0.012);
                } else {
                    circle_into(s, l(x, -0.040), r, 0.0);
                }
            }
        },
        hub,
        false,
    );
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                if shut(side) {
                    continue;
                }
                let c = l(side * 0.082, -0.040);
                for k in 0..6 {
                    let (sa, ca) = math::sin_cos(crate::rig::rad(spin + k as f32 * 60.0));
                    let a = add(c, (r * 0.25 * ca, r * 0.25 * sa));
                    let z = add(c, (r * 0.62 * ca, r * 0.62 * sa));
                    bars_into(s, &[a, z], 0.008, 0.0);
                }
                circle_into(s, c, r * 0.22, 0.0);
            }
        },
        window,
        false,
    );
    if e == Eyes::Cool {
        // A lid of the label across the top: unimpressed.
        br.piece(
            &|s, _| {
                for side in [-1.0f32, 1.0] {
                    let lid = l(side * 0.082, -0.040 - r * 0.55);
                    rrect_into(s, lid, r * 2.3, r, 0.004, deg, 0.0);
                }
            },
            label,
            false,
        );
    }
}
