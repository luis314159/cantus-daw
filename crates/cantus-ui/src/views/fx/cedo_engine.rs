//! Cedo's engine (design sheet v1): a beam pump in the manner of a Victorian engine
//! model, standing on its cast baseplate — what the ducker does, as a machine.
//!
//! - **The driver**, under the beam's left end. In Tempo and MIDI, a **cam** on the shaft
//!   of a spoked **flywheel**: the cam's profile *is* the drawn shape (its radius at each
//!   angle the shape's amount at that point of the cycle), and it turns with the shape's
//!   phase, so the roller of the follower on top of it rides the shape as the song plays
//!   (one turn a cycle in Tempo, one turn a note in MIDI). In Sidechain, a **push
//!   solenoid** fed by the key's cable: its winding glows when the key fires, its plunger
//!   lifting the rod.
//! - **The walking beam** on its A-frame, pivoted in a bearing with its oil cup; the push
//!   rod through its two guides lifts the left end, so the long right arm comes down.
//! - **The bellows** on its stand under the right arm: squeezed by the duck — the air the
//!   track is left with — open at rest.
//! - **The level gauge** beside the bellows: an enamel card in a brass frame on its post,
//!   its index on the bellows' top board, marked in the decibels the depth makes of each
//!   height (∞ for silence).
//!
//! The beam's lift is the amount ducked, so the machine shows the duck as it happens; the
//! cam's angle is the phase. Every size in units of the engine's design (470 by 500)
//! scaled to its area. Colours from `kit::skin::engines`. Original to Cantus.

use super::art::{self, c, dir, hash, lit};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::engines as pal;
use crate::kit::text::Role;
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::math::{asin, cos, sin};
use egui::{pos2, vec2, Align2, Pos2, Rect, Vec2};
use std::f32::consts::{PI, TAU};

/// The engine's design size, units, and the design's top (what lies above is empty).
pub const W: f32 = 470.0;
pub const H: f32 = 500.0;
const TOP: f32 = 90.0;
/// The baseplate's top, the shaft's centre, the beam's pivot.
const GROUND: f32 = 560.0;
const SHAFT: Pos2 = pos2(100.0, 462.0);
const PIVOT: Pos2 = pos2(230.0, 150.0);
/// The flywheel's radius; the cam's base radius and its lift; the roller's radius.
const FLY: f32 = 84.0;
const CAM_BASE: f32 = 30.0;
const CAM_LIFT: f32 = 40.0;
const ROLLER: f32 = 9.0;
/// The beam's arms.
const ARM_L: f32 = 130.0;
const ARM_R: f32 = 170.0;
/// The A-frame's feet either side of the pivot.
const SPREAD: f32 = 46.0;
/// The bellows' stand top, the rod from the beam to the top board, the bellows' centre and
/// width.
const STAND: f32 = 420.0;
const LINK: f32 = 120.0;
const BELLOWS_X: f32 = 400.0;
const BELLOWS_W: f32 = 104.0;
/// The gauge's post.
const GAUGE_X: f32 = 296.0;
/// Points round the cam's profile.
const CAM_N: usize = 120;

/// What drives the beam.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Driver {
    /// The cam on the flywheel, at the cycle's phase (0 … 1).
    Cam { phase: f32 },
    /// The push solenoid; `lit` while the key fires it.
    Solenoid { lit: f32 },
}

/// The engine's state for one frame.
pub struct Pose<'a> {
    pub driver: Driver,
    /// The beam's lift, 0 … 1 (the amount ducked).
    pub lift: f32,
    /// The shape's amount at a point of the cycle (the cam's profile).
    pub shape: &'a dyn Fn(f32) -> f32,
    /// The depth in dB (≥ 80 is silence) and the mix: the gauge's marks.
    pub depth: f32,
    pub mix: f32,
}

/// The engine's frame of reference: units to points.
#[derive(Copy, Clone)]
struct Map {
    o: Pos2,
    e: f32,
}

impl Map {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.o + vec2(x, y - TOP) * self.e
    }
    fn q(&self, p: Pos2) -> Pos2 {
        self.p(p.x, p.y)
    }
    fn d(&self, v: f32) -> f32 {
        v * self.e
    }
}

/// The beam's angle (radians, positive turning the left end down) for a lift.
fn beam_angle(lift: f32) -> f32 {
    let dy = (CAM_LIFT * 0.5) - CAM_LIFT * lift.clamp(0.0, 1.0);
    asin((dy / ARM_L).clamp(-1.0, 1.0))
}

/// Where the left and the right ends of the beam are, units.
fn beam_ends(lift: f32) -> (Pos2, Pos2) {
    let a = beam_angle(lift);
    let (s, co) = (sin(a), cos(a));
    (
        pos2(PIVOT.x - ARM_L * co, PIVOT.y + ARM_L * s),
        pos2(PIVOT.x + ARM_R * co, PIVOT.y - ARM_R * s),
    )
}

/// The A-frame leg's centre line at height `y`, on `side` (−1 left, 1 right).
fn leg_x(y: f32, side: f32) -> f32 {
    let top = PIVOT.y + 10.0;
    PIVOT.x + side * (10.0 + SPREAD * ((y - top) / (GROUND - top)).clamp(0.0, 1.0))
}

