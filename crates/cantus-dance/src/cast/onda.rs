//! **Onda**, the oscilloscope (`builtin:onda`, round 40, ADR 0122; the owner's O1,
//! `docs/round40/design/dancer/onda.md`): a figure drawn by a scope's phosphor trace, with
//! no body at all. Every limb is a line rippling with the waveform (its amplitude the
//! level); the head is a **Lissajous** loop turning in depth with the beat, two bright dwell
//! points for eyes. Amber phosphor — the cast's gold — with a halo at night, a sharp
//! dark-amber line on paper; a bright **beam dot** runs along the trace, and a faint
//! **afterglow** of where it was a moment ago stays behind at night. Its signature on the
//! drop, **Square**: the ripples snap into a square wave and its poses go angular (pop and
//! lock), then a saw, and it melts back into a sine.
//!
//! **Drawing the trace cheaply** (the note's plan): the samples (one every 0.008 units,
//! about 400 a frame with the head) are generated into arrays on the stack, never the heap;
//! every segment is a quad wound the rig's way, extended by half its width at both ends so
//! the joins close; all of a layer goes into one path — the halo (translucent, night only),
//! the line, the core — three fills for the whole figure, never a blur.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::draw::{alpha, Sink};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{circle_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Joints, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use std::f32::consts::PI;
use tiny_skia::Color;

pub const ID: &str = "builtin:onda";

pub const PROPS: Props = Props {
    leg: 0.47,
    thigh: 0.245,
    shin: 0.235,
    torso: 0.28,
    neck: 0.04,
    head: 0.085,
    shoulder: 0.085,
    hipw: 0.045,
    stance: 0.065,
    upper: 0.155,
    fore: 0.150,
    foot: 0.10,
};

pub const STYLE: Style = Style {
    bounce: 0.07,
    sway: 0.07,
    step: 0.17,
    arms: 1.0,
    lag: 0.10,
    hop: 0.14,
    squash: 0.05,
    snap: 0.0,
    lean: 1.0,
    open: (0.62, 0.88),
    chest: 0.02,
    lift_step: 0.07,
};

/// The square's pop and lock.
const SQ_STYLE: Style = Style {
    bounce: 0.06,
    sway: 0.05,
    snap: 1.0,
    ..STYLE
};

/// The afterglow: how far back (beats).
const AFTER: f32 = 0.06;
/// The ripple's wavelength and the sample spacing (units).
const WAVELENGTH: f32 = 0.075;
const SPACING: f32 = 0.008;
/// The most samples a chain holds (the longest, a leg, is about 0.5 units).
const MAX_SAMPLES: usize = 96;
/// The Lissajous head's samples.
const LOOP: usize = 141;

/// Two beats of the shared groove, the level jumping with the beat.
pub fn groove(b: f32, st: &Style) -> Fig {
    let mut p = moves::groove(b, st);
    p.level = 0.4 + 0.5 * beat_down(pmod(b, 1.0), 2.5);
    p.eyes = Eyes::Open;
    p
}

/// Four beats of the shared step-and-touch.
pub fn step(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    p.level = 0.4 + 0.4 * beat_down(pmod(b, 1.0), 2.5);
    p.eyes = Eyes::Open;
    p
}

/// Two beats: the ripples snap square and the poses go angular (pop and lock); half way a
/// saw; it melts back to a sine at the end.
pub fn square(b: f32, _st: &Style) -> Fig {
    let bb = pmod(b, 2.0);
    let mut p = moves::pump(b, &SQ_STYLE);
    p.shape = ease_out_back(clamp01(bb / 0.15), 1.6) * (1.0 - smooth((bb - 1.6) / 0.4));
    p.sig = if (1.0..1.6).contains(&bb) { 1.0 } else { 0.0 }; // the saw
    p.level = 0.95;
    p.eyes = if bb < 0.3 { Eyes::Wide } else { Eyes::Open };
    p
}

const LINE_NIGHT: [u8; 4] = rgb(0xF2C766);
const LINE_PAPER: [u8; 4] = rgb(0x9C711C);
const CORE: [u8; 4] = rgb(0xFFF4D6);
const HALO: [u8; 4] = rgb(0xE2B85A);
const DOT_NIGHT: [u8; 4] = rgb(0xFFF4D6);
const DOT_PAPER: [u8; 4] = rgb(0x5E4210);

