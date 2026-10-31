//! Clips and notes: make a clip, add notes, replace them, edit them (transpose, quantise,
//! humanise, move, velocity, length), set a clip's bounds and name, duplicate it, write a
//! drum pattern. Positions are the song's (beats or bars:beats), never the clip's own.

use super::args::{self, plan, s_id, s_len, s_pos, Res, A, MAX_NOTES};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::tempo::TempoMap;
use cantus_core::{Clip, ClipId, Command, Note, NoteId, Project, Ticks, TrackId, PPQ};
use serde_json::{json, Value};

/// The JSON schema of one note to write.
pub fn note_item_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "start": s_pos("Where the note starts in the song"),
            "start_beats": {"type": "number", "minimum": 0, "description": "The same as `start`, in beats (older callers)."},
            "length": s_len("How long it sounds"),
            "length_beats": {"type": "number", "exclusiveMinimum": 0, "description": "The same as `length`, in beats (older callers)."},
            "pitch": {"type": ["integer", "string"], "description": "MIDI note 0-127 (60 = C4, middle C) or a name: \"C4\", \"Eb3\", \"F#5\"."},
            "velocity": {"type": "integer", "minimum": 1, "maximum": 127, "default": 100}
        },
        "required": ["pitch"],
        "additionalProperties": false
    })
}

/// A note to write: song position, length, pitch, velocity.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NewNote {
    pub at: Ticks,
    pub len: Ticks,
    pub pitch: u8,
    pub velocity: u8,
}

pub fn pitch_of(v: &Value, k: &str) -> Res<u8> {
    if let Some(n) = v.as_u64() {
        return u8::try_from(n)
            .ok()
            .filter(|p| *p <= 127)
            .ok_or_else(|| ToolError::bad(k, "a MIDI note is 0 to 127"));
    }
    if let Some(s) = v.as_str() {
        return cantus_core::harmony::parse_pitch(s).ok_or_else(|| {
            ToolError::bad(k, format!("{s:?} is not a note name such as C4 or Eb3"))
        });
    }
    Err(ToolError::bad(
        k,
        "a MIDI note number or a name such as \"C4\"",
    ))
}

pub fn parse_notes(map: &TempoMap, items: &[Value]) -> Res<Vec<NewNote>> {
    if items.is_empty() || items.len() > MAX_NOTES {
        return Err(ToolError::bad(
            "notes",
            format!("give 1 to {MAX_NOTES} notes"),
        ));
    }
    let mut out = Vec::with_capacity(items.len());
    for (i, n) in items.iter().enumerate() {
        let a = A(n);
        let k = |f: &str| format!("notes[{i}].{f}");
        let at = match (a.get("start"), a.get("start_beats")) {
            (Some(v), _) | (None, Some(v)) => args::pos_of(map, v, &k("start"))?,
            _ => return Err(ToolError::bad(k("start"), "is required")),
        };
        let len = match (a.get("length"), a.get("length_beats")) {
            (Some(v), _) | (None, Some(v)) => args::len_of(map, at, v, &k("length"))?,
            _ => return Err(ToolError::bad(k("length"), "is required")),
        };
        let pitch = pitch_of(a.req("pitch")?, &k("pitch"))?;
        let velocity = match a.opt_u64("velocity")? {
            None => 100,
            Some(v) if (1..=127).contains(&v) => v as u8,
            Some(_) => return Err(ToolError::bad(k("velocity"), "is 1 to 127")),
        };
        out.push(NewNote {
            at,
            len,
            pitch,
            velocity,
        });
    }
    Ok(out)
}

/// Where a clip's content tick 0 sits in the song.
pub fn origin(c: &Clip) -> Ticks {
    c.start - c.offset
}

/// Notes as a clip holds them, with fresh ids.
pub fn to_clip_notes(cx: &mut ToolCx<'_>, clip_origin: Ticks, notes: &[NewNote]) -> Vec<Note> {
    let bus = cx.bus_mut();
    notes
        .iter()
        .map(|n| bus.new_note(n.at - clip_origin, n.len, n.pitch, n.velocity))
        .collect()
}