/// The engine in `area`.
pub fn draw(b: &mut Brush, s: &Style, area: Rect, pose: &Pose<'_>) {
    let e = (area.width() / W).min(area.height() / H);
    let o = pos2(area.center().x - W * e * 0.5, area.bottom() - H * e);
    let m = Map { o, e };
    let f = art::finish(s);
    let lift = pose.lift.clamp(0.0, 1.0);
    let (left, right) = beam_ends(lift);
    art::shadow(
        b,
        area.expand(m.d(10.0)),
        m.p(W * 0.5, GROUND + 22.0),
        vec2(m.d(W * 0.52), m.d(10.0)),
        1.0,
    );
    baseplate(b, &m, f);
    stand(b, &m, f);
    a_frame(b, &m, f);
    let top_board = right.y + LINK;
    gauge(b, s, &m, f, top_board, pose);
    bellows(b, &m, f, top_board, lift);
    match pose.driver {
        Driver::Cam { phase } => {
            flywheel(b, &m, f, phase);
            guides(b, &m, f);
            let top = cam(b, &m, f, phase, pose.shape);
            follower(b, &m, f, top, left);
        }
        Driver::Solenoid { lit } => {
            guides(b, &m, f);
            solenoid(b, &m, f, lit, left);
        }
    }
    // The link down to the bellows, then the beam over everything, then its oil cup.
    art::rod(b, m.q(right), m.p(right.x, top_board), m.d(5.0), &f.steel);
    beam(b, &m, f, lift);
    oil_cup(b, &m, f);
}

/// The cast baseplate everything is bolted to: a moulded top edge, the lining, its bolts.
fn baseplate(b: &mut Brush, m: &Map, f: &pal::Finish) {
    let r = Rect::from_min_max(m.p(4.0, GROUND), m.p(W - 4.0, GROUND + 18.0));
    b.rrect(
        r,
        m.d(3.0),
        Fill::Stops(&[
            (0.0, lit(&f.iron, 0.9)),
            (0.18, c(f.iron.mid)),
            (1.0, c(f.iron.dark)),
        ]),
    );
    b.hline(
        r.left() + m.d(3.0),
        r.right() - m.d(3.0),
        r.top() + m.d(0.8),
        m.d(1.0),
        lit(&f.iron, 1.0),
    );
    // The moulding: a step a third of the way down.
    b.hline(
        r.left() + m.d(2.0),
        r.right() - m.d(2.0),
        r.top() + m.d(6.0),
        m.d(0.8),
        with_alpha(c(pal::SHADOW), 0.6),
    );
    lining(
        b,
        m,
        f,
        Rect::from_min_max(m.p(10.0, GROUND + 9.0), m.p(W - 10.0, GROUND + 14.0)),
    );
    let mut x = 22.0;
    while x < W - 10.0 {
        art::bolt(b, m.p(x, GROUND + 11.5), m.d(2.6), hash(x as u32), &f.steel);
        x += 56.0;
    }
}

/// The lining a colourway paints along a casting (Locomotive's red, the others a fine
/// rule of their lining).
fn lining(b: &mut Brush, m: &Map, f: &pal::Finish, r: Rect) {
    let a = if f.surface == pal::Surface::Gloss {
        0.95
    } else {
        0.35
    };
    b.rrect_stroke(r, m.d(1.5), m.d(0.8), with_alpha(c(f.lining), a));
}

/// The bellows' stand: a cast table on two splayed legs and a rail.
fn stand(b: &mut Brush, m: &Map, f: &pal::Finish) {
    let (x0, x1) = (BELLOWS_X - 64.0, BELLOWS_X + 64.0);
    for (top, foot) in [(x0 + 14.0, x0 + 4.0), (x1 - 14.0, x1 - 4.0)] {
        let w = 9.0;
        let leg = [
            m.p(top - w * 0.5, STAND + 10.0),
            m.p(top + w * 0.5, STAND + 10.0),
            m.p(foot + w * 0.7, GROUND),
            m.p(foot - w * 0.7, GROUND),
        ];
        b.polygon(&leg, c(f.iron.mid));
        b.segment(leg[0], leg[3], m.d(1.2), lit(&f.iron, 0.6), false);
        b.segment(leg[1], leg[2], m.d(1.2), c(f.iron.dark), false);
        if f.surface == pal::Surface::Gloss {
            b.segment(
                art::lerp(leg[0], leg[1], 0.5),
                art::lerp(leg[3], leg[2], 0.5),
                m.d(0.7),
                c(f.lining),
                false,
            );
        }
        let pad = Rect::from_center_size(m.p(foot, GROUND - 3.0), vec2(m.d(24.0), m.d(6.0)));
        b.rrect(pad, m.d(1.5), Fill::V(lit(&f.iron, 0.5), c(f.iron.dark)));
        art::bolt(b, m.p(foot, GROUND - 6.0), m.d(2.8), 0.1, &f.steel);
    }
    // The rails between the legs, an arch under the top.
    for (y, h) in [(STAND + 80.0, 6.0), (STAND + 26.0, 4.0)] {
        let rail = Rect::from_min_max(m.p(x0 + 10.0, y), m.p(x1 - 10.0, y + h));
        b.rrect(rail, m.d(1.0), Fill::V(lit(&f.iron, 0.4), c(f.iron.dark)));
    }
    art::turned(b, m.p(BELLOWS_X, STAND + 83.0), m.d(6.0), &f.brass);
    let slab = Rect::from_min_max(m.p(x0, STAND), m.p(x1, STAND + 12.0));
    b.rrect(slab, m.d(2.0), Fill::V(lit(&f.iron, 0.55), c(f.iron.dark)));
    b.hline(
        slab.left() + m.d(2.0),
        slab.right() - m.d(2.0),
        slab.top() + m.d(1.0),
        m.d(1.0),
        lit(&f.iron, 1.0),
    );
    lining(b, m, f, slab.shrink(m.d(3.0)));
}

