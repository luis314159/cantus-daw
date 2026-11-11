//! Slate, the whole window, held to the pixel (round 39, `lapis2`): the default theme must
//! not move when another theme of its family does (Slate Lapis's selection tint, gold
//! readouts and night-blue room are tokens Slate leaves clear). The arrangement (a clip
//! of the Chords track selected, a loop of bars 5–9, the playhead in bar 6), the mixer
//! maximised in Wide (the Chords strip selected) and the piano roll (six notes selected),
//! in Slate Night and Slate Paper at 1280 × 720, 100 %, through the software renderer,
//! compared with `tests/golden/window/<view>-<theme>.png`.
//!
//! A change to Slate's look is a reviewed change to these images. To re-bless them after
//! an intended change:
//!
//! ```text
//! CANTUS_BLESS=1 cargo test -p cantus-ui --test slate_window_golden
//! ```
//!
//! then look at the new PNGs before committing them. A failing comparison writes the
//! rendered window to `target/slate-window/<view>-<theme>-actual.png`.
//!
//! **What the images are.** First blessed by `lapis2` at `034a1ac0` (on `4c52b48b`);
//! re-blessed by `fix2` from `r39/fix2` at `7edb9fb1` (`r39/integration` at `9c973bc5`, every
//! round 39 branch, `themes` included, merged into fix2's fixes). Every pixel that moved
//! since `4c52b48b` was rendered, diffed and looked at; all of it is an intended change:
//!
//! - lo-mixer (`fa769a57`): the desk fills its width with the next empty strips (numbered,
//!   painted only) — the docked mixer under the arrangement shows 16–27 after the returns;
//! - the fold rule (lo-mixer's overflow slot; `fix2`: a slot only on a side with strips
//!   hidden beyond it, as wide as its count needs — 30 pt for one digit, 36 for two): the
//!   maximised mixer in Wide shows strips 1–11 where the old image had them and folds 12–13
//!   into "2 ›" in a slot of its own (682 + 30 pt of the 715 there are; the old image painted
//!   its "2 ›" over strip 11's meter), the returns at the desk's edge;
//! - shots' menus (`5b3cbb20`, the mixer's menus on their sheet): no pixel in these windows,
//!   where no menu is open;
//! - themes (`9c973bc5`): the arrangement's bar lines at 3:1 and beat lines at 1.35:1 on
//!   the lanes (Night and Paper; in the piano roll's window too, under the arrangement), and
//!   solo's ink at 3:1 on its key (no strip is soloed here, so no pixel); in Slate Paper only,
//!   the track marks at 3:1 (the headers' stripes), the clip bodies with more of their
//!   colour under half the veil, and the darker fader-cap rim (the desk's faders are drawn
//!   by the mixer's own look, so no pixel here);
//! - `fix2`, Slate Paper only: the desk's strip-head stripes at 3:1 on the strip, at rest and
//!   selected (`look::Pal::stripe`, held by `arrangement_legibility`'s `desk_marks`), in the
//!   maximised mixer and the docked one.
//!
//! Slate Night's colours did not otherwise move: from fix2's render before `themes` to
//! after it, Night's mixer is identical to the pixel and its arrangement differs in 438
//! pixels, all of them bar and beat lines; the fitted slot moved Night's mixer only where
//! strip 11 and the pill are, and the desk's stripes leave Night as it was. Re-blessed again
//! at `fix2`'s fitted slot and Paper stripes (the commit after `1d93b577`).
//!
//! Tolerance, as the kit gallery's (`tests/kit_gallery.rs`): anti-aliasing may move by
//! float noise between machines, so a pixel counts as different only when a channel
//! differs by more than [`CHANNEL_TOLERANCE`]; a window fails when more than
//! [`MAX_DIFFERENT`] of its pixels do, or when any is off by more than [`HARD_LIMIT`] —
//! which no rounding explains and any change of colour does.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::kit::gallery;
use cantus_ui::raster;
use cantus_ui::views::mixer::layout::WidthMode;
use cantus_ui::views::workspace::Pane;
use std::path::PathBuf;

const CHANNEL_TOLERANCE: u8 = 4;
const MAX_DIFFERENT: f64 = 0.002;
const HARD_LIMIT: u8 = 48;

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
    M.lock().unwrap_or_else(|e| e.into_inner())
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("window")
}

