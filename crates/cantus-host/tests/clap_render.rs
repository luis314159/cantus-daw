//! A bounce with a hosted plug-in: fresh instances loaded on this (main) thread with the
//! live ones' state, processed by the offline renderer — and the result is what playing
//! the project gave, to the bit.

mod support;
use cantus_core::*;
use cantus_engine::{render_with_units, RenderOptions};
use support::*;

#[test]
fn an_offline_render_with_a_hosted_plugin_equals_playing_it() {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    rig.set_param(strip, id, P_GAIN, -6.0);
    rig.set_param(strip, id, P_LATENCY, 64.0);
    rig.settle();
    assert_eq!(rig.host.hosted(id).unwrap().latency(), 64);
    let played = rig.play(24_000, 256);

    let units = rig.host.render_units(rig.bus.project(), SR);
    assert_eq!(units.len(), 1, "one instance for the render");
    let snapshot = std::sync::Arc::new(Snapshot::compile(rig.bus.project(), 1));
    let opts = RenderOptions {
        block_frames: 333,
        tail_seconds: 0.25,
        ..Default::default()
    };
    let rendered = render_with_units(snapshot, &opts, units).unwrap().samples;
    rig.host.finish_render();
    assert!(rendered.len() >= played.len());
    let differing = played
        .iter()
        .zip(&rendered)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    assert_eq!(differing, 0, "the bounce is what was heard");
    assert!(peak(&played) > 0.02);
    // The live plug-in is untouched by the render.
    assert!(rig.host.hosted(id).is_some());
    assert_eq!(rig.host.hosted(id).unwrap().param_value(P_GAIN), Some(-6.0));
}
