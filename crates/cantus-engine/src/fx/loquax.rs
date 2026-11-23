//! Loquax, the vocoder (ADR 0136), behind the engine's plug-in contract.
//! Owned by the wave-A builder.
//!
//! **Routing.** The strip's own sound is the voice (the modulator). The carrier is the
//! built-in synthesiser, played by the notes of the track the slot listens to
//! (`EffectSlot::midi_from`, ADR 0022 — the processor hands each note to [`Unit::note`] at
//! its sample), and/or **the slot's sidechain**: another strip routed as the slot's key
//! (`EffectSlot::sidechain`), which the strip runner reads and passes to `process` as `key`
//! exactly as it does for a compressor's detector. Nothing new was built for it: the key
//! *is* the carrier (`src` sidechain or both), or, with `swap`, the voice while the strip
//! becomes the carrier. A strip with a note listener never rests (`graph::strip::may_rest`),
//! so a held key always reaches the unit.
//!
//! Notes wait in a fixed list (no allocation) until the next `process`, which hands them to
//! the DSP with their frames; past [`PENDING`] in one block the oldest are acted on at the
//! block's start. Latency 0. The null: the mix at 0 — the DSP passes every sample untouched.

use super::{FxNote, FxUnit, Telemetry};
use cantus_core::fx::loquax::{BAND_KEYS, MAX_BANDS};
use cantus_core::EffectSpec;
use cantus_dsp::loquax::carrier::SynthParams;
use cantus_dsp::loquax::{BandView, Loquax, LoquaxSettings, NoteEvent, NoteKind, Source, ORDERS};

/// Notes kept for one block.
pub const PENDING: usize = 64;

/// The telemetry after the DSP's own (`cantus_dsp::loquax::t`, the first 32 numbers): the
/// bands' output levels, dBFS, in [`SHOWN`] groups from the lowest band (each group the
/// loudest of the bands it holds — one band each up to 32 bands, two from 33 on).
pub const T_BANDS: usize = 32;
pub const SHOWN: usize = 32;

/// Loquax's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> LoquaxSettings {
    let on = |k: &str| spec.get(k) >= 0.5;
    let mut band_db = [0.0f32; MAX_BANDS];
    for (d, k) in band_db.iter_mut().zip(BAND_KEYS) {
        *d = spec.get(k);
    }
    LoquaxSettings {
        mix: spec.get("mix"),
        out_db: spec.get("out"),
        source: match spec.get("src").round() as i32 {
            0 => Source::Sidechain,
            2 => Source::Both,
            _ => Source::Synth,
        },
        swap: on("swap"),
        bands: spec.get("bands").round().clamp(8.0, 64.0) as usize,
        lo_hz: spec.get("lo"),
        hi_hz: spec.get("hi"),
        width: spec.get("width"),
        order: ORDERS[(spec.get("order").round().clamp(0.0, 2.0)) as usize],
        attack_ms: spec.get("att"),
        release_ms: spec.get("rel"),
        formant: spec.get("formant"),
        band_shift: spec.get("bshift"),
        tilt: spec.get("tilt"),
        flatten: spec.get("flat"),
        sibilance: spec.get("sib"),
        sib_hz: spec.get("sibhz"),
        sib_gated: on("sibgate"),
        unvoiced: spec.get("unv"),
        hold: on("hold"),
        spread: spec.get("spread"),
        carrier_pass: spec.get("cpass"),
        band_db,
        synth: SynthParams {
            saw: spec.get("saw"),
            pulse: spec.get("pulse"),
            width: spec.get("pw"),
            noise: spec.get("noise"),
            unison: spec.get("uni").round().clamp(1.0, 7.0) as usize,
            detune: spec.get("detune"),
            spread: spec.get("uspread"),
            octave: spec.get("oct").round().clamp(-3.0, 3.0) as i32,
            attack_ms: spec.get("satt"),
            release_ms: spec.get("srel"),
            level_db: spec.get("slevel"),
        },
        robot: on("robot"),
        robot_note: spec.get("rnote").round(),
        robot_gate_db: spec.get("rgate"),
    }
}

#[derive(Clone)]
pub struct Unit {
    dsp: Box<Loquax>,
    pending: [NoteEvent; PENDING],
    np: usize,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "loquax::Unit({} notes waiting)", self.np)
    }
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let mut dsp = Box::new(Loquax::new(sample_rate));
        dsp.set(&LoquaxSettings::default());
        dsp.reset();
        Unit {
            dsp,
            pending: [NoteEvent {
                frame: 0,
                kind: NoteKind::AllOff,
            }; PENDING],
            np: 0,
        }
    }

    /// The DSP, for the editor's drawing and the tests.
    pub fn dsp(&self) -> &Loquax {
        &self.dsp
    }
}

impl FxUnit for Unit {
    /// r42:tuning — the strip's tuning (ADR 0155), where this unit's notes sit.
    fn set_tuning(&mut self, table: &cantus_core::tuning::TuningTable) {
        self.dsp.set_tuning(&table.0);
    }

    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len()) as u32;
        // The notes in time order (the host gives them so): an insertion sort, stable and
        // allocation-free (the standard stable sort takes a buffer).
        let notes = &mut self.pending[..self.np];
        for e in notes.iter_mut() {
            e.frame = e.frame.min(n.saturating_sub(1));
        }
        for i in 1..notes.len() {
            let mut j = i;
            while j > 0 && notes[j - 1].frame > notes[j].frame {
                notes.swap(j - 1, j);
                j -= 1;
            }
        }
        self.dsp.process(left, right, key, notes);
        self.np = 0;
    }

    fn note(&mut self, offset: usize, note: FxNote) {
        let kind = match note {
            FxNote::On { pitch, velocity } => NoteKind::On {
                key: pitch,
                velocity: f32::from(velocity) / 127.0,
            },
            FxNote::Off { pitch } => NoteKind::Off { key: pitch },
            FxNote::AllOff => NoteKind::AllOff,
        };
        if self.np == PENDING {
            // No room: the oldest moves to the block's start (it still acts, in order).
            self.pending.copy_within(1.., 0);
            self.np -= 1;
        }
        self.pending[self.np] = NoteEvent {
            frame: u32::try_from(offset).unwrap_or(u32::MAX),
            kind,
        };
        self.np += 1;
    }

    /// fx/final — at a start, a seek or a loop's wrap: every glide at its target.
    fn snap(&mut self) {
        self.dsp.snap();
    }

    fn reset(&mut self) {
        self.np = 0;
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.np == 0 && self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        if self.np > 0 {
            return Some("a note waits");
        }
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let mut out = [0.0f32; 64];
        self.dsp.telemetry(&mut out);
        let mut v = BandView::default();
        self.dsp.bands(&mut v);
        let per = v.bands.div_ceil(SHOWN).max(1);
        for g in 0..SHOWN {
            let from = g * per;
            let to = (from + per).min(v.bands);
            out[T_BANDS + g] = if from < to {
                v.output_db[from..to].iter().cloned().fold(-200.0, f32::max)
            } else {
                -200.0
            };
        }
        out
    }
}
