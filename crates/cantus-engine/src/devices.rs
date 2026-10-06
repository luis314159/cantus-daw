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

/// Whether this build can open ASIO drivers (the `asio` feature, Windows only).
pub const ASIO_BUILT: bool = cfg!(all(windows, feature = "asio"));

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

/// Driver types available on this machine. The default comes first.
pub fn drivers() -> Vec<DriverInfo> {
    let default = cpal::default_host().id();
    let mut v: Vec<DriverInfo> = cpal::available_hosts()
        .into_iter()
        .map(|id| DriverInfo {
            name: id.name().to_string(),
            is_default: id == default,
        })
        .collect();
    v.sort_by_key(|d| !d.is_default);
    v
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
    let default = cpal::default_host().id();
    driver
        .and_then(|name| {
            hosts
                .iter()
                .find(|(id, _)| id.name().eq_ignore_ascii_case(name))
        })
        .or_else(|| hosts.iter().find(|(id, _)| *id == default))
        .or(hosts.first())
        .map(|(_, h)| h)
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
    /// A stream reported that its device is gone or its stream must be rebuilt
    /// (unplugged, or the driver reset): the device is lost until it is opened again.
    pub stream_lost: AtomicBool,
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
pub(crate) fn stream_error(telemetry: &DeviceTelemetry, e: &cpal::Error, which: &str) {
    use cpal::ErrorKind as K;
    match e.kind() {
        K::Xrun => {
            telemetry.xruns.fetch_add(1, Ordering::Relaxed);
        }
        K::DeviceNotAvailable | K::StreamInvalidated | K::HostUnavailable => {
            telemetry.stream_lost.store(true, Ordering::Release);
            log::error!("audio {which} lost: {e}");
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
    /// Start the input stream, its callback feeding the rings.
    pub fn start(
        self,
        telemetry: &Arc<DeviceTelemetry>,
        rate: u32,
    ) -> Result<cpal::Stream, String> {
        use cpal::traits::StreamTrait;
        let name = self.name;
        let sink = InputSink {
            ring: self.ring,
            probe: self.probe,
            telemetry: telemetry.clone(),
            rate: rate as f64,
            latency: Smoothed::default(),
            channels: self.channels,
        };
        let stream = build_input(&self.device, &self.supported, self.buffer, sink)
            .map_err(|e| format!("{name}: {e}"))?;
        stream.play().map_err(|e| format!("{name}: {e}"))?;
        log::info!("audio input: {name} @ {rate} Hz, {} ch", self.channels);
        Ok(stream)
    }
}

/// Frames the input ring holds: at least eight blocks, and never less than a third of a
/// second at 48 kHz, so a slow first callback of the output cannot overflow it.
pub(crate) fn ring_frames(buffer_frames: Option<u32>) -> usize {
    (buffer_frames.unwrap_or(1024) as usize * 8).max(16_384)
}

/// The input device the configuration names: `None` the driver's default (on ASIO the
/// output device itself when it records, so one driver serves both directions),
/// `Some("")` none at all, a name that is gone the default.
pub(crate) fn pick_input(
    host: &cpal::Host,
    config: &AudioConfig,
    output: &cpal::Device,
) -> Option<cpal::Device> {
    match config.input_device_name.as_deref() {
        Some("") => None,
        Some(name) => host
            .input_devices()
            .ok()
            .and_then(|mut it| it.find(|d| name_of(d).as_deref() == Some(name)))
            .or_else(|| host.default_input_device()),
        None => {
            if is_asio(host) && output.supports_input() {
                Some(output.clone())
            } else {
                host.default_input_device()
            }
        }
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
    /// The output callbacks counted at the last poll that saw them move, and when.
    pub out_seen: (u64, Option<std::time::Instant>),
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

    /// Whether the output has stopped calling back for more than a second while open — a
    /// driver that stalled (unplugged, or an ASIO driver that cannot run both directions).
    pub fn output_stalled(&self) -> bool {
        self.is_live()
            && self
                .dev
                .out_seen
                .1
                .is_some_and(|at| at.elapsed() > std::time::Duration::from_secs(1))
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
            let n = t.output_callbacks.load(Ordering::Relaxed);
            if n != self.dev.out_seen.0 || self.dev.out_seen.1.is_none() {
                self.dev.out_seen = (n, Some(std::time::Instant::now()));
            }
            if self.rec.input_channels > 0 {
                self.rec.set_input_latency(t.input_latency());
            }
            self.rec.set_output_latency(t.output_latency());
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
    fn note_latency(&mut self, info: &cpal::OutputCallbackInfo) {
        self.telemetry
            .output_callbacks
            .fetch_add(1, Ordering::Relaxed);
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
        self.note_latency(info);
        self.fill(data);
    }

    /// The callback for any other format: render into the scratch, convert.
    pub fn render<T: OutputSample>(&mut self, data: &mut [T], info: &cpal::OutputCallbackInfo) {
        self.note_latency(info);
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
