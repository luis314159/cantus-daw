//! Audio devices for recording (ADR 0025): the driver types (WASAPI, ASIO where built
//! with it), input and output devices and their channels, the sample formats a device
//! speaks, and opening the input stream that feeds the processor.
//!
//! The input stream runs on its own thread (the driver's). Its callback converts whatever
//! the device delivers to `f32` and pushes it, interleaved, into an `rtrb` ring whose
//! consumer the processor holds (`Processor::attach_input`). It never blocks: a full ring
//! drops the rest of the callback's samples and counts them. It also measures the input's
//! latency from the driver's timestamps and publishes it through [`DeviceTelemetry`].

use crate::backend::{AudioConfig, DeviceInfo};
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

/// Whether this build can open ASIO drivers: built with the `asio` feature (Windows) **and**
/// with the Steinberg SDK linked (round 21: without the SDK the patched `asio-sys` builds
/// stub bindings and cpal does not offer the ASIO host). Every released build has it
/// (`xtask package` refuses to build without it); a developer build may not.
pub fn asio_built() -> bool {
    static BUILT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *BUILT.get_or_init(|| {
        cfg!(all(windows, feature = "asio"))
            && cpal::available_hosts()
                .iter()
                .any(|id| id.name().eq_ignore_ascii_case("ASIO"))
    })
}

/// A driver type the machine offers, by cpal's host name ("WASAPI", "ASIO").
#[derive(Clone, Debug, PartialEq)]
pub struct DriverInfo {
    pub name: String,
    pub is_default: bool,
}

/// One input device.
#[derive(Clone, Debug, PartialEq)]
pub struct InputDeviceInfo {
    pub name: String,
    pub channels: u16,
    pub is_default: bool,
}

/// Driver types available on this machine, the best first: the default (on Windows the
/// system's, WASAPI; on Linux the first of PipeWire, JACK and ALSA that is running —
/// ADR 0065), then the others in that order. A JACK whose server is not running (or whose
/// library is not installed) is not offered: cpal's JACK host would have no devices.
pub fn drivers() -> Vec<DriverInfo> {
    // A JACK server started or stopped since the last look is seen (the device page asks
    // when it opens and on *Rescan*).
    rescan_jack();
    let default = default_id();
    let mut v: Vec<DriverInfo> = hosts()
        .iter()
        .filter(|(id, _)| usable(*id, current(*id)))
        .map(|(id, _)| DriverInfo {
            name: id.name().to_string(),
            is_default: Some(*id) == default,
        })
        .collect();
    v.sort_by_key(|d| (!d.is_default, preference(&d.name)));
    v
}

/// Where a driver type ranks among Linux's (ADR 0065): PipeWire's native API, then JACK,
/// then ALSA (always there, the fallback), then anything else. Elsewhere every host ranks
/// the same and the order stays cpal's.
pub fn preference(name: &str) -> u8 {
    if !cfg!(target_os = "linux") {
        return 0;
    }
    match name.to_ascii_lowercase().as_str() {
        "pipewire" => 0,
        "jack" => 1,
        "alsa" => 2,
        _ => 3,
    }
}

/// Whether a host can be opened now and carries the machine's sound.
///
/// - JACK says it is always available (its library is opened at run time): it counts only
///   with a device, which needs a running server.
/// - PipeWire counts when it carries the desktop's audio ([`pipewire_serves_audio`]): on
///   Ubuntu 22.04 and Mint 21 PipeWire runs for screen sharing only while PulseAudio owns
///   the sound card, and a stream to its default would go nowhere — ALSA's `default`
///   reaches PulseAudio there.
fn usable(id: cpal::HostId, host: &cpal::Host) -> bool {
    let name = id.name();
    if name.eq_ignore_ascii_case("JACK") {
        return host
            .devices()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);
    }
    if name.eq_ignore_ascii_case("PipeWire") {
        let sinks = host
            .output_devices()
            .map(|d| {
                let names: Vec<String> = d.filter_map(|d| name_of(&d)).collect();
                real_sinks(names.iter().map(String::as_str))
            })
            .unwrap_or(0);
        return pipewire_serves_audio(PipeWireFacts {
            real_sinks: sinks,
            pipewire_pulse: pipewire_pulse_running(),
        });
    }
    true
}

/// What decides whether PipeWire carries the desktop's audio (ADR 0065).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PipeWireFacts {
    /// Output nodes that are real sinks (not cpal's `default_*` stand-ins).
    pub real_sinks: usize,
    /// PulseAudio's server is PipeWire's (`pipewire-pulse` runs).
    pub pipewire_pulse: bool,
}

/// PipeWire carries the sound when it has a sink of its own, or serves PulseAudio's
/// clients; a PipeWire with neither (screen sharing only) is not offered.
pub fn pipewire_serves_audio(f: PipeWireFacts) -> bool {
    f.real_sinks > 0 || f.pipewire_pulse
}

/// How many of cpal's PipeWire output devices, by name, are real sinks.
pub fn real_sinks<'a>(names: impl Iterator<Item = &'a str>) -> usize {
    names
        .filter(|n| !matches!(*n, "default_sink" | "default_output" | "default_input"))
        .count()
}

/// Whether a `pipewire-pulse` process runs (read from `/proc`; never on other systems).
fn pipewire_pulse_running() -> bool {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return false;
    };
    dir.flatten().any(|e| {
        std::fs::read_to_string(e.path().join("comm")).is_ok_and(|c| c.trim() == "pipewire-pulse")
    })
}

/// Linux's default: the best usable host by [`preference`], from `(name, usable)` pairs.
pub fn linux_default<'a>(hosts: impl Iterator<Item = (&'a str, bool)>) -> Option<&'a str> {
    hosts
        .filter(|(_, usable)| *usable)
        .min_by_key(|(name, _)| preference(name))
        .map(|(name, _)| name)
}

