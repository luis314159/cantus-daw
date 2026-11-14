//! Vetus's noise (ADR 0133, provisional): what a medium adds — a record's crackle and pops,
//! the hiss of a tape or of a vinyl surface, the mains' hum and its buzz.
//!
//! **The noise is a pure function of the song sample**: every random is a hash of a seed and
//! a count (`modfx_clock::hash64`), and every shape a table read at the time since an event,
//! so nothing runs that holds memory. The noise at a moment of the song is the same offline
//! and live, at every block size, and a render from bar 5 hears what the whole render hears
//! there — to the bit, whatever came before. (The few values kept between samples are a cache
//! of that function, refilled whenever the song jumps.)
//!
//! - **Crackle**: the song is cut in cells of ⅔ ms; in each, a hash says whether a dust tick
//!   falls (8 … 408 a second with the amount), where, how big (a cube of a uniform: most are
//!   small) and which way. A tick is a 2.6 kHz ring dying in a quarter of a millisecond.
//!   Each channel has its own ticks. **Pops** (0.2 … 3 a second, larger: a scratch's thump of
//!   700 and 180 Hz over 30 ms) fall in cells of 10.7 ms, common to both channels.
//! - **Hiss**: white noise from the hash, brightened for tape (a first difference) or
//!   darkened for a vinyl surface (an 8-tap binomial low-pass, about 4.5 kHz), both of unit
//!   power; 100 % is −24 dBFS RMS and the amount is squared, so the knob's low end is fine.
//! - **Hum**: the mains' fundamental (50 or 60 Hz) with its 2nd, 3rd and 5th harmonics, mono,
//!   its phase the song sample's; 100 % is about −30 dBFS RMS.

use crate::math;
use crate::modfx_clock::hash64;

/// A uniform in 0 … 1 from a hash.
#[inline]
pub fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// The hash of a seed, a stream and a count.
#[inline]
pub fn hash3(seed: u64, stream: u64, n: i64) -> u64 {
    hash64(seed ^ stream.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ hash64(n as u64))
}

const SEED: u64 = 0x7E75_50A7;
/// Streams of the hash: each random has its own.
const S_TICK: [u64; 2] = [1, 2];
const S_POP: u64 = 5;
const S_HISS: [u64; 2] = [7, 8];
/// Cells an event may have started in and still sound, the current one included.
const KEEP: usize = 4;
/// The vinyl hiss's binomial low-pass.
const BINOMIAL: [f32; 8] = [1.0, 7.0, 21.0, 35.0, 35.0, 21.0, 7.0, 1.0];

/// The noise's settings, after Character.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoiseParams {
    pub crackle: f32,
    pub hiss: f32,
    /// Vinyl surface (true) or tape.
    pub vinyl: bool,
    pub hum: f32,
    pub hum_60: bool,
}

/// An event in a cell: where it starts (a song sample) and its size (0: none).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Event {
    start: i64,
    size: f32,
}

/// The events of the last [`KEEP`] cells, the newest last: a cache of the hash.
#[derive(Clone, Debug)]
struct Cells {
    newest: i64,
    ev: [Event; KEEP],
}

impl Default for Cells {
    fn default() -> Self {
        Cells {
            newest: i64::MIN,
            ev: [Event::default(); KEEP],
        }
    }
}

impl Cells {
    /// The events up to cell `c`, `f` the event of a cell; how many new cells had one.
    #[inline]
    fn at(&mut self, c: i64, f: impl Fn(i64) -> Event) -> u32 {
        if c == self.newest {
            return 0;
        }
        let mut fresh = 0;
        if self.newest != i64::MIN && c == self.newest + 1 {
            self.ev.copy_within(1.., 0);
            self.ev[KEEP - 1] = f(c);
            fresh += u32::from(self.ev[KEEP - 1].size != 0.0);
        } else {
            for (k, e) in self.ev.iter_mut().enumerate() {
                *e = f(c - (KEEP - 1 - k) as i64);
            }
        }
        self.newest = c;
        fresh
    }

    /// The events' sum at song sample `n` through `shape`.
    #[inline]
    fn sum(&self, n: i64, shape: &[f32]) -> f32 {
        let mut y = 0.0;
        for e in &self.ev {
            if e.size != 0.0 {
                let k = n - e.start;
                if k >= 0 && (k as usize) < shape.len() {
                    y += e.size * shape[k as usize];
                }
            }
        }
        y
    }
}

