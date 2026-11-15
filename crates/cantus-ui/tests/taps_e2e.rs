//! r40:pads — *Hear the key* and *Record: Before FX · After FX* end to end (ADR 0118), the
//! way a user reaches them: the app made by `CantusApp::assemble` with the engine's handle,
//! the inspector's sections opened and their rows clicked, the live engine driven by hand.
//!
//! - Hear the key: a click on the keyed compressor's *Hear the key* has the live engine play
//!   the detector's input — to the bit what the unit's own *Listen: Key* plays — and a
//!   second click gives the song back.
//! - Record: a click on *After FX* (one undo step, saved) makes the next take the strip after
//!   its inserts, in stereo, to the bit what was heard.

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
    M.lock().unwrap_or_else(|e| e.into_inner())
}

fn boot(dir: &std::path::Path, p: Project) -> (CantusApp, egui::Context, Processor) {
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
    let app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
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

/// Click the painted text `word` (the right-most, the inspector being at the window's
/// right), over a few frames.
fn click_text(app: &mut CantusApp, ctx: &egui::Context, word: &str) {
    let shapes = frame(app, ctx, vec![]);
    let all = texts(&shapes);
    let at = all
        .iter()
        .filter(|(s, _)| s == word)
        .max_by(|a, b| a.1.center().x.total_cmp(&b.1.center().x))
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| {
            let words: Vec<&str> = all.iter().map(|t| t.0.as_str()).collect();
            panic!("{word:?} is painted: {words:?}")
        });
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(app, ctx, vec![egui::Event::PointerMoved(at)]);
    let _ = frame(app, ctx, vec![press(true)]);
    let _ = frame(app, ctx, vec![press(false)]);
    let _ = frame(app, ctx, vec![]);
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i}: {:e} vs {:e}", a[i], b[i]);
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

// ---- Hear the key ------------------------------------------------------------------------

/// A lead keyed from a bass (the bass into a muted sink), the lead's strip at unity; the
/// compressor's settings `set`.
fn keyed_song(set: &[(&str, f32)]) -> (Project, EffectId) {
    let mut p = Project::new("Keyed");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let mut c = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    for i in 0..16i64 {
        c.notes
            .push(p.make_note(Ticks::EIGHTH * i, Ticks::EIGHTH, 60 + (i % 5) as u8, 90));
    }
    lead.clips.push(c);
    p.tracks.push(lead);
    let mut bass = p.make_track("Bass");
    let mut c = p.make_clip("Bass", Ticks::ZERO, Ticks::beats(8));
    for i in 0..8i64 {
        c.notes
            .push(p.make_note(Ticks::beats(i), Ticks::QUARTER, 36, 110));
    }
    bass.clips.push(c);
    p.tracks.push(bass);
    let (ls, bs) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    let sink = p.strips.last().unwrap().id;
    p.strip_mut(bs).unwrap().output = Some(sink);
    p.strip_mut(sink).unwrap().muted = true;
    let mut fx = p.make_effect(EffectKind::Compressor);
    for (k, v) in set {
        fx.spec.set(k, *v).unwrap();
    }
    fx.sidechain = Some(bs);
    let id = fx.id;
    let s = p.strip_mut(ls).unwrap();
    s.effects.push(fx);
    s.pan_law = Some(PanLaw::ZeroDb);
    (p, id)
}

const SET: [(&str, f32); 5] = [
    ("thr", -40.0),
    ("ratio", 20.0),
    ("key_gain", -6.0),
    ("sc0.used", 1.0),
    ("sc0.freq", 300.0),
];

/// `blocks` blocks of the live engine behind `h`, playing.
fn play(h: &mut EngineHandle, proc_: &mut Processor, blocks: usize) -> Vec<f32> {
    h.play();
    let mut out = vec![0f32; 256 * 2];
    let mut all = Vec::new();
    for _ in 0..blocks {
        proc_.process(&mut out, 2);
        h.poll();
        all.extend_from_slice(&out);
    }
    all
}

