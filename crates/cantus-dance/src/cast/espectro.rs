//! **Espectro**, the analyser (`builtin:espectro`, round 40, ADR 0122;
//! `docs/round40/design/dancer/espectro.md`): a figure made of spectrum bars — its body is
//! the music. Every part is a little analyser of segmented bars lit from the part's bottom
//! up by a band of the master's spectrum: the legs the lows (the kick), the torso the mids,
//! the arms the snare and claps, the head the highs (the hats). Segments light lapis, then
//! gold, then vermilion near the top (the logo's colours as an analyser's), a gold peak cap
//! on each bar; the unlit segments are the well, so the silhouette always shows. Its eyes and
//! mouth are cut through the head's bars. Its signature on the drop, **Pico**: every bar
//! drops, climbs, hits the top on the beat and breaks into gold sparks.
//!
//! **The spectrum** is the real one, heard on the interface's side (the companion's
//! analyser, `Pose::bands`) — never on the audio thread; with none (a preview, a stopped
//! song) the bars dance to a spectrum made from the beat, as the designs do.
//! **No masks:** each bar's span inside its part is computed (the vertical extent of the
//! part's capsules, ellipse or outline at that column) and filled as plain rectangles,
//! batched by colour.

use crate::character::{Bounds, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::{rgb, Canvas};
use crate::rig::dance;
use crate::rig::draw::{mix, polygon_into, Sink};
use crate::rig::P;
use crate::slate::fig::*;
use crate::slate::moves::{self, Style};
use crate::slate::paint::{capsule_into, rrect_into, Brush};
use crate::slate::{self, place, solve, Eyes, Fig, Figure, Joints, Props};
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use tiny_skia::Color;

pub const ID: &str = "builtin:espectro";

pub const PROPS: Props = Props {
    leg: 0.46,
    thigh: 0.240,
    shin: 0.230,
    torso: 0.27,
    neck: 0.03,
    head: 0.095,
    shoulder: 0.100,
    hipw: 0.055,
    stance: 0.070,
    upper: 0.155,
    fore: 0.150,
    foot: 0.10,
};

pub const STYLE: Style = Style {
    bounce: 0.10,
    sway: 0.08,
    step: 0.18,
    arms: 1.0,
    lag: 0.10,
    hop: 0.16,
    squash: 0.08,
    snap: 0.0,
    lean: 1.0,
    open: (0.60, 0.90),
    chest: 0.02,
    lift_step: 0.08,
};

/// A bar's width, the gap between bars, a segment's height (units).
const BAR: f32 = 0.020;
const GAP: f32 = 0.008;
const SEG: f32 = 0.022;
/// The room-coloured line between segments.
const SEG_GAP: f32 = 0.0045;

/// Two beats of the shared groove.
pub fn groove(b: f32, st: &Style) -> Fig {
    let mut p = moves::groove(b, st);
    p.eyes = Eyes::Open;
    p.sig = 0.0;
    p
}

/// Four beats of the shared step-and-touch.
pub fn step(b: f32, st: &Style) -> Fig {
    let mut p = moves::steps(b, st);
    p.eyes = if pmod(b, 2.0) > 0.8 {
        Eyes::Happy
    } else {
        Eyes::Open
    };
    p
}

/// Two beats: it crouches as every bar drops; it rises, stretching, as every bar climbs; on
/// beat 2 every bar hits the top and breaks into sparks; it lands, bars low, and the music
/// comes back.
pub fn pico(b: f32, _st: &Style) -> Fig {
    let mut p = Fig::default();
    let bb = pmod(b, 2.0);
    if bb < 1.0 {
        let down = smooth(bb / 0.35) * (1.0 - smooth((bb - 0.35) / 0.65));
        let rise = smooth((bb - 0.35) / 0.65);
        p.squash = 0.14 * down - 0.12 * rise;
        p.hip = (0.0, 0.18 * down);
        p.hands = [
            (-0.62 - 0.1 * rise, 0.90 - 1.85 * rise),
            (0.62 + 0.1 * rise, 0.90 - 1.85 * rise),
        ];
        p.level = 0.12 * (1.0 - rise) + rise * rise;
        p.eyes = if down > 0.5 { Eyes::Closed } else { Eyes::Wide };
        p.sig = 0.0;
    } else {
        let t = bb - 1.0;
        p.sig = t; // the sparks' age
        let land = smooth(t / 0.3);
        p.squash = if t < 0.3 { 0.08 * (1.0 - land) } else { 0.0 };
        p.hands = [(-0.95, 0.30), (0.95, 0.30)];
        p.level = 0.15 + 0.5 * smooth((t - 0.4) / 0.6);
        p.eyes = Eyes::Happy;
        p.mouth = 0.9 * (1.0 - land);
    }
    p.accent = beat_down(pmod(b, 1.0), 8.0);
    p
}

/// (night, paper): the well, the lapis, the gold, the vermilion, the caps.
struct Pal {
    dim: [u8; 4],
    low: [u8; 4],
    mid: [u8; 4],
    high: [u8; 4],
    cap: [u8; 4],
}

const NIGHT: Pal = Pal {
    dim: rgb(0x323B44),
    low: rgb(0x6A86DA),
    mid: rgb(0xE2B85A),
    high: rgb(0xF0784A),
    cap: rgb(0xF2C766),
};

const PAPER: Pal = Pal {
    dim: rgb(0xD6DDE3),
    low: rgb(0x4664C0),
    mid: rgb(0x9C711C),
    high: rgb(0xD2541A),
    cap: rgb(0x7A5A04),
};

pub struct Espectro {
    info: CharacterInfo,
    family: dance::Style,
}

impl Espectro {
    pub fn new() -> Espectro {
        Espectro {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Espectro".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            family: dance::Style {
                bounce: 0.07,
                squash: 0.08,
                arms: 1.1,
                sway: 0.07,
                lag: 0.10,
                hop: 0.20,
                stride: 0.14,
            },
        }
    }
}

