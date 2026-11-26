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
    preset!("arcus.preset.violin", VIOLIN, ["inst" => 2.0, "ring" => 1.6, "gain" => 5.9]),
    preset!("arcus.preset.violin_cantabile", VIOLIN,
        ["inst" => 2.0, "vib.depth" => 1.2, "vib.delay" => 160.0, "vib.attack" => 320.0,
         "bow.pos" => 1.09, "bow.attack" => 1.4, "dyn" => 0.65, "body.ring" => 1.15,
         "gain" => 6.6]),
    preset!("arcus.preset.violin_tasto", VIOLIN,
        ["inst" => 2.0, "bow.pos" => 1.34, "bow.force" => 0.81, "bow.speed" => 0.8,
         "vib.depth" => 0.8, "bow.attack" => 1.8, "gain" => 9.7]),
    preset!("arcus.preset.violin_plain", VIOLIN,
        ["inst" => 2.0, "vib.depth" => 0.0, "bow.noise" => 3.0, "gain" => 6.2]),
    preset!("arcus.preset.violin_detache", VIOLIN,
        ["inst" => 2.0, "mode" => 1.0, "bow.release" => 60.0, "bow.change" => 16.0,
         "vib.delay" => 110.0, "gain" => 5.9]),
    preset!("arcus.preset.violin_portamento", VIOLIN,
        ["inst" => 2.0, "porta" => 140.0, "vib.depth" => 1.15, "gain" => 5.9]),
    // r42 arcus-expr — the fiddle (country, bluegrass; `docs/round42/arcus-expr.md` §5): the
    // bow nearer the bridge and heavier, more grit, short strokes, little vibrato and late,
    // quick fingers and changes, the open strings left ringing.
    preset!("arcus.preset.violin_fiddle", VIOLIN,
        // r42 fiddle, the owner's "too scratchy": no flat +4 dB of hair noise (`bow.noise`
        // 4 → 0) and the grit at 0.75 — at the bow's heavy, near-bridge contact the hair's
        // roughness took the string out of Helmholtz motion (YIN d' 0.15–0.19, 35–44 % of the
        // frames raucous, the 4–8 kHz noise between the harmonics −25 dB re the level against
        // the references' −37/−38); now 0.01–0.03, 6–11 %, −37 … −38 dB
        // (`docs/round42/fiddle.md` §6). The grit is the loudest notes' (take A); the bite
        // at 0.5 gives each note its share by the dynamic — the soft ones none (take B), the
        // ordinary ones about half (take C, the owner's middle), the accents all of it.
        ["inst" => 2.0, "bow.pos" => 0.73, "bow.force" => 1.32,
         "bow.attack" => 0.7, "bow.release" => 70.0, "bow.change" => 14.0, "finger" => 14.0,
         "vib.depth" => 0.35, "vib.delay" => 450.0, "vib.var" => 0.35, "ring" => 1.8,
         "body.ring" => 0.9, "gain" => 5.4,
         // r42 fiddle (ADR 0164): one bow on two strings, the open strings in sympathy, the
         // grit, a steel set, double stops tuned most of the way to just.
         "bow.pair" => 1.0, "sym" => 1.0, "bow.grit" => 0.75, "steel" => 1.0,
         "bow.bite" => 0.5, "dstop.just" => 0.6,
         // … and one close microphone, as a fiddle is miked (the pair's mono sum was dark);
         // r42 fiddle: the bow nearer the bridge (0.82 → 0.73) and heavier (1.15 → 1.32) — in
         // the lab the fast notes' re-articulation 0.48 → 0.17 of the onsets (the references'
         // 0.10–0.17), the steady noise between the harmonics −16.5 → −10.7 dB.
         "body.mic" => 1.0,
         // … through a fiddle's bridge: its hill raised, its top falling sooner (the stems'
         // 1.6–3.2 kHz −15 … −17 → −12.7 dB re the total, the bluegrass references' −11 … −13);
         // at 1.3 since the scratch went (the hair's noise had carried part of the top: the
         // bite back +0.6 dB, 5 kHz −0.6 dB more).
         "body.bridge" => 1.3]),
    preset!("arcus.preset.violin_pizz", PLUCKED,
        ["inst" => 2.0, "tech" => 1.0, "mode" => 0.0, "pizz.decay" => 1.0, "ring" => 0.4,
         "gain" => 2.4]),
    preset!("arcus.preset.violin_tremolo", VIOLIN,
        ["inst" => 2.0, "tech" => 2.0, "trem.rate" => 15.0, "vib.depth" => 0.4,
         "gain" => 8.5]),
    // ---- the viola ----
    preset!("arcus.preset.viola", VIOLA, ["inst" => 3.0, "ring" => 1.6, "gain" => 4.4]),
    preset!("arcus.preset.viola_cantabile", VIOLA,
        ["inst" => 3.0, "vib.depth" => 1.2, "vib.delay" => 180.0, "vib.attack" => 340.0,
         "bow.pos" => 1.08, "bow.attack" => 1.5, "dyn" => 0.62, "body.ring" => 1.2,
         "gain" => 4.8]),
    preset!("arcus.preset.viola_tasto", VIOLA,
        ["inst" => 3.0, "bow.pos" => 1.34, "bow.force" => 0.81, "bow.speed" => 0.8,
         "vib.depth" => 0.75, "bow.attack" => 1.8, "gain" => 8.2]),
    preset!("arcus.preset.viola_detache", VIOLA,
        ["inst" => 3.0, "mode" => 1.0, "bow.release" => 60.0, "bow.change" => 18.0,
         "vib.delay" => 120.0, "gain" => 4.4]),
    preset!("arcus.preset.viola_pizz", PLUCKED,
        ["inst" => 3.0, "tech" => 1.0, "mode" => 0.0, "pizz.decay" => 1.2, "ring" => 0.5,
         "gain" => 0.8]),
    preset!("arcus.preset.viola_tremolo", VIOLA,
        ["inst" => 3.0, "tech" => 2.0, "trem.rate" => 14.0, "vib.depth" => 0.4,
         "gain" => 5.7]),
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
