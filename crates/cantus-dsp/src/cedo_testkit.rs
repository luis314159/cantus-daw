//! Cedo's test kit (for the tests and the listening example): the automation editor's
//! segment shapes mirrored for a DSP-only build, a driver that runs a unit in blocks and
//! splits them at events (as the engine's passes split), the song's grid in exact
//! rationals, and a few signals and measures.

use super::{Cedo, CedoSettings};
use crate::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};
use crate::math;
use crate::modfx_clock::Transport;

/// The automation editor's shapes as `cantus_core::automation::Shape::at` draws them:
/// 0 the single curve (`u^p` / its mirror, `p = 1 + 7|t|`), 1 the S-curve, 2 a hold;
/// the rest as straight lines (the tests use none of them).
pub fn shape_fn(shape: u8, tension: f32, u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    let t = if tension.is_finite() {
        tension.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let bend = |u: f32, t: f32| {
        if t == 0.0 {
            u
        } else {
            let p = 1.0 + 7.0 * t.abs();
            if t > 0.0 {
                math::powf(u, p)
            } else {
                1.0 - math::powf(1.0 - u, p)
            }
        }
    };
    match shape {
        0 => bend(u, t),
        1 => {
            if u < 0.5 {
                0.5 * bend(2.0 * u, t.abs())
            } else {
                1.0 - 0.5 * bend(2.0 - 2.0 * u, t.abs())
            }
        }
        2 => 0.0,
        _ => u,
    }
}

/// Something that happens at a sample of a run.
#[derive(Copy, Clone, Debug)]
pub enum Event {
    /// New settings.
    Set(CedoSettings),
    /// The transport starts (or jumps) here, at this song sample and tempo.
    Play { song: i64, bpm: f64 },
    /// The transport stops here.
    Stop,
    /// A note-on from the trigger track, at a velocity.
    Note(u8),
}

/// A run: the input, the key (if any), events at samples, the block size.
pub struct Run<'a> {
    pub left: &'a [f32],
    pub right: &'a [f32],
    pub key: Option<(&'a [f32], &'a [f32])>,
    pub events: &'a [(usize, Event)],
    pub block: usize,
}

/// Runs `u` over `r`, the blocks split at every event (the transport told at every block's
/// start, as the engine tells it each pass). Returns the output and each block's meter
/// gain reduction at its last sample (for the timing measures with one-sample blocks).
pub fn run(u: &mut Cedo, r: &Run) -> (Vec<f32>, Vec<f32>) {
    drive(u, r, None)
}

fn drive(u: &mut Cedo, r: &Run, mut trace: Option<&mut Vec<f32>>) -> (Vec<f32>, Vec<f32>) {
    let n = r.left.len();
    let (mut l, mut rr) = (r.left.to_vec(), r.right.to_vec());
    let splits: Vec<(usize, Event)> = r
        .events
        .iter()
        .copied()
        .filter(|x| !matches!(x.1, Event::Note(_)))
        .collect();
    let notes: Vec<(usize, u8)> = r
        .events
        .iter()
        .filter_map(|x| match x.1 {
            Event::Note(v) => Some((x.0, v)),
            _ => None,
        })
        .collect();
    let mut playing: Option<(i64, usize, f64)> = None;
    let (mut i, mut e, mut k) = (0, 0, 0);
    while i < n {
        while e < splits.len() && splits[e].0 <= i {
            match splits[e].1 {
                Event::Set(s) => u.set(&s),
                Event::Play { song, bpm } => playing = Some((song, i, bpm)),
                Event::Stop => {
                    if let Some((song, at, bpm)) = playing {
                        u.set_transport(&Transport {
                            playing: false,
                            tempo_bpm: bpm,
                            samples: song + (i - at) as i64,
                        });
                    }
                    playing = None;
                }
                Event::Note(_) => {}
            }
            e += 1;
        }
        let next_split = splits.get(e).map_or(n, |x| x.0.min(n));
        let end = (i + r.block).min(next_split).min(n).max(i + 1);
        if let Some((song, at, bpm)) = playing {
            u.set_transport(&Transport {
                playing: true,
                tempo_bpm: bpm,
                samples: song + (i - at) as i64,
            });
        }
        // Notes inside the block, at their offsets (they do not split it).
        while k < notes.len() && notes[k].0 < end {
            u.note(notes[k].0.saturating_sub(i), notes[k].1);
            k += 1;
        }
        let key = r.key.map(|(a, b)| (&a[i..end], &b[i..end]));
        u.process(&mut l[i..end], &mut rr[i..end], key);
        if let Some(t) = trace.as_mut() {
            t.push(u.meter().gr_now[0]);
        }
        i = end;
    }
    (l, rr)
}

