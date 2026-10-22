//! Where the audio callback's time goes (round 20, `docs/perf/benchmark.md`).
//!
//! Two instruments, both on the audio thread and both within ADR 0005 — no allocation, no
//! lock, no syscall beyond the clock the callback already read:
//!
//! - **A histogram of every block's wall time** ([`BlockHistogram`]): 64 buckets, four
//!   per octave from 1 µs to about 55 ms, counted since the processor was built. It
//!   travels to the UI inside [`crate::EngineStats`] like every other reading, by value
//!   over the existing ring, so "lock-free" costs nothing new. Percentiles come out to
//!   the bucket's upper edge — about 19 % resolution, plenty to tell 0.4 ms from 2 ms.
//! - **A dropout detector with blame** ([`Profiler`]): every nanosecond of a block is
//!   charged to exactly one *node* — the engine's own work, a track's instrument, a
//!   strip's mixing, one insert, one master insert — by stamping the clock at each
//!   boundary. When a block overruns its budget, the node that took longest is named
//!   ([`Dropout`]), and the status line says it in words: "Spatium on Vocal took 3.1 ms".
//!   The totals over a run are what the benchmark reports as hot spots.
//!
//! The profiler only reads the clock and adds integers: it cannot change a sample, so the
//! null tests hold with it on or off (`tests/profile.rs` proves it).

use crate::messages::{MAX_EFFECTS_PER_TRACK, MAX_STRIPS, MAX_TRACKS};
use cantus_core::{EffectId, StripId, TrackId};
use std::time::Instant;

/// Buckets in a [`BlockHistogram`].
pub const HISTOGRAM_BUCKETS: usize = 64;
/// Buckets per doubling of the block time.
const PER_OCTAVE: f64 = 4.0;

/// Block wall times, in log-spaced buckets: bucket 0 is under 1 µs, bucket `b ≥ 1` holds
/// `[2^((b−1)/4), 2^(b/4))` µs, and the last bucket everything slower.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlockHistogram {
    counts: [u32; HISTOGRAM_BUCKETS],
}

impl Default for BlockHistogram {
    fn default() -> Self {
        BlockHistogram {
            counts: [0; HISTOGRAM_BUCKETS],
        }
    }
}

impl BlockHistogram {
    /// The bucket a block of `us` microseconds falls in.
    pub fn bucket(us: f64) -> usize {
        if us.is_nan() || us < 1.0 {
            return 0;
        }
        ((cantus_dsp::math::log2(us) * PER_OCTAVE) as usize + 1).min(HISTOGRAM_BUCKETS - 1)
    }

    /// The smallest time bucket `b` holds, µs.
    pub fn lower_us(b: usize) -> f64 {
        if b == 0 {
            0.0
        } else {
            cantus_dsp::math::powf(2f64, (b - 1) as f64 / PER_OCTAVE)
        }
    }

    /// The time bucket `b` holds up to, µs (the last bucket is open: its edge is reported).
    pub fn upper_us(b: usize) -> f64 {
        cantus_dsp::math::powf(2f64, b as f64 / PER_OCTAVE)
    }

    /// Count one block. Saturates rather than wraps: after 4 × 10⁹ blocks in one bucket
    /// (a month at 64 samples) the shape is what matters.
    pub fn record(&mut self, us: f64) {
        let b = Self::bucket(us);
        self.counts[b] = self.counts[b].saturating_add(1);
    }

    pub fn counts(&self) -> &[u32; HISTOGRAM_BUCKETS] {
        &self.counts
    }

    /// Blocks counted.
    pub fn count(&self) -> u64 {
        self.counts.iter().map(|c| *c as u64).sum()
    }

