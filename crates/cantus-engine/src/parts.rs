//! The parts a track slot holds only while its track uses them (ADR 0107).
//!
//! Every one of the engine's 256 track slots once held a Pulsus drum machine (≈ 120 KB)
//! and a note-effect chain (≈ 55 KB) whether its track was a drum track, had note effects,
//! or existed at all — 45 MB on an empty song. Now both are built off the audio thread,
//! here, and sent in (`EngineMessage::AddPart`) before the snapshot that uses more of them
//! than the engine holds, as the native instruments are. The processor keeps every part it
//! is given — in a slot, or spare — and moves parts between slots as tracks start and stop
//! using them (`processor_parts.rs`): it never allocates and never frees one.

use cantus_core::{InstrumentSpec, Snapshot};
use cantus_dsp::Pulsus;

/// A part on its way to a slot, or back.
#[derive(Debug)]
pub enum SlotPart {
    /// A Pulsus drum machine, for a track hosting Pulsus.
    Kit(Box<Pulsus>),
    /// A note-effect chain (ADR 0069), for a track with note effects.
    NoteFx(Box<crate::notefx::Chain>),
}

/// How many of each part a snapshot uses at once.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Needed {
    /// Tracks hosting Pulsus (taps excluded: they play their source's bus).
    pub kits: usize,
    /// Tracks with note effects (switched on or not: the chain is configured either way).
    pub note_fx: usize,
}

/// What `snapshot` uses, within the engine's track limit.
pub fn needed(snapshot: &Snapshot) -> Needed {
    let tracks = snapshot.tracks.iter().take(crate::messages::MAX_TRACKS);
    let mut n = Needed::default();
    for t in tracks {
        if is_drum(t) {
            n.kits += 1;
        }
        if has_note_fx(t) {
            n.note_fx += 1;
        }
    }
    n
}

/// A track that plays a kit of its own.
pub fn is_drum(t: &cantus_core::TrackSnapshot) -> bool {
    t.tap.is_none() && matches!(t.instrument, InstrumentSpec::Pulsus(_))
}

/// A track whose notes go through note effects (or may, once one is switched on).
pub fn has_note_fx(t: &cantus_core::TrackSnapshot) -> bool {
    !t.note_fx.is_empty()
}

/// A kit for one drum track. Allocates (≈ 120 KB): never on the audio thread.
pub fn kit(sample_rate: f32) -> SlotPart {
    SlotPart::Kit(Box::new(Pulsus::new(sample_rate)))
}

/// A note-effect chain for one track. Allocates: never on the audio thread.
pub fn note_fx() -> SlotPart {
    SlotPart::NoteFx(Box::default())
}

/// The parts to send before `snapshot` to an engine that holds `held`: as many of each as
/// it lacks. What the backend sends (`EngineMessage::AddPart`).
pub fn parts_for(snapshot: &Snapshot, held: Needed, sample_rate: f32) -> Vec<SlotPart> {
    let want = needed(snapshot);
    let kits = (held.kits..want.kits).map(|_| kit(sample_rate));
    let chains = (held.note_fx..want.note_fx).map(|_| note_fx());
    kits.chain(chains).collect()
}

/// Track slots a processor starts with provisioned (ADR 0107): their pass buffers made, the
/// rest bare until a song needs them. Enough for a small song driven by hand (a test, the
/// bench); the application sends what a snapshot needs beyond them ([`SlotCells`]).
pub const START_TRACKS: usize = 16;
/// Strip slots a processor starts with provisioned: a new project's sixteen and as many more.
pub const START_STRIPS: usize = 32;

/// Track and strip slots' pass buffers for the engine to provision (ADR 0107): built off the
/// audio thread, sent before a snapshot with more tracks or strips than the engine has
/// slots provisioned for (`EngineMessage::Cells`), swapped into the next bare slots, and
/// handed back holding the bare ones (`EngineEvent::GarbageCells`).
pub struct SlotCells {
    pub(crate) tracks: Vec<crate::processor::exec::TrackParts>,
    pub(crate) strips: Vec<crate::processor::exec::StripParts>,
}

impl SlotCells {
    /// `tracks` track slots' and `strips` strip slots' memory. Allocates (≈ 30 KB a track,
    /// 32 KB a strip): never on the audio thread.
    pub fn new(sample_rate: f32, tracks: usize, strips: usize) -> Self {
        use crate::processor::exec::{StripParts, TrackParts};
        SlotCells {
            tracks: (0..tracks)
                .map(|_| {
                    let mut p = TrackParts::make(sample_rate, true);
                    p.prefault();
                    p
                })
                .collect(),
            strips: (0..strips)
                .map(|_| {
                    let mut p = StripParts::make(true);
                    p.prefault();
                    p
                })
                .collect(),
        }
    }

    /// The slots it provisions: `(tracks, strips)`.
    pub fn len(&self) -> (usize, usize) {
        (self.tracks.len(), self.strips.len())
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() && self.strips.is_empty()
    }
}

impl std::fmt::Debug for SlotCells {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotCells")
            .field("tracks", &self.tracks.len())
            .field("strips", &self.strips.len())
            .finish()
    }
}

/// The track and strip slots `snapshot` needs provisioned: one per track and per strip,
/// within the engine's limits. A slot whose track went is reused before a fresh one, so this
/// is enough whatever the last snapshot held.
pub fn cells_needed(snapshot: &Snapshot) -> (usize, usize) {
    (
        snapshot.tracks.len().min(crate::messages::MAX_TRACKS),
        snapshot.strips.len().min(crate::messages::MAX_STRIPS),
    )
}
