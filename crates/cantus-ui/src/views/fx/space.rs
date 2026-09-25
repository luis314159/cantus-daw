//! Spatium's editor. Owned by Spatium; nothing else edits this file.
//!
//! What the research on the benchmark reverb found missing, and what this does about it
//! (`research/fabfilter/pro-r.md`, "How we beat it"):
//!
//! - **Decay in seconds, not in percent.** The curve display's axis is the RT60 itself;
//!   the drawn curve's bands are dragged in seconds and stored as multipliers. The
//!   octave table under it prints the curve, **what the engine achieves** (its fitted
//!   filters, reported per octave), and **what was measured** on the last tail that went
//!   through it — so a request the filters cannot make is visible, not silent.
//! - **A time axis.** The tail view draws the pre-delay, the early reflections as they
//!   are, the late field's build-up and its decay per band, and the measured envelope of
//!   the last event; pre-delay, the reflections' window and level, and the decay are
//!   handles on it.
//! - **Early reflections you can shape** (level and size), **ducking with real units**
//!   (range, threshold, attack, release) keyed from any strip, and a bypass per curve.
//! - **Tempo-synced pre-delay** computed here from the project's tempo; the engine only
//!   ever sees milliseconds.
//!
//! Every drag and knob turn is one step of the project's undo.

#[path = "space_curve.rs"]
mod curve;
#[path = "space_tail.rs"]
mod tail;

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::fx::space::{decay_key, post_key, DECAY_FIELDS, POST_FIELDS};
use cantus_core::{Command, EffectId, EffectSlot, EffectSpec, MergeKey, StripId};
use cantus_dsp::spatium::fdn::{prime_lengths, Absorber, LateDesign, LINES};
use cantus_dsp::spatium::model::{
    er_level_db, line_lengths_ms, natural_decay, room_at, room_index, sync_ms, Style, SYNC_BEATS,
};
use cantus_dsp::spatium::{mix_gains, SpatiumSettings, OCTAVES};
use cantus_engine::fx::space::{settings, tel};
use egui::{pos2, vec2, Align2, RichText, Sense, Stroke, StrokeKind};
use std::collections::VecDeque;

/// Below this wet correlation the sides cancel audibly in mono: the meter turns red and
/// the width warning shows.
const MONO_TROUBLE: f32 = -0.5;

pub use tail::{Capture, Measured, WF_HOP, WF_ROWS};

/// Which curve the display's gestures and the band card act on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    #[default]
    Decay,
    Post,
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    pub mode: Curve,
    pub selected: Option<(Curve, usize)>,
    drag: Option<curve::Drag>,
    time_drag: Option<tail::Grab>,
    /// The tail view shows the measured spectrum over time instead of the envelope.
    pub tail_spectrum: bool,
    /// The recent audio of the effect, and the last tail measured from it.
    pub capture: Capture,
    /// What the engine would achieve, computed here when no engine reports it (the
    /// screenshot tool, a stopped engine), for the settings it was made for.
    design: Option<(SpatiumSettings, f32, Box<LateDesign>)>,
    absorber: Option<Box<Absorber>>,
    /// Ducking's gain reduction, one value a frame.
    duck: VecDeque<f32>,
}

/// Columns of ducking history kept: at one a frame, about four seconds.
const DUCK_HISTORY: usize = 240;

/// Everything a frame of the editor reads, gathered once.
pub(crate) struct View<'a> {
    pub strip: StripId,
    pub effect: EffectId,
    pub slot: &'a EffectSlot,
    pub s: SpatiumSettings,
    pub tel: cantus_engine::Telemetry,
    /// Whether the telemetry comes from a running engine (or a demo) at all.
    pub alive: bool,
    pub sample_rate: f32,
}

impl View<'_> {
    pub fn spec(&self) -> &EffectSpec {
        &self.slot.spec
    }
}

