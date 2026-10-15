//! The sample cache's side of the pitch renders: which render each clip plays, what is on
//! its way, and letting go of renders nothing plays any more — never on the audio thread:
//! a render no snapshot asks for waits here until this is its last holder (the audio
//! thread only ever decrements a count), then it is freed.

use super::{render_key, PitchService};
use cantus_core::{ClipId, SampleData, Snapshot};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Default)]
pub(crate) struct Renders {
    pub service: Arc<PitchService>,
    done: HashMap<u64, Arc<SampleData>>,
    pending: HashSet<u64>,
    failed: HashMap<u64, String>,
    /// The render each clip played last: what it keeps playing while its next is made.
    last: HashMap<ClipId, u64>,
    graveyard: Vec<Arc<SampleData>>,
}

impl Renders {
    /// Attach every edited clip's render, made here (`background` off) or queued (on).
    /// `resolve` turns a stored path into the file's. A clip whose render is not made yet
    /// keeps what it was given (its source) — or the render it played before.
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
                let Some(edit) = clip.pitch.clone().filter(|e| e.audible()) else {
                    continue;
                };
                // A source that is not there plays nothing, edited or not.
                if clip.data.is_none() {
                    continue;
                }
                seen.insert(clip.clip);
                let path = resolve(&clip.path);
                let key = render_key(&path, &edit, rate);
                used.insert(key);
                if !self.done.contains_key(&key)
                    && !self.failed.contains_key(&key)
                    && !self.pending.contains(&key)
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
                        self.pending.insert(key);
                    } else {
                        match self.service.render_now(&path, &edit, rate) {
                            Ok(d) => {
                                self.done.insert(key, Arc::new(d));
                            }
                            Err(e) => {
                                self.failed.insert(key, e);
                            }
                        }
                    }
                }
                if let Some(d) = self.done.get(&key) {
                    clip.data = Some(d.clone());
                    self.last.insert(clip.clip, key);
                } else if let Some(e) = self.failed.get(&key) {
                    if !errors.iter().any(|(p, _)| *p == path) {
                        errors.push((path, e.clone()));
                    }
                } else if let Some(d) = self.last.get(&clip.clip).and_then(|k| self.done.get(k)) {
                    clip.data = Some(d.clone());
                    used.insert(*self.last.get(&clip.clip).expect("just read"));
                }
            }
        }
        self.last.retain(|c, _| seen.contains(c));
        let keep: HashSet<u64> = used.iter().chain(self.last.values()).copied().collect();
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
            self.pending.remove(&key);
            any = true;
            match r {
                Ok(d) => {
                    self.done.insert(key, Arc::new(d));
                }
                Err(e) => {
                    self.failed.insert(key, e);
                }
            }
        }
        self.sweep();
        any
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
