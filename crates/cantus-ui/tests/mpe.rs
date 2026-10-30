//! Per-note expression in the application (round 33, ADR 0080): every note operation
//! keeps a note's curves where they belong — slicing, transposing, moving, copying,
//! editing through a linked copy, gluing; an expression edit is one undo step with its own
//! name (the boot path is `mpe_boot.rs`).

mod support;

use cantus_core::model::note_expr::{ExprEdit, ExprKind, ExprPoint, NoteExpr};
use cantus_core::*;
use cantus_ui::CantusApp;
use support::app;

fn st(x: f64) -> i64 {
    ExprKind::Bend.store(x)
}

/// One track, one clip, one note bent up two semitones over its two beats.
fn bent() -> (CantusApp, TrackId, ClipId, NoteId) {
    let mut p = Project::new("mpe");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    let mut n = p.make_note(Ticks::ZERO, Ticks::beats(2), 60, 100);
    n.expr = NoteExpr {
        bend: vec![ExprPoint(0, 0), ExprPoint(1920, st(2.0))],
        ..NoteExpr::default()
    }
    .boxed();
    let id = n.id;
    c.notes.push(n);
    let (tid, cid) = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    let mut a = app(p);
    a.select_clip(tid, cid);
    a.focus_piano_roll();
    (a, tid, cid, id)
}

fn notes(a: &CantusApp, t: TrackId, c: ClipId) -> Vec<Note> {
    a.project().track(t).unwrap().clip(c).unwrap().notes.clone()
}

#[test]
fn slicing_a_note_gives_each_half_what_it_sounded() {
    let (mut a, t, c, id) = bent();
    a.selected_notes = vec![id];
    assert_eq!(a.slice_at(Ticks::beats(1)), 1);
    let ns = notes(&a, t, c);
    assert_eq!(ns.len(), 2);
    let (l, r) = (&ns[0], &ns[1]);
    let (le, re) = (l.expr.as_deref().unwrap(), r.expr.as_deref().unwrap());
    assert_eq!(le.value_at(ExprKind::Bend, 480), Some(0.5));
    assert_eq!(
        re.value_at(ExprKind::Bend, 0),
        Some(1.0),
        "the right half starts where it was"
    );
    assert_eq!(re.value_at(ExprKind::Bend, 960), Some(2.0));
    a.undo();
    assert_eq!(notes(&a, t, c).len(), 1);
    assert!(notes(&a, t, c)[0].expr.is_some());
}

#[test]
fn transposing_moving_and_copying_carry_the_curves() {
    let (mut a, t, c, id) = bent();
    let before = notes(&a, t, c)[0].expr.clone();
    a.selected_notes = vec![id];
    a.transpose_selection(5);
    a.nudge_selection(Ticks::QUARTER);
    let n = &notes(&a, t, c)[0];
    assert_eq!((n.pitch, n.start), (65, Ticks::QUARTER));
    assert_eq!(n.expr, before, "relative to the note: nothing to move");
    // Copy and paste: the copy has the curve too.
    a.run_action(cantus_ui::keymap::Action::Copy);
    a.run_action(cantus_ui::keymap::Action::Paste);
    let ns = notes(&a, t, c);
    assert!(ns.len() >= 2, "{ns:?}");
    assert!(ns.iter().all(|n| n.expr == before));
}

#[test]
fn an_expression_edit_is_one_named_step_and_reaches_linked_copies() {
    let (mut a, t, c, id) = bent();
    a.focus_arrangement();
    a.select_clip(t, c);
    a.run_action(cantus_ui::keymap::Action::Duplicate);
    let copy = a.clip_selection()[0];
    assert_ne!(copy.1, c);
    let cmd = cantus_core::model::note_expr::cmd::command_for(
        a.project(),
        t,
        copy.1,
        &[id],
        ExprEdit::Flatten,
        |_| None,
    )
    .expect("a change");
    assert_eq!(cmd.label_key(), "cmd.mpe.flatten");
    assert!(a.exec(cmd));
    // Linked: the original lost its curve too.
    assert!(notes(&a, t, c)[0].expr.is_none());
    assert!(notes(&a, copy.0, copy.1)[0].expr.is_none());
    a.undo();
    assert!(notes(&a, t, c)[0].expr.is_some());
    assert!(cantus_ui::i18n::t("cmd.mpe.flatten") != "cmd.mpe.flatten");
}

