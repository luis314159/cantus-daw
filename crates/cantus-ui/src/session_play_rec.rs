//! Recording into a slot (ADR 0076 §8): Live's record button on a slot. The track records
//! through the application's own recording pass — the count-in, the input, the files, the
//! session journal and its recovery are the arrangement's — punched to the stretch the slot
//! records: from the boundary the press waits for (at once from stop, after the count-in)
//! to the bar line after it is stopped (the record button again, or launching the slot), or
//! after the session's fixed length. Boundaries are the performance's (its scenes' tempos
//! and metres).
//!
//! When the pass ends what it recorded becomes the slot's clip (MIDI quantised as the
//! session's record quantisation says; audio as its file's window), one undo step, playing
//! on looping. **MIDI plays on seamlessly**: a lead before the recording's end the clip is
//! laid in the projection with the notes played so far and launched exactly there — the
//! notes of its last moments are at its end, heard a whole pass later, by when the take is
//! whole. **Audio plays on seamlessly too**: a lead (and the time to load it) before the end,
//! what the take's file holds so far — its beginning, which is what plays at the end — is
//! copied aside and laid in the slot as the clip, launched exactly at the end; when the file
//! is whole the take replaces it under the same id, sample for sample the same where they
//! overlap. A take too short to have been written out yet plays on from when its file is
//! whole, as before. Recorded over a looping MIDI clip
//! that plays, the notes go into it where each was heard in its loop (overdub). A recording
//! cut short by a crash comes back into its slot (the journal keeps the slot).

use crate::app::CantusApp;
use crate::i18n::t;
use crate::record_pass::{audio_windows, source_of, PassPlan};
use crate::recording::RecordedNote;
use cantus_core::session::edit::new_note_clip;
use cantus_core::session::play::{Input, Timed};
use cantus_core::session::{boundary_at_or_after, SessionClip, SessionCommand, SlotRef};
use cantus_core::{AudioRegion, AudioSource, AudioSourceId, ClipId, Command, Ticks};
use cantus_engine::messages::{SessionEvent, SlotPhase};
use cantus_engine::FinishedTake;

/// A recording into a slot under way.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotRecording {
    pub slot: SlotRef,
    /// Where it records from (a boundary).
    pub from: Ticks,
    /// Where it stops (a bar line), once asked or by the fixed length.
    pub until: Option<Ticks>,
    /// Over the slot's looping MIDI clip that plays: its notes go into it.
    pub overdub: bool,
    /// The view was told it records.
    pub told: bool,
    /// The clip laid in the projection ahead of the end (its id is the take's).
    pub ahead: Option<SessionClip>,
    /// Audio: the source the clip laid ahead plays (the take's beginning, copied aside).
    pub ahead_source: Option<AudioSource>,
    /// Laying ahead was tried (audio: a file not written out yet is not tried again).
    pub tried: bool,
}

/// Unknown: the punch's end before the recording is stopped.
const OPEN: Ticks = Ticks(i64::MAX / 4);

/// How long before its end (beyond the lead) a MIDI take is laid ahead, seconds: a few
/// interface frames.
const AHEAD_S: f64 = 0.1;
/// Audio's: the copy of the take's beginning is loaded in the background meanwhile.
const AHEAD_AUDIO_S: f64 = 0.3;

impl CantusApp {
    /// Record into `slot` (the engine's `SessionMsg::Record`).
    pub(crate) fn record_slot(&mut self, slot: SlotRef) -> bool {
        if self.perform.recording.is_some() || self.is_recording() {
            return false;
        }
        let Some(track) = self.project().track(slot.track) else {
            return false;
        };
        if track.frozen.is_some() {
            self.set_status(t("session.frozen_track"));
            return false;
        }
        let armed = track.armed;
        let clip = self.project().slot_clip(slot).cloned();
        // A clip in the slot is overdubbed when it is a looping clip of notes that plays.
        let overdub = match &clip {
            None => false,
            Some(c) if c.clip.audio.is_none() && c.looping() && self.slot_playing(slot) => true,
            Some(_) => {
                self.set_status(t("session.slot_full"));
                return false;
            }
        };
        if !armed {
            self.toggle_arm(slot.track, false);
        }
        let playing = self.engine.is_playing();
        let (at, now) = self.press_at();
        let map = self.play_map();
        let from = if playing && !now {
            let s = &self.project().session.settings;
            boundary_at_or_after(&map, at, s.launch_quantize.grid(s.launch_quantize))
        } else {
            self.engine.position()
        };
        let fixed = self.project().session.settings.fixed_bars;
        let until = fixed.map(|b| map.add_bars(from, i64::from(b)));
        self.perform.recording = Some(SlotRecording {
            slot,
            from,
            until,
            overdub,
            told: false,
            ahead: None,
            ahead_source: None,
            tried: false,
        });
        // An empty slot's track stops where the recording starts (its stop button).
        if !overdub {
            self.push_input(Timed {
                at: from,
                now: true,
                input: Input::Launch {
                    slot,
                    velocity: 1.0,
                },
                pressed: from,
            });
        }
        self.engine.push_session_event(SessionEvent::Slot {
            slot,
            phase: SlotPhase::RecordQueued { at: from },
        });
        if !playing {
            // From stop: the playhead is where it records from; the pass counts in.
            self.start_recording();
        }
        true
    }

