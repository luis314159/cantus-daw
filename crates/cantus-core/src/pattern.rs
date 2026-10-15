//! Pattern mode (ADR 0052, BACKLOG §6e): FL's PAT / SONG switch.
//!
//! *Song mode* plays the arrangement. *Pattern mode* plays the **current pattern** alone,
//! looped over its length, through the tracks' own chains — what FL plays when its switch
//! says PAT.
//!
//! ## What the current pattern is
//!
//! Cantus has no pattern objects: the channel rack edits a **pattern region** — the window
//! of the clip being edited (its start, and one period if it repeats or its whole length if
//! not) across every track (`views::channel_rack`, round 17). So the current pattern is
//! that window and, on each track, the note clip the rack shows in it: the one holding the
//! window's start, or failing that the first one overlapping the window. What the rack
//! shows is what pattern mode plays; in the common case — one clip in its window — that is
//! the clip alone. Audio clips, takes and automation clips are the song's and stay out, as
//! FL's playlist clips stay out of its pattern mode.
//!
//! ## How it plays
//!
//! [`audition`] makes the project the engine is handed in pattern mode: the same project
//! with every other clip taken out and the loop set to the window, on. The engine needs no
//! mode of its own — it plays a snapshot compiled from that project exactly as it plays
//! any other, so pattern mode is sample-accurate by construction (offline equals real
//! time, block size changes nothing), and the project itself is never touched.

use crate::model::{Clip, ClipId, LoopRegion, Project, Track, TrackId};
use crate::time::Ticks;

/// The current pattern: a window of the timeline and, per track, the clip played in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternRegion {
    pub start: Ticks,
    /// One period of the clip, or its whole length when it does not repeat; never zero.
    pub length: Ticks,
    /// The clip each track plays in the window, in track order (tracks with none left out).
    pub clips: Vec<(TrackId, ClipId)>,
}

impl PatternRegion {
    /// The pattern around the note clip `clip` on `track` — the clip the piano roll and the
    /// channel rack edit. `None` when there is no such clip, it holds audio or automation,
    /// or it has no length.
    pub fn of(project: &Project, track: TrackId, clip: ClipId) -> Option<PatternRegion> {
        let c = project.track(track)?.clip(clip)?;
        if !is_pattern_clip(c) {
            return None;
        }
        let length = c.period().unwrap_or(c.length).min(c.length);
        if length <= Ticks::ZERO {
            return None;
        }
        let start = c.start;
        let end = start + length;
        let clips = project
            .tracks
            .iter()
            .filter_map(|t| {
                if t.id == track {
                    return Some((t.id, clip));
                }
                clip_in(t, start, end).map(|c| (t.id, c.id))
            })
            .collect();
        Some(PatternRegion {
            start,
            length,
            clips,
        })
    }

    pub fn end(&self) -> Ticks {
        self.start + self.length
    }

    /// The clip `track` plays in the pattern, if any.
    pub fn clip_of(&self, track: TrackId) -> Option<ClipId> {
        self.clips.iter().find(|(t, _)| *t == track).map(|(_, c)| *c)
    }
}

/// A note clip on the playing lane: what a pattern is made of.
fn is_pattern_clip(c: &Clip) -> bool {
    c.audio.is_none() && c.automation.is_none() && c.lane == 0
}

/// The note clip of `track` in the window `[start, end)`: the one holding `start`, or
/// failing that the first one overlapping the window (the channel rack's rule).
fn clip_in(track: &Track, start: Ticks, end: Ticks) -> Option<&Clip> {
    let notes = || track.clips.iter().filter(|c| is_pattern_clip(c));
    notes()
        .find(|c| c.start <= start && c.end() > start)
        .or_else(|| notes().find(|c| c.start < end && c.end() > start))
}

