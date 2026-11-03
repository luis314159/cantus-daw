//! The surfaces as the interface runs them: ports in, ports out, the virtual units.
//!
//! Each frame ([`tick`]): what the surfaces' ports said (taken by [`heard`] before the generic
//! map sees it, and the port taken whole in the filter, [`taken_ports`]) goes through the
//! set, its actions are applied as commands ([`super::apply`]), held rewind/forward move the
//! playhead, and what the units should show is rendered (only what changed) and written to
//! their output ports through the pool the clock shares — or to a virtual unit.

use super::{apply, view};
use crate::app::CantusApp;
use cantus_core::external_emulation::{VirtualHui, VirtualMcu};
use cantus_core::external_surface::{Kind, SurfaceAction, SurfaceSet};
use cantus_core::{Command, MergeKey, Origin};
use cantus_engine::midi::Heard;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// One surface of the studio, as Settings keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurfaceChoice {
    /// `mcu`, `mcu-xt1`…, `hui` (`Kind::id`).
    pub kind: String,
    /// The MIDI input port's name (empty for a virtual unit).
    pub input: String,
    /// The MIDI output port's name; empty chooses the one that answers the input.
    pub output: String,
    pub enabled: bool,
    /// A virtual unit (the emulation) in place of ports.
    pub virtual_unit: bool,
    /// The faders sense touch (every MCU and HUI does); off, a moved fader's motor holds a
    /// moment instead of fighting the hand.
    pub touch_sensing: bool,
}

impl Default for SurfaceChoice {
    fn default() -> Self {
        SurfaceChoice {
            kind: "mcu".into(),
            input: String::new(),
            output: String::new(),
            enabled: true,
            virtual_unit: false,
            touch_sensing: true,
        }
    }
}

/// A virtual unit.
#[derive(Clone, Debug)]
pub enum Unit {
    Mcu(Box<VirtualMcu>),
    Hui(Box<VirtualHui>),
}

impl Unit {
    fn new(kind: Kind) -> Unit {
        match kind {
            Kind::Hui => Unit::Hui(Box::new(VirtualHui::new())),
            Kind::McuExtender(_) => Unit::Mcu(Box::new(VirtualMcu::new(true))),
            Kind::Mcu => Unit::Mcu(Box::new(VirtualMcu::new(false))),
        }
    }

    fn receive(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        match self {
            Unit::Mcu(u) => u.receive(bytes),
            Unit::Hui(u) => u.receive(bytes),
        }
    }
}

const LOG: usize = 48;

/// The surfaces' runtime state.
#[derive(Default)]
pub struct SurfaceService {
    pub set: Option<SurfaceSet>,
    choices: Vec<SurfaceChoice>,
    /// Per device: its virtual unit, when it is one.
    pub units: Vec<Option<Unit>>,
    /// The last messages, newest last: `in 90 5E 7F`, `out E0 00 40`, notes on unmapped ports.
    pub log: VecDeque<String>,
    held: (bool, bool),
    pub received: u64,
    pub sent: u64,
    /// The service's clock (the keep-alive, the meters' refresh, the no-touch hold count by
    /// it), and each device's losses already said.
    epoch: Option<std::time::Instant>,
    said_lost: Vec<u32>,
    /// The clock as the tests set it (ms), in place of the wall's.
    pub clock_override: Option<u64>,
    /// The hands' gesture (ADR 0095 §2): every fader and V-pot moved from the first touch to
    /// the last release is one undo step, however many are ridden at once.
    gesture: Gesture,
}

/// The surfaces' gesture: the targets moved (each with the latest command that sets it), the
/// step's key, when it last moved.
#[derive(Default)]
struct Gesture {
    serial: u64,
    moves: Vec<(String, Command)>,
    key: Option<MergeKey>,
    last_ms: u64,
}

/// The history name of a gesture that moved more than one target.
pub const GESTURE_LABEL: &str = "cmd.external.surface_move";

