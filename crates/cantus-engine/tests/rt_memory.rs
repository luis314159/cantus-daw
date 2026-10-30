//! Memory on and off the audio thread (ADR 0022): an EQ holds only what its mode needs; the
//! linear-phase buffers arrive from the UI thread and the switch, when they land, gives
//! exactly what an EQ made in linear phase gives; and the processor itself never allocates
//! while it plays — through a mode switch, the answer to its request, a soft bypass, a
//! gate played by a track's notes, the click and the recorder.
//!
//! The file installs a counting allocator. It counts per thread, so the tests of this file
//! running in parallel do not see each other's allocations — and on every thread of the
//! engine's pool (round 22): a worker marks itself when it starts
//! (`cantus_engine::processor::on_worker_start`), and while any test is counting, a heap
//! event on a worker is counted too.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::aequalis::{latency_of, Mode, Resolution};
use cantus_engine::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use support::*;

struct Counting;

thread_local! {
    /// While set, every allocation, reallocation and free on this thread is counted.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static EVENTS: Cell<usize> = const { Cell::new(0) };
    /// Net bytes this thread holds from the heap (allocated minus freed).
    static LIVE: Cell<isize> = const { Cell::new(0) };
    /// The size of the first counted event (negative for a free): a clue to what it was.
    static FIRST: Cell<isize> = const { Cell::new(0) };
}

// Pool workers mark themselves; their heap events count while any test counts.
thread_local! {
    static WORKER: Cell<bool> = const { Cell::new(false) };
}
static COUNTING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static WORKER_EVENTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn mark_worker(_slot: usize, serving: bool) {
    WORKER.with(|w| w.set(serving));
}

fn count(bytes: isize) {
    use std::sync::atomic::Ordering::Relaxed;
    if WORKER.try_with(|w| w.get()).unwrap_or(false) && COUNTING.load(Relaxed) > 0 {
        WORKER_EVENTS.fetch_add(1, Relaxed);
    }
    let _ = LIVE.try_with(|c| c.set(c.get() + bytes));
    let _ = ARMED.try_with(|a| {
        if a.get() {
            let _ = EVENTS.try_with(|n| {
                if n.get() == 0 {
                    let _ = FIRST.try_with(|f| f.set(bytes));
                }
                n.set(n.get() + 1)
            });
        }
    });
}

// SAFETY: every call is forwarded unchanged to the system allocator; the bookkeeping
// touches only const-initialised thread locals, which neither allocate nor panic.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count(-(layout.size() as isize));
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size as isize - layout.size() as isize);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live() -> isize {
    LIVE.with(|c| c.get())
}

/// Run `f` counting every heap event on this thread; returns what it returned and the
/// count.
fn armed<T>(f: impl FnOnce() -> T) -> (T, usize) {
    use std::sync::atomic::Ordering::SeqCst;
    EVENTS.with(|n| n.set(0));
    COUNTING.fetch_add(1, SeqCst);
    let workers = WORKER_EVENTS.load(SeqCst);
    ARMED.with(|a| a.set(true));
    let out = f();
    ARMED.with(|a| a.set(false));
    let workers = WORKER_EVENTS.load(SeqCst) - workers;
    COUNTING.fetch_sub(1, SeqCst);
    (out, EVENTS.with(|n| n.get()) + workers)
}

/// Run `f` counting the heap events of this thread alone — for code that runs on one
/// thread (a MIDI port's callback): the pool workers other tests run meanwhile are theirs,
/// not its (macOS's workers join a workgroup as they start, and `armed` counted that once).
fn armed_here<T>(f: impl FnOnce() -> T) -> (T, usize) {
    EVENTS.with(|n| n.set(0));
    ARMED.with(|a| a.set(true));
    let out = f();
    ARMED.with(|a| a.set(false));
    (out, EVENTS.with(|n| n.get()))
}

/// Net heap bytes `f`'s result holds.
fn footprint<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = live();
    let out = f();
    (out, (live() - before).max(0) as usize)
}

fn eq_slot(p: &mut Project, pairs: &[(&str, f32)]) -> EffectSlot {
    let mut slot = p.make_effect(EffectKind::Eq);
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.freq", 800.0),
        ("b0.gain", 6.0),
        ("b0.q", 1.5),
        ("b1.used", 1.0),
        ("b1.shape", 3.0),
        ("b1.freq", 120.0),
        ("b1.slope", 24.0),
    ]
    .iter()
    .chain(pairs)
    {
        slot.spec.set(k, *v).expect("an EQ parameter");
    }
    slot
}

/// The heap an EQ holds, by mode. Zero latency and natural phase hold no linear-phase
/// buffers at all; linear phase holds what its resolution needs, not what the largest
/// would.
#[test]
fn an_eq_holds_only_what_its_mode_needs() {
    let mut p = Project::new("memory");
    let zl = eq_slot(&mut p, &[]);
    let np = eq_slot(&mut p, &[("mode", 1.0)]);
    let medium = eq_slot(&mut p, &[("mode", 2.0), ("lp_res", 1.0)]);
    let very_high = eq_slot(&mut p, &[("mode", 2.0), ("lp_res", 3.0)]);
    let (a, zl_bytes) = footprint(|| EffectUnit::new(&zl, 48_000.0));
    let (b, np_bytes) = footprint(|| EffectUnit::new(&np, 48_000.0));
    let (c, medium_bytes) = footprint(|| EffectUnit::new(&medium, 48_000.0));
    let (d, vh_bytes) = footprint(|| EffectUnit::new(&very_high, 48_000.0));
    eprintln!(
        "EQ heap at 48 kHz: zero latency {zl_bytes} B, natural phase {np_bytes} B, \
         linear Medium {medium_bytes} B, linear Very High {vh_bytes} B"
    );
    // Zero latency: the bands, the natural-phase correction (kept, so switching to it
    // never waits) and the key's analyser (96 KiB of it) — no convolver. Measured
    // 153 232 B at the time of writing; round 16's EQ held about 4.2 MB whatever its mode.
    assert!(
        zl_bytes < 192 * 1024,
        "a zero-latency EQ holds {zl_bytes} B, over the 192 KiB bound"
    );
    assert!(
        np_bytes < 192 * 1024,
        "a natural-phase EQ holds {np_bytes} B"
    );
    assert!(a.memory_need().is_none() && b.memory_need().is_none());
    assert!(
        c.memory_need().is_none(),
        "made on the UI thread, it has its buffers"
    );
    assert!(
        medium_bytes > zl_bytes + 512 * 1024,
        "linear phase does hold its convolver ({medium_bytes} B)"
    );
    assert!(
        vh_bytes > medium_bytes * 3,
        "Very High holds about four times Medium ({vh_bytes} B vs {medium_bytes} B)"
    );
    drop((a, b, c, d));
}

/// Switching an EQ to linear phase: it keeps running its previous path until the buffers
/// arrive, then switches — and from the switch on its output is, to the bit, what an EQ
/// made in linear phase gives for the same input.
#[test]
fn switching_to_linear_phase_matches_an_eq_made_in_it() {
    let sr = 48_000.0;
    let mut p = Project::new("switch");
    let zl = eq_slot(&mut p, &[]);
    let mut lp = zl.clone();
    lp.spec.set("mode", 2.0).unwrap();
    lp.spec.set("lp_res", 1.0).unwrap();
    let n = 48_000;
    let mut seed = 12_345u32;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
            0.3 * noise + 0.2 * math::sin(i as f32 * 0.031)
        })
        .collect();
    let run = |u: &mut EffectUnit, from: usize, to: usize, out: &mut Vec<f32>| {
        for start in (from..to).step_by(256) {
            let end = (start + 256).min(to);
            let mut l = x[start..end].to_vec();
            let mut r: Vec<f32> = x[start..end].iter().map(|v| v * 0.5).collect();
            u.process(&mut l, &mut r);
            for (a, b) in l.iter().zip(&r) {
                out.push(*a);
                out.push(*b);
            }
        }
    };
    let (switch_at, arrive_at) = (8_192, 12_288);
    let mut a = EffectUnit::new(&zl, sr);
    let mut reference = EffectUnit::new(&zl, sr);
    let mut out_a = Vec::new();
    let mut out_ref = Vec::new();
    run(&mut a, 0, switch_at, &mut out_a);
    a.configure(&lp.spec);
    assert_eq!(
        a.memory_need(),
        Some(MemoryNeed::EqLinear(Resolution::Medium)),
        "linear phase is asked for and its buffers are not there"
    );
    assert_eq!(a.latency(), 0, "until they arrive, the previous path runs");
    run(&mut a, switch_at, arrive_at, &mut out_a);
    run(&mut reference, 0, arrive_at, &mut out_ref);
    assert_eq!(
        out_a, out_ref,
        "while waiting, the EQ is exactly its zero-latency self"
    );
    let need = a.memory_need().unwrap();
    assert!(a.supply(EffectMemory::build(need, sr)).is_none());
    assert_eq!(
        a.latency(),
        latency_of(Mode::LinearPhase, Resolution::Medium, sr)
    );
    assert!(a.memory_need().is_none());
    let mut after = Vec::new();
    run(&mut a, arrive_at, n, &mut after);
    let mut made = EffectUnit::new(&lp, sr);
    let mut fresh = Vec::new();
    run(&mut made, arrive_at, n, &mut fresh);
    assert!(
        fresh.iter().any(|v| v.abs() > 0.05),
        "the reference is not silent"
    );
    assert_eq!(
        after, fresh,
        "after the switch, the EQ is exactly one made in linear phase"
    );
}