/// The A-frame: two tapered legs on flanged feet, two cross braces, the bearing block at
/// the top with its cap and bolts.
fn a_frame(b: &mut Brush, m: &Map, f: &pal::Finish) {
    let top = PIVOT.y + 10.0;
    for side in [-1.0f32, 1.0] {
        let (tx, fx) = (leg_x(top, side), leg_x(GROUND, side));
        let leg = [
            m.p(tx - 7.0, top),
            m.p(tx + 7.0, top),
            m.p(fx + 10.0, GROUND - 6.0),
            m.p(fx - 10.0, GROUND - 6.0),
        ];
        b.polygon(&leg, c(f.iron.mid));
        // The web down the middle, lit on one side.
        b.segment(
            art::lerp(leg[0], leg[1], 0.5),
            art::lerp(leg[3], leg[2], 0.5),
            m.d(2.4),
            lit(&f.iron, 0.75),
            false,
        );
        b.segment(leg[0], leg[3], m.d(1.0), lit(&f.iron, 0.5), false);
        b.segment(leg[1], leg[2], m.d(1.4), c(f.iron.dark), false);
        if f.surface == pal::Surface::Gloss {
            let inset = |a: Pos2, z: Pos2| art::lerp(a, z, 0.22);
            for (a, z, c0, c1) in [(0, 1, 3, 2), (1, 0, 2, 3)] {
                b.segment(
                    inset(leg[a], leg[z]),
                    inset(leg[c0], leg[c1]),
                    m.d(0.8),
                    c(f.lining),
                    false,
                );
            }
        }
        let flange = Rect::from_center_size(m.p(fx, GROUND - 4.0), vec2(m.d(40.0), m.d(8.0)));
        b.rrect(flange, m.d(2.0), Fill::V(lit(&f.iron, 0.5), c(f.iron.dark)));
        for dx in [-13.0, 13.0] {
            art::bolt(
                b,
                m.p(fx + dx, GROUND - 8.0),
                m.d(2.8),
                hash(fx as u32) * 0.2,
                &f.steel,
            );
        }
    }
    // Two cross braces, a turned boss at each middle.
    for y in [300.0, 450.0] {
        let half = leg_x(y, 1.0) - PIVOT.x;
        let brace = Rect::from_min_max(m.p(PIVOT.x - half, y - 3.5), m.p(PIVOT.x + half, y + 3.5));
        b.rrect(brace, m.d(1.0), Fill::V(lit(&f.iron, 0.5), c(f.iron.dark)));
        art::turned(b, m.p(PIVOT.x, y), m.d(7.0), &f.brass);
    }
    // The bearing block: its base, the brass bearing, the cap and its bolts.
    let base = Rect::from_center_size(m.p(PIVOT.x, PIVOT.y + 12.0), vec2(m.d(46.0), m.d(12.0)));
    b.rrect(base, m.d(2.0), Fill::V(lit(&f.iron, 0.6), c(f.iron.dark)));
    let cap = Rect::from_center_size(m.p(PIVOT.x, PIVOT.y - 1.0), vec2(m.d(30.0), m.d(16.0)));
    b.rrect(cap, m.d(5.0), Fill::V(c(f.brass.light), c(f.brass.dark)));
    b.rrect_stroke(cap, m.d(5.0), m.d(0.6), with_alpha(c(f.brass.dark), 0.9));
    for dx in [-18.0, 18.0] {
        art::bolt(
            b,
            m.p(PIVOT.x + dx, PIVOT.y + 12.0),
            m.d(3.0),
            0.3,
            &f.steel,
        );
    }
    lining(b, m, f, base.shrink(m.d(2.5)));
}

/// The oil cup on the bearing: a brass foot, a glass reservoir with its oil, a lid.
fn oil_cup(b: &mut Brush, m: &Map, f: &pal::Finish) {
    let x = PIVOT.x;
    let stem = Rect::from_min_max(m.p(x - 2.5, PIVOT.y - 30.0), m.p(x + 2.5, PIVOT.y - 8.0));
    b.rect(stem, Fill::V(c(f.brass.light), c(f.brass.dark)));
    let glass = Rect::from_min_max(m.p(x - 8.0, PIVOT.y - 52.0), m.p(x + 8.0, PIVOT.y - 32.0));
    b.rrect(
        glass,
        m.d(2.0),
        Fill::Solid(with_alpha(c(pal::GLINT), 0.16)),
    );
    let oil = Rect::from_min_max(
        pos2(glass.left(), glass.top() + glass.height() * 0.45),
        glass.max,
    );
    b.rrect(
        oil,
        m.d(2.0),
        Fill::V(
            with_alpha(c(f.brass.light), 0.75),
            with_alpha(c(f.brass.dark), 0.85),
        ),
    );
    b.vline(
        glass.left() + m.d(3.0),
        glass.top() + m.d(2.0),
        glass.bottom() - m.d(2.0),
        m.d(1.2),
        with_alpha(c(pal::GLINT), 0.6),
    );
    for (y, w) in [(PIVOT.y - 32.0, 20.0), (PIVOT.y - 52.0, 20.0)] {
        let ring = Rect::from_center_size(m.p(x, y), vec2(m.d(w), m.d(4.0)));
        b.rrect(ring, m.d(1.5), Fill::V(c(f.brass.light), c(f.brass.dark)));
    }
    let lid = Rect::from_center_size(m.p(x, PIVOT.y - 57.0), vec2(m.d(12.0), m.d(6.0)));
    b.rrect(lid, m.d(3.0), Fill::V(c(f.brass.light), c(f.brass.dark)));
}

