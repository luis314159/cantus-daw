//! What every whole-window picture shares (round 39, `shots`; `tools/shots/reshoot.sh`).
//! Included by the picture examples with `#[path = "shots_common/studio.rs"] mod studio;`
//! (a folder without `main.rs` is no example of its own).

// Not every example that includes this file calls everything in it.
#![allow(dead_code)]

use cantus_ui::CantusApp;

/// The interface every picture shows: the audio setup's demo, an ASIO driver at 48 kHz.
pub const DEVICE: &str = "ASIO4ALL v2";

/// A working studio, as on a machine with an interface plugged in, so no picture says
/// "No audio": the audio setup's demo interface (`DEVICE` at 48 kHz and `block` samples),
/// named by its driver as an unlisted interface is. And no dropout on the status line: a
/// headless shot runs the engine on a machine that may be busy, and a block that missed its
/// deadline there is not part of the picture. Call it after `attach`.
pub fn studio(app: &mut CantusApp, block: u32) {
    cantus_ui::audio_setup::demo::setup(app);
    if let Some(live) = app.audio_setup.demo.as_mut() {
        live.driver = "ASIO".into();
        live.device = DEVICE.into();
        live.wanted = live.device.clone();
        live.block = block;
    }
    app.audio_setup.scan = None;
    app.settings.audio.device = Some(DEVICE.into());
    cantus_ui::views::transport::perf::hush_dropouts(app);
    // The dummy engine's "Audio device unavailable" line, said before the demo interface
    // was there, off the status bar.
    app.set_status(String::new());
}

/// The rect of a window or area drawn last frame (Settings is `"cantus-settings"`), in points.
pub fn area(ctx: &egui::Context, id: &str) -> Option<egui::Rect> {
    ctx.memory(|m| m.area_rect(egui::Id::new(id)))
}

/// The rects a picture is cut from, beside its PNG as `<png>.json`: `{"name": [x, y, w, h]}`
/// in the PNG's pixels (points × `ppp`, rounded outwards, clamped to `size`).
/// `tools/shots/place.py` reads them, so a crop follows the layout when a panel changes
/// shape. A rect that was not drawn is left out, and place.py says so.
pub fn sidecar(
    png: &std::path::Path,
    size: egui::Vec2,
    ppp: f32,
    rects: &[(&str, Option<egui::Rect>)],
) {
    let mut out = serde_json::Map::new();
    for (name, r) in rects {
        let Some(r) = r else {
            eprintln!("{}: no rect for {name}", png.display());
            continue;
        };
        let r = r.intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
        let x0 = (r.left() * ppp).floor().max(0.0);
        let y0 = (r.top() * ppp).floor().max(0.0);
        let x1 = (r.right() * ppp).ceil().min(size.x * ppp);
        let y1 = (r.bottom() * ppp).ceil().min(size.y * ppp);
        out.insert(
            (*name).to_string(),
            serde_json::json!([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32]),
        );
    }
    let path = png.with_extension("png.json");
    let text = serde_json::to_string_pretty(&out).expect("the rects");
    std::fs::write(&path, text).expect("the sidecar is written");
}