/// Drive a processor the way the audio callback does, with every heap event counted while
/// it runs; between blocks, answer its requests the way the backend does (off the audio
/// thread, uncounted).
struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    p: Processor,
    cap: rtrb::Consumer<f32>,
    buf: Vec<f32>,
    /// Heap events while processing, per phase of the test: (phase, count).
    allocations: Vec<(&'static str, usize)>,
    known: std::collections::HashSet<EffectId>,
}

impl Rig {
    fn new() -> Self {
        Self::with(None)
    }

    /// With the pool's workers given (`CANTUS_WORKERS` otherwise).
    fn with(workers: Option<Workers>) -> Self {
        cantus_engine::processor::on_worker_start(mark_worker);
        let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let (cap_tx, cap) = rtrb::RingBuffer::<f32>::new(CAPTURE_SAMPLES);
        let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
        if let Some(w) = workers {
            p.set_workers(w);
        }
        p.attach_capture(cap_tx);
        Rig {
            tx,
            rx,
            p,
            cap,
            buf: vec![0.0; 512 * 2],
            allocations: Vec::new(),
            known: Default::default(),
        }
    }

    fn snapshot(&mut self, project: &Project) {
        let snap = Snapshot::compile(project, 1);
        for slot in snap.effect_slots() {
            if self.known.insert(slot.id) {
                let unit = Box::new(EffectUnit::new(slot, 48_000.0));
                self.tx.push(EngineMessage::AddEffect(unit)).unwrap();
            }
        }
        let snap = Arc::new(snap);
        // Its automation's runtime first (ADR 0027): built here, off the audio thread.
        self.tx
            .push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .unwrap();
        self.tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    }

    /// `blocks` blocks of 512, counted under `phase`; the UI's side between each.
    fn play(&mut self, phase: &'static str, blocks: usize) {
        let mut total = 0;
        for _ in 0..blocks {
            let buf = &mut self.buf;
            let p = &mut self.p;
            let ((), n) = armed(|| p.process(buf, 2));
            total += n;
            self.answer();
        }
        self.allocations.push((phase, total));
    }

    fn answer(&mut self) {
        while let Ok(ev) = self.rx.pop() {
            match ev {
                EngineEvent::PdcNeed { count, delay } => {
                    let lines = cantus_engine::pdc::lines_for(count, delay);
                    self.tx.push(EngineMessage::PdcLines(lines)).unwrap();
                }
                EngineEvent::EffectNeed { effect, need } => {
                    let memory = EffectMemory::build(need, 48_000.0);
                    self.tx
                        .push(EngineMessage::EffectMemory { effect, memory })
                        .unwrap();
                }
                _ => {}
            }
        }
        while self.cap.pop().is_ok() {}
    }
}

/// Everything this round added to the audio thread, played through: an EQ switched to
/// linear phase and to a larger resolution (buffers requested, delivered, swapped, the old
/// ones handed back), soft bypass on and off, the chain switch, a gate played by a track's
/// notes, the delayed click and a strip recorder. Not one allocation while processing.
#[test]
fn the_audio_thread_never_allocates() {
    audio_thread_scenario(None);
}

/// The same with three workers in the pool, always parallel: neither the callback nor any
/// worker allocates or frees.
#[test]
fn the_pool_threads_never_allocate() {
    audio_thread_scenario(Some(Workers::parallel(3)));
}

/// Voices on several threads (ADR 0038): chords on Calor, Unda, Exemplar and Chorda with three
/// workers, their voices forked to the pool block after block — not one allocation on the
/// callback or a worker, and the workers really did render voices.
#[test]
fn forked_voices_never_allocate() {
    let mut p = Project::new("voices");
    p.tempo_bpm = 120.0;
    for (t, kind) in [
        InstrumentKind::Analog,
        InstrumentKind::Wavetable,
        InstrumentKind::Sampler,
        InstrumentKind::Chorda,
    ]
    .into_iter()
    .enumerate()
    {
        let mut tr = p.make_track(format!("{kind:?}"));
        tr.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        let mut clip = p.make_clip("chords", Ticks::ZERO, Ticks::beats(8));
        for bar in 0..4i64 {
            for k in 0..8u8 {
                let len = Ticks::beats(1 + (k as i64 % 2));
                let note = p.make_note(Ticks::beats(bar * 2), len, 40 + t as u8 * 5 + k * 4, 90);
                clip.notes.push(note);
            }
        }
        tr.clips.push(clip);
        p.tracks.push(tr);
    }
    let mut rig = Rig::with(Some(Workers::parallel(3)));
    let snap = Snapshot::compile(&p, 1);
    // Chorda's keyboard designed before its unit is made (a unit never waits for it).
    cantus_engine::inst::chorda::ensure(Default::default(), 48_000.0);
    for unit in cantus_engine::inst::units_for(&snap, 48_000.0, |_, _| false) {
        rig.tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("warm", 20);
    rig.allocations.clear();
    rig.play("chords", 300);
    let s = *rig.p.stats();
    assert!(
        s.voice_jobs_helped > 0,
        "no worker rendered a voice ({} forks)",
        s.voice_forks
    );
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "forked voices allocated or freed: {:?} (the first: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

fn audio_thread_scenario(workers: Option<Workers>) {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    p.metronome = true;
    let s0 = p.tracks[0].strip.unwrap();
    let s1 = p.tracks[1].strip.unwrap();
    let eq = p.make_effect(EffectKind::Eq);
    let eq_id = eq.id;
    p.strip_mut(s0).unwrap().effects.push(eq);
    let mut gate = p.make_effect(EffectKind::Gate);
    gate.spec.set("trig", 1.0).unwrap();
    gate.spec.set("look", 5.0).unwrap();
    gate.midi_from = Some(p.tracks[0].id);
    p.strip_mut(s1).unwrap().effects.push(gate);
    let mut rig = Rig::with(workers);
    rig.snapshot(&p);
    rig.tx
        .push(EngineMessage::SetCapture(CaptureTarget::Strip(s1)))
        .unwrap();
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("start", 20);

    let set = |p: &mut Project, k: &str, v: f32| {
        let e = p.strip_mut(s0).unwrap().effect_mut(eq_id).unwrap();
        e.spec.set(k, v).unwrap();
    };
    // Linear phase at Medium: asked for, delivered, switched.
    set(&mut p, "b0.used", 1.0);
    set(&mut p, "b0.gain", 4.0);
    set(&mut p, "mode", 2.0);
    set(&mut p, "lp_res", 1.0);
    rig.snapshot(&p);
    rig.play("linear Medium", 40);
    assert_eq!(
        rig.p.stats().latency_samples as usize,
        latency_of(Mode::LinearPhase, Resolution::Medium, 48_000.0),
        "the EQ switched: its strip is now the slowest path"
    );
    // Very High: larger buffers; the Medium ones go back as garbage.
    set(&mut p, "lp_res", 3.0);
    rig.snapshot(&p);
    rig.play("linear Very High", 80);
    assert_eq!(
        rig.p.stats().latency_samples as usize,
        latency_of(Mode::LinearPhase, Resolution::VeryHigh, 48_000.0)
    );
    // Soft bypass, the chain switch, back to Medium (it fits), back to zero latency.
    p.strip_mut(s0).unwrap().effect_mut(eq_id).unwrap().bypass = true;
    rig.snapshot(&p);
    rig.play("soft bypass", 10);
    p.strip_mut(s0).unwrap().fx_enabled = false;
    rig.snapshot(&p);
    rig.play("chain off", 10);
    p.strip_mut(s0).unwrap().fx_enabled = true;
    p.strip_mut(s0).unwrap().effect_mut(eq_id).unwrap().bypass = false;
    set(&mut p, "lp_res", 1.0);
    rig.snapshot(&p);
    rig.play("Medium again", 10);
    set(&mut p, "mode", 0.0);
    rig.snapshot(&p);
    rig.play("zero latency", 10);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(1))).unwrap();
    rig.play("seek", 10);
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("stop", 4);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

/// The master's chain (ADR 0026) on the audio thread: a limiter and an EQ that switches to
/// linear phase (memory asked for and delivered), a compressor keyed from a strip, soft
/// bypass, the chain switch, an insert taken out (its unit handed back), the master take.
/// Not one allocation while processing.
#[test]
fn the_master_chain_never_allocates() {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    p.metronome = true;
    let key = p.tracks[1].strip.unwrap();
    let lim = p.make_effect(EffectKind::Limiter);
    let lim_id = lim.id;
    let eq = p.make_effect(EffectKind::Eq);
    let eq_id = eq.id;
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.sidechain = Some(key);
    comp.mix = 0.5;
    p.master.effects = vec![eq, comp, lim];
    let mut rig = Rig::new();
    rig.snapshot(&p);
    rig.tx
        .push(EngineMessage::SetCapture(CaptureTarget::Master))
        .unwrap();
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("master start", 20);
    let set = |p: &mut Project, k: &str, v: f32| {
        let e = p.effect_mut(StripId::MASTER, eq_id).unwrap();
        e.spec.set(k, v).unwrap();
    };
    set(&mut p, "b0.used", 1.0);
    set(&mut p, "b0.gain", 3.0);
    set(&mut p, "mode", 2.0);
    set(&mut p, "lp_res", 1.0);
    rig.snapshot(&p);
    rig.play("master linear phase", 40);
    let latency = rig.p.stats().latency_samples as usize;
    assert!(
        latency >= latency_of(Mode::LinearPhase, Resolution::Medium, 48_000.0),
        "the master's EQ in linear phase is in the output's latency: {latency}"
    );
    p.effect_mut(StripId::MASTER, eq_id).unwrap().bypass = true;
    rig.snapshot(&p);
    rig.play("master soft bypass", 10);
    assert_eq!(rig.p.stats().latency_samples as usize, latency);
    p.master.fx_enabled = false;
    rig.snapshot(&p);
    rig.play("master chain off", 10);
    p.master.fx_enabled = true;
    p.master.effects.retain(|e| e.id != lim_id);
    rig.snapshot(&p);
    rig.play("master insert removed", 10);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(1))).unwrap();
    rig.play("master seek", 10);
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("master stop", 4);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

