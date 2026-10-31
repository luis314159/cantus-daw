//! Movement: automation lanes (points in the parameter's own units, FL's shapes) and
//! modulators (LFO, envelope, follower, steps, random) with their routes.

use super::args::{self, plan, s_id, s_len, s_pos, Res, A, MAX_POINTS};
use super::json::firmus_key;
use super::notes::origin;
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::automation::range::range_of;
use cantus_core::automation::{edit as auto_edit, AutoPoint, AutoTarget, Automation, Shape};
use cantus_core::modulator::{
    EnvTrigger, LfoShape, Listen, ModRate, ModRoute, ModSource, Retrigger, MAX_PER_HOST, MAX_STEPS,
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
        "description": "What moves. One of: {track_id, param: \"gain_db\"|\"pan\"} a track's own level or pan; {track_id, param: <instrument key>} its instrument (get_device/get_unit list keys; Pulsus pads cannot be automated); {strip_id, param: \"gain_db\"|\"pan\"|\"width\"} a strip's (strip_id \"master\" for the master); {strip_id, param: \"send\", to: <strip id>} a send's level; {effect_id, param: <key>} an effect's parameter; {effect_id or track_id, plugin_param: <id>} a hosted plug-in's (0-1 normalised); {modulator_id} a modulator's amount.",
        "properties": {
            "track_id": {"type": "integer", "minimum": 1},
            "strip_id": {"type": ["integer", "string"]},
            "effect_id": {"type": "integer", "minimum": 1},
            "modulator_id": {"type": "integer", "minimum": 1},
            "param": {"type": "string"},
            "to": {"type": "integer", "minimum": 1},
            "plugin_param": {"type": "integer", "minimum": 0}
        }
    })
}

/// A target from its JSON, checked to exist in `p`.
pub fn target_of(p: &Project, v: &Value, k: &str) -> Res<AutoTarget> {
    let a = A(v);
    let param = a.opt_str("param")?;
    let bad = |why: &str| ToolError::bad(k, why.to_string());
    let t = if let Some(m) = a.opt_u64("modulator_id")? {
        AutoTarget::ModAmount(ModulatorId(m))
    } else if a.has("effect_id") {
        let (_, e) = args::effect(p, a, "effect_id")?;
        match (a.opt_u64("plugin_param")?, param) {
            (Some(id), _) => AutoTarget::Plugin {
                effect: e,
                id: id as u32,
            },
            (None, Some(key)) => {
                let host = p.effect_host(e).expect("found");
                let kind = p.effect(host, e).expect("found").spec.kind;
                if kind.param(key).is_none() {
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

pub fn modulator_schema(adding: bool) -> Value {
    let mut props = json!({
        "name": {"type": "string", "maxLength": 40},
        "amount": {"type": "number", "minimum": 0, "maximum": 1, "description": "Scales every route at once."},
        "muted": {"type": "boolean"},
        "rate": {"type": ["string", "number"], "description": format!("LFO and random: a synced division ({}) or Hz (0.01-40).", DIVISIONS.join(", "))},
        "shape": {"enum": ["sine", "triangle", "square", "saw_up", "saw_down"], "description": "LFO wave."},
        "phase": {"type": "number", "minimum": 0, "maximum": 1},
        "retrigger_on_notes": {"type": "boolean", "description": "LFO: restart at every note of the host track (else it follows the song position)."},
        "unipolar": {"type": "boolean", "description": "0…1 instead of −1…1 (LFO, steps, random)."},
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
        "seed": {"type": "integer", "minimum": 1, "maximum": 999},
        "add_routes": {"type": "array", "maxItems": 32, "description": "Routes added to the ones it has (a target it already drives is refused).",
            "items": {"type": "object", "additionalProperties": false, "required": ["target", "depth"],
                "properties": {"target": target_schema(), "depth": {"type": "number", "minimum": -1, "maximum": 1}}}},
        "routes": {"type": "array", "maxItems": 32, "description": "What it drives, for a modulator with no routes yet (set_modulator refuses to replace routes it has: add_routes adds, remove_items' modulator_routes removes): depth is a share of the control's whole travel, −1…1 (0.25 swings a quarter of the knob each way).",
            "items": {"type": "object", "additionalProperties": false, "required": ["target", "depth"],
                "properties": {"target": target_schema(), "depth": {"type": "number", "minimum": -1, "maximum": 1}}}}
    });
    if adding {
        props["host"] = json!({"type": ["integer", "string"], "description": "Where it lives: a track's id (a number), \"strip:<id>\" for a mixer strip, or \"master\"."});
        props["kind"] = json!({"enum": ["lfo", "envelope", "follower", "step", "random"]});
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

/// Apply the settings in `a` to `m`.
fn configure(p: &Project, m: &mut Modulator, a: A<'_>) -> Res<()> {
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
            let target = target_of(p, ra.req("target")?, &format!("add_routes[{i}].target"))?;
            if m.routes.iter().any(|x| x.target == target) {
                return Err(ToolError::bad(
                    format!("add_routes[{i}]"),
                    "it already drives that",
                ));
            }
            let depth = ra.f64_in("depth", -1.0, 1.0)? as f32;
            m.routes.push(ModRoute { target, depth });
        }
    }
    if let Some(routes) = a.opt_array("routes")? {
        let mut out = Vec::with_capacity(routes.len());
        for (i, r) in routes.iter().enumerate() {
            let ra = A(r);
            let target = target_of(p, ra.req("target")?, &format!("routes[{i}].target"))?;
            let depth = ra.f64_in("depth", -1.0, 1.0)? as f32;
            out.push(ModRoute { target, depth });
        }
        m.routes = out;
    }
    if !m.is_valid() {
        return Err(ToolError::bad(
            "routes",
            "the modulator is not valid: a target twice, its own amount, or a value out of range",
        ));
    }
    Ok(())
}

pub fn add_modulator(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let host = host_of(p, a.req("host")?)?;
    let kind = match a
        .opt_choice("kind", &["lfo", "envelope", "follower", "step", "random"])?
        .unwrap_or("lfo")
    {
        "envelope" => ModKind::Envelope,
        "follower" => ModKind::Follower,
        "step" => ModKind::Step,
        "random" => ModKind::Random,
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
    configure(cx.project(), &mut m, a)?;
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
    configure(p, &mut m, a)?;
    let host = p
        .all_modulators()
        .find(|(_, x)| x.id == id)
        .map(|(h, _)| h)
        .expect("found");
    let said = Said::new("ai.tools.set_modulator")
        .nested("modulator", which)
        .nested("host", words::mod_host_of(p, host));
    Ok(plan(
        Command::SetModulator {
            modulator: Box::new(m),
        },
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
