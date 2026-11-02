//! The characters Cantus includes (ADR 0090): ours and original, for the whole family,
//! good on both themes, each with every move and every mood.
//!
//! The family (ADR 0090), in the picker's order:
//! - [`songbird`] — **Trino** (`builtin:songbird`), a new dancer's character;
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

pub mod cat;
pub mod child;
pub mod kit;
pub mod lamb;
pub mod note;
pub mod placeholder;
pub mod robot;
pub mod songbird;

/// Every included character, in the picker's order (the first is a new dancer's).
pub fn builtin() -> Vec<Arc<dyn Character>> {
    vec![
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
