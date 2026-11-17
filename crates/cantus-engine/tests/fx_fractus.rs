//! Fractus, the glitch sequencer and grain cloud, through the whole engine (ADR 0137):
//! at its defaults the mix untouched, to the bit, no latency, and no memory
//! taken; painted, its DSP asked for off the audio thread and played from the sample it
//! lands; the starting preset sounds; offline equals real time; the notes of another track
//! play the lanes; a fork carries on as the unit does. // fxa:fractus

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::fx::fractus::{Block, Pattern};
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use modfx::*;
use std::sync::Arc;
use support::*;

const SR: f32 = 48_000.0;

fn slot_of(p: &mut Project, fx: EffectId) -> EffectSlot {
    strip_of(p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone()
}

fn paint(p: &mut Project, fx: EffectId, lanes: &[(u8, Block)]) {
    let e = strip_of(p, 0)
        .effects
        .iter_mut()
        .find(|e| e.id == fx)
        .unwrap();
    let mut pat = Pattern::default();
    for (lane, b) in lanes {
        let mut blocks = pat.lane(usize::from(*lane)).to_vec();
        blocks.push(*b);
        pat.replace(*lane, &blocks);
    }
    e.spec.fractus = (!pat.is_empty()).then(|| Box::new(pat));
}

#[test]
fn at_its_defaults_it_touches_no_sample_and_takes_no_memory() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Glitch);
    let unit = EffectUnit::new(&slot_of(&mut p, fx), SR);
    assert_eq!(unit.latency(), 0);
    assert_eq!(
        unit.memory_need(),
        None,
        "an empty Fractus asks for nothing"
    );
    assert_null(&reference, &render_of(&p), 0.0, "Fractus at its defaults");
}

#[test]
fn the_starting_preset_sounds_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Glitch);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Glitch);
    strip_of(&mut p, 0).effects.push(slot);
    let a = render_of(&p);
    let dry = render_of(&demo_project());
    assert!(
        a.iter().zip(&dry).any(|(x, y)| (x - y).abs() > 1e-3),
        "it sounds"
    );
    let b = realtime(&p, a.len() / 2);
    assert_null(&a[..b.len()], &b, 0.0, "real time equals offline");
    for block in [1usize, 37, 64, 512] {
        let c = render_with(&p, block);
        assert_null(&a, &c, 0.0, &format!("blocks of {block}"));
    }
}

/// Every lane painted, the cloud up: live (irregular blocks, the units sent as the UI
/// sends them) is the offline render, to the bit.
#[test]
fn every_lane_live_is_the_offline_render() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Glitch);
    let lanes: Vec<(u8, Block)> = (0..10u8)
        .map(|l| (l, Block::new(u16::from(l) % 14, 2)))
        .collect();
    paint(&mut p, fx, &lanes);
    set_all(
        &mut p,
        fx,
        &[("clevel", 0.4), ("swing", 0.3), ("rramp", 1.0)],
    );
    let a = render_of(&p);
    let b = realtime(&p, a.len() / 2);
    assert_null(&a[..b.len()], &b, 0.0, "every lane, live = offline");
}

/// Painted while it plays: the unit, built empty, asks for its DSP (`EffectNeed`, the
/// memory path); the answer built off the audio thread is taken and it sounds from then
/// on — before it, the wire it was.
#[test]
fn painted_while_playing_it_asks_for_its_dsp_and_takes_it() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Glitch);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let snap = Snapshot::compile(&p, 1);
    for strip in &snap.strips {
        for slot in &strip.effects {
            tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
                slot, SR,
            ))))
            .unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut asked = 0;
    let mut run = |proc: &mut Processor,
                   tx: &mut rtrb::Producer<EngineMessage>,
                   blocks: usize,
                   asked: &mut usize| {
        for _ in 0..blocks {
            let mut buf = vec![0f32; 512 * 2];
            proc.process(&mut buf, 2);
            while let Ok(ev) = rx.pop() {
                if let EngineEvent::EffectNeed {
                    effect,
                    pedal,
                    need,
                } = ev
                {
                    assert_eq!(effect, fx);
                    let need = need.need();
                    assert_eq!(need, MemoryNeed::Fractus);
                    *asked += 1;
                    let memory = EffectMemory::build(need, SR);
                    tx.push(EngineMessage::EffectMemory {
                        effect,
                        pedal,
                        memory,
                    })
                    .unwrap();
                }
            }
        }
    };
    run(&mut proc, &mut tx, 20, &mut asked);
    assert_eq!(asked, 0, "empty, it asks for nothing");
    paint(&mut p, fx, &[(0, Block::new(0, 16))]);
    tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &p, 2,
    ))))
    .unwrap();
    run(&mut proc, &mut tx, 200, &mut asked);
    assert_eq!(asked, 1, "painted, it asks once");
}

/// The notes of another track play the lanes (`trig` notes): the bass's notes on the
/// repeat's key and above change the sound; with no source nothing plays.
#[test]
fn the_notes_of_another_track_play_the_lanes() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Glitch);
    // The bass plays C2 and G2 (36, 43): from a key of 34 they play the tape stop and the
    // cloud.
    set_all(&mut p, fx, &[("trig", 1.0), ("key", 34.0), ("quant", 0.0)]);
    let bass = p.tracks[1].id;
    strip_of(&mut p, 0).effects[0].midi_from = Some(bass);
    let mut deaf = p.clone();
    strip_of(&mut deaf, 0).effects[0].midi_from = None;
    let (a, b) = (render_of(&p), render_of(&deaf));
    let moved = a
        .iter()
        .zip(&b)
        .fold(0f32, |m, (x, y)| m.max((x - y).abs()));
    assert!(moved > 1e-3, "the notes change the sound ({moved})");
    assert_null(&render_of(&demo_project()), &b, 0.0, "no notes: the mix");
    let live = realtime(&p, a.len() / 2);
    assert_null(&a[..live.len()], &live, 0.0, "notes: live = offline");
}

/// A fork carries on as the unit does (the history, the capture, the cloud's grains and
/// the sequence's place copied): the Arca checkpoint lesson.
#[test]
fn a_fork_carries_on_from_where_the_unit_is() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Glitch);
    strip_of(&mut p, 0).effects[0].spec = support::heavy::heavy(EffectKind::Glitch);
    let slot = slot_of(&mut p, fx);
    let mut a = EffectUnit::new(&slot, SR);
    let pos = cantus_engine::fx::SongPosition {
        playing: true,
        tempo_bpm: 120.0,
        tempo_now: 120.0,
        time_sig_num: 4,
        time_sig_den: 4,
        ..Default::default()
    };
    let music = |i: usize| {
        let t = i as f32 / SR;
        0.3 * cantus_dsp::math::sin(t * 220.0 * std::f32::consts::TAU)
            * (1.0 + cantus_dsp::math::sin(t * 3.0))
    };
    let mut at = 0usize;
    let block = |u: &mut EffectUnit, at: usize| {
        let mut l: Vec<f32> = (at..at + 480).map(music).collect();
        let mut r = l.clone();
        let mut p = pos;
        p.samples = at as i64;
        p.beats = at as f64 / 24_000.0;
        u.set_position(&p);
        u.process(&mut l, &mut r);
        l
    };
    for _ in 0..300 {
        block(&mut a, at);
        at += 480;
    }
    let mut b = a.fork().expect("a fork");
    for _ in 0..200 {
        let x = block(&mut a, at);
        let y = block(&mut b, at);
        assert_eq!(x, y, "at {at}");
        at += 480;
    }
}
