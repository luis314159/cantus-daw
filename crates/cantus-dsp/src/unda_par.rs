//! Unda's voices on several threads (ADR 0038; a child of `unda::synth`).
//!
//! The serial render walks the control grid piece by piece — the free LFOs and every
//! voice's control at a grid point, then every voice's piece into one sum, then the
//! decimator. Here a span of up to [`SPAN`] frames is cut into the same pieces first: the
//! free LFOs are ticked for each grid point up front (they depend on nothing a voice does),
//! then each voice runs all of the span's pieces — control at each grid point, render — into
//! **its own lane**, on whatever thread takes it; then, piece by piece and in voice order,
//! the lanes are summed exactly as the serial loop adds them, the decimator runs, and the
//! allocator hears of the voices that ended in the order the serial loop told it.

use super::{Globals, Unda, CONTROL, MAX_VOICES};
use crate::par::{add_into, clear_lane, run_each, Parallel};

/// The longest span one fork covers, in base frames (the lanes hold it at 4×).
pub(super) const SPAN: usize = CONTROL * 4;
/// The most grid pieces a span can have (one more when it starts off the grid).
const PIECES: usize = SPAN / CONTROL + 1;
/// A voice that sounded to the span's end.
const ON: usize = usize::MAX;

/// One piece of a span: where it starts (base frames into the span), its length, and the
/// free LFOs' values if it starts on the grid.
#[derive(Copy, Clone, Default)]
struct Piece {
    at: usize,
    len: usize,
    grid: Option<[f32; 8]>,
}

impl Unda {
    /// [`Self::process`] with the voices handed to `par` when it has helpers and two or
    /// more voices sound. The same bits as the serial render.
    pub fn process_par(&mut self, left: &mut [f32], right: &mut [f32], par: &dyn Parallel) -> bool {
        let sounding = self.voices.iter().filter(|v| v.active).nth(1).is_some();
        if par.width() < 2 || !sounding {
            return self.process(left, right);
        }
        let n = left.len().min(right.len());
        let q = self.s.oversample.clamp(1, 4);
        self.peak = [self.peak[0] * 0.9, self.peak[1] * 0.9];
        let mut wrote = false;
        let mut i = 0;
        while i < n {
            let m = (n - i).min(SPAN);
            wrote |= self.span_par(&mut left[i..i + m], &mut right[i..i + m], q, par);
            i += m;
        }
        wrote
    }

    fn span_par(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        q: usize,
        par: &dyn Parallel,
    ) -> bool {
        let m = left.len();
        // The pieces, and the free LFOs at each grid point (as `control_all` ticks them).
        let mut pieces = [Piece::default(); PIECES];
        let mut count = 0;
        let (mut at, mut clock) = (0usize, self.clock);
        while at < m && count < PIECES {
            let grid = clock.is_multiple_of(CONTROL as u64).then(|| {
                self.tick_free(CONTROL);
                self.free_val
            });
            let len = (CONTROL - (clock % CONTROL as u64) as usize).min(m - at);
            pieces[count] = Piece { at, len, grid };
            count += 1;
            at += len;
            clock += len as u64;
        }
        let pieces = &pieces[..count];
        // Every voice sounding now renders the whole span into its lane.
        for (v, used) in self.voices.iter().zip(self.used.iter_mut()) {
            *used = v.active;
        }
        {
            let s = &*self.s;
            let (sr, globals) = (self.sr, &self.globals);
            let lanes = self.lanes.split(m * q).zip(self.fin.iter_mut());
            let jobs = self.voices.iter_mut().zip(lanes).filter(|(v, _)| v.active);
            run_each(par, jobs, &|(v, ((ll, lr), fin))| {
                clear_lane(ll, lr);
                *fin = ON;
                for (k, p) in pieces.iter().enumerate() {
                    if let Some(free) = &p.grid {
                        let g = Globals {
                            sr,
                            oversample: s.oversample,
                            src: globals,
                            free_lfo: free,
                        };
                        v.control(s, &g, false);
                    }
                    let (a, b) = (p.at * q, (p.at + p.len) * q);
                    if !v.render(s, q, p.len, &mut ll[a..b], &mut lr[a..b]) {
                        *fin = k;
                        break;
                    }
                }
            });
        }
        // Piece by piece, as the serial loop: the sum in voice order, the decimator, the
        // ends told to the allocator.
        let mut wrote = false;
        for (k, p) in pieces.iter().enumerate() {
            let live = |vi: usize| self.used[vi] && self.fin[vi] >= k;
            let any = (0..MAX_VOICES).any(live);
            if !any && p.grid.is_some() && self.decim.peak() < super::GATE {
                self.decim.clear();
            }
            if any || self.decim.peak() > 0.0 {
                let (a, len) = (p.at * q, p.len * q);
                self.acc_l[..len].fill(0.0);
                self.acc_r[..len].fill(0.0);
                for vi in (0..MAX_VOICES).filter(|&vi| live(vi)) {
                    let (ll, lr) = self.lanes.lane(vi, a + len);
                    add_into(&mut self.acc_l[..len], &ll[a..]);
                    add_into(&mut self.acc_r[..len], &lr[a..]);
                }
                for j in 0..p.len {
                    let (l, r) = self
                        .decim
                        .tick(q, &self.acc_l[j * q..], &self.acc_r[j * q..]);
                    if l != 0.0 || r != 0.0 {
                        wrote = true;
                        left[p.at + j] += l;
                        right[p.at + j] += r;
                        self.peak[0] = self.peak[0].max(l.abs());
                        self.peak[1] = self.peak[1].max(r.abs());
                    }
                }
            }
            for vi in 0..MAX_VOICES {
                if self.used[vi] && self.fin[vi] == k {
                    self.alloc.voice_finished(vi);
                }
            }
            self.clock += p.len as u64;
        }
        wrote
    }
}
