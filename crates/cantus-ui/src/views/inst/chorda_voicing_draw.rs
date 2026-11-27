//! Chorda's voicing view's pictures (round 42, `chorda-presets`): each group's numbers as the
//! model takes them, drawn from the voicing itself (`cantus_dsp::chorda::voicing`) — the
//! technician's voicing as a dashed line, the voicing as set in the group's hue over it.
//!
//! - **Hammers**: the felt's stiffness across the keyboard (A0 … C8, log).
//! - **Strings**: the sustain across the keyboard (log), the overtones' beside it (thin).
//! - **Soundboard and case**: the bass formant's response, 50 Hz … 5 kHz.
//! - **Action**: the dampers' hold across the keyboard (log).
//! - **Microphones**: each key's arrival-time difference between the pair (ms).
//! - **Tone**: the top roll-off's response, 1 … 20 kHz.

use crate::views::inst::organ::parts::label;
use crate::i18n::tf;
use crate::kit::light;
use crate::kit::text::Role;
use crate::kit::{Brush, Style};
use cantus_dsp::chorda::voicing::Voicing;
use cantus_dsp::math;
use egui::{Pos2, Rect, Vec2};

/// The two voicings a picture compares: the technician's and the one set, with the knobs
/// over the voicing that act beside it (the hardness, the decay).
pub struct Pair {
    pub base: Voicing,
    pub set: Voicing,
    pub hardness: f64,
    pub decay: f64,
    pub tilt: f64,
}

/// What a picture of one group is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picture {
    Hammers,
    Strings,
    Board,
    Action,
    Mics,
    Tone,
}

const KEYS: std::ops::RangeInclusive<u8> = 21..=108;

fn caption(d: Rect, s: &Style) -> (Rect, Rect) {
    let line = Rect::from_min_size(
        Pos2::new(d.left() + s.s(10.0), d.top() + s.s(3.0)),
        Vec2::new(d.width() - s.s(20.0), s.s(15.0)),
    );
    let room = Rect::from_min_max(
        Pos2::new(d.left() + s.s(10.0), line.bottom() + s.s(4.0)),
        Pos2::new(d.right() - s.s(10.0), d.bottom() - s.s(6.0)),
    );
    (line, room)
}

fn plot(b: &mut Brush, room: Rect, s: &Style) -> Rect {
    light::sunken(b, room, s.r(3.0), s.t.well, 1.0, s.t.depth);
    room.shrink(s.s(6.0))
}

/// A scale as a musician reads it against the voiced one: `×1.25`.
fn times(x: f64) -> String {
    format!("\u{d7}{x:.2}")
}

/// Two curves over the keyboard, `base` dashed and `set` in the hue, on a log scale that
/// holds both (at least an octave each way of the base's middle).
fn keyboard_log(b: &mut Brush, r: Rect, s: &Style, base: &[f64], set: &[f64]) {
    let ln = |x: &f64| math::ln(x.max(1e-9));
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for v in base.iter().chain(set) {
        lo = lo.min(ln(v));
        hi = hi.max(ln(v));
    }
    let mid = 0.5 * (lo + hi);
    let half = (0.5 * (hi - lo)).max(math::ln(2.0)) * 1.1;
    let y = |v: &f64| r.center().y - ((ln(v) - mid) / half) as f32 * r.height() * 0.5;
    curves(b, r, s, base, set, y);
}

fn curves(b: &mut Brush, r: Rect, s: &Style, base: &[f64], set: &[f64], y: impl Fn(&f64) -> f32) {
    let n = base.len().max(2);
    let x = |i: usize| r.left() + i as f32 / (n - 1) as f32 * r.width();
    // Every C an octave line, middle C stronger.
    for (i, m) in KEYS.enumerate() {
        if m % 12 == 0 {
            let c = if m == 60 { s.t.grid_strong } else { s.t.grid };
            b.vline(x(i), r.top(), r.bottom(), b.px(), c);
        }
    }
    let pts = |v: &[f64]| -> Vec<Pos2> {
        v.iter()
            .enumerate()
            .map(|(i, v)| Pos2::new(x(i), y(v).clamp(r.top(), r.bottom())))
            .collect()
    };
    b.dashed(&pts(base), s.s(1.5), s.t.text_dim, s.s(4.0), s.s(3.0));
    b.polyline(&pts(set), s.s(2.0), s.hue);
}

/// A response in dB over `lo … hi` Hz (log), `base` dashed and `set` in the hue, the 0 dB
/// line drawn, between `floor` and `top` dB.
#[allow(clippy::too_many_arguments)]
fn response(
    b: &mut Brush,
    r: Rect,
    s: &Style,
    (lo, hi): (f64, f64),
    (floor, top): (f64, f64),
    base: impl Fn(f64) -> f64,
    set: impl Fn(f64) -> f64,
) {
    const N: usize = 64;
    let f_of = |i: usize| {
        math::exp(math::ln(lo) + (math::ln(hi) - math::ln(lo)) * i as f64 / (N - 1) as f64)
    };
    let y =
        |db: &f64| r.bottom() - ((db - floor) / (top - floor)).clamp(0.0, 1.0) as f32 * r.height();
    b.hline(r.left(), r.right(), y(&0.0), b.px(), s.t.grid_strong);
    // Decade lines (100 Hz, 1 kHz, 10 kHz) inside the span.
    let x_of = |f: f64| {
        r.left() + ((math::ln(f) - math::ln(lo)) / (math::ln(hi) - math::ln(lo))) as f32 * r.width()
    };
    for f in [100.0, 1_000.0, 10_000.0] {
        if f > lo && f < hi {
            b.vline(x_of(f), r.top(), r.bottom(), b.px(), s.t.grid);
        }
    }
    let line = |g: &dyn Fn(f64) -> f64| -> Vec<Pos2> {
        (0..N)
            .map(|i| {
                Pos2::new(
                    r.left() + i as f32 / (N - 1) as f32 * r.width(),
                    y(&g(f_of(i))),
                )
            })
            .collect()
    };
    b.dashed(&line(&base), s.s(1.5), s.t.text_dim, s.s(4.0), s.s(3.0));
    b.polyline(&line(&set), s.s(2.0), s.hue);
}

