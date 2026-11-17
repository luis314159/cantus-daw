//! The mixer: strips (and the master), routing, sends; effects on any strip — add, set,
//! bypass, move, replace — with their parameters by key in their units.

use super::args::{self, plan, s_id, Res, A};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::model::{EqField, MAX_INSERTS, MAX_WIDTH};
use cantus_core::{Command, EffectKind, EffectSlot, EffectSpec, Project, Send, StripId};
use serde_json::{json, Map, Value};

pub fn add_strip_schema() -> Value {
    args::object(
        json!({
            "name": {"type": "string", "minLength": 1, "maxLength": 80, "description": "The strip's name (\"Drum bus\", \"Reverb\")."},
            "output": args::s_strip("Where it sends its sound (left out: the master)"),
            "tracks": args::s_ids("Route these tracks into the new strip (a bus): each leaves the strip it had.")
        }),
        &["name"],
    )
}

pub fn add_strip(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let name = a
        .opt_name("name", 80)?
        .ok_or_else(|| ToolError::bad("name", "is required"))?;
    let output = a
        .get("output")
        .map(|v| args::strip_of(p, v, "output"))
        .transpose()?;
    let tracks: Vec<cantus_core::TrackId> = match a.opt_array("tracks")? {
        None => Vec::new(),
        Some(v) => v
            .iter()
            .map(|x| {
                let id = cantus_core::TrackId(
                    x.as_u64()
                        .ok_or_else(|| ToolError::bad("tracks", "track ids"))?,
                );
                p.track(id)
                    .map(|_| id)
                    .ok_or_else(|| ToolError::NotFound(format!("track {}", id.0)))
            })
            .collect::<Res<_>>()?,
    };
    let index = p.strips.len();
    let mut s = cx.bus_mut().new_strip(name.clone());
    if let Some(o) = output.filter(|o| !o.is_master()) {
        s.output = Some(o);
    }
    let id = s.id;
    let mut cmds = vec![Command::InsertStrip {
        index,
        strip: Box::new(s),
    }];
    for t in &tracks {
        cmds.push(Command::SetTrackStrip {
            id: *t,
            strip: Some(id),
        });
    }
    let said = Said::new("ai.tools.add_strip").name("name", args::clean(&name));
    Ok(plan(
        args::one(cmds, "cmd.ai.add_strip")?,
        said,
        json!({"strip_id": id.0, "tracks": tracks.iter().map(|t| t.0).collect::<Vec<_>>()}),
    ))
}

pub fn set_strip_schema() -> Value {
    args::object(
        json!({
            "strip_id": args::s_strip("The strip"),
            "name": {"type": "string", "minLength": 1, "maxLength": 80},
            "gain_db": {"type": "number", "minimum": -96, "maximum": 12, "description": "The fader in dB, -96 to +12."},
            "pan": {"type": "number", "minimum": -1, "maximum": 1},
            "width": {"type": "number", "minimum": 0, "maximum": MAX_WIDTH, "description": "Stereo width: 0 mono, 1 as is, 2 twice as wide."},
            "muted": {"type": "boolean"},
            "soloed": {"type": "boolean"},
            "phase_invert": {"type": "boolean"},
            "fx_enabled": {"type": "boolean", "description": "Switch the whole insert chain on or off."},
            "output": args::s_strip("Route the strip's output (not for the master)"),
            "eq": {"type": "array", "maxItems": 3, "description": "The strip's own three-band EQ (0 low shelf, 1 bell, 2 high shelf).",
                "items": {"type": "object", "additionalProperties": false, "required": ["band"], "properties": {
                    "band": {"type": "integer", "minimum": 0, "maximum": 2},
                    "freq_hz": {"type": "number", "minimum": 20, "maximum": 20000},
                    "gain_db": {"type": "number", "minimum": -18, "maximum": 18},
                    "q": {"type": "number", "minimum": 0.1, "maximum": 10}
                }}}
        }),
        &["strip_id"],
    )
}

