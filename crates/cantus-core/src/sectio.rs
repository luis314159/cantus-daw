//! Sectio — Exemplar's slicer (round 37, ADR 0099): a loop cut at its slice points, one
//! slice per key from C1, each slice a zone of the same file. FL Studio's Slicex and Fruity
//! Slicer, Ableton Simpler's slice mode, Logic's Quick Sampler, Bitwig's *Sliced*.
//!
//! The slice points live in the Exemplar's [`NativeSpec`](crate::NativeSpec) beside its
//! zones (schema 55), so a project keeps them and the editor can move one and rebuild the
//! zones — every edit a command with an exact inverse:
//!
//! | Key | What |
//! |---|---|
//! | `sl.n` | how many slices (0: not sliced) |
//! | `sl.p{i}` | where slice `i` starts, in the file's frames (`sl.p0` is the first) |
//! | `sl.len`, `sl.rate` | the file's length (frames) and sample rate |
//! | `sl.bpm` | the loop's tempo (from its ACID chunk, its grid, or the user; 0 unknown) |
//! | `sl.mode` | how the points were found: 0 markers, 1 transients, 2 grid, 3 by hand |
//! | `sl.sens` | the transients' sensitivity (0 … 1) |
//! | `sl.grid` | the grid's steps per beat (1, 2, 4, 8) |
//! | `sl.style` | the slicing style applied (1-based; 0 none) |
//!
//! Slice `i` is zone `z{i}`: key C1 + `i`, its own root (it plays at its pitch), its own
//! group `g{i}` (its envelope), from `sl.p{i}` to the next point. What a slice carries of
//! its own — pitch, gain, pan, reverse, choke group, low-pass — is its zone's.

use crate::effect::p;
use crate::inst::sampler::{zf, Zone, MAX_GROUPS};
use crate::inst::InstParamInfo;

/// The key slice 0 plays on (C1), and the most slices a loop takes (one group each).
pub const FIRST_KEY: u8 = 36;
pub const MAX_SLICES: usize = MAX_GROUPS;

/// How the slice points were found.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SliceMode {
    /// The file's own markers (WAV `cue `, `smpl` loops' starts, an ACID file's beats).
    Markers,
    /// The transients (SuperFlux onsets above the sensitivity).
    Transients,
    /// A beat grid (the loop's tempo, `sl.grid` steps a beat).
    Grid,
    /// Placed, moved or removed by hand.
    Manual,
}

impl SliceMode {
    pub fn from_index(v: f32) -> SliceMode {
        match v.round() as i32 {
            0 => SliceMode::Markers,
            2 => SliceMode::Grid,
            3 => SliceMode::Manual,
            _ => SliceMode::Transients,
        }
    }
    pub fn index(self) -> f32 {
        match self {
            SliceMode::Markers => 0.0,
            SliceMode::Transients => 1.0,
            SliceMode::Grid => 2.0,
            SliceMode::Manual => 3.0,
        }
    }
    pub fn name_key(self) -> &'static str {
        match self {
            SliceMode::Markers => "slicer.mode.markers",
            SliceMode::Transients => "slicer.mode.transients",
            SliceMode::Grid => "slicer.mode.grid",
            SliceMode::Manual => "slicer.mode.manual",
        }
    }
}

/// The slicer's own numbers.
pub const PARAMS: &[InstParamInfo] = &[
    p(
        "sl.n",
        "slicer.count",
        0.0,
        MAX_SLICES as f32,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "sl.len",
        "slicer.len",
        0.0,
        16_777_215.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "sl.rate",
        "slicer.rate",
        0.0,
        384_000.0,
        0.0,
        "",
        false,
        true,
    ),
    p("sl.bpm", "slicer.bpm", 0.0, 999.0, 0.0, "", false, false),
    p("sl.mode", "slicer.mode", 0.0, 3.0, 1.0, "", false, true),
    p("sl.sens", "slicer.sens", 0.0, 1.0, 0.5, "%", false, false),
    p("sl.grid", "slicer.grid", 1.0, 8.0, 4.0, "", false, true),
    p("sl.style", "slicer.style", 0.0, 16.0, 0.0, "", false, true),
];

/// A slice point's template (`sl.p{i}`).
const POINT: InstParamInfo = p(
    "sl.p",
    "slicer.point",
    0.0,
    16_777_215.0,
    0.0,
    "",
    false,
    true,
);

/// `sl.p{i}` → `i`.
pub fn point_key(key: &str) -> Option<usize> {
    let rest = key.strip_prefix("sl.p")?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if rest.len() > 1 && rest.starts_with('0') {
        return None;
    }
    let i: usize = rest.parse().ok()?;
    (i < MAX_SLICES).then_some(i)
}

