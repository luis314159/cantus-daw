//! Each grid's own bytes (ADR 0077): which port, which mode, what its messages mean and how
//! its lights are lit. See the module docs of `controller_grid` for the references.

use super::{lit, Arrow, GridDevice, GridFrame, GridInput, Light, GRID_COLS, GRID_ROWS};

/// The Launchpads' SysEx header, before the device's id (Novation, `02`).
const NOVATION: [u8; 5] = [0xF0, 0x00, 0x20, 0x29, 0x02];

fn novation_id(d: GridDevice) -> u8 {
    match d {
        GridDevice::LaunchpadMiniMk3 => 0x0D,
        GridDevice::LaunchpadX => 0x0C,
        _ => 0x0E,
    }
}

fn is_launchpad(d: GridDevice) -> bool {
    matches!(
        d,
        GridDevice::LaunchpadMiniMk3 | GridDevice::LaunchpadX | GridDevice::LaunchpadProMk3
    )
}

pub(super) fn detect(name: &str) -> Option<GridDevice> {
    let lower = name.to_ascii_lowercase();
    // Windows names a device's second and third interfaces `MIDIIN2 (…)`, `MIDIIN3 (…)`.
    let later = lower.contains("midiin") || lower.contains("midiout");
    // A Launchpad Mini's or X's programmer mode talks on its *second* interface (the first
    // is the DAW's): `MIDIIN2 (LPMiniMK3 MIDI)` on Windows, `… LPMiniMK3 MIDI …` on macOS,
    // `… LPMiniMK3 MI …` on Linux — never a name that starts with the token (Windows's
    // first interface) or carries `DA`/`DAW`.
    let second = |token: &str| {
        let t = token.to_ascii_lowercase();
        lower.contains(&t)
            && !lower.contains(&format!("{t} da"))
            && (lower.contains("midiin2")
                || lower.contains("midiout2")
                || (!later && !lower.starts_with(&t) && lower.contains(&format!("{t} mi"))))
    };
    if second("LPMiniMK3") {
        return Some(GridDevice::LaunchpadMiniMk3);
    }
    if second("LPX") {
        return Some(GridDevice::LaunchpadX);
    }
    // The Pro's programmer mode is on its *first* interface (`LPProMK3 MIDI`); its second
    // is the DIN port, its third the DAW's.
    if lower.contains("lppromk3 mi") && !later {
        return Some(GridDevice::LaunchpadProMk3);
    }
    if lower.contains("apc mini mk2") && !later && !lower.contains("notes") {
        return Some(GridDevice::ApcMiniMk2);
    }
    if lower.contains("apc40 mkii") && !later {
        return Some(GridDevice::Apc40Mk2);
    }
    None
}

fn sysex(head: &[u8], body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(head.len() + body.len() + 1);
    v.extend_from_slice(head);
    v.extend_from_slice(body);
    v.push(0xF7);
    v
}

fn novation(d: GridDevice, body: &[u8]) -> Vec<u8> {
    let mut head = NOVATION.to_vec();
    head.push(novation_id(d));
    sysex(&head, body)
}

/// Akai's introduction message: `F0 47 7F <model> 60 00 04 <mode> 00 00 00 F7`.
fn akai_intro(model: u8, mode: u8) -> Vec<u8> {
    sysex(
        &[0xF0, 0x47, 0x7F, model],
        &[0x60, 0x00, 0x04, mode, 0x00, 0x00, 0x00],
    )
}

const APC_MINI: u8 = 0x4F;
const APC40: u8 = 0x29;

pub(super) fn hello(d: GridDevice) -> Vec<Vec<u8>> {
    match d {
        // Programmer / Live mode switch: `0Eh 01h`.
        GridDevice::LaunchpadMiniMk3 | GridDevice::LaunchpadX => vec![novation(d, &[0x0E, 0x01])],
        // Select layout 11h (Programmer mode), page 0.
        GridDevice::LaunchpadProMk3 => vec![novation(d, &[0x00, 0x11, 0x00, 0x00])],
        GridDevice::ApcMiniMk2 => vec![akai_intro(APC_MINI, 0x00)],
        // Mode 41h: the host lights every LED (Ableton Live mode).
        GridDevice::Apc40Mk2 => vec![akai_intro(APC40, 0x41)],
    }
}

pub(super) fn goodbye(d: GridDevice) -> Vec<Vec<u8>> {
    let mut out = lights(d, None, &GridFrame::dark());
    match d {
        GridDevice::LaunchpadMiniMk3 | GridDevice::LaunchpadX => {
            out.push(novation(d, &[0x0E, 0x00]))
        }
        // Back to the Note layout (04h): Session needs the DAW's mode.
        GridDevice::LaunchpadProMk3 => out.push(novation(d, &[0x00, 0x04, 0x00, 0x00])),
        GridDevice::ApcMiniMk2 => {}
        // Mode 40h: generic, the device's own.
        GridDevice::Apc40Mk2 => out.push(akai_intro(APC40, 0x40)),
    }
    out
}

