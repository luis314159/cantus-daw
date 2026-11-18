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
    // The tempo (round 40): a quarter from the note value's list turns the sync on, one
    // step; undo takes both back.
    let was = spec(&r, strip, effect);
    let field = find_in(&r, "1/8\u{b7}", below).expect("the note field");
    r.click(field.center());
    // The list opens beside its field, never over it, and scrolls where the window is
    // short of its 27 values (beta.14's dropdown rule): the wheel brings the quarter up.
    for _ in 0..30 {
        match r.text_starting("1/4  ") {
            Some(q) if r.ctx.content_rect().contains_rect(q) && q.height() > 8.0 => break,
            _ => {
                let at = field.center_bottom() + vec2(0.0, 60.0);
                r.wheel(at, -2.0, NONE);
            }
        }
    }
    let quarter = r
        .text_starting("1/4  ")
        .expect("the quarter note in the list");
    r.click(quarter.center());
    let now = spec(&r, strip, effect);
    assert_eq!(
        (now.get("sync"), now.get("note")),
        (1.0, 12.0),
        "a quarter, synced"
    );
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.delay_sync_on"));
    r.app.undo();
    let back = spec(&r, strip, effect);
    assert_eq!(
        (back.get("sync"), back.get("note")),
        (was.get("sync"), was.get("note"))
    );
}

