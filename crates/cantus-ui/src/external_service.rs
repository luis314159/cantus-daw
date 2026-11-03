//! External units on the interface (round 36, ADR 0094): what is present, setting a unit up
//! and taking it away, the wiring to the engine and the MIDI-out thread, the ping.
//!
//! Each frame ([`poll`]):
//! - the **routes** of the enabled instruments (and of a unit being pinged) go to the MIDI-out
//!   thread (`SyncService::set_unit_routes`): without them a stamped note goes nowhere;
//! - the **presence** — which units can play with what is open, and the interface's own
//!   round trip — goes to the engine handle; a change sends the project again, so a unit
//!   whose interface is gone gives no link and a new buffer size re-bases the stored trips;
//! - finished pings come back, are evaluated off this thread and stored with
//!   `SetExternalLatency` (one undo step). A ping that does not come back within
//!   [`PING_TIMEOUT`], or whose device changed meanwhile, is given up and said.

use crate::app::CantusApp;
use cantus_core::external_unit::{
    availability, route_for, Availability, ExternalUnit, ExternalUnitId, MidiRoute, Now, Pair,
    Present, RestoreInput, Staleness, UnitKind,
};
use cantus_core::{Command, TrackId};
use cantus_engine::external_ping::{PingError, PingJob, PingResult};
use cantus_engine::external_presence::ExternalPresence;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// The key an instrument ping plays (middle C); the velocity is the engine's.
pub const PING_KEY: u8 = 60;
/// A ping longer than this has not come back (its run is about seven seconds at most).
pub const PING_TIMEOUT: Duration = Duration::from_secs(15);

/// One ping that is running or being evaluated.
#[derive(Debug)]
struct Running {
    unit: ExternalUnitId,
    rate: u32,
    started: Instant,
    /// The interface when it started (its measurement is stored with it).
    now: Now,
    /// The route its notes go out on (registered while it runs, even for a unit that is off).
    route: Option<(u64, MidiRoute)>,
}

/// The answer of one ping, as the screen keeps it.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Measuring,
    Done(PingResult),
    Failed(PingError),
}

type Evaluated = (ExternalUnitId, u32, Now, Result<PingResult, PingError>);

#[derive(Debug)]
pub struct ExternalService {
    running: Vec<Running>,
    /// What each unit's last ping came to (this session).
    pub outcomes: HashMap<ExternalUnitId, Outcome>,
    /// What is present, as the screenshots and the tests say it (`None`: the real one).
    pub present_override: Option<Present>,
    /// The interface as the tests say it (`None`: the engine's).
    pub now_override: Option<Now>,
    /// The MIDI outputs present, read at most once a second (an enumeration of the ports is
    /// slow: never every frame).
    outputs: std::cell::RefCell<Option<(Instant, Vec<String>)>>,
    tx: Sender<Evaluated>,
    rx: Receiver<Evaluated>,
}

impl Default for ExternalService {
    fn default() -> Self {
        let (tx, rx) = channel();
        ExternalService {
            running: Vec::new(),
            outcomes: HashMap::new(),
            present_override: None,
            now_override: None,
            outputs: Default::default(),
            tx,
            rx,
        }
    }
}

impl ExternalService {
    pub fn is_pinging(&self) -> bool {
        !self.running.is_empty()
    }
}

/// The open device's name (cheap: no port enumeration).
fn device_name(app: &CantusApp) -> Option<String> {
    match &app.external.present_override {
        Some(p) => p.device.clone(),
        None => (!app.engine.device_name.is_empty()).then(|| app.engine.device_name.clone()),
    }
}

/// What is present now: the open device and the MIDI outputs.
pub fn present(app: &CantusApp) -> Present {
    if let Some(p) = &app.external.present_override {
        return p.clone();
    }
    Present {
        device: (!app.engine.device_name.is_empty()).then(|| app.engine.device_name.clone()),
        in_channels: app.input_channels() as u16,
        out_channels: app.engine.channels as u16,
        midi_outputs: midi_outputs(app),
    }
}

