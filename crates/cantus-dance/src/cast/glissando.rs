//! **Glissando**, the figure of light (`builtin:glissando`, round 40, ADR 0122;
//! `docs/round40/design/dancer/glissando.md`): neon-futuristic, our own. A near-black suit;
//! **lapis neon** (a periwinkle lapis, never cyan) along the outer edge of each limb, rings
//! of light at the elbows and knees, lines at the collar and the belt; the logo's five
//! vermilion bars on the chest lit by the level; a sleek dark head with two gold glowing eyes
//! and gold headphones. At night each line is a halo, a line and a pale core; on paper the
//! halo goes and the line stays. Fast hands and feet leave **light trails**. It tuts and
//! glides; its signature on the drop, **Halo**: its hands draw a circle of light round
//! itself, which drops to the floor as a ring on the beat while it lands in a crouch.
//!
//! **The glow is batched layers, never a blur** (the note's rule): every neon line of a
//! colour and a depth goes into one path a layer — the translucent halo, the line, the core
//! — so a colour costs three fills however many lines it has.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance;
use crate::rig::draw::{alpha, blob_into, ribbon_into, Sink};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{bars_into, capsule_into, circle_into, rrect_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Joints, Props};
use cantus_core::inst::dancer::Move;
use cantus_dsp::math;
use std::f32::consts::PI;

pub const ID: &str = "builtin:glissando";

pub const PROPS: Props = Props {
    leg: 0.49,
    thigh: 0.256,
    shin: 0.244,
    torso: 0.27,
    neck: 0.040,
    head: 0.080,
    shoulder: 0.094,
    hipw: 0.050,
    stance: 0.066,
    upper: 0.160,
    fore: 0.155,
    foot: 0.10,
};

pub const STYLE: Style = Style {
    bounce: 0.06,
    sway: 0.07,
    step: 0.18,
    arms: 1.0,
    lag: 0.08,
    hop: 0.12,
    squash: 0.04,
    snap: 0.7,
    lean: 1.0,
    open: (0.50, 0.84),
    chest: -0.05,
    lift_step: 0.03,
};

/// The Halo's centre (from the neck) and radius, units.
const HALO: (f32, f32, f32) = (0.0, -0.07, 0.25);
/// The light trails: how many earlier moments, how far apart (beats).
const TRAIL_N: usize = 6;
const TRAIL_STEP: f32 = 0.035;

/// Tutting: geometric arm positions by angle and bend.
const TUT: [[(f32, f32); 2]; 4] = [
    [(90.0, 90.0), (90.0, 90.0)],
    [(90.0, -90.0), (30.0, 120.0)],
    [(150.0, 0.0), (90.0, -90.0)],
    [(30.0, 120.0), (90.0, -90.0)],
];

fn lerp_fk(a: [(f32, f32); 2], b: [(f32, f32); 2], t: f32) -> [(f32, f32); 2] {
    std::array::from_fn(|i| (lerp(a[i].0, b[i].0, t), lerp(a[i].1, b[i].1, t)))
}

/// Two beats of tutting: a geometric arm shape on each half beat, snapped with an overshoot;
/// the knees bounce on the beat; the joints flash on the beat.
pub fn tut(b: f32, st: &Style) -> Fig {
    let bs = snap_time(b, 0.5, 0.30, st.snap, 2.0);
    let mut p = Fig::default();
    let ph = pmod(b, 1.0);
    let k0 = ((bs / 0.5).floor() as i64).rem_euclid(4) as usize;
    let t = bs / 0.5 - (bs / 0.5).floor();
    p.fk = Some(lerp_fk(TUT[k0], TUT[(k0 + 1) % 4], t));
    p.hip = (0.05 * cos1(b / 2.0), st.bounce * beat_down(ph, 2.0));
    p.chest = (st.chest * cos1(b / 2.0), 0.0);
    p.lean = 3.0 * cos1(b / 2.0);
    p.tilt = 8.0 * cos1((b - st.lag) / 2.0);
    p.nod = 8.0 * beat_down(pmod(b - st.lag, 1.0), 2.0) - 3.0;
    p.accent = beat_down(ph, 6.0);
    p.level = 0.5 + 0.45 * beat_down(ph, 3.0);
    p.eyes = Eyes::Open;
    p
}

