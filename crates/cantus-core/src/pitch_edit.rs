//! The edits, as pure functions from one [`PitchEdit`] to the next: the editor computes the
//! new edit and sends it as one `Command::SetClipPitch`, whose inverse is the edit as it
//! was (ADR 0004). Every function keeps the edit valid ([`PitchEdit::is_valid`]): values
//! are clamped, a timing edit stops at its neighbours.

use super::curves::Frames;
use super::{PitchEdit, PitchNote, MAX_FORMANT, MAX_GAIN_DB, MAX_MOVE, MAX_SCALE, MIN_GAIN_DB};

/// A per-note value set on a selection.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Field {
    /// Pitch centre, 0 … 1.
    Centre,
    /// Drift scale, 0 … 2.
    Drift,
    /// Vibrato scale, 0 … 2.
    Vibrato,
    /// Formant shift, semitones.
    Formant,
    /// Gain, dB.
    Gain,
}

impl Field {
    pub const ALL: [Field; 5] = [
        Field::Centre,
        Field::Drift,
        Field::Vibrato,
        Field::Formant,
        Field::Gain,
    ];

    pub fn range(self) -> (f32, f32) {
        match self {
            Field::Centre => (0.0, 1.0),
            Field::Drift | Field::Vibrato => (0.0, MAX_SCALE),
            Field::Formant => (-MAX_FORMANT, MAX_FORMANT),
            Field::Gain => (MIN_GAIN_DB, MAX_GAIN_DB),
        }
    }

    pub fn default_value(self) -> f32 {
        match self {
            Field::Drift | Field::Vibrato => 1.0,
            _ => 0.0,
        }
    }

    pub fn get(self, n: &PitchNote) -> f32 {
        match self {
            Field::Centre => n.centre,
            Field::Drift => n.drift,
            Field::Vibrato => n.vibrato,
            Field::Formant => n.formant,
            Field::Gain => n.gain_db,
        }
    }

    fn slot(self, n: &mut PitchNote) -> &mut f32 {
        match self {
            Field::Centre => &mut n.centre,
            Field::Drift => &mut n.drift,
            Field::Vibrato => &mut n.vibrato,
            Field::Formant => &mut n.formant,
            Field::Gain => &mut n.gain_db,
        }
    }

    pub fn op(self) -> super::PitchOp {
        match self {
            Field::Centre => super::PitchOp::Centre,
            Field::Drift => super::PitchOp::Drift,
            Field::Vibrato => super::PitchOp::Vibrato,
            Field::Formant => super::PitchOp::Formant,
            Field::Gain => super::PitchOp::Gain,
        }
    }
}

/// Set `field` to `value` (clamped) on the notes `ids`. Returns whether anything changed.
pub fn set_field(edit: &mut PitchEdit, ids: &[u32], field: Field, value: f32) -> bool {
    if !value.is_finite() {
        return false;
    }
    let (lo, hi) = field.range();
    let v = value.clamp(lo, hi);
    let mut changed = false;
    for n in edit.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
        let s = field.slot(n);
        if *s != v {
            *s = v;
            changed = true;
        }
    }
    changed
}

/// Put note `id` on `pitch` (MIDI, fractional allowed), at most [`MAX_MOVE`] from where
/// it was sung.
pub fn set_pitch(edit: &mut PitchEdit, id: u32, pitch: f32) -> bool {
    let Some(n) = edit.note_mut(id).filter(|_| pitch.is_finite()) else {
        return false;
    };
    let p = pitch.clamp(n.home() - MAX_MOVE, n.home() + MAX_MOVE);
    let changed = n.pitch != p;
    n.pitch = p;
    // Moved by hand: the hand's, no longer the macro's.
    if changed {
        n.corrected = false;
    }
    changed
}

/// Move the notes `ids` by `semitones` (each from where it is now).
pub fn move_pitch(edit: &mut PitchEdit, ids: &[u32], semitones: f32) -> bool {
    let mut changed = false;
    for id in ids {
        if let Some(p) = edit.note(*id).map(|n| n.pitch + semitones) {
            changed |= set_pitch(edit, *id, p);
        }
    }
    changed
}

