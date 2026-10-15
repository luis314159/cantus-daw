//! The modulation family live after a wait (ADR 0055): a unit that ran stopped for a while
//! before Play plays what the offline render does — every LFO, carrier and glide starts
//! again at the start of playback.

mod support;
#[path = "support/modfx.rs"]
mod modfx;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

fn busy(kind: EffectKind) -> &'static [(&'static str, f32)] {
    match kind {
        EffectKind::Chorus => &[("mode", 7.0), ("mix", 0.6), ("rate", 1.3)],
        EffectKind::Flanger => &[("mix", 0.5), ("fb", 0.6), ("rate", 0.7)],
        EffectKind::Phaser => &[("mix", 0.5), ("fb", 0.5), ("rate", 0.9)],
        EffectKind::Tremolo => &[("depth", 0.8), ("sync", 0.0), ("rate", 5.0)],
        EffectKind::Vibrato => &[("depth", 40.0), ("rate", 5.5)],
        EffectKind::RingMod => &[("freq", 333.0), ("mix", 0.8), ("lfo", 5.0)],
        EffectKind::FreqShift => &[("shift", 77.0), ("mix", 0.8), ("lfo", 20.0)],
        _ => &[("depth", 0.9), ("div", 6.0), ("smooth", 2.0)],
    }
}

#[test]
fn every_unit_plays_after_a_wait_what_the_offline_render_does() {
    for kind in [
        EffectKind::Chorus,
        EffectKind::Flanger,
        EffectKind::Phaser,
        EffectKind::Tremolo,
        EffectKind::Vibrato,
        EffectKind::RingMod,
        EffectKind::FreqShift,
        EffectKind::VolumeShaper,
    ] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, kind);
        set_all(&mut p, fx, busy(kind));
        let offline = render_of(&p);
        let live = realtime_after_idle(&p, offline.len(), 12_345);
        assert_null(&offline, &live, 0.0, &format!("{kind:?}"));
    }
}
