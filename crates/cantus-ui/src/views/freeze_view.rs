//! Freezing on screen (round 29, ADR 0064): a snowflake on a frozen track's header and a
//! frozen strip's head (dimmed on what a frozen strip holds), its clips under a frosted
//! veil, the inspector's *Freeze* row, the track menu's and the strip menu's entries, and
//! the corner card — a render on its way with its Stop, an edit a freeze refused with its
//! Unfreeze.

use crate::app::CantusApp;
use crate::bounce::{Plan, Range, Through};
use crate::i18n::{t, tf};
use crate::kit::color::with_alpha;
use crate::kit::glyph::{self, Glyph};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{Brush, Fill, Style};
use cantus_core::freeze::{FreezeTarget, Locks};
use cantus_core::{StripId, TrackId};
use egui::{Rect, Vec2};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

/// What the freezes hold, worked out once per project change.
#[derive(Debug, Default)]
pub struct Held {
    /// Frozen themselves.
    pub strips: BTreeSet<StripId>,
    pub tracks: BTreeSet<TrackId>,
    /// Held by a freeze (frozen, or inside a frozen strip's region): locked.
    pub held_strips: BTreeSet<StripId>,
    /// Tracks whose clips are in a render (their source is locked).
    pub held_tracks: BTreeSet<TrackId>,
}

thread_local! {
    static HELD: RefCell<(u64, u64, Rc<Held>)> = RefCell::new((u64::MAX, u64::MAX, Rc::default()));
}

/// What the freezes hold now (cached by revision).
pub fn held(app: &CantusApp) -> Rc<Held> {
    let rev = app.bus.revision();
    let epoch = app.epoch;
    HELD.with(|h| {
        let mut h = h.borrow_mut();
        if h.0 != rev || h.1 != epoch {
            let p = app.project();
            let mut out = Held::default();
            if p.any_frozen() {
                for t in p.frozen_targets() {
                    match t {
                        FreezeTarget::Strip(s) => {
                            out.strips.insert(s);
                        }
                        FreezeTarget::Track(t) => {
                            out.tracks.insert(t);
                        }
                    }
                }
                let l = Locks::of(p);
                out.held_strips = l.chain.clone();
                out.held_tracks = l.source.clone();
            }
            *h = (rev, epoch, Rc::new(out));
        }
        h.2.clone()
    })
}

/// The snowflake's ink: the theme's focus hue.
fn ice(s: &Style) -> egui::Color32 {
    s.t.focus
}

/// A frozen track's header: a snowflake badge at the icon's top-right corner (its own
/// freeze, or the strip it feeds — the lit one; a track only held, dim).
pub fn paint_header_frozen(
    painter: &egui::Painter,
    icon: Rect,
    app: &CantusApp,
    track: TrackId,
    s: &Style,
) {
    let h = held(app);
    if !h.held_tracks.contains(&track) {
        return;
    }
    let own = h.tracks.contains(&track)
        || app
            .project()
            .track(track)
            .and_then(|t| t.strip)
            .is_some_and(|st| h.strips.contains(&st));
    let d = icon.width().min(icon.height()) * 0.62;
    let badge = Rect::from_center_size(
        icon.right_top() + Vec2::new(-d * 0.18, d * 0.18),
        Vec2::splat(d),
    );
    crate::kit::with_brush(painter, |b| {
        b.circle(badge.center(), d * 0.5, Fill::Solid(s.t.panel));
        let ink = if own {
            ice(s)
        } else {
            with_alpha(s.t.text_dim, 0.8)
        };
        glyph::paint(b, badge.shrink(d * 0.08), Glyph::Frozen, ink);
    });
}