pub fn set_strip(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let id = args::strip(p, a, "strip_id")?;
    let master = id.is_master();
    let mut cmds = Vec::new();
    let mut done = Map::new();
    let only_strip = |k: &str| -> Res<()> {
        if master {
            Err(ToolError::bad(k, "the master has no such setting"))
        } else {
            Ok(())
        }
    };
    if let Some(name) = a.opt_name("name", 80)? {
        only_strip("name")?;
        done.insert("name".into(), json!(name));
        cmds.push(Command::SetStripName { id, name });
    }
    if let Some(g) = a.opt_in("gain_db", -96.0, 12.0)? {
        let gain_db = g as f32;
        cmds.push(if master {
            Command::SetMasterGain { gain_db }
        } else {
            Command::SetStripGain { id, gain_db }
        });
        done.insert("gain_db".into(), json!(g));
    }
    if let Some(v) = a.opt_in("pan", -1.0, 1.0)? {
        let pan = v as f32;
        cmds.push(if master {
            Command::SetMasterPan { pan }
        } else {
            Command::SetStripPan { id, pan }
        });
        done.insert("pan".into(), json!(v));
    }
    if let Some(v) = a.opt_in("width", 0.0, MAX_WIDTH as f64)? {
        let width = v as f32;
        cmds.push(if master {
            Command::SetMasterWidth { width }
        } else {
            Command::SetStripWidth { id, width }
        });
        done.insert("width".into(), json!(v));
    }
    if let Some(m) = a.opt_bool("muted")? {
        only_strip("muted")?;
        cmds.push(Command::SetStripMute { id, muted: m });
        done.insert("muted".into(), json!(m));
    }
    if let Some(s) = a.opt_bool("soloed")? {
        only_strip("soloed")?;
        cmds.push(Command::SetStripSolo { id, soloed: s });
        done.insert("soloed".into(), json!(s));
    }
    if let Some(i) = a.opt_bool("phase_invert")? {
        cmds.push(if master {
            Command::SetMasterPhaseInvert { invert: i }
        } else {
            Command::SetStripPhaseInvert { id, invert: i }
        });
        done.insert("phase_invert".into(), json!(i));
    }
    if let Some(e) = a.opt_bool("fx_enabled")? {
        cmds.push(if master {
            Command::SetMasterFxEnabled { enabled: e }
        } else {
            Command::SetStripFxEnabled { id, enabled: e }
        });
        done.insert("fx_enabled".into(), json!(e));
    }
    if let Some(v) = a.get("output") {
        only_strip("output")?;
        let o = args::strip_of(p, v, "output")?;
        if o == id {
            return Err(ToolError::bad("output", "a strip cannot feed itself"));
        }
        let output = (!o.is_master()).then_some(o);
        if p.routing_would_cycle(id, output) {
            return Err(ToolError::bad("output", "that route would make a loop"));
        }
        cmds.push(Command::SetStripOutput { id, output });
        done.insert("output".into(), args::strip_json(o));
    }
    if let Some(bands) = a.opt_array("eq")? {
        only_strip("eq")?;
        for (i, b) in bands.iter().enumerate() {
            let ba = A(b);
            let band = ba.f64_in("band", 0.0, 2.0)? as u8;
            let k = |f: &str| format!("eq[{i}].{f}");
            for (field, key, lo, hi) in [
                (EqField::Freq, "freq_hz", 20.0, 20_000.0),
                (EqField::Gain, "gain_db", -18.0, 18.0),
                (EqField::Q, "q", 0.1, 10.0),
            ] {
                if let Some(v) = ba.opt_f64(key)? {
                    if !(lo..=hi).contains(&v) {
                        return Err(ToolError::bad(k(key), format!("must be {lo} to {hi}")));
                    }
                    cmds.push(Command::SetStripEq {
                        id,
                        band,
                        field,
                        value: v as f32,
                    });
                }
            }
        }
        done.insert("eq".into(), json!(bands));
    }
    let said = Said::new(if master {
        "ai.tools.set_master"
    } else {
        "ai.tools.set_strip"
    })
    .nested("strip", words::strip(p, id))
    .nested("what", words::fields(done.keys()));
    done.insert("strip_id".into(), args::strip_json(id));
    Ok(plan(
        args::one(cmds, "cmd.ai.set_strip")?,
        said,
        Value::Object(done),
    ))
}

