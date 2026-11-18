//! fx/final — a Delay on a Tabula board (ADR 0131) holds the line its automated note value
//! can reach before the song gets there, exactly as an inserted Delay does (round 40's
//! A-M2 floor, ADR 0119): the board's pedal is automated by its routed key (`p0.note`), or
//! moved by a macro a lane drives, and renders the inserted Delay's bits offline and in real
//! time. (`rt_memory.rs` holds the board's Delay to no allocation on the audio thread.)

use cantus_core::fx::tabula::{Board, Macro, MacroMap, Pedal, PedalId};
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::sync::Arc;

const SR: u32 = 48_000;

/// How the Delay is put on the strip and how its note value is automated.
#[derive(Clone, Copy, PartialEq)]
enum Where {
    /// Inserted, a lane on `note`.
    Inserted,
    /// The only pedal of a board, a lane on the board's `p0.note`.
    Pedal,
    /// The only pedal of a board, its note mapped to macro 1 (a quarter … a whole note), a
    /// lane on `m1`.
    Macro,
}

/// One track, a short note at beat 0; at 60 BPM a Delay synced to a quarter (1 s; its line
/// 3 s, a quarter at the slowest tempo), fully wet, one repeat. From beat 4 a lane makes it a
/// whole note (4 s): the note at beat 0 echoes at 4 s only if the line already held 4 s of
/// history when the lane got there — the floor built it before the song started.
fn project(at: Where) -> Project {
    let mut p = Project::new("delay on a board");
    p.tempo_bpm = 60.0;
    let mut t = p.make_track("Keys");
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(12));
    clip.notes
        .push(p.make_note(Ticks::ZERO, Ticks::SIXTEENTH, 72, 110));
    t.clips.push(clip);
    let mut delay = EffectSpec::new(EffectKind::Delay);
    for (k, v) in [
        ("mix", 1.0),
        ("feedback", 0.0),
        ("ping_pong", 0.0),
        ("tone_hz", 20_000.0),
        ("sync", 1.0),
        ("note", 12.0),
    ] {
        delay.set(k, v).unwrap();
    }
    let mut fx = p.make_effect(if at == Where::Inserted {
        EffectKind::Delay
    } else {
        EffectKind::Board
    });
    let (key, pos) = match at {
        Where::Inserted => {
            fx.spec = delay;
            ("note", 18.0 / 26.0)
        }
        Where::Pedal => {
            fx.spec.board = Some(Box::new(Board {
                pedals: vec![Pedal::new(PedalId(0), delay)],
                macros: Vec::new(),
            }));
            ("p0.note", 18.0 / 26.0)
        }
        Where::Macro => {
            fx.spec.board = Some(Box::new(Board {
                pedals: vec![Pedal::new(PedalId(0), delay)],
                macros: vec![Macro {
                    name: None,
                    maps: vec![MacroMap {
                        pedal: PedalId(0),
                        key: "note".into(),
                        from: 12.0,
                        to: 18.0,
                    }],
                }],
            }));
            ("m1", 1.0)
        }
    };
    let effect = fx.id;
    let strip = t.strip.unwrap();
    p.tracks.push(t);
    p.strip_mut(strip).unwrap().effects.push(fx);
    let c = p.make_automation_clip(
        "a",
        AutoTarget::Effect {
            effect,
            key: key.into(),
        },
        Ticks::beats(4),
        Ticks::beats(8),
        vec![AutoPoint::new(Ticks::ZERO, pos)],
    );
    p.tracks[0].clips.push(c);
    p
}

fn offline(p: &Project) -> Vec<f32> {
    render(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            sample_rate: SR,
            end: Some(Ticks::beats(8)),
            tail_seconds: 0.5,
            block_frames: 256,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

/// The live engine over `p` (varied blocks), its events answered after each block as the
/// backend does.
fn real_time(p: &Project, frames: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        let unit = EffectUnit::new(slot, SR as f32);
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(frames * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::EffectNeed {
                effect,
                pedal,
                need,
            } = ev
            {
                let memory = EffectMemory::build(need.need(), SR as f32);
                tx.push(EngineMessage::EffectMemory {
                    effect,
                    pedal,
                    memory,
                })
                .unwrap();
            }
        }
    }
    out
}

/// The loudest sample on the left between `from` and `to` seconds.
fn peak(x: &[f32], from: f64, to: f64) -> f32 {
    let (a, b) = (
        (from * f64::from(SR)) as usize,
        (to * f64::from(SR)) as usize,
    );
    (a..b.min(x.len() / 2)).fold(0.0, |m, i| m.max(x[i * 2].abs()))
}

#[test]
fn an_automated_delay_on_a_board_holds_its_line_as_an_inserted_one() {
    let inserted = offline(&project(Where::Inserted));
    let quarter = peak(&inserted, 1.0, 1.3);
    assert!(quarter > 0.05, "the quarter's echo at 1 s: {quarter}");
    let whole = peak(&inserted, 4.0, 4.3);
    assert!(
        whole > quarter * 0.5,
        "the whole note's echo of beat 0 at 4 s (the floor's line): {whole}"
    );
    for at in [Where::Pedal, Where::Macro] {
        let name = if at == Where::Pedal {
            "the pedal's p0.note"
        } else {
            "macro 1 on the pedal's note"
        };
        let p = project(at);
        let board = offline(&p);
        assert_null(&inserted, &board, 0.0, &format!("{name}: offline"));
        let live = real_time(&p, board.len() / 2);
        assert_null(&board, &live, 0.0, &format!("{name}: real time"));
    }
}