/// Automation (ADR 0027) on the audio thread: a fader ride and a pan sweep ramping, an
/// EQ's band gain and Firmus's cutoff configured at control points, the master's width;
/// a new snapshot (and runtime) mid-play, a seek, a stop. Not one allocation.
#[test]
fn automation_never_allocates() {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    let s0 = p.tracks[0].strip.unwrap();
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("b0.used", 1.0).unwrap();
    let eq_id = eq.id;
    p.strip_mut(s0).unwrap().effects.push(eq);
    let lead = p.tracks[0].id;
    let ramp = |a: f32, b: f32| {
        vec![
            AutoPoint::new(Ticks::ZERO, a),
            AutoPoint::new(Ticks::beats(4), b),
        ]
    };
    let targets = [
        AutoTarget::StripGain(s0),
        AutoTarget::StripPan(s0),
        AutoTarget::Effect {
            effect: eq_id,
            key: "b0.gain".into(),
        },
        AutoTarget::Firmus {
            track: lead,
            param: FirmusParam::Cutoff,
        },
        AutoTarget::MasterWidth,
        AutoTarget::TrackGain(lead),
    ];
    for t in targets {
        let c = p.make_automation_clip("a", t, Ticks::ZERO, Ticks::beats(8), ramp(0.2, 0.9));
        p.tracks[0].clips.push(c);
    }
    let mut rig = Rig::new();
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("automation start", 30);
    p.tracks[0]
        .clips
        .last_mut()
        .unwrap()
        .automation
        .as_mut()
        .unwrap()
        .points[1]
        .value = 0.1;
    rig.snapshot(&p);
    rig.play("automation new snapshot", 30);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(2))).unwrap();
    rig.play("automation seek", 10);
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("automation stop", 4);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

/// Modulators (ADR 0057) on the audio thread: every kind at once — an LFO on an EQ band,
/// steps on a fader, a random source on a pan, an envelope from notes (sequenced and live)
/// on Firmus's cutoff, a follower on the master's width, one nested into another's
/// amount — through a new snapshot (state carried over), live notes, a seek, a stop (the
/// modulators run on, stopped) and a new start. Not one allocation.
#[test]
fn modulators_never_allocate() {
    use cantus_core::modulator::*;
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    let s0 = p.tracks[0].strip.unwrap();
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("b0.used", 1.0).unwrap();
    let eq_id = eq.id;
    p.strip_mut(s0).unwrap().effects.push(eq);
    let lead = p.tracks[0].id;
    let routes = [
        (
            ModKind::Lfo,
            AutoTarget::Effect {
                effect: eq_id,
                key: "b0.gain".into(),
            },
        ),
        (ModKind::Step, AutoTarget::StripGain(s0)),
        (ModKind::Random, AutoTarget::StripPan(s0)),
        (
            ModKind::Envelope,
            AutoTarget::Firmus {
                track: lead,
                param: FirmusParam::Cutoff,
            },
        ),
        (ModKind::Follower, AutoTarget::MasterWidth),
    ];
    let mut first = None;
    for (kind, target) in routes {
        let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
        m.routes.push(ModRoute { target, depth: 0.3 });
        first.get_or_insert(m.id);
        p.tracks[0].modulators.push(m);
    }
    let mut nest = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    nest.routes.push(ModRoute {
        target: AutoTarget::ModAmount(first.unwrap()),
        depth: -0.5,
    });
    p.master.modulators.push(nest);
    let mut rig = Rig::new();
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("modulators start", 30);
    p.tracks[0].modulators[0].routes[0].depth = -0.7;
    rig.snapshot(&p);
    rig.play("modulators new snapshot", 30);
    rig.tx
        .push(EngineMessage::NoteOn {
            track: lead,
            pitch: 64,
            velocity: 100,
        })
        .unwrap();
    rig.play("modulators live note", 10);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(2))).unwrap();
    rig.play("modulators seek", 10);
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("modulators stopped", 20);
    rig.tx
        .push(EngineMessage::NoteOff {
            track: lead,
            pitch: 64,
        })
        .unwrap();
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("modulators play again", 10);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

/// Pre-fader sends (round 25) on the audio thread: undelayed and through compensation
/// lines, from a fader under automation (the pre pair kept apart from the ramped one), a
/// tap switched mid-play, the source muted and unmuted. Not one allocation.
#[test]
fn pre_fader_sends_never_allocate() {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    let (lead, bass) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    let (aux, bus) = (p.strips[3].id, p.strips[4].id);
    let lim = p.make_effect(EffectKind::Limiter);
    p.strip_mut(bus).unwrap().effects.push(lim);
    p.strip_mut(bass).unwrap().output = Some(bus);
    let pre = |to: StripId, level: f32| Send {
        to,
        level,
        enabled: true,
        tap: SendTap::Pre,
    };
    p.strip_mut(lead).unwrap().sends = vec![pre(aux, 0.5)];
    p.strip_mut(bass).unwrap().sends = vec![pre(aux, 0.7)];
    for target in [AutoTarget::StripGain(lead), AutoTarget::StripPan(lead)] {
        let points = vec![
            AutoPoint::new(Ticks::ZERO, 0.2),
            AutoPoint::new(Ticks::beats(4), 0.9),
        ];
        let c = p.make_automation_clip("a", target, Ticks::ZERO, Ticks::beats(8), points);
        p.tracks[0].clips.push(c);
    }
    let mut rig = Rig::new();
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("pre sends start", 30);
    p.strip_mut(lead).unwrap().sends[0].tap = SendTap::Post;
    rig.snapshot(&p);
    rig.play("pre sends: one switched to post", 10);
    p.strip_mut(lead).unwrap().sends[0].tap = SendTap::Pre;
    p.strip_mut(lead).unwrap().muted = true;
    p.strip_mut(bass).unwrap().muted = true;
    rig.snapshot(&p);
    rig.play("pre sends muted", 10);
    p.strip_mut(lead).unwrap().muted = false;
    p.strip_mut(bass).unwrap().muted = false;
    rig.snapshot(&p);
    rig.play("pre sends unmuted", 10);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(1))).unwrap();
    rig.play("pre sends seek", 10);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