/// The noise for both channels.
#[derive(Clone, Debug)]
pub struct Noise {
    sr: f32,
    tick_cell: i64,
    pop_cell: i64,
    tick: Box<[f32]>,
    pop: Box<[f32]>,
    ticks: [Cells; 2],
    pops: Cells,
    /// The last eight whites of each channel, the newest last, and the sample it is of.
    white: [[f32; 8]; 2],
    white_n: i64,
    /// The settings the caches were filled for.
    filled: Option<(f32, bool)>,
    /// Ticks and pops that fell since this was last cleared (the editor's specks).
    pub clicks: u32,
}

/// A decaying ring of `hz`, its time constant `tau` seconds, `len` samples, tapered to 0.
fn ring(sr: f32, len: usize, parts: &[(f32, f32, f32)]) -> Box<[f32]> {
    let mut v: Vec<f32> = (0..len)
        .map(|k| {
            let t = k as f32 / sr;
            let taper = 1.0 - k as f32 / len as f32;
            parts
                .iter()
                .map(|(hz, tau, a)| {
                    a * math::sin(std::f32::consts::TAU * hz * t) * math::exp(-t / tau)
                })
                .sum::<f32>()
                * taper
        })
        .collect();
    let peak = v.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-9);
    for x in &mut v {
        *x /= peak;
    }
    v.into_boxed_slice()
}

impl Noise {
    /// Builds its tables: allocates, so off the audio thread.
    pub fn new(sample_rate: f32) -> Noise {
        let sr = sample_rate;
        // A tick's ring starts at its peak (a cosine's phase) so it is heard as a tick.
        let tick_len = ((0.002 * sr) as usize).max(8);
        let tick = ring(
            sr,
            tick_len,
            &[(2_600.0, 0.000_25, 1.0), (6_100.0, 0.000_1, 0.6)],
        );
        let pop_len = ((0.03 * sr) as usize).max(32);
        let pop = ring(sr, pop_len, &[(700.0, 0.004, 1.0), (180.0, 0.008, 0.6)]);
        Noise {
            sr,
            tick_cell: ((sr / 1_500.0) as i64).max(1),
            pop_cell: ((sr * 0.010_67) as i64).max(1),
            tick,
            pop,
            ticks: [Cells::default(), Cells::default()],
            pops: Cells::default(),
            white: [[0.0; 8]; 2],
            white_n: i64::MIN,
            filled: None,
            clicks: 0,
        }
    }

    /// Forget the caches (they are refilled from the hash).
    pub fn reset(&mut self) {
        self.ticks = [Cells::default(), Cells::default()];
        self.pops = Cells::default();
        self.white_n = i64::MIN;
        self.filled = None;
        self.clicks = 0;
    }

    #[inline]
    fn white_of(ch: usize, n: i64) -> f32 {
        let h = hash3(SEED, S_HISS[ch], n);
        // Two uniforms' sum is triangular: unit power once scaled by √6.
        (unit(h) + unit(hash64(h)) - 1.0) * 2.449_49
    }

