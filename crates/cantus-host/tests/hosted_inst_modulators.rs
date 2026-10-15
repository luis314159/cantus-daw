//! A hosted CLAP instrument's parameter driven by a modulator (ADR 0057): the track splits
//! its block at note times, so the timed values are handed to each segment at their
//! offsets inside it — and land on their samples whatever the block size. The plug-in is
//! `cantus-test-synth`, whose gain applies its events sample-accurately.

mod support;
use cantus_core::modulator::*;
use cantus_core::*;

use support::*;

const FRAMES: usize = 48_000 * 2;
/// The synth's gain, −60…12 dB.
const S_GAIN: u32 = 0;

fn rig_with_synth() -> (Rig, TrackId) {
    let project = chord_project();
    let track = project.tracks[0].id;
    let mut rig = Rig::new(project, host_with_synth());
    rig.set_synth(track);
    rig.settle();
    (rig, track)
}

#[test]
fn a_step_sequence_on_a_hosted_instruments_gain_lands_on_its_samples() {
    let (mut rig, track) = rig_with_synth();
    let plain = rig.play(FRAMES, 256);
    let mut m = Modulator::new(ModulatorId(rig.bus.project().next_id + 77), ModKind::Step);
    if let ModSource::Step(s) = &mut m.source {
        s.steps = vec![-1.0, 0.5];
        s.division = DIV_SIXTEENTH; // 6 000 samples at 120 BPM
    }
    m.routes.push(ModRoute {
        target: AutoTarget::PluginInstrument { track, id: S_GAIN },
        depth: 0.25,
    });
    rig.exec(Command::InsertModulator {
        host: ModHost::Track(track),
        index: 0,
        modulator: Box::new(m),
    });
    rig.settle();
    let reference = rig.play(FRAMES, 256);
    for block in [1usize, 7, 1000, 4096] {
        let out = rig.play(FRAMES, block);
        assert_eq!(differing(&out, &reference), 0, "block {block}");
    }
    // The chord sounds from sample 24 000 for a quarter: steps 4 (−1) and 5 (0.5).
    let base = 60.0 / 72.0;
    let gain = |s: f64| 10f64.powf((-60.0 + 72.0 * (base + 0.25 * s)) / 20.0);
    let ratio = |f: usize| f64::from(reference[2 * f]) / f64::from(plain[2 * f]);
    for (f, s) in [
        (24_010usize, -1.0),
        (29_999, -1.0),
        (30_000, 0.5),
        (35_000, 0.5),
    ] {
        assert!(plain[2 * f].abs() > 1e-4, "frame {f} sounds");
        assert!(
            (ratio(f) / gain(s) - 1.0).abs() < 1e-4,
            "frame {f}: {} wants {}",
            ratio(f),
            gain(s)
        );
    }
}
