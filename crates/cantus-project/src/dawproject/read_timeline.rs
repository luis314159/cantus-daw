//! The arrangement of a `.dawproject`: clips of notes by track, and a count of everything
//! else the timeline holds (audio, automation, markers, the clip launcher).
//!
//! A timeline element may name its track (`track="…"`) and its unit (`timeUnit`); both are
//! inherited by what it contains. A clip's own content is in the clip's content time: its
//! `playStart` is the content shown at its left edge — Cantus's `offset` — and its loop is
//! `loopStart`…`loopEnd` of that content — Cantus's period, when the loop starts where
//! playing does.

use super::read::{attr, attr_f64, child, parse_color};
use crate::report::Report;
use cantus_core::{Color, Project, Ticks, PPQ};
use roxmltree::Node;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct NoteData {
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: u8,
    pub velocity: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipData {
    pub name: String,
    pub color: Option<Color>,
    pub start: Ticks,
    pub length: Ticks,
    pub offset: Ticks,
    pub period: Option<Ticks>,
    pub muted: bool,
    pub notes: Vec<NoteData>,
    /// An audio clip's file and warp (round 29, ADR 0061).
    pub audio: Option<super::audio::AudioData>,
    /// An audio clip's fades (in, out).
    pub fades: (Ticks, Ticks),
}

/// The note clips of every track, by the track's id.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub clips: HashMap<String, Vec<ClipData>>,
    /// Tracks whose notes are all on MIDI channel 10: drum parts.
    pub drums: HashSet<String>,
    /// The tempo automation (round 29, ADR 0060): where, the tempo, whether it ramps.
    pub tempo: Vec<(Ticks, f64, bool)>,
    /// The time-signature automation: where, numerator, denominator.
    pub metres: Vec<(Ticks, u8, u8)>,
    /// The markers: where, the name, the colour.
    pub markers: Vec<(Ticks, String, Option<Color>)>,
}

#[derive(Clone)]
struct Ctx {
    track: Option<String>,
    seconds: bool,
}

#[derive(Default)]
struct Counts {
    audio: usize,
    automation: usize,
    markers: usize,
    launcher: usize,
    expressions: usize,
    loops: Vec<String>,
    references: usize,
    in_seconds: bool,
    /// Notes per track on channel 10 and on the others.
    channels: HashMap<String, (usize, usize)>,
}

/// A time in the file's unit as Cantus ticks.
fn ticks(v: f64, seconds: bool, bpm: f64) -> Ticks {
    let beats = if seconds { v * bpm / 60.0 } else { v };
    Ticks((beats * PPQ as f64).round() as i64)
}

/// Read the arrangement.
pub fn read(arr: Node, project: &Project, report: &mut Report) -> Timeline {
    let mut out = Timeline::default();
    let mut counts = Counts::default();
    let bpm = project.tempo_bpm;
    let ctx = Ctx {
        track: attr(arr, "track").map(str::to_string),
        seconds: attr(arr, "timeUnit") == Some("seconds"),
    };
    walk(arr, &ctx, bpm, project, &mut out, &mut counts);
    // The tempo map (round 29, ADR 0060): the arrangement's tempo and time-signature
    // automation and its markers come in, in the arrangement's unit or their own.
    let unit = |n: Node| attr(n, "timeUnit").map_or(ctx.seconds, |u| u == "seconds");
    if let Some(t) = child(arr, "TempoAutomation") {
        let secs = unit(t);
        for p in t.children().filter(|c| c.has_tag_name("RealPoint")) {
            if let (Some(time), Some(value)) = (attr_f64(p, "time"), attr_f64(p, "value")) {
                let ramp = attr(p, "interpolation") == Some("linear");
                out.tempo.push((ticks(time, secs, bpm), value, ramp));
            }
        }
    }
    if let Some(t) = child(arr, "TimeSignatureAutomation") {
        let secs = unit(t);
        for p in t
            .children()
            .filter(|c| c.has_tag_name("TimeSignaturePoint"))
        {
            let (Some(time), Some(n), Some(d)) = (
                attr_f64(p, "time"),
                attr_f64(p, "numerator"),
                attr_f64(p, "denominator"),
            ) else {
                continue;
            };
            out.metres.push((ticks(time, secs, bpm), n as u8, d as u8));
        }
    }
    if let Some(m) = child(arr, "Markers") {
        let secs = unit(m);
        for k in m.children().filter(|c| c.has_tag_name("Marker")) {
            if let Some(time) = attr_f64(k, "time") {
                let name = attr(k, "name").unwrap_or("").to_string();
                out.markers
                    .push((ticks(time, secs, bpm), name, parse_color(attr(k, "color"))));
            }
        }
    }
    report.count_dropped("interop.report.d_audio_clips", counts.audio);
    report.count_dropped("interop.report.d_automation", counts.automation);
    report.count_dropped("interop.report.d_markers", counts.markers);
    report.count_dropped("interop.report.d_launcher", counts.launcher);
    report.count_dropped("interop.report.d_expressions", counts.expressions);
    report.count_dropped("interop.report.d_references", counts.references);
    for clip in counts.loops {
        report.dropped("interop.report.d_loop", vec![("name", clip)]);
    }
    if counts.in_seconds {
        report.info("interop.report.d_seconds", vec![("bpm", format!("{bpm}"))]);
    }
    for (track, (drum, other)) in counts.channels {
        if drum > 0 && other == 0 {
            out.drums.insert(track);
        }
    }
    out
}

