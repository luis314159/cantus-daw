//! **Live MPE** (round 33, ADR 0080): an MPE controller's member channels turned into
//! notes that each carry their own bend, pressure and timbre — to the live track's
//! instrument (and the tracks echoing it) by note id and channel, and, while recording, to
//! the interface as [`ExprInput`]s it binds to the notes it is making.
//!
//! **Where it applies.** Only to what reaches the engine from a port marked MPE: every
//! other port's channel messages arrive on channel 1 (`inst::mpe::for_engine`), the lower
//! zone's master or no zone at all, and take the ordinary path; with no port marked the
//! layout is off and nothing here runs (the null rule).
//!
//! **A member channel** (MPE 1.1 §3): its note-on starts a note of its own (a note already
//! on that channel is released first); its pitch bend — at the zone's member bend range —,
//! channel pressure (or a key pressure on it) and CC 74 are that note's, and arrive before
//! or after its note-on: values sent before are held and given at the note-on. **The master
//! channel** is the zone's: its messages go the ordinary way (bend, pedal, wheel). **RPNs**
//! on any channel are read as they go by: the MCM (RPN 6 on a master) sets the zone, RPN 0
//! the bend range (on a member: the members'; on the master: the master's).
//!
//! Runs on the audio thread: fixed arrays, no allocation (`rt_memory.rs`).

use crate::inst::mpe::{ExprInput, MpeIn};
use crate::inst::NoteExpression;
use crate::messages::{EngineEvent, MidiEvent};
use crate::processor::track::TrackSlot;
use crate::processor::Processor;
use crate::processor::{SlotInstrument, LIVE_TAG};
use cantus_core::model::note_expr::midi2;
use cantus_core::model::note_expr::zones::{MpeZones, Role};
use cantus_core::model::note_expr::ExprKind;
use cantus_core::{Ticks, TrackId};

/// The note ids live MPE notes carry: one per channel and key, below the plain live
/// keys' (`processor::native_id`), never a clip note's (those count up from 1).
pub(crate) const MPE_TAG_BASE: u64 = LIVE_TAG - 1024;

/// The free slots of the engine's event ring below which a recorded value is not sent: room
/// kept for the notes' ons and offs (review D8).
pub(crate) const VALUE_ROOM: usize = 64;

/// The id of the note on member channel `ch`, key `key`.
pub(crate) fn mpe_id(ch: u8, key: u8) -> u64 {
    MPE_TAG_BASE - u64::from(ch & 15) * 128 - u64::from(key & 127)
}

/// One channel's state.
#[derive(Copy, Clone, Debug)]
struct Chan {
    /// The sounding note: its key, and where it was heard (what recording names it by).
    key: Option<u8>,
    start: Ticks,
    /// The last values, 14 and 7 bits; whether pressure and timbre were ever sent.
    bend: u16,
    pressure: u8,
    timbre: u8,
    has_pressure: bool,
    has_timbre: bool,
    /// The RPN selected (CC 101, CC 100), 127 nothing.
    rpn: (u8, u8),
}

impl Default for Chan {
    fn default() -> Self {
        Chan {
            key: None,
            start: Ticks::ZERO,
            bend: 8192,
            pressure: 0,
            timbre: 64,
            has_pressure: false,
            has_timbre: false,
            rpn: (127, 127),
        }
    }
}

/// The processor's live MPE state.
#[derive(Clone, Debug, Default)]
pub(crate) struct MpeLive {
    /// `None`: no port is MPE, nothing here runs.
    zones: Option<MpeZones>,
    chans: [Chan; 16],
}

impl MpeLive {
    /// The layout of the MPE ports (`None`: none is MPE). Every channel starts afresh.
    pub(crate) fn set(&mut self, zones: Option<MpeZones>) {
        // Marked but empty, it waits for the controller's MCM.
        self.zones = zones.map(MpeZones::repaired);
        self.chans = [Chan::default(); 16];
    }
}

impl TrackSlot {
    /// A live MPE note on: to a native unit with its id and channel; Firmus and Pulsus play
    /// it as a plain live key.
    pub(crate) fn note_on_mpe(&mut self, key: u8, velocity: u8, id: u64, channel: u8) {
        match (self.instrument, self.native.as_mut()) {
            (SlotInstrument::Native, Some(n)) => n.unit.note_on(crate::inst::NoteOn {
                key,
                velocity: f32::from(velocity) / 127.0,
                id,
                channel,
            }),
            _ => self.note_on(key, velocity, LIVE_TAG),
        }
    }

    pub(crate) fn note_off_mpe(&mut self, key: u8, id: u64, release: u8) {
        match (self.instrument, self.native.as_mut()) {
            (SlotInstrument::Native, Some(n)) => {
                n.unit.note_off(key, id, f32::from(release) / 127.0)
            }
            _ => self.note_off(key, LIVE_TAG, release),
        }
    }

    pub(crate) fn expression_note(&mut self, id: u64, e: NoteExpression, value: f32) {
        if let (SlotInstrument::Native, Some(n)) = (self.instrument, self.native.as_mut()) {
            n.unit.expression(id, e, value);
        }
    }
}

