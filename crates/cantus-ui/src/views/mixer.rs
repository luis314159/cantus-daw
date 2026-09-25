//! Mixer: the master on the left, then every strip the project has, numbered — fed or not.
//!
//! A desk has channels before anything is plugged into them, and so does this: a project
//! opens with sixteen strips and `+` adds more. A track feeds a strip (`Track::strip`); a
//! strip feeds the master or another strip (`MixerStrip::output`), which is what a bus is;
//! and a strip can *send* a post-fader copy into other strips as well (`MixerStrip::sends`),
//! which is what a return is. Faders, pans, EQ, sends and inserts here are the strip's own,
//! and the same commands the inspector issues, so the two views always agree.
//!
//! What a strip *shows* comes from what feeds it: the header takes the colour and, when
//! the strip has no name of its own, the name of the first track routed into it, so the
//! desk reads left to right as the song's instruments without anybody labelling it.
//!
//! Selection is FL's: click selects a strip, `Ctrl`+click adds it to a multi-selection,
//! and a fader dragged inside a multi-selection moves every selected fader by the same
//! amount. The strip's menu (right-click on its header) is where the selection is acted
//! on: route the selected strips here, send them here, make a submix of them.

use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::keymap::Action;
use crate::settings::MixerDensity;
use crate::theme;
use crate::widgets::{bar, fader, Bar, Fader};
use cantus_core::{
    linear_to_db, Command, Dock, EqField, GroupId, MergeKey, MixerStrip, PanLaw, Send, StripId,
    EQ_MAX_GAIN_DB,
};
use cantus_engine::CaptureTarget;
use egui::{pos2, vec2, Align2, Rect, RichText, Sense, Stroke, StrokeKind};
use std::cell::Cell;
use std::collections::VecDeque;
use std::time::Instant;

thread_local! {
    /// The strip width for the frame being drawn, from the density setting. Set once at
    /// the top of [`show`]; every function that lays out a strip reads it through
    /// [`strip_w`], so the density is one decision rather than a parameter in twenty
    /// signatures.
    static STRIP_WIDTH: Cell<f32> = const { Cell::new(space::GRID * 24.0) };
    static COMPACT: Cell<bool> = const { Cell::new(false) };
    /// The fader's travel for this frame: as long as the panel allows, as on a console,
    /// where the fader is the thing the strip is built around.
    static FADER_HEIGHT: Cell<f32> = const { Cell::new(space::GRID * 36.0) };
}

fn fader_h() -> f32 {
    FADER_HEIGHT.with(Cell::get)
}

fn strip_w() -> f32 {
    STRIP_WIDTH.with(Cell::get)
}

fn compact() -> bool {
    COMPACT.with(Cell::get)
}

/// A strip's width at each density, on the 4-px grid.
pub fn density_width(d: MixerDensity) -> f32 {
    match d {
        MixerDensity::Compact => space::GRID * 18.0,
        MixerDensity::Normal => space::GRID * 24.0,
        MixerDensity::Wide => space::GRID * 32.0,
    }
}

/// Strip geometry, all on the 4-px grid (`design::space`).
const HEADER_H: f32 = space::ROW;
/// A group separator's width: enough for a vertical name and a chevron.
const SEP_W: f32 = space::GRID * 6.0;
/// How many frames of level the meter-wave view keeps per strip.
const WAVE_LEN: usize = 96;
const WAVE_H: f32 = space::GRID * 8.0;
/// The shortest a fader gets; it grows with the panel (see [`fader_h`]).
const FADER_MIN_H: f32 = space::GRID * 36.0;
/// The tallest: past this a fader's travel is longer than a hand wants.
const FADER_MAX_H: f32 = space::GRID * 110.0;
/// What a strip needs besides its fader, at the normal density: header, pan, width and
/// switches, EQ bars, the readouts, the button row, the route and the sends header.
const STRIP_CHROME_H: f32 = space::GRID * 96.0;
const METER_W: f32 = space::S4;
const HOLD_SECONDS: f32 = 1.2;
/// How fast a meter falls, in dB per second. Rise is instant, as on a digital peak meter;
/// the fall is the ballistics — 24 dB/s reads as motion rather than as flicker, and lets
/// a transient be *seen*, which a block-by-block reading does not.
const FALL_DB_PER_S: f32 = 24.0;

#[derive(Debug, Default)]
pub struct MixerView {
    /// Peak-hold per strip (strip index; the master is stored past the last strip).
    holds: Vec<(f32, Instant)>,
    /// The strip being renamed in place, and the text so far.
    rename: Option<(StripId, String)>,
    /// Recent peaks per strip, for the meter-wave view (index as `holds`).
    waves: Vec<VecDeque<f32>>,
    /// What each meter shows after ballistics, in dB, and when it was last updated.
    shown: Vec<(f32, Instant)>,
}

impl MixerView {
    fn hold(&mut self, index: usize, peak: f32) -> f32 {
        while self.holds.len() <= index {
            self.holds.push((0.0, Instant::now()));
        }
        let (held, at) = &mut self.holds[index];
        if peak >= *held || at.elapsed().as_secs_f32() > HOLD_SECONDS {
            *held = peak;
            *at = Instant::now();
        }
        *held
    }

    /// Start renaming a strip in place (the menu's *Rename…* and a double-click).
    pub fn begin_rename(&mut self, strip: &MixerStrip) {
        self.rename = Some((strip.id, strip.name.clone()));
    }

    /// The level a meter shows: the engine's peak when it is higher, else the last shown
    /// level fallen at [`FALL_DB_PER_S`] since. Linear in, linear out.
    fn ballistic(&mut self, index: usize, peak: f32) -> f32 {
        self.ballistic_at(index, peak, Instant::now())
    }

    fn ballistic_at(&mut self, index: usize, peak: f32, now: Instant) -> f32 {
        while self.shown.len() <= index {
            self.shown.push((-120.0, now));
        }
        let (db, at) = &mut self.shown[index];
        let fallen = *db - FALL_DB_PER_S * now.duration_since(*at).as_secs_f32();
        // Silence is its own value, not `linear_to_db`'s −96 dB floor, or a meter would
        // never finish falling.
        let incoming = if peak > 0.0 {
            20.0 * peak.log10()
        } else {
            -120.0
        };
        *db = incoming.max(fallen).max(-120.0);
        *at = now;
        if *db <= -100.0 {
            0.0
        } else {
            10f32.powf(*db / 20.0)
        }
    }

    /// Whether any meter is still falling, so the view keeps drawing until it has landed
    /// even when the transport is stopped.
    fn falling(&self) -> bool {
        self.shown.iter().any(|(db, _)| *db > -60.0)
    }

