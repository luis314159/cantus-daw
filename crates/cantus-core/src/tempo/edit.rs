//! The tempo track's, the metre changes' and the markers' commands (ADR 0004, ADR 0060):
//! each `apply_*` validates, changes the project and returns the exact inverse.
//!
//! Tempo points are addressed by index (a drag keeps a point between its neighbours, so
//! the index is stable through a gesture). Metre changes are addressed by their tick and
//! edited **by bars**: adding, moving or removing one keeps every later change on the bar
//! it was on (Cubase's and Studio One's signature track), so the music after it keeps its
//! bar numbers; the inverse is the whole metre list as it was.

use super::{valid_bpm, valid_signature, Marker, MarkerId, MetreChange, TempoMap, TempoPoint};
use crate::command::{Command, CommandError};
use crate::model::Project;
use crate::time::{Ticks, TimeSignature};

fn invalid(what: &str) -> CommandError {
    CommandError::InvalidValue(what.into())
}

/// A tempo track the model can hold: empty, or from tick 0 at `bpm`, ticks strictly
/// increasing, every tempo valid.
fn check_points(bpm: f64, points: &[TempoPoint]) -> Result<(), CommandError> {
    if !valid_bpm(bpm) {
        return Err(invalid("tempo out of range"));
    }
    let Some(first) = points.first() else {
        return Ok(());
    };
    if first.tick != Ticks::ZERO || first.bpm != bpm {
        return Err(invalid(
            "a tempo track starts at tick 0 at the song's tempo",
        ));
    }
    if points.iter().any(|p| !valid_bpm(p.bpm)) {
        return Err(invalid("tempo out of range"));
    }
    if points.windows(2).any(|w| w[1].tick <= w[0].tick) {
        return Err(invalid("tempo points out of order"));
    }
    Ok(())
}

pub(crate) fn apply_set_tempo_track(
    project: &mut Project,
    bpm: f64,
    points: Vec<TempoPoint>,
) -> Result<Command, CommandError> {
    check_points(bpm, &points)?;
    let old_bpm = std::mem::replace(&mut project.tempo_bpm, bpm);
    let old = std::mem::replace(&mut project.tempo_points, points);
    Ok(Command::SetTempoTrack {
        bpm: old_bpm,
        points: old,
    })
}

/// Whether `point` may stand at `index` between `before` and `after`.
fn fits(point: &TempoPoint, before: Option<&TempoPoint>, after: Option<&TempoPoint>) -> bool {
    valid_bpm(point.bpm)
        && before.is_none_or(|b| b.tick < point.tick)
        && after.is_none_or(|a| point.tick < a.tick)
}

pub(crate) fn apply_insert_tempo_point(
    project: &mut Project,
    index: usize,
    point: TempoPoint,
) -> Result<Command, CommandError> {
    let pts = &mut project.tempo_points;
    if pts.is_empty() || index == 0 || index > pts.len() {
        return Err(CommandError::IndexOutOfRange {
            index,
            len: pts.len(),
        });
    }
    if !fits(&point, pts.get(index - 1), pts.get(index)) {
        return Err(invalid("tempo point"));
    }
    pts.insert(index, point);
    Ok(Command::RemoveTempoPoint { index })
}

pub(crate) fn apply_remove_tempo_point(
    project: &mut Project,
    index: usize,
) -> Result<Command, CommandError> {
    let pts = &mut project.tempo_points;
    if index == 0 || index >= pts.len() {
        return Err(CommandError::IndexOutOfRange {
            index,
            len: pts.len(),
        });
    }
    let point = pts.remove(index);
    Ok(Command::InsertTempoPoint { index, point })
}

pub(crate) fn apply_set_tempo_point(
    project: &mut Project,
    index: usize,
    point: TempoPoint,
) -> Result<Command, CommandError> {
    let len = project.tempo_points.len();
    if index >= len {
        return Err(CommandError::IndexOutOfRange { index, len });
    }
    let pts = &project.tempo_points;
    let ok = if index == 0 {
        point.tick == Ticks::ZERO && valid_bpm(point.bpm)
    } else {
        fits(&point, pts.get(index - 1), pts.get(index + 1))
    };
    if !ok {
        return Err(invalid("tempo point"));
    }
    if index == 0 {
        project.tempo_bpm = point.bpm;
    }
    let old = std::mem::replace(&mut project.tempo_points[index], point);
    Ok(Command::SetTempoPoint { index, point: old })
}

// --- metre --------------------------------------------------------------------------------

