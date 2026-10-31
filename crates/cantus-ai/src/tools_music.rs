//! The musical tools: the linter over the project's notes, and the chunky writers — a part
//! over a chart, voiced chords, the sustain pedal from a chart — each one undo step.

use super::args::{self, plan, s_id, Res, A};
use super::notes::{place_notes, NewNote, GRIDS};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::harmony::part::{ArpDirection, BassStyle, Rhythm};
use cantus_core::harmony::{
    lay_part, lint, pitch_name, voice_chart, Chart, Issue, LintNote, LintOptions, PartSpec,
    Pattern, Severity, VoiceSpec,
};
use cantus_core::tempo::TempoMap;
use cantus_core::{ClipId, InstrumentKind, InstrumentSpec, Project, Ticks, TrackId, PPQ};
use serde_json::{json, Value};

pub const CHART_DOC: &str = "The chords. Text in bars: bars split by |, the chords of a bar by spaces sharing it equally, \".\" holds the chord before for one more share, \"%\" repeats the bar before, \"N.C.\" is no chord — \"Fm9 | Dbmaj7 | Abadd9 | Eb Eb/G\" (4 bars; the last two beats each in 4/4), \"C . . G7\" (C for three beats, G7 for one). Symbols: C, Cm, C7, Cmaj7 (CM7, CΔ), Cm7, Cm7b5 (Cø), Cdim7, Caug, Csus2, Csus4, C7sus4, C6, C6/9, Cadd9, C9, Cm9, Cmaj9, C11, C13, C7b9, C7#9, C7#11, C7alt, C5, slash basses (Eb/G). Or a list of {\"at\", \"chord\", \"length\"} with positions.";

pub fn chart_props() -> Value {
    json!({
        "chart": {"type": ["string", "array"], "description": CHART_DOC,
            "items": {"type": "object", "additionalProperties": false, "required": ["at", "chord"],
                "properties": {"at": args::s_pos("Where the chord starts"), "chord": {"type": "string"}, "length": args::s_len("How long (left out: to the next chord; required for the last)")}}},
        "chart_start_bar": {"type": "integer", "minimum": 1, "maximum": 1_000_000, "default": 1, "description": "The bar the text chart's first bar is (ignored for a list)."}
    })
}

/// A chart that ends inside the song's horizon (review #10).
fn within_horizon(chart: Chart) -> Res<Chart> {
    match chart.span() {
        Some((_, end)) if end <= cantus_core::tempo::HORIZON => Ok(chart),
        _ => Err(ToolError::bad(
            "chart",
            "it runs past the song's horizon (1 000 000 beats)",
        )),
    }
}

pub fn chart_of(map: &TempoMap, a: A<'_>) -> Res<Chart> {
    let v = a.req("chart")?;
    if let Some(text) = v.as_str() {
        let first = a
            .opt_in("chart_start_bar", 1.0, 1_000_000.0)?
            .unwrap_or(1.0) as i64;
        let chart = Chart::parse_bars(text, map, first)
            .map_err(|e| ToolError::bad("chart", e.to_string()))?;
        return within_horizon(chart);
    }
    let items = v
        .as_array()
        .ok_or_else(|| ToolError::bad("chart", "text in bars, or a list of chords"))?;
    let mut spans = Vec::with_capacity(items.len());
    let mut starts = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        starts.push(args::pos_of(
            map,
            A(it).req("at")?,
            &format!("chart[{i}].at"),
        )?);
    }
    for (i, it) in items.iter().enumerate() {
        let ia = A(it);
        let at = starts[i];
        let end = match args::opt_len(map, at, ia, "length")? {
            Some(l) => at + l,
            None => *starts.get(i + 1).ok_or_else(|| {
                ToolError::bad(
                    format!("chart[{i}].length"),
                    "the last chord needs a length",
                )
            })?,
        };
        spans.push((at, end, Some(ia.str("chord")?)));
    }
    within_horizon(Chart::from_spans(spans).map_err(|e| ToolError::bad("chart", e.to_string()))?)
}

