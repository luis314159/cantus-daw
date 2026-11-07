//! A kit split to strips (round 37, ADR 0100) through DAWproject: the export is valid
//! against the official schemas, it says what it cannot carry (DAWproject has no outputs for
//! an instrument), and the kit comes back whole — every pad as it was, on the kit's strip,
//! never routed by a stale strip id to another strip.
mod support;
use cantus_core::*;
use cantus_project::dawproject;
use support::xsd::Schema;

fn split_kit_project() -> Project {
    let mut p = Project::new("Kit");
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [36u8, 38, 42, 46].iter().enumerate() {
        c.notes
            .push(p.make_note(Ticks::beats(i as i64), Ticks::QUARTER, *pitch, 100));
    }
    t.clips.push(c);
    let id = t.id;
    p.tracks.push(t);
    let view = p.clone();
    let cmd = cantus_core::padout::split_with(&view, id, |n| p.make_strip(n)).unwrap();
    cmd.apply(&mut p).unwrap();
    // One pad strip with a fader and an insert of its own: what the file cannot hold.
    let s = p.tracks[0].instrument.output_strips()[0];
    let comp = p.make_effect(EffectKind::Compressor);
    let st = p.strip_mut(s).unwrap();
    st.gain_db = -4.0;
    st.effects.push(comp);
    p
}

#[test]
fn a_split_kit_is_written_whole_validly_and_comes_back_whole() {
    let p = split_kit_project();
    assert!(cantus_core::padout::is_split(&p, p.tracks[0].id));
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dawproject/");
    let read = |f: &str| std::fs::read_to_string(format!("{dir}{f}")).unwrap();
    let parts = dawproject::write::parts(&p, "0.1.0");
    assert!(
        Schema::parse(&read("Project.xsd"))
            .validate(&parts.project_xml)
            .is_empty(),
        "valid against Project.xsd"
    );
    assert!(Schema::parse(&read("MetaData.xsd"))
        .validate(&parts.metadata_xml)
        .is_empty());
    let out = dawproject::export(&p, "0.1.0");
    // Said, honestly: the split and the pads' strips are not in the file.
    let said = out
        .report
        .get("interop.report.x_split_kit")
        .expect("the export says the split is lost");
    assert_eq!(said.arg("track"), Some("Drums"));
    assert_eq!(said.arg("n"), Some("14"));
    assert!(!out.report.is_lossless());
    // Back: the kit whole, every pad as it was but its output.
    let back = dawproject::import(&out.bytes).unwrap().project;
    let kit = back.tracks[0].instrument.pulsus().expect("a kit");
    assert!(!kit.has_outputs(), "the kit comes back unsplit");
    let mut want = p.tracks[0].instrument.pulsus().unwrap().clone();
    for pad in &mut want.pads {
        pad.output = None;
    }
    assert_eq!(kit, &want, "every pad as it was");
    // No strip id from the old project names anything in the new one.
    assert!(back
        .tracks
        .iter()
        .all(|t| t.instrument.output_strips().is_empty()));
}

/// A strip's own pan law has no place in DAWproject: the export says so.
#[test]
fn a_strips_own_pan_law_is_said_lost() {
    let mut p = Project::new("Law");
    let t = p.make_track("Keys");
    let s = t.strip.unwrap();
    p.tracks.push(t);
    p.strip_mut(s).unwrap().pan_law = Some(PanLaw::ZeroDb);
    let out = dawproject::export(&p, "0.1.0");
    assert!(out.report.has("interop.report.x_pan_law"));
    let mut plain = p.clone();
    plain.strip_mut(s).unwrap().pan_law = None;
    assert!(!dawproject::export(&plain, "0.1.0")
        .report
        .has("interop.report.x_pan_law"));
}

/// A kit with a pad's chain but not split (review 2 #3): the export names the chains it
/// loses, and does not call the kit split.
#[test]
fn a_pads_chain_is_said_lost_and_the_kit_is_not_called_split() {
    let mut p = Project::new("Chains");
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let id = t.id;
    p.tracks.push(t);
    let pad = p.tracks[0].instrument.pulsus().unwrap().pads[0].id;
    let fx = p.make_effect(EffectKind::Compressor);
    let view = p.clone();
    let cmd = cantus_core::padout::chain_add(&view, id, pad, fx, |n| p.make_strip(n)).unwrap();
    cmd.apply(&mut p).unwrap();
    assert!(!cantus_core::padout::is_split(&p, id));
    let out = dawproject::export(&p, "0.1.0");
    let said = out
        .report
        .get("interop.report.x_pad_chains")
        .expect("the chains are said lost");
    assert_eq!(
        (said.arg("track"), said.arg("n")),
        (Some("Drums"), Some("1"))
    );
    assert!(!out.report.has("interop.report.x_split_kit"));
}