    /// Record this frame's peak for the meter-wave view.
    fn push_wave(&mut self, index: usize, peak: f32) -> &VecDeque<f32> {
        while self.waves.len() <= index {
            self.waves.push(VecDeque::with_capacity(WAVE_LEN));
        }
        let w = &mut self.waves[index];
        if w.len() == WAVE_LEN {
            w.pop_front();
        }
        w.push_back(peak);
        w
    }
}

/// What the desk draws, left to right, in one dock: strips and the separators of the
/// groups they sit in. A collapsed group is its separator alone.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Separator(GroupId),
    Strip(StripId, usize),
}

/// The items of one dock, in desk order. A group's separator goes before each run of its
/// members; members of a collapsed group are left out.
fn dock_items(app: &CantusApp, dock: Dock) -> Vec<Item> {
    let p = app.project();
    let mut items = Vec::new();
    let mut run: Option<GroupId> = None;
    for (i, s) in p.strips.iter().enumerate().filter(|(_, s)| s.dock == dock) {
        let group = s.group.filter(|g| p.strip_group(*g).is_some());
        if group != run {
            if let Some(g) = group {
                items.push(Item::Separator(g));
            }
            run = group;
        }
        let collapsed = group
            .and_then(|g| p.strip_group(g))
            .is_some_and(|g| g.collapsed);
        if !collapsed {
            items.push(Item::Strip(s.id, i));
        }
    }
    items
}

/// The width a run of items takes, frames and spacing included — what a pinned dock
/// reserves so the scrolling middle can have the rest.
fn items_width(items: &[Item]) -> f32 {
    let strip_outer = strip_w() + 2.0 * space::S2 + 2.0;
    items
        .iter()
        .map(|i| match i {
            Item::Separator(_) => SEP_W,
            Item::Strip(..) => strip_outer,
        } + space::S2)
        .sum()
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    if app.mixer.falling() {
        ui.ctx().request_repaint();
    }
    let density = app.settings.mixer_density;
    STRIP_WIDTH.with(|w| w.set(density_width(density)));
    COMPACT.with(|c| c.set(density == MixerDensity::Compact));
    toolbar(app, ui);
    let chrome = if compact() {
        STRIP_CHROME_H - space::GRID * 30.0
    } else {
        STRIP_CHROME_H
    };
    let fader = (ui.available_height() - chrome).clamp(FADER_MIN_H, FADER_MAX_H);
    FADER_HEIGHT.with(|h| h.set(fader.round()));
    let left = dock_items(app, Dock::Left);
    let middle = dock_items(app, Dock::Middle);
    let right = dock_items(app, Dock::Right);
    let count = app.project().strips.len();
    let right_w = if right.is_empty() {
        0.0
    } else {
        items_width(&right) + space::S4
    };
    ui.horizontal_top(|ui| {
        // Master and the left dock are pinned: they never scroll away.
        master_strip(app, ui, count);
        ui.add_space(space::S2);
        draw_items(app, ui, &left);
        if !left.is_empty() {
            dock_divider(ui);
        }
        // The middle scrolls, and takes whatever the pinned docks leave.
        let avail = (ui.available_width() - right_w).max(strip_w());
        egui::ScrollArea::horizontal()
            .id_salt("mixer-middle")
            .max_width(avail)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    draw_items(app, ui, &middle);
                    ui.add_space(space::S2);
                    add_strip_button(app, ui);
                });
            });
        if !right.is_empty() {
            dock_divider(ui);
            draw_items(app, ui, &right);
        }
    });
}

fn draw_items(app: &mut CantusApp, ui: &mut egui::Ui, items: &[Item]) {
    for item in items {
        match *item {
            Item::Separator(g) => group_separator(app, ui, g),
            Item::Strip(id, index) => strip(app, ui, id, index),
        }
    }
}

/// The line between a pinned dock and the scrolling middle: two hairlines with a gap, so
/// it reads as an edge and not as another strip's border.
fn dock_divider(ui: &mut egui::Ui) {
    let pal = theme::p();
    let h = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(vec2(space::S2, h), Sense::hover());
    let p = ui.painter();
    for x in [rect.left() + 1.0, rect.right() - 1.0] {
        p.vline(x, rect.y_range(), Stroke::new(1.0, pal.grid_strong));
    }
}

