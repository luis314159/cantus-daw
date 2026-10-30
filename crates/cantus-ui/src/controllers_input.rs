//! The controllers' frame (round 29, ADR 0062 §1, §3): what the tap heard becomes what it
//! asks — a learnt control while one is armed, otherwise each mapping's drive — and the
//! moves become commands: **one command per target per frame** (the newest value), merged
//! into **one undo step per gesture**. A gesture ends [`GESTURE_IDLE`] after its last
//! message — judged by the messages' own times, and woken for when the window is idle —
//! and two knobs moved together are one step however they join (the step widens). A target
//! a gesture moved and that did not move this frame is re-sent with the very command it
//! last had, never one rebuilt from a read-back — while the history is as the gesture left
//! it: an undo, a redo, a mouse edit or a load under it ends it, and only what moves next
//! is sent (review 2). A mapped move while a pass records is written into the pass's
//! automation **at the message's own time**. A target a frozen sound refuses is left out
//! of the gesture (the others keep moving) and said once.

use crate::app::CantusApp;
use crate::controllers::{decode, Key, Scope};
use cantus_core::controller::{Drive, Mapping, MappingTarget, GESTURE_LABEL};
use cantus_core::{AutoTarget, Command, MergeKey, Origin};
use cantus_engine::midi::{FilterSet, Heard};
use std::time::{Duration, Instant};

/// After this long without a message, a controller gesture is sealed into its undo step.
pub const GESTURE_IDLE: Duration = Duration::from_millis(500);

/// A gesture's target: the target, and for the tempo the point that governs it — a turn
/// that crosses into another point's stretch moves another point, so another step (as the
/// top bar's field and tap tempo key per point, ADR 0060 §2).
pub type GestureTarget = (MappingTarget, usize);

/// The gesture going on.
#[derive(Debug, Default)]
pub struct Gesture {
    /// The targets it moved, in the order they came, each with the command it last sent.
    pub moves: Vec<(GestureTarget, Command)>,
    /// The undo step's key: the gesture's own serial and its targets — never another
    /// gesture's, so a sealed or buried step is never merged into again (review 2, #20).
    pub key: Option<MergeKey>,
    /// Its last message's time.
    pub last: Option<Instant>,
    /// The project's revision right after its own last step: another means the history
    /// changed under it (review 2, #0).
    pub rev: Option<u64>,
    /// Which gesture this is (`ControllerUi::gestures` when it began).
    pub serial: u64,
    /// The frozen sounds that refused one of its targets, said once each.
    pub refused: Vec<cantus_core::freeze::FreezeTarget>,
}

/// Where a message `age` seconds old was heard, the playhead being at tick `now`: through
/// the tempo map, so a knob turned across a ramp lands where it was turned.
pub fn tick_of_message(map: &cantus_core::TempoMap, now: f64, age: f64) -> i64 {
    if age.is_nan() || age <= 0.0 {
        return now.round() as i64;
    }
    map.tick_at_seconds(map.seconds_at(now) - age).round() as i64
}

/// [`tick_of_message`] inside a loop: a message heard before a wrap that `wrapped` says
/// came since, read after it, is the end of the pass before (review 1, #23). With no wrap
/// since — the first entry into the loop, a pass recorded from its start — the tick is
/// where it was heard (review 2, #18).
pub fn tick_in_loop(tick: i64, now: i64, loop_range: Option<(i64, i64)>, wrapped: bool) -> i64 {
    match loop_range {
        Some((start, end)) if wrapped && end > start && now >= start && tick < start => {
            (end - (start - tick)).max(start)
        }
        _ => tick,
    }
}

/// One target's newest value this frame.
struct Pending {
    target: GestureTarget,
    pos: f32,
    cmd: Command,
    keys: Vec<Key>,
}

impl CantusApp {
    /// Every frame: publish the filter the maps ask for, read what the ports heard, learn
    /// or drive, seal an idle gesture, send feedback, and ask for the frames the clocks
    /// need (a gesture's end, a learn's window, feedback's quiet). Interface thread.
    pub fn controller_input(&mut self) {
        // The grids found first: a new grid port is taken in the same filter, before any
        // of its pads can reach the engine (round 33, ADR 0077).
        crate::controllers::grid::find(self);
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
        self.controller_wakes();
    }

