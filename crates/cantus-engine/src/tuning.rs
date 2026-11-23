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
    /// r42 fix-engine — whether Cantus is the MTS-ESP master now (its clients then hear the
    /// project's table). Lock-free: the audio thread asks it each block.
    fn leads(&self) -> bool {
        false
    }
}

/// r42 fix-engine — the one "no MTS-ESP" every engine and processor starts with (review
/// correctness 9): a clone of a value that lives for the whole process, so the audio thread
/// that replaces it with the app's link frees nothing.
pub fn no_external() -> std::sync::Arc<dyn ExternalTuning> {
    static NONE: std::sync::OnceLock<std::sync::Arc<dyn ExternalTuning>> =
        std::sync::OnceLock::new();
    NONE.get_or_init(|| std::sync::Arc::new(NoExternal)).clone()
}

/// r42 fix-engine — what MTS-ESP gives its clients now (ADR 0156): the project's `own`
/// table while Cantus is the master, the outside master's table read into `scratch`, or
/// `None` with no master. Allocation-free (the audio thread's refresh calls it).
pub fn heard<'a>(
    ext: &dyn ExternalTuning,
    own: &'a TuningTable,
    scratch: &'a mut TuningTable,
) -> Option<&'a TuningTable> {
    if ext.leads() {
        Some(own)
    } else if ext.read(scratch) {
        Some(scratch)
    } else {
        None
    }
}

/// r42 fix-engine — the per-note tuning a hosted plug-in that reads MTS-ESP itself still
/// needs: `table` less what MTS-ESP gives it (`heard`), key by key — exactly `0.0` where the
/// two agree (a track that follows the master, or the project Cantus publishes), so no
/// note expression is sent. An unmapped key stays `NAN`; a key MTS-ESP leaves unmapped is
/// sent whole (the plug-in filters it itself).
pub fn beyond(table: &TuningTable, heard: &TuningTable) -> TuningTable {
    let mut out = *table;
    for (o, h) in out.0.iter_mut().zip(&heard.0) {
        if h.is_finite() && o.is_finite() {
            *o = if o.to_bits() == h.to_bits() {
                0.0
            } else {
                *o - *h
            };
        }
    }
    out
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
///
/// r42 fix-engine: also what MTS-ESP gives its clients at this compile
/// (`Snapshot::mts_heard`, ADR 0156) — the outside master's table, or the project's while
/// Cantus is the master — which an offline render's hosted plug-ins hear beneath their
/// track's tuning.
pub fn fill_external(snapshot: &mut cantus_core::Snapshot, ext: &dyn ExternalTuning) {
    let mut table = TuningTable::EQUAL;
    let leads = ext.leads();
    let read = !leads && ext.read(&mut table);
    let tracks = snapshot.tracks.iter_mut().map(|t| &mut t.tuning);
    for t in tracks.chain([&mut snapshot.tuning]).filter(|t| t.external) {
        if read {
            t.table = std::sync::Arc::new(table);
        }
    }
    snapshot.mts_heard = if leads {
        Some(snapshot.tuning.table.clone())
    } else if read {
        Some(std::sync::Arc::new(table))
    } else {
        None
    };
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

    /// r42 fix-engine (review correctness 1): what a hosted MTS-ESP client is still sent —
    /// nothing where its track's table is MTS-ESP's, the difference elsewhere; an unmapped
    /// key stays unmapped.
    #[test]
    fn a_client_is_sent_only_what_mts_esp_does_not_give_it() {
        let mut heard = TuningTable::EQUAL;
        heard.0[60] = 0.25;
        heard.0[62] = f32::NAN;
        let same = beyond(&heard, &heard);
        assert!(same.0.iter().enumerate().all(|(k, v)| if k == 62 {
            v.is_nan()
        } else {
            v.to_bits() == 0
        }));
        let mut own = heard;
        own.0[60] = 0.5;
        own.0[62] = 0.1;
        let rest = beyond(&own, &heard);
        assert_eq!(rest.0[60], 0.25);
        assert_eq!(rest.0[62], 0.1, "MTS-ESP leaves it unmapped: sent whole");
        assert_eq!(rest.0[61].to_bits(), 0);
    }

    /// r42 fix-engine: a compile records what MTS-ESP gives its clients — the outside
    /// master's table, the project's while Cantus leads, nothing without a master.
    #[test]
    fn a_compile_records_what_mts_esp_gives_its_clients() {
        #[derive(Debug)]
        struct Leading;
        impl ExternalTuning for Leading {
            fn read(&self, _out: &mut TuningTable) -> bool {
                false
            }
            fn leads(&self) -> bool {
                true
            }
        }
        let mut p = cantus_core::Project::new("t");
        p.tuning = cantus_core::tuning::Tuning {
            a4_hz: 415.0,
            ..Default::default()
        };
        let mut snap = cantus_core::Snapshot::compile(&p, 1);
        fill_external(&mut snap, &NoExternal);
        assert!(snap.mts_heard.is_none());
        let mut want = TuningTable::EQUAL;
        want.0[60] = 0.25;
        fill_external(&mut snap, &Fixed(want));
        assert_eq!(snap.mts_heard.as_deref(), Some(&want));
        fill_external(&mut snap, &Leading);
        assert_eq!(snap.mts_heard.as_deref(), Some(&*snap.tuning.table));
        assert!(!snap.tuning.table.is_equal(), "the project's A415");
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
