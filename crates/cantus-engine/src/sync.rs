//! The sync seam (round 33, ADR 0078 §4, ADR 0079): the one way an outside clock reaches
//! the audio thread, and the one way the audio thread's clock reaches outside.
//!
//! The engine knows nothing of Ableton Link, of MIDI clock or of time code here — only an
//! **abstract timeline** a source publishes (a tempo, a beat or a song time at a host
//! time, playing or not, a quantum) and a **ring of proposals** the audio thread writes
//! back (a start, a stop, a tempo of its own). Link (`cantus-link`, GPL, through the
//! application's bridge), MIDI clock in and MTC in ([`clock`], [`mtc`]) are three sources
//! of that one seam; one at a time is followed (`SourceKind`).
//!
//! Going the other way, the audio thread stamps every MIDI clock and time-code message with
//! the sample it belongs to and pushes it into a ring; a thread of its own ([`out`])
//! sends each when its audio reaches the speakers. The audio thread also publishes the line
//! from its sample clock to host time ([`HostTimeFilter`]), which both directions use.
//!
//! Everything the audio thread touches is lock-free and allocation-free: a seqlock of
//! atomics read with a bounded number of tries, `rtrb` rings (ADR 0005).

use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

// The sources' own state machines and the MIDI-out thread.
#[path = "sync_clock.rs"]
pub mod clock;
#[path = "sync_mtc.rs"]
pub mod mtc;
#[path = "sync_out.rs"]
pub mod out;

static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);

/// The host clock every party of the seam reads: µs since this process first asked. A
/// monotonic counter (`QueryPerformanceCounter`, `clock_gettime`), no system call that
/// blocks, so the audio thread may read it (it already reads `Instant::now` each block).
pub fn host_micros() -> i64 {
    ORIGIN.elapsed().as_micros() as i64
}

/// Which outside clock a timeline comes from.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceKind {
    /// Nothing is followed: the transport plays the project's own tempo map.
    #[default]
    None,
    /// Ableton Link: tempo and phase within a quantum; the beats' numbering is the
    /// session's, only their phase is matched.
    Link,
    /// MIDI clock: tempo and an absolute song position (from the song position pointer).
    Clock,
    /// MIDI time code: an absolute song time in seconds; the tempo stays the project's.
    Mtc,
}

impl SourceKind {
    fn to_u64(self) -> u64 {
        match self {
            SourceKind::None => 0,
            SourceKind::Link => 1,
            SourceKind::Clock => 2,
            SourceKind::Mtc => 3,
        }
    }
    fn from_u64(v: u64) -> SourceKind {
        match v {
            1 => SourceKind::Link,
            2 => SourceKind::Clock,
            3 => SourceKind::Mtc,
            _ => SourceKind::None,
        }
    }
}

/// What a source publishes: where its music is at a host time.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    pub kind: SourceKind,
    /// BPM (Link, clock). Zero for time code.
    pub tempo: f64,
    /// The host time (µs, [`host_micros`]) the position below is at.
    pub host: i64,
    /// Link and clock: the beat at `host`; time code: the song's seconds at `host`.
    pub position: f64,
    /// Time code: song seconds per second of host time (≈ 1, the master's speed against
    /// ours). Unused by the others (their speed is the tempo).
    pub rate: f64,
    /// Whether the source's music plays.
    pub playing: bool,
    /// Link's quantum (beats whose phase is matched); 0 for an absolute position.
    pub quantum: f64,
    /// Link: the session's start and stop are followed (*start/stop sync*); off, Cantus
    /// starts and stops on its own, in phase.
    pub start_stop: bool,
    /// Counts discontinuities — a locate, a start, a session joined: the follower jumps
    /// to the new place instead of drifting there.
    pub epoch: u32,
}

impl Timeline {
    /// The beat (Link, clock) at host time `t`.
    pub fn beat_at(&self, t: i64) -> f64 {
        self.position + (t - self.host) as f64 * self.tempo / 60e6
    }
    /// The song seconds (time code) at host time `t`.
    pub fn seconds_at(&self, t: i64) -> f64 {
        self.position + (t - self.host) as f64 * self.rate / 1e6
    }
    /// The host time Link's or the clock's beat `b` falls at.
    pub fn time_at(&self, b: f64) -> i64 {
        if self.tempo <= 0.0 {
            return self.host;
        }
        self.host + ((b - self.position) * 60e6 / self.tempo).round() as i64
    }
}

