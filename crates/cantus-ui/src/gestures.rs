//! FL's quick gestures, from the application's side (round 27, ADR 0051): what a
//! double-click on a track header and on a clip open. The views decide *where* a gesture
//! landed; this decides what it means, so the arrangement, the clip browser and the tests
//! all reach the same editor by the same road.

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
    /// piano roll, or the channel rack for a step pattern ([`clips::editor_for`]), the
    /// automation editor for an automation clip, the inspector for an audio clip. Returns
    /// which, or `None` when the clip is gone.
    pub fn open_clip_editor(&mut self, track: TrackId, clip: ClipId) -> Option<ClipEditor> {
        let which = clips::editor_for(
            clips::clip_of(self.project(), (track, clip))?,
            self.rack.grid,
        );
        match which {
            ClipEditor::Automation => {
                self.open_automation_editor(track, clip);
                self.show_pane(Pane::PianoRoll);
                self.focus_piano_roll();
            }
            ClipEditor::Inspector => {
                self.select_clip(track, clip);
                self.show_pane(Pane::Inspector);
            }
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
                // The whole pattern across the rack, from its first step.
                self.rack.fit = true;
                self.rack.scroll_steps = 0.0;
                self.focus_piano_roll();
            }
        }
        Some(which)
    }
}
