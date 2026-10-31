//! The phrases the tools' sentences are made of — a clip, a strip, an effect, a parameter,
//! a list — as [`Said`]s the host words in the interface's language (keys `ai.phrase.*`,
//! `ai.field.*` in the `ai` locale area). Names stay the user's, quoted by the host.

use crate::tool::Said;
use cantus_core::automation::AutoTarget;
use cantus_core::tempo::TempoMap;
use cantus_core::{Clip, EffectId, ModKind, ModulatorId, Project, StripId, Ticks, TrackId};

/// Bars `from..to` as numbers from 1 (the bar of `to - 1` tick: a span ending on a bar line
/// does not reach into the next).
pub fn bar_numbers(map: &TempoMap, from: Ticks, to: Ticks) -> (u32, u32) {
    let (a, _, _) = map.bbt(from);
    let (b, _, _) = map.bbt(Ticks((to.0 - 1).max(from.0)));
    (
        a.clamp(1, u32::MAX as i64) as u32,
        b.clamp(1, u32::MAX as i64) as u32,
    )
}

pub fn track_name(p: &Project, id: TrackId) -> String {
    super::args::clean(
        &p.track(id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| format!("{}", id.0)),
    )
}

/// `“Verse” on “Piano” (bars 1–4)`.
pub fn clip(p: &Project, map: &TempoMap, track: TrackId, c: &Clip) -> Said {
    clip_span(p, map, track, &c.name, c.start, c.end())
}

/// A clip's name and track with the bars a range of it covers.
pub fn clip_span(
    p: &Project,
    map: &TempoMap,
    track: TrackId,
    name: &str,
    from: Ticks,
    to: Ticks,
) -> Said {
    let (a, b) = bar_numbers(map, from, to);
    Said::new("ai.phrase.clip")
        .name("clip", super::args::clean(name))
        .name("track", track_name(p, track))
        .bars("bars", a, b)
}

/// A strip by its name, or the master.
pub fn strip(p: &Project, id: StripId) -> Said {
    if id.is_master() {
        Said::new("ai.phrase.master")
    } else {
        let name = p.strip(id).map(|s| s.name.clone()).unwrap_or_default();
        Said::new("ai.phrase.strip").name("strip", super::args::clean(&name))
    }
}

/// `Aequalis on “Piano”`.
pub fn effect(p: &Project, host: StripId, id: EffectId) -> Said {
    let kind = match p.effect(host, id) {
        Some(e) => key(super::args::effect_names(e.spec.kind).1),
        None => Said::new("ai.phrase.an_effect"),
    };
    Said::new("ai.phrase.effect")
        .nested("kind", kind)
        .nested("strip", strip(p, host))
}

/// A track's instrument, or its kit.
pub fn instrument_of(p: &Project, track: TrackId) -> Said {
    Said::new("ai.phrase.track_instrument").name("track", track_name(p, track))
}

pub fn kit_of(p: &Project, track: TrackId) -> Said {
    Said::new("ai.phrase.track_kit").name("track", track_name(p, track))
}

/// A parameter's key, verbatim (it is Cantus's own identifier, the same in every language).
pub fn key(k: &str) -> Said {
    Said::new("ai.phrase.key").arg("key", k)
}

/// What a setting is called, for the fields the tools change.
pub fn field(k: &str) -> Said {
    let key = match k {
        "name" => "ai.field.name",
        "color" => "ai.field.color",
        "gain_db" => "ai.field.gain_db",
        "pan" => "ai.field.pan",
        "width" => "ai.field.width",
        "muted" => "ai.field.muted",
        "soloed" => "ai.field.soloed",
        "index" => "ai.field.index",
        "strip_id" => "ai.field.strip_id",
        "output" => "ai.field.output",
        "phase_invert" => "ai.field.phase_invert",
        "fx_enabled" => "ai.field.fx_enabled",
        "eq" => "ai.field.eq",
        "time_signature" => "ai.field.time_signature",
        "key" => "ai.field.key",
        "loop" => "ai.field.loop",
        "metronome" => "ai.field.metronome",
        "start" => "ai.field.start",
        "length" => "ai.field.length",
        "loop_beats" => "ai.field.loop_beats",
        "track_id" => "ai.field.track_id",
        "bypass" => "ai.field.bypass",
        "mix" => "ai.field.mix",
        "sidechain_strip_id" => "ai.field.sidechain",
        "reset" => "ai.field.reset",
        "plugin_params" => "ai.field.plugin_params",
        other => return key(other),
    };
    Said::new(key)
}