/// The default driver type, worked out once: on Linux the best usable host by
/// [`preference`]; elsewhere cpal's default. cpal's `default_host()` builds a new host each
/// call (PipeWire's enumerates the graph), so it is asked at most once.
fn default_id() -> Option<cpal::HostId> {
    static DEFAULT: std::sync::OnceLock<Option<cpal::HostId>> = std::sync::OnceLock::new();
    *DEFAULT.get_or_init(|| {
        let hosts = hosts();
        if cfg!(target_os = "linux") {
            let found: Vec<(cpal::HostId, bool)> = hosts
                .iter()
                .map(|(id, _)| (*id, usable(*id, current(*id))))
                .collect();
            let best = linux_default(found.iter().map(|(id, u)| (id.name(), *u)))?;
            return found
                .iter()
                .find(|(id, _)| id.name() == best)
                .map(|(id, _)| *id);
        }
        let id = cpal::default_host().id();
        hosts.iter().any(|(h, _)| *h == id).then_some(id)
    })
}

/// JACK's host as last looked at: cpal's JACK host lists its devices once, when made, so a
/// server started (or stopped) since needs a new one. Hosts are kept for the process
/// (see [`hosts`]); a replaced JACK host is leaked on purpose — made only when the server
/// came or went, a few hundred bytes each time.
static JACK_NOW: std::sync::Mutex<Option<&'static cpal::Host>> = std::sync::Mutex::new(None);

/// The host to use for `id`: JACK's latest look, any other's one host.
fn current(id: cpal::HostId) -> &'static cpal::Host {
    if id.name().eq_ignore_ascii_case("JACK") {
        if let Some(h) = *JACK_NOW.lock().unwrap_or_else(|e| e.into_inner()) {
            return h;
        }
    }
    hosts()
        .iter()
        .find(|(h, _)| *h == id)
        .map(|(_, h)| h)
        .unwrap_or_else(|| host(None))
}

/// Look at JACK again (Linux): when a server is there now and was not, or the other way
/// round, its host is made anew.
pub fn rescan_jack() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let Some(id) = hosts()
        .iter()
        .map(|(id, _)| *id)
        .find(|id| id.name().eq_ignore_ascii_case("JACK"))
    else {
        return;
    };
    let was = usable(id, current(id));
    let Ok(fresh) = cpal::host_from_id(id) else {
        return;
    };
    if usable(id, &fresh) != was {
        *JACK_NOW.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::leak(Box::new(fresh)));
    }
}

/// Every host this machine offers, made once for the life of the process. ASIO allows one
/// driver loaded at a time and cpal's ASIO host keeps track of it, so two ASIO hosts side
/// by side (a device scan while the engine runs, say) would each think they own the
/// driver — loading a driver after another host enumerated them crashed in a test. One
/// host per driver type, shared, keeps that bookkeeping whole.
fn hosts() -> &'static [(cpal::HostId, cpal::Host)] {
    static HOSTS: std::sync::OnceLock<Vec<(cpal::HostId, cpal::Host)>> = std::sync::OnceLock::new();
    HOSTS.get_or_init(|| {
        cpal::available_hosts()
            .into_iter()
            .filter_map(|id| match cpal::host_from_id(id) {
                Ok(h) => Some((id, h)),
                Err(e) => {
                    log::warn!("driver {}: {e}", id.name());
                    None
                }
            })
            .collect()
    })
}

/// The host for a driver name (case-insensitive); the default host when the name is
/// `None`, unknown, or not available on this machine (built without it, say).
pub(crate) fn host(driver: Option<&str>) -> &'static cpal::Host {
    let hosts = hosts();
    let default = default_id();
    if let Some(name) = driver {
        if let Some((id, _)) = hosts
            .iter()
            .find(|(id, _)| id.name().eq_ignore_ascii_case(name))
        {
            return current(*id);
        }
    }
    hosts
        .iter()
        .find(|(id, _)| Some(*id) == default)
        .map(|(id, _)| current(*id))
        .or(hosts.first().map(|(_, h)| h))
        .unwrap_or_else(|| {
            // No host at all is not a machine cpal knows; keep one default for the process.
            static FALLBACK: std::sync::OnceLock<cpal::Host> = std::sync::OnceLock::new();
            FALLBACK.get_or_init(cpal::default_host)
        })
}

/// A device's name as the settings store it.
pub(crate) fn name_of(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|x| x.name().to_string())
}

/// Whether a host is ASIO's.
fn is_asio(host: &cpal::Host) -> bool {
    host.id().name().eq_ignore_ascii_case("ASIO")
}

/// The ASIO drivers installed, by name, whether or not this build can open them (the
/// setup says so when it cannot). Read from the registry; no driver is loaded.
///
/// The ASIO drivers installed, by name, read from the registry where drivers register
/// themselves (`HKLM\SOFTWARE\ASIO`) — without loading any. cpal's ASIO device list loads
/// every driver in turn to read its channels; on this machine loading a driver whose
/// interface is unplugged and then opening another crashed the process, and a scan must
/// not load a driver while the engine holds one. The first is the default, as in cpal.
pub fn installed_asio_drivers() -> Vec<String> {
    asio_driver_names()
}

fn asio_driver_names() -> Vec<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\ASIO"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        let Ok(out) = out else {
            return Vec::new();
        };
        let prefix = r"HKEY_LOCAL_MACHINE\SOFTWARE\ASIO\";
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().strip_prefix(prefix))
            .filter(|n| !n.is_empty() && !n.contains('\\'))
            .map(str::to_string)
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

/// Input devices on a driver (`None`: the default driver), the default one flagged, each
/// with the channels it opens with by default (the most it offers when that is more). On
/// ASIO the drivers are listed without being loaded, so their channels read 0 (unknown).
pub fn input_devices(driver: Option<&str>) -> Vec<InputDeviceInfo> {
    let host = host(driver);
    if is_asio(host) {
        return asio_driver_names()
            .into_iter()
            .enumerate()
            .map(|(i, name)| InputDeviceInfo {
                name,
                channels: 0,
                is_default: i == 0,
            })
            .collect();
    }
    let default = host.default_input_device().and_then(|d| name_of(&d));
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|d| {
            let name = name_of(&d)?;
            let by_default = d.default_input_config().map(|c| c.channels()).ok();
            let most = d
                .supported_input_configs()
                .ok()
                .and_then(|it| it.map(|c| c.channels()).max());
            let channels = by_default.max(most).unwrap_or(0);
            Some(InputDeviceInfo {
                is_default: Some(&name) == default.as_ref(),
                name,
                channels,
            })
        })
        .collect()
}

