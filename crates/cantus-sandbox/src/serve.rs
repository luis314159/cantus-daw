//! The helper's main thread (ADR 0063): Cantus's calls, the plug-ins' windows and requests,
//! and the heartbeat.
//!
//! **Life.** The main thread bumps a progress count each time round its loop, and each
//! time the platform's pulse runs — which it does inside any message loop the thread is in,
//! a plug-in's modal dialog or a dragged window included. A heartbeat thread of its own
//! says *alive* to Cantus while that count moves: a main thread inside a modal loop lives,
//! one stuck in code that never pumps falls silent (review #0, #48). From a nested loop the
//! pulse also serves Cantus's calls, but those that end or replace an instance (and every
//! one after them, in order), which wait for the main loop.
//!
//! **Blame.** Before calling into an instance, the thread writes its number in the lobby
//! ([`cantus_host::bridge::sandbox::MAIN_AT`]), 0 after: a crash there is laid on it.
//!
//! Every call that names an instance is answered with the instance's news after it — the
//! requests it made (`MainThread::idle`) and all its values — so Cantus's mirror is current
//! the moment the call returns (a restart the plug-in asked for while its state loaded is
//! seen at once, as in process). Between calls, every [`TICK`] each instance is served as
//! the host's frame loop serves it in process: its requests, a flush it asked for, its
//! window's close box and resizes; what changed goes to Cantus as news.

use crate::platform::{Pipes, Wake};
use crate::worker::{self, Half, Rt, Table};
use cantus_host::bridge::proto::{
    self, After, Answer, Call, Described, Failure, Message, Request, WireEvent, WireNotice,
};
use cantus_host::bridge::slot::Slot;
use cantus_host::window::PluginWindow;
use cantus_host::{AudioSetup, HostError, MainThread, Notice, ParamChange, ParamOut};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::io::{BufReader, BufWriter, Write};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the instances are served between calls.
const TICK: Duration = Duration::from_millis(15);
/// How often the heartbeat goes out (while the main thread makes progress).
const ALIVE: Duration = Duration::from_millis(200);

/// The main thread's progress: bumped by its loop and by the pulse.
static PROGRESS: AtomicU64 = AtomicU64::new(0);

/// The protocol's output, shared by the main thread and the heartbeat's.
type Out = Arc<Mutex<BufWriter<std::fs::File>>>;

fn out_lock(out: &Out) -> MutexGuard<'_, BufWriter<std::fs::File>> {
    out.lock().unwrap_or_else(|e| e.into_inner())
}

struct Instance {
    main: Box<dyn MainThread>,
    half: Half,
    /// Its entry in the processing threads' table.
    rt: Arc<Rt>,
    window: Option<PluginWindow>,
    /// The values Cantus was last told.
    told: HashMap<u32, f64>,
    ids: Vec<u32>,
}

impl Instance {
    /// Ends it, on the main thread: its window, its audio half (once no job runs it), its
    /// main half. It left the threads' table before.
    fn finish(mut self) {
        if let Some(w) = self.window.take() {
            if let Some(h) = w.handle() {
                crate::platform::blame::forget_window(h);
            }
            self.main.close_editor();
            #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
            drop(w);
        }
        let half = self.half.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(half);
        drop(self.main);
        cantus_host::clap::collect_garbage();
    }

    /// Its requests (window ones handled here) and the values that changed.
    fn news(&mut self) -> (Vec<WireNotice>, Vec<(u32, f64)>) {
        let mut notices = Vec::new();
        for n in self.main.idle() {
            match &n {
                Notice::EditorResized(size) => {
                    if let Some(w) = self.window.as_mut() {
                        w.fit(*size);
                    }
                }
                Notice::EditorClosed => self.close_window(),
                _ => {}
            }
            if let Some(w) = proto::WireNotice::of(&n) {
                notices.push(w);
            }
        }
        if self.window.as_ref().is_some_and(|w| w.closed()) {
            self.close_window();
            notices.push(WireNotice::EditorClosed);
        }
        if let Some(w) = self.window.as_mut() {
            if let Some(size) = w.take_resize() {
                if let Some(taken) = self.main.resize_editor(size) {
                    w.fit(taken);
                }
            }
        }
        if notices.contains(&WireNotice::ParamsChanged) {
            self.ids = self.main.params().iter().map(|p| p.id).collect();
        }
        let mut values = Vec::new();
        for id in &self.ids {
            if let Some(v) = self.main.param_value(*id) {
                if self.told.get(id).map(|x| x.to_bits()) != Some(v.to_bits()) {
                    self.told.insert(*id, v);
                    values.push((*id, v));
                }
            }
        }
        (notices, values)
    }

