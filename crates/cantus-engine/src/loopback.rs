//! The loopback measurement (ADR 0025 §7): with a cable from an output to an input, play a
//! train of noise bursts and find them on the input by cross-correlation. What it
//! measures is the round trip *as the engine sees it* — from the output frame a sample
//! was written at to the output frame the processor pops it back from the input ring —
//! so the number it returns — the measured round trip minus what the drivers report and
//! minus the input ring's wait it saw (measured as the processor measures it) — is
//! exactly the correction a recording needs on top of `RecordLatency`'s other parts
//! (ADR 0025 §7).
//!
//! It runs inside the engine's own streams, so it works on drivers that allow a device
//! to be opened only once (ASIO): while a probe runs, the output callback plays the probe
//! instead of calling the processor, and the input callback sends its samples to the
//! probe's ring instead of the processor's. The probe and its buffers are built on the UI
//! thread, travel through rings, and come back to be freed there (ADR 0005).

use crate::backend::EngineHandle;
use crate::devices::DeviceTelemetry;
use cantus_dsp::math;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Noise bursts in the train.
const BURSTS: usize = 8;
/// Frames in each burst.
const BURST_FRAMES: usize = 1024;
/// Burst level (linear peak before the window).
const BURST_LEVEL: f32 = 0.5;
/// How strong the peak must stand above the correlation's spread to be believed.
pub const MIN_CONFIDENCE: f32 = 8.0;

/// Why a loopback measurement gave no number.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoopbackError {
    /// No audio device is open.
    NoDevice,
    /// No input is open to listen on.
    NoInput,
    /// A measurement is already running.
    Busy,
    /// Nothing came back from the engine in time (the device stalled).
    Timeout,
    /// The input delivered nothing while the probe played.
    NoInputData,
    /// The bursts were not found on any input channel.
    NotFound,
}

impl LoopbackError {
    /// The locale key the settings page shows for it.
    pub fn key(self) -> &'static str {
        match self {
            LoopbackError::NoDevice => "devices.loopback.no_device",
            LoopbackError::NoInput => "devices.loopback.no_input",
            LoopbackError::Busy => "devices.loopback.busy",
            LoopbackError::Timeout => "devices.loopback.timeout",
            LoopbackError::NoInputData => "devices.loopback.no_data",
            LoopbackError::NotFound => "devices.loopback.not_found",
        }
    }
}

impl std::fmt::Display for LoopbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LoopbackError::NoDevice => "no audio device is open",
            LoopbackError::NoInput => "no input device is open",
            LoopbackError::Busy => "a measurement is already running",
            LoopbackError::Timeout => "the device did not play the measurement",
            LoopbackError::NoInputData => "no input arrived while measuring",
            LoopbackError::NotFound => {
                "the bursts were not found on any input: connect an output to an input with a cable"
            }
        })
    }
}

/// What a loopback measurement found.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LoopbackResult {
    /// Output frame written → output frame the processor reads it back on.
    pub round_trip: u32,
    /// What the drivers report for the same trip (input + output).
    pub reported: u32,
    /// How long the input frames waited in the ring before the probe took them, on
    /// average, measured as the processor measures `EngineStats::input_queue`.
    pub queue: i64,
    /// `round_trip − reported − queue`: the record offset to store. A recording moves a
    /// take by the reported latencies, the ring's wait *as it is then*, and this — so the
    /// offset is what the drivers do not say, and not a wait that differs from one
    /// stream start to the next (ADR 0025 §7).
    pub offset: i64,
    /// The input channel the bursts were found on (0-based).
    pub channel: usize,
    /// The peak over the correlation's spread (≥ [`MIN_CONFIDENCE`] to be accepted).
    pub confidence: f32,
    /// Bursts heard on their own at that lag (≥ [`MIN_BURSTS`] to be accepted).
    pub bursts: usize,
}