    /// What the tap heard, in order (the tests drive this directly).
    pub fn controller_heard(&mut self, heard: &[Heard]) {
        self.follow_project();
        self.note_playhead();
        // The burst's earliest message (it comes port by port, not in time order, review
        // 4, #12).
        let first = heard
            .iter()
            .map(|h| h.at)
            .min()
            .unwrap_or_else(Instant::now);
        // A gesture idle since before these messages came is sealed first: by their time,
        // not the frame's (review 1, #3).
        self.seal_idle_gesture(first);
        let mut pending: Vec<Pending> = Vec::new();
        // A 14-bit half left waiting past PAIR_GAP before this burst is read alone first:
        // the burst's halves are new pairs.
        self.drive_wide_alone(first, &mut pending);
        for h in heard {
            // Feedback come back (a loopback port, an echoing device): neither a move nor
            // the control being learnt (review 2, #4).
            if self.is_feedback_echo(h) {
                continue;
            }
            // A session grid's port is the grid's (round 33, ADR 0077).
            if crate::controllers::grid::heard(self, h) {
                continue;
            }
            if self.controllers.learning.is_some() {
                self.learn_heard(h);
            } else {
                self.drive_heard(h, &mut pending);
            }
        }
        // A 14-bit pair's half alone a moment ([`crate::controllers::PAIR_GAP`]): its
        // partner is not coming — a coarse half is read with a fine 0 (MIDI's rule), a
        // fine-first controller's fine half with its coarse half as it stands (review 2,
        // #5; review 3: never one frame early, when the partner is a moment away).
        let now = heard
            .iter()
            .map(|h| h.at)
            .max()
            .unwrap_or_else(Instant::now);
        let now = now.max(Instant::now());
        self.drive_wide_alone(now, &mut pending);
        self.learn_tick();
        let last = heard.iter().map(|h| h.at).max();
        self.apply_moves(pending, last);
    }

    /// Another project in this one's place (opened, new, a demo, recovered): a learn armed
    /// for the last one lets go, and what the session kept by mapping id — takeover,
    /// feedback sent, activity, the gesture — starts again (review 3, #25).
    fn follow_project(&mut self) {
        if self.controllers.epoch == self.epoch {
            return;
        }
        self.controllers.epoch = self.epoch;
        // Let go silently: the open's own words (links or warps repaired, migrated) stay
        // in the status (review 4, #29).
        if self.controllers.learning.take().is_some() {
            self.controllers.tap.filter().set_learning(false);
        }
        self.controllers.states.clear();
        self.controllers.sent.clear();
        self.controllers.activity.clear();
        self.controllers.gesture = Gesture::default();
    }

    /// The 14-bit halves that have waited [`crate::controllers::PAIR_GAP`] by `now`, read
    /// alone and driven.
    fn drive_wide_alone(&mut self, now: Instant, pending: &mut Vec<Pending>) {
        let alone = crate::controllers::wide_alone(&mut self.controllers.wide, now);
        if self.controllers.learning.is_some() {
            return;
        }
        for (port, channel, n, value, at) in alone {
            let control = cantus_core::controller::Control::Cc14(n);
            self.drive_decoded(
                port,
                channel,
                &[control],
                &[(control, value, true)],
                at,
                pending,
            );
        }
    }

    /// The playhead's wraps: a loop that came round since a message was heard puts it at
    /// the end of the pass before.
    pub(crate) fn note_playhead(&mut self) {
        let pos = self.engine.position();
        let playing = self.engine.is_playing();
        let looped = self
            .project()
            .loop_region
            .as_ref()
            .filter(|_| self.project().loop_enabled)
            .map(|r| (r.start, r.end));
        // A wrap is the playhead back by more than half the loop, inside it, playing — the
        // recorder's own rule: a stale report from before a count-in's seek (stopped) or a
        // step back is none (review 4, #17, #38). Seen once a frame, before anything heard
        // in it is recorded: the pass starts at the loop's start from here on.
        if let (Some((a, b)), Some(prev), true) = (looped, self.controllers.prev_pos, playing) {
            if pos.0 < prev && prev - pos.0 > (b - a).0 / 2 && pos >= a && pos < b {
                self.controllers.last_wrap = Some(Instant::now());
                if let Some(live) = self.rec.pass.as_mut() {
                    if live.plan.loop_region.is_some() {
                        live.pass_start = a;
                    }
                }
            }
        }
        if playing {
            self.controllers.prev_pos = Some(pos.0);
        }
        // The latch on or off since the last snapshot (the pass begun, the punch-out passed,
        // a loop back inside the punch): the engine is sent the lanes again (review 4, #16,
        // #25, #31).
        let latching = self.recording_latches();
        if latching != self.controllers.latching {
            self.controllers.latching = latching;
            self.force_resync();
        }
    }

    /// Seal the gesture when `at` is [`GESTURE_IDLE`] past its last message — its own step
    /// only (a mouse drag on top is left alone).
    fn seal_idle_gesture(&mut self, at: Instant) {
        let g = &self.controllers.gesture;
        if g.last
            .is_some_and(|l| at.saturating_duration_since(l) >= GESTURE_IDLE)
        {
            if let Some(key) = g.key {
                self.bus.end_gesture_if(key);
            }
            self.controllers.gesture = Gesture::default();
        }
    }

