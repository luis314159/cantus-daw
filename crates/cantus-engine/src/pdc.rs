//! Plug-in delay compensation: every path to a summing point arrives with the same delay.
//!
//! A limiter's lookahead or a linear-phase EQ delays its strip. Where that strip meets
//! others — at a bus, at the master — the undelayed signals must wait for it, or the mix
//! smears and, with anything in parallel, combs. Compensation is per **edge**, not per
//! strip: a source sent both to the master and to a latent return needs its master edge
//! delayed and its send edge not, which one delay on the strip cannot express.
//!
//! The arithmetic is done once per snapshot, on the audio thread, over arrays it already
//! has (no allocation): walking the strips in processing order, each strip's input arrival
//! `A` is the latest of what reaches it, its output leaves at `P = A + own latency`, and
//! an edge from `src` into `dst` is delayed by `A(dst) − P(src)`. Tracks leave at zero.
//!
//! The delay lines come from a pool of [`POOL`] places, **[`START_LINES`] of them filled at
//! start-up** (ADR 0107; it was all 192, 12 MB, whether a project had latency or not):
//! the rest is memory only once a project can need it. Before a snapshot that may need more
//! lines than the engine holds, the UI thread sends them (`EngineMessage::PdcLines` of
//! [`short_lines`], as many as [`lines_needed`] counts edges that can be delayed), so the
//! application's engine never waits for one; an edge keeps its line across snapshots while
//! its delay is unchanged, so an edit elsewhere does not click, and a new edge takes a line
//! only once every kept edge has kept its own (no edge's line is ever taken from it). A
//! line's ring runs at exactly its delay, so its length is only a ceiling and the memory it
//! touches is what the delay needs.
//!
//! **Longer delays grow the pool, off the audio thread.** A linear-phase EQ asks for tens
//! of thousands of samples, and every path in parallel with it needs a line that long.
//! When no free line is long enough, the edge is left uncompensated for the moment and
//! the audio thread asks for lines ([`Compensation::take_request`] →
//! `EngineEvent::PdcNeed`); the UI thread allocates them (`EngineMessage::PdcLines`), the
//! audio thread swaps them in for free short ones ([`Compensation::supply`]), plans again,
//! and hands the short ones back to be freed. The gap is a few milliseconds after the
//! edit that caused it; an offline render settles it before its first block.
//! A project with no latency uses no line at all and renders exactly as it did before
//! compensation existed.

use cantus_rt::cell::NodeCell;
use std::sync::Arc;

/// Each line's length at start-up: 8192 samples is 170 ms at 48 kHz.
pub const START_LEN: usize = 8192;
/// The longest compensation, in samples: 2¹⁹ is 10.9 s at 48 kHz, 2.7 s at 192 kHz.
pub const MAX_DELAY: usize = 1 << 19;
/// Delay lines available. An edge that finds none free is left uncompensated and counted.
pub const POOL: usize = 192;
/// Lines a processor starts with (ADR 0107): 2 MB, enough for a new project's sixteen strips
/// with latency on one and a few tracks
/// (and for whatever drives a processor without sending lines ahead — a test, the bench);
/// the application sends what a snapshot can need beyond them ([`lines_needed`]).
pub const START_LINES: usize = 32;

/// `count` new lines able to hold `delay` samples, rounded up so small changes in latency
/// do not ask again. Allocates: for the UI thread (or an offline render) only.
pub fn lines_for(count: usize, delay: usize) -> Vec<DelayLine> {
    // A short delay asked for (a pool that had no line at all) gets a start-up line; a
    // longer one, the power of two above it, as it always did.
    let len = delay.min(MAX_DELAY).next_power_of_two().max(START_LEN);
    (0..count.min(POOL))
        .map(|_| DelayLine::with_len(len))
        .collect()
}

/// `count` start-up lines ([`START_LEN`]), what a snapshot is sent ahead of it. Allocates.
pub fn short_lines(count: usize) -> Vec<DelayLine> {
    (0..count.min(POOL)).map(|_| DelayLine::new()).collect()
}