/// The probe signal: `BURSTS` Hann-windowed noise bursts (each its own noise, so no burst
/// correlates with another), a quarter second apart, after a tenth of a second of silence.
pub fn probe_signal(rate: f64) -> Vec<f32> {
    let (lead, spacing) = burst_layout(rate);
    let mut v = vec![0.0f32; lead + spacing * BURSTS];
    let mut seed = 0x2545_f491u32;
    for at in burst_starts(rate) {
        for i in 0..BURST_FRAMES {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let n = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let w = 0.5 - 0.5 * math::cos(std::f32::consts::TAU * i as f32 / BURST_FRAMES as f32);
            v[at + i] = BURST_LEVEL * n * w;
        }
    }
    v
}

/// The silence before the first burst and the distance between bursts, in frames.
fn burst_layout(rate: f64) -> (usize, usize) {
    (
        (rate * 0.1) as usize,
        ((rate * 0.25) as usize).max(BURST_FRAMES * 2),
    )
}

/// Where each burst starts in the probe signal.
fn burst_starts(rate: f64) -> impl Iterator<Item = usize> {
    let (lead, spacing) = burst_layout(rate);
    (0..BURSTS).map(move |b| lead + b * spacing)
}

/// How many bursts came back at `lag`, each judged on its own: its normalised
/// correlation with what was recorded there is at least [`MIN_BURST_CORRELATION`]. A
/// cable gives nearly 1 on every burst; a chance peak in noise or hum agrees with few.
pub fn bursts_agreeing(signal: &[f32], recorded: &[f32], lag: usize, rate: f64) -> usize {
    burst_starts(rate)
        .filter(|&at| {
            let Some(rec) = recorded.get(at + lag..at + lag + BURST_FRAMES) else {
                return false;
            };
            let sig = &signal[at..at + BURST_FRAMES];
            let (mut xy, mut xx, mut yy) = (0.0f64, 0.0f64, 0.0f64);
            for (x, y) in sig.iter().zip(rec) {
                xy += *x as f64 * *y as f64;
                xx += *x as f64 * *x as f64;
                yy += *y as f64 * *y as f64;
            }
            xx > 0.0 && yy > 0.0 && (xy / (xx * yy).sqrt()).abs() >= MIN_BURST_CORRELATION
        })
        .count()
}

/// The correlation a burst must show to count as heard.
pub const MIN_BURST_CORRELATION: f64 = 0.3;
/// Bursts that must be heard, of the eight.
pub const MIN_BURSTS: usize = 6;

/// One probe run, built on the UI thread with every buffer it needs.
#[derive(Debug)]
pub struct Probe {
    signal: Vec<f32>,
    /// The input, interleaved (`channels` per frame), from the first frame popped.
    recorded: Vec<f32>,
    channels: usize,
    /// Output frames played so far.
    pos: usize,
    /// Output frames to run for: the signal and a second for the trip back.
    total: usize,
    /// Interleaved samples recorded so far.
    filled: usize,
    /// The output frame the first input frame was popped on.
    first_pop: Option<usize>,
    /// Output frames played in silence waiting for the input to start.
    waited: usize,
    rate: f64,
    /// The input ring's wait (`crate::devices::queue_wait`) summed over every callback
    /// since the first pop, and how many there were.
    queue_sum: f64,
    queue_n: u32,
}

impl Probe {
    pub fn new(rate: f64, in_channels: usize) -> Self {
        let signal = probe_signal(rate);
        let total = signal.len() + rate as usize;
        let channels = in_channels.max(1);
        Probe {
            recorded: vec![0.0; total * channels],
            signal,
            channels,
            pos: 0,
            total,
            filled: 0,
            first_pop: None,
            waited: 0,
            rate,
            queue_sum: 0.0,
            queue_n: 0,
        }
    }

    /// How long input frames waited before the probe took them, on average (frames).
    pub fn queue(&self) -> f64 {
        if self.queue_n == 0 {
            0.0
        } else {
            self.queue_sum / self.queue_n as f64
        }
    }

    /// The probe signal played.
    pub fn signal(&self) -> &[f32] {
        &self.signal
    }

