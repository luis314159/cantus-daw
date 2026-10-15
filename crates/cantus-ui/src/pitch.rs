//! The pitch editor, from the application's side (round 28, ADR 0054): opening it on an
//! audio clip, the take's analysis (asked of the engine's pitch service, off this thread),
//! every edit as one `SetClipPitch`, and what the editor makes — MIDI notes on an
//! instrument track and a double on a track of its own.
//!
//! The editor itself is `views/pitch.rs`; it opens in the piano roll's place, the way the
//! automation editor does (ADR 0027, ADR 0051).

use crate::app::{CantusApp, CentralView};
use crate::views::workspace::Pane;
use cantus_core::pitch::midi::{self, Interval};
use cantus_core::pitch::{PitchEdit, PitchOp};
use cantus_core::{AudioSource, Clip, ClipId, Command, MergeKey, Project, Ticks, TrackId};
use cantus_engine::pitch::TakeAnalysis;
use std::path::PathBuf;
use std::sync::Arc;

#[path = "pitch_demo.rs"]
pub mod demo;

/// What the editor knows about a clip's take.
#[derive(Clone, Debug)]
pub enum Take {
    Ready(Arc<TakeAnalysis>),
    /// Being analysed.
    Pending,
    /// The file could not be read (the reason).
    Failed(String),
    /// Not an audio clip, or its source is gone.
    None,
}

/// The editor's state, kept between frames on the app.
#[derive(Debug, Default)]
pub struct PitchUi {
    pub view: crate::views::pitch::PitchView,
    /// The arrangement's pitch lanes (review 1, §3c).
    pub lane: crate::views::pitch_lane::LaneState,
}

/// Where a clip's source frames sit on the timeline: content tick `c` reads source frame
/// `seconds(c) · rate + slip` (ADR 0025 §7), at the project's tempo — mirrored inside the
/// window when the clip plays backwards, as the engine reads it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ClipTime {
    pub start: Ticks,
    pub offset: Ticks,
    pub slip: i32,
    pub rate: u32,
    pub bpm: f64,
    /// Played backwards over the window that ends at `end`.
    pub reverse: bool,
    pub end: Ticks,
}

impl ClipTime {
    pub fn of(project: &Project, clip: &Clip, source: &AudioSource) -> ClipTime {
        ClipTime {
            start: clip.start,
            offset: clip.offset,
            slip: clip.audio.as_ref().map_or(0, |a| a.slip_frames),
            rate: source.sample_rate.max(1),
            bpm: project.tempo_bpm,
            reverse: clip.audio.as_ref().is_some_and(|a| a.reverse),
            end: clip.end(),
        }
    }

    fn forward_frame(&self, t: Ticks) -> f64 {
        let content = t - self.start + self.offset;
        content.to_seconds(self.bpm) * self.rate as f64 + self.slip as f64
    }

    fn forward_tick(&self, f: f64) -> Ticks {
        let content = Ticks::from_seconds((f - self.slip as f64) / self.rate as f64, self.bpm);
        content + self.start - self.offset
    }

    /// The source frame under timeline tick `t`.
    pub fn frame(&self, t: Ticks) -> f64 {
        if self.reverse {
            let (a, b) = (self.forward_frame(self.start), self.forward_frame(self.end));
            a + b - self.forward_frame(t)
        } else {
            self.forward_frame(t)
        }
    }

    /// The timeline tick at source frame `f`.
    pub fn tick(&self, f: f64) -> Ticks {
        if self.reverse {
            let (a, b) = (self.forward_frame(self.start), self.forward_frame(self.end));
            self.forward_tick(a + b - f)
        } else {
            self.forward_tick(f)
        }
    }
}

impl CantusApp {
    /// Open the pitch editor on an audio clip: selected, in the piano roll's place, in
    /// front, with the focus, fitted to the clip — and its take analysed if it is not yet.
    pub fn open_pitch_editor(&mut self, track: TrackId, clip: ClipId) {
        self.select_clip(track, clip);
        self.central = CentralView::PianoRoll;
        self.show_pane(Pane::PianoRoll);
        self.pitch.view.fit_pending = true;
        self.focus_piano_roll();
        let _ = self.pitch_take(track, clip);
    }

