//! Cribrum's movement: the LFO's waves (the modulation family's six and a drift), the
//! follower's level as a modulation, and the drawn curve smoothed as a function of the song
//! position alone.
//!
//! **Every source but the follower is a function of the clock** (ADR 0055): the LFO's phase
//! is the song's (synced) or the playback's (free), the curve's is the song's, and the
//! curve's smoothing is not a glide with a memory but a window over the curve itself —
//! nine points of it across `smooth` milliseconds, Hann-weighted and centred, read on a
//! grid of the song whose step is the points' spacing and drawn straight between, so a step
//! of the curve becomes a raised slope (an S of eight straight pieces) that starts before
//! it and ends after it. So a render from bar 5 moves the
//! filter as bar 5 of the full render does, at every block size, and a unit that slept
//! through silence is where one that processed it would be.

use crate::fluctus::{Curve, CurvePoint, ShapeFn, POINTS};
use crate::math;
use crate::modfx_clock::{random_bipolar, synced_inc, DIVISIONS as FAMILY};
use crate::modfx_lfo::{self, Shape};

/// Beats in a cycle of each synced length (`cribrum.div.{i}`): the family's seventeen
/// (8/1 … 1/32), then 1/32 triplet and 1/64.
pub const DIVISIONS: [f64; 19] = [
    FAMILY[0],
    FAMILY[1],
    FAMILY[2],
    FAMILY[3],
    FAMILY[4],
    FAMILY[5],
    FAMILY[6],
    FAMILY[7],
    FAMILY[8],
    FAMILY[9],
    FAMILY[10],
    FAMILY[11],
    FAMILY[12],
    FAMILY[13],
    FAMILY[14],
    FAMILY[15],
    FAMILY[16],
    1.0 / 12.0,
    0.0625,
];

/// The beats of length `index` (clamped into the table; 1/4 if it is not a number).
pub fn division_beats(index: f32) -> f64 {
    let i = if index.is_finite() {
        index.round().clamp(0.0, (DIVISIONS.len() - 1) as f32) as usize
    } else {
        8
    };
    DIVISIONS[i]
}

/// The LFO's waves (`cribrum.shape.{i}`): the family's six, then a drift.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Wave {
    #[default]
    Sine,
    Triangle,
    Square,
    SawUp,
    SawDown,
    /// Sample and hold: a new value each cycle, a hash of the seed and the cycle.
    Hold,
    /// The same values joined by a raised cosine: a wandering, never a step.
    Drift,
}

impl Wave {
    pub const ALL: [Wave; 7] = [
        Wave::Sine,
        Wave::Triangle,
        Wave::Square,
        Wave::SawUp,
        Wave::SawDown,
        Wave::Hold,
        Wave::Drift,
    ];

    pub fn from_index(i: f32) -> Wave {
        let i = if i.is_finite() { i.round() as i64 } else { 0 };
        Wave::ALL[i.clamp(0, Wave::ALL.len() as i64 - 1) as usize]
    }

    /// The family's shape that draws it (the drift has none).
    pub fn family(self) -> Option<Shape> {
        Some(match self {
            Wave::Sine => Shape::Sine,
            Wave::Triangle => Shape::Triangle,
            Wave::Square => Shape::Square,
            Wave::SawUp => Shape::SawUp,
            Wave::SawDown => Shape::SawDown,
            Wave::Hold => Shape::Random,
            Wave::Drift => return None,
        })
    }
}

/// The wave at `phase` (0 … 1) of cycle `cycle`: −1 … 1. The editor draws with it too.
pub fn wave(w: Wave, phase: f64, cycle: i64, edge: f32, seed: u64) -> f32 {
    match w.family() {
        Some(s) => modfx_lfo::wave(s, phase, cycle, edge, seed),
        None => {
            let p = (phase - phase.floor()) as f32;
            let a = random_bipolar(seed, cycle);
            let b = random_bipolar(seed, cycle + 1);
            a + (b - a) * (0.5 - 0.5 * math::cos(std::f32::consts::PI * p))
        }
    }
}

/// The follower's level (linear, after its sensitivity) as a modulation: 0 at −48 dBFS
/// and below, 1 at 0 dBFS and above, straight in decibels between — so a quiet passage
/// and a loud one both move the filter.
#[inline]
pub fn env_amount(level: f32) -> f32 {
    if level <= 0.003_981 {
        // −48 dB.
        return 0.0;
    }
    ((20.0 * math::log10(level) + ENV_FLOOR_DB) / ENV_FLOOR_DB).clamp(0.0, 1.0)
}