fn walk(n: Node, ctx: &Ctx, bpm: f64, project: &Project, out: &mut Timeline, counts: &mut Counts) {
    for c in n.children().filter(|c| c.is_element()) {
        let mut cx = ctx.clone();
        if let Some(t) = attr(c, "track") {
            cx.track = Some(t.to_string());
        }
        if let Some(u) = attr(c, "timeUnit") {
            cx.seconds = u == "seconds";
        }
        counts.in_seconds |= cx.seconds;
        match c.tag_name().name() {
            "Lanes" => walk(c, &cx, bpm, project, out, counts),
            "Clips" => {
                for clip in c.children().filter(|x| x.has_tag_name("Clip")) {
                    if let Some(data) = clip_of(clip, &cx, bpm, counts) {
                        if let Some(t) = &cx.track {
                            out.clips.entry(t.clone()).or_default().push(data);
                        }
                    }
                }
            }
            "Notes" => {
                // Notes straight on a track's timeline, in no clip: one clip around them,
                // from the bar before the first to the bar after the last.
                let Some(t) = &cx.track else { continue };
                let notes = notes_of(c, &cx, bpm, t, counts);
                if notes.is_empty() {
                    continue;
                }
                let bar = project.time_signature.ticks_per_bar().max(Ticks(1));
                let first = notes.iter().map(|n| n.start).min().unwrap_or(Ticks::ZERO);
                let last = notes
                    .iter()
                    .map(|n| n.start + n.length)
                    .max()
                    .unwrap_or(bar);
                let from = first.snap_floor(bar);
                let to = Ticks((last.0 + bar.0 - 1).div_euclid(bar.0) * bar.0).max(from + bar);
                out.clips.entry(t.clone()).or_default().push(ClipData {
                    name: String::new(),
                    color: None,
                    start: from,
                    length: to - from,
                    offset: from,
                    period: None,
                    muted: false,
                    notes,
                    audio: None,
                    fades: (Ticks::ZERO, Ticks::ZERO),
                });
            }
            "Points" => counts.automation += 1,
            // The arrangement's own markers come in (round 29); markers inside a lane or
            // a clip have nowhere to go.
            "Markers" | "markers" if !c.parent().is_some_and(|p| p.has_tag_name("Arrangement")) => {
                counts.markers += c.children().filter(|m| m.has_tag_name("Marker")).count()
            }
            "ClipSlot" => counts.launcher += 1,
            "Audio" | "Warps" | "Video" => counts.audio += 1,
            _ => {}
        }
    }
}

/// The notes of a `Notes` element, in the given unit, counting drums and expressions.
fn notes_of(n: Node, cx: &Ctx, bpm: f64, track: &str, counts: &mut Counts) -> Vec<NoteData> {
    let mut notes = Vec::new();
    let chan = counts.channels.entry(track.to_string()).or_default();
    for note in n.children().filter(|x| x.has_tag_name("Note")) {
        let (Some(time), Some(key)) = (attr_f64(note, "time"), attr_f64(note, "key")) else {
            continue;
        };
        if note.children().any(|x| x.is_element()) {
            counts.expressions += 1;
        }
        if attr_f64(note, "channel") == Some(9.0) {
            chan.0 += 1;
        } else {
            chan.1 += 1;
        }
        let start = ticks(time, cx.seconds, bpm);
        let mut length = ticks(attr_f64(note, "duration").unwrap_or(0.0), cx.seconds, bpm);
        if length <= Ticks::ZERO {
            length = crate::midi::import::MIN_NOTE;
        }
        let velocity = attr_f64(note, "vel")
            .map(|v| (v * 127.0).round().clamp(1.0, 127.0) as u8)
            .unwrap_or(100);
        notes.push(NoteData {
            start,
            length,
            pitch: key.clamp(0.0, 127.0) as u8,
            velocity,
        });
    }
    notes
}