/// Factory presets: a label key and the parameters it sets (anything unnamed goes back to
/// its default).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "space.preset.vocal_plate",
        &[
            ("style", 1.0),
            ("space", 0.45),
            ("decay", 1.8),
            ("predelay", 30.0),
            ("bright", 0.6),
            ("mix", 0.2),
            ("e0.used", 1.0),
            ("e0.shape", 3.0),
            ("e0.freq", 180.0),
        ],
    ),
    (
        "space.preset.drum_room",
        &[
            ("space", 0.2),
            ("decay", 0.6),
            ("distance", 0.2),
            ("er", 4.0),
            ("character", 0.15),
            ("mix", 0.3),
        ],
    ),
    (
        "space.preset.concert_hall",
        &[
            ("space", 0.64),
            ("decay", 2.4),
            ("predelay", 20.0),
            ("distance", 0.5),
            ("bright", 0.4),
            ("mix", 0.25),
            ("d0.used", 1.0),
            ("d0.shape", 1.0),
            ("d0.freq", 250.0),
            ("d0.mult", 1.3),
        ],
    ),
    (
        "space.preset.cathedral",
        &[
            ("space", 0.9),
            ("decay", 7.0),
            ("predelay", 45.0),
            ("distance", 0.7),
            ("bright", 0.35),
            ("character", 0.45),
            ("mix", 0.3),
        ],
    ),
    (
        "space.preset.ducked_vocal",
        &[
            ("space", 0.55),
            ("decay", 2.2),
            ("predelay", 60.0),
            ("duck", 10.0),
            ("duck_thr", -30.0),
            ("duck_rel", 350.0),
            ("mix", 0.3),
        ],
    ),
    (
        "space.preset.eighties_snare",
        &[
            ("style", 2.0),
            ("space", 0.5),
            ("decay", 1.4),
            ("character", 0.7),
            ("bright", 0.7),
            ("mix", 0.35),
        ],
    ),
    (
        "space.preset.surf_spring",
        &[
            ("style", 3.0),
            ("space", 0.6),
            ("decay", 2.6),
            ("mix", 0.35),
            ("bright", 0.55),
        ],
    ),
    (
        "space.preset.dark_tail",
        &[
            ("space", 0.75),
            ("decay", 4.0),
            ("bright", 0.15),
            ("d0.used", 1.0),
            ("d0.shape", 2.0),
            ("d0.freq", 3_000.0),
            ("d0.mult", 0.5),
            ("mix", 0.25),
        ],
    ),
];

/// A room's name, from its index.
pub fn room_name(index: usize) -> String {
    t(&format!("space.room.{index}"))
}

/// `1.2 s`, `450 ms`.
pub fn seconds(v: f32) -> String {
    if v < 1.0 {
        format!("{:.0} ms", v * 1_000.0)
    } else if v < 10.0 {
        format!("{v:.2} s")
    } else {
        format!("{v:.1} s")
    }
}

impl CantusApp {
    /// Set several of Spatium's parameters as one command; a gesture merges frame to frame
    /// into one undo step.
    pub fn set_space_params(
        &mut self,
        strip: StripId,
        effect: EffectId,
        fields: &[(String, f32)],
        gesture: bool,
    ) {
        let commands: Vec<Command> = fields
            .iter()
            .map(|(k, v)| Command::SetEffectParam {
                strip,
                effect,
                key: k.clone(),
                value: *v,
            })
            .collect();
        let cmd = Command::Batch {
            label_key: "cmd.space_edit".into(),
            commands,
        };
        if gesture {
            let names: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
            self.exec_merging(cmd, MergeKey::of(&("space", effect.0, names)));
        } else {
            self.exec(cmd);
        }
    }
}

/// The window's content under the shared header.
pub fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let s = settings(&slot.spec);
    let tel = app.fx_telemetry();
    let sample_rate = app.engine.sample_rate as f32;
    // What the engine sent since the last frame, into the capture the measured displays
    // read. The dry part is taken back out, so the capture is the reverb alone.
    let (dry, wet) = mix_gains(s.mix);
    let pairs: Vec<(f32, f32)> = app.fx.pairs.drain(..).collect();
    app.fx
        .space
        .capture
        .push(&pairs, sample_rate, slot.mix, dry, wet);
    app.fx.space.capture.analyse();
    let st = &mut app.fx.space;
    if st.duck.len() == DUCK_HISTORY {
        st.duck.pop_front();
    }
    st.duck.push_back(tel[tel::DUCK_DB]);
    let view = View {
        strip,
        effect,
        slot,
        s,
        alive: tel[tel::ALIVE] > 0.5,
        tel,
        sample_rate,
    };
    // The achieved decay when no engine reports it: the same fit the engine runs.
    if !view.alive {
        local_design(&mut app.fx.space, &view.s, sample_rate);
    }

    macro_row(app, ui, &view);
    ui.add_space(space::S1);
    let table_h = space::GRID * 13.0;
    let cards_h = space::GRID * 40.0;
    let display_h = (ui.available_height() - table_h - cards_h).max(space::GRID * 44.0);
    ui.horizontal(|ui| {
        let w = ui.available_width();
        let curve_w = (w * 0.6).floor();
        ui.vertical(|ui| {
            ui.set_width(curve_w);
            curve::display(app, ui, &view, vec2(curve_w, display_h));
            curve::octave_table(app, ui, &view, vec2(curve_w, table_h));
        });
        ui.vertical(|ui| {
            let tw = ui.available_width();
            tail::display(app, ui, &view, vec2(tw, display_h + table_h));
        });
    });
    ui.add_space(space::S1);
    cards(app, ui, &view);
}

