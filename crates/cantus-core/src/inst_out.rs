//! An instrument's **named outputs** — a kit's microphones, a piano's perspectives and its
//! noises, a pad — each heard on the instrument's own strip (*Main*), on a strip made for
//! it (*its own strip*) or on a strip the user chose (round 40 phase 3, ADR 0123, which
//! generalises ADR 0100's per-pad outputs).
//!
//! **What is stored.** A native instrument's [`NativeSpec::outputs`](crate::NativeSpec)
//! lists the outputs that play **somewhere other than Main**, one [`OutputSlot`] each,
//! sorted by [`OutputId`]; an output it does not list plays on Main. Every older spec has
//! none, so every older song plays exactly as it did. What an output *is* (its name, its
//! family, its channels) belongs to the instrument's definition — a kit, a pack — and is
//! copied into the slot only so a song whose pack is missing still names its strips.
//! A Pulsus kit keeps its own model (`DrumPad::output`, ADR 0100): [`OutputView`] shows
//! both through one shape.
//!
//! **The internal mixer.** Each output's level, pan, width, polarity, mute, solo, delay
//! and whether it is loaded are ordinary numeric keys of the instrument, `o{n}.<field>`
//! ([`param`]): automation, presets, undo, the agents and a plug-in export reach them as
//! every other number. Each is **null at its default** (0 dB, centre, full width, not
//! inverted, not muted, no delay, loaded): an instrument whose keys are all at their
//! defaults renders to the bit what it rendered before it had outputs.
//!
//! **Buses.** As in ADR 0100 §2: bus 0 is the track's own output; bus `k ≥ 1` is the
//! `k`-th distinct strip the slots name, in slot order ([`strips_of`], [`bus_of`]). The
//! engine carries each bus as a tap track. **The split sums to the unsplit to the bit**
//! when the instrument renders each output on its own lane and adds the lanes in output
//! order, and *Each to its own strip* places the new strips in that order with the 0 dB
//! pan law (ADR 0100 §1, §3) — the builders' tests hold it.
//!
//! Owned by `tymp-engine` (round 40 phase 3); the contract declares the shapes.

use crate::effect::{p, EffectParamInfo};
use crate::model::{Project, StripId, TrackId};
use serde::{Deserialize, Serialize};

/// The most named outputs one native instrument has (`o0` … `o31`): a 16-channel
/// DrumGizmo kit's mics, bitKlavier's nine images and a piano's noises fit.
pub const MAX_OUTPUTS: usize = 32;

/// One output of an instrument: stable for the instrument's definition (a kit's mic, a
/// pack's perspective), the `n` of its `o{n}.` keys. For a Pulsus kit's [`OutputView`] it
/// is the pad's id.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutputId(pub u16);

/// What an output carries: the mixer's family heads (*Kick, Snare, Hi-hat, Toms, Cymbals,
/// Rooms*), a piano's perspectives and noises, a pad. *Pieces to strips* gives each family
/// one strip. Names are forever once shipped (they are in the file).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputFamily {
    #[default]
    Other,
    Kick,
    Snare,
    HiHat,
    Toms,
    Cymbals,
    Overheads,
    Rooms,
    /// A piano's microphone perspective (close, player, hall, binaural…).
    Perspective,
    /// A piano's noises (hammer and release, pedal, keys, release resonance).
    Noise,
    /// A pad of a Pulsus kit (ADR 0100).
    Pad,
}

impl OutputFamily {
    pub const ALL: [OutputFamily; 11] = [
        OutputFamily::Other,
        OutputFamily::Kick,
        OutputFamily::Snare,
        OutputFamily::HiHat,
        OutputFamily::Toms,
        OutputFamily::Cymbals,
        OutputFamily::Overheads,
        OutputFamily::Rooms,
        OutputFamily::Perspective,
        OutputFamily::Noise,
        OutputFamily::Pad,
    ];

    /// The locale key of the family's head (`areas/inst_mixer`, `tymp-ui`).
    pub fn label_key(self) -> &'static str {
        match self {
            OutputFamily::Other => "inst_mixer.family.other",
            OutputFamily::Kick => "inst_mixer.family.kick",
            OutputFamily::Snare => "inst_mixer.family.snare",
            OutputFamily::HiHat => "inst_mixer.family.hihat",
            OutputFamily::Toms => "inst_mixer.family.toms",
            OutputFamily::Cymbals => "inst_mixer.family.cymbals",
            OutputFamily::Overheads => "inst_mixer.family.overheads",
            OutputFamily::Rooms => "inst_mixer.family.rooms",
            OutputFamily::Perspective => "inst_mixer.family.perspective",
            OutputFamily::Noise => "inst_mixer.family.noise",
            OutputFamily::Pad => "inst_mixer.family.pad",
        }
    }
}

