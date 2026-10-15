//! Modulators on the audio thread (ADR 0057): each snapshot's modulator plans, the state
//! of the ones that cannot be known ahead, and the timed changes a hosted plug-in hears.
//!
//! **What is a function of time** (an LFO, a step sequence, a random source, an envelope
//! opened by the sequenced notes) is evaluated at each control point straight from
//! `cantus_core::modulator::eval`, so a render from bar 5 hears what a full render hears
//! there, and offline equals real time. **What follows audio or live notes** (a follower,
//! an envelope opened by a level or by a key pressed now) is stepped from control point to
//! control point: restarted when playback starts, carried from one snapshot to the next
//! (an edit while playing never drops a follower to zero), its value one control point
//! behind so a gain it drives ramps from the last point's value to this one's.

use crate::fx::HostedValue;
use cantus_core::modulator::compile::{ListenAt, ModPlan};
use cantus_core::modulator::eval::{output, Follower, LiveEnv, ModTime};
use cantus_core::modulator::{EnvTrigger, ModSource};
use cantus_core::ModulatorId;

/// Modulators whose live value the interface is told, per block.
pub const MOD_TELEMETRY: usize = 64;

/// What the modulators are doing (ADR 0057), once a block: each one's output times its
/// amount at the block's first control point — what a knob's ring and the Modulators
/// panel draw. A snapshot's first [`MOD_TELEMETRY`] modulators; `count` of them are live.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModTelemetry {
    pub count: usize,
    pub ids: [u64; MOD_TELEMETRY],
    pub values: [f32; MOD_TELEMETRY],
}

impl Default for ModTelemetry {
    fn default() -> Self {
        ModTelemetry {
            count: 0,
            ids: [0; MOD_TELEMETRY],
            values: [0.0; MOD_TELEMETRY],
        }
    }
}

impl ModTelemetry {
    /// The live value of modulator `id`, if it is reported.
    pub fn value(&self, id: ModulatorId) -> Option<f32> {
        self.ids[..self.count.min(MOD_TELEMETRY)]
            .iter()
            .position(|x| *x == id.0)
            .map(|i| self.values[i])
    }
}

/// Timed changes one hosted unit holds at most for a pass: 64 grid points of a full pass
/// for 128 parameters all moving (a still parameter queues nothing after its first value;
/// 128 is also how many parameters a VST3 block carries). More in one pass are dropped and
/// counted in `EngineStats::dropped_events`.
pub const HOSTED_QUEUE: usize = 8192;

/// One timed change for a hosted unit.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Timed {
    /// Samples into the pass.
    pub offset: u32,
    pub id: u32,
    pub value: HostedValue,
}

/// The timed changes a hosted unit hears in the next pass, in time order (ADR 0057): the
/// control phase fills it, the node hands each to the unit just before the call it falls
/// in, with its offset into that call. Fixed capacity: never allocates once made.
#[derive(Debug, Default)]
pub struct AutoQueue {
    items: Vec<Timed>,
    /// The first item not yet handed over.
    next: usize,
    /// Changes that did not fit, since the queue was made.
    pub dropped: u64,
}

impl AutoQueue {
    /// Room for `n` changes (0 for a unit that is never hosted).
    pub fn with_capacity(n: usize) -> Self {
        AutoQueue {
            items: Vec::with_capacity(n),
            next: 0,
            dropped: 0,
        }
    }

    /// A new pass: what the last one did not hand over is gone.
    #[inline]
    pub fn clear(&mut self) {
        self.items.clear();
        self.next = 0;
    }

    /// A change `offset` samples into the pass (offsets arrive in order); whether it found
    /// room (what did not is counted in `dropped`).
    #[inline]
    pub fn push(&mut self, offset: u32, id: u32, value: HostedValue) -> bool {
        if self.items.len() < self.items.capacity() {
            self.items.push(Timed { offset, id, value });
            true
        } else {
            self.dropped += 1;
            false
        }
    }

    /// What did not fit since the last time this was asked.
    #[inline]
    pub fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.dropped)
    }

    pub fn is_empty(&self) -> bool {
        self.next >= self.items.len()
    }

    /// Hand `f` every change in the call `[from, to)` of the pass, with its offset into
    /// the call.
    #[inline]
    pub fn deliver(&mut self, from: usize, to: usize, mut f: impl FnMut(usize, u32, HostedValue)) {
        while let Some(t) = self.items.get(self.next) {
            let at = t.offset as usize;
            if at >= to {
                break;
            }
            f(at.saturating_sub(from), t.id, t.value);
            self.next += 1;
        }
    }

    /// Hand `f` every change left (a unit called once for the whole pass).
    #[inline]
    pub fn deliver_all(&mut self, f: impl FnMut(usize, u32, HostedValue)) {
        self.deliver(0, usize::MAX, f);
    }
}

