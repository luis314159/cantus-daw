//! A dancer on screen (round 35, ADR 0091 §1): the frame budget held with one floating over
//! the whole window while the song plays — the same rig as the rest of this file (the real
//! application, its processor run for a 60-Hz block before every frame), measured with and
//! without the dancer: a track added to the 16-strip desk, a dancer on it or not, its unit
//! in the engine and its feed read by the interface (`crate::dance_live`).

use super::*;
use cantus_ui::stage::Surface;

/// The desk with a track added: a plain one (`None`), or a dancer shown as `view.mode` says.
fn rig(dancer_mode: Option<f32>) -> (Rig, cantus_core::TrackId) {
    use cantus_core::{Command, InstrumentKind, InstrumentSpec, NativeSpec};
    let mut rig = Rig::new(16, true, HD);
    rig.app.add_track();
    let track = rig.app.project().tracks.last().unwrap().id;
    if let Some(mode) = dancer_mode {
        assert!(rig.app.exec(Command::SetInstrument {
            track,
            spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
                InstrumentKind::Dancer
            ))),
        }));
        rig.app.set_inst_value(track, "x", 0.82, false);
        rig.app.set_inst_value(track, "y", 0.92, false);
        rig.app.set_inst_value(track, "view.mode", mode, false);
    }
    rig.app.sync_engine();
    rig.play();
    (rig, track)
}

/// Deterministic: what the stage adds to a frame for a dancer on screen, against the same
/// dancer hidden — playing, one texture redrawn and uploaded a frame, one more draw call, a
/// handful of triangles; stopped, on the frames between two breaths (no redraw), next to
/// nothing. (What a dancer track costs the rest of the window is printed beside it.)
#[test]
fn a_dancer_on_screen_stays_in_budget() {
    let plain = rig(None).0.run(20, 30);
    let base = rig(Some(2.0)).0.run(20, 30);
    let _ = summary("mixer16 and a plain track, whole window", &plain);
    let (mut with, track) = rig(Some(0.0));
    let frames = with.run(20, 30);
    let (a0, b0, t0, d0, _) = summary("mixer16, whole window, a dancer hidden", &base);
    let (a1, b1, t1, d1, _) = summary("mixer16, whole window, a dancer", &frames);
    assert_eq!(with.app.stage.textures(), 1, "one texture for the dancer");
    let p = with
        .app
        .stage
        .pixmap(track, Surface::Float)
        .expect("the dancer is drawn");
    let px = u64::from(p.width()) * u64::from(p.height());
    // What drawing the character alone allocates, on a pixmap of that size.
    let (drawing, drawn_bytes) = {
        let d = &with.app.stage.dancers()[0];
        let stage = cantus_dance::render::Stage::new(
            p.width(),
            p.height(),
            cantus_dance::render::Background::Transparent,
        );
        let mut pm = tiny_skia::Pixmap::new(p.width(), p.height()).unwrap();
        let (_, n, bytes) = counted(|| {
            cantus_dance::render::render_into(&mut pm, &stage, &[(d.character.as_ref(), d.pose)])
        });
        (n, bytes)
    };
    let redraws = with.app.stage.redraws;
    eprintln!(
        "a dancer playing: {px} pixels a redraw, {redraws} redraws in 50 frames; +{} allocations ({drawing} drawing the character, {} KB), +{} KB, +{} triangles, +{} draw calls a frame",
        a1.saturating_sub(a0),
        drawn_bytes / 1024,
        b1.saturating_sub(b0) / 1024,
        t1.saturating_sub(t0),
        d1.saturating_sub(d0)
    );
    assert!(
        (20..=30).contains(&redraws),
        "playing, it is drawn 30 times a second (frames come at 60): {redraws} in 50 frames"
    );
    // Beyond what drawing the character costs (its paths: the cast's and the renderer's),
    // the stage's own share.
    assert!(
        a1 <= a0 + drawing + PLAYING_ALLOCS,
        "{a1} allocations against {a0} (+{drawing} drawing)"
    );
    // The texture's pixels (egui takes an owned image), drawing's scratch, little else.
    assert!(
        b1 <= b0 + px * 4 + drawn_bytes + 192 * 1024,
        "{b1} bytes against {b0}"
    );
    assert!(d1 <= d0 + 2, "{d1} draw calls against {d0}");
    assert!(t1 <= t0 + 64, "{t1} triangles against {t0}");

    // Stopped: the frames on which it does not redraw, against the window without it.
    let (mut quiet, _) = rig(Some(2.0));
    quiet.stop();
    let q0: Vec<u64> = quiet.run(20, 30).iter().map(|f| f.allocs).collect();
    with.stop();
    with.run(10, 0);
    let mut still = Vec::new();
    for _ in 0..60 {
        let before = with.app.stage.redraws;
        let f = with.step();
        if with.app.stage.redraws == before {
            still.push(f.allocs);
        }
    }
    assert!(
        still.len() >= 30,
        "stopped, most frames do not redraw ({})",
        still.len()
    );
    let (q, s) = (median(q0), median(still));
    eprintln!("a dancer stopped, between breaths: {s} allocations against {q}");
    assert!(s <= q + STILL_ALLOCS, "{s} allocations against {q}");
}

