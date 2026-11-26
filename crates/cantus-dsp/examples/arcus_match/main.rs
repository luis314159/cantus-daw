//! Arcus measured against recorded cellos and double basses, and matched to them (round 42,
//! `docs/round42/arcus-match.md`): every reference note found under the downloads folder
//! (the Iowa MIS's anechoic runs, TinySOL, VSCO 2 CE's solo bass, Karoryfer's cello,
//! Smolken's bass) and Arcus at the same instrument, technique, key and dynamic, each
//! measured the same way (`features`): harmonics 1–40, the third-octave spectrum, the
//! centroid, the noise between harmonics, the attack, the vibrato, a pluck's decays and
//! thump. The distance of each Arcus note to the references' spread at that note (pooled a
//! semitone either way), on the notes held out of the fit — a third, `(key + dynamic) % 3 ==
//! 0` — before (model 1) and after (model 2 with the fitted match). `fit` fits the match on
//! the rest and writes it into `arcus_tables.rs`; plots of the held-out mf notes go to
//! `<out>/`. Run by `tools/arcus/match.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_match -- <refs dir> <out dir> [fit]
//! ```

mod audio;
mod features;
mod plot;
mod refs;

use cantus_dsp::arcus::tables::{Instrument, Match, EQ_BANDS, EQ_HZ, NEUTRAL};
use cantus_dsp::arcus::{Arcus, ArcusSettings, Technique};
use cantus_dsp::math;
use features::{band_hz, Features, BANDS};
use refs::{Dyn, Inst, Note};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;
/// Arcus's velocity for p, mf, f.
const VEL: [f32; 3] = [0.3, 0.6, 0.9];

type Key = (Inst, bool, u8, Dyn);

fn held_out(key: u8, d: Dyn) -> bool {
    (key as usize + d).is_multiple_of(3)
}

fn render(k: Key, model: u8, m: Option<Match>) -> Features {
    let (inst, pizz, key, d) = k;
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        model,
        matched: m,
        instrument: if inst == Inst::Cello {
            Instrument::Cello
        } else {
            Instrument::Bass
        },
        technique: if pizz {
            Technique::Pizzicato
        } else {
            Technique::Arco
        },
        ..ArcusSettings::default()
    });
    a.note_on(key, VEL[d], 1);
    let secs = if pizz { 1.6 } else { 2.4 };
    let frames = (secs * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    let x: Vec<f32> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
    let steady = if pizz { 0.04 } else { 0.9 };
    features::measure(&x, SR as f64, refs::hz(key as f64), steady, pizz)
}

/// The references' spread for a component: `get` of every reference note at `k` (and a
/// semitone either way), as `(min, max)`; a lone value spreads ±1 dB.
fn spread(refs: &[&Note], get: &dyn Fn(&Features) -> Option<f64>) -> Option<(f64, f64)> {
    let v: Vec<f64> = refs.iter().filter_map(|n| get(&n.f)).collect();
    if v.is_empty() {
        return None;
    }
    let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some((lo - 1.0, hi + 1.0))
}

fn excess(x: f64, (lo, hi): (f64, f64)) -> f64 {
    if x < lo {
        lo - x
    } else if x > hi {
        x - hi
    } else {
        0.0
    }
}

/// The distances (dB, RMS over the components) of `a` to the references' spread: the
/// harmonics 1–20, the third-octave spectrum 63 Hz – 10 kHz, the centroid (semitones), the
/// noise between harmonics (dB).
#[derive(Clone, Copy, Debug, Default)]
struct Dist {
    harm: f64,
    ltas: f64,
    centroid: f64,
    noise: f64,
}

/// A level under this (dB of the whole) is the recordings' noise floor, not the instrument's:
/// both sides are compared clamped to it.
const FLOOR: f64 = -60.0;