impl Default for Espectro {
    fn default() -> Self {
        Espectro::new()
    }
}

/// A Slate character for the cast list.
pub fn character() -> slate::Slate<Espectro> {
    slate::Slate(Espectro::new())
}

impl Figure for Espectro {
    fn info(&self) -> &CharacterInfo {
        &self.info
    }

    fn props(&self) -> &Props {
        &PROPS
    }

    fn family(&self) -> &dance::Style {
        &self.family
    }

    fn hears_spectrum(&self) -> bool {
        true
    }

    fn own(&self, mv: Move, b: f32) -> Option<Fig> {
        Some(match mv {
            Move::Bounce => groove(b, &STYLE),
            Move::StepLeft | Move::StepRight => step(b, &STYLE),
            Move::Jump => pico(b, &STYLE),
            _ => return None,
        })
    }

    fn routine(&self) -> &[Move] {
        &[Move::Bounce, Move::StepRight, Move::Jump]
    }

    /// Its moods are the meter's: excited and its eighth mood (loud, Shy's place) at full,
    /// calm and sleepy low.
    fn mood(&self, mood: Mood, f: &mut Fig) {
        match mood {
            Mood::Excited => {
                f.level = 1.0;
                f.mouth = f.mouth.max(0.8);
            }
            Mood::Surprised => f.mouth = f.mouth.max(0.5),
            Mood::Calm => f.level = f.level.min(0.3),
            Mood::Sleepy => f.level = f.level.min(0.1),
            Mood::Shy => {
                f.eyes = Eyes::Happy;
                f.level = 1.0;
                f.mouth = 1.0;
            }
            _ => {}
        }
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin, Pico's sparks included;
    /// `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.58,
            top: -1.36,
            right: 0.58,
            bottom: 0.07,
        }
    }

    fn draw(&self, f: &Fig, pose: &Pose, c: &mut Canvas) {
        draw(f, pose, c);
    }
}