/// How many lines `snapshot` can need at most: one per edge that can be delayed — every
/// track's (and its own lateness), every strip's output, send and insert dry path, every
/// master insert's, the click's and the recorder's — and none at all when nothing in it
/// can have latency (no insert, no instrument unit, no frozen render, no lateness, no
/// outboard gear). Never more than [`POOL`]. What the backend holds the engine to before
/// it sends the snapshot (ADR 0107).
pub fn lines_needed(snapshot: &cantus_core::Snapshot) -> usize {
    use cantus_core::InstrumentSpec;
    let s = snapshot;
    let latent = s
        .strips
        .iter()
        .any(|st| !st.effects.is_empty() || st.frozen.is_some())
        || !s.master_effects.is_empty()
        || s.master_frozen.is_some()
        || s.tracks.iter().any(|t| {
            matches!(
                t.instrument,
                InstrumentSpec::Native(_) | InstrumentSpec::External(_)
            ) || t.frozen.is_some()
                || t.lateness.is_some()
                || !t.external.is_empty()
        });
    if !latent {
        return 0;
    }
    let tracks: usize = s
        .tracks
        .iter()
        .map(|t| 1 + usize::from(t.lateness.is_some()))
        .sum();
    let strips: usize = s
        .strips
        .iter()
        .map(|st| 1 + st.sends.len() + st.effects.len())
        .sum();
    (tracks + strips + s.master_effects.len() + 2).min(POOL)
}

/// Which edge a line serves.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Edge {
    /// A track slot into its strip (or the master).
    Track(usize),
    /// A track slot's source before its level: the delay it keeps (`Track::lateness`, a
    /// bounce's — ADR 0064, review 3).
    Source(usize),
    /// A strip slot into its output.
    Out(usize),
    /// A strip slot's `k`-th send.
    Send(usize, usize),
    /// A strip slot's `i`-th insert's dry path: every latent insert has one, for its slot
    /// mix and for its soft bypass (ADR 0022).
    Dry(usize, usize),
    /// The metronome, delayed by the output's latency so a beat's click comes out with
    /// the beat (ADR 0022).
    Click,
    /// The strip the mix recorder takes, delayed by its path to the master so its take
    /// lines up with what is heard (ADR 0022).
    Capture,
    /// The master chain's `i`-th insert's dry path (ADR 0026), as [`Edge::Dry`] is a
    /// strip's.
    MasterDry(usize),
}

/// One stereo delay line.
#[derive(Clone, Debug)]
pub struct DelayLine {
    pub edge: Option<Edge>,
    delay: usize,
    buf_l: Box<[f32]>,
    buf_r: Box<[f32]>,
    pos: usize,
    /// Consecutive silent input samples; the line is empty once this reaches `delay`.
    silent: usize,
    /// The output buffers of the edge its place serves (ADR 0107), for a place that had
    /// none: given to the place when the line is supplied (`Processor::edges_for_lines`),
    /// so a place holds edge buffers only once it holds a line. Empty once given.
    pub(crate) out: [Vec<f32>; 2],
}

impl DelayLine {
    pub fn new() -> Self {
        Self::with_len(START_LEN)
    }

    /// A line that can delay by up to `len` samples.
    pub fn with_len(len: usize) -> Self {
        DelayLine {
            edge: None,
            delay: 0,
            buf_l: vec![0.0; len].into_boxed_slice(),
            buf_r: vec![0.0; len].into_boxed_slice(),
            pos: 0,
            silent: usize::MAX,
            out: if len == 0 {
                [Vec::new(), Vec::new()]
            } else {
                let mut pair = [
                    vec![0.0; crate::graph::PASS_FRAMES],
                    vec![0.0; crate::graph::PASS_FRAMES],
                ];
                // Touched here, off the audio thread (ADR 0031 R11).
                for v in &mut pair {
                    v.fill(f32::MIN_POSITIVE);
                    std::hint::black_box(&mut *v);
                    v.fill(0.0);
                }
                pair
            },
        }
    }

    // r34:listen — a checkpoint's image leaves fresh lines out (ADR 0083).
    /// Whether this line is, to the bit, as [`Self::new`] makes it.
    pub(crate) fn is_fresh(&self) -> bool {
        self.edge.is_none()
            && self.delay == 0
            && self.pos == 0
            && self.silent == usize::MAX
            && self.buf_l.len() == START_LEN
            && self.buf_r.len() == START_LEN
            && self
                .buf_l
                .iter()
                .chain(self.buf_r.iter())
                .all(|x| x.to_bits() == 0)
    }

