//! Editing what is there: notes (transpose, quantise, humanise, move, velocity, length,
//! mute), a clip's bounds, name, mute, colour, loop and track, duplicates, drum patterns.

use super::args::{self, plan, s_id, s_ids, s_len, s_pos, Res, A, MAX_NOTES};
use super::notes::{clip_on, grid, origin, place_notes, NewNote, GRIDS};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::{Clip, ClipId, Command, InstrumentSpec, Note, Ticks, PPQ};
use serde_json::{json, Value};

pub fn edit_notes_schema() -> Value {
    args::object(
        json!({
            "clip_id": s_id("The clip."),
            "note_ids": s_ids("Only these notes (get_clip_notes lists them). Left out: every note of the clip, or of the range."),
            "from": s_pos("Only notes starting here or later"),
            "to": s_pos("…and before here"),
            "pitch_from": {"type": "integer", "minimum": 0, "maximum": 127, "description": "Only notes at this pitch or higher."},
            "pitch_to": {"type": "integer", "minimum": 0, "maximum": 127, "description": "Only notes at this pitch or lower."},
            "transpose_semitones": {"type": "integer", "minimum": -48, "maximum": 48},
            "move_beats": {"type": "number", "minimum": -1000, "maximum": 1000, "description": "Shift in time by this many beats (quarter notes); a note cannot move before its clip's content start."},
            "quantize": {"type": "object", "additionalProperties": false, "description": "Snap starts (and ends) to a grid.",
                "properties": {
                    "grid": {"enum": GRIDS},
                    "strength": {"type": "number", "minimum": 0, "maximum": 1, "default": 1, "description": "1 snaps fully, 0.5 halfway."},
                    "ends": {"type": "boolean", "default": false, "description": "Snap the note ends too."}
                }, "required": ["grid"]},
            "humanize": {"type": "object", "additionalProperties": false, "description": "Small seeded random changes: the same seed gives the same result.",
                "properties": {
                    "timing_beats": {"type": "number", "minimum": 0, "maximum": 0.25, "default": 0.02, "description": "Each start moves within ± this."},
                    "velocity": {"type": "integer", "minimum": 0, "maximum": 40, "default": 8, "description": "Each velocity moves within ± this."},
                    "seed": {"type": "integer", "minimum": 0, "default": 1}
                }},
            "velocity": {"type": "integer", "minimum": 1, "maximum": 127, "description": "Set every velocity."},
            "velocity_scale": {"type": "number", "minimum": 0.05, "maximum": 4, "description": "Multiply velocities (then 1-127)."},
            "length_beats": {"type": "number", "exclusiveMinimum": 0, "maximum": 4096, "description": "Set every length."},
            "length_scale": {"type": "number", "minimum": 0.05, "maximum": 8, "description": "Multiply lengths."},
            "legato": {"type": "boolean", "description": "Extend each note to the next start at another time (chords move together)."},
            "muted": {"type": "boolean", "description": "Mute or unmute the notes."}
        }),
        &["clip_id"],
    )
}

/// Whether a note at content tick `start` is heard: inside the clip's window, and inside a
/// loop's first pass.
pub fn heard(c: &cantus_core::Clip, start: Ticks) -> bool {
    let end = match c.content_length {
        Some(period) => c.offset + period.min(c.length),
        None => c.offset + c.length,
    };
    start >= c.offset && start < end.max(c.offset + Ticks(1))
}

fn hash(seed: u64, n: u64) -> u64 {
    let mut x = seed ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    x ^ (x >> 32)
}

fn jitter(seed: u64, n: u64, r: i64) -> i64 {
    if r <= 0 {
        0
    } else {
        (hash(seed, n) % (2 * r as u64 + 1)) as i64 - r
    }
}

fn snap(t: i64, g: i64, strength: f64) -> i64 {
    let s = (t as f64 / g as f64).round() as i64 * g;
    t + ((s - t) as f64 * strength).round() as i64
}

