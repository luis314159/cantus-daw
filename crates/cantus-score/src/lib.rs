//! # Cantus — the score (round 32, ADR 0075)
//!
//! Notes in, an engraved score out. A track's or a clip's notes are **read** (rounded for
//! reading on a grid each passage chooses, never moved), split into staves and voices,
//! written as values the metre shows with ties, rests, tuplets and beams, spelled in the
//! song's key with accidentals per bar — the logical [`model::Score`]. MusicXML 4.0 is
//! written from it (`musicxml`) and read back as notes (`musicxml_in`). The layout
//! engraves it after Dorico, MuseScore 4, Sibelius and Gould's *Behind Bars*, in the SMuFL
//! font Bravura with its engraving defaults and anchors, into a display list in staff
//! spaces that the screen, the PDF and the golden tests share.
//!
//! No egui here: the interface draws the display list with its own painter.

pub mod build;
pub mod display;
pub mod glyphs;
mod glyphs_meta;
pub mod hands;
pub mod layout;
pub mod metre;
pub mod model;
pub mod musicxml;
pub mod musicxml_in;
pub mod outline;
pub mod pdf;
pub mod read;
pub mod rhythm;
pub mod samples;
pub mod source;
pub mod spell;
pub mod staff;
mod staff_marks;
pub mod text;
pub mod voices;

pub use build::{build, PartSource, Song};
pub use display::{DisplayList, Item};
pub use glyphs::{Glyph, BRAVURA};
pub use layout::{engrave, Engraved, Format, LayoutOptions, Paper};
pub use model::Score;
pub use read::SourceNote;
