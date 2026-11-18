//! Vinculum, the bus compressor, through the whole engine (ADR 0139): at its
//! defaults the mix untouched, to the bit, no latency; the starting preset (the glue)
//! sounds; every model the same render at every block size and live as offline; the
//! external key through the engine's own sidechain path, filtered by the side chain's
//! high-pass; a fork carries on as the unit does. // fxb:vinculum

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
    let fx = add_effect(&mut p, EffectKind::BusCompressor);
    let slot = strip_of(&mut p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    assert_eq!(EffectUnit::new(&slot, SR).latency(), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Vinculum at its defaults");
    // Each model at its defaults: the threshold over the signal touches nothing either.
    for model in [1.0, 2.0] {
        let mut q = p.clone();
        set(&mut q, fx, "model", model);
        assert_null(&reference, &render_of(&q), 0.0, "a model at its defaults");
    }
}

#[test]
fn the_starting_preset_sounds_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::BusCompressor);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::BusCompressor);
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

/// Each model working hard (the vari-mu with its colour at 2×, the opto at Limit): the
/// same render at every block size and in real time.
#[test]
fn every_model_is_block_size_independent_and_live_as_offline() {
    for (what, kv) in [
        (
            "VCA Auto",
            &[("rel", 4.0), ("ratio", 2.0), ("att", 0.0)][..],
        ),
        ("Opto Limit", &[("model", 1.0), ("char", 1.0)][..]),
        (
            "Vari-mu colour",
            &[("model", 2.0), ("colour", 1.0), ("time", 4.0)][..],
        ),
    ] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::BusCompressor);
        set_all(&mut p, fx, &[("thr", -30.0), ("makeup", 6.0), ("mix", 0.7)]);
        set_all(&mut p, fx, kv);
        let a = render_of(&p);
        assert!(
            differs(&a, &render_of(&demo_project())) > 1e-3,
            "{what} works"
        );
        for block in [1usize, 37, 512] {
            assert_null(
                &a,
                &render_with(&p, block),
                0.0,
                &format!("{what}, {block}"),
            );
        }
        let live = realtime(&p, a.len() / 2);
        assert_null(&a[..live.len()], &live, 0.0, &format!("{what}: live"));
    }
}

/// The lead compressed by the bass: the bass's strip routed as the slot's key.
fn keyed_by_the_bass() -> Project {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::BusCompressor);
    set_all(&mut p, fx, &[("thr", -36.0), ("ratio", 2.0), ("att", 1.0)]);
    let bass = p.strips[1].id;
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    p
}

/// The key comes from another strip (the classic bus compressor's external key): routed,
/// the lead is compressed by the bass, not by itself; the side chain's high-pass filters
/// the key (a bass key past 500 Hz barely moves it); keyed, live is offline and the block
/// size changes no sample.
#[test]
fn the_external_key_drives_it_through_the_sidechain() {
    let p = keyed_by_the_bass();
    let mut unrouted = p.clone();
    strip_of(&mut unrouted, 0).effects[0].sidechain = None;
    let (a, b) = (render_of(&p), render_of(&unrouted));
    assert!(
        differs(&a, &b) > 1e-3,
        "the key decides ({})",
        differs(&a, &b)
    );
    let mut filtered = p.clone();
    let id = strip_of(&mut filtered, 0).effects[0].id;
    set(&mut filtered, id, "hpf", 500.0);
    let c = render_of(&filtered);
    assert!(differs(&a, &c) > 1e-3, "the high-pass hears the key");
    let live = realtime(&p, a.len() / 2);
    assert_null(&a[..live.len()], &live, 0.0, "keyed: live = offline");
    for block in [1usize, 37, 512] {
        assert_null(
            &a,
            &render_with(&p, block),
            0.0,
            &format!("keyed, blocks of {block}"),
        );
    }
}