/// Output devices on a driver (`None`: the default driver).
pub fn output_devices_on(driver: Option<&str>) -> Vec<DeviceInfo> {
    let host = host(driver);
    if is_asio(host) {
        return asio_driver_names()
            .into_iter()
            .enumerate()
            .map(|(i, name)| DeviceInfo {
                name,
                is_default: i == 0,
            })
            .collect();
    }
    let default = host.default_output_device().and_then(|d| name_of(&d));
    host.output_devices()
        .map(|it| {
            it.filter_map(|d| name_of(&d))
                .map(|name| DeviceInfo {
                    is_default: Some(&name) == default.as_ref(),
                    name,
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------
// Sample formats

/// A sample a device delivers, as `f32` in −1…1. Integer formats are scaled by their
/// half range (`i16`: 1/32768), so every value maps exactly where the format's definition
/// puts it; unsigned formats are offset to their midpoint first.
pub trait InputSample: Copy + Send + 'static {
    fn to_f32(self) -> f32;
}

/// A sample a device plays, from `f32` (clamped to −1…1, rounded to the nearest step).
pub trait OutputSample: Copy + Send + 'static {
    fn from_f32(x: f32) -> Self;
}

impl InputSample for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}
impl InputSample for f64 {
    fn to_f32(self) -> f32 {
        self as f32
    }
}
impl InputSample for i8 {
    fn to_f32(self) -> f32 {
        self as f32 / 128.0
    }
}
impl InputSample for i16 {
    fn to_f32(self) -> f32 {
        self as f32 / 32_768.0
    }
}
impl InputSample for cpal::I24 {
    fn to_f32(self) -> f32 {
        self.inner() as f32 / 8_388_608.0
    }
}
impl InputSample for i32 {
    fn to_f32(self) -> f32 {
        (self as f64 / 2_147_483_648.0) as f32
    }
}
impl InputSample for i64 {
    fn to_f32(self) -> f32 {
        (self as f64 / 9_223_372_036_854_775_808.0) as f32
    }
}
impl InputSample for u8 {
    fn to_f32(self) -> f32 {
        (self as i16 - 128) as f32 / 128.0
    }
}
impl InputSample for u16 {
    fn to_f32(self) -> f32 {
        (self as i32 - 32_768) as f32 / 32_768.0
    }
}
impl InputSample for cpal::U24 {
    fn to_f32(self) -> f32 {
        (self.inner() - 8_388_608) as f32 / 8_388_608.0
    }
}
impl InputSample for u32 {
    fn to_f32(self) -> f32 {
        ((self as i64 - 2_147_483_648) as f64 / 2_147_483_648.0) as f32
    }
}
impl InputSample for u64 {
    fn to_f32(self) -> f32 {
        ((self as i128 - (1i128 << 63)) as f64 / 9_223_372_036_854_775_808.0) as f32
    }
}

/// `x` (clamped) scaled by `half` and rounded into `lo..=hi`.
fn scale(x: f32, half: f64, lo: f64, hi: f64) -> f64 {
    let x = if x.is_finite() {
        x.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    (x as f64 * half).round().clamp(lo, hi)
}

impl OutputSample for f32 {
    fn from_f32(x: f32) -> Self {
        x
    }
}
impl OutputSample for f64 {
    fn from_f32(x: f32) -> Self {
        x as f64
    }
}
impl OutputSample for i16 {
    fn from_f32(x: f32) -> Self {
        scale(x, 32_768.0, -32_768.0, 32_767.0) as i16
    }
}
impl OutputSample for cpal::I24 {
    fn from_f32(x: f32) -> Self {
        cpal::I24::new_unchecked(scale(x, 8_388_608.0, -8_388_608.0, 8_388_607.0) as i32)
    }
}
impl OutputSample for i32 {
    fn from_f32(x: f32) -> Self {
        scale(x, 2_147_483_648.0, -2_147_483_648.0, 2_147_483_647.0) as i32
    }
}
impl OutputSample for u16 {
    fn from_f32(x: f32) -> Self {
        (scale(x, 32_768.0, -32_768.0, 32_767.0) + 32_768.0) as u16
    }
}
impl OutputSample for u8 {
    fn from_f32(x: f32) -> Self {
        (scale(x, 128.0, -128.0, 127.0) + 128.0) as u8
    }
}
impl OutputSample for i8 {
    fn from_f32(x: f32) -> Self {
        scale(x, 128.0, -128.0, 127.0) as i8
    }
}

/// Push interleaved samples into the ring as `f32`, without blocking: what does not fit
/// is dropped. Returns how many samples were dropped.
pub fn push_converted<T: InputSample>(producer: &mut rtrb::Producer<f32>, data: &[T]) -> usize {
    let n = producer.slots().min(data.len());
    if n > 0 {
        if let Ok(chunk) = producer.write_chunk_uninit(n) {
            chunk.fill_from_iter(data[..n].iter().map(|s| s.to_f32()));
        }
    }
    data.len() - n
}

// ---------------------------------------------------------------------------------------
// What the stream threads report

/// Numbers the stream callbacks publish for the UI thread. Everything is an atomic, so the
/// callbacks never lock.
#[derive(Debug, Default)]
pub struct DeviceTelemetry {
    /// The input's latency (capture → callback) in frames, as the driver reports it.
    pub input_latency: AtomicU32,
    /// The output's latency (callback → playback) in frames, as the driver reports it.
    pub output_latency: AtomicU32,
    /// Input samples dropped because the ring was full.
    pub input_dropped: AtomicU64,
    /// Input callbacks so far (a stalled input stays still).
    pub input_callbacks: AtomicU64,
    /// Output callbacks so far.
    pub output_callbacks: AtomicU64,
    /// While set, the input goes to the loopback probe's ring instead of the processor's.
    pub probing: AtomicBool,
    /// A stream reported that its device is gone (unplugged, the driver unloaded): the
    /// device is lost until it is opened again.
    pub stream_lost: AtomicBool,
    /// A stream reported that it must be rebuilt while its device is still there: an ASIO
    /// driver asked for a reset or a resync, or changed its rate; WASAPI invalidated the
    /// stream's resources (the format changed in Windows' settings). Reopening the same
    /// device fixes it (round 21).
    pub restart_requested: AtomicBool,
    /// Frames in the output's last callback: the buffer the driver really runs.
    pub output_frames: AtomicU32,
    /// When each stream last called back (round 21): a driver that stops calling back is
    /// told from one that is slow to start, whatever the UI's frame rate.
    pub output_beat: Beat,
    pub input_beat: Beat,
    /// Underruns and overruns the driver reported (cpal's `Xrun`): dropouts.
    pub xruns: AtomicU64,
    /// When the input's last buffer arrived, and how long it was: what the processor and
    /// the loopback probe need to know how long the frames they pop have waited
    /// (ADR 0025 §7).
    pub input_clock: Arc<InputClock>,
}

/// When the input stream's last callback pushed its buffer, for the output side to tell
/// how long a frame has waited in the input ring (ADR 0025 §7). Both sides read the same
/// monotonic clock (`Instant`), so the time between the input's callback and the
/// processor's pop is measured, not assumed.
#[derive(Debug)]
pub struct InputClock {
    base: std::time::Instant,
    /// Nanoseconds from `base` to the last push, plus one (0: no buffer yet).
    last: AtomicU64,
    /// Frames in the last buffer pushed.
    frames: AtomicU32,
}

impl Default for InputClock {
    fn default() -> Self {
        InputClock {
            base: std::time::Instant::now(),
            last: AtomicU64::new(0),
            frames: AtomicU32::new(0),
        }
    }
}

impl InputClock {
    /// The input callback pushed `frames` frames just now. Call after the push, so a
    /// reader that sees the time also sees the frames.
    pub fn arrived(&self, frames: usize) {
        self.frames.store(frames as u32, Ordering::Relaxed);
        let ns = self.base.elapsed().as_nanos() as u64;
        self.last.store(ns.saturating_add(1), Ordering::Release);
    }

    /// The last buffer's length and how long ago it arrived, in frames at `rate`; `None`
    /// before the first buffer. Allocates nothing; safe on the audio thread.
    pub fn last_buffer(&self, rate: f64) -> Option<(f64, f64)> {
        let last = self.last.load(Ordering::Acquire);
        if last == 0 {
            return None;
        }
        let now = self.base.elapsed().as_nanos() as u64;
        let age = now.saturating_sub(last - 1) as f64 * 1e-9 * rate;
        Some((self.frames.load(Ordering::Relaxed) as f64, age))
    }
}

/// How long the frames popped at the top of a block have waited since their input
/// buffer arrived, in frames (ADR 0025 §7): `avail` frames were waiting when the pop
/// began; the newest `b_in` of them arrived `age` frames ago, so the oldest — the one
/// placed at the block's first sample — arrived `avail − b_in + age` frames before it is
/// placed. Without a clock (a processor driven by hand, an offline render) the block
/// itself stands for the last buffer and it arrived just now: the depth after popping.
///
/// Signed: what matters is capture → placement, which is the input's reported latency
/// plus this, and a buffer's first frame was captured `b_in` frames before its last.
pub fn queue_wait(avail: f64, block: f64, clock: Option<(f64, f64)>) -> f64 {
    let (b_in, age) = clock.unwrap_or((block, 0.0));
    avail - b_in + age
}

/// When a stream last called back, on a monotonic clock. Written by the callback (one
/// atomic store; `Instant::now` reads QPC — no lock, no allocation), read by the UI.
#[derive(Debug)]
pub struct Beat {
    base: std::time::Instant,
    /// Nanoseconds from `base` to the last callback, plus one (0: none yet).
    last: AtomicU64,
}

impl Default for Beat {
    fn default() -> Self {
        Beat {
            base: std::time::Instant::now(),
            last: AtomicU64::new(0),
        }
    }
}

impl Beat {
    /// A callback ran now.
    pub fn tick(&self) {
        let ns = self.base.elapsed().as_nanos() as u64;
        self.last.store(ns.saturating_add(1), Ordering::Release);
    }

    /// How long ago the last callback ran; `None` before the first.
    pub fn since(&self) -> Option<std::time::Duration> {
        let last = self.last.load(Ordering::Acquire);
        (last != 0).then(|| {
            self.base
                .elapsed()
                .saturating_sub(std::time::Duration::from_nanos(last - 1))
        })
    }
}

/// How long a device that has just opened may take to call back the first time before it
/// counts as stalled. ASIO4ALL took over a second here (it opens kernel streaming first).
pub const START_GRACE: std::time::Duration = std::time::Duration::from_secs(4);
/// No callback for this long, once running: the driver stopped responding.
pub const STALL_AFTER: std::time::Duration = std::time::Duration::from_secs(1);

/// The state of the open device, as the UI says it (round 21). Only [`DeviceState::Lost`]
/// means the device is gone; a driver that stopped calling back or asked to be reset is
/// still there, and restarting the audio is the cure.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum DeviceState {
    /// No device open (a dummy or manual handle).
    #[default]
    Off,
    /// Opened, waiting for the first callback (within [`START_GRACE`]).
    Starting,
    Running,
    /// The driver stopped calling back (or never started) while the device is present.
    Stalled,
    /// The driver asked for its streams to be rebuilt (an ASIO reset request, a rate it
    /// changed, WASAPI's invalidated stream).
    RestartRequested,
    /// The device is gone (unplugged, its driver unloaded).
    Lost,
}

impl DeviceState {
    /// Whether nothing is heard in this state.
    pub fn is_silent(self) -> bool {
        matches!(
            self,
            DeviceState::Stalled | DeviceState::RestartRequested | DeviceState::Lost
        )
    }
}

/// What the device's state is, from what the streams reported: gone, asked to restart,
/// and how long ago each direction last called back (`input`: `None` when no input is
/// open, `Some(None)` when it is open but has not called back). Pure, for the tests.
pub fn classify(
    lost: bool,
    restart: bool,
    since_open: std::time::Duration,
    output: Option<std::time::Duration>,
    input: Option<Option<std::time::Duration>>,
) -> DeviceState {
    if lost {
        return DeviceState::Lost;
    }
    if restart {
        return DeviceState::RestartRequested;
    }
    let stalled = |since: Option<std::time::Duration>| match since {
        None => since_open >= START_GRACE,
        Some(d) => d > STALL_AFTER,
    };
    if stalled(output) || input.is_some_and(stalled) {
        DeviceState::Stalled
    } else if output.is_none() {
        DeviceState::Starting
    } else {
        DeviceState::Running
    }
}

impl DeviceTelemetry {
    pub fn input_latency(&self) -> u32 {
        self.input_latency.load(Ordering::Relaxed)
    }
    pub fn output_latency(&self) -> u32 {
        self.output_latency.load(Ordering::Relaxed)
    }
}

/// What a stream's error callback does: log it, and count what the UI shows — a device
/// that is gone (lost), an xrun (a dropout). Atomics only; the callback never blocks.
pub fn stream_error(telemetry: &DeviceTelemetry, e: &cpal::Error, which: &str) {
    use cpal::ErrorKind as K;
    match e.kind() {
        K::Xrun => {
            telemetry.xruns.fetch_add(1, Ordering::Relaxed);
        }
        K::DeviceNotAvailable | K::HostUnavailable => {
            telemetry.stream_lost.store(true, Ordering::Release);
            log::error!("audio {which} lost: {e}");
        }
        // The device is still there; its stream must be built again (an ASIO reset
        // request, a rate the driver changed, WASAPI's invalidated resources). Not "lost":
        // the UI restarts the audio instead of saying the interface was disconnected.
        K::StreamInvalidated => {
            telemetry.restart_requested.store(true, Ordering::Release);
            log::warn!("audio {which} needs a restart: {e}");
        }
        _ => log::error!("audio {which} error: {e}"),
    }
}

/// An error's text for the user, with a tag the UI recognises when another application
/// holds the device (cpal's `DeviceBusy`, WASAPI's `AUDCLNT_E_DEVICE_IN_USE`).
pub fn describe_error(e: &cpal::Error) -> String {
    if e.kind() == cpal::ErrorKind::DeviceBusy {
        format!("{e} {BUSY_TAG}")
    } else {
        e.to_string()
    }
}

/// The tag [`describe_error`] puts on a device another application holds.
pub const BUSY_TAG: &str = "[device busy]";

/// Frames in a duration at a rate.
pub(crate) fn frames_in(d: std::time::Duration, rate: f64) -> f64 {
    (d.as_secs_f64() * rate).min(u32::MAX as f64)
}

/// A latency as the callbacks see it, smoothed: the timestamps jitter by a frame or two
/// from one callback to the next, and a take should not move with that. The first
/// reading is taken as it is; each later one moves the mean a twentieth of the way.
#[derive(Debug, Default)]
pub(crate) struct Smoothed {
    mean: Option<f64>,
}

impl Smoothed {
    pub fn add(&mut self, frames: f64, out: &AtomicU32) {
        let m = match self.mean {
            Some(m) => m + (frames - m) * 0.05,
            None => frames,
        };
        self.mean = Some(m);
        out.store(m.round() as u32, Ordering::Relaxed);
    }
}

/// The input stream callback's state: the processor's ring, the loopback probe's ring,
/// and where to report.
pub(crate) struct InputSink {
    pub ring: rtrb::Producer<f32>,
    pub probe: rtrb::Producer<f32>,
    pub telemetry: Arc<DeviceTelemetry>,
    pub rate: f64,
    pub latency: Smoothed,
    /// Interleaved channels per frame.
    pub channels: usize,
}

impl InputSink {
    pub fn feed<T: InputSample>(&mut self, data: &[T], info: &cpal::InputCallbackInfo) {
        let ts = info.timestamp();
        if let Some(d) = ts.callback.checked_duration_since(ts.capture) {
            self.latency
                .add(frames_in(d, self.rate), &self.telemetry.input_latency);
        }
        self.telemetry
            .input_callbacks
            .fetch_add(1, Ordering::Relaxed);
        self.telemetry.input_beat.tick();
        let ring = if self.telemetry.probing.load(Ordering::Acquire) {
            &mut self.probe
        } else {
            &mut self.ring
        };
        let dropped = push_converted(ring, data);
        let channels = self.channels.max(1);
        self.telemetry
            .input_clock
            .arrived((data.len() - dropped) / channels);
        if dropped > 0 {
            self.telemetry
                .input_dropped
                .fetch_add(dropped as u64, Ordering::Relaxed);
        }
    }
}

/// The input chosen and its rings, before its stream starts. The consumers go to the
/// processor (and the probe) first; the stream starts after the output's, so the output
/// — what must never fail — is up before the input is touched (ASIO drivers that serve
/// both directions want the output prepared first, too).
pub(crate) struct InputPlan {
    device: cpal::Device,
    supported: cpal::SupportedStreamConfig,
    buffer: cpal::BufferSize,
    pub name: String,
    pub channels: usize,
    ring: rtrb::Producer<f32>,
    probe: rtrb::Producer<f32>,
}

/// The consumer ends of an input's rings.
pub(crate) struct InputRings {
    pub ring: rtrb::Consumer<f32>,
    pub probe: rtrb::Consumer<f32>,
}

impl InputPlan {
    /// Build the input stream, its callback feeding the rings. It is started by the
    /// caller, after the output's is built (round 21: both directions are built before
    /// either starts, so an ASIO driver prepares both at once).
    pub fn build(
        self,
        telemetry: &Arc<DeviceTelemetry>,
        rate: u32,
    ) -> Result<cpal::Stream, String> {
        let name = self.name;
        let sink = InputSink {
            ring: self.ring,
            probe: self.probe,
            telemetry: telemetry.clone(),
            rate: rate as f64,
            latency: Smoothed::default(),
            channels: self.channels,
        };
        build_input(&self.device, &self.supported, self.buffer, sink)
            .map_err(|e| format!("{name}: {e}"))
    }
}

/// The output's rate: the one asked for when the device can run at it, else the device's
/// own — and on ASIO, which refuses a rate it does not offer, the rate asked for when it
/// is refused (shown to the user). A device that reports no rate yet (Realtek's ASIO
/// before it runs) gets 48 kHz, or 44.1, or the highest it offers.
pub(crate) fn output_rate(
    host: &cpal::Host,
    device: &cpal::Device,
    default: &cpal::SupportedStreamConfig,
    asked: Option<u32>,
) -> (u32, Option<u32>) {
    let ranges: Vec<cpal::SupportedStreamConfigRange> = device
        .supported_output_configs()
        .map(|it| it.collect())
        .unwrap_or_default();
    let offers = |r: u32| ranges.is_empty() || ranges.iter().any(|c| c.contains_rate(r));
    let own = if default.sample_rate() > 0 {
        default.sample_rate()
    } else {
        [48_000, 44_100]
            .into_iter()
            .find(|r| offers(*r))
            .or_else(|| ranges.iter().map(|c| c.max_sample_rate()).max())
            .unwrap_or(48_000)
    };
    match asked {
        // WASAPI converts a rate its mix format does not run at; only ASIO refuses.
        Some(r) if !is_asio(host) || offers(r) => (r, None),
        Some(r) => (own, Some(r)),
        None => (own, None),
    }
}

/// The buffer to ask the driver for: the size asked when the device can take it, else the
/// driver's own (which the handle reads back, [`crate::EngineHandle::device_buffer`]).
pub(crate) fn buffer_request(
    asked: Option<u32>,
    supported: &cpal::SupportedBufferSize,
) -> cpal::BufferSize {
    match (asked, supported) {
        (Some(n), cpal::SupportedBufferSize::Range { min, max }) if (*min..=*max).contains(&n) => {
            cpal::BufferSize::Fixed(n)
        }
        (Some(n), cpal::SupportedBufferSize::Unknown) => cpal::BufferSize::Fixed(n),
        _ => cpal::BufferSize::Default,
    }
}

/// Frames the input ring holds: at least eight blocks, and never less than a third of a
/// second at 48 kHz, so a slow first callback of the output cannot overflow it.
pub(crate) fn ring_frames(buffer_frames: Option<u32>) -> usize {
    (buffer_frames.unwrap_or(1024) as usize * 8).max(16_384)
}

/// The input device the configuration names: `None` the driver's default, `Some("")` none
/// at all, a name that is gone the default. On ASIO, which runs one driver at a time, the
/// input is always the output's driver (when it records) — a different name cannot open.
pub(crate) fn pick_input(
    host: &cpal::Host,
    config: &AudioConfig,
    output: &cpal::Device,
) -> Option<cpal::Device> {
    if is_asio(host) {
        return match config.input_device_name.as_deref() {
            Some("") => None,
            _ => output.supports_input().then(|| output.clone()),
        };
    }
    match config.input_device_name.as_deref() {
        Some("") => None,
        Some(name) => host
            .input_devices()
            .ok()
            .and_then(|mut it| it.find(|d| name_of(d).as_deref() == Some(name)))
            .or_else(|| host.default_input_device()),
        None => host.default_input_device(),
    }
}

/// The output device the configuration names (`None`: the driver's default). On ASIO only
/// the driver named is loaded — never the others, so an unplugged interface's driver is
/// not touched on the way (the patched cpal's `device_by_id`) — and a driver that does not
/// load is an error, not a quiet switch to another driver. Elsewhere a name that is gone
/// falls back to the default.
pub(crate) fn pick_output(host: &cpal::Host, name: Option<&str>) -> Result<cpal::Device, String> {
    match name {
        Some(name) if is_asio(host) => host
            .device_by_id(&cpal::DeviceId::new(host.id(), name))
            .ok_or_else(|| {
                format!("the ASIO driver {name} did not load (is its interface connected?)")
            }),
        Some(name) => host
            .output_devices()
            .map_err(|e| e.to_string())?
            .find(|d| name_of(d).as_deref() == Some(name))
            .or_else(|| host.default_output_device())
            .ok_or_else(|| "no output device".to_string()),
        None => host
            .default_output_device()
            .ok_or_else(|| "no output device".to_string()),
    }
}

/// The input configuration to open at `rate`: the device's default when it runs at that
/// rate, otherwise the range that offers the rate with the default's channel count (or
/// the most channels), `f32` preferred. An error says why there is none.
pub(crate) fn input_config(
    device: &cpal::Device,
    rate: u32,
) -> Result<cpal::SupportedStreamConfig, String> {
    let default = device.default_input_config().map_err(|e| e.to_string())?;
    if default.sample_rate() == rate {
        return Ok(default);
    }
    let ranges: Vec<_> = device
        .supported_input_configs()
        .map_err(|e| e.to_string())?
        .filter(|r| r.contains_rate(rate))
        .collect();
    let best = ranges.into_iter().max_by_key(|r| {
        (
            r.channels() == default.channels(),
            r.sample_format() == cpal::SampleFormat::F32,
            r.channels(),
        )
    });
    match best {
        Some(r) => Ok(r.with_sample_rate(rate)),
        None => Err(format!(
            "the input runs at {} Hz and cannot run at {rate} Hz",
            default.sample_rate()
        )),
    }
}

/// Build the input stream on `device` at `rate` with the given block size request, its
/// callback feeding `sink`. Every sample format cpal knows is converted to `f32`.
pub(crate) fn build_input(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    buffer: cpal::BufferSize,
    mut sink: InputSink,
) -> Result<cpal::Stream, String> {
    let config = cpal::StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size: buffer,
    };
    let telemetry = sink.telemetry.clone();
    let err = move |e: cpal::Error| stream_error(&telemetry, &e, "input");
    macro_rules! build {
        ($t:ty) => {
            device.build_input_stream::<$t, _, _>(
                config,
                move |data: &[$t], info: &cpal::InputCallbackInfo| sink.feed(data, info),
                err,
                None,
            )
        };
    }
    use cpal::SampleFormat as F;
    let stream = match supported.sample_format() {
        F::F32 => build!(f32),
        F::F64 => build!(f64),
        F::I8 => build!(i8),
        F::I16 => build!(i16),
        F::I24 => build!(cpal::I24),
        F::I32 => build!(i32),
        F::I64 => build!(i64),
        F::U8 => build!(u8),
        F::U16 => build!(u16),
        F::U24 => build!(cpal::U24),
        F::U32 => build!(u32),
        F::U64 => build!(u64),
        other => {
            return Err(format!(
                "the input's sample format ({other}) is not supported"
            ))
        }
    };
    stream.map_err(|e| describe_error(&e))
}

/// Choose the input the configuration names, for an output running at `rate`, and make
/// its rings. `Ok(None)` when no input is wanted or there is none; an error says why the
/// one asked for cannot open (the output runs without it).
pub(crate) fn plan_input(
    host: &cpal::Host,
    config: &AudioConfig,
    output: &cpal::Device,
    rate: u32,
) -> Result<Option<(InputPlan, InputRings)>, String> {
    let Some(device) = pick_input(host, config, output) else {
        return Ok(None);
    };
    let name = name_of(&device).unwrap_or_default();
    let supported = input_config(&device, rate).map_err(|e| format!("{name}: {e}"))?;
    let channels = supported.channels().max(1) as usize;
    // A fixed block size the input cannot take: its own.
    let buffer = match (config.buffer_frames, supported.buffer_size()) {
        (Some(n), cpal::SupportedBufferSize::Range { min, max }) if (*min..=*max).contains(&n) => {
            cpal::BufferSize::Fixed(n)
        }
        _ => cpal::BufferSize::Default,
    };
    let frames = ring_frames(config.buffer_frames);
    let (ring_tx, ring_rx) = rtrb::RingBuffer::<f32>::new(frames * channels);
    let (probe_tx, probe_rx) = rtrb::RingBuffer::<f32>::new(frames * channels);
    Ok(Some((
        InputPlan {
            device,
            supported,
            buffer,
            name,
            channels,
            ring: ring_tx,
            probe: probe_tx,
        },
        InputRings {
            ring: ring_rx,
            probe: probe_rx,
        },
    )))
}

// ---------------------------------------------------------------------------------------
// The handle's side

/// The latencies the drivers report, in frames at the engine's rate.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceLatency {
    /// Capture → the input callback.
    pub input: u32,
    /// The output callback → playback.
    pub output: u32,
}

