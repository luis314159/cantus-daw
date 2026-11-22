//! The status line says a short hint, never a dialog's text (round 41: the launch video's
//! frames showed "Replace the instrument on Piano S M L Curated Name Vendor … Search 13
//! devices …" along the bottom of the window while the instrument picker was open). The
//! app `CantusApp::assemble` puts together on the demo it opens with; each dialog, picker
//! and menu opened the way the interface opens it; the pointer on the ground of every
//! widget there that holds the words of several things (a dialog's panel, its scrim, a
//! list) — and the status line at the window's foot, as painted, never spells three of
//! the words under the pointer: a container gives the help line nothing of its children's,
//! only a control with its own hint does.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

#[path = "ux_sweep/mod.rs"]
mod sweep;

use cantus_ui::i18n::{t, Locale};
use cantus_ui::picker::PickPurpose;
use cantus_ui::preset_audition::Target;
use egui::{Order, Pos2, Rect};
use std::collections::BTreeSet;
use sweep::{Look, Painted, Rig, View};

/// A dialog, picker or menu a user opens.
#[derive(Clone, Debug)]
enum Dialog {
    /// The device picker on the first track: "Replace the instrument on …".
    ReplaceInstrument,
    /// The device picker on the first strip: a new insert.
    AddEffect,
    /// The preset browser on the first track's instrument.
    Presets,
    /// One of the sweep's views: Settings, the first-run setup, the start page, the export
    /// dialog, the top bar's menus.
    View(View),
}

fn open(r: &mut Rig, d: &Dialog) {
    r.reset();
    match d {
        Dialog::ReplaceInstrument => {
            let track = r.app.project().tracks[0].id;
            r.app.open_picker(PickPurpose::ReplaceInstrument { track });
        }
        Dialog::AddEffect => {
            let strip = r.app.project().tracks[0].strip.expect("a strip");
            r.app.open_picker(PickPurpose::AddEffect { strip });
        }
        Dialog::Presets => {
            let track = r
                .app
                .project()
                .tracks
                .iter()
                .find(|t| matches!(t.instrument, cantus_core::InstrumentSpec::Native(_)))
                .expect("a native instrument in the demo")
                .id;
            r.app.open_preset_browser(Target::Track(track));
            assert!(r.app.presets.open, "the preset browser opened");
        }
        Dialog::View(v) => sweep::show(r, v),
    }
    r.idle(4);
}

/// The status bar's rect: the window's foot, as tall as `workspace_status` makes it.
fn status_rect(r: &Rig) -> Rect {
    let s = cantus_ui::kit::Style::current();
    let h = (cantus_ui::views::workspace_status::HEIGHT * s.t.density).round();
    Rect::from_min_max(Pos2::new(0.0, r.size.y - h), r.size.to_pos2())
}

/// What the status line paints: every word inside the status bar, in order.
fn status_words(r: &Rig, texts: &[Painted]) -> String {
    let bar = status_rect(r).expand(1.0);
    let mut out = String::new();
    for p in texts.iter().filter(|p| bar.contains_rect(p.rect)) {
        out.push_str(&p.text);
        out.push(' ');
    }
    out
}

/// The distinct words painted inside `rect` (above the status bar): what a container
/// holds.
fn held(r: &Rig, texts: &[Painted], rect: Rect) -> BTreeSet<String> {
    let bar = status_rect(r);
    texts
        .iter()
        .filter(|p| rect.expand(1.0).contains_rect(p.rect) && !bar.intersects(p.rect))
        .map(|p| p.text.trim().to_string())
        .filter(|w| w.chars().count() >= 2)
        .collect()
}

/// Whether `said` holds `w` whole: not inside a longer word or key ("Ctrl+S" is not in
/// "Ctrl+Shift+N", "Alt+A" not in "Shift+Alt+A").
fn holds_whole(said: &str, w: &str) -> bool {
    let joins = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '+');
    said.match_indices(w).any(|(i, _)| {
        !joins(said[..i].chars().next_back()) && !joins(said[i + w.len()..].chars().next())
    })
}

/// Which of `held` the words `said` spell out, whole.
fn spelled(said: &str, held: &BTreeSet<String>) -> Vec<String> {
    held.iter()
        .filter(|w| holds_whole(said, w))
        .cloned()
        .collect()
}

/// A point of the help line that spelled out a container's words.
#[derive(Debug)]
#[allow(dead_code)]
struct Fault {
    dialog: String,
    at: Pos2,
    said: String,
    spelled: Vec<String>,
}

/// Every foreground widget that holds the words of several things, and up to three points of
/// its ground (on none of its own widgets): `(id, rect, points)`.
fn grounds(r: &Rig, texts: &[Painted]) -> Vec<(egui::Id, Rect, Vec<Pos2>)> {
    let screen = Rect::from_min_size(Pos2::ZERO, r.size);
    let widgets = sweep::interactive(&r.ctx, screen);
    let mut out = Vec::new();
    for (id, layer, rect) in &widgets {
        if layer.order != Order::Foreground || held(r, texts, *rect).len() < 3 {
            continue;
        }
        let inner: Vec<Rect> = widgets
            .iter()
            .filter(|(o, l, rr)| o != id && l == layer && rect.contains_rect(*rr))
            .map(|(_, _, rr)| rr.expand(2.0))
            .collect();
        let mut points = Vec::new();
        'grid: for j in 1..8 {
            for i in 1..8 {
                let p = Pos2::new(
                    rect.left() + rect.width() * i as f32 / 8.0,
                    rect.top() + rect.height() * j as f32 / 8.0,
                );
                if !status_rect(r).contains(p) && !inner.iter().any(|q| q.contains(p)) {
                    points.push(p);
                    if points.len() == 3 {
                        break 'grid;
                    }
                }
            }
        }
        out.push((*id, *rect, points));
    }
    out
}