#[test]
fn hear_the_key_from_the_inspector_plays_the_detectors_input_live() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let (p, comp) = keyed_song(&SET);
    let (mut app, ctx, mut proc_) = boot(dir.path(), p);
    let lead = app.project().tracks[0].id;
    app.select_track(lead);
    for _ in 0..3 {
        let _ = frame(&mut app, &ctx, vec![]);
    }
    click_text(&mut app, &ctx, "Sidechain");
    click_text(&mut app, &ctx, "Hear the key");
    assert_eq!(app.engine.hear_key(), Some(comp), "the row turned it on");
    assert!(
        app.bus.undo_label_key().is_none(),
        "a monitor state, not an edit"
    );
    app.sync_engine();
    let heard = play(&mut app.engine, &mut proc_, 120);
    // The unit's own *Listen: Key*, in another engine with the switch off.
    let mut set = SET.to_vec();
    set.push(("listen", 1.0));
    let (q, _) = keyed_song(&set);
    let (mut h, mut ref_proc) = EngineHandle::manual(48_000.0);
    let snap = h.compile(&q, 1);
    h.set_snapshot(snap);
    let want = play(&mut h, &mut ref_proc, 120);
    same_bits(&heard, &want, "Hear the key");
    // A second click gives the song back.
    click_text(&mut app, &ctx, "Hear the key");
    assert_eq!(app.engine.hear_key(), None);
}

// ---- Record: Before FX · After FX --------------------------------------------------------

#[test]
fn after_fx_from_the_inspector_records_the_strip_after_its_inserts() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::new("Rec");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    let strip = t.strip.unwrap();
    p.tracks.push(t);
    let mut eq = p.make_effect(EffectKind::Eq);
    for (k, v) in [("b0.used", 1.0), ("b0.gain", 9.0), ("b0.freq", 2_000.0)] {
        eq.spec.set(k, v).unwrap();
    }
    let s = p.strip_mut(strip).unwrap();
    s.effects.push(eq);
    s.pan_law = Some(PanLaw::ZeroDb);
    let (mut app, ctx, mut proc_) = boot(dir.path(), p);
    let voice = app.project().tracks[0].id;
    app.select_track(voice);
    for _ in 0..3 {
        let _ = frame(&mut app, &ctx, vec![]);
    }
    click_text(&mut app, &ctx, "Recording");
    click_text(&mut app, &ctx, "After FX");
    assert_eq!(
        app.project().track(voice).unwrap().record_tap,
        RecordTap::AfterFx
    );
    assert_eq!(app.bus.undo_label_key(), Some("cmd.set_track_record_tap"));
    // Saved with the song.
    let path = dir.path().join("rec.cantus");
    app.save_to_path(&path);
    let back = cantus_project::load(&path, false).unwrap().project;
    assert_eq!(back.tracks[0].record_tap, RecordTap::AfterFx);

    // A take, through the app's own engine handle.
    app.sync_engine();
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(96_000);
    proc_.attach_input(consumer, 1);
    let takes_dir = dir.path().join("takes");
    assert_eq!(app.engine.begin_recording(&takes_dir), Ok(1));
    app.engine.set_recording(true);
    app.engine.play();
    let mut x = 0x2545_F491u32;
    let mut out = vec![0f32; 480 * 2];
    let mut heard = Vec::new();
    for _ in 0..40 {
        for _ in 0..480 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            ring.push((x as f32 / u32::MAX as f32) - 0.5).unwrap();
        }
        proc_.process(&mut out, 2);
        heard.extend_from_slice(&out);
        app.engine.poll();
    }
    app.engine.stop();
    for _ in 0..480 {
        ring.push(0.0).unwrap();
    }
    proc_.process(&mut out, 2);
    app.engine.poll();
    let takes = app.engine.finish_recording();
    assert_eq!(takes.len(), 1);
    let mut r = hound::WavReader::open(&takes[0].path).unwrap();
    assert_eq!(r.spec().channels, 2, "the strip's pair");
    let take: Vec<f32> = r.samples::<f32>().map(|s| s.unwrap()).collect();
    same_bits(
        &take,
        &heard[..take.len()],
        "the take is what the inserts played",
    );
    assert_eq!(take.len(), heard.len());

    // One undo: back to Before FX.
    app.undo();
    assert_eq!(
        app.project().track(voice).unwrap().record_tap,
        RecordTap::BeforeFx
    );
}
