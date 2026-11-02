//! The dancer's unit (round 35, ADR 0087): an instrument that makes **no sound**.
//!
//! It runs like any native instrument — a snapshot configures it with its numbers after
//! automation and modulators (ADR 0057), notes and controllers reach it between segments —
//! and writes nothing: `process` returns `false` and never touches a sample, so a track
//! with a dancer renders bit-identical to the track without it (the null tests,
//! `tests/dancer.rs`). What it does is **tell the screen**: at the end of every segment it
//! publishes a [`DanceFrame`] into its [`DanceFeed`] — where the song is, the tempo and the
//! bar, its numbers, the last note it was given and the keys held. Atomics only on the
//! audio thread; offline the feed also keeps the timeline a video export samples.
//!
//! **Its own matrix.** Like the organ, its modulation sources are global (the macros and
//! the channel's controllers) and move only at events, so the shared matrix is evaluated
//! then: the published values are the stored ones plus each route's depth × the target's
//! range, clamped.
//!
//! **Notes.** A note in the move map is stamped with the song position of the segment it
//! starts (the processor splits segments at events, so the next `set_position` is exactly
//! the note's sample), and with where it was let go; the one before it is kept too (what
//! the pose blends from). Every key is held until its note-off. A stop lets every key go
//! where the song is (the move still finishes its cycle on screen). **When the song jumps**
//! — a seek, a loop wrap, a start somewhere else: the position a segment starts at is not
//! where the last one ended — the notes and the last change of `move` are forgotten (they
//! belong to a time that is no longer behind it); a note given on the very sample the song
//! lands on is kept. A reset forgets them too.

use super::{Controller, InstrumentUnit, NoteOn};
use crate::dance::{DanceFeed, DanceFrame, DanceNote, VALUES};
use cantus_core::inst::dancer::{Move, PARAMS};
use cantus_core::inst::modulation::{MACRO_KEYS, SLOTS};
use cantus_core::NativeSpec;
use cantus_dsp::modmatrix::{ModCurve, ModMatrix, ModRoute};
use std::sync::Arc;

/// The sources a dancer produces, in its own index order (`dancer::SOURCES`, the same
/// list: `the_sources_are_the_models`).
pub const SOURCE_TAGS: [&str; 12] = [
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "mw", "expr",
    "at", "sus",
];
const SRC_MW: usize = 8;
const SRC_EXPR: usize = 9;
const SRC_AT: usize = 10;
const SRC_SUS: usize = 11;
/// `move`'s index in the values.
const MOVE: usize = 0;
/// How far the position a segment starts at may be from where the last one ended before
/// it is a jump (a seek, a loop wrap): far more than rounding, far less than any segment.
const JUMP_SECONDS: f64 = 2e-3;

/// The song position a segment starts at: what the frame is counted from.
#[derive(Copy, Clone, Debug, PartialEq)]
struct At {
    playing: bool,
    beats: f64,
    seconds: f64,
    tempo_bpm: f64,
    bar: i32,
    bar_start_beats: f64,
    bar_beats: f64,
}

impl Default for At {
    fn default() -> Self {
        At {
            playing: false,
            beats: 0.0,
            seconds: 0.0,
            tempo_bpm: 120.0,
            bar: 0,
            bar_start_beats: 0.0,
            bar_beats: 4.0,
        }
    }
}

impl At {
    fn of(p: &crate::fx::SongPosition) -> At {
        let num = f64::from(p.time_sig_num.max(1));
        let den = f64::from(p.time_sig_den.max(1));
        At {
            playing: p.playing,
            beats: p.beats,
            seconds: p.seconds,
            tempo_bpm: if p.tempo_bpm > 0.0 {
                p.tempo_bpm
            } else {
                120.0
            },
            bar: p.bar_number,
            bar_start_beats: p.bar_start_beats,
            bar_beats: num * 4.0 / den,
        }
    }

