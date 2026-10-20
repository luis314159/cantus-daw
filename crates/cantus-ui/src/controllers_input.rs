//! The controllers' frame (round 29, ADR 0062 §1, §3): what the tap heard becomes what it
//! asks — a learnt control while one is armed, otherwise each mapping's drive — and the
//! moves become commands: **one command per target per frame** (the newest value), merged
//! into one undo step per gesture (a gesture ends after [`GESTURE_IDLE`] without a message;
//! two knobs moved at once are one step, a batch of both), as a mouse drag is. A mapped move
//! while a pass records is written into the pass's automation **at the message's own time**.

use crate::app::CantusApp;
use crate::controllers::{decode, Key, Scope};
use cantus_core::controller::{Drive, Mapping, MappingTarget, GESTURE_LABEL};
use cantus_core::{AutoTarget, Command, MergeKey, Origin};
use cantus_engine::midi::{FilterSet, Heard};
use std::time::{Duration, Instant};

/// After this long without a message, a controller gesture is sealed into its undo step.
pub const GESTURE_IDLE: Duration = Duration::from_millis(500);

/// The gesture going on.
#[derive(Debug, Default)]
pub struct Gesture {
    /// The targets it moved, in the order they came: its undo key is made of them.
    pub targets: Vec<MappingTarget>,
    /// Its last move.
    pub last: Option<Instant>,
}

/// Where a message `age` seconds old was heard, the playhead being at tick `now`: through
/// the tempo map, so a knob turned across a ramp lands where it was turned.
pub fn tick_of_message(map: &cantus_core::TempoMap, now: f64, age: f64) -> i64 {
    if age.is_nan() || age <= 0.0 {
        return now.round() as i64;
    }
    map.tick_at_seconds(map.seconds_at(now) - age).round() as i64
}

/// One target's newest value this frame.
struct Pending {
    target: MappingTarget,
    pos: f32,
    cmd: Command,
    keys: Vec<Key>,
}

impl CantusApp {
    /// Every frame: publish the filter the maps ask for, read what the ports heard, learn
    /// or drive, seal an idle gesture, send feedback. Interface thread.
    pub fn controller_input(&mut self) {
        self.publish_controller_filter();
        let mut buf = std::mem::take(&mut self.controllers.buf);
        buf.clear();
        if let Some(inputs) = self.midi_inputs.as_mut() {
            inputs.drain(&mut buf);
        }
        // Esc lets go of a learn in progress.
        if self.controllers.learning.is_some() {
            let esc = self.theme_ctx.as_ref().is_some_and(|c| {
                c.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            });
            if esc {
                self.cancel_learning();
            }
        }
        self.controller_heard(&buf);
        self.controllers.buf = buf;
        self.controller_feedback();
    }

    /// What the tap heard, in order (the tests drive this directly).
    pub fn controller_heard(&mut self, heard: &[Heard]) {
        let mut pending: Vec<Pending> = Vec::new();
        for h in heard {
            if self.controllers.learning.is_some() {
                self.learn_heard(h);
            } else {
                self.drive_heard(h, &mut pending);
            }
        }
        self.learn_tick();
        self.apply_moves(pending);
        if self
            .controllers
            .gesture
            .last
            .is_some_and(|t| t.elapsed() >= GESTURE_IDLE)
        {
            self.end_gesture();
            self.controllers.gesture = Gesture::default();
        }
    }

    /// The mappings a message reaches: the project's, else the global map's (the project
    /// comes first); dormant ones (their target gone) are passed by.
    fn mappings_hearing(
        &self,
        port: u8,
        channel: u8,
        control: cantus_core::controller::Control,
    ) -> Vec<(Scope, Mapping)> {
        let names = &self.midi_port_names;
        let hears = |m: &&Mapping| {
            m.source.control == control
                && m.source.channel.is_none_or(|c| c == channel)
                && m.source.port.matches(names, usize::from(port))
        };
        let p = self.project();
        let project: Vec<(Scope, Mapping)> = p
            .controllers
            .mappings
            .iter()
            .filter(hears)
            .filter(|m| m.target.exists(p))
            .map(|m| (Scope::Project, m.clone()))
            .collect();
        if !project.is_empty() {
            return project;
        }
        self.controllers
            .global
            .mappings
            .iter()
            .filter(hears)
            .map(|m| (Scope::Global, m.clone()))
            .collect()
    }