    /// The time under which a fraction `q` (0…1) of the blocks ran, to the upper edge of
    /// its bucket; 0 when nothing has been counted.
    pub fn quantile(&self, q: f64) -> f64 {
        let n = self.count();
        if n == 0 {
            return 0.0;
        }
        let want = ((q.clamp(0.0, 1.0) * n as f64).ceil() as u64).max(1);
        let mut seen = 0u64;
        for (b, c) in self.counts.iter().enumerate() {
            seen += *c as u64;
            if seen >= want {
                return Self::upper_us(b);
            }
        }
        Self::upper_us(HISTOGRAM_BUCKETS - 1)
    }

    /// Blocks in buckets that start at or above `us`: the blocks surely slower than it.
    pub fn at_least(&self, us: f64) -> u64 {
        let b = Self::bucket(us);
        let from = if Self::lower_us(b) >= us { b } else { b + 1 };
        self.counts[from.min(HISTOGRAM_BUCKETS)..]
            .iter()
            .map(|c| *c as u64)
            .sum()
    }

    /// What was counted since `earlier` (a copy taken before): the histogram of a window.
    pub fn since(&self, earlier: &BlockHistogram) -> BlockHistogram {
        let mut out = BlockHistogram::default();
        for (o, (a, b)) in out
            .counts
            .iter_mut()
            .zip(self.counts.iter().zip(earlier.counts.iter()))
        {
            *o = a.saturating_sub(*b);
        }
        out
    }
}

/// What ran longest in a block that overran, in the project's terms.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Culprit {
    /// The engine's own work: messages, events, meters, the master's fader, the click.
    Engine,
    /// A track's instrument (and its audio clips and monitored input).
    Instrument(TrackId),
    /// A strip's own mixing: its EQ, width, fader, sends — not its inserts.
    Strip(StripId),
    /// One insert on a strip, by its place in the chain.
    Insert {
        strip: StripId,
        effect: EffectId,
        index: u8,
    },
    /// One insert on the master chain.
    MasterInsert { effect: EffectId, index: u8 },
}

/// A block that ran past its budget, and who to blame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Dropout {
    pub culprit: Culprit,
    /// The culprit's share of the block, µs.
    pub culprit_us: f32,
    /// The whole block, µs.
    pub block_us: f32,
    /// What the block had: its frames at the sample rate, µs.
    pub budget_us: f32,
    /// Which block (`EngineStats::blocks_processed` when it happened).
    pub block: u64,
}

/// A place the profiler charges time to. Slots, not ids: the processor resolves them to a
/// [`Culprit`] only when a block overran.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Engine,
    /// A track slot's instrument.
    Track(usize),
    /// A strip slot's mixing.
    Strip(usize),
    /// A strip slot's insert, by index.
    Insert(usize, usize),
    /// The master chain's insert, by index.
    MasterInsert(usize),
}

const TRACK0: usize = 1;
const STRIP0: usize = TRACK0 + MAX_TRACKS;
const INSERT0: usize = STRIP0 + MAX_STRIPS;
const MASTER0: usize = INSERT0 + MAX_STRIPS * MAX_EFFECTS_PER_TRACK;
/// Every node there can be.
pub const NODES: usize = MASTER0 + MAX_EFFECTS_PER_TRACK;

impl Node {
    fn index(self) -> usize {
        match self {
            Node::Engine => 0,
            Node::Track(s) => TRACK0 + s.min(MAX_TRACKS - 1),
            Node::Strip(s) => STRIP0 + s.min(MAX_STRIPS - 1),
            Node::Insert(s, i) => {
                INSERT0
                    + s.min(MAX_STRIPS - 1) * MAX_EFFECTS_PER_TRACK
                    + i.min(MAX_EFFECTS_PER_TRACK - 1)
            }
            Node::MasterInsert(i) => MASTER0 + i.min(MAX_EFFECTS_PER_TRACK - 1),
        }
    }