pub fn set_send_schema() -> Value {
    args::object(
        json!({
            "strip_id": s_id("The strip that sends."),
            "to": s_id("The strip it sends to (a reverb or delay return, a bus)."),
            "level_db": {"type": "number", "minimum": -96, "maximum": 0, "description": "The send's level in dB, -96 to 0 (0 sends the whole signal). A new send starts at 0 dB."},
            "enabled": {"type": "boolean"},
            "pre_fader": {"type": "boolean", "description": "Take the copy before the fader (a cue mix) instead of after it (the default)."}
        }),
        &["strip_id", "to"],
    )
}

pub fn db_to_level(db: f64) -> f32 {
    if db <= -96.0 {
        0.0
    } else {
        10f64.powf(db / 20.0) as f32
    }
}

pub fn set_send(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let from = args::strip(p, a, "strip_id")?;
    let to = args::strip(p, a, "to")?;
    if from.is_master() || to.is_master() {
        return Err(ToolError::bad(
            "to",
            "sends run between strips, not to or from the master",
        ));
    }
    if from == to {
        return Err(ToolError::bad("to", "a strip cannot send to itself"));
    }
    let s = p.strip(from).expect("checked");
    let existing = s.send_to(to).copied();
    let mut send = existing.unwrap_or_else(|| Send::new(to));
    if let Some(db) = a.opt_in("level_db", -96.0, 0.0)? {
        send.level = db_to_level(db);
    }
    if let Some(e) = a.opt_bool("enabled")? {
        send.enabled = e;
    }
    if let Some(pre) = a.opt_bool("pre_fader")? {
        send.tap = if pre {
            cantus_core::model::SendTap::Pre
        } else {
            cantus_core::model::SendTap::Post
        };
    }
    let cmd = if existing.is_some() {
        Command::SetStripSend { id: from, send }
    } else {
        if p.routing_would_cycle(from, Some(to)) {
            return Err(ToolError::bad("to", "that send would make a loop"));
        }
        Command::InsertStripSend {
            id: from,
            index: s.sends.len(),
            send,
        }
    };
    let said = Said::new("ai.tools.set_send").nested("send", words::send(p, from, to));
    Ok(plan(
        cmd,
        said,
        json!({"strip_id": from.0, "to": to.0, "level": send.level, "enabled": send.enabled}),
    ))
}

// --- effects ---------------------------------------------------------------------------

/// Where an effect goes: a strip, the master, or a track's strip.
fn host_of(p: &Project, a: A<'_>) -> Res<StripId> {
    if let Some(v) = a.get("strip_id") {
        return args::strip_of(p, v, "strip_id");
    }
    if a.has("track_id") {
        let t = args::track(p, a, "track_id")?;
        return p
            .track(t)
            .and_then(|t| t.strip)
            .ok_or_else(|| ToolError::bad("track_id", "the track goes straight to the master with no strip: route it to a strip with set_track, or put the effect on the master"));
    }
    Err(ToolError::bad(
        "strip_id",
        "give strip_id (or \"master\") or track_id",
    ))
}

