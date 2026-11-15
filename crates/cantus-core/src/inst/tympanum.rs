//! Tympanum — the acoustic drummer (round 40 phase 3, ADR 0124): its parameter model.
//! Owned by `tymp-engine`; the contract declares the shapes and the key families.
//!
//! A native instrument (`InstrumentKind::AcousticDrums`) over **Pulsus's engine**: a kit of
//! pieces (kick, snare, hats, toms, cymbals), each with its articulations on their keys,
//! heard through microphones — **modelled** kits (Pulsus's modal voices, virtual mics with
//! a position, an aim and a pattern, which can move) and **recorded** multi-mic kits
//! (packs: DrumGizmo, SFZ). Every microphone is a named output (ADR 0123): its internal
//! mixer is the `o{n}.` keys, its route the spec's `outputs`. Pulsus stays the pad drum
//! machine.
//!
//! **What the project holds**: which kit (the text key [`KIT_KEY`], a [`KitRef`]), and only
//! what moved from the kit's own values — numbers by key, sparse as every native spec. What
//! the kit *is* (its pieces, articulations, mics, positions, default bleed) is the kit's
//! definition ([`super::tympanum_kit::KitDef`]): Cantus's tables for a modelled kit, the
//! pack's files for a recorded one — so a pack's update never changes a song's mix, and a
//! song names nothing it does not use.
//!
//! | Group | Keys |
//! |---|---|
//! | Kit | `model` (stored with every instance), `seed`, `gain` (dB), `humanize` (%), `hat.cc` (the openness controller, 4) |
//! | Room | `room` (0 the kit's own — a recorded kit's, the default — 1 Small, 2 Studio, 3 Hall), `amb.level` (dB, the room mics together), `amb.size` (%), `amb.decay` (%), `bleed` (dB: every mic's bleed at once, DrumGizmo's fader; a mic's own pieces untouched) |
//! | Piece `p{n}.` | `tune` (st), `damp` (%), `level` (dB), `wires` (% — a snare's), `mute` (0/1) |
//! | Mic `m{n}.` (modelled kits: the mic moves) | `x`, `y`, `z` (cm from its place), `az`, `el` (° of aim), `pattern` (a [`MicPattern`]) |
//! | Bleed `b{p}.{m}.` (piece × mic) | `db` (dB), `inv` (0/1), `on` (0/1: off unloads that cell's samples) |
//! | Outputs `o{n}.` | ADR 0123 (`crate::inst_out`) |
//! | Text | `kit` (a [`KitRef`]), `voice` (`hybrid` or absent: a modelled kit's [`Voice`]) |
//!
//! Every default is null: a key at its default changes no sample.

use super::{find, InstParamInfo};

// r40:tymp-engine — Tympanum's edits as batches of every native instrument's commands.
#[path = "tympanum_cmd.rs"]
pub mod cmd;
// r40:drums-hyb — a modelled kit's voice: generated or hybrid (ADR 0124's third amendment).
#[path = "tympanum_voice.rs"]
pub mod voice;
use crate::effect::p;
use crate::model::TrackIcon;
pub use voice::{Voice, VOICE_KEY};

pub const NAME_KEY: &str = "instrument.tympanum";
pub const ICON: TrackIcon = TrackIcon::Drums;
/// In the picker since round 40's close (the owner: *"Add Tympanum at once"*, 2026-11-14);
/// its sound waits for his blind listening (`docs/BACKLOG-excellence.md`).
pub const READY: bool = true;

/// The model a new instance plays (stored with every instance, ADR 0059).
pub const MODEL: f32 = 1.0;

/// The text key naming the kit (a [`KitRef`] in its text form).
pub const KIT_KEY: &str = "kit";

pub const MAX_PIECES: usize = 32;
pub const MAX_MICS: usize = crate::inst_out::MAX_OUTPUTS;
/// Articulations one piece has at most (centre, edge, rim shot, side stick, rim, flam, roll,
/// buzz; a hi-hat's openness counts once — it is continuous).
pub const MAX_ARTICULATIONS: usize = 12;

/// Set, never automated: the model; the room's choice and the openness controller.
/// r40:tymp-engine — and the room's surfaces, the take rule and the foot's chick.
pub const SET_KEYS: [&str; 8] = [
    "model",
    "room",
    "hat.cc",
    "floor",
    "walls",
    "rug",
    "vary",
    "hat.chick",
];