/// The fitted design for the settings, kept until they change.
fn local_design(st: &mut State, s: &SpatiumSettings, sample_rate: f32) {
    if st
        .design
        .as_ref()
        .is_some_and(|(k, sr, _)| k == s && *sr == sample_rate)
    {
        return;
    }
    if st
        .absorber
        .as_ref()
        .is_none_or(|a| a.sample_rate() != sample_rate)
    {
        st.absorber = Some(Box::new(Absorber::new(sample_rate)));
    }
    let Some(absorber) = st.absorber.as_ref() else {
        return;
    };
    let room = room_at(s.space);
    let lengths = prime_lengths(&line_lengths_ms::<LINES>(&room, s.style), sample_rate);
    let d = LateDesign::new(absorber, &s.target_curve(), &lengths);
    st.design = Some((*s, sample_rate, Box::new(d)));
}

/// The decay the engine achieves at `hz`: its own report (per octave, interpolated), or
/// the same fit computed here.
pub(crate) fn achieved_t60(st: &State, view: &View, hz: f32) -> Option<f32> {
    if view.s.style == Style::Spring {
        // The spring tank's fit includes its dispersion; the curve is what it is asked.
        return Some(view.s.target_curve().t60(hz));
    }
    if view.alive {
        let v: Vec<f32> = (0..OCTAVES.len())
            .map(|i| view.tel[tel::ACHIEVED + i])
            .collect();
        if v.iter().all(|x| *x > 0.0) {
            let pos = (hz / OCTAVES[0])
                .log2()
                .clamp(0.0, (OCTAVES.len() - 1) as f32);
            let i = (pos.floor() as usize).min(OCTAVES.len() - 2);
            let f = pos - i as f32;
            return Some((v[i].ln() * (1.0 - f) + v[i + 1].ln() * f).exp());
        }
    }
    st.design.as_ref().map(|(_, _, d)| d.achieved_t60(hz))
}

/// A column of fixed width, contents centred: the macro row is a grid, not a flow.
fn column<R>(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        vec2(width, space::GRID * 30.0),
        egui::Layout::top_down(egui::Align::Center),
        add,
    )
    .inner
}

