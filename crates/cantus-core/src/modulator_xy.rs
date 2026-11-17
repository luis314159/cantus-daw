//! The X-Y pad (ADR 0147): a puck on a square whose two axes are two outputs, each driving
//! any number of parameters, with inertia and looping paths.
//!
//! **Where the puck heads.** Its *target* is the pad's own X and Y (0…1 each — what the
//! hand sets, automation on `AutoTarget::ModInput` replays, a MIDI controller moves), or,
//! while the song plays and the path is switched on, the **path**: a closed loop of
//! points spaced evenly in time over `beats`, read as a function of the song position (a
//! render from bar 5 is bars 5… of a full one).
//!
//! **Inertia** is a critically damped follow — two equal one-pole stages, so the puck
//! starts gently, never overshoots and settles — with time constant `inertia_ms`. It is
//! state, stepped by the engine at each control point over the exact interval to the next
//! (the closed form of the two stages for a target held over it), on the 16-sample grid
//! while playing: the same at every block size, offline as live. 0 ms is no state at all:
//! the puck is the target.
//!
//! **Outputs.** Each axis is bipolar by default (the centre is the parameter's own value,
//! as Bitwig's modulators are) or unipolar (left and bottom are); a route's own range and
//! curve (`ModRoute::range`) make an axis FL's way: this far to that far, bent.

use serde::{Deserialize, Serialize};

/// Points a path holds at most.
pub const MAX_PATH: usize = 256;
/// The longest inertia, in milliseconds.
pub const MAX_INERTIA_MS: f32 = 10_000.0;
/// A path's loop, in beats, at its shortest and longest.
pub const MIN_PATH_BEATS: f32 = 0.25;
pub const MAX_PATH_BEATS: f32 = 256.0;

/// A looping gesture: points evenly spaced over `beats`, the last joined to the first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XyPath {
    /// X and Y, 0…1 each; 2 to [`MAX_PATH`].
    pub points: Vec<[f32; 2]>,
    /// The loop's length in beats.
    pub beats: f32,
    /// A smooth curve through the points (Catmull–Rom) instead of straight lines.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub smooth: bool,
}

impl XyPath {
    pub fn is_valid(&self) -> bool {
        (2..=MAX_PATH).contains(&self.points.len())
            && self
                .points
                .iter()
                .all(|p| p.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)))
            && self.beats.is_finite()
            && (MIN_PATH_BEATS..=MAX_PATH_BEATS).contains(&self.beats)
    }

    /// Where the path is at song position `beats`.
    pub fn at(&self, beats: f64) -> [f32; 2] {
        let n = self.points.len();
        if n == 0 {
            return [0.5, 0.5];
        }
        let len = f64::from(self.beats.max(MIN_PATH_BEATS));
        let phase = (beats / len).rem_euclid(1.0) * n as f64;
        let i = (phase.floor() as usize).min(n - 1);
        let u = (phase - i as f64) as f32;
        let p = |k: usize| self.points[k % n];
        let (a, b) = (p(i), p(i + 1));
        let mut out = [0.0f32; 2];
        for (d, o) in out.iter_mut().enumerate() {
            *o = if self.smooth {
                let (p0, p3) = (p(i + n - 1)[d], p(i + 2)[d]);
                catmull_rom(p0, a[d], b[d], p3, u)
            } else {
                a[d] + (b[d] - a[d]) * u
            }
            .clamp(0.0, 1.0);
        }
        out
    }

    /// A gesture recorded as `(beat, x, y)` samples, resampled to `points` evenly spaced
    /// over `beats` from `start` (each point the latest sample at or before its time; the
    /// first sample before any). `None` without samples.
    pub fn from_gesture(
        samples: &[(f64, f32, f32)],
        start: f64,
        beats: f32,
        points: usize,
    ) -> Option<XyPath> {
        let first = samples.first()?;
        let n = points.clamp(2, MAX_PATH);
        let beats = beats.clamp(MIN_PATH_BEATS, MAX_PATH_BEATS);
        let mut k = 0usize;
        let mut cur = (first.1, first.2);
        let pts = (0..n)
            .map(|j| {
                let t = start + f64::from(beats) * j as f64 / n as f64;
                while k < samples.len() && samples[k].0 <= t {
                    cur = (samples[k].1, samples[k].2);
                    k += 1;
                }
                [cur.0.clamp(0.0, 1.0), cur.1.clamp(0.0, 1.0)]
            })
            .collect();
        Some(XyPath {
            points: pts,
            beats,
            smooth: true,
        })
    }
}

