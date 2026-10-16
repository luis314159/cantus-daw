//! The recording workflow, application side (ADR 0025): arming, inputs and monitoring,
//! the record options, and a pass from the record button to the clips it leaves.
//!
//! - **What records.** The record button records every armed track at once: audio from
//!   its input on a track that has one, the notes played on one that does not. With
//!   nothing armed it is the quick record every earlier version had: the notes go into the
//!   selected track, merged into its clip.
//! - **A pass.** On record: `begin_recording(dir)` for the audio, the count-in by seeking
//!   back and `set_record_start`, then play with recording on. While it runs, each armed
//!   track's take grows in red with its live waveform (`poll_recording`), and the notes
//!   are drawn as they are closed. On stop: `finish_recording`, and **one undo step** that
//!   puts the files in the pool and the clips on their lanes (`record_pass`).
//! - **Where the files go.** `<project folder>/Audio` once the project has a path; before,
//!   a scratch folder of this session under the app's data folder (saving moves them in).
//!
//! The methods are `CantusApp`'s, written here so the application file keeps only the
//! calls into them.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::record_pass::{self, PassPlan};
use crate::recording::RecordedNote;
use cantus_core::audio::{MonitorMode, RecordSettings, TakeMode, TrackInput};
use cantus_core::{Command, MergeKey, Ticks, TrackIcon, TrackId};
use cantus_engine::{FinishedTake, NoteInput, PEAK_FRAMES};
use std::path::PathBuf;

/// The recording state that is not the project's: a pass in progress, and what the
/// interface needs to draw one.
#[derive(Debug, Default)]
pub struct RecSession {
    pub pass: Option<Live>,
    /// Input levels to show when no device is open — the screenshots' and the tests' input
    /// device. Its length is the channel count.
    pub demo_inputs: Option<Vec<f32>>,
    /// The folder an unsaved project records into this session (made on first use).
    scratch: Option<PathBuf>,
    /// Frames the pass lost to a full ring (the disk fell behind): counted while it runs
    /// (`EngineHandle::record_dropped`), shown until the next pass.
    pub dropped: u64,
    /// The armed instrument tracks that play the live notes with the live track while a
    /// pass records them (`EngineHandle::set_live_echo`).
    pub echo: Vec<TrackId>,
    /// A count-in is under way with the metronome off (or set to click only while
    /// recording): the engine's snapshots click during the count-in anyway.
    pub count_in_click: bool,
}

/// A pass in progress.
#[derive(Debug)]
pub struct Live {
    pub plan: PassPlan,
    /// Where the transport started: before `plan.record_start` by the count-in.
    pub transport_start: Ticks,
    /// The engine is recording audio this pass.
    pub audio: bool,
    pub takes: Vec<LiveTake>,
    /// The notes closed so far.
    pub notes: Vec<RecordedNote>,
    /// The playhead last frame, to see the loop wrap.
    pub last_pos: Ticks,
    /// Where the pass being played began (the record start, then the loop's start).
    pub pass_start: Ticks,
    /// The engine has said its transport runs since the pass began. Until then a
    /// *stopped* from it is the word from before the *Play* was taken, not a stop.
    pub transport_seen: bool,
}

/// One armed track's take as it grows.
#[derive(Debug, Clone)]
pub struct LiveTake {
    pub track: TrackId,
    pub frames: u64,
    /// Min/max of every `PEAK_FRAMES` frames since the pass began.
    pub peaks: Vec<(f32, f32)>,
    /// Where in `peaks` the loop pass being played begins.
    pub pass_from: usize,
}

impl Live {
    /// The peaks of the loop pass being played, oldest first.
    pub fn current_peaks(&self, track: TrackId) -> Option<&[(f32, f32)]> {
        self.takes
            .iter()
            .find(|t| t.track == track)
            .map(|t| &t.peaks[t.pass_from.min(t.peaks.len())..])
    }

    pub fn records(&self, track: TrackId) -> bool {
        self.plan.audio_tracks.contains(&track)
            || self.plan.midi_tracks.contains(&track)
            || self.plan.quick.is_some_and(|(t, _)| t == track)
    }
}

/// "In 1", "In 1-2": a track input as the header and the menus show it.
pub fn input_label(input: &TrackInput, b: &mut String) {
    use std::fmt::Write as _;
    // The interface's own names for its channels, when given (round 20).
    if crate::audio_setup::names::write_input(input.first as usize, input.stereo, b) {
        return;
    }
    let n = input.first as u32 + 1;
    if input.stereo {
        let _ = write!(b, "{} {n}-{}", t("rec.in"), n + 1);
    } else {
        let _ = write!(b, "{} {n}", t("rec.in"));
    }
}

/// Every input a track can take on a device with `channels` inputs: each mono channel,
/// then each stereo pair (1-2, 3-4, …), as FL's and Cubase's input menus list them.
pub fn input_choices(channels: usize) -> Vec<TrackInput> {
    let mut v: Vec<TrackInput> = (0..channels as u16).map(TrackInput::mono).collect();
    v.extend((0..channels as u16 / 2).map(|i| TrackInput::stereo(i * 2)));
    v
}

/// An *audio track* (ADR 0025 §3): one with an input and no note clips. Its instrument
/// is never played, so the interface shows its input and its audio instead.
pub fn is_audio_track(track: &cantus_core::Track) -> bool {
    track.input.is_some() && track.clips.iter().all(|c| c.audio.is_some())
}