/// Streaming from disk (ADR 0050): a piano-like Exemplar whose long samples stream — chords
/// held past the heads, so voices copy blocks from the pool, publish their cursors and, as
/// this rig runs far faster than real time, also meet late blocks and fade — with and
/// without the pool's workers. Not one allocation on the callback or a worker.
#[test]
fn streaming_voices_never_allocate() {
    use cantus_core::inst::sampler::Zone;
    let dir = tempfile::tempdir().unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    let mut spec = NativeSpec::new(InstrumentKind::Sampler);
    // The same instrument with its second zone removed: its stream let go while voices
    // play (second review #2).
    let mut first_only = NativeSpec::new(InstrumentKind::Sampler);
    for (n, (lo, hi, root)) in [(0u8, 59u8, 48u8), (60, 127, 72)].iter().enumerate() {
        let path = dir.path().join(format!("s{n}.wav"));
        let w = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wr = hound::WavWriter::create(&path, w).unwrap();
        for i in 0..200_000 {
            let v = (math::sin(i as f32 * 0.031 * (n + 1) as f32) * 9_000.0) as i16;
            wr.write_sample(v).unwrap();
            wr.write_sample(-v).unwrap();
        }
        wr.finalize().unwrap();
        let z = Zone::new(Some(path.to_string_lossy().into()), *lo, *hi, *root);
        for (k, v) in z.values_for(n) {
            spec.set(&k, v).unwrap();
            if n == 0 {
                first_only.set(&k, v).unwrap();
            }
        }
        for (k, t) in z.texts_for(n) {
            spec.set_text(&k, Some(t.clone())).unwrap();
            if n == 0 {
                first_only.set_text(&k, Some(t)).unwrap();
            }
        }
    }
    let mut p = Project::new("stream");
    p.tempo_bpm = 120.0;
    let mut tr = p.make_track("Piano");
    tr.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("chords", Ticks::ZERO, Ticks::beats(16));
    for bar in 0..4i64 {
        for k in [36u8, 43, 48, 55, 60, 64, 67, 72] {
            let note = p.make_note(Ticks::beats(bar * 4), Ticks::beats(4), k + bar as u8, 90);
            clip.notes.push(note);
        }
    }
    tr.clips.push(clip);
    p.tracks.push(tr);
    for (name, workers) in [("serial", None), ("pool", Some(Workers::parallel(3)))] {
        let mut rig = Rig::with(workers);
        let mut cache = SampleCache::new();
        cache.set_streaming(true);
        let mut snap = Snapshot::compile(&p, 1);
        assert!(cache.attach(&mut snap).is_empty());
        assert!(
            cantus_engine::inst::stream::lookup(snap.tracks[0].assets[0].1.as_ref().unwrap())
                .is_some(),
            "streamed"
        );
        for unit in cantus_engine::inst::units_for(&snap, 48_000.0, |_, _| false) {
            rig.tx.push(EngineMessage::AddInstrument(unit)).unwrap();
        }
        let snap = Arc::new(snap);
        rig.tx
            .push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .unwrap();
        rig.tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
        rig.tx.push(EngineMessage::Play).unwrap();
        rig.play("warm", 4);
        rig.allocations.clear();
        rig.play("streaming", 700);
        // A voice wakes the reader only when it needs it sooner than the reader's next look
        // (a non-blocking system call on the audio thread, counted: ADR 0050 §4); ordinary
        // playback like this makes none (inst_stream's `ordinary_playback_wakes_the_reader_
        // not_once` holds that), so here only the bound: at most one per voice per callback.
        let wakes = rig.p.stats().stream_wakes;
        assert!(wakes <= 32 * 704, "wakes bounded ({name}): {wakes}");
        // The second zone removed while its voices play: the zone's source and the ended
        // voices' playheads let go of its stream on the audio thread — never the last
        // holder (the hub's entry and the sample cache's head are), so never a free.
        let mut q = p.clone();
        q.tracks[0].instrument = InstrumentSpec::Native(first_only.clone());
        let mut fewer = Snapshot::compile(&q, 2);
        assert!(cache.attach(&mut fewer).is_empty());
        let fewer = Arc::new(fewer);
        rig.tx
            .push(EngineMessage::Automation(AutomationRuntime::build(&fewer)))
            .unwrap();
        rig.tx.push(EngineMessage::SetSnapshot(fewer)).unwrap();
        rig.play("letting go", 300);
        assert!(
            rig.allocations.iter().all(|(_, n)| *n == 0),
            "streaming allocated or freed ({name}): {:?} (the first: {} B)",
            rig.allocations,
            FIRST.with(|f| f.get())
        );
    }
}

/// The tempo map (ADR 0060) on the audio thread: a ramp, a step and metre changes played
/// through — passes cut at the steps, the click counted through the metres, a synced
/// tremolo and automation following, a loop over the tempo change wrapping — then the map
/// edited while it plays (the transport re-anchors and lets go of the old map, which the
/// old snapshot still holds), a seek before the song (the count-in), a stop. Serially and
/// with the pool. Not one allocation, not one free.
#[test]
fn a_tempo_map_never_allocates() {
    use cantus_core::tempo::{MetreChange, TempoPoint};
    for workers in [None, Some(Workers::parallel(3))] {
        let mut p = demo_project();
        p.tempo_bpm = 90.0;
        let pt = |beats: i64, bpm: f64, ramp: bool| TempoPoint {
            tick: Ticks::beats(beats),
            bpm,
            ramp,
        };
        p.tempo_points = vec![pt(0, 90.0, true), pt(4, 140.0, false), pt(6, 100.0, false)];
        p.metre_changes = vec![
            MetreChange {
                tick: Ticks::ZERO,
                signature: TimeSignature::new(4, 4),
            },
            MetreChange {
                tick: Ticks::beats(4),
                signature: TimeSignature::new(7, 8),
            },
        ];
        p.metronome = true;
        p.loop_region = Some(LoopRegion {
            start: Ticks::beats(2),
            end: Ticks::beats(7),
        });
        p.loop_enabled = true;
        let s0 = p.tracks[0].strip.unwrap();
        let trem = p.make_effect(EffectKind::Tremolo);
        p.strip_mut(s0).unwrap().effects.push(trem);
        let points = vec![
            AutoPoint::new(Ticks::ZERO, 0.2),
            AutoPoint::new(Ticks::beats(6), 0.9),
        ];
        let c = p.make_automation_clip(
            "a",
            AutoTarget::StripGain(s0),
            Ticks::ZERO,
            Ticks::beats(8),
            points,
        );
        p.tracks[0].clips.push(c);
        let mut rig = Rig::with(workers);
        rig.snapshot(&p);
        rig.tx.push(EngineMessage::Play).unwrap();
        rig.play("tempo map: the ramp, the loop", 400);
        p.tempo_points[1].bpm = 150.0;
        rig.snapshot(&p);
        rig.play("tempo map: edited while playing", 40);
        rig.tx.push(EngineMessage::Seek(Ticks(-3840))).unwrap();
        rig.play("tempo map: the count-in", 40);
        rig.tx.push(EngineMessage::Stop).unwrap();
        rig.play("tempo map: stopped", 8);
        assert!(
            rig.allocations.iter().all(|(_, n)| *n == 0),
            "the tempo map allocated or freed: {:?} (the first of the last phase: {} B)",
            rig.allocations,
            FIRST.with(|f| f.get())
        );
    }
}

impl Rig {
    /// Send a snapshot made elsewhere (frozen renders attached by an `EngineHandle`).
    fn send(&mut self, snap: Arc<Snapshot>) {
        for slot in snap.effect_slots() {
            if self.known.insert(slot.id) {
                let unit = Box::new(EffectUnit::new(slot, 48_000.0));
                self.tx.push(EngineMessage::AddEffect(unit)).unwrap();
            }
        }
        self.tx
            .push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .unwrap();
        self.tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    }
}