/// The expression a member channel's message carries: the kind, the value the instrument
/// hears and the curve's stored value.
fn expression_of(kind: ExprKind, raw: i64) -> (NoteExpression, f32) {
    (
        crate::inst::expr_play::expression_of(kind),
        kind.read(raw) as f32,
    )
}

impl Processor {
    /// The layout of the MPE ports (`EngineMessage::SetMpe`): what is sounding on a member
    /// channel is let go first.
    pub(super) fn set_mpe(&mut self, zones: Option<MpeZones>) {
        self.mpe_release_all();
        self.mpe.set(zones);
    }

    /// Release every live MPE note (a new layout, a panic).
    fn mpe_release_all(&mut self) {
        let Some(track) = self.live_track else {
            self.mpe.chans.iter_mut().for_each(|c| c.key = None);
            return;
        };
        for ch in 0..16u8 {
            if let Some(key) = self.mpe.chans[ch as usize].key.take() {
                self.mpe_sound(track, ch, key, 0, 0);
            }
        }
    }

    /// An event from an MPE port: `true` when it was a member channel's (and is done
    /// with), `false` for the ordinary path. Reads the RPNs it sees on the way.
    pub(super) fn mpe_midi(&mut self, ev: MidiEvent) -> bool {
        let Some(zones) = self.mpe.zones else {
            return false;
        };
        let ch = ev.status & 15;
        let kind = ev.status & 0xF0;
        if kind == 0xB0 {
            self.mpe_rpn(ch, ev.data1, ev.data2);
        }
        // The MCM may have just changed the layout.
        let zones = self.mpe.zones.unwrap_or(zones);
        // Another port's panic (review D7): its own plain notes go, the MPE notes sound on.
        if kind == 0xB0
            && (ev.data1 == 120 || ev.data1 == 123)
            && ev.data2 == crate::inst::mpe::PLAIN_PANIC
        {
            if let Some(track) = self.live_track {
                for t in self.live_targets(track).into_iter().flatten() {
                    if let Some(mut s) = self.slot_for(t).and_then(|slot| self.nodes.track(slot)) {
                        for key in 0..128u8 {
                            s.note_off(key, LIVE_TAG, 0);
                        }
                    }
                }
            }
            return true;
        }
        // A panic (all sound off, all notes off; review fix 9): on a member channel its note
        // goes; anywhere else every MPE note goes, and the ordinary path does the rest.
        if kind == 0xB0 && (ev.data1 == 120 || ev.data1 == 123) {
            if let Role::Member { .. } = zones.role(ch) {
                if let (Some(track), Some(key)) =
                    (self.live_track, self.mpe.chans[ch as usize].key.take())
                {
                    self.mpe_sound(track, ch, key, 0, 0);
                }
                return true;
            }
            self.mpe_release_all();
            return false;
        }
        let Role::Member { upper } = zones.role(ch) else {
            return false;
        };
        let range = zones.zone(upper).map_or(48, |z| z.member_bend);
        let Some(track) = self.live_track else {
            return true;
        };
        match kind {
            0x90 if ev.data2 > 0 => {
                if let Some(old) = self.mpe.chans[ch as usize].key.take() {
                    self.mpe_sound(track, ch, old, 0, 0);
                }
                let start = self.heard_position();
                let c = &mut self.mpe.chans[ch as usize];
                c.key = Some(ev.data1 & 127);
                c.start = start;
                self.mpe_sound(track, ch, ev.data1 & 127, ev.data2, 0);
                // What was sent before the note is where it starts.
                let c = self.mpe.chans[ch as usize];
                self.mpe_value(track, ch, ExprKind::Bend, midi2::bend_of_14(c.bend, range));
                if c.has_pressure {
                    self.mpe_value(track, ch, ExprKind::Pressure, midi2::unit_of_7(c.pressure));
                }
                if c.has_timbre {
                    self.mpe_value(track, ch, ExprKind::Timbre, midi2::unit_of_7(c.timbre));
                }
            }
            0x80 | 0x90 => {
                let c = &mut self.mpe.chans[ch as usize];
                if c.key == Some(ev.data1 & 127) {
                    c.key = None;
                    let release = if kind == 0x80 { ev.data2 & 127 } else { 0 };
                    self.mpe_sound(track, ch, ev.data1 & 127, 0, release);
                }
            }
            0xE0 => {
                let raw = (u16::from(ev.data2 & 127) << 7) | u16::from(ev.data1 & 127);
                self.mpe.chans[ch as usize].bend = raw;
                self.mpe_value(track, ch, ExprKind::Bend, midi2::bend_of_14(raw, range));
            }
            0xD0 | 0xA0 => {
                let v = if kind == 0xD0 { ev.data1 } else { ev.data2 } & 127;
                let c = &mut self.mpe.chans[ch as usize];
                c.pressure = v;
                c.has_pressure = true;
                self.mpe_value(track, ch, ExprKind::Pressure, midi2::unit_of_7(v));
            }
            0xB0 if ev.data1 == 74 => {
                let c = &mut self.mpe.chans[ch as usize];
                c.timbre = ev.data2 & 127;
                c.has_timbre = true;
                self.mpe_value(
                    track,
                    ch,
                    ExprKind::Timbre,
                    midi2::unit_of_7(ev.data2 & 127),
                );
            }
            // Other controllers and programs on a member channel mean nothing per note.
            _ => {}
        }
        true
    }