    fn drive_heard(&mut self, h: &Heard, pending: &mut Vec<Pending>) {
        let (channel, decoded, n) = decode(&mut self.controllers.wide, h.port, &h.event);
        for (control, value, on) in decoded.into_iter().take(n) {
            for (scope, m) in self.mappings_hearing(h.port, channel, control) {
                self.drive_one(scope, &m, value, on, h.at, pending);
            }
        }
    }

    fn drive_one(
        &mut self,
        scope: Scope,
        m: &Mapping,
        value: u16,
        on: bool,
        at: Instant,
        pending: &mut Vec<Pending>,
    ) {
        let Some(target) = self.resolve_target(&m.target) else {
            return;
        };
        let key = (scope, m.id);
        self.controllers.activity.insert(key, Instant::now());
        let input = m.input(value, on);
        let current = pending
            .iter()
            .find(|p| p.target == target)
            .map(|p| p.pos)
            .or_else(|| self.controller_pos(&target));
        let mut st = self.controllers.states.remove(&key).unwrap_or_default();
        let drive = m.drive(input, current, &mut st);
        self.controllers.states.insert(key, st);
        match drive {
            Drive::Fire => {
                if let MappingTarget::Action(id) = &target {
                    if let Some(a) = crate::keymap::Action::from_id(id) {
                        self.run_action(a);
                    }
                }
            }
            Drive::Set(pos) => {
                let Some(cmd) = self.target_command(&target, pos) else {
                    return;
                };
                self.record_controller(&target, &cmd, at);
                match pending.iter_mut().find(|p| p.target == target) {
                    Some(p) => {
                        p.pos = pos;
                        p.cmd = cmd;
                        if !p.keys.contains(&key) {
                            p.keys.push(key);
                        }
                    }
                    None => pending.push(Pending {
                        target,
                        pos,
                        cmd,
                        keys: vec![key],
                    }),
                }
            }
            Drive::Nothing => {}
        }
    }

    /// Where `target` is for a controller (0…1): a parameter's value as heard where
    /// automation drives it — unless a pass is recording it, when it follows the
    /// controller (ADR 0062 §3) — else the project's; a hosted parameter's through the
    /// plug-in's range; a switch, a mix, the tempo from the project.
    pub fn controller_pos(&self, target: &MappingTarget) -> Option<f32> {
        match target {
            MappingTarget::Param(a) if self.latched(a) => self.project_pos(target),
            MappingTarget::Param(a) => self.base_pos(a),
            _ => self.project_pos(target),
        }
    }

    /// The target's position as the project holds it (no automation).
    pub fn project_pos(&self, target: &MappingTarget) -> Option<f32> {
        match target {
            MappingTarget::Param(a) if a.is_plugin() => self.plugin_pos(a),
            _ => target.current_pos(self.project(), self.engine.position()),
        }
    }

    /// Whether a pass is recording `target`'s automation (a controller or the mouse moved
    /// it during the pass): it then follows what moves it, not its clip.
    pub fn latched(&self, target: &AutoTarget) -> bool {
        self.rec.pass.is_some()
            && self.project().recording.record_automation
            && self.modulators.rec.points.contains_key(target)
    }

    /// The command that puts `target` at `pos`.
    pub fn target_command(&self, target: &MappingTarget, pos: f32) -> Option<Command> {
        match target {
            MappingTarget::Param(a) if a.is_plugin() => {
                let (_, info) = self.plugin_param(a)?;
                let mut v = info.min + f64::from(pos) * (info.max - info.min);
                if info.stepped {
                    v = v.round();
                }
                a.command(self.project(), v)
            }
            _ => target.command(self.project(), pos, self.engine.position()),
        }
    }

    /// A mapped move while a pass records: written at the message's own time — the
    /// playhead now, less how long ago the message came, through the tempo map.
    fn record_controller(&mut self, target: &MappingTarget, cmd: &Command, at: Instant) {
        if !matches!(target, MappingTarget::Param(_)) || self.rec.pass.is_none() {
            return;
        }
        let now = self.engine.position().0 as f64;
        let age = Instant::now().saturating_duration_since(at).as_secs_f64();
        let tick = if self.engine.is_playing() {
            tick_of_message(&self.project().tempo_map(), now, age)
        } else {
            now.round() as i64
        };
        self.record_tweak_at(cmd, Some(tick));
    }

