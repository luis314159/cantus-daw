//! **Per-note expression** (round 33, ADR 0080, schema 47): what an MPE controller — a
//! Seaboard, a LinnStrument, an Osmose — says about each note while it sounds, kept on the
//! note itself: its **pitch bend**, its **pressure** and its **timbre** (MPE's "slide",
//! CC 74), each a curve over the note's length.
//!
//! **Where a curve lives.** A point's `at` is ticks from the note's *start*, so moving,
//! transposing, quantising, copying or linking a note carries its expression with it and
//! no edit has to touch the curves. A point past the note's end is kept (shrinking a note
//! and growing it back is lossless, as for notes outside a clip's window); what is heard
//! stops where the note does, and the bend's last value is held into the release.
//!
//! **Values are integers, at MIDI 2.0's resolution**, so `Note` keeps deriving `Eq` and a
//! curve survives any number of saves bit for bit:
//!
//! - the bend in units of [`BEND_UNIT`] per semitone (2²⁴: ±128 semitones, finer than the
//!   25-bit fraction of MIDI 2.0's per-note *Pitch 7.25*, which it converts to exactly);
//! - pressure and timbre as 0…[`UNIT_MAX`] (`u32::MAX`), MIDI 2.0's 32-bit controllers —
//!   0 nothing, `UNIT_MAX` all the way.
//!
//! Between two points the value moves in a straight line; before the first point it is the
//! first point's value; after the last, the last's. A note with no curve of a kind sends
//! nothing of that kind (its instrument is not told anything: the null rule).

use serde::{Deserialize, Serialize};

/// Bend units per semitone.
pub const BEND_UNIT: i64 = 1 << 24;
/// The widest bend a curve holds, in semitones either way (MPE allows 96).
pub const MAX_BEND_SEMITONES: i64 = 127;
/// Pressure and timbre at full.
pub const UNIT_MAX: i64 = u32::MAX as i64;
/// The most points one curve keeps (a minute of dense playing, thinned, is far fewer).
pub const MAX_POINTS: usize = 4096;

/// One of a note's three expression dimensions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExprKind {
    /// Semitones from the note's pitch (MPE member pitch bend).
    Bend,
    /// 0…1 (MPE channel pressure on the note's member channel; poly aftertouch).
    Pressure,
    /// 0…1 (MPE slide: CC 74 on the member channel; CLAP brightness, VST3 brightness).
    Timbre,
}

impl ExprKind {
    pub const ALL: [ExprKind; 3] = [ExprKind::Bend, ExprKind::Pressure, ExprKind::Timbre];

    /// The stored value of `v` in the kind's own unit (semitones for the bend, 0…1 else),
    /// clamped to what a curve holds.
    pub fn store(self, v: f64) -> i64 {
        match self {
            ExprKind::Bend => {
                let m = (MAX_BEND_SEMITONES * BEND_UNIT) as f64;
                (v * BEND_UNIT as f64).round().clamp(-m, m) as i64
            }
            _ => (v * UNIT_MAX as f64).round().clamp(0.0, UNIT_MAX as f64) as i64,
        }
    }

    /// A stored value in the kind's own unit.
    pub fn read(self, raw: i64) -> f64 {
        match self {
            ExprKind::Bend => raw as f64 / BEND_UNIT as f64,
            _ => raw as f64 / UNIT_MAX as f64,
        }
    }

    /// The value a note rests at (no bend, no pressure, timbre in the middle as MPE says
    /// CC 74 rests at 64).
    pub fn rest(self) -> i64 {
        match self {
            ExprKind::Bend | ExprKind::Pressure => 0,
            // MIDI 2.0's centre, which CC 74 at 64 scales up to exactly.
            ExprKind::Timbre => 1 << 31,
        }
    }

    /// Clamp a stored value into the kind's range.
    pub fn clamp(self, raw: i64) -> i64 {
        match self {
            ExprKind::Bend => raw.clamp(
                -MAX_BEND_SEMITONES * BEND_UNIT,
                MAX_BEND_SEMITONES * BEND_UNIT,
            ),
            _ => raw.clamp(0, UNIT_MAX),
        }
    }

    /// The history's and the lane's name key (`mpe.kind.*`).
    pub fn key(self) -> &'static str {
        match self {
            ExprKind::Bend => "mpe.kind.bend",
            ExprKind::Pressure => "mpe.kind.pressure",
            ExprKind::Timbre => "mpe.kind.timbre",
        }
    }
}

/// A point of a curve: `at` ticks from the note's start, `value` in the kind's integer unit.
/// Stored as a pair (`[at, value]`) to keep a dense curve small in the file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExprPoint(pub i64, pub i64);

impl ExprPoint {
    pub fn at(self) -> i64 {
        self.0
    }
    pub fn value(self) -> i64 {
        self.1
    }
}

/// A note's expression: up to three curves. Held as `Note::expr`, `None` on an ordinary
/// note — which then serialises exactly as before schema 47.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteExpr {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bend: Vec<ExprPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pressure: Vec<ExprPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timbre: Vec<ExprPoint>,
}

impl NoteExpr {
    pub fn curve(&self, kind: ExprKind) -> &[ExprPoint] {
        match kind {
            ExprKind::Bend => &self.bend,
            ExprKind::Pressure => &self.pressure,
            ExprKind::Timbre => &self.timbre,
        }
    }

