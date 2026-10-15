//! The pitch editor's bar: the clip, the tools, the snap, the key, *Original* (A/B), join
//! and reset, play from the selection, MIDI out and *Double*, the quantise grid, and what
//! the take is.

use super::{Snap, Tool};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::icons::Icon;
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{self, hint};
use crate::pitch::Take;
use crate::views::clips_chrome::{menu_frame, Toolbar};
use cantus_core::pitch::midi::Interval;
use cantus_core::pitch::{edit, PitchEdit, PitchKey, PitchOp};
use cantus_core::{ClipId, Ticks, TrackId};

/// The note names, C … B (Tonus's, one language for both).
pub const KEY_NAMES: [&str; 12] = crate::views::fx::tonus::KEY_NAMES;

/// "A minor": a key in words.
pub fn key_words(k: PitchKey) -> String {
    tf(
        "pitch.key.value",
        &[
            ("tonic", &t(KEY_NAMES[k.tonic as usize % 12])),
            ("scale", &t(&format!("fx.tonus.scale.{}", k.scale.min(12)))),
        ],
    )
}

pub(super) fn show(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    clip: ClipId,
    take: &Take,
    current: Option<&PitchEdit>,
) {
    let id = ui.id().with(("pitch-bar", clip.0));
    let mut bar = Toolbar::new(ui);
    {
        let a: &CantusApp = app;
        let tr = a.project().track(track);
        let clip_name = tr.and_then(|t| t.clip(clip)).map(|c| c.name.clone()).unwrap_or_default();
        let track_name = tr.map(|t| t.name.clone());
        bar.title(ui, Src::Str(&clip_name), track_name.as_deref());
    }
    // What the take is, on the right: the notes and the key, or the analysis under way.
    let status = match (take, current) {
        (_, Some(e)) => crate::i18n::tf_count(
            "pitch.status",
            e.notes.len(),
            &[("n", &e.notes.len()), ("key", &key_words(e.key))],
        ),
        (Take::Pending, None) => t("pitch.analysing_short"),
        _ => String::new(),
    };
    if !status.is_empty() {
        bar.readout_right(ui, &status, 120.0, false);
    }
    bar.divider(ui);
    for (tool, icon, key) in [
        (Tool::Edit, Icon::Select, "pitch.tool.edit"),
        (Tool::Split, Icon::Slice, "pitch.tool.split"),
    ] {
        let lit = app.pitch.view.tool == tool;
        if bar
            .key(ui, id.with(key), icon, lit, true, || t(&format!("{key}_tip")))
            .is_some_and(|r| r.clicked())
        {
            app.pitch.view.tool = tool;
        }
    }
    let Some(e) = current.cloned() else {
        return;
    };
    bar.divider(ui);
    // The snap of a pitch drag.
    let snap = app.pitch.view.snap;
    let cap = t("pitch.snap");
    if let Some(resp) = bar.field(ui, id.with("snap"), Some(&cap), &t(snap.label_key()), 84.0, || {
        t("pitch.snap_tip")
    }) {
        egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
            let items: Vec<Item<'_>> = Snap::ALL
                .iter()
                .map(|s| Item::Action {
                    label: Src::Key(s.label_key()),
                    check: Check::Radio(*s == snap),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            if let Some(i) = kit::widgets::menu_list(ui, &items, 160.0).1.clicked {
                app.pitch.view.snap = Snap::ALL[i];
                ui.close();
            }
        });
    }
    // The key: detected, editable.
    let cap = t("pitch.key");
    if let Some(resp) = bar.field(ui, id.with("key"), Some(&cap), &key_words(e.key), 110.0, || {
        t("pitch.key_tip")
    }) {
        egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
            key_menu(app, ui, track, clip, &e);
        });
    }
    // A/B: the original, as it was sung.
    if bar
        .led(ui, id.with("ab"), e.bypass, "pitch.original", 76.0, || t("pitch.original_tip"))
        .is_some_and(|r| r.clicked())
    {
        let mut x = e.clone();
        x.bypass = !x.bypass;
        app.commit_pitch(track, clip, Some(x), PitchOp::Compare, false);
    }
    bar.divider(ui);
    let sel = app.pitch.view.selected.clone();
    let join = t("pitch.join");
    if let Some(r) = bar.word(ui, id.with("join"), &join, 50.0, || t("pitch.join_tip")) {
        if r.clicked() {
            let mut x = e.clone();
            if edit::join(&mut x, &sel) {
                app.pitch.view.selected.truncate(1);
                app.commit_pitch(track, clip, Some(x), PitchOp::Join, false);
            } else {
                app.set_status(t("pitch.join_needs"));
            }
        }
    }
    let reset = t("pitch.reset");
    if let Some(r) = bar.word(ui, id.with("reset"), &reset, 54.0, || t("pitch.reset_tip")) {
        if r.clicked() {
            let ids: Vec<u32> = if sel.is_empty() {
                e.notes.iter().map(|n| n.id).collect()
            } else {
                sel.clone()
            };
            let mut x = e.clone();
            if edit::reset(&mut x, &ids) {
                app.commit_pitch(track, clip, Some(x), PitchOp::Reset, false);
            }
        }
    }
    bar.divider(ui);
    // Play from the selection (or the clip's start).
    let playing = app.engine.is_playing();
    if bar
        .key(ui, id.with("play"), if playing { Icon::Stop } else { Icon::Play }, playing, true, || {
            t("pitch.play_tip")
        })
        .is_some_and(|r| r.clicked())
    {
        if playing {
            app.toggle_play();
        } else {
            play_from_selection(app, track, clip, &e);
        }
    }
    // MIDI out: drag onto an instrument track, or click for a new one.
    midi_chip(app, ui, &mut bar, id, track, clip);
    let double = t("pitch.double");
    if let Some(resp) = bar.word(ui, id.with("double"), &double, 62.0, || t("pitch.double_tip")) {
        egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
            let items: Vec<Item<'_>> = Interval::ALL
                .iter()
                .map(|i| Item::action(Src::Key(i.label_key())))
                .collect();
            if let Some(i) = kit::widgets::menu_list(ui, &items, 180.0).1.clicked {
                if let Some((t2, c2)) = app.pitch_double(track, clip, Interval::ALL[i]) {
                    app.set_status(tf("pitch.doubled", &[("name", &clip_name(app, t2, c2))]));
                }
                ui.close();
            }
        });
    }
    bar.divider(ui);
    // The quantise grid.
    let grid = app.pitch.view.grid;
    let gname = crate::views::GRIDS
        .iter()
        .find(|(_, g)| Ticks(*g) == grid)
        .map_or_else(|| t("grid.sixteenth"), |(k, _)| t(k));
    let cap = t("pitch.grid");
    if let Some(resp) = bar.field(ui, id.with("grid"), Some(&cap), &gname, 70.0, || t("pitch.grid_tip")) {
        egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
            let items: Vec<Item<'_>> = crate::views::GRIDS
                .iter()
                .map(|(k, g)| Item::Action {
                    label: Src::Key(k),
                    check: Check::Radio(Ticks(*g) == grid),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            if let Some(i) = kit::widgets::menu_list(ui, &items, 140.0).1.clicked {
                app.pitch.view.grid = Ticks(crate::views::GRIDS[i].1);
                ui.close();
            }
        });
    }
}

