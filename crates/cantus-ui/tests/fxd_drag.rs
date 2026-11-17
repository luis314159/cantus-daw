//! fxd step 3 — the controllers' windows end to end (ADRs 0147, 0148), on the boot path: an
//! app made by `CantusApp::assemble`, driven with real pointer and key events (`paths_rig`).
//! Tactus and Calculus open from their modulator's row in the inspector (the *Open* key);
//! a drag on the pad sets X and Y as one undo step, and while a pass writes automation it
//! writes two lanes; a drawn and a recorded path are one step each; the cards' knobs and
//! switches; the formula typed on its display and on the keypad is committed when it
//! compiles (one step a session), a broken draft never reaches the project and shows its
//! error, Esc drops it; the dials; a preset chosen from the header; the puck where the
//! telemetry says.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

mod paths_rig;

use cantus_core::modulator::formula::Program;
use cantus_core::modulator::{presets, FormulaSource, ModHost, ModKind, ModSource, XySource};
use cantus_core::*;
use cantus_ui::views::workspace::Pane;
use egui::{pos2, vec2, Event, Key, PointerButton, Pos2, Rect};
use paths_rig::*;

/// A song with a controller of `kind` on the master, linked to the master's pan, the
/// master in the inspector.
fn rig(kind: ModKind) -> (Rig, ModulatorId) {
    let mut r = Rig::new(cantus_ui::welcome_project());
    let id = r
        .app
        .add_modulator(ModHost::Master, kind, None)
        .expect("a controller on the master");
    r.app
        .set_route_depth(id, &AutoTarget::MasterPan, 0.5, false);
    r.app.select_master();
    if !r.app.workspace.contains(Pane::Inspector) {
        r.app.toggle_pane(Pane::Inspector);
    }
    r.settle();
    (r, id)
}

/// The window opened as a person opens it: a click on the row's *Open* key.
fn open(r: &mut Rig, id: ModulatorId) {
    let insp = r.leaf_of(Pane::Inspector).expect("the inspector is shown");
    let key = r.must_in("Open", insp);
    r.click(key.center());
    assert_eq!(
        r.app.fx.fxd.open,
        Some(id),
        "Open opens the controller's window"
    );
    r.settle();
}

fn pad_of(r: &Rig, id: ModulatorId) -> XySource {
    match &r.app.project().modulator(id).unwrap().2.source {
        ModSource::XyPad(x) => x.clone(),
        _ => panic!("a pad"),
    }
}

fn formula_of(r: &Rig, id: ModulatorId) -> FormulaSource {
    match &r.app.project().modulator(id).unwrap().2.source {
        ModSource::Formula(f) => f.clone(),
        _ => panic!("a formula"),
    }
}

/// A point of the pad (0…1, y up) on screen.
fn on_pad(r: &Rig, x: f32, y: f32) -> Pos2 {
    let p = r.app.fx.fxd.rects_xy.pad;
    pos2(p.left() + x * p.width(), p.bottom() - y * p.height())
}

/// Where a point on screen is on the pad.
fn pad_at(r: &Rig, p: Pos2) -> [f32; 2] {
    let o = r.app.fx.fxd.rects_xy.pad;
    [
        ((p.x - o.left()) / o.width()).clamp(0.0, 1.0),
        ((o.bottom() - p.y) / o.height()).clamp(0.0, 1.0),
    ]
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// A drag whose frames each first put the song at the next of `beats` (the song plays).
fn drag_playing(r: &mut Rig, a: Pos2, b: Pos2, beats: &[f64]) {
    r.hover(a);
    let tick = |x: f64| Ticks((x * cantus_core::PPQ as f64) as i64);
    r.app.engine.set_heard_for_tests(tick(beats[0]), true);
    r.frame(vec![Event::PointerButton {
        pos: a,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: NONE,
    }]);
    let n = beats.len() - 1;
    for (i, at) in beats.iter().enumerate().skip(1) {
        r.app.engine.set_heard_for_tests(tick(*at), true);
        r.pointer = a + (b - a) * (i as f32 / n as f32);
        r.idle(1);
    }
    r.frame(vec![Event::PointerButton {
        pos: b,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: NONE,
    }]);
    r.settle();
}

fn item(r: &Rig, card: usize, item: usize, formula: bool) -> Rect {
    let items = if formula {
        &r.app.fx.fxd.rects_f.items
    } else {
        &r.app.fx.fxd.rects_xy.items
    };
    items
        .iter()
        .find(|(k, _)| *k == (card, item))
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("card {card} item {item} is drawn"))
}

