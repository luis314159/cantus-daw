//! The track heads over the columns (name, colour, arm, mute, solo and *back to
//! arrangement*), the master column at the right (the scene launchers with their names and
//! tempo/metre tags, *stop all*), and the stop row under the grid (ADR 0077).

use super::super::layout::{Geometry, Hit};
use super::super::paint::{self, HeadLook, SceneLook};
use super::super::Pressed;
use super::grid::blink_on;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Policy, Role, Text};
use crate::kit::{with_brush, Style};
use cantus_core::controller::MappingTarget;
use cantus_core::Command;
use cantus_engine::messages::{ScenePhase, SlotPhase};
use egui::{pos2, Id, Rect, Sense, Ui};

pub(crate) fn heads(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry, s: &Style) {
    let k = s.k;
    let (cols, _) = geo.visible();
    let clip = geo.heads.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut tints = std::mem::take(&mut app.session_view.tints);
    let mut clicks: Vec<(usize, usize)> = Vec::new();
    #[allow(clippy::type_complexity)]
    let mut heads: Vec<(
        Rect,
        [Rect; 4],
        crate::views::clips_chrome::Tint,
        bool,
        bool,
        bool,
        bool,
        bool,
    )> = Vec::new();
    for i in cols.clone() {
        let r = geo.head(i);
        let keys = paint::head_keys(r, k);
        let id = Id::new(("cantus-session-head", geo.tracks[i].0));
        let resp = ui.interact(r.intersect(clip), id, Sense::click());
        for (n, kr) in keys.iter().enumerate() {
            let kresp = ui.interact(kr.intersect(clip), id.with(n), Sense::click());
            if kresp.clicked() && !app.controllers.learn_mode {
                clicks.push((i, n));
            }
            if kresp.hovered() {
                let tip = [
                    "session.head.arm_tip",
                    "session.head.mute_tip",
                    "session.head.solo_tip",
                    "session.head.back_tip",
                ][n];
                hint::set_text(ui.ctx(), &t(tip), "", Caps::NONE);
            }
        }
        if resp.clicked() && !app.controllers.learn_mode {
            clicks.push((i, 9));
        }
        let track = &app.project().tracks[i];
        let colour = crate::theme::track_color(track.color);
        let look = HeadLook {
            name: &track.name,
            tint: tints.of(colour, s),
            held: app.session_view.live.holds(track.id),
            selected: app.selected_track == Some(track.id),
        };
        heads.push((
            r,
            keys,
            look.tint,
            look.held,
            look.selected,
            track.armed,
            track.muted,
            track.soloed,
        ));
        let t_id = geo.tracks[i];
        crate::controllers::learn_spot_in(app, ui, keys[0], clip, MappingTarget::TrackArm(t_id));
        crate::controllers::learn_spot_in(app, ui, keys[1], clip, MappingTarget::TrackMute(t_id));
        crate::controllers::learn_spot_in(app, ui, keys[2], clip, MappingTarget::TrackSolo(t_id));
    }
    app.session_view.tints = tints;
    // Every head in one brush: one mesh for the band, whatever the columns.
    let (mute, solo) = (t("session.head.mute"), t("session.head.solo"));
    with_brush(&painter, |b| {
        let hov = |q: Rect| pointer.is_some_and(|p| q.contains(p));
        for (n, (r, keys, tint, held, selected, armed, muted, soloed)) in heads.iter().enumerate() {
            let i = cols.start + n;
            let look = HeadLook {
                name: &app.project().tracks[i].name,
                tint: *tint,
                held: *held,
                selected: *selected,
            };
            paint::head(b, *r, s, &look);
            paint::glyph_key(
                b,
                keys[0],
                s,
                crate::kit::glyph::Glyph::Record,
                armed.then_some(s.t.danger),
                hov(keys[0]),
            );
            paint::key(
                b,
                keys[1],
                s,
                &mute,
                muted.then_some(s.t.highlight),
                hov(keys[1]),
            );
            paint::key(
                b,
                keys[2],
                s,
                &solo,
                soloed.then_some(s.t.selection),
                hov(keys[2]),
            );
            paint::glyph_key(
                b,
                keys[3],
                s,
                crate::kit::glyph::Glyph::Arrangement,
                held.then_some(s.t.highlight),
                hov(keys[3]),
            );
        }
    });
    for (i, n) in clicks {
        let id = geo.tracks[i];
        let Some(track) = app.project().track(id) else {
            continue;
        };
        let (armed, muted, soloed) = (track.armed, track.muted, track.soloed);
        match n {
            0 => {
                app.exec(Command::SetTrackArmed { id, armed: !armed });
            }
            1 => {
                app.exec(Command::SetTrackMute { id, muted: !muted });
            }
            2 => {
                app.exec(Command::SetTrackSolo {
                    id,
                    soloed: !soloed,
                });
            }
            3 => super::super::back_to_arrangement(app, Some(id)),
            _ => app.select_track(id),
        }
    }
}

