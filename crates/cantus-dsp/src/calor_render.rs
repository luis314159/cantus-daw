//! The render: voices into oversampled buffers a chunk at a time, one decimator for their
//! sum, then the chorus and the output stage; and what the editor is told (a child of
//! `calor::synth`).

use super::super::voice::Shared;
use super::super::CARDS;
use super::{tel, Calor, CHUNK, TAIL};

impl Calor {
    /// Render added into `left`/`right`. Returns whether anything but zeros was written.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        let sounding = self.voices.iter().any(|v| v.active) || self.ghosts.iter().any(|g| g.active);
        if !sounding && self.tail == 0 {
            self.clock += n as u64;
            return false;
        }
        let started = std::time::Instant::now();
        let factor = self.factor;
        let fs_os = self.fs * factor as f32;
        let solve = self.s.quality.solve();
        let mut wrote = false;
        let mut voice_frames = 0usize;
        let mut offset = 0;
        while offset < n {
            let frames = (n - offset).min(CHUNK);
            let os_n = frames * factor;
            self.buf_l[..os_n].fill(0.0);
            self.buf_r[..os_n].fill(0.0);
            {
                let sh = Shared {
                    s: &self.s,
                    fs: self.fs,
                    fs_os,
                    factor,
                    solve,
                    matrix: &self.matrix,
                    globals: &self.globals,
                    bend: self.bend,
                    wheel: self.wheel,
                    trims: &self.trims,
                    coefs: self.coefs,
                    cutoff_log2: self.cutoff_log2,
                    routes_shape_filter: self.routes_shape_filter,
                    routes_env: self.routes_env,
                    clock: self.clock,
                };
                let bl = &mut self.buf_l[..os_n];
                let br = &mut self.buf_r[..os_n];
                for v in self.voices.iter_mut().chain(self.ghosts.iter_mut()) {
                    if v.active {
                        v.render(&sh, bl, br);
                        voice_frames += frames;
                    }
                }
            }
            for f in 0..frames {
                let b = f * factor;
                let (yl, yr) = if factor == 1 {
                    (self.buf_l[b], self.buf_r[b])
                } else {
                    let mut ql = [0f32; 4];
                    let mut qr = [0f32; 4];
                    ql[..factor].copy_from_slice(&self.buf_l[b..b + factor]);
                    qr[..factor].copy_from_slice(&self.buf_r[b..b + factor]);
                    (self.os.down(0, &ql), self.os.down(1, &qr))
                };
                self.out_l[f] = yl;
                self.out_r[f] = yr;
            }
            let (ol, or) = (&mut self.out_l[..frames], &mut self.out_r[..frames]);
            self.chorus
                .process(self.s.chorus, self.s.chorus_mix, ol, or, self.clock);
            self.out.process(ol, or, self.s.gain);
            for f in 0..frames {
                let (a, b) = (self.out_l[f], self.out_r[f]);
                wrote |= a != 0.0 || b != 0.0;
                left[offset + f] += a;
                right[offset + f] += b;
            }
            self.clock += frames as u64;
            offset += frames;
        }
        self.finish_voices();
        // Keep going after the last voice until the decimator, the chorus and the DC block
        // have let go of everything (then the output is exactly zero and stays so).
        let still = self.voices.iter().any(|v| v.active) || self.ghosts.iter().any(|g| g.active);
        if still {
            self.tail = TAIL;
        } else {
            self.tail = self.tail.saturating_sub(n);
            if self.tail == 0 && !(self.out.is_silent() && self.chorus.is_silent()) {
                self.tail = CHUNK;
            }
        }
        let mut peak = 0f32;
        for v in self.voices.iter_mut() {
            peak = peak.max(v.in_peak);
            v.in_peak = 0.0;
        }
        self.in_peak = peak;
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

    /// Voices whose sound has ended give their note slot back to the allocator once the
    /// whole stack is done.
    fn finish_voices(&mut self) {
        let stack = self.s.stack.max(1);
        for a in 0..self.alloc_busy.len() {
            if !self.alloc_busy[a] {
                continue;
            }
            let done = (0..stack)
                .map(|j| a * stack + j)
                .all(|p| self.voices.get(p).is_none_or(|v| !v.active));
            if done {
                self.alloc_busy[a] = false;
                self.alloc.voice_finished(a);
            }
        }
    }

    /// What the editor draws: the frame's slots, then the cards, the level into the
    /// filter, the newest voice's cutoff and envelopes, the quality, the CPU.
    pub fn telemetry(&self, t: &mut [f32]) {
        if t.len() < 48 {
            return;
        }
        t[0] = self.alloc.active() as f32;
        t[1] = self.polyphony() as f32;
        t[2] = self.stolen as f32;
        for c in 0..CARDS {
            let mut level = 0f32;
            let mut key = 0f32;
            for v in self.voices.iter().filter(|v| v.active && v.card == c) {
                level = level.max(v.level());
                key = v.key as f32 + 1.0;
            }
            t[tel::CARD_LEVEL + c] = level;
            t[tel::CARD_KEY + c] = key;
        }
        t[tel::IN_PEAK] = self.in_peak;
        let v = &self.voices[self.newest.min(self.voices.len() - 1)];
        t[tel::CUTOFF] = if v.active { v.cut_hz } else { 0.0 };
        let [e1, e2] = v.env_levels();
        t[tel::ENV1] = e1;
        t[tel::ENV2] = e2;
        t[tel::FACTOR] = self.factor as f32;
        t[tel::CPU_NS] = self.cpu_ns;
        t[tel::LATENCY] = self.latency() as f32;
        t[tel::NEWEST_CARD] = if v.active { v.card as f32 } else { -1.0 };
    }
}
