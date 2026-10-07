//! Where the mix leaves the interface (round 20, audio setup): the main pair the master
//! plays on, a second pair for a headphone cue, and a test tone a musician uses to check
//! that the speakers they chose are the ones that sound.
//!
//! The processor writes the master into the first two channels of the device's buffer
//! (and nothing into the others). [`route`] runs after it, in the output callback: it
//! moves the master to the main pair when that is not 1–2, copies it into the cue pair at
//! the cue's level, and adds the tone on its pair. With the defaults (main 1–2, no cue, no
//! tone) it returns before touching a sample, so the output is bit-identical to the
//! processor's — the null test in `tests/routing.rs`.
//!
//! The UI sets the routing through atomics ([`OutputRouting`]); the callback reads them
//! once per buffer. Nothing here allocates, locks or blocks (ADR 0005).

use cantus_dsp::math;
use std::sync::atomic::{AtomicU32, Ordering};

/// "No pair": the cue and the tone are off.
pub const OFF: u32 = u32::MAX;

/// The test tone's level: −18 dBFS, the calibration level of most interfaces' meters —
/// clearly heard, never loud.
pub const TONE_LEVEL: f32 = 0.125_892_54;
/// The test tone's pitch, in hertz (A4).
pub const TONE_HZ: f64 = 440.0;
/// How long the tone takes to fade in or out, in seconds (no click when it starts).
pub const TONE_FADE: f64 = 0.02;

/// What the UI sets and the output callback reads.
#[derive(Debug)]
pub struct OutputRouting {
    /// The first channel (0-based) of the pair the master plays on.
    main: AtomicU32,
    /// The first channel of the cue pair, or [`OFF`].
    cue: AtomicU32,
    /// The cue's gain, linear, as `f32` bits.
    cue_gain: AtomicU32,
    /// The first channel of the pair the test tone plays on, or [`OFF`].
    tone: AtomicU32,
}

impl Default for OutputRouting {
    fn default() -> Self {
        OutputRouting {
            main: AtomicU32::new(0),
            cue: AtomicU32::new(OFF),
            cue_gain: AtomicU32::new(1.0f32.to_bits()),
            tone: AtomicU32::new(OFF),
        }
    }
}

/// One reading of the routing, for one buffer.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Route {
    pub main: usize,
    pub cue: Option<(usize, f32)>,
    pub tone: Option<usize>,
}

impl OutputRouting {
    pub fn set_main(&self, first: u16) {
        self.main.store(first as u32, Ordering::Relaxed);
    }

    /// The cue pair and its gain (linear); `None` turns it off.
    pub fn set_cue(&self, cue: Option<(u16, f32)>) {
        match cue {
            Some((first, gain)) => {
                let g = if gain.is_finite() {
                    gain.clamp(0.0, 4.0)
                } else {
                    0.0
                };
                self.cue_gain.store(g.to_bits(), Ordering::Relaxed);
                self.cue.store(first as u32, Ordering::Release);
            }
            None => self.cue.store(OFF, Ordering::Release),
        }
    }

    pub fn set_tone(&self, pair: Option<u16>) {
        self.tone
            .store(pair.map_or(OFF, |p| p as u32), Ordering::Release);
    }

    /// What the callback works with this buffer.
    pub fn read(&self) -> Route {
        let cue = self.cue.load(Ordering::Acquire);
        let tone = self.tone.load(Ordering::Acquire);
        Route {
            main: self.main.load(Ordering::Relaxed) as usize,
            cue: (cue != OFF).then(|| {
                (
                    cue as usize,
                    f32::from_bits(self.cue_gain.load(Ordering::Relaxed)),
                )
            }),
            tone: (tone != OFF).then_some(tone as usize),
        }
    }
}

/// The test tone's oscillator: its phase and how far it has faded in, kept between
/// buffers by the output callback.
#[derive(Copy, Clone, Debug, Default)]
pub struct Tone {
    phase: f64,
    /// 0 silent … 1 full level.
    env: f64,
    /// The pair it last played on, so it fades out where it was.
    pair: Option<usize>,
}

impl Tone {
    /// Whether it is still sounding (fading out counts).
    pub fn sounding(&self) -> bool {
        self.env > 0.0
    }
}

