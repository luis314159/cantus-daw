//! The **master node** (ADR 0031 §1.1, ADR 0026): it gathers what feeds the master in
//! canonical order — the tracks routed to it, then each strip's output edge in the order —
//! into the device's buffer, runs the master's insert chain on the sum, then the master
//! strip (width, gain, balance, polarity last), the master take and the click. It runs
//! whole passes; the click alone splits itself at its beats.
//!
//! **The chain** is FL's master track with its own slots, run on the summed mix **before**
//! the master's width, gain, balance and polarity, the order a mastering chain expects.
//! Everything a strip's chain does, it does: units come in through the incoming pool
//! (`EngineMessage::AddEffect`) and leave through the garbage path; bypass and the chain
//! switch are soft for latent inserts (ADR 0022, each latent insert has a dry line,
//! [`Edge::MasterDry`]); each slot has its dry/wet; a slot may be keyed from any strip (the
//! master runs after every strip, so the key is that pass's) and hear a track's notes; an
//! open editor reads its telemetry and the analyser its input and output.
//!
//! **Latency.** The chain delays every path equally, so it compensates nothing inside the
//! mix: it is added to the output's latency (`EngineStats::latency_samples`, the heard
//! position, the click's line, a strip take's line). A project with an empty chain plans
//! exactly what it planned before.
//!
//! **The null rule.** An empty chain, a chain switched off or a chain of bypassed inserts
//! with no latency never touches the mix: the section returns before reading a sample.
//! Otherwise the mix is lifted out of the interleaved output, processed, and written back;
//! lifting and writing back are copies, exact to the bit.

use super::*;
use crate::graph::notes::Listener;
use crate::pdc::Edge;

/// The master's chain: the units, each slot's switches and routing, and a stereo buffer
/// to work in. Everything is sized at construction (ADR 0005).
#[allow(clippy::vec_box)]
pub(super) struct MasterChain {
    pub(super) effects: Vec<Box<EffectUnit>>,
    pub(super) bypass: [bool; MAX_EFFECTS_PER_TRACK],
    pub(super) mix: [f32; MAX_EFFECTS_PER_TRACK],
    key_id: [Option<StripId>; MAX_EFFECTS_PER_TRACK],
    pub(super) key: [Option<usize>; MAX_EFFECTS_PER_TRACK],
    pub(super) midi_id: [Option<TrackId>; MAX_EFFECTS_PER_TRACK],
    /// Each insert's place in the processor's listeners, when it hears a track's notes.
    pub(super) listener: [Option<u16>; MAX_EFFECTS_PER_TRACK],
    pub(super) fx_enabled: bool,
    /// Each insert's dry line (`Edge::MasterDry`), resolved once per plan.
    pub(super) dry_line: [Option<usize>; MAX_EFFECTS_PER_TRACK],
    pub(super) buf_l: Vec<f32>,
    pub(super) buf_r: Vec<f32>,
    pub(super) dry_l: Vec<f32>,
    pub(super) dry_r: Vec<f32>,
    /// The deepest reduction of the chain's dynamics over the last block, dB.
    pub(super) gr: f32,
    /// Per insert: bypassed hard for low-latency monitoring (ADR 0064 §4).
    pub(super) ll_off: [bool; MAX_EFFECTS_PER_TRACK],
}

impl MasterChain {
    // r34:listen — a range render's checkpoint (ADR 0083).
    /// A copy with every unit's state; `None` when some unit's cannot be copied.
    pub(super) fn fork(&self) -> Option<MasterChain> {
        let mut effects = Vec::with_capacity(self.effects.capacity());
        for e in &self.effects {
            effects.push(Box::new(e.fork()?));
        }
        Some(MasterChain {
            effects,
            bypass: self.bypass,
            mix: self.mix,
            key_id: self.key_id,
            key: self.key,
            midi_id: self.midi_id,
            listener: self.listener,
            fx_enabled: self.fx_enabled,
            dry_line: self.dry_line,
            buf_l: self.buf_l.clone(),
            buf_r: self.buf_r.clone(),
            dry_l: self.dry_l.clone(),
            dry_r: self.dry_r.clone(),
            gr: self.gr,
            ll_off: self.ll_off,
        })
    }