/// The audio clip the inspector shows for an audio track: the selected one when it is
/// on that track, else its first clip on the playing lane.
pub fn inspected_audio_clip(app: &CantusApp, track: TrackId) -> Option<cantus_core::ClipId> {
    let t = app.project().track(track)?;
    let audio = |c: &&cantus_core::Clip| c.audio.is_some();
    if let Some(c) = app.selected_clip.and_then(|c| t.clip(c)).filter(audio) {
        return Some(c.id);
    }
    t.clips
        .iter()
        .filter(audio)
        .filter(|c| c.lane == 0)
        .min_by_key(|c| c.start)
        .map(|c| c.id)
}

/// The monitor mode after `m`, as the header's button cycles them.
pub fn next_monitor(m: MonitorMode) -> MonitorMode {
    match m {
        MonitorMode::Off => MonitorMode::Auto,
        MonitorMode::Auto => MonitorMode::On,
        MonitorMode::On => MonitorMode::Off,
    }
}

pub fn monitor_key(m: MonitorMode) -> &'static str {
    match m {
        MonitorMode::Off => "rec.monitor.off",
        MonitorMode::Auto => "rec.monitor.auto",
        MonitorMode::On => "rec.monitor.on",
    }
}

pub fn take_mode_key(m: TakeMode) -> &'static str {
    match m {
        TakeMode::Stack => "rec.mode.stack",
        TakeMode::Replace => "rec.mode.replace",
        TakeMode::Overdub => "rec.mode.overdub",
    }
}

impl CantusApp {
    // ---- inputs ------------------------------------------------------------------------

    /// Inputs on the open device (or the demo device).
    pub fn input_channels(&self) -> usize {
        match &self.rec.demo_inputs {
            Some(d) => d.len(),
            None => self.engine.input_channel_count(),
        }
    }

    /// The level of a track's input this frame, 0…1 linear (the louder side of a pair).
    pub fn input_level(&self, input: &TrackInput) -> f32 {
        let at = |ch: usize| match &self.rec.demo_inputs {
            Some(d) => d.get(ch).copied().unwrap_or(0.0),
            None => {
                let m = self.engine.meters();
                if ch < m.input_count.min(m.inputs.len()) {
                    m.inputs[ch]
                } else {
                    0.0
                }
            }
        };
        let first = input.first as usize;
        if input.stereo {
            at(first).max(at(first + 1))
        } else {
            at(first)
        }
    }

    /// Where this project records: its `Audio` folder once saved, else this session's
    /// scratch folder.
    pub fn audio_record_dir(&mut self) -> PathBuf {
        if let Some(p) = &self.path {
            return p.with_file_name("Audio");
        }
        if self.rec.scratch.is_none() {
            let stamp = crate::takes::stamp_now();
            self.rec.scratch = Some(
                crate::app::data_dir()
                    .join("recordings")
                    .join(format!("session {stamp}")),
            );
        }
        self.rec.scratch.clone().unwrap_or_default()
    }

    // ---- arming, monitoring, inputs ----------------------------------------------------

    /// Arm or disarm a track; `exclusive` (Shift+click) arms it alone.
    pub fn toggle_arm(&mut self, id: TrackId, exclusive: bool) {
        let Some(on) = self.project().track(id).map(|t| t.armed) else {
            return;
        };
        let mut cmds = Vec::new();
        if exclusive {
            for t in &self.project().tracks {
                if t.id != id && t.armed {
                    cmds.push(Command::SetTrackArmed {
                        id: t.id,
                        armed: false,
                    });
                }
            }
            cmds.push(Command::SetTrackArmed { id, armed: true });
        } else {
            cmds.push(Command::SetTrackArmed { id, armed: !on });
        }
        let mut cmds: Vec<Command> = cmds
            .into_iter()
            .filter(|c| match c {
                Command::SetTrackArmed { id, armed } => {
                    self.project().track(*id).is_some_and(|t| t.armed != *armed)
                }
                _ => true,
            })
            .collect();
        match cmds.len() {
            0 => {}
            1 => {
                if let Some(c) = cmds.pop() {
                    self.exec(c);
                }
            }
            _ => {
                self.exec(Command::Batch {
                    label_key: "cmd.set_track_armed".into(),
                    commands: cmds,
                });
            }
        }
    }

    /// The arm key: the selected track.
    pub fn arm_selected(&mut self) {
        if let Some(id) = self.selected_track {
            self.toggle_arm(id, false);
        }
    }

    /// Set or clear a track's input (keeping its monitor mode and trim).
    pub fn set_track_input(&mut self, id: TrackId, input: Option<TrackInput>) {
        let Some(old) = self.project().track(id).map(|t| t.input.clone()) else {
            return;
        };
        let input = match (input, &old) {
            (Some(mut new), Some(old)) => {
                new.monitor = old.monitor;
                new.trim_db = old.trim_db;
                Some(new)
            }
            (new, _) => new,
        };
        if input != old {
            self.exec(Command::SetTrackInput { id, input });
        }
    }

    /// Off → Auto → On on the header's monitor button.
    pub fn cycle_monitor(&mut self, id: TrackId) {
        let Some(Some(mut input)) = self.project().track(id).map(|t| t.input.clone()) else {
            return;
        };
        input.monitor = next_monitor(input.monitor);
        self.exec(Command::SetTrackInput {
            id,
            input: Some(input),
        });
        self.end_gesture();
    }

