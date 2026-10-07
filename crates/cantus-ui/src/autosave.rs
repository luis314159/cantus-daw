//! The autosave, off the interface's thread (round 22, *soak*).
//!
//! The soak found a frame of 0.7–1.8 s every autosave interval: writing the autosave —
//! the project serialised and hashed twice, written, `fsync`ed, read back, parsed and
//! hashed again to verify it (ADR 0006) — ran inside the frame. Now the frame only takes
//! the copy (the project as a save would write it, hosted plug-ins' state as it is now:
//! that must be read on this thread) and a thread does the rest; the next frames collect
//! the outcome. One write is in flight at a time, and the close waits for it before its
//! own last-defence write (they share the file).

use crate::app::{CantusApp, APP_VERSION};
use crate::i18n::{t, tf};
use cantus_project::{SaveReport, StoreError};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

/// An autosave being written.
#[derive(Debug)]
pub struct Job {
    /// The revision it holds, and the project's file then: a result for a project that has
    /// since been replaced is not this one's.
    revision: u64,
    epoch: u64,
    path: Option<PathBuf>,
    rx: Receiver<Result<SaveReport, StoreError>>,
}

impl CantusApp {
    /// Once a frame: collect a finished autosave, and start one when it is due and there
    /// are unsaved changes.
    pub fn maybe_autosave(&mut self) {
        self.poll_autosave(false);
        if self.autosave_job.is_some() || self.last_autosave.elapsed() < self.autosave_interval()
        {
            return;
        }
        self.last_autosave = Instant::now();
        if !self.bus.is_dirty() || self.bus.revision() == self.autosaved_revision {
            return;
        }
        let project = self.autosave_copy();
        let (tx, rx) = mpsc::channel();
        let (path, dir) = (self.path.clone(), self.autosave_dir.clone());
        let p2 = path.clone();
        let spawned = std::thread::Builder::new()
            .name("cantus-autosave".into())
            .spawn(move || {
                let r = cantus_project::autosave(&project, p2.as_deref(), &dir, APP_VERSION);
                let _ = tx.send(r);
            });
        match spawned {
            Ok(_) => {
                self.autosave_job = Some(Job {
                    revision: self.bus.revision(),
                    epoch: self.epoch,
                    path,
                    rx,
                })
            }
            Err(e) => log::warn!("autosave thread: {e}"),
        }
    }

    /// Whether an autosave is being written.
    pub fn autosave_in_flight(&self) -> bool {
        self.autosave_job.is_some()
    }

    /// Collect the autosave in flight, if it finished (or wait for it, with `wait`).
    pub fn poll_autosave(&mut self, wait: bool) {
        let Some(job) = self.autosave_job.as_ref() else {
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
            Err(TryRecvError::Disconnected) => {
                self.autosave_job = None;
                return;
            }
        };
        let Some(job) = self.autosave_job.take() else {
            return;
        };
        // The project was replaced (opened, new, a demo, a recovery) while it was written.
        if job.epoch != self.epoch || job.path != self.path {
            return;
        }
        match result {
            Ok(r) => {
                self.autosaved_revision = self.autosaved_revision.max(job.revision);
                match &r.fallback {
                    // Said once: the safety net holds, but not where the user expects it.
                    Some(why) if !self.autosave_warned => {
                        self.autosave_warned = true;
                        self.set_error(tf(
                            "recovery.autosave_elsewhere",
                            &[("error", why), ("path", &r.path.display())],
                        ));
                    }
                    Some(_) => {}
                    None => {
                        self.autosave_warned = false;
                        if !self.status.error {
                            self.set_status(t("status.autosaved"));
                        }
                    }
                }
                self.journal_persisted(Some(&r.path));
            }
            Err(e) => {
                log::warn!("autosave failed: {e}");
                // Silence here would let the user believe the work is safe.
                if !self.autosave_warned {
                    self.autosave_warned = true;
                    self.set_error(tf("recovery.autosave_failed", &[("error", &e)]));
                }
            }
        }
    }
}
