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
use cantus_core::tempo::{TempoMap, TempoPoint};
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
    /// An audio clip at its natural speed: the fine offset (frames at its file's rate)
    /// its `playStart` seconds leave after whole ticks.
    pub slip: i32,
    /// Whether the tempo never changes (an on-line warp comes back as no warp then).
    pub one_tempo: bool,
}

/// The note clips of every track, by the track's id.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub clips: HashMap<String, Vec<ClipData>>,
    /// Tracks whose notes are all on MIDI channel 10: drum parts.
    pub drums: HashSet<String>,
    /// The tempo automation (round 29, ADR 0060) as the tempo track: the first point at
    /// tick 0 (the transport's tempo standing in where the automation starts later), one
    /// point a tick, the last holding — empty with none.
    pub tempo: Vec<TempoPoint>,
    /// The time-signature automation: where, numerator, denominator.
    pub metres: Vec<(Ticks, u8, u8)>,
    /// The markers: where, the name, the colour.
    pub markers: Vec<(Ticks, String, Option<Color>)>,
}

/// Where a time is read: the track, the unit, and — for seconds — how they reach the
/// timeline through the tempo map (review 2, findings 6, 19 and 57: one tempo misplaced
/// everything after a tempo change).
#[derive(Clone)]
struct Ctx {
    track: Option<String>,
    seconds: bool,
    /// What this context's ticks add to be timeline ticks: 0 on the timeline, a clip's
    /// start less its offset in its content.
    shift: f64,
    /// Where this context's seconds count from: the timeline tick, and its seconds there
    /// (the song's start; a clip's start and its `playStart`).
    origin: (f64, f64),
}

impl Ctx {
    /// A time in this context's unit, as its ticks.
    fn at(&self, map: &TempoMap, v: f64) -> Ticks {
        if !self.seconds {
            return Ticks((v * PPQ as f64).round() as i64);
        }
        let t = map.tick_after_seconds(self.origin.0, v - self.origin.1);
        Ticks((t - self.shift).round() as i64)
    }

