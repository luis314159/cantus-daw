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
        let rad_still = self.rad.iter().all(|r| *r == [0.0; 4]);
        let upper_still = self.upper_body.is_still(); // r42:arcus-upper
        if !sounding
            && rad_still
            && self.sym.is_still() // r42 fiddle
            && self.body.is_still()
            && upper_still // r42:arcus-upper
            && self.eq.is_still()
            && self.out.is_still()
        {
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
            // r42 fiddle: the open strings in sympathy, driven by the bridge (none at `sym` 0).
            self.sym
                .process(&mut self.buf_l[..frames], &mut self.buf_r[..frames]);
            let quiet = !any && self.sym.is_still() && self.rad.iter().all(|r| *r == [0.0; 4]);
            let out_quiet = quiet && self.body.is_still() && self.upper_body.is_still(); // r42:arcus-upper
            let (bl, br) = (&mut self.buf_l[..frames], &mut self.buf_r[..frames]);
            // The radiation (r42:arcus-p5: a second-order Butterworth high-pass, 12 dB an
            // octave under the corner), its state let go at exact zeros so the instance rests.
            // (r42 arcus-classical: model 2's air mode adds a second section, `rad_c2`.)
            for (stage, [b0, b1, b2, a1, a2]) in [self.rad_c, self.rad_c2].into_iter().enumerate() {
                if b0 == 0.0 {
                    continue;
                }
                for side in 0..2 {
                    let buf: &mut [f32] = if side == 0 { &mut *bl } else { &mut *br };
                    let [mut x1, mut x2, mut y1, mut y2] = self.rad[2 * stage + side];
                    if quiet && [x1, x2, y1, y2] == [0.0; 4] {
                        continue;
                    }
                    for x in buf.iter_mut() {
                        let v = *x;
                        let y = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                        (x2, x1, y2, y1) = (x1, v, y1, y);
                        if v == 0.0 && x2 == 0.0 && y.abs() < 1e-20 && y2.abs() < 1e-20 {
                            (x1, x2, y1, y2) = (0.0, 0.0, 0.0, 0.0);
                        }
                        *x = y1;
                    }
                    self.rad[2 * stage + side] = [x1, x2, y1, y2];
                }
            }
            self.body.process(bl, br, self.clock, quiet);
            self.upper_body.process(bl, br, self.clock, quiet); // r42:arcus-upper
            let out_quiet = out_quiet && self.eq.is_still();
            let bodies_still = self.body.is_still() && self.upper_body.is_still(); // r42:arcus-upper
            self.eq.process(bl, br, quiet && bodies_still); // r42:arcus-upper
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
