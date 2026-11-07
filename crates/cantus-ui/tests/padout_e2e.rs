//! Pulsus III end to end (round 37, ADR 0100): through `CantusApp::assemble` — the boot
//! path — and the real engine (a manual engine's processor, driven as the callback drives
//! it), a kit is split to strips from the app's own action, one undo step; the split kit at
//! unity plays the unsplit kit to the bit; a pad strip's fader moves only that pad; *Merge
//! back* and undo bring the kit back. Its own binary: it points `CANTUS_DATA_DIR` at a
//! temporary folder.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// A kit track on its own strip playing a two-bar groove on every pad.
fn kit_song() -> Project {
    let mut p = Project::new("Kit");
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
            let vel = 50 + ((step * 17 + k * 29) % 77) as u8;
            c.notes.push(p.make_note(at, Ticks(PPQ / 3), pitch, vel));
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// The app on the boot path with `p` opened, and its engine's processor.
fn boot(p: Project) -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
    (app, proc)
}

/// Play from the top and take `frames` of what the engine sends the device.
fn play(app: &mut CantusApp, proc: &mut Processor, frames: usize) -> Vec<f32> {
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let mut out = Vec::with_capacity(frames * 2);
    let sizes = [512usize, 256, 333, 1024];
    let mut i = 0;
    while out.len() < frames * 2 {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
    }
    app.toggle_play();
    app.sync_engine();
    out
}

/// What the offline renderer makes of `p` (the export's engine), its first `frames`.
fn offline(app: &mut CantusApp, p: &Project, frames: usize) -> Vec<f32> {
    let snap = app.engine.compile_for_render(p, 1);
    let mut r = render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    r.truncate(frames * 2);
    r
}