/// Four beats: the step-and-touch as a glide, the arms swapping high and low on the beat.
pub fn glide(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    let bs = snap_time(b, 0.5, 0.3, 0.7, 1.9);
    let keys = [[(40.0, 0.0), (140.0, 0.0)], [(140.0, 0.0), (40.0, 0.0)]];
    let k = (bs.floor() as i64).rem_euclid(2) as usize;
    p.fk = Some(lerp_fk(keys[k], keys[(k + 1) % 2], smooth(pmod(bs, 1.0))));
    p.accent = beat_down(pmod(b, 1.0), 6.0);
    p.level = 0.5 + 0.4 * beat_down(pmod(b, 1.0), 3.0);
    p.eyes = Eyes::Open;
    p
}

/// Two beats, its signature: the hands sweep up round both sides, drawing a circle of light
/// that closes overhead on beat 2; on that beat the circle drops to the floor as a ring and
/// fades, the figure lands in a crouch, one hand to the floor.
pub fn halo(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    let draw = ease_in_out(clamp01(bb));
    let after = clamp01(bb - 1.0);
    p.sig = if bb < 1.0 { draw } else { 1.0 + after };
    let (sa, ca) = math::sin_cos(PI * draw);
    let a = PROPS.arm();
    let (cx, cy, r) = HALO;
    let hl = ((cx - r * sa) / a, (cy + r * ca) / a);
    let hr = ((cx + r * sa) / a, (cy + r * ca) / a);
    if bb < 1.0 {
        p.hands = [hl, hr];
        p.lean = 0.0;
        p.nod = -10.0 * draw;
        p.hip = (0.0, 0.02 - 0.03 * draw);
        p.eyes = Eyes::Open;
    } else {
        let land = ease_out_back(clamp01(after / 0.25), 1.6);
        let rise = smooth((after - 0.6) / 0.4);
        let k = land * (1.0 - rise);
        p.hands = [
            lerp_p((-0.10, -1.0), (-0.40, 1.25), k),
            lerp_p((0.10, -1.0), (0.85, 0.20), k),
        ];
        p.hip = (0.0, 0.26 * k);
        p.feet = [(-0.10 * k, 0.0), (0.16 * k, -0.02)];
        p.lean = -6.0 * k;
        p.nod = 14.0 * k - 6.0 * rise;
        p.eyes = if after < 0.3 { Eyes::Wide } else { Eyes::Open };
    }
    p.accent = if (1.0..1.15).contains(&bb) {
        1.0
    } else {
        beat_down(pmod(b, 1.0), 6.0) * 0.6
    };
    p.level = if bb >= 1.0 { 1.0 } else { 0.4 + 0.6 * draw };
    p
}

mod pal {
    use crate::render::rgb;
    pub const SUIT: [u8; 4] = rgb(0x0B0E12);
    pub const RIM: [u8; 4] = rgb(0x2B3239);
    pub const LINE_NIGHT: [u8; 4] = rgb(0x8FAAF5);
    pub const LINE_PAPER: [u8; 4] = rgb(0x5B79D6);
    pub const CORE: [u8; 4] = rgb(0xE6EEFF);
    pub const HALO: [u8; 4] = rgb(0x6F8FE8);
    pub const GOLD_NIGHT: [u8; 4] = rgb(0xF2C766);
    pub const GOLD_PAPER: [u8; 4] = rgb(0x9C711C);
    pub const GOLD_CORE: [u8; 4] = rgb(0xFFF4D6);
    pub const GOLD_HALO: [u8; 4] = rgb(0xE2B85A);
    pub const EYE: [u8; 4] = rgb(0xF2C766);
    pub const BARS: [u8; 4] = rgb(0xF0784A);
    pub const BARS_CORE: [u8; 4] = rgb(0xFFE1D2);
}

pub struct Glissando {
    info: CharacterInfo,
    family: dance::Style,
}

impl Glissando {
    pub fn new() -> Glissando {
        Glissando {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Glissando".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            family: dance::Style {
                bounce: 0.05,
                squash: 0.04,
                arms: 1.2,
                sway: 0.07,
                lag: 0.08,
                hop: 0.18,
                stride: 0.15,
            },
        }
    }
}

