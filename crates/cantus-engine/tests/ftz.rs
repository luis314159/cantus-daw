//! Denormals inside `Processor::process` (ADR 0031 §5.2): every unit runs with the
//! hardware's flush-to-zero and denormals-are-zero set — so the callback, the offline
//! render, the stems, the bench and every test run in one floating-point mode — and the
//! caller gets its own mode back when `process` returns.
//!
//! The probe is an insert that reads the control register from inside its `process`.
//! (The whole null corpus — `null_tests.rs` and the `v5.cantus` hash — runs with the flags
//! on, since every render goes through `process`.)

#![cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#![allow(unsafe_code)]

use cantus_core::*;
use cantus_engine::fx::FxUnit;
use cantus_engine::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The bits that flush: x86-64 MXCSR's FTZ (15) and DAZ (6); AArch64 FPCR's FZ (24).
#[cfg(target_arch = "x86_64")]
const FLUSH: u64 = 0x8040;
#[cfg(target_arch = "aarch64")]
const FLUSH: u64 = 1 << 24;

#[cfg(target_arch = "x86_64")]
fn read_mode() -> u64 {
    let mut csr: u32 = 0;
    // SAFETY: stmxcsr stores the SSE control register into `csr`, nothing else.
    unsafe {
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
    }
    csr as u64
}

#[cfg(target_arch = "x86_64")]
fn write_mode(mode: u64) {
    let csr = mode as u32;
    // SAFETY: loads a valid MXCSR value (one read from the register, with mode bits
    // changed) into the SSE control register.
    unsafe {
        std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, preserves_flags));
    }
}

#[cfg(target_arch = "aarch64")]
fn read_mode() -> u64 {
    let fpcr: u64;
    // SAFETY: reads the floating-point control register.
    unsafe {
        std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags));
    }
    fpcr
}

#[cfg(target_arch = "aarch64")]
fn write_mode(mode: u64) {
    // SAFETY: writes a value read from FPCR, with mode bits changed, back to it.
    unsafe {
        std::arch::asm!("msr fpcr, {}", in(reg) mode, options(nomem, nostack, preserves_flags));
    }
}

/// An insert that records the floating-point mode it runs in, and leaves the audio alone.
#[derive(Debug)]
struct Probe(Arc<AtomicU64>);

impl FxUnit for Probe {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.0.store(read_mode() | 1 << 63, Ordering::Relaxed);
    }
    fn reset(&mut self) {}
}

/// A processor playing a project with the probe on a strip and on the master.
fn rig(seen: &Arc<AtomicU64>) -> (Processor, rtrb::Producer<EngineMessage>) {
    let mut p = Project::new("ftz");
    let mut t = p.make_track("t");
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    clip.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(2), 60, 100));
    t.clips.push(clip);
    let strip = t.strip.expect("a strip");
    p.tracks.push(t);
    let probe = p.make_effect(EffectKind::Gate);
    p.strip_mut(strip).unwrap().effects.push(probe);
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let (mut tx, rx) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (etx, _erx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc_ = Processor::without_midi(48_000.0, rx, etx);
    for slot in snap.effect_slots() {
        let unit = EffectUnit::from_fx(slot, Box::new(Probe(seen.clone())));
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    proc_.settle();
    (proc_, tx)
}

#[test]
fn process_runs_with_denormals_flushed_and_restores_the_callers_mode() {
    let seen = Arc::new(AtomicU64::new(0));
    let (mut p, _tx) = rig(&seen);
    let mut buf = vec![0.0f32; 256 * 2];
    let before = read_mode();
    for caller in [before & !FLUSH, before | FLUSH, before & !FLUSH] {
        write_mode(caller);
        seen.store(0, Ordering::Relaxed);
        p.process(&mut buf, 2);
        let inside = seen.load(Ordering::Relaxed);
        assert_ne!(inside, 0, "the probe ran");
        assert_eq!(
            inside & FLUSH,
            FLUSH,
            "inside process the flags are set (caller's mode {caller:#x}, inside {inside:#x})"
        );
        assert_eq!(read_mode(), caller, "the caller's mode is restored");
    }
    write_mode(before);
}

/// The output does not depend on the caller's floating-point mode: the probe's project,
/// rendered once from a caller with the flags off and once with them on, to the bit.
#[test]
fn the_callers_mode_does_not_change_the_output() {
    let render = |flags_on: bool| {
        let seen = Arc::new(AtomicU64::new(0));
        let (mut p, _tx) = rig(&seen);
        let before = read_mode();
        write_mode(if flags_on { before | FLUSH } else { before & !FLUSH });
        let mut out = Vec::new();
        let mut buf = vec![0.0f32; 128 * 2];
        for _ in 0..40 {
            p.process(&mut buf, 2);
            out.extend(buf.iter().map(|x| x.to_bits()));
        }
        write_mode(before);
        out
    };
    let a = render(false);
    let b = render(true);
    assert!(a.iter().any(|x| *x != 0), "the project plays");
    assert_eq!(a, b, "the output depends on the caller's floating-point mode");
}

/// Render `p` (every effect's unit built as the engine builds it) with the guard on or
/// off: the output's bits, block by block.
fn render_bits(p: &Project, flush: bool, blocks: usize) -> Vec<u32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (etx, mut erx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc_ = Processor::without_midi(48_000.0, rx, etx);
    proc_.set_flush_denormals(flush);
    for slot in snap.effect_slots() {
        let unit = EffectUnit::new(slot, 48_000.0);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    for unit in cantus_engine::inst::units_for(&snap, 48_000.0, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    proc_.settle();
    while let Ok(ev) = erx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(pdc::lines_for(count, delay)))
                .unwrap();
            proc_.settle();
        }
    }
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; 256 * 2];
    for _ in 0..blocks {
        proc_.process(&mut buf, 2);
        out.extend(buf.iter().map(|x| x.to_bits()));
        while erx.pop().is_ok() {}
    }
    out
}

/// The corpus with the guard on equals the corpus with it off, to the bit: the DSP never
/// relies on a subnormal (ADR 0031 §5.2, R9). A notes-and-strips project with every native
/// effect kind on its strips and the master, through the song and well into the tails.
#[test]
fn flushing_denormals_changes_no_sample_of_the_corpus() {
    let mut p = Project::new("ftz corpus");
    p.tempo_bpm = 128.0;
    for (i, kind) in EffectKind::ALL.iter().enumerate() {
        let mut t = p.make_track(format!("T{i}"));
        let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        for k in 0..8 {
            let pitch = 40 + (i as u8 * 5 + k * 7) % 40;
            clip.notes.push(p.make_note(
                Ticks::EIGHTH * k as i64,
                Ticks::EIGHTH,
                pitch,
                60 + k * 8,
            ));
        }
        t.clips.push(clip);
        let strip = t.strip.expect("a strip");
        p.tracks.push(t);
        let fx = p.make_effect(*kind);
        p.strip_mut(strip).unwrap().effects.push(fx);
    }
    for kind in [EffectKind::Eq, EffectKind::Maximizer, EffectKind::Limiter] {
        let fx = p.make_effect(kind);
        p.master.effects.push(fx);
    }
    // Two seconds of music, then six of tails decaying towards (and through) the
    // subnormal range.
    let blocks = 48_000 * 8 / 256;
    let on = render_bits(&p, true, blocks);
    let off = render_bits(&p, false, blocks);
    assert!(on.iter().any(|x| *x != 0), "the corpus plays");
    let first = on.iter().zip(&off).position(|(a, b)| a != b);
    assert_eq!(first, None, "FTZ/DAZ changed a sample (first at {first:?})");
}
