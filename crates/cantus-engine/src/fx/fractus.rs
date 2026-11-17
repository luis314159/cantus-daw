//! Fractus, the glitch sequencer and grain cloud (ADR 0137), behind the
//! engine's plug-in contract. Owned by the wave-A builder.
//!
//! **Its memory is built off the audio thread.** The DSP holds 8.6 MB at 48 kHz (34 MB at
//! 192 kHz): eight seconds of history, a repeat's capture, the cloud's buffer and its two
//! octave copies, the throw's lines. A unit is made without it — a wire, which at the
//! defaults is exactly what the DSP would play — and asks for it through the memory path
//! (ADR 0022, `MemoryNeed::Fractus`) as soon as its settings make it more than a wire (a
//! lane painted, the cloud up or frozen, notes playing the modules, the output moved). The
//! UI thread builds the whole DSP (`EffectMemory::Fractus`, `Fractus::new`) and the unit
//! takes the box in [`FxUnit::supply`] — a pointer moved, nothing allocated or freed on the
//! audio thread — with its settings, its pattern and the song's position given before its
//! first sample. A unit built for a project (`EffectUnit::new`, an offline render) gets it
//! at once, so a render is the same from its first sample; an empty Fractus costs nothing.
//! The DSP stays once it came (a pattern cleared and painted again does not ask twice);
//! it is freed with the unit, off the audio thread.
//!
//! [`settings`] reads the table (`cantus_core::fx::fractus`) into the DSP's settings;
//! [`pattern_into`] writes the painted blocks (`EffectSpec::fractus`) into the DSP's steps
//! in place. Notes arrive at their sample (`TAKES_NOTES`). Latency 0. A fork copies the
//! whole DSP (history, capture, cloud, grains, the sequence's place): a render resumed
//! from a checkpoint is the whole render's.

use super::modfx::{telemetry_of, transport_of};
use super::{FxNote, FxUnit, SongPosition, Telemetry};
use crate::effects::{EffectMemory, MemoryNeed};
use cantus_core::fx::fractus as model;
use cantus_core::EffectSpec;
use cantus_dsp::fractus::{
    note_beats, CloudSettings, CrushSettings, FilterKind, FilterSettings, Fractus, FractusSettings,
    GateSettings, GrainWindow, Note, Pattern, PitchSettings, RepeatSettings, ReverseSettings,
    Scale, ShuffleSettings, Step, TapeKind, TapeSettings, ThrowSettings, Trigger, STEP_BEATS,
};
use cantus_dsp::modfx_clock::Transport;

/// A note value whose index 0 means something of its own (`vslice`, `ttime`, `grate`):
/// 0 → `zero`, `i` → the DSP's note value `i − 1`.
fn shifted_note(index: f32, zero: f64) -> f64 {
    let i = index.round();
    if i >= 1.0 {
        note_beats(i - 1.0)
    } else {
        zero
    }
}

/// Fractus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> FractusSettings {
    let on = |k: &str| spec.get(k) >= 0.5;
    let step_i = spec
        .get("step")
        .round()
        .clamp(0.0, (STEP_BEATS.len() - 1) as f32) as usize;
    FractusSettings {
        step: STEP_BEATS[step_i],
        bars: spec.get("bars").round().clamp(1.0, 4.0) as u8,
        swing: spec.get("swing"),
        xfade_ms: spec.get("xfade"),
        trigger: Trigger::from_index(spec.get("trig")),
        quantise: on("quant"),
        key: spec.get("key").round().clamp(0.0, 127.0) as u8,
        seed: spec.get("seed").round().clamp(1.0, 9_999.0) as u32,
        mix: spec.get("mix"),
        out_db: spec.get("out"),
        repeat: RepeatSettings {
            size: note_beats(spec.get("rsize")),
            ramp: spec.get("rramp").round().clamp(0.0, 3.0) as u8,
            pitch_st: spec.get("rpitch"),
            decay_db: spec.get("rdecay"),
            duty: spec.get("rduty"),
        },
        reverse: ReverseSettings {
            slice: shifted_note(spec.get("vslice"), 0.0),
        },
        tape: TapeSettings {
            kind: TapeKind::from_index(spec.get("tkind")),
            time: shifted_note(spec.get("ttime"), 0.0),
            curve: spec.get("tcurve"),
        },
        shuffle: ShuffleSettings {
            slice: note_beats(spec.get("sslice")),
            range: spec.get("srange").round().clamp(1.0, 8.0) as u8,
            reverse: spec.get("srev"),
        },
        pitch: PitchSettings {
            semis: spec.get("psemi"),
            window_ms: spec.get("pwin"),
            clean: on("pstyle"),
        },
        crush: CrushSettings {
            bits: spec.get("kbits"),
            rate_hz: spec.get("krate"),
        },
        filter: FilterSettings {
            kind: FilterKind::from_index(spec.get("fkind")),
            from_hz: spec.get("ffrom"),
            to_hz: spec.get("fto"),
            res: spec.get("fres"),
            curve: spec.get("fcurve"),
        },
        gate: GateSettings {
            rate: shifted_note(spec.get("grate"), 0.0),
            duty: spec.get("gduty"),
            depth: spec.get("gdepth"),
        },
        throw: ThrowSettings {
            time: note_beats(spec.get("wtime")),
            feedback: spec.get("wfb"),
            tone_hz: spec.get("wtone"),
            pingpong: on("wpp"),
            level: spec.get("wlvl"),
        },
        cloud: CloudSettings {
            level: spec.get("clevel"),
            position_ms: spec.get("cpos"),
            spray_ms: spec.get("cspray"),
            size_ms: spec.get("csize"),
            density_hz: spec.get("cdens"),
            pitch_st: spec.get("cpitch"),
            pitch_rand_st: spec.get("crand"),
            scale: Scale::from_index(spec.get("cscale")),
            spread: spec.get("cspread"),
            reverse: spec.get("crev"),
            window: GrainWindow::from_index(spec.get("cwin")),
            freeze: on("cfreeze"),
        },
    }
}

