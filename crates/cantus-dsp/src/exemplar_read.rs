//! Reading a [`Playhead`]'s sample: taps through the loop mapping, the interpolator at 2×,
//! and the half-band decimator. The hot path of the sampler.

use super::*;
use crate::math;
use std::f32::consts::FRAC_PI_2;

/// Eight lanes over contiguous slices: what the compiler turns into vector code.
#[inline]
fn lanes8(c: &[f32], s: &[f32]) -> [f32; 8] {
    let mut acc = [0.0f32; 8];
    for (cc, ss) in c.chunks_exact(8).zip(s.chunks_exact(8)) {
        for j in 0..8 {
            acc[j] += cc[j] * ss[j];
        }
    }
    acc
}

#[inline]
fn dot(c: &[f32], s: &[f32]) -> f32 {
    let n = c.len().min(s.len());
    let n8 = n / 8 * 8;
    let acc = lanes8(&c[..n8], &s[..n8]);
    let mut sum = (acc[0] + acc[4]) + (acc[1] + acc[5]) + (acc[2] + acc[6]) + (acc[3] + acc[7]);
    for j in n8..n {
        sum += c[j] * s[j];
    }
    sum
}

/// Stereo: `c2` holds every coefficient twice (c0 c0 c1 c1 …) against interleaved
/// frames (l0 r0 l1 r1 …), so the even lanes sum the left and the odd ones the right.
#[inline]
fn dot_lr(c2: &[f32], s: &[f32]) -> [f32; 2] {
    let n = c2.len().min(s.len());
    let n8 = n / 8 * 8;
    let acc = lanes8(&c2[..n8], &s[..n8]);
    let mut l = (acc[0] + acc[4]) + (acc[2] + acc[6]);
    let mut r = (acc[1] + acc[5]) + (acc[3] + acc[7]);
    for j in (n8..n).step_by(2) {
        l += c2[j] * s[j];
        r += c2[j + 1] * s[j + 1];
    }
    [l, r]
}

/// The interpolator's dot product over `taps` contiguous frames.
#[inline]
fn taps_dot(c: &[f32], d: &[f32], mono: bool) -> [f32; 2] {
    if mono {
        let v = dot(c, d);
        [v, v]
    } else {
        let mut c2 = [0.0f32; 2 * MAX_TAPS];
        for (j, x) in c.iter().enumerate() {
            c2[2 * j] = *x;
            c2[2 * j + 1] = *x;
        }
        dot_lr(&c2[..2 * c.len()], d)
    }
}

impl Playhead {
    /// Frame `j` of the current level, silence outside the file and past the region's end.
    #[inline]
    fn phys(&self, j: i64, out: &mut [f32; 2]) {
        let l = &self.lvl;
        if j < 0 || j >= l.end.min(l.frames) {
            *out = [0.0; 2];
            return;
        }
        *out = match self.slice(j as usize, 1) {
            Some(&[v]) => [v, v],
            Some(&[a, b]) => [a, b],
            _ => {
                // A streamed frame the window does not hold (a read wider than it): from
                // the pool, or silence and a miss.
                let mut f = [0.0f32; 2];
                if self.src.stream.is_some() && self.pool_read(j as usize, 1, &mut f) {
                    if self.src.channels == 1 {
                        [f[0], f[0]]
                    } else {
                        f
                    }
                } else {
                    [0.0; 2]
                }
            }
        };
    }

    /// Frame `b` of the loop, crossfaded with the audio one loop earlier near its end.
    #[inline]
    fn xfaded(&self, b: i64, on: bool, out: &mut [f32; 2]) {
        let l = &self.lvl;
        if on && l.xf > 0 && b > l.le - l.xf {
            let g = (b - (l.le - l.xf)) as f32 / l.xf as f32;
            let (wa, wb) = if self.region.xf_power {
                (math::cos(g * FRAC_PI_2), math::sin(g * FRAC_PI_2))
            } else {
                (1.0 - g, g)
            };
            let mut a = [0.0; 2];
            let mut c = [0.0; 2];
            self.phys(b, &mut a);
            self.phys(b - l.len, &mut c);
            *out = [wa * a[0] + wb * c[0], wa * a[1] + wb * c[1]];
        } else {
            self.phys(b, out);
        }
    }

    /// The looped signal at unwrapped index `i` of the current level.
    #[inline]
    pub(super) fn fetch(&self, i: i64, out: &mut [f32; 2]) {
        let l = &self.lvl;
        let r = &self.region;
        if r.reverse {
            return self.phys(l.end - 1 - (i - l.start), out);
        }
        if !r.looping {
            return self.phys(i, out);
        }
        let rl = self.release_lap;
        if i <= l.le {
            return self.xfaded(i, rl > 0, out);
        }
        let o = i - l.le - 1;
        match r.dir {
            LoopDir::Forward => {
                let lap = o / l.len + 1;
                if lap >= rl {
                    self.phys(i - rl * l.len, out)
                } else {
                    self.xfaded(i - lap * l.len, true, out)
                }
            }
            LoopDir::Backward => self.phys(l.le - o % l.len, out),
            LoopDir::PingPong => {
                let p = (l.len - 1).max(1);
                let lap = o / p + 1;
                let t = o % p;
                self.phys(
                    if lap % 2 == 1 {
                        l.le - 1 - t
                    } else {
                        l.ls + 1 + t
                    },
                    out,
                )
            }
        }
    }

