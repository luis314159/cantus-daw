//! The controller mappings pane (round 29, ADR 0062 §5): every MIDI mapping — the user's
//! global map and the project's, in two sections — with its control (port, channel,
//! message), its target in words, its mode, the range it covers in the target's own units,
//! its bend, its takeover and its feedback, each edited in place with the kit's controls; a
//! live dot when a message arrives; *learn again* and *remove* on every row; a filter; and
//! a header with the learn mode, *Add* (the targets a global map can hold: the transport,
//! the tempo, the master, the selected track and channel, the focused device's knobs) and
//! the takeover new mappings get. A mapping whose target is gone stays, dimmed and marked;
//! two mappings on one control are allowed (FL's multilink) and flagged *Shared*. FL's
//! *Remote control settings* and Ableton's mapping browser are the references.

use crate::app::CantusApp;
use crate::controllers::{
    control_text, channel_text, mode_text, port_text, takeover_text, Scope,
};
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{LedButton, MenuField};
use crate::kit::{band, with_brush, Fill, State, Style};
use crate::views::workspace::Pane;
use cantus_core::controller::{
    DeskParam, Encoding, MapMode, Mapping, MappingId, MappingTarget, Takeover, DEVICE_PARAMS,
    TEMPO_TRAVEL,
};
use cantus_core::{AutoTarget, MergeKey};
use egui::{Align2, Id, Rect, Sense, Ui, Vec2};
use std::time::Duration;

/// The pane's state.
#[derive(Debug, Default)]
pub struct ViewState {
    /// What the filter field holds: rows whose control or target contain it.
    pub filter: String,
    /// Published geometry for tests: (name, rect) of the controls drawn last frame.
    pub geo: Vec<(String, Rect)>,
}

/// Show or hide the pane (the View menu's entry and Ctrl+Shift+K).
pub fn toggle_pane(app: &mut CantusApp) {
    app.toggle_pane(Pane::Controllers);
}

/// How long a mapping's dot stays lit after a message.
const LIT: Duration = Duration::from_millis(300);

/// The columns at 100 %: the dot, control, target (the rest, at least this), mode, from,
/// to, curve, takeover, feedback, learn again, remove.
const COLS: [f32; 11] = [
    16.0, 210.0, 220.0, 150.0, 92.0, 92.0, 70.0, 104.0, 76.0, 28.0, 28.0,
];
/// The row's height and the gap between cells, at 100 %.
const ROW_H: f32 = 28.0;
const GAP: f32 = 6.0;
/// The header's height at 100 %.
const HEAD_H: f32 = 34.0;

/// The columns' rects in a row at `rect` (the target column takes what is left).
fn columns(rect: Rect, k: f32) -> [Rect; 11] {
    let fixed: f32 = COLS.iter().enumerate().filter(|(i, _)| *i != 2).map(|(_, w)| w).sum();
    let gaps = GAP * (COLS.len() as f32 - 1.0);
    let target = (rect.width() / k - fixed - gaps).max(COLS[2]);
    let mut x = rect.left() + 4.0 * k;
    let mut out = [Rect::NOTHING; 11];
    for (i, w) in COLS.iter().enumerate() {
        let w = if i == 2 { target } else { *w } * k;
        out[i] = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(w, rect.height()));
        x += w + GAP * k;
    }
    out
}

/// The table's whole width at 100 % (for the horizontal scroll).
fn table_width() -> f32 {
    COLS.iter().sum::<f32>() + GAP * (COLS.len() as f32 - 1.0) + 8.0
}

