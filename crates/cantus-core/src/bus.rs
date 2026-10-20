//! The command bus: owns the project, executes commands, keeps undo/redo history and an
//! audit log. There is exactly one of these per open project (ADR 0004).

use crate::command::{Command, CommandError, Origin};
use crate::model::Project;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Identifies a continuous gesture (dragging a fader, sweeping a knob). Consecutive
/// commands with the same key collapse into one undo step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MergeKey(pub u64);

impl MergeKey {
    pub fn of<T: Hash>(value: &T) -> MergeKey {
        let mut h = DefaultHasher::new();
        value.hash(&mut h);
        MergeKey(h.finish())
    }
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub label_key: String,
    pub command: Command,
    pub inverse: Command,
    pub origin: Origin,
    merge_key: Option<MergeKey>,
}

#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub seq: u64,
    pub revision: u64,
    pub origin: Origin,
    pub label_key: String,
    pub kind: AuditKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AuditKind {
    Execute,
    Undo,
    Redo,
    Rejected,
}

#[derive(Debug)]
pub struct CommandBus {
    project: Project,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    /// Bumped on every successful mutation (including undo/redo).
    revision: u64,
    saved_revision: u64,
    audit: Vec<AuditEntry>,
    audit_seq: u64,
    max_history: usize,
    max_audit: usize,
}

impl Default for CommandBus {
    fn default() -> Self {
        Self::new(Project::default())
    }
}

