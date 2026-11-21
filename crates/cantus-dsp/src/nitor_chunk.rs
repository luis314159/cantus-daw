//! Nitor's audio, a grid cell at a time.
//!
//! A **cell** is [`GRID`] samples counted from the start of the stream. What a cell runs
//! (which paths, whether the level is fed) is decided at its first sample ([`Cell`]), and
//! what is cleared or switched happens at its last ([`Nitor::end_cell`]) — so a block that
//! ends inside a cell changes nothing, and the output does not depend on the block size.
//! Inside a cell everything runs sample by sample.
//!
//! **The dynamics wait for sound.** Multiplex's upward compression acts on silence (it
//! lifts what is under its threshold), so it never comes to rest by itself. After
//! [`PARK_S`] of exact silence, its output exactly zero, it is *parked*: reset, blended out
//! at once (silence: nothing to hear), and blended back in over [`super::GLIDE_MS`] from the
//! end of the first cell that carries sound. A new unit starts parked; so a song that
//! starts again after a silence is what a new unit makes of it.

use super::low::lift_gains;
use super::run::GRID;
use super::{excite, Nitor};

/// Exact silence this long parks the dynamics, seconds.
pub const PARK_S: f32 = 0.5;

/// What a cell runs, decided at its first sample.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub(super) struct Cell {
    pub busy: bool,
    pub dyn_running: bool,
    pub matching: bool,
    pub pre_on: bool,
    pub use_level: bool,
    pub low_band: bool,
    /// So far in the cell: the input was exact silence; the dynamics' output was.
    pub silent: bool,
    pub mp_quiet: bool,
}

impl Nitor {
    pub(super) fn latch(&self) -> Cell {
        let dyn_running = !self.ramps.dynamics.idle();
        let colouring = !self.ramps.idle();
        let matching = self.target.level_match && colouring;
        let pre_on = dyn_running;
        Cell {
            busy: colouring || self.level_live,
            dyn_running,
            matching,
            pre_on,
            use_level: self.level_live || matching || pre_on,
            low_band: self.ramps.low_band(),
            silent: true,
            mp_quiet: true,
        }
    }

    /// The end of a cell: what landed on zero is cleared, the dynamics parked or woken,
    /// the level let go.
    pub(super) fn end_cell(&mut self) {
        let c = self.cell;
        if c.dyn_running {
            if self.ramps.dynamics.idle() {
                // Blended out: fresh for when it comes back.
                self.reset_dynamics();
            } else {
                self.zero_run = if c.silent {
                    self.zero_run.saturating_add(GRID as u64)
                } else {
                    0
                };
                if self.zero_run >= self.park_after && c.mp_quiet {
                    self.reset_dynamics();
                    self.ramps.dynamics.park();
                    self.parked = true;
                    self.zero_run = 0;
                }
            }
        }
        if self.parked && !c.silent {
            self.parked = false;
            self.ramps.dynamics.to(1.0);
        }
        if !self.ramps.sub.idle() && self.target.sub_mode == super::SubMode::Octave {
            self.low.settle_sub();
        }
        self.clear_idle();
        if c.use_level {
            self.level_live = true;
            if !c.matching && !c.pre_on && self.level.at_unity() {
                self.level.clear();
                self.level_live = false;
            }
        }
    }

    /// One chunk inside the current cell.
    pub(super) fn chunk(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len()).min(GRID);
        let mut dl = [0.0f32; GRID];
        let mut dr = [0.0f32; GRID];
        dl[..n].copy_from_slice(&l[..n]);
        dr[..n].copy_from_slice(&r[..n]);
        let c = self.cell;
        if c.silent && !silent(&dl[..n], &dr[..n]) {
            self.cell.silent = false;
        }

        // The input's loudness, and the pre-gain sample by sample.
        let mut coef = [0.0f64; GRID];
        let mut pre = [1.0f32; GRID];
        if c.use_level {
            for j in 0..n {
                coef[j] = self.level.input(dl[j], dr[j], c.pre_on);
                pre[j] = self.level.gains().0;
            }
        } else {
            self.level.skip(n);
        }

