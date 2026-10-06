//! Comprimo inside the engine: a wire at settings that do nothing, to the bit; the
//! lookahead's latency is what it reports and exactly what it delays; the result does not
//! depend on the block size with every feature on; the key's EQ decides what a key from
//! another strip can trigger; and the telemetry the editor reads comes through.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::comprimo::tele;
use cantus_dsp::nulltest::{assert_null, rms};
use cantus_engine::*;
use support::*;

/// A compressor that cannot compress is a wire: the render is the render without it, to
/// the bit — at 1:1 with auto gain on, the mix at 200 % and a key EQ band; with no range
/// to work in; and at 0 % mix while it is working hard.
#[test]
fn a_compressor_that_cannot_compress_is_a_wire() {
    let p = demo_project();
    let dry = render_of(&p);
    let cases: [&[(&str, f32)]; 3] = [
        &[
            ("ratio", 1.0),
            ("thr", -60.0),
            ("auto_gain", 1.0),
            ("mix", 2.0),
            ("sc0.used", 1.0),
            ("sc0.shape", 3.0),
        ],
        &[("range", 0.0), ("thr", -60.0), ("auto_gain", 1.0)],
        &[("mix", 0.0), ("thr", -60.0), ("ratio", 100.0)],
    ];
    for values in cases {
        let mut with = p.clone();
        let fx = add_effect(&mut with, EffectKind::Compressor);
        let slot = strip_of(&mut with, 0).effect_mut(fx).unwrap();
        for (k, v) in values {
            slot.spec.set(k, *v).unwrap();
        }
        assert_null(&dry, &render_of(&with), 0.0, &format!("{values:?}"));
    }
}

/// The lookahead delays the strip by exactly the latency the unit reports: with nothing
/// to compress and the other strip silent, the render is the dry one shifted by it.
#[test]
fn lookahead_latency_is_reported_and_exact() {
    let mut p = demo_project();
    strip_of(&mut p, 1).gain_db = MIN_GAIN_DB;
    let dry = render_of(&p);
    let mut with = p.clone();
    let fx = add_effect(&mut with, EffectKind::Compressor);
    {
        let slot = strip_of(&mut with, 0).effect_mut(fx).unwrap();
        slot.spec.set("ratio", 1.0).unwrap();
        slot.spec.set("look", 5.0).unwrap();
    }
    let slot = strip_of(&mut with, 0).effect(fx).unwrap().clone();
    let unit = EffectUnit::new(&slot, 48_000.0);
    assert_eq!(unit.latency(), 240, "5 ms at 48 kHz");
    let late = render_of(&with);
    let d = 240 * 2; // interleaved stereo
    assert!(late[..d].iter().all(|x| *x == 0.0));
    assert_null(
        &dry[..dry.len() - d],
        &late[d..],
        0.0,
        "the lookahead is a pure delay of its latency",
    );
}

/// Every feature on — lookahead, a key EQ, a custom engine with feedback and a second
/// release stage, auto gain, a dry path, mix, in and out gain — and the render is the
/// same at every block size, down to one frame.
#[test]
fn everything_on_is_block_size_independent() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Compressor);
    {
        let slot = strip_of(&mut p, 0).effect_mut(fx).unwrap();
        for (k, v) in [
            ("thr", -36.0),
            ("ratio", 6.0),
            ("att", 0.5),
            ("rel", 80.0),
            ("hold", 3.0),
            ("look", 1.7),
            ("auto_gain", 1.0),
            ("auto_rel", 1.0),
            ("dry", -18.0),
            ("mix", 1.3),
            ("in_gain", 3.0),
            ("out_gain", -2.0),
            ("custom", 1.0),
            ("det_rms", 4.0),
            ("det_fb", 0.3),
            ("det_prog", 1.0),
            ("det_dual", 0.5),
            ("det_curve", 0.4),
            ("key_gain", 2.0),
            ("sc0.used", 1.0),
            ("sc0.shape", 3.0),
            ("sc0.freq", 150.0),
            ("sc0.slope", 36.0),
            ("sc1.used", 1.0),
            ("sc1.freq", 3_000.0),
            ("sc1.gain", 6.0),
        ] {
            slot.spec.set(k, v).unwrap();
        }
    }
    let reference = render_with(&p, 512);
    assert!(reference.iter().all(|x| x.is_finite()));
    assert!(
        rms(&reference) > 1e-3,
        "the render is not silence: {}",
        rms(&reference)
    );
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