fn kind_schema() -> Value {
    // fx:contract — the description names the side round's units (each builder words its own;
    // fx3:contract — wave 3's four).
    json!({"enum": args::effect_words(), "description": "The effect: aequalis (parametric EQ), comprimo (compressor), terminus (limiter), spatium (reverb), multiplex (multiband dynamics), sibilus (de-esser), porta (gate), fervor (saturation), latitudo (stereo), culmen (mastering maximizer), delay, concentus (chorus), flexus (flanger), phasis (phaser), tremor (tremolo/auto-pan), vibrans (vibrato), anulus (ring mod), translatio (frequency shifter), fluctus (volume shaper), tonsor (clipper), impetus (transients), discrimen (transient splitter), speculum (analyser, passes sound through), tonus (auto-tune), aula (convolution reverb: an impulse response, Cantus's own or a file via set_impulse_response), cribrum (auto filter: sweeps, wobbles, auto-wah, talking vowels), fragor (distortion and fuzz pedals, up to three in a chain), lapsus (time shaper: tape stops, half time, stutters, gates, synced to the song), vetus (lo-fi: bits, sample rate, wow and flutter, crackle and hiss, tape or radio tone), libra (utility: gain, balance, width, polarity, channel delay, mono check), tabula (pedalboard: Cantus's own effects as pedals in any order, an A/B split and eight macros; a pedal's parameter is p{id}.{key}, get_unit lists them), saltus (pitch: shifter, harmoniser in key or on MIDI chords, octaver analog or clean, whammy by pedal or notes; mono 2.7 ms or poly 85 ms), rota (rotary speaker), arca (amp and cabinet), loquax (vocoder: the track speaks through a synthesiser played by MIDI notes or through a sidechain; robot voices, talk-box, choirs, string machines, talking drums), fractus (glitch sequencer and grain cloud: repeats, reverses, tape stops, shuffles, pitch jumps, crushes, filter sweeps, gates and delay throws painted on lanes in time with the song, or played by held notes; set_effect's lanes paints them; stutter builds, beat repeats, grain pads, frozen clouds), caelum (shimmer reverb: tails that climb an octave, a fifth or any interval up to three octaves either way, two voices; infinite washes that hold at the level they had and layer what you play under a soft ceiling (isus, Sustain: 100 % holds the wash for ever with its tone, lower lets the climb carry it away), freeze to hold a chord, bloom swells, ducking under the dry or another track via sidechain, a pre-delay synced to the song; cathedrals, frozen pads, drones, ambient guitars, vocal shimmers). Plain names also work: eq, compressor, limiter, reverb (Spatium), chorus…"})
}

fn params_schema() -> Value {
    json!({"type": "object", "maxProperties": 256, "additionalProperties": {"type": ["number", "boolean"]},
        "description": "Keys and values in the parameter's unit (get_unit lists every key, range, unit and the numbers of a stepped one's choices); true/false for switches. Out of range is refused."})
}

/// Set `params` on `spec`, refusing unknown keys and values out of range.
pub fn apply_params(spec: &mut EffectSpec, params: &Map<String, Value>) -> Res<Vec<(String, f32)>> {
    let mut out = Vec::new();
    for (k, v) in params {
        // fx3:contract — routed: a board's `p{id}.…` is its pedal's.
        let info = spec.param(k).ok_or_else(|| {
            ToolError::bad(
                k.as_str(),
                format!(
                    "{} has no parameter {k:?} (get_unit lists them)",
                    args::effect_names(spec.kind).0
                ),
            )
        })?;
        let x = match v {
            Value::Bool(b) => f32::from(u8::from(*b)),
            _ => v
                .as_f64()
                .filter(|x| x.is_finite())
                .ok_or_else(|| ToolError::bad(k.as_str(), "must be a number"))?
                as f32,
        };
        if x < info.min - 1e-6 || x > info.max + 1e-6 {
            return Err(ToolError::bad(
                k.as_str(),
                format!(
                    "must be {} to {} {}, not {x}",
                    info.min, info.max, info.unit
                ),
            ));
        }
        let stored = spec.set(k, x).expect("known key");
        out.push((k.clone(), stored));
    }
    Ok(out)
}

pub fn add_effect_schema() -> Value {
    args::object(
        json!({
            "strip_id": args::s_strip("The strip to put it on"),
            "track_id": s_id("Or: the strip this track plays through."),
            "kind": kind_schema(),
            "index": {"type": "integer", "minimum": 0, "maximum": MAX_INSERTS - 1, "description": "Its place in the chain (0 first); left out, the end."},
            "neutral": {"type": "boolean", "default": false, "description": "Start at its neutral defaults instead of its starting preset (effects load a musical starting preset on insert, as in the app)."},
            "params": params_schema(),
            "mix": {"type": "number", "minimum": 0, "maximum": 1, "description": "The slot's dry/wet, 0 dry … 1 all effect (default 1)."},
            "bypass": {"type": "boolean"}
        }),
        &["kind"],
    )
}

