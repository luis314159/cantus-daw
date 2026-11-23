//! The tuning's engine side (round 42, ADR 0155) and the MTS-ESP seam (ADR 0156). Owned by
//! the `tuning` builder; `tuning-face` implements [`ExternalTuning`] in `cantus-rt::mts`.
//!
//! The engine never loads a library itself (`forbid(unsafe_code)`): the app hands it an
//! [`ExternalTuning`] at start-up, and a track whose tuning is the MTS-ESP master's reads its
//! table through it once per block.

use cantus_core::tuning::TuningTable;

/// The MTS-ESP master's table, read and published (ADR 0156). Every method may be called on
/// the audio thread: an implementation never locks, blocks, allocates or does I/O there.
pub trait ExternalTuning: Send + Sync + std::fmt::Debug {
    /// Write the master's table into `out` (semitones from 12-TET at A440 per key); `false`
    /// — leaving `out` as it was — when there is no master.
    fn read(&self, out: &mut TuningTable) -> bool;
    /// Publish `table` as Cantus's, when Cantus is the master; nothing otherwise.
    fn publish(&self, table: &TuningTable) {
        let _ = table;
    }
}

/// No MTS-ESP: no master to read, nothing published — every machine without the library,
/// and every test.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoExternal;

impl ExternalTuning for NoExternal {
    fn read(&self, _out: &mut TuningTable) -> bool {
        false
    }
}

/// r42:tuning — the master's table, read now, into every track of `snapshot` whose tuning
/// is `External` (ADR 0156): what an offline render plays from its start, and what a live
/// engine plays until its first block reads the master itself. Without a master the track
/// keeps the table it was compiled with (equal temperament at its diapason). Main thread.
pub fn fill_external(snapshot: &mut cantus_core::Snapshot, ext: &dyn ExternalTuning) {
    let mut table = TuningTable::EQUAL;
    let mut read: Option<bool> = None;
    for t in snapshot.tracks.iter_mut().filter(|t| t.tuning.external) {
        if *read.get_or_insert_with(|| ext.read(&mut table)) {
            t.tuning.table = std::sync::Arc::new(table);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A master with a fixed table (the tests' stand-in for MTS-ESP).
    #[derive(Debug)]
    struct Fixed(TuningTable);

    impl ExternalTuning for Fixed {
        fn read(&self, out: &mut TuningTable) -> bool {
            *out = self.0;
            true
        }
    }

    #[test]
    fn a_compile_takes_the_master_s_table_for_external_tracks_only() {
        use cantus_core::tuning::{TrackTuning, Tuning, TuningSystem};
        let mut p = cantus_core::Project::new("t");
        for name in ["ext", "plain"] {
            let t = p.make_track(name);
            p.tracks.push(t);
        }
        let (a, b) = (p.tracks[0].id, p.tracks[1].id);
        p.tuning = Tuning {
            system: TuningSystem::External,
            ..Tuning::default()
        };
        p.track_mut(b).unwrap().tuning = TrackTuning::Equal;
        let mut snap = cantus_core::Snapshot::compile(&p, 1);
        let mut want = TuningTable::EQUAL;
        want.0[60] = 0.25;
        fill_external(&mut snap, &Fixed(want));
        let of = |id| snap.tracks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(*of(a).tuning.table, want);
        assert!(of(a).tuning.external);
        assert!(of(b).tuning.table.is_equal());
        // No master: the compiled table stays.
        let mut snap2 = cantus_core::Snapshot::compile(&p, 1);
        fill_external(&mut snap2, &NoExternal);
        assert!(snap2.tracks[0].tuning.table.is_equal());
    }
}
