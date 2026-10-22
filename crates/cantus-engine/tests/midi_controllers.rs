//! Hardware MIDI into the engine with a controller map (round 29, ADR 0062): a mapped
//! controller or pad is consumed before the engine's ring, so the instrument never hears
//! it, while everything unmapped plays as before; and the messages the engine used to parse
//! and drop — channel pressure, key pressure, program change — reach the live instrument.

use cantus_core::controller::Control;
use cantus_core::*;
use cantus_engine::inst::{self as einst, Controller, InstrumentUnit, NoteExpression, NoteOn};
use cantus_engine::midi::{route, FilterSet, Heard, MidiTap, PortState};
use cantus_engine::{EngineEvent, EngineMessage, MidiEvent, MidiSender, NativeUnit, Processor};
use rtrb::RingBuffer;

/// What a live instrument heard: [0] notes held, [1] the last mod wheel, [2] the last
/// channel pressure, [3] the last key pressure, [4] the last program + 1, [5] controllers
/// heard, [6] the last CC 74.
#[derive(Debug)]
struct Ear {
    held: Vec<u64>,
    tel: einst::Telemetry,
}

impl Default for Ear {
    fn default() -> Self {
        Ear {
            held: Vec::new(),
            tel: [0.0; einst::TELEMETRY_LEN],
        }
    }
}

impl InstrumentUnit for Ear {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &einst::Assets) {}
    fn note_on(&mut self, note: NoteOn) {
        self.held.push(note.id);
    }
    fn note_off(&mut self, _key: u8, id: u64, _velocity: f32) {
        self.held.retain(|h| *h != id);
    }
    fn expression(&mut self, id: u64, e: NoteExpression, value: f32) {
        if e == NoteExpression::Pressure && self.held.contains(&id) {
            self.tel[3] = value;
        }
    }
    fn controller(&mut self, c: Controller, value: f32) {
        self.tel[5] += 1.0;
        match c {
            Controller::ModWheel => self.tel[1] = value,
            Controller::Pressure => self.tel[2] = value,
            Controller::Cc(74) => self.tel[6] = value,
            _ => {}
        }
    }
    fn program(&mut self, program: u8) {
        self.tel[4] = f32::from(program) + 1.0;
    }
    fn all_notes_off(&mut self) {
        self.held.clear();
    }
    fn reset(&mut self) {
        self.held.clear();
    }
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32]) -> bool {
        false
    }
    fn active_voices(&self) -> usize {
        self.held.len()
    }
    fn telemetry(&self) -> einst::Telemetry {
        let mut t = self.tel;
        t[0] = self.held.len() as f32;
        t
    }
}

struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    proc: Processor,
    sender: MidiSender,
    tap: MidiTap,
    ui: rtrb::Producer<Heard>,
    heard: rtrb::Consumer<Heard>,
    held: PortState,
    track: TrackId,
}

impl Rig {
    fn new() -> Self {
        let (mut tx, rx_engine) = RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, rx) = RingBuffer::<EngineEvent>::new(4096);
        let (midi_tx, midi_rx) = RingBuffer::<MidiEvent>::new(256);
        let (ui, heard) = RingBuffer::<Heard>::new(256);
        let proc = Processor::new(48_000.0, rx_engine, tx_engine, midi_rx);
        let mut p = Project::new("ear");
        let mut t = p.make_track("Ear");
        t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog));
        let track = t.id;
        p.tracks.push(t);
        let send = |tx: &mut rtrb::Producer<EngineMessage>, m| assert!(tx.push(m).is_ok());
        send(
            &mut tx,
            EngineMessage::AddInstrument(Box::new(NativeUnit {
                track,
                kind: cantus_engine::UnitKind::Native(InstrumentKind::Analog),
                serial: 1,
                unit: Box::new(Ear::default()),
                automation: Default::default(),
            })),
        );
        send(
            &mut tx,
            EngineMessage::SetSnapshot(std::sync::Arc::new(Snapshot::compile(&p, 1))),
        );
        send(&mut tx, EngineMessage::SetInstrumentProbe(Some(track)));
        send(&mut tx, EngineMessage::SetLiveTrack(Some(track)));
        Rig {
            tx,
            rx,
            proc,
            sender: MidiSender::new(midi_tx),
            tap: MidiTap::default(),
            ui,
            heard,
            held: PortState::default(),
            track,
        }
    }

    /// A message from the hardware's port 0, through the callback's path.
    fn hardware(&mut self, status: u8, data1: u8, data2: u8) {
        let ev = MidiEvent {
            status,
            data1,
            data2,
        };
        route(ev, 0, &self.tap, &self.sender, &mut self.ui, &mut self.held);
    }

    /// Run a few blocks and tell what the instrument heard.
    fn ear(&mut self) -> einst::Telemetry {
        let mut out = vec![0.0; 256 * 2];
        for _ in 0..4 {
            self.proc.process(&mut out, 2);
        }
        let mut tel = [0.0; einst::TELEMETRY_LEN];
        while let Ok(e) = self.rx.pop() {
            if let EngineEvent::Meters(m) = e {
                tel = m.inst;
            }
        }
        tel
    }
}

