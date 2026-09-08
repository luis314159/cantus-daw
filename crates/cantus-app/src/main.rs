//! Cantus — the application binary. All logic lives in `cantus-ui` and below; this file
//! only parses arguments and starts the app.

#![forbid(unsafe_code)]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use cantus_ui::LaunchOptions;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut options = LaunchOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--stress" => options.stress = true,
            "--locale" => options.locale = args.next(),
            "--version" | "-V" => {
                println!("cantus {}", cantus_ui::APP_VERSION);
                return Ok(());
            }
            "--help" | "-h" => {
                println!("cantus [--stress] [--locale en|es] [project.cantus]");
                return Ok(());
            }
            other => options.project = Some(PathBuf::from(other)),
        }
    }
    cantus_ui::run(options)
}