/// A fork carries on as the unit does (the ballistics, the opto's cell and panel, the
/// valve's filters copied): the Arca checkpoint lesson.
#[test]
fn a_fork_carries_on_from_where_the_unit_is() {
    for model in [0.0, 1.0, 2.0] {
        let mut p = demo_project();
        let mut slot = p.make_effect(EffectKind::BusCompressor);
        slot.spec = support::heavy::heavy(EffectKind::BusCompressor);
        slot.spec.set("model", model).unwrap();
        let mut a = EffectUnit::new(&slot, SR);
        let music = |i: usize| {
            let t = i as f32 / SR;
            0.8 * cantus_dsp::math::sin(t * 110.0 * std::f32::consts::TAU)
                * (0.5 + 0.5 * cantus_dsp::math::sin(t * 3.0 * std::f32::consts::TAU))
        };
        let block = |u: &mut EffectUnit, at: usize| {
            let mut l: Vec<f32> = (at..at + 480).map(music).collect();
            let mut r = l.clone();
            let k: Vec<f32> = l.iter().map(|x| 0.7 * x).collect();
            u.process_keyed(&mut l, &mut r, Some((&k, &k)));
            l
        };
        let mut at = 0;
        for _ in 0..100 {
            block(&mut a, at);
            at += 480;
        }
        let mut b = a.fork().expect("a fork");
        for _ in 0..100 {
            let x = block(&mut a, at);
            let y = block(&mut b, at);
            assert_eq!(x, y, "model {model} at {at}");
            at += 480;
        }
    }
}

/// fxb:vinculum (phase B) — Vinculum as a pedal on Tabula's board: unkeyed, the board is
/// Vinculum inserted alone on its own input, to the bit; keyed, the board takes the key
/// (because its pedal does) and the key reaches the pedal's detector as it reaches an
/// insert's — the board keyed by the bass is Vinculum inserted alone keyed by the bass, to
/// the bit, offline and live.
#[test]
fn as_a_pedal_it_compresses_its_own_input_and_hears_the_board_s_key() {
    use cantus_core::fx::tabula::{pedal_allowed, Board, Pedal, PedalId};
    assert!(
        pedal_allowed(EffectKind::BusCompressor),
        "Vinculum may be a pedal"
    );
    let kv = [
        ("thr", -30.0),
        ("ratio", 2.0),
        ("att", 1.0),
        ("rel", 1.0),
        ("makeup", 6.0),
        ("hpf", 80.0),
    ];
    let board_of = |spec: EffectSpec, key: Option<StripId>| {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Board);
        let slot = strip_of(&mut p, 0)
            .effects
            .iter_mut()
            .find(|e| e.id == fx)
            .unwrap();
        slot.spec.board = Some(Box::new(Board {
            pedals: vec![Pedal::new(PedalId(1), spec)],
            macros: Vec::new(),
        }));
        assert!(
            slot.spec.takes_sidechain(),
            "a board with Vinculum takes a key"
        );
        slot.sidechain = key;
        p
    };
    // Vinculum inserted alone on its own input, then keyed by the bass.
    let mut alone = demo_project();
    let fx = add_effect(&mut alone, EffectKind::BusCompressor);
    set_all(&mut alone, fx, &kv);
    let own = render_of(&alone);
    assert!(
        differs(&own, &render_of(&demo_project())) > 1e-3,
        "it compresses"
    );
    let bass = alone.strips[1].id;
    strip_of(&mut alone, 0).effects[0].sidechain = Some(bass);
    let keyed_alone = render_of(&alone);
    let spec = strip_of(&mut alone, 0).effects[0].spec.clone();
    // The same Vinculum as the board's one pedal.
    let p = board_of(spec.clone(), None);
    assert_null(
        &own,
        &render_of(&p),
        0.0,
        "the board = the insert on its own input",
    );
    let p = board_of(spec, Some(bass));
    let keyed = render_of(&p);
    assert_null(
        &keyed_alone,
        &keyed,
        0.0,
        "the board keyed = the insert keyed",
    );
    let live = realtime(&p, keyed.len() / 2);
    assert_null(
        &keyed[..live.len()],
        &live,
        0.0,
        "the board: live = offline",
    );
    assert!(
        differs(&own, &keyed) > 1e-3,
        "the key changes what the pedal does"
    );
}