/// The MIDI outputs present, enumerated at most once a second.
fn midi_outputs(app: &CantusApp) -> Vec<String> {
    let mut cache = app.external.outputs.borrow_mut();
    match cache.as_ref() {
        Some((at, names)) if at.elapsed() < Duration::from_secs(1) => names.clone(),
        _ => {
            let names = cantus_engine::midi::list_outputs();
            *cache = Some((Instant::now(), names.clone()));
            names
        }
    }
}

/// The interface now: its name, its reported round trip (input + output latency + one
/// buffer), its buffer and rate. `None` without a device.
pub fn interface_now(app: &CantusApp) -> Option<Now> {
    if let Some(n) = &app.external.now_override {
        return Some(n.clone());
    }
    let device = device_name(app)?;
    let lat = app.engine.device_latency();
    let buffer = app.engine.device_buffer().unwrap_or(0);
    Some(Now {
        device,
        rtt: lat.round_trip() + buffer,
        buffer,
        rate: app.engine.sample_rate.round() as u32,
    })
}

/// Whether a unit can play (or be pinged) with what is present.
pub fn state(app: &CantusApp, unit: &ExternalUnit) -> Availability {
    availability(app.project(), unit, &present(app))
}

/// How a unit's measurement stands against the interface now.
pub fn staleness(app: &CantusApp, unit: &ExternalUnit) -> Staleness {
    match (&unit.latency, interface_now(app)) {
        (None, _) => Staleness::NotMeasured,
        (Some(m), Some(now)) => m.staleness(&now),
        (Some(m), None) => Staleness::OtherDevice {
            measured_buffer: m.buffer,
        },
    }
}

/// Why a ping cannot start now, or `None`.
pub fn cannot_ping(app: &CantusApp, id: ExternalUnitId) -> Option<&'static str> {
    let Some(u) = app.project().external_unit(id) else {
        return Some("external.ping.no_unit");
    };
    if app.engine.is_playing() {
        return Some("external.ping.playing");
    }
    if app.external.is_pinging() {
        return Some("external.ping.busy");
    }
    // A unit that is off is pinged all the same (to see it), but one not present is not.
    match state(app, u) {
        Availability::Ready | Availability::Off => None,
        other => Some(other.key()),
    }
}

/// Start the ping of unit `id`. The transport must be stopped.
pub fn start_ping(app: &mut CantusApp, id: ExternalUnitId) -> Result<(), &'static str> {
    if let Some(why) = cannot_ping(app, id) {
        return Err(why);
    }
    let Some(u) = app.project().external_unit(id).cloned() else {
        return Err("external.ping.no_unit");
    };
    let Some(now) = interface_now(app) else {
        return Err("external.state.no_device");
    };
    let rate = app.engine.sample_rate;
    let mut route = None;
    let job = match u.kind {
        UnitKind::Effect => PingJob::audio(id, rate, u.send.unwrap_or(Pair::stereo(2)), u.ret),
        UnitKind::Instrument => {
            let Some(m) = u.midi.clone() else {
                return Err("external.state.port");
            };
            // Its own route, registered while it runs: a unit that is off has none in the
            // project's table, and never borrows another's.
            let r = route_for(id, &m);
            route = Some((r, m.clone()));
            PingJob::onset(id, rate, r, m.channel, PING_KEY, u.ret)
        }
    };
    app.external.running.push(Running {
        unit: id,
        rate: rate as u32,
        started: Instant::now(),
        now,
        route,
    });
    publish_routes(app);
    app.engine.start_ping(Box::new(job));
    app.external.outcomes.insert(id, Outcome::Measuring);
    Ok(())
}

/// The routes the MIDI-out thread must know: every enabled instrument's, and a running
/// ping's.
pub fn routes(app: &CantusApp) -> Vec<(u64, String, u8)> {
    let mut v: Vec<(u64, String, u8)> = app
        .project()
        .midi_routes()
        .into_iter()
        .map(|(r, m, _)| (r, m.port, m.ordinal))
        .collect();
    for run in &app.external.running {
        if let Some((r, m)) = &run.route {
            if !v.iter().any(|(x, _, _)| x == r) {
                v.push((*r, m.port.clone(), m.ordinal));
            }
        }
    }
    v
}

fn publish_routes(app: &mut CantusApp) {
    let r = routes(app);
    app.sync.set_unit_routes(r);
}

