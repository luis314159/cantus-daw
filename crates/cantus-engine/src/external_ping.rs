//! The automatic latency ping of an external unit (ADR 0094 §3).
//!
//! Two pings, one for each thing a unit's trip can be asked:
//!
//! * [`AudioPing`] — **an External Effect, or the interface's own trip.** A train of noise
//!   bursts (the loopback's, `loopback::probe_signal`) goes out of a pair of outputs and is
//!   found on the return pair by cross-correlation. The number is the round trip *as the
//!   engine sees it*: from the output frame written to the output frame the processor pops
//!   it back from the input ring — exactly the delay compensation must apply (ADR 0025 §7).
//!   Each burst is also located on its own, so the answer carries its **jitter**.
//! * [`OnsetPing`] — **an External Instrument.** A note-on is stamped at a known frame
//!   and the first frame the return rises above the noise is found, four times; the answer
//!   is the median. It includes the MIDI path (the out thread's timing, the cable, the
//!   synth's own response) as well as the audio return, which no audio loopback could
//!   measure.
//!
//! Both run **block by block inside the audio callback** and never allocate there: every
//! buffer is made in `new` on the interface thread; the heavy part — [`AudioPing::evaluate`]
//! — runs on whichever thread the finished ping is handed back to (as the loopback's
//! probe is). Neither opens a device nor touches the project: the caller owns the I/O and
//! stores the result with `Command::SetExternalLatency`.
//!
//! The tests drive both through a [`Wire`] — a virtual interface with a delay, a gain, a
//! polarity, noise and a chosen channel — so the arithmetic is proved to the sample without
//! hardware; what hardware adds (the driver's own jitter) is the owner's to run.

use crate::loopback::{self, BURSTS, BURST_FRAMES, MIN_BURSTS, MIN_CONFIDENCE};
use cantus_core::external_unit::{Measured, Pair};

/// Why a ping gave no number.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PingError {
    /// Nothing came back: the return is silent (a cable, a mute, the wrong pair).
    Silent,
    /// The input never delivered a block while the ping ran (no input open).
    NoInput,
    /// Something came back but not the ping: too few bursts agreed, or the peak did not
    /// stand out (hum, another source on the return).
    NotFound,
    /// The input lost blocks while the ping ran: the number would be wrong.
    Dropped,
    /// The bursts disagreed with each other by more than [`MAX_JITTER`] frames.
    Unstable,
    /// A note never rose above the noise (a muted patch, the wrong channel, a port that is
    /// not connected).
    NoNote,
    /// Another ping is running, or the transport is.
    Busy,
    /// The transport started while it ran.
    Aborted,
    /// It never came back from the engine (the device stopped, the engine restarted).
    TimedOut,
}

impl PingError {
    /// The locale key the screen says it with.
    pub fn key(self) -> &'static str {
        match self {
            PingError::Silent => "external.ping.silent",
            PingError::NoInput => "external.ping.no_input",
            PingError::NotFound => "external.ping.not_found",
            PingError::Dropped => "external.ping.dropped",
            PingError::Unstable => "external.ping.unstable",
            PingError::NoNote => "external.ping.no_note",
            PingError::Busy => "external.ping.busy",
            PingError::Aborted => "external.ping.aborted",
            PingError::TimedOut => "external.ping.timed_out",
        }
    }
}

/// The most the bursts may differ from each other, in frames, before the measurement is
/// refused as unstable. (A healthy loopback differs by 0; a driver that jitters by a
/// buffer is told so rather than guessed at.)
pub const MAX_JITTER: u32 = 8;
/// How far around the global lag each burst is searched on its own, in frames.
const LOCAL: usize = 96;

/// A finished audio ping.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PingResult {
    /// The trip, in frames at the ping's rate: the median of the bursts.
    pub frames: u32,
    /// The widest disagreement between bursts, in frames.
    pub jitter: u32,
    /// The correlation's peak over its spread.
    pub confidence: f32,
    /// Bursts heard on their own.
    pub bursts: usize,
    /// The channel of the return pair it was found on (0-based within the pair).
    pub channel: u16,
    /// The return came back with its polarity inverted (a flipped cable or a unit that
    /// inverts): said, not corrected.
    pub inverted: bool,
}

