//! Sync, the application's side (round 33, ADR 0078–0079): one source at a time — none,
//! Ableton Link, MIDI clock on a port, time code on a port — turned into the engine's
//! abstract timeline (`cantus_engine::transport::sync`), and the engine's MIDI clock and
//! time code sent to the ports that take them.
//!
//! A thread of its own (never the audio thread, never under MMCSS) runs every millisecond:
//! it feeds the chosen port's clock or time code to its follower, reads Link's session, and
//! publishes the timeline; it hands the engine's proposals (the user's play, stop and tempo
//! while Link is followed) to Link. The **MIDI-out** thread (`sync::out`) is the engine's;
//! this file only tells it the ports. Link (`cantus-link`, GPL) lives here, in the GPL
//! application, never in the MPL engine (ADR 0047).
//!
//! Wired from the boot path: `CantusApp::assemble` and `restart_audio` (`wire`,
//! `engine_replaced`), so the shipped app — which never calls `CantusApp::new` — has it.

use crate::app::CantusApp;
use cantus_engine::midi::sync::{BLOCKS_CONTROLLERS, BLOCKS_NOTES, TAKES_CLOCK, TAKES_MTC};
use cantus_engine::midi::MidiTap;
use cantus_engine::transport::sync::{self, mtc, out, Proposal, SyncHandle, SyncShared};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[path = "sync_service_run.rs"]
mod run;

/// The source followed.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    /// The project's own tempo.
    #[default]
    None,
    /// Ableton Link.
    Link,
    /// MIDI clock on `SyncSettings::source_port`.
    Clock,
    /// MIDI time code on `SyncSettings::source_port`.
    Mtc,
}

/// What one MIDI port does (Settings ▸ MIDI's table). An input's notes and controllers
/// reach the engine and the controller map; its clock or time code reach sync when it is
/// the source's port. An output sends clock, time code and controller feedback, later by
/// its offset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PortSettings {
    pub name: String,
    pub notes: bool,
    pub controllers: bool,
    pub clock: bool,
    pub mtc: bool,
    pub feedback: bool,
    /// An output's messages leave this much later (earlier, negative) than its audio, ms.
    pub offset_ms: f32,
}

impl Default for PortSettings {
    fn default() -> Self {
        PortSettings {
            name: String::new(),
            notes: true,
            controllers: true,
            clock: false,
            mtc: false,
            feedback: true,
            offset_ms: 0.0,
        }
    }
}

/// The most a port's offset reaches, ms.
pub const MAX_OFFSET_MS: f32 = 100.0;

/// Sync's choices, kept in `settings.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncSettings {
    pub source: SyncSource,
    /// The input port the clock or time code comes from.
    pub source_port: Option<String>,
    /// Link's quantum, beats (phase is matched within it: 4 is a bar of 4/4).
    pub quantum: f64,
    /// Link: start and stop with the session (Live's *Start Stop Sync*).
    pub start_stop: bool,
    /// Time code out's rate (0 = 24, 1 = 25, 2 = 29.97 drop-frame, 3 = 30).
    pub mtc_rate: u8,
    /// The time code at the song's start, `hh:mm:ss:ff`.
    pub mtc_offset: String,
    pub inputs: Vec<PortSettings>,
    pub outputs: Vec<PortSettings>,
}

impl Default for SyncSettings {
    fn default() -> Self {
        SyncSettings {
            source: SyncSource::None,
            source_port: None,
            quantum: 4.0,
            start_stop: false,
            mtc_rate: 1,
            mtc_offset: "00:00:00:00".into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

/// The quanta offered (beats).
pub const QUANTA: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 8.0];

impl SyncSettings {
    fn port(list: &[PortSettings], name: &str) -> PortSettings {
        list.iter()
            .find(|p| p.name == name)
            .cloned()
            .unwrap_or_else(|| PortSettings {
                name: name.to_string(),
                ..PortSettings::default()
            })
    }
    fn port_mut<'a>(list: &'a mut Vec<PortSettings>, name: &str) -> &'a mut PortSettings {
        match list.iter().position(|p| p.name == name) {
            Some(i) => &mut list[i],
            None => {
                list.push(PortSettings {
                    name: name.to_string(),
                    ..PortSettings::default()
                });
                list.last_mut().unwrap_or_else(|| unreachable!())
            }
        }
    }
    pub fn input(&self, name: &str) -> PortSettings {
        Self::port(&self.inputs, name)
    }
    pub fn input_mut(&mut self, name: &str) -> &mut PortSettings {
        Self::port_mut(&mut self.inputs, name)
    }
    pub fn output(&self, name: &str) -> PortSettings {
        Self::port(&self.outputs, name)
    }
    pub fn output_mut(&mut self, name: &str) -> &mut PortSettings {
        Self::port_mut(&mut self.outputs, name)
    }