/// A seqlock of `N` words: one writer, any readers, no lock. A reader that keeps meeting a
/// write gives up after a few tries (the audio thread never spins) and keeps what it had.
#[derive(Debug)]
struct Seq<const N: usize> {
    seq: AtomicU32,
    words: [AtomicU64; N],
}

impl<const N: usize> Default for Seq<N> {
    fn default() -> Self {
        Seq {
            seq: AtomicU32::new(0),
            words: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl<const N: usize> Seq<N> {
    fn write(&self, w: [u64; N]) {
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);
        for (a, v) in self.words.iter().zip(w) {
            a.store(v, Ordering::Relaxed);
        }
        self.seq.store(s.wrapping_add(2), Ordering::Release);
    }

    fn read(&self) -> Option<[u64; N]> {
        for _ in 0..8 {
            let s0 = self.seq.load(Ordering::Acquire);
            if s0 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let w: [u64; N] = std::array::from_fn(|i| self.words[i].load(Ordering::Relaxed));
            std::sync::atomic::fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == s0 {
                return Some(w);
            }
        }
        None
    }
}

/// The timeline a source publishes and the audio thread reads.
#[derive(Debug, Default)]
pub struct TimelineCell(Seq<8>);

impl TimelineCell {
    pub fn publish(&self, t: &Timeline) {
        let flags = u64::from(t.playing) | u64::from(t.start_stop) << 1;
        self.0.write([
            t.kind.to_u64(),
            t.tempo.to_bits(),
            t.host as u64,
            t.position.to_bits(),
            t.rate.to_bits(),
            flags,
            t.quantum.to_bits(),
            u64::from(t.epoch),
        ]);
    }

    /// Hold the cell as a writer pre-empted between its stores would (the sequence odd), or
    /// let it go: the tests' way to make every read fail for a while.
    #[doc(hidden)]
    pub fn hold_for_tests(&self, held: bool) {
        let s = self.0.seq.load(Ordering::Relaxed);
        if (s & 1 == 1) != held {
            self.0.seq.store(s.wrapping_add(1), Ordering::Release);
        }
    }

    /// The newest timeline, or `None` if a write kept getting in the way.
    pub fn read(&self) -> Option<Timeline> {
        let w = self.0.read()?;
        Some(Timeline {
            kind: SourceKind::from_u64(w[0]),
            tempo: f64::from_bits(w[1]),
            host: w[2] as i64,
            position: f64::from_bits(w[3]),
            rate: f64::from_bits(w[4]),
            playing: w[5] & 1 == 1,
            start_stop: w[5] & 2 == 2,
            quantum: f64::from_bits(w[6]),
            epoch: w[7] as u32,
        })
    }
}

/// The audio thread's sample clock against host time: `host = a + b · (sample − s0)`, and
/// the output's latency, for the MIDI-out thread.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct HostLine {
    /// A sample of the processor's clock and its host time (µs).
    pub s0: u64,
    pub a: f64,
    /// µs per sample (≈ 1e6 / rate; the device's real rate against the host clock).
    pub b: f64,
    /// The output's latency (device), µs: a sample is heard this long after its host time.
    pub latency_us: f64,
}

impl HostLine {
    /// The host time sample `s` is rendered at.
    pub fn host_of(&self, s: u64) -> f64 {
        self.a + self.b * (s as f64 - self.s0 as f64)
    }
}

#[derive(Debug, Default)]
pub struct HostLineCell(Seq<4>);

impl HostLineCell {
    pub fn publish(&self, l: &HostLine) {
        self.0
            .write([l.s0, l.a.to_bits(), l.b.to_bits(), l.latency_us.to_bits()]);
    }
    pub fn read(&self) -> Option<HostLine> {
        let w = self.0.read()?;
        Some(HostLine {
            s0: w[0],
            a: f64::from_bits(w[1]),
            b: f64::from_bits(w[2]),
            latency_us: f64::from_bits(w[3]),
        })
    }
}

/// How many (sample, host time) points the filter fits its line through: twenty seconds of
/// 512-frame blocks at 48 kHz (its slope is then within a few ppm through a millisecond of
/// callback jitter).
pub const FILTER_POINTS: usize = 2048;

/// The line from the sample clock to host time: an ordinary least-squares fit (slope
/// `Σ(x−x̄)(y−ȳ) / Σ(x−x̄)²`, intercept through the means) of the last [`FILTER_POINTS`]
/// block starts against the host times they ran at. A callback woken late moves the line by
/// a two-thousandth of its lateness, and the slope is the device's real rate against the
/// host clock — the drift a follower must not mistake for the source's.
#[derive(Clone, Debug)]
pub struct HostTimeFilter {
    xs: [f64; FILTER_POINTS],
    ys: [f64; FILTER_POINTS],
    n: usize,
    next: usize,
    /// The first point's sample and host time: the fit is relative to it (precision).
    x0: u64,
    y0: i64,
    nominal_us_per_sample: f64,
    /// The line last fitted and the points added since: the fit (a pass over the window)
    /// is made every [`REFIT`] points once the window is full, so a block costs a fraction
    /// of a microsecond (review r33 #7: two passes a block, ~9 µs).
    fit: HostLine,
    since: usize,
}

/// Points added between two fits of a full window.
pub const REFIT: usize = 32;

impl HostTimeFilter {
    pub fn new(sample_rate: f64) -> HostTimeFilter {
        let nominal = 1e6 / sample_rate.max(1.0);
        HostTimeFilter {
            xs: [0.0; FILTER_POINTS],
            ys: [0.0; FILTER_POINTS],
            n: 0,
            next: 0,
            x0: 0,
            y0: 0,
            nominal_us_per_sample: nominal,
            fit: HostLine {
                b: nominal,
                ..HostLine::default()
            },
            since: 0,
        }
    }

