//! The mix cues' switches on the master (round 36, ADR 0093 §3): its inserts' switch and
//! each insert's bypass, written in place (no allocation).

use crate::processor::Processor;

impl Processor {
    /// Region switches for the master's chain.
    pub(crate) fn scene_master(&mut self, fx_enabled: bool, bypass: &[bool]) {
        let m = &mut self.master;
        m.fx_enabled = fx_enabled;
        let n = m.effects.len().min(m.bypass.len());
        for (i, b) in bypass.iter().enumerate().take(n) {
            m.bypass[i] = *b;
        }
    }
}