pub fn edit_notes(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let (track, c) = args::clip(p, a, "clip_id")?;
    let c = c.clone();
    let o = origin(&c);
    let from = args::opt_pos(&map, a, "from")?;
    let to = args::opt_pos(&map, a, "to")?;
    let lo = a.opt_u64("pitch_from")?.unwrap_or(0);
    let hi = a.opt_u64("pitch_to")?.unwrap_or(127);
    let ids: Option<Vec<u64>> = a
        .opt_array("note_ids")?
        .map(|v| {
            v.iter()
                .map(|x| {
                    x.as_u64()
                        .ok_or_else(|| ToolError::bad("note_ids", "ids are numbers"))
                })
                .collect()
        })
        .transpose()?;
    let mut chosen: Vec<Note> = c
        .notes
        .iter()
        .filter(|n| ids.as_ref().is_none_or(|ids| ids.contains(&n.id.0)))
        .filter(|n| from.is_none_or(|f| o + n.start >= f) && to.is_none_or(|t| o + n.start < t))
        .filter(|n| (lo..=hi).contains(&(n.pitch as u64)))
        // The notes the window shows (a split's other half, a trimmed part are its own),
        // unless note_ids names them (fix review #1).
        .filter(|n| ids.is_some() || heard(&c, n.start))
        .cloned()
        .collect();
    let before: std::collections::HashMap<u64, (Ticks, Ticks)> = chosen
        .iter()
        .map(|n| (n.id.0, (n.start, n.length)))
        .collect();
    if let Some(ids) = &ids {
        if let Some(missing) = ids.iter().find(|i| !c.notes.iter().any(|n| n.id.0 == **i)) {
            return Err(ToolError::NotFound(format!(
                "note {missing} in clip {}",
                c.id.0
            )));
        }
    }
    if chosen.is_empty() {
        return Err(ToolError::bad("note_ids", "no note of the clip matches"));
    }
    if chosen.len() > MAX_NOTES {
        return Err(ToolError::bad(
            "note_ids",
            format!("at most {MAX_NOTES} notes in one call"),
        ));
    }
    let mut what = Vec::new();
    let mut ops: Vec<Said> = Vec::new();
    if let Some(st) = a.opt_in("transpose_semitones", -48.0, 48.0)? {
        let st = st.round() as i32;
        for n in &mut chosen {
            let q = n.pitch as i32 + st;
            if !(0..=127).contains(&q) {
                return Err(ToolError::bad(
                    "transpose_semitones",
                    format!("note {} would leave 0-127", n.id.0),
                ));
            }
            n.pitch = q as u8;
        }
        what.push(format!("transpose {st:+}"));
        ops.push(Said::new("ai.phrase.op.transpose").arg("n", format!("{st:+}")));
    }
    if let Some(b) = a.opt_in("move_beats", -1000.0, 1000.0)? {
        let d = Ticks::from_beats_f64(b);
        for n in &mut chosen {
            n.start = (n.start + d).max(Ticks::ZERO);
        }
        what.push(format!("move {}", args::trim_num(b)));
        ops.push(Said::new("ai.phrase.op.move").nested("n", words::num(b)));
    }
    if let Some(q) = a.get("quantize") {
        let qa = A(q);
        let g = grid(qa.str("grid")?, "quantize.grid")?.0;
        let strength = qa.opt_in("strength", 0.0, 1.0)?.unwrap_or(1.0);
        let ends = qa.opt_bool("ends")?.unwrap_or(false);
        for n in &mut chosen {
            let abs = o.0 + n.start.0;
            let end = abs + n.length.0;
            let s = snap(abs, g, strength).max(o.0);
            let e = if ends {
                snap(end, g, strength)
            } else {
                end - abs + s
            };
            n.start = Ticks(s - o.0);
            n.length = Ticks((e - s).max(g / 4).max(1));
        }
        what.push(format!("quantize {}", qa.str("grid")?));
        ops.push(Said::new("ai.phrase.op.quantize").arg("grid", qa.str("grid")?));
    }
    if let Some(h) = a.get("humanize") {
        let ha = A(h);
        let r = Ticks::from_beats_f64(ha.opt_in("timing_beats", 0.0, 0.25)?.unwrap_or(0.02)).0;
        let rv = ha.opt_in("velocity", 0.0, 40.0)?.unwrap_or(8.0) as i64;
        let seed = ha.opt_u64("seed")?.unwrap_or(1);
        for n in chosen.iter_mut() {
            let k = n.id.0;
            n.start = Ticks((n.start.0 + jitter(seed, 2 * k, r)).max(0));
            n.velocity = (n.velocity as i64 + jitter(seed, 2 * k + 1, rv)).clamp(1, 127) as u8;
        }
        what.push("humanize".into());
        ops.push(Said::new("ai.phrase.op.humanize"));
    }
    if let Some(v) = a.opt_in("velocity", 1.0, 127.0)? {
        chosen.iter_mut().for_each(|n| n.velocity = v as u8);
        what.push(format!("velocity {v}"));
        ops.push(Said::new("ai.phrase.op.velocity").arg("n", v));
    }
    if let Some(s) = a.opt_in("velocity_scale", 0.05, 4.0)? {
        chosen.iter_mut().for_each(|n| {
            n.velocity = ((n.velocity as f64 * s).round() as i64).clamp(1, 127) as u8
        });
        what.push(format!("velocity ×{}", args::trim_num(s)));
        ops.push(Said::new("ai.phrase.op.velocity_scale").nested("n", words::num(s)));
    }
    if let Some(l) = a.opt_f64("length_beats")? {
        if !(l > 0.0 && l <= 4096.0) {
            return Err(ToolError::bad(
                "length_beats",
                "must be more than 0 and at most 4096",
            ));
        }
        chosen
            .iter_mut()
            .for_each(|n| n.length = Ticks::from_beats_f64(l).max(Ticks(1)));
        what.push(format!("length {}", args::trim_num(l)));
        ops.push(Said::new("ai.phrase.op.length").nested("n", words::num(l)));
    }
    if let Some(s) = a.opt_in("length_scale", 0.05, 8.0)? {
        chosen
            .iter_mut()
            .for_each(|n| n.length = Ticks(((n.length.0 as f64 * s).round() as i64).max(1)));
        what.push(format!("length ×{}", args::trim_num(s)));
        ops.push(Said::new("ai.phrase.op.length_scale").nested("n", words::num(s)));
    }
    if a.opt_bool("legato")? == Some(true) {
        let mut starts: Vec<i64> = chosen.iter().map(|n| n.start.0).collect();
        starts.sort_unstable();
        starts.dedup();
        for n in chosen.iter_mut() {
            if let Some(next) = starts.iter().find(|s| **s > n.start.0) {
                n.length = Ticks(next - n.start.0);
            }
        }
        what.push("legato".into());
        ops.push(Said::new("ai.phrase.op.legato"));
    }
    if let Some(m) = a.opt_bool("muted")? {
        chosen.iter_mut().for_each(|n| n.muted = m);
        what.push(if m { "mute" } else { "unmute" }.into());
        ops.push(Said::new(if m {
            "ai.phrase.op.mute"
        } else {
            "ai.phrase.op.unmute"
        }));
    }
    if what.is_empty() {
        return Err(ToolError::bad(
            "transpose_semitones",
            "say what to change (transpose_semitones, quantize, humanize, velocity…)",
        ));
    }
    // Where the notes land must sound (review #13): not before the clip's window (its hidden
    // part), not past a loop's first pass; past the end of a clip that does not loop, the
    // clip grows to the bar, as add_notes does.
    let mut grow = None;
    for x in &chosen {
        // Only what this call moved or lengthened is checked, and grows the clip.
        let (s0, l0) = before[&x.id.0];
        if x.start == s0 && x.length == l0 {
            continue;
        }
        if x.start < c.offset {
            return Err(ToolError::bad(
                "move_beats",
                format!(
                    "note {} would move before the clip's start, where it is not heard",
                    x.id.0
                ),
            ));
        }
        if let Some(period) = c.content_length {
            if x.start >= c.offset + period {
                return Err(ToolError::bad(
                    "move_beats",
                    format!(
                        "note {} would leave the loop's first pass, where it is not heard",
                        x.id.0
                    ),
                ));
            }
        } else if o + x.start + x.length > c.end() && x.start + x.length > s0 + l0 && heard(&c, s0)
        {
            let end = o + x.start + x.length;
            grow = Some(grow.map_or(end, |g: Ticks| g.max(end)));
        }
    }
    let mut cmds = Vec::new();
    if let Some(end) = grow {
        let (_, to) = map.bars_around(c.start, end);
        cmds.push(Command::SetClipBounds {
            track,
            clip: c.id,
            start: c.start,
            length: to - c.start,
        });
    }
    let n = chosen.len();
    let (lo_t, hi_t) = chosen
        .iter()
        .fold((Ticks(i64::MAX), Ticks(i64::MIN)), |(l, h), x| {
            (l.min(o + x.start), h.max(o + x.start + x.length))
        });
    let said = Said::new("ai.tools.edit_notes")
        .nested("n", words::count(n, "notes"))
        .nested(
            "clip",
            words::clip_span(p, &map, track, &c.name, lo_t, hi_t),
        )
        .nested("what", words::list(ops));
    let ids: Vec<u64> = chosen.iter().map(|n| n.id.0).collect();
    cmds.push(Command::SetNotes {
        track,
        clip: c.id,
        notes: chosen,
    });
    Ok(plan(
        args::one(cmds, "cmd.ai.edit_notes")?,
        said,
        json!({"clip_id": c.id.0, "note_ids": ids, "done": what, "clip_grown": grow.is_some()}),
    ))
}

