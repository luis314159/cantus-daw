//! Controller grids in the interface (round 33, ADR 0077): each port a grid answers on —
//! found by its name (`GridDevice::detect`) or chosen in Settings ▸ MIDI or the
//! controllers pane — is put in its programmer mode, its messages are taken from the
//! engine (they launch, they do not play notes), and its lights mirror the session inside
//! its *session ring*: pads in their clip's colour, green pulsing while playing, flashing
//! while queued, red recording; scene buttons; stop buttons; the arrows lit where the ring
//! can move. Only what changed is sent, once a frame, through the feedback outputs.

use crate::app::CantusApp;
use cantus_core::controller::grid::{
    arrow_lights, GridDevice, GridFrame, GridInput, Light, Rgb, Ring, GRID_COLS,
};
use cantus_core::session::SlotRef;
use cantus_engine::messages::{ScenePhase, SlotPhase};
use cantus_engine::midi::Heard;
use serde::{Deserialize, Serialize};

/// The user's choice for a port (`Settings::grids`): a device, or off. A port with no
/// choice is a grid when its name says it is one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridChoice {
    pub port: String,
    #[serde(default)]
    pub device: Option<GridDevice>,
}

/// A port driven as a grid.
#[derive(Clone, Debug, PartialEq)]
pub struct GridPort {
    /// Its index among the open inputs, and its name.
    pub index: usize,
    pub name: String,
    pub device: GridDevice,
    pub ring: Ring,
    /// What its lights show (`None`: unknown — everything is sent).
    pub shown: Option<GridFrame>,
    pub shift: bool,
    /// A Mini's or an X's bottom row lent to the stop buttons (a toggle); a Pro's Stop Clip
    /// held.
    pub stop_mode: bool,
    /// The pads and scene buttons held, with what their press launched: a release lets go
    /// of that, wherever the ring has moved since.
    pub held_pads: Vec<((u8, u8), SlotRef)>,
    pub held_scenes: Vec<(u8, cantus_core::session::SceneId)>,
}

/// Every grid, and the ports they were found among.
#[derive(Clone, Debug, Default)]
pub struct Grids {
    pub ports: Vec<GridPort>,
    found_for: Option<(Vec<String>, Vec<GridChoice>)>,
}

/// The device a port named `name` is driven as: the user's choice, else what its name
/// says.
pub fn device_for(choices: &[GridChoice], name: &str) -> Option<GridDevice> {
    let clean = cantus_core::controller::clean_port_name(name);
    match choices.iter().find(|c| c.port == clean) {
        Some(c) => c.device,
        None => GridDevice::detect(name),
    }
}

/// Send `bytes` to the output answering input `name` (or the tests' log).
fn send(app: &mut CantusApp, name: &str, bytes: &[u8]) {
    match app.controllers.feedback_log.as_mut() {
        Some(log) => log.push((name.to_string(), bytes.to_vec())),
        None => {
            let _ = app.controllers.outputs.send(name, bytes);
        }
    }
}

/// The ports as they are now: new grids greeted, gone ones forgotten.
pub(crate) fn find(app: &mut CantusApp) {
    // Nothing to do while the ports and the choices are as they were (nothing cloned: an
    // idle frame allocates nothing).
    if app
        .controllers
        .grids
        .found_for
        .as_ref()
        .is_some_and(|(n, c)| *n == app.midi_port_names && *c == app.settings.grids)
    {
        return;
    }
    let names = app.midi_port_names.clone();
    let choices = app.settings.grids.clone();
    let key = (names.clone(), choices.clone());
    let old = std::mem::take(&mut app.controllers.grids.ports);
    let mut ports = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let Some(device) = device_for(&choices, name) else {
            continue;
        };
        let kept = old
            .iter()
            .find(|p| p.name == *name && p.device == device)
            .cloned();
        let mut p = kept.unwrap_or(GridPort {
            index,
            name: name.clone(),
            device,
            ring: Ring::default(),
            shown: None,
            shift: false,
            stop_mode: false,
            held_pads: Vec::new(),
            held_scenes: Vec::new(),
        });
        if p.shown.is_none() {
            for m in device.hello() {
                send(app, name, &m);
            }
        }
        p.index = index;
        ports.push(p);
    }
    // A port given back (turned off, or another device chosen): its own mode again.
    for p in &old {
        if !ports
            .iter()
            .any(|q| q.name == p.name && q.device == p.device)
            && names.contains(&p.name)
        {
            for m in p.device.goodbye() {
                send(app, &p.name, &m);
            }
        }
    }
    app.controllers.grids.ports = ports;
    app.controllers.grids.found_for = Some(key);
    app.controllers.published = None;
}

