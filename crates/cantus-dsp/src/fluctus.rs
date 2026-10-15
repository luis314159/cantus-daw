//! Fluctus: the volume shaper (ADR 0055) — LFO Tool and Kickstart's job: the level follows
//! a **drawn curve** that repeats in time with the song, the sidechain pump without a key.
//!
//! - **The curve** is up to sixteen points across one cycle, each with the segment that
//!   starts there shaped as the automation editor shapes it (its six: curve, S-curve,
//!   hold, stairs, pulse, wave, with their tension) — the shape function is handed in by
//!   the engine (`cantus_core::automation::Shape::at`), so the editor, the automation and
//!   Fluctus draw a segment the same way. The curve wraps: the last point runs to the
//!   first one a cycle later.
//! - **The cycle** is a note value from 4/1 to 1/16, **a function of the song position**
//!   (the family's clock): a render from bar 5 hears what bar 5 of the full render hears.
//! - **Smoothing**: a one-pole glide on the level (0 … 50 ms), so a step — a hold's jump,
//!   the wrap from full to nothing — never clicks.
//! - **Depth**: the level is `1 − depth·(1 − curve)`; at depth 0 (the default) every sample
//!   passes untouched.
//!
//! The level depends on the song position alone, never on the audio, so silence in gives
//! silence out; `skip` runs the level's glide exactly as processing silence would.

use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate};
use crate::modfx_line::Glide;

/// How many points the curve may hold.
pub const POINTS: usize = 16;

/// One point of the curve.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CurvePoint {
    /// Where in the cycle, 0 … 1.
    pub x: f32,
    /// The level, 0 … 1.
    pub y: f32,
    /// The tension of the segment that starts here, −1 … 1.
    pub tension: f32,
    /// The segment's shape (the automation editor's, by index).
    pub shape: u8,
}

/// How far (0 … 1) a segment of `shape` with `tension` is at `u` (0 … 1) of the way.
pub type ShapeFn = fn(u8, f32, f32) -> f32;