pub fn set_clip_schema() -> Value {
    args::object(
        json!({
            "clip_id": s_id("The clip."),
            "start": s_pos("Move the clip here (its content moves with it)"),
            "length": s_len("Its new length (the window; its content stays)"),
            "name": {"type": "string", "minLength": 1, "maxLength": 80},
            "muted": {"type": "boolean"},
            "color": {"type": ["string", "null"], "pattern": "^#[0-9a-fA-F]{6}$", "description": "\"#rrggbb\", or null for the track's."},
            "loop_beats": {"type": ["number", "null"], "minimum": 0.015625, "description": "Loop the content every this many beats (at least 1/64 beat) across the window; null stops looping."},
            "track_id": s_id("Move the clip to this track (same position).")
        }),
        &["clip_id"],
    )
}

pub fn set_clip(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let (track, c) = args::clip(p, a, "clip_id")?;
    let c = c.clone();
    let mut cmds = Vec::new();
    let mut done = serde_json::Map::new();
    let start = args::opt_pos(&map, a, "start")?;
    let length = args::opt_len(&map, start.unwrap_or(c.start), a, "length")?;
    if start.is_some() || length.is_some() {
        let s = start.unwrap_or(c.start);
        let l = length.unwrap_or(c.length);
        cmds.push(Command::SetClipBounds {
            track,
            clip: c.id,
            start: s,
            length: l,
        });
        done.insert("start".into(), args::pos_json(&map, s));
        done.insert("end".into(), args::pos_json(&map, s + l));
    }
    if let Some(name) = a.opt_name("name", 80)? {
        done.insert("name".into(), json!(name));
        cmds.push(Command::SetClipName {
            track,
            clip: c.id,
            name,
        });
    }
    if let Some(m) = a.opt_bool("muted")? {
        done.insert("muted".into(), json!(m));
        cmds.push(Command::SetClipMute {
            track,
            clip: c.id,
            muted: m,
        });
    }
    if a.0.get("color").is_some() {
        let color = a
            .opt_str("color")?
            .map(|s| super::tracks::parse_color(s, "color"))
            .transpose()?;
        done.insert("color".into(), a.0["color"].clone());
        cmds.push(Command::SetClipColor {
            track,
            clip: c.id,
            color,
        });
    }
    if a.0.get("loop_beats").is_some() {
        let content_length = match a.opt_f64("loop_beats")? {
            None => None,
            Some(b) if (1.0 / 64.0..=100_000.0).contains(&b) => Some(Ticks::from_beats_f64(b)),
            Some(_) => {
                return Err(ToolError::bad(
                    "loop_beats",
                    "is at least 1/64 beat (0.015625)",
                ))
            }
        };
        done.insert("loop_beats".into(), a.0["loop_beats"].clone());
        cmds.push(Command::SetClipLoop {
            track,
            clip: c.id,
            content_length,
        });
    }
    if a.has("track_id") {
        let to = args::track(p, a, "track_id")?;
        if to != track {
            done.insert("track_id".into(), json!(to.0));
            cmds.push(Command::MoveClipToTrack {
                from: track,
                to,
                clip: c.id,
            });
        }
    }
    let said = Said::new("ai.tools.set_clip")
        .nested("clip", words::clip(p, &map, track, &c))
        .nested("what", words::fields(done.keys()));
    done.insert("clip_id".into(), json!(c.id.0));
    Ok(plan(
        args::one(cmds, "cmd.ai.set_clip")?,
        said,
        Value::Object(done),
    ))
}