    /// The clip's source and its file, resolved against the project's folder.
    pub fn pitch_source(&self, track: TrackId, clip: ClipId) -> Option<(AudioSource, PathBuf)> {
        let c = self.project().track(track)?.clip(clip)?;
        let a = c.audio.as_ref()?;
        let s = self.project().audio_source(a.source)?.clone();
        let path = crate::waveform::resolve(self.audio_base(), &s.path);
        Some((s, path))
    }

    /// The take's analysis, asked of the engine's pitch service (queued the first time).
    pub fn pitch_take(&self, track: TrackId, clip: ClipId) -> Take {
        let Some((s, path)) = self.pitch_source(track, clip) else {
            return Take::None;
        };
        // An edit made at the file's rate is read at that rate, whatever the file says now.
        let rate = self
            .stored_pitch(track, clip)
            .map_or(s.sample_rate, |e| e.rate);
        match self.engine.samples.pitch().lookup(&path, rate) {
            cantus_engine::pitch::Lookup::Ready(a) => Take::Ready(a),
            cantus_engine::pitch::Lookup::Pending => Take::Pending,
            cantus_engine::pitch::Lookup::Failed(e) => Take::Failed(e),
        }
    }

    /// The clip's stored pitch edit, if it has one.
    pub fn stored_pitch(&self, track: TrackId, clip: ClipId) -> Option<&PitchEdit> {
        self.project()
            .track(track)?
            .clip(clip)?
            .audio
            .as_ref()?
            .pitch
            .as_deref()
    }

    /// What the editor shows and edits: the stored edit, or the take's notes untouched.
    pub fn pitch_edit(&self, track: TrackId, clip: ClipId, take: &Take) -> Option<PitchEdit> {
        if let Some(e) = self.stored_pitch(track, clip) {
            return Some(e.clone());
        }
        match take {
            Take::Ready(a) => Some(a.edit()),
            _ => None,
        }
    }

    /// Write a pitch edit: one undo step, or merged into the gesture's when `merge` (a
    /// drag). The first edit of a take stores its notes with it (ADR 0054 §2).
    pub fn commit_pitch(
        &mut self,
        track: TrackId,
        clip: ClipId,
        edit: Option<PitchEdit>,
        op: PitchOp,
        merge: bool,
    ) -> bool {
        let cmd = Command::SetClipPitch {
            track,
            clip,
            pitch: edit.map(Box::new),
            op,
        };
        if merge {
            self.exec_merging(cmd, MergeKey::of(&("pitch", clip.0, op.label_key())))
        } else {
            self.exec(cmd)
        }
    }

    /// The clip's frames ↔ ticks.
    pub fn clip_time(&self, track: TrackId, clip: ClipId) -> Option<ClipTime> {
        let c = self.project().track(track)?.clip(clip)?;
        let (s, _) = self.pitch_source(track, clip)?;
        Some(ClipTime::of(self.project(), c, &s))
    }

