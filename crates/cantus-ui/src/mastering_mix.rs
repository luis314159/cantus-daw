//! What the listening tools answer (round 34, ADR 0083): an audition's numbers, each
//! track's place in the mix, who masks whom in which band, and the trims that bring tracks
//! to their loudness targets. Pure functions of what a listen heard, so the tests read
//! them without a host.

use super::listening::{
    clicks_json, level, reading_between, reading_json, resumed_json, round, spectrum_json,
    tick_json, Range,
};
use cantus_core::{Project, Ticks, TrackId};
use cantus_dsp::speculum::ear::{octave_hz, Ear, OCTAVES, THIRDS};
use cantus_engine::listen::Heard;
use serde_json::{json, Value};

/// The range a listen covered, as the agent reads it.
pub fn range_json(project: &Project, range: Range) -> Value {
    let end = range.end.unwrap_or_else(|| project.content_end());
    json!({
        "start": tick_json(project, range.start),
        "end": tick_json(project, end),
    })
}

/// How the render went: where it began, how much the engine rendered, checkpoints taken.
pub fn rendered_json(project: &Project, h: &Heard) -> Value {
    let mut v = resumed_json(project, &h.resumed);
    v["seconds_rendered"] = json!(round(h.rendered_frames as f64 / h.sample_rate as f64, 2));
    v["checkpoints_taken"] = json!(h.checkpoints_taken);
    v["latency_samples"] = json!(h.latency);
    v
}

/// An audition's answer (the WAV's place added by the caller).
pub fn audition_json(
    project: &Project,
    range: Range,
    h: &Heard,
    reference: Option<(&str, &[f32; OCTAVES])>,
) -> Value {
    let whole = h.master.reading(0, h.range_frames as u64);
    let bounds = super::listening::boundaries(project, range);
    let sections: Vec<Value> = bounds
        .windows(2)
        .map(|w| {
            let r = reading_between(&h.master, project, h.sample_rate, range.start, w[0], w[1]);
            json!({
                "start": tick_json(project, w[0]),
                "end": tick_json(project, w[1]),
                "name": project.markers.iter().find(|m| m.tick == w[0]).map(|m| m.name.clone()),
                "integrated_lufs": r.integrated_lufs.map_or(Value::Null, level),
                "short_term_max_lufs": r.short_term_max_lufs.map_or(Value::Null, level),
                "true_peak_dbtp": level(r.true_peak_dbtp as f64),
                "range_lu": r.range_lu.map_or(Value::Null, level),
            })
        })
        .collect();
    json!({
        "range": range_json(project, range),
        "rendered": rendered_json(project, h),
        "loudness": reading_json(&whole),
        "sections": sections,
        "spectrum": {
            "relative_to": "1 kHz octave",
            "reference": reference.map(|r| r.0),
            "octaves": spectrum_json(&whole, reference.map(|r| r.1)),
        },
        "clicks": clicks_json(&h.master, project, h.sample_rate, range),
    })
}

/// The loudest short-term (3 s) moment of `ear` in its first `frames`, as a start tick.
fn loudest_moment(ear: &Ear, frames: u64) -> Option<(usize, f64)> {
    let cols = ear.columns();
    let n = ((frames / ear.column_len() as u64) as usize).min(cols.len());
    if n < 300 {
        return None;
    }
    let mut sum: f64 = cols[..300].iter().map(|c| c.k).sum();
    let mut best = (0, sum);
    for i in 300..n {
        sum += cols[i].k - cols[i - 300].k;
        if sum > best.1 {
            best = (i + 1 - 300, sum);
        }
    }
    Some(best)
}

/// Each track's place in the mix: its loudness against the mix's, its spectrum, crest,
/// peak, and its loudest three seconds. `tracks` are snapshot track indices, the taps'
/// order in `h`.
pub fn tracks_json(project: &Project, range: Range, h: &Heard, tracks: &[usize]) -> Value {
    let mix = h.master.reading(0, h.range_frames as u64);
    let mix_i = mix.integrated_lufs;
    let map = project.tempo_map();
    let to_tick = |col: usize| {
        let secs = col as f64 / 100.0 + map.seconds_at(range.start.0 as f64);
        Ticks(map.tick_at_seconds(secs).round() as i64)
    };
    let rows: Vec<Value> = tracks
        .iter()
        .zip(&h.taps)
        .map(|(&ti, (_, ear))| {
            let t = &project.tracks[ti];
            let r = ear.reading(0, h.range_frames as u64);
            let share = match (r.integrated_lufs, mix_i) {
                (Some(a), Some(b)) => level(a - b),
                _ => Value::Null,
            };
            let octaves: Vec<Value> = (0..OCTAVES)
                .map(|o| json!({ "hz": round(octave_hz(o) as f64, 1), "db": level(r.octaves_db[o] as f64) }))
                .collect();
            let loudest = loudest_moment(ear, h.range_frames as u64)
                .map(|(col, _)| tick_json(project, to_tick(col)));
            json!({
                "track_id": t.id.0,
                "name": t.name,
                "loudness": reading_json(&r),
                "share_of_mix_lu": share,
                "octaves_db": octaves,
                "loudest_3s_from": loudest,
                "silent": r.integrated_lufs.is_none() && r.sample_peak_dbfs == f32::NEG_INFINITY,
            })
        })
        .collect();
    json!({
        "range": range_json(project, range),
        "rendered": rendered_json(project, h),
        "mix": reading_json(&mix),
        "tracks": rows,
    })
}

