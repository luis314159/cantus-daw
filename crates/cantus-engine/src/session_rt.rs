//! The session on the audio thread (ADR 0076 §7). The session plays as **projections** —
//! the song with what is launched written into its arrangement, made beside the engine
//! handle by the conductor (`cantus_core::session::play`) and installed as any snapshot is,
//! ahead of the boundary it plays from — so the audio thread has nothing of its own to run:
//! no launch, no follow action, nothing that could allocate or block.
//!
//! What it must do is play on **seamlessly** across those installs, so a performance played
//! live sounds as its offline render, to the bit: voices keep ringing across a snapshot
//! (they always have), event cursors reseek where they are, and — here — a snapshot that
//! arrives while the transport plays is installed on the automation's control grid
//! (`automation::GRID`, 16 samples), never between two of its points: there the automation
//! evaluates afresh exactly where the offline render evaluates too (or where it holds a
//! value it reads the same), so no ramp is cut short off the grid. The messages after it
//! wait with it, in order.

use super::*;
use crate::automation::GRID;

impl Processor {
    /// Whether a snapshot arriving now must wait for the control grid.
    pub(super) fn defer_install(&self) -> bool {
        self.transport.is_playing() && !self.transport.samples().is_multiple_of(GRID)
    }

    /// Whether a waiting snapshot is due: the transport on the grid (or stopped).
    pub(super) fn deferred_due(&self) -> bool {
        self.deferred.is_some() && !self.defer_install()
    }

    /// The waiting snapshot installed, and the messages after it drained.
    pub(super) fn deferred_install(&mut self) {
        if let Some((snap, plan)) = self.deferred.take() {
            self.install_snapshot(snap, plan);
            self.drain_messages();
        }
    }

    /// Where a pass must end at the latest while a snapshot waits: on the next grid point.
    pub(super) fn deferred_end(&self, done: usize, end: usize) -> usize {
        if self.deferred.is_none() || !self.defer_install() {
            return end;
        }
        let to = GRID - self.transport.samples() % GRID;
        end.min(done + to as usize)
    }
}

impl Processor {
    /// A snapshot installed while playing that ends, before the playhead, a note the old one
    /// had started and still had sounding (an edit, a projection that arrived late): its
    /// note-off is played now, or the note would ring on — its off is behind the playhead
    /// and the cursor reseeks past it. The notes are worked out where the snapshot was sent
    /// (`Snapshot::cut_from`, against the one playing then): this walks that list once —
    /// linear, bounded, nothing allocates. A note the old snapshot never started (pasted
    /// behind the playhead) is not in it.
    pub(super) fn release_cut_notes(&mut self) {
        if !self.transport.is_playing() {
            return;
        }
        let Some(new) = self.snapshot.clone() else {
            return;
        };
        // The first tick the cursor will play (`reseek_events`).
        let x = cantus_core::Ticks(self.transport.position_f64().ceil() as i64);
        for c in new.cuts.iter().filter(|c| c.due(x)) {
            let Some(&slot) = self.track_to_slot.get(c.track) else {
                continue;
            };
            let Some(mut s) = self.nodes.track(slot) else {
                continue;
            };
            if s.frozen.is_some() || s.notes_only || s.fx_active() {
                continue;
            }
            s.note_off(c.pitch, c.note.0, c.release);
        }
    }
}

impl Processor {
    /// The scene tempos this block plays through (`Snapshot::scene_tempos`), proposed to a
    /// followed Link session timed to when each tick is heard — Live's scene tempo at its
    /// launch (`propose_tempo`: nothing unless Link is followed). A walk of a short list;
    /// nothing allocates.
    pub(super) fn propose_scene_tempos(&mut self, frames: usize) {
        if !self.transport.is_playing() {
            return;
        }
        let Some(snap) = self.snapshot.clone() else {
            return;
        };
        if snap.scene_tempos.is_empty() {
            return;
        }
        let from = self.transport.position_f64();
        let to = from + self.transport.ticks_per_sample() * frames as f64;
        for &(t, bpm) in &snap.scene_tempos {
            let x = t.0 as f64;
            if x >= from && x < to {
                self.propose_tempo(bpm, x);
            }
        }
    }
}

impl crate::samples::SampleCache {
    /// Every session clip's audio source loaded — in the background when the engine is
    /// live — before any is launched (review finding 9): a launch then plays its audio from
    /// its first projection, never the silence of a file on its way. Kept as any loaded
    /// source is.
    pub fn preload_session(&mut self, project: &cantus_core::Project, rate: u32) {
        for t in &project.tracks {
            for c in t.session_clips() {
                let Some(a) = &c.clip.audio else {
                    continue;
                };
                if let Some(src) = project.audio_source(a.source) {
                    let _ = self.load_audio(&src.path, rate);
                }
            }
        }
    }

    /// The warp renders a launch of a session clip on a boundary plays (N5), asked for
    /// before any is launched and kept as what plays: the first launch of a warped clip
    /// plays its render, not the varispeed bridge (`session::edit::warp_ahead`).
    pub fn preload_session_warps(&mut self, project: &cantus_core::Project, rate: u32) {
        for q in cantus_core::session::edit::warp_ahead(project) {
            let mut snap = cantus_core::Snapshot::compile(&q, 0);
            self.attach_warp_ahead(&mut snap, rate);
        }
    }
}