    pub(super) fn new() -> Self {
        MasterChain {
            effects: Vec::with_capacity(MAX_EFFECTS_PER_TRACK),
            bypass: [false; MAX_EFFECTS_PER_TRACK],
            mix: [1.0; MAX_EFFECTS_PER_TRACK],
            key_id: [None; MAX_EFFECTS_PER_TRACK],
            key: [None; MAX_EFFECTS_PER_TRACK],
            midi_id: [None; MAX_EFFECTS_PER_TRACK],
            listener: [None; MAX_EFFECTS_PER_TRACK],
            fx_enabled: true,
            dry_line: [None; MAX_EFFECTS_PER_TRACK],
            buf_l: vec![0.0; PASS_FRAMES],
            buf_r: vec![0.0; PASS_FRAMES],
            dry_l: vec![0.0; PASS_FRAMES],
            dry_r: vec![0.0; PASS_FRAMES],
            gr: 0.0,
            ll_off: [false; MAX_EFFECTS_PER_TRACK],
        }
    }

    /// The chain's latency: every insert's, bypassed or not (soft bypass keeps it).
    pub(super) fn latency(&self) -> usize {
        self.effects
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.ll_off[*i])
            .map(|(_, e)| e.latency())
            .sum()
    }

    /// Whether the chain has anything to do to the mix this pass: an insert that runs, or
    /// a latent one whose dry line must keep time (not one bypassed hard for monitoring).
    pub(super) fn runs(&self) -> bool {
        self.effects.iter().enumerate().any(|(i, e)| {
            !self.ll_off[i] && ((self.fx_enabled && !self.bypass[i]) || e.latency() > 0)
        })
    }
}

/// The master strip's settings for a block, from the snapshot (`Processor::render`).
#[derive(Copy, Clone, Debug)]
pub(super) struct MasterStrip {
    pub gain: f32,
    pub width: f32,
    /// The balance's gains (`balance_gains(pan)`).
    pub l: f32,
    pub r: f32,
    pub sign: f32,
}

impl Processor {
    /// Take the snapshot's master chain: the units it keeps (in its order, re-configured),
    /// new ones from the incoming pool, the old ones handed back; keys resolved to strip
    /// slots, which then keep their post-fader output. Runs after the strips' chains and
    /// keys are resolved, before unclaimed units are thrown away.
    pub(super) fn install_master(&mut self, snapshot: &Snapshot) {
        self.scratch_effects.clear();
        // The hard bypass goes with its unit (review 2), as on a strip.
        let mut was_off = [(cantus_core::EffectId(u64::MAX), false); MAX_EFFECTS_PER_TRACK];
        for (i, e) in self
            .master
            .effects
            .iter()
            .enumerate()
            .take(MAX_EFFECTS_PER_TRACK)
        {
            was_off[i] = (e.id, self.master.ll_off[i]);
        }
        while let Some(e) = self.master.effects.pop() {
            self.scratch_effects.push(e);
        }
        for want in snapshot.master_effects.iter().take(MAX_EFFECTS_PER_TRACK) {
            let found = self
                .scratch_effects
                .iter()
                .position(|e| e.id == want.id)
                .map(|p| self.scratch_effects.swap_remove(p))
                .or_else(|| {
                    self.incoming_effects
                        .iter()
                        .position(|e| e.id == want.id)
                        .map(|p| self.incoming_effects.swap_remove(p))
                });
            match found {
                Some(mut unit) => {
                    unit.configure(&want.spec);
                    let m = &mut self.master;
                    let idx = m.effects.len();
                    m.bypass[idx] = want.bypass;
                    m.mix[idx] = want.mix.clamp(0.0, 1.0);
                    m.key_id[idx] = want.sidechain;
                    m.midi_id[idx] = want.midi_from;
                    m.ll_off[idx] = was_off.iter().any(|(id, off)| *off && *id == unit.id);
                    m.effects.push(unit);
                }
                // Not delivered yet (or dropped): skipped this snapshot, as on a strip.
                None => self.stats.dropped_events += 1,
            }
        }
        while let Some(e) = self.scratch_effects.pop() {
            self.dispose_effect(e);
        }
        self.master.fx_enabled = snapshot.master_fx_enabled;
        for i in 0..MAX_EFFECTS_PER_TRACK {
            let k = if i < self.master.effects.len() {
                self.master.key_id[i].and_then(|id| self.strip_slot_for(id))
            } else {
                None
            };
            self.master.key[i] = k;
            if let Some(k) = k {
                if let Some(mut s) = self.nodes.strip(k) {
                    s.is_key = true;
                }
            }
        }
    }