/// The presence the engine handle is given.
pub fn presence(app: &CantusApp) -> ExternalPresence {
    ExternalPresence {
        ready: app
            .project()
            .external_units
            .iter()
            .filter(|u| state(app, u).is_ready())
            .map(|u| u.id)
            .collect(),
        now: interface_now(app),
    }
}

/// Each frame: routes, presence, pings.
pub fn poll(app: &mut CantusApp) {
    publish_routes(app);
    // Nothing outboard: nothing to look at (no port enumeration on a project without units).
    if !app.project().external_units.is_empty() || app.external.is_pinging() {
        let p = presence(app);
        if app.engine.set_external_presence(p) {
            app.force_resync();
        }
    }
    // Pings that do not come back, or whose interface changed under them, are given up.
    // (The device only matters while a ping runs: no port enumeration otherwise.)
    let device_now = if app.external.running.is_empty() {
        None
    } else {
        device_name(app)
    };
    let mut gave_up = vec![];
    app.external.running.retain(|r| {
        let lost =
            r.started.elapsed() > PING_TIMEOUT || device_now.as_deref() != Some(&r.now.device);
        if lost {
            gave_up.push(r.unit);
        }
        !lost
    });
    for u in gave_up {
        app.external
            .outcomes
            .insert(u, Outcome::Failed(PingError::TimedOut));
    }
    for job in app.engine.take_pings() {
        let unit = job.unit;
        // A job given up already is let go.
        let Some(run) = app.external.running.iter().find(|r| r.unit == unit) else {
            continue;
        };
        let (rate, now) = (run.rate, run.now.clone());
        let tx = app.external.tx.clone();
        std::thread::spawn(move || {
            let r = job.evaluate();
            let _ = tx.send((unit, rate, now, r));
        });
    }
    while let Ok((unit, rate, now, result)) = app.external.rx.try_recv() {
        if !app.external.running.iter().any(|r| r.unit == unit) {
            continue;
        }
        app.external.running.retain(|r| r.unit != unit);
        match result {
            Ok(r) => {
                app.exec(Command::SetExternalLatency {
                    id: unit,
                    latency: Some(r.measured(rate, &now)),
                });
                app.external.outcomes.insert(unit, Outcome::Done(r));
            }
            Err(e) => {
                app.external.outcomes.insert(unit, Outcome::Failed(e));
            }
        }
    }
}

/// The engine was replaced (an audio restart, another device): a ping in flight is gone.
pub fn engine_replaced(app: &mut CantusApp) {
    for r in std::mem::take(&mut app.external.running) {
        app.external
            .outcomes
            .insert(r.unit, Outcome::Failed(PingError::TimedOut));
    }
}

/// The unit a track would get, with the routing its first free choices suggest: an
/// instrument sends to the first MIDI output and listens on the interface's first pair; an
/// effect goes out of outputs 3-4 (past the master's) when there are, back on the first pair.
pub fn new_unit(app: &mut CantusApp, track: TrackId, kind: UnitKind) -> Option<ExternalUnit> {
    let present = present(app);
    let n = app.project().next_id;
    app.bus.reserve_ids(n + 1);
    let id = ExternalUnitId(n);
    let ret = Pair::stereo(0);
    let mut u = match kind {
        UnitKind::Instrument => ExternalUnit::instrument(
            id,
            track,
            MidiRoute {
                port: present.midi_outputs.first().cloned().unwrap_or_default(),
                ordinal: 0,
                channel: 0,
            },
            ret,
        ),
        UnitKind::Effect => {
            let send = Pair::stereo(if present.out_channels >= 4 { 2 } else { 0 });
            ExternalUnit::effect(id, track, send, ret)
        }
    };
    u.device = present.device.clone().unwrap_or_default();
    u.name = app.project().track(track).map(|t| t.name.clone())?;
    Some(u)
}

