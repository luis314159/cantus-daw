//! VCAs on the audio thread (ADR 0093): the decibels a member's fader lane adds for the
//! VCAs that follow it, evaluated at the same control point as the lane itself, so the
//! sum is one ramp from this grid point to the next — block-size independent, offline
//! the same as live, serial the same as parallel (nothing here reads another thread's
//! state).

use super::{At, Runtime};
use cantus_core::vca::sum_db;
use cantus_core::Snapshot;

impl Runtime {
    /// What lane `li` adds, in dB, at this control point and at the next, or `None` when
    /// the snapshot links no VCA to it. The link is found by index, never by a search.
    pub(super) fn vca_extra(&self, snapshot: &Snapshot, li: usize, at: &At) -> Option<(f32, f32)> {
        let k = *snapshot.vca_by_lane.get(li)?;
        let link = snapshot.vca_links.get(k as usize)?;
        let (mut a, mut b) = (link.static_db, link.static_db);
        for &j in &link.from {
            let (Some(lane), Some(rt)) = (snapshot.automation.get(j), self.lanes.get(j)) else {
                continue;
            };
            let (ma, mb) = self.mod_sum(j);
            let value = |tick: f64, m: f32| {
                if m == 0.0 {
                    lane.value_at(tick)
                } else {
                    let p = lane.pos_at(tick).unwrap_or(rt.base_pos);
                    lane.range.value((p + m).clamp(0.0, 1.0))
                }
            };
            a = sum_db(a, value(at.tick, ma));
            b = sum_db(b, value(at.next_tick, mb));
        }
        Some((a, b))
    }
}

impl super::Ramp {
    /// A mix cue's fade (ADR 0093 §3) on a segment `k0` samples after its control point:
    /// each sample scaled by the ramp, held at its end (0 or 1) past it.
    #[inline]
    pub(crate) fn fade(&self, l: &mut [f32], r: &mut [f32], k0: u64) {
        for (i, (x, y)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            let g = self.at(k0 + i as u64).clamp(0.0, 1.0);
            *x *= g;
            *y *= g;
        }
    }
}