    /// The longest delay this line can hold.
    pub fn len(&self) -> usize {
        self.buf_l.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf_l.is_empty()
    }

    /// Take this line for `edge` at `delay` samples (1 ..= [`Self::len`]), emptied.
    pub fn assign(&mut self, edge: Edge, delay: usize) {
        self.edge = Some(edge);
        self.delay = delay.clamp(1, self.len());
        self.buf_l[..self.delay].iter_mut().for_each(|x| *x = 0.0);
        self.buf_r[..self.delay].iter_mut().for_each(|x| *x = 0.0);
        self.pos = 0;
        self.silent = usize::MAX;
    }

    /// A new delay for the edge this line already serves, **keeping what it holds**
    /// (ADR 0056): the read point moves. Shorter, the next sample out is the one `delay`
    /// back — the stretch between is skipped; longer, the difference comes out as silence,
    /// then the stream resumes. An automated latency switch then costs at most the change,
    /// never a dropout of the whole delay. No allocation: the ring is turned in place.
    pub fn retime(&mut self, delay: usize) {
        let new = delay.clamp(1, self.len());
        let old = self.delay;
        if new == old {
            return;
        }
        // Oldest first from index 0.
        self.buf_l[..old].rotate_left(self.pos);
        self.buf_r[..old].rotate_left(self.pos);
        if new < old {
            let s = old - new;
            self.buf_l.copy_within(s..old, 0);
            self.buf_r.copy_within(s..old, 0);
        } else {
            let s = new - old;
            self.buf_l.copy_within(0..old, s);
            self.buf_r.copy_within(0..old, s);
            self.buf_l[..s].iter_mut().for_each(|x| *x = 0.0);
            self.buf_r[..s].iter_mut().for_each(|x| *x = 0.0);
        }
        self.delay = new;
        self.pos = 0;
        // Whatever it held may still come out.
        self.silent = 0;
    }

    pub fn delay(&self) -> usize {
        self.delay
    }

    /// Still holding signal that has not come out.
    pub fn ringing(&self) -> bool {
        self.silent < self.delay
    }

    /// Delay a stereo block in place. The ring is exactly `delay` long: each sample reads
    /// what was written `delay` samples ago, then takes its place.
    #[inline]
    pub fn run(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        for i in 0..n {
            let (ol, or) = (self.buf_l[self.pos], self.buf_r[self.pos]);
            self.buf_l[self.pos] = l[i];
            self.buf_r[self.pos] = r[i];
            if l[i] == 0.0 && r[i] == 0.0 {
                self.silent = self.silent.saturating_add(1);
            } else {
                self.silent = 0;
            }
            self.pos += 1;
            if self.pos == self.delay {
                self.pos = 0;
            }
            l[i] = ol;
            r[i] = or;
        }
    }
}

impl Default for DelayLine {
    fn default() -> Self {
        Self::new()
    }
}

/// The pool of lines, each behind its own cell (`cantus_rt::cell`): the node that owns an
/// edge runs its line during a pass, on whichever thread runs the node; the planner below
/// reassigns them between passes, on the callback. Shared by both through an `Arc`.
#[derive(Debug)]
pub struct Lines(Box<[NodeCell<DelayLine>]>);

impl Lines {
    /// [`POOL`] places, the first [`START_LINES`] holding a start-up line and the rest
    /// empty (ADR 0107): more is memory only once a project can need it.
    pub fn new() -> Self {
        Self::with(START_LINES)
    }

    /// [`POOL`] places, the first `n` holding a start-up line.
    pub fn with(n: usize) -> Self {
        Lines(
            (0..POOL)
                .map(|i| NodeCell::new(DelayLine::with_len(if i < n { START_LEN } else { 0 })))
                .collect(),
        )
    }

    /// Line `i`'s cell.
    #[inline]
    pub fn cell(&self, i: usize) -> Option<&NodeCell<DelayLine>> {
        self.0.get(i)
    }

