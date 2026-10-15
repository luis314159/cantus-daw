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

/// The tonics, in the letters the notes are named with everywhere in the editor (the
/// blobs, the keyboard, the readouts): one notation in every language (review 1).
pub const TONICS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// "A minor" / "A menor": a key in words, the mode as a phrase reads it.
pub fn key_words(k: PitchKey) -> String {
    tf(
        "pitch.key.value",
        &[
            ("tonic", &TONICS[k.tonic as usize % 12]),
            ("scale", &t(&format!("pitch.key.scale.{}", k.scale.min(12)))),
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
        let clip_name = tr
            .and_then(|t| t.clip(clip))
            .map(|c| c.name.clone())
            .unwrap_or_default();
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
    let Some(e) = current else {
        if !status.is_empty() {
            bar.readout_right(ui, &status, 120.0, false);
        }
        tools(app, ui, &mut bar, id);
        return;
    };
    // What fits (review 1): the groups are measured, and those the bar has no room for —
    // the status first, then the grid, play and MIDI and Double, join and reset, and on a
    // very narrow bar Original, the key and the snap (review 2) — fold into a ⋯ menu at
    // the end; nothing is cut off. The widths are the ones the drawing below takes: the
    // tools' divider and two keys, a divider, each field.
    let snap = app.pitch.view.snap;
    let target_name = quantise_target_name(app, track);
    let need_tools = DIV + KEY + KEY + DIV;
    let core = [
        field_w(ui, &t("pitch.snap"), &t(snap.label_key()), 84.0),
        field_w(ui, &t("pitch.key"), &key_words(e.key), 110.0),
        ORIGINAL_W,
    ];
    let need_core = need_tools + core.iter().sum::<f32>();
    let need_edit =
        DIV + wide(ui, &t("pitch.join"), 50.0) + 4.0 + wide(ui, &t("pitch.reset"), 54.0) + 4.0;
    let need_play = DIV
        + KEY
        + wide(ui, &t("pitch.midi"), 50.0)
        + 4.0
        + wide(ui, &t("pitch.double"), 62.0)
        + 4.0;
    let need_grid = DIV + field_w(ui, &t("pitch.quantise_to"), &target_name, 70.0);
    let status_w = if status.is_empty() {
        0.0
    } else {
        status_need(ui, &status)
    };
    let room = bar.room() / bar.s.k;
    let more = MORE_W;
    let total = need_core + need_edit + need_play + need_grid;
    let mut show = [true; 3]; // edit, play, grid
    let mut show_core = [true; 3]; // snap, key, original
    let mut with_status = !status.is_empty();
    if total + status_w > room && total > room {
        with_status = false;
        // Original, then the key, then the snap, while even they and ⋯ do not fit.
        let mut used = need_core + more;
        for i in (0..3).rev() {
            if used > room {
                show_core[i] = false;
                used -= core[i];
            }
        }
        for (i, w) in [need_edit, need_play, need_grid].into_iter().enumerate() {
            show[i] = used + w <= room && show[..i].iter().all(|s| *s);
            if show[i] {
                used += w;
            }
        }
    } else if total + status_w > room {
        // Everything fits once the status gives way: no ⋯.
        with_status = false;
    }
    let folded = show.iter().chain(&show_core).any(|s| !*s);
    if with_status {
        bar.readout_right(ui, &status, 120.0, false);
    }
    tools(app, ui, &mut bar, id);
    bar.divider(ui);
    // The snap of a pitch drag.
    let cap = t("pitch.snap");
    if !show_core[0] {
        // Folded into ⋯.
    } else if let Some(resp) = bar.field(
        ui,
        id.with("snap"),
        Some(&cap),
        &t(snap.label_key()),
        84.0,
        || t("pitch.snap_tip"),
    ) {
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
    if !show_core[1] {
        // Folded into ⋯.
    } else if let Some(resp) = bar.field(
        ui,
        id.with("key"),
        Some(&cap),
        &key_words(e.key),
        110.0,
        || t("pitch.key_tip"),
    ) {
        egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
            key_menu(app, ui, track, clip, e);
        });
    }
    // A/B: the original, as it was sung.
    if show_core[2]
        && bar
            .led(ui, id.with("ab"), e.bypass, "pitch.original", 96.0, || {
                t("pitch.original_tip")
            })
            .is_some_and(|r| r.clicked())
    {
        compare(app, track, clip, e);
    }
    if show[0] {
        bar.divider(ui);
        let join = t("pitch.join");
        if let Some(r) = bar.word(ui, id.with("join"), &join, wide(ui, &join, 50.0), || {
            t("pitch.join_tip")
        }) {
            if r.clicked() {
                join_selection(app, track, clip, e);
            }
        }
        let reset = t("pitch.reset");
        if let Some(r) = bar.word(ui, id.with("reset"), &reset, wide(ui, &reset, 54.0), || {
            t("pitch.reset_tip")
        }) {
            if r.clicked() {
                reset_selection(app, track, clip, e);
            }
        }
    }
    if show[1] {
        bar.divider(ui);
        // Play from the selection (or the clip's start).
        let playing = app.engine.is_playing();
        if bar
            .key(
                ui,
                id.with("play"),
                if playing { Icon::Stop } else { Icon::Play },
                playing,
                true,
                || t("pitch.play_tip"),
            )
            .is_some_and(|r| r.clicked())
        {
            play(app, track, clip);
        }
        // MIDI out: drag onto an instrument track, or click for a new one.
        midi_chip(app, ui, &mut bar, id, track, clip);
        let word = t("pitch.double");
        if let Some(resp) = bar.word(ui, id.with("double"), &word, wide(ui, &word, 62.0), || {
            t("pitch.double_tip")
        }) {
            egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
                let items: Vec<Item<'_>> = Interval::ALL
                    .iter()
                    .map(|i| Item::action(Src::Key(i.label_key())))
                    .collect();
                if let Some(i) = kit::widgets::menu_list(ui, &items, 180.0).1.clicked {
                    double(app, track, clip, Interval::ALL[i]);
                    ui.close();
                }
            });
        }
    }
    if show[2] {
        bar.divider(ui);
        let cap = t("pitch.quantise_to");
        if let Some(resp) = bar.field(ui, id.with("grid"), Some(&cap), &target_name, 70.0, || {
            t("pitch.grid_tip")
        }) {
            egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
                quantise_menu(app, ui, track);
            });
        }
    }
    if folded {
        bar.divider(ui);
        if let Some(resp) = bar.word(ui, id.with("more"), "\u{2026}", 30.0, || {
            t("pitch.more_tip")
        }) {
            egui::Popup::menu(&resp).frame(menu_frame()).show(|ui| {
                more_menu(app, ui, track, clip, e, show, show_core);
            });
        }
    }
    app.pitch.view.bar_folded = [
        !show[0],
        !show[1],
        !show[2],
        !show_core[0],
        !show_core[1],
        !show_core[2],
    ];
}

