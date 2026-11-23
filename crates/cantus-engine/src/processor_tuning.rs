//! r42:tuning — the tuning on the audio thread (ADR 0155, 0156): each track's table handed
//! to its instrument after every configure, and the MTS-ESP master's table read once per
//! live block for the tracks that follow it. Nothing here allocates (`rt_memory.rs`).

use super::*;
use cantus_core::tuning::TuningTable;

impl Processor {
    /// The master's table into every track that follows it, when it changed. A live block
    /// only: an offline render plays the table its snapshot was compiled with (ADR 0156 —
    /// a render is a snapshot).
    pub(super) fn refresh_external_tuning(&mut self) {
        if self.offline {
            return;
        }
        let mut table = TuningTable::EQUAL;
        let mut read = false;
        for i in 0..self.ready.0 {
            let Some(mut s) = self.nodes.track(i) else {
                continue;
            };
            if !s.tuning_external {
                continue;
            }
            if !read {
                // One read per block, shared by every track (no master: nothing changes).
                if !self.external_tuning.read(&mut table) {
                    return;
                }
                read = true;
            }
            s.set_tuning(&table);
        }
    }
}

/// The tuning a strip's pitched effects follow (ADR 0155): its tracks' when every track that
/// feeds it plays in one (a track's own strip), the project's otherwise (a bus, a strip no
/// track feeds). Allocation-free.
pub(crate) fn strip_tuning(snapshot: &cantus_core::Snapshot, strip: usize) -> &TuningTable {
    let mut found: Option<&TuningTable> = None;
    for t in snapshot.tracks.iter().filter(|t| t.strip == Some(strip)) {
        match found {
            None => found = Some(&t.tuning.table),
            Some(f) if cantus_dsp::tuning::same(&f.0, &t.tuning.table.0) => {}
            Some(_) => return &snapshot.tuning.table,
        }
    }
    found.unwrap_or(&snapshot.tuning.table)
}
