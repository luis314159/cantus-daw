//! Audio set up directly (round 20, `docs/BACKLOG-excellence.md` §6b): the always-visible
//! audio chip and its popover, the first-run setup, plug-and-play, channel names and the
//! routing page, and problems said in words with a fix.
//!
//! This module is the model; the views are `views::audio_*`. Everything a view needs is
//! read through [`Live`] (the running engine, or a demo's numbers) and [`Scan`] (what is
//! plugged in), and every change goes through [`apply`], which stores the choice for the
//! interface it belongs to and reopens the device — or, when [`AudioSetup::offline`] is
//! set (tests, screenshots), records it without touching the machine's sound card.

pub mod demo;
pub mod interfaces;
pub mod latency;
pub mod memory;
pub mod names;
pub mod plug;
pub mod problems;

use crate::app::CantusApp;
use crate::settings::AudioSettings;
use cantus_engine::{DeviceLatency, DriverInfo, InputDeviceInfo};
use interfaces::Interface;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// What the running engine says, gathered once a frame (or a demo's numbers).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    pub live: bool,
    pub driver: String,
    pub device: String,
    /// The output device asked for (the settings'), for a sentence about it.
    pub wanted: String,
    pub input: Option<String>,
    pub rate: f64,
    /// Frames per block the device runs (or asked for, before the first block).
    pub block: u32,
    pub in_channels: usize,
    pub out_channels: usize,
    /// Each input channel's peak since the last frame, linear (only while a view meters).
    pub peaks: Vec<f32>,
    pub latency: DeviceLatency,
    pub lost: bool,
    pub stalled: bool,
    /// Dropouts so far: the driver's xruns, the processor's overruns, dropped input.
    pub dropouts: u64,
    pub input_error: Option<String>,
    /// Why the device asked for did not open.
    pub open_error: Option<String>,
}

/// What is plugged in: the drivers, the system driver's endpoints, the ASIO drivers, and
/// those grouped into interfaces.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    pub drivers: Vec<DriverInfo>,
    pub outputs: Vec<String>,
    pub inputs: Vec<InputDeviceInfo>,
    pub asio: Vec<String>,
    pub interfaces: Vec<Interface>,
}

impl Scan {
    /// Scan the machine. Slow (tens of ms): the views ask for it on a thread.
    pub fn now() -> Scan {
        use cantus_engine::devices;
        let drivers = devices::drivers();
        let outputs: Vec<String> = devices::output_devices_on(None)
            .into_iter()
            .map(|d| d.name)
            .collect();
        let inputs = devices::input_devices(None);
        let asio: Vec<String> = if drivers.iter().any(|d| d.name.eq_ignore_ascii_case("ASIO")) {
            devices::output_devices_on(Some("ASIO"))
                .into_iter()
                .map(|d| d.name)
                .collect()
        } else {
            Vec::new()
        };
        Scan::of(drivers, outputs, inputs, asio)
    }

    /// A scan of given lists (a demo, a test).
    pub fn of(
        drivers: Vec<DriverInfo>,
        outputs: Vec<String>,
        inputs: Vec<InputDeviceInfo>,
        asio: Vec<String>,
    ) -> Scan {
        let names: Vec<String> = inputs.iter().map(|i| i.name.clone()).collect();
        let interfaces = interfaces::group(&outputs, &names, &asio);
        Scan {
            drivers,
            outputs,
            inputs,
            asio,
            interfaces,
        }
    }

    pub fn has_driver(&self, name: &str) -> bool {
        self.drivers.iter().any(|d| d.name.eq_ignore_ascii_case(name))
    }

    pub fn interface(&self, key: &str) -> Option<&Interface> {
        self.interfaces.iter().find(|i| i.key == key)
    }
}

/// Where the loopback measurement is (first-run's offer).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Measure {
    #[default]
    Idle,
    Running,
    Done(cantus_engine::LoopbackResult),
    Failed(cantus_engine::LoopbackError),
}

/// The first-run setup's own state.
#[derive(Clone, Debug, Default)]
pub struct FirstRun {
    /// The test tone is playing.
    pub tone: bool,
    pub measure: Measure,
    /// The meters' held peaks, in dB, per input channel.
    pub hold: Vec<f32>,
    /// The loudest input lately (0-based), once one has moved.
    pub heard: Option<usize>,
}

/// A card at the corner of the window: something happened to a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// An interface was plugged in: switch to it?
    Connected(String),
    /// The interface in use was unplugged.
    Lost(String),
    /// It came back, and Cantus opened it again.
    Reconnected(String),
}