/// A metre list the model can hold: empty, or from tick 0 at `signature`, ticks strictly
/// increasing, every signature valid. A change off the bar lines of the ones before it is
/// allowed here (an earlier metre edited under it, an imported file): the bar before it is
/// cut short; the commands that place a change put it on a line.
fn check_metres(signature: TimeSignature, metres: &[MetreChange]) -> Result<(), CommandError> {
    if !valid_signature(signature) {
        return Err(invalid("time signature"));
    }
    let Some(first) = metres.first() else {
        return Ok(());
    };
    if first.tick != Ticks::ZERO || first.signature != signature {
        return Err(invalid("a metre list starts at tick 0 at the song's metre"));
    }
    for i in 1..metres.len() {
        let m = metres[i];
        if !valid_signature(m.signature) || m.tick <= metres[i - 1].tick {
            return Err(invalid("metre change"));
        }
    }
    Ok(())
}

pub(crate) fn apply_set_metre_track(
    project: &mut Project,
    signature: TimeSignature,
    metres: Vec<MetreChange>,
) -> Result<Command, CommandError> {
    check_metres(signature, &metres)?;
    let old_sig = std::mem::replace(&mut project.time_signature, signature);
    let old = std::mem::replace(&mut project.metre_changes, metres);
    Ok(Command::SetMetreTrack {
        signature: old_sig,
        metres: old,
    })
}

/// The metre list as it stands, with the song's metre at tick 0 when it has no changes.
fn current(project: &Project) -> Vec<MetreChange> {
    if project.metre_changes.is_empty() {
        vec![MetreChange {
            tick: Ticks::ZERO,
            signature: project.time_signature,
        }]
    } else {
        project.metre_changes.clone()
    }
}

/// Where change `i` of `metres` sits against the bars of the metre before it: its 0-based
/// bar index and its remainder (the ticks of a short bar before it; 0 on a bar line).
fn seat(metres: &[MetreChange], i: usize) -> (i64, i64) {
    let map = TempoMap::new(120.0, metres[0].signature, &[], metres);
    let bar = map.bbt(metres[i].tick).0 - 1;
    let prev = metres[i - 1];
    let rem = (metres[i].tick - prev.tick)
        .0
        .rem_euclid(prev.signature.ticks_per_bar().0);
    (bar, rem)
}

/// Put change `next` back on bar `bar` with its remainder `rem`, the segment before it now
/// starting at `start` (0-based bar `b_start`) in `sig`; every later change moves with it
/// (so each keeps its bar number). Fails when the change would no longer come after
/// `start`.
fn reseat(
    metres: &mut [MetreChange],
    next: usize,
    start: Ticks,
    b_start: i64,
    sig: TimeSignature,
    (bar, rem): (i64, i64),
) -> Result<(), CommandError> {
    if next >= metres.len() {
        return Ok(());
    }
    let full = bar - b_start - i64::from(rem > 0);
    if full < 0 {
        return Err(invalid("a metre change stays before the next"));
    }
    let at = start.0 + full * sig.ticks_per_bar().0 + rem;
    if at <= start.0 {
        return Err(invalid("a metre change stays before the next"));
    }
    let delta = at - metres[next].tick.0;
    for m in &mut metres[next..] {
        m.tick.0 += delta;
    }
    Ok(())
}

/// The change list after an edit, set with its exact inverse (the whole list as it was).
fn set_list(project: &mut Project, metres: Vec<MetreChange>) -> Result<Command, CommandError> {
    let signature = metres[0].signature;
    apply_set_metre_track(project, signature, metres)
}

/// A metre change at the bar line `tick` (the one there changed, or one added): the
/// changes before it keep their ticks, it lands exactly at `tick`, and the ones after it
/// move so each keeps its bar number (an off-line one keeping its short bar). At tick 0 it
/// is the song's metre.
pub(crate) fn apply_set_metre_at(
    project: &mut Project,
    tick: Ticks,
    signature: TimeSignature,
) -> Result<Command, CommandError> {
    if !valid_signature(signature) {
        return Err(invalid("time signature"));
    }
    let map = project.tempo_map();
    if tick.is_negative() || map.bar_start(tick) != tick {
        return Err(invalid("a metre change falls on a bar line"));
    }
    let mut metres = current(project);
    let b_tick = map.bbt(tick).0 - 1;
    let i = match metres.iter().position(|m| m.tick == tick) {
        Some(i) => {
            metres[i].signature = signature;
            i
        }
        None => {
            let i = metres.partition_point(|m| m.tick < tick);
            metres.insert(i, MetreChange { tick, signature });
            i
        }
    };
    if i + 1 < metres.len() {
        // The next change's seat in the list as it was.
        let old = current(project);
        let j = old.iter().position(|m| m.tick == metres[i + 1].tick).unwrap_or(0);
        if j > 0 {
            let seat = seat(&old, j);
            reseat(&mut metres, i + 1, tick, b_tick, signature, seat)?;
        }
    }
    set_list(project, metres)
}

