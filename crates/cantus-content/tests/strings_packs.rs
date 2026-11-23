//! r42:strings-rec — the recorded strings' packs measured against their sources (ADR 0158):
//! every zone of every strings pack in the bundled manifest, played by Exemplar at its root
//! with the tuning the manifest gives it, is **in tune within a cent** (measured by McLeod's
//! NSDF — not the YIN that set the tuning), and every loop the manifest keeps is **click-free**
//! as Exemplar plays it (the seam no louder above 5 kHz than the loop's own playing, +3 dB).
//!
//! The samples are not in the repository: this test is ignored, and run on the sources
//! `tools/content/strings/fetch_sources.sh` fetched —
//! `CANTUS_STRINGS_SRC=<dir> cargo test --release -p cantus-content --test strings_packs -- --ignored --nocapture`.

use cantus_content::manifest::{Manifest, Source};
use cantus_content::strings_analysis as an;
use cantus_content::wav;
use cantus_dsp::exemplar::{resample, Exemplar, Globals, Region, Source as Src, ZoneParams};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

/// The bytes of every file a pack's zones name, from its local source.
fn files(src: &std::path::Path, pack: &cantus_content::manifest::Pack) -> HashMap<String, Vec<u8>> {
    let wanted: std::collections::HashSet<&str> = pack
        .strings
        .as_ref()
        .unwrap()
        .instruments
        .iter()
        .flat_map(|i| i.zones.iter().map(|z| z.file.as_str()))
        .collect();
    let mut out = HashMap::new();
    match &pack.source {
        Source::Files(_) => {
            for f in &wanted {
                out.insert(
                    f.to_string(),
                    std::fs::read(src.join("vsco").join(f)).unwrap(),
                );
            }
        }
        Source::Zip(z) => {
            let local = [
                "bigcat.zip",
                "smolken.zip",
                "meatbass.zip",
                "sneakybass.zip",
            ]
            .into_iter()
            .find(|l| std::fs::metadata(src.join(l)).is_ok_and(|m| m.len() == z.size))
            .expect("a local copy of the pack's zip");
            let mut a =
                zip::ZipArchive::new(std::fs::File::open(src.join(local)).unwrap()).unwrap();
            for i in 0..a.len() {
                let mut e = a.by_index(i).unwrap();
                let n = e.name().map(|n| n.to_string()).unwrap_or_default();
                if let Some(rel) = n.strip_prefix(z.strip.as_str()) {
                    if wanted.contains(rel) {
                        let mut b = Vec::new();
                        e.read_to_end(&mut b).unwrap();
                        out.insert(rel.to_string(), b);
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// Exemplar at the source's own rate playing `x` (mono) at its root with `tune` cents, looped
/// at `lp` when given, for `secs`: what it plays.
fn play(x: &[f32], rate: f32, tune: f32, lp: Option<[i64; 2]>, xf: i64, secs: f32) -> Vec<f32> {
    let mut levels: [Option<Arc<[f32]>>; resample::MAX_LEVELS] = Default::default();
    levels[0] = Some(Arc::from(x.to_vec()));
    let src = Src {
        levels,
        channels: 1,
        rate,
        frames: x.len(),
        tune_ct: 0.0,
        stream: None,
    };
    let mut e = Exemplar::new(rate);
    e.set_globals(&Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 50.0],
        vel_amp: 0.0,
        ..Globals::default()
    });
    let region = match lp {
        Some([ls, le]) => Region {
            end: x.len() as i64,
            looping: true,
            ls,
            le,
            xf,
            ..Region::default()
        },
        None => Region {
            end: x.len() as i64,
            ..Region::default()
        },
    };
    e.put_zone(
        0,
        &ZoneParams {
            root: 60,
            tune_ct: tune,
            region,
            ..ZoneParams::default()
        },
        &src,
    );
    e.note_on(60, 1.0, 1);
    let n = (rate * secs) as usize;
    let mut out = Vec::with_capacity(n);
    let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
    while out.len() < n {
        l.fill(0.0);
        r.fill(0.0);
        e.process(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out
}

#[test]
#[ignore = "needs the packs' sources: CANTUS_STRINGS_SRC"]
fn every_zone_is_in_tune_within_a_cent_and_every_loop_is_click_free() {
    let src = PathBuf::from(std::env::var("CANTUS_STRINGS_SRC").expect("CANTUS_STRINGS_SRC"));
    let m = Manifest::bundled();
    let (mut tuned, mut off, mut unmeasured, mut loops, mut clicks) =
        (0, Vec::new(), 0, 0, Vec::new());
    for pack in m.packs.iter().filter(|p| p.strings.is_some()) {
        let bytes = files(&src, pack);
        let mut seen = std::collections::HashSet::new();
        for inst in &pack.strings.as_ref().unwrap().instruments {
            for z in &inst.zones {
                if !seen.insert(z.file.clone()) {
                    continue;
                }
                let a = wav::parse(&bytes[&z.file]).unwrap();
                let rate = a.rate as f32;
                let x = an::mono(&a.samples, a.channels);
                let held = matches!(z.art.as_str(), "sustain" | "tremolo");
                let secs = if held {
                    (x.len() as f32 / rate).min(6.0)
                } else {
                    (x.len() as f32 / rate).min(0.7)
                };
                let xf = (z.xf_ms / 1000.0 * rate) as i64;
                let out = play(&x, rate, z.tune, z.looped, xf, secs);
                // In tune: over the steady part of a held note, the first 0.6 s of a short one.
                let (a0, b0) = an::steady_span(&out, rate)
                    .filter(|_| held)
                    .unwrap_or(((rate * 0.03) as usize, out.len()));
                let hint = an::hz_of_key(z.root as f64);
                match an::nsdf_cents(&out, rate, a0, b0, hint) {
                    Some(c) if z.tune != 0.0 || held => {
                        let err = c - z.root as f64 * 100.0;
                        if err.abs() <= 1.0 {
                            tuned += 1;
                        } else {
                            off.push((pack.id.clone(), z.file.clone(), err));
                        }
                    }
                    _ => unmeasured += 1,
                }
                if let Some([ls, le]) = z.looped {
                    loops += 1;
                    let len = (le - ls + 1) as usize;
                    let need = (le as usize + 1 + 3 * len + 512) as f32 / rate;
                    let o = play(&x, rate, 0.0, z.looped, xf, need);
                    let seams: Vec<usize> = (0..3)
                        .map(|k| le as usize + 1 + k * len - ls as usize)
                        .collect();
                    let ex = an::seam_excess_db(&o[ls as usize..], rate, &seams);
                    if ex > 3.0 {
                        clicks.push((pack.id.clone(), z.file.clone(), ex));
                    }
                }
            }
        }
    }
    let total = tuned + off.len();
    println!("in tune within a cent: {tuned} of {total}; not measurable: {unmeasured}; loops: {loops}, clicking: {}", clicks.len());
    for (p, f, e) in &off {
        println!("  off  {p} {f}: {e:+.2} ct");
    }
    for (p, f, e) in &clicks {
        println!("  click {p} {f}: {e:.1} dB");
    }
    assert!(clicks.is_empty(), "loops that click: {clicks:?}");
    assert!(
        off.is_empty(),
        "{} zones out of tune by more than a cent",
        off.len()
    );
}
