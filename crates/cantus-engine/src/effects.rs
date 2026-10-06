//! Running instances of native effects. Units are **built on the UI thread** (they own
//! delay lines) and handed to the audio thread inside a `Box`; when a snapshot stops
//! referencing one, the audio thread hands the box back so it is freed off-thread.

use cantus_core::{eq_key, EffectId, EffectKind, EffectSlot, EffectSpec};
use cantus_dsp::aequalis::MAX_BANDS;
use cantus_dsp::math;
use cantus_dsp::{
    Aequalis, BandSettings, CompMeter, CompSettings, Comprimo, DelaySettings, EqShape, Listen,
    Placement, Reverb, ReverbSettings, StereoDelay, Style,
};

#[derive(Debug)]
enum Dsp {
    Delay(StereoDelay),
    Reverb(Reverb),
    /// Boxed: two dozen bands of state is not something to move around by value.
    Eq(Box<Aequalis>),
    Comp(Box<Comprimo>),
    /// Every plug-in behind the engine's contract (`crate::fx`).
    Fx(Box<dyn crate::fx::FxUnit>),
}

/// A compressor's parameters as the DSP wants them; also what its editor draws from, so
/// the picture and the sound come from one reading of the spec. Allocation-free: it runs
/// on the audio thread in `configure`.
pub fn comp_settings(spec: &EffectSpec) -> CompSettings {
    use cantus_core::fx::compressor::{params, KEY_BANDS, KEY_BAND_FIELDS};
    use cantus_dsp::comprimo::{KeyBand, StyleSpec};
    let g = |k: &str| spec.get(k);
    let on = |k: &str| spec.get(k) >= 0.5;
    // The bands' keys are the last block of the parameter list, band by band, in the
    // order of `KEY_BAND_FIELDS` — read through their `&'static str`s, because the audio
    // thread calls this and must not build strings.
    let infos = params();
    let fields = KEY_BAND_FIELDS.len();
    let base = infos.len() - KEY_BANDS * fields;
    let field = |band: usize, f: usize| spec.get(infos[base + band * fields + f].key);
    let mut key_bands = [KeyBand::default(); cantus_dsp::comprimo::KEY_BANDS];
    for (i, b) in key_bands.iter_mut().enumerate().take(KEY_BANDS) {
        *b = KeyBand {
            active: field(i, 0) >= 0.5 && field(i, 1) >= 0.5,
            shape: EqShape::from_index(field(i, 2).round().max(0.0) as usize),
            freq: field(i, 3),
            gain_db: field(i, 4),
            q: field(i, 5),
            slope: (field(i, 6) / 6.0).round().clamp(1.0, 16.0) * 6.0,
        };
    }
    CompSettings {
        threshold_db: g("thr"),
        ratio: g("ratio"),
        knee_db: g("knee"),
        range_db: g("range"),
        attack_ms: g("att"),
        release_ms: g("rel"),
        auto_release: on("auto_rel"),
        hold_ms: g("hold"),
        lookahead_ms: g("look"),
        makeup_db: g("makeup"),
        auto_gain: on("auto_gain"),
        dry_db: g("dry"),
        mix: g("mix"),
        input_db: g("in_gain"),
        output_db: g("out_gain"),
        style: Style::from_index(g("style").round().max(0.0) as usize),
        engine: on("custom").then(|| StyleSpec {
            rms_ms: g("det_rms"),
            feedback: g("det_fb"),
            program_release: g("det_prog"),
            dual: g("det_dual"),
            curve: g("det_curve"),
        }),
        link: g("link"),
        key_hpf_hz: g("key_hpf"),
        key_gain_db: g("key_gain"),
        key_eq: on("key_eq"),
        key_bands,
        listen: Listen::from_index(g("listen").round().max(0.0) as usize),
        oversample: cantus_dsp::comprimo::oversample::FACTORS
            [(g("os").round().max(0.0) as usize).min(2)],
    }
}