/// A modulator's state on the audio thread.
#[derive(Debug)]
pub(crate) struct ModRt {
    pub id: ModulatorId,
    /// What it heard of audio (a follower).
    pub follower: Follower,
    /// Its envelope driven by live notes or by an audio level.
    pub env: LiveEnv,
    /// Live notes held on its notes track.
    pub held: u32,
    /// The beat of the last live note-on (an LFO retriggered by notes).
    pub live_on: Option<f64>,
    /// The track slot whose live notes it hears (resolved at install).
    pub notes_slot: Option<usize>,
    /// Whose audio it hears (resolved at install).
    pub ear: Option<Ear>,
    /// Its stateful value at the last control point (what a gain ramps from).
    pub held_value: f32,
    /// Raw outputs at this control point and at the next grid point.
    pub now: f32,
    pub next: f32,
    /// Its amount (0…1) at this control point and the next, automation and nesting in.
    pub amount_now: f32,
    pub amount_next: f32,
    /// What an inner grid point of a pass reads (hosted targets): output and amount there.
    pub inner: f32,
    pub inner_amount: f32,
}

impl ModRt {
    pub fn new(plan: &ModPlan) -> Self {
        ModRt {
            id: plan.id,
            follower: Follower::default(),
            env: LiveEnv::default(),
            held: 0,
            live_on: None,
            notes_slot: None,
            ear: None,
            held_value: 0.0,
            now: 0.0,
            next: 0.0,
            amount_now: plan.amount,
            amount_next: plan.amount,
            inner: 0.0,
            inner_amount: plan.amount,
        }
    }

    /// Take over what an older runtime's twin carried (the same modulator, a new
    /// snapshot): what follows audio and live notes goes on where it was.
    pub fn carry(&mut self, old: &ModRt) {
        self.follower = old.follower;
        self.env = old.env;
        self.held = old.held;
        self.live_on = old.live_on;
        self.held_value = old.held_value;
    }

    /// Back to rest: playback starts.
    pub fn restart(&mut self) {
        self.follower = Follower::default();
        self.env.reset();
        self.held = 0;
        self.live_on = None;
        self.held_value = 0.0;
    }

    /// Whether its value is stepped state (it lags a control point and ramps).
    pub fn stateful(plan: &ModPlan) -> bool {
        plan.source.follows_audio()
    }

    /// Step what carries state over `dt` seconds; `peak` is the listened audio's peak over
    /// them (linear).
    pub fn step(&mut self, plan: &ModPlan, dt: f64, peak: f32) {
        match &plan.source {
            ModSource::Follower(f) => self.follower.step(peak, f.attack_ms, f.release_ms, dt),
            ModSource::Envelope(e) => {
                if let EnvTrigger::Audio { threshold_db, .. } = e.trigger {
                    let db = if peak > 0.0 && peak.is_finite() {
                        cantus_core::linear_to_db(peak)
                    } else {
                        -200.0
                    };
                    // Open above the threshold, closed 3 dB under it: no chatter.
                    if !self.env.gate && db > threshold_db {
                        self.env.set_gate(true, e, false);
                    } else if self.env.gate && db < threshold_db - 3.0 {
                        self.env.set_gate(false, e, false);
                    }
                }
                self.env.step(e, dt);
            }
            _ => {}
        }
    }

    /// The stateful part of its output now.
    pub fn state_value(&self, plan: &ModPlan) -> f32 {
        match &plan.source {
            ModSource::Follower(f) => self.follower.output(f.gain_db),
            ModSource::Envelope(_) => self.env.level,
            _ => 0.0,
        }
    }

    /// Its output at `t`: the function of time, or (for what holds state) the state; an
    /// envelope opened by notes is the louder of the sequenced notes' and the live ones'.
    pub fn value_at(&self, plan: &ModPlan, t: &ModTime) -> f32 {
        let f = output(&plan.source, t, plan.gates.as_deref(), self.live_on);
        match &plan.source {
            ModSource::Follower(_) => self.state_value(plan),
            ModSource::Envelope(e) => match e.trigger {
                EnvTrigger::Audio { .. } => self.env.level,
                EnvTrigger::Notes { .. } => f.max(self.env.level),
            },
            _ => f,
        }
    }

    /// A live note on its notes track, at beat `beat`.
    pub fn live_note(&mut self, plan: &ModPlan, on: bool, beat: f64) {
        if on {
            self.held += 1;
            self.live_on = Some(beat);
        } else {
            self.held = self.held.saturating_sub(1);
        }
        if let ModSource::Envelope(e) = &plan.source {
            if matches!(e.trigger, EnvTrigger::Notes { .. }) {
                self.env.set_gate(self.held > 0, e, on);
            }
        }
    }

    /// Every live note let go (a stop, a seek, a panic).
    pub fn live_release(&mut self, plan: &ModPlan) {
        self.held = 0;
        // A retrigger belongs to where it was played: after a wrap, a seek or a stop the
        // song is elsewhere.
        self.live_on = None;
        if let ModSource::Envelope(e) = &plan.source {
            if matches!(e.trigger, EnvTrigger::Notes { .. }) {
                self.env.set_gate(false, e, false);
            }
        }
    }
}

/// Where a plan listens, as the processor's slots see it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ear {
    Strip(usize),
    Master,
}

impl Ear {
    pub fn of(at: ListenAt, strip_to_slot: &[usize]) -> Option<Ear> {
        match at {
            ListenAt::Strip(i) => strip_to_slot.get(i).copied().map(Ear::Strip),
            ListenAt::Master => Some(Ear::Master),
            ListenAt::Silent => None,
        }
    }
}

