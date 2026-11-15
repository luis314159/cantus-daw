//! The bands drawn and sensed (`super`'s model): their rows over the strips, the gaps
//! between top-level bands, a folded band's one strip, and the drag of a group band's right
//! edge that takes in or gives back the strip beside it.

use super::super::desk::{self, Place};
use super::super::look::{self, Pal, W};
use super::super::strip::Pass;
use super::super::{Geo, MixerView};
use super::{band_h, instrument_word, whole_families, Band, Kind, BAND, SUB};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{contrast, mix, with_alpha};
use crate::kit::{Brush, Fill, State};
use cantus_core::{Command, Dock, GroupId, Project, TrackId};
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

/// The band's fill: its colour over the desk's base, as much of it as keeps the band's
/// words (ink 1 and ink 2) at 4.5:1 (ADR 0111).
pub(super) fn band_fill(p: &Pal, hue: Color32, start: f32) -> Color32 {
    let mut k = start;
    loop {
        let fill = p.hue(hue, k);
        if k <= 0.04 || (contrast(p.ink1, fill) >= 4.5 && contrast(p.ink2, fill) >= 4.5) {
            return fill;
        }
        k -= 0.02;
    }
}

/// Where a desk position starts on screen: its dock's origin, the strips and gaps before it.
fn place_x(v: &MixerView, geo: &Geo, place: Place) -> f32 {
    let origin = match place.dock {
        Dock::Left => geo.left.left() + geo.master_w,
        Dock::Middle => geo.middle.left() - v.scroll_x,
        Dock::Right => geo.right.left(),
    };
    origin + f32::from(place.strips) * geo.pitch + f32::from(place.seps) * desk::SEPARATOR
}

fn dock_clip(geo: &Geo, dock: Dock) -> Rect {
    match dock {
        Dock::Middle => geo.middle,
        Dock::Left => geo.left,
        Dock::Right => geo.right,
    }
}

/// A band's colour and name.
fn look_of(p_: &Project, b: &Band) -> Option<(Color32, String)> {
    match b.kind {
        Kind::Family(t) => p_
            .track(t)
            .map(|tr| (crate::theme::track_color(tr.color), tr.name.clone())),
        Kind::Group { id, .. } => p_
            .strip_group(id)
            .map(|g| (crate::theme::track_color(g.color), g.name.clone())),
    }
}

/// A family's glyph: the instrument track's own icon, else one for its kind (a kit for the
/// drums, a keyboard for the rest).
fn family_glyph(app: &CantusApp, t: TrackId) -> Option<crate::views::track_glyphs::TrackGlyph> {
    use crate::views::track_glyphs as tg;
    tg::track_icon(app, t).or_else(|| {
        let drums = app
            .project()
            .track(t)
            .is_some_and(|x| matches!(x.icon(), cantus_core::TrackIcon::Drums));
        tg::by_id(if drums { "drumkit" } else { "midikeyboard" })
    })
}

/// The chevron: down while open, right while folded.
fn chevron(b: &mut Brush, c: egui::Pos2, folded: bool, ink: Color32) {
    if folded {
        b.polyline(
            &[
                pos2(c.x - 1.5, c.y - 3.5),
                pos2(c.x + 2.0, c.y),
                pos2(c.x - 1.5, c.y + 3.5),
            ],
            1.3,
            ink,
        );
    } else {
        crate::kit::well::chevron(b, c, 7.0, 4.0, false, 1.3, ink);
    }
}

/// What a band's click asked for.
enum Act {
    FoldFamily(TrackId),
    Collapse(GroupId, bool),
    Rename(GroupId),
    Menu(GroupId, egui::Pos2),
}

