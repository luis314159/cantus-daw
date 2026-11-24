//! r42 fix2-model (review 7, ADR 0155): a MIDI file can carry the song's tuning as MIDI
//! Tuning Standard messages at each tuned track's start — off by default, so a file is the
//! same as before unless asked. The bytes are checked against the MMA's MIDI Tuning
//! Standard layout (the bulk tuning dump `F0 7E <dev> 08 01 <prog> <name ×16>
//! <xx yy zz ×128> <checksum> F7`, 408 bytes; the 2-byte scale/octave tuning `F0 7E <dev>
//! 08 09 <ff gg hh> <ss tt ×12> F7`, the channel mask's bits 16–15, 14–8, 7–1; the tuning
//! program change RPN 0x0003), computed here from the layout, not from our writer, and
//! read back by our MTS reader (tested against the MMA's example and Scale Workshop's dump).

use cantus_core::tuning::{
    ScalaScale, ScaleDegree, Temperament, TrackTuning, Tuning, TuningSystem,
};
use cantus_core::tuning_mts::{self, MtsMessage};
use cantus_core::*;
use cantus_project::midi::{self, smf, Event, ExportOptions};

fn song(tuning: Tuning) -> Project {
    let mut p = Project::new("tuned");
    p.tuning = tuning;
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("line", Ticks::ZERO, Ticks::beats(4));
    for i in 0..4 {
        c.notes
            .push(p.make_note(Ticks::beats(i), Ticks::QUARTER, 60 + 2 * i as u8, 100));
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn file(p: &Project, tuning: bool) -> Vec<u8> {
    midi::export(
        p,
        &ExportOptions {
            tuning,
            ..ExportOptions::default()
        },
    )
    .unwrap()
}

/// The SysEx messages and controllers of the file's track `i` (1: the first after the
/// conductor) at tick 0.
/// A track's SysEx messages and its controllers as `(channel, controller, value)`.
type AtStart = (Vec<Vec<u8>>, Vec<(u8, u8, u8)>);

fn at_start(bytes: &[u8], i: usize) -> AtStart {
    let s = smf::read(bytes).unwrap();
    let mut sysex = Vec::new();
    let mut cc = Vec::new();
    for e in s.tracks[i].iter().filter(|e| e.tick == 0) {
        match &e.event {
            Event::SysEx(m) => sysex.push(m.clone()),
            Event::Controller {
                channel,
                controller,
                value,
            } => cc.push((*channel, *controller, *value)),
            _ => {}
        }
    }
    (sysex, cc)
}

fn werckmeister(a4: f64) -> Tuning {
    Tuning {
        a4_hz: a4,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 0,
            follow_song_key: false,
        },
        map: None,
    }
}

#[test]
fn off_by_default_a_file_is_unchanged() {
    assert!(!ExportOptions::default().tuning);
    let plain = file(&song(Tuning::default()), false);
    // A tuned song with the option off writes the very bytes it wrote before.
    assert_eq!(file(&song(werckmeister(440.0)), false), plain);
    // And with it on, a song in 12-TET at A440 has nothing to write.
    assert_eq!(file(&song(Tuning::default()), true), plain);
}

#[test]
fn a_twelve_note_temperament_is_a_bulk_dump_and_scale_octave_tuning() {
    let tu = werckmeister(440.0);
    let table = tu.offsets();
    let (sysex, cc) = at_start(&file(&song(tu), true), 1);
    assert_eq!(sysex.len(), 2, "a dump and a scale/octave message");
    // The bulk dump, by the MMA's layout.
    let dump = &sysex[0];
    assert_eq!(dump.len(), 408);
    assert_eq!(&dump[..6], &[0xF0, 0x7E, 0x7F, 0x08, 0x01, 0x00]);
    assert_eq!(&dump[6..10], b"Lead");
    assert_eq!(*dump.last().unwrap(), 0xF7);
    let sum = dump[1..406].iter().fold(0u8, |a, b| a ^ b) & 0x7F;
    assert_eq!(
        dump[406], sum,
        "CA-020's checksum: XOR of 7E … the last data byte"
    );
    // Each key's word: the semitone below and 14 bits of a semitone above it.
    for k in [0u8, 60, 61, 69, 127] {
        let pitch = f64::from(k) + f64::from(table.at(k));
        let semi = pitch.floor();
        let frac = ((pitch - semi) * 16384.0).round() as u32;
        let (semi, frac) = if frac == 16384 {
            (semi + 1.0, 0)
        } else {
            (semi, frac)
        };
        let at = 22 + 3 * usize::from(k);
        assert_eq!(
            &dump[at..at + 3],
            &[semi as u8, (frac >> 7) as u8, (frac & 0x7F) as u8],
            "key {k}"
        );
    }
    match tuning_mts::parse(dump).unwrap() {
        MtsMessage::KeyDump { keys, .. } => {
            for k in 0..128u8 {
                let want = f64::from(k) + f64::from(table.at(k));
                assert!((keys[usize::from(k)].unwrap() - want).abs() < 0.0001, "{k}");
            }
        }
        m => panic!("{m:?}"),
    }
    // Scale/octave tuning, 2-byte form, on the track's channel (channel 1: bit 0 of hh).
    let oct = &sysex[1];
    assert_eq!(&oct[..8], &[0xF0, 0x7E, 0x7F, 0x08, 0x09, 0x00, 0x00, 0x01]);
    assert_eq!(oct.len(), 8 + 24 + 1);
    for pc in 0..12usize {
        let c = 100.0 * f64::from(table.at(60 + pc as u8));
        let v = (8192.0 + c * 8192.0 / 100.0).round() as u16;
        assert_eq!(
            &oct[8 + 2 * pc..10 + 2 * pc],
            &[(v >> 7) as u8, (v & 0x7F) as u8],
            "pitch class {pc}"
        );
    }
    assert!(cc.is_empty(), "no program selected: never tuned twice");
}

#[test]
fn a_tuning_scale_octave_cannot_hold_is_a_dump_and_its_program_selected() {
    // 31-EDO repeats every 31 keys, not every 12.
    let edo31 = ScalaScale {
        description: "31".into(),
        degrees: (1..=31)
            .map(|i| ScaleDegree::Cents(1200.0 * f64::from(i) / 31.0))
            .collect(),
    };
    let mut p = song(Tuning::default());
    p.tracks[0].tuning = TrackTuning::Own {
        tuning: Tuning {
            system: TuningSystem::Scale(edo31),
            ..Tuning::default()
        },
    };
    let (sysex, cc) = at_start(&file(&p, true), 1);
    assert_eq!(sysex.len(), 1, "the dump alone");
    assert_eq!(&sysex[0][..6], &[0xF0, 0x7E, 0x7F, 0x08, 0x01, 0x00]);
    // RPN 0x0003 (tuning program change) = program 0, then the null RPN.
    assert_eq!(
        cc,
        vec![
            (0, 101, 0),
            (0, 100, 3),
            (0, 6, 0),
            (0, 101, 127),
            (0, 100, 127)
        ]
    );
    // A diapason more than a semitone from 440 cannot be scale/octave either.
    let low = song(Tuning {
        a4_hz: 380.0,
        ..Tuning::default()
    });
    let (sysex, cc) = at_start(&file(&low, true), 1);
    assert_eq!(sysex.len(), 1);
    assert_eq!(cc.len(), 5);
}

#[test]
fn a_kit_track_is_never_tuned() {
    let mut p = song(werckmeister(440.0));
    let mut kit = p.make_track("Drums");
    kit.instrument = InstrumentSpec::Pulsus(Default::default());
    let mut c = p.make_clip("beat", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 36, 100));
    kit.clips.push(c);
    p.tracks.push(kit);
    let bytes = file(&p, true);
    assert_eq!(at_start(&bytes, 1).0.len(), 2, "the lead is tuned");
    assert!(at_start(&bytes, 2).0.is_empty(), "the kit is not");
}
