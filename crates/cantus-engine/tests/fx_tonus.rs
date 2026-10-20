//! Tonus through the whole engine: its nulls (amount 0, every note on bypass, a MIDI source
//! with nothing held, the slot's bypass) are the dry render delayed by exactly the latency
//! it reports; the same render at every block size while it corrects; offline equal to
//! real time; MIDI targets from another track's clip, at their sample; and what a singer
//! monitoring through it hears.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::pitch::testkit::{self, Vowel};
use cantus_dsp::pitch::{analyse, hz_to_midi};
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

const SR: u32 = 48_000;
const LATENCY: usize = 128;

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in samples {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

/// A sung line a little out of tune: 196 Hz + 30 cents, then 247 Hz − 25 cents with a
/// vibrato, a breath of silence between. Two seconds, 120 BPM (a beat is 24 000 samples).
fn sung() -> Vec<f32> {
    let off = |hz: f32, c: f32| hz * cantus_dsp::math::exp2(c / 1200.0);
    let mut x = testkit::voice(SR as f32, 0.9, Vowel::A, 0.4, |_| off(196.0, 30.0));
    x.extend(std::iter::repeat_n(0.0, 4_800));
    let vib = testkit::vibrato(off(246.94, -25.0), 30.0, 5.5);
    x.extend(testkit::voice(SR as f32, 0.9, Vowel::I, 0.4, &vib));
    x
}

/// The voice on an audio track with Tonus on its strip, a synth on a second track (so the
/// mix shows delay compensation), and a muted MIDI track whose notes Tonus may hear.
fn project(dir: &Path) -> (Project, EffectId, TrackId) {
    let x = sung();
    let path = dir.join("voice.wav");
    write_wav(&path, &x);
    let mut p = support::demo_project();
    p.tempo_bpm = 120.0;
    p.metronome = false;
    p.tracks.truncate(2);
    p.tracks.swap(0, 1); // the bass first, then the lead becomes the voice's track
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "voice".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames: x.len() as u64,
        sha256: None,
        recorded: false,
    });
    p.tracks[1].clips.clear();
    let clip = p.make_audio_clip("voice", id, Ticks::ZERO, Ticks::beats(4));
    p.tracks[1].clips.push(clip);
    // The MIDI track: notes only, muted, on the voice's strip.
    let mut keys = p.make_track("Keys");
    keys.muted = true;
    keys.strip = p.tracks[1].strip;
    let mut c = p.make_clip("targets", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::beats(1), Ticks::beats(1), 67, 100)); // G
    c.notes
        .push(p.make_note(Ticks::beats(2), Ticks::beats(2), 71, 100)); // B
    keys.clips.push(c);
    let tid = keys.id;
    p.tracks.push(keys);
    let mut fx = p.make_effect(EffectKind::AutoTune);
    fx.midi_from = Some(tid);
    let fid = fx.id;
    support::strip_of(&mut p, 1).effects.push(fx);
    (p, fid, tid)
}

fn set(p: &mut Project, fx: EffectId, key: &str, v: f32) {
    let s = support::strip_of(p, 1);
    let slot = s.effects.iter_mut().find(|e| e.id == fx).unwrap();
    slot.spec.set(key, v).unwrap();
}

fn snapshot(p: &Project) -> Arc<Snapshot> {
    let mut s = Snapshot::compile(p, 1);
    let errors = SampleCache::new().attach_audio(&mut s, SR);
    assert!(errors.is_empty(), "{errors:?}");
    Arc::new(s)
}

fn render_with(p: &Project, block: usize) -> Vec<f32> {
    let opts = RenderOptions {
        sample_rate: SR,
        tail_seconds: 0.2,
        block_frames: block,
        ..Default::default()
    };
    render(snapshot(p), &opts).unwrap().samples
}

fn without_tonus(p: &Project) -> Project {
    let mut q = p.clone();
    support::strip_of(&mut q, 1).effects.clear();
    q
}

/// `out` is `reference` exactly `LATENCY` frames later (interleaved stereo).
fn assert_delayed(reference: &[f32], out: &[f32], what: &str) {
    let shift = 2 * LATENCY;
    assert!(
        out[..shift].iter().all(|x| *x == 0.0),
        "{what}: sound before the latency"
    );
    assert_null(
        &reference[..reference.len() - shift],
        &out[shift..],
        0.0,
        what,
    );
}

#[test]
fn the_unit_reports_its_latency() {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, _, _) = project(dir.path());
    let slot = support::strip_of(&mut p, 1).effects[0].clone();
    assert_eq!(EffectUnit::new(&slot, SR as f32).latency(), LATENCY);
    // The owner's target: under 3 ms at 48 kHz.
    const _: () = assert!(LATENCY <= 144);
}