/// The bands, row by row over the strips' heads. A family's: the track's colour as a solid
/// bar, its name and *Tympanum · 3 mics*, the strips added to it marked. A group's: a thin
/// line in its colour along the top, its name, its count at the end; its right edge dragged
/// takes in the next strip or gives back its last. A sub-group's: a head under its family's
/// band. A click folds or unfolds; a group's right-click opens its menu, a double-click
/// renames it, a drag of its body moves it.
pub(in crate::views::mixer) fn bands(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    geo: &Geo,
) {
    if v.desk.fam.list.is_empty() {
        return;
    }
    // Strips that left a family's band since the desk was last worked out: said once.
    left_notice(v, app, ui);
    let total = band_h(&v.desk);
    let top = geo.y0 - total - 1.0;
    let list = v.desk.fam.list.clone();
    let mut acts = Vec::new();
    for band in &list {
        let (Some(&first), Some(&last)) = (band.members.first(), band.members.last()) else {
            continue;
        };
        let (Some(a), Some(z)) = (
            v.desk.place.get(first).copied(),
            v.desk.place.get(last).copied(),
        ) else {
            continue;
        };
        let x0 = place_x(v, geo, a);
        let x1 = place_x(v, geo, z) + geo.pitch - 2.0;
        let sub = matches!(band.kind, Kind::Group { sub: true, .. });
        let y0 = top + f32::from(band.row) * BAND;
        let h = if sub { SUB } else { BAND } - 2.0;
        let r = Rect::from_min_max(pos2(x0, y0), pos2(x1, y0 + h));
        let shown = r.intersect(dock_clip(geo, a.dock));
        if shown.width() < 4.0 {
            continue;
        }
        if let Some(act) = band_one(v, app, ui, b, pass, geo, band, r, shown) {
            acts.push(act);
        }
    }
    for act in acts {
        match act {
            Act::FoldFamily(t) => super::toggle(v, t),
            Act::Collapse(id, collapsed) => {
                app.exec(Command::SetStripGroupCollapsed { id, collapsed });
            }
            Act::Rename(g) => v.begin_group_rename(app, g),
            Act::Menu(g, at) => v.popups.open(super::super::menu::Menu::Group(g), at),
        }
    }
}

/// Say once that a strip added to a family left its band (its input stopped coming only from
/// the family).
fn left_notice(v: &mut MixerView, app: &mut CantusApp, ui: &Ui) {
    let key = egui::Id::new("mixer-band-left");
    let seen: Vec<(u64, u64)> = ui.ctx().data(|d| d.get_temp(key)).unwrap_or_default();
    let now: Vec<(u64, u64)> = v.desk.fam.left.iter().map(|(s, t)| (s.0, t.0)).collect();
    if now == seen {
        return;
    }
    let p = app.project();
    if let Some((s, t)) = v
        .desk
        .fam
        .left
        .iter()
        .find(|(s, t)| !seen.contains(&(s.0, t.0)))
    {
        let strip = p.strip(*s).map(|x| x.name.clone()).unwrap_or_default();
        let name = p.track(*t).map(|x| x.name.clone()).unwrap_or_default();
        app.set_error(tf(
            "inst_mixer.left_band",
            &[("strip", &strip), ("name", &name)],
        ));
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, now));
}

