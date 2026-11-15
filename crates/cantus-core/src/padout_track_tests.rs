//! r40:pads — a pad's own mixer track in the model (ADR 0117).

use super::*;
use crate::bus::CommandBus;
use crate::command::Origin;
use crate::instrument::InstrumentSpec;
use crate::pads::PulsusParams;

/// A kit on its strip, a synth on another, and a free strip after them.
fn project() -> (Project, TrackId, StripId, StripId) {
    let mut p = Project::new("Kit");
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let kit_strip = t.strip.unwrap();
    let id = t.id;
    p.tracks.push(t);
    let keys = p.make_track("Keys");
    let keys_strip = keys.strip.unwrap();
    p.tracks.push(keys);
    (p, id, kit_strip, keys_strip)
}

fn pad(p: &Project, t: TrackId, i: usize) -> DrumPad {
    p.track(t).unwrap().instrument.pulsus().unwrap().pads[i].clone()
}

fn track_of(p: &Project, t: TrackId, i: usize) -> Option<StripId> {
    let tr = p.track(t).unwrap();
    pad_track(p, tr, &tr.instrument.pulsus().unwrap().pads[i])
}

#[test]
fn every_pad_shows_the_kits_track_until_one_is_chosen() {
    let (p, t, kit_strip, _) = project();
    let n = p.track(t).unwrap().instrument.pulsus().unwrap().pads.len();
    for i in 0..n {
        assert_eq!(track_of(&p, t, i), Some(kit_strip), "pad {i}");
        let tr = p.track(t).unwrap();
        assert!(!has_own_track(
            &p,
            tr,
            &tr.instrument.pulsus().unwrap().pads[i]
        ));
    }
}

#[test]
fn a_pad_sent_to_a_free_track_and_back_is_one_step_each_and_undo_is_exact() {
    let (p, t, kit_strip, _) = project();
    let free = p.first_free_strip().unwrap();
    let kick = pad(&p, t, 0).id;
    let before = p.clone();
    let mut bus = CommandBus::new(p);
    let cmd = set_pad_track(bus.project(), t, kick, Some(free)).unwrap();
    assert_eq!(cmd.label_key(), "cmd.set_pad_track");
    bus.execute(cmd, Origin::User).unwrap();
    assert_eq!(track_of(bus.project(), t, 0), Some(free));
    assert_eq!(pad(bus.project(), t, 0).output, Some(free));
    // The free strip takes the pad's name.
    let name = pad(bus.project(), t, 0).name;
    assert_eq!(bus.project().strip(free).unwrap().name, name);
    // The other pads stay on the kit.
    assert_eq!(track_of(bus.project(), t, 1), Some(kit_strip));
    // The strip is no longer free: a pad plays on it.
    assert_ne!(bus.project().first_free_strip(), Some(free));
    let after = bus.project().clone();
    // Back: `None`, or the kit's own strip named, are the same choice.
    for back in [None, Some(kit_strip)] {
        let cmd = set_pad_track(bus.project(), t, kick, back).unwrap();
        bus.execute(cmd, Origin::User).unwrap();
        assert_eq!(pad(bus.project(), t, 0).output, None);
        bus.undo();
        assert_eq!(bus.project(), &after);
    }
    bus.undo();
    assert_eq!(bus.project(), &before);
    bus.redo();
    assert_eq!(bus.project(), &after);
}