pub fn duplicate_schema() -> Value {
    args::object(
        json!({
            "clip_id": s_id("The clip to copy."),
            "times": {"type": "integer", "minimum": 1, "maximum": 64, "default": 1, "description": "How many copies, one after another."},
            "at": s_pos("Where the first copy starts (left out: right after the clip)"),
            "track_id": s_id("Put the copies on this track (left out: the clip's own).")
        }),
        &["clip_id"],
    )
}

/// A copy of `c` at `start` with fresh ids for it and its notes (linked to nothing).
pub fn copy_clip(cx: &mut ToolCx<'_>, c: &Clip, start: Ticks) -> Clip {
    let bus = cx.bus_mut();
    let mut k = c.clone();
    k.id = ClipId(bus.alloc_id());
    k.start = start;
    k.link = None;
    for n in &mut k.notes {
        n.id = cantus_core::NoteId(bus.alloc_id());
    }
    k
}

pub fn duplicate_clip(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let (track, c) = args::clip(p, a, "clip_id")?;
    let c = c.clone();
    let times = a.opt_u64("times")?.unwrap_or(1);
    if !(1..=64).contains(&times) {
        return Err(ToolError::bad("times", "is 1 to 64"));
    }
    let to = args::opt_track(p, a, "track_id")?.unwrap_or(track);
    let target = words::clip(p, &map, track, &c);
    let mut at = args::opt_pos(&map, a, "at")?.unwrap_or(c.end());
    let mut cmds = Vec::new();
    let mut made = Vec::new();
    for _ in 0..times {
        let k = copy_clip(cx, &c, at);
        made.push(json!({"clip_id": k.id.0, "start": args::pos_json(&map, at)}));
        at += c.length;
        cmds.push(Command::InsertClip {
            track: to,
            clip: Box::new(k),
        });
    }
    let said = Said::new("ai.tools.duplicate_clip")
        .nested("clip", target)
        .nested("n", words::count(times as usize, "times"));
    Ok(plan(
        args::one(cmds, "cmd.ai.duplicate_clip")?,
        said,
        json!({"copies": made}),
    ))
}

