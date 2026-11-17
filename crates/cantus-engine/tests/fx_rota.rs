//! Rota, the rotary speaker, through the whole engine (ADR 0133): at its
//! defaults (mix 0) the mix untouched, to the bit, no latency; the starting preset sounds;
//! offline equals real time; and on Tabula's board, a board of one is the insert, to the
//! bit. // fx3:rota

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

#[test]
fn at_its_defaults_it_touches_no_sample() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Rotary);
    let slot = strip_of(&mut p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    assert_eq!(EffectUnit::new(&slot, SR).latency(), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Rota at its defaults");
}

#[test]
fn the_starting_preset_sounds_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Rotary);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Rotary);
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
fn a_board_of_one_rota_is_the_insert() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Rotary);
    set_all(&mut p, fx, &[("mix", 1.0), ("speed", 2.0), ("drive", 0.4)]);
    let mut q = p.clone();
    let slot = strip_of(&mut q, 0).effects.remove(0);
    let mut board = q.make_effect(EffectKind::Board);
    board.spec.board = Some(Box::new(Board {
        pedals: vec![Pedal::new(PedalId(1), slot.spec.clone())],
        macros: Vec::new(),
    }));
    strip_of(&mut q, 0).effects.push(board);
    assert_null(&render_of(&p), &render_of(&q), 0.0, "a board of one Rota");
}
