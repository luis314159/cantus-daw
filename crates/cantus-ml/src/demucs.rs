//! Adapted from `demucs/htdemucs.py` (`_spec`, `_ispec`), `demucs/spec.py` and
//! `demucs/apply.py` of <https://github.com/facebookresearch/demucs> @ e976d93e — MIT,
//! Copyright (c) Meta Platforms, Inc. and affiliates (the licence:
//! `crates/cantus-content/models/LICENSE`, and THIRD-PARTY-NOTICES.md).
//!
//! HT-Demucs around its network, as Meta's code runs it (`demucs.separate` with
//! `demucs.apply.apply_model(shifts=0, split=True, overlap=0.25)` and
//! `HTDemucs.forward`), so the network sees what it was trained on and its output is put
//! together the same way:
//!
//! 1. the track is normalised by its mono mix's mean and deviation;
//! 2. it is cut into segments of 7.8 s every `(1 − overlap)` of that, each centred in a
//!    full-length window with zeros past the track's ends (`TensorChunk.padded`);
//! 3. each segment's spectrogram goes to the network with the segment (`stft.rs`);
//! 4. each source is the time branch plus the inverse of the spectral branch, trimmed back
//!    to the segment (`center_trim`);
//! 5. segments are cross-faded with triangular weights, divided by the weights' sum, and
//!    the normalisation undone.
//!
//! Frames are written as soon as no later segment can touch them, so memory does not grow
//! with the song (one segment's worth per source).

use crate::audio::{Input, Output};
use crate::jobs::Stop;
use crate::stft::{self, Stft};

/// One inference: the network with HT-Demucs's contract.
pub trait Network {
    /// `mix`: the segment, `[left; L][right; L]`; `mag`: its spectrogram as
    /// [`Stft::spec`] lays it out. Returns each source's spectrogram (`[S][4][BINS][T]`,
    /// the same layout per source) and waveform (`[S][2][L]`).
    fn run(&mut self, mix: &[f32], mag: &[f32]) -> Result<(Vec<f32>, Vec<f32>), Stop>;
}

/// Where the segments go: demucs's offsets and its cross-fade.
pub struct Plan {
    pub segment: usize,
    pub stride: usize,
    pub offsets: Vec<u64>,
    /// `weight[i]`: the triangle `apply_model` cross-fades with (`transition_power` 1).
    pub weight: Vec<f32>,
}

impl Plan {
    pub fn new(frames: u64, segment: usize, overlap: f32) -> Plan {
        let stride = (((1.0 - overlap as f64) * segment as f64) as usize).max(1);
        let offsets = (0..frames.max(1)).step_by(stride).collect();
        let half = segment / 2;
        let max = half.max(segment - half) as f32;
        let weight = (1..=half)
            .chain((1..=segment - half).rev())
            .map(|w| w as f32 / max)
            .collect();
        Plan {
            segment,
            stride,
            offsets,
            weight,
        }
    }
}

/// What a separation did.
#[derive(Clone, Copy, Debug)]
pub struct Separated {
    pub inferences: u32,
    /// Seconds inside the network (the rest is the STFT, the files, the cross-fade).
    pub network_seconds: f64,
}

/// What the network is and how the track is cut.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub sources: usize,
    /// Samples per inference.
    pub segment: usize,
    /// How much consecutive segments overlap (0.25).
    pub overlap: f32,
    /// The track moved this many frames later before it is cut, and the result moved back
    /// (`apply_model`'s shift: the virtual track is `delay` zeros, then the track).
    pub delay: u64,
}