    /// When the `n` taps from `ws` are `n` consecutive frames of the file, where they start.
    #[inline]
    fn window_base(&self, ws: i64, n: i64) -> Option<i64> {
        let l = &self.lvl;
        let r = &self.region;
        let lim = l.end.min(l.frames);
        let ok = |b: i64| (b >= 0 && b + n <= lim).then_some(b);
        if r.reverse {
            return None;
        }
        if !r.looping {
            return ok(ws);
        }
        let we = ws + n - 1;
        let rl = self.release_lap;
        let fade_top = if l.xf > 0 { l.le - l.xf } else { l.le };
        let plain_top = if rl > 0 { fade_top } else { l.le };
        if we <= plain_top {
            return ok(ws);
        }
        if ws <= l.le {
            return if rl == 0 { ok(ws) } else { None };
        }
        if r.dir != LoopDir::Forward {
            return None;
        }
        let lap_a = (ws - l.le - 1) / l.len + 1;
        if lap_a >= rl {
            return ok(ws - rl * l.len);
        }
        let lap_b = (we - l.le - 1) / l.len + 1;
        if lap_a != lap_b {
            return None;
        }
        let b = ws - lap_a * l.len;
        if b + n - 1 <= fade_top {
            ok(b)
        } else {
            None
        }
    }

    /// The band-limited value at level position `q`.
    #[inline]
    pub(super) fn interp(&mut self, q: f64, k: &Kernel, coefs: &mut [f32; MAX_TAPS]) -> [f32; 2] {
        let fl = q.floor();
        let frac = q - fl;
        let n = k.taps as i64;
        let ws = fl as i64 - (n / 2 - 1);
        let reach = (ws + n) << self.level;
        if reach > self.furthest {
            self.furthest = reach;
        }
        k.coefs(frac, coefs);
        let c = &coefs[..k.taps];
        let base = self.window_base(ws, n);
        if let Some(d) = base.and_then(|b| self.slice(b as usize, k.taps)) {
            return taps_dot(c, d, self.src.channels == 1);
        }
        // A streamed window the voice's own does not hold (a read wider than it): copied
        // from the pool, so the same dot product sums the same numbers.
        if let (Some(b), true) = (base, self.src.stream.is_some()) {
            let mut far = [0.0f32; 2 * MAX_TAPS];
            if self.pool_read(b as usize, k.taps, &mut far) {
                let ch = if self.src.channels == 1 { 1 } else { 2 };
                return taps_dot(c, &far[..k.taps * ch], ch == 1);
            }
        }
        {
            let mut acc = [0.0f32; 2];
            let mut t = [0.0f32; 2];
            for (j, cj) in c.iter().enumerate() {
                self.fetch(ws + j as i64, &mut t);
                acc[0] += cj * t[0];
                acc[1] += cj * t[1];
            }
            acc
        }
    }

    /// The half-band decimator, polyphase: ½ × the centre value plus two plain dot
    /// products over contiguous slices (the symmetric taps split into halves).
    #[inline]
    fn decimate(&self, dec: &super::super::resample::Decimator, chans: usize) -> [f32; 2] {
        let m = dec.odd.len();
        let mut out = [0.0f32; 2];
        for (c, o) in out.iter_mut().enumerate().take(chans) {
            let w = &self.u2[c][self.p2..self.p2 + 2 * m];
            let (lo, hi) = w.split_at(m);
            *o = 0.5 * self.u1[c][self.p1] + dot(&dec.rev, lo) + dot(&dec.odd, hi);
        }
        if chans == 1 {
            out[1] = out[0];
        }
        out
    }

    /// One output frame at `ratio` source frames per output frame, then advance.
    #[inline]
    pub fn next(&mut self, ratio: f64, reader: &Reader, coefs: &mut [f32; MAX_TAPS]) -> [f32; 2] {
        if self.done {
            return [0.0; 2];
        }
        let out = match self.mode {
            Mode::Exact => {
                let i = self.pos as i64;
                if i > self.furthest {
                    self.furthest = i;
                }
                let mut v = [0.0; 2];
                self.fetch(i, &mut v);
                v
            }
            Mode::Linear => {
                let fl = self.pos.floor();
                let f = (self.pos - fl) as f32;
                let i = fl as i64;
                if i + 1 > self.furthest {
                    self.furthest = i + 1;
                }
                let (mut a, mut b) = ([0.0; 2], [0.0; 2]);
                self.fetch(i, &mut a);
                self.fetch(i + 1, &mut b);
                [a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1])]
            }
            Mode::Resampled => {
                let l = reader.dec.len;
                let d2 = (l / 2) as f64;
                let scale = (1u64 << self.level) as f64;
                let s = ratio / scale / 2.0;
                let pk = self.pos / scale;
                let u1 = self.interp(pk + (d2 - 1.0) * s, &reader.kernel, coefs);
                let u2 = self.interp(pk + d2 * s, &reader.kernel, coefs);
                self.push_pair(u1, u2, reader.dec.odd.len());
                self.decimate(&reader.dec, self.src.channels.clamp(1, 2))
            }
        };
        self.pos += if self.mode == Mode::Exact { 1.0 } else { ratio };
        self.check_done(ratio, reader);
        out
    }

    fn check_done(&mut self, ratio: f64, reader: &Reader) {
        let r = &self.region;
        let end = if !r.looping || r.reverse {
            r.end
        } else if r.dir == LoopDir::Forward && self.release_lap != i64::MAX {
            r.end
                .saturating_add(self.release_lap.saturating_mul(r.loop_len()))
        } else {
            return;
        };
        // Let the interpolator's and the decimator's delay drain past the end.
        let margin = match self.mode {
            Mode::Resampled => (reader.dec.len / 2 + reader.kernel.taps) as f64 * ratio,
            _ => 1.0,
        };
        if self.pos >= end as f64 + margin {
            self.done = true;
        }
    }
}