pub fn add_effect(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let host = host_of(p, a)?;
    let name = a.str("kind")?;
    let kind = args::effect_kind(name).ok_or_else(|| {
        ToolError::bad(
            "kind",
            format!("{name:?} is not an effect (see the schema)"),
        )
    })?;
    let chain = p.effects_of(host).map_or(0, |c| c.len());
    if chain >= MAX_INSERTS {
        return Err(ToolError::Refused(format!(
            "the strip already has {MAX_INSERTS} inserts, the most it can hold"
        )));
    }
    let index = a
        .opt_u64("index")?
        .map_or(chain, |i| (i as usize).min(chain));
    let mut slot: EffectSlot = cx.bus_mut().new_effect(kind);
    if !A(a.0).opt_bool("neutral")?.unwrap_or(false) {
        cantus_core::starting::apply(&mut slot.spec);
    }
    let set = match a.opt_object("params")? {
        Some(m) => apply_params(&mut slot.spec, m)?,
        None => Vec::new(),
    };
    if let Some(m) = a.opt_in("mix", 0.0, 1.0)? {
        slot.mix = m as f32;
    }
    if let Some(b) = a.opt_bool("bypass")? {
        slot.bypass = b;
    }
    let id = slot.id;
    let (kid, kname) = args::effect_names(kind);
    let said = Said::new("ai.tools.add_effect")
        .arg("kind", kname)
        .nested("strip", words::strip(cx.project(), host));
    let result = json!({"effect_id": id.0, "strip_id": args::strip_json(host), "kind": kid, "index": index,
        "set": set.into_iter().map(|(k, v)| (k, json!(v))).collect::<Map<_, _>>()});
    Ok(plan(
        Command::InsertEffect {
            strip: host,
            index,
            slot: Box::new(slot),
        },
        said,
        result,
    ))
}

pub fn strip_name(p: &Project, id: StripId) -> String {
    if id.is_master() {
        "Master".into()
    } else {
        p.strip(id).map(|s| s.name.clone()).unwrap_or_default()
    }
}

pub fn set_effect_schema() -> Value {
    args::object(
        json!({
            "effect_id": s_id("The effect (get_mixer lists them)."),
            "params": params_schema(),
            "reset": {"type": "array", "items": {"type": "string"}, "description": "Keys to put back at their defaults."},
            "bypass": {"type": "boolean"},
            "mix": {"type": "number", "minimum": 0, "maximum": 1, "description": "The slot's dry/wet."},
            "index": {"type": "integer", "minimum": 0, "maximum": MAX_INSERTS - 1, "description": "Move it to this place in its chain."},
            "sidechain_strip_id": {"type": ["integer", "null"], "minimum": 1, "description": "Key its detector from this strip (compressors, gates…); null: its own input."},
            "plugin_params": {"type": "object", "additionalProperties": {"type": "number"}, "description": "A hosted plug-in's parameters by its numeric ids, in its own range."},
            "lanes": lanes_schema()
        }),
        &["effect_id"],
    )
}

/// fxa:fractus — Fractus's lanes, painted by name.
fn lanes_schema() -> Value {
    json!({"type": "object", "maxProperties": 10,
        "propertyNames": {"enum": cantus_core::fx::fractus::LANE_KEYS},
        "additionalProperties": {"type": "array", "maxItems": 256, "items": {"type": "object",
            "properties": {
                "at": {"type": "integer", "minimum": 0, "maximum": 255, "description": "The step it starts on (0 the cycle's first; step and bars set the grid)."},
                "len": {"type": "integer", "minimum": 1, "maximum": 256, "description": "Steps it holds, its start included."},
                "amount": {"type": "number", "minimum": 0, "maximum": 1, "description": "The module's mix for the event (default 1)."},
                "chance": {"type": "number", "minimum": 0, "maximum": 1, "description": "The chance it plays each cycle (default 1)."}
            },
            "required": ["at", "len"], "additionalProperties": false}},
        "description": "Fractus only: lanes replaced by name (repeat, reverse, tape, shuffle, pitch, crush, filter, gate, throw, cloud), each a list of blocks; [] clears a lane. Overlapping blocks are cut where the one before ends."})
}

