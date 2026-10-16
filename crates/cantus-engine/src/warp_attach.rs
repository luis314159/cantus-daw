//! The sample cache's side of the warp renders (ADR 0061 §6): which render each warped
//! clip plays, what is on its way, and letting go of renders nothing plays — never on the
//! audio thread (a render waits here until this store is its last holder).
//!
//! **While a render is on its way** the clip plays, in order: the render's **bridge** (the
//! same map at varispeed, made in milliseconds on its own worker), or the newest render it
//! had — never its source at the wrong speed; a clip warped for the first time is silent
//! until its bridge arrives (a few milliseconds). **Kept:** each clip's last [`HISTORY`]
//! renders (undo and redo play theirs at once), within [`KEEP_BYTES`] beyond what plays.

use super::{render_key, Job, Plan, WarpService};
use cantus_core::{ClipId, SampleData, Snapshot};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The renders kept per clip: the one it plays and the ones before.
pub const HISTORY: usize = 3;
/// The most the kept (not playing) renders may hold, bytes.
pub const KEEP_BYTES: usize = 512 << 20;

/// What a warped clip's input is when it can be rendered: its key (the source's, or the
/// pitch render's); `None` while the input is not the one the clip will play (a pitch
/// render on its way).
pub(crate) type InputKey<'a> = &'a dyn Fn(&cantus_core::AudioClipSnapshot, &Path) -> Option<u64>;

#[derive(Debug, Default)]
pub(crate) struct Renders {
    pub service: Arc<WarpService>,
    done: HashMap<u64, Arc<SampleData>>,
    bridges: HashMap<u64, Arc<SampleData>>,
    /// Finals on their way, with the file each one renders.
    pending: HashMap<u64, PathBuf>,
    failed: HashMap<u64, (PathBuf, String)>,
    /// Each clip's recent render keys, newest first.
    last: HashMap<ClipId, Vec<u64>>,
    graveyard: Vec<Arc<SampleData>>,
}

impl Renders {
    fn remember(&mut self, clip: ClipId, key: u64) {
        let h = self.last.entry(clip).or_default();
        h.retain(|k| *k != key);
        h.insert(0, key);
        h.truncate(HISTORY);
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
        let mut errors = Vec::new();
        let mut used: HashSet<u64> = HashSet::new();
        let mut seen: HashSet<ClipId> = HashSet::new();
        let map = snapshot.tempo.clone();
        for track in &mut snapshot.tracks {
            for clip in &mut track.audio {
                let Some(warp) = clip.warp.clone() else {
                    continue;
                };
                seen.insert(clip.clip);
                let Some(data) = clip.data.clone() else {
                    continue; // A source that is not there plays nothing, warped or not.
                };
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
                        key,
                        input: data.clone(),
                        plan: Arc::new(plan),
                    };
                    if background {
                        let bridge = !self.bridges.contains_key(&key);
                        if let Some(old) = self.service.queue_render(job, bridge) {
                            self.pending.remove(&old);
                        }
                        self.pending.insert(key, path.clone());
                    } else {
                        match self.service.render_now(&job) {
                            Ok(d) => {
                                self.done.insert(key, Arc::new(d));
                            }
                            Err(e) => {
                                self.failed.insert(key, (path.clone(), e));
                            }
                        }
                    }
                }
                let played = if let Some(d) = self.done.get(&key) {
                    let d = d.clone();
                    self.remember(clip.clip, key);
                    Some(d)
                } else if let Some((_, e)) = self.failed.get(&key) {
                    if !errors.iter().any(|(p, _)| *p == path) {
                        errors.push((path.clone(), e.clone()));
                    }
                    None
                } else if let Some(b) = self.bridges.get(&key) {
                    Some(b.clone())
                } else {
                    self.last.get(&clip.clip).and_then(|h| {
                        h.iter()
                            .filter_map(|k| self.done.get(k))
                            .find(|d| d.rate == rate)
                            .cloned()
                    })
                };
                clip.data = played;
                clip.slip = zero_slip;
            }
        }
        self.last.retain(|c, _| seen.contains(c));
        // Kept: what plays, what is coming, each clip's history within the budget.
        let mut keep: HashSet<u64> = used.clone();
        let mut budget = KEEP_BYTES;
        for h in self.last.values() {
            for k in h {
                if keep.contains(k) {
                    continue;
                }
                let size = self.done.get(k).map_or(0, |d| d.frames.len() * 4);
                if size <= budget {
                    budget -= size;
                    keep.insert(*k);
                }
            }
        }
        let gone: Vec<u64> = self
            .done
            .keys()
            .filter(|k| !keep.contains(k))
            .copied()
            .collect();
        for k in gone {
            if let Some(d) = self.done.remove(&k) {
                self.graveyard.push(d);
            }
        }
        // A bridge is only for the render it stands in for.
        let stale: Vec<u64> = self
            .bridges
            .keys()
            .filter(|k| !self.pending.contains_key(k) || !used.contains(k))
            .copied()
            .collect();
        for k in stale {
            if let Some(d) = self.bridges.remove(&k) {
                self.graveyard.push(d);
            }
        }
        self.sweep();
        errors
    }

    /// Free the renders only this store still holds.
    pub fn sweep(&mut self) {
        self.graveyard.retain(|d| Arc::strong_count(d) > 1);
    }

    /// Take in what the workers finished; whether anything arrived.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        for (key, bridge, r) in self.service.take_finished() {
            any = true;
            if bridge {
                if let (Ok(d), true) = (r, self.pending.contains_key(&key)) {
                    self.bridges.insert(key, Arc::new(d));
                }
                continue;
            }
            let path = self.pending.remove(&key).unwrap_or_default();
            if let Some(b) = self.bridges.remove(&key) {
                self.graveyard.push(b);
            }
            match r {
                Ok(d) => {
                    self.done.insert(key, Arc::new(d));
                }
                Err(e) => {
                    self.failed.insert(key, (path, e));
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

    /// Final renders waiting or being made.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Renders held (playing, kept, bridges, waiting to be freed).
    pub fn held(&self) -> usize {
        self.done.len() + self.bridges.len() + self.graveyard.len()
    }
}