/// A group's separator: a column in the group's colour with its name running up it, a
/// chevron that folds the group, and the member count. Click folds; right-click is the
/// group's menu (rename, colour, dissolve).
fn group_separator(app: &mut CantusApp, ui: &mut egui::Ui, id: GroupId) {
    let Some(group) = app.project().strip_group(id).cloned() else {
        return;
    };
    let members = app.project().strips_in_group(id).count();
    let pal = theme::p();
    let color = theme::track_color(group.color);
    let h = ui.available_height().max(HEADER_H * 4.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(SEP_W, h), Sense::click());
    let fill = theme::tint(pal.panel, color, if resp.hovered() { 0.45 } else { 0.3 });
    let p = ui.painter();
    p.rect_filled(rect, pal.corner_radius, fill);
    p.rect_filled(
        Rect::from_min_max(rect.min, pos2(rect.left() + 3.0, rect.bottom())),
        egui::CornerRadius {
            nw: pal.corner_radius,
            sw: pal.corner_radius,
            ne: 0,
            se: 0,
        },
        color,
    );
    let ink = theme::on_color(&pal, fill);
    p.text(
        pos2(rect.center().x, rect.top() + space::S3),
        Align2::CENTER_CENTER,
        if group.collapsed {
            "\u{25b8}"
        } else {
            "\u{25be}"
        },
        design::paint::label(),
        ink,
    );
    p.text(
        pos2(rect.center().x, rect.top() + space::S8),
        Align2::CENTER_CENTER,
        members.to_string(),
        design::paint::ruler(),
        ink.gamma_multiply(0.8),
    );
    // The name runs up the column, as a console's scribble strip does on its side.
    let galley = ui
        .painter()
        .layout_no_wrap(group.name.clone(), design::paint::label(), ink);
    let at = pos2(
        rect.center().x - galley.size().y * 0.5,
        rect.top() + space::S8 * 2.0 + galley.size().x,
    );
    ui.painter().add(
        egui::epaint::TextShape::new(at, galley, ink).with_angle(-std::f32::consts::FRAC_PI_2),
    );
    let resp = resp.on_hover_text(tf(
        "mixer.group_tip",
        &[("name", &group.name), ("n", &members)],
    ));
    if resp.clicked() {
        app.exec(Command::SetStripGroupCollapsed {
            id,
            collapsed: !group.collapsed,
        });
    }
    resp.context_menu(|ui| {
        let mut name = group.name.clone();
        ui.label(
            RichText::new(t("mixer.group.name"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        let edit = ui.text_edit_singleline(&mut name);
        if edit.changed() && !name.trim().is_empty() {
            app.exec_merging(
                Command::SetStripGroupName {
                    id,
                    name: name.clone(),
                },
                MergeKey::of(&("gname", id.0)),
            );
        }
        if edit.lost_focus() {
            app.end_gesture();
        }
        ui.horizontal(|ui| {
            for c in cantus_core::Color::PALETTE {
                let (r, swatch) =
                    ui.allocate_exact_size(vec2(space::S4, space::S4), Sense::click());
                ui.painter()
                    .rect_filled(r, pal.corner_radius, theme::track_color(c));
                if swatch.hovered() || group.color == c {
                    ui.painter().rect_stroke(
                        r,
                        pal.corner_radius,
                        Stroke::new(1.0, pal.text),
                        StrokeKind::Inside,
                    );
                }
                if swatch.clicked() {
                    app.exec(Command::SetStripGroupColor { id, color: c });
                }
            }
        });
        ui.separator();
        if ui.button(t("mixer.group.select")).clicked() {
            let members: Vec<StripId> = app.project().strips_in_group(id).map(|s| s.id).collect();
            if let Some(first) = members.first().copied() {
                app.select_strip(first);
                app.selected_strips = members;
            }
            ui.close();
        }
        if ui.button(t("mixer.group.dissolve")).clicked() {
            app.dissolve_strip_group(id);
            ui.close();
        }
    });
}

/// Title, the count, and the two desk-wide switches that only appear when they have
/// something to undo: every mute off, every solo off.
/// The record button on a strip or the master. Red and filled while it records: the one
/// colour on the desk that means "this is being written to disk".
fn record_button(app: &CantusApp, ui: &mut egui::Ui, target: CaptureTarget) -> egui::Response {
    let pal = theme::p();
    let on = app.capturing(target);
    let (rect, resp) = ui.allocate_exact_size(vec2(space::S4, space::S4), Sense::click());
    let c = rect.center();
    let r = rect.width() * 0.32;
    if on {
        // A slow pulse: steady enough to read, alive enough to notice.
        let t = ui.input(|i| i.time) as f32;
        let a = 0.75 + 0.25 * (t * 3.0).sin();
        ui.painter()
            .circle_filled(c, r, pal.danger.gamma_multiply(a));
        ui.ctx().request_repaint();
    } else {
        ui.painter().circle_stroke(
            c,
            r,
            Stroke::new(
                1.5,
                if resp.hovered() {
                    pal.danger
                } else {
                    pal.text_dim
                },
            ),
        );
    }
    resp.on_hover_text(if on {
        t("take.stop_tip")
    } else {
        tf("take.record_tip", &[("what", &app.capture_label(target))])
    })
}

/// While a take runs: what, and for how long. Otherwise the takes made so far, as a menu.
fn takes_status(app: &mut CantusApp, ui: &mut egui::Ui) {
    let pal = theme::p();
    if let Some((label, secs)) = app.mix_recorder.status() {
        let text = format!(
            "\u{25cf} {}  {}:{:04.1}",
            label,
            (secs / 60.0).floor() as u64,
            secs % 60.0
        );
        if ui
            .button(
                RichText::new(text)
                    .text_style(design::mono_small())
                    .color(pal.danger),
            )
            .on_hover_text(t("take.stop_tip"))
            .clicked()
        {
            app.stop_capture();
        }
        return;
    }
    let n = app.mix_recorder.takes.len();
    if n == 0 {
        return;
    }
    ui.menu_button(tf("take.list", &[("n", &n)]), |ui| {
        let takes = app.mix_recorder.takes.clone();
        for take in takes.iter().rev() {
            let text = format!(
                "{}  \u{b7}  {:.1} s  \u{b7}  {:.1} dB",
                take.label,
                take.seconds,
                linear_to_db(take.peak)
            );
            if ui
                .button(text)
                .on_hover_text(take.path.display().to_string())
                .clicked()
            {
                reveal(&take.path);
                ui.close();
            }
        }
    });
}

/// Show a file in the system's file browser.
fn reveal(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(dir) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("mixer.title")).heading().color(pal.gold));
        takes_status(app, ui);
        ui.label(
            RichText::new(tf("mixer.strips", &[("n", &app.project().strips.len())]))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        let selected = app.strip_selection().len();
        if selected > 1 {
            ui.label(
                RichText::new(tf("mixer.selected", &[("n", &selected)]))
                    .text_style(design::caption())
                    .color(pal.gold),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Density: how much of each strip is shown. Compact drops the width bar,
            // the EQ bars and the send rows; wide gives every bar more travel.
            for d in [
                MixerDensity::Wide,
                MixerDensity::Normal,
                MixerDensity::Compact,
            ] {
                let on = app.settings.mixer_density == d;
                if ui
                    .selectable_label(
                        on,
                        RichText::new(t(d.label_key())).text_style(design::caption()),
                    )
                    .clicked()
                    && !on
                {
                    app.settings.mixer_density = d;
                    app.store_settings();
                }
            }
            let wave = app.settings.mixer_meter_wave;
            if ui
                .selectable_label(
                    wave,
                    RichText::new(t("mixer.wave")).text_style(design::caption()),
                )
                .on_hover_text(format!(
                    "{} ({})",
                    t("mixer.wave_tip"),
                    app.keymap.display_for(Action::ToggleMeterWave)
                ))
                .clicked()
            {
                app.run_action(Action::ToggleMeterWave);
            }
            ui.separator();
            if app.project().any_strip_solo() && ui.small_button(t("mixer.solo_off")).clicked() {
                app.clear_strip_solos();
            }
            if app.project().strips.iter().any(|s| s.muted)
                && ui.small_button(t("mixer.unmute_all")).clicked()
            {
                app.unmute_all_strips();
            }
        });
    });
}

fn meter(ui: &mut egui::Ui, rect: Rect, peak: f32, hold: f32) {
    let p = ui.painter();
    // A meter is a well cut into the strip, like the fader's groove beside it — the same
    // colour rule, so the two read as one control rather than two unrelated boxes.
    p.rect_filled(rect, 2.0, theme::recess(&theme::p()));
    let frac = |v: f32| ((linear_to_db(v) + 60.0) / 60.0).clamp(0.0, 1.0);
    let h = rect.height() * frac(peak);
    let bar = Rect::from_min_max(
        pos2(rect.left() + 1.0, rect.bottom() - h),
        pos2(rect.right() - 1.0, rect.bottom()),
    );
    p.rect_filled(bar, 1.0, theme::meter_color(peak));
    let hy = rect.bottom() - rect.height() * frac(hold);
    p.hline(
        rect.x_range(),
        hy,
        Stroke::new(1.5, theme::meter_color(hold)),
    );
    for db in [0.0f32, -12.0, -24.0, -48.0] {
        let y = rect.bottom() - rect.height() * ((db + 60.0) / 60.0);
        p.hline(
            rect.left()..=rect.left() + 3.0,
            y,
            Stroke::new(1.0, theme::p().grid_bar),
        );
    }
    p.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, theme::p().grid_strong),
        StrokeKind::Inside,
    );
}