/// The pitches an instrument plays, when it has a range of its own; for a kit, its pads.
pub fn playable(spec: &InstrumentSpec) -> Option<(u128, String)> {
    let span = |lo: u8, hi: u8| -> u128 { (lo..=hi).fold(0u128, |m, p| m | 1u128 << p) };
    match spec {
        InstrumentSpec::Pulsus(kit) => Some((
            kit.pads
                .iter()
                .fold(0u128, |m, p| m | 1u128 << p.pitch.min(127)),
            "its pads".into(),
        )),
        InstrumentSpec::Native(n) => {
            let (lo, hi) = match n.kind {
                InstrumentKind::Chorda => (21, 108),
                InstrumentKind::ElectricPiano => (28, 100),
                InstrumentKind::Organ => (24, 96),
                InstrumentKind::Bass => (12, 72),
                _ => return None,
            };
            Some((
                span(lo, hi),
                format!("{}–{}", pitch_name(lo), pitch_name(hi)),
            ))
        }
        _ => None,
    }
}

pub fn lint_schema() -> Value {
    let mut props = chart_props();
    props["track_ids"] = args::s_ids("The tracks to check (left out: every track with notes).");
    props["from"] = args::s_pos("Check notes from here");
    props["to"] = args::s_pos("…to here");
    props["key"] = json!({"type": "object", "additionalProperties": false, "required": ["root", "scale"],
        "description": "The key to check against (left out: the project's key, if it has one).",
        "properties": {"root": {"type": "string"}, "scale": {"type": "string", "description": "major, minor, dorian, mixolydian, harmonic_minor… (set_song's list)."}}});
    props["info"] = json!({"type": "boolean", "default": true, "description": "Include notes outside the key (info), not only warnings and errors."});
    props["limit"] = json!({"type": "integer", "minimum": 1, "maximum": 1000, "default": 200});
    // A chart is optional here: without one only doubling, the key and the range are checked.
    args::object(props, &[])
}

/// The notes of `tracks` as the linter reads them (looped clips repeat), and what each is.
/// The most notes (looped clips' repetitions counted) one lint reads; past it the answer
/// says `notes_capped` (review #14).
pub const MAX_LINT_NOTES: usize = 200_000;

fn collect(
    p: &Project,
    tracks: &[TrackId],
    from: Ticks,
    to: Ticks,
) -> (Vec<LintNote>, Vec<(TrackId, ClipId, u64)>, bool) {
    let mut notes = Vec::new();
    let mut who = Vec::new();
    let mut at = Vec::new();
    let mut capped = false;
    'all: for &tid in tracks {
        let Some(t) = p.track(tid) else { continue };
        for c in t
            .clips
            .iter()
            .filter(|c| !c.muted && c.lane == 0 && c.automation.is_none() && c.audio.is_none())
        {
            for n in c.notes.iter().filter(|n| !n.muted) {
                // A loop's repeats counted one by one, never all laid out first (fix review
                // #5: a short loop on a long clip would make hundreds of millions).
                at.clear();
                match c.content_length {
                    Some(period) if period.0 > 0 => {
                        if n.start < c.offset || n.start >= c.offset + period {
                            continue;
                        }
                        let mut s = c.start + (n.start - c.offset);
                        // From the first repeat in range (fix review 3 #5).
                        if s < from {
                            s += Ticks(((from - s).0 + period.0 - 1) / period.0 * period.0);
                        }
                        while s < c.end() && s < to {
                            if at.len() + notes.len() >= MAX_LINT_NOTES {
                                capped = true;
                                break;
                            }
                            at.push(s);
                            s += period;
                        }
                    }
                    _ => c.timeline_positions(n.start, &mut at),
                }
                for &s in &at {
                    if s < from || s >= to {
                        continue;
                    }
                    if notes.len() >= MAX_LINT_NOTES {
                        capped = true;
                        break 'all;
                    }
                    let end = (s + n.length).min(c.end());
                    notes.push(LintNote {
                        part: tid.0,
                        id: who.len() as u64,
                        start: s,
                        end,
                        pitch: n.pitch,
                    });
                    who.push((tid, c.id, n.id.0));
                }
            }
        }
    }
    (notes, who, capped)
}

