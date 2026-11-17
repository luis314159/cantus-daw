//! Movement: automation lanes (points in the parameter's own units, FL's shapes) and
//! modulators (LFO, envelope, follower, steps, random; the X-Y pad and the formula
//! controller, ADRs 0147, 0148) with their routes, axes and ranges.

use super::args::{self, plan, s_id, s_len, s_pos, Res, A, MAX_POINTS};
use super::json::{firmus_key, input_names};
use super::notes::origin;
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::automation::range::range_of;
use cantus_core::automation::{edit as auto_edit, AutoPoint, AutoTarget, Automation, Shape};
use cantus_core::modulator::formula::{ErrorKind, FormulaError, Program, MAX_TEXT};
use cantus_core::modulator::xy::{MAX_INERTIA_MS, MAX_PATH, MAX_PATH_BEATS, MIN_PATH_BEATS};
use cantus_core::modulator::{
    EnvTrigger, LfoShape, Listen, ModRate, ModRoute, ModSource, Retrigger, RouteLink, RouteRange,
    XyPath, MAX_PER_HOST, MAX_STEPS,
};
use cantus_core::{
    Command, EffectId, FirmusParam, InstrumentSpec, ModHost, ModKind, Modulator, ModulatorId,
    Project, StripId, Ticks, TrackId,
};
use serde_json::{json, Value};

/// The JSON schema of a target.
pub fn target_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "description": "What moves. One of: {track_id, param: \"gain_db\"|\"pan\"} a track's own level or pan; {track_id, param: <instrument key>} its instrument (get_device/get_unit list keys; Pulsus pads cannot be automated); {strip_id, param: \"gain_db\"|\"pan\"|\"width\"} a strip's (strip_id \"master\" for the master); {strip_id, param: \"send\", to: <strip id>} a send's level; {effect_id, param: <key>} an effect's parameter; {effect_id or track_id, plugin_param: <id>} a hosted plug-in's (0-1 normalised); {modulator_id} (or param \"amount\") a modulator's amount; {modulator_id, param: \"x\"|\"y\"} an X-Y pad's axis, {modulator_id, param: \"a\"|\"b\"|\"c\"} a formula controller's knob (0-1 each). min, max and value are what answers carry; they are ignored when given.",
        "properties": {
            "track_id": {"type": "integer", "minimum": 1},
            "strip_id": {"type": ["integer", "string"]},
            "effect_id": {"type": "integer", "minimum": 1},
            "modulator_id": {"type": "integer", "minimum": 1},
            "param": {"type": "string"},
            "to": {"type": "integer", "minimum": 1},
            "plugin_param": {"type": "integer", "minimum": 0},
            "min": {"type": "number", "description": "Read only: the range's bottom (ignored when given)."},
            "max": {"type": "number", "description": "Read only: the range's top (ignored when given)."},
            "value": {"type": "number", "description": "Read only: a controller input's position now (ignored when given)."}
        }
    })
}

/// A target from its JSON, checked to exist in `p`.
pub fn target_of(p: &Project, v: &Value, k: &str) -> Res<AutoTarget> {
    let a = A(v);
    let param = a.opt_str("param")?;
    let bad = |why: &str| ToolError::bad(k, why.to_string());
    let t = if let Some(m) = a.opt_u64("modulator_id")? {
        let id = ModulatorId(m);
        match param {
            None | Some("amount") => AutoTarget::ModAmount(id),
            // fxd: a controller's input by its name (what `target_json` writes).
            Some(name) => {
                let kind = p
                    .modulator(id)
                    .map(|(_, _, x)| x.source.kind())
                    .ok_or_else(|| bad("that target does not exist in the project"))?;
                let names = input_names(Some(kind));
                let input = names.iter().position(|n| *n == name).ok_or_else(|| {
                    bad(&if names.is_empty() {
                        format!(
                            "a modulator's param is \"amount\" (or left out): {} has no inputs",
                            kind_id(kind)
                        )
                    } else {
                        format!(
                            "a modulator's param is \"amount\" or one of its inputs: {}",
                            names.join(", ")
                        )
                    })
                })?;
                AutoTarget::ModInput {
                    modulator: id,
                    input: input as u8,
                }
            }
        }
    } else if a.has("effect_id") {
        let (_, e) = args::effect(p, a, "effect_id")?;
        match (a.opt_u64("plugin_param")?, param) {
            (Some(id), _) => AutoTarget::Plugin {
                effect: e,
                id: id as u32,
            },
            (None, Some(key)) => {
                let host = p.effect_host(e).expect("found");
                // fx3:contract — routed: a board's `p{id}.…` is its pedal's.
                let spec = &p.effect(host, e).expect("found").spec;
                let kind = spec.kind;
                if spec.param(key).is_none() {
                    return Err(bad(&format!(
                        "{} has no parameter {key:?} (get_unit lists them)",
                        args::effect_names(kind).1
                    )));
                }
                AutoTarget::Effect {
                    effect: e,
                    key: key.to_string(),
                }
            }
            _ => return Err(bad("an effect target needs param (a key) or plugin_param")),
        }
    } else if let Some(sv) = a.get("strip_id") {
        let s = args::strip_of(p, sv, "strip_id")?;
        match (param.unwrap_or(""), s.is_master()) {
            ("gain_db", false) => AutoTarget::StripGain(s),
            ("pan", false) => AutoTarget::StripPan(s),
            ("width", false) => AutoTarget::StripWidth(s),
            ("send", false) => AutoTarget::Send {
                strip: s,
                to: StripId(a.u64("to")?),
            },
            ("gain_db", true) => AutoTarget::MasterGain,
            ("pan", true) => AutoTarget::MasterPan,
            ("width", true) => AutoTarget::MasterWidth,
            _ => {
                return Err(bad(
                    "a strip's param is gain_db, pan, width or send (with to)",
                ))
            }
        }
    } else if a.has("track_id") {
        let track = args::track(p, a, "track_id")?;
        let spec = &p.track(track).expect("checked").instrument;
        if let Some(id) = a.opt_u64("plugin_param")? {
            AutoTarget::PluginInstrument {
                track,
                id: id as u32,
            }
        } else {
            match param.ok_or_else(|| bad("a track target needs param"))? {
                "gain_db" => AutoTarget::TrackGain(track),
                "pan" => AutoTarget::TrackPan(track),
                key => instrument_target(spec, track, key).ok_or_else(|| {
                    bad(&format!(
                        "the instrument has no automatable parameter {key:?}"
                    ))
                })?,
            }
        }
    } else {
        return Err(bad("name a track_id, strip_id, effect_id or modulator_id"));
    };
    range_of(p, &t).ok_or_else(|| bad("that target does not exist in the project"))?;
    Ok(t)
}

