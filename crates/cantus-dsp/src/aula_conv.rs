//! Aula's convolver (ADR 0112): zero latency, constant cost, the same bits however the
//! host cuts its blocks.
//!
//! - **The head** (the first 64 taps) runs in direct form, sample by sample.
//! - **The levels** ([`crate::aula_kernel`]) run in the frequency domain, overlap-save, with
//!   a delay line of the inputs' spectra each (uniform partitions within a level). Both
//!   inputs share one complex transform (left real, right imaginary) and both outputs one
//!   inverse.
//! - **Everything happens on a grid counted from the stream's start**, never from a host
//!   block: at each 64-sample boundary the head and level 0 take the next kernel if one
//!   waits, level 0 runs whole, and every other level runs its schedule's piece for that
//!   tick. Any split of the host's blocks gives the same bits.
//! - **A new response is swapped in whole**: the head and level 0 at the next boundary,
//!   every other level at the start of its next period, each period computed with one
//!   response throughout. The delay lines are the inputs', not the response's, so the tail
//!   of what was played is kept across the swap.
//! - **Nothing allocates** after [`State::new`] and [`crate::aula_kernel::Kernel::new`],
//!   both made off the audio thread.
//!
//! **At rest** (ADR 0031 §5.1): after [`Layout::settle`] samples of silence every ring,
//! delay line and work buffer holds only zeros. Some of those zeros are −0.0 (a transform
//! of +0.0 multiplies by negative twiddles); a signed zero changes no sum that has any other
//! term, so only the sign of an exact-zero result could differ from a new convolver's, and
//! every output is written as `y + 0.0`, which makes it +0.0.

use crate::aula_fft::Fft;
use crate::aula_kernel::{Format, KLevel, Kernel, Layout, Op, Unit, B, LEVELS, SIZES, STARTS};
use std::sync::Arc;

/// One level's running state.
#[derive(Clone, Debug)]
struct SLevel {
    fft: Fft,
    /// Partitions the delay line holds.
    slots: usize,
    bins: usize,
    /// The inputs' spectra: `[(input · slots + slot) · bins + bin]`.
    fdl_re: Vec<f32>,
    fdl_im: Vec<f32>,
    work_re: Vec<f32>,
    work_im: Vec<f32>,
    /// The outputs' spectra being summed: `[output · bins + bin]`.
    y_re: Vec<f32>,
    y_im: Vec<f32>,
    /// Computing this period with the waiting kernel.
    on_next: bool,
}

/// What a convolver of a capacity holds: rings, delay lines, work buffers. Made off the
/// audio thread; all zeros when new.
#[derive(Clone, Debug)]
pub struct State {
    cap: Layout,
    mask: usize,
    in_l: Vec<f32>,
    in_r: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    /// The last `B` inputs per side, twice over (a window without wrapping).
    hist: [[f32; 2 * B]; 2],
    levels: Vec<SLevel>,
}

impl State {
    /// Room for any response of layout `cap` or smaller. Allocates.
    pub fn new(cap: Layout) -> State {
        let ring = cap.ring();
        let levels = (0..cap.levels)
            .map(|k| {
                let l = SIZES[k];
                let bins = l + 1;
                let slots = cap.parts[k].max(1);
                SLevel {
                    fft: Fft::new(2 * l),
                    slots,
                    bins,
                    fdl_re: vec![0.0; 2 * slots * bins],
                    fdl_im: vec![0.0; 2 * slots * bins],
                    work_re: vec![0.0; 2 * l],
                    work_im: vec![0.0; 2 * l],
                    y_re: vec![0.0; 2 * bins],
                    y_im: vec![0.0; 2 * bins],
                    on_next: false,
                }
            })
            .collect();
        State {
            cap,
            mask: ring - 1,
            in_l: vec![0.0; ring],
            in_r: vec![0.0; ring],
            out_l: vec![0.0; ring],
            out_r: vec![0.0; ring],
            hist: [[0.0; 2 * B]; 2],
            levels,
        }
    }

    pub fn cap(&self) -> Layout {
        self.cap
    }

    /// Bytes it holds, for the memory report.
    pub fn bytes(&self) -> usize {
        let lv: usize = self
            .levels
            .iter()
            .map(|l| l.fdl_re.len() * 2 + l.work_re.len() * 2 + l.y_re.len() * 2 + l.fft.size() * 2)
            .sum();
        4 * (4 * self.in_l.len() + 4 * B + lv)
    }

