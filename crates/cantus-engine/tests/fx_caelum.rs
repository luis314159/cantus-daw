//! Caelum, the shimmer reverb, through the whole engine (ADR 0138): at its
//! defaults (mix 0) the mix untouched, to the bit, no latency; the starting preset sounds;
//! offline equals real time; the ducking hears another strip through the sidechain; a
//! synced pre-delay is its note at the song's tempo; a fork carries on as the unit does;
//! the owner's two open choices are parameters. // fxa:caelum

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::caelum::{settings, t};
use cantus_engine::fx::SongPosition;
use cantus_engine::EffectUnit;
use modfx::*;
use support::*;

const SR: f32 = 48_000.0;

#[test]
fn at_its_defaults_it_touches_no_sample() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Shimmer);
    let slot = strip_of(&mut p, 0)
        .effects
        .iter()
        .find(|e| e.id == fx)
        .unwrap()
        .clone();
    assert_eq!(EffectUnit::new(&slot, SR).latency(), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Caelum at its defaults");
}

#[test]
fn the_starting_preset_sounds_and_renders_the_same_every_way() {
    let mut p = demo_project();
    let mut slot = p.make_effect(EffectKind::Shimmer);
    slot.spec = cantus_core::starting::starting_spec(EffectKind::Shimmer);
    strip_of(&mut p, 0).effects.push(slot);
    let a = render_of(&p);
    let dry = render_of(&demo_project());
    assert!(
        a.iter().zip(&dry).any(|(x, y)| (x - y).abs() > 1e-3),
        "it sounds"
    );
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

/// Ducking keyed by another strip (the slot's sidechain, the engine's ordinary path): the
/// bass ducks the lead's space where the lead's own level would not; live = offline.
#[test]
fn the_ducking_hears_the_sidechain() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Shimmer);
    set_all(
        &mut p,
        fx,
        &[
            ("mix", 1.0),
            ("duck", 24.0),
            ("dthr", -50.0),
            ("decay", 3.0),
        ],
    );
    let own = render_of(&p);
    let bass = p.strips[1].id;
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let keyed = render_of(&p);
    let moved = own
        .iter()
        .zip(&keyed)
        .fold(0f32, |m, (x, y)| m.max((x - y).abs()));
    assert!(moved > 1e-3, "the key changes the ducking ({moved})");
    let live = realtime(&p, keyed.len() / 2);
    assert_null(&keyed[..live.len()], &live, 0.0, "keyed: live = offline");
}

fn run(u: &mut EffectUnit, pos: &SongPosition, frames: usize, from: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let mut at = from;
    while at < from + frames {
        let mut l: Vec<f32> = (at..at + 480)
            .map(|i| {
                let x = i as f32 / SR;
                0.3 * cantus_dsp::math::sin(x * 330.0 * std::f32::consts::TAU)
                    * if (i / 12_000) % 2 == 0 { 1.0 } else { 0.0 }
            })
            .collect();
        let mut r = l.clone();
        let mut p = *pos;
        p.samples = at as i64;
        u.set_position(&p);
        u.process(&mut l, &mut r);
        out.extend_from_slice(&l);
        at += 480;
    }
    out
}

fn position(tempo: f64) -> SongPosition {
    SongPosition {
        playing: true,
        tempo_bpm: tempo,
        tempo_now: tempo,
        time_sig_num: 4,
        time_sig_den: 4,
        ..Default::default()
    }
}

/// A synced pre-delay (`psync`) is its note value at the song's tempo: a sixteenth at
/// 128 BPM is 117.2 ms, and at 90 BPM 166.7 ms; free, it is the milliseconds set.
#[test]
fn a_synced_pre_delay_is_its_note_at_the_tempo() {
    let mut slot = slot_of(EffectSpec::new(EffectKind::Shimmer));
    slot.spec.set("mix", 1.0).unwrap();
    slot.spec.set("psync", 2.0).unwrap();
    for (tempo, want) in [(128.0, 117.1875f32), (90.0, 166.666_67)] {
        let mut u = EffectUnit::new(&slot, SR);
        assert!(u.follows_song());
        run(&mut u, &position(tempo), 96_000, 0);
        let got = u.telemetry().unwrap()[t::PREDELAY];
        assert!((got - want).abs() < 0.5, "{tempo}: {got} ms, want {want}");
    }
    slot.spec.set("psync", 0.0).unwrap();
    slot.spec.set("pre", 40.0).unwrap();
    let mut u = EffectUnit::new(&slot, SR);
    assert!(!u.follows_song());
    run(&mut u, &position(128.0), 48_000, 0);
    assert!((u.telemetry().unwrap()[t::PREDELAY] - 40.0).abs() < 0.5);
}