    /// Everything Cantus mirrors, after a call.
    fn after(&mut self) -> After {
        let (notices, _) = self.news();
        let values: Vec<(u32, f64)> = self
            .ids
            .iter()
            .filter_map(|id| self.main.param_value(*id).map(|v| (*id, v)))
            .collect();
        for (id, v) in &values {
            self.told.insert(*id, *v);
        }
        After {
            notices,
            values,
            latency: self.main.latency(),
        }
    }

    fn close_window(&mut self) {
        if let Some(w) = self.window.take() {
            if let Some(h) = w.handle() {
                crate::platform::blame::forget_window(h);
            }
            self.main.close_editor();
            #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
            drop(w);
        }
    }

    fn described(&self) -> Described {
        Described {
            info: self.main.info().clone(),
            params: self.main.params(),
            latency: self.main.latency(),
            has_editor: self.main.has_editor(),
        }
    }
}

fn failure(e: HostError) -> Failure {
    match e {
        HostError::Load(s) => Failure::Load(s),
        HostError::NotFound(s) => Failure::NotFound(s),
        HostError::Instantiate(s) => Failure::Instantiate(s),
        HostError::Activate(s) => Failure::Activate(s),
        HostError::Unsupported(s) => Failure::Unsupported(s.to_string()),
        HostError::State(s) => Failure::State(s),
        HostError::Editor(s) => Failure::Editor(s),
        HostError::Crashed(s) | HostError::Busy(s) => Failure::Load(s),
        HostError::Ended { why, .. } => Failure::Load(why),
    }
}

struct Server {
    out: Out,
    /// Each instance in a cell of its own: a call into one leaves the others free, so a
    /// plug-in's dialog inside a call (a licence nag in a load, a progress window in a
    /// state) never stops the others' calls being served from its loop (review
    /// isolation-4 #13, #43, #54).
    instances: RefCell<HashMap<u32, Rc<RefCell<Instance>>>>,
    /// Instances unloaded while a frame further out still held them: ended by the main
    /// loop once nothing does.
    leaving: RefCell<Vec<(u32, Rc<RefCell<Instance>>)>>,
    /// What the processing threads serve, their lobby's bell, their stop.
    table: Arc<Table>,
    bell: Arc<cantus_rt::ipc::LobbyBell>,
    stop: Arc<AtomicBool>,
    threads: RefCell<Vec<JoinHandle<()>>>,
    /// The most processing threads: as many as the plug-ins Cantus may call at once, no
    /// more than the cores beside Cantus's own (ADR 0031 R15) — or what Cantus asked for
    /// (`--threads`, tests).
    max_threads: usize,
    /// The lobby's count of finished jobs.
    jobs_done: &'static AtomicU32,
}

impl Server {
    fn send(&self, msg: &Message, blob: &[u8]) -> bool {
        proto::write_frame(&mut *out_lock(&self.out), msg, blob).is_ok()
    }

    fn instance(&self, n: u32) -> Option<Rc<RefCell<Instance>>> {
        self.instances.borrow().get(&n).cloned()
    }

    /// Whether a call into `n` is under way further out on this thread (its cell is held).
    fn busy(&self, n: u32) -> bool {
        self.instance(n)
            .is_some_and(|i| i.try_borrow_mut().is_err())
    }

    fn load(
        &self,
        instance: u32,
        info: &cantus_host::PluginInfo,
        sample_rate: f64,
        max_block: usize,
        slot_name: &str,
    ) -> Result<Answer, Failure> {
        let before = crate::platform::blame::modules_now();
        let result = self.load_inner(instance, info, sample_rate, max_block, slot_name);
        // A plug-in may have put its own crash filter in place of ours, even one that
        // failed to load: ours goes back first (review isolation-4 #61).
        crate::platform::blame::reinstall();
        if result.is_ok() {
            crate::platform::blame::learn_modules(instance, &info.path, &before);
        }
        result
    }

