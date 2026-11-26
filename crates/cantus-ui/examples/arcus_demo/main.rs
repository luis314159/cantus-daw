//! **Andante cantabile** in D major — Arcus's modelled strings heard as a piece of music
//! (round 42, ADR 0157, ADR 0158): an original composition for violin, viola, cello and
//! double bass in which the instruments enter one by one, carry the theme to a climax and
//! leave again, ending with the violin and the cello alone. For the owner's ears.
//!
//! ```text
//! cargo run -p cantus-ui --example arcus_demo -- [out-dir]
//! ```
//!
//! (`tools/arcus/demo.sh` runs it.) The song is built as a user builds it, in a running app
//! (`build.rs`); then, into `out-dir` (default `../listen/arcus-demo` beside the worktree):
//!
//! - `arcus-cuarteto.cantus` — the project, to open in Cantus (*File ▸ Open*);
//! - `arcus-cuarteto.wav` — the export dialog's export (48 kHz, 24-bit, −16 LUFS, peaks
//!   held under −1 dBTP by the export's own true-peak limiter);
//! - `stems/` — every strip as a 24-bit WAV at the mix's level (the export's stems);
//! - `index.html` — a page in Spanish with a player, the sections and their times.
//!
//! And it checks what it wrote: a second export is the same to the byte; the saved project,
//! loaded into a new app, exports the same bytes again; and the measures of `measure.rs` —
//! loudness and peaks, no full-scale sample, each instrument audible where it plays and
//! silent where it does not, no click at any note's start or end, the hall's tail whole.
//!
//! r42 arcus-expr: the quartet is **performed** by default (`perform.rs`: bows, attacks,
//! dynamics, vibrato, timing and intonation entered as a user enters them);
//! `ARCUS_DEMO_STYLE=plain` builds pass 6's take. `arcus_demo -- <out-dir> solos` renders the
//! solo showcase (`solos.rs`), every instrument plain and performed; `… fiddle` the fiddle's
//! hoedown with its band and one passage classical and fiddle (`fiddle.rs`).

mod build;
mod fiddle;
mod fiddle2;
mod measure;
mod page;
mod parts;
mod perform;
mod score;
mod solos;

use cantus_core::Project;
use cantus_engine::EngineHandle;
use cantus_export::{Channels, Depth, Dither, ExportSettings, Format, Loudness, Rate, Scope};
use cantus_ui::app::Files;
use cantus_ui::i18n::Locale;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

pub const TITLE: &str = "Andante cantabile en re mayor";
/// Seconds after the last note, for the hall to ring out.
const TAIL: f32 = 7.0;
/// The loudness the master is written at, and its ceiling.
const LUFS: f32 = -16.0;
const CEILING: f32 = -1.0;

