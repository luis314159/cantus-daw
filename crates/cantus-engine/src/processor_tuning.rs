//! r42:tuning — the tuning on the audio thread (ADR 0155, 0156): each track's table handed
//! to its instrument after every configure, and the MTS-ESP master's table read once per
//! live block for the tracks that follow it. Nothing here allocates (`rt_memory.rs`).

use super::*;
use cantus_core::tuning::TuningTable;

impl Processor {
    /// The master's table into every track that follows it, when it changed. A live block
    /// only: an offline render plays the table its snapshot was compiled with (ADR 0156 —
    /// a render is a snapshot).
    ///
    /// r42 fix-engine: a track whose master went back to the table it was compiled with
    /// (review correctness 8); and every hosted instrument that reads MTS-ESP itself is told
    /// what MTS-ESP gives it now — the outside master's table, or the project's while Cantus
    /// is the master — so it is handed only the rest (review correctness 1).
    pub(super) fn refresh_external_tuning(&mut self) {
        if self.offline {
            return;
        }
        let Some(snap) = self.snapshot.as_ref() else {
            return;
        };
        let mut table = TuningTable::EQUAL;
        // One look per block, shared by every track (and none when no track needs one).
        let mut looked: Option<(bool, bool)> = None;
        for (ti, t) in snap.tracks.iter().enumerate() {
            let Some(&slot) = self.track_to_slot.get(ti) else {
                continue;
            };
            if slot >= self.ready.0 {
                continue;
            }
            let Some(mut s) = self.nodes.track(slot) else {
                continue;
            };
            if !s.tuning_external && !s.tuning_by_mts {
                continue;
            }
            let ext = &self.external_tuning;
            let (leads, read) = *looked.get_or_insert_with(|| {
                let leads = ext.leads();
                (leads, !leads && ext.read(&mut table))
            });
            let heard = if leads {
                Some(&*snap.tuning.table)
            } else if read {
                Some(&table)
            } else {
                None
            };
            s.set_heard(heard);
            if s.tuning_external {
                if read {
                    s.set_tuning(&table);
                    s.tuning_live = true;
                } else if s.tuning_live {
                    s.set_tuning(&t.tuning.table);
                    s.tuning_live = false;
                }
            }
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
