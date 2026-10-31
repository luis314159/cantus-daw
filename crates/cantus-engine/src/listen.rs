//! Listening (round 34, ADR 0083): a render an agent — or the mastering assistant — reads
//! numbers from, of a whole song or of one range of it, with **analysis taps** on every
//! strip and track it asks for, and **checkpoints** so a range plays from a saved state
//! instead of from the top.
//!
//! **The same render.** [`listen`] sets the processor up exactly as [`crate::render`] does
//! (the units, the automation's runtime, the planned graph, delay compensation settled,
//! the loop and the click off) and drives it block by block; what it gives for a range is,
//! sample for sample, what `render` gives from the song's start with `compensate_latency`
//! (the test `a_section_from_its_checkpoint_is_the_whole_render_to_the_bit` holds it).
//! The taps are the stems' (`crate::stems::StemTap`): they copy what is about to be mixed
//! and never write, so hearing changes no sample (`the_taps_change_no_sample`).
//!
//! **Checkpoints.** At each section start the caller names (markers, bars), the render
//! keeps a copy of the whole engine paused there ([`crate::Processor::fork`]). A later
//! range starts from the latest checkpoint at or before it whose snapshot plays the same
//! up to it ([`same_until`]): an agent that rewrote bars 65–72 hears them again without
//! rendering 1–64. A snapshot changed after the checkpoint is installed on the copy as
//! playback installs an edit. Where the engine cannot be copied — a hosted plug-in, a
//! frozen render, a voice streaming from disk — the range starts a **pre-roll** before
//! itself instead, and [`Heard::resumed`] says so and why.

use crate::effects::EffectUnit;
use crate::messages::{EngineEvent, EngineMessage};
use crate::processor::Processor;
use crate::render::RenderError;
use crate::stems::{StemSource, StemTap};
use cantus_core::{Snapshot, Ticks};
use cantus_dsp::speculum::ear::Ear;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;

#[path = "listen_prefix.rs"]
mod prefix;
pub use prefix::same_until;

/// The pre-roll a range takes when the engine cannot be copied, seconds: enough for a long
/// hall's tail to fall under −60 dB (measured, ADR 0083).
pub const DEFAULT_PREROLL_S: f64 = 8.0;
/// Checkpoints a [`Checkpoints`] keeps at most (each is a whole engine: the least
/// recently used goes): room for two setups of a song with a dozen sections — the song and
/// a solo, or the song and the assistant's mix (review 2: twelve evicted the song's own when
/// a variant of a 13-section song was heard). The showcase's ten took under 250 MB.
pub const DEFAULT_CHECKPOINTS: usize = 24;
/// The most the checkpoints of one project hold together, bytes (each one's estimated when
/// it is taken; the least recently used go past it — review 3). Internal: no setting shows it.
pub const DEFAULT_CHECKPOINT_BYTES: usize = 512 << 20;

/// What to render and hear.
#[derive(Clone, Debug, PartialEq)]
pub struct ListenOptions {
    pub sample_rate: u32,
    /// Internal block size: any value gives the same samples.
    pub block_frames: usize,
    /// The range: from here…
    pub start: Ticks,
    /// …to here (`None`: the song's end). A range that ends before the song stops there
    /// and its tail rings out, as an export's does.
    pub end: Option<Ticks>,
    pub tail_seconds: f64,
    /// Strips and tracks to hear besides the master, each aligned to the range's start.
    pub taps: Vec<StemSource>,
    /// Keep the master's samples ([`Heard::audio`]), to write the WAV an agent listens to.
    pub keep_audio: bool,
    /// Where to take checkpoints (section starts), in any order; those past the range's
    /// end are not reached.
    pub sections: Vec<Ticks>,
    /// The pre-roll when the engine cannot be copied, seconds; `f64::INFINITY` renders from
    /// the song's start instead (exact, slower).
    pub preroll_seconds: f64,
}

impl Default for ListenOptions {
    fn default() -> Self {
        ListenOptions {
            sample_rate: 48_000,
            block_frames: 512,
            start: Ticks::ZERO,
            end: None,
            tail_seconds: 2.0,
            taps: Vec::new(),
            keep_audio: false,
            sections: Vec::new(),
            preroll_seconds: DEFAULT_PREROLL_S,
        }
    }
}