fn undo_steps(r: &Rig) -> usize {
    r.app.bus.history().len()
}

#[test]
fn the_pad_opens_from_its_row_and_a_drag_sets_x_and_y_in_one_step() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    r.shot("fxd-pad-open");
    let pad = r.app.fx.fxd.rects_xy.pad;
    assert!(pad.width() > 400.0, "the pad is drawn: {pad:?}");
    let steps = undo_steps(&r);
    // From the centre to the upper right.
    let (a, b) = (on_pad(&r, 0.5, 0.5), on_pad(&r, 0.8, 0.7));
    r.drag(a, b);
    let src = pad_of(&r, id);
    let want = pad_at(&r, b);
    assert!(
        close(src.x, want[0], 1e-4) && close(src.y, want[1], 1e-4),
        "the puck's hand where the drag let go: {:?} vs {want:?}",
        (src.x, src.y)
    );
    assert_eq!(undo_steps(&r), steps + 1, "one drag, one undo step");
    // A second drag is a second step.
    let c = on_pad(&r, 0.2, 0.3);
    r.drag(b, c);
    assert_eq!(undo_steps(&r), steps + 2, "two drags, two steps");
    r.app.undo();
    let src = pad_of(&r, id);
    assert!(close(src.x, want[0], 1e-4) && close(src.y, want[1], 1e-4));
    r.app.undo();
    let src = pad_of(&r, id);
    assert_eq!((src.x, src.y), (0.5, 0.5), "one undo restores both axes");
}

#[test]
fn a_drag_while_a_pass_writes_automation_writes_two_lanes() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    r.app.engine.seek(Ticks::beats(1));
    r.app.start_recording();
    assert!(
        r.app.automation.write.writing(),
        "the pass writes automation"
    );
    let (a, b) = (on_pad(&r, 0.3, 0.3), on_pad(&r, 0.9, 0.6));
    r.drag(a, b);
    let written = r.app.written_points();
    for input in [0u8, 1] {
        assert!(
            written.contains_key(&AutoTarget::ModInput {
                modulator: id,
                input
            }),
            "the pad's input {input} is written: {:?}",
            written.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_drawn_path_is_one_step() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    let draw = r.app.fx.fxd.rects_xy.draw;
    r.click(draw.center());
    assert_eq!(
        r.app.fx.fxd.mode,
        cantus_ui::views::fx::fxd_xy::PadMode::Draw
    );
    let steps = undo_steps(&r);
    let (a, b) = (on_pad(&r, 0.2, 0.2), on_pad(&r, 0.8, 0.9));
    r.drag(a, b);
    let src = pad_of(&r, id);
    let path = src.path.expect("a path was drawn");
    assert!(
        path.points.len() >= 2 && path.points.len() <= 64,
        "{}",
        path.points.len()
    );
    assert_eq!(path.beats, 4.0, "over the default loop");
    assert!(src.play_path, "a path drawn plays");
    let last = *path.points.last().unwrap();
    let want = pad_at(&r, b);
    assert!(
        close(last[0], want[0], 0.02) && close(last[1], want[1], 0.02),
        "the path ends where the drag did: {last:?} vs {want:?}"
    );
    let first = path.points[0];
    assert!(
        first[0] < 0.4 && first[1] < 0.4,
        "it starts where the drag did: {first:?}"
    );
    assert_eq!((src.x, src.y), (0.5, 0.5), "drawing does not move the hand");
    assert_eq!(undo_steps(&r), steps + 1, "one drawn path, one step");
    assert_eq!(
        r.app.fx.fxd.mode,
        cantus_ui::views::fx::fxd_xy::PadMode::Hand,
        "the hand again after"
    );
    r.app.undo();
    assert!(pad_of(&r, id).path.is_none(), "undone: no path");
}

#[test]
fn a_gesture_recorded_while_the_song_plays_becomes_the_path() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    let rec = r.app.fx.fxd.rects_xy.record;
    r.click(rec.center());
    assert_eq!(
        r.app.fx.fxd.mode,
        cantus_ui::views::fx::fxd_xy::PadMode::Record
    );
    let steps = undo_steps(&r);
    // From beat 5 to beat 6 of the song: the second loop of four beats, its second beat.
    let beats: Vec<f64> = (0..=10).map(|i| 5.0 + i as f64 * 0.1).collect();
    let (a, b) = (on_pad(&r, 0.1, 0.2), on_pad(&r, 0.9, 0.8));
    drag_playing(&mut r, a, b, &beats);
    let src = pad_of(&r, id);
    let path = src.path.expect("a path was recorded");
    assert_eq!(path.beats, 4.0);
    assert_eq!(
        path.points.len(),
        (4.0 * cantus_ui::views::fx::fxd_edit::RECORD_PER_BEAT) as usize
    );
    // The loop begins at beat 4: the gesture's first sample holds until beat 5 and its last
    // after beat 6 (`XyPath::from_gesture` over the loop it began in).
    let n = path.points.len();
    let first = path.points[0];
    assert_eq!(
        path.points[n / 4 - 1],
        first,
        "held before the gesture began"
    );
    let last = path.points[n - 1];
    let want = pad_at(&r, b);
    assert!(
        close(last[0], want[0], 0.02) && close(last[1], want[1], 0.02),
        "held after it ended: {last:?} vs {want:?}"
    );
    assert!(
        first[0] < 0.35,
        "it starts where the gesture did: {first:?}"
    );
    assert_eq!(
        undo_steps(&r),
        steps + 1,
        "the moves heard and the path laid: one step"
    );
    r.app.undo();
    let src = pad_of(&r, id);
    assert!(src.path.is_none());
    assert_eq!(
        (src.x, src.y),
        (0.5, 0.5),
        "the hand's moves undone with it"
    );
}

