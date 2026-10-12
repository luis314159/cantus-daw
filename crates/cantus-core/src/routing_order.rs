//! Strips and groups moving along the desk (round 25, the owner's answer 12): a strip or
//! the selection dragged to a place, a group dragged by its separator as one block, and
//! Alt+←/→ moving the selection — or, when the selection is a whole group, the group.
//! Each is one [`Command::Batch`] (`cmd.move_strips`, `cmd.move_group`): one step of undo.
//!
//! **Docks are respected**: a drag names the dock it lands in (a strip dragged into the
//! right dock is pinned there); a nudge never leaves its dock. **Groups are never split by
//! accident**: a strip dropped *between two members* of a group joins it; a member dragged
//! away from every other member leaves its group; a nudge hops a loose strip over a whole
//! group, and a member nudged past its group's edge steps out of the group where it
//! stands; a group dropped inside another group's run lands before that run.
//!
//! The desk shows each dock as the project's strips in order, filtered by dock (ADR 0016),
//! so every move is a reordering of `Project::strips` (by `MoveStrip`) plus the docks and
//! groups that change.

use crate::command::Command;
use crate::model::{Dock, GroupId, Project, StripId};

/// Where a dragged strip, selection or group lands: in `dock`, just before the strip
/// `before`, or at the end of the dock when `None`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Spot {
    pub dock: Dock,
    pub before: Option<StripId>,
}

impl Spot {
    pub fn before(dock: Dock, strip: StripId) -> Spot {
        Spot {
            dock,
            before: Some(strip),
        }
    }

    pub fn end(dock: Dock) -> Spot {
        Spot { dock, before: None }
    }
}

/// The spot before the `index`-th strip of `dock` in desk order (separators not counted),
/// or the dock's end when `index` is past its last strip — what a drop between two strip
/// columns names.
pub fn spot_at(project: &Project, dock: Dock, index: usize) -> Spot {
    Spot {
        dock,
        before: project
            .strips
            .iter()
            .filter(|s| s.dock == dock)
            .nth(index)
            .map(|s| s.id),
    }
}

/// A strip's group, if the group exists (the desk ignores a dangling one).
fn group_of(project: &Project, id: StripId) -> Option<GroupId> {
    project
        .strip(id)
        .and_then(|s| s.group)
        .filter(|g| project.strip_group(*g).is_some())
}

fn dock_of(project: &Project, id: StripId) -> Dock {
    project.strip(id).map(|s| s.dock).unwrap_or_default()
}

fn members(project: &Project, group: GroupId) -> Vec<StripId> {
    project
        .strips
        .iter()
        .filter(|s| s.group == Some(group))
        .map(|s| s.id)
        .collect()
}

/// `MoveStrip`s that turn the project's strip order into `desired` (the same strips),
/// each applied to the order the ones before it left.
pub fn reorder(project: &Project, desired: &[StripId]) -> Vec<Command> {
    let mut now: Vec<StripId> = project.strips.iter().map(|s| s.id).collect();
    let mut out = Vec::new();
    for (k, id) in desired.iter().enumerate() {
        if now.get(k) == Some(id) {
            continue;
        }
        let Some(j) = now.iter().position(|x| x == id) else {
            continue;
        };
        now.remove(j);
        now.insert(k, *id);
        out.push(Command::MoveStrip { id: *id, index: k });
    }
    out
}

fn batch(label: &str, commands: Vec<Command>) -> Option<Command> {
    (!commands.is_empty()).then(|| Command::Batch {
        label_key: label.to_string(),
        commands,
    })
}

/// The strip order with `block` taken out and put back just before `before` (or at the
/// end), and the commands that dock the block in `dock`.
fn placed(
    project: &Project,
    block: &[StripId],
    dock: Dock,
    before: Option<StripId>,
) -> (Vec<StripId>, Vec<Command>) {
    let mut order: Vec<StripId> = project
        .strips
        .iter()
        .map(|s| s.id)
        .filter(|id| !block.contains(id))
        .collect();
    let at = before
        .and_then(|b| order.iter().position(|x| *x == b))
        .unwrap_or(order.len());
    order.splice(at..at, block.iter().copied());
    let mut commands = reorder(project, &order);
    commands.extend(
        block
            .iter()
            .filter(|id| dock_of(project, **id) != dock)
            .map(|id| Command::SetStripDock { id: *id, dock }),
    );
    (order, commands)
}

