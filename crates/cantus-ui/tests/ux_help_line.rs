//! The help line is never empty (round 39, ADR 0109): every control a pointer can rest on
//! gives the hint bar and the help line a sentence. The app `CantusApp::assemble` puts
//! together on the demo it opens with; every view a user reaches (the main window, each
//! pane, every instrument and effect editor, Settings, the first-run setup, the export
//! dialog, the top bar's menus); the pointer on each widget that takes a click or a drag
//! and that nothing covers — and a widget whose hover writes no hint fails, named by the
//! words painted on it.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

#[path = "ux_sweep/mod.rs"]
mod sweep;

use egui::{Event, Rect};
use std::collections::HashSet;
use sweep::{Look, Rig, View};

/// A widget whose hover wrote no hint: the view, where, and the words on it.
#[derive(Debug)]
struct Gap {
    view: String,
    rect: Rect,
    words: String,
}

/// Every widget of `view` the pointer can rest on, hovered in turn; the ones without a
/// hint. `seen` skips the widgets an earlier view already checked.
fn walk(r: &mut Rig, view: &View, seen: &mut HashSet<egui::Id>) -> (usize, Vec<Gap>) {
    r.reset();
    sweep::show(r, view);
    let texts = r.idle(2);
    let screen = Rect::from_min_size(egui::Pos2::ZERO, r.size);
    let widgets = sweep::interactive(&r.ctx, screen);
    let mut gaps = Vec::new();
    let mut checked = 0;
    let (mut dup, mut covered) = (0, 0);
    for (id, layer, rect) in widgets {
        // A canvas the size of a pane speaks for its tool, not as a control; the debug
        // and tooltip layers are not the user's.
        let big = rect.width() > r.size.x * 0.45 && rect.height() > r.size.y * 0.3;
        // A window's or a popup's own ground (egui moves or raises it there) is not a
        // control.
        let ground = id == layer.id.with("move");
        // A control scrolled almost out of its view shows a sliver no one rests on.
        let sliver = rect.width() < 8.0 || rect.height() < 8.0;
        if big
            || ground
            || sliver
            || matches!(layer.order, egui::Order::Tooltip | egui::Order::Debug)
        {
            continue;
        }
        if !seen.insert(id) {
            dup += 1;
            continue;
        }
        let at = rect.center();
        r.frame_at(None, vec![Event::PointerGone]);
        r.frame(Some(at));
        r.frame(Some(at));
        // Something else lies over it here (a window, a popup): not this widget's hover.
        if !sweep::hovered(&r.ctx, id) {
            covered += 1;
            continue;
        }
        checked += 1;
        if cantus_ui::kit::hint::current(&r.ctx).is_none() {
            let words: Vec<&str> = texts
                .iter()
                .filter(|p| rect.expand(2.0).contains_rect(p.rect))
                .map(|p| p.text.as_str())
                .collect();
            eprintln!("  gap layer {layer:?} id {id:?}");
            gaps.push(Gap {
                view: format!("{view:?}"),
                rect,
                words: words.join(" | "),
            });
        }
    }
    eprintln!("  {view:?}: {dup} seen before, {covered} covered");
    (checked, gaps)
}

#[test]
fn every_control_a_pointer_rests_on_gives_the_help_line_a_sentence() {
    let mut r = Rig::new(Look::PLAIN);
    let mut seen = HashSet::new();
    let mut gaps = Vec::new();
    let mut checked = 0;
    for view in sweep::views() {
        let (n, g) = walk(&mut r, &view, &mut seen);
        eprintln!("{view:?}: {n} controls, {} without a hint", g.len());
        checked += n;
        gaps.extend(g);
    }
    for g in &gaps {
        eprintln!(
            "GAP {} at ({:.0}, {:.0}) {:.0}x{:.0}: {:?}",
            g.view,
            g.rect.left(),
            g.rect.top(),
            g.rect.width(),
            g.rect.height(),
            g.words
        );
    }
    assert!(checked > 500, "the walk reached {checked} controls");
    assert!(
        gaps.is_empty(),
        "{} of {checked} controls give the help line nothing (listed above)",
        gaps.len()
    );
}

/// Every view photographed into `CANTUS_UX_SHOTS` (a folder), to look at what the walk
/// walks: `cargo test -p cantus-ui --test ux_help_line -- --ignored shoot`.
#[test]
#[ignore]
fn shoot_every_view() {
    let dir = std::path::PathBuf::from(
        std::env::var("CANTUS_UX_SHOTS").unwrap_or_else(|_| "ux-shots".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = Rig::new(Look::PLAIN);
    for (i, view) in sweep::views().iter().enumerate() {
        r.reset();
        sweep::show(&mut r, view);
        let name = format!("{i:02}-{view:?}.png")
            .replace(['(', ')', '"', ' '], "")
            .to_lowercase();
        sweep::shoot(&mut r, &dir.join(name), None);
    }
}
