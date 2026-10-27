//! Note effects in the engine (round 31, ADR 0069): an empty or switched-off chain, and a
//! scale with no key, render as no chain to the bit; an arpeggio, chords and a scale render
//! exactly as the notes they bake to; at every block size, in real time as offline, on the
//! pool as serially; stems and a freeze hear them; live input goes through them, also
//! stopped; their parameters are automatable.

use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::notefx::{
    Arp, ArpMode, Chord, ChordShape, NoteEffect, NoteFxId, NoteFxKind, ScaleFx,
};
use cantus_core::*;
use cantus_engine::freeze::render::prepare;
use cantus_engine::notefx::bake::BakedNote;
use cantus_engine::stems::{render_stems, StemOptions, StemSource, StemSpec};
use cantus_engine::*;
use std::sync::Arc;

const SR: f64 = 48_000.0;

fn fx(id: u64, kind: NoteFxKind) -> NoteEffect {
    NoteEffect {
        id: NoteFxId(id),
        enabled: true,
        kind,
    }
}

fn arp(id: u64, f: impl FnOnce(&mut Arp)) -> NoteEffect {
    let mut a = Arp {
        gate: 0.5,
        ..Arp::default()
    };
    f(&mut a);
    fx(id, NoteFxKind::Arp(a))
}

/// A Firmus lead holding four chords of two beats each (C, Am, F, G), the last one tied
/// over a short gap; a Pulsus kit beside it, four on the floor.
fn song(chain: Vec<NoteEffect>) -> Project {
    let mut p = Project::new("notefx");
    p.tempo_bpm = 128.0;
    let mut lead = p.make_track("Lead");
    let mut c = p.make_clip("Chords", Ticks::ZERO, Ticks::beats(8));
    let chords: [[u8; 3]; 4] = [[60, 64, 67], [57, 60, 64], [53, 57, 60], [55, 59, 62]];
    for (i, ch) in chords.iter().enumerate() {
        for (k, pitch) in ch.iter().enumerate() {
            let start = Ticks::beats(2 * i as i64) + Ticks(k as i64 * 3);
            let n = p.make_note(
                start,
                Ticks::beats(2) - Ticks(40),
                *pitch,
                80 + 10 * k as u8,
            );
            c.notes.push(n);
        }
    }
    lead.clips.push(c);
    lead.note_fx = chain;
    p.tracks.push(lead);
    p
}

fn snap(p: &Project) -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(p, 1))
}

fn opts(block: usize) -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.5,
        end: Some(Ticks::beats(8)),
        block_frames: block,
        ..Default::default()
    }
}

