//! Loquax, the vocoder, through the whole engine (ADR 0139, provisional): at its defaults
//! (mix 0) the mix untouched, to the bit, no latency; the starting preset (the robot)
//! sounds; offline equals real time; the synthesiser plays the notes of another track
//! (`midi_from`); the carrier comes from another strip through the sidechain, and `swap`
//! trades the roles; a fork carries on as the unit does. // fxa:loquax

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::EffectUnit;
use modfx::*;
use support::*;

const SR: f32 = 48_000.0;

fn differs(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0f32, |m, (x, y)| m.max((x - y).abs()))
}

#[test]
fn at_its_defaults_it_touches_no_sample() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Vocoder);
    let slot = strip_of(&mut p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    assert_eq!(EffectUnit::new(&slot, SR).latency(), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Loquax at its defaults");
}

#[test]
fn the_starting_preset_sounds_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Vocoder);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Vocoder);
    strip_of(&mut p, 0).effects.push(slot);
    let a = render_of(&p);
    assert!(differs(&a, &render_of(&demo_project())) > 1e-3, "it sounds");
    let b = realtime(&p, a.len() / 2);
    assert_null(&a[..b.len()], &b, 0.0, "real time equals offline");
    for block in [1usize, 37, 64, 512] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("blocks of {block}"),
        );
    }
}

/// The lead speaks through the synthesiser played by the bass's notes.
fn synth_on_the_bass() -> Project {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Vocoder);
    set_all(
        &mut p,
        fx,
        &[("mix", 1.0), ("src", 1.0), ("sib", 0.0), ("unv", 0.0)],
    );
    let bass = p.tracks[1].id;
    strip_of(&mut p, 0).effects[0].midi_from = Some(bass);
    p
}

/// The lead speaks through the bass's own sound: the carrier on the sidechain.
fn sidechain_from_the_bass() -> Project {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Vocoder);
    set_all(
        &mut p,
        fx,
        &[("mix", 1.0), ("src", 0.0), ("sib", 0.0), ("unv", 0.0)],
    );
    let bass = p.strips[1].id;
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    p
}

/// The notes of another track play the carrier: with them the vocoded lead sounds; with
/// no source (and no sidechain) the carrier is silent and nothing is vocoded. Live is the
/// offline render.
#[test]
fn the_notes_of_another_track_play_the_synthesiser() {
    let p = synth_on_the_bass();
    let mut deaf = p.clone();
    strip_of(&mut deaf, 0).effects[0].midi_from = None;
    let (a, b) = (render_of(&p), render_of(&deaf));
    assert!(
        differs(&a, &b) > 1e-2,
        "the notes are heard ({})",
        differs(&a, &b)
    );
    let live = realtime(&p, a.len() / 2);
    assert_null(&a[..live.len()], &live, 0.0, "notes: live = offline");
    for block in [1usize, 64] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("notes, blocks of {block}"),
        );
    }
}

/// The sidechain carries (the engine's own key path, as a compressor's detector): with the
/// bass routed the lead is vocoded on the bass's sound; without it, no carrier. `swap`
/// trades the roles (the bass speaks through the lead): another sound again. Live is the
/// offline render.
#[test]
fn the_sidechain_carries_and_swap_trades_the_roles() {
    let p = sidechain_from_the_bass();
    let mut unrouted = p.clone();
    strip_of(&mut unrouted, 0).effects[0].sidechain = None;
    let (a, b) = (render_of(&p), render_of(&unrouted));
    assert!(
        differs(&a, &b) > 1e-2,
        "the key carries ({})",
        differs(&a, &b)
    );
    let mut swapped = p.clone();
    let id = strip_of(&mut swapped, 0).effects[0].id;
    set(&mut swapped, id, "swap", 1.0);
    let c = render_of(&swapped);
    assert!(
        differs(&a, &c) > 1e-2,
        "swap trades the roles ({})",
        differs(&a, &c)
    );
    let live = realtime(&p, a.len() / 2);
    assert_null(&a[..live.len()], &live, 0.0, "sidechain: live = offline");
    for block in [1usize, 37, 512] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("keyed, blocks of {block}"),
        );
    }
}

/// A fork carries on as the unit does (the banks, the followers, the synthesiser's voices
/// and the detector copied): the Arca checkpoint lesson.
#[test]
fn a_fork_carries_on_from_where_the_unit_is() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Vocoder);
    slot.spec = support::heavy::heavy(EffectKind::Vocoder);
    let mut a = EffectUnit::new(&slot, SR);
    let voice = |i: usize| {
        let t = i as f32 / SR;
        0.3 * cantus_dsp::math::sin(t * 180.0 * std::f32::consts::TAU)
            * (0.6 + 0.4 * cantus_dsp::math::sin(t * 5.0 * std::f32::consts::TAU))
    };
    let block = |u: &mut EffectUnit, at: usize| {
        if at % 24_000 == 0 {
            u.note(
                7,
                cantus_engine::FxNote::On {
                    pitch: 48 + (at / 24_000 % 12) as u8,
                    velocity: 100,
                },
            );
        }
        let mut l: Vec<f32> = (at..at + 480).map(voice).collect();
        let mut r = l.clone();
        let kl: Vec<f32> = l.iter().map(|x| 0.5 * x).collect();
        u.process_keyed(&mut l, &mut r, Some((&kl, &kl)));
        l
    };
    let mut at = 0;
    for _ in 0..200 {
        block(&mut a, at);
        at += 480;
    }
    let mut b = a.fork().expect("a fork");
    for _ in 0..200 {
        let x = block(&mut a, at);
        let y = block(&mut b, at);
        assert_eq!(x, y, "at {at}");
        at += 480;
    }
}
