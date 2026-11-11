//! r39:el-arrange — the sections lane (ADR 0113; `des-arrange`, *Sections lane*): the song's
//! named spans above the tracks, each with its length ("Drop · 8 bars"), the playing one in
//! ink 1, every other one a step lighter; the markers over them. A section under the
//! pointer lifts and shows its end grip; a click selects it, a double-click renames it,
//! a drag moves it (along the bars), its end's drag resizes it; its menu renames, recolours,
//! selects or loops its time and removes it. The lane's menu makes a section of the time
//! selection, or of the four bars under the pointer.

use super::super::slate::{say, width, Ink, W};
use super::super::{beats_to_ticks, chrome, ticks_to_beats, TimelineGeometry};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::{Brush, Fill, Style};
use cantus_core::arrange::{ArrangeCommand, Section, SectionId};
use cantus_core::{Command, MergeKey, Ticks};
use egui::{pos2, Align2, Id, Rect, Sense};

/// The lane's state between frames.
#[derive(Debug, Default)]
pub struct State {
    pub selected: Option<SectionId>,
    /// The section being renamed, and its words so far.
    pub rename: Option<(SectionId, String)>,
    /// A drag: the section, what it was at the press, and whether it moves or resizes.
    grab: Option<(SectionId, Section, bool)>,
    /// Each visible section's span on screen last frame (tests aim with it).
    pub drawn: Vec<(SectionId, Rect)>,
}

/// The sections' spans on screen, in the lane.
fn span(geo: &TimelineGeometry, sec: &Section, lane: Rect) -> Rect {
    Rect::from_min_max(
        pos2(geo.x(ticks_to_beats(sec.start)), lane.top()),
        pos2(geo.x(ticks_to_beats(sec.end())), lane.bottom()),
    )
}

/// How many sections start before `sec` (its rank: every other one is a step lighter).
fn rank(p: &cantus_core::Project, sec: &Section) -> usize {
    p.sections.iter().filter(|o| o.start < sec.start).count()
}

/// The lane: its ground and every section in view.
pub fn paint(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, lane: Rect, s: &Style) {
    let ink = Ink::of(s);
    let k = s.k;
    let ground = Ink::to(ink.surface, ink.base, 0.35);
    b.rect(b.snap_rect(lane), Fill::Solid(ground));
    let p = app.project();
    let pos = app.engine.position();
    let st = &app.arrangement.sections;
    let ts = p.time_signature;
    let bar_px = ts.numerator as f32 * 4.0 / ts.denominator as f32 * geo.px_per_beat;
    let pointer = b.ctx().input(|i| i.pointer.hover_pos());
    for (si, sec) in p.sections.iter().enumerate() {
        let r = span(geo, sec, lane);
        if r.right() < lane.left() || r.left() > lane.right() {
            continue;
        }
        let body = r.intersect(lane);
        if rank(p, sec) % 2 == 1 {
            b.rect(
                b.snap_rect(body),
                Fill::Solid(Ink::to(ground, ink.ink1, 0.035)),
            );
        }
        let selected = st.selected == Some(sec.id);
        let hovered = pointer.is_some_and(|q| body.contains(q));
        if selected {
            b.rrect(
                body.shrink(1.0 * k),
                s.r(3.0),
                Fill::Solid(Ink::a(ink.accent, 0.20)),
            );
            b.rrect_stroke(body.shrink(1.0 * k), s.r(3.0), b.px(), ink.accent);
        } else if hovered {
            b.rrect(
                body.shrink(1.0 * k),
                s.r(3.0),
                Fill::Solid(Ink::a(ink.ink1, 0.08)),
            );
        }
        if hovered || selected {
            let g = Rect::from_min_max(
                pos2(r.right() - 3.0 * k, lane.top() + 4.0 * k),
                pos2(r.right() - 1.0 * k, lane.bottom() - 4.0 * k),
            );
            b.rect(g, Fill::Solid(ink.ink3));
        }
        let playing = sec.contains(pos);
        let line = if playing {
            ink.ink3
        } else {
            Ink::a(ink.ink3, 0.7)
        };
        if r.left() >= lane.left() {
            b.vline(r.left(), lane.top(), lane.bottom(), b.px(), line);
        }
        if st.rename.as_ref().is_some_and(|(id, _)| *id == sec.id) {
            continue;
        }
        // The name keeps to the left edge when the section began before the view.
        let x = r.left().max(lane.left()) + 6.0 * k;
        let (w, c) = if playing {
            (W::Medium, ink.ink1)
        } else {
            (W::Regular, ink.ink2)
        };
        let room = Rect::from_min_max(
            pos2(x, lane.top()),
            pos2(r.right() - 4.0 * k, lane.bottom()),
        );
        if room.width() < 8.0 * k {
            continue;
        }
        let used = say(b, room, &sec.name, w, 11.0, c, Align2::LEFT_CENTER, s);
        if bar_px >= 20.0 * k {
            let len = app
                .arrangement
                .words
                .sections
                .get(si)
                .map_or("", String::as_str);
            let lx = used.map_or(x, |u| u.rect.right()) + 6.0 * k;
            let need = width(b.ctx(), len, W::Regular, 10.0, s);
            if lx + need <= room.right() {
                let r2 =
                    Rect::from_min_max(pos2(lx, lane.top()), pos2(room.right(), lane.bottom()));
                let _ = say(
                    b,
                    r2,
                    len,
                    W::Regular,
                    10.0,
                    ink.ink3,
                    Align2::LEFT_CENTER,
                    s,
                );
            }
        }
    }
}

