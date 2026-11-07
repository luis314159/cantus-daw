//! Chorda's look (round 37, ADR 0104): the piano its scene draws — the body (a concert
//! grand, a grand, a parlour grand, an upright) and its finish (black, white, walnut,
//! rosewood) — kept with the instrument as two text keys, so a project reopens with the
//! piano it was shown as.
//!
//! **Never audible.** The look is two *text* keys: no knob, no automation lane, no
//! modulation destination (only numeric parameters are), and the engine's `configure`
//! reads none of it — a test renders a phrase with every look and finds the same bits.
//!
//! **Honest.** Chorda models one grand's strings (Bensa et al.'s measured C2/C4/C7 — about
//! a two-metre grand's scale) voiced against a recorded Yamaha C5 (ADR 0098). So a factory
//! preset may show only a body its sound can stand for — [`fits`]: a grand where the strings
//! are a grand's (stiffness, the scale's inharmonicity) and in tune within a grand's
//! unisons; an upright only where the sound is voiced as one (the case shut over the board —
//! the lid closed or nearly — and an upright's mark: short-scale stiffness, a honky-tonk's
//! detuned unisons, or the felt rail's soft, noisy action). The user may still pick any
//! look: it is their picture, and the picker says it never changes the sound.

use super::NativeSpec;

/// The text key of the body.
pub const BODY_KEY: &str = "look.body";
/// The text key of the finish.
pub const FINISH_KEY: &str = "look.finish";
/// Both, in order.
pub const KEYS: [&str; 2] = [BODY_KEY, FINISH_KEY];

/// The piano drawn.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Body {
    /// A nine-foot concert grand (2.74 m).
    #[default]
    Concert,
    /// A grand of about two metres: the model's own scale (a C5 class).
    Grand,
    /// A parlour (baby) grand, 1.55 m.
    Parlour,
    /// A studio upright, 1.25 m tall.
    Upright,
}

impl Body {
    pub const ALL: [Body; 4] = [Body::Concert, Body::Grand, Body::Parlour, Body::Upright];

    /// The stored word (forever once shipped).
    pub fn key(self) -> &'static str {
        match self {
            Body::Concert => "concert",
            Body::Grand => "grand",
            Body::Parlour => "parlour",
            Body::Upright => "upright",
        }
    }

    pub fn from_key(k: &str) -> Option<Body> {
        Body::ALL.into_iter().find(|b| b.key() == k)
    }

    /// The string key of its name (`chorda.look.body.*`).
    pub fn label_key(self) -> &'static str {
        match self {
            Body::Concert => "chorda.look.body.concert",
            Body::Grand => "chorda.look.body.grand",
            Body::Parlour => "chorda.look.body.parlour",
            Body::Upright => "chorda.look.body.upright",
        }
    }

    pub fn is_grand(self) -> bool {
        !matches!(self, Body::Upright)
    }
}

/// The case's finish.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Finish {
    /// Polished black (ebony) lacquer.
    #[default]
    Black,
    /// Polished white lacquer.
    White,
    /// Satin walnut.
    Walnut,
    /// Polished rosewood.
    Rosewood,
}

impl Finish {
    pub const ALL: [Finish; 4] = [
        Finish::Black,
        Finish::White,
        Finish::Walnut,
        Finish::Rosewood,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Finish::Black => "black",
            Finish::White => "white",
            Finish::Walnut => "walnut",
            Finish::Rosewood => "rosewood",
        }
    }

    pub fn from_key(k: &str) -> Option<Finish> {
        Finish::ALL.into_iter().find(|f| f.key() == k)
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Finish::Black => "chorda.look.finish.black",
            Finish::White => "chorda.look.finish.white",
            Finish::Walnut => "chorda.look.finish.walnut",
            Finish::Rosewood => "chorda.look.finish.rosewood",
        }
    }

    /// Whether the finish is wood (its grain drawn).
    pub fn is_wood(self) -> bool {
        matches!(self, Finish::Walnut | Finish::Rosewood)
    }
}

