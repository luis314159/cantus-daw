//! Round 40's instruments read whole at every editor scale (fx/final's site pictures: at
//! 200 % Chorda's and Tympanum's views lost most of their labels). Its own test binary:
//! the scenes paint on this thread.
//!
//! The app made by `CantusApp::assemble`; its editor hosted at its size for the scale, as
//! a window sized to it shows it (and as the site's pictures are taken): Chorda (Play,
//! Mixer, Edit, Matrix) and Tympanum (Kit, Mixer, Matrix) at the editor's 100 %, 150 % and
//! 200 %, and at the narrowest window the editor allows at each (three quarters of its
//! size) — the rules are `inst_scale`'s, shared with `instruments_narrow_e2e` (every other
//! kind):
//!
//! - every word painted whole at 100 % is painted whole at the other scales;
//! - none is cut — elided, dropped, or painted past its card (`kit::text::probe`'s
//!   clipped probe);
//! - no two overlap, and none runs under a button (its own label inside it aside);
//!
//! in English and in Spanish.

mod inst_scale;
mod paths_rig;

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, TrackId};
use cantus_ui::kit::shell::{self, Shell};
use cantus_ui::kit::text::probe;
use cantus_ui::views::inst::{self, View};
use egui::{Rect, Vec2};
use inst_scale::*;
use paths_rig::*;

fn song() -> (Project, TrackId, TrackId) {
    let mut p = cantus_ui::welcome_project();
    let mut ids = Vec::new();
    for (name, kind) in [
        ("Piano", InstrumentKind::Chorda),
        ("Drums", InstrumentKind::AcousticDrums),
    ] {
        let mut t = p.make_track(name);
        let strip = p.make_strip(name);
        t.strip = Some(strip.id);
        p.strips.push(strip);
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        ids.push(t.id);
        p.tracks.push(t);
    }
    (p, ids[0], ids[1])
}

/// The editor's size at 100 % where `view` lays itself out whole: the shell's, or more
/// where the view asks for more (below its least size it scrolls, by design — a row half
/// out of sight there is no cut).
fn whole(kind: InstrumentKind, view: View) -> Vec2 {
    let least = match (kind, view) {
        (InstrumentKind::Chorda, View::Mixer) => inst::chorda_source::mixer::MIN,
        (InstrumentKind::AcousticDrums, View::Mixer) => {
            Vec2::new(inst::tympanum::MIXER_MIN_W, 560.0)
        }
        (InstrumentKind::AcousticDrums, _) => inst::tympanum::KIT_MIN,
        // Every part with its picture: taller than the window, it scrolls there.
        (InstrumentKind::Chorda, View::Edit) => Vec2::new(0.0, 1250.0),
        // r42:chorda-presets — every group of the voicing, with its picture.
        (InstrumentKind::Chorda, View::Voicing) => Vec2::new(0.0, inst::chorda::VOICING_H),
        // Its source list whole, unscrolled (at the shell's height it scrolls).
        (InstrumentKind::Chorda, View::Play) => Vec2::new(0.0, 800.0),
        _ => Vec2::ZERO,
    };
    inst::size_for(least)
}

#[test]
fn chorda_and_tympanum_read_whole_at_every_scale_and_the_narrowest_window() {
    let _theme_turn = theme_turn();
    let _turn = turn();
    inst::chorda::scene::set_blocking(true);
    let (p, chorda, tymp) = song();
    let mut r = Rig::new(p);
    r.idle(4);
    let mut h = Host::new();
    let mut bad: Vec<String> = Vec::new();
    let cases = [
        Case {
            track: chorda,
            kind: InstrumentKind::Chorda,
            name: "chorda",
            views: views_of(InstrumentKind::Chorda),
        },
        Case {
            track: tymp,
            kind: InstrumentKind::AcousticDrums,
            name: "tympanum",
            views: views_of(InstrumentKind::AcousticDrums),
        },
    ];
    let read = read_views(&mut r, &mut h, &cases, whole, &mut bad);
    assert!(
        read > 300,
        "only {read} words were read:
{}",
        bad.join(
            "
"
        )
    );
    assert!(
        bad.is_empty(),
        "{} problems:
{}",
        bad.len(),
        bad.join(
            "
"
        )
    );
}