    /// MIDI out: the notes as they sound, in a new note clip on `onto` (an instrument
    /// track) at the voice's place on the timeline — or on a new instrument track under
    /// the voice's. One undo step. Returns the clip made.
    pub fn pitch_to_midi(
        &mut self,
        track: TrackId,
        clip: ClipId,
        onto: Option<TrackId>,
    ) -> Option<(TrackId, ClipId)> {
        let take = self.pitch_take(track, clip);
        let edit = self.pitch_edit(track, clip, &take)?;
        let time = self.clip_time(track, clip)?;
        // Each note's edges on the timeline (a clip played backwards turns them round),
        // only what the clip plays: its window (one period of a loop, repeated to its end),
        // a note across an edge cut there.
        let (cs, ce, period) = {
            let c = self.project().track(track)?.clip(clip)?;
            (
                c.start,
                c.end(),
                c.period().filter(|p| *p < c.length).unwrap_or(c.length),
            )
        };
        let mut notes: Vec<(Ticks, Ticks, u8, u8)> = Vec::new();
        for n in midi::notes(&edit) {
            let (a, b) = (time.tick(n.start as f64), time.tick(n.end as f64));
            let (a, b) = (a.min(b).max(cs), a.max(b).min(cs + period));
            if b <= a {
                continue;
            }
            let mut k = Ticks::ZERO;
            while cs + k < ce && period > Ticks::ZERO {
                let (x, y) = (a + k, (b + k).min(ce));
                if y > x {
                    notes.push((x, y, n.pitch, n.velocity));
                }
                k += period;
            }
        }
        notes.sort_by_key(|n| (n.0, n.2));
        let (Some(first), Some(last)) = (
            notes.iter().map(|n| n.0).min(),
            notes.iter().map(|n| n.1).max(),
        ) else {
            return None;
        };
        let bar = self.project().time_signature.ticks_per_bar().max(Ticks(1));
        let start = first.snap_floor(bar).max(Ticks::ZERO);
        let length = Ticks(((last - start).0 + bar.0 - 1) / bar.0 * bar.0).max(bar);
        let name = self
            .project()
            .track(track)
            .and_then(|t| t.clip(clip))
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let mut c = self.bus.new_clip(format!("{name} · MIDI"), start, length);
        for &(a, b, pitch, velocity) in &notes {
            let a = (a - start).max(Ticks::ZERO);
            let b = (b - start).max(a + Ticks(1));
            c.notes.push(self.bus.new_note(a, b - a, pitch, velocity));
        }
        c.notes.sort_by_key(|n| (n.start, n.pitch));
        let clip_id = c.id;
        let mut commands = Vec::new();
        let target = match onto.filter(|t| {
            self.project()
                .track(*t)
                .is_some_and(|t| !crate::record_session::is_audio_track(t))
        }) {
            Some(t) => t,
            None => {
                let index = self
                    .project()
                    .track_index(track)
                    .map_or(self.project().tracks.len(), |i| i + 1);
                let mut t = self.bus.new_track(format!("{name} · MIDI"));
                t.strip = crate::audio_files::free_strips(self.project())
                    .first()
                    .copied();
                let id = t.id;
                commands.push(Command::InsertTrack {
                    index,
                    track: Box::new(t),
                });
                id
            }
        };
        commands.push(Command::InsertClip {
            track: target,
            clip: Box::new(c),
        });
        self.exec(Command::Batch {
            label_key: "cmd.pitch.midi".into(),
            commands,
        })
        .then_some((target, clip_id))
    }

    /// *Double*: a second voice `interval` above, from the same take, in a new clip on a
    /// new track under the voice's (its own fader, so the blend is the musician's). One
    /// undo step. Returns the clip made.
    pub fn pitch_double(
        &mut self,
        track: TrackId,
        clip: ClipId,
        interval: Interval,
    ) -> Option<(TrackId, ClipId)> {
        let take = self.pitch_take(track, clip);
        let edit = self.pitch_edit(track, clip, &take)?;
        let (src_track, src_clip) = {
            let t = self.project().track(track)?;
            (t.clone(), t.clip(clip)?.clone())
        };
        let index = self.project().track_index(track)? + 1;
        let label = crate::i18n::t(interval.label_key());
        let mut t = self.bus.new_track(format!("{} · {label}", src_track.name));
        t.color = src_track.color;
        t.strip = crate::audio_files::free_strips(self.project())
            .first()
            .copied();
        let tid = t.id;
        let mut c = self.bus.new_clip(
            format!("{} · {label}", src_clip.name),
            src_clip.start,
            src_clip.length,
        );
        c.offset = src_clip.offset;
        c.content_length = src_clip.content_length;
        c.color = src_clip.color;
        let mut audio = src_clip.audio.clone()?;
        audio.pitch = Some(Box::new(midi::double(&edit, interval)));
        c.audio = Some(audio);
        let cid = c.id;
        self.exec(Command::Batch {
            label_key: "cmd.pitch.double".into(),
            commands: vec![
                Command::InsertTrack {
                    index,
                    track: Box::new(t),
                },
                Command::InsertClip {
                    track: tid,
                    clip: Box::new(c),
                },
            ],
        })
        .then_some((tid, cid))
    }

