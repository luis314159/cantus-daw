//! Arcus's violin and viola for the owner's ears (round 42, `arcus-upper`): per instrument a
//! solo note at pp, mf and ff, a four-bar legato phrase with a CC1 swell, pizzicato, tremolo
//! and détaché with the bow's direction alternating; and **ABX pairs** — a recorded note (the
//! Iowa MIS's anechoic violin and viola, VSCO 2 CE's solo violin) and Arcus playing the same
//! key, dynamic and length, both at −23 LUFS — with `abx.html`'s `SETS` printed for them.
//! 48 kHz 24-bit WAVs (a reference at its own rate). Run by `tools/arcus/upper-listen.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_upper_listen -- <out dir> <refs dir>
//! ```

#[allow(dead_code)]
#[path = "arcus_match/audio.rs"]
mod audio;
#[allow(dead_code)]
#[path = "arcus_match/features.rs"]
mod features;

use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{Arcus, ArcusSettings, Art, BowDir, Technique};
use cantus_dsp::math;
use cantus_dsp::terminus::loudness::LoudnessMeter;
use cantus_dsp::voices::PlayMode;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;

#[derive(Copy, Clone, Debug)]
enum Ev {
    On(u8, f32),
    Off(u8),
    Cc1(f32),
    /// The next note's own articulation and bow.
    Art(u8, Option<Art>, Option<BowDir>),
}

/// A render: its file's name, the settings, the score (seconds, event), its length (s).
type Job = (String, ArcusSettings, Vec<(f64, Ev)>, f64);

fn id(k: u8) -> u64 {
    k as u64 + 9_000
}

fn render(s: &ArcusSettings, score: &[(f64, Ev)], secs: f64) -> (Vec<f32>, Vec<f32>) {
    let mut a = Arcus::new(SR);
    a.set(s);
    let frames = (secs * SR as f64) as usize;
    let mut ev: Vec<(usize, Ev)> = score
        .iter()
        .map(|(t, e)| ((t * SR as f64) as usize, *e))
        .collect();
    ev.sort_by_key(|e| e.0);
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    let (mut at, mut i) = (0usize, 0usize);
    while at < frames {
        while i < ev.len() && ev[i].0 <= at {
            match ev[i].1 {
                Ev::On(k, v) => a.note_on(k, v, id(k)),
                Ev::Off(k) => a.note_off(k, id(k)),
                Ev::Cc1(v) => a.controller(0, v),
                Ev::Art(k, art, bow) => a.note_art(id(k), art, bow),
            }
            i += 1;
        }
        let next = ev.get(i).map_or(frames, |e| e.0);
        let n = 256.min(next.max(at + 1) - at).min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l, r)
}