/// The flywheel behind the cam: the rim (its machined face bright), six curved spokes, a
/// balance weight, the hub; turned to the phase. Its pedestal behind it.
fn flywheel(b: &mut Brush, m: &Map, f: &pal::Finish, phase: f32) {
    let ctr = m.q(SHAFT);
    let rot = TAU * phase;
    let ped = [
        m.p(SHAFT.x - 13.0, SHAFT.y),
        m.p(SHAFT.x + 13.0, SHAFT.y),
        m.p(SHAFT.x + 28.0, GROUND - 4.0),
        m.p(SHAFT.x - 28.0, GROUND - 4.0),
    ];
    b.polygon(&ped, mix(c(f.iron.mid), c(f.iron.dark), 0.55));
    b.segment(ped[0], ped[3], m.d(1.2), lit(&f.iron, 0.6), false);
    b.segment(ped[1], ped[2], m.d(1.2), c(f.iron.dark), false);
    // The cast web down its face.
    b.segment(
        art::lerp(ped[0], ped[1], 0.5),
        art::lerp(ped[3], ped[2], 0.5),
        m.d(3.0),
        lit(&f.iron, 0.35),
        false,
    );
    let foot = Rect::from_center_size(m.p(SHAFT.x, GROUND - 4.0), vec2(m.d(74.0), m.d(8.0)));
    b.rrect(foot, m.d(2.0), Fill::V(lit(&f.iron, 0.4), c(f.iron.dark)));
    for dx in [-29.0, 29.0] {
        art::bolt(b, m.p(SHAFT.x + dx, GROUND - 7.0), m.d(2.8), 0.6, &f.steel);
    }
    // The spokes: tapered, bowed a little along the turn.
    for i in 0..6 {
        let a = rot + TAU * i as f32 / 6.0;
        let mut l = [Pos2::ZERO; 7];
        let mut r = [Pos2::ZERO; 7];
        for j in 0..7 {
            let t = j as f32 / 6.0;
            let rad = 14.0 + (FLY - 16.0) * t;
            let bow = 0.10 * sin(PI * t);
            let w = 5.5 - 2.5 * t;
            let d = dir(a + bow);
            let side = vec2(-d.y, d.x);
            l[j] = ctr + (d * rad + side * w) * m.e;
            r[j] = ctr + (d * rad - side * w) * m.e;
        }
        b.between(&l, &r, c(f.iron.mid));
        b.polyline(&l, m.d(0.9), lit(&f.iron, 0.7));
        b.polyline(&r, m.d(0.9), c(f.iron.dark));
    }
    // The rim: its painted band, the machined face, the light round its top.
    let rim_w = 11.0;
    b.circle_stroke(ctr, m.d(FLY - rim_w * 0.5), m.d(rim_w), c(f.iron.mid));
    b.circle_stroke(ctr, m.d(FLY - 1.5), m.d(3.0), c(f.steel.mid));
    b.arc(
        ctr,
        m.d(FLY - 1.5),
        -2.9,
        -0.3,
        m.d(1.4),
        c(f.steel.light),
        false,
    );
    b.arc(
        ctr,
        m.d(FLY - rim_w + 0.5),
        0.4,
        2.7,
        m.d(1.2),
        c(f.iron.dark),
        false,
    );
    b.arc(
        ctr,
        m.d(FLY - rim_w + 0.5),
        -2.7,
        -0.4,
        m.d(0.8),
        lit(&f.iron, 0.8),
        false,
    );
    if f.surface == pal::Surface::Gloss {
        b.circle_stroke(
            ctr,
            m.d(FLY - rim_w * 0.55),
            m.d(0.9),
            with_alpha(c(f.lining), 0.9),
        );
    }
    // A balance weight between two spokes, opposite the cam's lobe.
    let wa = rot + PI + TAU / 12.0;
    let mut wl = [Pos2::ZERO; 9];
    let mut wr = [Pos2::ZERO; 9];
    for j in 0..9 {
        let a = wa + (j as f32 / 8.0 - 0.5) * 0.7;
        wl[j] = ctr + dir(a) * m.d(FLY - rim_w);
        wr[j] = ctr + dir(a) * m.d(FLY - rim_w - 9.0);
    }
    b.between(&wl, &wr, c(f.iron.dark));
    b.circle(ctr, m.d(15.0), Fill::V(lit(&f.iron, 0.6), c(f.iron.dark)));
}

/// The cam: its profile is the shape, turned by the phase so the point under the roller
/// (straight up) is the shape now. Returns the profile's radius there, units.
fn cam(b: &mut Brush, m: &Map, f: &pal::Finish, phase: f32, shape: &dyn Fn(f32) -> f32) -> f32 {
    let ctr = m.q(SHAFT);
    let mut outer = [Pos2::ZERO; CAM_N + 1];
    let mut inner = [Pos2::ZERO; CAM_N + 1];
    let mut face = [Pos2::ZERO; CAM_N + 1];
    let radius = |beta: f32| {
        let u = (phase - beta / TAU).rem_euclid(1.0);
        CAM_BASE + CAM_LIFT * shape(u).clamp(0.0, 1.0)
    };
    for i in 0..=CAM_N {
        let beta = TAU * i as f32 / CAM_N as f32;
        let r = radius(beta);
        let d = dir(beta);
        outer[i] = ctr + d * m.d(r);
        face[i] = ctr + d * m.d(r - 3.0) + vec2(-0.5, -0.7) * m.e;
        inner[i] = ctr + d * m.d(2.0);
    }
    art::shadow(
        b,
        Rect::from_center_size(ctr, Vec2::splat(m.d(170.0))),
        ctr + vec2(m.d(2.0), m.d(4.0)),
        Vec2::splat(m.d(CAM_BASE + CAM_LIFT * 0.7)),
        0.7,
    );
    b.between(&outer, &inner, c(f.cam.dark));
    b.between(&face, &inner, c(f.cam.mid));
    // The ground face catches the light at its top left; turning marks round the hub.
    b.radial(
        Rect::from_center_size(ctr, Vec2::splat(m.d(2.0 * (CAM_BASE + CAM_LIFT)))),
        ctr + vec2(-0.3, -0.35) * m.d(CAM_BASE),
        Vec2::splat(m.d(CAM_BASE)),
        &[
            (0.0, with_alpha(c(f.cam.light), 0.6)),
            (1.0, with_alpha(c(f.cam.light), 0.0)),
        ],
        8,
    );
    for i in 1..5 {
        b.circle_stroke(
            ctr,
            m.d(12.0 + 4.0 * i as f32),
            m.d(0.4),
            with_alpha(c(f.cam.dark), 0.22),
        );
    }
    b.polyline(&outer, m.d(1.0), with_alpha(c(pal::SHADOW), 0.85));
    // The index of the cycle's start (the beat): a line engraved from the hub to the edge,
    // a lining dot at its end.
    let a0 = TAU * phase;
    let tip = radius(a0 + 0.001) - 5.0;
    b.segment(
        ctr + dir(a0) * m.d(13.0),
        ctr + dir(a0) * m.d(tip),
        m.d(1.4),
        with_alpha(c(pal::SHADOW), 0.7),
        false,
    );
    b.circle(
        ctr + dir(a0) * m.d(tip + 1.5),
        m.d(1.8),
        Fill::Solid(c(f.lining)),
    );
    // The hub with its key and set screw.
    art::turned(b, ctr, m.d(12.0), &f.steel);
    let key = ctr + dir(a0 + PI) * m.d(9.0);
    b.rect(
        Rect::from_center_size(key, Vec2::splat(m.d(4.0))),
        Fill::Solid(c(f.steel.dark)),
    );
    art::bolt(b, ctr, m.d(4.0), phase, &f.steel);
    radius(0.0)
}