/// An EQ's parameters as the DSP wants them. Also what the editor draws its curve from, so
/// the picture and the sound come from one reading of the spec: the gain scale and the
/// gain–Q interaction are applied here, once, for both.
pub fn eq_bands(spec: &EffectSpec) -> [BandSettings; MAX_BANDS] {
    use cantus_core::fx::eq::{params, EQ_BAND_FIELDS};
    let scale = spec.get("scale");
    let mut out = [BandSettings::default(); MAX_BANDS];
    // The band keys are read through the parameter table's `&'static str`s, band by band
    // in `EQ_BAND_FIELDS` order: the audio thread calls this in `configure`, and building
    // `b3.gain` as a `String` there allocated about a thousand times per snapshot (found
    // by the allocation-counting test, ADR 0022).
    let infos = params();
    let fields = EQ_BAND_FIELDS.len();
    let at = |name: &str| EQ_BAND_FIELDS.iter().position(|f| f.0 == name);
    for (i, b) in out.iter_mut().enumerate() {
        let g = |f: &str| match at(f) {
            Some(k) if i * fields + k < infos.len() => spec.get(infos[i * fields + k].key),
            _ => spec.get(&eq_key(i, f)),
        };
        let shape = EqShape::from_index(g("shape").round().max(0.0) as usize);
        let gain = g("gain") * scale;
        // Gain–Q interaction: a bell narrows as it grows — its Q doubles every 12 dB of
        // gain either way — the way the classic analog designs behave.
        let q = if g("gq") >= 0.5 && shape == EqShape::Bell {
            (g("q") * math::powf(10f32, gain.abs() / 40.0)).min(40.0)
        } else {
            g("q")
        };
        *b = BandSettings {
            active: g("used") >= 0.5 && g("on") >= 0.5,
            shape,
            freq: g("freq"),
            gain_db: gain,
            q,
            slope: (g("slope") / 6.0).round().clamp(1.0, 16.0) * 6.0,
            placement: Placement::from_index(g("place").round().max(0.0) as usize),
            dynamic: g("dyn") >= 0.5,
            threshold_db: g("thr"),
            range_db: g("range") * scale,
            ratio: g("ratio"),
            attack_ms: g("att"),
            release_ms: g("rel"),
            below: g("dir") >= 0.5,
            external: g("key") >= 0.5,
        };
    }
    out
}

/// Memory an effect needs for a setting and must not allocate on the audio thread
/// (ADR 0022): the engine asks for it (`EngineEvent::EffectNeed`), the UI thread builds it
/// ([`EffectMemory::build`]) and sends it (`EngineMessage::EffectMemory`), and the effect
/// switches when it arrives, running its previous path meanwhile.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemoryNeed {
    /// Aequalis's linear-phase convolver, with room for this resolution.
    EqLinear(cantus_dsp::aequalis::Resolution),
}

/// What answers a [`MemoryNeed`]: built off the audio thread, and freed off it when an
/// effect hands it back (`EngineEvent::GarbageMemory`).
#[derive(Debug)]
pub enum EffectMemory {
    EqLinear(Box<cantus_dsp::aequalis::phase::LinearPhase>),
}

impl EffectMemory {
    /// Build what `need` asks for. Allocates: for the UI thread or an offline render.
    pub fn build(need: MemoryNeed, sample_rate: f32) -> Self {
        match need {
            MemoryNeed::EqLinear(res) => EffectMemory::EqLinear(Box::new(
                cantus_dsp::aequalis::phase::LinearPhase::new(sample_rate, res),
            )),
        }
    }
}

#[derive(Debug)]
pub struct EffectUnit {
    pub id: EffectId,
    pub kind: EffectKind,
    dsp: Dsp,
    /// Swapped out for a unit with the same id (`EngineMessage::ReplaceEffect`): the id is
    /// still in the engine, so the handle must not forget it when this one comes back.
    pub replaced: bool,
    /// The memory last asked for, so an unchanged need is asked for once.
    asked: Option<MemoryNeed>,
}