/// A divider and a toolbar key, at 100 %, with their gaps.
const DIV: f32 = 10.0;
const KEY: f32 = crate::views::clips_chrome::KEY + 4.0;
/// The Original LED with its gap (`Toolbar::led` at 96).
const ORIGINAL_W: f32 = 100.0;
/// The ⋯ key: its divider, the word's 30 and its gap (review 2: counted as 34, it could
/// be left out).
const MORE_W: f32 = DIV + 30.0 + 4.0;

/// The width a caption | value field takes (as `Toolbar::field` lays it out).
fn field_w(ui: &egui::Ui, caption: &str, value: &str, w: f32) -> f32 {
    use crate::kit::text::{measure, Role, Text};
    let s = crate::kit::Style::current();
    let cap = measure(
        ui.ctx(),
        &Text::new(caption, Role::Label, s.t.text_dim),
        s.tk(),
    )
    .x / s.k;
    let val = measure(ui.ctx(), &Text::new(value, Role::Control, s.t.text), s.tk()).x / s.k;
    cap + 2.0 + 4.0 + w.max(val + 30.0) + 4.0
}

fn status_need(ui: &egui::Ui, status: &str) -> f32 {
    use crate::kit::text::{measure, Role, Text};
    let s = crate::kit::Style::current();
    let m = measure(ui.ctx(), &Text::new(status, Role::Body, s.t.text), s.tk()).x / s.k;
    m.max(106.0) + 14.0 + 4.0
}

/// The two tools.
fn tools(app: &mut CantusApp, ui: &mut egui::Ui, bar: &mut Toolbar, id: egui::Id) {
    bar.divider(ui);
    for (tool, icon, key) in [
        (Tool::Edit, Icon::Select, "pitch.tool.edit"),
        (Tool::Split, Icon::Slice, "pitch.tool.split"),
    ] {
        let lit = app.pitch.view.tool == tool;
        if bar
            .key(ui, id.with(key), icon, lit, true, || {
                t(&format!("{key}_tip"))
            })
            .is_some_and(|r| r.clicked())
        {
            app.pitch.view.tool = tool;
        }
    }
}