/// The commands that add `unit`: the unit, and the track's input made its return
/// (monitored, so it is heard), the input as it was kept for its removal. One undo step.
pub fn add_commands(p: &cantus_core::Project, mut unit: ExternalUnit) -> Command {
    let before = p.track(unit.track).and_then(|t| t.input.clone());
    unit.restore_input = Some(RestoreInput {
        input: before.clone(),
    });
    let input = cantus_core::audio::TrackInput {
        first: unit.ret.first,
        stereo: unit.ret.stereo,
        monitor: cantus_core::audio::MonitorMode::On,
        trim_db: before.as_ref().map_or(0.0, |i| i.trim_db),
    };
    let track = unit.track;
    Command::Batch {
        label_key: "cmd.external.add".into(),
        commands: vec![
            Command::InsertExternalUnit {
                index: p.external_units.len(),
                unit: Box::new(unit),
            },
            Command::SetTrackInput {
                id: track,
                input: Some(input),
            },
        ],
    }
}

/// Add a unit to `track` (one undo step).
pub fn add_unit(app: &mut CantusApp, track: TrackId, kind: UnitKind) -> bool {
    if app.project().external_unit_of(track, kind).is_some() {
        return false;
    }
    let Some(unit) = new_unit(app, track, kind) else {
        return false;
    };
    let cmd = add_commands(app.project(), unit);
    app.exec(cmd)
}

/// Remove unit `id`, giving the track back the input it had before (one undo step).
pub fn remove_unit(app: &mut CantusApp, id: ExternalUnitId) -> bool {
    let Some(u) = app.project().external_unit(id).cloned() else {
        return false;
    };
    let mut commands = vec![Command::RemoveExternalUnit { id }];
    if let Some(r) = u.restore_input {
        if app.project().track(u.track).is_some() {
            commands.push(Command::SetTrackInput {
                id: u.track,
                input: r.input,
            });
        }
    }
    app.exec(Command::Batch {
        label_key: "cmd.external.remove".into(),
        commands,
    })
}

/// The track menu's *External instrument…* / *External effect…* (or their removal).
/// Returns whether one was chosen.
pub fn track_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    let has = |k| app.project().external_unit_of(id, k).map(|u| u.id);
    let (inst, fx) = (has(UnitKind::Instrument), has(UnitKind::Effect));
    let item = |label| Item::Action {
        label,
        check: Check::None,
        shortcut: None,
        submenu: false,
        enabled: true,
    };
    let items = [
        Item::Band(Src::Key("external.menu.band")),
        item(Src::Key(if inst.is_some() {
            "external.menu.remove_instrument"
        } else {
            "external.menu.instrument"
        })),
        item(Src::Key(if fx.is_some() {
            "external.menu.remove_effect"
        } else {
            "external.menu.effect"
        })),
    ];
    match crate::kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
        Some(1) => {
            match inst {
                Some(u) => remove_unit(app, u),
                None => add_unit(app, id, UnitKind::Instrument),
            };
            true
        }
        Some(2) => {
            match fx {
                Some(u) => remove_unit(app, u),
                None => add_unit(app, id, UnitKind::Effect),
            };
            true
        }
        _ => false,
    }
}

/// The tracks whose outboard gear an offline render leaves out (enabled units, tracks that
/// play).
pub fn outboard_tracks(p: &cantus_core::Project) -> Vec<String> {
    let mut v: Vec<String> = p
        .external_units
        .iter()
        .filter(|u| u.enabled)
        .filter_map(|u| {
            p.track(u.track)
                .filter(|t| !t.muted)
                .map(|t| t.name.clone())
        })
        .collect();
    v.dedup();
    v
}

/// [`export_refusal`] with the words of another render (`key` takes `{track}`).
pub fn render_refusal(app: &CantusApp, key: &str) -> Option<String> {
    let track = outboard_tracks(app.project()).into_iter().next()?;
    Some(crate::i18n::tf(key, &[("track", &track)]))
}

/// Why the song cannot be exported (or a track bounced) as long as outboard gear plays in it:
/// an offline render cannot play a synth or a rack processor. `None` when nothing outboard
/// sounds.
pub fn export_refusal(app: &CantusApp) -> Option<String> {
    let p = app.project();
    let u = p
        .external_units
        .iter()
        .find(|u| u.enabled && p.track(u.track).is_some_and(|t| !t.muted))?;
    let track = p.track(u.track).map(|t| t.name.clone()).unwrap_or_default();
    Some(crate::i18n::tf(
        "external.cannot_export",
        &[("track", &track)],
    ))
}
