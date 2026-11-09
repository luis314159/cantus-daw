//! What turning each of Chorda's controls costs the interface (round 38, the owner's report
//! "some of Chorda's knobs stick or are slow"): a Chorda added the way the picker adds one,
//! its editor open in the Play view at 1920×1080 pixels and 150 %, each control dragged
//! across its range at a hand's pace (frames every 16.7 ms), the scene's worker on its own
//! thread as in the app. Prints each control's frame times. Release only:
//! `cargo test --release -p cantus-ui --test chorda_knobs_measure -- --ignored --nocapture`.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

const PPP: f32 = 1.5;
const SIZE: egui::Vec2 = egui::vec2(1280.0, 720.0);

/// One frame of the app; its time (ms) on this thread, tessellation included.
fn frame(ctx: &egui::Context, app: &mut CantusApp, t: f64) -> f64 {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(t),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let t0 = Instant::now();
    let out = ctx.run_ui(input, |root| app.frame(root));
    let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
    t0.elapsed().as_secs_f64() * 1e3
}

fn stats(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    (q(0.5), q(0.95), *v.last().unwrap())
}

#[test]
#[ignore]
fn measure_turning_each_control() {
    let _ = data();
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(PPP);
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let track = app
        .project()
        .tracks
        .iter()
        .rev()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .unwrap();
    app.inst.view = cantus_ui::views::inst::View::Play;
    app.open_instrument_window(track);
    let mut t = 0.0;
    let tick = |app: &mut CantusApp, t: &mut f64| {
        let start = Instant::now();
        *t += 1.0 / 60.0;
        let ms = frame(&ctx, app, *t);
        let left = Duration::from_secs_f64(1.0 / 60.0).saturating_sub(start.elapsed());
        std::thread::sleep(left);
        ms
    };
    // Settle: the window laid out, the first bake landed.
    for _ in 0..240 {
        tick(&mut app, &mut t);
    }
    let controls: &[(&str, f32, f32)] = &[
        ("hardness", -1.0, 1.0),
        ("dynamics", 0.5, 1.5),
        ("sympathy", 0.0, 2.0),
        ("body", 0.0, 2.0),
        ("tilt", -12.0, 12.0),
        ("gain", -24.0, 6.0),
        ("pedal", 0.0, 1.0),
        ("persp", 0.0, 1.0),
        ("width", 0.0, 1.0),
        ("lid", 0.0, 1.0),
        ("room", 0.0, 1.0),
        ("roomsize", 0.0, 1.0),
        ("decay", 0.25, 4.0),
        ("hysteresis", 0.0, 0.98),
        ("phantom", 0.0, 3.0),
        ("coupling", 0.0, 0.9),
        ("unison", 0.0, 8.0),
        ("a4", 415.0, 466.0),
        ("stretch", 0.0, 2.0),
        ("stiffness", 0.5, 1.5),
    ];
    eprintln!("control      drag: median  p95   max (ms)   then rest: median  p95   max");
    for (key, lo, hi) in controls {
        let mut drag = Vec::new();
        // 1.5 s across the range and back, a value each frame, as a drag sends.
        for i in 0..90 {
            let u = i as f32 / 89.0;
            let u = if u < 0.5 { 2.0 * u } else { 2.0 - 2.0 * u };
            app.set_inst_value(track, key, lo + (hi - lo) * u, false);
            drag.push(tick(&mut app, &mut t));
        }
        let rest: Vec<f64> = (0..90).map(|_| tick(&mut app, &mut t)).collect();
        let (a, b, c) = stats(drag);
        let (d, e, f) = stats(rest);
        eprintln!("{key:12} {a:12.1} {b:5.1} {c:6.1}   {d:17.1} {e:5.1} {f:6.1}");
    }
}