/// General MIDI's drum notes, by the names an agent uses.
pub const GM_DRUMS: &[(&str, u8)] = &[
    ("kick", 36),
    ("kick2", 35),
    ("rim", 37),
    ("snare", 38),
    ("clap", 39),
    ("snare2", 40),
    ("low_tom", 45),
    ("closed_hat", 42),
    ("pedal_hat", 44),
    ("open_hat", 46),
    ("mid_tom", 47),
    ("high_mid_tom", 48),
    ("crash", 49),
    ("high_tom", 50),
    ("ride", 51),
    ("china", 52),
    ("ride_bell", 53),
    ("tambourine", 54),
    ("splash", 55),
    ("cowbell", 56),
    ("crash2", 57),
    ("shaker", 82),
    ("maracas", 70),
    ("conga_high", 62),
    ("conga_low", 64),
    ("bongo_high", 60),
    ("bongo_low", 61),
    ("woodblock", 76),
    ("triangle", 81),
];

pub fn drums_schema() -> Value {
    let names: Vec<&str> = GM_DRUMS.iter().map(|(n, _)| *n).collect();
    args::object(
        json!({
            "track_id": s_id("A drum track (Pulsus, or any instrument on the General MIDI drum map)."),
            "at": s_pos("Where the pattern starts"),
            "bars": {"type": "integer", "minimum": 1, "maximum": 256, "default": 1, "description": "How many bars to fill; each row's steps repeat."},
            "step": {"enum": GRIDS, "default": "1/16", "description": "The length of one step."},
            "rows": {"type": "array", "minItems": 1, "maxItems": 32, "items": {
                "type": "object", "additionalProperties": false,
                "properties": {
                    "pad": {"type": ["integer", "string"], "description": format!("The drum: a MIDI note, a pad's name on this kit, or one of {}.", names.join(", "))},
                    "steps": {"type": "string", "minLength": 1, "maxLength": 256, "description": "One character a step: x hit, X accent, o ghost, . or - rest (spaces and | are ignored): \"x...x...x...x...\" is four on the floor in 1/16 steps."},
                    "velocity": {"type": "integer", "minimum": 1, "maximum": 127, "default": 100}
                }, "required": ["pad", "steps"]}},
            "swing": {"type": "number", "minimum": 0, "maximum": 1, "default": 0, "description": "Delay every second step: 0 straight, 1 a full triplet shuffle."},
            "clip_id": s_id("Write into this clip instead of the one under the start or a new one.")
        }),
        &["track_id", "at", "rows"],
    )
}