fn compare(app: &mut CantusApp, track: TrackId, clip: ClipId, e: &PitchEdit) {
    let mut x = e.clone();
    x.bypass = !x.bypass;
    app.commit_pitch(track, clip, Some(x), PitchOp::Compare, false);
}

fn join_selection(app: &mut CantusApp, track: TrackId, clip: ClipId, e: &PitchEdit) {
    let sel = app.pitch.view.selected.clone();
    let mut x = e.clone();
    if edit::join(&mut x, &sel) {
        app.pitch.view.selected.truncate(1);
        app.commit_pitch(track, clip, Some(x), PitchOp::Join, false);
    } else {
        app.set_status(t("pitch.join_needs"));
    }
}

fn reset_selection(app: &mut CantusApp, track: TrackId, clip: ClipId, e: &PitchEdit) {
    let sel = app.pitch.view.selected.clone();
    let ids: Vec<u32> = if sel.is_empty() {
        e.notes.iter().map(|n| n.id).collect()
    } else {
        sel
    };
    let mut x = e.clone();
    if edit::reset(&mut x, &ids) {
        app.commit_pitch(track, clip, Some(x), PitchOp::Reset, false);
    }
}

fn play(app: &mut CantusApp, track: TrackId, clip: ClipId) {
    if app.engine.is_playing() {
        app.toggle_play();
    } else {
        app.pitch_play_from_selection(track, clip);
    }
}

fn double(app: &mut CantusApp, track: TrackId, clip: ClipId, interval: Interval) {
    if let Some((t2, c2)) = app.pitch_double(track, clip, interval) {
        app.set_status(tf("pitch.doubled", &[("name", &clip_name(app, t2, c2))]));
    }
}

fn midi_new_track(app: &mut CantusApp, track: TrackId, clip: ClipId) {
    if let Some((t3, c3)) = app.pitch_to_midi(track, clip, None) {
        app.set_status(tf("pitch.midi_made", &[("name", &clip_name(app, t3, c3))]));
    }
}

/// The quantise target in words: the grid's name, or the other track's.
fn quantise_target_name(app: &CantusApp, track: TrackId) -> String {
    match app.quantise_target(track) {
        Some(other) => app
            .project()
            .track(other)
            .map(|t| t.name.clone())
            .unwrap_or_default(),
        None => {
            let grid = app.pitch.view.grid;
            crate::views::GRIDS
                .iter()
                .find(|(_, g)| Ticks(*g) == grid)
                .map_or_else(|| t("grid.sixteenth"), |(k, _)| t(k))
        }
    }
}

