//! Learning a control (round 29, ADR 0062 §2): while a target is armed, the first message
//! from any port names the port and the channel; a key, a pad or a program change is learnt
//! at once, a knob after a short window that tells a plain knob from a 14-bit fader or an
//! endless encoder (`cantus_core::controller::learn::detect`). Then the mapping is made in
//! the target's map, and the status line says what was learnt — and what else that control
//! drives, when it drives something already (allowed: one knob on several targets).

use crate::app::CantusApp;
use crate::controllers::{decode, source_text, Scope};
use crate::i18n::tf;
use cantus_core::controller::{
    learn::detect, Mapping, MappingTarget, MidiSource, PortRef, TEMPO_TRAVEL,
};
use cantus_engine::midi::Heard;
use std::time::{Duration, Instant};

/// How long a knob is listened to before it is learnt: enough for a turn to show whether it
/// is an encoder or a 14-bit fader.
pub const LEARN_WINDOW: Duration = Duration::from_millis(300);
/// An armed learn that hears nothing this long lets go: the keyboard plays again (every
/// message is the control being learnt meanwhile, review 1, #32).
pub const LEARN_TIMEOUT: Duration = Duration::from_secs(15);
/// Enough messages to tell: learnt at once.
const ENOUGH: usize = 8;

impl CantusApp {
    /// A message while a target is armed.
    pub(crate) fn learn_heard(&mut self, h: &Heard) {
        let (channel, decoded, n) = decode(&mut self.controllers.wide, h.port, &h.event);
        if n == 0 {
            return;
        }
        // The 7-bit reading: detection pairs a coarse controller with its fine twin itself.
        let (control, value, on) = decoded[0];
        let Some(l) = self.controllers.learning.as_mut() else {
            return;
        };
        match l.source {
            None => {
                if !on {
                    // A release with nothing pressed (a key let go after arming).
                    return;
                }
                l.source = Some((h.port, channel));
                l.first = Some(Instant::now());
                l.heard.push((control, value));
                if control.is_button() {
                    self.finish_learning();
                }
            }
            Some((p, c)) if p == h.port && c == channel => {
                if on && l.heard.len() < 64 {
                    l.heard.push((control, value));
                }
            }
            // Another port or channel while one is being learnt: not this control.
            Some(_) => {}
        }
    }

    /// Learn the armed control once it has said enough.
    pub(crate) fn learn_tick(&mut self) {
        let ready = self.controllers.learning.as_ref().is_some_and(|l| {
            l.heard.len() >= ENOUGH || l.first.is_some_and(|t| t.elapsed() >= LEARN_WINDOW)
        });
        if ready {
            self.finish_learning();
            return;
        }
        let stale = self
            .controllers
            .learning
            .as_ref()
            .is_some_and(|l| l.first.is_none() && l.armed_at.elapsed() >= LEARN_TIMEOUT);
        if stale {
            self.cancel_learning();
        }
    }

    /// Make the mapping the armed control asks for.
    pub(crate) fn finish_learning(&mut self) {
        let Some(l) = self.controllers.learning.take() else {
            return;
        };
        let (Some((port, channel)), Some((control, mode))) =
            (l.source, detect(&l.heard, &l.target))
        else {
            self.controllers.tap.filter().set_learning(false);
            return;
        };
        let source = MidiSource {
            port: PortRef::of(&self.midi_port_names, usize::from(port)),
            channel: Some(channel),
            control,
        };
        let existing = l
            .replace
            .and_then(|id| self.controller_map(l.scope).get(id).cloned());
        let replacing = existing.is_some();
        let m = match existing {
            // Learnt again: the same mapping, the new control.
            Some(old) => Mapping {
                source,
                mode,
                ..old
            },
            None => {
                let id = self.new_mapping_id();
                let mut m = Mapping::new(id, source, l.target.clone(), mode);
                m.takeover = self.settings.controller_takeover;
                // A hosted parameter's id means something only in its plug-in.
                m.plugin = l.target.plugin_of(self.project());
                if l.target == MappingTarget::Tempo {
                    // A knob covers 60…180 BPM to start with (the range narrows or widens it).
                    let (lo, hi) = TEMPO_TRAVEL;
                    m.min = ((60.0 - lo) / (hi - lo)) as f32;
                    m.max = ((180.0 - lo) / (hi - lo)) as f32;
                }
                m
            }
        };
        if replacing {
            self.set_mapping(l.scope, m.clone(), None);
        } else {
            self.add_mapping(l.scope, m.clone());
        }
        // The control is the map's before the tap stops taking everything: its next moves
        // drive the target, none reaches the instrument (review 1, #16).
        self.publish_controller_filter();
        self.controllers.tap.filter().set_learning(false);
        self.controllers.states.remove(&(l.scope, m.id));
        let others: Vec<String> = [Scope::Project, Scope::Global]
            .into_iter()
            .flat_map(|s| {
                self.controller_map(s)
                    .sharing(&m)
                    .map(|o| o.target.clone())
                    .collect::<Vec<_>>()
            })
            .map(|t| self.mapping_target_name(&t))
            .collect();
        let name = self.mapping_target_name(&m.target);
        let what = source_text(&m.source);
        let text = if others.is_empty() {
            tf("ctl.status.learnt", &[("source", &what), ("name", &name)])
        } else {
            tf(
                "ctl.status.learnt_shared",
                &[
                    ("source", &what),
                    ("name", &name),
                    ("others", &others.join(", ")),
                ],
            )
        };
        self.set_status(text);
    }
}
