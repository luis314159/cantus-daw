//! Latitudo (stereo control and mid/side) inside the engine: the null tests before
//! anything else, then block-size independence, real time against offline, and the
//! telemetry through the plug-in contract.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::latitudo::view::telemetry;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::sync::Arc;
use support::*;

fn set(p: &mut Project, effect: EffectId, key: &str, value: f32) {
    for s in &mut p.strips {
        if let Some(e) = s.effects.iter_mut().find(|e| e.id == effect) {
            e.spec.set(key, value).expect("a Latitudo parameter");
        }
    }
}

/// Every stage working at once.
const BUSY: &[(&str, f32)] = &[
    ("width", 1.4),
    ("side", 1.5),
    ("rot", -8.0),
    ("bal", 0.15),
    ("inv_r", 1.0),
    ("mono", 140.0),
    ("mono_slope", 3.0),
    ("w_lo", 0.4),
    ("w_hi", 1.7),
    ("m_lo", 2.0),
    ("s_hi", 3.5),
    ("tone_slope", 1.0),
    ("haas", 9.0),
    ("haas_mode", 2.0),
    ("out", -1.5),
];

fn busy_project() -> Project {
    let mut p = demo_project();
    let e = add_effect(&mut p, EffectKind::Stereo);
    for (k, v) in BUSY {
        set(&mut p, e, k, *v);
    }
    p
}

/// At its defaults Latitudo renders exactly as no insert at all.
#[test]
fn latitudo_at_its_defaults_renders_exactly_as_no_insert() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Stereo);
    assert_null(&dry, &render_of(&p), 0.0, "Latitudo at its defaults");
}

/// Neutral values of every stage — shelves at 0 dB wherever they sit, bands at 100 %
/// wherever the crossovers are, a Haas amount with no delay, other slopes — are a wire.
#[test]
fn neutral_values_of_every_stage_render_exactly_as_no_insert() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    let e = add_effect(&mut p, EffectKind::Stereo);
    for (k, v) in [
        ("x_lo", 100.0),
        ("x_hi", 9_000.0),
        ("m_lo_f", 600.0),
        ("s_hi_f", 2_500.0),
        ("tone_slope", 1.0),
        ("mono_slope", 0.0),
        ("haas_mode", 2.0),
        ("haas_amt", 1.0),
    ] {
        set(&mut p, e, k, v);
    }
    assert_null(&dry, &render_of(&p), 0.0, "neutral stages");
}

/// Swapped twice is not swapped: two inserts that each exchange left and right render as
/// none.
#[test]
fn two_swaps_render_as_none() {
    let dry = render_of(&demo_project());
    let mut p = demo_project();
    for _ in 0..2 {
        let e = add_effect(&mut p, EffectKind::Stereo);
        set(&mut p, e, "swap", 1.0);
    }
    assert_null(&dry, &render_of(&p), 0.0, "two swaps");
}

/// With every stage working, the render is the same at every block size.
#[test]
fn every_stage_on_does_not_depend_on_the_block_size() {
    let p = busy_project();
    let reference = render_with(&p, 512);
    assert!(reference.iter().all(|x| x.is_finite()));
    let dry = render_of(&demo_project());
    let moved = dry
        .iter()
        .zip(&reference)
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(moved > 0.01, "Latitudo did nothing: {moved}");
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("Latitudo at block {block}"),
        );
    }
}

/// Driven the way the audio callback drives it — uneven blocks — it renders what the
/// offline renderer does, to the bit.
#[test]
fn real_time_matches_the_offline_render() {
    let p = busy_project();
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 256,
        ..Default::default()
    };
    let offline = render(snap.clone(), &opts).unwrap();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    // The units come from the UI thread, as the backend sends them.
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, 48_000.0);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    let mut snap_rt = (*snap).clone();
    snap_rt.loop_region = None;
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap_rt)))
        .unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = offline.frames();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    assert_null(
        &offline.samples,
        &out,
        0.0,
        "Latitudo, real time against offline",
    );
}

/// Through the contract: no latency at any setting, and the telemetry reads the signal —
/// a centred tone is correlated, the same tone with one side inverted is not, and the
/// vectorscope's points are counted.
#[test]
fn latency_and_telemetry_through_the_contract() {
    let mut unit = cantus_engine::fx::make(EffectKind::Stereo, 48_000.0).unwrap();
    let mut spec = EffectSpec::new(EffectKind::Stereo);
    for (k, v) in BUSY {
        spec.set(k, *v);
    }
    unit.configure(&spec);
    unit.reset();
    assert_eq!(unit.latency(), 0, "Latitudo never delays");
    let tone: Vec<f32> = (0..48_000)
        .map(|i| 0.4 * math::sin(i as f32 * 0.05))
        .collect();
    let (mut l, mut r) = (tone.clone(), tone.clone());
    let spec = EffectSpec::new(EffectKind::Stereo);
    unit.configure(&spec);
    unit.reset();
    for (a, b) in l.chunks_mut(480).zip(r.chunks_mut(480)) {
        unit.process(a, b, None);
    }
    let t = unit.telemetry();
    assert!(
        (t[telemetry::CORR] - 1.0).abs() < 1e-3,
        "{}",
        t[telemetry::CORR]
    );
    assert_eq!(t[telemetry::SAMPLE_RATE], 48_000.0);
    // A point every `stride` samples, the first on sample 0.
    let stride = t[telemetry::STRIDE] as usize;
    assert_eq!(t[telemetry::SEQ], 48_000usize.div_ceil(stride) as f32);
    let mut inv = tone.clone();
    for v in &mut inv {
        *v = -*v;
    }
    let mut l = tone;
    // The meters integrate over 300 ms: start them afresh for the second signal.
    unit.reset();
    for (a, b) in l.chunks_mut(480).zip(inv.chunks_mut(480)) {
        unit.process(a, b, None);
    }
    assert!(unit.telemetry()[telemetry::CORR] < -0.99);
}