    /// The RPN state machine of channel `ch` for one controller: the MCM and bend ranges.
    fn mpe_rpn(&mut self, ch: u8, cc: u8, value: u8) {
        let c = &mut self.mpe.chans[ch as usize];
        match cc {
            101 => c.rpn.0 = value & 127,
            100 => c.rpn.1 = value & 127,
            6 => {
                let rpn = c.rpn;
                // One value per selection: what the MCM or the bend range said is done with,
                // so a later data entry (another port's CC 6 on channel 1) is never read as a
                // new one (review fix 9).
                if matches!(rpn, (0, 6) | (0, 0)) {
                    c.rpn = (127, 127);
                }
                let Some(zones) = self.mpe.zones else { return };
                match rpn {
                    // The MCM, on a master channel.
                    (0, 6) if ch == 0 || ch == 15 => {
                        let z = zones.after_mcm(ch, value & 127);
                        // A zone turned off leaves the layout on but empty of it: the other
                        // may still play; with neither, member traffic is ordinary.
                        self.mpe_release_all();
                        self.mpe.zones = Some(z);
                    }
                    // Pitch bend sensitivity, semitones (cents in CC 38 are not read).
                    (0, 0) => {
                        let mut z = zones;
                        match z.role(ch) {
                            Role::Member { upper } => {
                                if let Some(zone) = if upper {
                                    z.upper.as_mut()
                                } else {
                                    z.lower.as_mut()
                                } {
                                    zone.member_bend = value.clamp(1, 96);
                                }
                            }
                            // The manager's range is the instrument's own (review fix 8).
                            Role::Master { .. } => {}
                            Role::None => {}
                        }
                        self.mpe.zones = Some(z);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Sound (or release, `velocity` 0) a member channel's note on the live track and its
    /// echoes; report it while recording.
    fn mpe_sound(&mut self, track: TrackId, ch: u8, key: u8, velocity: u8, release: u8) {
        let id = mpe_id(ch, key);
        for t in self.live_targets(track).into_iter().flatten() {
            let Some(slot) = self.slot_for(t) else {
                continue;
            };
            let through = self.nodes.track(slot).is_some_and(|s| s.fx_active());
            if through {
                // Note effects remake the notes: they take the key as a plain live key.
                self.sound_live(slot, key, velocity, release);
                continue;
            }
            self.auto_live_note(slot, velocity > 0);
            if let Some(mut s) = self.nodes.track(slot) {
                if velocity > 0 {
                    s.note_on_mpe(key, velocity, id, ch);
                } else {
                    s.note_off_mpe(key, id, release);
                }
            }
            if self.listened[slot] {
                let note = if velocity > 0 {
                    crate::fx::FxNote::On {
                        pitch: key,
                        velocity,
                    }
                } else {
                    crate::fx::FxNote::Off { pitch: key }
                };
                self.notes_to_fx(slot, self.note_at, note);
            }
        }
        // Recorded by the interface's MPE take, by channel (review fix 7), not as plain notes.
        if self.recording && self.transport.is_playing() {
            let tick = self.heard_position();
            let start = if velocity > 0 {
                tick
            } else {
                self.mpe.chans[ch as usize].start
            };
            let what = if velocity > 0 {
                MpeIn::On { velocity }
            } else {
                MpeIn::Off { release }
            };
            let _ = self.tx.push(EngineEvent::ExprInput(ExprInput {
                track,
                tick,
                channel: ch,
                pitch: key,
                start,
                what,
            }));
        }
    }

    /// A value for the note on member channel `ch` (none sounding: held for its note-on).
    fn mpe_value(&mut self, track: TrackId, ch: u8, kind: ExprKind, raw: i64) {
        let c = self.mpe.chans[ch as usize];
        let Some(key) = c.key else { return };
        let id = mpe_id(ch, key);
        let (e, v) = expression_of(kind, raw);
        for t in self.live_targets(track).into_iter().flatten() {
            if let Some(mut s) = self.slot_for(t).and_then(|slot| self.nodes.track(slot)) {
                s.expression_note(id, e, v);
            }
        }
        // A value gives way when the ring is nearly full, so a note's on and off always fit
        // (review D8): a curve loses a point, a take never loses a note.
        if self.recording && self.transport.is_playing() && self.tx.slots() > VALUE_ROOM {
            let _ = self.tx.push(EngineEvent::ExprInput(ExprInput {
                track,
                tick: self.heard_position(),
                channel: ch,
                pitch: key,
                start: c.start,
                what: MpeIn::Value { kind, value: raw },
            }));
        }
    }
}
