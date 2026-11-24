//! Arcus's violin and viola measured beside recordings (round 42, `arcus-upper`): every
//! reference note found under the references' folder — the Iowa MIS's anechoic violin and
//! viola runs (`Violin.arco.mf.sulA.A4B4.aiff`, `Viola.arco.sulA.mf.A4B4.aiff`), VSCO 2 CE's
//! solo violin (`LLVln_ArcoVib_A4_f.wav`, `LLVln_Pizz_…`), the Good-Sounds violin notes when
//! present — and Arcus at the same instrument, technique, key and dynamic, each measured the
//! same way (the match tool's `features`: harmonics 1–40, third-octave spectrum, centroid, the
//! noise between harmonics, the attack, the vibrato, a pluck's decays). Prints each feature's
//! distance between Arcus and the references beside the distance between the references
//! themselves (two sources at the same note), and writes the table to `<out>/compare.txt`.
//! Run by `tools/arcus/upper-compare.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_upper_compare -- <refs dir> <out dir>
//! ```

// The match tool's reading and measuring, shared as they are (what this tool leaves unused is
// the match tool's).
#[allow(dead_code)]
#[path = "arcus_match/audio.rs"]
mod audio;
#[allow(dead_code)]
#[path = "arcus_match/features.rs"]
mod features;

use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{Arcus, ArcusSettings, Technique};
use cantus_dsp::math;
use features::Features;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;
/// Arcus's velocity for p, mf, f.
const VEL: [f32; 3] = [0.3, 0.6, 0.9];

struct Note {
    viola: bool,
    pizz: bool,
    key: u8,
    dynamic: usize,
    source: &'static str,
    dur: f64,
    f: Features,
}

fn hz(key: f64) -> f64 {
    440.0 * math::exp2((key - 69.0) / 12.0)
}

/// "C2", "Gb2", "A#0" → MIDI, and where the name ended.
fn parse_note(s: &str) -> Option<(u8, usize)> {
    let b = s.as_bytes();
    let base = match b.first()?.to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let (mut i, mut acc) = (1, 0i32);
    if b.get(1) == Some(&b'b') {
        acc = -1;
        i = 2;
    } else if b.get(1) == Some(&b'#') {
        acc = 1;
        i = 2;
    }
    let mut j = i;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    let oct: i32 = s.get(i..j)?.parse().ok()?;
    Some(((12 * (oct + 1) + base + acc) as u8, j))
}

fn dyn_of(tok: &str) -> Option<usize> {
    match tok {
        "pp" | "p" => Some(0),
        "mf" | "mp" => Some(1),
        "ff" | "f" => Some(2),
        _ => None,
    }
}

fn steady_at(dur: f64, pizz: bool) -> Option<f64> {
    if pizz {
        return (dur > 0.3).then_some(0.04);
    }
    let want = (0.3 * dur).clamp(0.3, 0.7);
    if want + features::WIN_S + 0.05 <= dur {
        Some(want)
    } else if dur > features::WIN_S + 0.2 {
        Some(dur - features::WIN_S - 0.1)
    } else {
        None
    }
}