pub fn lint_music(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let chart = if a.has("chart") {
        Some(chart_of(&map, a)?)
    } else {
        None
    };
    let tracks: Vec<TrackId> = match a.opt_array("track_ids")? {
        Some(v) => v
            .iter()
            .map(|x| {
                let id = TrackId(
                    x.as_u64()
                        .ok_or_else(|| ToolError::bad("track_ids", "track ids"))?,
                );
                p.track(id)
                    .map(|_| id)
                    .ok_or_else(|| ToolError::NotFound(format!("track {}", id.0)))
            })
            .collect::<Res<_>>()?,
        None => p
            .tracks
            .iter()
            .filter(|t| t.clips.iter().any(|c| !c.notes.is_empty()))
            .map(|t| t.id)
            .collect(),
    };
    let key = match a.get("key") {
        Some(k) => {
            let ka = A(k);
            let root = ka.str("root")?;
            let (pc, rest) = cantus_core::harmony::chord::parse_pc(root)
                .ok_or_else(|| ToolError::bad("key.root", "a note name"))?;
            if !rest.is_empty() {
                return Err(ToolError::bad("key.root", "a note name"));
            }
            let scale: cantus_core::ScaleKind = serde_json::from_value(json!(ka.str("scale")?))
                .map_err(|_| ToolError::bad("key.scale", "not a scale"))?;
            Some(cantus_core::Key::new(pc, scale))
        }
        None => p.key,
    };
    let from = args::opt_pos(&map, a, "from")?.unwrap_or(Ticks::ZERO);
    let to = args::opt_pos(&map, a, "to")?.unwrap_or(Ticks(i64::MAX));
    let (notes, who, capped) = collect(p, &tracks, from, to);
    let unpitched: Vec<u64> = tracks
        .iter()
        .filter(|t| p.track(**t).is_some_and(|t| !t.instrument.is_chromatic()))
        .map(|t| t.0)
        .collect();
    let ranges: Vec<(u64, u128, String)> = tracks
        .iter()
        .filter_map(|t| playable(&p.track(*t)?.instrument).map(|(m, s)| (t.0, m, s)))
        .collect();
    let masks: Vec<(u64, u128)> = ranges.iter().map(|(t, m, _)| (*t, *m)).collect();
    let o = LintOptions {
        chart: chart.as_ref(),
        key,
        unpitched: &unpitched,
        playable: &masks,
        ..LintOptions::default()
    };
    let info = a.opt_bool("info")?.unwrap_or(true);
    let limit = a.opt_u64("limit")?.unwrap_or(200).clamp(1, 1000) as usize;
    let found: Vec<_> = lint(&notes, &o)
        .into_iter()
        .filter(|f| info || f.severity != Severity::Info)
        .collect();
    let count = |s: Severity| found.iter().filter(|f| f.severity == s).count();
    let chord = |i: usize| {
        chart
            .as_ref()
            .and_then(|c| c.chords[i].chord.as_ref())
            .map(|c| cantus_core::harmony::spell(&c.symbol))
            .unwrap_or_default()
    };
    let rows: Vec<Value> = found
        .iter()
        .take(limit)
        .map(|f| {
            let (tid, clip, note) = who[f.note as usize];
            let tname = p.track(tid).map(|t| t.name.clone()).unwrap_or_default();
            let n = pitch_name(f.pitch);
            let at = args::bar_beat_text(&map, f.at);
            let message = match &f.issue {
                Issue::RingsIntoChange {
                    into,
                    overlap,
                    against,
                    ..
                } => format!(
                    "{n} rings {} beats into {} at {at}{} — end it at the change",
                    args::trim_num(args::beats(*overlap)),
                    chord(*into),
                    against
                        .map(|x| format!(
                            ", a semitone against its {}",
                            cantus_core::harmony::PC_NAMES[x as usize]
                        ))
                        .unwrap_or_default()
                ),
                Issue::Clash {
                    chord: c,
                    against,
                    held,
                } => format!(
                    "{n} is held {} beats under {} at {at}, a semitone from its {}",
                    args::trim_num(args::beats(*held)),
                    chord(*c),
                    cantus_core::harmony::PC_NAMES[*against as usize]
                ),
                Issue::Doubled { same_start, .. } => {
                    if *same_start {
                        format!("{n} at {at} is written twice")
                    } else {
                        format!("{n} at {at} overlaps another {n}")
                    }
                }
                Issue::OutOfScale => format!("{n} at {at} is outside the key and the chord"),
                Issue::OutOfRange => {
                    let r = ranges
                        .iter()
                        .find(|(t, ..)| *t == tid.0)
                        .map(|(_, _, s)| s.clone())
                        .unwrap_or_default();
                    format!("{n} at {at} is outside what the instrument plays ({r})")
                }
            };
            json!({
                "severity": f.severity.name(), "issue": f.issue.kind(), "message": message,
                "track_id": tid.0, "track": tname, "clip_id": clip.0, "note_id": note,
                "pitch": f.pitch, "at": args::pos_json(&map, f.at),
            })
        })
        .collect();
    Ok(json!({
        "errors": count(Severity::Error), "warnings": count(Severity::Warning), "info": count(Severity::Info),
        "notes_checked": notes.len(), "notes_capped": capped, "truncated": found.len() > limit, "findings": rows,
    }))
}

