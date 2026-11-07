//! The comparison's report: every note and phrase measured (`analyse.rs`) for each source,
//! the first source the reference; `report.md` (the gaps by register and band, the phrases),
//! `notes.csv`, `phrases.csv` and the plots.

use super::analyse::{self, NoteM, OCTAVES};
use super::phrases::{self, DYNAMICS, NOTE_EVERY, NOTE_HELD};
use super::plot;
use super::render;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// The registers the summaries group keys by.
pub const REGISTERS: [(&str, u8, u8); 3] = [
    ("bass A0-B2", 21, 47),
    ("middle C3-B5", 48, 83),
    ("treble C6-C8", 84, 108),
];
/// Octave groups of the third-octave shapes (centre, the thirds it averages).
pub const SHAPE_OCTAVES: [(&str, usize); 9] = [
    ("63", 3),
    ("125", 6),
    ("250", 9),
    ("500", 12),
    ("1k", 15),
    ("2k", 18),
    ("4k", 21),
    ("8k", 24),
    ("16k", 27),
];

pub fn colour(i: usize) -> plot::Rgb {
    [
        plot::REFERENCE,
        plot::MODEL1,
        plot::MODEL2,
        plot::GREEN,
        plot::PURPLE,
    ][i % 5]
}

pub fn mean(v: impl Iterator<Item = f64>) -> f64 {
    let (s, n) = v
        .filter(|x| x.is_finite())
        .fold((0.0, 0), |(s, n), x| (s + x, n + 1));
    if n == 0 {
        f64::NAN
    } else {
        s / n as f64
    }
}

/// An octave's level from a third-octave shape: the three thirds around its centre, summed.
pub fn oct(shape: &[f64], centre: usize) -> f64 {
    let e: f64 = (centre.saturating_sub(1)..=(centre + 1).min(shape.len() - 1))
        .map(|i| cantus_dsp::math::powf(10.0f64, shape[i] / 10.0))
        .sum();
    analyse::db(e)
}

pub fn f1(x: f64) -> String {
    if x.is_finite() {
        format!("{x:+.1}")
    } else {
        "–".into()
    }
}

pub fn r2(x: f64) -> String {
    if x.is_finite() {
        format!("{x:.2}")
    } else {
        "–".into()
    }
}

pub type Notes = BTreeMap<(String, &'static str), Vec<NoteM>>;

pub fn measure_notes(out: &Path, names: &[String]) -> Result<Notes, String> {
    let mut all = Notes::new();
    for name in names {
        for (d, vel) in DYNAMICS {
            let path = out.join("wav").join(name).join(format!("notes_{d}.wav"));
            let s = render::read_wav(&path)?;
            let v = phrases::single_keys()
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    analyse::note(&s, *k, vel, analyse::secs(i as f64 * NOTE_EVERY), NOTE_HELD)
                })
                .collect();
            all.insert((name.clone(), d), v);
        }
    }
    Ok(all)
}

pub fn notes_csv(out: &Path, all: &Notes, offset: &BTreeMap<String, f64>) -> Result<(), String> {
    let mut s = String::from("source,dyn,key,vel,level_aligned_db,peak_db,centroid_early,centroid_mid,thump_db,noise,rise_ms,decay_early,decay_late");
    for o in OCTAVES {
        let _ = write!(s, ",decay_{o}");
    }
    s.push_str(",corr,balance_db,side_db,release_ms,release_hf_db");
    for c in analyse::thirds() {
        let _ = write!(s, ",early_{c:.0}");
    }
    for c in analyse::thirds() {
        let _ = write!(s, ",mid_{c:.0}");
    }
    s.push('\n');
    for ((name, d), v) in all {
        for m in v {
            let _ = write!(
                s,
                "{name},{d},{},{},{:.2},{:.2},{:.0},{:.0},{:.2},{:.4},{:.1},{:.2},{:.2}",
                m.key,
                m.vel,
                m.level + offset[name],
                m.peak,
                m.centroid_early,
                m.centroid_mid,
                m.thump,
                m.noise,
                m.rise_ms,
                m.decay_early,
                m.decay_late
            );
            for x in &m.decay {
                let _ = write!(s, ",{x:.2}");
            }
            let _ = write!(
                s,
                ",{:.3},{:.2},{:.2},{:.0},{:.2}",
                m.corr, m.balance, m.side, m.release_ms, m.release_hf
            );
            for x in m.early.iter().chain(&m.mid) {
                let _ = write!(s, ",{x:.2}");
            }
            s.push('\n');
        }
    }
    std::fs::write(out.join("notes.csv"), s).map_err(|e| e.to_string())
}