/// fxa:fractus — the commands that paint `lanes` on a Fractus.
fn lane_commands(
    slot: &cantus_core::EffectSlot,
    strip: StripId,
    lanes: &Map<String, Value>,
) -> Res<Vec<Command>> {
    use cantus_core::fx::fractus::{Block, LANE_KEYS, MAX_STEPS};
    if slot.spec.kind != cantus_core::EffectKind::Glitch {
        return Err(ToolError::bad("lanes", "only Fractus has lanes"));
    }
    let mut out = Vec::new();
    for (name, blocks) in lanes {
        let lane = LANE_KEYS
            .iter()
            .position(|k| k == name)
            .ok_or_else(|| ToolError::bad("lanes", format!("no lane {name:?}")))?;
        let list = blocks
            .as_array()
            .ok_or_else(|| ToolError::bad("lanes", "each lane is a list of blocks"))?;
        let mut v = Vec::with_capacity(list.len());
        for b in list {
            let n = |k: &str| b.get(k).and_then(Value::as_f64);
            let (Some(at), Some(len)) = (n("at"), n("len")) else {
                return Err(ToolError::bad("lanes", "a block needs at and len"));
            };
            if !(0.0..MAX_STEPS as f64).contains(&at) || !(1.0..=MAX_STEPS as f64).contains(&len) {
                return Err(ToolError::bad("lanes", "at 0..255, len 1..256"));
            }
            let unit = |k: &str| -> Res<f32> {
                match n(k) {
                    None => Ok(1.0),
                    Some(x) if (0.0..=1.0).contains(&x) => Ok(x as f32),
                    Some(_) => Err(ToolError::bad("lanes", format!("{k} is 0 to 1"))),
                }
            };
            v.push(Block {
                at: at as u16,
                len: len as u16,
                amount: unit("amount")?,
                prob: unit("chance")?,
            });
        }
        out.push(Command::SetFractusLane {
            strip,
            effect: slot.id,
            lane: lane as u8,
            blocks: cantus_core::fx::fractus::sanitize(&v),
        });
    }
    Ok(out)
}

