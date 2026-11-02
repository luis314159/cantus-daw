//! What the engine tells the screen about a dancer (round 35, ADR 0087).
//!
//! A dancer is a native instrument that makes no sound (`cantus_core::inst::dancer`). Its
//! unit runs on the audio thread like any other — automation, modulators and MIDI learn
//! reach its numbers, notes reach it between blocks — and at the end of every block it
//! **publishes** what it knows as a [`DanceFrame`] into its [`DanceFeed`]: where the song
//! is, the tempo, its numbers after automation and modulation, the last note it was given.
//! It never touches a sample.
//!
//! **Live**, the interface reads the latest frame each screen frame ([`DanceFeed::latest`]):
//! a sequence lock over atomics — the writer never waits, a reader retries a torn read.
//! **A timeline** is kept only when a video export asks for it ([`RenderFeeds::want_timelines`]
//! before the render, [`DanceFeed::keep_timeline`] on each feed): the unit then appends every
//! frame, which the export samples at its frame times after the render
//! ([`DanceFeed::take_timeline`]). Any other render — a WAV, stems, a freeze, a listen —
//! keeps nothing (a dancer reacting to a track publishes every 16 samples: hundreds of MB
//! over a song). Keeping happens on a render's worker thread, never the audio
//! thread, so that append may allocate.
//!
//! The feed is made off the audio thread with the unit and handed to the interface before
//! the unit goes to the engine (`InstrumentUnit::dance_feed`).

use cantus_core::TrackId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The dancer's numbers a frame carries, in `cantus_core::inst::dancer::KEYS` order.
pub const VALUES: usize = cantus_core::inst::dancer::KEYS.len();

/// A note in a dancer's move map (`cantus_core::inst::dancer::BASE_KEY…`): what chooses
/// its move. Keys outside the map are held (`DanceFrame::held`) but never a `DanceNote`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DanceNote {
    pub key: u8,
    /// 0…1.
    pub velocity: f32,
    /// Where the song was when it started: quarter notes and seconds from the song start.
    pub beats: f64,
    pub seconds: f64,
    /// Notes the unit was given since it was made: a new note, even on the same key at the
    /// same beat after a loop, is a new count.
    pub count: u32,
    /// Where the song was when it was let go (quarter notes); `None` while it is held. A
    /// stop or a loop wrap lets it go where the song was.
    pub released: Option<f64>,
}

/// One block's account, as it stood at the block's end.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DanceFrame {
    /// Quarter notes and seconds from the song start (the tempo map's).
    pub beats: f64,
    pub seconds: f64,
    pub tempo_bpm: f32,
    pub playing: bool,
    /// The bar the song is in (0-based from bar 1, through every metre change), where it
    /// started (quarter notes) and how many quarter notes it lasts: what a routine counts.
    pub bar: i32,
    pub bar_start_beats: f64,
    pub bar_beats: f32,
    /// Every number in plain units, after automation and modulation.
    pub values: [f32; VALUES],
    /// The last note in the move map, and the one before it (what the last one took over
    /// from: the pose blends from it).
    pub note: Option<DanceNote>,
    pub prev_note: Option<DanceNote>,
    /// Where `move` last changed (quarter notes) and what it was before: an automated
    /// change of move blends from the one it left. `None` until it changes (and after a
    /// seek).
    pub move_change: Option<(f64, f32)>,
    /// The keys held now (bit `k` of `held[k / 64]`).
    pub held: [u64; 2],
}

impl Default for DanceFrame {
    fn default() -> Self {
        let mut values = [0.0; VALUES];
        for (v, p) in values.iter_mut().zip(cantus_core::inst::dancer::PARAMS) {
            *v = p.default;
        }
        DanceFrame {
            beats: 0.0,
            seconds: 0.0,
            tempo_bpm: 120.0,
            playing: false,
            bar: 0,
            bar_start_beats: 0.0,
            bar_beats: 4.0,
            values,
            note: None,
            prev_note: None,
            move_change: None,
            held: [0; 2],
        }
    }
}

impl DanceFrame {
    /// A number by key (`None` for a key the dancer does not have).
    pub fn value(&self, key: &str) -> Option<f32> {
        cantus_core::inst::dancer::KEYS
            .iter()
            .position(|k| *k == key)
            .map(|i| self.values[i])
    }

