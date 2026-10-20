//! Audio clips in a `.dawproject` (round 29, ADR 0061): written as `<Warps>` over
//! `<Audio>` — the clip's file (in the archive when it can be read, else referenced where it
//! is), its warp markers as `<Warp time contentTime>` pairs (beats of the clip's content
//! against seconds of the file), its mode as the `algorithm` — and read back the same way
//! from any application's file (Bitwig and Studio One write them so). A clip at its natural
//! speed is two warp points on its natural line, which reads back unwarped.

use super::xml::{a, num, XmlWriter};
use crate::report::Report;
use cantus_core::warp::{Warp, WarpMarker, WarpMode};
use cantus_core::{AudioRegion, AudioSource, Clip, Project, Ticks, PPQ};
use std::path::Path;

/// The algorithm a mode is written as (Ableton's words, which Bitwig reads too).
pub fn algorithm(m: WarpMode) -> &'static str {
    match m {
        WarpMode::Complex => "complex",
        WarpMode::Rhythm => "beats",
        WarpMode::Melodic => "tones",
        WarpMode::Repitch => "repitch",
    }
}

/// The mode an algorithm name reads as (anything else: Complex).
pub fn mode_of(algorithm: Option<&str>) -> WarpMode {
    match algorithm.map(str::to_ascii_lowercase).as_deref() {
        Some("beats" | "rhythm" | "slice") => WarpMode::Rhythm,
        Some("tones" | "melodic" | "monophonic" | "solo") => WarpMode::Melodic,
        Some("repitch" | "re-pitch" | "varispeed" | "resample") => WarpMode::Repitch,
        _ => WarpMode::Complex,
    }
}

/// The warp points of a clip: its markers (beats of content, seconds of the file); a clip
/// not warped, its natural line at the project's tempo, from its first frame to its last.
pub fn points(project: &Project, a: &AudioRegion, s: &AudioSource) -> Vec<(f64, f64)> {
    let rate = s.sample_rate.max(1) as f64;
    let end = s.frames as f64;
    match a.warp.as_deref().filter(|w| w.follow) {
        Some(w) => {
            let mut pts: Vec<(f64, f64)> = w
                .markers
                .iter()
                .map(|m| (m.tick.0 as f64 / PPQ as f64, m.frame / rate))
                .collect();
            // Its ends, where the markers do not reach them.
            let t0 = w.tick_at(0.0);
            if pts.first().is_none_or(|p| p.1 > 0.0) {
                pts.insert(0, (t0 / PPQ as f64, 0.0));
            }
            let t1 = w.tick_at(end);
            if pts.last().is_none_or(|p| p.1 < end / rate) {
                pts.push((t1 / PPQ as f64, end / rate));
            }
            pts
        }
        None => {
            let slip = a.slip_frames as f64 / rate;
            let beats = |secs: f64| (secs - slip) * project.tempo_bpm / 60.0;
            vec![
                (beats(slip.max(0.0)), slip.max(0.0)),
                (beats(end / rate), end / rate),
            ]
        }
    }
}

/// Write `c`'s audio content (inside its `<Clip>`): `<Warps>` over `<Audio>`. `base` is the
/// project's folder (a stored path is relative to it); the file goes into the archive as
/// `audio/<id> <name>` when it can be read, `files` collecting it.
#[allow(clippy::too_many_arguments)]
pub fn write(
    w: &mut XmlWriter,
    next_id: &mut dyn FnMut() -> String,
    project: &Project,
    c: &Clip,
    au: &AudioRegion,
    base: Option<&Path>,
    files: &mut Vec<(String, Vec<u8>)>,
    report: &mut Report,
) {
    let Some(s) = project.audio_source(au.source) else {
        return;
    };
    let mode = au.warp.as_deref().map_or(WarpMode::Repitch, |w| {
        if w.follow {
            w.mode
        } else {
            WarpMode::Repitch
        }
    });
    w.open(
        "Warps",
        &[("contentTimeUnit", a_str("seconds")), ("id", a(next_id()))],
    );
    let seconds = s.frames as f64 / s.sample_rate.max(1) as f64;
    w.open(
        "Audio",
        &[
            ("algorithm", a(algorithm(mode))),
            ("channels", a(s.channels.max(1))),
            ("sampleRate", a(s.sample_rate)),
            ("duration", a(num(seconds))),
            ("id", a(next_id())),
        ],
    );
    let stored = Path::new(&s.path);
    let file = match base {
        Some(b) if stored.is_relative() => b.join(stored),
        _ => stored.to_path_buf(),
    };
    let name = stored
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio.wav".into());
    match std::fs::read(&file) {
        Ok(bytes) => {
            let inner = format!("audio/{} {name}", s.id.0);
            if !files.iter().any(|(p, _)| *p == inner) {
                files.push((inner.clone(), bytes));
            }
            w.empty("File", &[("path", a(&inner)), ("external", a("false"))]);
        }
        Err(_) => {
            report.info(
                "interop.report.x_audio_external",
                vec![("file", s.path.clone())],
            );
            w.empty("File", &[("path", a(&s.path)), ("external", a("true"))]);
        }
    }
    w.close(); // Audio
    for (t, secs) in points(project, au, s) {
        w.empty(
            "Warp",
            &[("time", a(num(t))), ("contentTime", a(num(secs)))],
        );
    }
    w.close(); // Warps
               // What DAWproject has no words for.
    if au
        .warp
        .as_deref()
        .is_some_and(|w| w.cents != 0 && w.mode.transposes())
    {
        report.dropped(
            "interop.report.x_warp_transpose",
            vec![("clip", c.name.clone())],
        );
    }
    if au.pitch.as_deref().is_some_and(|p| p.edited()) {
        report.dropped(
            "interop.report.x_pitch_edits",
            vec![("clip", c.name.clone())],
        );
    }
    if au.reverse {
        report.dropped("interop.report.x_reverse", vec![("clip", c.name.clone())]);
    }
}

