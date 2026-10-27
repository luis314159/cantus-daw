//! The piano roll and the song's key (round 31, ADR 0069) — FL's *scale highlighting*,
//! Ableton 12's *scale awareness*: the key's rows lit and its root marked, the rows folded
//! to the key, drawing, moving and transposing kept in it, and the key on the toolbar.
//!
//! The piano roll calls in from a few marked lines (`piano_roll.rs`, its painter); what is
//! drawn and decided is here. A kit's rows are its pads: none of this applies to them.

use super::{Axis, Geo};
use crate::app::CantusApp;
use crate::kit::color::with_alpha;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::{pitch_name, Clip, Key, Pitch};
use egui::{pos2, Align2, Rect};

/// The key the piano roll follows while it is lit, folded or snapping.
fn key(app: &CantusApp) -> Option<Key> {
    crate::notefx_ui::key_of(app.project())
}

/// The axis the piano roll lays out: folded to the key's rows (and every row a note of
/// the clip is on, so nothing hides) when *Fold* is on and the song has a key; a kit's,
/// or a keyboard with no fold, as it is.
pub(super) fn fold(app: &CantusApp, axis: Axis, clip: &Clip) -> Axis {
    let Some(k) = key(app).filter(|_| app.notefx.fold) else {
        return axis;
    };
    if !axis.is_chromatic() {
        return axis;
    }
    let pitches: Vec<Pitch> = (0..=127u8)
        .filter(|p| k.contains(*p) || clip.notes.iter().any(|n| n.pitch == *p))
        .collect();
    let names = pitches.iter().map(|p| pitch_name(*p)).collect();
    Axis {
        pitches,
        names,
        pads: false,
    }
}

/// Whether the rows are folded to the key (a keyboard whose rows are not every pitch).
fn folded(geo: &Geo<'_>) -> bool {
    !geo.axis.pads() && !geo.axis.is_chromatic()
}

/// The grid's rows lit: outside the key a shade darker, the root a touch of the
/// selection's hue (FL's scale highlighting). Over the rows, under the lines and notes.
pub(super) fn rows(b: &mut Brush, app: &CantusApp, geo: &Geo<'_>, s: &Style) {
    let Some(k) = key(app).filter(|_| app.notefx.highlight) else {
        return;
    };
    if geo.axis.pads() {
        return;
    }
    let t = &s.t;
    let g = geo.grid;
    let lo = geo.lane(g.bottom()).floor().max(0.0) as i32;
    let hi = (geo.lane(g.top()).ceil() as i32).min(geo.axis.len() as i32);
    for lane in lo..hi {
        let pitch = geo.axis.pitch(lane as usize);
        let y1 = geo.y(lane as f32);
        let r = b.snap_rect(Rect::from_min_max(
            pos2(g.left(), y1 - geo.key_h),
            pos2(g.right(), y1),
        ));
        if k.is_root(pitch) {
            b.rect(r, Fill::Solid(with_alpha(t.selection, 0.20)));
        } else if k.contains(pitch) {
            b.rect(r, Fill::Solid(with_alpha(t.selection, 0.09)));
        } else if !k.contains(pitch) {
            b.rect(r, Fill::Solid(with_alpha(t.dark_shade, 0.22)));
        }
    }
}