    pub fn is_held(&self, key: u8) -> bool {
        self.held[usize::from(key) / 64] & (1 << (key % 64)) != 0
    }

    /// The same frame with the song moved to `seconds` at its tempo (playing; stopped, the
    /// song stands where it is): the bar counted on or back across its bar lines. What
    /// places a frame at the exact moment shown — a video's frame time, the position
    /// heard — when it was published at its segment's end.
    pub fn at_seconds(&self, seconds: f64) -> DanceFrame {
        let mut f = *self;
        if !f.playing {
            return f;
        }
        let dt = seconds - f.seconds;
        f.seconds = seconds;
        f.beats += dt * f64::from(f.tempo_bpm) / 60.0;
        let len = f64::from(f.bar_beats).max(0.25);
        while f.beats >= f.bar_start_beats + len {
            f.bar += 1;
            f.bar_start_beats += len;
        }
        while f.beats < f.bar_start_beats && f.bar > 0 {
            f.bar -= 1;
            f.bar_start_beats -= len;
        }
        f
    }
}

impl DanceFrame {
    /// [`Self::at_seconds`] by quarter notes: the frame moved to `beats` at its tempo.
    pub fn at_beats(&self, beats: f64) -> DanceFrame {
        if !self.playing {
            return *self;
        }
        let tempo = f64::from(self.tempo_bpm).max(1.0);
        let mut f = self.at_seconds(self.seconds + (beats - self.beats) * 60.0 / tempo);
        f.beats = beats;
        f
    }
}

/// The frame of a timeline in song order (an export plays straight through) at `seconds`:
/// the frame of the segment that holds that moment — the first one published after it, as
/// each tells how its segment ended — moved back to the moment itself. A segment ending
/// within a tenth of a microsecond of the moment ends there (its seconds are summed in
/// floating point; a segment is never shorter than a sample). `None` for an empty
/// timeline.
pub fn sample(timeline: &[DanceFrame], seconds: f64) -> Option<DanceFrame> {
    let i = timeline.partition_point(|f| f.seconds <= seconds + 1e-7);
    timeline
        .get(i)
        .or(timeline.last())
        .map(|f| f.at_seconds(seconds))
}

/// One [`DanceNote`] in atomics (inside the feed's sequence lock).
#[derive(Debug, Default)]
struct NoteCell {
    has: AtomicBool,
    key: AtomicU32,
    velocity: AtomicU32,
    beats: AtomicU64,
    seconds: AtomicU64,
    count: AtomicU32,
    has_release: AtomicBool,
    released: AtomicU64,
}

impl NoteCell {
    fn store(&self, n: Option<DanceNote>) {
        self.has.store(n.is_some(), Ordering::Relaxed);
        let n = n.unwrap_or_default();
        self.key.store(u32::from(n.key), Ordering::Relaxed);
        self.velocity.store(n.velocity.to_bits(), Ordering::Relaxed);
        self.beats.store(n.beats.to_bits(), Ordering::Relaxed);
        self.seconds.store(n.seconds.to_bits(), Ordering::Relaxed);
        self.count.store(n.count, Ordering::Relaxed);
        self.has_release
            .store(n.released.is_some(), Ordering::Relaxed);
        self.released
            .store(n.released.unwrap_or(0.0).to_bits(), Ordering::Relaxed);
    }

    fn load(&self) -> Option<DanceNote> {
        self.has.load(Ordering::Relaxed).then(|| DanceNote {
            key: self.key.load(Ordering::Relaxed) as u8,
            velocity: f32::from_bits(self.velocity.load(Ordering::Relaxed)),
            beats: f64::from_bits(self.beats.load(Ordering::Relaxed)),
            seconds: f64::from_bits(self.seconds.load(Ordering::Relaxed)),
            count: self.count.load(Ordering::Relaxed),
            released: self
                .has_release
                .load(Ordering::Relaxed)
                .then(|| f64::from_bits(self.released.load(Ordering::Relaxed))),
        })
    }
}