/// A body and a finish.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Look {
    pub body: Body,
    pub finish: Finish,
}

impl Look {
    pub const fn new(body: Body, finish: Finish) -> Look {
        Look { body, finish }
    }
}

/// The look a spec shows: its keys, or the default (a black concert grand) for a key not
/// there or a word this build does not know.
pub fn of(spec: &NativeSpec) -> Look {
    Look {
        body: spec
            .text(BODY_KEY)
            .and_then(Body::from_key)
            .unwrap_or_default(),
        finish: spec
            .text(FINISH_KEY)
            .and_then(Finish::from_key)
            .unwrap_or_default(),
    }
}

/// The inharmonicity scale above which the strings are a short scale's (an upright's or a
/// small grand's: shorter strings at the same pitch are stiffer, B ∝ 1/L²), and at or below
/// which they are long enough for a concert grand.
pub const SHORT_SCALE: f32 = 1.15;
pub const LONG_SCALE: f32 = 1.05;
/// The widest unison spread (cents) a grand in tune keeps; past it, a honky-tonk's.
pub const GRAND_UNISON: f32 = 4.0;
/// The most open lid an upright's sound can have: its case shuts over the board.
pub const UPRIGHT_LID: f32 = 0.5;

/// Whether a sound with parameter values `get` can honestly be shown as `body` (see the
/// module docs). `get(key)` reads a parameter as the spec does (stored or default).
pub fn fits(body: Body, get: impl Fn(&str) -> f32) -> bool {
    let stiffness = get("stiffness");
    let unison = get("unison");
    match body {
        Body::Concert => stiffness <= LONG_SCALE && unison <= GRAND_UNISON,
        Body::Grand => stiffness <= SHORT_SCALE && unison <= GRAND_UNISON,
        // A parlour grand's scale is shorter: a little more stiffness is still one.
        Body::Parlour => stiffness <= SHORT_SCALE + 0.1 && unison <= GRAND_UNISON,
        Body::Upright => {
            let felt = get("hardness") <= -0.6 && get("noise") >= 1.2;
            get("lid") <= UPRIGHT_LID
                && (stiffness >= SHORT_SCALE + 0.1 || unison > GRAND_UNISON || felt)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InstrumentKind;

    #[test]
    fn keys_round_trip_and_unknown_words_read_the_default() {
        for b in Body::ALL {
            assert_eq!(Body::from_key(b.key()), Some(b));
        }
        for f in Finish::ALL {
            assert_eq!(Finish::from_key(f.key()), Some(f));
        }
        let mut spec = NativeSpec::new(InstrumentKind::Chorda);
        assert_eq!(of(&spec), Look::default());
        spec.set_text(BODY_KEY, Some("upright".into())).unwrap();
        spec.set_text(FINISH_KEY, Some("walnut".into())).unwrap();
        assert_eq!(of(&spec), Look::new(Body::Upright, Finish::Walnut));
        spec.set_text(BODY_KEY, Some("harpsichord".into())).unwrap();
        assert_eq!(of(&spec).body, Body::Concert);
    }

    #[test]
    fn the_look_is_text_never_a_parameter() {
        let k = InstrumentKind::Chorda;
        for key in KEYS {
            assert!(k.is_text_key(key) && !k.is_asset_key(key), "{key}");
            assert!(k.param(key).is_none(), "{key} must not be a parameter");
        }
        // Only Chorda holds it.
        assert!(!InstrumentKind::Organ.is_text_key(BODY_KEY));
    }

    #[test]
    fn a_default_chorda_is_a_grand_and_never_an_upright() {
        let spec = NativeSpec::new(InstrumentKind::Chorda);
        let get = |k: &str| spec.get(k);
        assert!(fits(Body::Concert, get));
        assert!(fits(Body::Grand, get));
        assert!(!fits(Body::Upright, get), "an open lid, a grand's strings");
    }
}
