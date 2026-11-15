//! Decent Sampler's aux outputs and buses read as Exemplar's group outputs (round 40 phase
//! 3, ADR 0125; `chorda-src`), tested with the published examples of the Decent Sampler
//! developers' guide ("How to Use Buses and Auxiliary Outputs" and "The buses element",
//! decentsampler-developers-guide.readthedocs.io, read 2026-11-12) — the guide's own XML,
//! wrapped in a preset only where the guide shows a fragment.

use cantus_core::inst::sampler::gf;
use cantus_ui::views::inst::sampler::ds;
use std::path::Path;

fn parse(text: &str) -> cantus_ui::views::inst::sampler::import::Import {
    ds::parse(text, Path::new("C:/lib"), &|_| None).expect("parsed")
}

fn out_of(imp: &cantus_ui::views::inst::sampler::import::Import, m: usize) -> Option<f32> {
    imp.groups
        .iter()
        .find(|g| g.0 == m)
        .and_then(|g| g.1.values[gf::OUT])
}

/// The guide's Example 1, as published — a self-closing `<group/>` followed by `</group>`
/// (malformed: read leniently) — routed to the main output *and* aux output 1 at half.
const EXAMPLE_1: &str = r#"<DecentSampler>
<groups>
<group output1Target="MAIN_OUTPUT" output1Volume="1.0" output2Target="AUX_STEREO_OUTPUT_1" output2Volume="0.5" />
    <!-- Samples go here -->
    <sample path="a.wav" rootNote="60"/>
</group>
</groups>
</DecentSampler>"#;

/// The guide's Example 2: a bus with an effect, and a group sent to the main output and
/// the bus.
const EXAMPLE_2: &str = r#"<DecentSampler>
<buses>
    <bus busVolume="0.5" output1Target="MAIN_OUTPUT" output2Target="AUX_STEREO_OUTPUT_1" output1Volume="0.8" output2Volume="0.5">
        <effects>
            <effect type="reverb" wetLevel="1"/>
        </effects>
    </bus>
</buses>
<groups>
<group output1Target="MAIN_OUTPUT" output2Target="BUS_1" output2Volume="1.0">
    <sample path="a.wav" rootNote="60"/>
</group>
</groups>
</DecentSampler>"#;

#[test]
fn a_group_heard_on_the_main_output_stays_there() {
    for text in [EXAMPLE_1, EXAMPLE_2] {
        let imp = parse(text);
        assert_eq!(out_of(&imp, 0), None, "{text}");
        assert!(
            imp.log
                .iter()
                .any(|l| l.contains("aux output 1 is not kept")),
            "{:?}",
            imp.log
        );
    }
    assert!(parse(EXAMPLE_2)
        .log
        .iter()
        .any(|l| l.contains("buses' effects")));
}

#[test]
fn a_multi_out_piano_puts_each_mic_on_its_aux_output() {
    // Helsinki 1967 Pro's "Multi Out" shape (research-p3 formats.md §2): each microphone's
    // group on an aux output of its own, the noises on theirs — the guide's attributes,
    // our sample names.
    let text = r#"<DecentSampler>
<buses>
  <bus busVolume="1.0" output1Target="AUX_STEREO_OUTPUT_5" output1Volume="0.5"/>
</buses>
<groups>
  <group name="57" output1Target="AUX_STEREO_OUTPUT_1"><sample path="57/C4.wav" rootNote="60"/></group>
  <group name="OM" output1Target="AUX_STEREO_OUTPUT_2" output1Volume="0.5"><sample path="om/C4.wav" rootNote="60"/></group>
  <group name="Room" output1Target="AUX_STEREO_OUTPUT_3"><sample path="room/C4.wav" rootNote="60"/></group>
  <group name="Release" output1Target="AUX_STEREO_OUTPUT_4" trigger="release"><sample path="rel/C4.wav" rootNote="60"/></group>
  <group name="Pedalnoise" output1Target="BUS_1"><sample path="pedal/down.wav" rootNote="60"/></group>
</groups>
</DecentSampler>"#;
    let imp = parse(text);
    let outs: Vec<_> = (0..5).map(|m| out_of(&imp, m)).collect();
    assert_eq!(
        outs,
        [Some(1.0), Some(2.0), Some(3.0), Some(4.0), Some(5.0)]
    );
    // Sent at half: −6 dB on the group.
    let gain = |m: usize| imp.groups[m].1.values[gf::GAIN].unwrap_or(0.0);
    assert!((gain(1) + 6.02).abs() < 0.01, "{}", gain(1));
    assert!((gain(4) + 6.02).abs() < 0.01, "{}", gain(4));
    assert_eq!(gain(0), 0.0);
    // The outputs Chorda's mixer shows, named by the groups.
    let spec = cantus_ui::views::samples_content::sounds::spec_of(&imp);
    let outs = cantus_core::inst::chorda_source::outputs_of_zones(&spec);
    let names: Vec<_> = outs.iter().map(|o| (o.id.0, o.name.as_str())).collect();
    assert_eq!(
        names,
        [
            (1, "57"),
            (2, "OM"),
            (3, "Room"),
            (4, "Release"),
            (5, "Pedalnoise")
        ]
    );
}
