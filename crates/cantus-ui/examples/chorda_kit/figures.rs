//! The comparison's phrases table, its plots, and `write` — the whole report at once.

use super::analyse::{self, NoteM, PhraseM, OCTAVES};
use super::phrases::{self, DYNAMICS, NOTE_HELD};
use super::plot::{self, Plot};
use super::render;
use super::report::{
    colour, distance, f1, mean, measure_notes, notes_csv, oct, summary, Notes, SHAPE_OCTAVES,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

type Phrases = BTreeMap<(String, &'static str), PhraseM>;

/// A plot of one measure by key: file, title, y axis, y range and step, the measure.
type OnePlot = (
    &'static str,
    &'static str,
    &'static str,
    (f64, f64, f64),
    fn(&NoteM) -> f64,
);

fn measure_phrases(out: &Path, names: &[String]) -> Phrases {
    let mut m = BTreeMap::new();
    for ph in phrases::all().into_iter().filter(|p| p.listen) {
        for name in names {
            let path = out.join("wav").join(name).join(format!("{}.wav", ph.id));
            if let Ok(s) = render::read_wav(&path) {
                m.insert((name.clone(), ph.id), analyse::phrase(&s));
            }
        }
    }
    m
}

fn phrase_table(md: &mut String, pm: &Phrases, names: &[String]) {
    let refn = &names[0];
    let _ = writeln!(md, "## Phrases\n\nLoudness (LUFS, integrated), peak to loudness (PLR, dB), spectral centroid, stereo correlation and side under mid; then the long-term spectrum's difference from {refn} per octave (dB, level-free).\n");
    let mut head = String::from("| phrase | source | LUFS | PLR | centroid Hz | corr | side dB |");
    let mut rule = String::from("|---|---|---|---|---|---|---|");
    for (o, _) in SHAPE_OCTAVES {
        let _ = write!(head, " {o} |");
        rule.push_str("---|");
    }
    let _ = writeln!(md, "{head}\n{rule}");
    for ph in phrases::all().into_iter().filter(|p| p.listen) {
        let Some(r) = pm.get(&(refn.clone(), ph.id)) else {
            continue;
        };
        for name in names {
            let Some(m) = pm.get(&(name.clone(), ph.id)) else {
                continue;
            };
            let mut row = format!(
                "| {} | {name} | {:.1} | {:.1} | {:.0} | {:.2} | {:.1} |",
                ph.id, m.lufs, m.plr, m.centroid, m.corr, m.side
            );
            for (_, c) in SHAPE_OCTAVES {
                let _ = write!(row, " {} |", f1(oct(&m.ltas, c) - oct(&r.ltas, c)));
            }
            let _ = writeln!(md, "{row}");
        }
    }
    let _ = writeln!(md);
}

fn by_key(v: &[NoteM], f: &dyn Fn(&NoteM) -> f64) -> Vec<(f64, f64)> {
    v.iter().map(|m| (m.key as f64, f(m))).collect()
}

struct KeyPlot<'a> {
    file: String,
    title: String,
    yname: &'a str,
    y: (f64, f64, f64),
    d: &'static str,
}

fn key_plot(
    out: &Path,
    k: KeyPlot,
    all: &Notes,
    names: &[String],
    f: &dyn Fn(&str, &NoteM) -> f64,
) -> Result<(), String> {
    let mut p = Plot::new(1000, 520, &k.title, (20.0, 109.0, false), (k.y.0, k.y.1));
    p.axes(
        &plot::key_ticks(),
        &plot::lin_ticks(k.y.0, k.y.1, k.y.2),
        "KEY",
        k.yname,
    );
    for (i, n) in names.iter().enumerate() {
        p.line(&by_key(&all[&(n.clone(), k.d)], &|m| f(n, m)), colour(i), 2);
        p.key(n, colour(i));
    }
    p.save(&out.join("plots").join(k.file))
}

fn kp<'a>(
    file: &str,
    title: &str,
    yname: &'a str,
    y: (f64, f64, f64),
    d: &'static str,
) -> KeyPlot<'a> {
    KeyPlot {
        file: file.into(),
        title: title.into(),
        yname,
        y,
        d,
    }
}

