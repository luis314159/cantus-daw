//! Tabula, the pedalboard, through the engine (ADR 0134, provisional): a null when empty and
//! when its split is empty, its latency summed and its lanes aligned, a footswitch that never
//! changes the latency, structural changes (add, remove, reorder, a lane, a footswitch)
//! that land the same whatever the block size, pedals handed back when they have faded,
//! the song position each pedal hears, and the macros over automation.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::fx::tabula::{Board, Macro, MacroMap, Path, Pedal, PedalId};
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::tabula::pedal_unit;
use cantus_engine::*;
use support::heavy::Rng;
use support::*;

const SR: f32 = 48_000.0;

fn unit(spec: &EffectSpec) -> EffectUnit {
    let slot = EffectSlot {
        id: EffectId(1),
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: spec.clone(),
    };
    EffectUnit::new(&slot, SR)
}

fn pedal(id: u16, kind: EffectKind, path: Path, kv: &[(&str, f32)]) -> Pedal {
    let mut p = Pedal::new(PedalId(id), EffectSpec::new(kind));
    p.path = path;
    for (k, v) in kv {
        p.spec.set(k, *v).unwrap();
    }
    p
}

fn board(pedals: Vec<Pedal>) -> EffectSpec {
    let mut s = EffectSpec::new(EffectKind::Board);
    s.board = Some(Box::new(Board {
        pedals,
        macros: Vec::new(),
    }));
    s
}

/// A linear-phase EQ that changes nothing but the time: a pure delay of its latency.
fn flat_linear() -> Vec<(&'static str, f32)> {
    vec![("mode", 2.0), ("b0.used", 1.0), ("b0.gain", 0.0)]
}

/// Noise and tones, stereo.
fn music(frames: usize, seed: u64) -> (Vec<f32>, Vec<f32>) {
    let mut rng = Rng::new(seed);
    let (l, r) = support::heavy::signal(frames);
    let jitter = |v: f32, rng: &mut Rng| v + (rng.unit() - 0.5) * 0.01;
    (
        l.into_iter().map(|v| jitter(v, &mut rng)).collect(),
        r.into_iter().map(|v| jitter(v, &mut rng)).collect(),
    )
}

/// Run `x` through `u` in blocks of `block` (0: random blocks from `seed`).
fn run(
    u: &mut EffectUnit,
    x: &(Vec<f32>, Vec<f32>),
    block: usize,
    seed: u64,
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = x.clone();
    let mut rng = Rng::new(seed);
    let mut at = 0;
    while at < l.len() {
        let b = if block == 0 { rng.block() } else { block };
        let b = b.min(l.len() - at);
        u.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
    (l, r)
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn an_empty_board_renders_exactly_as_no_board() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Board);
    assert_null(&reference, &render_of(&p), 0.0, "an empty board");
}

/// A split whose lanes are wires (their pedals switched off) at `ab` 50 %: exactly a wire.
#[test]
fn an_empty_split_is_a_null() {
    let spec = board(vec![
        Pedal {
            on: false,
            ..pedal(0, EffectKind::Chorus, Path::A, &[("mix", 0.5)])
        },
        Pedal {
            on: false,
            ..pedal(1, EffectKind::Tremolo, Path::B, &[])
        },
    ]);
    let mut u = unit(&spec);
    let x = music(20_000, 1);
    let y = run(&mut u, &x, 0, 2);
    assert!(same_bits(&x.0, &y.0) && same_bits(&x.1, &y.1));
    assert_eq!(u.latency(), 0);
}

