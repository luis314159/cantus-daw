//! The engine's [`ExternalTuning`] on MTS-ESP (ADR 0156): `cantus-rt::mts`'s safe API (the
//! only place its `unsafe` lives) seen as the tuning's seam — the master's table read as
//! semitones from 12-TET at A440, and Cantus's table published in Hz when Cantus is the
//! master. Built on the main thread at start-up (it loads the library); every method of the
//! trait is then safe on the audio thread: no lock, no allocation (`tests/mts_external.rs`).
//! r42:tuning-face

use crate::tuning::ExternalTuning;
use cantus_core::tuning::TuningTable;
use cantus_rt::mts::{LoadError, MasterState, Mts, Refusal, KEYS};
use std::sync::Arc;

/// MTS-ESP when the library is installed, or why it is not.
#[derive(Debug, Clone)]
pub struct MtsLink {
    mts: Result<Arc<Mts>, LoadError>,
}

impl MtsLink {
    /// `LIBMTS` from its standard place, registered as a client (main thread).
    pub fn standard() -> MtsLink {
        MtsLink::from(Mts::open_standard())
    }

    /// The library at `path` (a test's fake), registered as a client.
    pub fn open(path: &std::path::Path) -> MtsLink {
        MtsLink::from(Mts::open(path))
    }

    /// No library: every call nothing, the interface saying why.
    pub fn absent() -> MtsLink {
        MtsLink {
            mts: Err(LoadError::NoPlace),
        }
    }

    fn from(r: Result<Mts, LoadError>) -> MtsLink {
        let mts = r.map(Arc::new);
        if let Ok(m) = &mts {
            m.register_client();
        }
        MtsLink { mts }
    }

    /// The loaded library, if there is one.
    pub fn library(&self) -> Option<&Mts> {
        self.mts.as_ref().ok().map(|m| &**m)
    }

    /// Why there is none (for the window's greyed rows).
    pub fn absence(&self) -> Option<&LoadError> {
        self.mts.as_ref().err()
    }

    /// Whether Cantus may be the master now.
    pub fn master_state(&self) -> Option<MasterState> {
        self.library().map(Mts::master_state)
    }

    /// Become the master and publish `table` at once (main thread).
    pub fn become_master(
        &self,
        reinitialize: bool,
        table: &TuningTable,
        name: &str,
    ) -> Result<(), Refusal> {
        let m = self.library().ok_or(Refusal::Unsupported)?;
        m.become_master(reinitialize)?;
        self.publish_all(table, name);
        Ok(())
    }

    /// Give the master's role back (main thread).
    pub fn resign_master(&self) {
        if let Some(m) = self.library() {
            m.resign_master();
        }
    }

    pub fn is_master(&self) -> bool {
        self.library().is_some_and(Mts::is_master)
    }

    /// The table, its unmapped keys and its name, when master (main thread).
    pub fn publish_all(&self, table: &TuningTable, name: &str) {
        let Some(m) = self.library().filter(|m| m.is_master()) else {
            return;
        };
        m.publish(&hz_of(table));
        let unmapped: [bool; KEYS] = std::array::from_fn(|k| table.0[k].is_nan());
        m.filter(&unmapped);
        m.set_scale_name(name);
    }

    /// The master's scale name, when following one (main thread).
    pub fn master_name(&self) -> Option<String> {
        self.library().and_then(Mts::scale_name)
    }

    /// Whether another program is the master now.
    pub fn follows(&self) -> bool {
        self.library()
            .is_some_and(|m| !m.is_master() && m.has_master())
    }
}

/// `table` in Hz per key, MTS-ESP's unit; an unmapped key gets the next lower mapped key's
/// frequency, or the next higher one with none below (ODDSound's header: "it is suggested
/// to provide a frequency for all MIDI notes, even unmapped ones").
pub fn hz_of(table: &TuningTable) -> [f64; KEYS] {
    let mut hz = [0.0f64; KEYS];
    for (k, v) in hz.iter_mut().enumerate() {
        let off = f64::from(table.0[k]);
        *v = if off.is_nan() {
            f64::NAN
        } else {
            440.0 * cantus_dsp::math::exp2((k as f64 - 69.0 + off) / 12.0)
        };
    }
    for k in 0..KEYS {
        if hz[k].is_nan() {
            let below = (0..k).rev().find(|&j| !hz[j].is_nan());
            let above = (k + 1..KEYS).find(|&j| !hz[j].is_nan());
            hz[k] = below.or(above).map_or(440.0, |j| hz[j]);
        }
    }
    hz
}

/// The master's table (Hz per key) as semitones from 12-TET at A440.
pub fn table_of(hz: &[f64; KEYS], out: &mut TuningTable) {
    for (k, v) in out.0.iter_mut().enumerate() {
        let f = hz[k];
        *v = if f.is_finite() && f > 0.0 {
            (12.0 * cantus_dsp::math::log2(f / 440.0) - (k as f64 - 69.0)) as f32
        } else {
            0.0
        };
    }
}

impl ExternalTuning for MtsLink {
    fn read(&self, out: &mut TuningTable) -> bool {
        let Some(m) = self.library() else {
            return false;
        };
        let mut hz = [0.0f64; KEYS];
        if !m.read(&mut hz) {
            return false;
        }
        table_of(&hz, out);
        for (k, v) in out.0.iter_mut().enumerate() {
            if m.should_filter(k as u8) {
                *v = f32::NAN;
            }
        }
        true
    }

    fn publish(&self, table: &TuningTable) {
        if let Some(m) = self.library().filter(|m| m.is_master()) {
            m.publish(&hz_of(table));
        }
    }
}