fn clip_name(app: &CantusApp, track: TrackId, clip: ClipId) -> String {
    app.project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .map(|c| c.name.clone())
        .unwrap_or_default()
}

/// The key menu: the tonic, then the scale (Tonus's list).
fn key_menu(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId, e: &PitchEdit) {
    let mut items: Vec<Item<'_>> = vec![Item::Band(Src::Key("pitch.key.tonic"))];
    for (i, k) in KEY_NAMES.iter().enumerate() {
        items.push(Item::Action {
            label: Src::Key(k),
            check: Check::Radio(e.key.tonic as usize == i),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    items.push(Item::Band(Src::Key("pitch.key.scale")));
    let scales: Vec<String> = (0..13).map(|i| format!("fx.tonus.scale.{i}")).collect();
    for (i, k) in scales.iter().enumerate() {
        items.push(Item::Action {
            label: Src::Key(k),
            check: Check::Radio(e.key.scale as usize == i),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    if let Some(i) = kit::widgets::menu_list(ui, &items, 170.0).1.clicked {
        let mut x = e.clone();
        match i {
            1..=12 => x.key.tonic = (i - 1) as u8,
            14..=26 => x.key.scale = (i - 14) as u8,
            _ => return,
        }
        app.commit_pitch(track, clip, Some(x), PitchOp::Key, false);
        ui.close();
    }
}

/// Play from the first selected note (or the clip's start), a beat early.
fn play_from_selection(app: &mut CantusApp, track: TrackId, clip: ClipId, e: &PitchEdit) {
    let sel = &app.pitch.view.selected;
    let first = e.notes.iter().find(|n| sel.contains(&n.id)).map(|n| n.start);
    let at = match (first, app.clip_time(track, clip)) {
        (Some(f), Some(tm)) => {
            let beat = app.project().time_signature.ticks_per_beat();
            (tm.tick(f as f64) - beat).max(Ticks::ZERO)
        }
        _ => app
            .project()
            .track(track)
            .and_then(|t| t.clip(clip))
            .map_or(Ticks::ZERO, |c| c.start),
    };
    app.seek(at);
    app.toggle_play();
}

/// The MIDI chip: dragged onto a track of the arrangement it lands there as a note clip
/// at the voice's place; clicked, it makes a new instrument track for it.
fn midi_chip(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    bar: &mut Toolbar,
    id: egui::Id,
    track: TrackId,
    clip: ClipId,
) {
    let word = t("pitch.midi");
    let Some(r) = bar.word(ui, id.with("midi"), &word, 50.0, || t("pitch.midi_tip")) else {
        return;
    };
    let drag = ui.interact(r.rect, id.with("midi-drag"), egui::Sense::drag());
    if drag.drag_started() {
        app.pitch.view.midi_drag = true;
    }
    if app.pitch.view.midi_drag {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        hint::set_text(ui.ctx(), &t("pitch.midi_drop"), "", hint::Caps::NONE);
    }
    if drag.drag_stopped() {
        app.pitch.view.midi_drag = false;
        let pos = ui.input(|i| i.pointer.interact_pos());
        let onto = pos.and_then(|p| {
            let g = app.arrangement.geometry()?;
            if p.x < g.left || p.x > g.left + g.width || p.y < g.body_top {
                return None;
            }
            let row = g.row_at(p.y)?;
            app.project().tracks.get(row).map(|t| t.id)
        });
        match onto {
            Some(t2) => {
                if let Some((t3, c3)) = app.pitch_to_midi(track, clip, Some(t2)) {
                    app.set_status(tf("pitch.midi_made", &[("name", &clip_name(app, t3, c3))]));
                }
            }
            None => app.set_status(t("pitch.midi_drop")),
        }
    } else if r.clicked() {
        if let Some((t3, c3)) = app.pitch_to_midi(track, clip, None) {
            app.set_status(tf("pitch.midi_made", &[("name", &clip_name(app, t3, c3))]));
        }
    }
}
