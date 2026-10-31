//! The *Convert to MIDI* dialog, drawn with the kit (ADR 0024): a floating panel over a
//! dimmed room — its title, the clip it converts, *What* (melody, notes and chords, drums),
//! how it listens (sensitivity, shortest note, pitch bends, or the drum classes to write),
//! where the notes go (instrument, quantise), the model with its licence and size, and a
//! footer with the progress and the buttons. No stock egui widget.

use super::run;
use super::{Grid, Instrument, DRUM_MODEL, NOTE_MODEL};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{band, well, with_brush, Fill, State, Style, Tok};
use crate::views::audio_parts as parts;
use cantus_content::models::ModelStatus;
use cantus_ml::transcribe::spec::Mode;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

/// The window at 100 %.
pub const SIZE: Vec2 = Vec2::new(600.0, 640.0);
/// A row's caption column.
const CAPTION: f32 = 120.0;

fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, Id::new("cantus-transcribe"))
}

/// The dialog, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(mut d) = app.transcribe.dialog.clone() else {
        return;
    };
    if std::mem::take(&mut d.fresh) {
        egui::Popup::close_all(ctx);
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let busy = app.transcribe.busy();
    egui::Area::new(Id::new("cantus-transcribe-veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            with_brush(ui.painter(), |b| {
                b.rect(
                    r,
                    Fill::Solid(crate::kit::color::with_alpha(s.t.shadow, 0.5)),
                )
            });
        });
    let size = (SIZE * k).min(screen.size() - Vec2::splat(24.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let mut close = false;
    let mut convert = false;
    let mut stop = false;
    let mut get_drums = false;
    egui::Area::new(layer().id)
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 34.0 * k));
            if crate::views::workspace_settings::title(
                ui,
                head,
                &s,
                Glyph::PianoRoll,
                "transcribe.title",
            ) {
                close = true;
            }
            let foot = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, rect.bottom() - 52.0 * k),
                Pos2::new(rect.right() - 18.0 * k, rect.bottom() - 14.0 * k),
            );
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, head.bottom() + 10.0 * k),
                Pos2::new(rect.right() - 18.0 * k, foot.top() - 10.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .id_salt("cantus-transcribe-body")
                .auto_shrink([false, false])
                .max_height(body.height())
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 12.0 * k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 6.0 * k);
                    ui.add_enabled_ui(!busy, |ui| {
                        get_drums = rows(app, ui, &mut d);
                    });
                });
            let (c, go, st) = footer(app, ui, foot, &d);
            close |= c;
            convert |= go;
            stop |= st;
        });
    ctx.move_to_top(layer());
    let typing = ctx.egui_wants_keyboard_input() || crate::kit::field::active(ctx);
    let list_open = egui::Popup::is_any_open(ctx);
    let escape = !list_open && !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let enter = !list_open && !typing && ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if let Some(open) = app.transcribe.dialog.as_mut() {
        open.settings = d.settings.clone();
        open.fresh = false;
    }
    if get_drums {
        if let Err(e) = run::install(app, DRUM_MODEL) {
            app.transcribe.last = Some(Err(e));
        }
    }
    if stop {
        run::stop(app);
    } else if (convert || enter) && !busy {
        run::start(app);
    } else if (close || escape) && !busy {
        run::close(app);
    }
}

/// A band naming the rows under it.
fn band_row(ui: &mut Ui, key: &str) {
    let s = Style::current();
    ui.add_space(4.0 * s.k);
    let (r, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), s.d(band::BAND)),
        Sense::hover(),
    );
    with_brush(ui.painter(), |b| {
        band::section(b, r, &s, State::REST, Src::Key(key), None, false)
    });
}

fn row<R>(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let s = Style::current();
    ui.horizontal(|ui| {
        crate::views::workspace_widgets::row_label(ui, key, CAPTION, 24.0);
        ui.add_space(6.0 * s.k);
        add(ui)
    })
    .inner
}

