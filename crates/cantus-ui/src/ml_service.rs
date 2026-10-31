//! The application's one machine-learning service (round 34, ADR 0085): the one model store
//! (`<data dir>/Models`, one `accepted.json`) and the one helper client (one `cantus-ml`
//! process, started on the first job), shared by every feature that runs a model — stems,
//! audio to MIDI, the agents' tools. Made by [`wire`] from `CantusApp::assemble` (the
//! boot's path) and turned by [`MlService::poll`] each frame.
//!
//! **Jobs.** [`MlService::start`] gives an [`MlJob`]: its id and a channel of its updates,
//! forwarded by the service's router thread as the helper says them — whether or not the
//! window draws (an agent's job may run while Cantus is minimised), the window woken for
//! its progress bars. [`MlService::job`] keeps each job's state for the views;
//! [`MlService::cancel`] stops one. The client's own updates are read by the router only —
//! never call `poll` or `wait` on [`MlService::client`], or a job loses its updates.

use cantus_content::models::{ModelEvent, ModelManifest, Models};
use cantus_ml::client::{ClientConfig, JobId, MlClient, Outcome, Update};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// What a job has said so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JobState {
    /// The helper began it (it may wait in the queue until then).
    pub started: bool,
    /// 0…1 across the whole job.
    pub done: f32,
    pub stage: String,
    /// Its end, once it ended.
    pub outcome: Option<Outcome>,
}

struct Inner {
    cfg: ClientConfig,
    client: Option<MlClient>,
    jobs: HashMap<JobId, JobState>,
    to: HashMap<JobId, Sender<Update>>,
    /// Ended jobs, oldest first: only the last [`KEPT_ENDED`] keep their state, so a starter
    /// that never forgets cannot grow the table for good. // r34:transcribe
    ended: std::collections::VecDeque<JobId>,
}

/// Ended jobs whose state is kept for the views. // r34:transcribe
pub const KEPT_ENDED: usize = 32;

impl Inner {
    fn client(&mut self) -> &mut MlClient {
        let cfg = self.cfg.clone();
        self.client.get_or_insert_with(|| MlClient::new(cfg))
    }
}

struct Shared {
    inner: Mutex<Inner>,
    /// The window to wake when a job moves (progress bars).
    repaint: Mutex<Option<egui::Context>>,
    stop: AtomicBool,
}

fn lock(s: &Shared) -> MutexGuard<'_, Inner> {
    s.inner.lock().unwrap_or_else(|p| p.into_inner())
}

/// The helper client, borrowed (see the module's note: never `poll` or `wait` on it).
pub struct ClientGuard<'a>(MutexGuard<'a, Inner>);

impl std::ops::Deref for ClientGuard<'_> {
    type Target = MlClient;
    fn deref(&self) -> &MlClient {
        self.0.client.as_ref().expect("made by client()")
    }
}

impl std::ops::DerefMut for ClientGuard<'_> {
    fn deref_mut(&mut self) -> &mut MlClient {
        self.0.client()
    }
}

/// A job started: its id, and its updates — every one, in order, delivered by the
/// service's router thread whether or not the window draws; the sender is dropped after
/// `Update::Ended`. A worker thread may block on `updates.recv_timeout`.
pub struct MlJob {
    pub id: JobId,
    pub updates: Receiver<Update>,
}

/// Jobs from another thread: an agent's job decodes and converts its audio on its worker
/// and only then starts the helper's job. (The same as transcribe's `MlStarter`.)
#[derive(Clone)]
pub struct MlStarter(Arc<Shared>);

impl MlStarter {
    pub fn start(&self, job: &str, args: serde_json::Value) -> MlJob {
        let mut g = lock(&self.0);
        let id = g.client().start(job, args);
        let (tx, updates) = channel();
        g.jobs.insert(id, JobState::default());
        g.to.insert(id, tx);
        MlJob { id, updates }
    }

    pub fn cancel(&self, id: JobId) {
        lock(&self.0).client().cancel(id);
    }

