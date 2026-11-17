//! The report, as Markdown.

use super::cpu::Cpu;
use cantus_dsp::aequalis::fit::{fit, FitOptions};
use cantus_dsp::comparatio::match_eq::{suggest, MatchOptions};
use cantus_dsp::comparatio::measure::{HIST_BINS, HIST_LO};
use cantus_dsp::comparatio::stereo::BANDS;
use cantus_dsp::comparatio::tonal::compare;
use cantus_dsp::comparatio::WholeAnalysis;
use std::fmt::Write;

fn num(v: Option<f64>, unit: &str) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{x:.2} {unit}"),
        _ => "—".to_string(),
    }
}

fn diff(a: Option<f64>, b: Option<f64>, unit: &str) -> String {
    match (a, b) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => format!("{:+.2} {unit}", x - y),
        _ => "—".to_string(),
    }
}

pub fn report(
    sr: f32,
    len: usize,
    a: &WholeAnalysis,
    b: &WholeAnalysis,
    switched: (f64, f64, f32),
    cpu: &Cpu,
) -> String {
    let mut s = String::new();
    let w = &mut s;
    let _ = writeln!(w, "# Comparatio — the offline report (DSP phase)\n");
    let _ = writeln!(
        w,
        "Written by `cargo run -p cantus-dsp --release --example comparatio_report`. The DI \
         guitar (*Strummed Guitar Chords* by Carbonthief, CC0) plays the owner's progression \
         Am–F–C–G at 90 BPM, twice ({:.1} s at {} Hz, the WAVs' own rate), processed two ways:\n",
        len as f32 / sr,
        sr
    );
    let _ = writeln!(
        w,
        "- **The mix**: Fragor's green overdrive (drive 0.35), Aequalis's low cut at 90 Hz and \
         +2 dB at 2.5 kHz, nearly mono (a 3 ms delay at 20 % in the left).\n\
         - **The reference**: Fragor's triangle muff, +3 dB under 150 Hz, a high cut at 7 kHz, \
         a wide Haas image (the right 14 ms late), pushed into a soft clip (louder, less crest).\n"
    );
    let (ra, rb) = (&a.readings, &b.readings);
    let _ = writeln!(w, "## Loudness and dynamics\n");
    let _ = writeln!(w, "| | The mix | The reference | Mix − reference |");
    let _ = writeln!(w, "|---|---|---|---|");
    let f = |x: f64| Some(x).filter(|v| v.is_finite());
    let rows: [(&str, Option<f64>, Option<f64>, &str); 9] = [
        ("Integrated loudness", ra.integrated, rb.integrated, "LUFS"),
        (
            "Short-term maximum",
            f(ra.max_short_term),
            f(rb.max_short_term),
            "LUFS",
        ),
        (
            "Momentary maximum",
            f(ra.max_momentary),
            f(rb.max_momentary),
            "LUFS",
        ),
        ("Loudness range", ra.range, rb.range, "LU"),
        ("True peak", f(ra.true_peak_db), f(rb.true_peak_db), "dBTP"),
        (
            "Sample peak",
            f(ra.sample_peak_db),
            f(rb.sample_peak_db),
            "dBFS",
        ),
        ("Crest factor", ra.crest_db, rb.crest_db, "dB"),
        ("Peak to loudness (PLR)", ra.plr, rb.plr, "LU"),
        (
            "Peak to short-term loudness (PSR), mean",
            ra.psr_mean,
            rb.psr_mean,
            "LU",
        ),
    ];
    for (name, x, y, unit) in rows {
        let unit_d = if unit == "LUFS" || unit == "dBTP" || unit == "dBFS" {
            "dB"
        } else {
            unit
        };
        let _ = writeln!(
            w,
            "| {name} | {} | {} | {} |",
            num(x, unit),
            num(y, unit),
            diff(x, y, unit_d)
        );
    }
    let _ = writeln!(w, "\n## Stereo\n");
    let _ = writeln!(
        w,
        "| Band | Mix correlation | Mix width | Reference correlation | Reference width |"
    );
    let _ = writeln!(w, "|---|---|---|---|---|");
    let names = ["Whole", "Lows (under 200 Hz)", "Mids", "Highs (over 4 kHz)"];
    for (k, name) in names.iter().enumerate().take(BANDS) {
        let (x, y) = (ra.stereo[k], rb.stereo[k]);
        let o = |v: Option<f64>| v.map_or("—".into(), |v| format!("{v:+.3}"));
        let p = |v: Option<f64>| v.map_or("—".into(), |v| format!("{v:.3}"));
        let _ = writeln!(
            w,
            "| {name} | {} | {} | {} | {} |",
            o(x.correlation),
            p(x.width),
            o(y.correlation),
            p(y.width)
        );
    }
    let _ = writeln!(
        w,
        "\nWidth: 0 mono, 1 two unrelated channels, 2 out of phase.\n"
    );
    let _ = writeln!(w, "## The short-term loudness, every 100 ms (counts)\n");
    let _ = writeln!(w, "| LUFS | The mix | The reference |");
    let _ = writeln!(w, "|---|---|---|");
    let bar = |n: u32| "█".repeat((n as usize).div_ceil(4));
    for i in (0..HIST_BINS).rev() {
        let (x, y) = (a.histogram[i], b.histogram[i]);
        if x + y == 0 {
            continue;
        }
        let lo = HIST_LO + i as f64;
        let _ = writeln!(
            w,
            "| {lo:.0} … {:.0} | {x} {} | {y} {} |",
            lo + 1.0,
            bar(x),
            bar(y)
        );
    }
    let d = compare(&a.curve, &b.curve);
    let _ = writeln!(
        w,
        "\n## Tonal balance (third octaves, each curve relative to its own loudness)\n"
    );
    let _ = writeln!(
        w,
        "| Band | The mix | The reference | Mix − reference | Uncertainty | Confidence |"
    );
    let _ = writeln!(w, "|---|---|---|---|---|---|");
    for k in 0..d.bands.count {
        let hz = d.bands.centre[k];
        let label = if hz >= 1_000.0 {
            format!("{:.1} kHz", hz / 1_000.0)
        } else {
            format!("{hz:.0} Hz")
        };
        let _ = writeln!(
            w,
            "| {label} | {:.1} dB | {:.1} dB | {:+.1} dB | {:.2} dB | {:.2} |",
            a.curve.db[k], b.curve.db[k], d.db[k], d.uncertainty[k], d.confidence[k]
        );
    }
    let opts = MatchOptions::default();
    let m = suggest(&d, &opts);
    let _ = writeln!(
        w,
        "\n## The match-EQ suggestion\n\nHalf the way (amount {}), limited to ±{} dB, smoothed \
         over {} octave, {}–{} Hz; output gain {:+.2} dB to keep the loudness.\n",
        opts.amount, opts.max_db, opts.smooth_oct, opts.lo_hz, opts.hi_hz, m.makeup_db
    );
    let pts: Vec<String> = m
        .points()
        .iter()
        .filter(|(hz, _)| *hz >= 25.0 && *hz <= 20_000.0)
        .map(|(hz, db)| format!("{hz:.0} Hz {db:+.1}"))
        .collect();
    let _ = writeln!(w, "{}\n", pts.join(" · "));
    let fitted = fit(m.points(), &FitOptions::default(), sr);
    let _ = writeln!(
        w,
        "Aequalis's fit draws it in {} bands (rms {:.2} dB, worst {:.2} dB from the curve):\n",
        fitted.bands.len(),
        fitted.rms_db,
        fitted.max_db
    );
    for bd in &fitted.bands {
        let _ = writeln!(
            w,
            "- {:?} at {:.0} Hz, {:+.1} dB, Q {:.2}",
            bd.shape, bd.freq, bd.gain_db, bd.q
        );
    }
    let (heard, mix, gain) = switched;
    let _ = writeln!(
        w,
        "\n## The switch, on the unit's own output\n\nThe unit played the mix, then the \
         reference from 10 s (integrated match, locked to the song). Over the second half the \
         output read **{heard:.2} LUFS** against the mix's **{mix:.2} LUFS** over the same \
         stretch ({:+.2} LU); the reference's gain was {gain:+.2} dB.\n",
        heard - mix
    );
    let _ = writeln!(
        w,
        "## CPU at 48 kHz (release, blocks of 512, best of three, one core)\n"
    );
    let _ = writeln!(w, "| Case | One core |");
    let _ = writeln!(w, "|---|---|");
    for (name, pct) in &cpu.cases {
        let _ = writeln!(w, "| {name} | {pct:.3} % |");
    }
    let _ = writeln!(
        w,
        "| the tonal worker, both sources, sixth octaves (off the audio thread) | {:.3} % |\n\n\
         The tonal worker's longest single call (a frame of each source): {:.2} ms.",
        cpu.tonal, cpu.tonal_worst_ms
    );
    s
}
