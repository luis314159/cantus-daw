//! Hardware MIDI input (midir). Each port's callback runs on its own OS thread and pushes
//! into one shared ring buffer that the audio thread drains at the start of every block.
//!
//! **The controller tap** (round 29, ADR 0062): each callback also tees what the interface
//! needs — the messages the controller map takes, and every message while a control is
//! being learnt — into a ring of its own port (single producer, single consumer: no lock),
//! with the port and the moment it came, and wakes the interface. A message the map takes
//! is **consumed**: it never reaches the engine's ring, so a knob mapped to a filter no
//! longer bends the instrument and a mapped pad no longer plays a note. Which messages the
//! map takes is a lock-free bit set ([`MidiFilter`]) the interface publishes. Nothing here
//! runs on the audio thread.

use crate::messages::MidiEvent;
use cantus_core::controller::Control;
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Cloneable sender for MIDI threads. The mutex is only ever contended between MIDI
/// callbacks, never touched by the audio thread.
#[derive(Clone)]
pub struct MidiSender {
    inner: Arc<Mutex<Producer<MidiEvent>>>,
    /// Events sent since the sender was made: the top bar's MIDI-input lamp (ADR 0052).
    sent: Arc<AtomicU64>,
}

impl std::fmt::Debug for MidiSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MidiSender")
    }
}

impl MidiSender {
    pub fn new(producer: Producer<MidiEvent>) -> Self {
        MidiSender {
            inner: Arc::new(Mutex::new(producer)),
            sent: Arc::default(),
        }
    }

    /// How many events have come in through this sender and its clones, ever: what the top
    /// bar's MIDI lamp watches. Off the audio thread; never reset.
    pub fn received(&self) -> u64 {
        self.sent.load(Ordering::Relaxed)
    }

    /// Count an event that came in but went to the controller map only: the lamp lights
    /// for a mapped knob too.
    pub fn count(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns `false` if the queue was full (the event is dropped).
    pub fn send(&self, ev: MidiEvent) -> bool {
        self.count();
        match self.inner.lock() {
            Ok(mut p) => p.push(ev).is_ok(),
            Err(_) => false,
        }
    }
}

/// Parse a raw MIDI message into the subset the engine understands.
pub fn parse(bytes: &[u8]) -> Option<MidiEvent> {
    let status = *bytes.first()?;
    match status & 0xF0 {
        0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => Some(MidiEvent {
            status,
            data1: *bytes.get(1)?,
            data2: *bytes.get(2)?,
        }),
        0xC0 | 0xD0 => Some(MidiEvent {
            status,
            data1: *bytes.get(1)?,
            data2: 0,
        }),
        _ => None, // system / realtime messages
    }
}

// ---- the controller tap (round 29, ADR 0062) ----------------------------------------------

/// The most ports the filter tells apart (more are heard, never consumed).
pub const FILTER_PORTS: usize = 32;
/// Bits per port and channel: 128 controllers, 128 notes, 128 key pressures, 128 programs,
/// the pitch wheel and channel pressure.
const SLOTS: usize = 514;
const WORDS: usize = SLOTS.div_ceil(64);
/// How many messages each port's tap ring holds for the interface (it drains every frame).
pub const TAP_QUEUE: usize = 1024;

/// A message the interface hears from the tap.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Heard {
    /// The port's index among the ports open now ([`MidiInputs::names`]).
    pub port: u8,
    pub event: MidiEvent,
    /// When it came (the callback's clock): a mapped move is recorded at its own time.
    pub at: Instant,
}

/// The bit of a message's address in one port-and-channel's row.
fn slot_of(ev: &MidiEvent) -> Option<usize> {
    let n = usize::from(ev.data1 & 0x7F);
    Some(match ev.status & 0xF0 {
        0xB0 => n,
        0x80 | 0x90 => 128 + n,
        0xA0 => 256 + n,
        0xC0 => 384 + n,
        0xE0 => 512,
        0xD0 => 513,
        _ => return None,
    })
}

/// The bits a control takes in its row.
fn slots_of(control: Control) -> ([usize; 2], usize) {
    match control {
        Control::Cc(n) => ([usize::from(n & 0x7F), 0], 1),
        Control::Cc14(n) => {
            let n = usize::from(n & 31);
            ([n, n + 32], 2)
        }
        Control::Note(n) => ([128 + usize::from(n & 0x7F), 0], 1),
        Control::PolyPressure(n) => ([256 + usize::from(n & 0x7F), 0], 1),
        Control::Program(n) => ([384 + usize::from(n & 0x7F), 0], 1),
        Control::PitchBend => ([512, 0], 1),
        Control::ChannelPressure => ([513, 0], 1),
    }
}

/// Which messages the controller map takes, as the interface wants it: plain bits, built
/// on the interface thread and [published](MidiFilter::publish) whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterSet {
    words: Vec<u64>,
}

impl Default for FilterSet {
    fn default() -> Self {
        FilterSet {
            words: vec![0; FILTER_PORTS * 16 * WORDS],
        }
    }
}