fn shape_plots(out: &Path, all: &Notes, names: &[String]) -> Result<(), String> {
    for key in [21u8, 36, 60, 84, 108] {
        for (d, _) in DYNAMICS {
            for (w, title) in [("early", "FIRST 50 MS"), ("mid", "AT 0.5 S")] {
                let name = key_name(key);
                let mut p = Plot::new(
                    1000,
                    520,
                    &format!("{name} {d}: SPECTRUM {title}, THIRD OCTAVES"),
                    (25.0, 20_000.0, true),
                    (-70.0, 0.0),
                );
                p.axes(
                    &plot::hz_ticks(),
                    &plot::lin_ticks(-70.0, 0.0, 10.0),
                    "HZ",
                    "DB OF TOTAL",
                );
                for (i, n) in names.iter().enumerate() {
                    let m = all[&(n.clone(), d)]
                        .iter()
                        .find(|m| m.key == key)
                        .expect("the key");
                    let s = if w == "early" { &m.early } else { &m.mid };
                    let pts: Vec<(f64, f64)> = analyse::thirds()
                        .into_iter()
                        .zip(s.iter().copied())
                        .collect();
                    p.line(&pts, colour(i), 2);
                    p.key(n, colour(i));
                }
                p.save(&out.join("plots").join(format!("shape_{name}_{d}_{w}.png")))?;
            }
        }
    }
    Ok(())
}

fn plots(
    out: &Path,
    all: &Notes,
    names: &[String],
    offset: &BTreeMap<String, f64>,
    pm: &Phrases,
) -> Result<(), String> {
    std::fs::create_dir_all(out.join("plots")).map_err(|e| e.to_string())?;
    for (d, _) in DYNAMICS {
        let t = format!("LEVEL ACROSS THE KEYBOARD, {d} (ALIGNED AT MF)");
        key_plot(
            out,
            kp(
                &format!("level_{d}.png"),
                &t,
                "DB RMS 1 S",
                (-60.0, 0.0, 10.0),
                d,
            ),
            all,
            names,
            &|n, m| m.level + offset[n],
        )?;
        let t = format!("BRIGHTNESS OF THE ATTACK (0-50 MS), {d}");
        key_plot(
            out,
            kp(
                &format!("centroid_early_{d}.png"),
                &t,
                "CENTROID HZ",
                (0.0, 8000.0, 1000.0),
                d,
            ),
            all,
            names,
            &|_, m| m.centroid_early,
        )?;
        let t = format!("BRIGHTNESS AT 0.5 S, {d}");
        key_plot(
            out,
            kp(
                &format!("centroid_mid_{d}.png"),
                &t,
                "CENTROID HZ",
                (0.0, 6000.0, 1000.0),
                d,
            ),
            all,
            names,
            &|_, m| m.centroid_mid,
        )?;
    }
    let d = "mf";
    let one: [OnePlot; 9] = [
        (
            "thump_mf.png",
            "THE KNOCK: SHARE BELOW 0.7 F0 IN 60 MS, MF",
            "DB",
            (-60.0, 0.0, 10.0),
            |m| m.thump,
        ),
        (
            "noise_mf.png",
            "STRIKE NOISE: FLATNESS 1-8 KHZ IN 30 MS, MF",
            "FLATNESS",
            (0.0, 0.6, 0.1),
            |m| m.noise,
        ),
        (
            "rise_mf.png",
            "RISE TIME 10-90 %, MF",
            "MS",
            (0.0, 40.0, 5.0),
            |m| m.rise_ms,
        ),
        (
            "side_mf.png",
            "STEREO: SIDE UNDER MID, FIRST SECOND, MF",
            "DB",
            (-40.0, 0.0, 5.0),
            |m| m.side,
        ),
        (
            "balance_mf.png",
            "STEREO: LEFT OVER RIGHT, MF",
            "DB",
            (-12.0, 12.0, 3.0),
            |m| m.balance,
        ),
        (
            "release_mf.png",
            "RELEASE: MS TO 30 DB DOWN AFTER KEY UP, MF",
            "MS",
            (0.0, 2400.0, 300.0),
            |m| m.release_ms,
        ),
        (
            "release_hf_mf.png",
            "RELEASE NOISE: 2 KHZ+ AFTER/BEFORE KEY UP, MF",
            "DB",
            (-40.0, 10.0, 5.0),
            |m| m.release_hf,
        ),
        (
            "decay_early_mf.png",
            "DECAY 0.1-0.6 S, MF",
            "DB/S",
            (0.0, 80.0, 10.0),
            |m| m.decay_early,
        ),
        (
            "decay_late_mf.png",
            "DECAY 1.5 S - RELEASE, MF",
            "DB/S",
            (0.0, 40.0, 5.0),
            |m| m.decay_late,
        ),
    ];
    for (file, title, y, r, f) in one {
        key_plot(out, kp(file, title, y, r, d), all, names, &|_, m| f(m))?;
    }
    for (b, o) in OCTAVES.iter().enumerate().take(6) {
        let t = format!("DECAY IN THE {o:.0} HZ OCTAVE, MF");
        key_plot(
            out,
            kp(
                &format!("decay_{o:.0}_mf.png"),
                &t,
                "DB/S",
                (0.0, 80.0, 10.0),
                d,
            ),
            all,
            names,
            &|_, m| m.decay[b],
        )?;
    }
    let mut p = Plot::new(
        1000,
        520,
        "DYNAMIC SPAN: FF OVER PP",
        (20.0, 109.0, false),
        (0.0, 50.0),
    );
    p.axes(
        &plot::key_ticks(),
        &plot::lin_ticks(0.0, 50.0, 10.0),
        "KEY",
        "DB",
    );
    for (i, n) in names.iter().enumerate() {
        let pp = &all[&(n.clone(), "pp")];
        let ff = &all[&(n.clone(), "ff")];
        let pts: Vec<(f64, f64)> = pp
            .iter()
            .zip(ff)
            .map(|(a, b)| (a.key as f64, b.level - a.level))
            .collect();
        p.line(&pts, colour(i), 2);
        p.key(n, colour(i));
    }
    p.save(&out.join("plots").join("span.png"))?;
    shape_plots(out, all, names)?;
    for ph in phrases::all().into_iter().filter(|p| p.listen) {
        let mut p = Plot::new(
            1000,
            520,
            &format!("{}: LONG-TERM SPECTRUM", ph.id),
            (25.0, 20_000.0, true),
            (-70.0, 0.0),
        );
        p.axes(
            &plot::hz_ticks(),
            &plot::lin_ticks(-70.0, 0.0, 10.0),
            "HZ",
            "DB OF TOTAL",
        );
        for (i, n) in names.iter().enumerate() {
            if let Some(m) = pm.get(&(n.clone(), ph.id)) {
                let pts: Vec<(f64, f64)> = analyse::thirds()
                    .into_iter()
                    .zip(m.ltas.iter().copied())
                    .collect();
                p.line(&pts, colour(i), 2);
                p.key(n, colour(i));
            }
        }
        p.save(&out.join("plots").join(format!("ltas_{}.png", ph.id)))?;
    }
    Ok(())
}