    pub fn set_monitor(&mut self, id: TrackId, mode: MonitorMode) {
        let Some(Some(mut input)) = self.project().track(id).map(|t| t.input.clone()) else {
            return;
        };
        if input.monitor != mode {
            input.monitor = mode;
            self.exec(Command::SetTrackInput {
                id,
                input: Some(input),
            });
            self.end_gesture();
        }
    }

    /// The input trim, merged while it is dragged.
    pub fn set_input_trim(&mut self, id: TrackId, db: f32) {
        let Some(Some(mut input)) = self.project().track(id).map(|t| t.input.clone()) else {
            return;
        };
        input.trim_db = db.clamp(-24.0, 24.0);
        self.exec_merging(
            Command::SetTrackInput {
                id,
                input: Some(input),
            },
            MergeKey::of(&("trim", id.0)),
        );
    }

    /// *Add audio track*: a track with the first input (the first stereo pair on a
    /// two-input device), monitor Auto, a microphone, no clips — armed and selected, ready
    /// to record into.
    pub fn add_audio_track(&mut self) {
        let n = self
            .project()
            .tracks
            .iter()
            .filter(|t| t.input.is_some())
            .count()
            + 1;
        let mut track = self.bus.new_track(tf("rec.audio_track_name", &[("n", &n)]));
        track.input = Some(if self.input_channels() == 2 {
            TrackInput::stereo(0)
        } else {
            TrackInput::mono(0)
        });
        track.icon = Some(TrackIcon::Vocal);
        // Ready to record (round 22, the manual's friction): armed, and heard while it is
        // armed or recording (monitoring *Auto*), as a new audio track is in every DAW.
        track.armed = true;
        if let Some(i) = track.input.as_mut() {
            i.monitor = MonitorMode::Auto;
        }
        let id = track.id;
        let index = self
            .selected_track
            .and_then(|s| self.project().tracks.iter().position(|t| t.id == s))
            .map(|i| i + 1)
            .unwrap_or(self.project().tracks.len());
        if self.exec(Command::InsertTrack {
            index,
            track: Box::new(track),
        }) {
            self.select_track(id);
        }
    }

    /// Show or hide a track's take lanes.
    pub fn toggle_takes(&mut self, id: TrackId) {
        if let Some(open) = self.project().track(id).map(|t| t.takes_open) {
            self.exec(Command::SetTrackTakesOpen { id, open: !open });
        }
    }

    /// The show-takes key: the selected track.
    pub fn toggle_selected_takes(&mut self) {
        if let Some(id) = self.selected_track {
            self.toggle_takes(id);
        }
    }

    // ---- record options ----------------------------------------------------------------

    /// Change how recording behaves; a run of changes from one control merges.
    pub fn set_record_settings(&mut self, settings: RecordSettings) {
        if settings != self.project().recording {
            self.exec_merging(
                Command::SetRecordSettings { settings },
                MergeKey::of(&"record-settings"),
            );
        }
    }

    /// Count-in on (the last length used, two bars the first time) or off.
    pub fn toggle_count_in(&mut self) {
        let mut s = self.project().recording.clone();
        s.count_in_bars = if s.count_in_bars > 0 { 0 } else { 2 };
        self.set_record_settings(s);
        self.end_gesture();
    }

    /// Punch on or off; turning it on with no range takes the time selection, else the
    /// loop, else the two bars from the playhead.
    pub fn toggle_punch(&mut self) {
        let mut s = self.project().recording.clone();
        s.punch_enabled = !s.punch_enabled;
        if s.punch_enabled && s.punch.is_none() {
            s.punch = Some(self.default_punch());
        }
        self.set_record_settings(s);
        self.end_gesture();
    }

    fn default_punch(&self) -> cantus_core::LoopRegion {
        if let Some((a, b)) = self.time_selection {
            return cantus_core::LoopRegion { start: a, end: b };
        }
        if let Some(l) = self.project().loop_region.clone() {
            return l;
        }
        // Two bars from the playhead's bar, through the metre changes (ADR 0060).
        let a = crate::views::bars::bar_start(self.project(), self.engine.position());
        cantus_core::LoopRegion {
            start: a,
            end: crate::views::bars::bars_from(self.project(), a, 2),
        }
    }

    /// *Set punch from selection*: the punch range becomes the time selection, and punch
    /// turns on.
    pub fn punch_from_selection(&mut self) {
        let Some((a, b)) = self.time_selection else {
            self.set_status(t("rec.no_selection"));
            return;
        };
        let mut s = self.project().recording.clone();
        s.punch = Some(cantus_core::LoopRegion { start: a, end: b });
        s.punch_enabled = true;
        self.set_record_settings(s);
        self.end_gesture();
    }
}

impl CantusApp {
    // ---- a pass ------------------------------------------------------------------------

    pub fn is_recording(&self) -> bool {
        self.rec.pass.is_some() || self.recorder.is_recording()
    }

    /// The armed tracks: those with an input record audio, the others notes.
    pub fn armed_tracks(&self) -> (Vec<TrackId>, Vec<TrackId>) {
        let mut audio = Vec::new();
        let mut midi = Vec::new();
        for t in self.project().tracks.iter().filter(|t| t.armed) {
            if t.input.is_some() {
                audio.push(t.id);
            } else {
                midi.push(t.id);
            }
        }
        (audio, midi)
    }

