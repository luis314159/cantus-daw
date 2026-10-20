//! Audio clips in a `.dawproject` (round 29, ADR 0061). A clip that **follows the tempo**
//! (warped) is written as `<Warps>` over `<Audio>`: its markers as `<Warp time
//! contentTime>` pairs (beats of the clip's content against seconds of the file), its mode
//! as the `algorithm`. A clip at its **natural speed** (not warped, or not following) is a
//! bare `<Audio>` in a clip whose content is in seconds, its `playStart` the second of the
//! file at its left edge — so it plays at natural speed in any application, whatever the
//! tempo map (round 29's review: a natural line at the first tempo made it follow the
//! tempo elsewhere). Each file goes into the archive once (stored, not deflated), or is
//! referenced where it is (its absolute path) when it cannot be read. Read back the same
//! way from any application's file (Bitwig and Studio One write them so).

use super::xml::{a, num, XmlWriter};
use crate::report::Report;
use cantus_core::warp::{Warp, WarpMarker, WarpMode, WarpTail};
use cantus_core::{AudioRegion, AudioSource, Clip, Project, Ticks, PPQ};
use std::path::{Path, PathBuf};

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

/// Whether a clip plays at its natural speed (written as bare `<Audio>`).
pub fn natural(a: &AudioRegion) -> bool {
    a.warp.as_deref().is_none_or(|w| !w.follow)
}

/// The seconds of the file a natural-speed clip shows at its left edge (and where its
/// loop ends, when it repeats): through the project's tempo map, its fine offset added.
pub fn seconds_of(project: &Project, c: &Clip, a: &AudioRegion, rate: u32) -> (f64, Option<f64>) {
    let map = project.tempo_map();
    let slip = a.slip_frames as f64 / rate.max(1) as f64;
    let start = cantus_core::audio_time::offset_seconds(&map, c.start, c.offset) + slip;
    let end = c.period().map(|p| {
        cantus_core::audio_time::source_seconds(&map, c.start, c.offset, (c.start + p).0 as f64)
            + slip
    });
    (start, end)
}

/// The warp points of a clip that follows the tempo: its markers (beats of content, seconds
/// of the file), and its ends where the markers do not reach them.
pub fn points(w: &Warp, s: &AudioSource) -> Vec<(f64, f64)> {
    let rate = s.sample_rate.max(1) as f64;
    let end = s.frames as f64;
    let mut pts: Vec<(f64, f64)> = w
        .markers
        .iter()
        .map(|m| (m.tick.0 as f64 / PPQ as f64, m.frame / rate))
        .collect();
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

/// Where a source's file is on disk: its stored path, a relative one against `base`.
pub fn file_of(s: &AudioSource, base: Option<&Path>) -> PathBuf {
    let stored = Path::new(&s.path);
    match base {
        Some(b) if stored.is_relative() => b.join(stored),
        _ => stored.to_path_buf(),
    }
}

/// Write `c`'s audio content (inside its `<Clip>`). `base` is the project's folder (a
/// stored path is relative to it); a file that can be read goes into the archive as
/// `audio/<id> <name>` — listed once in `audio` (archive path, file on disk), read when the
/// archive is written — else it is referenced at its absolute path, and said.
#[allow(clippy::too_many_arguments)]
pub fn write(
    w: &mut XmlWriter,
    next_id: &mut dyn FnMut() -> String,
    project: &Project,
    c: &Clip,
    au: &AudioRegion,
    base: Option<&Path>,
    audio: &mut Vec<(String, PathBuf)>,
    report: &mut Report,
) {
    let Some(s) = project.audio_source(au.source) else {
        return;
    };
    let natural = natural(au);
    let warp = au.warp.as_deref().filter(|w| w.follow);
    let mode = warp.map_or(WarpMode::Repitch, |w| w.mode);
    if !natural {
        w.open(
            "Warps",
            &[
                ("contentTimeUnit", Some("seconds".into())),
                ("id", a(next_id())),
            ],
        );
    }
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
    let file = file_of(s, base);
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio.wav".into());
    let inner = format!("audio/{} {name}", s.id.0);
    if audio.iter().any(|(p, _)| *p == inner) || file.is_file() {
        if !audio.iter().any(|(p, _)| *p == inner) {
            audio.push((inner.clone(), file.clone()));
        }
        w.empty("File", &[("path", a(&inner)), ("external", a("false"))]);
    } else {
        let at = file.to_string_lossy().into_owned();
        report.info(
            "interop.report.x_audio_external",
            vec![("file", at.clone())],
        );
        w.empty("File", &[("path", a(&at)), ("external", a("true"))]);
    }
    w.close(); // Audio
    if let Some(wp) = warp {
        for (t, secs) in points(wp, s) {
            w.empty(
                "Warp",
                &[("time", a(num(t))), ("contentTime", a(num(secs)))],
            );
        }
        w.close(); // Warps
    }
    dropped(c, au, report);
}

/// What DAWproject has no words for, said (never dropped in silence, ADR 0021).
fn dropped(c: &Clip, au: &AudioRegion, report: &mut Report) {
    let clip = || vec![("clip", c.name.clone())];
    if let Some(w) = au.warp.as_deref() {
        if w.cents != 0 && w.mode.transposes() {
            report.dropped("interop.report.x_warp_transpose", clip());
            if !w.formants {
                report.dropped("interop.report.x_warp_formants", clip());
            }
        }
        if w.follow && w.mode == WarpMode::Rhythm && w.tail != WarpTail::Fade {
            report.dropped("interop.report.x_warp_tail", clip());
        }
        if !w.follow && w.markers.len() > 1 {
            report.dropped("interop.report.x_warp_markers", clip());
        }
    }
    if au.pitch.as_deref().is_some_and(|p| p.edited()) {
        report.dropped("interop.report.x_pitch_edits", clip());
    }
    if au.reverse {
        report.dropped("interop.report.x_reverse", clip());
    }
    if au.gain_db.abs() >= 0.005 {
        report.dropped("interop.report.x_clip_gain", clip());
    }
    let curve = cantus_core::audio::FadeCurve::default();
    if (au.fade_in > Ticks::ZERO && au.fade_in_curve != curve)
        || (au.fade_out > Ticks::ZERO && au.fade_out_curve != curve)
    {
        report.dropped("interop.report.x_fade_curve", clip());
    }
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

/// The region an imported audio clip plays: its warp from the points. Points that lie on
/// the natural line at `bpm` come back unwarped (the fine offset kept) only when `collapse`
/// — the project has one tempo; under a tempo map they stay a warp that follows it (round
/// 29's review). No points: natural speed (the caller places it from its `playStart`).
pub fn region(
    source: cantus_core::AudioSourceId,
    d: &AudioData,
    bpm: f64,
    collapse: bool,
) -> AudioRegion {
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
    let on_line = pts
        .iter()
        .all(|(b, s)| ((b * 60.0 / bpm + slip) - s).abs() * rate < 0.5);
    if on_line && collapse {
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
        let m = WarpMarker::at(s * rate, Ticks((b * PPQ as f64).round() as i64));
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
        path: attr(file, "path")?.trim_start_matches("./").to_string(),
        external: attr(file, "external") == Some("true"),
        channels: attr_f64(audio, "channels").unwrap_or(2.0).clamp(1.0, 2.0) as u8,
        rate: attr_f64(audio, "sampleRate").unwrap_or(48_000.0).max(1.0) as u32,
        seconds: attr_f64(audio, "duration").unwrap_or(0.0).max(0.0),
        algorithm: attr(audio, "algorithm").map(str::to_string),
        warps,
    })
}
