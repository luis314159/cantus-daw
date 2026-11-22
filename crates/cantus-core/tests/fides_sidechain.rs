//! An instrument's sidechain (round 41, ADR 0154): Fides's input chosen from a strip, as an
//! insert's key is — refused for an instrument that takes none, a strip with no audio, and
//! every key that would close a loop (its own strip, one it reaches, a route made after the
//! key that would reach it back); the key orders the strips, holds its strip in use, goes
//! with an instrument that takes none and comes back on undo; a freeze of the track holds
//! what its key is made of.

use cantus_core::freeze::{region, FreezeTarget};
use cantus_core::*;

/// A source track on strip 0, a Fides track on strip 1, strips 2 and 3 free.
fn desk() -> (CommandBus, TrackId, TrackId, Vec<StripId>) {
    let mut bus = CommandBus::default();
    let p = bus.project().clone();
    let mut q = p.clone();
    let src = q.make_track("Source");
    let mut fides = q.make_track("Fides");
    fides.strip = q.strips.get(1).map(|s| s.id);
    fides.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Resonator));
    let ids = (src.id, fides.id);
    bus.reserve_ids(q.next_id);
    for (i, t) in [src, fides].into_iter().enumerate() {
        bus.execute(
            Command::InsertTrack {
                index: i,
                track: Box::new(t),
            },
            Origin::User,
        )
        .expect("a track");
    }
    let strips = bus.project().strips.iter().map(|s| s.id).collect();
    (bus, ids.0, ids.1, strips)
}

fn key(track: TrackId, source: Option<StripId>) -> Command {
    Command::SetTrackSidechain { track, source }
}

#[test]
fn a_key_is_refused_where_nothing_hears_it_or_it_would_close_a_loop() {
    let (mut bus, src, fides, s) = desk();
    assert_eq!(bus.project().track(src).unwrap().strip, Some(s[0]));
    // An instrument that takes none (Firmus).
    assert!(matches!(
        bus.execute(key(src, Some(s[1])), Origin::User),
        Err(CommandError::InvalidValue(_))
    ));
    // Its own strip, the master, a strip that is not there.
    assert!(matches!(
        bus.execute(key(fides, Some(s[1])), Origin::User),
        Err(CommandError::RoutingCycle(_))
    ));
    assert!(bus
        .execute(key(fides, Some(StripId::MASTER)), Origin::User)
        .is_err());
    assert!(matches!(
        bus.execute(key(fides, Some(StripId(999_999))), Origin::User),
        Err(CommandError::StripNotFound(_))
    ));
    // A strip its own strip feeds, by its output or by a send.
    bus.execute(
        Command::SetStripOutput {
            id: s[1],
            output: Some(s[2]),
        },
        Origin::User,
    )
    .expect("a bus");
    assert!(matches!(
        bus.execute(key(fides, Some(s[2])), Origin::User),
        Err(CommandError::RoutingCycle(_))
    ));
    bus.execute(
        Command::SetStripOutput {
            id: s[1],
            output: None,
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(
        Command::SetStripSend {
            id: s[1],
            send: Send::new(s[3]),
        },
        Origin::User,
    )
    .expect("a send");
    assert!(matches!(
        bus.execute(key(fides, Some(s[3])), Origin::User),
        Err(CommandError::RoutingCycle(_))
    ));
    // Nothing of the refused is left.
    assert_eq!(bus.project().track(fides).unwrap().sidechain, None);
    // The source's strip: taken, and it comes first.
    bus.execute(key(fides, Some(s[0])), Origin::User)
        .expect("the source's strip");
    let p = bus.project();
    assert_eq!(p.track(fides).unwrap().key(), Some(s[0]));
    let order = p.strip_order();
    let at = |id: StripId| order.iter().position(|i| p.strips[*i].id == id).unwrap();
    assert!(at(s[0]) < at(s[1]), "the key before the strip Fides plays into");
}

#[test]
fn a_route_made_after_the_key_may_not_reach_it_back() {
    let (mut bus, _src, fides, s) = desk();
    bus.execute(key(fides, Some(s[0])), Origin::User).unwrap();
    let refused = [
        // Fides into its key, or into a strip that feeds it.
        Command::SetTrackStrip {
            id: fides,
            strip: Some(s[0]),
        },
        // Fides's strip into its key, or sending to it.
        Command::SetStripOutput {
            id: s[1],
            output: Some(s[0]),
        },
        Command::SetStripSend {
            id: s[1],
            send: Send::new(s[0]),
        },
    ];
    for c in refused {
        let what = format!("{c:?}");
        assert!(
            matches!(
                bus.execute(c, Origin::User),
                Err(CommandError::RoutingCycle(_))
            ),
            "{what}"
        );
    }
    bus.execute(
        Command::SetStripOutput {
            id: s[2],
            output: Some(s[0]),
        },
        Origin::User,
    )
    .expect("strip 2 into the key");
    assert!(matches!(
        bus.execute(
            Command::SetTrackStrip {
                id: fides,
                strip: Some(s[2]),
            },
            Origin::User
        ),
        Err(CommandError::RoutingCycle(_))
    ));
    // An insert on the key's strip keyed from Fides's strip would need each first.
    let comp = bus.project().clone().make_effect(EffectKind::Compressor);
    let effect = comp.id;
    bus.reserve_ids(effect.0 + 1);
    bus.execute(
        Command::InsertEffect {
            strip: s[0],
            index: 0,
            slot: Box::new(comp),
        },
        Origin::User,
    )
    .unwrap();
    assert!(matches!(
        bus.execute(
            Command::SetEffectSidechain {
                strip: s[0],
                effect,
                source: Some(s[1]),
            },
            Origin::User
        ),
        Err(CommandError::RoutingCycle(_))
    ));
    // The key's strip is in use while it is the key.
    assert!(matches!(
        bus.execute(Command::RemoveStrip { id: s[0] }, Origin::User),
        Err(CommandError::StripInUse(_))
    ));
    // Elsewhere, Fides goes where it likes.
    bus.execute(
        Command::SetTrackStrip {
            id: fides,
            strip: Some(s[3]),
        },
        Origin::User,
    )
    .expect("another strip");
}

#[test]
fn another_instrument_lets_the_key_go_and_undo_brings_both_back() {
    let (mut bus, _src, fides, s) = desk();
    bus.execute(key(fides, Some(s[0])), Origin::User).unwrap();
    let keyed = bus.project().clone();
    bus.execute(
        Command::SetInstrument {
            track: fides,
            spec: Box::new(InstrumentSpec::default()),
        },
        Origin::User,
    )
    .unwrap();
    let t = bus.project().track(fides).unwrap();
    assert_eq!(t.sidechain, None, "no key where nothing hears it");
    assert!(!bus.project().strip_order().is_empty());
    // Its strip is free again.
    bus.undo().expect("undo the swap");
    assert_eq!(bus.project(), &keyed, "Fides and its key back");
    bus.redo().expect("redo");
    assert_eq!(bus.project().track(fides).unwrap().sidechain, None);
    // A Fides for a Fides (a preset) keeps it.
    bus.undo().unwrap();
    bus.execute(
        Command::SetInstrument {
            track: fides,
            spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
                InstrumentKind::Resonator,
            ))),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(bus.project().track(fides).unwrap().sidechain, Some(s[0]));
    // A track brought in with a key is checked as the command checks it.
    let mut q = bus.project().clone();
    let mut t = q.make_track("Fides 2");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Resonator));
    t.strip = Some(s[3]);
    t.sidechain = Some(s[3]);
    bus.reserve_ids(q.next_id);
    assert!(matches!(
        bus.execute(
            Command::InsertTrack {
                index: 0,
                track: Box::new(t.clone()),
            },
            Origin::User
        ),
        Err(CommandError::RoutingCycle(_))
    ));
    t.sidechain = Some(s[2]);
    bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(t),
        },
        Origin::User,
    )
    .expect("a key with no loop");
}

