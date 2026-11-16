//! Arca, amp and cabinet, through the whole engine (ADR 0137, provisional): at its defaults
//! (no amp, no cabinet) the mix untouched, to the bit; an amp's latency is its oversampler's
//! and the dry comes out exactly that late; a cabinet alone adds none; the factory cabinet
//! is built at once for a render; offline equals real time; and on Tabula's board, a board
//! of one is the insert, to the bit. // fx3:arca

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::fx::tabula::{Board, Pedal, PedalId};
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::EffectUnit;
use modfx::*;
use support::*;

const SR: f32 = 48_000.0;

fn latency_of(p: &mut Project, fx: EffectId) -> usize {
    let slot = strip_of(p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    EffectUnit::new(&slot, SR).latency()
}

#[test]
fn at_its_defaults_it_touches_no_sample() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Amp);
    assert_eq!(latency_of(&mut p, fx), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Arca at its defaults");
}

/// An amp at mix 0 is the dry delayed by the latency it reports (55 samples at 4×).
#[test]
fn an_amps_dry_comes_out_where_its_latency_says() {
    let reference = render_of(&demo_project());
    for (os, want) in [(0.0, 47usize), (1.0, 55), (2.0, 57)] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Amp);
        set_all(&mut p, fx, &[("model", 2.0), ("mix", 0.0), ("os", os)]);
        assert_eq!(latency_of(&mut p, fx), want);
        let out = render_of(&p);
        let shift = 2 * want;
        assert!(out[..shift].iter().all(|x| *x == 0.0));
        assert_null(
            &reference[..reference.len() - shift],
            &out[shift..],
            0.0,
            "the dry, delayed",
        );
    }
}

/// The starting preset (an amp into its cabinet) sounds at once in a render — the
/// cabinet built when the unit is — and the same at every block size and in real time.
#[test]
fn the_starting_amp_sounds_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Amp);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Amp);
    strip_of(&mut p, 0).effects.push(slot);
    let a = render_of(&p);
    let dry = render_of(&demo_project());
    assert!(
        a.iter().zip(&dry).any(|(x, y)| (x - y).abs() > 1e-3),
        "it sounds"
    );
    let b = realtime(&p, a.len() / 2);
    assert_null(&a[..b.len()], &b, 0.0, "real time equals offline");
}

#[test]
fn a_board_of_one_arca_is_the_insert() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Amp);
    set_all(
        &mut p,
        fx,
        &[("model", 4.0), ("gain", 0.6), ("cab", 4.0), ("mix", 0.8)],
    );
    let mut q = p.clone();
    let slot = strip_of(&mut q, 0).effects.remove(0);
    let mut board = q.make_effect(EffectKind::Board);
    board.spec.board = Some(Box::new(Board {
        pedals: vec![Pedal::new(PedalId(1), slot.spec.clone())],
        macros: Vec::new(),
    }));
    assert_eq!(
        EffectUnit::new(&board, SR).latency(),
        EffectUnit::new(&slot, SR).latency()
    );
    strip_of(&mut q, 0).effects.push(board);
    assert_null(&render_of(&p), &render_of(&q), 0.0, "a board of one Arca");
}
