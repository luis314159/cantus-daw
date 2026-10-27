//! Writing automation while the song plays (ADR 0068) in the application: the global arm,
//! each track's mode, the pass from the transport's start to its stop, and its commit as
//! one undo step. The rules are `cantus_core::automation::write` (a pure [`Writer`]); here
//! is where they meet the frame, the bus and the engine.
//!
//! - **When.** A pass runs while the transport plays and either the arm is on and some
//!   track (or the master) is in Write, Touch or Latch, or a recording pass records
//!   automation (FL's *record automation*, round 28): then every control on a track in
//!   Read writes as **Latch** — the simple case, not a second path.
//! - **What the hand did** arrives as the commands it ran ([`CantusApp::record_tweak_at`],
//!   from `note_tweak` before each runs, and the controllers' own times); which controls
//!   are **held** is read from the bus each frame: an open gesture on top of the history
//!   is a drag, a plug-in window's gesture or a controller still moving.
//! - **What is heard** while it writes: `latch_recorded` hands the snapshot's lanes to
//!   `write_commit::live_lanes`; the snapshot is sent again whenever what a target hears
//!   changes (held, held still, gliding).
//! - **The commit** at the stop: the clips, the knobs put back, a Write track into Touch —
//!   folded with every move the hand made during the pass into **one undo step**
//!   (`CommandBus::execute_folding`).

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::automation::range::range_of;
use cantus_core::automation::write::{AutoMode, Live, Writer};
use cantus_core::automation::write_commit::{commit, heard_pos, live_lanes, Commit};
use cantus_core::{AutoTarget, Command, HistoryMark, Origin, Project, Ticks, TrackId};
use std::hash::{Hash, Hasher};

/// The application's write state.
#[derive(Debug, Default)]
pub struct WriteUi {
    /// The global arm (the top bar's): track modes other than Read write only while it is
    /// on. Off when the application starts — opening a project never writes by surprise.
    pub armed: bool,
    pass: Option<Pass>,
    /// What the engine was last told it hears for the pass's targets.
    live_sig: u64,
}

#[derive(Debug)]
struct Pass {
    writer: Writer,
    /// The project when the pass began: what the knobs are put back to, the old curves a
    /// touch glides back to.
    before: Box<Project>,
    mark: HistoryMark,
    /// The playhead at the last frame.
    last: Ticks,
    /// The engine handle's seeks at the last frame.
    seeks: u64,
    /// A recording pass records automation (Read writes as Latch).
    recording: bool,
}

impl WriteUi {
    /// Whether a pass is writing now.
    pub fn writing(&self) -> bool {
        self.pass.is_some()
    }

    /// What the pass has written so far, per target: its runs, oldest first.
    pub fn written(&self) -> Vec<(AutoTarget, Vec<cantus_core::automation::write::Run>)> {
        self.pass.as_ref().map_or_else(Vec::new, |p| {
            p.writer
                .written()
                .map(|(t, r)| (t.clone(), r.to_vec()))
                .collect()
        })
    }
}

/// Whether `cmd` only sets parameters (what a pass folds into its step).
/// [`sets_parameters`], or a recording pass's own take (ADR 0068: a pass that records
/// automation is one undo step with its take).
fn take_or_parameters(cmd: &Command) -> bool {
    matches!(cmd, Command::Batch { label_key, .. } if label_key == "cmd.record_take")
        || sets_parameters(cmd)
}

fn sets_parameters(cmd: &Command) -> bool {
    match cmd {
        Command::Batch { commands, .. } => commands.iter().all(sets_parameters),
        // A modulator's amount dragged in its panel (ADR 0057).
        Command::SetModulator { .. } => true,
        c => AutoTarget::value_of_command(c).is_some(),
    }
}

/// Every target a command sets.
fn targets_of(cmd: &Command, out: &mut Vec<AutoTarget>) {
    match cmd {
        Command::Batch { commands, .. } => commands.iter().for_each(|c| targets_of(c, out)),
        c => {
            if let Some(t) = AutoTarget::of_command(c) {
                out.push(t);
            }
        }
    }
}

impl CantusApp {
    /// The global arm.
    pub fn auto_armed(&self) -> bool {
        self.automation.write.armed
    }

    /// Arm or disarm writing. Disarming during a pass ends it there (its writing kept).
    pub fn set_auto_armed(&mut self, on: bool) {
        self.automation.write.armed = on;
        if !on
            && self
                .automation
                .write
                .pass
                .as_ref()
                .is_some_and(|p| !p.recording)
        {
            let at = self.engine.position();
            self.finish_write_pass(at, None);
        }
    }

