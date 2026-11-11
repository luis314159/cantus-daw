//! The type each design system sets the interface in (round 39, ADR 0111): the bundled
//! faces, the family chains egui resolves, and every text role's size, face, tracking and
//! case per [`Face`].
//!
//! **One set of family names, many faces.** Every role, and every older call site, names
//! a family by a stable name (`cond`, `cond-medium`, `value`, `clock`, egui's
//! `Proportional` and `Monospace`…). A face decides which file each name resolves to, so
//! switching the theme's face re-points the names and every view follows with no change
//! of its own. Only the faces in use are handed to egui ([`data`]): IBM Plex always (the
//! units' own lettering, ADR 0074, and the fallback after every other face), plus the
//! face's own files.
//!
//! The files: IBM Plex as bundled since round 24 (with Plex Sans Condensed ExtraLight and
//! Light for Studio ink's clock and tempo), and Barlow Semi Condensed and Sofia Sans as
//! `tools/fonts/build_faces.py` builds them from the published files — tabular figures by
//! default (egui applies no OpenType feature), unhinted, Sofia as static instances. All
//! SIL OFL 1.1; the licences are `assets/fonts/LICENSE-OFL*.txt`.

use super::system::Face;
use crate::design::role::Role;

/// A face file: its `font_data` key and its bytes.
pub type FaceFile = (&'static str, &'static [u8]);

pub const PLEX_COND_EXTRALIGHT: FaceFile = (
    "plex-cond-extralight",
    include_bytes!("../../assets/fonts/IBMPlexSansCondensed-ExtraLight.ttf"),
);
pub const PLEX_COND_LIGHT: FaceFile = (
    "plex-cond-light",
    include_bytes!("../../assets/fonts/IBMPlexSansCondensed-Light.ttf"),
);

/// Barlow Semi Condensed 1.408 (Slate): ExtraLight for the clock, Light for its ticks and
/// the large readouts, Regular for names and values, Medium for titles, SemiBold for
/// section titles and tags.
pub const BARLOW: [FaceFile; 5] = [
    (
        "barlow-extralight",
        include_bytes!("../../assets/fonts/BarlowSemiCondensed-ExtraLight.ttf"),
    ),
    (
        "barlow-light",
        include_bytes!("../../assets/fonts/BarlowSemiCondensed-Light.ttf"),
    ),
    (
        "barlow-regular",
        include_bytes!("../../assets/fonts/BarlowSemiCondensed-Regular.ttf"),
    ),
    (
        "barlow-medium",
        include_bytes!("../../assets/fonts/BarlowSemiCondensed-Medium.ttf"),
    ),
    (
        "barlow-semibold",
        include_bytes!("../../assets/fonts/BarlowSemiCondensed-SemiBold.ttf"),
    ),
];

/// Sofia Sans 4.101 (Graphite and gold), static instances: Semi Condensed 500, 600 and 650
/// for words, Condensed 600 for narrow places, Extra Condensed 200 and 300 for the clock and
/// the large values.
pub const SOFIA: [FaceFile; 6] = [
    (
        "sofia-semi-500",
        include_bytes!("../../assets/fonts/SofiaSansSemiCondensed-Medium.ttf"),
    ),
    (
        "sofia-semi-600",
        include_bytes!("../../assets/fonts/SofiaSansSemiCondensed-SemiBold.ttf"),
    ),
    (
        "sofia-semi-650",
        include_bytes!("../../assets/fonts/SofiaSansSemiCondensed-SemiBold650.ttf"),
    ),
    (
        "sofia-cond-600",
        include_bytes!("../../assets/fonts/SofiaSansCondensed-SemiBold.ttf"),
    ),
    (
        "sofia-xc-200",
        include_bytes!("../../assets/fonts/SofiaSansExtraCondensed-ExtraLight.ttf"),
    ),
    (
        "sofia-xc-300",
        include_bytes!("../../assets/fonts/SofiaSansExtraCondensed-Light.ttf"),
    ),
];

/// The files a face adds to IBM Plex's.
pub fn data(face: Face) -> &'static [FaceFile] {
    match face {
        Face::Plex => &[],
        Face::Barlow => &BARLOW,
        Face::Sofia => &SOFIA,
        Face::PlexInk => &[PLEX_COND_EXTRALIGHT, PLEX_COND_LIGHT],
    }
}

