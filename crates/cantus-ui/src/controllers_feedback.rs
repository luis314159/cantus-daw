//! Feedback to the controller (round 29, ADR 0062 §3): a mapping with feedback on sends its
//! target's value back on its control — the output port of the same device, found by name —
//! whenever the value changes by anything (the mouse, undo, automation, another mapping),
//! so a motor fader follows and an LED ring or a pad's light shows where the parameter is,
//! and a transport button's light shows whether it plays, records, loops.
//!
//! Never while that mapping's own control is moving (a motor fader would fight the hand);
//! never for a global mapping a live project mapping on the same control shadows; each
//! channel message sent on its own (MIDI APIs take one at a time, review 1, #11); and what
//! comes back on the input (a loopback port, an echoing device) within a moment of being
//! sent is not taken for a move (review 1, #15) — the moment judged by when the message
//! arrived, not when the interface read it, and too short for a hand: a real press of the
//! same bytes a little later is a press (review 2, #3).

use crate::app::CantusApp;
use crate::controllers::Scope;
use cantus_core::controller::{Control, Mapping};
use cantus_engine::midi::Heard;
use std::time::{Duration, Instant};

/// A mapping whose control moved this recently sends nothing back.
pub const QUIET_AFTER_MOVE: Duration = Duration::from_millis(250);
/// What arrives on the input this soon after being sent is the feedback's echo: a
/// loopback or a device answers within a millisecond or two; no hand presses again so
/// soon.
pub const ECHO_WINDOW: Duration = Duration::from_millis(30);
/// The two threads' clocks read a moment apart: an echo stamped this much before its
/// send is still its echo.
pub const ECHO_SKEW: Duration = Duration::from_millis(2);
/// How long a message sent is remembered, at most, waiting for an interface that reads its
/// input late (a minimised window, a long frame) — and how many are remembered.
pub const ECHO_KEEP: Duration = Duration::from_secs(2);
pub const ECHO_MAX: usize = 256;

/// The channel messages that put `control` on `channel` at `value` (its own range: 7 or 14
/// bits), each its own message of at most three bytes; none for a program change.
pub fn feedback_messages(control: Control, channel: u8, value: u16) -> Vec<Vec<u8>> {
    let ch = channel & 0x0F;
    let v7 = value.min(127) as u8;
    match control {
        Control::Cc(n) => vec![vec![0xB0 | ch, n & 0x7F, v7]],
        Control::Cc14(n) => {
            let v = value.min(16_383);
            vec![
                vec![0xB0 | ch, n & 31, (v >> 7) as u8],
                vec![0xB0 | ch, (n & 31) + 32, (v & 0x7F) as u8],
            ]
        }
        Control::Note(n) => vec![vec![0x90 | ch, n & 0x7F, v7]],
        // A motor fader on the pitch wheel (Mackie's way).
        Control::PitchBend => {
            let v = value.min(16_383);
            vec![vec![0xE0 | ch, (v & 0x7F) as u8, (v >> 7) as u8]]
        }
        Control::ChannelPressure => vec![vec![0xD0 | ch, v7]],
        Control::PolyPressure(n) => vec![vec![0xA0 | ch, n & 0x7F, v7]],
        // A program change has no value to show.
        Control::Program(_) => Vec::new(),
    }
}

/// Whether feedback can say anything for `m`: not from a program change, not for an action
/// that is no state (stop, tap tempo, a marker) — the pane shows no *Send* there.
pub fn can_feed_back(app: &CantusApp, m: &Mapping) -> bool {
    if matches!(m.source.control, Control::Program(_)) {
        return false;
    }
    match &m.target {
        cantus_core::controller::MappingTarget::Action(id) => app.action_state(id).is_some(),
        _ => true,
    }
}

