//! The count-in before playback (round 30, the owner's friction: "I set it to one bar and
//! nothing sounded before Play"). The count-in is a recording aid, as in FL and Ableton: it
//! plays before recording from stop. *Also on play* (Reaper's option, a global setting,
//! off by default) makes Play from stop count in too. With it off, the first Play from stop
//! with a count-in set says once, on the status line, that the count-in waits for Record.
//!
//! The engine's count-in is the record start (`EngineHandle::set_record_start`): the click
//! sounds while the transport is before it, whatever the metronome switch says. A play
//! count-in sets it without recording, and takes it away when the transport stops.

use crate::app::CantusApp;
use crate::i18n::t;

/// The bars to count in when Play is pressed from stop, if any: `on_play` is the
/// *Also on play* setting, `bars` the project's count-in.
pub fn bars_on_play(on_play: bool, bars: u8) -> Option<u8> {
    (on_play && bars > 0).then_some(bars)
}

/// Whether Play from stop should say, once, that the count-in plays before recording.
pub fn tell_only_before_recording(on_play: bool, bars: u8, told: bool) -> bool {
    !on_play && bars > 0 && !told
}

/// What the engine compiles, when it is not the song as it is: the pattern playing in
/// pattern mode, and — while a count-in clicks with the metronome off — that (or the song)
/// with the click on for the count-in only (`ClickMode::CountInOnly`); the project itself is
/// never touched. `None`: the song as it is.
pub fn project_to_compile(
    pattern: Option<cantus_core::Project>,
    song: &cantus_core::Project,
    count_in_click: bool,
) -> Option<cantus_core::Project> {
    if !count_in_click {
        return pattern;
    }
    let mut p = pattern.unwrap_or_else(|| song.clone());
    p.metronome = true;
    p.recording.click = cantus_core::ClickMode::CountInOnly;
    Some(p)
}

impl CantusApp {
    /// Play from where the playhead is (the transport is stopped): with *Also on play*
    /// and a count-in set, the transport starts that many bars early and the click counts
    /// in; the music starts where the playhead was.
    pub fn start_playback(&mut self) {
        self.end_play_count_in();
        let bars = self.project().recording.count_in_bars;
        let on_play = self.settings.count_in_on_play;
        if let Some(bars) = bars_on_play(on_play, bars) {
            let pos = self.engine.position();
            let from = crate::views::bars::add_bars(self.project(), pos, -i64::from(bars));
            self.engine.seek(from);
            self.engine.set_record_start(Some(pos));
            self.rec.play_from = Some(pos);
            if !self.project().metronome {
                self.rec.count_in_click = true;
                self.force_resync();
                self.sync_engine();
            }
            self.rec.play_count_in = true;
        } else if tell_only_before_recording(on_play, bars, self.rec.count_in_told) {
            self.rec.count_in_told = true;
            self.set_status(t("ship.count_in.only_rec"));
        }
        self.engine.play();
    }

    /// The transport stopped: a play count-in's record start and forced click go, and a
    /// Stop pressed during the count-in puts the playhead back where Play was pressed (it
    /// never creeps back a bar per aborted Play; review 1).
    pub fn end_play_count_in(&mut self) {
        let from = self.rec.play_from.take();
        if std::mem::take(&mut self.rec.play_count_in) {
            if let Some(pos) = from {
                if self.engine.position() < pos {
                    self.engine.seek(pos);
                }
            }
            self.count_in_done();
        }
    }

    /// The count-in is over (the transport reached where Play was pressed, or stopped):
    /// the record start goes — or the click would count in again before that point on
    /// every lap of a loop (review 3) — and the forced click with it.
    fn count_in_done(&mut self) {
        if !self.is_recording() {
            self.engine.set_record_start(None);
            if std::mem::take(&mut self.rec.count_in_click) {
                self.force_resync();
            }
        }
    }

    /// Each frame: once the transport has passed where Play was pressed, the play
    /// count-in ends while the music goes on.
    pub fn tick_play_count_in(&mut self) {
        let Some(pos) = self.rec.play_from else {
            return;
        };
        if self.rec.play_count_in && self.engine.is_playing() && self.engine.position() >= pos {
            self.rec.play_count_in = false;
            self.rec.play_from = None;
            self.count_in_done();
        }
    }

    /// Stop the transport wherever it is asked (Stop, a project replaced or opened, a
    /// recovery, an import): a play count-in ends with it (review 5).
    pub fn stop_transport(&mut self) {
        self.engine.stop();
        self.end_play_count_in();
    }

    /// Whether a play count-in is under way or was started (tests, the chip).
    pub fn play_count_in(&self) -> bool {
        self.rec.play_count_in
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_in_clicks_over_the_pattern_in_pattern_mode() {
        let song = crate::welcome_project();
        assert!(
            project_to_compile(None, &song, false).is_none(),
            "the song as it is"
        );
        let mut pattern = song.clone();
        pattern.tracks.truncate(1);
        let n = pattern.tracks.len();
        let got = project_to_compile(Some(pattern.clone()), &song, false).unwrap();
        assert_eq!(got.tracks.len(), n, "pattern mode plays the pattern");
        // Review 2: the count-in's click is laid over the pattern, not the whole song.
        let got = project_to_compile(Some(pattern), &song, true).unwrap();
        assert_eq!(got.tracks.len(), n);
        assert!(got.metronome);
        assert_eq!(got.recording.click, cantus_core::ClickMode::CountInOnly);
        let got = project_to_compile(None, &song, true).unwrap();
        assert_eq!(got.tracks.len(), song.tracks.len());
    }

    #[test]
    fn play_counts_in_only_with_the_option_and_a_count_in() {
        assert_eq!(bars_on_play(false, 2), None, "off by default: Record only");
        assert_eq!(bars_on_play(true, 0), None, "no count-in set");
        assert_eq!(bars_on_play(true, 1), Some(1));
        assert_eq!(bars_on_play(true, 4), Some(4));
    }

    #[test]
    fn the_first_play_with_a_count_in_says_it_waits_for_record_once() {
        assert!(tell_only_before_recording(false, 1, false));
        assert!(!tell_only_before_recording(false, 1, true), "once");
        assert!(
            !tell_only_before_recording(false, 0, false),
            "nothing to say"
        );
        assert!(!tell_only_before_recording(true, 1, false), "it counts in");
    }
}