    /// Whether a write pass is open (armed, playing, a track writing).
    pub fn write_pass_open(&self) -> bool {
        self.automation.write.pass.is_some()
    }

    /// End a write pass open now, here, into the project it rode (a recording's too): what
    /// comes before another project takes this one's place (integrated review #1).
    pub(crate) fn end_write_pass_here(&mut self) {
        let Some(last) = self.automation.write.pass.as_ref().map(|p| p.last) else {
            return;
        };
        let at = self.engine.position();
        self.finish_write_pass(last.max(at), None);
    }

    /// The mode a target's own track (or the master) is in.
    pub fn own_auto_mode(&self, target: &AutoTarget) -> AutoMode {
        let p = self.project();
        match target {
            AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth => {
                p.master.auto_mode
            }
            // An insert on the master is the master's.
            AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. }
                if p.effect_host(*effect).is_some_and(|h| h.is_master()) =>
            {
                p.master.auto_mode
            }
            _ => crate::automation::host_track(p, target, self.selected_track)
                .and_then(|t| p.track(t))
                .map_or(AutoMode::Read, |t| t.auto_mode),
        }
    }

    /// The mode a target writes in now: its track's when armed, else Latch while a
    /// recording pass records automation, else Read.
    pub fn auto_mode_now(&self, target: &AutoTarget) -> AutoMode {
        let own = self.own_auto_mode(target);
        if self.automation.write.armed && own.writes() {
            return own;
        }
        if self
            .automation
            .write
            .pass
            .as_ref()
            .is_some_and(|p| p.recording)
        {
            return AutoMode::Latch;
        }
        AutoMode::Read
    }

    /// Whether any track or the master writes when armed.
    pub fn any_writing_mode(&self) -> bool {
        let p = self.project();
        p.master.auto_mode.writes() || p.tracks.iter().any(|t| t.auto_mode.writes())
    }

    /// The window a pass writes in: the punch range when punch is on, else everything.
    fn write_window(&self) -> Option<(i64, i64)> {
        let r = &self.project().recording;
        r.punch
            .as_ref()
            .filter(|_| r.punch_enabled)
            .filter(|p| p.end > p.start)
            .map(|p| (p.start.0, p.end.0))
    }

    /// Begin a pass at the playhead (a recording one says so).
    fn begin_write_pass(&mut self, recording: bool) {
        let at = self.engine.position();
        if let Some(p) = self.automation.write.pass.as_mut() {
            p.recording |= recording;
            return;
        }
        let window = if recording {
            self.rec.pass.as_ref().map(|live| {
                let (a, b) = match live.plan.punch {
                    Some((a, b)) => (a.0, b.0),
                    None => (live.pass_start.0, i64::MAX),
                };
                (a.max(0), b)
            })
        } else {
            self.write_window()
        };
        let mut writer = Writer::new(at.0, window);
        // Write: every target a track in Write already automates holds where it is heard.
        if self.automation.write.armed {
            let lanes = self.auto_lanes();
            for lane in lanes.iter() {
                if self.own_auto_mode(&lane.target) == AutoMode::Write {
                    let pos = lane
                        .pos_at(at.0 as f64)
                        .unwrap_or_else(|| lane.range.pos(lane.base));
                    writer.arm(lane.target.clone(), at.0, pos);
                }
            }
        }
        self.automation.write.pass = Some(Pass {
            writer,
            before: Box::new(self.project().clone()),
            mark: self.bus.mark(),
            last: at,
            seeks: self.engine.seeks(),
            recording,
        });
        self.automation.write.live_sig = 0;
    }

    /// Once a frame, before anything heard in it is recorded: a pass begins, follows the
    /// playhead's jumps (a wrap: back in time; a seek: the engine handle's count — never a
    /// slow frame) and ends when the transport stops. The holds and the hands let go are
    /// [`Self::auto_write_settle`]'s, after the frame's controller messages.
    pub fn auto_write_frame(&mut self) {
        let playing = self.engine.is_playing();
        let pos = self.engine.position();
        let seeks = self.engine.seeks();
        let wanted = playing && self.automation.write.armed && self.any_writing_mode();
        match self.automation.write.pass.as_ref() {
            None if wanted => self.begin_write_pass(false),
            // A recording pass ends with the recording (`commit_recorded_automation_in`).
            Some(p) if !p.recording && !wanted => {
                let last = p.last;
                self.finish_write_pass(last.max(pos), None);
                return;
            }
            _ => {}
        }
        self.sync_record_window();
        let Some(pass) = self.automation.write.pass.as_mut() else {
            return;
        };
        if !playing {
            return;
        }
        let jumped = pos < pass.last || seeks != pass.seeks;
        pass.seeks = seeks;
        if jumped {
            pass.writer.jump(pos.0);
        }
        pass.last = pos;
        pass.writer.now = Some(pos.0);
    }

    /// Once a frame, after the frame's controller messages and plug-in windows' moves: the
    /// hands let go begin their glide, latched and written targets hold their value up to
    /// the playhead.
    pub fn auto_write_settle(&mut self) {
        if !self.engine.is_playing() || self.automation.write.pass.is_none() {
            return;
        }
        let pos = self.engine.position();
        let held = self.held_targets();
        let glide = self.settings.touch_glide_seconds();
        // A hosted parameter with no curve glides back to what the project held for it.
        let plugin_base: Vec<(AutoTarget, f32)> = self
            .automation
            .write
            .pass
            .as_ref()
            .map(|p| {
                p.writer
                    .targets
                    .keys()
                    .filter(|t| t.is_plugin())
                    .filter_map(|t| Some((t.clone(), self.stored_pos(&p.before, t)?)))
                    .collect()
            })
            .unwrap_or_default();
        let map = self.project().tempo_map();
        let Some(pass) = self.automation.write.pass.as_mut() else {
            return;
        };
        let before = &pass.before;
        let glide_end = |t: i64| map.tick_after_seconds(t as f64, glide).round() as i64;
        let old = |target: &AutoTarget, t: i64| {
            let fallback = plugin_base
                .iter()
                .find(|(x, _)| x == target)
                .map_or(0.5, |(_, v)| *v);
            heard_pos(before, target, t, fallback)
        };
        pass.writer
            .frame(pos.0, &|t| held.contains(t), &glide_end, &old);
        self.resync_if_live_changed();
    }

    /// The targets a hand holds now: the open gesture on top of the history, while a
    /// pointer is down (a drag), a controller's gesture is still moving or a plug-in's window
    /// holds its gesture. A wheel step, an arrow key or a double-click leaves a merge open
    /// with no hand on anything: a touch let go at once.
    fn held_targets(&self) -> Vec<AutoTarget> {
        let mut out = Vec::new();
        let Some(key) = self.bus.open_gesture() else {
            return out;
        };
        let Some(top) = self.bus.history().last() else {
            return out;
        };
        let pointer = self
            .theme_ctx
            .as_ref()
            .is_none_or(|c| c.input(|i| i.pointer.any_down()));
        let controller = self.controllers.gesture.key == Some(key);
        let plugin = matches!(
            top.command,
            Command::SetPluginParam { .. } | Command::SetInstrumentPluginParam { .. }
        );
        if pointer || controller || plugin {
            targets_of(&top.command, &mut out);
        }
        out
    }

    /// Send the snapshot again when what a written target hears changed.
    fn resync_if_live_changed(&mut self) {
        let Some(pass) = self.automation.write.pass.as_ref() else {
            return;
        };
        let at = self.engine.position().0;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for target in pass.writer.targets.keys() {
            target.hash(&mut h);
            match pass.writer.live_at(target, at) {
                Live::Curve => 0u8.hash(&mut h),
                Live::Hand => 1u8.hash(&mut h),
                Live::Hold(v) => (2u8, v.to_bits()).hash(&mut h),
                Live::Glide { a, b, from, to } => {
                    (3u8, a, b, from.to_bits(), to.to_bits()).hash(&mut h)
                }
            }
        }
        let sig = h.finish();
        if sig != self.automation.write.live_sig {
            self.automation.write.live_sig = sig;
            self.force_resync();
        }
    }

    /// What the engine hears for the pass's targets: the snapshot's lanes changed by
    /// `write_commit::live_lanes`. Nothing changes outside a pass.
    pub fn live_written_lanes(&self, snapshot: &mut std::sync::Arc<cantus_core::Snapshot>) {
        let Some(pass) = self.automation.write.pass.as_ref() else {
            return;
        };
        if pass.writer.targets.is_empty() {
            return;
        }
        let s = std::sync::Arc::make_mut(snapshot);
        live_lanes(
            &mut s.automation,
            &pass.writer,
            self.project(),
            &pass.before,
            self.engine.position().0,
        );
    }

    /// Whether a pass is writing `target` and the engine follows the hand or a held value
    /// for it (what a controller's feedback reads as the project's value).
    pub fn writing_target(&self, target: &AutoTarget) -> bool {
        self.automation.write.pass.as_ref().is_some_and(|p| {
            matches!(
                p.writer.live_at(target, self.engine.position().0),
                Live::Hand | Live::Hold(_)
            )
        })
    }

    /// A hand put `target` at `pos` (0…1 of its travel) at `tick`: written when a pass
    /// writes it in its mode now.
    pub fn write_value(&mut self, target: AutoTarget, pos: f32, tick: i64) {
        if self.automation.write.pass.is_none() {
            return;
        }
        self.sync_record_window();
        let mode = self.auto_mode_now(&target);
        if !mode.writes() {
            return;
        }
        if let Some(p) = self.automation.write.pass.as_mut() {
            p.writer.value(target, mode, tick, pos);
        }
        self.resync_if_live_changed();
    }

    /// The position (0…1) a command puts its target at, with the target.
    pub fn command_pos(&self, cmd: &Command) -> Option<(AutoTarget, f32)> {
        let (target, v) = AutoTarget::value_of_command(cmd)?;
        let pos = if target.is_plugin() {
            let (_, info) = self.plugin_param(&target)?;
            let span = info.max - info.min;
            if span.abs() > 0.0 {
                ((v - info.min) / span).clamp(0.0, 1.0) as f32
            } else {
                0.0
            }
        } else {
            range_of(self.project(), &target)?.pos(v as f32)
        };
        Some((target, pos))
    }

    /// A recording pass began: it records automation when its options say so (round 28).
    pub fn begin_recorded_write(&mut self) {
        if self.project().recording.record_automation {
            self.begin_write_pass(true);
        } else if let Some(p) = self.automation.write.pass.as_mut() {
            p.recording = false;
        }
    }

    /// End the pass at `stop` (clamped to `end`, a punch-out) and commit what it wrote:
    /// one undo step with every move the hand made during it. Says whether it wrote.
    pub fn finish_write_pass(&mut self, stop: Ticks, end: Option<Ticks>) -> bool {
        let Some(mut pass) = self.automation.write.pass.take() else {
            return false;
        };
        self.automation.write.live_sig = 0;
        let stop = end.map_or(stop, |e| stop.min(e));
        pass.writer.finish(stop.0);
        // The written targets go back to their clips, whatever is committed.
        self.force_resync();
        if pass.writer.is_empty() {
            return false;
        }
        let selected = self.selected_track;
        let wrote: Vec<TrackId> = self
            .project()
            .tracks
            .iter()
            .filter(|t| t.auto_mode == AutoMode::Write)
            .map(|t| t.id)
            .collect();
        let master_wrote =
            self.automation.write.armed && self.project().master.auto_mode == AutoMode::Write;
        let wrote = if self.automation.write.armed {
            wrote
        } else {
            Vec::new()
        };
        let names: std::collections::BTreeMap<AutoTarget, String> = pass
            .writer
            .written()
            .map(|(t, _)| (t.clone(), self.target_name(t)))
            .collect();
        let mut next = self.bus.project().next_id;
        let (commands, n) = {
            let project = self.bus.project();
            let host = |t: &AutoTarget| crate::automation::host_track(project, t, selected);
            let name = |t: &AutoTarget| names.get(t).cloned().unwrap_or_default();
            let mut ids = || {
                let id = next;
                next += 1;
                id
            };
            let mut c = Commit {
                before: &pass.before,
                host: &host,
                name: &name,
                ids: &mut ids,
                wrote: &wrote,
                master_wrote,
            };
            commit(project, &pass.writer, &mut c)
        };
        self.bus.reserve_ids(next);
        if commands.is_empty() {
            return false;
        }
        let fold: &dyn Fn(&Command) -> bool = if pass.recording {
            &take_or_parameters
        } else {
            &sets_parameters
        };
        let batch = Command::Batch {
            // A recording pass's step is its take's, with the automation in it.
            label_key: if pass.recording {
                "cmd.record_take".into()
            } else {
                "automodes.cmd.write_pass".into()
            },
            commands,
        };
        match self
            .bus
            .execute_folding(batch, Origin::User, pass.mark, fold)
        {
            Ok(_) => {
                self.set_status(tf("automodes.status.written", &[("n", &n)]));
                true
            }
            Err(e) => {
                log::warn!("write pass: {e}");
                self.set_error(t("automodes.status.write_failed"));
                false
            }
        }
    }

    /// Put every track (and the master) in `mode`: the top bar's *set all tracks to…*.
    /// One undo step.
    pub fn set_all_auto_modes(&mut self, mode: AutoMode) -> bool {
        let p = self.project();
        let mut commands: Vec<Command> = p
            .tracks
            .iter()
            .filter(|t| t.auto_mode != mode)
            .map(|t| Command::SetTrackAutoMode { track: t.id, mode })
            .collect();
        if p.master.auto_mode != mode {
            commands.push(Command::SetMasterAutoMode { mode });
        }
        if commands.is_empty() {
            return false;
        }
        self.exec(Command::Batch {
            label_key: "automodes.cmd.set_all".into(),
            commands,
        })
    }
}