/// The push rod's guides: two arms from the frame's left leg, each with a bronze bush.
fn guides(b: &mut Brush, m: &Map, f: &pal::Finish) {
    for y in [236.0, 330.0] {
        let lx = leg_x(y, -1.0);
        let arm = Rect::from_min_max(m.p(SHAFT.x, y - 3.5), m.p(lx, y + 3.5));
        b.rrect(arm, m.d(2.0), Fill::V(lit(&f.iron, 0.55), c(f.iron.dark)));
        let bush = Rect::from_center_size(m.p(SHAFT.x, y), vec2(m.d(18.0), m.d(15.0)));
        b.rrect(bush, m.d(3.0), Fill::V(c(f.brass.light), c(f.brass.dark)));
        b.rrect_stroke(bush, m.d(3.0), m.d(0.6), c(f.brass.dark));
        b.hline(
            bush.left() + m.d(2.0),
            bush.right() - m.d(2.0),
            bush.top() + m.d(1.0),
            m.d(0.8),
            with_alpha(c(pal::GLINT), 0.5),
        );
        art::bolt(b, m.p(lx - 7.0, y), m.d(2.5), 0.2, &f.steel);
    }
}

/// The follower: its roller on the cam's top (`top`, units), the yoke and the push rod up
/// to the beam's left end.
fn follower(b: &mut Brush, m: &Map, f: &pal::Finish, top: f32, left: Pos2) {
    let rc = pos2(SHAFT.x, SHAFT.y - top - ROLLER);
    art::rod(b, m.q(left), m.p(rc.x, rc.y - 12.0), m.d(6.0), &f.steel);
    // The rod through its bushes: the bushes' fronts over it.
    for y in [236.0, 330.0] {
        let lip = Rect::from_center_size(m.p(SHAFT.x, y), vec2(m.d(18.0), m.d(5.0)));
        b.rrect(lip, m.d(1.5), Fill::V(c(f.brass.light), c(f.brass.mid)));
    }
    let yoke = Rect::from_min_max(m.p(rc.x - 13.0, rc.y - 16.0), m.p(rc.x + 13.0, rc.y + 1.0));
    b.rrect(yoke, m.d(2.5), Fill::V(lit(&f.iron, 0.6), c(f.iron.dark)));
    b.circle(
        m.q(rc),
        m.d(ROLLER),
        Fill::V(c(f.steel.light), c(f.steel.dark)),
    );
    b.circle_stroke(
        m.q(rc),
        m.d(ROLLER - 2.0),
        m.d(0.6),
        with_alpha(c(f.steel.dark), 0.8),
    );
    b.circle(
        m.q(rc),
        m.d(2.6),
        Fill::V(c(f.brass.light), c(f.brass.dark)),
    );
}