pub fn set_effect(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let (strip, effect) = args::effect(p, a, "effect_id")?;
    let slot = p.effect(strip, effect).expect("found").clone();
    let mut cmds = Vec::new();
    let mut done = Map::new();
    if let Some(m) = a.opt_object("params")? {
        let mut spec = slot.spec.clone();
        for (k, v) in apply_params(&mut spec, m)? {
            done.insert(k.clone(), json!(v));
            cmds.push(Command::SetEffectParam {
                strip,
                effect,
                key: k,
                value: v,
            });
        }
    }
    if let Some(keys) = a.opt_array("reset")? {
        for k in keys {
            let k = k
                .as_str()
                .ok_or_else(|| ToolError::bad("reset", "keys are text"))?;
            slot.spec
                .kind
                .param(k)
                .ok_or_else(|| ToolError::bad("reset", format!("no parameter {k:?}")))?;
            cmds.push(Command::ClearEffectParam {
                strip,
                effect,
                key: k.to_string(),
            });
        }
        done.insert("reset".into(), json!(keys));
    }
    if let Some(m) = a.opt_object("plugin_params")? {
        if slot.spec.external.is_none() {
            return Err(ToolError::bad(
                "plugin_params",
                "only a hosted plug-in has them; use params",
            ));
        }
        for (k, v) in m {
            let id: u32 = k.parse().map_err(|_| {
                ToolError::bad("plugin_params", "keys are the plug-in's numeric ids")
            })?;
            let value = v
                .as_f64()
                .ok_or_else(|| ToolError::bad("plugin_params", "values are numbers"))?;
            cmds.push(Command::SetPluginParam {
                strip,
                effect,
                id,
                value: Some(value),
            });
        }
        done.insert("plugin_params".into(), json!(m));
    }
    // fxa:fractus
    if let Some(m) = a.opt_object("lanes")? {
        cmds.extend(lane_commands(&slot, strip, m)?);
        done.insert("lanes".into(), json!(m));
    }
    if let Some(b) = a.opt_bool("bypass")? {
        cmds.push(Command::SetEffectBypass {
            strip,
            effect,
            bypass: b,
        });
        done.insert("bypass".into(), json!(b));
    }
    if let Some(m) = a.opt_in("mix", 0.0, 1.0)? {
        cmds.push(Command::SetEffectMix {
            strip,
            effect,
            mix: m as f32,
        });
        done.insert("mix".into(), json!(m));
    }
    if a.0.get("sidechain_strip_id").is_some() {
        let source = match a.opt_u64("sidechain_strip_id")? {
            None => None,
            Some(s) => {
                let id = StripId(s);
                p.strip(id)
                    .ok_or_else(|| ToolError::NotFound(format!("strip {s}")))?;
                if !slot.spec.takes_sidechain() {
                    return Err(ToolError::bad(
                        "sidechain_strip_id",
                        "this effect has no sidechain",
                    ));
                }
                Some(id)
            }
        };
        cmds.push(Command::SetEffectSidechain {
            strip,
            effect,
            source,
        });
        done.insert("sidechain_strip_id".into(), json!(source.map(|s| s.0)));
    }
    if let Some(i) = a.opt_u64("index")? {
        let len = p.effects_of(strip).map_or(1, |c| c.len());
        let index = (i as usize).min(len - 1);
        cmds.push(Command::MoveEffect {
            strip,
            effect,
            index,
        });
        done.insert("index".into(), json!(index));
    }
    let (_, kname) = args::effect_names(slot.spec.kind);
    let said = Said::new("ai.tools.set_effect")
        .arg("kind", kname)
        .nested("strip", words::strip(p, strip))
        .nested("what", words::fields(done.keys()));
    done.insert("effect_id".into(), json!(effect.0));
    Ok(plan(
        args::one(cmds, "cmd.ai.set_effect")?,
        said,
        Value::Object(done),
    ))
}

pub fn replace_effect_schema() -> Value {
    args::object(
        json!({
            "effect_id": s_id("The effect to replace."),
            "kind": kind_schema(),
            "neutral": {"type": "boolean", "default": false, "description": "Start at its neutral defaults instead of its starting preset."},
            "params": params_schema()
        }),
        &["effect_id", "kind"],
    )
}

/// Put another effect in an effect's place (its settings go: destructive). The new one has
/// a new id, given back.
pub fn replace_effect(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let (strip, effect) = args::effect(p, a, "effect_id")?;
    let old = p.effect(strip, effect).expect("found").clone();
    let name = a.str("kind")?;
    let kind = args::effect_kind(name)
        .ok_or_else(|| ToolError::bad("kind", format!("{name:?} is not an effect")))?;
    if kind == EffectKind::External {
        return Err(ToolError::bad(
            "kind",
            "a hosted plug-in is chosen in Cantus",
        ));
    }
    let mut slot = cx.bus_mut().new_effect(kind);
    if !a.opt_bool("neutral")?.unwrap_or(false) {
        cantus_core::starting::apply(&mut slot.spec);
    }
    if let Some(m) = a.opt_object("params")? {
        apply_params(&mut slot.spec, m)?;
    }
    slot.mix = old.mix;
    let id = slot.id;
    let said = Said::new("ai.tools.replace_effect")
        .arg("old", args::effect_names(old.spec.kind).1)
        .arg("kind", args::effect_names(kind).1)
        .nested("strip", words::strip(cx.project(), strip));
    Ok(plan(
        Command::ReplaceEffect {
            strip,
            effect,
            slot: Box::new(slot),
        },
        said,
        json!({"effect_id": id.0, "replaced": effect.0, "strip_id": args::strip_json(strip)}),
    ))
}