    /// This frame's moves as one command, merged into the gesture's undo step.
    fn apply_moves(&mut self, pending: Vec<Pending>) {
        if pending.is_empty() {
            return;
        }
        let g = &mut self.controllers.gesture;
        let grew = pending.iter().any(|p| !g.targets.contains(&p.target));
        if grew {
            if !g.targets.is_empty() {
                // A new target joins: the step so far is sealed, a wider one begins (a
                // batch of another shape would not merge).
                self.bus.end_gesture();
            }
            let g = &mut self.controllers.gesture;
            for p in &pending {
                if !g.targets.contains(&p.target) {
                    g.targets.push(p.target.clone());
                }
            }
        }
        let targets = self.controllers.gesture.targets.clone();
        let key = MergeKey::of(&("controllers", &targets));
        let command = if targets.len() == 1 {
            pending[0].cmd.clone()
        } else {
            // Every target of the gesture, the ones that did not move this frame at the
            // value they hold: the same shape every frame, so the newest replaces the last.
            let commands: Vec<Command> = targets
                .iter()
                .filter_map(|t| match pending.iter().find(|p| &p.target == t) {
                    Some(p) => Some(p.cmd.clone()),
                    None => self
                        .project_pos(t)
                        .and_then(|pos| self.target_command(t, pos)),
                })
                .collect();
            Command::Batch {
                label_key: GESTURE_LABEL.into(),
                commands,
            }
        };
        if let Some(a) = pending.iter().rev().find_map(|p| match &p.target {
            MappingTarget::Param(a) => Some(a.clone()),
            _ => None,
        }) {
            self.automation.last_tweaked = Some(a);
        }
        if let Err(e) = self.bus.execute_merging(command, Origin::User, key) {
            self.set_error(e.to_string());
        }
        // What the target holds now (a stepped one rounds): what each mapping gave it.
        for p in &pending {
            let now = self.controller_pos(&p.target);
            for k in &p.keys {
                if let Some(st) = self.controllers.states.get_mut(k) {
                    st.last_out = now.or(st.last_out);
                }
            }
        }
        self.controllers.gesture.last = Some(Instant::now());
    }

    /// Publish what the maps take (only when the project changed, the global map or the
    /// ports did): mapped messages are consumed before the engine. Nothing is cloned while
    /// nothing changed (a revision and two comparisons a frame).
    pub fn publish_controller_filter(&mut self) {
        let rev = self.bus.revision();
        let same = self
            .controllers
            .published
            .as_ref()
            .is_some_and(|(r, g, n)| {
                *r == rev && *g == self.controllers.global && *n == self.midi_port_names
            });
        if same {
            return;
        }
        let names = self.midi_port_names.clone();
        let p = self.project();
        let mut set = FilterSet::default();
        // Dormant mappings take nothing: their messages play as if unmapped.
        let live = p.controllers.mappings.iter().filter(|m| m.target.exists(p));
        for m in live.chain(&self.controllers.global.mappings) {
            for i in 0..names.len() {
                if m.source.port.matches(&names, i) {
                    set.take(i, m.source.channel, m.source.control);
                }
            }
        }
        self.controllers.tap.filter().publish(&set);
        self.controllers.published = Some((rev, self.controllers.global.clone(), names));
    }

    /// While a pass records automation, a parameter it has recorded follows what moves it,
    /// not its clip (ADR 0062 §3, Cubase's and Pro Tools' *latch*): its lane in the snapshot
    /// holds the project's value — the controller's newest — until the pass ends and the
    /// recorded clip takes over. Nothing changes outside a recording pass.
    pub fn latch_recorded(&self, snapshot: &mut std::sync::Arc<cantus_core::Snapshot>) {
        if self.rec.pass.is_none()
            || !self.project().recording.record_automation
            || self.modulators.rec.points.is_empty()
        {
            return;
        }
        let touched = |t: &AutoTarget| self.modulators.rec.points.contains_key(t);
        if !snapshot.automation.iter().any(|l| touched(&l.target)) {
            return;
        }
        let s = std::sync::Arc::make_mut(snapshot);
        for lane in s.automation.iter_mut().filter(|l| touched(&l.target)) {
            lane.segs.clear();
        }
    }
}
