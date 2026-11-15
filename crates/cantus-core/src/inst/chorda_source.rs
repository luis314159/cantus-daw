//! Chorda's **source** — the modelled piano or a recorded one — and its mixer (round 40
//! phase 3, ADR 0125). Owned by `chorda-src`; the contract declares the shapes. The model's
//! voicing (`cantus_dsp::chorda*`, the engine's `inst::chorda`) is the `chorda` builder's
//! and is not touched here.
//!
//! **The source** is the text key [`SOURCE_KEY`]: absent, Chorda plays its model (every
//! older song); `pack:<pack>/<instrument>` plays that recorded piano — a curated free pack
//! (the Salamander, Upright Piano KW, bitKlavier) or one the user installed from a file (a
//! bought SFZ, Decent Sampler or SF2 piano, ADR 0125 §3).
//!
//! **Through Exemplar's engine, not a copy.** A recorded source keeps the pack's zones in
//! Chorda's own spec, under [`REC_PREFIX`] (`rec.z12.lo`, `rec.z12.sample`, `rec.g0.gain`,
//! `rec.release`): Exemplar's key space, prefixed, so a pack's update never changes a song
//! and nothing of the model's keys collides. [`snapshot_spec`] compiles such a Chorda, in
//! the snapshot only, into the Exemplar spec it plays — its zones unprefixed, its outputs
//! carried — and the engine plays it with Exemplar's own unit (its streaming, its round
//! robin, its buses). The project still says *Chorda*: the editor, presets, the agents and
//! a switch back to the model see one instrument.
//!
//! **The mixer** is ADR 0123's: the perspectives (a pack's mics; the model's virtual mics,
//! which the `chorda` builder adds after its voicing pass) and the noises (hammer and
//! release, pedal, keys, release resonance — where the source has them) are named outputs,
//! `o{n}.` keys and `outputs` routes, each its own strip in one click.
//!
//! **The switch** model ↔ recorded is one undo step that keeps the notes, the automation
//! that still applies, the strips and the outputs ([`switch`]).

use super::{sampler, InstParamInfo, InstrumentKind, NativeSpec};
use crate::inst_out::{OutputFamily, OutputField, OutputId, OutputSlot};
use crate::model::{Project, TrackId};

/// The text key holding the source (absent: the model).
pub const SOURCE_KEY: &str = "source";

/// The prefix of a recorded source's Exemplar keys inside Chorda's spec.
pub const REC_PREFIX: &str = "rec.";

/// What Chorda plays.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChordaSource {
    /// The modelled piano (ADR 0059, 0098): every older Chorda.
    #[default]
    Model,
    Recorded(RecordedPiano),
}

/// A recorded piano: an instrument of an installed pack (`cantus_content::PackInstrument`)
/// — the variant is the pack instrument (a full and a light version of one piano are two).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RecordedPiano {
    pub pack: String,
    pub instrument: String,
}

impl ChordaSource {
    pub fn parse(text: &str) -> Option<ChordaSource> {
        let rest = text.strip_prefix("pack:")?;
        let (pack, instrument) = rest.split_once('/')?;
        (!pack.is_empty() && !instrument.is_empty()).then(|| {
            ChordaSource::Recorded(RecordedPiano {
                pack: pack.to_string(),
                instrument: instrument.to_string(),
            })
        })
    }

    /// The text [`SOURCE_KEY`] holds; `None` for the model (the key absent).
    pub fn text(&self) -> Option<String> {
        match self {
            ChordaSource::Model => None,
            ChordaSource::Recorded(r) => Some(format!("pack:{}/{}", r.pack, r.instrument)),
        }
    }

    /// What `spec` plays. A source that does not parse is the model.
    pub fn of(spec: &NativeSpec) -> ChordaSource {
        spec.text(SOURCE_KEY)
            .and_then(ChordaSource::parse)
            .unwrap_or_default()
    }

    pub fn is_recorded(&self) -> bool {
        matches!(self, ChordaSource::Recorded(_))
    }
}

