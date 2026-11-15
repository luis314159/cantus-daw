//! Tympanum's listening renders (round 40 phase 3, checkpoint B): a short groove — kick,
//! snare with ghosts, the hi-hat closing and opening by CC4 with the foot's chicks, a tom
//! fill, a crash — played by a recorded kit through its microphones (the full mix and each
//! mic soloed) and by the modelled studio kit, through the engine's offline render, written
//! as 24-bit FLAC.
//!
//! ```text
//! cargo run --release -p cantus-ui --example tympanum_groove -- <out dir> <content root> [pack/kit]
//! ```
//!
//! `<content root>` holds installed packs (`<root>/<pack>/cantus-pack.json`); the kit
//! defaults to `gogodze-phu-vol-ii/kit`.
#[path = "tympanum_measure/groove.rs"]
#[allow(dead_code)] // the phrase is tympanum_measure's
mod groove;

use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use groove::{groove, stats, write_flac};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).expect("an output folder"));
    let root = PathBuf::from(args.get(2).expect("a content root"));
    let kit = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "gogodze-phu-vol-ii/kit".into());
    std::fs::create_dir_all(&out).unwrap();
    cantus_ui::tympanum_load::install(root);
    let (mut engine, _proc) = EngineHandle::manual(48_000.0);
    let mut renders: Vec<(String, String, Option<u16>)> = Vec::new();
    let pack_kit = format!("pack:{kit}");
    // Read the kit once, to name its mics.
    let def = cantus_engine::inst::tympanum::cache::recorded(
        &cantus_core::inst::tympanum::KitRef::parse(&pack_kit).expect("pack/kit"),
    )
    .expect("the kit reads");
    for line in &def.log {
        println!("{kit}: {line}");
    }
    renders.push(("recorded-full-mix".into(), pack_kit.clone(), None));
    for m in &def.def.mics {
        let name = m.name.to_lowercase().replace(' ', "-");
        renders.push((
            format!("recorded-mic-{:02}-{name}", m.output.0),
            pack_kit.clone(),
            Some(m.output.0),
        ));
    }
    let studio = cantus_core::inst::tympanum_kit::modelled_kit("studio").unwrap();
    renders.push((
        "modelled-studio-full-mix".into(),
        "model:studio".into(),
        None,
    ));
    for m in &studio.mics {
        if matches!(
            m.name.as_str(),
            "Overheads" | "Room Near" | "Kick Out" | "Snare Top"
        ) {
            let name = m.name.to_lowercase().replace(' ', "-");
            renders.push((
                format!("modelled-mic-{:02}-{name}", m.output.0),
                "model:studio".into(),
                Some(m.output.0),
            ));
        }
    }
    for (i, (name, kit, solo)) in renders.iter().enumerate() {
        let mut p = Project::new("Tympanum");
        let p = groove(&mut p, kit, *solo);
        let snap = engine.compile_for_render(&p, i as u64 + 1);
        let r = render(
            snap,
            &RenderOptions {
                tail_seconds: 2.5,
                ..Default::default()
            },
        )
        .expect("render");
        let (peak, rms) = stats(&r.samples);
        println!(
            "{name}: {:.1} s, peak {peak:.1} dBFS, RMS {rms:.1} dBFS",
            r.samples.len() as f32 / 96_000.0
        );
        write_flac(&out.join(format!("{name}.flac")), &r.samples);
    }
}