    /// Ask for a frame when a controller clock runs out with the window otherwise idle: a
    /// gesture's end, a learn's window or its timeout, a feedback mapping's quiet.
    fn controller_wakes(&mut self) {
        let Some(ctx) = self.theme_ctx.clone() else {
            return;
        };
        let now = Instant::now();
        let mut due: Vec<Instant> = Vec::new();
        if let Some(l) = self.controllers.gesture.last {
            due.push(l + GESTURE_IDLE);
        }
        // A 14-bit half waiting for its partner.
        due.extend(
            self.controllers
                .wide
                .values()
                .filter_map(|w| w.pending.or(w.pending_lsb))
                .map(|t| t + crate::controllers::PAIR_GAP),
        );
        if let Some(l) = &self.controllers.learning {
            match l.first {
                Some(f) => due.push(
                    f + if crate::controllers_learn::held_button(&l.heard) {
                        crate::controllers_learn::HOLD_WINDOW
                    } else {
                        crate::controllers_learn::LEARN_WINDOW
                    },
                ),
                None => due.push(l.armed_at + crate::controllers_learn::LEARN_TIMEOUT),
            }
        }
        due.extend(self.feedback_due());
        if let Some(t) = due.into_iter().min() {
            let wait = t
                .saturating_duration_since(now)
                .max(Duration::from_millis(1));
            crate::views::workspace_ticker::wake_after(&ctx, wait);
        }
    }

    /// Whether `m` can drive its target now: alive in the project (its target there, its
    /// plug-in the one it was learnt on) and, for a hosted parameter, the plug-in loaded and
    /// the parameter among its own (review 1, #55). A mapping that cannot is dormant.
    pub fn mapping_alive(&self, scope: Scope, m: &Mapping) -> bool {
        if scope == Scope::Project && !m.alive(self.project()) {
            return false;
        }
        // An action this build does not know takes nothing (its control plays as unmapped
        // and shadows no other mapping), in either map (review 4, #14).
        if let MappingTarget::Action(id) = &m.target {
            if crate::keymap::Action::from_id(id).is_none() {
                return false;
            }
        }
        match &m.target {
            // Unloaded because its sound is frozen: alive — its control is still the map's
            // (it must not reach another instrument), and a move is refused and said
            // (review 2, #54).
            MappingTarget::Param(a) if a.is_plugin() => {
                self.hosted_param_info(a).is_some() || self.frozen_refusal(a).is_some()
            }
            _ => true,
        }
    }

    /// What `target`'s values are, for a controller (review 3, #0, #1): a switch (two
    /// states: a mute, an LED, a band's On, a hosted on/off), the steps a stepped one moves
    /// through, else a range.
    pub fn target_shape(&self, target: &MappingTarget) -> cantus_core::controller::Steps {
        use cantus_core::controller::Steps;
        match target {
            // A hosted parameter by the plug-in's own step count, never by its plain values
            // (a JUCE VST3's four choices are 0…1: no switch; review 4, #1, #58).
            MappingTarget::Param(a) if a.is_plugin() => self
                .hosted_param_info(a)
                .and_then(|i| i.step_count())
                .map_or(Steps::default(), |n| Steps {
                    switch: n == 1,
                    steps: Some(n),
                }),
            MappingTarget::Param(a) => cantus_core::automation::range::range_of(self.project(), a)
                .map_or(Steps::default(), |r| {
                    Steps::of_range(r.stepped, f64::from(r.min), f64::from(r.max))
                }),
            t if t.is_switch() => Steps {
                switch: true,
                steps: Some(1),
            },
            // The global quantisation's fourteen grids (round 33, ADR 0077).
            MappingTarget::SessionQuantize => Steps {
                switch: false,
                steps: Some(cantus_core::controller::SESSION_QUANTIZE.len() as u32 - 1),
            },
            _ => Steps::default(),
        }
    }

    /// A hosted parameter's description when its plug-in is loaded. Allocates nothing:
    /// asked every frame for the filter's signature (review 2, #30).
    pub fn hosted_param_info(&self, a: &AutoTarget) -> Option<&cantus_host::ParamInfo> {
        match a {
            AutoTarget::Plugin { effect, id } => self
                .plugins
                .hosted(*effect)?
                .params
                .iter()
                .find(|p| p.id == *id),
            AutoTarget::PluginInstrument { track, id } => self
                .plugins
                .hosted_instrument(*track)?
                .params
                .iter()
                .find(|p| p.id == *id),
            _ => None,
        }
    }

    /// A hosted parameter's own words for `value` (the loaded plug-in's), when it has them.
    pub fn hosted_param_text(&self, a: &AutoTarget, id: u32, value: f64) -> Option<String> {
        match a {
            AutoTarget::Plugin { effect, .. } => {
                self.plugins.hosted(*effect)?.param_text(id, value)
            }
            AutoTarget::PluginInstrument { track, .. } => self
                .plugins
                .hosted_instrument(*track)?
                .param_text(id, value),
            _ => None,
        }
    }

    /// The frozen sound that refuses a change of `a` (a hosted parameter whose plug-in rests
    /// while its track or strip is frozen).
    fn frozen_refusal(&self, a: &AutoTarget) -> Option<cantus_core::freeze::FreezeTarget> {
        let p = self.project();
        if !p.any_frozen() {
            return None;
        }
        cantus_core::freeze::refuses(p, &a.command(p, 0.0)?)
    }