impl FilterSet {
    /// Take `control` on port `port` (its index now), on `channel` or every channel.
    pub fn take(&mut self, port: usize, channel: Option<u8>, control: Control) {
        if port >= FILTER_PORTS {
            return;
        }
        let channels = match channel {
            Some(c) if c < 16 => c..c + 1,
            Some(_) => return,
            None => 0..16,
        };
        let (slots, n) = slots_of(control);
        for c in channels {
            for s in &slots[..n] {
                let bit = (port * 16 + usize::from(c)) * WORDS * 64 + s;
                self.words[bit / 64] |= 1 << (bit % 64);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }
}

/// The lock-free set the callbacks read: atomics written by the interface, read by the MIDI
/// threads with a relaxed load (a message racing an edit of the map goes one way or the
/// other, and the next is right).
#[derive(Debug)]
pub struct MidiFilter {
    words: Box<[AtomicU64]>,
    /// A control is being learnt: every message is heard, and none reaches the engine (the
    /// knob the user turns to teach Cantus must not bend the instrument meanwhile).
    learning: AtomicBool,
}

impl Default for MidiFilter {
    fn default() -> Self {
        MidiFilter {
            words: (0..FILTER_PORTS * 16 * WORDS)
                .map(|_| AtomicU64::new(0))
                .collect(),
            learning: AtomicBool::new(false),
        }
    }
}

impl MidiFilter {
    /// Whether the map takes `ev` from port `port`.
    pub fn takes(&self, port: usize, ev: &MidiEvent) -> bool {
        if port >= FILTER_PORTS {
            return false;
        }
        let Some(s) = slot_of(ev) else {
            return false;
        };
        let bit = (port * 16 + usize::from(ev.status & 0x0F)) * WORDS * 64 + s;
        self.words[bit / 64].load(Ordering::Relaxed) & (1 << (bit % 64)) != 0
    }

    /// Make `set` the messages taken.
    pub fn publish(&self, set: &FilterSet) {
        for (w, v) in self.words.iter().zip(&set.words) {
            w.store(*v, Ordering::Relaxed);
        }
    }

    /// Learning on or off. Released: the words published before it are seen by a callback
    /// that sees learning end — a control just learnt is taken from its first message on
    /// (review 2, #14).
    pub fn set_learning(&self, on: bool) {
        self.learning.store(on, Ordering::Release);
    }

    pub fn learning(&self) -> bool {
        self.learning.load(Ordering::Acquire)
    }
}

type Wake = Arc<dyn Fn() + Send + Sync>;

struct TapShared {
    filter: MidiFilter,
    /// The thread that wakes the interface (set at boot: egui's repaint). The callbacks
    /// only unpark it — egui's repaint takes the context's lock, which the interface holds
    /// for a whole frame's tessellation, and a MIDI callback must never wait on that
    /// (review 1, #13).
    waker: Option<std::thread::Thread>,
    /// Tells the waker thread to end (the tap dropped).
    stop: Arc<AtomicBool>,
    /// A wake is on its way and the interface has not drained yet: no other is sent.
    pending: AtomicBool,
    /// What a full ring could not take, the newest per address (review 2, #16): a stalled
    /// interface still gets a control's final value and a pad's release.
    overflow: Box<[AtomicU32]>,
    /// Each slot's arrival: the order among them (`seq`) and the time since `epoch`, so
    /// the table hands them over in the order and at the times they came (review 3, #11,
    /// #22).
    overflow_seq: Box<[AtomicU64]>,
    overflow_at: Box<[AtomicU64]>,
    seq: AtomicU64,
    epoch: Instant,
    overflowed: AtomicBool,
}

/// Slots of the overflow table (an address hashes to one; a collision keeps the newest).
const OVERFLOW_SLOTS: usize = 1024;

fn overflow_slot(port: usize, ev: &MidiEvent) -> usize {
    // Notes on and off share their address (a release replaces its press); the pitch
    // wheel and channel pressure have one per channel — their first byte is their value
    // (review 3, #10).
    let kind = match ev.status & 0xF0 {
        0x80 => 0x90,
        k => k,
    };
    let number = match kind {
        0xD0 | 0xE0 => 0,
        _ => u32::from(ev.data1),
    };
    let key = (port as u32) << 16 | u32::from(kind | (ev.status & 0x0F)) << 8 | number;
    (key.wrapping_mul(0x9E37_79B1) >> 22) as usize % OVERFLOW_SLOTS
}

impl Drop for TapShared {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = &self.waker {
            t.unpark();
        }
    }
}

/// The controller tap's shared half: the filter and the interface's wake. Cloned into
/// every port's callback; it survives a rescan and an audio restart (the app keeps it and
/// opens the new ports with it).
#[derive(Clone)]
pub struct MidiTap {
    shared: Arc<TapShared>,
}

impl std::fmt::Debug for MidiTap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiTap")
            .field("wake", &self.shared.waker.is_some())
            .finish()
    }
}

impl Default for MidiTap {
    fn default() -> Self {
        MidiTap::new(None)
    }
}

impl MidiTap {
    pub fn new(wake: Option<Wake>) -> MidiTap {
        let stop = Arc::new(AtomicBool::new(false));
        let waker = wake.and_then(|w| {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("cantus-midi-waker".into())
                .spawn(move || loop {
                    std::thread::park();
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    w();
                })
                .ok()
                .map(|h| h.thread().clone())
        });
        MidiTap {
            shared: Arc::new(TapShared {
                filter: MidiFilter::default(),
                waker,
                stop,
                pending: AtomicBool::new(false),
                overflow: (0..OVERFLOW_SLOTS).map(|_| AtomicU32::new(0)).collect(),
                overflow_seq: (0..OVERFLOW_SLOTS).map(|_| AtomicU64::new(0)).collect(),
                overflow_at: (0..OVERFLOW_SLOTS).map(|_| AtomicU64::new(0)).collect(),
                seq: AtomicU64::new(0),
                epoch: Instant::now(),
                overflowed: AtomicBool::new(false),
            }),
        }
    }

    pub fn filter(&self) -> &MidiFilter {
        &self.shared.filter
    }

    /// Whether something wakes the interface (the boot path sets it).
    pub fn wakes(&self) -> bool {
        self.shared.waker.is_some()
    }

