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
