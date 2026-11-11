//! A theme's **design system** (round 39, ADR 0111): what a theme decides beyond its
//! colours — the type family the interface is set in and how a key is built.
//!
//! The colours of a theme are its tokens ([`super::tokens::Tokens`]); these two choices
//! travel beside them, in the theme ([`crate::theme::Theme::system`]) and in the tokens
//! every painter is handed ([`super::tokens::Tokens::system`]), so a painter asks its
//! style, never a global, how to draw.
//!
//! - **Slate** (the default, `dir-a`): Barlow Semi Condensed, keys recessed into their bar.
//! - **Graphite and gold** (`dir-b`): Sofia Sans in three widths, keys recessed.
//! - **Studio ink** (`dir-c`): IBM Plex Sans Condensed with Plex Mono for values, keys
//!   recessed.
//! - **Classic**: today's IBM Plex and raised keys (round 37) — the high-contrast pair and
//!   every theme a user made before round 39 keep it, so nothing of theirs changes look.
//!
//! The faces themselves (the bundled files, each role's size and weight) are
//! [`super::faces`].

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

/// The type family a theme sets its interface in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Face {
    /// IBM Plex Sans Condensed, with Plex Mono for the readouts (rounds 24–38).
    #[default]
    Plex,
    /// Barlow Semi Condensed, tabular figures, one family from a 30-pt ExtraLight clock
    /// down to 9-pt tags (Slate, `dir-a`).
    Barlow,
    /// Sofia Sans in three widths: Semi Condensed for words, Condensed for narrow places,
    /// Extra Condensed for the clock (Graphite and gold, `dir-b`).
    Sofia,
    /// IBM Plex Sans Condensed with Plex Mono for every value that changes and an
    /// ExtraLight clock (Studio ink, `dir-c`).
    PlexInk,
}

impl Face {
    pub const ALL: [Face; 4] = [Face::Plex, Face::Barlow, Face::Sofia, Face::PlexInk];

    fn index(self) -> u8 {
        match self {
            Face::Plex => 0,
            Face::Barlow => 1,
            Face::Sofia => 2,
            Face::PlexInk => 3,
        }
    }

    fn from_index(i: u8) -> Face {
        match i {
            1 => Face::Barlow,
            2 => Face::Sofia,
            3 => Face::PlexInk,
            _ => Face::Plex,
        }
    }
}

/// How a key is built.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Keys {
    /// An object standing up from its bar, lit from above (round 37, ADR 0103).
    #[default]
    Raised,
    /// A flat well a shade darker than its bar: a 1-px dark edge above, a 1-px light lip
    /// below, no gloss. Hover lifts the well a step, pressed darkens it to its edge, on
    /// tints it with the accent and draws the glyph in the accent (round 39, `dir-a`).
    Recessed,
}

/// A theme's design system: its type family, its keys, and how its mixer draws cables.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct System {
    pub face: Face,
    pub keys: Keys,
    /// The cables' look where the user chose none (r39:el-cables). Absent in a theme
    /// file written before it: Flow, today's default in every family.
    pub cables: CableStyle,
}

/// How the mixer draws its routing cables (round 39, r39:el-cables; des-mixer's
/// `options-cables-*`): *Patch* hangs a curve from jack to jack; *Flow* is Patch with a
/// gleam that travels along each cable while audio flows; *Subway* lays orthogonal routes
/// in lanes under the strips. A theme names its own ([`System::cables`]); the user may
/// choose another (`Settings::cable_style`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CableStyle {
    Patch,
    #[default]
    Flow,
    Subway,
    /// Real ¼-inch patch cables: seated plugs, cords hanging on a catenary, a settle after
    /// a drop (des-mixer's `options-cables-jack-*`; the vintage theme's own, later).
    Jack,
}

impl CableStyle {
    pub const ALL: [CableStyle; 4] = [
        CableStyle::Patch,
        CableStyle::Flow,
        CableStyle::Subway,
        CableStyle::Jack,
    ];

    /// Its name (an i18n key).
    pub fn key(self) -> &'static str {
        match self {
            CableStyle::Patch => "cables.style.patch",
            CableStyle::Flow => "cables.style.flow",
            CableStyle::Subway => "cables.style.subway",
            CableStyle::Jack => "cables.style.jack",
        }
    }

    /// Whether a gleam runs along its cables while audio flows (Flow; Jack's cords too).
    pub fn gleams(self) -> bool {
        matches!(self, CableStyle::Flow | CableStyle::Jack)
    }
}

impl System {
    /// Rounds 24–38: Plex and raised keys. What a theme file without a `system` is.
    pub const CLASSIC: System = System {
        face: Face::Plex,
        keys: Keys::Raised,
        cables: CableStyle::Flow,
    };
    /// Slate (round 39, the default).
    pub const SLATE: System = System {
        face: Face::Barlow,
        keys: Keys::Recessed,
        cables: CableStyle::Flow,
    };
    /// Graphite and gold (round 39).
    pub const GRAPHITE: System = System {
        face: Face::Sofia,
        keys: Keys::Recessed,
        cables: CableStyle::Flow,
    };
    /// Studio ink (round 39).
    pub const STUDIO_INK: System = System {
        face: Face::PlexInk,
        keys: Keys::Recessed,
        cables: CableStyle::Flow,
    };

    pub fn is_classic(&self) -> bool {
        *self == System::CLASSIC
    }

    /// Whether keys are recessed wells.
    pub fn recessed(&self) -> bool {
        self.keys == Keys::Recessed
    }
}

/// The face the process's text roles are set in: the installed theme's. Text roles are
/// asked for without a style in hundreds of places (`design::role::Role::font`), so the
/// face, like the installed tokens, is process-wide; [`crate::theme::apply`] sets it and
/// installs the matching fonts. Until a theme is applied it is Plex, the face every
/// part was drawn in before round 39 (the kit's golden images, a plug-in window); the
/// program's start (`boot`) sets the default theme's at once.
static FACE: AtomicU8 = AtomicU8::new(0);

/// The face text roles resolve to now.
pub fn face() -> Face {
    Face::from_index(FACE.load(Ordering::Relaxed))
}

/// Make `f` the face text roles resolve to. Returns whether it changed.
pub fn set_face(f: Face) -> bool {
    FACE.swap(f.index(), Ordering::Relaxed) != f.index()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_file_without_a_system_is_classic() {
        let s: System = serde_json::from_str("{}").unwrap();
        assert_eq!(s, System::CLASSIC);
        assert!(s.is_classic());
        let back: System =
            serde_json::from_str(&serde_json::to_string(&System::SLATE).unwrap()).unwrap();
        assert_eq!(back, System::SLATE);
    }

    #[test]
    fn faces_round_trip_through_their_index() {
        for f in Face::ALL {
            assert_eq!(Face::from_index(f.index()), f);
        }
    }
}