/// The commands that put `notes` on `track`: into `clip` (growing it to hold them), into
/// the note clip whose window holds the first, or into a new clip of whole bars. The clip's
/// id and whether it is new.
pub fn place_notes(
    cx: &mut ToolCx<'_>,
    track: TrackId,
    clip: Option<ClipId>,
    notes: &[NewNote],
    clip_name: &str,
    fresh: bool,
) -> Res<(Vec<Command>, ClipId, bool, Vec<NoteId>)> {
    let p = cx.project();
    let map = p.tempo_map();
    let first = notes.iter().map(|n| n.at).min().expect("notes");
    let last = notes.iter().map(|n| n.at + n.len).max().expect("notes");
    let t = p.track(track).expect("checked");
    let target = match clip {
        Some(id) => Some(
            t.clip(id)
                .ok_or_else(|| ToolError::NotFound(format!("clip {} on track {}", id.0, track.0)))?
                .clone(),
        ),
        // A writer's part is a clip of its own (review #6): never notes mixed into the
        // user's clip without being asked; the track must have none where it goes.
        None if fresh => {
            if let Some(c) = t.clips.iter().find(|c| {
                c.lane == 0
                    && c.automation.is_none()
                    && c.audio.is_none()
                    && c.start < last
                    && first < c.end()
            }) {
                return Err(ToolError::Refused(format!(
                    "the track already has a clip there, {} ({}…{}): give clip_id to write into it, or write elsewhere",
                    c.id.0,
                    super::args::bar_beat_text(&map, c.start),
                    super::args::bar_beat_text(&map, c.end())
                )));
            }
            None
        }
        None => t
            .clips
            .iter()
            .filter(|c| c.automation.is_none() && c.audio.is_none() && c.lane == 0)
            .find(|c| c.start <= first && first < c.end())
            .cloned(),
    };
    if let Some(c) = &target {
        if c.automation.is_some() || c.audio.is_some() {
            return Err(ToolError::bad("clip_id", "is not a note clip"));
        }
    }
    let mut cmds = Vec::new();
    let (id, o, created) = match target {
        Some(c) => {
            if first < c.start {
                return Err(ToolError::bad(
                    "notes",
                    format!("a note starts at {}, before the clip's start at {}: move the clip with set_clip or leave clip_id out", args::bar_beat_text(&map, first), args::bar_beat_text(&map, c.start)),
                ));
            }
            if let Some(period) = c.content_length {
                if notes.iter().any(|n| n.at - origin(&c) >= c.offset + period) {
                    return Err(ToolError::bad(
                        "notes",
                        "the clip loops: notes must fall in its first pass (set_clip's loop_beats)",
                    ));
                }
            }
            if last > c.end() {
                let (_, end) = map.bars_around(c.start, last);
                cmds.push(Command::SetClipBounds {
                    track,
                    clip: c.id,
                    start: c.start,
                    length: end - c.start,
                });
            }
            (c.id, origin(&c), false)
        }
        None => {
            let (from, to) = map.bars_around(first, last);
            let c = cx.bus_mut().new_clip(clip_name, from, to - from);
            let id = c.id;
            cmds.push(Command::InsertClip {
                track,
                clip: Box::new(c),
            });
            (id, from, true)
        }
    };
    let made = to_clip_notes(cx, o, notes);
    let ids = made.iter().map(|n| n.id).collect();
    cmds.push(Command::InsertNotes {
        track,
        clip: id,
        notes: made,
    });
    Ok((cmds, id, created, ids))
}

pub fn add_clip_schema() -> Value {
    args::object(
        json!({
            "track_id": s_id("The track."),
            "start": s_pos("Where the clip starts"),
            "length": s_len("How long it is"),
            "name": {"type": "string", "minLength": 1, "maxLength": 80}
        }),
        &["track_id", "start", "length"],
    )
}

pub fn add_clip(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let track = args::track(p, a, "track_id")?;
    let start = args::pos(&map, a, "start")?;
    let length = args::len(&map, start, a, "length")?;
    let name = a.opt_name("name", 80)?.unwrap_or_else(|| "Clip".into());
    let tname = p.track(track).expect("checked").name.clone();
    let c = cx.bus_mut().new_clip(name.clone(), start, length);
    let id = c.id;
    let said = Said::new("ai.tools.add_clip")
        .name("track", args::clean(&tname))
        .nested("at", words::pos(&map, start));
    let result = json!({"clip_id": id.0, "start": args::pos_json(&map, start), "end": args::pos_json(&map, start + length)});
    Ok(plan(
        Command::InsertClip {
            track,
            clip: Box::new(c),
        },
        said,
        result,
    ))
}

pub fn add_notes_schema() -> Value {
    args::object(
        json!({
            "track_id": s_id("The track."),
            "clip_id": s_id("Write into this clip (it grows to hold the notes). Left out: the note clip already under the first note, or a new clip of whole bars."),
            "notes": {"type": "array", "minItems": 1, "maxItems": MAX_NOTES, "items": note_item_schema(),
                "description": "The notes; start and length are required (as `start`/`length` or `start_beats`/`length_beats`)."}
        }),
        &["track_id", "notes"],
    )
}