    /// Stop recording into `slot` at the next bar line of the performance.
    pub(crate) fn stop_slot_recording(&mut self, slot: SlotRef) -> bool {
        let (at, _) = self.press_at();
        let map = self.play_map();
        let Some(r) = self.perform.recording.as_mut().filter(|r| r.slot == slot) else {
            return false;
        };
        if r.until.is_none() {
            let at = at.max(r.from + Ticks(1));
            let bar = if map.bar_start(at) == at {
                at
            } else {
                map.bar_end(at)
            };
            r.until = Some(bar.max(map.bar_end(r.from)));
        }
        false
    }

    /// The recording's frame: started when it reaches its boundary, laid ahead a lead
    /// before its end (MIDI), stopped once the playhead heard its end.
    pub(crate) fn record_frame(&mut self) {
        let Some(r) = self.perform.recording.clone() else {
            return;
        };
        let heard = self.engine.position();
        let now = self.engine.engine_position();
        if !r.told && self.engine.is_playing() && now + Ticks(1) >= r.from {
            if !self.is_recording() {
                self.start_recording();
            }
            if let Some(x) = self.perform.recording.as_mut() {
                x.told = true;
            }
            self.engine.push_session_event(SessionEvent::Slot {
                slot: r.slot,
                phase: SlotPhase::Recording { since: r.from },
            });
        }
        if let Some(until) = r.until {
            self.set_pass_punch(r.from, until);
            // A lead and an interface frame or two before the end (laid early is no harm:
            // the notes after it are at the clip's end, a pass later).
            let (lead_at, _) = self.press_at();
            let bpm = self.heard_bpm(lead_at);
            let margin = Ticks::from_seconds(AHEAD_S, bpm);
            if r.ahead.is_none()
                && !r.overdub
                && lead_at + margin >= until
                && self.records_notes(r.slot)
            {
                self.lay_ahead(&r, until);
            }
            let audio_margin = Ticks::from_seconds(AHEAD_AUDIO_S, bpm);
            if !r.tried
                && !r.overdub
                && lead_at + audio_margin >= until
                && !self.records_notes(r.slot)
                && self.is_recording()
            {
                if let Some(x) = self.perform.recording.as_mut() {
                    x.tried = true;
                }
                self.lay_ahead_audio(&r, until);
            }
            if heard >= until && self.is_recording() {
                self.stop_recording();
            }
        }
        if !self.engine.is_playing() && !self.is_recording() && r.told {
            // The transport stopped first: the pass ended where it stopped.
            self.perform.recording = None;
        }
    }

    /// Whether the slot's track records notes (no input: no audio).
    fn records_notes(&self, slot: SlotRef) -> bool {
        self.project()
            .track(slot.track)
            .is_some_and(|t| t.input.is_none())
    }

    /// The MIDI clip as recorded so far, laid in the projection and launched at `until`.
    fn lay_ahead(&mut self, r: &SlotRecording, until: Ticks) {
        let notes: Vec<RecordedNote> = self
            .rec
            .pass
            .as_ref()
            .map(|p| p.notes.clone())
            .unwrap_or_default();
        let id = ClipId(self.bus.alloc_id());
        let clip = self.notes_clip(r, until, &notes, id);
        if let Some(x) = self.perform.recording.as_mut() {
            x.ahead = Some(SessionClip::new(clip));
        }
        // In step with the end, however late this is planned.
        self.push_input(Timed {
            at: until,
            now: true,
            input: Input::Resume {
                slot: r.slot,
                since: until,
            },
            pressed: until,
        });
        self.perform_projection_changed();
        self.replan();
    }