pub const PARAMS: &[InstParamInfo] = &[
    p("model", "tympanum.model", 1.0, 1.0, MODEL, "", false, true),
    p("seed", "tympanum.seed", 0.0, 65535.0, 0.0, "", false, true),
    p(
        "gain",
        "tympanum.gain",
        -24.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "humanize",
        "tympanum.humanize",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "hat.cc",
        "tympanum.hat_cc",
        0.0,
        127.0,
        4.0,
        "",
        false,
        true,
    ),
    p("room", "tympanum.room", 0.0, 3.0, 0.0, "", false, true),
    p(
        "amb.level",
        "tympanum.amb_level",
        -96.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "amb.size",
        "tympanum.amb_size",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "amb.decay",
        "tympanum.amb_decay",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "bleed",
        "tympanum.bleed",
        -96.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    // r40:tymp-engine — the *Feel* card: each hit up to ± `timing` ms off the grid
    // (seeded, the same every render); `vary` 1 never plays the take before on the key,
    // 0 draws freely.
    p(
        "timing",
        "tympanum.timing",
        0.0,
        20.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p(
        "vary",
        "tympanum.never_twice",
        0.0,
        1.0,
        1.0,
        "",
        false,
        true,
    ),
    // The foot's chick when the pedal closes a ringing hat (1 on).
    p(
        "hat.chick",
        "tympanum.foot_chick",
        0.0,
        1.0,
        1.0,
        "",
        false,
        true,
    ),
    // The *Kit* card: every piece at once — tuned (semitones, on top of `p{n}.tune`) and
    // ringing longer or shorter (a modelled kit's decays times `ring`).
    p(
        "tune",
        "tympanum.kit_tune",
        -12.0,
        12.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "ring",
        "tympanum.kit_ring",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    // The kick's beater: 0 felt (the kit's own) … 1 a hard plastic beater (a shorter
    // contact, a brighter click).
    p(
        "beater",
        "tympanum.beater",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    // The room's surfaces (0 the room's own): a modelled room's reflections follow them.
    p(
        "floor",
        "tympanum.room.floor",
        0.0,
        8.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "walls",
        "tympanum.room.walls",
        0.0,
        10.0,
        0.0,
        "",
        false,
        true,
    ),
    p("rug", "tympanum.room.rug", 0.0, 4.0, 0.0, "", false, true),
];

/// `p{n}.<field>`: a piece's sound.
pub const PIECE_PARAMS: &[InstParamInfo] = &[
    p(
        "p.tune",
        "tympanum.tune",
        -12.0,
        12.0,
        0.0,
        "st",
        false,
        false,
    ),
    p("p.damp", "tympanum.damp", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "p.level",
        "tympanum.level",
        -96.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "p.wires",
        "tympanum.wires",
        0.0,
        2.0,
        1.0,
        "%",
        false,
        false,
    ),
    p("p.mute", "tympanum.mute", 0.0, 1.0, 0.0, "", false, true),
    // r40:tymp-engine — where the stick lands: `p.pos` moves a modelled piece's strike
    // point from its own (−1 the centre … 1 the edge), `p.spread` how far each hit strays
    // from it (the model's own scatter at its default).
    p(
        "p.pos",
        "tympanum.from_centre",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "p.spread",
        "tympanum.spread",
        0.0,
        1.0,
        SPREAD,
        "%",
        false,
        false,
    ),
];

/// A modelled piece's own scatter of the strike point (`p{n}.spread`'s default).
pub const SPREAD: f32 = 0.06;

/// `m{n}.<field>`: where a modelled kit's mic stands and points (offsets from its place).
pub const MIC_PARAMS: &[InstParamInfo] = &[
    p(
        "m.x",
        "tympanum.mic_x",
        -300.0,
        300.0,
        0.0,
        "cm",
        false,
        false,
    ),
    p(
        "m.y",
        "tympanum.mic_y",
        -300.0,
        300.0,
        0.0,
        "cm",
        false,
        false,
    ),
    p(
        "m.z",
        "tympanum.mic_z",
        -300.0,
        300.0,
        0.0,
        "cm",
        false,
        false,
    ),
    p(
        "m.az",
        "tympanum.mic_az",
        -180.0,
        180.0,
        0.0,
        "°",
        false,
        false,
    ),
    p(
        "m.el",
        "tympanum.mic_el",
        -90.0,
        90.0,
        0.0,
        "°",
        false,
        false,
    ),
    // 0 the kit's own pattern for this mic, else a `MicPattern` index + 1.
    p(
        "m.pattern",
        "tympanum.mic_pattern",
        0.0,
        5.0,
        0.0,
        "",
        false,
        true,
    ),
];

/// `b{p}.{m}.<field>`: one cell of the piece × mic bleed matrix, relative to the kit's.
pub const BLEED_PARAMS: &[InstParamInfo] = &[
    p(
        "b.db",
        "tympanum.bleed_db",
        -96.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "b.inv",
        "tympanum.bleed_inv",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("b.on", "tympanum.bleed_on", 0.0, 1.0, 1.0, "", false, true),
];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

pub const SOURCES: &[&str] = &[
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "vel", "key",
    "mw", "at", "expr",
];

/// A microphone's polar pattern.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MicPattern {
    Omni,
    Cardioid,
    Supercardioid,
    Hypercardioid,
    Figure8,
}

impl MicPattern {
    pub const ALL: [MicPattern; 5] = [
        MicPattern::Omni,
        MicPattern::Cardioid,
        MicPattern::Supercardioid,
        MicPattern::Hypercardioid,
        MicPattern::Figure8,
    ];
}

/// The room a kit plays in (`room`): a recorded kit's own by default; Cantus's rooms (Aula's
/// responses) on the room mics as the option the owner asked for.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum RoomChoice {
    #[default]
    Own,
    Small,
    Studio,
    Hall,
}

impl RoomChoice {
    pub const ALL: [RoomChoice; 4] = [
        RoomChoice::Own,
        RoomChoice::Small,
        RoomChoice::Studio,
        RoomChoice::Hall,
    ];
}

/// Which kit a Tympanum plays, as the text key [`KIT_KEY`] holds it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum KitRef {
    /// One of Cantus's modelled kits, by its id (`model:studio-maple`).
    Modelled(String),
    /// A kit inside an installed pack (`pack:crocellkit/kit`): the pack's id and the pack
    /// instrument's id (`cantus_content::PackInstrument`).
    Pack { pack: String, kit: String },
}

impl KitRef {
    /// The kit a new Tympanum plays (a modelled kit: instant, no download).
    pub fn default_kit() -> KitRef {
        KitRef::Modelled("studio".into())
    }

    pub fn parse(text: &str) -> Option<KitRef> {
        if let Some(id) = text.strip_prefix("model:") {
            return (!id.is_empty()).then(|| KitRef::Modelled(id.to_string()));
        }
        let rest = text.strip_prefix("pack:")?;
        let (pack, kit) = rest.split_once('/')?;
        (!pack.is_empty() && !kit.is_empty()).then(|| KitRef::Pack {
            pack: pack.to_string(),
            kit: kit.to_string(),
        })
    }

    pub fn text(&self) -> String {
        match self {
            KitRef::Modelled(id) => format!("model:{id}"),
            KitRef::Pack { pack, kit } => format!("pack:{pack}/{kit}"),
        }
    }

    /// The kit a spec names: its text key, or the default kit when it names none.
    pub fn of(spec: &super::NativeSpec) -> KitRef {
        spec.text(KIT_KEY)
            .and_then(KitRef::parse)
            .unwrap_or_else(KitRef::default_kit)
    }
}

/// `b{p}.{m}.field` → `(p, m, field)`. Allocation-free.
pub fn bleed_key(key: &str) -> Option<(usize, usize, &str)> {
    let (piece, rest) = super::sampler::split_indexed(key, b'b', MAX_PIECES)?;
    let dot = rest.find('.')?;
    let (m, field) = (&rest[..dot], &rest[dot + 1..]);
    let n: usize = m.parse().ok()?;
    let canonical = n.to_string() == m;
    (canonical && n < MAX_MICS && !field.is_empty()).then_some((piece, n, field))
}

fn template(list: &'static [InstParamInfo], field: &str) -> Option<&'static InstParamInfo> {
    list.iter()
        .find(|i| i.key.len() == field.len() + 2 && &i.key[2..] == field)
}

pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    if let Some((_, field)) = super::sampler::split_indexed(key, b'p', MAX_PIECES) {
        return template(PIECE_PARAMS, field);
    }
    if let Some((_, field)) = super::sampler::split_indexed(key, b'm', MAX_MICS) {
        return template(MIC_PARAMS, field);
    }
    if let Some((_, _, field)) = bleed_key(key) {
        return template(BLEED_PARAMS, field);
    }
    find(PARAMS, key).or_else(|| super::modulation::param(key))
}

pub fn is_text_key(key: &str) -> bool {
    key == KIT_KEY
        || key == VOICE_KEY
        || super::modulation::is_text_key(key)
        // r40:tymp-ui — the room's look (schema 69, ADR 0126): picture only, never heard.
        || super::room_look::KEYS.contains(&key)
}

pub fn is_asset_key(key: &str) -> bool {
    let _ = key;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_parse_and_every_default_sits_in_its_range() {
        for list in [PARAMS, PIECE_PARAMS, MIC_PARAMS, BLEED_PARAMS] {
            for i in list {
                assert!(i.min <= i.default && i.default <= i.max, "{}", i.key);
            }
        }
        assert_eq!(param("p3.tune").unwrap().key, "p.tune");
        assert_eq!(param("m0.az").unwrap().key, "m.az");
        assert_eq!(param("b4.12.db").unwrap().key, "b.db");
        assert!(param("b4.012.db").is_none());
        assert!(param("b4.32.db").is_none());
        assert_eq!(param("amb.level").unwrap().default, 0.0);
        assert!(is_text_key(KIT_KEY));
        for r in [
            KitRef::default_kit(),
            KitRef::Pack {
                pack: "crocellkit".into(),
                kit: "kit".into(),
            },
        ] {
            assert_eq!(KitRef::parse(&r.text()), Some(r));
        }
        assert_eq!(KitRef::parse("pack:x"), None);
    }
}