fn instrument_target(spec: &InstrumentSpec, track: TrackId, key: &str) -> Option<AutoTarget> {
    match spec {
        InstrumentSpec::Native(n) => {
            (n.kind.param(key).is_some() && n.kind.automatable(key)).then(|| {
                AutoTarget::Instrument {
                    track,
                    key: key.to_string(),
                }
            })
        }
        InstrumentSpec::Firmus(_) => FirmusParam::ALL
            .into_iter()
            .find(|f| firmus_key(*f) == key)
            .map(|param| AutoTarget::Firmus { track, param }),
        _ => None,
    }
}

/// The track whose lanes hold automation of `t` by default.
fn lane_track(p: &Project, t: &AutoTarget) -> Option<TrackId> {
    let first_on = |s: StripId| p.tracks_on_strip(s).next().map(|t| t.id);
    match t {
        AutoTarget::TrackGain(id) | AutoTarget::TrackPan(id) => Some(*id),
        AutoTarget::Instrument { track, .. }
        | AutoTarget::Firmus { track, .. }
        | AutoTarget::PluginInstrument { track, .. }
        | AutoTarget::NoteFx { track, .. } => Some(*track),
        AutoTarget::StripGain(s) | AutoTarget::StripPan(s) | AutoTarget::StripWidth(s) => {
            first_on(*s)
        }
        AutoTarget::Send { strip, .. } => first_on(*strip),
        AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. } => p
            .effect_host(*effect)
            .filter(|h| !h.is_master())
            .and_then(first_on),
        // fxd: a controller's input is drawn on its host's lanes.
        AutoTarget::ModInput { modulator, .. } => match p.modulator(*modulator)?.0 {
            ModHost::Track(t) => Some(t),
            ModHost::Strip(s) => first_on(s),
            ModHost::Master => None,
        },
        _ => None,
    }
}

pub const SHAPES: &[&str] = &["curve", "s_curve", "hold", "stairs", "pulse", "wave"];

fn shape_of(s: &str) -> Shape {
    match s {
        "s_curve" => Shape::SCurve,
        "hold" => Shape::Hold,
        "stairs" => Shape::Stairs,
        "pulse" => Shape::Pulse,
        "wave" => Shape::Wave,
        _ => Shape::Curve,
    }
}

pub fn automate_schema() -> Value {
    args::object(
        json!({
            "target": target_schema(),
            "points": {"type": "array", "minItems": 1, "maxItems": MAX_POINTS, "items": {
                "type": "object", "additionalProperties": false, "required": ["at", "value"],
                "properties": {
                    "at": s_pos("Where"),
                    "value": {"type": "number", "description": "In the parameter's own unit (dB, Hz, 0-1, ms…), within its range."},
                    "shape": {"enum": SHAPES, "default": "curve", "description": "How the curve travels to the next point: curve (bent by tension; 0 is a straight line), s_curve, hold (a step at the next point), stairs, pulse, wave."},
                    "tension": {"type": "number", "minimum": -1, "maximum": 1, "default": 0}
                }}},
            "lane_track_id": s_id("The track whose automation lanes hold the clip (left out: the target's track, or the first track playing through the target's strip; needed for the master)."),
            "start": s_pos("The clip's start (left out: the bar of the first point)"),
            "length": s_len("The clip's length (left out: to the bar after the last point)"),
            "name": {"type": "string", "minLength": 1, "maxLength": 80}
        }),
        &["target", "points"],
    )
}

/// Draw automation; never over the user's points (review #2): a span that already holds
/// points is refused here and redrawn by [`replace_automation`], which asks.
pub fn automate(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    draw(cx, a, false)
}

/// Draw automation over a span that holds points: they go (destructive, asked).
pub fn replace_automation(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    draw(cx, a, true)
}