/// Route one buffer of `channels` interleaved channels at `rate`, after the processor has
/// written the master into channels 0 and 1. Allocates nothing.
pub fn route(data: &mut [f32], channels: usize, rate: f64, r: Route, tone: &mut Tone) {
    let channels = channels.max(1);
    // A pair the device does not have (fewer channels than when it was chosen): 1–2.
    let main = if r.main < channels { r.main } else { 0 };
    let cue = r.cue.filter(|(c, _)| *c < channels);
    let wants_tone = r.tone.filter(|p| *p < channels);
    if wants_tone.is_some() {
        tone.pair = wants_tone;
    }
    let tone_on = wants_tone.is_some() || tone.sounding();
    if main == 0 && cue.is_none() && !tone_on {
        return;
    }
    let step = 1.0 / (TONE_FADE * rate.max(1.0));
    let dphi = TONE_HZ / rate.max(1.0) * std::f64::consts::TAU;
    for frame in data.chunks_exact_mut(channels) {
        let l = frame[0];
        let r_ = if channels > 1 { frame[1] } else { l };
        if main != 0 {
            frame[0] = 0.0;
            if channels > 1 {
                frame[1] = 0.0;
            }
            frame[main] += l;
            if main + 1 < channels {
                frame[main + 1] += r_;
            }
        }
        if let Some((c, g)) = cue {
            frame[c] += l * g;
            if c + 1 < channels {
                frame[c + 1] += r_ * g;
            }
        }
        if tone_on {
            let target = if wants_tone.is_some() { 1.0 } else { 0.0 };
            if tone.env < target {
                tone.env = (tone.env + step).min(1.0);
            } else if tone.env > target {
                tone.env = (tone.env - step).max(0.0);
            }
            let x = (math::sin(tone.phase) * tone.env) as f32 * TONE_LEVEL;
            tone.phase = (tone.phase + dphi) % std::f64::consts::TAU;
            if let Some(p) = tone.pair.filter(|p| *p < channels) {
                frame[p] += x;
                if p + 1 < channels {
                    frame[p + 1] += x;
                }
            }
        }
    }
    if !tone.sounding() && wants_tone.is_none() {
        tone.pair = None;
        tone.phase = 0.0;
    }
}

// ---------------------------------------------------------------------------------------
// The handle's side

impl crate::backend::EngineHandle {
    /// Play the master on the pair starting at channel `first` (0-based; 0 is 1–2).
    pub fn set_main_output(&self, first: u16) {
        if let Some(r) = &self.dev.routing {
            r.set_main(first);
        }
    }

    /// A copy of the master on a second pair, for headphones, at `gain` (linear); `None`
    /// turns it off.
    pub fn set_cue_output(&self, cue: Option<(u16, f32)>) {
        if let Some(r) = &self.dev.routing {
            r.set_cue(cue);
        }
    }

    /// The test tone on the pair starting at `first`; `None` stops it (it fades out).
    pub fn set_test_tone(&self, first: Option<u16>) {
        if let Some(r) = &self.dev.routing {
            r.set_tone(first);
        }
    }

    /// The routing as the output callback will read it next (defaults without a device).
    pub fn output_route(&self) -> Route {
        self.dev
            .routing
            .as_ref()
            .map(|r| r.read())
            .unwrap_or_default()
    }

    /// Channels the output stream plays (the device's own count; 2 without a device).
    pub fn output_channel_count(&self) -> usize {
        self.channels
    }

    /// Whether the device is **gone**: a stream said its device is not available
    /// (unplugged, its driver unloaded). A driver that stopped calling back or asked to be
    /// reset is not lost — see [`Self::device_state`] (round 21). The handle keeps working
    /// — messages are accepted and dropped — so the session stays editable until the
    /// device is opened again.
    pub fn device_lost(&self) -> bool {
        self.device_state() == crate::devices::DeviceState::Lost
    }

    /// Underruns and overruns the driver reported since the device opened.
    pub fn xruns(&self) -> u64 {
        self.dev
            .telemetry
            .as_ref()
            .map_or(0, |t| t.xruns.load(std::sync::atomic::Ordering::Relaxed))
    }
}