/// The slicer's parameter for `key`, if it is one.
pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    if point_key(key).is_some() {
        return Some(&POINT);
    }
    PARAMS.iter().find(|i| i.key == key)
}

/// A loop's slicing.
#[derive(Clone, Debug, PartialEq)]
pub struct Slicing {
    pub mode: SliceMode,
    pub sens: f32,
    pub bpm: f32,
    pub grid: u8,
    /// The file's length (frames) and rate.
    pub len: u32,
    pub rate: u32,
    /// Where each slice starts, strictly rising, the first ≥ 0, every one < `len`.
    pub points: Vec<u32>,
}

impl Slicing {
    /// Points sorted, deduplicated, inside the file, at most [`MAX_SLICES`].
    pub fn tidy(mut self) -> Slicing {
        self.points.retain(|p| *p < self.len.max(1));
        self.points.sort_unstable();
        self.points.dedup();
        self.points.truncate(MAX_SLICES);
        self
    }

    /// Slice `i`'s frames: `[start, end)`.
    pub fn span(&self, i: usize) -> (u32, u32) {
        let start = self.points[i];
        let end = self.points.get(i + 1).copied().unwrap_or(self.len);
        (start, end)
    }

    /// Read from an Exemplar's spec; `None` when it is not sliced.
    pub fn of(spec: &crate::NativeSpec) -> Option<Slicing> {
        let n = spec.get("sl.n").round().max(0.0) as usize;
        if n == 0 {
            return None;
        }
        let points = (0..n.min(MAX_SLICES))
            .map(|i| spec.get(&format!("sl.p{i}")).max(0.0) as u32)
            .collect();
        Some(Slicing {
            mode: SliceMode::from_index(spec.get("sl.mode")),
            sens: spec.get("sl.sens"),
            bpm: spec.get("sl.bpm"),
            grid: spec.get("sl.grid").round().clamp(1.0, 8.0) as u8,
            len: spec.get("sl.len").max(0.0) as u32,
            rate: spec.get("sl.rate").max(0.0) as u32,
            points,
        })
    }

    /// The values that store it (`sl.*`).
    pub fn values(&self) -> Vec<(String, f32)> {
        let mut v = vec![
            ("sl.n".to_string(), self.points.len() as f32),
            ("sl.len".to_string(), self.len as f32),
            ("sl.rate".to_string(), self.rate as f32),
            ("sl.bpm".to_string(), self.bpm),
            ("sl.mode".to_string(), self.mode.index()),
            ("sl.sens".to_string(), self.sens),
            ("sl.grid".to_string(), self.grid as f32),
        ];
        for (i, p) in self.points.iter().enumerate() {
            v.push((format!("sl.p{i}"), *p as f32));
        }
        v
    }

    /// The zones that play it: slice `i` on key C1 + `i`, at its own pitch, in group `i`.
    pub fn zones(&self, sample: &str) -> Vec<Zone> {
        (0..self.points.len())
            .map(|i| {
                let key = FIRST_KEY + i as u8;
                let (start, end) = self.span(i);
                let mut z = Zone::new(Some(sample.to_string()), key, key, key);
                z.set(zf::START, start as f32)
                    .set(zf::END, end as f32)
                    .set(zf::GROUP, i as f32);
                z
            })
            .collect()
    }

    /// Where slice `i` falls on a timeline at the loop's tempo, in ticks from the loop's
    /// start (rounded to the nearest tick).
    pub fn tick_of(&self, frame: u32) -> i64 {
        tick_of_frame(frame, self.rate, self.bpm as f64)
    }
}

/// A frame of a file at `rate` played at `bpm`, as a tick from its start (rounded).
pub fn tick_of_frame(frame: u32, rate: u32, bpm: f64) -> i64 {
    if rate == 0 || bpm <= 0.0 {
        return 0;
    }
    let beats = frame as f64 / rate as f64 * bpm / 60.0;
    (beats * crate::time::PPQ as f64).round() as i64
}

/// One note of a dumped clip: tick, length (ticks), key, velocity.
pub type DumpNote = (i64, i64, u8, u8);

/// The notes that replay the loop slice by slice at `ticks` (each slice's start tick, from
/// [`Slicing::tick_of`], and the loop's end tick last): slice `i` on its key, until the
/// next starts, at full velocity.
pub fn dump(ticks: &[i64]) -> Vec<DumpNote> {
    ticks
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[1] > w[0])
        .map(|(i, w)| (w[0], w[1] - w[0], FIRST_KEY + i as u8, 127))
        .collect()
}
