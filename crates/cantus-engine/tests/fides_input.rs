//! Fides's live input as a sidechain from a strip (round 41, ADR 0154) through the engine:
//! unkeyed, Fides is what it was (its *Input* exciter silent, a keyed pluck the unkeyed
//! one's bits); keyed, the strip's audio excites it audibly — the same at every block size,
//! offline and as the callback plays it, serial and on the pool; a key that would close a
//! loop is heard as silence, never as a conflict; and a latent key strip delays the track
//! as much, so Fides meets the mix where everything else does.

mod support;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;
use support::strip_of;

const SR: f64 = 48_000.0;
/// Fides's exciter kinds (`cantus_core::inst::fides::EXCITERS`): pluck 0, input 5.
const PLUCK: f32 = 0.0;
const INPUT: f32 = 5.0;

/// A source track playing short notes (Firmus) on the first strip, from beat 1 — after the
/// Input exciter's gate has opened — and a Fides track holding a fifth for four beats on
/// the second strip, its exciter `exciter`, keyed from the source's strip when `keyed`.
fn project(keyed: bool, exciter: f32, source_notes: bool) -> Project {
    let mut p = Project::new("fides input");
    p.tempo_bpm = 120.0;
    p.loop_enabled = false;
    let mut src = p.make_track("Source");
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(4));
    if source_notes {
        let q = Ticks::QUARTER;
        for (at, pitch) in [(2i64, 60u8), (3, 64), (4, 67), (6, 72)] {
            clip.notes.push(p.make_note(q * at / 2, q / 2, pitch, 110));
        }
    }
    src.clips.push(clip);
    let key = src.strip.expect("the source's strip");
    p.tracks.push(src);
    let mut f = p.make_track("Fides");
    let mut spec = NativeSpec::new(InstrumentKind::Resonator);
    spec.set("exc.type", exciter).expect("the exciter");
    f.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Held", Ticks::ZERO, Ticks::beats(4));
    for pitch in [48u8, 55] {
        clip.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(4), pitch, 100));
    }
    f.clips.push(clip);
    if keyed {
        f.sidechain = Some(key);
    }
    p.tracks.push(f);
    p
}

/// The project's source strip given a limiter with nothing to do: a pure delay of its
/// look-ahead (`fx_limiter`'s null test).
fn with_latency(mut p: Project) -> Project {
    let mut slot = p.make_effect(EffectKind::Limiter);
    slot.spec.set("ceiling", 0.0).unwrap();
    slot.spec.set("tp", 1.0).unwrap();
    strip_of(&mut p, 0).effects.push(slot);
    p
}

fn offline(p: &Project, block: usize) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: 0.5,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("render")
        .samples
}

/// A processor as the application drives it (`pool.rs`'s rig): units, automation and the
/// plan sent before the snapshot, compensation's lines answered.
struct Rig {
    p: Processor,
    rx: rtrb::Consumer<EngineEvent>,
}

impl Rig {
    fn new(project: &Project, workers: Workers) -> Rig {
        let snap = Arc::new(Snapshot::compile(project, 1));
        let effects = snap.effect_slots().count();
        let instruments = cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false);
        let (mut tx, rx_engine) =
            rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
        p.set_workers(workers);
        for slot in snap.effect_slots() {
            tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
                slot, SR as f32,
            ))))
            .ok();
        }
        for unit in instruments {
            tx.push(EngineMessage::AddInstrument(unit)).ok();
        }
        tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .ok();
        let plan = cantus_engine::graph::compile(&snap);
        tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
        p.settle();
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::PdcNeed { count, delay } = ev {
                tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                    count, delay,
                )))
                .ok();
                p.settle();
            }
        }
        tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        tx.push(EngineMessage::Play).ok();
        Rig { p, rx }
    }

    fn play(&mut self, blocks: &[usize]) -> Vec<f32> {
        let mut out = Vec::new();
        let mut buf = vec![0f32; MAX_BLOCK_FRAMES * 2];
        for &n in blocks {
            let b = &mut buf[..n * 2];
            self.p.process(b, 2);
            out.extend_from_slice(b);
            while self.rx.pop().is_ok() {}
        }
        out
    }
}

