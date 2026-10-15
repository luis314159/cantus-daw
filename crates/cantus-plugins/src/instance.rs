//! The format-free heart of an exported plug-in: what the CLAP and VST3 layers share.
//!
//! - [`Shared`] is the instance's state both threads see: every parameter's value as an
//!   atomic, the latency the running effect reports, the telemetry for an editor.
//! - [`Processor`] is the audio thread's: the engine's own [`EffectUnit`] — the same code
//!   that runs an insert inside Cantus, reading the same `EffectSpec` — and the rule for
//!   when a parameter change takes effect.
//!
//! **When a change takes effect.** Every one of the seven DSPs updates its coefficients,
//! glides and detectors on a grid of [`GRID`] samples counted from the start of the
//! stream, not from the start of a block — that is what makes them independent of the
//! block size. A parameter change is applied the same way: at the first grid point at or
//! after its time. So a change lands at the same sample whatever blocks the host cuts,
//! the output is identical at any block size, and the unit is reconfigured at most once
//! per 16 samples however dense the automation.
//!
//! **The audio thread** (ADR 0005): `Processor::process` never allocates, locks or
//! blocks. Everything it touches is built in `Processor::new`, on the main thread.

use crate::desc::{self, Descriptor};
use crate::params::{self, Table};
use cantus_core::{EffectId, EffectSlot, EffectSpec};
use cantus_engine::{EffectUnit, FxNote, SongPosition, Telemetry, TELEMETRY_LEN};
use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Samples between the points where parameter changes take effect: the DSPs' own update
/// grid (`cantus_dsp::aequalis::UPDATE` and its siblings).
pub const GRID: u64 = 16;

/// The longest piece the DSP is handed at once. Hosts may send any block; the DSP is
/// block-size independent, so cutting is invisible, and it bounds the bypass line.
pub const MAX_CHUNK: usize = 4096;

/// How long the bypass takes to cross-fade, in seconds.
const BYPASS_FADE_S: f32 = 0.010;

/// The longest latency the bypass can keep, in seconds. Aequalis's linear phase at its
/// highest resolution is the longest of the family, 352 ms.
const BYPASS_MAX_DELAY_S: f32 = 0.5;

/// One parameter change for the audio thread, in the table's terms.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ParamEvent {
    /// Sample offset in the block.
    pub offset: u32,
    /// Index into the plug-in's [`Table`].
    pub index: u32,
    /// The plain value (already converted from the standard's form), or for a
    /// modulation the offset added to it, in CLAP's units for that parameter
    /// (`Param::to_clap`).
    pub value: f32,
    /// A CLAP modulation: an offset on top of the automated value, not a new value.
    pub modulation: bool,
}

/// A note from the host for a kind that hears notes (`EffectKind::takes_notes`: Tonus's
/// targets, Porta's MIDI trigger — ADR 0022), at a sample offset in the block: CLAP's note
/// port, VST3's event bus.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NoteIn {
    pub offset: u32,
    pub note: FxNote,
}

/// Notes one block can carry; beyond, the rest are dropped but an all-notes-off is kept.
pub const NOTE_CAPACITY: usize = 1024;

/// Keep a note in a buffer reserved for [`NOTE_CAPACITY`]: never allocates; when full,
/// only an all-notes-off still gets in (over the last note), so nothing stays stuck.
pub fn push_note(out: &mut Vec<NoteIn>, n: NoteIn) {
    if out.len() < out.capacity() {
        out.push(n);
    } else if n.note == FxNote::AllOff {
        if let Some(last) = out.last_mut() {
            *last = n;
        }
    }
}

/// The host's transport at the start of a block, as either standard gives it (CLAP's
/// `clap_event_transport`, VST3's `ProcessContext`): what a synced unit follows in another
/// DAW (ADR 0055). Fields a host does not fill are `None`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct HostTransport {
    pub playing: bool,
    /// BPM, when the host says.
    pub tempo: Option<f64>,
    /// Quarter notes from the song's start, when the host says.
    pub beats: Option<f64>,
    /// The current bar's start, quarter notes.
    pub bar_start: Option<f64>,
    pub time_sig: Option<(u16, u16)>,
    /// The loop, quarter notes, while it is on.
    pub loop_beats: Option<(f64, f64)>,
}