/// Separate `input` into `out` (one file per source, in the network's order).
/// `progress(k, of)` after each segment; `cancelled()` is asked before each.
pub fn separate(
    input: &mut Input,
    net: &mut dyn Network,
    shape: Shape,
    out: &mut Output,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Separated, Stop> {
    let Shape {
        sources,
        segment,
        overlap,
        delay,
    } = shape;
    let real = input.frames;
    if real == 0 {
        return Err(Stop::Failed(crate::proto::JobError::Input(
            "the file holds no audio".into(),
        )));
    }
    let (mean, std) = input.mono_stats(cancelled)?;
    if cancelled() {
        return Err(Stop::Cancelled);
    }
    // A silent track: demucs would divide by zero; any scale gives the same silence.
    let std = if std > 1e-8 { std } else { 1.0 };
    // The virtual track: `delay` zeros, then the track.
    let n = real + delay;
    let plan = Plan::new(n, segment, overlap);
    let st = Stft::new();
    let l = segment;
    // The accumulators cover frames [base, base + L).
    let mut acc = vec![vec![0f32; 2 * l]; sources];
    let mut wsum = vec![0f32; l];
    let mut base = 0u64;
    let (mut seg_l, mut seg_r) = (vec![0f32; l], vec![0f32; l]);
    let mut mix = vec![0f32; 2 * l];
    let count = plan.offsets.len();
    let mut network = 0f64;
    for (k, &o) in plan.offsets.iter().enumerate() {
        if cancelled() {
            return Err(Stop::Cancelled);
        }
        // Move the window to start at this segment (what lies before is written).
        let shift = (o - base) as usize;
        if shift > 0 {
            for a in acc.iter_mut() {
                for c in 0..2 {
                    let ch = &mut a[c * l..(c + 1) * l];
                    ch.copy_within(shift.min(l).., 0);
                    ch[l - shift.min(l)..].iter_mut().for_each(|v| *v = 0.0);
                }
            }
            wsum.copy_within(shift.min(l).., 0);
            wsum[l - shift.min(l)..].iter_mut().for_each(|v| *v = 0.0);
            base = o;
        }
        // The segment, centred in a full window (`TensorChunk.padded`), normalised; the
        // padding is zeros after normalisation, as there.
        let len = (l as u64).min(n - o) as usize;
        let delta = l - len;
        let start = o as i64 - (delta / 2) as i64;
        let real_start = start - delay as i64;
        read_window(input, real_start, &mut seg_l, &mut seg_r)?;
        for i in 0..l {
            let at = real_start + i as i64;
            let inside = at >= 0 && (at as u64) < real;
            let norm = |v: f32| {
                if inside {
                    ((v as f64 - mean) / std) as f32
                } else {
                    0.0
                }
            };
            mix[i] = norm(seg_l[i]);
            mix[l + i] = norm(seg_r[i]);
        }
        let mag = st.spec(&mix[..l], &mix[l..]);
        let t = std::time::Instant::now();
        let (spec, wave) = net.run(&mix, &mag)?;
        network += t.elapsed().as_secs_f64();
        let le = stft::frames(l);
        let per = 4 * stft::BINS * le;
        if spec.len() != sources * per || wave.len() != sources * 2 * l {
            return Err(Stop::Failed(crate::proto::JobError::Model(format!(
                "the network gave {} and {} values for {sources} sources",
                spec.len(),
                wave.len()
            ))));
        }
        // Each source: the inverse of its spectrogram plus its waveform, the centre `len`
        // frames, cross-faded into the window. Sources are independent: one thread each.
        let trim = delta / 2;
        let weight = &plan.weight;
        std::thread::scope(|scope| {
            for (s, a) in acc.iter_mut().enumerate() {
                let spec = &spec[s * per..(s + 1) * per];
                let wave = &wave[s * 2 * l..(s + 1) * 2 * l];
                let st = &st;
                scope.spawn(move || {
                    let (mut xl, mut xr) = (vec![0f32; l], vec![0f32; l]);
                    st.ispec(spec, le, l, &mut xl, &mut xr);
                    for i in 0..len {
                        let w = weight[i];
                        a[i] += w * (xl[trim + i] + wave[trim + i]);
                        a[l + i] += w * (xr[trim + i] + wave[l + trim + i]);
                    }
                });
            }
        });
        for i in 0..len {
            wsum[i] += weight[i];
        }
        // Frames before the next segment's start are final.
        let next = plan.offsets.get(k + 1).copied().unwrap_or(n);
        let done = (next.min(n) - o) as usize;
        // The delay's own frames are not the track's: not written.
        let skip = (delay.saturating_sub(o) as usize).min(done);
        let (mut wl, mut wr) = (vec![0f32; done - skip], vec![0f32; done - skip]);
        for (s, a) in acc.iter().enumerate() {
            for i in skip..done {
                let w = wsum[i];
                wl[i - skip] = ((a[i] / w) as f64 * std + mean) as f32;
                wr[i - skip] = ((a[l + i] / w) as f64 * std + mean) as f32;
            }
            out.write(s, &wl, &wr)?;
        }
        progress(k + 1, count);
    }
    Ok(Separated {
        inferences: count as u32,
        network_seconds: network,
    })
}

/// Frames `start .. start + window` of the input, zeros outside it.
fn read_window(input: &mut Input, start: i64, l: &mut [f32], r: &mut [f32]) -> Result<(), Stop> {
    let w = l.len();
    l.iter_mut().for_each(|v| *v = 0.0);
    r.iter_mut().for_each(|v| *v = 0.0);
    let from = start.max(0) as u64;
    let skip = (from as i64 - start) as usize;
    if skip >= w {
        return Ok(());
    }
    input.read(from, &mut l[skip..], &mut r[skip..])?;
    Ok(())
}

/// The fixture network (`spec::Family::Fixture`): source `s` is `spec[s]` times the
/// spectral branch's input plus `wave[s]` times the time branch's, so the separation of a
/// track is `(spec[s] + wave[s])` times the track — exactly, but for the STFT's rounding —
/// which tests the whole path around a real network.
pub struct Fixture {
    pub spec: Vec<f32>,
    pub wave: Vec<f32>,
    pub delay_ms: u64,
    pub crash_at: Option<usize>,
    pub panic_at: Option<usize>,
    pub hang_at: Option<usize>,
    pub runs: usize,
}

impl Network for Fixture {
    fn run(&mut self, mix: &[f32], mag: &[f32]) -> Result<(Vec<f32>, Vec<f32>), Stop> {
        if self.panic_at == Some(self.runs) {
            panic!("the fixture panics at segment {}", self.runs);
        }
        if self.crash_at == Some(self.runs) {
            std::process::abort();
        }
        if self.hang_at == Some(self.runs) {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
        self.runs += 1;
        if self.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        }
        let spec = self
            .spec
            .iter()
            .flat_map(|&a| mag.iter().map(move |v| a * v))
            .collect();
        let wave = self
            .wave
            .iter()
            .flat_map(|&b| mix.iter().map(move |v| b * v))
            .collect();
        Ok((spec, wave))
    }
}
