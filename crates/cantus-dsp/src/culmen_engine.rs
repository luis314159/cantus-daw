//! Culmen's running state: the three stages, the reference path, the meters.

use super::telemetry::{self as tel, Column};
use super::{db, MaxSettings, GRID, HI, LO, MID};
use crate::math;
use crate::multiplex::Multiplex;
use crate::terminus::loudness::{self, column_len, KWeighting};
use crate::terminus::{self, Ditherer, Monitor, Terminus, DITHER_BITS};

/// Columns of input peaks kept to line the input up with the output (the latency).
const IN_RING: usize = 64;

/// A delay line of up to `len − 1` samples.
#[derive(Clone, Debug)]
struct Line {
    buf: Box<[f32]>,
    pos: usize,
}

impl Line {
    fn new(max_delay: usize) -> Self {
        Line {
            buf: vec![0.0; max_delay + 1].into_boxed_slice(),
            pos: 0,
        }
    }

    /// Push `x`, read what went in `delay` samples ago (0: `x` itself).
    #[inline]
    fn push(&mut self, x: f32, delay: usize) -> f32 {
        let n = self.buf.len();
        self.buf[self.pos] = x;
        let d = delay.min(n - 1);
        let at = if self.pos >= d {
            self.pos - d
        } else {
            self.pos + n - d
        };
        let y = self.buf[at];
        self.pos = if self.pos + 1 == n { 0 } else { self.pos + 1 };
        y
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.pos = 0;
    }