/// Σ Pre + max(A, B) + Σ Post, whoever is switched off.
#[test]
fn the_latency_is_the_longest_path() {
    let eq = unit(&{
        let mut s = EffectSpec::new(EffectKind::Eq);
        for (k, v) in flat_linear() {
            s.set(k, v);
        }
        s
    })
    .latency();
    let sat = unit(&{
        let mut s = EffectSpec::new(EffectKind::Saturator);
        s.set("os", 3.0);
        s
    })
    .latency();
    assert!(eq > 0 && sat > 0 && eq != sat, "{eq} {sat}");
    let spec = board(vec![
        pedal(0, EffectKind::Saturator, Path::Pre, &[("os", 3.0)]),
        pedal(1, EffectKind::Eq, Path::A, &flat_linear()),
        Pedal {
            on: false,
            ..pedal(2, EffectKind::Saturator, Path::B, &[("os", 3.0)])
        },
        pedal(3, EffectKind::Saturator, Path::Post, &[("os", 3.0)]),
    ]);
    assert_eq!(unit(&spec).latency(), sat + eq.max(sat) + sat);
}

/// An impulse through A (a latent pedal) and B (a wire) arrives once: B waits for A.
#[test]
fn the_lanes_are_aligned() {
    let spec = board(vec![pedal(0, EffectKind::Eq, Path::A, &flat_linear())]);
    let mut u = unit(&spec);
    let lat = u.latency();
    assert!(lat > 0);
    let n = lat * 3;
    let mut x = (vec![0.0; n], vec![0.0; n]);
    x.0[0] = 1.0;
    x.1[0] = 1.0;
    let y = run(&mut u, &x, 64, 0);
    let (top, at) =
        y.0.iter().enumerate().fold(
            (0f32, 0),
            |m, (i, v)| if v.abs() > m.0 { (v.abs(), i) } else { m },
        );
    assert_eq!(at, lat, "the impulse arrives at the board's latency");
    assert!((top - 1.0).abs() < 1e-3, "once, whole: {top}");
    let elsewhere =
        y.0.iter()
            .enumerate()
            .filter(|(i, _)| *i != lat)
            .fold(0f32, |m, (_, v)| m.max(v.abs()));
    assert!(elsewhere < 1e-3, "no second arrival: {elsewhere}");
}

/// A latent pedal switched off keeps the board's latency, and what leaves it is its input
/// exactly that late; switched on again it sounds.
#[test]
fn a_footswitch_never_changes_the_latency() {
    let on = board(vec![pedal(4, EffectKind::Eq, Path::Pre, &flat_linear())]);
    let mut off = on.clone();
    off.board.as_mut().unwrap().pedals[0].on = false;
    let mut u = unit(&on);
    let lat = u.latency();
    let x = music(48_000, 3);
    let _ = run(&mut u, &x, 256, 0);
    u.configure(&off);
    assert_eq!(u.latency(), lat);
    let y = run(&mut u, &x, 256, 0);
    // After the fade (and the line's own length), the input, `lat` late, to the bit.
    let from = 4_800 + lat;
    for i in from..x.0.len() {
        assert_eq!(y.0[i].to_bits(), x.0[i - lat].to_bits(), "frame {i}");
    }
    // A latency-free pedal off is a wire, and stops running.
    let trem = board(vec![pedal(
        0,
        EffectKind::Tremolo,
        Path::Pre,
        &[("depth", 1.0)],
    )]);
    let mut t_off = trem.clone();
    t_off.board.as_mut().unwrap().pedals[0].on = false;
    let mut u = unit(&trem);
    let _ = run(&mut u, &x, 128, 0);
    u.configure(&t_off);
    let y = run(&mut u, &x, 128, 0);
    assert!(same_bits(&x.0[960..], &y.0[960..]));
    assert_eq!(u.latency(), 0);
    u.configure(&trem);
    let y = run(&mut u, &x, 128, 0);
    assert!(!same_bits(&x.0[960..], &y.0[960..]), "back on, it sounds");
}