/// One line of text the width of the body, dimmed or warning.
fn line(ui: &mut Ui, text: &str, warn: bool) {
    let s = Style::current();
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), s.d(18.0)), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            r,
            &Text::new(
                text,
                Role::Body,
                if warn { s.t.danger } else { s.t.text_dim },
            )
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

/// The choices. Returns whether *Download the drum model* was clicked.
fn rows(app: &CantusApp, ui: &mut Ui, d: &mut run::Dialog) -> bool {
    let s = Style::current();
    let k = s.k;
    let st = &mut d.settings;
    let w = ui.available_width() / k - CAPTION - 14.0;
    if let Ok(src) = super::source(app, d.track, d.clip) {
        line(
            ui,
            &tf(
                "transcribe.source",
                &[
                    ("clip", &src.clip_name),
                    ("track", &src.track_name),
                    ("secs", &format!("{:.1}", src.seconds())),
                ],
            ),
            false,
        );
    }
    band_row(ui, "transcribe.band.what");
    let modes = [Mode::Melody, Mode::Polyphonic, Mode::Drums];
    let mut i = modes.iter().position(|m| *m == st.mode).unwrap_or(1);
    row(ui, "transcribe.mode", |ui| {
        crate::kit::widgets::segmented(
            ui,
            &mut i,
            &[
                Src::Key("transcribe.mode.melody"),
                Src::Key("transcribe.mode.poly"),
                Src::Key("transcribe.mode.drums"),
            ],
            &t("transcribe.mode"),
            w,
        );
    });
    st.mode = modes[i];
    line(ui, &t(mode_hint(st.mode)), false);
    band_row(ui, "transcribe.band.listen");
    ui.horizontal(|ui| {
        ui.add_space((CAPTION + 6.0) * k);
        // The melody tracker decides what is voiced by itself: no sensitivity to set.
        let mut pct = st.sensitivity * 100.0;
        let name = t("transcribe.sensitivity");
        if st.mode != Mode::Melody {
            crate::kit::widgets::Param::new(&mut pct, 0.0..=100.0, 50.0)
                .format(|v, b| {
                    use std::fmt::Write;
                    let _ = write!(b, "{v:.0} %");
                })
                .size(crate::kit::knob::KnobSize::M)
                .cell(Src::Key("transcribe.sensitivity"))
                .name(&name)
                .show(ui);
            st.sensitivity = (pct / 100.0).clamp(0.0, 1.0);
            ui.add_space(18.0 * k);
        }
        if st.mode != Mode::Drums {
            let name = t("transcribe.shortest");
            crate::kit::widgets::Param::new(&mut st.shortest_ms, 30.0..=1_000.0, 127.7)
                .format(|v, b| {
                    use std::fmt::Write;
                    let _ = write!(b, "{v:.0} ms");
                })
                .log(true)
                .size(crate::kit::knob::KnobSize::M)
                .cell(Src::Key("transcribe.shortest"))
                .name(&name)
                .show(ui);
        }
    });
    let drum_model = app.ml().models().status(DRUM_MODEL);
    let mut get = false;
    if st.mode == Mode::Drums {
        row(ui, "transcribe.classes", |ui| {
            let keys = [
                "transcribe.class.kick",
                "transcribe.class.snare",
                "transcribe.class.toms",
                "transcribe.class.hihat",
                "transcribe.class.cymbals",
            ];
            for (j, key) in keys.iter().enumerate() {
                crate::kit::widgets::led_button(
                    ui,
                    &mut st.classes[j],
                    Src::Key(key),
                    &t(key),
                    74.0,
                );
            }
        });
        line(
            ui,
            &t(if drum_model == ModelStatus::Ready {
                "transcribe.classes.with_model"
            } else {
                "transcribe.classes.without_model"
            }),
            false,
        );
        let stems = app.ml().models().ready(super::STEMS_MODEL).is_some();
        row(ui, "transcribe.separate", |ui| {
            crate::kit::widgets::led_button_if(
                ui,
                &mut st.separate,
                Src::Key("transcribe.separate.on"),
                &t("transcribe.separate"),
                230.0,
                stems,
            );
        });
        if !stems {
            st.separate = false;
            line(ui, &t("transcribe.separate.absent"), false);
        }
    } else {
        row(ui, "transcribe.bends", |ui| {
            crate::kit::widgets::led_button(
                ui,
                &mut st.bends,
                Src::Key("transcribe.bends.keep"),
                &t("transcribe.bends"),
                150.0,
            );
        });
    }
    band_row(ui, "transcribe.band.where");
    if st.mode != Mode::Drums {
        let insts = [Instrument::Unda, Instrument::Exemplar];
        let mut j = insts.iter().position(|x| *x == st.instrument).unwrap_or(0);
        row(ui, "transcribe.instrument", |ui| {
            crate::kit::widgets::segmented(
                ui,
                &mut j,
                &[
                    Src::Key("transcribe.inst.unda"),
                    Src::Key("transcribe.inst.exemplar"),
                ],
                &t("transcribe.instrument"),
                w.min(260.0),
            );
        });
        st.instrument = insts[j];
    } else {
        row(ui, "transcribe.instrument", |ui| {
            line(ui, &t("transcribe.inst.pulsus"), false);
        });
    }
    let mut q = Grid::ALL
        .iter()
        .position(|g| *g == st.quantize)
        .unwrap_or(0);
    row(ui, "transcribe.quantize", |ui| {
        crate::kit::widgets::segmented(
            ui,
            &mut q,
            &[
                Src::Key("transcribe.quantize.off"),
                Src::Str("1/8"),
                Src::Str("1/16"),
                Src::Str("1/32"),
            ],
            &t("transcribe.quantize"),
            w.min(300.0),
        );
    });
    st.quantize = Grid::ALL[q];
    line(ui, &t("transcribe.time_hint"), false);
    band_row(ui, "transcribe.band.model");
    if st.mode == Mode::Drums {
        model_line(app, ui, DRUM_MODEL, &drum_model);
        if !matches!(
            drum_model,
            ModelStatus::Ready | ModelStatus::Downloading { .. } | ModelStatus::Preparing
        ) {
            line(ui, &t("transcribe.model.drums_optional"), false);
            ui.horizontal(|ui| {
                ui.add_space((CAPTION + 6.0) * k);
                get = parts::button(ui, &t("transcribe.model.get_drums"), false, true, 200.0)
                    .clicked();
            });
        }
    } else if st.mode == Mode::Melody {
        line(ui, &t("transcribe.model.own"), false);
    } else {
        let status = app.ml().models().status(NOTE_MODEL);
        model_line(app, ui, NOTE_MODEL, &status);
        if status != ModelStatus::Ready {
            line(ui, &t("transcribe.model.first_use"), false);
        }
    }
    get
}

