//! Sibilus through the whole engine: a wire at settings that do nothing (to the bit, with
//! and without lookahead), the same render at every block size while it works, a key from
//! another strip, and the latency the unit reports.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::sibilus::latency_for;
use cantus_engine::fx::deesser::settings;
use support::*;

fn set(p: &mut Project, fx: EffectId, key: &str, v: f32) {
    let s = strip_of(p, 0);
    let slot = s.effects.iter_mut().find(|e| e.id == fx).unwrap();
    slot.spec.set(key, v);
}

/// With nothing over the threshold, at lookahead 0 and wide band, the render is the render
/// without the de-esser, to the bit — in both detection modes and every stereo mode.
#[test]
fn a_de_esser_that_does_nothing_renders_bit_identically() {
    let reference = render_of(&demo_project());
    for mode in [0.0, 1.0] {
        for stereo in [0.0, 1.0, 2.0, 3.0] {
            let mut p = demo_project();
            let fx = add_effect(&mut p, EffectKind::DeEsser);
            set(&mut p, fx, "thr", 0.0);
            set(&mut p, fx, "look", 0.0);
            set(&mut p, fx, "mode", mode);
            set(&mut p, fx, "stereo", stereo);
            assert_null(
                &reference,
                &render_of(&p),
                0.0,
                &format!("mode {mode} stereo {stereo}"),
            );
        }
    }
}

/// With lookahead and the linear-phase split, the lead comes out exactly `latency` samples
/// late and otherwise untouched (the bass is taken out so the lead is all there is).
#[test]
fn with_latency_it_is_the_same_render_later() {
    let mut base = demo_project();
    base.tracks.truncate(1);
    let reference = render_of(&base);
    for proc in [0.0, 2.0] {
        let mut p = base.clone();
        let fx = add_effect(&mut p, EffectKind::DeEsser);
        set(&mut p, fx, "thr", 0.0);
        set(&mut p, fx, "look", 5.0);
        set(&mut p, fx, "proc", proc);
        let spec = strip_of(&mut p, 0).effects[0].spec.clone();
        let lat = latency_for(&settings(&spec), 48_000.0);
        assert_eq!(lat, 240 + if proc > 0.0 { 127 } else { 0 });
        let out = render_of(&p);
        // Interleaved stereo: `lat` frames are 2·lat samples.
        let shift = 2 * lat;
        assert!(out[..shift].iter().all(|x| *x == 0.0));
        assert_null(
            &reference[..reference.len() - shift],
            &out[shift..],
            0.0,
            &format!("proc {proc}"),
        );
    }
}

/// Working hard (Allround, a low threshold, the split band, lookahead), the render is the
/// same to the bit at block sizes 1, 7, 64, 1000 and 4096.
#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::DeEsser);
    for (k, v) in [
        ("mode", 1.0),
        ("thr", -60.0),
        ("range", 12.0),
        ("proc", 2.0),
        ("lo", 2_000.0),
        ("look", 3.0),
        ("stereo", 1.0),
    ] {
        set(&mut p, fx, k, v);
    }
    let reference = render_with(&p, 512);
    let dry = render_with(&demo_project(), 512);
    assert!(
        reference != dry,
        "at −60 dB on a 2 kHz band the synth is de-essed"
    );
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

/// A key from another strip: the lead is cut by what the bass does, and the render still
/// does not depend on the block size.
#[test]
fn a_key_from_another_strip_drives_it() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::DeEsser);
    for (k, v) in [
        ("mode", 1.0),
        ("thr", -60.0),
        ("lo", 1_000.0),
        ("look", 0.0),
    ] {
        set(&mut p, fx, k, v);
    }
    let own = render_of(&p);
    let bass = p.tracks[1].strip.unwrap();
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let keyed = render_with(&p, 512);
    assert!(keyed != own, "the key changes what is cut");
    assert_null(&keyed, &render_with(&p, 7), 0.0, "keyed, block 7");
}

/// The unit reports the DSP's latency.
#[test]
fn the_unit_reports_its_latency() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::DeEsser);
    set(&mut p, fx, "look", 7.0);
    set(&mut p, fx, "proc", 1.0);
    let slot = strip_of(&mut p, 0).effects[0].clone();
    let unit = cantus_engine::EffectUnit::new(&slot, 48_000.0);
    assert_eq!(unit.latency(), 7 * 48 + 127);
}
