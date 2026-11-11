//! Aula's editor: the convolution reverb (round 39, ADR 0112).
//!
//! A convolution reverb is its impulse response, so the response is the editor's picture:
//! its outline in decibels over time, after the pre-delay, as the controls shape it (the
//! picture is drawn from a low-rate copy, so it follows a knob at once while the real
//! kernel is built off the audio thread). Beside it, where the response comes from:
//! Cantus's own rooms, halls, plates and springs, a file (loaded, or dropped on the
//! editor), and a pack's credit when the file is one. Under them, four cards: length and
//! time, the envelope, the tone, the output.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::band;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::{Command, EffectId, EffectKind, IrFile, StripId};
use egui::Rect;
use std::fmt::Write as _;

#[path = "aula_view.rs"]
mod view;
pub use view::{demo_feed, demo_setup, preview};

pub(crate) const KIND: EffectKind = EffectKind::Aula;

/// Factory presets: data (`presets/effects/aula.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The names of Cantus's own responses, by the `ir` parameter's index.
pub const IR_KEYS: [&str; cantus_core::fx::aula::FACTORY_COUNT] = [
    "aula.ir.0",
    "aula.ir.1",
    "aula.ir.2",
    "aula.ir.3",
    "aula.ir.4",
    "aula.ir.5",
    "aula.ir.6",
    "aula.ir.7",
    "aula.ir.8",
    "aula.ir.9",
    "aula.ir.10",
    "aula.ir.11",
    "aula.ir.12",
    "aula.ir.13",
    "aula.ir.14",
];

/// The extensions a response may be loaded from.
pub const EXTENSIONS: [&str; 6] = ["wav", "aif", "aiff", "aifc", "flac", "ogg"];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The outline drawn, and the source and shape it was drawn for.
    outline: Option<((u64, [u32; 9]), cantus_engine::fx::aula_store::Outline)>,
    /// The words over the display and what they were made from.
    words: Option<(WordsKey, String)>,
    /// Where the display was drawn last frame.
    pub display: Option<Rect>,
}

type WordsKey = (u64, u32, usize, crate::i18n::Locale);

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.aula);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    // A file dropped on the window: the response from now on.
    if let Some(path) = dropped(ui) {
        set_file(app, strip, effect, Some(path));
    }
    let out = editor(&mut DawHost::new(app, strip, effect, "aula"), &mut st, ui);
    // The browser's files land on the editor too (`samples::drop`).
    crate::samples::drop::publish(
        ui.ctx(),
        out.body,
        crate::samples::drop::Target::Aula { strip, effect },
    );
    app.fx.aula = st;
    out
}

/// The audio file dropped on the editor this frame, if one was (other files are named
/// in the status line and left).
fn dropped(ui: &egui::Ui) -> Option<String> {
    let files = ui.ctx().input(|i| i.raw.dropped_files.clone());
    let over = ui
        .ctx()
        .pointer_hover_pos()
        .is_none_or(|p| ui.max_rect().contains(p));
    if !over {
        return None;
    }
    files
        .iter()
        .map(|f| f.path().to_string_lossy().to_string())
        .find(|p| !p.is_empty() && is_audio(p))
}

pub fn is_audio(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    EXTENSIONS.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// The pack a file belongs to: one whose folder (its id) is on the file's path.
pub fn pack_of(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    cantus_content::manifest::Manifest::bundled()
        .packs
        .iter()
        .find(|p| parts.iter().any(|c| *c == p.id))
        .map(|p| p.id.clone())
}

/// The credit line of the pack `id`, as the manifest gives it (ADR 0042).
pub fn credit_of(id: &str) -> Option<String> {
    cantus_content::manifest::Manifest::bundled()
        .packs
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| p.credit_line())
}

/// Where a stored path points: itself when absolute, else beside the project file.
pub fn resolve(app: &CantusApp, path: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(path);
    match app.path.as_deref().and_then(|f| f.parent()) {
        Some(dir) if p.is_relative() => dir.join(p),
        _ => p.to_path_buf(),
    }
}

/// Play the response in `path` (`None`: back to Cantus's own), one step of the history.
pub fn set_file(app: &mut CantusApp, strip: StripId, effect: EffectId, path: Option<String>) {
    let ir = path.map(|p| {
        Box::new(IrFile {
            pack: pack_of(&p),
            path: p,
        })
    });
    let name = ir.as_ref().map(|f| f.name().to_string());
    app.exec(Command::SetEffectIr { strip, effect, ir });
    if let Some(name) = name {
        app.set_status(crate::i18n::tf("aula.status.loaded", &[("name", &name)]));
    }
}