/// `f(src, ref)` averaged over the notes of a register at one dynamic.
pub fn reg_mean(
    all: &Notes,
    name: &str,
    refn: &str,
    d: &'static str,
    (lo, hi): (u8, u8),
    f: &dyn Fn(&NoteM, &NoteM) -> f64,
) -> f64 {
    let a = &all[&(name.to_string(), d)];
    let b = &all[&(refn.to_string(), d)];
    mean(
        a.iter()
            .zip(b)
            .filter(|(m, _)| m.key >= lo && m.key <= hi)
            .map(|(m, r)| f(m, r)),
    )
}

pub fn ratio(a: f64, b: f64) -> f64 {
    if a.is_finite() && b.is_finite() && b.abs() > 1e-9 {
        a / b
    } else {
        f64::NAN
    }
}

pub fn summary(md: &mut String, all: &Notes, names: &[String], offset: &BTreeMap<String, f64>) {
    let refn = &names[0];
    let _ = writeln!(md, "## Single notes against {refn}, by register\n");
    let _ = writeln!(md, "Each source's levels are aligned so its mean mezzo-forte level equals the reference's (offsets: {}). Shapes are third-octave spectra relative to each note's own total (level-free), grouped by octave; a positive number is *more* energy there than the reference. Decay ratios are the source's dB/s over the reference's (> 1: dies faster).\n",
        offset.iter().map(|(k, v)| format!("{k} {v:+.1} dB")).collect::<Vec<_>>().join(", "));
    for name in &names[1..] {
        let _ = writeln!(md, "### {name}\n");
        for (d, _) in DYNAMICS {
            let _ = writeln!(md, "**{d}** — spectral shape difference, first 50 ms (dB per octave band) / at 0.5 s\n");
            let mut head = String::from("| register | Δlevel |");
            let mut rule = String::from("|---|---|");
            for (o, _) in SHAPE_OCTAVES {
                let _ = write!(head, " {o} |");
                rule.push_str("---|");
            }
            let _ = writeln!(md, "{head}\n{rule}");
            for (rn, lo, hi) in REGISTERS {
                let lvl = reg_mean(all, name, refn, d, (lo, hi), &|m, r| {
                    m.level + offset[name] - r.level - offset[refn]
                });
                let mut row = format!("| {rn} | {} |", f1(lvl));
                for (_, c) in SHAPE_OCTAVES {
                    let e = reg_mean(all, name, refn, d, (lo, hi), &|m, r| {
                        oct(&m.early, c) - oct(&r.early, c)
                    });
                    let mid = reg_mean(all, name, refn, d, (lo, hi), &|m, r| {
                        oct(&m.mid, c) - oct(&r.mid, c)
                    });
                    let _ = write!(row, " {} / {} |", f1(e), f1(mid));
                }
                let _ = writeln!(md, "{row}");
            }
            let _ = writeln!(
                md,
                "\nattack, decay, stereo and release (source vs reference)\n"
            );
            let _ = writeln!(md, "| register | centroid 50 ms | centroid 0.5 s | thump dB | noise | rise ms | decay early dB/s | decay late dB/s | decay ×(250/1k/4k) | side dB | release ms | release HF dB |\n|---|---|---|---|---|---|---|---|---|---|---|---|");
            for (rn, lo, hi) in REGISTERS {
                let g = |f: &dyn Fn(&NoteM) -> f64| {
                    (
                        reg_mean(all, name, refn, d, (lo, hi), &|m, _| f(m)),
                        reg_mean(all, refn, refn, d, (lo, hi), &|m, _| f(m)),
                    )
                };
                let pair = |p: (f64, f64), fmt: &dyn Fn(f64) -> String| {
                    format!("{} vs {}", fmt(p.0), fmt(p.1))
                };
                let hz = |x: f64| {
                    if x.is_finite() {
                        format!("{x:.0}")
                    } else {
                        "–".into()
                    }
                };
                let dec = |b: usize| {
                    reg_mean(all, name, refn, d, (lo, hi), &|m, r| {
                        ratio(m.decay[b], r.decay[b])
                    })
                };
                let _ = writeln!(
                    md,
                    "| {rn} | {} | {} | {} | {} | {} | {} | {} | {}/{}/{} | {} | {} | {} |",
                    pair(g(&|m| m.centroid_early), &hz),
                    pair(g(&|m| m.centroid_mid), &hz),
                    pair(g(&|m| m.thump), &f1),
                    pair(g(&|m| m.noise), &r2),
                    pair(g(&|m| m.rise_ms), &hz),
                    pair(g(&|m| m.decay_early), &f1),
                    pair(g(&|m| m.decay_late), &f1),
                    r2(dec(1)),
                    r2(dec(3)),
                    r2(dec(5)),
                    pair(g(&|m| m.side), &f1),
                    pair(g(&|m| m.release_ms), &hz),
                    pair(g(&|m| m.release_hf), &f1),
                );
            }
            let _ = writeln!(md);
        }
    }
}