/// One parameter's knob. The house knob prints its label and its value under it, so the
/// state reads at rest; `readout` is that value in the parameter's own words. `also` adds
/// parameters that move with it in the same gesture (Space moves the decay).
fn macro_knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    view: &View,
    key: &str,
    diameter: f32,
    readout: impl Fn(f32) -> String,
    also: impl Fn(f32) -> Vec<(String, f32)>,
) -> egui::Response {
    let Some(info) = view.spec().kind.param(key) else {
        return ui.label(key);
    };
    let mut v = view.spec().get(key);
    let r = ui.add(
        Knob::new(&mut v, info.min..=info.max, info.default)
            .diameter(diameter)
            .log(info.logarithmic)
            .stepped(info.stepped)
            .label(t(info.label_key))
            .format(&readout),
    );
    if r.changed() {
        let mut fields = vec![(key.to_string(), v)];
        fields.extend(also(v));
        app.set_space_params(view.strip, view.effect, &fields, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    r
}

fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

/// The top row: the style, freeze, and the macros — Space the largest, as the hero.
fn macro_row(app: &mut CantusApp, ui: &mut egui::Ui, view: &View) {
    let pal = theme::p();
    let s = view.s;
    ui.horizontal(|ui| {
        // Style and freeze stacked on the left.
        ui.vertical(|ui| {
            ui.label(
                RichText::new(t("fx.space.style"))
                    .text_style(design::caption())
                    .color(pal.text_dim),
            );
            ui.horizontal(|ui| {
                for style in Style::ALL {
                    if ui
                        .selectable_label(
                            s.style == style,
                            t(&format!("space.style.{}", style.index())),
                        )
                        .on_hover_text(t(&format!("space.style.{}_tip", style.index())))
                        .clicked()
                    {
                        app.set_fx_param(
                            view.strip,
                            view.effect,
                            "style",
                            style.index() as f32,
                            false,
                        );
                    }
                }
            });
            ui.add_space(space::S1);
            let mut frozen = s.freeze;
            let label = if frozen {
                RichText::new(t("fx.space.freeze")).color(theme::on_color(&pal, pal.selection))
            } else {
                RichText::new(t("fx.space.freeze"))
            };
            let button = egui::Button::new(label).selected(frozen).fill(if frozen {
                pal.selection
            } else {
                pal.panel_raised
            });
            if ui
                .add(button)
                .on_hover_text(t("space.freeze_tip"))
                .clicked()
            {
                frozen = !frozen;
                app.set_fx_param(
                    view.strip,
                    view.effect,
                    "freeze",
                    if frozen { 1.0 } else { 0.0 },
                    false,
                );
            }
            if view.tel[tel::FREEZE] > 0.0 && view.tel[tel::FREEZE] < 1.0 {
                ui.label(
                    RichText::new(pct(view.tel[tel::FREEZE]))
                        .text_style(design::mono_small())
                        .color(pal.text_dim),
                );
            }
        });
        ui.separator();
        let big = space::GRID * 20.0;
        let k = space::GRID * 12.0;
        let col = space::GRID * 20.0;
        // Space: moving it moves the decay with it, keeping the ratio to the room's own
        // decay — the benchmark's hero control, one gesture and one undo step.
        let old_space = s.space;
        let old_decay = s.decay_s;
        column(ui, big + space::S8, |ui| {
            macro_knob(
                app,
                ui,
                view,
                "space",
                big,
                |v| room_name(room_index(v)),
                move |v| {
                    let ratio = old_decay / natural_decay(old_space);
                    vec![(
                        "decay".to_string(),
                        (natural_decay(v) * ratio).clamp(0.1, 30.0),
                    )]
                },
            );
        });
        column(ui, col, |ui| {
            macro_knob(app, ui, view, "decay", k, seconds, |_| Vec::new());
            // The room's own decay, for reference: Space suggests it.
            ui.label(
                RichText::new(tf(
                    "space.natural",
                    &[("s", &seconds(natural_decay(s.space)))],
                ))
                .text_style(design::caption())
                .color(pal.text_dim),
            );
        });
        column(ui, col, |ui| predelay(app, ui, view, k));
        for key in ["distance", "character", "bright", "width", "mix"] {
            column(ui, col, |ui| {
                macro_knob(app, ui, view, key, k, pct, |_| Vec::new());
            });
        }
    });
}

/// Pre-delay: a knob in milliseconds, and under it the note value it follows, if any.
/// Choosing a note value writes its length at the project's tempo; turning the knob
/// frees it again. If the tempo has moved since, the stale value says so and offers to
/// follow.
fn predelay(app: &mut CantusApp, ui: &mut egui::Ui, view: &View, k: f32) {
    let pal = theme::p();
    let bpm = app.project().tempo_bpm;
    let sync = (view.spec().get("sync").round().max(0.0) as usize).min(SYNC_BEATS.len() - 1);
    {
        macro_knob(
            app,
            ui,
            view,
            "predelay",
            k,
            |v| crate::widgets::knob::readout(v, "ms"),
            |_| vec![("sync".to_string(), 0.0)],
        );
        egui::ComboBox::from_id_salt(("space-sync", view.effect.0))
            .selected_text(t(&format!("space.sync.{sync}")))
            .width(space::GRID * 16.0)
            .show_ui(ui, |ui| {
                for i in 0..SYNC_BEATS.len() {
                    let text = if i == 0 {
                        t("space.sync.0")
                    } else {
                        format!(
                            "{}  {}",
                            t(&format!("space.sync.{i}")),
                            crate::widgets::knob::readout(sync_ms(i, bpm).min(500.0), "ms")
                        )
                    };
                    if ui.selectable_label(sync == i, text).clicked() {
                        let mut fields = vec![("sync".to_string(), i as f32)];
                        if i > 0 {
                            fields.push(("predelay".to_string(), sync_ms(i, bpm).min(500.0)));
                        }
                        app.set_space_params(view.strip, view.effect, &fields, false);
                    }
                }
            })
            .response
            .on_hover_text(tf("space.sync_tip", &[("bpm", &format!("{bpm:.0}"))]));
        if sync > 0 {
            let want = sync_ms(sync, bpm).min(500.0);
            if (want - view.s.predelay_ms).abs() > 0.5
                && ui
                    .small_button(
                        RichText::new(tf("space.resync", &[("bpm", &format!("{bpm:.0}"))]))
                            .color(pal.gold),
                    )
                    .clicked()
            {
                app.set_fx_param(view.strip, view.effect, "predelay", want, false);
            }
        }
    }
}

/// The row of cards under the displays: the selected band, the early reflections, the
/// ducking, and the output.
fn cards(app: &mut CantusApp, ui: &mut egui::Ui, view: &View) {
    ui.horizontal_top(|ui| {
        let w = ui.available_width();
        let gap = space::S2;
        let widths = [w * 0.29, w * 0.17, w * 0.33, w * 0.21 - 3.0 * gap];
        card(ui, widths[0], |ui| band_card(app, ui, view));
        ui.add_space(gap);
        card(ui, widths[1], |ui| early_card(app, ui, view));
        ui.add_space(gap);
        card(ui, widths[2], |ui| duck_card(app, ui, view));
        ui.add_space(gap);
        card(ui, widths[3], |ui| output_card(app, ui, view));
    });
}

fn card(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    let pal = theme::p();
    ui.allocate_ui_with_layout(
        vec2(width, space::GRID * 38.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            design::Elevation::Card.frame(&pal).show(ui, |ui| {
                ui.set_width(width - space::S4);
                ui.set_min_height(space::GRID * 34.0);
                add(ui);
            });
        },
    );
}

fn card_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .text_style(design::strong())
            .color(theme::p().text),
    );
}

