//! *Split to strips* and *Merge back* in the model, and the taps in the snapshot.

use super::*;
use crate::bus::CommandBus;
use crate::command::{CommandError, Origin};
use crate::instrument::InstrumentSpec;
use crate::pads::PulsusParams;
use crate::vca::StripKind;

/// A project with one kit track on a strip of its own, and a synth after it.
fn project() -> (Project, TrackId, StripId) {
    let mut p = Project::new("Kit");
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let kit_strip = t.strip.unwrap();
    let id = t.id;
    p.tracks.push(t);
    let keys = p.make_track("Keys");
    p.tracks.push(keys);
    (p, id, kit_strip)
}

fn kit(p: &Project, t: TrackId) -> &PulsusParams {
    p.track(t).unwrap().instrument.pulsus().unwrap()
}

#[test]
fn split_gives_every_pad_that_sounds_a_strip_into_the_kit_in_one_undo_step() {
    let (p, t, kit_strip) = project();
    let before = p.clone();
    let sounding: Vec<String> = kit(&p, t)
        .pads
        .iter()
        .filter(|x| sounds(x))
        .map(|x| x.name.clone())
        .collect();
    assert!(sounding.len() >= 8, "the studio kit has its pads");
    let mut bus = CommandBus::new(p);
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let p = bus.project();
    let k = kit(p, t);
    let outs = k.output_strips();
    assert_eq!(outs.len(), sounding.len(), "one strip per pad that sounds");
    // Right after the kit's strip, in pad order, named after the pads, into the kit.
    let at = p.strip_index(kit_strip).unwrap();
    for (i, (s, name)) in outs.iter().zip(&sounding).enumerate() {
        assert_eq!(p.strip_index(*s), Some(at + 1 + i));
        let st = p.strip(*s).unwrap();
        assert_eq!(&st.name, name);
        assert_eq!(st.output, Some(kit_strip));
        assert_eq!(st.pan_law, Some(PanLaw::ZeroDb));
        assert_eq!((st.gain_db, st.pan, st.width), (0.0, 0.0, 1.0));
        assert!(p.tracks_on_strip(*s).any(|x| x.id == t), "the kit feeds it");
    }
    // Each pad its own bus, in pad order.
    for (i, pad) in k.pads.iter().enumerate() {
        if let Some(o) = pad.output {
            let bus = outs.iter().position(|s| *s == o).unwrap() + 1;
            assert_eq!(k.bus_of(i) as usize, bus);
        } else {
            assert_eq!(k.bus_of(i), 0);
        }
    }
    // The strips are processed in pad order, before the kit's.
    let order = p.strip_order();
    let place = |s: StripId| {
        order
            .iter()
            .position(|i| *i == p.strip_index(s).unwrap())
            .unwrap()
    };
    for w in outs.windows(2) {
        assert!(place(w[0]) < place(w[1]));
    }
    assert!(place(*outs.last().unwrap()) < place(kit_strip));
    // One undo step back to exactly what it was.
    assert_eq!(bus.undo_label_key(), Some("cmd.split_kit"));
    bus.undo();
    let mut back = bus.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back, before);
    // Nothing more to split once split.
    bus.redo();
    assert_eq!(split(&mut bus, t).err(), Some(Refusal::Nothing));
}