    /// Where the song is `frames` samples later (within one segment: no metre change, a
    /// tempo ramp's step held; the next segment's position corrects any rounding).
    fn after(self, frames: usize, sample_rate: f32) -> At {
        if !self.playing || frames == 0 {
            return self;
        }
        let dt = frames as f64 / f64::from(sample_rate);
        let mut a = self;
        a.seconds += dt;
        a.beats += dt * self.tempo_bpm / 60.0;
        while a.bar_beats > 0.0 && a.beats >= a.bar_start_beats + a.bar_beats {
            a.bar += 1;
            a.bar_start_beats += a.bar_beats;
        }
        a
    }
}

/// Every number by index (`dancer::PARAMS` order), without modulation.
pub fn values_of(spec: &NativeSpec) -> [f32; VALUES] {
    let mut v = [0.0; VALUES];
    for (i, info) in PARAMS.iter().enumerate() {
        v[i] = spec.get(info.key);
    }
    v
}

fn source_index(tag: &str) -> Option<u16> {
    SOURCE_TAGS.iter().position(|t| *t == tag).map(|i| i as u16)
}

/// The spec's live connections, resolved to the dancer's indices (a view key is never a
/// destination: it is set, never modulated).
pub fn routes(spec: &NativeSpec, matrix: &mut ModMatrix) {
    matrix.clear();
    for k in SLOTS.iter() {
        let (Some(src), Some(dst)) = (spec.text(k.src), spec.text(k.dst)) else {
            continue;
        };
        if spec.get(k.on) < 0.5 || !spec.kind.modulatable(dst) {
            continue;
        }
        let (Some(src), Some(dst)) = (source_index(src), PARAMS.iter().position(|i| i.key == dst))
        else {
            continue;
        };
        let via = match spec.text(k.via) {
            Some(v) => match source_index(v) {
                Some(i) => Some(i),
                None => continue,
            },
            None => None,
        };
        matrix.push(ModRoute {
            src,
            dst: dst as u16,
            depth: spec.get(k.amt),
            via,
            curve: ModCurve::from_index(spec.get(k.curve)),
        });
    }
}

/// The values after the matrix: each route's offset × the target's range, clamped.
pub fn modulated(base: &[f32; VALUES], matrix: &ModMatrix, sources: &[f32]) -> [f32; VALUES] {
    let mut eff = *base;
    if matrix.is_empty() {
        return eff;
    }
    let mut off = [0.0f32; VALUES];
    matrix.apply(sources, &mut off);
    for (i, info) in PARAMS.iter().enumerate() {
        if off[i] != 0.0 {
            eff[i] = (base[i] + off[i] * (info.max - info.min)).clamp(info.min, info.max);
        }
    }
    eff
}