    pub fn curve_mut(&mut self, kind: ExprKind) -> &mut Vec<ExprPoint> {
        match kind {
            ExprKind::Bend => &mut self.bend,
            ExprKind::Pressure => &mut self.pressure,
            ExprKind::Timbre => &mut self.timbre,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bend.is_empty() && self.pressure.is_empty() && self.timbre.is_empty()
    }

    /// The expression as a note holds it: `None` when every curve is empty.
    pub fn boxed(self) -> Option<Box<NoteExpr>> {
        (!self.is_empty()).then(|| Box::new(self))
    }

    /// The stored value of `kind` at `at` ticks from the note's start, `None` when the
    /// note has no curve of that kind.
    pub fn raw_at(&self, kind: ExprKind, at: i64) -> Option<i64> {
        raw_at(self.curve(kind), at)
    }

    /// The value of `kind` at `at`, in its own unit (semitones, 0…1).
    pub fn value_at(&self, kind: ExprKind, at: i64) -> Option<f64> {
        self.raw_at(kind, at).map(|r| kind.read(r))
    }

    /// Whether the curves are well formed: sorted by strictly increasing `at`, values
    /// in range, at most [`MAX_POINTS`] each. A point may lie before the start or past the
    /// end: a trimmed note keeps what shapes its edges, so a split and a trim are exact.
    /// What a command checks before storing one.
    pub fn is_valid(&self) -> bool {
        ExprKind::ALL.iter().all(|&k| {
            let c = self.curve(k);
            c.len() <= MAX_POINTS
                && c.windows(2).all(|w| w[0].0 < w[1].0)
                && c.iter().all(|p| k.clamp(p.1) == p.1)
        })
    }

    /// The same expression, put right: sorted, one point per tick (the last), of the points
    /// before the note's start only the last (all that the start's value needs), values
    /// clamped, at most [`MAX_POINTS`].
    pub fn normalised(mut self) -> NoteExpr {
        for k in ExprKind::ALL {
            let c = self.curve_mut(k);
            c.sort_by_key(|p| p.0);
            let mut out: Vec<ExprPoint> = Vec::with_capacity(c.len());
            for p in c.iter() {
                let p = ExprPoint(p.0, k.clamp(p.1));
                match out.last_mut() {
                    Some(last) if last.0 == p.0 => *last = p,
                    _ => out.push(p),
                }
            }
            // Before the start, only the last point counts (it shapes the way in).
            let before = out.partition_point(|p| p.0 < 0);
            if before > 1 {
                out.drain(..before - 1);
            }
            out.truncate(MAX_POINTS);
            *c = out;
        }
        self
    }
}

/// The stored value of a curve at `at` (linear between points, held outside them).
pub fn raw_at(curve: &[ExprPoint], at: i64) -> Option<i64> {
    let first = curve.first()?;
    if at <= first.0 {
        return Some(first.1);
    }
    let i = curve.partition_point(|p| p.0 <= at);
    if i >= curve.len() {
        return Some(curve[curve.len() - 1].1);
    }
    let (a, b) = (curve[i - 1], curve[i]);
    if a.0 == at {
        return Some(a.1);
    }
    // Exact integer interpolation, rounded half away from zero.
    let span = i128::from(b.0 - a.0);
    let num = i128::from(b.1 - a.1) * i128::from(at - a.0);
    let q = if num >= 0 {
        (num + span / 2) / span
    } else {
        (num - span / 2) / span
    };
    Some(a.1 + q as i64)
}

/// Every note's expression put right as a command would accept it (a hand-edited file:
/// points out of order, values out of range, an empty record): normalised, or dropped
/// when nothing is left. Returns how many notes were repaired. Run on every load.
pub fn repair_project(project: &mut crate::model::Project) -> usize {
    let mut n = 0;
    for t in &mut project.tracks {
        let windows = t.comps.iter_mut().flat_map(|c| c.windows.iter_mut());
        // The session's clips too (ADR 0076).
        let slots = t
            .slots
            .iter_mut()
            .filter_map(|s| s.clip.as_mut().map(|sc| &mut sc.clip));
        for c in t.clips.iter_mut().chain(windows).chain(slots) {
            for note in c.notes.iter_mut() {
                let Some(e) = note.expr.as_deref() else {
                    continue;
                };
                if e.is_empty() || !e.is_valid() {
                    note.expr = e.clone().normalised().boxed();
                    n += 1;
                }
            }
        }
    }
    n
}

/// A note's expression with the curves of `kind` replaced (`None`: removed).
pub fn with_curve(
    expr: Option<&NoteExpr>,
    kind: ExprKind,
    curve: Vec<ExprPoint>,
) -> Option<Box<NoteExpr>> {
    let mut e = expr.cloned().unwrap_or_default();
    *e.curve_mut(kind) = curve;
    e.normalised().boxed()
}

#[path = "note_expr_ops.rs"]
pub mod ops;

#[path = "note_expr_cmd.rs"]
pub mod cmd;
pub use cmd::ExprEdit;

#[path = "note_expr_compile.rs"]
pub mod compile;

#[path = "note_expr_midi2.rs"]
pub mod midi2;

#[path = "note_expr_zones.rs"]
pub mod zones;

#[path = "note_expr_defaults.rs"]
pub mod defaults;

#[cfg(test)]
#[path = "note_expr_tests.rs"]
mod tests;
