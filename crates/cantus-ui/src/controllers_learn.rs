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
    learn::detect_in, Control, Mapping, MappingTarget, MidiSource, PortRef, TEMPO_TRAVEL,
};
use cantus_engine::midi::Heard;
use std::time::{Duration, Instant};

/// How long a knob is listened to before it is learnt: enough for a turn to show whether it
/// is an encoder or a 14-bit fader.
pub const LEARN_WINDOW: Duration = Duration::from_millis(300);
/// An armed learn that hears nothing this long lets go: the keyboard plays again (every
/// message is the control being learnt meanwhile, review 1, #32).
pub const LEARN_TIMEOUT: Duration = Duration::from_secs(15);
/// A controller that sent only its top value (127) is a button being held: its release
/// is waited for this long — a momentary button (press, release) toggles; one that never
/// lets go is a button latching in the controller (review 2, #2).
pub const HOLD_WINDOW: Duration = Duration::from_millis(1500);
/// Enough messages to tell: learnt at once.
const ENOUGH: usize = 8;

impl CantusApp {
    /// A message while a target is armed.
    pub(crate) fn learn_heard(&mut self, h: &Heard) {
        let (channel, decoded, n) = decode(&mut self.controllers.wide, h.port, &h.event, h.at);
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
                // An NRPN or RPN — its number on CC 99/98 or 101/100, its value by data
                // entry on CC 6/38 or 96/97, which many controllers send alone while the
                // knob turns: not a control Cantus maps (review 3, #6; review 4, #8). Said,
                // and passed over: the learn stays armed (the burst still swallowed) for the
                // next control moved (review 4, #24).
                if matches!(control, Control::Cc(6 | 38 | 96 | 97 | 98..=101)) {
                    self.set_error(crate::i18n::t("ctl.status.nrpn"));
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
                // A program change after bank select: the keyboard's program button, learnt
                // now (its program change must not reach the instrument, review 2, #6).
                if cantus_core::controller::learn::program_after_bank(&l.heard).is_some() {
                    self.finish_learning();
                }
            }
            // Another port or channel while one is being learnt: not this control.
            Some(_) => {}
        }
    }

    /// Learn the armed control once it has said enough.
    pub(crate) fn learn_tick(&mut self) {
        let ready = self.controllers.learning.as_ref().is_some_and(|l| {
            let held = held_button(&l.heard);
            let window = if held { HOLD_WINDOW } else { LEARN_WINDOW };
            l.heard.len() >= ENOUGH
                || released_button(&l.heard)
                || l.first.is_some_and(|t| t.elapsed() >= window)
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

    /// The hosted plug-in a learnt mapping of `target` belongs to, with its name and the
    /// parameter's (review 4, #47).
    fn learnt_plugin(&self, target: &MappingTarget) -> Option<cantus_core::controller::PluginRef> {
        let mut r = target.plugin_of(self.project())?;
        if let MappingTarget::Param(a) = target {
            if let Some(info) = self.hosted_param_info(a) {
                r.param = info.name.clone();
            }
        }
        Some(r)
    }

    /// Make the mapping the armed control asks for.
    pub(crate) fn finish_learning(&mut self) {
        let Some(l) = self.controllers.learning.take() else {
            return;
        };
        // Armed from a control that stands for two values: read as two states (a pad
        // toggles between them).
        let mut shape = self.target_shape(&l.target);
        if l.range.is_some() {
            shape.switch = true;
        }
        let (Some((port, channel)), Some((control, mode))) =
            (l.source, detect_in(&l.heard, &l.target, shape))
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
            // Learnt again: the same mapping, the new control — on the plug-in there now
            // (review 2, #1).
            Some(old) => {
                let mut m = Mapping {
                    source,
                    mode,
                    plugin: self.learnt_plugin(&l.target),
                    ..old
                };
                if let Some((from, to)) = l.range {
                    (m.min, m.max) = (from, to);
                }
                m
            }
            None => {
                let id = self.new_mapping_id();
                let mut m = Mapping::new(id, source, l.target.clone(), mode);
                m.takeover = self.settings.controller_takeover;
                // A hosted parameter's id means something only in its plug-in; its names
                // name the row if the plug-in goes (review 4, #47).
                m.plugin = self.learnt_plugin(&l.target);
                if let Some((from, to)) = l.range {
                    (m.min, m.max) = (from, to);
                }
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
                // Mappings of a swapped-out plug-in drive nothing: not "also" (review 3).
                self.controller_map(s)
                    .sharing_in(&m, self.project())
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

/// A controller heard sending only its top value, a few times at most: a button held down,
/// whose release is still to come.
pub(crate) fn held_button(heard: &[(Control, u16)]) -> bool {
    matches!(heard.first(), Some((Control::Cc(_), 127)))
        && heard.len() < 3
        && heard.iter().all(|&(c, v)| c == heard[0].0 && v == 127)
}

/// A controller button pressed and let go (its two ends heard): nothing more to wait for.
fn released_button(heard: &[(Control, u16)]) -> bool {
    match heard.first() {
        Some(&(c @ Control::Cc(_), 127)) => heard.iter().any(|&(o, v)| o == c && v == 0),
        _ => false,
    }
}
