//! The characters Cantus includes (ADR 0090): ours and original, for the whole family,
//! good on both themes, each with every move and every mood.
//!
//! The cast, in the picker's order: first the Slate cast (round 40, ADR 0122), drawn on the
//! Slate rig (`crate::slate`) in the theme's own light —
//! - [`compas`] — **Compás** (`builtin:compas`), the pictogram, a new dancer's character;
//! - [`tono`] — **Tono** (`builtin:tono`), the knob;
//! - [`sincopa`] — **Síncopa** (`builtin:sincopa`), noir;
//! - [`bajo`] — **Bajo** (`builtin:bajo`), the speaker;
//! - [`onda`] — **Onda** (`builtin:onda`), the oscilloscope;
//! - [`voz`] — **Voz** (`builtin:voz`), the microphone;
//! - [`glissando`] — **Glissando** (`builtin:glissando`), the figure of light;
//! - [`eco`] — **Eco** (`builtin:eco`), the delay's ghost;
//! - [`vinilo`] — **Vinilo** (`builtin:vinilo`), the DJ;
//! - [`cinta`] — **Cinta** (`builtin:cinta`), the cassette;
//! - [`metronomo`] — **Metrónomo** (`builtin:metronomo`), the serious one;
//! - [`espectro`] — **Espectro** (`builtin:espectro`), the analyser;
//!
//! then the family (ADR 0090, ADR 0105), which old songs keep:
//! - [`ninja`] — **Fuga** (`builtin:ninja`), the default of rounds 37–39;
//! - [`mecha`] — **Volta** (`builtin:mecha`, round 37);
//! - [`songbird`] — **Trino** (`builtin:songbird`), the default of rounds 35–36;
//! - [`cat`] — **Miso** (`builtin:cat`);
//! - [`lamb`] — **Lana** (`builtin:lamb`);
//! - [`robot`] — **Tic** (`builtin:robot`);
//! - [`note`] — **Corchea** (`builtin:note`);
//! - [`child`] — **Sol** (`builtin:child`).
//!
//! They share the rig (`crate::rig`): one construction, one way of painting, one
//! choreography danced in each one's own style. Their names and blurbs are the locale
//! files' (`cast.<id>.name`, `cast.<id>.blurb`: [`name_key`], [`blurb_key`]);
//! `CharacterInfo::name` holds the English name for where no locale is at hand.
//! [`placeholder`] is the contract's stand-in, kept as the last resort of
//! [`default_character`] (never in the picker).

use crate::character::Character;
use std::sync::Arc;

pub mod bajo;
pub mod cat;
pub mod child;
pub mod cinta;
pub mod compas;
pub mod eco;
pub mod edge;
pub mod espectro;
pub mod glissando;
pub mod kit;
pub mod lamb;
pub mod mecha;
pub mod metronomo;
pub mod ninja;
pub mod note;
pub mod onda;
pub mod placeholder;
pub mod robot;
pub mod sincopa;
pub mod songbird;
pub mod tono;
pub mod vinilo;
pub mod voz;

/// Every included character, in the picker's order (the first is a new dancer's).
pub fn builtin() -> Vec<Arc<dyn Character>> {
    vec![
        Arc::new(compas::character()),
        Arc::new(tono::character()),
        Arc::new(sincopa::character()),
        Arc::new(bajo::character()),
        Arc::new(onda::character()),
        Arc::new(voz::character()),
        Arc::new(glissando::character()),
        Arc::new(eco::character()),
        Arc::new(vinilo::character()),
        Arc::new(cinta::character()),
        Arc::new(metronomo::character()),
        Arc::new(espectro::character()),
        Arc::new(ninja::Ninja::new()),
        Arc::new(mecha::Mecha::new()),
        Arc::new(songbird::Songbird::new()),
        Arc::new(cat::Cat::new()),
        Arc::new(lamb::Lamb::new()),
        Arc::new(robot::Robot::new()),
        Arc::new(note::Note::new()),
        Arc::new(child::Child::new()),
    ]
}

/// One included character by its id (`builtin:<id>`).
pub fn by_id(id: &str) -> Option<Arc<dyn Character>> {
    builtin().into_iter().find(|c| c.info().id == id)
}

/// The locale key of an included character's name: `cast.<id>.name` for `builtin:<id>`.
pub fn name_key(id: &str) -> String {
    format!("cast.{}.name", id.trim_start_matches("builtin:"))
}

/// The locale key of an included character's one-line blurb: `cast.<id>.blurb`.
pub fn blurb_key(id: &str) -> String {
    format!("cast.{}.blurb", id.trim_start_matches("builtin:"))
}

/// What a new dancer shows, and what draws when a character cannot be found.
pub fn default_character() -> Arc<dyn Character> {
    by_id(cantus_core::inst::dancer::DEFAULT_CHARACTER)
        .or_else(|| builtin().into_iter().next())
        .unwrap_or_else(|| Arc::new(placeholder::Placeholder::new()))
}
