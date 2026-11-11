//! r39:el-arrange — the add-track menu and the track's menu (`des-arrange`, *Add track* and
//! *Track menu*): one list each, a submenu opening in the same place (Height ▸,
//! Automation ▸, Recording and more ▸) with a way back.

use super::folders;
use super::head::HEIGHTS;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{self, Style};
use crate::theme;
use cantus_core::{Command, TrackId};
use egui::{vec2, Sense};

/// Which page of the track's menu is open.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Main,
    Height,
    Automation,
    More,
}

const W: f32 = 236.0;

fn act(key: &str) -> Item<'_> {
    Item::action(Src::Key(key))
}

fn sub(label: Src<'_>) -> Item<'_> {
    Item::Action {
        label,
        check: Check::None,
        shortcut: None,
        submenu: true,
        enabled: true,
    }
}

fn keyed<'a>(key: &'a str, shortcut: &'a str) -> Item<'a> {
    Item::Action {
        label: Src::Key(key),
        check: Check::None,
        shortcut: (!shortcut.is_empty()).then_some(shortcut),
        submenu: false,
        enabled: true,
    }
}

/// The add-track menu (the row under the last track, and the corner's plus).
pub fn add_track(app: &mut CantusApp, ui: &mut egui::Ui, index: Option<usize>) {
    use crate::keymap::Action;
    let audio_key = app.keymap.display_for(Action::AddAudioTrack);
    // r39:ux-b — the instruments chosen last, one click away (ADR 0110).
    let recent = crate::quick_devices::recent(app, crate::picker_catalog::Role::Instrument);
    let words: Vec<String> = recent
        .iter()
        .map(|q| tf("workflow.palette.add_instrument", &[("name", &q.name)]))
        .collect();
    let mut items = vec![act("arrange.add.instrument")];
    // The instruments chosen last right under it (one click, little travel).
    if !words.is_empty() {
        items.push(Item::Band(Src::Key("ux.paths.recent_instruments")));
        items.extend(words.iter().map(|w| Item::action(Src::Str(w))));
        items.push(Item::Separator);
    }
    let first_recent = 2;
    let more = items.len();
    items.extend([
        keyed("arrange.add.audio", &audio_key),
        act("arrange.add.drums"),
        act("arrange.add.folder"),
        Item::Separator,
        sub(Src::Key("arrange.add.preset")),
        act("arrange.add.file"),
        Item::Separator,
        act("arrange.add.return"),
        act("arrange.add.automation"),
    ]);
    let purpose = crate::picker::PickPurpose::AddInstrument { index };
    let clicked = kit::widgets::menu_list(ui, &items, W).1.clicked;
    // The rows after the recents, numbered as without them.
    let clicked = match clicked {
        Some(0) => Some(0),
        Some(i) if !words.is_empty() && i >= first_recent && i - first_recent < recent.len() => {
            crate::quick_devices::add(app, &recent[i - first_recent].key, purpose);
            ui.close();
            return;
        }
        Some(i) if i >= more => Some(i - more + 1),
        _ => None,
    };
    match clicked {
        Some(0) => app.open_picker(purpose),
        Some(1) => app.add_audio_track(),
        Some(2) => {
            app.add_track();
            if let Some(id) = app.selected_track {
                app.exec(Command::SetInstrument {
                    track: id,
                    spec: Box::new(cantus_core::InstrumentSpec::Pulsus(Default::default())),
                });
            }
        }
        Some(3) => {
            let tracks: Vec<TrackId> = app.selected_track.into_iter().collect();
            let _ = folders::new_folder(app, &tracks, &t("arrange.folder.new_name"));
        }
        Some(5) => app.open_picker(purpose),
        Some(6) => app.import_audio_dialog(),
        Some(8) => {
            let from: Vec<cantus_core::StripId> = app
                .selected_track
                .and_then(|t| app.project().track(t))
                .and_then(|t| t.strip)
                .into_iter()
                .collect();
            let _ = app.new_return(&from, crate::routing_actions::ReturnKind::Reverb);
        }
        // The control touched last gets its lane (ADR 0027's "automate last tweaked").
        Some(9) => app.run_action(Action::AutomateLastTweaked),
        _ => return,
    }
    ui.close();
}

/// The track's menu: what a header's right-click and its "more" open.
pub fn track(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, to_delete: &mut Option<TrackId>) {
    let page = match app.arrangement.menu_page {
        Some((t, p)) if t == id => p,
        _ => Page::Main,
    };
    let done = match page {
        Page::Main => main(app, ui, id, to_delete),
        Page::Height => height(app, ui, id),
        Page::Automation => back(app, ui, id) || super::autolanes::track_menu(app, ui, id),
        Page::More => back(app, ui, id) || more(app, ui, id),
    };
    if done {
        app.arrangement.menu_page = None;
        ui.close();
    }
}

/// "‹ Back": the main page again.
fn back(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    let items = [act("arrange.menu.back"), Item::Separator];
    if kit::widgets::menu_list(ui, &items, W).1.clicked == Some(0) {
        app.arrangement.menu_page = Some((id, Page::Main));
    }
    false
}

fn main(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    to_delete: &mut Option<TrackId>,
) -> bool {
    use crate::keymap::Action;
    let Some(tr) = app.project().track(id) else {
        return true;
    };
    let in_folder = tr.folder.is_some();
    let frozen = tr.frozen.is_some();
    let name = tr.name.clone();
    let inst = tf(
        "arrange.menu.instrument",
        &[("name", &crate::swap::instrument_name(&tr.instrument))],
    );
    let h_now = tr.height();
    let height_now = HEIGHTS
        .iter()
        .find(|(_, h)| (h_now - h / super::head::ROW).abs() < 0.02)
        .map_or_else(|| t("arrange.height.custom"), |(k, _)| t(k));
    let height_label = tf("arrange.menu.height", &[("now", &height_now)]);
    let dup = app.keymap.display_for(Action::Duplicate);
    let s = Style::current();
    // Rename, then the colour row.
    if kit::widgets::menu_list(ui, &[keyed("arrange.menu.rename", "F2")], W)
        .1
        .clicked
        == Some(0)
    {
        app.arrangement.rename = Some((id, name));
        return true;
    }
    if colours(app, ui, id, &s) {
        return true;
    }
    let items = [
        Item::Separator,
        sub(Src::Str(&inst)),
        sub(Src::Str(&height_label)),
        sub(Src::Key("arrange.menu.automation")),
        act("arrange.menu.icon"),
        Item::Separator,
        act("arrange.menu.insert_above"),
        act("arrange.menu.insert_below"),
        keyed("arrange.menu.duplicate", &dup),
        act(if in_folder {
            "arrange.menu.out_of_folder"
        } else {
            "arrange.menu.new_folder"
        }),
        Item::Separator,
        act(if frozen {
            "arrange.menu.unfreeze"
        } else {
            "arrange.menu.freeze"
        }),
        act("arrange.menu.show_mixer"),
        sub(Src::Key("arrange.menu.more")),
        Item::Separator,
        act("arrange.menu.delete"),
    ];
    let index = app.project().track_index(id).unwrap_or(0);
    match kit::widgets::menu_list(ui, &items, W).1.clicked {
        Some(1) => {
            app.select_track(id);
            app.open_picker(crate::picker::PickPurpose::ReplaceInstrument { track: id });
        }
        Some(2) => {
            app.arrangement.menu_page = Some((id, Page::Height));
            return false;
        }
        Some(3) => {
            app.arrangement.menu_page = Some((id, Page::Automation));
            return false;
        }
        Some(4) => {
            let at = ui.min_rect().left_top();
            app.arrangement.glyphs.open_for(id, at, ui.ctx());
        }
        Some(6) => {
            app.open_picker(crate::picker::PickPurpose::AddInstrument { index: Some(index) })
        }
        Some(7) => app.open_picker(crate::picker::PickPurpose::AddInstrument {
            index: Some(index + 1),
        }),
        Some(8) => duplicate(app, id),
        Some(9) if in_folder => {
            app.exec(Command::Arrange(
                cantus_core::ArrangeCommand::SetTrackFolder {
                    track: id,
                    folder: None,
                },
            ));
        }
        Some(9) => {
            let _ = folders::new_folder(app, &[id], &t("arrange.folder.new_name"));
        }
        Some(11) => app.toggle_freeze_track(id),
        Some(12) => {
            if let Some(strip) = app.project().track(id).and_then(|t| t.strip) {
                app.select_strip(strip);
            }
            app.show_pane(crate::views::workspace::Pane::Mixer);
        }
        Some(13) => {
            app.arrangement.menu_page = Some((id, Page::More));
            return false;
        }
        Some(15) => *to_delete = Some(id),
        _ => return false,
    }
    true
}

/// The colour row: eleven swatches, the chosen one ringed.
fn colours(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, s: &Style) -> bool {
    let now = app.project().track(id).map(|t| t.color);
    let (rect, _) = ui.allocate_exact_size(vec2(W, s.d(26.0)), Sense::hover());
    kit::with_brush(ui.painter(), |b| kit::menu::paint(b, rect, s, &[], None));
    let mut chosen = None;
    let d = s.s(11.0);
    let mut x = rect.left() + s.s(16.0);
    for c in cantus_core::Color::PALETTE.iter() {
        let r = egui::Rect::from_center_size(egui::pos2(x + d * 0.5, rect.center().y), vec2(d, d));
        let resp = ui.interact(
            r.expand(s.s(2.0)),
            ui.id().with(("arr-swatch", id.0, x as i32)),
            Sense::click(),
        );
        kit::with_brush(ui.painter(), |b| {
            b.circle(
                r.center(),
                d * 0.5,
                kit::Fill::Solid(theme::track_color(*c)),
            );
            if Some(*c) == now {
                b.circle_stroke(r.center(), d * 0.5 + s.s(2.0), s.s(1.2), s.t.menu_text);
            } else if resp.hovered() {
                b.circle_stroke(r.center(), d * 0.5 + s.s(2.0), s.s(1.0), s.t.text_dim);
            }
        });
        if resp.clicked() {
            chosen = Some(*c);
        }
        x += d + s.s(8.0);
    }
    if let Some(color) = chosen {
        app.exec(Command::SetTrackColor { id, color });
        return true;
    }
    false
}

fn height(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    back(app, ui, id);
    let now = app.project().track(id).map_or(1.0, |t| t.height());
    let mut items: Vec<Item<'_>> = HEIGHTS
        .iter()
        .map(|(k, h)| Item::Action {
            label: Src::Key(k),
            check: Check::Radio((now - h / super::head::ROW).abs() < 0.02),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    items.push(Item::Separator);
    items.push(act("arrange.height.fit"));
    items.push(act("arrange.height.all"));
    match kit::widgets::menu_list(ui, &items, W).1.clicked {
        Some(i) if i < HEIGHTS.len() => {
            app.exec(Command::SetTrackHeight {
                id,
                height: HEIGHTS[i].1 / super::head::ROW,
            });
            true
        }
        Some(i) if i == HEIGHTS.len() + 1 => {
            fit_window(app);
            true
        }
        Some(i) if i == HEIGHTS.len() + 2 => {
            app.set_all_track_heights(now);
            true
        }
        _ => false,
    }
}

/// Every track's height so the song fills the arrangement's height (one step).
pub fn fit_window(app: &mut CantusApp) {
    let Some(geo) = app.arrangement.geometry() else {
        return;
    };
    let n = app.project().tracks.len().max(1) as f32;
    let room = (geo_room(&geo) / n / app.arrangement.track_height)
        .clamp(cantus_core::MIN_TRACK_HEIGHT, cantus_core::MAX_TRACK_HEIGHT);
    app.set_all_track_heights(room);
}

fn geo_room(geo: &super::TimelineGeometry) -> f32 {
    (geo.lanes_bottom - geo.body_top).max(1.0)
}

/// Recording, pitch, external gear, stems and controllers: the menus each feature keeps.
fn more(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    super::rec::track_menu(app, ui, id)
        || super::super::pitch_lane::track_menu(app, ui, id)
        || super::super::freeze_view::track_menu(app, ui, id)
        || crate::external_service::track_menu(app, ui, id)
        || crate::stems_ui::track_menu(app, ui, id)
        || crate::controllers::track_menu(app, ui, id)
        || icon_menu(app, ui, id)
}

/// The type icon (what the instrument shows), as before: chosen or automatic.
fn icon_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    let chosen = app.project().track(id).and_then(|t| t.icon);
    let mut items = vec![Item::Band(Src::Key("track.icon"))];
    for icon in cantus_core::TrackIcon::ALL {
        items.push(Item::Action {
            label: Src::Key(icon.label_key()),
            check: Check::Radio(chosen == Some(icon)),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    items.push(Item::Action {
        label: Src::Key("track.icon_auto"),
        check: Check::Radio(chosen.is_none()),
        shortcut: None,
        submenu: false,
        enabled: true,
    });
    match kit::widgets::menu_list(ui, &items, W).1.clicked {
        Some(i) if (1..=cantus_core::TrackIcon::ALL.len()).contains(&i) => {
            app.exec(Command::SetTrackIcon {
                id,
                icon: Some(cantus_core::TrackIcon::ALL[i - 1]),
            });
            true
        }
        Some(i) if i == cantus_core::TrackIcon::ALL.len() + 1 => {
            app.exec(Command::SetTrackIcon { id, icon: None });
            true
        }
        _ => false,
    }
}

/// A copy of a track under it: its clips with fresh ids, its lanes' targets its own.
pub fn duplicate(app: &mut CantusApp, id: TrackId) {
    let Some(src) = app.project().track(id).cloned() else {
        return;
    };
    let index = app.project().track_index(id).unwrap_or(0) + 1;
    let new_id = TrackId(app.bus.alloc_id());
    let mut tr = src.clone();
    tr.id = new_id;
    tr.name = tf("arrange.copy_name", &[("name", &src.name)]);
    tr.armed = false;
    tr.frozen = None;
    tr.slots.clear();
    tr.modulators.clear();
    tr.comps.clear();
    tr.active_comp = 0;
    tr.lane_modes.clear();
    let bus = &mut app.bus;
    let mut ids = || bus.alloc_id();
    for fx in &mut tr.note_fx {
        fx.id = cantus_core::NoteFxId(ids());
    }
    tr.clips = src
        .clips
        .iter()
        .map(|c| {
            let mut n = cantus_core::session::edit::fresh_clip(c, &mut ids);
            n.lane = c.lane;
            if let Some(a) = n.automation.as_mut() {
                retarget(&mut a.target, id, new_id);
            }
            n
        })
        .collect();
    if app.exec(Command::InsertTrack {
        index,
        track: Box::new(tr),
    }) {
        app.select_track(new_id);
    }
}

fn retarget(t: &mut cantus_core::AutoTarget, from: TrackId, to: TrackId) {
    use cantus_core::AutoTarget as A;
    match t {
        A::TrackGain(x) | A::TrackPan(x) if *x == from => *x = to,
        A::Instrument { track, .. }
        | A::Firmus { track, .. }
        | A::PluginInstrument { track, .. }
            if *track == from =>
        {
            *track = to
        }
        _ => {}
    }
}