/// A plain line, for a unit built without the automation's shapes (tests, tools).
pub fn linear(_shape: u8, _tension: f32, u: f32) -> f32 {
    u
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FluctusSettings {
    /// 0 … 1.
    pub depth: f32,
    /// Beats in a cycle.
    pub beats: f64,
    /// Where in the song's cycle the curve starts, 0 … 1.
    pub offset: f32,
    pub smooth_ms: f32,
    pub out_db: f32,
    pub points: [Option<CurvePoint>; POINTS],
}

impl Default for FluctusSettings {
    fn default() -> Self {
        let mut points = [None; POINTS];
        points[0] = Some(CurvePoint {
            x: 0.0,
            y: 0.0,
            tension: -0.6,
            shape: 0,
        });
        points[1] = Some(CurvePoint {
            x: 0.5,
            y: 1.0,
            tension: 0.0,
            shape: 2,
        });
        FluctusSettings {
            depth: 0.0,
            beats: 1.0,
            offset: 0.0,
            smooth_ms: 2.0,
            out_db: 0.0,
            points,
        }
    }
}

/// The curve, its points sorted by place: what is evaluated per sample.
#[derive(Copy, Clone, Debug)]
pub struct Curve {
    pts: [CurvePoint; POINTS],
    n: usize,
    shape: ShapeFn,
}

impl Curve {
    pub fn new(points: &[Option<CurvePoint>; POINTS], shape: ShapeFn) -> Curve {
        let mut pts = [CurvePoint::default(); POINTS];
        let mut n = 0;
        for p in points.iter().flatten() {
            pts[n] = CurvePoint {
                x: p.x.clamp(0.0, 1.0),
                y: p.y.clamp(0.0, 1.0),
                ..*p
            };
            n += 1;
        }
        // Sorted by place, equal places in their numbers' order: an insertion sort, which
        // allocates nothing (this runs on the audio thread).
        for i in 1..n {
            let mut j = i;
            while j > 0 && pts[j - 1].x > pts[j].x {
                pts.swap(j - 1, j);
                j -= 1;
            }
        }
        Curve { pts, n, shape }
    }

    /// The points, sorted.
    pub fn points(&self) -> &[CurvePoint] {
        &self.pts[..self.n]
    }

    /// The level at `phase` (0 … 1): 1 with no point.
    pub fn at(&self, phase: f32) -> f32 {
        let n = self.n;
        if n == 0 {
            return 1.0;
        }
        let p = &self.pts[..n];
        // The segment that holds `phase`: from the last point at or before it, wrapping
        // from the last point to the first one a cycle on.
        let i = p.partition_point(|q| q.x <= phase);
        let (a, b, x0, x1) = if i == 0 {
            (p[n - 1], p[0], p[n - 1].x - 1.0, p[0].x)
        } else if i == n {
            (p[n - 1], p[0], p[n - 1].x, p[0].x + 1.0)
        } else {
            (p[i - 1], p[i], p[i - 1].x, p[i].x)
        };
        let span = x1 - x0;
        if span <= 0.0 {
            return b.y;
        }
        let u = ((phase - x0) / span).clamp(0.0, 1.0);
        a.y + (b.y - a.y) * (self.shape)(a.shape, a.tension, u)
    }
}

const SNAP: f32 = 1e-6;

/// Fluctus's own telemetry: the level now (0 … 1).
pub const T_LEVEL: usize = tel::OWN;

#[derive(Clone, Debug)]
pub struct Fluctus {
    sr: f32,
    set: FluctusSettings,
    clock: Clock,
    lfo: Lfo,
    curve: Curve,
    shape: ShapeFn,
    /// The level's glide (the smoothing).
    level: f32,
    k: f32,
    depth: Glide,
    out: Glide,
    tel: [f32; 16],
    blocks: u32,
}

impl Fluctus {
    /// A shaper whose segments take `shape` (the automation editor's `Shape::at`).
    pub fn new(sample_rate: f32, shape: ShapeFn) -> Fluctus {
        let set = FluctusSettings::default();
        let mut f = Fluctus {
            sr: sample_rate,
            set,
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            curve: Curve::new(&set.points, shape),
            shape,
            level: 1.0,
            k: 1.0,
            depth: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            tel: [0.0; 16],
            blocks: 0,
        };
        f.set(&set);
        f.reset();
        f
    }

    pub fn settings(&self) -> &FluctusSettings {
        &self.set
    }

    pub fn curve(&self) -> &Curve {
        &self.curve
    }

    pub fn set(&mut self, s: &FluctusSettings) {
        self.curve = Curve::new(&s.points, self.shape);
        let n = s.smooth_ms.clamp(0.0, 50.0) * 0.001 * self.sr;
        self.k = if n < 1.0 {
            1.0
        } else {
            1.0 - crate::math::exp(-1.0 / n)
        };
        self.depth.target = s.depth.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        let lfo = LfoSettings {
            rate: Rate::Beats(s.beats.max(1e-3)),
            offset: f64::from(s.offset.clamp(0.0, 1.0)),
            ..LfoSettings::default()
        };
        self.lfo.set(&lfo, &self.clock);
        self.set = *s;
    }

    pub fn set_transport(&mut self, t: &Transport) {
        let epoch = self.clock.epoch();
        self.clock.set(t);
        self.lfo.follow(&self.clock);
        if self.clock.epoch() != epoch {
            // Playback starts on the curve where the song is, as the full render is there:
            // no glide from wherever the level was while stopped. (A seek or a loop's wrap
            // while playing glides, so it never clicks.)
            let d = self.depth.value;
            self.level = if d == 0.0 {
                1.0
            } else {
                1.0 - d * (1.0 - self.curve.at(self.phase(0)))
            };
        }
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        self.depth.jump(self.depth.target);
        self.out.jump(self.out.target);
        self.level = 1.0 - self.depth.value * (1.0 - self.curve.at(self.phase(0)));
        self.tel = [0.0; 16];
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    /// At rest: the depth and the gain landed. The level follows the song alone, never the
    /// audio, and [`Fluctus::skip`] runs it as processing would.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        (!self.depth.still() || !self.out.still()).then_some("a glide is moving")
    }

    /// The curve's phase `i` samples into the next call.
    #[inline]
    fn phase(&self, i: usize) -> f32 {
        self.lfo.phase(&self.clock, i).0 as f32
    }

    /// The gain for sample `i` of the next call; advances the level's glide.
    #[inline]
    fn step(&mut self, i: usize) -> f32 {
        let depth = self.depth.next();
        let out = self.out.next();
        let target = if depth == 0.0 {
            1.0
        } else {
            1.0 - depth * (1.0 - self.curve.at(self.phase(i)))
        };
        let next = self.level + (target - self.level) * self.k;
        // Landed, or stalled (the step rounds away in f32, a few millionths short of the
        // target): on the target, so a held level is exactly the curve's.
        self.level = if next == self.level || (next - target).abs() < 1e-7 {
            target
        } else {
            next
        };
        self.level * out
    }

    /// `frames` of silence skipped: the level glides as it would have.
    pub fn skip(&mut self, frames: usize) {
        for i in 0..frames {
            let _ = self.step(i);
        }
        self.clock.skip(frames);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        for i in 0..n {
            let g = self.step(i);
            if g != 1.0 {
                l[i] *= g;
                r[i] *= g;
            }
        }
        let ph = self.phase(n.saturating_sub(1));
        self.clock.advance(n);
        self.blocks = (self.blocks + 1) % (1 << 24);
        self.tel[tel::SAMPLE_RATE] = self.sr;
        self.tel[tel::TEMPO] = self.clock.tempo() as f32;
        self.tel[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        self.tel[tel::PHASE_L] = ph;
        self.tel[tel::PHASE_R] = ph;
        self.tel[tel::MOD_L] = 2.0 * self.level - 1.0;
        self.tel[tel::MOD_R] = self.tel[tel::MOD_L];
        self.tel[tel::IN_PEAK] = peak_db(in_peak);
        self.tel[tel::OUT_PEAK] = peak_db(peak_of(&l[..n], &r[..n]));
        self.tel[tel::BLOCKS] = self.blocks as f32;
        self.tel[T_LEVEL] = self.level;
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "fluctus_tests.rs"]
mod tests;