    fn clear(&mut self) {
        for v in [
            &mut self.in_l,
            &mut self.in_r,
            &mut self.out_l,
            &mut self.out_r,
        ] {
            v.fill(0.0);
        }
        self.hist = [[0.0; 2 * B]; 2];
        for l in &mut self.levels {
            for v in [
                &mut l.fdl_re,
                &mut l.fdl_im,
                &mut l.work_re,
                &mut l.work_im,
                &mut l.y_re,
                &mut l.y_im,
            ] {
                v.fill(0.0);
            }
            l.on_next = false;
        }
    }
}

/// What [`Convolver::supply`] answers: the state it replaced, or what it could not use.
pub type Supplied = Result<Option<Box<State>>, (Arc<Kernel>, Option<Box<State>>)>;

/// The convolver: a state and the kernel(s) it runs. Its output is the wet signal alone.
#[derive(Clone, Debug)]
pub struct Convolver {
    state: Box<State>,
    active: Arc<Kernel>,
    /// A kernel being taken level by level.
    next: Option<Arc<Kernel>>,
    /// One that arrived while another was being taken (the newest wins).
    queued: Option<Arc<Kernel>>,
    head_next: bool,
    /// Samples since the stream's start (the grid).
    t: u64,
    /// Samples of silent input in a row.
    quiet: u64,
}

impl Convolver {
    /// A convolver over `state` running `kernel`, whose capacity must be the state's.
    pub fn new(state: Box<State>, kernel: Arc<Kernel>) -> Convolver {
        assert_eq!(
            state.cap,
            kernel.cap(),
            "a kernel made for another capacity"
        );
        Convolver {
            state,
            active: kernel,
            next: None,
            queued: None,
            head_next: false,
            t: 0,
            quiet: u64::MAX / 2,
        }
    }

    pub fn cap(&self) -> Layout {
        self.state.cap
    }

    /// The kernel it will be running once every swap is through: the newest it was given.
    pub fn kernel(&self) -> &Arc<Kernel> {
        self.queued
            .as_ref()
            .or(self.next.as_ref())
            .unwrap_or(&self.active)
    }

    /// Whether a swap is still under way.
    pub fn swapping(&self) -> bool {
        self.next.is_some()
    }

    /// Bytes of state (the kernels are counted where they are kept).
    pub fn state_bytes(&self) -> usize {
        self.state.bytes()
    }

    /// Run `kernel` from now on. With `state` (a new, empty one: the kernel needs more room
    /// than this one has), the swap is at once and the old state is handed back to be freed
    /// off the audio thread; without, the kernel must fit this state, and is taken level by
    /// level (at once when at rest). A kernel that fits neither is handed back unused.
    /// Kernels let go are only `Arc`s dropped: their owner keeps a clone, so the audio
    /// thread never frees one. No allocation.
    pub fn supply(&mut self, kernel: Arc<Kernel>, state: Option<Box<State>>) -> Supplied {
        match state {
            Some(s) if s.cap == kernel.cap() => {
                let old = std::mem::replace(&mut self.state, s);
                self.active = kernel;
                self.next = None;
                self.queued = None;
                self.head_next = false;
                self.quiet = u64::MAX / 2;
                Ok(Some(old))
            }
            None if kernel.cap() == self.state.cap => {
                if self.at_rest() {
                    self.active = kernel;
                } else if self.next.is_none() {
                    self.next = Some(kernel);
                } else {
                    self.queued = Some(kernel);
                }
                Ok(None)
            }
            other => Err((kernel, other)),
        }
    }

    /// At rest: everything zero, no swap waiting (exact, by counting).
    pub fn at_rest(&self) -> bool {
        self.next.is_none() && self.quiet >= self.state.cap.settle() as u64
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if self.next.is_some() {
            Some("an impulse response is being swapped in")
        } else if self.quiet < self.state.cap.settle() as u64 {
            Some("the convolution's tail is still sounding")
        } else {
            None
        }
    }

    /// `frames` of silence skipped while at rest: the grid moves on.
    pub fn skip(&mut self, frames: usize) {
        self.t += frames as u64;
    }

    /// Back to silence at the stream's start. Clears the state unless it is all zero
    /// already; any swap waiting is done at once.
    pub fn reset(&mut self) {
        if !self.at_rest() {
            self.state.clear();
        }
        if let Some(k) = self.queued.take().or(self.next.take()) {
            self.active = k;
        }
        self.head_next = false;
        for l in &mut self.state.levels {
            l.on_next = false;
        }
        self.t = 0;
        self.quiet = u64::MAX / 2;
    }