    fn loop_now(&self) -> Option<(Ticks, Ticks)> {
        let p = self.project();
        p.loop_region
            .as_ref()
            .filter(|_| p.loop_enabled)
            .filter(|l| l.end > l.start)
            .map(|l| (l.start, l.end))
    }

    /// Record every armed track (or, with none armed, the selected one's notes), with the
    /// count-in, punch, loop and take mode of the project's record options.
    pub fn start_recording(&mut self) {
        if self.is_recording() {
            return;
        }
        let (audio, midi) = self.armed_tracks();
        let rs = self.project().recording.clone();
        let pos = self.engine.position();
        let mut plan = PassPlan::new(pos);
        plan.punch = rs
            .punch
            .as_ref()
            .filter(|_| rs.punch_enabled)
            .filter(|r| r.end > r.start)
            .map(|r| (r.start, r.end));
        plan.loop_region = self.loop_now();
        plan.loop_takes = rs.loop_takes;
        plan.mode = rs.take_mode;
        plan.take_word = t("rec.take");
        if audio.is_empty() && midi.is_empty() {
            let Some(track) = self.selected_track else {
                self.set_status(t("rec.nothing_armed"));
                return;
            };
            plan.quick = Some((track, self.selected_clip));
        } else {
            plan.audio_tracks = audio;
            plan.midi_tracks = midi;
        }
        let mut audio_on = false;
        // The files already in the pass's folder, to see which ones this pass makes (the
        // session journal keeps them for a recovery).
        let mut journal_dir = None;
        if !plan.audio_tracks.is_empty() {
            if self.input_channels() == 0 {
                // Honest about it, and one click from the cure: the audio popover, where
                // the input is chosen in place (round 20).
                self.set_error(t("rec.no_input"));
                self.audio_setup.popover = true;
                self.audio_setup.request_scan();
                plan.audio_tracks.clear();
            } else {
                let dir = self.audio_record_dir();
                let made = std::fs::create_dir_all(&dir).map_err(|e| e.to_string());
                let before = if self.session.is_some() {
                    crate::recovery_session::wavs_in(&dir)
                } else {
                    Vec::new()
                };
                match made.and_then(|()| self.engine.begin_recording(&dir)) {
                    Ok(n) if n > 0 => {
                        audio_on = true;
                        journal_dir = Some((dir, before));
                    }
                    Ok(_) => {
                        self.set_error(t("rec.no_input"));
                        plan.audio_tracks.clear();
                    }
                    Err(e) => {
                        self.set_error(tf("rec.failed", &[("error", &e)]));
                        plan.audio_tracks.clear();
                    }
                }
            }
            if plan.audio_tracks.is_empty() && plan.midi_tracks.is_empty() {
                return;
            }
        }
        // The count-in: start that many bars early; the take begins where the playhead was.
        let bars = rs.count_in_bars as i64;
        let transport_start = if bars > 0 && !self.engine.is_playing() {
            // Before bar 1 the transport pre-rolls into negative time, so the whole
            // count-in is heard; the take still starts at `pos`. The bars counted are the
            // ones before `pos` (ADR 0060: through the metre changes; before tick 0 the
            // first metre).
            let from = crate::views::bars::add_bars(self.project(), pos, -bars);
            self.engine.seek(from);
            self.engine.set_record_start(Some(pos));
            // A count-in clicks whatever the metronome switch says (round 22): the engine
            // gets a snapshot with the click on, counting in only, until the pass ends.
            if !self.project().metronome {
                self.rec.count_in_click = true;
                self.force_resync();
                self.sync_engine();
            }
            from
        } else {
            self.engine.set_record_start(None);
            pos
        };
        // The notes of the armed instrument track are heard: it becomes the live track.
        if let Some(first) = plan.midi_tracks.first().copied() {
            if self.selected_track != Some(first) && plan.audio_tracks.is_empty() {
                self.select_track(first);
            }
        }
        self.recorder.start();
        // Every armed instrument track is heard as it is recorded into, not only the
        // live one (multitrack MIDI recording).
        if !plan.midi_tracks.is_empty() {
            self.engine.set_live_echo(&plan.midi_tracks);
            self.rec.echo = plan.midi_tracks.clone();
        }
        self.engine.set_recording(true);
        if !self.engine.is_playing() {
            self.engine.play();
        }
        self.rec.dropped = 0;
        if let Some((dir, before)) = journal_dir {
            self.journal_pass_started(&dir, &before, &plan, transport_start);
        }
        let recording_n = plan.audio_tracks.len() + plan.midi_tracks.len();
        let takes = plan
            .audio_tracks
            .iter()
            .map(|&track| LiveTake {
                track,
                frames: 0,
                peaks: Vec::new(),
                pass_from: 0,
            })
            .collect();
        // Automation is written from here (ADR 0057).
        self.begin_automation_pass();
        self.rec.pass = Some(Live {
            plan,
            transport_start,
            audio: audio_on,
            takes,
            notes: Vec::new(),
            last_pos: transport_start,
            pass_start: pos,
            transport_seen: false,
        });
        self.set_status(if audio_on {
            tf("rec.recording_tracks", &[("n", &recording_n)])
        } else {
            t("status.recording")
        });
    }