/// One clip of notes. `None` for a clip of audio (counted) or of nothing we can read.
fn clip_of(clip: Node, cx: &Ctx, bpm: f64, counts: &mut Counts) -> Option<ClipData> {
    let time = attr_f64(clip, "time")?;
    if attr(clip, "reference").is_some() {
        counts.references += 1;
        return None;
    }
    let content_seconds = match attr(clip, "contentTimeUnit") {
        Some(u) => u == "seconds",
        None => cx.seconds,
    };
    let inner = Ctx {
        track: cx.track.clone(),
        seconds: content_seconds,
    };
    let track = cx.track.clone().unwrap_or_default();
    let content = clip.children().find(|c| c.is_element());
    let mut audio = None;
    let mut notes = Vec::new();
    match content.map(|c| (c, c.tag_name().name())) {
        Some((c, "Notes")) => notes = notes_of(c, &inner, bpm, &track, counts),
        Some((c, "Lanes")) => {
            for part in c.descendants().filter(|d| d.has_tag_name("Notes")) {
                notes.extend(notes_of(part, &inner, bpm, &track, counts));
            }
            counts.automation += c.descendants().filter(|d| d.has_tag_name("Points")).count();
        }
        Some((c, "Clips")) => {
            // Clips inside a clip: note clips are flattened into this one, at their own
            // place in its content; audio is counted.
            for sub in c.children().filter(|x| x.has_tag_name("Clip")) {
                let Some(d) = clip_of(sub, &inner, bpm, counts) else {
                    continue;
                };
                // Audio inside a clip of clips has nowhere to go: counted.
                if d.audio.is_some() {
                    counts.audio += 1;
                }
                for n in d.notes {
                    let at = d.start + (n.start - d.offset);
                    if at >= d.start && at < d.start + d.length {
                        notes.push(NoteData { start: at, ..n });
                    }
                }
            }
            if notes.is_empty() {
                return None;
            }
        }
        // An audio clip (round 29, ADR 0061): its file and its warp points.
        Some((c, "Audio" | "Warps")) => match super::audio::read(c, bpm, content_seconds) {
            Some(d) => audio = Some(d),
            None => {
                counts.audio += 1;
                return None;
            }
        },
        Some((_, "Video")) => {
            counts.audio += 1;
            return None;
        }
        Some((_, "Points")) => {
            counts.automation += 1;
            return None;
        }
        _ => {}
    }
    let start = ticks(time, cx.seconds, bpm);
    let offset = ticks(
        attr_f64(clip, "playStart").unwrap_or(0.0),
        content_seconds,
        bpm,
    );
    let end_of_notes = notes
        .iter()
        .map(|n| n.start + n.length)
        .max()
        .unwrap_or(Ticks::ZERO);
    let length = attr_f64(clip, "duration")
        .map(|d| ticks(d, cx.seconds, bpm))
        .unwrap_or(end_of_notes - offset)
        .max(Ticks(1));
    let name = attr(clip, "name").unwrap_or("").to_string();
    let period = match (attr_f64(clip, "loopStart"), attr_f64(clip, "loopEnd")) {
        (Some(ls), Some(le)) if le > ls => {
            let ls = ticks(ls, content_seconds, bpm);
            if ls != offset {
                counts.loops.push(name.clone());
            }
            Some(ticks(le, content_seconds, bpm) - ls)
        }
        _ => None,
    };
    Some(ClipData {
        color: parse_color(attr(clip, "color")),
        start,
        length,
        offset,
        period,
        muted: attr(clip, "enable") == Some("false"),
        notes,
        name,
        audio,
        fades: {
            let secs = match attr(clip, "fadeTimeUnit") {
                Some(u) => u == "seconds",
                None => cx.seconds,
            };
            let f = |k: &str| {
                attr_f64(clip, k).map_or(Ticks::ZERO, |v| ticks(v, secs, bpm).max(Ticks::ZERO))
            };
            (f("fadeInTime"), f("fadeOutTime"))
        },
    })
}