/// The metre change at `tick` removed: the metre before it runs on, and the changes after
/// it move so each keeps its bar number.
pub(crate) fn apply_remove_metre_at(
    project: &mut Project,
    tick: Ticks,
) -> Result<Command, CommandError> {
    let old = current(project);
    let i = old
        .iter()
        .position(|m| m.tick == tick)
        .filter(|i| *i > 0 && !project.metre_changes.is_empty())
        .ok_or_else(|| invalid("no metre change there"))?;
    let mut metres = old.clone();
    let prev = metres[i - 1];
    let b_prev = if i - 1 == 0 { 0 } else { seat(&old, i - 1).0 };
    let next_seat = (i + 1 < old.len()).then(|| seat(&old, i + 1));
    metres.remove(i);
    if let Some(seat) = next_seat {
        reseat(&mut metres, i, prev.tick, b_prev, prev.signature, seat)?;
    }
    set_list(project, metres)
}

/// The metre change at `from` moved to `to`, a bar line of the metre before it, between
/// its neighbours: it lands exactly at `to`, and the changes after it keep their bars.
pub(crate) fn apply_move_metre(
    project: &mut Project,
    from: Ticks,
    to: Ticks,
) -> Result<Command, CommandError> {
    let old = current(project);
    let i = old
        .iter()
        .position(|m| m.tick == from)
        .filter(|i| *i > 0 && !project.metre_changes.is_empty())
        .ok_or_else(|| invalid("no metre change there"))?;
    let prev = old[i - 1];
    let len = prev.signature.ticks_per_bar().0;
    let past = to - prev.tick;
    if past.0 <= 0 || past.0 % len != 0 || old.get(i + 1).is_some_and(|n| to >= n.tick) {
        return Err(invalid(
            "a metre change moves along the bar lines of the metre before it",
        ));
    }
    let b_prev = if i - 1 == 0 { 0 } else { seat(&old, i - 1).0 };
    let b_to = b_prev + past.0 / len;
    let next_seat = (i + 1 < old.len()).then(|| seat(&old, i + 1));
    let mut metres = old.clone();
    metres[i].tick = to;
    let sig = metres[i].signature;
    if let Some(seat) = next_seat {
        reseat(&mut metres, i + 1, to, b_to, sig, seat)?;
    }
    set_list(project, metres)
}

// --- markers ------------------------------------------------------------------------------

/// Markers are kept sorted by position, then id, so a removal's inverse lands where it was.
fn insert_sorted(markers: &mut Vec<Marker>, marker: Marker) {
    let at = markers.partition_point(|m| (m.tick, m.id) < (marker.tick, marker.id));
    markers.insert(at, marker);
}

pub(crate) fn apply_add_marker(
    project: &mut Project,
    marker: Marker,
) -> Result<Command, CommandError> {
    if project.markers.iter().any(|m| m.id == marker.id) {
        return Err(CommandError::DuplicateId(marker.id.0));
    }
    if marker.tick.is_negative() {
        return Err(invalid("a marker before the song"));
    }
    let id = marker.id;
    insert_sorted(&mut project.markers, marker);
    Ok(Command::RemoveMarker { id })
}

pub(crate) fn apply_remove_marker(
    project: &mut Project,
    id: MarkerId,
) -> Result<Command, CommandError> {
    let i = project
        .markers
        .iter()
        .position(|m| m.id == id)
        .ok_or_else(|| invalid("no such marker"))?;
    let marker = project.markers.remove(i);
    Ok(Command::AddMarker { marker })
}

pub(crate) fn apply_set_marker(
    project: &mut Project,
    marker: Marker,
) -> Result<Command, CommandError> {
    if marker.tick.is_negative() {
        return Err(invalid("a marker before the song"));
    }
    let i = project
        .markers
        .iter()
        .position(|m| m.id == marker.id)
        .ok_or_else(|| invalid("no such marker"))?;
    let old = project.markers.remove(i);
    insert_sorted(&mut project.markers, marker);
    Ok(Command::SetMarker { marker: old })
}

// --- what the interface asks for (the top bar, tap tempo, the tempo lane, controllers,
// the AI bridge) ------------------------------------------------------------------------

/// The index of the tempo point that governs `at`: the last at or before it (the first
/// for anything before tick 0); `None` without a tempo track.
pub fn point_at(project: &Project, at: Ticks) -> Option<usize> {
    let pts = &project.tempo_points;
    if pts.is_empty() {
        return None;
    }
    Some(pts.partition_point(|p| p.tick <= at).saturating_sub(1))
}