fn write(path: &Path, sr: f64, l: &[f32], r: &[f32], gain: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sr.round() as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create");
    for (a, b) in l.iter().zip(r) {
        for x in [a, b] {
            let v = ((x * gain).clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
            w.write_sample(v).expect("write");
        }
    }
    w.finalize().expect("finalize");
}

/// The gain that brings `l`/`r` (at `sr`) to −23 LUFS integrated.
fn to_lufs23(sr: f64, l: &[f32], r: &[f32]) -> (f32, f64) {
    let mut m = LoudnessMeter::new(sr as f32);
    m.process(l, r);
    let lufs = m.integrated().unwrap_or(-70.0);
    (math::db_to_gain((-23.0 - lufs) as f32), lufs)
}

/// A legato line (key, beats) at `bpm`, each note overlapping the next by 30 ms, with a CC1
/// swell over bars 2–3 and back over bar 4 (the cello's, `arcus_listen.rs`).
fn legato(line: &[(u8, f64)], bpm: f64, vel: f32, lo: f32, hi: f32) -> (Vec<(f64, Ev)>, f64) {
    let beat = 60.0 / bpm;
    let mut ev = vec![(0.0, Ev::Cc1(lo))];
    let mut t = 0.2;
    for (i, (k, beats)) in line.iter().enumerate() {
        ev.push((t, Ev::On(*k, vel)));
        let end = t + beats * beat;
        ev.push((
            if i + 1 < line.len() { end + 0.03 } else { end },
            Ev::Off(*k),
        ));
        t = end;
    }
    let bar = 4.0 * beat;
    for j in 0..=80 {
        let u = j as f64 / 80.0;
        let (time, v) = if u < 2.0 / 3.0 {
            (0.2 + bar + u * 3.0 * bar, lo + (hi - lo) * (u * 1.5) as f32)
        } else {
            let w = (u - 2.0 / 3.0) * 3.0;
            (0.2 + 3.0 * bar + w * bar, hi + (lo - hi) * w as f32)
        };
        ev.push((time, Ev::Cc1(v)));
    }
    (ev, t + 2.0)
}

/// A reference note: its file, which note of a run (Iowa's), the instrument, the key it is
/// named, the velocity Arcus plays it at, plucked.
struct Pair {
    tag: &'static str,
    file: &'static str,
    nth: usize,
    inst: Instrument,
    key: u8,
    vel: f32,
    pizz: bool,
}

const PAIRS: &[Pair] = &[
    // The study's listening references (`downloads/study/abx-refs`, TinySOL, CC BY 4.0:
    // IRCAM's instruments, ordinario, the second string).
    Pair {
        tag: "violin-A4-tinysol-pp",
        file: "violin_tinysol_A4_pp.wav",
        nth: 0,
        inst: Instrument::Violin,
        key: 69,
        vel: 0.22,
        pizz: false,
    },
    Pair {
        tag: "violin-A4-tinysol-mf",
        file: "violin_tinysol_A4_mf.wav",
        nth: 0,
        inst: Instrument::Violin,
        key: 69,
        vel: 0.6,
        pizz: false,
    },
    Pair {
        tag: "violin-A4-tinysol-ff",
        file: "violin_tinysol_A4_ff.wav",
        nth: 0,
        inst: Instrument::Violin,
        key: 69,
        vel: 0.95,
        pizz: false,
    },
    Pair {
        tag: "viola-D4-tinysol-pp",
        file: "viola_tinysol_D4_pp.wav",
        nth: 0,
        inst: Instrument::Viola,
        key: 62,
        vel: 0.22,
        pizz: false,
    },
    Pair {
        tag: "viola-D4-tinysol-mf",
        file: "viola_tinysol_D4_mf.wav",
        nth: 0,
        inst: Instrument::Viola,
        key: 62,
        vel: 0.6,
        pizz: false,
    },
    Pair {
        tag: "viola-D4-tinysol-ff",
        file: "viola_tinysol_D4_ff.wav",
        nth: 0,
        inst: Instrument::Viola,
        key: 62,
        vel: 0.95,
        pizz: false,
    },
    Pair {
        tag: "violin-A4-f",
        file: "LLVln_ArcoVib_A4_f.wav",
        nth: 0,
        inst: Instrument::Violin,
        key: 69,
        vel: 0.85,
        pizz: false,
    },
    Pair {
        tag: "violin-A4-p",
        file: "LLVln_ArcoVib_A4_p.wav",
        nth: 0,
        inst: Instrument::Violin,
        key: 69,
        vel: 0.35,
        pizz: false,
    },
    Pair {
        tag: "violin-E5-f",
        file: "LLVln_ArcoVib_E5_f.wav",
        nth: 0,
        inst: Instrument::Violin,
        key: 76,
        vel: 0.85,
        pizz: false,
    },
    Pair {
        tag: "violin-G3-f",
        file: "LLVln_ArcoVib_G3_f.wav",
        nth: 0,
        inst: Instrument::Violin,
        key: 55,
        vel: 0.85,
        pizz: false,
    },
    Pair {
        tag: "violin-D4-mf",
        file: "Violin.arco.mf.sulD.D4A4.aiff",
        nth: 0,
        inst: Instrument::Violin,
        key: 62,
        vel: 0.6,
        pizz: false,
    },
    Pair {
        tag: "violin-A4-pizz",
        file: "Violin.pizz.mf.sulA.A4B4.aiff",
        nth: 0,
        inst: Instrument::Violin,
        key: 69,
        vel: 0.6,
        pizz: true,
    },
    Pair {
        tag: "viola-A4-mf",
        file: "Viola.arco.sulA.mf.A4B4.aiff",
        nth: 0,
        inst: Instrument::Viola,
        key: 69,
        vel: 0.6,
        pizz: false,
    },
    Pair {
        tag: "viola-A4-ff",
        file: "Viola.arco.sulA.ff.A4B4.aiff",
        nth: 0,
        inst: Instrument::Viola,
        key: 69,
        vel: 0.9,
        pizz: false,
    },
    Pair {
        tag: "viola-A4-pp",
        file: "Viola.arco.sulA.pp.A4B4.aiff",
        nth: 0,
        inst: Instrument::Viola,
        key: 69,
        vel: 0.3,
        pizz: false,
    },
    Pair {
        tag: "viola-D4-mf",
        file: "Viola.arco.sulD.mf.D4B4.aiff",
        nth: 0,
        inst: Instrument::Viola,
        key: 62,
        vel: 0.6,
        pizz: false,
    },
    Pair {
        tag: "viola-C3-mf",
        file: "Viola.arco.sulC.mf.C3B3.aiff",
        nth: 0,
        inst: Instrument::Viola,
        key: 48,
        vel: 0.6,
        pizz: false,
    },
    Pair {
        tag: "viola-D4-pizz",
        file: "Viola.pizz.sulD.mf.D4B4.aiff",
        nth: 0,
        inst: Instrument::Viola,
        key: 62,
        vel: 0.6,
        pizz: true,
    },
];

fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            if let Some(f) = find(&p, name) {
                return Some(f);
            }
        } else if p.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(p);
        }
    }
    None
}

