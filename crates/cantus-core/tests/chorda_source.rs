//! Chorda's recorded source (round 40 phase 3, ADR 0125; `chorda-src`): what the snapshot
//! plays, the switch and the outputs it carries.

use cantus_core::inst::chorda_source::{
    self as cs, ChordaSource, RecordedPiano, SourceChoice, SourceRefusal, SOURCE_KEY,
};
use cantus_core::inst_out::{key, OutputFamily, OutputField, OutputId, OutputSlot};
use cantus_core::*;

fn piano(pack: &str) -> ChordaSource {
    ChordaSource::Recorded(RecordedPiano {
        pack: pack.into(),
        instrument: "grand".into(),
    })
}

/// A recorded piano as the picker reads it (Exemplar's keys): `Close` on output 0, `Room`
/// on 1, `Releases` on 16.
fn zones() -> NativeSpec {
    let mut z = NativeSpec::new(InstrumentKind::Sampler);
    for (m, (name, out)) in [("Close", 0.0), ("Room", 1.0), ("Releases", 16.0)]
        .into_iter()
        .enumerate()
    {
        z.set(&format!("g{m}.out"), out);
        z.set_text(&format!("g{m}.name"), Some(name.into()))
            .unwrap();
    }
    for n in 0..6 {
        z.set_text(&format!("z{n}.sample"), Some(format!("C:/p/{n}.wav")))
            .unwrap();
        z.set(&format!("z{n}.group"), (n % 3) as f32);
        z.set(&format!("z{n}.lo"), 21.0 + 3.0 * n as f32);
        z.set(&format!("z{n}.vhi"), 100.0);
    }
    z.set("gain", -4.0);
    z.set("amp.release", 900.0);
    z
}

#[test]
fn a_model_chorda_and_a_source_without_zones_compile_as_they_are() {
    let model = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
    assert_eq!(cs::snapshot_spec(&model), model);
    // The v68 fixture's case: a source named, nothing read — the model plays.
    let mut named = NativeSpec::new(InstrumentKind::Chorda);
    named
        .set_text(SOURCE_KEY, Some("pack:salamander-grand-piano/grand".into()))
        .unwrap();
    named.set("rec.z0.lo", 60.0).unwrap();
    let named = InstrumentSpec::Native(named);
    assert_eq!(cs::snapshot_spec(&named), named);
    let other = InstrumentSpec::Native(zones());
    assert_eq!(cs::snapshot_spec(&other), other);
}

#[test]
fn a_recorded_chorda_compiles_to_the_exemplar_of_its_zones() {
    let z = zones();
    let mut c = cs::with_source(
        &NativeSpec::new(InstrumentKind::Chorda),
        &piano("p"),
        Some(&z),
    );
    assert!(cs::has_zones(&c));
    assert_eq!(
        cs::zones_of(&c),
        z,
        "the zones read back unprefixed, all of them"
    );
    let InstrumentSpec::Native(e) = cs::snapshot_spec(&InstrumentSpec::Native(c.clone())) else {
        panic!("native");
    };
    assert_eq!(
        e, z,
        "the snapshot plays exactly the Exemplar the zones are"
    );
    // The mixer's keys and routes go with it; the model's keys do not.
    c.set(&key(OutputId(1), OutputField::Level), -6.0).unwrap();
    c.set("hardness", 0.3);
    c.outputs.push(OutputSlot {
        id: OutputId(1),
        name: "Room".into(),
        family: OutputFamily::Rooms,
        strip: StripId(9),
    });
    let InstrumentSpec::Native(e) = cs::snapshot_spec(&InstrumentSpec::Native(c)) else {
        panic!("native");
    };
    assert_eq!(e.params.get("o1.level"), Some(&-6.0));
    assert!(!e.params.contains_key("hardness"));
    assert_eq!(e.outputs.len(), 1);
}

#[test]
fn an_output_turned_off_loads_nothing_and_all_off_is_silence() {
    let mut c = cs::with_source(
        &NativeSpec::new(InstrumentKind::Chorda),
        &piano("p"),
        Some(&zones()),
    );
    c.set(&key(OutputId(1), OutputField::On), 0.0).unwrap();
    let e = cs::exemplar_of(&c);
    let files = |e: &NativeSpec| {
        e.text
            .keys()
            .filter(|k| k.ends_with(".sample"))
            .cloned()
            .collect::<Vec<_>>()
    };
    // Zones 1 and 4 are in group 1, the Room.
    assert_eq!(
        files(&e),
        ["z0.sample", "z2.sample", "z3.sample", "z5.sample"]
    );
    for o in [0, 16] {
        c.set(&key(OutputId(o), OutputField::On), 0.0).unwrap();
    }
    let e = cs::exemplar_of(&c);
    assert!(files(&e).is_empty());
    assert!(
        e.text.contains_key("z0.name"),
        "a zone key without a file: never Exemplar's starting sine"
    );
}

#[test]
fn the_outputs_of_a_piano_are_its_groups_named_and_ordered() {
    let o = cs::outputs_of_zones(&zones());
    let names: Vec<_> = o
        .iter()
        .map(|o| (o.id.0, o.name.as_str(), o.family))
        .collect();
    assert_eq!(
        names,
        [
            (0, "Close", OutputFamily::Perspective),
            (1, "Room", OutputFamily::Rooms),
            (16, "Releases", OutputFamily::Noise)
        ]
    );
    let model = cs::outputs(&NativeSpec::new(InstrumentKind::Chorda));
    assert_eq!(model.len(), 5);
    assert_eq!(model[3].name, "Room");
}

