//! Round 42 (ADR 0155): an external instrument set to retune by the MIDI Tuning Standard
//! hears its track's tuning as MTS SysEx on its own port — the bytes `tuning_mts` writes,
//! whole, through the MIDI-out thread (`MidiOut::send_sysex`); another route's port hears
//! none. What a real synth does with them is the owner's checklist (`docs/beta-checklist.md`).

use cantus_core::tuning::{Temperament, Tuning, TuningSystem};
use cantus_core::tuning_mts;
use cantus_engine::midi::OutPort;
use cantus_engine::transport::sync::{self, out};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
struct Port(Arc<Mutex<Vec<Vec<u8>>>>);

impl OutPort for Port {
    fn write(&mut self, bytes: &[u8]) -> bool {
        self.0.lock().unwrap().push(bytes.to_vec());
        true
    }
}

static SYNTH: Mutex<Option<Port>> = Mutex::new(None);
static OTHER: Mutex<Option<Port>> = Mutex::new(None);

fn connect(name: &str) -> Option<Box<dyn OutPort>> {
    let p = match name {
        "Synth" => SYNTH.lock().unwrap().clone(),
        "Other" => OTHER.lock().unwrap().clone(),
        _ => None,
    };
    p.map(|p| Box::new(p) as Box<dyn OutPort>)
}

/// Until `ready` holds, or a minute (the thread looks every few milliseconds).
fn until(ready: impl Fn() -> bool) {
    let t = Instant::now();
    while !ready() {
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "the out thread never sent it"
        );
        std::thread::yield_now();
    }
}

#[test]
fn an_mts_instrument_hears_its_tuning_as_sysex_on_its_own_port() {
    let (synth, other) = (Port::default(), Port::default());
    *SYNTH.lock().unwrap() = Some(synth.clone());
    *OTHER.lock().unwrap() = Some(other.clone());
    let (_port, mut handle) = sync::pair();
    // (The test drives no clock: the thread serves the routes alone.)
    let thread = out::MidiOut::start(
        handle.out.take().unwrap(),
        handle.shared.clone(),
        cantus_engine::midi::pool::OutPool::new(connect),
    );
    let t = Tuning {
        a4_hz: 415.0,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 3,
            follow_song_key: false,
        },
        map: None,
    };
    let table = t.offsets();
    let messages = tuning_mts::write_table_changes(tuning_mts::ALL_DEVICES, 0, &table);
    // Sent before its route is known: kept until the route's port opens.
    thread.send_sysex(7, messages.clone());
    thread.set_unit_routes(vec![(7, "Synth".into(), 0), (8, "Other".into(), 0)]);
    until(|| synth.0.lock().unwrap().len() == messages.len());
    assert_eq!(*synth.0.lock().unwrap(), messages, "whole, in order");
    // What the synth reads back is the table, key by key, within the word's step.
    let mut p = tuning_mts::equal_pitches();
    for m in synth.0.lock().unwrap().iter() {
        tuning_mts::parse(m).unwrap().apply(&mut p);
    }
    // (Keys 0 and 1 at A415 lie below the frequency word's range.)
    for (k, (got, off)) in p.iter().zip(table.0).enumerate().skip(2) {
        let want = k as f64 + f64::from(off);
        assert!((got - want).abs() * 100.0 < 0.0031, "key {k}");
    }
    // A route refused a port drops its messages (as its notes are).
    thread.send_sysex(9, messages.clone());
    thread.set_unit_routes(vec![(7, "Synth".into(), 0), (9, "Nowhere".into(), 0)]);
    std::thread::sleep(Duration::from_millis(50));
    drop(thread);
    assert!(
        other.0.lock().unwrap().is_empty(),
        "another route hears nothing"
    );
    assert_eq!(synth.0.lock().unwrap().len(), messages.len());
}
