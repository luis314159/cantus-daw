//! The projects the start-up scorecard opens Cantus with (`docs/scorecard/README.md`), so
//! Cantus is measured with what the other programs start with as well as with its own
//! start (the demo it opens when it is given nothing):
//!
//! - `empty.cantus` — the *Empty song* starter: what REAPER starts with (an empty project);
//! - `beat.cantus` — the *Beat* starter: the nearest to FL Studio's default template (a
//!   few drum channels and a return);
//! - `demo.cantus` — the demo Cantus opens on, saved, for checking that opening it from a
//!   file costs what starting on it does.
//!
//! ```text
//! cargo run --release -p cantus-ui --example scorecard_projects -- <dir>
//! ```

use cantus_ui::templates::Starter;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir)?;
    for (name, project) in [
        ("empty", Starter::Empty.build()),
        ("beat", Starter::Beat.build()),
        ("demo", cantus_ui::demos::startup_project()),
    ] {
        let path = dir.join(format!("{name}.cantus"));
        cantus_project::save(&project, &path, cantus_ui::APP_VERSION)?;
        println!("{} ({} tracks)", path.display(), project.tracks.len());
    }
    Ok(())
}