pub struct Onda {
    info: CharacterInfo,
    family: dance::Style,
}

impl Onda {
    pub fn new() -> Onda {
        Onda {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Onda".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            family: dance::Style {
                bounce: 0.06,
                squash: 0.05,
                arms: 1.1,
                sway: 0.07,
                lag: 0.10,
                hop: 0.18,
                stride: 0.14,
            },
        }
    }
}

impl Default for Onda {
    fn default() -> Self {
        Onda::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Onda> {
    slate::Slate(Onda::new())
}

impl Figure for Onda {
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
            Move::Bounce => groove(b, &STYLE),
            Move::StepLeft | Move::StepRight => step(b, &STYLE),
            Move::Jump => square(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Its moods are the wave's: excited loud, calm and sleepy quiet; its eighth mood
    /// (Shy's place) is the square.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Excited => f.level = 1.0,
            Mood::Calm => f.level = f.level.min(0.2),
            Mood::Sleepy => f.level = f.level.min(0.05),
            Mood::Surprised => f.shape = f.shape.max(1.0),
            Mood::Shy => {
                f.eyes = Eyes::Open;
                f.shape = f.shape.max(1.0);
                f.level = f.level.max(0.8);
            }
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin, the square's ripple included;
    /// `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.52,
            top: -1.18,
            right: 0.52,
            bottom: 0.07,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        let after = if pose.playing {
            Some(solve(&slate::fig_before(self, pose, AFTER), &PROPS))
        } else {
            None
        };
        draw(f, after.as_ref(), pose, c);
    }
}

/// The waveform at `x` (cycles): a sine squaring with `shape`, or a saw.
fn wave(x: f32, shape: f32, saw: bool) -> f32 {
    if saw {
        return 2.0 * pmod(x + 0.25, 1.0) - 1.0;
    }
    let s = math::sin(2.0 * PI * x);
    let k = (1.0 - shape * 0.96).max(0.06);
    let m = math::powf(s.abs().max(1e-9), k);
    if s < 0.0 {
        -m
    } else {
        m
    }
}

/// The seven chains of the skeleton the trace follows (as placed): two legs, the spine, the
/// shoulders, two arms, the hips.
fn chains(j: &Joints) -> [([P; 3], usize); 7] {
    let p = |q: P| place(q, j);
    let hips = [p(j.hips[0]), p(j.hips[1])];
    let none = (0.0, 0.0);
    [
        ([hips[0], p(j.knees[0]), p(j.feet[0])], 3),
        ([hips[1], p(j.knees[1]), p(j.feet[1])], 3),
        ([p(j.pelvis), p(j.neck), none], 2),
        ([p(j.shoulders[0]), p(j.neck), p(j.shoulders[1])], 3),
        ([p(j.shoulders[0]), p(j.elbows[0]), p(j.hands[0])], 3),
        ([p(j.shoulders[1]), p(j.elbows[1]), p(j.hands[1])], 3),
        ([hips[0], p(j.pelvis), hips[1]], 3),
    ]
}

/// The rippled samples of a chain into `out` (the design's `trace`), returning how many.
fn trace(pts: &[P], f: &Fig, t: f32, phase: f32, out: &mut [P; MAX_SAMPLES]) -> usize {
    let amp = (0.006 + 0.016 * f.level) * (1.0 + 0.6 * f.shape);
    let total: f32 = pts.windows(2).map(|w| length(sub(w[1], w[0]))).sum();
    let saw = f.sig > 0.5;
    let mut n = 0;
    let mut run = 0.0;
    let last = pts.len().saturating_sub(2);
    for (i, w) in pts.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let l = length(sub(b, a)).max(1e-6);
        let u = mul(sub(b, a), 1.0 / l);
        let nrm = (-u.1, u.0);
        let steps = ((l / SPACING) as usize).max(4);
        let end = steps + usize::from(i == last);
        for k in 0..end {
            if n >= MAX_SAMPLES {
                return n;
            }
            let s = k as f32 / steps as f32;
            let along = run + l * s;
            let pos = add(a, mul(u, l * s));
            let wv = wave(along / WAVELENGTH - t * 2.0 + phase, f.shape, saw);
            let fade = (along / 0.03).min((total - along) / 0.03).clamp(0.0, 1.0);
            out[n] = add(pos, mul(nrm, amp * wv * fade));
            n += 1;
        }
        run += l;
    }
    n
}

/// Every segment of `pts` (every `stride`th point, the last kept) as a quad `w` wide,
/// extended by half its width at both ends (so the joins close), wound the rig's way: one
/// sub-path a segment, no folds. A wide soft layer (the halo) takes fewer, longer segments.
fn strip_into(s: &mut dyn Sink, pts: &[P], w: f32, stride: usize) {
    let h = w * 0.5;
    let stride = stride.max(1);
    let n = pts.len();
    if n < 2 {
        return;
    }
    let mut i = 0;
    while i + 1 < n {
        let j = (i + stride).min(n - 1);
        let (a, b) = (pts[i], pts[j]);
        i = j;
        let d = sub(b, a);
        let l = length(d);
        if l < 1e-7 {
            continue;
        }
        let u = mul(d, 1.0 / l);
        let nrm = (u.1, -u.0);
        let a2 = sub(a, mul(u, h));
        let b2 = add(b, mul(u, h));
        let q = [
            add(a2, mul(nrm, h)),
            add(b2, mul(nrm, h)),
            sub(b2, mul(nrm, h)),
            sub(a2, mul(nrm, h)),
        ];
        s.move_to(q[0].0, q[0].1);
        for p in &q[1..] {
            s.line_to(p.0, p.1);
        }
        s.close();
    }
}

/// The trace's centre lines, visited (the chains rippled, the Lissajous head, the eyes'
/// lines) with each one's width and strength.
fn lines(f: &Fig, j: &Joints, t: f32, v: &mut dyn FnMut(&[P], f32, f32)) {
    let mut buf = [(0.0, 0.0); MAX_SAMPLES];
    for (k, (pts, n)) in chains(j).iter().enumerate() {
        let m = trace(&pts[..*n], f, t, k as f32 * 0.37, &mut buf);
        v(&buf[..m], 0.010, 1.0);
    }
    let hc = place(j.head, j);
    let r = 0.070;
    let dl = t * PI * 0.5;
    let mut head = [(0.0, 0.0); LOOP];
    for (i, q) in head.iter_mut().enumerate() {
        let a = i as f32 * 2.0 * PI / (LOOP - 1) as f32;
        *q = add(
            hc,
            (r * math::sin(3.0 * a + dl), r * 1.08 * math::sin(2.0 * a)),
        );
    }
    v(&head, 0.008, 1.0);
    for side in [-1.0f32, 1.0] {
        let c = add(hc, (side * 0.024, -0.006));
        match f.eyes {
            Eyes::Happy => v(
                &[
                    add(c, (-0.010, 0.006)),
                    add(c, (0.0, -0.004)),
                    add(c, (0.010, 0.006)),
                ],
                0.006,
                1.0,
            ),
            Eyes::Closed => v(
                &[add(c, (-0.010, 0.002)), add(c, (0.010, 0.002))],
                0.005,
                1.0,
            ),
            Eyes::Sleepy => v(
                &[add(c, (-0.010, 0.002)), add(c, (0.010, 0.002))],
                0.005,
                0.6,
            ),
            Eyes::Wink if side > 0.0 => v(
                &[add(c, (-0.010, 0.002)), add(c, (0.010, 0.002))],
                0.005,
                1.0,
            ),
            Eyes::Cool if side < 0.0 => v(
                &[add(hc, (-0.040, -0.006)), add(hc, (0.040, -0.006))],
                0.008,
                1.0,
            ),
            _ => {}
        }
    }
}

/// Where the beam is: a sample of the whole trace, running along it once every 1.25 beats.
fn beam(f: &Fig, j: &Joints, t: f32) -> P {
    let mut buf = [(0.0, 0.0); MAX_SAMPLES];
    let mut counts = [0usize; 7];
    let cs = chains(j);
    let mut total = 0;
    for (k, (pts, n)) in cs.iter().enumerate() {
        counts[k] = trace(&pts[..*n], f, t, k as f32 * 0.37, &mut buf);
        total += counts[k];
    }
    let mut at = ((pmod(t * 0.8, 1.0)) * total.saturating_sub(1) as f32) as usize;
    for (k, (pts, n)) in cs.iter().enumerate() {
        if at < counts[k] {
            trace(&pts[..*n], f, t, k as f32 * 0.37, &mut buf);
            return buf[at];
        }
        at -= counts[k];
    }
    place(j.head, j)
}

fn draw(f: &Fig, after: Option<&Joints>, pose: &Pose, c: &mut Canvas) {
    let j = solve(f, &PROPS);
    let mut br = Brush::new(c, 0.0);
    let night = br.ink.dark;
    let t = pose.beats as f32;
    let gx = (place(j.feet[0], &j).0 + place(j.feet[1], &j).0) / 2.0;
    slate::on_ground(br.c, pose, |c| {
        let mut b = Brush::new(c, 0.0);
        b.shadow((gx, 0.006), 0.15, 0.020, 0.45);
        if night {
            b.blot((gx, 0.004), 0.12, 0.012, HALO, 0.10);
        }
    });
    // Persistence: a faint afterglow of where the trace was a moment ago (night only).
    if let (true, Some(a)) = (night, after) {
        let col = br.fixed(alpha(LINE_NIGHT, 0.16));
        br.piece(
            &|s, _| {
                for (pts, n) in chains(a) {
                    strip_into(s, &pts[..n], 0.010, 1);
                }
            },
            col,
            false,
        );
    }
    let line = if night { LINE_NIGHT } else { LINE_PAPER };
    // Below about 140 pixels tall (a picker tile, a small window) the halo and the core
    // would be mush: the line alone.
    let small = br.c.px_per_unit < 140.0;
    let layer = |br: &mut Brush, k: f32, stride: usize, col: Color| {
        br.piece(
            &|s, _| {
                lines(f, &j, t, &mut |pts, w, o| {
                    if o > 0.05 {
                        strip_into(s, pts, w * k * (0.45 + 0.55 * o), stride);
                    }
                });
            },
            col,
            false,
        );
    };
    let night = night && !small;
    // The halo is wide and soft: a quarter of the samples draw it; the core, a third.
    if night {
        let halo = br.hued(alpha(HALO, 0.22));
        layer(&mut br, 3.4, 4, halo);
    }
    let lc = br.hued(line);
    layer(&mut br, if night { 1.0 } else { 1.2 }, 1, lc);
    if night {
        let core = br.fixed(CORE);
        layer(&mut br, 0.35, 3, core);
    }
    // The dwell points for eyes (open, wide, the open one of a wink), and the beam dot.
    let hc = place(j.head, &j);
    let dot = br.fixed(if night { DOT_NIGHT } else { DOT_PAPER });
    let glow = br.hued(alpha(HALO, 0.35));
    let mut dots = [((0.0, 0.0), 0.0f32, 0.0f32); 3];
    let mut nd = 0;
    for side in [-1.0f32, 1.0] {
        let open = match f.eyes {
            Eyes::Open | Eyes::Wide => true,
            Eyes::Wink => side < 0.0,
            _ => false,
        };
        if open {
            let wide = f.eyes == Eyes::Wide;
            dots[nd] = (
                add(hc, (side * 0.024, -0.006)),
                if wide { 0.016 } else { 0.012 },
                if wide { 0.008 } else { 0.006 },
            );
            nd += 1;
        }
    }
    if pose.playing {
        dots[nd] = (beam(f, &j, t), 0.022, 0.010);
        nd += 1;
    }
    if night {
        br.piece(
            &|s, _| {
                dots[..nd]
                    .iter()
                    .for_each(|(p, r, _)| circle_into(s, *p, *r, 0.0))
            },
            glow,
            false,
        );
    }
    br.piece(
        &|s, _| {
            dots[..nd]
                .iter()
                .for_each(|(p, _, r)| circle_into(s, *p, *r, 0.0))
        },
        dot,
        false,
    );
}