/// Where a dancer's unit publishes and the interface reads.
#[derive(Debug)]
pub struct DanceFeed {
    seq: AtomicU64,
    beats: AtomicU64,
    seconds: AtomicU64,
    tempo: AtomicU32,
    playing: AtomicBool,
    bar: AtomicU32,
    bar_start: AtomicU64,
    bar_beats: AtomicU32,
    values: [AtomicU32; VALUES],
    notes: [NoteCell; 2],
    has_change: AtomicBool,
    change_at: AtomicU64,
    change_from: AtomicU32,
    held: [AtomicU64; 2],
    /// Frames published so far (a reader sees whether anything new came).
    published: AtomicU64,
    keep: AtomicBool,
    timeline: Mutex<Vec<DanceFrame>>,
}

impl Default for DanceFeed {
    fn default() -> Self {
        let f = DanceFeed {
            seq: AtomicU64::new(0),
            beats: AtomicU64::new(0),
            seconds: AtomicU64::new(0),
            tempo: AtomicU32::new(0),
            playing: AtomicBool::new(false),
            bar: AtomicU32::new(0),
            bar_start: AtomicU64::new(0),
            bar_beats: AtomicU32::new(0),
            values: std::array::from_fn(|_| AtomicU32::new(0)),
            notes: Default::default(),
            has_change: AtomicBool::new(false),
            change_at: AtomicU64::new(0),
            change_from: AtomicU32::new(0),
            held: [AtomicU64::new(0), AtomicU64::new(0)],
            published: AtomicU64::new(0),
            keep: AtomicBool::new(false),
            timeline: Mutex::new(Vec::new()),
        };
        f.store(&DanceFrame::default());
        f.published.store(0, Ordering::Release);
        f
    }
}

impl DanceFeed {
    /// Publish a block's frame. Real-time safe (atomics only) unless a video export asked
    /// for the timeline (a render's worker thread), which appends to it.
    pub fn publish(&self, frame: &DanceFrame) {
        self.store(frame);
        if self.keep.load(Ordering::Relaxed) {
            if let Ok(mut t) = self.timeline.lock() {
                t.push(*frame);
            }
        }
    }

    fn store(&self, f: &DanceFrame) {
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);
        self.beats.store(f.beats.to_bits(), Ordering::Relaxed);
        self.seconds.store(f.seconds.to_bits(), Ordering::Relaxed);
        self.tempo.store(f.tempo_bpm.to_bits(), Ordering::Relaxed);
        self.playing.store(f.playing, Ordering::Relaxed);
        self.bar.store(f.bar as u32, Ordering::Relaxed);
        self.bar_start
            .store(f.bar_start_beats.to_bits(), Ordering::Relaxed);
        self.bar_beats
            .store(f.bar_beats.to_bits(), Ordering::Relaxed);
        for (a, v) in self.values.iter().zip(f.values) {
            a.store(v.to_bits(), Ordering::Relaxed);
        }
        self.notes[0].store(f.note);
        self.notes[1].store(f.prev_note);
        self.has_change
            .store(f.move_change.is_some(), Ordering::Relaxed);
        let (at, from) = f.move_change.unwrap_or_default();
        self.change_at.store(at.to_bits(), Ordering::Relaxed);
        self.change_from.store(from.to_bits(), Ordering::Relaxed);
        for (a, h) in self.held.iter().zip(f.held) {
            a.store(h, Ordering::Relaxed);
        }
        self.seq.store(s.wrapping_add(2), Ordering::Release);
        self.published.fetch_add(1, Ordering::Release);
    }

    /// The latest frame, or `None` when every try met the writer mid-way (the caller keeps
    /// the frame it had).
    pub fn latest(&self) -> Option<DanceFrame> {
        for _ in 0..8 {
            let s0 = self.seq.load(Ordering::Acquire);
            if s0 % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let f = self.load();
            std::sync::atomic::fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == s0 {
                return Some(f);
            }
        }
        None
    }

    fn load(&self) -> DanceFrame {
        DanceFrame {
            beats: f64::from_bits(self.beats.load(Ordering::Relaxed)),
            seconds: f64::from_bits(self.seconds.load(Ordering::Relaxed)),
            tempo_bpm: f32::from_bits(self.tempo.load(Ordering::Relaxed)),
            playing: self.playing.load(Ordering::Relaxed),
            bar: self.bar.load(Ordering::Relaxed) as i32,
            bar_start_beats: f64::from_bits(self.bar_start.load(Ordering::Relaxed)),
            bar_beats: f32::from_bits(self.bar_beats.load(Ordering::Relaxed)),
            values: std::array::from_fn(|i| f32::from_bits(self.values[i].load(Ordering::Relaxed))),
            note: self.notes[0].load(),
            prev_note: self.notes[1].load(),
            move_change: self.has_change.load(Ordering::Relaxed).then(|| {
                (
                    f64::from_bits(self.change_at.load(Ordering::Relaxed)),
                    f32::from_bits(self.change_from.load(Ordering::Relaxed)),
                )
            }),
            held: [
                self.held[0].load(Ordering::Relaxed),
                self.held[1].load(Ordering::Relaxed),
            ],
        }
    }

    /// Frames published since the feed was made.
    pub fn published(&self) -> u64 {
        self.published.load(Ordering::Acquire)
    }

    /// Keep every frame from now on (a video export's render asked), or stop keeping them.
    pub fn keep_timeline(&self, keep: bool) {
        self.keep.store(keep, Ordering::Relaxed);
    }

    /// Whether it keeps every frame.
    pub fn keeps_timeline(&self) -> bool {
        self.keep.load(Ordering::Relaxed)
    }

    /// The frames kept, oldest first; the timeline is emptied.
    pub fn take_timeline(&self) -> Vec<DanceFrame> {
        self.timeline
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default()
    }
}