/// Where a range's render began.
#[derive(Clone, Debug, PartialEq)]
pub enum Resumed {
    /// From the song's start: exact.
    Start,
    /// From a checkpoint at this tick: exact.
    Checkpoint(Ticks),
    /// From this tick, a pre-roll of `seconds` before the range, because the engine could
    /// not be copied (`why`): close, not exact — tails and envelopes started before the
    /// pre-roll are missing.
    Preroll {
        from: Ticks,
        seconds: f64,
        why: String,
    },
}

impl Resumed {
    pub fn is_exact(&self) -> bool {
        !matches!(self, Resumed::Preroll { .. })
    }
}

/// What a listen heard.
#[derive(Clone, Debug)]
pub struct Heard {
    pub sample_rate: u32,
    /// The master's output from the range's first sample (sample 0 of the ear and of
    /// `audio`), the tail included.
    pub master: Ear,
    /// Each tap asked for, in order; a source the engine does not hold hears silence.
    pub taps: Vec<(StemSource, Ear)>,
    /// The master's samples, interleaved stereo (empty unless `keep_audio`).
    pub audio: Vec<f32>,
    /// Frames of the range itself, before its tail.
    pub range_frames: usize,
    pub resumed: Resumed,
    /// Frames the engine processed for this call (how much it saved: compare with the
    /// range's start in samples).
    pub rendered_frames: usize,
    /// The project's total latency, samples (what the output was aligned by).
    pub latency: usize,
    /// Checkpoints taken during this call.
    pub checkpoints_taken: usize,
    /// Notes that played only the start of a streamed sample.
    pub cut_notes: u64,
}

/// One engine paused at a section's start.
struct Checkpoint {
    tick: Ticks,
    /// The transport's sample there (the processor's frames since the song's start).
    frame: u64,
    snapshot: Arc<Snapshot>,
    sample_rate: u32,
    block: usize,
    rig: Rig,
    used: u64,
    /// What its image holds, estimated (`Processor::approx_bytes`).
    bytes: usize,
}

/// The checkpoints of one project, kept between listens (see the module notes).
pub struct Checkpoints {
    entries: Vec<Checkpoint>,
    /// The last project whose engine could not be copied at a checkpoint, and why: a later
    /// listen of the same setup pre-rolls and says so instead of rendering from the top
    /// every time in silence (review 1).
    refused: Option<(Arc<Snapshot>, String)>,
    /// At most this many are kept.
    pub limit: usize,
    /// And at most this many bytes, estimated (the least recently used go first).
    pub budget_bytes: usize,
    clock: u64,
}

impl Default for Checkpoints {
    fn default() -> Self {
        Checkpoints {
            entries: Vec::new(),
            refused: None,
            limit: DEFAULT_CHECKPOINTS,
            budget_bytes: DEFAULT_CHECKPOINT_BYTES,
            clock: 0,
        }
    }
}

impl std::fmt::Debug for Checkpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkpoints")
            .field("ticks", &self.ticks())
            .field("limit", &self.limit)
            .field("bytes", &self.bytes())
            .finish()
    }
}

