//! The content manager the interface talks to: what is installed, what each pack is
//! doing, and installs and removals that run on worker threads. Nothing here blocks the
//! caller except [`Manager::remove`] (a rename) and the initial scan of receipts.

use crate::fetch::{Net, Transport};
use crate::install::{self, InstallError, Job, Layout, Patience, Phase, Receipt};
use crate::manifest::{Manifest, Pack};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// What a pack is, from the interface's point of view.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    NotInstalled,
    Installed {
        version: u32,
    },
    /// Installed, and the manifest has a newer version.
    Update {
        have: u32,
        new: u32,
    },
    /// Installing; the last phase reported, if any yet.
    Busy(Option<Phase>),
}

/// An install that ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Finished {
    pub id: String,
    pub result: Result<Receipt, InstallError>,
}

enum Msg {
    Phase(String, Phase),
    Done(String, Result<Receipt, InstallError>),
}

struct Running {
    cancel: Arc<AtomicBool>,
    phase: Option<Phase>,
}

pub struct Manager {
    layout: Layout,
    manifest: Manifest,
    transport: Arc<dyn Transport>,
    patience: Patience,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    running: HashMap<String, Running>,
    installed: Vec<Receipt>,
    /// The last failure of each pack, until it is tried again.
    errors: HashMap<String, InstallError>,
}

impl Manager {
    /// A manager over `root` (`<data dir>/Content`) with the real network.
    pub fn new(root: impl Into<PathBuf>, manifest: Manifest) -> Manager {
        Self::with_transport(root, manifest, Arc::new(Net::new()), Patience::default())
    }

    pub fn with_transport(
        root: impl Into<PathBuf>,
        manifest: Manifest,
        transport: Arc<dyn Transport>,
        patience: Patience,
    ) -> Manager {
        let layout = Layout::new(root);
        let (tx, rx) = channel();
        let installed = install::installed(&layout);
        Manager {
            layout,
            manifest,
            transport,
            patience,
            tx,
            rx,
            running: HashMap::new(),
            installed,
            errors: HashMap::new(),
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn root(&self) -> &Path {
        &self.layout.root
    }

    pub fn installed(&self) -> &[Receipt] {
        &self.installed
    }

    pub fn pack_dir(&self, id: &str) -> PathBuf {
        self.layout.pack_dir(id)
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.iter().any(|r| r.id == id)
    }

    pub fn status(&self, id: &str) -> Status {
        if let Some(r) = self.running.get(id) {
            return Status::Busy(r.phase.clone());
        }
        let have = self
            .installed
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.version);
        let new = self.manifest.pack(id).map(|p| p.version);
        match (have, new) {
            (None, _) => Status::NotInstalled,
            (Some(have), Some(new)) if new > have => Status::Update { have, new },
            (Some(version), _) => Status::Installed { version },
        }
    }

    /// The last failure of a pack that is not running.
    pub fn error(&self, id: &str) -> Option<&InstallError> {
        self.errors.get(id)
    }

    pub fn busy(&self) -> bool {
        !self.running.is_empty()
    }

    /// Start installing (or updating) a pack on a worker thread. False when it is
    /// unknown or already running.
    pub fn install(&mut self, id: &str) -> bool {
        self.start(id, None)
    }

    /// Install a zip pack from an archive the user already has (offline): the same
    /// checks as a download, from `path`.
    pub fn install_from_file(&mut self, id: &str, path: &Path) -> bool {
        self.start(id, Some(crate::fetch::file_url(path)))
    }

    fn start(&mut self, id: &str, archive: Option<String>) -> bool {
        let Some(pack) = self.manifest.pack(id).cloned() else {
            return false;
        };
        if self.running.contains_key(id) {
            return false;
        }
        self.errors.remove(id);
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.insert(
            id.to_string(),
            Running {
                cancel: cancel.clone(),
                phase: None,
            },
        );
        let (layout, transport, patience, tx) = (
            self.layout.clone(),
            self.transport.clone(),
            self.patience,
            self.tx.clone(),
        );
        let spawned = std::thread::Builder::new()
            .name(format!("cantus-content-{id}"))
            .spawn(move || run(pack, layout, transport, patience, archive, cancel, tx));
        if spawned.is_err() {
            self.running.remove(id);
            return false;
        }
        true
    }

    /// Ask a running install to stop; it reports `Cancelled` and keeps what it has
    /// downloaded for next time.
    pub fn cancel(&mut self, id: &str) {
        if let Some(r) = self.running.get(id) {
            r.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Delete an installed pack. Refused while it is installing.
    pub fn remove(&mut self, id: &str) -> std::io::Result<()> {
        if self.running.contains_key(id) {
            return Err(std::io::Error::other("the pack is being installed"));
        }
        install::remove(&self.layout, id)?;
        self.installed.retain(|r| r.id != id);
        Ok(())
    }

    /// Take what the workers reported. Call once a frame.
    pub fn poll(&mut self) -> Vec<Finished> {
        let mut done = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Phase(id, phase) => {
                    if let Some(r) = self.running.get_mut(&id) {
                        r.phase = Some(phase);
                    }
                }
                Msg::Done(id, result) => {
                    self.running.remove(&id);
                    match &result {
                        Ok(_) => self.installed = install::installed(&self.layout),
                        Err(e) => {
                            self.errors.insert(id.clone(), e.clone());
                        }
                    }
                    done.push(Finished { id, result });
                }
            }
        }
        done
    }

    /// Block until every install has finished (tests, and the app on exit).
    pub fn wait_idle(&mut self, timeout: std::time::Duration) -> Vec<Finished> {
        let until = std::time::Instant::now() + timeout;
        let mut all = Vec::new();
        while self.busy() && std::time::Instant::now() < until {
            all.extend(self.poll());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        all.extend(self.poll());
        all
    }

    /// The pack the browser offers when nothing is installed, unless it is.
    pub fn starter(&self) -> Option<&Pack> {
        self.manifest
            .recommended()
            .filter(|p| !self.is_installed(&p.id))
    }
}

fn run(
    pack: Pack,
    layout: Layout,
    transport: Arc<dyn Transport>,
    patience: Patience,
    archive: Option<String>,
    cancel: Arc<AtomicBool>,
    tx: Sender<Msg>,
) {
    let job = Job {
        layout: &layout,
        transport: transport.as_ref(),
        cancel: &cancel,
        patience,
        archive,
    };
    // Report at most every 1/20 s, so a fast local copy does not flood the channel.
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut report = |phase: Phase| {
        let urgent = !matches!(phase, Phase::Downloading { .. } | Phase::Unpacking { .. });
        if urgent || last.elapsed() >= std::time::Duration::from_millis(50) {
            last = std::time::Instant::now();
            let _ = tx.send(Msg::Phase(pack.id.clone(), phase));
        }
    };
    let result = install::install(&job, &pack, &mut report);
    if result.is_err() {
        // Partial downloads stay (they resume); a half-unpacked folder does not.
        if !matches!(result, Err(InstallError::Fetch(_))) {
            install::clear_staging(&layout, &pack.id);
        }
    }
    let _ = tx.send(Msg::Done(pack.id.clone(), result));
}
