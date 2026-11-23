//! Arcus's bowed pitch (ADR 0157's gate, item 3): with the measured table (`arcus::pitch`), a
//! bowed note sounds within a cent of its key over the table's grid and between its points —
//! keys a few cents off the grid, dynamics and contact points between the measured ones —
//! for the cello and the double bass; and with no table it is the stick and slip's own (as
//! Fides measured, a few cents), so the table is doing the work.

use super::pitch::{DYN, POS};
use super::tables::Instrument;
use super::tests::{cents, measure_hz, SR};
use super::*;
use crate::math;

/// The cents a bowed note at `key` (fractional, by `fine` cents) and dynamic `d` and contact
/// `pos` sounds from its target, with the built-in table or none.
fn offset(inst: Instrument, key: u8, fine: f32, d: f64, pos: f64, table: bool) -> f64 {
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        instrument: inst,
        vib_depth: 0.0,
        body_mix: 0.0,
        dyn_src: DynSource::Velocity,
        bow_pos: pos as f32,
        fine_ct: fine,
        ..ArcusSettings::default()
    });
    if !table {
        a.set_pitch_table(None);
    }
    a.note_on(key, (d as f32).max(0.01), 1);
    let frames = (1.5 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    let want = 440.0 * math::exp2((key as f64 + fine as f64 / 100.0 - 69.0) / 12.0);
    cents(measure_hz(&l, want, 0.5, 1.45), want)
}

#[test]
fn the_bowed_pitch_is_within_a_cent_on_the_grid_and_between() {
    let mut worst = (0f64, String::new());
    let mut raw = 0f64;
    for inst in [Instrument::Cello, Instrument::Bass] {
        let spec = inst.spec();
        let mut key = spec.bottom();
        while key <= spec.top {
            for d in DYN {
                for p in POS {
                    let e = offset(inst, key, 0.0, d, p, true);
                    if e.abs() > worst.0 {
                        worst = (e.abs(), format!("{inst:?} key {key} d {d} pos {p}: {e:+.3} ct"));
                    }
                }
            }
            // Between the points: a dynamic, a contact and a key between the measured ones.
            for (d, p, fine) in [(0.37, -0.4, 37.0f32), (0.62, 0.55, -23.0), (0.9, 0.2, 49.0)] {
                let e = offset(inst, key, fine, d, p, true);
                if e.abs() > worst.0 {
                    worst = (
                        e.abs(),
                        format!("{inst:?} key {key}{fine:+} ct d {d} pos {p}: {e:+.3} ct"),
                    );
                }
            }
            raw = raw.max(offset(inst, key, 0.0, 0.5, 0.0, false).abs());
            key += 3;
        }
    }
    println!("worst bowed with the table: {}", worst.1);
    println!("worst at mf with none: {raw:.2} ct");
    assert!(worst.0 <= 1.0, "{}", worst.1);
}