fn peak(a: &[f32]) -> f32 {
    a.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

/// A fresh app on the boot path with the kit song, split to strips by the app's own action.
/// (Each capture has its own app: a stop lets the kit's tails ring, so a second play from
/// the same engine starts over them — the transport's behaviour, with or without a split.)
fn split_app() -> (CantusApp, Processor, TrackId) {
    let song = kit_song();
    let t = song.tracks[0].id;
    let (mut app, proc) = boot(song);
    assert!(app.split_kit(t));
    (app, proc, t)
}

#[test]
fn a_kit_split_through_the_app_plays_itself_and_a_pad_fader_moves_only_its_pad() {
    let frames = 4 * 48_000;
    let (mut app, mut proc) = boot(kit_song());
    let t = app.project().tracks[0].id;
    let opened = app.project().clone();
    let unsplit = offline(&mut app, &opened, frames);
    assert!(peak(&unsplit) > 0.1, "the groove sounds");
    // The app's own action: one undo step, a strip per pad, into the kit's strip.
    assert!(app.split_kit(t));
    assert_eq!(app.bus.undo_label_key(), Some("cmd.split_kit"));
    let outs = app.project().tracks[0].instrument.output_strips();
    assert_eq!(outs.len(), 14);
    let kit_strip = app.project().tracks[0].strip;
    assert!(outs
        .iter()
        .all(|s| app.project().strip(*s).unwrap().output == kit_strip));
    assert_eq!(
        app.kit_on_strip(outs[3]),
        Some(t),
        "a pad strip's menu finds its kit"
    );
    // Played live by the real engine: the unsplit kit, to the bit.
    let live = play(&mut app, &mut proc, frames);
    assert_eq!(live.len(), unsplit.len());
    if let Some(i) = live.iter().zip(&unsplit).position(|(a, b)| a != b) {
        panic!(
            "the split kit live differs at {i}: {} vs {}",
            live[i], unsplit[i]
        );
    }
    // Undo: the song as opened.
    app.bus.undo();
    assert_eq!(
        app.project().tracks[0].instrument,
        opened.tracks[0].instrument
    );
    assert_eq!(app.project().strips, opened.strips);
    assert!(!app.merge_kit(t), "nothing to merge: a notice, no command");

    // A pad strip's fader down: only that pad's sound changes.
    let (mut app, mut proc, _) = split_app();
    let outs = app.project().tracks[0].instrument.output_strips();
    let k = 2;
    assert!(app.exec(Command::SetStripGain {
        id: outs[k],
        gain_db: -12.0,
    }));
    let moved = play(&mut app, &mut proc, frames);
    let moved_song = app.project().clone();
    let exported = offline(&mut app, &moved_song, frames);
    if let Some(i) = moved.iter().zip(&exported).position(|(a, b)| a != b) {
        panic!(
            "the moved fader live differs at {i}: {} vs {}",
            moved[i], exported[i]
        );
    }
    let mut alone = moved_song.clone();
    let pitch = alone.tracks[0].instrument.pulsus().unwrap().pads[k].pitch;
    alone.tracks[0].clips[0].notes.retain(|n| n.pitch == pitch);
    alone.strip_mut(outs[k]).unwrap().gain_db = 0.0;
    let alone = offline(&mut app, &alone, frames);
    let g = db_to_linear(-12.0) - 1.0;
    let worst = moved
        .iter()
        .zip(&live)
        .zip(&alone)
        .map(|((m, l), a)| ((m - l) - a * g).abs())
        .fold(0.0f32, f32::max);
    assert!(peak(&alone) > 0.01, "the pad plays");
    assert!(worst < 2e-6, "something besides the pad moved: {worst:e}");

    // Merge back from the app: the kit again, to the bit, one undo step.
    let (mut app, mut proc, t) = split_app();
    assert!(app.merge_kit(t));
    assert_eq!(app.bus.undo_label_key(), Some("cmd.merge_kit"));
    assert!(!app.project().tracks[0]
        .instrument
        .pulsus()
        .unwrap()
        .has_outputs());
    let merged = play(&mut app, &mut proc, frames);
    assert!(merged == unsplit, "merged back, the kit plays as it did");
}

/// The kit's ⋯ menu, row by row as a click picks it: *Split to strips* and *Merge back*
/// are the rows that do it (the review of round 37's first delivery, item 7).
#[test]
fn the_kits_menu_rows_split_and_merge() {
    use cantus_ui::views::pulsus_editor::{more, more_row, more_rows, MoreRow};
    let (mut app, _proc) = boot(kit_song());
    let t = app.project().tracks[0].id;
    let rows = more_rows();
    let split = rows.iter().position(|r| *r == MoreRow::Split).unwrap();
    let merge = rows.iter().position(|r| *r == MoreRow::Merge).unwrap();
    assert_eq!(more_row(split), MoreRow::Split);
    more(&mut app, t, more_row(split));
    assert!(
        cantus_core::padout::is_split(app.project(), t),
        "split by its row"
    );
    assert_eq!(app.bus.undo_label_key(), Some("cmd.split_kit"));
    more(&mut app, t, more_row(merge));
    assert!(
        !cantus_core::padout::is_split(app.project(), t),
        "merged by its row"
    );
    // Every voice has its add row, and the last row closes.
    for v in cantus_core::PadVoice::ALL {
        assert!(rows.contains(&MoreRow::AddPad(v)));
    }
    assert_eq!(rows.last(), Some(&MoreRow::Close));
}

/// The kit's ⋯ menu opens the preset browser on the kit (round 37's library), and a kit
/// loaded from it is one undo step — driven through the menu's row as a click drives it.
#[test]
fn the_kits_menu_opens_the_preset_browser_and_a_kit_loads_in_one_step() {
    use cantus_ui::views::pulsus_editor::{more, more_rows, MoreRow};
    let (mut app, _proc) = boot(kit_song());
    let t = app.project().tracks[0].id;
    let rows = more_rows();
    let browse = rows
        .iter()
        .position(|r| *r == MoreRow::Browse)
        .expect("the row");
    more(&mut app, t, rows[browse]);
    assert!(app.presets.open, "the browser is open");
    // Every kit of the floor is there, each saying its style and that it is drums.
    let kits: Vec<String> = app.presets.entries.iter().map(|e| e.id.clone()).collect();
    assert!(kits.len() >= 16, "the kit floor: {kits:?}");
    assert!(app
        .presets
        .entries
        .iter()
        .all(|e| !e.styles.is_empty() && !e.roles.is_empty()));
    let before = app.project().clone();
    assert!(app.apply_preset_id("pulsus.kit.house"));
    assert_ne!(
        app.project().tracks[0].instrument,
        before.tracks[0].instrument
    );
    app.bus.undo();
    assert_eq!(
        app.project().tracks[0].instrument,
        before.tracks[0].instrument,
        "one step"
    );
}

// ---- a pad's chain by clicking its card (review 2 #8) ---------------------------------

/// Every straight text a frame painted, and where.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) if t.angle == 0.0 => {
                out.push((
                    t.galley.text().to_string(),
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                ));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        walk(&s.shape, &mut out);
    }
    out
}