/// The held peak as text: `-\u{221e}` for silence, one decimal otherwise, and in the danger
/// colour once it has touched full scale. FL prints this under every meter; it is how a
/// strip that clipped once is found without watching all of them.
fn peak_readout(ui: &mut egui::Ui, hold: f32) {
    let pal = theme::p();
    let text = if hold <= 0.0 {
        "-\u{221e}".to_string()
    } else {
        format!("{:.1}", linear_to_db(hold))
    };
    ui.label(
        RichText::new(text)
            .text_style(design::mono_small())
            .color(if hold >= 1.0 {
                pal.danger
            } else {
                pal.text_dim
            }),
    )
    .on_hover_text(t("mixer.peak"));
}

/// What a strip is called on the desk: its own name, else the first track feeding it,
/// else nothing — the number is always shown beside it.
fn strip_label(app: &CantusApp, strip: &MixerStrip) -> Option<String> {
    if !strip.name.is_empty() {
        return Some(strip.name.clone());
    }
    app.project()
        .tracks_on_strip(strip.id)
        .next()
        .map(|t| t.name.clone())
}

/// The colour a strip wears: its own, else the first feeding track's, else its group's.
fn strip_color(app: &CantusApp, strip: &MixerStrip) -> Option<egui::Color32> {
    strip
        .color
        .or_else(|| {
            app.project()
                .tracks_on_strip(strip.id)
                .next()
                .map(|t| t.color)
        })
        .or_else(|| {
            strip
                .group
                .and_then(|g| app.project().strip_group(g))
                .map(|g| g.color)
        })
        .map(theme::track_color)
}

/// The meter-wave view: the strip's recent level as a mirrored envelope scrolling left,
/// FL's replacement for the peak meters when you want to *see* a transient.
fn wave_view(ui: &mut egui::Ui, history: &VecDeque<f32>, color: egui::Color32) {
    let pal = theme::p();
    let (rect, _) = ui.allocate_exact_size(vec2(strip_w() - space::S4, WAVE_H), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, theme::recess(&pal));
    if history.len() > 1 {
        let dx = rect.width() / (WAVE_LEN - 1) as f32;
        let x0 = rect.right() - dx * (history.len() - 1) as f32;
        let level = |v: f32| ((linear_to_db(v) + 60.0) / 60.0).clamp(0.0, 1.0);
        let mut mesh = egui::Mesh::default();
        for (i, v) in history.iter().enumerate() {
            let x = x0 + dx * i as f32;
            let h = level(*v) * rect.height() * 0.5;
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(pos2(x, rect.center().y - h), color);
            mesh.colored_vertex(pos2(x, rect.center().y + h), color);
            if i > 0 {
                mesh.add_triangle(base - 2, base - 1, base);
                mesh.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(mesh);
    }
    p.hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, pal.grid_strong),
    );
}

/// The header block every strip and the master share: a numbered, tinted plate with the
/// name on it. FL colours the whole plate at full strength; this is a tint, so the accent
/// stays free to mean *state*.
fn header_plate(
    ui: &mut egui::Ui,
    number: &str,
    name: &str,
    color: Option<egui::Color32>,
    selected: bool,
) -> egui::Response {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(vec2(strip_w(), HEADER_H), Sense::click());
    let base = if selected {
        pal.panel_raised
    } else {
        pal.panel
    };
    let fill = match color {
        Some(c) => theme::tint(base, c, if selected { 0.6 } else { 0.42 }),
        None => theme::tint(base, pal.grid_strong, 0.3),
    };
    let p = ui.painter();
    p.rect_filled(rect, pal.corner_radius, fill);
    if let Some(c) = color {
        p.rect_filled(
            Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 3.0)),
            egui::CornerRadius {
                nw: pal.corner_radius,
                ne: pal.corner_radius,
                sw: 0,
                se: 0,
            },
            c,
        );
    }
    let ink = theme::on_color(&pal, fill);
    // Number left, in the monospace face so the desk's numbers line up strip to strip.
    p.text(
        pos2(rect.left() + 6.0, rect.center().y + 1.0),
        Align2::LEFT_CENTER,
        number,
        design::paint::ruler(),
        ink.gamma_multiply(0.8),
    );
    let name_rect = Rect::from_min_max(pos2(rect.left() + 26.0, rect.top()), rect.max);
    p.with_clip_rect(name_rect).text(
        pos2(name_rect.left(), rect.center().y + 1.0),
        Align2::LEFT_CENTER,
        name,
        design::paint::label(),
        ink,
    );
    if resp.hovered() || selected {
        p.rect_stroke(
            rect,
            pal.corner_radius,
            Stroke::new(1.0, if selected { pal.gold } else { pal.text_dim }),
            StrokeKind::Inside,
        );
    }
    resp
}

/// A small state switch in a strip's button row: `M`, `S`, `Ø`, `⇄`, `FX`. `on` is the
/// *unusual* state — muted, soloed, inverted, swapped, FX off — and `tone` is the colour
/// it takes then, so the desk at rest is quiet and anything switched stands out.
fn switch(
    ui: &mut egui::Ui,
    on: bool,
    label: &str,
    tone: egui::Color32,
    tip: &str,
) -> egui::Response {
    ui.selectable_label(
        on,
        RichText::new(label)
            .text_style(design::caption())
            .color(if on { tone } else { theme::p().text_dim }),
    )
    .on_hover_text(tip)
}