impl CantusApp {
    /// The selected track's mode, the next of Read, Touch, Latch, Write (the order a
    /// session goes through: play, try, keep, overwrite).
    pub fn cycle_track_auto_mode(&mut self) {
        let Some(track) = self.selected_track else {
            return;
        };
        let Some(mode) = self.project().track(track).map(|t| t.auto_mode) else {
            return;
        };
        let next = match mode {
            AutoMode::Read => AutoMode::Touch,
            AutoMode::Touch => AutoMode::Latch,
            AutoMode::Latch => AutoMode::Write,
            AutoMode::Write => AutoMode::Read,
        };
        if self.exec(Command::SetTrackAutoMode { track, mode: next }) {
            self.set_status(tf(
                "automodes.status.mode",
                &[("mode", &t(next.label_key()))],
            ));
        }
    }
}

impl CantusApp {
    /// A recording pass writes from where the pass being played began — the record start,
    /// the loop's start once it came round (ADR 0057's rule: a count-in inside the loop is
    /// not the pass) — or the punch-in, to the punch-out.
    fn sync_record_window(&mut self) {
        let Some(live) = self.rec.pass.as_ref() else {
            return;
        };
        let window = match live.plan.punch {
            Some((a, b)) => (a.0.max(0), b.0),
            None => (live.pass_start.0.max(0), i64::MAX),
        };
        if let Some(p) = self.automation.write.pass.as_mut().filter(|p| p.recording) {
            p.writer.window = Some(window);
        }
    }
}