/// A band's level at column `u` (0…1 across the part) at beat `t`: the real band where one
/// is heard, else one made from the beat (kick on the beats, snare on 2 and 4, hats on the
/// eighths), with a wobble across the columns; sized by the figure's level.
fn spectrum(band: usize, u: f32, t: f32, level: f32, heard: Option<[f32; 4]>) -> f32 {
    let wob = 0.10 * math::sin(u * 11.0 + t * 7.3) + 0.06 * math::sin(u * 23.0 - t * 5.1);
    let base = match heard {
        Some(b) => 0.15 + 0.85 * b[band],
        None => {
            let ph = pmod(t, 1.0);
            let kick = beat_down(ph, 2.5);
            let snare = beat_down(pmod(t - 1.0, 2.0) / 2.0, 6.0);
            let hats = beat_down(pmod(2.0 * t, 1.0), 3.0);
            [
                0.30 + 0.60 * kick,
                0.35 + 0.30 * kick + 0.25 * snare,
                0.25 + 0.65 * snare,
                0.25 + 0.55 * hats,
            ][band]
        }
    };
    clamp01((base + wob) * (0.45 + 0.75 * level)).max(0.15)
}

/// A part of the body: its outline's pieces, its band, a seed for its columns.
#[derive(Copy, Clone)]
enum Shape {
    /// Capsules through three points with three widths (a leg, an arm).
    Tube([P; 3], [f32; 3]),
    /// A convex outline (the torso).
    Poly([P; 10]),
    /// An ellipse (the head).
    Oval(P, f32, f32),
}

impl Shape {
    /// Its vertical extents at `x` (top, bottom): at most two (a bent leg's thigh and
    /// shin apart), how many.
    fn spans(&self, x: f32) -> ([(f32, f32); 2], usize) {
        let mut out = [(0.0, 0.0); 2];
        let mut n = 0;
        let mut put = |lo: f32, hi: f32| {
            if hi <= lo {
                return;
            }
            for o in out.iter_mut().take(n) {
                if lo <= o.1 && hi >= o.0 {
                    *o = (o.0.min(lo), o.1.max(hi));
                    return;
                }
            }
            if n < 2 {
                out[n] = (lo, hi);
                n += 1;
            }
        };
        match self {
            Shape::Tube(p, w) => {
                // Circles along each segment: each segment's extent at x.
                for seg in 0..2 {
                    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                    for k in 0..=10 {
                        let t = k as f32 / 10.0;
                        let c = lerp_p(p[seg], p[seg + 1], t);
                        let r = lerp(w[seg], w[seg + 1], t) * 0.5;
                        let dx = x - c.0;
                        if dx.abs() < r {
                            let h = (r * r - dx * dx).sqrt();
                            lo = lo.min(c.1 - h);
                            hi = hi.max(c.1 + h);
                        }
                    }
                    put(lo, hi);
                }
            }
            Shape::Poly(pts) => {
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for i in 0..pts.len() {
                    let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                    if (a.0 - x) * (b.0 - x) <= 0.0 && (a.0 - b.0).abs() > 1e-6 {
                        let y = a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0);
                        lo = lo.min(y);
                        hi = hi.max(y);
                    }
                }
                put(lo, hi);
            }
            Shape::Oval(c, rx, ry) => {
                let dx = (x - c.0) / rx;
                if dx.abs() < 1.0 {
                    let h = ry * (1.0 - dx * dx).sqrt();
                    put(c.1 - h, c.1 + h);
                }
            }
        }
        (out, n)
    }

    /// Its bounding box (left, top, right, bottom), padded.
    fn bbox(&self, pad: f32) -> (f32, f32, f32, f32) {
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut grow = |p: P| {
            b = (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1));
        };
        match self {
            Shape::Tube(p, _) => p.iter().for_each(|q| grow(*q)),
            Shape::Poly(p) => p.iter().for_each(|q| grow(*q)),
            Shape::Oval(c, rx, ry) => {
                grow((c.0 - rx, c.1 - ry));
                grow((c.0 + rx, c.1 + ry));
            }
        }
        (b.0 - pad, b.1 - pad, b.2 + pad, b.3 + pad)
    }

    /// Its outline into a sink, grown by `g`.
    fn outline(&self, s: &mut dyn Sink, g: f32) {
        match self {
            Shape::Tube(p, w) => {
                capsule_into(s, p[0], p[1], w[0] + 2.0 * g, w[1] + 2.0 * g, 0.0);
                capsule_into(s, p[1], p[2], w[1] + 2.0 * g, w[2] + 2.0 * g, 0.0);
            }
            Shape::Poly(p) => crate::rig::draw::blob_grown_into(s, p, g),
            Shape::Oval(c, rx, ry) => crate::rig::draw::ellipse_into(s, *c, rx + g, ry + g, 0.0),
        }
    }
}