/// Per spectrum frame and band (thirds or octaves), each tap's mid power.
fn band_frames(ear: &Ear, octaves: bool) -> Vec<Vec<f32>> {
    ear.spectra()
        .iter()
        .map(|f| {
            if octaves {
                (0..OCTAVES)
                    .map(|o| {
                        let k = 2 + 3 * o;
                        f.mid[k - 1] + f.mid[k] + f.mid[(k + 1).min(THIRDS - 1)]
                    })
                    .collect()
            } else {
                f.mid.to_vec()
            }
        })
        .collect()
}

/// Who fights whom in which band (see the module notes of `ai_tools_listen`): in each
/// spectrum frame and band, two tracks *fight* when both sound within 15 dB of the band's
/// loudest track and within 6 dB of each other; the overlap is the share of the band's
/// energy over the range spent in such frames. The pairs come sorted by overlap.
pub fn masking_json(
    project: &Project,
    range: Range,
    h: &Heard,
    tracks: &[usize],
    octaves: bool,
    top: usize,
) -> Value {
    let frames: Vec<Vec<Vec<f32>>> = h
        .taps
        .iter()
        .map(|(_, e)| band_frames(e, octaves))
        .collect();
    let nf = frames.iter().map(Vec::len).min().unwrap_or(0);
    let bands = if octaves { OCTAVES } else { THIRDS };
    let hz = |b: usize| {
        if octaves {
            octave_hz(b)
        } else {
            cantus_dsp::speculum::ear::third_hz(b)
        }
    };
    let n = tracks.len();
    let mut pairs = Vec::new();
    let mut crowded = Vec::new();
    #[allow(clippy::needless_range_loop)] // bands and frames index the taps' frames
    for b in 0..bands {
        let mut fight = vec![0f64; n * n];
        let mut level_in = vec![0f64; n * n];
        let mut total = 0f64;
        let mut busy = 0usize;
        for f in 0..nf {
            let p: Vec<f32> = (0..n).map(|t| frames[t][f][b]).collect();
            let top_p = p.iter().copied().fold(0f32, f32::max);
            // Under −70 dB the band is silence here.
            if top_p < 1e-7 {
                continue;
            }
            total += top_p as f64;
            let near = top_p * 10f32.powf(-1.5);
            let active: Vec<usize> = (0..n).filter(|t| p[*t] >= near).collect();
            if active.len() >= 3 {
                busy += 1;
            }
            for (x, &i) in active.iter().enumerate() {
                for &j in &active[x + 1..] {
                    let (a, c) = (p[i].max(1e-30), p[j].max(1e-30));
                    if (a / c).max(c / a) <= 10f32.powf(0.6) {
                        fight[i * n + j] += top_p as f64;
                        level_in[i * n + j] += 10.0 * (a / c).log10() as f64 * top_p as f64;
                    }
                }
            }
        }
        if total <= 0.0 {
            continue;
        }
        crowded.push(json!({
            "hz": round(hz(b) as f64, 1),
            "frames_with_three_or_more": round(busy as f64 / nf.max(1) as f64, 3),
        }));
        for i in 0..n {
            for j in i + 1..n {
                let w = fight[i * n + j];
                if w <= 0.0 {
                    continue;
                }
                let overlap = w / total;
                let louder = level_in[i * n + j] / w;
                let who = |k: usize| {
                    let t = &project.tracks[tracks[k]];
                    json!({ "track_id": t.id.0, "name": t.name })
                };
                pairs.push((
                    overlap,
                    json!({
                        "a": who(i),
                        "b": who(j),
                        "band_hz": round(hz(b) as f64, 1),
                        "overlap": round(overlap, 3),
                        "a_over_b_db": round(louder, 1),
                    }),
                ));
            }
        }
    }
    pairs.sort_by(|x, y| y.0.total_cmp(&x.0));
    let list: Vec<Value> = pairs.into_iter().take(top.max(1)).map(|p| p.1).collect();
    json!({
        "range": range_json(project, range),
        "rendered": rendered_json(project, h),
        "bands": if octaves { "octave" } else { "third" },
        "rule": "two tracks fight in a band and moment when both are within 15 dB of the band's loudest track and within 6 dB of each other; overlap is the share of the band's energy spent so",
        "pairs": list,
        "crowded": crowded,
    })
}

/// One track's trim toward a loudness target: the gain it needs and what the model allows.
#[derive(Clone, Debug, PartialEq)]
pub struct Trim {
    pub track: TrackId,
    pub measured: Option<f64>,
    pub target: f64,
    pub old_gain_db: f32,
    pub new_gain_db: f32,
    /// The gain wanted lies past the model's limits (−96 … +12 dB): the trim stops there.
    pub limited: bool,
}

/// The trims that bring each track's channel loudness to its target (the channel's level is
/// linear, so one render solves it). A track silent in the range keeps its gain.
pub fn trims(project: &Project, targets: &[(TrackId, f64)], measured: &[Option<f64>]) -> Vec<Trim> {
    targets
        .iter()
        .zip(measured)
        .map(|(&(track, target), m)| {
            let old = project.track(track).map_or(0.0, |t| t.gain_db);
            let (new, limited) = match m {
                Some(m) => {
                    let want = old as f64 + (target - m);
                    let clamped = want.clamp(
                        cantus_core::MIN_GAIN_DB as f64,
                        cantus_core::MAX_GAIN_DB as f64,
                    );
                    (clamped as f32, (clamped - want).abs() > 1e-6)
                }
                None => (old, false),
            };
            Trim {
                track,
                measured: *m,
                target,
                old_gain_db: old,
                new_gain_db: new,
                limited,
            }
        })
        .collect()
}
