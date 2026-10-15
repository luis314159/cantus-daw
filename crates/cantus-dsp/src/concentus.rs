//! Concentus: the chorus (ADR 0055) — the classic ensembles and voices of its own.
//!
//! Bucket-brigade-style modulated delays: each wet voice is the signal read from a line at
//! a delay a triangle (or sine) sweeps, darkened by a two-pole low-pass as a BBD's
//! reconstruction filter darkens it.
//!
//! - **Juno I, II, I+II**: the Juno-60's chorus as its circuit is measured and published —
//!   one BBD per channel, the right one swept by the inverted triangle; I at 0.513 Hz and
//!   II at 0.863 Hz sweeping 1.66 … 5.35 ms, I+II a fast shallow shimmer at 9.75 Hz over
//!   3.3 … 3.7 ms. The dry and the wet at equal level, as the instrument mixes them.
//! - **Dimension 1 … 4**: in the spirit of the four buttons of Roland's SDD-320 (our own
//!   voicing, not a measurement): two lines swept in opposite phase, each also fed **out of
//!   phase** into the other channel, so the width grows with the button and the mono sum
//!   loses little.
//! - **Free**: 1 … 4 voices with the family's LFO (synced or free), their phases spread
//!   evenly round the cycle and their places across the stereo field.
//!
//! The mix is an equal-gain crossfade: at 50 % dry and wet are both at full level (the
//! classic chorus sum), at 0 % the dry alone **to the bit** (the default), at 100 % the
//! wet alone. The editor's **mono check** comes from the telemetry: the output's L/R
//! correlation and how much level the mono sum loses.

use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate, Shape};
use crate::modfx_line::{Glide, Lp2, Lp2Coef, ModLine};

/// The chorus's models (`concentus.mode.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    JunoI,
    JunoII,
    JunoBoth,
    Dim1,
    Dim2,
    Dim3,
    Dim4,
    Free,
}

impl Mode {
    pub const ALL: [Mode; 8] = [
        Mode::JunoI,
        Mode::JunoII,
        Mode::JunoBoth,
        Mode::Dim1,
        Mode::Dim2,
        Mode::Dim3,
        Mode::Dim4,
        Mode::Free,
    ];

