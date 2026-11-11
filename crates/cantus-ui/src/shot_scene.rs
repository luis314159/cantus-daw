//! A capture scene for the real application (round 37, finish; ADR 0103): with
//! `CANTUS_SHOT_SCENE=hero` the window sets itself up as the README's hero picture — the
//! house demo, the desk Extra large with the selected strip's cables, the browser open on a
//! folder of sounds, the whole song across the arrangement — so the blind comparisons with
//! FL Studio use screen captures of the real app, as FL's are, not the CPU renderer's
//! shots. `CANTUS_SHOT_THEME` picks the theme (Cantus Night by default). Nothing happens
//! without the variable. `tools/look/shots_of_scene.ps1` drives it. // r37:finish

use crate::app::CantusApp;
use crate::kit::appearance::Mode;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The frames the scene has seen.
static FRAME: AtomicUsize = AtomicUsize::new(0);
/// The real packs (r37:finish twelfth): 0 none asked, 1 installing, 2 shown.
static PACKS: AtomicUsize = AtomicUsize::new(0);

/// Cantus's own CC0 packs the hero shows, by id and archive name, read from the folder
/// `CANTUS_SHOT_PACKS` names (never from the repository: ADR 0030).
const PACK_ZIPS: [(&str, &str); 2] = [
    ("cantus-loops", "cantus-loops-1.zip"),
    ("cantus-one-shots", "cantus-one-shots-1.zip"),
];
/// The folder of the loops pack the hero opens, and the sound it chooses there.
const PACK_FOLDER: &str = "Bass";
const PACK_SOUND: &str = "Gravis - Acid 303 - 120 BPM.wav";

/// Install the packs whose archives `CANTUS_SHOT_PACKS` holds (each checked against the
/// manifest's SHA-256 as any install from a file is); whether any was started.
fn install_packs(app: &mut CantusApp) -> bool {
    let Some(dir) = std::env::var_os("CANTUS_SHOT_PACKS").map(std::path::PathBuf::from) else {
        return false;
    };
    let mut started = false;
    for (id, zip) in PACK_ZIPS {
        let path = dir.join(zip);
        if path.is_file() {
            started |= app.samples.content().install_from_file(id, &path);
        }
    }
    started
}

/// Once the loops pack is in: open it at its bass loops, one of them chosen.
fn show_packs(app: &mut CantusApp) -> bool {
    let c = app.samples.content();
    if !c.installed().iter().any(|r| r.id == PACK_ZIPS[0].0) {
        return false;
    }
    let root = c.pack_dir(PACK_ZIPS[0].0);
    app.samples.open.insert(root.clone());
    app.samples.open.insert(root.join(PACK_FOLDER));
    app.samples.selected = Some(root.join(PACK_FOLDER).join(PACK_SOUND));
    app.samples.refresh();
    true
}

/// The scene asked for, read once (the frame allocates nothing for it).
fn wanted() -> Option<&'static str> {
    static W: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    W.get_or_init(|| {
        std::env::var("CANTUS_SHOT_SCENE")
            .ok()
            .filter(|s| !s.is_empty())
    })
    .as_deref()
}

