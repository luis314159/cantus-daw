//! Try a pack's stereo mix of a DrumGizmo kit without installing it (round 28).
//!
//! `cargo run --release -p cantus-content --example drumkit_mix -- <kit folder> <out folder> [pack id] [manifest.json]`
//!
//! Renders the build step of the pack (default `drskit`, from the bundled manifest or the
//! one given) from the unpacked kit in `<kit folder>` into `<out folder>` — the SFZ and its
//! stereo files, the kit untouched — then reads every file back and prints, per
//! instrument, its hits, the loudest peak and the range of levels, and the whole mix's peak.

use cantus_content::kitbuild;
use cantus_content::manifest::Manifest;
use cantus_content::wav;
use std::path::Path;
use std::sync::atomic::AtomicBool;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (src, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let id = args.get(2).map(String::as_str).unwrap_or("drskit");
    let manifest = match args.get(3) {
        Some(p) => Manifest::parse(&std::fs::read_to_string(p).unwrap()).unwrap(),
        None => Manifest::bundled(),
    };
    let pack = manifest.pack(id).expect("the pack");
    let b = pack.build.as_ref().expect("a build step");
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out).unwrap();
    let t = std::time::Instant::now();
    let built = kitbuild::render(
        b,
        src,
        out,
        &pack.credit_full().unwrap_or_default(),
        &AtomicBool::new(false),
        &mut |_, _| {},
    )
    .unwrap();
    println!(
        "{} files, {} regions, peak {:.2} dBFS, {} clamped, {} left out, {:.1} s",
        built.files,
        built.regions,
        20.0 * built.peak.max(1e-9).log10(),
        built.clamped,
        built.dropped,
        t.elapsed().as_secs_f64()
    );
    for l in &built.log {
        println!("  {l}");
    }
    let mut bytes = 0u64;
    for k in &b.keys {
        let dir = out.join(&b.folder).join(&k.instrument);
        let mut peaks = Vec::new();
        let mut levels = Vec::new();
        let mut secs = 0.0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            bytes += e.metadata().unwrap().len();
            let a = wav::read(&e.path()).unwrap();
            let p = a.samples.iter().fold(0f32, |m, x| m.max(x.abs()));
            peaks.push(20.0 * p.max(1e-9).log10());
            levels.push(kitbuild::level_db(&a.samples, a.rate));
            secs += a.frames() as f64 / a.rate as f64;
        }
        let max = |v: &[f32]| v.iter().cloned().fold(f32::MIN, f32::max);
        let min = |v: &[f32]| v.iter().cloned().fold(f32::MAX, f32::min);
        println!(
            "{:>22} keys {:?}: {:2} hits, peak {:6.1} dBFS, level {:6.1} … {:6.1} dB, {:5.1} s",
            k.instrument,
            k.keys,
            peaks.len(),
            max(&peaks),
            min(&levels),
            max(&levels),
            secs
        );
    }
    println!("{:.0} MB of stereo files", bytes as f64 / 1e6);
}