/// The recorded piano offered first (the owner, 2026-11-12): the Salamander — until his ABX
/// of bitKlavier against it says otherwise.
pub const DEFAULT_RECORDED: (&str, &str) = ("salamander-grand-piano", "grand");

/// One entry of the source list, in its order: the model (Chorda 2), the previous model
/// (Chorda 1 — the owner keeps it in the list, 2026-11-12: `model` = 1), then every
/// installed recorded piano. The model's version stays the `model` key (ADR 0059);
/// [`ChordaSource::Model`] covers both models.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SourceChoice {
    /// The model of this version (`model` = the value).
    Model(u8),
    Recorded(RecordedPiano),
}

/// The model's perspectives and its mechanics, as named outputs (ADR 0123): what Chorda's
/// mixer shows for the modelled piano — **Hammers, Player, Audience, Room, Mechanics**,
/// fixed, never moved (the owner, 2026-11-12: movable or added mics on the modelled piano
/// are later, BACKLOG). The `chorda` builder renders them as model-2 buses after its
/// voicing pass, null at their defaults; ids are forever.
pub const MODEL_OUTPUTS: [(u16, &str, crate::inst_out::OutputFamily); 5] = {
    use crate::inst_out::OutputFamily::{Noise, Perspective, Rooms};
    [
        (0, "chorda_source.out.hammers", Perspective),
        (1, "chorda_source.out.player", Perspective),
        (2, "chorda_source.out.audience", Perspective),
        (3, "chorda_source.out.room", Rooms),
        (4, "chorda_source.out.mechanics", Noise),
    ]
};

/// Whether Chorda's model renders [`MODEL_OUTPUTS`] apart (on lanes a strip can take).
/// Until the `chorda` builder's model does, the five play on Main together: the mixer says
/// so, and *Each mic to its own strip* leaves them there (`inst_out::definition`).
pub const MODEL_RENDERS_OUTPUTS: bool = false;

/// The English names of [`MODEL_OUTPUTS`], in their order: what a switch matches a
/// recorded piano's outputs by (a strip named *Room* keeps meaning the room).
pub const MODEL_OUTPUT_NAMES: [&str; 5] = ["Hammers", "Player", "Audience", "Room", "Mechanics"];

/// One output a source has, in the mixer's order.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceOutput {
    pub id: OutputId,
    /// As the source says it (a pack's mic: *Stage Lid ORTF*; the model's: *Hammers*).
    pub name: String,
    pub family: OutputFamily,
}

/// The model's outputs (Hammers, Player, Audience, Room, Mechanics).
pub fn model_outputs() -> Vec<SourceOutput> {
    MODEL_OUTPUTS
        .iter()
        .zip(MODEL_OUTPUT_NAMES)
        .map(|((id, _, family), name)| SourceOutput {
            id: OutputId(*id),
            name: name.to_string(),
            family: *family,
        })
        .collect()
}

/// The family a recorded output's name says: its noises (*Releases*, *Pedal*,
/// *Hammers*, *Keys*), its rooms (*Room*, *Hall*, *Ambience*), else a perspective.
pub fn family_of(name: &str) -> OutputFamily {
    let n = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&[
        "release", "pedal", "hammer", "noise", "mechanic", "damper", "relaj", "ruido", "martill",
    ]) {
        OutputFamily::Noise
    } else if has(&["room", "hall", "ambien", "sala"]) || n == "amb" {
        OutputFamily::Rooms
    } else {
        OutputFamily::Perspective
    }
}