/// One number per source: the RMS of the shape differences (early and 0.5 s, every third
/// octave 63 Hz – 16 kHz, every note and dynamic) against the reference, in dB.
pub fn distance(all: &Notes, name: &str, refn: &str) -> (f64, f64) {
    let (mut e, mut m, mut n) = (0.0, 0.0, 0);
    for (d, _) in DYNAMICS {
        let a = &all[&(name.to_string(), d)];
        let b = &all[&(refn.to_string(), d)];
        for (x, r) in a.iter().zip(b) {
            for i in 3..28 {
                let de = x.early[i] - r.early[i];
                let dm = x.mid[i] - r.mid[i];
                if de.is_finite() && dm.is_finite() {
                    e += de * de;
                    m += dm * dm;
                    n += 1;
                }
            }
        }
    }
    ((e / n.max(1) as f64).sqrt(), (m / n.max(1) as f64).sqrt())
}

/// One number for a voicing search, lower closer to the reference, and its parts: the
/// shapes' distance (dB), the decays' (10·|log₂ ratio|, early and late), the knock's (dB/2),
/// the brightness's (10·|log₂ ratio| of the centroids), the evenness across the keyboard
/// (the spread of the aligned level differences, dB) and the stereo's (side, dB/2).
pub fn score(all: &Notes, name: &str, refn: &str, offset: &BTreeMap<String, f64>) -> [f64; 7] {
    let (e, m) = distance(all, name, refn);
    let l2 = |a: f64, b: f64| {
        if a.is_finite() && b.is_finite() && a > 0.0 && b > 0.0 {
            (cantus_dsp::math::log2(a / b)).abs()
        } else {
            f64::NAN
        }
    };
    let mut parts = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (d, _) in DYNAMICS {
        let a = &all[&(name.to_string(), d)];
        let b = &all[&(refn.to_string(), d)];
        for (x, r) in a.iter().zip(b) {
            parts[0].push(
                10.0 * mean(
                    [
                        l2(x.decay_early, r.decay_early),
                        l2(x.decay_late, r.decay_late),
                    ]
                    .into_iter(),
                ),
            );
            parts[1].push(0.5 * (x.thump - r.thump).abs());
            parts[2].push(
                10.0 * mean(
                    [
                        l2(x.centroid_early, r.centroid_early),
                        l2(x.centroid_mid, r.centroid_mid),
                    ]
                    .into_iter(),
                ),
            );
            parts[3].push(x.level + offset[name] - r.level - offset[refn]);
            parts[4].push(0.5 * (x.side - r.side).abs());
        }
    }
    let lv = mean(parts[3].iter().copied());
    let spread = mean(parts[3].iter().map(|x| (x - lv) * (x - lv))).sqrt();
    let s = [
        0.5 * (e + m),
        mean(parts[0].iter().copied()),
        mean(parts[1].iter().copied()),
        mean(parts[2].iter().copied()),
        spread,
        mean(parts[4].iter().copied()),
        0.0,
    ];
    let total: f64 = s[..6].iter().filter(|x| x.is_finite()).sum();
    [s[0], s[1], s[2], s[3], s[4], s[5], total]
}
