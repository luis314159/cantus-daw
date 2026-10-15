//! The sample cache's side of the pitch renders: which render each clip plays, what is on
//! its way, and letting go of renders nothing plays any more — never on the audio thread:
//! a render no snapshot asks for waits here until this is its last holder (the audio
//! thread only ever decrements a count), then it is freed.
//!
//! **What is kept** (review 1): each clip's last few renders ([`HISTORY`]), so undo and
//! redo play their own render at once; and the render of a clip compared with its original
//! (*Original*, A/B), so switching back plays the edits at once. A fallback while a render
//! is on its way is only ever a render at the engine's current rate.

use super::{render_key, PitchService};
use cantus_core::{ClipId, SampleData, Snapshot};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The renders kept per clip: the one it plays and the ones before (undo, redo).
pub const HISTORY: usize = 3;

#[derive(Debug, Default)]
pub(crate) struct Renders {
    pub service: Arc<PitchService>,
    done: HashMap<u64, Arc<SampleData>>,
    /// On their way, with the file each one renders.
    pending: HashMap<u64, PathBuf>,
    /// Failed, with the file and why (forgotten when the file is tried again).
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

    /// Attach every edited clip's render, made here (`background` off) or queued on the
    /// pitch service's worker (on). `resolve` turns a stored path into the file's. A clip
    /// whose render is not made yet plays the newest render it had at this `rate`, else
    /// its source — never silence.
    pub fn attach(
        &mut self,
        snapshot: &mut Snapshot,
        rate: u32,
        background: bool,
        resolve: &dyn Fn(&str) -> PathBuf,
    ) -> Vec<(PathBuf, String)> {
        let mut errors = Vec::new();
        let mut used: HashSet<u64> = HashSet::new();
        let mut seen: HashSet<ClipId> = HashSet::new();
        for track in &mut snapshot.tracks {
            for clip in &mut track.audio {
                let Some(edit) = clip.pitch.clone().filter(|e| e.edited()) else {
                    continue;
                };
                // Its history is kept while its source loads, and while it is compared
                // with the original: the edits' render stays for the way back.
                seen.insert(clip.clip);
                let path = resolve(&clip.path);
                if edit.bypass {
                    let mut on = (*edit).clone();
                    on.bypass = false;
                    used.insert(render_key(&path, &on, rate));
                    continue;
                }
                // A source that is not there plays nothing, edited or not.
                if clip.data.is_none() {
                    continue;
                }
                let key = render_key(&path, &edit, rate);
                used.insert(key);
                if !self.done.contains_key(&key)
                    && !self.failed.contains_key(&key)
                    && !self.pending.contains_key(&key)
                {
                    if background {
                        let replaced = self.service.queue_render(
                            clip.clip,
                            key,
                            path.clone(),
                            edit.clone(),
                            rate,
                        );
                        if let Some(old) = replaced {
                            self.pending.remove(&old);
                        }
                        self.pending.insert(key, path.clone());
                    } else {
                        match self.service.render_now(&path, &edit, rate) {
                            Ok(d) => {
                                self.done.insert(key, Arc::new(d));
                            }
                            Err(e) => {
                                self.failed.insert(key, (path.clone(), e));
                            }
                        }
                    }
                }
                if let Some(d) = self.done.get(&key) {
                    clip.data = Some(d.clone());
                    self.remember(clip.clip, key);
                } else if let Some((_, e)) = self.failed.get(&key) {
                    if !errors.iter().any(|(p, _)| *p == path) {
                        errors.push((path, e.clone()));
                    }
                } else if let Some(d) = self.last.get(&clip.clip).and_then(|h| {
                    h.iter()
                        .filter_map(|k| self.done.get(k))
                        .find(|d| d.rate == rate)
                }) {
                    clip.data = Some(d.clone());
                }
            }
        }
        self.last.retain(|c, _| seen.contains(c));
        let keep: HashSet<u64> = used
            .iter()
            .copied()
            .chain(self.last.values().flatten().copied())
            .collect();
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
        self.sweep();
        errors
    }

    /// Free the renders only this store still holds.
    pub fn sweep(&mut self) {
        self.graveyard.retain(|d| Arc::strong_count(d) > 1);
    }

    /// Take in what the worker finished; whether anything arrived.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        for (key, r) in self.service.take_finished() {
            let path = self.pending.remove(&key).unwrap_or_default();
            any = true;
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

    /// The file at `path` is to be tried again: its failed analysis and renders forgotten.
    pub fn forget(&mut self, path: &Path) {
        self.service.forget(path);
        self.failed.retain(|_, (p, _)| p != path);
    }

    /// Renders waiting or being made.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Renders held for playing (for the tests and the status line).
    pub fn held(&self) -> usize {
        self.done.len() + self.graveyard.len()
    }
}
