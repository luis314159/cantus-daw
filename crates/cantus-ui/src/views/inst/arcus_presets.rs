//! Arcus's factory presets (round 42, ADR 0157, ADR 0101's floor of 24): the cello and the
//! double bass solo — sustained lines, cantabile and plain, the contact point from sul tasto
//! to near the bridge, détaché and marcato, pizzicato orchestral and ringing, tremolo,
//! portamento, double stops, MPE. Four macros on each, a new Arcus's: 1 the expression (the
//! dynamic), 2 where the bow plays, 3 the body, 4 the vibrato. Their levels are fitted to their roles
//! (`tools/arcus/presets.sh fit`: 0.0 LU from the role's target where the gain's range
//! and the −1.2 dBFS phrase peak allow); their sound is the owner's to judge.

use crate::views::inst::FactoryPreset;

/// The four macros. Their names are locale keys (`arcus.macro.*`).
const MACROS: &[(&str, &str)] = &[
    ("macro1.name", "arcus.macro.expression"),
    ("mod1.src", "macro1"),
    ("mod1.dst", "dyn"),
    ("macro2.name", "arcus.macro.contact"),
    ("mod2.src", "macro2"),
    ("mod2.dst", "bow.pos"),
    ("macro3.name", "arcus.macro.body"),
    ("mod3.src", "macro3"),
    ("mod3.dst", "body.mix"),
    ("macro4.name", "arcus.macro.vibrato"),
    ("mod4.src", "macro4"),
    ("mod4.dst", "vib.depth"),
];

macro_rules! preset {
    ($label:literal, $tags:expr, [$($k:literal => $v:expr),* $(,)?]) => {
        FactoryPreset {
            label_key: $label,
            values: &[
                ("mod1.amt", 0.4),
                ("mod2.amt", 0.5),
                ("mod3.amt", -0.4),
                ("mod4.amt", 0.5),
                $(($k, $v)),*
            ],
            text: MACROS,
            tags: $tags,
        }
    };
}

const CELLO: &[&str] = &[
    "preset.role.lead",
    "arcus.tag.cello",
    "preset.style.film",
    "preset.style.worship",
];
const BASS: &[&str] = &[
    "preset.role.bass",
    "arcus.tag.bass",
    "preset.style.film",
    "preset.style.jazz",
];
const PLUCKED: &[&str] = &[
    "preset.role.pluck",
    "arcus.tag.pizz",
    "preset.style.film",
    "preset.style.jazz",
];