/// Like [`run`] in blocks of one: the reduction (dB) on each output sample.
pub fn gain_trace(u: &mut Cedo, r: &Run) -> Vec<f32> {
    assert_eq!(r.block, 1);
    let mut t = Vec::with_capacity(r.left.len());
    drive(u, r, Some(&mut t));
    t
}

/// The sample the song puts position `num/den` quarter notes on, at `bpm` (whole) and
/// `rate` — in exact rationals, rounded half up: the grid the pump is held to.
pub fn grid_sample(num: i128, den: i128, bpm: i128, rate: i128) -> i64 {
    let n = num * 60 * rate;
    let d = den * bpm;
    ((2 * n + d).div_euclid(2 * d)) as i64
}

/// A sine.
pub fn sine(hz: f32, amp: f32, n: usize, rate: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            // The phase in f64: an f32 argument jitters by −68 dB over a second.
            let t = (i % rate as usize) as f64 / rate as f64;
            amp * math::sin(std::f64::consts::TAU * hz as f64 * t) as f32
        })
        .collect()
}

/// A deterministic noise in −1 … 1.
pub fn noise(n: usize, seed: u64) -> Vec<f32> {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        })
        .collect()
}

/// A pad: three detuned saws on a chord, gently low-passed (−12 dBFS-ish).
pub fn pad(n: usize, rate: f32) -> Vec<f32> {
    let freqs = [110.0f32, 164.81, 220.0, 277.18, 329.63];
    let mut ph = [0.0f32; 10];
    let mut lp = 0.0f32;
    (0..n)
        .map(|_| {
            let mut v = 0.0;
            for (j, f) in freqs.iter().enumerate() {
                for (d, det) in [0.997f32, 1.003].iter().enumerate() {
                    let p = &mut ph[j * 2 + d];
                    *p += f * det / rate;
                    *p -= p.floor();
                    v += 2.0 * *p - 1.0;
                }
            }
            lp += 0.08 * (v * 0.05 - lp);
            lp
        })
        .collect()
}

/// A high-pass at `hz`, 96 dB/oct (Aequalis's design), for the splatter measure.
pub struct HighPass {
    n: usize,
    c: [Coefs; MAX_SECTIONS],
    z: [[f64; 2]; MAX_SECTIONS],
}

impl HighPass {
    pub fn new(hz: f32, rate: f32) -> HighPass {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let n = design(
            EqShape::LowCut,
            rate,
            hz,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            96.0,
            &mut c,
        );
        HighPass {
            n,
            c,
            z: [[0.0; 2]; MAX_SECTIONS],
        }
    }

    pub fn run(&mut self, x: &[f32]) -> Vec<f32> {
        x.iter()
            .map(|v| {
                let mut y = *v as f64;
                for (c, z) in self.c[..self.n].iter().zip(self.z.iter_mut()) {
                    let o = c.b0 as f64 * y + z[0];
                    z[0] = c.b1 as f64 * y - c.a1 as f64 * o + z[1];
                    z[1] = c.b2 as f64 * y - c.a2 as f64 * o;
                    y = o;
                }
                y as f32
            })
            .collect()
    }
}

/// Root mean square.
pub fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len() as f64).sqrt()
}

/// dB of a ratio.
pub fn db(x: f64) -> f64 {
    20.0 * math::log10(x.max(1e-30))
}
