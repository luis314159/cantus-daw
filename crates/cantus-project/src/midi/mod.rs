//! Standard MIDI Files: a clip, a track or a song out as type 1; type 0, 1 or 2 in as new
//! tracks and clips (ADR 0021).
//!
//! - [`smf`] is the file format, byte for byte.
//! - [`export`] writes what the engine would play.
//! - [`import`] reads notes and says what it could not keep; placing them in a project is
//!   a separate step that yields one undoable batch.
//! - [`gm`] is General MIDI's drum map and program names.

pub mod export;
pub mod gm;
pub mod import;
pub mod smf;

pub use export::{
    export, export_with, to_file_ticks, to_smf, to_smf_with, ExportOptions, NoteFxPlay, Notes,
    Scope, DEFAULT_PPQ, DRUM_CHANNEL,
};
pub use import::{
    import_commands, read, Built, ImportOptions, ImportedNote, ImportedTrack, MidiImport,
    Placement, TempoMode, TrackLabel,
};
pub use smf::{Division, Event, MidiError, Smf, TimedEvent};

/// The file extensions a MIDI file goes by.
pub const EXTENSIONS: [&str; 3] = ["mid", "midi", "smf"];

/// Whether a path looks like a MIDI file, by its extension.
pub fn is_midi_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}
