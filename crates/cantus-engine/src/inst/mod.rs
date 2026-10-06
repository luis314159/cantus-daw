//! The instrument contract on the engine side (ADR 0023): one [`InstrumentUnit`] per native
//! instrument, in a module of its own, so an instrument is owned by its files and the
//! processor never learns its name. The twin of `crate::fx` for effects.
//!
//! A unit is built off the audio thread ([`make`]), delivered in an
//! [`EngineMessage::AddInstrument`](crate::EngineMessage::AddInstrument) *before* the
//! snapshot that needs it, and handed back as garbage when the track stops hosting it.
//! Every method is called on the audio thread and must not allocate, lock, block or panic
//! (ADR 0005).
//!
//! **Timing.** The processor splits each block at event times, so a note arrives *between*
//! calls to [`InstrumentUnit::process`]: a note-on is sample-accurate without an offset,
//! and a unit never sees a block longer than `MAX_BLOCK_FRAMES`.
//!
//! **Notes** carry an id (the clip note's id, or a live-input tag) so a note-off, a choke
//! or a per-note expression addresses one voice even when two notes share a key — CLAP's
//! note id and VST3's note id map onto it one to one.

use cantus_core::{
    ExternalPlugin, InstrumentKind, InstrumentSpec, NativeSpec, SampleData, TrackId,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub mod analog;
pub mod bass;
pub mod epiano;
pub mod fm;
pub mod organ;
pub mod sampler;
pub mod wavetable;

/// Numbers an instrument reports to its editor each block: voices, envelope stages, the
/// modulation a knob is receiving — whatever its display animates. Their meaning is the
/// instrument's own; they travel in `Meters::inst` for the track whose editor is open.
pub const TELEMETRY_LEN: usize = crate::fx::TELEMETRY_LEN;
pub type Telemetry = [f32; TELEMETRY_LEN];

/// Decoded audio for the asset keys of a [`NativeSpec`] (`NativeSpec::assets`), as
/// `(key, audio)` in key order. `None` is a file that could not be read: its sound is
/// silence, never an error on the audio thread.
pub type Assets = [(String, Option<Arc<SampleData>>)];

/// Find one asset by key.
pub fn asset<'a>(assets: &'a Assets, key: &str) -> Option<&'a Arc<SampleData>> {
    assets
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, a)| a.as_ref())
}

/// A note starting.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoteOn {
    pub key: u8,
    /// 0 < velocity ≤ 1. A clip's 1…127 arrives as `v / 127`.
    pub velocity: f32,
    /// Stable for the note's life: note-offs, chokes and expressions name it.
    pub id: u64,
    /// MIDI channel 0…15 (MPE: one note per channel).
    pub channel: u8,
}

impl NoteOn {
    /// The MIDI velocity this came from, 1…127 (for engines written against MIDI).
    pub fn velocity_u8(&self) -> u8 {
        (self.velocity * 127.0).round().clamp(1.0, 127.0) as u8
    }
}

/// A per-note expression — the intersection of CLAP's and VST3's, which is also
/// `cantus_host::NoteExpression`, so a hosted instrument and a native one read the same
/// stream.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoteExpression {
    /// Semitones from the note's key (MPE pitch bend, CLAP tuning).
    Tuning,
    /// 0…1 (poly aftertouch, MPE pressure).
    Pressure,
    /// 0…1 (MPE slide / CC74, CLAP brightness).
    Brightness,
    /// −1…1.
    Pan,
    /// Linear gain 0…4.
    Volume,
    /// 0…1.
    Vibrato,
    /// 0…1.
    Expression,
}

/// A channel-wide controller.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Controller {
    /// Semitones (the instrument's own bend range is its business; this is already scaled
    /// to ±2 by the processor for plain MIDI).
    PitchBend,
    /// 0…1.
    ModWheel,
    /// 0 up, 1 down; values between are half-pedalling.
    Sustain,
    /// 0…1 channel pressure.
    Pressure,
    /// 0…1 (CC11).
    Expression,
    /// Any other CC, 0…1, by number.
    Cc(u8),
}