/// Text in a cell, elided, at the left.
fn cell_text(ui: &Ui, rect: Rect, s: &Style, words: &str, color: egui::Color32, role: Role) {
    with_brush(ui.painter(), |b| {
        text::paint(
            b,
            rect,
            &Text::new(words, role, color)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

/// The pane.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    app.controllers.view.geo.clear();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, s.t.ink);
    let head = Rect::from_min_size(full.min, Vec2::new(full.width(), HEAD_H * k));
    header(app, ui, head, &s);
    let mut top = head.bottom();
    if let Some(note) = learning_note(app) {
        let r = Rect::from_min_size(egui::pos2(full.left(), top), Vec2::new(full.width(), 26.0 * k));
        with_brush(ui.painter(), |b| {
            b.rect(r, Fill::Solid(s.t.led.gamma_multiply(0.16)));
        });
        cell_text(ui, r.shrink2(Vec2::new(10.0 * k, 0.0)), &s, &note, s.t.text, Role::Body);
        top = r.bottom();
    }
    let body = Rect::from_min_max(egui::pos2(full.left(), top), full.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
    egui::ScrollArea::both()
        .id_salt("controllers-pane")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            let w = (table_width() * k).max(ui.available_width());
            ui.set_min_width(w);
            section(app, ui, Scope::Global, w);
            ui.add_space(8.0 * k);
            section(app, ui, Scope::Project, w);
            ui.add_space(12.0 * k);
        });
    // The dots fade: keep drawing while one is lit.
    if app
        .controllers
        .activity
        .values()
        .any(|t| t.elapsed() < LIT)
    {
        ui.ctx().request_repaint_after(Duration::from_millis(40));
    }
}

/// While a target is armed: what to do, in words.
fn learning_note(app: &CantusApp) -> Option<String> {
    let l = app.controllers.learning.as_ref()?;
    Some(tf(
        "ctl.view.armed",
        &[("name", &app.mapping_target_name(&l.target))],
    ))
}

/// Publish a control's rect for the tests.
fn put(app: &mut CantusApp, name: impl Into<String>, r: Rect) {
    app.controllers.view.geo.push((name.into(), r));
}

/// A list opened from `field`; the option clicked.
fn pick<T: Clone + PartialEq>(
    field: &egui::Response,
    options: &[(T, String)],
    current: Option<&T>,
    width: f32,
) -> Option<T> {
    let mut chosen = None;
    egui::Popup::menu(field)
        .frame(crate::kit::automate::frame())
        .show(|ui| {
            let items: Vec<Item<'_>> = options
                .iter()
                .map(|(v, label)| Item::Action {
                    label: Src::Str(label),
                    check: Check::Radio(current == Some(v)),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            if let Some(i) = crate::kit::widgets::menu_list(ui, &items, width).1.clicked {
                chosen = options.get(i).map(|o| o.0.clone());
                ui.close();
            }
        });
    chosen
}

/// The header: the learn mode, *Add*, the filter, the takeover new mappings get, the ports.
fn header(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        b.rect(r, Fill::Solid(s.t.panel));
        b.hline(r.left(), r.right(), r.bottom() - b.px(), b.px(), s.t.edge);
    });
    let h = 22.0 * k;
    let y = r.center().y - h * 0.5;
    let mut x = r.left() + 8.0 * k;
    let mut at = |w: f32| {
        let rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w * k, h));
        x += (w + 8.0) * k;
        rect
    };
    // The learn mode (Ableton's MIDI Map mode).
    let lr = at(96.0);
    let learn = LedButton::new(app.controllers.learn_mode, Src::Key("ctl.view.learn"))
        .name(&t("ctl.view.learn_tip"))
        .id(Id::new("ctl-pane-learn"))
        .show_at(ui, lr);
    put(app, "learn", lr);
    if learn.clicked() {
        app.toggle_learn_mode();
    }
    // Add: a target a global map can hold, armed to learn.
    let ar = at(150.0);
    let add_words = t("ctl.view.add");
    let (add, _) = MenuField::new(&add_words)
        .id(Id::new("ctl-pane-add"))
        .show_at(ui, ar);
    put(app, "add", ar);
    add_menu(app, &add);
    // The filter.
    let fr = at(190.0);
    let hint_words = t("ctl.view.filter");
    let mut filter = std::mem::take(&mut app.controllers.view.filter);
    crate::kit::field::Field::new(Id::new("ctl-pane-filter"))
        .hint(&hint_words)
        .show(ui, fr, &mut filter);
    app.controllers.view.filter = filter;
    put(app, "filter", fr);
    // The takeover new mappings get.
    let tl = at(118.0);
    cell_text(ui, tl, s, &t("ctl.view.new_takeover"), s.t.text_dim, Role::Label);
    let tr = at(110.0);
    let current = app.settings.controller_takeover;
    let words = takeover_text(current);
    let (field, _) = MenuField::new(&words)
        .id(Id::new("ctl-pane-takeover"))
        .show_at(ui, tr);
    put(app, "new_takeover", tr);
    let options: Vec<(Takeover, String)> = [Takeover::Jump, Takeover::Pickup, Takeover::Scale]
        .into_iter()
        .map(|k| (k, takeover_text(k)))
        .collect();
    if let Some(k) = pick(&field, &options, Some(&current), tr.width()) {
        app.settings.controller_takeover = k;
        app.store_settings();
    }
    // The ports, at the right: what is plugged in.
    let ports = if app.midi_port_names.is_empty() {
        t("ctl.view.no_ports")
    } else {
        tf(
            "ctl.view.ports",
            &[("names", &app.midi_port_names.join(", "))],
        )
    };
    let pr = Rect::from_min_max(egui::pos2(x, y), egui::pos2(r.right() - 8.0 * k, y + h));
    if pr.width() > 40.0 * k {
        with_brush(ui.painter(), |b| {
            text::paint(
                b,
                pr,
                &Text::new(&ports, Role::Label, s.t.text_dim)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
    }
}

/// Every target a global map can hold, in sections: what *Add* offers.
pub fn portable_targets() -> Vec<(Option<&'static str>, MappingTarget)> {
    use crate::keymap::{Action, Category};
    let mut out: Vec<(Option<&'static str>, MappingTarget)> = Vec::new();
    let band = |out: &mut Vec<(Option<&'static str>, MappingTarget)>, key: &'static str| {
        out.push((Some(key), MappingTarget::Tempo));
    };
    band(&mut out, "ctl.add.transport");
    for a in Action::ALL
        .into_iter()
        .filter(|a| a.category() == Category::Transport)
    {
        out.push((None, MappingTarget::Action(a.id().to_string())));
    }
    out.push((None, MappingTarget::Tempo));
    band(&mut out, "ctl.add.master");
    for t in [
        AutoTarget::MasterGain,
        AutoTarget::MasterPan,
        AutoTarget::MasterWidth,
    ] {
        out.push((None, MappingTarget::Param(t)));
    }
    band(&mut out, "ctl.add.selected_track");
    for d in DeskParam::ALL {
        out.push((None, MappingTarget::SelectedTrack(d)));
    }
    band(&mut out, "ctl.add.selected_strip");
    for d in DeskParam::ALL.into_iter().filter(|d| *d != DeskParam::Arm) {
        out.push((None, MappingTarget::SelectedStrip(d)));
    }
    band(&mut out, "ctl.add.device");
    for n in 0..DEVICE_PARAMS {
        out.push((None, MappingTarget::DeviceParam(n)));
    }
    out
}

/// *Add*'s list: a click arms the target (the next control moved is mapped to it).
fn add_menu(app: &mut CantusApp, field: &egui::Response) {
    let entries = portable_targets();
    let names: Vec<Option<String>> = entries
        .iter()
        .map(|(band, t)| match band {
            Some(_) => None,
            None => Some(app.mapping_target_name(t)),
        })
        .collect();
    let mut chosen = None;
    egui::Popup::menu(field)
        .frame(crate::kit::automate::frame())
        .show(|ui| {
            let items: Vec<Item<'_>> = entries
                .iter()
                .zip(&names)
                .map(|((band, _), name)| match (band, name) {
                    (Some(key), _) => Item::Band(Src::Key(key)),
                    (None, Some(n)) => Item::action(Src::Str(n)),
                    (None, None) => Item::Separator,
                })
                .collect();
            let s = Style::current();
            egui::ScrollArea::vertical()
                .max_height(420.0 * s.k)
                .show(ui, |ui| {
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 280.0).1.clicked {
                        chosen = entries.get(i).map(|e| e.1.clone());
                        ui.close();
                    }
                });
        });
    if let Some(target) = chosen {
        app.learn_target(target);
    }
}

/// The modes a mapping can take: an action fires; a key or a pad cannot be an encoder.
fn modes_for(m: &Mapping) -> Vec<MapMode> {
    if m.target.is_action() {
        return vec![MapMode::Trigger];
    }
    let mut out = vec![MapMode::Absolute];
    if !m.source.control.is_button() {
        out.extend(Encoding::ALL.map(MapMode::Relative));
    }
    out.extend([MapMode::Toggle, MapMode::Momentary, MapMode::Trigger]);
    out
}

/// A stand-in parameter whose range and words a relative target's value shares.
fn proxy(target: &MappingTarget) -> Option<AutoTarget> {
    match target {
        MappingTarget::SelectedTrack(DeskParam::Volume)
        | MappingTarget::SelectedStrip(DeskParam::Volume) => Some(AutoTarget::MasterGain),
        MappingTarget::SelectedTrack(DeskParam::Pan)
        | MappingTarget::SelectedStrip(DeskParam::Pan) => Some(AutoTarget::MasterPan),
        _ => None,
    }
}

/// A position of `target`'s travel in its own words: "−6.0 dB", "1.20 kHz", "On", "96 BPM".
pub fn value_words(app: &CantusApp, target: &MappingTarget, pos: f32) -> String {
    let p = app.project();
    let pct = |pos: f32| format!("{:.0} %", pos * 100.0);
    if target.is_switch() {
        return t(if pos >= 0.5 { "ctl.value.on" } else { "ctl.value.off" });
    }
    match target {
        MappingTarget::Param(a) if a.is_plugin() => pct(pos),
        MappingTarget::Param(a) => match cantus_core::automation::range::range_of(p, a) {
            Some(r) => {
                let mut out = String::new();
                crate::automation::write_value(p, a, r.value(pos), &mut out);
                out
            }
            None => pct(pos),
        },
        MappingTarget::Tempo => {
            let (lo, hi) = TEMPO_TRAVEL;
            format!("{:.0} BPM", lo + f64::from(pos) * (hi - lo))
        }
        MappingTarget::Action(_) => "\u{2014}".to_string(),
        other => match proxy(other) {
            Some(a) => {
                let r = cantus_core::automation::range::range_of(p, &a).expect("master");
                let mut out = String::new();
                crate::automation::write_value(p, &a, r.value(pos), &mut out);
                out
            }
            None => pct(pos),
        },
    }
}

/// What was typed into a value cell, as a position of `target`'s travel.
pub fn parse_value(app: &CantusApp, target: &MappingTarget, typed: &str) -> Option<f32> {
    let w = typed.trim().to_lowercase();
    if target.is_switch() {
        let on = t("ctl.value.on").to_lowercase();
        let off = t("ctl.value.off").to_lowercase();
        return match w.as_str() {
            x if x == on || x == "1" || x == "on" => Some(1.0),
            x if x == off || x == "0" || x == "off" => Some(0.0),
            _ => None,
        };
    }
    // Pan: "C", "L 30", "R 30".
    let signed = if let Some(rest) = w.strip_prefix('l') {
        format!("-{}", rest.trim())
    } else if let Some(rest) = w.strip_prefix('r') {
        rest.trim().to_string()
    } else if w == "c" {
        "0".to_string()
    } else {
        w.clone()
    };
    let num: String = signed
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | ','))
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    let mut v: f32 = num.parse().ok()?;
    if w.contains('k') && !w.contains("bpm") {
        v *= 1000.0;
    }
    let p = app.project();
    let pos = match target {
        MappingTarget::Param(a) if !a.is_plugin() => {
            let r = cantus_core::automation::range::range_of(p, a)?;
            let v = match a {
                AutoTarget::TrackPan(_) | AutoTarget::StripPan(_) | AutoTarget::MasterPan => {
                    v / 100.0
                }
                AutoTarget::StripWidth(_)
                | AutoTarget::MasterWidth
                | AutoTarget::Send { .. }
                | AutoTarget::ModAmount(_) => v / 100.0,
                _ => v,
            };
            r.pos(v)
        }
        MappingTarget::Tempo => {
            let (lo, hi) = TEMPO_TRAVEL;
            ((f64::from(v) - lo) / (hi - lo)) as f32
        }
        other => match proxy(other) {
            Some(a) => {
                let r = cantus_core::automation::range::range_of(p, &a)?;
                r.pos(if a == AutoTarget::MasterPan { v / 100.0 } else { v })
            }
            None => v / 100.0,
        },
    };
    pos.is_finite().then(|| pos.clamp(0.0, 1.0))
}

/// A glyph key filling `rect`.
fn glyph_at(ui: &Ui, rect: Rect, id: Id, g: Glyph, lit: bool, tip: &str) -> egui::Response {
    let s = Style::current();
    let resp = ui.interact(rect, id, Sense::click());
    let st = State::of(&resp, lit);
    if st.hovered {
        hint::set_text(ui.ctx(), tip, "", Caps::NONE);
    }
    let side = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), Vec2::splat(side));
    with_brush(ui.painter(), |b| {
        crate::kit::cap::toolbar_key(
            b,
            r,
            &s.scaled(side / 28.0),
            st,
            crate::kit::cap::KeyFace::Glyph(g),
        )
    });
    resp
}

