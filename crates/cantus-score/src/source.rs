//! Where a part's notes come from: a clip (its window and repeats, as it plays) or a
//! whole track (its playing lane's note clips, as the arrangement plays them).

use crate::build::PartSource;
use crate::read::SourceNote;
use cantus_core::model::{Clip, Track};
use cantus_core::time::Ticks;

/// The notes a clip shows on the timeline: every repetition inside its window, each cut
/// at the window's end. Muted notes are left out unless `muted` says keep them.
pub fn clip_notes(clip: &Clip, muted: bool) -> Vec<SourceNote> {
    if clip.audio.is_some() || clip.automation.is_some() {
        return Vec::new();
    }
    let end = clip.end().0;
    let mut out = Vec::with_capacity(clip.notes.len());
    let mut at: Vec<Ticks> = Vec::new();
    for n in &clip.notes {
        if n.muted && !muted {
            continue;
        }
        clip.timeline_positions(n.start, &mut at);
        for t in &at {
            let len = n.length.0.min(end - t.0);
            if len > 0 {
                out.push(SourceNote {
                    id: n.id.0,
                    start: t.0,
                    len,
                    pitch: n.pitch,
                    velocity: n.velocity,
                });
            }
        }
    }
    out.sort_by_key(|n| (n.start, n.pitch, n.id));
    out
}

/// The notes a track plays: the note clips of its playing lane that are not muted.
pub fn track_notes(track: &Track) -> Vec<SourceNote> {
    let mut out: Vec<SourceNote> = track
        .clips
        .iter()
        .filter(|c| c.lane == 0 && !c.muted)
        .flat_map(|c| clip_notes(c, false))
        .collect();
    out.sort_by_key(|n| (n.start, n.pitch, n.id));
    out
}

/// A track's part.
pub fn track_part(track: &Track) -> PartSource {
    PartSource {
        name: track.name.clone(),
        notes: track_notes(track),
        settings: track.score.clone(),
    }
}

/// A clip's part, written with its track's settings (a muted clip is still shown: it is
/// the one asked for).
pub fn clip_part(track: &Track, clip: &Clip) -> PartSource {
    PartSource {
        name: track.name.clone(),
        notes: clip_notes(clip, false),
        settings: track.score.clone(),
    }
}
