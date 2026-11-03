//! The audio editor's tools column (ADR 0096): level (gain, normalize), fades, noise
//! reduction by profile, remove silence and DC, and the spectral region's attenuation.
//! Each button asks the application for one edit through an [`Act`]; the knobs are the
//! editor's own settings (`Tools`), kept between takes.

use super::{Act, Session, Tools};
use crate::i18n::{t, tf};
use crate::kit::band;
use crate::kit::knob::KnobSize;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, State, Style};
use crate::views::audio_parts as parts;
use cantus_core::audio::FadeCurve;
use cantus_core::audio_edit::{Edit, Level};
use egui::{Rect, Sense, Ui, Vec2};

/// The column's width at 100 %.
pub const WIDTH: f32 = 292.0;

fn band_row(ui: &mut Ui, key: &str) {
    let s = Style::current();
    ui.add_space(6.0 * s.k);
    let (r, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), s.d(band::BAND)),
        Sense::hover(),
    );
    with_brush(ui.painter(), |b| {
        band::section(b, r, &s, State::REST, Src::Key(key), None, false)
    });
}

fn line(ui: &mut Ui, text: &str, warn: bool) {
    // Broken into lines at words: a hint reads whole in every language and width.
    let s = Style::current();
    let w = ui.available_width();
    crate::views::workspace_settings::wrapped(
        ui,
        text,
        Role::Body,
        if warn { s.t.danger } else { s.t.text_dim },
        w,
    );
}