fn range_of(a: A<'_>, k: &str, default: (u8, u8)) -> Res<(u8, u8)> {
    let Some(v) = a.get(k) else {
        return Ok(default);
    };
    let ra = A(v);
    let lo = super::notes::pitch_of(ra.req("low")?, &format!("{k}.low"))?;
    let hi = super::notes::pitch_of(ra.req("high")?, &format!("{k}.high"))?;
    if hi < lo.saturating_add(11) {
        return Err(ToolError::bad(
            k,
            "needs at least an octave between low and high",
        ));
    }
    Ok((lo, hi))
}

fn range_schema(what: &str) -> Value {
    json!({"type": "object", "additionalProperties": false, "required": ["low", "high"], "description": what,
        "properties": {"low": {"type": ["integer", "string"]}, "high": {"type": ["integer", "string"]}}})
}

pub fn write_part_schema() -> Value {
    let mut props = chart_props();
    props["track_id"] = s_id("The track to write on.");
    props["pattern"] = json!({"enum": ["block", "pulse", "arpeggio", "bass"], "description": "block: each chord held; pulse: the chord on every hit of the rhythm; arpeggio: one voice per hit, cycling; bass: the chord's bass (slash note or root) on each hit."});
    props["rhythm"] = json!({"type": "string", "description": "A step pattern, one character per step, repeated without a break from the chart's start (a pattern longer than a bar runs across the bar lines): x hit, X accent, - tie (the hit before goes on), . rest — \"x..x..x.\" with step 1/8 is a 3-3-2 figure. Left out: one hit per `rate`."});
    props["step"] = json!({"enum": GRIDS, "default": "1/16", "description": "The rhythm's step."});
    props["rate"] = json!({"enum": GRIDS, "default": "1/8", "description": "With no rhythm: a hit every this."});
    props["voices"] = json!({"type": "integer", "minimum": 1, "maximum": 8, "default": 4, "description": "Voices of each chord (block, pulse, arpeggio)."});
    props["range"] =
        range_schema("Where the voices lie (default F3–F5; bass E1–E3), as MIDI notes or names.");
    props["with_bass"] = json!({"type": "boolean", "default": false, "description": "Add the chord's bass under the voices (block, pulse)."});
    props["direction"] = json!({"enum": ["up", "down", "up_down", "random"], "default": "up"});
    props["octaves"] = json!({"type": "integer", "minimum": 1, "maximum": 4, "default": 1, "description": "Arpeggio: octaves it climbs."});
    props["bass_style"] =
        json!({"enum": ["root", "root_fifth", "octave", "root_fifth_octave"], "default": "root"});
    props["gate"] = json!({"type": "number", "minimum": 0.05, "maximum": 1, "default": 0.9, "description": "The share of each hit a note sounds (1 legato)."});
    props["velocity"] = json!({"type": "integer", "minimum": 1, "maximum": 127, "default": 90});
    props["accent"] = json!({"type": "integer", "minimum": 0, "maximum": 60, "default": 12, "description": "Added on accents and downbeats."});
    props["swing"] = json!({"type": "number", "minimum": 0, "maximum": 1, "default": 0, "description": "0 straight … 1 a full triplet swing of the off-beats of `swing_grid`."});
    props["swing_grid"] = json!({"enum": ["1/8", "1/16"], "default": "1/8"});
    props["humanize"] = json!({"type": "object", "additionalProperties": false, "properties": {
        "timing_beats": {"type": "number", "minimum": 0, "maximum": 0.1, "default": 0.01},
        "velocity": {"type": "integer", "minimum": 0, "maximum": 30, "default": 6},
        "seed": {"type": "integer", "minimum": 0, "default": 1}}});
    props["tie_common"] = json!({"type": "boolean", "default": true, "description": "Block: hold a pitch the next chord also has, as a pianist would."});
    props["clip_id"] = s_id("Write into this clip instead of a new one of whole bars.");
    args::object(props, &["track_id", "chart", "pattern"])
}