    /// Forget every point (the stream restarted, or a point is far off the line).
    pub fn reset(&mut self) {
        self.n = 0;
        self.next = 0;
        self.since = 0;
    }

    /// Block start `sample` was rendered at host time `host`.
    pub fn add(&mut self, sample: u64, host: i64) {
        if self.n > 8 {
            // A point a quarter second off the line is a restart, not jitter.
            if (self.fit.host_of(sample) - host as f64).abs() > 250_000.0 {
                self.reset();
            }
        }
        if self.n == 0 {
            self.x0 = sample;
            self.y0 = host;
        }
        self.xs[self.next] = sample as f64 - self.x0 as f64;
        self.ys[self.next] = (host - self.y0) as f64;
        self.next = (self.next + 1) % FILTER_POINTS;
        self.n = (self.n + 1).min(FILTER_POINTS);
        self.since += 1;
        // While the window fills (every point moves the line much), at each point; then
        // every `REFIT`.
        if self.n < FILTER_POINTS / 8 || self.since >= REFIT {
            self.fit = self.fitted();
            self.since = 0;
        }
    }

    /// The line, as last fitted.
    pub fn line(&self) -> HostLine {
        self.fit
    }

    /// The line fitted now (the nominal rate through the points' mean while they span less
    /// than a second).
    fn fitted(&self) -> HostLine {
        let n = self.n;
        if n == 0 {
            return HostLine {
                s0: 0,
                a: 0.0,
                b: self.nominal_us_per_sample,
                latency_us: 0.0,
            };
        }
        let (mut sx, mut sy) = (0.0, 0.0);
        for i in 0..n {
            sx += self.xs[i];
            sy += self.ys[i];
        }
        let (mx, my) = (sx / n as f64, sy / n as f64);
        let (mut sxx, mut sxy) = (0.0, 0.0);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..n {
            let dx = self.xs[i] - mx;
            sxx += dx * dx;
            sxy += dx * (self.ys[i] - my);
            lo = lo.min(self.xs[i]);
            hi = hi.max(self.xs[i]);
        }
        let mut b = self.nominal_us_per_sample;
        if sxx > 0.0 && (hi - lo) * b > 1e6 {
            let fit = sxy / sxx;
            // Never 1 % off the nominal rate: that is no device, that is a broken clock.
            if (fit / b - 1.0).abs() < 0.01 {
                b = fit;
            }
        }
        HostLine {
            s0: self.x0,
            a: self.y0 as f64 + my - b * mx,
            b,
            latency_us: 0.0,
        }
    }
}

/// What the audio thread asks of the source (Link): the user's own start, stop or tempo
/// while following, in host time.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Proposal {
    /// Start the session's music at host time `at`.
    Play { at: i64 },
    /// Stop it at `at`.
    Stop { at: i64 },
    /// The tempo the project asks for, from `at` on.
    Tempo { bpm: f64, at: i64 },
}

