//! The score's toolbar (round 32, ADR 0075): the track's score settings — clef, staves,
//! the hands' split, transposition, reading grid, dynamics, the part's name — each a
//! command; zoom, following the playhead, note input; and the files: MusicXML and PDF
//! out, printing, MusicXML in.

use super::clips_chrome::Toolbar;
use super::score_view::Place;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::score::{self, Entry, Scope};
use cantus_core::notation::{ClefChoice, ReadingGrid, ScoreSettings, StaffChoice};
use cantus_core::TrackId;
use cantus_score::layout::Paper;
use cantus_score::model::Dur;

/// Transpositions offered by name: `(semitones, key)`.
const TRANSPOSITIONS: [(i8, &str); 7] = [
    (0, "score.transpose.concert"),
    (2, "score.transpose.b_flat"),
    (9, "score.transpose.e_flat"),
    (7, "score.transpose.f"),
    (3, "score.transpose.a"),
    (12, "score.transpose.octave_up"),
    (-12, "score.transpose.octave_down"),
];

/// Splits offered: follow the hands, or a fixed line.
const SPLITS: [(Option<u8>, &str); 4] = [
    (None, "score.split.hands"),
    (Some(60), "score.split.c4"),
    (Some(55), "score.split.g3"),
    (Some(65), "score.split.f4"),
];

fn track_of(scope: Scope) -> Option<TrackId> {
    match scope {
        Scope::Clip(t, _) | Scope::Track(t) => Some(t),
        Scope::Song => None,
    }
}