/// The ring's view on a device: its columns and the rows that hold scenes.
fn view_of(p: &GridPort) -> (usize, usize) {
    let rows = p.device.rows();
    let rows = if p.stop_mode && !p.device.has_stop_row() {
        rows - 1
    } else {
        rows
    };
    (GRID_COLS, rows)
}

fn size_of(app: &CantusApp) -> (usize, usize) {
    (
        app.project().tracks.len(),
        app.project().session.scenes.len(),
    )
}

/// What a grid shows now.
pub fn picture(app: &CantusApp, p: &GridPort) -> GridFrame {
    let mut f = GridFrame::dark();
    let pr = app.project();
    let live = &app.session_view.live;
    let (cols, rows) = view_of(p);
    let dim_white = Rgb::WHITE.dim(0.25);
    for row in 0..rows {
        let scene = pr.session.scenes.get(p.ring.scene + row);
        for col in 0..cols {
            let Some(track) = pr.tracks.get(p.ring.track + col) else {
                continue;
            };
            let Some(scene) = scene else { continue };
            let slot = SlotRef::new(track.id, scene.id);
            let phase = live.phase(slot);
            f.pads[row][col] = match (pr.slot_clip(slot), phase) {
                (_, SlotPhase::Recording { .. }) => Light::Pulse(Rgb::RED),
                (_, SlotPhase::RecordQueued { .. }) => Light::Flash(Rgb::RED),
                (Some(_), SlotPhase::Playing { .. }) => Light::Pulse(Rgb::GREEN),
                (Some(_), SlotPhase::Queued { .. }) => Light::Flash(Rgb::GREEN),
                (Some(c), SlotPhase::Stopping { .. }) => {
                    Light::Flash(rgb(c.clip.color.unwrap_or(track.color)))
                }
                (Some(c), _) => Light::On(rgb(c.clip.color.unwrap_or(track.color))),
                (None, _) if track.armed => Light::On(Rgb::RED.dim(0.15)),
                (None, _) => Light::Off,
            };
        }
        if let Some(scene) = scene {
            f.scenes[row] = match live.scene_phase(scene.id) {
                ScenePhase::Playing { .. } => Light::Pulse(Rgb::GREEN),
                ScenePhase::Queued { .. } => Light::Flash(Rgb::GREEN),
                ScenePhase::Idle => Light::On(scene.color.map_or(dim_white, rgb)),
            };
        }
    }
    let pro = p.device == GridDevice::LaunchpadProMk3;
    for col in 0..cols {
        let Some(track) = pr.tracks.get(p.ring.track + col) else {
            continue;
        };
        let stop = match live.playing_on(track.id) {
            Some((_, SlotPhase::Stopping { .. })) => Light::Flash(Rgb::RED),
            Some(_) => Light::On(Rgb::RED),
            None => Light::On(Rgb::RED.dim(0.15)),
        };
        // The Pro's track select row selects tracks (in their colours, the chosen one
        // bright) unless Stop Clip is held, when it is the stops.
        f.stops[col] = if pro && !p.stop_mode {
            let bright = if app.selected_track == Some(track.id) {
                1.0
            } else {
                0.3
            };
            Light::On(rgb(track.color).dim(bright))
        } else {
            stop
        };
        if p.stop_mode && !p.device.has_stop_row() {
            f.pads[7][col] = stop;
        }
    }
    // The Pro's Stop Clip lights while held (Shift with it stops all); a Mini's or an X's
    // Session button is stop all, lit while anything plays.
    f.stop_all = if pro {
        if p.stop_mode {
            Light::On(Rgb::RED)
        } else {
            Light::On(Rgb::RED.dim(0.15))
        }
    } else if live.busy() {
        Light::On(Rgb::RED)
    } else {
        Light::On(Rgb::RED.dim(0.15))
    };
    f.stop_mode = if p.stop_mode {
        Light::On(Rgb::RED)
    } else {
        Light::On(Rgb::RED.dim(0.15))
    };
    f.arrows = arrow_lights(p.ring, size_of(app), (cols, rows), Rgb::WHITE.dim(0.6));
    f
}