    /// The pitch editor is what the edit keys act on: the piano roll's place has the focus
    /// and shows an audio clip.
    pub fn pitch_editing(&self) -> Option<(TrackId, ClipId)> {
        if self.focus != crate::app::EditFocus::PianoRoll
            || !self.workspace.is_visible(Pane::PianoRoll)
        {
            return None;
        }
        let (t, c) = self.selected_track.zip(self.selected_clip)?;
        self.project()
            .track(t)?
            .clip(c)?
            .audio
            .is_some()
            .then_some((t, c))
    }

    /// The keymap's edit actions, in the pitch editor (the same keys as in the piano roll):
    /// Ctrl+A selects every note, Esc lets them go (and never the clip), ↑/↓ move the
    /// selection a step of the snap, Shift an octave, Ctrl+J joins, Q quantises. Returns
    /// whether the action was the editor's.
    pub fn pitch_action(&mut self, action: crate::keymap::Action) -> bool {
        use crate::keymap::Action;
        use cantus_core::pitch::edit;
        let Some((track, clip)) = self.pitch_editing() else {
            return false;
        };
        let take = self.pitch_take(track, clip);
        let Some(current) = self.pitch_edit(track, clip, &take) else {
            return matches!(action, Action::ClearSelection | Action::SelectAll);
        };
        let sel = self.pitch.view.selected.clone();
        let step = |app: &mut CantusApp, dir: i32, octave: bool| {
            if sel.is_empty() {
                return;
            }
            let mut e = current.clone();
            for n in current.notes.iter().filter(|n| sel.contains(&n.id)) {
                let to = if octave {
                    n.pitch + 12.0 * dir as f32
                } else if app.pitch.view.snap == crate::views::pitch::Snap::Scale {
                    current.key.step(n.pitch, dir)
                } else {
                    n.pitch + dir as f32
                };
                edit::set_pitch(&mut e, n.id, to);
            }
            app.commit_pitch(track, clip, Some(e), PitchOp::Pitch, false);
        };
        match action {
            Action::SelectAll => {
                self.pitch.view.selected = current.notes.iter().map(|n| n.id).collect();
            }
            Action::ClearSelection => self.pitch.view.selected.clear(),
            Action::TransposeUp => step(self, 1, false),
            Action::TransposeDown => step(self, -1, false),
            Action::TransposeOctaveUp => step(self, 1, true),
            Action::TransposeOctaveDown => step(self, -1, true),
            Action::GlueClips => {
                let mut e = current.clone();
                if edit::join(&mut e, &sel) {
                    self.pitch.view.selected.truncate(1);
                    self.commit_pitch(track, clip, Some(e), PitchOp::Join, false);
                } else {
                    self.set_status(crate::i18n::t("pitch.join_needs"));
                }
            }
            Action::Quantize => {
                let ids: Vec<u32> = if sel.is_empty() {
                    current.notes.iter().map(|n| n.id).collect()
                } else {
                    sel
                };
                crate::views::pitch::quantise(self, track, clip, &current, &ids);
            }
            _ => return false,
        }
        true
    }