impl HostTransport {
    /// The engine's song position `offset` samples into a block that starts where `self`
    /// says, at `sample_rate`; `stream` is the samples since activation, the timeline when
    /// the host gives none.
    pub fn position(&self, offset: usize, sample_rate: f64, stream: u64) -> SongPosition {
        let tempo = self
            .tempo
            .filter(|t| t.is_finite() && *t > 0.0)
            .unwrap_or(cantus_dsp::modfx_clock::DEFAULT_TEMPO);
        let (beats, samples) = match self.beats.filter(|b| b.is_finite()) {
            // The block's start in samples at this tempo, then whole samples on from it:
            // the same integers whatever the block is cut into.
            Some(b) => {
                let start = (b * 60.0 / tempo * sample_rate).round() as i64;
                if self.playing {
                    (
                        b + offset as f64 * tempo / (60.0 * sample_rate),
                        start + offset as i64,
                    )
                } else {
                    (b, start)
                }
            }
            None => {
                let s = stream as i64 + offset as i64;
                (s as f64 * tempo / (60.0 * sample_rate), s)
            }
        };
        let (num, den) = self.time_sig.unwrap_or((4, 4));
        let bar = f64::from(num.max(1)) * 4.0 / f64::from(den.max(1));
        SongPosition {
            playing: self.playing,
            tempo_bpm: tempo,
            beats,
            samples,
            bar_start_beats: self.bar_start.unwrap_or((beats / bar).floor() * bar),
            time_sig_num: num,
            time_sig_den: den,
            loop_beats: self.loop_beats,
        }
    }
}

/// The host's transport as the audio thread last saw it, in atomics an editor reads (its
/// bar lines, a synced display's playhead).
#[derive(Debug, Default)]
struct HostView {
    /// A transport has been seen.
    known: AtomicBool,
    playing: AtomicBool,
    /// `f64` bits.
    tempo: AtomicU64,
    beats: AtomicU64,
    /// Numerator in the high half, denominator in the low.
    sig: AtomicU32,
}

/// What an editor reads of the host's transport.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HostSeen {
    pub playing: bool,
    pub tempo: f64,
    pub beats: f64,
    pub time_sig: (u16, u16),
}

/// One thing a plug-in's own editor did, in the plug-in's parameter indices, for the
/// format layer to apply and tell the host (CLAP output events, VST3's handler).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Edit {
    Begin(u32),
    /// A plain value.
    Value(u32, f32),
    End(u32),
}

/// What both threads see of one instance.
#[derive(Debug)]
pub struct Shared {
    pub desc: &'static Descriptor,
    pub table: &'static Table,
    /// Every parameter's current plain value, as `f32` bits. Written by the main thread
    /// (state, the editor, a host setting values while inactive) and by the audio thread
    /// (the host's automation), so a host or an editor reading it sees what plays.
    values: Box<[AtomicU32]>,
    /// Bumped each time the main thread changes values: the audio thread then takes all
    /// of them at its next grid point.
    generation: AtomicU32,
    /// The latency the running unit reports, samples.
    latency: AtomicU32,
    /// Set by the audio thread when the latency changed; the format layer tells the host
    /// from the main thread and clears it.
    latency_changed: AtomicBool,
    /// The editor is open: publish telemetry.
    pub editor_open: AtomicBool,
    telemetry: [AtomicU32; TELEMETRY_LEN],
    /// Blocks published since the start, so an editor can tell new telemetry from old.
    pub telemetry_serial: AtomicU32,
    /// The sample rate of the last activation, `f32` bits (0 before the first).
    sample_rate: AtomicU32,
    /// The host's transport as the audio thread last saw it, for an editor.
    host: HostView,
    /// The analyser tap's ring: (input, output) mono pairs from the audio thread to an
    /// open editor. Each end is lent out — the producer to the running [`Processor`], the
    /// consumer to the editor — and given back when they go; the lock is only ever taken
    /// on the main thread, when one of them is made or dropped.
    pairs: Mutex<TapEnds>,
    /// Speculum's frames its tap could not take (the editor drained too late), since the
    /// editor last asked (ADR 0056, review 1).
    pub scope_lost: AtomicU64,
}

/// The analyser tap's two ends, each here while nobody has it.
type TapEnds = (Option<Producer<(f32, f32)>>, Option<Consumer<(f32, f32)>>);