/// The push solenoid in Sidechain: the coil between bakelite cheeks on its base, the
/// winding warming when the key fires it, the key's cable to its terminals and the plunger
/// up to the beam.
fn solenoid(b: &mut Brush, m: &Map, f: &pal::Finish, glow: f32, left: Pos2) {
    let (x0, x1) = (SHAFT.x - 40.0, SHAFT.x + 40.0);
    let (y0, y1) = (392.0, GROUND - 14.0);
    art::rod(b, m.q(left), m.p(SHAFT.x, y0 + 8.0), m.d(7.0), &f.steel);
    for y in [236.0, 330.0] {
        let lip = Rect::from_center_size(m.p(SHAFT.x, y), vec2(m.d(18.0), m.d(5.0)));
        b.rrect(lip, m.d(1.5), Fill::V(c(f.brass.light), c(f.brass.mid)));
    }
    let base = Rect::from_min_max(m.p(x0 - 14.0, y1), m.p(x1 + 14.0, GROUND));
    b.rrect(base, m.d(2.0), Fill::V(lit(&f.iron, 0.4), c(f.iron.dark)));
    for dx in [-46.0, 46.0] {
        art::bolt(b, m.p(SHAFT.x + dx, y1 + 7.0), m.d(2.8), 0.4, &f.steel);
    }
    // The winding: copper, fine turns, warmer and brighter as it pulls.
    let coil = Rect::from_min_max(m.p(x0 + 4.0, y0 + 12.0), m.p(x1 - 4.0, y1 - 12.0));
    let warm = glow.clamp(0.0, 1.0);
    let body = mix(c(pal::COPPER.mid), c(pal::COPPER.light), 0.6 * warm);
    if warm > 0.0 {
        b.radial(
            coil.expand(m.d(40.0)),
            coil.center(),
            vec2(coil.width() * 1.1, coil.height() * 0.9),
            &[
                (0.0, with_alpha(c(pal::COPPER.light), 0.35 * warm)),
                (1.0, with_alpha(c(pal::COPPER.light), 0.0)),
            ],
            8,
        );
    }
    b.hrect(
        coil,
        &[
            (0.0, c(pal::COPPER.dark)),
            (0.22, body),
            (0.45, mix(body, c(pal::GLINT), 0.25 + 0.25 * warm)),
            (0.7, c(pal::COPPER.mid)),
            (1.0, c(pal::COPPER.dark)),
        ],
    );
    let turns = 40;
    for i in 0..turns {
        let y = coil.top() + coil.height() * (i as f32 + 0.5) / turns as f32;
        b.hline(
            coil.left(),
            coil.right(),
            y,
            m.d(0.5),
            with_alpha(c(pal::COPPER.dark), 0.5),
        );
    }
    // The paper wrap's band across the middle, its printed rule.
    let band = Rect::from_center_size(coil.center(), vec2(coil.width(), m.d(18.0)));
    b.hrect(
        band,
        &[
            (0.0, c(pal::ENAMEL_SHADE)),
            (0.4, c(pal::ENAMEL)),
            (1.0, c(pal::ENAMEL_SHADE)),
        ],
    );
    b.hline(
        band.left(),
        band.right(),
        band.center().y,
        m.d(0.8),
        with_alpha(c(pal::INDEX), 0.8),
    );
    // The cheeks.
    for y in [y0, y1 - 12.0] {
        let cheek = Rect::from_min_max(m.p(x0 - 4.0, y), m.p(x1 + 4.0, y + 12.0));
        b.rrect(
            cheek,
            m.d(2.0),
            Fill::V(c(pal::BAKELITE.light), c(pal::BAKELITE.dark)),
        );
        b.hline(
            cheek.left() + m.d(2.0),
            cheek.right() - m.d(2.0),
            cheek.top() + m.d(0.8),
            m.d(0.8),
            with_alpha(c(pal::GLINT), 0.25),
        );
    }
    // The terminals and the key's cable coming in from the left, cloth-covered.
    let term = m.p(x0 - 4.0, y0 + 26.0);
    for dy in [0.0, 14.0] {
        art::bolt(b, term + vec2(0.0, m.d(dy)), m.d(3.0), 0.0, &f.brass);
    }
    let mut cable = [Pos2::ZERO; 16];
    for (i, p) in cable.iter_mut().enumerate() {
        let t = i as f32 / 15.0;
        let x = 4.0 + (x0 - 10.0 - 4.0) * t;
        let y = (GROUND + 8.0) + ((y0 + 33.0) - (GROUND + 8.0)) * t * t;
        *p = m.p(x, y);
    }
    b.polyline(&cable, m.d(5.5), c(pal::BAKELITE.dark));
    b.polyline(&cable, m.d(2.4), with_alpha(c(pal::BAKELITE.light), 0.8));
    for (i, p) in cable.iter().enumerate().skip(1) {
        if i % 2 == 0 {
            b.circle(*p, m.d(1.0), Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)));
        }
    }
}

/// The walking beam: fish-bellied, a slot down each arm, bosses and pins at its ends and
/// on the pivot; tilted by the lift.
fn beam(b: &mut Brush, m: &Map, f: &pal::Finish, lift: f32) {
    let a = beam_angle(lift);
    let (s, co) = (sin(a), cos(a));
    // A point along the beam (x from the pivot, y down from its axis), turned.
    let at = |x: f32, y: f32| m.p(PIVOT.x + x * co + y * s, PIVOT.y - x * s + y * co);
    const N: usize = 28;
    let (xa, xb) = (-ARM_L - 12.0, ARM_R + 12.0);
    let depth = |x: f32| {
        let u = if x < 0.0 { x / xa } else { x / xb };
        6.0 + 11.0 * (1.0 - u * u)
    };
    let mut top = [Pos2::ZERO; N + 1];
    let mut bot = [Pos2::ZERO; N + 1];
    for i in 0..=N {
        let x = xa + (xb - xa) * i as f32 / N as f32;
        top[i] = at(x, -8.0);
        bot[i] = at(x, depth(x));
    }
    let drop = vec2(m.d(2.0), m.d(4.0));
    let shadow_top = top.map(|p| p + drop);
    let shadow_bot = bot.map(|p| p + drop);
    b.between(&shadow_top, &shadow_bot, with_alpha(c(pal::SHADOW), 0.35));
    b.between(&top, &bot, c(f.iron.mid));
    // The flange along the top, lit; the belly's edge in shadow.
    let mut flange = [Pos2::ZERO; N + 1];
    for (i, p) in flange.iter_mut().enumerate() {
        *p = at(xa + (xb - xa) * i as f32 / N as f32, -5.0);
    }
    b.between(&top, &flange, lit(&f.iron, 0.55));
    b.polyline(&top, m.d(1.2), lit(&f.iron, 1.0));
    b.polyline(&bot, m.d(1.4), c(f.iron.dark));
    // The slots, one down each arm, with a rib across each.
    for (x0, x1) in [(-ARM_L + 16.0, -20.0), (20.0, ARM_R - 16.0)] {
        let mut st = [Pos2::ZERO; 9];
        let mut sb = [Pos2::ZERO; 9];
        for j in 0..9 {
            let x = x0 + (x1 - x0) * j as f32 / 8.0;
            let d = depth(x);
            let mid = (d - 5.0) * 0.5;
            let half = ((d + 5.0) * 0.5 - 5.0).max(1.0);
            st[j] = at(x, mid - half * 0.5);
            sb[j] = at(x, mid + half * 0.5);
        }
        b.between(&st, &sb, c(f.iron.dark));
        b.polyline(&sb, m.d(0.8), lit(&f.iron, 0.7));
        let xm = 0.5 * (x0 + x1);
        b.segment(
            at(xm, -4.0),
            at(xm, depth(xm) - 1.0),
            m.d(4.0),
            c(f.iron.mid),
            false,
        );
        b.segment(
            at(xm - 1.5, -4.0),
            at(xm - 1.5, depth(xm) - 1.0),
            m.d(0.8),
            lit(&f.iron, 0.8),
            false,
        );
    }
    if f.surface == pal::Surface::Gloss {
        let mut line = [Pos2::ZERO; N + 1];
        for (i, p) in line.iter_mut().enumerate() {
            *p = at(xa + 6.0 + (xb - xa - 12.0) * i as f32 / N as f32, -2.5);
        }
        b.polyline(&line, m.d(0.9), with_alpha(c(f.lining), 0.95));
    }
    art::pin(b, at(-ARM_L, 0.0), m.d(9.0), &f.iron, &f.steel);
    art::pin(b, at(ARM_R, 0.0), m.d(9.0), &f.iron, &f.steel);
    art::pin(b, at(0.0, 0.0), m.d(13.0), &f.brass, &f.steel);
}

