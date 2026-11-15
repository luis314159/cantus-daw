//! The fx side round's faces (provisional ADRs 0130–0133): Cribrum, Fragor, Lapsus, Vetus and
//! Libra, and the colourways the user can choose for them (ADR 0130 §Colourways). The tests
//! below hold every face **and every offered colourway** to the house rule: none shares a
//! material and an accent within 20° with any other unit's face or colourway, and no two of
//! the round's sheets speak in the same voices.

use super::super::color::Hsl;
use super::{
    Case, Colourway, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin,
    SwitchShape, Tint, Weight,
};

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
} // fx:fragor
use cantus_core::EffectKind;

// fx:cribrum — Cribrum's own face (ADR 0130, provisional), in three colourways the user
// chooses among (ADR 0130 §Colourways); the first, Nocturne, is its face. Every display is
// a lit screen, dark in both themes (the owner's choice for the round's displays).

const fn chsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// Cribrum's face apart from its colours: machined caps, lamp keys, a printed legend, screws
/// and rules, condensed capitals and mono readouts.
const fn cribrum(
    key: &'static str,
    surface: Tint,
    accent: Pair,
    accent2: Pair,
    material: Material,
    display: Display,
) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(accent2),
        material,
        display,
        knob: KnobShape::Cap,
        switch: SwitchShape::Lamp,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.10,
            weight: Weight::SemiBold,
            condensed: true,
            mono_values: true,
        },
        plate: Plate::Legend,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: true,
            rack_ears: false,
        },
    }
}

/// Cribrum, the auto filter (*the sieve*) — **Nocturne**, its face: violet carbon weave, the
/// response ice-cyan on a dark phosphor screen over the music it sieves, the movement amber.
/// No other unit is violet carbon (the flanger's violet is lacquer).
pub const CRIBRUM: Skin = cribrum(
    "fx.cribrum",
    Tint {
        hue: 272.0,
        sat: 0.26,
        amount: 0.6,
    },
    Pair {
        dark: chsl(190.0, 0.80, 0.66),
        light: chsl(194.0, 0.80, 0.30),
    },
    Pair {
        dark: chsl(40.0, 0.90, 0.64),
        light: chsl(34.0, 0.80, 0.36),
    },
    Material::Carbon,
    Display::Phosphor,
);

/// Cribrum in **Brass**: gold anodised aluminium, the response teal on a fluorescent
/// display, the movement brick.
pub const CRIBRUM_BRASS: Skin = cribrum(
    "fx.cribrum.brass",
    Tint {
        hue: 48.0,
        sat: 0.34,
        amount: 0.6,
    },
    Pair {
        dark: chsl(172.0, 0.72, 0.56),
        light: chsl(176.0, 0.80, 0.26),
    },
    Pair {
        dark: chsl(8.0, 0.78, 0.66),
        light: chsl(6.0, 0.70, 0.42),
    },
    Material::Anodised,
    Display::Vfd,
);

/// Cribrum in **Petrol**: deep teal-blue lacquer, the response hot orange on a phosphor
/// screen, the movement lime. The orange sits at 20° (it was 24°, 19° from Chorda's gold
/// lacquer: the house rule wants more than 20°).
pub const CRIBRUM_PETROL: Skin = cribrum(
    "fx.cribrum.petrol",
    Tint {
        hue: 192.0,
        sat: 0.34,
        amount: 0.6,
    },
    Pair {
        dark: chsl(20.0, 0.95, 0.62),
        light: chsl(16.0, 0.85, 0.40),
    },
    Pair {
        dark: chsl(84.0, 0.70, 0.62),
        light: chsl(88.0, 0.70, 0.28),
    },
    Material::Lacquer,
    Display::Phosphor,
);

/// Cribrum's colourways, its face first.
pub const CRIBRUM_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "nocturne",
        name_key: "cribrum.colourway.nocturne",
        sheet: &CRIBRUM,
    },
    Colourway {
        id: "brass",
        name_key: "cribrum.colourway.brass",
        sheet: &CRIBRUM_BRASS,
    },
    Colourway {
        id: "petrol",
        name_key: "cribrum.colourway.petrol",
        sheet: &CRIBRUM_PETROL,
    },
];