/// One band: painted in `shown` (the part of `r` its dock shows) and sensed.
#[allow(clippy::too_many_arguments)]
fn band_one(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    geo: &Geo,
    band: &Band,
    r: Rect,
    shown: Rect,
) -> Option<Act> {
    let p = pass.pal;
    let s = pass.s;
    let (hue, name, inst) = {
        let p_ = app.project();
        let (hue, name) = look_of(p_, band)?;
        let inst = match band.kind {
            Kind::Family(t) => instrument_word(p_, t),
            Kind::Group { .. } => String::new(),
        };
        (hue, name, inst)
    };
    // The ids the desk's separator and family band had: a group's band is its handle now.
    let id = match band.kind {
        Kind::Group { id, .. } => egui::Id::new(("mixer-group", id.0)),
        Kind::Family(t) => egui::Id::new(("mixer-family", t.0)),
    };
    let resp = ui.interact(shown, id, Sense::click_and_drag());
    let hot = resp.hovered();
    let base = r.center().y + 4.0;
    match band.kind {
        Kind::Family(tr) => {
            let n = band.folds.len() - band.added.len();
            let about = if n == 1 {
                tf("inst_mixer.band_about_one", &[("inst", &inst)])
            } else {
                tf("inst_mixer.band_about", &[("inst", &inst), ("n", &n)])
            };
            let resp = crate::views::hint::said_with(resp, || {
                tf("inst_mixer.band_tip", &[("name", &name), ("inst", &inst)])
            });
            let rr = b.snap_rect(shown);
            let fill = band_fill(&p, hue, if p.dark { 0.38 } else { 0.20 });
            let fill = if hot { mix(fill, p.ink1, 0.06) } else { fill };
            b.rrect(rr, 3.0, Fill::Solid(fill));
            // The family's mark: a solid bar of its colour at its start.
            if shown.left() <= r.left() + 0.5 {
                b.rect(
                    b.snap_rect(Rect::from_min_size(rr.min, vec2(3.0, rr.height()))),
                    Fill::Solid(p.stripe(hue, 1.0)),
                );
            }
            // Sticky: the head stands at the visible part's left, all of it or none of it
            // (never a bare chevron); the tooltip and the strips name the family meanwhile.
            let glyph = family_glyph(app, tr);
            let gw = if glyph.is_some() { 16.0 } else { 0.0 };
            let tx = shown.left() + 20.0 + gw;
            let nw = look::width_of(ui.ctx(), s, &name, 11.0, W::SemiBold);
            let room = (shown.right() - 6.0 - tx).max(0.0);
            if nw <= room {
                chevron(
                    b,
                    pos2(shown.left() + 11.0, r.center().y - 0.5),
                    band.folded,
                    p.ink1,
                );
                if let Some(g) = glyph {
                    let gr = Rect::from_center_size(
                        pos2(shown.left() + 20.0 + 6.0, r.center().y),
                        vec2(12.0, 12.0),
                    );
                    let inks = crate::views::track_glyphs::inks(s, p.ink1, band_fill(&p, hue, 0.3));
                    crate::views::track_glyphs::paint_with(b, gr, g, &inks);
                }
                look::say(
                    b,
                    s,
                    &name,
                    tx,
                    base,
                    Align2::LEFT_BOTTOM,
                    nw + 1.0,
                    11.0,
                    W::SemiBold,
                    p.ink1,
                );
            }
            let ax = tx + nw + 8.0;
            let aw = look::width_of(ui.ctx(), s, &about, 10.0, W::Regular);
            if shown.right() - 6.0 - ax >= aw {
                look::say(
                    b,
                    s,
                    &about,
                    ax,
                    base,
                    Align2::LEFT_BOTTOM,
                    aw + 1.0,
                    10.0,
                    W::Regular,
                    p.ink2,
                );
            }
            // The strips the user added: marked at the band's end.
            if let Some(first) = band
                .added
                .first()
                .and_then(|i| v.desk.place.get(*i).copied())
            {
                let ax = place_x(v, geo, first);
                if ax > shown.left() + 4.0 && ax < shown.right() {
                    b.vline(
                        b.snap(ax - 1.0),
                        rr.top() + 3.0,
                        rr.bottom() - 3.0,
                        b.px(),
                        with_alpha(p.ink2, 0.6),
                    );
                    let word = t("inst_mixer.added");
                    let ww = look::width_of(ui.ctx(), s, &word, 10.0, W::Regular);
                    if shown.right() - ax - 6.0 >= ww {
                        look::say(
                            b,
                            s,
                            &word,
                            ax + 4.0,
                            base,
                            Align2::LEFT_BOTTOM,
                            ww + 1.0,
                            10.0,
                            W::Regular,
                            p.ink2,
                        );
                    }
                }
            }
            resp.clicked().then_some(Act::FoldFamily(tr))
        }
        Kind::Group { id: gid, sub } => {
            let n = band.members.len();
            let rr = b.snap_rect(shown);
            if sub {
                // A sub-group's head, as Tympanum's own mixer heads its families: the
                // chevron, the name, a hairline in the group's colour to its end.
                if hot {
                    b.rrect(rr, 3.0, Fill::Solid(mix(p.base, p.ink1, 0.05)));
                }
                let tx = shown.left() + 18.0;
                let nw = look::width_of(ui.ctx(), s, &name, 11.0, W::Medium);
                let room = (shown.right() - 6.0 - tx).max(0.0);
                if nw <= room {
                    chevron(
                        b,
                        pos2(shown.left() + 9.0, r.center().y - 0.5),
                        band.folded,
                        p.ink2,
                    );
                    look::say(
                        b,
                        s,
                        &name,
                        tx,
                        base,
                        Align2::LEFT_BOTTOM,
                        nw + 1.0,
                        11.0,
                        W::Medium,
                        p.ink1,
                    );
                }
                let lx = tx + nw + 6.0;
                if lx < shown.right() - 8.0 {
                    b.hline(
                        lx,
                        shown.right() - 4.0,
                        b.snap(r.center().y) + b.px() * 0.5,
                        b.px(),
                        p.stripe(hue, 0.9),
                    );
                }
            } else {
                // A group's band: its colour faint, a thin line of it along the top.
                let fill = band_fill(&p, hue, if p.dark { 0.16 } else { 0.10 });
                b.rrect(
                    rr,
                    3.0,
                    Fill::Solid(if hot { mix(fill, p.ink1, 0.05) } else { fill }),
                );
                b.rect(
                    b.snap_rect(Rect::from_min_size(rr.min, vec2(rr.width(), 2.0))),
                    Fill::Solid(p.stripe(hue, 1.0)),
                );
                let count = n.to_string();
                let cw = look::width_of(ui.ctx(), s, &count, 10.0, W::Medium);
                let tx = shown.left() + 20.0;
                // Sticky: the chevron and the name at the visible part's left, the count at
                // its right. The name with its count when both fit, alone when only it
                // does; neither (and no chevron) when even the name has no room.
                let nw = look::width_of(ui.ctx(), s, &name, 11.0, W::SemiBold);
                let with_count = nw <= (shown.right() - 10.0 - cw - tx).max(0.0);
                let named = with_count || nw <= (shown.right() - 6.0 - tx).max(0.0);
                if named {
                    chevron(
                        b,
                        pos2(shown.left() + 11.0, r.center().y + 0.5),
                        band.folded,
                        p.ink1,
                    );
                    look::say(
                        b,
                        s,
                        &name,
                        tx,
                        base + 1.0,
                        Align2::LEFT_BOTTOM,
                        nw + 1.0,
                        11.0,
                        W::SemiBold,
                        p.ink1,
                    );
                }
                if with_count {
                    look::say(
                        b,
                        s,
                        &count,
                        shown.right() - 6.0,
                        base + 1.0,
                        Align2::RIGHT_BOTTOM,
                        cw + 1.0,
                        10.0,
                        W::Medium,
                        p.ink2,
                    );
                }
            }
            // The band carries the group: its menu, its name, moving it (and a drop on it),
            // folding it — what its separator did before.
            super::super::drag::source(v, app, ui, &resp, super::super::drag::Source::Group(gid));
            let target = super::super::drag::target_group(v, app, ui, gid, shown);
            super::super::cables::column_hatch(b, s, shown, target);
            if resp.hovered() {
                let tip = tf("inst_mixer.group_tip", &[("name", &name), ("n", &n)]);
                let caps = crate::kit::hint::Caps {
                    menu: true,
                    ..crate::kit::hint::Caps::NONE
                };
                crate::kit::hint::set_text(ui.ctx(), &tip, &v.words.group, caps);
            }
            if matches!(v.rename, Some((super::super::Renaming::Group(g), _)) if g == gid) {
                v.rename_at = Some(shown);
            }
            if !sub && shown.right() >= r.right() - 0.5 {
                edge(v, app, ui, b, pass, geo, band, gid, r);
            }
            if resp.secondary_clicked() {
                let at = resp.interact_pointer_pos().unwrap_or(shown.center());
                Some(Act::Menu(gid, at))
            } else if resp.double_clicked() {
                Some(Act::Rename(gid))
            } else if resp.clicked() {
                Some(Act::Collapse(gid, !band.folded))
            } else {
                None
            }
        }
    }
}