/// Each chord's bass pitch in `lo..=hi`, moving to the nearest one each time.
fn bass_line(chart: &Chart, lo: u8, hi: u8) -> Vec<Vec<u8>> {
    let mut prev = lo + 5;
    chart
        .chords
        .iter()
        .map(|c| match &c.chord {
            None => Vec::new(),
            Some(ch) => {
                let p = (lo..=hi)
                    .filter(|p| p % 12 == ch.bass)
                    .min_by_key(|p| ((*p as i32 - prev as i32).abs(), *p))
                    .unwrap_or(lo);
                prev = p;
                vec![p]
            }
        })
        .collect()
}

fn lay(
    cx: &ToolCx<'_>,
    a: A<'_>,
    map: &TempoMap,
    chart: &Chart,
    pattern_name: &str,
) -> Res<Vec<NewNote>> {
    let bass = pattern_name == "bass";
    let range = range_of(a, "range", if bass { (28, 52) } else { (53, 77) })?;
    let voices = a.opt_in("voices", 1.0, 8.0)?.unwrap_or(4.0) as usize;
    let voicings = if bass {
        bass_line(chart, range.0, range.1)
    } else {
        let with_bass = a.opt_bool("with_bass")?.unwrap_or(false);
        let spec = VoiceSpec {
            voices,
            low: range.0,
            high: range.1,
            bass: with_bass.then_some((28, 52)),
        };
        voice_chart(chart, &spec).ok_or_else(|| {
            ToolError::bad(
                "range",
                "the range cannot hold that many voices: widen it or use fewer",
            )
        })?
    };
    let pattern = match pattern_name {
        "block" => Pattern::Block,
        "pulse" => Pattern::Pulse,
        "arpeggio" => Pattern::Arpeggio {
            direction: match a
                .opt_choice("direction", &["up", "down", "up_down", "random"])?
                .unwrap_or("up")
            {
                "down" => ArpDirection::Down,
                "up_down" => ArpDirection::UpDown,
                "random" => ArpDirection::Random,
                _ => ArpDirection::Up,
            },
            octaves: a.opt_in("octaves", 1.0, 4.0)?.unwrap_or(1.0) as u8,
        },
        _ => Pattern::Bass(
            match a
                .opt_choice(
                    "bass_style",
                    &["root", "root_fifth", "octave", "root_fifth_octave"],
                )?
                .unwrap_or("root")
            {
                "root_fifth" => BassStyle::RootFifth,
                "octave" => BassStyle::Octave,
                "root_fifth_octave" => BassStyle::RootFifthOctave,
                _ => BassStyle::Root,
            },
        ),
    };
    let rhythm = match a.opt_str("rhythm")? {
        Some(r) => {
            let step = super::notes::grid(a.opt_str("step")?.unwrap_or("1/16"), "step")?;
            Rhythm::parse_steps(r, step).ok_or_else(|| {
                ToolError::bad(
                    "rhythm",
                    "use x X - . (at least one hit, at most 256 steps)",
                )
            })?
        }
        None => Rhythm::Every(super::notes::grid(
            a.opt_str("rate")?.unwrap_or("1/8"),
            "rate",
        )?),
    };
    let h = a.get("humanize").map(A);
    let spec = PartSpec {
        pattern,
        rhythm,
        gate: a.opt_in("gate", 0.05, 1.0)?.unwrap_or(0.9) as f32,
        velocity: a.opt_in("velocity", 1.0, 127.0)?.unwrap_or(90.0) as u8,
        accent: a.opt_in("accent", 0.0, 60.0)?.unwrap_or(12.0) as u8,
        swing: a.opt_in("swing", 0.0, 1.0)?.unwrap_or(0.0) as f32,
        swing_grid: super::notes::grid(a.opt_str("swing_grid")?.unwrap_or("1/8"), "swing_grid")?,
        humanize_ticks: match h {
            Some(h) => Ticks::from_beats_f64(h.opt_in("timing_beats", 0.0, 0.1)?.unwrap_or(0.01)).0,
            None => 0,
        },
        humanize_velocity: match h {
            Some(h) => h.opt_in("velocity", 0.0, 30.0)?.unwrap_or(6.0) as u8,
            None => 0,
        },
        seed: match h {
            Some(h) => h.opt_u64("seed")?.unwrap_or(1),
            None => 1,
        },
        tie_common: a.opt_bool("tie_common")?.unwrap_or(true),
    };
    let _ = cx;
    let laid = lay_part(chart, &voicings, map, &spec);
    if laid.is_empty() {
        return Err(ToolError::bad(
            "chart",
            "the chart gives nothing to play (only N.C.?)",
        ));
    }
    if laid.len() > args::MAX_NOTES {
        return Err(ToolError::bad(
            "chart",
            format!(
                "more than {} notes: write the part in pieces",
                args::MAX_NOTES
            ),
        ));
    }
    Ok(laid
        .into_iter()
        .map(|n| NewNote {
            at: n.start,
            len: n.length,
            pitch: n.pitch,
            velocity: n.velocity,
        })
        .collect())
}