// fx:fragor — Fragor's face (ADR 0131), the owner's pick (palette A, 2026-11-13): a valve
// amp at night — warm amber, a tube's glow, on smoked glass; the sound drawn on an amber
// phosphor screen behind glass (dark in every theme, as a device's screen), a white-hot second
// voice, skirted knobs, rocker switches, engraved capitals, screws and rules. Its tubes,
// grille, jewel lights and brass are drawn by `views/fx/valve_draw.rs`; its pedals wear
// their own finishes (`skin/pedals.rs`).
pub const FRAGOR: Skin = Skin {
    key: "fx.fragor",
    surface: Tint {
        hue: 32.0,
        sat: 0.16,
        amount: 0.35,
    },
    accent: Some(Pair {
        dark: hsl(36.0, 0.92, 0.58),
        light: hsl(30.0, 0.85, 0.38),
    }),
    accent2: Some(Pair {
        // A filament's white heat, nearly unsaturated: the second voice keeps its own
        // colour beside the amber, not a second amber (the displays' rule).
        dark: hsl(44.0, 0.18, 0.84),
        light: hsl(28.0, 0.45, 0.28),
    }),
    // Smoked glass, as a valve's envelope and the screen's pane: the amber family's
    // matte, brushed, anodised, lacquer, carbon and vinyl are taken near this hue
    // (Phasis, Fervor, Chorda, Aula …).
    material: Material::Glass,
    display: Display::Phosphor,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Rocker,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.12,
        weight: Weight::SemiBold,
        condensed: true,
        mono_values: false,
    },
    plate: Plate::Engraved,
    ornaments: Ornaments {
        screws: true,
        vents: false,
        rules: true,
        rack_ears: false,
    },
}; // fx:fragor

// fx:lapsus — Lapsus's own face (ADR 0132, provisional; v3 after the integration review).
/// Lapsus, the time shaper (*a slip*): a DJ mixer at midnight — a dark teal-petrol body in
/// matte rubberised paint (a mixer's top plate), the time curve glowing amber on a
/// vacuum-fluorescent screen (the old decks' displays), the level and the controls in ice
/// blue, soft rubber knobs, lamp keys, a painted stripe across the header and printed rules,
/// upper-case lettering and mono readouts. Its accent is the ice blue, so its material is
/// one no unit wears near that hue (anodised, brushed, lacquer, carbon, glass and vinyl
/// are taken there; amber was taken in every finish but paper). The body sits 18° from
/// Cribrum's Petrol and the ice blue 18° from Cribrum's cyan (`tests` below).
pub const LAPSUS: Skin = Skin {
    key: "fx.lapsus",
    surface: super::Tint {
        hue: 174.0,
        sat: 0.26,
        amount: 0.6,
    },
    accent: Some(super::Pair {
        dark: super::super::color::Hsl {
            h: 208.0,
            s: 0.72,
            l: 0.68,
        },
        light: super::super::color::Hsl {
            h: 212.0,
            s: 0.80,
            l: 0.34,
        },
    }),
    accent2: Some(super::Pair {
        dark: super::super::color::Hsl {
            h: 36.0,
            s: 0.95,
            l: 0.60,
        },
        light: super::super::color::Hsl {
            h: 32.0,
            s: 0.90,
            l: 0.36,
        },
    }),
    material: super::Material::Matte,
    display: super::Display::Vfd,
    knob: super::KnobShape::Soft,
    switch: super::SwitchShape::Lamp,
    lettering: super::Lettering {
        case: super::Case::Upper,
        tracking: 0.08,
        weight: super::Weight::SemiBold,
        condensed: false,
        mono_values: true,
    },
    plate: super::Plate::Stripe,
    ornaments: super::Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: false,
    },
};