    /// End the pass: the files come back from the engine and the pass becomes clips, in
    /// one undo step. An audio pass stops the transport (the writers close their files);
    /// a notes-only pass leaves it playing, as a punch-out on the fly.
    pub fn stop_recording(&mut self) {
        let Some(live) = self.rec.pass.take() else {
            return;
        };
        let at = self.engine.position();
        let tail = self.recorder.stop(at);
        self.engine.set_recording(false);
        if !self.rec.echo.is_empty() {
            self.engine.set_live_echo(&[]);
            self.rec.echo.clear();
        }
        if live.audio && self.engine.is_playing() {
            self.engine.stop();
        }
        let takes = if live.audio {
            self.engine.finish_recording()
        } else {
            Vec::new()
        };
        self.engine.set_record_start(None);
        if std::mem::take(&mut self.rec.count_in_click) {
            self.force_resync();
        }
        let mut notes = live.notes;
        notes.extend(tail);
        let mut plan = live.plan;
        plan.stop_at = at;
        plan.last_pass = self.recorder.pass();
        self.finish_pass(plan, takes, notes);
        // The parameters moved while it recorded, as automation clips (ADR 0057).
        self.commit_recorded_automation(at);
        if live.audio {
            self.journal_pass_finished();
        }
        self.end_gesture();
    }