impl DeviceLatency {
    pub fn round_trip(&self) -> u32 {
        self.input + self.output
    }
}

/// What the handle keeps about the devices it opened.
#[derive(Default)]
pub(crate) struct HandleDevices {
    /// The driver in use, by cpal's host name.
    pub driver: String,
    /// The block size asked for (`None`: the driver's).
    pub buffer_frames: Option<u32>,
    /// Why the input asked for did not open.
    pub input_error: Option<String>,
    pub telemetry: Option<Arc<DeviceTelemetry>>,
    /// Held so the input runs as long as the handle.
    #[allow(dead_code)]
    pub input_stream: Option<cpal::Stream>,
    /// Probes to the output callback, and back.
    pub probe_tx: Option<rtrb::Producer<Box<crate::loopback::Probe>>>,
    pub probe_back: Option<rtrb::Consumer<Box<crate::loopback::Probe>>>,
    /// When the running probe was sent.
    pub probe_sent: Option<std::time::Instant>,
    /// When the streams were opened (the start grace counts from here).
    pub opened: Option<std::time::Instant>,
    /// The buffer the output stream reported when it opened (ASIO: what the driver
    /// created, which the driver may dictate), before any callback says.
    pub stream_buffer: Option<u32>,
    /// The rate asked for when the device could not run at it and runs at its own.
    pub rate_refused: Option<u32>,
    /// The output's routing, shared with its callback (round 20).
    pub routing: Option<Arc<crate::routing::OutputRouting>>,
}

