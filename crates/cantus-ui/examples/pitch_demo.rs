//! The pitch editor's demo project (round 28, ADR 0054), for the owner's ears:
//!
//! ```text
//! cargo run -p cantus-ui --example pitch_demo -- <folder>
//! ```
//!
//! writes `pitch-demo.cantus` and `voice-take.wav` into the folder (see
//! `cantus_ui::pitch::demo`). Open the project with *File ▸ Open*.

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/pitch-demo"));
    match cantus_ui::pitch::demo::write_demo(&dir) {
        Ok(p) => println!("wrote {}", p.display()),
        Err(e) => {
            eprintln!("pitch demo: {e}");
            std::process::exit(1);
        }
    }
}