/// Frozen playback (round 29, ADR 0064) on the audio thread and the pool: a frozen strip
/// and a frozen track streaming their renders from disk, their regions resting; a seek, a
/// loop (its start kept ready), an unfreeze (the chain back, the render let go — never the
/// last holder: the old snapshot still is), a freeze again, a stop. Not one allocation, not
/// one free.
#[test]
fn frozen_playback_never_allocates() {
    use cantus_core::freeze::{self, FreezeTarget};
    let dir = tempfile::tempdir().unwrap();
    let mut p = demo_project();
    let mut delay = p.make_effect(EffectKind::Delay);
    delay.spec.set("feedback", 0.6).unwrap();
    let s0 = p.tracks[0].strip.unwrap();
    p.strip_mut(s0).unwrap().effects.push(delay);
    // Keyed from the bass's strip: a desk solo of it thaws it (review 3).
    let s1 = p.tracks[1].strip.unwrap();
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.sidechain = Some(s1);
    p.strip_mut(s0).unwrap().effects.push(comp);
    let (mut h, _) = EngineHandle::manual(48_000.0);
    h.frozen.set_app_dir(dir.path().to_path_buf());
    let bass = p.tracks[1].id;
    for target in [FreezeTarget::Strip(s0), FreezeTarget::Track(bass)] {
        let job = cantus_engine::freeze::render::prepare(&mut h, &p, target).unwrap();
        job.run(Vec::new(), Vec::new(), &RenderProgress::default())
            .unwrap();
        h.frozen.forget_missing();
        freeze::freeze_commands(&p, target)
            .unwrap()
            .apply(&mut p)
            .unwrap();
    }
    let frozen = h.compile(&p, 1);
    assert!(frozen
        .strips
        .iter()
        .any(|s| s.frozen.as_ref().is_some_and(|f| f.plays())));
    assert!(frozen
        .tracks
        .iter()
        .any(|t| t.frozen.as_ref().is_some_and(|f| f.plays())));
    let mut live = p.clone();
    freeze::thaw(&mut live);
    // The review-3 states, compiled here, off the audio thread: the strip silenced by a
    // solo elsewhere, played live under its own (its render kept at the playhead), the
    // bass running for its notes only and resting with its render, a track's kept delay.
    let mut silenced = p.clone();
    silenced.strips[3].soloed = true;
    let silenced = h.compile(&silenced, 3);
    let mut solo = p.clone();
    solo.strip_mut(s0).unwrap().soloed = true;
    let parked = h.compile(&solo, 4);
    assert!(parked
        .strips
        .iter()
        .any(|s| s.frozen.as_ref().is_some_and(|f| f.live)));
    let mut notes = (*frozen).clone();
    notes.tracks[1].notes_only = true;
    let notes = Arc::new(notes);
    let mut rests = (*frozen).clone();
    rests.tracks[1].rest = true;
    let rests = Arc::new(rests);
    let mut late = p.clone();
    freeze::thaw(&mut late);
    late.tracks[0].lateness = Some(Lateness {
        frames: 3_000,
        rate: 48_000,
    });
    let late = h.compile(&late, 5);
    let live = h.compile(&live, 2);
    for (name, workers) in [("serial", None), ("pool", Some(Workers::parallel(3)))] {
        let mut rig = Rig::with(workers);
        rig.send(frozen.clone());
        rig.tx.push(EngineMessage::Play).unwrap();
        rig.play("warm", 4);
        rig.allocations.clear();
        rig.play("frozen", 200);
        rig.tx.push(EngineMessage::Seek(Ticks::beats(2))).unwrap();
        rig.play("seek", 40);
        let mut looped = (*frozen).clone();
        looped.loop_region = Some(LoopRegion {
            start: Ticks::beats(1),
            end: Ticks::beats(3),
        });
        rig.send(Arc::new(looped));
        rig.play("loop", 200);
        rig.send(live.clone());
        rig.play("unfrozen", 40);
        rig.send(frozen.clone());
        rig.play("frozen again", 40);
        // Review 3: Render, Silenced, Live (parked) and back, while playing.
        for _ in 0..2 {
            rig.send(silenced.clone());
            rig.play("silenced", 20);
            rig.send(parked.clone());
            rig.play("live, render kept", 20);
            rig.send(frozen.clone());
            rig.play("render again", 20);
        }
        rig.send(notes.clone());
        rig.play("notes only", 20);
        rig.send(rests.clone());
        rig.play("rests with its render", 20);
        rig.send(frozen.clone());
        rig.play("frozen once more", 20);
        rig.send(late.clone());
        rig.play("a kept delay", 40);
        rig.tx.push(EngineMessage::Stop).unwrap();
        rig.play("stop", 10);
        assert!(
            rig.allocations.iter().all(|(_, n)| *n == 0),
            "frozen playback allocated or freed ({name}): {:?} (the first: {} B)",
            rig.allocations,
            FIRST.with(|f| f.get())
        );
    }
}

/// Low-latency monitoring (round 29, ADR 0064 §4) on the audio thread and the pool: an
/// armed track monitored through a strip with a latent insert while another strip and
/// the master have theirs — the mode switched on (the path re-planned, the inserts over
/// the limit bypassed hard, their lines given back), played, recording started and
/// stopped, switched off and on again. Not one allocation, not one free.
#[test]
fn low_latency_monitoring_never_allocates() {
    let mut p = demo_project();
    let s0 = p.tracks[0].strip.unwrap();
    let s1 = p.tracks[1].strip.unwrap();
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::Auto;
    p.tracks[0].input = Some(input);
    p.tracks[0].armed = true;
    for s in [s0, s1] {
        let eq = eq_slot(&mut p, &[("mode", 2.0), ("b0.used", 1.0), ("b0.gain", 3.0)]);
        p.strip_mut(s).unwrap().effects.push(eq);
    }
    let mut lim = p.make_effect(EffectKind::Limiter);
    lim.spec.set("look", 5.0).unwrap();
    p.master.effects.push(lim);
    let mut without_eq = p.clone();
    without_eq.strip_mut(s1).unwrap().effects.clear();
    let rec_track = p.tracks[0].id;
    for (name, workers) in [("serial", None), ("pool", Some(Workers::parallel(3)))] {
        // The take's ring, made here (it would be the writer's).
        let (rec_ring, _rec_reader) = rtrb::RingBuffer::<f32>::new(48_000 * 4);
        let mut rig = Rig::with(workers);
        rig.snapshot(&p);
        rig.tx.push(EngineMessage::Play).unwrap();
        rig.play("warm", 20);
        rig.allocations.clear();
        rig.tx
            .push(EngineMessage::SetLowLatency(Some(144)))
            .unwrap();
        rig.play("low latency on", 40);
        rig.tx.push(EngineMessage::SetRecording(true)).unwrap();
        rig.play("recording", 40);
        // Review 3: the heard latency moved while a pass records (the mode switched, a
        // latent insert gone and back), the take moved a round trip later — into rings.
        rig.tx
            .push(EngineMessage::AttachRecorder(Box::new(RecordRings {
                tracks: vec![cantus_engine::recorder::TrackRing::new(
                    rec_track, 1, rec_ring,
                )],
            })))
            .unwrap();
        rig.tx.push(EngineMessage::SetDeviceRoundTrip(600)).unwrap();
        rig.play("recording into rings", 20);
        rig.tx.push(EngineMessage::SetLowLatency(None)).unwrap();
        rig.play("mode off while recording", 20);
        rig.tx
            .push(EngineMessage::SetLowLatency(Some(144)))
            .unwrap();
        rig.play("mode on while recording", 20);
        rig.snapshot(&without_eq);
        rig.play("a latent insert gone while recording", 20);
        rig.snapshot(&p);
        rig.play("and back", 20);
        rig.tx.push(EngineMessage::SetRecording(false)).unwrap();
        rig.tx.push(EngineMessage::SetLowLatency(None)).unwrap();
        rig.play("off", 40);
        rig.tx
            .push(EngineMessage::SetLowLatency(Some(144)))
            .unwrap();
        rig.play("on again", 40);
        rig.tx.push(EngineMessage::Stop).unwrap();
        rig.play("stop", 10);
        assert!(
            rig.allocations.iter().all(|(_, n)| *n == 0),
            "low-latency monitoring allocated or freed ({name}): {:?} (the first: {} B)",
            rig.allocations,
            FIRST.with(|f| f.get())
        );
    }
}

/// Round 29 (ADR 0062): what live MIDI now routes that it used to drop — channel
/// pressure, key pressure on a held key, program changes — on a native instrument whose
/// unit reads pressure, while it plays: not one allocation.
#[test]
fn live_pressure_and_program_changes_never_allocate() {
    let mut p = Project::new("live");
    let mut tr = p.make_track("Wave");
    tr.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable));
    let track = tr.id;
    p.tracks.push(tr);
    let mut rig = Rig::new();
    let snap = Snapshot::compile(&p, 1);
    for unit in cantus_engine::inst::units_for(&snap, 48_000.0, |_, _| false) {
        rig.tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    rig.snapshot(&p);
    rig.tx
        .push(EngineMessage::SetLiveTrack(Some(track)))
        .unwrap();
    rig.play("warm", 4);
    rig.allocations.clear();
    let ev = |status: u8, data1: u8, data2: u8| {
        EngineMessage::Midi(cantus_engine::MidiEvent {
            status,
            data1,
            data2,
        })
    };
    for i in 0..40u8 {
        rig.tx.push(ev(0x90, 48 + i % 12, 100)).unwrap();
        rig.tx.push(ev(0xD0, i * 3, 0)).unwrap();
        rig.tx.push(ev(0xA0, 48 + i % 12, i * 2)).unwrap();
        rig.tx.push(ev(0xC0, i % 8, 0)).unwrap();
        rig.play("live pressure and programs", 1);
        rig.tx.push(ev(0x80, 48 + i % 12, 0)).unwrap();
    }
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "live MIDI allocated: {:?}",
        rig.allocations
    );
}