    fn is_clear(&self) -> bool {
        crate::rest::all_zero(&self.buf)
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct Acc {
    energy_in: f64,
    in_peak: f32,
    /// Each band's dynamic gain, the largest in size read this column.
    gains: [f32; 4],
}

/// The maximizer, stereo in place.
#[derive(Clone, Debug)]
pub struct Culmen {
    sr: f32,
    s: MaxSettings,
    bands: Box<Multiplex>,
    full: Box<Multiplex>,
    lim: Box<Terminus>,
    drive: f32,
    drive_target: f32,
    glide: f32,
    ref_a: [Line; 2],
    ref_t: [Line; 2],
    k_in: [KWeighting; 2],
    col_len: usize,
    col_n: usize,
    /// Samples since the stream began, modulo the grid.
    clock: usize,
    acc: Acc,
    in_ring: [f32; IN_RING],
    in_pos: usize,
    st_in: Box<[f64]>,
    st_out: Box<[f64]>,
    st_pos: usize,
    st_seen: usize,
    match_gain: f32,
    match_target: f32,
    match_glide: f32,
    dither: [Ditherer; 2],
    telemetry: [f32; tel::LEN],
    seq: u32,
}

fn coef(ms: f32, sr: f32) -> f32 {
    math::exp(-1.0 / (ms * 0.001 * sr))
}

impl Culmen {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let bands = Box::new(Multiplex::new(sr));
        // The longest the band stage can delay: its linear-phase split.
        let max_a = {
            let mut probe = Multiplex::new(sr);
            let mut s = MaxSettings {
                mode: crate::multiplex::Mode::Linear,
                ..MaxSettings::default()
            };
            s.count = 3;
            probe.set(&s.band_stage());
            probe.latency()
        };
        let max_t = {
            let worst = terminus::LimSettings {
                lookahead_ms: terminus::MAX_LOOKAHEAD_MS,
                true_peak: true,
                clip: 1.0,
                ..terminus::LimSettings::default()
            };
            terminus::latency_of(&worst, sr)
        };
        let mut c = Culmen {
            sr,
            s: MaxSettings::default(),
            bands,
            full: Box::new(Multiplex::new(sr)),
            lim: Box::new(Terminus::new(sr)),
            drive: 1.0,
            drive_target: 1.0,
            glide: 1.0 - coef(10.0, sr),
            ref_a: [Line::new(max_a), Line::new(max_a)],
            ref_t: [Line::new(max_t), Line::new(max_t)],
            k_in: [KWeighting::new(sr); 2],
            col_len: column_len(sr),
            col_n: 0,
            clock: 0,
            acc: Acc::default(),
            in_ring: [-160.0; IN_RING],
            in_pos: 0,
            st_in: vec![0.0; loudness::SHORT_COLUMNS].into_boxed_slice(),
            st_out: vec![0.0; loudness::SHORT_COLUMNS].into_boxed_slice(),
            st_pos: 0,
            st_seen: 0,
            match_gain: 1.0,
            match_target: 1.0,
            match_glide: 1.0 - coef(1_000.0, sr),
            dither: [Ditherer::new(0x2545_f491), Ditherer::new(0xc2b2_ae35)],
            telemetry: [0.0; tel::LEN],
            seq: 0,
        };
        // The stages are left unconfigured: the first `set` snaps every one of them to its
        // settings (a project opened with Culmen working starts working, with no glide from
        // the defaults). Until then each stage is its own default — a wire.
        c.reset();
        c
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &MaxSettings {
        &self.s
    }

    /// New settings. Every stage glides on its own; the drive glides over ~10 ms.
    pub fn set(&mut self, s: &MaxSettings) {
        let first = self.seq == 0 && self.col_n == 0 && self.clock == 0;
        self.s = *s;
        if first {
            // Nothing has been processed: these are the first settings, whoever set others
            // before (a plug-in activated, then given its state). Every stage takes them at
            // once, as Terminus does on its own.
            self.bands.snap_next();
            self.full.snap_next();
        }
        self.bands.set(&s.band_stage());
        self.full.set(&s.full_stage());
        self.lim.set(&s.brickwall());
        self.drive_target = if s.drive_db == 0.0 {
            1.0
        } else {
            math::powf(10f32, s.drive_db / 20.0)
        };
        if first {
            self.drive = self.drive_target;
        }
        self.telemetry[tel::LATENCY] = self.latency() as f32;
        self.telemetry[tel::RATE] = self.sr;
    }

    /// Clear every state; glides land on their targets.
    pub fn reset(&mut self) {
        self.bands.reset();
        self.full.reset();
        self.lim.reset();
        self.drive = self.drive_target;
        for l in self.ref_a.iter_mut().chain(self.ref_t.iter_mut()) {
            l.clear();
        }
        for k in &mut self.k_in {
            k.reset();
        }
        self.col_n = 0;
        self.clock = 0;
        self.acc = Acc::default();
        self.in_ring = [-160.0; IN_RING];
        self.in_pos = 0;
        self.st_in.iter_mut().for_each(|x| *x = 0.0);
        self.st_out.iter_mut().for_each(|x| *x = 0.0);
        self.st_pos = 0;
        self.st_seen = 0;
        self.match_gain = 1.0;
        self.match_target = 1.0;
        self.dither = [Ditherer::new(0x2545_f491), Ditherer::new(0xc2b2_ae35)];
        self.telemetry = [0.0; tel::LEN];
        self.telemetry[tel::LATENCY] = self.latency() as f32;
        self.telemetry[tel::RATE] = self.sr;
        self.seq = 0;
    }

    /// At rest (ADR 0031 §5.1): the three stages at rest, the drive landed, the reference
    /// lines and the input's loudness filter empty, and the Match monitor still — its
    /// three-second window all silence, its gain landed, and at unity in Match mode, as a
    /// new unit's is. With dither on, both ditherers at rest too (R10's counter-based dither,
    /// round 22, as Terminus).
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// What keeps it from resting, if anything: the first condition of [`Self::at_rest`]
    /// that fails, by name (for the tests and a profiler that asks why).
    pub fn rest_blocker(&self) -> Option<&'static str> {
        let s = &self.s;
        if DITHER_BITS[s.dither.min(DITHER_BITS.len() - 1)] > 0
            && !self.dither.iter().all(Ditherer::at_rest)
        {
            return Some("dither");
        }
        if self.drive != self.drive_target {
            return Some("drive glide");
        }
        if !crate::rest::glide_still(self.match_gain, self.match_target, self.match_glide, 1e-6) {
            return Some("match glide");
        }
        if s.monitor == Monitor::Match && self.match_gain != 1.0 {
            return Some("match gain");
        }
        if !crate::rest::z64(self.acc.energy_in) || !self.k_in.iter().all(KWeighting::is_clear) {
            return Some("input loudness");
        }
        if !crate::rest::all_zero_f64(&self.st_in) || !crate::rest::all_zero_f64(&self.st_out) {
            return Some("match window");
        }
        if !self.ref_a.iter().chain(&self.ref_t).all(Line::is_clear) {
            return Some("reference lines");
        }
        if let Some(b) = self.bands.rest_blocker() {
            return Some(b);
        }
        if let Some(b) = self.full.rest_blocker() {
            return Some(b);
        }
        self.lim.rest_blocker()
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): each stage skips them, and
    /// the grid and the 10 ms columns go on as processing would have moved them — each
    /// column silent, so the Match window stays all silence.
    pub fn skip(&mut self, frames: usize) {
        self.bands.skip(frames);
        self.full.skip(frames);
        self.lim.skip(frames);
        self.clock = (self.clock + frames % GRID) % GRID;
        let total = self.col_n + frames;
        let cols = total / self.col_len;
        if cols > 0 {
            let len = self.st_in.len();
            self.st_pos = (self.st_pos + cols % len) % len;
            self.st_seen = self.st_seen.saturating_add(cols);
            self.in_pos = (self.in_pos + cols % IN_RING) % IN_RING;
            self.seq =
                ((self.seq as usize + cols % tel::WRAP as usize) % tel::WRAP as usize) as u32;
            self.acc = Acc::default();
        }
        self.col_n = total % self.col_len;
    }

    /// Samples of delay: the band stage's (its linear-phase split) plus the brickwall's.
    pub fn latency(&self) -> usize {
        self.bands.latency() + self.full.latency() + self.lim.latency()
    }

    /// What the editor reads (see [`super::telemetry`]).
    pub fn telemetry(&self) -> [f32; tel::LEN] {
        self.telemetry
    }

    /// The brickwall's reduction now, dB, the deeper channel.
    pub fn reduction_now_db(&self) -> f32 {
        let g = self.lim.gain_reduction_db();
        g[0].max(g[1])
    }

    /// Each band's dynamic gain now — low, mid, high, full — dB.
    pub fn band_gains_now_db(&self) -> [f32; 4] {
        let a = self.bands.gains_now_db();
        let b = self.full.gains_now_db();
        [a[LO], a[MID], a[HI], b[0]]
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        let mut i = 0;
        while i < n {
            let to_grid = GRID - self.clock % GRID;
            let to_col = self.col_len - self.col_n;
            let len = (n - i).min(to_grid).min(to_col);
            self.piece(&mut left[i..i + len], &mut right[i..i + len]);
            i += len;
        }
    }

    /// At most [`GRID`] samples that cross neither the grid nor a column's end.
    fn piece(&mut self, l: &mut [f32], r: &mut [f32]) {
        let m = l.len();
        let lat_a = self.bands.latency() + self.full.latency();
        let lat_t = self.lim.latency();
        let mut ra = [[0f32; GRID]; 2];
        for j in 0..m {
            for (ch, x) in [l[j], r[j]].into_iter().enumerate() {
                let k = self.k_in[ch].tick(x as f64);
                self.acc.energy_in += k * k;
                self.acc.in_peak = self.acc.in_peak.max(x.abs());
                ra[ch][j] = self.ref_a[ch].push(x, lat_a);
            }
        }
        // Every stage is called every time, so its own grid counts the same samples.
        self.bands.process(l, r, None);
        self.full.process(l, r, None);
        let mut rt = [[0f32; GRID]; 2];
        for j in 0..m {
            if self.drive != self.drive_target {
                self.drive += (self.drive_target - self.drive) * self.glide;
                if (self.drive - self.drive_target).abs() <= 1e-6 * self.drive_target {
                    self.drive = self.drive_target;
                }
            }
            let d = self.drive;
            l[j] *= d;
            r[j] *= d;
            rt[0][j] = self.ref_t[0].push(ra[0][j] * d, lat_t);
            rt[1][j] = self.ref_t[1].push(ra[1][j] * d, lat_t);
        }
        self.lim.process(l, r, None);
        let s = self.s;
        let bits = DITHER_BITS[s.dither.min(DITHER_BITS.len() - 1)];
        for j in 0..m {
            for (ch, buf) in [&mut *l, &mut *r].into_iter().enumerate() {
                let v = buf[j];
                let mut o = if s.delta { rt[ch][j] - v } else { v };
                match s.monitor {
                    Monitor::Normal => {}
                    Monitor::Unity => o /= self.drive,
                    Monitor::Match => o *= self.match_gain,
                }
                if bits > 0 {
                    o = self.dither[ch].process(o, bits, s.shaping);
                }
                buf[j] = o;
            }
            if self.match_gain != self.match_target {
                self.match_gain += (self.match_target - self.match_gain) * self.match_glide;
                if (self.match_gain - self.match_target).abs() < 1e-6 {
                    self.match_gain = self.match_target;
                }
            }
        }
        self.clock = (self.clock + m) % GRID;
        self.col_n += m;
        let column_ends = self.col_n == self.col_len;
        if self.clock == 0 || column_ends {
            let now = self.band_gains_now_db();
            for (acc, g) in self.acc.gains.iter_mut().zip(now) {
                if g.abs() > acc.abs() {
                    *acc = g;
                }
            }
        }
        if column_ends {
            self.finish_column();
            self.col_n = 0;
        }
    }

    fn finish_column(&mut self) {
        let t = self.lim.telemetry();
        let seq_t = t[terminus::T_SEQ].max(0.0) as u32;
        let lc = terminus::Column::read(&t, (seq_t + terminus::T_WRAP - 1) % terminus::T_WRAP);
        let n = self.col_len as f64;
        let energy_in = self.acc.energy_in / n;
        // The Match monitor: the short-term (3 s) energies of input and output.
        self.st_in[self.st_pos] = energy_in;
        self.st_out[self.st_pos] = lc.energy as f64;
        self.st_pos = (self.st_pos + 1) % self.st_in.len();
        self.st_seen += 1;
        if self.st_seen >= self.st_in.len() {
            let e_in: f64 = self.st_in.iter().sum::<f64>() / self.st_in.len() as f64;
            let e_out: f64 = self.st_out.iter().sum::<f64>() / self.st_out.len() as f64;
            // Hold through silence: only a signal above the absolute gate re-levels it.
            if loudness::lufs(e_in) > loudness::ABSOLUTE_GATE && e_out > 0.0 {
                self.match_target = (e_in / e_out).sqrt().clamp(0.01, 10.0) as f32;
            }
        }
        // The input's peak, as many columns back as the latency is long.
        self.in_ring[self.in_pos] = db(self.acc.in_peak);
        let back =
            ((self.latency() as f32 / self.col_len as f32).round() as usize).min(IN_RING - 1);
        let input_db = self.in_ring[(self.in_pos + IN_RING - back) % IN_RING];
        self.in_pos = (self.in_pos + 1) % IN_RING;
        let c = Column {
            energy_out: lc.energy,
            energy_in: energy_in as f32,
            input_db,
            output_db: lc.output_db,
            limit_db: lc.gr_db,
            bands_db: self.acc.gains,
        };
        c.write(&mut self.telemetry, self.seq);
        self.seq = (self.seq + 1) % tel::WRAP;
        let gr_now = self.reduction_now_db();
        let t = &mut self.telemetry;
        t[tel::SEQ] = self.seq as f32;
        t[tel::MATCH] = if self.s.monitor == Monitor::Match {
            db(self.match_gain)
        } else {
            0.0
        };
        t[tel::CLIP] = lc.clip_db;
        let a = self.bands.levels_now_db();
        let b = self.full.levels_now_db();
        t[tel::LEVELS..tel::LEVELS + 4].copy_from_slice(&[a[LO], a[MID], a[HI], b[0]]);
        t[tel::GR_NOW] = gr_now;
        self.acc = Acc::default();
    }
}