/// Every structural change lands on the board's grid: the same bits at any block size.
#[test]
fn structural_changes_do_not_depend_on_the_block_size() {
    let rig = cantus_core::fx::tabula::guitar_rig();
    let mut steps: Vec<EffectSpec> = Vec::new();
    // A pedal moved (a dip), one onto lane B, a footswitch, one taken out, the board's
    // levels moved, one added (adopted first), and the order taken back.
    let mut s = rig.clone();
    let b = s.board.as_mut().unwrap();
    let d = b.pedals.remove(2);
    b.pedals.insert(0, d);
    steps.push(s.clone());
    s.board.as_mut().unwrap().pedals[3].path = Path::B;
    steps.push(s.clone());
    s.board.as_mut().unwrap().pedals[1].on = false;
    steps.push(s.clone());
    s.board.as_mut().unwrap().pedals.remove(2);
    steps.push(s.clone());
    s.set("mix", 0.7);
    s.set("ab", 0.25);
    s.set("in", -4.0);
    steps.push(s.clone());
    let added = pedal(9, EffectKind::Eq, Path::Post, &flat_linear());
    s.board.as_mut().unwrap().pedals.push(added.clone());
    steps.push(s.clone());
    steps.push(rig.clone());
    let x = music(9 * 7_000, 5);
    let play = |block: usize, seed: u64| {
        let mut u = unit(&rig);
        let mut rng = Rng::new(seed);
        let (mut l, mut r) = x.clone();
        let mut at = 0;
        for (i, step) in std::iter::once(None)
            .chain(steps.iter().map(Some))
            .enumerate()
        {
            if let Some(spec) = step {
                if spec.pedals().iter().any(|p| p.id == PedalId(9))
                    && u.pedals_mut().iter().all(|k| k.pedal != PedalId(9))
                {
                    assert!(u
                        .adopt(PedalId(9), pedal_unit(EffectId(1), &added, SR))
                        .is_none());
                }
                u.configure(spec);
                if let Some(need) = u.take_request() {
                    drop(u.supply(EffectMemory::build(need.need(), SR)));
                }
            }
            let end = (i + 1) * 7_000;
            while at < end {
                let b = if block == 0 { rng.block() } else { block };
                let b = b.min(end - at);
                u.process(&mut l[at..at + b], &mut r[at..at + b]);
                at += b;
            }
            while u.take_retired().is_some() {}
        }
        (l, r)
    };
    let reference = play(512, 0);
    assert!(reference.0.iter().all(|v| v.is_finite()));
    for (block, seed) in [(1, 0), (7, 0), (4096, 0), (0, 11), (0, 12)] {
        let out = play(block, seed);
        assert!(
            same_bits(&reference.0, &out.0) && same_bits(&reference.1, &out.1),
            "blocks of {block} (seed {seed})"
        );
    }
}

/// A pedal taken out fades and is handed back; the board then holds one fewer.
#[test]
fn a_pedal_taken_out_is_handed_back() {
    let rig = cantus_core::fx::tabula::guitar_rig();
    let mut u = unit(&rig);
    let x = music(4_800, 7);
    let _ = run(&mut u, &x, 256, 0);
    let mut less = rig.clone();
    less.board.as_mut().unwrap().pedals.remove(1);
    u.configure(&less);
    assert!(u.take_retired().is_none(), "it fades first");
    let _ = run(&mut u, &x, 256, 0);
    assert!(u.take_retired().is_some());
    assert!(u.take_retired().is_none());
    assert_eq!(u.pedals_mut().len(), 3);
    // A pedal the board did not ask for waits; one it names joins.
    let more = pedal(7, EffectKind::Phaser, Path::Pre, &[]);
    assert!(u
        .adopt(PedalId(7), pedal_unit(EffectId(1), &more, SR))
        .is_none());
    u.configure(&less);
    assert_eq!(u.pedals_mut().len(), 4, "adopted, not yet named: kept");
    let mut with = less.clone();
    with.board.as_mut().unwrap().pedals.push(more);
    u.configure(&with);
    let _ = run(&mut u, &x, 256, 0);
    assert_eq!(u.pedals_mut().len(), 4);
    assert!(u.take_retired().is_none());
}