impl PingResult {
    /// What the project stores: the trip, and what the interface reported when it was
    /// measured (`now`), so a later buffer size re-bases it (ADR 0094 §4).
    pub fn measured(&self, rate: u32, now: &cantus_core::external_unit::Now) -> Measured {
        Measured {
            frames: self.frames,
            rate,
            confidence: self.confidence,
            rtt: now.rtt,
            buffer: now.buffer,
            device: now.device.clone(),
        }
    }
}

// ---------------------------------------------------------------------------------------
// The audio ping

/// The audio ping in flight: it plays the burst train on a send pair and records the
/// return pair.
#[derive(Debug)]
pub struct AudioPing {
    signal: Vec<f32>,
    /// What the return delivered, interleaved (`width` per frame), from the first block.
    recorded: Vec<f32>,
    width: usize,
    /// Frames played so far.
    pos: usize,
    /// Frames recorded so far.
    filled: usize,
    total: usize,
    rate: f64,
    started: bool,
    dropped: bool,
    /// Frames waited before the first input block (the clock starts with it).
    waited: usize,
}

impl AudioPing {
    /// A ping for a return of `ret.width()` channels at `rate`. Allocates: not for the audio
    /// thread.
    pub fn new(rate: f64, ret: Pair) -> Self {
        let signal = loopback::probe_signal(rate);
        let total = signal.len() + rate as usize;
        let width = usize::from(ret.width());
        AudioPing {
            recorded: vec![0.0; total * width],
            signal,
            width,
            pos: 0,
            filled: 0,
            total,
            rate,
            started: false,
            dropped: false,
            waited: 0,
        }
    }

    /// One callback's block of `out.len()` frames. `input` is what the return pair
    /// delivered this callback (interleaved, `width` per frame; `None` when the input
    /// delivered nothing). `out` receives the mono signal for the send pair. Returns whether
    /// the ping is finished. Allocation-free.
    pub fn block(&mut self, input: Option<&[f32]>, out: &mut [f32]) -> bool {
        let n = out.len();
        match input {
            Some(inp) if !inp.is_empty() => {
                self.started = true;
                let frames = (inp.len() / self.width).min(n);
                let room = self.recorded.len() - self.filled;
                let take = (frames * self.width).min(room);
                self.recorded[self.filled..self.filled + take].copy_from_slice(&inp[..take]);
                // A short block is a gap: pad it so the timeline keeps its place.
                let missing = n.saturating_sub(frames) * self.width;
                let pad = missing.min(room - take);
                self.recorded[self.filled + take..self.filled + take + pad].fill(0.0);
                if missing > 0 {
                    self.dropped = true;
                }
                self.filled += take + pad;
            }
            _ if self.started => {
                self.dropped = true;
                let room = self.recorded.len() - self.filled;
                let pad = (n * self.width).min(room);
                self.recorded[self.filled..self.filled + pad].fill(0.0);
                self.filled += pad;
            }
            _ => {
                // An input that has just opened can take a moment: wait in silence (two
                // seconds at most), so the trip is measured from a steady state.
                self.waited += n;
                out.fill(0.0);
                return self.waited as f64 >= 2.0 * self.rate;
            }
        }
        for o in out.iter_mut() {
            *o = self.signal.get(self.pos).copied().unwrap_or(0.0);
            self.pos += 1;
        }
        self.pos >= self.total || self.filled >= self.recorded.len()
    }

