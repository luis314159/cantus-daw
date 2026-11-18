//! One output's delay line (`o{n}.delay`, ADR 0123 §3; fix-e): a line that never forgets.
//!
//! **It behaves as a line fed every sample.** A change of delay never clears it: while it
//! holds sound the read head **glides** to the new delay (an exponential approach of
//! [`DELAY_GLIDE_MS`], at most half a sample per sample, so it never reverses — a tape's
//! pitch bend, never a step), reading between samples while it moves; once it arrives it
//! reads whole samples again, so **a static delay gives exactly the samples it always
//! gave**. While everything its head can reach is silence the delay is taken at once (the
//! glide would only move over zeros). A line its owner does not run because nothing comes
//! in is told so ([`OutDelay::skip`]): it moves on as if fed zeros, so what it plays never
//! depends on how a block was cut.
//!
//! **At 0 it passes its input through to the bit.** A line exists only when its owner was
//! built for it (the output's delay is set, or a lane automates it: ADR 0123 §3's rule,
//! `sampler_outs::delay_mask`); an output with none has no line at all.

/// The longest `o{n}.delay`, ms.
pub const MAX_DELAY_MS: f32 = 50.0;
/// The time constant of a delay's glide when it changes while the line sounds, ms.
pub const DELAY_GLIDE_MS: f32 = 30.0;
/// Fraction bits of the delay as the line holds it (1/65 536 of a sample).
const FRAC: u32 = 16;
const ONE: i64 = 1 << FRAC;
/// The fastest glide: half a sample per sample (the head never runs backwards).
const MAX_SLEW: i64 = ONE / 2;
/// The slowest step of the glide's tail (so it arrives exactly): 1/1 024 sample.
const MIN_SLEW: i64 = ONE / 1024;

/// One output's delay line: preallocated for [`MAX_DELAY_MS`] at the unit's rate.
#[derive(Clone, Debug)]
pub struct OutDelay {
    buf: Vec<f32>,
    cap: usize,
    /// Where the next frame is written.
    pos: usize,
    /// The delay now and where it glides to, in 1/65 536 samples.
    cur: i64,
    target: i64,
    /// The glide's time constant, frames.
    tau: i64,
    /// How many of the newest frames written are silence (both sides exactly zero),
    /// up to the whole line.
    zeros: usize,
}

impl OutDelay {
    /// Allocates: with the unit, never on the audio thread. The line starts silent, at 0.
    pub fn new(fs: f32) -> OutDelay {
        // The longest delay in whole samples, and one more frame the interpolation reads.
        let cap = (MAX_DELAY_MS * fs / 1000.0).ceil() as usize + 2;
        OutDelay {
            buf: vec![0.0; cap * 2],
            cap,
            pos: 0,
            cur: 0,
            target: 0,
            tau: ((DELAY_GLIDE_MS * fs / 1000.0) as i64).max(1),
            zeros: cap,
        }
    }

    /// Set the delay in ms (clamped to the line), whole samples. Nothing is cleared: a
    /// line that sounds glides there; a silent one is there at once.
    pub fn set_ms(&mut self, ms: f32, fs: f32) {
        let f = ((ms.max(0.0) * fs / 1000.0).round() as usize).min(self.cap - 2);
        self.target = (f as i64) << FRAC;
        if self.cur != self.target && self.quiet_for(self.cur) && self.quiet_for(self.target) {
            self.cur = self.target;
        }
    }

    /// The delay it is set to, in whole samples.
    pub fn frames(&self) -> usize {
        (self.target >> FRAC) as usize
    }

    /// Whether the head is still on its way to the delay set.
    pub fn gliding(&self) -> bool {
        self.cur != self.target
    }

    /// Whether everything a head at `d` reads (its sample and the one after) is silence.
    fn quiet_for(&self, d: i64) -> bool {
        self.zeros > (d >> FRAC) as usize + 1
    }

    /// Whether the line still holds anything: sound anywhere in it (a delay moved later
    /// may reach it), or a glide on its way. A line that does not ring is all silence at
    /// its delay, so a block its owner never runs (at rest) changes nothing it will play.
    pub fn ringing(&self) -> bool {
        self.zeros < self.cap || self.cur != self.target
    }

    /// fx/final — the head at the delay set, at once (the playhead jumped); what the line
    /// holds is kept.
    pub fn snap(&mut self) {
        self.cur = self.target;
    }

