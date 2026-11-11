//! Slot parts built off the audio thread (ADR 0107): a track slot holds a Pulsus kit only
//! while its track hosts Pulsus, and a note-effect chain only while it has note effects. The
//! engine is sent the parts a snapshot uses before it, keeps every part it is given and
//! moves them between slots; a drum track whose kit has not come is silent, asks once, and
//! plays from the block its kit arrives.

use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

const SR: f64 = 48_000.0;

/// A drum track of a bar of kicks and snares, added to `p`.
fn add_drums(p: &mut Project, name: &str) {
    let mut t = p.make_track(name);
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let mut clip = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(8));
    for s in 0..16i64 {
        let pitch = [36u8, 38][(s % 2) as usize];
        clip.notes
            .push(p.make_note(Ticks::EIGHTH * s, Ticks::SIXTEENTH, pitch, 100));
    }
    t.clips.push(clip);
    p.tracks.push(t);
}

/// `n` drum tracks.
fn drums(n: usize) -> Project {
    let mut p = Project::new("kits");
    p.tempo_bpm = 120.0;
    for k in 0..n {
        add_drums(&mut p, &format!("Drums {k}"));
    }
    p
}

struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    p: Processor,
    /// The kits `PartsNeed` asked for, in order.
    asked: Vec<usize>,
    /// The slots `CellsNeed` asked for, in order.
    cells_asked: Vec<(usize, usize)>,
}

impl Rig {
    fn new() -> Self {
        let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(512);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        Rig {
            tx,
            rx,
            p: Processor::without_midi(SR, rx_engine, tx_engine),
            asked: Vec::new(),
            cells_asked: Vec::new(),
        }
    }

    /// The snapshot, with `kits` sent before it as the backend sends them.
    fn send(&mut self, p: &Project, kits: usize) {
        let snap = Arc::new(Snapshot::compile(p, 1));
        for _ in 0..kits {
            let kit = cantus_engine::parts::kit(SR as f32);
            self.tx.push(EngineMessage::AddPart(kit)).unwrap();
        }
        self.tx
            .push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .unwrap();
        self.tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    }

    fn start(&mut self) {
        self.tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
        self.tx.push(EngineMessage::Play).unwrap();
    }

    fn play(&mut self, blocks: usize) -> Vec<f32> {
        let mut out = Vec::new();
        for _ in 0..blocks {
            let mut buf = vec![0f32; 256 * 2];
            self.p.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            while let Ok(ev) = self.rx.pop() {
                if let EngineEvent::CellsNeed { tracks, strips } = ev {
                    self.cells_asked.push((tracks, strips));
                } else if let EngineEvent::PartsNeed { kits: count, .. } = ev {
                    self.asked.push(count);
                }
            }
        }
        out
    }
}

fn loud(x: &[f32]) -> bool {
    x.iter().any(|v| v.abs() > 1e-3)
}

#[test]
fn an_engine_before_any_song_holds_no_kit() {
    let r = Rig::new();
    assert_eq!(r.p.parts_held().kits, 0);
}

/// A kit sent in a message and a kit built in place (an offline render's way) play the
/// same bits, and the drum track sounds.
#[test]
fn a_kit_sent_plays_as_a_kit_built_in_place() {
    let p = drums(2);
    let mut a = Rig::new();
    a.send(&p, 2);
    a.start();
    let x = a.play(200);
    let mut b = Rig::new();
    b.p.hold_parts_for(&Snapshot::compile(&p, 1));
    b.send(&p, 0);
    b.start();
    let y = b.play(200);
    assert!(loud(&x), "the drums sound");
    assert_eq!(x, y);
    assert!(a.asked.is_empty() && b.asked.is_empty(), "nobody asked");
    assert_eq!((a.p.parts_held().kits, b.p.parts_held().kits), (2, 2));
    assert_eq!(a.p.stats().kits_waiting, 0);
}

/// Sent without its kit, a drum track is silent and asks once — again and again the same
/// shortfall is not asked twice — and plays from the block its kit arrives.
#[test]
fn a_drum_track_without_its_kit_is_silent_asks_once_and_plays_when_it_comes() {
    let p = drums(1);
    let mut r = Rig::new();
    r.send(&p, 0);
    r.start();
    let quiet = r.play(40);
    assert!(!loud(&quiet), "no kit, no sound");
    assert_eq!(r.p.stats().kits_waiting, 1);
    r.send(&p, 0);
    r.play(10);
    assert_eq!(r.asked, vec![1], "asked once");
    r.tx.push(EngineMessage::AddPart(cantus_engine::parts::kit(SR as f32)))
        .unwrap();
    let after = r.play(200);
    assert!(loud(&after), "its kit came: it plays");
    assert_eq!(r.p.stats().kits_waiting, 0);
    assert_eq!(r.asked, vec![1]);
}