fn write(
    cx: &mut ToolCx<'_>,
    a: A<'_>,
    pattern: &str,
    said_key: &'static str,
    label: &str,
) -> Res<EditPlan> {
    let p = cx.project();
    let map = p.tempo_map();
    let track = args::track(p, a, "track_id")?;
    if !p.track(track).expect("checked").instrument.is_chromatic() {
        return Err(ToolError::bad(
            "track_id",
            "a drum kit plays pads, not chords: use write_drum_pattern",
        ));
    }
    let chart = chart_of(&map, a)?;
    let notes = lay(cx, a, &map, &chart, pattern)?;
    let tname = cx.project().track(track).expect("checked").name.clone();
    let clip = a.opt_u64("clip_id")?.map(ClipId);
    let (cmds, id, created, ids) = place_notes(cx, track, clip, &notes, "Part", true)?;
    let (s, e) = chart.span().expect("a chart has chords");
    let said = Said::new(said_key)
        .name("track", args::clean(&tname))
        .nested("from", words::pos(&map, s))
        .nested("to", words::pos(&map, e))
        .nested(
            "pattern",
            Said::new(match pattern {
                "pulse" => "ai.phrase.pattern.pulse",
                "arpeggio" => "ai.phrase.pattern.arpeggio",
                "bass" => "ai.phrase.pattern.bass",
                _ => "ai.phrase.pattern.block",
            }),
        );
    Ok(plan(
        args::one(cmds, label)?,
        said,
        json!({"clip_id": id.0, "clip_created": created, "notes": ids.len(), "chords": chart.chords.len(),
            "from": args::pos_json(&map, s), "to": args::pos_json(&map, e)}),
    ))
}

