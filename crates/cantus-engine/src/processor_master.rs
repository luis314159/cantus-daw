//! The master's insert chain on the audio thread (ADR 0026): FL's master track with its
//! own slots. It runs on the summed mix, **before** the master's width, gain, balance and
//! polarity, the order a mastering chain expects.
//!
//! Everything a strip's chain does, it does: units come in through the incoming pool
//! (`EngineMessage::AddEffect`) and leave through the garbage path; bypass and the chain
//! switch are soft for latent inserts (ADR 0022, each latent insert has a dry line,
//! [`Edge::MasterDry`]); each slot has its dry/wet; a slot may be keyed from any strip (the
//! master is processed after every strip, so the key is that segment's) and hear a track's
//! notes; an open editor reads its telemetry and the analyser its input and output.
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
use crate::pdc::Edge;

/// The master's chain: the units, each slot's switches and routing, and a stereo buffer
/// to work in. Everything is sized at construction (ADR 0005).
#[allow(clippy::vec_box)]
pub(super) struct MasterChain {
    pub(super) effects: Vec<Box<EffectUnit>>,
    bypass: [bool; MAX_EFFECTS_PER_TRACK],
    mix: [f32; MAX_EFFECTS_PER_TRACK],
    key_id: [Option<StripId>; MAX_EFFECTS_PER_TRACK],
    key: [Option<usize>; MAX_EFFECTS_PER_TRACK],
    pub(super) midi_id: [Option<TrackId>; MAX_EFFECTS_PER_TRACK],
    fx_enabled: bool,
    /// Each insert's dry line (`Edge::MasterDry`), resolved once per plan.
    pub(super) dry_line: [Option<usize>; MAX_EFFECTS_PER_TRACK],
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    dry_l: Vec<f32>,
    dry_r: Vec<f32>,
    /// The deepest reduction of the chain's dynamics over the last block, dB.
    gr: f32,
}

impl MasterChain {
    pub(super) fn new() -> Self {
        MasterChain {
            effects: Vec::with_capacity(MAX_EFFECTS_PER_TRACK),
            bypass: [false; MAX_EFFECTS_PER_TRACK],
            mix: [1.0; MAX_EFFECTS_PER_TRACK],
            key_id: [None; MAX_EFFECTS_PER_TRACK],
            key: [None; MAX_EFFECTS_PER_TRACK],
            midi_id: [None; MAX_EFFECTS_PER_TRACK],
            fx_enabled: true,
            dry_line: [None; MAX_EFFECTS_PER_TRACK],
            buf_l: vec![0.0; MAX_BLOCK_FRAMES],
            buf_r: vec![0.0; MAX_BLOCK_FRAMES],
            dry_l: vec![0.0; MAX_BLOCK_FRAMES],
            dry_r: vec![0.0; MAX_BLOCK_FRAMES],
            gr: 0.0,
        }
    }

    /// The chain's latency: every insert's, bypassed or not (soft bypass keeps it).
    pub(super) fn latency(&self) -> usize {
        self.effects.iter().map(|e| e.latency()).sum()
    }

    /// Whether the chain has anything to do to the mix this segment: an insert that runs,
    /// or a latent one whose dry line must keep time.
    fn runs(&self) -> bool {
        self.effects
            .iter()
            .enumerate()
            .any(|(i, e)| (self.fx_enabled && !self.bypass[i]) || e.latency() > 0)
    }
}

impl Processor {
    /// Take the snapshot's master chain: the units it keeps (in its order, re-configured),
    /// new ones from the incoming pool, the old ones handed back; keys resolved to strip
    /// slots, which then keep their post-fader output. Runs after the strips' chains and
    /// keys are resolved, before unclaimed units are thrown away.
    pub(super) fn install_master(&mut self, snapshot: &Snapshot) {
        self.scratch_effects.clear();
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
                self.strips[k].is_key = true;
            }
        }
    }

    /// Plan the chain's part of delay compensation, given the master's arrival `master`
    /// (the latest path into it): each latent insert's dry line, and the notes for inserts
    /// that hear a track — due when the audio reaching the slot does. Returns the chain's
    /// latency, which the output's latency adds.
    pub(super) fn plan_master(
        &mut self,
        kept: &mut [bool; crate::pdc::POOL],
        master: usize,
    ) -> usize {
        let mut ahead = master as u64;
        for (i, e) in self.master.effects.iter().enumerate() {
            self.pdc.want(kept, Edge::MasterDry(i), e.latency());
            let track = self.master.midi_id[i]
                .and_then(|id| self.slots.iter().position(|s| s.id == Some(id)));
            if let Some(track) = track {
                if self.fx_listeners.len() < self.fx_listeners.capacity() {
                    self.fx_listeners.push(FxListener {
                        track,
                        effect: e.id,
                        delay: ahead,
                    });
                    self.listened[track] = true;
                }
            }
            ahead += e.latency() as u64;
        }
        self.master.latency()
    }

    /// Every unit on the master, for the messages that look for an effect by id (memory,
    /// a panic, a replaced plug-in).
    pub(super) fn master_units(&mut self) -> impl Iterator<Item = &mut Box<EffectUnit>> {
        self.master.effects.iter_mut()
    }
}