    /// Line `i`, exclusively — `None` while a node holds it (never, between passes).
    #[inline]
    pub fn get(&self, i: usize) -> Option<cantus_rt::cell::NodeMut<'_, DelayLine>> {
        self.0.get(i)?.try_borrow_mut()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for Lines {
    fn default() -> Self {
        Self::new()
    }
}

/// The pool, and which line each edge uses.
#[derive(Clone, Debug)]
pub struct Compensation {
    pub lines: Arc<Lines>,
    /// Each line's edge, delay and length, mirrored here so planning reads plain arrays.
    edge: [Option<Edge>; POOL],
    len: [usize; POOL],
    delay: [usize; POOL],
    /// The total delay at the master: what the output lags the playhead by.
    pub total: usize,
    /// Edges that needed a line and found none long enough at the last snapshot.
    pub unserved: usize,
    /// What the unserved edges need: how many lines, and the longest delay among them.
    need: (usize, usize),
    /// The last request made, so an unchanged shortfall is not asked for every snapshot.
    asked: (usize, usize),
    /// Edges that want a line they do not have, given one in [`Self::finish`] once every
    /// kept edge has kept its own.
    pending: [(Edge, usize); POOL],
    pending_n: usize,
}

impl Compensation {
    /// The pool a processor starts with: [`START_LINES`] start-up lines.
    pub fn new() -> Self {
        Self::with_lines(START_LINES)
    }

    /// A pool of `n` start-up lines (0: none at all).
    pub fn with_lines(n: usize) -> Self {
        let n = n.min(POOL);
        let mut len = [0; POOL];
        len[..n].fill(START_LEN);
        Compensation {
            lines: Arc::new(Lines::with(n)),
            edge: [None; POOL],
            len,
            pending: [(Edge::Click, 0); POOL],
            pending_n: 0,
            delay: [0; POOL],
            total: 0,
            unserved: 0,
            need: (0, 0),
            asked: (0, 0),
        }
    }

    /// The line serving `edge`, if it has one.
    #[inline]
    pub fn line_for(&self, edge: Edge) -> Option<usize> {
        self.edge.iter().position(|e| *e == Some(edge))
    }

    /// The edge line `i` serves.
    #[inline]
    pub fn edge_of(&self, i: usize) -> Option<Edge> {
        self.edge.get(i).copied().flatten()
    }

    /// Line `i`'s delay.
    pub fn delay_of(&self, i: usize) -> usize {
        self.delay[i]
    }

    /// Give line `i` to `edge` at `delay`, emptied. A line held by a node (never between
    /// passes) is left as it was.
    fn assign(&mut self, i: usize, edge: Edge, delay: usize) {
        if let Some(mut l) = self.lines.get(i) {
            l.assign(edge, delay);
            self.edge[i] = Some(edge);
            self.delay[i] = l.delay();
        }
    }

    /// Line `i`, which keeps its edge, at a new delay with what it holds (see
    /// [`DelayLine::retime`]).
    fn retime(&mut self, i: usize, delay: usize) {
        if let Some(mut l) = self.lines.get(i) {
            l.retime(delay);
            self.delay[i] = l.delay();
        }
    }

    fn release(&mut self, i: usize) {
        self.edge[i] = None;
        if let Some(mut l) = self.lines.get(i) {
            l.edge = None;
        }
    }

    /// Start of a snapshot's assignment: every line is up for grabs, but keeps its edge
    /// until [`Self::want`] or [`Self::finish`] decides.
    pub fn begin(&mut self) -> [bool; POOL] {
        self.unserved = 0;
        self.need = (0, 0);
        self.pending_n = 0;
        [false; POOL]
    }