#[derive(Clone, Debug)]
pub struct Unit {
    sample_rate: f32,
    feed: Arc<DanceFeed>,
    base: [f32; VALUES],
    matrix: ModMatrix,
    sources: [f32; SOURCE_TAGS.len()],
    /// The numbers after the matrix: what a frame carries.
    values: [f32; VALUES],
    /// Where the current segment started, and where the last one ended.
    at: At,
    end: At,
    note: Option<DanceNote>,
    prev_note: Option<DanceNote>,
    /// A note given since the last position: stamped with the next one (its own sample).
    pending: bool,
    /// The same for a release, and for the release of the note a new one took over from.
    pending_release: bool,
    takeover: bool,
    /// Where `move` last changed and from what; a change since the last position is
    /// stamped with the next one (the control grid's point it landed on).
    move_change: Option<(f64, f32)>,
    pending_change: bool,
    /// Configured once: the first values are where it starts, not a change.
    configured: bool,
    /// Told a position at least once (what a jump is measured from).
    positioned: bool,
    notes: u32,
    held: [u64; 2],
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let mut base = [0.0; VALUES];
        for (i, info) in PARAMS.iter().enumerate() {
            base[i] = info.default;
        }
        Unit {
            sample_rate: sample_rate.max(1.0),
            feed: Arc::new(DanceFeed::default()),
            base,
            matrix: ModMatrix::default(),
            sources: [0.0; SOURCE_TAGS.len()],
            values: base,
            at: At::default(),
            end: At::default(),
            note: None,
            prev_note: None,
            pending: false,
            pending_release: false,
            takeover: false,
            move_change: None,
            pending_change: false,
            configured: false,
            positioned: false,
            notes: 0,
            held: [0; 2],
        }
    }

    fn update(&mut self) {
        let was = self.values[MOVE];
        self.values = modulated(&self.base, &self.matrix, &self.sources);
        let now = self.values[MOVE];
        if now.round() != was.round() && self.configured {
            // Twice before a position (a knob and its automation): from what showed.
            let from = match self.move_change {
                Some((_, f)) if self.pending_change => f,
                _ => was,
            };
            self.move_change = Some((self.at.beats, from));
            self.pending_change = true;
        }
    }

    fn stamp(&mut self, at: At) {
        if self.pending {
            if let Some(n) = self.note.as_mut() {
                n.beats = at.beats;
                n.seconds = at.seconds;
            }
            if self.takeover {
                if let Some(r) = self.prev_note.as_mut().and_then(|n| n.released.as_mut()) {
                    *r = at.beats;
                }
            }
            self.pending = false;
            self.takeover = false;
        }
        if self.pending_change {
            if let Some((b, _)) = self.move_change.as_mut() {
                *b = at.beats;
            }
            self.pending_change = false;
        }
        if self.pending_release {
            if let Some(r) = self.note.as_mut().and_then(|n| n.released.as_mut()) {
                *r = at.beats;
            }
            self.pending_release = false;
        }
    }

    /// The latest note is let go (now, or at the next position when `exact`). Whether it
    /// was held until now.
    fn release(&mut self, exact: bool) -> bool {
        let now = self.at.beats;
        match self.note.as_mut().filter(|n| n.released.is_none()) {
            Some(n) => {
                n.released = Some(now);
                self.pending_release = exact;
                true
            }
            None => false,
        }
    }

    /// Publish where the song is after `frames` more samples of this segment.
    fn advance(&mut self, frames: usize) {
        let at = self.at;
        self.stamp(at);
        self.end = at.after(frames, self.sample_rate);
        self.at = self.end;
        self.feed.publish(&self.frame());
    }

    /// The frame as it stands at the end of the last segment.
    pub fn frame(&self) -> DanceFrame {
        let e = self.end;
        DanceFrame {
            beats: e.beats,
            seconds: e.seconds,
            tempo_bpm: e.tempo_bpm as f32,
            playing: e.playing,
            bar: e.bar,
            bar_start_beats: e.bar_start_beats,
            bar_beats: e.bar_beats as f32,
            values: self.values,
            note: self.note,
            prev_note: self.prev_note,
            move_change: self.move_change,
            held: self.held,
        }
    }

    /// The song jumped: the notes and the last change of `move` belong to a time no longer
    /// behind it. What was given on the sample it landed on (pending) stays.
    fn forget_the_past(&mut self) {
        if self.pending {
            self.prev_note = None;
            self.takeover = false;
        } else {
            self.note = None;
            self.prev_note = None;
            self.pending_release = false;
        }
        if !self.pending_change {
            self.move_change = None;
        }
    }

    fn hold(&mut self, key: u8, down: bool) {
        let (w, bit) = (usize::from(key) / 64, 1u64 << (key % 64));
        if down {
            self.held[w] |= bit;
        } else {
            self.held[w] &= !bit;
        }
    }
}

