//! fx/final — a unit snapped after a configure is the unit made at those settings: for every
//! native kind, a unit made at its defaults, configured at the bench's heavy settings
//! (`support/heavy.rs`) or at settings drawn at random, and snapped (`EffectUnit::snap`)
//! renders the same bits as a unit made at those settings from the start — nothing glides
//! from where it was. This is what lets a render that starts mid-song be the whole render
//! from there (`render_from_bar.rs`): at a start, a seek or a loop's wrap the processor
//! snaps every unit a lane drives.

mod support;
use cantus_core::*;
use cantus_engine::{EffectMemory, EffectUnit};
use support::heavy::{self, Rng, SAMPLE_RATE};

fn unit(spec: &EffectSpec) -> EffectUnit {
    let slot = EffectSlot {
        id: EffectId(1),
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: spec.clone(),
    };
    let mut u = EffectUnit::new(&slot, SAMPLE_RATE);
    feed(&mut u);
    u
}

/// Hand the unit the memory it asks for, as the processor's partner does.
fn feed(u: &mut EffectUnit) {
    for _ in 0..4 {
        let Some(need) = u.take_request() else { return };
        let memory = EffectMemory::build(need.need(), SAMPLE_RATE);
        let _ = u.supply(memory);
    }
}

fn run(u: &mut EffectUnit, x: &(Vec<f32>, Vec<f32>)) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = x.clone();
    let mut at = 0;
    while at < l.len() {
        let b = 256.min(l.len() - at);
        u.process_keyed(&mut l[at..at + b], &mut r[at..at + b], None);
        at += b;
    }
    (l, r)
}

fn first_difference(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>)) -> Option<String> {
    for (ch, (x, y)) in [(&a.0, &b.0), (&a.1, &b.1)].into_iter().enumerate() {
        if let Some(i) = (0..x.len()).find(|&i| x[i].to_bits() != y[i].to_bits()) {
            return Some(format!(
                "channel {ch}, frame {i}: {:e} against {:e}",
                x[i], y[i]
            ));
        }
    }
    None
}

#[test]
fn a_snapped_unit_is_the_unit_made_at_its_settings_for_every_kind() {
    let x = heavy::signal(SAMPLE_RATE as usize / 4);
    let mut rng = Rng::new(0x5eed_5aa9);
    let mut failures = Vec::new();
    for kind in EffectKind::ALL {
        if kind == EffectKind::Board {
            // A board's pedals are units of their own, built off the audio thread and
            // adopted (`EngineMessage::AddPedal`): a configure cannot make them. Its snap is
            // its pedals' (`fx_tabula`, `render_from_bar`).
            continue;
        }
        // From the defaults to the heavy settings and to settings drawn at random, and from
        // one draw to another (a parameter that moves, not one that switches on).
        let mut pairs = vec![(
            "defaults to heavy".to_string(),
            EffectSpec::new(kind),
            heavy::heavy(kind),
        )];
        for i in 0..3 {
            let to = heavy::random_effect(kind, 1.0, &mut rng);
            pairs.push((
                format!("defaults to random #{i}"),
                EffectSpec::new(kind),
                to,
            ));
            let from = heavy::random_effect(kind, 1.0, &mut rng);
            let to = heavy::random_effect(kind, 0.5, &mut rng);
            pairs.push((format!("random to random #{i}"), from, to));
        }
        for (name, mut from, to) in pairs {
            let stop = |s: &EffectSpec| s.get("speed").round() == 0.0;
            if kind == EffectKind::Rotary && stop(&from) != stop(&to) {
                // A brake parks the rotors where it parks them: their angle is the cabinet's
                // state, which a snap keeps (the speed it snaps). Compared from the same
                // side of the brake.
                from.set("speed", to.get("speed"));
            }
            let mut moved = unit(&from);
            moved.configure(&to);
            feed(&mut moved);
            moved.snap();
            let mut made = unit(&to);
            if let Some(d) = first_difference(&run(&mut moved, &x), &run(&mut made, &x)) {
                failures.push(format!("{kind:?} ({name}): {d}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
