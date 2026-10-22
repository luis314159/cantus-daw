//! *Save* off the interface's thread (round 24, *beta-rest*), the twin of the autosave's
//! (`autosave`, round 22).
//!
//! What a save needs from this thread is taken here, in the frame: the audio collected into
//! the project's folder (its commands change the project), the hosted plug-ins' state as it
//! is now (a plug-in answers only on its main thread) — both inside
//! [`project_for_save`](crate::plugins), which hands back the project as an owned copy.
//! Before that, every hosted plug-in is asked for its state as it is now: an isolated one
//! answers later, and the save waits for the answers frame by frame — never on this thread
//! — until each is in or has passed its own limit; one kept as it was last said is named
//! when the save lands (round 29, review isolation-5 rule 2).
//! Everything else — the JSON, its hash, the atomic write beside the file, the `.bak`, the
//! `fsync`, reading it back and verifying it (ADR 0006) — runs on a thread; the next frames
//! collect the outcome. Measured (`save_async.rs`, ignored): the copy is a small fraction
//! of the serialisation and write, so the copy is what is taken.
//!
//! Rules:
//!
//! - One save at a time. A save asked for while one runs is **queued** (the last asked
//!   wins) and starts when the first ends — for the project it was asked for: when the
//!   project is replaced meanwhile (an import, the undo of one), the queued save and the
//!   errand waiting on it are dropped, never written with the project that came in.
//! - *Save* while a save is in flight goes where that save goes (an untitled project is
//!   not asked for a name twice).
//! - The project is marked saved **at the revision that was written**: an edit made while
//!   the file was being written keeps the project dirty.
//! - A failed save says why, in the interface's language, and leaves the project dirty and
//!   its file where it was.
//! - New, Open, a demo, a DAWproject import, a recovery and the close wait for a save in
//!   flight (the undo of a replaced project does not: the epoch rule above holds it); the
//!   *Save changes?* question's **Save** carries out what it was asked for only after the
//!   save succeeded — and asks again if the project was edited while it was written.
//! - The crash journal lets go only of the recording passes whose takes are in the file
//!   written: those finished when the copy was taken, not those finished since.

use crate::app::{CantusApp, APP_VERSION};
use crate::i18n::tf;
use crate::recovery::JournalPass;
use crate::views::unsaved::Then;
use cantus_project::{SaveReport, StoreError};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

/// A save being written.
#[derive(Debug)]
pub struct Job {
    /// The revision it holds: what becomes "saved" when it lands.
    revision: u64,
    /// The project's epoch then (a result for a project since replaced is not this one's).
    epoch: u64,
    path: PathBuf,
    started: Instant,
    /// The recording passes finished when the copy was taken: their takes are in it.
    persisted: Vec<JournalPass>,
    rx: Receiver<Result<SaveReport, StoreError>>,
    /// Plug-ins saved as they were last said (not to be had as they are now), and why.
    stale: Vec<(String, String)>,
}

/// A save waiting for its plug-ins' states (asked when it began; looked at each frame).
#[derive(Debug)]
pub struct Gathering {
    path: PathBuf,
    epoch: u64,
    started: Instant,
}

/// A save asked for while one ran, and the project's epoch it was asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    pub path: PathBuf,
    pub epoch: u64,
}

/// The save state the application keeps.
#[derive(Debug, Default)]
pub struct Saving {
    pub(crate) job: Option<Job>,
    /// A save waiting for its plug-ins' states.
    pub(crate) gathering: Option<Gathering>,
    /// A save asked for while one ran.
    pub(crate) queued: Option<Queued>,
    /// What the *Save changes?* question waits to do once the save lands.
    pub(crate) then: Option<Then>,
    /// How long the last save took on this thread (the copy) and in all: what the
    /// measurement in `save_async.rs` (ignored) prints.
    pub last_frame_ms: f64,
    pub last_total_ms: f64,
    /// Tests only: the next save's thread waits for a message (or the sender's drop)
    /// before it writes, so a test can act while a save is in flight.
    #[doc(hidden)]
    pub hold: Option<Receiver<()>>,
}

impl Saving {
    /// Where a save asked for now, for the project of `epoch`, would go without asking:
    /// the queued save's file, else the one in flight's.
    fn target(&self, epoch: u64) -> Option<PathBuf> {
        self.queued
            .as_ref()
            .filter(|q| q.epoch == epoch)
            .map(|q| q.path.clone())
            .or_else(|| {
                self.job
                    .as_ref()
                    .filter(|j| j.epoch == epoch)
                    .map(|j| j.path.clone())
            })
            .or_else(|| {
                self.gathering
                    .as_ref()
                    .filter(|g| g.epoch == epoch)
                    .map(|g| g.path.clone())
            })
    }
}

impl CantusApp {
    /// Where *Save* writes without asking: the file a save in flight (or queued) for this
    /// project goes to, else the project's file; `None` when *Save As* must ask.
    pub fn save_target(&self) -> Option<PathBuf> {
        self.saving.target(self.epoch).or_else(|| self.path.clone())
    }

    /// Start saving the project to `path` on a thread and return at once; `false` when a
    /// save is already running (this one is queued behind it). The status line says
    /// "Saving…" and then how it went.
    pub fn start_save(&mut self, path: &Path) -> bool {
        if self.saving.job.is_some() || self.saving.gathering.is_some() {
            self.saving.queued = Some(Queued {
                path: path.to_path_buf(),
                epoch: self.epoch,
            });
            self.set_status(tf("save.queued", &[("file", &file_name(path))]));
            return false;
        }
        // Hosted plug-ins are asked for their state as it is now; the file is written once
        // they have answered (`continue_save`, each frame). A waiting card closed before
        // shows again: the save may wait on that very dialog (review isolation-6 #27).
        self.plugins.ask_states();
        self.plugin_crashes.left_waiting.clear();
        self.saving.gathering = Some(Gathering {
            path: path.to_path_buf(),
            epoch: self.epoch,
            started: Instant::now(),
        });
        self.set_status(tf("save.saving", &[("file", &file_name(path))]));
        self.continue_save(false);
        true
    }