impl CantusApp {
    /// Where a hosted parameter stood in `project` (0…1 of the plug-in's range), from the
    /// value the project stored for it; `None` when it stored none or the plug-in says
    /// nothing of the parameter.
    fn stored_pos(&self, project: &Project, target: &AutoTarget) -> Option<f32> {
        let stored = match target {
            AutoTarget::Plugin { effect, id } => {
                let h = project.effect_host(*effect)?;
                project
                    .effect(h, *effect)?
                    .spec
                    .external
                    .as_deref()?
                    .params
                    .get(id)
                    .copied()
            }
            AutoTarget::PluginInstrument { track, id } => project
                .track(*track)?
                .instrument
                .external()?
                .params
                .get(id)
                .copied(),
            _ => None,
        }?;
        let (_, info) = self.plugin_param(target)?;
        let span = info.max - info.min;
        (span.abs() > 0.0).then(|| ((stored - info.min) / span).clamp(0.0, 1.0) as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review #5: a wheel step or an arrow key leaves its merge open with no hand on the
    /// control: in Touch it is let go at once (it glides back), not held like a drag.
    #[test]
    fn a_merge_left_open_without_a_hand_is_no_hold() {
        let mut a = CantusApp::from_parts(
            Some(crate::starter_project()),
            cantus_engine::EngineHandle::dummy(),
            false,
        );
        // A context: the pointer is not down.
        a.theme_ctx = Some(egui::Context::default());
        let t = a.project().tracks[0].id;
        assert!(a.exec(Command::SetTrackAutoMode {
            track: t,
            mode: AutoMode::Touch
        }));
        a.set_auto_armed(true);
        a.engine.set_heard_for_tests(Ticks(960), true);
        a.auto_write_frame();
        // Ctrl+wheel over the pan: a merged step, never sealed.
        a.exec_merging(
            Command::SetTrackPan { id: t, pan: 0.3 },
            cantus_core::MergeKey::of(&"wheel"),
        );
        a.auto_write_settle();
        a.engine.set_heard_for_tests(Ticks(1000), true);
        a.auto_write_frame();
        a.auto_write_settle();
        assert!(
            !a.writing_target(&AutoTarget::TrackPan(t)),
            "let go: it glides back"
        );
    }
}