/// The output range note `i` may occupy: after the previous note (or the take's start),
/// before the next (or its end), strictly apart where the source has a gap there.
pub fn bounds(edit: &PitchEdit, i: usize) -> (u64, u64) {
    let n = &edit.notes[i];
    let (prev_end, prev_src) = match i.checked_sub(1) {
        Some(p) => (edit.notes[p].end, edit.notes[p].src_end),
        None => (0, 0),
    };
    let (next_start, next_src) = match edit.notes.get(i + 1) {
        Some(q) => (q.start, q.src_start),
        None => (edit.frames, edit.frames),
    };
    let lo = prev_end + u64::from(n.src_start > prev_src);
    let hi = next_start - u64::from(next_src > n.src_end).min(next_start);
    (lo, hi.max(lo))
}

/// Play note `id` over output frames `[start, end)`, clamped to its neighbours and to the
/// shortest note: stretching a note moves only it and the gaps beside it.
pub fn set_timing(edit: &mut PitchEdit, id: u32, start: u64, end: u64) -> bool {
    let Some(i) = edit.notes.iter().position(|n| n.id == id) else {
        return false;
    };
    let (lo, hi) = bounds(edit, i);
    let min = edit.min_len().min(hi - lo);
    let s = start.clamp(lo, hi - min);
    let e = end.clamp(s + min, hi);
    let n = &mut edit.notes[i];
    let changed = n.start != s || n.end != e;
    n.start = s;
    n.end = e;
    changed
}

/// Move note `id` in time by `delta` output frames, keeping its length (clamped).
pub fn shift_timing(edit: &mut PitchEdit, id: u32, delta: i64) -> bool {
    let Some(i) = edit.notes.iter().position(|n| n.id == id) else {
        return false;
    };
    let (lo, hi) = bounds(edit, i);
    let n = &edit.notes[i];
    let len = n.end - n.start;
    let max = hi.saturating_sub(len) as i64;
    let s = (n.start as i64 + delta).clamp(lo as i64, max.max(lo as i64)) as u64;
    set_timing(edit, id, s, s + len)
}

/// Split note `id` at source frame `at`. The halves keep its edits (a moved note stays
/// moved: each half on its own sung centre, measured from `frames` when given, plus the
/// same move); the left half keeps the id. Refused within the shortest note of an end.
pub fn split(edit: &mut PitchEdit, id: u32, at: u64, frames: Option<&Frames>) -> bool {
    let min = edit.min_len();
    let Some(i) = edit.notes.iter().position(|n| n.id == id) else {
        return false;
    };
    let n = edit.notes[i].clone();
    if at < n.src_start + min || at + min > n.src_end {
        return false;
    }
    let out_at = {
        let k = (at - n.src_start) as f64 / (n.src_end - n.src_start) as f64;
        let o = n.start as f64 + k * (n.end - n.start) as f64;
        (o.round() as u64).clamp(n.start + 1, n.end - 1)
    };
    let moved = n.pitch - n.home();
    let resung = |a: u64, b: u64| frames.and_then(|f| f.centre(a, b)).unwrap_or(n.sung);
    let mut left = n.clone();
    left.src_end = at;
    left.end = out_at;
    left.sung = resung(n.src_start, at);
    left.pitch = left.sung.round() + moved;
    let mut right = n.clone();
    right.id = edit.next_id();
    right.src_start = at;
    right.start = out_at;
    right.sung = resung(at, n.src_end);
    right.pitch = right.sung.round() + moved;
    edit.notes[i] = left;
    edit.notes.insert(i + 1, right);
    true
}

/// Join the notes `ids` (adjacent ones; at least two) into one: the first's edits, the
/// whole span, the centre the length-weighted mean of theirs. Returns whether it joined.
pub fn join(edit: &mut PitchEdit, ids: &[u32]) -> bool {
    let mut idx: Vec<usize> = edit
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| ids.contains(&n.id))
        .map(|(i, _)| i)
        .collect();
    idx.sort_unstable();
    if idx.len() < 2 || idx.windows(2).any(|w| w[1] != w[0] + 1) {
        return false;
    }
    let (a, b) = (idx[0], *idx.last().expect("two"));
    let first = edit.notes[a].clone();
    let last = &edit.notes[b];
    let (mut w, mut sum, mut lvl) = (0.0f64, 0.0f64, 0.0f64);
    for n in &edit.notes[a..=b] {
        let len = (n.src_end - n.src_start) as f64;
        w += len;
        sum += n.sung as f64 * len;
        lvl += n.level_db as f64 * len;
    }
    let mut joined = first.clone();
    joined.src_end = last.src_end;
    joined.end = last.end;
    joined.sung = (sum / w) as f32;
    joined.level_db = (lvl / w) as f32;
    joined.pitch = joined.sung.round() + (first.pitch - first.home());
    edit.notes.splice(a..=b, [joined]);
    true
}