    fn from_index(i: usize) -> Node {
        if i < TRACK0 {
            Node::Engine
        } else if i < STRIP0 {
            Node::Track(i - TRACK0)
        } else if i < INSERT0 {
            Node::Strip(i - STRIP0)
        } else if i < MASTER0 {
            let k = i - INSERT0;
            Node::Insert(k / MAX_EFFECTS_PER_TRACK, k % MAX_EFFECTS_PER_TRACK)
        } else {
            Node::MasterInsert(i - MASTER0)
        }
    }
}

/// Charges a block's time to its nodes. Built off the audio thread (it holds two arrays of
/// [`NODES`] counters); after that it only stamps the clock and adds.
#[derive(Debug)]
pub struct Profiler {
    on: bool,
    /// The node being charged, and since when.
    current: usize,
    mark: Option<Instant>,
    /// This block's nanoseconds per node, and which nodes it touched (so the end of a
    /// block visits those, not all of them).
    block: Vec<u64>,
    touched: Vec<u16>,
    /// Nanoseconds per node since the last [`Profiler::reset_totals`].
    totals: Vec<u64>,
    /// Blocks and nanoseconds in the totals.
    total_blocks: u64,
    total_ns: u64,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    /// A profiler, on. Allocates: build it with the processor.
    pub fn new() -> Self {
        Profiler {
            on: true,
            current: 0,
            mark: None,
            block: vec![0; NODES],
            touched: Vec::with_capacity(NODES),
            totals: vec![0; NODES],
            total_blocks: 0,
            total_ns: 0,
        }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Switch the per-node clock on or off (the block histogram is kept either way). Off,
    /// [`Profiler::enter`] is a branch and nothing else; overruns are still counted but
    /// nobody is blamed.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        self.mark = None;
    }

    /// A block begins at `start` (the instant the callback already read), in the engine.
    #[inline]
    pub fn begin(&mut self, start: Instant) {
        if self.on {
            self.current = 0;
            self.mark = Some(start);
        }
    }

    /// From now on, time is `node`'s.
    #[inline]
    pub fn enter(&mut self, node: Node) {
        if !self.on {
            return;
        }
        let now = Instant::now();
        self.charge(now);
        self.current = node.index();
    }

    fn charge(&mut self, now: Instant) {
        if let Some(m) = self.mark {
            let ns = now.saturating_duration_since(m).as_nanos() as u64;
            self.add(self.current, ns);
        }
        self.mark = Some(now);
    }

    /// The block ends at `end`. Its nodes are added to the totals and cleared; when
    /// `blame` is set, the node that took longest is returned with its nanoseconds.
    pub fn end(&mut self, end: Instant, blame: bool) -> Option<(Node, u64)> {
        self.end_among(end, blame, |_| true)
    }

    /// [`Profiler::end`], blaming only among the nodes `eligible` accepts — with a pool,
    /// the late block's critical path (ADR 0031 §2.9): a node that ran beside it on another
    /// core made nobody late.
    pub fn end_among(
        &mut self,
        end: Instant,
        blame: bool,
        eligible: impl Fn(Node) -> bool,
    ) -> Option<(Node, u64)> {
        if !self.on {
            return None;
        }
        self.charge(end);
        self.mark = None;
        let mut worst: Option<(usize, u64)> = None;
        let mut block_ns = 0u64;
        for &i in &self.touched {
            let i = i as usize;
            let ns = self.block[i];
            block_ns += ns;
            if blame && worst.is_none_or(|(_, w)| ns > w) && eligible(Node::from_index(i)) {
                worst = Some((i, ns));
            }
            self.totals[i] += ns;
            self.block[i] = 0;
        }
        self.touched.clear();
        self.total_blocks += 1;
        self.total_ns += block_ns;
        worst.map(|(i, ns)| (Node::from_index(i), ns))
    }