impl crate::backend::EngineHandle {
    /// The driver type in use ("WASAPI", "ASIO"; "none" without a device).
    pub fn driver_name(&self) -> &str {
        if self.dev.driver.is_empty() {
            "none"
        } else {
            &self.dev.driver
        }
    }

    /// The block size asked for (`None`: the driver's own).
    pub fn requested_buffer(&self) -> Option<u32> {
        self.dev.buffer_frames
    }

    /// Why the input device asked for did not open (the output runs without it).
    pub fn input_error(&self) -> Option<&str> {
        self.dev.input_error.as_deref()
    }

    /// The latencies the drivers report now (0 until the streams have run a block).
    pub fn device_latency(&self) -> DeviceLatency {
        self.dev
            .telemetry
            .as_ref()
            .map(|t| DeviceLatency {
                input: if self.rec.input_channels > 0 {
                    t.input_latency()
                } else {
                    0
                },
                output: t.output_latency(),
            })
            .unwrap_or_default()
    }

    /// Input samples dropped because the processor did not keep up (a full ring).
    pub fn input_dropped(&self) -> u64 {
        self.dev
            .telemetry
            .as_ref()
            .map_or(0, |t| t.input_dropped.load(Ordering::Relaxed))
    }

    /// The device's state now: running, starting, stalled, asked to restart, lost
    /// (round 21). Read from the streams' own clocks, so a slow UI frame never makes a
    /// running device look stalled.
    pub fn device_state(&self) -> DeviceState {
        let (Some(t), Some(opened)) = (self.dev.telemetry.as_ref(), self.dev.opened) else {
            return DeviceState::Off;
        };
        if !self.is_live() {
            return DeviceState::Off;
        }
        classify(
            t.stream_lost.load(Ordering::Acquire),
            t.restart_requested.load(Ordering::Acquire),
            opened.elapsed(),
            t.output_beat.since(),
            (self.rec.input_channels > 0).then(|| t.input_beat.since()),
        )
    }

