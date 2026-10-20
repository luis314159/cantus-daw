//! The sample cache's side of the warp renders (ADR 0061 §6): which render each warped
//! clip plays, what is on its way, and letting go of renders nothing plays — never on the
//! audio thread (a render waits in the graveyard until this store is its last holder; a
//! render or bridge replaced or let go goes there, never dropped in place).
//!
//! **What plays.** A clip piece (a clip, or one repetition of a looped clip) plays its
//! render; while that is on its way, the render's **bridge** (`warp_render::bridge`: the
//! right speed and pitch, made in milliseconds on its own worker); and before that, the
//! **newest** render or bridge that piece had — so while a marker or the tempo is dragged
//! the clip plays the latest bridge made (one frame behind at most), never its source at
//! the wrong speed and never back at its source's pitch. A clip warped for the first time
//! is silent until its bridge arrives (milliseconds).
//!
//! **Kept.** What plays, and beyond it the renders and bridges most recently played or
//! made, within [`KEEP_BYTES`] — whether or not their clip is in the snapshot attached now:
//! muting a clip, auditioning a pattern or rendering a comp merge never throws a clip's
//! render away (round 29's review), and undo, redo or unmuting play theirs at once.
//!
//! **Waited for.** Finals are cancelled only when nothing plays their key any more (a
//! loop's pieces each keep theirs); a final still being made for a key nothing plays no
//! longer counts as pending, so an export never waits for a deleted clip.

use super::{render_key, Finished, Job, Plan, WarpService};
use cantus_core::{ClipId, SampleData, Snapshot, Ticks};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The renders and bridges remembered per clip piece, newest first.
pub const HISTORY: usize = 4;
/// The most the kept (not playing) renders and bridges may hold, bytes.
pub const KEEP_BYTES: usize = 512 << 20;

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
    /// Each piece's renders and bridges, newest first.
    recent: HashMap<Piece, Vec<u64>>,
    /// When each held render or bridge was last played or made (an attach count).
    stamp: HashMap<u64, u64>,
    generation: u64,
    graveyard: Vec<Arc<SampleData>>,
}

impl Renders {
    fn remember(&mut self, piece: Piece, key: u64) {
        let h = self.recent.entry(piece).or_default();
        h.retain(|k| *k != key);
        h.insert(0, key);
        h.truncate(HISTORY);
        self.stamp.insert(key, self.generation);
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

    /// What a piece falls back on: its newest render or bridge at `rate`, else the newest
    /// any piece of the same clip had.
    fn fallback(&self, piece: Piece, rate: u32) -> Option<Arc<SampleData>> {
        let held = |k: &u64| {
            self.done
                .get(k)
                .or_else(|| self.bridges.get(k))
                .filter(|d| d.rate == rate)
        };
        if let Some(d) = self
            .recent
            .get(&piece)
            .and_then(|h| h.iter().find_map(held))
        {
            return Some(d.clone());
        }
        self.recent
            .iter()
            .filter(|(p, _)| p.0 == piece.0)
            .flat_map(|(_, h)| h.iter())
            .filter(|k| held(k).is_some())
            .max_by_key(|k| (self.stamp.get(*k).copied().unwrap_or(0), **k))
            .and_then(held)
            .cloned()
    }

    /// Attach every warped clip's render, made here (`background` off) or queued (on).
    pub fn attach(
        &mut self,
        snapshot: &mut Snapshot,
        rate: u32,
        background: bool,
        resolve: &dyn Fn(&str) -> PathBuf,
        input: InputKey,
    ) -> Vec<(PathBuf, String)> {
        self.generation += 1;
        let mut errors = Vec::new();
        let mut used: HashSet<u64> = HashSet::new();
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
                // The window's start is the render's frame 0 (`Transport::clip_zero`).
                let zero_slip =
                    -cantus_core::audio_time::offset_seconds(&map, clip.start, clip.offset);
                let ready = input(clip, &path).is_some();
                let key = render_key(&plan);
                used.insert(key);
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
                let played = if self.done.contains_key(&key) || self.bridges.contains_key(&key) {
                    self.remember(piece, key);
                    self.done
                        .get(&key)
                        .or_else(|| self.bridges.get(&key))
                        .cloned()
                } else if let Some((_, e)) = self.failed.get(&key) {
                    if !errors.iter().any(|(p, _)| *p == path) {
                        errors.push((path.clone(), e.clone()));
                    }
                    None
                } else {
                    self.fallback(piece, rate)
                };
                clip.data = played;
                clip.slip = zero_slip;
            }
        }
        // What nothing plays any more is not made (a waiting final, a waiting bridge), and
        // a final still being made for such a key is not waited for.
        for k in self.service.retain_queued(&used) {
            self.pending.remove(&k);
        }
        self.pending.retain(|k, _| used.contains(k));
        self.keep(&used);
        self.sweep();
        errors
    }

    /// Keep what plays, and the renders and bridges played or made most recently within
    /// the budget; the rest to the graveyard.
    fn keep(&mut self, used: &HashSet<u64>) {
        let mut rest: Vec<(u64, u64, usize)> = self
            .done
            .iter()
            .chain(self.bridges.iter())
            .filter(|(k, _)| !used.contains(k))
            .map(|(k, d)| {
                let at = self.stamp.get(k).copied().unwrap_or(0);
                (*k, at, d.frames.len() * 4)
            })
            .collect();
        rest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut budget = KEEP_BYTES;
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
        self.recent = recent;
    }

    /// Free the renders only this store still holds.
    pub fn sweep(&mut self) {
        self.graveyard.retain(|d| Arc::strong_count(d) > 1);
    }

    /// Take in what the workers finished; whether anything arrived. A render or bridge for
    /// a key nothing plays any more is still kept (as its piece's newest: the clip plays it
    /// until the render for its new key comes).
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
    pub fn held(&self) -> usize {
        self.done.len() + self.bridges.len() + self.graveyard.len()
    }
}