impl CantusApp {
    /// The feedback mappings, each with its scope: the global ones a live project mapping
    /// on the same control shadows left out (review 1, #7), dormant ones too.
    fn feedback_mappings(&self) -> Vec<(Scope, Mapping)> {
        let p = self.project();
        // None sends feedback (the usual case): nothing gathered, nothing allocated (review
        // 2, #30: an idle frame allocates nothing).
        if !p
            .controllers
            .mappings
            .iter()
            .chain(&self.controllers.global.mappings)
            .any(|m| m.feedback)
        {
            return Vec::new();
        }
        let project: Vec<&Mapping> = p
            .controllers
            .mappings
            .iter()
            .filter(|m| self.mapping_alive(Scope::Project, m))
            .collect();
        let mut out: Vec<(Scope, Mapping)> = project
            .iter()
            .filter(|m| m.feedback)
            .map(|m| (Scope::Project, (*m).clone()))
            .collect();
        out.extend(
            self.controllers
                .global
                .mappings
                .iter()
                .filter(|m| m.feedback)
                .filter(|g| !project.iter().any(|m| m.source.overlaps(&g.source)))
                .map(|m| (Scope::Global, m.clone())),
        );
        out
    }

    /// When a feedback mapping's quiet ends (for the frame that sends its final value).
    pub(crate) fn feedback_due(&self) -> Vec<Instant> {
        let now = Instant::now();
        [Scope::Project, Scope::Global]
            .into_iter()
            .flat_map(|s| {
                self.controller_map(s)
                    .mappings
                    .iter()
                    .filter(|m| m.feedback)
                    .filter_map(move |m| self.controllers.activity.get(&(s, m.id)).copied())
            })
            .map(|t| t + QUIET_AFTER_MOVE)
            .filter(|t| *t > now)
            .collect()
    }

    /// Whether `h` is a message feedback sent a moment before it arrived, come back (then
    /// forgotten).
    pub(crate) fn is_feedback_echo(&mut self, h: &Heard) -> bool {
        let echoes = &mut self.controllers.echoes;
        if echoes.is_empty() {
            return false;
        }
        let bytes = [h.event.status, h.event.data1, h.event.data2];
        let short = matches!(h.event.status & 0xF0, 0xC0 | 0xD0);
        match echoes.iter().position(|(port, sent, t)| {
            *port == h.port
                && sent[..2] == bytes[..2]
                && (short || sent[2] == bytes[2])
                && h.at + ECHO_SKEW >= *t
                && h.at.saturating_duration_since(*t) <= ECHO_WINDOW
        }) {
            Some(i) => {
                echoes.remove(i);
                true
            }
            None => false,
        }
    }

    /// Send every feedback mapping's value that changed.
    pub(crate) fn controller_feedback(&mut self) {
        // What was sent long ago can no longer come back: the list stays short whatever
        // the input does (review 2, #11).
        let now = Instant::now();
        self.controllers
            .echoes
            .retain(|(_, _, t)| now.saturating_duration_since(*t) < ECHO_KEEP);
        let todo = self.feedback_mappings();
        if todo.is_empty() {
            return;
        }
        let names = self.midi_port_names.clone();
        for (scope, m) in todo {
            let key = (scope, m.id);
            if self
                .controllers
                .activity
                .get(&key)
                .is_some_and(|t| t.elapsed() < QUIET_AFTER_MOVE)
            {
                continue;
            }
            let Some(pos) = self
                .resolve_target(&m.target)
                .and_then(|t| self.controller_pos(&t))
            else {
                continue;
            };
            let value = m.feedback_value(pos);
            if self.controllers.sent.get(&key) == Some(&value) {
                continue;
            }
            let Some(port) = m.source.port.index_in(&names) else {
                continue;
            };
            let channel = m.source.channel.unwrap_or(0);
            let messages = feedback_messages(m.source.control, channel, value);
            if messages.is_empty() {
                continue;
            }
            let mut all = true;
            for msg in &messages {
                // Stamped before it goes: a loopback's echo can arrive (and be stamped)
                // before the send returns (review 3, #14).
                let sent_at = Instant::now();
                let ok = match self.controllers.feedback_log.as_mut() {
                    Some(log) => {
                        log.push((names[port].clone(), msg.clone()));
                        true
                    }
                    None => self.controllers.outputs.send(&names[port], msg),
                };
                if ok {
                    let mut b = [0u8; 3];
                    b[..msg.len()].copy_from_slice(msg);
                    let echoes = &mut self.controllers.echoes;
                    if echoes.len() >= ECHO_MAX {
                        echoes.remove(0);
                    }
                    echoes.push((port as u8, b, sent_at));
                }
                all &= ok;
            }
            // Counted as sent only when every message went: a port without an output is
            // asked again (it answers at once: `MidiOutputs` remembers it is missing).
            if all {
                self.controllers.sent.insert(key, value);
            }
        }
    }
}