/// Frames of the kit's editor, `events` in the first ones: the last frame's shapes.
fn editor(
    ctx: &egui::Context,
    app: &mut CantusApp,
    frames: Vec<Vec<egui::Event>>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut shapes = Vec::new();
    for events in frames {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            cantus_ui::views::pulsus_editor::window_in(app, ui);
        });
        out.textures_delta.clear();
        shapes = out.shapes;
    }
    shapes
}

fn click_at(p: egui::Pos2) -> Vec<Vec<egui::Event>> {
    let button = |pressed| egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    vec![
        vec![egui::Event::PointerMoved(p)],
        vec![button(true)],
        vec![button(false)],
        Vec::new(),
        Vec::new(),
    ]
}

/// The painted text `what` (any case): the last one painted, the one on top.
fn find(shapes: &[egui::epaint::ClippedShape], what: &str) -> egui::Pos2 {
    texts(shapes)
        .into_iter()
        .rev()
        .find(|(s, _)| s.eq_ignore_ascii_case(what))
        .unwrap_or_else(|| panic!("\"{what}\" is not painted"))
        .1
        .center()
}

/// On the boot path: the selected pad's chain card — *Add insert* ▸ Compressor makes the
/// pad's chain with it, one undo step; the card then paints the insert; *Remove* takes it
/// out, and the chain with it.
#[test]
fn a_pads_chain_is_made_and_emptied_by_clicking_its_card() {
    use cantus_ui::i18n::t;
    let (mut app, _proc) = boot(kit_song());
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let track = app.project().tracks[0].id;
    let snare = |app: &CantusApp| {
        app.project().tracks[0]
            .instrument
            .pulsus()
            .unwrap()
            .pads
            .iter()
            .find(|p| p.pitch == 38)
            .unwrap()
            .clone()
    };
    cantus_ui::views::pulsus_editor::open(&mut app, track);
    app.pulsus.view = cantus_ui::views::pulsus_editor::View::Edit;
    app.pulsus.selected = Some(snare(&app).id);
    let before = app.project().clone();
    let shapes = editor(&ctx, &mut app, vec![Vec::new(); 3]);
    let add = find(&shapes, &t("padout.chain_add"));
    let shapes = editor(&ctx, &mut app, click_at(add));
    let compressor = t(EffectKind::Compressor.name_key());
    let item = find(&shapes, &compressor);
    editor(&ctx, &mut app, click_at(item));
    let pad = snare(&app);
    let chain = padout::chain_of(app.project(), &pad).expect("the pad has its chain");
    let fx = padout::chain_effects(app.project(), &pad);
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].spec.kind, EffectKind::Compressor);
    // The card paints it, with its editor and its removal.
    let shapes = editor(&ctx, &mut app, vec![Vec::new(); 2]);
    find(&shapes, &compressor);
    let remove = find(&shapes, &t("padout.chain_remove"));
    // One undo step.
    let made = app.project().clone();
    app.undo();
    let mut back = app.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back, before, "the add undoes at once");
    app.redo();
    assert_eq!(app.project(), &made);
    editor(&ctx, &mut app, click_at(remove));
    assert!(padout::chain_of(app.project(), &snare(&app)).is_none());
    assert!(
        app.project().strip(chain).is_none(),
        "the chain went with its last insert"
    );
}