#[test]
fn recording_over_a_playing_path_hears_the_hand_and_lays_the_new_path() {
    let (mut r, id) = rig(ModKind::XyPad);
    // A path that plays already.
    let mut m = r.app.project().modulator(id).unwrap().2.clone();
    if let ModSource::XyPad(x) = &mut m.source {
        x.path = Some(cantus_core::modulator::xy::XyPath {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            beats: 4.0,
            smooth: false,
        });
        x.play_path = true;
    }
    assert!(r.app.edit_modulator(m, None));
    open(&mut r, id);
    let rec = r.app.fx.fxd.rects_xy.record;
    r.click(rec.center());
    let steps = undo_steps(&r);
    let beats: Vec<f64> = (0..=10).map(|i| 5.0 + i as f64 * 0.1).collect();
    let (a, b) = (on_pad(&r, 0.2, 0.7), on_pad(&r, 0.8, 0.3));
    // Halfway: the old path is off, so the engine plays the hand's inputs.
    r.hover(a);
    let tick = |x: f64| Ticks((x * cantus_core::PPQ as f64) as i64);
    r.app.engine.set_heard_for_tests(tick(beats[0]), true);
    r.frame(vec![Event::PointerButton {
        pos: a,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: NONE,
    }]);
    r.pointer = a + (b - a) * 0.5;
    r.app.engine.set_heard_for_tests(tick(5.5), true);
    r.idle(1);
    assert!(
        !pad_of(&r, id).play_path,
        "while recording, the old path is silent: the hand is heard"
    );
    for (i, at) in beats.iter().enumerate().skip(6) {
        r.app.engine.set_heard_for_tests(tick(*at), true);
        r.pointer = a + (b - a) * (i as f32 / 10.0);
        r.idle(1);
    }
    r.frame(vec![Event::PointerButton {
        pos: b,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: NONE,
    }]);
    r.settle();
    let src = pad_of(&r, id);
    assert!(src.play_path, "the new path plays");
    assert_ne!(
        src.path.as_ref().unwrap().points.len(),
        2,
        "the new path laid"
    );
    assert_eq!(undo_steps(&r), steps + 1, "one step");
    r.app.undo();
    let src = pad_of(&r, id);
    assert!(src.play_path && src.path.as_ref().unwrap().points.len() == 2);
}