/// What a screen reader says of the source list: the response of Cantus's own playing, or
/// that none is because a file is.
pub fn source_words(spec: &cantus_core::EffectSpec) -> String {
    match &spec.ir {
        Some(f) => crate::i18n::tf("aula.source.spoken_file", &[("name", &f.name())]),
        None => {
            let i = (spec.get("ir").round().max(0.0) as usize).min(IR_KEYS.len() - 1);
            crate::i18n::tf(
                "aula.source.spoken_own",
                &[("name", &frame::tr(IR_KEYS[i]))],
            )
        }
    }
}

pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The source card's width at 100 %.
const SOURCE_W: f32 = 300.0;
const LED_W: f32 = 84.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
) {
    let k = s.k;
    let gap = s.d(4.0);
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row2 = controls::card_height(Size::M) * k;
    let row1 = (area.height() - row2 - gap).max(180.0 * k);
    let total = area.width() - gap;
    let sw = SOURCE_W * k;
    let r1: Vec<Rect> = g.row(row1, &[(total - sw) / total, sw / total]).collect();
    let sp = s.function(Function::Space);
    let disp = (r1[0].height() / k - band::TITLE - 1.0).max(20.0);
    controls::card_with(
        ui,
        &sp,
        r1[0],
        Src::Key("aula.card.response"),
        None,
        Some(disp),
        |ui, cr| {
            if let Some(d) = cr.display {
                st.display = Some(d);
                view::display(host, st, ui, &sp, d, tel);
            }
        },
    );
    source_card(host, ui, &s.function(Function::Source), r1[1]);

    let w = [
        controls::cells_width(&[Size::M; 2]) + 4.0 + LED_W + 16.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
        controls::cells_width(&[Size::M; 4]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        &sp,
        r2[0],
        Src::Key("aula.card.shape"),
        None,
        |ui, _| {
            controls::param_with(ui, &sp, host, "stretch", Size::M, Some(&modfx_pct));
            controls::param_with(ui, &sp, host, "length", Size::M, Some(&modfx_pct));
            controls::column(ui, &sp, LED_W, |ui| {
                controls::led_param(ui, &sp, host, "reverse", Src::Key("fx.aula.reverse"), LED_W);
            });
        },
    );
    let sd = s.function(Function::Dynamics);
    controls::card(
        ui,
        &sd,
        r2[1],
        Src::Key("aula.card.envelope"),
        None,
        |ui, _| {
            controls::param(ui, &sd, host, "attack", Size::M);
            controls::param_with(ui, &sd, host, "decay", Size::M, Some(&modfx_pct));
            controls::param_with(ui, &sd, host, "gate", Size::M, Some(&gate_pill));
        },
    );
    let stn = s.function(Function::Tone);
    controls::card(
        ui,
        &stn,
        r2[2],
        Src::Key("aula.card.tone"),
        None,
        |ui, _| {
            controls::param_with(ui, &stn, host, "low", Size::M, Some(&low_pill));
            controls::param_with(ui, &stn, host, "high", Size::M, Some(&high_pill));
            controls::param_with(ui, &stn, host, "damp", Size::M, Some(&modfx_pct));
        },
    );
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[3],
        Src::Key("aula.card.output"),
        None,
        |ui, _| {
            controls::param(ui, &so, host, "predelay", Size::M);
            controls::param_with(ui, &so, host, "width", Size::M, Some(&modfx_pct));
            controls::param_with(ui, &so, host, "mix", Size::M, Some(&modfx_pct));
            controls::param(ui, &so, host, "level", Size::M);
        },
    );
}

fn modfx_pct(v: f32, b: &mut String) {
    super::modfx::pct_pill(v, b);
}

fn gate_pill(v: f32, b: &mut String) {
    if v <= cantus_dsp::aula_shape::GATE_OFF {
        b.push_str(frame::tr("aula.onoff.0"));
    } else {
        let _ = write!(b, "{v:.0} dB");
    }
}

fn low_pill(v: f32, b: &mut String) {
    if v <= cantus_dsp::aula_shape::LOW_OFF * 1.001 {
        b.push_str(frame::tr("aula.onoff.0"));
    } else {
        super::modfx::hz_pill(v, b);
    }
}

fn high_pill(v: f32, b: &mut String) {
    if v >= cantus_dsp::aula_shape::HIGH_OFF * 0.999 {
        b.push_str(frame::tr("aula.onoff.0"));
    } else {
        super::modfx::hz_pill(v, b);
    }
}