    fn load_inner(
        &self,
        instance: u32,
        info: &cantus_host::PluginInfo,
        sample_rate: f64,
        max_block: usize,
        slot_name: &str,
    ) -> Result<Answer, Failure> {
        let backend = cantus_host::backend_for(info.format)
            .ok_or_else(|| Failure::Unsupported("format".into()))?;
        let slot = Arc::new(
            Slot::open(slot_name, max_block)
                .map_err(|e| Failure::Load(format!("shared memory: {e}")))?,
        );
        // SAFETY: the exchange lives in `slot`, which the instance's `Rt` keeps alive.
        let completer = unsafe { cantus_rt::ipc::Completer::open(slot_name, slot.exchange()) }
            .map_err(|e| Failure::Load(format!("shared memory: {e}")))?;
        let loaded = backend
            .load(
                info,
                AudioSetup {
                    sample_rate,
                    max_block,
                },
            )
            .map_err(failure)?;
        let half: Half = Arc::new(Mutex::new(Some(loaded.audio)));
        let rt = Arc::new(Rt {
            instance,
            slot,
            completer,
            half: half.clone(),
        });
        self.table.change(|t| t.push(rt.clone()));
        let count = self.instances.borrow().len() + 1;
        self.grow(count)?;
        let main = loaded.main;
        let ids = main.params().iter().map(|p| p.id).collect();
        let inst = Instance {
            main,
            half,
            rt,
            window: None,
            told: HashMap::new(),
            ids,
        };
        let described = inst.described();
        self.instances
            .borrow_mut()
            .insert(instance, Rc::new(RefCell::new(inst)));
        Ok(Answer::Loaded(described))
    }

    /// Starts processing threads until there are as many as `instances` (at most
    /// `max_threads`); returns once each is waiting, so a first block finds it ready.
    fn grow(&self, instances: usize) -> Result<(), Failure> {
        let want = instances.min(self.max_threads).max(1);
        let mut threads = self.threads.borrow_mut();
        while threads.len() < want {
            let (table, bell, stop) = (self.table.clone(), self.bell.clone(), self.stop.clone());
            let done = self.jobs_done;
            let k = threads.len();
            let t = std::thread::Builder::new()
                .name(format!("cantus-sandbox-audio-{k}"))
                .spawn(move || worker::serve(table, bell, stop, done))
                .map_err(|e| Failure::Load(e.to_string()))?;
            threads.push(t);
        }
        Ok(())
    }

    /// An instance leaves the threads' table, then ends on this thread — at once, or, when
    /// a frame further out still holds it, as soon as none does.
    fn unload(&self, instance: u32) {
        let Some(i) = self.instances.borrow_mut().remove(&instance) else {
            return;
        };
        let rt = i.borrow().rt.clone();
        self.table.change(|t| t.retain(|x| !Arc::ptr_eq(x, &rt)));
        match Rc::try_unwrap(i) {
            Ok(cell) => end(instance, cell.into_inner()),
            Err(held) => self.leaving.borrow_mut().push((instance, held)),
        }
    }

    /// The instances unloaded while held, once nothing holds them.
    fn end_leaving(&self) {
        let ready: Vec<(u32, Rc<RefCell<Instance>>)> = {
            let mut leaving = self.leaving.borrow_mut();
            let (ready, held) = leaving
                .drain(..)
                .partition(|(_, i)| Rc::strong_count(i) == 1);
            *leaving = held;
            ready
        };
        for (n, i) in ready {
            if let Ok(cell) = Rc::try_unwrap(i) {
                end(n, cell.into_inner());
            }
        }
    }