/// The stable family names roles and call sites use (egui's `Proportional` and
/// `Monospace` aside, which [`proportional`] and [`monospace`] give).
pub const FAMILIES: [&str; 10] = [
    "cond",
    "cond-medium",
    "cond-semibold",
    "cond-italic",
    "value",
    "big",
    "clock",
    "strong",
    "mono-strong",
    "mono-light",
];

/// The Plex `font_data` keys `design` registers (the classic face's files).
pub struct Plex {
    pub sans: &'static str,
    pub sans_strong: &'static str,
    pub mono: &'static str,
    pub mono_strong: &'static str,
    pub mono_light: &'static str,
    pub cond: &'static str,
    pub cond_medium: &'static str,
    pub cond_semibold: &'static str,
    pub cond_italic: &'static str,
}

/// The first file a family name resolves to in `face`; the chain then falls back to the
/// Plex face of the same role and to the symbol font ([`crate::design`]).
pub fn first(face: Face, family: &str, p: &Plex) -> &'static str {
    match face {
        Face::Plex => match family {
            "cond" | "big" | "clock" => p.cond,
            "cond-medium" | "value" => p.cond_medium,
            "cond-semibold" => p.cond_semibold,
            "cond-italic" => p.cond_italic,
            "strong" => p.sans_strong,
            "mono-strong" => p.mono_strong,
            "mono-light" => p.mono_light,
            "monospace" => p.mono,
            _ => p.sans,
        },
        Face::Barlow => match family {
            "cond-medium" | "mono-strong" => "barlow-medium",
            "cond-semibold" | "strong" => "barlow-semibold",
            "big" | "mono-light" => "barlow-light",
            "clock" => "barlow-extralight",
            "cond-italic" => p.cond_italic,
            _ => "barlow-regular",
        },
        Face::Sofia => match family {
            "cond-medium" | "mono-strong" | "strong" => "sofia-semi-600",
            "cond-semibold" => "sofia-semi-650",
            "big" | "mono-light" => "sofia-xc-300",
            "clock" => "sofia-xc-200",
            "cond-italic" => p.cond_italic,
            _ => "sofia-semi-500",
        },
        Face::PlexInk => match family {
            "cond-medium" => p.cond_medium,
            "cond-semibold" | "strong" => p.cond_semibold,
            "cond-italic" => p.cond_italic,
            "value" | "monospace" => p.mono,
            "mono-strong" => p.mono_strong,
            "mono-light" => p.mono_light,
            "big" => "plex-cond-light",
            "clock" => "plex-cond-extralight",
            _ => p.cond,
        },
    }
}

/// The Plex file a family falls back to when its face lacks a character.
pub fn fallback(family: &str, p: &Plex) -> &'static str {
    match family {
        "cond-medium" | "value" | "mono-strong" => p.cond_medium,
        "cond-semibold" | "strong" => p.cond_semibold,
        "cond-italic" => p.cond_italic,
        _ => p.cond,
    }
}

/// How a role is set in a face.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Spec {
    /// Points at 100 %.
    pub size: f32,
    /// One of [`FAMILIES`].
    pub family: &'static str,
    /// Extra space between letters, points at 100 %.
    pub tracking: f32,
    /// Set in capitals.
    pub caps: bool,
}

const fn spec(size: f32, family: &'static str, tracking: f32, caps: bool) -> Spec {
    Spec {
        size,
        family,
        tracking,
        caps,
    }
}

