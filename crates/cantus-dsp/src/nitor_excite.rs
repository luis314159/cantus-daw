//! Nitor's exciter by band: harmonics, and only harmonics, added to three bands.
//!
//! Each band — **body** (a band-pass at 400 Hz), **presence** (at 2.5 kHz) and **air** (a
//! high-pass at 7 kHz) — is taken from the input and **normalised** by its own envelope
//! (a peak follower, 1 ms up, 60 ms down), so what follows sees a signal near ±1 whatever
//! the level and the harmonics keep the same share of the band at −40 dBFS as at −6. It is
//! driven into two curves, oversampled 4× (minimum phase, `fervor::os`):
//!
//! - **odd**: `s(v) − v` with `s(v) = v / √(1 + v²)` — the third, fifth … harmonics, the
//!   band's own fundamental taken out (`− v`);
//! - **even**: `s(v)²` — the second, fourth … (and a DC the band's high-pass takes out).
//!
//! The *colour* blends them (0 even, warm; 1 odd, bright). Both are smooth, bounded, and
//! need no transcendental function, so they cost little and give the same bits everywhere.
//! Brought down, the harmonics are high-passed at the band's lower edge (taking out the DC
//! and the slow beats under the band) and multiplied back by the envelope, then by the level.
//!
//! **Null**: a band at level 0 is not run and its state is cleared on the sample its level
//! landed there, so it starts afresh when it is turned up again.

use crate::fervor::os::Oversampler;
use crate::math;
use crate::multiplex::filters::{warp, Svf, SvfCoefs};

/// The bands: body, presence, air.
pub const BANDS: usize = 3;
/// The bands' centres (body, presence) and corner (air), Hz.
pub const BAND_HZ: [f32; BANDS] = [400.0, 2_500.0, 7_000.0];
/// The high-pass after the curves, at each band's lower edge, Hz.
pub const EDGE_HZ: [f32; BANDS] = [250.0, 1_600.0, 5_000.0];
/// The oversampling (minimum phase: the harmonics come about a sample late, which a
/// harmonic that was not there before cannot be heard to be).
pub const FACTOR: usize = 4;
/// The drive into the curves of a normalised band.
const DRIVE: f32 = 1.6;
/// Harmonics at level 1, relative to the band (the even curve's and the odd's own scale
/// make them alike at the colour's middle).
const EVEN_GAIN: f32 = 0.55;
const ODD_GAIN: f32 = 1.1;
/// The envelope's times, seconds, and its floor.
const ATTACK_S: f32 = 0.001;
const RELEASE_S: f32 = 0.06;
const ENV_FLUSH: f32 = 1e-20;

/// One band of one channel.
#[derive(Clone, Debug)]
struct Band {
    pick: Svf,
    post: [Svf; 2],
    env: f32,
    os: Oversampler,
}

impl Band {
    fn new() -> Band {
        Band {
            pick: Svf::default(),
            post: [Svf::default(); 2],
            env: 0.0,
            os: Oversampler::new(FACTOR, true),
        }
    }

    fn is_clear(&self) -> bool {
        self.pick.is_clear()
            && self.post.iter().all(Svf::is_clear)
            && crate::rest::z(self.env)
            && self.os.is_clear()
    }
}

/// The curves' sum at `v`, for an even share `even` (1 − colour).
#[inline]
pub fn harmonics(v: f32, even: f32) -> f32 {
    let s = v / (1.0 + v * v).sqrt();
    even * EVEN_GAIN * s * s + (1.0 - even) * ODD_GAIN * (s - v)
}

/// The exciter, stereo.
#[derive(Clone, Debug)]
pub struct Exciter {
    bands: [[Band; BANDS]; 2],
    pick_c: [SvfCoefs; BANDS],
    post_c: [SvfCoefs; BANDS],
    att: f32,
    rel: f32,
}

impl Exciter {
    pub fn new(sample_rate: f32) -> Exciter {
        let sr = sample_rate.max(1.0);
        let mut pick_c = [SvfCoefs::default(); BANDS];
        let mut post_c = [SvfCoefs::default(); BANDS];
        for i in 0..BANDS {
            // Body and presence: a band-pass of Q 1; air: a Butterworth high-pass.
            let k = if i == 2 {
                core::f32::consts::SQRT_2
            } else {
                1.0
            };
            pick_c[i] = SvfCoefs::new(warp(BAND_HZ[i], sr), k);
            post_c[i] = SvfCoefs::new(warp(EDGE_HZ[i], sr), core::f32::consts::SQRT_2);
        }
        let coef = |s: f32| 1.0 - math::exp(-1.0 / (s * sr));
        Exciter {
            bands: [
                [Band::new(), Band::new(), Band::new()],
                [Band::new(), Band::new(), Band::new()],
            ],
            pick_c,
            post_c,
            att: coef(ATTACK_S),
            rel: coef(RELEASE_S),
        }
    }

    /// Band `b`'s state, both channels, as new.
    pub fn clear(&mut self, b: usize) {
        for ch in &mut self.bands {
            ch[b] = Band::new();
        }
    }

    pub fn reset(&mut self) {
        for b in 0..BANDS {
            self.clear(b);
        }
    }

    pub fn is_clear(&self, b: usize) -> bool {
        self.bands.iter().all(|ch| ch[b].is_clear())
    }

    /// Band `b`'s harmonics for one sample of channel `ch` (before its level).
    #[inline]
    pub fn tick(&mut self, ch: usize, b: usize, x: f32, even: f32) -> f32 {
        let (att, rel) = (self.att, self.rel);
        let band = &mut self.bands[ch][b];
        let (_, bp, hp) = band.pick.tick(&self.pick_c[b], x);
        let picked = if b == 2 { hp } else { self.pick_c[b].k * bp };
        let a = picked.abs();
        band.env += (a - band.env) * if a > band.env { att } else { rel };
        if band.env < ENV_FLUSH {
            band.env = 0.0;
        }
        let v = if band.env > 0.0 {
            DRIVE * picked / band.env.max(1e-9)
        } else {
            0.0
        };
        let mut up = [0.0f32; 8];
        band.os.up(v, &mut up);
        for u in up.iter_mut().take(FACTOR) {
            *u = harmonics(*u, even);
        }
        let h = band.os.down(&mut up);
        let (_, _, h1) = band.post[0].tick(&self.post_c[b], h);
        let (_, _, h2) = band.post[1].tick(&self.post_c[b], h1);
        h2 * band.env * (1.0 / DRIVE)
    }
}
