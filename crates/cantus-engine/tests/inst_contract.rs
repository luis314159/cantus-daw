//! The instrument contract (ADR 0023), checked for every native kind at once, and the unit
//! lifecycle checked with a probe unit whose behaviour is known.
//!
//! Each instrument's own tests go further; these hold the family's floor: finite output
//! identical at every block size, headroom on a loud chord, exact silence once the notes
//! are over (so an idle track can be skipped), and units that arrive before the snapshot
//! that needs them and go back when it no longer does.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::inst as einst;
use cantus_engine::*;
use support::*;

fn with_native(kind: InstrumentKind) -> Project {
    let mut p = demo_project();
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(kind));
    p
}

/// The native track alone: the Firmus bass removed.
fn alone(kind: InstrumentKind) -> Project {
    let mut p = with_native(kind);
    p.tracks.truncate(1);
    p
}

#[test]
fn every_kind_is_finite_and_block_size_independent_at_its_defaults() {
    for kind in InstrumentKind::ALL {
        let p = with_native(kind);
        let reference = render_with(&p, 512);
        assert!(
            reference.iter().all(|x| x.is_finite()),
            "{kind:?} produced a sample that is not a number"
        );
        for block in [1usize, 64, 1000] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("{kind:?} at block size {block}"),
            );
        }
    }
}

#[test]
fn every_kind_keeps_headroom_on_a_loud_chord() {
    for kind in InstrumentKind::ALL {
        let mut p = alone(kind);
        let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(4));
        for pitch in [36u8, 48, 55, 60, 64, 67, 72, 79] {
            clip.notes
                .push(p.make_note(Ticks::ZERO, Ticks::beats(3), pitch, 127));
        }
        p.tracks[0].clips = vec![clip];
        let out = render_of(&p);
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(
            peak < 1.0,
            "{kind:?}: an eight-note chord at full velocity peaks at {peak} (0 dBFS or over)"
        );
    }
}

#[test]
fn every_kind_is_exactly_silent_once_its_notes_are_over() {
    for kind in InstrumentKind::ALL {
        let p = alone(kind);
        let opts = RenderOptions {
            tail_seconds: 6.0,
            block_frames: 256,
            ..Default::default()
        };
        let out = render(std::sync::Arc::new(Snapshot::compile(&p, 1)), &opts)
            .expect("render")
            .samples;
        let last = &out[out.len() - 48_000 * 2..];
        assert!(
            last.iter().all(|x| *x == 0.0),
            "{kind:?} still sounds six seconds after its last note: an idle track must reach \
             exact zero (flush denormals, gate the tail) so the engine can skip it"
        );
    }
}

// ---- the lifecycle, with a probe ----------------------------------------------------------

/// A unit that adds 0.25 per held note, and reports what it heard in its telemetry:
/// [0] voices, [8] the last controller value, [9] how many times it was configured.
#[derive(Debug, Default)]
struct Probe {
    held: Vec<u64>,
    last_controller: f32,
    configured: f32,
}

impl InstrumentUnit for Probe {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &einst::Assets) {
        self.configured += 1.0;
    }
    fn note_on(&mut self, note: NoteOn) {
        self.held.push(note.id);
    }
    fn note_off(&mut self, _key: u8, id: u64, _velocity: f32) {
        self.held.retain(|h| *h != id);
    }
    fn controller(&mut self, _c: Controller, value: f32) {
        self.last_controller = value;
    }
    fn all_notes_off(&mut self) {
        self.held.clear();
    }
    fn reset(&mut self) {
        self.held.clear();
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        if self.held.is_empty() {
            return false;
        }
        let v = 0.25 * self.held.len() as f32;
        left.iter_mut().for_each(|x| *x += v);
        right.iter_mut().for_each(|x| *x += v);
        true
    }
    fn active_voices(&self) -> usize {
        self.held.len()
    }
    fn telemetry(&self) -> einst::Telemetry {
        let mut t = [0.0; einst::TELEMETRY_LEN];
        t[0] = self.held.len() as f32;
        t[8] = self.last_controller;
        t[9] = self.configured;
        t
    }
}

struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    proc: Processor,
}

