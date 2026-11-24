//! Arcus's bowed pitch (ADR 0157's gate, item 3): with the measured table (`arcus::pitch`), a
//! bowed note sounds within a cent of its key on the table's grid and between its points — the
//! table and the closed loop that trims what it leaves (`arcus::ear`) —
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
    let mut all: Vec<(f64, String)> = Vec::new();
    for inst in [Instrument::Cello, Instrument::Bass] {
        let spec = inst.spec();
        let mut key = spec.bottom();
        let mut n = 0usize;
        while key <= spec.top {
            // On the grid (a dynamic and a contact the table holds), and held out: keys a few
            // cents off, dynamics and contacts between the measured ones, none used to build
            // the table.
            let held: [(f64, f64, f32); 4] = [
                (DYN[(n * 3) % DYN.len()], POS[(n * 2) % POS.len()], 0.0),
                (
                    0.06 + 0.11 * (n % 9) as f64,
                    -0.9 + 0.37 * (n % 5) as f64,
                    37.0,
                ),
                (
                    0.93 - 0.13 * (n % 7) as f64,
                    0.85 - 0.41 * (n % 4) as f64,
                    -23.0,
                ),
                (
                    0.31 + 0.07 * (n % 5) as f64,
                    0.62 - 0.29 * (n % 6) as f64,
                    49.0,
                ),
            ];
            for (d, p, fine) in held {
                let e = offset(inst, key, fine, d, p, true);
                all.push((
                    e.abs(),
                    format!("{inst:?} key {key}{fine:+} d {d:.2} pos {p:.2}: {e:+.2}"),
                ));
                if e.abs() > worst.0 {
                    worst = (
                        e.abs(),
                        format!("{inst:?} key {key}{fine:+} ct d {d:.2} pos {p:.2}: {e:+.3} ct"),
                    );
                }
            }
            raw = raw.max(offset(inst, key, 0.0, 0.5, 0.0, false).abs());
            key += 2;
            n += 1;
        }
    }
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    let over = all.iter().filter(|e| e.0 > 1.0).count();
    println!("{} points, {over} over a cent; the worst:", all.len());
    for e in all.iter().take(25) {
        println!("  {}", e.1);
    }
    println!("worst bowed with the table: {}", worst.1);
    println!("worst at mf with none: {raw:.2} ct");
    let median = all[all.len() / 2].0;
    let within3 = all.iter().filter(|e| e.0 <= 3.0).count();
    println!(
        "median {median:.3} ct; {} of {} within a cent, {within3} within 3",
        all.len() - over,
        all.len()
    );
    // The gate asks a cent everywhere, and it holds: the table and the loop that hears the
    // fundamental's phase (`ear`) — measured 168 of 168, the worst 0.52 cent, the median
    // 0.011 (`docs/round42/arcus.md`). Round 42's first loop timed the main slips and left
    // four points 1.0–1.4 cents off (pp far over the fingerboard on the lowest strings,
    // where the motion settles for a second with the slips' period steady but the
    // fundamental's phase drifting).
    assert!(
        over == 0,
        "{over} of {} over a cent; the worst {}",
        all.len(),
        worst.1
    );
    assert!(worst.0 <= 1.0, "the worst {}", worst.1);
    assert!(median <= 0.1, "median {median}");
}

/// A probe (ignored): the gate's hardest points (round 42's first loop's last four and two
/// more) over four seconds — the pitch in windows, the loop's trim every 0.1 s and the bow's
/// place in the Helmholtz region (force over its lower and upper limits), its speed and
/// gesture: `cargo test -p cantus-dsp --release --lib arcus::tests_pitch::probe -- --ignored
/// --nocapture`.
#[test]
#[ignore]
fn probe_the_last_points() {
    let pts: [(Instrument, u8, f32, f64, f64); 6] = [
        (Instrument::Bass, 32, 37.0, 0.28, -0.16),
        (Instrument::Cello, 36, 37.0, 0.06, -0.90),
        (Instrument::Cello, 38, 37.0, 0.17, -0.53),
        (Instrument::Bass, 34, 49.0, 0.52, -0.25),
        (Instrument::Bass, 30, 37.0, 0.17, -0.53),
        (Instrument::Cello, 76, 37.0, 0.28, -0.90),
    ];
    for (inst, key, fine, d, pos) in pts {
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
        a.note_on(key, (d as f32).max(0.01), 1);
        let frames = (4.0 * SR) as usize;
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        let mut trims = Vec::new();
        for (i, at) in (0..frames).step_by(256).enumerate() {
            let n = 256.min(frames - at);
            a.process(&mut l[at..at + n], &mut r[at..at + n]);
            if i % 19 == 18 && at < 2 * SR as usize {
                let v = &a.voices[0];
                let (lo, hi) = v.limits();
                trims.push(format!(
                    "{:.2} [F/lo {:.2} F/hi {:.2} v {:.3} {:?}]",
                    v.trim,
                    v.force / lo.max(1e-9),
                    v.force / hi.max(1e-9),
                    v.v_mag,
                    v.phase
                ));
            }
        }
        let want = 440.0 * math::exp2((key as f64 + fine as f64 / 100.0 - 69.0) / 12.0);
        let w: Vec<String> = [(0.5, 1.45), (1.0, 2.0), (2.0, 3.0), (3.0, 3.95)]
            .iter()
            .map(|(a, b)| format!("{:+.2}", cents(measure_hz(&l, want, *a, *b), want)))
            .collect();
        println!("{inst:?} {key}{fine:+} d {d} pos {pos}: windows {w:?}; trim per 0.1 s {trims:?}");
    }
}
