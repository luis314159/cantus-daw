//! r42:strings-rec — which pitch measure a zone's tuning should be set by (ADR 0158): over
//! what a held zone plays (Exemplar through three turns of its loop, or its steady part), the
//! partials' fit on 1, 2, 3, 4 and 8 harmonics and YIN, each against the held-out NSDF near
//! the root. Prints, per measure, the share of zones the two read within a cent and within
//! five, and the median difference — a string's stiffness stretches its upper partials sharp,
//! so a fit on many of them reads above the fundamental the other measures hear.
//!
//!   CANTUS_STRINGS_SRC=<dir> cargo run --release -p cantus-content --example strings_tune_probe [step]

use cantus_content::manifest::{Manifest, Source};
use cantus_content::strings_analysis as an;
use cantus_content::wav;
use std::io::Read;

fn main() {
    let step: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let src =
        std::path::PathBuf::from(std::env::var("CANTUS_STRINGS_SRC").expect("CANTUS_STRINGS_SRC"));
    let m = Manifest::bundled();
    let names = [
        "fit8",
        "yin",
        "nsdf16",
        "fit8 sus",
        "yin sus",
        "fit8 trem",
        "yin trem",
    ];
    let mut diffs: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
    let mut k = 0usize;
    for pack in m.packs.iter().filter(|p| p.strings.is_some()) {
        let mut zip = match &pack.source {
            Source::Zip(z) => [
                "bigcat.zip",
                "smolken.zip",
                "meatbass.zip",
                "sneakybass.zip",
            ]
            .into_iter()
            .find(|l| std::fs::metadata(src.join(l)).is_ok_and(|m| m.len() == z.size))
            .map(|l| {
                (
                    zip::ZipArchive::new(std::fs::File::open(src.join(l)).unwrap()).unwrap(),
                    z.strip.clone(),
                )
            }),
            _ => None,
        };
        let mut seen = std::collections::HashSet::new();
        for inst in &pack.strings.as_ref().unwrap().instruments {
            for z in &inst.zones {
                if !matches!(z.art.as_str(), "sustain" | "tremolo") || !seen.insert(z.file.clone())
                {
                    continue;
                }
                k += 1;
                if !k.is_multiple_of(step) {
                    continue;
                }
                let bytes = match zip.as_mut() {
                    Some((a, strip)) => {
                        let mut e = a.by_name(&format!("{strip}{}", z.file)).unwrap();
                        let mut b = Vec::new();
                        e.read_to_end(&mut b).unwrap();
                        b
                    }
                    None => std::fs::read(src.join("vsco").join(&z.file)).unwrap(),
                };
                let a = wav::parse(&bytes).unwrap();
                let rate = a.rate as f32;
                let x = an::mono(&a.samples, a.channels);
                let xf = (z.xf_ms / 1000.0 * rate) as i64;
                let (y, s0, s1) = an::held_play(&x, rate, z.looped, xf);
                let s1 = s1.min(y.len());
                let root = an::hz_of_key(z.root as f64);
                let Some(check) = an::nsdf_near_cents(&y, rate, s0, s1, root) else {
                    continue;
                };
                let fit = an::harmonic_cents_k(&y, rate, s0, s1, root, 8);
                let yin = an::yin_cents(&y, rate, s0, s1, root);
                let long = an::nsdf_near_cents_w(&y, rate, s0, s1, root, 16.0, 8_192);
                let sus = z.art == "sustain";
                let got = [
                    fit,
                    yin,
                    long,
                    fit.filter(|_| sus),
                    yin.filter(|_| sus),
                    fit.filter(|_| !sus),
                    yin.filter(|_| !sus),
                ];
                for (d, g) in diffs.iter_mut().zip(got) {
                    if let Some(g) = g {
                        d.push(g - check);
                    }
                }
            }
        }
    }
    for (n, d) in names.iter().zip(&diffs) {
        let mut a: Vec<f64> = d.iter().map(|x| x.abs()).collect();
        a.sort_by(|x, y| x.total_cmp(y));
        let within =
            |c: f64| 100.0 * a.iter().filter(|x| **x <= c).count() as f64 / a.len().max(1) as f64;
        let mut s = d.clone();
        s.sort_by(|x, y| x.total_cmp(y));
        println!(
            "{n}: {} zones, {:.1} % within a cent, {:.1} % within five, median difference {:+.2} ct",
            a.len(),
            within(1.0),
            within(5.0),
            s.get(s.len() / 2).copied().unwrap_or(0.0)
        );
    }
}