/// What a stamped message is, so the out thread sends it to the ports that take it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum OutKind {
    #[default]
    Clock,
    Mtc,
}

/// A MIDI message the audio thread made, stamped with the processor sample it is heard at
/// (the compensation's latency already in it).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stamped {
    pub at: u64,
    pub kind: OutKind,
    pub len: u8,
    pub bytes: [u8; 10],
}

impl Stamped {
    pub fn new(at: u64, kind: OutKind, msg: &[u8]) -> Stamped {
        let mut bytes = [0u8; 10];
        let len = msg.len().min(10);
        bytes[..len].copy_from_slice(&msg[..len]);
        Stamped {
            at,
            kind,
            len: len as u8,
            bytes,
        }
    }
    pub fn msg(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

/// The follower's state, for the top bar's lock light.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Lock {
    /// No source, or the source plays nothing.
    #[default]
    Idle,
    /// A start is waiting for its moment (the next quantum, the master's first clock).
    Queued,
    /// Following, within a millisecond.
    Locked,
    /// Following, still pulling in.
    Seeking,
}

/// What the interface sets for the audio thread: which messages it makes, the time code's
/// rate and the project's start offset.
#[derive(Debug)]
pub struct OutConfig {
    pub clock: AtomicBool,
    pub mtc: AtomicBool,
    /// [`mtc::Rate`] as a number (0 = 24, 1 = 25, 2 = 29.97 drop, 3 = 30).
    pub mtc_rate: AtomicU32,
    /// The time code at the song's start, in seconds (an hour is common), for time code
    /// out and in.
    pub mtc_offset_s: AtomicU64,
}

impl Default for OutConfig {
    fn default() -> Self {
        OutConfig {
            clock: AtomicBool::new(false),
            mtc: AtomicBool::new(false),
            mtc_rate: AtomicU32::new(1),
            mtc_offset_s: AtomicU64::new(0f64.to_bits()),
        }
    }
}

impl OutConfig {
    pub fn offset_s(&self) -> f64 {
        f64::from_bits(self.mtc_offset_s.load(Ordering::Relaxed))
    }
    pub fn set_offset_s(&self, s: f64) {
        self.mtc_offset_s.store(s.to_bits(), Ordering::Relaxed);
    }
}

/// Everything both sides share.
#[derive(Debug, Default)]
pub struct SyncShared {
    pub timeline: TimelineCell,
    pub host_line: HostLineCell,
    pub out: OutConfig,
    /// The follower's state (`Lock` as a number), its last error (µs, signed) and the tempo
    /// it plays now (BPM bits): what the top bar shows.
    pub lock: AtomicU32,
    pub error_us: AtomicI64,
    pub tempo_bits: AtomicU64,
    /// Messages the out ring had no room for (counted, never waited for).
    pub dropped: AtomicU64,
}

impl SyncShared {
    pub fn lock_state(&self) -> Lock {
        match self.lock.load(Ordering::Relaxed) {
            1 => Lock::Queued,
            2 => Lock::Locked,
            3 => Lock::Seeking,
            _ => Lock::Idle,
        }
    }
    pub fn set_lock(&self, l: Lock) {
        let v = match l {
            Lock::Idle => 0,
            Lock::Queued => 1,
            Lock::Locked => 2,
            Lock::Seeking => 3,
        };
        self.lock.store(v, Ordering::Relaxed);
    }
    pub fn tempo(&self) -> f64 {
        f64::from_bits(self.tempo_bits.load(Ordering::Relaxed))
    }
}

/// The ring sizes: proposals are rare; the out ring holds a second of clock at 300 BPM
/// with time code beside it.
pub const PROPOSALS: usize = 64;
pub const OUT_RING: usize = 1024;

/// The audio thread's end of the seam.
#[derive(Debug)]
pub struct SyncPort {
    pub shared: Arc<SyncShared>,
    pub proposals: Producer<Proposal>,
    pub out: Producer<Stamped>,
}

/// The other end: the source's and the out thread's.
#[derive(Debug)]
pub struct SyncHandle {
    pub shared: Arc<SyncShared>,
    pub proposals: Consumer<Proposal>,
    /// Taken by the MIDI-out thread when it starts.
    pub out: Option<Consumer<Stamped>>,
}

/// A seam's two ends.
pub fn pair() -> (SyncPort, SyncHandle) {
    let shared = Arc::new(SyncShared::default());
    let (ptx, prx) = RingBuffer::new(PROPOSALS);
    let (otx, orx) = RingBuffer::new(OUT_RING);
    (
        SyncPort {
            shared: shared.clone(),
            proposals: ptx,
            out: otx,
        },
        SyncHandle {
            shared,
            proposals: prx,
            out: Some(orx),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timeline_comes_back_as_it_went() {
        let c = TimelineCell::default();
        let t = Timeline {
            kind: SourceKind::Link,
            tempo: 123.25,
            host: -77,
            position: 1234.5,
            rate: 1.0001,
            playing: true,
            quantum: 4.0,
            start_stop: true,
            epoch: 9,
        };
        c.publish(&t);
        assert_eq!(c.read(), Some(t));
        assert_eq!(t.time_at(t.beat_at(1_000_000)), 1_000_000);
    }

    /// Callbacks woken up to 2 ms late on a device 30 ppm fast: the line's slope is the
    /// real rate within the lateness's own statistics over five seconds (tens of ppm; the
    /// follower smooths it over ten more), and the line stays within 150 µs of the truth
    /// plus the lateness's mean.
    #[test]
    fn the_filter_finds_the_real_rate_through_jitter() {
        let sr = 48_000.0;
        let mut f = HostTimeFilter::new(sr);
        let us = 1e6 / (sr * (1.0 + 30e-6));
        let mut x = 7u64;
        let mut worst = 0.0f64;
        let mut rng = 0x2545_F491_4F6C_DD1Du64;
        for i in 0..2_000u64 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let late = (rng % 10_000) as f64 / 10_000.0 * 2_000.0;
            let truth = 1_000_000.0 + x as f64 * us;
            f.add(x, (truth + late) as i64);
            if i > 600 {
                let l = f.line();
                worst = worst.max((l.host_of(x) - truth - 1_000.0).abs());
                assert!((l.b / us - 1.0).abs() < 30e-6, "slope {} vs {us}", l.b);
            }
            x += 512;
        }
        println!("filter: worst error {worst:.1} µs around the mean lateness");
        assert!(worst < 150.0, "{worst}");
    }

    /// Review r33 #7: the filter's cost per block (a point added, the line read), full.
    /// Printed for the record (`docs/round33/sync.md`); bounded loosely so a debug build
    /// passes.
    #[test]
    fn the_filter_costs_little_per_block() {
        let mut f = HostTimeFilter::new(48_000.0);
        let mut x = 0u64;
        for _ in 0..FILTER_POINTS {
            f.add(x, (x as f64 / 0.048) as i64);
            x += 64;
        }
        let n = 20_000u32;
        let t = std::time::Instant::now();
        let mut sum = 0.0;
        for _ in 0..n {
            f.add(x, (x as f64 / 0.048) as i64);
            sum += f.line().b;
            x += 64;
        }
        let per = t.elapsed().as_secs_f64() * 1e6 / f64::from(n);
        println!("host-time filter: {per:.3} µs a block (full window), {sum:.0}");
        // ~9 µs before the fit was spread (review r33 #7); a fraction of one after.
        assert!(per < 4.0, "{per} µs");
    }
}