/// Keyed from the bass strip, the lead ducks; with a 1 kHz low cut on the key the bass
/// cannot reach the threshold and the lead is left (all but) alone — the key EQ is on the detector,
/// not on the audio. Bypassing the key EQ brings the ducking back.
#[test]
fn the_key_eq_decides_what_a_key_can_trigger() {
    let mut p = demo_project();
    let (lead, bass) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    // Keep the bass out of the master, so the render is the lead alone.
    let sink = p.strips[8].id;
    p.strip_mut(bass).unwrap().output = Some(sink);
    p.strip_mut(sink).unwrap().muted = true;
    let alone = render_of(&p);
    let mut comp = p.make_effect(EffectKind::Compressor);
    for (k, v) in [("thr", -40.0), ("ratio", 20.0), ("att", 0.5)] {
        comp.spec.set(k, v).unwrap();
    }
    comp.sidechain = Some(bass);
    let mut ducked = p.clone();
    ducked.strip_mut(lead).unwrap().effects.push(comp.clone());
    let ducked = render_of(&ducked);
    assert!(rms(&ducked) < rms(&alone) * 0.6, "the bass ducks the lead");

    for (k, v) in [
        ("sc0.used", 1.0),
        ("sc0.shape", 3.0),
        ("sc0.freq", 1_000.0),
        ("sc0.slope", 96.0),
        ("key_gain", -12.0),
    ] {
        comp.spec.set(k, v).unwrap();
    }
    let mut filtered = p.clone();
    filtered.strip_mut(lead).unwrap().effects.push(comp.clone());
    let filtered = render_of(&filtered);
    assert!(
        rms(&filtered) > rms(&alone) * 0.95,
        "a 1 kHz cut on the key leaves the lead alone: {} vs {}",
        rms(&filtered),
        rms(&alone)
    );
    comp.spec.set("key_eq", 0.0).unwrap();
    let mut bypassed = p.clone();
    bypassed.strip_mut(lead).unwrap().effects.push(comp);
    let bypassed = render_of(&bypassed);
    assert!(
        rms(&bypassed) < rms(&alone) * 0.8,
        "the EQ's bypass puts it back"
    );
}

/// The unit reports its telemetry — the editor's meters, loudness and statistics — and
/// what it reports is the DSP's own reading.
#[test]
fn the_unit_reports_its_telemetry() {
    let mut p = Project::new("t");
    let mut slot = p.make_effect(EffectKind::Compressor);
    slot.spec.set("thr", -30.0).unwrap();
    slot.spec.set("look", 3.0).unwrap();
    let mut unit = EffectUnit::new(&slot, 48_000.0);
    let mut l: Vec<f32> = (0..48_000)
        .map(|i| 0.5 * math::sin(std::f32::consts::TAU * 220.0 * i as f32 / 48_000.0))
        .collect();
    let mut r = l.clone();
    for (a, b) in l.chunks_mut(480).zip(r.chunks_mut(480)) {
        unit.process(a, b);
    }
    let t = unit.telemetry().expect("a compressor reports telemetry");
    assert_eq!(t[tele::SAMPLE_RATE], 48_000.0);
    assert_eq!(t[tele::LATENCY], 144.0);
    assert_eq!(t[tele::HOPS], 10.0);
    assert!(t[tele::GR] > 5.0, "{}", t[tele::GR]);
    assert!(t[tele::OUT_S] > -40.0 && t[tele::OUT_S] < 0.0);
    let m = unit.comp_meter().unwrap();
    assert!(m.gr_db >= t[tele::GR]);
}

/// The desk's reduction lane reads the engine's per-strip value: the lead's strip, with a
/// compressor working hard, shows its reduction; the bass's strip, with none, shows 0 —
/// whether or not an editor is open.
#[test]
fn every_strip_reports_its_compressors_reduction_to_the_desk() {
    use std::sync::Arc;
    let mut p = demo_project();
    let id = add_effect(&mut p, EffectKind::Compressor);
    let slot = strip_of(&mut p, 0).effect_mut(id).unwrap();
    slot.spec.set("thr", -50.0).unwrap();
    slot.spec.set("ratio", 20.0).unwrap();
    let lead = p.tracks[0].strip.unwrap();
    let lead_index = p.strips.iter().position(|s| s.id == lead).unwrap();
    let bass = p.tracks[1].strip.unwrap();
    let bass_index = p.strips.iter().position(|s| s.id == bass).unwrap();
    assert_ne!(lead, bass);

    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let snap = Snapshot::compile(&p, 1);
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, 48_000.0);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut deepest = [0f32; 2];
    for _ in 0..200 {
        let mut buf = vec![0f32; 512 * 2];
        proc.process(&mut buf, 2);
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::Meters(m) = ev {
                deepest[0] = deepest[0].max(m.strip_gr[lead_index]);
                deepest[1] = deepest[1].max(m.strip_gr[bass_index]);
            }
        }
    }
    assert!(
        deepest[0] > 6.0,
        "the working compressor showed {} dB",
        deepest[0]
    );
    assert_eq!(
        deepest[1], 0.0,
        "a strip with no compressor showed reduction"
    );
}