fn strip(app: &mut CantusApp, ui: &mut egui::Ui, id: StripId, index: usize) {
    let Some(strip) = app.project().strip(id).cloned() else {
        return;
    };
    let selected = app.selected_strip == Some(id);
    let in_selection = app.strip_selection().contains(&id);
    let color = strip_color(app, &strip);
    let label = strip_label(app, &strip);
    let fed_by: Vec<String> = app
        .project()
        .tracks_on_strip(id)
        .map(|t| t.name.clone())
        .collect();
    let peak = app
        .engine
        .meters()
        .strips
        .get(index)
        .copied()
        .unwrap_or(0.0);
    let hold = app.mixer.hold(index, peak);
    let peak = app.mixer.ballistic(index, peak);
    let pal = theme::p();

    let frame = egui::Frame::NONE
        .fill(if selected {
            pal.panel_raised
        } else {
            pal.panel
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                pal.gold
            } else if in_selection {
                pal.gold.gamma_multiply(0.55)
            } else {
                pal.grid
            },
        ))
        .inner_margin(egui::Margin::same(space::S2 as i8))
        .corner_radius(pal.corner_radius);
    let wave = app
        .settings
        .mixer_meter_wave
        .then(|| app.mixer.push_wave(index, peak).clone());
    frame.show(ui, |ui| {
        ui.set_width(strip_w());
        ui.vertical_centered(|ui| {
            // Header: number and name. Click selects the strip (and the first track on
            // it, so the inspector follows); Ctrl+click extends the selection;
            // double-click renames; right-click opens the strip's menu.
            let renaming = app.mixer.rename.as_ref().is_some_and(|(rid, _)| *rid == id);
            if renaming {
                let mut done = None;
                if let Some((_, text)) = app.mixer.rename.as_mut() {
                    let edit = ui.add(
                        egui::TextEdit::singleline(text)
                            .desired_width(strip_w() - space::S2)
                            .font(design::strong()),
                    );
                    edit.request_focus();
                    if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        done = Some(text.clone());
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        done = Some(strip.name.clone());
                    }
                }
                if let Some(name) = done {
                    app.mixer.rename = None;
                    if name != strip.name {
                        app.exec(Command::SetStripName { id, name });
                    }
                }
            } else {
                let resp = header_plate(
                    ui,
                    &(index + 1).to_string(),
                    label.as_deref().unwrap_or(""),
                    color,
                    selected || in_selection,
                );
                if resp.clicked() {
                    if ui.input(|i| i.modifiers.command) {
                        app.toggle_strip_selected(id);
                    } else {
                        app.select_strip(id);
                    }
                }
                if resp.double_clicked() {
                    app.mixer.begin_rename(&strip);
                }
                strip_menu(app, &resp, &strip, index);
                resp.on_hover_text(if fed_by.is_empty() {
                    t("mixer.fed_by_none")
                } else {
                    tf("mixer.fed_by", &[("tracks", &fed_by.join(", "))])
                });
            }
            // What feeds this strip, and how many inserts it carries — the two facts
            // that make an unlabelled strip legible.
            ui.label(
                RichText::new(match (fed_by.len(), strip.effects.len()) {
                    (0, 0) => "·".to_string(),
                    (n, 0) => tf("mixer.inputs", &[("n", &n)]),
                    (0, e) => tf("mixer.effects", &[("n", &e)]),
                    (n, e) => format!(
                        "{} · {}",
                        tf("mixer.inputs", &[("n", &n)]),
                        tf("mixer.effects", &[("n", &e)])
                    ),
                })
                .text_style(design::caption())
                .color(pal.text_dim),
            );
            if let Some(history) = &wave {
                wave_view(ui, history, color.unwrap_or(pal.gold));
            }
            // Pan, width and the input-stage switches: what a channel strip does to the
            // stereo field, in the order the signal meets them.
            let lane = strip_w() - space::S4;
            let mut pan = strip.pan;
            let pr = ui.add(Bar::pan(&mut pan).size(lane).name(t("track.pan")));
            if pr.changed() {
                app.exec_merging(
                    Command::SetStripPan { id, pan },
                    MergeKey::of(&("spans", id.0)),
                );
            }
            if pr.drag_stopped() {
                app.end_gesture();
            }
            ui.label(
                RichText::new(bar::pan_text(pan))
                    .text_style(design::mono_small())
                    .color(pal.text_dim),
            );
            let mut width = strip.width;
            if !compact() {
                let wr = ui
                    .add(Bar::width(&mut width).size(lane).name(t("track.width")))
                    .on_hover_text(format!("{} {}", t("track.width"), bar::width_text(width)));
                if wr.changed() {
                    app.exec_merging(
                        Command::SetStripWidth { id, width },
                        MergeKey::of(&("swidth", id.0)),
                    );
                }
                if wr.drag_stopped() {
                    app.end_gesture();
                }
            }
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Mono is the width control's end stop, so the button is a shortcut
                    // to it rather than a second, disagreeing switch.
                    let is_mono = strip.width <= 0.005;
                    if switch(
                        ui,
                        is_mono,
                        &t("track.mono"),
                        pal.gold_bright,
                        &t("track.mono_tip"),
                    )
                    .clicked()
                    {
                        app.exec(Command::SetStripWidth {
                            id,
                            width: if is_mono { 1.0 } else { 0.0 },
                        });
                    }
                    if switch(
                        ui,
                        strip.swap_channels,
                        &t("mixer.swap"),
                        pal.gold_bright,
                        &t("mixer.swap_tip"),
                    )
                    .clicked()
                    {
                        app.exec(Command::SetStripSwapChannels {
                            id,
                            swap: !strip.swap_channels,
                        });
                    }
                    if phase_button(ui, strip.phase_invert).clicked() {
                        app.exec(Command::SetStripPhaseInvert {
                            id,
                            invert: !strip.phase_invert,
                        });
                    }
                });
            });
            if !compact() {
                eq_bars(app, ui, &strip, lane);
            }
            ui.add_space(space::S1);
            // Fader + meter side by side.
            ui.horizontal(|ui| {
                let mut gain = strip.gain_db;
                // The mixer's scroll area scrolls horizontally, so the wheel is free for
                // the faders (`Fader::wheel`).
                let g = ui.add(
                    Fader::new(&mut gain)
                        .vertical(fader_h())
                        .wheel(true)
                        .name(label.as_deref().unwrap_or("")),
                );
                if g.changed() {
                    app.set_strip_gain(id, gain);
                }
                if g.drag_stopped() {
                    app.end_gesture();
                }
                ui.add_space(space::S2);
                let (rect, _) = ui.allocate_exact_size(vec2(METER_W, fader_h()), Sense::hover());
                meter(ui, rect, peak, hold);
                // A Comprimo on the strip shows its gain reduction in a lane beside it.
                crate::views::comp_editor::strip_gr_meter(app, ui, id, fader_h());
            });
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(fader::read_out(strip.gain_db))
                        .text_style(design::readout_small()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    peak_readout(ui, hold);
                });
            });
            ui.horizontal(|ui| {
                if switch(
                    ui,
                    strip.muted,
                    &t("track.mute"),
                    pal.danger,
                    &t("mixer.mute_strip"),
                )
                .clicked()
                {
                    app.exec(Command::SetStripMute {
                        id,
                        muted: !strip.muted,
                    });
                }
                if switch(
                    ui,
                    strip.soloed,
                    &t("track.solo"),
                    pal.gold_bright,
                    &t("mixer.solo_strip"),
                )
                .clicked()
                {
                    app.exec(Command::SetStripSolo {
                        id,
                        soloed: !strip.soloed,
                    });
                }
                if record_button(app, ui, CaptureTarget::Strip(id)).clicked() {
                    app.toggle_capture(CaptureTarget::Strip(id));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // FL's lamp: lit while the chain is *out*, because that is the state
                    // worth noticing on a desk of forty strips.
                    let tip = if strip.fx_enabled {
                        t("mixer.fx_tip")
                    } else {
                        format!("{} · {}", t("mixer.fx_off"), t("mixer.fx_tip"))
                    };
                    if switch(ui, !strip.fx_enabled, &t("mixer.fx"), pal.danger, &tip).clicked() {
                        app.exec(Command::SetStripFxEnabled {
                            id,
                            enabled: !strip.fx_enabled,
                        });
                    }
                });
            });
            output_picker(app, ui, id, strip.output);
            if compact() {
                // The sends as a count; the rows are a density up, and in the inspector.
                if !strip.sends.is_empty() {
                    ui.label(
                        RichText::new(format!("{} {}", t("mixer.sends"), strip.sends.len()))
                            .text_style(design::caption())
                            .color(pal.text_dim),
                    );
                }
            } else {
                sends(app, ui, &strip, lane);
            }
        });
    });
}

