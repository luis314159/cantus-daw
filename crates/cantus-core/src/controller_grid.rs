//! Controller grids (round 33, ADR 0077): Novation's Launchpads (Mini MK3, X, Pro MK3, in
//! their programmer modes) and Akai's APCs (mini mk2, APC40 mkII, in their host-controlled
//! modes) as grids that mirror the session view — pads launch, their lights show each
//! slot's colour and state, the scene buttons launch rows, the stop buttons stop tracks,
//! the arrows move the *session ring* (the box of slots the grid shows).
//!
//! Pure and deterministic: what a device's message means ([`GridDevice::decode`]), the
//! messages that put it in its mode and back ([`GridDevice::hello`], [`GridDevice::goodbye`])
//! and the messages that turn one picture of the grid into another
//! ([`GridDevice::lights`]). The interface reads the ports, builds the picture
//! ([`GridFrame`]) from the session and the engine's events, and sends what changed.
//!
//! Each device's protocol is its maker's published reference (ADR 0077 cites them): the
//! Launchpads' *Programmer's Reference Guide*s — the SysEx header `F0 00 20 29 02 <id>`,
//! the programmer layout (pads 11–88 from the bottom left, the right column's CCs 19–89,
//! the top row's CCs 91–98), static / flashing / pulsing lighting and the LED SysEx
//! (command 03h) with RGB colour specs; Akai's *Communication Protocol*s — the introduction
//! SysEx (`F0 47 7F <model> 60 …`), the pads' notes, the channel that sets a light's
//! behaviour and the APC mini mk2's RGB SysEx (`F0 47 7F 4F 24 …`).

use serde::{Deserialize, Serialize};

/// The columns every grid has (eight tracks).
pub const GRID_COLS: usize = 8;
/// The most rows a grid has (the Launchpads' eight scenes).
pub const GRID_ROWS: usize = 8;

/// A device Cantus drives as a grid.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GridDevice {
    LaunchpadMiniMk3,
    LaunchpadX,
    LaunchpadProMk3,
    ApcMiniMk2,
    Apc40Mk2,
}

/// An arrow (or a bank button): the ring moved by a track or a scene.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Arrow {
    Up,
    Down,
    Left,
    Right,
}

impl Arrow {
    pub const ALL: [Arrow; 4] = [Arrow::Up, Arrow::Down, Arrow::Left, Arrow::Right];

    fn index(self) -> usize {
        match self {
            Arrow::Up => 0,
            Arrow::Down => 1,
            Arrow::Left => 2,
            Arrow::Right => 3,
        }
    }
}

/// What a message from a grid means. Rows count from the top, columns from the left.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GridInput {
    /// A pad pressed at `velocity` (1…127), or let go (`0`).
    Pad {
        col: u8,
        row: u8,
        velocity: u8,
    },
    /// A scene button (the right column), pressed or let go.
    Scene {
        row: u8,
        pressed: bool,
    },
    /// A track's stop button.
    Stop {
        col: u8,
        pressed: bool,
    },
    /// *Stop all clips*.
    StopAll {
        pressed: bool,
    },
    Arrow {
        arrow: Arrow,
        pressed: bool,
    },
    Shift {
        pressed: bool,
    },
    /// A Launchpad Mini's or X's lowest scene button: the bottom row of pads becomes the
    /// tracks' stop buttons while it is on (Ableton's *Stop Clip* mode on those devices).
    /// The Launchpad Pro's *Stop Clip*: its track select row is the stops while it is held.
    StopMode {
        pressed: bool,
    },
    /// A Launchpad Pro's track select button (the row under the grid).
    TrackSelect {
        col: u8,
        pressed: bool,
    },
}

/// A colour, 0…255 a channel.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const WHITE: Rgb = Rgb(255, 255, 255);
    pub const RED: Rgb = Rgb(255, 0, 0);
    pub const GREEN: Rgb = Rgb(0, 255, 0);
    pub const AMBER: Rgb = Rgb(255, 160, 0);

    /// The colour at `f` (0…1) of its brightness.
    pub fn dim(self, f: f32) -> Rgb {
        let f = f.clamp(0.0, 1.0);
        let c = |v: u8| (f32::from(v) * f).round() as u8;
        Rgb(c(self.0), c(self.1), c(self.2))
    }

    fn is_black(self) -> bool {
        self == Rgb(0, 0, 0)
    }
}

/// One light.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Light {
    #[default]
    Off,
    /// Steady in its colour.
    On(Rgb),
    /// Flashing between its colour and dark, a beat a period (the devices' own clock).
    Flash(Rgb),
    /// Pulsing between dark and its colour.
    Pulse(Rgb),
}

impl Light {
    fn lit(self) -> bool {
        match self {
            Light::Off => false,
            Light::On(c) | Light::Flash(c) | Light::Pulse(c) => !c.is_black(),
        }
    }
}

/// A picture of a grid: every light it has (a device leaves out what it does not have).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GridFrame {
    /// `pads[row][col]`, row 0 at the top.
    pub pads: [[Light; GRID_COLS]; GRID_ROWS],
    pub scenes: [Light; GRID_ROWS],
    pub stops: [Light; GRID_COLS],
    pub stop_all: Light,
    /// Up, down, left, right: lit where the ring can move.
    pub arrows: [Light; 4],
    /// A Launchpad Mini's or X's stop-mode button (its lowest scene button).
    pub stop_mode: Light,
}

impl GridFrame {
    /// Every light dark: what a device is told before it is let go.
    pub fn dark() -> GridFrame {
        GridFrame::default()
    }
}