/// A strip or the selection dragged to `spot`: one block, in desk order, docked where it
/// lands; groups joined or left by the rules above, the block ordered so it splits no
/// group (the members staying in the left neighbour's group first, the right's last).
/// `None` when nothing changes.
pub fn move_strips(project: &Project, strips: &[StripId], spot: Spot) -> Option<Command> {
    let block: Vec<StripId> = project
        .strips
        .iter()
        .map(|s| s.id)
        .filter(|id| strips.contains(id))
        .collect();
    if block.is_empty() {
        return None;
    }
    let mut before = spot.before;
    if let Some(b) = before {
        if dock_of(project, b) != spot.dock || project.strip(b).is_none() {
            return None;
        }
        if block.contains(&b) {
            // Dropped onto itself: before the first strip after it that stays.
            let dock: Vec<StripId> = project
                .strips
                .iter()
                .filter(|s| s.dock == spot.dock)
                .map(|s| s.id)
                .collect();
            let from = dock.iter().position(|x| *x == b).unwrap_or(0);
            before = dock[from..].iter().copied().find(|x| !block.contains(x));
        }
    }
    // The block's neighbours in its new dock decide its groups (they do not depend on the
    // order inside the block).
    let (order, _) = placed(project, &block, spot.dock, before);
    let seq: Vec<StripId> = order
        .iter()
        .copied()
        .filter(|id| block.contains(id) || dock_of(project, *id) == spot.dock)
        .collect();
    let first = seq.iter().position(|x| *x == block[0]).unwrap_or(0);
    let last = first + block.len() - 1;
    let gp = first
        .checked_sub(1)
        .and_then(|i| seq.get(i))
        .and_then(|id| group_of(project, *id));
    let gn = seq.get(last + 1).and_then(|id| group_of(project, *id));
    let next: Vec<Option<GroupId>> = block
        .iter()
        .map(|id| {
            let own = group_of(project, *id);
            if gp.is_some() && gp == gn {
                gp
            } else {
                own.filter(|g| {
                    gp == Some(*g)
                        || gn == Some(*g)
                        || members(project, *g).iter().all(|m| block.contains(m))
                })
            }
        })
        .collect();
    // The block in an order that splits no group: the strips staying in the left
    // neighbour's group first, those in the right neighbour's group last, every other
    // group's strips together where its first one was (a loose strip of the selection
    // never lands between two members).
    let rank = |k: usize| -> (u8, usize) {
        match next[k] {
            g @ Some(_) if g == gp => (0, 0),
            g @ Some(_) if g == gn => (2, 0),
            Some(g) => (1, next.iter().position(|x| *x == Some(g)).unwrap_or(k)),
            None => (1, k),
        }
    };
    let mut by: Vec<usize> = (0..block.len()).collect();
    by.sort_by_key(|k| (rank(*k), *k));
    let block: Vec<StripId> = by.iter().map(|k| block[*k]).collect();
    let next: Vec<Option<GroupId>> = by.iter().map(|k| next[*k]).collect();
    let (_, mut commands) = placed(project, &block, spot.dock, before);
    for (id, next) in block.iter().zip(next) {
        if next != group_of(project, *id) {
            commands.push(Command::SetStripGroup {
                id: *id,
                group: next,
            });
        }
    }
    batch("cmd.move_strips", commands)
}

/// A group dragged by its separator: every member, in order, as one block before `spot`
/// (before the whole run of another group it would split), docked where it lands, still
/// in its group. `None` when nothing changes.
pub fn move_group(project: &Project, group: GroupId, spot: Spot) -> Option<Command> {
    project.strip_group(group)?;
    let block = members(project, group);
    if block.is_empty() {
        return None;
    }
    let mut before = spot.before;
    if let Some(b) = before {
        if dock_of(project, b) != spot.dock || block.contains(&b) {
            return None;
        }
        if let Some(h) = group_of(project, b) {
            let dock: Vec<StripId> = project
                .strips
                .iter()
                .filter(|s| s.dock == spot.dock && !block.contains(&s.id))
                .map(|s| s.id)
                .collect();
            let mut at = dock.iter().position(|x| *x == b)?;
            while at > 0 && group_of(project, dock[at - 1]) == Some(h) {
                at -= 1;
            }
            before = Some(dock[at]);
        }
    }
    let (_, commands) = placed(project, &block, spot.dock, before);
    batch("cmd.move_group", commands)
}