/// Each instrument's editor opened fresh in the assembled app opens at its own size, its
/// default view uncut at the window's edge (fx/final: Tympanum's Kit at the shell's
/// 1 040 × 680 ran past the window's right and foot). A list inside it may scroll.
#[test]
fn an_instrument_opens_at_its_own_size_its_view_uncut() {
    let _theme_turn = theme_turn();
    let _turn = turn();
    inst::chorda::scene::set_blocking(true);
    let (p, chorda, tymp) = song();
    let mut r = Rig::new(p);
    r.idle(4);
    let mut bad = Vec::new();
    for (track, kind) in [
        (chorda, InstrumentKind::Chorda),
        (tymp, InstrumentKind::AcousticDrums),
    ] {
        r.app.open_inst_editor(track);
        r.idle(12);
        let key = inst::editor_id(track).value();
        let Some((ed, _)) = cantus_ui::views::workspace_device::shown(&r.ctx, key) else {
            bad.push(format!("{kind:?}: the editor is not drawn"));
            continue;
        };
        r.shot(&format!("opened-{kind:?}"));
        let want = inst::editor_size(kind);
        if ed.width() + 0.5 < want.x || ed.height() + 0.5 < want.y {
            bad.push(format!(
                "{kind:?}: opened at {:?}, not its {want:?}",
                ed.size()
            ));
        }
        // No word cut — at the editor's edge, or by the view scrolling because the window
        // is short of its whole size. Only the lists at the left (Chorda's sources,
        // Tympanum's kits) scroll by design.
        let list = Rect::from_min_size(ed.min, egui::vec2(260.0, ed.height()));
        for t in &r.texts {
            if t.text.trim().is_empty() || !t.rect.intersects(ed) || t.seen == t.rect {
                continue;
            }
            let in_list = list.contains_rect(t.rect);
            if !ed.contains_rect(t.rect) || !in_list {
                bad.push(format!(
                    "{kind:?}: “{}” cut at the editor's edge: {:?} of {:?}",
                    t.text.trim(),
                    t.seen,
                    t.rect
                ));
            }
        }
        // Every card whole, its edge and foot included, in the view it opens in and in its
        // Mixer — not only its words (fx/final: Tympanum's Kit, 16 points short, scrolled
        // and lost its cards' foot while every word still read whole).
        for view in [View::Play, View::Mixer] {
            r.app.inst.view = view;
            r.idle(4);
            probe::start_cards();
            r.idle(1);
            let cards = probe::take_cards();
            r.shot(&format!("cards-{kind:?}-{view:?}"));
            let mut seen = 0;
            for (card, clip) in cards {
                if !card.intersects(ed) || !clip.intersects(card) {
                    continue;
                }
                // A list's own scroll (Chorda's sources): its rows half out of sight by
                // design — a clip inside the list's column.
                if list.contains_rect(clip) {
                    continue;
                }
                seen += 1;
                if !ed.expand(0.5).contains_rect(card) || !clip.expand(0.5).contains_rect(card) {
                    bad.push(format!(
                        "{kind:?} {view:?}: a card at {card:?} is cut — the editor {ed:?}, \
                         its clip {clip:?}"
                    ));
                }
            }
            if seen < 3 {
                bad.push(format!("{kind:?} {view:?}: only {seen} cards painted"));
            }
        }
        r.app.inst.view = View::Play;
        r.app.close_inst_editor();
        r.idle(4);
    }
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// The shell's header never cuts an instrument's name: at the narrowest window the editor
/// allows, every instrument's name is painted whole, in English and Spanish — the
/// track's part and the other secondary items give way first.
#[test]
fn the_header_keeps_every_instruments_name_whole_at_the_narrowest_window() {
    let _theme_turn = theme_turn();
    let _turn = turn();
    inst::chorda::scene::set_blocking(true);
    let mut p = cantus_ui::welcome_project();
    let mut tracks = Vec::new();
    for kind in InstrumentKind::ALL {
        let mut t = p.make_track("A long track name");
        let strip = p.make_strip("A long track name");
        t.strip = Some(strip.id);
        p.strips.push(strip);
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        tracks.push((kind, t.id));
        p.tracks.push(t);
    }
    let mut r = Rig::new(p);
    r.idle(4);
    let mut h = Host::new();
    let mut bad = Vec::new();
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for &(kind, track) in &tracks {
            r.app.open_inst_editor(track);
            Shell::new(("inst", track.0)).set_scale(&h.ctx, 1.0);
            let shot = format!("header-{kind:?}-narrowest-{}", lang.code());
            let (words, cuts, _) = host(&mut r, &mut h, shell::INSTRUMENT * 0.75, false, &shot);
            let name = cantus_ui::i18n::t(kind.name_key());
            let whole = words.iter().any(|w| {
                w.text.eq_ignore_ascii_case(&name)
                    && w.seen == w.rect
                    && w.rect.top() < shell::HEADER
            });
            if !whole {
                let seen: Vec<&str> = words
                    .iter()
                    .filter(|w| w.rect.top() < shell::HEADER)
                    .map(|w| w.text.as_str())
                    .collect();
                bad.push(format!(
                    "{lang:?} {kind:?}: “{name}” not whole in the header: {seen:?}"
                ));
            }
            for c in cuts.iter().filter(|c| c.rect.top() < shell::HEADER - 0.5) {
                bad.push(format!(
                    "{lang:?} {kind:?}: the header cut {:?} -> {:?} ({})",
                    c.full, c.shown, c.how
                ));
            }
            r.app.close_inst_editor();
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