impl Default for Glissando {
    fn default() -> Self {
        Glissando::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Glissando> {
    slate::Slate(Glissando::new())
}

impl Figure for Glissando {
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
            Move::Bounce => tut(b, &STYLE),
            Move::StepLeft | Move::StepRight => glide(b, &STYLE),
            Move::Jump => halo(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin, the Halo's ring at its widest on
    /// the floor; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.62,
            top: -1.25,
            right: 0.62,
            bottom: 0.08,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        let mut past = [None; TRAIL_N];
        if pose.playing {
            for (k, o) in past.iter_mut().enumerate() {
                let fb = slate::fig_before(self, pose, (k + 1) as f32 * TRAIL_STEP);
                *o = Some(solve(&fb, &PROPS));
            }
        }
        draw(f, &past, pose, c);
    }
}

/// A line of light to draw: its points, width, whether smooth, and how strong (0…1, drawn
/// as width: the layers are opaque fills but the halo).
type Visit<'a> = dyn FnMut(&[P], f32, bool, f32) + 'a;

/// The polyline along a tube's outer edge (`side` −1/+1: away from the body).
fn edge(pts: [P; 3], widths: [f32; 3], side: f32) -> [P; 3] {
    std::array::from_fn(|i| {
        let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(2)]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let ln = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (mut nx, mut ny) = (-dy / ln, dx / ln);
        if nx * side < 0.0 {
            (nx, ny) = (-nx, -ny);
        }
        let h = widths[i] / 2.0 - 0.009;
        (pts[i].0 + nx * h, pts[i].1 + ny * h)
    })
}

/// A short arc of light on a joint's outer side.
fn joint(c: P, r: f32, side: f32) -> [P; 7] {
    let a0 = if side > 0.0 { 0.0 } else { 180.0 };
    std::array::from_fn(|k| {
        let (s, co) = math::sin_cos(crate::rig::rad(a0 - 60.0 + 20.0 * k as f32));
        add(c, (r * co, r * s))
    })
}

/// Where everything is, for the light lines.
struct Rig<'a> {
    f: &'a Fig,
    j: &'a Joints,
    past: &'a [Option<Joints>; TRAIL_N],
}