fn onset(x: &[f32]) -> usize {
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    x.iter().position(|v| v.abs() > 0.01 * peak).unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn push(out: &mut Vec<Note>, x: &[f32], sr: f64, meta: (bool, bool, u8, usize, &'static str)) {
    let (viola, pizz, key, dynamic, source) = meta;
    let dur = x.len() as f64 / sr;
    let Some(steady) = steady_at(dur, pizz) else {
        return;
    };
    let f = features::measure(x, sr, hz(key as f64), steady, pizz);
    let heard = 69.0 + 12.0 * math::log2(f.f0 / 440.0);
    if (heard - key as f64).abs() > 0.6 {
        return;
    }
    out.push(Note {
        viola,
        pizz,
        key,
        dynamic,
        source,
        dur,
        f,
    });
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        v.sort();
        for p in v {
            if p.is_dir() {
                files(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

fn load(root: &Path) -> Vec<Note> {
    let mut all = Vec::new();
    files(root, &mut all);
    let mut out = Vec::new();
    for p in all {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let iowa = name.starts_with("Violin.") || name.starts_with("Viola.");
        let vsco = name.starts_with("LLVln_");
        if !iowa && !vsco {
            continue;
        }
        let Some(m) = audio::read(&p) else { continue };
        let parts: Vec<&str> = name
            .trim_end_matches(".aiff")
            .trim_end_matches(".wav")
            .split(['.', '_'])
            .collect();
        if iowa {
            // Violin.arco.mf.sulA.A4B4 · Viola.arco.sulA.mf.A4B4 (the viola's order differs).
            let viola = name.starts_with("Viola.");
            let pizz = parts.get(1) == Some(&"pizz");
            let (d, range) = if viola {
                (parts.get(3), parts.get(4))
            } else {
                (parts.get(2), parts.get(4))
            };
            let (Some(dynamic), Some(range)) = (d.and_then(|t| dyn_of(t)), range) else {
                continue;
            };
            let Some((lo, used)) = parse_note(range) else {
                continue;
            };
            let hi = parse_note(&range[used..]).map_or(lo, |(h, _)| h);
            let segs = audio::segments(&m);
            let span = (hi - lo + 1) as usize;
            if segs.len() != span {
                eprintln!("  {name}: {} notes cut, {span} named — skipped", segs.len());
                continue;
            }
            for (i, (a, b)) in segs.iter().enumerate() {
                let x = &m.x[*a..*b];
                push(
                    &mut out,
                    x,
                    m.sr,
                    (viola, pizz, lo + i as u8, dynamic, "Iowa MIS"),
                );
            }
        } else {
            // LLVln_ArcoVib_A4_f · LLVln_Pizz_A4_f (VSCO 2 CE's solo violin).
            let pizz = parts.get(1).is_some_and(|t| t.starts_with("Pizz"));
            if !pizz && parts.get(1) != Some(&"ArcoVib") {
                continue;
            }
            let (Some((key, _)), Some(dynamic)) = (
                parts.get(2).and_then(|t| parse_note(t)),
                parts.get(3).and_then(|t| dyn_of(t)),
            ) else {
                continue;
            };
            let a = onset(&m.x);
            push(
                &mut out,
                &m.x[a..],
                m.sr,
                (false, pizz, key, dynamic, "VSCO 2 CE"),
            );
        }
    }
    out
}

/// Arcus playing `n`'s note as long as the reference's (arco held, the pluck left ringing).
fn arcus(n: &Note) -> Option<Features> {
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        instrument: if n.viola {
            Instrument::Viola
        } else {
            Instrument::Violin
        },
        technique: if n.pizz {
            Technique::Pizzicato
        } else {
            Technique::Arco
        },
        ..ArcusSettings::default()
    });
    let dur = n.dur.clamp(1.0, 4.0);
    let frames = ((dur + 0.4) * SR as f64) as usize;
    let off = (dur * SR as f64) as usize;
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    a.note_on(n.key, VEL[n.dynamic], 1);
    for at in (0..frames).step_by(256) {
        if at <= off && off < at + 256 {
            a.note_off(n.key, 1);
        }
        let k = 256.min(frames - at);
        a.process(&mut l[at..at + k], &mut r[at..at + k]);
    }
    let x = &l[onset(&l)..off];
    let steady = steady_at(x.len() as f64 / SR as f64, n.pizz)?;
    Some(features::measure(
        x,
        SR as f64,
        hz(n.key as f64),
        steady,
        n.pizz,
    ))
}

/// Each feature's distance between two notes' measures.
fn dist(a: &Features, b: &Features, pizz: bool) -> [Option<f64>; 6] {
    let harm = {
        let d: Vec<f64> = (0..12)
            .filter_map(|k| Some(a.harm.get(k)?.as_ref()? - b.harm.get(k)?.as_ref()?))
            .collect();
        (!d.is_empty()).then(|| (d.iter().map(|x| x * x).sum::<f64>() / d.len() as f64).sqrt())
    };
    let ltas = {
        let d: Vec<f64> = (3..23).map(|i| a.ltas[i] - b.ltas[i]).collect();
        let m = d.iter().sum::<f64>() / d.len() as f64;
        Some((d.iter().map(|x| (x - m).powi(2)).sum::<f64>() / d.len() as f64).sqrt())
    };
    let cent = Some(1200.0 * math::log2(a.centroid_hz.max(1.0) / b.centroid_hz.max(1.0)) / 100.0);
    let noise = Some(a.noise_db - b.noise_db);
    let (attack, decay) = if pizz {
        let r: Vec<f64> = (0..4)
            .filter_map(|k| {
                Some(math::log2(
                    a.t60.get(k)?.as_ref()? / b.t60.get(k)?.as_ref()?,
                ))
            })
            .collect();
        (
            None,
            (!r.is_empty()).then(|| r.iter().sum::<f64>() / r.len() as f64),
        )
    } else {
        (Some(a.attack_ms - b.attack_ms), None)
    };
    [harm, ltas, cent, noise, attack, decay]
}

const NAMES: [&str; 6] = [
    "harmonics 1–12 (dB rms)",
    "third-octave spectrum 100 Hz–10 kHz (dB rms)",
    "centroid (semitones)",
    "noise between harmonics (dB)",
    "attack 10–90 % (ms)",
    "pluck's decay, partials 1–4 (octaves of T60)",
];

/// The long-term third-octave spectrum (dB, its mean taken out) of `x` at `sr`, 100 Hz – 10 kHz:
/// 8192-point Hann frames, half-overlapped, frames under −50 dB of the loudest left out.
fn ltas(x: &[f32], sr: f64) -> Vec<f64> {
    let n = 8192;
    let mut frames: Vec<(f64, Vec<f64>)> = Vec::new();
    let mut at = 0;
    while at + n <= x.len() {
        let mut re: Vec<f32> = (0..n)
            .map(|i| {
                x[at + i]
                    * (0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64)) as f32
            })
            .collect();
        let mut im = vec![0.0f32; n];
        cantus_dsp::spectrum::fft(&mut re, &mut im);
        let mut bands = vec![0.0f64; 21];
        let mut total = 0.0;
        for b in 1..n / 2 {
            let hz = b as f64 * sr / n as f64;
            let p = (re[b] as f64).powi(2) + (im[b] as f64).powi(2);
            total += p;
            let k = (3.0 * math::log2(hz / 100.0) + 0.5).floor();
            if k >= 0.0 && (k as usize) < bands.len() {
                bands[k as usize] += p;
            }
        }
        frames.push((total, bands));
        at += n / 2;
    }
    let loud = frames.iter().fold(0.0f64, |m, f| m.max(f.0));
    let mut sum = vec![0.0f64; 21];
    for (t, b) in &frames {
        if *t > loud * 1e-5 {
            for (s, v) in sum.iter_mut().zip(b) {
                *s += v;
            }
        }
    }
    let db: Vec<f64> = sum
        .iter()
        .map(|v| 10.0 * math::log10(v.max(1e-30)))
        .collect();
    let mean = db.iter().sum::<f64>() / db.len() as f64;
    db.iter().map(|v| v - mean).collect()
}

fn rms_diff(a: &[f64], b: &[f64]) -> f64 {
    (a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f64>() / a.len() as f64).sqrt()
}

/// The CNSM dataset's scales (Zenodo 10.5281/zenodo.18696786, CC BY 4.0: three violins, the
/// same players) beside Arcus's violin playing a two-octave G major scale up and down, détaché
/// at mf: each violin's long-term spectrum, the distances between them and to Arcus's.
fn cnsm(refs: &Path, report: &mut String) {
    let mut all = Vec::new();
    files(refs, &mut all);
    let mut by: BTreeMap<String, Vec<Vec<f64>>> = BTreeMap::new();
    for p in all
        .iter()
        .filter(|p| p.to_string_lossy().contains("recordings"))
    {
        let s = p.to_string_lossy().replace('\\', "/");
        let Some(violin) = ["Klimke", "Levaggi", "Stoppani"]
            .into_iter()
            .find(|v| s.contains(v))
        else {
            continue;
        };
        let Some(m) = audio::read(p) else { continue };
        by.entry(violin.to_string())
            .or_default()
            .push(ltas(&m.x, m.sr));
    }
    if by.is_empty() {
        return;
    }
    let mean = |v: &[Vec<f64>]| -> Vec<f64> {
        (0..21)
            .map(|i| v.iter().map(|x| x[i]).sum::<f64>() / v.len() as f64)
            .collect()
    };
    let violins: Vec<(String, Vec<f64>)> = by.iter().map(|(k, v)| (k.clone(), mean(v))).collect();
    // Arcus: G3 … G5 and back, 0.5 s a note, re-bowed.
    let scale = [55u8, 57, 59, 60, 62, 64, 66, 67, 69, 71, 72, 74, 76, 78, 79];
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        instrument: Instrument::Violin,
        mode: cantus_dsp::voices::PlayMode::Mono,
        ..ArcusSettings::default()
    });
    let keys: Vec<u8> = scale
        .iter()
        .chain(scale.iter().rev().skip(1))
        .cloned()
        .collect();
    let per = (0.5 * SR) as usize;
    let mut out = vec![0.0f32; per * keys.len() + SR as usize];
    let mut r = out.clone();
    for (i, k) in keys.iter().enumerate() {
        a.note_on(*k, 0.6, i as u64 + 1);
        let at = i * per;
        a.process(&mut out[at..at + per - 960], &mut r[at..at + per - 960]);
        a.note_off(*k, i as u64 + 1);
        a.process(
            &mut out[at + per - 960..at + per],
            &mut r[at + per - 960..at + per],
        );
    }
    let arcus = ltas(&out, SR as f64);
    // The match's correction it suggests (`tables::Match::eq_db` at `EQ_HZ`): the violins'
    // mean long-term spectrum over Arcus's, read at each band's centre, its mean taken out
    // and held within ±9 dB.
    let target: Vec<f64> = (0..21)
        .map(|i| violins.iter().map(|(_, l)| l[i]).sum::<f64>() / violins.len() as f64)
        .collect();
    let diff: Vec<f64> = target.iter().zip(&arcus).map(|(t, a)| t - a).collect();
    let at = |hz: f64| {
        let x = (3.0 * math::log2(hz / 100.0)).clamp(0.0, 20.0);
        let (i, fr) = (x.floor() as usize, x - x.floor());
        diff[i] + (diff[(i + 1).min(20)] - diff[i]) * fr
    };
    let eq: Vec<f64> = cantus_dsp::arcus::tables::EQ_HZ
        .iter()
        .map(|h| at(*h as f64))
        .collect();
    let m = eq.iter().sum::<f64>() / eq.len() as f64;
    let eq: Vec<String> = eq
        .iter()
        .map(|v| format!("{:.1}", (v - m).clamp(-9.0, 9.0)))
        .collect();
    let _ = writeln!(
        report,
        "  the correction it suggests (dB at EQ_HZ): [{}]",
        eq.join(", ")
    );
    let _ = writeln!(
        report,
        "The CNSM violins' scales, long-term third-octave spectra (100 Hz – 10 kHz, dB rms):"
    );
    for (i, (va, la)) in violins.iter().enumerate() {
        let others: Vec<String> = violins
            .iter()
            .skip(i + 1)
            .map(|(vb, lb)| format!("{vb} {:.2}", rms_diff(la, lb)))
            .collect();
        let _ = writeln!(
            report,
            "  {va} ({} recordings): to Arcus {:.2}{}{}",
            by[va].len(),
            rms_diff(la, &arcus),
            if others.is_empty() { "" } else { "; to " },
            others.join(", ")
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let refs = PathBuf::from(args.get(1).map_or("refs", |s| s.as_str()));
    let out = PathBuf::from(args.get(2).map_or("compare", |s| s.as_str()));
    let notes = load(&refs);
    eprintln!("{} reference notes", notes.len());
    // Per group (instrument, technique): Arcus vs each reference; reference vs reference at
    // the same note and dynamic from another source.
    let mut acc: BTreeMap<(bool, bool), [(Vec<f64>, Vec<f64>); 6]> = BTreeMap::new();
    let mut vib: BTreeMap<(bool, bool), Vec<(f64, f64)>> = BTreeMap::new();
    let mut rows = String::new();
    for n in &notes {
        let Some(f) = arcus(n) else { continue };
        let e = acc
            .entry((n.viola, n.pizz))
            .or_insert_with(Default::default);
        let d = dist(&f, &n.f, n.pizz);
        for (i, v) in d.iter().enumerate() {
            if let Some(v) = v {
                e[i].0.push(*v);
            }
        }
        for m in &notes {
            if m.source != n.source
                && m.source > n.source
                && (m.viola, m.pizz, m.key, m.dynamic) == (n.viola, n.pizz, n.key, n.dynamic)
            {
                for (i, v) in dist(&m.f, &n.f, n.pizz).iter().enumerate() {
                    if let Some(v) = v {
                        e[i].1.push(*v);
                    }
                }
            }
        }
        if let (Some(a), Some(b)) = (f.vib, n.f.vib) {
            vib.entry((n.viola, n.pizz))
                .or_default()
                .push((a.0 - b.0, a.1 - b.1));
        }
        let _ = writeln!(
            rows,
            "{} {} key {} dyn {} ({}): harm {:.1} dB, ltas {:.1} dB, centroid {:+.2} st, noise {:+.1} dB, attack {:+.0} ms",
            if n.viola { "viola" } else { "violin" },
            if n.pizz { "pizz" } else { "arco" },
            n.key,
            ["p", "mf", "f"][n.dynamic],
            n.source,
            d[0].unwrap_or(f64::NAN),
            d[1].unwrap_or(f64::NAN),
            d[2].unwrap_or(f64::NAN),
            d[3].unwrap_or(f64::NAN),
            d[4].unwrap_or(f64::NAN)
        );
    }
    let mut report = String::new();
    let stat = |v: &[f64]| {
        if v.is_empty() {
            return "—".to_string();
        }
        let n = v.len() as f64;
        let mean = v.iter().sum::<f64>() / n;
        let rms = (v.iter().map(|x| x * x).sum::<f64>() / n).sqrt();
        format!("mean {mean:+.2}, rms {rms:.2} (n {})", v.len())
    };
    for ((viola, pizz), e) in &acc {
        let _ = writeln!(
            report,
            "{} {}:",
            if *viola { "Viola" } else { "Violin" },
            if *pizz { "pizzicato" } else { "arco" }
        );
        for (i, (a, r)) in e.iter().enumerate() {
            if a.is_empty() {
                continue;
            }
            let _ = writeln!(
                report,
                "  {:<46} Arcus–recording {} · recording–recording {}",
                NAMES[i],
                stat(a),
                stat(r)
            );
        }
        if let Some(v) = vib.get(&(*viola, *pizz)) {
            let (r, d): (Vec<f64>, Vec<f64>) = v.iter().cloned().unzip();
            let _ = writeln!(
                report,
                "  vibrato rate (Hz) {} · depth (± cents) {}",
                stat(&r),
                stat(&d)
            );
        }
    }
    cnsm(&refs, &mut report);
    print!("{report}");
    std::fs::create_dir_all(&out).ok();
    std::fs::write(out.join("compare.txt"), format!("{report}\n{rows}")).expect("write");
    eprintln!("written {}", out.join("compare.txt").display());
}
