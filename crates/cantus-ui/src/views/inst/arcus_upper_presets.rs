//! r42:arcus-upper — Arcus's violin and viola presets (round 42, `docs/round42/arcus-upper.md`):
//! solo lines cantabile and plain, the contact point from sul tasto to near the bridge,
//! détaché, pizzicato, tremolo, portamento — the cello's four macros on each (the expression,
//! where the bow plays, the body, the vibrato). Their levels are fitted to their roles
//! (`tools/arcus/presets.sh <dir> fit`, the cello's tool, which writes this file's gains too); their sound is the owner's to judge.

use crate::views::inst::FactoryPreset;

/// The four macros, the cello's (`arcus_presets.rs`).
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

const VIOLIN: &[&str] = &[
    "preset.role.lead",
    "arcus.tag.violin",
    "preset.style.film",
    "preset.style.worship",
];
const VIOLA: &[&str] = &[
    "preset.role.lead",
    "arcus.tag.viola",
    "preset.style.film",
    "preset.style.worship",
];
const PLUCKED: &[&str] = &["preset.role.pluck", "arcus.tag.pizz", "preset.style.film"];

// inst: 2 Violin · 3 Viola. tech: 0 Arco · 1 Pizzicato · 2 Tremolo. mode: 0 Poly · 1 Mono ·
// 2 Legato.
pub const PRESETS: &[FactoryPreset] = &[
    // ---- the violin ----
    preset!("arcus.preset.violin", VIOLIN, ["inst" => 2.0, "gain" => 0.0]),
    preset!("arcus.preset.violin_cantabile", VIOLIN,
        ["inst" => 2.0, "vib.depth" => 1.2, "vib.delay" => 160.0, "vib.attack" => 320.0,
         "bow.pos" => 1.09, "bow.attack" => 1.4, "dyn" => 0.65, "body.ring" => 1.15,
         "gain" => 0.0]),
    preset!("arcus.preset.violin_tasto", VIOLIN,
        ["inst" => 2.0, "bow.pos" => 1.34, "bow.force" => 0.81, "bow.speed" => 0.8,
         "vib.depth" => 0.8, "bow.attack" => 1.8, "gain" => 0.0]),
    preset!("arcus.preset.violin_plain", VIOLIN,
        ["inst" => 2.0, "vib.depth" => 0.0, "bow.noise" => 3.0, "gain" => 0.0]),
    preset!("arcus.preset.violin_detache", VIOLIN,
        ["inst" => 2.0, "mode" => 1.0, "bow.release" => 60.0, "bow.change" => 16.0,
         "vib.delay" => 110.0, "gain" => 0.0]),
    preset!("arcus.preset.violin_portamento", VIOLIN,
        ["inst" => 2.0, "porta" => 140.0, "vib.depth" => 1.15, "gain" => 0.0]),
    preset!("arcus.preset.violin_pizz", PLUCKED,
        ["inst" => 2.0, "tech" => 1.0, "mode" => 0.0, "pizz.decay" => 1.0, "ring" => 0.4,
         "gain" => 0.0]),
    preset!("arcus.preset.violin_tremolo", VIOLIN,
        ["inst" => 2.0, "tech" => 2.0, "trem.rate" => 15.0, "vib.depth" => 0.4,
         "gain" => 0.0]),
    // ---- the viola ----
    preset!("arcus.preset.viola", VIOLA, ["inst" => 3.0, "gain" => 0.0]),
    preset!("arcus.preset.viola_cantabile", VIOLA,
        ["inst" => 3.0, "vib.depth" => 1.2, "vib.delay" => 180.0, "vib.attack" => 340.0,
         "bow.pos" => 1.08, "bow.attack" => 1.5, "dyn" => 0.62, "body.ring" => 1.2,
         "gain" => 0.0]),
    preset!("arcus.preset.viola_tasto", VIOLA,
        ["inst" => 3.0, "bow.pos" => 1.34, "bow.force" => 0.81, "bow.speed" => 0.8,
         "vib.depth" => 0.75, "bow.attack" => 1.8, "gain" => 0.0]),
    preset!("arcus.preset.viola_detache", VIOLA,
        ["inst" => 3.0, "mode" => 1.0, "bow.release" => 60.0, "bow.change" => 18.0,
         "vib.delay" => 120.0, "gain" => 0.0]),
    preset!("arcus.preset.viola_pizz", PLUCKED,
        ["inst" => 3.0, "tech" => 1.0, "mode" => 0.0, "pizz.decay" => 1.2, "ring" => 0.5,
         "gain" => 0.0]),
    preset!("arcus.preset.viola_tremolo", VIOLA,
        ["inst" => 3.0, "tech" => 2.0, "trem.rate" => 14.0, "vib.depth" => 0.4,
         "gain" => 0.0]),
];

/// The cello's and the bass's presets, then these (one list, `arcus::FACTORY_PRESETS`).
pub const fn joined<const N: usize>(
    a: &[FactoryPreset],
    b: &[FactoryPreset],
) -> [FactoryPreset; N] {
    let mut out = [a[0]; N];
    let mut i = 0;
    while i < N {
        out[i] = if i < a.len() { a[i] } else { b[i - a.len()] };
        i += 1;
    }
    out
}