// inst: 0 Cello · 1 Double bass. tech: 0 Arco · 1 Pizzicato · 2 Tremolo. mode: 0 Poly ·
// 1 Mono · 2 Legato.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    // ---- the cello ----
    preset!("arcus.preset.cello", CELLO, ["gain" => 2.8]),
    preset!("arcus.preset.cello_cantabile", CELLO,
        ["vib.depth" => 1.25, "vib.delay" => 180.0, "vib.attack" => 350.0, "bow.pos" => 1.09,
         "bow.attack" => 1.5, "dyn" => 0.65, "body.ring" => 1.2, "gain" => 4.1]),
    preset!("arcus.preset.cello_tasto", CELLO,
        ["bow.pos" => 1.34, "bow.force" => 0.81, "bow.speed" => 0.8, "vib.depth" => 0.8,
         "bow.attack" => 1.8, "gain" => 6.2]),
    preset!("arcus.preset.cello_bridge", CELLO,
        ["bow.pos" => 0.76, "bow.force" => 1.19, "vib.depth" => 0.9, "gain" => 1.6]),
    preset!("arcus.preset.cello_plain", CELLO,
        ["vib.depth" => 0.0, "bow.noise" => 3.0, "gain" => 2.4]),
    preset!("arcus.preset.cello_detache", CELLO,
        ["mode" => 1.0, "bow.release" => 60.0, "bow.change" => 18.0, "vib.delay" => 120.0,
         "gain" => 2.9]),
    preset!("arcus.preset.cello_marcato", CELLO,
        ["mode" => 1.0, "vel.attack" => 0.9, "bow.attack" => 0.5, "bow.force" => 1.15,
         "bow.release" => 80.0, "gain" => 3.0]),
    preset!("arcus.preset.cello_portamento", CELLO,
        ["porta" => 160.0, "vib.depth" => 1.15, "gain" => 2.9]),
    preset!("arcus.preset.cello_double", CELLO,
        ["mode" => 0.0, "poly" => 3.0, "vib.depth" => 0.85, "gain" => 2.2]),
    preset!("arcus.preset.cello_pizz", PLUCKED,
        ["tech" => 1.0, "mode" => 0.0, "pizz.decay" => 1.0, "ring" => 0.4, "gain" => -4.1]),
    preset!("arcus.preset.cello_pizz_ring", PLUCKED,
        ["tech" => 1.0, "mode" => 0.0, "pizz.decay" => 2.0, "pizz.soft" => 1.8, "ring" => 1.5,
         "body.ring" => 1.3, "gain" => -3.8]),
    preset!("arcus.preset.cello_tremolo", CELLO,
        ["tech" => 2.0, "trem.rate" => 14.0, "vib.depth" => 0.4, "gain" => 4.4]),
    preset!("arcus.preset.cello_slow", CELLO,
        ["bow.speed" => 0.6, "bow.force" => 1.23, "bow.friction" => 1.0, "vib.depth" => 0.9,
         "gain" => 7.6]),
    preset!("arcus.preset.cello_mpe", CELLO,
        ["mode" => 0.0, "mpe.press" => 1.0, "mpe.slide" => 1.0, "vib.src" => 3.0, "gain" => 2.3]),
    // ---- the double bass ----
    preset!("arcus.preset.bass", BASS, ["inst" => 1.0, "gain" => 0.0]),
    preset!("arcus.preset.bass_cantabile", BASS,
        ["inst" => 1.0, "vib.depth" => 1.25, "vib.delay" => 200.0, "bow.pos" => 1.07,
         "bow.attack" => 1.4, "gain" => 1.1]),
    preset!("arcus.preset.bass_tasto", BASS,
        ["inst" => 1.0, "bow.pos" => 1.34, "bow.force" => 0.81, "bow.speed" => 0.8,
         "vib.depth" => 0.7, "gain" => 3.6]),
    preset!("arcus.preset.bass_plain", BASS,
        ["inst" => 1.0, "vib.depth" => 0.0, "gain" => 0.0]),
    preset!("arcus.preset.bass_detache", BASS,
        ["inst" => 1.0, "mode" => 1.0, "bow.release" => 70.0, "bow.change" => 22.0,
         "gain" => 0.3]),
    preset!("arcus.preset.bass_marcato", BASS,
        ["inst" => 1.0, "mode" => 1.0, "vel.attack" => 0.9, "bow.attack" => 0.5,
         "bow.force" => 1.15, "gain" => 0.7]),
    preset!("arcus.preset.bass_portamento", BASS,
        ["inst" => 1.0, "porta" => 200.0, "gain" => 0.0]),
    preset!("arcus.preset.bass_pizz", PLUCKED,
        ["inst" => 1.0, "tech" => 1.0, "mode" => 0.0, "pizz.decay" => 1.0, "ring" => 0.5,
         "gain" => -6.7]),
    preset!("arcus.preset.bass_pizz_jazz", PLUCKED,
        ["inst" => 1.0, "tech" => 1.0, "mode" => 0.0, "pizz.pos" => 0.3, "pizz.soft" => 2.2,
         "pizz.decay" => 2.2, "ring" => 1.2, "gain" => -6.7]),
    preset!("arcus.preset.bass_tremolo", BASS,
        ["inst" => 1.0, "tech" => 2.0, "trem.rate" => 12.0, "vib.depth" => 0.3, "gain" => 4.5]),
];