impl CommandBus {
    pub fn new(project: Project) -> Self {
        CommandBus {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            saved_revision: 0,
            audit: Vec::new(),
            audit_seq: 0,
            max_history: 10_000,
            max_audit: 50_000,
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }

    /// Mark `revision` as the one in the file: a save written off the interface's thread
    /// holds the revision it was taken at, and an edit made while it was written keeps
    /// the project dirty. A revision from the future is the present.
    pub fn mark_saved_at(&mut self, revision: u64) {
        self.saved_revision = revision.min(self.revision);
    }

    // Id allocation is deliberately *not* a command: ids are monotonic and never reused, so
    // allocating one is never undone and never conflicts. These helpers build entities
    // with fresh ids; inserting them is still done through a command.

    /// A fresh id, for callers that build a whole entity themselves (a clip copied with
    /// all its notes, say) rather than through the helpers below.
    pub fn alloc_id(&mut self) -> u64 {
        self.project.alloc_id()
    }

    /// Promise that every id below `next` has been handed out.
    ///
    /// An operation that builds several entities at once needs to read the project while
    /// it allocates, which it cannot do through [`Self::alloc_id`] — that borrows the bus.
    /// It takes ids from `project.next_id` upwards instead and says so here afterwards.
    /// Ids only ever go up, so a caller that reserves more than it used wastes numbers
    /// from a 64-bit supply and nothing else.
    pub fn reserve_ids(&mut self, next: u64) {
        self.project.next_id = self.project.next_id.max(next);
    }

    pub fn new_track(&mut self, name: impl Into<String>) -> crate::model::Track {
        self.project.make_track(name)
    }

    pub fn new_clip(
        &mut self,
        name: impl Into<String>,
        start: crate::time::Ticks,
        length: crate::time::Ticks,
    ) -> crate::model::Clip {
        self.project.make_clip(name, start, length)
    }

    pub fn new_effect(&mut self, kind: crate::effect::EffectKind) -> crate::effect::EffectSlot {
        self.project.make_effect(kind)
    }

    pub fn new_strip(&mut self, name: impl Into<String>) -> crate::model::MixerStrip {
        self.project.make_strip(name)
    }

    pub fn new_note(
        &mut self,
        start: crate::time::Ticks,
        length: crate::time::Ticks,
        pitch: crate::model::Pitch,
        velocity: crate::model::Velocity,
    ) -> crate::model::Note {
        self.project.make_note(start, length, pitch, velocity)
    }

    /// Execute a command as its own undo step.
    pub fn execute(&mut self, command: Command, origin: Origin) -> Result<(), CommandError> {
        self.execute_inner(command, origin, None)
    }

    /// Execute a command, merging it into the previous undo step if that step carries the
    /// same key. Call [`CommandBus::end_gesture`] when the gesture (drag) ends.
    pub fn execute_merging(
        &mut self,
        command: Command,
        origin: Origin,
        key: MergeKey,
    ) -> Result<(), CommandError> {
        self.execute_inner(command, origin, Some(key))
    }

    /// Seal the current gesture so the next command starts a new undo step.
    pub fn end_gesture(&mut self) {
        if let Some(top) = self.undo.last_mut() {
            top.merge_key = None;
        }
    }

    /// Seal `key`'s gesture only: a gesture that ends on its own clock (a controller's)
    /// never seals another's (a mouse drag going on). Its step is sealed wherever it is —
    /// buried under another step, an undo bringing it back to the top must not let a later
    /// gesture with the same key merge into it (review 2, #53).
    pub fn end_gesture_if(&mut self, key: MergeKey) {
        if let Some(e) = self
            .undo
            .iter_mut()
            .rev()
            .find(|e| e.merge_key == Some(key))
        {
            e.merge_key = None;
        }
    }

    /// Widen the gesture on top: `command` (a batch over every target the gesture moves now,
    /// more than before) is applied, and if the step on top is `from`'s it becomes that
    /// step — its command the new one (which sets every target it moves from scratch), its
    /// inverse the new inverse then the old (which restore the joining targets, then the
    /// earlier ones, to where they were before the gesture) — under the key `to`. Otherwise
    /// it is [`Self::execute_merging`] under `to`. One gesture, one undo step, however its
    /// targets join (a controller's knobs, ADR 0062 review 1).
    pub fn widen_merging(
        &mut self,
        command: Command,
        origin: Origin,
        from: MergeKey,
        to: MergeKey,
    ) -> Result<(), CommandError> {
        let widen = self
            .undo
            .last()
            .is_some_and(|top| top.merge_key == Some(from) && top.origin == origin);
        if !widen {
            return self.execute_inner(command, origin, Some(to));
        }
        let label_key = command.label_key().to_string();
        let (command, inverse) = match self.apply_keeping_audio(command, &label_key) {
            Ok(both) => both,
            Err(e) => {
                self.audit_push(origin, label_key, AuditKind::Rejected);
                return Err(e);
            }
        };
        self.revision += 1;
        self.redo.clear();
        let top = self.undo.last_mut().expect("checked above");
        let placeholder = Command::SetMetronome { enabled: false };
        let old_inv = std::mem::replace(&mut top.inverse, placeholder);
        top.inverse = compose(&label_key, old_inv, inverse, true);
        top.command = command;
        top.label_key = label_key.clone();
        top.merge_key = Some(to);
        self.audit_push(origin, label_key, AuditKind::Execute);
        Ok(())
    }

    /// Apply `command` and, when it changed the tempo map, the window edits that keep
    /// every unwarped audio window's source frame at its start (`tempo::keep`, ADR 0060
    /// §4): the command and its inverse, the edits composed into both. Undo and redo
    /// replay the stored pair, so nothing is derived twice.
    fn apply_keeping_audio(
        &mut self,
        command: Command,
        label_key: &str,
    ) -> Result<(Command, Command), CommandError> {
        let kept = crate::tempo::keep::before(&self.project, &command);
        let inverse = command.clone().apply(&mut self.project)?;
        let fix = kept
            .map(|k| crate::tempo::keep::after(&k, &self.project))
            .filter(|f| !f.is_empty());
        let Some(fix) = fix else {
            return Ok((command, inverse));
        };
        let fix = Command::Batch {
            label_key: label_key.to_string(),
            commands: fix,
        };
        match fix.clone().apply(&mut self.project) {
            Ok(fix_inverse) => Ok((
                compose(label_key, command, fix, false),
                compose(label_key, inverse, fix_inverse, true),
            )),
            Err(e) => {
                // Never half an edit: the command is taken back.
                let _ = inverse.apply(&mut self.project);
                Err(e)
            }
        }
    }

    fn execute_inner(
        &mut self,
        command: Command,
        origin: Origin,
        merge_key: Option<MergeKey>,
    ) -> Result<(), CommandError> {
        let label_key = command.label_key().to_string();
        let (command, inverse) = match self.apply_keeping_audio(command, &label_key) {
            Ok(both) => both,
            Err(e) => {
                self.audit_push(origin, label_key, AuditKind::Rejected);
                return Err(e);
            }
        };
        self.revision += 1;
        self.redo.clear();

        let fresh = match (merge_key, self.undo.last_mut()) {
            (Some(key), Some(top)) if top.merge_key == Some(key) && top.origin == origin => {
                if command.supersedes(&top.command) {
                    // Same parameter again: keep the original inverse (restores the
                    // pre-gesture state), replace the forward command with the latest value.
                    top.command = command;
                } else {
                    // Accumulating gesture (e.g. a recording take): compose. Redo replays
                    // old then new; undo applies new⁻¹ then old⁻¹.
                    let label = top.label_key.clone();
                    let placeholder = Command::SetMetronome { enabled: false };
                    let old_cmd = std::mem::replace(&mut top.command, placeholder.clone());
                    let old_inv = std::mem::replace(&mut top.inverse, placeholder);
                    top.command = compose(&label, old_cmd, command, false);
                    top.inverse = compose(&label, old_inv, inverse, true);
                }
                None
            }
            _ => Some((command, inverse)),
        };
        if let Some((command, inverse)) = fresh {
            self.undo.push(HistoryEntry {
                label_key: label_key.clone(),
                command,
                inverse,
                origin,
                merge_key,
            });
            if self.undo.len() > self.max_history {
                let excess = self.undo.len() - self.max_history;
                self.undo.drain(..excess);
            }
        }
        self.audit_push(origin, label_key, AuditKind::Execute);
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label_key(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label_key.as_str())
    }

    pub fn redo_label_key(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label_key.as_str())
    }