fn mode_hint(m: Mode) -> &'static str {
    match m {
        Mode::Melody => "transcribe.mode.melody.hint",
        Mode::Polyphonic => "transcribe.mode.poly.hint",
        Mode::Drums => "transcribe.mode.drums.hint",
    }
}

/// The model's name, maker, licence, size and where it stands.
fn model_line(app: &CantusApp, ui: &mut Ui, id: &str, status: &ModelStatus) {
    let Some(m) = app.ml().models().manifest().model(id) else {
        return;
    };
    let name = match crate::i18n::locale() {
        crate::i18n::Locale::Es => &m.name.es,
        crate::i18n::Locale::En => &m.name.en,
    };
    let size = bytes(m.download_size());
    let state = match status {
        ModelStatus::Ready => t("transcribe.model.ready"),
        ModelStatus::Downloading { done, total } => tf(
            "transcribe.model.downloading",
            &[(
                "pct",
                &format!("{:.0}", 100.0 * *done as f64 / (*total).max(1) as f64),
            )],
        ),
        ModelStatus::Retrying { .. } => t("transcribe.model.retrying"),
        ModelStatus::Preparing => t("transcribe.model.preparing"),
        ModelStatus::Failed(e) => tf("transcribe.model.failed", &[("why", e)]),
        ModelStatus::Unavailable => t("transcribe.err.unavailable"),
        ModelStatus::Absent { .. } => t("transcribe.model.absent"),
    };
    line(
        ui,
        &tf(
            "transcribe.model.line",
            &[
                ("name", name),
                ("licence", &m.licence),
                ("size", &size),
                ("state", &state),
            ],
        ),
        matches!(status, ModelStatus::Failed(_)),
    );
}