fn render_at(p: &Project, block: usize) -> Vec<f32> {
    render(snap(p), &opts(block)).unwrap().samples
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
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

/// The same song with the notes its lead's chain plays written out, and no chain.
fn baked(p: &Project) -> Project {
    let s = Snapshot::compile(p, 1);
    let t = &s.tracks[0];
    let notes: Vec<BakedNote> =
        cantus_engine::notefx::bake(&t.note_fx, s.key, &t.events, &t.direct, Ticks::beats(8));
    let mut q = p.clone();
    q.tracks[0].note_fx.clear();
    let written: Vec<Note> = notes
        .iter()
        .map(|n| q.make_note(n.start, n.length, n.pitch, n.velocity))
        .collect();
    q.tracks[0].clips[0].notes = written;
    q
}

#[test]
fn an_empty_chain_a_switched_off_one_and_a_keyless_scale_play_as_no_chain_to_the_bit() {
    let plain = render_at(&song(vec![]), 512);
    let mut off = arp(1, |_| {});
    off.enabled = false;
    same_bits(&render_at(&song(vec![off]), 512), &plain, "switched off");
    // A scale with no key anywhere passes every note: through the chain, the same bits.
    let wire = fx(2, NoteFxKind::Scale(ScaleFx::default()));
    same_bits(
        &render_at(&song(vec![wire.clone()]), 512),
        &plain,
        "keyless scale",
    );
    // A kit through it too: its seeded hits keep their ids, ticks and ordinals.
    let kit = |chain: Vec<NoteEffect>| {
        let mut p = song(chain);
        p.tracks[0].instrument = InstrumentSpec::Pulsus(PulsusParams::default());
        let pads: Vec<u8> = PulsusParams::default()
            .pads
            .iter()
            .map(|x| x.pitch)
            .collect();
        let hits: Vec<Note> = (0..16)
            .map(|i| {
                let pitch = pads[i % pads.len()];
                p.make_note(Ticks::EIGHTH * i as i64, Ticks::SIXTEENTH, pitch, 100)
            })
            .collect();
        p.tracks[0].clips[0].notes = hits;
        p
    };
    same_bits(
        &render_at(&kit(vec![wire]), 512),
        &render_at(&kit(vec![]), 512),
        "a kit through a keyless scale",
    );
}

#[test]
fn an_arpeggio_renders_as_the_notes_it_bakes_to_at_every_block_size() {
    let p = song(vec![arp(1, |a| {
        a.mode = ArpMode::UpDown;
        a.octaves = 2;
        a.rate = 12;
    })]);
    let reference = render_at(&p, 512);
    let plain = render_at(&song(vec![]), 512);
    assert!(
        reference.iter().zip(&plain).any(|(a, b)| a != b),
        "the arpeggio is heard"
    );
    same_bits(&render_at(&baked(&p), 512), &reference, "its baked notes");
    for block in [1usize, 7, 64, 1000, 4096] {
        same_bits(&render_at(&p, block), &reference, &format!("block {block}"));
    }
}

#[test]
fn chords_into_an_arpeggio_with_swing_and_random_are_block_size_independent() {
    let key = Some(Key::new(0, ScaleKind::Major));
    let chain = vec![
        fx(
            1,
            NoteFxKind::Chord(Chord {
                shape: ChordShape::ScaleSeventh,
                spread: 1,
                ..Chord::default()
            }),
        ),
        arp(2, |a| {
            a.mode = ArpMode::Random;
            a.seed = 42;
            a.rate = 14;
            a.swing = 0.62;
            a.gate = 1.3;
        }),
        fx(3, NoteFxKind::Scale(ScaleFx::default())),
    ];
    let mut p = song(chain);
    p.key = key;
    let reference = render_at(&p, 512);
    for block in [7usize, 64, 333, 4096] {
        same_bits(&render_at(&p, block), &reference, &format!("block {block}"));
    }
}

#[test]
fn chords_and_a_scale_render_as_their_baked_notes() {
    let mut p = song(vec![
        fx(
            1,
            NoteFxKind::Chord(Chord {
                shape: ChordShape::Minor9,
                inversion: 2,
                ..Chord::default()
            }),
        ),
        fx(2, NoteFxKind::Scale(ScaleFx::default())),
    ]);
    p.key = Some(Key::new(9, ScaleKind::Minor));
    same_bits(
        &render_at(&baked(&p), 256),
        &render_at(&p, 256),
        "chords and a scale",
    );
}

/// The processor driven the way the audio callback drives it — blocks of every size, on
/// `workers` threads — from tick 0 for `total` frames.
fn live_render(s: &Arc<Snapshot>, total: usize, workers: Option<usize>) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    if let Some(n) = workers {
        p.set_workers(Workers::parallel(n));
    }
    let mut live = (**s).clone();
    live.loop_region = None;
    live.metronome = false;
    let live = Arc::new(live);
    for unit in cantus_engine::inst::units_for(&live, SR as f32, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&live)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(live)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17, 5];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

#[test]
fn real_time_and_the_pool_play_what_the_offline_render_plays() {
    let mut p = song(vec![
        fx(
            1,
            NoteFxKind::Chord(Chord {
                shape: ChordShape::Sus4,
                ..Chord::default()
            }),
        ),
        arp(2, |a| {
            a.mode = ArpMode::Played;
            a.rate = 11;
            a.swing = 0.58;
        }),
    ]);
    // A second arpeggiated track, so the pool has two to share out.
    let mut other = p.tracks[0].clone();
    other.id = TrackId(p.alloc_id());
    other.note_fx = vec![arp(3, |a| a.mode = ArpMode::Down)];
    p.tracks.push(other);
    let s = snap(&p);
    let offline = render(s.clone(), &opts(256)).unwrap();
    let frames = offline.frames();
    same_bits(
        &live_render(&s, frames, None),
        &offline.samples,
        "real time",
    );
    same_bits(
        &live_render(&s, frames, Some(3)),
        &offline.samples,
        "on the pool",
    );
}

#[test]
fn a_tracks_stem_hears_its_note_effects() {
    let p = song(vec![arp(1, |a| a.octaves = 3)]);
    let spec = |p: &Project| {
        let s = snap(p);
        render_stems(
            s,
            &StemOptions {
                render: opts(512),
                keep_silent: true,
                through_master: false,
            },
            &[StemSpec {
                source: StemSource::Track(0),
                file_name: "01 Lead.wav".into(),
            }],
        )
        .unwrap()
        .remove(0)
        .samples
    };
    same_bits(&spec(&p), &spec(&baked(&p)), "the lead's stem");
}

