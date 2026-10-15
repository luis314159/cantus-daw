//! What the modulation family's engine tests share (ADR 0055): `#[path =
//! "support/modfx.rs"] mod modfx;` beside `mod support;`.
#![allow(dead_code)]

use super::support::*;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

/// Set one parameter of the effect `fx` on the lead's strip.
pub fn set(p: &mut Project, fx: EffectId, key: &str, v: f32) {
    let s = strip_of(p, 0);
    let slot = s.effects.iter_mut().find(|e| e.id == fx).unwrap();
    assert!(slot.spec.set(key, v).is_some(), "{key} is a parameter");
}

/// Set several.
pub fn set_all(p: &mut Project, fx: EffectId, kv: &[(&str, f32)]) {
    for (k, v) in kv {
        set(p, fx, k, *v);
    }
}

/// The demo with only its lead (no bass under it).
pub fn lead_only() -> Project {
    let mut p = demo_project();
    p.tracks.truncate(1);
    p
}

/// Render offline from `start`, 48 kHz stereo, a quarter-second tail.
pub fn render_from(project: &Project, start: Ticks, block_frames: usize) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames,
        start,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(project, 1)), &opts)
        .unwrap()
        .samples
}

/// Drive a processor as the audio callback does — irregular blocks, the events drained —
/// for as many samples as `len`: what playing the project live sounds like.
pub fn realtime(project: &Project, len: usize) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let mut snap = Snapshot::compile(project, 1);
    // The units come from the UI thread, as the backend sends them.
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, 48_000.0);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    snap.loop_region = None;
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(len);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() < len {
        let n = sizes[i % sizes.len()].min((len - out.len()) / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

/// Samples per beat of the demo (128 BPM at 48 kHz).
pub const BEAT: f64 = 60.0 / 128.0 * 48_000.0;

/// As [`realtime`], but the processor runs stopped for `idle` frames first — as an app
/// does between building a unit and the press of Play — and only what follows Play is
/// returned.
pub fn realtime_after_idle(project: &Project, len: usize, idle: usize) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let mut snap = Snapshot::compile(project, 1);
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, 48_000.0);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    snap.loop_region = None;
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    let mut done = 0;
    while done < idle {
        let n = 441.min(idle - done);
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        while rx.pop().is_ok() {}
        done += n;
    }
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(len);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() < len {
        let n = sizes[i % sizes.len()].min((len - out.len()) / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}
