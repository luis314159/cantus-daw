//! Drums with Magenta's Onsets and Frames Drums (ADR 0086 §3): the network's onset
//! probabilities for kicks, snares, toms, hi-hats and cymbals; Cantus's own detector
//! (`cantus_dsp::onset::drums`) says whether a hat was open. Without the model every class
//! is the detector's.
//!
//! The network takes 44.1 kHz mono and gives 100 frames a second. Magenta trains it with
//! `drum_data_map='8-hit'`, which maps every hi-hat to pitch **46**; the third party's
//! conversion gathered pitch 42 (trained toward zero: it never answers) for the hat, so
//! Cantus edits the gather at install to take 46 (`manifest/models.json`, `prepare.patch`).
//! The outputs are then 36 kick, 38 snare, 48 toms, 46 hi-hats, 51 ride, 53 ride bell (and
//! cowbell), 49 crash, 75 clave.
//! A hit is the first frame of a run above a class's threshold (Onsets and Frames' rule);
//! the thresholds, and a tom dropped when the kick fires harder beside it, were found on
//! MDB Drums' first eight tracks and Pulsus's kits (`tools/measure_oaf_drums.py`). Long
//! files go through in windows of a minute that overlap by two seconds, each frame taken
//! from the window it is furthest inside.

use crate::jobs::Stop;
use crate::proto::JobError;
use crate::runtime;
use crate::transcribe::spec::{DrumClass, DrumHit};
use cantus_dsp::onset::drums as d;
use std::sync::atomic::AtomicBool;

/// The network's rate and frame rate.
pub const RATE: u32 = 44_100;
pub const FPS: f64 = 100.0;
const HOP: usize = 441;
/// Its eight pitches, in its output order.
pub const PITCHES: [u8; 8] = [36, 38, 48, 46, 51, 53, 49, 75];
/// A window: a minute; windows overlap by two seconds.
const WINDOW: usize = 60 * RATE as usize;
const OVERLAP: usize = 2 * RATE as usize;

/// The network's answer for a whole file: `frames × 8`, row-major.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Probs {
    pub frames: usize,
    pub onset: Vec<f32>,
    pub velocity: Vec<f32>,
}

/// The thresholds at sensitivity 0.5 (kick, snare, toms, cymbals) and the tom rule.
const THRESHOLD: [f32; 4] = [0.3, 0.4, 0.3, 0.3];
/// The hi-hat's threshold at sensitivity 0.5 (the same search).
const HAT_THRESHOLD: f32 = 0.2;
/// A tom is the kick's when the kick's probability beside it exceeds the tom's times this.
const TOM_LEAK: f32 = 0.5;

/// Run the network over `x` (44.1 kHz mono), window by window.
pub fn probs(
    model: &mut runtime::Model<'_>,
    x: &[f32],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f32),
) -> Result<Probs, Stop> {
    let frames = x.len() / HOP + 1;
    let mut out = Probs {
        frames,
        onset: vec![0.0; frames * 8],
        velocity: vec![0.0; frames * 8],
    };
    let step = WINDOW - OVERLAP;
    let mut start = 0usize;
    loop {
        let end = (start + WINDOW).min(x.len());
        // The network wants at least 2 049 samples.
        let mut chunk = x[start..end].to_vec();
        if chunk.len() < 2_049 {
            chunk.resize(2_049, 0.0);
        }
        let outs = model.run(
            &[runtime::Input {
                name: "waveform",
                shape: &[1, chunk.len()],
                data: &chunk,
            }],
            cancel,
        )?;
        let take = |name: &str| -> Result<&runtime::Output, Stop> {
            outs.iter().find(|o| o.name == name).ok_or_else(|| {
                Stop::Failed(JobError::Model(format!(
                    "the drum model has no output {name}"
                )))
            })
        };
        let (on, vel) = (take("onset_probs")?, take("velocity")?);
        let n = on.data.len() / 8;
        // This window owns its frames from half the overlap in (but the first window's
        // from 0) to half the overlap before its end (but the last's to its end).
        let first = start / HOP;
        let lo = if start == 0 { 0 } else { OVERLAP / 2 / HOP };
        let last = end >= x.len();
        let hi = if last {
            n
        } else {
            n.saturating_sub(OVERLAP / 2 / HOP)
        };
        for i in lo..hi {
            let f = first + i;
            if f >= frames {
                break;
            }
            out.onset[f * 8..f * 8 + 8].copy_from_slice(&on.data[i * 8..i * 8 + 8]);
            out.velocity[f * 8..f * 8 + 8].copy_from_slice(&vel.data[i * 8..i * 8 + 8]);
        }
        progress(end as f32 / x.len().max(1) as f32);
        if last {
            break;
        }
        start += step;
    }
    Ok(out)
}