    /// Undo the last step. Returns its label key.
    pub fn undo(&mut self) -> Option<String> {
        let entry = self.undo.pop()?;
        // The inverse of a successfully applied command must apply; if it does not, the
        // model has a bug and we must not silently desync — restore the entry and bail.
        match entry.inverse.clone().apply(&mut self.project) {
            Ok(re_inverse) => {
                self.revision += 1;
                let label = entry.label_key.clone();
                self.audit_push(entry.origin, label.clone(), AuditKind::Undo);
                self.redo.push(HistoryEntry {
                    label_key: entry.label_key,
                    command: entry.command,
                    inverse: re_inverse,
                    origin: entry.origin,
                    merge_key: None,
                });
                Some(label)
            }
            Err(_) => {
                self.undo.push(entry);
                None
            }
        }
    }

    pub fn redo(&mut self) -> Option<String> {
        let entry = self.redo.pop()?;
        match entry.command.clone().apply(&mut self.project) {
            Ok(inverse) => {
                self.revision += 1;
                let label = entry.label_key.clone();
                self.audit_push(entry.origin, label.clone(), AuditKind::Redo);
                self.undo.push(HistoryEntry {
                    label_key: entry.label_key,
                    command: entry.command,
                    inverse,
                    origin: entry.origin,
                    merge_key: None,
                });
                Some(label)
            }
            Err(_) => {
                self.redo.push(entry);
                None
            }
        }
    }

    pub fn history(&self) -> &[HistoryEntry] {
        &self.undo
    }

    /// What redo would apply, the next first last.
    pub fn redo_history(&self) -> &[HistoryEntry] {
        &self.redo
    }

    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit
    }

    /// Replace the project and wipe history (new/open). Use `Command::ReplaceProject`
    /// instead when the replacement should be undoable.
    pub fn reset(&mut self, project: Project) {
        self.project = project;
        self.undo.clear();
        self.redo.clear();
        self.revision += 1;
        self.saved_revision = self.revision;
    }

    fn audit_push(&mut self, origin: Origin, label_key: String, kind: AuditKind) {
        self.audit_seq += 1;
        self.audit.push(AuditEntry {
            seq: self.audit_seq,
            revision: self.revision,
            origin,
            label_key,
            kind,
        });
        if self.audit.len() > self.max_audit {
            let excess = self.audit.len() - self.max_audit;
            self.audit.drain(..excess);
        }
    }
}