/// Round 40 (ADR 0119): the switch follows the tempo and keeps what is heard. *Sync* picks
/// the note nearest the time it was playing; the knob turns the note then; a new tempo moves
/// the echoes the window draws and the words under the switch; *Free* keeps the time it was
/// playing, in milliseconds. Each one step that undo takes back.
#[test]
fn the_sync_switch_follows_the_tempo_and_keeps_the_time() {
    let (mut r, strip, effect) = rig();
    let d = open(&mut r, strip, effect);
    let below = Rect::from_min_max(pos2(0.0, d.bottom() + 20.0), pos2(SIZE.x, SIZE.y));
    let tempo = |r: &Rig| cantus_ui::views::bars::bpm_at(r.app.project(), r.app.engine.position());
    // A time near a dotted eighth at the song's tempo.
    let t0 = tempo(&r);
    r.app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: "time_ms".into(),
        value: (45_000.0 / t0) as f32 + 3.0,
    });
    r.settle();
    let sync = find_in(&r, "Sync", below).expect("the sync switch");
    r.click(sync.center());
    let on = spec(&r, strip, effect);
    assert_eq!(on.get("sync"), 1.0, "synced");
    assert_eq!(on.get("note"), 10.0, "the note nearest: a dotted eighth");
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.delay_sync_on"));
    r.settle();
    // The knob is the note value now: its readout names it.
    assert!(
        find_in(&r, "1/8\u{b7}", below).is_some(),
        "the knob reads the note"
    );
    // A new tempo: what is drawn and said follows (the dotted eighth at 100 BPM is 450 ms).
    r.app.exec(Command::SetTempo { bpm: 100.0 });
    r.settle();
    let words: Vec<&str> = r.texts.iter().map(|p| p.text.as_str()).collect();
    assert!(
        words.iter().any(|w| w.contains("450 ms")),
        "the time at 100 BPM is said: {words:?}"
    );
    // Free: the time it was playing, in milliseconds, as one step.
    let free = find_in(&r, "Free", below).expect("the free side of the switch");
    r.click(free.center());
    let off = spec(&r, strip, effect);
    assert_eq!((off.get("sync"), off.get("time_ms")), (0.0, 450.0));
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.delay_sync_off"));
    r.app.undo();
    let again = spec(&r, strip, effect);
    assert_eq!(
        (again.get("sync"), again.get("note")),
        (1.0, 10.0),
        "undone: synced"
    );
    // A drag across the echoes, synced, steps the note value.
    let a = pos2(d.left() + d.width() * 0.2, d.top() + d.height() * 0.3);
    r.drag(a, a + vec2(80.0, 0.0));
    assert!(
        spec(&r, strip, effect).get("note") > 10.0,
        "a longer note under the pointer"
    );
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.delay_echoes"));
    r.shot("delay-window-synced");
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
    for words in [
        "Ecos",
        "Lo que se oye",
        "Tiempo",
        "Repeticiones",
        "Salida",
        "Libre",
        "Al tempo",
    ] {
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

/// Review mid-round 40, C-9: the note value's list open, in English and in Spanish — every
/// value whole (nothing cut with "…"), one row each, none over another; the pictures when
/// `CANTUS_PATHS_SHOTS` names a folder (`delay-notes-en.png`, `delay-notes-es.png`).
#[test]
fn the_note_list_reads_whole_in_both_languages() {
    for (locale, tag) in [
        (cantus_ui::i18n::Locale::En, "en"),
        (cantus_ui::i18n::Locale::Es, "es"),
    ] {
        let (mut r, strip, effect) = rig();
        cantus_ui::i18n::set_thread_locale(locale);
        r.settle();
        let d = open(&mut r, strip, effect);
        let below = Rect::from_min_max(pos2(0.0, d.bottom() + 20.0), pos2(SIZE.x, SIZE.y));
        let field = find_in(&r, "1/8\u{b7}", below).expect("the note field");
        r.click(field.center());
        r.settle();
        r.shot(&format!("delay-notes-{tag}"));
        let rows: Vec<Rect> = r
            .texts
            .iter()
            .filter(|p| p.text.trim_start().starts_with("1/") && p.seen != field)
            .map(|p| p.seen)
            .collect();
        let all: Vec<&str> = r.texts.iter().map(|p| p.text.as_str()).collect();
        assert!(
            rows.len() >= 12,
            "{tag}: the list's values: {} in {all:?}",
            rows.len()
        );
        for p in r
            .texts
            .iter()
            .filter(|p| p.text.trim_start().starts_with("1/"))
        {
            assert!(
                !p.text.contains('…'),
                "{tag}: cut at {:?} (seen {:?}): {}",
                p.rect,
                p.seen,
                p.text
            );
        }
        for (i, a) in rows.iter().enumerate() {
            for b in &rows[i + 1..] {
                assert!(
                    a.intersect(*b).area() < 1.0,
                    "{tag}: rows over each other {a:?} {b:?}"
                );
            }
        }
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let _ = spec(&r, strip, effect);
    }
}

// Review 2 of round 40, R2-B6: in a short window (720 points) the note list stays inside
// it — above its field when it fits there, else scrolling in the window's height — never
// running off the bottom (at 900 points it reached y 898).
#[test]
fn the_note_list_stays_inside_a_short_window() {
    let (mut r, strip, effect) = rig();
    r.size = vec2(1600.0, 720.0);
    r.settle();
    let d = open(&mut r, strip, effect);
    let below = Rect::from_min_max(pos2(0.0, d.bottom() + 20.0), pos2(r.size.x, r.size.y));
    let field = find_in(&r, "1/8\u{b7}", below).expect("the note field");
    r.click(field.center());
    r.settle();
    r.shot("delay-notes-720");
    let rows: Vec<Rect> = r
        .texts
        .iter()
        .filter(|p| p.text.trim_start().starts_with("1/") && p.seen != field)
        .filter(|p| p.seen.area() > 0.0)
        .map(|p| p.seen)
        .collect();
    assert!(
        rows.len() >= 12,
        "the list's values in view: {}",
        rows.len()
    );
    for row in &rows {
        assert!(
            row.top() >= 0.0 && row.bottom() <= r.size.y,
            "a value outside the window: {row:?}"
        );
    }
    // A value still chosen with a click: the list works where it now lies.
    let pick = rows[rows.len() / 2];
    r.click(pick.center());
    r.settle();
    assert!(
        spec(&r, strip, effect).get("sync") >= 0.5,
        "a value chosen turns the sync on"
    );
}