impl InstrumentUnit for Unit {
    /// A copy with all its state, publishing into a feed of its own: a range render's
    /// checkpoint is a render of its own, never the screen's.
    fn fork(&self) -> Option<Box<dyn InstrumentUnit>> {
        let mut u = self.clone();
        let feed = DanceFeed::default();
        feed.keep_timeline(self.feed.keeps_timeline());
        u.feed = Arc::new(feed);
        Some(Box::new(u))
    }

    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.base = values_of(spec);
        routes(spec, &mut self.matrix);
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.sources[m] = spec.get(key);
        }
        self.update();
        self.configured = true;
    }

    fn note_on(&mut self, note: NoteOn) {
        self.hold(note.key, true);
        if Move::from_key(note.key).is_none() {
            return;
        }
        self.notes = self.notes.wrapping_add(1);
        let was_pending = self.pending;
        // The note it takes over from: let go where this one starts if it was still held —
        // unless that note is one given on this same sample, which never showed (the one
        // before it stays as it was).
        let took = !was_pending && self.release(false);
        if was_pending {
            self.note = self.prev_note;
        }
        self.prev_note = self.note;
        self.note = Some(DanceNote {
            key: note.key,
            velocity: note.velocity.clamp(0.0, 1.0),
            beats: self.at.beats,
            seconds: self.at.seconds,
            count: self.notes,
            released: None,
        });
        self.pending = true;
        self.pending_release = false;
        if !was_pending {
            self.takeover = took;
        }
    }

    fn note_off(&mut self, key: u8, _id: u64, _velocity: f32) {
        self.hold(key, false);
        if self.note.is_some_and(|n| n.key == key) {
            self.release(true);
        }
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        let src = match controller {
            Controller::ModWheel => SRC_MW,
            Controller::Expression => SRC_EXPR,
            Controller::Pressure => SRC_AT,
            Controller::Sustain => SRC_SUS,
            _ => return,
        };
        let value = value.clamp(0.0, 1.0);
        if self.sources[src] != value {
            self.sources[src] = value;
            self.update();
        }
    }

    fn all_notes_off(&mut self) {
        self.held = [0; 2];
        // Let go where the song is: before a loop wrap's new position arrives.
        let _ = self.release(false);
    }

    fn reset(&mut self) {
        self.positioned = false;
        self.held = [0; 2];
        self.note = None;
        self.prev_note = None;
        self.pending = false;
        self.pending_release = false;
        self.move_change = None;
        self.pending_change = false;
    }

    fn set_position(&mut self, position: &crate::fx::SongPosition) {
        let at = At::of(position);
        // The song jumped (a seek, a loop wrap): what came before is not behind it now.
        if self.positioned && (at.seconds - self.end.seconds).abs() > JUMP_SECONDS {
            self.forget_the_past();
        }
        self.positioned = true;
        self.at = at;
        self.end = at;
        self.stamp(at);
    }

    fn process(&mut self, left: &mut [f32], _right: &mut [f32]) -> bool {
        self.advance(left.len());
        false
    }

    /// Always: it writes no sample, so skipping it changes none (ADR 0031 §5.1). Skipped,
    /// it still publishes ([`Self::skip`]).
    fn at_rest(&self) -> bool {
        true
    }

    fn skip(&mut self, frames: usize) {
        self.advance(frames);
    }

    fn active_voices(&self) -> usize {
        0
    }

    fn dance_feed(&self) -> Option<Arc<DanceFeed>> {
        Some(self.feed.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::inst::dancer::{self, Move};
    use cantus_core::InstrumentKind;

    #[test]
    fn the_sources_are_the_models() {
        assert_eq!(&SOURCE_TAGS[..], dancer::SOURCES);
        assert_eq!(VALUES, PARAMS.len());
    }

    #[test]
    fn a_macro_routed_to_energy_moves_what_it_publishes() {
        let mut spec = NativeSpec::new(InstrumentKind::Dancer);
        spec.set("macro2", 0.5).unwrap();
        spec.set_text("mod1.src", Some("macro2".into())).unwrap();
        spec.set_text("mod1.dst", Some("energy".into())).unwrap();
        spec.set("mod1.amt", -0.4).unwrap();
        // A view key is never a destination.
        spec.set_text("mod2.src", Some("mw".into())).unwrap();
        spec.set_text("mod2.dst", Some("view.mode".into())).unwrap();
        spec.set("mod2.amt", 1.0).unwrap();
        let mut u = Unit::new(48_000.0);
        u.configure(&spec, &[]);
        u.controller(Controller::ModWheel, 1.0);
        let mut l = [0.0f32; 64];
        let mut r = [0.0f32; 64];
        assert!(!u.process(&mut l, &mut r));
        let f = u.dance_feed().unwrap().latest().unwrap();
        assert!((f.value("energy").unwrap() - (0.7 - 0.2)).abs() < 1e-6);
        assert_eq!(f.value("view.mode"), Some(0.0));
        assert!(l.iter().chain(&r).all(|x| x.to_bits() == 0));
    }

    #[test]
    fn a_note_is_stamped_with_the_segment_it_starts_and_held_until_let_go() {
        let mut u = Unit::new(48_000.0);
        u.configure(&NativeSpec::new(InstrumentKind::Dancer), &[]);
        let pos = |beats: f64| crate::fx::SongPosition {
            playing: true,
            beats,
            seconds: beats * 0.5,
            bar_start_beats: (beats / 4.0).floor() * 4.0,
            bar_number: (beats / 4.0).floor() as i32,
            ..crate::fx::SongPosition::default()
        };
        let mut l = [0.0f32; 480];
        let mut r = [0.0f32; 480];
        u.set_position(&pos(3.96));
        u.process(&mut l, &mut r);
        let key = dancer::BASE_KEY + Move::Clap.index() as u8;
        u.note_on(NoteOn {
            key,
            velocity: 0.5,
            id: 7,
            channel: 0,
        });
        u.set_position(&pos(3.98));
        u.process(&mut l, &mut r);
        let f = u.dance_feed().unwrap().latest().unwrap();
        let n = f.note.unwrap();
        assert_eq!((n.key, n.beats, n.count), (key, 3.98, 1));
        assert!(f.is_held(key));
        // 480 samples at 120 BPM, 48 kHz: 0.02 beats on, past bar 1's end.
        assert!((f.beats - 4.0).abs() < 1e-9 && f.bar == 1 && f.bar_start_beats == 4.0);
        u.note_off(key, 7, 0.0);
        u.set_position(&pos(4.0));
        u.process(&mut l, &mut r);
        let f = u.dance_feed().unwrap().latest().unwrap();
        assert!(!f.is_held(key), "let go");
        assert_eq!(f.note.unwrap().released, Some(4.0), "at its own sample");
        // A key outside the map is held, never the note; the next map note keeps the last.
        u.note_on(NoteOn {
            key: 100,
            velocity: 1.0,
            id: 8,
            channel: 0,
        });
        let wave = dancer::BASE_KEY + Move::Wave.index() as u8;
        u.note_on(NoteOn {
            key: wave,
            velocity: 1.0,
            id: 9,
            channel: 0,
        });
        u.set_position(&pos(4.02));
        u.process(&mut l, &mut r);
        let f = u.dance_feed().unwrap().latest().unwrap();
        assert!(f.is_held(100) && f.is_held(wave));
        assert_eq!(f.note.unwrap().key, wave);
        assert_eq!(f.note.unwrap().beats, 4.02);
        assert_eq!(f.prev_note.unwrap().key, key);
        assert_eq!(f.prev_note.unwrap().released, Some(4.0));
        // A stop lets it go where the song is.
        u.all_notes_off();
        let f = u.frame();
        assert!(f.held == [0; 2] && f.note.unwrap().released.is_some());
        u.reset();
        u.process(&mut l, &mut r);
        assert!(u.dance_feed().unwrap().latest().unwrap().note.is_none());
    }

    #[test]
    fn a_change_of_move_is_stamped_where_it_landed() {
        let mut u = Unit::new(48_000.0);
        let mut spec = NativeSpec::new(InstrumentKind::Dancer);
        spec.set("move", dancer::move_value(Move::Wave)).unwrap();
        u.configure(&spec, &[]);
        let mut l = [0.0f32; 64];
        let mut r = [0.0f32; 64];
        u.process(&mut l, &mut r);
        assert_eq!(u.frame().move_change, None, "where it starts is no change");
        spec.set("move", dancer::move_value(Move::Clap)).unwrap();
        u.configure(&spec, &[]);
        u.set_position(&crate::fx::SongPosition {
            playing: true,
            beats: 8.0,
            ..Default::default()
        });
        u.process(&mut l, &mut r);
        assert_eq!(
            u.frame().move_change,
            Some((8.0, dancer::move_value(Move::Wave)))
        );
        // The same move configured again (another key moved): no change.
        spec.set("hue", 20.0).unwrap();
        u.configure(&spec, &[]);
        u.process(&mut l, &mut r);
        assert_eq!(
            u.frame().move_change,
            Some((8.0, dancer::move_value(Move::Wave)))
        );
        u.reset();
        assert_eq!(u.frame().move_change, None);
    }

    fn playing_at(beats: f64) -> crate::fx::SongPosition {
        crate::fx::SongPosition {
            playing: true,
            beats,
            seconds: beats * 0.5,
            tempo_bpm: 120.0,
            bar_start_beats: (beats / 4.0).floor() * 4.0,
            bar_number: (beats / 4.0).floor() as i32,
            time_sig_num: 4,
            time_sig_den: 4,
            ..crate::fx::SongPosition::default()
        }
    }

    fn note(u: &mut Unit, m: Move) {
        u.note_on(NoteOn {
            key: dancer::BASE_KEY + m.index() as u8,
            velocity: 1.0,
            id: 1,
            channel: 0,
        });
    }

    /// 480 samples at 48 kHz and 120 BPM: 0.02 beats.
    fn segment(u: &mut Unit, beats: f64) {
        let mut l = [0.0f32; 480];
        let mut r = [0.0f32; 480];
        u.set_position(&playing_at(beats));
        u.process(&mut l, &mut r);
    }

    #[test]
    fn a_jump_of_the_song_forgets_the_notes_and_the_change_of_move() {
        let mut u = Unit::new(48_000.0);
        let mut spec = NativeSpec::new(InstrumentKind::Dancer);
        u.configure(&spec, &[]);
        segment(&mut u, 11.98);
        // Beat 12 of a 16-beat loop: a pad pressed, `move` changed.
        note(&mut u, Move::Clap);
        spec.set("move", dancer::move_value(Move::Wave)).unwrap();
        u.configure(&spec, &[]);
        segment(&mut u, 12.0);
        segment(&mut u, 12.02);
        let f = u.frame();
        assert!(f.note.is_some() && f.move_change.is_some());
        assert_eq!(f.move_change.unwrap().0, 12.0);
        // The loop wraps to 0: the pad and the change belong to a later time.
        segment(&mut u, 0.0);
        let f = u.frame();
        assert_eq!((f.note, f.prev_note, f.move_change), (None, None, None));
        // A note given on the very sample the song lands on is kept.
        note(&mut u, Move::Jump);
        segment(&mut u, 8.0);
        let n = u.frame().note.expect("the note given where it landed");
        assert_eq!(
            (n.key, n.beats),
            (dancer::BASE_KEY + Move::Jump.index() as u8, 8.0)
        );
    }

    #[test]
    fn two_notes_on_one_sample_leave_the_older_ones_release_alone() {
        let mut u = Unit::new(48_000.0);
        u.configure(&NativeSpec::new(InstrumentKind::Dancer), &[]);
        segment(&mut u, 0.98);
        note(&mut u, Move::Sway);
        segment(&mut u, 1.0);
        u.note_off(dancer::BASE_KEY + Move::Sway.index() as u8, 1, 0.0);
        segment(&mut u, 1.02);
        assert_eq!(u.frame().note.unwrap().released, Some(1.02));
        // Two map notes on one sample at beat 1.04: the older note keeps its release.
        note(&mut u, Move::Clap);
        note(&mut u, Move::Kick);
        segment(&mut u, 1.04);
        let f = u.frame();
        assert_eq!(
            f.note.unwrap().key,
            dancer::BASE_KEY + Move::Kick.index() as u8
        );
        assert_eq!(f.prev_note.unwrap().released, Some(1.02));
    }

    #[test]
    fn a_fork_publishes_into_a_feed_of_its_own() {
        let u = Unit::new(48_000.0);
        let mut f = u.fork().unwrap();
        let mut l = [0.0f32; 16];
        let mut r = [0.0f32; 16];
        f.process(&mut l, &mut r);
        assert_eq!(f.dance_feed().unwrap().published(), 1);
        assert_eq!(u.dance_feed().unwrap().published(), 0);
    }
}
