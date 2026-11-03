//! The session's sync: once per interface frame (and whenever the project changes).

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use cantus_ara::document::{musical_of, Document};
use cantus_core::ara::AraPlugin;
use cantus_core::audio::AudioSource;
use cantus_core::model::{AudioSourceId, ClipId, Project};
use cantus_core::{Command, SampleData};

use super::{binding_of, new_bound, open_doc, Agreed, AraSession, Live, SourceState};
use crate::ara::{AraStatus, GESTURE, RENDER_BLOCK};
use crate::ara_worker::{Job, Worker};
use crate::backend::{AudioSetup, PluginInfo};

impl AraSession {
    /// Keep the plug-ins' documents in step with `project`, bind instances for new regions,
    /// put an undone state back into the plug-in, collect the plug-in's edits and requests,
    /// queue renders (at `engine_rate`) and take in the ones made. `find` locates a plug-in
    /// (the database); `audio` gives a source at its own rate (the sample cache's, decoded
    /// off this thread). Returns whether the renders changed (compile a snapshot).
    pub fn sync(
        &mut self,
        project: &Project,
        find: &dyn Fn(&AraPlugin) -> Option<PluginInfo>,
        audio: &dyn Fn(&AudioSource) -> SourceState,
        engine_rate: u32,
    ) -> bool {
        let before = self.revision;
        self.take_renders();
        self.watchdog();
        let wanted = if self.enabled {
            crate::ara::bound_plugins(project)
        } else {
            Default::default()
        };
        self.close_unwanted(&wanted);
        for (k, plugin) in &wanted {
            if self.docs.contains_key(k) || self.failed.contains_key(k) {
                continue;
            }
            self.mark(k, "opening");
            let opened = open_doc(plugin, find, &project.name);
            self.unmark(k);
            match opened {
                Ok(d) => {
                    self.docs.insert(k.clone(), d);
                }
                Err(e) => {
                    log::warn!("ARA plug-in {}: {e:?}", plugin.name);
                    self.failed.insert(k.clone(), e);
                }
            }
        }
        let end = project
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| c.start + c.length)
            .max()
            .map_or(1.0, |t| project.tempo_map().seconds_at(t.0 as f64));
        let musical = musical_of(project, end);
        let mut loading: Vec<AudioSourceId> = Vec::new();
        let mut ready: HashMap<AudioSourceId, Arc<SampleData>> = HashMap::new();
        for s in &project.audio_pool {
            match audio(s) {
                SourceState::Ready(a) => {
                    ready.insert(s.id, a);
                }
                SourceState::Loading => loading.push(s.id),
                SourceState::Missing => {}
            }
        }
        let source_audio = |id: AudioSourceId| ready.get(&id).cloned();
        let mut live: Live = Live::new();
        let mut refused: Vec<(ClipId, String)> = Vec::new();
        let now = Instant::now();
        let keys: Vec<String> = self.docs.keys().cloned().collect();
        for k in keys {
            self.sync_doc(&k, project, &musical, &source_audio, &mut refused, now);
            if let Some(pd) = self.docs.get(&k) {
                live.extend(pd.instances.keys().copied());
            }
        }
        self.agreed.retain(|c, _| live.contains(c));
        self.submit_due(project, engine_rate, now);
        self.statuses(project, &live, &refused, &loading);
        let before_len = self.renders.len();
        self.renders.retain(|c, _| live.contains(c));
        if self.renders.len() != before_len {
            self.revision += 1;
        }
        self.revision != before
    }

    /// Documents of plug-ins no clip names any more go, each clip's state kept for undo.
    fn close_unwanted(&mut self, wanted: &std::collections::BTreeMap<String, AraPlugin>) {
        let closing: Vec<String> = self
            .docs
            .keys()
            .filter(|k| !wanted.contains_key(*k))
            .cloned()
            .collect();
        for k in closing {
            let Some(mut pd) = self.docs.remove(&k) else {
                continue;
            };
            let live: Vec<ClipId> = pd.instances.keys().copied().collect();
            pd.instances.clear();
            let plugin = binding_plugin(&pd);
            if let Some(doc) = pd.doc.as_ref() {
                for c in doc.modifications().collect::<Vec<_>>() {
                    if let (Some(a), Some(b)) = (doc.store(c), doc.binding(c)) {
                        // A clip let go of its plug-in can be bound again with its edits.
                        if let (true, Some(p)) = (live.contains(&c), plugin.clone()) {
                            let mut clip = cantus_core::ara::AraClip::new(p);
                            clip.binding = b;
                            clip.archive = Some(a.clone());
                            self.previous.insert(c, clip);
                        }
                        self.kept.insert(c, (b, a));
                    }
                }
            }
        }
        if self.enabled {
            // A plug-in no clip names is tried afresh when one does again.
            self.failed
                .retain(|k, s| wanted.contains_key(k) || matches!(s, AraStatus::Stopped(_)));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sync_doc(
        &mut self,
        k: &str,
        project: &Project,
        musical: &cantus_ara::callbacks::Musical,
        source_audio: &dyn Fn(AudioSourceId) -> Option<Arc<SampleData>>,
        refused: &mut Vec<(ClipId, String)>,
        now: Instant,
    ) {
        let next = Document::of_plugin(project, k);
        let debounce = self.debounce;
        // Clips about to leave: their binding and state kept, to bind again with its edits.
        if let Some(pd) = self.docs.get(k) {
            if let Some(doc) = pd.doc.as_ref() {
                for c in pd.instances.keys() {
                    if next.regions.contains_key(c) {
                        continue;
                    }
                    if let (Some(a), Some(b), Some(p)) =
                        (doc.store(*c), doc.binding(*c), binding_plugin(pd))
                    {
                        let mut clip = cantus_core::ara::AraClip::new(p);
                        clip.binding = b;
                        clip.archive = Some(a);
                        self.previous.insert(*c, clip);
                    }
                }
            }
        }
        self.mark(k, "editing");
        let kept = &self.kept;
        let archive = |c: ClipId| {
            let b = binding_of(project, c).map(|(_, a)| a.binding);
            kept.get(&c)
                .filter(|(kb, _)| Some(*kb) == b)
                .map(|(_, a)| a.clone())
                .or_else(|| binding_of(project, c).and_then(|(_, a)| a.archive.clone()))
        };
        let Some(pd) = self.docs.get_mut(k) else {
            return;
        };
        let Some(doc) = pd.doc.as_mut() else { return };
        let instances = &mut pd.instances;
        let ch = doc.update(&next, musical, source_audio, &archive, &mut |c| {
            instances.remove(&c);
        });
        refused.extend(ch.refused.iter().cloned());
        for c in &ch.removed {
            if self.renders.remove(c).is_some() {
                self.revision += 1;
            }
        }
        for c in &ch.created {
            let Some((region, frames)) = doc.region(*c) else {
                continue;
            };
            let setup = AudioSetup {
                sample_rate: f64::from(frames.rate),
                max_block: RENDER_BLOCK,
            };
            // SAFETY: `doc`'s controller and region outlive the instance (dropped first).
            match unsafe { crate::vst3::ara::load_bound(&pd.info, setup, doc.dc_ref(), &[region]) }
            {
                Ok((loaded, ext)) => {
                    let mut b = new_bound(loaded, ext);
                    b.due = Some(now);
                    pd.instances.insert(*c, b);
                }
                Err(e) => refused.push((*c, format!("the plug-in would not load: {e}"))),
            }
        }
        for c in &ch.changed {
            if let Some(b) = pd.instances.get_mut(c) {
                // A region that moves renders once it rests; the last render plays meanwhile.
                b.due = Some(now + debounce);
                b.range = Some(None);
            }
        }
        // The project's state and the plug-in's, each clip: an undo or redo of the plug-in's
        // edit goes back into it; a first sight records what both agree on.
        for c in pd.instances.keys().copied().collect::<Vec<_>>() {
            let project_state = binding_of(project, c).and_then(|(_, a)| a.archive.clone());
            let project_bytes = project_state.as_ref().map(|a| a.data.clone());
            if let std::collections::hash_map::Entry::Vacant(e) = self.baseline.entry(c) {
                if let Some(a) = doc.store(c) {
                    e.insert(a);
                }
            }
            match self.agreed.get(&c) {
                Some(a) if a.project == project_bytes => {}
                Some(_) => {
                    let target = project_state
                        .clone()
                        .or_else(|| self.baseline.get(&c).cloned());
                    if let Some(t) = target {
                        if let Err(e) = doc.reload(c, &t) {
                            refused.push((c, e));
                        }
                    }
                    if let Some(live_now) = doc.store(c) {
                        self.agreed.insert(
                            c,
                            Agreed {
                                project: project_bytes,
                                live: live_now.data,
                            },
                        );
                    }
                    if let Some(b) = pd.instances.get_mut(&c) {
                        b.due = Some(now);
                        b.range = Some(None);
                    }
                }
                None => {
                    if let Some(live_now) = doc.store(c) {
                        self.agreed.insert(
                            c,
                            Agreed {
                                project: project_bytes,
                                live: live_now.data,
                            },
                        );
                    }
                }
            }
        }
        // What the plug-in said: renders of what changed, its edits as commands, its wishes
        // for the transport.
        let said = doc.poll();
        self.requests.extend(doc.take_playback_requests());
        let mut look: Vec<ClipId> = said.clips.clone();
        if said.document {
            look.extend(pd.instances.keys().copied());
            look.sort();
            look.dedup();
        }
        for c in look {
            if let (Some(b), Some(r)) = (pd.instances.get_mut(&c), said.ranges.get(&c)) {
                b.due = Some(now);
                b.range = Some(match (b.range.take(), *r) {
                    (None, r) => r,
                    (Some(Some(a)), Some(n)) => Some((a.0.min(n.0), a.1.max(n.1))),
                    _ => None,
                });
            }
            let (Some(live_now), Some((track, _))) = (doc.store(c), binding_of(project, c)) else {
                continue;
            };
            if self.agreed.get(&c).is_some_and(|a| a.live == live_now.data) {
                continue;
            }
            let gesture = match self.gestures.get(&c) {
                Some((g, at)) if now.duration_since(*at) < GESTURE => *g,
                _ => {
                    self.next_gesture += 1;
                    self.next_gesture
                }
            };
            self.gestures.insert(c, (gesture, now));
            self.edits.push((
                Command::SetClipAraState {
                    track,
                    clip: c,
                    archive: Some(live_now.clone()),
                },
                (c, gesture),
            ));
            self.agreed.insert(
                c,
                Agreed {
                    project: Some(live_now.data.clone()),
                    live: live_now.data,
                },
            );
        }
        // Editor windows the user closed; the instance's own requests serviced.
        for b in pd.instances.values_mut() {
            if b.window.as_ref().is_some_and(|w| w.closed()) {
                b.window = None;
                b.main.close_editor();
            }
            let _ = b.main.idle();
        }
        self.unmark(k);
    }

    /// Queue the renders that are due, each its clip's newest (an older one is cancelled).
    fn submit_due(&mut self, project: &Project, engine_rate: u32, now: Instant) {
        let tempo = Arc::new(project.tempo_map());
        self.worker.get_or_insert_with(Worker::new);
        let mut marks: Vec<String> = Vec::new();
        let mut pending_jobs: Vec<Job> = Vec::new();
        for (k, pd) in &mut self.docs {
            let Some(doc) = pd.doc.as_ref() else { continue };
            for (c, b) in pd.instances.iter_mut() {
                if b.due.is_none_or(|d| d > now) {
                    continue;
                }
                b.due = None;
                let mut range = b.range.take().flatten();
                // A render on its way that this one cancels: this one covers what it did
                // (a part never replaces a whole render in flight).
                if b.rendering {
                    range = match (b.inflight.flatten(), range) {
                        (Some(x), Some(y)) if b.inflight.is_some() => {
                            Some((x.0.min(y.0), x.1.max(y.1)))
                        }
                        _ => None,
                    };
                }
                b.inflight = Some(range);
                let (Some((_, frames)), Some(src)) = (doc.region(*c), doc.source_audio(*c)) else {
                    continue;
                };
                let generation = b.generation.fetch_add(1, Ordering::SeqCst) + 1;
                b.progress.store(0, Ordering::Relaxed);
                b.rendering = true;
                if !marks.contains(k) {
                    marks.push(k.clone());
                }
                pending_jobs.push(Job {
                    clip: *c,
                    generation,
                    current: b.generation.clone(),
                    audio: b.audio.clone(),
                    source: src,
                    frames,
                    latency: b.main.latency(),
                    range,
                    base: self.renders.get(c).cloned(),
                    tempo: tempo.clone(),
                    engine_rate,
                    progress: b.progress.clone(),
                });
            }
        }
        // The marker names the plug-ins first: a render may take Cantus down at once.
        for k in marks {
            self.mark(&k, "rendering");
        }
        if let Some(w) = self.worker.as_ref() {
            for j in pending_jobs {
                w.submit(j);
            }
        }
    }

    /// Take in the renders the render thread made: the current one of each clip replaces
    /// what it played; one a newer edit overtook is dropped.
    pub(super) fn take_renders(&mut self) {
        let Some(w) = self.worker.as_ref() else {
            return;
        };
        let done = w.take();
        let idle = w.pending() == 0;
        for d in done {
            let Some(b) = self
                .docs
                .values_mut()
                .find_map(|pd| pd.instances.get_mut(&d.clip))
            else {
                continue;
            };
            if b.generation.load(Ordering::SeqCst) != d.generation {
                continue;
            }
            b.rendering = false;
            b.inflight = None;
            if let Some(r) = d.render {
                self.renders.insert(d.clip, r);
                self.renders_made += 1;
                self.revision += 1;
                self.last_render = Some(d.took);
            }
        }
        if idle {
            let rendering: Vec<String> = self
                .in_flight
                .iter()
                .filter(|(_, w)| **w == "rendering")
                .map(|(k, _)| k.clone())
                .collect();
            for k in rendering {
                self.unmark(&k);
            }
        }
    }

    /// A render that runs past the limit: its plug-in is stopped for the session, its
    /// thread and instances left to themselves, a new render thread started.
    fn watchdog(&mut self) {
        let Some((clip, ran)) = self.worker.as_ref().and_then(Worker::running) else {
            return;
        };
        if ran < self.hang_limit {
            return;
        }
        let Some(k) = self
            .docs
            .iter()
            .find(|(_, pd)| pd.instances.contains_key(&clip))
            .map(|(k, _)| k.clone())
        else {
            return;
        };
        log::warn!(
            "ARA plug-in {k} stopped answering while it rendered clip {}",
            clip.0
        );
        if let Some(pd) = self.docs.remove(&k) {
            self.abandoned.push(pd);
        }
        if let Some(w) = self.worker.take() {
            w.abandon();
        }
        self.failed.insert(
            k.clone(),
            AraStatus::Stopped("it stopped answering while it rendered".into()),
        );
        self.unmark(&k);
        self.revision += 1;
    }

    fn statuses(
        &mut self,
        project: &Project,
        live: &Live,
        refused: &[(ClipId, String)],
        loading: &[AudioSourceId],
    ) {
        let mut status = HashMap::new();
        for t in &project.tracks {
            for c in &t.clips {
                let Some(a) = c.audio.as_ref().filter(|a| a.ara.is_some()) else {
                    continue;
                };
                let k = a
                    .ara
                    .as_ref()
                    .map(|r| r.plugin.id.to_ascii_uppercase())
                    .unwrap_or_default();
                let s = if !self.enabled {
                    AraStatus::Off
                } else if live.contains(&c.id) {
                    AraStatus::Live
                } else if let Some(e) = self.failed.get(&k) {
                    e.clone()
                } else if let Some((_, e)) = refused.iter().find(|(x, _)| *x == c.id) {
                    AraStatus::Failed(e.clone())
                } else if c.lane != 0 {
                    AraStatus::OnTakeLane
                } else if loading.contains(&a.source) {
                    AraStatus::Loading
                } else {
                    AraStatus::NoSource
                };
                status.insert(c.id, s);
            }
        }
        self.status = status;
    }
}

fn binding_plugin(pd: &super::PluginDoc) -> Option<AraPlugin> {
    let f = pd.doc.as_ref()?.factory();
    Some(crate::ara::plugin_ref(&pd.info, &f.factory_id()))
}