/// The feeds of the dancers' units a handle has sent, by track, each with the serial of the
/// unit that publishes into it: the interface asks for a track's feed and gets it only
/// while the engine holds (or is about to get) that very unit.
#[derive(Debug, Default)]
pub struct DanceFeeds {
    map: HashMap<TrackId, (u64, Arc<DanceFeed>)>,
}

impl DanceFeeds {
    /// A unit is on its way to the engine for `track`: keep its feed, if it has one.
    pub fn insert(&mut self, track: TrackId, serial: u64, feed: Option<Arc<DanceFeed>>) {
        match feed {
            Some(f) => {
                self.map.insert(track, (serial, f));
            }
            None => {
                self.map.remove(&track);
            }
        }
    }

    /// The feed of the unit with this serial on `track`.
    pub fn get(&self, track: TrackId, serial: Option<u64>) -> Option<Arc<DanceFeed>> {
        self.map
            .get(&track)
            .filter(|(s, _)| Some(*s) == serial)
            .map(|(_, f)| f.clone())
    }

    /// Forget every feed whose unit `keep` does not name.
    pub fn retain(&mut self, keep: impl Fn(TrackId, u64) -> bool) {
        self.map.retain(|t, (s, _)| keep(*t, *s));
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// A render's dancers' feeds (`RenderProgress::dance`): the render keeps them as it makes
/// its units, and whoever ran it reads every dancer's timeline afterwards — what a video
/// export samples at its frame times.
#[derive(Debug, Default)]
pub struct RenderFeeds {
    feeds: Mutex<Vec<(TrackId, Arc<DanceFeed>)>>,
    /// The render's owner wants every dancer's timeline (a video export): asked before
    /// the render, never otherwise.
    want: AtomicBool,
}

impl RenderFeeds {
    /// Ask, before the render, for every dancer's timeline (a video export does; nothing
    /// else should: a timeline holds a frame for every segment of the song).
    pub fn want_timelines(&self) {
        self.want.store(true, Ordering::Relaxed);
    }

    /// Called by the render, off the audio thread, before the units go to the processor:
    /// the feeds kept, and each told to keep its timeline when one was asked for.
    pub fn keep(&self, feeds: Vec<(TrackId, Arc<DanceFeed>)>) {
        let want = self.want.load(Ordering::Relaxed);
        for (_, f) in &feeds {
            f.keep_timeline(want);
        }
        if let Ok(mut f) = self.feeds.lock() {
            *f = feeds;
        }
    }

    /// Every dancer's frames, by track, oldest first (each feed's timeline is emptied).
    pub fn timelines(&self) -> Vec<(TrackId, Vec<DanceFrame>)> {
        self.feeds
            .lock()
            .map(|f| f.iter().map(|(t, d)| (*t, d.take_timeline())).collect())
            .unwrap_or_default()
    }
}

/// The feeds of a render's units, by track, taken before the units go to the render (a
/// video export samples their timelines afterwards).
pub fn feeds_of(units: &[Box<crate::inst::NativeUnit>]) -> Vec<(TrackId, Arc<DanceFeed>)> {
    units
        .iter()
        .filter_map(|u| u.unit.dance_feed().map(|f| (u.track, f)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_comes_back_as_it_was_published() {
        let feed = DanceFeed::default();
        assert_eq!(feed.published(), 0);
        assert_eq!(feed.latest(), Some(DanceFrame::default()));
        let mut f = DanceFrame {
            beats: 12.5,
            seconds: 6.25,
            tempo_bpm: 120.0,
            playing: true,
            note: Some(DanceNote {
                key: 50,
                velocity: 0.8,
                beats: 12.0,
                seconds: 6.0,
                count: 3,
                released: Some(12.5),
            }),
            prev_note: Some(DanceNote {
                key: 49,
                count: 2,
                ..DanceNote::default()
            }),
            move_change: Some((8.0, 3.0)),
            ..DanceFrame::default()
        };
        f.values[0] = 5.0;
        f.held[0] = 1 << 50;
        feed.publish(&f);
        assert_eq!(feed.latest(), Some(f));
        assert_eq!(feed.published(), 1);
        assert!(f.is_held(50) && !f.is_held(51));
        assert_eq!(f.value("move"), Some(5.0));
        assert!(feed.take_timeline().is_empty(), "live keeps no timeline");
    }

    #[test]
    fn a_timeline_asked_for_keeps_every_frame() {
        let feed = DanceFeed::default();
        feed.keep_timeline(true);
        for i in 0..5 {
            feed.publish(&DanceFrame {
                beats: f64::from(i),
                ..DanceFrame::default()
            });
        }
        let t = feed.take_timeline();
        assert_eq!(t.len(), 5);
        assert_eq!(t[4].beats, 4.0);
        assert!(feed.take_timeline().is_empty());
    }

    #[test]
    fn a_frame_moves_to_the_moment_shown_across_bar_lines() {
        let f = DanceFrame {
            beats: 8.02,
            seconds: 4.01,
            playing: true,
            bar: 2,
            bar_start_beats: 8.0,
            ..DanceFrame::default()
        };
        let back = f.at_seconds(3.995);
        assert!((back.beats - 7.99).abs() < 1e-9);
        assert_eq!((back.bar, back.bar_start_beats), (1, 4.0));
        let on = f.at_seconds(6.0);
        assert!((on.beats - 12.0).abs() < 1e-9);
        assert_eq!((on.bar, on.bar_start_beats), (3, 12.0));
        let stopped = DanceFrame {
            playing: false,
            ..f
        };
        assert_eq!(stopped.at_seconds(9.0), stopped);
        let b = f.at_beats(12.5);
        assert!((b.seconds - 6.25).abs() < 1e-9 && b.beats == 12.5 && b.bar == 3);
    }

    #[test]
    fn a_timeline_is_sampled_from_the_segment_that_holds_the_moment() {
        let t: Vec<DanceFrame> = (1..=4)
            .map(|i| {
                let mut f = DanceFrame {
                    beats: f64::from(i),
                    seconds: f64::from(i) * 0.5,
                    playing: true,
                    ..DanceFrame::default()
                };
                f.values[0] = i as f32;
                f
            })
            .collect();
        // 0.5 s is where the first segment ended: the moment belongs to the second.
        assert_eq!(sample(&t, 0.5).unwrap().values[0], 2.0);
        assert_eq!(sample(&t, 0.49).unwrap().values[0], 1.0);
        assert!((sample(&t, 0.75).unwrap().beats - 1.5).abs() < 1e-12);
        assert_eq!(
            sample(&t, 9.0).unwrap().values[0],
            4.0,
            "past the end: the last"
        );
        assert!(sample(&[], 1.0).is_none());
    }

    #[test]
    fn a_feed_is_given_only_for_the_unit_the_engine_holds() {
        let mut feeds = DanceFeeds::default();
        let t = TrackId(3);
        let f = Arc::new(DanceFeed::default());
        feeds.insert(t, 10, Some(f.clone()));
        assert!(feeds.get(t, Some(10)).is_some_and(|g| Arc::ptr_eq(&g, &f)));
        assert!(feeds.get(t, Some(11)).is_none() && feeds.get(t, None).is_none());
        feeds.retain(|_, s| s == 11);
        assert!(feeds.is_empty());
        feeds.insert(t, 12, Some(f));
        feeds.insert(t, 13, None);
        assert!(
            feeds.is_empty(),
            "a track that hosts something else forgets it"
        );
    }
}