/// Render `snapshot` offline over `opts`' range (no audio kept) and trace modulator `id`'s
/// output × amount at every control point (ADR 0057): what *Bake to automation* reads for a
/// source that follows audio. `units` and `hosted` as [`crate::render::render_with_instruments`].
pub fn trace(
    snapshot: std::sync::Arc<cantus_core::Snapshot>,
    opts: &crate::render::RenderOptions,
    mut units: Vec<crate::effects::EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
    id: ModulatorId,
) -> Vec<(f64, f32)> {
    use crate::messages::{EngineEvent, EngineMessage};
    let end = opts.end.unwrap_or(snapshot.content_end);
    let sr = f64::from(opts.sample_rate);
    let mut snap = (*snapshot).clone();
    snap.loop_region = None;
    snap.metronome = false;
    crate::samples::conform_audio(&mut snap, opts.sample_rate);
    let snap = std::sync::Arc::new(snap);
    let mut instruments = crate::inst::units_for(&snap, sr as f32, |_, _| false);
    for u in &mut instruments {
        u.unit.set_offline(true);
    }
    instruments.extend(hosted.into_iter().map(Box::new));
    let n_fx = snap.effect_slots().count();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(16 + n_fx + instruments.len());
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(64 + n_fx + instruments.len());
    let mut p = crate::processor::Processor::without_midi(sr, rx_engine, tx_engine);
    p.set_profiling(false);
    for slot in snap.effect_slots() {
        let unit = match units.iter().position(|u| u.id == slot.id) {
            Some(i) => units.swap_remove(i),
            None => crate::effects::EffectUnit::new(slot, sr as f32),
        };
        tx.push(EngineMessage::AddEffect(Box::new(unit))).ok();
    }
    for unit in instruments {
        tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    tx.push(EngineMessage::Automation(
        crate::automation::Runtime::build(&snap),
    ))
    .ok();
    let plan = crate::graph::compile(&snap);
    tx.push(EngineMessage::SetSnapshotPlanned(snap.clone(), plan))
        .ok();
    p.settle();
    while let Ok(ev) = rx.pop() {
        match ev {
            EngineEvent::PdcNeed { count, delay } => {
                tx.push(EngineMessage::PdcLines(crate::pdc::lines_for(count, delay)))
                    .ok();
                p.settle();
            }
            EngineEvent::EffectNeed { effect, need } => {
                let memory = crate::effects::EffectMemory::build(need, sr as f32);
                tx.push(EngineMessage::EffectMemory { effect, memory }).ok();
                p.settle();
            }
            _ => {}
        }
    }
    let frames = ((end - opts.start)
        .max(cantus_core::Ticks::ZERO)
        .to_seconds(snap.tempo_bpm)
        * sr)
        .ceil() as usize;
    // A control point every 16 samples at most, and the jumps.
    p.trace_modulator(Some(id), frames / 16 + 64);
    tx.push(EngineMessage::Seek(opts.start)).ok();
    tx.push(EngineMessage::Play).ok();
    let channels = opts.channels.max(1) as usize;
    let block = opts
        .block_frames
        .clamp(1, crate::messages::MAX_BLOCK_FRAMES);
    let mut buf = vec![0f32; block * channels];
    let mut done = 0;
    while done < frames {
        let n = block.min(frames - done);
        p.process(&mut buf[..n * channels], channels);
        done += n;
        while rx.pop().is_ok() {}
    }
    p.take_modulator_trace()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::modulator::{ModKind, Retrigger};

    #[test]
    fn a_live_note_let_go_by_a_wrap_or_a_stop_does_not_freeze_a_retriggered_lfo() {
        // Review 1: `live_on` outlived a wrap, a seek and a stop, so an LFO retriggered
        // by notes sat at its start phase until the song passed the old live note again.
        let ModSource::Lfo(mut l) = ModKind::Lfo.source() else {
            unreachable!()
        };
        l.retrigger = Retrigger::Note;
        let plan = ModPlan {
            id: ModulatorId(1),
            source: ModSource::Lfo(l),
            amount: 1.0,
            amount_lane: None,
            notes: None,
            gates: None,
            listen: None,
            routes: Vec::new(),
        };
        let mut rt = ModRt::new(&plan);
        rt.live_note(&plan, true, 15.0);
        rt.live_note(&plan, false, 15.5);
        // The loop wraps to beat 0: every live note is let go.
        rt.live_release(&plan);
        let at = |b: f64| rt.value_at(&plan, &ModTime::at_beats(b, 0.0, 120.0));
        assert_ne!(at(0.3), at(0.6), "the LFO moves after the wrap");
        // Even a stale beat ahead of the song is not a retrigger.
        let ahead = output(
            &plan.source,
            &ModTime::at_beats(0.3, 0.0, 120.0),
            None,
            Some(15.0),
        );
        let none = output(
            &plan.source,
            &ModTime::at_beats(0.3, 0.0, 120.0),
            None,
            None,
        );
        assert_eq!(ahead, none);
    }
}