#[test]
fn a_pad_can_share_another_channels_track() {
    let (p, t, _, keys_strip) = project();
    let snare = pad(&p, t, 1).id;
    let mut bus = CommandBus::new(p);
    let cmd = set_pad_track(bus.project(), t, snare, Some(keys_strip)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let on: Vec<TrackId> = bus
        .project()
        .tracks_on_strip(keys_strip)
        .map(|x| x.id)
        .collect();
    assert!(
        on.contains(&t),
        "the kit feeds the keys' strip through its pad"
    );
    assert_eq!(track_of(bus.project(), t, 1), Some(keys_strip));
}

#[test]
fn a_chained_pad_keeps_its_chain_and_the_chain_plays_into_the_track_chosen() {
    let (mut p, t, kit_strip, _) = project();
    let free = p.first_free_strip().unwrap();
    let kick = pad(&p, t, 0).id;
    let slot = p.make_effect(crate::effect::EffectKind::Eq);
    let view = p.clone();
    let add = chain_add(&view, t, kick, slot, |n| p.make_strip(n)).unwrap();
    add.apply(&mut p).unwrap();
    let chain = chain_of(&p, &pad(&p, t, 0)).unwrap();
    assert_eq!(p.strip(chain).unwrap().output, Some(kit_strip));
    let before = p.clone();
    let mut bus = CommandBus::new(p);
    bus.reserve_ids(before.next_id);
    let cmd = set_pad_track(bus.project(), t, kick, Some(free)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let q = bus.project();
    assert_eq!(
        pad(q, t, 0).output,
        Some(chain),
        "the chain stays the pad's"
    );
    assert_eq!(q.strip(chain).unwrap().output, Some(free));
    assert_eq!(track_of(q, t, 0), Some(free));
    let cmd = set_pad_track(bus.project(), t, kick, None).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    assert_eq!(bus.project().strip(chain).unwrap().output, Some(kit_strip));
    bus.undo();
    bus.undo();
    assert_eq!(bus.project(), &before);
}

#[test]
fn refusals_name_why() {
    let (mut p, t, kit_strip, keys_strip) = project();
    let kick = pad(&p, t, 0).id;
    // Already there.
    assert_eq!(set_pad_track(&p, t, kick, None), Err(Refusal::Nothing));
    assert_eq!(
        set_pad_track(&p, t, kick, Some(kit_strip)),
        Err(Refusal::Nothing)
    );
    // Not a strip of the project.
    assert_eq!(
        set_pad_track(&p, t, kick, Some(StripId(9_999_999))),
        Err(Refusal::Unfit)
    );
    // A VCA has no audio.
    let mut vca = p.make_strip("VCA");
    vca.kind = StripKind::Vca;
    let vca_id = vca.id;
    p.strips.push(vca);
    assert_eq!(
        set_pad_track(&p, t, kick, Some(vca_id)),
        Err(Refusal::Unfit)
    );
    // A pad's chain is not a mixer track.
    let mut chain = p.make_strip("chain");
    chain.kind = StripKind::Chain;
    let chain_id = chain.id;
    p.strips.push(chain);
    assert_eq!(
        set_pad_track(&p, t, kick, Some(chain_id)),
        Err(Refusal::Unfit)
    );
    // No such pad; not a kit.
    assert_eq!(
        set_pad_track(&p, t, PadId(60_000), Some(keys_strip)),
        Err(Refusal::NoPad)
    );
    let keys = p.tracks[1].id;
    assert_eq!(
        set_pad_track(&p, keys, kick, Some(keys_strip)),
        Err(Refusal::NotAKit)
    );
}

#[test]
fn a_strip_a_pad_plays_on_is_fed() {
    let (p, t, _, _) = project();
    let free = p.first_free_strip().unwrap();
    assert!(!crate::routing::is_fed(&p, free));
    let kick = pad(&p, t, 0).id;
    let mut q = p.clone();
    set_pad_track(&p, t, kick, Some(free))
        .unwrap()
        .apply(&mut q)
        .unwrap();
    assert!(crate::routing::is_fed(&q, free), "the pad feeds it");
}

// Review mid-round 40, B-2 and B-3, and review 2, R2-B3: a strip the user chose for a
// pad — a free one given an insert, another channel's — is the user's routing: it belongs
// to no kit (it bounces as any strip, and has no *Split / Merge* of its own), and *Merge
// back* leaves the pad on it — Merge back undoes *Split to strips*, only that (ADR 0117
// §5). A kit whose pads play only on strips the user chose has nothing to merge back.
#[test]
fn merge_back_keeps_the_users_routing_and_a_strip_the_user_chose_belongs_to_no_kit() {
    let (p, t, _, keys_strip) = project();
    let free = p.first_free_strip().unwrap();
    let (kick, snare) = (pad(&p, t, 0).id, pad(&p, t, 1).id);
    let mut bus = CommandBus::new(p);
    for (pad, to) in [(kick, free), (snare, keys_strip)] {
        let cmd = set_pad_track(bus.project(), t, pad, Some(to)).unwrap();
        bus.execute(cmd, Origin::User).unwrap();
    }
    let fx = bus.new_effect(crate::effect::EffectKind::Compressor);
    bus.execute(
        Command::InsertEffect {
            strip: free,
            index: 0,
            slot: Box::new(fx.clone()),
        },
        Origin::User,
    )
    .unwrap();
    for s in [free, keys_strip] {
        assert_eq!(kit_of_strip(bus.project(), s), None, "{s:?}: the user's");
    }
    assert!(is_split(bus.project(), t), "the kit is heard off its strip");
    assert!(!can_merge(bus.project(), t), "nothing Split to strips made");
    assert!(matches!(merge(bus.project(), t), Err(Refusal::Nothing)));
    // Split, the strips it made belong to the kit; merged, they go, and the user's
    // routing stays as the user left it.
    let made = split(&mut bus, t).unwrap();
    bus.execute(made, Origin::User).unwrap();
    let outs: Vec<_> = bus
        .project()
        .track(t)
        .unwrap()
        .instrument
        .output_strips()
        .into_iter()
        .filter(|s| ![free, keys_strip].contains(s))
        .collect();
    assert!(!outs.is_empty());
    assert_eq!(kit_of_strip(bus.project(), outs[0]), Some(t));
    assert!(can_merge(bus.project(), t));
    let cmd = merge(bus.project(), t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let q = bus.project();
    assert!(
        outs.iter().all(|s| q.strip(*s).is_none()),
        "Split's strips gone"
    );
    assert_eq!(
        pad(q, t, 0).output,
        Some(free),
        "the kick stays where the user put it"
    );
    assert_eq!(pad(q, t, 1).output, Some(keys_strip), "and the snare");
    let kept = q.strip(free).expect("the user's strip kept");
    assert_eq!(kept.effects.len(), 1, "with its insert");
    assert_eq!(kept.effects[0].id, fx.id);
    assert!(!can_merge(q, t));
    // Undo puts Split's strips back, the user's routing untouched throughout.
    bus.undo().unwrap();
    assert!(outs.iter().all(|s| bus.project().strip(*s).is_some()));
    assert_eq!(pad(bus.project(), t, 0).output, Some(free));
}

// R2-B3: a render's copy still takes every pad back (a bounce of the kit's track holds
// the whole kit, ADR 0100 §9), the user's strips included.
#[test]
fn merge_in_place_for_a_render_takes_every_pad_back() {
    let (p, t, _, keys_strip) = project();
    let free = p.first_free_strip().unwrap();
    let kick = pad(&p, t, 0).id;
    let mut q = p.clone();
    set_pad_track(&p, t, kick, Some(free))
        .unwrap()
        .apply(&mut q)
        .unwrap();
    let snare = pad(&q, t, 1).id;
    let r = q.clone();
    set_pad_track(&r, t, snare, Some(keys_strip))
        .unwrap()
        .apply(&mut q)
        .unwrap();
    assert!(merge_in_place(&mut q, t));
    assert_eq!(pad(&q, t, 0).output, None);
    assert_eq!(pad(&q, t, 1).output, None);
    assert!(q.strip(free).is_some() && q.strip(keys_strip).is_some());
}