/// Row `i` of the pad's *What it drives* list as drawn (its axis tag and its bar).
fn link_row(r: &Rig, i: usize) -> cantus_ui::views::fx::fxd_xy::LinkRow {
    use cantus_ui::views::fx::fxd_xy;
    let k = r.app.fx.fxd.rects_xy.pad.width() / fxd_xy::PAD;
    let n = r.app.project().modulators(ModHost::Master).unwrap()[0]
        .routes
        .len();
    fxd_xy::link_rows(r.app.fx.fxd.rects_xy.links, k, n, 0, true)
        .into_iter()
        .find(|row| row.i == i)
        .expect("the row is drawn")
}

#[test]
fn a_link_s_axis_and_range_are_set_in_what_it_drives_one_step_each() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    let link = |r: &Rig| pad_of(r, id).links.first().cloned();
    assert!(link(&r).is_none(), "a new route reads X whole: no link");
    let steps = undo_steps(&r);
    // The axis tag: X becomes Y, one step; again, X (no link left).
    r.click(link_row(&r, 0).tag.center());
    assert_eq!(link(&r).map(|l| l.output), Some(1), "the pan now follows Y");
    assert_eq!(undo_steps(&r), steps + 1);
    r.click(link_row(&r, 0).tag.center());
    assert!(link(&r).is_none(), "back to X whole");
    assert_eq!(undo_steps(&r), steps + 2);
    // The bar's right end (the whole travel at depth 0.5 ends at three quarters) dragged
    // to five eighths: the route sweeps to half its reach, one step.
    let bar = link_row(&r, 0).bar;
    let x_at = |f: f32| bar.left() + f * bar.width();
    r.drag(
        pos2(x_at(0.75), bar.center().y),
        pos2(x_at(0.625), bar.center().y),
    );
    let g = link(&r).and_then(|l| l.range).expect("a range");
    assert_eq!(g.from, -1.0, "the other end stays");
    assert!(close(g.to, 0.5, 0.02), "the dragged end: {}", g.to);
    assert_eq!(undo_steps(&r), steps + 3, "one drag, one step");
    // Its middle dragged up bends it.
    r.drag(
        pos2(x_at(0.5), bar.center().y),
        pos2(x_at(0.5), bar.center().y - 40.0),
    );
    let g = link(&r).and_then(|l| l.range).expect("a range");
    assert!(g.curve > 0.2, "bent: {}", g.curve);
    assert_eq!(undo_steps(&r), steps + 4);
    // A double-click gives the whole travel back.
    r.double_click(pos2(x_at(0.5), bar.center().y));
    assert!(link(&r).is_none(), "whole and unbent: no link");
    r.app.undo();
    r.app.undo();
    let g = link(&r).and_then(|l| l.range).expect("a range again");
    assert!(
        close(g.to, 0.5, 0.02) && g.curve == 0.0,
        "undone a step at a time"
    );
}

#[test]
fn the_pads_cards_move_its_settings_one_step_each() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    // Inertia: a knob dragged up.
    let k = item(&r, 1, 0, false);
    let steps = undo_steps(&r);
    r.drag(k.center(), k.center() - vec2(0.0, 60.0));
    let inertia = pad_of(&r, id).inertia_ms;
    assert!(inertia > 0.0, "the inertia moved: {inertia}");
    assert_eq!(undo_steps(&r), steps + 1, "one drag, one step");
    // The horizontal knob is the X input.
    let k = item(&r, 0, 0, false);
    r.drag(k.center(), k.center() - vec2(0.0, 40.0));
    assert!(pad_of(&r, id).x > 0.5, "X moved by its knob");
    assert_eq!(undo_steps(&r), steps + 2);
    // Polarity: One way.
    let sw = item(&r, 0, 2, false);
    r.click(pos2(sw.right() - sw.width() * 0.25, sw.center().y));
    assert!(pad_of(&r, id).unipolar, "One way chosen");
    assert_eq!(undo_steps(&r), steps + 3);
    r.app.undo();
    assert!(!pad_of(&r, id).unipolar);
    r.app.undo();
    assert_eq!(pad_of(&r, id).x, 0.5);
    r.app.undo();
    assert_eq!(pad_of(&r, id).inertia_ms, 0.0);
}