    /// Whether the output (or the input) has stopped calling back while the device is
    /// still there — a driver that stopped responding, not an unplugged interface.
    pub fn output_stalled(&self) -> bool {
        self.device_state() == DeviceState::Stalled
    }

    /// Whether the driver asked for its streams to be rebuilt (see [`DeviceState`]).
    pub fn restart_requested(&self) -> bool {
        self.device_state() == DeviceState::RestartRequested
    }

    /// The buffer the device really runs, in frames: the output's last callback, else what
    /// the stream reported when it opened, else what was asked for. ASIO drivers often
    /// dictate it (their control panel's setting), so it is read back, never assumed.
    pub fn device_buffer(&self) -> Option<u32> {
        self.dev
            .telemetry
            .as_ref()
            .map(|t| t.output_frames.load(Ordering::Relaxed))
            .filter(|n| *n > 0)
            .or(self.dev.stream_buffer)
            .or(self.dev.buffer_frames)
    }

    /// The rate asked for when the device could not run at it (it runs at
    /// [`Self::sample_rate`] instead).
    pub fn rate_refused(&self) -> Option<u32> {
        self.dev.rate_refused
    }

    /// Callbacks so far on the input and the output streams (a stalled stream stands
    /// still), for a diagnosis.
    pub fn device_callbacks(&self) -> (u64, u64) {
        self.dev.telemetry.as_ref().map_or((0, 0), |t| {
            (
                t.input_callbacks.load(Ordering::Relaxed),
                t.output_callbacks.load(Ordering::Relaxed),
            )
        })
    }