fn draw(cx: &mut ToolCx<'_>, a: &Value, replace: bool) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let target = target_of(p, a.req("target")?, "target")?;
    let range = range_of(p, &target).expect("checked");
    let lane = match a.opt_u64("lane_track_id")? {
        Some(_) => args::track(p, a, "lane_track_id")?,
        None => lane_track(p, &target).ok_or_else(|| {
            ToolError::bad(
                "lane_track_id",
                "say which track's lanes hold this automation",
            )
        })?,
    };
    let items = a.array("points")?;
    if items.is_empty() || items.len() > MAX_POINTS {
        return Err(ToolError::bad(
            "points",
            format!("give 1 to {MAX_POINTS} points"),
        ));
    }
    let mut pts: Vec<(Ticks, AutoPoint)> = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        let pa = A(it);
        let k = |f: &str| format!("points[{i}].{f}");
        let at = args::pos_of(&map, pa.req("at")?, &k("at"))?;
        let v = pa
            .f64(&k("value").replace(&format!("points[{i}]."), ""))
            .or_else(|_| pa.f64("value"))?;
        let (lo, hi) = (
            range.min.min(range.max) as f64,
            range.max.max(range.min) as f64,
        );
        if v < lo - 1e-6 || v > hi + 1e-6 {
            return Err(ToolError::bad(k("value"), format!("must be {lo} to {hi}")));
        }
        let mut point = AutoPoint::new(Ticks::ZERO, range.pos(v as f32));
        point.shape = shape_of(pa.opt_choice("shape", SHAPES)?.unwrap_or("curve"));
        point.tension = pa.opt_in("tension", -1.0, 1.0)?.unwrap_or(0.0) as f32;
        pts.push((at, point));
    }
    pts.sort_by_key(|(t, _)| *t);
    let first = pts[0].0;
    let last = pts[pts.len() - 1].0;
    let t = p.track(lane).expect("checked");
    // A clip of this target already under the first point takes the points (drawn over
    // what was there between the first and the last); otherwise a new clip.
    let existing = t
        .clips
        .iter()
        .find(|c| {
            c.automation.as_ref().is_some_and(|x| x.target == target)
                && c.start <= first
                && first < c.end()
        })
        .cloned();
    // Where the new curve will be heard: from the first point in the clip that takes it,
    // from the new clip's start otherwise. Any clip of this target with points there is the
    // user's curve: never drawn over without replace (fix review 2 #7).
    let span_start = if existing.is_some() {
        first
    } else {
        args::opt_pos(&map, a, "start")?.unwrap_or_else(|| map.bar_start(first))
    };
    let mut clear = Vec::new();
    let mut other_points = 0;
    // Every track's clips of this target, wherever their lane is (fix review 3 #4).
    for (owner, c) in p
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c)))
    {
        if existing.as_ref().is_some_and(|e| e.id == c.id) {
            continue;
        }
        let Some(auto) = c.automation.as_ref().filter(|x| x.target == target) else {
            continue;
        };
        let o = origin(c);
        let inside = |tick: Ticks| {
            let at = o + tick;
            at >= span_start && at <= last && at >= c.start && at < c.end()
        };
        let n = auto.points.iter().filter(|q| inside(q.tick)).count();
        if n > 0 {
            let mut kept = auto.clone();
            kept.points.retain(|q| !inside(q.tick));
            other_points += n;
            clear.push(Command::SetClipAutomation {
                track: owner,
                clip: c.id,
                automation: Some(kept),
            });
        }
    }
    if other_points > 0 && !replace {
        return Err(ToolError::Refused(format!(
            "another clip of this parameter has {other_points} point(s) from {} to {}: draw where there are none, or redraw with replace_automation (it asks the user)",
            args::bar_beat_text(&map, span_start),
            args::bar_beat_text(&map, last)
        )));
    }
    let said = Said::new("ai.tools.automate")
        .nested("n", words::count(pts.len(), "points"))
        .nested("target", words::target(p, &target))
        .name("track", args::clean(&t.name))
        .nested("bars", bars_said(&map, first, last + Ticks(1)));
    if let Some(c) = existing {
        let o = origin(&c);
        let mut auto = c.automation.clone().expect("automation");
        let held = auto
            .points
            .iter()
            .filter(|q| q.tick >= first - o && q.tick <= last - o)
            .count();
        if held > 0 && !replace {
            return Err(ToolError::Refused(format!(
                "the lane already has {held} point(s) of this parameter from {} to {}: draw where it has none, or redraw with replace_automation (it asks the user)",
                args::bar_beat_text(&map, first),
                args::bar_beat_text(&map, last)
            )));
        }
        let said = if held + other_points > 0 {
            Said::new("ai.tools.replace_automation")
                .nested("target", words::target(p, &target))
                .nested("old", words::count(held + other_points, "points"))
                .nested("bars", bars_said(&map, first, last + Ticks(1)))
        } else {
            said
        };
        let new: Vec<AutoPoint> = pts
            .iter()
            .map(|(at, pt)| AutoPoint {
                tick: *at - o,
                ..*pt
            })
            .collect();
        // Over [first, last]: a point at the last new one's tick is replaced, not doubled.
        auto_edit::splice(&mut auto.points, first - o, last - o + Ticks(1), new);
        let mut cmds = clear;
        if last >= c.end() {
            let (_, end) = map.bars_around(c.start, last);
            let end = if end <= last {
                last + Ticks(cantus_core::PPQ / 4)
            } else {
                end
            };
            cmds.push(Command::SetClipBounds {
                track: lane,
                clip: c.id,
                start: c.start,
                length: end - c.start,
            });
        }
        cmds.push(Command::SetClipAutomation {
            track: lane,
            clip: c.id,
            automation: Some(auto),
        });
        return Ok(plan(
            args::one(cmds, "cmd.ai.automate")?,
            said,
            json!({"clip_id": c.id.0, "clip_created": false, "lane_track_id": lane.0}),
        ));
    }
    let start = span_start;
    if start > first {
        return Err(ToolError::bad(
            "start",
            "must be at or before the first point",
        ));
    }
    let length = match args::opt_len(&map, start, a, "length")? {
        Some(l) => l,
        // To the bar line after the last point; a point on a bar line gets a sixteenth's
        // tail so it is inside the clip, not a whole bar more.
        None => {
            let end = map.bars_around(start, last).1;
            (if end <= last {
                last + Ticks(cantus_core::PPQ / 4)
            } else {
                end
            }) - start
        }
    };
    let name = a
        .opt_name("name", 80)?
        .unwrap_or_else(|| "Automation".into());
    let mut clip = cx.bus_mut().new_clip(name, start, length);
    let points: Vec<AutoPoint> = pts
        .iter()
        .map(|(at, pt)| AutoPoint {
            tick: *at - start,
            ..*pt
        })
        .collect();
    clip.automation = Some(Automation::new(target.clone(), points));
    let id = clip.id;
    let said = if other_points > 0 {
        Said::new("ai.tools.replace_automation")
            .nested("target", words::target(cx.project(), &target))
            .nested("old", words::count(other_points, "points"))
            .nested("bars", bars_said(&map, start, last + Ticks(1)))
    } else {
        said
    };
    let mut cmds = clear;
    cmds.push(Command::InsertClip {
        track: lane,
        clip: Box::new(clip),
    });
    Ok(plan(
        args::one(cmds, "cmd.ai.automate")?,
        said,
        json!({"clip_id": id.0, "clip_created": true, "lane_track_id": lane.0}),
    ))
}

// --- modulators -------------------------------------------------------------------------