    pub fn rate(&self) -> mtc::Rate {
        mtc::Rate::from_code(u32::from(self.mtc_rate))
    }

    /// The start offset, parsed (`None` when it does not read as `hh:mm:ss:ff`).
    pub fn offset_code(&self) -> Option<mtc::Timecode> {
        parse_code(&self.mtc_offset, self.rate())
    }
}

/// `hh:mm:ss:ff` (or `;` before the frames) at `rate`, within range.
pub fn parse_code(s: &str, rate: mtc::Rate) -> Option<mtc::Timecode> {
    let parts: Vec<&str> = s.trim().split([':', ';', '.']).collect();
    let [h, m, sec, f] = parts.as_slice() else {
        return None;
    };
    let n = |x: &str, max: u8| x.trim().parse::<u8>().ok().filter(|v| *v <= max);
    let tc = mtc::Timecode {
        h: n(h, 23)?,
        m: n(m, 59)?,
        s: n(sec, 59)?,
        f: n(f, (rate.nominal() - 1) as u8)?,
        rate,
    };
    // A dropped label is no time.
    if rate == mtc::Rate::Df2997 && tc.s == 0 && !tc.m.is_multiple_of(10) && tc.f < 2 {
        return None;
    }
    Some(tc)
}

/// What the service tells the interface (lock-free; the top bar reads it each frame).
#[derive(Debug, Default)]
pub struct Status {
    pub link_on: AtomicBool,
    pub peers: AtomicU32,
    pub link_tempo: AtomicU64,
    /// Link could not open its sockets (the network, a port refused): said once.
    pub link_failed: AtomicBool,
    /// The followed time code now (seconds bits) and its rate, for the display.
    pub code_s: AtomicU64,
    pub code_rate: AtomicU32,
    /// The clock or code is arriving from the source port.
    pub receiving: AtomicBool,
}

/// The service's choices as its thread reads them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Cfg {
    pub source: SyncSource,
    pub port: Option<usize>,
    pub quantum: f64,
    pub start_stop: bool,
    pub project_bpm: f64,
}

/// The engine's side the thread works with, swapped on an audio restart.
pub(crate) struct EngineSide {
    pub shared: Arc<SyncShared>,
    pub proposals: rtrb::Consumer<Proposal>,
}

/// The service.
#[derive(Default)]
pub struct SyncService {
    cfg: Arc<Mutex<Cfg>>,
    engine: Arc<Mutex<Option<EngineSide>>>,
    pub status: Arc<Status>,
    shared: Option<Arc<SyncShared>>,
    /// A tempo the user asks Link for (the tempo well while Link is followed).
    tempo_req: Arc<Mutex<Option<f64>>>,
    out: Option<out::MidiOut>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// The last port choices given to the out thread.
    out_ports: Vec<out::PortChoice>,
    /// The output ports, shared by clock out and controller feedback (integrated review
    /// #9: two connections to one port, the class driver refuses the second).
    pool: Option<Arc<cantus_engine::midi::pool::OutPool>>,
}

impl std::fmt::Debug for SyncService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncService")
            .field("running", &self.thread.is_some())
            .finish()
    }
}

impl Drop for SyncService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl SyncService {
    /// Whether its thread runs (the boot path's test).
    pub fn running(&self) -> bool {
        self.thread.is_some()
    }

    /// A seam of its own for the screenshots (a dummy engine has none): the top bar and
    /// Settings then read the lock, the error and the tempo set on it.
    pub fn demo_seam(&mut self) -> Arc<SyncShared> {
        let (_port, h) = sync::pair();
        self.shared = Some(h.shared.clone());
        h.shared
    }

