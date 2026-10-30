//! A hosted instrument's per-note expression (ADR 0080): values reach the plug-in at their
//! offsets; MPE out puts each note on a member channel of its own, the zone announced once;
//! *off* sends notes only.

use super::*;
use crate::backend::{ExprSupport, ParamChange};
use cantus_core::external::ExprRoute;
use cantus_core::PluginFormat;
use std::sync::Mutex;

/// An audio half that keeps every event list it is handed.
struct Tape(Arc<Mutex<Vec<NoteEvent>>>);

impl AudioThread for Tape {
    fn process(
        &mut self,
        _a: Audio<'_>,
        _p: &[ParamChange],
        notes: &[NoteEvent],
        _o: &mut crate::backend::ParamOut,
    ) {
        self.0.lock().unwrap().extend_from_slice(notes);
    }
    fn reset(&mut self) {}
    fn stop(&mut self) {}
}

fn unit(
    support: ExprSupport,
    route: Option<ExprRoute>,
) -> (HostedInstrument, Arc<Mutex<Vec<NoteEvent>>>) {
    let tape = Arc::new(Mutex::new(Vec::new()));
    let mut u = HostedInstrument::new(
        Box::new(Tape(tape.clone())),
        &[],
        |_| None,
        0,
        48_000.0,
        Arc::new(AtomicU64::new(0)),
    )
    .supports(support);
    let mut p = ExternalPlugin::new(PluginFormat::Clap, "x", "X");
    p.expression = route;
    u.configure_hosted(&p);
    (u, tape)
}

fn on(id: u64, key: u8) -> NoteOn {
    NoteOn {
        key,
        velocity: 0.8,
        id,
        channel: 0,
    }
}

fn run(u: &mut HostedInstrument) {
    let (mut l, mut r) = (vec![0f32; 64], vec![0f32; 64]);
    u.process(&mut l, &mut r);
}

#[test]
fn the_route_follows_what_the_plug_in_declares_unless_chosen() {
    let both = ExprSupport {
        note_expressions: true,
        midi: true,
    };
    let midi = ExprSupport {
        note_expressions: false,
        midi: true,
    };
    assert_eq!(unit(both, None).0.route(), ExprRoute::NoteExpression);
    assert_eq!(unit(midi, None).0.route(), ExprRoute::MpeMidi);
    assert_eq!(unit(ExprSupport::default(), None).0.route(), ExprRoute::Off);
    assert_eq!(
        unit(both, Some(ExprRoute::MpeMidi)).0.route(),
        ExprRoute::MpeMidi
    );
}

#[test]
fn note_expressions_arrive_at_their_offsets() {
    let (mut u, tape) = unit(
        ExprSupport {
            note_expressions: true,
            midi: false,
        },
        None,
    );
    assert!(u.expression_offsets());
    u.note_on(on(5, 60));
    u.expression_at(5, NoteExpression::Tuning, 1.5, 0);
    u.expression_at(5, NoteExpression::Tuning, 2.0, 37);
    run(&mut u);
    let t = tape.lock().unwrap();
    let ex: Vec<(u32, f64)> = t
        .iter()
        .filter_map(|n| match n.kind {
            NoteKind::Expression { value, .. } => Some((n.offset, value)),
            _ => None,
        })
        .collect();
    assert_eq!(ex, vec![(0, 1.5), (37, 2.0)]);
}

#[test]
fn mpe_out_gives_each_note_a_member_channel_and_its_own_bend() {
    let (mut u, tape) = unit(
        ExprSupport {
            note_expressions: false,
            midi: true,
        },
        None,
    );
    u.note_on(on(1, 60));
    u.note_on(on(2, 64));
    u.expression_at(2, NoteExpression::Tuning, 12.0, 10);
    u.expression_at(1, NoteExpression::Pressure, 1.0, 11);
    run(&mut u);
    u.note_off(60, 1, 0.5);
    u.note_on(on(3, 67));
    run(&mut u);
    let t = tape.lock().unwrap();
    let mcm = t
        .iter()
        .filter(|n| {
            matches!(
                n.kind,
                NoteKind::Midi {
                    status: 0xB0,
                    data1: 100,
                    data2: 6
                }
            )
        })
        .count();
    assert_eq!(mcm, 1, "the zone is announced once");
    let ons: Vec<(u8, u8)> = t
        .iter()
        .filter(|n| matches!(n.kind, NoteKind::On { .. }))
        .map(|n| (n.channel, n.key))
        .collect();
    assert_eq!(ons.len(), 3);
    assert!(ons.iter().all(|o| (1..=15).contains(&o.0)));
    assert_ne!(ons[0].0, ons[1].0);
    assert_ne!(
        ons[2].0, ons[0].0,
        "a channel just let go is the last one taken"
    );
    let bend = t.iter().find(|n| n.offset == 10).unwrap();
    assert_eq!(
        bend.kind,
        NoteKind::Midi {
            status: 0xE0 | ons[1].0,
            data1: 0,
            data2: 80
        }
    );
    assert_eq!(bend.channel, ons[1].0);
    let press = t.iter().find(|n| n.offset == 11).unwrap();
    assert_eq!(
        press.kind,
        NoteKind::Midi {
            status: 0xD0 | ons[0].0,
            data1: 127,
            data2: 0
        }
    );
    let off = t
        .iter()
        .find(|n| matches!(n.kind, NoteKind::Off { .. }))
        .unwrap();
    assert_eq!(
        (off.channel, off.key),
        (ons[0].0, 60),
        "let go on its own channel"
    );
}