/// The drag of a group band's right edge: past the next strip's middle it takes that strip
/// in (a strip of a family brings its whole family); back past its last member's middle it
/// gives that one back. A dashed ghost and a label say which while it is held; the release
/// is one undo step; Esc leaves the group as it was.
#[allow(clippy::too_many_arguments)]
fn edge(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    geo: &Geo,
    band: &Band,
    gid: GroupId,
    r: Rect,
) {
    let p = pass.pal;
    let s = pass.s;
    let handle = Rect::from_min_max(
        pos2(r.right() - 5.0, r.top()),
        pos2(r.right() + 5.0, r.bottom()),
    );
    let id = egui::Id::new(("mixer-band-edge", gid.0));
    let resp = ui.interact(handle, id, Sense::drag());
    let resp = crate::views::hint::said_with(resp, || t("inst_mixer.edge_tip"));
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        // The grip: three dots on the edge.
        for k in -1..=1 {
            b.circle(
                pos2(r.right() - 1.0, r.center().y + k as f32 * 3.5),
                1.1,
                Fill::Solid(p.ink2),
            );
        }
    }
    let cancelled = egui::Id::new(("mixer-band-edge-esc", gid.0));
    if resp.drag_started() {
        ui.ctx().data_mut(|d| d.insert_temp(cancelled, false));
    }
    // Esc while held (egui may end the drag on the same key: the frame it stops counts too).
    if (resp.dragged() || resp.drag_stopped())
        && ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_down(egui::Key::Escape))
    {
        ui.ctx().data_mut(|d| d.insert_temp(cancelled, true));
    }
    if !(resp.dragged() || resp.drag_stopped()) {
        return;
    }
    if ui
        .ctx()
        .data(|d| d.get_temp::<bool>(cancelled))
        .unwrap_or(false)
    {
        return;
    }
    let Some(q) = resp.interact_pointer_pos() else {
        return;
    };
    let Some(&last) = band.members.last() else {
        return;
    };
    let p_ = app.project().clone();
    let p_ = &p_;
    let lp = v.desk.place[last];
    // The strip just after the band (past its gap), and the band's last member.
    let next = p_.strips.iter().enumerate().find(|(i, x)| {
        let pl = v.desk.place[*i];
        x.on_desk() && pl.dock == lp.dock && pl.strips == lp.strips + 1 && !pl.folded
    });
    let last_x = place_x(v, geo, lp);
    let mut want: Option<(bool, cantus_core::StripId, Rect)> = None;
    if let Some((ni, nx)) = next {
        let nxp = place_x(v, geo, v.desk.place[ni]);
        if q.x > nxp + geo.pitch * 0.5 && nx.group != Some(gid) {
            want = Some((true, nx.id, pass.body(nxp, geo.pitch - 2.0)));
        }
    }
    if want.is_none() && band.members.len() > 1 && q.x < last_x + geo.pitch * 0.5 {
        if let Some(st) = p_.strips.get(last) {
            want = Some((false, st.id, pass.body(last_x, geo.pitch - 2.0)));
        }
    }
    let gname = p_
        .strip_group(gid)
        .map(|g| g.name.clone())
        .unwrap_or_default();
    if let Some((add, sid, body)) = want {
        let sname = p_
            .strip(sid)
            .map(|x| x.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                p_.tracks_on_strip(sid)
                    .next()
                    .map(|t| t.name.clone())
                    .unwrap_or_default()
            });
        if resp.dragged() {
            let ghost = Rect::from_min_max(
                r.min,
                pos2(
                    if add { body.right() } else { body.left() - 2.0 },
                    r.bottom(),
                ),
            );
            let pts = [
                ghost.left_top(),
                ghost.right_top(),
                ghost.right_bottom(),
                ghost.left_bottom(),
                ghost.left_top(),
            ];
            b.dashed(&pts, b.px(), p.ink2, 3.0, 2.0);
            b.rrect_stroke(b.snap_rect(body), 3.0, 1.5, p.accent);
            b.rrect(
                b.snap_rect(body),
                3.0,
                Fill::Solid(with_alpha(p.accent, 0.08)),
            );
            let words = if add {
                tf(
                    "inst_mixer.add_to_group",
                    &[("strip", &sname), ("group", &gname)],
                )
            } else {
                tf(
                    "inst_mixer.take_from_group",
                    &[("strip", &sname), ("group", &gname)],
                )
            };
            let tw = look::width_of(ui.ctx(), s, &words, 11.0, W::SemiBold) + 16.0;
            let at = Rect::from_min_size(
                pos2(body.center().x - tw * 0.5, r.bottom() + 120.0),
                vec2(tw, 22.0),
            );
            crate::views::inst::inst_mixer::ui::paint_button(
                b,
                s,
                at,
                State::LIT,
                &words,
                crate::views::inst::inst_mixer::ui::Lead::None,
                true,
            );
        } else if resp.drag_stopped() {
            if add {
                let take = whole_families(p_, &[sid]);
                // A sub-group's band takes no strip from outside its family.
                let ok = !matches!(band.kind, Kind::Group { sub: true, .. });
                if ok {
                    app.add_strips_to_group(&take, gid);
                }
            } else {
                app.remove_strips_from_group(&[sid]);
            }
        }
    }
}