impl Checkpoints {
    /// The ticks checkpoints are held at, in order.
    pub fn ticks(&self) -> Vec<Ticks> {
        let mut t: Vec<Ticks> = self.entries.iter().map(|c| c.tick).collect();
        t.sort();
        t
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// What the checkpoints hold together, bytes, estimated.
    pub fn bytes(&self) -> usize {
        self.entries.iter().map(|c| c.bytes).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Forget every checkpoint (the project was replaced).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.refused = None;
    }

    /// A fork of `snap`'s engine failed for `refusal`: remembered only when it lasts (a
    /// moment's — a snapshot waiting for the grid — may copy at the next place). Whether the
    /// render should stop trying.
    fn note_refusal(
        &mut self,
        snap: &Arc<Snapshot>,
        refusal: Option<crate::processor::Refusal>,
    ) -> bool {
        match refusal {
            Some(r) if r.lasting => {
                self.refused = Some((snap.clone(), r.why.to_string()));
                true
            }
            _ => false,
        }
    }

    /// Why `snap`'s engine could not be copied the last time, when its setup is the same
    /// (everything but what happens in time compared, as `same_until` the song's start).
    fn refusal_for(&self, snap: &Snapshot) -> Option<String> {
        let (s, why) = self.refused.as_ref()?;
        same_until(s, snap, Ticks::ZERO).then(|| why.clone())
    }

    /// The latest usable checkpoint at or before `start` for `snap` (see the module notes).
    fn best(&mut self, snap: &Snapshot, start: Ticks, sr: u32, block: usize) -> Option<usize> {
        let mut found: Option<usize> = None;
        for (i, c) in self.entries.iter().enumerate() {
            if c.tick > start || c.sample_rate != sr || c.block != block {
                continue;
            }
            if found.is_some_and(|j| self.entries[j].tick >= c.tick) {
                continue;
            }
            if same_until(&c.snapshot, snap, c.tick) {
                found = Some(i);
            }
        }
        if let Some(i) = found {
            self.clock += 1;
            self.entries[i].used = self.clock;
        }
        found
    }

    fn holds(&self, snap: &Snapshot, tick: Ticks, sr: u32, block: usize) -> bool {
        self.entries.iter().any(|c| {
            c.tick == tick
                && c.sample_rate == sr
                && c.block == block
                && same_until(&c.snapshot, snap, tick)
        })
    }

    fn keep(&mut self, mut c: Checkpoint) {
        self.clock += 1;
        c.used = self.clock;
        // Kept beside every other at the same place: a solo, a mute, the master out or the
        // assistant's mix are other setups, each its own checkpoints (review 1: they replaced
        // the song's own). What it does replace: the same setup's checkpoint at the same
        // place from before an edit that came earlier in the song — stale, never usable
        // again by this setup (review 2: those filled the room first).
        let stale = |e: &Checkpoint| {
            e.tick == c.tick
                && e.sample_rate == c.sample_rate
                && e.block == c.block
                && same_until(&e.snapshot, &c.snapshot, Ticks::ZERO)
                && !same_until(&e.snapshot, &c.snapshot, c.tick)
        };
        self.entries.retain(|e| !stale(e));
        self.entries.push(c);
        while self.entries.len() > self.limit.max(1)
            || (self.bytes() > self.budget_bytes && !self.entries.is_empty())
        {
            let oldest = (0..self.entries.len())
                .min_by_key(|&i| self.entries[i].used)
                .unwrap_or(0);
            self.entries.swap_remove(oldest);
        }
    }
}

/// A processor and the rings the render talks to it through.
struct Rig {
    p: Processor,
    tx: Producer<EngineMessage>,
    rx: Consumer<EngineEvent>,
    /// The slots its lineage was ever given (what a copy must take — see `Cells`).
    cells: crate::processor::Cells,
}

/// Rings a fork talks through: room for an install's messages, and for a block's events.
const FORK_MESSAGES: usize = 64;
const FORK_EVENTS: usize = 1_024;

impl Rig {
    /// A copy that plays on as this one would, or `None` (see [`Processor::fork`]): from a
    /// checkpoint's image, a full processor.
    fn resume(&self) -> Option<Rig> {
        let (p, tx, rx) = self
            .p
            .fork_cells(FORK_MESSAGES, FORK_EVENTS, &self.cells, false)?;
        Some(Rig {
            p,
            tx,
            rx,
            cells: self.cells.clone(),
        })
    }

    /// What a checkpoint keeps: a copy holding only the cells in use, never played — only
    /// [`Self::resume`]d (review 3: a whole engine was 75 MB a checkpoint).
    fn image(&self) -> Option<Rig> {
        let (p, tx, rx) = self.p.fork_cells(1, 1, &self.cells, true)?;
        Some(Rig {
            p,
            tx,
            rx,
            cells: self.cells.clone(),
        })
    }

    /// Install what was sent, answering what the engine asks for on the way (longer
    /// compensation lines, an effect's memory) — between blocks, as `render` does before
    /// its first.
    fn settle(&mut self, sr: f32) {
        self.p.settle();
        self.cells.note(&self.p);
        while let Ok(ev) = self.rx.pop() {
            match ev {
                EngineEvent::PdcNeed { count, delay } => {
                    self.tx
                        .push(EngineMessage::PdcLines(crate::pdc::lines_for(count, delay)))
                        .ok();
                    self.p.settle();
                    self.cells.note(&self.p);
                }
                EngineEvent::EffectNeed { effect, need } => {
                    let memory = crate::effects::EffectMemory::build(need, sr);
                    self.tx
                        .push(EngineMessage::EffectMemory { effect, memory })
                        .ok();
                    self.p.settle();
                    self.cells.note(&self.p);
                }
                _ => {}
            }
        }
    }