    /// A save whose plug-ins' states are in — or, `now` (New, Open, the close, a test
    /// waiting), as each was last said, which the save then names: written on a thread.
    fn continue_save(&mut self, now: bool) {
        let Some(g) = self.saving.gathering.as_ref() else {
            return;
        };
        if g.epoch != self.epoch {
            // Another project came in meanwhile: not this one's to write.
            self.saving.gathering = None;
            return;
        }
        if !now && !self.plugins.states_in() {
            return;
        }
        if let Some(g) = self.saving.gathering.take() {
            self.write_save(&g.path, g.started);
        }
    }

    /// Writes the project to `path` on a thread; the outcome is collected by `poll_save`.
    fn write_save(&mut self, path: &Path, t0: Instant) {
        let frame = Instant::now();
        // The audio comes home first, so the file is written with its new paths.
        self.collect_audio(path);
        // The hosted plug-ins' states asked for when the save began.
        let project = self.plugins.project_for_save(self.bus.project());
        let stale = self.plugins.saved_stale().to_vec();
        let revision = self.bus.revision();
        let persisted = self.journal_finished_passes();
        let (tx, rx) = mpsc::channel();
        let hold = self.saving.hold.take();
        let target = path.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("cantus-save".into())
            .spawn(move || {
                if let Some(h) = hold {
                    let _ = h.recv();
                }
                let _ = tx.send(cantus_project::save(&project, &target, APP_VERSION));
            });
        if let Err(e) = spawned {
            // No thread to be had: write here rather than not at all.
            log::warn!("save thread: {e}");
            let project = self.plugins.project_for_save(self.bus.project());
            let (tx, rx2) = mpsc::channel();
            let _ = tx.send(cantus_project::save(&project, path, APP_VERSION));
            self.saving.job = Some(Job {
                revision,
                epoch: self.epoch,
                path: path.to_path_buf(),
                started: t0,
                persisted,
                rx: rx2,
                stale,
            });
            self.poll_save(true);
            return;
        }
        self.saving.last_frame_ms = frame.elapsed().as_secs_f64() * 1e3;
        self.saving.job = Some(Job {
            revision,
            epoch: self.epoch,
            path: path.to_path_buf(),
            started: t0,
            persisted,
            rx,
            stale,
        });
    }

    /// Whether a save is being written (or waits for its plug-ins' states).
    pub fn save_in_flight(&self) -> bool {
        self.saving.job.is_some() || self.saving.gathering.is_some()
    }

    /// Wait until no save is in flight or queued (New, Open, the close; tests). A save
    /// still waiting for plug-ins' states takes what each said last, and names them: the
    /// interface never waits on a plug-in (round 29, review isolation-5).
    pub fn wait_for_save(&mut self) {
        while self.save_in_flight() {
            self.poll_save(true);
        }
    }

    /// Collect the save in flight, if it finished (or wait for it, with `wait`); then
    /// start the queued one, or carry out what the *Save changes?* question waited for.
    pub fn poll_save(&mut self, wait: bool) {
        self.continue_save(wait);
        let Some(job) = self.saving.job.as_ref() else {
            return;
        };
        let result = if wait {
            job.rx.recv().map_err(|_| TryRecvError::Disconnected)
        } else {
            job.rx.try_recv()
        };
        let result = match result {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(StoreError::Io(std::io::Error::other(
                "the save's thread ended without an answer",
            ))),
        };
        let Some(job) = self.saving.job.take() else {
            return;
        };
        self.saving.last_total_ms = job.started.elapsed().as_secs_f64() * 1e3;
        let ok = result.is_ok();
        // New, Open and the rest wait for a save; the undo of a replaced project does
        // not: a result for another project is only logged.
        let current = job.epoch == self.epoch;
        match result {
            Ok(r) if current => {
                self.path = Some(job.path.clone());
                self.sync_audio_base();
                self.bus.mark_saved_at(job.revision);
                // Its renders beside it, and only those it may want (round 29, review 2).
                crate::freeze::saved(self);
                self.autosaved_revision = self.autosaved_revision.max(job.revision);
                self.set_status(tf("status.saved", &[("path", &r.path.display())]));
                // Never silently: a plug-in kept as it was last said is named.
                if let Some(said) = crate::views::plugin_notice::saved_stale(&job.stale) {
                    self.set_error(said);
                }
                self.journal_persisted(None, &job.persisted);
            }
            Ok(r) => log::info!("saved {} (a project since replaced)", r.path.display()),
            // The session is untouched and still unsaved; the message says why and where.
            Err(e) => self.set_error(crate::recovery_text::save_failed(&job.path, &e)),
        }
        if !current {
            // What was asked for the project before is not this one's to do.
            self.saving.then = None;
        }
        if let Some(next) = self.saving.queued.take() {
            // A save asked for a project since replaced is dropped, never written with
            // this one. The same file again after a failure would only fail again.
            // What the question waits for waits for this one too.
            if next.epoch != self.epoch {
                log::info!(
                    "a queued save of {} dropped: the project was replaced",
                    next.path.display()
                );
            } else if ok || next.path != job.path {
                self.start_save(&next.path);
                return;
            }
        }
        if let Some(then) = self.saving.then.take() {
            if ok {
                if self.bus.is_dirty() {
                    // Edited while it was written: what is unsaved now is asked about.
                    self.unsaved_prompt = Some(then);
                } else {
                    self.carry_out(then);
                }
            }
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
