//! Recording automation (ADR 0057): while a pass records and the project's record options
//! say so (`RecordSettings::record_automation`, on by default, as FL), every parameter
//! moved — a knob in Cantus, or a parameter in a hosted plug-in's own window, which the
//! host already turns into commands — is written where it was heard; when the pass ends,
//! each parameter moved becomes an automation clip over the stretch it was moved in, in
//! **one undo step**. A loop pass that comes round again writes over what it wrote before,
//! as FL's does.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::automation::edit::{cover, from_samples};
use cantus_core::automation::range::range_of;
use cantus_core::{AutoTarget, Automation, Command, Ticks};
use std::collections::BTreeMap;

/// What a pass has recorded so far: per parameter, (heard tick, position 0…1), in the
/// order they came (a loop's wrap brings the ticks back).
#[derive(Debug, Default)]
pub struct AutoRec {
    pub points: BTreeMap<AutoTarget, Vec<(i64, f32)>>,
}

impl CantusApp {
    /// A command moved a parameter: written into the pass's automation when a pass is
    /// recording it.
    pub fn record_tweak(&mut self, cmd: &Command) {
        self.record_tweak_at(cmd, None);
    }

    /// [`Self::record_tweak`] at `tick` (a controller's message, at its own time: round 29,
    /// ADR 0062), or at the heard position now.
    pub fn record_tweak_at(&mut self, cmd: &Command, tick: Option<i64>) {
        if self.rec.pass.is_none() || !self.project().recording.record_automation {
            return;
        }
        let Some((target, v)) = AutoTarget::value_of_command(cmd) else {
            return;
        };
        let pos = if target.is_plugin() {
            let Some((_, info)) = self.plugin_param(&target) else {
                return;
            };
            let span = info.max - info.min;
            if span.abs() > 0.0 {
                ((v - info.min) / span).clamp(0.0, 1.0) as f32
            } else {
                0.0
            }
        } else {
            match range_of(self.project(), &target) {
                Some(r) => r.pos(v as f32),
                None => return,
            }
        };
        let tick = tick.unwrap_or(self.engine.position().0);
        self.modulators
            .rec
            .points
            .entry(target)
            .or_default()
            .push((tick, pos));
    }

    /// What hosted plug-ins' own windows did since the last frame (the host writes their
    /// gestures to the bus itself and keeps each command it ran): every parameter they
    /// moved is recorded when a pass records, and the newest is *last tweaked*.
    pub fn host_tweaks(&mut self) {
        for cmd in self.plugins.take_tweaks() {
            // `note_tweak` records it too, when a pass records.
            self.note_tweak(&cmd);
        }
    }

    /// A pass starts: nothing recorded yet.
    pub fn begin_automation_pass(&mut self) {
        self.modulators.rec.points.clear();
    }

    /// The pass ended at `stop`: every parameter it moved becomes a clip, one undo step.
    pub fn commit_recorded_automation(&mut self, stop: Ticks) -> bool {
        let recorded = std::mem::take(&mut self.modulators.rec.points);
        if recorded.is_empty() {
            return false;
        }
        let selected = self.selected_track;
        let mut commands = Vec::new();
        let mut opened = Vec::new();
        let mut n = 0;
        for (target, raw) in recorded {
            let samples = overwrite_passes(&raw);
            let (Some(first), Some(last)) = (samples.first(), samples.last()) else {
                continue;
            };
            let from = Ticks(first.0);
            let to = Ticks(stop.0.max(last.0 + 1));
            let project = self.bus.project();
            let Some(track) = crate::automation::host_track(project, &target, selected) else {
                continue;
            };
            let mut points = from_samples(&samples, from);
            // It holds its last value to the end of the clip.
            if let Some(p) = points.last().copied() {
                points.push(cantus_core::AutoPoint::new(to - from, p.value));
            }
            commands.extend(cover(project, &target, from, to));
            let name = self.target_name(&target);
            let mut clip = self.bus.new_clip(name, from, to - from);
            clip.automation = Some(Automation::new(target, points));
            commands.push(Command::InsertClip {
                track,
                clip: Box::new(clip),
            });
            if !opened.contains(&track)
                && !self
                    .project()
                    .track(track)
                    .is_some_and(|t| t.automation_open)
            {
                opened.push(track);
                commands.push(Command::SetTrackAutomationOpen {
                    id: track,
                    open: true,
                });
            }
            n += 1;
        }
        if commands.is_empty() {
            return false;
        }
        let ok = self.exec(Command::Batch {
            label_key: "mods.cmd.record_automation".into(),
            commands,
        });
        if ok {
            self.set_status(tf("mods.status.recorded", &[("n", &n)]));
        } else {
            self.set_error(t("mods.status.record_failed"));
        }
        ok
    }
}

/// The samples of every pass round a loop, each later pass written over the stretch it
/// covers: in tick order, the newest value at any tick.
fn overwrite_passes(raw: &[(i64, f32)]) -> Vec<(i64, f32)> {
    // Split where the ticks go back (a wrap), then lay the passes in order.
    let mut passes: Vec<&[(i64, f32)]> = Vec::new();
    let mut start = 0;
    for i in 1..raw.len() {
        if raw[i].0 < raw[i - 1].0 {
            passes.push(&raw[start..i]);
            start = i;
        }
    }
    if start < raw.len() {
        passes.push(&raw[start..]);
    }
    let mut out: Vec<(i64, f32)> = Vec::new();
    for pass in passes {
        let (a, b) = (pass[0].0, pass[pass.len() - 1].0);
        out.retain(|(t, _)| *t < a || *t > b);
        out.extend_from_slice(pass);
        out.sort_by_key(|x| x.0);
    }
    // At one tick, the last value written.
    let mut dedup: Vec<(i64, f32)> = Vec::with_capacity(out.len());
    for s in out {
        match dedup.last_mut() {
            Some(l) if l.0 == s.0 => l.1 = s.1,
            _ => dedup.push(s),
        }
    }
    dedup
}

#[cfg(test)]
mod tests {
    use super::overwrite_passes;

    #[test]
    fn a_loop_pass_writes_over_the_stretch_it_covers() {
        let raw = [
            (0, 0.1),
            (100, 0.2),
            (200, 0.3),
            (300, 0.4),
            // The loop came round: the second pass covers 100…200 only.
            (100, 0.9),
            (150, 0.8),
            (200, 0.7),
        ];
        assert_eq!(
            overwrite_passes(&raw),
            vec![(0, 0.1), (100, 0.9), (150, 0.8), (200, 0.7), (300, 0.4)]
        );
    }
}