/// The nulls through the whole engine: the dry mix (voice and synth), later by exactly
/// the latency — delay compensation holds the synth back with the voice.
#[test]
fn its_nulls_are_the_dry_render_delayed_by_its_latency() {
    let dir = tempfile::tempdir().unwrap();
    let (base, fx, _) = project(dir.path());
    let reference = render_with(&without_tonus(&base), 256);
    let mut cases: Vec<(&str, Project)> = Vec::new();
    let mut p = base.clone();
    set(&mut p, fx, "amount", 0.0);
    set(&mut p, fx, "speed", 400.0);
    cases.push(("amount 0, slowest, every note allowed", p));
    let mut p = base.clone();
    for k in cantus_core::fx::tonus::NOTE_KEYS {
        set(&mut p, fx, k, 3.0);
    }
    cases.push(("every note on bypass", p));
    let mut p = base.clone();
    set(&mut p, fx, "src", 1.0);
    support::strip_of(&mut p, 1).effects[0].midi_from = None;
    cases.push(("MIDI, nothing held", p));
    let mut p = base.clone();
    support::strip_of(&mut p, 1).effects[0].bypass = true;
    cases.push(("the slot bypassed", p));
    for (what, p) in &cases {
        assert_delayed(&reference, &render_with(p, 256), what);
    }
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_corrects() {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, fx, _) = project(dir.path());
    set(&mut p, fx, "speed", 0.0);
    set(&mut p, fx, "throat", 2.0);
    set(&mut p, fx, "vdepth", 20.0);
    let reference = render_with(&p, 512);
    // Corrected, not merely delayed: against the dry mix moved by Tonus's latency (what a
    // Tonus that never engaged would give), the render differs.
    let dry = render_with(&without_tonus(&p), 512);
    let shift = 2 * LATENCY;
    let moved = reference[shift..]
        .iter()
        .zip(&dry[..dry.len() - shift])
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(moved > 1e-3, "the voice is corrected: {moved}");
    for block in [1usize, 37, 64, 4_096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

#[test]
fn offline_render_equals_real_time() {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, fx, _) = project(dir.path());
    set(&mut p, fx, "src", 1.0);
    set(&mut p, fx, "speed", 10.0);
    let snap = snapshot(&p);
    let offline = render(
        snap.clone(),
        &RenderOptions {
            sample_rate: SR,
            tail_seconds: 0.2,
            block_frames: 256,
            ..Default::default()
        },
    )
    .unwrap();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR as f64, rx_engine, tx_engine);
    // The units come from the UI thread, as the backend sends them.
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, SR as f32);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
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
    assert_null(&offline.samples, &out, 0.0, "real time against offline");
}

/// The notes of the MIDI track become the targets where they are written: G from beat 1,
/// B from beat 2 — the voice (sung at G + 30 cents, then B − 25 cents) lands on each, and
/// before the first note (nothing held) it passes untouched.
#[test]
fn midi_targets_come_from_another_tracks_notes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, fx, _) = project(dir.path());
    set(&mut p, fx, "src", 1.0);
    set(&mut p, fx, "speed", 0.0);
    // The voice alone: the synth out of the mix.
    p.tracks[0].muted = true;
    let out = render_with(&p, 256);
    let left: Vec<f32> = out.chunks_exact(2).map(|f| f[0]).collect();
    let a = analyse(&left, SR as f32);
    let pitch_at = |from: f32, to: f32| -> Vec<f32> {
        a.frames
            .iter()
            .filter(|f| {
                f.voiced
                    && f.centre >= (from * SR as f32) as f64
                    && f.centre < (to * SR as f32) as f64
            })
            .map(|f| hz_to_midi(f.f0))
            .collect()
    };
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    // Beat 0 to 1 (0.0–0.5 s): no note held, the voice as sung (G3 + 30 cents).
    let before = pitch_at(0.1, 0.45);
    assert!(
        (mean(&before) - 55.30).abs() < 0.05,
        "before: {}",
        mean(&before)
    );
    // Beat 1 to 2 (0.5–1.0 s): G held: G3.
    let on_g = pitch_at(0.56, 0.88);
    assert!((mean(&on_g) - 55.0).abs() < 0.03, "on G: {}", mean(&on_g));
    // Beat 2 to 4 (1.0–2.0 s): B held: B3 (the voice starts again at 1.0 s).
    let on_b = pitch_at(1.15, 1.8);
    assert!((mean(&on_b) - 59.0).abs() < 0.04, "on B: {}", mean(&on_b));
}