#[test]
fn a_freeze_of_the_track_holds_what_its_key_is_made_of() {
    let (mut bus, src, fides, s) = desk();
    let before = region(bus.project(), FreezeTarget::Track(fides));
    assert!(!before.tracks.contains(&src));
    bus.execute(key(fides, Some(s[0])), Origin::User).unwrap();
    let r = region(bus.project(), FreezeTarget::Track(fides));
    assert!(r.strips.contains(&s[0]), "its key's strip");
    assert!(r.tracks.contains(&src), "and the track playing into it");
    // Fides's strip's region holds them too.
    let r = region(bus.project(), FreezeTarget::Strip(s[1]));
    assert!(r.strips.contains(&s[0]) && r.tracks.contains(&src));
    // The snapshot names the key; an instrument that takes none would name nothing.
    let snap = Snapshot::compile(bus.project(), 1);
    let t = snap.tracks.iter().find(|t| t.id == fides).unwrap();
    assert_eq!(t.sidechain, Some(s[0]));
    assert!(snap
        .tracks
        .iter()
        .filter(|t| t.id != fides)
        .all(|t| t.sidechain.is_none()));
}

#[test]
fn a_saved_key_round_trips() {
    let (mut bus, _src, fides, s) = desk();
    bus.execute(key(fides, Some(s[0])), Origin::User).unwrap();
    let json = serde_json::to_string(bus.project()).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, bus.project());
    assert_eq!(back.track(fides).unwrap().sidechain, Some(s[0]));
    // A track with none writes nothing of it.
    let t = serde_json::to_string(back.tracks.iter().find(|t| t.id != fides).unwrap()).unwrap();
    assert!(!t.contains("sidechain"));
}