    /// The noise at song sample `n`, both channels, scaled by `gain`.
    #[inline]
    pub fn sample(&mut self, p: &NoiseParams, n: i64, gain: f32) -> (f32, f32) {
        let mut out = [0.0f32; 2];
        if p.crackle > 0.0 {
            let c = p.crackle;
            if self.filled != Some((c, p.vinyl)) {
                self.ticks = [Cells::default(), Cells::default()];
                self.pops = Cells::default();
                self.filled = Some((c, p.vinyl));
            }
            let tick_p = (8.0 + 400.0 * c * c) / self.sr * self.tick_cell as f32;
            let pop_p = (0.2 + 2.8 * c * c) / self.sr * self.pop_cell as f32;
            let tc = self.tick_cell;
            for (ch, o) in out.iter_mut().enumerate() {
                let fresh = self.ticks[ch].at(n.div_euclid(tc), |cell| {
                    let h = hash3(SEED, S_TICK[ch], cell);
                    if unit(h) >= tick_p {
                        return Event::default();
                    }
                    let g = hash64(h);
                    let v = unit(g);
                    let size = 0.6 * c * v * v * v;
                    Event {
                        start: cell * tc + (unit(hash64(g)) * tc as f32) as i64,
                        size: if g & 1 == 0 { size } else { -size },
                    }
                });
                self.clicks += fresh;
                *o += self.ticks[ch].sum(n, &self.tick);
            }
            let pc = self.pop_cell;
            self.clicks += self.pops.at(n.div_euclid(pc), |cell| {
                let h = hash3(SEED, S_POP, cell);
                if unit(h) >= pop_p {
                    return Event::default();
                }
                let g = hash64(h);
                let size = (0.35 + 0.65 * unit(g)) * 0.5 * c;
                Event {
                    start: cell * pc + (unit(hash64(g)) * pc as f32) as i64,
                    size: if g & 1 == 0 { size } else { -size },
                }
            });
            let pop = self.pops.sum(n, &self.pop);
            out[0] += pop;
            out[1] += pop;
        }
        if p.hiss > 0.0 {
            if n == self.white_n.wrapping_add(1) && self.white_n != i64::MIN {
                for (ch, w) in self.white.iter_mut().enumerate() {
                    w.copy_within(1.., 0);
                    w[7] = Self::white_of(ch, n);
                }
            } else if n != self.white_n {
                for (ch, w) in self.white.iter_mut().enumerate() {
                    for (k, x) in w.iter_mut().enumerate() {
                        *x = Self::white_of(ch, n - (7 - k as i64));
                    }
                }
            }
            self.white_n = n;
            let level = p.hiss * p.hiss * 0.063;
            for (o, w) in out.iter_mut().zip(&self.white) {
                let h = if p.vinyl {
                    let s: f32 = w.iter().zip(BINOMIAL).map(|(x, b)| x * b).sum();
                    s * (2.185 / 128.0)
                } else {
                    (w[7] - 0.55 * w[6]) * 0.876
                };
                *o += h * level;
            }
        }
        if p.hum > 0.0 {
            let f0 = if p.hum_60 { 60.0 } else { 50.0 };
            // The phase from the song sample, in f64 so an hour's count keeps its fraction.
            let cyc = (n as f64 * f0 / f64::from(self.sr)).rem_euclid(1.0) as f32;
            let w = std::f32::consts::TAU * cyc;
            let h = math::sin(w)
                + 0.45 * math::sin(2.0 * w)
                + 0.3 * math::sin(3.0 * w)
                + 0.12 * math::sin(5.0 * w);
            // The four partials' power is 0.65 of the fundamental's; 100 % is −30 dBFS RMS.
            let level = p.hum * p.hum * 0.0316 * std::f32::consts::SQRT_2 / 1.1874;
            out[0] += h * level;
            out[1] += h * level;
        }
        (out[0] * gain, out[1] * gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(crackle: f32, hiss: f32, hum: f32) -> NoiseParams {
        NoiseParams {
            crackle,
            hiss,
            vinyl: false,
            hum,
            hum_60: false,
        }
    }

    /// The noise is a function of the song sample: asked for the same samples after a jump,
    /// in another order of calls, it gives the same bits.
    #[test]
    fn the_noise_is_a_function_of_the_song_sample() {
        let p = NoiseParams {
            vinyl: true,
            ..params(0.6, 0.5, 0.3)
        };
        let mut a = Noise::new(48_000.0);
        let mut b = Noise::new(48_000.0);
        let x: Vec<(f32, f32)> = (1_000..9_000).map(|n| a.sample(&p, n, 1.0)).collect();
        for n in 0..500 {
            b.sample(&p, n * 3, 1.0);
        }
        let y: Vec<(f32, f32)> = (1_000..9_000).map(|n| b.sample(&p, n, 1.0)).collect();
        assert_eq!(x, y);
        assert!(x.iter().any(|v| v.0 != v.1), "the ticks are stereo");
    }

    #[test]
    fn the_levels_are_what_the_table_says() {
        let sr = 48_000.0;
        let rms = |p: &NoiseParams| {
            let mut g = Noise::new(sr);
            let s: f64 = (0..96_000)
                .map(|n| {
                    let v = f64::from(g.sample(p, n, 1.0).0);
                    v * v
                })
                .sum();
            10.0 * math::log10(s / 96_000.0)
        };
        for vinyl in [false, true] {
            let hiss = rms(&NoiseParams {
                vinyl,
                ..params(0.0, 1.0, 0.0)
            });
            assert!((hiss + 24.0).abs() < 1.0, "hiss at 100 %: {hiss:.1} dBFS");
        }
        let hum = rms(&params(0.0, 0.0, 1.0));
        assert!((hum + 30.0).abs() < 1.0, "hum at 100 %: {hum:.1} dBFS");
        let quiet = rms(&params(0.0, 0.1, 0.0));
        assert!(quiet < -60.0, "10 % is far below: {quiet:.1}");
    }

    #[test]
    fn crackle_grows_with_its_amount() {
        let count = |c: f32| {
            let mut g = Noise::new(48_000.0);
            for n in 0..48_000 {
                g.sample(&params(c, 0.0, 0.0), n, 1.0);
            }
            g.clicks
        };
        let (lo, hi) = (count(0.1), count(1.0));
        assert!((10..40).contains(&lo), "{lo}");
        assert!((600..1_000).contains(&hi), "{hi}");
    }
}
