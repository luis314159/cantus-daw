//! Stems through the master chain (ADR 0026), an option of a stems export.
//!
//! **The decision.** By default a stem does *not* pass through the master's inserts, as in
//! most DAWs: a stem is a part of the mix, taken where it leaves its strip, so the stems
//! of the strips feeding the master still add up to the master's input exactly — which is
//! what a mix engineer receiving them needs. A mastering chain is non-linear (a limiter on
//! the sum is not the sum of limiters), so no stem "through the master" can add up to the
//! mastered mix anyway.
//!
//! **The option** (`StemOptions::through_master`) is what Ableton's *include master
//! effects* gives: each stem as it would sound soloed into the master — through a fresh
//! instance of every master insert (built by the same `make_unit` as the pass's, so a test
//! can stand a pure delay in), with each slot's bypass, mix and the chain switch, and
//! trimmed by the chain's latency so sample 0 is still the song's first tick. The master's
//! fader, balance and width are not applied (no stem takes the master strip). A master
//! insert keyed from a strip hears no key here: a stem is heard alone.

use crate::effects::EffectUnit;
use cantus_core::{EffectSlot, Snapshot};

/// One stem's own copy of the master chain.
pub(crate) struct Through {
    units: Vec<(EffectUnit, bool, f32)>,
    on: bool,
    l: Vec<f32>,
    r: Vec<f32>,
    dry_l: Vec<f32>,
    dry_r: Vec<f32>,
    /// Each latent insert's dry line, so its mix and bypass meet a dry signal in step.
    lines: Vec<Option<crate::pdc::DelayLine>>,
}

impl Through {
    /// A chain for one stem, or `None` when the master has no insert to pass through.
    pub(crate) fn new(
        snapshot: &Snapshot,
        sample_rate: f32,
        block: usize,
        make_unit: &mut dyn FnMut(&EffectSlot, f32) -> EffectUnit,
    ) -> Option<Box<Through>> {
        Self::of_chain(
            &snapshot.master_effects,
            snapshot.master_fx_enabled,
            sample_rate,
            block,
            make_unit,
        )
    }

    /// A copy of `chain` (switched on or off as a whole by `on`), or `None` when it is
    /// empty — the mastering assistant's trial runs (round 34, ADR 0084).
    pub(crate) fn of_chain(
        chain: &[EffectSlot],
        on: bool,
        sample_rate: f32,
        block: usize,
        make_unit: &mut dyn FnMut(&EffectSlot, f32) -> EffectUnit,
    ) -> Option<Box<Through>> {
        if chain.is_empty() {
            return None;
        }
        let units: Vec<(EffectUnit, bool, f32)> = chain
            .iter()
            .map(|slot| {
                let mut u = make_unit(slot, sample_rate);
                u.configure(&slot.spec);
                (u, slot.bypass, slot.mix.clamp(0.0, 1.0))
            })
            .collect();
        let lines = units
            .iter()
            .map(|(u, _, _)| {
                let n = u.latency();
                (n > 0).then(|| {
                    let mut line = crate::pdc::lines_for(1, n).pop().expect("one line");
                    line.assign(crate::pdc::Edge::MasterDry(0), n);
                    line
                })
            })
            .collect();
        Some(Box::new(Through {
            units,
            on,
            l: vec![0.0; block],
            r: vec![0.0; block],
            dry_l: vec![0.0; block],
            dry_r: vec![0.0; block],
            lines,
        }))
    }

    /// The chain's latency: what the stem must be trimmed by, besides its own.
    pub(crate) fn latency(&self) -> usize {
        self.units.iter().map(|(u, _, _)| u.latency()).sum()
    }

    /// Pass one block of the stem through; the result is in [`Self::out`].
    pub(crate) fn run(&mut self, l: &[f32], r: &[f32]) {
        let n = l.len();
        self.l[..n].copy_from_slice(l);
        self.r[..n].copy_from_slice(r);
        let (bl, br) = (&mut self.l[..n], &mut self.r[..n]);
        for (i, (u, bypass, mix)) in self.units.iter_mut().enumerate() {
            let off = *bypass || !self.on;
            let latent = u.latency() > 0;
            if off && !latent {
                continue;
            }
            if *mix >= 1.0 && !latent {
                u.process(bl, br);
                continue;
            }
            let (dl, dr) = (&mut self.dry_l[..n], &mut self.dry_r[..n]);
            dl.copy_from_slice(bl);
            dr.copy_from_slice(br);
            if let Some(line) = self.lines[i].as_mut() {
                line.run(dl, dr);
            }
            u.process(bl, br);
            if off {
                bl.copy_from_slice(dl);
                br.copy_from_slice(dr);
            } else if *mix < 1.0 {
                let dry = 1.0 - *mix;
                for k in 0..n {
                    bl[k] = bl[k] * *mix + dl[k] * dry;
                    br[k] = br[k] * *mix + dr[k] * dry;
                }
            }
        }
    }

    /// The last block, through the chain.
    pub(crate) fn out(&self, n: usize) -> (&[f32], &[f32]) {
        (&self.l[..n], &self.r[..n])
    }
}