#[test]
fn the_puck_is_drawn_where_the_telemetry_says() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    r.app.fx.fxd.injected = Some(([0.25, 0.75], [0.6, 0.4]));
    r.settle();
    let live = &r.app.fx.fxd.live_xy;
    assert_eq!((live.puck, live.heads), ([0.25, 0.75], [0.6, 0.4]));
    let at = r.must("X 0.25   Y 0.75");
    assert!(
        r.app.fx.fxd.rects_xy.pad.contains(at.center()),
        "its reading on the glass"
    );
    // The window goes when its modulator does.
    r.app.exec(Command::RemoveModulator { id });
    r.settle();
    assert_eq!(r.app.fx.fxd.open, None, "removed: the window closes");
}

/// A click on the formula's display just after its last character: typing there.
fn focus_end(r: &mut Rig) {
    let d = r.app.fx.fxd.rects_f.display;
    r.click(pos2(d.right() - 20.0, d.center().y));
    assert!(
        r.app.fx.fxd.live_f.focused,
        "the display holds the keyboard"
    );
}

#[test]
fn the_formula_typed_on_its_display_is_one_step_a_session() {
    let (mut r, id) = rig(ModKind::Formula);
    open(&mut r, id);
    r.shot("fxd-formula-open");
    let start = formula_of(&r, id).text;
    assert_eq!(start, FormulaSource::DEFAULT_TEXT);
    let steps = undo_steps(&r);
    focus_end(&mut r);
    // "+0" compiles: committed; a Backspace leaves "+", which does not: a draft.
    r.type_text("+0");
    assert_eq!(formula_of(&r, id).text, format!("{start}+0"));
    r.key(Key::Backspace, NONE);
    let draft = format!("{start}+");
    assert_eq!(
        r.app.fx.fxd.live_f.draft.as_deref(),
        Some(draft.as_str()),
        "the broken draft is shown"
    );
    assert_eq!(
        formula_of(&r, id).text,
        format!("{start}+0"),
        "a draft that does not compile never reaches the project"
    );
    let err = Program::compile(&draft).unwrap_err();
    assert_eq!(err.at, draft.len(), "the error is at the end");
    r.must("The formula ends where a value is needed");
    // The arrows and Home: "1" typed at the start, "a" at the end.
    r.key(Key::Home, NONE);
    r.type_text("1");
    r.key(Key::End, NONE);
    r.key(Key::ArrowLeft, NONE);
    r.key(Key::ArrowRight, NONE);
    r.type_text("a");
    let want = format!("1{start}+a");
    assert_eq!(formula_of(&r, id).text, want, "typed at the caret");
    r.key(Key::Enter, NONE);
    assert!(!r.app.fx.fxd.live_f.focused, "Enter ends the session");
    assert_eq!(undo_steps(&r), steps + 1, "one session, one undo step");
    r.app.undo();
    assert_eq!(formula_of(&r, id).text, start, "undone whole");
}