    /// One call; `None` for Quit.
    fn handle(&self, req: Request, blob: Vec<u8>) -> Option<()> {
        let id = req.id;
        // Its time limit runs from now (review #37).
        self.send(&Message::Started { id }, &[]);
        let _at = instance_of(&req.call).map(crate::platform::blame::calling);
        let mut reply_blob = Vec::new();
        let mut named = None;
        let result = match req.call {
            Call::Hello { .. } => Ok(Answer::Hello {
                protocol: proto::PROTOCOL,
                pid: std::process::id(),
            }),
            Call::Quit => {
                let _ = self.send(
                    &Message::Reply {
                        id,
                        result: Ok(Answer::Done),
                        after: None,
                    },
                    &[],
                );
                return None;
            }
            Call::Surface => {
                crate::platform::surface();
                Ok(Answer::Done)
            }
            Call::Load {
                instance,
                info,
                sample_rate,
                max_block,
                slot,
            } => {
                named = Some(instance);
                let r = self.load(instance, &info, sample_rate, max_block, &slot);
                // Its first good state travels with the answer: a crash a moment later
                // still leaves Cantus something to keep.
                if let Some(i) = self.instance(instance) {
                    if let Ok(state) = i.borrow_mut().main.save_state() {
                        reply_blob = state;
                    }
                }
                r
            }
            Call::Unload { instance } => {
                self.unload(instance);
                Ok(Answer::Done)
            }
            call => {
                let instance = instance_of(&call).unwrap_or(0);
                named = Some(instance);
                let result = match self.instance(instance) {
                    None => Err(Failure::NoInstance),
                    Some(inst) => match inst.try_borrow_mut() {
                        Ok(mut i) => on_instance(&mut i, call, &blob, &mut reply_blob),
                        // Held further out (never served so: `must_wait`).
                        Err(_) => Err(Failure::Load("the plug-in is busy".into())),
                    },
                };
                // A plug-in may have put its own crash filter in place of ours while its
                // editor opened or it restarted (review isolation-4 #61).
                crate::platform::blame::reinstall();
                result
            }
        };
        let after = named.and_then(|n| {
            let inst = self.instance(n)?;
            let mut i = inst.try_borrow_mut().ok()?;
            Some(Box::new((n, i.after())))
        });
        self.send(&Message::Reply { id, result, after }, &reply_blob);
        Some(())
    }

    /// Between calls: each instance's requests, flushed events and changed values — but
    /// one a frame further out is calling into, and one unloaded meanwhile.
    fn tick(&self) {
        let list: Vec<(u32, Rc<RefCell<Instance>>)> = self
            .instances
            .borrow()
            .iter()
            .map(|(n, i)| (*n, i.clone()))
            .collect();
        for (n, inst) in list {
            if !self.instances.borrow().contains_key(&n) {
                continue;
            }
            let Ok(mut i) = inst.try_borrow_mut() else {
                continue;
            };
            let _at = crate::platform::blame::calling(n);
            let mut out = ParamOut::new();
            i.main.flush(&[], &mut out);
            let flushed: Vec<WireEvent> = out.events().iter().map(WireEvent::of).collect();
            let (notices, values) = i.news();
            drop(i);
            if !notices.is_empty() || !values.is_empty() || !flushed.is_empty() {
                self.send(
                    &Message::News {
                        instance: n,
                        notices,
                        values,
                        flushed,
                    },
                    &[],
                );
            }
        }
        cantus_host::clap::collect_garbage();
        // Snapshots the processing threads no longer hold are freed here, not there.
        self.table.collect();
    }

    fn flush(&self) {
        let _ = out_lock(&self.out).flush();
    }
}

/// Ends instance `n` on this thread, as a call into it — a crash in its destructor is its
/// own (review isolation-4 #2, #45, #56) — and only then forgets its libraries.
fn end(n: u32, inst: Instance) {
    {
        let _at = crate::platform::blame::calling(n);
        inst.finish();
    }
    crate::platform::blame::forget_instance(n);
}

/// The instance a call names (Load and Unload included).
fn instance_of(call: &Call) -> Option<u32> {
    match call {
        Call::Load { instance, .. }
        | Call::Unload { instance }
        | Call::DropAudio { instance }
        | Call::Restart { instance }
        | Call::SaveState { instance }
        | Call::LoadState { instance }
        | Call::Flush { instance, .. }
        | Call::ParamText { instance, .. }
        | Call::Params { instance }
        | Call::OpenEditor { instance, .. }
        | Call::CloseEditor { instance }
        | Call::FocusEditor { instance }
        | Call::ResizeEditor { instance, .. } => Some(*instance),
        Call::Hello { .. } | Call::Surface | Call::Quit => None,
    }
}