/// The strip's EQ as three bipolar bars — low, mid, high — each filling out from 0 dB.
/// The frequencies and Qs are in the inspector; here the three moves a mix engineer makes
/// most are one gesture each.
fn eq_bars(app: &mut CantusApp, ui: &mut egui::Ui, strip: &MixerStrip, lane: f32) {
    let pal = theme::p();
    let labels = ["mixer.eq_low", "mixer.eq_mid", "mixer.eq_high"];
    let names = [
        "inspector.eq_band_low",
        "inspector.eq_band_mid",
        "inspector.eq_band_high",
    ];
    for (i, band) in strip.eq.bands.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t(labels[i]))
                    .text_style(design::mono_small())
                    .color(if band.gain_db != 0.0 {
                        pal.gold_bright
                    } else {
                        pal.text_dim
                    }),
            );
            let mut gain = band.gain_db;
            let r = ui.add(
                Bar::new(&mut gain, -EQ_MAX_GAIN_DB..=EQ_MAX_GAIN_DB, 0.0)
                    .size(lane - space::S4)
                    .name(t(names[i])),
            );
            if r.changed() {
                app.exec_merging(
                    Command::SetStripEq {
                        id: strip.id,
                        band: i as u8,
                        field: EqField::Gain,
                        value: gain,
                    },
                    MergeKey::of(&("seq", strip.id.0, i)),
                );
            }
            if r.drag_stopped() {
                app.end_gesture();
            }
            r.on_hover_text(tf(
                "mixer.eq_tip",
                &[
                    ("band", &t(names[i])),
                    ("gain", &format!("{:+.1}", band.gain_db)),
                    ("hz", &hz_text(band.freq_hz)),
                ],
            ));
        });
    }
}

/// `120 Hz`, `1.5 kHz` — the way a frequency is said, not stored.
pub fn hz_text(hz: f32) -> String {
    if hz >= 1_000.0 {
        format!("{:.1} kHz", hz / 1_000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// The strip's sends: one row per destination — the number (click: on/off, right-click:
/// remove) and a level bar — and `+` to add one. FL puts the switches on every *other*
/// strip and lights them for the selected one; keeping them on the source keeps the
/// question "where does this go?" answerable by looking at one strip.
fn sends(app: &mut CantusApp, ui: &mut egui::Ui, strip: &MixerStrip, lane: f32) {
    let pal = theme::p();
    let id = strip.id;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("mixer.sends").to_uppercase())
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let targets: Vec<(StripId, usize, String)> = app
                .project()
                .strips
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.id != id
                        && strip.send_to(s.id).is_none()
                        && !app.project().routing_would_cycle(id, Some(s.id))
                })
                .map(|(i, s)| (s.id, i + 1, s.name.clone()))
                .collect();
            ui.menu_button(RichText::new("+").text_style(design::caption()), |ui| {
                if targets.is_empty() {
                    ui.label(t("mixer.send_none"));
                }
                for (sid, n, name) in &targets {
                    let text = if name.is_empty() {
                        tf("mixer.send_to", &[("n", n)])
                    } else {
                        format!("{} · {name}", tf("mixer.send_to", &[("n", n)]))
                    };
                    if ui.button(text).clicked() {
                        app.set_strip_send(id, Send::new(*sid));
                        app.end_gesture();
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(t("mixer.send_add"));
        });
    });
    for send in &strip.sends {
        let Some(n) = app.project().strip_number(send.to) else {
            continue;
        };
        let name = app
            .project()
            .strip(send.to)
            .and_then(|s| strip_label(app, s))
            .unwrap_or_default();
        ui.horizontal(|ui| {
            let tip = tf(
                "mixer.send_tip",
                &[
                    ("n", &n),
                    ("name", &name),
                    ("level", &format!("{:.0}%", send.level * 100.0)),
                ],
            );
            let toggle = ui
                .selectable_label(
                    send.enabled,
                    RichText::new(n.to_string()).text_style(design::mono_small()),
                )
                .on_hover_text(&tip);
            if toggle.clicked() {
                app.set_strip_send(
                    id,
                    Send {
                        enabled: !send.enabled,
                        ..*send
                    },
                );
                app.end_gesture();
            }
            toggle.context_menu(|ui| {
                if ui.button(t("mixer.send_remove")).clicked() {
                    app.remove_strip_send(id, send.to);
                    ui.close();
                }
            });
            let mut level = send.level;
            let r = ui.add(
                Bar::new(&mut level, 0.0..=1.0, 0.0)
                    .size(lane - space::S6)
                    .name(tf("mixer.send_to", &[("n", &n)])),
            );
            if r.changed() {
                app.set_strip_send(id, Send { level, ..*send });
            }
            if r.drag_stopped() {
                app.end_gesture();
            }
            r.on_hover_text(tip);
        });
    }
}