/// A frozen track's clip: a frosted veil (the theme's focus hue, faint) and a snowflake
/// in its corner when there is room.
pub fn veil_clip(b: &mut Brush, body: Rect, lanes: Rect, s: &Style) {
    let r = body.intersect(lanes);
    if r.width() <= 1.0 || r.height() <= 1.0 {
        return;
    }
    b.rect(r, Fill::Solid(with_alpha(s.t.panel, 0.45)));
    b.rect(r, Fill::Solid(with_alpha(ice(s), 0.10)));
    let g = s.s(11.0);
    if body.width() > g * 3.0 && body.height() > g * 1.4 && body.right() - g * 1.3 > lanes.left() {
        let gr = Rect::from_min_size(
            egui::pos2(body.right() - g * 1.3, body.top() + g * 0.25),
            Vec2::splat(g),
        );
        if lanes.contains_rect(gr) {
            glyph::paint(b, gr, Glyph::Frozen, ice(s));
        }
    }
}

/// A strip's head: the snowflake in its number cell's corner — lit when it is frozen, dim
/// when a frozen strip holds it.
pub fn paint_strip_mark(b: &mut Brush, cell: Rect, app: &CantusApp, strip: StripId, s: &Style) {
    let h = held(app);
    if !h.held_strips.contains(&strip) {
        return;
    }
    let lit = h.strips.contains(&strip);
    let d = (cell.height() * 0.62).min(s.s(12.0));
    let gr = Rect::from_min_size(
        egui::pos2(
            cell.right() - d - s.s(2.0),
            cell.top() + (cell.height() - d) * 0.5,
        ),
        Vec2::splat(d),
    );
    let ink = if lit {
        ice(s)
    } else {
        with_alpha(s.t.text_dim, 0.8)
    };
    glyph::paint(b, gr, Glyph::Frozen, ink);
}

// ---- the corner card ----------------------------------------------------------------

/// What the card's buttons do.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Stop,
    Unfreeze(FreezeTarget),
    Dismiss,
}

/// The card, when there is one: a render on its way, or an edit a freeze refused (the
/// newest wins). Title, body, buttons, and what ✕ does.
pub fn card(app: &CantusApp) -> Option<(String, String, Vec<(String, Answer)>, Answer)> {
    if let Some(j) = &app.freeze.job {
        let pct = (j.progress.fraction() * 100.0).round() as u32;
        let (title, body) = match &j.want {
            crate::freeze::Want::Freeze(_) => (
                tf("freeze.card.freezing", &[("name", &j.name)]),
                tf("freeze.card.freezing_body", &[("pct", &pct)]),
            ),
            crate::freeze::Want::Refresh(_) => (
                tf("freeze.card.refreshing", &[("name", &j.name)]),
                tf("freeze.card.refreshing_body", &[("pct", &pct)]),
            ),
            crate::freeze::Want::Bounce(_) => (
                j.name.clone(),
                tf("freeze.card.bouncing_body", &[("pct", &pct)]),
            ),
        };
        return Some((
            title,
            body,
            vec![(t("freeze.card.stop"), Answer::Stop)],
            Answer::Stop,
        ));
    }
    let (target, at) = app.freeze.refusal?;
    if at.elapsed() > std::time::Duration::from_secs(12) || app.project().frozen(target).is_none() {
        return None;
    }
    let name = crate::freeze::name_of(app.project(), target);
    Some((
        tf("freeze.card.refused", &[("name", &name)]),
        t("freeze.card.refused_body"),
        vec![
            (t("freeze.unfreeze"), Answer::Unfreeze(target)),
            (t("freeze.card.ok"), Answer::Dismiss),
        ],
        Answer::Dismiss,
    ))
}

pub fn answer(app: &mut CantusApp, a: Answer) {
    match a {
        Answer::Stop => app.cancel_freeze(),
        Answer::Unfreeze(target) => {
            app.freeze.refusal = None;
            app.unfreeze_target(target);
        }
        Answer::Dismiss => app.freeze.refusal = None,
    }
}

// ---- the inspector -----------------------------------------------------------------

/// The inspector rows' acts.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InspAct {
    Freeze(FreezeTarget),
    Unfreeze(FreezeTarget),
    LowLatency(bool),
}

