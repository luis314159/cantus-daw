//! r40:pads — *Record: Before FX · After FX* (ADR 0118, schema 62) on the engine: a pass
//! driven by hand, the test pushing into the input ring as the input stream would.
//!
//! - **Before FX** (the default) records the input to the bit, whatever the strip's inserts
//!   do — today's take.
//! - **After FX** records the strip after its inserts: the input as the chain made it,
//!   stereo, to the bit equal to what the monitored strip played — and, with a latent insert,
//!   moved back by the strip's lateness, so frame `f` of the take is the input's frame `f`
//!   through the chain (it lands where it was played).

use cantus_core::*;
use cantus_engine::*;

const SR: u32 = 48_000;

fn signal(frames: usize, seed: u32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    (0..frames)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 1.2 - 0.6
        })
        .collect()
}

/// One armed track with a mono input (monitored while recording: Auto) on a strip that
/// passes its signal at unity (centred, its own law 0 dB), an insert of `kind` set by
/// `set` on it, recording `tap`.
fn project(tap: RecordTap, kind: EffectKind, set: &[(&str, f32)]) -> Project {
    let mut p = Project::new("rec");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    t.record_tap = tap;
    let strip = t.strip.unwrap();
    p.tracks.push(t);
    let mut fx = p.make_effect(kind);
    for (k, v) in set {
        fx.spec.set(k, *v).unwrap();
    }
    let s = p.strip_mut(strip).unwrap();
    s.pan_law = Some(PanLaw::ZeroDb);
    s.effects.push(fx);
    p
}

/// Record `input` (mono) through the engine, 480 frames a block; the take's samples and
/// the channels it has, and what the device played (stereo).
fn record(p: &Project, input: &[f32]) -> (Vec<f32>, u16, Vec<f32>) {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, mut proc_) = EngineHandle::manual(SR as f64);
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 2);
    proc_.attach_input(consumer, 1);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    assert_eq!(h.begin_recording(dir.path()), Ok(1));
    h.set_recording(true);
    h.play();
    let mut out = vec![0f32; 480 * 2];
    let mut heard = Vec::with_capacity(input.len() * 2);
    for chunk in input.chunks(480) {
        for x in chunk {
            ring.push(*x).unwrap();
        }
        let out = &mut out[..chunk.len() * 2];
        proc_.process(out, 2);
        heard.extend_from_slice(out);
        h.poll();
    }
    h.stop();
    for _ in 0..480 {
        ring.push(0.0).unwrap();
    }
    proc_.process(&mut out, 2);
    h.poll();
    let takes = h.finish_recording();
    assert_eq!(takes.len(), 1);
    let mut r = hound::WavReader::open(&takes[0].path).unwrap();
    let ch = r.spec().channels;
    let v = r.samples::<f32>().map(|s| s.unwrap()).collect();
    (v, ch, heard)
}

/// A boost and a cut: the chain changes every sample.
const TONE: [(&str, f32); 6] = [
    ("b0.used", 1.0),
    ("b0.gain", 9.0),
    ("b0.freq", 2_000.0),
    ("b1.used", 1.0),
    ("b1.gain", -6.0),
    ("b1.freq", 200.0),
];

#[test]
fn before_fx_records_the_input_to_the_bit_whatever_the_inserts_do() {
    let p = project(RecordTap::BeforeFx, EffectKind::Eq, &TONE);
    let x = signal(24_000, 3);
    let (take, ch, heard) = record(&p, &x);
    assert_eq!(ch, 1);
    assert_eq!(take, x, "the input, to the bit");
    // The strip did change what was heard: the take is not what the inserts made.
    let l: Vec<f32> = heard.chunks_exact(2).map(|f| f[0]).collect();
    assert_ne!(&l[..x.len()], &x[..]);
}

#[test]
fn after_fx_records_the_input_through_the_chain_in_stereo() {
    let p = project(RecordTap::AfterFx, EffectKind::Eq, &TONE);
    let x = signal(24_000, 4);
    let (take, ch, heard) = record(&p, &x);
    assert_eq!(ch, 2, "the strip's pair");
    assert_eq!(take.len(), x.len() * 2);
    // What the monitored strip played: its inserts' output at unity through the fader.
    let played = &heard[..x.len() * 2];
    if let Some(i) = take
        .iter()
        .zip(played)
        .position(|(a, b)| a.to_bits() != b.to_bits())
    {
        panic!("sample {i}: {} vs {}", take[i], played[i]);
    }
    assert_ne!(
        take.chunks_exact(2).map(|f| f[0]).collect::<Vec<_>>(),
        x,
        "the chain changed it"
    );
}