/// The pairs: each reference cut and loudness-matched, Arcus beside it; `abx.html`'s SETS.
fn pairs(out: &Path, refs: &Path) -> String {
    let dir = out.join("ref");
    std::fs::create_dir_all(&dir).expect("ref dir");
    let mut sets = String::from("const SETS = {\n");
    for p in PAIRS {
        let Some(path) = find(refs, p.file) else {
            eprintln!("  {}: {} not found under {}", p.tag, p.file, refs.display());
            continue;
        };
        let Some(m) = audio::read(&path) else {
            continue;
        };
        let (a, b) = if p.file.ends_with(".aiff") {
            let segs = audio::segments(&m);
            match segs.get(p.nth) {
                Some(s) => *s,
                None => continue,
            }
        } else {
            let peak = m.x.iter().fold(0f32, |q, v| q.max(v.abs()));
            let a = m.x.iter().position(|v| v.abs() > 0.01 * peak).unwrap_or(0);
            (a, m.x.len())
        };
        // At most 4 s, with a 20 ms fade at the cut's end (a run's next note follows).
        let b = b.min(a + (4.0 * m.sr) as usize);
        let mut x: Vec<f32> = m.x[a..b].to_vec();
        let fade = ((0.02 * m.sr) as usize).min(x.len());
        let n = x.len();
        for (i, v) in x[n - fade..].iter_mut().enumerate() {
            *v *= 1.0 - i as f32 / fade as f32;
        }
        // The key the recording sounds (its file's name can be an octave off).
        let f0 = features::find_f0(
            &x,
            m.sr,
            440.0 * math::exp2((p.key as f64 - 69.0) / 12.0),
            true,
        );
        let key = (69.0 + 12.0 * math::log2(f0 / 440.0)).round() as u8;
        let dur = n as f64 / m.sr;
        let s = ArcusSettings {
            instrument: p.inst,
            technique: if p.pizz {
                Technique::Pizzicato
            } else {
                Technique::Arco
            },
            ..ArcusSettings::default()
        };
        let (al, ar) = render(
            &s,
            &[(0.0, Ev::On(key, p.vel)), (dur - 0.15, Ev::Off(key))],
            dur + 0.3,
        );
        let (g_ref, l_ref) = to_lufs23(m.sr, &x, &x);
        let (g_arc, l_arc) = to_lufs23(SR as f64, &al, &ar);
        write(
            &dir.join(format!("{}-rec-lufs23.wav", p.tag)),
            m.sr,
            &x,
            &x,
            g_ref,
        );
        write(
            &dir.join(format!("{}-arcus-lufs23.wav", p.tag)),
            SR as f64,
            &al,
            &ar,
            g_arc,
        );
        eprintln!(
            "  {}: {} cut {:.2} s, key {key} (named {}), {:.1} / {:.1} LUFS raw",
            p.tag, p.file, dur, p.key, l_ref, l_arc
        );
        let source = if p.file.starts_with("LLVln") {
            "VSCO 2 CE"
        } else if p.file.contains("tinysol") {
            "TinySOL"
        } else {
            "Iowa MIS"
        };
        let _ = writeln!(
            sets,
            "  \"{}\": {{ arcus: \"ref/{}-arcus-lufs23.wav\", rec: \"ref/{}-rec-lufs23.wav\", recName: \"{} grabado ({source})\" }},",
            p.tag,
            p.tag,
            p.tag,
            if p.inst == Instrument::Viola { "viola" } else { "violín" }
        );
    }
    sets.push_str("};\n");
    sets
}

