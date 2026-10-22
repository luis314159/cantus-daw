//! The application's side of crash recovery (`recovery`): starting the session journal
//! and keeping it current, ending it, and restoring or discarding what a gone session
//! left. The methods are `CantusApp`'s, written here so the application file keeps only
//! the calls into them.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::record_pass::PassPlan;
use crate::recovery::{self, JournalPass, JournalTake, Session};
use crate::recovery_restore::{self, Item};
use cantus_core::{Command, Project, TrackId};
use std::path::{Path, PathBuf};

/// The `.wav` files in `dir` (a recording pass's folder), to see which ones a pass made.
pub fn wavs_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("wav"))
        })
        .collect();
    v.sort();
    v
}

/// A track's name as the recorder writes it into a file name (`recorder::unique_path`).
fn file_stem_of(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let clean = clean.trim().trim_end_matches('.').trim();
    if clean.is_empty() {
        "Take".to_string()
    } else {
        clean.to_string()
    }
}

/// Which new file is which armed track's: the recorder names each `<track> <stamp>.wav`
/// (` 2`, ` 3`… after a name already taken), in the order of the tracks.
pub fn match_takes(tracks: &[(TrackId, String)], new: &[PathBuf]) -> Vec<JournalTake> {
    let mut left: Vec<PathBuf> = new.to_vec();
    let mut out = Vec::new();
    for (id, name) in tracks {
        let stem = format!("{} ", file_stem_of(name));
        // Shortest first: `Gtr <stamp>.wav` before `Gtr <stamp> 2.wav`.
        let pick = left
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&stem))
            })
            .min_by_key(|(_, p)| (p.as_os_str().len(), (*p).clone()))
            .map(|(i, _)| i);
        if let Some(i) = pick {
            out.push(JournalTake {
                track: *id,
                path: left.remove(i),
            });
        }
    }
    out
}

impl CantusApp {
    /// Start this session's journal under `data_dir`, after reading what gone sessions
    /// left there (offered by `views::recovery`). The boot calls it; tests may.
    pub fn start_session(&mut self, data_dir: &Path) {
        let dir = recovery::sessions_dir(data_dir);
        let offer = recovery_restore::offer(recovery::orphans(&dir));
        if !offer.items.is_empty() {
            self.recovery = Some(offer);
        }
        match Session::start(&dir, crate::APP_VERSION) {
            Ok(s) => self.session = Some(s),
            Err(e) => log::warn!("no session journal: {e}"),
        }
        self.journal_sync(true);
    }

    /// Once a frame: bring the journal up to date with the project — and with the pass
    /// recording (the latency its start was placed by, its wraps; review 3). Nothing is
    /// written (or even formatted) unless something changed.
    pub fn journal_tick(&mut self) {
        let pass = self.engine.current_pass();
        let mut moved = false;
        if let (Some((latency, wraps)), Some(s)) = (pass, self.session.as_mut()) {
            if let Some(p) = s.journal.passes.iter_mut().rev().find(|p| !p.finished) {
                if p.latency_frames != latency.total() || p.marked_wraps != wraps {
                    p.latency_frames = latency.total();
                    p.marked_wraps = wraps;
                    moved = true;
                }
            }
        }
        self.journal_sync(moved);
    }

    fn journal_sync(&mut self, force: bool) {
        let dirty = self.bus.is_dirty();
        let name = &self.bus.project().name;
        let Some(s) = self.session.as_mut() else {
            return;
        };
        let j = &mut s.journal;
        let changed = j.dirty != dirty || j.project_path != self.path || &j.project_name != name;
        if changed {
            j.dirty = dirty;
            j.project_path = self.path.clone();
            j.project_name = name.clone();
        }
        if changed || force {
            if let Err(e) = s.sync() {
                log::warn!("session journal: {e}");
            }
        }
    }

