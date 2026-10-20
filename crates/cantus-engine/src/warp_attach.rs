//! The sample cache's side of the warp renders (ADR 0061 §6): which render each warped
//! clip plays, what is on its way, and letting go of renders nothing plays — never on the
//! audio thread (a render waits in the graveyard until this store is its last holder; a
//! render or bridge replaced or let go goes there, never dropped in place).
//!
//! **What plays.** A clip piece (a clip, or one repetition of a looped clip) plays its
//! render; while that is on its way, the render's **bridge** (`warp_render::bridge`: the
//! right speed and pitch, made quickly on its own worker); before that, of the renders and
//! bridges its clip has, the one **asked for most recently** — so while a marker or the
//! tempo is dragged the clip plays the latest bridge made, and a slow final of an earlier
//! step arriving late never brings that step back (round 29's second review); and with none
//! at all (a clip warped for the first time), its **input** — the source, or its pitch
//! render — laid as the unwarped rule lays it: the right time, the pitch late, never
//! silence. A render plays from the window's first sample (`zero_at_start`); when a piece's
//! data changes while it plays, the snapshot carries the old data, and the engine
//! crossfades the two (`fade_from`) — never a click.
//!
//! **Kept.** What the last live snapshot plays — whatever a foreign compile (a bounce, a
//! freeze, a comp merge, an export) asks meanwhile — and beyond it the renders and bridges
//! asked for most recently, within [`KEEP_BYTES`]: muting a clip, auditioning a pattern or
//! rendering a comp merge never throws a playing clip's render away, and undo, redo or
//! unmuting play theirs at once.
//!
//! **Waited for.** Finals are cancelled only when nothing plays their key any more (a
//! loop's pieces each keep theirs); a final still being made for a key nothing plays no
//! longer counts as pending, so an export never waits for a deleted clip.

use super::{render_key, Finished, Job, Plan, WarpService};
use cantus_core::snapshot::FadeFrom;
use cantus_core::{ClipId, SampleData, Snapshot, Ticks};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The renders and bridges remembered per clip piece.
pub const HISTORY: usize = 4;
/// The most the kept (not playing) renders and bridges may hold, bytes.
pub const KEEP_BYTES: usize = 512 << 20;
/// Attaches a piece may be gone for before its history goes (its clip's newest stays).
const GRACE: u64 = 64;

/// What a warped clip's input is when it can be rendered: its key (the source's, or the
/// pitch render's); `None` while the input is not the one the clip will play (a pitch
/// render on its way).
pub(crate) type InputKey<'a> = &'a dyn Fn(&cantus_core::AudioClipSnapshot, &Path) -> Option<u64>;

/// A clip piece: the clip and where the piece starts.
type Piece = (ClipId, Ticks);

#[derive(Debug, Default)]
pub(crate) struct Renders {
    pub service: Arc<WarpService>,
    done: HashMap<u64, Arc<SampleData>>,
    bridges: HashMap<u64, Arc<SampleData>>,
    /// Finals on their way for keys something plays, with the file each one renders.
    pending: HashMap<u64, PathBuf>,
    failed: HashMap<u64, (PathBuf, String)>,
    /// Each piece's renders and bridges, newest asked first.
    recent: HashMap<Piece, Vec<u64>>,
    /// When each key was last asked for (an attach's count) — never when it arrived: what
    /// a fallback orders by.
    stamp: HashMap<u64, u64>,
    /// When each piece was last in an attached snapshot.
    seen: HashMap<Piece, u64>,
    /// What the last live snapshot plays (renders, bridges, fallbacks): kept whatever a
    /// foreign compile asks.
    live: HashSet<u64>,
    /// Each piece's data in the last live snapshot (for a swap's crossfade).
    played: HashMap<Piece, FadeFrom>,
    generation: u64,
    graveyard: Vec<Arc<SampleData>>,
    /// The budget of the kept renders, bytes ([`KEEP_BYTES`] at 0; a test lowers it).
    budget: usize,
}

impl Renders {
    fn stamp_of(&self, k: u64) -> u64 {
        self.stamp.get(&k).copied().unwrap_or(0)
    }

    /// `key` joins `piece`'s history (newest asked first, at most [`HISTORY`]).
    fn remember(&mut self, piece: Piece, key: u64) {
        let mut h = self.recent.remove(&piece).unwrap_or_default();
        h.retain(|k| *k != key);
        h.push(key);
        h.sort_by_key(|k| std::cmp::Reverse((self.stamp_of(*k), *k)));
        h.truncate(HISTORY);
        self.recent.insert(piece, h);
    }