impl Rig<'_> {
    fn p(&self, q: P) -> P {
        place(q, self.j)
    }

    fn legs(&self, i: usize) -> ([P; 3], [f32; 3]) {
        let (hip, knee, foot) = (
            self.p(self.j.hips[i]),
            self.p(self.j.knees[i]),
            self.p(self.j.feet[i]),
        );
        let pel = self.p(self.j.pelvis);
        let top = add(add(hip, mul(sub(pel, hip), 0.3)), (0.0, -0.03));
        ([top, knee, add(foot, (0.0, -0.040))], [0.078, 0.064, 0.058])
    }

    fn arms(&self, i: usize) -> ([P; 3], [f32; 3]) {
        let nk = self.p(self.j.neck);
        let (sh, el, ha) = (
            self.p(self.j.shoulders[i]),
            self.p(self.j.elbows[i]),
            self.p(self.j.hands[i]),
        );
        (
            [add(sh, mul(sub(nk, sh), 0.12)), el, ha],
            [0.068, 0.056, 0.048],
        )
    }

    fn l(&self, x: f32, y: f32) -> P {
        add(self.p(self.j.neck), rot0((x, y), self.j.torso_deg))
    }

    fn h(&self, x: f32, y: f32) -> P {
        add(self.p(self.j.head), rot0((x, y), self.j.head_deg))
    }

    /// Behind the suit: the Halo being drawn or dropping, the light trails.
    fn back(&self, gx: f32, v: &mut Visit) {
        let f = self.f;
        let nk = self.p(self.j.neck);
        let (cx, c0, r) = HALO;
        if 0.0 < f.sig && f.sig <= 1.0 {
            let ang = 180.0 * f.sig;
            for side in [-1.0f32, 1.0] {
                let pts: [P; 16] = std::array::from_fn(|i| {
                    let (s, c) = math::sin_cos(crate::rig::rad(ang * i as f32 / 15.0));
                    add(nk, (cx + side * r * s, c0 + r * c))
                });
                v(&pts, 0.012, true, 1.0);
            }
        } else if f.sig > 1.0 {
            let t = f.sig - 1.0;
            let cy = lerp(nk.1 + c0, -0.01, ease_out(clamp01(t / 0.35)));
            let rx = r * (1.0 + 0.9 * t);
            let ry = lerp(r, rx * 0.14, ease_out(clamp01(t / 0.35)));
            let pts: [P; 16] = std::array::from_fn(|i| {
                let (s, c) = math::sin_cos(crate::rig::rad(24.0 * i as f32));
                (gx + rx * c, cy + ry * s)
            });
            v(&pts, 0.012 * (1.0 - t * 0.6), true, 1.0 - t);
        }
        // Light trails: where the hands and feet just were.
        if self.past[0].is_none() {
            return;
        }
        for i in 0..2 {
            for hand in [true, false] {
                let at = |j: &Joints| place(if hand { j.hands[i] } else { j.feet[i] }, j);
                let mut path = [at(self.j); TRAIL_N + 1];
                for (k, pj) in self.past.iter().enumerate() {
                    if let Some(pj) = pj {
                        path[k + 1] = at(pj);
                    }
                }
                let travel: f32 = path.windows(2).map(|w| length(sub(w[0], w[1]))).sum();
                if travel < 0.08 {
                    continue;
                }
                let n = path.len() as f32;
                for (k, w) in path.windows(2).enumerate() {
                    let o = 0.85 * (1.0 - k as f32 / (n - 1.0));
                    v(w, 0.014 * (1.0 - k as f32 / n), false, o);
                }
            }
        }
    }

    /// On the suit: leg edges and knee rings, shoe lines, torso edges, collar, belt, an ear.
    fn body(&self, v: &mut Visit) {
        let acc = 0.6 + 0.4 * self.f.accent;
        for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let (pts, ws) = self.legs(i);
            v(&edge(pts, ws, side), 0.009, false, 1.0);
            v(&joint(pts[1], 0.030, side), 0.007, true, acc);
            let foot = self.p(self.j.feet[i]);
            let ang = -side * self.j.toes[i] * 0.6;
            let base = add(foot, (side * 0.022, -0.016));
            let a = add(base, rot0((-0.050 * side, -0.002), ang));
            let z = add(base, rot0((0.062 * side, -0.002), ang));
            v(&[a, z], 0.008, false, 1.0);
        }
        let t = PROPS.torso;
        for s in [-1.0f32, 1.0] {
            let pts = [
                self.l(s * 0.100, 0.022),
                self.l(s * 0.112, 0.060),
                self.l(s * 0.084, 0.170),
                self.l(s * 0.078, t),
            ];
            v(&pts, 0.009, true, 1.0);
        }
        v(
            &[
                self.l(-0.044, 0.004),
                self.l(0.0, 0.030),
                self.l(0.044, 0.004),
            ],
            0.008,
            true,
            1.0,
        );
        v(
            &[self.l(-0.084, t + 0.006), self.l(0.084, t + 0.006)],
            0.008,
            false,
            1.0,
        );
        v(
            &[
                self.h(-0.056, 0.040),
                self.h(-0.066, -0.006),
                self.h(-0.058, -0.052),
            ],
            0.006,
            true,
            0.8,
        );
    }

    /// On the arms: their edges and elbow rings.
    fn arms_light(&self, v: &mut Visit) {
        let acc = 0.6 + 0.4 * self.f.accent;
        for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let (pts, ws) = self.arms(i);
            v(&edge(pts, ws, side), 0.008, false, 1.0);
            v(&joint(pts[1], 0.027, side), 0.007, true, acc);
        }
    }

    /// The five vermilion bars on the chest, lit by the level.
    fn bars(&self, v: &mut Visit) {
        for (n, x) in [-0.036f32, -0.018, 0.0, 0.018, 0.036]
            .into_iter()
            .enumerate()
        {
            let shape = [0.45, 0.75, 1.0, 0.75, 0.45][n];
            let hgt = 0.050 * shape * (0.35 + 0.65 * self.f.level);
            v(
                &[self.l(x, 0.120 - hgt / 2.0), self.l(x, 0.120 + hgt / 2.0)],
                0.008,
                false,
                1.0,
            );
        }
    }

    /// The gold headphone band.
    fn band(&self, v: &mut Visit) {
        let pts: [P; 8] = std::array::from_fn(|k| {
            let (s, c) = math::sin_cos(crate::rig::rad(200.0 + 20.0 * k as f32));
            self.h(0.074 * c, -0.008 + 0.090 * s)
        });
        v(&pts, 0.007, true, 1.0);
    }

    /// The gold eyes.
    fn eyes(&self, v: &mut Visit) {
        let e = self.f.eyes;
        for side in [-1.0f32, 1.0] {
            let x = side * 0.027;
            let (w, o) = match e {
                Eyes::Wide => (0.012, 1.0),
                Eyes::Sleepy => (0.006, 0.5),
                _ => (0.009, 1.0),
            };
            match e {
                Eyes::Happy => v(
                    &[
                        self.h(x - 0.014, 0.006),
                        self.h(x, -0.006),
                        self.h(x + 0.014, 0.006),
                    ],
                    w,
                    false,
                    o,
                ),
                Eyes::Closed | Eyes::Sleepy => v(
                    &[self.h(x - 0.014, 0.004), self.h(x + 0.014, 0.004)],
                    w,
                    false,
                    o,
                ),
                Eyes::Wink if side > 0.0 => v(
                    &[self.h(x - 0.014, 0.002), self.h(x + 0.014, 0.002)],
                    w,
                    false,
                    o,
                ),
                Eyes::Wide => v(&[self.h(x, -0.004), self.h(x, 0.008)], w, false, o),
                Eyes::Cool => {
                    if side < 0.0 {
                        v(&[self.h(-0.048, 0.0), self.h(0.048, 0.0)], w, false, o);
                    }
                }
                _ => v(
                    &[
                        self.h(x - 0.014 * side, 0.003),
                        self.h(x + 0.012 * side, -0.003),
                    ],
                    w,
                    false,
                    o,
                ),
            }
        }
    }
}