/// The progress or the last outcome, and the buttons. Returns (close, convert, stop).
fn footer(app: &CantusApp, ui: &mut Ui, rect: Rect, d: &run::Dialog) -> (bool, bool, bool) {
    let s = Style::current();
    let k = s.k;
    let buttons = Rect::from_min_max(Pos2::new(rect.right() - 250.0 * k, rect.top()), rect.max);
    let status = Rect::from_min_max(
        rect.min,
        Pos2::new(buttons.left() - 16.0 * k, rect.bottom()),
    );
    if let Some(r) = &app.transcribe.running {
        let bar = Rect::from_min_max(
            Pos2::new(status.left(), status.center().y - 11.0 * k),
            Pos2::new(status.right(), status.center().y + 11.0 * k),
        );
        with_brush(ui.painter(), |b| {
            progress(b, bar, &s, &t(r.stage.key()), r.done)
        });
    } else if let Some(last) = &app.transcribe.last {
        let (msg, warn) = match last {
            Ok(m) => (m.as_str(), false),
            Err(m) => (m.as_str(), true),
        };
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                status,
                &Text::new(
                    msg,
                    Role::Body,
                    if warn { s.t.danger } else { s.t.text_dim },
                )
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
        });
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(buttons)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let (mut close, mut convert, mut stop) = (false, false, false);
    if app.transcribe.busy() {
        stop = parts::button(&mut child, &t("transcribe.stop"), true, true, 96.0).clicked();
    } else {
        let needs =
            run::needed(d.settings.mode).is_some_and(|id| app.ml().models().ready(id).is_none());
        let label = if needs {
            t("transcribe.convert_download")
        } else {
            t("transcribe.convert")
        };
        let any = d.settings.mode != Mode::Drums || d.settings.classes.iter().any(|c| *c);
        convert = parts::button(&mut child, &label, true, any, 110.0).clicked() && any;
        child.add_space(8.0 * k);
        close = parts::button(&mut child, &t("transcribe.close"), false, true, 80.0).clicked();
    }
    (close, convert, stop)
}

/// "240 KB", "6.0 MB".
pub fn bytes(n: u64) -> String {
    if n < 1_000_000 {
        format!("{:.0} KB", (n as f64 / 1e3).max(1.0))
    } else {
        format!("{:.1} MB", n as f64 / 1e6)
    }
}

/// A stage's progress bar.
fn progress(b: &mut crate::kit::Brush, r: Rect, s: &Style, phase: &str, fraction: f32) {
    b.register(
        r,
        "transcribe-progress",
        &[Tok::Well, Tok::Selection, Tok::Text],
        &[],
    );
    well::paint(b, r, s, State::REST, None);
    let inner = r.shrink(s.s(2.0));
    let f = fraction.clamp(0.0, 1.0);
    b.rrect(
        Rect::from_min_size(inner.min, Vec2::new(inner.width() * f, inner.height())),
        s.r(1.5),
        Fill::Solid(crate::kit::color::with_alpha(s.t.selection, 0.55)),
    );
    let label = format!("{phase}  {:.0} %", f * 100.0);
    let _ = text::paint(
        b,
        inner,
        &Text::new(&label, Role::Value, s.t.text).align(Align2::CENTER_CENTER),
        s.tk(),
    );
}