/// A smaller knob bound to one parameter, value printed under it.
fn small_knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    view: &View,
    key: &str,
    readout: impl Fn(f32) -> String,
) {
    macro_knob(app, ui, view, key, space::GRID * 9.0, readout, |_| {
        Vec::new()
    });
}

fn early_card(app: &mut CantusApp, ui: &mut egui::Ui, view: &View) {
    let pal = theme::p();
    card_title(ui, &t("space.early"));
    let room = room_at(view.s.space);
    let window = room.er_end_ms;
    ui.horizontal(|ui| {
        small_knob(app, ui, view, "er", |v| {
            if v <= -30.0 {
                t("space.off")
            } else {
                format!("{v:+.1} dB")
            }
        });
        small_knob(app, ui, view, "er_size", move |v| {
            crate::widgets::knob::readout(window * v, "ms")
        });
    });
    let level = er_level_db(view.s.style, view.s.distance, view.s.er_db);
    let text = match level {
        Some(db) if view.s.style != Style::Spring => tf(
            "space.early_now",
            &[
                ("db", &format!("{db:+.1}")),
                ("ms", &format!("{:.0}", room.er_end_ms * view.s.er_size)),
            ],
        ),
        _ => t("space.early_off"),
    };
    ui.label(
        RichText::new(text)
            .text_style(design::caption())
            .color(pal.text_dim),
    );
}

/// A band's number as the display draws it: decay bands 1–6, post-EQ bands A–F.
pub fn band_label(curve: Curve, n: usize) -> String {
    match curve {
        Curve::Decay => (n + 1).to_string(),
        Curve::Post => ((b'A' + n as u8) as char).to_string(),
    }
}

/// The key of a band's field on either curve.
pub fn band_key(curve: Curve, n: usize, field: &str) -> String {
    match curve {
        Curve::Decay => decay_key(n, field),
        Curve::Post => post_key(n, field),
    }
}

/// Whether a band exists (has been made and not removed).
pub fn band_used(spec: &EffectSpec, curve: Curve, n: usize) -> bool {
    spec.get(&band_key(curve, n, "used")) >= 0.5
}

impl CantusApp {
    /// Remove a band: every field it stored goes, so its number is free and the file
    /// forgets it. One undo step.
    pub fn remove_space_band(&mut self, strip: StripId, effect: EffectId, curve: Curve, n: usize) {
        let fields = match curve {
            Curve::Decay => DECAY_FIELDS,
            Curve::Post => POST_FIELDS,
        };
        let commands = fields
            .iter()
            .map(|f| Command::ClearEffectParam {
                strip,
                effect,
                key: band_key(curve, n, f.0),
            })
            .collect();
        self.exec(Command::Batch {
            label_key: "cmd.space_band_remove".into(),
            commands,
        });
        if self.fx.space.selected == Some((curve, n)) {
            self.fx.space.selected = None;
        }
    }
}

