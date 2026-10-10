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

/// Voices on several threads (ADR 0038): chords on Calor, Unda and Exemplar with three
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