#[test]
fn the_keypad_types_at_the_caret_and_esc_drops_a_broken_draft() {
    let (mut r, id) = rig(ModKind::Formula);
    open(&mut r, id);
    let start = formula_of(&r, id).text;
    let steps = undo_steps(&r);
    let keys = r.app.fx.fxd.rects_f.keys.clone();
    let key_of = |w: &str| {
        let i = cantus_ui::views::fx::fxd_formula::KEYS
            .iter()
            .position(|k| k.1 == w)
            .unwrap();
        keys[i].center()
    };
    // A key typed with no session begins one at the end: "+" (broken), then "a".
    r.click(key_of("+"));
    assert_eq!(
        formula_of(&r, id).text,
        start,
        "a broken draft is not committed"
    );
    r.click(key_of("a"));
    assert_eq!(formula_of(&r, id).text, format!("{start}+a"));
    r.key(Key::Enter, NONE);
    assert_eq!(undo_steps(&r), steps + 1, "one session, one step");
    // Esc: the draft "+" that does not compile is dropped; the last good text shows.
    focus_end(&mut r);
    r.type_text("*");
    assert!(r
        .app
        .fx
        .fxd
        .live_f
        .draft
        .as_deref()
        .is_some_and(|d| d.ends_with('*')));
    r.key(Key::Escape, NONE);
    assert_eq!(r.app.fx.fxd.live_f.draft, None, "Esc drops the draft");
    assert_eq!(formula_of(&r, id).text, format!("{start}+a"));
    assert_eq!(undo_steps(&r), steps + 1, "nothing more in the history");
    assert_eq!(
        r.app.fx.fxd.open,
        Some(id),
        "Esc while typing keeps the window"
    );
    r.app.undo();
    assert_eq!(formula_of(&r, id).text, start);
}

#[test]
fn the_dials_turn_a_b_c_one_step_a_drag() {
    let (mut r, id) = rig(ModKind::Formula);
    open(&mut r, id);
    let before = formula_of(&r, id).inputs;
    let steps = undo_steps(&r);
    let d = r.app.fx.fxd.rects_f.dials[1];
    r.drag(d.center(), d.center() - vec2(0.0, 30.0));
    let after = formula_of(&r, id).inputs;
    assert!(
        after[1] > before[1] + 0.1,
        "b turned up: {before:?} -> {after:?}"
    );
    assert_eq!(
        (after[0], after[2]),
        (before[0], before[2]),
        "a and c untouched"
    );
    assert_eq!(undo_steps(&r), steps + 1, "one drag, one step");
    r.app.undo();
    assert_eq!(formula_of(&r, id).inputs, before);
    // The card's knob a is the same input.
    let k = item(&r, 0, 0, true);
    r.drag(k.center(), k.center() - vec2(0.0, 40.0));
    assert!(
        formula_of(&r, id).inputs[0] > before[0],
        "a by its card's knob"
    );
}

#[test]
fn a_preset_chosen_from_the_header_is_one_step_and_named() {
    let (mut r, id) = rig(ModKind::Formula);
    open(&mut r, id);
    let header = r.app.fx.fxd.header.expect("the header is drawn");
    let steps = undo_steps(&r);
    // The well's next arrow: the kind's first preset (the settings match none yet).
    r.click(header.preset_next.expect("the well's arrows").center());
    let first = presets::of_kind(ModKind::Formula).next().unwrap();
    let m = r.app.project().modulator(id).unwrap().2.clone();
    assert_eq!(presets::matching(&m).map(|p| p.id), Some(first.id));
    assert_eq!(undo_steps(&r), steps + 1, "a preset, one step");
    let name = cantus_ui::i18n::t(&format!("fxd.preset.{}", first.id));
    let well = header.preset.unwrap();
    r.must_in(&name, well.expand(4.0));
    // The list: its third row.
    r.click(well.center());
    let rows = r.app.fx.fxd.menu_rows.clone();
    assert!(rows.len() >= 3, "the list is open: {rows:?}");
    r.click(rows[2].center());
    let third = presets::of_kind(ModKind::Formula).nth(2).unwrap();
    let m = r.app.project().modulator(id).unwrap().2.clone();
    assert_eq!(presets::matching(&m).map(|p| p.id), Some(third.id));
    assert_eq!(undo_steps(&r), steps + 2);
    assert!(r.app.fx.fxd.menu_rows.is_empty(), "the list closed");
    r.app.undo();
    let m = r.app.project().modulator(id).unwrap().2.clone();
    assert_eq!(presets::matching(&m).map(|p| p.id), Some(first.id));
}

