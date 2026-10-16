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

/// Each metre change as the 0-based bar it starts and its signature.
fn to_bars(map: &TempoMap) -> Vec<(i64, TimeSignature)> {
    map.metres()
        .iter()
        .map(|m| (map.bbt(m.tick).0 - 1, m.signature))
        .collect()
}

/// The metre changes for changes on bars (0-based, increasing, the first at bar 0).
fn from_bars(bars: &[(i64, TimeSignature)]) -> Vec<MetreChange> {
    let mut out: Vec<MetreChange> = Vec::with_capacity(bars.len());
    let mut tick = 0i64;
    for (i, (bar, sig)) in bars.iter().enumerate() {
        if i > 0 {
            let (prev_bar, prev_sig) = bars[i - 1];
            tick += (bar - prev_bar) * prev_sig.ticks_per_bar().0;
        }
        out.push(MetreChange {
            tick: Ticks(tick),
            signature: *sig,
        });
    }
    out
}

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

/// Replace the metre by `bars` (changes on 0-based bars), returning the inverse.
fn set_by_bars(
    project: &mut Project,
    bars: &[(i64, TimeSignature)],
) -> Result<Command, CommandError> {
    let metres = from_bars(bars);
    let signature = metres[0].signature;
    apply_set_metre_track(project, signature, metres)
}

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
    let bar = map.bbt(tick).0 - 1;
    let mut bars = to_bars(&map);
    match bars.iter().position(|(b, _)| *b >= bar) {
        Some(i) if bars[i].0 == bar => bars[i].1 = signature,
        Some(i) => bars.insert(i, (bar, signature)),
        None => bars.push((bar, signature)),
    }
    set_by_bars(project, &bars)
}

pub(crate) fn apply_remove_metre_at(
    project: &mut Project,
    tick: Ticks,
) -> Result<Command, CommandError> {
    let map = project.tempo_map();
    let i = project
        .metre_changes
        .iter()
        .position(|m| m.tick == tick)
        .filter(|i| *i > 0)
        .ok_or_else(|| invalid("no metre change there"))?;
    let mut bars = to_bars(&map);
    bars.remove(i);
    set_by_bars(project, &bars)
}

pub(crate) fn apply_move_metre(
    project: &mut Project,
    from: Ticks,
    to: Ticks,
) -> Result<Command, CommandError> {
    let map = project.tempo_map();
    let i = project
        .metre_changes
        .iter()
        .position(|m| m.tick == from)
        .filter(|i| *i > 0)
        .ok_or_else(|| invalid("no metre change there"))?;
    // The change moves along the bar lines of the metre before it.
    let mut without = project.metre_changes.clone();
    let moved = without.remove(i);
    let other = TempoMap::new(120.0, project.time_signature, &[], &without);
    if to.0 <= 0 || other.bar_start(to) != to {
        return Err(invalid("a metre change falls on a bar line"));
    }
    let mut bars = to_bars(&map);
    let new_bar = other.bbt(to).0 - 1;
    if new_bar <= bars[i - 1].0 || bars.get(i + 1).is_some_and(|(b, _)| new_bar >= *b) {
        return Err(invalid("a metre change stays between its neighbours"));
    }
    bars[i] = (new_bar, moved.signature);
    set_by_bars(project, &bars)
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