fn a_str(s: &str) -> Option<String> {
    Some(s.to_string())
}

/// What a `<Warps>` or `<Audio>` clip content holds.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioData {
    /// The file: a path in the archive (`external` false) or on disk.
    pub path: String,
    pub external: bool,
    pub channels: u8,
    pub rate: u32,
    pub seconds: f64,
    pub algorithm: Option<String>,
    /// (content beats, file seconds), in order; empty: natural speed.
    pub warps: Vec<(f64, f64)>,
}

/// The region an imported audio clip plays: its warp from the points (none when they lie
/// on the natural line at `bpm` — the clip plays as recorded, its fine offset kept).
pub fn region(source: cantus_core::AudioSourceId, d: &AudioData, bpm: f64) -> AudioRegion {
    let mut r = AudioRegion::of(source);
    let rate = d.rate.max(1) as f64;
    let mut pts: Vec<(f64, f64)> = d
        .warps
        .iter()
        .copied()
        .filter(|p| p.0.is_finite() && p.1.is_finite())
        .collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    if pts.is_empty() {
        return r;
    }
    // On the natural line: secs = beats · 60 / bpm + slip.
    let slip = pts[0].1 - pts[0].0 * 60.0 / bpm;
    let natural = pts
        .iter()
        .all(|(b, s)| ((b * 60.0 / bpm + slip) - s).abs() * rate < 0.5);
    if natural {
        r.slip_frames = (slip * rate).round() as i32;
        return r;
    }
    let (b0, s0) = pts[0];
    let (b1, s1) = pts[pts.len() - 1];
    let bpm_src = if s1 > s0 && b1 > b0 {
        ((b1 - b0) / (s1 - s0) * 60.0).clamp(cantus_core::warp::MIN_BPM, cantus_core::warp::MAX_BPM)
    } else {
        bpm
    };
    let mut w = Warp::new(d.rate.max(1), bpm_src);
    w.mode = mode_of(d.algorithm.as_deref());
    for (b, s) in pts {
        let m = WarpMarker {
            frame: s * rate,
            tick: Ticks((b * PPQ as f64).round() as i64),
        };
        if let Some(n) = w.with_marker(m.frame, m.tick) {
            w = n;
        }
    }
    if w.is_valid() {
        r.warp = Some(Box::new(w));
    }
    r
}

/// Read a clip's `<Warps>` (over `<Audio>`) or bare `<Audio>` content; `None` for anything
/// it cannot place (no file, warp points in beats of the file).
pub fn read(n: roxmltree::Node, bpm: f64, beats_in_seconds: bool) -> Option<AudioData> {
    use super::read::{attr, attr_f64, child};
    let (audio, warps) = match n.tag_name().name() {
        "Warps" => {
            if attr(n, "contentTimeUnit") != Some("seconds") {
                return None;
            }
            // The warp points' own unit: the Warps' or what it inherits.
            let secs_time = match attr(n, "timeUnit") {
                Some(u) => u == "seconds",
                None => beats_in_seconds,
            };
            let pts = n
                .children()
                .filter(|c| c.has_tag_name("Warp"))
                .filter_map(|c| Some((attr_f64(c, "time")?, attr_f64(c, "contentTime")?)))
                .map(|(t, s)| (if secs_time { t * bpm / 60.0 } else { t }, s))
                .collect();
            (child(n, "Audio")?, pts)
        }
        "Audio" => (n, Vec::new()),
        _ => return None,
    };
    let file = child(audio, "File")?;
    Some(AudioData {
        path: attr(file, "path")?.to_string(),
        external: attr(file, "external") == Some("true"),
        channels: attr_f64(audio, "channels").unwrap_or(2.0).clamp(1.0, 2.0) as u8,
        rate: attr_f64(audio, "sampleRate").unwrap_or(48_000.0).max(1.0) as u32,
        seconds: attr_f64(audio, "duration").unwrap_or(0.0).max(0.0),
        algorithm: attr(audio, "algorithm").map(str::to_string),
        warps,
    })
}