    fn put_done(&mut self, key: u64, d: SampleData) {
        if let Some(old) = self.done.insert(key, Arc::new(d)) {
            self.graveyard.push(old);
        }
        if let Some(b) = self.bridges.remove(&key) {
            self.graveyard.push(b);
        }
    }

    fn put_bridge(&mut self, key: u64, d: SampleData) {
        if let Some(old) = self.bridges.insert(key, Arc::new(d)) {
            self.graveyard.push(old);
        }
    }

    /// The render (else the bridge) held for `k` at `rate`.
    fn held(&self, k: u64, rate: u32) -> Option<&Arc<SampleData>> {
        self.done
            .get(&k)
            .or_else(|| self.bridges.get(&k))
            .filter(|d| d.rate == rate)
    }

    /// What a piece falls back on: of the renders and bridges its clip holds at `rate`,
    /// the one asked for most recently (the piece's own first among equals: a loop's
    /// pieces are asked together).
    fn fallback(&self, piece: Piece, rate: u32) -> Option<(u64, Arc<SampleData>)> {
        self.recent
            .iter()
            .filter(|(p, _)| p.0 == piece.0)
            .flat_map(|(p, h)| h.iter().map(move |k| (*p == piece, *k)))
            .filter(|(_, k)| self.held(*k, rate).is_some())
            .max_by_key(|(own, k)| (self.stamp_of(*k), *own, *k))
            .and_then(|(_, k)| self.held(k, rate).map(|d| (k, d.clone())))
    }

    /// Attach every warped clip's render, made here (`background` off) or queued (on).
    /// `live`: the snapshot is the one that will play (not a foreign or offline compile's).
    pub fn attach(
        &mut self,
        snapshot: &mut Snapshot,
        rate: u32,
        background: bool,
        resolve: &dyn Fn(&str) -> PathBuf,
        input: InputKey,
        live: bool,
    ) -> Vec<(PathBuf, String)> {
        self.generation += 1;
        let now = self.generation;
        let mut errors = Vec::new();
        let mut used: HashSet<u64> = HashSet::new();
        let mut played: HashMap<Piece, FadeFrom> = HashMap::new();
        let map = snapshot.tempo.clone();
        for track in &mut snapshot.tracks {
            for clip in &mut track.audio {
                let Some(warp) = clip.warp.clone() else {
                    continue;
                };
                let Some(data) = clip.data.clone() else {
                    continue; // A source that is not there plays nothing, warped or not.
                };
                let piece = (clip.clip, clip.start);
                self.seen.insert(piece, now);
                let path = resolve(&clip.path);
                let plan = Plan {
                    warp,
                    map: map.clone(),
                    start: clip.start,
                    end: clip.end,
                    offset: clip.offset,
                    slip_frames: clip.slip * clip.source_rate as f64,
                    source_rate: clip.source_rate,
                    rate,
                    path: path.clone(),
                    input_key: input(clip, &path).unwrap_or(0),
                };
                let ready = input(clip, &path).is_some();
                let key = render_key(&plan);
                used.insert(key);
                self.stamp.insert(key, now);
                if ready
                    && !self.done.contains_key(&key)
                    && !self.failed.contains_key(&key)
                    && !self.pending.contains_key(&key)
                {
                    let job = Job {
                        clip: clip.clip,
                        piece: clip.start,
                        key,
                        input: data.clone(),
                        plan: Arc::new(plan),
                    };
                    if background {
                        let bridge = !self.bridges.contains_key(&key);
                        self.service.queue_render(job, bridge);
                        self.pending.insert(key, path.clone());
                    } else {
                        match self.service.render_now(&job) {
                            Ok(d) => self.put_done(key, d),
                            Err(e) => {
                                self.failed.insert(key, (path.clone(), e));
                            }
                        }
                    }
                }
                // (the data, whether its frame 0 is the window's start)
                let (playing, at_start) = if let Some(d) = self.held(key, rate).cloned() {
                    self.remember(piece, key);
                    (Some(d), true)
                } else if let Some((_, e)) = self.failed.get(&key) {
                    if !errors.iter().any(|(p, _)| *p == path) {
                        errors.push((path.clone(), e.clone()));
                    }
                    (None, true)
                } else if let Some((k, d)) = self.fallback(piece, rate) {
                    // What it plays is kept while it plays (round 29's second review).
                    used.insert(k);
                    (Some(d), true)
                } else {
                    // Nothing made for it yet: its input, laid as unwarped (the clip's own
                    // offset and fine offset) — never silence (round 29's second review).
                    (Some(data), false)
                };
                clip.data = playing;
                clip.zero_at_start = at_start;
                if live {
                    if let Some(d) = clip.data.clone() {
                        if let Some(before) = self.played.get(&piece) {
                            if !Arc::ptr_eq(&before.data, &d) {
                                clip.fade_from = Some(before.clone());
                            }
                        }
                        played.insert(
                            piece,
                            FadeFrom {
                                data: d,
                                slip: clip.slip,
                                zero_at_start: at_start,
                            },
                        );
                    }
                }
            }
        }
        if live {
            self.played = played;
            self.live = used.clone();
        }
        let protected: HashSet<u64> = used.union(&self.live).copied().collect();
        // What nothing plays any more is not made (a waiting final, a waiting bridge), and
        // a final still being made for such a key is not waited for.
        for k in self.service.retain_queued(&protected) {
            self.pending.remove(&k);
        }
        self.pending.retain(|k, _| protected.contains(k));
        self.keep(&protected);
        self.sweep();
        errors
    }

