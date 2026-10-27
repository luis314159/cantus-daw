//! Recording automation (ADR 0057), now the **Latch** case of writing (ADR 0068): while a
//! pass records and the project's record options say so (`RecordSettings::record_automation`,
//! on by default, as FL), every parameter moved — a knob in Cantus, a parameter in a hosted
//! plug-in's own window, a mapped controller — is written where it was heard, and when the
//! pass ends each becomes an automation clip over the stretch it was moved in, in **one
//! undo step**. A loop pass that comes round again writes over what it wrote before. The
//! writing itself is `auto_write` (the pass's `Writer`): these are the entry points the
//! commands, the controllers and the recording call.

use crate::app::CantusApp;
use cantus_core::automation::write::overlay;
use cantus_core::{AutoTarget, Command, Ticks};
use std::collections::BTreeMap;

impl CantusApp {
    /// A command moved a parameter: written when a pass writes it.
    pub fn record_tweak(&mut self, cmd: &Command) {
        self.record_tweak_at(cmd, None);
    }

    /// [`Self::record_tweak`] at `tick` (a controller's message, at its own time: round 29,
    /// ADR 0062), or at the heard position now.
    pub fn record_tweak_at(&mut self, cmd: &Command, tick: Option<i64>) {
        if !self.automation.write.writing() {
            return;
        }
        // A batch (ganged faders, a controller's knobs at once) writes every member.
        if let Command::Batch { commands, .. } = cmd {
            for c in commands {
                self.record_tweak_at(c, tick);
            }
            return;
        }
        let Some((target, pos)) = self.command_pos(cmd) else {
            return;
        };
        let tick = tick.unwrap_or(self.engine.position().0);
        self.write_value(target, pos, tick);
    }

    /// A parameter at `pos` (0…1 of its travel) at `tick`, into the pass (what a controller
    /// writes: its target and position, whatever its command, ADR 0062). Outside the pass's
    /// window — the count-in, the pre-roll, past the punch-out — it is where the pass
    /// begins, or nothing.
    pub fn record_point_at(&mut self, target: AutoTarget, pos: f32, tick: i64) {
        self.write_value(target, pos, tick);
    }

    /// What hosted plug-ins' own windows did since the last frame (the host writes their
    /// gestures to the bus itself and keeps each command it ran): every parameter they
    /// moved is written when a pass writes it, and the newest is *last tweaked*.
    pub fn host_tweaks(&mut self) {
        for cmd in self.plugins.take_tweaks() {
            // `note_tweak` writes it too, when a pass writes.
            self.note_tweak(&cmd);
        }
    }

    /// A recording pass starts: it writes automation when its options say so; no wrap seen
    /// yet.
    pub fn begin_automation_pass(&mut self) {
        self.begin_recorded_write();
        // The seek to the pass's start is no wrap.
        self.controllers.last_wrap = None;
        self.controllers.prev_pos = None;
    }

    /// The pass ended at `stop`: every parameter it moved becomes a clip, one undo step.
    pub fn commit_recorded_automation(&mut self, stop: Ticks) -> bool {
        self.commit_recorded_automation_in(stop, Ticks(i64::MIN), None)
    }

    /// [`Self::commit_recorded_automation`] for a pass that began at `start` (the record
    /// start or the punch-in) and, punched, ended at `end`: a clip never runs past the
    /// punch-out (review 3, #16), and a pass stopped before its start — a false start in
    /// the count-in or the pre-roll — writes nothing (#18): its window was never entered.
    pub fn commit_recorded_automation_in(
        &mut self,
        stop: Ticks,
        start: Ticks,
        end: Option<Ticks>,
    ) -> bool {
        let _ = start;
        self.finish_write_pass(stop, end)
    }

    /// What the pass has written so far, per target: its runs laid in pass order (the
    /// last pass wins inside a loop), as `(tick, position)`.
    pub fn written_points(&self) -> BTreeMap<AutoTarget, Vec<(i64, f32)>> {
        self.automation
            .write
            .written()
            .into_iter()
            .map(|(t, runs)| (t, overlay(&runs).concat()))
            .collect()
    }
}