/// Where the response comes from: Cantus's own (a list), or a file (in Cantus: load one,
/// see what became of it, its pack's credit, back to Cantus's own).
fn source_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    let inner = SOURCE_W - 16.0;
    controls::card(ui, s, rect, Src::Key("aula.card.source"), None, |ui, _| {
        controls::column(ui, s, inner, |ui| {
            controls::caption(ui, s, Src::Key("aula.source.own"), inner);
            let now = (host.spec().get("ir").round().max(0.0) as usize).min(IR_KEYS.len() - 1);
            let id = ui.id().with("aula-ir");
            let name = frame::tr("fx.aula.ir");
            // A file playing is the source in use: the list says plainly that none of
            // Cantus's own is, to the eye and to a screen reader.
            let file_on = host.spec().ir.is_some();
            let value = if file_on {
                frame::tr("aula.source.none")
            } else {
                frame::tr(IR_KEYS[now])
            };
            let field =
                Rect::from_min_size(ui.cursor().min, egui::vec2(inner * s.k, s.d(controls::ROW)));
            let open = controls::field_key(ui, s, host, "ir", id, value, name, inner);
            let spoken = source_words(host.spec());
            ui.interact(field, id.with("spoken"), egui::Sense::hover())
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &spoken));
            if let Some(at) = open {
                let items: Vec<_> = IR_KEYS
                    .iter()
                    .enumerate()
                    .map(|(i, key)| frame::radio(Src::Key(key), !file_on && i == now))
                    .collect();
                if let Some(i) = controls::list(ui, s, id, at, inner, &items) {
                    host.set("ir", i as f32, None);
                    // Choosing one of Cantus's own is choosing it: a file gives way.
                    if let Some((app, strip, effect)) = host.daw() {
                        if app
                            .project()
                            .effect(strip, effect)
                            .is_some_and(|e| e.spec.ir.is_some())
                        {
                            set_file(app, strip, effect, None);
                        }
                    }
                }
            }
            let Some((app, strip, effect)) = host.daw() else {
                return;
            };
            let file = app
                .project()
                .effect(strip, effect)
                .and_then(|e| e.spec.ir.as_deref().cloned());
            controls::caption(ui, s, Src::Key("aula.source.file"), inner);
            let t = s.t;
            match &file {
                Some(f) => {
                    controls::line(ui, s, f.name(), s.hue, inner);
                    use cantus_engine::fx::aula_store::{status, IrStatus};
                    match status(&f.path) {
                        IrStatus::Failed(why) => {
                            let text = if resolve(app, &f.path).exists() {
                                crate::i18n::tf("aula.failed", &[("why", &why)])
                            } else {
                                crate::i18n::tf("aula.missing", &[("path", &f.path)])
                            };
                            controls::line(ui, s, &text, t.danger, inner);
                        }
                        IrStatus::Loaded {
                            rate,
                            channels,
                            seconds,
                        } => {
                            let fmt = view::format_key(channels);
                            let text = format!(
                                "{} \u{b7} {:.1} kHz \u{b7} {:.2} s",
                                frame::tr(fmt),
                                rate as f32 / 1000.0,
                                seconds
                            );
                            controls::line(ui, s, &text, t.text_dim, inner);
                        }
                        IrStatus::Unknown => {}
                    }
                    if let Some(c) = f.pack.as_deref().and_then(credit_of) {
                        let text = crate::i18n::tf("aula.credit", &[("credit", &c)]);
                        controls::line(ui, s, &text, t.text_dim, inner);
                    }
                }
                None => controls::line(ui, s, frame::tr("aula.drop"), t.text_dim, inner),
            }
            let half = (inner - 4.0) / 2.0;
            ui.horizontal(|ui| {
                let load = controls::key(
                    ui,
                    s,
                    Src::Key("aula.load"),
                    frame::tr("aula.load.tip"),
                    half,
                    false,
                );
                if load.clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Audio", &EXTENSIONS)
                        .pick_file()
                    {
                        set_file(app, strip, effect, Some(p.to_string_lossy().to_string()));
                    }
                }
                let own = ui.add_enabled_ui(file.is_some(), |ui| {
                    controls::key(
                        ui,
                        s,
                        Src::Key("aula.own"),
                        frame::tr("aula.own.tip"),
                        half,
                        false,
                    )
                });
                if own.inner.clicked() {
                    set_file(app, strip, effect, None);
                }
            });
        });
    });
}