    /// Fold what the stream threads published into the handle. Called from `poll`.
    pub(crate) fn sync_devices(&mut self) {
        if let Some(t) = self.dev.telemetry.as_ref() {
            if self.rec.input_channels > 0 {
                self.rec.set_input_latency(t.input_latency());
            }
            self.rec.set_output_latency(t.output_latency());
            // The engine moves a take for a heard-latency change only after this much
            // (review 3); told when it changes.
            let trip = self
                .rec
                .latency
                .input
                .saturating_add(self.rec.latency.output);
            if self.rec.round_trip_sent != Some(trip) {
                self.rec.round_trip_sent = Some(trip);
                self.send(crate::messages::EngineMessage::SetDeviceRoundTrip(trip));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// The output callback

/// Frames converted at a time for a device that does not take `f32`.
const SCRATCH_FRAMES: usize = 4096;

/// The output stream callback's state: the processor, the loopback probe, and a scratch
/// buffer (allocated here, on the UI thread) for devices that play integer samples.
pub(crate) struct OutputDriver {
    pub processor: crate::processor::Processor,
    pub channels: usize,
    pub rate: f64,
    pub telemetry: Arc<DeviceTelemetry>,
    pub probe: crate::loopback::ProbeSlot,
    pub scratch: Vec<f32>,
    pub latency: Smoothed,
    /// Where the master goes, the cue and the test tone (round 20, `routing.rs`).
    pub routing: Arc<crate::routing::OutputRouting>,
    pub tone: crate::routing::Tone,
}

impl OutputDriver {
    fn note_latency(&mut self, info: &cpal::OutputCallbackInfo, samples: usize) {
        self.telemetry
            .output_callbacks
            .fetch_add(1, Ordering::Relaxed);
        self.telemetry
            .output_frames
            .store((samples / self.channels.max(1)) as u32, Ordering::Relaxed);
        self.telemetry.output_beat.tick();
        let ts = info.timestamp();
        if let Some(d) = ts.playback.checked_duration_since(ts.callback) {
            self.latency
                .add(frames_in(d, self.rate), &self.telemetry.output_latency);
        }
    }

    fn fill(&mut self, data: &mut [f32]) {
        if !self.probe.run(data, self.channels) {
            self.processor.process(data, self.channels);
            let route = self.routing.read();
            crate::routing::route(data, self.channels, self.rate, route, &mut self.tone);
        }
    }

    /// The callback for an `f32` device.
    pub fn render_f32(&mut self, data: &mut [f32], info: &cpal::OutputCallbackInfo) {
        self.note_latency(info, data.len());
        self.fill(data);
    }

    /// The callback for any other format: render into the scratch, convert.
    pub fn render<T: OutputSample>(&mut self, data: &mut [T], info: &cpal::OutputCallbackInfo) {
        self.note_latency(info, data.len());
        let mut scratch = std::mem::take(&mut self.scratch);
        let step = (scratch.len() / self.channels).max(1) * self.channels;
        for chunk in data.chunks_mut(step) {
            let s = &mut scratch[..chunk.len()];
            s.fill(0.0);
            self.fill(s);
            for (o, x) in chunk.iter_mut().zip(s.iter()) {
                *o = T::from_f32(*x);
            }
        }
        self.scratch = scratch;
    }
}

/// Build the output stream for `driver` in the device's own sample format.
pub(crate) fn build_output(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    format: cpal::SampleFormat,
    mut driver: OutputDriver,
) -> Result<cpal::Stream, String> {
    driver.scratch = vec![0.0; SCRATCH_FRAMES * driver.channels.max(1)];
    let telemetry = driver.telemetry.clone();
    let err = move |e: cpal::Error| stream_error(&telemetry, &e, "output");
    macro_rules! build {
        ($t:ty) => {
            device.build_output_stream::<$t, _, _>(
                config,
                move |data: &mut [$t], info: &cpal::OutputCallbackInfo| driver.render(data, info),
                err,
                None,
            )
        };
    }
    use cpal::SampleFormat as F;
    let stream = match format {
        F::F32 => device.build_output_stream::<f32, _, _>(
            config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| driver.render_f32(data, info),
            err,
            None,
        ),
        F::F64 => build!(f64),
        F::I8 => build!(i8),
        F::I16 => build!(i16),
        F::I24 => build!(cpal::I24),
        F::I32 => build!(i32),
        F::U8 => build!(u8),
        F::U16 => build!(u16),
        other => {
            return Err(format!(
                "the output's sample format ({other}) is not supported"
            ))
        }
    };
    stream.map_err(|e| describe_error(&e))
}