    /// The budget of the kept renders (tests).
    pub fn set_budget(&mut self, bytes: usize) {
        self.budget = bytes.max(1);
    }

    fn budget(&self) -> usize {
        if self.budget == 0 {
            KEEP_BYTES
        } else {
            self.budget
        }
    }

    /// Keep what plays (`protected`), and the renders and bridges asked for most recently
    /// within the budget; the rest to the graveyard. A piece gone for long lets its history
    /// go (its clip's newest piece keeps its own).
    fn keep(&mut self, protected: &HashSet<u64>) {
        let mut rest: Vec<(u64, u64, usize)> = self
            .done
            .iter()
            .chain(self.bridges.iter())
            .filter(|(k, _)| !protected.contains(k))
            .map(|(k, d)| (*k, self.stamp_of(*k), d.frames.len() * 4))
            .collect();
        rest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut budget = self.budget();
        let mut gone: HashSet<u64> = HashSet::new();
        for (k, _, size) in rest {
            if size <= budget {
                budget -= size;
            } else {
                gone.insert(k);
            }
        }
        for k in &gone {
            if let Some(d) = self.done.remove(k) {
                self.graveyard.push(d);
            }
            if let Some(d) = self.bridges.remove(k) {
                self.graveyard.push(d);
            }
            self.stamp.remove(k);
        }
        let mut recent = std::mem::take(&mut self.recent);
        for h in recent.values_mut() {
            h.retain(|k| self.done.contains_key(k) || self.bridges.contains_key(k));
        }
        recent.retain(|_, h| !h.is_empty());
        // Each clip's piece asked for last.
        let mut newest: HashMap<ClipId, (u64, Piece)> = HashMap::new();
        for (p, h) in &recent {
            let s = h.iter().map(|k| self.stamp_of(*k)).max().unwrap_or(0);
            let e = newest.entry(p.0).or_insert((s, *p));
            if (s, p.1) > (e.0, e.1 .1) {
                *e = (s, *p);
            }
        }
        let now = self.generation;
        let seen = &self.seen;
        let fresh = |p: &Piece| seen.get(p).is_some_and(|s| now - s <= GRACE);
        recent.retain(|p, _| fresh(p) || newest.get(&p.0).is_some_and(|n| n.1 == *p));
        self.seen
            .retain(|p, s| now - *s <= GRACE || recent.contains_key(p));
        self.recent = recent;
    }

    /// Free the renders only this store still holds.
    pub fn sweep(&mut self) {
        self.graveyard.retain(|d| Arc::strong_count(d) > 1);
    }

    /// Take in what the workers finished; whether anything arrived. A render or bridge for
    /// a key nothing plays any more is still kept — at the place its key was asked for in
    /// its piece's history, never ahead of what was asked since (round 29's second review).
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        let finished: Vec<Finished> = self.service.take_finished();
        for f in finished {
            any = true;
            let piece = (f.clip, f.piece);
            if f.bridge {
                if let Ok(d) = f.result {
                    if !self.done.contains_key(&f.key) {
                        self.put_bridge(f.key, d);
                        self.remember(piece, f.key);
                    }
                }
                continue;
            }
            let path = self.pending.remove(&f.key).unwrap_or_default();
            match f.result {
                Ok(d) => {
                    self.put_done(f.key, d);
                    self.remember(piece, f.key);
                }
                Err(e) => {
                    self.failed.insert(f.key, (path, e));
                }
            }
        }
        self.sweep();
        any
    }

    /// The file at `path` is to be tried again.
    pub fn forget(&mut self, path: &Path) {
        self.service.forget(path);
        self.failed.retain(|_, (p, _)| p != path);
    }

    /// Final renders waiting or being made for what plays.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Renders held (playing, kept, bridges, waiting to be freed).
    pub fn held_count(&self) -> usize {
        self.done.len() + self.bridges.len() + self.graveyard.len()
    }
}

#[cfg(test)]
#[path = "warp_attach_tests.rs"]
mod tests;