/// The scene's tempo and metre, as its launcher's tag says them ("120 · 3/4").
pub(crate) fn scene_tag(sc: &cantus_core::session::Scene) -> Option<String> {
    let tempo = sc.tempo.map(|b| {
        if (b - b.round()).abs() < 0.005 {
            format!("{b:.0}")
        } else {
            format!("{b:.2}")
        }
    });
    let metre = sc
        .metre
        .map(|m| format!("{}/{}", m.numerator, m.denominator));
    match (tempo, metre) {
        (Some(a), Some(b)) => Some(format!("{a} \u{b7} {b}")),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(crate) fn master(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry, s: &Style) {
    let k = s.k;
    let view = geo.scene_view();
    let clip = view.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    // The master head: what the column is.
    let mh = geo.master_head();
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(mh);
        b.rrect(r, s.r(2.5), crate::kit::Fill::Solid(s.t.header_cell));
        let tx = Text {
            caps: true,
            ..Text::key("session.scenes", Role::Section, s.t.label)
                .align(egui::Align2::CENTER_CENTER)
                .policy(Policy::Elide(text::Elide::End))
        };
        let _ = text::paint(b, r, &tx, s.tk());
    });
    let resp = ui.interact(
        view,
        Id::new("cantus-session-scenes"),
        Sense::click_and_drag(),
    );
    // The screen reader hears the launchers as the cursor's scene.
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, spoken_scene(app))
    });
    let pointer = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| view.contains(*p));
    let hover = pointer.and_then(|p| geo.hit(p));
    let blink = blink_on(ui);
    let (_, rows) = geo.visible();
    let selected = app.session_view.cursor.map(|c| c.scene);
    let renaming = match &app.session_view.rename {
        Some(super::super::Rename::Scene(s, _)) => Some(*s),
        _ => None,
    };
    with_brush(&painter, |b| {
        for j in rows.clone() {
            let Some(sc) = app.project().session.scenes.get(j) else {
                continue;
            };
            let tag = scene_tag(sc);
            let phase = app.session_view.live.scene_phase(sc.id);
            let launcher = match phase {
                ScenePhase::Idle => paint::Launcher::Idle,
                ScenePhase::Queued { .. } => paint::Launcher::Queued { on: blink },
                ScenePhase::Playing { .. } => paint::Launcher::Playing,
            };
            let look = SceneLook {
                number: j + 1,
                name: &sc.name,
                tag: tag.as_deref(),
                colour: sc.color.map(crate::theme::track_color),
                launcher,
                selected: selected == Some(sc.id),
                hovered: hover == Some(Hit::Scene(sc.id)),
                renaming: renaming == Some(sc.id),
            };
            paint::scene(b, geo.scene(j), s, &look);
        }
        // The row under the last scene: a double-click there adds one.
        let n = geo.scenes.len();
        if rows.contains(&n) {
            let r = b.snap_rect(geo.scene(n));
            let faint = crate::kit::color::with_alpha(
                s.t.text_dim,
                if hover == Some(Hit::NewScene) {
                    0.7
                } else {
                    0.3
                },
            );
            b.rrect_stroke(r.shrink(b.px()), s.r(2.5), b.px(), faint);
            let g = Rect::from_center_size(r.center(), egui::vec2(10.0 * k, 10.0 * k));
            crate::kit::glyph::paint(b, g, crate::kit::glyph::Glyph::Plus, faint);
        }
    });
    let (pressed, released) =
        ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released()));
    if app.controllers.learn_mode {
        for j in rows.clone() {
            if let Some(sc) = geo.scenes.get(j) {
                crate::controllers::learn_spot_in(
                    app,
                    ui,
                    geo.scene(j),
                    clip,
                    MappingTarget::SessionScene(*sc),
                );
            }
        }
    } else {
        for j in rows.clone() {
            if let Some(sc) = geo.scenes.get(j) {
                let target = MappingTarget::SessionScene(*sc);
                if app.is_mapped(&target) {
                    crate::controllers::learn_spot_in(app, ui, geo.scene(j), clip, target);
                }
            }
        }
        if let (true, Some(Hit::Scene(sc)), Some(p)) =
            (pressed && resp.contains_pointer(), hover, pointer)
        {
            let j = geo.scenes.iter().position(|x| *x == sc).expect("hit");
            let r = geo.scene(j);
            if p.x < r.left() + 20.0 * k {
                super::super::launch_scene(app, sc, 1.0);
                app.session_view.pressed = Some(Pressed::Scene(sc));
            }
            choose_scene(app, sc);
        }
        if released {
            if let Some(Pressed::Scene(sc)) = app.session_view.pressed {
                app.session_view.pressed = None;
                super::super::release_scene(app, sc);
            }
        }
        if resp.double_clicked() {
            match hover {
                Some(Hit::Scene(sc)) => {
                    let name = app
                        .project()
                        .session
                        .scene(sc)
                        .map(|x| x.name.clone())
                        .unwrap_or_default();
                    app.session_view.rename = Some(super::super::Rename::Scene(sc, name));
                }
                Some(Hit::NewScene) => super::super::keys::insert_scene(app),
                _ => {}
            }
        }
        if resp.secondary_clicked() {
            if let Some(Hit::Scene(sc)) = hover {
                choose_scene(app, sc);
            }
            app.session_view.menu_on = hover;
        }
        let subject = app.session_view.menu_on;
        egui::Popup::context_menu(&resp)
            .frame(crate::views::clips_chrome::menu_frame())
            .show(|ui| {
                if let Some(h) = subject {
                    if super::super::menu::grid_menu(app, ui, h) {
                        ui.close();
                    }
                }
            });
    }
    if let Some(Hit::Scene(_)) = hover {
        hint::set_text(
            ui.ctx(),
            &t("session.scene.tip"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    // Stop all, under the scenes.
    let sa = geo.stop_all();
    let sresp = ui.interact(sa, Id::new("cantus-session-stop-all"), Sense::click());
    sresp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("session.stop_all_tip"))
    });
    let any = app.session_view.live.busy();
    with_brush(ui.painter(), |b| {
        paint::stop_button(b, sa, s, any, None, sresp.hovered());
        let tx = Text::key("session.stop_all", Role::Label, s.t.text_dim)
            .align(egui::Align2::LEFT_CENTER)
            .policy(Policy::Elide(text::Elide::End));
        let _ = text::paint(
            b,
            Rect::from_min_max(pos2(sa.center().x + 8.0 * k, sa.top()), sa.max),
            &tx,
            s.tk(),
        );
    });
    if sresp.hovered() {
        hint::set_text(ui.ctx(), &t("session.stop_all_tip"), "", Caps::NONE);
    }
    if !crate::controllers::learn_action_spot(app, ui, sa, crate::keymap::Action::SessionStopAll)
        && sresp.clicked()
        && !app.controllers.learn_mode
    {
        super::super::stop_all(app);
    }
}