    /// Plan the chain's part of delay compensation, given the master's arrival `master`
    /// (the latest path into it): each latent insert's dry line, and the listeners for
    /// inserts that hear a track — whose notes wait until the audio reaching the slot does.
    /// Returns the chain's latency, which the output's latency adds.
    pub(super) fn plan_master(
        &mut self,
        kept: &mut [bool; crate::pdc::POOL],
        master: usize,
    ) -> usize {
        let mut ahead = master as u64;
        self.master.listener = [None; MAX_EFFECTS_PER_TRACK];
        for i in 0..self.master.effects.len() {
            let (effect, latency) = {
                let e = &self.master.effects[i];
                // Bypassed hard for monitoring: no latency, no dry line (ADR 0064 §4).
                (
                    e.id,
                    if self.master.ll_off[i] {
                        0
                    } else {
                        e.latency()
                    },
                )
            };
            self.pdc.want(kept, Edge::MasterDry(i), latency);
            let track = self.master.midi_id[i].and_then(|id| self.slot_for(id));
            if let Some(track) = track {
                self.master.listener[i] = self.listen(track, effect, ahead);
            }
            ahead += latency as u64;
        }
        self.master.latency()
    }

    /// An insert `effect` starts (or keeps) hearing track slot `track`'s notes, `delay`
    /// samples late: its place among the listeners. A listener that was already there
    /// keeps its cursor, so a new snapshot loses no note on its way.
    pub(super) fn listen(&mut self, track: usize, effect: EffectId, delay: u64) -> Option<u16> {
        if self.listener_count >= MAX_LISTENERS {
            return None;
        }
        let nodes = &*self.nodes;
        let written = nodes
            .read(&nodes.tracks[track].outbox)
            .map_or(0, |o| o.written());
        let cursor = self
            .listeners_prev
            .iter()
            .find(|l| l.effect == effect && l.track == track)
            .map_or(written, |l| l.cursor);
        let at = self.listener_count;
        *nodes.listener(at)? = Listener {
            track,
            effect,
            delay,
            cursor,
        };
        self.listener_count += 1;
        self.listened[track] = true;
        Some(at as u16)
    }

    /// Every unit on the master, for the messages that look for an effect by id (memory,
    /// a panic, a replaced plug-in).
    pub(super) fn master_units(&mut self) -> impl Iterator<Item = &mut Box<EffectUnit>> {
        self.master.effects.iter_mut()
    }

    /// The id of the master chain's insert at `index`, for the profiler's blame.
    pub(super) fn master_effect(&self, index: usize) -> Option<cantus_core::EffectId> {
        self.master.effects.get(index).map(|e| e.id)
    }

    /// The chain's reduction for the meters.
    pub(super) fn master_gr(&self) -> f32 {
        if self.master.fx_enabled {
            self.master.gr
        } else {
            0.0
        }
    }
}