    /// The recording passes finished now: their takes are clips in the project, so a copy
    /// of it taken now holds them. A save or an autosave takes this with its copy and
    /// hands it back to [`journal_persisted`](Self::journal_persisted) when it lands.
    pub(crate) fn journal_finished_passes(&self) -> Vec<JournalPass> {
        self.session
            .as_ref()
            .map(|s| {
                s.journal
                    .passes
                    .iter()
                    .filter(|p| p.finished)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The project reached a file (a save, or an autosave at `autosave`) from a copy taken
    /// when the passes `persisted` were finished: their takes are in it now. A pass
    /// finished since (the copy was written on a thread meanwhile) stays in the journal.
    pub(crate) fn journal_persisted(&mut self, autosave: Option<&Path>, persisted: &[JournalPass]) {
        if let Some(s) = self.session.as_mut() {
            let j = &mut s.journal;
            j.passes.retain(|p| !(p.finished && persisted.contains(p)));
            if let Some(a) = autosave {
                j.autosave = Some(a.to_path_buf());
                j.autosaved_at = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                );
            }
        }
        self.journal_sync(true);
    }

    /// A pass began recording audio into `dir`, where `before` were the files already.
    pub(crate) fn journal_pass_started(
        &mut self,
        dir: &Path,
        before: &[PathBuf],
        plan: &PassPlan,
        transport_start: cantus_core::Ticks,
    ) {
        if self.session.is_none() {
            return;
        }
        let new: Vec<PathBuf> = wavs_in(dir)
            .into_iter()
            .filter(|p| !before.contains(p))
            .collect();
        let tracks: Vec<(TrackId, String)> = plan
            .audio_tracks
            .iter()
            .filter_map(|&id| self.project().track(id).map(|t| (id, t.name.clone())))
            .collect();
        let pass = JournalPass {
            record_start: plan.record_start,
            transport_start,
            punch: plan.punch,
            loop_region: plan.loop_region,
            loop_takes: plan.loop_takes,
            mode: plan.mode,
            tempo_bpm: self.project().tempo_bpm,
            tempo_points: self.project().tempo_points.clone(),
            sample_rate: self.engine.sample_rate.round().max(1.0) as u32,
            // The callback's figure for now: the engine's own replaces it once the pass
            // has started (`journal_tick`, review 3).
            latency_frames: self.engine.record_latency().total(),
            marked_wraps: Vec::new(),
            takes: match_takes(&tracks, &new),
            finished: false,
        };
        if let Some(s) = self.session.as_mut() {
            s.journal.passes.push(pass);
        }
        self.journal_sync(true);
    }

    /// The pass ended: its takes are clips in the project, not yet in a file.
    pub(crate) fn journal_pass_finished(&mut self) {
        if let Some(p) = self
            .session
            .as_mut()
            .and_then(|s| s.journal.passes.iter_mut().rev().find(|p| !p.finished))
        {
            p.finished = true;
        }
        self.journal_sync(true);
    }

    /// The window is closing: a pass in progress is finished (its files closed, its clips
    /// placed), unsaved changes are autosaved — the last line of defence — and the journal
    /// ends, or stays to offer those changes at the next start.
    pub fn close_session(&mut self) {
        if self.is_recording() {
            self.stop_recording();
        }
        // A freeze or a bounce rendering: stopped, its partial files gone (review 1).
        crate::freeze::close(self);
        // A save being written lands first (then nothing may be unsaved at all), and an
        // autosave being written shares the file with this one: let both finish. What the
        // *Save changes?* question waited to do (open another file…) is moot now.
        self.saving.then = None;
        self.wait_for_save();
        // Its renders copied into its folder before the process ends (review 4: a quit
        // right after a save killed the copy halfway).
        crate::freeze::finish_saving(self, std::time::Duration::from_secs(120));
        self.poll_autosave(true);
        let mut unsaved = false;
        if self.bus.is_dirty() {
            let persisted = self.journal_finished_passes();
            let project = self.autosave_copy();
            match cantus_project::autosave(
                &project,
                self.path.as_deref(),
                &self.autosave_dir,
                crate::APP_VERSION,
            ) {
                Ok(r) => {
                    self.journal_persisted(Some(&r.path), &persisted);
                    unsaved = true;
                }
                Err(e) => log::warn!("autosave on close failed: {e}"),
            }
        }
        self.end_session(unsaved);
    }

    /// End the journal: removed, or kept (marked closed) when `unsaved` changes are in
    /// its autosave.
    pub fn end_session(&mut self, unsaved: bool) {
        self.journal_sync(true);
        if let Some(s) = self.session.take() {
            s.end(unsaved);
        }
    }

    /// Put the offered item `i` in the place of the open project: its last save, then
    /// its autosave over it as one undo step, then the takes recorded since, as one more.
    pub fn restore_recovery(&mut self, i: usize) -> bool {
        let Some(item) = self
            .recovery
            .as_mut()
            .and_then(|o| (i < o.items.len()).then(|| o.items.remove(i)))
        else {
            return false;
        };
        // A save in flight writes the project being replaced: it lands first.
        self.wait_for_save();
        if self.is_recording() {
            self.stop_recording();
        }
        self.engine.stop();
        let base = item
            .project_path
            .as_ref()
            .and_then(|p| cantus_project::load(p, false).ok())
            .map(|l| l.project);
        let had_base = base.is_some();
        match (&item.autosave, base) {
            (Some((_, auto)), base) => {
                let base = base.unwrap_or_else(|| Project::new(&auto.name));
                self.bus.reset(base);
                self.exec(Command::ReplaceProject {
                    project: Box::new(auto.clone()),
                });
            }
            (None, Some(base)) => self.bus.reset(base),
            (None, None) => {}
        }
        self.epoch += 1;
        if item.autosave.is_some() || had_base {
            self.path = item.project_path.clone();
        }
        self.sync_audio_base();
        self.selected_track = None;
        self.selected_clip = None;
        self.selected_notes.clear();
        self.select_first();
        let takes = self.restore_takes(&item);
        item.orphan.forget();
        self.seek(cantus_core::Ticks::ZERO);
        self.set_status(tf(
            "recovery.restored",
            &[("name", &item.name), ("n", &takes)],
        ));
        self.recovery = self.recovery.take().filter(|o| !o.items.is_empty());
        true
    }

    /// Lay out each recovered pass as a finished pass is laid out; a take whose track is
    /// gone comes in on a new track. Returns how many takes came back.
    fn restore_takes(&mut self, item: &Item) -> usize {
        let mut n = 0;
        for pass in &item.passes {
            let (here, gone): (Vec<_>, Vec<_>) = pass
                .takes
                .iter()
                .cloned()
                .partition(|t| self.project().track(t.track).is_some());
            if !here.is_empty() {
                let j = &pass.journal;
                let mut plan = PassPlan::new(j.record_start);
                plan.punch = j.punch;
                plan.loop_region = j.loop_region;
                plan.loop_takes = j.loop_takes;
                plan.mode = j.mode;
                plan.take_word = t("rec.take");
                plan.audio_tracks = here.iter().map(|t| t.track).collect();
                // Where the playhead was when the pass ended: after the last wrap, from
                // the loop's start; else from where the transport started.
                let longest = here.iter().max_by_key(|t| t.frames);
                plan.stop_at = match (longest, j.loop_region) {
                    (Some(t), Some((a, _))) if !t.wraps.is_empty() => {
                        let last = t.wraps.last().copied().unwrap_or(0);
                        cantus_core::Ticks(
                            j.tick_after(a.0 as f64, (t.frames - last) as f64, j.sample_rate)
                                .round() as i64,
                        )
                    }
                    (Some(t), _) => cantus_core::Ticks(
                        j.tick_after(j.transport_start.0 as f64, t.frames as f64, j.sample_rate)
                            .round() as i64,
                    ),
                    (None, _) => j.record_start,
                };
                if self.finish_pass_with_latency(plan, here.clone(), j.latency_frames) {
                    n += here.len();
                }
            }
            if !gone.is_empty() {
                let paths: Vec<PathBuf> = gone.iter().map(|t| t.path.clone()).collect();
                n += self.import_audio_as(&paths, pass.journal.record_start, None, true);
            }
        }
        n
    }

    /// Forget what was offered: the journals go; the autosaves and recordings stay on
    /// disk where they are.
    pub fn discard_recovery(&mut self) {
        if let Some(o) = self.recovery.take() {
            for i in o.items {
                i.orphan.forget();
            }
        }
    }
}