#[test]
fn merge_back_returns_the_pads_and_removes_only_the_strips_it_made() {
    let (p, t, kit_strip) = project();
    let mut bus = CommandBus::new(p);
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let split_state = bus.project().clone();
    let outs = kit(&split_state, t).output_strips();
    // Something else uses one of them: a send from the keys' strip into it.
    let keys_strip = split_state.tracks[1].strip.unwrap();
    bus.execute(
        Command::InsertStripSend {
            id: keys_strip,
            index: 0,
            send: crate::model::Send::new(outs[0]),
        },
        Origin::User,
    )
    .unwrap();
    let with_send = bus.project().clone();
    let cmd = merge(bus.project(), t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let p = bus.project();
    assert!(!is_split(p, t), "every pad back on the kit");
    assert!(p.strip(outs[0]).is_some(), "the strip a send uses stays");
    for s in &outs[1..] {
        assert!(p.strip(*s).is_none(), "the strip the split made is gone");
    }
    assert!(p.strip(kit_strip).is_some());
    assert_eq!(bus.undo_label_key(), Some("cmd.merge_kit"));
    bus.undo();
    assert_eq!(bus.project(), &with_send);
    assert_eq!(merge(&Project::new("x"), t).err(), Some(Refusal::NotAKit));
}

#[test]
fn a_strip_a_pad_plays_on_cannot_be_removed_and_is_not_free() {
    let (p, t, _) = project();
    let mut bus = CommandBus::new(p);
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let s = kit(bus.project(), t).output_strips()[0];
    assert_eq!(
        bus.execute(Command::RemoveStrip { id: s }, Origin::User),
        Err(CommandError::StripInUse(s))
    );
    assert_ne!(bus.project().first_free_strip(), Some(s));
    assert_eq!(kit_of_strip(bus.project(), s), Some(t));
}

#[test]
fn a_pad_output_must_name_an_audio_strip() {
    let (mut p, t, _) = project();
    let pad = kit(&p, t).pads[0].id;
    let gone = StripId(999_999);
    assert_eq!(
        Command::SetPadOutput {
            track: t,
            pad,
            output: Some(gone),
        }
        .apply(&mut p),
        Err(CommandError::StripNotFound(gone))
    );
}

#[test]
fn the_snapshot_carries_each_bus_as_a_tap_after_the_tracks() {
    let (p, t, kit_strip) = project();
    let unsplit = Snapshot::compile(&p, 1);
    assert_eq!(unsplit.tracks.len(), 2, "no taps before the split");
    let mut bus = CommandBus::new(p);
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let p = bus.project();
    let snap = Snapshot::compile(p, 2);
    let outs = kit(p, t).output_strips();
    assert_eq!(snap.tracks.len(), 2 + outs.len());
    assert_eq!(
        snap.tracks[0].id, t,
        "the project's tracks keep their indices"
    );
    assert_eq!(snap.tracks[0].strip, p.strip_index(kit_strip));
    for (k, s) in outs.iter().enumerate() {
        let i = snap.tracks[0].buses[k];
        let tap = &snap.tracks[i];
        assert_eq!(tap.id, tap_id(t, k as u8 + 1));
        assert!(is_tap(tap.id) && !is_tap(t));
        assert_eq!(
            tap.tap,
            Some(Tap {
                source: 0,
                bus: k as u8 + 1
            })
        );
        assert_eq!(tap.strip, p.strip_index(*s));
        assert!(tap.events.is_empty());
        assert_eq!((tap.gain, tap.pan, tap.audible), (1.0, 0.0, true));
    }
    let law = snap.strips[p.strip_index(outs[0]).unwrap()].pan_law;
    assert_eq!(law, Some(PanLaw::ZeroDb));
    // The kit's mute is its taps' too.
    let mut muted = p.clone();
    muted.track_mut(t).unwrap().muted = true;
    let snap = Snapshot::compile(&muted, 3);
    assert!(snap.tracks.iter().skip(2).all(|x| !x.audible));
}

#[test]
fn a_tap_rests_with_its_source_or_its_strip() {
    let (p, t, _) = project();
    let mut bus = CommandBus::new(p);
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let mut snap = Snapshot::compile(bus.project(), 1);
    settle_taps(&mut snap);
    assert!(snap.tracks.iter().all(|x| !x.rest));
    let first_tap = snap.tracks[0].buses[0];
    let its_strip = snap.tracks[first_tap].strip.unwrap();
    snap.strips[its_strip].rest = true;
    settle_taps(&mut snap);
    assert!(snap.tracks[first_tap].rest, "its strip rests");
    assert!(!snap.tracks[snap.tracks[0].buses[1]].rest);
    snap.strips[its_strip].rest = false;
    snap.tracks[0].rest = true;
    settle_taps(&mut snap);
    assert!(snap
        .tracks
        .iter()
        .filter(|x| x.tap.is_some())
        .all(|x| x.rest));
}

/// A pad's chain: made with its first insert (a hidden strip the pad plays into, into the
/// kit's strip), grown, emptied — the last insert takes it — each one undo step.
#[test]
fn a_pads_chain_is_made_with_its_first_insert_and_goes_with_its_last() {
    let (p, t, kit_strip) = project();
    let before = p.clone();
    let mut bus = CommandBus::new(p);
    let pad = kit(bus.project(), t).pads[0].id;
    let fx = bus.new_effect(crate::effect::EffectKind::Compressor);
    let view = bus.project().clone();
    let cmd = chain_add(&view, t, pad, fx, |n| bus.new_strip(n)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let p = bus.project();
    let pd = kit(p, t).by_id(pad).unwrap().clone();
    let chain = chain_of(p, &pd).expect("the pad plays into its chain");
    let st = p.strip(chain).unwrap();
    assert_eq!(st.kind, StripKind::Chain);
    assert_eq!(st.output, Some(kit_strip), "into the kit's strip");
    assert_eq!(st.pan_law, Some(PanLaw::ZeroDb));
    assert_eq!(chain_effects(p, &pd).len(), 1);
    assert_eq!(
        heard_on(p, &p.tracks[0], &pd),
        None,
        "still heard on the kit"
    );
    assert!(!is_split(p, t) && can_split(p, t));
    assert_ne!(
        p.first_free_strip(),
        Some(chain),
        "a chain is never a new track's"
    );
    // A second insert goes on the same chain.
    let fx2 = bus.new_effect(crate::effect::EffectKind::Eq);
    let view = bus.project().clone();
    let cmd = chain_add(&view, t, pad, fx2, |n| bus.new_strip(n)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let pd = kit(bus.project(), t).by_id(pad).unwrap().clone();
    assert_eq!(chain_effects(bus.project(), &pd).len(), 2);
    // Out again, one by one; the last takes the chain.
    for _ in 0..2 {
        let pd = kit(bus.project(), t).by_id(pad).unwrap().clone();
        let e = chain_effects(bus.project(), &pd)[0].id;
        let cmd = chain_remove(bus.project(), t, pad, e).unwrap();
        bus.execute(cmd, Origin::User).unwrap();
    }
    let mut back = bus.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back, before, "no chain, no strip left");
}

/// Split and merge with chains: a chained pad's chain is what moves (into the pad's new
/// strip, and back into the kit's); merge removes the pad strips and keeps the chains.
#[test]
fn split_and_merge_move_a_pads_chain_and_keep_it() {
    let (p, t, kit_strip) = project();
    let mut bus = CommandBus::new(p);
    let pad = kit(bus.project(), t).pads[1].id;
    let fx = bus.new_effect(crate::effect::EffectKind::Gate);
    let view = bus.project().clone();
    let cmd = chain_add(&view, t, pad, fx, |n| bus.new_strip(n)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let chain = chain_of(bus.project(), kit(bus.project(), t).by_id(pad).unwrap()).unwrap();
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let p = bus.project();
    let pd = kit(p, t).by_id(pad).unwrap();
    assert_eq!(pd.output, Some(chain), "the pad still plays into its chain");
    let own = heard_on(p, &p.tracks[0], pd).expect("heard on a strip of its own");
    assert_eq!(p.strip(chain).unwrap().output, Some(own));
    assert_eq!(p.strip(own).unwrap().output, Some(kit_strip));
    assert!(is_split(p, t));
    let cmd = merge(bus.project(), t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let p = bus.project();
    assert!(!is_split(p, t));
    assert!(p.strip(own).is_none(), "the pad's strip is gone");
    assert_eq!(
        p.strip(chain).unwrap().output,
        Some(kit_strip),
        "the chain kept, into the kit"
    );
    assert!(orphans(p).is_empty());
}

/// Another kit loaded onto a split kit keeps the outputs by pitch, then voice; a kit
/// replaced by another instrument leaves no strip behind; deleting the kit's track takes
/// its pad strips and chains with it — each in the same undo step.
#[test]
fn what_a_kits_pads_played_on_goes_with_the_kit_or_carries_over() {
    let (p, t, _) = project();
    let mut bus = CommandBus::new(p);
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let split_state = bus.project().clone();
    let old = kit(&split_state, t).clone();
    // Another kit: outputs carried over by pitch.
    let mut new = PulsusParams::default();
    carry_outputs(&old, &mut new);
    for pad in &new.pads {
        let was = old.pads.iter().find(|o| o.pitch == pad.pitch);
        assert_eq!(
            pad.output,
            was.and_then(|o| o.output),
            "pitch {}",
            pad.pitch
        );
    }
    // Another instrument: the pad strips go with the kit, in one step.
    let cmd = with_orphans_removed(
        bus.project(),
        Command::SetInstrument {
            track: t,
            spec: Box::new(InstrumentSpec::default()),
        },
    );
    bus.execute(cmd, Origin::User).unwrap();
    for s in old.output_strips() {
        assert!(bus.project().strip(s).is_none(), "an orphan pad strip left");
    }
    bus.undo();
    assert_eq!(bus.project(), &split_state, "one undo step");
    // The kit's track deleted: the same.
    let cmd = with_orphans_removed(bus.project(), Command::RemoveTrack { id: t });
    bus.execute(cmd, Origin::User).unwrap();
    assert!(orphans(bus.project()).is_empty());
    assert!(old
        .output_strips()
        .iter()
        .all(|s| bus.project().strip(*s).is_none()));
    bus.undo();
    assert_eq!(bus.project(), &split_state);
    // An orphan left before (a kit loaded by an older build): merge takes it away.
    let mut stale = split_state.clone();
    let first = old.output_strips()[0];
    let pad = stale.tracks[0]
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .find(|p| p.output == Some(first))
        .unwrap()
        .id;
    stale.tracks[0]
        .instrument
        .pulsus_mut()
        .unwrap()
        .by_id_mut(pad)
        .unwrap()
        .output = None;
    assert_eq!(orphans(&stale), vec![first]);
    let cmd = merge(&stale, t).unwrap();
    cmd.apply(&mut stale).unwrap();
    assert!(stale.strip(first).is_none(), "merge took the orphan too");
}

/// Two passes: the pitch first, then the voice — a new rim (a snare-voiced pad on another
/// note) never takes the snare's strip while the new kit has a snare on the snare's note.
#[test]
fn outputs_carry_by_pitch_first_then_by_voice() {
    use crate::pads::{DrumPad, PadVoice};
    let mut old = PulsusParams::of(vec![
        DrumPad::new(36, "Kick", PadVoice::Kick),
        DrumPad::new(38, "Snare", PadVoice::Snare),
    ]);
    old.pads[0].output = Some(StripId(901));
    old.pads[1].output = Some(StripId(902));
    // The rim (a snare voice, note 37) comes first in pitch order.
    let mut new = PulsusParams::of(vec![
        DrumPad::new(37, "Rim", PadVoice::Snare),
        DrumPad::new(38, "Snare", PadVoice::Snare),
        DrumPad::new(35, "Kick 2", PadVoice::Kick),
    ]);
    carry_outputs(&old, &mut new);
    let out = |pitch: u8| new.pads.iter().find(|p| p.pitch == pitch).unwrap().output;
    assert_eq!(
        out(38),
        Some(StripId(902)),
        "the snare keeps the snare's strip"
    );
    assert_eq!(out(37), None, "the rim does not take it");
    assert_eq!(
        out(35),
        Some(StripId(901)),
        "a kick on another note takes the kick's"
    );
}

/// The bus itself takes what a kit's pads played on with the kit (review 2 #4): a plain
/// `SetInstrument`, a plain `RemoveTrack` and a batch holding one — no caller wraps them —
/// each in the same undo step, chains included. A strip of the user's own, its own pan law
/// set (#5: not pad-made), stays.
#[test]
fn the_bus_removes_what_a_kits_pads_leave_unused_on_every_path() {
    let (p, t, _) = project();
    let mut bus = CommandBus::new(p);
    // One pad with a chain, the kit split, and a user's own strip with its own law.
    let pad = kit(bus.project(), t).pads[1].id;
    let fx = bus.new_effect(crate::effect::EffectKind::Compressor);
    let view = bus.project().clone();
    let cmd = chain_add(&view, t, pad, fx, |n| bus.new_strip(n)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let mine = bus.new_strip("Mine");
    let mine_id = mine.id;
    let index = bus.project().strips.len();
    bus.execute(
        Command::InsertStrip {
            index,
            strip: Box::new(mine),
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(
        Command::SetStripPanLaw {
            id: mine_id,
            law: Some(PanLaw::ZeroDb),
        },
        Origin::User,
    )
    .unwrap();
    let full = bus.project().clone();
    let made: Vec<StripId> = full
        .strips
        .iter()
        .filter(|s| pad_made(s))
        .map(|s| s.id)
        .collect();
    assert!(made.len() > 2, "pad strips and a chain: {made:?}");
    assert!(
        !pad_made(full.strip(mine_id).unwrap()),
        "an own law is not a pad's"
    );
    let gone = |bus: &CommandBus| {
        made.iter().all(|s| bus.project().strip(*s).is_none())
            && bus.project().strip(mine_id).is_some()
    };
    let paths = [
        Command::SetInstrument {
            track: t,
            spec: Box::new(InstrumentSpec::default()),
        },
        Command::RemoveTrack { id: t },
        Command::Batch {
            label_key: "cmd.batch".into(),
            commands: vec![
                Command::SetTrackName {
                    id: t,
                    name: "x".into(),
                },
                Command::RemoveTrack { id: t },
            ],
        },
    ];
    for cmd in paths {
        let what = format!("{cmd:?}").chars().take(40).collect::<String>();
        bus.execute(cmd, Origin::User).unwrap();
        assert!(
            gone(&bus),
            "{what}: a pad's strip left, or the user's own taken"
        );
        assert!(orphans(bus.project()).is_empty());
        bus.undo();
        assert_eq!(bus.project(), &full, "{what}: one undo step");
    }
}

/// Round 37's review, item 1: a kit routed to another strip takes its pads' chains with it
/// (the same undo step), so a chained pad is never left on the old strip through a hidden
/// chain — nor that strip handed to the next new track as free. A split kit's pad strips
/// gather into the new strip too; a chain into a pad's own strip stays on it.
#[test]
fn a_kit_routed_to_another_strip_takes_its_chains_and_pad_strips() {
    let (p, t, kit_strip) = project();
    let mut bus = CommandBus::new(p);
    let other = bus.new_strip("Other");
    let other_id = other.id;
    let n = bus.project().strips.len();
    bus.execute(
        Command::InsertStrip {
            index: n,
            strip: Box::new(other),
        },
        Origin::User,
    )
    .unwrap();
    let kick = kit(bus.project(), t).pads[0].id;
    let fx = bus.new_effect(crate::effect::EffectKind::Compressor);
    let view = bus.project().clone();
    let cmd = chain_add(&view, t, kick, fx, |n| bus.new_strip(n)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let chain = chain_of(bus.project(), kit(bus.project(), t).by_id(kick).unwrap()).unwrap();
    let before = bus.project().clone();
    // Routed 1 → 2: the chain follows, in one undo step.
    bus.execute(
        Command::SetTrackStrip {
            id: t,
            strip: Some(other_id),
        },
        Origin::User,
    )
    .unwrap();
    let p = bus.project();
    assert_eq!(
        p.strip(chain).unwrap().output,
        Some(other_id),
        "the chain follows"
    );
    let pd = kit(p, t).by_id(kick).unwrap();
    assert_eq!(
        heard_on(p, p.track(t).unwrap(), pd),
        None,
        "heard on the kit"
    );
    assert!(!is_split(p, t), "not split: no Merge back, freeze allowed");
    assert_eq!(
        p.first_free_strip(),
        Some(kit_strip),
        "the old strip is free now"
    );
    // To the master and back: the chain to the master, then onto the strip again.
    for to in [None, Some(other_id)] {
        bus.execute(Command::SetTrackStrip { id: t, strip: to }, Origin::User)
            .unwrap();
        assert_eq!(bus.project().strip(chain).unwrap().output, to);
    }
    for _ in 0..3 {
        bus.undo().unwrap();
    }
    let mut back = bus.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back, before, "one undo step each, exactly back");
    // Split, then routed: the pad strips gather into the new strip, the chain stays on its
    // pad's own strip.
    let cmd = split(&mut bus, t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let own = heard_on(
        bus.project(),
        bus.project().track(t).unwrap(),
        kit(bus.project(), t).by_id(kick).unwrap(),
    )
    .unwrap();
    bus.execute(
        Command::SetTrackStrip {
            id: t,
            strip: Some(other_id),
        },
        Origin::User,
    )
    .unwrap();
    let p = bus.project();
    for s in kit(p, t).output_strips() {
        let st = p.strip(s).unwrap();
        if st.kind == StripKind::Pad {
            assert_eq!(
                st.output,
                Some(other_id),
                "{} gathers into the new strip",
                st.name
            );
        }
    }
    assert_eq!(
        p.strip(chain).unwrap().output,
        Some(own),
        "the chain stays on its pad's strip"
    );
    assert_eq!(p.strip(own).unwrap().output, Some(other_id));
}

/// Round 37's review, item 1: a strip a chain or a pad's own strip plays into is never
/// handed to a new track as free, even when no track names it any more.
#[test]
fn a_strip_a_chain_plays_into_is_not_free() {
    let (mut p, t, kit_strip) = project();
    let mut chain = p.make_strip("Chain");
    chain.kind = StripKind::Chain;
    chain.output = Some(kit_strip);
    let c = chain.id;
    p.strips.push(chain);
    p.tracks[0].instrument.pulsus_mut().unwrap().pads[0].output = Some(c);
    // A hand-edited file: the track left on the master, the chain still into the strip.
    p.track_mut(t).unwrap().strip = None;
    p.tracks[1].strip = None;
    assert_ne!(p.first_free_strip(), Some(kit_strip));
    assert_ne!(p.first_free_strip(), Some(c));
}

/// Round 37's review, item 5: a sample kit's pads are all `Sample`; a sample kit loaded
/// onto a split one never hands the old strips out by that voice in order — by pitch, then
/// by name, then by place.
#[test]
fn sample_pads_carry_outputs_by_pitch_name_or_place_not_by_voice() {
    use crate::pads::{DrumPad, PadVoice};
    let mut old = PulsusParams::of(vec![
        DrumPad::new(36, "Kick", PadVoice::Sample),
        DrumPad::new(38, "Snare", PadVoice::Sample),
        DrumPad::new(42, "Hat", PadVoice::Sample),
    ]);
    old.pads[0].output = Some(StripId(901));
    old.pads[1].output = Some(StripId(902));
    old.pads[2].output = Some(StripId(903));
    let mut new = PulsusParams::of(vec![
        DrumPad::new(36, "Kick", PadVoice::Sample),
        DrumPad::new(40, "Clap", PadVoice::Sample),
        DrumPad::new(44, "hat", PadVoice::Sample),
    ]);
    carry_outputs(&old, &mut new);
    let out = |pitch: u8| new.pads.iter().find(|p| p.pitch == pitch).unwrap().output;
    assert_eq!(out(36), Some(StripId(901)), "by pitch");
    assert_eq!(out(44), Some(StripId(903)), "by name, the hat's");
    assert_eq!(out(40), Some(StripId(902)), "by place: the second pad's");
    // By voice, the hat (the first sample pad left) would take the kick's strip:
    let mut new = PulsusParams::of(vec![
        DrumPad::new(35, "Hat", PadVoice::Sample),
        DrumPad::new(37, "Snare", PadVoice::Sample),
    ]);
    carry_outputs(&old, &mut new);
    let out = |pitch: u8| new.pads.iter().find(|p| p.pitch == pitch).unwrap().output;
    assert_eq!(
        out(35),
        Some(StripId(903)),
        "the hat takes the hat's, not the kick's"
    );
    assert_eq!(out(37), Some(StripId(902)), "the snare the snare's");
}