/// The synced rates' names, as `cantus_dsp::modfx_clock::DIVISIONS` orders them.
pub const DIVISIONS: [&str; 17] = [
    "8/1", "4/1", "2/1", "1/1", "1/2.", "1/2", "1/4.", "1/2t", "1/4", "1/8.", "1/4t", "1/8",
    "1/16.", "1/8t", "1/16", "1/16t", "1/32",
];

fn rate_of(v: &Value, k: &str) -> Res<ModRate> {
    if let Some(s) = v.as_str() {
        let i = DIVISIONS.iter().position(|d| *d == s).ok_or_else(|| {
            ToolError::bad(
                k,
                format!("a synced rate is one of {}", DIVISIONS.join(", ")),
            )
        })?;
        return Ok(ModRate::Sync { division: i as u8 });
    }
    let hz = v
        .as_f64()
        .ok_or_else(|| ToolError::bad(k, "a division (\"1/4\") or a rate in Hz"))?;
    if !(ModRate::MIN_HZ as f64..=ModRate::MAX_HZ as f64).contains(&hz) {
        return Err(ToolError::bad(
            k,
            format!("Hz must be {} to {}", ModRate::MIN_HZ, ModRate::MAX_HZ),
        ));
    }
    Ok(ModRate::Hz { hz: hz as f32 })
}

/// The kinds `add_modulator` makes, by the names agents write.
pub const KINDS: [&str; 7] = [
    "lfo", "envelope", "follower", "step", "random", "xy_pad", "formula",
];

/// A kind's name as agents write it.
pub fn kind_id(kind: ModKind) -> &'static str {
    match kind {
        ModKind::Lfo => "lfo",
        ModKind::Envelope => "envelope",
        ModKind::Follower => "follower",
        ModKind::Step => "step",
        ModKind::Random => "random",
        ModKind::XyPad => "xy_pad",
        ModKind::Formula => "formula",
    }
}

/// A route's range as JSON's schema (fxd, ADR 0147 §2); `null` too where it may be cleared.
fn range_schema(nullable: bool) -> Value {
    json!({"type": if nullable { json!(["object", "null"]) } else { json!("object") },
    "additionalProperties": false, "required": ["from", "to"],
    "description": "X-Y pad and formula: the route's own range over its output (FL's link range) — at the output's bottom it adds `from`, at its top `to` (shares of the parameter's travel, -1…1, before depth and amount), bent between by `curve` (-1…1, the automation's curve; 0 straight). Left out: the output as it is.",
    "properties": {
        "from": {"type": "number", "minimum": -1, "maximum": 1},
        "to": {"type": "number", "minimum": -1, "maximum": 1},
        "curve": {"type": "number", "minimum": -1, "maximum": 1, "default": 0}
    }})
}

/// A route's JSON schema: what it drives, how far and, for a controller, through which
/// axis and range.
fn route_schema() -> Value {
    json!({"type": "object", "additionalProperties": false, "required": ["target", "depth"],
    "properties": {
        "target": target_schema(),
        "depth": {"type": "number", "minimum": -1, "maximum": 1},
        "axis": {"enum": ["x", "y"], "default": "x", "description": "X-Y pad: the axis the route reads."},
        "range": range_schema(false)
    }})
}