#[test]
fn the_switch_carries_routes_and_mixes_by_name_then_family() {
    let mut base = NativeSpec::new(InstrumentKind::Chorda);
    // The model's Room on strip 7 at −3 dB, its Mechanics on strip 8, Hammers at −2 dB.
    base.set(&key(OutputId(3), OutputField::Level), -3.0)
        .unwrap();
    base.set(&key(OutputId(0), OutputField::Level), -2.0)
        .unwrap();
    for (id, name, fam, strip) in [
        (3u16, "Room", OutputFamily::Rooms, 7u64),
        (4, "Mechanics", OutputFamily::Noise, 8),
    ] {
        base.outputs.push(OutputSlot {
            id: OutputId(id),
            name: name.into(),
            family: fam,
            strip: StripId(strip),
        });
    }
    let c = cs::with_source(&base, &piano("p"), Some(&zones()));
    // Room by name (model 3 → 1), Releases by family (Mechanics 4 → 16), Close by family
    // (the first perspective, Hammers 0 → 0).
    assert_eq!(c.params.get("o1.level"), Some(&-3.0));
    assert_eq!(c.params.get("o0.level"), Some(&-2.0));
    assert!(!c.params.contains_key("o3.level"));
    let routes: Vec<_> = c
        .outputs
        .iter()
        .map(|s| (s.id.0, s.strip.0, s.name.as_str()))
        .collect();
    assert_eq!(routes, [(1, 7, "Room"), (16, 8, "Releases")]);
    // And back: the model's outputs take them again.
    let m = cs::with_source(&c, &ChordaSource::Model, None);
    assert!(!cs::has_zones(&m) && m.text(SOURCE_KEY).is_none());
    let routes: Vec<_> = m.outputs.iter().map(|s| (s.id.0, s.strip.0)).collect();
    assert_eq!(routes, [(3, 7), (4, 8)]);
    assert_eq!(m.params.get("o3.level"), Some(&-3.0));
    assert_eq!(m.params.get("o0.level"), Some(&-2.0));
}

#[test]
fn the_switch_is_one_undo_step_that_keeps_the_notes() {
    let mut bus = CommandBus::default();
    let mut p = bus.project().clone();
    let mut t = p.make_track("Piano");
    let mut chorda = NativeSpec::new(InstrumentKind::Chorda);
    chorda.set("hardness", 0.25);
    t.instrument = InstrumentSpec::Native(chorda);
    let mut clip = p.make_clip("Play", Ticks::ZERO, Ticks(3840));
    let note = p.make_note(Ticks::ZERO, Ticks(960), 60, 90);
    clip.notes.push(note);
    t.clips.push(clip);
    let id = t.id;
    bus.reserve_ids(p.next_id);
    bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(t),
        },
        Origin::User,
    )
    .unwrap();
    let before = bus.project().clone();
    let cmd = cs::switch(bus.project(), id, &piano("p"), Some(&zones())).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let after = bus.project().track(id).unwrap();
    assert_eq!(
        after.clips,
        before.track(id).unwrap().clips,
        "notes untouched"
    );
    let n = after.instrument.native().unwrap();
    assert_eq!(
        cs::current_choice(n),
        SourceChoice::Recorded(RecordedPiano {
            pack: "p".into(),
            instrument: "grand".into()
        })
    );
    assert_eq!(
        n.get("hardness"),
        0.25,
        "the model's keys wait for the switch back"
    );
    // The same piano again: refused.
    assert_eq!(
        cs::switch(bus.project(), id, &piano("p"), Some(&zones())),
        Err(SourceRefusal::Same)
    );
    // Chorda 1, the previous model, in one step.
    let cmd = cs::switch_choice(bus.project(), id, &SourceChoice::Model(1), None).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let n = bus
        .project()
        .track(id)
        .unwrap()
        .instrument
        .native()
        .unwrap();
    assert_eq!(n.get("model"), 1.0);
    assert!(!cs::has_zones(n));
    bus.undo();
    bus.undo();
    assert_eq!(*bus.project(), before, "two switches, two undo steps");
    // Not a Chorda; not installed; nothing in it.
    assert_eq!(
        cs::switch(bus.project(), TrackId(u64::MAX), &piano("p"), None),
        Err(SourceRefusal::NotChorda)
    );
    assert_eq!(
        cs::switch(bus.project(), id, &piano("p"), None),
        Err(SourceRefusal::NotInstalled)
    );
    assert!(matches!(
        cs::switch(
            bus.project(),
            id,
            &piano("p"),
            Some(&NativeSpec::new(InstrumentKind::Sampler))
        ),
        Err(SourceRefusal::Unreadable(_))
    ));
}

#[test]
fn each_pack_names_its_piano_in_the_picture() {
    use cantus_core::inst::chorda_look::Body;
    assert_eq!(
        cs::scene_piano(&piano("bitklavier-grand")),
        Some(Body::Concert)
    );
    assert_eq!(
        cs::scene_piano(&piano("salamander-grand-piano")),
        Some(Body::Grand)
    );
    assert_eq!(
        cs::scene_piano(&piano("upright-piano-kw")),
        Some(Body::Upright)
    );
    assert_eq!(cs::scene_piano(&piano("old-piano-fb")), Some(Body::Upright));
    assert_eq!(cs::scene_piano(&ChordaSource::Model), None);
}