/// The tick under `x`, on the bar unless `free`.
fn bar_at(app: &CantusApp, geo: &TimelineGeometry, x: f32, free: bool) -> Ticks {
    let t = beats_to_ticks(geo.beats(x).max(0.0));
    if free {
        t
    } else {
        let p = app.project();
        let start = crate::views::bars::bar_start(p, t);
        let len = crate::views::bars::bar_len(p, t);
        if (t - start).0 * 2 >= len.0 {
            start + len
        } else {
            start
        }
    }
}

/// The sections' gestures, over the lane's own response and under the markers' tabs.
pub fn input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    geo: &TimelineGeometry,
    lane: Rect,
    s: &Style,
) {
    let k = s.k;
    let mods = ui.input(|i| i.modifiers);
    let sections = app.project().sections.clone();
    let mut drawn = std::mem::take(&mut app.arrangement.sections.drawn);
    drawn.clear();
    for sec in &sections {
        let r = span(geo, sec, lane).intersect(lane);
        if r.width() <= 0.0 {
            continue;
        }
        drawn.push((sec.id, r));
        let end = Rect::from_min_max(pos2(r.right() - 5.0 * k, r.top()), r.max);
        let resp = ui.interact(r, Id::new(("section", sec.id.0)), Sense::click_and_drag());
        let on_end = resp.hover_pos().is_some_and(|q| end.contains(q));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(if on_end {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::Grab
            });
            let name = sec.name.clone();
            hint::set_text(
                ui.ctx(),
                &name,
                chrome::word("arrange.section.tip"),
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        if resp.double_clicked() {
            app.arrangement.sections.rename = Some((sec.id, sec.name.clone()));
        } else if resp.clicked() {
            app.arrangement.sections.selected = Some(sec.id);
        }
        if resp.drag_started_by(egui::PointerButton::Primary) {
            let press = ui.input(|i| i.pointer.press_origin());
            let resize = press.is_some_and(|q| end.contains(q));
            app.arrangement.sections.selected = Some(sec.id);
            app.arrangement.sections.grab = Some((sec.id, sec.clone(), resize));
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let (Some((id, was, resize)), Some(p)) = (
                app.arrangement.sections.grab.clone(),
                resp.interact_pointer_pos(),
            ) {
                if id == sec.id {
                    let press = ui.input(|i| i.pointer.press_origin()).map_or(p.x, |o| o.x);
                    let mut now = was.clone();
                    if resize {
                        let end = bar_at(app, geo, p.x, mods.command);
                        now.length = (end - was.start).max(Ticks(1));
                    } else {
                        let d = beats_to_ticks((p.x - press) / geo.px_per_beat);
                        let start = bar_at(
                            app,
                            geo,
                            geo.x(ticks_to_beats((was.start + d).max(Ticks::ZERO))),
                            mods.command,
                        );
                        now.start = start.max(Ticks::ZERO);
                    }
                    let current = app.project().section(id).cloned();
                    if current.as_ref() != Some(&now) {
                        // Refused where it would overlap: the section stays where it was.
                        app.exec_merging(
                            Command::Arrange(ArrangeCommand::SetSection { section: now }),
                            MergeKey::of(&("section", id.0)),
                        );
                    }
                }
            }
        }
        if resp.drag_stopped() {
            app.arrangement.sections.grab = None;
            app.end_gesture();
        }
        egui::Popup::context_menu(&resp)
            .frame(chrome::menu_frame())
            .show(|ui| section_menu(app, ui, sec.id));
    }
    app.arrangement.sections.drawn = drawn;
    rename(app, ui, geo, lane, s);
}

/// A section's menu.
fn section_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: SectionId) {
    use crate::kit::menu::Item;
    use crate::kit::text::Src;
    let Some(sec) = app.project().section(id).cloned() else {
        return;
    };
    let items = [
        Item::action(Src::Key("arrange.menu.rename")),
        Item::action(Src::Key("arrange.section.select_time")),
        Item::action(Src::Key("arrange.section.loop")),
        Item::Separator,
        Item::action(Src::Key("arrange.section.remove")),
    ];
    match crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
        Some(0) => app.arrangement.sections.rename = Some((id, sec.name.clone())),
        Some(1) => app.set_time_selection(sec.start, sec.end()),
        Some(2) => {
            app.exec(Command::Batch {
                label_key: "cmd.set_loop_region".into(),
                commands: vec![
                    Command::SetLoopRegion {
                        region: Some(cantus_core::LoopRegion {
                            start: sec.start,
                            end: sec.end(),
                        }),
                    },
                    Command::SetLoopEnabled { enabled: true },
                ],
            });
        }
        Some(4) => {
            app.exec(Command::Arrange(ArrangeCommand::RemoveSection { id }));
            app.arrangement.sections.selected = None;
        }
        _ => return,
    }
    ui.close();
}