    /// The round trip. Heavy (an FFT of the whole recording): call it off the audio thread.
    pub fn evaluate(&self) -> Result<PingResult, PingError> {
        if !self.started {
            return Err(PingError::NoInput);
        }
        if self.dropped {
            return Err(PingError::Dropped);
        }
        let frames = self.filled / self.width;
        let max_lag = self.rate as usize;
        let mut best: Option<(usize, usize, f32, usize, Vec<f32>)> = None;
        for ch in 0..self.width {
            let mono: Vec<f32> = (0..frames)
                .map(|f| self.recorded[f * self.width + ch])
                .collect();
            if mono.iter().all(|x| x.abs() < 1e-6) {
                continue;
            }
            if let Some((lag, conf)) = loopback::find_delay(&self.signal, &mono, max_lag) {
                let bursts = loopback::bursts_agreeing(&self.signal, &mono, lag, self.rate);
                if best.as_ref().is_none_or(|b| (bursts, conf) > (b.3, b.2)) {
                    best = Some((ch, lag, conf, bursts, mono));
                }
            }
        }
        let Some((channel, lag, confidence, bursts, mono)) = best else {
            return Err(PingError::Silent);
        };
        if confidence < MIN_CONFIDENCE || bursts < MIN_BURSTS {
            return Err(PingError::NotFound);
        }
        // Each burst on its own, around the global lag.
        let mut lags: Vec<usize> = Vec::with_capacity(BURSTS);
        let mut sign = 0.0f64;
        for at in loopback::burst_starts(self.rate) {
            let sig = &self.signal[at..at + BURST_FRAMES];
            let lo = (at + lag).saturating_sub(LOCAL).max(at);
            let hi = at + lag + LOCAL;
            let mut top: Option<(usize, f64, f64)> = None;
            for cand in lo..=hi {
                let Some(rec) = mono.get(cand..cand + BURST_FRAMES) else {
                    break;
                };
                let (mut xy, mut yy) = (0.0f64, 0.0f64);
                for (x, y) in sig.iter().zip(rec) {
                    xy += f64::from(*x) * f64::from(*y);
                    yy += f64::from(*y) * f64::from(*y);
                }
                let score = xy.abs() / yy.sqrt().max(1e-12);
                if top.is_none_or(|t| score > t.1) {
                    top = Some((cand - at, score, xy));
                }
            }
            if let Some((l, score, xy)) = top {
                if score > 0.0 {
                    lags.push(l);
                    sign += xy.signum();
                }
            }
        }
        if lags.len() < MIN_BURSTS {
            return Err(PingError::NotFound);
        }
        lags.sort_unstable();
        let jitter = (lags[lags.len() - 1] - lags[0]) as u32;
        if jitter > MAX_JITTER {
            return Err(PingError::Unstable);
        }
        Ok(PingResult {
            frames: lags[lags.len() / 2] as u32,
            jitter,
            confidence,
            bursts,
            channel: channel as u16,
            inverted: sign < 0.0,
        })
    }
}

// ---------------------------------------------------------------------------------------
// The onset ping

/// Repetitions of the note.
pub const ONSET_REPS: usize = 4;

/// What the onset ping asks the caller to do this block.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct OnsetStep {
    /// A note-on stamped `offset` frames into this block.
    pub on: Option<u32>,
    /// A note-off stamped `offset` frames into this block.
    pub off: Option<u32>,
    pub done: bool,
}

/// The onset ping in flight. The caller stamps the notes the steps ask for (key and
/// velocity are the caller's), and hands each block's return to [`Self::block`].
#[derive(Debug)]
pub struct OnsetPing {
    rate: f64,
    /// Block starts, in frames since the first input block.
    t: u64,
    started: bool,
    waited: usize,
    dropped: bool,
    noise: f32,
    threshold: f32,
    /// Frames of each repetition's note-on and note-off, since the start.
    on_at: [u64; ONSET_REPS],
    off_at: [u64; ONSET_REPS],
    /// The first frame (since the note-on) the return crossed the threshold.
    found: [Option<u64>; ONSET_REPS],
    /// The return's peak in the first 20 ms after each onset: the note's level.
    peak: [f32; ONSET_REPS],
    done_at: u64,
}

impl OnsetPing {
    pub fn new(rate: f64) -> Self {
        let lead = (rate * 0.15) as u64;
        let gate = (rate * 0.3) as u64;
        let spacing = (rate * 1.2) as u64;
        let mut on_at = [0; ONSET_REPS];
        let mut off_at = [0; ONSET_REPS];
        for i in 0..ONSET_REPS {
            on_at[i] = lead + i as u64 * spacing;
            off_at[i] = on_at[i] + gate;
        }
        OnsetPing {
            rate,
            t: 0,
            started: false,
            waited: 0,
            dropped: false,
            noise: 0.0,
            threshold: 0.0,
            on_at,
            off_at,
            found: [None; ONSET_REPS],
            peak: [0.0; ONSET_REPS],
            done_at: on_at[ONSET_REPS - 1] + spacing,
        }
    }