    pub fn from_index(i: f32) -> Mode {
        let i = if i.is_finite() { i.round() as i64 } else { 0 };
        Mode::ALL[i.clamp(0, 7) as usize]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// The fixed voicing of a model: (rate Hz, centre ms, sweep ± ms, cross-feed), or
    /// `None` for Free.
    pub fn voicing(self) -> Option<Voicing> {
        let v = |rate, centre, sweep, cross| {
            Some(Voicing {
                rate,
                centre_ms: centre,
                sweep_ms: sweep,
                cross,
            })
        };
        // Juno-60: 1.66 … 5.35 ms, so 3.505 ± 1.845; I+II: 3.3 … 3.7 ms.
        match self {
            Mode::JunoI => v(0.513, 3.505, 1.845, 0.0),
            Mode::JunoII => v(0.863, 3.505, 1.845, 0.0),
            Mode::JunoBoth => v(9.75, 3.5, 0.2, 0.0),
            Mode::Dim1 => v(0.25, 6.0, 0.25, 0.25),
            Mode::Dim2 => v(0.25, 6.0, 0.45, 0.35),
            Mode::Dim3 => v(0.25, 6.0, 0.7, 0.45),
            Mode::Dim4 => v(0.5, 6.0, 0.9, 0.5),
            Mode::Free => None,
        }
    }

    pub fn is_dimension(self) -> bool {
        matches!(self, Mode::Dim1 | Mode::Dim2 | Mode::Dim3 | Mode::Dim4)
    }
}

/// A model's fixed sweep.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Voicing {
    pub rate: f64,
    pub centre_ms: f32,
    pub sweep_ms: f32,
    /// How much of each line feeds the other channel, out of phase (Dimension).
    pub cross: f32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ConcentusSettings {
    pub mode: Mode,
    /// 0 … 1: equal-gain crossfade, dry and wet both at full level at ½.
    pub mix: f32,
    /// Free: voices, 1 … 4.
    pub voices: u32,
    /// Free: the LFO.
    pub lfo: LfoSettings,
    /// Free: the centre delay and the sweep either side, ms.
    pub delay_ms: f32,
    pub depth_ms: f32,
    /// 0 … 1: the stereo spread (the right channel's opposite sweep; the voices' places).
    pub spread: f32,
    /// The wet path's low-pass, Hz (20 kHz: open).
    pub tone_hz: f32,
    pub out_db: f32,
}

impl Default for ConcentusSettings {
    fn default() -> Self {
        ConcentusSettings {
            mode: Mode::JunoI,
            mix: 0.0,
            voices: 3,
            lfo: LfoSettings {
                rate: Rate::Hz(0.8),
                shape: Shape::Triangle,
                ..LfoSettings::default()
            },
            delay_ms: 7.0,
            depth_ms: 2.0,
            spread: 1.0,
            tone_hz: 9_000.0,
            out_db: 0.0,
        }
    }
}

/// The longest delay a voice reads, ms (the free centre's 30 and the sweep's 8, and room).
pub const MAX_MS: f32 = 40.0;
/// Most voices.
pub const MAX_VOICES: usize = 4;
/// The tone at or above this is open (no filter).
pub const TONE_OPEN: f32 = 19_999.0;
const SNAP: f32 = 1e-6;
const SETTLE: f32 = 1e-20;
const SETTLE_RUN: u32 = 64;

/// Concentus's own telemetry: the output's L/R correlation (−1 … 1, smoothed over about
/// 300 ms), what the mono sum loses against the stereo (dB, ≤ 0 … ), and voice 1's delay
/// now (ms).
pub const T_CORR: usize = tel::OWN;
pub const T_MONO_DB: usize = tel::OWN + 1;
pub const T_DELAY_MS: usize = tel::OWN + 2;

/// The equal-gain crossfade's two gains at `mix`: (dry, wet).
#[inline]
pub fn mix_gains(mix: f32) -> (f32, f32) {
    ((2.0 * (1.0 - mix)).min(1.0), (2.0 * mix).min(1.0))
}

/// A voice's place across the stereo field, −1 … 1: `spread` wide, evenly.
#[inline]
pub fn voice_pan(v: usize, n: usize, spread: f32) -> f32 {
    if n <= 1 {
        0.0
    } else {
        spread * (2.0 * v as f32 / (n - 1) as f32 - 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct Concentus {
    sr: f32,
    set: ConcentusSettings,
    clock: Clock,
    lfo: Lfo,
    lines: [ModLine; 2],
    /// The centre and the sweep, samples; the cross-feed; the spread; the mix; the gain.
    centre: Glide,
    sweep: Glide,
    cross: Glide,
    spread: Glide,
    mix: Glide,
    out: Glide,
    tone: Option<Lp2Coef>,
    lp: [Lp2; 2],
    zrun: u32,
    /// The mono check's sums (L², R², L·R), leaky.
    sums: [f32; 3],
    leak: f32,
    tel: [f32; 16],
    blocks: u32,
}

impl Concentus {
    pub fn new(sample_rate: f32) -> Concentus {
        let max = (MAX_MS * 0.001 * sample_rate) as usize + 8;
        let mut c = Concentus {
            sr: sample_rate,
            set: ConcentusSettings::default(),
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            lines: [ModLine::new(max), ModLine::new(max)],
            centre: Glide::new(0.0, 40.0, sample_rate, 1e-3),
            sweep: Glide::new(0.0, 40.0, sample_rate, 1e-3),
            cross: Glide::new(0.0, 40.0, sample_rate, SNAP),
            spread: Glide::new(1.0, 40.0, sample_rate, SNAP),
            mix: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            tone: None,
            lp: [Lp2::default(); 2],
            zrun: 0,
            sums: [0.0; 3],
            leak: 1.0 - crate::math::exp(-1.0 / (0.3 * sample_rate)),
            tel: [0.0; 16],
            blocks: 0,
        };
        let s = c.set;
        c.set(&s);
        c.reset();
        c
    }

    pub fn settings(&self) -> &ConcentusSettings {
        &self.set
    }

    /// The LFO a model runs: its fixed triangle, or Free's.
    fn lfo_of(s: &ConcentusSettings) -> LfoSettings {
        match s.mode.voicing() {
            Some(v) => LfoSettings {
                rate: Rate::Hz(v.rate),
                shape: Shape::Triangle,
                ..LfoSettings::default()
            },
            None => s.lfo,
        }
    }

    pub fn set(&mut self, s: &ConcentusSettings) {
        let ms = |v: f32| v * 0.001 * self.sr;
        let (centre, sweep, cross) = match s.mode.voicing() {
            Some(v) => (ms(v.centre_ms), ms(v.sweep_ms), v.cross),
            None => {
                let c = ms(s.delay_ms.clamp(1.0, 30.0));
                (c, ms(s.depth_ms.clamp(0.0, 8.0)).min(c - 2.0).max(0.0), 0.0)
            }
        };
        self.centre.target = centre;
        self.sweep.target = sweep;
        self.cross.target = cross;
        self.spread.target = s.spread.clamp(0.0, 1.0);
        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        if s.tone_hz != self.set.tone_hz || self.tone.is_none() {
            self.tone = (s.tone_hz < TONE_OPEN)
                .then(|| Lp2Coef::new(s.tone_hz.clamp(500.0, 0.45 * self.sr), self.sr));
        }
        self.lfo.set(&Self::lfo_of(s), &self.clock);
        self.set = *s;
        self.set.voices = s.voices.clamp(1, MAX_VOICES as u32);
    }

    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
        self.lfo.follow(&self.clock);
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        for l in &mut self.lines {
            l.reset();
        }
        for g in [
            &mut self.centre,
            &mut self.sweep,
            &mut self.cross,
            &mut self.spread,
            &mut self.mix,
            &mut self.out,
        ] {
            g.jump(g.target);
        }
        self.lp = [Lp2::default(); 2];
        self.zrun = 0;
        self.sums = [0.0; 3];
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
        if [
            &self.centre,
            &self.sweep,
            &self.cross,
            &self.spread,
            &self.mix,
            &self.out,
        ]
        .iter()
        .any(|g| !g.still())
        {
            return Some("a glide is moving");
        }
        if !self.lines.iter().all(ModLine::is_silent) {
            return Some("a line holds sound");
        }
        if !self.lp.iter().all(Lp2::is_zero) {
            return Some("the tone filter rings");
        }
        None
    }

    pub fn skip(&mut self, frames: usize) {
        self.clock.skip(frames);
        for l in &mut self.lines {
            l.skip(frames);
        }
        self.zrun = self.zrun.saturating_add(frames as u32);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let free = self.set.mode == Mode::Free;
        let voices = self.set.voices.clamp(1, MAX_VOICES as u32) as usize;
        let norm = 1.0 / (voices as f32).sqrt();
        let (mut ml, mut mr, mut d_now) = (0.0f32, 0.0f32, 0.0f32);
        for i in 0..n {
            let (x, y) = (l[i], r[i]);
            self.lines[0].push(x);
            self.lines[1].push(y);
            let centre = self.centre.next();
            let sweep = self.sweep.next();
            let cross = self.cross.next();
            let spread = self.spread.next();
            let (gd, gw) = mix_gains(self.mix.next());
            let out = self.out.next();
            let (mut wl, mut wr);
            if free {
                wl = 0.0;
                wr = 0.0;
                for v in 0..voices {
                    let ph = v as f64 / voices as f64;
                    let m = self.lfo.value_offset(&self.clock, i, ph);
                    let d = centre + sweep * m;
                    if v == 0 {
                        ml = m;
                        d_now = d;
                    }
                    if voices == 1 {
                        let m2 = if spread == 0.0 {
                            m
                        } else {
                            self.lfo.value_offset(&self.clock, i, 0.5 * f64::from(spread))
                        };
                        mr = m2;
                        wl = self.lines[0].read(d);
                        wr = self.lines[1].read(centre + sweep * m2);
                    } else {
                        let (gl, gr) = crate::tremor::pan_gains(voice_pan(v, voices, spread));
                        wl += self.lines[0].read(d) * gl;
                        wr += self.lines[1].read(d) * gr;
                    }
                }
                wl *= norm;
                wr *= norm;
            } else {
                ml = self.lfo.value(&self.clock, i);
                mr = if spread == 0.0 {
                    ml
                } else {
                    self.lfo.value_offset(&self.clock, i, 0.5 * f64::from(spread))
                };
                d_now = centre + sweep * ml;
                let w1 = self.lines[0].read(d_now);
                let w2 = self.lines[1].read(centre + sweep * mr);
                if cross == 0.0 {
                    wl = w1;
                    wr = w2;
                } else {
                    wl = w1 - cross * w2;
                    wr = w2 - cross * w1;
                }
            }
            // The bucket brigade's darker wet path; flushed after a run of silence.
            if x.to_bits() == 0 && y.to_bits() == 0 {
                self.zrun = self.zrun.saturating_add(1);
                if self.zrun >= SETTLE_RUN {
                    for lp in &mut self.lp {
                        lp.settle(SETTLE);
                    }
                }
            } else {
                self.zrun = 0;
            }
            if let Some(c) = &self.tone {
                wl = self.lp[0].run(wl, c);
                wr = self.lp[1].run(wr, c);
            }
            let (mut ox, mut oy) = if gw == 0.0 {
                (x, y)
            } else if gd == 1.0 {
                (x + wl * gw, y + wr * gw)
            } else {
                (x * gd + wl * gw, y * gd + wr * gw)
            };
            if out != 1.0 {
                ox *= out;
                oy *= out;
            }
            l[i] = ox;
            r[i] = oy;
            // The mono check (a meter: it never touches the sound).
            let k = self.leak;
            self.sums[0] += (ox * ox - self.sums[0]) * k;
            self.sums[1] += (oy * oy - self.sums[1]) * k;
            self.sums[2] += (ox * oy - self.sums[2]) * k;
        }
        self.clock.advance(n);
        self.blocks = (self.blocks + 1) % (1 << 24);
        let cycles = self.lfo.cycles(&self.clock, 0);
        let (pl, _) = self.lfo.phase_at(cycles);
        let (pr, _) = self.lfo.phase_at(cycles + 0.5 * f64::from(self.spread.value));
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
        let [ll, rr, lr] = self.sums;
        let (corr, loss) = mono_check(ll, rr, lr);
        self.tel[T_CORR] = corr;
        self.tel[T_MONO_DB] = loss;
        self.tel[T_DELAY_MS] = d_now / self.sr * 1000.0;
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

/// From the sums of L², R² and L·R: the correlation (−1 … 1; 1 for silence) and what the
/// mono sum `(L + R)/2` loses against the stereo's mean level, dB (0: nothing; −∞ when it
/// cancels).
pub fn mono_check(ll: f32, rr: f32, lr: f32) -> (f32, f32) {
    let p = ll + rr;
    if p <= 1e-20 {
        return (1.0, 0.0);
    }
    let corr = (lr / (ll * rr).max(1e-30).sqrt()).clamp(-1.0, 1.0);
    let mono = ((p + 2.0 * lr) * 0.25).max(1e-30);
    let loss = 10.0 * crate::math::log10(mono / (p * 0.5));
    (corr, loss.max(-60.0))
}

#[cfg(test)]
#[path = "concentus_tests.rs"]
mod tests;