pub trait InstrumentUnit: Send + std::fmt::Debug {
    /// New parameter values and assets. Called on the audio thread with each snapshot:
    /// read the spec, set the DSP, never allocate. Changes should glide where a jump
    /// would click.
    fn configure(&mut self, spec: &NativeSpec, assets: &Assets);
    fn note_on(&mut self, note: NoteOn);
    /// Release the note with this id (or, for id-less input, the key).
    fn note_off(&mut self, key: u8, id: u64, velocity: f32);
    /// Stop the note at once, as a choke group does. Defaults to a release.
    fn choke(&mut self, key: u8, id: u64) {
        self.note_off(key, id, 0.0);
    }
    fn expression(&mut self, _id: u64, _expression: NoteExpression, _value: f32) {}
    fn controller(&mut self, _controller: Controller, _value: f32) {}
    /// Release every voice (transport stop, loop wrap). Tails may ring.
    fn all_notes_off(&mut self);
    /// Silence at once and forget every tail (seek, panic, instrument switched).
    fn reset(&mut self);
    /// Render one segment **added** into `left`/`right` (the caller zeroes them). Returns
    /// whether anything was written; `false` lets the processor skip an idle track, so it
    /// must only be `false` when the output is exactly zero.
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool;
    /// True when the unit is **at rest** (ADR 0031 §5.1). Exactly:
    ///
    /// 1. with no note, expression or controller arriving, `process` writes nothing
    ///    (returns `false`) and leaves it at rest;
    /// 2. **suspension is exact**: skipping those calls — telling the unit with
    ///    [`skip`](Self::skip) — changes no sample of what follows, whatever is played and
    ///    whatever parameter changes come next;
    /// 3. **it is as good as new**: it renders exactly what a freshly reset unit with the same
    ///    configuration renders (that unit skipped as far as this one has run) for anything
    ///    played, as long as the parameters stay as they are.
    ///
    /// Stricter than [`Self::process`] returning `false`: a unit can be silent with a
    /// filter still ringing below the flush threshold, a glide in flight, a held
    /// controller or a free-running LFO whose phase would differ after a skip — none of
    /// those is at rest. Cheap: the engine may ask once per block. Default `false`.
    /// Checked for every native kind by the twin tests (`tests/inst_contract.rs`).
    fn at_rest(&self) -> bool {
        false
    }
    /// The engine did not call `process` for `frames` samples because the unit was
    /// [at rest](Self::at_rest) and nothing arrived: advance whatever counts samples,
    /// exactly as processing would have. Cheap, no allocation. Called only while at rest,
    /// and on a fresh unit; it stays at rest. Default: nothing counts.
    fn skip(&mut self, frames: usize) {
        let _ = frames;
    }
    fn active_voices(&self) -> usize;
    /// Samples of delay between a note and its sound, for delay compensation.
    fn latency(&self) -> usize {
        0
    }
    /// What the editor draws, refreshed each block.
    fn telemetry(&self) -> Telemetry {
        [0.0; TELEMETRY_LEN]
    }

    // ---- Additions (round 20, ADR 0028), each with a default, so a native unit is
    // ---- unchanged. A hosted CLAP/VST3 instrument (`cantus_host`) uses all of them.

    /// The values of a hosted instrument's spec ([`InstrumentSpec::External`]), with each
    /// snapshot, in place of [`Self::configure`]. Audio thread, no allocation.
    fn configure_hosted(&mut self, _plugin: &ExternalPlugin) {}
    /// Where the song is, before each segment is processed (tempo, position, loop).
    fn set_position(&mut self, _position: &crate::fx::SongPosition) {}
    /// What the instrument's own editor did during the last `process` (a hosted plug-in's
    /// window): gestures and values for the host to record as commands.
    fn param_events(&self) -> &[crate::fx::FxParamEvent] {
        &[]
    }
    fn clear_param_events(&mut self) {}
    /// It will not be processed again: called on the audio thread before the unit is
    /// handed back (a hosted plug-in stops processing here, as its standard asks).
    fn stop(&mut self) {}
}

/// What a unit is, as the processor matches it against what a snapshot's track hosts: a
/// native kind, or a hosted plug-in (ADR 0028) named by [`hosted_key`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnitKind {
    Native(InstrumentKind),
    Hosted(u64),
}

impl UnitKind {
    /// The unit a track's instrument needs; `None` for Firmus and Pulsus, which every slot
    /// holds already.
    pub fn of(spec: &InstrumentSpec) -> Option<UnitKind> {
        match spec {
            InstrumentSpec::Native(n) => Some(UnitKind::Native(n.kind)),
            InstrumentSpec::External(p) => Some(UnitKind::Hosted(hosted_key(p))),
            InstrumentSpec::Firmus(_) | InstrumentSpec::Pulsus(_) => None,
        }
    }