    /// One callback's block of `n` frames and what the return delivered (mono: the caller
    /// sums or picks a channel; `None`: nothing arrived). Allocation-free.
    pub fn block(&mut self, input: Option<&[f32]>, n: usize) -> OnsetStep {
        let mut step = OnsetStep::default();
        match input {
            Some(i) if !i.is_empty() => {
                self.started = true;
                if i.len() < n {
                    self.dropped = true;
                }
            }
            _ if self.started => self.dropped = true,
            _ => {
                self.waited += n;
                step.done = self.waited as f64 >= 2.0 * self.rate;
                return step;
            }
        }
        let t0 = self.t;
        let t1 = t0 + n as u64;
        // The noise floor: the lead-in's peak. The threshold is fixed when the first note
        // is about to be sent.
        if let Some(i) = input {
            let lead_end = self.on_at[0];
            for (k, x) in i.iter().enumerate() {
                let t = t0 + k as u64;
                if t < lead_end {
                    self.noise = self.noise.max(x.abs());
                }
            }
        }
        if t0 <= self.on_at[0] && self.on_at[0] < t1 {
            self.threshold = (self.noise * 4.0).max(0.003);
        }
        for r in 0..ONSET_REPS {
            if (t0..t1).contains(&self.on_at[r]) {
                step.on = Some((self.on_at[r] - t0) as u32);
            }
            if (t0..t1).contains(&self.off_at[r]) {
                step.off = Some((self.off_at[r] - t0) as u32);
            }
            // Watching from the note-on to its note-off and a little after.
            let watch_end = self.off_at[r] + (self.rate * 0.1) as u64;
            if self.found[r].is_none() && self.threshold > 0.0 && t1 > self.on_at[r] {
                if let Some(i) = input {
                    for (k, x) in i.iter().enumerate().take(n) {
                        let t = t0 + k as u64;
                        if t >= self.on_at[r] && t < watch_end && x.abs() > self.threshold {
                            self.found[r] = Some(t - self.on_at[r]);
                            break;
                        }
                    }
                }
            }
            // The note's level, over the 20 ms after its onset.
            if let (Some(f), Some(i)) = (self.found[r], input) {
                let (from, to) = (
                    self.on_at[r] + f,
                    self.on_at[r] + f + (self.rate * 0.02) as u64,
                );
                for (k, x) in i.iter().enumerate().take(n) {
                    let t = t0 + k as u64;
                    if t >= from && t < to {
                        self.peak[r] = self.peak[r].max(x.abs());
                    }
                }
            }
        }
        self.t = t1;
        step.done = t1 >= self.done_at;
        step
    }

    /// The trip: the median of the repetitions, which must agree within [`MAX_JITTER`]
    /// frames. Cheap (a sort of four numbers).
    pub fn evaluate(&self) -> Result<PingResult, PingError> {
        if !self.started {
            return Err(PingError::NoInput);
        }
        if self.dropped {
            return Err(PingError::Dropped);
        }
        let mut got: Vec<u64> = self.found.iter().flatten().copied().collect();
        if got.len() < ONSET_REPS - 1 {
            return Err(PingError::NoNote);
        }
        got.sort_unstable();
        let jitter = (got[got.len() - 1] - got[0]) as u32;
        // MIDI is not sample-accurate: the notes may differ by a millisecond; the limit is
        // in time, not in frames.
        let limit = (self.rate * 0.002) as u32;
        if jitter > limit.max(MAX_JITTER) {
            return Err(PingError::Unstable);
        }
        Ok(PingResult {
            frames: got[got.len() / 2] as u32,
            jitter,
            // The notes' level over the room's noise (their median), divided by one plus the
            // repeats' spread in frames: a loud, steady answer is believed more than a quiet,
            // wandering one — never a constant.
            confidence: {
                let mut p: Vec<f32> = self.peak.iter().copied().filter(|x| *x > 0.0).collect();
                p.sort_by(f32::total_cmp);
                let level = p.get(p.len() / 2).copied().unwrap_or(0.0);
                (level / self.noise.max(1e-4)).min(1_000.0) / (1.0 + jitter as f32)
            },
            bursts: got.len(),
            channel: 0,
            inverted: false,
        })
    }
}

// ---------------------------------------------------------------------------------------
// The job the engine runs

/// What a ping measures and with what.
#[derive(Debug)]
pub enum PingKind {
    /// Burst train out of `send`, found on `ret` (an effect, or the interface's trip).
    Audio {
        ping: AudioPing,
        send: Pair,
        ret: Pair,
    },
    /// A note on `channel` of the unit's `route` (key `key`), the return `ret` watched.
    Onset {
        ping: OnsetPing,
        route: u64,
        channel: u8,
        key: u8,
        ret: Pair,
    },
}

