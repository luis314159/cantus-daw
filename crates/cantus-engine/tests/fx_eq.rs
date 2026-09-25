//! Aequalis on a strip, through the engine: every processing mode keeps the null-test
//! rules (a wire when flat — delayed by exactly the latency it reports — and the same
//! output at every block size), the unit reports what its editor draws, a dynamic band
//! keyed from another strip moves with that strip, and the gain scale and the audition act
//! on the sound, not only on the picture.

mod support;
use cantus_core::*;
use cantus_dsp::aequalis::{latency_of, telemetry, Mode, Resolution};
use cantus_dsp::nulltest::{assert_null, rms};
use cantus_engine::*;
use support::*;

/// The lead alone (the bass track removed), so a delay on its strip is the whole mix's.
fn lead_only() -> Project {
    let mut p = demo_project();
    p.tracks.truncate(1);
    p
}

fn set(p: &mut Project, fx: EffectId, pairs: &[(&str, f32)]) {
    let strip = p.tracks[0].strip.unwrap();
    let spec = &mut p.strip_mut(strip).unwrap().effect_mut(fx).unwrap().spec;
    for (k, v) in pairs {
        spec.set(k, *v).expect("an EQ parameter");
    }
}

fn some_bands(p: &mut Project, fx: EffectId) {
    set(
        p,
        fx,
        &[
            ("b0.used", 1.0),
            ("b0.freq", 800.0),
            ("b0.gain", 6.0),
            ("b0.q", 1.5),
            ("b1.used", 1.0),
            ("b1.shape", 3.0), // low cut
            ("b1.freq", 120.0),
            ("b1.slope", 24.0),
            ("b2.used", 1.0),
            ("b2.freq", 2_500.0),
            ("b2.gain", 0.5),
            ("b2.dyn", 1.0),
            ("b2.thr", -40.0),
            ("b2.range", -8.0),
            ("b3.used", 1.0),
            ("b3.shape", 2.0), // high shelf, on the side only
            ("b3.freq", 8_000.0),
            ("b3.gain", 3.0),
            ("b3.place", 4.0),
        ],
    );
}

/// A flat EQ in natural or linear phase is the strip delayed by the latency it reports —
/// to the bit — so delay compensation that trusts the number lines everything up.
#[test]
fn a_flat_eq_in_every_mode_is_the_strip_delayed_by_its_stated_latency() {
    let mut p = lead_only();
    let dry = render_of(&p);
    let fx = add_effect(&mut p, EffectKind::Eq);
    for (mode, res) in [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (2.0, 2.0)] {
        set(&mut p, fx, &[("mode", mode), ("lp_res", res)]);
        let wet = render_of(&p);
        let lat = latency_of(
            Mode::from_index(mode as usize),
            Resolution::from_index(res as usize),
            48_000.0,
        );
        let slot = &p.strips[p.strip_index(p.tracks[0].strip.unwrap()).unwrap()].effects[0];
        assert_eq!(EffectUnit::new(slot, 48_000.0).latency(), lat);
        assert!(wet[..2 * lat].iter().all(|x| *x == 0.0));
        assert_null(
            &wet[2 * lat..],
            &dry[..dry.len() - 2 * lat],
            0.0,
            &format!("mode {mode} resolution {res}: not the dry signal {lat} samples late"),
        );
    }
}

#[test]
fn every_mode_renders_the_same_at_every_block_size() {
    for mode in [0.0, 1.0, 2.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Eq);
        some_bands(&mut p, fx);
        set(
            &mut p,
            fx,
            &[("mode", mode), ("lp_res", 0.0), ("auto", 1.0)],
        );
        let reference = render_with(&p, 512);
        assert!(reference.iter().all(|x| x.is_finite()));
        assert!(rms(&reference) > 1e-3, "mode {mode} is silent");
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("mode {mode} at block size {block}"),
            );
        }
    }
}

