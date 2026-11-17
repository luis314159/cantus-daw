//! The safe-graphics start (GitHub issue #1, ADR 0149 — provisional number): each flag
//! reaches what opens the window — `cantus_ui::native_options`, which `run` hands eframe,
//! and `boot::main_window_commands`, which the splash sends to become the main window —
//! parsed as `crates/cantus-app`'s `main` parses the command line.

use cantus_ui::boot::{self, main_window_commands, MAIN_SIZE, MIN_SIZE};
use cantus_ui::graphics_options::Graphics;
use cantus_ui::{native_options, LaunchOptions};
use egui::{vec2, ViewportCommand as C};

/// Options as `main` builds them from a command line (graphics flags only, plus
/// `--window`).
fn launch(args: &[&str]) -> LaunchOptions {
    let mut o = LaunchOptions::default();
    let mut g = Graphics::default();
    let mut it = args.iter().map(|s| s.to_string());
    while let Some(a) = it.next() {
        if g.take_arg(&a, &mut it) {
            continue;
        }
        if a == "--window" {
            let w = it.next().and_then(|s| s.parse::<f32>().ok());
            let h = it.next().and_then(|s| s.parse::<f32>().ok());
            o.window = w.zip(h);
            continue;
        }
        panic!("not a flag here: {a}");
    }
    o.graphics = g.over(Graphics::default());
    o
}

#[test]
fn by_default_the_window_opens_as_the_splash_with_vsync_and_no_multisampling() {
    let n = native_options(&launch(&[]), false);
    assert!(n.glow_options.vsync);
    assert_eq!(n.multisampling, 0);
    assert_eq!(n.viewport.decorations, Some(false));
    assert_eq!(n.viewport.resizable, Some(false));
    assert_eq!(n.viewport.inner_size, Some(boot::size(false)));
    assert_eq!(n.viewport.transparent, Some(false), "opaque on Windows");
    assert!(n.centered);
    // The splash then becomes the main window: each command once, the size before the
    // maximise.
    let c = main_window_commands(&launch(&[]), "Song \u{2014} Cantus");
    assert_eq!(
        c,
        vec![
            C::Title("Song \u{2014} Cantus".into()),
            C::Decorations(true),
            C::Transparent(false),
            C::Resizable(true),
            C::MinInnerSize(MIN_SIZE),
            C::InnerSize(MAIN_SIZE),
            C::Maximized(true),
        ]
    );
    let c = main_window_commands(&launch(&["--window", "1600", "900"]), "t");
    assert_eq!(c.last(), Some(&C::InnerSize(vec2(1600.0, 900.0))));
    assert!(!c.contains(&C::Maximized(true)));
}

#[test]
fn vsync_and_multisampling_reach_the_native_options() {
    assert!(
        !native_options(&launch(&["--vsync", "off"]), false)
            .glow_options
            .vsync
    );
    assert!(
        native_options(&launch(&["--vsync", "on"]), false)
            .glow_options
            .vsync
    );
    assert_eq!(
        native_options(&launch(&["--msaa", "4"]), false).multisampling,
        4
    );
    assert_eq!(
        native_options(&launch(&["--msaa", "0"]), false).multisampling,
        0
    );
}

#[test]
fn no_splash_opens_the_main_window_at_once_and_the_boot_only_titles_it() {
    let o = launch(&["--no-splash"]);
    let n = native_options(&o, true);
    assert_eq!(n.viewport.decorations, Some(true));
    assert_eq!(n.viewport.resizable, Some(true));
    assert_eq!(n.viewport.maximized, Some(true));
    assert_eq!(n.viewport.inner_size, Some(MAIN_SIZE));
    assert_eq!(n.viewport.min_inner_size, Some(MIN_SIZE));
    assert_eq!(
        main_window_commands(&o, "t"),
        vec![C::Title("t".into()), C::Transparent(false)],
        "no geometry is sent: the window is already the main window"
    );
    let o = launch(&["--no-splash", "--window", "1600", "900"]);
    let n = native_options(&o, false);
    assert_eq!(n.viewport.maximized, Some(false));
    assert_eq!(n.viewport.inner_size, Some(vec2(1600.0, 900.0)));
}

#[test]
fn safe_graphics_is_every_safer_choice_at_once() {
    let o = launch(&["--safe-graphics"]);
    let n = native_options(&o, false);
    assert!(n.glow_options.vsync);
    assert_eq!(n.multisampling, 0);
    assert_eq!(n.viewport.decorations, Some(true), "no splash");
    assert_eq!(n.viewport.transparent, Some(false));
    assert_eq!(o.graphics.scale(), Some(1.0));
    // A flag still wins inside safe graphics.
    let o = launch(&["--safe-graphics", "--vsync", "off", "--scale", "1.5"]);
    assert!(!native_options(&o, false).glow_options.vsync);
    assert_eq!(o.graphics.scale(), Some(1.5));
    // The variable's words, under the command line's.
    let env = Graphics::from_words("safe");
    let o = LaunchOptions {
        graphics: Graphics::default().over(env),
        ..Default::default()
    };
    assert_eq!(native_options(&o, false).viewport.decorations, Some(true));
}

/// The boot sends `main_window_commands` and nothing else to become the main window (the
/// boot itself opens the audio device, so it is read here, not run).
#[test]
fn the_boot_sends_the_tested_commands() {
    let src = include_str!("../src/boot.rs").replace('\r', "");
    let f = &src[src.find("fn become_main_window(").expect("the transition")..];
    let f = &f[..f.find("\n}\n").unwrap()];
    assert!(
        f.contains("main_window_commands(options, title)"),
        "the transition is the tested list:\n{f}"
    );
    assert_eq!(f.matches("send_viewport_cmd").count(), 1, "{f}");
    let lib = include_str!("../src/lib.rs").replace('\r', "");
    let run = &lib[lib.find("pub fn run(").unwrap()..];
    assert!(
        run.contains("native_options(&options, verse.shown.is_some())"),
        "run hands eframe the tested options"
    );
}