fn band_card(app: &mut CantusApp, ui: &mut egui::Ui, view: &View) {
    let pal = theme::p();
    let Some((curve, n)) = app
        .fx
        .space
        .selected
        .filter(|(c, n)| band_used(view.spec(), *c, *n))
    else {
        card_title(ui, &t("space.band"));
        ui.label(
            RichText::new(t("space.band_hint"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        return;
    };
    let key = |f: &str| band_key(curve, n, f);
    let shape = view.spec().get(&key("shape")).round() as usize;
    ui.horizontal(|ui| {
        let (chip, _) = ui.allocate_exact_size(vec2(space::S3, space::S3), Sense::hover());
        ui.painter().circle_filled(
            chip.center(),
            chip.width() * 0.5,
            curve::band_color(curve, n),
        );
        let title = match curve {
            Curve::Decay => tf("space.decay_band_n", &[("n", &band_label(curve, n))]),
            Curve::Post => tf("space.post_band_n", &[("n", &band_label(curve, n))]),
        };
        card_title(ui, &title);
        let shapes = match curve {
            Curve::Decay => 4,
            Curve::Post => 5,
        };
        let prefix = match curve {
            Curve::Decay => "space.dshape",
            Curve::Post => "space.pshape",
        };
        egui::ComboBox::from_id_salt(("space-shape", view.effect.0, curve == Curve::Post, n))
            .selected_text(t(&format!("{prefix}.{shape}")))
            .width(space::GRID * 22.0)
            .show_ui(ui, |ui| {
                for i in 0..shapes {
                    if ui
                        .selectable_label(shape == i, t(&format!("{prefix}.{i}")))
                        .clicked()
                    {
                        app.set_space_params(
                            view.strip,
                            view.effect,
                            &[(key("shape"), i as f32)],
                            false,
                        );
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        let mut on = view.spec().get(&key("on")) >= 0.5;
        if ui.checkbox(&mut on, t("fx.space.band.on")).changed() {
            app.set_space_params(
                view.strip,
                view.effect,
                &[(key("on"), if on { 1.0 } else { 0.0 })],
                false,
            );
        }
        if curve == Curve::Post {
            let place = view.spec().get(&key("place")).round() as usize;
            egui::ComboBox::from_id_salt(("space-place", view.effect.0, n))
                .selected_text(t(&format!("eq.place.{place}")))
                .width(space::GRID * 18.0)
                .show_ui(ui, |ui| {
                    for i in 0..5 {
                        if ui
                            .selectable_label(place == i, t(&format!("eq.place.{i}")))
                            .clicked()
                        {
                            app.set_space_params(
                                view.strip,
                                view.effect,
                                &[(key("place"), i as f32)],
                                false,
                            );
                        }
                    }
                });
        }
        if ui.small_button(t("space.remove_band")).clicked() {
            app.remove_space_band(view.strip, view.effect, curve, n);
        }
    });
    let curve_now = view.s.target_curve();
    let freq = view.spec().get(&key("freq"));
    if curve == Curve::Decay {
        // The multiplier in the units that matter: what the decay is there, and was.
        ui.label(
            RichText::new(tf(
                "space.band_now",
                &[
                    ("hz", &crate::widgets::knob::readout(freq, "Hz")),
                    ("s", &seconds(curve_now.t60(freq))),
                    ("base", &seconds(curve_now.base_t60(freq))),
                ],
            ))
            .text_style(design::caption())
            .color(pal.text_dim),
        );
    }
    ui.horizontal(|ui| {
        small_knob(app, ui, view, &key("freq"), |v| {
            crate::widgets::knob::readout(v, "Hz")
        });
        match curve {
            Curve::Decay if shape != 3 => {
                small_knob(app, ui, view, &key("mult"), |v| format!("\u{d7}{v:.2}"));
            }
            Curve::Post if shape < 3 => {
                small_knob(app, ui, view, &key("gain"), |v| format!("{v:+.1} dB"));
            }
            _ => {}
        }
        if curve == Curve::Post && shape >= 3 {
            small_knob(app, ui, view, &key("slope"), |v| format!("{v:.0} dB/oct"));
        } else {
            small_knob(app, ui, view, &key("q"), |v| format!("Q {v:.2}"));
        }
    });
}

fn duck_card(app: &mut CantusApp, ui: &mut egui::Ui, view: &View) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        card_title(ui, &t("space.ducking"));
        let current = view.slot.sidechain;
        let label = match current {
            Some(k) => app.capture_label(cantus_engine::CaptureTarget::Strip(k)),
            None => t("comp.key_own"),
        };
        egui::ComboBox::from_id_salt(("space-key", view.effect.0))
            .selected_text(label)
            .width(space::GRID * 30.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_none(), t("comp.key_own"))
                    .clicked()
                {
                    app.exec(Command::SetEffectSidechain {
                        strip: view.strip,
                        effect: view.effect,
                        source: None,
                    });
                }
                let options: Vec<StripId> = app
                    .project()
                    .strips
                    .iter()
                    .map(|x| x.id)
                    .filter(|id| {
                        *id != view.strip
                            && !app.project().routing_would_cycle(*id, Some(view.strip))
                    })
                    .collect();
                for id in options {
                    let text = app.capture_label(cantus_engine::CaptureTarget::Strip(id));
                    if ui.selectable_label(current == Some(id), text).clicked() {
                        app.exec(Command::SetEffectSidechain {
                            strip: view.strip,
                            effect: view.effect,
                            source: Some(id),
                        });
                    }
                }
            })
            .response
            .on_hover_text(t("space.key_tip"));
    });
    ui.horizontal(|ui| {
        small_knob(app, ui, view, "duck", |v| {
            if v <= 0.0 {
                t("space.off")
            } else {
                format!("{v:.1} dB")
            }
        });
        small_knob(app, ui, view, "duck_thr", |v| format!("{v:.1} dB"));
        small_knob(app, ui, view, "duck_att", |v| {
            crate::widgets::knob::readout(v, "ms")
        });
        small_knob(app, ui, view, "duck_rel", |v| {
            crate::widgets::knob::readout(v, "ms")
        });
    });
    {
        // Gain reduction: the last seconds, hanging from the top, and the number.
        let (rect, _) = ui.allocate_exact_size(
            vec2(
                ui.available_width().max(space::GRID * 12.0),
                space::GRID * 5.0,
            ),
            Sense::hover(),
        );
        let p = ui.painter_at(rect);
        p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
        let hist = &app.fx.space.duck;
        let range = view.s.duck_db.max(6.0);
        let dx = rect.width() / DUCK_HISTORY as f32;
        let x0 = rect.right() - dx * hist.len() as f32;
        let mut mesh = egui::Mesh::default();
        let fill = pal.danger.gamma_multiply(0.55);
        for (i, gr) in hist.iter().enumerate() {
            let x = x0 + dx * i as f32;
            let y = rect.top() + (gr / range).clamp(0.0, 1.0) * rect.height();
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(pos2(x, rect.top()), fill);
            mesh.colored_vertex(pos2(x, y), fill);
            if i > 0 {
                mesh.add_triangle(base - 2, base - 1, base);
                mesh.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(mesh);
        p.text(
            rect.right_center() + vec2(-space::S1, 0.0),
            Align2::RIGHT_CENTER,
            tf(
                "space.gr",
                &[("db", &format!("{:.1}", view.tel[tel::DUCK_DB]))],
            ),
            design::paint::ruler(),
            pal.text,
        );
        p.rect_stroke(
            rect,
            pal.corner_radius,
            Stroke::new(1.0, pal.grid_strong),
            StrokeKind::Inside,
        );
    }
}

fn output_card(_app: &mut CantusApp, ui: &mut egui::Ui, view: &View) {
    let pal = theme::p();
    card_title(ui, &t("space.output"));
    let bar = |ui: &mut egui::Ui, label: &str, frac: f32, color: egui::Color32, text: String| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(label)
                    .text_style(design::mono_small())
                    .color(pal.text_dim),
            );
            let (rect, _) = ui.allocate_exact_size(
                vec2(
                    (ui.available_width() - space::GRID * 13.0).max(space::GRID * 6.0),
                    space::S3,
                ),
                Sense::hover(),
            );
            let p = ui.painter_at(rect);
            p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
            let mut fill = rect;
            fill.set_width(rect.width() * frac.clamp(0.0, 1.0));
            p.rect_filled(fill, pal.corner_radius, color);
            // Scale ticks every 12 dB, so the colour is not the only cue.
            for k in 1..4 {
                let x = rect.left() + rect.width() * k as f32 / 4.0;
                p.vline(x, rect.y_range(), Stroke::new(1.0, pal.grid_strong));
            }
            ui.label(
                RichText::new(text)
                    .text_style(design::mono_small())
                    .color(pal.text),
            );
        });
    };
    let db = |x: f32| if x > 1e-6 { 20.0 * x.log10() } else { -120.0 };
    for (label, i) in [("L", tel::WET_L), ("R", tel::WET_R)] {
        let d = db(view.tel[i]);
        bar(
            ui,
            label,
            (d + 48.0) / 48.0,
            theme::meter_color(view.tel[i]),
            if d > -100.0 {
                format!("{d:5.1}")
            } else {
                "  -\u{221e}".into()
            },
        );
    }
    let rho = view.tel[tel::CORRELATION];
    bar(
        ui,
        &t("space.corr"),
        (rho + 1.0) * 0.5,
        // A decorrelated tail swings either side of zero over a 300 ms window; only a
        // reading well below it means the sides are fighting in mono.
        if rho < MONO_TROUBLE {
            pal.danger
        } else if rho < 0.0 {
            pal.meter_high
        } else {
            pal.ok
        },
        format!("{rho:+.2}"),
    );
    if view.s.width > 1.0 && rho < MONO_TROUBLE {
        ui.label(
            RichText::new(t("space.mono_warning"))
                .text_style(design::caption())
                .color(pal.danger),
        );
    } else {
        ui.label(
            RichText::new(t("space.latency_none"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
    }
}

/// The inspector's picture: the decay curve in seconds across the spectrum, and the room,
/// the decay and the pre-delay in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let pal = theme::p();
    let s = settings(spec);
    let curve = s.target_curve();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), space::GRID * 14.0),
        Sense::click(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let plot = rect.shrink2(vec2(space::S2, space::S2));
    let (lo, hi) = curve::axis_seconds(s.decay_s);
    let x = |hz: f32| plot.left() + (hz / 20.0).log(1_000.0) * plot.width();
    let y = |sec: f32| plot.bottom() - (sec / lo).ln() / (hi / lo).ln() * plot.height();
    p.hline(
        plot.x_range(),
        y(s.decay_s),
        Stroke::new(1.0, pal.grid_strong),
    );
    let pts: Vec<egui::Pos2> = (0..=80)
        .map(|i| {
            let hz = 20.0 * 1_000f32.powf(i as f32 / 80.0);
            pos2(x(hz), y(curve.t60(hz).clamp(lo, hi)))
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.5, pal.gold)));
    p.text(
        rect.left_top() + vec2(space::S2, space::S1),
        Align2::LEFT_TOP,
        format!(
            "{} \u{b7} {} \u{b7} {}",
            room_name(room_index(s.space)),
            seconds(s.decay_s),
            crate::widgets::knob::readout(s.predelay_ms, "ms")
        ),
        design::paint::ruler(),
        pal.text,
    );
    if s.freeze {
        p.text(
            rect.right_top() + vec2(-space::S2, space::S1),
            Align2::RIGHT_TOP,
            t("fx.space.freeze"),
            design::paint::ruler(),
            pal.selection,
        );
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(
            1.0,
            if resp.hovered() {
                pal.gold
            } else {
                pal.grid_strong
            },
        ),
        StrokeKind::Inside,
    );
    resp.on_hover_text(t("space.open_tip"))
}