    pub fn native(self) -> Option<InstrumentKind> {
        match self {
            UnitKind::Native(k) => Some(k),
            UnitKind::Hosted(_) => None,
        }
    }
}

/// A hosted plug-in's name as the engine matches it: FNV-1a over its format and id. Two
/// plug-ins share a key only if they share format and id, which is what makes them the
/// same plug-in. No allocation, so the audio thread computes it from a snapshot.
pub fn hosted_key(plugin: &ExternalPlugin) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let format = match plugin.format {
        cantus_core::PluginFormat::Clap => 1u8,
        cantus_core::PluginFormat::Vst3 => 2u8,
    };
    for b in std::iter::once(format).chain(plugin.id.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A unit on its way to the audio thread, labelled with the track and kind it was built
/// for, so the processor can match it against a snapshot without asking it anything.
#[derive(Debug)]
pub struct NativeUnit {
    pub track: TrackId,
    /// A native kind, or a hosted plug-in (ADR 0028).
    pub kind: UnitKind,
    /// Unique per unit ever built, so the backend can tell a unit handed back late from
    /// the one the engine holds now.
    pub serial: u64,
    pub unit: Box<dyn InstrumentUnit>,
}

static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

impl NativeUnit {
    /// Build a unit for `track`. Allocates: never on the audio thread.
    pub fn new(track: TrackId, kind: InstrumentKind, sample_rate: f32) -> Self {
        NativeUnit {
            track,
            kind: UnitKind::Native(kind),
            serial: next_serial(),
            unit: make(kind, sample_rate),
        }
    }

    /// A unit built elsewhere — a hosted plug-in's adapter (`cantus_host`, ADR 0028) — for
    /// the plug-in named by `key` ([`hosted_key`]).
    pub fn hosted(track: TrackId, key: u64, unit: Box<dyn InstrumentUnit>) -> Self {
        NativeUnit {
            track,
            kind: UnitKind::Hosted(key),
            serial: next_serial(),
            unit,
        }
    }
}

/// A serial no unit has had.
pub fn next_serial() -> u64 {
    NEXT_SERIAL.fetch_add(1, Ordering::Relaxed)
}

/// Build the unit for a kind. Main or worker thread: this allocates.
pub fn make(kind: InstrumentKind, sample_rate: f32) -> Box<dyn InstrumentUnit> {
    match kind {
        InstrumentKind::Wavetable => Box::new(wavetable::Unit::new(sample_rate)),
        InstrumentKind::Analog => Box::new(analog::Unit::new(sample_rate)),
        InstrumentKind::Fm => Box::new(fm::Unit::new(sample_rate)),
        InstrumentKind::Sampler => Box::new(sampler::Unit::new(sample_rate)),
        InstrumentKind::Bass => Box::new(bass::Unit::new(sample_rate)),
        InstrumentKind::ElectricPiano => Box::new(epiano::Unit::new(sample_rate)),
        InstrumentKind::Organ => Box::new(organ::Unit::new(sample_rate)),
    }
}

/// The units a snapshot needs that `have` does not account for: one per track hosting a
/// native instrument whose `(track, kind)` is not in `have`. What the backend and the
/// offline renderer both send before a snapshot. Boxed, because each travels in its own
/// message and the message stays small.
#[allow(clippy::vec_box)]
pub fn units_for(
    snapshot: &cantus_core::Snapshot,
    sample_rate: f32,
    have: impl Fn(TrackId, InstrumentKind) -> bool,
) -> Vec<Box<NativeUnit>> {
    prepare_assets(snapshot);
    snapshot
        .tracks
        .iter()
        .filter_map(|t| {
            let kind = t.instrument.native()?.kind;
            (!have(t.id, kind)).then(|| Box::new(NativeUnit::new(t.id, kind, sample_rate)))
        })
        .collect()
}

/// Let a kind derive what it needs from its decoded assets (a wavetable's mip levels)
/// here, off the audio thread, before the snapshot that carries them is sent — so
/// `configure` only looks the result up. [`units_for`] calls it, which is the one place
/// both the backend and the offline renderer pass through with every snapshot.
pub fn prepare_assets(snapshot: &cantus_core::Snapshot) {
    for t in &snapshot.tracks {
        if let Some(n) = t.instrument.native() {
            if n.kind == InstrumentKind::Wavetable {
                wavetable::prepare(n, &t.assets);
            }
        }
    }
}