/// A move of `target` by the surfaces: into the gesture (one undo step for everything the
/// hands move between the first touch and the last release). A target the recording mode's
/// lock or a frozen sound refuses is skipped quietly — the others keep moving. Returns
/// whether it moved.
pub fn gesture_move(app: &mut CantusApp, target: String, cmd: Command) -> bool {
    if app.bus.locked_against(&cmd) || cantus_core::freeze::refuses(app.project(), &cmd).is_some() {
        return false;
    }
    let now = app.surfaces.now_ms();
    let g = &mut app.surfaces.gesture;
    if g.moves.is_empty() {
        g.serial += 1;
    }
    let mut grew = false;
    match g.moves.iter_mut().find(|(t, _)| *t == target) {
        Some(m) => m.1 = cmd,
        None => {
            g.moves.push((target, cmd));
            grew = true;
        }
    }
    let ids: Vec<&String> = g.moves.iter().map(|(t, _)| t).collect();
    let key = MergeKey::of(&("surface", g.serial, ids));
    let command = if g.moves.len() == 1 {
        g.moves[0].1.clone()
    } else {
        Command::Batch {
            label_key: GESTURE_LABEL.into(),
            commands: g.moves.iter().map(|(_, c)| c.clone()).collect(),
        }
    };
    let old = g.key.replace(key);
    g.last_ms = now;
    let result = match old {
        Some(from) if grew => app.bus.widen_merging(command, Origin::User, from, key),
        _ => app.bus.execute_merging(command, Origin::User, key),
    };
    match result {
        Ok(()) => true,
        Err(e) => {
            gesture_end(app);
            app.set_error(e.to_string());
            false
        }
    }
}

/// The gesture ends: its step is sealed (only its own — never a mouse drag going on).
pub fn gesture_end(app: &mut CantusApp) {
    let g = &mut app.surfaces.gesture;
    if let Some(k) = g.key.take() {
        app.bus.end_gesture_if(k);
    }
    g.moves.clear();
}

