//! Tonus, the real-time auto-tuner, behind the engine's plug-in contract. Owned by Tonus.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::tonus`) into the DSP's settings
//! (`cantus_dsp::tonus`) — the editor draws from the same reading. The notes of the slot's
//! MIDI source (ADR 0022) arrive through [`FxUnit::note`], to the sample. The latency is
//! fixed (128 samples at 48 kHz) and reported to delay compensation.
//!
//! ## Telemetry
//!
//! The editor's pitch graph must not depend on the frame rate, so the DSP closes columns of
//! [`column_len`](cantus_dsp::tonus::Tonus::column_len) samples (5.3 ms at 48 kHz)
//! counted from the start of the stream; the telemetry carries the last
//! [`HISTORY`] and a running count, and the editor appends the ones it has not seen.
//!
//! | Index | What |
//! |---|---|
//! | 0 | columns completed since reset, modulo [`SEQ_WRAP`] |
//! | 1 | sample rate, Hz |
//! | 2 | latency, samples |
//! | 3 | samples per column |
//! | 4 | the sung pitch now, MIDI (0: unvoiced) |
//! | 5 | the target note, MIDI (−1: none) |
//! | 6 | the shift now, semitones |
//! | 7 | flags: 1 voiced, 2 on a bypass note, 4 shifting |
//! | 8 | the pitch classes held on MIDI (bit 0 = C) |
//! | 9, 10 | the pitch classes that are targets, and those on bypass |
//! | 11 | the input level, dBFS |
//! | 12 + 2·k … | column slot `k` (`k` = column index mod [`HISTORY`]): sung and output pitch, MIDI (0: unvoiced) |

use super::{FxNote, FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::tonus::NOTE_KEYS;
use cantus_core::EffectSpec;
use cantus_dsp::tonus::{
    Column, NoteEvent, NoteState, Scale, Source, Tonus, TonusSettings, HISTORY,
};

pub const T_SEQ: usize = 0;
pub const T_SAMPLE_RATE: usize = 1;
pub const T_LATENCY: usize = 2;
pub const T_COLUMN_LEN: usize = 3;
pub const T_PITCH: usize = 4;
pub const T_TARGET: usize = 5;
pub const T_SHIFT: usize = 6;
pub const T_FLAGS: usize = 7;
pub const T_HELD: usize = 8;
pub const T_ON: usize = 9;
pub const T_BYPASS: usize = 10;
pub const T_LEVEL: usize = 11;
pub const T_RING: usize = 12;
pub const T_COLUMN: usize = 2;
pub const FLAG_VOICED: u32 = 1;
pub const FLAG_BYPASS: u32 = 2;
pub const FLAG_SHIFTING: u32 = 4;
/// The count wraps here: below 2²⁴ (an f32 holds every integer up to it) and a multiple
/// of [`HISTORY`], so a column's slot survives the wrap.
pub const SEQ_WRAP: u64 = (1 << 24) / HISTORY as u64 * HISTORY as u64;

const _: () = assert!(T_RING + HISTORY * T_COLUMN <= TELEMETRY_LEN);

pub use cantus_dsp::tonus::HISTORY as COLUMNS;

/// Tonus's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> TonusSettings {
    let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
    let mut notes = [NoteState::Scale; 12];
    for (n, k) in notes.iter_mut().zip(NOTE_KEYS) {
        *n = NoteState::from_index(idx(k));
    }
    TonusSettings {
        source: Source::from_index(idx("src")),
        key: idx("key").min(11) as u8,
        scale: Scale::from_index(idx("scale")),
        notes,
        speed_ms: spec.get("speed"),
        amount: spec.get("amount"),
        humanize: spec.get("human"),
        flex_cents: spec.get("flex"),
        vibrato: spec.get("vib"),
        vib_depth: spec.get("vdepth"),
        vib_rate: spec.get("vrate"),
        formants: spec.get("formant") >= 0.5,
        throat: spec.get("throat"),
        transpose: spec.get("trans").round(),
        a4: spec.get("a4"),
    }
}

/// The telemetry of a running Tonus.
pub fn encode(d: &Tonus) -> Telemetry {
    let mut t = [0.0; TELEMETRY_LEN];
    let seq = d.seq();
    t[T_SEQ] = (seq % SEQ_WRAP) as f32;
    t[T_SAMPLE_RATE] = d.sample_rate();
    t[T_LATENCY] = d.latency() as f32;
    t[T_COLUMN_LEN] = d.column_len() as f32;
    t[T_PITCH] = d.detected().unwrap_or(0.0);
    t[T_TARGET] = d.target().map(|n| n as f32).unwrap_or(-1.0);
    t[T_SHIFT] = d.shift();
    let mut flags = 0;
    if d.detected().is_some() {
        flags |= FLAG_VOICED;
    }
    if d.on_bypass() {
        flags |= FLAG_BYPASS;
    }
    if d.shift() != 0.0 {
        flags |= FLAG_SHIFTING;
    }
    t[T_FLAGS] = flags as f32;
    t[T_HELD] = d.held_classes() as f32;
    let (on, by) = d.target_classes();
    t[T_ON] = on as f32;
    t[T_BYPASS] = by as f32;
    t[T_LEVEL] = d.level_db();
    for index in seq.saturating_sub(HISTORY as u64)..seq {
        if let Some(c) = d.column_at(index) {
            let at = T_RING + T_COLUMN * (index % HISTORY as u64) as usize;
            t[at] = c.input;
            t[at + 1] = c.output;
        }
    }
    t
}

