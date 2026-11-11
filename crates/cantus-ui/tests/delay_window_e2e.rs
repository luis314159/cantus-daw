//! Delay's window end to end (round 39, ADR 0115), on the boot path: an app made by
//! `CantusApp::assemble`, driven as a person drives it (`paths_rig`) — Delay inserted the way
//! the picker inserts it (its starting preset), its window opened by a click on its insert in
//! the inspector; a drag on the echoes sets the time and the feedback as one step of undo;
//! every control writes its parameter as a command that undo takes back; the window speaks
//! Spanish; and at rest it asks for no frame — only sound coming from the tap does.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

mod paths_rig;

use cantus_core::*;
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_ticker::{self as ticker, Reason};
use egui::{pos2, vec2, Pos2, Rect};
use paths_rig::*;

/// A song with Delay on its first track's strip, that strip selected, the window closed.
fn rig() -> (Rig, StripId, EffectId) {
    let mut r = Rig::new(cantus_ui::welcome_project());
    let strip = r.app.project().tracks[0].strip.expect("a strip");
    r.app.add_effect(strip, EffectKind::Delay);
    let effect = r
        .app
        .project()
        .strip(strip)
        .unwrap()
        .effects
        .iter()
        .find(|e| e.spec.kind == EffectKind::Delay)
        .unwrap()
        .id;
    r.app.select_strip(strip);
    if !r.app.workspace.contains(Pane::Inspector) {
        r.app.toggle_pane(Pane::Inspector);
    }
    r.settle();
    (r, strip, effect)
}

fn spec(r: &Rig, strip: StripId, effect: EffectId) -> EffectSpec {
    r.app.project().effect(strip, effect).unwrap().spec.clone()
}

/// Painted words, any case (the face sets its titles in capitals), inside `region`.
fn find_in(r: &Rig, s: &str, region: Rect) -> Option<Rect> {
    r.texts
        .iter()
        .rev()
        .find(|p| p.text.trim().eq_ignore_ascii_case(s) && region.contains(p.seen.center()))
        .map(|p| p.seen)
}

/// The window opened from the insert: a click on Delay's row in the inspector.
fn open(r: &mut Rig, strip: StripId, effect: EffectId) -> Rect {
    let insp = r.leaf_of(Pane::Inspector).expect("the inspector is shown");
    let tile = find_in(r, "Delay", insp).unwrap_or_else(|| {
        let all: Vec<&str> = r.texts.iter().map(|p| p.text.as_str()).collect();
        panic!("Delay's insert is not in the inspector: {all:?}")
    });
    r.click(tile.center());
    assert_eq!(
        r.app.fx.open,
        Some((strip, effect, EffectKind::Delay)),
        "a click on the insert opens Delay's own window"
    );
    r.settle();
    r.app.fx.delay.display.expect("the echoes are drawn")
}

#[test]
fn a_click_on_the_insert_opens_the_window() {
    let (mut r, strip, effect) = rig();
    let d = open(&mut r, strip, effect);
    r.shot("delay-window");
    let whole = Rect::from_min_size(Pos2::ZERO, SIZE);
    for words in [
        "Echoes",
        "What is heard",
        "Time",
        "Repeats",
        "Output",
        "Ping-pong",
    ] {
        assert!(
            find_in(&r, words, whole).is_some(),
            "“{words}” is in the window"
        );
    }
    assert!(
        d.width() > 400.0 && d.height() > 200.0,
        "the display is the window's centre: {d:?}"
    );
}

#[test]
fn a_drag_on_the_echoes_sets_the_time_and_the_feedback_in_one_step() {
    let (mut r, strip, effect) = rig();
    let d = open(&mut r, strip, effect);
    let before = spec(&r, strip, effect);
    let steps = r.app.bus.history().len();
    // From the upper lane, to the right and up.
    let a = pos2(d.left() + d.width() * 0.3, d.top() + d.height() * 0.3);
    r.drag(a, a + vec2(60.0, -30.0));
    let after = spec(&r, strip, effect);
    assert!(
        after.get("time_ms") > before.get("time_ms") + 10.0,
        "the time follows across"
    );
    assert!(
        after.get("feedback") > before.get("feedback") + 0.02,
        "up is more feedback"
    );
    assert_eq!(r.app.bus.history().len(), steps + 1, "one drag, one step");
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.delay_echoes"));
    r.app.undo();
    let back = spec(&r, strip, effect);
    assert_eq!(back.get("time_ms"), before.get("time_ms"));
    assert_eq!(back.get("feedback"), before.get("feedback"));
}