/// *Correct pitch* on the notes `ids` by `amount` (0 … 1): each note pulled that far from
/// where it was sung toward its target — the nearest note of the key (its own semitone
/// when that is in the key), or the note it was moved to by hand. At 0 nothing moves; at
/// 1 it sounds on its target; applied again at another amount, as if applied once — in
/// the key the edit has now (review 2: which pitch is the macro's is stored on the note,
/// `PitchNote::corrected`, not guessed from the key, so a key change re-targets it and a
/// hand move onto the key's note is kept).
pub fn correct_pitch(edit: &mut PitchEdit, ids: &[u32], amount: f32) -> bool {
    let amount = amount.clamp(0.0, 1.0);
    let key = edit.key;
    let mut changed = false;
    for n in edit.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
        let home = n.home();
        let auto = if key.contains(home as i32) {
            home
        } else {
            key.nearest(n.sung)
        };
        // The macro's own pitch (an earlier run), or one never moved: set toward the key's
        // note; anything moved by hand stays the target.
        let by_macro = n.corrected || n.pitch == home;
        let pitch = if by_macro {
            home + amount * (auto - home)
        } else {
            n.pitch
        };
        if n.pitch != pitch || n.centre != amount || n.corrected != by_macro {
            n.pitch = pitch;
            n.centre = amount;
            n.corrected = by_macro;
            changed = true;
        }
    }
    changed
}

/// *Correct drift* on the notes `ids` by `amount` (0 … 1): the drift scaled to `1 − amount`.
pub fn correct_drift(edit: &mut PitchEdit, ids: &[u32], amount: f32) -> bool {
    set_field(edit, ids, Field::Drift, 1.0 - amount.clamp(0.0, 1.0))
}

/// *Quantise timing*: each of the notes `ids` moved (its length kept, its neighbours
/// respected) so it starts where `snap` puts its start (output frames → output frames).
pub fn quantise(edit: &mut PitchEdit, ids: &[u32], snap: impl Fn(u64) -> u64) -> bool {
    quantise_by(edit, ids, |n| snap(n.start) as i64 - n.start as i64)
}

/// [`quantise`] with the move of each note given whole (`delta`, output frames): a clip
/// played backwards quantises the edge it starts with on the timeline, its source end.
pub fn quantise_by(edit: &mut PitchEdit, ids: &[u32], delta: impl Fn(&PitchNote) -> i64) -> bool {
    let mut changed = false;
    // Where each note wants to go, decided before any moves: then the notes moving earlier
    // move first to last, and those moving later last to first, so none is blocked by a
    // neighbour that is about to move out of its way.
    let moves: Vec<(u32, i64)> = edit
        .notes
        .iter()
        .filter(|n| ids.contains(&n.id))
        .map(|n| (n.id, delta(n)))
        .collect();
    for (id, delta) in moves.iter().filter(|m| m.1 < 0) {
        changed |= shift_timing(edit, *id, *delta);
    }
    for (id, delta) in moves.iter().rev().filter(|m| m.1 > 0) {
        changed |= shift_timing(edit, *id, *delta);
    }
    changed
}

/// *Quantise timing to another track*: each of the notes `ids` moved so its first edge on
/// the timeline — its start, or for a clip played backwards (`reverse`) its end — lands on
/// the nearest of `onsets` (output frames, fractional, sorted or not) within `reach`
/// frames: its length kept, its neighbours respected; a note with no onset in reach stays.
/// The editor's *Quantise* to a track runs this.
pub fn quantise_to(
    edit: &mut PitchEdit,
    ids: &[u32],
    onsets: &[f64],
    reach: f64,
    reverse: bool,
) -> bool {
    quantise_by(edit, ids, |n| {
        let at = if reverse { n.end } else { n.start } as f64;
        onsets
            .iter()
            .map(|o| o - at)
            .filter(|d| d.abs() <= reach)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .map_or(0, |d| d.round() as i64)
    })
}

/// Every edit on the notes `ids` undone (their segmentation kept).
pub fn reset(edit: &mut PitchEdit, ids: &[u32]) -> bool {
    let mut changed = false;
    for id in ids {
        let Some(n) = edit.note_mut(*id) else {
            continue;
        };
        let mut r = n.reset();
        // The timing goes back as far as the neighbours let it (they may have moved).
        (r.start, r.end) = (n.start, n.end);
        if *n != r {
            *n = r;
            changed = true;
        }
        let (a, b) = (n.src_start, n.src_end);
        changed |= set_timing(edit, *id, a, b);
    }
    changed
}