/// One neon colour: its halo (translucent, night only), its line, its core (night only).
struct Neon {
    halo: [u8; 4],
    line: [u8; 4],
    core: [u8; 4],
}

/// Draw every line a `lines` visits as three batched layers.
fn neon(br: &mut Brush, n: &Neon, lines: &dyn Fn(&mut Visit)) {
    let night = br.ink.dark;
    let layer = |br: &mut Brush, width: f32, col: tiny_skia::Color| {
        br.piece(
            &|s: &mut dyn Sink, _| {
                lines(&mut |pts: &[P], w: f32, smooth: bool, o: f32| {
                    if o < 0.05 || pts.len() < 2 {
                        return;
                    }
                    let w = w * width * (0.45 + 0.55 * o.min(1.0));
                    if smooth {
                        ribbon_into(s, pts, w);
                    } else {
                        bars_into(s, pts, w, 0.0);
                    }
                });
            },
            col,
            false,
        );
    };
    if night {
        let halo = br.fixed(alpha(n.halo, 0.24));
        layer(br, 3.2, halo);
    }
    let line = br.fixed(n.line);
    layer(br, if night { 1.0 } else { 1.15 }, line);
    if night {
        let core = br.fixed(n.core);
        layer(br, 0.38, core);
    }
}

fn draw(f: &Fig, past: &[Option<Joints>; TRAIL_N], pose: &Pose, c: &mut Canvas) {
    let j = solve(f, &PROPS);
    let rig = Rig { f, j: &j, past };
    let p = |q: P| place(q, &j);
    let mut br = Brush::new(c, 0.0);
    let night = br.ink.dark;
    let lapis = Neon {
        halo: pal::HALO,
        line: if night {
            pal::LINE_NIGHT
        } else {
            pal::LINE_PAPER
        },
        core: pal::CORE,
    };
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    let (lift, accent) = (f.lift, f.accent);
    slate::on_ground(br.c, pose, |c| {
        let mut b = Brush::new(c, 0.0);
        let k = (1.0 - lift * 3.0).max(0.0);
        b.shadow((gx, 0.008), 0.16 * k + 0.05, 0.022 * k + 0.008, 0.55);
        if night {
            // The light on the floor.
            b.blot(
                (gx, 0.006),
                0.13 * k + 0.03,
                0.014 * k + 0.004,
                pal::HALO,
                0.10 + 0.12 * accent,
            );
        }
    });
    neon(&mut br, &lapis, &|v| rig.back(gx, v));
    // The suit: legs, shoes, torso, neck, head.
    let suit = br.fixed(pal::SUIT);
    br.piece(
        &|s, _| {
            for i in 0..2 {
                let (pts, ws) = rig.legs(i);
                capsule_into(s, pts[0], pts[1], ws[0], ws[1], 0.0);
                capsule_into(s, pts[1], pts[2], ws[1], ws[2], 0.0);
            }
            let t = PROPS.torso;
            let torso = [
                rig.l(-0.040, -0.004),
                rig.l(-0.108, 0.010),
                rig.l(-0.120, 0.050),
                rig.l(-0.090, 0.170),
                rig.l(-0.084, t),
                rig.l(-0.098, t + 0.040),
                rig.l(0.098, t + 0.040),
                rig.l(0.084, t),
                rig.l(0.090, 0.170),
                rig.l(0.120, 0.050),
                rig.l(0.108, 0.010),
                rig.l(0.040, -0.004),
            ];
            blob_into(s, &torso);
            capsule_into(s, rig.l(0.0, 0.0), rig.h(0.0, 0.06), 0.040, 0.040, 0.0);
            let head = [
                rig.h(-0.064, -0.010),
                rig.h(-0.058, -0.064),
                rig.h(-0.020, -0.090),
                rig.h(0.026, -0.088),
                rig.h(0.062, -0.058),
                rig.h(0.066, 0.004),
                rig.h(0.050, 0.056),
                rig.h(0.0, 0.080),
                rig.h(-0.050, 0.056),
            ];
            blob_into(s, &head);
        },
        suit,
        false,
    );
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let foot = p(j.feet[i]);
        let ang = -side * j.toes[i] * 0.6;
        let base = add(foot, (side * 0.022, -0.016));
        let up: [P; 6] = [
            (-0.056, 0.0),
            (-0.050, -0.044),
            (-0.016, -0.060),
            (0.034, -0.050),
            (0.066, -0.018),
            (0.066, 0.0),
        ]
        .map(|(x, y)| add(base, rot0((x * side, y), ang)));
        br.piece(&|s, _| blob_into(s, &up), suit, false);
    }
    // The light on the suit.
    neon(&mut br, &lapis, &|v| rig.body(v));
    let bars = Neon {
        halo: pal::BARS,
        line: pal::BARS,
        core: pal::BARS_CORE,
    };
    neon(&mut br, &bars, &|v| rig.bars(v));
    let gold = Neon {
        halo: pal::GOLD_HALO,
        line: if night {
            pal::GOLD_NIGHT
        } else {
            pal::GOLD_PAPER
        },
        core: pal::GOLD_CORE,
    };
    neon(&mut br, &gold, &|v| rig.band(v));
    let cups = br.hued(gold.line);
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                rrect_into(s, rig.h(side * 0.068, 0.006), 0.024, 0.044, 0.010, 0.0, 0.0);
            }
        },
        cups,
        false,
    );
    let eyes = Neon {
        halo: pal::EYE,
        line: pal::EYE,
        core: pal::GOLD_CORE,
    };
    neon(&mut br, &eyes, &|v| rig.eyes(v));
    // Arms over the torso: a dark rim, the suit, the hands; then their light.
    let rim = br.fixed(pal::RIM);
    br.piece(
        &|s, _| {
            for i in 0..2 {
                let (pts, ws) = rig.arms(i);
                capsule_into(s, pts[0], pts[1], ws[0] + 0.014, ws[1] + 0.014, 0.0);
                capsule_into(s, pts[1], pts[2], ws[1] + 0.014, ws[2] + 0.014, 0.0);
            }
        },
        rim,
        false,
    );
    br.piece(
        &|s, _| {
            for i in 0..2 {
                let (pts, ws) = rig.arms(i);
                capsule_into(s, pts[0], pts[1], ws[0], ws[1], 0.0);
                capsule_into(s, pts[1], pts[2], ws[1], ws[2], 0.0);
                let fd = sub(pts[2], pts[1]);
                let fl = length(fd).max(1e-6);
                circle_into(s, add(pts[2], mul(fd, 0.026 / fl)), 0.030, 0.0);
            }
        },
        suit,
        false,
    );
    neon(&mut br, &lapis, &|v| rig.arms_light(v));
}