/// A gap between a band and its neighbour: the desk's base, a 1-px seam down its middle
/// (the right dock's own device: value, not a border).
pub(in crate::views::mixer) fn gap(b: &mut Brush, pass: &Pass, x: f32, w: f32) {
    let p = pass.pal;
    let body = pass.body(x, w);
    let sx = b.snap(x + w * 0.5 - 1.0) + b.px() * 0.5;
    b.vline(
        sx,
        body.top() + 4.0,
        body.bottom() - 4.0,
        b.px(),
        mix(p.base, p.line, 0.7),
    );
}

/// A folded band's one strip in place of what it folds: their numbers and its name in the
/// head, the members named, their meters side by side, *Unfold*. Returns whether strip
/// `index` is one (else the desk draws it as itself).
#[allow(clippy::too_many_arguments)]
pub(in crate::views::mixer) fn folded(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    index: usize,
    x: f32,
    w: f32,
) -> bool {
    use super::super::layout::Row;
    let Some(band) = v.desk.fam.folded_at(index).cloned() else {
        return false;
    };
    let p = pass.pal;
    let s = pass.s;
    let (hue, title, members, nums, unfold_tip) = {
        let proj = app.project();
        let Some((hue, name)) = look_of(proj, &band) else {
            return false;
        };
        let title = match band.kind {
            Kind::Family(_) => tf("inst_mixer.mics_of", &[("name", &name)]),
            Kind::Group { .. } => name.clone(),
        };
        let num = |i: usize| {
            proj.strips
                .get(i)
                .and_then(|st| proj.strip_number(st.id))
                .unwrap_or(0)
        };
        let (a, z) = (
            num(*band.folds.first().unwrap_or(&index)),
            num(*band.folds.last().unwrap_or(&index)),
        );
        let nums = if a == z {
            a.to_string()
        } else {
            format!("{a}\u{2013}{z}")
        };
        let members: Vec<(usize, String)> = band
            .folds
            .iter()
            .map(|i| {
                let st = proj.strips.get(*i);
                let n = st
                    .map(|x| x.name.clone())
                    .filter(|n| !n.is_empty())
                    .or_else(|| {
                        st.and_then(|x| proj.tracks_on_strip(x.id).next())
                            .map(|t| t.name.clone())
                    })
                    .unwrap_or_default();
                (*i, n)
            })
            .collect();
        (
            hue,
            title,
            members,
            nums,
            tf("inst_mixer.unfold_tip", &[("name", &name)]),
        )
    };
    let bw = w - 2.0;
    let body = pass.body(x, bw);
    look::body(b, body, &p, None);
    look::stripe(b, body, p.stripe(hue, 0.72));
    if let (Some((na, nz)), Some(st)) = (
        pass.row(Row::Number),
        app.project().strips.get(index).map(|s| s.id),
    ) {
        let zone = Rect::from_min_max(pos2(x, na), pos2(x + bw, nz)).intersect(pass.clip);
        let id = super::super::strip::pid(Some(st), super::super::strip::Part::Head);
        if ui.interact(zone, id, Sense::click()).clicked() {
            app.select_strip(st);
        }
    }
    let cx = x + bw * 0.5;
    let roomy = bw >= 44.0;
    let say = |b: &mut Brush, txt: &str, y: f32, size: f32, wt: W, ink: Color32| {
        look::say(
            b,
            s,
            txt,
            cx,
            y,
            Align2::CENTER_BOTTOM,
            bw - 4.0,
            size,
            wt,
            ink,
        );
    };
    // Wide and Large set the name under the numbers in the head; Compact the numbers alone.
    match (pass.row(Row::Number), pass.row(Row::Name)) {
        (Some((na, nz)), Some((ma, mz))) => {
            say(b, &nums, (na + nz) * 0.5 + 4.0, 10.0, W::Regular, p.ink3);
            if roomy {
                say(b, &title, (ma + mz) * 0.5 + 4.0, 12.0, W::SemiBold, p.ink1);
            }
        }
        (Some((na, nz)), None) if roomy => {
            say(b, &nums, na + 13.0, 10.0, W::Regular, p.ink3);
            say(b, &title, nz - 6.0, 12.0, W::SemiBold, p.ink1);
        }
        (Some((na, nz)), None) => say(b, &nums, (na + nz) * 0.5 + 4.0, 10.0, W::Regular, p.ink3),
        _ => {}
    }
    // The members, named, where the inserts are; how many folded under them.
    let mut y = pass
        .row(Row::Chain)
        .map_or(body.top() + 60.0, |r| r.0 + 4.0);
    let listed = if roomy { members.len().min(4) } else { 0 };
    let list = Rect::from_min_size(
        pos2(x + 3.0, y - 2.0),
        vec2(bw - 6.0, 15.0 * listed as f32 + 4.0),
    );
    if roomy {
        look::panel(b, list, &p);
    }
    for (_, nm) in members.iter().take(listed) {
        b.circle(pos2(x + 9.0, y + 6.0), 1.5, Fill::Solid(p.ink3));
        look::say(
            b,
            s,
            nm,
            x + 14.0,
            y + 10.0,
            Align2::LEFT_BOTTOM,
            bw - 18.0,
            10.0,
            W::Regular,
            p.ink2,
        );
        y += 15.0;
    }
    if roomy {
        let words = tf("inst_mixer.folded", &[("n", &members.len())]);
        say(b, &words, list.bottom() + 16.0, 10.0, W::Regular, p.ink3);
    } else if let (Some((_, nz)), Some((fa, _))) = (pass.row(Row::Number), pass.row(Row::Fader)) {
        // Compact: the name runs up the column, as a Compact strip's name does.
        use crate::kit::text::{self, Elide, Policy, Role, Text};
        let nr = Rect::from_min_max(pos2(x + 3.0, nz + 2.0), pos2(x + bw - 2.0, fa - 4.0));
        if nr.height() > 12.0 {
            let tt = Text::new(&title, Role::Label, p.ink1)
                .align(Align2::CENTER_TOP)
                .policy(Policy::Rotate90(Elide::Middle));
            let _ = text::paint(b, nr, &tt, s.tk());
        }
    }
    // Their meters, side by side, on the fader's travel.
    if let Some((fa, fz)) = pass.row(Row::Fader) {
        let n = members.len().max(1) as f32;
        let bwid = ((bw - 12.0) / (n * 2.0 + (n - 1.0) * 0.6)).clamp(1.5, 4.0);
        let span = n * (2.0 * bwid + 1.0) + (n - 1.0) * 3.0;
        let mut mx = cx - span * 0.5;
        let top = fa.max(list.bottom() + 24.0);
        for (i, _) in &members {
            let peak = app.engine.meters().strips.get(*i).copied().unwrap_or(0.0);
            let lv = v.meters.strip(*i, peak, pass.now, false);
            let bars = (lv.level[0] > crate::kit::meter::MIN_DB).then_some(look::Bars {
                level: lv.level,
                hold: lv.hold,
            });
            look::meter(b, mx, top + 8.0, fz - 4.0, bwid, bars, false, false, &p);
            mx += 2.0 * bwid + 1.0 + 3.0;
        }
    }
    // Unfold, where the others' values are (the row every strip has the room of).
    let (fa, fz) = pass
        .row(Row::Readout)
        .or(pass.row(Row::Output))
        .unwrap_or((body.bottom() - 26.0, body.bottom() - 4.0));
    let h = (fz - fa).clamp(14.0, 20.0);
    let br = Rect::from_min_size(pos2(x + 5.0, (fa + fz) * 0.5 - h * 0.5), vec2(bw - 10.0, h));
    let id = egui::Id::new(("mixer-unfold", format!("{:?}", band.kind)));
    let resp = ui.interact(br.intersect(pass.clip), id, Sense::click());
    let unfold = t("inst_mixer.unfold");
    let fits = look::width_of(ui.ctx(), s, &unfold, 12.0, W::Medium) + 14.0 <= br.width();
    let word = if fits {
        unfold
    } else {
        "\u{2039}\u{203a}".to_string()
    };
    let resp = crate::views::hint::said_with(resp, || unfold_tip);
    let st = State::of(&resp, false);
    crate::views::inst::inst_mixer::ui::paint_button(
        b,
        s,
        br,
        st,
        &word,
        crate::views::inst::inst_mixer::ui::Lead::None,
        true,
    );
    if resp.clicked() {
        match band.kind {
            Kind::Family(t) => super::toggle(v, t),
            Kind::Group { id, .. } => {
                app.exec(Command::SetStripGroupCollapsed {
                    id,
                    collapsed: false,
                });
            }
        }
    }
    true
}