    /// Forget everything (a reset): silent, at its delay.
    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.zeros = self.cap;
        self.cur = self.target;
    }

    /// One step of the glide.
    #[inline]
    fn glide(&mut self) {
        let d = self.target - self.cur;
        if d == 0 {
            return;
        }
        let mut step = d / self.tau;
        if step == 0 {
            step = d.signum() * d.abs().min(MIN_SLEW);
        }
        self.cur += step.clamp(-MAX_SLEW, MAX_SLEW);
    }

    /// Delay a lane in place.
    pub fn run(&mut self, l: &mut [f32], r: &mut [f32]) {
        let cap = self.cap;
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            self.glide();
            let (x, y) = (*a, *b);
            self.buf[2 * self.pos] = x;
            self.buf[2 * self.pos + 1] = y;
            self.zeros = if x == 0.0 && y == 0.0 {
                (self.zeros + 1).min(cap)
            } else {
                0
            };
            let i = (self.cur >> FRAC) as usize;
            let frac = self.cur & (ONE - 1);
            let at = (self.pos + cap - i) % cap;
            let (mut ol, mut or) = (self.buf[2 * at], self.buf[2 * at + 1]);
            if frac != 0 {
                // Between this sample and the one before it (one frame older).
                let older = (at + cap - 1) % cap;
                let t = frac as f32 / ONE as f32;
                ol += (self.buf[2 * older] - ol) * t;
                or += (self.buf[2 * older + 1] - or) * t;
            }
            *a = ol;
            *b = or;
            self.pos = (self.pos + 1) % cap;
        }
    }

    /// `n` frames of silence in, while the line gives nothing ([`Self::ringing`] false):
    /// exactly what [`Self::run`] would do with zeros, without writing what is already
    /// silence.
    pub fn skip(&mut self, n: usize) {
        let cap = self.cap;
        if self.zeros < cap {
            let m = n.min(cap);
            let first = m.min(cap - self.pos);
            self.buf[2 * self.pos..2 * (self.pos + first)].fill(0.0);
            self.buf[..2 * (m - first)].fill(0.0);
            self.zeros = (self.zeros + n).min(cap);
        }
        self.pos = (self.pos + n % cap) % cap;
        // The glide moves on, frame by frame (integer steps: the same however cut).
        let mut k = 0;
        while k < n && self.cur != self.target {
            self.glide();
            k += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    /// A 200 Hz tone at 0.5 from `from` on (silence before), `n` frames.
    fn tone(n: usize, from: usize) -> Vec<f32> {
        (0..n)
            .map(|t| {
                if t < from {
                    0.0
                } else {
                    let ph = (t as f32 / FS * 200.0).fract();
                    0.5 * crate::math::sin(ph * std::f32::consts::TAU)
                }
            })
            .collect()
    }

    /// The largest step from one sample to the next.
    fn max_step(x: &[f32]) -> f32 {
        x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
    }

    /// The tone through a line whose delay is set to `ms(t)` every 16 frames (an
    /// automation lane's grid), in chunks of `chunk`.
    fn through(input: &[f32], chunk: usize, ms: &dyn Fn(usize) -> f32) -> Vec<f32> {
        let mut d = OutDelay::new(FS);
        let mut out = Vec::with_capacity(input.len());
        let mut t = 0;
        while t < input.len() {
            if t.is_multiple_of(16) {
                d.set_ms(ms(t), FS);
            }
            let c = chunk.min(16 - t % 16).min(input.len() - t);
            let (mut l, mut r) = (input[t..t + c].to_vec(), input[t..t + c].to_vec());
            d.run(&mut l, &mut r);
            out.extend_from_slice(&l);
            t += c;
        }
        out
    }

    #[test]
    fn an_automated_ramp_glides_with_no_silent_gap_and_no_step() {
        let n = 48_000;
        let input = tone(n, 0);
        // 0 → 20 ms over the second, then a jump back to 5 ms (a knob let go).
        let ramp = |t: usize| {
            if t < 40_000 {
                20.0 * t as f32 / 40_000.0
            } else {
                5.0
            }
        };
        for chunk in [16, 7, 1] {
            let out = through(&input, chunk, &ramp);
            // A cleared line would drop to silence and jump back (a step of 0.5).
            let step = max_step(&out);
            assert!(step < 0.03, "step {step} at chunk {chunk}");
            // Never a silent stretch longer than the tone's own zero crossing.
            let mut run = 0;
            for x in &out[1..] {
                run = if x.abs() < 1e-3 { run + 1 } else { 0 };
                assert!(run < 8, "a silent gap at chunk {chunk}");
            }
        }
        assert_eq!(through(&input, 16, &ramp), through(&input, 5, &ramp));
    }

    #[test]
    fn a_static_delay_shifts_by_whole_samples_to_the_bit_and_zero_passes_through() {
        let input = tone(6_000, 100);
        for (ms, f) in [(0.0f32, 0usize), (2.5, 120), (50.0, 2_400)] {
            let out = through(&input, 64, &|_| ms);
            for t in 0..input.len() {
                let want = if t >= f { input[t - f] } else { 0.0 };
                assert_eq!(out[t].to_bits(), want.to_bits(), "{ms} ms at {t}");
            }
        }
    }

    #[test]
    fn skipping_silence_is_feeding_it_whatever_the_cut() {
        // A burst, silence (skipped while the line gives nothing), the delay moved during
        // the silence, then another burst.
        let mut input = tone(20_000, 0);
        input[3_000..12_000].iter_mut().for_each(|x| *x = 0.0);
        let ms = |t: usize| if t < 8_000 { 4.0 } else { 9.0 };
        let fed = through(&input, 16, &ms);
        for chunk in [16usize, 11, 3] {
            let mut d = OutDelay::new(FS);
            let mut out = Vec::new();
            let mut t = 0;
            while t < input.len() {
                if t.is_multiple_of(16) {
                    d.set_ms(ms(t), FS);
                }
                let c = chunk.min(16 - t % 16).min(input.len() - t);
                let silent = input[t..t + c].iter().all(|x| *x == 0.0);
                if silent && !d.ringing() {
                    d.skip(c);
                    out.extend(std::iter::repeat_n(0.0, c));
                } else {
                    let (mut l, mut r) = (input[t..t + c].to_vec(), input[t..t + c].to_vec());
                    d.run(&mut l, &mut r);
                    out.extend_from_slice(&l);
                }
                t += c;
            }
            let a: Vec<u32> = fed.iter().map(|x| x.to_bits()).collect();
            let b: Vec<u32> = out.iter().map(|x| x.to_bits()).collect();
            assert_eq!(a, b, "chunk {chunk}");
        }
    }
}