/// The strip's menu, on its header. Rename and colour; solo and mute; the selection's
/// routing — FL's *route selected to this track*, *send selected to this track* and
/// *create submix*; move; remove.
fn strip_menu(app: &mut CantusApp, resp: &egui::Response, strip: &MixerStrip, index: usize) {
    let id = strip.id;
    let pal = theme::p();
    resp.context_menu(|ui| {
        if ui.button(t("mixer.menu.rename")).clicked() {
            app.mixer.begin_rename(strip);
            ui.close();
        }
        ui.label(
            RichText::new(t("mixer.menu.color"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        ui.horizontal(|ui| {
            for c in cantus_core::Color::PALETTE {
                let (r, swatch) =
                    ui.allocate_exact_size(vec2(space::S4, space::S4), Sense::click());
                ui.painter()
                    .rect_filled(r, pal.corner_radius, theme::track_color(c));
                if swatch.hovered() || strip.color == Some(c) {
                    ui.painter().rect_stroke(
                        r,
                        pal.corner_radius,
                        Stroke::new(1.0, pal.text),
                        StrokeKind::Inside,
                    );
                }
                if swatch.clicked() {
                    app.exec(Command::SetStripColor { id, color: Some(c) });
                    ui.close();
                }
            }
        });
        if strip.color.is_some() && ui.button(t("mixer.menu.color_track")).clicked() {
            app.exec(Command::SetStripColor { id, color: None });
            ui.close();
        }
        ui.separator();
        if ui
            .selectable_label(strip.soloed, t("track.solo_tip"))
            .clicked()
        {
            app.toggle_strip_solo(id);
            ui.close();
        }
        if ui
            .selectable_label(strip.muted, t("track.mute_tip"))
            .clicked()
        {
            app.toggle_strip_mute(id);
            ui.close();
        }
        ui.separator();
        // The selection, minus this strip: what "selected" means from here.
        let others: Vec<StripId> = app
            .strip_selection()
            .into_iter()
            .filter(|s| *s != id)
            .collect();
        if ui
            .add_enabled(
                !others.is_empty(),
                egui::Button::new(t("mixer.menu.route_here")),
            )
            .clicked()
        {
            app.route_strips_to(&others, Some(id));
            ui.close();
        }
        if ui
            .add_enabled(
                !others.is_empty(),
                egui::Button::new(t("mixer.menu.send_here")),
            )
            .clicked()
        {
            app.send_strips_to(&others, id);
            ui.close();
        }
        let members = app.strip_selection();
        if ui
            .add_enabled(
                members.contains(&id),
                egui::Button::new(t("mixer.menu.submix")),
            )
            .clicked()
        {
            app.create_submix(&members);
            ui.close();
        }
        // FL's *route selected channels to this track*: the channel rack's selection is
        // the selected track.
        let track = app
            .selected_track
            .and_then(|t| app.project().track(t))
            .map(|t| (t.id, t.name.clone(), t.strip));
        if let Some((tid, tname, tstrip)) = track {
            if ui
                .add_enabled(
                    tstrip != Some(id),
                    egui::Button::new(tf("mixer.menu.route_track", &[("track", &tname)])),
                )
                .clicked()
            {
                app.route_track(tid, Some(id));
                ui.close();
            }
        }
        // A Comprimo keyed from another strip, set to duck, in one step.
        crate::views::comp_editor::duck_menu(app, ui, id);
        ui.separator();
        // Dock: where the strip — or the whole selection — sits.
        ui.menu_button(t("mixer.menu.dock"), |ui| {
            for d in Dock::ALL {
                if ui
                    .selectable_label(strip.dock == d, t(d.label_key()))
                    .clicked()
                {
                    let targets = if members.contains(&id) {
                        members.clone()
                    } else {
                        vec![id]
                    };
                    app.set_strips_dock(&targets, d);
                    ui.close();
                }
            }
        });
        ui.menu_button(t("mixer.menu.group"), |ui| {
            let targets = if members.contains(&id) {
                members.clone()
            } else {
                vec![id]
            };
            if ui.button(t("mixer.menu.group_new")).clicked() {
                app.create_strip_group(&targets);
                ui.close();
            }
            let groups: Vec<(GroupId, String)> = app
                .project()
                .strip_groups
                .iter()
                .map(|g| (g.id, g.name.clone()))
                .collect();
            if !groups.is_empty() {
                ui.separator();
            }
            for (gid, name) in groups {
                if ui
                    .selectable_label(strip.group == Some(gid), name)
                    .clicked()
                {
                    app.add_strips_to_group(&targets, gid);
                    ui.close();
                }
            }
            if strip.group.is_some() {
                ui.separator();
                if ui.button(t("mixer.menu.group_leave")).clicked() {
                    app.remove_strips_from_group(&targets);
                    ui.close();
                }
            }
        });
        ui.separator();
        let last = app.project().strips.len().saturating_sub(1);
        let left = format!(
            "{}  {}",
            t("mixer.menu.move_left"),
            app.keymap.display_for(Action::MoveStripLeft)
        );
        if ui.add_enabled(index > 0, egui::Button::new(left)).clicked() {
            app.move_strip(id, -1);
            ui.close();
        }
        let right = format!(
            "{}  {}",
            t("mixer.menu.move_right"),
            app.keymap.display_for(Action::MoveStripRight)
        );
        if ui
            .add_enabled(index < last, egui::Button::new(right))
            .clicked()
        {
            app.move_strip(id, 1);
            ui.close();
        }
        let in_use = app.project().tracks_on_strip(id).next().is_some()
            || app
                .project()
                .strips
                .iter()
                .any(|s| s.feeds().any(|to| to == id));
        if ui
            .add_enabled(!in_use, egui::Button::new(t("mixer.menu.remove")))
            .on_disabled_hover_text(t("mixer.menu.remove_tip"))
            .clicked()
        {
            app.remove_strip(id);
            ui.close();
        }
    });
}

/// Where the strip goes: the master, or another strip. Routes that would loop are not
/// offered, so the command's refusal is never the first the user hears of it.
fn output_picker(app: &mut CantusApp, ui: &mut egui::Ui, id: StripId, current: Option<StripId>) {
    let label = match current.and_then(|o| app.project().strip_number(o)) {
        Some(n) => format!("→ {n}"),
        None => format!("→ {}", t("mixer.master_short")),
    };
    let mut chosen: Option<Option<StripId>> = None;
    egui::ComboBox::from_id_salt(("out", id.0))
        .width(strip_w() - space::S4)
        .selected_text(RichText::new(label).text_style(design::mono_small()))
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), t("mixer.master"))
                .clicked()
            {
                chosen = Some(None);
            }
            let options: Vec<(StripId, usize, String)> = app
                .project()
                .strips
                .iter()
                .enumerate()
                .filter(|(_, s)| s.id != id && !app.project().routing_would_cycle(id, Some(s.id)))
                .map(|(i, s)| (s.id, i + 1, s.name.clone()))
                .collect();
            for (sid, n, name) in options {
                let text = if name.is_empty() {
                    n.to_string()
                } else {
                    format!("{n} · {name}")
                };
                if ui
                    .selectable_label(
                        current == Some(sid),
                        RichText::new(text).text_style(design::mono_small()),
                    )
                    .clicked()
                {
                    chosen = Some(Some(sid));
                }
            }
        })
        .response
        .on_hover_text(t("mixer.output_tip"));
    if let Some(output) = chosen {
        if output != current {
            app.exec(Command::SetStripOutput { id, output });
        }
    }
}

/// The `+` at the end of the desk.
fn add_strip_button(app: &mut CantusApp, ui: &mut egui::Ui) {
    let (rect, resp) = ui.allocate_exact_size(vec2(space::GRID * 8.0, HEADER_H), Sense::click());
    let pal = theme::p();
    ui.painter().rect(
        rect,
        pal.corner_radius,
        if resp.hovered() {
            pal.grid_strong
        } else {
            pal.panel
        },
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "+",
        design::paint::label(),
        pal.text,
    );
    if resp.on_hover_text(t("mixer.add_strip")).clicked() {
        app.add_strip();
    }
}

