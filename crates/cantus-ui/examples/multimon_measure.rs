//! Data folders for measuring what a window of its own costs (ADR 0151): `cargo run -p
//! cantus-ui --example multimon_measure -- <dir> <screen-width> <screen-height> <scale>`
//! writes `<dir>/w1`, `w2` and `w3` — a configured Cantus (no first-run setup, no start
//! page, no plug-in scan) whose layout opens the main window alone, with the mixer in a
//! window of its own, and with the piano roll in a third — the windows side by side on the
//! one screen given (its system name `\\.\DISPLAY1`). `tools/scorecard/windows.ps1` starts
//! Cantus on each and reads its cost.

use cantus_ui::layout::{Layout, Layouts};
use cantus_ui::screens::{Place, PxRect, ScreenKey};
use cantus_ui::views::workspace::Pane;
use cantus_ui::windows::WindowShape;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("an output folder"));
    let w: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1920);
    let h: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1080);
    let scale: f32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let screen = ScreenKey::new(r"\\.\DISPLAY1", [w, h], [0, 0], scale);
    let half = |y: i32, hh: i32| Place {
        screen: screen.clone(),
        rect: PxRect {
            x: (w / 2) as i32,
            y,
            w: (w / 2) as i32,
            h: hh,
        },
        maximised: false,
        full_screen: false,
    };
    let window = |key: u32, pane: Pane, place: Place| WindowShape {
        key,
        name: String::new(),
        dock: cantus_ui::layout::dock_to_value(&egui_dock::DockState::new(vec![pane])),
        place: Some(place),
        bar: true,
        groups: Vec::new(),
        on_top: false,
        scale: None,
    };
    let hh = (h as i32 - 60) / 2;
    for n in 1..=3usize {
        let d = dir.join(format!("w{n}"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false,"ui_scale_follows_screen":true}"#,
        )
        .unwrap();
        let mut lay = Layout::default_layout();
        if n >= 2 {
            lay.windows.push(window(1, Pane::Mixer, half(0, hh)));
        }
        if n >= 3 {
            lay.windows.push(window(2, Pane::PianoRoll, half(hh, hh)));
        }
        let mut layouts = Layouts::default();
        layouts.current = lay;
        layouts.save(&cantus_ui::layout::layouts_path(&d)).unwrap();
        println!("{}", d.display());
    }
}