#[inline]
fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, u: f32) -> f32 {
    let u2 = u * u;
    let u3 = u2 * u;
    0.5 * (2.0 * p1
        + (p2 - p0) * u
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2
        + (3.0 * (p1 - p2) + p3 - p0) * u3)
}

/// The X-Y pad.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XySource {
    /// Where the hand left the puck, 0…1 each (the centre at rest).
    pub x: f32,
    pub y: f32,
    /// Left and bottom are the parameters' own values (else the centre is).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unipolar: bool,
    /// How long the puck takes to follow (the time constant of each stage), ms.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub inertia_ms: f32,
    /// A looping path, kept while switched off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<XyPath>,
    /// The path drives the puck while the song plays.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub play_path: bool,
    /// Its routes' axes and ranges, by target (a route without one is the X axis as it is).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<super::RouteLink>,
    /// The face's colourway (0 the first; the face's own list, `fxd.xy.look.*`).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub look: u8,
}

fn is_zero_u8(x: &u8) -> bool {
    *x == 0
}

/// Colourways a controller's face offers.
pub const LOOKS: u8 = 3;

fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

impl Default for XySource {
    fn default() -> Self {
        XySource {
            x: 0.5,
            y: 0.5,
            unipolar: false,
            inertia_ms: 0.0,
            path: None,
            play_path: false,
            links: Vec::new(),
            look: 0,
        }
    }
}

impl XySource {
    pub fn is_valid(&self) -> bool {
        let unit = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        unit(self.x)
            && unit(self.y)
            && self.inertia_ms.is_finite()
            && (0.0..=MAX_INERTIA_MS).contains(&self.inertia_ms)
            && self.path.as_ref().is_none_or(XyPath::is_valid)
            && super::links_valid(&self.links, 2)
            && self.look < LOOKS
    }

    /// Where the puck heads at song position `beats`: the path while it plays, else the
    /// pad's X and Y (`inputs`: where automation or a controller moves them).
    pub fn target(&self, beats: f64, playing: bool, inputs: [f32; 2]) -> [f32; 2] {
        match &self.path {
            Some(p) if self.play_path && playing => p.at(beats),
            _ => inputs,
        }
    }

    /// An axis position as the output (bipolar −1…1 unless unipolar).
    #[inline]
    pub fn polar(&self, u: f32) -> f32 {
        if self.unipolar {
            u
        } else {
            2.0 * u - 1.0
        }
    }

    /// Whether its puck carries state (inertia).
    pub fn has_inertia(&self) -> bool {
        self.inertia_ms > 0.0
    }
}

/// The puck's inertia: two equal one-pole stages per axis (critically damped).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Puck {
    pub stage1: [f32; 2],
    pub pos: [f32; 2],
    /// Placed on its first target (playback starts with the puck where it heads).
    pub placed: bool,
}

impl Puck {
    /// Step `dt` seconds towards `target` held over them, time constant `tau_ms`: the
    /// exact response of the two stages. A puck on its target stays there to the bit.
    pub fn step(&mut self, target: [f32; 2], tau_ms: f32, dt: f64) {
        if !self.placed || tau_ms <= 0.0 {
            self.stage1 = target;
            self.pos = target;
            self.placed = true;
            return;
        }
        let x = dt / (f64::from(tau_ms) * 1e-3);
        let a = cantus_dsp::math::exp(-x);
        for (d, &goal) in target.iter().enumerate() {
            let t = f64::from(goal);
            let e1 = f64::from(self.stage1[d]) - t;
            let e2 = f64::from(self.pos[d]) - t;
            self.stage1[d] = (t + e1 * a) as f32;
            self.pos[d] = (t + (e2 + e1 * x) * a) as f32;
        }
    }
}