/// Every control of the window — the four knobs, the ping-pong switch and the note value
/// at the tempo — writes its parameter, and undo takes it back.
#[test]
fn every_control_writes_its_parameter_with_undo() {
    let (mut r, strip, effect) = rig();
    let d = open(&mut r, strip, effect);
    let below = Rect::from_min_max(pos2(0.0, d.bottom() + 20.0), pos2(SIZE.x, SIZE.y));
    for (label, key) in [
        ("Time", "time_ms"),
        ("Feedback", "feedback"),
        ("Tone", "tone_hz"),
        ("Mix", "mix"),
    ] {
        let at = find_in(&r, label, below).unwrap_or_else(|| panic!("the {label} knob"));
        let was = spec(&r, strip, effect).get(key);
        let knob = at.center() + vec2(0.0, 30.0);
        r.drag(knob, knob + vec2(0.0, -40.0));
        let now = spec(&r, strip, effect).get(key);
        assert!(now > was, "{label}: {was} → {now}");
        r.app.undo();
        assert_eq!(spec(&r, strip, effect).get(key), was, "{label}: undone");
    }
    // Ping-pong: its switch.
    let was = spec(&r, strip, effect).get("ping_pong");
    let sw = find_in(&r, "Ping-pong", below).expect("the ping-pong switch");
    r.click(sw.center());
    assert_ne!(
        spec(&r, strip, effect).get("ping_pong"),
        was,
        "the switch flips it"
    );
    r.app.undo();
    assert_eq!(spec(&r, strip, effect).get("ping_pong"), was);
    // At the tempo: a quarter note sets the time once.
    let was = spec(&r, strip, effect).get("time_ms");
    let field = find_in(&r, "1/8·", below)
        .or_else(|| find_in(&r, "Free", below))
        .expect("the note field");
    r.click(field.center());
    let quarter = r.text("1/4").expect("the quarter note in the list");
    r.click(quarter.center());
    let bpm = cantus_ui::views::bars::bpm_at(r.app.project(), r.app.engine.position());
    let want = ((60_000.0 / bpm) * 10.0).round() as f32 / 10.0;
    assert_eq!(
        spec(&r, strip, effect).get("time_ms"),
        want,
        "a quarter at {bpm} BPM"
    );
    r.app.undo();
    assert_eq!(spec(&r, strip, effect).get("time_ms"), was);
}

/// The window at rest asks for no frame; sound from the tap moves it, and when it stops
/// the window is still again.
#[test]
fn at_rest_nothing_repaints_and_sound_moves_it() {
    let (mut r, strip, effect) = rig();
    open(&mut r, strip, effect);
    r.idle(30);
    let t = ticker::last(&r.ctx);
    assert!(
        !t.has(Reason::Live),
        "the open window asks for nothing at rest: {t:?}"
    );
    // Sound through the strip: the tap's pairs, every frame — four frames of a note, then
    // silence still coming from the tap.
    let mut live = Vec::new();
    for i in 0..48 {
        let v = if i < 4 { 0.5 } else { 0.0 };
        r.app.fx.feed(&[v, v].repeat(800));
        r.idle(1);
        live.push(ticker::last(&r.ctx).has(Reason::Live));
    }
    assert!(live[..4].iter().all(|l| *l), "the note moves it: {live:?}");
    assert!(
        !live[8],
        "between the note and its first repeat, nothing moves: {live:?}"
    );
    assert!(
        live[14..40].iter().any(|l| *l),
        "the first repeat lights as the note reaches it: {live:?}"
    );
    // The tap falls quiet: half a second on, nothing moves.
    r.idle(60);
    assert!(!ticker::last(&r.ctx).has(Reason::Live), "still again");
}

/// The window in Spanish: its own words, nothing left in English.
#[test]
fn the_window_speaks_spanish() {
    let (mut r, strip, effect) = rig();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    r.settle();
    let insp = r.leaf_of(Pane::Inspector).expect("the inspector");
    let tile = find_in(&r, "Delay", insp).expect("the insert");
    r.click(tile.center());
    r.settle();
    assert_eq!(r.app.fx.open.map(|o| o.1), Some(effect));
    let _ = strip;
    let whole = Rect::from_min_size(Pos2::ZERO, SIZE);
    for words in ["Ecos", "Lo que se oye", "Tiempo", "Repeticiones", "Salida"] {
        assert!(
            find_in(&r, words, whole).is_some(),
            "“{words}” is in the window"
        );
    }
    for english in ["Echoes", "What is heard", "Repeats"] {
        assert!(
            find_in(&r, english, whole).is_none(),
            "“{english}” is left in English"
        );
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Every word of the window in English has its Spanish twin.
#[test]
fn every_word_has_its_spanish_twin() {
    let keys = |s: &str| -> Vec<String> {
        s.lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim().to_string()))
            .collect()
    };
    let en = keys(include_str!("../locales/fx/delay.en.txt"));
    let es = keys(include_str!("../locales/fx/delay.es.txt"));
    assert!(!en.is_empty());
    assert_eq!(en, es, "the same keys in the same order");
}
