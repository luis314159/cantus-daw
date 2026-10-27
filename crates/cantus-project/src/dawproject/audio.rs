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
//!
//! **Cantus's own warp** goes beside each `<Warps>` as `cantus/warps/<id>.json` (its
//! markers with the warp's own flagged, Follow, the transposition, the formants, the tail,
//! the fine offset): a file Cantus wrote comes back with the warp it had, while any other
//! application reads the `<Warp>` points (round 29's third review: the warp's own markers
//! came back as the user's, the barcode with them).

use super::xml::{a, num, XmlWriter};
use crate::report::Report;
use cantus_core::warp::{Warp, WarpMarker, WarpMode, WarpTail};
use cantus_core::{AudioRegion, AudioSource, Clip, Project, Ticks, PPQ};
use serde::{Deserialize, Serialize};
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

/// What Cantus writes beside a `<Warps>` ([`side_path`]): the clip's warp as it is, and its
/// fine offset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Side {
    pub warp: Warp,
    #[serde(default)]
    pub slip_frames: i32,
}

/// The archive path of the warp beside the `<Warps>` whose id is `id`.
pub fn side_path(id: &str) -> String {
    format!("cantus/warps/{id}.json")
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
    files: &mut Vec<(String, Vec<u8>)>,
    report: &mut Report,
) {
    let Some(s) = project.audio_source(au.source) else {
        return;
    };
    let natural = natural(au);
    let warp = au.warp.as_deref().filter(|w| w.follow);
    let mode = warp.map_or(WarpMode::Repitch, |w| w.mode);
    if !natural {
        let id = next_id();
        if let Some(wp) = warp {
            let side = Side {
                warp: wp.clone(),
                slip_frames: au.slip_frames,
            };
            if let Ok(json) = serde_json::to_vec(&side) {
                files.push((side_path(&id), json));
            }
        }
        w.open(
            "Warps",
            &[("contentTimeUnit", Some("seconds".into())), ("id", a(id))],
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
    /// The `<Warps>` element's id (Cantus's own warp beside it: [`side_path`]).
    pub id: Option<String>,
}

/// Whether warp points `pts` (content beats, file seconds; sorted) lie on the natural line
/// — the content at natural speed from timeline tick `origin` through `map`, moved by
/// `slip` seconds — to within half a frame at `rate`: at every point, and along every
/// straight segment between two (at each tempo point inside it, and through a ramp at
/// samples along it) — round 29's third review: points alone missed a segment across a
/// tempo step whose ends happened to land on the curve.
fn on_natural_line(
    pts: &[(f64, f64)],
    map: &cantus_core::tempo::TempoMap,
    origin: f64,
    slip: f64,
    rate: f64,
) -> bool {
    let natural = |b: f64| map.seconds_between_f64(origin, origin + b * PPQ as f64);
    let on = |b: f64, s: f64| ((natural(b) + slip) - s).abs() * rate < 0.5;
    if !pts.iter().all(|(b, s)| on(*b, *s)) {
        return false;
    }
    pts.windows(2).all(|w| {
        let ((b0, s0), (b1, s1)) = (w[0], w[1]);
        if b1 <= b0 {
            return true;
        }
        let line = |b: f64| s0 + (s1 - s0) * (b - b0) / (b1 - b0);
        let (t0, t1) = (origin + b0 * PPQ as f64, origin + b1 * PPQ as f64);
        let mut corners = vec![t0, t1];
        let mut t = t0;
        while let Some(n) = map.next_change_after(t).filter(|n| *n < t1) {
            corners.push(n);
            t = n;
        }
        corners.sort_by(f64::total_cmp);
        let beat = |t: f64| (t - origin) / PPQ as f64;
        corners.iter().all(|t| on(beat(*t), line(beat(*t))))
            && corners.windows(2).all(|c| {
                // A ramp between two corners: its curve sampled.
                let mid = 0.5 * (c[0] + c[1]);
                !map.in_ramp(mid)
                    || (1..32).all(|k| {
                        let t = c[0] + (c[1] - c[0]) * k as f64 / 32.0;
                        on(beat(t), line(beat(t)))
                    })
            })
    })
}

/// The region an imported audio clip plays: its warp from the points. Points that lie on
/// the clip's natural line — its content at natural speed from its content origin
/// (`origin`, a timeline tick) through the tempo map `map`, to within half a frame, along
/// every segment ([`on_natural_line`]) — come back unwarped, the fine offset kept (round 29's
/// second review: a line at one tempo was taken for natural speed under a map that
/// changes); from a file Cantus wrote (`ours`: every `<Warps>` of it follows the tempo),
/// as the warp that follows from natural speed (`Warp::natural_of` over `window`, the
/// clip's start and length — third review: it came back not following). The second value
/// says a `<Warps>` was read as natural speed. No points: natural speed (the caller places
/// it from its `playStart`).
pub fn region(
    source: cantus_core::AudioSourceId,
    d: &AudioData,
    map: &cantus_core::tempo::TempoMap,
    origin: f64,
    ours: Option<(Ticks, Ticks)>,
) -> (AudioRegion, bool) {
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
        return (r, false);
    }
    // On the natural line: secs = the seconds from the origin to the content beat, through
    // the map, + slip.
    let natural = |b: f64| map.seconds_between_f64(origin, origin + b * PPQ as f64);
    let slip = pts[0].1 - natural(pts[0].0);
    if on_natural_line(&pts, map, origin, slip, rate) {
        r.slip_frames = (slip * rate).round() as i32;
        if let Some((start, length)) = ours {
            let offset = start - Ticks(origin.round() as i64);
            let mut w = Warp::natural_of(map, start, offset, length, r.slip_frames, d.rate.max(1));
            w.mode = mode_of(d.algorithm.as_deref());
            if w.is_valid() {
                r.warp = Some(Box::new(w));
                return (r, false);
            }
        }
        return (r, true);
    }
    let bpm = map.bpm_at(origin);
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
    (r, false)
}

/// Read a clip's `<Warps>` (over `<Audio>`) or bare `<Audio>` content; `None` for anything
/// it cannot place (no file, warp points in beats of the file). `to_beats` turns a warp
/// point's time in seconds (of the clip's content, from its start) into beats of it —
/// through the tempo map from the clip's content origin (round 29's second review: at one
/// tempo, it misplaced every point after a tempo change).
pub fn read(
    n: roxmltree::Node,
    to_beats: &dyn Fn(f64) -> f64,
    beats_in_seconds: bool,
) -> Option<AudioData> {
    use super::read::{attr, attr_f64, child};
    let id = n
        .has_tag_name("Warps")
        .then(|| attr(n, "id").map(str::to_string))
        .flatten();
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
                .map(|(t, s)| (if secs_time { to_beats(t) } else { t }, s))
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
        id,
    })
}

