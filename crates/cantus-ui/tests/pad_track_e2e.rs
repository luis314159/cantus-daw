//! r40:pads — a pad's own mixer track end to end (ADR 0117), the way a user reaches it: the
//! app made by `CantusApp::assemble` with the engine's handle, the channel rack in front, a
//! click on the kick's well and a click on *First free strip* in the list it opens. The kick
//! then plays there and only there (the song rendered through the engine's compile, the
//! path export, freeze and stems take), the well reads the new strip's number, the step is
//! one undo away, and the choice is saved and read back. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{render_stems, EngineHandle, RenderOptions, StemOptions, StemSource, StemSpec};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::CantusApp;

fn song() -> Project {
    let mut p = Project::new("Pads");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let pitches: Vec<u8> = t
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .map(|x| x.pitch)
        .collect();
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        for k in 0..1 + (step % 3) {
            let pitch = pitches[((step * 5 + k * 3) as usize) % pitches.len()];
            c.notes
                .push(p.make_note(at, Ticks(PPQ / 3), pitch, 50 + (step * 13 % 70) as u8));
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn boot(dir: &std::path::Path) -> (CantusApp, egui::Context, cantus_engine::Processor) {
    std::fs::write(
        dir.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
    )
    .unwrap();
    std::env::set_var("CANTUS_DATA_DIR", dir);
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(song()), engine, None, None, None, false);
    (app, ctx, proc_)
}

static FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn frame(
    app: &mut CantusApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1800.0),
        )),
        time: Some(FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as f64 / 60.0),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |root| app.frame(root));
    out.textures_delta.clear();
    out.shapes
}

fn click(app: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(app, ctx, vec![egui::Event::PointerMoved(at)]);
    let _ = frame(app, ctx, vec![press(true)]);
    let _ = frame(app, ctx, vec![press(false)]);
}

/// Every text a frame painted, with where.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((
                t.galley.job.text.clone(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// The stems of the desk's strips `strips` of the app's song, through the engine's compile.
fn stems(app: &mut CantusApp, p: &Project, strips: &[usize]) -> Vec<Vec<f32>> {
    app.sync_engine();
    let snap = app.engine.compile_for_render(p, 1);
    let specs: Vec<StemSpec> = strips
        .iter()
        .map(|i| StemSpec {
            source: StemSource::Strip(*i),
            file_name: format!("s{i}"),
        })
        .collect();
    let opts = StemOptions {
        render: RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
        ..Default::default()
    };
    render_stems(snap, &opts, &specs)
        .unwrap()
        .into_iter()
        .map(|s| s.samples)
        .collect()
}

#[test]
fn the_kicks_well_sends_it_to_a_free_strip_where_alone_it_plays() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, ctx, _proc) = boot(dir.path());
    let t = app.project().tracks[0].id;
    let c = app.project().tracks[0].clips[0].id;
    let kit_strip = app.project().tracks[0].strip.unwrap();
    let kick = app.project().tracks[0].instrument.pulsus().unwrap().pads[0].clone();
    let free = app.project().first_free_strip().unwrap();
    let free_n = app.project().strip_number(free).unwrap();
    let kit_n = app.project().strip_number(kit_strip).unwrap();
    let before = app.project().clone();
    app.select_clip(t, c);
    app.show_central(CentralView::ChannelRack);
    for _ in 0..4 {
        let _ = frame(&mut app, &ctx, vec![]);
    }
    let well = app
        .rack
        .pad_wells
        .iter()
        .find(|(tr, pad, _)| *tr == t && *pad == kick.id)
        .map(|w| w.2)
        .unwrap_or_else(|| {
            let shapes = frame(&mut app, &ctx, vec![]);
            let words: Vec<String> = texts(&shapes).into_iter().map(|t| t.0).collect();
            panic!(
                "the kick's row has its well: wells {:?}, folds {:?}, painted {words:?}",
                app.rack.pad_wells, app.rack.kit_folds
            )
        });
    // The well reads the kit's strip until a track is chosen.
    let shapes = frame(&mut app, &ctx, vec![]);
    let at_well = |shapes: &[egui::epaint::ClippedShape]| {
        texts(shapes)
            .into_iter()
            .find(|(_, r)| well.contains(r.center()))
            .map(|(s, _)| s)
    };
    assert_eq!(at_well(&shapes), Some(kit_n.to_string()));
    // The click opens the list; the list offers the first free strip.
    click(&mut app, &ctx, well.center());
    let shapes = frame(&mut app, &ctx, vec![]);
    let want = format!("First free strip ({free_n})");
    let (_, entry) = texts(&shapes)
        .into_iter()
        .find(|(s, _)| *s == want)
        .unwrap_or_else(|| panic!("the list offers {want:?}"));
    click(&mut app, &ctx, entry.center());
    let _ = frame(&mut app, &ctx, vec![]);
    let now = app.project().tracks[0].instrument.pulsus().unwrap().pads[0].output;
    assert_eq!(now, Some(free), "the kick plays on the free strip");
    assert_eq!(app.bus.undo_label_key(), Some("cmd.set_pad_track"));
    let shapes = frame(&mut app, &ctx, vec![]);
    assert_eq!(
        at_well(&shapes),
        Some(free_n.to_string()),
        "the well reads it"
    );

    // Heard there and only there: the free strip is the kick alone, the kit's strip the kit
    // without it — to the bit.
    let p = app.project().clone();
    let (ki, fi) = (
        p.strip_index(kit_strip).unwrap(),
        p.strip_index(free).unwrap(),
    );
    let got = stems(&mut app, &p, &[fi, ki]);
    let mut alone = before.clone();
    let mut rest = before.clone();
    alone.tracks[0].clips[0]
        .notes
        .retain(|n| n.pitch == kick.pitch);
    rest.tracks[0].clips[0]
        .notes
        .retain(|n| n.pitch != kick.pitch);
    let alone = stems(&mut app, &alone, &[ki]).remove(0);
    let rest = stems(&mut app, &rest, &[ki]).remove(0);
    assert!(got[0].iter().any(|x| *x != 0.0), "the free strip sounds");
    assert!(
        got[0]
            .iter()
            .zip(&alone)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "the free strip is the kick alone"
    );
    assert!(
        got[1]
            .iter()
            .zip(&rest)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "the kit's strip is the kit without the kick"
    );

    // Saved and read back.
    let path = dir.path().join("pads.cantus");
    app.save_to_path(&path);
    let back = cantus_project::load(&path, false).unwrap().project;
    assert_eq!(
        back.tracks[0].instrument.pulsus().unwrap().pads[0].output,
        Some(free)
    );

    // One step undoes it: the kick back on the kit's strip.
    app.undo();
    assert_eq!(app.project(), &before, "undo is exact");
    let shapes = frame(&mut app, &ctx, vec![]);
    assert_eq!(at_well(&shapes), Some(kit_n.to_string()));
}