/// What one dancer may add to a frame's allocations while the song plays, beyond drawing
/// the character: its image and texture delta, its area's layer and widgets.
const PLAYING_ALLOCS: u64 = 60;
/// What it may add on a frame that does not redraw it: its area's layer and widgets.
const STILL_ALLOCS: u64 = 40;

/// `cargo test --release -p cantus-ui --test frame_budget -- --ignored --nocapture dancer`
#[test]
#[ignore = "timing: run in release with --ignored"]
fn a_dancer_on_screen_timing() {
    let (mut with, _) = rig(Some(0.0));
    timing("mixer16 + a dancer, whole window", &mut with, 2.5);
    let st = &with.app.stage;
    let mean = st.draw_seconds / st.redraws.max(1) as f64 * 1e3;
    eprintln!(
        "the dancer: {} redraws, {mean:.3} ms a redraw (mean), {} pixels each",
        st.redraws,
        st.drawn_px / st.redraws.max(1)
    );
    // The contract's measured budget for one character on a 600 × 600 stage, which this
    // box is well under.
    assert!(mean <= 1.5, "{mean:.3} ms a redraw");
}

/// Where a hidden dancer track costs the window anything: each view alone, the desk with a
/// plain track added against the same desk with that track a hidden dancer (printed for
/// the report; it fails only if the stage itself draws a hidden dancer).
#[test]
fn where_a_hidden_dancer_costs() {
    let measure = |rig: &mut Rig, view: &str| -> (u64, usize) {
        let mut tris = Vec::new();
        let mut allocs = Vec::new();
        for i in 0..12 {
            rig.processor.process(&mut rig.audio, 2);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, HD)),
                time: Some(i as f64 / 60.0),
                ..Default::default()
            };
            let app = &mut rig.app;
            let (out, n, _) = counted(|| {
                rig.ctx.run_ui(input, |root| {
                    egui::CentralPanel::default().show(root, |ui| match view {
                        "arrangement" => views::arrangement::show(app, ui),
                        "mixer" => views::mixer::show(app, ui),
                        "inspector" => views::inspector::show(app, ui),
                        "rack" => views::channel_rack::show(app, ui),
                        _ => app.frame(ui),
                    });
                })
            });
            let mut out = out;
            let prims = rig
                .ctx
                .tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
            out.textures_delta.clear();
            let t: usize = prims
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                    _ => 0,
                })
                .sum();
            if i >= 6 {
                tris.push(t);
                allocs.push(n);
            }
        }
        (median(allocs), tris.into_iter().max().unwrap_or(0))
    };
    for view in ["arrangement", "mixer", "inspector", "rack", "window"] {
        let (mut plain, _) = rig(None);
        let (mut hidden, _) = rig(Some(2.0));
        let (a0, t0) = measure(&mut plain, view);
        let (a1, t1) = measure(&mut hidden, view);
        eprintln!(
            "{view}: a hidden dancer track instead of a plain one: {:+} allocations, {:+} triangles",
            a1 as i64 - a0 as i64,
            t1 as i64 - t0 as i64
        );
        if view == "window" {
            let tabs: Vec<String> = hidden
                .app
                .workspace
                .dock
                .iter_all_tabs()
                .map(|(_, p)| format!("{p:?}"))
                .collect();
            eprintln!("the window's tabs with the hidden dancer: {tabs:?}");
            assert_eq!(
                hidden.app.stage.textures(),
                0,
                "the stage draws no hidden dancer"
            );
        }
    }
}
