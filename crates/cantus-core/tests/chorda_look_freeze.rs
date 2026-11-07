//! Round 37 (ADR 0104): a Chorda's look is nothing to hear, so freezing ignores it — a look
//! changed on a Chorda that feeds a frozen strip is allowed and keeps the freeze key; a
//! change to its sound is still refused. And a song keyed before the look existed keys the
//! same with it written in (schema 58's migration). // r37:scene

use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::inst::chorda_look::{BODY_KEY, FINISH_KEY};
use cantus_core::*;

fn song() -> (CommandBus, TrackId, StripId) {
    let mut p = Project::new("Look");
    let strip = p.strips[0].id;
    let mut t = p.make_track("Piano");
    t.strip = Some(strip);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
    let mut c = p.make_clip("Phrase", Ticks::ZERO, Ticks::beats(4));
    let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
    c.notes.push(n);
    t.clips.push(c);
    let id = t.id;
    p.tracks.push(t);
    (CommandBus::new(p), id, strip)
}

fn freeze(bus: &mut CommandBus, t: FreezeTarget) {
    let cmd = freeze::freeze_commands(bus.project(), t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    assert!(bus.project().frozen(t).is_some());
}

#[test]
fn a_look_changed_on_a_frozen_chorda_is_allowed_and_keeps_the_key() {
    let (mut bus, track, strip) = song();
    for target in [FreezeTarget::Strip(strip), FreezeTarget::Track(track)] {
        freeze(&mut bus, target);
        let key = freeze::content_key(bus.project(), target).unwrap();
        for (k, v) in [(BODY_KEY, "upright"), (FINISH_KEY, "walnut")] {
            bus.execute(
                Command::SetInstrumentText {
                    track,
                    key: k.into(),
                    text: Some(v.into()),
                },
                Origin::User,
            )
            .expect("a look is never refused by a freeze");
        }
        assert_eq!(freeze::content_key(bus.project(), target).unwrap(), key);
        assert_eq!(
            freeze::consistent(bus.project()),
            Ok(()),
            "the frozen keys still hold"
        );
        // A change to the sound is still refused.
        let r = bus.execute(
            Command::SetInstrumentValue {
                track,
                key: "hardness".into(),
                value: 0.5,
            },
            Origin::User,
        );
        assert!(matches!(r, Err(CommandError::Frozen(_))), "{r:?}");
        // And a sound change would move the key.
        let mut p = bus.project().clone();
        p.track_mut(track)
            .and_then(|t| t.instrument.native_mut())
            .unwrap()
            .set("hardness", 0.5);
        assert_ne!(freeze::content_key(&p, target).unwrap(), key);
        // Thaw for the next target.
        let thaw = match target {
            FreezeTarget::Strip(s) => Command::SetStripFrozen {
                strip: s,
                frozen: None,
            },
            FreezeTarget::Track(t) => Command::SetTrackFrozen {
                track: t,
                frozen: None,
            },
        };
        bus.execute(thaw, Origin::User).unwrap();
    }
}

#[test]
fn a_song_keyed_before_the_look_keys_the_same_with_it() {
    let (bus, track, strip) = song();
    let mut p = bus.project().clone();
    let before = freeze::content_key(&p, FreezeTarget::Strip(strip)).unwrap();
    let n = p
        .track_mut(track)
        .and_then(|t| t.instrument.native_mut())
        .unwrap();
    n.set_text(BODY_KEY, Some("concert".into())).unwrap();
    n.set_text(FINISH_KEY, Some("black".into())).unwrap();
    assert_eq!(
        freeze::content_key(&p, FreezeTarget::Strip(strip)).unwrap(),
        before
    );
    // Other text (a modulation route) still counts.
    p.track_mut(track)
        .and_then(|t| t.instrument.native_mut())
        .unwrap()
        .set_text("mod1.src", Some("vel".into()))
        .unwrap();
    assert_ne!(
        freeze::content_key(&p, FreezeTarget::Strip(strip)).unwrap(),
        before
    );
}