/// A ping on its way to the engine and back (built on the interface thread, run block by
/// block on the audio thread, evaluated on the interface thread again).
#[derive(Debug)]
pub struct PingJob {
    pub unit: cantus_core::ExternalUnitId,
    pub kind: PingKind,
    /// Set by the engine when the run ends (or is refused).
    pub finished: bool,
    /// Why it was refused or stopped early, when it was.
    pub refused: Option<PingError>,
    /// The output zeroed while it ran: whether the input repeated frames (an underrun).
    pub underrun: bool,
}

impl PingJob {
    pub fn audio(unit: cantus_core::ExternalUnitId, rate: f64, send: Pair, ret: Pair) -> Self {
        PingJob {
            unit,
            kind: PingKind::Audio {
                ping: AudioPing::new(rate, ret),
                send,
                ret,
            },
            finished: false,
            refused: None,
            underrun: false,
        }
    }

    pub fn onset(
        unit: cantus_core::ExternalUnitId,
        rate: f64,
        route: u64,
        channel: u8,
        key: u8,
        ret: Pair,
    ) -> Self {
        PingJob {
            unit,
            kind: PingKind::Onset {
                ping: OnsetPing::new(rate),
                route,
                channel,
                key,
                ret,
            },
            finished: false,
            refused: None,
            underrun: false,
        }
    }

    /// The result (heavy for an audio ping: off the audio thread).
    pub fn evaluate(&self) -> Result<PingResult, PingError> {
        if let Some(e) = self.refused {
            return Err(e);
        }
        if self.underrun {
            return Err(PingError::Dropped);
        }
        match &self.kind {
            PingKind::Audio { ping, .. } => ping.evaluate(),
            PingKind::Onset { ping, .. } => ping.evaluate(),
        }
    }
}

// ---------------------------------------------------------------------------------------
// The virtual interface the tests (and the emulation of a unit without hardware) use

/// A virtual interface: what is written comes back after `delay` frames, scaled, flipped
/// and with noise, on one channel of a `width`-channel return.
#[derive(Debug)]
pub struct Wire {
    line: Vec<f32>,
    delay: usize,
    at: usize,
    pub gain: f32,
    pub width: usize,
    pub channel: usize,
    pub noise: f32,
    seed: u32,
}

