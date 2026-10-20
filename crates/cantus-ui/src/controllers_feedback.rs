//! Feedback to the controller (round 29, ADR 0062 §3): a mapping with feedback on sends its
//! target's value back on its control — the output port of the same device, found by name —
//! whenever the value changes by anything (the mouse, undo, automation, another mapping),
//! so a motor fader follows and an LED ring or a pad's light shows where the parameter is.
//! Never while that mapping's own control is moving (a motor fader would fight the hand).

use crate::app::CantusApp;
use crate::controllers::Scope;
use cantus_core::controller::{Control, Mapping};
use std::time::Duration;

/// A mapping whose control moved this recently sends nothing back.
const QUIET_AFTER_MOVE: Duration = Duration::from_millis(250);

/// The bytes that put `control` on `channel` at `value` (its own range: 7 or 14 bits).
pub fn feedback_bytes(control: Control, channel: u8, value: u16) -> Vec<u8> {
    let ch = channel & 0x0F;
    let v7 = value.min(127) as u8;
    match control {
        Control::Cc(n) => vec![0xB0 | ch, n & 0x7F, v7],
        Control::Cc14(n) => {
            let v = value.min(16_383);
            vec![
                0xB0 | ch,
                n & 31,
                (v >> 7) as u8,
                0xB0 | ch,
                (n & 31) + 32,
                (v & 0x7F) as u8,
            ]
        }
        Control::Note(n) => vec![0x90 | ch, n & 0x7F, v7],
        // A motor fader on the pitch wheel (Mackie's way).
        Control::PitchBend => {
            let v = value.min(16_383);
            vec![0xE0 | ch, (v & 0x7F) as u8, (v >> 7) as u8]
        }
        Control::ChannelPressure => vec![0xD0 | ch, v7],
        Control::PolyPressure(n) => vec![0xA0 | ch, n & 0x7F, v7],
        // A program change has no value to show.
        Control::Program(_) => Vec::new(),
    }
}

impl CantusApp {
    /// Send every feedback mapping's value that changed.
    pub(crate) fn controller_feedback(&mut self) {
        let mut todo: Vec<(Scope, Mapping)> = Vec::new();
        for scope in [Scope::Project, Scope::Global] {
            todo.extend(
                self.controller_map(scope)
                    .mappings
                    .iter()
                    .filter(|m| m.feedback)
                    .map(|m| (scope, m.clone())),
            );
        }
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
            let bytes = feedback_bytes(m.source.control, m.source.channel.unwrap_or(0), value);
            if bytes.is_empty() {
                continue;
            }
            // A port with no output is tried once (and its value counted as sent).
            self.controllers.outputs.send(&names[port], &bytes);
            self.controllers.sent.insert(key, value);
        }
    }
}