/// A sensitivity's threshold for a class whose default is `base`.
fn threshold(base: f32, sensitivity: f32) -> f32 {
    (base - 0.4 * (sensitivity - 0.5)).clamp(0.05, 0.95)
}

/// The first frame of each run above `thr` in column `col`.
fn runs(p: &Probs, col: impl Fn(usize) -> f32, thr: f32) -> Vec<usize> {
    let mut out = Vec::new();
    let mut was = false;
    for f in 0..p.frames {
        let on = col(f) > thr;
        if on && !was {
            out.push(f);
        }
        was = on;
    }
    out
}

fn velocity(v: f32) -> u8 {
    ((v.clamp(0.0, 1.0) * 127.0) as u8).max(1)
}

/// The hits: the network's kicks, snares, toms, hi-hats and cymbals (`x` at `rate`, the
/// file the network heard), named on the studio kit's pads; the detector says whether a
/// hat was open. `None` when `cancelled` said so while the detector ran (it is asked at
/// each of the detector's factorisation steps).
pub fn hybrid(
    x: &[f32],
    rate: u32,
    sensitivity: f32,
    p: &Probs,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<DrumHit>> {
    let at = |f: usize, j: usize| p.onset[f * 8 + j];
    let vel = |f: usize, j: usize| velocity(p.velocity[f * 8 + j]);
    let mut out = Vec::new();
    let spec = d::spectrogram(x, rate as f32);
    for f in runs(p, |f| at(f, 0), threshold(THRESHOLD[0], sensitivity)) {
        out.push((f, DrumClass::Kick, 36, vel(f, 0)));
    }
    for f in runs(p, |f| at(f, 1), threshold(THRESHOLD[1], sensitivity)) {
        out.push((f, DrumClass::Snare, 38, vel(f, 1)));
    }
    for f in runs(p, |f| at(f, 2), threshold(THRESHOLD[2], sensitivity)) {
        let (lo, hi) = (f.saturating_sub(3), (f + 4).min(p.frames));
        let kick = (lo..hi).map(|g| at(g, 0)).fold(0.0f32, f32::max);
        let tom = (lo..hi).map(|g| at(g, 2)).fold(0.0f32, f32::max);
        if kick > TOM_LEAK * tom {
            continue;
        }
        let key = match d::tom_kind_at(&spec, f as f64 / FPS) {
            d::Kind::LowTom => 45,
            d::Kind::HighTom => 50,
            _ => 47,
        };
        out.push((f, DrumClass::Toms, key, vel(f, 2)));
    }
    let cym = |f: usize| at(f, 4).max(at(f, 5)).max(at(f, 6));
    for f in runs(p, cym, threshold(THRESHOLD[3], sensitivity)) {
        // Which cymbal: the one the network is surest of over the next 30 ms. The ride
        // bell (and the cowbell Magenta folds into it) plays the ride: the studio kit has
        // no bell pad.
        let (j, _) = [4usize, 5, 6]
            .iter()
            .map(|&j| {
                (
                    j,
                    (f..(f + 3).min(p.frames))
                        .map(|g| at(g, j))
                        .fold(0.0, f32::max),
                )
            })
            .fold((6, -1.0f32), |b, c| if c.1 > b.1 { c } else { b });
        let key = if j == 6 { 49 } else { 51 };
        out.push((f, DrumClass::Cymbals, key, vel(f, j)));
    }
    // The detector's hats say which of the network's were open.
    let ours = d::transcribe_until(
        x,
        rate as f32,
        sensitivity,
        &d::Tuning::default(),
        cancelled,
    )?;
    for f in runs(p, |f| at(f, 3), threshold(HAT_THRESHOLD, sensitivity)) {
        let t = f as f64 / FPS;
        let open = ours.iter().any(|h| {
            h.class == d::Class::HiHat && h.kind == d::Kind::OpenHat && (h.time - t).abs() <= 0.04
        });
        out.push((f, DrumClass::HiHat, if open { 46 } else { 42 }, vel(f, 3)));
    }
    let mut hits: Vec<DrumHit> = out
        .into_iter()
        .map(|(f, class, key, velocity)| DrumHit {
            time: f as f64 / FPS,
            class,
            key,
            velocity,
        })
        .collect();
    hits.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.class.cmp(&b.class)));
    Some(hits)
}

/// Every key [`hybrid`] and the detector write: the studio kit has a pad for each.
pub const KEYS: [u8; 9] = [36, 38, 42, 45, 46, 47, 49, 50, 51];