pub fn modulator_schema(adding: bool) -> Value {
    let mut props = json!({
        "name": {"type": "string", "maxLength": 40},
        "amount": {"type": "number", "minimum": 0, "maximum": 1, "description": "Scales every route at once."},
        "muted": {"type": "boolean"},
        "rate": {"type": ["string", "number"], "description": format!("LFO and random: a synced division ({}) or Hz (0.01-40).", DIVISIONS.join(", "))},
        "shape": {"enum": ["sine", "triangle", "square", "saw_up", "saw_down"], "description": "LFO wave."},
        "phase": {"type": "number", "minimum": 0, "maximum": 1},
        "retrigger_on_notes": {"type": "boolean", "description": "LFO: restart at every note of the host track (else it follows the song position)."},
        "unipolar": {"type": "boolean", "description": "0…1 instead of −1…1 (LFO, steps, random; an X-Y pad's axes: left and bottom are the parameters' own values instead of the centre; a formula is 0…1 unless bipolar)."},
        "attack_ms": {"type": "number", "minimum": 0, "maximum": 10000},
        "decay_ms": {"type": "number", "minimum": 0, "maximum": 10000},
        "sustain": {"type": "number", "minimum": 0, "maximum": 1},
        "release_ms": {"type": "number", "minimum": 0, "maximum": 10000},
        "listen_strip_id": args::s_strip("Follower: the strip it listens to (left out: its host's) — a sidechain made of a modulator"),
        "gain_db": {"type": "number", "minimum": -48, "maximum": 48, "description": "Follower: gain on the level it reads."},
        "steps": {"type": "array", "minItems": 1, "maxItems": MAX_STEPS, "items": {"type": "number", "minimum": -1, "maximum": 1}, "description": "Step sequence values."},
        "step_rate": {"enum": DIVISIONS, "description": "Steps: the length of one step."},
        "glide": {"type": "number", "minimum": 0, "maximum": 1},
        "smooth": {"type": "boolean", "description": "Random: glide between values instead of sample-and-hold."},
        "seed": {"type": "integer", "minimum": 1, "maximum": 999}
    });
    // fxd: the controllers' settings (ADRs 0147, 0148), then the routes.
    let controllers = json!({
        "x": {"type": "number", "minimum": 0, "maximum": 1, "description": "X-Y pad: the puck's horizontal position (0.5 the centre). On an existing pad it moves as the hand does (an input of its own, automatable as {modulator_id, param: \"x\"})."},
        "y": {"type": "number", "minimum": 0, "maximum": 1, "description": "X-Y pad: the puck's vertical position."},
        "inertia_ms": {"type": "number", "minimum": 0, "maximum": MAX_INERTIA_MS, "description": "X-Y pad: how long the puck takes to follow (0: at once)."},
        "path": {"type": ["object", "null"], "additionalProperties": false, "required": ["points", "beats"],
            "description": "X-Y pad: a looping gesture — points evenly spaced in time over `beats`, the last joined to the first, read from the song position while the song plays and play_path is on; null removes it.",
            "properties": {
                "points": {"type": "array", "minItems": 2, "maxItems": MAX_PATH, "items": {"type": "array", "minItems": 2, "maxItems": 2, "items": {"type": "number", "minimum": 0, "maximum": 1}}, "description": "[x, y] pairs, 0-1 each."},
                "beats": {"type": "number", "minimum": MIN_PATH_BEATS, "maximum": MAX_PATH_BEATS},
                "smooth": {"type": "boolean", "default": false, "description": "A smooth curve through the points instead of straight lines."}
            }},
        "play_path": {"type": "boolean", "description": "X-Y pad: the path drives the puck while the song plays."},
        "formula": {"type": "string", "minLength": 1, "maxLength": MAX_TEXT, "description": "Formula controller: FL's Formula Controller syntax, output 0-1 — the knobs a, b, c; SongTime (quarters), Pi, GetTempo, Rand; + - * / ^ % !, comparisons, div mod, and or not; Sin, Cos, Tan, Sqrt, Abs, Min, Max, Round, Frac, If, Inter… and Cantus's sine/saw/tri/square/pulse(x, width) cycle waves and clamp. A formula that does not compile is refused, naming the character where it fails. Example: a*Sin(SongTime*Pi*b)+c."},
        "a": {"type": "number", "minimum": 0, "maximum": 1, "description": "Formula controller: knob a. On an existing formula it moves as the hand does (an input of its own, automatable as {modulator_id, param: \"a\"})."},
        "b": {"type": "number", "minimum": 0, "maximum": 1, "description": "Formula controller: knob b."},
        "c": {"type": "number", "minimum": 0, "maximum": 1, "description": "Formula controller: knob c."},
        "bipolar": {"type": "boolean", "description": "Formula controller: −1…1 (twice the value less one) instead of FL's 0…1."}
    });
    let routes = json!({
        "add_routes": {"type": "array", "maxItems": 32, "description": "Routes added to the ones it has (a target it already drives is refused). An X-Y pad's route reads its axis (x unless said); a pad's or a formula's route may have its own range.",
            "items": route_schema()},
        "change_routes": {"type": "array", "maxItems": 32, "description": "Routes it already has, changed by target: the depth and, for an X-Y pad or a formula, the axis and the range (range null: the output as it is).",
            "items": {"type": "object", "additionalProperties": false, "required": ["target"],
                "properties": {
                    "target": target_schema(),
                    "depth": {"type": "number", "minimum": -1, "maximum": 1},
                    "axis": {"enum": ["x", "y"], "description": "X-Y pad: the axis the route reads."},
                    "range": range_schema(true)
                }}},
        "routes": {"type": "array", "maxItems": 32, "description": "What it drives, for a modulator with no routes yet (set_modulator refuses to replace routes it has: add_routes adds, change_routes changes, remove_items' modulator_routes removes): depth is a share of the control's whole travel, −1…1 (0.25 swings a quarter of the knob each way); an X-Y pad's route reads an axis, a pad's or a formula's may have its own range.",
            "items": route_schema()}
    });
    for more in [controllers, routes] {
        if let (Some(all), Value::Object(more)) = (props.as_object_mut(), more) {
            all.extend(more);
        }
    }
    if adding {
        props["host"] = json!({"type": ["integer", "string"], "description": "Where it lives: a track's id (a number), \"strip:<id>\" for a mixer strip, or \"master\"."});
        props["kind"] = json!({"enum": KINDS, "description": "xy_pad: an X-Y pad (two axes, each driving its own routes; inertia, looping paths). formula: a formula controller (FL's syntax, three knobs a, b, c)."});
        args::object(props, &["host", "kind"])
    } else {
        props["modulator_id"] =
            s_id("The modulator (get_track lists a track's; get_mixer's strips have theirs).");
        args::object(props, &["modulator_id"])
    }
}

fn host_of(p: &Project, v: &Value) -> Res<ModHost> {
    if let Some(t) = v.as_u64() {
        let id = TrackId(t);
        p.track(id)
            .ok_or_else(|| ToolError::NotFound(format!("track {t}")))?;
        return Ok(ModHost::Track(id));
    }
    match v.as_str() {
        Some("master") => Ok(ModHost::Master),
        Some(s) if s.starts_with("strip:") => {
            let id = StripId(
                s[6..]
                    .parse()
                    .map_err(|_| ToolError::bad("host", "\"strip:<id>\""))?,
            );
            p.strip(id)
                .ok_or_else(|| ToolError::NotFound(format!("strip {}", id.0)))?;
            Ok(ModHost::Strip(id))
        }
        _ => Err(ToolError::bad(
            "host",
            "a track id, \"strip:<id>\" or \"master\"",
        )),
    }
}

/// The settings only an X-Y pad has, and only a formula controller.
const PAD_KEYS: [&str; 5] = ["x", "y", "inertia_ms", "path", "play_path"];
const FORMULA_KEYS: [&str; 5] = ["formula", "a", "b", "c", "bipolar"];