fn rgb(c: cantus_core::Color) -> Rgb {
    let c = crate::theme::track_color(c);
    Rgb(c.r(), c.g(), c.b())
}

/// Every frame: the ports found, every grid's lights brought up to date, the rings shown.
pub fn frame(app: &mut CantusApp) {
    find(app);
    if app.controllers.grids.ports.is_empty() {
        app.session_view.rings.clear();
        return;
    }
    let size = size_of(app);
    let mut ports = std::mem::take(&mut app.controllers.grids.ports);
    let mut rings = Vec::with_capacity(ports.len());
    for p in &mut ports {
        // A session that shrank keeps the ring inside it.
        let view = view_of(p);
        p.ring = Ring {
            scene: p.ring.scene.min(size.1.saturating_sub(view.1)),
            track: p.ring.track.min(size.0.saturating_sub(view.0)),
        };
        let next = picture(app, p);
        let msgs = p.device.lights(p.shown.as_ref(), &next);
        for m in &msgs {
            send(app, &p.name, m);
        }
        p.shown = Some(next);
        rings.push((
            p.ring.track,
            p.ring.scene,
            view.0.min(size.0),
            view.1.min(size.1),
        ));
    }
    app.controllers.grids.ports = ports;
    app.session_view.rings = rings;
}

/// A message from port `h.port`: `true` when a grid took it (its press acted on the
/// session; nothing else reads it).
pub fn heard(app: &mut CantusApp, h: &Heard) -> bool {
    let Some(i) = app
        .controllers
        .grids
        .ports
        .iter()
        .position(|p| p.index == usize::from(h.port))
    else {
        return false;
    };
    let (device, shift) = {
        let p = &app.controllers.grids.ports[i];
        (p.device, p.shift)
    };
    // What is no part of the grid (an APC's faders and knobs, a button Cantus leaves
    // alone) goes on to MIDI learn and the mappings — never to the note path: the port
    // stays taken from the engine.
    let Some(input) = device.decode(h.event.status, h.event.data1, h.event.data2, shift) else {
        // A pad's aftertouch stays the grid's (pressing a pad during learn learns nothing).
        return !GridDevice::passes_on(h.event.status);
    };
    act(app, i, input);
    true
}

