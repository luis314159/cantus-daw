//! Vibrans: the vibrato (ADR 0055).
//!
//! Pitch vibrato by a modulated delay: the signal is read from a line at `D₀ + A·m(t)`, so
//! its pitch moves by `−A·m′(t)` — the depth is set **in cents** and the sweep `A` follows
//! from the rate and the shape (`A = (2^{c/1200} − 1)/(2πf)` for a sine, `/(4f)` for a
//! triangle), so a deeper or slower vibrato never changes what the knob says.
//!
//! - **Rise**: after a silence (50 ms under −50 dBFS) the depth fades in from zero over the
//!   rise time, as a singer's or a violinist's vibrato arrives after the note starts.
//! - **Stereo phase**: the right channel's LFO ahead of the left.
//! - **Mix**: at 100 % (the default) only the moving copy is heard — pitch vibrato; lower,
//!   the dry joins it and it becomes a chorus.
//!
//! **Nulls**: at depth 0 (the default) the line is read at no delay and every sample comes
//! back to the bit; the output at 0 dB touches nothing. The delay is part of the effect
//! (not reported as latency: a vibrato's centre delay moves with its depth and rate).

use crate::modfx_clock::{peak_db, peak_of, synced_inc, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate, Shape};
use crate::modfx_line::{Glide, ModLine};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VibransSettings {
    /// The pitch's swing either way, cents.
    pub depth_cents: f32,
    pub lfo: LfoSettings,
    /// How long the depth takes to arrive after a silence, ms (0: at once).
    pub rise_ms: f32,
    /// The right channel's LFO ahead of the left, cycles (0 … ½).
    pub spread: f32,
    /// 0 … 1: the moving copy against the dry.
    pub mix: f32,
    pub out_db: f32,
}

impl Default for VibransSettings {
    fn default() -> Self {
        VibransSettings {
            depth_cents: 0.0,
            lfo: LfoSettings {
                rate: Rate::Hz(5.5),
                ..LfoSettings::default()
            },
            rise_ms: 0.0,
            spread: 0.0,
            mix: 1.0,
            out_db: 0.0,
        }
    }
}

/// The longest sweep, seconds: the depth is reached down to about 0.2 Hz (sine, 100 ct).
pub const MAX_SWEEP_S: f32 = 0.1;
/// Samples kept beyond the sweep for the interpolator.
const MARGIN: f32 = 2.0;
/// Under this, the input is quiet (−50 dBFS).
const QUIET_LEVEL: f32 = 0.003_16;
/// Quiet samples in a row that make the next sound an onset (50 ms at 48 kHz, scaled).
const QUIET_S: f32 = 0.05;
const SNAP: f32 = 1e-4;

/// The sweep in samples that swings the pitch by `cents` at `hz` with `shape`.
pub fn sweep_for(cents: f32, hz: f64, shape: Shape, sample_rate: f32) -> f32 {
    if cents <= 0.0 || hz <= 0.0 {
        return 0.0;
    }
    let ratio = crate::math::exp2(cents / 1200.0) - 1.0;
    let per = match shape {
        Shape::Triangle => 4.0 * hz,
        _ => std::f64::consts::TAU * hz,
    };
    ((f64::from(ratio) / per) as f32 * sample_rate).min(MAX_SWEEP_S * sample_rate)
}

#[derive(Clone, Debug)]
pub struct Vibrans {
    sr: f32,
    set: VibransSettings,
    clock: Clock,
    lfo: Lfo,
    lines: [ModLine; 2],
    /// The sweep `A` and the centre `D₀`, samples.
    amp: Glide,
    base: Glide,
    mix: Glide,
    out: Glide,
    spread: Glide,
    /// Quiet samples in a row (saturating), and samples since the last onset.
    quiet: u32,
    since: u32,
    quiet_n: u32,
    tel: [f32; 16],
    blocks: u32,
}

impl Vibrans {
    pub fn new(sample_rate: f32) -> Vibrans {
        let max = (MAX_SWEEP_S * 2.0 * sample_rate) as usize + 8;
        Vibrans {
            sr: sample_rate,
            set: VibransSettings::default(),
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            lines: [ModLine::new(max), ModLine::new(max)],
            amp: Glide::new(0.0, 60.0, sample_rate, SNAP),
            base: Glide::new(0.0, 120.0, sample_rate, SNAP),
            mix: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            out: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            spread: Glide::new(0.0, 30.0, sample_rate, 1e-6),
            quiet: u32::MAX,
            since: u32::MAX,
            quiet_n: (QUIET_S * sample_rate) as u32,
            tel: [0.0; 16],
            blocks: 0,
        }
    }

    pub fn settings(&self) -> &VibransSettings {
        &self.set
    }

    pub fn set(&mut self, s: &VibransSettings) {
        self.lfo.set(&s.lfo, &self.clock);
        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        self.spread.target = s.spread.clamp(0.0, 0.5);
        self.set = *s;
        self.retarget();
    }

    /// The LFO's rate now, Hz.
    fn hz(&self) -> f64 {
        match self.set.lfo.rate {
            Rate::Hz(h) => h,
            Rate::Beats(b) => synced_inc(b, self.clock.tempo(), f64::from(self.sr)) * f64::from(self.sr),
        }
    }