/// Irregular live blocks covering `frames`.
fn live_blocks(frames: usize) -> Vec<usize> {
    let sizes = [128usize, 333, 512, 96, 1024, 17, 256, 1];
    let mut out = Vec::new();
    let (mut done, mut i) = (0, 0);
    while done < frames {
        let n = sizes[i % sizes.len()].min(frames - done);
        out.push(n);
        done += n;
        i += 1;
    }
    out
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i} differs: {} vs {}", a[i], b[i]);
    }
}

fn rms_db(x: &[f32]) -> f64 {
    let e = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64;
    10.0 * cantus_dsp::math::log10(e.max(1e-30))
}

/// What Fides adds to the mix: the keyed render less the unkeyed one (whose Input exciter
/// is silent), frame by frame (left channel).
fn fides_part(keyed: &[f32], unkeyed: &[f32]) -> Vec<f32> {
    keyed
        .chunks_exact(2)
        .zip(unkeyed.chunks_exact(2))
        .map(|(a, b)| a[0] - b[0])
        .collect()
}

#[test]
fn with_no_input_chosen_fides_is_what_it_was() {
    // The Input exciter unkeyed is silent: the mix is the source's alone, to the bit.
    let mut alone = project(false, INPUT, true);
    alone.tracks.truncate(1);
    same_bits(
        &offline(&project(false, INPUT, true), 512),
        &offline(&alone, 512),
        "an unkeyed Input exciter",
    );
    // An exciter that hears no input is the unkeyed one's bits, keyed or not — the plan
    // that runs the track after its key changes no sum.
    same_bits(
        &offline(&project(true, PLUCK, true), 512),
        &offline(&project(false, PLUCK, true), 512),
        "a keyed pluck",
    );
    // A key whose strip is silent excites nothing.
    same_bits(
        &offline(&project(true, INPUT, false), 512),
        &offline(&project(false, INPUT, false), 512),
        "a silent key",
    );
}

#[test]
fn a_strip_s_audio_excites_fides_audibly() {
    let keyed = offline(&project(true, INPUT, true), 512);
    let unkeyed = offline(&project(false, INPUT, true), 512);
    let part = fides_part(&keyed, &unkeyed);
    let beat = (SR / 2.0) as usize;
    // Nothing before the source's first note (beat 1); heard after it.
    assert!(
        part[..beat].iter().all(|v| *v == 0.0),
        "Fides sounded before its input"
    );
    let heard = rms_db(&part[beat..3 * beat]);
    println!("Fides excited by the source: {heard:.1} dBFS RMS over beats 1–3");
    assert!(heard > -45.0, "Fides is not heard: {heard:.1} dBFS");
    assert!(part.iter().all(|v| v.is_finite() && v.abs() < 4.0));
}

#[test]
fn keyed_fides_is_the_same_at_every_block_size_and_live() {
    let p = project(true, INPUT, true);
    let reference = offline(&p, 4096);
    for block in [1usize, 37, 64, 512, 1000] {
        same_bits(&reference, &offline(&p, block), &format!("block {block}"));
    }
    // As the callback plays it, in irregular blocks: the offline render's bits.
    let frames = reference.len() / 2;
    let mut rig = Rig::new(&p, Workers::SERIAL);
    let live = rig.play(&live_blocks(frames));
    same_bits(&reference, &live, "live, serial");
    assert_eq!(rig.p.stats().schedule_conflicts, 0);
}