/// Pairs the tap's ring holds: about a third of a second at 48 kHz, more than an editor
/// frame ever waits.
pub const PAIRS: usize = 16_384;

impl Shared {
    pub fn new(desc: &'static Descriptor) -> Arc<Shared> {
        let table = params::table(desc.kind);
        Arc::new(Shared {
            desc,
            table,
            values: table
                .params
                .iter()
                .map(|p| AtomicU32::new(p.default.to_bits()))
                .collect(),
            generation: AtomicU32::new(0),
            latency: AtomicU32::new(0),
            latency_changed: AtomicBool::new(false),
            editor_open: AtomicBool::new(false),
            telemetry: [const { AtomicU32::new(0) }; TELEMETRY_LEN],
            telemetry_serial: AtomicU32::new(0),
            sample_rate: AtomicU32::new(0),
            host: HostView::default(),
            pairs: {
                let (tx, rx) = RingBuffer::new(PAIRS);
                Mutex::new((Some(tx), Some(rx)))
            },
            scope_lost: AtomicU64::new(0),
        })
    }

    /// The consumer end of the analyser tap, for an editor (main thread).
    pub fn take_pairs(&self) -> Option<Consumer<(f32, f32)>> {
        self.pairs.lock().ok().and_then(|mut p| p.1.take())
    }

    /// Give the consumer end back when the editor closes (main thread).
    pub fn return_pairs(&self, rx: Consumer<(f32, f32)>) {
        if let Ok(mut p) = self.pairs.lock() {
            p.1 = Some(rx);
        }
    }

    /// The host's transport as the audio thread last saw it; `None` until a host gave one.
    pub fn host_transport(&self) -> Option<HostSeen> {
        let h = &self.host;
        h.known.load(Ordering::Acquire).then(|| {
            let sig = h.sig.load(Ordering::Relaxed);
            HostSeen {
                playing: h.playing.load(Ordering::Relaxed),
                tempo: f64::from_bits(h.tempo.load(Ordering::Relaxed)),
                beats: f64::from_bits(h.beats.load(Ordering::Relaxed)),
                time_sig: ((sig >> 16) as u16, (sig & 0xFFFF) as u16),
            }
        })
    }

    /// The audio thread's view of the host's transport (no allocation, no lock).
    /// The transport at the start of a block of `frames`; what an editor reads is the
    /// position of the block's **last** frame (the newest it has drained — review 1).
    fn publish_transport(&self, t: &HostTransport, frames: usize) {
        let h = &self.host;
        let tempo = t.tempo.unwrap_or(cantus_dsp::modfx_clock::DEFAULT_TEMPO);
        let sr = f64::from(self.sample_rate()).max(1.0);
        let last = frames.saturating_sub(1) as f64 * tempo / 60.0 / sr;
        let (num, den) = t.time_sig.unwrap_or((4, 4));
        h.playing.store(t.playing, Ordering::Relaxed);
        h.tempo.store(tempo.to_bits(), Ordering::Relaxed);
        h.beats.store(
            t.beats.map_or(0.0, |b| b + if t.playing { last } else { 0.0 }).to_bits(),
            Ordering::Relaxed,
        );
        h.sig
            .store((u32::from(num) << 16) | u32::from(den), Ordering::Relaxed);
        h.known.store(true, Ordering::Release);
    }