    /// What the input delivered (interleaved), its channel count, and the output frame
    /// the first input frame was popped on.
    pub fn recorded(&self) -> (&[f32], usize, Option<usize>) {
        (&self.recorded[..self.filled], self.channels, self.first_pop)
    }

    /// Play one callback's worth into `data` (`out_channels` interleaved) and take what the
    /// input delivered. Returns whether the probe is finished.
    pub fn run(
        &mut self,
        data: &mut [f32],
        out_channels: usize,
        input: &mut rtrb::Consumer<f32>,
    ) -> bool {
        self.run_clocked(data, out_channels, input, None)
    }

    /// [`Self::run`] with the input's clock (`InputClock::last_buffer`): the last
    /// buffer's length and age in frames, for the ring's wait.
    pub fn run_clocked(
        &mut self,
        data: &mut [f32],
        out_channels: usize,
        input: &mut rtrb::Consumer<f32>,
        clock: Option<(f64, f64)>,
    ) -> bool {
        let out_channels = out_channels.max(1);
        // The input first, as the processor pops it at the top of a block.
        let whole = input.slots() / self.channels * self.channels;
        if whole == 0 && self.first_pop.is_none() {
            // An input that has just opened can take a moment to deliver: the probe
            // waits for it in silence (two seconds at most), so the trip is measured
            // from a steady state.
            data.fill(0.0);
            self.waited += data.len() / out_channels;
            return self.waited >= self.total;
        }
        if whole > 0 {
            self.first_pop.get_or_insert(self.pos);
            let room = self.recorded.len() - self.filled;
            let take = whole.min(room);
            if let Ok(chunk) = input.read_chunk(whole) {
                let (a, b) = chunk.as_slices();
                for (i, x) in a.iter().chain(b).take(take).enumerate() {
                    self.recorded[self.filled + i] = *x;
                }
                chunk.commit_all();
            }
            self.filled += take;
        }
        // (Not once the recording is full: what the input delivers then is not counted.)
        if let Some(first) = self.first_pop.filter(|_| self.filled < self.recorded.len()) {
            // The frame placed at this callback's first output frame is input frame
            // `pos − first`; everything delivered after it was waiting too — as the
            // processor counts what is in its ring when it pops.
            let avail = (self.filled / self.channels) as f64 - (self.pos - first) as f64;
            let block = (data.len() / out_channels) as f64;
            self.queue_sum += crate::devices::queue_wait(avail, block, clock);
            self.queue_n += 1;
        }
        for frame in data.chunks_mut(out_channels) {
            let v = self.signal.get(self.pos).copied().unwrap_or(0.0);
            frame.fill(v);
            self.pos += 1;
        }
        self.pos >= self.total || self.filled == self.recorded.len()
    }
}

/// The output callback's side of the probe: where probes arrive, the one running, where
/// it goes back, and the input it listens to.
pub(crate) struct ProbeSlot {
    pub incoming: rtrb::Consumer<Box<Probe>>,
    pub done: rtrb::Producer<Box<Probe>>,
    pub input: Option<rtrb::Consumer<f32>>,
    pub active: Option<Box<Probe>>,
    pub telemetry: Arc<DeviceTelemetry>,
}