/// A track that stops hosting Pulsus gives its kit back and a new drum track takes it:
/// two kits serve every edit that keeps two drum tracks, and nothing is asked for.
#[test]
fn kits_move_between_slots_and_are_never_asked_for_twice() {
    let mut p = drums(2);
    let mut r = Rig::new();
    r.send(&p, 2);
    r.start();
    r.play(20);
    // The first becomes a synth; a third drum track comes.
    p.tracks[0].instrument = InstrumentSpec::Firmus(Default::default());
    add_drums(&mut p, "Drums 2");
    r.send(&p, 0);
    let out = r.play(200);
    assert!(r.asked.is_empty(), "asked {:?}", r.asked);
    assert_eq!(r.p.parts_held().kits, 2);
    assert_eq!(r.p.stats().kits_waiting, 0);
    assert!(loud(&out));
    // Every track gone: the kits stay, spare.
    p.tracks.clear();
    r.send(&p, 0);
    r.play(4);
    assert_eq!(r.p.parts_held().kits, 2);
}

/// The application's way: the backend sends a kit per drum track before the snapshot,
/// once — the engine keeps them — and an engine that played two drum tracks needs no new
/// kit for a song that has two again.
#[test]
fn the_backend_sends_each_kit_once() {
    let (mut h, mut proc_) = EngineHandle::manual(SR);
    let mut p = drums(2);
    h.set_snapshot(Arc::new(Snapshot::compile(&p, 1)));
    assert_eq!(h.parts_sent().kits, 2);
    let mut buf = vec![0f32; 512];
    proc_.process(&mut buf, 2);
    h.poll();
    assert_eq!(proc_.parts_held().kits, 2);
    p.tracks.pop();
    h.set_snapshot(Arc::new(Snapshot::compile(&p, 1)));
    proc_.process(&mut buf, 2);
    add_drums(&mut p, "Again");
    h.set_snapshot(Arc::new(Snapshot::compile(&p, 1)));
    proc_.process(&mut buf, 2);
    h.poll();
    assert_eq!(h.parts_sent().kits, 2, "the engine had them");
    assert_eq!(proc_.parts_held().kits, 2);
    assert_eq!(proc_.stats().kits_waiting, 0);
}

/// A synth track with an arpeggiator (ADR 0069) on it.
fn arpeggio() -> Project {
    use cantus_core::notefx::{Arp, NoteEffect, NoteFxId, NoteFxKind};
    let mut p = Project::new("arp");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Lead");
    let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(8));
    for pitch in [60u8, 64, 67] {
        clip.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(8), pitch, 100));
    }
    t.clips.push(clip);
    t.note_fx = vec![NoteEffect {
        id: NoteFxId(1),
        enabled: true,
        kind: NoteFxKind::Arp(Arp {
            gate: 0.5,
            ..Arp::default()
        }),
    }];
    p.tracks.push(t);
    p
}

/// A track's note-effect chain is a part like a kit: sent ahead it plays as one built in
/// place; an engine before any song holds none; and a track that loses its note effects
/// gives its chain back for the next.
#[test]
fn a_note_effect_chain_is_given_while_its_track_has_note_effects() {
    let p = arpeggio();
    let mut a = Rig::new();
    assert_eq!(a.p.parts_held().note_fx, 0, "no chain before any song");
    a.tx.push(EngineMessage::AddPart(cantus_engine::parts::note_fx()))
        .unwrap();
    a.send(&p, 0);
    a.start();
    let x = a.play(300);
    let mut b = Rig::new();
    b.p.hold_parts_for(&Snapshot::compile(&p, 1));
    b.send(&p, 0);
    b.start();
    let y = b.play(300);
    assert!(loud(&x));
    assert_eq!(x, y, "a chain sent plays as a chain built in place");
    assert_eq!(a.p.stats().note_fx_waiting, 0);
    // Played without its chain the chord is held whole, not arpeggiated: it differs.
    let mut plain = arpeggio();
    plain.tracks[0].note_fx.clear();
    let mut c = Rig::new();
    c.send(&plain, 0);
    c.start();
    assert_ne!(c.play(300), x, "the arpeggio is heard");
    // Its note effects removed: the chain goes back, spare.
    let mut q = p.clone();
    q.tracks[0].note_fx.clear();
    a.send(&q, 0);
    a.play(4);
    assert_eq!(a.p.parts_held().note_fx, 1, "kept, spare");
}