/// Things a test can click, with the rect they were drawn in this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Chip,
    /// The popover's interface field, and row `i` of its list once open.
    InterfaceField,
    InterfaceRow(usize),
    Driver(String),
    Rate(Option<u32>),
    Buffer(u32),
    InputField,
    InputRow(usize),
    Tone,
    Routing,
    Assistant,
    Settings,
    /// Fix `j` of problem `i`.
    Fix(usize, usize),
    /// Button `j` of notice `i`.
    Notice(usize, usize),
    /// First run: interface card `i`, the tone, measure, done, skip, add-a-track.
    Card(usize),
    FirstTone,
    FirstMeasure,
    FirstDone,
    FirstSkip,
    /// Routing page: the main pair `p`, the cue pair (`None` off), a channel's name field.
    Main(u16),
    Cue(Option<u16>),
    InputName(usize),
    OutputName(usize),
    /// A window's close key; the popover's open list.
    Close,
    List,
}

/// The rects published this frame.
#[derive(Clone, Debug, Default)]
pub struct Geo {
    pass: u64,
    pub hits: Vec<(Hit, egui::Rect)>,
}

impl Geo {
    /// Where `h` was drawn this frame.
    pub fn rect(&self, h: &Hit) -> Option<egui::Rect> {
        self.hits.iter().rev().find(|(x, _)| x == h).map(|(_, r)| *r)
    }
}

/// Dropouts over time: the chip is amber for a few seconds after one, and a few in a row
/// name the buffer as the cause.
#[derive(Clone, Debug, Default)]
pub struct Dropouts {
    seen: Option<u64>,
    events: VecDeque<Instant>,
}

impl Dropouts {
    /// Fold the total in; returns how many arrived since the last call.
    pub fn update(&mut self, total: u64, now: Instant) -> u64 {
        let new = self.seen.map_or(0, |s| total.saturating_sub(s));
        self.seen = Some(total);
        for _ in 0..new.min(64) {
            self.events.push_back(now);
        }
        while self
            .events
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(30))
        {
            self.events.pop_front();
        }
        new
    }

    /// Dropouts in the last `window`.
    pub fn within(&self, window: Duration, now: Instant) -> u64 {
        self.events
            .iter()
            .filter(|t| now.duration_since(**t) <= window)
            .count() as u64
    }

    /// Forget them (a new device, a new buffer).
    pub fn reset(&mut self) {
        self.seen = None;
        self.events.clear();
    }
}

/// The open popover's lists.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PopList {
    Interface,
    Input,
}

/// Everything this round keeps on the application (`CantusApp::audio_setup`).
#[derive(Default)]
pub struct AudioSetup {
    pub popover: bool,
    pub pop_list: Option<PopList>,
    /// The popover's height last frame (it sizes itself to what it shows).
    pub pop_h: f32,
    /// Where the open list hangs (under its field).
    pub list_at: egui::Pos2,
    pub first_run: Option<FirstRun>,
    pub routing: bool,
    pub scan: Option<Scan>,
    scanning: Option<mpsc::Receiver<Scan>>,
    /// Never open the real device: record what would be applied (tests, screenshots).
    pub offline: bool,
    pub applied: Vec<AudioSettings>,
    /// Numbers to show instead of the engine's.
    pub demo: Option<Live>,
    pub plug: plug::Plug,
    pub notices: Vec<(Notice, Instant)>,
    pub dropouts: Dropouts,
    /// The pair the test tone plays on, while it plays.
    pub tone: Option<u16>,
    pub geo: Geo,
    /// The name field being typed in on the routing page: input?, channel, text.
    pub editing: Option<(bool, usize, String)>,
    /// The device the channel names were last looked up for.
    names_for: String,
}

impl AudioSetup {
    /// Publish where `h` was drawn (cleared when a new frame begins).
    pub fn hit(&mut self, ctx: &egui::Context, h: Hit, r: egui::Rect) {
        let pass = ctx.cumulative_pass_nr();
        if self.geo.pass != pass {
            self.geo.pass = pass;
            self.geo.hits.clear();
        }
        self.geo.hits.push((h, r));
    }

    /// Start a scan on a thread (or keep the one a demo set when offline).
    pub fn request_scan(&mut self) {
        if self.offline || self.scanning.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("cantus-device-scan".into())
            .spawn(move || {
                let _ = tx.send(Scan::now());
            });
        if spawned.is_ok() {
            self.scanning = Some(rx);
        }
    }

    /// Take a finished scan in.
    pub fn poll_scan(&mut self) {
        if let Some(rx) = &self.scanning {
            match rx.try_recv() {
                Ok(s) => {
                    self.scan = Some(s);
                    self.scanning = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.scanning = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning.is_some()
    }

    pub fn notice(&mut self, n: Notice) {
        self.notices.retain(|(x, _)| x != &n);
        self.notices.push((n, Instant::now()));
    }
}

mod apply;
pub use apply::*;