    /// Forget an ended job's state. // r34:transcribe
    pub fn forget(&self, id: JobId) {
        let mut g = lock(&self.0);
        g.jobs.remove(&id);
        g.ended.retain(|j| *j != id);
    }

    /// How many jobs' states are kept (tests). // r34:transcribe
    pub fn kept(&self) -> usize {
        lock(&self.0).jobs.len()
    }
}

pub struct MlService {
    models: Models,
    shared: Arc<Shared>,
    router: Option<std::thread::JoinHandle<()>>,
    wired: bool,
    events: Vec<ModelEvent>,
    /// Agents' model jobs under way, each held from its tool's prepare until the job ends
    /// (its edit made or not): the helper and the models are in use meanwhile, between
    /// the job's helper runs too (integration review: one counter for every agent tool).
    ml_agents: Arc<std::sync::atomic::AtomicUsize>,
}

/// An agent's model job counted as under way while this lives ([`MlService::hold_ml_agent`]):
/// made in the tool's prepare, moved into its work and from there into its edit.
pub struct MlAgentHold(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for MlAgentHold {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Default for MlService {
    /// Not wired: a store over a folder that is never made and a client never started
    /// (`CantusApp::from_parts`, tests that do not run models).
    fn default() -> Self {
        let root = std::env::temp_dir().join("cantus-ml-unwired");
        MlService::new(
            Models::new(root, ModelManifest::bundled()),
            ClientConfig::new(ClientConfig::default_exe()),
            false,
        )
    }
}

impl MlService {
    /// A service over `models` with a client configured by `cfg` (the helper starts on the
    /// first job). `wired` says it is the application's own.
    pub fn new(models: Models, cfg: ClientConfig, wired: bool) -> MlService {
        let shared = Arc::new(Shared {
            inner: Mutex::new(Inner {
                cfg,
                client: None,
                jobs: HashMap::new(),
                ended: Default::default(), // r34:transcribe
                to: HashMap::new(),
            }),
            repaint: Mutex::new(None),
            stop: AtomicBool::new(false),
        });
        let router = wired.then(|| {
            let s = shared.clone();
            std::thread::Builder::new()
                .name("cantus-ml-router".into())
                .spawn(move || route(&s))
                .expect("the router thread starts")
        });
        MlService {
            models,
            shared,
            router,
            wired,
            events: Vec::new(),
            ml_agents: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Count an agent's model job as under way until the hold is dropped.
    pub fn hold_ml_agent(&self) -> MlAgentHold {
        self.ml_agents
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        MlAgentHold(self.ml_agents.clone())
    }

    /// How many agents' model jobs are under way.
    pub fn ml_agents(&self) -> usize {
        self.ml_agents.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Whether `assemble` made this service (the boot path's test asks).
    pub fn is_wired(&self) -> bool {
        self.wired
    }

    pub fn models(&self) -> &Models {
        &self.models
    }

    pub fn models_mut(&mut self) -> &mut Models {
        &mut self.models
    }

    /// The helper client (made on first use). Never `poll` or `wait` on it.
    pub fn client(&self) -> ClientGuard<'_> {
        let mut g = lock(&self.shared);
        g.client();
        ClientGuard(g)
    }

    /// Queue a job (`cantus_ml::spec` has the arguments); the helper starts if need be.
    /// Its updates come on [`MlJob::updates`]; [`MlService::job`] keeps its state too.
    pub fn start(&self, job: &str, args: serde_json::Value) -> MlJob {
        let mut g = lock(&self.shared);
        let id = g.client().start(job, args);
        let (tx, updates) = channel();
        g.jobs.insert(id, JobState::default());
        g.to.insert(id, tx);
        MlJob { id, updates }
    }

    pub fn cancel(&self, id: JobId) {
        lock(&self.shared).client().cancel(id);
    }

    /// What job `id` has said so far.
    pub fn job(&self, id: JobId) -> Option<JobState> {
        lock(&self.shared).jobs.get(&id).cloned()
    }

    /// How many jobs were started and not forgotten (tests).
    pub fn job_count(&self) -> usize {
        lock(&self.shared).jobs.len()
    }

    /// Whether a job is under way (started, not ended): the helper is in use.
    pub fn busy(&self) -> bool {
        lock(&self.shared)
            .jobs
            .values()
            .any(|j| j.outcome.is_none())
    }

    /// Forget an ended job's state.
    pub fn forget(&self, id: JobId) {
        self.starter().forget(id); // r34:transcribe — the ended list too
    }

    /// How many jobs' states are kept (tests). // r34:transcribe
    pub fn kept(&self) -> usize {
        self.starter().kept()
    }

    /// End the helper as soon as no job runs: its memory and its files go back (a model can
    /// then be removed, on Windows too).
    pub fn release_helper(&self) {
        let g = lock(&self.shared);
        if let Some(c) = g.client.as_ref() {
            c.release();
        }
    }

    /// A starter another thread can start and cancel jobs with (an agent's job).
    pub fn starter(&self) -> MlStarter {
        MlStarter(self.shared.clone())
    }

    /// Each frame: the model store's downloads and preparations. Their ends are kept for
    /// [`MlService::take_model_events`].
    pub fn poll(&mut self) {
        self.events.extend(self.models.poll());
    }

    /// The model events since the last call (installs finished or failed).
    pub fn take_model_events(&mut self) -> Vec<ModelEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Drop for MlService {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(r) = self.router.take() {
            let _ = r.join();
        }
    }
}

/// The router: the client's updates into the job table, as they come.
fn route(s: &Shared) {
    while !s.stop.load(Ordering::Relaxed) {
        let mut any = false;
        {
            let mut g = lock(s);
            let updates = match g.client.as_mut() {
                Some(c) => c.poll(),
                None => Vec::new(),
            };
            for u in updates {
                any = true;
                let id = match &u {
                    Update::Started(id) | Update::Progress { id, .. } | Update::Ended(id, _) => {
                        Some(*id)
                    }
                    Update::Log { .. } => None,
                };
                if let Some(id) = id {
                    let ended = matches!(u, Update::Ended(..));
                    if let Some(tx) = g.to.get(&id) {
                        let _ = tx.send(u.clone());
                    }
                    if ended {
                        g.to.remove(&id);
                    }
                }
                // A job its starter forgot is not made again by its late updates (the
                // table would grow for good). // r34:transcribe
                match u {
                    Update::Started(id) => {
                        if let Some(j) = g.jobs.get_mut(&id) {
                            j.started = true;
                        }
                    }
                    Update::Progress { id, done, stage } => {
                        if let Some(j) = g.jobs.get_mut(&id) {
                            j.done = done;
                            j.stage = stage;
                        }
                    }
                    Update::Ended(id, o) => {
                        if let Some(j) = g.jobs.get_mut(&id) {
                            j.outcome = Some(o);
                            g.ended.push_back(id);
                            while g.ended.len() > KEPT_ENDED {
                                if let Some(old) = g.ended.pop_front() {
                                    g.jobs.remove(&old);
                                }
                            }
                        }
                    }
                    Update::Log { level, message } => match level.as_str() {
                        "warn" | "error" => log::warn!("cantus-ml: {message}"),
                        _ => log::info!("cantus-ml: {message}"),
                    },
                }
            }
        }
        if any {
            if let Some(ctx) = s.repaint.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
                ctx.request_repaint();
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Make the application's service (`CantusApp::assemble`): the store under
/// `<data dir>/Models`, the helper beside the program.
pub fn wire(app: &mut crate::app::CantusApp, ctx: &egui::Context) {
    let root = crate::app::data_dir().join("Models");
    app.ml = MlService::new(
        Models::new(root, ModelManifest::bundled()),
        ClientConfig::new(ClientConfig::default_exe()),
        true,
    );
    *app.ml
        .shared
        .repaint
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(ctx.clone());
}