    /// Play from the selection's earliest note on the timeline (a beat early), or the
    /// clip's start.
    pub fn pitch_play_from_selection(&mut self, track: TrackId, clip: ClipId) {
        let take = self.pitch_take(track, clip);
        let edit = self.pitch_edit(track, clip, &take);
        let sel = &self.pitch.view.selected;
        let first = match (edit, self.clip_time(track, clip)) {
            (Some(e), Some(tm)) => e
                .notes
                .iter()
                .filter(|n| sel.contains(&n.id))
                .map(|n| tm.tick(n.start as f64).min(tm.tick(n.end as f64)))
                .min(),
            _ => None,
        };
        let at = match first {
            Some(t) => {
                let beat = self.project().time_signature.ticks_per_beat();
                (t - beat).max(Ticks::ZERO)
            }
            None => self
                .project()
                .track(track)
                .and_then(|t| t.clip(clip))
                .map_or(Ticks::ZERO, |c| c.start),
        };
        self.seek(at);
        if !self.engine.is_playing() {
            self.toggle_play();
        }
    }

    /// MIDI out (`None`) or a double (`Some(interval)`) of a clip, from its menu: said in
    /// the status line, or that the take is still being analysed.
    pub fn pitch_make(&mut self, track: TrackId, clip: ClipId, double: Option<Interval>) {
        let made = match double {
            None => self.pitch_to_midi(track, clip, None),
            Some(i) => self.pitch_double(track, clip, i),
        };
        let name = |a: &CantusApp, (t, c): (TrackId, ClipId)| {
            a.project()
                .track(t)
                .and_then(|t| t.clip(c))
                .map(|c| c.name.clone())
                .unwrap_or_default()
        };
        match made {
            Some(m) => {
                let key = if double.is_some() {
                    "pitch.doubled"
                } else {
                    "pitch.midi_made"
                };
                let n = name(self, m);
                self.set_status(crate::i18n::tf(key, &[("name", &n)]));
            }
            None => self.set_status(crate::i18n::t("pitch.not_ready")),
        }
    }

    /// Where another track's notes start on the timeline: a note clip's notes as they
    /// play (loops repeated), an audio clip's pitch notes (stored or, analysed, found) —
    /// what *Quantise* moves notes onto when it quantises to another track (§3c).
    pub fn pitch_onsets(&self, other: TrackId) -> Vec<Ticks> {
        let mut out: Vec<Ticks> = Vec::new();
        let snap = cantus_core::Snapshot::compile(self.project(), 0);
        if let Some(t) = snap.tracks.iter().find(|t| t.id == other) {
            out.extend(t.events.iter().filter(|e| e.velocity > 0).map(|e| e.tick));
        }
        if let Some(t) = self.project().track(other) {
            for c in t
                .clips
                .iter()
                .filter(|c| c.audio.is_some() && c.lane == 0 && !c.muted)
            {
                let take = self.pitch_take(other, c.id);
                let (Some(e), Some(tm)) = (
                    self.pitch_edit(other, c.id, &take),
                    self.clip_time(other, c.id),
                ) else {
                    continue;
                };
                for n in &e.notes {
                    let (a, b) = (tm.tick(n.start as f64), tm.tick(n.end as f64));
                    let at = a.min(b);
                    if at >= c.start && at < c.end() {
                        out.push(at);
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// The tracks *Quantise* can move `track`'s notes onto: every other track with notes.
    pub fn pitch_onset_tracks(&self, track: TrackId) -> Vec<(TrackId, String)> {
        self.project()
            .tracks
            .iter()
            .filter(|t| t.id != track)
            .filter(|t| {
                t.clips.iter().any(|c| {
                    !c.notes.is_empty() || c.audio.as_ref().is_some_and(|a| a.pitch.is_some())
                })
            })
            .map(|t| (t.id, t.name.clone()))
            .collect()
    }

    /// A take landed (a recording, an import): analyse it now, on the worker, so the
    /// editor opens on its notes at once.
    pub fn analyse_take_soon(&self, source: &AudioSource) {
        let path = crate::waveform::resolve(self.audio_base(), &source.path);
        self.analyse_file_soon(&path, source.sample_rate);
    }

    /// [`Self::analyse_take_soon`] for a file by its path and rate (a finished take).
    pub fn analyse_file_soon(&self, path: &std::path::Path, rate: u32) {
        let _ = self.engine.samples.pitch().lookup(path, rate);
    }
}