impl ProbeSlot {
    /// Run the probe if one is in flight. Returns whether it wrote the output (the
    /// processor must not run this callback).
    pub fn run(&mut self, data: &mut [f32], out_channels: usize) -> bool {
        let Some(input) = self.input.as_mut() else {
            return false;
        };
        if self.active.is_none() {
            match self.incoming.pop() {
                Ok(p) => {
                    // Whatever was waiting came before the probe started.
                    let stale = input.slots();
                    if let Ok(c) = input.read_chunk(stale) {
                        c.commit_all();
                    }
                    self.active = Some(p);
                }
                Err(_) => return false,
            }
        }
        let Some(p) = self.active.as_mut() else {
            return false;
        };
        let clock = self.telemetry.input_clock.last_buffer(p.rate);
        if p.run_clocked(data, out_channels, input, clock) {
            self.telemetry.probing.store(false, Ordering::Release);
            if let Some(p) = self.active.take() {
                // The return ring holds one probe; a full one cannot happen while the
                // handle runs one probe at a time.
                let _ = self.done.push(p);
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------------------
// The correlator

/// In-place complex FFT (radix 2, `re.len()` a power of two), with exact twiddles so a
/// quarter-million points keep their precision. `inverse` runs the unscaled inverse.
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let twiddles: Vec<(f64, f64)> = (0..n / 2)
        .map(|k| {
            let a = sign * std::f64::consts::TAU * k as f64 / n as f64;
            (math::cos(a), math::sin(a))
        })
        .collect();
    let mut len = 2;
    while len <= n {
        let step = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (cr, ci) = twiddles[k * step];
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Find `signal` in `recorded`: the lag (0..`max_lag`) at which they correlate most, and
/// how far that peak stands above the correlation's spread (its RMS over the lags
/// searched). `None` when either is silent.
pub fn find_delay(signal: &[f32], recorded: &[f32], max_lag: usize) -> Option<(usize, f32)> {
    if signal.is_empty() || recorded.is_empty() || max_lag == 0 {
        return None;
    }
    let n = (signal.len() + recorded.len()).next_power_of_two();
    let mut ar: Vec<f64> = recorded.iter().map(|&x| x as f64).collect();
    ar.resize(n, 0.0);
    let mut ai = vec![0.0; n];
    let mut br: Vec<f64> = signal.iter().map(|&x| x as f64).collect();
    br.resize(n, 0.0);
    let mut bi = vec![0.0; n];
    fft(&mut ar, &mut ai, false);
    fft(&mut br, &mut bi, false);
    // A · conj(B): correlation, recorded lagging the signal by the index.
    for k in 0..n {
        let (xr, xi) = (ar[k], ai[k]);
        let (yr, yi) = (br[k], -bi[k]);
        ar[k] = xr * yr - xi * yi;
        ai[k] = xr * yi + xi * yr;
    }
    fft(&mut ar, &mut ai, true);
    let lags = max_lag.min(recorded.len()).min(n);
    let corr = &ar[..lags];
    let (best, peak) = corr
        .iter()
        .enumerate()
        .map(|(i, v)| (i, v.abs()))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let spread = (corr.iter().map(|v| v * v).sum::<f64>() / lags as f64).sqrt();
    if peak <= 0.0 || spread <= 0.0 {
        return None;
    }
    Some((best, (peak / spread) as f32))
}

/// Evaluate a finished probe: the channel where the bursts stand out most, and the trip.
pub fn evaluate(probe: &Probe, rate: f64, reported: u32) -> Result<LoopbackResult, LoopbackError> {
    let Some(first_pop) = probe.first_pop else {
        return Err(LoopbackError::NoInputData);
    };
    let frames = probe.filled / probe.channels;
    let max_lag = rate as usize;
    let mut best: Option<(usize, usize, f32, usize)> = None;
    for ch in 0..probe.channels {
        let mono: Vec<f32> = (0..frames)
            .map(|f| probe.recorded[f * probe.channels + ch])
            .collect();
        if mono.iter().all(|x| x.abs() < 1e-6) {
            continue;
        }
        if let Some((lag, conf)) = find_delay(&probe.signal, &mono, max_lag) {
            let bursts = bursts_agreeing(&probe.signal, &mono, lag, rate);
            if best.is_none_or(|b| (bursts, conf) > (b.3, b.2)) {
                best = Some((ch, lag, conf, bursts));
            }
        }
    }
    match best {
        Some((channel, lag, confidence, bursts))
            if confidence >= MIN_CONFIDENCE && bursts >= MIN_BURSTS =>
        {
            let round_trip = (first_pop + lag) as u32;
            let queue = probe.queue().round() as i64;
            Ok(LoopbackResult {
                round_trip,
                reported,
                queue,
                offset: round_trip as i64 - reported as i64 - queue,
                channel,
                confidence,
                bursts,
            })
        }
        _ => Err(LoopbackError::NotFound),
    }
}

// ---------------------------------------------------------------------------------------
// The handle's API

/// The longest a probe may take before it is given up on: its own length and a margin
/// for a device that starts slowly.
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

impl EngineHandle {
    /// Start a loopback measurement: the engine's output plays the probe instead of the
    /// project for about three seconds (stop the transport first) and listens on the
    /// input. Poll [`Self::poll_loopback`] each frame for the result.
    pub fn begin_loopback(&mut self) -> Result<(), LoopbackError> {
        if !self.is_live() {
            return Err(LoopbackError::NoDevice);
        }
        if self.dev.probe_sent.is_some() {
            return Err(LoopbackError::Busy);
        }
        let channels = self.input_channel_count();
        let (Some(tx), Some(t)) = (self.dev.probe_tx.as_mut(), self.dev.telemetry.as_ref()) else {
            return Err(LoopbackError::NoInput);
        };
        if channels == 0 {
            return Err(LoopbackError::NoInput);
        }
        // A probe that came back after its measurement timed out is stale.
        if let Some(back) = self.dev.probe_back.as_mut() {
            while back.pop().is_ok() {}
        }
        let probe = Box::new(Probe::new(self.sample_rate, channels));
        t.probing.store(true, Ordering::Release);
        if tx.push(probe).is_err() {
            t.probing.store(false, Ordering::Release);
            return Err(LoopbackError::Busy);
        }
        self.dev.probe_sent = Some(Instant::now());
        Ok(())
    }

    /// Whether a loopback measurement is running.
    pub fn is_measuring(&self) -> bool {
        self.dev.probe_sent.is_some()
    }

    /// The measurement's result once it is in (`None` while it runs or when none was
    /// started). The reported latencies it is compared with are the ones in force when
    /// it finished.
    pub fn poll_loopback(&mut self) -> Option<Result<LoopbackResult, LoopbackError>> {
        match self.take_loopback_probe()? {
            Ok(probe) => {
                let reported = self.device_latency().round_trip();
                Some(evaluate(&probe, self.sample_rate, reported))
            }
            Err(e) => Some(Err(e)),
        }
    }

    /// The finished probe itself, for a diagnosis (`poll_loopback` evaluates it).
    #[doc(hidden)]
    pub fn take_loopback_probe(&mut self) -> Option<Result<Box<Probe>, LoopbackError>> {
        let sent = self.dev.probe_sent?;
        let back = self.dev.probe_back.as_mut()?.pop().ok();
        match back {
            Some(probe) => {
                self.dev.probe_sent = None;
                Some(Ok(probe))
            }
            None if sent.elapsed() > PROBE_TIMEOUT => {
                // The probe may still be inside the callback: leave it be (it comes
                // back and is dropped by the next measurement's poll) and stop waiting.
                self.dev.probe_sent = None;
                if let Some(t) = self.dev.telemetry.as_ref() {
                    t.probing.store(false, Ordering::Release);
                }
                Some(Err(LoopbackError::Timeout))
            }
            None => None,
        }
    }
}

/// Measure the round trip with a loopback cable, blocking until it is done (about three
/// seconds): returns the measured latency in frames minus what the drivers report — the
/// record offset to store. The settings page uses the non-blocking
/// [`EngineHandle::begin_loopback`] / [`EngineHandle::poll_loopback`] pair instead.
pub fn measure_loopback(handle: &mut EngineHandle) -> Result<i64, String> {
    handle.begin_loopback().map_err(|e| e.to_string())?;
    loop {
        std::thread::sleep(Duration::from_millis(20));
        // Keep the event ring drained while waiting, as a UI frame would.
        handle.poll();
        if let Some(r) = handle.poll_loopback() {
            return r.map(|r| r.offset).map_err(|e| e.to_string());
        }
    }
}
