//! The panel under the pitch editor's canvas: **the note** — what the selection sings and
//! sounds, its pitch centre, drift, vibrato, formant and gain — and **the macros** on the
//! selection (every note when none is selected): correct pitch and correct drift by an
//! amount, and quantise timing to the grid.

use crate::app::CantusApp;
use crate::kit::knob::KnobSize;
use crate::kit::text::Src;
use crate::kit::{Style, Tok};
use crate::views::fx::controls::{self, Cell};
use cantus_core::pitch::edit::{self, Field};
use cantus_core::pitch::{PitchEdit, PitchOp};
use cantus_core::{pitch_name, ClipId, TrackId};
use egui::{pos2, Rect};

/// The fields of the note card, with their labels and units.
const FIELDS: [(Field, &str, &str); 5] = [
    (Field::Centre, "pitch.field.centre", "%"),
    (Field::Drift, "pitch.field.drift", "%"),
    (Field::Vibrato, "pitch.field.vibrato", "%"),
    (Field::Formant, "pitch.field.formant", "st"),
    (Field::Gain, "pitch.field.gain", "dB"),
];

/// "E4 −3 ct": a pitch as the nearest note and its distance from it.
pub fn note_words(m: f32) -> String {
    let near = m.round();
    let cents = ((m - near) * 100.0).round() as i32;
    format!("{} {cents:+} ct", pitch_name(near.clamp(0.0, 127.0) as u8))
}

pub(super) fn show(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    clip: ClipId,
    current: &PitchEdit,
    rect: Rect,
    s: &Style,
) {
    let gap = s.s(6.0);
    let inner = rect.shrink(s.s(4.0));
    let note_w = (controls::cells_width(&[KnobSize::M; 5]) + 140.0 + 24.0) * s.k;
    let split = (inner.left() + note_w).min(inner.left() + inner.width() * 0.62);
    let left = Rect::from_min_max(inner.min, pos2(split, inner.bottom()));
    let right = Rect::from_min_max(pos2(split + gap, inner.top()), inner.max);
    note_card(app, ui, track, clip, current, left, s);
    macro_card(app, ui, track, clip, current, right, s);
}

fn note_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    clip: ClipId,
    current: &PitchEdit,
    rect: Rect,
    s: &Style,
) {
    let sel = app.pitch.view.selected.clone();
    let first = current.notes.iter().find(|n| sel.contains(&n.id)).cloned();
    let hs = s.hue_of(Tok::HueTone);
    let _ = controls::card(ui, &hs, rect, Src::Key("pitch.card.note"), None, |ui, _| {
        controls::column(ui, &hs, 140.0, |ui| {
            controls::caption(ui, &hs, Src::Key("pitch.note.sounds"), 140.0);
            let sounds = first
                .as_ref()
                .map(|n| note_words(n.out_centre()))
                .unwrap_or_else(|| crate::i18n::t("pitch.note.none"));
            controls::readout(
                ui,
                &hs,
                &sounds,
                false,
                &crate::i18n::t("pitch.note.sounds_tip"),
                140.0,
            );
            controls::caption(ui, &hs, Src::Key("pitch.note.sung"), 140.0);
            let sung = first
                .as_ref()
                .map(|n| note_words(n.sung))
                .unwrap_or_default();
            controls::readout(
                ui,
                &hs,
                &sung,
                false,
                &crate::i18n::t("pitch.note.sung_tip"),
                140.0,
            );
        });
        ui.scope(|ui| {
            if first.is_none() {
                ui.disable();
            }
            for (field, label, unit) in FIELDS {
                let (lo, hi) = field.range();
                let value = first
                    .as_ref()
                    .map_or(field.default_value(), |n| field.get(n));
                let name = crate::i18n::t(&format!("{label}_tip"));
                let out = Cell::new(value, lo..=hi, field.default_value())
                    .size(KnobSize::M)
                    .label(Src::Key(label))
                    .name(&name)
                    .unit(unit)
                    .show(ui, &hs);
                if let Some(v) = out.changed {
                    let mut e = current.clone();
                    if edit::set_field(&mut e, &sel, field, v) {
                        app.commit_pitch(track, clip, Some(e), field.op(), true);
                    }
                }
                if out.done {
                    app.end_gesture();
                }
            }
        });
    });
}

fn macro_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    clip: ClipId,
    current: &PitchEdit,
    rect: Rect,
    s: &Style,
) {
    let hs = s.hue_of(Tok::HueSource);
    let sel = app.pitch.view.selected.clone();
    let ids: Vec<u32> = if sel.is_empty() {
        current.notes.iter().map(|n| n.id).collect()
    } else {
        sel
    };
    let title = if app.pitch.view.selected.is_empty() {
        "pitch.card.macros_all"
    } else {
        "pitch.card.macros"
    };
    let _ = controls::card(ui, &hs, rect, Src::Key(title), None, |ui, _| {
        let amount = Cell::new(app.pitch.view.correct_pitch, 0.0..=1.0, 1.0)
            .size(KnobSize::M)
            .label(Src::Key("pitch.macro.pitch"))
            .name(&crate::i18n::t("pitch.macro.pitch_tip"))
            .unit("%")
            .show(ui, &hs);
        if let Some(v) = amount.changed {
            app.pitch.view.correct_pitch = v;
        }
        let drift = Cell::new(app.pitch.view.correct_drift, 0.0..=1.0, 0.5)
            .size(KnobSize::M)
            .label(Src::Key("pitch.macro.drift"))
            .name(&crate::i18n::t("pitch.macro.drift_tip"))
            .unit("%")
            .show(ui, &hs);
        if let Some(v) = drift.changed {
            app.pitch.view.correct_drift = v;
        }
        controls::column(ui, &hs, 112.0, |ui| {
            let apply = |ui: &mut egui::Ui, key: &'static str, tip: &'static str| {
                controls::key(ui, &hs, Src::Key(key), &crate::i18n::t(tip), 112.0, false).clicked()
            };
            if apply(ui, "pitch.macro.apply_pitch", "pitch.macro.apply_pitch_tip") {
                let mut e = current.clone();
                if edit::correct_pitch(&mut e, &ids, app.pitch.view.correct_pitch) {
                    app.commit_pitch(track, clip, Some(e), PitchOp::CorrectPitch, false);
                }
            }
            if apply(ui, "pitch.macro.apply_drift", "pitch.macro.apply_drift_tip") {
                let mut e = current.clone();
                if edit::correct_drift(&mut e, &ids, app.pitch.view.correct_drift) {
                    app.commit_pitch(track, clip, Some(e), PitchOp::CorrectDrift, false);
                }
            }
            if apply(ui, "pitch.macro.quantise", "pitch.macro.quantise_tip") {
                quantise(app, track, clip, current, &ids);
            }
        });
    });
}

/// *Quantise timing*: each note's start to the nearest line of the editor's grid on the
/// timeline (its length kept, its neighbours respected).
pub(super) fn quantise(
    app: &mut CantusApp,
    track: TrackId,
    clip: ClipId,
    current: &PitchEdit,
    ids: &[u32],
) {
    let Some(tm) = app.clip_time(track, clip) else {
        return;
    };
    let grid = app.pitch.view.grid.0.max(1);
    let snap = |f: u64| -> u64 {
        let t = tm.tick(f as f64).0;
        let q = (t as f64 / grid as f64).round() as i64 * grid;
        tm.frame(cantus_core::Ticks(q)).round().max(0.0) as u64
    };
    let mut e = current.clone();
    if edit::quantise(&mut e, ids, snap) {
        app.commit_pitch(track, clip, Some(e), PitchOp::Quantise, false);
    }
}