    /// `edge` needs `delay` samples. Keeps the line it had while it is long enough — at a
    /// new delay, with what it holds (`DelayLine::retime`).
    pub fn want(&mut self, kept: &mut [bool; POOL], edge: Edge, delay: usize) {
        if delay == 0 {
            return;
        }
        let delay = delay.min(MAX_DELAY);
        if let Some(i) = self.line_for(edge) {
            if self.len[i] >= delay {
                if self.delay[i] != delay {
                    self.retime(i, delay);
                }
                kept[i] = true;
                return;
            }
            // Too short for the new delay: let it go and look for a longer one.
            self.release(i);
        }
        // A new line, chosen in `finish`: once every edge that keeps its line has kept it,
        // no line an edge still wants is ever taken from it (ADR 0107 — a pool sized to
        // what the project needs has no spare lines to make that harmless).
        if self.pending_n < POOL {
            self.pending[self.pending_n] = (edge, delay);
            self.pending_n += 1;
        } else {
            self.unserved += 1;
            self.need = (self.need.0 + 1, self.need.1.max(delay));
        }
    }

    /// End of the assignment: lines no edge kept are freed, then every edge waiting for a
    /// line takes the shortest free one long enough, in the order it asked.
    pub fn finish(&mut self, kept: &[bool; POOL]) {
        let mut kept = *kept;
        for (i, k) in kept.iter().enumerate() {
            if !k && self.edge[i].is_some() {
                self.release(i);
            }
        }
        for p in 0..std::mem::take(&mut self.pending_n) {
            let (edge, delay) = self.pending[p];
            let free = (0..POOL)
                .filter(|i| !kept[*i] && self.edge[*i].is_none() && self.len[*i] >= delay)
                .min_by_key(|i| self.len[*i]);
            match free {
                Some(i) => {
                    self.assign(i, edge, delay);
                    kept[i] = true;
                }
                None => {
                    self.unserved += 1;
                    self.need = (self.need.0 + 1, self.need.1.max(delay));
                }
            }
        }
    }

    /// Line `i`'s length as the planner knows it (0: no line there yet).
    pub fn len_of(&self, i: usize) -> usize {
        self.len[i]
    }

    /// Lines in the pool (of any length).
    pub fn held(&self) -> usize {
        self.len.iter().filter(|l| **l > 0).count()
    }

    /// Lines to ask the UI thread for — `(count, longest delay)` — when the last snapshot
    /// left edges unserved for want of length and this shortfall has not been asked for.
    /// Asking can only help while some line is free to be swapped out.
    pub fn take_request(&mut self) -> Option<(usize, usize)> {
        if self.need.0 == 0 {
            self.asked = (0, 0);
            return None;
        }
        let free = self.edge.iter().filter(|e| e.is_none()).count();
        let want = (self.need.0.min(free), self.need.1);
        if want.0 == 0 || want == self.asked {
            return None;
        }
        self.asked = want;
        Some(want)
    }

    /// The request could not be sent: ask again at the next chance.
    pub fn forget_request(&mut self) {
        self.asked = (0, 0);
    }

    /// Swap `fresh` lines in for the shortest free ones. Afterwards `fresh` holds the lines
    /// taken out (and any fresh line that found no free, shorter place): hand it back to be
    /// freed off the audio thread. No allocation.
    pub fn supply(&mut self, fresh: &mut [DelayLine]) {
        for f in fresh.iter_mut() {
            let slot = (0..POOL)
                .filter(|i| self.edge[*i].is_none() && self.len[*i] < f.len())
                .min_by_key(|i| self.len[*i]);
            if let Some(i) = slot {
                if let Some(mut l) = self.lines.get(i) {
                    std::mem::swap(&mut *l, f);
                    self.len[i] = l.len();
                    self.delay[i] = l.delay();
                }
            }
        }
        self.asked = (0, 0);
    }
}

impl Default for Compensation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pool as it was before ADR 0107: every place holding a start-up line.
    fn pool() -> Compensation {
        Compensation::with_lines(POOL)
    }