/// The painted blocks of `model` written into `out`'s steps, in place: every step off,
/// then each block a start and its holds. Allocation-free.
pub fn pattern_into(model: Option<&model::Pattern>, out: &mut Pattern) {
    for lane in out.lanes.iter_mut() {
        lane.fill(Step::OFF);
    }
    let Some(m) = model else {
        return;
    };
    for (&l, blocks) in &m.lanes {
        let Some(lane) = out.lanes.get_mut(usize::from(l)) else {
            continue;
        };
        for b in blocks {
            let from = usize::from(b.at);
            for (k, s) in lane
                .iter_mut()
                .enumerate()
                .skip(from)
                .take(usize::from(b.len.max(1)))
            {
                *s = if k == from {
                    Step::start(b.amount, b.prob)
                } else {
                    Step::HOLD
                };
            }
        }
    }
}

/// Whether settings and a pattern make the DSP more than a wire (then the unit needs it).
pub fn needs_dsp(s: &FractusSettings, model: Option<&model::Pattern>) -> bool {
    model.is_some_and(|p| !p.is_empty())
        || s.cloud.level > 0.0
        || s.cloud.freeze
        || s.trigger != Trigger::Sequence
        || s.out_db != 0.0
}

#[derive(Clone)]
pub struct Unit {
    sample_rate: f32,
    set: FractusSettings,
    pat: Box<Pattern>,
    need: bool,
    transport: Transport,
    dsp: Option<Box<Fractus>>,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fractus::Unit(needs {}, built {})",
            self.need,
            self.dsp.is_some()
        )
    }
}

impl Unit {
    /// A unit without its DSP (a wire until its memory comes): cheap, any thread.
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            sample_rate,
            set: FractusSettings::default(),
            pat: Box::new(Pattern::EMPTY),
            need: false,
            transport: Transport {
                playing: false,
                tempo_bpm: 120.0,
                samples: 0,
            },
            dsp: None,
        }
    }

    /// The DSP once it came, for the editor's drawing and the tests.
    pub fn dsp(&self) -> Option<&Fractus> {
        self.dsp.as_deref()
    }
}

/// What a DSP built for `sample_rate` is: the answer to `MemoryNeed::Fractus`.
pub fn build(sample_rate: f32) -> Box<Fractus> {
    Box::new(Fractus::new(sample_rate))
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.set = settings(spec);
        let model = spec.fractus.as_deref();
        pattern_into(model, &mut self.pat);
        self.need = needs_dsp(&self.set, model);
        if let Some(d) = self.dsp.as_deref_mut() {
            d.set(&self.set);
            d.set_pattern(&self.pat);
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        if let Some(d) = self.dsp.as_deref_mut() {
            d.process(left, right);
        }
    }

    fn reset(&mut self) {
        if let Some(d) = self.dsp.as_deref_mut() {
            d.reset();
        }
    }

    fn at_rest(&self) -> bool {
        self.dsp.as_deref().is_none_or(Fractus::at_rest)
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.as_deref().and_then(Fractus::rest_blocker)
    }

    fn skip(&mut self, frames: usize) {
        if let Some(d) = self.dsp.as_deref_mut() {
            d.skip(frames);
        }
    }

    fn latency(&self) -> usize {
        0
    }

    fn telemetry(&self) -> Telemetry {
        match self.dsp.as_deref() {
            Some(d) => telemetry_of(|t| d.telemetry(t)),
            None => [0.0; super::TELEMETRY_LEN],
        }
    }

    fn follows_song(&self) -> bool {
        true
    }

    fn set_position(&mut self, position: &SongPosition) {
        self.transport = transport_of(position);
        if let Some(d) = self.dsp.as_deref_mut() {
            d.set_transport(&self.transport);
        }
    }

    /// The notes of the slot's track (ADR 0022): they play the lanes, at their sample.
    fn note(&mut self, offset: usize, note: FxNote) {
        if let Some(d) = self.dsp.as_deref_mut() {
            d.note(
                offset,
                match note {
                    FxNote::On { pitch, velocity } => Note::On(pitch, velocity.max(1)),
                    FxNote::Off { pitch } => Note::Off(pitch),
                    FxNote::AllOff => Note::AllOff,
                },
            );
        }
    }

    fn memory_need(&self) -> Option<MemoryNeed> {
        (self.need && self.dsp.is_none()).then_some(MemoryNeed::Fractus)
    }

    fn supply(&mut self, memory: EffectMemory) -> Option<EffectMemory> {
        match memory {
            EffectMemory::Fractus(mut d)
                if self.dsp.is_none() && d.sample_rate() == self.sample_rate =>
            {
                d.set(&self.set);
                d.set_pattern(&self.pat);
                d.set_transport(&self.transport);
                self.dsp = Some(d);
                None
            }
            other => Some(other),
        }
    }
}