/// Apply the settings in `a` to `m`. A controller's inputs (the pad's x and y, the
/// formula's a, b, c) go into its source when `adding`; otherwise they are answered, to be
/// moved by their own command (`Command::SetModulatorInput`, as the hand moves them).
fn configure(p: &Project, m: &mut Modulator, a: A<'_>, adding: bool) -> Res<Vec<(u8, f32)>> {
    let kind = m.source.kind();
    for (keys, whose, of) in [
        (&PAD_KEYS, "an X-Y pad's", ModKind::XyPad),
        (&FORMULA_KEYS, "a formula controller's", ModKind::Formula),
    ] {
        if let Some(k) = keys.iter().find(|k| a.0.get(**k).is_some()) {
            if kind != of {
                return Err(ToolError::bad(
                    *k,
                    format!("is {whose} setting; this modulator is {}", kind_id(kind)),
                ));
            }
        }
    }
    let mut inputs = Vec::new();
    for (k, name) in input_names(Some(kind)).iter().enumerate() {
        if let Some(v) = a.opt_in(name, 0.0, 1.0)? {
            inputs.push((k as u8, v as f32));
        }
    }
    if adding {
        for (k, v) in inputs.drain(..) {
            m.source.set_input(k, v);
        }
    }
    if let Some(n) = a.opt_str("name")? {
        m.name = n.chars().take(40).collect();
    }
    if let Some(x) = a.opt_in("amount", 0.0, 1.0)? {
        m.amount = x as f32;
    }
    if let Some(x) = a.opt_bool("muted")? {
        m.muted = x;
    }
    let uni = a.opt_bool("unipolar")?;
    match &mut m.source {
        ModSource::Lfo(l) => {
            if let Some(v) = a.get("rate") {
                l.rate = rate_of(v, "rate")?;
            }
            if let Some(s) = a.opt_choice(
                "shape",
                &["sine", "triangle", "square", "saw_up", "saw_down"],
            )? {
                l.shape = match s {
                    "triangle" => LfoShape::Triangle,
                    "square" => LfoShape::Square,
                    "saw_up" => LfoShape::SawUp,
                    "saw_down" => LfoShape::SawDown,
                    _ => LfoShape::Sine,
                };
            }
            if let Some(x) = a.opt_in("phase", 0.0, 1.0)? {
                l.phase = x as f32;
            }
            if let Some(r) = a.opt_bool("retrigger_on_notes")? {
                l.retrigger = if r { Retrigger::Note } else { Retrigger::Song };
            }
            if let Some(u) = uni {
                l.unipolar = u;
            }
        }
        ModSource::Envelope(e) => {
            for (k, f) in [
                ("attack_ms", &mut e.attack_ms),
                ("decay_ms", &mut e.decay_ms),
                ("release_ms", &mut e.release_ms),
            ] {
                if let Some(x) = a.opt_in(k, 0.0, 10_000.0)? {
                    *f = x as f32;
                }
            }
            if let Some(x) = a.opt_in("sustain", 0.0, 1.0)? {
                e.sustain = x as f32;
            }
            let _ = &e.trigger as &EnvTrigger;
        }
        ModSource::Follower(f) => {
            for (k, v) in [
                ("attack_ms", &mut f.attack_ms),
                ("release_ms", &mut f.release_ms),
            ] {
                if let Some(x) = a.opt_in(k, 0.0, 10_000.0)? {
                    *v = x as f32;
                }
            }
            if let Some(x) = a.opt_in("gain_db", -48.0, 48.0)? {
                f.gain_db = x as f32;
            }
            if let Some(v) = a.get("listen_strip_id") {
                let s = args::strip_of(p, v, "listen_strip_id")?;
                f.listen = if s.is_master() {
                    Listen::Master
                } else {
                    Listen::Strip(s)
                };
            }
        }
        ModSource::Step(s) => {
            if let Some(v) = a.opt_array("steps")? {
                let steps: Vec<f32> = v
                    .iter()
                    .map(|x| {
                        x.as_f64()
                            .filter(|x| (-1.0..=1.0).contains(x))
                            .map(|x| x as f32)
                            .ok_or_else(|| ToolError::bad("steps", "values are -1 to 1"))
                    })
                    .collect::<Res<_>>()?;
                if steps.is_empty() || steps.len() > MAX_STEPS {
                    return Err(ToolError::bad("steps", format!("1 to {MAX_STEPS} steps")));
                }
                s.steps = steps;
            }
            if let Some(r) = a.opt_choice("step_rate", &DIVISIONS)? {
                s.division = DIVISIONS.iter().position(|d| *d == r).expect("listed") as u8;
            }
            if let Some(x) = a.opt_in("glide", 0.0, 1.0)? {
                s.glide = x as f32;
            }
            if let Some(u) = uni {
                s.unipolar = u;
            }
        }
        ModSource::Random(r) => {
            if let Some(v) = a.get("rate") {
                r.rate = rate_of(v, "rate")?;
            }
            if let Some(x) = a.opt_bool("smooth")? {
                r.smooth = x;
            }
            if let Some(x) = a.opt_in("seed", 1.0, 999.0)? {
                r.seed = x as u32;
            }
            if let Some(u) = uni {
                r.unipolar = u;
            }
        }
        // fxd: the controllers (ADRs 0147, 0148); their inputs are above.
        ModSource::XyPad(x) => {
            if let Some(u) = uni {
                x.unipolar = u;
            }
            if let Some(v) = a.opt_in("inertia_ms", 0.0, f64::from(MAX_INERTIA_MS))? {
                x.inertia_ms = v as f32;
            }
            match a.0.get("path") {
                None => {}
                Some(Value::Null) => {
                    x.path = None;
                    x.play_path = false;
                }
                Some(v) => x.path = Some(path_of(v)?),
            }
            if let Some(b) = a.opt_bool("play_path")? {
                x.play_path = b;
            }
            if x.play_path && x.path.is_none() {
                return Err(ToolError::bad(
                    "play_path",
                    "the pad has no path to play: give path",
                ));
            }
        }
        ModSource::Formula(f) => {
            if let Some(text) = a.opt_str("formula")? {
                if let Err(e) = Program::compile(text) {
                    return Err(ToolError::bad("formula", formula_error(text, &e)));
                }
                f.text = text.to_string();
            }
            match (a.opt_bool("bipolar")?, uni) {
                (Some(b), Some(u)) if b == u => {
                    return Err(ToolError::bad(
                        "bipolar",
                        "bipolar and unipolar say opposite things: give one",
                    ))
                }
                (Some(b), _) => f.bipolar = b,
                (None, Some(u)) => f.bipolar = !u,
                (None, None) => {}
            }
        }
    }
    if a.has("add_routes") && a.has("routes") {
        return Err(ToolError::bad(
            "add_routes",
            "give routes or add_routes, not both",
        ));
    }
    if let Some(more) = a.opt_array("add_routes")? {
        for (i, r) in more.iter().enumerate() {
            let ra = A(r);
            let at = format!("add_routes[{i}]");
            let target = target_of(p, ra.req("target")?, &format!("{at}.target"))?;
            if m.routes.iter().any(|x| x.target == target) {
                return Err(ToolError::bad(at, "it already drives that"));
            }
            let depth = ra.f64_in("depth", -1.0, 1.0)? as f32;
            let (output, range) = link_of(kind, ra, &at)?;
            set_link(m, &target, output.unwrap_or(0), range.flatten());
            m.routes.push(ModRoute { target, depth });
        }
    }
    if let Some(routes) = a.opt_array("routes")? {
        let mut out = Vec::with_capacity(routes.len());
        if let Some(l) = m.source.links_mut() {
            l.clear();
        }
        for (i, r) in routes.iter().enumerate() {
            let ra = A(r);
            let at = format!("routes[{i}]");
            let target = target_of(p, ra.req("target")?, &format!("{at}.target"))?;
            let depth = ra.f64_in("depth", -1.0, 1.0)? as f32;
            let (output, range) = link_of(kind, ra, &at)?;
            set_link(m, &target, output.unwrap_or(0), range.flatten());
            out.push(ModRoute { target, depth });
        }
        m.routes = out;
    }
    if let Some(list) = a.opt_array("change_routes")? {
        let mut seen: Vec<AutoTarget> = Vec::with_capacity(list.len());
        for (i, r) in list.iter().enumerate() {
            let ra = A(r);
            let at = format!("change_routes[{i}]");
            let target = target_of(p, ra.req("target")?, &format!("{at}.target"))?;
            if seen.contains(&target) {
                return Err(ToolError::bad(at, "that route is named twice"));
            }
            let Some(route) = m.routes.iter_mut().find(|x| x.target == target) else {
                return Err(ToolError::bad(
                    at,
                    "it drives no such target: add_routes adds one",
                ));
            };
            if let Some(d) = ra.opt_in("depth", -1.0, 1.0)? {
                route.depth = d as f32;
            }
            let (output, range) = link_of(kind, ra, &at)?;
            let (o0, r0) = m.source.link(&target);
            set_link(m, &target, output.unwrap_or(o0), range.unwrap_or(r0));
            seen.push(target);
        }
    }
    tidy_links(m);
    if !m.is_valid() {
        return Err(ToolError::bad(
            "routes",
            "the modulator is not valid: a target twice, its own amount or input, or a value out of range",
        ));
    }
    Ok(inputs)
}