// ---- decoding ----------------------------------------------------------------------------

pub(super) fn decode(d: GridDevice, status: u8, d1: u8, d2: u8, shift: bool) -> Option<GridInput> {
    let kind = status & 0xF0;
    let ch = status & 0x0F;
    let (note, cc) = match kind {
        0x90 => (Some((d1, d2)), None),
        0x80 => (Some((d1, 0)), None),
        0xB0 => (None, Some((d1, d2))),
        _ => return None,
    };
    if is_launchpad(d) {
        launchpad_in(d, note, cc, shift)
    } else if d == GridDevice::ApcMiniMk2 {
        let (n, v) = note?;
        apc_mini_in(n, v > 0, shift)
    } else {
        let (n, v) = note?;
        apc40_in(n, ch, v > 0)
    }
}

/// A Launchpad in programmer mode: pads send notes 11–88 (row 1 at the bottom), every
/// button a CC (127 pressed, 0 let go).
fn launchpad_in(
    d: GridDevice,
    note: Option<(u8, u8)>,
    cc: Option<(u8, u8)>,
    shift: bool,
) -> Option<GridInput> {
    if let Some((n, v)) = note {
        let (row, col) = (n / 10, n % 10);
        if (1..=8).contains(&row) && (1..=8).contains(&col) {
            return Some(GridInput::Pad {
                col: col - 1,
                row: 8 - row,
                velocity: v,
            });
        }
        return None;
    }
    let (n, v) = cc?;
    let pressed = v > 0;
    // The right column: CC 89 (top) … 19 (bottom).
    if n % 10 == 9 && (1..=8).contains(&(n / 10)) {
        let row = 8 - n / 10;
        if !d_has_stop_row(d) && row == 7 {
            return Some(GridInput::StopMode { pressed });
        }
        return Some(GridInput::Scene { row, pressed });
    }
    let arrow = |a| Some(GridInput::Arrow { arrow: a, pressed });
    match (d, n) {
        (GridDevice::LaunchpadProMk3, 80) => arrow(Arrow::Up),
        (GridDevice::LaunchpadProMk3, 70) => arrow(Arrow::Down),
        (GridDevice::LaunchpadProMk3, 91) => arrow(Arrow::Left),
        (GridDevice::LaunchpadProMk3, 92) => arrow(Arrow::Right),
        (GridDevice::LaunchpadProMk3, 90) => Some(GridInput::Shift { pressed }),
        // The reference's *track control* row (CC 1–8): its last, *Stop Clip*, held makes
        // the *track select* row (CC 101–108, under the grid) the tracks' stops, as
        // Ableton's script does; with Shift it stops everything. The others (record arm,
        // mute, solo, volume, pan, sends, device) are no part of the grid.
        (GridDevice::LaunchpadProMk3, 8) if shift => Some(GridInput::StopAll { pressed }),
        (GridDevice::LaunchpadProMk3, 8) => Some(GridInput::StopMode { pressed }),
        (GridDevice::LaunchpadProMk3, 101..=108) => Some(GridInput::TrackSelect {
            col: n - 101,
            pressed,
        }),
        // Its other buttons (Session, Note, Chord, Custom, Sequencer, Projects, the track
        // control row's first seven, the left column's) are no part of the grid.
        (GridDevice::LaunchpadProMk3, _) => None,
        (_, 91) => arrow(Arrow::Up),
        (_, 92) => arrow(Arrow::Down),
        (_, 93) => arrow(Arrow::Left),
        (_, 94) => arrow(Arrow::Right),
        // The Mini's and the X's Session button: stop all.
        (_, 95) => Some(GridInput::StopAll { pressed }),
        _ => None,
    }
}

fn d_has_stop_row(d: GridDevice) -> bool {
    d.has_stop_row()
}

/// The APC mini mk2: pads on notes 0–63 (0 at the bottom left), the track buttons on
/// 100–107, the scene buttons on 112–119, Shift on 122. Shift with track buttons 1–4 moves
/// the ring (up, down, left, right), Shift with the last scene button stops everything (as
/// Ableton's script does).
fn apc_mini_in(n: u8, pressed: bool, shift: bool) -> Option<GridInput> {
    match n {
        0..=63 => Some(GridInput::Pad {
            col: n % 8,
            row: 7 - n / 8,
            velocity: if pressed { 127 } else { 0 },
        }),
        100..=103 if shift => Some(GridInput::Arrow {
            arrow: Arrow::ALL[usize::from(n - 100)],
            pressed,
        }),
        // Shift's other layers (Ableton's track modes) are no part of the grid.
        104..=107 | 112..=118 if shift => None,
        100..=107 => Some(GridInput::Stop {
            col: n - 100,
            pressed,
        }),
        119 if shift => Some(GridInput::StopAll { pressed }),
        112..=119 => Some(GridInput::Scene {
            row: n - 112,
            pressed,
        }),
        122 => Some(GridInput::Shift { pressed }),
        _ => None,
    }
}