#[test]
fn the_window_closes_by_its_key_and_esc_and_opens_by_a_double_click_on_the_picture() {
    let (mut r, id) = rig(ModKind::XyPad);
    open(&mut r, id);
    // Its close key.
    let close = r
        .app
        .fx
        .fxd
        .header
        .and_then(|h| h.window)
        .and_then(|w| w.close)
        .expect("the window's close key");
    r.click(close.center());
    assert_eq!(r.app.fx.fxd.open, None, "the close key closes it");
    // A double-click on the row's picture (just above the Window row) opens it again.
    let insp = r.leaf_of(Pane::Inspector).unwrap();
    let row = r.must_in("Window", insp);
    r.double_click(pos2(insp.center().x, row.top() - 22.0));
    assert_eq!(
        r.app.fx.fxd.open,
        Some(id),
        "a double-click on the picture opens it"
    );
    r.settle();
    // Esc closes the device clicked last: a click on the window, then Esc.
    let wheel = r.app.fx.fxd.rects_xy.links;
    r.click(pos2(wheel.center().x, wheel.top() - 60.0));
    r.key(Key::Escape, NONE);
    assert_eq!(r.app.fx.fxd.open, None, "Esc closes it");
}

#[test]
fn many_links_scroll_by_the_wheel() {
    let (mut r, id) = rig(ModKind::XyPad);
    let strips: Vec<StripId> = r
        .app
        .project()
        .tracks
        .iter()
        .filter_map(|t| t.strip)
        .collect();
    for s in strips {
        for t in [
            AutoTarget::StripGain(s),
            AutoTarget::StripPan(s),
            AutoTarget::StripWidth(s),
        ] {
            r.app.set_route_depth(id, &t, 0.3, false);
        }
    }
    r.app
        .set_route_depth(id, &AutoTarget::MasterWidth, 0.3, false);
    let n = r.app.project().modulator(id).unwrap().2.routes.len();
    assert!(n > 8, "more routes than the list shows: {n}");
    open(&mut r, id);
    let list = r.app.fx.fxd.rects_xy.links;
    assert_eq!(r.app.fx.fxd.live_xy.links_first, 0);
    r.wheel(list.center(), -1.0, NONE);
    assert_eq!(
        r.app.fx.fxd.live_xy.links_first, 1,
        "a notch down, a row on"
    );
    r.wheel(list.center(), 1.0, NONE);
    assert_eq!(r.app.fx.fxd.live_xy.links_first, 0, "a notch up, back");
}

#[test]
fn a_dial_double_clicked_goes_back_to_the_middle() {
    let (mut r, id) = rig(ModKind::Formula);
    open(&mut r, id);
    r.app.exec(Command::SetModulatorInput {
        id,
        input: 2,
        value: 0.9,
    });
    r.settle();
    let steps = undo_steps(&r);
    let d = r.app.fx.fxd.rects_f.dials[2];
    r.double_click(d.center());
    assert_eq!(formula_of(&r, id).inputs[2], 0.5);
    assert_eq!(undo_steps(&r), steps + 1);
}

#[test]
fn a_formula_longer_than_the_line_keeps_the_caret_in_sight() {
    let (mut r, id) = rig(ModKind::Formula);
    let long = "0.5+0.25*Sin(SongTime*Pi*a)+0.125*Cos(SongTime*Pi*b*2)+0.0625*tri(SongTime*c*4)+0.03125*saw(SongTime/4)";
    let mut m = r.app.project().modulator(id).unwrap().2.clone();
    if let ModSource::Formula(f) = &mut m.source {
        f.text = long.into();
    }
    assert!(r.app.edit_modulator(m, None));
    open(&mut r, id);
    let d = r.app.fx.fxd.rects_f.display;
    let shown = |r: &Rig| {
        r.texts
            .iter()
            .rev()
            .find(|p| d.contains(p.seen.center()) && p.text.len() > 8)
            .map(|p| p.text.clone())
            .expect("the line is painted")
    };
    let at_rest = shown(&r);
    assert!(
        long.starts_with(&at_rest) && at_rest.len() < long.len(),
        "{at_rest}"
    );
    focus_end(&mut r);
    r.key(Key::End, NONE);
    let end = shown(&r);
    assert!(long.ends_with(&end), "the end in sight: {end}");
    r.key(Key::Home, NONE);
    let start = shown(&r);
    assert!(long.starts_with(&start), "the start in sight: {start}");
    assert!(
        !r.texts
            .iter()
            .any(|p| p.elided && d.contains(p.seen.center())),
        "no letter cut"
    );
}