/// One part: its shape, its band, its seed.
#[derive(Copy, Clone)]
struct Part {
    shape: Shape,
    band: usize,
    seed: f32,
    pad: f32,
}

/// The layers of the meters: the well, the three colours, the caps.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Layer {
    Dim,
    Low,
    Mid,
    High,
    Cap,
}

/// Every segment rectangle of `parts` in `layer` into a sink (bars computed, no masks).
fn meters(
    s: &mut dyn Sink,
    parts: &[Part],
    layer: Layer,
    f: &Fig,
    t: f32,
    heard: Option<[f32; 4]>,
) {
    for p in parts {
        // Lit from the bottom of the part's (padded) box, as the design's meters are.
        let (x0, top, x1, y1) = p.shape.bbox(p.pad);
        let bot = y1;
        let hgt = (bot - top).max(0.02);
        let n = ((x1 - x0) / (BAR + GAP)) as usize;
        for i in 0..=n {
            let x = x0 + i as f32 * (BAR + GAP);
            // The bar's spans inside the part, at its middle.
            let (spans, ns) = p.shape.spans(x + BAR * 0.5);
            if ns == 0 {
                continue;
            }
            let u = (i as f32 + p.seed * 0.37) / (n + 1) as f32;
            let mut lv = spectrum(p.band, u + p.seed, t, f.level, heard);
            if f.sig > 0.0 && f.sig < 0.15 {
                lv = 1.0;
            }
            let peak = match heard {
                Some(_) => (lv + 0.06).min(1.0),
                None => lv
                    .max(spectrum(p.band, u + p.seed, t - 0.12, f.level, None) - 0.08)
                    .max(spectrum(p.band, u + p.seed, t - 0.24, f.level, None) - 0.16),
            };
            for &(s_top, s_bot) in &spans[..ns] {
                bar(s, layer, x, (s_top, s_bot), (bot, hgt, y1), lv, peak);
            }
        }
    }
}

/// One bar's segments in `layer`, inside the span (`s_top`, `s_bot`), lit from the part's
/// bottom to `lv` of its height, its cap at `peak`.
fn bar(
    s: &mut dyn Sink,
    layer: Layer,
    x: f32,
    (s_top, s_bot): (f32, f32),
    (bot, hgt, y1): (f32, f32, f32),
    lv: f32,
    peak: f32,
) {
    if layer == Layer::Cap {
        let y = y1.min(bot) - peak * hgt - 0.010;
        if y > s_top && y < s_bot {
            rrect_into(s, (x + BAR * 0.5, y + 0.003), BAR, 0.006, 0.0, 0.0, 0.0);
        }
        return;
    }
    // Segments from the part's bottom up, SEG apart, each clipped to the span.
    let mut k = 0;
    loop {
        let seg_bot = bot - k as f32 * SEG;
        let seg_top = seg_bot - (SEG - SEG_GAP);
        if seg_bot < s_top {
            break;
        }
        k += 1;
        let (a, b) = (seg_top.max(s_top), seg_bot.min(s_bot));
        if b - a < 1e-4 {
            continue;
        }
        // How far up the part this segment is: what colour it lights.
        let share = (bot - (a + b) * 0.5) / hgt;
        let want = if share > lv {
            Layer::Dim
        } else if share < 0.62 {
            Layer::Low
        } else if share < 0.86 {
            Layer::Mid
        } else {
            Layer::High
        };
        if want == layer {
            polygon_into(s, &[(x, a), (x + BAR, a), (x + BAR, b), (x, b)]);
        }
    }
}