/// Called once a frame from the window's frame: a step of the scene on its frame.
pub fn tick(app: &mut CantusApp, ctx: &egui::Context) {
    if wanted() != Some("hero") {
        return;
    }
    let f = FRAME.fetch_add(1, Ordering::Relaxed);
    if f < 90 || PACKS.load(Ordering::Relaxed) == 1 {
        // An idle window does not repaint: the scene's frames are asked for (and while the
        // packs install).
        ctx.request_repaint();
    }
    if f >= 40 && PACKS.load(Ordering::Relaxed) == 1 && show_packs(app) {
        PACKS.store(2, Ordering::Relaxed);
    }
    // The packs' "installed" notices are the install's, not the picture's: the foot keeps
    // the chosen sound's preview strip.
    if PACKS.load(Ordering::Relaxed) == 2 {
        app.samples.notice = None;
    }
    // A dropout while the release starts (its first frames' work) is not the picture's.
    if f >= 30 && app.status.error {
        app.status.error = false;
        app.status.text.clear();
    }
    match f {
        // The window's first frames settle its size; the scene starts after them.
        // Maximised first (the capture is the screen's), then set up.
        5 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true)),
        30 => {
            setup(app);
            if install_packs(app) {
                PACKS.store(1, Ordering::Relaxed);
            }
        }
        40 => {
            app.toggle_pane(crate::views::workspace::Pane::Browser);
            // r37:finish — the selected strip's inserts are the desk's own panel: the
            // inspector closed, the desk the window's width.
            if app
                .workspace
                .is_visible(crate::views::workspace::Pane::Inspector)
            {
                app.toggle_pane(crate::views::workspace::Pane::Inspector);
            }
            // The real packs when they are installing; the synthesized folder without them.
            if PACKS.load(Ordering::Relaxed) == 0 {
                let dir = demo_sounds();
                app.settings.sample_folders.push(dir.clone());
                app.samples.open.insert(dir.clone());
                app.samples.refresh();
                app.samples.selected = Some(dir.join("Chord Stab Am.wav"));
            }
        }
        _ => {}
    }
    // The browser 366 points wide (458 px at 125 %, as wide as FL's; r37:finish thirteenth): its own split's fraction from that
    // split's width, on every frame from the 50th (the dock's tree is not the same on the
    // frame the browser opens, a measure a frame old overshot, and the window may be
    // maximised again after the scene's frames).
    if f >= 50 {
        size_browser(app);
    }
    // Where each area is, in pixels, for the judge's crops (`judge_real.py`): written
    // beside the data on every frame whose layout differs from the last one written, so
    // the file is always the last frame's (r37:finish thirteenth: written every fourth
    // frame, a Night capture kept a transient layout — the browser half the window —
    // when the window's last frames after its final maximise were fewer than four).
    if f >= 50 {
        write_layout(app, ctx);
    }
    if f == 50 {
        // The arrangement tall enough for every track of the song (the window is the
        // screen's, a little shorter than the README's 1080).
        app.workspace.impose_sizes(330.0, 360.0);
    }
}

fn setup(app: &mut CantusApp) {
    // The first start's audio questions and the verse: not part of the picture.
    app.audio_setup.first_run = None;
    app.verse = None;
    app.workflow.start.close();
    let preset = std::env::var("CANTUS_SHOT_THEME").unwrap_or_else(|_| "slate_night".into());
    let dark = crate::kit::appearance::PRESETS
        .iter()
        .find(|(id, _)| *id == preset)
        .is_none_or(|(_, theme)| theme().dark);
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.apply_appearance();
    if let Some(demo) = crate::demos::find("house") {
        app.replace_project((demo.build)());
    }
    // The strips nobody named and nothing fills, as the README's shot leaves them out.
    let empty: Vec<_> = app
        .project()
        .strips
        .iter()
        .filter(|s| s.name.is_empty() && s.effects.is_empty())
        .map(|s| s.id)
        .collect();
    for id in empty {
        app.exec(cantus_core::Command::RemoveStrip { id });
    }
    if let Some(strip) = app.project().tracks.get(5).and_then(|t| t.strip) {
        app.select_strip(strip);
    }
    use crate::views::mixer::layout::{CableView, WidthMode};
    crate::views::mixer::set_width_mode(app, WidthMode::from_index(2));
    let mut prefs = app.mixer.prefs();
    prefs.cables = CableView::Selected;
    // r37:finish — the selected strip's inserts as a panel at the desk's right.
    prefs.inserts = true;
    // r37:finish twelfth — the faders their longest throw (judge: short faders): the
    // meters' row its least.
    prefs.ratio = 0.0;
    // The FX and record keys' row and the sends' rows hidden: the room goes to the faders.
    {
        use crate::views::mixer::layout::Row;
        prefs.rows.set(Row::FxRec, false);
        prefs.rows.set(Row::SendList, false);
    }
    app.mixer.set_prefs(prefs);
    let beats = app.project().content_end().0 as f32 / cantus_core::Ticks::beats(1).0 as f32;
    app.arrangement.px_per_beat = 1060.0 / beats.max(1.0);
    app.arrangement.scroll_beats = 0.0;
    app.arrangement.track_height = 26.0;
    // Room for every track's name whole beside its keys at the screen's 125 %.
    app.arrangement.header_width = 240.0;
    app.settings.stage.hidden = true;
    app.seek(cantus_core::Ticks::beats(22 * 4));
}