/// One output that plays somewhere other than Main.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputSlot {
    pub id: OutputId,
    /// The output's name when it was routed ("Snare Top", "Close", "Pedal"): what a song
    /// whose kit or pack is missing still shows, and what carries a route over when the
    /// kit changes (matched by name, then by family).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub family: OutputFamily,
    /// The strip it plays into: one made for it (`StripKind::Pad`, *its own strip*) or one
    /// of the desk the user chose.
    pub strip: StripId,
}

/// One output as the mixer shows it, for any instrument (a Pulsus pad, a native output).
#[derive(Clone, Debug, PartialEq)]
pub struct OutputView {
    pub id: OutputId,
    pub name: String,
    pub family: OutputFamily,
    /// `None`: Main.
    pub strip: Option<StripId>,
}

/// Where an output is heard, as the mixer's **Out** cell says it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RouteView {
    /// The instrument's own strip (its track's).
    Main,
    /// A strip made for this output (`StripKind::Pad`), numbered on the desk.
    Own(StripId),
    /// A strip of the desk the user chose (another output's own strip, a bus).
    Chosen(StripId),
}

/// The one-click actions of the mixer (*All to Main · Pieces to strips · Each mic to its own
/// strip*): actions, not a mode — a cell changed afterwards is not a fourth state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OutputPlan {
    /// Every output back on Main; the strips made for them removed when nothing else uses
    /// them (ADR 0100 §4's *Merge back*).
    AllToMain,
    /// One strip per family, its outputs into it (*Pieces to strips*).
    ByFamily,
    /// One strip per output, in output order (*Each mic to its own strip*).
    EachOwn,
}

/// A field of an output's internal mixer, `o{n}.<field>`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OutputField {
    Level,
    Pan,
    Width,
    Invert,
    Mute,
    Solo,
    Delay,
    /// Loaded: off unloads the output's samples (a mic's power, ADR 0123 §4).
    On,
}

impl OutputField {
    pub const ALL: [OutputField; 8] = [
        OutputField::Level,
        OutputField::Pan,
        OutputField::Width,
        OutputField::Invert,
        OutputField::Mute,
        OutputField::Solo,
        OutputField::Delay,
        OutputField::On,
    ];

    pub fn name(self) -> &'static str {
        match self {
            OutputField::Level => "level",
            OutputField::Pan => "pan",
            OutputField::Width => "width",
            OutputField::Invert => "invert",
            OutputField::Mute => "mute",
            OutputField::Solo => "solo",
            OutputField::Delay => "delay",
            OutputField::On => "on",
        }
    }

    pub fn from_name(s: &str) -> Option<OutputField> {
        Self::ALL.into_iter().find(|f| f.name() == s)
    }

    /// Whether automation and modulators may drive it: all but *On*, which loads and
    /// unloads samples off the audio thread.
    pub fn automatable(self) -> bool {
        self != OutputField::On
    }
}