/// A recorded piano's outputs, read off its zones (Exemplar's keys, unprefixed): each
/// distinct group output in ascending order, named by its first group's name (*Output n*
/// when none is named). A piano whose groups name none has one output, 0.
pub fn outputs_of_zones(zones: &NativeSpec) -> Vec<SourceOutput> {
    let mut used = [false; sampler::MAX_GROUPS];
    let mut zone_has_group = std::collections::BTreeSet::new();
    for (k, v) in &zones.params {
        if let Some((z, "group")) = sampler::zone_key(k) {
            used[(*v as usize).min(sampler::MAX_GROUPS - 1)] = true;
            zone_has_group.insert(z);
        }
    }
    // A zone storing no group plays in group 0.
    used[0] |= zones
        .text
        .keys()
        .filter_map(|k| match sampler::zone_key(k) {
            Some((z, "sample")) => Some(z),
            _ => None,
        })
        .any(|z| !zone_has_group.contains(&z));
    let mut out: Vec<SourceOutput> = Vec::new();
    for (m, _) in used.iter().enumerate().filter(|(_, u)| **u) {
        let id = zones
            .params
            .get(&format!("g{m}.out"))
            .map_or(0, |v| *v as u16);
        if out.iter().any(|o| o.id.0 == id) {
            continue;
        }
        let name = zones
            .text(&format!("g{m}.name"))
            .map(str::to_string)
            .unwrap_or_else(|| format!("Output {}", id + 1));
        out.push(SourceOutput {
            id: OutputId(id),
            family: family_of(&name),
            name,
        });
    }
    out.sort_by_key(|o| o.id);
    if out.is_empty() {
        out.push(SourceOutput {
            id: OutputId(0),
            name: "Output 1".into(),
            family: OutputFamily::Perspective,
        });
    }
    out
}

/// A Chorda's outputs: its recorded piano's (read off its `rec.` zones), or the model's.
pub fn outputs(spec: &NativeSpec) -> Vec<SourceOutput> {
    if has_zones(spec) {
        outputs_of_zones(&zones_of(spec))
    } else {
        model_outputs()
    }
}

/// The piano type the scene draws for a recorded piano (ADR 0125 §3, ADR 0126): concert
/// grand for bitKlavier's Steinway D, grand for the Salamander's C5 (and its Accurate
/// edition), upright for Upright Piano KW, Old Piano FB and a user's pack with *upright* in
/// its id. `None`: the stored look decides (the model, a piano nobody told us about).
pub fn scene_piano(source: &ChordaSource) -> Option<super::chorda_look::Body> {
    use super::chorda_look::Body;
    let ChordaSource::Recorded(r) = source else {
        return None;
    };
    let id = r.pack.to_ascii_lowercase();
    if id.starts_with("bitklavier") {
        Some(Body::Concert)
    } else if id.contains("salamander") {
        Some(Body::Grand)
    } else if id.contains("upright") || id.contains("old-piano") || id.contains("honky") {
        Some(Body::Upright)
    } else if id.contains("baby") || id.contains("parlour") {
        Some(Body::Parlour)
    } else {
        None
    }
}

/// A recorded source's key without its prefix (`rec.z3.lo` → `z3.lo`).
pub fn rec_key(key: &str) -> Option<&str> {
    key.strip_prefix(REC_PREFIX)
}

/// The description of a recorded source's key (Exemplar's, prefixed).
pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    super::sampler::param(rec_key(key)?)
}

pub fn is_text_key(key: &str) -> bool {
    key == SOURCE_KEY || rec_key(key).is_some_and(super::sampler::is_text_key)
}

pub fn is_asset_key(key: &str) -> bool {
    rec_key(key).is_some_and(super::sampler::is_asset_key)
}

/// Whether a Chorda holds a recorded piano's zones (a file in some `rec.z{n}.sample`).
pub fn has_zones(spec: &NativeSpec) -> bool {
    spec.text.iter().any(|(k, v)| {
        !v.is_empty() && matches!(rec_key(k).and_then(sampler::zone_key), Some((_, "sample")))
    })
}

/// The recorded piano a Chorda holds, as the Exemplar spec of its zones (every `rec.`
/// key unprefixed; nothing else).
pub fn zones_of(spec: &NativeSpec) -> NativeSpec {
    let mut e = NativeSpec::new(InstrumentKind::Sampler);
    for (k, v) in &spec.params {
        if let Some(r) = rec_key(k) {
            e.params.insert(r.to_string(), *v);
        }
    }
    for (k, v) in &spec.text {
        if let Some(r) = rec_key(k) {
            e.text.insert(r.to_string(), v.clone());
        }
    }
    e
}