    /// The sample rate of the last activation (0 before the first).
    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::Relaxed))
    }

    /// How long the output goes on after the input stops, samples: the reverb's decay
    /// (to 90 dB down, half again its 60 dB time), infinite while frozen; the latency for
    /// the others. `u32::MAX` is infinite in both standards.
    pub fn tail_samples(&self) -> u32 {
        let latency = self.latency();
        if self.table.kind != cantus_core::EffectKind::Space {
            return latency;
        }
        let get = |key: &str| {
            self.table
                .by_key(key)
                .map(|p| self.value(p.index))
                .unwrap_or(0.0)
        };
        if get("freeze") >= 0.5 {
            return u32::MAX;
        }
        let seconds = get("decay") * 1.5 + get("predelay") / 1_000.0 + 0.05;
        let samples = (seconds * self.sample_rate()) as u64 + latency as u64;
        samples.min(i32::MAX as u64 - 1) as u32
    }

    pub fn value(&self, index: usize) -> f32 {
        f32::from_bits(self.values[index].load(Ordering::Relaxed))
    }

    pub fn values(&self) -> Vec<f32> {
        (0..self.values.len()).map(|i| self.value(i)).collect()
    }

    /// A value set on the main thread: clamped, stored, and taken by the audio thread at
    /// its next grid point. Returns what was stored.
    pub fn set_value(&self, index: usize, value: f64) -> f32 {
        let v = self.table.params[index].clamp(value);
        self.values[index].store(v.to_bits(), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
        v
    }

    /// A value the host already holds (VST3's controller echo): stored for display only;
    /// the audio thread gets it from the host's own stream of changes.
    pub fn show_value(&self, index: usize, value: f64) -> f32 {
        let v = self.table.params[index].clamp(value);
        self.values[index].store(v.to_bits(), Ordering::Relaxed);
        v
    }

    /// Replace every value (a state load), as one change.
    pub fn set_all(&self, values: &[f32]) {
        for (i, v) in values.iter().enumerate().take(self.values.len()) {
            self.values[i].store(v.to_bits(), Ordering::Relaxed);
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// The state to save: `EffectSpec` JSON (`crate::state`).
    pub fn save_state(&self) -> Vec<u8> {
        crate::state::save(self.table, &self.values())
    }

    pub fn load_state(&self, bytes: &[u8]) -> Result<(), String> {
        let values = crate::state::load(self.table, bytes)?;
        self.set_all(&values);
        Ok(())
    }

    /// The latency the effect reports now, samples.
    pub fn latency(&self) -> u32 {
        self.latency.load(Ordering::Acquire)
    }

    /// Whether the latency changed since the last call (and clear it).
    pub fn take_latency_change(&self) -> bool {
        self.latency_changed.swap(false, Ordering::AcqRel)
    }

    /// The last telemetry the audio thread published (zeros before the first block).
    pub fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        for (o, a) in t.iter_mut().zip(&self.telemetry) {
            *o = f32::from_bits(a.load(Ordering::Relaxed));
        }
        t
    }

    /// The spec the current values make — what a DAW slot would hold.
    pub fn spec(&self) -> EffectSpec {
        spec_of(self.table, &self.values())
    }
}

/// An `EffectSpec` with every one of the table's keys present, so that later changes are
/// writes into existing entries and never an insertion (which would allocate).
fn spec_of(table: &Table, values: &[f32]) -> EffectSpec {
    let params: BTreeMap<String, f32> = table.params[..table.spec_count]
        .iter()
        .map(|p| (p.key.to_string(), values[p.index]))
        .collect();
    EffectSpec {
        kind: table.kind,
        params,
        // Our own effects are native kinds, never a hosted plug-in.
        external: None,
    }
}

/// The soft bypass: the input delayed by the effect's latency, cross-faded in, so a
/// bypass changes neither the timing nor the host's compensation (ADR 0018 wanted this
/// for the DAW too). The effect keeps running underneath, so un-bypassing does not click.
#[derive(Debug)]
struct Bypass {
    ring: [Box<[f32]>; 2],
    mask: usize,
    write: usize,
    /// 0 = the effect, 1 = the dry signal.
    gain: f32,
    target: f32,
    step: f32,
}

impl Bypass {
    fn new(sample_rate: f32) -> Bypass {
        let len = ((sample_rate * BYPASS_MAX_DELAY_S) as usize + MAX_CHUNK).next_power_of_two();
        Bypass {
            ring: [
                vec![0.0; len].into_boxed_slice(),
                vec![0.0; len].into_boxed_slice(),
            ],
            mask: len - 1,
            write: 0,
            gain: 0.0,
            target: 0.0,
            step: 1.0 / (BYPASS_FADE_S * sample_rate).max(1.0),
        }
    }

    /// The longest delay it can keep.
    fn max_delay(&self) -> usize {
        self.mask + 1 - MAX_CHUNK
    }

    /// Keep the input (before the effect touches it).
    fn push(&mut self, l: &[f32], r: &[f32]) {
        let n = l.len();
        let at = self.write;
        let first = n.min(self.mask + 1 - at);
        for (ch, src) in [l, r].into_iter().enumerate() {
            self.ring[ch][at..at + first].copy_from_slice(&src[..first]);
            self.ring[ch][..n - first].copy_from_slice(&src[first..n]);
        }
        self.write = (at + n) & self.mask;
    }

    fn active(&self) -> bool {
        self.gain > 0.0 || self.target > 0.0
    }

    /// Mix the delayed input into the effect's output (after [`Bypass::push`] of the
    /// same samples).
    fn mix(&mut self, l: &mut [f32], r: &mut [f32], delay: usize) {
        let n = l.len();
        let delay = delay.min(self.max_delay());
        let start = self.write.wrapping_sub(n).wrapping_sub(delay) & self.mask;
        for i in 0..n {
            if self.gain != self.target {
                self.gain = if self.target > self.gain {
                    (self.gain + self.step).min(self.target)
                } else {
                    (self.gain - self.step).max(self.target)
                };
            }
            let at = (start + i) & self.mask;
            let g = self.gain;
            if g >= 1.0 {
                l[i] = self.ring[0][at];
                r[i] = self.ring[1][at];
            } else {
                l[i] += (self.ring[0][at] - l[i]) * g;
                r[i] += (self.ring[1][at] - r[i]) * g;
            }
        }
    }
}

/// The audio thread's half of an instance.
#[derive(Debug)]
pub struct Processor {
    shared: Arc<Shared>,
    unit: EffectUnit,
    spec: EffectSpec,
    /// The values the host set (plain), one per parameter.
    base: Box<[f32]>,
    /// CLAP modulation on top of `base`, one per parameter.
    modulation: Box<[f32]>,
    /// `base` changed since the unit was last configured.
    dirty: bool,
    /// Samples since the stream started: the DSP's grid is counted from here.
    pos: u64,
    seen_generation: u32,
    latency: usize,
    sidechain: bool,
    bypass: Bypass,
    /// The analyser tap, lent by [`Shared`] while this processor lives.
    pairs: Option<Producer<(f32, f32)>>,
    /// The host's transport at the start of the current block, when it gives one.
    transport: Option<HostTransport>,
}

impl Drop for Processor {
    fn drop(&mut self) {
        // Main thread (deactivation): the tap's producer goes back for the next one.
        if let (Some(tx), Ok(mut p)) = (self.pairs.take(), self.shared.pairs.lock()) {
            p.0 = Some(tx);
        }
    }
}

impl Processor {
    /// Build the effect for a sample rate. Main thread: this is where memory is taken.
    pub fn new(shared: Arc<Shared>, sample_rate: f32) -> Processor {
        let table = shared.table;
        let seen_generation = shared.generation.load(Ordering::Acquire);
        let base: Box<[f32]> = shared.values().into_boxed_slice();
        let spec = spec_of(table, &base);
        let slot = EffectSlot {
            id: EffectId(0),
            bypass: false,
            spec: spec.clone(),
            mix: 1.0,
            sidechain: None,
            midi_from: None,
        };
        let mut unit = EffectUnit::new(&slot, sample_rate);
        // Inside Cantus, an EQ's linear-phase buffers arrive from the UI thread when its
        // mode asks for them (ADR 0022). A plug-in has no such thread to answer on the
        // audio thread's behalf, and a host may automate the mode at any sample: so the
        // EQ is given room for the largest resolution here, once, and every switch after
        // is real-time safe (about 4 MB per instance, as the DAW's EQ held before).
        if unit.kind == cantus_core::EffectKind::Eq {
            let most =
                cantus_engine::MemoryNeed::EqLinear(cantus_dsp::aequalis::Resolution::VeryHigh);
            drop(unit.supply(cantus_engine::EffectMemory::build(most, sample_rate)));
        }
        shared
            .sample_rate
            .store(sample_rate.to_bits(), Ordering::Relaxed);
        let latency = unit.latency();
        shared.latency.store(latency as u32, Ordering::Release);
        let mut bypass = Bypass::new(sample_rate);
        bypass.target = if base[table.bypass] >= 0.5 { 1.0 } else { 0.0 };
        bypass.gain = bypass.target;
        let pairs = shared.pairs.lock().ok().and_then(|mut p| p.0.take());
        Processor {
            pairs,
            transport: None,
            modulation: vec![0.0; base.len()].into_boxed_slice(),
            sidechain: base[table.sidechain] >= 0.5,
            shared,
            unit,
            spec,
            base,
            dirty: false,
            pos: 0,
            seen_generation,
            latency,
            bypass,
        }
    }

    /// The host's transport at the start of the next block (`None`: the host gave none,
    /// and the units keep their own time). Audio thread; no allocation.
    pub fn set_transport(&mut self, t: Option<HostTransport>) {
        self.transport = t;
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Samples of delay the effect adds now.
    pub fn latency(&self) -> usize {
        self.latency
    }

    /// Back to the start of a stream: silence in every line, the grid at zero.
    pub fn reset(&mut self) {
        self.unit.reset();
        self.pos = 0;
        for ch in &mut self.bypass.ring {
            ch.fill(0.0);
        }
        self.bypass.gain = self.bypass.target;
    }

    /// Take values now, outside a block (CLAP's `flush` between blocks).
    pub fn flush(&mut self, events: &[ParamEvent]) {
        self.pull_main();
        for ev in events {
            self.apply(ev);
        }
        if self.dirty {
            self.configure();
        }
    }

    fn apply(&mut self, ev: &ParamEvent) {
        let i = ev.index as usize;
        if i >= self.base.len() {
            return;
        }
        let p = &self.shared.table.params[i];
        if ev.modulation {
            self.modulation[i] = if ev.value.is_finite() { ev.value } else { 0.0 };
        } else {
            let v = p.clamp(ev.value as f64);
            self.base[i] = v;
            self.shared.values[i].store(v.to_bits(), Ordering::Relaxed);
        }
        self.dirty = true;
    }

    /// Take everything the main thread changed since the last look.
    fn pull_main(&mut self) {
        let g = self.shared.generation.load(Ordering::Acquire);
        if g != self.seen_generation {
            self.seen_generation = g;
            for (i, b) in self.base.iter_mut().enumerate() {
                *b = self.shared.value(i);
            }
            self.dirty = true;
            // A state (or values) set before the first sample is where the effect
            // *starts*, as the DAW's insert starts on its settings: configured, then reset,
            // so nothing glides from the defaults (a reverb's pre-delay sweeping from
            // 20 ms to 12 ms at the top of the song). Hosts commonly load state after
            // activating; this is what makes that the same as loading it before.
            if self.pos == 0 {
                self.configure();
                self.unit.reset();
                self.bypass.gain = self.bypass.target;
            }
        }
    }

    /// Hand the current values to the unit. Allocation-free: every key already exists
    /// in `spec`, and the table knows their order.
    fn configure(&mut self) {
        let table = self.shared.table;
        for (slot, &i) in self.spec.params.values_mut().zip(&table.spec_order) {
            let m = self.modulation[i];
            *slot = if m == 0.0 {
                self.base[i]
            } else {
                // A modulation is in the standard's units: through the knob's position
                // for a parameter CLAP sees that way.
                let p = &table.params[i];
                p.from_clap(p.to_clap(self.base[i]) + m as f64)
            };
        }
        self.unit.configure(&self.spec);
        self.sidechain = self.base[table.sidechain] >= 0.5;
        self.bypass.target = if self.base[table.bypass] >= 0.5 {
            1.0
        } else {
            0.0
        };
        let latency = self.unit.latency();
        if latency != self.latency {
            self.latency = latency;
            self.shared.latency.store(latency as u32, Ordering::Release);
            self.shared.latency_changed.store(true, Ordering::Release);
        }
        self.dirty = false;
    }

    /// Process one block in place. `events` are sorted by offset; each takes effect at
    /// the first grid point at or after it (see the module).
    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        events: &[ParamEvent],
    ) {
        self.process_with_notes(left, right, key, events, &[]);
    }

    /// [`Self::process`] with the host's notes (sorted by offset), each handed to the unit
    /// at its own sample — as the engine hands a slot its track's notes (ADR 0022).
    pub fn process_with_notes(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        events: &[ParamEvent],
        notes: &[NoteIn],
    ) {
        let n = left.len().min(right.len());
        let mut next_note = 0usize;
        let key = key.filter(|(a, b)| a.len() >= n && b.len() >= n);
        self.pull_main();
        let block_start = self.pos;
        let mut s = 0usize;
        let mut next = 0usize;
        while s < n {
            if self.pos.is_multiple_of(GRID) {
                while next < events.len() && events[next].offset as usize <= s {
                    self.apply(&events[next]);
                    next += 1;
                }
                if self.dirty {
                    self.configure();
                }
            }
            let to_grid = (GRID - self.pos % GRID) as usize;
            let end = if self.dirty {
                s + to_grid
            } else if let Some(ev) = events.get(next) {
                let e = ev.offset as usize;
                if e <= s {
                    s + to_grid
                } else {
                    let at = (block_start + e as u64).div_ceil(GRID) * GRID;
                    (at - block_start) as usize
                }
            } else {
                n
            };
            let end = end.min(n).min(s + MAX_CHUNK);
            if let Some(t) = &self.transport {
                let sr = f64::from(self.shared.sample_rate());
                let p = t.position(s, sr, block_start + s as u64);
                self.unit.set_position(&p);
            }
            // The notes in this piece, at their offsets in it (a note past the block's
            // end, which a host should not send, plays at its last sample).
            while let Some(nt) = notes.get(next_note) {
                let at = (nt.offset as usize).min(n - 1);
                if at >= end {
                    break;
                }
                self.unit.note(at.saturating_sub(s), nt.note);
                next_note += 1;
            }
            let k = key.map(|(a, b)| (&a[s..end], &b[s..end]));
            self.run(&mut left[s..end], &mut right[s..end], k);
            self.pos += (end - s) as u64;
            s = end;
        }
        // Changes later than the block's last grid point: in force at the next one.
        for ev in &events[next..] {
            self.apply(ev);
        }
        // Notes of a call with no audio: at the start of the next block.
        for nt in &notes[next_note..] {
            self.unit.note(0, nt.note);
        }
        if let Some(t) = &self.transport {
            self.shared.publish_transport(t, n);
        }
        if self.shared.editor_open.load(Ordering::Relaxed) {
            self.publish_telemetry();
        }
    }

    fn run(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let key = if self.sidechain { key } else { None };
        let tap = self.shared.editor_open.load(Ordering::Relaxed);
        // The input is kept only where something will read it: the bypass (which needs
        // the last `latency` samples of it, or this block's while it fades) and the tap.
        if self.latency > 0 || self.bypass.active() || tap {
            self.bypass.push(l, r);
        }
        self.unit.process_keyed(l, r, key);
        if self.bypass.active() {
            self.bypass.mix(l, r, self.latency);
        }
        if tap {
            self.tap(l, r);
        }
    }

    /// (input, output) mono pairs for an open editor's analyser, as the engine's tap
    /// makes them; a full ring drops the rest of the block (it is a picture).
    fn tap(&mut self, l: &[f32], r: &[f32]) {
        let Some(tx) = self.pairs.as_mut() else {
            return;
        };
        let n = l.len();
        // Speculum is a pass-through and measures stereo (ADR 0056): its pairs are the
        // frames themselves, left and right, every one of them.
        if self.shared.desc.kind == cantus_core::EffectKind::Analyzer {
            for i in 0..n {
                if tx.slots() == 0 {
                    self.shared
                        .scope_lost
                        .fetch_add((n - i) as u64, Ordering::Relaxed);
                    break;
                }
                let _ = tx.push((l[i], r[i]));
            }
            return;
        }
        let b = &self.bypass;
        let start = b.write.wrapping_sub(n) & b.mask;
        for i in 0..n {
            if tx.slots() == 0 {
                break;
            }
            let at = (start + i) & b.mask;
            let x = (b.ring[0][at] + b.ring[1][at]) * 0.5;
            let _ = tx.push((x, (l[i] + r[i]) * 0.5));
        }
    }

    fn publish_telemetry(&self) {
        if let Some(t) = self.unit.telemetry() {
            for (a, v) in self.shared.telemetry.iter().zip(t) {
                a.store(v.to_bits(), Ordering::Relaxed);
            }
            self.shared.telemetry_serial.fetch_add(1, Ordering::Release);
        }
    }
}

/// A fresh instance of a compiled plug-in, by CLAP id (tests and the format layers).
pub fn shared_for(clap_id: &str) -> Option<Arc<Shared>> {
    desc::by_clap_id(clap_id).map(Shared::new)
}