fn master_strip(app: &mut CantusApp, ui: &mut egui::Ui, index: usize) {
    let raw = app.engine.meters().master;
    let peak = raw[0].max(raw[1]);
    let hold = app.mixer.hold(index, peak);
    // Left and right have ballistics of their own, after every strip's slots.
    let m = [
        app.mixer.ballistic(index + 1, raw[0]),
        app.mixer.ballistic(index + 2, raw[1]),
    ];
    let pal = theme::p();
    let frame = egui::Frame::NONE
        .fill(pal.panel_raised)
        .stroke(Stroke::new(1.0, pal.gold.gamma_multiply(0.6)))
        .inner_margin(egui::Margin::same(space::S2 as i8))
        .corner_radius(pal.corner_radius);
    let wave = app
        .settings
        .mixer_meter_wave
        .then(|| app.mixer.push_wave(index, peak).clone());
    frame.show(ui, |ui| {
        ui.set_width(strip_w());
        ui.vertical_centered(|ui| {
            header_plate(ui, "M", &t("mixer.master"), Some(pal.gold), false);
            if let Some(history) = &wave {
                wave_view(ui, history, pal.gold);
            }
            ui.label(
                RichText::new(t("mixer.stereo"))
                    .text_style(design::caption())
                    .color(pal.text_dim),
            );
            // The master is a channel strip too: a mix can be off-centre, too wide, or
            // out of polarity against a reference. Its pan is a *balance* — the pan law
            // does not apply to it, because the master is already stereo (ADR 0012).
            let lane = strip_w() - space::S4;
            let mut mpan = app.project().master.pan;
            let pr = ui.add(Bar::pan(&mut mpan).size(lane).name(t("mixer.master")));
            if pr.changed() {
                app.exec_merging(
                    Command::SetMasterPan { pan: mpan },
                    MergeKey::of(&"master-pan"),
                );
            }
            if pr.drag_stopped() {
                app.end_gesture();
            }
            ui.label(
                RichText::new(bar::pan_text(mpan))
                    .text_style(design::mono_small())
                    .color(pal.text_dim),
            );
            let mut mwidth = app.project().master.width;
            let wr = ui.add(Bar::width(&mut mwidth).size(lane).name(t("track.width")));
            if wr.changed() {
                app.exec_merging(
                    Command::SetMasterWidth { width: mwidth },
                    MergeKey::of(&"master-width"),
                );
            }
            if wr.drag_stopped() {
                app.end_gesture();
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(bar::width_text(mwidth))
                        .text_style(design::mono_small())
                        .color(pal.text_dim),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let inverted = app.project().master.phase_invert;
                    if phase_button(ui, inverted).clicked() {
                        app.exec(Command::SetMasterPhaseInvert { invert: !inverted });
                    }
                });
            });
            ui.add_space(space::S1);
            ui.horizontal(|ui| {
                let mut gain = app.project().master.gain_db;
                let g = ui.add(
                    Fader::new(&mut gain)
                        .vertical(fader_h())
                        .wheel(true)
                        .name(t("mixer.master")),
                );
                if g.changed() {
                    app.exec_merging(
                        Command::SetMasterGain { gain_db: gain },
                        MergeKey::of(&"master"),
                    );
                }
                if g.drag_stopped() {
                    app.end_gesture();
                }
                ui.add_space(space::S1);
                let (l, _) = ui.allocate_exact_size(vec2(METER_W, fader_h()), Sense::hover());
                meter(ui, l, m[0], hold);
                let (r, _) = ui.allocate_exact_size(vec2(METER_W, fader_h()), Sense::hover());
                meter(ui, r, m[1], hold);
            });
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(fader::read_out(app.project().master.gain_db))
                        .text_style(design::readout_small()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    peak_readout(ui, hold);
                });
            });
            ui.horizontal(|ui| {
                if record_button(app, ui, CaptureTarget::Master).clicked() {
                    app.toggle_capture(CaptureTarget::Master);
                }
                ui.label(
                    RichText::new(t("take.record_mix"))
                        .text_style(design::caption())
                        .color(pal.text_dim),
                );
            });
            pan_law_picker(app, ui);
        });
    });
}

/// The polarity switch. A Ø that is *on* is a state, so it gets the accent fill — the one
/// thing the design system reserves it for.
pub fn phase_button(ui: &mut egui::Ui, inverted: bool) -> egui::Response {
    switch(
        ui,
        inverted,
        &t("track.phase"),
        theme::p().gold_bright,
        &t("track.phase_tip"),
    )
}

/// The pan law lives on the master strip because that is where somebody mixing looks for
/// it, and it is a *project* setting rather than a preference: it decides what the file
/// renders as (ADR 0012), so it travels with the file.
fn pan_law_picker(app: &mut CantusApp, ui: &mut egui::Ui) {
    let current = app.project().pan_law;
    ui.label(
        RichText::new(t("mixer.pan_law").to_uppercase())
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
    let mut chosen = None;
    egui::ComboBox::from_id_salt("pan-law")
        .width(strip_w() - space::S4)
        .selected_text(RichText::new(current.id()).text_style(design::mono_small()))
        .show_ui(ui, |ui| {
            for law in PanLaw::ALL {
                if ui
                    .selectable_label(law == current, t(law.label_key()))
                    .clicked()
                    && law != current
                {
                    chosen = Some(law);
                }
            }
        })
        .response
        .on_hover_text(t("panlaw.tip"));
    if let Some(law) = chosen {
        app.exec(Command::SetPanLaw { law });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Instant rise, a constant fall in dB: a 0 dBFS hit reads −12 dB half a second
    /// later, whatever the frame rate, and lands at silence.
    #[test]
    fn meters_rise_at_once_and_fall_at_a_constant_rate() {
        let mut v = MixerView::default();
        let t0 = Instant::now();
        assert_eq!(v.ballistic_at(0, 1.0, t0), 1.0);
        let half = v.ballistic_at(0, 0.0, t0 + Duration::from_millis(500));
        assert!(
            (linear_to_db(half) + 12.0).abs() < 0.01,
            "{}",
            linear_to_db(half)
        );
        // In ten small steps or one big one, the same place.
        let mut w = MixerView::default();
        w.ballistic_at(0, 1.0, t0);
        let mut last = 0.0;
        for i in 1..=10 {
            last = w.ballistic_at(0, 0.0, t0 + Duration::from_millis(50 * i));
        }
        assert!((linear_to_db(last) - linear_to_db(half)).abs() < 0.01);
        // A louder block in the fall takes over at once.
        assert_eq!(v.ballistic_at(0, 0.9, t0 + Duration::from_millis(600)), 0.9);
        assert!(v.falling());
        assert_eq!(v.ballistic_at(0, 0.0, t0 + Duration::from_secs(10)), 0.0);
        assert!(!v.falling(), "a meter at silence has landed");
    }
}
