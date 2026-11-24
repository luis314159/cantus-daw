//! The references: every recording found under the downloads folder, cut into notes, each
//! note's instrument, technique, key and dynamic read from its name (the key checked by its
//! own fundamental among the octaves around it).

use super::audio::{self, Mono};
use super::features::{self, Features};
use cantus_dsp::math;
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Inst {
    Cello,
    Bass,
}

/// 0 p, 1 mf, 2 f.
pub type Dyn = usize;

#[derive(Clone, Debug)]
pub struct Note {
    pub inst: Inst,
    pub pizz: bool,
    pub key: u8,
    pub dynamic: Dyn,
    pub source: &'static str,
    pub f: Features,
}

pub fn hz(key: f64) -> f64 {
    440.0 * math::exp2((key - 69.0) / 12.0)
}

/// "C2", "Gb2", "A#0", "Ab5" → MIDI.
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
    let mut i = 1;
    let mut acc = 0i32;
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

fn dyn_of(tok: &str) -> Option<Dyn> {
    match tok {
        "pp" | "p" | "v1" => Some(0),
        "mf" | "mp" => Some(1),
        "ff" | "f" | "v3" => Some(2),
        _ => None,
    }
}

/// The key, among `key − 12`, `key`, `key + 12`, whose first eight harmonics hold the most.
fn check_key(m: &Mono, a: usize, b: usize, key: u8) -> u8 {
    let win = &m.x[a..b];
    let f = features::find_f0(win, m.sr, hz(key as f64), true);
    let k = 69.0 + 12.0 * math::log2(f / 440.0);
    k.round().clamp(20.0, 100.0) as u8
}

/// Where a single-note file's note starts: its first sample over −40 dB under its peak.
fn onset(x: &[f32]) -> usize {
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    x.iter().position(|v| v.abs() > 0.01 * peak).unwrap_or(0)
}

/// The steady window's start (s from the onset) for a note `dur` seconds long.
pub fn steady_at(dur: f64, pizz: bool) -> Option<f64> {
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

fn push(
    out: &mut Vec<Note>,
    m: &Mono,
    a: usize,
    b: usize,
    meta: (Inst, bool, u8, Dyn, &'static str),
) {
    let (inst, pizz, key, dynamic, source) = meta;
    let dur = (b - a) as f64 / m.sr;
    let Some(steady) = steady_at(dur, pizz) else {
        return;
    };
    let f = features::measure(&m.x[a..b], m.sr, hz(key as f64), steady, pizz);
    let heard = 69.0 + 12.0 * math::log2(f.f0 / 440.0);
    if (heard - key as f64).abs() > 0.6 {
        return; // not the note its name says (a slide, a harmonic, a mis-cut)
    }
    out.push(Note {
        inst,
        pizz,
        key,
        dynamic,
        source,
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

/// Every reference note under `root` (`F:\projects\daw-r42\downloads\arcus`).
pub fn load(root: &Path) -> Vec<Note> {
    let mut all = Vec::new();
    files(root, &mut all);
    let mut out = Vec::new();
    for p in all {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let Some(m) = audio::read(&p) else {
            continue;
        };
        let parts: Vec<&str> = name
            .trim_end_matches(".aiff")
            .trim_end_matches(".aif")
            .trim_end_matches(".wav")
            .split(['.', '_', '-'])
            .collect();
        if name.starts_with("Cello.") || name.starts_with("Bass.") {
            // Iowa MIS: Inst.tech.dyn.string.range — a chromatic run.
            let inst = if name.starts_with("Cello.") {
                Inst::Cello
            } else {
                Inst::Bass
            };
            let pizz = parts.get(1) == Some(&"pizz");
            let Some(dynamic) = parts.get(2).and_then(|t| dyn_of(t)) else {
                continue;
            };
            let range = parts.get(4).copied().unwrap_or("");
            let Some((lo, used)) = parse_note(range) else {
                continue;
            };
            let hi = parse_note(&range[used..]).map_or(lo, |(h, _)| h);
            let segs = audio::segments(&m);
            let span = (hi - lo + 1) as usize;
            for (i, (a, b)) in segs.iter().enumerate() {
                let key = if segs.len() == span {
                    lo + i as u8
                } else {
                    check_key(&m, *a, *b, lo + (i.min(span - 1)) as u8)
                };
                if key < lo || key > hi {
                    continue;
                }
                push(&mut out, &m, *a, *b, (inst, pizz, key, dynamic, "Iowa MIS"));
            }
        } else if name.starts_with("Vc-") || name.starts_with("Cb-") {
            // TinySOL: Vc-ord-A3-ff-4c-N.wav
            let inst = if name.starts_with("Vc-") {
                Inst::Cello
            } else {
                Inst::Bass
            };
            if parts.get(1) != Some(&"ord") {
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
                &m,
                a,
                m.x.len(),
                (inst, false, key, dynamic, "TinySOL"),
            );
        } else if name.starts_with("BKCtbss_") {
            // VSCO 2 CE's solo bass: BKCtbss_SusNV_E0_v1_rr1 — its octaves one under ours.
            let pizz = parts.get(1) == Some(&"Pizz");
            if !pizz && parts.get(1) != Some(&"SusNV") {
                continue;
            }
            let (Some((key, _)), Some(dynamic)) = (
                parts.get(2).and_then(|t| parse_note(t)),
                parts.get(3).and_then(|t| dyn_of(t)),
            ) else {
                continue;
            };
            let a = onset(&m.x);
            let key = check_key(&m, a, m.x.len().min(a + (1.2 * m.sr) as usize), key + 12);
            push(
                &mut out,
                &m,
                a,
                m.x.len(),
                (Inst::Bass, pizz, key, dynamic, "VSCO 2 CE"),
            );
        } else if name.starts_with("cello_") && name.ends_with("_d.wav") {
            // Karoryfer Bigcat: cello_A2_mf_d.wav — its octaves one under ours.
            let (Some((key, _)), Some(dynamic)) = (
                parts.get(1).and_then(|t| parse_note(t)),
                parts.get(2).and_then(|t| dyn_of(t)),
            ) else {
                continue;
            };
            let a = onset(&m.x);
            let key = check_key(&m, a, m.x.len().min(a + (1.2 * m.sr) as usize), key + 12);
            push(
                &mut out,
                &m,
                a,
                m.x.len(),
                (Inst::Cello, false, key, dynamic, "Karoryfer"),
            );
        } else if name.starts_with("bass_arco_") || name.starts_with("bass_pizz_") {
            // Smolken: bass_arco_d2_mf_down.wav, bass_pizz_d2_ma.wav.
            let pizz = name.starts_with("bass_pizz_");
            let Some((key, _)) = parts.get(2).and_then(|t| parse_note(t)) else {
                continue;
            };
            let dynamic = parts.get(3).and_then(|t| dyn_of(t)).unwrap_or(1);
            let a = onset(&m.x);
            push(
                &mut out,
                &m,
                a,
                m.x.len(),
                (Inst::Bass, pizz, key, dynamic, "Smolken"),
            );
        }
    }
    out
}
