//! Cantus — the application binary. All logic lives in `cantus-ui` and below; this file
//! only parses arguments, hands over the art baked in at build time (`build.rs`: the
//! splash's logo and the window's icon), and starts the app.

#![forbid(unsafe_code)]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use cantus_ui::LaunchOptions;
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/baked.rs"));

/// A measurement build's heap census (ADR 0120 §1): never in a release.
#[cfg(all(feature = "census", windows))]
#[global_allocator]
static CENSUS: cantus_rt::census::Census = cantus_rt::census::Census;

fn main() -> anyhow::Result<()> {
    // Before any window: the taskbar identity the installer's shortcuts carry too, so a
    // Cantus started by the launcher groups (and pins) under its shortcut.
    let app_id = cantus_launch::set_app_id();
    #[cfg(all(feature = "census", windows))]
    cantus_rt::census::report_to_env();
    // The log goes to the console and to `logs/cantus.log` in the data folder: a release on
    // Windows has no console, so the file is what a user's report can send (issue #1, ADR
    // 0149). egui_glow's debug lines name the OpenGL driver (version, renderer, vendor).
    let mut logger = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,egui_glow=debug"),
    );
    if let Some(file) = cantus_ui::window_watch::open_log(&cantus_ui::app::data_dir()) {
        logger.target(env_logger::Target::Pipe(Box::new(Tee(file))));
    }
    logger.init();
    log::info!("cantus {} ({})", cantus_ui::APP_VERSION, build_level());
    if let Err(hr) = app_id {
        log::warn!("could not set the taskbar identity (HRESULT {hr:#x})");
    }
    let mut options = LaunchOptions {
        art: Some(ART),
        ..Default::default()
    };
    let mut args = std::env::args().skip(1);
    let mut graphics = cantus_ui::graphics_options::Graphics::default();
    while let Some(arg) = args.next() {
        // `--safe-graphics`, `--vsync`, `--msaa`, `--no-splash`, `--scale` (issue #1).
        if graphics.take_arg(&arg, &mut args) {
            continue;
        }
        match arg.as_str() {
            "--stress" => options.stress = true,
            "--locale" => options.locale = args.next(),
            "--mixer" => options.mixer = true,
            "--rack" => options.rack = true,
            "--track" => options.track = args.next().and_then(|s| s.parse().ok()),
            "--shortcuts" => options.shortcuts = true,
            "--settings" => options.settings = true,
            "--editor" => options.editor = args.next(),
            "--play" => options.play = true,
            "--window" => {
                let w = args.next().and_then(|s| s.parse::<f32>().ok());
                let h = args.next().and_then(|s| s.parse::<f32>().ok());
                options.window = w.zip(h);
            }
            // The driver types this build has (the release task checks for ASIO).
            "--audio-drivers" => {
                println!("{}", cantus_ui::audio_setup::drivers::report());
                return Ok(());
            }
            "--version" | "-V" => {
                println!("cantus {} ({})", cantus_ui::APP_VERSION, build_level());
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "cantus [--stress] [--locale en|es] [--mixer] [--rack] [--track N] [--shortcuts] [--settings] [--editor eq|comp] [--play] [--window W H] [--safe-graphics] [--vsync on|off] [--msaa N] [--no-splash] [--scale F] [--audio-drivers] [project.cantus]"
                );
                println!(
                    "\nGraphics trouble (a flickering or black window): docs/troubleshooting-graphics.md. CANTUS_GRAPHICS=safe does what --safe-graphics does; CANTUS_WINDOW_TRACE=1 logs every change of the window. The log: logs/cantus.log in the data folder."
                );
                return Ok(());
            }
            other => options.project = Some(PathBuf::from(other)),
        }
    }
    // A flag on the command line wins over `CANTUS_GRAPHICS`.
    options.graphics = graphics.over(cantus_ui::graphics_options::Graphics::from_env());
    cantus_ui::run(options)
}

/// The log written to the file and to the console both.
struct Tee(std::fs::File);

impl std::io::Write for Tee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        self.0.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        self.0.flush()
    }
}

/// The x86-64 level this program was compiled for (`x86-64-v3`, `x86-64-v2`, …): a release
/// carries one program per level and the launcher starts the highest the processor runs
/// (`crates/cantus-launch`). A plain `cargo build` is `x86-64` (or the target's arch).
fn build_level() -> &'static str {
    match cantus_launch::Level::of_this_build() {
        Some(level) => level.name(),
        None if cfg!(target_arch = "x86_64") => "x86-64",
        None => std::env::consts::ARCH,
    }
}