/// A folder of short sounds for the hero's browser, written once into the temp folder:
/// drum hits and a few one-shots, synthesised (a decaying tone, filtered noise), 16-bit.
/// // r37:finish
pub fn demo_sounds() -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("cantus-shoot-{}", std::process::id()))
        .join("Drums and one-shots");
    let _ = std::fs::create_dir_all(&dir);
    // (name, start pitch in Hz, end pitch, noise share, length in seconds)
    let sounds: [(&str, f32, f32, f32, f32); 10] = [
        ("Kick 808 Deep.wav", 120.0, 42.0, 0.02, 0.9),
        ("Kick Punch.wav", 160.0, 55.0, 0.05, 0.4),
        ("Snare Tight.wav", 220.0, 180.0, 0.7, 0.3),
        ("Clap Wide.wav", 900.0, 900.0, 0.95, 0.35),
        ("Hat Closed.wav", 6000.0, 6000.0, 1.0, 0.08),
        ("Hat Open.wav", 6000.0, 6000.0, 1.0, 0.45),
        ("Drum Loop 124 BPM.wav", 140.0, 60.0, 0.45, 1.9),
        ("Pad Swell F#m.wav", 185.0, 185.0, 0.05, 2.6),
        ("Bass Stab C.wav", 65.4, 65.4, 0.0, 0.7),
        ("Chord Stab Am.wav", 220.0, 220.0, 0.0, 0.8),
    ];
    let rate = 44_100u32;
    for (name, f0, f1, noise, len) in sounds {
        let path = dir.join(name);
        if path.exists() {
            continue;
        }
        let n = (len * rate as f32) as usize;
        let mut pcm = Vec::with_capacity(n);
        let mut phase = 0.0f32;
        let mut seed = 0x2545_f491_u32;
        let mut lp = 0.0f32;
        for i in 0..n {
            let t = i as f32 / n as f32;
            let f = f0 + (f1 - f0) * (1.0 - (1.0 - t) * (1.0 - t));
            phase = (phase + f / rate as f32).fract();
            // A triangle (no transcendental), noise from a xorshift, a linear-squared decay.
            let tone = 1.0 - 4.0 * (phase - 0.5).abs();
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            lp += (white - lp) * 0.35;
            // r37:finish — envelopes as such sounds have them (judge 9: the thumbnails were
            // wedges): a drum's sharp hit and long tail, a loop's hits on the beat, a pad's
            // swell, a stab's quick attack and release.
            let secs = i as f32 / rate as f32;
            let tail = |x: f32| {
                let d = (1.0 - x).clamp(0.0, 1.0);
                let d2 = d * d;
                d2 * d2 * d
            };
            let env = if name.starts_with("Drum Loop") {
                let beat = 60.0 / 124.0 / 2.0;
                let k = (secs / beat) as usize;
                let x = (secs - k as f32 * beat) / beat;
                tail(x) * if k.is_multiple_of(2) { 1.0 } else { 0.6 }
            } else if name.starts_with("Pad") {
                let up = (t / 0.45).min(1.0);
                up * up * (1.0 - t * t) * (0.85 + 0.15 * tone)
            } else if name.contains("Stab") {
                (secs * 120.0).min(1.0) * (1.0 - t) * (1.0 - t * 0.5)
            } else {
                tail(t)
            };
            let v = (tone * (1.0 - noise) + lp * noise) * env * 0.8;
            pcm.push((v.clamp(-1.0, 1.0) * 32_000.0) as i16);
        }
        let data_len = (pcm.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + pcm.len() * 2);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for v in pcm {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let _ = std::fs::write(&path, out);
    }
    dir
}