/// Review 1 of ADR 0062, #13: what a MIDI port's callback runs for a mapped message — the
/// filter, the interface's ring, the wake (a flag and an unpark: never egui's lock) — and
/// for a release it must still hand the engine: not one allocation.
#[test]
fn the_midi_callback_never_allocates() {
    use cantus_engine::midi::{route, FilterSet, Heard, MidiTap, PortState};
    let woken = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let w = woken.clone();
    let tap = MidiTap::new(Some(std::sync::Arc::new(move || {
        w.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    })));
    let mut set = FilterSet::default();
    set.take(0, None, cantus_core::controller::Control::Cc(74));
    tap.filter().publish(&set);
    let (p, mut engine) = rtrb::RingBuffer::<cantus_engine::MidiEvent>::new(256);
    let sender = cantus_engine::MidiSender::new(p);
    let (mut ui, mut heard) = rtrb::RingBuffer::<Heard>::new(256);
    let mut held = PortState::default();
    let ev = |status: u8, data1: u8, data2: u8| cantus_engine::MidiEvent {
        status,
        data1,
        data2,
    };
    // macOS meets a thread's first message once: GitHub's Mac runner counted one 64-byte
    // block in round 0 alone (the system's first use of the waker's unpark or of a
    // thread-local on this thread; the port's thread, never the audio thread). So there
    // the steady state is what is held to zero: one round first, uncounted.
    if cfg!(target_os = "macos") {
        route(ev(0xB0, 74, 0), 0, &tap, &sender, &mut ui, &mut held);
        route(ev(0x80, 60, 0), 0, &tap, &sender, &mut ui, &mut held);
        while engine.pop().is_ok() {}
        while heard.pop().is_ok() {}
    }
    for i in 0..50u8 {
        let ((), n) = armed_here(|| {
            route(ev(0xB0, 74, i), 0, &tap, &sender, &mut ui, &mut held);
            route(ev(0x90, 60, 100), 0, &tap, &sender, &mut ui, &mut held);
            tap.filter().set_learning(true);
            route(ev(0x80, 60, 0), 0, &tap, &sender, &mut ui, &mut held);
            tap.filter().set_learning(false);
        });
        assert_eq!(
            n,
            0,
            "the callback allocated (round {i}; the first block: {} bytes)",
            FIRST.with(|f| f.get())
        );
        while engine.pop().is_ok() {}
        while heard.pop().is_ok() {}
        tap.rearm();
    }
}

/// Warped playback (round 29's third review, ADR 0061 §12) on the audio thread and the
/// pool: audio clips playing renders the sample cache laid (`zero_at`), forwards and
/// reversed, their data swapped while they play (the crossfade armed, carried by the next
/// install, done), a seek, a loop's wraps and a swap inside the loop, a stop. Not one
/// allocation, not one free.
#[test]
fn warped_playback_never_allocates() {
    use cantus_core::snapshot::FadeFrom;
    let mut p = demo_project();
    let source = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: source,
        name: "take".into(),
        path: "C:/nowhere/take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 8,
        sha256: None,
        recorded: false,
    });
    let mut t = p.make_track("Take");
    for (k, reverse) in [(0i64, false), (1, true)] {
        let mut c = p.make_audio_clip("take", source, Ticks::beats(8 * k), Ticks::beats(8));
        c.audio.as_mut().unwrap().reverse = reverse;
        t.clips.push(c);
    }
    p.tracks.push(t);
    let tone = |sign: f32| {
        let frames: Vec<f32> = (0..48_000 * 8)
            .map(|i| sign * 0.3 * math::sin(i as f32 * 0.03))
            .collect();
        Arc::new(SampleData {
            stream: None,
            rate: 48_000,
            channels: 1,
            frames: frames.into(),
        })
    };
    let (a, b) = (tone(1.0), tone(-1.0));
    let snap = |data: &Arc<SampleData>, from: Option<(&Arc<SampleData>, u64)>, looped: bool| {
        let mut s = Snapshot::compile(&p, 1);
        let ti = s.tracks.len() - 1;
        let mut swaps = Vec::new();
        for (ci, c) in s.tracks[ti].audio.iter_mut().enumerate() {
            c.data = Some(data.clone());
            c.zero_at = Some((c.start, 0.0));
            if let Some((d, id)) = from {
                c.fade_from = Some(FadeFrom {
                    data: d.clone(),
                    id: id + ci as u64,
                });
                swaps.push((ti, ci));
            }
        }
        s.swaps = swaps;
        if looped {
            s.loop_region = Some(LoopRegion {
                start: Ticks::beats(8),
                end: Ticks::beats(10),
            });
        }
        Arc::new(s)
    };
    for (name, workers) in [("serial", None), ("pool", Some(Workers::parallel(3)))] {
        let mut rig = Rig::with(workers);
        rig.send(snap(&a, None, false));
        rig.tx.push(EngineMessage::Play).unwrap();
        rig.play("warm", 4);
        rig.allocations.clear();
        rig.play("render", 200);
        rig.send(snap(&b, Some((&a, 1)), false));
        rig.play("swap", 10);
        rig.send(snap(&b, Some((&a, 1)), false));
        rig.play("carried", 10);
        rig.tx.push(EngineMessage::Seek(Ticks::beats(8))).unwrap();
        rig.play("seek to the reversed", 100);
        rig.send(snap(&b, None, true));
        rig.play("loop", 200);
        rig.send(snap(&a, Some((&b, 10)), true));
        rig.play("swap in the loop", 200);
        rig.tx.push(EngineMessage::Stop).unwrap();
        rig.play("stop", 10);
        assert!(
            rig.allocations.iter().all(|(_, n)| *n == 0),
            "warped playback allocated or freed ({name}): {:?} (the first: {} B)",
            rig.allocations,
            FIRST.with(|f| f.get())
        );
    }
}

/// A note clip's controller lanes and its notes' release velocities (ADR 0068) on the audio
/// thread: bend, the mod wheel and pressure fired at their samples through a loop's wraps
/// (each wrap chases), a new snapshot mid-play, a seek (rest, then chase) and a stop (rest).
/// Not one allocation.
#[test]
fn controller_lanes_never_allocate() {
    use cantus_core::clip_lanes::{ClipLane, LaneKind};
    let mut p = demo_project();
    p.tempo_bpm = 140.0;
    let ramp = |a: f32, b: f32| {
        vec![
            AutoPoint::new(Ticks::ZERO, a),
            AutoPoint::new(Ticks::beats(2), b),
        ]
    };
    for t in &mut p.tracks {
        for c in t
            .clips
            .iter_mut()
            .filter(|c| c.audio.is_none() && c.automation.is_none())
        {
            c.lanes = vec![
                ClipLane::new(LaneKind::PitchBend, ramp(0.5, 0.9)),
                ClipLane::new(LaneKind::Cc(1), ramp(0.0, 1.0)),
                ClipLane::new(LaneKind::Pressure, ramp(0.3, 0.6)),
            ];
            for (i, n) in c.notes.iter_mut().enumerate() {
                n.release = 1 + ((i * 37) % 127) as u8;
            }
        }
    }
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(1),
        end: Ticks::beats(3),
    });
    p.loop_enabled = true;
    let mut rig = Rig::new();
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("lanes start and wraps", 60);
    p.tracks[0].clips[0].lanes[0].points[1].value = 0.1;
    rig.snapshot(&p);
    rig.play("lanes new snapshot", 20);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(2))).unwrap();
    rig.play("lanes seek", 10);
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("lanes stop", 4);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