    /// The engine's seam now (`None` without an audio device).
    pub fn shared(&self) -> Option<&Arc<SyncShared>> {
        self.shared.as_ref()
    }

    /// The shared output ports (made the first time).
    pub fn pool(&mut self) -> &Arc<cantus_engine::midi::pool::OutPool> {
        self.pool
            .get_or_insert_with(|| cantus_engine::midi::pool::OutPool::new(out::connect_system))
    }

    /// Start the thread on `tap`'s clock and time code (once).
    fn start(&mut self, tap: &MidiTap) {
        if self.thread.is_some() {
            return;
        }
        let Some(rx) = tap.sync_input().take_receiver() else {
            return;
        };
        let (cfg, engine, status, stop, req) = (
            self.cfg.clone(),
            self.engine.clone(),
            self.status.clone(),
            self.stop.clone(),
            self.tempo_req.clone(),
        );
        self.thread = std::thread::Builder::new()
            .name("cantus-sync".into())
            .spawn(move || run::run(rx, cfg, engine, status, stop, req))
            .ok();
    }

    /// Ask Link's session for `bpm` (applied on the sync thread within a millisecond).
    pub fn request_tempo(&self, bpm: f64) {
        if let Ok(mut r) = self.tempo_req.lock() {
            *r = Some(bpm);
        }
        self.status
            .link_tempo
            .store(bpm.to_bits(), Ordering::Relaxed);
    }

    /// A new engine (start, an audio restart): its seam, and the out thread for it.
    pub fn attach(&mut self, handle: Option<SyncHandle>) {
        self.out = None;
        self.out_ports.clear();
        let Some(mut h) = handle else {
            self.shared = None;
            *self.engine.lock().unwrap_or_else(|e| e.into_inner()) = None;
            return;
        };
        if let Some(ring) = h.out.take() {
            let pool = self.pool().clone();
            self.out = Some(out::MidiOut::start(ring, h.shared.clone(), pool));
        }
        self.shared = Some(h.shared.clone());
        *self.engine.lock().unwrap_or_else(|e| e.into_inner()) = Some(EngineSide {
            shared: h.shared,
            proposals: h.proposals,
        });
    }

    /// Apply the settings: the source, the ports' choices, what the engine makes.
    pub fn apply(
        &mut self,
        st: &SyncSettings,
        inputs: &[String],
        outputs: &[String],
        tap: &MidiTap,
        project_bpm: f64,
    ) {
        self.start(tap);
        for (i, name) in inputs.iter().enumerate() {
            let p = st.input(name);
            let source = st.source_port.as_deref() == Some(name.as_str());
            let mut bits = 0;
            if source && st.source == SyncSource::Clock {
                bits |= TAKES_CLOCK;
            }
            if source && st.source == SyncSource::Mtc {
                bits |= TAKES_MTC;
            }
            if !p.notes {
                bits |= BLOCKS_NOTES;
            }
            if !p.controllers {
                bits |= BLOCKS_CONTROLLERS;
            }
            tap.sync_input().set_takes(i, bits);
        }
        let port = st
            .source_port
            .as_ref()
            .and_then(|n| inputs.iter().position(|x| x == n));
        let cfg = Cfg {
            source: st.source,
            port,
            quantum: st.quantum.max(1.0),
            start_stop: st.start_stop,
            project_bpm,
        };
        *self.cfg.lock().unwrap_or_else(|e| e.into_inner()) = cfg;
        let choices: Vec<out::PortChoice> = outputs
            .iter()
            .map(|n| {
                let p = st.output(n);
                out::PortChoice {
                    name: n.clone(),
                    clock: p.clock,
                    mtc: p.mtc,
                    offset_ms: p.offset_ms.clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS),
                }
            })
            .collect();
        if let Some(shared) = &self.shared {
            let o = &shared.out;
            o.clock
                .store(choices.iter().any(|c| c.clock), Ordering::Relaxed);
            o.mtc
                .store(choices.iter().any(|c| c.mtc), Ordering::Relaxed);
            o.mtc_rate
                .store(u32::from(st.mtc_rate.min(3)), Ordering::Relaxed);
            o.set_offset_s(st.offset_code().map_or(0.0, |c| c.seconds()));
        }
        if choices != self.out_ports {
            if let Some(o) = &self.out {
                o.set_ports(choices.clone());
            }
            self.out_ports = choices;
        }
    }
}