/// The bellows on the stand: leather pleats folded in and out between the top board (at
/// `top`, units) and the bottom board, rounded by the light, brass corners, a spout.
fn bellows(b: &mut Brush, m: &Map, f: &pal::Finish, top: f32, lift: f32) {
    let x = BELLOWS_X;
    let hw = BELLOWS_W * 0.5;
    let bottom = STAND - 10.0;
    let y0 = top + 10.0;
    let folds = 7;
    let h = (bottom - y0).max(4.0);
    // Squeezed, the folds stand out further.
    let inset = 9.0 + 4.0 * lift;
    let bands = folds * 2;
    let ys = |i: usize| y0 + h * i as f32 / bands as f32;
    let side = |i: usize| if i.is_multiple_of(2) { inset } else { 0.0 };
    // The pleats: a band from a seam (in) down to a crease (out) faces up and is lit; from
    // a crease down to a seam faces down and is dark. Each in three slices, its sides
    // turning away from the light.
    for i in 0..bands {
        let (ya, yb) = (ys(i), ys(i + 1));
        let (ia, ib) = (side(i), side(i + 1));
        let up = i % 2 == 0;
        let base = if up {
            lit(&f.leather, 0.45)
        } else {
            mix(c(f.leather.mid), c(f.leather.dark), 0.55)
        };
        let edge = mix(base, c(pal::SHADOW), 0.35);
        let slices = [
            (0.0f32, 0.16f32, edge),
            (0.16, 0.84, base),
            (0.84, 1.0, mix(base, c(pal::SHADOW), 0.5)),
        ];
        for (u0, u1, col) in slices {
            let xa0 = x - hw + ia + (BELLOWS_W - 2.0 * ia) * u0;
            let xa1 = x - hw + ia + (BELLOWS_W - 2.0 * ia) * u1;
            let xb0 = x - hw + ib + (BELLOWS_W - 2.0 * ib) * u0;
            let xb1 = x - hw + ib + (BELLOWS_W - 2.0 * ib) * u1;
            b.polygon(
                &[m.p(xa0, ya), m.p(xa1, ya), m.p(xb1, yb), m.p(xb0, yb)],
                col,
            );
        }
        // The crease's ridge catches the light; the seam is a dark line.
        let (l, r) = (m.p(x - hw + ia, ya), m.p(x + hw - ia, ya));
        if i % 2 == 1 {
            b.segment(l, r, m.d(0.9), lit(&f.leather, 1.0), false);
        } else if i > 0 {
            b.segment(l, r, m.d(1.0), with_alpha(c(pal::SHADOW), 0.7), false);
        }
    }
    // The zigzag outline down each side.
    let mut ls = [Pos2::ZERO; 15];
    let mut rs = [Pos2::ZERO; 15];
    for i in 0..=bands {
        ls[i] = m.p(x - hw + side(i), ys(i));
        rs[i] = m.p(x + hw - side(i), ys(i));
    }
    b.polyline(&ls[..=bands], m.d(1.0), with_alpha(c(pal::SHADOW), 0.8));
    b.polyline(&rs[..=bands], m.d(1.0), with_alpha(c(pal::SHADOW), 0.8));
    // The boards with their brass corners and screws.
    for (yt, yb) in [(top, top + 10.0), (bottom, bottom + 10.0)] {
        let r = Rect::from_min_max(m.p(x - hw - 6.0, yt), m.p(x + hw + 6.0, yb));
        b.rrect(r, m.d(1.5), Fill::V(c(f.boards.light), c(f.boards.dark)));
        for j in 0..3 {
            let gy = r.top() + r.height() * (0.3 + 0.2 * j as f32);
            b.hline(
                r.left() + m.d(14.0),
                r.right() - m.d(14.0),
                gy,
                m.d(0.4),
                with_alpha(c(f.boards.dark), 0.5),
            );
        }
        for cx in [r.left(), r.right() - m.d(12.0)] {
            let corner = Rect::from_min_size(pos2(cx, r.top()), vec2(m.d(12.0), r.height()));
            b.rect(corner, Fill::V(c(f.brass.light), c(f.brass.dark)));
            b.circle(corner.center(), m.d(1.4), Fill::Solid(c(f.brass.dark)));
        }
    }
    // The spout on the bottom board, and the eye on the top board the link holds.
    let spout = Rect::from_min_max(
        m.p(x + hw + 6.0, bottom + 2.0),
        m.p(x + hw + 18.0, bottom + 8.0),
    );
    b.rrect(spout, m.d(1.5), Fill::V(c(f.brass.light), c(f.brass.dark)));
    b.circle(
        m.p(x + hw + 18.0, bottom + 5.0),
        m.d(2.0),
        Fill::Solid(c(pal::SHADOW)),
    );
    art::pin(b, m.p(x, top - 2.0), m.d(6.0), &f.brass, &f.steel);
}

