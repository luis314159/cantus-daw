//! The Tuning window's controls, down its left side: the track's override (a track's
//! window), the system and its choices (a temperament and its key, *follow the song's key*;
//! a Scala scale and its keyboard map; the MTS-ESP master's), the diapason and its presets,
//! the Scala files, MTS-ESP, and the audition. Each control makes one command through
//! `super::set` — nothing here writes the song. r42:tuning-face

use super::window::{band_row, pick, put, row};
use super::{words, Target};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::text::Src;
use crate::kit::widgets::{menu_field, segmented};
use crate::kit::{with_brush, Fill, Style};
use crate::views::audio_parts as parts;
use crate::views::workspace_settings::note;
use cantus_core::tuning::{
    Temperament, TrackTuning, Tuning, TuningSystem, A4_MAX_HZ, A4_MIN_HZ, A4_PRESETS_HZ,
};
use cantus_rt::mts::{MasterState, Refusal};
use egui::{Rect, Ui};

/// A row's control at 100 %: what is left of the row after its caption.
fn width(ui: &Ui) -> f32 {
    let s = Style::current();
    (ui.available_width() / s.k - 1.0).max(160.0)
}

/// A segmented control published as `name#i`; the choice clicked when it changed.
fn seg(ui: &mut Ui, selected: usize, labels: &[Src<'_>], name: &str) -> (Option<usize>, Rect) {
    let mut i = selected;
    let wd = width(ui);
    let r = segmented(ui, &mut i, labels, &t(name), wd).rect;
    let n = labels.len() as f32;
    for j in 0..labels.len() {
        let w = r.width() / n;
        let x = r.left() + w * j as f32;
        put(ui, format!("{name}#{j}"), Rect::from_min_size(egui::pos2(x, r.top()), egui::vec2(w, r.height())));
    }
    ((i != selected).then_some(i), r)
}

fn field(ui: &mut Ui, value: &str, name: &str, steppers: bool) -> (egui::Response, crate::kit::widgets::FieldClicks) {
    let wd = width(ui);
    let (r, c) = menu_field(ui, value, steppers, false, &t(name), wd);
    put(ui, name, r.rect);
    (r, c)
}

/// A switch row of the kit (a lamp and its words), published as `name`.
fn check(ui: &mut Ui, on: bool, enabled: bool, label: &str, name: &str) -> bool {
    let r = ui.vertical(|ui| crate::views::interop::form::choice(ui, on, enabled, label));
    put(ui, name, r.response.rect);
    r.inner && enabled
}

/// A segment of `r` (one of `n`) veiled: greyed, its reason said beside it.
fn veil_segment(ui: &Ui, r: Rect, n: usize, i: usize) {
    let s = Style::current();
    let w = r.width() / n as f32;
    let seg = Rect::from_min_size(egui::pos2(r.left() + w * i as f32, r.top()), egui::vec2(w, r.height()));
    with_brush(ui.painter(), |b| {
        b.rrect(seg.shrink(1.0), s.r(3.0), Fill::Solid(crate::kit::color::with_alpha(s.t.panel, 0.6)))
    });
}

/// Every row.
pub(super) fn body(app: &mut CantusApp, ui: &mut Ui, target: Target) {
    if let Target::Track(id) = target {
        track_rows(app, ui, id);
    }
    system_rows(app, ui, target);
    pitch_rows(app, ui, target);
    file_rows(app, ui, target);
    mts_rows(app, ui, target);
    audition_rows(app, ui, target);
}

fn track_rows(app: &mut CantusApp, ui: &mut Ui, id: cantus_core::TrackId) {
    let name = app.project().track(id).map(|t| t.name.clone()).unwrap_or_default();
    band_row(ui, "tuning.band.track");
    note(ui, &tf("tuning.track.which", &[("track", &name)]), false);
    let mode = super::track_mode(app, Target::Track(id)).unwrap_or_default();
    let at = match mode {
        TrackTuning::Follow => 0,
        TrackTuning::Equal => 1,
        TrackTuning::Own { .. } => 2,
    };
    let labels = [
        Src::Key("tuning.track.follow"),
        Src::Key("tuning.track.equal"),
        Src::Key("tuning.track.own"),
    ];
    if let (Some(i), _) = row(ui, "tuning.row.plays", |ui| seg(ui, at, &labels, "track")) {
        let next = match i {
            0 => TrackTuning::Follow,
            1 => TrackTuning::Equal,
            // Its own starts from what it plays now.
            _ => TrackTuning::Own {
                tuning: super::tuning_of(app, Target::Track(id)),
            },
        };
        super::set_track_mode(app, id, next);
    }
    if !matches!(mode, TrackTuning::Own { .. }) {
        note(ui, &t("tuning.track.own_hint"), false);
    }
}

fn system_rows(app: &mut CantusApp, ui: &mut Ui, target: Target) {
    let tu = super::tuning_of(app, target);
    let song_key = app.project().key;
    band_row(ui, "tuning.band.system");
    let at = match tu.system {
        TuningSystem::Equal => 0,
        TuningSystem::Temperament { .. } => 1,
        TuningSystem::Scale(_) => 2,
        TuningSystem::External => 3,
    };
    let labels = [
        Src::Key("tuning.system.equal"),
        Src::Key("tuning.system.temperament"),
        Src::Key("tuning.system.scale"),
        Src::Key("tuning.system.external"),
    ];
    let mts_here = app.tuning.mts.as_ref().is_some_and(|m| m.library().is_some());
    let (clicked, r) = row(ui, "tuning.row.system", |ui| seg(ui, at, &labels, "system"));
    if !mts_here {
        veil_segment(ui, r, 4, 3);
    }
    if let Some(i) = clicked {
        let system = match i {
            0 => Some(TuningSystem::Equal),
            1 => Some(TuningSystem::Temperament {
                id: Temperament::WerckmeisterIii,
                key: song_key.map_or(0, |k| k.root),
                follow_song_key: false,
            }),
            // A scale comes from a file: the import, not this key.
            2 => {
                super::io::import_dialog(app, target);
                None
            }
            _ if mts_here => Some(TuningSystem::External),
            _ => None,
        };
        if let Some(system) = system {
            super::set(app, target, Tuning { system, ..tu.clone() });
        }
    }
    match &tu.system {
        TuningSystem::Equal => note(ui, &t("tuning.system.equal_hint"), false),
        TuningSystem::Temperament { id, key, follow_song_key } => {
            temperament_rows(app, ui, target, &tu, *id, *key, *follow_song_key)
        }
        TuningSystem::Scale(s) => scale_rows(ui, &tu, s),
        TuningSystem::External => {
            let words = match app.tuning.mts.as_ref() {
                Some(m) if m.follows() => match m.master_name() {
                    Some(n) => tf("tuning.mts.following_named", &[("name", &n)]),
                    None => t("tuning.mts.following"),
                },
                _ => t("tuning.mts.no_master"),
            };
            note(ui, &words, false);
        }
    }
    if !mts_here && at != 3 {
        note(ui, &t("tuning.mts.absent_short"), false);
    }
}

fn temperament_rows(
    app: &mut CantusApp,
    ui: &mut Ui,
    target: Target,
    tu: &Tuning,
    id: Temperament,
    key: u8,
    follow: bool,
) {
    let song_key = app.project().key;
    let (f, _) = row(ui, "tuning.row.temperament", |ui| field(ui, &t(id.name_key()), "temperament", false));
    let options: Vec<(Temperament, String)> = Temperament::ALL.iter().map(|x| (*x, t(x.name_key()))).collect();
    if let Some(next) = pick(&f, &options, Some(id)) {
        let system = TuningSystem::Temperament { id: next, key, follow_song_key: follow };
        super::set(app, target, Tuning { system, ..tu.clone() });
    }
    let shown = super::effective(tu, song_key);
    let shown_key = match shown.system {
        TuningSystem::Temperament { key, .. } => key,
        _ => key,
    };
    let (f, clicks) = row(ui, "tuning.row.key", |ui| field(ui, words::key_name(shown_key), "key", true));
    let options: Vec<(u8, String)> = (0..12u8).map(|k| (k, words::key_name(k).to_string())).collect();
    let mut next = if follow { None } else { pick(&f, &options, Some(key)) };
    if !follow && clicks.prev {
        next = Some((key + 11) % 12);
    }
    if !follow && clicks.next {
        next = Some((key + 1) % 12);
    }
    if let Some(k) = next {
        let system = TuningSystem::Temperament { id, key: k, follow_song_key: false };
        super::set_merging(app, target, Tuning { system, ..tu.clone() }, "key");
    }
    let label = t("tuning.follow_song_key");
    if row(ui, "", |ui| check(ui, follow, song_key.is_some(), &label, "follow")) {
        let system = TuningSystem::Temperament {
            id,
            key: song_key.map_or(key, |k| k.root),
            follow_song_key: !follow,
        };
        super::set(app, target, Tuning { system, ..tu.clone() });
    }
    match song_key {
        Some(k) if follow => note(
            ui,
            &tf("tuning.follow.on", &[("key", &crate::notefx_ui::key_name(Some(k)))]),
            false,
        ),
        None => note(ui, &t("tuning.follow.no_key"), false),
        _ => {}
    }
}

fn scale_rows(ui: &mut Ui, tu: &Tuning, s: &cantus_core::tuning::ScalaScale) {
    let name = words::scale_name(s);
    row(ui, "tuning.row.scale", |ui| field(ui, &name, "scale", false));
    let period = s.degrees.last().map(words::degree_written).unwrap_or_default();
    note(
        ui,
        &tf(
            "tuning.scale.notes",
            &[("n", &s.degrees.len().to_string()), ("period", &period)],
        ),
        false,
    );
    let map = match &tu.map {
        None => t("tuning.map.none"),
        Some(m) => tf(
            "tuning.map.some",
            &[
                ("size", &m.size.to_string()),
                ("key", &crate::controllers::note_name(m.reference_key)),
                ("hz", &words::hz_text(m.reference_hz)),
            ],
        ),
    };
    row(ui, "tuning.row.map", |ui| field(ui, &map, "map", false));
}

fn pitch_rows(app: &mut CantusApp, ui: &mut Ui, target: Target) {
    let tu = super::tuning_of(app, target);
    band_row(ui, "tuning.band.pitch");
    let caption = if tu.map.is_some() { "tuning.row.a4_map" } else { "tuning.row.a4" };
    let value = tf("tuning.hz", &[("hz", &words::hz_text(tu.a4_hz))]);
    let (_, clicks) = row(ui, caption, |ui| field(ui, &value, "a4", true));
    let step = |hz: f64, d: f64| ((hz + d) * 10.0).round() / 10.0;
    let next = if clicks.prev {
        Some(step(tu.a4_hz, -1.0))
    } else if clicks.next {
        Some(step(tu.a4_hz, 1.0))
    } else {
        None
    };
    if let Some(hz) = next {
        let hz = hz.clamp(A4_MIN_HZ, A4_MAX_HZ);
        super::set_merging(app, target, Tuning { a4_hz: hz, ..tu.clone() }, "a4");
    }
    let labels: Vec<String> = A4_PRESETS_HZ.iter().map(|h| words::hz_text(*h)).collect();
    let srcs: Vec<Src<'_>> = labels.iter().map(|l| Src::Str(l)).collect();
    let at = A4_PRESETS_HZ.iter().position(|h| *h == tu.a4_hz).unwrap_or(usize::MAX);
    if let (Some(i), _) = row(ui, "tuning.row.presets", |ui| seg(ui, at, &srcs, "preset")) {
        if let Some(hz) = A4_PRESETS_HZ.get(i) {
            super::set(app, target, Tuning { a4_hz: *hz, ..tu.clone() });
        }
    }
    let hint = if tu.map.is_some() { "tuning.a4.map_hint" } else { "tuning.a4.hint" };
    note(ui, &t(hint), false);
}

fn file_rows(app: &mut CantusApp, ui: &mut Ui, target: Target) {
    band_row(ui, "tuning.band.files");
    row(ui, "tuning.row.scala", |ui| {
        let b = parts::button(ui, &t("tuning.import"), false, true, 120.0);
        put(ui, "import", b.rect);
        if b.clicked() {
            super::io::import_dialog(app, target);
        }
        let b = parts::button(ui, &t("tuning.export"), false, true, 100.0);
        put(ui, "export", b.rect);
        if b.clicked() {
            super::io::export_dialog(app, target);
        }
    });
    match &app.tuning.last {
        Some(Ok(w)) => note(ui, w, false),
        Some(Err(w)) => note(ui, w, true),
        None => note(ui, &t("tuning.files.hint"), false),
    }
}

fn mts_rows(app: &mut CantusApp, ui: &mut Ui, target: Target) {
    band_row(ui, "tuning.band.mts");
    let link = app.tuning.mts.clone();
    let lib = link.as_ref().and_then(|l| l.library());
    let Some(lib) = lib else {
        note(ui, &t("tuning.mts.absent"), false);
        let label = t("tuning.mts.master");
        row(ui, "", |ui| check(ui, false, false, &label, "master"));
        return;
    };
    let state = lib.master_state();
    let status = match state {
        MasterState::Ours => match lib.clients() {
            Some(n) => tf("tuning.mts.ours_clients", &[("n", &n.to_string())]),
            None => t("tuning.mts.ours"),
        },
        MasterState::Free => t("tuning.mts.free"),
        MasterState::Another { .. } => t("tuning.mts.another"),
        MasterState::Unsupported => t("tuning.mts.unsupported"),
    };
    note(ui, &status, false);
    let project = target == Target::Project;
    let external = matches!(super::tuning_of(app, Target::Project).system, TuningSystem::External);
    let enabled = project && !external && !matches!(state, MasterState::Unsupported);
    let on = state == MasterState::Ours;
    let label = t("tuning.mts.master");
    if row(ui, "", |ui| check(ui, on, enabled, &label, "master")) {
        super::set_master(app, !on, false);
    }
    if !project {
        note(ui, &t("tuning.mts.master_project_only"), false);
    } else if external {
        note(ui, &t("tuning.mts.master_not_external"), false);
    }
    match app.tuning.master_refused {
        Some(Refusal::Another { can_reinitialize }) => {
            note(ui, &t("tuning.mts.refused"), true);
            if can_reinitialize {
                row(ui, "", |ui| {
                    let b = parts::button(ui, &t("tuning.mts.reinitialize"), false, true, 140.0);
                    put(ui, "reinitialize", b.rect);
                    if b.clicked() {
                        super::set_master(app, true, true);
                    }
                });
                note(ui, &t("tuning.mts.reinitialize_hint"), false);
            }
        }
        Some(Refusal::Unsupported) => note(ui, &t("tuning.mts.unsupported"), true),
        None => {}
    }
}

fn audition_rows(app: &mut CantusApp, ui: &mut Ui, target: Target) {
    band_row(ui, "tuning.band.audition");
    let track = super::audition_track(app, target);
    let (tonic, minor) = super::audition_key(app, target);
    let playing = app.tuning.audition.is_some();
    let label = if playing {
        t("tuning.audition.stop")
    } else {
        let mode = if minor { "tuning.audition.minor" } else { "tuning.audition.major" };
        tf("tuning.audition.play", &[("key", words::key_name(tonic)), ("mode", &t(mode))])
    };
    let clicked = row(ui, "tuning.row.audition", |ui| {
        let b = parts::button(ui, &label, true, track.is_some(), 160.0);
        put(ui, "audition", b.rect);
        b.clicked()
    });
    if clicked {
        if playing {
            super::stop_audition(app);
        } else {
            let now = ui.input(|i| i.time);
            super::start_audition(app, target, now);
        }
    }
    let words = match track.and_then(|t| app.project().track(t)) {
        Some(t) => tf("tuning.audition.on", &[("track", &t.name)]),
        None => crate::i18n::t("tuning.audition.no_track"),
    };
    note(ui, &words, false);
}