/// The project pattern mode hands the engine: every clip but the pattern's taken out, the
/// loop set to the pattern's window and switched on. With no pattern (no note clip being
/// edited) it is the project with no clips at all, looping its first bar — the click still
/// counts, as FL's empty pattern does.
pub fn audition(project: &Project, region: Option<&PatternRegion>) -> Project {
    let mut p = project.clone();
    for t in &mut p.tracks {
        let keep = region.and_then(|r| r.clip_of(t.id));
        t.clips.retain(|c| Some(c.id) == keep);
    }
    let (start, end) = match region {
        Some(r) => (r.start, r.end()),
        None => (Ticks::ZERO, project.time_signature.ticks_per_bar()),
    };
    p.loop_region = Some(LoopRegion { start, end });
    p.loop_enabled = true;
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::Snapshot;

    /// Two tracks: drums with a one-bar clip at bar 3 and another at bar 9; bass with a
    /// two-bar clip from bar 2 (it holds bar 3's start) and an audio clip at bar 3.
    fn song() -> (Project, [TrackId; 2], [ClipId; 3]) {
        let mut p = Project::new("Song");
        let bar = Ticks::beats(4);
        let mut drums = p.make_track("Drums");
        let mut a = p.make_clip("A", bar * 2, bar);
        for i in 0..4 {
            let n = p.make_note(Ticks::QUARTER * i, Ticks::SIXTEENTH, 36, 100);
            a.notes.push(n);
        }
        let mut b = p.make_clip("B", bar * 8, bar);
        let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 38, 100);
        b.notes.push(n);
        let (a_id, b_id) = (a.id, b.id);
        drums.clips.extend([a, b]);
        let mut bass = p.make_track("Bass");
        let mut c = p.make_clip("Line", bar, bar * 2);
        let n = p.make_note(bar, Ticks::QUARTER, 40, 90);
        c.notes.push(n);
        let c_id = c.id;
        bass.clips.push(c);
        let ids = [drums.id, bass.id];
        p.tracks.extend([drums, bass]);
        (p, ids, [a_id, b_id, c_id])
    }

    #[test]
    fn the_pattern_is_the_edited_clips_window_across_the_tracks() {
        let (p, [drums, bass], [a, _, line]) = song();
        let r = PatternRegion::of(&p, drums, a).expect("a pattern");
        assert_eq!(r.start, Ticks::beats(8));
        assert_eq!(r.length, Ticks::beats(4));
        assert_eq!(r.clips, vec![(drums, a), (bass, line)]);
        // A repeating clip's pattern is one period.
        let mut q = p.clone();
        let c = q.track_mut(drums).unwrap().clip_mut(a).unwrap();
        c.content_length = Some(Ticks::beats(2));
        let r = PatternRegion::of(&q, drums, a).unwrap();
        assert_eq!(r.length, Ticks::beats(2));
    }

    /// The switch is the project's (FL saves it with the song): a command with its exact
    /// inverse, not written while it says *song* — a project that never used it saves as
    /// before.
    #[test]
    fn the_mode_is_a_command_saved_with_the_song() {
        use crate::command::Command;
        let mut p = Project::new("P");
        assert!(!p.pattern_mode);
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("pattern_mode"), "{json}");
        let undo = Command::SetPatternMode { on: true }.apply(&mut p).unwrap();
        assert!(p.pattern_mode);
        assert_eq!(undo, Command::SetPatternMode { on: false });
        let json = serde_json::to_string(&p).unwrap();
        assert!(serde_json::from_str::<Project>(&json).unwrap().pattern_mode);
        undo.apply(&mut p).unwrap();
        assert!(!p.pattern_mode);
        assert_eq!(
            Command::SetPatternMode { on: true }.label_key(),
            "cmd.set_pattern_mode"
        );
    }

    #[test]
    fn audio_and_automation_clips_are_not_patterns() {
        let (mut p, [drums, _], [a, _, _]) = song();
        p.track_mut(drums).unwrap().clip_mut(a).unwrap().lane = 1;
        assert_eq!(PatternRegion::of(&p, drums, a), None, "a take is not a pattern");
        assert_eq!(PatternRegion::of(&p, drums, ClipId(999)), None);
    }

    /// Pattern mode compiles exactly the pattern's notes, at their places in the song,
    /// with the loop over the window; nothing else plays and the project is untouched.
    #[test]
    fn the_audition_plays_the_pattern_alone_looped_over_it() {
        let (p, [drums, bass], [a, _, _]) = song();
        let before = p.clone();
        let r = PatternRegion::of(&p, drums, a).unwrap();
        let heard = audition(&p, Some(&r));
        assert_eq!(p, before, "the project is never touched");
        let s = Snapshot::compile(&heard, 1);
        assert!(s.loop_enabled);
        let lr = s.loop_region.clone().unwrap();
        assert_eq!((lr.start, lr.end), (Ticks::beats(8), Ticks::beats(12)));
        let on = |t: TrackId| -> Vec<(Ticks, u8)> {
            s.track(t)
                .unwrap()
                .events
                .iter()
                .filter(|e| e.is_on())
                .map(|e| (e.tick, e.pitch))
                .collect()
        };
        let d: Vec<_> = (0..4).map(|i| (Ticks::beats(8 + i), 36)).collect();
        assert_eq!(on(drums), d, "pattern A's four kicks and nothing of clip B");
        assert_eq!(on(bass), vec![(Ticks::beats(8), 40)], "the bass line in the window");
        // No pattern: nothing plays, the first bar loops.
        let empty = Snapshot::compile(&audition(&p, None), 1);
        assert!(empty.tracks.iter().all(|t| t.events.is_empty()));
        let lr = empty.loop_region.unwrap();
        assert_eq!((lr.start, lr.end), (Ticks::ZERO, Ticks::beats(4)));
    }
}