/// The templates of `o{n}.<field>` (the key printed is `o.<field>`). Every default is null.
pub const OUTPUT_PARAMS: &[EffectParamInfo] = &[
    p(
        "o.level",
        "inst_mixer.level",
        -96.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p("o.pan", "inst_mixer.pan", -1.0, 1.0, 0.0, "", false, false),
    p(
        "o.width",
        "inst_mixer.width",
        0.0,
        1.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "o.invert",
        "inst_mixer.invert",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("o.mute", "inst_mixer.mute", 0.0, 1.0, 0.0, "", false, true),
    p("o.solo", "inst_mixer.solo", 0.0, 1.0, 0.0, "", false, true),
    p(
        "o.delay",
        "inst_mixer.delay",
        0.0,
        50.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p("o.on", "inst_mixer.on", 0.0, 1.0, 1.0, "", false, true),
];

/// `o{n}.field` → `(n, field)`. Allocation-free (the engine calls it on the audio thread).
#[inline]
pub fn output_key(key: &str) -> Option<(usize, OutputField)> {
    let (n, f) = crate::inst::sampler::split_indexed(key, b'o', MAX_OUTPUTS)?;
    Some((n, OutputField::from_name(f)?))
}

/// The key of output `id`'s `field`.
pub fn key(id: OutputId, field: OutputField) -> String {
    format!("o{}.{}", id.0, field.name())
}

/// The description of an `o{n}.<field>` key, for a kind that has named outputs
/// ([`crate::InstrumentKind::has_outputs`]).
pub fn param(key: &str) -> Option<&'static EffectParamInfo> {
    let (_, f) = output_key(key)?;
    OUTPUT_PARAMS.iter().find(|i| &i.key[2..] == f.name())
}

/// The distinct strips `slots` play into, in slot order: buses 1, 2, …
pub fn strips_of(slots: &[OutputSlot]) -> Vec<StripId> {
    let mut out: Vec<StripId> = Vec::new();
    for s in slots {
        if !out.contains(&s.strip) {
            out.push(s.strip);
        }
    }
    out
}

/// The bus output `id` plays on: 0 (Main) when no slot names it, else 1 + the index of its
/// strip among [`strips_of`]. Allocation-free.
pub fn bus_of(slots: &[OutputSlot], id: OutputId) -> u8 {
    let Some(at) = slots.iter().position(|s| s.id == id) else {
        return 0;
    };
    let strip = slots[at].strip;
    let mut bus = 0u8;
    for (i, s) in slots.iter().enumerate() {
        if slots[..i].iter().any(|x| x.strip == s.strip) {
            continue;
        }
        bus = bus.saturating_add(1);
        if s.strip == strip {
            return bus;
        }
    }
    0
}

/// Where output `id` of `track` is heard. Stub of the contract: `Main` for an output no
/// slot names, `Own` for a strip made for outputs (`StripKind::Pad`), `Chosen` otherwise.
pub fn route_view(project: &Project, track: TrackId, id: OutputId) -> RouteView {
    let strip = project
        .track(track)
        .and_then(|t| t.instrument.outputs().into_iter().find(|o| o.id == id))
        .and_then(|o| o.strip);
    match strip {
        None => RouteView::Main,
        Some(s) if project.strip(s).is_some_and(crate::padout::pad_made) => RouteView::Own(s),
        Some(s) => RouteView::Chosen(s),
    }
}

/// The command that sends output `id` of `track` to `strip` (`None`: Main) — a Pulsus
/// pad's `SetPadOutput`, a native output's `SetInstrumentOutput` — or why it cannot.
pub fn set_route(
    project: &Project,
    track: TrackId,
    id: OutputId,
    strip: Option<StripId>,
) -> Result<crate::Command, crate::padout::Refusal> {
    use crate::padout::Refusal;
    let t = project.track(track).ok_or(Refusal::NotAKit)?;
    if let Some(s) = strip {
        if project.strip(s).is_none() || project.is_vca(s) {
            return Err(Refusal::Unfit);
        }
    }
    if t.instrument.pulsus().is_some() {
        return Ok(crate::Command::SetPadOutput {
            track,
            pad: crate::PadId(id.0),
            output: strip,
        });
    }
    let n = t.instrument.native().ok_or(Refusal::NotAKit)?;
    if !n.kind.has_outputs() || usize::from(id.0) >= MAX_OUTPUTS {
        return Err(Refusal::NotAKit);
    }
    // r40:fix-f — the slot keeps the output's name and family from the instrument's
    // definition (a kit's mic, a piano's perspective), so a song whose pack goes missing
    // still says what each routed output was (review of 40a27bb7, finding 9).
    let old = definition(project, track)
        .into_iter()
        .find(|o| o.id == id)
        .map(|o| (o.name, o.family))
        .or_else(|| {
            n.outputs
                .iter()
                .find(|s| s.id == id)
                .map(|o| (o.name.clone(), o.family))
        });
    let slot = strip.map(|s| OutputSlot {
        id,
        name: old.as_ref().map(|o| o.0.clone()).unwrap_or_default(),
        family: old.as_ref().map(|o| o.1).unwrap_or_default(),
        strip: s,
    });
    Ok(crate::Command::SetInstrumentOutput { track, id, slot })
}

// The one-click actions and every output's definition (`tymp-engine`).
#[path = "inst_out_plan.rs"]
mod plan_impl;
pub use plan_impl::{definition, family_name, plan, plan_with};

// ---- Finding the microphones of an imported instrument (ADR 0123 §5) ----------------

/// Which rule found an imported instrument's microphones — logged by the importer. In
/// order, the first that applies wins (research-p3 `formats.md` §1.6).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MicRule {
    /// SFZ `output=n` (or Decent Sampler's `AUX_STEREO_OUTPUT_n`): each distinct value.
    Outputs,
    /// An ARIA `<master>` (or `<group>`) with its own `amplitude_onccN`/`volume_onccN`,
    /// named by `label_ccN` (Decent Sampler: a tag bound to `AMP_VOLUME`).
    LevelCc,
    /// Sample folders or name tokens that differ only by a mic word (`/OH/`, `_room`).
    Folders,
    /// A DrumGizmo kit's channels (one multichannel file per hit).
    Channels,
    /// None of them: one output.
    Single,
}

/// One microphone an importer found.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundMic {
    pub name: String,
    pub family: OutputFamily,
    /// The CC that was its fader in the source (`amplitude_oncc80`): a Cantus fader now.
    pub cc: Option<u8>,
    /// Its starting level, dB (from `set_ccN`), 0 when the source says none.
    pub level_db: f32,
    /// A negative `amplitude` in the source: the strip's ø.
    pub invert: bool,
    /// Mono (1) or a stereo pair (2): a DrumGizmo kit's OHL/OHR are one stereo output.
    pub channels: u8,
}