/// A fresh app on `song`, its data in `data`, a manual engine at 48 kHz (no device).
fn app_on(data: &Path, song: Project) -> (CantusApp, cantus_engine::Processor) {
    std::env::set_var("CANTUS_DATA_DIR", data);
    std::fs::write(
        data.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .expect("the settings");
    let files = Files::read(data, Some(Locale::Es));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    (app, proc_)
}

/// The export dialog's choices: WAV, 24-bit, the session's rate; the mix at −16 LUFS under
/// −1 dBTP, or the stems at their level.
fn choices(stems: bool) -> ExportSettings {
    let mut s = ExportSettings {
        scope: if stems { Scope::Stems } else { Scope::Song },
        format: Format::Wav,
        depth: Depth::Int24,
        rate: Rate::Session,
        channels: Channels::Stereo,
        dither: Dither::Tpdf,
        loudness: Loudness {
            target_lufs: Some(LUFS),
            ceiling_dbtp: CEILING,
            limit: true,
        },
        tail_seconds: TAIL,
        ..Default::default()
    };
    s.tags.title = TITLE.to_string();
    s.tags.artist = "Cantus · Arcus".to_string();
    s.tags.year = "2026".to_string();
    s.tags.comment = "Arcus, las cuerdas frotadas modeladas de Cantus (ronda 42).".to_string();
    s
}

/// Export through the app's own export (`views::export::start_with`, what the dialog's
/// button runs) and wait for it; the summary it says.
fn export(app: &mut CantusApp, target: &Path, stems: bool) -> String {
    assert!(
        cantus_ui::views::export::start_with(app, target, Some(choices(stems))),
        "the export did not start"
    );
    cantus_ui::views::export::wait(app);
    match &app.export.last {
        Some(Ok(text)) => text.clone(),
        other => panic!("the export of {} failed: {other:?}", target.display()),
    }
}

fn main() {
    let solos_only = std::env::args().nth(2).is_some_and(|a| a == "solos");
    let fiddle_only = std::env::args().nth(2).is_some_and(|a| a == "fiddle");
    let out: PathBuf = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../listen/arcus-demo")
        });
    std::fs::create_dir_all(&out).expect("the output folder");
    let since = std::time::Instant::now();
    if solos_only || fiddle_only {
        let ok = if fiddle_only {
            fiddle::render(&out)
        } else {
            solos::render(&out)
        };
        println!(
            "{} in {:.0} s",
            if ok {
                "ALL CHECKS PASS"
            } else {
                "A CHECK FAILED"
            },
            since.elapsed().as_secs_f64()
        );
        std::process::exit(if ok { 0 } else { 1 });
    }
    let style = match std::env::var("ARCUS_DEMO_STYLE").as_deref() {
        Ok("plain") => build::Style::Plain,
        _ => build::Style::Performed,
    };
    println!("style: {style:?}");

    // ---- the song, built and saved ----
    let data = tempfile::tempdir().expect("a data folder");
    let (mut app, _proc) = app_on(data.path(), Project::new(TITLE));
    let built = build::build(&mut app, &parts::QUARTET, style);
    let project_path = out.join("arcus-cuarteto.cantus");
    app.save_to_path(&project_path);
    println!("saved {}", project_path.display());

    // ---- the mix and the stems, through the export ----
    let wav = out.join("arcus-cuarteto.wav");
    println!("mix: {}", export(&mut app, &wav, false));
    let stems = out.join("stems");
    if stems.is_dir() {
        for f in std::fs::read_dir(&stems)
            .expect("the stems folder")
            .flatten()
        {
            if f.path().extension().is_some_and(|e| e == "wav") {
                let _ = std::fs::remove_file(f.path());
            }
        }
    }
    println!("stems: {}", export(&mut app, &stems, true));
    println!("rendered in {:.0} s", since.elapsed().as_secs_f64());

    // ---- the same bytes: again, and from the saved file in a new app ----
    let check = tempfile::tempdir().expect("a check folder");
    let again = check.path().join("again.wav");
    export(&mut app, &again, false);
    let same_again = measure::same_bytes(&wav, &again);
    println!("a second export is the same to the byte: {same_again}");
    let loaded = cantus_project::store::load(&project_path, true)
        .expect("the saved project loads")
        .project;
    let same_project = loaded == *app.project();
    println!("the loaded project equals the built one: {same_project}");
    let data2 = tempfile::tempdir().expect("a second data folder");
    let (mut app2, _proc2) = app_on(data2.path(), loaded);
    let reloaded = check.path().join("reloaded.wav");
    export(&mut app2, &reloaded, false);
    let same_reloaded = measure::same_bytes(&wav, &reloaded);
    println!("the loaded project exports the same bytes: {same_reloaded}");

    // ---- listened to by measurement, and the page ----
    let report = page::Report::measure(app.project(), &built, &wav, &stems);
    report.print();
    page::write(&out, &report).expect("the page");
    println!("wrote {}", out.join("index.html").display());
    let ok = same_again && same_reloaded && report.passes();
    println!(
        "{} in {:.0} s",
        if ok {
            "ALL CHECKS PASS"
        } else {
            "A CHECK FAILED"
        },
        since.elapsed().as_secs_f64()
    );
    if !ok {
        std::process::exit(1);
    }
}