    /// The audio take as written so far — its beginning, copied aside — laid in the slot
    /// and launched at `until`: the take, whole, replaces it under the same id.
    fn lay_ahead_audio(&mut self, r: &SlotRecording, until: Ticks) {
        let Some(take) = self
            .engine
            .growing_takes()
            .into_iter()
            .find(|k| k.track == r.slot.track && k.frames > 0)
        else {
            return;
        };
        let Some(plan) = self.rec.pass.as_ref().map(|p| p.plan.clone()) else {
            return;
        };
        let dir = std::env::temp_dir().join("cantus-session-heads");
        let id = ClipId(self.bus.alloc_id());
        let head = dir.join(format!("head-{}-{}.wav", std::process::id(), id.0));
        let Some(frames) = std::fs::create_dir_all(&dir)
            .ok()
            .and_then(|()| copy_head(&take.path, &head))
        else {
            return;
        };
        // Placed as the whole take will be: the same window, the same fine offset.
        let whole = FinishedTake {
            frames: take.frames.max(1) + u64::from(take.sample_rate) * 3600,
            ..take.clone()
        };
        let latency = self.engine.pass_latency().total();
        let sid = AudioSourceId(self.bus.alloc_id());
        let Some(mut clip) = self.take_clip(&whole, &plan, r.from, until, latency, sid) else {
            let _ = std::fs::remove_file(&head);
            return;
        };
        clip.id = id;
        let mut source = source_of(
            &FinishedTake {
                path: head.clone(),
                frames,
                ..take
            },
            sid,
        );
        source.recorded = false;
        self.perform.heads.push(head);
        if let Some(x) = self.perform.recording.as_mut() {
            x.ahead = Some(SessionClip::new(clip));
            x.ahead_source = Some(source);
        }
        self.push_input(Timed {
            at: until,
            now: true,
            input: Input::Resume {
                slot: r.slot,
                since: until,
            },
            pressed: until,
        });
        self.perform_projection_changed();
        self.replan();
    }

    /// The clip an audio take makes in a slot recording `[from, until)`: its last window
    /// in the punch, playing source `sid`.
    fn take_clip(
        &mut self,
        take: &FinishedTake,
        plan: &PassPlan,
        from: Ticks,
        until: Ticks,
        latency: i64,
        sid: AudioSourceId,
    ) -> Option<cantus_core::Clip> {
        let map = self.bus.project().tempo_map();
        let mut p = plan.clone();
        p.punch = Some((from, until));
        let w = audio_windows(take, &p, &map, latency).last().copied()?;
        let (start, length, offset) = w.ticks()?;
        let mut clip = self.bus.new_clip(t("rec.take"), Ticks::ZERO, length);
        let mut region = AudioRegion::of(sid);
        region.slip_frames = w.slip_frames_in(start, offset, &map, take.sample_rate);
        clip.audio = Some(region);
        clip.offset = offset;
        clip.content_length = Some(length);
        Some(clip)
    }

    /// The take's clip of notes: `[from, until)` of the notes played, quantised.
    fn notes_clip(
        &mut self,
        r: &SlotRecording,
        until: Ticks,
        notes: &[RecordedNote],
        id: ClipId,
    ) -> cantus_core::Clip {
        let len = (until - r.from).max(Ticks(1));
        let rq = self.project().session.settings.record_quantize;
        let mut clip = {
            let mut ids = || self.bus.alloc_id();
            new_note_clip(&t("rec.take"), len, &mut ids).clip
        };
        clip.id = id;
        for n in notes
            .iter()
            .filter(|n| n.start >= r.from && n.start < until)
        {
            let at = rq.snap(n.start - r.from).clamp(Ticks::ZERO, len - Ticks(1));
            let length = n.length.min(len - at).max(Ticks(1));
            let mut note = self.bus.new_note(at, length, n.pitch, n.velocity);
            note.release = n.release;
            // What an MPE controller played on it, cut where the take's end cuts it.
            note.expr = if length < n.length {
                cantus_core::model::note_expr::ops::split(n.expr.as_deref(), length.0).0
            } else {
                n.expr.clone()
            };
            clip.notes.push(note);
        }
        clip
    }

    /// The punch of the pass recording into a slot: only its stretch.
    fn set_pass_punch(&mut self, from: Ticks, until: Ticks) {
        if let Some(live) = self.rec.pass.as_mut() {
            live.plan.punch = Some((from, until));
        }
    }

    /// The pass's plan when it records into a slot: its track alone, punched from where
    /// the slot records, no loop takes, the slot kept (the journal's, for a recovery).
    pub(crate) fn slot_pass_plan(&self, plan: &mut PassPlan) {
        let Some(r) = &self.perform.recording else {
            return;
        };
        plan.audio_tracks.retain(|t| *t == r.slot.track);
        plan.midi_tracks.retain(|t| *t == r.slot.track);
        plan.quick = None;
        plan.punch = Some((r.from, r.until.unwrap_or(OPEN)));
        plan.loop_region = None;
        plan.loop_takes = false;
        plan.slot = Some(r.slot);
    }