/// A route's axis and range as given (`None`: not given; a range of `Some(None)`: given
/// as null, the output as it is), refused on a modulator that keeps neither.
#[allow(clippy::type_complexity)]
fn link_of(kind: ModKind, ra: A<'_>, at: &str) -> Res<(Option<u8>, Option<Option<RouteRange>>)> {
    let axis = ra.opt_choice("axis", &["x", "y"])?;
    let range = match ra.0.get("range") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => {
            let r = A(v);
            let part = |f: &str, need: bool| -> Res<Option<f32>> {
                let x = r.opt_in(f, -1.0, 1.0).map_err(|_| {
                    ToolError::bad(format!("{at}.range.{f}"), "must be a number, -1 to 1")
                })?;
                if need && x.is_none() {
                    return Err(ToolError::bad(format!("{at}.range.{f}"), "is required"));
                }
                Ok(x.map(|x| x as f32))
            };
            Some(Some(RouteRange {
                from: part("from", true)?.unwrap_or(0.0),
                to: part("to", true)?.unwrap_or(0.0),
                curve: part("curve", false)?.unwrap_or(0.0),
            }))
        }
    };
    if axis.is_none() && range.is_none() {
        return Ok((None, None));
    }
    if !ModKind::CONTROLLERS.contains(&kind) {
        return Err(ToolError::bad(
            at,
            format!(
                "only an X-Y pad's or a formula controller's routes have an axis or a range; this modulator is {}",
                kind_id(kind)
            ),
        ));
    }
    let output = match axis {
        Some("y") if kind.outputs() < 2 => {
            return Err(ToolError::bad(
                format!("{at}.axis"),
                "a formula controller has one output: leave axis out",
            ))
        }
        Some("y") => Some(1),
        Some(_) => Some(0),
        None => None,
    };
    Ok((output, range))
}

/// Keep `m`'s link for `target`: which output its route reads and its range — none kept
/// when it reads output 0 as it is (what a route without a link does).
fn set_link(m: &mut Modulator, target: &AutoTarget, output: u8, range: Option<RouteRange>) {
    if let Some(links) = m.source.links_mut() {
        links.retain(|l| &l.target != target);
        if output != 0 || range.is_some() {
            links.push(RouteLink {
                target: target.clone(),
                output,
                range,
            });
        }
    }
}

/// A controller's links kept consistent with its routes: one per route that needs one,
/// none left by a removed route (`cantus-core` ignores such a link; it is not kept).
pub fn tidy_links(m: &mut Modulator) {
    let routes: Vec<AutoTarget> = m.routes.iter().map(|r| r.target.clone()).collect();
    if let Some(links) = m.source.links_mut() {
        links.retain(|l| routes.contains(&l.target) && (l.output != 0 || l.range.is_some()));
    }
}

/// A pad's path from its JSON.
fn path_of(v: &Value) -> Res<XyPath> {
    let a = A(v);
    let items = a
        .array("points")
        .map_err(|_| ToolError::bad("path.points", "is required: [x, y] pairs"))?;
    if !(2..=MAX_PATH).contains(&items.len()) {
        return Err(ToolError::bad(
            "path.points",
            format!("give 2 to {MAX_PATH} points"),
        ));
    }
    let points = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let xy: Vec<f64> = it
                .as_array()
                .map(|pair| pair.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            match xy.as_slice() {
                [x, y] if (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y) => {
                    Ok([*x as f32, *y as f32])
                }
                _ => Err(ToolError::bad(
                    format!("path.points[{i}]"),
                    "an [x, y] pair, 0 to 1 each",
                )),
            }
        })
        .collect::<Res<Vec<_>>>()?;
    let beats = a
        .f64_in(
            "beats",
            f64::from(MIN_PATH_BEATS),
            f64::from(MAX_PATH_BEATS),
        )
        .map_err(|_| {
            ToolError::bad(
                "path.beats",
                format!("is required, {MIN_PATH_BEATS} to {MAX_PATH_BEATS} beats"),
            )
        })? as f32;
    let smooth = a.opt_bool("smooth")?.unwrap_or(false);
    Ok(XyPath {
        points,
        beats,
        smooth,
    })
}