#[test]
fn a_frozen_track_plays_its_arpeggio_to_the_bit() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("notefx-freeze");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (mut h, _p) = EngineHandle::manual(SR);
    h.frozen.set_app_dir(dir);
    let mut p = song(vec![arp(1, |a| a.mode = ArpMode::DownUp)]);
    let mix = |h: &mut EngineHandle, p: &Project| {
        let snap = h.compile_for_render(p, 1);
        render(
            snap,
            &RenderOptions {
                tail_seconds: 1.0,
                compensate_latency: true,
                ..Default::default()
            },
        )
        .unwrap()
        .samples
    };
    let live = mix(&mut h, &p);
    let target = FreezeTarget::Track(p.tracks[0].id);
    let job = prepare(&mut h, &p, target).expect("prepared");
    job.run(Vec::new(), Vec::new(), &RenderProgress::default())
        .expect("rendered");
    h.frozen.forget_missing();
    freeze::freeze_commands(&p, target)
        .unwrap()
        .apply(&mut p)
        .unwrap();
    assert!(p.tracks[0].frozen.is_some());
    same_bits(&mix(&mut h, &p), &live, "frozen");
}

fn octaves_target(p: &Project) -> AutoTarget {
    AutoTarget::NoteFx {
        track: p.tracks[0].id,
        effect: p.tracks[0].note_fx[0].id,
        key: "octaves".into(),
    }
}

#[test]
fn a_note_effects_parameter_is_automatable() {
    // Octaves held at 3 by an automation clip renders as the chain set to 3.
    let mut p = song(vec![arp(1, |_| {})]);
    let target = octaves_target(&p);
    let at_three = 2.0 / 3.0;
    let clip = p.make_automation_clip(
        "octaves",
        target.clone(),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint::new(Ticks::ZERO, at_three),
            AutoPoint::new(Ticks::beats(8), at_three),
        ],
    );
    p.tracks[0].clips.push(clip);
    let three = song(vec![arp(1, |a| a.octaves = 3)]);
    same_bits(
        &render_at(&p, 512),
        &render_at(&three, 512),
        "automated to 3",
    );
    // A step from 1 to 4 halfway: heard, and the same at every block size.
    let mut q = song(vec![arp(1, |_| {})]);
    let clip = q.make_automation_clip(
        "climb",
        octaves_target(&q),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.0),
            AutoPoint {
                shape: Shape::Hold,
                ..AutoPoint::new(Ticks::beats(4), 0.0)
            },
            AutoPoint::new(Ticks::beats(4), 1.0),
        ],
    );
    q.tracks[0].clips.push(clip);
    let reference = render_at(&q, 512);
    assert_ne!(reference, render_at(&song(vec![arp(1, |_| {})]), 512));
    for block in [7usize, 1000] {
        same_bits(
            &render_at(&q, block),
            &reference,
            &format!("automated, block {block}"),
        );
    }
}

#[test]
fn a_modulator_moves_a_note_effects_parameter() {
    let mut p = song(vec![arp(1, |_| {})]);
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    m.routes = vec![ModRoute {
        target: AutoTarget::NoteFx {
            track: p.tracks[0].id,
            effect: NoteFxId(1),
            key: "gate".into(),
        },
        depth: 0.8,
    }];
    p.tracks[0].modulators.push(m);
    let reference = render_at(&p, 512);
    assert_ne!(
        reference,
        render_at(&song(vec![arp(1, |_| {})]), 512),
        "heard"
    );
    for block in [64usize, 4096] {
        same_bits(
            &render_at(&p, block),
            &reference,
            &format!("modulated, block {block}"),
        );
    }
}

/// A processor with `p`'s snapshot installed, stopped, its first track live.
fn stopped(p: &Project) -> (rtrb::Producer<EngineMessage>, Processor, TrackId) {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc_ = Processor::without_midi(SR, rx_engine, tx_engine);
    std::mem::forget(_rx);
    let s = snap(p);
    for unit in cantus_engine::inst::units_for(&s, SR as f32, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(s)).unwrap();
    let track = p.tracks[0].id;
    let mut warm = vec![0f32; 64 * 2];
    proc_.process(&mut warm, 2);
    (tx, proc_, track)
}

fn run(p: &mut Processor, frames: usize, out: &mut Vec<f32>) {
    let mut done = 0;
    while done < frames {
        let n = (frames - done).min(512);
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        done += n;
    }
}

