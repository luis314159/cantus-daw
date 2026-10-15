//! FL's quick gestures, from the application's side (round 27, ADR 0051): what a
//! double-click on a track header and on a clip in the arrangement open. The arrangement
//! decides *where* a gesture landed; this decides what it means, and the tests drive the
//! same road. (A double-click on a clip browser tile selects the clip's copies instead,
//! ADR 0037.)

use crate::app::{CantusApp, CentralView};
use crate::clips::{self, ClipEditor};
use crate::views::workspace::Pane;
use cantus_core::{ClipId, TrackId};

impl CantusApp {
    /// A double-click on a track header: the track is selected and its instrument's
    /// window opens, or comes back from the tray and to the front — a native
    /// instrument's editor, Pulsus's, a hosted plug-in's own window. A track whose
    /// instrument has no window shows the inspector instead, where it is edited: Firmus,
    /// and an **audio track** (ADR 0025 §3), whose instrument is never played and whose
    /// inspector shows its input and its audio.
    pub fn open_track_instrument(&mut self, track: TrackId) {
        let Some(audio) = self
            .project()
            .track(track)
            .map(crate::record_session::is_audio_track)
        else {
            return;
        };
        self.select_track(track);
        if audio {
            self.show_pane(Pane::Inspector);
        } else {
            self.open_instrument_window(track);
        }
    }

    /// A double-click on a clip: its editor, on it, in front and with the focus — the
    /// piano roll, or the channel rack for a step pattern it shows with nothing lost
    /// ([`clips::editor_for`]), the automation editor for an automation clip, the pitch
    /// editor for an audio clip (ADR 0054). Returns which, or `None` when the clip is gone.
    pub fn open_clip_editor(&mut self, track: TrackId, clip: ClipId) -> Option<ClipEditor> {
        let (which, grid) = {
            let t = self.project().track(track)?;
            let c = t.clip(clip)?;
            let lanes = t.instrument.lanes();
            (
                clips::editor_for(c, lanes, self.rack.grid),
                clips::rack_grid(c, t.instrument.lanes(), self.rack.grid),
            )
        };
        match which {
            ClipEditor::Automation => {
                self.open_automation_editor(track, clip);
                self.show_pane(Pane::PianoRoll);
                self.focus_piano_roll();
            }
            ClipEditor::Pitch => self.open_pitch_editor(track, clip),
            ClipEditor::PianoRoll => {
                self.select_clip(track, clip);
                self.central = CentralView::PianoRoll;
                self.show_pane(Pane::PianoRoll);
                crate::views::piano_roll::fit_clip(self, track, clip);
                self.focus_piano_roll();
            }
            ClipEditor::ChannelRack => {
                self.select_clip(track, clip);
                self.central = CentralView::ChannelRack;
                self.show_pane(Pane::ChannelRack);
                // At the clip's own grid, the whole pattern across the rack from its first
                // step, the clip's track scrolled into view.
                if let Some(g) = grid {
                    self.rack.grid = g;
                }
                self.rack.fit = true;
                self.rack.scroll_steps = 0.0;
                self.rack.reveal_track(track);
                self.focus_piano_roll();
            }
        }
        Some(which)
    }
}