/// How `role` is set in `face`.
///
/// - **Plex** — ADR 0024 §3 as refined in round 37.
/// - **Barlow** (Slate, `dir-a`'s `system.png`): clock ExtraLight 30; names Regular 12;
///   titles Medium 13; labels Regular 11; section titles SemiBold 11 in title case (never
///   capital bars); tags SemiBold 9 in capitals.
/// - **Sofia** (Graphite, `dir-b`): clock Extra Condensed 200 at 34, large values Extra
///   Condensed 300 at 23; titles 650 at 14; body 500 at 12; micro 650 at 8, capitals +0.4.
/// - **PlexInk** (Studio ink, `dir-c`): clock ExtraLight 31, tempo Light 22, values in Plex
///   Mono, captions Medium 8.5 in capitals +0.9, section heads in sentence case.
pub fn of(face: Face, role: Role) -> Spec {
    match face {
        Face::Plex => match role {
            Role::Micro => spec(8.5, "cond-semibold", 0.3, true),
            Role::Label => spec(11.0, "cond-medium", 0.25, false),
            Role::Body => spec(12.5, "cond", 0.0, false),
            Role::Control => spec(12.0, "cond-medium", 0.0, false),
            Role::Title => spec(13.0, "cond-medium", 0.0, false),
            Role::Section => spec(11.0, "cond-semibold", 0.8, true),
            Role::Value => spec(12.0, "value", 0.0, false),
            Role::Hero => spec(18.0, "big", 0.0, false),
            Role::Display => spec(26.0, "clock", -0.4, false),
            Role::Placeholder => spec(12.5, "cond-italic", 0.0, false),
        },
        Face::Barlow => match role {
            Role::Micro => spec(9.0, "cond-semibold", 0.3, true),
            Role::Label => spec(11.0, "cond", 0.0, false),
            Role::Body => spec(12.0, "cond", 0.0, false),
            Role::Control => spec(12.0, "cond-medium", 0.0, false),
            Role::Title => spec(13.0, "cond-medium", 0.0, false),
            Role::Section => spec(11.0, "cond-semibold", 0.0, false),
            Role::Value => spec(12.0, "value", 0.0, false),
            Role::Hero => spec(18.0, "big", 0.0, false),
            Role::Display => spec(30.0, "clock", 0.0, false),
            Role::Placeholder => spec(12.0, "cond", 0.0, false),
        },
        Face::Sofia => match role {
            Role::Micro => spec(8.0, "cond-semibold", 0.4, true),
            Role::Label => spec(11.0, "cond", 0.0, false),
            Role::Body => spec(12.0, "cond", 0.0, false),
            Role::Control => spec(12.0, "cond-medium", 0.0, false),
            Role::Title => spec(14.0, "cond-semibold", 0.0, false),
            Role::Section => spec(11.0, "cond-semibold", 0.0, false),
            Role::Value => spec(12.0, "value", 0.0, false),
            Role::Hero => spec(23.0, "big", 0.0, false),
            Role::Display => spec(34.0, "clock", 0.0, false),
            Role::Placeholder => spec(12.0, "cond", 0.0, false),
        },
        Face::PlexInk => match role {
            Role::Micro => spec(8.5, "cond-medium", 0.9, true),
            Role::Label => spec(11.0, "cond", 0.0, false),
            Role::Body => spec(12.0, "cond", 0.0, false),
            Role::Control => spec(12.0, "cond-medium", 0.0, false),
            Role::Title => spec(13.0, "cond-medium", 0.0, false),
            Role::Section => spec(11.0, "cond-semibold", 0.0, false),
            Role::Value => spec(12.0, "value", 0.0, false),
            Role::Hero => spec(22.0, "big", 0.0, false),
            Role::Display => spec(31.0, "clock", 0.0, false),
            Role::Placeholder => spec(12.0, "cond", 0.0, false),
        },
    }
}

/// The bytes the faces of round 39 add to the program, in total and per face (for the
/// installer's size, reported in `docs/round39/kit.md`).
pub fn bundled_bytes(face: Face) -> usize {
    data(face).iter().map(|(_, b)| b.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_of_every_face_names_a_known_family() {
        for face in Face::ALL {
            for role in Role::ALL {
                let s = of(face, role);
                assert!(FAMILIES.contains(&s.family), "{face:?} {role:?}");
                assert!(s.size >= 8.0, "{face:?} {role:?}: under 8 pt");
            }
        }
    }

    #[test]
    fn the_new_faces_figures_are_tabular() {
        // Every digit one advance: a counting value never shifts (build_faces.py's remap).
        use skrifa::raw::TableProvider;
        use skrifa::{FontRef, MetadataProvider};
        for (key, bytes) in BARLOW.iter().chain(SOFIA.iter()) {
            let font = FontRef::new(bytes).expect(key);
            let charmap = font.charmap();
            let hmtx = font.hmtx().expect(key);
            let widths: std::collections::BTreeSet<u16> = "0123456789"
                .chars()
                .map(|c| {
                    let g = charmap.map(c).expect(key);
                    hmtx.advance(g).expect(key)
                })
                .collect();
            assert_eq!(widths.len(), 1, "{key}: digits of {widths:?} units");
        }
    }
}