// ---- crossfades and gain envelopes (round 31, ADR 0070) ------------------------------

/// What DAWproject says of a clip's crossfades: the length of the overlap it crosses in
/// (its fade in) and out (its fade out), and the curves.
pub type Crossings = (
    Option<Ticks>,
    Option<Ticks>,
    Vec<cantus_core::crossfade::XfadeCurve>,
);

/// The crossings of a track's clips, by clip (ADR 0070 §9): a crossed overlap is a fade in
/// on the later window's first piece and a fade out on the earlier one's last, each the
/// overlap's length. Found once per track (review 1: not once per clip).
pub fn crossings_of_track(
    t: &cantus_core::Track,
) -> std::collections::HashMap<cantus_core::ClipId, Crossings> {
    let mut out: std::collections::HashMap<cantus_core::ClipId, Crossings> =
        std::collections::HashMap::new();
    for o in cantus_core::crossfade::overlaps_of_track(t) {
        let Some(curve) = o.curve else {
            continue;
        };
        let len = o.to - o.from;
        let e = out.entry(o.right_first).or_default();
        e.0 = Some(len);
        e.2.push(curve);
        let e = out.entry(o.left_last).or_default();
        e.1 = Some(len);
        e.2.push(curve);
    }
    out
}

/// Write a clip's gain envelope as DAWproject's clip-level gain expression, beside its
/// audio in the clip's `<Lanes>`: points in the clip's content beats, linear gain, linear
/// or held between them (the shapes DAWproject has; the others are reported).
pub fn write_gain(
    w: &mut XmlWriter,
    next_id: &mut dyn FnMut() -> String,
    c: &Clip,
    env: &cantus_core::clip_gain::GainEnvelope,
    report: &mut Report,
) {
    use cantus_core::Shape;
    w.open(
        "Points",
        &[
            ("id", a(next_id())),
            ("unit", a("linear")),
            ("timeUnit", a("beats")),
        ],
    );
    w.empty("Target", &[("expression", a("gain"))]);
    let mut bent = false;
    for (i, p) in env.points.iter().enumerate() {
        let last = i + 1 == env.points.len();
        let hold = p.shape == Shape::Hold;
        bent |= !last && !hold && (p.shape != Shape::Curve || p.tension != 0.0);
        w.empty(
            "RealPoint",
            &[
                ("time", a(num(p.tick.0 as f64 / PPQ as f64))),
                (
                    "value",
                    a(num(f64::from(cantus_core::clip_gain::gain_of(p.db)))),
                ),
                ("interpolation", a(if hold { "hold" } else { "linear" })),
            ],
        );
    }
    w.close(); // Points
    if bent {
        report.dropped("clipfx.report.env_shape", vec![("clip", c.name.clone())]);
    }
}