fn size_browser(app: &mut CantusApp) {
    let Some(p) = app
        .workspace
        .find(crate::views::workspace::Pane::Browser)
        .filter(|p| p.surface.is_main())
    else {
        return;
    };
    let node = p.node;
    let Some(parent) = node.parent() else {
        return;
    };
    let main = app.workspace.dock.main_surface_mut();
    if let egui_dock::Node::Horizontal(split) = &mut main[parent] {
        let w = split.rect.width();
        if w.is_finite() && w > 400.0 {
            let f = (366.0 / w).clamp(0.05, 0.6);
            split.fraction = if node.is_left() { f } else { 1.0 - f };
        }
    }
}

/// `shot-layout.json` in the data folder: each area's rect in the window's pixels.
fn write_layout(app: &CantusApp, ctx: &egui::Context) {
    use crate::views::workspace::Pane;
    let Ok(dir) = std::env::var("CANTUS_DATA_DIR") else {
        return;
    };
    let ppp = ctx.pixels_per_point();
    let mut parts = Vec::new();
    for (name, pane) in [
        ("browser", Pane::Browser),
        ("arrangement", Pane::Arrangement),
        ("mixer", Pane::Mixer),
        ("inspector", Pane::Inspector),
    ] {
        let rect = app
            .workspace
            .find(pane)
            .filter(|p| p.surface.is_main())
            .and_then(|p| app.workspace.dock.leaf(p.node_path()).ok())
            .map(|l| l.rect);
        if let Some(r) = rect.filter(|r| r.is_positive()) {
            parts.push(format!(
                "\"{name}\": [{:.0}, {:.0}, {:.0}, {:.0}]",
                r.left() * ppp,
                r.top() * ppp,
                r.right() * ppp,
                r.bottom() * ppp
            ));
        }
    }
    let json = format!(
        "{{{}}}
",
        parts.join(", ")
    );
    static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    // The scene's step runs before the dock lays out, so the rects read are the frame
    // before's: a change asks for one more frame, which writes the settled layout (the
    // Night capture's file held the frame before the last maximise's).
    if write_if_changed(std::path::Path::new(&dir), json, &LAST) {
        ctx.request_repaint();
    }
}

/// Write `json` to `dir/shot-layout.json` when it differs from what `last` holds (the last
/// written); whether it wrote.
fn write_if_changed(dir: &std::path::Path, json: String, last: &std::sync::Mutex<String>) -> bool {
    let Ok(mut last) = last.lock() else {
        return false;
    };
    if *last == json {
        return false;
    }
    let wrote = std::fs::write(dir.join("shot-layout.json"), &json).is_ok();
    if wrote {
        *last = json;
    }
    wrote
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout file is the last frame's, whatever the frames' count: a changed layout
    /// is written at once, an unchanged one not again (the thirteenth delivery's crop bug:
    /// written every fourth frame, a Night capture kept a transient layout).
    #[test]
    fn the_layout_file_follows_every_change() {
        let dir = tempfile::tempdir().unwrap();
        let last = std::sync::Mutex::new(String::new());
        let read = || std::fs::read_to_string(dir.path().join("shot-layout.json")).unwrap();
        assert!(write_if_changed(
            dir.path(),
            "{\"browser\": [0, 82, 1108, 965]}
"
            .into(),
            &last
        ));
        assert!(!write_if_changed(
            dir.path(),
            "{\"browser\": [0, 82, 1108, 965]}
"
            .into(),
            &last
        ));
        assert!(write_if_changed(
            dir.path(),
            "{\"browser\": [0, 82, 458, 965]}
"
            .into(),
            &last
        ));
        assert!(
            read().contains("458"),
            "the last layout on disk: {}",
            read()
        );
    }
}