/// Merge two commands into one batch. `prepend` puts `new` before `old` (inverses).
fn compose(label: &str, old: Command, new: Command, prepend: bool) -> Command {
    let mut commands = match old {
        Command::Batch {
            label_key,
            commands,
        } if label_key == label => commands,
        other => vec![other],
    };
    if prepend {
        commands.insert(0, new);
    } else {
        commands.push(new);
    }
    Command::Batch {
        label_key: label.to_string(),
        commands,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gesture that widens (a second knob joins a turning one) is one undo step: undo
    /// takes both back to where they were before it; redo sets both; and another gesture's
    /// step is not sealed by this one's clock.
    #[test]
    fn a_widened_gesture_is_one_step_and_seals_only_itself() {
        let mut bus = CommandBus::default();
        let m = bus.project().master.clone();
        let (a, b) = (MergeKey::of(&"a"), MergeKey::of(&"ab"));
        bus.execute_merging(Command::SetMasterPan { pan: 0.2 }, Origin::User, a)
            .unwrap();
        bus.execute_merging(Command::SetMasterPan { pan: 0.3 }, Origin::User, a)
            .unwrap();
        let both = |pan: f32, width: f32| Command::Batch {
            label_key: "cmd.controllers.gesture".into(),
            commands: vec![
                Command::SetMasterPan { pan },
                Command::SetMasterWidth { width },
            ],
        };
        bus.widen_merging(both(0.4, 1.5), Origin::User, a, b)
            .unwrap();
        bus.execute_merging(both(0.5, 1.7), Origin::User, b)
            .unwrap();
        assert_eq!(bus.history().len(), 1, "one step");
        assert_eq!(
            (bus.project().master.pan, bus.project().master.width),
            (0.5, 1.7)
        );
        bus.undo();
        assert_eq!(
            (bus.project().master.pan, bus.project().master.width),
            (m.pan, m.width)
        );
        bus.redo();
        assert_eq!(
            (bus.project().master.pan, bus.project().master.width),
            (0.5, 1.7)
        );
        // Sealing by another key leaves the step open; by its own, closes it.
        let c = MergeKey::of(&"c");
        bus.execute_merging(both(0.6, 1.8), Origin::User, c)
            .unwrap();
        bus.end_gesture_if(MergeKey::of(&"mouse"));
        bus.execute_merging(both(0.65, 1.8), Origin::User, c)
            .unwrap();
        assert_eq!(bus.history().len(), 2);
        bus.end_gesture_if(c);
        bus.execute_merging(both(0.7, 1.9), Origin::User, c)
            .unwrap();
        assert_eq!(bus.history().len(), 3);
    }
    /// Review 2, #20/#53: a gesture sealed while another step is on top is sealed all the
    /// same — an undo that brings it back to the top never lets a later turn merge into it.
    #[test]
    fn a_buried_gesture_is_sealed_by_its_key() {
        let mut bus = CommandBus::default();
        let k = MergeKey::of(&"knob");
        bus.execute_merging(Command::SetMasterPan { pan: 0.2 }, Origin::User, k)
            .unwrap();
        bus.execute(Command::SetMetronome { enabled: true }, Origin::User)
            .unwrap();
        bus.end_gesture_if(k);
        bus.undo();
        bus.execute_merging(Command::SetMasterPan { pan: 0.4 }, Origin::User, k)
            .unwrap();
        assert_eq!(bus.history().len(), 2, "two separate turns");
        bus.undo();
        assert_eq!(bus.project().master.pan, 0.2);
    }

    use crate::model::TrackId;

    #[test]
    fn accumulating_gesture_composes_and_undoes_completely() {
        let (mut bus, id) = bus_with_track();
        let clip = bus.new_clip("c", crate::time::Ticks::ZERO, crate::time::Ticks::beats(4));
        let cid = clip.id;
        bus.execute(
            Command::InsertClip {
                track: id,
                clip: Box::new(clip),
            },
            Origin::User,
        )
        .unwrap();
        let before = bus.project().clone();
        let key = MergeKey::of(&"take");
        for pitch in [60u8, 64, 67] {
            let n = bus.new_note(
                crate::time::Ticks::ZERO,
                crate::time::Ticks::QUARTER,
                pitch,
                100,
            );
            bus.execute_merging(
                Command::Batch {
                    label_key: "take".into(),
                    commands: vec![Command::InsertNotes {
                        track: id,
                        clip: cid,
                        notes: vec![n],
                    }],
                },
                Origin::User,
                key,
            )
            .unwrap();
        }
        bus.end_gesture();
        assert_eq!(bus.history().len(), 3); // track, clip, one take
        assert_eq!(bus.project().tracks[0].clips[0].notes.len(), 3);
        bus.undo();
        let mut now = bus.project().clone();
        now.next_id = before.next_id;
        assert_eq!(now, before);
        bus.redo();
        assert_eq!(bus.project().tracks[0].clips[0].notes.len(), 3);
    }

    fn bus_with_track() -> (CommandBus, TrackId) {
        let mut bus = CommandBus::default();
        let mut p = bus.project().clone();
        let t = p.make_track("Lead");
        let id = t.id;
        bus.reset(p);
        bus.execute(
            Command::InsertTrack {
                index: 0,
                track: Box::new(t),
            },
            Origin::User,
        )
        .unwrap();
        (bus, id)
    }

    #[test]
    fn undo_redo_roundtrip() {
        let (mut bus, id) = bus_with_track();
        let before = bus.project().clone();
        bus.execute(Command::SetTrackGain { id, gain_db: -6.0 }, Origin::User)
            .unwrap();
        assert_eq!(bus.project().tracks[0].gain_db, -6.0);
        assert!(bus.is_dirty());
        assert_eq!(bus.undo_label_key(), Some("cmd.set_track_gain"));
        bus.undo();
        assert_eq!(*bus.project(), before);
        bus.redo();
        assert_eq!(bus.project().tracks[0].gain_db, -6.0);
    }

    #[test]
    fn gesture_merges_into_one_step() {
        let (mut bus, id) = bus_with_track();
        let before = bus.project().clone();
        let key = MergeKey::of(&("gain", id));
        for g in [-1.0, -2.0, -3.0, -4.0] {
            bus.execute_merging(Command::SetTrackGain { id, gain_db: g }, Origin::User, key)
                .unwrap();
        }
        bus.end_gesture();
        assert_eq!(bus.history().len(), 2); // insert track + one merged gain step
        bus.undo();
        assert_eq!(*bus.project(), before);
        bus.redo();
        assert_eq!(bus.project().tracks[0].gain_db, -4.0);
    }

    #[test]
    fn different_origins_never_merge() {
        let (mut bus, id) = bus_with_track();
        let key = MergeKey::of(&"g");
        bus.execute_merging(
            Command::SetTrackGain { id, gain_db: -1.0 },
            Origin::User,
            key,
        )
        .unwrap();
        bus.execute_merging(Command::SetTrackGain { id, gain_db: -2.0 }, Origin::Ai, key)
            .unwrap();
        assert_eq!(bus.history().len(), 3);
    }

    #[test]
    fn rejected_commands_leave_no_trace_in_history() {
        let (mut bus, _) = bus_with_track();
        let n = bus.history().len();
        assert!(bus
            .execute(Command::RemoveTrack { id: TrackId(999) }, Origin::Mcp)
            .is_err());
        assert_eq!(bus.history().len(), n);
        assert_eq!(bus.audit_log().last().unwrap().kind, AuditKind::Rejected);
    }
}
