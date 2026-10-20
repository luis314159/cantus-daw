//! Low-latency monitoring (round 29, ADR 0064 §4; BACKLOG §4c step 2): while it is on, a
//! musician monitoring an input hears it without waiting for the project's latency.
//!
//! **The monitored path** is every strip an input that could be heard reaches — a track
//! with an input that monitors *On*, or *Auto* and armed (the rule that does not change at
//! the moment recording starts, so the plan does not move under the take) — through its
//! outputs and sends, and the master. On it, **an insert whose latency is above the limit is
//! bypassed hard** (`ll_off`: not run, no latency, no dry line — unlike ADR 0022's soft
//! bypass), and named on screen.
//!
//! **Constrained compensation** (Cubase's *Constrain Delay Compensation*): a strip that
//! carries monitored input only — the singer's own strip, a bus of monitored strips — is
//! not held back for the latency elsewhere: its output and its sends go out at once, and
//! the backing, aligned among itself, sets the master's arrival alone. A strip that mixes
//! the monitored input with backing keeps the backing aligned (the monitored input waits
//! there, as it is mixed with it): to hear yourself at the least delay, monitor through a
//! strip of your own. What is recorded is the raw input, so the take is placed as before:
//! by the backing's latency, captured when recording starts (`crate::recorder`).

use super::*;

/// Effects named in the stats as bypassed for monitoring, at most.
pub const MAX_LOW_LATENCY_OFF: usize = 8;

/// Low-latency monitoring as the engine last planned it (`EngineStats::low_latency`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LowLatency {
    /// The mode is on.
    pub on: bool,
    /// Something is monitored (an input that could be heard).
    pub monitoring: bool,
    /// Samples the monitored input waits inside the engine: its instrument's, its chain's
    /// and the master's — and, where a strip mixes it with backing, the backing's there.
    pub monitored_samples: u64,
    /// The inserts bypassed for it (effect ids; the first `off_count`).
    pub off: [u64; MAX_LOW_LATENCY_OFF],
    pub off_count: u8,
    /// How many are bypassed in all (the names stop at [`MAX_LOW_LATENCY_OFF`]).
    pub off_total: u16,
}

/// What [`Processor::monitor_plan`] decided for the planner.
pub(super) struct MonitorPlan {
    pub on: bool,
    /// Track slots whose input could be heard.
    pub track: [bool; MAX_TRACKS],
    /// Strip slots on the monitored path.
    pub path: [bool; MAX_STRIPS],
    /// Strip slots that carry monitored input only: not held back.
    pub pure: [bool; MAX_STRIPS],
}

impl Processor {
    /// Switch low-latency monitoring on with an insert limit of `limit` samples, or off.
    pub(super) fn set_low_latency(&mut self, limit: Option<u32>) {
        let limit = limit.map(|l| l as usize);
        if self.low_latency != limit {
            self.low_latency = limit;
            self.compensate();
        }
    }

    /// The monitored path and its bypasses (see the module's docs), the strips' and the
    /// master's `ll_off` flags set (all cleared when the mode is off). Allocation-free.
    pub(super) fn monitor_plan(&mut self, rest_s: &[bool; MAX_STRIPS]) -> MonitorPlan {
        let nodes = self.nodes.clone();
        let mut plan = MonitorPlan {
            on: self.low_latency.is_some(),
            track: [false; MAX_TRACKS],
            path: [false; MAX_STRIPS],
            pure: [false; MAX_STRIPS],
        };
        // Whether a strip gathers something that is not monitored input.
        let mut mixed = [false; MAX_STRIPS];
        let mut any = false;
        if plan.on {
            for &slot in &self.track_to_slot {
                let Some(s) = nodes.track(slot) else {
                    continue;
                };
                if s.rest {
                    continue;
                }
                let i = &s.input;
                // An audio input that could be heard, or an armed track with none: its
                // notes played live into its instrument (review 1).
                let heard = if i.has {
                    match i.monitor {
                        cantus_core::MonitorMode::On => true,
                        cantus_core::MonitorMode::Auto => i.armed,
                        cantus_core::MonitorMode::Off => false,
                    }
                } else {
                    i.armed
                };
                plan.track[slot] = heard;
                any |= heard;
                if let Some(d) = s.strip {
                    if heard {
                        plan.path[d] = true;
                    } else {
                        mixed[d] = true;
                    }
                }
            }
            for &slot in &self.strip_play_order {
                let Some(st) = nodes.strip(slot) else {
                    continue;
                };
                if rest_s[slot] {
                    continue;
                }
                plan.pure[slot] = plan.path[slot] && !mixed[slot];
                let dests = st
                    .output
                    .into_iter()
                    .chain(st.sends.iter().take(st.send_count).map(|(d, _)| *d));
                for d in dests {
                    if plan.path[slot] {
                        plan.path[d] = true;
                    }
                    if !plan.pure[slot] {
                        mixed[d] = true;
                    }
                }
            }
        }
        let limit = self.low_latency.unwrap_or(usize::MAX);
        let mut stats = LowLatency {
            on: plan.on,
            monitoring: any,
            ..LowLatency::default()
        };
        let name = |id: EffectId, stats: &mut LowLatency| {
            if (stats.off_count as usize) < MAX_LOW_LATENCY_OFF {
                stats.off[stats.off_count as usize] = id.0;
                stats.off_count += 1;
            }
            stats.off_total = stats.off_total.saturating_add(1);
        };
        for slot in 0..MAX_STRIPS {
            if self.strip_ids[slot].is_none() {
                continue;
            }
            if let Some(mut st) = nodes.strip(slot) {
                let on_path = any && plan.path[slot];
                let StripSlot {
                    effects, ll_off, ..
                } = &mut *st;
                for (i, e) in effects.iter_mut().enumerate() {
                    let was = ll_off[i];
                    ll_off[i] = on_path && e.latency() > limit;
                    if ll_off[i] {
                        name(e.id, &mut stats);
                    } else if was {
                        // Back from a hard bypass, it holds nothing from before it
                        // (review 1: a burst of old audio as the mode ended).
                        e.reset();
                    }
                }
                for f in ll_off.iter_mut().skip(effects.len()) {
                    *f = false;
                }
            }
        }
        // The master is on every monitored path.
        let m = &mut self.master;
        for (i, e) in m.effects.iter_mut().enumerate() {
            let was = m.ll_off[i];
            m.ll_off[i] = any && e.latency() > limit;
            if m.ll_off[i] {
                name(e.id, &mut stats);
            } else if was {
                e.reset();
            }
        }
        for f in m.ll_off.iter_mut().skip(m.effects.len()) {
            *f = false;
        }
        self.stats.low_latency = stats;
        plan
    }

