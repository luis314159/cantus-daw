//! The violin's and the viola's bowed-pitch table (round 42, `arcus-upper`): the cello's
//! method (`examples/arcus_pitch_table.rs`) on the upper strings, whose voices run at
//! `upper::factor` × the rate — every key bowed at nine dynamics and five contact points, its
//! fundamental measured, the offset written as the loop's correction, then measured again
//! with it applied and what is left added. Writes `crates/cantus-dsp/src/arcus_upper_pitch_data.rs`.
//! Run by `tools/arcus/upper-pitch.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_upper_pitch -- <path of arcus_upper_pitch_data.rs>
//! ```

use cantus_dsp::arcus::pitch::{InstTable, PitchTable, DYN, POS, TABLE};
use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{Arcus, ArcusSettings, DynSource};
use cantus_dsp::math;
use std::fmt::Write as _;

const SR: f32 = 48_000.0;

/// The fundamental near `near` Hz over `from…to` s (Fides's measure: heterodyne, two boxcars
/// of four periods, the phase's least-squares slope).
fn measure_hz(x: &[f32], near: f64, from: f64, to: f64) -> f64 {
    let sr = SR as f64;
    let len = ((4.0 * sr / near).round() as usize).max(4);
    let w = std::f64::consts::TAU * near / sr;
    let (mut re, mut im) = (Vec::with_capacity(x.len()), Vec::with_capacity(x.len()));
    for (n, v) in x.iter().enumerate() {
        let (s, c) = math::sin_cos(w * n as f64);
        re.push(*v as f64 * c);
        im.push(-(*v as f64) * s);
    }
    let boxcar = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; v.len()];
        let mut acc = 0.0;
        for i in 0..v.len() {
            acc += v[i];
            if i >= len {
                acc -= v[i - len];
            }
            out[i] = acc / len as f64;
        }
        out
    };
    let (re, im) = (boxcar(&boxcar(&re)), boxcar(&boxcar(&im)));
    let (a, b) = ((from * sr) as usize, ((to * sr) as usize).min(x.len()));
    let (mut ts, mut ph) = (Vec::new(), Vec::new());
    let (mut last, mut unwrap) = (0.0f64, 0.0f64);
    for i in (a..b).step_by(16) {
        let p = math::atan2(im[i], re[i]);
        if !ts.is_empty() {
            let d = p - last;
            if d > std::f64::consts::PI {
                unwrap -= std::f64::consts::TAU;
            } else if d < -std::f64::consts::PI {
                unwrap += std::f64::consts::TAU;
            }
        }
        last = p;
        ts.push(i as f64 / sr);
        ph.push(p + unwrap);
    }
    let n = ts.len() as f64;
    let (mt, mp) = (ts.iter().sum::<f64>() / n, ph.iter().sum::<f64>() / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (t, p) in ts.iter().zip(&ph) {
        num += (t - mt) * (p - mp);
        den += (t - mt) * (t - mt);
    }
    near + num / den / std::f64::consts::TAU
}

/// The settings a table's point is measured with: no vibrato, no body, the dynamic from the
/// velocity alone.
pub fn probe_settings(inst: Instrument, pos: f64) -> ArcusSettings {
    ArcusSettings {
        instrument: inst,
        vib_depth: 0.0,
        body_mix: 0.0,
        dyn_src: DynSource::Velocity,
        bow_pos: pos as f32,
        ..ArcusSettings::default()
    }
}

/// The cents a bowed `key` at dynamic `d` and contact `pos` sounds from its target, with
/// `table` correcting it (or none).
pub fn offset(
    inst: Instrument,
    key: u8,
    d: f64,
    pos: f64,
    table: Option<&'static PitchTable>,
) -> f64 {
    let mut a = Arcus::new(SR);
    a.set(&probe_settings(inst, pos));
    a.set_pitch_table(table);
    a.note_on(key, (d as f32).max(0.01), 1);
    let frames = (1.5 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    let want = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    1200.0 * math::log2(measure_hz(&l, want, 0.5, 1.45) / want)
}

fn leak(v: Vec<f32>) -> &'static [f32] {
    Box::leak(v.into_boxed_slice())
}