// fx:vetus — Vetus's face (ADR 0133, provisional) in three colourways (ADR 0130
// §Colourways); the first, Cassette, is its face (the owner's choice, 2026-11-13). The record,
// the cassette and the spectrum sit on a dark display in every colourway and theme (the
// round's rule for displays); the objects keep their own colours (`skin/records.rs`).

/// Vetus's face apart from its colours: dials with printed scales, bat-handle toggles, a
/// badge, screws, wide spaced capitals — a hi-fi deck of the seventies, without the vents.
const fn vetus(
    key: &'static str,
    surface: Tint,
    accent: Pair,
    accent2: Pair,
    material: Material,
    display: Display,
) -> Skin {
    Skin {
        key,
        surface,
        accent: Some(accent),
        accent2: Some(accent2),
        material,
        display,
        knob: KnobShape::Dial,
        switch: SwitchShape::Bat,
        lettering: Lettering {
            case: Case::Upper,
            tracking: 0.12,
            weight: Weight::SemiBold,
            condensed: false,
            mono_values: true,
        },
        plate: Plate::Badge,
        ornaments: Ornaments {
            screws: true,
            vents: false,
            rules: false,
            rack_ears: false,
        },
    }
}

/// Vetus, lo-fi (*old*) — **Cassette**, its face: a tape's glossy black shell, its label's
/// red, the medium's band a label's cream, on a dark phosphor screen. Red near this hue is
/// matte (Limiter) or vinyl covering (Delay): lacquer is free there.
pub const VETUS: Skin = vetus(
    "fx.vetus",
    Tint {
        hue: 222.0,
        sat: 0.06,
        amount: 0.40,
    },
    Pair {
        dark: chsl(354.0, 0.78, 0.62),
        light: chsl(354.0, 0.72, 0.42),
    },
    Pair {
        dark: chsl(44.0, 0.40, 0.82),
        light: chsl(30.0, 0.45, 0.30),
    },
    Material::Lacquer,
    Display::Phosphor,
);

/// Vetus in **Sleeve**: a record sleeve's card in a light, clean cream, the music vermilion,
/// the band a printer's blue (teal beside vermilion is Cribrum's Brass).
pub const VETUS_SLEEVE: Skin = vetus(
    "fx.vetus.sleeve",
    Tint {
        hue: 38.0,
        sat: 0.22,
        amount: 0.35,
    },
    Pair {
        dark: chsl(12.0, 0.82, 0.63),
        light: chsl(10.0, 0.78, 0.40),
    },
    Pair {
        dark: chsl(214.0, 0.62, 0.68),
        light: chsl(218.0, 0.62, 0.34),
    },
    Material::Paper,
    Display::Phosphor,
);

/// Vetus in **Walnut**: a hi-fi cabinet's oiled walnut, old gold on a fluorescent display,
/// the band ivory. The gold sits at 31° (a brass at 44° was 8° from Anulus's on wood).
pub const VETUS_WALNUT: Skin = vetus(
    "fx.vetus.walnut",
    Tint {
        hue: 24.0,
        sat: 0.36,
        amount: 0.6,
    },
    Pair {
        dark: chsl(31.0, 0.78, 0.60),
        light: chsl(30.0, 0.75, 0.32),
    },
    Pair {
        dark: chsl(40.0, 0.30, 0.84),
        light: chsl(24.0, 0.55, 0.28),
    },
    Material::Wood,
    Display::Vfd,
);

/// Vetus's colourways, its face first.
pub const VETUS_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "cassette",
        name_key: "vetus.colourway.cassette",
        sheet: &VETUS,
    },
    Colourway {
        id: "sleeve",
        name_key: "vetus.colourway.sleeve",
        sheet: &VETUS_SLEEVE,
    },
    Colourway {
        id: "walnut",
        name_key: "vetus.colourway.walnut",
        sheet: &VETUS_WALNUT,
    },
]; // fx:vetus