fn draw(f: &Fig, pose: &Pose, c: &mut Canvas) {
    let j = solve(f, &PROPS);
    let mut br = Brush::new(c, 0.0);
    let pal = if br.ink.dark { &NIGHT } else { &PAPER };
    let t = pose.beats as f32;
    let heard = (pose.bands.iter().any(|b| *b > 0.0)).then_some(pose.bands);
    let p = |q: P| place(q, &j);
    let gx = (p(j.feet[0]).0 + p(j.feet[1]).0) / 2.0;
    let lift = f.lift;
    slate::on_ground(br.c, pose, |c| {
        let k = (1.0 - lift * 3.0).max(0.0);
        Brush::new(c, 0.0).shadow((gx, 0.008), 0.17 * k + 0.05, 0.024 * k + 0.008, 0.5);
    });
    let (body, arms) = parts(&j);
    let room = br.room;
    let col = |br: &Brush, c: [u8; 4]| br.hued(c);
    let layers = [
        (Layer::Dim, br.fixed(pal.dim)),
        (Layer::Low, col(&br, pal.low)),
        (Layer::Mid, col(&br, pal.mid)),
        (Layer::High, col(&br, pal.high)),
        (Layer::Cap, col(&br, pal.cap)),
    ];
    // The legs, the torso and the head: their room (the gaps), then the meters' layers.
    br.piece(
        &|s, _| body.iter().for_each(|q| q.shape.outline(s, 0.0)),
        room,
        false,
    );
    for (layer, colr) in layers {
        br.piece(&|s, _| meters(s, &body, layer, f, t, heard), colr, false);
    }
    face(&mut br, f, &j);
    // The arms: cut from the torso in the room's colour, then their meters.
    br.piece(
        &|s, _| arms.iter().for_each(|q| q.shape.outline(s, 0.011)),
        room,
        false,
    );
    for (layer, colr) in layers {
        br.piece(&|s, _| meters(s, &arms, layer, f, t, heard), colr, false);
    }
    if f.sig > 0.0 {
        sparks(&mut br, f, &body, &arms, pal);
    }
}

/// The body's parts and the arms (drawn over it).
fn parts(j: &Joints) -> ([Part; 4], [Part; 2]) {
    let p = |q: P| place(q, j);
    let pelvis = p(j.pelvis);
    let leg = |i: usize| {
        let (hip, knee, foot) = (p(j.hips[i]), p(j.knees[i]), p(j.feet[i]));
        let top = add(add(hip, mul(sub(pelvis, hip), 0.3)), (0.0, -0.03));
        Part {
            shape: Shape::Tube([top, knee, add(foot, (0.0, -0.010))], [0.096, 0.082, 0.090]),
            band: 0,
            seed: 0.1 + i as f32 * 0.5,
            pad: 0.05,
        }
    };
    let neck = p(j.neck);
    let l = |x: f32, y: f32| add(neck, rot0((x, y), j.torso_deg));
    let tt = PROPS.torso;
    let torso = [
        l(-0.050, -0.004),
        l(-0.118, 0.012),
        l(-0.130, 0.060),
        l(-0.105, 0.170),
        l(-0.100, tt + 0.030),
        l(0.100, tt + 0.030),
        l(0.105, 0.170),
        l(0.130, 0.060),
        l(0.118, 0.012),
        l(0.050, -0.004),
    ];
    let hc = p(j.head);
    let r = PROPS.head;
    let arm = |i: usize| {
        let (sh, el, ha) = (p(j.shoulders[i]), p(j.elbows[i]), p(j.hands[i]));
        let sh = add(sh, mul(sub(neck, sh), 0.10));
        let fd = sub(ha, el);
        let fl = length(fd).max(1e-6);
        Part {
            shape: Shape::Tube(
                [sh, el, add(ha, mul(fd, 0.030 / fl))],
                [0.080, 0.070, 0.074],
            ),
            band: 2,
            seed: 0.2 + i as f32 * 0.4,
            pad: 0.045,
        }
    };
    (
        [
            leg(0),
            leg(1),
            Part {
                shape: Shape::Poly(torso),
                band: 1,
                seed: 0.3,
                pad: 0.0,
            },
            Part {
                shape: Shape::Oval(hc, r * 1.05, r * 1.10),
                band: 3,
                seed: 0.7,
                pad: 0.0,
            },
        ],
        [arm(0), arm(1)],
    )
}

