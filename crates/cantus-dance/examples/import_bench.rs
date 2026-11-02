//! What an imported character costs a frame (ADR 0089, the budget of ADR 0087 §3: one
//! character on a 600×600 stage in ≤ 1.5 ms): our generated figure in every format, and
//! any files given, each over its routine.
//!
//! `cargo run --release -p cantus-dance --example import_bench -- [files…]`

use cantus_dance::import::{self, samples};
use std::path::PathBuf;

fn main() {
    let dir = std::env::temp_dir().join(format!("cantus-import-bench-{}", std::process::id()));
    let mut all = samples::aseprite("aseprite");
    all.extend(samples::texturepacker("texturepacker", true));
    all.extend(samples::lottie("lottie"));
    all.extend(samples::dotlottie("package"));
    all.extend(samples::dragonbones("dragonbones"));
    samples::write(&dir, &all).expect("write the samples");
    let mut files: Vec<PathBuf> = [
        "aseprite.json",
        "texturepacker.json",
        "lottie.json",
        "package.lottie",
        "dragonbones_ske.json",
    ]
    .iter()
    .map(|f| dir.join(f))
    .collect();
    files.extend(std::env::args().skip(1).map(PathBuf::from));
    println!(
        "{:<28} {:>9} {:>11} {:>11}  heavy",
        "file", "load ms", "frame ms", "hue sweep"
    );
    for f in files {
        let t0 = std::time::Instant::now();
        let found = match import::inspect(&f) {
            Ok(found) => found,
            Err(e) => {
                println!("{}: {e}", f.display());
                continue;
            }
        };
        let load = t0.elapsed().as_secs_f64() * 1000.0;
        let heavy = found.source.heavy();
        let settings = import::guessed(&found);
        let ch = import::character(found, "file:bench".into(), settings);
        let ms = import::cost_ms(&ch, 240);
        // A hue sweep: the colour turning one degree a frame (automation on `hue`).
        let sweep = {
            use cantus_dance::render::{render_into, Background, Stage};
            let stage = Stage::new(600, 600, Background::Transparent);
            let mut pm = tiny_skia::Pixmap::new(600, 600).expect("a stage");
            let f = cantus_dance::DanceFrame {
                beats: 1.3,
                seconds: 0.65,
                playing: true,
                ..cantus_dance::DanceFrame::default()
            };
            let mut pose = cantus_dance::pose_at(&f, &ch, 0.0);
            let t0 = std::time::Instant::now();
            for i in 0..240 {
                pose.hue = (i % 360) as f32;
                render_into(&mut pm, &stage, &[(&ch, pose)]);
            }
            t0.elapsed().as_secs_f64() * 1000.0 / 240.0
        };
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        println!("{name:<28} {load:>9.2} {ms:>11.3} {sweep:>11.3}  {heavy}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