// fx:libra — Libra's face (ADR 0133, provisional): a measuring instrument — blue-grey
// anodised aluminium, the needle and the cloud a signal green on an oscilloscope's graticule
// (dark in every theme), the balance and width amber; plain caps, lamp keys, an engraved
// legend, screws and rules. Green near this hue is wood (Porta) or anodised 40° away
// (Translatio): no unit wears it on anodised aluminium.
pub const LIBRA: Skin = Skin {
    key: "fx.libra",
    surface: Tint {
        hue: 212.0,
        sat: 0.12,
        amount: 0.45,
    },
    accent: Some(Pair {
        dark: chsl(142.0, 0.60, 0.56),
        light: chsl(146.0, 0.70, 0.27),
    }),
    accent2: Some(Pair {
        dark: chsl(36.0, 0.85, 0.62),
        light: chsl(30.0, 0.80, 0.34),
    }),
    material: Material::Anodised,
    display: Display::Phosphor,
    knob: KnobShape::Cap,
    switch: SwitchShape::Lamp,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.08,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: true,
    },
    plate: Plate::Engraved,
    ornaments: Ornaments {
        screws: true,
        vents: false,
        rules: true,
        rack_ears: false,
    },
}; // fx:libra

// fx3:contract — wave 3's placeholder faces (provisional ADRs 0134–0137): each unit wears
// a tone-family sibling's look under a key of its own (the tests here leave a placeholder
// out) until its builder draws the real face — one that differs from every other unit's in
// hue or material, display included.

/// Tabula, the pedalboard (ADR 0134): a pedalboard's case — black vinyl of a fine grain
/// (the owner's "más fino": darker, cleaner, never competing with the pedals), brass piping
/// for its accent (the board's nameplate), chicken-head knobs, bat toggles, a badge and
/// screws; no vents. Its own face: the only fine vinyl, and no other vinyl unit wears a
/// gold near 52°. // fx3:tabula
pub const TABULA: Skin = Skin {
    key: "fx.tabula",
    surface: Tint {
        hue: 30.0,
        sat: 0.05,
        amount: 0.55,
    },
    accent: Some(Pair {
        dark: super::super::color::Hsl {
            h: 46.0,
            s: 0.62,
            l: 0.62,
        },
        light: super::super::color::Hsl {
            h: 40.0,
            s: 0.70,
            l: 0.32,
        },
    }),
    accent2: None,
    material: Material::FineTolex,
    display: Display::Plain,
    knob: KnobShape::ChickenHead,
    switch: SwitchShape::Bat,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.10,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: false,
    },
    plate: Plate::Badge,
    ornaments: Ornaments {
        screws: true,
        vents: false,
        rules: false,
        rack_ears: false,
    },
};

/// Saltus, pitch: for now Tonus's look (the pitch unit's cousin). // fx3:saltus
pub const SALTUS: Skin = Skin {
    key: "fx.saltus",
    ..super::tone::TONUS
};

/// Rota, the rotary speaker: for now Spatium's look. // fx3:rota
pub const ROTA: Skin = Skin {
    key: "fx.rota",
    ..super::tone::SPATIUM
};

/// Arca, amp and cabinet: for now Aequalis's look. // fx3:arca
pub const ARCA: Skin = Skin {
    key: "fx.arca",
    ..super::tone::AEQUALIS
};

/// fx:colourways — the colourways a unit of the side round offers (empty: one face).
pub fn colourways(kind: EffectKind) -> &'static [Colourway] {
    match kind {
        EffectKind::AutoFilter => &CRIBRUM_COLOURWAYS, // fx:cribrum
        EffectKind::Pedal => &FRAGOR_COLOURWAYS,       // fx:fragor
        EffectKind::LoFi => &VETUS_COLOURWAYS,         // fx:vetus
        _ => &[],
    }
}

/// The sheet of a unit of the side round.
pub fn of(kind: EffectKind) -> Option<&'static Skin> {
    match kind {
        EffectKind::AutoFilter => Some(&CRIBRUM),
        EffectKind::Pedal => Some(&FRAGOR),
        EffectKind::TimeShaper => Some(&LAPSUS),
        EffectKind::LoFi => Some(&VETUS),
        EffectKind::Utility => Some(&LIBRA),
        EffectKind::Board => Some(&TABULA),      // fx3:tabula
        EffectKind::PitchShift => Some(&SALTUS), // fx3:saltus
        EffectKind::Rotary => Some(&ROTA),       // fx3:rota
        EffectKind::Amp => Some(&ARCA),          // fx3:arca
        _ => None,
    }
}