/// A knob; `(unit, decimals, log)` says how its value reads and moves.
fn knob(
    ui: &mut Ui,
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    key: &str,
    (unit, decimals, log): (&'static str, usize, bool),
) {
    let name = t(key);
    crate::kit::widgets::Param::new(v, range, default)
        .format(move |x, b| {
            use std::fmt::Write;
            let _ = write!(b, "{x:.decimals$} {unit}");
        })
        .log(log)
        .size(KnobSize::M)
        .cell(Src::Key(key))
        .name(&name)
        .show(ui);
}

fn button(ui: &mut Ui, key: &str, enabled: bool, primary: bool) -> bool {
    parts::button(ui, &t(key), primary, enabled, 84.0).clicked() && enabled
}

/// The column, in `rect`. `busy` greys the buttons while the thread works.
pub fn show(ui: &mut Ui, rect: Rect, tools: &mut Tools, s: &Session, acts: &mut Vec<Act>) {
    let st = Style::current();
    let k = st.k;
    let busy = s.busy.is_some() || s.buf.is_none();
    let has_sel = s.sel.is_some_and(|x| !x.is_empty());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    egui::ScrollArea::vertical()
        .id_salt("cantus-editor-tools")
        .auto_shrink([false, false])
        .max_height(rect.height())
        .show(&mut child, |ui| {
            ui.set_width(rect.width() - 10.0 * k);
            ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 6.0 * k);
            // Level.
            band_row(ui, "editor.band.level");
            ui.horizontal(|ui| {
                knob(
                    ui,
                    &mut tools.gain_db,
                    -24.0..=24.0,
                    0.0,
                    "editor.gain",
                    ("dB", 1, false),
                );
                knob(
                    ui,
                    &mut tools.target_db,
                    -48.0..=0.0,
                    -1.0,
                    "editor.target",
                    ("dB", 1, false),
                );
                ui.vertical(|ui| {
                    if button(ui, "editor.apply_gain", !busy, false) {
                        acts.push(Act::Edit(Edit::Gain { db: tools.gain_db }));
                    }
                    if button(ui, "editor.normalize", !busy, true) {
                        acts.push(Act::Edit(Edit::Normalize {
                            level: tools.level,
                            target_db: tools.target_db,
                            ceiling_db: tools.ceiling_on.then_some(tools.ceiling_db),
                        }));
                    }
                });
            });
            let mut i = match tools.level {
                Level::Peak => 0,
                Level::Rms => 1,
                Level::Lufs => 2,
            };
            crate::kit::widgets::segmented(
                ui,
                &mut i,
                &[
                    Src::Key("editor.level.peak"),
                    Src::Key("editor.level.rms"),
                    Src::Key("editor.level.lufs"),
                ],
                &t("editor.level"),
                rect.width() / k - 14.0,
            );
            tools.level = [Level::Peak, Level::Rms, Level::Lufs][i];
            line(ui, &t("editor.level.hint"), false);
            ui.horizontal(|ui| {
                let name = t("editor.ceiling");
                crate::kit::widgets::led_button(
                    ui,
                    &mut tools.ceiling_on,
                    Src::Key("editor.ceiling"),
                    &name,
                    120.0,
                );
                if tools.ceiling_on {
                    knob(
                        ui,
                        &mut tools.ceiling_db,
                        -12.0..=0.0,
                        -1.0,
                        "editor.ceiling_db",
                        ("dB", 1, false),
                    );
                }
            });
            // Fades.
            // Joins: where pieces meet (cuts, pastes, silence).
            band_row(ui, "editor.band.joins");
            let name = t("editor.joins");
            crate::kit::widgets::led_button(
                ui,
                &mut tools.smooth_joins,
                Src::Key("editor.joins"),
                &name,
                rect.width() / k - 14.0,
            );
            line(ui, &t("editor.joins.hint"), false);
            band_row(ui, "editor.band.fades");
            let mut c = FadeCurve::ALL
                .iter()
                .position(|x| *x == tools.fade)
                .unwrap_or(0);
            crate::kit::widgets::segmented(
                ui,
                &mut c,
                &[
                    Src::Key("editor.curve.linear"),
                    Src::Key("editor.curve.equal"),
                    Src::Key("editor.curve.exp"),
                    Src::Key("editor.curve.log"),
                    Src::Key("editor.curve.s"),
                ],
                &t("editor.curve"),
                rect.width() / k - 14.0,
            );
            tools.fade = FadeCurve::ALL[c];
            ui.horizontal(|ui| {
                if button(ui, "editor.fade_in", !busy, false) {
                    acts.push(Act::Edit(Edit::FadeIn { curve: tools.fade }));
                }
                if button(ui, "editor.fade_out", !busy, false) {
                    acts.push(Act::Edit(Edit::FadeOut { curve: tools.fade }));
                }
            });
            // Noise reduction by profile.
            band_row(ui, "editor.band.noise");
            let hint = match (&s.profile, has_sel) {
                (Some(p), _) => tf("editor.noise.have", &[("n", &p.frames.to_string())]),
                (None, true) => t("editor.noise.ready"),
                (None, false) => t("editor.noise.how"),
            };
            line(ui, &hint, false);
            ui.horizontal(|ui| {
                if button(
                    ui,
                    "editor.noise.learn",
                    !busy && has_sel,
                    s.profile.is_none(),
                ) {
                    acts.push(Act::Learn);
                }
                if button(
                    ui,
                    "editor.noise.reduce",
                    !busy && s.profile.is_some(),
                    true,
                ) {
                    acts.push(Act::Edit(Edit::Denoise {
                        profile: s.profile.clone().expect("a profile"),
                        params: tools.denoise,
                    }));
                }
            });
            let p = &mut tools.denoise;
            ui.horizontal(|ui| {
                let mut pct = p.amount * 100.0;
                knob(
                    ui,
                    &mut pct,
                    0.0..=100.0,
                    100.0,
                    "editor.noise.amount",
                    ("%", 0, false),
                );
                p.amount = pct / 100.0;
                knob(
                    ui,
                    &mut p.reduction_db,
                    0.0..=60.0,
                    24.0,
                    "editor.noise.reduction",
                    ("dB", 0, false),
                );
                knob(
                    ui,
                    &mut p.sensitivity,
                    0.5..=6.0,
                    3.0,
                    "editor.noise.sensitivity",
                    ("x", 1, false),
                );
                let mut sm = p.smoothing * 100.0;
                knob(
                    ui,
                    &mut sm,
                    0.0..=100.0,
                    50.0,
                    "editor.noise.smoothing",
                    ("%", 0, false),
                );
                p.smoothing = sm / 100.0;
            });
            // Silence and DC.
            band_row(ui, "editor.band.silence");
            ui.horizontal(|ui| {
                let z = &mut tools.silence;
                knob(
                    ui,
                    &mut z.threshold_db,
                    -90.0..=-20.0,
                    -50.0,
                    "editor.silence.threshold",
                    ("dB", 0, false),
                );
                knob(
                    ui,
                    &mut z.min_ms,
                    50.0..=3000.0,
                    300.0,
                    "editor.silence.min",
                    ("ms", 0, true),
                );
                knob(
                    ui,
                    &mut z.keep_ms,
                    0.0..=1000.0,
                    80.0,
                    "editor.silence.keep",
                    ("ms", 0, false),
                );
            });
            ui.horizontal(|ui| {
                if button(ui, "editor.silence.remove", !busy, true) {
                    acts.push(Act::Edit(Edit::RemoveSilence(tools.silence)));
                }
                if button(ui, "editor.dc", !busy, false) {
                    acts.push(Act::Edit(Edit::DcRemove));
                }
            });
            // The spectral region.
            // Clicks.
            band_row(ui, "editor.band.clicks");
            line(ui, &t("editor.clicks.hint"), false);
            ui.horizontal(|ui| {
                let mut pct = tools.declick * 100.0;
                knob(
                    ui,
                    &mut pct,
                    0.0..=100.0,
                    50.0,
                    "editor.clicks.sensitivity",
                    ("%", 0, false),
                );
                tools.declick = pct / 100.0;
                ui.vertical(|ui| {
                    if button(ui, "editor.clicks.find", !busy, true) {
                        acts.push(Act::Edit(Edit::Declick {
                            sensitivity: tools.declick,
                        }));
                    }
                    if button(ui, "editor.clicks.repair", !busy && has_sel, false) {
                        acts.push(Act::Edit(Edit::Repair));
                    }
                });
            });
            band_row(ui, "editor.band.spectral");
            let region = match s.band {
                Some(b) => tf(
                    "editor.region.is",
                    &[("lo", &hz_text(b.lo_hz)), ("hi", &hz_text(b.hi_hz))],
                ),
                None => t("editor.region.how"),
            };
            line(ui, &region, false);
            ui.horizontal(|ui| {
                knob(
                    ui,
                    &mut tools.atten_db,
                    -60.0..=12.0,
                    -12.0,
                    "editor.region.gain",
                    ("dB", 1, false),
                );
                if button(
                    ui,
                    "editor.region.apply",
                    !busy && s.band.is_some() && has_sel,
                    true,
                ) {
                    acts.push(Act::Edit(Edit::Attenuate {
                        lo_hz: 0.0,
                        hi_hz: 0.0,
                        gain_db: tools.atten_db,
                    }));
                }
            });
        });
}

/// "850 Hz", "2.4 kHz".
pub fn hz_text(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}
