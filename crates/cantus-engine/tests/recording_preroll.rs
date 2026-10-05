//! Round 19 fixes on the recording path: the count-in before bar 1 plays in full (the
//! transport pre-rolls before zero), and every armed instrument track sounds the live
//! notes while it is recorded into (multitrack MIDI recording).

use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

const SR: u32 = 48_000;

struct Rig {
    p: Processor,
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
}

impl Rig {
    fn new(snap: Snapshot) -> Rig {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let p = Processor::without_midi(SR as f64, rx_engine, tx_engine);
        tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
        Rig { p, tx, rx }
    }

    /// Render `frames` frames; the left channel.
    fn run(&mut self, frames: usize) -> Vec<f32> {
        let mut left = Vec::with_capacity(frames);
        let mut out = vec![0f32; 500 * 2];
        while left.len() < frames {
            self.p.process(&mut out, 2);
            left.extend(out.chunks_exact(2).map(|f| f[0]));
        }
        left.truncate(frames);
        left
    }

    fn events(&mut self) -> Vec<EngineEvent> {
        let mut v = Vec::new();
        while let Ok(e) = self.rx.pop() {
            v.push(e);
        }
        v
    }
}

fn reference_click(frames: usize) -> Vec<f32> {
    let mut c = cantus_dsp::Click::new(SR as f32);
    (0..frames)
        .map(|i| {
            if i % 24_000 == 0 {
                c.trigger(i % 96_000 == 0);
            }
            c.next()
        })
        .collect()
}

#[test]
fn a_count_in_before_bar_1_plays_in_full() {
    // Recording at bar 1 with a bar of count-in: the transport starts a bar before zero,
    // all four clicks of the count-in sound, and the timeline begins at frame 96 000.
    let mut p = Project::new("preroll");
    p.tempo_bpm = 120.0;
    p.metronome = true;
    p.recording.click = ClickMode::CountInOnly;
    let mut rig = Rig::new(Snapshot::compile(&p, 1));
    rig.tx.push(EngineMessage::Seek(Ticks::beats(-4))).unwrap();
    rig.tx
        .push(EngineMessage::SetRecordStart(Some(Ticks::ZERO)))
        .unwrap();
    rig.tx.push(EngineMessage::SetRecording(true)).unwrap();
    rig.tx.push(EngineMessage::Play).unwrap();
    let got = rig.run(200_000);
    let want = reference_click(200_000);
    assert!(want[..96_000].iter().any(|x| *x != 0.0));
    assert_eq!(
        &got[..96_000],
        &want[..96_000],
        "the whole count-in is heard"
    );
    assert!(
        got[96_000..].iter().all(|x| *x == 0.0),
        "and stops at bar 1"
    );
    let last = rig
        .events()
        .into_iter()
        .filter_map(|e| match e {
            EngineEvent::Position { tick, .. } => Some(tick),
            _ => None,
        })
        .next_back()
        .unwrap();
    // 200 000 frames from a bar before zero: 104 000 frames into the timeline.
    assert_eq!(last, Ticks((104_000 / 25) as i64));
}

fn two_synths() -> (Project, TrackId, TrackId) {
    let mut p = Project::new("echo");
    p.tempo_bpm = 120.0;
    let mut a = p.make_track("A");
    a.strip = None;
    let mut b = p.make_track("B");
    b.strip = None;
    let (ia, ib) = (a.id, b.id);
    p.tracks.push(a);
    p.tracks.push(b);
    (p, ia, ib)
}

/// Play one live note on `a` and return the voices sounding and the notes reported.
fn play(echo: &[TrackId]) -> (usize, usize) {
    let (p, a, _) = two_synths();
    let mut rig = Rig::new(Snapshot::compile(&p, 1));
    let mut e = [None; MAX_LIVE_ECHO];
    for (o, t) in e.iter_mut().zip(echo) {
        *o = Some(*t);
    }
    rig.tx.push(EngineMessage::SetLiveTrack(Some(a))).unwrap();
    rig.tx.push(EngineMessage::SetLiveEcho(e)).unwrap();
    rig.tx.push(EngineMessage::SetRecording(true)).unwrap();
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.run(500);
    rig.tx
        .push(EngineMessage::NoteOn {
            track: a,
            pitch: 60,
            velocity: 100,
        })
        .unwrap();
    rig.run(500);
    let voices = rig.p.stats().active_voices;
    let notes = rig
        .events()
        .into_iter()
        .filter(|e| matches!(e, EngineEvent::NoteInput(_)))
        .count();
    (voices, notes)
}

#[test]
fn every_armed_instrument_track_sounds_the_live_notes() {
    let (_, a, b) = two_synths();
    let (alone, n1) = play(&[]);
    assert!(alone > 0);
    let (both, n2) = play(&[a, b]);
    assert_eq!(both, alone * 2, "the echo track plays the note too");
    // Reported once, for the live track: the UI copies it into every armed track.
    assert_eq!((n1, n2), (1, 1));
}

#[test]
fn a_track_that_stops_echoing_lets_go() {
    let (p, a, b) = two_synths();
    let mut rig = Rig::new(Snapshot::compile(&p, 1));
    let mut e = [None; MAX_LIVE_ECHO];
    e[0] = Some(b);
    rig.tx.push(EngineMessage::SetLiveTrack(Some(a))).unwrap();
    rig.tx.push(EngineMessage::SetLiveEcho(e)).unwrap();
    rig.tx
        .push(EngineMessage::NoteOn {
            track: a,
            pitch: 64,
            velocity: 90,
        })
        .unwrap();
    rig.run(500);
    let held = rig.p.stats().active_voices;
    rig.tx
        .push(EngineMessage::SetLiveEcho([None; MAX_LIVE_ECHO]))
        .unwrap();
    rig.run(48_000);
    assert!(rig.p.stats().active_voices < held);
}