/// The state part of a telemetry block, read back.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub seq: u64,
    pub sample_rate: f32,
    pub latency: usize,
    pub column_len: usize,
    /// The sung pitch, MIDI, when voiced.
    pub pitch: Option<f32>,
    /// The target note, MIDI.
    pub target: Option<i32>,
    pub shift: f32,
    pub on_bypass: bool,
    pub held: u16,
    pub on: u16,
    pub bypass: u16,
    pub level_db: f32,
}

pub fn header(t: &Telemetry) -> Header {
    let flags = t[T_FLAGS].max(0.0) as u32;
    Header {
        seq: t[T_SEQ].max(0.0) as u64,
        sample_rate: t[T_SAMPLE_RATE],
        latency: t[T_LATENCY].max(0.0) as usize,
        column_len: t[T_COLUMN_LEN].max(0.0) as usize,
        pitch: (flags & FLAG_VOICED != 0).then_some(t[T_PITCH]),
        target: (t[T_TARGET] >= 0.0).then_some(t[T_TARGET] as i32),
        shift: t[T_SHIFT],
        on_bypass: flags & FLAG_BYPASS != 0,
        held: t[T_HELD].max(0.0) as u16,
        on: t[T_ON].max(0.0) as u16,
        bypass: t[T_BYPASS].max(0.0) as u16,
        level_db: t[T_LEVEL],
    }
}

/// Column `index` from a telemetry block whose count is `seq`, if the block still holds it.
pub fn column(t: &Telemetry, seq: u64, index: u64) -> Option<Column> {
    if index >= seq || seq - index > HISTORY as u64 {
        return None;
    }
    let at = T_RING + T_COLUMN * (index % HISTORY as u64) as usize;
    Some(Column {
        input: t[at],
        output: t[at + 1],
    })
}

#[derive(Debug)]
pub struct Unit {
    dsp: Tonus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Tonus::new(sample_rate),
        }
    }
}

impl FxUnit for Unit {
    fn configure(&mut self, spec: &EffectSpec) {
        let s = settings(spec);
        if s != *self.dsp.settings() {
            self.dsp.set(&s);
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        encode(&self.dsp)
    }

    fn note(&mut self, offset: usize, note: FxNote) {
        self.dsp.note(
            offset,
            match note {
                FxNote::On { pitch, .. } => NoteEvent::On(pitch),
                FxNote::Off { pitch } => NoteEvent::Off(pitch),
                FxNote::AllOff => NoteEvent::AllOff,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    /// The defaults of the table read as the DSP's defaults.
    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::AutoTune);
        assert_eq!(settings(&spec), TonusSettings::default());
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::AutoTune);
        spec.set("src", 2.0);
        spec.set("key", 9.0);
        spec.set("scale", 12.0);
        spec.set("formant", 0.0);
        spec.set("n4", 3.0);
        spec.set("n11", 2.0);
        spec.set("trans", -7.0);
        let s = settings(&spec);
        assert_eq!(s.source, Source::Melody);
        assert_eq!(s.key, 9);
        assert_eq!(s.scale, Scale::Blues);
        assert!(!s.formants);
        assert_eq!(s.notes[4], NoteState::Bypass);
        assert_eq!(s.notes[11], NoteState::Remove);
        assert_eq!(s.transpose, -7.0);
    }

    #[test]
    fn telemetry_round_trips() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::AutoTune));
        let x = cantus_dsp::pitch::testkit::voice(
            48_000.0,
            0.3,
            cantus_dsp::pitch::testkit::Vowel::A,
            0.4,
            |_| 215.0,
        );
        let (mut l, mut r) = (x.clone(), x);
        u.note(
            0,
            FxNote::On {
                pitch: 60,
                velocity: 100,
            },
        );
        u.process(&mut l, &mut r, None);
        let t = u.telemetry();
        let h = header(&t);
        assert_eq!(h.seq, u.dsp.seq());
        assert_eq!(h.latency, 128);
        assert_eq!(h.held, 1);
        assert!(h.pitch.is_some());
        assert_eq!(h.target, Some(57));
        for index in h.seq.saturating_sub(30)..h.seq + 1 {
            assert_eq!(column(&t, h.seq, index), u.dsp.column_at(index));
        }
    }
}