/// The instrument a track's snapshot plays (`Snapshot::compile`'s one call). A Chorda
/// with a recorded source and its zones becomes the Exemplar spec of its zones (ADR 0125
/// §2): every `rec.` key unprefixed, the mixer's `o{n}.` keys and the routes carried, and
/// the zones of an output turned off (`o{n}.on` = 0) left out — it loads nothing. A Chorda
/// whose source names a piano but holds no zones (nothing was read) plays its model; every
/// other instrument plays as it is.
pub fn snapshot_spec(spec: &crate::InstrumentSpec) -> crate::InstrumentSpec {
    let exemplar = match spec.native() {
        Some(n)
            if n.kind == InstrumentKind::Chorda
                && ChordaSource::of(n).is_recorded()
                && has_zones(n) =>
        {
            exemplar_of(n)
        }
        // An Exemplar with more zones than every unit holds says how many it needs.
        Some(n) if n.kind == InstrumentKind::Sampler => n.clone(),
        _ => return spec.clone(),
    };
    crate::InstrumentSpec::Native(sized(exemplar))
}

/// An Exemplar spec stamped with the zone table it needs, when that is more than every
/// unit holds ([`sampler::CAPACITY_KEY`]); as it is otherwise.
fn sized(mut e: NativeSpec) -> NativeSpec {
    let cap = sampler::capacity_needed(&e);
    if cap > sampler::BASE_ZONES {
        e.params
            .insert(sampler::CAPACITY_KEY.to_string(), cap as f32);
    }
    e
}

/// [`snapshot_spec`]'s Exemplar: see there.
pub fn exemplar_of(n: &NativeSpec) -> NativeSpec {
    // Outputs turned off, and each group's output.
    let mut off = 0u32;
    for (k, v) in &n.params {
        if let Some((o, OutputField::On)) = crate::inst_out::output_key(k) {
            if *v < 0.5 {
                off |= 1 << o;
            }
        }
    }
    let mut group_out = [0u8; sampler::MAX_GROUPS];
    let mut zone_group = vec![0u8; sampler::MAX_ZONES];
    for (k, v) in &n.params {
        let Some(r) = rec_key(k) else {
            continue;
        };
        if let Some((m, "out")) = sampler::group_key(r) {
            group_out[m] = (*v as u8).min(31);
        } else if let Some((z, "group")) = sampler::zone_key(r) {
            zone_group[z] = (*v as u8).min(sampler::MAX_GROUPS as u8 - 1);
        }
    }
    let dropped = |r: &str| {
        off != 0
            && sampler::zone_key(r)
                .is_some_and(|(z, _)| off & (1 << group_out[zone_group[z] as usize]) != 0)
    };
    let mut e = NativeSpec::new(InstrumentKind::Sampler);
    for (k, v) in &n.params {
        if let Some(r) = rec_key(k) {
            if !dropped(r) {
                e.params.insert(r.to_string(), *v);
            }
        } else if crate::inst_out::output_key(k).is_some() {
            e.params.insert(k.clone(), *v);
        }
    }
    for (k, v) in &n.text {
        if let Some(r) = rec_key(k) {
            if !dropped(r) {
                e.text.insert(r.to_string(), v.clone());
            }
        }
    }
    // Every output off: no zone at all — silence, never Exemplar's starting sine.
    if !e.text.keys().any(|k| sampler::zone_key(k).is_some()) {
        e.text.insert("z0.name".into(), String::new());
    }
    e.outputs = n.outputs.clone();
    e
}

/// Why a source cannot be switched to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceRefusal {
    /// The track is gone or does not host a Chorda.
    NotChorda,
    /// The pack is not installed (or its instrument is not in it).
    NotInstalled,
    /// The pack's instrument could not be read; the reason, for the notice and the log.
    Unreadable(String),
    /// Already playing it.
    Same,
    /// Not built yet (the contract's stub). No longer returned.
    NotYet,
}

