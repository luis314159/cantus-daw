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

/// Delay's parameters as the DSP wants them; also what its window draws the echoes from
/// (round 40: the synced time, ADR 0119). Allocation-free. // r40:delay
pub fn delay_settings(spec: &EffectSpec) -> DelaySettings {
    DelaySettings {
        time_ms: spec.get("time_ms"),
        feedback: spec.get("feedback"),
        tone_hz: spec.get("tone_hz"),
        ping_pong: spec.get("ping_pong") >= 0.5,
        mix: spec.get("mix"),
        sync: spec.get("sync") >= 0.5,
        note: cantus_dsp::delay::sync::note_index(spec.get("note")),
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
    /// Aula's response, by what it is built from (ADR 0112). // r39:conv
    Aula(crate::fx::aula::AulaNeed),
    /// Delay's line for a synced time, in cells (ADR 0119). // r40:delay
    DelayLine(usize),
    /// Lapsus's history, in frames (ADR 0129). // fx:lapsus
    Lapsus(usize),
    /// Tabula's alignment and dry lines, in frames (ADR 0131). // fx3:contract
    Tabula(usize),
    /// Arca's cabinet, by what it is made from (ADR 0134). // fx3:arca
    Arca(crate::fx::arca::ArcaNeed),
    /// Fractus's whole DSP, its buffers with it (ADR 0137). // fxa:fractus
    Fractus,
}

/// What travels in [`crate::messages::EngineEvent::EffectNeed`]: a [`MemoryNeed`] small
/// enough to go by value (an EQ's resolution), or a handle to the unit's own mailbox where
/// a larger one waits (Aula's response, `crate::need_mailbox`) — so the event stays within
/// ADR 0107's 64 B. Cloning the handle on the audio thread counts a reference; it never
/// allocates or frees there.
#[derive(Clone, Debug)]
pub enum NeedTicket {
    EqLinear(cantus_dsp::aequalis::Resolution),
    Aula(std::sync::Arc<crate::need_mailbox::NeedMailbox>),
    DelayLine(usize),                // r40:delay
    Lapsus(usize),                   // fx:lapsus
    Tabula(usize),                   // fx3:contract
    Arca(crate::fx::arca::ArcaNeed), // fx3:arca
    Fractus,                         // fxa:fractus
}

impl NeedTicket {
    /// The need itself — for a mailbox, the latest the unit wrote. Off the audio thread.
    pub fn need(&self) -> MemoryNeed {
        match self {
            NeedTicket::EqLinear(r) => MemoryNeed::EqLinear(*r),
            NeedTicket::Aula(m) => MemoryNeed::Aula(m.read()),
            NeedTicket::DelayLine(n) => MemoryNeed::DelayLine(*n), // r40:delay
            NeedTicket::Lapsus(n) => MemoryNeed::Lapsus(*n),       // fx:lapsus
            NeedTicket::Tabula(n) => MemoryNeed::Tabula(*n),       // fx3:contract
            NeedTicket::Arca(n) => MemoryNeed::Arca(*n),           // fx3:arca
            NeedTicket::Fractus => MemoryNeed::Fractus,            // fxa:fractus
        }
    }
}

/// What answers a [`MemoryNeed`]: built off the audio thread, and freed off it when an
/// effect hands it back (`EngineEvent::GarbageMemory`).
#[derive(Debug)]
pub enum EffectMemory {
    EqLinear(Box<cantus_dsp::aequalis::phase::LinearPhase>),
    /// Aula's kernel (and a state when it needs more room). // r39:conv
    Aula(Box<crate::fx::aula::Memory>),
    /// Delay's longer (or the free) line (ADR 0119). // r40:delay
    DelayLine(Box<cantus_dsp::delay::Lines>),
    /// Lapsus's history. // fx:lapsus
    Lapsus(Box<cantus_dsp::lapsus::Ring>),
    /// Tabula's lines. // fx3:contract
    Tabula(Box<crate::fx::tabula::Lines>),
    /// Arca's cabinet. // fx3:arca
    Arca(Box<crate::fx::arca::Memory>),
    /// Fractus's DSP. // fxa:fractus
    Fractus(Box<cantus_dsp::fractus::Fractus>),
}

/// The mailbox a unit of `kind` asks with, when its requests are too large for the event.
fn mailbox_for(kind: EffectKind) -> Option<std::sync::Arc<crate::need_mailbox::NeedMailbox>> {
    (kind == EffectKind::Aula).then(Default::default)
}

impl EffectMemory {
    /// Build what `need` asks for. Allocates: for the UI thread or an offline render.
    pub fn build(need: MemoryNeed, sample_rate: f32) -> Self {
        match need {
            MemoryNeed::EqLinear(res) => EffectMemory::EqLinear(Box::new(
                cantus_dsp::aequalis::phase::LinearPhase::new(sample_rate, res),
            )),
            MemoryNeed::Aula(n) => {
                EffectMemory::Aula(Box::new(crate::fx::aula_store::build(n, sample_rate)))
            } // r39:conv
            MemoryNeed::DelayLine(len) => {
                EffectMemory::DelayLine(Box::new(cantus_dsp::delay::Lines::new(len)))
            } // r40:delay
            MemoryNeed::Lapsus(n) => {
                EffectMemory::Lapsus(Box::new(cantus_dsp::lapsus::Ring::new(n)))
            } // fx:lapsus
            MemoryNeed::Tabula(n) => {
                EffectMemory::Tabula(Box::new(crate::fx::tabula::Lines::new(n)))
            } // fx3:contract
            MemoryNeed::Arca(n) => {
                EffectMemory::Arca(Box::new(crate::fx::arca::build(n, sample_rate)))
            } // fx3:arca
            MemoryNeed::Fractus => EffectMemory::Fractus(crate::fx::fractus::build(sample_rate)), // fxa:fractus
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
    /// Where a request too large for the event waits (Aula's; [`NeedTicket`]): made with
    /// the unit, off the audio thread.
    mailbox: Option<std::sync::Arc<crate::need_mailbox::NeedMailbox>>,
    /// A hosted plug-in's timed parameter changes for the next pass (ADR 0057): room only
    /// for a hosted unit.
    pub automation: crate::modulator::AutoQueue,
    /// How late the audio reaching this insert is against the transport, in samples:
    /// its strip's compensation and the latency of the inserts before it. It hears the
    /// song position that much earlier (`Processor::scope_delays`; integration review).
    pub late: usize,
    /// A latency it reports in place of its own: a stand-in for a hosted plug-in that is
    /// not instantiated, in a plan made only to know when audio arrives
    /// (`freeze::render::strip_arrival`, review 3). Never processed.
    hint: Option<usize>,
}

impl EffectUnit {
    // r34:listen — a range render's checkpoint (ADR 0083).
    /// A copy of this unit with all its state, or `None` when its state cannot be copied
    /// (a hosted plug-in): what a range render resumes from.
    pub fn fork(&self) -> Option<EffectUnit> {
        let dsp = match &self.dsp {
            Dsp::Delay(d) => Dsp::Delay(d.clone()),
            Dsp::Reverb(r) => Dsp::Reverb(r.clone()),
            Dsp::Eq(e) => Dsp::Eq(e.clone()),
            Dsp::Comp(c) => Dsp::Comp(c.clone()),
            Dsp::Fx(f) => Dsp::Fx(f.fork()?),
        };
        Some(EffectUnit {
            id: self.id,
            kind: self.kind,
            dsp,
            replaced: self.replaced,
            asked: self.asked,
            mailbox: mailbox_for(self.kind),
            automation: self.automation.clone(),
            late: self.late,
            hint: self.hint,
        })
    }

    /// Build a unit for a slot. On the UI thread (or an offline render), so whatever the
    /// slot's settings need — linear-phase buffers for an EQ loaded in linear phase — is
    /// made here and the unit starts on the path it is set to.
    pub fn new(slot: &EffectSlot, sample_rate: f32) -> Self {
        let dsp = match slot.spec.kind {
            EffectKind::Delay => Dsp::Delay(StereoDelay::new(sample_rate)),
            EffectKind::Reverb => Dsp::Reverb(Reverb::new(sample_rate)),
            EffectKind::Eq => Dsp::Eq(Box::new(Aequalis::realtime(sample_rate))),
            EffectKind::Compressor => Dsp::Comp(Box::new(Comprimo::new(sample_rate))),
            // fx3:contract — a board builds its pedals here, off the audio thread.
            EffectKind::Board => Dsp::Fx(Box::new(crate::fx::tabula::Unit::build(
                &slot.spec,
                sample_rate,
            ))),
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
            mailbox: mailbox_for(slot.spec.kind),
            automation: crate::modulator::AutoQueue::default(),
            late: 0,
            hint: None,
        };
        unit.configure(&slot.spec);
        if let Some(need) = unit.memory_need() {
            drop(unit.supply(EffectMemory::build(need, sample_rate)));
        }
        unit.reset();
        drop(unit.take_spent());
        unit
    }

    /// Memory the unit's settings need and it has not got; `None` when it has all it needs.
    pub fn memory_need(&self) -> Option<MemoryNeed> {
        match &self.dsp {
            Dsp::Eq(e) => e.linear_need().map(MemoryNeed::EqLinear),
            Dsp::Fx(u) => u.memory_need(), // r39:conv
            Dsp::Delay(d) => d.need().map(MemoryNeed::DelayLine), // r40:delay
            _ => None,
        }
    }

    /// The need to ask the UI thread for, if it has changed since the last ask; `None`
    /// otherwise — as the event carries it ([`NeedTicket`]). Allocation-free: the audio
    /// thread calls it after every snapshot.
    pub fn take_request(&mut self) -> Option<NeedTicket> {
        let need = self.memory_need();
        if need == self.asked {
            return None;
        }
        let Some(want) = need else {
            self.asked = None;
            return None;
        };
        let ticket = match want {
            MemoryNeed::EqLinear(r) => NeedTicket::EqLinear(r),
            MemoryNeed::DelayLine(n) => NeedTicket::DelayLine(n), // r40:delay
            MemoryNeed::Aula(n) => {
                // No mailbox (a unit not made by `new`): nothing to ask with.
                let m = self.mailbox.as_ref()?;
                m.write(&n);
                NeedTicket::Aula(m.clone())
            }
            MemoryNeed::Lapsus(n) => NeedTicket::Lapsus(n), // fx:lapsus
            MemoryNeed::Tabula(n) => NeedTicket::Tabula(n), // fx3:contract
            MemoryNeed::Arca(n) => NeedTicket::Arca(n),     // fx3:arca
            MemoryNeed::Fractus => NeedTicket::Fractus,     // fxa:fractus
        };
        self.asked = need;
        Some(ticket)
    }

    /// The request could not be sent: ask again at the next chance.
    pub fn forget_request(&mut self) {
        self.asked = None;
    }

    /// fix-a (A-M2): the longest note a Delay's automation or modulation of its `note` and
    /// `sync` can reach (`automation::FxCopy::line_note`): its line is built for that note
    /// before the song gets there, so it never finds it short. `None`: what its settings
    /// want.
    /// fx/final — a board passes it to every Delay among its pedals (`None`: each holds the
    /// line its settings want; [`Self::set_pedal_line_floor`] sets one).
    pub fn set_line_floor(&mut self, note: Option<usize>) {
        match &mut self.dsp {
            Dsp::Delay(d) => d.set_line_floor(note),
            Dsp::Fx(u) => {
                for k in u.pedals_mut() {
                    k.unit.set_line_floor(note);
                }
            }
            _ => {}
        }
    }

    /// fx/final — the line floor of one Delay on this board (`automation::FxCopy::
    /// pedal_lines`), as [`Self::set_line_floor`] sets an inserted Delay's. No allocation.
    pub fn set_pedal_line_floor(
        &mut self,
        pedal: cantus_core::fx::tabula::PedalId,
        note: Option<usize>,
    ) {
        if let Some(u) = self.pedal_mut(pedal) {
            u.set_line_floor(note);
        }
    }

    /// fix-a (A-M3): memory the unit let go of in a pass (a Delay's old line, once its
    /// history has moved into the new one), to be freed off the audio thread.
    pub fn take_spent(&mut self) -> Option<EffectMemory> {
        match &mut self.dsp {
            Dsp::Delay(d) => d.take_spent().map(EffectMemory::DelayLine),
            // fx/final — a Delay on a board (Tabula) lets go of its old line too.
            Dsp::Fx(u) => u.pedals_mut().iter_mut().find_map(|k| k.unit.take_spent()),
            _ => None,
        }
    }

    // fx3:contract — a board's pedals (ADR 0131; `FxUnit`'s defaults for every other unit).

    /// A pedal built for this board (`EngineMessage::AddPedal`); what comes back is dropped
    /// off the audio thread. No allocation.
    pub fn adopt(
        &mut self,
        pedal: cantus_core::fx::tabula::PedalId,
        unit: Box<EffectUnit>,
    ) -> Option<Box<EffectUnit>> {
        match &mut self.dsp {
            Dsp::Fx(u) => u.adopt(pedal, unit),
            _ => Some(unit),
        }
    }

    /// A pedal that has faded out, to be dropped off the audio thread.
    pub fn take_retired(&mut self) -> Option<Box<EffectUnit>> {
        match &mut self.dsp {
            Dsp::Fx(u) => u.take_retired(),
            _ => None,
        }
    }

    /// fix-a (A-M3): the unit is moving memory in or clearing it, or holds some to give
    /// back ([`Self::take_spent`]).
    pub fn busy(&mut self) -> bool {
        match &mut self.dsp {
            Dsp::Delay(d) => d.busy(),
            // fx/final — and a board while one of its pedals is.
            Dsp::Fx(u) => u.pedals_mut().iter_mut().any(|k| k.unit.busy()),
            _ => false,
        }
    }

    /// The pedals inside this unit (none but a board's).
    pub fn pedals_mut(&mut self) -> &mut [crate::fx::tabula::Kid] {
        match &mut self.dsp {
            Dsp::Fx(u) => u.pedals_mut(),
            _ => &mut [],
        }
    }

    /// One pedal inside this unit.
    pub fn pedal_mut(
        &mut self,
        pedal: cantus_core::fx::tabula::PedalId,
    ) -> Option<&mut EffectUnit> {
        self.pedals_mut()
            .iter_mut()
            .find(|k| k.pedal == pedal)
            .map(|k| &mut *k.unit) // fx3:tabula — a pedal's unit is boxed
    }

    /// What one pedal's editor draws (`EngineMessage::SetAnalysedPedal`).
    pub fn pedal_telemetry(
        &self,
        pedal: cantus_core::fx::tabula::PedalId,
    ) -> Option<crate::fx::Telemetry> {
        match &self.dsp {
            Dsp::Fx(u) => u.pedal_telemetry(pedal),
            _ => None,
        }
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
            (memory, Dsp::Fx(u)) => u.supply(memory), // r39:conv
            (EffectMemory::DelayLine(l), Dsp::Delay(d)) => d.supply(l).map(EffectMemory::DelayLine), // r40:delay
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
            Dsp::Delay(d) => d.set(&delay_settings(spec)),
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
            mailbox: mailbox_for(slot.spec.kind),
            automation: crate::modulator::AutoQueue::with_capacity(crate::modulator::HOSTED_QUEUE),
            late: 0,
            hint: None,
        }
    }

    /// A stand-in reporting `latency` (a hosted plug-in's, known from the session) for a
    /// plan that is never processed (review 3).
    pub fn latency_stand_in(slot: &EffectSlot, sample_rate: f32, latency: usize) -> Self {
        let mut unit = EffectUnit::new(slot, sample_rate);
        unit.hint = Some(latency);
        unit
    }

    /// Where the song is, for a unit that follows it (a hosted plug-in, a modulation
    /// unit, Delay's synced time — ADR 0119).
    #[inline]
    pub fn set_position(&mut self, position: &crate::fx::SongPosition) {
        match &mut self.dsp {
            Dsp::Fx(u) => u.set_position(position),
            Dsp::Delay(d) => d.set_tempo(position.tempo_bpm, position.tempo_inc != 0.0), // r40:delay
            _ => {}
        }
    }

    /// fx3:tabula — tell a unit that follows the song where it is, `self.late` samples
    /// before `position` (exact across tempo maps): a board tells each of its pedals as late
    /// as it hears it (`FxUnit::set_position_late`), every other unit hears its own.
    #[inline]
    pub fn hear_position(
        &mut self,
        position: &crate::fx::SongPosition,
        sample_rate: f64,
        map: &cantus_core::TempoMap,
    ) {
        if !self.follows_song() {
            return;
        }
        let late = self.late;
        match &mut self.dsp {
            Dsp::Fx(u) if self.kind == EffectKind::Board => {
                u.set_position_late(&|before| position.earlier_in(late + before, sample_rate, map))
            }
            Dsp::Fx(u) => u.set_position(&position.earlier_in(late, sample_rate, map)),
            Dsp::Delay(d) => {
                let p = position.earlier_in(late, sample_rate, map);
                d.set_tempo(p.tempo_bpm, p.tempo_inc != 0.0) // r40:delay
            }
            _ => {}
        }
    }

    /// Whether the unit reads the song position (`FxUnit::follows_song`): Delay only while
    /// its time is synced, so with the sync off nothing about its passes changes.
    #[inline]
    pub fn follows_song(&self) -> bool {
        match &self.dsp {
            Dsp::Fx(u) => u.follows_song(),
            Dsp::Delay(d) => d.settings().sync, // r40:delay
            _ => false,
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
        if let Some(h) = self.hint {
            return h;
        }
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

    /// True when the unit is at rest (ADR 0031 §5.1, [`crate::fx::FxUnit::at_rest`]):
    /// silence in gives silence out, skipping its calls on silent input — telling it with
    /// [`Self::skip`] — changes no sample, and it renders as a new unit would while its
    /// parameters stay. Exact; `false` whenever the unit cannot prove it.
    pub fn at_rest(&self) -> bool {
        match &self.dsp {
            Dsp::Delay(d) => d.at_rest(),
            Dsp::Reverb(r) => r.at_rest(),
            Dsp::Eq(e) => e.at_rest(),
            Dsp::Comp(c) => c.at_rest(),
            Dsp::Fx(u) => u.at_rest(),
        }
    }

    /// Why the unit is not at rest, by name; `None` exactly when [`Self::at_rest`] is true.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        match &self.dsp {
            Dsp::Eq(e) => e.rest_blocker(),
            Dsp::Comp(c) => c.rest_blocker(),
            Dsp::Fx(u) => u.rest_blocker(),
            _ => (!self.at_rest()).then_some("not at rest"),
        }
    }

    /// The engine skipped `frames` samples of silence while the unit was at rest
    /// ([`crate::fx::FxUnit::skip`]): advance whatever counts samples. Cheap.
    pub fn skip(&mut self, frames: usize) {
        match &mut self.dsp {
            Dsp::Delay(d) => d.skip(frames),
            // Integer ring positions only: an all-zero ring reads the same from any.
            Dsp::Reverb(_) => {}
            Dsp::Eq(e) => e.skip(frames),
            Dsp::Comp(c) => c.skip(frames),
            Dsp::Fx(u) => u.skip(frames),
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

    /// r40:pads — *Hear the key* (ADR 0118): tell the unit whether the strip is
    /// auditioning its key. `true` when the unit plays its own detector's input then (the
    /// compressor: its key after level, high-pass and EQ, latency-aligned); `false` when it
    /// has none of its own, and the strip plays the key as it arrives instead.
    #[inline]
    pub fn set_hear_key(&mut self, on: bool) -> bool {
        match &mut self.dsp {
            Dsp::Comp(c) => {
                c.set_hear_key(on);
                true
            }
            _ => false,
        }
    }

    /// r40:pads — whether the unit is playing its detector's input for *Hear the key*.
    #[inline]
    pub fn hearing_key(&self) -> bool {
        matches!(&self.dsp, Dsp::Comp(c) if c.hears_key())
    }

    /// Process with an external key for the effects that take one (a compressor's
    /// sidechain); the others ignore it. A hosted unit first hears the pass's timed
    /// parameter changes (ADR 0057).
    #[inline]
    pub fn process_keyed(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        match &mut self.dsp {
            Dsp::Delay(d) => d.process(l, r),
            Dsp::Reverb(v) => v.process(l, r),
            Dsp::Eq(e) => e.process_keyed(l, r, key),
            Dsp::Comp(c) => c.process(l, r, key),
            Dsp::Fx(u) => {
                self.automation
                    .deliver_all(|offset, id, value| u.automate(offset, id, value));
                u.process(l, r, key)
            }
        }
    }

    /// Hosted changes that found no room (its queue, the plug-in's event lists) since the
    /// last time this was asked.
    #[inline]
    pub fn take_dropped(&mut self) -> u64 {
        let own = self.automation.take_dropped();
        match &mut self.dsp {
            Dsp::Fx(u) => own + u.take_dropped(),
            _ => own,
        }
    }

    /// The engine's automation was planned again (ADR 0057): a hosted unit lets go of
    /// what it is no longer sent.
    #[inline]
    pub fn automation_replanned(&mut self) {
        self.automation.clear();
        if let Dsp::Fx(u) = &mut self.dsp {
            u.automation_replanned();
        }
    }
}