/// Why a formula does not compile, naming the character where (counted from 1) — what
/// the editor shows under its display (`fxd.formula.err.*`), in English for the agent.
pub fn formula_error(text: &str, e: &FormulaError) -> String {
    let at = e.at.min(text.len());
    let n = text.char_indices().take_while(|(i, _)| *i < at).count() + 1;
    let here = text[at..]
        .chars()
        .next()
        .map_or_else(|| "the end".to_string(), |c| format!("{c:?}"));
    let why = match &e.kind {
        ErrorKind::Empty => "the formula is empty".to_string(),
        ErrorKind::TooLong => format!("the formula is longer than {MAX_TEXT} characters"),
        ErrorKind::Unexpected(c) => format!("{c:?} has no meaning at character {n}"),
        ErrorKind::UnexpectedEnd => {
            format!("the formula ends at character {n}, where a value is needed")
        }
        ErrorKind::Trailing => {
            format!("something follows a complete formula at character {n} ({here})")
        }
        ErrorKind::Unclosed => format!(
            "the parenthesis at character {n} ({here}) is never closed, or closes nothing"
        ),
        ErrorKind::UnknownName(w) => {
            format!("no variable or function is called {w:?} (character {n})")
        }
        ErrorKind::NotDeterministic(w) => format!(
            "{w:?} (character {n}) depends on the moment, so a render would never repeat"
        ),
        ErrorKind::NotSupported(w) => format!(
            "{w:?} (character {n}) is not supported: its published definition does not say what it computes"
        ),
        ErrorKind::Arguments { name, wants } => format!(
            "{name:?} (character {n}) takes {wants} value{} in parentheses",
            if *wants == 1 { "" } else { "s" }
        ),
        ErrorKind::BadNumber => format!("the number at character {n} is not written correctly"),
        ErrorKind::TooComplex => format!(
            "the formula is too complex (at character {n}): shorten it or nest it less"
        ),
    };
    format!("does not compile: {why}")
}

pub fn add_modulator(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let host = host_of(p, a.req("host")?)?;
    let kind = match a.opt_choice("kind", &KINDS)?.unwrap_or("lfo") {
        "envelope" => ModKind::Envelope,
        "follower" => ModKind::Follower,
        "step" => ModKind::Step,
        "random" => ModKind::Random,
        "xy_pad" => ModKind::XyPad,
        "formula" => ModKind::Formula,
        _ => ModKind::Lfo,
    };
    let count = p.modulators(host).map_or(0, |m| m.len());
    if count >= MAX_PER_HOST {
        return Err(ToolError::Refused(format!(
            "the host already has {MAX_PER_HOST} modulators"
        )));
    }
    let id = ModulatorId(cx.bus_mut().alloc_id());
    let mut m = Modulator::new(id, kind);
    configure(cx.project(), &mut m, a, true)?;
    let said = Said::new("ai.tools.add_modulator")
        .nested("kind", words::mod_kind(kind, true))
        .nested("host", words::mod_host(cx.project(), host))
        .nested("routes", words::count(m.routes.len(), "parameters"));
    Ok(plan(
        Command::InsertModulator {
            host,
            index: count,
            modulator: Box::new(m),
        },
        said,
        json!({"modulator_id": id.0}),
    ))
}

pub fn set_modulator(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let id = ModulatorId(a.u64("modulator_id")?);
    let which = words::modulator(p, id);
    let has = p
        .all_modulators()
        .find(|(_, m)| m.id == id)
        .map_or(0, |(_, m)| m.routes.len());
    if has > 0 && A(a.0).has("routes") {
        return Err(ToolError::Refused(format!(
            "it already drives {has} parameter(s): add_routes adds to them; remove_items' modulator_routes removes one (it asks)"
        )));
    }
    let mut m = p
        .all_modulators()
        .find(|(_, m)| m.id == id)
        .map(|(_, m)| m.clone())
        .ok_or_else(|| ToolError::NotFound(format!("modulator {}", id.0)))?;
    let (host, _, was) = p.modulator(id).expect("found");
    let inputs = configure(p, &mut m, a, false)?;
    // fxd: a controller's inputs move by their own command, as the hand moves them (its
    // undo, its recording, its merging are the input's); the rest is the modulator's.
    let mut cmds = Vec::with_capacity(1 + inputs.len());
    if inputs.is_empty() || m != *was {
        cmds.push(Command::SetModulator {
            modulator: Box::new(m),
        });
    }
    let only_inputs = cmds.is_empty();
    for (input, value) in &inputs {
        cmds.push(Command::SetModulatorInput {
            id,
            input: *input,
            value: *value,
        });
    }
    let said = if only_inputs {
        Said::new("ai.tools.set_controller_inputs")
            .nested(
                "inputs",
                words::list(
                    inputs
                        .iter()
                        .map(|(k, v)| words::mod_input_to(p, id, *k, f64::from(*v)))
                        .collect(),
                ),
            )
            .nested("modulator", which)
            .nested("host", words::mod_host_of(p, host))
    } else {
        Said::new("ai.tools.set_modulator")
            .nested("modulator", which)
            .nested("host", words::mod_host_of(p, host))
    };
    Ok(plan(
        args::one(cmds, "cmd.ai.set_modulator")?,
        said,
        json!({"modulator_id": id.0}),
    ))
}

/// Unused targets are still targets: keep the type names in scope for docs.
pub fn _effect_id(e: EffectId) -> u64 {
    e.0
}

/// Bars as a sentence's argument (`bar 5`, `bars 5–8`, worded by the host).
pub fn bars_said(map: &cantus_core::tempo::TempoMap, from: Ticks, to: Ticks) -> Said {
    let (a, b) = words::bar_numbers(map, from, to);
    Said::new("ai.phrase.bars_only").bars("bars", a, b)
}