/// What an importer found: the rule, the mics in output order, and each zone's (or each
/// channel's) mic index.
#[derive(Clone, Debug, PartialEq)]
pub struct MicFinding {
    pub rule: MicRule,
    pub mics: Vec<FoundMic>,
    pub of_zone: Vec<u8>,
}

/// The family a microphone's name says (`Kick In` → Kick, `OH L` → Overheads, `Room` →
/// Rooms, `Close` → Perspective, `Pedal` → Noise): English and Spanish words and the packs'
/// own names (DrumGizmo's `KDrumInside`, `FTom1`, `AmbL`; Gogodze's `front`, `side`,
/// `wndw`, `retro`; bitKlavier's `Decca Tail`, `Binaural`). Unknown: [`OutputFamily::Other`].
pub fn family_of_name(name: &str) -> OutputFamily {
    let lower = name.to_lowercase();
    // Every letter and digit run together, and the words (split at anything else and at a
    // lower-to-upper change: `KDrumInside` → k, drum, inside).
    let compact: String = lower.chars().filter(|c| c.is_alphanumeric()).collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if (!c.is_alphanumeric()
            || (c.is_uppercase() && prev_lower)
            || (c.is_ascii_digit()
                && !cur.is_empty()
                && !cur.chars().last().is_some_and(|l| l.is_ascii_digit())))
            && !cur.is_empty()
        {
            tokens.push(std::mem::take(&mut cur));
        }
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
        }
        prev_lower = c.is_lowercase();
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    let has = |words: &[&str]| words.iter().any(|w| compact.contains(w));
    let token = |words: &[&str]| tokens.iter().any(|t| words.contains(&t.as_str()));
    if has(&["kick", "kdrum", "bassdrum", "bombo"]) || token(&["bd", "kik"]) {
        return OutputFamily::Kick;
    }
    if has(&["snare", "caja", "redoblante", "tarola"]) || token(&["sd", "sn"]) {
        return OutputFamily::Snare;
    }
    if has(&["hihat", "hat", "charles", "contratiempo"]) || token(&["hh"]) {
        return OutputFamily::HiHat;
    }
    let toms = compact.replace("bottom", "");
    if toms.contains("tom") || has(&["floor", "timbal", "goliat"]) {
        return OutputFamily::Toms;
    }
    if has(&[
        "ride", "crash", "splash", "china", "cymbal", "plato", "platillo", "címbalo", "cimbalo",
    ]) {
        return OutputFamily::Cymbals;
    }
    if has(&["overhead", "aereo", "aéreo", "cenital"]) || token(&["oh", "ohl", "ohr", "side"]) {
        return OutputFamily::Overheads;
    }
    if has(&[
        "room", "amb", "sala", "front", "window", "wndw", "retro", "far", "hall", "vintage",
        "lejano",
    ]) {
        return OutputFamily::Rooms;
    }
    if has(&[
        "pedal",
        "release",
        "noise",
        "hammer",
        "mechanic",
        "ruido",
        "mecánica",
        "mecanica",
        "martillo",
        "teclas",
    ]) {
        return OutputFamily::Noise;
    }
    if has(&[
        "close",
        "player",
        "audience",
        "decca",
        "ortf",
        "binaural",
        "spaced",
        "stage",
        "lid",
        "pianobar",
        "harp",
        "house",
        "cercan",
        "intérprete",
        "interprete",
        "público",
        "publico",
        "perspectiv",
    ]) || token(&["ab", "xy", "ms", "in", "rm"])
    {
        return OutputFamily::Perspective;
    }
    OutputFamily::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_keys_parse_and_every_default_is_null() {
        assert_eq!(output_key("o0.level"), Some((0, OutputField::Level)));
        assert_eq!(output_key("o31.on"), Some((31, OutputField::On)));
        assert_eq!(output_key("o32.on"), None);
        assert_eq!(output_key("o01.on"), None);
        assert_eq!(output_key("o3.nope"), None);
        assert_eq!(output_key("osc1.table"), None);
        for f in OutputField::ALL {
            let k = key(OutputId(7), f);
            assert_eq!(output_key(&k), Some((7, f)), "{k}");
            let info = param(&k).expect("described");
            let null = match f {
                OutputField::Width | OutputField::On => 1.0,
                _ => 0.0,
            };
            assert_eq!(info.default, null, "{k}");
        }
    }

    #[test]
    fn a_mics_name_says_its_family_in_english_spanish_and_the_packs_words() {
        use OutputFamily as F;
        for (name, f) in [
            ("Kick In", F::Kick),
            ("KDrumInside", F::Kick),
            ("Kick Sub", F::Kick),
            ("kick_mic", F::Kick),
            ("Bombo", F::Kick),
            ("Snare Top", F::Snare),
            ("SnareBottom", F::Snare),
            ("Caja abajo", F::Snare),
            ("Hihat", F::HiHat),
            ("HH", F::HiHat),
            ("Tom1", F::Toms),
            ("FTom2", F::Toms),
            ("Floor 1", F::Toms),
            ("Ride", F::Cymbals),
            ("OH L", F::Overheads),
            ("OHR", F::Overheads),
            ("oh_mic", F::Overheads),
            ("side_mic", F::Overheads),
            ("Overheads", F::Overheads),
            ("AmbL", F::Rooms),
            ("Room Near", F::Rooms),
            ("wndw_mic", F::Rooms),
            ("front_mic", F::Rooms),
            ("retro_mic", F::Rooms),
            ("Sala", F::Rooms),
            ("Pedalnoise", F::Noise),
            ("Decca Tail", F::Perspective),
            ("Binaural", F::Perspective),
            ("Close", F::Perspective),
            ("Bass", F::Other),
        ] {
            assert_eq!(family_of_name(name), f, "{name}");
        }
    }

    #[test]
    fn buses_count_distinct_strips_in_slot_order() {
        let slot = |id, s| OutputSlot {
            id: OutputId(id),
            name: String::new(),
            family: OutputFamily::Other,
            strip: StripId(s),
        };
        let slots = [slot(1, 10), slot(2, 11), slot(4, 10), slot(6, 12)];
        assert_eq!(
            strips_of(&slots),
            vec![StripId(10), StripId(11), StripId(12)]
        );
        assert_eq!(bus_of(&slots, OutputId(0)), 0);
        assert_eq!(bus_of(&slots, OutputId(1)), 1);
        assert_eq!(bus_of(&slots, OutputId(2)), 2);
        assert_eq!(bus_of(&slots, OutputId(4)), 1);
        assert_eq!(bus_of(&slots, OutputId(6)), 3);
    }

    /// r40:fix-f (finding 9): an output sent from Main to a strip keeps its name and family
    /// in the song's slot, from the kit's definition; a VCA is refused.
    #[test]
    fn a_route_from_main_keeps_the_outputs_name_and_a_vca_is_refused() {
        use crate::{InstrumentKind, InstrumentSpec, NativeSpec, Project};
        let mut p = Project::new("t");
        let mut t = p.make_track("Drums");
        t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::AcousticDrums));
        let id = t.id;
        p.tracks.push(t);
        let to = p.strips[3].id;
        let def = crate::inst::tympanum_kit::modelled_kit("studio").unwrap();
        let snare = def.mics.iter().find(|m| m.name == "Snare Top").unwrap();
        let cmd = set_route(&p, id, snare.output, Some(to)).unwrap();
        let crate::Command::SetInstrumentOutput {
            slot: Some(slot), ..
        } = cmd
        else {
            panic!("{cmd:?}");
        };
        assert_eq!(slot.name, "Snare Top");
        assert_eq!(slot.family, OutputFamily::Snare);
        p.strips[4].kind = crate::vca::StripKind::Vca;
        let vca = p.strips[4].id;
        assert!(p.is_vca(vca));
        assert!(set_route(&p, id, snare.output, Some(vca)).is_err());
    }
}