/// Add a section over `from..to`, named for its place in the song ("Section 3"), unless
/// another one is there. Returns it, renaming.
pub fn add(app: &mut CantusApp, from: Ticks, to: Ticks) -> Option<SectionId> {
    if to <= from {
        return None;
    }
    let n = app.project().sections.len() + 1;
    let id = SectionId(app.bus.alloc_id());
    let section = Section {
        id,
        start: from,
        length: to - from,
        name: crate::i18n::tf("arrange.section.new_name", &[("n", &n)]),
        color: None,
    };
    let index = app.project().sections.len();
    let name = section.name.clone();
    if app.exec(Command::Arrange(ArrangeCommand::InsertSection {
        index,
        section,
    })) {
        app.arrangement.sections.selected = Some(id);
        app.arrangement.sections.rename = Some((id, name));
        Some(id)
    } else {
        None
    }
}

/// Add a section of four bars from the bar at `at` (the lane's menu), or of the time
/// selection when there is one.
pub fn add_here(app: &mut CantusApp, at: Ticks) -> Option<SectionId> {
    if let Some((a, b)) = app.time_selection.filter(|(a, b)| b > a) {
        return add(app, a, b);
    }
    let p = app.project();
    let start = crate::views::bars::bar_start(p, at);
    let end = crate::views::bars::bars_from(p, start, 4);
    // Up to the next section, if it begins sooner.
    let next = p
        .sections
        .iter()
        .filter(|s| s.start > start)
        .map(|s| s.start)
        .min();
    add(app, start, next.map_or(end, |n| n.min(end)))
}

/// The name editor over a section being renamed.
fn rename(app: &mut CantusApp, ui: &mut egui::Ui, geo: &TimelineGeometry, lane: Rect, s: &Style) {
    let Some((id, _)) = app.arrangement.sections.rename.clone() else {
        return;
    };
    let Some(sec) = app.project().section(id).cloned() else {
        app.arrangement.sections.rename = None;
        return;
    };
    let r = span(geo, &sec, lane).intersect(lane);
    let at = Rect::from_min_max(
        pos2(r.left() + 2.0 * s.k, r.top() + 1.0),
        pos2(r.right().max(r.left() + 90.0 * s.k), r.bottom() - 1.0),
    );
    let Some((_, text)) = app.arrangement.sections.rename.as_mut() else {
        return;
    };
    let out = crate::kit::field::Field::new(Id::new(("section-rename", id.0)))
        .opens_when_shown()
        .show(ui, at, text);
    match out.event {
        Some(crate::kit::field::Event::Commit) => {
            if let Some((_, text)) = app.arrangement.sections.rename.take() {
                let name = text.trim().to_string();
                if !name.is_empty() && name != sec.name {
                    app.exec(Command::Arrange(ArrangeCommand::SetSection {
                        section: Section { name, ..sec },
                    }));
                }
            }
        }
        Some(crate::kit::field::Event::Cancel) => app.arrangement.sections.rename = None,
        None => {}
    }
}
