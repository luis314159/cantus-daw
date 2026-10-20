//! The other way round from [`AutoTarget::value_of_command`] (round 29, ADR 0062): the
//! command that sets a target to a value in the parameter's own units — what a controller
//! mapping, a script or anything else that holds an [`AutoTarget`] and a value sends.
//! `AutoTarget::value_of_command(&t.command(p, v)?) == Some((t, v))` for every target a
//! command sets, and `command(value_of_command(c)) == c` for every command it reads.

use super::AutoTarget;
use crate::command::Command;
use crate::instrument::InstrumentParam;
use crate::model::Project;

impl AutoTarget {
    /// The command that sets this target to `value` (its own units: dB for a level, a
    /// hosted parameter in the plug-in's range, a modulator's amount 0…1), or `None` when
    /// the target names nothing in `project` or the value is not a number. The value is
    /// passed as it is: the command validates (and clamps) it as it does a mouse's.
    pub fn command(&self, project: &Project, value: f64) -> Option<Command> {
        if !value.is_finite() {
            return None;
        }
        let v = value as f32;
        Some(match self {
            AutoTarget::TrackGain(id) => {
                project.track(*id)?;
                Command::SetTrackGain {
                    id: *id,
                    gain_db: v,
                }
            }
            AutoTarget::TrackPan(id) => {
                project.track(*id)?;
                Command::SetTrackPan { id: *id, pan: v }
            }
            AutoTarget::StripGain(id) => {
                project.strip(*id)?;
                Command::SetStripGain {
                    id: *id,
                    gain_db: v,
                }
            }
            AutoTarget::StripPan(id) => {
                project.strip(*id)?;
                Command::SetStripPan { id: *id, pan: v }
            }
            AutoTarget::StripWidth(id) => {
                project.strip(*id)?;
                Command::SetStripWidth { id: *id, width: v }
            }
            AutoTarget::Send { strip, to } => {
                let mut send = *project.strip(*strip)?.send_to(*to)?;
                send.level = v;
                Command::SetStripSend { id: *strip, send }
            }
            AutoTarget::MasterGain => Command::SetMasterGain { gain_db: v },
            AutoTarget::MasterPan => Command::SetMasterPan { pan: v },
            AutoTarget::MasterWidth => Command::SetMasterWidth { width: v },
            AutoTarget::Effect { effect, key } => {
                let strip = project.effect_host(*effect)?;
                project.effect(strip, *effect)?.spec.kind.param(key)?;
                Command::SetEffectParam {
                    strip,
                    effect: *effect,
                    key: key.clone(),
                    value: v,
                }
            }
            AutoTarget::Instrument { track, key } => {
                project
                    .track(*track)?
                    .instrument
                    .native()?
                    .kind
                    .param(key)?;
                Command::SetInstrumentValue {
                    track: *track,
                    key: key.clone(),
                    value: v,
                }
            }
            AutoTarget::Firmus { track, param } => {
                project.track(*track)?.instrument.firmus()?;
                Command::SetInstrumentParam {
                    track: *track,
                    param: InstrumentParam::Firmus(*param),
                    value: v,
                }
            }
            AutoTarget::Plugin { effect, id } => {
                let strip = project.effect_host(*effect)?;
                project.effect(strip, *effect)?.spec.external.as_ref()?;
                Command::SetPluginParam {
                    strip,
                    effect: *effect,
                    id: *id,
                    value: Some(value),
                }
            }
            AutoTarget::PluginInstrument { track, id } => {
                project.track(*track)?.instrument.external()?;
                Command::SetInstrumentPluginParam {
                    track: *track,
                    id: *id,
                    value: Some(value),
                }
            }
            AutoTarget::ModAmount(id) => {
                let (_, _, m) = project.modulator(*id)?;
                let mut m = m.clone();
                m.amount = v;
                Command::SetModulator {
                    modulator: Box::new(m),
                }
            }
        })
    }
}