    /// Whether `other` is this tap (a clone of it).
    pub fn same(&self, other: &MidiTap) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    /// Wake the interface, once until it drains.
    /// Lock-free: a flag and an unpark (no lock the interface holds, no allocation).
    pub(crate) fn wake(&self) {
        if !self.shared.pending.swap(true, Ordering::AcqRel) {
            if let Some(t) = &self.shared.waker {
                t.unpark();
            }
        }
    }

    /// The interface is about to drain the rings: the next message wakes it again.
    /// A swap, not a store (review 2, #13): ordered with the callback's own swap, either the
    /// callback sees the rearm and wakes, or this sees its push. Returns whether a wake was
    /// pending.
    pub fn rearm(&self) -> bool {
        self.shared.pending.swap(false, Ordering::AcqRel)
    }

    /// Whether messages are parked: then what comes next is parked too, never handed over
    /// before what came first (review 3, #11).
    fn parking(&self) -> bool {
        self.shared.overflowed.load(Ordering::Acquire)
    }

    /// Keep `ev` from `port`, which a full ring could not take: the newest per address,
    /// with its arrival's order and time.
    fn overflow(&self, port: usize, ev: &MidiEvent, at: Instant) {
        let packed = 1 << 31
            | (port.min(127) as u32) << 24
            | u32::from(ev.status) << 16
            | u32::from(ev.data1 & 0x7F) << 8
            | u32::from(ev.data2 & 0x7F);
        let i = overflow_slot(port, ev);
        let sh = &self.shared;
        let seq = sh.seq.fetch_add(1, Ordering::Relaxed);
        let nanos = at.saturating_duration_since(sh.epoch).as_nanos() as u64;
        sh.overflow_seq[i].store(seq, Ordering::Relaxed);
        sh.overflow_at[i].store(nanos, Ordering::Relaxed);
        sh.overflow[i].store(packed, Ordering::Release);
        sh.overflowed.store(true, Ordering::Release);
    }

    /// What full rings could not take since the last call, the newest per address, in the
    /// order and at the times they came (after the rings' messages, which are older).
    /// Interface thread.
    pub fn take_overflow(&self, out: &mut Vec<Heard>) {
        if !self.shared.overflowed.swap(false, Ordering::AcqRel) {
            return;
        }
        let sh = &self.shared;
        let mut taken: Vec<(u64, Heard)> = Vec::new();
        for (i, slot) in sh.overflow.iter().enumerate() {
            let v = slot.swap(0, Ordering::AcqRel);
            if v & 1 << 31 == 0 {
                continue;
            }
            let at = sh.epoch
                + std::time::Duration::from_nanos(sh.overflow_at[i].load(Ordering::Relaxed));
            taken.push((
                sh.overflow_seq[i].load(Ordering::Relaxed),
                Heard {
                    port: (v >> 24 & 0x7F) as u8,
                    event: MidiEvent {
                        status: (v >> 16) as u8,
                        data1: (v >> 8 & 0x7F) as u8,
                        data2: (v & 0x7F) as u8,
                    },
                    at,
                },
            ));
        }
        taken.sort_by_key(|(seq, _)| *seq);
        out.extend(taken.into_iter().map(|(_, h)| h));
    }
}

/// What the engine holds from one port: the keys it got a note-on for and has not been
/// released, and whether its sustain pedal is down, per channel. Kept by the port's
/// callback (one thread: no lock), so a release is never swallowed — learning starts, or a
/// mapping is published, while a key or the pedal is held (review 1, #12).
#[derive(Clone, Debug, Default)]
pub struct PortState {
    notes: [[u64; 2]; 16],
    /// The sustain (64), sostenuto (66) and soft (67) pedals anywhere but up, per channel
    /// (a half pedal holds a continuous piano half damped).
    pedal: [[bool; 3]; 16],
}

/// The pedal a controller number is: 0 sustain, 1 sostenuto, 2 soft (Chorda's una
/// corda, review 3, #13).
fn pedal_of(cc: u8) -> Option<usize> {
    match cc {
        64 => Some(0),
        66 => Some(1),
        67 => Some(2),
        _ => None,
    }
}

impl PortState {
    fn bit(note: u8) -> (usize, u64) {
        let n = note & 0x7F;
        (usize::from(n / 64), 1 << (n % 64))
    }

    /// Whether the engine holds `note` on `channel`.
    pub fn holds(&self, channel: u8, note: u8) -> bool {
        let (w, b) = Self::bit(note);
        self.notes[usize::from(channel & 15)][w] & b != 0
    }

    /// What the engine got: a key down or up, the pedal down or up.
    fn sent(&mut self, ev: &MidiEvent) {
        let ch = usize::from(ev.status & 15);
        let (w, b) = Self::bit(ev.data1);
        match ev.status & 0xF0 {
            0x90 if ev.data2 > 0 => self.notes[ch][w] |= b,
            0x80 | 0x90 => self.notes[ch][w] &= !b,
            0xB0 => match (pedal_of(ev.data1), ev.data1) {
                (Some(p), _) => self.pedal[ch][p] = ev.data2 > 0,
                // All sound off, all notes off: nothing held any more. (Reset all
                // controllers is not: the native instruments ignore it, review 3, #12.)
                (None, 120 | 123) => self.notes[ch] = [0; 2],
                _ => {}
            },
            _ => {}
        }
    }

    /// Whether `ev` releases something the engine holds (a key, the pedal).
    fn releases(&self, ev: &MidiEvent) -> bool {
        let ch = usize::from(ev.status & 15);
        match ev.status & 0xF0 {
            0x80 => self.holds(ev.status & 15, ev.data1),
            0x90 if ev.data2 == 0 => self.holds(ev.status & 15, ev.data1),
            // Any taken message of a pedal the engine holds (fully or half down): the engine
            // gets it fully up, once.
            0xB0 => pedal_of(ev.data1).is_some_and(|p| self.pedal[ch][p]),
            _ => false,
        }
    }