/// A clip's gain envelope read from a `<Points>` whose target is the gain expression:
/// `to_ticks` turns its times into content ticks; linear gain or dB (`unit`).
pub fn read_gain(
    points: roxmltree::Node,
    to_ticks: &dyn Fn(f64) -> Ticks,
) -> Option<cantus_core::clip_gain::GainEnvelope> {
    use cantus_core::clip_gain::{GainEnvelope, GainPoint, ENV_MAX_DB, ENV_MIN_DB};
    let gain = points
        .children()
        .find(|c| c.has_tag_name("Target"))
        .is_some_and(|t| t.attribute("expression") == Some("gain"));
    if !gain {
        return None;
    }
    let db_of = |v: f64| -> f32 {
        match points.attribute("unit") {
            Some("decibel") => v as f32,
            _ if v <= 0.0 => ENV_MIN_DB,
            _ => (20.0 * v.log10()) as f32,
        }
    };
    let mut out = Vec::new();
    for p in points.children().filter(|c| c.has_tag_name("RealPoint")) {
        let (Some(time), Some(v)) = (
            p.attribute("time").and_then(|x| x.parse::<f64>().ok()),
            p.attribute("value").and_then(|x| x.parse::<f64>().ok()),
        ) else {
            continue;
        };
        if !time.is_finite() || !v.is_finite() {
            continue;
        }
        let mut g = GainPoint::new(to_ticks(time), db_of(v).clamp(ENV_MIN_DB, ENV_MAX_DB));
        if p.attribute("interpolation") == Some("hold") {
            g.shape = cantus_core::Shape::Hold;
        }
        out.push(g);
    }
    let env = GainEnvelope::new(out);
    (!env.points.is_empty() && env.is_valid()).then_some(env)
}

/// After a track's clips are read: two overlapping audio clips whose fades both span the
/// overlap — the later's in, the earlier's out — are a crossfade (DAWproject's way of
/// saying one), crossed on the linear curve its fades meant.
pub fn crossfades_from_fades(track: &mut cantus_core::Track) {
    use cantus_core::crossfade::XfadeCurve;
    let near = |a: Ticks, b: Ticks| (a.0 - b.0).abs() <= 1;
    let mut crossed = Vec::new();
    for o in cantus_core::crossfade::overlaps_of_track(track) {
        let len = o.to - o.from;
        let (Some(ra), Some(la)) = (
            track.clip(o.right_first).and_then(|c| c.audio.as_ref()),
            track.clip(o.left_last).and_then(|c| c.audio.as_ref()),
        ) else {
            continue;
        };
        if near(ra.fade_in, len) && near(la.fade_out, len) {
            crossed.push((o.right_first, o.left_last));
        }
    }
    for (r, l) in crossed {
        if let Some(a) = track.clip_mut(r).and_then(|c| c.audio.as_mut()) {
            a.cross = Some(XfadeCurve::Linear);
            a.fade_in = Ticks::ZERO;
        }
        if let Some(a) = track.clip_mut(l).and_then(|c| c.audio.as_mut()) {
            a.fade_out = Ticks::ZERO;
        }
    }
}