#[test]
fn after_fx_with_a_latent_insert_lands_where_it_was_played() {
    // A compressor that hears ahead (lookahead 5 ms) and cannot compress: a delay line of
    // its latency, so what it gives is the input moved later by exactly that much.
    let set = [("look", 5.0), ("ratio", 1.0)];
    let p = project(RecordTap::AfterFx, EffectKind::Compressor, &set);
    let x = signal(24_000, 5);
    let (take, ch, heard) = record(&p, &x);
    assert_eq!(ch, 2);
    let d = 240; // 5 ms at 48 kHz
                 // The device played the input `d` frames late (the delay compensation's total)...
    let l: Vec<f32> = heard.chunks_exact(2).map(|f| f[0]).collect();
    assert_eq!(&l[d..x.len()], &x[..x.len() - d]);
    // ...and the take is moved back by as much: its first frame is the input's first.
    let tl: Vec<f32> = take.chunks_exact(2).map(|f| f[0]).collect();
    assert_eq!(
        tl.len(),
        x.len() - d,
        "the inserts' last frames are not played yet"
    );
    assert_eq!(
        &tl[..],
        &x[..x.len() - d],
        "frame f of the take is frame f of the input"
    );
}

// fix-a (mid-round review A-M1): *Hear the key* is a monitor state (ADR 0118 §1), never in
// a take. A vocal recorded After FX through a compressor keyed from a kick, the key heard:
// the take is the take with nothing heard, to the bit — not the kick.

/// `project` with a second track playing notes on its own strip, the insert keyed from it.
fn keyed_project(tap: RecordTap) -> (Project, EffectId) {
    let mut p = project(tap, EffectKind::Compressor, &DUCK);
    let mut k = p.make_track("Kick");
    let mut clip = p.make_clip("k", Ticks::ZERO, Ticks::beats(4));
    for b in 0..4 {
        clip.notes
            .push(p.make_note(Ticks::beats(b), Ticks::EIGHTH, 36, 120));
    }
    k.clips.push(clip);
    let key = k.strip.unwrap();
    p.tracks.push(k);
    let voice = p.tracks[0].strip.unwrap();
    let fx = &mut p.strip_mut(voice).unwrap().effects[0];
    fx.sidechain = Some(key);
    let id = fx.id;
    (p, id)
}

/// A compressor that ducks hard whenever its key sounds.
const DUCK: [(&str, f32); 3] = [("thr", -40.0), ("ratio", 20.0), ("att", 0.5)];

/// `record`, the live engine told to hear `hear` before it plays.
fn record_hearing(p: &Project, input: &[f32], hear: Option<EffectId>) -> (Vec<f32>, Vec<f32>) {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, mut proc_) = EngineHandle::manual(SR as f64);
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 2);
    proc_.attach_input(consumer, 1);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    h.set_hear_key(hear);
    assert_eq!(h.begin_recording(dir.path()), Ok(1));
    h.set_recording(true);
    h.play();
    let mut out = vec![0f32; 480 * 2];
    let mut heard = Vec::with_capacity(input.len() * 2);
    for chunk in input.chunks(480) {
        for x in chunk {
            ring.push(*x).unwrap();
        }
        let out = &mut out[..chunk.len() * 2];
        proc_.process(out, 2);
        heard.extend_from_slice(out);
        h.poll();
    }
    h.stop();
    for _ in 0..480 {
        ring.push(0.0).unwrap();
    }
    proc_.process(&mut out, 2);
    h.poll();
    let takes = h.finish_recording();
    assert_eq!(takes.len(), 1);
    let mut r = hound::WavReader::open(&takes[0].path).unwrap();
    let v = r.samples::<f32>().map(|s| s.unwrap()).collect();
    (v, heard)
}

#[test]
fn after_fx_a_take_never_holds_the_key_heard() {
    let (p, comp) = keyed_project(RecordTap::AfterFx);
    let x = signal(48_000, 6);
    let (plain, _) = record_hearing(&p, &x, None);
    let (hearing, _) = record_hearing(&p, &x, Some(comp));
    assert_eq!(plain.len(), hearing.len());
    if let Some(i) = plain
        .iter()
        .zip(&hearing)
        .position(|(a, b)| a.to_bits() != b.to_bits())
    {
        panic!(
            "sample {i}: the take heard the key ({} vs {})",
            hearing[i], plain[i]
        );
    }
    // The key did duck the voice: the take is not the input.
    let l: Vec<f32> = plain.chunks_exact(2).map(|f| f[0]).collect();
    assert_ne!(&l[..], &x[..l.len()], "the compressor worked on the voice");
}

#[test]
fn before_fx_a_take_is_the_input_while_a_key_is_heard() {
    let (p, comp) = keyed_project(RecordTap::BeforeFx);
    let x = signal(24_000, 7);
    let (take, heard) = record_hearing(&p, &x, Some(comp));
    assert_eq!(take, x, "the input, to the bit");
    // Before FX the strip is not recorded from: the key is heard while recording.
    let l: Vec<f32> = heard.chunks_exact(2).map(|f| f[0]).collect();
    let (_, plain) = record_hearing(&p, &x, None);
    let pl: Vec<f32> = plain.chunks_exact(2).map(|f| f[0]).collect();
    assert_ne!(l, pl, "the key is heard");
}
