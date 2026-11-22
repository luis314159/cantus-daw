//! The render (a child of `fides`): the voices of each chunk into the chunk's buffer —
//! each into its own lane on the pool's threads when it can help, the lanes summed in voice
//! order (ADR 0038) — then the body and the output stage; and what the editor is told.

use super::voice::Shared;
use super::{target as tg, tel, Fides, CHUNK, GHOSTS, MAX_VOICES, SOURCES, VOICE_SOURCES};
use crate::par::{add_into, clear_lane, run_each, Parallel, Serial};

impl Fides {
    /// Render added into `left`/`right`. Returns whether anything but zeros was written.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.render(left, right, None, &Serial)
    }

    /// [`Self::process`] with the voices handed to `par` (the same bits).
    pub fn process_par(&mut self, left: &mut [f32], right: &mut [f32], par: &dyn Parallel) -> bool {
        self.render(left, right, None, par)
    }

    /// [`Self::process_par`] with a live input for the Input exciter: `input` (mono, as
    /// long as the block) excites every voice that plays it.
    pub fn process_input(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        input: &[f32],
        par: &dyn Parallel,
    ) -> bool {
        self.render(left, right, Some(input), par)
    }

    fn render(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        input: Option<&[f32]>,
        par: &dyn Parallel,
    ) -> bool {
        let n = left.len().min(right.len());
        let sounding = self.voices.iter().any(|v| v.active());
        if !sounding && self.body.is_still() && self.out.is_still() {
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
            self.global_step();
            let any = self.voices.iter().any(|v| v.active());
            if any {
                let sh = Shared {
                    s: &self.s,
                    sr: self.sr,
                    matrix: &self.matrix,
                    globals: &self.globals,
                    bend_st: self.bend * self.s.bend_st,
                    hammer: &self.hammer,
                    hammer_area: self.hammer_area,
                    clock: self.clock,
                    input: input.map(|x| &x[offset.min(x.len())..(offset + frames).min(x.len())]),
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
            let quiet = !any;
            // The output stage's input is zeros only when the body's is and the body was
            // still: a body ringing after the last voice ended is heard through the gain
            // and the tremolo like any other chunk.
            let out_quiet = quiet && self.body.is_still();
            let (bl, br) = (&mut self.buf_l[..frames], &mut self.buf_r[..frames]);
            self.body.process(bl, br, self.clock, quiet);
            let trem = (self.s.trem_depth + self.global_mods[1]).clamp(0.0, 1.0);
            self.out.process(bl, br, self.clock, trem, out_quiet);
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

    /// The instance-wide targets, from the global sources alone, at each chunk.
    fn global_step(&mut self) {
        let mut mix = 0.0;
        let mut trem = 0.0;
        if !self.matrix.is_empty()
            && (self.matrix.targets(tg::BODY_MIX) || self.matrix.targets(tg::TREM_DEPTH))
        {
            let mut src = [0f32; SOURCES];
            src[VOICE_SOURCES..].copy_from_slice(&self.globals);
            let mut m = [0f32; tg::COUNT];
            self.matrix.apply(&src, &mut m);
            mix = m[tg::BODY_MIX as usize];
            trem = m[tg::TREM_DEPTH as usize];
        }
        if mix != self.global_mods[0] {
            self.global_mods[0] = mix;
            self.body.set_mix(self.s.body.mix + mix);
        }
        self.global_mods[1] = trem;
    }

    /// What the editor draws: the frame's slots, then Fides's own.
    pub fn telemetry(&self, t: &mut [f32]) {
        if t.len() < tel::RATIOS + tel::MODE_COUNT {
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
        let s = &self.s;
        t[tel::EXCITER] = s.exc.kind as usize as f32;
        t[tel::RES_A] = s.a.kind as usize as f32;
        t[tel::RES_B] = if s.b_on {
            s.b.kind as usize as f32
        } else {
            -1.0
        };
        t[tel::BODY] = s.body.kind as usize as f32;
        t[tel::PEAK] = self.peak;
        t[tel::CPU_NS] = self.cpu_ns;
        t[tel::ALIVE] = 1.0;
        let v = self.newest();
        if v.active() {
            t[tel::KEY] = v.key() as f32;
            t[tel::F0] = v.a.f0 as f32;
            t[tel::LEVEL] = v.peak();
            t[tel::BOW] = v.bow_velocity();
            t[tel::PRESSURE] = v.pressure();
            t[tel::POS] = v.position();
            if !v.a.kind().is_guide() {
                for k in 0..tel::MODE_COUNT {
                    t[tel::MODES + k] = v.a.modal.level(k);
                    t[tel::RATIOS + k] = v.a.modal.ratio(k) as f32;
                }
            }
        } else {
            t[tel::KEY] = -1.0;
            t[tel::POS] = s.exc.pos;
        }
    }
}