impl std::fmt::Debug for SurfaceService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceService")
            .field("devices", &self.choices.len())
            .finish()
    }
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl SurfaceService {
    /// Milliseconds on the service's clock.
    pub fn now_ms(&mut self) -> u64 {
        if let Some(ms) = self.clock_override {
            return ms;
        }
        self.epoch
            .get_or_insert_with(std::time::Instant::now)
            .elapsed()
            .as_millis() as u64
    }

    fn note(&mut self, line: String) {
        if self.log.len() >= LOG {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }

    /// The choices applied: the set is rebuilt when they changed.
    pub fn configure(&mut self, choices: &[SurfaceChoice]) -> bool {
        let live: Vec<SurfaceChoice> = choices.iter().filter(|c| c.enabled).cloned().collect();
        if live == self.choices {
            return false;
        }
        let kinds: Vec<Kind> = live
            .iter()
            .map(|c| Kind::from_id(&c.kind).unwrap_or(Kind::Mcu))
            .collect();
        self.units = live
            .iter()
            .zip(&kinds)
            .map(|(c, k)| c.virtual_unit.then(|| Unit::new(*k)))
            .collect();
        let bank = self.set.as_ref().map_or(0, |s| s.bank);
        let mut set = SurfaceSet::new(&kinds);
        set.bank = bank;
        for (d, c) in set.devices.iter_mut().zip(&live) {
            d.touch_sensing = c.touch_sensing;
        }
        self.set = (!kinds.is_empty()).then_some(set);
        self.choices = live;
        self.held = (false, false);
        true
    }

    /// The device bound to the input port `name`.
    fn device_of(&self, name: &str) -> Option<usize> {
        self.choices
            .iter()
            .position(|c| !c.virtual_unit && !c.input.is_empty() && c.input == name)
    }

    /// The enabled surface choices, in device order.
    pub fn choices(&self) -> &[SurfaceChoice] {
        &self.choices
    }
}

/// The input ports (indices into the open ports) the surfaces take whole.
pub fn taken_ports(app: &CantusApp) -> Vec<usize> {
    app.midi_port_names
        .iter()
        .enumerate()
        .filter(|(_, n)| app.surfaces.device_of(n).is_some())
        .map(|(i, _)| i)
        .collect()
}

/// A message of a surface's port: consumed here (`true`) so the generic map never sees it.
pub fn heard(app: &mut CantusApp, h: &Heard) -> bool {
    let Some(name) = app.midi_port_names.get(usize::from(h.port)).cloned() else {
        return false;
    };
    let Some(dev) = app.surfaces.device_of(&name) else {
        return false;
    };
    let two = matches!(h.event.status & 0xF0, 0xC0 | 0xD0);
    let bytes: Vec<u8> = if two {
        vec![h.event.status, h.event.data1]
    } else {
        vec![h.event.status, h.event.data1, h.event.data2]
    };
    feed(app, dev, &bytes);
    true
}

/// Bytes from device `dev` (a port's, or a virtual unit's): into the set, the actions applied.
pub fn feed(app: &mut CantusApp, dev: usize, bytes: &[u8]) {
    let Some(mut set) = app.surfaces.set.take() else {
        return;
    };
    let v = view::surface_view(app);
    app.surfaces.received += 1;
    app.surfaces.note(format!("in  {}", hex(bytes)));
    set.now_ms = set.now_ms.max(app.surfaces.now_ms());
    let input = set.input(dev, bytes, &v);
    for (zone, port) in &input.unmapped {
        app.surfaces
            .note(format!("unmapped HUI zone {zone:#04x} port {port}"));
    }
    for r in &input.reply {
        app.surfaces.note(format!("out {}", hex(r)));
        write(app, dev, r, &mut set);
    }
    for a in &input.actions {
        match a {
            SurfaceAction::Rewind(d) => app.surfaces.held.0 = *d,
            SurfaceAction::Forward(d) => app.surfaces.held.1 = *d,
            // The last fader let go: the gesture's one step is sealed.
            SurfaceAction::Touch { on: false, .. } if !set.any_touched() => gesture_end(app),
            _ => {
                apply::apply(app, a);
            }
        }
    }
    app.surfaces.set = Some(set);
}

/// Write `bytes` to device `dev`: its output port, or its virtual unit (whose answer comes
/// straight back).
fn write(app: &mut CantusApp, dev: usize, bytes: &[u8], set: &mut SurfaceSet) {
    app.surfaces.sent += 1;
    let choice = app.surfaces.choices.get(dev).cloned();
    let Some(choice) = choice else {
        return;
    };
    if choice.virtual_unit {
        let replies = app
            .surfaces
            .units
            .get_mut(dev)
            .and_then(|u| u.as_mut())
            .map(|u| u.receive(bytes))
            .unwrap_or_default();
        for r in replies {
            let v = view::surface_view(app);
            let input = set.input(dev, &r, &v);
            for a in &input.actions {
                apply::apply(app, a);
            }
        }
        return;
    }
    let name = if choice.output.is_empty() {
        cantus_engine::midi::output_name_for(&choice.input).unwrap_or_default()
    } else {
        choice.output.clone()
    };
    if name.is_empty() {
        return;
    }
    if let Some(port) = app.sync.pool().get(&name) {
        if let Ok(mut p) = port.lock() {
            p.write(bytes);
        }
    }
}

/// Each frame: apply the choices, repeat a held rewind/forward, render what changed.
pub fn tick(app: &mut CantusApp) {
    let choices = app.settings.surfaces.clone();
    if app.surfaces.configure(&choices) {
        // The ports a surface takes changed: the filter is made again.
        app.controllers.published = None;
        if let Some(s) = app.surfaces.set.as_mut() {
            s.refresh();
        }
    }
    let Some(mut set) = app.surfaces.set.take() else {
        return;
    };
    let (rew, fwd) = app.surfaces.held;
    if rew {
        apply::move_playhead(app, -apply::SCAN_TICKS);
    }
    if fwd {
        apply::move_playhead(app, apply::SCAN_TICKS);
    }
    let v = view::surface_view(app);
    let now = app.surfaces.now_ms();
    // Moves with no touch held (a V-pot, a fader without touch sensing) end their gesture
    // when the hands have been still a moment.
    let g = &app.surfaces.gesture;
    if g.key.is_some()
        && !set.any_touched()
        && now >= g.last_ms + cantus_core::external_surface::NO_TOUCH_HOLD_MS
    {
        gesture_end(app);
    }
    for (dev, bytes) in set.output_at(&v, now) {
        app.surfaces.note(format!("out {}", hex(&bytes)));
        write(app, dev, &bytes, &mut set);
    }
    // A unit that stopped answering its ping is said once per loss.
    app.surfaces.said_lost.resize(set.devices.len(), 0);
    for (i, d) in set.devices.iter().enumerate() {
        if d.lost > app.surfaces.said_lost[i] {
            app.surfaces.said_lost[i] = d.lost;
            app.surfaces
                .note(format!("surface {} lost: no answer to the ping", i + 1));
        }
    }
    app.surfaces.set = Some(set);
}

/// SysEx from the surfaces' ports (a Mackie's host connection query): to its device.
pub fn sysex(app: &mut CantusApp, port: usize, bytes: &[u8]) {
    let Some(name) = app.midi_port_names.get(port).cloned() else {
        return;
    };
    if let Some(dev) = app.surfaces.device_of(&name) {
        feed(app, dev, bytes);
    }
}

/// The kinds a Settings page offers, with the words each is shown with.
pub fn kinds() -> Vec<(String, &'static str)> {
    vec![
        ("mcu".into(), "external.surface.mcu"),
        ("mcu-xt1".into(), "external.surface.xt1"),
        ("mcu-xt2".into(), "external.surface.xt2"),
        ("hui".into(), "external.surface.hui"),
    ]
}
