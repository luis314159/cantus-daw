//! Multiplex in the engine: null tests first (a Multiplex that does nothing renders the
//! song to the bit), then the contract (block size, determinism, finite output), the key
//! from another strip, the latency it reports, and the telemetry its editor reads.

mod support;
use cantus_core::fx::multiband::mb_key;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::multiband::{settings, T_GAIN, T_INPUT, T_LATENCY, T_LEVEL, T_OUTPUT};
use cantus_engine::EffectUnit;
use support::*;

fn set(p: &mut Project, fx: EffectId, key: &str, v: f32) {
    let slot = strip_of(p, 0)
        .effects
        .iter_mut()
        .find(|e| e.id == fx)
        .unwrap();
    slot.spec.set(key, v).expect("a Multiplex parameter");
}

/// Four bands across the spectrum, shaped by `f(band, field) -> Option<value>`.
fn four_bands(p: &mut Project, fx: EffectId, f: impl Fn(usize, &str) -> Option<f32>) {
    for (b, from) in [20.0, 180.0, 1_500.0, 7_000.0].into_iter().enumerate() {
        set(p, fx, &mb_key(b, "used"), 1.0);
        set(p, fx, &mb_key(b, "from"), from);
        for field in [
            "thr", "ratio", "range", "thr_lo", "ratio_lo", "range_lo", "knee", "att", "rel", "la",
            "gain", "place", "key",
        ] {
            if let Some(v) = f(b, field) {
                set(p, fx, &mb_key(b, field), v);
            }
        }
    }
}

#[test]
fn a_multiplex_that_does_nothing_renders_the_song_to_the_bit() {
    let reference = render_of(&demo_project());
    // A new one: no bands.
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Multiband);
    assert_null(&reference, &render_of(&p), 0.0, "a new Multiplex");
    // Bands at unity, in both modes: nothing can act, nothing runs.
    for mode in [0.0, 1.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Multiband);
        set(&mut p, fx, "mode", mode);
        four_bands(&mut p, fx, |_, f| match f {
            "ratio" | "ratio_lo" => Some(1.0),
            _ => None,
        });
        assert_null(
            &reference,
            &render_of(&p),
            0.0,
            &format!("unity bands, mode {mode}"),
        );
    }
    // The dynamic mode with bands that could act but whose thresholds the song never
    // reaches: still the song, to the bit.
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Multiband);
    four_bands(&mut p, fx, |_, f| match f {
        "thr" => Some(0.0),
        "knee" => Some(0.0),
        "ratio" => Some(4.0),
        _ => None,
    });
    assert_null(
        &reference,
        &render_of(&p),
        0.0,
        "dynamic bands under their thresholds",
    );
}

#[test]
fn a_busy_multiplex_is_finite_and_block_size_independent() {
    // Dynamic, split and linear phase.
    for mode in [0.0, 1.0, 2.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Multiband);
        set(&mut p, fx, "mode", mode);
        set(&mut p, fx, "slope", 48.0);
        four_bands(&mut p, fx, |b, f| match f {
            "thr" => Some(-36.0 + 3.0 * b as f32),
            "ratio" => Some([4.0, 0.6, 2.0, 8.0][b]),
            "thr_lo" => Some(-55.0),
            "ratio_lo" => Some([1.0, 2.0, 0.5, 3.0][b]),
            "att" => Some([0.05, 2.0, 10.0, 0.5][b]),
            "la" => Some([0.0, 2.0, 0.0, 4.0][b]),
            "place" => Some((b % 3) as f32),
            "gain" => Some(b as f32 - 2.0),
            _ => None,
        });
        let reference = render_with(&p, 512);
        assert!(reference.iter().all(|x| x.is_finite()));
        assert_ne!(
            reference,
            render_of(&demo_project()),
            "the bands should be working"
        );
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("mode {mode}, block {block}"),
            );
        }
        assert_eq!(render_with(&p, 512), reference, "deterministic");
    }
}

/// A band keyed from the bass strip ducks the lead when the bass plays; the key strip is
/// processed first, and the result does not depend on the block size.
#[test]
fn a_band_can_listen_to_another_strip() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Multiband);
    four_bands(&mut p, fx, |b, f| match f {
        "thr" => Some(-40.0),
        "ratio" => Some(10.0),
        "range" => Some(30.0),
        "key" => Some(if b == 1 { 1.0 } else { 0.0 }),
        "ratio_lo" => Some(1.0),
        _ => None,
    });
    let own = render_of(&p);
    let bass = p.tracks[1].strip.unwrap();
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let keyed = render_with(&p, 256);
    assert!(keyed.iter().all(|x| x.is_finite()));
    assert_ne!(keyed, own, "the key changed nothing");
    for block in [1usize, 64, 1000] {
        assert_null(
            &keyed,
            &render_with(&p, block),
            0.0,
            &format!("keyed, block {block}"),
        );
    }
}

#[test]
fn the_unit_reports_its_lookahead_as_latency_and_fills_its_telemetry() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Multiband);
    four_bands(&mut p, fx, |b, f| match f {
        "la" => Some([0.0, 5.0, 2.0, 0.0][b]),
        "thr" => Some(-40.0),
        "ratio" => Some(4.0),
        _ => None,
    });
    let slot = strip_of(&mut p, 0).effects[0].clone();
    let mut unit = EffectUnit::new(&slot, 48_000.0);
    assert_eq!(unit.latency(), 240, "5 ms at 48 kHz");
    let s = settings(&slot.spec);
    assert!(s.bands[..4].iter().all(|b| b.used));
    // A loud noise burst: every band works, and the telemetry says so.
    let mut seed = 5u32;
    let mut l: Vec<f32> = (0..4_800)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32) * 1.6 - 0.8
        })
        .collect();
    let mut r = l.clone();
    unit.process(&mut l, &mut r);
    let t = unit.telemetry().expect("a plug-in unit has telemetry");
    for b in 0..4 {
        assert!(t[T_GAIN + b] < -1.0, "band {b} gain {}", t[T_GAIN + b]);
        assert!(t[T_LEVEL + b] > -40.0, "band {b} level {}", t[T_LEVEL + b]);
    }
    assert!(t[T_INPUT] > 0.5 && t[T_OUTPUT] > 0.0);
    assert_eq!(t[T_LATENCY], 240.0);

    // The linear-phase mode adds its filters' delay to the lookahead: 2048 + 256 samples.
    set(&mut p, fx, "mode", 2.0);
    let slot = strip_of(&mut p, 0).effects[0].clone();
    let unit = EffectUnit::new(&slot, 48_000.0);
    assert_eq!(unit.latency(), 240 + 2048 + 256);
}
