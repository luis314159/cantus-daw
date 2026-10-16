//! The helper's main thread (ADR 0063): Cantus's calls, the plug-ins' windows and requests,
//! and the heartbeat.
//!
//! Every call that names an instance is answered with the instance's news after it — the
//! requests it made (`MainThread::idle`) and all its values — so Cantus's mirror is current
//! the moment the call returns (a restart the plug-in asked for while its state loaded is
//! seen at once, as in process). Between calls, every [`TICK`] each instance is served as
//! the host's frame loop serves it in process: its requests, a flush it asked for, its
//! window's close box and resizes; what changed goes to Cantus as news.

use crate::platform::{Pipes, Wake};
use crate::worker::{self, Half};
use cantus_host::bridge::proto::{
    self, After, Answer, Call, Described, Failure, Message, Request, WireEvent, WireNotice,
};
use cantus_host::bridge::slot::Slot;
use cantus_host::window::PluginWindow;
use cantus_host::{
    AudioSetup, HostError, MainThread, Notice, ParamChange, ParamOut,
};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the instances are served between calls.
const TICK: Duration = Duration::from_millis(15);
/// How often the heartbeat goes out.
const ALIVE: Duration = Duration::from_millis(200);

struct Instance {
    main: Box<dyn MainThread>,
    half: Half,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    window: Option<PluginWindow>,
    /// The values Cantus was last told.
    told: HashMap<u32, f64>,
    ids: Vec<u32>,
}

impl Instance {
    fn finish(mut self) {
        if let Some(w) = self.window.take() {
            self.main.close_editor();
            #[cfg_attr(not(windows), allow(clippy::drop_non_drop))]
            drop(w);
        }
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
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
        HostError::Crashed(s) => Failure::Load(s),
    }
}

struct Server {
    out: BufWriter<std::fs::File>,
    instances: HashMap<u32, Instance>,
}

impl Server {
    fn send(&mut self, msg: &Message, blob: &[u8]) -> bool {
        proto::write_frame(&mut self.out, msg, blob).is_ok()
    }

    fn load(
        &mut self,
        instance: u32,
        info: &cantus_host::PluginInfo,
        sample_rate: f64,
        max_block: usize,
        slot_name: &str,
    ) -> Result<Answer, Failure> {
        let backend = cantus_host::backend_for(info.format)
            .ok_or_else(|| Failure::Unsupported("format".into()))?;
        let slot = Arc::new(
            Slot::open(slot_name, max_block).map_err(|e| Failure::Load(format!("shared memory: {e}")))?,
        );
        // SAFETY: the exchange lives in `slot`, which the worker thread keeps alive.
        let bells = unsafe { cantus_rt::ipc::Worker::open(slot_name, slot.exchange()) }
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
        let stop = Arc::new(AtomicBool::new(false));
        let (ready, started) = mpsc::channel();
        let thread = {
            let (slot, half, stop) = (slot.clone(), half.clone(), stop.clone());
            std::thread::Builder::new()
                .name(format!("cantus-sandbox-audio-{instance}"))
                .spawn(move || worker::serve(slot, bells, half, stop, ready))
                .map_err(|e| Failure::Load(e.to_string()))?
        };
        // The first block finds the thread waiting, not starting.
        let _ = started.recv_timeout(Duration::from_secs(5));
        let main = loaded.main;
        let ids = main.params().iter().map(|p| p.id).collect();
        let inst = Instance {
            main,
            half,
            stop,
            thread: Some(thread),
            window: None,
            told: HashMap::new(),
            ids,
        };
        let described = inst.described();
        self.instances.insert(instance, inst);
        Ok(Answer::Loaded(described))
    }

    /// One call; `None` for Quit.
    fn handle(&mut self, req: Request, blob: Vec<u8>) -> Option<()> {
        let id = req.id;
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
            Call::Load {
                instance,
                info,
                sample_rate,
                max_block,
                slot,
            } => {
                named = Some(instance);
                self.load(instance, &info, sample_rate, max_block, &slot)
            }
            Call::Unload { instance } => {
                if let Some(i) = self.instances.remove(&instance) {
                    i.finish();
                }
                Ok(Answer::Done)
            }
            call => {
                let instance = match &call {
                    Call::DropAudio { instance }
                    | Call::Restart { instance }
                    | Call::SaveState { instance }
                    | Call::LoadState { instance }
                    | Call::Flush { instance, .. }
                    | Call::ParamText { instance, .. }
                    | Call::Params { instance }
                    | Call::OpenEditor { instance, .. }
                    | Call::CloseEditor { instance }
                    | Call::FocusEditor { instance }
                    | Call::ResizeEditor { instance, .. } => *instance,
                    _ => 0,
                };
                named = Some(instance);
                match self.instances.get_mut(&instance) {
                    None => Err(Failure::NoInstance),
                    Some(inst) => on_instance(inst, call, &blob, &mut reply_blob),
                }
            }
        };
        let after = named.and_then(|n| self.instances.get_mut(&n).map(|i| (n, i.after())));
        self.send(&Message::Reply { id, result, after }, &reply_blob);
        Some(())
    }

    /// Between calls: each instance's requests, flushed events and changed values.
    fn tick(&mut self) {
        let mut news = Vec::new();
        for (n, inst) in &mut self.instances {
            let mut out = ParamOut::new();
            inst.main.flush(&[], &mut out);
            let flushed: Vec<WireEvent> = out.events().iter().map(WireEvent::of).collect();
            let (notices, values) = inst.news();
            if !notices.is_empty() || !values.is_empty() || !flushed.is_empty() {
                news.push(Message::News {
                    instance: *n,
                    notices,
                    values,
                    flushed,
                });
            }
        }
        for m in news {
            self.send(&m, &[]);
        }
        cantus_host::clap::collect_garbage();
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
        Call::OpenEditor { title, .. } => {
            if let Some(w) = inst.window.as_ref() {
                w.focus();
                return Ok(Answer::Editor {
                    width: 0,
                    height: 0,
                });
            }
            let w = PluginWindow::open(&title, None, inst.main.as_mut()).map_err(failure)?;
            w.focus();
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

/// Serves until Cantus says Quit or its pipe closes; returns the exit code.
pub fn run(pipes: Pipes, group: &str) -> i32 {
    let _ = group;
    cantus_host::mark_main_thread();
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
    let mut server = Server {
        out: BufWriter::new(pipes.output),
        instances: HashMap::new(),
    };
    let mut last_tick = Instant::now();
    let mut last_alive = Instant::now() - ALIVE;
    loop {
        wake.wait(TICK);
        loop {
            match rx.try_recv() {
                Ok(Some((req, blob))) => {
                    if server.handle(req, blob).is_none() {
                        finish(server);
                        return 0;
                    }
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                    // Cantus is gone.
                    finish(server);
                    return 0;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            server.tick();
        }
        if last_alive.elapsed() >= ALIVE {
            last_alive = Instant::now();
            if !server.send(&Message::Alive, &[]) {
                finish(server);
                return 0;
            }
        }
        let _ = server.out.flush();
    }
}

fn finish(mut server: Server) {
    for (_, i) in server.instances.drain() {
        i.finish();
    }
}