    /// From now on time is `node`'s, but `elsewhere_ns` of the time since the last mark
    /// was already charged by a slot's own profiler (the callback running nodes as slot 0
    /// of the pool): only the rest goes to the node being left.
    pub fn enter_except(&mut self, node: Node, elsewhere_ns: u64) {
        if !self.on {
            return;
        }
        let now = Instant::now();
        if let Some(m) = self.mark {
            let ns = now
                .saturating_duration_since(m)
                .as_nanos()
                .saturating_sub(elsewhere_ns as u128) as u64;
            self.add(self.current, ns);
        }
        self.mark = Some(now);
        self.current = node.index();
    }

    /// From now on time is `node`'s, and the time since the last mark is charged to no
    /// one (the callback waiting on a parallel pass's last task).
    pub fn restart(&mut self, node: Node) {
        if self.on {
            self.mark = Some(Instant::now());
            self.current = node.index();
        }
    }

    /// Take what a slot's profiler charged this block (ADR 0031 §2.9: each node on the
    /// worker that ran it), leaving it empty.
    pub fn fold(&mut self, slot: &mut SlotProfile) {
        for &i in &slot.touched {
            let i = i as usize;
            let ns = std::mem::take(&mut slot.block[i]);
            self.add(i, ns);
        }
        slot.touched.clear();
    }

    /// What this block has charged to `node` so far (before [`Profiler::end`]).
    pub fn block_ns(&self, node: Node) -> u64 {
        self.block[node.index()]
    }

    fn add(&mut self, i: usize, ns: u64) {
        let slot = &mut self.block[i];
        if *slot == 0 && ns > 0 && self.touched.len() < self.touched.capacity() {
            self.touched.push(i as u16);
        }
        *slot += ns;
    }

    /// Every node that has been charged since the last reset, with its nanoseconds.
    pub fn totals(&self) -> impl Iterator<Item = (Node, u64)> + '_ {
        self.totals
            .iter()
            .enumerate()
            .filter(|(_, ns)| **ns > 0)
            .map(|(i, ns)| (Node::from_index(i), *ns))
    }

    /// Blocks and nanoseconds the totals cover.
    pub fn total_blocks(&self) -> u64 {
        self.total_blocks
    }

    pub fn total_ns(&self) -> u64 {
        self.total_ns
    }

    /// Start the totals afresh (a benchmark after its warm-up). Not for the audio thread:
    /// it walks every node.
    pub fn reset_totals(&mut self) {
        self.totals.iter_mut().for_each(|x| *x = 0);
        self.total_blocks = 0;
        self.total_ns = 0;
    }
}

/// One pool slot's profiler (ADR 0031 §2.9): what the nodes it ran took, charged as on
/// the callback's [`Profiler`] and folded into it after the block. Each thread of the pool
/// owns one (in its scratch), so charging needs no atomics.
#[derive(Debug)]
pub struct SlotProfile {
    on: bool,
    current: usize,
    mark: Option<Instant>,
    block: Vec<u64>,
    touched: Vec<u16>,
    /// Nanoseconds charged since [`SlotProfile::take_charged`].
    charged: u64,
}

impl Default for SlotProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl SlotProfile {
    /// Allocates: built with the pool's scratch.
    pub fn new() -> Self {
        SlotProfile {
            on: true,
            current: 0,
            mark: None,
            block: vec![0; NODES],
            touched: Vec::with_capacity(NODES),
            charged: 0,
        }
    }

    #[inline]
    pub fn set_on(&mut self, on: bool) {
        if !on {
            self.mark = None;
        }
        self.on = on;
    }

    /// From now on, time is `node`'s.
    #[inline]
    pub fn enter(&mut self, node: Node) {
        if !self.on {
            return;
        }
        let now = Instant::now();
        self.charge(now);
        self.current = node.index();
    }

    /// The node being charged ends at `now`; nothing is charged until the next `enter`.
    #[inline]
    pub fn stop(&mut self, now: Instant) {
        if self.on {
            self.charge(now);
            self.mark = None;
        }
    }