/// The frame budget with a dense MPE take (ADR 0080): 96 notes, each with a vibrato of a
/// few hundred points and pressure and timbre, the curves on the notes and all three rows
/// open — what the piano roll paints stays within a bound of the same take without
/// expression (points are sampled to the pixels, not drawn one by one), and the time a
/// frame takes is printed (`--nocapture`; the release timing is the frame_budget suite's).
#[test]
fn a_dense_mpe_clip_stays_in_the_frame_budget() {
    use egui::vec2;
    let build = |with: bool| {
        let mut p = Project::new("dense");
        let mut t = p.make_track("t");
        let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(32));
        for i in 0..96i64 {
            let mut n = p.make_note(Ticks(i * 320), Ticks(300), 48 + (i % 24) as u8, 90);
            if with {
                n.expr = NoteExpr {
                    bend: (0..300)
                        .map(|k| ExprPoint(k, st(0.3 * ((k % 20) as f64 / 10.0 - 1.0))))
                        .collect(),
                    pressure: (0..60)
                        .map(|k| ExprPoint(k * 5, ExprKind::Pressure.store(k as f64 / 60.0)))
                        .collect(),
                    timbre: vec![ExprPoint(0, 0), ExprPoint(300, ExprKind::Timbre.store(1.0))],
                }
                .boxed();
            }
            c.notes.push(n);
        }
        let (tid, cid) = (t.id, c.id);
        t.clips.push(c);
        p.tracks.push(t);
        let mut a = app(p);
        a.select_clip(tid, cid);
        a.focus_piano_roll();
        a.piano_roll.lanes.expr = [with; 3];
        a.piano_roll.vel_h = 300.0;
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        let mut verts = 0usize;
        let mut ms = Vec::new();
        for frame in 0..12 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(1600.0, 900.0),
                )),
                ..Default::default()
            };
            let t0 = std::time::Instant::now();
            let out = ctx.run_ui(input, |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| cantus_ui::views::piano_roll::show(&mut a, ui));
            });
            let mut out = out;
            let shapes = std::mem::take(&mut out.shapes);
            let prims = ctx.tessellate(shapes, out.pixels_per_point);
            out.textures_delta.clear();
            if frame >= 4 {
                ms.push(t0.elapsed().as_secs_f64() * 1e3);
            }
            verts = prims
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                    _ => 0,
                })
                .sum();
        }
        ms.sort_by(f64::total_cmp);
        (verts, ms[ms.len() / 2])
    };
    let (plain, plain_ms) = build(false);
    let (dense, dense_ms) = build(true);
    println!("piano roll: {plain} vertices, p50 {plain_ms:.2} ms; dense MPE: {dense} vertices, p50 {dense_ms:.2} ms (debug)");
    assert!(
        dense <= plain * 3 + 60_000,
        "{dense} vertices against {plain}"
    );
}

/// Review fix 1: a port marked MPE that is not open (the Seaboard unplugged) is no MPE
/// input — the engine is told none, so a keyboard on channel 2 stays an ordinary keyboard.
#[test]
fn a_marked_port_that_is_not_open_is_no_mpe_input() {
    let (mut a, _, _, _) = bent();
    a.settings.mpe_ports = vec!["Seaboard".into()];
    a.midi_port_names = vec!["Keys".into()];
    cantus_ui::mpe::input::publish(&mut a);
    assert_eq!(cantus_ui::mpe::input::last_published(), Some(None));
    a.midi_port_names = vec!["Keys".into(), "Seaboard".into()];
    cantus_ui::mpe::input::publish(&mut a);
    assert!(matches!(
        cantus_ui::mpe::input::last_published(),
        Some(Some(_))
    ));
}

/// Review D3: what the engine reported at the end of one take (a note-on heard as the
/// transport stopped) never opens a note in the next: the take starts empty.
#[test]
fn a_take_starts_without_the_last_ones_mpe_events() {
    use cantus_engine::inst::mpe::{ExprInput, MpeIn};
    let mut a = app(cantus_ui::starter_project());
    let track = a.selected_track.unwrap();
    let clip = a.selected_clip.unwrap();
    let ev = |tick: i64, pitch: u8, what| ExprInput {
        track,
        tick: Ticks(tick),
        channel: 1,
        pitch,
        start: Ticks(tick.min(960)),
        what,
    };
    // Left over from before this take.
    a.engine
        .inject_expr_inputs(&[ev(100, 70, MpeIn::On { velocity: 90 })]);
    a.start_recording();
    a.engine
        .inject_expr_inputs(&[ev(960, 62, MpeIn::On { velocity: 100 })]);
    a.ingest_note_inputs(Vec::new());
    a.engine
        .inject_expr_inputs(&[ev(1440, 62, MpeIn::Off { release: 0 })]);
    a.ingest_note_inputs(Vec::new());
    a.stop_recording();
    let c = a.project().track(track).unwrap().clip(clip).unwrap();
    assert!(c
        .notes
        .iter()
        .any(|n| n.pitch == 62 && n.start == Ticks(960)));
    assert!(
        !c.notes.iter().any(|n| n.pitch == 70),
        "a stale note-on opened a note"
    );
}