/// `a, b, c` — at most eight, then `and 3 more`.
pub fn list(items: Vec<Said>) -> Said {
    let n = items.len();
    let mut it = items.into_iter();
    let Some(first) = it.next() else {
        return Said::new("ai.phrase.nothing");
    };
    let mut acc = first;
    for s in it.by_ref().take(7) {
        acc = Said::new("ai.phrase.join").nested("a", acc).nested("b", s);
    }
    if n > 8 {
        acc = Said::new("ai.phrase.more").nested("a", acc).arg("n", n - 8);
    }
    acc
}

/// The fields a call sets, as a list.
pub fn fields<'a>(keys: impl IntoIterator<Item = &'a String>) -> Said {
    list(keys.into_iter().map(|k| field(k)).collect())
}

/// A modulator's kind, with an article (`an LFO`) or bare (`LFO`).
pub fn mod_kind(kind: ModKind, article: bool) -> Said {
    Said::new(match (kind, article) {
        (ModKind::Lfo, true) => "ai.phrase.mod.lfo",
        (ModKind::Envelope, true) => "ai.phrase.mod.envelope",
        (ModKind::Follower, true) => "ai.phrase.mod.follower",
        (ModKind::Step, true) => "ai.phrase.mod.step",
        (ModKind::Random, true) => "ai.phrase.mod.random",
        (ModKind::Lfo, false) => "ai.phrase.modname.lfo",
        (ModKind::Envelope, false) => "ai.phrase.modname.envelope",
        (ModKind::Follower, false) => "ai.phrase.modname.follower",
        (ModKind::Step, false) => "ai.phrase.modname.step",
        (ModKind::Random, false) => "ai.phrase.modname.random",
    })
}

/// A modulator by its name, or by its kind and number on its host (`LFO 2`), as the panel
/// names it.
pub fn modulator(p: &Project, id: ModulatorId) -> Said {
    let Some((host, m)) = p.all_modulators().find(|(_, m)| m.id == id) else {
        return Said::new("ai.phrase.key").arg("key", id.0);
    };
    if !m.name.is_empty() {
        return Said::new("ai.phrase.name").name("name", super::args::clean(&m.name));
    }
    let kind = m.source.kind();
    let n = p
        .modulators(host)
        .map(|all| {
            all.iter()
                .filter(|x| x.source.kind() == kind)
                .position(|x| x.id == id)
                .unwrap_or(0)
                + 1
        })
        .unwrap_or(1);
    Said::new("ai.phrase.modnumbered")
        .nested("kind", mod_kind(kind, false))
        .arg("n", n)
}

/// Where a modulator lives.
pub fn mod_host(p: &Project, h: cantus_core::ModHost) -> Said {
    match h {
        cantus_core::ModHost::Track(t) => {
            Said::new("ai.phrase.name").name("name", track_name(p, t))
        }
        cantus_core::ModHost::Strip(s) => strip(p, s),
        cantus_core::ModHost::Master => Said::new("ai.phrase.master"),
    }
}