    /// Why a mapping does nothing now, for the pane: `None` when it is live.
    pub fn mapping_state(&self, scope: Scope, m: &Mapping) -> Option<crate::controllers::Idle> {
        use crate::controllers::Idle;
        let p = self.project();
        // An action this build does not know (renamed, removed, a later Cantus's).
        if let MappingTarget::Action(id) = &m.target {
            if crate::keymap::Action::from_id(id).is_none() {
                return Some(Idle::Missing);
            }
        }
        // A record arm whose strip has nothing to arm (review 4, #6): dim, its control free.
        if let MappingTarget::StripArm(s) = &m.target {
            if p.strip(*s).is_some() && !p.tracks_on_strip(*s).any(|t| t.input.is_some()) {
                return Some(Idle::NoInput);
            }
        }
        // A hosted mapping whose place holds another device now — another plug-in, or one
        // of Cantus's own — is the swapped-out plug-in's, not a deleted target (review 4,
        // #49, #61).
        if scope == Scope::Project && m.plugin.is_some() && !m.fits(p) && place_exists(p, &m.target)
        {
            return Some(Idle::OtherPlugin);
        }
        if !m.target.exists(p) {
            return Some(Idle::Missing);
        }
        if scope == Scope::Project && !m.fits(p) {
            return Some(Idle::OtherPlugin);
        }
        // Its port is not open here (unplugged, another machine): dim, review 3, #45.
        if !m.source.port.is_any() && m.source.port.index_in(&self.midi_port_names).is_none() {
            return Some(Idle::NotConnected);
        }
        if let MappingTarget::Param(a) = &m.target {
            if a.is_plugin() {
                if self.hosted_param_info(a).is_none() {
                    return Some(if self.frozen_refusal(a).is_some() {
                        Idle::Frozen
                    } else if self.plugins_loaded_at(a) {
                        Idle::Missing
                    } else {
                        Idle::NotLoaded
                    });
                }
                // Loaded, but crashed: it plays nothing until it is reloaded (review 3, #57).
                if self.hosted_crashed(a) {
                    return Some(Idle::NotLoaded);
                }
            }
        }
        // Any target a frozen sound refuses (review 3, #8).
        if p.any_frozen() {
            let target = self.resolve_target(&m.target)?;
            let pos = self.controller_pos(&target).unwrap_or(0.0);
            if let Some(cmd) = self.target_command(&target, pos) {
                if cantus_core::freeze::refuses(p, &cmd).is_some() {
                    return Some(Idle::Frozen);
                }
            }
        }
        None
    }

    /// Whether the hosted plug-in at `a`'s place crashed (it is kept, silent, until it is
    /// reloaded).
    fn hosted_crashed(&self, a: &AutoTarget) -> bool {
        match a {
            AutoTarget::Plugin { effect, .. } => self
                .plugins
                .hosted(*effect)
                .is_some_and(|h| h.crashed().is_some()),
            AutoTarget::PluginInstrument { track, .. } => self
                .plugins
                .hosted_instrument(*track)
                .is_some_and(|h| h.crashed().is_some()),
            _ => false,
        }
    }

    /// Whether the plug-in at `a`'s place is loaded (its parameter then is what is gone).
    fn plugins_loaded_at(&self, a: &AutoTarget) -> bool {
        match a {
            AutoTarget::Plugin { effect, .. } => self.plugins.hosted(*effect).is_some(),
            AutoTarget::PluginInstrument { track, .. } => {
                self.plugins.hosted_instrument(*track).is_some()
            }
            _ => false,
        }
    }

    /// The mappings a message reaches: the project's that take any of its addresses (a
    /// 7-bit controller is also its 14-bit pair's half), else the global map's — the project
    /// comes first for the whole message (review 1, #7); dormant ones are passed by.
    fn mappings_hearing(
        &self,
        port: u8,
        channel: u8,
        controls: &[cantus_core::controller::Control],
    ) -> (Scope, Vec<Mapping>) {
        let names = &self.midi_port_names;
        let hears = |m: &Mapping| {
            controls.contains(&m.source.control)
                && m.source.channel.is_none_or(|c| c == channel)
                && m.source.port.matches(names, usize::from(port))
        };
        let project: Vec<Mapping> = self
            .project()
            .controllers
            .mappings
            .iter()
            .filter(|m| hears(m) && self.mapping_alive(Scope::Project, m))
            .cloned()
            .collect();
        if !project.is_empty() {
            return (Scope::Project, project);
        }
        let global = self
            .controllers
            .global
            .mappings
            .iter()
            .filter(|m| hears(m) && self.mapping_alive(Scope::Global, m))
            .cloned()
            .collect();
        (Scope::Global, global)
    }