impl EffectUnit {
    /// Build a unit for a slot. On the UI thread (or an offline render), so whatever the
    /// slot's settings need — linear-phase buffers for an EQ loaded in linear phase — is
    /// made here and the unit starts on the path it is set to.
    pub fn new(slot: &EffectSlot, sample_rate: f32) -> Self {
        let dsp = match slot.spec.kind {
            EffectKind::Delay => Dsp::Delay(StereoDelay::new(sample_rate)),
            EffectKind::Reverb => Dsp::Reverb(Reverb::new(sample_rate)),
            EffectKind::Eq => Dsp::Eq(Box::new(Aequalis::realtime(sample_rate))),
            EffectKind::Compressor => Dsp::Comp(Box::new(Comprimo::new(sample_rate))),
            other => Dsp::Fx(
                crate::fx::make(other, sample_rate).expect("every other kind is a plug-in unit"),
            ),
        };
        let mut unit = EffectUnit {
            id: slot.id,
            kind: slot.spec.kind,
            dsp,
            replaced: false,
            asked: None,
        };
        unit.configure(&slot.spec);
        if let Some(need) = unit.memory_need() {
            drop(unit.supply(EffectMemory::build(need, sample_rate)));
        }
        unit.reset();
        unit
    }

    /// Memory the unit's settings need and it has not got; `None` when it has all it needs.
    pub fn memory_need(&self) -> Option<MemoryNeed> {
        match &self.dsp {
            Dsp::Eq(e) => e.linear_need().map(MemoryNeed::EqLinear),
            _ => None,
        }
    }

    /// The need to ask the UI thread for, if it has changed since the last ask; `None`
    /// otherwise. Allocation-free: the audio thread calls it after every snapshot.
    pub fn take_request(&mut self) -> Option<MemoryNeed> {
        let need = self.memory_need();
        if need == self.asked {
            return None;
        }
        self.asked = need;
        need
    }

    /// The request could not be sent: ask again at the next chance.
    pub fn forget_request(&mut self) {
        self.asked = None;
    }

    /// Hand the unit memory built for it. What comes back — memory it replaced, or what
    /// was offered if the unit had no use for it — is to be freed off the audio thread.
    /// No allocation.
    pub fn supply(&mut self, memory: EffectMemory) -> Option<EffectMemory> {
        self.asked = None;
        match (memory, &mut self.dsp) {
            (EffectMemory::EqLinear(lp), Dsp::Eq(e)) => {
                e.supply_linear(lp).map(EffectMemory::EqLinear)
            }
            (memory, _) => Some(memory),
        }
    }

    /// Apply parameters. A spec of another kind is ignored (cannot happen through the
    /// command layer, but the audio thread must never trust its inputs blindly).
    pub fn configure(&mut self, spec: &EffectSpec) {
        if spec.kind != self.kind {
            return;
        }
        match &mut self.dsp {
            Dsp::Delay(d) => d.set(&DelaySettings {
                time_ms: spec.get("time_ms"),
                feedback: spec.get("feedback"),
                tone_hz: spec.get("tone_hz"),
                ping_pong: spec.get("ping_pong") >= 0.5,
                mix: spec.get("mix"),
            }),
            Dsp::Reverb(r) => r.set(&ReverbSettings {
                size: spec.get("size"),
                damping: spec.get("damping"),
                width: spec.get("width"),
                predelay_ms: spec.get("predelay_ms"),
                mix: spec.get("mix"),
            }),
            Dsp::Eq(e) => {
                let solo = spec.get("solo").round();
                e.set_options(cantus_dsp::aequalis::Options {
                    output_db: spec.get("out"),
                    mode: cantus_dsp::aequalis::Mode::from_index(spec.get("mode").round() as usize),
                    resolution: cantus_dsp::aequalis::Resolution::from_index(
                        spec.get("lp_res").round() as usize,
                    ),
                    auto_gain: spec.get("auto") >= 0.5,
                    solo: (solo >= 1.0).then(|| solo as usize - 1),
                });
                e.set(&eq_bands(spec), spec.get("out"));
            }
            Dsp::Comp(c) => c.set(&comp_settings(spec)),
            Dsp::Fx(u) => u.configure(spec),
        }
    }