/// The APC40 mkII in Ableton mode: clip pads on notes 0–39 (0x20–0x27 the top row), scene
/// launch 0x52–0x56, clip stop 0x34 on the track's channel, stop all 0x51, bank select
/// up/down/right/left 0x5E–0x61, Shift 0x62.
fn apc40_in(n: u8, ch: u8, pressed: bool) -> Option<GridInput> {
    match n {
        0..=39 => Some(GridInput::Pad {
            col: n % 8,
            row: 4 - n / 8,
            velocity: if pressed { 127 } else { 0 },
        }),
        0x52..=0x56 => Some(GridInput::Scene {
            row: n - 0x52,
            pressed,
        }),
        0x34 if usize::from(ch) < GRID_COLS => Some(GridInput::Stop { col: ch, pressed }),
        0x51 => Some(GridInput::StopAll { pressed }),
        0x5E => Some(GridInput::Arrow {
            arrow: Arrow::Up,
            pressed,
        }),
        0x5F => Some(GridInput::Arrow {
            arrow: Arrow::Down,
            pressed,
        }),
        0x60 => Some(GridInput::Arrow {
            arrow: Arrow::Right,
            pressed,
        }),
        0x61 => Some(GridInput::Arrow {
            arrow: Arrow::Left,
            pressed,
        }),
        0x62 => Some(GridInput::Shift { pressed }),
        _ => None,
    }
}

#[path = "controller_grid_lights.rs"]
mod lights_impl;
pub(super) use lights_impl::lights;
pub use lights_impl::palette_index;

/// Every light a device has, by its address: what [`lights`] walks.
pub(super) fn addresses(d: GridDevice, f: &GridFrame) -> Vec<(Addr, Light)> {
    let mut v = Vec::with_capacity(96);
    let rows = d.rows();
    for (row, line) in f.pads.iter().enumerate().take(rows) {
        for (col, l) in line.iter().enumerate() {
            v.push((Addr::Pad(col as u8, row as u8), *l));
        }
    }
    let scene_rows = if d.has_stop_row() { rows } else { rows - 1 };
    for (row, l) in f.scenes.iter().enumerate().take(scene_rows.min(GRID_ROWS)) {
        v.push((Addr::Scene(row as u8), *l));
    }
    if d.has_stop_row() {
        for (col, l) in f.stops.iter().enumerate() {
            v.push((Addr::Stop(col as u8), *l));
        }
    } else {
        v.push((Addr::StopMode, f.stop_mode));
    }
    if d != GridDevice::ApcMiniMk2 {
        v.push((Addr::StopAll, f.stop_all));
        for (i, l) in f.arrows.iter().enumerate() {
            v.push((Addr::Arrow(Arrow::ALL[i]), *l));
        }
    }
    v
}

/// A light's place on a device.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Addr {
    Pad(u8, u8),
    Scene(u8),
    Stop(u8),
    StopAll,
    StopMode,
    Arrow(Arrow),
}

/// Whether a single-colour button is on.
pub(super) fn on(l: Light) -> bool {
    lit(l)
}

/// A Launchpad light's LED index (its programmer layout's note or CC number).
pub(super) fn launchpad_led(d: GridDevice, a: Addr) -> u8 {
    match a {
        Addr::Pad(col, row) => (8 - row) * 10 + col + 1,
        Addr::Scene(row) => (8 - row) * 10 + 9,
        Addr::StopMode => 19,
        // The Pro's track select row (the stops while Stop Clip is held), and Stop Clip.
        Addr::Stop(col) => 101 + col,
        Addr::StopAll => match d {
            GridDevice::LaunchpadProMk3 => 8,
            _ => 95,
        },
        Addr::Arrow(a) => match (d, a) {
            (GridDevice::LaunchpadProMk3, Arrow::Up) => 80,
            (GridDevice::LaunchpadProMk3, Arrow::Down) => 70,
            (GridDevice::LaunchpadProMk3, Arrow::Left) => 91,
            (GridDevice::LaunchpadProMk3, Arrow::Right) => 92,
            (_, Arrow::Up) => 91,
            (_, Arrow::Down) => 92,
            (_, Arrow::Left) => 93,
            (_, Arrow::Right) => 94,
        },
    }
}