    fn drive_heard(&mut self, h: &Heard, pending: &mut Vec<Pending>) {
        let (channel, decoded, n) = decode(&mut self.controllers.wide, h.port, &h.event, h.at);
        // Every address the message is: a controller below 64 is also its 14-bit pair's,
        // even when the pair's value waits for its fine half.
        let mut controls: Vec<cantus_core::controller::Control> =
            decoded.iter().take(n).map(|d| d.0).collect();
        if let (0xB0, n7) = (h.event.status & 0xF0, h.event.data1 & 0x7F) {
            if n7 < 64 {
                let pair = cantus_core::controller::Control::Cc14(n7 % 32);
                if !controls.contains(&pair) {
                    controls.push(pair);
                }
            }
        }
        self.drive_decoded(h.port, channel, &controls, &decoded[..n], h.at, pending);
    }

    /// A message's readings, `controls` every address it is, driving the mappings that
    /// hear it.
    fn drive_decoded(
        &mut self,
        port: u8,
        channel: u8,
        controls: &[cantus_core::controller::Control],
        decoded: &[(cantus_core::controller::Control, u16, bool)],
        at: Instant,
        pending: &mut Vec<Pending>,
    ) {
        let (scope, maps) = self.mappings_hearing(port, channel, controls);
        for &(control, value, on) in decoded {
            for m in maps.iter().filter(|m| m.source.control == control) {
                self.drive_one(scope, m, value, on, at, pending);
            }
        }
    }

