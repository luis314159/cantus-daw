//! Cedo, the ducker (ADR 0141), behind the engine's plug-in contract. Owned
//! by the wave-B builder.
//!
//! [`settings`] reads the table (`cantus_core::fx::cedo`) into the DSP's settings, and the
//! drawn shape ([`cantus_core::EffectSpec::cedo`], the classic pump when `None`) into its
//! curve, bent by the automation editor's shapes (`fluctus::shape_at`, as Lapsus's). Its
//! three triggers reach it the ways the engine already has:
//!
//! - **Sidechain**: the slot's key (`EffectSlot::sidechain`), another strip's post-fader
//!   output, passed to `process` by the strip runner. The strip's rest check sees the key
//!   (`graph::strip::may_rest`: a keyed insert whose key strip sounded this pass keeps its
//!   strip awake), so a ducker whose own input is silent while its key plays keeps its
//!   envelope where the key puts it, and its input comes back ducked as it should.
//! - **Tempo**: the song's position each pass ([`FxUnit::set_position`]).
//! - **MIDI**: the notes of the track the slot listens to (`EffectSlot::midi_from`, ADR
//!   0022 — another track's), each at its sample ([`FxUnit::note`]); a strip with a note
//!   listener never rests.
//!
//! The latency is the lookahead plus, in a band mode split at linear phase, the kernel's at
//! its floor (`lfloor`: Auto an octave under the crossover as last committed, `lfhz`, or a
//! fixed 40, 60, 80, 120 Hz — 55.3, 36.8, 27.6, 18.3 ms at 48 kHz,
//! `cantus_dsp::cedo::band::Floor`); the engine reads it each pass, so its delay
//! compensation follows either.

use super::fluctus::shape_at;
use super::modfx::transport_of;
use super::{FxNote, FxUnit, SongPosition, Telemetry, TELEMETRY_LEN};
use cantus_core::fx::cedo::MAX_POINTS;
use cantus_core::fx::lapsus::Point;
use cantus_core::EffectSpec;
use cantus_dsp::cedo::band::{Band, Crossover};
use cantus_dsp::cedo::env::{Detector, KeyFilterMode, ReleaseShape};
use cantus_dsp::cedo::{Cedo, CedoSettings, Listen, Trigger, CLASSIC_PUMP};
use cantus_dsp::fluctus::CurvePoint;

/// The telemetry's layout (what the editor draws), in the meter's units.
pub mod t {
    /// The most gain taken away in the block, dB (positive), and each channel's at its end.
    pub const GR: usize = 0;
    pub const GR_L: usize = 1;
    pub const GR_R: usize = 2;
    /// The amount, 0 … 1, at the block's end.
    pub const AMOUNT: usize = 3;
    /// The filtered key's peak, dBFS (−200: silence or none); 1 when it reached the
    /// threshold in the block.
    pub const KEY_DB: usize = 4;
    pub const TRIGGERED: usize = 5;
    /// Where the shape's cycle is (0 … 1), −1 when none runs: the display's playhead.
    pub const PHASE: usize = 6;
    /// The input's and the output's peaks, dBFS.
    pub const IN_DB: usize = 7;
    pub const OUT_DB: usize = 8;
    /// The latency, samples.
    pub const LATENCY: usize = 9;
    /// The song's tempo; 1 while it plays.
    pub const TEMPO: usize = 10;
    pub const PLAYING: usize = 11;
    /// 1 while a key is routed.
    pub const KEYED: usize = 12;
    /// The trigger in force, 0 … 2.
    pub const TRIGGER: usize = 13;
    pub const SAMPLE_RATE: usize = 14;
    /// The end of Cedo's numbers.
    pub const END: usize = 15;
}

/// Cedo's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> CedoSettings {
    let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
    CedoSettings {
        trigger: Trigger::from_index(idx("trig")),
        depth_db: spec.get("depth"),
        threshold_db: spec.get("thr"),
        attack_ms: spec.get("att"),
        hold_ms: spec.get("hold"),
        release_ms: spec.get("rel"),
        release_shape: ReleaseShape::from_index(idx("rshape")),
        key_filter: KeyFilterMode::from_index(idx("kfilt")),
        detector: Detector::from_index(idx("det")),
        rms_ms: spec.get("rms"),
        key_hz: spec.get("khz"),
        key_q: spec.get("kq"),
        lookahead_ms: spec.get("look"),
        division: idx("div"),
        swing: spec.get("swing"),
        edge_ms: spec.get("edge"),
        band: Band::from_index(idx("band")),
        crossover: if idx("xover") >= 1 {
            Crossover::LinearPhase
        } else {
            Crossover::MinimumPhase
        },
        cross_hz: spec.get("xhz"),
        floor: cantus_dsp::cedo::band::Floor::from_index(idx("lfloor")),
        auto_floor_hz: spec.get("lfhz"),
        link: spec.get("link"),
        mix: spec.get("mix"),
        out_db: spec.get("out"),
        listen: match idx("listen") {
            1 => Listen::Delta,
            2 => Listen::Key,
            _ => Listen::Output,
        },
        velocity: spec.get("vel") >= 0.5,
    }
}

