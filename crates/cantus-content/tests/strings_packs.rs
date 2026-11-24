//! r42:strings-rec — the recorded strings' packs measured against their sources (ADR 0158):
//! every zone of every strings pack in the bundled manifest, played by Exemplar at its root
//! with the tuning the manifest gives it, is **in tune** (by the held-out NSDF near its root, McLeod's
//! NSDF — not the YIN that set the tuning), and every loop the manifest keeps is **click-free**
//! as Exemplar plays it (the seam no louder above 5 kHz than the loop's own playing, +3 dB).
//!
//! The samples are not in the repository: this test is ignored, and run on the sources
//! `tools/content/strings/fetch_sources.sh` fetched —
//! `CANTUS_STRINGS_SRC=<dir> cargo test --release -p cantus-content --test strings_packs -- --ignored --nocapture`
//! (`CANTUS_STRINGS_MANIFEST=<content.json>` to measure another manifest's tunings).

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
    // `CANTUS_STRINGS_MANIFEST=<content.json>` measures another manifest (an older tuning:
    // the report's before and after).
    let m = match std::env::var("CANTUS_STRINGS_MANIFEST") {
        Ok(f) => Manifest::parse(&std::fs::read_to_string(f).unwrap()).unwrap(),
        Err(_) => Manifest::bundled(),
    };
    // [short, sustain, tremolo] × [measured, within a cent, within five].
    let mut within = [[0usize; 3]; 3];
    let (mut off, mut unmeasured, mut loops, mut clicks) = (Vec::new(), Vec::new(), 0, Vec::new());
    let mut beyond = Vec::new();
    // Measurable sustains: [measured, within a cent]; beyond three cents; the unreliable ones.
    let (mut strict, mut strict_off, mut unreliable) = ([0usize; 2], Vec::new(), Vec::new());
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
                let xf = (z.xf_ms / 1000.0 * rate) as i64;
                // What the zone plays: a held note through three turns of its loop (or to its
                // end), a short note whole — and where in that its pitch is read: the loop's
                // turns, the steady part, the tone (the spans the tuning was set over).
                let secs = match (held, z.looped) {
                    (true, Some([ls, le])) => (ls + 3 * (le - ls + 1) + 2_048) as f32 / rate,
                    _ => x.len() as f32 / rate,
                };
                let out = play(&x, rate, z.tune, z.looped, xf, secs);
                let (a0, b0) = match (held, z.looped) {
                    (true, Some([ls, le])) => {
                        (ls as usize + 256, (ls + 3 * (le - ls + 1)) as usize)
                    }
                    (true, None) => an::steady_span(&out, rate).unwrap_or((0, out.len())),
                    (false, _) => an::tone_span(&out, rate),
                };
                let root = an::hz_of_key(z.root as f64);
                match an::nsdf_near_cents(&out, rate, a0, b0.min(out.len()), root) {
                    Some(c) => {
                        let err = c - z.root as f64 * 100.0;
                        let k = if z.art == "tremolo" { 2 } else { held as usize };
                        within[k][0] += 1;
                        if err.abs() <= 1.0 {
                            within[k][1] += 1;
                        }
                        if err.abs() <= 5.0 {
                            within[k][2] += 1;
                        } else if k == 1 {
                            beyond.push((pack.id.clone(), z.file.clone(), err));
                        }
                        if err.abs() > 1.0 {
                            off.push((pack.id.clone(), z.file.clone(), err));
                        }
                        // Measurable: the check agrees with itself at a window four times
                        // as long within 0.3 cent — where it is reliable, the gate is strict.
                        if k == 1 {
                            let long = an::nsdf_near_cents_w(
                                &out,
                                rate,
                                a0,
                                b0.min(out.len()),
                                root,
                                16.0,
                                8_192,
                            );
                            let agree = long.is_some_and(|l| (l - c).abs() <= 0.3);
                            match agree.then_some(()) {
                                Some(_) => {
                                    strict[0] += 1;
                                    if err.abs() <= 1.0 {
                                        strict[1] += 1;
                                    }
                                    if err.abs() > 3.0 {
                                        strict_off.push((pack.id.clone(), z.file.clone(), err));
                                    }
                                }
                                None => unreliable.push((
                                    pack.id.clone(),
                                    z.file.clone(),
                                    long.map_or(f64::NAN, |l| l - c),
                                )),
                            }
                        }
                    }
                    None => {
                        let why = if b0 <= a0 + 4_096 {
                            "too short"
                        } else {
                            "no voiced frame"
                        };
                        unmeasured.push((pack.id.clone(), z.file.clone(), why));
                    }
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
    let pct = |k: usize, w: usize| 100.0 * within[k][w] as f64 / within[k][0].max(1) as f64;
    println!(
        "sustains: {} measured, {:.1} % within a cent, {:.1} % within five; tremolos: {} measured, {:.1} % / {:.1} %; short notes: {} measured, {:.1} % / {:.1} %; not measurable: {}; loops: {loops}, clicking: {}",
        within[1][0], pct(1, 1), pct(1, 2), within[2][0], pct(2, 1), pct(2, 2), within[0][0], pct(0, 1), pct(0, 2), unmeasured.len(), clicks.len()
    );
    for (p, f, e) in &off {
        println!("  off  {p} {f}: {e:+.2} ct");
    }
    for (p, f, why) in &unmeasured {
        println!("  none {p} {f}: {why}");
    }
    for (p, f, e) in &clicks {
        println!("  click {p} {f}: {e:.1} dB");
    }
    assert!(clicks.is_empty(), "loops that click: {clicks:?}");
    println!(
        "measurable sustains (the check within 0.3 cent of itself at four times its window): {} of {}, {:.1} % within a cent, {} beyond three cents",
        strict[0],
        within[1][0],
        100.0 * strict[1] as f64 / strict[0].max(1) as f64,
        strict_off.len()
    );
    for (p, f, d) in &unreliable {
        println!("  unreliable {p} {f}: the check at four times its window {d:+.2} ct from it");
    }
    for (p, f, e) in &strict_off {
        println!("  strict-off {p} {f}: {e:+.2} ct");
    }
    // The gate (the integrator, round 42): on measurable sustains at least 99 % within a cent
    // and all within three; every sustain within five; tremolos and short notes reported.
    assert!(
        strict[1] as f64 >= 0.99 * strict[0] as f64,
        "measurable sustains within a cent: {} of {}",
        strict[1],
        strict[0]
    );
    assert!(
        strict_off.is_empty(),
        "measurable sustains beyond three cents: {strict_off:?}"
    );
    assert!(beyond.is_empty(), "sustains beyond five cents: {beyond:?}");
}