    /// Install `snap` on a running copy, as playback installs an edit: the automation's
    /// runtime first, then the snapshot with its plan.
    fn install(&mut self, snap: &Arc<Snapshot>, sr: f32) {
        self.tx
            .push(EngineMessage::Automation(
                crate::automation::Runtime::build(snap),
            ))
            .ok();
        let plan = crate::graph::compile(snap);
        self.tx
            .push(EngineMessage::SetSnapshotPlanned(snap.clone(), plan))
            .ok();
        self.settle(sr);
    }
}

/// The snapshot as a render plays it: no loop, no click, audio clips at the render's rate.
fn prepare(snapshot: &Snapshot, sample_rate: u32) -> Arc<Snapshot> {
    let mut snap = snapshot.clone();
    snap.loop_region = None;
    snap.metronome = false;
    crate::samples::conform_audio(&mut snap, sample_rate);
    Arc::new(snap)
}

/// A processor set up for `snap` exactly as [`crate::render`] sets one up, the tap on,
/// not yet playing.
fn fresh(
    snap: &Arc<Snapshot>,
    sr: u32,
    block: usize,
    mut units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
) -> Rig {
    let srf = sr as f32;
    let effect_count = snap.effect_slots().count();
    let mut instruments = crate::inst::units_for(snap, srf, |_, _| false);
    for u in &mut instruments {
        u.unit.set_offline(true);
    }
    instruments.extend(hosted.into_iter().map(Box::new));
    let (tx, rx_engine) = RingBuffer::<EngineMessage>::new(16 + effect_count + instruments.len());
    let (tx_engine, rx) = RingBuffer::<EngineEvent>::new(64 + effect_count + instruments.len());
    let mut p = Processor::without_midi(sr as f64, rx_engine, tx_engine);
    p.set_profiling(false);
    p.set_offline(true);
    let mut rig = Rig {
        p,
        tx,
        rx,
        cells: Default::default(),
    };
    for slot in snap.effect_slots() {
        let unit = match units.iter().position(|u| u.id == slot.id) {
            Some(i) => units.swap_remove(i),
            None => EffectUnit::new(slot, srf),
        };
        rig.tx.push(EngineMessage::AddEffect(Box::new(unit))).ok();
    }
    for unit in instruments {
        rig.tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    rig.tx
        .push(EngineMessage::Automation(
            crate::automation::Runtime::build(snap),
        ))
        .ok();
    let plan = crate::graph::compile(snap);
    rig.tx
        .push(EngineMessage::SetSnapshotPlanned(snap.clone(), plan))
        .ok();
    rig.settle(srf);
    rig.p.attach_stem_tap(Box::new(StemTap::new(block)));
    rig
}

/// Why `snap` could not be copied at a checkpoint, before trying: what a hosted plug-in,
/// a frozen render or a streamed sample makes uncopiable (the fork itself is the last word).
fn uncopiable(
    snap: &Snapshot,
    units: &[EffectUnit],
    hosted: &[crate::inst::NativeUnit],
) -> Option<String> {
    if !units.is_empty() || !hosted.is_empty() {
        return Some("a hosted plug-in's state belongs to the plug-in".into());
    }
    if snap.tracks.iter().any(|t| t.frozen.is_some())
        || snap.strips.iter().any(|s| s.frozen.is_some())
    {
        return Some("a frozen render is read from disk".into());
    }
    let streams =
        |d: &Option<Arc<cantus_core::SampleData>>| d.as_ref().is_some_and(|d| d.stream.is_some());
    if snap.tracks.iter().any(|t| {
        t.samples.iter().any(streams)
            || t.assets.iter().any(|(_, a)| streams(a))
            || t.audio.iter().any(|c| streams(&c.data))
    }) {
        return Some("a sample streams from disk".into());
    }
    None
}

/// Where a source is kept in the tap and how late it is (as the stems trim it); `None`
/// for a source the engine does not hold (silence).
fn tapped(p: &Processor, s: StemSource) -> Option<(Option<crate::stems::Tapped>, usize)> {
    use crate::stems::Tapped;
    Some(match s {
        StemSource::Master => (None, p.stats().latency_samples as usize),
        StemSource::Direct => (Some(Tapped::Direct), p.master_arrival()),
        StemSource::Track(i) => (Some(Tapped::Track(p.track_slot(i)?)), p.track_latency(i)),
        StemSource::Strip(i) => (Some(Tapped::Strip(p.strip_slot(i)?)), p.strip_latency(i)),
    })
}

/// One source being heard: where the tap keeps it, how late it is, its ear.
struct Hearing {
    source: StemSource,
    /// `None`: a source the engine does not hold; `Some(None)`: the master's output.
    at: Option<Option<crate::stems::Tapped>>,
    late: usize,
    ear: Ear,
}

/// Render `snapshot` over `opts`' range and hear it: the master and every tap, each from
/// the range's first sample (see the module notes). `units` and `hosted` are the hosted
/// plug-ins' and instruments' own instances for this render, as
/// [`crate::render_with_progress`] takes them; `progress` hears the fraction done after
/// every block and stops the render by answering `false`. `checkpoints` is read for a place
/// to start from and given the sections this render passes.
pub fn listen(
    snapshot: &Snapshot,
    opts: &ListenOptions,
    units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
    checkpoints: &mut Checkpoints,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<Heard, RenderError> {
    let sr = opts.sample_rate.max(1);
    let srf = sr as f32;
    let block = opts
        .block_frames
        .clamp(1, crate::messages::MAX_BLOCK_FRAMES);
    let end = opts.end.unwrap_or(snapshot.content_end);
    if end <= opts.start && opts.end.is_none() {
        return Err(RenderError::Empty);
    }
    let snap = prepare(snapshot, sr);
    let map = snap.tempo.clone();
    let at = |t: Ticks| crate::transport::sample_at_map(&map, 0.0, t.0 as f64, sr as f64);
    // The range in song samples (the output's frames once the latency is taken off).
    let s0 = at(opts.start);
    // Counted from the song's start, as a render from the top counts them, so a range's
    // last frame is that render's (a tempo whose ticks fall between samples rounded the two
    // differently — review 1).
    let from_top = |secs: f64| (secs * sr as f64).ceil() as u64;
    let to_end = map.seconds_between(Ticks::ZERO, end.max(opts.start));
    let range_frames = from_top(to_end).saturating_sub(s0) as usize;
    let total = from_top(to_end + opts.tail_seconds.max(0.0)).saturating_sub(s0) as usize;
    let stop_song = opts.end.map(at);
    // Checkpoint places: the transport's sample at each section start, down to the
    // automation's control grid (a snapshot installed there waits for nothing).
    let grid = crate::automation::GRID;
    let mut places: Vec<(Ticks, u64)> = opts
        .sections
        .iter()
        .filter(|t| **t > Ticks::ZERO && **t <= end)
        .map(|t| (*t, at(*t) / grid * grid))
        .collect();
    places.sort_by_key(|p| p.1);
    places.dedup_by_key(|p| p.1);

    // Where to begin.
    let why = uncopiable(&snap, &units, &hosted).or_else(|| checkpoints.refusal_for(&snap));
    let cannot_copy = why.is_some();
    let found = if why.is_none() {
        checkpoints.best(&snap, opts.start, sr, block)
    } else {
        None
    };
    let resumed_from = found.and_then(|i| {
        let c = &checkpoints.entries[i];
        let mut rig = c.rig.resume()?;
        if *c.snapshot != *snap {
            rig.install(&snap, srf);
        }
        Some((rig, c.frame, Resumed::Checkpoint(c.tick)))
    });
    let (mut rig, mut pos, resumed) = match resumed_from {
        Some(r) => r,
        None => begin(
            &snap,
            sr,
            block,
            units,
            hosted,
            opts.start,
            opts.preroll_seconds,
            why,
        ),
    };
    let start_pos = pos;
    let latency = rig.p.stats().latency_samples as usize;
    let seconds_hint = total as f64 / sr as f64;
    let mut master = Ear::new(srf, seconds_hint);
    let mut taps: Vec<Hearing> = opts
        .taps
        .iter()
        .map(|&source| {
            let t = tapped(&rig.p, source);
            Hearing {
                source,
                at: t.map(|t| t.0),
                late: t.map_or(0, |t| t.1),
                ear: Ear::new(srf, seconds_hint),
            }
        })
        .collect();
    let mut audio = Vec::with_capacity(if opts.keep_audio { total * 2 } else { 0 });
    // The engine plays until every source has given the range and its tail.
    let longest = taps
        .iter()
        .map(|h| h.late)
        .chain([latency])
        .max()
        .unwrap_or(0) as u64;
    let last = s0 + total as u64 + longest;
    let mut buf = vec![0f32; block * 2];
    let (mut ml, mut mr) = (vec![0f32; block], vec![0f32; block]);
    let zeros = vec![0f32; block];
    let mut taken = 0;
    // A setup whose engine cannot be copied (known before, or found at a place): no more
    // attempts (review 2: an exact render of it tried, and failed, at every place).
    let mut refused = cannot_copy;
    let mut stopped = stop_song.is_some_and(|s| pos > s);
    let span = last.saturating_sub(pos).max(1) as f32;
    while pos < last {
        if stop_song == Some(pos) && !stopped {
            rig.tx.push(EngineMessage::Stop).ok();
            stopped = true;
        }
        // A checkpoint here, unless one holds this place already.
        // Only from a render that began exactly: a pre-roll's state is close, never the
        // song's (review 1: it held only because a pre-roll's fork failed anyway).
        let exact = resumed.is_exact() && !refused;
        if let Some(&(tick, _)) = places.iter().find(|p| p.1 == pos && exact) {
            if !checkpoints.holds(&snap, tick, sr, block) {
                let copy = rig.image();
                // Remembered only when it lasts (review 2): a moment's refusal — a snapshot
                // waiting for the grid — may copy at the next place.
                if copy.is_none() {
                    refused = checkpoints.note_refusal(&snap, rig.p.fork_refusal());
                }
                if let Some(mut copy) = copy {
                    copy.p.set_workers(crate::processor::Workers::SERIAL);
                    let bytes = copy.p.approx_bytes();
                    checkpoints.keep(Checkpoint {
                        tick,
                        frame: pos,
                        snapshot: snap.clone(),
                        sample_rate: sr,
                        block,
                        rig: copy,
                        used: 0,
                        bytes,
                    });
                    taken += 1;
                }
            }
        }
        // The block ends at the next place, the stop, or the end.
        let mut n = (block as u64).min(last - pos);
        let stop_edge = stop_song.filter(|_| !stopped);
        for edge in places.iter().map(|p| p.1).chain(stop_edge) {
            if edge > pos && edge < pos + n {
                n = edge - pos;
            }
        }
        let n = n as usize;
        let chunk = &mut buf[..n * 2];
        rig.p.process(chunk, 2);
        // A snapshot waiting for the control grid installs inside a block.
        rig.cells.note(&rig.p);
        for ((f, a), b) in chunk.chunks_exact(2).zip(ml.iter_mut()).zip(mr.iter_mut()) {
            *a = f[0];
            *b = f[1];
        }
        // This block's frames in a source's own time, kept from s0 for `total`.
        let window = |late: usize| {
            let first = pos as i64 - late as i64;
            let from = (s0 as i64 - first).clamp(0, n as i64) as usize;
            let to = ((s0 + total as u64) as i64 - first).clamp(0, n as i64) as usize;
            (from, to.max(from))
        };
        let (a, b) = window(latency);
        master.push(&ml[a..b], &mr[a..b]);
        if opts.keep_audio {
            audio.extend_from_slice(&chunk[a * 2..b * 2]);
        }
        if !taps.is_empty() {
            let tap = rig.p.stem_tap().expect("attached at the start");
            for h in &mut taps {
                let (a, b) = window(h.late);
                match h.at {
                    Some(Some(t)) => {
                        let [l, r] = tap.pair(t);
                        h.ear.push(&l[a..b], &r[a..b]);
                    }
                    Some(None) => h.ear.push(&ml[a..b], &mr[a..b]),
                    None => h.ear.push(&zeros[a..b], &zeros[a..b]),
                }
            }
        }
        pos += n as u64;
        while rig.rx.pop().is_ok() {}
        if !progress((pos - start_pos) as f32 / span) {
            return Err(RenderError::Cancelled);
        }
    }
    if rig.p.stats().stream_late > 0 {
        return Err(RenderError::Unreadable(crate::render::unreadable_in(&snap)));
    }
    Ok(Heard {
        sample_rate: sr,
        master,
        taps: taps.into_iter().map(|h| (h.source, h.ear)).collect(),
        audio,
        range_frames,
        resumed,
        rendered_frames: (pos - start_pos) as usize,
        latency,
        checkpoints_taken: taken,
        cut_notes: rig.p.stats().stream_cut,
    })
}

/// A fresh processor for a listen: from the song's start (exact) or, when the engine
/// cannot be copied (`why`), from a pre-roll of `preroll_s` before `start`. Returns the
/// rig, the transport's sample it starts at, and how.
#[allow(clippy::too_many_arguments)]
fn begin(
    snap: &Arc<Snapshot>,
    sr: u32,
    block: usize,
    units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
    start: Ticks,
    preroll_s: f64,
    why: Option<String>,
) -> (Rig, u64, Resumed) {
    let mut rig = fresh(snap, sr, block, units, hosted);
    let map = &snap.tempo;
    let (from, resumed) = match why {
        // A pre-roll that reaches back to the song's start is the exact render from it.
        Some(why) if map.seconds_at(start.0 as f64) > preroll_s.max(0.0) => {
            let back = map.seconds_at(start.0 as f64) - preroll_s.max(0.0);
            let from = Ticks(map.tick_at_seconds(back).floor().max(0.0) as i64);
            let seconds = map.seconds_between(from, start);
            (from, Resumed::Preroll { from, seconds, why })
        }
        _ => (Ticks::ZERO, Resumed::Start),
    };
    // The transport starts on the whole sample where `from` is reached — a render from the
    // top counts its samples from 0, so a pre-roll anchored at the fractional sample of a
    // tick would meet every event and every synced phase a fraction of a sample apart.
    let pos = crate::transport::sample_at_map(map, 0.0, from.0 as f64, sr as f64);
    if pos == 0 {
        rig.tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    } else {
        let at = map.tick_at_sample(pos as f64, sr as f64);
        rig.tx.push(EngineMessage::SeekFrac(at)).ok();
    }
    rig.tx.push(EngineMessage::Play).ok();
    (rig, pos, resumed)
}

/// Interleaved stereo `audio` through a fresh copy of the insert chain `chain` (each slot's
/// bypass and mix, the chain switched by `on`), trimmed by the chain's latency so sample 0
/// is the input's sample 0 and every input sample comes out: what a master chain does to a
/// mix, without rendering the song again — the mastering assistant's trials (ADR 0084).
/// Units that follow the song's position hear none here; the assistant's final numbers come
/// from a real render.
pub fn run_chain(
    chain: &[cantus_core::EffectSlot],
    on: bool,
    sample_rate: u32,
    audio: &[f32],
) -> Vec<f32> {
    const BLOCK: usize = 512;
    let sr = sample_rate as f32;
    let Some(mut t) =
        crate::stems::through::Through::of_chain(chain, on, sr, BLOCK, &mut |s, sr| {
            EffectUnit::new(s, sr)
        })
    else {
        return audio.to_vec();
    };
    let frames = audio.len() / 2;
    let late = t.latency();
    let mut out = Vec::with_capacity(audio.len());
    let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
    let mut at = 0;
    while at < frames + late {
        let n = BLOCK.min(frames + late - at);
        for i in 0..n {
            let f = at + i;
            (l[i], r[i]) = if f < frames {
                (audio[2 * f], audio[2 * f + 1])
            } else {
                (0.0, 0.0)
            };
        }
        t.run(&l[..n], &r[..n]);
        let (ol, or) = t.out(n);
        for i in 0..n {
            if at + i >= late && out.len() < audio.len() {
                out.push(ol[i]);
                out.push(or[i]);
            }
        }
        at += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::Refusal;

    /// Review 3: the checkpoints keep to their budget in bytes as well as to their count —
    /// the least recently used go first — and one kept still serves its section.
    #[test]
    fn the_checkpoints_keep_to_their_budget_in_bytes() {
        let mut p = cantus_core::Project::new("budget");
        let mut t = p.make_track("Lead");
        let bar = Ticks::beats(4);
        let mut c = p.make_clip("c", Ticks::ZERO, bar * 8);
        for b in 0..32 {
            let n = p.make_note(Ticks::beats(b), Ticks::QUARTER, 60 + (b % 5) as u8, 100);
            c.notes.push(n);
        }
        t.clips.push(c);
        p.tracks.push(t);
        let snap = Snapshot::compile(&p, 1);
        let opts = ListenOptions {
            sample_rate: 48_000,
            sections: (1..8).map(|i| bar * i).collect(),
            ..Default::default()
        };
        let mut all = Checkpoints::default();
        listen(&snap, &opts, vec![], vec![], &mut all, &mut |_| true).unwrap();
        assert_eq!(all.len(), 7);
        let one = all.entries.iter().map(|c| c.bytes).max().unwrap();
        assert!(one < 8 << 20, "an image of a one-track song: {one} bytes");
        let mut tight = Checkpoints {
            budget_bytes: one * 3 + one / 2,
            ..Default::default()
        };
        listen(&snap, &opts, vec![], vec![], &mut tight, &mut |_| true).unwrap();
        assert_eq!(tight.len(), 3);
        assert!(tight.bytes() <= tight.budget_bytes);
        assert_eq!(tight.ticks(), vec![bar * 5, bar * 6, bar * 7]);
        let late = ListenOptions {
            start: bar * 7,
            ..opts.clone()
        };
        let h = listen(&snap, &late, vec![], vec![], &mut tight, &mut |_| true).unwrap();
        assert_eq!(h.resumed, Resumed::Checkpoint(bar * 7));
    }

    /// What a checkpoint's image holds for each unit kind (review 3: the figures
    /// `Processor::approx_bytes` goes by): a one-track song with that instrument, or with
    /// four of that insert on its strip, imaged sixteen times, against the song with neither.
    #[test]
    #[ignore]
    fn what_each_unit_adds_to_an_image() {
        use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec, Project};
        let ws = || cantus_rt::usage::working_set_bytes().unwrap_or(0) as f64;
        let image = |p: &Project| -> (f64, usize) {
            let snap = prepare(&Snapshot::compile(p, 1), 48_000);
            let mut rig = fresh(&snap, 48_000, 64, vec![], vec![]);
            rig.tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
            rig.tx.push(EngineMessage::Play).ok();
            let mut buf = vec![0f32; 128];
            for _ in 0..200 {
                rig.p.process(&mut buf, 2);
                while rig.rx.pop().is_ok() {}
            }
            rig.cells.note(&rig.p);
            let w0 = ws();
            let kept: Vec<Rig> = (0..16).map(|_| rig.image().expect("an image")).collect();
            let each = (ws() - w0) / 16.0;
            let est = kept[0].p.approx_bytes();
            (each, est)
        };
        let song = || {
            let mut p = Project::new("unit");
            let mut t = p.make_track("Lead");
            let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(16));
            for i in 0..16 {
                let n = p.make_note(Ticks::beats(i), Ticks::QUARTER, 60, 100);
                c.notes.push(n);
            }
            t.clips.push(c);
            p.tracks.push(t);
            p
        };
        let (base, base_est) = image(&song());
        println!(
            "base: {:.2} MB measured, {:.2} estimated",
            base / 1e6,
            base_est as f64 / 1e6
        );
        for k in EffectKind::ALL
            .iter()
            .filter(|k| **k != EffectKind::External)
        {
            // Four of it, so the noise of one image's heap is a quarter.
            let mut p = song();
            let s = p.tracks[0].strip.unwrap();
            for _ in 0..4 {
                let slot = p.make_effect(*k);
                p.strip_mut(s).unwrap().effects.push(slot);
            }
            let (m, e) = image(&p);
            println!(
                "{k:?}: +{:.0} KB each measured, +{:.0} estimated",
                (m - base) / 4e3,
                (e as f64 - base_est as f64) / 4e3
            );
        }
        for k in InstrumentKind::ALL.iter() {
            let mut p = song();
            p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(*k));
            let (m, e) = image(&p);
            println!(
                "{k:?}: +{:.0} KB measured, +{:.0} estimated",
                (m - base) / 1e3,
                (e as f64 - base_est as f64) / 1e3
            );
        }
    }

    /// Review 3: a moment's refusal is not remembered — the render keeps trying, and a later
    /// place still checkpoints; a lasting one is, and the same setup pre-rolls next time.
    #[test]
    fn a_passing_refusal_is_not_remembered_and_a_lasting_one_is() {
        let p = cantus_core::Project::new("refusal");
        let snap = Arc::new(Snapshot::compile(&p, 1));
        let mut cps = Checkpoints::default();
        let passing = Refusal {
            why: "a snapshot was waiting for the control grid",
            lasting: false,
        };
        assert!(!cps.note_refusal(&snap, Some(passing)), "keep trying");
        assert_eq!(cps.refusal_for(&snap), None);
        let lasting = Refusal {
            why: "a frozen render is read from disk",
            lasting: true,
        };
        assert!(cps.note_refusal(&snap, Some(lasting)));
        assert_eq!(
            cps.refusal_for(&snap).as_deref(),
            Some("a frozen render is read from disk")
        );
    }
}