#[test]
fn keyed_fides_on_the_pool_is_the_serial_bits() {
    let mut p = project(true, INPUT, true);
    // A second keyed Fides on the same key, and a bus after both: more to run at once.
    let mut second = p.make_track("Fides 2");
    second.instrument = p.tracks[1].instrument.clone();
    second.clips = p.tracks[1].clips.clone();
    second.sidechain = p.tracks[1].sidechain;
    p.tracks.push(second);
    let blocks = live_blocks(3 * 48_000);
    let reference = Rig::new(&p, Workers::SERIAL).play(&blocks);
    assert!(reference.iter().any(|v| *v != 0.0));
    for w in [1usize, 3, 7] {
        let mut workers = Workers::parallel(w);
        workers.shuffle = Some(41 + w as u64);
        let mut rig = Rig::new(&p, workers);
        let out = rig.play(&blocks);
        same_bits(&reference, &out, &format!("W = {w}"));
        assert_eq!(rig.p.stats().schedule_conflicts, 0, "W = {w}");
    }
}

#[test]
fn a_key_that_would_close_a_loop_is_heard_as_silence() {
    // Hand-edited files (the commands refuse both): Fides keyed from its own strip, and
    // from a strip its own strip feeds.
    let unkeyed = offline(&project(false, INPUT, true), 512);
    let mut own = project(false, INPUT, true);
    own.tracks[1].sidechain = own.tracks[1].strip;
    let mut through = project(false, INPUT, true);
    let bus = through.strips[2].id;
    strip_of(&mut through, 1).output = Some(bus);
    through.tracks[1].sidechain = Some(bus);
    let mut through_unkeyed = through.clone();
    through_unkeyed.tracks[1].sidechain = None;
    for (what, p, want) in [
        ("its own strip", &own, unkeyed.clone()),
        (
            "a strip its strip feeds",
            &through,
            offline(&through_unkeyed, 512),
        ),
    ] {
        let snap = Snapshot::compile(p, 1);
        assert!(snap.tracks[1].sidechain.is_some(), "{what}: the key is sent");
        let plan = cantus_engine::graph::compile(&snap);
        assert_eq!(plan.track_key(1), None, "{what}: not honoured");
        same_bits(&offline(p, 512), &want, what);
        let mut rig = Rig::new(p, Workers::parallel(3));
        let frames = want.len() / 2;
        let live = rig.play(&live_blocks(frames));
        same_bits(&live, &want, &format!("{what}, live on the pool"));
        assert_eq!(rig.p.stats().schedule_conflicts, 0, "{what}");
    }
}

#[test]
fn a_latent_key_strip_delays_fides_with_the_mix() {
    let onset = |x: &[f32]| x.iter().position(|v| *v != 0.0).expect("Fides sounds");
    let plain = fides_part(
        &offline(&project(true, INPUT, true), 512),
        &offline(&project(false, INPUT, true), 512),
    );
    let latent = with_latency(project(true, INPUT, true));
    let snap = Arc::new(Snapshot::compile(&latent, 1));
    let lat = EffectUnit::new(&snap.strips[0].effects[0], SR as f32).latency();
    assert!(lat > 0, "the limiter is latent");
    let delayed = fides_part(
        &offline(&latent, 512),
        &offline(&with_latency(project(false, INPUT, true)), 512),
    );
    // Everything reaches the master `lat` late (the source through its limiter, the rest
    // waiting for it): Fides, which hears the source late by as much, leaves late by as
    // much — no later.
    let (a, b) = (onset(&plain), onset(&delayed));
    println!("Fides's onset {a}, with a {lat}-sample key strip {b}");
    assert_eq!(b, a + lat, "Fides is not where the mix is");
    let mut rig = Rig::new(&latent, Workers::SERIAL);
    rig.play(&[512]);
    assert_eq!(rig.p.stats().latency_samples as usize, lat, "the mix's latency");
    // And as heard live, it is the offline render.
    let reference = offline(&latent, 512);
    let mut rig = Rig::new(&latent, Workers::SERIAL);
    let live = rig.play(&live_blocks(reference.len() / 2));
    same_bits(&reference, &live, "latent, live");
}