pub fn apply_insp(app: &mut CantusApp, a: InspAct) {
    match a {
        InspAct::Freeze(t) => app.freeze_target(t),
        InspAct::Unfreeze(t) => app.unfreeze_target(t),
        InspAct::LowLatency(on) => super::freeze_monitor::set(app, on),
    }
}

// ---- menus --------------------------------------------------------------------------

/// The track menu's *Freeze* and *Bounce in place* entries (`arrangement::track_menu`).
/// Returns whether one was chosen.
pub fn track_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    let Some(track) = app.project().track(id) else {
        return false;
    };
    let has_strip = track.strip.is_some();
    let holder = app.freeze_holding_track(id);
    let freeze_label = match holder {
        Some(_) => Src::Key("freeze.menu.unfreeze"),
        None => Src::Key("freeze.menu.freeze"),
    };
    let bounce_ok = crate::bounce::refusal(app, id).is_none();
    let removes = app.settings.bounce_removes;
    let key = app
        .keymap
        .bindings_for(crate::keymap::Action::FreezeTrack)
        .first()
        .map(|b| b.display());
    let items = [
        Item::Band(Src::Key("freeze.menu.band")),
        Item::Action {
            label: freeze_label,
            check: Check::None,
            shortcut: key.as_deref(),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("bounce.menu.inserts"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: bounce_ok && has_strip,
        },
        Item::Action {
            label: Src::Key("bounce.menu.source"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: bounce_ok,
        },
        Item::Action {
            label: Src::Key("bounce.menu.removes"),
            check: if removes { Check::On } else { Check::Off },
            shortcut: None,
            submenu: false,
            enabled: true,
        },
    ];
    match crate::kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
        Some(1) => app.toggle_freeze_track(id),
        Some(2) => app.bounce(Plan {
            track: id,
            through: Through::Inserts,
            range: Range::Track,
        }),
        Some(3) => app.bounce(Plan {
            track: id,
            through: Through::Source,
            range: Range::Track,
        }),
        Some(4) => {
            app.settings.bounce_removes = !removes;
            app.store_settings();
        }
        _ => return false,
    }
    true
}

/// The keys' and the clip menu's bounce: the selected clips on their track when some are
/// selected, else the time selection on the selected track, else the whole track.
pub fn bounce_selection(app: &mut CantusApp, through: Through) {
    let sel = app.clip_selection();
    if let Some((track, _)) = sel.first() {
        let track = *track;
        let clips = sel
            .iter()
            .filter(|(t, _)| *t == track)
            .map(|(_, c)| *c)
            .collect();
        app.bounce(Plan {
            track,
            through,
            range: Range::Clips(clips),
        });
        return;
    }
    let Some(track) = app.selected_track else {
        app.set_status(t("bounce.no_track"));
        return;
    };
    let range = match app.time_selection {
        Some((a, b)) if b > a => Range::Time(a, b),
        _ => Range::Track,
    };
    app.bounce(Plan {
        track,
        through,
        range,
    });
}

/// The keymap's actions (`CantusApp::run_action`'s pre-hook): whether `action` was one.
pub fn action(app: &mut CantusApp, action: crate::keymap::Action) -> bool {
    use crate::keymap::Action;
    match action {
        Action::FreezeTrack => match app.selected_track {
            Some(t) => app.toggle_freeze_track(t),
            None => match app.selected_strip {
                Some(s) => match app.freeze_holding_strip(s) {
                    Some(h) => app.unfreeze_target(h),
                    None => app.freeze_target(FreezeTarget::Strip(s)),
                },
                None => app.set_status(t("bounce.no_track")),
            },
        },
        Action::BounceInPlace => bounce_selection(app, Through::Inserts),
        Action::BounceSource => bounce_selection(app, Through::Source),
        Action::ToggleLowLatency => {
            let on = !app.settings.low_latency;
            super::freeze_monitor::set(app, on);
        }
        _ => return false,
    }
    true
}