    fn charge(&mut self, now: Instant) {
        if let Some(m) = self.mark {
            let ns = now.saturating_duration_since(m).as_nanos() as u64;
            let slot = &mut self.block[self.current];
            if *slot == 0 && ns > 0 && self.touched.len() < self.touched.capacity() {
                self.touched.push(self.current as u16);
            }
            *slot += ns;
            self.charged += ns;
        }
        self.mark = Some(now);
    }

    /// Nanoseconds charged since the last call.
    pub fn take_charged(&mut self) -> u64 {
        std::mem::take(&mut self.charged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_are_four_per_octave_and_quantiles_land_on_their_edges() {
        assert_eq!(BlockHistogram::bucket(0.2), 0);
        assert_eq!(BlockHistogram::bucket(1.0), 1);
        assert_eq!(BlockHistogram::bucket(2.0), 5);
        assert_eq!(BlockHistogram::bucket(1e9), HISTOGRAM_BUCKETS - 1);
        for b in 1..HISTOGRAM_BUCKETS - 1 {
            let mid = (BlockHistogram::lower_us(b) + BlockHistogram::upper_us(b)) * 0.5;
            assert_eq!(BlockHistogram::bucket(mid), b, "bucket {b}");
        }
        let mut h = BlockHistogram::default();
        assert_eq!(h.quantile(0.5), 0.0);
        for _ in 0..99 {
            h.record(100.0);
        }
        h.record(3_000.0);
        assert_eq!(h.count(), 100);
        let p50 = h.quantile(0.5);
        assert!(
            (100.0..=100.0 * cantus_dsp::math::powf(2f64, 0.25)).contains(&p50),
            "{p50}"
        );
        let max = h.quantile(1.0);
        assert!(
            (3_000.0..=3_000.0 * cantus_dsp::math::powf(2f64, 0.25)).contains(&max),
            "{max}"
        );
        assert_eq!(h.at_least(2_000.0), 1);
        let before = h;
        h.record(5.0);
        assert_eq!(h.since(&before).count(), 1);
    }

    #[test]
    fn node_indices_round_trip() {
        for n in [
            Node::Engine,
            Node::Track(0),
            Node::Track(MAX_TRACKS - 1),
            Node::Strip(3),
            Node::Insert(0, 0),
            Node::Insert(MAX_STRIPS - 1, MAX_EFFECTS_PER_TRACK - 1),
            Node::MasterInsert(2),
        ] {
            assert_eq!(Node::from_index(n.index()), n);
        }
        assert_eq!(
            Node::MasterInsert(MAX_EFFECTS_PER_TRACK - 1).index(),
            NODES - 1
        );
    }

    #[test]
    fn every_nanosecond_goes_to_one_node_and_the_longest_is_blamed() {
        // Spun, not slept: Windows rounds a sleep to its 15.6 ms timer, which made 3 ms and
        // 12 ms alike on GitHub's runner (Track(2) blamed). 1 ms against 30 ms leaves room
        // for a preemption inside the first.
        let spin = |ms: u64| {
            let t = Instant::now();
            while t.elapsed() < std::time::Duration::from_millis(ms) {
                std::hint::spin_loop();
            }
        };
        let mut p = Profiler::new();
        let t0 = Instant::now();
        p.begin(t0);
        p.enter(Node::Track(2));
        spin(1);
        p.enter(Node::Insert(1, 4));
        spin(30);
        p.enter(Node::Engine);
        let end = Instant::now();
        let (node, ns) = p.end(end, true).expect("blamed");
        assert_eq!(node, Node::Insert(1, 4));
        assert!(ns >= 30_000_000, "{ns}");
        let sum: u64 = p.totals().map(|(_, ns)| ns).sum();
        assert_eq!(
            sum,
            (end - t0).as_nanos() as u64,
            "time charged twice or lost"
        );
        assert_eq!(p.total_ns(), sum);
        p.set_on(false);
        p.begin(Instant::now());
        p.enter(Node::Track(1));
        assert_eq!(p.end(Instant::now(), true), None);
    }
}