    /// A unit around any [`crate::fx::FxUnit`] — for tests of the host (a pure delay that
    /// exercises compensation) and, later, for units the engine did not build itself.
    pub fn from_fx(slot: &EffectSlot, mut unit: Box<dyn crate::fx::FxUnit>) -> Self {
        unit.configure(&slot.spec);
        EffectUnit {
            id: slot.id,
            kind: slot.spec.kind,
            dsp: Dsp::Fx(unit),
            replaced: false,
            asked: None,
        }
    }

    /// Where the song is, for a unit that follows it (a hosted plug-in).
    #[inline]
    pub fn set_position(&mut self, position: &crate::fx::SongPosition) {
        if let Dsp::Fx(u) = &mut self.dsp {
            u.set_position(position);
        }
    }

    /// Hand what the unit's own editor did to `f`, oldest first, and forget it.
    #[inline]
    pub fn drain_param_events(&mut self, mut f: impl FnMut(crate::fx::FxParamEvent)) {
        if let Dsp::Fx(u) = &mut self.dsp {
            for e in u.param_events() {
                f(*e);
            }
            u.clear_param_events();
        }
    }

    /// The unit will not be processed again: called on the audio thread before it is
    /// handed back to be dropped.
    pub fn stop(&mut self) {
        if let Dsp::Fx(u) = &mut self.dsp {
            u.stop();
        }
    }

    /// Samples of delay this unit adds, for delay compensation.
    pub fn latency(&self) -> usize {
        match &self.dsp {
            Dsp::Eq(e) => e.latency(),
            Dsp::Comp(c) => c.latency(),
            Dsp::Fx(u) => u.latency(),
            _ => 0,
        }
    }

    /// What a plug-in behind the contract reports to its editor; `None` for the others.
    pub fn telemetry(&self) -> Option<crate::fx::Telemetry> {
        match &self.dsp {
            Dsp::Fx(u) => Some(u.telemetry()),
            Dsp::Eq(e) => Some(e.telemetry()),
            Dsp::Comp(c) => Some(c.telemetry()),
            _ => None,
        }
    }

    /// What a compressor did in the last block; `None` for anything else.
    pub fn comp_meter(&self) -> Option<CompMeter> {
        match &self.dsp {
            Dsp::Comp(c) => Some(c.meter()),
            _ => None,
        }
    }

    /// The gain reduction a dynamics unit applies now, dB (positive): a compressor's, or
    /// what a unit behind the contract reports (the limiter, the maximizer). `None` for
    /// the others.
    pub fn gain_reduction_db(&self) -> Option<f32> {
        match &self.dsp {
            Dsp::Comp(c) => Some(c.meter().gr_db),
            Dsp::Fx(u) => u.gain_reduction_db(),
            _ => None,
        }
    }

    /// The gain each dynamic EQ band is applying now, dB; `None` for anything but an EQ.
    pub fn eq_deltas(&self) -> Option<[f32; MAX_BANDS]> {
        match &self.dsp {
            Dsp::Eq(e) => Some(e.dynamic_deltas()),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        match &mut self.dsp {
            Dsp::Delay(d) => d.reset(),
            Dsp::Reverb(r) => r.reset(),
            Dsp::Eq(e) => e.reset(),
            Dsp::Comp(c) => c.reset(),
            Dsp::Fx(u) => u.reset(),
        }
    }

    #[inline]
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.process_keyed(l, r, None);
    }

    /// A note from the slot's MIDI source, `offset` samples into the next process call
    /// (ADR 0022). Only plug-ins behind the contract hear notes; the others ignore them.
    #[inline]
    pub fn note(&mut self, offset: usize, note: crate::fx::FxNote) {
        if let Dsp::Fx(u) = &mut self.dsp {
            u.note(offset, note);
        }
    }

    /// Process with an external key for the effects that take one (a compressor's
    /// sidechain); the others ignore it.
    #[inline]
    pub fn process_keyed(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        match &mut self.dsp {
            Dsp::Delay(d) => d.process(l, r),
            Dsp::Reverb(v) => v.process(l, r),
            Dsp::Eq(e) => e.process_keyed(l, r, key),
            Dsp::Comp(c) => c.process(l, r, key),
            Dsp::Fx(u) => u.process(l, r, key),
        }
    }
}