fn actual_dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("slate-window");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// The window in `preset` showing `view`, rendered.
fn window(view: &str, preset: &str, dark: bool) -> raster::Canvas {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let dir = tempfile::tempdir().expect("temp dir");
    let project = cantus_ui::demos::startup_project();
    let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
    app.settings_path = dir.path().join("settings.json");
    app.layouts_path = dir.path().join("layouts.json");
    app.keymap_path = dir.path().join("keymap.toml");
    app.appearance_path = dir.path().join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    app.appearance
        .assign(app.appearance.mode, format!("preset:{preset}"));
    app.attach(&ctx);
    let chords = app.project().tracks.iter().find_map(|t| {
        if !t.name.to_lowercase().contains("chord") {
            return None;
        }
        t.clips.iter().find(|c| !c.notes.is_empty()).map(|c| {
            let notes: Vec<_> = c.notes.iter().take(6).map(|n| n.id).collect();
            (t.id, c.id, notes)
        })
    });
    let (t, c, notes) = chords.expect("the startup song has a Chords clip");
    app.select_clip(t, c);
    app.selected_notes = notes;
    let bar = cantus_core::PPQ * 4;
    app.loop_span(cantus_core::Ticks(bar * 4), cantus_core::Ticks(bar * 8));
    app.engine
        .seek(cantus_core::Ticks(bar * 5 + cantus_core::PPQ * 2));
    let mut maximise = false;
    match view {
        "mixer" => {
            app.show_central(CentralView::Mixer);
            cantus_ui::views::mixer::set_width_mode(&mut app, WidthMode::Wide);
            maximise = true;
        }
        "pianoroll" => {
            app.piano_roll.key_height = 12.0;
            cantus_ui::views::piano_roll::fit_clip(&mut app, t, c);
            app.show_central(CentralView::PianoRoll);
        }
        _ => {}
    }
    let mut frames = 0;
    raster::shoot(&ctx, egui::vec2(1280.0, 720.0), 1.0, 8, None, |root| {
        frames += 1;
        if frames == 2 && maximise {
            app.workspace.toggle_fold(Pane::Arrangement);
        }
        app.frame(root);
    })
}

fn check(view: &str, preset: &str, dark: bool) {
    let _g = serial();
    let canvas = window(view, preset, dark);
    let name = format!("{view}-{preset}");
    let file = golden_dir().join(format!("{name}.png"));
    let png = gallery::encode_png(&canvas).expect("encode");
    if std::env::var_os("CANTUS_BLESS").is_some() {
        std::fs::create_dir_all(golden_dir()).expect("golden dir");
        std::fs::write(&file, &png).expect("write golden");
        return;
    }
    let bytes = std::fs::read(&file).unwrap_or_else(|_| {
        panic!(
            "{name}: no golden image at {} (bless with CANTUS_BLESS=1)",
            file.display()
        )
    });
    let (w, h, want) = gallery::decode_png(&bytes).expect("decode golden");
    let keep = |why: String| -> String {
        let out = actual_dir().join(format!("{name}-actual.png"));
        let _ = std::fs::write(&out, &png);
        format!("{name}: {why} (rendered: {})", out.display())
    };
    assert_eq!(
        (w, h),
        (canvas.width, canvas.height),
        "{}",
        keep(format!("the golden is {w}×{h}"))
    );
    let (mut different, mut worst) = (0usize, 0u8);
    for y in 0..h {
        for x in 0..w {
            let got = canvas.pixel(x, y);
            let exp = want[y * w + x];
            let d = (0..3).map(|c| got[c].abs_diff(exp[c])).max().unwrap_or(0);
            worst = worst.max(d);
            if d > CHANNEL_TOLERANCE {
                different += 1;
            }
        }
    }
    let share = different as f64 / (w * h) as f64;
    assert!(
        share <= MAX_DIFFERENT && worst <= HARD_LIMIT,
        "Slate's window changed: {}",
        keep(format!(
            "{different} pixels differ ({:.3} %), the worst by {worst}",
            share * 100.0
        ))
    );
}

#[test]
fn slate_night_arrangement_is_unchanged() {
    check("arrangement", "slate_night", true);
}

#[test]
fn slate_paper_arrangement_is_unchanged() {
    check("arrangement", "slate_paper", false);
}

#[test]
fn slate_night_mixer_is_unchanged() {
    check("mixer", "slate_night", true);
}

#[test]
fn slate_paper_mixer_is_unchanged() {
    check("mixer", "slate_paper", false);
}

#[test]
fn slate_night_piano_roll_is_unchanged() {
    check("pianoroll", "slate_night", true);
}

#[test]
fn slate_paper_piano_roll_is_unchanged() {
    check("pianoroll", "slate_paper", false);
}