/// The level gauge: an enamel card in a brass frame on its post beside the bellows, marked
/// in the dB the depth makes of each height of the top board; the index rides the board.
fn gauge(b: &mut Brush, s: &Style, m: &Map, f: &pal::Finish, top: f32, pose: &Pose<'_>) {
    let (_, open) = beam_ends(0.0);
    let (_, shut) = beam_ends(1.0);
    let (y_open, y_shut) = (open.y + LINK + 5.0, shut.y + LINK + 5.0);
    let x = GAUGE_X;
    let card = Rect::from_min_max(m.p(x - 16.0, y_open - 14.0), m.p(x + 16.0, y_shut + 14.0));
    // The post down to the baseplate, its foot bolted.
    let post = Rect::from_min_max(
        pos2(card.center().x - m.d(2.5), card.bottom()),
        m.p(x + 2.5, GROUND),
    );
    b.rect(post, Fill::V(lit(&f.iron, 0.5), c(f.iron.dark)));
    let foot = Rect::from_center_size(m.p(x, GROUND - 3.0), vec2(m.d(18.0), m.d(6.0)));
    b.rrect(foot, m.d(1.5), Fill::V(lit(&f.iron, 0.5), c(f.iron.dark)));
    b.rrect(
        card.expand(m.d(3.0)),
        m.d(4.0),
        Fill::V(c(f.brass.light), c(f.brass.dark)),
    );
    b.rrect(
        card,
        m.d(2.0),
        Fill::V(c(pal::ENAMEL), c(pal::ENAMEL_SHADE)),
    );
    for y in [card.top() - m.d(1.0), card.bottom() + m.d(1.0)] {
        art::rivet(b, pos2(card.center().x, y), m.d(1.8), &f.brass);
    }
    // The marks: decibels at a readable step down to the depth.
    let depth = pose.depth.max(0.0);
    let mix = pose.mix.clamp(0.0, 1.0);
    let inf = depth >= cantus_dsp::cedo::DEPTH_INF;
    let span = if inf {
        1.0
    } else {
        1.0 - cantus_dsp::math::db_to_gain(-depth)
    } * mix;
    let y_of = |db: f32| {
        let a = if span > 1e-6 {
            ((1.0 - cantus_dsp::math::db_to_gain(-db)) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        m.p(0.0, y_open + (y_shut - y_open) * a).y
    };
    let ink = c(pal::FIGURES);
    let shown = if inf { 36.0 } else { depth * mix };
    let step = match shown {
        d if d <= 4.0 => 1.0,
        d if d <= 9.0 => 3.0,
        d if d <= 24.0 => 6.0,
        _ => 12.0,
    };
    let yb = m.p(0.0, y_shut).y;
    let mut d = 0.0f32;
    while d < shown - 0.4 * step {
        let y = y_of(d);
        if yb - y > m.d(8.0) {
            b.hline(
                card.left() + m.d(2.0),
                card.left() + m.d(9.0),
                y,
                m.d(0.8),
                ink,
            );
            figure(
                b,
                s,
                pos2(card.right() - m.d(3.0), y),
                ink,
                format_args!("{d:.0}"),
            );
        }
        d += step;
    }
    // The bottom of the travel: the depth (∞ for silence), in red.
    let red = c(pal::INDEX);
    b.hline(
        card.left() + m.d(2.0),
        card.left() + m.d(11.0),
        yb,
        m.d(1.2),
        red,
    );
    if inf {
        figure(
            b,
            s,
            pos2(card.right() - m.d(3.0), yb),
            red,
            format_args!("\u{221e}"),
        );
    } else {
        figure(
            b,
            s,
            pos2(card.right() - m.d(3.0), yb),
            red,
            format_args!("{:.0}", depth * mix),
        );
    }
    // The index: an arm from the top board's edge to the card, its tip a red point.
    let ty = m.p(0.0, top + 5.0).y;
    let tip = pos2(card.left() + m.d(5.0), ty);
    b.segment(
        m.p(BELLOWS_X - BELLOWS_W * 0.5 - 6.0, top + 5.0),
        pos2(card.right(), ty),
        m.d(2.0),
        c(f.brass.dark),
        false,
    );
    b.segment(
        pos2(card.right(), ty),
        tip + vec2(m.d(6.0), 0.0),
        m.d(1.0),
        red,
        false,
    );
    b.polygon(
        &[
            tip,
            tip + vec2(m.d(7.0), -m.d(3.5)),
            tip + vec2(m.d(7.0), m.d(3.5)),
        ],
        red,
    );
}

/// A gauge's figure, right-aligned at `at`.
fn figure(b: &mut Brush, s: &Style, at: Pos2, ink: egui::Color32, args: std::fmt::Arguments<'_>) {
    super::super::canvas::putf(
        b,
        at,
        Align2::RIGHT_CENTER,
        Role::Micro,
        ink,
        s.k,
        None,
        args,
    );
}