/// Start sync on the boot path (`CantusApp::assemble`).
pub fn wire(app: &mut CantusApp) {
    // Feedback writes through the ports clock out shares.
    let pool = app.sync.pool().clone();
    app.controllers
        .outputs
        .set_pool(pool, cantus_engine::midi::output_name_for);
    let handle = app.engine.take_sync();
    app.sync.attach(handle);
    apply(app);
}

/// The engine was replaced (an audio restart): the new one's seam.
pub fn engine_replaced(app: &mut CantusApp) {
    let handle = app.engine.take_sync();
    app.sync.attach(handle);
    apply(app);
}

/// The settings or the ports changed: tell the service.
pub fn apply(app: &mut CantusApp) {
    let outputs = cantus_engine::midi::list_outputs();
    let blocked: Vec<String> = outputs
        .iter()
        .filter(|n| !app.settings.sync.output(n).feedback)
        .cloned()
        .collect();
    app.controllers.outputs.set_blocked(blocked);
    let bpm = app.project().tempo_bpm;
    let tap = app.controllers.tap.clone();
    let names = app.midi_port_names.clone();
    app.sync
        .apply(&app.settings.sync, &names, &outputs, &tap, bpm);
}

/// Whether `cmd` edits the song's tempo (a user's tempo change, proposed to Link).
pub fn is_tempo_edit(cmd: &cantus_core::Command) -> bool {
    use cantus_core::Command as C;
    matches!(
        cmd,
        C::SetTempo { .. }
            | C::SetTempoTrack { .. }
            | C::InsertTempoPoint { .. }
            | C::RemoveTempoPoint { .. }
            | C::SetTempoPoint { .. }
    )
}

/// A tempo edit of the user's ran (`edited`): while Link is followed its session takes the
/// song's tempo at the playhead. Only explicit edits propose — never a new snapshot's map
/// (integrated review #7); a scene's tempo is the session conductor's to propose
/// (`Processor::propose_tempo`).
pub fn tempo_edited(app: &mut CantusApp, edited: bool) {
    if !edited || app.settings.sync.source != SyncSource::Link {
        return;
    }
    let map = app.project().tempo_map();
    let bpm = map.bpm_at(app.engine.position().0 as f64);
    app.sync.request_tempo(bpm);
}

/// Turn Link on or off (the top bar's key): Link is then the one source, or none.
pub fn toggle_link(app: &mut CantusApp) {
    let s = &mut app.settings.sync;
    s.source = if s.source == SyncSource::Link {
        SyncSource::None
    } else {
        SyncSource::Link
    };
    apply(app);
    app.store_settings();
}

/// The host clock sync runs on (the engine's).
pub fn now() -> i64 {
    sync::host_micros()
}

impl CantusApp {
    /// The sync service (the tests, the top bar).
    pub fn sync_service(&self) -> &SyncService {
        &self.sync
    }
}

impl CantusApp {
    /// The sync service, to change (the screenshots).
    pub fn sync_service_mut(&mut self) -> &mut SyncService {
        &mut self.sync
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integrated review #7: a tempo edit of the user's, while Link is followed, is
    /// proposed to the session (the service is not started: no socket in a test).
    #[test]
    fn a_tempo_edit_is_proposed_to_link() {
        let mut app = CantusApp::from_parts(None, cantus_engine::EngineHandle::dummy(), false);
        app.settings.sync.source = SyncSource::Link;
        assert!(app.exec(cantus_core::Command::SetTempo { bpm: 137.0 }));
        let t = f64::from_bits(app.sync.status.link_tempo.load(Ordering::Relaxed));
        assert_eq!(t, 137.0);
        // Not following Link: nothing proposed.
        app.settings.sync.source = SyncSource::None;
        assert!(app.exec(cantus_core::Command::SetTempo { bpm: 90.0 }));
        let t = f64::from_bits(app.sync.status.link_tempo.load(Ordering::Relaxed));
        assert_eq!(t, 137.0);
    }
}