/// What a value cell did.
struct CellDone {
    /// The new position, while dragged or once typed.
    pos: Option<f32>,
    /// The drag let go, or a value was typed: the end of the undo step.
    done: bool,
}

/// A value cell: the words in a well, a bar for where it is; drag (Shift fine) to change,
/// double-click to type, right-click resets to `default`.
#[allow(clippy::too_many_arguments)]
fn value_cell(
    app: &mut CantusApp,
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    name: &str,
    words: &str,
    pos: f32,
    default: f32,
    target: &MappingTarget,
) -> CellDone {
    let s = Style::current();
    let typing = ui.ctx().data(|d| d.get_temp::<bool>(id.with("typing")).unwrap_or(false));
    if typing {
        let mut text = ui
            .ctx()
            .data(|d| d.get_temp::<String>(id.with("text")))
            .unwrap_or_else(|| words.to_string());
        let out = crate::kit::field::Field::new(id.with("field"))
            .open(true)
            .centered()
            .show(ui, rect, &mut text);
        ui.ctx().data_mut(|d| d.insert_temp(id.with("text"), text.clone()));
        if let Some(ev) = out.event {
            ui.ctx().data_mut(|d| {
                d.remove::<bool>(id.with("typing"));
                d.remove::<String>(id.with("text"));
            });
            if ev == crate::kit::field::Event::Commit && out.edited {
                if let Some(p) = parse_value(app, target, &text) {
                    return CellDone {
                        pos: Some(p),
                        done: true,
                    };
                }
                app.set_error(t("ctl.status.not_a_value"));
            }
        }
        return CellDone {
            pos: None,
            done: false,
        };
    }
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let st = State::of(&resp, false);
    if st.hovered {
        hint::set_text(ui.ctx(), name, words, Caps::NONE);
    }
    with_brush(ui.painter(), |b| {
        let r = rect.shrink2(Vec2::new(0.0, 3.0 * s.k));
        b.rrect(r, s.r(3.0), Fill::Solid(s.t.well));
        let bar = Rect::from_min_size(
            egui::pos2(r.left(), r.bottom() - 2.0 * s.k),
            Vec2::new(r.width() * pos.clamp(0.0, 1.0), 2.0 * s.k),
        );
        b.rect(bar, Fill::Solid(s.t.highlight.gamma_multiply(0.8)));
        if st.hovered {
            b.rrect_stroke(r, s.r(3.0), s.s(1.0), s.t.focus);
        }
        text::paint(
            b,
            r,
            &Text::new(words, Role::Value, s.t.text).policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    if resp.double_clicked() {
        ui.ctx().data_mut(|d| {
            d.insert_temp(id.with("typing"), true);
            d.insert_temp(id.with("text"), words.to_string());
        });
        return CellDone {
            pos: None,
            done: false,
        };
    }
    if resp.secondary_clicked() {
        return CellDone {
            pos: Some(default),
            done: true,
        };
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        let fine = if ui.input(|i| i.modifiers.shift) { 0.2 } else { 1.0 };
        let p = (pos + (d.x - d.y) * fine / (160.0 * s.k)).clamp(0.0, 1.0);
        return CellDone {
            pos: (p != pos).then_some(p),
            done: false,
        };
    }
    CellDone {
        pos: None,
        done: resp.drag_stopped(),
    }
}

/// One map's section: its band, the column titles, its rows (filtered).
fn section(app: &mut CantusApp, ui: &mut Ui, scope: Scope, w: f32) {
    let s = Style::current();
    let k = s.k;
    let (title, empty) = match scope {
        Scope::Global => ("ctl.view.global", "ctl.view.global_empty"),
        Scope::Project => ("ctl.view.project", "ctl.view.project_empty"),
    };
    let (br, _) = ui.allocate_exact_size(Vec2::new(w, s.d(band::BAND)), Sense::hover());
    with_brush(ui.painter(), |b| {
        band::section(b, br, &s, State::REST, Src::Key(title), None, false)
    });
    put(app, format!("{title}"), br);
    let (hr, _) = ui.allocate_exact_size(Vec2::new(w, 20.0 * k), Sense::hover());
    let cols = columns(hr, k);
    const TITLES: [&str; 11] = [
        "",
        "ctl.col.control",
        "ctl.col.target",
        "ctl.col.mode",
        "ctl.col.from",
        "ctl.col.to",
        "ctl.col.curve",
        "ctl.col.takeover",
        "ctl.col.feedback",
        "",
        "",
    ];
    for (r, key) in cols.iter().zip(TITLES) {
        if !key.is_empty() {
            cell_text(ui, *r, &s, &t(key), s.t.text_dim, Role::Micro);
        }
    }
    let filter = app.controllers.view.filter.trim().to_lowercase();
    let all = app.controller_map(scope).mappings.clone();
    let rows: Vec<Mapping> = all
        .into_iter()
        .filter(|m| {
            filter.is_empty()
                || crate::controllers::source_text(&m.source)
                    .to_lowercase()
                    .contains(&filter)
                || app
                    .mapping_target_name(&m.target)
                    .to_lowercase()
                    .contains(&filter)
        })
        .collect();
    if rows.is_empty() {
        let (nr, _) = ui.allocate_exact_size(Vec2::new(w, 26.0 * k), Sense::hover());
        let words = if filter.is_empty() {
            t(empty)
        } else {
            t("ctl.view.no_match")
        };
        cell_text(ui, nr.shrink2(Vec2::new(10.0 * k, 0.0)), &s, &words, s.t.text_dim, Role::Body);
        return;
    }
    for (i, m) in rows.iter().enumerate() {
        row(app, ui, scope, m, i, w);
    }
}

/// One mapping's row.
fn row(app: &mut CantusApp, ui: &mut Ui, scope: Scope, m: &Mapping, i: usize, w: f32) {
    let s = Style::current();
    let k = s.k;
    let (rr, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H * k), Sense::hover());
    if !ui.is_rect_visible(rr) {
        return;
    }
    let sc = match scope {
        Scope::Global => 0u8,
        Scope::Project => 1u8,
    };
    let tag = |what: &str| format!("{}#{}#{what}", sc, m.id.0);
    let cid = |what: &str| Id::new(("ctl-row", sc, m.id.0, what.to_string()));
    let c = columns(rr, k);
    let key = (scope, m.id);
    let alive = scope == Scope::Global || m.target.exists(app.project());
    let lit = app
        .controllers
        .activity
        .get(&key)
        .is_some_and(|t| t.elapsed() < LIT);
    let armed = app
        .controllers
        .learning
        .as_ref()
        .is_some_and(|l| l.scope == scope && l.replace == Some(m.id));
    with_brush(ui.painter(), |b| {
        if i % 2 == 1 {
            b.rect(rr, Fill::Solid(s.t.panel.gamma_multiply(0.45)));
        }
        if armed {
            b.rect(rr, Fill::Solid(s.t.led.gamma_multiply(0.14)));
        }
        let dot = if lit { s.t.led } else { s.t.edge };
        b.circle(c[0].center(), 3.6 * k, Fill::Solid(dot));
    });
    put(app, tag("row"), rr);
    put(app, tag("dot"), c[0]);
    // The control, and *Shared* when another mapping takes it too.
    let others: Vec<String> = [Scope::Project, Scope::Global]
        .into_iter()
        .flat_map(|sc| {
            app.controller_map(sc)
                .sharing(m)
                .map(|o| o.target.clone())
                .collect::<Vec<_>>()
        })
        .map(|t| app.mapping_target_name(&t))
        .collect();
    let control = format!(
        "{} \u{b7} {} \u{b7} {}",
        control_text(m.source.control),
        channel_text(m.source.channel),
        port_text(&m.source.port)
    );
    let mut cr = c[1];
    if !others.is_empty() {
        let pill_w = 52.0 * k;
        let pr = Rect::from_min_max(
            egui::pos2(cr.right() - pill_w, cr.center().y - 8.0 * k),
            egui::pos2(cr.right(), cr.center().y + 8.0 * k),
        );
        cr.max.x = pr.left() - 4.0 * k;
        pill(ui, pr, &t("ctl.view.shared"), s.t.highlight);
        put(app, tag("shared"), pr);
        let resp = ui.interact(pr, cid("shared"), Sense::hover());
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                &t("ctl.view.shared_tip"),
                &others.join(", "),
                Caps::NONE,
            );
        }
    }
    cell_text(ui, cr, &s, &control, s.t.text, Role::Body);
    let hover = ui.interact(cr, cid("control"), Sense::hover());
    if hover.hovered() {
        hint::set_text(ui.ctx(), &t("ctl.col.control"), &control, Caps::NONE);
    }
    // The target; a dormant one dimmed and marked.
    let name = app.mapping_target_name(&m.target);
    let mut tr = c[2];
    if !alive {
        let pill_w = 64.0 * k;
        let pr = Rect::from_min_max(
            egui::pos2(tr.right() - pill_w, tr.center().y - 8.0 * k),
            egui::pos2(tr.right(), tr.center().y + 8.0 * k),
        );
        tr.max.x = pr.left() - 4.0 * k;
        pill(ui, pr, &t("ctl.view.missing"), s.t.danger);
        put(app, tag("missing"), pr);
    }
    cell_text(
        ui,
        tr,
        &s,
        &name,
        if alive { s.t.text } else { s.t.text_dim },
        Role::Body,
    );
    let hover = ui.interact(tr, cid("target"), Sense::hover());
    if hover.hovered() {
        hint::set_text(ui.ctx(), &t("ctl.col.target"), &name, Caps::NONE);
    }
    // Mode.
    let mr = c[3].shrink2(Vec2::new(0.0, 3.0 * k));
    let words = mode_text(m.mode);
    let (field, _) = MenuField::new(&words).id(cid("mode")).show_at(ui, mr);
    put(app, tag("mode"), mr);
    let options: Vec<(MapMode, String)> = modes_for(m)
        .into_iter()
        .map(|md| (md, mode_text(md)))
        .collect();
    if let Some(md) = pick(&field, &options, Some(&m.mode), 170.0 * k) {
        let mut n = m.clone();
        n.mode = md;
        app.set_mapping(scope, n, None);
    }
    // From, to, curve.
    let merge = |what: &str| MergeKey::of(&("ctl-edit", sc, m.id.0, what.to_string()));
    let target = m.target.clone();
    for (col, what, name_key) in [(4usize, "from", "ctl.col.from"), (5, "to", "ctl.col.to")] {
        let pos = if what == "from" { m.min } else { m.max };
        let words = value_words(app, &target, pos);
        let default = if what == "from" { 0.0 } else { 1.0 };
        let out = value_cell(
            app,
            ui,
            c[col],
            cid(what),
            &t(name_key),
            &words,
            pos,
            default,
            &target,
        );
        put(app, tag(what), c[col]);
        if let Some(p) = out.pos {
            let mut n = m.clone();
            if what == "from" {
                n.min = p;
            } else {
                n.max = p;
            }
            app.set_mapping(scope, n, (!out.done).then(|| merge(what)));
        }
        if out.done {
            app.end_gesture();
            if scope == Scope::Global {
                app.store_global_map();
            }
        }
    }
    let curve_words = format!("{:+.0} %", m.curve * 100.0);
    let out = value_cell(
        app,
        ui,
        c[6],
        cid("curve"),
        &t("ctl.col.curve"),
        &curve_words,
        (m.curve + 1.0) * 0.5,
        0.5,
        // A curve is typed in percent, −100…100.
        &MappingTarget::DeviceParam(0),
    );
    put(app, tag("curve"), c[6]);
    if let Some(p) = out.pos {
        let mut n = m.clone();
        n.curve = (p * 2.0 - 1.0).clamp(-1.0, 1.0);
        if (n.curve).abs() < 0.02 {
            n.curve = 0.0;
        }
        app.set_mapping(scope, n, (!out.done).then(|| merge("curve")));
    }
    if out.done {
        app.end_gesture();
        if scope == Scope::Global {
            app.store_global_map();
        }
    }
    // Takeover: an absolute control's only.
    let kr = c[7].shrink2(Vec2::new(0.0, 3.0 * k));
    if m.mode == MapMode::Absolute {
        let words = takeover_text(m.takeover);
        let (field, _) = MenuField::new(&words).id(cid("takeover")).show_at(ui, kr);
        put(app, tag("takeover"), kr);
        let options: Vec<(Takeover, String)> = [Takeover::Jump, Takeover::Pickup, Takeover::Scale]
            .into_iter()
            .map(|t| (t, takeover_text(t)))
            .collect();
        if let Some(tk) = pick(&field, &options, Some(&m.takeover), 120.0 * k) {
            let mut n = m.clone();
            n.takeover = tk;
            app.set_mapping(scope, n, None);
        }
    } else {
        cell_text(ui, kr, &s, "\u{2014}", s.t.text_dim, Role::Body);
    }
    // Feedback.
    let fr = c[8].shrink2(Vec2::new(0.0, 3.0 * k));
    let fb = LedButton::new(m.feedback, Src::Key("ctl.view.send"))
        .name(&t("ctl.view.send_tip"))
        .id(cid("feedback"))
        .show_at(ui, fr);
    put(app, tag("feedback"), fr);
    if fb.clicked() {
        let mut n = m.clone();
        n.feedback = !n.feedback;
        app.set_mapping(scope, n, None);
    }
    // Learn again; remove.
    let lr = c[9].shrink(2.0 * k);
    if glyph_at(ui, lr, cid("relearn"), Glyph::Controller, armed, &t("ctl.view.relearn")).clicked() {
        if armed {
            app.cancel_learning();
        } else {
            relearn(app, scope, m.id);
        }
    }
    put(app, tag("relearn"), lr);
    let xr = c[10].shrink(2.0 * k);
    if glyph_at(ui, xr, cid("remove"), Glyph::Close, false, &t("ctl.view.remove")).clicked() {
        app.remove_mapping(scope, m.id);
    }
    put(app, tag("remove"), xr);
}

/// Arm mapping `id` to learn its control again.
fn relearn(app: &mut CantusApp, scope: Scope, id: MappingId) {
    let Some(m) = app.controller_map(scope).get(id).cloned() else {
        return;
    };
    app.learn_target(m.target.clone());
    if let Some(l) = app.controllers.learning.as_mut() {
        l.scope = scope;
        l.replace = Some(id);
    }
}

/// A small word on a tinted pill.
fn pill(ui: &Ui, r: Rect, words: &str, hue: egui::Color32) {
    let s = Style::current();
    with_brush(ui.painter(), |b| {
        b.rrect(r, r.height() * 0.5, Fill::Solid(hue.gamma_multiply(0.22)));
        b.rrect_stroke(r, r.height() * 0.5, s.s(1.0), hue);
        text::paint(
            b,
            r,
            &Text::new(words, Role::Micro, s.t.text).policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}