/// A fork carries on as the unit does (the tank, the shifters, the diffusers, the
/// ducking's detector and the glides copied): the Arca checkpoint lesson.
#[test]
fn a_fork_carries_on_from_where_the_unit_is() {
    let slot = slot_of(support::heavy::heavy(EffectKind::Shimmer));
    let mut a = EffectUnit::new(&slot, SR);
    let pos = position(120.0);
    run(&mut a, &pos, 144_000, 0);
    let mut b = a.fork().expect("a fork");
    let x = run(&mut a, &pos, 96_000, 144_000);
    let y = run(&mut b, &pos, 96_000, 144_000);
    assert_eq!(x, y);
}

/// Infinite's Sustain (ADR 0138's amendment) is a parameter whose default holds for ever
/// (infinite means infinite) and reaches the DSP as set; the retired switches are gone.
#[test]
fn infinite_s_sustain_is_a_parameter_that_holds_by_default() {
    let mut s = EffectSpec::new(EffectKind::Shimmer);
    assert_eq!(settings(&s).sustain, 1.0);
    s.set("isus", 0.35).unwrap();
    assert_eq!(settings(&s).sustain, 0.35);
    assert!(s.set("ishim", 1.0).is_none() && s.set("ilevel", 1.0).is_none());
}

fn slot_of(spec: EffectSpec) -> EffectSlot {
    let mut p = demo_project();
    let mut s = p.make_effect(spec.kind);
    s.spec = spec;
    s
}

/// fxa:caelum (phase B) — Caelum as a pedal on Tabula's board: the board takes a key because
/// its pedal does, and the key reaches the pedal's ducking as it reaches an insert's — the
/// board keyed by the bass is Caelum inserted alone keyed by the bass, to the bit, offline
/// and live; unkeyed, the pedal ducks on its own input (it never "does nothing").
#[test]
fn as_a_pedal_its_ducking_hears_the_board_s_key() {
    use cantus_core::fx::tabula::{pedal_allowed, Board, Pedal, PedalId};
    assert!(pedal_allowed(EffectKind::Shimmer), "Caelum may be a pedal");
    let kv = [
        ("mix", 1.0),
        ("duck", 24.0),
        ("dthr", -50.0),
        ("decay", 3.0),
        ("shim", 0.5),
    ];
    // Caelum inserted alone, keyed by the bass.
    let mut alone = demo_project();
    let fx = add_effect(&mut alone, EffectKind::Shimmer);
    set_all(&mut alone, fx, &kv);
    let bass = alone.strips[1].id;
    strip_of(&mut alone, 0).effects[0].sidechain = Some(bass);
    let want = render_of(&alone);
    // The same Caelum as the board's one pedal, the board keyed by the bass.
    let mut p = demo_project();
    let spec = strip_of(&mut alone, 0).effects[0].spec.clone();
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
        "a board with Caelum takes a key"
    );
    slot.sidechain = Some(bass);
    let keyed = render_of(&p);
    assert_null(&want, &keyed, 0.0, "the board keyed = the insert keyed");
    let live = realtime(&p, keyed.len() / 2);
    assert_null(
        &keyed[..live.len()],
        &live,
        0.0,
        "the board: live = offline",
    );
    // Unkeyed, the pedal ducks on its own input: another sound than keyed.
    strip_of(&mut p, 0).effects[0].sidechain = None;
    let own = render_of(&p);
    let moved = own
        .iter()
        .zip(&keyed)
        .fold(0f32, |m, (x, y)| m.max((x - y).abs()));
    assert!(
        moved > 1e-3,
        "the key changes the pedal's ducking ({moved})"
    );
}