/// The audio side: a chord held on Chorda (pedal down), each control turned a value per
/// 512-sample block (a drag's pace), every block's processing time on this thread — a
/// block must finish well inside its 10.7 ms.
#[test]
#[ignore]
fn measure_the_engine_while_turning_each_control() {
    let _ = data();
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let (engine, mut proc_) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let track = app
        .project()
        .tracks
        .iter()
        .rev()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .unwrap();
    app.sync_engine();
    proc_.settle();
    let peak = std::cell::Cell::new(0f32);
    let mut buf = vec![0f32; 1024];
    let mut block = |proc_: &mut cantus_engine::Processor| {
        buf.fill(0.0);
        let t0 = Instant::now();
        proc_.process(&mut buf, 2);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        peak.set(buf.iter().fold(peak.get(), |m, x| m.max(x.abs())));
        ms
    };
    // The keyboard designed (on its own thread: wait in real time until a note sounds).
    let start = Instant::now();
    loop {
        app.engine.note_on(track, 60, 100);
        app.sync_engine();
        for _ in 0..20 {
            block(&mut proc_);
        }
        app.engine.note_off(track, 60);
        if peak.get() > 1e-3 || start.elapsed().as_secs() > 60 {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(peak.get() > 1e-3, "Chorda sounds");
    eprintln!("Chorda sounds after {:.1} s", start.elapsed().as_secs_f64());
    let controls: &[(&str, f32, f32)] = &[
        ("hardness", -1.0, 1.0),
        ("sympathy", 0.0, 2.0),
        ("body", 0.0, 2.0),
        ("tilt", -12.0, 12.0),
        ("gain", -24.0, 6.0),
        ("persp", 0.0, 1.0),
        ("width", 0.0, 1.0),
        ("lid", 0.0, 1.0),
        ("room", 0.0, 1.0),
        ("roomsize", 0.0, 1.0),
        ("decay", 0.25, 4.0),
        ("phantom", 0.0, 3.0),
        ("coupling", 0.0, 0.9),
        ("unison", 0.0, 8.0),
        ("a4", 415.0, 466.0),
        ("stretch", 0.0, 2.0),
        ("stiffness", 0.5, 1.5),
    ];
    eprintln!(
        "control      blocks while turning: median  p95   max (ms)   then: median  p95   max"
    );
    for (key, lo, hi) in controls {
        for p in [36u8, 48, 60, 64, 67, 72, 79, 84] {
            app.engine.note_on(track, p, 100);
        }
        let mut turning = Vec::new();
        for i in 0..90 {
            let u = i as f32 / 89.0;
            let u = if u < 0.5 { 2.0 * u } else { 2.0 - 2.0 * u };
            app.set_inst_value(track, key, lo + (hi - lo) * u, false);
            app.sync_engine();
            turning.push(block(&mut proc_));
        }
        let after: Vec<f64> = (0..200).map(|_| block(&mut proc_)).collect();
        for p in [36u8, 48, 60, 64, 67, 72, 79, 84] {
            app.engine.note_off(track, p);
        }
        for _ in 0..100 {
            block(&mut proc_);
        }
        let (a, b, c) = stats(turning);
        let (d, e, f) = stats(after);
        eprintln!(
            "{key:12} {a:26.2} {b:5.2} {c:6.2}   {d:13.2} {e:5.2} {f:6.2}   peak {:.3}",
            peak.get()
        );
        peak.set(0.0);
    }
}

/// The scene's bake at each quality and resolution (for choosing the glide's preview).
#[test]
#[ignore]
fn measure_the_bake_tiers() {
    use cantus_ui::views::inst::chorda::scene::{self, Quality};
    let spec = cantus_core::NativeSpec::new(InstrumentKind::Chorda);
    let i = scene::Inputs::of(&spec, true);
    let size = egui::vec2(710.0, 300.0);
    for ppp in [1.0f32, 1.5] {
        for (q, name) in [
            (Quality { ss: 1, full: false }, "ss1 plain"),
            (Quality { ss: 2, full: false }, "ss2 plain"),
            (Quality { ss: 1, full: true }, "ss1 full"),
            (Quality { ss: 2, full: true }, "ss2 full"),
        ] {
            for (scale, label) in [(0.5f32, "half"), (1.0, "whole")] {
                let mut best = f64::MAX;
                for _ in 0..3 {
                    let t0 = Instant::now();
                    let img = scene::render(
                        &i,
                        size,
                        ppp * scale,
                        q,
                        std::env::var("T").map_or(7, |t| t.parse().unwrap()),
                        16_384,
                    );
                    best = best.min(t0.elapsed().as_secs_f64() * 1e3);
                    std::hint::black_box(img);
                }
                eprintln!("ppp {ppp} {label:5} {name:9}: {best:6.1} ms");
            }
        }
    }
}

/// The preview before and after round 38, as raw RGBA for a side-by-side picture.
#[test]
#[ignore]
fn dump_previews() {
    use cantus_ui::views::inst::chorda::scene::{self, PREVIEW};
    let mut spec = cantus_core::NativeSpec::new(InstrumentKind::Chorda);
    spec.set("lid", 0.5);
    let i = scene::Inputs::of(&spec, true);
    let size = egui::vec2(710.0, 300.0);
    for (scale, name) in [(0.5f32, "old"), (0.68, "new")] {
        let img = scene::render(&i, size, 1.5 * scale, PREVIEW, 7, 16_384);
        let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        let p = format!(
            "F:/projects/daw-r38/shots/preview-{name}-{}x{}.rgba",
            img.size[0], img.size[1]
        );
        std::fs::write(p, bytes).unwrap();
    }
    for (scale, name) in [(0.5f32, "ss2half"), (0.6, "ss2six"), (1.0, "ss2whole")] {
        let t0 = Instant::now();
        let img = scene::render(
            &i,
            size,
            1.5 * scale,
            scene::Quality { ss: 2, full: false },
            7,
            16_384,
        );
        eprintln!("{name}: {:.0} ms", t0.elapsed().as_secs_f64() * 1e3);
        let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        let p = format!(
            "F:/projects/daw-r38/shots/preview-{name}-{}x{}.rgba",
            img.size[0], img.size[1]
        );
        std::fs::write(p, bytes).unwrap();
    }
    let img = scene::render(&i, size, 1.5, scene::FULL, 7, 16_384);
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    std::fs::write(
        format!(
            "F:/projects/daw-r38/shots/preview-full-{}x{}.rgba",
            img.size[0], img.size[1]
        ),
        bytes,
    )
    .unwrap();
}

/// The bakes' bits, to show a faster baker draws the same picture.
#[test]
#[ignore]
fn hash_the_bakes() {
    use cantus_ui::views::inst::chorda::scene::{self, Quality};
    use std::hash::{Hash, Hasher};
    let mut spec = cantus_core::NativeSpec::new(InstrumentKind::Chorda);
    for (lid, room, width) in [(1.0, 0.5, 0.5), (0.4, 0.9, 0.2), (0.0, 0.1, 1.0)] {
        spec.set("lid", lid);
        spec.set("room", room);
        spec.set("width", width);
        for dark in [true, false] {
            let i = scene::Inputs::of(&spec, dark);
            for (q, threads) in [
                (Quality { ss: 1, full: false }, 7),
                (Quality { ss: 2, full: false }, 3),
                (Quality { ss: 2, full: true }, 7),
                (Quality { ss: 2, full: true }, 1),
            ] {
                let img = scene::render(&i, egui::vec2(710.0, 300.0), 1.5, q, threads, 16_384);
                let mut h = std::collections::hash_map::DefaultHasher::new();
                img.pixels.hash(&mut h);
                eprintln!(
                    "lid {lid} room {room} width {width} dark {dark} {q:?} t{threads}: {:016x}",
                    h.finish()
                );
            }
        }
    }
}