/// The settings `--editor space` and the screenshot tool open it with: a hall with a
/// drawn curve, a post EQ, a synced pre-delay and some ducking.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let bpm = app.project().tempo_bpm;
    let fields: Vec<(String, f32)> = [
        ("space", 0.62),
        ("decay", 2.4),
        ("sync", 4.0),
        ("predelay", sync_ms(4, bpm).min(500.0)),
        ("distance", 0.45),
        ("character", 0.35),
        ("bright", 0.4),
        ("mix", 0.3),
        ("duck", 6.0),
        ("duck_thr", -24.0),
        ("duck_rel", 150.0),
        ("d0.used", 1.0),
        ("d0.shape", 1.0),
        ("d0.freq", 220.0),
        ("d0.mult", 1.45),
        ("d1.used", 1.0),
        ("d1.freq", 2_600.0),
        ("d1.mult", 0.55),
        ("d1.q", 1.1),
        ("e0.used", 1.0),
        ("e0.shape", 3.0),
        ("e0.freq", 140.0),
        ("e1.used", 1.0),
        ("e1.shape", 2.0),
        ("e1.freq", 7_500.0),
        ("e1.gain", -4.0),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect();
    app.set_space_params(strip, effect, &fields, false);
    app.fx.space.selected = Some((Curve::Decay, 1));
    app.fx.space.mode = Curve::Decay;
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// DSP run offline over a snare-like hit and its tail, as (input, output) pairs, and its
/// telemetry.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(slot) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .cloned()
    else {
        return;
    };
    use cantus_engine::FxUnit;
    let fs = 48_000.0f32;
    let mut unit = cantus_engine::fx::space::Unit::new(fs);
    unit.configure(&slot.spec);
    unit.reset();
    // A hall longer than the measure waits for is fed past its four seconds, so the
    // picture shows a cut tail (`shoot … space-cut`).
    let seconds = if settings(&slot.spec).decay_s > 4.0 {
        4.6
    } else {
        3.2
    };
    let n = (seconds * fs) as usize;
    let mut seed = 11u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let hit = |t: f32, noise: f32| -> f32 {
        if !(0.0..0.25).contains(&t) {
            return 0.0;
        }
        let body = (t * 190.0 * std::f32::consts::TAU).sin() * (-t / 0.05).exp();
        0.5 * body + 0.45 * noise * (-t / 0.07).exp()
    };
    let input: Vec<f32> = (0..n).map(|i| hit(i as f32 / fs - 0.05, noise())).collect();
    let mut pairs = Vec::with_capacity(n);
    let mut telemetry = None;
    let block = 800;
    for (b, chunk) in input.chunks(block).enumerate() {
        let mut l = chunk.to_vec();
        let mut r = chunk.to_vec();
        unit.process(&mut l, &mut r, None);
        for (i, x) in chunk.iter().enumerate() {
            pairs.push((*x, 0.5 * (l[i] + r[i])));
        }
        let t_now = (b * block) as f32 / fs;
        let st = &mut app.fx.space;
        if t_now < 4.0 {
            if st.duck.len() == DUCK_HISTORY {
                st.duck.pop_front();
            }
            st.duck.push_back(unit.telemetry()[tel::DUCK_DB]);
        }
        if telemetry.is_none() && t_now >= 0.35 {
            telemetry = Some(unit.telemetry());
        }
    }
    let s = settings(&slot.spec);
    let (dry, wet) = mix_gains(s.mix);
    let cap = &mut app.fx.space.capture;
    cap.push(&pairs, fs, slot.mix, dry, wet);
    cap.analyse();
    app.fx.injected = telemetry;
}