pub fn write_part(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let pattern = a
        .opt_choice("pattern", &["block", "pulse", "arpeggio", "bass"])?
        .ok_or_else(|| ToolError::bad("pattern", "is required"))?;
    write(cx, a, pattern, "ai.tools.write_part", "cmd.ai.write_part")
}

pub fn voice_chords_schema() -> Value {
    let mut props = chart_props();
    props["track_id"] = s_id("The track to write on.");
    props["voices"] = json!({"type": "integer", "minimum": 1, "maximum": 8, "default": 4, "description": "Voices above the bass; with fewer voices than tones the third and seventh stay, then tensions, then the root, the fifth last."});
    props["range"] =
        range_schema("Where the voices lie (default F3–F5), as MIDI notes or names (\"C3\").");
    props["with_bass"] = json!({"type": "boolean", "default": false, "description": "Add each chord's bass (its slash note or root) in E1–E3."});
    props["velocity"] = json!({"type": "integer", "minimum": 1, "maximum": 127, "default": 80});
    props["tie_common"] = json!({"type": "boolean", "default": true, "description": "Hold a pitch the next chord also has."});
    props["clip_id"] = s_id("Write into this clip instead of a new one.");
    args::object(props, &["track_id", "chart"])
}

/// The chart as held chords, each voiced to move as little as possible from the last.
pub fn voice_chords(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let mut v = a.clone();
    if let Some(m) = v.as_object_mut() {
        m.insert("gate".into(), json!(1.0));
        m.entry("velocity").or_insert(json!(80));
    }
    write(
        cx,
        A(&v),
        "block",
        "ai.tools.voice_chords",
        "cmd.ai.voice_chords",
    )
}

pub fn pedal_schema() -> Value {
    let mut props = chart_props();
    props["track_id"] =
        s_id("A track whose instrument has a sustain pedal parameter (`pedal`: Chorda).");
    props["lift_beats"] = json!({"type": "number", "minimum": 0.0625, "maximum": 1, "default": 0.25, "description": "How long after each change the pedal goes down again (legato pedalling: up exactly on the change, down a sixteenth later)."});
    args::object(props, &["track_id", "chart"])
}

/// The sustain pedal played from a chart: up on every change, down again a moment later,
/// so each chord rings and never the last one's.
pub fn pedal_from_chart(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let aa = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let track = args::track(p, aa, "track_id")?;
    let has_pedal = p
        .track(track)
        .and_then(|t| t.instrument.native())
        .is_some_and(|n| n.kind.param("pedal").is_some());
    if !has_pedal {
        return Err(ToolError::bad(
            "track_id",
            "its instrument has no `pedal` parameter (Chorda has)",
        ));
    }
    let chart = chart_of(&map, aa)?;
    let tname = words::track_name(p, track);
    let lift = Ticks::from_beats_f64(aa.opt_in("lift_beats", 0.0625, 1.0)?.unwrap_or(0.25));
    let (s, e) = chart.span().expect("chords");
    let mut pts = vec![(s, 0.0), (s + lift, 1.0)];
    for (_, at) in chart.changes() {
        pts.push((at, 0.0));
        pts.push((at + lift, 1.0));
    }
    pts.push((e, 0.0));
    let points: Vec<Value> = pts
        .iter()
        .map(|(t, v)| json!({"at": t.as_beats_f64(), "value": v, "shape": "hold"}))
        .collect();
    let mut plan = super::modulation::automate(
        cx,
        &json!({"target": {"track_id": track.0, "param": "pedal"}, "points": points, "name": "Pedal"}),
    )?;
    plan.said = Said::new("ai.tools.pedal_from_chart")
        .name("track", args::clean(&tname))
        .nested("changes", words::count(chart.changes().count(), "changes"))
        .nested("bars", super::modulation::bars_said(&map, s, e));
    if let Some(m) = plan.result.as_object_mut() {
        m.insert("changes".into(), json!(chart.changes().count()));
    }
    let _ = PPQ;
    Ok(plan)
}