impl Wire {
    pub fn new(delay: usize, width: usize) -> Wire {
        Wire {
            line: vec![0.0; delay],
            delay,
            at: 0,
            gain: 1.0,
            width,
            channel: 0,
            noise: 0.0,
            seed: 0x1234_5678,
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    /// Push `out` (mono) in and return the interleaved return block of the same length.
    /// The return of a frame is the sample written `delay` frames ago.
    pub fn cycle(&mut self, out: &[f32]) -> Vec<f32> {
        let mut back = vec![0.0; out.len() * self.width];
        for (i, x) in out.iter().enumerate() {
            let y = if self.delay == 0 {
                *x
            } else {
                let y = self.line[self.at];
                self.line[self.at] = *x;
                self.at = (self.at + 1) % self.line.len();
                y
            };
            for c in 0..self.width {
                let noise = self.noise * self.rand();
                back[i * self.width + c] = noise
                    + if c == self.channel {
                        y * self.gain
                    } else {
                        0.0
                    };
            }
        }
        back
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn run_audio(
        delay: usize,
        block: usize,
        wire: &mut Wire,
        ret: Pair,
    ) -> Result<PingResult, PingError> {
        let mut ping = AudioPing::new(RATE, ret);
        let mut out = vec![0.0f32; block];
        let mut guard = 0;
        // The first blocks have no input (a stream that has just opened).
        let mut back: Option<Vec<f32>> = None;
        loop {
            let done = ping.block(back.as_deref(), &mut out);
            back = Some(wire.cycle(&out));
            guard += 1;
            if done || guard > 100_000 {
                break;
            }
        }
        let _ = delay;
        ping.evaluate()
    }

    #[test]
    fn the_audio_ping_finds_the_delay_to_the_sample_at_every_block_size() {
        for &delay in &[0usize, 1, 37, 480, 612, 4_801, 24_000] {
            for &block in &[32usize, 64, 128, 256, 480, 1024] {
                let mut wire = Wire::new(delay, 2);
                let r = run_audio(delay, block, &mut wire, Pair::stereo(0)).unwrap();
                // The wire's own delay line is `delay` frames, and one block of the cycle
                // sits between a block's output and the next block's input.
                assert_eq!(
                    r.frames as usize,
                    delay + block,
                    "delay {delay} block {block}"
                );
                assert_eq!(r.jitter, 0);
                assert!(!r.inverted);
            }
        }
    }

    #[test]
    fn the_audio_ping_sees_gain_polarity_noise_and_the_right_channel() {
        let mut w = Wire::new(700, 2);
        w.gain = 0.05;
        w.channel = 1;
        w.noise = 0.004;
        let r = run_audio(700, 128, &mut w, Pair::stereo(0)).unwrap();
        assert_eq!(r.frames, 700 + 128);
        assert_eq!(r.channel, 1);
        let mut w = Wire::new(700, 2);
        w.gain = -0.5;
        let r = run_audio(700, 128, &mut w, Pair::stereo(0)).unwrap();
        assert!(r.inverted, "a flipped return is said");
        assert_eq!(r.frames, 828);
        // A mono return.
        let mut w = Wire::new(100, 1);
        let r = run_audio(100, 64, &mut w, Pair::mono(3)).unwrap();
        assert_eq!(r.frames, 164);
    }

    #[test]
    fn an_empty_noisy_or_missing_return_is_said_and_never_guessed() {
        // Cable out: silence.
        let mut w = Wire::new(300, 2);
        w.gain = 0.0;
        assert_eq!(
            run_audio(300, 128, &mut w, Pair::stereo(0)).unwrap_err(),
            PingError::Silent
        );
        // Only hum on the return.
        let mut w = Wire::new(300, 2);
        w.gain = 0.0;
        w.noise = 0.3;
        assert_eq!(
            run_audio(300, 128, &mut w, Pair::stereo(0)).unwrap_err(),
            PingError::NotFound
        );
        // No input ever delivered.
        let mut ping = AudioPing::new(RATE, Pair::stereo(0));
        let mut out = vec![0.0f32; 256];
        let mut n = 0;
        while !ping.block(None, &mut out) {
            n += 1;
            assert!(n < 10_000);
        }
        assert_eq!(ping.evaluate().unwrap_err(), PingError::NoInput);
        // A block lost in the middle.
        let mut ping = AudioPing::new(RATE, Pair::stereo(0));
        let mut w = Wire::new(300, 2);
        let mut back: Option<Vec<f32>> = None;
        let mut i = 0;
        loop {
            let input = if i == 40 { None } else { back.as_deref() };
            let done = ping.block(input, &mut out);
            back = Some(w.cycle(&out));
            i += 1;
            if done {
                break;
            }
        }
        assert_eq!(ping.evaluate().unwrap_err(), PingError::Dropped);
    }

    #[test]
    fn a_jittering_return_is_refused_as_unstable() {
        // A wire whose delay alternates between bursts' worth: 600 then 640 frames.
        let mut ping = AudioPing::new(RATE, Pair::mono(0));
        let mut out = vec![0.0f32; 64];
        let mut back: Option<Vec<f32>> = None;
        let mut sent: Vec<f32> = Vec::new();
        let mut got: Vec<f32> = Vec::new();
        let mut t = 0usize;
        loop {
            let done = ping.block(back.as_deref(), &mut out);
            sent.extend_from_slice(&out);
            let mut b = vec![0.0f32; 64];
            for (i, v) in b.iter_mut().enumerate() {
                let now = t + i;
                // Which burst is it? Two of the eight are late by 40 more frames.
                let burst = (now.saturating_sub(4_800)) / 12_000;
                let d = if burst % 4 == 3 { 640 } else { 600 };
                *v = now
                    .checked_sub(d)
                    .and_then(|k| sent.get(k))
                    .copied()
                    .unwrap_or(0.0);
            }
            got.extend_from_slice(&b);
            back = Some(b);
            t += 64;
            if done {
                break;
            }
        }
        assert_eq!(ping.evaluate().unwrap_err(), PingError::Unstable);
    }

    fn run_onset(
        latency: usize,
        block: usize,
        attack: usize,
        level: f32,
        noise: f32,
    ) -> Result<PingResult, PingError> {
        let mut ping = OnsetPing::new(RATE);
        let mut seed = 0x9e37_79b9u32;
        let mut t = 0usize;
        // The synth: a sine that starts `latency + attack` frames after the note-on's frame
        // and stops at the note-off's.
        let mut notes: Vec<(usize, bool)> = Vec::new();
        let mut back: Option<Vec<f32>> = None;
        let mut guard = 0;
        loop {
            let step = ping.block(back.as_deref(), block);
            if let Some(o) = step.on {
                notes.push((t + o as usize, true));
            }
            if let Some(o) = step.off {
                notes.push((t + o as usize, false));
            }
            let mut b = vec![0.0f32; block];
            for (i, v) in b.iter_mut().enumerate() {
                let now = t + i;
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *v = noise * (seed as f32 / u32::MAX as f32 * 2.0 - 1.0);
                // Sounding if the last note event heard (a note's frame + latency) is an on.
                let heard = notes.iter().rfind(|(at, _)| at + latency <= now);
                if let Some(&(at, true)) = heard {
                    let age = now - (at + latency);
                    if age >= attack {
                        *v += level * math_sin(age as f32 * 0.07 + 1.0);
                    } else {
                        *v += level * 0.01 * math_sin(age as f32 * 0.07 + 1.0);
                    }
                }
            }
            back = Some(b);
            t += block;
            guard += 1;
            if step.done || guard > 100_000 {
                break;
            }
        }
        ping.evaluate()
    }

    fn math_sin(x: f32) -> f32 {
        cantus_dsp::math::sin(x)
    }

    #[test]
    fn the_onset_ping_finds_the_note_to_the_sample_through_noise() {
        for &latency in &[0usize, 200, 1_440, 9_600] {
            for &block in &[64usize, 256, 512] {
                let r = run_onset(latency, block, 0, 0.5, 0.0005).unwrap();
                // A note stamped in a block can only be heard on the input of the block
                // after it: the trip is the latency and one block, as the audio ping's.
                assert_eq!(
                    r.frames as usize,
                    latency + block,
                    "latency {latency} block {block}"
                );
                assert_eq!(r.jitter, 0);
            }
        }
        // A quiet synth, a hot room.
        let r = run_onset(1_000, 128, 0, 0.05, 0.004).unwrap();
        assert!((r.frames as i64 - 1_128).abs() <= 2, "{}", r.frames);
    }

    #[test]
    fn the_onset_confidence_is_the_notes_level_over_the_noise_not_a_constant() {
        let clean = run_onset(1_000, 128, 0, 0.5, 0.0005).unwrap();
        let noisy = run_onset(1_000, 128, 0, 0.5, 0.01).unwrap();
        assert!(clean.confidence > 100.0, "{}", clean.confidence);
        assert!(
            noisy.confidence < clean.confidence / 5.0,
            "{} vs {}",
            noisy.confidence,
            clean.confidence
        );
    }

    #[test]
    fn a_silent_synth_is_no_note_and_a_missing_input_is_no_input() {
        assert_eq!(
            run_onset(500, 128, 0, 0.0, 0.0005).unwrap_err(),
            PingError::NoNote
        );
        let mut p = OnsetPing::new(RATE);
        let mut n = 0;
        while !p.block(None, 256).done {
            n += 1;
            assert!(n < 10_000);
        }
        assert_eq!(p.evaluate().unwrap_err(), PingError::NoInput);
    }

    #[test]
    fn the_result_is_what_the_project_stores() {
        let r = PingResult {
            frames: 612,
            jitter: 0,
            confidence: 40.0,
            bursts: 8,
            channel: 0,
            inverted: false,
        };
        let m = r.measured(48_000, &cantus_core::external_unit::Now::default());
        assert!(m.is_valid());
        assert_eq!((m.frames, m.rate), (612, 48_000));
    }

    #[test]
    fn no_step_of_either_ping_allocates() {
        // The buffers are made in `new`; a block only copies into them. (The counting
        // allocator is in `tests/rt_memory.rs`; here the capacity never grows.)
        let mut ping = AudioPing::new(RATE, Pair::stereo(0));
        let cap = ping.recorded.capacity();
        let mut w = Wire::new(100, 2);
        let mut out = vec![0.0f32; 256];
        let mut back: Option<Vec<f32>> = None;
        while !ping.block(back.as_deref(), &mut out) {
            back = Some(w.cycle(&out));
        }
        assert_eq!(ping.recorded.capacity(), cap);
    }
}