/// What a Chorda's outputs become when its source changes: each new output takes the old
/// one of the same name (any case), else the first unmatched old one of its family — its
/// strip (a perspective's own strip kept) and its `o{n}.` mix. Old outputs nothing takes
/// are left: their mix keys cleared, their slots gone (the strips stay on the desk).
pub fn carry_outputs(
    old_spec: &NativeSpec,
    old: &[SourceOutput],
    new: &[SourceOutput],
    into: &mut NativeSpec,
) {
    let mut taken = vec![false; old.len()];
    let mut pairs: Vec<(OutputId, OutputId, &SourceOutput)> = Vec::new();
    for n in new {
        let by_name = old
            .iter()
            .enumerate()
            .find(|(i, o)| !taken[*i] && o.name.eq_ignore_ascii_case(&n.name));
        let by_family = || {
            old.iter()
                .enumerate()
                .find(|(i, o)| !taken[*i] && o.family == n.family)
        };
        if let Some((i, o)) = by_name.or_else(by_family) {
            taken[i] = true;
            pairs.push((o.id, n.id, n));
        }
    }
    // The mixer's keys: every old one cleared, then each carried one written at its new id.
    let olds: Vec<(String, f32)> = old_spec
        .params
        .iter()
        .filter(|(k, _)| crate::inst_out::output_key(k).is_some())
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    for (k, _) in &olds {
        into.params.remove(k);
    }
    for (k, v) in &olds {
        let Some((o, f)) = crate::inst_out::output_key(k) else {
            continue;
        };
        if let Some((_, to, _)) = pairs.iter().find(|p| usize::from(p.0 .0) == o) {
            into.params.insert(crate::inst_out::key(*to, f), *v);
        }
    }
    let mut slots: Vec<OutputSlot> = Vec::new();
    for (from, to, n) in &pairs {
        if let Some(s) = old_spec.outputs.iter().find(|s| s.id == *from) {
            slots.push(OutputSlot {
                id: *to,
                name: n.name.clone(),
                family: n.family,
                strip: s.strip,
            });
        }
    }
    slots.sort_by_key(|s| s.id);
    into.outputs = slots;
}

/// The Chorda `base` playing `to`: the source key, every `rec.` key written from `zones`
/// (Exemplar's keys, unprefixed) or cleared for the model, the outputs carried
/// ([`carry_outputs`]). The model's own keys stay as they are, so a switch back finds
/// them where they were; nothing else changes.
pub fn with_source(base: &NativeSpec, to: &ChordaSource, zones: Option<&NativeSpec>) -> NativeSpec {
    let mut s = base.clone();
    s.params.retain(|k, _| rec_key(k).is_none());
    s.text.retain(|k, _| rec_key(k).is_none());
    s.text.remove(SOURCE_KEY);
    if let (Some(text), Some(z)) = (to.text(), zones) {
        s.text.insert(SOURCE_KEY.to_string(), text);
        for (k, v) in &z.params {
            if sampler::param(k).is_some() {
                s.params.insert(format!("{REC_PREFIX}{k}"), *v);
            }
        }
        for (k, v) in &z.text {
            if sampler::is_text_key(k) {
                s.text.insert(format!("{REC_PREFIX}{k}"), v.clone());
            }
        }
    }
    let new = outputs(&s);
    carry_outputs(base, &outputs(base), &new, &mut s);
    s
}