    /// A duration `d` from time `v`: beats as they are, seconds through the map.
    fn len(&self, map: &TempoMap, v: f64, d: f64) -> Ticks {
        if !self.seconds {
            return Ticks((d * PPQ as f64).round() as i64);
        }
        self.at(map, v + d) - self.at(map, v)
    }
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

/// Read the arrangement.
pub fn read(arr: Node, project: &Project, report: &mut Report) -> Timeline {
    let mut out = Timeline::default();
    let mut counts = Counts::default();
    let bpm = project.tempo_bpm;
    let ctx = Ctx {
        track: attr(arr, "track").map(str::to_string),
        seconds: attr(arr, "timeUnit") == Some("seconds"),
        shift: 0.0,
        origin: (0.0, 0.0),
    };
    // The tempo map first (round 29, ADR 0060; review 2): every time given in seconds is
    // placed through it.
    let unit = |n: Node| attr(n, "timeUnit").map_or(ctx.seconds, |u| u == "seconds");
    if let Some(t) = child(arr, "TempoAutomation") {
        out.tempo = tempo_points(t, unit(t), bpm, report);
    }
    let map = TempoMap::new(
        out.tempo.first().map_or(bpm, |p| p.bpm),
        project.time_signature,
        &out.tempo,
        &[],
    );
    let ctx_at = |secs: bool, v: f64| {
        Ctx {
            seconds: secs,
            ..ctx.clone()
        }
        .at(&map, v)
    };
    walk(arr, &ctx, &map, project, &mut out, &mut counts);
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
            out.metres.push((ctx_at(secs, time), n as u8, d as u8));
        }
    }
    if let Some(m) = child(arr, "Markers") {
        let secs = unit(m);
        for k in m.children().filter(|c| c.has_tag_name("Marker")) {
            if let Some(time) = attr_f64(k, "time") {
                let name = attr(k, "name").unwrap_or("").to_string();
                out.markers
                    .push((ctx_at(secs, time), name, parse_color(attr(k, "color"))));
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
        if map.is_constant() {
            report.info("interop.report.d_seconds", vec![("bpm", format!("{bpm}"))]);
        } else {
            report.info("tempo.report.seconds_map", Vec::new());
        }
    }
    for (track, (drum, other)) in counts.channels {
        if drum > 0 && other == 0 {
            out.drums.insert(track);
        }
    }
    out
}

fn walk(
    n: Node,
    ctx: &Ctx,
    map: &TempoMap,
    project: &Project,
    out: &mut Timeline,
    counts: &mut Counts,
) {
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
            "Lanes" => walk(c, &cx, map, project, out, counts),
            "Clips" => {
                for clip in c.children().filter(|x| x.has_tag_name("Clip")) {
                    if let Some(data) = clip_of(clip, &cx, map, counts) {
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
                let notes = notes_of(c, &cx, map, t, counts);
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
                    slip: 0,
                    one_tempo: true,
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
fn notes_of(n: Node, cx: &Ctx, map: &TempoMap, track: &str, counts: &mut Counts) -> Vec<NoteData> {
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
        let start = cx.at(map, time);
        let mut length = cx.len(map, time, attr_f64(note, "duration").unwrap_or(0.0));
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
fn clip_of(clip: Node, cx: &Ctx, map: &TempoMap, counts: &mut Counts) -> Option<ClipData> {
    let time = attr_f64(clip, "time")?;
    if attr(clip, "reference").is_some() {
        counts.references += 1;
        return None;
    }
    let content_seconds = match attr(clip, "contentTimeUnit") {
        Some(u) => u == "seconds",
        None => cx.seconds,
    };
    // Where the clip starts (in its context's ticks, and on the timeline), and its offset:
    // content seconds in whole ticks at the tempo where it starts, as an audio clip's
    // offset is read (ADR 0060 §5).
    let start = cx.at(map, time);
    let start_tl = start.0 as f64 + cx.shift;
    let play_start = attr_f64(clip, "playStart").unwrap_or(0.0);
    let mut offset = if content_seconds {
        Ticks::from_seconds(play_start, map.bpm_at(start_tl))
    } else {
        Ticks((play_start * PPQ as f64).round() as i64)
    };
    let inner = Ctx {
        track: cx.track.clone(),
        seconds: content_seconds,
        shift: start_tl - offset.0 as f64,
        origin: (start_tl, play_start),
    };
    let track = cx.track.clone().unwrap_or_default();
    let content = clip.children().find(|c| c.is_element());
    let mut audio = None;
    let mut notes = Vec::new();
    match content.map(|c| (c, c.tag_name().name())) {
        Some((c, "Notes")) => notes = notes_of(c, &inner, map, &track, counts),
        Some((c, "Lanes")) => {
            for part in c.descendants().filter(|d| d.has_tag_name("Notes")) {
                notes.extend(notes_of(part, &inner, map, &track, counts));
            }
            counts.automation += c.descendants().filter(|d| d.has_tag_name("Points")).count();
        }
        Some((c, "Clips")) => {
            // Clips inside a clip: note clips are flattened into this one, at their own
            // place in its content; audio is counted.
            for sub in c.children().filter(|x| x.has_tag_name("Clip")) {
                let Some(d) = clip_of(sub, &inner, map, counts) else {
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
        Some((c, "Audio" | "Warps")) => {
            match super::audio::read(c, map.bpm_at(start_tl), content_seconds) {
                Some(d) => audio = Some(d),
                None => {
                    counts.audio += 1;
                    return None;
                }
            }
        }
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
    // A bare file at its natural speed (round 29's review): its `playStart` is the second
    // of the file at its left edge, placed through the tempo map (ADR 0060's rule), whole
    // ticks in the offset and the rest in the fine offset.
    let mut slip = 0;
    if let Some(d) = audio
        .as_ref()
        .filter(|d| d.warps.is_empty() && content_seconds)
    {
        let at = Ticks(start_tl.round() as i64);
        (offset, slip) =
            cantus_core::audio_time::anchor(map, at, play_start.max(0.0), 0, d.rate as f64);
    }
    let end_of_notes = notes
        .iter()
        .map(|n| n.start + n.length)
        .max()
        .unwrap_or(Ticks::ZERO);
    let length = attr_f64(clip, "duration")
        .map(|d| cx.len(map, time, d))
        .unwrap_or(end_of_notes - offset)
        .max(Ticks(1));
    let name = attr(clip, "name").unwrap_or("").to_string();
    let period = match (attr_f64(clip, "loopStart"), attr_f64(clip, "loopEnd")) {
        (Some(ls), Some(le)) if le > ls => {
            let ls = inner.at(map, ls);
            if ls != offset {
                counts.loops.push(name.clone());
            }
            Some(inner.at(map, le) - ls)
        }
        _ => None,
    };
    let end_tl = start_tl + length.0 as f64;
    let fade_secs = match attr(clip, "fadeTimeUnit") {
        Some(u) => u == "seconds",
        None => cx.seconds,
    };
    // A fade in measured from the clip's start, a fade out back from its end.
    let fade = |k: &str, out: bool| {
        attr_f64(clip, k).map_or(Ticks::ZERO, |v| {
            let t = if !fade_secs {
                v * PPQ as f64
            } else if out {
                end_tl - map.tick_after_seconds(end_tl, -v)
            } else {
                map.tick_after_seconds(start_tl, v) - start_tl
            };
            Ticks(t.round() as i64).max(Ticks::ZERO)
        })
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
        fades: (fade("fadeInTime", false), fade("fadeOutTime", true)),
        slip,
        one_tempo: map.is_constant(),
    })
}

/// The tempo automation as the tempo track (review 2): its unit read (BPM as is,
/// `normalized` through the transport tempo's range, anything else dropped with a
/// report, finding 66); points in seconds placed one after the other through the map the
/// earlier points make (a ramp linear in beats: its log-mean tempo); two points at one time
/// a jump — the later wins, as `TempoMap::new` has it, a ramp into the earlier ending a
/// tick before (finding 58); the first at tick 0 (the transport's tempo before a later
/// start), a ramp to the same tempo and the last holding.
fn tempo_points(t: Node, secs: bool, transport: f64, report: &mut Report) -> Vec<TempoPoint> {
    let range = t
        .parent()
        .and_then(|arr| arr.parent())
        .and_then(|root| child(root, "Transport"))
        .and_then(|tr| child(tr, "Tempo"))
        .and_then(|tempo| Some((attr_f64(tempo, "min")?, attr_f64(tempo, "max")?)));
    let value = |v: f64| -> Option<f64> {
        match attr(t, "unit") {
            None | Some("bpm") => Some(v),
            Some("normalized") => range.map(|(lo, hi)| lo + v.clamp(0.0, 1.0) * (hi - lo)),
            Some(_) => None,
        }
    };
    let mut raw: Vec<(f64, f64, bool)> = Vec::new();
    for p in t.children().filter(|c| c.has_tag_name("RealPoint")) {
        let (Some(time), Some(v)) = (attr_f64(p, "time"), attr_f64(p, "value")) else {
            continue;
        };
        let Some(bpm) = value(v) else {
            report.dropped(
                "tempo.report.tempo_unit",
                vec![("unit", attr(t, "unit").unwrap_or("").to_string())],
            );
            return Vec::new();
        };
        let ramp = attr(p, "interpolation") == Some("linear");
        raw.push((time, cantus_core::time::clamp_bpm(bpm), ramp));
    }
    raw.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Each point's tick: beats as they are; seconds one segment after another.
    let ppq = PPQ as f64;
    let mut ticks: Vec<f64> = Vec::with_capacity(raw.len());
    for (i, &(time, bpm, _)) in raw.iter().enumerate() {
        let tick = if !secs {
            time * ppq
        } else if i == 0 {
            time * transport / 60.0 * ppq
        } else {
            let (t0, b0, ramp) = raw[i - 1];
            let tempo = if ramp && (bpm - b0).abs() > 1e-9 {
                (bpm - b0) / (bpm / b0).ln()
            } else {
                b0
            };
            ticks[i - 1] + (time - t0) * tempo / 60.0 * ppq
        };
        ticks.push(tick);
    }
    if secs && raw.windows(2).any(|w| w[0].2 && w[0].1 != w[1].1) {
        report.info("tempo.report.seconds_ramp", Vec::new());
    }
    let mut out: Vec<TempoPoint> = Vec::with_capacity(raw.len() + 1);
    for (&(_, bpm, ramp), &tick) in raw.iter().zip(&ticks) {
        let tick = Ticks(tick.round() as i64);
        if tick.0 < 0 {
            continue;
        }
        if let Some(earlier) = out.pop_if(|l| l.tick == tick) {
            if let Some(prev) = out.last_mut().filter(|p| p.ramp) {
                if prev.bpm == earlier.bpm {
                    prev.ramp = false;
                } else if tick.0 - 1 > prev.tick.0 {
                    out.push(TempoPoint {
                        tick: tick - Ticks(1),
                        bpm: earlier.bpm,
                        ramp: false,
                    });
                }
            }
        }
        out.push(TempoPoint { tick, bpm, ramp });
    }
    if out.is_empty() {
        return out;
    }
    if out[0].tick != Ticks::ZERO {
        out.insert(
            0,
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: transport,
                ramp: false,
            },
        );
    }
    for i in 0..out.len() {
        let next = out.get(i + 1).map(|n| n.bpm);
        if next.is_none_or(|b| b == out[i].bpm) {
            out[i].ramp = false;
        }
    }
    out
}