    /// The message the engine gets for `ev`, a release of what it holds: a pedal fully up
    /// (a continuous pedal's first value below half would leave a half-pedalling piano
    /// half damped, review 2, #15); a key's release as it is.
    fn release_of(ev: MidiEvent) -> MidiEvent {
        match ev.status & 0xF0 {
            0xB0 => MidiEvent { data2: 0, ..ev },
            _ => ev,
        }
    }
}

/// One message from port `port`: teed to the interface's ring when the map takes it or a
/// control is being learnt, and sent to the engine unless the map took it — but a release
/// of a key or of the pedal the engine holds always reaches it (`state`). Returns whether
/// the engine got it. What every port's callback runs (and what the tests drive).
/// Lock-free and allocation-free but for the engine's own ring (the sender's mutex,
/// contended only by the other ports' callbacks).
pub fn route(
    ev: MidiEvent,
    port: usize,
    tap: &MidiTap,
    engine: &MidiSender,
    ui: &mut Producer<Heard>,
    state: &mut PortState,
) -> bool {
    let consumed = tap.filter().learning() || tap.filter().takes(port, &ev);
    if consumed {
        // A full ring (an interface stalled) keeps the newest per address aside; while
        // anything is parked, what follows is parked behind it (review 3, #11).
        let at = Instant::now();
        let parked = tap.parking()
            || ui
                .push(Heard {
                    port: port.min(u8::MAX as usize) as u8,
                    event: ev,
                    at,
                })
                .is_err();
        if parked {
            tap.overflow(port, &ev, at);
        }
        tap.wake();
        if state.releases(&ev) {
            let release = PortState::release_of(ev);
            state.sent(&release);
            return engine.send(release);
        }
        engine.count();
        false
    } else {
        state.sent(&ev);
        engine.send(ev)
    }
}

pub struct MidiInputs {
    _connections: Vec<MidiInputConnection<PortState>>,
    pub names: Vec<String>,
    /// Each open port's tap ring, in the order of `names` (round 29, ADR 0062).
    taps: Vec<Consumer<Heard>>,
    tap: MidiTap,
}

impl std::fmt::Debug for MidiInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiInputs")
            .field("names", &self.names)
            .finish()
    }
}

/// Names of the MIDI input ports currently present.
pub fn list_ports() -> Vec<String> {
    let Ok(input) = MidiInput::new("cantus-scan") else {
        return Vec::new();
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .collect()
}

impl MidiInputs {
    /// Open every input port with a tap of its own (no wake, nothing consumed): what a
    /// caller that needs no controller map uses.
    pub fn open_all(sender: MidiSender) -> MidiInputs {
        MidiInputs::open_with(sender, MidiTap::default())
    }

    /// Open every input port, each teeing into `tap` (round 29, ADR 0062). Failures are
    /// logged and skipped: a broken driver must never stop the DAW from starting.
    pub fn open_with(sender: MidiSender, tap: MidiTap) -> MidiInputs {
        let mut connections = Vec::new();
        let mut names = Vec::new();
        let mut taps = Vec::new();
        let ports = match MidiInput::new("cantus-scan") {
            Ok(i) => i.ports(),
            Err(e) => {
                log::warn!("midi: cannot initialise input: {e}");
                return MidiInputs {
                    _connections: connections,
                    names,
                    taps,
                    tap,
                };
            }
        };
        for port in ports {
            let Ok(mut input) = MidiInput::new("cantus") else {
                continue;
            };
            input.ignore(Ignore::All);
            let name = input
                .port_name(&port)
                .unwrap_or_else(|_| "unknown".to_string());
            let tx = sender.clone();
            let (mut ui, heard) = RingBuffer::<Heard>::new(TAP_QUEUE);
            let index = connections.len();
            let tap2 = tap.clone();
            match input.connect(
                &port,
                "cantus-in",
                move |_stamp, bytes, held: &mut PortState| {
                    if let Some(ev) = parse(bytes) {
                        route(ev, index, &tap2, &tx, &mut ui, held);
                    }
                },
                PortState::default(),
            ) {
                Ok(c) => {
                    log::info!("midi: opened input {name}");
                    connections.push(c);
                    names.push(name);
                    taps.push(heard);
                }
                Err(e) => log::warn!("midi: cannot open {name}: {e}"),
            }
        }
        MidiInputs {
            _connections: connections,
            names,
            taps,
            tap,
        }
    }

    /// The tap the ports were opened with.
    pub fn tap(&self) -> &MidiTap {
        &self.tap
    }

    /// Everything the ports teed since the last call, port by port, each port's in order;
    /// then the tap may wake the interface again. Interface thread.
    pub fn drain(&mut self, out: &mut Vec<Heard>) {
        self.tap.rearm();
        for ring in &mut self.taps {
            while let Ok(h) = ring.pop() {
                out.push(h);
            }
        }
        self.tap.take_overflow(out);
    }
}

// ---- MIDI out, for controller feedback (round 29, ADR 0062) -------------------------------

/// Names of the MIDI output ports currently present.
pub fn list_outputs() -> Vec<String> {
    let Ok(out) = MidiOutput::new("cantus-scan") else {
        return Vec::new();
    };
    out.ports()
        .iter()
        .filter_map(|p| out.port_name(p).ok())
        .collect()
}

/// The output that belongs to the input called `input` (the same device's other half):
/// the same name; or, as Windows names them, `MIDIIN2 (X)` → `MIDIOUT2 (X)`; or the name
/// with its *in* word turned to *out*. `None` when no output looks like it.
pub fn output_for(input: &str, outputs: &[String]) -> Option<usize> {
    if let Some(i) = outputs.iter().position(|o| o == input) {
        return Some(i);
    }
    let swaps = [
        ("MIDIIN", "MIDIOUT"),
        ("MIDI IN", "MIDI OUT"),
        (" IN", " OUT"),
        (" In", " Out"),
        (" in", " out"),
        ("Input", "Output"),
        ("input", "output"),
    ];
    for (from, to) in swaps {
        if input.contains(from) {
            let want = input.replacen(from, to, 1);
            if let Some(i) = outputs.iter().position(|o| *o == want) {
                return Some(i);
            }
        }
    }
    None
}

/// Output ports opened for feedback, by the name of the input they answer. Opened on
/// demand (the first mapping with feedback on a port); interface thread.
pub struct MidiOutputs {
    open: Vec<(String, Box<dyn OutPort>)>,
    /// Inputs whose output could not be found or opened (not tried again until a rescan).
    missing: Vec<String>,
    /// Opens the output that answers an input (the system's; a test's own).
    connect: fn(&str) -> Option<Box<dyn OutPort>>,
}

impl Default for MidiOutputs {
    fn default() -> Self {
        MidiOutputs {
            open: Vec::new(),
            missing: Vec::new(),
            connect: |input| MidiOutputs::connect(input).map(|c| Box::new(c) as Box<dyn OutPort>),
        }
    }
}

/// An open output: what a message is written to.
pub trait OutPort: Send {
    /// Write `bytes`: whether it went out.
    fn write(&mut self, bytes: &[u8]) -> bool;
}

impl OutPort for MidiOutputConnection {
    fn write(&mut self, bytes: &[u8]) -> bool {
        self.send(bytes).is_ok()
    }
}

impl std::fmt::Debug for MidiOutputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiOutputs")
            .field(
                "open",
                &self.open.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            )
            .field("missing", &self.missing)
            .finish()
    }
}