/// Calls that end or replace an instance (or the process): never served from a loop
/// nested inside a plug-in's own code, when that may be the instance's.
fn waits_for_main_loop(call: &Call) -> bool {
    matches!(
        call,
        Call::Unload { .. }
            | Call::DropAudio { .. }
            | Call::Restart { .. }
            | Call::CloseEditor { .. }
            | Call::Quit
    )
}

/// How a call that arrives in a loop nested in a plug-in's code is served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hold {
    /// Now, from the loop.
    Serve,
    /// Held for the main loop, on purpose, and Cantus told (`Message::Deferred`): it ends
    /// or replaces the instance whose code runs the loop, or the process.
    Held,
    /// Queued behind its instance's call in progress further out (a plug-in's dialog, or a
    /// long state that pumps), and Cantus told (`Message::Behind`): it waits for it as for
    /// any call ahead of it — from when it starts — unless that call runs past its own
    /// limit (review isolation-4 #1, #54).
    Behind,
}

/// How a call is served from a loop nested in `inside`'s code (0: not known whose): Quit
/// is held; a call that ends or replaces the instance whose code runs the loop (any, when
/// not known) is held; a call on an instance a frame further out is calling into (`busy`)
/// waits behind it; a later call on an instance that already has one waiting waits as
/// that one does, in its order. Calls on the other instances are served (review #4, #17,
/// #44; isolation-4 #54).
fn must_wait(
    call: &Call,
    inside: u32,
    waiting: &VecDeque<(Request, Vec<u8>, Hold)>,
    busy: impl Fn(u32) -> bool,
) -> Hold {
    if matches!(call, Call::Quit) {
        return Hold::Held;
    }
    let named = instance_of(call);
    if waits_for_main_loop(call) && (inside == 0 || named == Some(inside)) {
        return Hold::Held;
    }
    if let Some(n) = named {
        if let Some((_, _, hold)) = waiting
            .iter()
            .rev()
            .find(|(r, _, _)| instance_of(&r.call) == Some(n))
        {
            return *hold;
        }
        if busy(n) {
            return Hold::Behind;
        }
    }
    Hold::Serve
}

/// The main thread's state: the server, the requests, those kept for the main loop. Used
/// through a shared reference, so a loop nested in a call (the pulse) serves too.
struct Here {
    server: Server,
    rx: mpsc::Receiver<Option<(Request, Vec<u8>)>>,
    deferred: RefCell<VecDeque<(Request, Vec<u8>, Hold)>>,
    quit: Cell<bool>,
}