/// A key's name: A0, C4, …
pub fn key_name(key: u8) -> String {
    const N: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", N[key as usize % 12], key as i32 / 12 - 1)
}

/// Measure everything under `out/wav` for `names` (the first the reference) and write the
/// report, the tables and the plots.
pub fn write(out: &Path, names: &[String]) -> Result<(), String> {
    let all = measure_notes(out, names)?;
    let refn = &names[0];
    let ref_mf = mean(all[&(refn.clone(), "mf")].iter().map(|m| m.level));
    let offset: BTreeMap<String, f64> = names
        .iter()
        .map(|n| {
            (
                n.clone(),
                ref_mf - mean(all[&(n.clone(), "mf")].iter().map(|m| m.level)),
            )
        })
        .collect();
    notes_csv(out, &all, &offset)?;
    let pm = measure_phrases(out, names);
    let mut csv = String::from("phrase,source,lufs,peak_db,plr,centroid,corr,side_db\n");
    for ((n, id), m) in &pm {
        let _ = writeln!(
            csv,
            "{id},{n},{:.2},{:.2},{:.2},{:.0},{:.3},{:.2}",
            m.lufs, m.peak, m.plr, m.centroid, m.corr, m.side
        );
    }
    std::fs::write(out.join("phrases.csv"), csv).map_err(|e| e.to_string())?;
    let mut md = String::from("# Chorda against the reference — measured\n\n");
    let _ = writeln!(md, "Rendered by `chorda_compare` through the real engine at 48 kHz; the reference is **{refn}**. Single notes: A0 … C8 every minor third at pp (30), mf (75), ff (115), held {NOTE_HELD} s.\n");
    let _ = writeln!(md, "**Distance** — RMS of the third-octave shape differences against {refn}, every note, 63 Hz – 16 kHz (dB; lower is closer): first 50 ms / at 0.5 s\n");
    for n in &names[1..] {
        let (e, m) = distance(&all, n, refn);
        let _ = writeln!(md, "- {n}: {e:.2} / {m:.2}");
    }
    let _ = writeln!(
        md,
        "
**Score** (lower is closer): total = shapes + decay + knock + brightness + evenness + stereo
"
    );
    for n in &names[1..] {
        let s = super::report::score(&all, n, refn, &offset);
        let line = format!(
            "SCORE {n} {:.2} = {:.2} + {:.2} + {:.2} + {:.2} + {:.2} + {:.2}",
            s[6], s[0], s[1], s[2], s[3], s[4], s[5]
        );
        println!("{line}");
        let _ = writeln!(md, "- {line}");
    }
    let _ = writeln!(md);
    summary(&mut md, &all, names, &offset);
    phrase_table(&mut md, &pm, names);
    plots(out, &all, names, &offset, &pm)?;
    std::fs::write(out.join("report.md"), md).map_err(|e| e.to_string())?;
    eprintln!("wrote {}", out.join("report.md").display());
    Ok(())
}