/// What a grid's press does.
pub fn act(app: &mut CantusApp, i: usize, input: GridInput) {
    let size = size_of(app);
    let p = app.controllers.grids.ports[i].clone();
    let (cols, rows) = view_of(&p);
    let track_at = |app: &CantusApp, col: u8| {
        app.project()
            .tracks
            .get(p.ring.track + usize::from(col))
            .map(|t| t.id)
    };
    let scene_at = |app: &CantusApp, row: u8| {
        app.project()
            .session
            .scenes
            .get(p.ring.scene + usize::from(row))
            .map(|s| s.id)
    };
    match input {
        // A pad let go: what its press launched, wherever the ring is now.
        GridInput::Pad {
            col,
            row,
            velocity: 0,
        } => {
            let held = &mut app.controllers.grids.ports[i].held_pads;
            if let Some(k) = held.iter().position(|(k, _)| *k == (col, row)) {
                let (_, slot) = held.remove(k);
                crate::views::session::release(app, slot);
            }
        }
        GridInput::Pad { col, row, velocity } => {
            if p.stop_mode && !p.device.has_stop_row() && usize::from(row) == 7 {
                if let (true, Some(t)) = (velocity > 0, track_at(app, col)) {
                    crate::views::session::stop_track(app, t);
                }
                return;
            }
            if usize::from(row) >= rows || usize::from(col) >= cols {
                return;
            }
            let (Some(t), Some(s)) = (track_at(app, col), scene_at(app, row)) else {
                return;
            };
            if velocity > 0 {
                let slot = SlotRef::new(t, s);
                let held = &mut app.controllers.grids.ports[i].held_pads;
                held.retain(|(k, _)| *k != (col, row));
                held.push(((col, row), slot));
                crate::views::session::press_slot(app, slot, f32::from(velocity) / 127.0);
            }
        }
        GridInput::Scene { row, pressed } => {
            if usize::from(row) >= rows {
                return;
            }
            if pressed {
                if let Some(s) = scene_at(app, row) {
                    let held = &mut app.controllers.grids.ports[i].held_scenes;
                    held.retain(|(r, _)| *r != row);
                    held.push((row, s));
                    crate::views::session::launch_scene(app, s, 1.0);
                }
            } else {
                let held = &mut app.controllers.grids.ports[i].held_scenes;
                if let Some(k) = held.iter().position(|(r, _)| *r == row) {
                    let (_, s) = held.remove(k);
                    crate::views::session::release_scene(app, s);
                }
            }
        }
        GridInput::Stop { col, pressed } => {
            if let (true, Some(t)) = (pressed, track_at(app, col)) {
                crate::views::session::stop_track(app, t);
            }
        }
        GridInput::StopAll { pressed } => {
            if pressed {
                crate::views::session::stop_all(app);
            }
        }
        GridInput::Arrow { arrow, pressed } => {
            if pressed {
                let q = &mut app.controllers.grids.ports[i];
                q.ring = q.ring.moved(arrow, q.shift, size, (cols, rows));
            }
        }
        GridInput::Shift { pressed } => app.controllers.grids.ports[i].shift = pressed,
        // A Pro's Stop Clip is held; a Mini's or an X's lowest scene button toggles.
        GridInput::StopMode { pressed } => {
            let q = &mut app.controllers.grids.ports[i];
            if q.device.has_stop_row() {
                q.stop_mode = pressed;
            } else if pressed {
                q.stop_mode = !q.stop_mode;
            }
        }
        // A Pro's track select row: the track's stop while Stop Clip is held, else the track
        // chosen.
        GridInput::TrackSelect { col, pressed } => {
            if let (true, Some(t)) = (pressed, track_at(app, col)) {
                if p.stop_mode {
                    crate::views::session::stop_track(app, t);
                } else {
                    app.select_track(t);
                }
            }
        }
    }
}

/// The ports the grids take whole (their notes and controllers never reach the engine).
pub fn taken_ports(app: &CantusApp) -> Vec<usize> {
    app.controllers
        .grids
        .ports
        .iter()
        .map(|p| p.index)
        .collect()
}

/// Let every grid go (the application closes): its own mode again.
pub fn release_all(app: &mut CantusApp) {
    let ports = std::mem::take(&mut app.controllers.grids.ports);
    for p in &ports {
        for m in p.device.goodbye() {
            send(app, &p.name, &m);
        }
    }
    app.controllers.grids.found_for = None;
}

/// Turn a port's grid on (as `device`) or off: the user's choice, kept in the settings.
pub fn choose(app: &mut CantusApp, port: &str, device: Option<GridDevice>) {
    let clean = cantus_core::controller::clean_port_name(port).into_owned();
    app.settings.grids.retain(|c| c.port != clean);
    if device != GridDevice::detect(port) {
        app.settings.grids.push(GridChoice {
            port: clean,
            device,
        });
    }
    app.store_settings();
}