/// A singer monitoring through Tonus hears their voice exactly its latency late — 128
/// samples, 2.7 ms — and nothing more, through the strip as ADR 0025 §4 has it.
#[test]
fn monitoring_through_it_is_late_by_its_latency_only() {
    let mut p = Project::new("monitor");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::On;
    t.input = Some(input);
    p.tracks.push(t);
    let mut fx = p.make_effect(EffectKind::AutoTune);
    fx.spec.set("amount", 0.0).unwrap();
    support::strip_of(&mut p, 0).effects.push(fx);
    let snap = Snapshot::compile(&p, 1);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR as f64, rx_engine, tx_engine);
    let (mut feed, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize);
    proc.attach_input(consumer, 1);
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, SR as f32);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    let x = testkit::voice(SR as f32, 0.3, Vowel::A, 0.4, |_| 220.0);
    let mut out = Vec::new();
    for block in x.chunks(256) {
        for v in block {
            feed.push(*v).unwrap();
        }
        let mut buf = vec![0f32; block.len() * 2];
        proc.process(&mut buf, 2);
        out.extend(buf.chunks_exact(2).map(|f| f[0]));
    }
    // The strip's pan law scales the centre by a constant: find it, then compare the
    // shape, delayed by the latency and nothing else.
    let (num, den) = (LATENCY..out.len()).fold((0.0f64, 0.0f64), |(n, d), i| {
        let v = x[i - LATENCY] as f64;
        (n + out[i] as f64 * v, d + v * v)
    });
    let g = (num / den) as f32;
    assert!(g > 0.3 && g <= 1.0, "gain {g}");
    assert!(
        out[..LATENCY].iter().all(|v| *v == 0.0),
        "heard before the latency"
    );
    for i in LATENCY..out.len() {
        let want = x[i - LATENCY] * g;
        assert!(
            (out[i] - want).abs() < 1e-6,
            "sample {i}: {} against {want}",
            out[i]
        );
    }
}

/// A singer monitored through Tonus while another strip has a linear-phase EQ (and,
/// `eq_on_voice`, the voice's own strip one too): how many samples late the voice comes
/// out, with low-latency monitoring at `low` samples or off (ADR 0064 §4), and the stats.
fn monitored(low: Option<u32>, eq_on_voice: bool) -> (usize, usize, EngineStats) {
    let mut p = Project::new("monitor");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::On;
    t.input = Some(input);
    p.tracks.push(t);
    let other = p.make_track("Other");
    p.tracks.push(other);
    let mut fx = p.make_effect(EffectKind::AutoTune);
    fx.spec.set("amount", 0.0).unwrap();
    support::strip_of(&mut p, 0).effects.push(fx);
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0).unwrap();
    support::strip_of(&mut p, 1).effects.push(eq.clone());
    if eq_on_voice {
        let mut eq2 = p.make_effect(EffectKind::Eq);
        eq2.spec.set("mode", 2.0).unwrap();
        support::strip_of(&mut p, 0).effects.push(eq2);
    }
    let snap = Snapshot::compile(&p, 1);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR as f64, rx_engine, tx_engine);
    let (mut feed, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize);
    proc.attach_input(consumer, 1);
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = EffectUnit::new(slot, SR as f32);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::SetLowLatency(low)).unwrap();
    let x = testkit::voice(SR as f32, 0.5, Vowel::A, 0.4, |_| 220.0);
    let mut out = Vec::new();
    for block in x.chunks(256) {
        for v in block {
            feed.push(*v).unwrap();
        }
        let mut buf = vec![0f32; block.len() * 2];
        proc.process(&mut buf, 2);
        out.extend(buf.chunks_exact(2).map(|f| f[0]));
    }
    let first = out.iter().position(|v| *v != 0.0).unwrap();
    let eq_latency = EffectUnit::new(&eq, SR as f32).latency();
    (first, eq_latency, *proc.stats())
}

/// What a singer hears when another strip has a latent insert (a linear-phase EQ), with
/// low-latency monitoring off: delay compensation aligns every strip to the project's
/// largest latency, the monitored voice included — so the voice is late by that, not by
/// Tonus.
#[test]
fn monitoring_waits_for_the_projects_largest_latency() {
    let (first, eq_latency, stats) = monitored(None, false);
    assert!(eq_latency > LATENCY);
    eprintln!("monitored voice: {first} samples late (the EQ's {eq_latency}, Tonus's {LATENCY})");
    assert_eq!(first, eq_latency.max(LATENCY));
    assert!(!stats.low_latency.on);
}

/// The same with low-latency monitoring on (ADR 0064 §4, its default limit of 3 ms): the
/// voice's own strip is not held back for the other strip's EQ — it arrives after Tonus's
/// 128 samples only (2.7 ms instead of 96) — while the backing keeps its alignment.
#[test]
fn low_latency_monitoring_hears_the_voice_after_tonus_only() {
    let limit = (0.003 * SR as f64).round() as u32;
    let (first, eq_latency, stats) = monitored(Some(limit), false);
    eprintln!("low-latency monitoring: {first} samples late (Tonus's {LATENCY})");
    assert_eq!(first, LATENCY);
    assert_eq!(
        stats.latency_samples as usize, eq_latency,
        "the backing stays aligned"
    );
    assert!(stats.low_latency.on && stats.low_latency.monitoring);
    assert_eq!(
        stats.low_latency.off_count, 0,
        "nothing on the voice's path is over 3 ms"
    );
    assert_eq!(stats.low_latency.monitored_samples as usize, LATENCY);
    // A linear-phase EQ on the voice's own strip is over the limit: bypassed, and named.
    let (first, _, stats) = monitored(Some(limit), true);
    assert_eq!(
        first, LATENCY,
        "the voice's EQ is bypassed while monitoring"
    );
    assert_eq!(stats.low_latency.off_count, 1);
}