#[path = "controller_grid_devices.rs"]
mod devices;
pub use devices::palette_index;

#[cfg(test)]
#[path = "controller_grid_tests.rs"]
mod tests;

impl GridDevice {
    pub const ALL: [GridDevice; 5] = [
        GridDevice::LaunchpadMiniMk3,
        GridDevice::LaunchpadX,
        GridDevice::LaunchpadProMk3,
        GridDevice::ApcMiniMk2,
        GridDevice::Apc40Mk2,
    ];

    /// Its name, as the maker writes it (not translated).
    pub fn name(self) -> &'static str {
        match self {
            GridDevice::LaunchpadMiniMk3 => "Launchpad Mini MK3",
            GridDevice::LaunchpadX => "Launchpad X",
            GridDevice::LaunchpadProMk3 => "Launchpad Pro MK3",
            GridDevice::ApcMiniMk2 => "APC mini mk2",
            GridDevice::Apc40Mk2 => "APC40 mkII",
        }
    }

    /// How many scene rows it shows (the APC40's five; the Launchpads' and the APC mini's
    /// eight — a Mini's or an X's seven while its stop mode is on).
    pub fn rows(self) -> usize {
        match self {
            GridDevice::Apc40Mk2 => 5,
            _ => GRID_ROWS,
        }
    }

    /// Whether it has stop buttons of its own (else its stop mode lends the bottom row).
    pub fn has_stop_row(self) -> bool {
        !matches!(self, GridDevice::LaunchpadMiniMk3 | GridDevice::LaunchpadX)
    }

    /// The device that answers on the port named `name`, when it is the port its
    /// programmer (or host-controlled) mode talks on — never its DAW port, nor the
    /// Launchpad Pro's DIN port: each Launchpad has several (Windows names the later ones
    /// `MIDIIN2 (…)`, `MIDIIN3 (…)`; Linux and macOS add the interface's name).
    pub fn detect(name: &str) -> Option<GridDevice> {
        devices::detect(name)
    }

    /// What puts it in the mode Cantus drives: the Launchpads' programmer mode, the APCs'
    /// host-controlled mode.
    pub fn hello(self) -> Vec<Vec<u8>> {
        devices::hello(self)
    }

    /// Whether a message from a grid's port that is no part of the grid goes on to MIDI
    /// learn and the mappings: a fader's or a knob's (a controller), the pitch wheel, a
    /// program change — never what a pad sends (a note, its aftertouch, channel pressure),
    /// which the grid keeps.
    pub fn passes_on(status: u8) -> bool {
        matches!(status & 0xF0, 0xB0 | 0xC0 | 0xE0)
    }

    /// What gives it back to itself (its own standalone mode) once Cantus lets it go.
    pub fn goodbye(self) -> Vec<Vec<u8>> {
        devices::goodbye(self)
    }

    /// What a channel message (`status`, `data1`, `data2`) from it means; `None` for what
    /// is no part of the grid (a fader, a knob, a button Cantus leaves alone). `shift` says
    /// whether its Shift button is held (an APC mini's arrows and *stop all* are Shift
    /// with a button).
    pub fn decode(self, status: u8, data1: u8, data2: u8, shift: bool) -> Option<GridInput> {
        devices::decode(self, status, data1, data2, shift)
    }

    /// The messages that turn the device's lights from `old` (what it shows; `None`: not
    /// known, everything is sent) into `new`, each a whole MIDI message; nothing when the
    /// two agree.
    pub fn lights(self, old: Option<&GridFrame>, new: &GridFrame) -> Vec<Vec<u8>> {
        devices::lights(self, old, new)
    }
}

/// Where the ring is, and how it moves: its first track and scene, kept inside a session
/// of `tracks` × `scenes` that a grid of `cols` × `rows` looks at.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ring {
    pub track: usize,
    pub scene: usize,
}

impl Ring {
    /// The ring moved one step by `arrow` (a whole page with `page`), kept where it still
    /// shows something: it never leaves a session larger than itself partly empty.
    pub fn moved(
        self,
        arrow: Arrow,
        page: bool,
        size: (usize, usize),
        view: (usize, usize),
    ) -> Ring {
        let (tracks, scenes) = size;
        let (cols, rows) = view;
        let step_t = if page { cols.max(1) } else { 1 };
        let step_s = if page { rows.max(1) } else { 1 };
        let max_t = tracks.saturating_sub(cols);
        let max_s = scenes.saturating_sub(rows);
        let mut r = self;
        match arrow {
            Arrow::Up => r.scene = r.scene.saturating_sub(step_s),
            Arrow::Down => r.scene = (r.scene + step_s).min(max_s),
            Arrow::Left => r.track = r.track.saturating_sub(step_t),
            Arrow::Right => r.track = (r.track + step_t).min(max_t),
        }
        r.track = r.track.min(max_t);
        r.scene = r.scene.min(max_s);
        r
    }

    /// Whether `arrow` would move it.
    pub fn can_move(self, arrow: Arrow, size: (usize, usize), view: (usize, usize)) -> bool {
        self.moved(arrow, false, size, view) != self
    }
}

/// Lights an arrow when the ring can move that way.
pub fn arrow_lights(ring: Ring, size: (usize, usize), view: (usize, usize), on: Rgb) -> [Light; 4] {
    let mut out = [Light::Off; 4];
    for a in Arrow::ALL {
        if ring.can_move(a, size, view) {
            out[a.index()] = Light::On(on);
        }
    }
    out
}

/// Whether a light is lit (for the devices with single-colour buttons).
pub(crate) fn lit(l: Light) -> bool {
    l.lit()
}
