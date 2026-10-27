//! The command bus: owns the project, executes commands, keeps undo/redo history and an
//! audit log. There is exactly one of these per open project (ADR 0004).

use crate::command::{Command, CommandError, Origin};
use crate::model::Project;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
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
    /// A tempo gesture still merging (review 4): its own commands so far, without the
    /// window edits `tempo::keep` added — what each later frame rebuilds the step from,
    /// so every frame is measured from where the gesture began.
    gesture: Option<Command>,
    /// The state this step leaves the project in, as a number of its own (round 31, ADR
    /// 0072): never reused, kept through undo and redo, renewed when a gesture merges more
    /// into the step — the history panel marks the step a save wrote by it.
    step: u64,
}

impl HistoryEntry {
    /// The state this step leaves the project in ([`CommandBus::current_step`]).
    pub fn step(&self) -> u64 {
        self.step
    }
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
    /// Steps pushed onto the undo stack since the bus was made, and undos and redos done:
    /// what a [`HistoryMark`] counts from (ADR 0068).
    pushed: u64,
    turns: u64,
    /// The next step number ([`HistoryEntry::step`]); 0 is the first state of a bus.
    next_step: u64,
    /// The state below the oldest step kept (the project as it was opened, or as it was
    /// before the steps the history let go of).
    base_step: u64,
    /// The state the file holds, if one the history can reach (round 31).
    saved_step: Option<u64>,
    /// Recent revisions and the state each was: a save written off the interface's thread
    /// names the revision it took ([`Self::mark_saved_at`]).
    states: VecDeque<(u64, u64)>,
    /// Automatic crossfades (ADR 0070): an edit that makes two audio clips overlap gives
    /// the overlap a crossfade on this curve, in the same step. `None` (a bus's default;
    /// the application sets its user's choice) leaves overlaps as the edit made them.
    auto_xfade: Option<crate::crossfade::XfadeCurve>,
}

/// A place in the history (`CommandBus::mark`): what a write pass folds its steps from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HistoryMark {
    pushed: u64,
    turns: u64,
}