/// The switch to `to` as one undo step: the source key, the recorded zones written (or
/// cleared), the outputs carried over by name and family (a perspective's strip kept), the
/// automation of keys the new source has kept — and the notes, which live in the clips,
/// never touched. `zones` is the recorded piano read off the audio thread (Exemplar's
/// keys, unprefixed: what the instrument picker would give an Exemplar). The model's own
/// keys stay in the spec while a piano plays, so their automation lanes are kept and play
/// again after a switch back.
pub fn switch(
    project: &Project,
    track: TrackId,
    to: &ChordaSource,
    zones: Option<&NativeSpec>,
) -> Result<crate::Command, SourceRefusal> {
    let base = project
        .track(track)
        .and_then(|t| t.instrument.native())
        .filter(|n| n.kind == InstrumentKind::Chorda)
        .ok_or(SourceRefusal::NotChorda)?;
    let now = ChordaSource::of(base);
    if let ChordaSource::Recorded(_) = to {
        let z = zones.ok_or(SourceRefusal::NotInstalled)?;
        if !z
            .text
            .iter()
            .any(|(k, v)| !v.is_empty() && matches!(sampler::zone_key(k), Some((_, "sample"))))
        {
            return Err(SourceRefusal::Unreadable(
                "the instrument has no sample in it".into(),
            ));
        }
        if now == *to && has_zones(base) && zones_of(base) == strip_kind(z) {
            return Err(SourceRefusal::Same);
        }
    } else if now == *to && !has_zones(base) {
        return Err(SourceRefusal::Same);
    }
    Ok(one_step(track, with_source(base, to, zones)))
}

/// The switch as one undo step, named as the source list says it (`cmd.chorda_source`).
fn one_step(track: TrackId, spec: NativeSpec) -> crate::Command {
    crate::Command::Batch {
        label_key: "cmd.chorda_source".into(),
        commands: vec![crate::Command::SetInstrument {
            track,
            spec: Box::new(crate::InstrumentSpec::Native(spec)),
        }],
    }
}

/// A spec compared as zones only (its kind made Exemplar's).
fn strip_kind(z: &NativeSpec) -> NativeSpec {
    let mut e = NativeSpec::new(InstrumentKind::Sampler);
    e.params = z.params.clone();
    e.text = z.text.clone();
    e
}

/// The source list's entry `choice` as a switch: a model version (Chorda 2, or Chorda 1
/// — the previous model, `model` = 1) or a recorded piano. Choosing a model while a piano
/// plays goes back to the model *and* sets its version, in one step.
pub fn switch_choice(
    project: &Project,
    track: TrackId,
    choice: &SourceChoice,
    zones: Option<&NativeSpec>,
) -> Result<crate::Command, SourceRefusal> {
    match choice {
        SourceChoice::Recorded(r) => {
            switch(project, track, &ChordaSource::Recorded(r.clone()), zones)
        }
        SourceChoice::Model(v) => {
            let base = project
                .track(track)
                .and_then(|t| t.instrument.native())
                .filter(|n| n.kind == InstrumentKind::Chorda)
                .ok_or(SourceRefusal::NotChorda)?;
            let recorded = ChordaSource::of(base).is_recorded() || has_zones(base);
            let model = base.get("model");
            if !recorded && model == f32::from(*v) {
                return Err(SourceRefusal::Same);
            }
            let mut spec = with_source(base, &ChordaSource::Model, None);
            spec.set("model", f32::from(*v));
            Ok(one_step(track, spec))
        }
    }
}

/// What a Chorda's source list marks as chosen.
pub fn current_choice(spec: &NativeSpec) -> SourceChoice {
    match ChordaSource::of(spec) {
        ChordaSource::Recorded(r) if has_zones(spec) => SourceChoice::Recorded(r),
        _ => SourceChoice::Model(spec.get("model").round().clamp(1.0, 255.0) as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_reads_back_and_an_older_chorda_plays_its_model() {
        let spec = NativeSpec::new(crate::InstrumentKind::Chorda);
        assert_eq!(ChordaSource::of(&spec), ChordaSource::Model);
        assert_eq!(ChordaSource::Model.text(), None);
        let r = ChordaSource::Recorded(RecordedPiano {
            pack: "upright-kw".into(),
            instrument: "upright".into(),
        });
        assert_eq!(ChordaSource::parse(&r.text().unwrap()), Some(r));
        assert_eq!(rec_key("rec.z3.lo"), Some("z3.lo"));
        assert!(param("rec.z3.lo").is_some());
        assert!(is_asset_key("rec.z3.sample") && is_text_key("rec.z3.sample"));
        assert!(!is_asset_key("z3.sample"));
        assert!(is_text_key(SOURCE_KEY));
    }
}