/// The grids' rows (the controllers pane, Settings ▸ MIDI): every input port with the
/// device it is driven as — off, the one its name says, or one chosen.
pub fn rows(app: &mut CantusApp, ui: &mut egui::Ui, w: f32) {
    use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
    use crate::kit::{band, with_brush, State, Style};
    use egui::{pos2, vec2, Align2, Rect, Sense};
    let s = Style::current();
    let k = s.k;
    let (br, _) = ui.allocate_exact_size(vec2(w, s.d(band::BAND)), Sense::hover());
    with_brush(ui.painter(), |b| {
        band::section(
            b,
            br,
            &s,
            State::REST,
            Src::Key("session.grid.title"),
            None,
            false,
        )
    });
    let names = app.midi_port_names.clone();
    if names.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(w, 24.0 * k), Sense::hover());
        with_brush(ui.painter(), |b| {
            let tx = Text::key("session.grid.no_ports", Role::Body, s.t.text_dim)
                .align(Align2::LEFT_CENTER);
            let _ = text::paint(b, r.shrink2(vec2(10.0 * k, 0.0)), &tx, s.tk());
        });
        return;
    }
    for name in names {
        let (r, _) = ui.allocate_exact_size(vec2(w, 26.0 * k), Sense::hover());
        let field_w = (200.0 * k).min(r.width() * 0.45);
        let field = Rect::from_min_max(
            pos2(r.right() - field_w - 8.0 * k, r.top() + 3.0 * k),
            pos2(r.right() - 8.0 * k, r.bottom() - 3.0 * k),
        );
        let label = Rect::from_min_max(
            pos2(r.left() + 10.0 * k, r.top()),
            pos2(field.left() - 8.0 * k, r.bottom()),
        );
        with_brush(ui.painter(), |b| {
            let tx = Text::new(&name, Role::Body, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle));
            let _ = text::paint(b, label, &tx, s.tk());
        });
        choice_field(app, ui, &name, field);
    }
}

/// The heading's key of the grid column in a per-port table (Settings ▸ MIDI).
pub const COLUMN_KEY: &str = "session.grid.column";

/// One port's grid choice as a cell of a per-port table, `width` points wide (Settings ▸
/// MIDI's inputs table, at sync's marked place).
pub fn cell(app: &mut CantusApp, ui: &mut egui::Ui, port: &str, width: f32) -> egui::Rect {
    let s = crate::kit::Style::current();
    let (r, _) = ui.allocate_exact_size(egui::vec2(width * s.k, 22.0 * s.k), egui::Sense::hover());
    choice_field(app, ui, port, r);
    r
}

/// How wide [`cell`] must be for every value it can show to read whole, points: the
/// longest device name (or Off) and the menu field's chevron and margins.
pub fn cell_width(ui: &egui::Ui) -> f32 {
    use crate::kit::text::{self, Role, Text};
    let s = crate::kit::Style::current();
    let off = crate::i18n::t("session.grid.off");
    GridDevice::ALL
        .iter()
        .map(|d| d.name())
        .chain([off.as_str()])
        .map(|w| text::measure(ui.ctx(), &Text::new(w, Role::Control, s.t.text), s.tk()).x / s.k)
        .fold(0.0, f32::max)
        + 32.0
}

/// The menu field of a port's grid choice in `field`: off, the device its name says, or
/// one chosen.
fn choice_field(app: &mut CantusApp, ui: &mut egui::Ui, name: &str, field: egui::Rect) {
    use crate::i18n::{t, tf};
    let s = crate::kit::Style::current();
    let found = GridDevice::detect(name);
    let now = device_for(&app.settings.grids, name);
    let mut options: Vec<(Option<GridDevice>, String)> = vec![(None, t("session.grid.off"))];
    for d in GridDevice::ALL {
        let words = if Some(d) == found {
            tf("session.grid.auto", &[("device", &d.name())])
        } else {
            d.name().to_string()
        };
        options.push((Some(d), words));
    }
    let at = options.iter().position(|(d, _)| *d == now);
    // The field shows the device's name alone (its menu says which one was found): the
    // cell is as wide as the longest name, not a name and a note.
    let value = match now {
        Some(d) => d.name().to_string(),
        None => t("session.grid.off"),
    };
    let words: Vec<String> = options.iter().map(|(_, w)| w.clone()).collect();
    let id = egui::Id::new(("cantus-grid-choice", name));
    if let Some(i) = crate::views::session::clip::menu(
        ui,
        &s,
        field,
        id,
        &value,
        &words,
        at,
        "session.grid.tip",
        false,
    ) {
        choose(app, name, options[i].0);
    }
}