pub fn write_drum_pattern(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let track = args::track(p, a, "track_id")?;
    let t = p.track(track).expect("checked");
    let at = args::pos(&map, a, "at")?;
    let bars = a.opt_in("bars", 1.0, 256.0)?.unwrap_or(1.0) as i64;
    let step = grid(a.opt_str("step")?.unwrap_or("1/16"), "step")?;
    let swing = a.opt_in("swing", 0.0, 1.0)?.unwrap_or(0.0);
    let (bar, _, _) = map.bbt(at);
    let end = at + (map.bar_tick(bar + bars) - map.bar_tick(bar));
    let mut notes = Vec::new();
    let rows = a.array("rows")?;
    if rows.is_empty() || rows.len() > 32 {
        return Err(ToolError::bad("rows", "give 1 to 32 rows"));
    }
    for (i, row) in rows.iter().enumerate() {
        let r = A(row);
        let k = format!("rows[{i}]");
        let pitch = drum_note(&t.instrument, r.req("pad")?, &format!("{k}.pad"))?;
        let vel = r.opt_in("velocity", 1.0, 127.0)?.unwrap_or(100.0) as i64;
        if r.str("steps")?.chars().count() > 256 {
            return Err(ToolError::bad(
                format!("{k}.steps"),
                "at most 256 characters",
            ));
        }
        let steps: Vec<char> = r
            .str("steps")?
            .chars()
            .filter(|c| !matches!(c, ' ' | '|'))
            .collect();
        if steps.is_empty()
            || steps
                .iter()
                .any(|c| !matches!(c, 'x' | 'X' | 'o' | '.' | '-'))
        {
            return Err(ToolError::bad(format!("{k}.steps"), "use only x X o . -"));
        }
        let mut s = 0usize;
        loop {
            let t0 = at.0 + step.0 * s as i64;
            if t0 >= end.0 {
                break;
            }
            let v = match steps[s % steps.len()] {
                'x' => Some(vel),
                'X' => Some((vel + 24).min(127)),
                'o' => Some((vel / 2).max(1)),
                _ => None,
            };
            if let Some(v) = v {
                let push = if s % 2 == 1 {
                    (step.0 as f64 * swing / 3.0).round() as i64
                } else {
                    0
                };
                notes.push(NewNote {
                    at: Ticks(t0 + push),
                    len: Ticks((step.0 / 2).max(1)),
                    pitch,
                    velocity: v as u8,
                });
            }
            s += 1;
        }
    }
    if notes.is_empty() {
        return Err(ToolError::bad("rows", "the steps hold no hits"));
    }
    if notes.len() > MAX_NOTES {
        return Err(ToolError::bad(
            "bars",
            format!("more than {MAX_NOTES} hits: write fewer bars per call"),
        ));
    }
    let clip = a.opt_u64("clip_id")?.map(ClipId);
    let tname = t.name.clone();
    let (cmds, id, created, ids) = place_notes(cx, track, clip, &notes, "Drums", true)?;
    let said = Said::new("ai.tools.write_drum_pattern")
        .name("track", args::clean(&tname))
        .nested("bars", words::count(bars as usize, "bars"))
        .nested("at", words::pos(&map, at));
    Ok(plan(
        args::one(cmds, "cmd.ai.write_drum_pattern")?,
        said,
        json!({"clip_id": id.0, "clip_created": created, "hits": ids.len()}),
    ))
}

/// The note a row names: a number, a pad of the kit by name, or a General MIDI drum.
fn drum_note(spec: &InstrumentSpec, v: &Value, k: &str) -> Res<u8> {
    if let Some(n) = v.as_u64() {
        return u8::try_from(n)
            .ok()
            .filter(|n| *n <= 127)
            .ok_or_else(|| ToolError::bad(k, "0 to 127"));
    }
    let name = v
        .as_str()
        .ok_or_else(|| ToolError::bad(k, "a note or a drum's name"))?;
    let low = name.to_lowercase();
    if let InstrumentSpec::Pulsus(kit) = spec {
        if let Some(pad) = kit.pads.iter().find(|p| p.name.to_lowercase() == low) {
            return Ok(pad.pitch);
        }
    }
    GM_DRUMS
        .iter()
        .find(|(n, _)| *n == low)
        .map(|(_, p)| *p)
        .ok_or_else(|| {
            ToolError::bad(
                k,
                format!("{name:?} is not a pad of this kit nor a General MIDI drum name"),
            )
        })
}

/// Clip lookups for other modules.
pub fn clip_of(cx: &ToolCx<'_>, id: ClipId) -> Option<(cantus_core::TrackId, Clip)> {
    clip_on(cx.project(), id)
}

/// A sixteenth, for the pedal and the patterns.
pub const SIXTEENTH: Ticks = Ticks(PPQ / 4);