    /// Convolve a stereo block in place: what comes out is the wet signal.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        // The silence counter, from the end of what came in.
        match (0..n).rev().find(|&i| l[i] != 0.0 || r[i] != 0.0) {
            Some(i) => self.quiet = (n - 1 - i) as u64,
            None => self.quiet = self.quiet.saturating_add(n as u64),
        }
        let mut i = 0;
        while i < n {
            if self.t.is_multiple_of(B as u64) {
                self.boundary();
            }
            let run = (n - i).min(B - (self.t % B as u64) as usize);
            self.samples(&mut l[i..i + run], &mut r[i..i + run]);
            i += run;
        }
    }

    fn samples(&mut self, l: &mut [f32], r: &mut [f32]) {
        let kern: &Kernel = match (&self.next, self.head_next) {
            (Some(k), true) => k,
            _ => &self.active,
        };
        let st = &mut *self.state;
        let format = kern.format();
        let head = &kern.head;
        for (xl, xr) in l.iter_mut().zip(r.iter_mut()) {
            let p = (self.t as usize) & st.mask;
            st.in_l[p] = *xl;
            st.in_r[p] = *xr;
            let h = (self.t % B as u64) as usize;
            st.hist[0][h] = *xl;
            st.hist[0][h + B] = *xl;
            st.hist[1][h] = *xr;
            st.hist[1][h + B] = *xr;
            let wl = &st.hist[0][h + 1..h + 1 + B];
            let wr = &st.hist[1][h + 1..h + 1 + B];
            let dot = |path: usize, w: &[f32]| -> f32 {
                let taps = &head[path * B..path * B + B];
                let mut s = 0.0f32;
                for j in 0..B {
                    s += taps[j] * w[j];
                }
                s
            };
            let (yl, yr) = match format {
                Format::Mono => (dot(0, wl), dot(0, wr)),
                Format::Stereo => (dot(0, wl), dot(1, wr)),
                Format::TrueStereo => (dot(0, wl) + dot(2, wr), dot(1, wl) + dot(3, wr)),
            };
            let ol = std::mem::take(&mut st.out_l[p]);
            let or = std::mem::take(&mut st.out_r[p]);
            *xl = (yl + ol) + 0.0;
            *xr = (yr + or) + 0.0;
            self.t += 1;
        }
    }

    /// A 64-sample boundary: take a waiting kernel where a period starts, then run each
    /// level's piece of work for this tick.
    #[allow(clippy::needless_range_loop)]
    fn boundary(&mut self) {
        let t = self.t;
        let levels = self.state.cap.levels;
        if self.next.is_some() {
            self.head_next = true;
            for (k, lv) in self.state.levels.iter_mut().enumerate() {
                if k == 0 || t.is_multiple_of(SIZES[k] as u64) {
                    lv.on_next = true;
                }
            }
            if self.state.levels.iter().all(|l| l.on_next) {
                if let Some(k) = self.next.take() {
                    self.active = k;
                }
                self.head_next = false;
                for l in &mut self.state.levels {
                    l.on_next = false;
                }
                self.next = self.queued.take();
            }
        }
        let st = &mut *self.state;
        for k in 0..levels {
            let size = SIZES[k] as u64;
            let j = if k == 0 {
                0
            } else {
                ((t % size) / B as u64) as usize
            };
            let ps = t - j as u64 * B as u64;
            let lv = &mut st.levels[k];
            let kern: &Kernel = match (&self.next, lv.on_next) {
                (Some(n), true) => n,
                _ => &self.active,
            };
            let kl = &kern.levels[k];
            let rings = Rings {
                mask: st.mask,
                in_l: &st.in_l,
                in_r: &st.in_r,
                out_l: &mut st.out_l,
                out_r: &mut st.out_r,
            };
            run(lv, kl, kern.format(), k, ps, kl.sched.tick(j), rings);
        }
    }
}

struct Rings<'a> {
    mask: usize,
    in_l: &'a [f32],
    in_r: &'a [f32],
    out_l: &'a mut [f32],
    out_r: &'a mut [f32],
}

