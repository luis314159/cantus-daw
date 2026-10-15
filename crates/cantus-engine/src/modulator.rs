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

/// Timed changes one hosted unit holds at most for a pass: 64 grid points of a full pass
/// for 32 parameters. More in one pass are dropped and counted.
pub const HOSTED_QUEUE: usize = 2048;

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

    /// A change `offset` samples into the pass (offsets arrive in order).
    #[inline]
    pub fn push(&mut self, offset: u32, id: u32, value: HostedValue) {
        if self.items.len() < self.items.capacity() {
            self.items.push(Timed { offset, id, value });
        } else {
            self.dropped += 1;
        }
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
        }
    }
}