        // The character's dynamics, brought to the reference level and back, blended.
        if c.dyn_running {
            for j in 0..n {
                l[j] = dl[j] * pre[j];
                r[j] = dr[j] * pre[j];
            }
            self.dynamics.process(&mut l[..n], &mut r[..n], None);
            if self.cell.mp_quiet && !silent(&l[..n], &r[..n]) {
                self.cell.mp_quiet = false;
            }
            for j in 0..n {
                let d = self.ramps.dynamics.next();
                let (wl, wr) = (l[j] / pre[j], r[j] / pre[j]);
                (l[j], r[j]) = if d == 1.0 {
                    (wl, wr)
                } else if d == 0.0 {
                    (dl[j], dr[j])
                } else {
                    (dl[j] + d * (wl - dl[j]), dr[j] + d * (wr - dr[j]))
                };
            }
        } else {
            self.dynamics.skip(n);
        }

        let mode = self.target.sub_mode;
        let mut m = self.meter_parts();
        for j in 0..n {
            let (xl, xr) = (dl[j], dr[j]);
            let (mut yl, mut yr) = (l[j], r[j]);
            let lb = if c.low_band {
                self.low.clock();
                self.low.band(xl, xr)
            } else {
                self.low.skip(1);
                0.0
            };
            if !self.ramps.tight.idle() {
                let t = self.ramps.tight.next();
                let g = self.low.tight_gain(lb, t);
                yl = self.low.tighten(0, yl);
                yr = self.low.tighten(1, yr);
                m.tight = m.tight.min(g);
            }
            if !self.ramps.lift.idle() {
                let v = self.ramps.lift.next();
                if v != self.lift_cache.0 {
                    let (a1, a2) = lift_gains(v);
                    self.lift_cache = (v, a1, a2);
                }
                let (_, a1, a2) = self.lift_cache;
                yl = self.low.lift(0, yl, a1, a2);
                yr = self.low.lift(1, yr, a1, a2);
            }
            let mut added = false;
            let (mut al, mut ar) = (0.0f32, 0.0f32);
            if !self.ramps.sub.idle() {
                let v = self.ramps.sub.next();
                let s = v * self.low.sub(lb, mode);
                al += s;
                ar += s;
                m.sub = m.sub.max(s.abs());
                added = true;
            }
            if !self.ramps.harm.idle() {
                let v = self.ramps.harm.next();
                let h = v * self.low.harm(lb);
                al += h;
                ar += h;
                m.harm = m.harm.max(h.abs());
                added = true;
            }
            for b in 0..excite::BANDS {
                let even = self.ramps.even[b].next();
                if !self.ramps.excite[b].idle() {
                    let v = self.ramps.excite[b].next();
                    let el = v * self.exciter.tick(0, b, xl, even);
                    let er = v * self.exciter.tick(1, b, xr, even);
                    al += el;
                    ar += er;
                    m.excite[b] = m.excite[b].max(el.abs().max(er.abs()));
                    added = true;
                }
            }
            if added {
                yl += al;
                yr += ar;
            }
            let gm = self.level.gains().1;
            if c.use_level {
                self.level.wet(yl, yr, coef[j], c.matching);
            }
            if gm != 1.0 {
                yl *= gm;
                yr *= gm;
            }
            let mix = self.mix.next();
            if mix != 1.0 {
                if mix == 0.0 {
                    (yl, yr) = (xl, xr);
                } else {
                    yl = xl + mix * (yl - xl);
                    yr = xr + mix * (yr - xr);
                }
            }
            let g = self.out.next();
            if g != 1.0 {
                yl *= g;
                yr *= g;
            }
            l[j] = yl;
            r[j] = yr;
        }
        self.meter_store(m);
    }

    /// A cell with nothing to colour: the output gain only (none at 0 dB: untouched).
    pub(super) fn idle_chunk(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        if self.cell.silent && !silent(&l[..n], &r[..n]) {
            self.cell.silent = false;
        }
        self.dynamics.skip(n);
        self.low.skip(n);
        self.level.skip(n);
        if self.out.landed() && self.out.v == 1.0 {
            return;
        }
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let g = self.out.next();
            *a *= g;
            *b *= g;
        }
    }
}

/// Every sample +0.0 (as the engine gathers silence).
#[inline]
fn silent(l: &[f32], r: &[f32]) -> bool {
    l.iter().chain(r).all(|v| v.to_bits() == 0)
}