    /// Review 1 (ADR 0056): a line whose delay changes while it runs keeps what it holds —
    /// shorter, it skips ahead; longer, it waits only the difference — instead of starting
    /// empty (a dropout of its whole delay at every automated latency switch).
    #[test]
    fn a_new_delay_keeps_what_the_line_holds() {
        let ramp =
            |a: usize, n: usize| -> Vec<f32> { (a..a + n).map(|v| (v + 1) as f32).collect() };
        let mut c = pool();
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Track(0), 100);
        c.finish(&kept);
        let i = c.line_for(Edge::Track(0)).unwrap();
        let run = |c: &Compensation, a: usize, n: usize| -> Vec<f32> {
            let mut l = ramp(a, n);
            let mut r = l.clone();
            c.lines.get(i).unwrap().run(&mut l, &mut r);
            l
        };
        let _ = run(&c, 0, 500);
        // Shorter: the very next sample is the one 60 back — no silence at all.
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Track(0), 60);
        c.finish(&kept);
        let y = run(&c, 500, 200);
        for (k, v) in y.iter().enumerate() {
            assert_eq!(*v, (500 + k + 1 - 60) as f32, "shorter, sample {k}");
        }
        // Longer: 40 samples of nothing (the difference), then the one 100 back.
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Track(0), 100);
        c.finish(&kept);
        let y = run(&c, 700, 200);
        for (k, v) in y.iter().enumerate() {
            let want = if k < 40 {
                0.0
            } else {
                (700 + k + 1 - 100) as f32
            };
            assert_eq!(*v, want, "longer, sample {k}");
        }
    }
    #[test]
    fn a_line_delays_by_exactly_its_delay_and_knows_when_it_is_empty() {
        let mut line = DelayLine::new();
        line.assign(Edge::Track(0), 3);
        let mut l = vec![1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0];
        let mut r = l.clone();
        line.run(&mut l[..4], &mut r[..4]);
        assert!(line.ringing());
        line.run(&mut l[4..], &mut r[4..]);
        assert_eq!(l, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 0.0]);
        assert_eq!(l, r);
        let mut z = vec![0.0; 3];
        let mut z2 = z.clone();
        line.run(&mut z, &mut z2);
        assert!(
            !line.ringing(),
            "three silent samples through a 3-sample line: empty"
        );
    }

    #[test]
    fn edges_keep_their_lines_while_their_delay_holds() {
        let mut c = pool();
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Out(2), 100);
        c.want(&mut kept, Edge::Send(2, 0), 0);
        c.finish(&kept);
        let first = c.line_for(Edge::Out(2)).unwrap();
        assert_eq!(c.lines.get(first).unwrap().edge, Some(Edge::Out(2)));
        assert!(c.line_for(Edge::Send(2, 0)).is_none(), "no delay, no line");
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Out(2), 100);
        c.finish(&kept);
        assert_eq!(c.line_for(Edge::Out(2)), Some(first));
        let mut kept = c.begin();
        c.finish(&kept);
        assert!(
            c.line_for(Edge::Out(2)).is_none(),
            "an edge no longer wanted is freed"
        );
        let _ = &mut kept;
    }

    #[test]
    fn a_delay_longer_than_every_line_is_asked_for_once_and_served_after_supply() {
        let mut c = pool();
        let long = START_LEN * 3;
        let plan = |c: &mut Compensation| {
            let mut kept = c.begin();
            c.want(&mut kept, Edge::Out(1), long);
            c.want(&mut kept, Edge::Out(2), long);
            c.want(&mut kept, Edge::Track(0), 10);
            c.finish(&kept);
        };
        plan(&mut c);
        assert_eq!(c.unserved, 2);
        assert!(
            c.line_for(Edge::Track(0)).is_some(),
            "short edges are served meanwhile"
        );
        assert_eq!(c.take_request(), Some((2, long)));
        plan(&mut c);
        assert_eq!(
            c.take_request(),
            None,
            "the same shortfall is not asked twice"
        );
        let mut fresh = lines_for(2, long);
        assert!(fresh.iter().all(|l| l.len() >= long));
        c.supply(&mut fresh);
        assert!(
            fresh.iter().all(|l| l.len() == START_LEN),
            "the short lines come back to be freed"
        );
        plan(&mut c);
        assert_eq!(c.unserved, 0);
        let i = c.line_for(Edge::Out(1)).unwrap();
        assert_eq!(c.delay_of(i), long);
        // And the long line delays by exactly its delay.
        let mut line = c.lines.get(i).unwrap();
        let mut l = vec![0.0; long + 2];
        l[0] = 1.0;
        let mut r = l.clone();
        line.run(&mut l, &mut r);
        assert_eq!(l.iter().position(|x| *x != 0.0), Some(long));
    }

    /// ADR 0107: a processor's pool starts with [`START_LINES`] lines (it was 192 lines of
    /// 2 × 32 KB, 12 MB, whether a project had latency or not), and can start with none.
    #[test]
    fn a_new_pool_holds_only_its_start_up_lines() {
        let c = Compensation::new();
        assert_eq!(c.held(), START_LINES);
        let held = (0..POOL).filter(|&i| c.lines.get(i).is_some_and(|l| !l.is_empty()));
        assert_eq!(held.count(), START_LINES);
        let none = Compensation::with_lines(0);
        assert_eq!(none.held(), 0);
        assert!((0..POOL).all(|i| none.lines.get(i).is_some_and(|l| l.is_empty())));
    }
    /// ADR 0107: with just the lines a project needs, a new edge never takes the line of an
    /// edge that keeps one — asked first or not, every kept edge keeps what its line holds.
    #[test]
    fn a_new_edge_never_takes_the_line_of_an_edge_that_keeps_one() {
        let mut c = Compensation::with_lines(0);
        c.supply(&mut short_lines(2));
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Out(1), 50);
        c.want(&mut kept, Edge::Out(2), 60);
        c.finish(&kept);
        let (a, b) = (c.line_for(Edge::Out(1)), c.line_for(Edge::Out(2)));
        assert!(a.is_some() && b.is_some());
        // The new edge asks first; the two old ones after it.
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Out(3), 70);
        c.want(&mut kept, Edge::Out(1), 50);
        c.want(&mut kept, Edge::Out(2), 60);
        c.finish(&kept);
        assert_eq!(c.line_for(Edge::Out(1)), a);
        assert_eq!(c.line_for(Edge::Out(2)), b);
        assert_eq!(c.line_for(Edge::Out(3)), None);
        assert_eq!(c.unserved, 1);
        assert_eq!(
            c.take_request(),
            Some((1, 70)),
            "asked for once: an empty place can take it"
        );
        // A third line arrives: the new edge has it at the next plan.
        c.supply(&mut short_lines(1));
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Out(3), 70);
        c.want(&mut kept, Edge::Out(1), 50);
        c.want(&mut kept, Edge::Out(2), 60);
        c.finish(&kept);
        assert!(c.line_for(Edge::Out(3)).is_some());
        assert_eq!((c.line_for(Edge::Out(1)), c.line_for(Edge::Out(2))), (a, b));
        assert_eq!(c.unserved, 0);
    }

    /// An empty pool asks for what an edge needs and is served by a start-up line.
    #[test]
    fn an_empty_pool_asks_and_a_short_delay_gets_a_start_up_line() {
        let mut c = Compensation::with_lines(0);
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Track(0), 64);
        c.finish(&kept);
        assert_eq!(c.unserved, 1);
        let (count, delay) = c.take_request().expect("asked");
        let mut fresh = lines_for(count, delay);
        assert!(fresh.iter().all(|l| l.len() == START_LEN));
        c.supply(&mut fresh);
        let mut kept = c.begin();
        c.want(&mut kept, Edge::Track(0), 64);
        c.finish(&kept);
        assert_eq!(c.unserved, 0);
        assert_eq!(c.held(), 1);
    }

    /// What a snapshot can need: nothing without a unit that can have latency; otherwise
    /// one line per edge that can be delayed.
    #[test]
    fn the_lines_a_snapshot_can_need() {
        use cantus_core::{EffectKind, Project, Snapshot};
        let mut p = Project::new("lines");
        for _ in 0..3 {
            let t = p.make_track("t");
            p.tracks.push(t);
        }
        assert_eq!(lines_needed(&Snapshot::compile(&p, 1)), 0, "no latency");
        let strip = p.tracks[0].strip.unwrap();
        let e = p.make_effect(EffectKind::Limiter);
        p.strip_mut(strip).unwrap().effects.push(e);
        let snap = Snapshot::compile(&p, 1);
        let want = snap.tracks.len()
            + snap
                .strips
                .iter()
                .map(|s| 1 + s.sends.len() + s.effects.len())
                .sum::<usize>()
            + snap.master_effects.len()
            + 2;
        assert_eq!(lines_needed(&snap), want.min(POOL));
        assert!(want > 3);
    }
}
