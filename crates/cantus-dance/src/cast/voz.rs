//! **Voz**, the microphone (`builtin:voz`, round 40, ADR 0122;
//! `docs/round40/design/dancer/voz.md`): a crooner, smooth and showy. Its head is a vintage
//! microphone's capsule — a grille of fine bars over a metal body, a gold ring at its base —
//! under a pork-pie hat with a gold band. It wears the evening: a dinner jacket white at
//! night and black on paper (it inverts with the theme, as the inks do), a gold bow tie, a
//! vermilion pocket square, slim trousers, two-tone spectator shoes. It moves like a crooner:
//! a hand on the heart, then out to the crowd; a finger-snapping slide; and on the drop its
//! signature, **Hat tip**: it tips its hat with a bow, then flings both arms out — ta-da.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::draw::{blob_into, ellipse_into, polygon_into};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, keyed, Style};
use crate::slate::paint::{bars_into, cel_blob, cel_rrect, cel_tube, rrect_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Joints, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use std::f32::consts::PI;

pub const ID: &str = "builtin:voz";

pub const PROPS: Props = Props {
    leg: 0.49,
    thigh: 0.255,
    shin: 0.240,
    torso: 0.262,
    neck: 0.050,
    head: 0.088,
    shoulder: 0.090,
    hipw: 0.044,
    stance: 0.058,
    upper: 0.155,
    fore: 0.150,
    foot: 0.10,
};

pub const STYLE: Style = Style {
    bounce: 0.05,
    sway: 0.07,
    step: 0.16,
    arms: 1.0,
    lag: 0.16,
    hop: 0.10,
    squash: 0.04,
    snap: 0.30,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: -0.04,
    lift_step: 0.03,
};

const CUT: f32 = 0.011;

/// Two beats: on beat 1 the right hand to the heart, on beat 2 out to the crowd, palm up,
/// the head back; the left hand in the pocket; a smooth sway.
pub fn croon(b: f32, st: &Style) -> Fig {
    let b = snap_time(b, 0.5, 0.5, st.snap, 1.9);
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let keys = [
        (0.0, [(-0.36, 0.86), (0.04, 0.44)]),
        (1.0, [(-0.36, 0.86), (0.96, 0.10)]),
    ];
    let (a, z, t) = keyed(&keys, b, 2.0);
    p.hands = lerp_pair(a, z, ease_in_out(t));
    let side = cos1(b / 2.0);
    p.hip = (0.06 * side, st.bounce * beat_down(ph, 2.0));
    p.chest = (st.chest * side, 0.0);
    p.lean = -2.0 - 4.0 * (1.0 - side) / 2.0;
    p.nod = 6.0 * side - 4.0;
    p.tilt = -6.0 * side;
    p.toe = [0.0, 10.0 * (-side).max(0.0)];
    p.eyes = if side > 0.3 {
        Eyes::Closed
    } else {
        Eyes::Happy
    };
    p.level = 0.5 + 0.4 * beat_down(ph, 2.0);
    p.accent = beat_down(ph, 8.0);
    p
}

/// Four beats: a smooth slide, the feet barely lifting, both hands snapping fingers at
/// shoulder height, one up on each beat.
pub fn slide(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    let bs = snap_time(b, 0.5, 0.35, 0.6, 1.9);
    let a = [(-0.58, 0.34), (0.58, 0.34)];
    let up_l = [(-0.66, 0.06), (0.58, 0.34)];
    let up_r = [(-0.58, 0.34), (0.66, 0.06)];
    p.hands = moves::hands_linear(&[(0.0, up_r), (0.5, a), (1.0, up_l), (1.5, a)], bs, 2.0);
    p.claps = beat_down(pmod(b, 1.0), 8.0);
    p.eyes = Eyes::Open;
    p.tilt = 8.0 * math::sin(PI * b / 2.0);
    p.lean *= 0.6;
    p
}

/// Two beats, its signature: the hand to the brim, the hat tipped with a bow; then the hat
/// back on and both arms flung out — ta-da — leaning back, a knee popped; held.
pub fn hat_tip(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    let reach = smooth(bb / 0.35);
    let tip = smooth((bb - 0.35) / 0.35) * (1.0 - smooth((bb - 0.85) / 0.2));
    let tada = ease_out_back(clamp01((bb - 0.95) / 0.30), 2.2);
    let out = smooth((bb - 1.8) / 0.2);
    let brim = (0.30, -0.95);
    let mut right = lerp_p((0.42, 0.90), brim, reach);
    right = lerp_p(right, (0.48, -1.12), tip);
    right = lerp_p(right, (1.02, -0.30), tada);
    right = lerp_p(right, (0.42, 0.90), out);
    let mut left = lerp_p((-0.42, 0.90), (-0.24, 0.72), reach * (1.0 - tada));
    left = lerp_p(left, (-1.02, -0.20), tada * (1.0 - out));
    p.hands = [left, right];
    p.sig = tip;
    p.nod = 22.0 * tip - 10.0 * tada * (1.0 - out);
    p.lean = -9.0 * tada * (1.0 - out);
    p.hip = (0.04 * tada * (1.0 - out), 0.02 + 0.05 * tip);
    p.chest = (0.0, 0.05 * tip);
    p.feet = [(-0.06 * tada, 0.0), (0.10 * tada, 0.0)];
    p.toe = [0.0, 22.0 * tada * (1.0 - out)];
    p.knee = [1.0, -1.0];
    p.eyes = if tip > 0.3 {
        Eyes::Closed
    } else if tada > 0.3 {
        Eyes::Happy
    } else {
        Eyes::Open
    };
    p.level = 0.5 + 0.5 * tada * (1.0 - out);
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

/// The evening, inverted with the theme: (night, paper).
struct Pal {
    jacket: [u8; 4],
    jacket_s: [u8; 4],
    lapel: [u8; 4],
    shirt: [u8; 4],
    pants: [u8; 4],
    pants_s: [u8; 4],
    grille: [u8; 4],
    grille_s: [u8; 4],
    bar: [u8; 4],
    body: [u8; 4],
    hat_rim: [u8; 4],
    eye: [u8; 4],
    tie: [u8; 4],
}

const NIGHT: Pal = Pal {
    jacket: rgb(0xE9E4DA),
    jacket_s: rgb(0xBDB4A4),
    lapel: rgb(0xD2CABB),
    shirt: rgb(0x14191E),
    pants: rgb(0x59636E),
    pants_s: rgb(0x3E4752),
    grille: rgb(0xC3CBD3),
    grille_s: rgb(0x8A96A2),
    bar: rgb(0x7D8996),
    body: rgb(0x3A434C),
    hat_rim: rgb(0x76818D),
    eye: rgb(0x14191E),
    tie: rgb(0xE2B85A),
};

const PAPER: Pal = Pal {
    jacket: rgb(0x232A31),
    jacket_s: rgb(0x14191E),
    lapel: rgb(0x323A42),
    shirt: rgb(0xF3F5F7),
    pants: rgb(0x2B3239),
    pants_s: rgb(0x1D2329),
    grille: rgb(0x6A7682),
    grille_s: rgb(0x47515B),
    bar: rgb(0x3A434C),
    body: rgb(0x2B3239),
    hat_rim: rgb(0x2B3239),
    eye: rgb(0xF3F5F7),
    tie: rgb(0xC99A3A),
};

const HAT: [u8; 4] = rgb(0x2B3239);
const SHOE_A: [u8; 4] = rgb(0xF3F5F7);
const SHOE_B: [u8; 4] = rgb(0x14191E);
const GLOVE: [u8; 4] = rgb(0xF3F5F7);
const GLOVE_S: [u8; 4] = rgb(0xBAC3CC);
const RING: [u8; 4] = rgb(0xE2B85A);
const RING_S: [u8; 4] = rgb(0xB38A35);

pub struct Voz {
    info: CharacterInfo,
    family: dance::Style,
}

impl Voz {
    pub fn new() -> Voz {
        Voz {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Voz".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            family: dance::Style {
                bounce: 0.04,
                squash: 0.04,
                arms: 1.1,
                sway: 0.07,
                lag: 0.16,
                hop: 0.14,
                stride: 0.13,
            },
        }
    }
}

impl Default for Voz {
    fn default() -> Self {
        Voz::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Voz> {
    slate::Slate(Voz::new())
}

impl Figure for Voz {
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
            Move::Bounce => croon(b, &STYLE),
            Move::StepLeft | Move::StepRight => slide(b, &STYLE),
            Move::Jump => hat_tip(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Its eighth mood is the hat tip (Shy's place): eyes closed, the hat lifted, a nod.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        if mood == Mood::Shy {
            f.eyes = Eyes::Closed;
            f.sig = 1.0;
            f.nod += 16.0;
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.52,
            top: -1.24,
            right: 0.52,
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
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    let lift = f.lift;
    slate::on_ground(br.c, pose, |c| {
        let k = (1.0 - lift * 3.0).max(0.0);
        Brush::new(c, CUT).shadow((gx, 0.008), 0.15 * k + 0.05, 0.022 * k + 0.008, 0.5);
    });
    // Slim trousers, spectator shoes.
    let pelvis = p(j.pelvis);
    let (pants, pants_s) = (col(&br, pal.pants), col(&br, pal.pants_s));
    for i in 0..2 {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        let top = add(add(hip, mul(sub(pelvis, hip), 0.3)), (0.0, -0.03));
        let pts = [top, knee, add(foot, (0.0, -0.040))];
        cel_tube(
            &mut br,
            pts,
            [0.084, 0.068, 0.064],
            pants,
            pants_s,
            (-0.011, -0.004),
        );
    }
    let (shoe_a, shoe_b) = (col(&br, SHOE_A), col(&br, SHOE_B));
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let foot = p(j.feet[i]);
        let ang = -s * j.toes[i] * 0.6;
        let base = add(foot, (s * 0.024, -0.012));
        let at = |x: f32, y: f32| add(base, rot0((x * s, y), ang));
        let up = [
            at(-0.050, 0.004),
            at(-0.046, -0.030),
            at(-0.010, -0.040),
            at(0.040, -0.026),
            at(0.070, -0.004),
            at(0.068, 0.006),
        ];
        br.piece(&|sk, _| blob_into(sk, &up), shoe_b, false);
        let up2 = [
            at(-0.044, -0.002),
            at(-0.040, -0.028),
            at(-0.008, -0.036),
            at(0.020, -0.028),
            at(0.022, -0.002),
        ];
        br.piece(&|sk, _| blob_into(sk, &up2), shoe_a, false);
    }
    jacket(&mut br, f, &j, pal);
    // The microphone's body up to the gold ring, the capsule, the hat.
    let hc = p(j.head);
    let hd = j.head_deg;
    let h = |x: f32, y: f32| add(hc, rot0((x, y), hd));
    let neck = p(j.neck);
    let l = |x: f32, y: f32| add(neck, rot0((x, y), j.torso_deg));
    br.tube(
        l(0.0, 0.004),
        h(0.0, 0.105),
        0.040,
        0.050,
        col(&br, pal.body),
        false,
    );
    capsule(&mut br, f, &h, hd, pal);
    hat(&mut br, f, &h, hd, pal);
    // Sleeves, shirt cuffs, white gloves.
    let room = br.room;
    let (jk, jk_s, shirt) = (
        col(&br, pal.jacket),
        col(&br, pal.jacket_s),
        col(&br, pal.shirt),
    );
    let (glove, glove_s) = (col(&br, GLOVE), col(&br, GLOVE_S));
    for i in 0..2 {
        let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
        let sh = add(sh, mul(sub(neck, sh), 0.15));
        let fl = length(sub(ha, el)).max(1e-6);
        let fdir = mul(sub(ha, el), 1.0 / fl);
        let wrist = sub(ha, mul(fdir, 0.014));
        let free = add(sh, mul(sub(el, sh), 0.6));
        br.bars(&[free, el, wrist], 0.064 + 2.0 * CUT, room, false);
        cel_tube(
            &mut br,
            [sh, el, wrist],
            [0.074, 0.064, 0.058],
            jk,
            jk_s,
            (-0.010, -0.009),
        );
        br.tube(
            sub(wrist, mul(fdir, 0.006)),
            add(wrist, mul(fdir, 0.010)),
            0.056,
            0.054,
            shirt,
            false,
        );
        let deg = math::atan2(fdir.1, fdir.0) * (180.0 / PI);
        br.piece(
            &|s, _| ellipse_into(s, add(ha, mul(fdir, 0.018)), 0.030, 0.033, deg),
            glove_s,
            false,
        );
        br.piece(
            &|s, _| ellipse_into(s, add(ha, mul(fdir, 0.014)), 0.027, 0.030, deg),
            glove,
            false,
        );
    }
}

/// The dinner jacket: the shirt, two halves with lapels, a pocket square, a gold bow tie.
fn jacket(br: &mut Brush, _f: &Fig, j: &Joints, pal: &Pal) {
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let t = PROPS.torso;
    let neck = place(j.neck, j);
    let l = |x: f32, y: f32| add(neck, rot0((x, y), j.torso_deg));
    let hl = t + 0.040;
    let shirt = [
        l(-0.050, 0.0),
        l(0.050, 0.0),
        l(0.050, 0.15),
        l(-0.050, 0.15),
    ];
    br.piece(&|s, _| blob_into(s, &shirt), col(br, pal.shirt), false);
    let (jk, jk_s, lapel) = (
        col(br, pal.jacket),
        col(br, pal.jacket_s),
        col(br, pal.lapel),
    );
    for side in [-1.0f32, 1.0] {
        let half = [
            l(side * 0.030, -0.004),
            l(side * 0.092, 0.006),
            l(side * 0.116, 0.048),
            l(side * 0.110, 0.140),
            l(side * 0.098, 0.230),
            l(side * 0.104, hl),
            l(side * 0.010, hl + 0.012),
            l(side * 0.004, 0.180),
            l(side * 0.026, 0.070),
        ];
        cel_blob(br, &half, jk, jk_s, (-0.014, -0.010));
        let lap = [
            l(side * 0.030, -0.002),
            l(side * 0.060, 0.030),
            l(side * 0.040, 0.050),
            l(side * 0.008, 0.172),
        ];
        br.piece(&|s, _| polygon_into(s, &lap), lapel, false);
    }
    let verm = br.fixed(br.ink.verm);
    let square = [l(-0.088, 0.075), l(-0.058, 0.075), l(-0.068, 0.096)];
    br.piece(&|s, _| polygon_into(s, &square), verm, false);
    let tie = [
        l(-0.034, 0.000),
        l(0.0, 0.014),
        l(0.034, 0.000),
        l(0.034, 0.030),
        l(0.0, 0.016),
        l(-0.034, 0.030),
    ];
    let tie_c = br.hued(pal.tie);
    br.piece(&|s, _| blob_into(s, &tie), tie_c, false);
}

/// The capsule: a pill of grille over a gold ring; fine bars; eyes on the grille.
fn capsule(br: &mut Brush, f: &Fig, h: &dyn Fn(f32, f32) -> P, hd: f32, pal: &Pal) {
    let col = |br: &Brush, c: [u8; 4]| br.fixed(c);
    let (w, ht) = (0.150, 0.215);
    let (g, gs) = (col(br, pal.grille), col(br, pal.grille_s));
    cel_rrect(
        br,
        h(0.0, -0.010),
        w,
        ht,
        w / 2.0,
        hd,
        g,
        gs,
        (-0.012, -0.008),
        true,
    );
    let bar = col(br, pal.bar);
    br.piece(
        &|s, _| {
            for n in -7i32..=7 {
                let y = n as f32 * 0.0125;
                let over = (y.abs() - (ht / 2.0 - w / 2.0)).max(0.0);
                let r = w / 2.0;
                let half = (w / 2.0 - 0.012) * (1.0 - over * over / (r * r)).max(0.0).sqrt();
                if half > 0.012 {
                    bars_into(s, &[h(-half, -0.010 + y), h(half, -0.010 + y)], 0.0045, 0.0);
                }
            }
        },
        bar,
        false,
    );
    let (ring, ring_s) = (br.hued(RING), br.hued(RING_S));
    cel_rrect(
        br,
        h(0.0, 0.100),
        0.136,
        0.030,
        0.012,
        hd,
        ring,
        ring_s,
        (-0.006, -0.004),
        false,
    );
    let eye = col(br, pal.eye);
    let e = f.eyes;
    let dark = col(br, SHOE_B);
    if e == Eyes::Cool {
        br.piece(
            &|s, _| {
                for side in [-1.0f32, 1.0] {
                    rrect_into(s, h(side * 0.032, 0.006), 0.050, 0.026, 0.010, hd, 0.0);
                }
                bars_into(s, &[h(-0.010, 0.002), h(0.010, 0.002)], 0.006, 0.0);
            },
            dark,
            false,
        );
        let gold = br.hued(br.ink.gold);
        br.bars(&[h(-0.050, -0.002), h(-0.040, 0.010)], 0.004, gold, false);
        return;
    }
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                let x = side * 0.032;
                match e {
                    Eyes::Happy => {
                        let pts = [h(x - 0.014, 0.014), h(x, 0.000), h(x + 0.014, 0.014)];
                        bars_into(s, &pts, 0.010, 0.0);
                    }
                    Eyes::Closed | Eyes::Sleepy => {
                        let pts = [h(x - 0.014, 0.006), h(x, 0.014), h(x + 0.014, 0.006)];
                        bars_into(s, &pts, 0.009, 0.0);
                    }
                    Eyes::Wink if side > 0.0 => {
                        bars_into(s, &[h(x - 0.014, 0.008), h(x + 0.014, 0.008)], 0.009, 0.0);
                    }
                    _ => {
                        let big = if e == Eyes::Wide { 1.3 } else { 1.0 };
                        ellipse_into(s, h(x, 0.008), 0.012 * big, 0.017 * big, hd);
                    }
                }
            }
        },
        eye,
        false,
    );
}

/// A pork-pie hat with a gold band; tipped (lifted and turned) by the signature.
fn hat(br: &mut Brush, f: &Fig, h: &dyn Fn(f32, f32) -> P, hd: f32, pal: &Pal) {
    let k = f.sig;
    let piv = h(0.110, -0.112);
    let tr = |x: f32, y: f32| rot(add(h(x, y), (0.0, -0.06 * k)), -22.0 * k, piv);
    let deg = hd - 22.0 * k;
    let (rim, hat) = (br.fixed(pal.hat_rim), br.fixed(HAT));
    for (x, y, w, ht, r) in [
        (0.0, -0.118, 0.236, 0.026, 0.013),
        (0.0, -0.150, 0.150, 0.060, 0.012),
    ] {
        let c = tr(x, y);
        br.rrect(c, w + 0.010, ht + 0.010, r + 0.004, deg, rim, true);
        br.rrect(c, w, ht, r, deg, hat, false);
    }
    let gold = br.hued(br.ink.gold);
    br.rrect(tr(0.0, -0.130), 0.150, 0.014, 0.004, deg, gold, false);
}