impl Here {
    /// Serves what waits. `nested`: from a message loop inside a plug-in's code — calls
    /// that end or replace an instance, calls on an instance in use further out, and every
    /// one after them, wait for the main loop.
    fn serve(&self, nested: bool) {
        loop {
            if self.quit.get() {
                return;
            }
            let held = if nested {
                // A call queued behind its instance's call that has ended since.
                self.free_behind()
            } else {
                self.deferred
                    .borrow_mut()
                    .pop_front()
                    .map(|(req, blob, _)| (req, blob))
            };
            if let Some((req, blob)) = held {
                if self.server.handle(req, blob).is_none() {
                    self.quit.set(true);
                    return;
                }
                continue;
            }
            match self.rx.try_recv() {
                Ok(Some((req, blob))) => {
                    let hold = if nested {
                        let inside = crate::platform::blame::current();
                        let deferred = self.deferred.borrow();
                        must_wait(&req.call, inside, &deferred, |n| self.server.busy(n))
                    } else {
                        Hold::Serve
                    };
                    if hold != Hold::Serve {
                        let id = req.id;
                        self.server.send(
                            &if hold == Hold::Held {
                                // Held on purpose: not late while this process lives.
                                Message::Deferred { id }
                            } else {
                                Message::Behind { id }
                            },
                            &[],
                        );
                        self.deferred.borrow_mut().push_back((req, blob, hold));
                        continue;
                    }
                    if self.server.handle(req, blob).is_none() {
                        self.quit.set(true);
                        return;
                    }
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                    // Cantus is gone.
                    self.quit.set(true);
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => return,
            }
        }
    }
}

impl Here {
    /// The first call queued behind an instance that is no longer busy, and first in its
    /// instance's order: it may be served from a nested loop.
    fn free_behind(&self) -> Option<(Request, Vec<u8>)> {
        let mut d = self.deferred.borrow_mut();
        let mut seen: Vec<u32> = Vec::new();
        for i in 0..d.len() {
            let Some(n) = instance_of(&d[i].0.call) else {
                continue;
            };
            if seen.contains(&n) {
                continue;
            }
            seen.push(n);
            if d[i].2 == Hold::Behind && !self.server.busy(n) {
                return d.remove(i).map(|(r, b, _)| (r, b));
            }
        }
        None
    }
}

thread_local! {
    static HERE: RefCell<Option<Rc<Here>>> = const { RefCell::new(None) };
}

fn here() -> Option<Rc<Here>> {
    HERE.try_with(|h| h.borrow().clone()).ok().flatten()
}

/// The pulse (`platform::start_pulse`): progress, and — from any message loop the main
/// thread is in inside a plug-in's code, one of the server's own calls included — the
/// calls that may be served there.
fn pulse() {
    PROGRESS.fetch_add(1, Ordering::Relaxed);
    if let Some(here) = here() {
        if !here.quit.get() {
            here.serve(true);
            here.server.flush();
        }
    }
}

/// Says *alive* every [`ALIVE`] while the main thread makes progress.
fn heartbeat(out: Out) {
    let mut seen = PROGRESS.load(Ordering::Relaxed).wrapping_sub(1);
    loop {
        let now = PROGRESS.load(Ordering::Relaxed);
        if now != seen {
            seen = now;
            if proto::write_frame(&mut *out_lock(&out), &Message::Alive, &[]).is_err() {
                return;
            }
        }
        std::thread::sleep(ALIVE);
    }
}

/// A call on one instance.
fn on_instance(
    inst: &mut Instance,
    call: Call,
    blob: &[u8],
    reply_blob: &mut Vec<u8>,
) -> Result<Answer, Failure> {
    match call {
        Call::DropAudio { .. } => {
            let half = inst.half.lock().unwrap_or_else(|e| e.into_inner()).take();
            drop(half);
            Ok(Answer::Done)
        }
        Call::Restart { .. } => match inst.main.restart() {
            Ok(None) => Ok(Answer::Restarted(None)),
            Ok(Some(audio)) => {
                *inst.half.lock().unwrap_or_else(|e| e.into_inner()) = Some(audio);
                inst.ids = inst.main.params().iter().map(|p| p.id).collect();
                Ok(Answer::Restarted(Some(inst.described())))
            }
            Err(e) => Err(failure(e)),
        },
        Call::SaveState { .. } => {
            let state = inst.main.save_state().map_err(failure)?;
            *reply_blob = state;
            Ok(Answer::State)
        }
        Call::LoadState { .. } => {
            inst.main.load_state(blob).map_err(failure)?;
            Ok(Answer::Done)
        }
        Call::Flush { params, .. } => {
            let changes: Vec<ParamChange> = params
                .into_iter()
                .map(|(offset, id, value)| ParamChange { offset, id, value })
                .collect();
            let mut out = ParamOut::new();
            let processed = inst.main.flush(&changes, &mut out);
            Ok(Answer::Flushed {
                processed,
                out: out.events().iter().map(WireEvent::of).collect(),
            })
        }
        Call::ParamText { id, value, .. } => Ok(Answer::Text(inst.main.param_text(id, value))),
        Call::Params { .. } => {
            let params = inst.main.params();
            inst.ids = params.iter().map(|p| p.id).collect();
            Ok(Answer::Params(params))
        }
        Call::OpenEditor { title, instance } => {
            if let Some(w) = inst.window.as_ref() {
                w.focus();
                return Ok(Answer::Editor {
                    width: 0,
                    height: 0,
                });
            }
            let w = PluginWindow::open(&title, None, inst.main.as_mut()).map_err(failure)?;
            w.focus();
            // Its messages are this instance's: a crash or a modal loop in them is laid on
            // it (review #1, #44).
            if let Some(h) = w.handle() {
                crate::platform::blame::own_window(h, instance);
            }
            inst.window = Some(w);
            Ok(Answer::Editor {
                width: 0,
                height: 0,
            })
        }
        Call::CloseEditor { .. } => {
            inst.close_window();
            Ok(Answer::Done)
        }
        Call::FocusEditor { .. } => {
            if let Some(w) = inst.window.as_ref() {
                w.focus();
            }
            Ok(Answer::Done)
        }
        Call::ResizeEditor { .. } => Ok(Answer::Done),
        _ => Err(Failure::NoInstance),
    }
}

/// Serves until Cantus says Quit or its pipe closes; returns the exit code. `lobby` names
/// the shared region (and bell) Cantus made for this helper's threads.
pub fn run(pipes: Pipes, group: &str, lobby: &str, threads: Option<usize>) -> i32 {
    let _ = group;
    cantus_host::mark_main_thread();
    let Ok(lobby_mem) =
        cantus_rt::shm::SharedMemory::open(lobby, cantus_host::bridge::sandbox::LOBBY_BYTES)
    else {
        return 4;
    };
    // Mapped for the life of the process: the threads, the bell and the blame word point
    // into it.
    let lobby_mem: &'static cantus_rt::shm::SharedMemory = Box::leak(Box::new(lobby_mem));
    // SAFETY: the lobby is at the start of its region, mapped for the life of the process.
    let Ok(bell) = (unsafe {
        cantus_rt::ipc::LobbyBell::open(lobby, lobby_mem.as_ptr() as *const cantus_rt::ipc::Lobby)
    }) else {
        return 4;
    };
    // SAFETY: an aligned u32 inside the region (`MAIN_AT` < its length), mapped for the
    // life of the process; Cantus only reads it.
    let main_at: &'static AtomicU32 = unsafe {
        &*(lobby_mem
            .as_ptr()
            .add(cantus_host::bridge::sandbox::MAIN_AT) as *const AtomicU32)
    };
    // SAFETY: as for `main_at`.
    let crash_at: &'static AtomicU32 = unsafe {
        &*(lobby_mem
            .as_ptr()
            .add(cantus_host::bridge::sandbox::CRASH_AT) as *const AtomicU32)
    };
    // SAFETY: as for `main_at`; the processing threads write it, Cantus reads it.
    let jobs_done: &'static AtomicU32 = unsafe {
        &*(lobby_mem
            .as_ptr()
            .add(cantus_host::bridge::sandbox::JOBS_DONE) as *const AtomicU32)
    };
    crate::platform::blame::install(crash_at, main_at);
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
    let wake = Wake::new();
    let (tx, rx) = mpsc::channel::<Option<(Request, Vec<u8>)>>();
    let input = pipes.input;
    std::thread::Builder::new()
        .name("cantus-sandbox-requests".into())
        .spawn(move || {
            let mut r = BufReader::new(input);
            loop {
                match proto::read_frame::<Request>(&mut r) {
                    Ok(Some(req)) => {
                        if tx.send(Some(req)).is_err() {
                            return;
                        }
                    }
                    _ => {
                        let _ = tx.send(None);
                        wake.ring();
                        return;
                    }
                }
                wake.ring();
            }
        })
        .ok();
    let out: Out = Arc::new(Mutex::new(BufWriter::new(pipes.output)));
    {
        let out = out.clone();
        std::thread::Builder::new()
            .name("cantus-sandbox-heartbeat".into())
            .spawn(move || heartbeat(out))
            .ok();
    }
    let server = Server {
        out,
        instances: RefCell::new(HashMap::new()),
        leaving: RefCell::new(Vec::new()),
        table: Arc::new(Table::default()),
        bell: Arc::new(bell),
        stop: Arc::new(AtomicBool::new(false)),
        threads: RefCell::new(Vec::new()),
        max_threads: threads.unwrap_or_else(|| cores.saturating_sub(2).clamp(1, 8)),
        jobs_done,
    };
    let here = Rc::new(Here {
        server,
        rx,
        deferred: RefCell::new(VecDeque::new()),
        quit: Cell::new(false),
    });
    HERE.with(|h| *h.borrow_mut() = Some(here.clone()));
    crate::platform::start_pulse(pulse);
    let mut last_tick = Instant::now();
    loop {
        wake.wait(TICK);
        PROGRESS.fetch_add(1, Ordering::Relaxed);
        here.serve(false);
        if !here.quit.get() && last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            here.server.tick();
        }
        here.server.end_leaving();
        here.server.flush();
        if here.quit.get() {
            HERE.with(|h| h.borrow_mut().take());
            finish(&here.server);
            return 0;
        }
    }
}