fn distance(a: &Features, refs: &[&Note]) -> Dist {
    let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len().max(1) as f64).sqrt();
    let mut h = Vec::new();
    for k in 0..20 {
        if let (Some(x), Some(s)) = (
            a.harm[k],
            spread(refs, &|f| f.harm[k].map(|v| v.max(FLOOR))),
        ) {
            h.push(excess(x.max(FLOOR), s));
        }
    }
    let mut l = Vec::new();
    for b in 2..BANDS - 2 {
        if let Some(s) = spread(refs, &|f| Some(f.ltas[b].max(FLOOR))) {
            l.push(excess(a.ltas[b].max(FLOOR), s));
        }
    }
    let c = spread(refs, &|f| Some(12.0 * math::log2(f.centroid_hz.max(1.0)))).map_or(0.0, |s| {
        excess(12.0 * math::log2(a.centroid_hz.max(1.0)), s)
    });
    let n = spread(refs, &|f| Some(f.noise_db.max(FLOOR)))
        .map_or(0.0, |s| excess(a.noise_db.max(FLOOR), s));
    Dist {
        harm: rms(&h),
        ltas: rms(&l),
        centroid: c,
        noise: n,
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

/// The references grouped by note, and the ones a key's spread pools (a semitone either way).
fn pool(all: &[Note], k: Key) -> Vec<&Note> {
    all.iter()
        .filter(|n| {
            n.inst == k.0
                && n.pizz == k.1
                && n.dynamic == k.3
                && (n.key as i32 - k.2 as i32).abs() <= 1
        })
        .collect()
}

/// One pass of the fit for one instrument: the correction moved by half the median gap
/// between the references' third-octave spectrum and Arcus's, smoothed across the bands (its
/// mean taken out: the level is the presets' fit's); the noise by how far it lies outside the
/// references' spread; the vibrato's depth and rate and the pluck's ring (its fundamental's)
/// by the median ratio.
fn fit_pass(all: &[Note], keys: &[Key], inst: Inst, m: &mut Match) {
    let mut gaps: Vec<Vec<f64>> = vec![Vec::new(); EQ_BANDS];
    let (mut noise, mut ring, mut vib, mut rate) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for k in keys.iter().filter(|k| k.0 == inst && !held_out(k.2, k.3)) {
        let refs = pool(all, *k);
        if refs.is_empty() {
            continue;
        }
        let a = render(*k, 2, Some(*m));
        if !k.1 {
            for (e, hz) in EQ_HZ.iter().enumerate() {
                let mut g = Vec::new();
                for b in 0..BANDS {
                    let bh = band_hz(b);
                    if (12.0 * math::log2(bh / *hz as f64)).abs() <= 4.5 {
                        let r = median(refs.iter().map(|n| n.f.ltas[b].max(FLOOR)).collect());
                        g.push(r - a.ltas[b].max(FLOOR));
                    }
                }
                if !g.is_empty() {
                    gaps[e].push(g.iter().sum::<f64>() / g.len() as f64);
                }
            }
            // The noise moves only by how far it lies outside the references' spread (the
            // recordings' floor and room are in theirs: inside it, it is the instrument's).
            if let Some(sp) = spread(&refs, &|f| Some(f.noise_db.max(FLOOR))) {
                let x = a.noise_db.max(FLOOR);
                noise.push(if x < sp.0 {
                    sp.0 - x
                } else if x > sp.1 {
                    sp.1 - x
                } else {
                    0.0
                });
            }
            let rv: Vec<f64> = refs.iter().filter_map(|n| n.f.vib.map(|v| v.1)).collect();
            if let (false, Some(v)) = (rv.is_empty(), a.vib) {
                vib.push(median(rv) / v.1);
            }
            let rr: Vec<f64> = refs.iter().filter_map(|n| n.f.vib.map(|v| v.0)).collect();
            if let (false, Some(v)) = (rr.is_empty(), a.vib) {
                rate.push(median(rr) / v.0);
            }
        } else {
            // The fundamental's ring (the upper partials' follow the string's own losses).
            for p in 0..1 {
                let r: Vec<f64> = refs
                    .iter()
                    .filter_map(|n| n.f.t60.get(p).copied().flatten())
                    .collect();
                if let (false, Some(Some(t))) = (r.is_empty(), a.t60.get(p)) {
                    ring.push(median(r) / t);
                }
            }
        }
    }
    // Half of each band's median gap, smoothed with its neighbours (¼ ½ ¼: the sections
    // overlap, and fitted one by one they fight).
    let raw: Vec<f64> = gaps.iter().map(|g| median(g.clone())).collect();
    let step: Vec<f64> = (0..EQ_BANDS)
        .map(|e| {
            let at = |i: usize| raw[i.min(EQ_BANDS - 1)];
            let lo = if e == 0 { at(0) } else { at(e - 1) };
            0.5 * (0.25 * lo + 0.5 * at(e) + 0.25 * at(e + 1))
        })
        .collect();
    let mean = step.iter().sum::<f64>() / step.len() as f64;
    for (e, s) in step.iter().enumerate() {
        m.eq_db[e] =
            ((m.eq_db[e] as f64 + s - mean).clamp(-12.0, 12.0) * 10.0).round() as f32 / 10.0;
    }
    if !noise.is_empty() {
        m.noise_db = ((m.noise_db as f64 + 0.7 * median(noise)).clamp(-24.0, 24.0) * 10.0).round()
            as f32
            / 10.0;
    }
    if !vib.is_empty() {
        m.vib_depth =
            ((m.vib_depth as f64 * median(vib)).clamp(0.3, 2.0) * 100.0).round() as f32 / 100.0;
    }
    if !rate.is_empty() {
        m.vib_rate =
            ((m.vib_rate as f64 * median(rate)).clamp(0.5, 2.0) * 100.0).round() as f32 / 100.0;
    }
    if !ring.is_empty() {
        m.pizz_decay =
            ((m.pizz_decay as f64 * median(ring)).clamp(0.3, 3.0) * 100.0).round() as f32 / 100.0;
    }
}

fn write_tables(path: &Path, cello: &Match, bass: &Match) {
    let mut src = std::fs::read_to_string(path).expect("arcus_tables.rs");
    let lit = |m: &Match| {
        let eq: Vec<String> = m.eq_db.iter().map(|g| format!("{g:.1}")).collect();
        format!(
            "Match {{\n    eq_db: [{}],\n    noise_db: {:.1},\n    pizz_decay: {:.2},\n    vib_depth: {:.2},\n    vib_rate: {:.2},\n}}",
            eq.join(", "),
            m.noise_db,
            m.pizz_decay,
            m.vib_depth,
            m.vib_rate
        )
    };
    for (name, m) in [("CELLO_MATCH", cello), ("BASS_MATCH", bass)] {
        let head = format!("pub const {name}: Match = ");
        let at = src.find(&head).expect("the match's constant");
        let end = at + src[at..].find(";\n").expect("its end") + 2;
        src.replace_range(at..end, &format!("{head}{};\n", lit(m)));
    }
    std::fs::write(path, src).expect("write the tables");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("render") {
        // The held-out notes as WAVs for the study's estimator (`tools/arcus/study_compare.py`):
        // `render <out dir>` writes `arcus-<cello|bass>-<arco|pizz>-<key>-<pp|mf|ff>.wav`,
        // each note held 3 s (a pluck 2.5 s) and a second of release, 48 kHz 24-bit mono.
        let out = PathBuf::from(&args[1]);
        std::fs::create_dir_all(&out).expect("the out dir");
        let vel = [0.2f32, 0.55, 0.95];
        // r42:arcus-p6: `ARCUS_SPLIT=train` renders the other two thirds instead (what a
        // change is tuned on; the held-out third judges it once).
        let train = std::env::var("ARCUS_SPLIT").is_ok_and(|s| s == "train");
        for (inst, lo, hi, name) in [
            (Instrument::Cello, 36u8, 79u8, "cello"),
            (Instrument::Bass, 28, 67, "bass"),
        ] {
            for pizz in [false, true] {
                for key in lo..=hi {
                    for (d, v) in vel.iter().enumerate() {
                        if held_out(key, d) == train {
                            continue;
                        }
                        let mut a = Arcus::new(SR);
                        // Pass 4: `ARCUS_FRICTION=thermal` renders with the thermal law;
                        // r42:arcus-p6: `memory` with the friction's memory.
                        let law = std::env::var("ARCUS_FRICTION").unwrap_or_default();
                        a.set(&ArcusSettings {
                            instrument: inst,
                            friction: match law.as_str() {
                                "thermal" => cantus_dsp::arcus::string::Friction::Thermal,
                                "memory" => cantus_dsp::arcus::string::Friction::Memory,
                                _ => cantus_dsp::arcus::string::Friction::Hyperbolic,
                            },
                            technique: if pizz {
                                Technique::Pizzicato
                            } else {
                                Technique::Arco
                            },
                            // r42 arcus-classical: `ARCUS_SHAPE=name=value,…` tries a stroke
                            // shape (`shape::Knobs::parse`).
                            shape_try: std::env::var("ARCUS_SHAPE")
                                .ok()
                                .map(|t| cantus_dsp::arcus::shape::KNOBS.parse(&t)),
                            ..ArcusSettings::default()
                        });
                        a.note_on(key, *v, 1);
                        let (hold, total) = if pizz { (2.5, 2.6) } else { (3.0, 4.0) };
                        let frames = (total * SR) as usize;
                        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
                        for at in (0..frames).step_by(256) {
                            if at == ((hold * SR) as usize) / 256 * 256 {
                                a.note_off(key, 1);
                            }
                            let n = 256.min(frames - at);
                            a.process(&mut l[at..at + n], &mut r[at..at + n]);
                        }
                        let spec = hound::WavSpec {
                            channels: 1,
                            sample_rate: SR as u32,
                            bits_per_sample: 24,
                            sample_format: hound::SampleFormat::Int,
                        };
                        let file = format!(
                            "arcus-{name}-{}-{key}-{}.wav",
                            if pizz { "pizz" } else { "arco" },
                            ["pp", "mf", "ff"][d]
                        );
                        let mut w = hound::WavWriter::create(out.join(file), spec).expect("a wav");
                        for (x, y) in l.iter().zip(&r) {
                            let s = (0.5 * (x + y)).clamp(-1.0, 1.0);
                            w.write_sample((s * 8_388_607.0).round() as i32)
                                .expect("a sample");
                        }
                        w.finalize().expect("the wav");
                    }
                }
            }
        }
        return;
    }
    if args.first().map(String::as_str) == Some("probe") {
        // A note's raw measures: `probe <cello|bass> <arco|pizz> <key> <0|1|2>`.
        let inst = if args[1] == "bass" {
            Inst::Bass
        } else {
            Inst::Cello
        };
        let k: Key = (
            inst,
            args[2] == "pizz",
            args[3].parse().unwrap(),
            args[4].parse().unwrap(),
        );
        let f = render(k, 2, None);
        println!("{f:#?}");
        return;
    }
    let root = PathBuf::from(args.first().map_or("../downloads/arcus", String::as_str));
    let out = PathBuf::from(
        args.get(1)
            .map_or("docs/round42/arcus-match", String::as_str),
    );
    let fit = args.iter().any(|a| a == "fit");
    std::fs::create_dir_all(&out).expect("the out dir");
    let all = refs::load(&root);
    let mut by_source: BTreeMap<(&str, Inst, bool), usize> = BTreeMap::new();
    for n in &all {
        *by_source.entry((n.source, n.inst, n.pizz)).or_default() += 1;
    }
    println!("references:");
    for ((s, i, p), c) in &by_source {
        println!(
            "  {s} {i:?} {}: {c} notes",
            if *p { "pizz" } else { "arco" }
        );
    }
    let mut keys: Vec<Key> = all
        .iter()
        .map(|n| (n.inst, n.pizz, n.key, n.dynamic))
        .collect();
    keys.sort();
    keys.dedup();
    // Keys Arcus plays (the cello C2–G5, the bass E1–G3).
    keys.retain(|k| match k.0 {
        Inst::Cello => (36..=79).contains(&k.2),
        Inst::Bass => (28..=67).contains(&k.2),
    });
    let (mut cello, mut bass) = (
        Instrument::Cello.spec().matched,
        Instrument::Bass.spec().matched,
    );
    if fit {
        cello = NEUTRAL;
        bass = NEUTRAL;
        for pass in 0..5 {
            fit_pass(&all, &keys, Inst::Cello, &mut cello);
            fit_pass(&all, &keys, Inst::Bass, &mut bass);
            println!("pass {pass}: cello {cello:?}\n        bass {bass:?}");
        }
        let tables = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/arcus_tables.rs");
        write_tables(&tables, &cello, &bass);
        println!("written {}", tables.display());
    }
    // Before (model 1) and after (model 2 with the match), on the held-out notes.
    let mut csv = String::from("inst,tech,key,dyn,held_out,refs,before_harm,before_ltas,before_centroid_st,before_noise,after_harm,after_ltas,after_centroid_st,after_noise,ref_attack_ms,arcus_attack_ms,ref_vib_hz,ref_vib_ct,arcus_vib_hz,arcus_vib_ct,ref_t60_1,arcus_t60_1,ref_thump_db,arcus_thump_db,arcus_rms_db\n");
    let mut sums: BTreeMap<(Inst, bool, bool), Vec<(Dist, Dist)>> = BTreeMap::new();
    // The other measures, reference against Arcus before and after, by instrument and
    // technique: (attack ms, vibrato rate, vibrato depth, T60 of partial 1, thump, centroid).
    type Row = [(f64, f64, f64); 6];
    let mut others: BTreeMap<(Inst, bool), Vec<Row>> = BTreeMap::new();
    for k in &keys {
        let refs = pool(&all, *k);
        if refs.is_empty() {
            continue;
        }
        let m = if k.0 == Inst::Cello { cello } else { bass };
        let before = render(*k, 1, None);
        let after = render(*k, 2, Some(m));
        let (db, da) = (distance(&before, &refs), distance(&after, &refs));
        let hold = held_out(k.2, k.3);
        {
            let r = |g: &dyn Fn(&Features) -> Option<f64>| {
                median(refs.iter().filter_map(|n| g(&n.f)).collect())
            };
            let row = |g: &dyn Fn(&Features) -> Option<f64>| {
                (r(g), g(&before).unwrap_or(0.0), g(&after).unwrap_or(0.0))
            };
            others.entry((k.0, k.1)).or_default().push([
                row(&|f| Some(f.attack_ms)),
                row(&|f| f.vib.map(|v| v.0)),
                row(&|f| f.vib.map(|v| v.1)),
                row(&|f| f.t60.first().copied().flatten()),
                row(&|f| if k.1 { Some(f.thump_db) } else { None }),
                row(&|f| Some(f.centroid_hz)),
            ]);
        }
        sums.entry((k.0, k.1, hold)).or_default().push((db, da));
        let rv = median(refs.iter().filter_map(|n| n.f.vib.map(|v| v.0)).collect());
        let rd = median(refs.iter().filter_map(|n| n.f.vib.map(|v| v.1)).collect());
        let rt = median(
            refs.iter()
                .filter_map(|n| n.f.t60.first().copied().flatten())
                .collect(),
        );
        let _ = writeln!(
            csv,
            "{:?},{},{},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.0},{:.0},{:.2},{:.1},{:.2},{:.1},{:.2},{:.2},{:.1},{:.1},{:.1}",
            k.0,
            if k.1 { "pizz" } else { "arco" },
            k.2,
            ["p", "mf", "f"][k.3],
            hold,
            refs.len(),
            db.harm, db.ltas, db.centroid, db.noise,
            da.harm, da.ltas, da.centroid, da.noise,
            median(refs.iter().map(|n| n.f.attack_ms).collect()),
            after.attack_ms,
            rv, rd,
            after.vib.map_or(0.0, |v| v.0),
            after.vib.map_or(0.0, |v| v.1),
            rt,
            after.t60.first().copied().flatten().unwrap_or(0.0),
            median(refs.iter().map(|n| n.f.thump_db).collect()),
            after.thump_db,
            after.rms_db,
        );
        if hold && k.3 == 1 {
            plot::note(&out, *k, &refs, &before, &after);
        }
    }
    std::fs::write(out.join("distances.csv"), csv).expect("the csv");
    println!(
        "
the other measures (medians over every note: references / before / after):"
    );
    for ((inst, pizz), rows) in &others {
        let names = [
            "attack ms",
            "vibrato Hz",
            "vibrato ±ct",
            "T60 1 s",
            "thump dB",
            "centroid Hz",
        ];
        let mut line = String::new();
        for (i, name) in names.iter().enumerate() {
            let col = |j: usize| {
                median(
                    rows.iter()
                        .map(|r| [r[i].0, r[i].1, r[i].2][j])
                        .filter(|v| *v != 0.0)
                        .collect(),
                )
            };
            let _ = write!(
                line,
                " {name} {:.1} / {:.1} / {:.1};",
                col(0),
                col(1),
                col(2)
            );
        }
        println!("  {inst:?} {}:{line}", if *pizz { "pizz" } else { "arco" });
    }
    println!("\nheld-out medians (dB; centroid in semitones), before → after:");
    for ((inst, pizz, hold), v) in &sums {
        if !hold {
            continue;
        }
        let pick = |f: &dyn Fn(&Dist) -> f64, which: usize| {
            median(
                v.iter()
                    .map(|p| f(if which == 0 { &p.0 } else { &p.1 }))
                    .collect(),
            )
        };
        println!(
            "  {inst:?} {} ({} notes): harmonics {:.2} → {:.2}, spectrum {:.2} → {:.2}, centroid {:.2} → {:.2}, noise {:.2} → {:.2}",
            if *pizz { "pizz" } else { "arco" },
            v.len(),
            pick(&|d| d.harm, 0), pick(&|d| d.harm, 1),
            pick(&|d| d.ltas, 0), pick(&|d| d.ltas, 1),
            pick(&|d| d.centroid, 0), pick(&|d| d.centroid, 1),
            pick(&|d| d.noise, 0), pick(&|d| d.noise, 1),
        );
    }
}