pub fn add_notes(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let track = args::track(p, a, "track_id")?;
    let clip = a.opt_u64("clip_id")?.map(ClipId);
    let notes = parse_notes(&map, a.array("notes")?)?;
    let tname = p.track(track).expect("checked").name.clone();
    let (cmds, id, created, ids) = place_notes(cx, track, clip, &notes, "Clip", false)?;
    let first = notes.iter().map(|n| n.at).min().expect("notes");
    let said = Said::new("ai.tools.add_notes")
        .nested("n", words::count(notes.len(), "notes"))
        .name("track", args::clean(&tname))
        .nested("at", words::pos(&map, first));
    let result = json!({
        "clip_id": id.0, "clip_created": created,
        "note_ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(),
    });
    Ok(plan(args::one(cmds, "cmd.ai.add_notes")?, said, result))
}

pub fn replace_notes_schema() -> Value {
    args::object(
        json!({
            "clip_id": s_id("The clip whose notes to replace."),
            "from": s_pos("Replace only notes starting here or later (left out: the clip's start)"),
            "to": s_pos("…and before here (left out: the clip's end)"),
            "notes": {"type": "array", "maxItems": MAX_NOTES, "items": note_item_schema(),
                "description": "The new notes (song positions, inside the clip). An empty list only removes."}
        }),
        &["clip_id", "notes"],
    )
}

/// Replace the notes of a clip (or a range of it): destructive — the old notes go.
pub fn replace_notes(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let (track, c) = args::clip(p, a, "clip_id")?;
    if c.automation.is_some() || c.audio.is_some() {
        return Err(ToolError::bad("clip_id", "is not a note clip"));
    }
    let c = c.clone();
    let from = args::opt_pos(&map, a, "from")?.unwrap_or(c.start);
    let to = args::opt_pos(&map, a, "to")?.unwrap_or(c.end());
    let target = words::clip_span(p, &map, track, &c.name, from.max(c.start), to.min(c.end()));
    let o = origin(&c);
    let old: Vec<NoteId> = c
        .notes
        .iter()
        .filter(|n| (from..to).contains(&(o + n.start)))
        .map(|n| n.id)
        .collect();
    let items = a.array("notes")?;
    let notes = if items.is_empty() {
        Vec::new()
    } else {
        parse_notes(&map, items)?
    };
    if notes.iter().any(|n| n.at < c.start || n.at >= c.end()) {
        return Err(ToolError::bad(
            "notes",
            "every note must start inside the clip",
        ));
    }
    // A looping clip plays the first pass of its content only (review #13).
    if let Some(period) = c.content_length {
        if notes.iter().any(|n| n.at - o >= c.offset + period) {
            return Err(ToolError::bad(
                "notes",
                "the clip loops: notes must fall in its first pass (set_clip's loop_beats)",
            ));
        }
    }
    let mut cmds = Vec::new();
    if !old.is_empty() {
        cmds.push(Command::RemoveNotes {
            track,
            clip: c.id,
            ids: old.clone(),
        });
    }
    let made = to_clip_notes(cx, o, &notes);
    let ids: Vec<u64> = made.iter().map(|n| n.id.0).collect();
    if !made.is_empty() {
        cmds.push(Command::InsertNotes {
            track,
            clip: c.id,
            notes: made,
        });
    }
    let said = Said::new("ai.tools.replace_notes")
        .nested("old", words::count(old.len(), "notes"))
        .nested("new", words::count(ids.len(), "notes"))
        .nested("clip", target);
    Ok(plan(
        args::one(cmds, "cmd.ai.replace_notes")
            .map_err(|_| ToolError::bad("notes", "the range holds no notes and none were given"))?,
        said,
        json!({"clip_id": c.id.0, "removed": old.len(), "note_ids": ids}),
    ))
}

/// A grid by name: `1/4`, `1/8`, `1/16`, `1/32`, `1/8t` (a triplet), `1/2`, `1/1`.
pub fn grid(s: &str, k: &str) -> Res<Ticks> {
    let (body, triplet) = match s.strip_suffix('t') {
        Some(b) => (b, true),
        None => (s, false),
    };
    let den: i64 = body
        .strip_prefix("1/")
        .and_then(|d| d.parse().ok())
        .filter(|d| [1, 2, 4, 8, 16, 32, 64].contains(d))
        .ok_or_else(|| {
            ToolError::bad(k, format!("{s:?} is not a grid such as 1/8, 1/16 or 1/8t"))
        })?;
    let t = PPQ * 4 / den;
    Ok(Ticks(if triplet { t * 2 / 3 } else { t }))
}

pub const GRIDS: &[&str] = &[
    "1/1", "1/2", "1/4", "1/8", "1/16", "1/32", "1/2t", "1/4t", "1/8t", "1/16t",
];

/// Every clip on `p`'s tracks, by id, mutable lookups avoided: what `edit_notes` needs.
pub fn clip_on(p: &Project, id: ClipId) -> Option<(TrackId, Clip)> {
    args::find_clip(p, id).map(|(t, c)| (t, c.clone()))
}