/// Single notes for the study's measuring chain (`tools/arcus/upper_study.py`): each
/// instrument's keys every four semitones, arco at pp, mf and ff (3 s) and plucked at mf, as
/// `<out>/study/<family>/arcus_<tech>_<midi>_<dyn>.wav`.
fn study_notes(out: &Path) {
    for (family, inst) in [("violin", Instrument::Violin), ("viola", Instrument::Viola)] {
        let dir = out.join("study").join(family);
        std::fs::create_dir_all(&dir).expect("study dir");
        let spec = inst.spec();
        let mut key = spec.bottom();
        while key + 3 <= spec.top {
            for (tech, d, vel, len) in [
                ("arco", "pp", 0.22f32, 3.0f64),
                ("arco", "mf", 0.6, 3.0),
                ("arco", "ff", 0.95, 3.0),
                ("pizz", "mf", 0.6, 2.0),
            ] {
                let s = ArcusSettings {
                    instrument: inst,
                    technique: if tech == "pizz" {
                        Technique::Pizzicato
                    } else {
                        Technique::Arco
                    },
                    ..ArcusSettings::default()
                };
                let (l, r) = render(
                    &s,
                    &[(0.05, Ev::On(key, vel)), (0.05 + len, Ev::Off(key))],
                    len + 0.8,
                );
                write(
                    &dir.join(format!("arcus_{tech}_{key}_{d}.wav")),
                    SR as f64,
                    &l,
                    &r,
                    1.0,
                );
            }
            key += 4;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(
        args.get(1)
            .cloned()
            .unwrap_or_else(|| "arcus-upper-listen".into()),
    );
    if args.get(2).is_some_and(|a| a == "study") {
        study_notes(&out);
        println!("study notes written to {}", out.join("study").display());
        return;
    }
    let refs = args.get(2).map(PathBuf::from);
    std::fs::create_dir_all(&out).expect("out dir");
    for (name, inst, note, line) in [
        (
            "violin",
            Instrument::Violin,
            74u8,
            [74u8, 76, 77, 79, 81, 82, 81, 79, 77, 76],
        ),
        (
            "viola",
            Instrument::Viola,
            62u8,
            [62u8, 64, 65, 67, 69, 70, 69, 67, 65, 64],
        ),
    ] {
        let base = ArcusSettings {
            instrument: inst,
            ..ArcusSettings::default()
        };
        let mut jobs: Vec<Job> = Vec::new();
        for (d, vel) in [("pp", 0.22f32), ("mf", 0.6), ("ff", 0.98)] {
            jobs.push((
                format!("{name}-note-{d}"),
                base,
                vec![(0.1, Ev::On(note, vel)), (4.1, Ev::Off(note))],
                5.5,
            ));
        }
        let beats = [2.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0, 4.0];
        let phrase: Vec<(u8, f64)> = line.iter().zip(beats).map(|(k, b)| (*k, b)).collect();
        let (ev, secs) = legato(&phrase, 72.0, 0.55, 0.3, 0.85);
        jobs.push((format!("{name}-phrase-legato-cc1"), base, ev, secs));
        // Détaché: eighth notes re-bowed, the bow alternating by itself.
        let det = ArcusSettings {
            mode: PlayMode::Mono,
            ..base
        };
        let mut ev = Vec::new();
        for (i, k) in line.iter().enumerate() {
            let t = 0.1 + i as f64 * 0.4;
            ev.push((t, Ev::Art(*k, Some(Art::Sustain), None)));
            ev.push((t, Ev::On(*k, 0.6 + 0.04 * (i % 3) as f32)));
            ev.push((t + 0.38, Ev::Off(*k)));
        }
        jobs.push((format!("{name}-detache"), det, ev, 5.5));
        let pizz = ArcusSettings {
            technique: Technique::Pizzicato,
            mode: PlayMode::Poly,
            ..base
        };
        let mut ev = Vec::new();
        for (i, k) in line.iter().take(8).enumerate() {
            let t = 0.1 + i as f64 * 0.6;
            ev.push((t, Ev::On(*k, 0.55 + 0.05 * (i % 3) as f32)));
            ev.push((t + 0.5, Ev::Off(*k)));
        }
        jobs.push((format!("{name}-pizzicato"), pizz, ev, 6.5));
        let trem = ArcusSettings {
            technique: Technique::Tremolo,
            ..base
        };
        jobs.push((
            format!("{name}-tremolo"),
            trem,
            vec![
                (0.1, Ev::On(note, 0.6)),
                (2.6, Ev::Off(note)),
                (2.7, Ev::On(note + 5, 0.8)),
                (5.2, Ev::Off(note + 5)),
            ],
            6.5,
        ));
        for (file, s, ev, secs) in jobs {
            let (l, r) = render(&s, &ev, secs);
            let peak = l.iter().chain(&r).fold(0f32, |m, x| m.max(x.abs()));
            let (_, lufs) = to_lufs23(SR as f64, &l, &r);
            println!(
                "{file:28} peak {:6.1} dBFS  {lufs:6.1} LUFS",
                20.0 * math::log10(peak.max(1e-9) as f64)
            );
            write(&out.join(format!("{file}.wav")), SR as f64, &l, &r, 1.0);
        }
    }
    if let Some(refs) = refs {
        let sets = pairs(&out, &refs);
        std::fs::write(out.join("sets.js.txt"), &sets).expect("sets");
        print!("{sets}");
    }
    println!("written to {}", out.display());
}