    pub fn toggle_record(&mut self) {
        if self.is_recording() {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    /// Put a finished pass in the project: one undo step. Public so that a pass can be
    /// finished with takes built by hand (tests, and the engine's own once it lands).
    pub fn finish_pass(
        &mut self,
        plan: PassPlan,
        takes: Vec<FinishedTake>,
        notes: Vec<RecordedNote>,
    ) -> bool {
        let latency = self.engine.record_latency().total();
        self.finish_pass_at(plan, takes, notes, latency)
    }

    /// A pass recovered after a crash (`recovery_session`): finished with the latency it
    /// was recorded with.
    pub(crate) fn finish_pass_with_latency(
        &mut self,
        plan: PassPlan,
        takes: Vec<FinishedTake>,
        latency: i64,
    ) -> bool {
        self.finish_pass_at(plan, takes, Vec::new(), latency)
    }

    fn finish_pass_at(
        &mut self,
        plan: PassPlan,
        takes: Vec<FinishedTake>,
        notes: Vec<RecordedNote>,
        latency: i64,
    ) -> bool {
        // The time lost, not the sum over tracks (they lose the same moments).
        let dropped: u64 = takes.iter().map(|t| t.dropped).max().unwrap_or(0);
        let mut next = self.bus.project().next_id;
        let built = {
            let mut ids = || {
                let id = next;
                next += 1;
                id
            };
            record_pass::build(self.bus.project(), &plan, &takes, &notes, latency, &mut ids)
        };
        self.bus.reserve_ids(next);
        let commands = match built {
            Ok(c) => c,
            Err(e) => {
                self.set_error(tf("rec.failed", &[("error", &e.to_string())]));
                return false;
            }
        };
        let ok = commands.is_empty()
            || self.exec(Command::Batch {
                label_key: "cmd.record_take".into(),
                commands,
            });
        self.rec.dropped = dropped;
        // Round 28 (ADR 0054): each take analysed now, beside the window, so the pitch
        // editor opens on its notes at once.
        for take in takes.iter().filter(|_| ok) {
            self.analyse_file_soon(&take.path, take.sample_rate);
        }
        if dropped > 0 {
            let ms = dropped as f64 * 1000.0 / self.engine.sample_rate.max(1.0);
            self.set_error(tf("rec.dropped", &[("ms", &format!("{ms:.0}"))]));
        } else if !takes.is_empty() {
            self.set_status(tf("rec.done", &[("n", &takes.len())]));
        } else {
            self.set_status(t("status.ready"));
        }
        ok
    }

    /// Feed live-note events (normally the engine's) into the pass.
    pub fn ingest_note_inputs(&mut self, inputs: Vec<NoteInput>) {
        self.ingest_notes(&inputs, false);
    }

    fn ingest_notes(&mut self, inputs: &[NoteInput], wrapped: bool) {
        if !self.recorder.is_recording() {
            return;
        }
        let lp = self
            .rec
            .pass
            .as_ref()
            .and_then(|l| l.plan.loop_region)
            .map(|(start, end)| cantus_core::LoopRegion { start, end });
        let done = self.recorder.ingest_pass(inputs, lp.as_ref(), wrapped);
        if let Some(l) = self.rec.pass.as_mut() {
            l.notes.extend(done);
        }
    }

    /// Once a frame: the playhead's wraps, the takes' new peaks, the notes; and the end of
    /// the pass when the transport stopped by itself.
    pub fn poll_recording(&mut self) {
        let inputs = self.engine.take_note_inputs();
        let pos = self.engine.position();
        let Some(live) = self.rec.pass.as_mut() else {
            return;
        };
        let wrapped = match live.plan.loop_region {
            Some((a, b)) => pos < live.last_pos && (live.last_pos - pos) > (b - a) / 2,
            None => false,
        };
        live.last_pos = pos;
        let playing = self.engine.is_playing();
        live.transport_seen |= playing;
        let seen = live.transport_seen;
        if wrapped {
            if let Some((a, _)) = live.plan.loop_region {
                live.pass_start = a;
            }
            for t in &mut live.takes {
                t.pass_from = t.peaks.len();
            }
        }
        if live.audio {
            for lp in self.engine.poll_recording() {
                if let Some(t) = live.takes.iter_mut().find(|t| t.track == lp.track) {
                    t.frames = lp.frames;
                    t.peaks.extend_from_slice(&lp.peaks);
                }
            }
        }
        let audio = live.audio;
        self.ingest_notes(&inputs, wrapped);
        if audio {
            // Frames lost while the pass runs: warned at once, not only at the end.
            let lost = self.engine.record_dropped();
            if lost > self.rec.dropped {
                self.rec.dropped = lost;
                let ms = lost as f64 * 1000.0 / self.engine.sample_rate.max(1.0);
                self.set_error(tf("rec.dropped_live", &[("ms", &format!("{ms:.0}"))]));
            }
        }
        if seen && !playing && self.engine.stats().blocks_processed > 0 {
            // The transport stopped (space, the end of the song): close the pass. Not
            // before the engine has said it runs: the frame after *Record* may come
            // before the callback has taken the *Play* (round 24).
            self.stop_recording();
        }
    }

    /// For the screenshots: a pass drawn as if it were running, with these peaks per armed
    /// audio track.
    pub fn demo_pass(&mut self, record_start: Ticks, peaks: Vec<(TrackId, Vec<(f32, f32)>)>) {
        let mut plan = PassPlan::new(record_start);
        let (audio, midi) = self.armed_tracks();
        plan.audio_tracks = audio;
        plan.midi_tracks = midi;
        let takes = peaks
            .into_iter()
            .map(|(track, peaks)| LiveTake {
                track,
                frames: (peaks.len() * PEAK_FRAMES) as u64,
                peaks,
                pass_from: 0,
            })
            .collect();
        self.rec.pass = Some(Live {
            plan,
            transport_start: record_start,
            audio: true,
            takes,
            notes: Vec::new(),
            last_pos: record_start,
            pass_start: record_start,
            transport_seen: false,
        });
    }

    // ---- comping -----------------------------------------------------------------------

    fn exec_built(
        &mut self,
        label: &str,
        f: impl FnOnce(&cantus_core::Project, crate::clips::Ids) -> Vec<Command>,
    ) {
        let mut next = self.bus.project().next_id;
        let commands = {
            let mut ids = || {
                let id = next;
                next += 1;
                id
            };
            f(self.bus.project(), &mut ids)
        };
        self.bus.reserve_ids(next);
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: label.into(),
                commands,
            });
        }
    }

    /// Swipe `from..to` of a take lane into the comp: one undo step.
    pub fn comp_swipe(&mut self, track: TrackId, lane: u8, from: Ticks, to: Ticks) {
        self.exec_built("cmd.comp_swipe", |p, ids| {
            crate::comping::swipe(p, track, lane, from, to, ids)
        });
    }

    pub fn promote_take_lane(&mut self, track: TrackId, lane: u8) {
        self.exec_built("cmd.comp_promote", |p, ids| {
            crate::comping::promote_lane(p, track, lane, ids)
        });
    }

    pub fn promote_take_clip(&mut self, track: TrackId, clip: cantus_core::ClipId) {
        self.exec_built("cmd.comp_promote", |p, ids| {
            crate::comping::promote_clip(p, track, clip, ids)
        });
    }

    pub fn delete_unused_takes(&mut self, track: TrackId) {
        self.exec_built("cmd.delete_unused_takes", |p, _| {
            crate::comping::delete_unused_takes(p, track)
        });
        self.drop_missing_clips();
    }

    pub fn explode_takes(&mut self, track: TrackId) {
        let word = t("rec.take");
        self.exec_built("cmd.explode_takes", |p, ids| {
            crate::comping::explode_takes(p, track, &word, ids)
        });
    }

    // ---- seams and alternate comps (ADR 0058) --------------------------------------------

    /// Move the seam between two comp windows to `at`, both sides at once — and the seam at
    /// the same place on every track that comps with this one; a drag merges into one undo
    /// step.
    pub fn move_comp_seam(
        &mut self,
        track: TrackId,
        left: cantus_core::ClipId,
        right: cantus_core::ClipId,
        at: Ticks,
        drag: bool,
    ) -> bool {
        let commands = crate::comping::move_seam(self.project(), track, left, right, at);
        if commands.is_empty() {
            return false;
        }
        let cmd = Command::Batch {
            label_key: "cmd.set_comp_seam".into(),
            commands,
        };
        if drag {
            let key = cantus_core::MergeKey::of(&("comp-seam", track.0, left.0, right.0));
            self.exec_merging(cmd, key)
        } else {
            self.exec(cmd)
        }
    }

    /// Set the crossfade of the seam into `right` — or of every seam of the track (and of
    /// the tracks that comp with it).
    pub fn set_seam_xfade(&mut self, track: TrackId, right: Option<cantus_core::ClipId>, x: Ticks) {
        self.exec_built("cmd.comp_xfade", |p, _| {
            crate::comping::set_seam_xfade(p, track, right, x)
        });
    }

    /// The project's seam crossfade set to `ms`, and every seam of the track (and of the
    /// tracks that comp with it) given it: one undo step (review 1).
    pub fn set_comp_xfade_default(&mut self, track: TrackId, ms: f32) {
        self.exec_built("cmd.comp_xfade", |p, _| {
            let mut rs = p.recording.clone();
            rs.comp_xfade_ms = ms;
            let mut after = p.clone();
            after.recording.comp_xfade_ms = ms;
            let at = after
                .track(track)
                .and_then(|t| t.clips.iter().find(|c| c.comp).map(|c| c.start))
                .unwrap_or(Ticks::ZERO);
            let x = crate::comping::xfade_ticks(&after, at);
            let mut out = Vec::new();
            if p.recording != rs {
                out.push(Command::SetRecordSettings { settings: rs });
            }
            out.extend(crate::comping::set_seam_xfade(p, track, None, x));
            out
        });
    }

    /// Play comp `to` — on every track that comps with this one (each that has it).
    pub fn switch_comp(&mut self, track: TrackId, to: u8) {
        self.exec_built("cmd.comp_switch", |p, _| {
            crate::comping::comp_group(p, track)
                .into_iter()
                .flat_map(|g| crate::comping::switch_comp(p, g, to))
                .collect()
        });
        self.drop_missing_clips();
    }

    /// A new comp, a copy of the one playing — on every track that comps with this one.
    pub fn duplicate_comp(&mut self, track: TrackId) {
        let full = self
            .project()
            .track(track)
            .is_some_and(|t| t.comps.len() >= cantus_core::comp::MAX_COMPS);
        if full {
            self.set_error(t("comping.status.full"));
            return;
        }
        self.exec_built("cmd.comp_duplicate", |p, ids| {
            let mut out = Vec::new();
            for g in crate::comping::comp_group(p, track) {
                out.extend(crate::comping::duplicate_comp(p, g, &mut *ids));
            }
            out
        });
    }

    pub fn delete_comp(&mut self, track: TrackId, which: u8) {
        self.exec_built("cmd.comp_delete", |p, _| {
            crate::comping::delete_comp(p, track, which)
        });
    }

    pub fn rename_comp(&mut self, track: TrackId, which: u8, name: &str) {
        self.exec_built("cmd.comp_rename", |p, _| {
            crate::comping::rename_comp(p, track, which, name)
        });
    }

    pub fn flatten_comp(&mut self, track: TrackId) {
        self.exec_built("cmd.comp_flatten", |p, _| crate::comping::flatten(p, track));
        self.drop_missing_clips();
    }

    /// Quick swipe comping on or off (Logic's), one undo step.
    pub fn toggle_quick_swipe(&mut self) {
        let mut rs = self.project().recording.clone();
        rs.quick_swipe = !rs.quick_swipe;
        self.set_record_settings(rs);
    }

    /// Grouped comping on or off, one undo step.
    pub fn toggle_comp_grouped(&mut self) {
        let mut rs = self.project().recording.clone();
        rs.comp_grouped = !rs.comp_grouped;
        self.set_record_settings(rs);
    }

    /// Flatten and merge: the comp — its windows, seams and pitch edits as they play —
    /// bounced into one recording of its own (written beside the takes; the takes' files
    /// stay on disk), which becomes the track's one clip there; the takes go. One undo step.
    ///
    /// Started here, finished by [`Self::poll_merge`] (review 1): it waits for every source
    /// and pitch render still loading — the file holds what plays, never a window still on
    /// its way — then renders on a worker, so the window never freezes; the result is used
    /// only if the project has not moved meanwhile.
    pub fn merge_comp(&mut self, track: TrackId) -> bool {
        if self.merge_job.is_some() {
            return false;
        }
        let Some(tr) = self.project().track(track) else {
            return false;
        };
        if !crate::comping::comp_windows(tr)
            .iter()
            .any(|c| c.audio.is_some())
        {
            self.set_error(t("rec.merge_nothing"));
            return false;
        }
        self.merge_job = Some(MergeJob {
            track,
            revision: self.bus.revision(),
            running: None,
        });
        self.set_status(t("rec.merging"));
        self.poll_merge();
        true
    }

    /// Whether a merge is under way.
    pub fn merging(&self) -> bool {
        self.merge_job.is_some()
    }

    /// Carry a merge on (each frame): start its render once nothing it plays is loading,
    /// and make the clip when the worker is done. Returns whether one finished this call.
    pub fn poll_merge(&mut self) -> bool {
        let Some(job) = self.merge_job.as_ref() else {
            return false;
        };
        let (track, revision) = (job.track, job.revision);
        if self.bus.revision() != revision {
            self.merge_job = None;
            self.set_error(t("rec.merge_moved"));
            return false;
        }
        if job.running.is_none() {
            if self.engine.samples.loading() {
                return false;
            }
            return self.start_merge_render(track);
        }
        let got = match &job.running {
            Some(r) => match r.rx.try_recv() {
                Ok(got) => got,
                Err(std::sync::mpsc::TryRecvError::Empty) => return false,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Err("the render stopped".to_string())
                }
            },
            None => return false,
        };
        let Some(job) = self.merge_job.take() else {
            return false;
        };
        let Some(run) = job.running else {
            return false;
        };
        let (channels, frames) = match got {
            Ok(x) => x,
            Err(e) => {
                self.set_error(tf("rec.merge_failed", &[("error", &e)]));
                return false;
            }
        };
        let source = cantus_core::AudioSource {
            id: cantus_core::AudioSourceId(self.bus.alloc_id()),
            name: run.name.clone(),
            path: run.path.to_string_lossy().into_owned(),
            sample_rate: run.rate,
            channels,
            frames,
            sha256: None,
            recorded: true,
        };
        let clip = {
            let mut c = self
                .bus
                .new_clip(run.name.clone(), run.from, run.to - run.from);
            c.audio = Some(cantus_core::AudioRegion::of(source.id));
            c
        };
        let mut commands = vec![Command::InsertAudioSource { source }];
        commands.extend(
            run.windows
                .iter()
                .map(|c| Command::RemoveClip { track, clip: *c }),
        );
        commands.push(Command::InsertClip {
            track,
            clip: Box::new(clip),
        });
        // Then the takes go, as flatten has them go.
        let mut sim = crate::record_pass::Sim::new(self.project());
        if sim.push(commands.clone()).is_err() {
            self.set_error(t("rec.merge_nothing"));
            return false;
        }
        commands.extend(crate::comping::flatten(&sim.project, track));
        let ok = self.exec(Command::Batch {
            label_key: "cmd.comp_merge".into(),
            commands,
        });
        self.drop_missing_clips();
        if ok {
            self.set_status(t("rec.merged"));
        }
        ok
    }

    /// Nothing loading: the comp's windows alone, compiled here, rendered and written on a
    /// worker.
    fn start_merge_render(&mut self, track: TrackId) -> bool {
        let Some(tr) = self.project().track(track) else {
            self.merge_job = None;
            return false;
        };
        let windows: Vec<cantus_core::Clip> = crate::comping::comp_windows(tr)
            .into_iter()
            .filter(|c| c.audio.is_some())
            .cloned()
            .collect();
        let (Some(from), Some(to)) = (
            windows.iter().map(|c| c.start).min(),
            windows.iter().map(|c| c.end()).max(),
        ) else {
            self.merge_job = None;
            self.set_error(t("rec.merge_nothing"));
            return false;
        };
        let name = tf("comping.merged_name", &[("name", &tr.name)]);
        // The windows alone, straight to the master at unity: what the comp plays.
        let mut alone = cantus_core::Project::empty(self.project().name.clone());
        alone.tempo_bpm = self.project().tempo_bpm;
        alone.time_signature = self.project().time_signature;
        // The comp plays under the song's tempo map (ADR 0060).
        alone.tempo_points = self.project().tempo_points.clone();
        alone.metre_changes = self.project().metre_changes.clone();
        alone.audio_pool = self.project().audio_pool.clone();
        alone.loop_enabled = false;
        let mut lone = alone.make_track("comp");
        lone.strip = None;
        lone.clips = windows.clone();
        alone.tracks.push(lone);
        let rate = self.engine.sample_rate.round() as u32;
        let snapshot = self.engine.compile_for_render(&alone, 1);
        if self.engine.samples.loading() {
            // What it plays started loading just now: wait for it.
            return false;
        }
        let opts = cantus_engine::RenderOptions {
            sample_rate: rate,
            start: from,
            end: Some(to),
            tail_seconds: 0.0,
            ..Default::default()
        };
        let dir = self.audio_record_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(crate::takes::take_file_name(
            &name,
            &crate::takes::stamp_now(),
        ));
        let (tx, rx) = std::sync::mpsc::channel();
        let at = path.clone();
        let spawned = std::thread::Builder::new()
            .name("cantus-merge".into())
            .spawn(move || {
                let got = cantus_engine::render(snapshot, &opts)
                    .map_err(|e| e.to_string())
                    .and_then(|r| {
                        cantus_engine::write_wav(&r, &at)
                            .map(|_| (r.channels.min(255) as u8, r.frames() as u64))
                            .map_err(|e| e.to_string())
                    });
                let _ = tx.send(got);
            });
        if let Err(e) = spawned {
            self.merge_job = None;
            self.set_error(tf("rec.merge_failed", &[("error", &e.to_string())]));
            return false;
        }
        if let Some(job) = self.merge_job.as_mut() {
            job.running = Some(MergeRun {
                rx,
                path,
                name,
                rate,
                from,
                to,
                windows: windows.iter().map(|c| c.id).collect(),
            });
        }
        false
    }

    /// Drop from the selection a clip an operation took away.
    fn drop_missing_clips(&mut self) {
        let p = self.bus.project();
        let keep: Vec<_> = self
            .selected_clips
            .iter()
            .copied()
            .filter(|(t, c)| p.track(*t).and_then(|t| t.clip(*c)).is_some())
            .collect();
        self.selected_clips = keep;
        if let (Some(t), Some(c)) = (self.selected_track, self.selected_clip) {
            if p.track(t).and_then(|t| t.clip(c)).is_none() {
                self.selected_clip = None;
            }
        }
    }
}

/// A flatten-and-merge under way (review 1): waiting for what it plays to load, then
/// rendering on a worker.
#[derive(Debug)]
pub struct MergeJob {
    track: TrackId,
    /// The project's revision when it began: the result is used only on this one.
    revision: u64,
    running: Option<MergeRun>,
}

/// The worker's render and what the clip will be.
#[derive(Debug)]
struct MergeRun {
    rx: std::sync::mpsc::Receiver<Result<(u8, u64), String>>,
    path: std::path::PathBuf,
    name: String,
    rate: u32,
    from: Ticks,
    to: Ticks,
    windows: Vec<cantus_core::ClipId>,
}