// fx:fragor — the two palettes the owner was shown beside the one he chose (2026-11-13),
// offered as Fragor's colourways (ADR 0130 §Colourways) after his own, Valve.
/// **Cream**: cream and brass, vintage — a printed card's cream (paper: brushed metal near
/// this brass is Aula's and Calor's), the second voice a bottle green (a brick red beside the
/// brass spoke in Vetus's two voices, and beside Cribrum's Brass body).
pub const FRAGOR_B: Skin = Skin {
    key: "fx.fragor.cream",
    surface: Tint {
        hue: 42.0,
        sat: 0.30,
        amount: 0.55,
    },
    accent: Some(Pair {
        dark: hsl(44.0, 0.60, 0.56),
        light: hsl(40.0, 0.70, 0.32),
    }),
    accent2: Some(Pair {
        dark: hsl(120.0, 0.45, 0.58),
        light: hsl(124.0, 0.50, 0.30),
    }),
    material: Material::Paper,
    display: Display::Vfd,
    ..FRAGOR
}; // fx:fragor
/// **Steel**: steel with a single signal red.
pub const FRAGOR_C: Skin = Skin {
    key: "fx.fragor.steel",
    surface: Tint {
        hue: 212.0,
        sat: 0.06,
        amount: 0.30,
    },
    accent: Some(Pair {
        dark: hsl(4.0, 0.80, 0.58),
        light: hsl(2.0, 0.74, 0.44),
    }),
    accent2: Some(Pair {
        dark: hsl(210.0, 0.08, 0.78),
        light: hsl(214.0, 0.10, 0.38),
    }),
    material: Material::Anodised,
    display: Display::Scope,
    ..FRAGOR
}; // fx:fragor
/// Fragor's colourways, its face first: Valve (the owner's pick), Cream and Steel. fx:fragor
pub const FRAGOR_COLOURWAYS: [Colourway; 3] = [
    Colourway {
        id: "valve",
        name_key: "fragor.colourway.valve",
        sheet: &FRAGOR,
    },
    Colourway {
        id: "cream",
        name_key: "fragor.colourway.cream",
        sheet: &FRAGOR_B,
    },
    Colourway {
        id: "steel",
        name_key: "fragor.colourway.steel",
        sheet: &FRAGOR_C,
    },
];

// fx:lapsus — the side round's faces never wear another's look (the integration review:
// Fragor and Lapsus were both amber glass; Lapsus's blue was Cribrum's, its body Petrol's).
#[cfg(test)]
mod tests {
    use super::super::{colourways, of, Skin, Unit};
    use super::{CRIBRUM, FRAGOR, LAPSUS, LIBRA, VETUS};