#[test]
fn off_sends_the_notes_alone() {
    let (mut u, tape) = unit(
        ExprSupport {
            note_expressions: true,
            midi: true,
        },
        Some(ExprRoute::Off),
    );
    u.note_on(on(1, 60));
    u.expression_at(1, NoteExpression::Tuning, 3.0, 4);
    run(&mut u);
    let t = tape.lock().unwrap();
    assert_eq!(t.len(), 1);
    assert!(matches!(t[0].kind, NoteKind::On { .. }));
}

/// Review fix 4: a member channel used again starts at rest — the timbre (CC 74) the last
/// note left it at goes back to the middle before the next note-on there; a channel left at
/// rest gets nothing.
#[test]
fn mpe_out_puts_a_reused_channel_back_to_rest() {
    let (mut u, tape) = unit(
        ExprSupport {
            note_expressions: false,
            midi: true,
        },
        None,
    );
    u.note_on(on(1, 60));
    u.expression_at(1, NoteExpression::Brightness, 1.0, 0);
    for id in 2..=15 {
        u.note_on(on(id, 60 + id as u8));
    }
    u.note_off(60, 1, 0.0);
    run(&mut u);
    tape.lock().unwrap().clear();
    u.note_on(on(16, 72));
    run(&mut u);
    let t = tape.lock().unwrap();
    let on_at = t
        .iter()
        .position(|n| matches!(n.kind, NoteKind::On { .. }))
        .unwrap();
    let ch = t[on_at].channel;
    assert!(
        t[..on_at].iter().any(|n| n.kind
            == NoteKind::Midi {
                status: 0xB0 | ch,
                data1: 74,
                data2: 64
            }),
        "{t:?}"
    );
    assert!(
        !t[..on_at]
            .iter()
            .any(|n| matches!(n.kind, NoteKind::Midi { status, .. } if status & 0xF0 == 0xE0)),
        "the bend was at rest: nothing sent"
    );
}

/// Review D1: after a reset (a seek, a panic) the plug-in still holds what its channels last
/// heard: the first note on a member channel puts bend, pressure and CC 74 back to rest.
#[test]
fn mpe_out_after_a_reset_starts_each_channel_at_rest() {
    let (mut u, tape) = unit(
        ExprSupport {
            note_expressions: false,
            midi: true,
        },
        None,
    );
    u.note_on(on(1, 60));
    u.expression_at(1, NoteExpression::Tuning, 5.0, 0);
    u.note_off(60, 1, 0.0);
    run(&mut u);
    u.reset();
    tape.lock().unwrap().clear();
    u.note_on(on(2, 62));
    run(&mut u);
    let t = tape.lock().unwrap();
    let at = t
        .iter()
        .position(|n| matches!(n.kind, NoteKind::On { .. }))
        .unwrap();
    let ch = t[at].channel;
    let before: Vec<NoteKind> = t[..at].iter().map(|n| n.kind).collect();
    for want in [
        NoteKind::Midi {
            status: 0xE0 | ch,
            data1: 0,
            data2: 64,
        },
        NoteKind::Midi {
            status: 0xD0 | ch,
            data1: 0,
            data2: 0,
        },
        NoteKind::Midi {
            status: 0xB0 | ch,
            data1: 74,
            data2: 64,
        },
    ] {
        assert!(before.contains(&want), "{want:?} missing: {before:?}");
    }
}