impl Processor {
    /// Run the master chain on the summed mix in `out` (interleaved, `n` frames, the
    /// segment `[done, seg_end)` of the block), in place. Called after every strip has
    /// added into `out` and before the master's width, gain, balance and polarity.
    ///
    /// A mono output carries the mix as `l + r`; the chain hears half of it on each side
    /// and gives back their sum, which at unity is the input to the bit.
    pub(super) fn run_master(
        &mut self,
        out: &mut [f32],
        channels: usize,
        n: usize,
        done: usize,
        seg_end: usize,
        position: &crate::fx::SongPosition,
    ) {
        if self.master.effects.is_empty() {
            self.master.gr = 0.0;
            return;
        }
        // The deepest reduction of this block: the first segment starts it afresh.
        if done == 0 {
            self.master.gr = 0.0;
        }
        if !self.master.runs() {
            return;
        }
        let m = &mut self.master;
        let l = &mut m.buf_l[..n];
        let r = &mut m.buf_r[..n];
        for i in 0..n {
            let frame = &out[i * channels..(i + 1) * channels];
            if channels > 1 {
                l[i] = frame[0];
                r[i] = frame[1];
            } else {
                l[i] = frame[0] * 0.5;
                r[i] = frame[0] * 0.5;
            }
        }
        let dry_l = &mut m.dry_l[..n];
        let dry_r = &mut m.dry_r[..n];
        for (i, e) in m.effects.iter_mut().enumerate() {
            self.prof.enter(crate::profile::Node::MasterInsert(i));
            let looked_at = self.analysed == Some(e.id);
            if looked_at {
                for k in 0..n {
                    self.ana_pre[k] = (l[k] + r[k]) * 0.5;
                }
            }
            // Notes from the track it listens to, due in this segment (ADR 0022).
            if !self.fx_notes.is_empty() {
                let seg0 = self.clock + done as u64;
                let seg1 = self.clock + seg_end as u64;
                let id = e.id;
                self.fx_notes.retain(|d| {
                    if d.effect == id && d.due < seg1 {
                        e.note(d.due.saturating_sub(seg0) as usize, d.note);
                        false
                    } else {
                        true
                    }
                });
            }
            // Bypassed or switched off: a wire when it has no latency, a soft bypass (its
            // dry line, the effect still running) when it has.
            let off = m.bypass[i] || !m.fx_enabled;
            let latent = e.latency() > 0;
            if off && !latent {
                if looked_at {
                    Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
                }
                continue;
            }
            let mix = m.mix[i];
            let key = m.key[i].map(|k| {
                if self.key_valid[k] {
                    (&self.key_bufs[k].0[..n], &self.key_bufs[k].1[..n])
                } else {
                    (&self.silence[..n], &self.silence[..n])
                }
            });
            e.set_position(position);
            if mix >= 1.0 && !latent {
                e.process_keyed(l, r, key);
            } else {
                dry_l.copy_from_slice(l);
                dry_r.copy_from_slice(r);
                if let Some(li) = m.dry_line[i] {
                    self.pdc.lines[li].run(dry_l, dry_r);
                }
                e.process_keyed(l, r, key);
                if off {
                    l.copy_from_slice(dry_l);
                    r.copy_from_slice(dry_r);
                } else if mix < 1.0 {
                    let dry = 1.0 - mix;
                    for k in 0..n {
                        l[k] = l[k] * mix + dry_l[k] * dry;
                        r[k] = r[k] * mix + dry_r[k] * dry;
                    }
                }
            }
            let effect = e.id;
            e.drain_param_events(|event| {
                if self
                    .tx
                    .push(EngineEvent::FxParam { effect, event })
                    .is_err()
                {
                    self.stats.dropped_events += 1;
                }
            });
            if looked_at {
                Self::analyse(&mut self.analyser, &self.ana_pre[..n], l, r);
            }
            if off {
                continue;
            }
            if let Some(gr) = e.gain_reduction_db() {
                m.gr = m.gr.max(gr);
            }
            if looked_at {
                if let Some(d) = e.eq_deltas() {
                    self.meters.eq_deltas = d;
                }
                if let Some(c) = e.comp_meter() {
                    self.meters.comp = [c.input, c.output, c.gr_db];
                }
                if let Some(t) = e.telemetry() {
                    self.meters.fx = t;
                }
            }
        }
        self.prof.enter(crate::profile::Node::Engine);
        for i in 0..n {
            let frame = &mut out[i * channels..(i + 1) * channels];
            if channels > 1 {
                frame[0] = l[i];
                frame[1] = r[i];
            } else {
                frame[0] = l[i] + r[i];
            }
        }
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
