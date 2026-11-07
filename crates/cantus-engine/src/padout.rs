//! Instruments with several outputs on the audio thread (Pulsus III, ADR 0100).
//!
//! A track whose instrument plays into more than its own strip has, in the snapshot, one
//! **tap** per extra bus (`cantus_core::padout`): a track node of its own that plays nothing
//! but what its source renders into it. Here is the glue:
//!
//! - the **source** renders its buses in one sweep (`Pulsus::process_buses`, or a native
//!   unit's [`crate::inst::InstrumentUnit::process_buses`]): bus 0 into its own buffer, as
//!   before; bus `k` into its tap's [`Outs`] buffer, at the offset of the segment it is
//!   rendering. The source runs before its taps (the plan orders them: a tap waits for its
//!   source), so the tap's cell is free;
//! - the source also hands its taps its level, balance, audibility and their ramps each pass
//!   ([`mirror`]): the channel's own level is every output's;
//! - the **tap** reads its buffer as its source ([`Outs::take`]) — silence when its source
//!   wrote nothing this pass — and goes on through the level, the line and the strip every
//!   track goes through.
//!
//! Nothing here allocates: a tap's buffer is the slot's own, built with the slot.

use crate::graph::PASS_FRAMES;

/// The most outputs a track's instrument has besides its own: a kit's every pad.
pub const MAX_BUSES: usize = cantus_dsp::MAX_PADS;
/// No tap on this bus.
const NONE: u16 = u16::MAX;

/// A track slot's outputs: the taps of its buses (when it is a source), or its bus buffer
/// (when it is a tap).
#[derive(Clone)]
pub(crate) struct Outs {
    /// As a source: the slot of the tap of bus `k + 1`, or [`NONE`].
    taps: [u16; MAX_BUSES],
    /// As a source: how many buses it has.
    pub count: usize,
    /// As a tap: its source's slot.
    pub source: Option<usize>,
    /// As a tap: it plays into a pad's chain, whose output joins the source's stem in its
    /// place (the chain is the instrument's sound, ADR 0100 §9). // r37:fixes
    pub into_chain: bool,
    /// As a tap: what the source rendered into its bus this pass, and the pass it is of.
    l: Vec<f32>,
    r: Vec<f32>,
    pass: u64,
}

impl Outs {
    pub fn new() -> Self {
        Outs {
            taps: [NONE; MAX_BUSES],
            count: 0,
            source: None,
            into_chain: false,
            l: vec![0.0; PASS_FRAMES],
            r: vec![0.0; PASS_FRAMES],
            pass: u64::MAX,
        }
    }

    /// Forget every bus and source (the snapshot sets them again).
    pub fn clear(&mut self) {
        self.taps = [NONE; MAX_BUSES];
        self.count = 0;
        self.source = None;
        self.into_chain = false;
        self.pass = u64::MAX;
    }

    /// As a source: bus `bus` (1 and up) plays through tap slot `slot`.
    pub fn set_tap(&mut self, bus: usize, slot: usize) {
        if (1..=MAX_BUSES).contains(&bus) {
            self.taps[bus - 1] = slot.min(NONE as usize - 1) as u16;
            self.count = self.count.max(bus);
        }
    }

    /// As a source: the tap slot of bus `bus`.
    pub fn tap(&self, bus: usize) -> Option<usize> {
        let t = *self.taps.get(bus.checked_sub(1)?)?;
        (t != NONE).then_some(t as usize)
    }

    /// As a tap: add `l`/`r` at `at` frames into the pass `pass` — the first write of a pass
    /// starts from +0.0 everywhere.
    pub fn feed(&mut self, pass: u64, at: usize, l: &[f32], r: &[f32]) {
        if self.pass != pass {
            self.l.iter_mut().for_each(|x| *x = 0.0);
            self.r.iter_mut().for_each(|x| *x = 0.0);
            self.pass = pass;
        }
        let n = l.len().min(r.len()).min(PASS_FRAMES.saturating_sub(at));
        for (o, x) in self.l[at..at + n].iter_mut().zip(&l[..n]) {
            *o += *x;
        }
        for (o, x) in self.r[at..at + n].iter_mut().zip(&r[..n]) {
            *o += *x;
        }
    }

    /// As a tap: its source for `[sub, sub + l.len())` of pass `pass` into `l`/`r`;
    /// `false` (and nothing written) when its source wrote nothing this pass.
    pub fn take(&self, pass: u64, sub: usize, l: &mut [f32], r: &mut [f32]) -> bool {
        if self.pass != pass {
            return false;
        }
        let n = l.len().min(r.len()).min(PASS_FRAMES.saturating_sub(sub));
        l[..n].copy_from_slice(&self.l[sub..sub + n]);
        r[..n].copy_from_slice(&self.r[sub..sub + n]);
        true
    }
}

impl Default for Outs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_hears_only_the_pass_it_was_fed_in() {
        let mut o = Outs::new();
        o.feed(7, 10, &[1.0, 2.0], &[3.0, 4.0]);
        o.feed(7, 11, &[0.5], &[0.5]);
        let (mut l, mut r) = ([9.0f32; 4], [9.0f32; 4]);
        assert!(o.take(7, 9, &mut l, &mut r));
        assert_eq!(l, [0.0, 1.0, 2.5, 0.0]);
        assert_eq!(r, [0.0, 3.0, 4.5, 0.0]);
        assert!(!o.take(8, 0, &mut l, &mut r), "nothing fed this pass");
        o.feed(8, 0, &[-0.0], &[-0.0]);
        assert!(o.take(8, 0, &mut l[..1], &mut r[..1]));
        assert_eq!(l[0].to_bits(), 0.0f32.to_bits(), "+0.0 plus −0.0 is +0.0");
    }

    #[test]
    fn buses_map_to_their_taps() {
        let mut o = Outs::new();
        assert_eq!(o.tap(1), None);
        o.set_tap(1, 40);
        o.set_tap(3, 41);
        assert_eq!(
            (o.tap(1), o.tap(2), o.tap(3), o.tap(0)),
            (Some(40), None, Some(41), None)
        );
        assert_eq!(o.count, 3);
        o.clear();
        assert_eq!((o.tap(1), o.count), (None, 0));
    }
}