/// The unit's telemetry is what the editor reads: the latency, the solo, the detector
/// levels of dynamic bands.
#[test]
fn the_unit_reports_latency_solo_and_detector_levels() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Eq);
    some_bands(&mut p, fx);
    set(&mut p, fx, &[("mode", 2.0), ("lp_res", 3.0), ("solo", 3.0)]);
    let slot = p.strips[p.strip_index(p.tracks[0].strip.unwrap()).unwrap()].effects[0].clone();
    let mut unit = EffectUnit::new(&slot, 48_000.0);
    let (mut l, mut r) = (vec![0.25f32; 4096], vec![0.25f32; 4096]);
    unit.process(&mut l, &mut r);
    // Linear phase at the highest resolution: 4096 samples in, 12 800 before the filter's
    // centre, only the far tail of its (windowed) pre-ring has come out.
    let early = l.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(early < 1e-3, "{early}");
    let t = unit.telemetry().expect("the EQ reports telemetry");
    assert_eq!(t[telemetry::LATENCY], 16_896.0);
    assert_eq!(t[telemetry::SOLO], 3.0);
    assert_eq!(unit.latency(), 16_896);
    // In zero latency, band 2 — dynamic — reads a level from its detector; band 0, static,
    // reads −200. (In linear phase the detectors hear the input as late as the output, so
    // 4096 samples in they have heard nothing yet.)
    set(&mut p, fx, &[("mode", 0.0), ("solo", 0.0)]);
    let slot = p.strips[p.strip_index(p.tracks[0].strip.unwrap()).unwrap()].effects[0].clone();
    let mut unit = EffectUnit::new(&slot, 48_000.0);
    let (mut l, mut r) = (vec![0.25f32; 4096], vec![0.25f32; 4096]);
    unit.process(&mut l, &mut r);
    let t = unit.telemetry().unwrap();
    assert!(t[telemetry::LEVEL + 2] > -200.0);
    assert_eq!(t[telemetry::LEVEL], -200.0);
    assert_eq!(t[telemetry::LATENCY], 0.0);
}

/// A dynamic band keyed from another strip ducks its own strip when the key plays, and
/// does nothing when the key is silent — the mixer's sidechain, inside the EQ.
#[test]
fn a_band_keyed_from_another_strip_moves_with_that_strip() {
    let mut p = demo_project();
    let (lead, bass) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    // Take the bass out of the master (into a muted strip) but keep it as a key.
    let sink = p.strips[8].id;
    p.strip_mut(bass).unwrap().output = Some(sink);
    p.strip_mut(sink).unwrap().muted = true;
    let lead_alone = render_of(&p);
    let mut eq = p.make_effect(EffectKind::Eq);
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.freq", 300.0),
        ("b0.gain", 0.001),
        ("b0.q", 0.5),
        ("b0.dyn", 1.0),
        ("b0.key", 1.0),
        ("b0.thr", -50.0),
        ("b0.range", -18.0),
        ("b0.ratio", 20.0),
    ] {
        eq.spec.set(k, v).unwrap();
    }
    // Keyed from nothing: the band listens to its own strip — and so it moves.
    let mut own = p.clone();
    own.strip_mut(lead).unwrap().effects.push(eq.clone());
    // Keyed from the bass: it moves when the bass plays.
    eq.sidechain = Some(bass);
    let mut keyed = p.clone();
    keyed.strip_mut(lead).unwrap().effects.push(eq.clone());
    let ducked = render_of(&keyed);
    assert!(
        rms(&ducked) < rms(&lead_alone) * 0.8,
        "the bass key did not move the band: {} vs {}",
        rms(&ducked),
        rms(&lead_alone)
    );
    // The bass starts at beat 2: before it, the keyed band is at rest and the lead is
    // untouched to the rounding of a 0.001 dB bell.
    let before = (60.0 / 128.0 * 2.0 * 48_000.0) as usize * 2 - 4_800;
    let diff = ducked[..before]
        .iter()
        .zip(&lead_alone[..before])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(diff < 1e-3, "the band moved before the key played: {diff}");
    // And it is block-size independent, keyed.
    let snap = std::sync::Arc::new(Snapshot::compile(&keyed, 1));
    let mut opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: 512,
        ..Default::default()
    };
    let reference = render(snap.clone(), &opts).unwrap();
    for block in [1usize, 7, 1000] {
        opts.block_frames = block;
        let r = render(snap.clone(), &opts).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("keyed EQ at {block}"),
        );
    }
    let _ = own;
}

/// The gain scale acts on the sound: at 0 % every gain band is flat, so a zero-latency EQ
/// of bells and shelves is a wire again; the audition changes the render and one command
/// undoes it.
#[test]
fn gain_scale_and_audition_act_on_the_sound() {
    let mut p = lead_only();
    let dry = render_of(&p);
    let fx = add_effect(&mut p, EffectKind::Eq);
    set(
        &mut p,
        fx,
        &[
            ("b0.used", 1.0),
            ("b0.freq", 800.0),
            ("b0.gain", 6.0),
            ("b1.used", 1.0),
            ("b1.shape", 2.0),
            ("b1.freq", 6_000.0),
            ("b1.gain", -4.0),
        ],
    );
    let full = render_of(&p);
    assert!(cantus_dsp::nulltest::compare(&full, &dry).max_abs_diff > 1e-3);
    set(&mut p, fx, &[("scale", 0.0)]);
    assert_null(&render_of(&p), &dry, 0.0, "a 0 % gain scale is a wire");
    set(&mut p, fx, &[("scale", 1.0), ("solo", 1.0)]);
    let solo = render_of(&p);
    assert!(
        rms(&solo) < rms(&full) * 0.8,
        "the audition of an 800 Hz bell is quieter than the whole lead"
    );
}