impl MidiOutputs {
    /// Outputs opened by `connect` (a test's ports).
    pub fn with_connect(connect: fn(&str) -> Option<Box<dyn OutPort>>) -> Self {
        MidiOutputs {
            connect,
            ..MidiOutputs::default()
        }
    }

    /// Send `bytes` to the output that answers the input `input`, opening it the first
    /// time. Returns whether it went out.
    pub fn send(&mut self, input: &str, bytes: &[u8]) -> bool {
        self.send_at(input, bytes).is_some()
    }

    /// [`Self::send`], answering when the write began: after the output was opened (an
    /// open enumerates the ports and may take long), immediately before the bytes left —
    /// the moment a loopback's echo is measured from (review 4 of ADR 0062, #13).
    pub fn send_at(&mut self, input: &str, bytes: &[u8]) -> Option<std::time::Instant> {
        if !self.open.iter().any(|(n, _)| n == input) {
            if self.missing.iter().any(|n| n == input) {
                return None;
            }
            match (self.connect)(input) {
                Some(c) => self.open.push((input.to_string(), c)),
                None => {
                    self.missing.push(input.to_string());
                    return None;
                }
            }
        }
        let (_, c) = self.open.iter_mut().find(|(n, _)| n == input)?;
        let at = std::time::Instant::now();
        c.write(bytes).then_some(at)
    }

    fn connect(input: &str) -> Option<MidiOutputConnection> {
        let out = MidiOutput::new("cantus").ok()?;
        let ports = out.ports();
        let names: Vec<String> = ports
            .iter()
            .map(|p| out.port_name(p).unwrap_or_default())
            .collect();
        let i = output_for(input, &names)?;
        match out.connect(&ports[i], "cantus-out") {
            Ok(c) => {
                log::info!("midi: opened output {} for feedback", names[i]);
                Some(c)
            }
            Err(e) => {
                log::warn!("midi: cannot open output {}: {e}", names[i]);
                None
            }
        }
    }

    /// Close everything (a rescan, a restart): the next send opens again.
    pub fn reset(&mut self) {
        self.open.clear();
        self.missing.clear();
    }