#[test]
fn live_keys_play_through_a_chord_as_the_chord_played_by_hand() {
    let mut empty = song(vec![]);
    empty.tracks[0].clips.clear();
    let mut with = empty.clone();
    with.tracks[0].note_fx = vec![fx(1, NoteFxKind::Chord(Chord::default()))];
    let (mut tx, mut a, track) = stopped(&with);
    let (mut ty, mut b, _) = stopped(&empty);
    let (mut oa, mut ob) = (Vec::new(), Vec::new());
    tx.push(EngineMessage::NoteOn {
        track,
        pitch: 60,
        velocity: 100,
    })
    .unwrap();
    for pitch in [60u8, 64, 67] {
        ty.push(EngineMessage::NoteOn {
            track,
            pitch,
            velocity: 100,
        })
        .unwrap();
    }
    run(&mut a, 6000, &mut oa);
    run(&mut b, 6000, &mut ob);
    tx.push(EngineMessage::NoteOff { track, pitch: 60 })
        .unwrap();
    for pitch in [60u8, 64, 67] {
        ty.push(EngineMessage::NoteOff { track, pitch }).unwrap();
    }
    run(&mut a, 24_000, &mut oa);
    run(&mut b, 24_000, &mut ob);
    same_bits(&oa, &ob, "a live chord");
}

#[test]
fn a_live_arpeggio_runs_while_stopped_on_the_sample_clock() {
    // At 120 BPM a 16th is 6 000 samples at 48 kHz; at gate 50 % each note is 3 000 long.
    let mut empty = song(vec![]);
    empty.tempo_bpm = 120.0;
    empty.tracks[0].clips.clear();
    let mut with = empty.clone();
    with.tracks[0].note_fx = vec![arp(1, |_| {})];
    let (mut tx, mut a, track) = stopped(&with);
    let (mut ty, mut b, _) = stopped(&empty);
    let (mut oa, mut ob) = (Vec::new(), Vec::new());
    for pitch in [64u8, 60] {
        tx.push(EngineMessage::NoteOn {
            track,
            pitch,
            velocity: 100,
        })
        .unwrap();
    }
    run(&mut a, 24_000, &mut oa);
    // By hand: C, E, C, E, each 3 000 samples, a 16th apart.
    for k in 0..4 {
        let pitch = if k % 2 == 0 { 60 } else { 64 };
        ty.push(EngineMessage::NoteOn {
            track,
            pitch,
            velocity: 100,
        })
        .unwrap();
        run(&mut b, 3_000, &mut ob);
        ty.push(EngineMessage::NoteOff { track, pitch }).unwrap();
        run(&mut b, 3_000, &mut ob);
    }
    same_bits(&oa, &ob, "a stopped arpeggio");
}

/// Two chords of a bar each, the second ending on the bar line where the loop ends (its
/// note-offs fall on the loop's last sample), through chords into an arpeggio.
fn looping(copies: usize) -> Project {
    let mut p = Project::new("loop");
    p.tempo_bpm = 128.0;
    let mut lead = p.make_track("Lead");
    for k in 0..copies {
        let at = Ticks::beats(8 * k as i64);
        let mut c = p.make_clip("Bars", at, Ticks::beats(8));
        for (bar, chord) in [[60u8, 64, 67], [57, 60, 64]].iter().enumerate() {
            for pitch in chord {
                let n = p.make_note(Ticks::beats(4 * bar as i64), Ticks::beats(4), *pitch, 90);
                c.notes.push(n);
            }
        }
        lead.clips.push(c);
    }
    lead.note_fx = vec![
        fx(
            1,
            NoteFxKind::Chord(Chord {
                shape: ChordShape::Sus4,
                ..Chord::default()
            }),
        ),
        arp(2, |a| {
            a.mode = ArpMode::UpDown;
            a.rate = 12;
        }),
    ];
    p.tracks.push(lead);
    p
}

/// The processor driven by blocks of `sizes`, the loop kept, from tick 0 for `total` frames.
fn loop_render(s: &Arc<Snapshot>, total: usize, sizes: &[usize]) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    for unit in cantus_engine::inst::units_for(s, SR as f32, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(s.clone())).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(total * 2);
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

#[test]
fn a_looped_arpeggio_plays_each_pass_as_the_notes_laid_out_again() {
    let mut once = looping(1);
    once.loop_region = Some(LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(8),
    });
    once.loop_enabled = true;
    // Two passes of the loop, against the same bars written twice and played straight.
    let frames = (16.0 * 60.0 / 128.0 * SR) as usize;
    let twice = render(
        snap(&looping(2)),
        &RenderOptions {
            tail_seconds: 0.0,
            end: Some(Ticks::beats(16)),
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    let twice = &twice[..frames * 2];
    for sizes in [&[512usize][..], &[7, 333, 64], &[1000, 4096, 17]] {
        let looped = loop_render(&snap(&once), frames, sizes);
        same_bits(&looped, twice, &format!("looped, blocks {sizes:?}"));
    }
}