/// Round 31 (ADR 0069): note effects on the audio thread — a chain installed and changed
/// (parameters in place, a stage added: the shape changes and it starts afresh), clip chords
/// into an arpeggio with a scale after it while playing, a loop's wraps, a seek, live keys
/// through it while stopped (the arpeggio on the sample clock), its parameter automated
/// and modulated. Not one allocation while processing.
#[test]
fn note_effects_never_allocate() {
    use cantus_core::automation::AutoPoint;
    use cantus_core::notefx::{ArpMode, NoteEffect, NoteFxId, NoteFxKind, NoteFxType};
    let mut p = Project::new("notefx");
    p.key = Some(Key::new(2, ScaleKind::Minor));
    let mut tr = p.make_track("Lead");
    let track = tr.id;
    let mut c = p.make_clip("chords", Ticks::ZERO, Ticks::beats(8));
    for (i, pitch) in [62u8, 65, 69, 67, 70, 74].into_iter().enumerate() {
        let start = Ticks::beats(4 * (i as i64 / 3));
        c.notes.push(p.make_note(start, Ticks::beats(4), pitch, 90));
    }
    tr.clips.push(c);
    let chord = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Chord);
    let arp = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Arp);
    let arp_id = arp.id;
    tr.note_fx = vec![chord, arp];
    p.tracks.push(tr);
    let gate = AutoTarget::NoteFx {
        track,
        effect: arp_id,
        key: "gate".into(),
    };
    let ride = p.make_automation_clip(
        "gate",
        gate.clone(),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.1),
            AutoPoint::new(Ticks::beats(8), 0.9),
        ],
    );
    p.tracks[0].clips.push(ride);
    let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    lfo.routes = vec![ModRoute {
        target: AutoTarget::NoteFx {
            track,
            effect: arp_id,
            key: "octaves".into(),
        },
        depth: 0.5,
    }];
    p.tracks[0].modulators.push(lfo);
    p.loop_region = Some(LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(2),
    });
    p.loop_enabled = true;
    let mut rig = Rig::new();
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("warm", 4);
    rig.allocations.clear();
    rig.play("an arpeggio of chords, looping", 60);
    // A parameter changed (in place), then a stage added (afresh).
    if let NoteFxKind::Arp(a) = &mut p.tracks[0].note_fx[1].kind {
        a.mode = ArpMode::Random;
    }
    rig.snapshot(&p);
    rig.play("a parameter changed", 8);
    let scale = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Scale);
    p.tracks[0].note_fx.push(scale);
    rig.snapshot(&p);
    rig.play("a stage added", 8);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(1))).unwrap();
    rig.play("a seek", 8);
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("stopped", 2);
    for i in 0..12u8 {
        rig.tx
            .push(EngineMessage::NoteOn {
                track,
                pitch: 50 + i,
                velocity: 100,
            })
            .unwrap();
        rig.play("live keys while stopped", 3);
        rig.tx
            .push(EngineMessage::NoteOff {
                track,
                pitch: 50 + i,
            })
            .unwrap();
    }
    rig.play("stopped, after", 4);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "note effects allocated: {:?}",
        rig.allocations
    );
}

/// Round 31 (ADR 0070): overlap crossfades and clip gain envelopes on the audio thread —
/// two clips crossing over an overlap (one reversed), both with a gain line of several
/// shapes, played through, sought into the overlap and looped across it. Not one
/// allocation, not one free.
#[test]
fn crossfades_and_clip_envelopes_never_allocate() {
    use cantus_core::clip_gain::{GainEnvelope, GainPoint};
    let mut p = demo_project();
    let source = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: source,
        name: "take".into(),
        path: "C:/nowhere/take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 16,
        sha256: None,
        recorded: false,
    });
    let mut t = p.make_track("Take");
    for (k, reverse) in [(0i64, false), (1, true)] {
        let mut c = p.make_audio_clip("take", source, Ticks::beats(6 * k), Ticks::beats(8));
        let a = c.audio.as_mut().unwrap();
        a.reverse = reverse;
        a.cross = Some(cantus_core::crossfade::XfadeCurve::SCurve);
        let mut pts: Vec<GainPoint> = (0..32)
            .map(|i| GainPoint::new(Ticks::beats(i) / 4, if i % 2 == 0 { -6.0 } else { 3.0 }))
            .collect();
        pts[3].shape = Shape::SCurve;
        pts[5].shape = Shape::Hold;
        pts[7].tension = 0.6;
        a.gain_env = Some(Box::new(GainEnvelope::new(pts)));
        t.clips.push(c);
    }
    p.tracks.push(t);
    let frames: Vec<f32> = (0..48_000 * 16)
        .map(|i| 0.3 * math::sin(i as f32 * 0.03))
        .collect();
    let data = Arc::new(SampleData {
        stream: None,
        rate: 48_000,
        channels: 1,
        frames: frames.into(),
    });
    let snap = |looped: bool| {
        let mut s = Snapshot::compile(&p, 1);
        for c in s.tracks.last_mut().unwrap().audio.iter_mut() {
            c.data = Some(data.clone());
        }
        if looped {
            s.loop_region = Some(LoopRegion {
                start: Ticks::beats(5),
                end: Ticks::beats(9),
            });
        }
        Arc::new(s)
    };
    for (name, workers) in [("serial", None), ("pool", Some(Workers::parallel(3)))] {
        let mut rig = Rig::with(workers);
        rig.send(snap(false));
        rig.tx.push(EngineMessage::Play).unwrap();
        rig.play("warm", 4);
        rig.allocations.clear();
        rig.play("through", 300);
        rig.tx.push(EngineMessage::Seek(Ticks::beats(7))).unwrap();
        rig.play("seek into the overlap", 100);
        rig.send(snap(true));
        rig.play("loop across it", 300);
        rig.tx.push(EngineMessage::Stop).unwrap();
        rig.play("stop", 10);
        assert!(
            rig.allocations.iter().all(|(_, n)| *n == 0),
            "crossfades or envelopes allocated or freed ({name}): {:?} (the first: {} B)",
            rig.allocations,
            FIRST.with(|f| f.get())
        );
    }
}

/// The session (ADR 0076 §7): a performance played as projections installed while the
/// transport plays — blocks of 441 frames, so every install waits for the automation's
/// grid inside a block — a launch, a follow action, an envelope, a stop. The audio thread
/// runs nothing of the session itself: not one allocation.
#[test]
fn the_session_never_allocates() {
    use cantus_core::session::play::{plan, project_play, Input, Timed};
    use cantus_core::session::*;
    let mut p = demo_project();
    let lead = p.tracks[0].id;
    let scene = SceneId(p.alloc_id());
    p.session.scenes.push(Scene::new(scene, "A"));
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    c.content_length = Some(Ticks::beats(4));
    for i in 0..8 {
        let n = p.make_note(Ticks(i * 480), Ticks(300), 60 + i as u8, 100);
        c.notes.push(n);
    }
    let mut sc = SessionClip::new(c);
    sc.follow = FollowActions {
        on: true,
        a: FollowAction::Again,
        b: FollowAction::Stop,
        chance_a: 80,
        after: FollowAfter::Linked,
        count: 1,
    };
    sc.envelopes.push(Automation::new(
        AutoTarget::TrackGain(lead),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.3),
            AutoPoint::new(Ticks::beats(4), 0.9),
        ],
    ));
    p.tracks[0].slots.push(Slot {
        scene,
        clip: Some(sc),
        stop: true,
    });
    let slot = SlotRef::new(lead, scene);
    let press = |at: Ticks, input| Timed::at(at, input);
    let inputs = [
        press(
            Ticks(10),
            Input::Launch {
                slot,
                velocity: 1.0,
            },
        ),
        press(Ticks::beats(10), Input::StopTrack { track: lead }),
    ];
    let mut rig = Rig::new();
    rig.buf = vec![0.0; 441 * 2];
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("session start", 4);
    for (i, h) in [8, 12, 16, 20].iter().enumerate() {
        let known = &inputs[..(i + 1).min(inputs.len())];
        let perf = plan(&p, known, Ticks::beats(*h)).perf;
        rig.snapshot(&project_play(&p, &perf));
        rig.play("session projection", 20);
    }
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("session stop", 4);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