/// Level `k`'s `units` for the period that started at `ps` (its block `[ps − L, ps)` the
/// newest complete).
fn run(
    lv: &mut SLevel,
    kl: &KLevel,
    format: Format,
    k: usize,
    ps: u64,
    units: &[Unit],
    rings: Rings<'_>,
) {
    let l = SIZES[k];
    let n = 2 * l;
    let bins = lv.bins;
    let m = (ps / l as u64) as usize;
    let slots = lv.slots;
    let newest = m % slots;
    let Rings {
        mask,
        in_l,
        in_r,
        out_l,
        out_r,
    } = rings;
    for u in units {
        let (from, to) = (u.from as usize, u.to as usize);
        match u.op {
            Op::Gather => {
                let base = ps.wrapping_sub(n as u64) as usize;
                for i in from..to {
                    let src = base.wrapping_add(i) & mask;
                    let d = lv.fft.rev(i);
                    lv.work_re[d] = in_l[src];
                    lv.work_im[d] = in_r[src];
                }
            }
            Op::Fwd(s) => lv
                .fft
                .stage(&mut lv.work_re, &mut lv.work_im, s, from, to, false),
            Op::Inv(s) => lv
                .fft
                .stage(&mut lv.work_re, &mut lv.work_im, s, from, to, true),
            Op::Unpack => {
                let at0 = newest * bins;
                let at1 = (slots + newest) * bins;
                for b in from..to {
                    let (a, bb) = (lv.work_re[b], lv.work_im[b]);
                    let nb = (n - b) & (n - 1);
                    let (c, d) = (lv.work_re[nb], lv.work_im[nb]);
                    lv.fdl_re[at0 + b] = 0.5 * (a + c);
                    lv.fdl_im[at0 + b] = 0.5 * (bb - d);
                    lv.fdl_re[at1 + b] = 0.5 * (bb + d);
                    lv.fdl_im[at1 + b] = 0.5 * (c - a);
                }
            }
            Op::Mac(p) => mac(lv, kl, format, newest, p as usize, from, to),
            Op::Pack => {
                let (yl, yr) = (0, bins);
                for i in from..to {
                    let (zr, zi) = if i <= l {
                        let (a, b) = (lv.y_re[yl + i], lv.y_im[yl + i]);
                        let (c, d) = (lv.y_re[yr + i], lv.y_im[yr + i]);
                        (a - d, b + c)
                    } else {
                        let j = n - i;
                        let (a, b) = (lv.y_re[yl + j], lv.y_im[yl + j]);
                        let (c, d) = (lv.y_re[yr + j], lv.y_im[yr + j]);
                        (a + d, c - b)
                    };
                    let dst = lv.fft.rev(i);
                    lv.work_re[dst] = zr;
                    lv.work_im[dst] = zi;
                }
            }
            Op::Extract => {
                let base = (ps as usize).wrapping_sub(l).wrapping_add(STARTS[k]);
                for i in from..to {
                    let o = base.wrapping_add(i) & mask;
                    out_l[o] += lv.work_re[l + i];
                    out_r[o] += lv.work_im[l + i];
                }
            }
        }
    }
}

/// Partition `p`'s products over bins `from .. to`: the first partition sets the outputs'
/// spectra, the others add.
fn mac(
    lv: &mut SLevel,
    kl: &KLevel,
    format: Format,
    newest: usize,
    p: usize,
    from: usize,
    to: usize,
) {
    let bins = lv.bins;
    let slots = lv.slots;
    let slot = (newest + slots - p % slots) % slots;
    let x0 = slot * bins;
    let x1 = (slots + slot) * bins;
    let h = |path: usize| (path * kl.parts + p) * bins;
    let (h0, h1, h2, h3) = (h(0), h(1), h(2), h(3));
    let first = p == 0;
    for b in from..to {
        let (ar, ai) = (lv.fdl_re[x0 + b], lv.fdl_im[x0 + b]);
        let (br, bi) = (lv.fdl_re[x1 + b], lv.fdl_im[x1 + b]);
        let cm = |hr: f32, hi: f32, xr: f32, xi: f32| (xr * hr - xi * hi, xr * hi + xi * hr);
        let ((lr, li), (rr, ri)) = match format {
            Format::Mono => {
                let (hr, hi) = (kl.re[h0 + b], kl.im[h0 + b]);
                (cm(hr, hi, ar, ai), cm(hr, hi, br, bi))
            }
            Format::Stereo => (
                cm(kl.re[h0 + b], kl.im[h0 + b], ar, ai),
                cm(kl.re[h1 + b], kl.im[h1 + b], br, bi),
            ),
            Format::TrueStereo => {
                let ll = cm(kl.re[h0 + b], kl.im[h0 + b], ar, ai);
                let lr = cm(kl.re[h1 + b], kl.im[h1 + b], ar, ai);
                let rl = cm(kl.re[h2 + b], kl.im[h2 + b], br, bi);
                let rr = cm(kl.re[h3 + b], kl.im[h3 + b], br, bi);
                ((ll.0 + rl.0, ll.1 + rl.1), (lr.0 + rr.0, lr.1 + rr.1))
            }
        };
        if first {
            lv.y_re[b] = lr;
            lv.y_im[b] = li;
            lv.y_re[bins + b] = rr;
            lv.y_im[bins + b] = ri;
        } else {
            lv.y_re[b] += lr;
            lv.y_im[b] += li;
            lv.y_re[bins + b] += rr;
            lv.y_im[bins + b] += ri;
        }
    }
}

const _: () = assert!(LEVELS == SIZES.len());

#[cfg(test)]
#[path = "aula_conv_tests.rs"]
mod tests;