/// How many recent revisions [`CommandBus::mark_saved_at`] can name.
const STATES_KEPT: usize = 1024;

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
            pushed: 0,
            turns: 0,
            next_step: 1,
            base_step: 0,
            saved_step: Some(0),
            states: VecDeque::from([(0, 0)]),
            auto_xfade: None,
        }
    }

    /// Turn automatic crossfades on (with the curve new ones take) or off (ADR 0070).
    pub fn set_auto_crossfades(&mut self, curve: Option<crate::crossfade::XfadeCurve>) {
        self.auto_xfade = curve;
    }

    /// Whether automatic crossfades are on, and their curve.
    pub fn auto_crossfades(&self) -> Option<crate::crossfade::XfadeCurve> {
        self.auto_xfade
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
        self.saved_step = Some(self.current_step());
    }

    /// Mark `revision` as the one in the file: a save written off the interface's thread
    /// holds the revision it was taken at, and an edit made while it was written keeps
    /// the project dirty. A revision from the future is the present.
    pub fn mark_saved_at(&mut self, revision: u64) {
        self.saved_revision = revision.min(self.revision);
        let at = self.saved_revision;
        self.saved_step = self
            .states
            .iter()
            .rev()
            .find(|(r, _)| *r == at)
            .map(|(_, s)| *s);
    }

    /// [`Self::mark_saved_at`] for a save that named the state it took as well
    /// ([`Self::current_step`] when it took it): the mark is kept however many revisions
    /// went by meanwhile, unless no step holds that state any more.
    pub fn mark_saved_state(&mut self, revision: u64, step: u64) {
        self.saved_revision = revision.min(self.revision);
        self.saved_step = Some(step);
        self.forget_unreachable_save();
    }

    /// The state the project is in now: the step on top's, or the base's (round 31).
    pub fn current_step(&self) -> u64 {
        self.undo.last().map_or(self.base_step, |e| e.step)
    }

    /// The state below the oldest step kept: the project as it was opened or begun, or as
    /// it was before the steps the history let go of.
    pub fn base_step(&self) -> u64 {
        self.base_step
    }

    /// The state the file holds — `None` when no step the history holds is that state (a
    /// gesture merged more into the step that was saved, the steps after it were undone
    /// and replaced, or it fell off the history's end).
    pub fn saved_step(&self) -> Option<u64> {
        self.saved_step
    }

    fn fresh_step(&mut self) -> u64 {
        let s = self.next_step;
        self.next_step += 1;
        s
    }

    /// The revision just made, and the state it is.
    fn note_state(&mut self) {
        let state = self.current_step();
        self.states.push_back((self.revision, state));
        while self.states.len() > STATES_KEPT {
            self.states.pop_front();
        }
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
            top.gesture = None;
        }
    }

    /// The gesture whose step is on top, still open (a drag going on), if any.
    pub fn open_gesture(&self) -> Option<MergeKey> {
        self.undo.last().and_then(|top| top.merge_key)
    }

    /// Whether the step on top is `key`'s gesture, still open (a drag going on).
    pub fn merging(&self, key: MergeKey) -> bool {
        self.undo
            .last()
            .is_some_and(|top| top.merge_key == Some(key))
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
            e.gesture = None;
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
        // A gesture that moves the tempo is rebuilt from where it began (review 4): the
        // widened command sets every target from scratch, the windows kept from there.
        let tempo = self.undo.last().is_some_and(|top| top.gesture.is_some())
            || crate::tempo::keep::changes_tempo(&command);
        if tempo {
            let label_key = command.label_key().to_string();
            return self.rebase(command, origin, Some(to), label_key, true);
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
        self.renew_top();
        self.audit_push(origin, label_key, AuditKind::Execute);
        Ok(())
    }

    /// Merge `command` into the tempo gesture on top by rebuilding it from where the gesture
    /// began (review 4): the step's inverse takes the song back there (exactly), the
    /// gesture's own commands — the earlier ones and this, or this alone when it sets every
    /// target from scratch (`replace`, or it supersedes them) — run again, and the window
    /// edits that keep the audio are measured once, from that state. A hundred frames are
    /// one edit, a drag back to its start leaves every window as it was, and the step holds
    /// one fix however long the gesture.
    fn rebase(
        &mut self,
        command: Command,
        origin: Origin,
        merge_key: Option<MergeKey>,
        label_key: String,
        replace: bool,
    ) -> Result<(), CommandError> {
        let top = self.undo.last().expect("a step to merge into");
        let earlier = top.gesture.clone().unwrap_or_else(|| top.command.clone());
        let step_label = if replace {
            label_key.clone()
        } else {
            top.label_key.clone()
        };
        let raw = if replace || command.supersedes(&earlier) {
            command
        } else {
            compose(&step_label, earlier, command, false)
        };
        let back = top.inverse.clone();
        if let Err(e) = back.apply(&mut self.project) {
            // The step's inverse always applies (undo relies on it); if not, nothing moved.
            self.audit_push(origin, label_key, AuditKind::Rejected);
            return Err(e);
        }
        let (command, inverse) = match self.apply_keeping_audio(raw.clone(), &step_label) {
            Ok(both) => both,
            Err(e) => {
                // Put the gesture back as it stood.
                let again = self.undo.last().expect("checked").command.clone();
                let _ = again.apply(&mut self.project);
                self.audit_push(origin, label_key, AuditKind::Rejected);
                return Err(e);
            }
        };
        self.revision += 1;
        self.redo.clear();
        let top = self.undo.last_mut().expect("checked");
        top.command = command;
        top.inverse = inverse;
        top.gesture = Some(raw);
        top.label_key = step_label;
        top.merge_key = merge_key;
        self.renew_top();
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
        // The overlaps before an edit that can make or end one (ADR 0070): new ones are
        // crossed when automatic crossfades are on, ended ones lose their stale curve.
        let overlaps =
            crate::crossfade::may_overlap(&command).then(|| crate::crossfade::pairs(&self.project));
        let inverse = command.clone().apply(&mut self.project)?;
        let fix: Vec<Command> = kept
            .map(|k| crate::tempo::keep::after(&k, &self.project))
            .unwrap_or_default();
        let (mut command, mut inverse) = (command, inverse);
        if !fix.is_empty() {
            (command, inverse) = self.apply_fix(label_key, command, inverse, fix)?;
        }
        // The crossfades, measured once the windows are where the edit (and the tempo's
        // fix) left them: each sets a region as it is now.
        if let Some(before) = overlaps {
            let fix = crate::crossfade::fresh(&before, &self.project, self.auto_xfade);
            if !fix.is_empty() {
                (command, inverse) = self.apply_fix(label_key, command, inverse, fix)?;
            }
        }
        Ok((command, inverse))
    }

    /// Apply `fix` after `command` (already applied, `inverse` its inverse), one step of
    /// the two; if it cannot apply, the command is taken back — never half an edit.
    fn apply_fix(
        &mut self,
        label_key: &str,
        command: Command,
        inverse: Command,
        fix: Vec<Command>,
    ) -> Result<(Command, Command), CommandError> {
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
                let _ = inverse.apply(&mut self.project);
                Err(e)
            }
        }
    }

    /// The step on top changed (a gesture merged more into it): a state of its own now, and
    /// a save of the state before is no longer reachable from it.
    fn renew_top(&mut self) {
        let step = self.fresh_step();
        if let Some(top) = self.undo.last_mut() {
            top.step = step;
        }
        self.forget_unreachable_save();
        self.note_state();
    }

    /// A saved state no step holds any more (it was undone and replaced, or renewed by a
    /// merge) is out of reach: nothing is marked saved.
    fn forget_unreachable_save(&mut self) {
        let reachable = |s: u64| {
            s == self.base_step
                || self.undo.iter().any(|e| e.step == s)
                || self.redo.iter().any(|e| e.step == s)
        };
        if self.saved_step.is_some_and(|s| !reachable(s)) {
            self.saved_step = None;
        }
    }

    fn execute_inner(
        &mut self,
        command: Command,
        origin: Origin,
        merge_key: Option<MergeKey>,
    ) -> Result<(), CommandError> {
        let label_key = command.label_key().to_string();
        let tempo = crate::tempo::keep::changes_tempo(&command);
        if let (Some(key), Some(top)) = (merge_key, self.undo.last()) {
            if top.merge_key == Some(key)
                && top.origin == origin
                && (top.gesture.is_some() || tempo)
            {
                return self.rebase(command, origin, merge_key, label_key, false);
            }
        }
        // A tempo gesture's own commands, for its later frames.
        let gesture = (merge_key.is_some() && tempo).then(|| command.clone());
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
        match fresh {
            Some((command, inverse)) => {
                let step = self.fresh_step();
                self.pushed += 1;
                self.undo.push(HistoryEntry {
                    label_key: label_key.clone(),
                    command,
                    inverse,
                    origin,
                    merge_key,
                    gesture,
                    step,
                });
                if self.undo.len() > self.max_history {
                    let excess = self.undo.len() - self.max_history;
                    if let Some(last) = self.undo.drain(..excess).next_back() {
                        self.base_step = last.step;
                    }
                }
                self.forget_unreachable_save();
                self.note_state();
            }
            // Merged into the step on top.
            None => self.renew_top(),
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
        self.turns += 1;
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
                    gesture: None,
                    step: entry.step,
                });
                self.note_state();
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
        self.turns += 1;
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
                    gesture: None,
                    step: entry.step,
                });
                self.note_state();
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

    /// Where the history stands now, for [`Self::execute_folding`].
    pub fn mark(&self) -> HistoryMark {
        HistoryMark {
            pushed: self.pushed,
            turns: self.turns,
        }
    }

    /// Execute `command` as a step, then fold into it every step pushed since `mark` — all
    /// on top of the stack — when nothing was undone or redone since and `fold` accepts
    /// each: one step whose command replays them all in order and whose inverse takes them
    /// all back (a write pass and the hand's moves it recorded, ADR 0068). Says whether it
    /// folded; a step it may not fold stays a step of its own.
    pub fn execute_folding(
        &mut self,
        command: Command,
        origin: Origin,
        mark: HistoryMark,
        fold: &dyn Fn(&Command) -> bool,
    ) -> Result<bool, CommandError> {
        self.execute_inner(command, origin, None)?;
        let since = (self.pushed - mark.pushed) as usize;
        let n = since.saturating_sub(1);
        let foldable = mark.turns == self.turns
            && n > 0
            && since <= self.undo.len()
            && self.undo[self.undo.len() - since..self.undo.len() - 1]
                .iter()
                .all(|e| fold(&e.command));
        if !foldable {
            return Ok(false);
        }
        let mut steps = self.undo.split_off(self.undo.len() - since);
        let top = steps.pop().expect("the command's own step");
        // The state the pass leaves is the one its own step left (round 31, ADR 0072).
        let step = top.step;
        let label_key = top.label_key.clone();
        let mut commands: Vec<Command> = steps.iter().map(|e| e.command.clone()).collect();
        commands.push(top.command);
        let mut inverses = vec![top.inverse];
        inverses.extend(steps.into_iter().rev().map(|e| e.inverse));
        self.undo.push(HistoryEntry {
            label_key: label_key.clone(),
            command: Command::Batch {
                label_key: label_key.clone(),
                commands,
            },
            inverse: Command::Batch {
                label_key,
                commands: inverses,
            },
            origin,
            merge_key: None,
            gesture: None,
            step,
        });
        Ok(true)
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
        self.turns += 1;
        self.undo.clear();
        self.redo.clear();
        self.revision += 1;
        self.saved_revision = self.revision;
        self.base_step = self.fresh_step();
        self.saved_step = Some(self.base_step);
        self.states.clear();
        self.note_state();
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

    /// ADR 0068: a write pass folds the hand's steps since its mark into its own — one undo
    /// step that takes everything back — and folds nothing after an undo, or over a step
    /// that is not a parameter's.
    #[test]
    fn a_pass_folds_the_steps_since_its_mark() {
        let params = |c: &Command| matches!(c, Command::SetMasterGain { .. });
        let mut bus = CommandBus::default();
        let before = bus.project().clone();
        let mark = bus.mark();
        bus.execute(Command::SetMasterGain { gain_db: -3.0 }, Origin::User)
            .unwrap();
        bus.execute(Command::SetMasterGain { gain_db: -6.0 }, Origin::User)
            .unwrap();
        let folded = bus
            .execute_folding(
                Command::SetMetronome { enabled: true },
                Origin::User,
                mark,
                &params,
            )
            .unwrap();
        assert!(folded);
        assert_eq!(bus.history().len(), 1);
        assert_eq!(bus.project().master.gain_db, -6.0);
        bus.undo().unwrap();
        assert_eq!(bus.project(), &before);
        bus.redo().unwrap();
        assert_eq!(bus.project().master.gain_db, -6.0);
        assert!(bus.project().metronome);

        // An undo since the mark: nothing folds.
        let mut bus = CommandBus::default();
        let mark = bus.mark();
        bus.execute(Command::SetMasterGain { gain_db: -3.0 }, Origin::User)
            .unwrap();
        bus.undo();
        bus.execute(Command::SetMasterGain { gain_db: -1.0 }, Origin::User)
            .unwrap();
        let folded = bus
            .execute_folding(
                Command::SetMetronome { enabled: true },
                Origin::User,
                mark,
                &params,
            )
            .unwrap();
        assert!(!folded);
        assert_eq!(bus.history().len(), 2);

        // A step that is no parameter's: nothing folds.
        let mut bus = CommandBus::default();
        let mark = bus.mark();
        bus.execute(Command::SetMasterGain { gain_db: -3.0 }, Origin::User)
            .unwrap();
        bus.execute(Command::SetTempo { bpm: 99.0 }, Origin::User)
            .unwrap();
        let folded = bus
            .execute_folding(
                Command::SetMetronome { enabled: true },
                Origin::User,
                mark,
                &params,
            )
            .unwrap();
        assert!(!folded);
        assert_eq!(bus.history().len(), 3);
    }

    /// Round 31 (ADR 0072): each state the history holds has a number of its own — kept
    /// through undo and redo — and the saved one is known while a step holds it: through
    /// undo past it and redo back, a save written off the thread at an older revision, a
    /// gesture that merges more into it (gone), and new steps over undone ones (gone).
    #[test]
    fn the_saved_state_is_known_while_the_history_holds_it() {
        let mut bus = CommandBus::default();
        let pan = |p: f32| Command::SetMasterPan { pan: p };
        assert_eq!(
            bus.saved_step(),
            Some(bus.base_step()),
            "a new bus is its file"
        );
        bus.execute(pan(0.1), Origin::User).unwrap();
        bus.execute(pan(0.2), Origin::User).unwrap();
        let second = bus.current_step();
        bus.mark_saved();
        assert_eq!(bus.saved_step(), Some(second));
        bus.execute(pan(0.3), Origin::User).unwrap();
        let third = bus.current_step();
        assert_ne!(third, second);
        // Undo below the save and redo back: the same numbers.
        bus.undo();
        assert_eq!(bus.current_step(), second);
        bus.undo();
        assert_eq!(bus.redo_history().len(), 2);
        assert_eq!(bus.saved_step(), Some(second), "held by a step to redo");
        bus.redo();
        bus.redo();
        assert_eq!(bus.current_step(), third);
        // A save that took the revision of the second state, landing now.
        let rev = bus.revision();
        bus.execute(pan(0.4), Origin::User).unwrap();
        bus.mark_saved_at(rev);
        assert_eq!(bus.saved_step(), Some(third));
        assert!(bus.is_dirty());
        // A gesture merging more into the saved step: that state is gone.
        let key = MergeKey::of(&"drag");
        bus.execute_merging(pan(0.5), Origin::User, key).unwrap();
        bus.mark_saved();
        let saved = bus.current_step();
        bus.execute_merging(pan(0.6), Origin::User, key).unwrap();
        assert_ne!(bus.current_step(), saved, "a merge is a state of its own");
        assert_eq!(bus.saved_step(), None);
        // Undone steps replaced by a new one: the saved one among them is gone.
        bus.end_gesture();
        bus.mark_saved();
        bus.undo();
        bus.execute(pan(0.7), Origin::User).unwrap();
        assert_eq!(bus.saved_step(), None);
        // A reset is its own file.
        bus.reset(Project::default());
        assert_eq!(bus.saved_step(), Some(bus.base_step()));
        assert_eq!(bus.current_step(), bus.base_step());
    }

    /// Review 7: a save that lands after more revisions than the bus remembers still marks
    /// the step it took, named when it took it.
    #[test]
    fn a_long_save_keeps_its_mark() {
        let mut bus = CommandBus::default();
        bus.execute(Command::SetMasterPan { pan: 0.1 }, Origin::User)
            .unwrap();
        let (rev, step) = (bus.revision(), bus.current_step());
        for i in 0..(STATES_KEPT + 50) {
            bus.execute(
                Command::SetMasterWidth {
                    width: 1.0 + (i % 7) as f32 * 0.01,
                },
                Origin::User,
            )
            .unwrap();
        }
        bus.mark_saved_at(rev);
        assert_eq!(bus.saved_step(), None, "the revision is out of memory");
        bus.mark_saved_state(rev, step);
        assert_eq!(bus.saved_step(), Some(step));
        assert!(bus.is_dirty());
    }

    /// The history's oldest steps let go of: the base is the state before the oldest kept.
    #[test]
    fn the_base_follows_the_history_s_end() {
        let mut bus = CommandBus {
            max_history: 3,
            ..CommandBus::default()
        };
        let mut steps = Vec::new();
        for i in 0..5 {
            bus.execute(
                Command::SetMasterPan {
                    pan: i as f32 * 0.1,
                },
                Origin::User,
            )
            .unwrap();
            steps.push(bus.current_step());
        }
        assert_eq!(bus.history().len(), 3);
        assert_eq!(bus.base_step(), steps[1]);
        assert_eq!(bus.saved_step(), None, "the opened state fell off the end");
    }
}