/// An analogue peaking filter's gain (dB) at `f`: the bass formant `[Hz, dB, Q]`.
fn peak_db(p: &[f64; 3], f: f64) -> f64 {
    if p[1] == 0.0 || p[0] <= 0.0 {
        return 0.0;
    }
    let a = math::powf(10.0f64, p[1] / 40.0);
    let q = p[2].max(0.1);
    let x = f / p[0];
    let d = (1.0 - x * x) * (1.0 - x * x);
    let num = d + (x * a / q) * (x * a / q);
    let den = d + (x / (a * q)) * (x / (a * q));
    10.0 * math::log10(num / den)
}

/// A second-order Butterworth low-pass's gain (dB) at `f` (flat when off, `hz` 0).
fn roll_db(hz: f64, f: f64) -> f64 {
    if hz <= 0.0 {
        return 0.0;
    }
    let x = f / hz;
    -10.0 * math::log10(1.0 + x * x * x * x)
}

/// The picture of `what` in the display `d`, its caption line above it.
pub fn paint(b: &mut Brush, d: Rect, s: &Style, what: Picture, p: &Pair) {
    let (line, room) = caption(d, s);
    let r = plot(b, room, s);
    let along = |f: &dyn Fn(&Voicing, u8) -> f64, v: &Voicing| -> Vec<f64> {
        KEYS.map(|m| f(v, m)).collect()
    };
    let at = |v: &[f64], m: u8| v[(m - 21) as usize];
    let text = match what {
        Picture::Hammers => {
            let hard = math::powf(4.0f64, p.hardness);
            let base = along(&|v, m| v.felt_of(m), &p.base);
            let set = along(&|v, m| v.felt_of(m) * hard, &p.set);
            keyboard_log(b, r, s, &base, &set);
            let k = |m| times(at(&set, m) / at(&base, m));
            tf(
                "chorda.v.line.hammers",
                &[("a0", &k(21)), ("c4", &k(60)), ("c8", &k(108))],
            )
        }
        Picture::Strings => {
            let base = along(&|v, m| v.decay_of(m), &p.base);
            let set = along(&|v, m| v.decay_of(m) * p.decay, &p.set);
            keyboard_log(b, r, s, &base, &set);
            let k = |m| times(at(&set, m) / at(&base, m));
            tf(
                "chorda.v.line.strings",
                &[("a0", &k(21)), ("c4", &k(60)), ("c8", &k(108))],
            )
        }
        Picture::Board => {
            let (fb, fs) = (p.base.formant, p.set.formant);
            response(
                b,
                r,
                s,
                (50.0, 5_000.0),
                (-12.0, 24.0),
                |f| peak_db(&fb, f),
                |f| peak_db(&fs, f),
            );
            tf(
                "chorda.v.line.board",
                &[
                    ("hz", &format!("{:.0}", fs[0])),
                    ("db", &format!("{:+.1}", fs[1])),
                    ("q", &format!("{:.1}", fs[2])),
                ],
            )
        }
        Picture::Action => {
            let base = along(&|v, m| v.damper_of(m), &p.base);
            let set = along(&|v, m| v.damper_of(m), &p.set);
            keyboard_log(b, r, s, &base, &set);
            let k = |m| times(at(&set, m) / at(&base, m));
            tf(
                "chorda.v.line.action",
                &[
                    ("a0", &k(21)),
                    ("c4", &k(60)),
                    ("floor", &format!("{:.0}", -p.set.damped_db)),
                ],
            )
        }
        Picture::Mics => {
            let ms = |v: &Voicing, m: u8| v.lag_of(m, 48_000.0) as f64 / 48.0;
            let base = along(&ms, &p.base);
            let set = along(&ms, &p.set);
            let span = base.iter().chain(&set).fold(0.5f64, |a, v| a.max(v.abs())) * 1.1;
            b.hline(r.left(), r.right(), r.center().y, b.px(), s.t.grid_strong);
            curves(b, r, s, &base, &set, |v| {
                r.center().y - (v / span) as f32 * r.height() * 0.5
            });
            tf(
                "chorda.v.line.mics",
                &[
                    ("a0", &format!("{:+.2}", at(&set, 21))),
                    ("c8", &format!("{:+.2}", at(&set, 108))),
                ],
            )
        }
        Picture::Tone => {
            let (hb, hs) = (p.base.top_hz, p.set.top_hz);
            response(
                b,
                r,
                s,
                (1_000.0, 20_000.0),
                (-24.0, 3.0),
                |f| roll_db(hb, f),
                |f| roll_db(hs, f),
            );
            let hz = if hs >= 1_000.0 {
                format!("{:.1} kHz", hs / 1_000.0)
            } else {
                format!("{hs:.0} Hz")
            };
            tf(
                "chorda.v.line.tone",
                &[("hz", &hz), ("tilt", &format!("{:+.1}", p.tilt))],
            )
        }
    };
    label(b, line, &text, Role::Label, s.t.text, s);
}