/// A radio menu of `choices` under `field`, setting the track's settings with `set`.
fn radio<T: Copy + PartialEq>(
    app: &mut CantusApp,
    r: &egui::Response,
    track: TrackId,
    choices: &[(T, &'static str)],
    current: T,
    set: impl Fn(&mut ScoreSettings, T),
) {
    egui::Popup::menu(r).frame(egui::Frame::NONE).show(|ui| {
        let items: Vec<Item<'_>> = choices
            .iter()
            .map(|(v, k)| Item::Action {
                label: Src::Key(k),
                check: Check::Radio(*v == current),
                shortcut: None,
                submenu: false,
                enabled: true,
            })
            .collect();
        let (_, clicks) = crate::kit::widgets::menu_list(ui, &items, 200.0);
        if let Some(i) = clicks.clicked {
            if let Some(mut st) = app.project().track(track).map(|t| t.score.clone()) {
                set(&mut st, choices[i].0);
                app.set_track_score(track, st);
            }
            ui.close();
        }
    });
}

pub fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, scope: Scope, place: Place) {
    let id = ui.id().with(("score-bar", place == Place::Split));
    let mut bar = Toolbar::new(ui);
    app.score.bar_hovered = ui.rect_contains_pointer(bar.rect);
    let Some(track) = track_of(scope) else { return };
    let Some(st) = app.project().track(track).map(|t| t.score.clone()) else {
        return;
    };
    if place == Place::Pane {
        let name = app
            .project()
            .track(track)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        bar.title(ui, Src::Str(&name), Some(&t("score.pane")));
        bar.divider(ui);
    }
    // The settings, each a field opening its choices.
    let clef_choices: Vec<(ClefChoice, &'static str)> = ClefChoice::ALL
        .iter()
        .map(|c| (*c, c.label_key()))
        .collect();
    if let Some(r) = bar.field(
        ui,
        id.with("clef"),
        Some(&t("score.clef")),
        &t(st.clef.label_key()),
        70.0,
        || t("score.clef_tip"),
    ) {
        radio(app, &r, track, &clef_choices, st.clef, |s, v| s.clef = v);
    }
    let staff_choices: Vec<(StaffChoice, &'static str)> = StaffChoice::ALL
        .iter()
        .map(|c| (*c, c.label_key()))
        .collect();
    if let Some(r) = bar.field(
        ui,
        id.with("staves"),
        Some(&t("score.staves")),
        &t(st.staves.label_key()),
        70.0,
        || t("score.staves_tip"),
    ) {
        radio(app, &r, track, &staff_choices, st.staves, |s, v| {
            s.staves = v
        });
    }
    let split_key = SPLITS
        .iter()
        .find(|(v, _)| *v == st.split)
        .map_or("score.split.custom", |(_, k)| k);
    if let Some(r) = bar.field(
        ui,
        id.with("split"),
        Some(&t("score.split")),
        &t(split_key),
        64.0,
        || t("score.split_tip"),
    ) {
        radio(app, &r, track, &SPLITS, st.split, |s, v| s.split = v);
    }
    let tr_key = TRANSPOSITIONS
        .iter()
        .find(|(v, _)| *v == st.transpose)
        .map_or("score.transpose.custom", |(_, k)| k);
    if let Some(r) = bar.field(
        ui,
        id.with("transpose"),
        Some(&t("score.transpose")),
        &t(tr_key),
        76.0,
        || t("score.transpose_tip"),
    ) {
        radio(app, &r, track, &TRANSPOSITIONS, st.transpose, |s, v| {
            s.transpose = v
        });
    }
    let grid_choices: Vec<(ReadingGrid, &'static str)> = ReadingGrid::ALL
        .iter()
        .map(|c| (*c, c.label_key()))
        .collect();
    if let Some(r) = bar.field(
        ui,
        id.with("grid"),
        Some(&t("score.grid")),
        &t(st.grid.label_key()),
        72.0,
        || t("score.grid_tip"),
    ) {
        radio(app, &r, track, &grid_choices, st.grid, |s, v| s.grid = v);
    }
    let dyn_r = bar.led(
        ui,
        id.with("dynamics"),
        st.dynamics,
        "score.dynamics",
        96.0,
        || t("score.dynamics_tip"),
    );
    if dyn_r.is_some_and(|r| r.clicked()) {
        let mut s2 = st.clone();
        s2.dynamics = !s2.dynamics;
        app.set_track_score(track, s2);
    }
    // The part's name: typed, Enter keeps it (empty is the track's own).
    if let Some(r) = bar.take_left(110.0, 22.0) {
        let mut text = app
            .score
            .name_edit
            .clone()
            .unwrap_or_else(|| st.name.clone().unwrap_or_default());
        let out = crate::kit::field::Field::new(id.with("name"))
            .hint(&t("score.name_hint"))
            .show(ui, r, &mut text);
        if out.focused {
            app.score.name_edit = Some(text.clone());
        }
        match out.event {
            Some(crate::kit::field::Event::Commit) => {
                let mut s2 = st.clone();
                let n = text.trim();
                s2.name = (!n.is_empty())
                    .then(|| n.chars().take(cantus_core::notation::MAX_NAME).collect());
                app.score.name_edit = None;
                if s2 != st {
                    app.set_track_score(track, s2);
                }
            }
            Some(crate::kit::field::Event::Cancel) => app.score.name_edit = None,
            _ => {}
        }
    }
    bar.divider(ui);
    // Note input (N) — only where there is a clip to put notes in.
    if let Scope::Clip(t_id, c_id) = scope {
        let on = app.score.entry.is_some();
        let r = bar.led(ui, id.with("entry"), on, "score.entry", 70.0, || {
            t("score.entry_tip")
        });
        if r.is_some_and(|r| r.clicked()) {
            toggle_entry(app, t_id, c_id);
        }
    }
    let follow = bar.led(
        ui,
        id.with("follow"),
        app.score.follow,
        "score.follow",
        76.0,
        || t("score.follow_tip"),
    );
    if follow.is_some_and(|r| r.clicked()) {
        app.score.follow = !app.score.follow;
    }
    for (k, f) in [("score.zoom_out", 1.0 / 1.15f32), ("score.zoom_in", 1.15)] {
        let word = if f > 1.0 { "+" } else { "−" };
        let r = bar.word(ui, id.with(k), word, 24.0, || t(k));
        if r.is_some_and(|r| r.clicked()) {
            let v = match place {
                Place::Split => &mut app.score.split_space,
                Place::Pane => &mut app.score.pane_space,
            };
            *v = (*v * f).clamp(score::SPACE_RANGE.0, score::SPACE_RANGE.1);
        }
    }
    // The files.
    if let Some(r) = bar.field(ui, id.with("files"), None, &t("score.files"), 72.0, || {
        t("score.files_tip")
    }) {
        egui::Popup::menu(&r).frame(egui::Frame::NONE).show(|ui| {
            let keys = [
                "score.export_musicxml",
                "score.export_song_musicxml",
                "score.export_pdf_a4",
                "score.export_pdf_letter",
                "score.print",
                "score.import_musicxml",
            ];
            let items: Vec<Item<'_>> = keys
                .iter()
                .map(|k| Item::Action {
                    label: Src::Key(k),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            let (_, clicks) = crate::kit::widgets::menu_list(ui, &items, 230.0);
            if let Some(i) = clicks.clicked {
                ui.close();
                match i {
                    0 => app.export_musicxml(scope),
                    1 => app.export_musicxml(Scope::Song),
                    2 => app.export_pdf(scope, Paper::A4),
                    3 => app.export_pdf(scope, Paper::LETTER),
                    4 => app.print_score(scope),
                    _ => app.import_musicxml(),
                }
            }
        });
    }
}

/// Note input on or off: on, the cursor starts after the selected note (or at the
/// clip's start), a quarter note at a time.
pub fn toggle_entry(app: &mut CantusApp, track: TrackId, clip: cantus_core::ClipId) {
    if app.score.entry.take().is_some() {
        return;
    }
    let Some(c) = app.project().track(track).and_then(|t| t.clip(clip)) else {
        return;
    };
    let after = c
        .notes
        .iter()
        .filter(|n| app.selected_notes.contains(&n.id))
        .map(|n| (c.start + (n.end() - c.offset), n.pitch))
        .max_by_key(|x| x.0 .0);
    let (at, last) = after.map_or((c.start.0, 67), |(t, p)| (t.0, p));
    app.score.entry = Some(Entry {
        at,
        dur: Dur::new(2, 0),
        last,
    });
    app.score.focused = true;
}