/// One instrument's table measured with `table` applied (zeros: none), its keys in
/// parallel on the machine's threads (each point its own instance: the same numbers in any
/// order).
fn measure(inst: Instrument, lo: u8, keys: usize, with: Option<&'static PitchTable>) -> Vec<f32> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, 4);
    let per = DYN.len() * POS.len();
    let mut v = vec![0.0f32; keys * per];
    std::thread::scope(|sc| {
        for (t, part) in v
            .chunks_mut(per * keys.div_ceil(threads).max(1))
            .enumerate()
        {
            let first = t * keys.div_ceil(threads).max(1);
            sc.spawn(move || {
                for (i, cell) in part.iter_mut().enumerate() {
                    let k = first + i / per;
                    let (d, p) = (DYN[(i % per) / POS.len()], POS[i % POS.len()]);
                    *cell = offset(inst, lo + k as u8, d, p, with) as f32;
                }
            });
        }
    });
    v
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "crates/cantus-dsp/src/arcus_upper_pitch_data.rs".into());
    let ranges: Vec<(Instrument, &str, u8, usize, usize)> = [
        (Instrument::Violin, "VIOLIN", 2usize),
        (Instrument::Viola, "VIOLA", 3),
    ]
    .iter()
    .map(|(i, name, slot)| {
        let s = i.spec();
        (
            *i,
            *name,
            s.bottom(),
            (s.top - s.bottom() + 1) as usize,
            *slot,
        )
    })
    .collect();
    let first: Vec<Vec<f32>> = ranges
        .iter()
        .map(|(i, _, lo, keys, _)| measure(*i, *lo, *keys, None))
        .collect();
    let mut t1 = TABLE;
    for (j, (_, _, lo, keys, slot)) in ranges.iter().enumerate() {
        t1.tables[*slot] = InstTable {
            lo: *lo,
            keys: *keys,
            cents: leak(first[j].clone()),
        };
    }
    let t1: &'static PitchTable = Box::leak(Box::new(t1));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "//! Arcus's violin and viola bowed-pitch table — **generated** by
         //! `tools/arcus/upper-pitch.sh` (`examples/arcus_upper_pitch.rs`); do not edit by hand.
         //! Cents each bowed key sounds from its loop, per key × dynamic ({:?}) × contact point
         //! ({:?}), measured twice (the offset, then what was left with it applied) at 48 kHz
         //! (the voices at `upper::factor` × it) with no vibrato and no body.

         #![allow(clippy::approx_constant)] // measured cents that happen to read like π or √2
",
        DYN, POS
    );
    for (j, (inst, name, lo, keys, _)) in ranges.iter().enumerate() {
        let left = measure(*inst, *lo, *keys, Some(t1));
        let fin: Vec<f32> = first[j].iter().zip(&left).map(|(a, b)| a + b).collect();
        let worst = left.iter().fold(0f32, |m, x| m.max(x.abs()));
        for (i, l) in left.iter().enumerate() {
            if l.abs() > 1.0 {
                let (k, rest) = (i / (DYN.len() * POS.len()), i % (DYN.len() * POS.len()));
                eprintln!(
                    "  {name} key {} d {} pos {}: offset {:+.2}, left {:+.2}",
                    *lo as usize + k,
                    DYN[rest / POS.len()],
                    POS[rest % POS.len()],
                    first[j][i],
                    l
                );
            }
        }
        let span = first[j]
            .iter()
            .fold((0f32, 0f32), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
        eprintln!(
            "{name}: offsets {:+.2} … {:+.2} ct; left after pass 1 at most {worst:.3} ct",
            span.0, span.1
        );
        let _ = writeln!(out, "pub const {name}_LO: u8 = {lo};");
        let _ = writeln!(out, "pub const {name}_KEYS: usize = {keys};");
        let _ = writeln!(out, "pub const {name}: [f32; {}] = [", fin.len());
        for row in fin.chunks(DYN.len() * POS.len()) {
            let cells: Vec<String> = row.iter().map(|c| format!("{c:.4}")).collect();
            let _ = writeln!(out, "    {},", cells.join(", "));
        }
        let _ = writeln!(out, "];");
    }
    std::fs::write(&path, out).expect("write the table");
    eprintln!("written {path}");
}