/// Round 33 (ADR 0080): per-note expression on the audio thread — clip curves on every
/// note of four listening instruments through a loop's wraps, a new snapshot and a seek;
/// then a live MPE controller (the MCM, member notes with their bends, pressure and CC 74,
/// a bend range RPN) while recording. Not one allocation.
#[test]
fn per_note_expression_never_allocates() {
    use cantus_core::model::note_expr::zones::MpeZones;
    use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
    let mut p = Project::new("mpe");
    p.tempo_bpm = 140.0;
    let st = |x: f64| ExprKind::Bend.store(x);
    let mut live = None;
    for kind in [
        InstrumentKind::Wavetable,
        InstrumentKind::Analog,
        InstrumentKind::Fm,
        InstrumentKind::Bass,
    ] {
        let mut t = p.make_track("t");
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        live.get_or_insert(t.id);
        let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        for i in 0..4 {
            let mut n = p.make_note(Ticks::beats(i), Ticks::beats(1), 48 + i as u8 * 3, 100);
            n.expr = NoteExpr {
                bend: (0..20)
                    .map(|k| ExprPoint(k * 48, st((k % 5) as f64 * 0.4)))
                    .collect(),
                pressure: vec![
                    ExprPoint(0, 0),
                    ExprPoint(900, ExprKind::Pressure.store(1.0)),
                ],
                timbre: vec![ExprPoint(0, ExprKind::Timbre.store(0.2))],
            }
            .boxed();
            c.notes.push(n);
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(1),
        end: Ticks::beats(3),
    });
    p.loop_enabled = true;
    let mut rig = Rig::new();
    let snap = Snapshot::compile(&p, 1);
    for unit in cantus_engine::inst::units_for(&snap, 48_000.0, |_, _| false) {
        rig.tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("expression start and wraps", 60);
    p.tracks[0].clips[0].notes[0].expr = None;
    rig.snapshot(&p);
    rig.play("expression new snapshot", 20);
    rig.tx.push(EngineMessage::Seek(Ticks::beats(2))).unwrap();
    rig.play("expression seek", 10);
    // Live MPE, recording.
    rig.tx.push(EngineMessage::SetLiveTrack(live)).unwrap();
    rig.tx
        .push(EngineMessage::SetMpe(Some(MpeZones::standard())))
        .unwrap();
    rig.tx.push(EngineMessage::SetRecording(true)).unwrap();
    rig.play("mpe on", 2);
    let ev = |status: u8, data1: u8, data2: u8| {
        EngineMessage::Midi(cantus_engine::MidiEvent {
            status,
            data1,
            data2,
        })
    };
    for (s, d1, d2) in [(0xB0, 101, 0), (0xB0, 100, 6), (0xB0, 6, 15)] {
        rig.tx.push(ev(s, d1, d2)).unwrap();
    }
    for i in 0..30u8 {
        let ch = 1 + i % 15;
        rig.tx.push(ev(0xE0 | ch, 0, 64 + i)).unwrap();
        rig.tx.push(ev(0xB0 | ch, 74, i * 4)).unwrap();
        rig.tx.push(ev(0x90 | ch, 48 + i % 24, 100)).unwrap();
        rig.tx.push(ev(0xD0 | ch, i * 4, 0)).unwrap();
        rig.tx.push(ev(0xB0 | ch, 101, 0)).unwrap();
        rig.tx.push(ev(0xB0 | ch, 100, 0)).unwrap();
        rig.tx.push(ev(0xB0 | ch, 6, 24 + i % 24)).unwrap();
        rig.play("mpe notes", 1);
        rig.tx.push(ev(0x80 | ch, 48 + i % 24, 30)).unwrap();
    }
    rig.play("mpe release", 2);
    rig.tx.push(EngineMessage::SetMpe(None)).unwrap();
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.play("mpe off and stop", 4);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?} (the first of the last phase: {} B)",
        rig.allocations,
        FIRST.with(|f| f.get())
    );
}

// ---- round 33, sync (ADR 0078–0079) ----------------------------------------------------

/// A clock that moves one 512-frame block each time the audio thread reads it.
static SYNC_NOW: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1_000_000);
fn sync_now() -> i64 {
    SYNC_NOW.fetch_add(10_667, std::sync::atomic::Ordering::Relaxed)
}

/// Following and making clock (round 33): a Link session joined in phase and followed
/// through a tempo change (retunes, new stretches), MIDI clock and time code out with a
/// loop's wraps, then a MIDI clock source, then none (unfollow) — on the audio thread and
/// the pool. Not one allocation, not one free.
#[test]
fn following_and_clock_out_never_allocate() {
    use cantus_engine::transport::sync::{self, SourceKind, Timeline};
    for workers in [None, Some(Workers::parallel(3))] {
        let mut p = demo_project();
        p.loop_region = Some(LoopRegion {
            start: Ticks::beats(1),
            end: Ticks::beats(5),
        });
        p.loop_enabled = true;
        let mut rig = Rig::with(workers);
        let (port, handle) = sync::pair();
        rig.p.attach_sync_clocked(port, None, sync_now);
        handle
            .shared
            .out
            .clock
            .store(true, std::sync::atomic::Ordering::Relaxed);
        handle
            .shared
            .out
            .mtc
            .store(true, std::sync::atomic::Ordering::Relaxed);
        rig.snapshot(&p);
        let mut tl = Timeline {
            kind: SourceKind::Link,
            tempo: 120.0,
            host: 0,
            position: 0.0,
            rate: 1.0,
            playing: true,
            quantum: 4.0,
            start_stop: true,
            epoch: 1,
        };
        handle.shared.timeline.publish(&tl);
        rig.play("sync: Link joined and followed", 400);
        tl.tempo = 133.0;
        tl.host = SYNC_NOW.load(std::sync::atomic::Ordering::Relaxed);
        tl.position = 77.0;
        handle.shared.timeline.publish(&tl);
        rig.play("sync: a tempo change and a jump", 200);
        rig.tx.push(EngineMessage::Stop).unwrap();
        rig.play("sync: our stop (a proposal)", 20);
        tl.kind = SourceKind::Clock;
        tl.quantum = 0.0;
        tl.host = SYNC_NOW.load(std::sync::atomic::Ordering::Relaxed);
        handle.shared.timeline.publish(&tl);
        rig.play("sync: a MIDI clock followed", 200);
        handle.shared.timeline.publish(&Timeline::default());
        rig.play("sync: none (unfollowed)", 40);
        assert!(
            rig.allocations.iter().all(|(_, n)| *n == 0),
            "sync allocated or freed: {:?} (the first of the last phase: {} B)",
            rig.allocations,
            FIRST.with(|f| f.get())
        );
        drop(handle);
    }
}

/// Review finding 10: snapshots installed inside a block (they waited for the automation's
/// grid) while another thread drains and drops the garbage as fast as it comes — as the
/// interface does — never leave the old snapshot's last reference to the audio thread.
#[test]
fn a_deferred_install_never_frees_on_the_audio_thread() {
    let mut p = demo_project();
    let lead = p.tracks[0].id;
    let c = p.make_automation_clip(
        "a",
        AutoTarget::TrackGain(lead),
        Ticks::ZERO,
        Ticks::beats(64),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.2),
            AutoPoint::new(Ticks::beats(64), 0.9),
        ],
    );
    p.tracks[0].clips.push(c);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let s2 = stop.clone();
    let drain = std::thread::spawn(move || {
        while !s2.load(std::sync::atomic::Ordering::Relaxed) {
            while let Ok(ev) = rx.pop() {
                drop(ev);
            }
            std::hint::spin_loop();
        }
    });
    let send = |tx: &mut rtrb::Producer<EngineMessage>, p: &Project, rev: u64| {
        let snap = Arc::new(Snapshot::compile(p, rev));
        tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .unwrap();
        tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    };
    send(&mut tx, &p, 1);
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0f32; 441 * 2];
    proc.process(&mut buf, 2);
    let mut events = 0;
    for rev in 2..60 {
        send(&mut tx, &p, rev);
        for _ in 0..3 {
            let ((), n) = armed(|| proc.process(&mut buf, 2));
            events += n;
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    drain.join().unwrap();
    assert_eq!(events, 0, "the audio thread allocated or freed");
}

/// N3: an install while playing that ends notes cut behind the playhead walks the list the
/// snapshot carries (`Snapshot::cut_from`, worked out off the audio thread): not one
/// allocation.
#[test]
fn an_install_that_ends_cut_notes_never_allocates() {
    let mut p = demo_project();
    let mut c = p.make_clip("long", Ticks::ZERO, Ticks::beats(16));
    for k in 0..8u8 {
        let n = p.make_note(Ticks::ZERO, Ticks::beats(16), 48 + k, 100);
        c.notes.push(n);
    }
    p.tracks[0].clips.push(c);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let old = Arc::new(Snapshot::compile(&p, 1));
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&old)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(old.clone())).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0f32; 480 * 2];
    // A second in: beat 2.
    for _ in 0..100 {
        proc.process(&mut buf, 2);
    }
    let last = p.tracks[0].clips.len() - 1;
    p.tracks[0].clips[last].length = Ticks::beats(1);
    let mut new = Snapshot::compile(&p, 2);
    new.cut_from(&old, Ticks::beats(2));
    assert_eq!(new.cuts.len(), 8);
    let new = Arc::new(new);
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&new)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(new)).unwrap();
    let mut events = 0;
    for _ in 0..4 {
        let ((), n) = armed(|| proc.process(&mut buf, 2));
        events += n;
    }
    drop(old);
    assert_eq!(events, 0, "the audio thread allocated or freed");
}