/// `n` synth tracks of a held chord each, and `extra` more mixer strips.
fn wide(n: usize, extra: usize) -> Project {
    let mut p = Project::new("wide");
    p.tempo_bpm = 120.0;
    for k in 0..extra {
        let s = p.make_strip(format!("Bus {k}"));
        p.strips.push(s);
    }
    for k in 0..n {
        let mut t = p.make_track(format!("Synth {k}"));
        let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(8));
        let pitch = 40 + (k % 40) as u8;
        clip.notes
            .push(p.make_note(Ticks::beats((k % 4) as i64), Ticks::beats(2), pitch, 90));
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

/// ADR 0107: the track and strip slots grow with the song. A processor starts with a few
/// provisioned; a snapshot with more tracks and strips than that, sent without their
/// cells, waits — asking once — while the one before plays on, and installs the block its
/// cells arrive; sent with them (the backend's way) or held in place (an offline render's),
/// it plays the same bits.
#[test]
fn slots_grow_with_the_song_and_a_snapshot_waits_for_them() {
    let p = wide(40, 30);
    let snap = Snapshot::compile(&p, 1);
    let need = cantus_engine::parts::cells_needed(&snap);
    assert!(need.0 > cantus_engine::parts::START_TRACKS);
    assert!(need.1 > cantus_engine::parts::START_STRIPS);
    // Held in place.
    let mut a = Rig::new();
    assert_eq!(
        a.p.slots_ready(),
        (
            cantus_engine::parts::START_TRACKS,
            cantus_engine::parts::START_STRIPS
        )
    );
    a.p.hold_parts_for(&snap);
    a.send(&p, 0);
    a.start();
    let x = a.play(200);
    assert!(loud(&x));
    assert!(a.p.slots_ready().0 >= need.0 && a.p.slots_ready().1 >= need.1);
    // Sent ahead, as the backend sends them.
    let mut b = Rig::new();
    let more = (
        need.0 - cantus_engine::parts::START_TRACKS,
        need.1 - cantus_engine::parts::START_STRIPS,
    );
    let cells = cantus_engine::parts::SlotCells::new(SR as f32, more.0, more.1);
    b.tx.push(EngineMessage::Cells(Box::new(cells))).unwrap();
    b.send(&p, 0);
    b.start();
    assert_eq!(b.play(200), x, "cells sent play as cells held in place");
    assert!(b.cells_asked.is_empty());
    // Sent without: it waits and asks once; the cells come; it plays.
    let mut c = Rig::new();
    c.send(&p, 0);
    c.start();
    let quiet = c.play(20);
    assert!(!loud(&quiet), "the snapshot waits: nothing installed yet");
    assert_eq!(c.p.stats().cells_waiting, 1);
    c.send(&p, 0);
    c.play(5);
    assert_eq!(c.cells_asked, vec![need], "asked once");
    let cells = cantus_engine::parts::SlotCells::new(SR as f32, more.0, more.1);
    c.tx.push(EngineMessage::Cells(Box::new(cells))).unwrap();
    let after = c.play(200);
    assert!(loud(&after), "its cells came: it plays");
    assert_eq!(c.p.stats().cells_waiting, 0);
}

/// The application's way: the backend provisions the slots a snapshot needs before it,
/// once; nothing waits.
#[test]
fn the_backend_provisions_slots_before_the_snapshot() {
    let (mut h, mut proc_) = EngineHandle::manual(SR);
    let p = wide(40, 30);
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let need = cantus_engine::parts::cells_needed(&snap);
    h.set_snapshot(snap);
    assert_eq!(h.slots_sent(), need);
    let mut buf = vec![0f32; 512];
    proc_.process(&mut buf, 2);
    h.poll();
    assert_eq!(proc_.slots_ready(), need);
    assert_eq!(proc_.stats().cells_waiting, 0);
}
