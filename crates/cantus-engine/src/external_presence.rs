//! What the interface knows about the external units now (ADR 0094 §1, §4): which of them can
//! play with the device and ports that are open, and the interface's own reported round trip
//! — applied to every snapshot the handle compiles.
//!
//! A unit that is not available (another interface, its channels or port missing, no device)
//! gives the engine **no link**: its track plays as if the unit were not there — no send is
//! made on a device that does not have the pair, no note goes to a port that is not open.
//! A ready unit's stored trip is **re-based** by the interface's round trip now (a buffer size
//! changed since the ping: the trip less the round trip reported then plus the one now).

use cantus_core::external_unit::{ExternalUnitId, Now};
use cantus_core::Snapshot;

/// The interface layer's view, set on the handle (`EngineHandle::set_external_presence`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExternalPresence {
    /// The units that can play now.
    pub ready: Vec<ExternalUnitId>,
    /// The interface now: its name, reported round trip, buffer and rate (`None`: no device).
    pub now: Option<Now>,
}

/// An offline render's snapshot: no outboard gear at all (the processor also plays none
/// offline, `Processor::set_offline`).
pub fn strip(snapshot: &mut Snapshot) {
    for t in snapshot.tracks.iter_mut() {
        t.external.clear();
    }
}

/// Apply `p` to a compiled snapshot: links of units not ready are taken out, the trips of the
/// others re-based.
pub fn apply(snapshot: &mut Snapshot, p: &ExternalPresence) {
    for t in snapshot.tracks.iter_mut() {
        if t.external.is_empty() {
            continue;
        }
        t.external.retain(|l| p.ready.contains(&l.unit));
        if let Some(now) = &p.now {
            for l in t.external.iter_mut() {
                if let Some(m) = l.latency.as_ref() {
                    l.latency = Some(m.rebased(now));
                }
            }
        }
    }
}

impl crate::EngineHandle {
    /// What the interface knows about the external units now; `true` when it changed (the
    /// caller sends the project again so the next snapshot carries it). r36:external
    pub fn set_external_presence(&mut self, p: ExternalPresence) -> bool {
        if self.external_presence.as_ref() == Some(&p) {
            return false;
        }
        self.external_presence = Some(p);
        true
    }
}