/// A target of automation or modulation: `the level of “Piano”`.
pub fn target(p: &Project, t: &AutoTarget) -> Said {
    let tn = |id: TrackId| track_name(p, id);
    let host = |e: EffectId| p.effect_host(e).unwrap_or(StripId::MASTER);
    match t {
        AutoTarget::TrackGain(id) => {
            Said::new("ai.phrase.target.track_gain").name("track", tn(*id))
        }
        AutoTarget::TrackPan(id) => Said::new("ai.phrase.target.track_pan").name("track", tn(*id)),
        AutoTarget::StripGain(s) => {
            Said::new("ai.phrase.target.strip_gain").nested("strip", strip(p, *s))
        }
        AutoTarget::StripPan(s) => {
            Said::new("ai.phrase.target.strip_pan").nested("strip", strip(p, *s))
        }
        AutoTarget::StripWidth(s) => {
            Said::new("ai.phrase.target.strip_width").nested("strip", strip(p, *s))
        }
        AutoTarget::Send { strip: a, to } => send(p, *a, *to),
        AutoTarget::MasterGain => Said::new("ai.phrase.target.master_gain"),
        AutoTarget::MasterPan => Said::new("ai.phrase.target.master_pan"),
        AutoTarget::MasterWidth => Said::new("ai.phrase.target.master_width"),
        AutoTarget::Effect { effect: e, key: k } => Said::new("ai.phrase.target.effect")
            .arg("key", k)
            .nested("effect", effect(p, host(*e), *e)),
        AutoTarget::Plugin { effect: e, id } => Said::new("ai.phrase.target.plugin")
            .arg("id", id)
            .nested("effect", effect(p, host(*e), *e)),
        AutoTarget::Instrument { track, key: k } => Said::new("ai.phrase.target.instrument")
            .arg("key", k)
            .name("track", tn(*track)),
        AutoTarget::Firmus { track, param } => Said::new("ai.phrase.target.instrument")
            .arg("key", super::json::firmus_key(*param))
            .name("track", tn(*track)),
        AutoTarget::PluginInstrument { track, id } => {
            Said::new("ai.phrase.target.plugin_instrument")
                .arg("id", id)
                .name("track", tn(*track))
        }
        AutoTarget::ModAmount(m) => {
            Said::new("ai.phrase.target.mod_amount").nested("modulator", modulator(p, *m))
        }
        AutoTarget::NoteFx { track, key: k, .. } => Said::new("ai.phrase.target.notefx")
            .arg("key", k)
            .name("track", tn(*track)),
    }
}

/// `the send from “Keys” to “Hall”`.
pub fn send(p: &Project, from: StripId, to: StripId) -> Said {
    Said::new("ai.phrase.target.send")
        .nested("from", strip(p, from))
        .nested("to", strip(p, to))
}

/// A number as the host writes it (`2.5`, `2,5`): its integer and its fraction as words of
/// their own (fix review 2 #9).
pub fn num(x: f64) -> Said {
    let t = super::args::trim_num(x);
    match t.split_once('.') {
        Some((i, f)) => Said::new("ai.phrase.dec").arg("int", i).arg("frac", f),
        None => Said::new("ai.phrase.int").arg("n", t),
    }
}

/// A position: `9:2.5` (`9:2,5`).
pub fn pos(map: &TempoMap, t: Ticks) -> Said {
    let (bar, beat, tick) = map.bbt(t);
    let b = beat as f64 + tick as f64 / map.beat_len_at(t).0 as f64;
    Said::new("ai.phrase.pos")
        .arg("bar", bar)
        .nested("beat", num(b))
}

/// Where a modulator lives, after its name: `on “Piano”` (`de «Piano»`, `del máster`).
pub fn mod_host_of(p: &Project, h: cantus_core::ModHost) -> Said {
    match h {
        cantus_core::ModHost::Track(t) => {
            Said::new("ai.phrase.of_track").name("track", track_name(p, t))
        }
        cantus_core::ModHost::Strip(s) => {
            let name = p.strip(s).map(|x| x.name.clone()).unwrap_or_default();
            Said::new("ai.phrase.of_strip").name("strip", super::args::clean(&name))
        }
        cantus_core::ModHost::Master => Said::new("ai.phrase.of_master"),
    }
}

/// A count with its noun, singular or plural as the language has it (`1 point`, `3 puntos`).
pub fn count(n: usize, what: &str) -> Said {
    let (one, many) = match what {
        "points" => ("ai.phrase.count.points.one", "ai.phrase.count.points.many"),
        "notes" => ("ai.phrase.count.notes.one", "ai.phrase.count.notes.many"),
        "times" => ("ai.phrase.count.times.one", "ai.phrase.count.times.many"),
        "bars" => ("ai.phrase.count.bars.one", "ai.phrase.count.bars.many"),
        "parameters" => (
            "ai.phrase.count.parameters.one",
            "ai.phrase.count.parameters.many",
        ),
        "changes" => (
            "ai.phrase.count.changes.one",
            "ai.phrase.count.changes.many",
        ),
        _ => ("ai.phrase.count.files.one", "ai.phrase.count.files.many"),
    };
    Said::new(if n == 1 { one } else { many }).arg("n", n)
}