    /// The monitored input's wait inside the engine, once the planner has the arrivals
    /// (review 1: it was guessed afterwards, wrong through a bus): where it enters each
    /// strip on its path and when it reaches the master, through the very delays planned —
    /// a track's instrument, each chain, each edge's line — plus the master's chain.
    pub(super) fn monitored_wait(&mut self, plan: &MonitorPlan, a: &Arrivals) {
        if !plan.on || !self.stats.low_latency.monitoring {
            self.stats.low_latency.monitored_samples = 0;
            return;
        }
        let nodes = self.nodes.clone();
        let mut at = [None::<usize>; MAX_STRIPS];
        let mut master: Option<usize> = None;
        let reach = |to: &mut Option<usize>, t: usize| *to = Some(to.map_or(t, |x| x.max(t)));
        for &slot in &self.track_to_slot {
            if !plan.track[slot] || a.rest_t[slot] {
                continue;
            }
            let feeds = a.feeds[slot];
            // Not held back (into a strip of monitored input only, or the master): it
            // leaves at its instrument's latency; else with what it is mixed with.
            let quick = feeds.is_none_or(|d| plan.pure[d]);
            match feeds {
                Some(d) if a.deaf[d] => {}
                Some(d) => reach(&mut at[d], if quick { a.leave[slot] } else { a.arrive[d] }),
                None => reach(&mut master, if quick { a.leave[slot] } else { a.master }),
            }
        }
        for &slot in &self.strip_play_order {
            if a.rest_s[slot] {
                continue;
            }
            let Some(enter) = at[slot] else {
                continue;
            };
            let Some(st) = nodes.strip(slot) else {
                continue;
            };
            let pure = plan.pure[slot];
            // Out of its chain, then its edge's line: `target - post` (as planned).
            let out = |target: usize| enter + a.own[slot] + target.saturating_sub(a.post[slot]);
            match st.output {
                Some(d) if a.deaf[d] => {}
                Some(d) if pure && plan.pure[d] => reach(&mut at[d], out(a.post[slot])),
                Some(d) => reach(&mut at[d], out(a.arrive[d])),
                None if pure => reach(&mut master, out(a.post[slot])),
                None => reach(&mut master, out(a.master)),
            }
            for (d, _) in st.sends.iter().take(st.send_count) {
                if a.deaf[*d] {
                    continue;
                }
                let target = if pure && plan.pure[*d] {
                    a.post[slot]
                } else {
                    a.arrive[*d]
                };
                reach(&mut at[*d], out(target));
            }
        }
        self.stats.low_latency.monitored_samples =
            (master.unwrap_or(0) + self.master.latency()) as u64;
    }
}

/// The planner's arrivals, for [`Processor::monitored_wait`].
pub(super) struct Arrivals<'a> {
    pub arrive: &'a [usize; MAX_STRIPS],
    pub post: &'a [usize; MAX_STRIPS],
    pub own: &'a [usize; MAX_STRIPS],
    pub master: usize,
    pub leave: &'a [usize; MAX_TRACKS],
    pub feeds: &'a [Option<usize>; MAX_TRACKS],
    pub deaf: &'a [bool; MAX_STRIPS],
    pub rest_s: &'a [bool; MAX_STRIPS],
    pub rest_t: &'a [bool; MAX_TRACKS],
}