fn finish(server: &Server) {
    let ids: Vec<u32> = server.instances.borrow().keys().copied().collect();
    for id in ids {
        server.unload(id);
    }
    server.end_leaving();
    server.stop.store(true, Ordering::Release);
    server.bell.bell().ring();
    for t in server.threads.borrow_mut().drain(..) {
        let _ = t.join();
    }
    server.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(id: u64, call: Call, hold: Hold) -> (Request, Vec<u8>, Hold) {
        (Request { id, call }, Vec::new(), hold)
    }

    /// What a loop nested in a plug-in's code serves (review isolation-4 #31, #54): the
    /// safety half — an instance is never ended or replaced under its own code, nor called
    /// into while a call of its is under way further out — and the other half: everything
    /// else is served at once.
    #[test]
    fn what_a_nested_loop_serves() {
        let none = VecDeque::new();
        let free = |_: u32| false;
        // Quit: always held.
        assert_eq!(must_wait(&Call::Quit, 0, &none, free), Hold::Held);
        // Not known whose loop: every call that ends or replaces an instance is held...
        assert_eq!(
            must_wait(&Call::Unload { instance: 2 }, 0, &none, free),
            Hold::Held
        );
        // ...the others are served.
        assert_eq!(
            must_wait(&Call::SaveState { instance: 2 }, 0, &none, free),
            Hold::Serve
        );
        // A's window's loop: A's ending calls held, B's served, A's others served.
        assert_eq!(
            must_wait(&Call::CloseEditor { instance: 1 }, 1, &none, free),
            Hold::Held
        );
        assert_eq!(
            must_wait(&Call::Restart { instance: 1 }, 1, &none, free),
            Hold::Held
        );
        assert_eq!(
            must_wait(&Call::Unload { instance: 2 }, 1, &none, free),
            Hold::Serve
        );
        assert_eq!(
            must_wait(&Call::DropAudio { instance: 2 }, 1, &none, free),
            Hold::Serve
        );
        assert_eq!(
            must_wait(&Call::SaveState { instance: 1 }, 1, &none, free),
            Hold::Serve
        );
        // A call of A's under way further out (its dialog inside a load): A's calls wait
        // behind it, B's are served.
        let busy = |n: u32| n == 1;
        assert_eq!(
            must_wait(&Call::SaveState { instance: 1 }, 1, &none, busy),
            Hold::Behind
        );
        assert_eq!(
            must_wait(&Call::SaveState { instance: 2 }, 1, &none, busy),
            Hold::Serve
        );
        // A call waiting: later ones on its instance wait as it does, in its order.
        let waiting = VecDeque::from([held(7, Call::CloseEditor { instance: 1 }, Hold::Held)]);
        assert_eq!(
            must_wait(
                &Call::OpenEditor {
                    instance: 1,
                    title: String::new()
                },
                1,
                &waiting,
                free
            ),
            Hold::Held
        );
        assert_eq!(
            must_wait(&Call::SaveState { instance: 2 }, 1, &waiting, free),
            Hold::Serve
        );
        let behind = VecDeque::from([held(8, Call::SaveState { instance: 1 }, Hold::Behind)]);
        assert_eq!(
            must_wait(&Call::Params { instance: 1 }, 1, &behind, free),
            Hold::Behind
        );
        // Bringing a dialog forward: never held.
        assert_eq!(must_wait(&Call::Surface, 1, &none, |_| true), Hold::Serve);
    }
}