    /// A finished pass that recorded into a slot: its clip (or the notes overdubbed), one
    /// undo step, the new clip playing on. `false`: the pass was not a slot's (the
    /// arrangement's own path builds it). A pass recovered after a crash knows its slot from
    /// the journal.
    pub(crate) fn finish_slot_pass(
        &mut self,
        plan: &PassPlan,
        takes: &[FinishedTake],
        notes: &[RecordedNote],
        latency: i64,
    ) -> bool {
        let r = match self.perform.recording.take() {
            Some(r) => r,
            None => match plan.slot {
                Some(slot) => SlotRecording {
                    slot,
                    from: plan.punch.map_or(plan.record_start, |p| p.0),
                    until: plan.punch.map(|p| p.1).filter(|e| *e < OPEN),
                    overdub: false,
                    told: true,
                    ahead: None,
                    ahead_source: None,
                    tried: true,
                },
                None => return false,
            },
        };
        let until = r.until.unwrap_or(plan.stop_at).max(r.from + Ticks(1));
        let song = self.bus.project().clone();
        let rq = song.session.settings.record_quantize;
        let mut commands = Vec::new();
        let mut made = false;
        if r.overdub {
            // Into the clip where each note was heard in its loop: the window that played
            // it, of all the times the clip played (review finding 3).
            let Some(c) = song.slot_clip(r.slot) else {
                return true;
            };
            let played: Vec<_> = self
                .perform
                .plan()
                .perf
                .played
                .iter()
                .filter(|p| p.slot == r.slot)
                .copied()
                .collect();
            let mut new = Vec::new();
            for n in notes
                .iter()
                .filter(|n| n.start >= r.from && n.start < until)
            {
                let Some(p) = played.iter().find(|p| p.at <= n.start && n.start < p.until) else {
                    continue;
                };
                let Some(at) = c.content_from(p.from, n.start - p.at) else {
                    continue;
                };
                let at = c.clip.offset + rq.snap(at - c.clip.offset);
                let mut note = self
                    .bus
                    .new_note(at, n.length.max(Ticks(1)), n.pitch, n.velocity);
                note.release = n.release;
                note.expr = n.expr.clone();
                new.push(note);
            }
            if !new.is_empty() {
                commands.push(Command::InsertNotes {
                    track: r.slot.track,
                    clip: c.id(),
                    notes: new,
                });
            }
        } else if let Some(take) = takes.iter().find(|k| k.track == r.slot.track) {
            let sid = AudioSourceId(self.bus.alloc_id());
            if let Some(mut clip) = self.take_clip(take, plan, r.from, until, latency, sid) {
                commands.push(Command::InsertAudioSource {
                    source: source_of(take, sid),
                });
                // Laid ahead: the take replaces what played under its id (the projection
                // plays on, the same samples from the whole file).
                match &r.ahead {
                    Some(a) => clip.id = a.id(),
                    None => made = true,
                }
                commands.push(slot_insert(r.slot, clip));
            }
        } else {
            // The clip laid ahead keeps its id: the projection plays on, the end's notes
            // added.
            let id = r
                .ahead
                .as_ref()
                .map(|a| a.id())
                .unwrap_or_else(|| ClipId(self.bus.alloc_id()));
            let clip = self.notes_clip(&r, until, notes, id);
            commands.push(slot_insert(r.slot, clip));
            made = r.ahead.is_none();
        }
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.session.record".into(),
                commands,
            });
        }
        self.perform_projection_changed();
        if made && self.project().slot_clip(r.slot).is_some() && self.engine.is_playing() {
            // Not laid ahead (audio, or a pass that ended early): playing on from now, in
            // time with where it would be had it started at the recording's end.
            let (at, _) = self.press_at();
            self.push_input(Timed {
                at: at.max(until),
                now: true,
                input: Input::Resume {
                    slot: r.slot,
                    since: until,
                },
                pressed: until,
            });
            self.replan();
        } else if self.perform.active() {
            self.replan();
        }
        true
    }
}

fn slot_insert(slot: SlotRef, clip: cantus_core::Clip) -> Command {
    Command::Session(SessionCommand::InsertSlotClip {
        slot,
        clip: Box::new(SessionClip::new(clip)),
    })
}

/// Copy what the WAV `from` (being written) holds so far to `to`: its frames, or `None`.
fn copy_head(from: &std::path::Path, to: &std::path::Path) -> Option<u64> {
    let mut r = hound::WavReader::open(from).ok()?;
    let spec = r.spec();
    let channels = u64::from(spec.channels.max(1));
    let frames = u64::from(r.duration());
    if frames == 0 || spec.sample_format != hound::SampleFormat::Float {
        return None;
    }
    let mut w = hound::WavWriter::create(to, spec).ok()?;
    for x in r.samples::<f32>().take((frames * channels) as usize) {
        w.write_sample(x.ok()?).ok()?;
    }
    w.finalize().ok()?;
    Some(frames)
}