    /// The inputs whose output is open now.
    pub fn open_names(&self) -> Vec<&str> {
        self.open.iter().map(|(n, _)| n.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A port that takes a while to open (a full enumeration) and keeps what it was given.
    struct SlowPort;

    impl OutPort for SlowPort {
        fn write(&mut self, _bytes: &[u8]) -> bool {
            true
        }
    }

    fn slow_connect(input: &str) -> Option<Box<dyn OutPort>> {
        std::thread::sleep(std::time::Duration::from_millis(40));
        (input == "Pads").then(|| Box::new(SlowPort) as Box<dyn OutPort>)
    }

    /// Review 4 of ADR 0062, #13: the moment a message left is after its output opened,
    /// not before — an echo measured from it is not late by the open's time.
    #[test]
    fn a_messages_moment_is_after_its_output_opened() {
        let mut out = MidiOutputs::with_connect(slow_connect);
        let before = std::time::Instant::now();
        let at = out.send_at("Pads", &[0x90, 36, 127]).expect("sent");
        assert!(
            at.duration_since(before) >= std::time::Duration::from_millis(40),
            "stamped before the open: {:?}",
            at.duration_since(before)
        );
        // Open now: the next one is stamped at once; a missing output answers nothing.
        let again = std::time::Instant::now();
        let at = out.send_at("Pads", &[0x80, 36, 0]).expect("sent");
        assert!(at.duration_since(again) < std::time::Duration::from_millis(20));
        assert_eq!(out.send_at("Keys", &[0x90, 1, 1]), None);
        assert!(!out.send("Keys", &[0x90, 1, 1]), "remembered as missing");
        assert_eq!(out.open_names(), vec!["Pads"]);
    }

    /// The top bar's MIDI lamp counts what comes in, through any clone of the sender.
    #[test]
    fn the_sender_counts_what_comes_in() {
        let (p, mut c) = rtrb::RingBuffer::<MidiEvent>::new(4);
        let a = MidiSender::new(p);
        let b = a.clone();
        assert_eq!(a.received(), 0);
        a.send(MidiEvent::note_off(60));
        b.send(MidiEvent::note_off(61));
        assert_eq!((a.received(), b.received()), (2, 2));
        assert!(c.pop().is_ok());
    }

    #[test]
    fn parses_channel_messages_and_ignores_system() {
        assert_eq!(
            parse(&[0x91, 60, 100]),
            Some(MidiEvent {
                status: 0x91,
                data1: 60,
                data2: 100
            })
        );
        assert_eq!(
            parse(&[0x80, 60, 0]),
            Some(MidiEvent::note_off(60)).map(|mut e| {
                e.status = 0x80;
                e
            })
        );
        assert_eq!(
            parse(&[0xC0, 5]),
            Some(MidiEvent {
                status: 0xC0,
                data1: 5,
                data2: 0
            })
        );
        assert_eq!(parse(&[0xF8]), None);
        assert_eq!(parse(&[0x90]), None);
        assert_eq!(parse(&[]), None);
    }

    fn ev(status: u8, data1: u8, data2: u8) -> MidiEvent {
        MidiEvent {
            status,
            data1,
            data2,
        }
    }

    /// The filter takes exactly the addresses published, per port and channel; a 14-bit
    /// pair takes both halves; every channel when none is named.
    #[test]
    fn the_filter_takes_what_the_map_names_and_nothing_else() {
        let f = MidiFilter::default();
        let mut set = FilterSet::default();
        assert!(set.is_empty());
        set.take(1, Some(2), Control::Cc(74));
        set.take(0, None, Control::Note(36));
        set.take(3, Some(0), Control::Cc14(7));
        set.take(2, Some(5), Control::PitchBend);
        set.take(2, Some(5), Control::Program(9));
        set.take(2, Some(5), Control::ChannelPressure);
        set.take(2, Some(5), Control::PolyPressure(60));
        f.publish(&set);
        assert!(f.takes(1, &ev(0xB2, 74, 10)));
        assert!(!f.takes(1, &ev(0xB3, 74, 10)), "another channel");
        assert!(!f.takes(0, &ev(0xB2, 74, 10)), "another port");
        assert!(!f.takes(1, &ev(0xB2, 75, 10)), "another controller");
        for ch in 0..16 {
            assert!(f.takes(0, &ev(0x90 | ch, 36, 100)));
            assert!(f.takes(0, &ev(0x80 | ch, 36, 0)), "its note-off too");
        }
        assert!(!f.takes(0, &ev(0x90, 37, 100)));
        assert!(f.takes(3, &ev(0xB0, 7, 1)) && f.takes(3, &ev(0xB0, 39, 1)));
        assert!(f.takes(2, &ev(0xE5, 0, 64)));
        assert!(f.takes(2, &ev(0xC5, 9, 0)) && !f.takes(2, &ev(0xC5, 8, 0)));
        assert!(f.takes(2, &ev(0xD5, 40, 0)));
        assert!(f.takes(2, &ev(0xA5, 60, 40)) && !f.takes(2, &ev(0xA5, 61, 40)));
        assert!(!f.takes(FILTER_PORTS, &ev(0xB2, 74, 10)));
        f.publish(&FilterSet::default());
        assert!(!f.takes(1, &ev(0xB2, 74, 10)));
    }

    /// A taken message goes to the interface's ring, not the engine's, and wakes the
    /// interface once until it drains; an untaken one goes to the engine only; while a
    /// control is learnt everything goes to the interface.
    #[test]
    fn route_tees_what_is_taken_and_wakes_once() {
        let wakes = Arc::new(AtomicU64::new(0));
        let w = wakes.clone();
        let tap = MidiTap::new(Some(Arc::new(move || {
            w.fetch_add(1, Ordering::Relaxed);
        })));
        assert!(tap.wakes());
        let mut set = FilterSet::default();
        set.take(0, Some(0), Control::Cc(74));
        tap.filter().publish(&set);
        let (p, mut engine) = RingBuffer::<MidiEvent>::new(16);
        let sender = MidiSender::new(p);
        let (mut ui, mut heard) = RingBuffer::<Heard>::new(16);
        let mut held = PortState::default();
        assert!(!route(
            ev(0xB0, 74, 3),
            0,
            &tap,
            &sender,
            &mut ui,
            &mut held
        ));
        assert!(!route(
            ev(0xB0, 74, 4),
            0,
            &tap,
            &sender,
            &mut ui,
            &mut held
        ));
        assert!(route(ev(0xB0, 1, 99), 0, &tap, &sender, &mut ui, &mut held));
        assert_eq!(engine.pop().unwrap(), ev(0xB0, 1, 99));
        assert!(engine.pop().is_err());
        assert_eq!(heard.pop().unwrap().event, ev(0xB0, 74, 3));
        assert_eq!(heard.pop().unwrap().event, ev(0xB0, 74, 4));
        assert!(heard.pop().is_err());
        settle(&wakes, 1);
        assert_eq!(wakes.load(Ordering::Relaxed), 1, "one wake until it drains");
        assert_eq!(sender.received(), 3, "the lamp counts the mapped ones too");
        tap.rearm();
        tap.filter().set_learning(true);
        assert!(!route(
            ev(0x90, 60, 100),
            0,
            &tap,
            &sender,
            &mut ui,
            &mut held
        ));
        assert_eq!(heard.pop().unwrap().event, ev(0x90, 60, 100));
        assert!(
            engine.pop().is_err(),
            "a learnt control does not play meanwhile"
        );
        settle(&wakes, 2);
        assert_eq!(wakes.load(Ordering::Relaxed), 2);
    }

    /// The waker thread runs the wake: wait for it (a second at most).
    fn settle(wakes: &AtomicU64, n: u64) {
        let t = Instant::now();
        while wakes.load(Ordering::Relaxed) < n && t.elapsed().as_secs() < 1 {
            std::thread::yield_now();
        }
    }

    /// Review 1, #12: a key or the pedal the engine holds is released in the engine even
    /// when learning starts, or a mapping takes the key, while it is held.
    #[test]
    fn a_held_key_or_pedal_is_released_whatever_the_filter_says() {
        let tap = MidiTap::default();
        let (p, mut engine) = RingBuffer::<MidiEvent>::new(16);
        let sender = MidiSender::new(p);
        let (mut ui, mut heard) = RingBuffer::<Heard>::new(16);
        let mut held = PortState::default();
        assert!(route(
            ev(0x90, 60, 100),
            0,
            &tap,
            &sender,
            &mut ui,
            &mut held
        ));
        assert!(route(
            ev(0xB0, 64, 127),
            0,
            &tap,
            &sender,
            &mut ui,
            &mut held
        ));
        assert!(held.holds(0, 60));
        tap.filter().set_learning(true);
        assert!(route(ev(0x80, 60, 0), 0, &tap, &sender, &mut ui, &mut held));
        assert!(route(ev(0xB0, 64, 0), 0, &tap, &sender, &mut ui, &mut held));
        assert!(!held.holds(0, 60));
        // A release of what the engine never got stays with the interface.
        assert!(!route(
            ev(0x80, 61, 0),
            0,
            &tap,
            &sender,
            &mut ui,
            &mut held
        ));
        let got: Vec<MidiEvent> = std::iter::from_fn(|| engine.pop().ok()).collect();
        assert_eq!(
            got,
            [
                ev(0x90, 60, 100),
                ev(0xB0, 64, 127),
                ev(0x80, 60, 0),
                ev(0xB0, 64, 0)
            ]
        );
        // The interface heard the releases too.
        assert_eq!(std::iter::from_fn(|| heard.pop().ok()).count(), 3);
        // A note-on at velocity 0 is a release too.
        tap.filter().set_learning(false);
        route(ev(0x91, 50, 90), 0, &tap, &sender, &mut ui, &mut held);
        tap.filter().set_learning(true);
        assert!(route(ev(0x91, 50, 0), 0, &tap, &sender, &mut ui, &mut held));
    }

    #[test]
    fn an_input_finds_its_output() {
        let outs: Vec<String> = [
            "Microsoft GS Wavetable Synth",
            "MIDIOUT2 (nanoKONTROL2)",
            "nanoKONTROL2",
            "Launchkey MK3 49 MIDI Out",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(output_for("nanoKONTROL2", &outs), Some(2));
        assert_eq!(output_for("MIDIIN2 (nanoKONTROL2)", &outs), Some(1));
        assert_eq!(output_for("Launchkey MK3 49 MIDI In", &outs), Some(3));
        assert_eq!(output_for("Arturia KeyStep", &outs), None);
    }

    // ---- review 2 -----------------------------------------------------------------------

    fn rig() -> (MidiTap, MidiSender, rtrb::Consumer<MidiEvent>) {
        let tap = MidiTap::new(None);
        tap.filter().set_learning(true);
        let (p, engine) = RingBuffer::<MidiEvent>::new(64);
        (tap, MidiSender::new(p), engine)
    }

    /// Review 2, #15: the pedal let go while its messages are taken reaches the engine as
    /// fully up (a half-pedalling piano is not left half damped); reset all controllers
    /// lets it go; the sostenuto pedal is kept like the sustain.
    #[test]
    fn a_pedal_let_go_while_taken_reaches_the_engine_fully_up() {
        let (tap, sender, mut engine) = rig();
        tap.filter().set_learning(false);
        let (mut ui, _heard) = RingBuffer::<Heard>::new(64);
        let mut held = PortState::default();
        for cc in [64u8, 66] {
            route(ev(0xB0, cc, 127), 0, &tap, &sender, &mut ui, &mut held);
            assert_eq!(engine.pop().unwrap(), ev(0xB0, cc, 127));
            tap.filter().set_learning(true);
            route(ev(0xB0, cc, 50), 0, &tap, &sender, &mut ui, &mut held);
            assert_eq!(engine.pop().unwrap(), ev(0xB0, cc, 0), "fully up");
            route(ev(0xB0, cc, 20), 0, &tap, &sender, &mut ui, &mut held);
            assert!(engine.pop().is_err(), "once");
            tap.filter().set_learning(false);
        }
        // The native instruments ignore reset-all-controllers (review 3, #12): the pedal
        // is still down after it, and its release, taken, still reaches the engine.
        route(ev(0xB0, 64, 127), 0, &tap, &sender, &mut ui, &mut held);
        route(ev(0xB0, 121, 0), 0, &tap, &sender, &mut ui, &mut held);
        while engine.pop().is_ok() {}
        tap.filter().set_learning(true);
        route(ev(0xB0, 64, 0), 0, &tap, &sender, &mut ui, &mut held);
        assert_eq!(engine.pop().unwrap(), ev(0xB0, 64, 0), "the release");
        tap.filter().set_learning(false);
        // A half pedal (review 3, #13): resting at 50, lifted while taken — fully up.
        route(ev(0xB0, 64, 50), 0, &tap, &sender, &mut ui, &mut held);
        assert_eq!(engine.pop().unwrap(), ev(0xB0, 64, 50));
        tap.filter().set_learning(true);
        route(ev(0xB0, 64, 30), 0, &tap, &sender, &mut ui, &mut held);
        assert_eq!(
            engine.pop().unwrap(),
            ev(0xB0, 64, 0),
            "a half pedal let go"
        );
        tap.filter().set_learning(false);
        // The soft pedal (Chorda's una corda) is kept too.
        route(ev(0xB0, 67, 127), 0, &tap, &sender, &mut ui, &mut held);
        assert_eq!(engine.pop().unwrap(), ev(0xB0, 67, 127));
        tap.filter().set_learning(true);
        route(ev(0xB0, 67, 0), 0, &tap, &sender, &mut ui, &mut held);
        assert_eq!(
            engine.pop().unwrap(),
            ev(0xB0, 67, 0),
            "the soft pedal let go"
        );
    }

    /// Review 3, #10, #11, #26: under a full ring, the pitch wheel and channel pressure end
    /// at their last values (one address per channel, not one per value); what came after
    /// the ring filled waits with what is parked, so a pad's release is never handed over
    /// before its press; the parked messages come in the order and at the times they came.
    #[test]
    fn a_full_ring_keeps_order_and_the_last_bend_and_pressure() {
        let (tap, sender, _engine) = rig();
        let (mut ui, mut heard) = RingBuffer::<Heard>::new(2);
        let mut held = PortState::default();
        route(ev(0xB0, 1, 1), 0, &tap, &sender, &mut ui, &mut held);
        route(ev(0xB0, 1, 2), 0, &tap, &sender, &mut ui, &mut held);
        let t0 = Instant::now();
        for v in [10u8, 90, 40, 64] {
            route(ev(0xE0, v, v), 0, &tap, &sender, &mut ui, &mut held);
        }
        for v in [100u8, 30, 0] {
            route(ev(0xD0, v, 0), 0, &tap, &sender, &mut ui, &mut held);
        }
        route(ev(0x90, 36, 100), 0, &tap, &sender, &mut ui, &mut held);
        // The ring has room again (the interface popped one), but the table is not taken:
        // what comes now waits behind it.
        assert!(heard.pop().is_ok());
        route(ev(0x80, 36, 0), 0, &tap, &sender, &mut ui, &mut held);
        let mut out = Vec::new();
        while let Ok(h) = heard.pop() {
            out.push(h.event);
        }
        assert_eq!(out, vec![ev(0xB0, 1, 2)]);
        let mut late = Vec::new();
        tap.take_overflow(&mut late);
        let events: Vec<MidiEvent> = late.iter().map(|h| h.event).collect();
        assert_eq!(
            events,
            vec![ev(0xE0, 64, 64), ev(0xD0, 0, 0), ev(0x80, 36, 0)],
            "the newest of each, in the order they came"
        );
        assert!(late.iter().all(|h| h.at >= t0), "at their own times");
        assert!(late.windows(2).all(|w| w[0].at <= w[1].at));
    }

    /// Review 2, #16: a full ring keeps the newest value of each control: the drain hands
    /// it over after the ring's.
    #[test]
    fn a_full_ring_keeps_the_newest_value() {
        let (tap, sender, _engine) = rig();
        let (mut ui, mut heard) = RingBuffer::<Heard>::new(2);
        let mut held = PortState::default();
        for v in 1..=6u8 {
            route(ev(0xB0, 7, v), 0, &tap, &sender, &mut ui, &mut held);
        }
        route(ev(0x90, 36, 100), 0, &tap, &sender, &mut ui, &mut held);
        route(ev(0x80, 36, 0), 0, &tap, &sender, &mut ui, &mut held);
        let mut out = Vec::new();
        while let Ok(h) = heard.pop() {
            out.push(h.event);
        }
        let mut late = Vec::new();
        tap.take_overflow(&mut late);
        let late: Vec<MidiEvent> = late.iter().map(|h| h.event).collect();
        assert_eq!(out, vec![ev(0xB0, 7, 1), ev(0xB0, 7, 2)]);
        assert!(late.contains(&ev(0xB0, 7, 6)), "{late:?}");
        assert!(late.contains(&ev(0x80, 36, 0)), "the release: {late:?}");
        let mut again = Vec::new();
        tap.take_overflow(&mut again);
        assert!(again.is_empty());
    }

    /// Review 2, #32: the wake runs on the tap's own thread, never on the callback's (it
    /// may take a lock the interface holds for a frame).
    #[test]
    fn the_wake_runs_on_its_own_thread() {
        let on = Arc::new(std::sync::Mutex::new(None));
        let o = on.clone();
        let tap = MidiTap::new(Some(Arc::new(move || {
            *o.lock().unwrap() = Some(std::thread::current().id());
        })));
        let mut set = FilterSet::default();
        set.take(0, Some(0), Control::Cc(74));
        tap.filter().publish(&set);
        let (p, _engine) = RingBuffer::<MidiEvent>::new(16);
        let sender = MidiSender::new(p);
        let (mut ui, _heard) = RingBuffer::<Heard>::new(16);
        let mut held = PortState::default();
        route(ev(0xB0, 74, 3), 0, &tap, &sender, &mut ui, &mut held);
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while on.lock().unwrap().is_none() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        let woke = on.lock().unwrap().expect("woken");
        assert_ne!(woke, std::thread::current().id());
    }

    /// Review 2, #13: the rearm is a swap, so a wake is never lost between the rearm and
    /// the rings' reads.
    #[test]
    fn the_rearm_reports_a_pending_wake() {
        let tap = MidiTap::new(None);
        tap.wake();
        assert!(tap.rearm(), "a wake was pending");
        assert!(!tap.rearm());
    }
}