/// The shape's points as the DSP's curve takes them (at most [`MAX_POINTS`]); the classic
/// pump when the spec has none.
pub fn shape_points(spec: &EffectSpec, out: &mut [CurvePoint; MAX_POINTS]) -> usize {
    let Some(shape) = spec.cedo.as_deref() else {
        out[..CLASSIC_PUMP.len()].copy_from_slice(&CLASSIC_PUMP);
        return CLASSIC_PUMP.len();
    };
    let n = shape.points.len().min(MAX_POINTS);
    for (d, p) in out.iter_mut().zip(&shape.points[..n]) {
        *d = curve_point(p);
    }
    n
}

fn curve_point(p: &Point) -> CurvePoint {
    CurvePoint {
        x: p.x,
        y: p.y,
        tension: p.t,
        shape: p.s,
    }
}

#[derive(Clone)]
pub struct Unit {
    dsp: Box<Cedo>,
    sample_rate: f32,
    keyed: bool,
    tempo: f32,
    playing: bool,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cedo::Unit({:?})", self.dsp.settings().trigger)
    }
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let mut dsp = Box::new(Cedo::new(sample_rate, shape_at));
        dsp.set_shape(&CLASSIC_PUMP);
        Unit {
            dsp,
            sample_rate,
            keyed: false,
            tempo: 120.0,
            playing: false,
        }
    }

    /// The DSP, for the editor's drawing and the tests.
    pub fn dsp(&self) -> &Cedo {
        &self.dsp
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
        let mut pts = [CurvePoint::default(); MAX_POINTS];
        let n = shape_points(spec, &mut pts);
        if self.dsp.shape_points() != &pts[..n] {
            self.dsp.set_shape(&pts[..n]);
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        self.keyed = key.is_some();
        self.dsp.process(left, right, key);
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

    fn gain_reduction_db(&self) -> Option<f32> {
        Some(self.dsp.meter().gr_db)
    }

    fn follows_song(&self) -> bool {
        true
    }

    fn set_position(&mut self, position: &SongPosition) {
        self.tempo = position.tempo_bpm as f32;
        self.playing = position.playing;
        self.dsp.set_transport(&transport_of(position));
    }

    /// The notes of the track the slot listens to (ADR 0022): each note-on fires the shape
    /// at its sample (the MIDI trigger; the DSP ignores them otherwise).
    fn note(&mut self, offset: usize, note: FxNote) {
        if let FxNote::On { velocity, .. } = note {
            self.dsp.note(offset, velocity);
        }
    }

    fn telemetry(&self) -> Telemetry {
        let m = self.dsp.meter();
        let mut out = [0.0; TELEMETRY_LEN];
        out[t::GR] = m.gr_db;
        out[t::GR_L] = m.gr_now[0];
        out[t::GR_R] = m.gr_now[1];
        out[t::AMOUNT] = m.amount;
        out[t::KEY_DB] = m.key_db;
        out[t::TRIGGERED] = if m.triggered { 1.0 } else { 0.0 };
        out[t::PHASE] = m.phase;
        out[t::IN_DB] = m.in_db;
        out[t::OUT_DB] = m.out_db;
        out[t::LATENCY] = self.dsp.latency() as f32;
        out[t::TEMPO] = self.tempo;
        out[t::PLAYING] = if self.playing { 1.0 } else { 0.0 };
        out[t::KEYED] = if self.keyed { 1.0 } else { 0.0 };
        out[t::TRIGGER] = match self.dsp.settings().trigger {
            Trigger::Sidechain => 0.0,
            Trigger::Tempo => 1.0,
            Trigger::Midi => 2.0,
        };
        out[t::SAMPLE_RATE] = self.sample_rate;
        out
    }
}
