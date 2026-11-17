//! fx3:tabula — what the board costs beside its pedals (ADR 0131 §3: below 1 %): eight
//! pedals run as eight inserts one after the other, and the same eight on one board, over
//! the same ten seconds of signal at 48 kHz in blocks of 256; best of seven of each.
//!
//! ```text
//! cargo run --release -p cantus-engine --example tabula_overhead
//! ```

use cantus_core::fx::tabula::{Board, Path, Pedal, PedalId};
use cantus_core::{EffectId, EffectKind, EffectSlot, EffectSpec};
use cantus_engine::EffectUnit;
use std::time::Instant;

const SR: f32 = 48_000.0;

fn slot(spec: EffectSpec) -> EffectSlot {
    EffectSlot {
        id: EffectId(1),
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec,
    }
}

fn pedals() -> Vec<EffectSpec> {
    [
        EffectKind::Pedal,
        EffectKind::Compressor,
        EffectKind::Chorus,
        EffectKind::Phaser,
        EffectKind::Tremolo,
        EffectKind::Delay,
        EffectKind::Space,
        EffectKind::Utility,
    ]
    .into_iter()
    .map(cantus_core::starting::starting_spec)
    .collect()
}

fn signal(n: usize) -> (Vec<f32>, Vec<f32>) {
    let mut seed = 0x2545_F491u32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let v = cantus_dsp::math::sin(std::f32::consts::TAU * 196.0 * t) * 0.3 + noise() * 0.05;
            (v, v * 0.9)
        })
        .unzip()
}

fn run(units: &mut [EffectUnit], x: &(Vec<f32>, Vec<f32>)) -> f64 {
    let (mut l, mut r) = x.clone();
    let t = Instant::now();
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        for u in units.iter_mut() {
            u.process(a, b);
        }
    }
    std::hint::black_box((&l, &r));
    t.elapsed().as_secs_f64()
}

fn main() {
    let x = signal(SR as usize * 10);
    let specs = pedals();
    let mut board = EffectSpec::new(EffectKind::Board);
    board.board = Some(Box::new(Board {
        pedals: specs
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut p = Pedal::new(PedalId(i as u16), s.clone());
                p.path = Path::Pre;
                p
            })
            .collect(),
        macros: Vec::new(),
    }));
    let (mut alone, mut boarded) = (f64::MAX, f64::MAX);
    for _ in 0..7 {
        let mut units: Vec<EffectUnit> = specs
            .iter()
            .map(|s| EffectUnit::new(&slot(s.clone()), SR))
            .collect();
        alone = alone.min(run(&mut units, &x));
        let mut b = vec![EffectUnit::new(&slot(board.clone()), SR)];
        boarded = boarded.min(run(&mut b, &x));
    }
    let overhead = (boarded - alone) / alone * 100.0;
    println!(
        "eight pedals as inserts: {:.1} ms; on one board: {:.1} ms; the board's own cost {overhead:+.2} %",
        alone * 1e3,
        boarded * 1e3
    );
}