/// Open `d`, rest the pointer on the ground of each container it shows; the points where
/// the status line or the hint spelled out three of the words under it. Returns how many
/// points were looked at, too.
fn check(r: &mut Rig, d: &Dialog) -> (usize, Vec<Fault>) {
    open(r, d);
    let texts = r.idle(2);
    let mut faults = Vec::new();
    let mut looked = 0;
    for (id, rect, points) in grounds(r, &texts) {
        for p in points {
            r.frame(Some(p));
            let texts = r.frame(Some(p));
            // Something else lies over it here: not this ground.
            if !sweep::hovered(&r.ctx, id) {
                continue;
            }
            looked += 1;
            let held = held(r, &texts, rect);
            let painted = status_words(r, &texts);
            let hint = cantus_ui::kit::hint::current(&r.ctx)
                .map(|h| format!("{} {}", h.name, h.value))
                .unwrap_or_default();
            for said in [painted, hint] {
                let s = spelled(&said, &held);
                if s.len() >= 3 {
                    faults.push(Fault {
                        dialog: format!("{d:?}"),
                        at: p,
                        said: said.clone(),
                        spelled: s,
                    });
                }
            }
        }
    }
    (looked, faults)
}

#[test]
fn the_instrument_pickers_ground_leaves_its_words_off_the_status_line() {
    for locale in [Locale::En, Locale::Es] {
        let mut r = Rig::new(Look {
            locale,
            ..Look::PLAIN
        });
        open(&mut r, &Dialog::ReplaceInstrument);
        assert!(r.app.picker.is_some(), "{locale:?}: the picker opened");
        let panel =
            sweep::widget_rect(&r.ctx, egui::Id::new("picker-panel")).expect("the picker's panel");
        // The panel's own ground, on none of its controls (the launch video's frames were
        // taken with the pointer there).
        let texts = r.idle(2);
        let (_, _, points) = grounds(&r, &texts)
            .into_iter()
            .find(|(id, _, _)| *id == egui::Id::new("picker-panel"))
            .expect("the panel holds the picker's words");
        assert!(!points.is_empty(), "the panel has a ground to rest on");
        let title = t("picker.title.replace_instrument");
        let search = t("picker.search");
        let search = search
            .split('{')
            .next()
            .unwrap_or(&search)
            .trim()
            .to_string();
        for p in points {
            assert!(panel.contains(p));
            r.frame(Some(p));
            let texts = r.frame(Some(p));
            assert!(
                sweep::hovered(&r.ctx, egui::Id::new("picker-panel")),
                "{locale:?}: the pointer rests on the panel at {p:?}"
            );
            let said = status_words(&r, &texts);
            let head = title.split('{').next().unwrap_or(&title).trim();
            assert!(
                !said.contains(head) && !said.contains(search.as_str()),
                "{locale:?}: the status line spells the picker at {p:?}: {said:?}"
            );
            let held = held(&r, &texts, panel);
            assert!(
                spelled(&said, &held).len() < 2,
                "{locale:?}: the status line repeats the picker's words at {p:?}: {said:?}"
            );
        }
    }
}

#[test]
fn no_dialogs_ground_spells_its_words_on_the_status_line() {
    let mut r = Rig::new(Look::PLAIN);
    let mut dialogs = vec![
        Dialog::ReplaceInstrument,
        Dialog::AddEffect,
        Dialog::Presets,
        Dialog::View(View::Settings),
        Dialog::View(View::FirstRun),
        Dialog::View(View::StartPage),
        Dialog::View(View::Export),
    ];
    dialogs.extend((0..sweep::MENUS).map(|i| Dialog::View(View::Menu(i))));
    let mut faults = Vec::new();
    let mut looked = 0;
    for d in &dialogs {
        let (n, f) = check(&mut r, d);
        eprintln!("{d:?}: {n} points, {} spelled", f.len());
        // The pickers and the browser are where the frames showed it: their grounds must
        // be reached. The others are walked where they have one in the foreground.
        if matches!(
            d,
            Dialog::ReplaceInstrument | Dialog::AddEffect | Dialog::Presets
        ) {
            assert!(n > 0, "{d:?}: no container's ground was reached");
        }
        looked += n;
        faults.extend(f);
    }
    for f in &faults {
        eprintln!("FAULT {f:?}");
    }
    assert!(looked > 40, "the walk rested on {looked} grounds");
    assert!(
        faults.is_empty(),
        "{} of {looked} points spell a dialog's words on the status line (listed above)",
        faults.len()
    );
}
