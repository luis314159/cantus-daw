//! The song the hour under OBS plays (round 35, ADR 0091 §5, `tools/obs/stage_hour.py`):
//! the welcome song, looping, its master turned down so an hour on this machine's speakers
//! is a murmur — the dancer follows the transport, not the volume, and the visualiser still
//! has a spectrum to show. With `float` or `hidden`, a dancer track is added beside the
//! song's own (the same music): what a dancer costs the main window is measured against it.
//!
//! ```text
//! cargo run -p cantus-ui --example stage_obs_song -- <out.cantus> [master dB, default -30] [none|float|hidden]
//! ```

fn main() {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().expect("the project file to write"));
    let gain: f32 = args.next().and_then(|g| g.parse().ok()).unwrap_or(-30.0);
    let dancer = args.next().unwrap_or_else(|| "none".into());
    let mut p = cantus_ui::welcome_project();
    if dancer != "none" {
        use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec};
        let mut t = p.make_track("Dancer");
        let mut spec = NativeSpec::new(InstrumentKind::Dancer);
        spec.set("x", 0.8);
        spec.set("view.mode", if dancer == "hidden" { 2.0 } else { 0.0 });
        t.instrument = InstrumentSpec::Native(spec);
        p.tracks.push(t);
    }
    p.name = "Morning Light (OBS test)".into();
    p.master.gain_db = gain;
    assert!(
        p.loop_enabled && p.loop_region.is_some(),
        "the welcome song loops"
    );
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).expect("the folder");
    }
    cantus_project::store::save(&p, &out, env!("CARGO_PKG_VERSION")).expect("saved");
    println!("{}", out.display());
}