/// The command that sets "the tempo" at `at`, as the top bar's field, tap tempo and a
/// controller mapped to the tempo do (ADR 0060 §2): without a tempo track the song's one
/// tempo (`SetTempo`); with one, the value of the point that governs `at` — for a ramp,
/// the value it starts from. `bpm` is clamped to the playable range. Merge the commands of
/// one gesture under [`set_tempo_merge_key`].
pub fn set_tempo_at(project: &Project, at: Ticks, bpm: f64) -> Command {
    let bpm = crate::time::clamp_bpm(bpm);
    match point_at(project, at) {
        None | Some(0) => Command::SetTempo { bpm },
        Some(index) => Command::SetTempoPoint {
            index,
            point: TempoPoint {
                bpm,
                ..project.tempo_points[index]
            },
        },
    }
}

/// The merge key of a gesture that sets the tempo at `at` (one undo step per drag, per
/// point).
pub fn set_tempo_merge_key(project: &Project, at: Ticks) -> crate::MergeKey {
    crate::MergeKey::of(&("tempo", point_at(project, at).unwrap_or(0)))
}

/// The command that sets "the metre" at `at` (ADR 0060 §3): without metre changes the
/// song's (`SetTimeSignature`); with them, the change in force at `at`, the later changes
/// keeping their bars.
pub fn set_metre_at(project: &Project, at: Ticks, signature: TimeSignature) -> Command {
    if project.metre_changes.is_empty() {
        return Command::SetTimeSignature {
            time_signature: signature,
        };
    }
    let i = project
        .metre_changes
        .partition_point(|m| m.tick <= at)
        .saturating_sub(1);
    Command::SetMetreAt {
        tick: project.metre_changes[i].tick,
        signature,
    }
}

/// The command that adds a tempo point at `tick` (a tempo track made with it when there is
/// none; the point there changed when one is there already).
pub fn add_tempo_point(project: &Project, tick: Ticks, bpm: f64, ramp: bool) -> Command {
    let bpm = crate::time::clamp_bpm(bpm);
    let point = TempoPoint { tick, bpm, ramp };
    let pts = &project.tempo_points;
    if tick.0 <= 0 {
        return set_tempo_at(project, Ticks::ZERO, bpm);
    }
    if pts.is_empty() {
        return Command::SetTempoTrack {
            bpm: project.tempo_bpm,
            points: vec![
                TempoPoint {
                    tick: Ticks::ZERO,
                    bpm: project.tempo_bpm,
                    ramp: false,
                },
                point,
            ],
        };
    }
    match pts.iter().position(|p| p.tick == tick) {
        Some(index) => Command::SetTempoPoint { index, point },
        None => Command::InsertTempoPoint {
            index: pts.partition_point(|p| p.tick < tick),
            point,
        },
    }
}

#[cfg(test)]
mod helpers {
    use super::*;

    #[test]
    fn the_tempo_now_is_the_governing_points() {
        let mut p = Project::empty("t");
        assert_eq!(
            set_tempo_at(&p, Ticks(5000), 90.0),
            Command::SetTempo { bpm: 90.0 }
        );
        add_tempo_point(&p, Ticks(3840), 140.0, false)
            .apply(&mut p)
            .unwrap();
        assert_eq!(p.tempo_points.len(), 2);
        assert_eq!(
            set_tempo_at(&p, Ticks(100), 99.0),
            Command::SetTempo { bpm: 99.0 }
        );
        let c = set_tempo_at(&p, Ticks(5000), 150.0);
        c.apply(&mut p).unwrap();
        assert_eq!(p.tempo_points[1].bpm, 150.0);
        assert_eq!(p.tempo_points[1].tick, Ticks(3840));
        // Adding where a point is changes it; before it inserts.
        add_tempo_point(&p, Ticks(3840), 160.0, true)
            .apply(&mut p)
            .unwrap();
        assert_eq!(p.tempo_points.len(), 2);
        assert!(p.tempo_points[1].ramp);
        add_tempo_point(&p, Ticks(1920), 100.0, false)
            .apply(&mut p)
            .unwrap();
        assert_eq!(p.tempo_points[1].tick, Ticks(1920));
        // The metre in force.
        assert!(matches!(
            set_metre_at(&p, Ticks(9000), TimeSignature::new(3, 4)),
            Command::SetTimeSignature { .. }
        ));
        Command::SetMetreAt {
            tick: Ticks(7680),
            signature: TimeSignature::new(7, 8),
        }
        .apply(&mut p)
        .unwrap();
        assert_eq!(
            set_metre_at(&p, Ticks(9000), TimeSignature::new(5, 8)),
            Command::SetMetreAt {
                tick: Ticks(7680),
                signature: TimeSignature::new(5, 8)
            }
        );
    }
}