/// Alt+←/→: every selected strip `delta` places along its own dock (a run moves as a
/// unit; a dock's edge or a selected strip that cannot move stops it). A loose strip hops
/// over a whole group; a member swaps inside its run and, at the run's edge, steps out of
/// the group where it stands. When the selection is exactly a group's members, the group
/// moves instead ([`nudge_group`]).
pub fn nudge(project: &Project, strips: &[StripId], delta: i32) -> Option<Command> {
    let sel: Vec<StripId> = project
        .strips
        .iter()
        .map(|s| s.id)
        .filter(|id| strips.contains(id))
        .collect();
    if sel.is_empty() || delta == 0 {
        return None;
    }
    if let Some(g) = group_of(project, sel[0]) {
        let m = members(project, g);
        if m.len() == sel.len() && m.iter().all(|x| sel.contains(x)) {
            return nudge_group(project, g, delta);
        }
    }
    let mut ids: Vec<StripId> = project.strips.iter().map(|s| s.id).collect();
    let mut grp: Vec<Option<GroupId>> = ids.iter().map(|id| group_of(project, *id)).collect();
    for dock in Dock::ALL {
        let slots: Vec<usize> = (0..ids.len())
            .filter(|k| dock_of(project, ids[*k]) == dock)
            .collect();
        if !slots.iter().any(|k| sel.contains(&ids[*k])) {
            continue;
        }
        let mut seq: Vec<(StripId, Option<GroupId>)> =
            slots.iter().map(|k| (ids[*k], grp[*k])).collect();
        for _ in 0..delta.unsigned_abs() {
            nudge_seq(&mut seq, &sel, delta.signum() as isize);
        }
        for (k, slot) in slots.iter().enumerate() {
            ids[*slot] = seq[k].0;
            grp[*slot] = seq[k].1;
        }
    }
    let mut commands = reorder(project, &ids);
    for (id, g) in ids.iter().zip(&grp) {
        if group_of(project, *id) != *g {
            commands.push(Command::SetStripGroup { id: *id, group: *g });
        }
    }
    batch("cmd.move_strips", commands)
}

/// One step of [`nudge`] in one dock, the leading strip first.
fn nudge_seq(seq: &mut Vec<(StripId, Option<GroupId>)>, sel: &[StripId], d: isize) {
    let mut leading: Vec<StripId> = seq
        .iter()
        .map(|x| x.0)
        .filter(|id| sel.contains(id))
        .collect();
    if d > 0 {
        leading.reverse();
    }
    let n = seq.len() as isize;
    for id in leading {
        let Some(i) = seq.iter().position(|x| x.0 == id) else {
            continue;
        };
        let j = i as isize + d;
        let inside = (0..n).contains(&j);
        match seq[i].1 {
            Some(g) => {
                if inside && seq[j as usize].1 == Some(g) {
                    if !sel.contains(&seq[j as usize].0) {
                        seq.swap(i, j as usize);
                    }
                } else {
                    // At its run's edge: out of the group, where it stands.
                    seq[i].1 = None;
                }
            }
            None => {
                if !inside || sel.contains(&seq[j as usize].0) {
                    continue;
                }
                match seq[j as usize].1 {
                    None => seq.swap(i, j as usize),
                    Some(h) => {
                        // Hop over the whole run of `h`.
                        let mut end = j;
                        while (0..n).contains(&(end + d)) && seq[(end + d) as usize].1 == Some(h) {
                            end += d;
                        }
                        let item = seq.remove(i);
                        seq.insert(end as usize, item);
                    }
                }
            }
        }
    }
}

/// A group `delta` places along its dock: past one loose strip, or over another group's
/// whole run, per step. Its members in the dock of its first member move as one block
/// (gathered there if they were apart); members stay members.
pub fn nudge_group(project: &Project, group: GroupId, delta: i32) -> Option<Command> {
    project.strip_group(group)?;
    let all = members(project, group);
    let dock = dock_of(project, *all.first()?);
    let slots: Vec<usize> = (0..project.strips.len())
        .filter(|k| project.strips[*k].dock == dock)
        .collect();
    // The dock as units: the group's block, loose strips, other groups' runs.
    let mut units: Vec<Vec<StripId>> = Vec::new();
    let mut block_at = None;
    let mut last_run: Option<GroupId> = None;
    for k in &slots {
        let id = project.strips[*k].id;
        let g = group_of(project, id);
        if g == Some(group) {
            if block_at.is_none() {
                block_at = Some(units.len());
                units.push(
                    slots
                        .iter()
                        .map(|k| project.strips[*k].id)
                        .filter(|x| group_of(project, *x) == Some(group))
                        .collect(),
                );
            }
            last_run = None;
            continue;
        }
        match (g, last_run, units.last_mut()) {
            (Some(h), Some(r), Some(u)) if h == r => u.push(id),
            _ => units.push(vec![id]),
        }
        last_run = g;
    }
    let mut b = block_at?;
    for _ in 0..delta.unsigned_abs() {
        let t = b as isize + delta.signum() as isize;
        if !(0..units.len() as isize).contains(&t) {
            break;
        }
        units.swap(b, t as usize);
        b = t as usize;
    }
    let seq: Vec<StripId> = units.into_iter().flatten().collect();
    let mut ids: Vec<StripId> = project.strips.iter().map(|s| s.id).collect();
    for (k, slot) in slots.iter().enumerate() {
        ids[*slot] = seq[k];
    }
    batch("cmd.move_group", reorder(project, &ids))
}
