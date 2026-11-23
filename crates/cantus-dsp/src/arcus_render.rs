//! Arcus's render (a child of `arcus`): the voices of each chunk into the chunk's buffer —
//! each into its own lane on the pool's threads when it helps, the lanes summed in voice
//! order (ADR 0038) — then Fides's body and output stage; and what the editor is told.

use super::voice::Shared;
use super::{tel, Arcus, CHUNK, GHOSTS, MAX_VOICES};
use crate::par::{add_into, clear_lane, run_each, Parallel, Serial};

impl Arcus {
    /// Render added into `left`/`right`. Returns whether anything but zeros was written.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.render(left, right, &Serial)
    }

    /// [`Self::process`] with the voices handed to `par` (the same bits).
    pub fn process_par(&mut self, left: &mut [f32], right: &mut [f32], par: &dyn Parallel) -> bool {
        self.render(left, right, par)
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32], par: &dyn Parallel) -> bool {
        let n = left.len().min(right.len());
        let sounding = self.voices.iter().any(|v| v.active());
        let rad_still = self.rad.iter().all(|r| *r == (0.0, 0.0));
        if !sounding && rad_still && self.body.is_still() && self.out.is_still() {
            self.clock += n as u64;
            self.peak = 0.0;
            return false;
        }
        let started = std::time::Instant::now();
        let mut voice_frames = 0usize;
        let mut wrote = false;
        let mut peak = 0f32;
        let mut offset = 0;
        while offset < n {
            let frames = (n - offset).min(CHUNK);
            self.buf_l[..frames].fill(0.0);
            self.buf_r[..frames].fill(0.0);
            let any = self.voices.iter().any(|v| v.active());
            if any {
                let sh = Shared {
                    s: &self.s,
                    sr: self.sr,
                    ctl: &self.ctl,
                    tuning: &self.tuning,
                    hammer: &self.hammer,
                    hammer_area: self.hammer_area,
                    table: self.table,
                    clock: self.clock,
                };
                let bl = &mut self.buf_l[..frames];
                let br = &mut self.buf_r[..frames];
                let count = self.voices.iter().filter(|v| v.active()).count();
                voice_frames += count * frames;
                if par.width() > 1 && count >= 2 {
                    let lanes = self.lanes.split(frames).zip(self.used.iter_mut());
                    let jobs = self
                        .voices
                        .iter_mut()
                        .zip(lanes)
                        .filter(|(v, _)| v.active());
                    run_each(par, jobs, &|(v, ((ll, lr), used))| {
                        clear_lane(ll, lr);
                        v.render(&sh, ll, lr);
                        *used = true;
                    });
                    for k in 0..MAX_VOICES + GHOSTS {
                        if std::mem::take(&mut self.used[k]) {
                            let (ll, lr) = self.lanes.lane(k, frames);
                            add_into(bl, ll);
                            add_into(br, lr);
                        }
                    }
                } else {
                    for v in self.voices.iter_mut().filter(|v| v.active()) {
                        v.render(&sh, bl, br);
                    }
                }
            }
            for (i, v) in self.voices.iter_mut().enumerate() {
                if v.take_ended() && i < MAX_VOICES {
                    self.alloc.voice_finished(i);
                }
            }
            let quiet = !any && self.rad.iter().all(|r| *r == (0.0, 0.0));
            let out_quiet = quiet && self.body.is_still();
            let (bl, br) = (&mut self.buf_l[..frames], &mut self.buf_r[..frames]);
            // The radiation: a first-order high-pass `(1 + R)/2 · (1 − z⁻¹)/(1 − R z⁻¹)`
            // (unity at Nyquist), its state let go at exact zeros so the instance rests.
            if self.rad_c > 0.0 {
                let (r, k) = (self.rad_c, 0.5 * (1.0 + self.rad_c));
                // Two stages: 12 dB an octave under the corner.
                for stage in 0..4 {
                    let buf: &mut [f32] = if stage % 2 == 0 { &mut *bl } else { &mut *br };
                    let (mut x1, mut y1) = self.rad[stage];
                    if quiet && x1 == 0.0 && y1 == 0.0 {
                        continue;
                    }
                    for x in buf.iter_mut() {
                        let y = k * (*x - x1) + r * y1;
                        x1 = *x;
                        y1 = if y.abs() < 1e-20 && *x == 0.0 { 0.0 } else { y };
                        *x = y1;
                    }
                    self.rad[stage] = (x1, y1);
                }
            }
            self.body.process(bl, br, self.clock, quiet);
            self.out.process(bl, br, self.clock, 0.0, out_quiet);
            for f in 0..frames {
                let (a, b) = (bl[f], br[f]);
                wrote |= a != 0.0 || b != 0.0;
                peak = peak.max(a.abs()).max(b.abs());
                left[offset + f] += a;
                right[offset + f] += b;
            }
            self.clock += frames as u64;
            offset += frames;
        }
        self.peak = peak;
        if voice_frames > 0 {
            let per = started.elapsed().as_nanos() as f32 / voice_frames as f32;
            self.cpu_ns = if self.cpu_ns == 0.0 {
                per
            } else {
                0.95 * self.cpu_ns + 0.05 * per
            };
        }
        wrote
    }

    /// What the editor draws: the frame's slots, then Arcus's own.
    pub fn telemetry(&self, t: &mut [f32]) {
        if t.len() <= tel::FMAX {
            return;
        }
        t[0] = self
            .voices
            .iter()
            .take(MAX_VOICES)
            .filter(|v| v.active())
            .count() as f32;
        t[1] = self.polyphony() as f32;
        t[2] = self.stolen as f32;
        t[tel::INSTRUMENT] = self.s.instrument as usize as f32;
        t[tel::TECHNIQUE] = self.s.technique as usize as f32;
        t[tel::PEAK] = self.peak;
        t[tel::CPU_NS] = self.cpu_ns;
        t[tel::ALIVE] = 1.0;
        let v = self.newest();
        if v.active() {
            let (lo, hi) = v.limits();
            t[tel::KEY] = v.key() as f32;
            t[tel::F0] = v.f0() as f32;
            t[tel::STRING] = v.string_index() as f32;
            t[tel::BETA] = v.beta();
            t[tel::FORCE] = v.bow_force();
            t[tel::SPEED] = v.bow_velocity();
            t[tel::DIR] = if v.bow_velocity() < 0.0 { -1.0 } else { 1.0 };
            t[tel::DYNAMIC] = v.dynamic();
            t[tel::VIBRATO] = v.vibrato();
            t[tel::STICK] = if v.sticking() { 1.0 } else { 0.0 };
            t[tel::LEVEL] = v.peak();
            t[tel::FMIN] = lo;
            t[tel::FMAX] = hi;
        } else {
            t[tel::KEY] = -1.0;
            t[tel::BETA] = 0.0;
        }
    }
}