/// A scene chosen: the cursor moves to its row (the same column, or the first).
pub(crate) fn choose_scene(app: &mut CantusApp, sc: cantus_core::session::SceneId) {
    let track = app
        .session_view
        .cursor
        .map(|c| c.track)
        .or_else(|| app.project().tracks.first().map(|t| t.id));
    if let Some(track) = track {
        app.session_view.cursor = Some(cantus_core::session::SlotRef::new(track, sc));
    }
    app.session_view.scene_chosen = true;
}

pub(crate) fn stops(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry, s: &Style) {
    let (cols, _) = geo.visible();
    let clip = geo.stops.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    let blink = blink_on(ui);
    let mut clicked = None;
    let mut buttons = Vec::with_capacity(cols.len());
    for i in cols.clone() {
        let r = geo.stop(i);
        let id = geo.tracks[i];
        let resp = ui.interact(
            r.intersect(clip),
            Id::new(("cantus-session-stop", id.0)),
            Sense::click(),
        );
        let on = app.session_view.live.playing_on(id);
        let playing = on.is_some();
        let stopping =
            on.and_then(|(_, p)| matches!(p, SlotPhase::Stopping { .. }).then_some(blink));
        buttons.push((r, playing, stopping, resp.hovered()));
        resp.widget_info(|| {
            let name = app
                .project()
                .track(id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let words = crate::i18n::tf("session.a11y.stop", &[("track", &name)]);
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, words)
        });
        if resp.hovered() {
            hint::set_text(ui.ctx(), &t("session.stop_tip"), "", Caps::NONE);
        }
        let learnt =
            crate::controllers::learn_spot_in(app, ui, r, clip, MappingTarget::SessionStop(id));
        if resp.clicked() && !learnt && !app.controllers.learn_mode {
            clicked = Some(id);
        }
    }
    with_brush(&painter, |b| {
        for (r, playing, stopping, hovered) in &buttons {
            paint::stop_button(b, *r, s, *playing, *stopping, *hovered);
        }
    });
    if let Some(id) = clicked {
        super::super::stop_track(app, id);
    }
}

/// The cursor's scene launcher in words, for the screen reader: its number or name, and
/// its tempo and metre when it sets them.
pub(crate) fn spoken_scene(app: &CantusApp) -> String {
    let p = app.project();
    let Some(c) = app.session_view.cursor else {
        return t("session.scenes");
    };
    let (Some(j), Some(sc)) = (p.session.scene_index(c.scene), p.session.scene(c.scene)) else {
        return t("session.scenes");
    };
    let name = if sc.name.is_empty() {
        (j + 1).to_string()
    } else {
        sc.name.clone()
    };
    let tag = scene_tag(sc).unwrap_or_default();
    crate::i18n::tf("session.a11y.scene", &[("scene", &name), ("tag", &tag)])
}