/// The keyboard: folded, its rows named (the key's notes, the root lit); otherwise a mark
/// at the front of each key in the key, a wider one on the root.
pub(super) fn keys(b: &mut Brush, app: &CantusApp, geo: &Geo<'_>, rect: Rect, s: &Style) {
    let Some(k) = key(app).filter(|_| app.notefx.highlight || app.notefx.fold) else {
        return;
    };
    if geo.axis.pads() {
        return;
    }
    let t = &s.t;
    let lo = geo.lane(rect.bottom()).floor().max(0.0) as i32;
    let hi = (geo.lane(rect.top()).ceil() as i32).min(geo.axis.len() as i32);
    if folded(geo) {
        b.rect(rect, Fill::Solid(t.key_white));
        let ink = crate::kit::color::on_fill(t.key_white);
        for lane in lo..hi {
            let pitch = geo.axis.pitch(lane as usize);
            let y1 = geo.y(lane as f32);
            let r = b.snap_rect(Rect::from_min_max(
                pos2(rect.left(), y1 - geo.key_h),
                pos2(rect.right(), y1),
            ));
            if k.is_root(pitch) {
                b.rect(r, Fill::Solid(with_alpha(t.selection, 0.35)));
            } else if !k.contains(pitch) {
                b.rect(r, Fill::Solid(with_alpha(t.dark_shade, 0.25)));
            }
            b.hline(
                r.left(),
                r.right(),
                r.bottom() - 0.5 * b.px(),
                b.px(),
                t.grid,
            );
            if geo.key_h >= 9.0 {
                let name = Text::new(geo.axis.name_ref(lane as usize), Role::Label, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = text::paint(b, r.shrink2(egui::vec2(6.0 * s.k, 0.0)), &name, s.tk());
            }
        }
        return;
    }
    if !app.notefx.highlight {
        return;
    }
    for lane in lo..hi {
        let pitch = geo.axis.pitch(lane as usize);
        if !k.contains(pitch) {
            continue;
        }
        let y1 = geo.y(lane as f32);
        let w = if k.is_root(pitch) { 5.0 } else { 3.0 } * s.k;
        let r = Rect::from_min_max(
            pos2(rect.right() - w, y1 - geo.key_h + 1.0),
            pos2(rect.right(), y1 - 1.0),
        );
        let c = if k.is_root(pitch) {
            t.selection
        } else {
            with_alpha(t.selection, 0.55)
        };
        b.rect(b.snap_rect(r), Fill::Solid(c));
    }
}

/// A drawn note's pitch: kept in the key when *Snap to scale* is on (the nearest note of
/// it); otherwise as pointed at.
pub(super) fn snap(app: &CantusApp, axis: &Axis, pitch: Pitch) -> Pitch {
    match key(app).filter(|_| app.notefx.snap && !axis.pads()) {
        Some(k) => k.snap(pitch),
        None => pitch,
    }
}

/// A note moved `rows` rows: in rows of the axis, then — snapping on a keyboard — onto the
/// key's nearest note the way it moved (so a note dragged up never falls back down). A
/// folded axis's rows are the key's already.
pub fn moved(app: &CantusApp, axis: &Axis, pitch: Pitch, rows: i32) -> Pitch {
    let p = axis.moved(pitch, rows);
    match key(app).filter(|_| app.notefx.snap && axis.is_chromatic()) {
        Some(k) if rows != 0 => k.snap_toward(p, rows.signum()),
        _ => p,
    }
}

/// The toolbar's key: the song's key (a click opens its roots and scales), and the switches
/// *Scale* (light the key's rows), *Fold* (only its rows) and *Snap* (keep notes in it).
pub(super) fn toolbar(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    bar: &mut super::Toolbar,
    id: egui::Id,
) {
    use crate::i18n::t;
    bar.divider(ui);
    let caption = t("notefx.key");
    let name = crate::notefx_ui::key_name(app.project().key);
    let field = bar.field(ui, id.with("key"), Some(&caption), &name, 96.0, || {
        t("notefx.key.tip")
    });
    if let Some(r) = field.filter(|r| r.clicked()) {
        app.notefx.key_menu = match app.notefx.key_menu {
            Some(_) => None,
            None => Some(r.rect),
        };
    }
    let has_key = app.project().key.is_some();
    let switches: [(&str, &str, bool, f32); 3] = [
        (
            "notefx.highlight",
            "notefx.highlight.tip",
            app.notefx.highlight,
            66.0,
        ),
        ("notefx.fold", "notefx.fold.tip", app.notefx.fold, 58.0),
        ("notefx.snap", "notefx.snap.tip", app.notefx.snap, 70.0),
    ];
    for (i, (key, tip, on, w)) in switches.into_iter().enumerate() {
        let Some(r) = bar.led(ui, id.with(key), on && has_key, key, w, || t(tip)) else {
            continue;
        };
        if r.clicked() {
            if !has_key {
                // No key yet: the switch asks for one first.
                app.notefx.key_menu = Some(r.rect);
                continue;
            }
            match i {
                0 => app.notefx.highlight = !app.notefx.highlight,
                1 => {
                    app.notefx.fold = !app.notefx.fold;
                    refold(app);
                }
                _ => app.notefx.snap = !app.notefx.snap,
            }
        }
    }
}

/// The piano roll lays its rows out again (the key or the fold changed).
pub fn forget_axis(app: &mut CantusApp) {
    app.piano_roll.axis = None;
}

/// *Fold* switched: the rows are laid out again, and the view stays on the pitch at its
/// middle (a folded keyboard has fewer rows: the same lane number is another pitch).
fn refold(app: &mut CantusApp) {
    let v = &app.piano_roll;
    let rows = (v.last_grid.1 / v.key_height.max(1.0)).max(1.0);
    let middle = (v.top_lane - rows / 2.0).max(0.0) as usize;
    let Some((_, old)) = v.axis.as_ref() else {
        forget_axis(app);
        return;
    };
    if old.pads() {
        forget_axis(app);
        return;
    }
    let center = old.pitch(middle);
    let clip = app
        .selected_track
        .zip(app.selected_clip)
        .and_then(|(t, c)| app.project().track(t)?.clip(c));
    let kept = |p: u8| clip.is_some_and(|c| c.notes.iter().any(|n| n.pitch == p));
    let (lane, len) = match key(app).filter(|_| app.notefx.fold) {
        Some(k) => {
            let row = |p: &u8| k.contains(*p) || kept(*p);
            (
                (0..center).filter(row).count(),
                (0..=127u8).filter(row).count(),
            )
        }
        None => (center as usize, 128),
    };
    app.piano_roll.top_lane = (lane as f32 + rows / 2.0).clamp(1.0, len.max(1) as f32);
    forget_axis(app);
}

/// Switch *Fold* on or off, keeping the view where it was (the toolbar's switch).
pub fn set_fold(app: &mut CantusApp, on: bool) {
    if app.notefx.fold != on {
        app.notefx.fold = on;
        refold(app);
    }
}

/// [`moved`] for one of the notes `all` being dragged together: when the key would put two
/// different pitches of them on one, the drag keeps their pitches (it moves them in time
/// only) — a squash could not be undone by dragging back.
pub fn moved_with(
    app: &CantusApp,
    axis: &Axis,
    all: &[cantus_core::Note],
    pitch: Pitch,
    rows: i32,
) -> Pitch {
    let snapping = key(app).is_some() && app.notefx.snap && axis.is_chromatic();
    if snapping
        && crate::notefx_ui::injective_fails(all.iter().map(|n| n.pitch), |p| {
            moved(app, axis, p, rows)
        })
    {
        return pitch;
    }
    moved(app, axis, pitch, rows)
}