impl Rig {
    fn new() -> Self {
        let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        Rig {
            tx,
            rx,
            proc: Processor::without_midi(48_000.0, rx_engine, tx_engine),
        }
    }
    fn send(&mut self, m: EngineMessage) {
        assert!(self.tx.push(m).is_ok());
    }
    fn probe(&mut self, track: TrackId, kind: InstrumentKind, serial: u64) {
        self.send(EngineMessage::AddInstrument(Box::new(NativeUnit {
            track,
            kind,
            serial,
            unit: Box::new(Probe::default()),
        })));
    }
    fn snapshot(&mut self, p: &Project, rev: u64) {
        let mut s = Snapshot::compile(p, rev);
        s.loop_region = None;
        s.metronome = false;
        self.send(EngineMessage::SetSnapshot(std::sync::Arc::new(s)));
    }
    fn run(&mut self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        for chunk in out.chunks_mut(200) {
            self.proc.process(chunk, 2);
        }
        out
    }
    /// Serials handed back, and the last telemetry seen.
    fn events(&mut self) -> (Vec<u64>, einst::Telemetry) {
        let mut garbage = Vec::new();
        let mut tel = [0.0; einst::TELEMETRY_LEN];
        while let Ok(e) = self.rx.pop() {
            match e {
                EngineEvent::GarbageInstrument(u) => garbage.push(u.serial),
                EngineEvent::Meters(m) => tel = m.inst,
                _ => {}
            }
        }
        (garbage, tel)
    }
}

fn one_note_project(kind: InstrumentKind) -> Project {
    let mut p = Project::new("probe");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Probe");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
    let mut clip = p.make_clip("One", Ticks::ZERO, Ticks::beats(4));
    clip.notes
        .push(p.make_note(Ticks::beats(1), Ticks::beats(1), 60, 100));
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

#[test]
fn a_delivered_unit_plays_its_notes_sample_accurately() {
    let p = one_note_project(InstrumentKind::Organ);
    let track = p.tracks[0].id;
    let mut rig = Rig::new();
    rig.probe(track, InstrumentKind::Organ, 1);
    rig.snapshot(&p, 1);
    rig.send(EngineMessage::Play);
    let out = rig.run(48_000);
    let first = out.iter().position(|x| *x != 0.0).map(|i| i / 2);
    // Beat 1 at 120 BPM is half a second: sample 24 000, whatever the block size.
    assert_eq!(first, Some(24_000));
}

#[test]
fn units_go_back_when_the_track_changes_instrument_or_nobody_claims_them() {
    let mut p = one_note_project(InstrumentKind::Organ);
    let track = p.tracks[0].id;
    let mut rig = Rig::new();
    rig.probe(track, InstrumentKind::Organ, 1);
    rig.snapshot(&p, 1);
    rig.run(400);
    assert_eq!(
        rig.events().0,
        Vec::<u64>::new(),
        "the first unit is in use"
    );

    // Another kind: its unit replaces the first, which comes back.
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Fm));
    rig.probe(track, InstrumentKind::Fm, 2);
    rig.snapshot(&p, 2);
    rig.run(400);
    assert_eq!(rig.events().0, vec![1]);

    // The same kind again, with a new value: the unit stays (voices keep ringing).
    p.tracks[0]
        .instrument
        .native_mut()
        .unwrap()
        .set("gain", -6.0);
    rig.snapshot(&p, 3);
    rig.run(400);
    assert_eq!(rig.events().0, Vec::<u64>::new());

    // Firmus: the native unit goes back.
    p.tracks[0].instrument = InstrumentSpec::default();
    rig.snapshot(&p, 4);
    rig.run(400);
    assert_eq!(rig.events().0, vec![2]);

    // A unit no snapshot claims is stale, and goes back with the next snapshot.
    rig.probe(track, InstrumentKind::Sampler, 3);
    rig.snapshot(&p, 5);
    rig.run(400);
    assert_eq!(rig.events().0, vec![3]);
}

#[test]
fn the_probed_track_reports_its_telemetry_and_live_controllers_arrive() {
    let p = one_note_project(InstrumentKind::Organ);
    let track = p.tracks[0].id;
    let mut rig = Rig::new();
    rig.probe(track, InstrumentKind::Organ, 1);
    rig.snapshot(&p, 1);
    rig.send(EngineMessage::SetInstrumentProbe(Some(track)));
    rig.send(EngineMessage::SetLiveTrack(Some(track)));
    rig.send(EngineMessage::NoteOn {
        track,
        pitch: 64,
        velocity: 90,
    });
    rig.send(EngineMessage::NoteOn {
        track,
        pitch: 67,
        velocity: 90,
    });
    rig.send(EngineMessage::Midi(MidiEvent {
        status: 0xB0,
        data1: 64,
        data2: 127,
    }));
    rig.run(400);
    let (_, tel) = rig.events();
    assert_eq!(tel[0], 2.0, "two live notes held, told apart by their ids");
    assert_eq!(tel[8], 1.0, "the sustain pedal arrived as a controller");
    assert!(tel[9] >= 1.0, "the unit was configured by the snapshot");
    rig.send(EngineMessage::NoteOff { track, pitch: 64 });
    rig.run(400);
    assert_eq!(
        rig.events().1[0],
        1.0,
        "the note-off released one note, not both"
    );
}