    /// Say once per gesture that a frozen sound refused one of its targets. The refusal
    /// counts as the gesture's move: the gesture ends after [`GESTURE_IDLE`] without one,
    /// and the next turn is said again (review 3, #20).
    fn refused_by_freeze(&mut self, t: cantus_core::freeze::FreezeTarget) {
        let now = Instant::now();
        let g = &mut self.controllers.gesture;
        g.last = Some(g.last.map_or(now, |l| l.max(now)));
        if !g.refused.contains(&t) {
            g.refused.push(t);
            crate::freeze::refused(self, t);
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
        // A session launcher (round 33, ADR 0077): its press launches, its release
        // releases — no value, no command.
        if target.is_launcher() {
            let key = (scope, m.id);
            self.controllers.activity.insert(key, Instant::now());
            let pressed = match m.input(value, on) {
                cantus_core::controller::Input::Press(p) => p,
                cantus_core::controller::Input::Abs(x) => x > 0.0,
                cantus_core::controller::Input::Steps(n) => n > 0,
            };
            let mut st = self.controllers.states.remove(&key).unwrap_or_default();
            let velocity = if m.source.control.is_button() && on {
                (f32::from(value) / f32::from(m.source.control.max())).clamp(0.0, 1.0)
            } else {
                1.0
            };
            if pressed {
                crate::views::session::learn::drive(self, &target, true, velocity);
            } else if st.held {
                crate::views::session::learn::drive(self, &target, false, 0.0);
            }
            st.held = pressed;
            self.controllers.states.insert(key, st);
            return;
        }
        let id = self.gesture_target(&target);
        let key = (scope, m.id);
        self.controllers.activity.insert(key, Instant::now());
        let current = pending
            .iter()
            .find(|p| p.target == id)
            .map(|p| p.pos)
            .or_else(|| self.controller_pos(&target));
        let mut st = self.controllers.states.remove(&key).unwrap_or_default();
        let shape = self.target_shape(&target);
        let drive = m.read_in(value, on, current, &mut st, shape);
        self.controllers.states.insert(key, st);
        match drive {
            Drive::Fire => {
                if let MappingTarget::Action(id) = &target {
                    if let Some(a) = crate::keymap::Action::from_id(id) {
                        // A controller's press of a session action needs no focus on the
                        // pane, as a key's does (round 33, ADR 0077).
                        self.session_view.controller_fire = true;
                        self.run_action(a);
                        self.session_view.controller_fire = false;
                    }
                }
            }
            Drive::Set(pos) => {
                let Some(cmd) = self.target_command(&target, pos) else {
                    // A hosted parameter resting while its sound is frozen.
                    if let MappingTarget::Param(a) = &target {
                        if let Some(t) = self.frozen_refusal(a) {
                            self.refused_by_freeze(t);
                        }
                    }
                    return;
                };
                // A frozen sound refuses it: left out, the gesture's other targets move on
                // (review 2, #25).
                if let Some(t) = cantus_core::freeze::refuses(self.project(), &cmd) {
                    self.refused_by_freeze(t);
                    return;
                }
                self.record_controller(&target, pos, at);
                // A strip's record arm is its tracks' arms, each its own in the gesture: a
                // track's arm pressed after it (or before) is the same target, the newest
                // press wins (review 4, #18).
                let parts: Vec<(GestureTarget, Command)> = match (&target, cmd) {
                    (MappingTarget::StripArm(_), Command::Batch { commands, .. }) => commands
                        .into_iter()
                        .filter_map(|c| match &c {
                            Command::SetTrackArmed { id, .. } => {
                                Some(((MappingTarget::TrackArm(*id), 0), c))
                            }
                            _ => None,
                        })
                        .collect(),
                    (_, cmd) => vec![(id, cmd)],
                };
                for (id, cmd) in parts {
                    match pending.iter_mut().find(|p| p.target == id) {
                        Some(p) => {
                            p.pos = pos;
                            p.cmd = cmd;
                            if !p.keys.contains(&key) {
                                p.keys.push(key);
                            }
                        }
                        None => pending.push(Pending {
                            target: id,
                            pos,
                            cmd,
                            keys: vec![key],
                        }),
                    }
                }
            }
            Drive::Nothing => {}
        }
    }

    /// A target's identity in a gesture: the tempo with the point that governs it now.
    fn gesture_target(&self, target: &MappingTarget) -> GestureTarget {
        let sub = match target {
            MappingTarget::Tempo => {
                cantus_core::tempo::edit::point_at(self.project(), self.engine.position())
                    .unwrap_or(0)
            }
            _ => 0,
        };
        (target.clone(), sub)
    }

    /// Where `target` is for a controller (0…1): a parameter's value as heard where
    /// automation drives it — unless a pass is recording it, when it follows the
    /// controller (ADR 0062 §3) — else the project's; a hosted parameter's through the
    /// plug-in's range; a switch, a mix, the tempo from the project; an action with a state
    /// (playing, recording, loop, metronome, PAT) that state, for its button's light.
    pub fn controller_pos(&self, target: &MappingTarget) -> Option<f32> {
        match target {
            MappingTarget::Param(a) if self.latched(a) => self.project_pos(target),
            MappingTarget::Param(a) => self.base_pos(a),
            MappingTarget::Action(id) => self.action_state(id),
            t if t.is_launcher() => crate::views::session::learn::launcher_pos(self, t),
            _ => self.project_pos(target),
        }
    }

    /// The state a transport action shows (1 on, 0 off), for feedback: `None` for an
    /// action that is not a state (stop, tap tempo, a marker).
    pub fn action_state(&self, id: &str) -> Option<f32> {
        use crate::keymap::Action;
        let on = |b: bool| if b { 1.0 } else { 0.0 };
        let p = self.project();
        let selected_track = self.selected_track.and_then(|t| p.track(t));
        let selected_strip = self.selected_strip.and_then(|s| p.strip(s));
        Some(on(match Action::from_id(id)? {
            Action::PlayPause => self.engine.is_playing(),
            Action::Record => self.is_recording(),
            Action::ToggleLoop => p.loop_enabled,
            Action::ToggleMetronome => p.metronome,
            Action::TogglePatternMode => p.pattern_mode,
            // Every toggle the Add menu offers that has a state (review 2, #50).
            Action::TogglePunch => p.recording.punch_enabled,
            Action::ToggleCountIn => p.recording.count_in_bars > 0,
            Action::ToggleLowLatency => self.settings.low_latency,
            Action::ToggleTempoTrack => self.tempo_lane_shown(),
            Action::ToggleTypingPiano => self.keyboard_piano,
            Action::ToggleMeterWave => self.settings.mixer_meter_wave,
            Action::ArmTrack => selected_track.is_some_and(|t| t.armed),
            Action::ToggleStripMute => selected_strip.is_some_and(|s| s.muted),
            Action::ToggleStripSolo => selected_strip.is_some_and(|s| s.soloed),
            // The learn mode (the top bar's MIDI lamp) and every area the View menu checks
            // (review 3, #48).
            Action::MidiLearnMode => self.controllers.learn_mode,
            // The session's (round 33, ADR 0077).
            Action::SessionArrangementRecord => self.session_view.live.arrangement_record,
            Action::SessionBackToArrangement => self.session_view.live.holds_any(),
            Action::ToggleSessionView => self
                .workspace
                .contains(crate::views::workspace::Pane::Session),
            Action::ToggleAutomationArm => self.auto_armed(),
            a => match crate::views::transport::menus::pane_of(a) {
                Some(pane) => self.workspace.contains(pane),
                None => match a {
                    Action::ToggleInstrumentWindow => self
                        .selected_track
                        .is_some_and(|t| self.instrument_window_open(t)),
                    Action::ShowTakes => selected_track.is_some_and(|t| t.takes_open),
                    // The open instrument editor's keyboard card (review 4, #26).
                    Action::ToggleDeviceKeyboard => {
                        self.inst.open.is_some() && crate::views::inst::keyboard_shown(self)
                    }
                    _ => return None,
                },
            },
        }))
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
        self.writing_target(target)
    }

    /// Whether a recording pass latches what it records now: not before the pass (a
    /// count-in, a pre-roll, before the punch-in), nor past the punch-out, where nothing is
    /// written (review 3, #16; review 4, #16).
    fn recording_latches(&self) -> bool {
        let Some(live) = self.rec.pass.as_ref() else {
            return false;
        };
        let pos = self.engine.position();
        let (from, to) = match live.plan.punch {
            Some((a, b)) => (a, Some(b)),
            None => (live.pass_start, None),
        };
        self.project().recording.record_automation
            && pos >= from
            && !to.is_some_and(|out| pos > out)
    }

    /// The command that puts `target` at `pos`.
    pub fn target_command(&self, target: &MappingTarget, pos: f32) -> Option<Command> {
        match target {
            MappingTarget::Param(a) if a.is_plugin() => {
                let info = self.hosted_param_info(a)?;
                // On the plug-in's own steps, not whole plain units (review 4, #1, #58).
                let v = info.snap(info.min + f64::from(pos) * (info.max - info.min));
                a.command(self.project(), v)
            }
            _ => target.command(self.project(), pos, self.engine.position()),
        }
    }

    /// A mapped move while a pass records: the target at `pos` written at the message's
    /// own time — the playhead now, less how long ago the message came, through the tempo
    /// map, inside the loop — whatever its command (a modulator's amount too, review 1,
    /// #5).
    fn record_controller(&mut self, target: &MappingTarget, pos: f32, at: Instant) {
        let MappingTarget::Param(a) = target else {
            return;
        };
        // Any pass that writes (ADR 0068): a recording one, or an armed Write, Touch or
        // Latch — the track's mode decides in `write_value`.
        if !self.automation.write.writing() {
            return;
        }
        let now = self.engine.position().0;
        let age = Instant::now().saturating_duration_since(at).as_secs_f64();
        let tick = if self.engine.is_playing() {
            let p = self.project();
            let t = tick_of_message(&p.tempo_map(), now as f64, age);
            let looping = p
                .loop_region
                .as_ref()
                .filter(|_| p.loop_enabled)
                .map(|r| (r.start.0, r.end.0));
            let wrapped = self.controllers.last_wrap.is_some_and(|w| w >= at);
            tick_in_loop(t, now, looping, wrapped)
        } else {
            now
        };
        // The position the target holds (a stepped one rounds), as the mouse's is.
        let held = match cantus_core::automation::range::range_of(self.project(), a) {
            Some(r) if !a.is_plugin() => r.pos(r.value(pos)),
            _ => pos,
        };
        self.record_point_at(a.clone(), held, tick);
    }

    /// This frame's moves as one command, merged into the gesture's undo step: the step
    /// widens when a target joins (one step per gesture, review 1, #22); the targets that
    /// did not move are sent with their last command (review 1, #18).
    fn apply_moves(&mut self, pending: Vec<Pending>, last: Option<Instant>) {
        // A knob on the tempo waits while audio records (review 4); the others move. The
        // refused knob is sent the tempo again (review 5: a motor fader stayed where the
        // hand left it).
        let mut pending = pending;
        if self.audio_take_running() {
            let (held, moved): (Vec<Pending>, Vec<Pending>) = pending
                .into_iter()
                .partition(|p| cantus_core::tempo::keep::changes_tempo(&p.cmd));
            pending = moved;
            if !held.is_empty() {
                for p in &held {
                    for k in &p.keys {
                        self.controllers.sent.remove(k);
                    }
                }
                self.set_status(crate::i18n::t("tempo.status.recording"));
            }
            // A gesture that turned the tempo before the take began is sealed: its tempo
            // command is never sent again during the take (round 29, review 3 of ADR 0062).
            let g = &self.controllers.gesture;
            if g.moves
                .iter()
                .any(|(_, c)| cantus_core::tempo::keep::changes_tempo(c))
            {
                if let Some(key) = g.key {
                    self.bus.end_gesture_if(key);
                }
                let refused = std::mem::take(&mut self.controllers.gesture.refused);
                self.controllers.gesture = Gesture {
                    refused,
                    ..Gesture::default()
                };
            }
        }
        if pending.is_empty() {
            return;
        }
        // The history changed under the gesture (an undo, a redo, a mouse edit, a load):
        // its step is sealed, and only what moves now is sent — never a target's old
        // command over what was done since (review 2, #0).
        let g = &self.controllers.gesture;
        if let (Some(rev), Some(key)) = (g.rev, g.key) {
            if rev != self.bus.revision() {
                self.bus.end_gesture_if(key);
                let refused = std::mem::take(&mut self.controllers.gesture.refused);
                self.controllers.gesture = Gesture {
                    refused,
                    ..Gesture::default()
                };
            }
        }
        if self.controllers.gesture.moves.is_empty() {
            self.controllers.gestures += 1;
            self.controllers.gesture.serial = self.controllers.gestures;
        }
        let old_key = self.controllers.gesture.key;
        let g = &mut self.controllers.gesture;
        let mut grew = false;
        for p in &pending {
            match g.moves.iter_mut().find(|(t, _)| *t == p.target) {
                Some(m) => m.1 = p.cmd.clone(),
                None => {
                    g.moves.push((p.target.clone(), p.cmd.clone()));
                    grew = true;
                }
            }
        }
        let ids: Vec<&GestureTarget> = g.moves.iter().map(|(t, _)| t).collect();
        let key = MergeKey::of(&("controllers", g.serial, ids));
        let command = if g.moves.len() == 1 {
            g.moves[0].1.clone()
        } else {
            Command::Batch {
                label_key: GESTURE_LABEL.into(),
                commands: g.moves.iter().map(|(_, c)| c.clone()).collect(),
            }
        };
        g.key = Some(key);
        let t = last.unwrap_or_else(Instant::now);
        g.last = Some(g.last.map_or(t, |l| l.max(t)));
        if let Some(a) = pending.iter().rev().find_map(|p| match &p.target.0 {
            MappingTarget::Param(a) => Some(a.clone()),
            _ => None,
        }) {
            self.automation.last_tweaked = Some(a);
        }
        let result = match old_key {
            Some(from) if grew => self.bus.widen_merging(command, Origin::User, from, key),
            _ => self.bus.execute_merging(command, Origin::User, key),
        };
        match result {
            Ok(()) => self.controllers.gesture.rev = Some(self.bus.revision()),
            Err(e) => {
                // A target gone under the gesture: its step sealed, it starts again with
                // the next message.
                self.bus.end_gesture_if(key);
                let refused = std::mem::take(&mut self.controllers.gesture.refused);
                self.controllers.gesture = Gesture {
                    refused,
                    ..Gesture::default()
                };
                match e {
                    cantus_core::CommandError::Frozen(t) => self.refused_by_freeze(t),
                    e => self.set_error(e.to_string()),
                }
            }
        }
        // What the target holds now (a stepped one rounds): what each mapping gave it.
        for p in &pending {
            let now = self.controller_pos(&p.target.0);
            for k in &p.keys {
                if let Some(st) = self.controllers.states.get_mut(k) {
                    st.last_out = now.or(st.last_out);
                }
            }
        }
    }

    /// What decides which messages the maps take beyond the maps themselves: which hosted
    /// parameters are loaded (a plug-in loaded, crashed away, its parameters rescanned).
    fn plugin_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let maps = self
            .project()
            .controllers
            .mappings
            .iter()
            .chain(&self.controllers.global.mappings);
        for m in maps {
            if let MappingTarget::Param(a) = &m.target {
                if a.is_plugin() {
                    (m.id, self.hosted_param_info(a).is_some()).hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// Publish what the maps take (only when the project, the global map, the ports or the
    /// loaded hosted parameters changed): mapped messages are consumed before the engine.
    /// Nothing is cloned while nothing changed.
    pub fn publish_controller_filter(&mut self) {
        let rev = self.bus.revision();
        let sig = self.plugin_signature();
        let same = self
            .controllers
            .published
            .as_ref()
            .is_some_and(|(r, s, g, n)| {
                *r == rev
                    && *s == sig
                    && *g == self.controllers.global
                    && *n == self.midi_port_names
            });
        if same {
            return;
        }
        let names = self.midi_port_names.clone();
        let mut set = FilterSet::default();
        // Dormant mappings take nothing: their messages play as if unmapped.
        let maps = self
            .project()
            .controllers
            .mappings
            .iter()
            .map(|m| (Scope::Project, m))
            .chain(
                self.controllers
                    .global
                    .mappings
                    .iter()
                    .map(|m| (Scope::Global, m)),
            );
        for (scope, m) in maps {
            if !self.mapping_alive(scope, m) {
                continue;
            }
            for i in 0..names.len() {
                if m.source.port.matches(&names, i) {
                    set.take(i, m.source.channel, m.source.control);
                }
            }
        }
        // A session grid's port is taken whole: its pads launch, they never play notes
        // (round 33, ADR 0077).
        for i in crate::controllers::grid::taken_ports(self) {
            for n in 0..128u8 {
                set.take(i, None, cantus_core::controller::Control::Note(n));
                set.take(i, None, cantus_core::controller::Control::Cc(n));
                set.take(i, None, cantus_core::controller::Control::PolyPressure(n));
            }
            set.take(i, None, cantus_core::controller::Control::ChannelPressure);
        }
        self.controllers.tap.filter().publish(&set);
        self.controllers.published = Some((rev, sig, self.controllers.global.clone(), names));
    }

    /// While a pass writes automation, a parameter it writes follows what moves it, not its
    /// clip (ADR 0062 §3; ADR 0068: held, the hand; latched, its value; let go in Touch, the
    /// glide back) until the pass ends and the written clip takes over. Nothing changes
    /// outside a pass.
    pub fn latch_recorded(&self, snapshot: &mut std::sync::Arc<cantus_core::Snapshot>) {
        self.live_written_lanes(snapshot);
    }
}

/// Whether the place a hosted target names is still there: the insert's slot, or the
/// track — whatever device it holds now.
fn place_exists(p: &cantus_core::Project, target: &MappingTarget) -> bool {
    match target {
        MappingTarget::Param(AutoTarget::Plugin { effect, .. }) => p.effect_host(*effect).is_some(),
        MappingTarget::Param(AutoTarget::PluginInstrument { track, .. }) => {
            p.track(*track).is_some()
        }
        _ => false,
    }
}