#[test]
fn a_mapped_controller_never_reaches_the_instrument() {
    let mut rig = Rig::new();
    let _ = (&rig.tx, rig.track);
    // Unmapped: the filter knob bends the instrument as it always did.
    rig.hardware(0xB0, 74, 127);
    let tel = rig.ear();
    assert_eq!(tel[6], 1.0, "unmapped CC 74 arrived");
    // Mapped (CC 74 and the pad on note 36, port 0, channel 1): consumed.
    let mut set = FilterSet::default();
    set.take(0, Some(0), Control::Cc(74));
    set.take(0, Some(0), Control::Note(36));
    rig.tap.filter().publish(&set);
    let before = tel[5];
    rig.hardware(0xB0, 74, 0);
    rig.hardware(0x90, 36, 100);
    let tel = rig.ear();
    assert_eq!(tel[6], 1.0, "the mapped knob no longer bends it");
    assert_eq!(tel[5], before, "no controller arrived at all");
    assert_eq!(tel[0], 0.0, "the mapped pad plays no note");
    // The interface heard both, in order.
    let a = rig.heard.pop().unwrap();
    let b = rig.heard.pop().unwrap();
    assert_eq!((a.event.data1, b.event.data1), (74, 36));
    // The same control on another channel still plays.
    rig.hardware(0x91, 36, 100);
    assert_eq!(rig.ear()[0], 1.0, "channel 2 is not mapped");
}

#[test]
fn pressure_and_program_change_reach_the_live_instrument() {
    let mut rig = Rig::new();
    rig.hardware(0xD0, 64, 0);
    rig.hardware(0x90, 60, 100);
    rig.hardware(0xA0, 60, 127);
    rig.hardware(0xC0, 7, 0);
    let tel = rig.ear();
    assert!(
        (tel[2] - 64.0 / 127.0).abs() < 1e-6,
        "channel pressure: {}",
        tel[2]
    );
    assert_eq!(tel[3], 1.0, "the held key's pressure");
    assert_eq!(tel[4], 8.0, "program 7");
    // Key pressure on a key not held names no note.
    rig.hardware(0xA0, 61, 10);
    assert_eq!(rig.ear()[3], 1.0);
}

/// Review 1, #38: a chord held when a control starts to be learnt is still released: no
/// stuck notes.
#[test]
fn learning_never_leaves_a_note_stuck() {
    let mut rig = Rig::new();
    rig.hardware(0x90, 60, 100);
    rig.hardware(0x90, 64, 100);
    assert_eq!(rig.ear()[0], 2.0);
    rig.tap.filter().set_learning(true);
    rig.hardware(0x80, 60, 0);
    rig.hardware(0x90, 64, 0);
    assert_eq!(rig.ear()[0], 0.0, "both released");
    // While learning, a new key plays nothing (it is the control being learnt).
    rig.hardware(0x90, 67, 100);
    assert_eq!(rig.ear()[0], 0.0);
}