/// A pedal after a latent one hears the song as late as its sound is: a board of an EQ in
/// linear phase and a synced tremolo renders as the two inserted one after the other.
#[test]
fn each_pedal_hears_the_song_as_late_as_it_is() {
    let mut inserts = demo_project();
    let eq = add_effect(&mut inserts, EffectKind::Eq);
    modfx::set_all(&mut inserts, eq, &flat_linear());
    let trem = add_effect(&mut inserts, EffectKind::Tremolo);
    modfx::set_all(&mut inserts, trem, &[("depth", 1.0), ("div", 6.0)]);
    let mut boarded = demo_project();
    let b = add_effect(&mut boarded, EffectKind::Board);
    let slot = strip_of(&mut boarded, 0)
        .effects
        .iter_mut()
        .find(|e| e.id == b)
        .unwrap();
    slot.spec = board(vec![
        pedal(0, EffectKind::Eq, Path::Pre, &flat_linear()),
        pedal(
            1,
            EffectKind::Tremolo,
            Path::Pre,
            &[("depth", 1.0), ("div", 6.0)],
        ),
    ]);
    assert_null(
        &render_of(&inserts),
        &render_of(&boarded),
        0.0,
        "board = inserts",
    );
}

/// A mapped target plays where its macro puts it, whatever a lane on it says.
#[test]
fn a_macro_wins_over_automation() {
    let make = |lane: bool, mapped: bool| {
        let mut p = demo_project();
        let b = add_effect(&mut p, EffectKind::Board);
        let slot = strip_of(&mut p, 0)
            .effects
            .iter_mut()
            .find(|e| e.id == b)
            .unwrap();
        let depth = if mapped { 0.0 } else { 0.35 };
        slot.spec = board(vec![pedal(
            0,
            EffectKind::Tremolo,
            Path::Pre,
            &[("depth", depth), ("div", 4.0)],
        )]);
        if mapped {
            slot.spec.board.as_mut().unwrap().macros = vec![Macro {
                name: None,
                maps: vec![MacroMap {
                    pedal: PedalId(0),
                    key: "depth".into(),
                    from: 0.35,
                    to: 0.35,
                }],
            }];
        }
        if lane {
            let c = p.make_automation_clip(
                "a",
                AutoTarget::Effect {
                    effect: b,
                    key: "p0.depth".into(),
                },
                Ticks::ZERO,
                Ticks::beats(8),
                vec![
                    AutoPoint::new(Ticks::ZERO, 0.0),
                    AutoPoint::new(Ticks::beats(8), 1.0),
                ],
            );
            p.tracks[0].clips.push(c);
        }
        p
    };
    let plain = render_of(&make(false, false));
    assert_null(
        &plain,
        &render_of(&make(false, true)),
        0.0,
        "the macro alone",
    );
    assert_null(
        &plain,
        &render_of(&make(true, true)),
        0.0,
        "the macro over a lane",
    );
    let laned = render_of(&make(true, false));
    assert!(
        laned.iter().zip(&plain).any(|(a, b)| a != b),
        "the lane alone moves it"
    );
}

/// The board is the same at every block size inside a project, and live equals offline.
#[test]
fn the_rig_renders_the_same_at_every_block_size() {
    let mut p = demo_project();
    let b = add_effect(&mut p, EffectKind::Board);
    strip_of(&mut p, 0)
        .effects
        .iter_mut()
        .find(|e| e.id == b)
        .unwrap()
        .spec = cantus_core::fx::tabula::guitar_rig();
    let reference = render_with(&p, 512);
    assert!(reference.iter().any(|v| *v != 0.0));
    for block in [1usize, 37, 64, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("blocks of {block}"),
        );
    }
    let live = modfx::realtime(&p, reference.len());
    assert_null(&reference[..live.len()], &live, 0.0, "live = offline");
}
