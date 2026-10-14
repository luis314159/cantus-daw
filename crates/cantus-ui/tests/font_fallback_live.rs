//! The fallback armed, as in the real window (its own test binary: arming is per process;
//! the tests share one state, so each uses characters of its own).

use cantus_ui::font_fallback::{self, Script};
use cantus_ui::kit::text::{self, Role, Text};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One test at a time: they share the fallback's state and the armed flag.
static SERIAL: Mutex<()> = Mutex::new(());

fn window() -> egui::Context {
    let ctx = egui::Context::default();
    // The real window's font stage (`boot`): arms the fallback and installs the fonts.
    font_fallback::install_for_window(&ctx);
    ctx
}

fn frame(ctx: &egui::Context, label: &str, events: Vec<egui::Event>) {
    let mut out = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            font_fallback::note_input(ui.ctx());
            if !label.is_empty() {
                let _ = text::measure(
                    ui.ctx(),
                    &Text::new(label, Role::Body, egui::Color32::WHITE),
                    1.0,
                );
            }
            font_fallback::poll(ui.ctx());
        },
    );
    out.textures_delta.clear();
}

fn system_keys(ctx: &egui::Context) -> Vec<String> {
    ctx.fonts(|f| {
        f.definitions()
            .font_data
            .keys()
            .filter(|k| k.starts_with("system:"))
            .cloned()
            .collect()
    })
}

fn wait_for(ctx: &egui::Context, label: &str, done: impl Fn(&egui::Context) -> bool) -> bool {
    for _ in 0..200 {
        frame(ctx, label, Vec::new());
        if done(ctx) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
}

/// The desktop app's font stage is what arms it: the path `run` → `boot` takes.
#[test]
fn the_boot_font_stage_arms_the_fallback() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _ = window();
    assert!(font_fallback::armed());
    let boot = include_str!("../src/boot.rs");
    let stage = &boot[boot.find("Stage::Fonts => {").expect("the font stage")..];
    let stage = &stage[..stage.find("Stage::Files =>").unwrap()];
    assert!(
        stage.contains("font_fallback::install_for_window(ctx)"),
        "boot's font stage must arm the fallback:\n{stage}"
    );
}

#[test]
fn a_japanese_label_brings_the_systems_face_without_blocking_the_frame() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if font_fallback::faces_for(Script::Japanese).is_empty() {
        eprintln!("no system Japanese font here: skipped");
        return;
    }
    let ctx = window();
    let t0 = Instant::now();
    frame(&ctx, "日本語のトラック", Vec::new());
    let first = t0.elapsed();
    assert!(
        first < Duration::from_millis(500),
        "the first frame took {first:?}"
    );
    assert!(
        wait_for(&ctx, "日本語のトラック", |c| font_fallback::asked(
            '語'
        ) && !system_keys(c)
            .is_empty()),
        "the Japanese face never arrived"
    );
    eprintln!("system face installed after {:?}", t0.elapsed());
    // Each face once in every chain, however many characters asked for it.
    let chain = ctx.fonts(|f| f.definitions().families[&egui::FontFamily::Proportional].clone());
    let mut keys = chain.clone();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), chain.len(), "{chain:?}");
    // A Latin-only text asks for nothing more.
    let before = system_keys(&ctx).len();
    frame(&ctx, "Kick, Snare, Hi-hat — Canción", Vec::new());
    assert_eq!(system_keys(&ctx).len(), before);
}

/// Text composed with an input method (how CJK is typed) asks too: the preedit and the commit.
#[test]
fn input_method_text_asks_for_its_faces() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = window();
    assert!(!font_fallback::asked('한'));
    frame(
        &ctx,
        "",
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "한".into(),
            active_range_chars: None,
        })],
    );
    assert!(font_fallback::asked('한'), "the preedit asked");
    assert!(!font_fallback::asked('글'));
    frame(
        &ctx,
        "",
        vec![egui::Event::Ime(egui::ImeEvent::Commit("글".into()))],
    );
    assert!(font_fallback::asked('글'), "the commit asked");
}

/// A miss on the interface thread is a few lookups, even for a character nothing covers
/// (asked once, then remembered): never a walk of the font folders.
#[test]
fn a_miss_costs_the_interface_thread_almost_nothing() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = window();
    // U+10FFFD: a private-use character no font maps.
    let odd = "\u{10FFFD}";
    font_fallback::note_text(&ctx, odd);
    assert!(wait_for(&ctx, "", |_| font_fallback::asked('\u{10FFFD}')));
    let t0 = Instant::now();
    for _ in 0..10_000 {
        font_fallback::note_text(&ctx, odd);
    }
    let each = t0.elapsed() / 10_000;
    eprintln!("a repeated miss costs {each:?}");
    assert!(each < Duration::from_micros(50), "{each:?} per miss");
}