    fn hue_gap(a: f32, b: f32) -> f32 {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d)
    }

    /// Two hues within this many degrees are one colour at a glance.
    const BAND: f32 = 15.0;

    /// The side round's faces as drawn (a sheet still wearing a sibling's look is left out).
    fn faces() -> Vec<&'static Skin> {
        [&CRIBRUM, &FRAGOR, &LAPSUS, &VETUS, &LIBRA]
            .into_iter()
            .filter(|s| !is_placeholder(s))
            .collect()
    }

    /// The faces and the colourways a user can choose for them.
    fn faces_and_colourways() -> Vec<&'static Skin> {
        let mut v = faces();
        for u in Unit::all() {
            v.extend(colourways(u).iter().map(|c| c.sheet));
        }
        let mut seen = std::collections::BTreeSet::new();
        v.retain(|s| seen.insert(s.key));
        v
    }

    /// A sheet that is another unit's look under a key of its own.
    fn is_placeholder(s: &Skin) -> bool {
        Unit::all().into_iter().map(of).any(|o| {
            o.key != s.key
                && o.surface == s.surface
                && o.accent == s.accent
                && o.accent2 == s.accent2
                && o.material == s.material
                && o.display == s.display
        })
    }

    /// The unit a sheet belongs to (`fx.cribrum.brass` → `cribrum`).
    fn unit_of(key: &str) -> &str {
        key.split('.').nth(1).unwrap_or(key)
    }

    /// The hues a sheet's voices read in, the nearly grey ones left out.
    fn voices(s: &Skin) -> Vec<f32> {
        [s.accent, s.accent2]
            .into_iter()
            .flatten()
            .filter(|p| p.dark.s >= 0.3)
            .map(|p| p.dark.h)
            .collect()
    }

    /// A body tinted enough to read as a colour of its own.
    fn tinted(s: &Skin) -> bool {
        s.surface.sat * s.surface.amount >= 0.1
    }

    /// The house rule (every family's): no face of the round, and no colourway a user can
    /// choose for it, shares a material and an accent within 20° with any other unit's face
    /// or colourway.
    #[test]
    fn no_face_or_colourway_of_the_side_round_shares_material_and_accent_with_another_unit() {
        let mut others: Vec<&'static Skin> = Unit::all().into_iter().map(of).collect();
        others.extend(faces_and_colourways());
        let mut bad = Vec::new();
        for s in faces_and_colourways() {
            let a = s.accent.expect("a face has an accent").dark.h;
            for o in &others {
                let Some(oa) = o.accent else { continue };
                if unit_of(o.key) == unit_of(s.key) || is_placeholder(o) {
                    continue;
                }
                if s.material == o.material && hue_gap(a, oa.dark.h) <= 20.0 {
                    bad.push(format!(
                        "{} and {}: {:?} near {a:.0}°",
                        s.key, o.key, s.material
                    ));
                }
            }
        }
        bad.sort();
        bad.dedup();
        assert!(
            bad.is_empty(),
            "{}",
            bad.join(
                "
"
            )
        );
    }

    /// Within the round, whatever the material: no two units' sheets speak in the same
    /// two voices (in either order), nor tint the body alike under a shared voice.
    #[test]
    fn the_side_round_s_sheets_never_share_their_palette() {
        let all = faces_and_colourways();
        let mut bad = Vec::new();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                if unit_of(a.key) == unit_of(b.key) {
                    continue;
                }
                let (va, vb) = (voices(a), voices(b));
                let near = |h: f32, y: &[f32]| y.iter().any(|g| hue_gap(h, *g) <= BAND);
                let covered = |x: &[f32], y: &[f32]| x.iter().all(|h| near(*h, y));
                if va.len() > 1 && vb.len() > 1 && covered(&va, &vb) && covered(&vb, &va) {
                    bad.push(format!("{} and {}: the same voices", a.key, b.key));
                }
                if tinted(a)
                    && tinted(b)
                    && hue_gap(a.surface.hue, b.surface.hue) <= BAND
                    && va.iter().any(|h| near(*h, &vb))
                {
                    bad.push(format!("{} and {}: one body under one voice", a.key, b.key));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{}",
            bad.join(
                "
"
            )
        );
    }

    /// The finding that made these tests, pinned: Lapsus is neither Fragor's amber glass
    /// nor Cribrum's ice blue on Petrol's body.
    #[test]
    fn lapsus_wears_neither_fragor_s_glass_nor_cribrum_s_blue_and_petrol() {
        assert_ne!(LAPSUS.material, FRAGOR.material);
        let ice = LAPSUS.accent.unwrap().dark.h;
        assert!(hue_gap(ice, CRIBRUM.accent.unwrap().dark.h) > BAND);
        assert!(hue_gap(LAPSUS.surface.hue, super::CRIBRUM_PETROL.surface.hue) > BAND);
        // The owner's pairing kept: the time curve amber, the level ice blue.
        assert!(hue_gap(LAPSUS.accent2.unwrap().dark.h, 36.0) <= 4.0);
        assert!((190.0..=230.0).contains(&ice));
    }
}