/// Where the follower's modulation starts, dB below full scale.
pub const ENV_FLOOR_DB: f32 = 48.0;

/// The smoothing window's points.
const TAPS: usize = 9;

/// The drawn curve as the filter reads it: smoothed, on the song.
#[derive(Copy, Clone, Debug)]
pub struct CurveMod {
    curve: Curve,
    sr: f64,
    beats: f64,
    /// The smoothing window, samples.
    window: f64,
    /// The grid the smoothed curve is read on: the window's points' spacing, samples of
    /// the song.
    grid: i64,
    /// The grid's two last readings: (grid sample, value at it, tempo bits), a memo of a
    /// pure function (it changes no value, only how often one is computed).
    memo: [(i64, f32, u64); 2],
}

/// The window's weights: a Hann window's nine inner points, about one in all.
const WEIGHTS: [f32; TAPS] = {
    // sin²(π(j+1)/10) for j = 0 … 8, over their sum (5).
    [
        0.095_491_5 / 5.0,
        0.345_491_5 / 5.0,
        0.654_508_5 / 5.0,
        0.904_508_5 / 5.0,
        1.0 / 5.0,
        0.904_508_5 / 5.0,
        0.654_508_5 / 5.0,
        0.345_491_5 / 5.0,
        0.095_491_5 / 5.0,
    ]
};

impl CurveMod {
    pub fn new(sample_rate: f32, shape: ShapeFn) -> CurveMod {
        CurveMod {
            curve: Curve::new(&[None; POINTS], shape),
            sr: f64::from(sample_rate),
            beats: 4.0,
            window: 0.0,
            grid: 1,
            memo: [(i64::MIN, 0.0, 0); 2],
        }
    }

    pub fn set(
        &mut self,
        points: &[Option<CurvePoint>; POINTS],
        shape: ShapeFn,
        beats: f64,
        smooth_ms: f32,
    ) {
        self.curve = Curve::new(points, shape);
        self.beats = beats.max(1e-3);
        let w = f64::from(smooth_ms.clamp(0.0, 50.0)) * 0.001 * self.sr;
        self.grid = ((w / (TAPS - 1) as f64).round() as i64).max(1);
        // The window spans the grid's points exactly.
        self.window = if w < 1.0 {
            0.0
        } else {
            (self.grid * (TAPS as i64 - 1)) as f64
        };
        self.memo = [(i64::MIN, 0.0, 0); 2];
    }

    pub fn curve(&self) -> &Curve {
        &self.curve
    }

    /// The curve's phase (0 … 1) at song sample `t`.
    #[inline]
    pub fn phase(&self, t: f64, tempo: f64) -> f64 {
        let c = t * synced_inc(self.beats, tempo, self.sr);
        c - c.floor()
    }

    /// The smoothed curve at song sample `t` (exactly the curve with no smoothing).
    pub fn smoothed(&self, t: i64, tempo: f64) -> f32 {
        if self.window < 1.0 {
            return self.curve.at(self.phase(t as f64, tempo) as f32);
        }
        // Over the weights' own sum, taken in the same order: a held level comes back
        // exactly.
        let (mut sum, mut norm) = (0.0f32, 0.0f32);
        for (j, w) in WEIGHTS.iter().enumerate() {
            let at = t + (j as i64 - (TAPS as i64 - 1) / 2) * self.grid;
            sum += w * self.curve.at(self.phase(at as f64, tempo) as f32);
            norm += w;
        }
        sum / norm
    }

    /// The value the filter takes at song sample `t`: the smoothed curve on the grid,
    /// straight between (exact, per sample, with no smoothing).
    #[inline]
    pub fn at(&mut self, t: i64, tempo: f64) -> f32 {
        if self.window < 1.0 {
            return self.smoothed(t, tempo);
        }
        let g = self.grid;
        let g0 = t.div_euclid(g) * g;
        let a = self.on_grid(g0, tempo);
        if t == g0 {
            return a;
        }
        let b = self.on_grid(g0 + g, tempo);
        a + (b - a) * ((t - g0) as f32 / g as f32)
    }

    #[inline]
    fn on_grid(&mut self, g: i64, tempo: f64) -> f32 {
        let bits = tempo.to_bits();
        for m in &self.memo {
            if m.0 == g && m.2 == bits {
                return m.1;
            }
        }
        let v = self.smoothed(g, tempo);
        self.memo[0] = self.memo[1];
        self.memo[1] = (g, v, bits);
        v
    }
}
