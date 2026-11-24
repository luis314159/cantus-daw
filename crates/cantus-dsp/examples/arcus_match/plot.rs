//! One held-out note's plot (SVG): its harmonics 1–20 and its third-octave spectrum, the
//! references' spread as a band, Arcus before (model 1) and after (model 2) as lines.

use super::features::{band_hz, Features, BANDS};
use super::refs::{Inst, Note};
use super::Key;
use std::fmt::Write as _;
use std::path::Path;

const W: f64 = 900.0;
const H: f64 = 300.0;

fn panel(
    svg: &mut String,
    y0: f64,
    title: &str,
    n: usize,
    band: &[Option<(f64, f64)>],
    lines: [(&[Option<f64>], &str); 2],
) {
    let (x0, pw, ph) = (60.0, W - 90.0, H - 70.0);
    let (lo, hi) = (-70.0, 0.0);
    let x = |i: usize| x0 + pw * i as f64 / (n - 1).max(1) as f64;
    let y = |v: f64| y0 + 30.0 + ph * (hi - v.clamp(lo, hi)) / (hi - lo);
    let _ = writeln!(
        svg,
        "<text x='{x0}' y='{}' font-size='14' fill='#222'>{title}</text>",
        y0 + 18.0
    );
    for d in (0..=70).step_by(10) {
        let yy = y(-(d as f64));
        let _ = writeln!(
            svg,
            "<line x1='{x0}' x2='{}' y1='{yy}' y2='{yy}' stroke='#ddd'/><text x='20' y='{}' font-size='10' fill='#666'>-{d}</text>",
            x0 + pw,
            yy + 3.0
        );
    }
    // The references' band.
    let mut top = String::new();
    let mut bot = Vec::new();
    for (i, b) in band.iter().enumerate() {
        if let Some((l, h)) = b {
            let _ = write!(top, "{:.1},{:.1} ", x(i), y(*h));
            bot.push(format!("{:.1},{:.1}", x(i), y(*l)));
        }
    }
    bot.reverse();
    let _ = writeln!(
        svg,
        "<polygon points='{top}{}' fill='#9bc4e2' fill-opacity='0.6' stroke='none'/>",
        bot.join(" ")
    );
    for (vals, colour) in lines {
        let pts: Vec<String> = vals
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| format!("{:.1},{:.1}", x(i), y(v))))
            .collect();
        let _ = writeln!(
            svg,
            "<polyline points='{}' fill='none' stroke='{colour}' stroke-width='2'/>",
            pts.join(" ")
        );
    }
}

pub fn note(out: &Path, k: Key, refs: &[&Note], before: &Features, after: &Features) {
    let (inst, pizz, key, _) = k;
    let spread = |get: &dyn Fn(&Features) -> Option<f64>| {
        let v: Vec<f64> = refs.iter().filter_map(|n| get(&n.f)).collect();
        if v.is_empty() {
            None
        } else {
            Some((
                v.iter().cloned().fold(f64::INFINITY, f64::min),
                v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            ))
        }
    };
    let hb: Vec<Option<(f64, f64)>> = (0..20).map(|i| spread(&|f| f.harm[i])).collect();
    let lb: Vec<Option<(f64, f64)>> = (0..BANDS).map(|i| spread(&|f| Some(f.ltas[i]))).collect();
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{W}' height='{}' font-family='sans-serif'>\n<rect width='100%' height='100%' fill='white'/>\n",
        2.0 * H + 30.0
    );
    let name = format!(
        "{} {} key {key} mf — the references' spread (blue band, {} notes), Arcus before (grey) and after (red)",
        if inst == Inst::Cello { "cello" } else { "double bass" },
        if pizz { "pizzicato" } else { "arco" },
        refs.len()
    );
    let _ = writeln!(
        svg,
        "<text x='20' y='20' font-size='13' fill='#000'>{name}</text>"
    );
    let bh: Vec<Option<f64>> = before.harm.iter().take(20).cloned().collect();
    let ah: Vec<Option<f64>> = after.harm.iter().take(20).cloned().collect();
    panel(
        &mut svg,
        30.0,
        "harmonics 1–20, dB of their sum",
        20,
        &hb,
        [(&bh, "#888"), (&ah, "#c22")],
    );
    let bl: Vec<Option<f64>> = before.ltas.iter().map(|v| Some(*v)).collect();
    let al: Vec<Option<f64>> = after.ltas.iter().map(|v| Some(*v)).collect();
    panel(
        &mut svg,
        30.0 + H,
        &format!(
            "third-octave spectrum {:.0} Hz – {:.0} kHz, dB of the whole",
            band_hz(0),
            band_hz(BANDS - 1) / 1000.0
        ),
        BANDS,
        &lb,
        [(&bl, "#888"), (&al, "#c22")],
    );
    svg.push_str("</svg>\n");
    let file = format!(
        "{}-{}-{key}.svg",
        if inst == Inst::Cello { "cello" } else { "bass" },
        if pizz { "pizz" } else { "arco" }
    );
    std::fs::write(out.join(file), svg).expect("the plot");
}
