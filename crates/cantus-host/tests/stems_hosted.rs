//! Stems with hosted plug-ins (round 23, the export audit): the stems render through the
//! same fresh instances a mix does, so a hosted effect is in its strip's stem and the
//! master stem is the mix, a hosted instrument's track is not silent — where before the
//! stems built every hosted effect as a wire and left hosted instruments out.

mod support;
use cantus_core::*;
use cantus_engine::{
    render_stems_hosted, render_with_progress, RenderOptions, RenderProgress, StemOptions,
    StemSource, StemSpec,
};
use std::sync::Arc;
use support::*;

fn opts() -> RenderOptions {
    RenderOptions {
        block_frames: 333,
        tail_seconds: 0.25,
        compensate_latency: true,
        ..Default::default()
    }
}

fn master() -> StemSpec {
    StemSpec {
        source: StemSource::Master,
        file_name: "00 Master.wav".into(),
    }
}

fn differing(a: &[f32], b: &[f32]) -> usize {
    assert_eq!(a.len(), b.len(), "the same length");
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count()
}

#[test]
fn a_hosted_effect_is_in_the_stems_and_the_master_stem_is_the_mix() {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    rig.set_param(strip, id, P_GAIN, -12.0);
    rig.set_param(strip, id, P_LATENCY, 64.0);
    rig.settle();
    let snapshot = Arc::new(Snapshot::compile(rig.bus.project(), 1));

    let units = rig.host.render_units(rig.bus.project(), SR);
    let mix = render_with_progress(
        snapshot.clone(),
        &opts(),
        units,
        Vec::new(),
        &RenderProgress::default(),
    )
    .unwrap()
    .samples;
    rig.host.finish_render();

    let stem_opts = StemOptions {
        render: opts(),
        ..Default::default()
    };
    let units = rig.host.render_units(rig.bus.project(), SR);
    assert_eq!(units.len(), 1);
    let stems =
        render_stems_hosted(snapshot.clone(), &stem_opts, &[master()], units, Vec::new()).unwrap();
    rig.host.finish_render();
    assert_eq!(
        differing(&stems[0].samples, &mix),
        0,
        "00 Master is the mix"
    );

    // Without the instance the plug-in is a wire: −12 dB missing from the stem.
    let wired =
        render_stems_hosted(snapshot, &stem_opts, &[master()], Vec::new(), Vec::new()).unwrap();
    assert!(
        differing(&wired[0].samples, &mix) > 0,
        "the instance makes the difference"
    );
}

#[test]
fn a_hosted_instruments_track_is_not_a_silent_stem() {
    let project = chord_project();
    let track = project.tracks[0].id;
    let mut rig = Rig::new(project, host_with_synth());
    rig.set_synth(track);
    rig.settle();
    let snapshot = Arc::new(Snapshot::compile(rig.bus.project(), 1));
    let specs = [
        master(),
        StemSpec {
            source: StemSource::Track(0),
            file_name: "01 Keys.wav".into(),
        },
    ];
    let stem_opts = StemOptions {
        render: RenderOptions {
            tail_seconds: 1.0,
            ..opts()
        },
        ..Default::default()
    };
    let instruments = rig.host.render_instruments(rig.bus.project(), SR);
    assert_eq!(instruments.len(), 1, "one fresh instance for the render");
    let stems = render_stems_hosted(
        snapshot.clone(),
        &stem_opts,
        &specs,
        Vec::new(),
        instruments,
    )
    .unwrap();
    rig.host.finish_render();
    assert!(
        !stems[1].silent,
        "the hosted instrument plays in its track's stem"
    );
    assert!(peak(&stems[1].samples) > 0.02);

    let instruments = rig.host.render_instruments(rig.bus.project(), SR);
    let mix = render_with_progress(
        snapshot,
        &stem_opts.render,
        Vec::new(),
        instruments,
        &RenderProgress::default(),
    )
    .unwrap()
    .samples;
    rig.host.finish_render();
    assert_eq!(
        differing(&stems[0].samples, &mix),
        0,
        "00 Master is the mix"
    );
}