    /// The sweep and the centre for the settings and the tempo.
    fn retarget(&mut self) {
        let a = sweep_for(self.set.depth_cents, self.hz(), self.set.lfo.shape, self.sr);
        self.amp.target = a;
        self.base.target = if a > 0.0 { a + MARGIN } else { 0.0 };
    }

    pub fn set_transport(&mut self, t: &Transport) {
        let epoch = self.clock.epoch();
        self.clock.set(t);
        self.lfo.follow(&self.clock);
        if matches!(self.set.lfo.rate, Rate::Beats(_)) {
            self.retarget();
            // At the start of playback the sweep starts where the tempo puts it, so a
            // render from any bar is the same as the full render there.
            if self.clock.epoch() != epoch {
                self.amp.jump(self.amp.target);
                self.base.jump(self.base.target);
            }
        }
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        for l in &mut self.lines {
            l.reset();
        }
        self.retarget();
        for g in [&mut self.amp, &mut self.base, &mut self.mix, &mut self.out, &mut self.spread] {
            g.jump(g.target);
        }
        self.quiet = u32::MAX;
        self.since = u32::MAX;
        self.tel = [0.0; 16];
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if [&self.amp, &self.base, &self.mix, &self.out, &self.spread]
            .iter()
            .any(|g| !g.still())
        {
            return Some("a glide is moving");
        }
        if !self.lines.iter().all(ModLine::is_silent) {
            return Some("the line holds sound");
        }
        if self.quiet < self.quiet_n {
            return Some("the rise is waiting for a silence");
        }
        None
    }

    pub fn skip(&mut self, frames: usize) {
        self.clock.skip(frames);
        for l in &mut self.lines {
            l.skip(frames);
        }
        self.quiet = self.quiet.saturating_add(frames as u32);
        self.since = if self.quiet >= self.quiet_n {
            u32::MAX
        } else {
            self.since.saturating_add(frames as u32)
        };
    }

    /// The depth's share now (0 … 1): the rise since the last onset.
    #[inline]
    fn rise(&self) -> f32 {
        let n = self.set.rise_ms * 0.001 * self.sr;
        if n < 1.0 || self.since == u32::MAX {
            1.0
        } else {
            (self.since as f32 / n).min(1.0)
        }
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let (mut ml, mut mr, mut dev) = (0.0f32, 0.0f32, 0.0f32);
        for i in 0..n {
            let (x, y) = (l[i], r[i]);
            self.lines[0].push(x);
            self.lines[1].push(y);
            // An onset after a silence starts the rise again.
            if x.abs().max(y.abs()) > QUIET_LEVEL {
                if self.quiet >= self.quiet_n {
                    self.since = 0;
                }
                self.quiet = 0;
            } else {
                self.quiet = self.quiet.saturating_add(1);
                if self.quiet >= self.quiet_n {
                    self.since = u32::MAX;
                }
            }
            let env = self.rise();
            self.since = self.since.saturating_add(1);
            let a = self.amp.next() * env;
            let d0 = self.base.next();
            let mix = self.mix.next();
            let out = self.out.next();
            let spread = self.spread.next();
            let (wl, wr) = if a == 0.0 && d0 == 0.0 {
                (x, y)
            } else {
                ml = self.lfo.value(&self.clock, i);
                mr = if spread == 0.0 {
                    ml
                } else {
                    self.lfo.value_offset(&self.clock, i, f64::from(spread))
                };
                dev = a;
                (
                    self.lines[0].read(d0 + a * ml),
                    self.lines[1].read(d0 + a * mr),
                )
            };
            let (mut ox, mut oy) = if mix >= 1.0 {
                (wl, wr)
            } else {
                (x + (wl - x) * mix, y + (wr - y) * mix)
            };
            if out != 1.0 {
                ox *= out;
                oy *= out;
            }
            l[i] = ox;
            r[i] = oy;
        }
        self.clock.advance(n);
        self.blocks = (self.blocks + 1) % (1 << 24);
        let cycles = self.lfo.cycles(&self.clock, 0);
        let (pl, _) = self.lfo.phase_at(cycles);
        let (pr, _) = self.lfo.phase_at(cycles + f64::from(self.spread.value));
        self.tel[tel::SAMPLE_RATE] = self.sr;
        self.tel[tel::TEMPO] = self.clock.tempo() as f32;
        self.tel[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        self.tel[tel::PHASE_L] = pl as f32;
        self.tel[tel::PHASE_R] = pr as f32;
        self.tel[tel::MOD_L] = ml;
        self.tel[tel::MOD_R] = mr;
        self.tel[tel::IN_PEAK] = peak_db(in_peak);
        self.tel[tel::OUT_PEAK] = peak_db(peak_of(&l[..n], &r[..n]));
        self.tel[tel::BLOCKS] = self.blocks as f32;
        self.tel[T_RISE] = self.rise();
        self.tel[T_SWEEP_MS] = dev / self.sr * 1000.0;
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

/// Vibrans's own telemetry: the rise's share now (0 … 1), and the sweep in ms.
pub const T_RISE: usize = tel::OWN;
pub const T_SWEEP_MS: usize = tel::OWN + 1;

#[cfg(test)]
#[path = "vibrans_tests.rs"]
mod tests;