/// The eyes and mouth, cut through the head's bars in the room's colour.
fn face(br: &mut Brush, f: &Fig, j: &Joints) {
    let hc = place(j.head, j);
    let room = br.room;
    let e = f.eyes;
    let rect = |s: &mut dyn Sink, c: P, w: f32, h: f32| rrect_into(s, c, w, h, 0.0, 0.0, 0.0);
    br.piece(
        &|s, _| {
            for side in [-1.0f32, 1.0] {
                let c = add(hc, (side * 0.034, -0.010));
                match e {
                    Eyes::Happy => {
                        crate::slate::paint::bars_into(
                            s,
                            &[
                                add(c, (-0.014, 0.008)),
                                add(c, (0.0, -0.006)),
                                add(c, (0.014, 0.008)),
                            ],
                            0.010,
                            0.0,
                        );
                    }
                    Eyes::Closed | Eyes::Sleepy => rect(s, add(c, (0.0, 0.002)), 0.028, 0.008),
                    Eyes::Wink if side > 0.0 => rect(s, add(c, (0.0, 0.002)), 0.028, 0.008),
                    Eyes::Cool => {
                        if side < 0.0 {
                            rect(s, add(hc, (0.0, -0.010)), 0.120, 0.022);
                        }
                    }
                    _ => {
                        let w = if e == Eyes::Wide { 0.026 } else { 0.020 };
                        rect(s, c, w, w * 1.4);
                    }
                }
            }
            let mw = if f.mouth > 0.3 { 0.050 } else { 0.036 };
            let mh = 0.010 + 0.020 * f.mouth;
            rect(s, add(hc, (0.0, 0.038 + mh * 0.5)), mw, mh);
        },
        room,
        false,
    );
}

/// Pico: sparks out of every part's top, flying and falling, fading into the room.
fn sparks(br: &mut Brush, f: &Fig, body: &[Part; 4], arms: &[Part; 2], pal: &Pal) {
    let t = f.sig;
    let a = (1.0 - t * 1.1).max(0.0);
    if a <= 0.0 {
        return;
    }
    let col: Color = br.hued(mix(pal.cap, br.ink.room, 1.0 - a));
    br.piece(
        &|s, _| {
            for (n, part) in body.iter().chain(arms.iter()).enumerate() {
                let (x0, y0, x1, _) = part.shape.bbox(part.pad);
                for k in 0..6 {
                    let r = crate::rig::hash((n * 7 + k) as i64);
                    let r2 = crate::rig::hash((n * 7 + k) as i64 + 1000);
                    let vx = (r - 0.5) * 0.9;
                    let vy = -0.9 - 0.6 * r2;
                    let sx = x0 + (x1 - x0) * ((k as f32 + 0.5) / 6.0);
                    let tt = t * 0.6;
                    let px = sx + vx * tt;
                    let py = y0 + vy * tt + 1.6 * tt * tt;
                    rrect_into(s, (px, py), 0.016, 0.016, 0.0, r * 90.0, 0.0);
                }
            }
        },
        col,
        false,
    );
}