/// What *Quantise* moves notes to: a grid, or another track's notes (§3c).
fn quantise_menu(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId) {
    let grid = app.pitch.view.grid;
    let to = app.quantise_target(track);
    let others = app.pitch_onset_tracks(track);
    let mut items: Vec<Item<'_>> = vec![Item::Band(Src::Key("pitch.grid"))];
    for (k, g) in crate::views::GRIDS.iter() {
        items.push(Item::Action {
            label: Src::Key(k),
            check: Check::Radio(to.is_none() && Ticks(*g) == grid),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    if !others.is_empty() {
        items.push(Item::Band(Src::Key("pitch.quantise.tracks")));
        for (t2, name) in &others {
            items.push(Item::Action {
                label: Src::Str(name),
                check: Check::Radio(to == Some(*t2)),
                shortcut: None,
                submenu: false,
                enabled: true,
            });
        }
    }
    let n = crate::views::GRIDS.len();
    if let Some(i) = kit::widgets::menu_list(ui, &items, 180.0).1.clicked {
        if (1..=n).contains(&i) {
            app.pitch.view.grid = Ticks(crate::views::GRIDS[i - 1].1);
            app.pitch.view.quantise_to = None;
        } else if i > n + 1 {
            app.pitch.view.quantise_to = others.get(i - n - 2).map(|o| o.0);
        }
        ui.close();
    }
}

/// The ⋯ menu: what the bar had no room for.
fn more_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    clip: ClipId,
    e: &PitchEdit,
    show: [bool; 3],
    show_core: [bool; 3],
) {
    #[derive(Clone, Copy)]
    enum Do {
        Original,
        Join,
        Reset,
        Play,
        Midi,
        MidiOnto(TrackId),
        Double(Interval),
    }
    // The tracks the melody can go onto (the MIDI chip's drag, on a wide bar).
    let onto: Vec<(TrackId, String)> = if show[1] {
        Vec::new()
    } else {
        app.project()
            .tracks
            .iter()
            .filter(|t| t.id != track && !crate::record_session::is_audio_track(t))
            .map(|t| (t.id, t.name.clone()))
            .collect()
    };
    let mut items: Vec<Item<'_>> = Vec::new();
    let mut acts: Vec<Option<Do>> = Vec::new();
    if !show_core[2] {
        items.push(Item::Action {
            label: Src::Key("pitch.original"),
            check: if e.bypass { Check::On } else { Check::Off },
            shortcut: None,
            submenu: false,
            enabled: true,
        });
        acts.push(Some(Do::Original));
    }
    let mut push = |items: &mut Vec<Item<'_>>, key: &'static str, d: Option<Do>| {
        items.push(match d {
            Some(_) => Item::action(Src::Key(key)),
            None => Item::Band(Src::Key(key)),
        });
        acts.push(d);
    };
    if !show[0] {
        push(&mut items, "pitch.join", Some(Do::Join));
        push(&mut items, "pitch.reset", Some(Do::Reset));
    }
    if !show[1] {
        push(&mut items, "pitch.play", Some(Do::Play));
        push(&mut items, "pitch.midi_new", Some(Do::Midi));
        push(&mut items, "pitch.double", None);
        push(
            &mut items,
            "pitch.double.third",
            Some(Do::Double(Interval::Third)),
        );
        push(
            &mut items,
            "pitch.double.fifth",
            Some(Do::Double(Interval::Fifth)),
        );
        if !onto.is_empty() {
            push(&mut items, "pitch.midi_onto", None);
            for (t2, name) in &onto {
                items.push(Item::action(Src::Str(name)));
                acts.push(Some(Do::MidiOnto(*t2)));
            }
        }
    }
    let picked = if items.is_empty() {
        None
    } else {
        kit::widgets::menu_list(ui, &items, 220.0).1.clicked
    };
    if !show_core[0] {
        // The snap's own list.
        let snap = app.pitch.view.snap;
        let mut list: Vec<Item<'_>> = vec![Item::Band(Src::Key("pitch.snap"))];
        list.extend(Snap::ALL.iter().map(|s| Item::Action {
            label: Src::Key(s.label_key()),
            check: Check::Radio(*s == snap),
            shortcut: None,
            submenu: false,
            enabled: true,
        }));
        if let Some(i) = kit::widgets::menu_list(ui, &list, 220.0).1.clicked {
            if i > 0 {
                app.pitch.view.snap = Snap::ALL[i - 1];
                ui.close();
            }
        }
    }
    if !show_core[1] {
        // The key's own list.
        key_menu(app, ui, track, clip, e);
    }
    if !show[2] {
        // The quantise target's own list, under the rest.
        quantise_menu(app, ui, track);
    }
    let Some(d) = picked.and_then(|i| acts.get(i).copied().flatten()) else {
        return;
    };
    match d {
        Do::Original => compare(app, track, clip, e),
        Do::MidiOnto(t2) => {
            if let Some((t3, c3)) = app.pitch_to_midi(track, clip, Some(t2)) {
                app.set_status(tf("pitch.midi_made", &[("name", &clip_name(app, t3, c3))]));
            }
        }
        Do::Join => join_selection(app, track, clip, e),
        Do::Reset => reset_selection(app, track, clip, e),
        Do::Play => play(app, track, clip),
        Do::Midi => midi_new_track(app, track, clip),
        Do::Double(i) => double(app, track, clip, i),
    }
    ui.close();
}

/// A key's width for its word: at least `min`, wide enough for the word in capitals.
fn wide(ui: &egui::Ui, word: &str, min: f32) -> f32 {
    use crate::kit::text::{measure, Role, Text};
    let s = crate::kit::Style::current();
    let caps = word.to_uppercase();
    let m = measure(ui.ctx(), &Text::new(&caps, Role::Control, s.t.text), s.tk()).x / s.k;
    min.max(m + 20.0)
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
    for (i, k) in TONICS.iter().enumerate() {
        items.push(Item::Action {
            label: Src::Str(k),
            check: Check::Radio(e.key.tonic as usize == i),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    items.push(Item::Band(Src::Key("pitch.key.scale")));
    let scales: Vec<String> = (0..13).map(|i| format!("pitch.key.scale.{i}")).collect();
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
    let Some(r) = bar.word(ui, id.with("midi"), &word, wide(ui, &word, 50.0), || {
        t("pitch.midi_tip")
    }) else {
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
        midi_new_track(app, track, clip);
    }
}
