//! Porta's editor: the gate's own window, where what you see is what you drag.
//!
//! The benchmark gate's display is read-only, its hysteresis is fixed per style, its
//! styles are named but not explained, and you cannot hear what it takes away
//! (`research/fabfilter/pro-g.md`, "How we beat it"). So here:
//!
//! - **The level history is the control.** The open threshold is a solid line and the
//!   close threshold a dashed one, both dragged on the history (Alt moves the open line
//!   alone, leaving the close line where it is); the band between them *is* the
//!   hysteresis. Under the history, a lane shows the gate's phase — open, hold, release,
//!   closed — by colour *and* pattern, so chatter is visible as flicker. The gain change
//!   hangs from the top. When the song plays, bar lines run through it.
//! - **The transfer curve is the control too**: drag the open and close points, the slope
//!   below the threshold (the ratio) and the floor (the range).
//! - **The key's filters are dragged on a spectrum** of what comes in, with the response
//!   they make drawn over it.
//! - **Every style says what it is**, in words, under the buttons.
//! - **Hear the gate, the key, or what the gate removes** — the last is what the benchmark
//!   cannot do.
//! - **The host's advantage**: *Learn* sets the thresholds from a passage as it plays; *Hits*
//!   turns the gate's openings into a MIDI clip on a new drum track, as one undo step.
//!
//! Every drag and knob turn is one step of the project's undo.

use super::host::{DawHost, FxHost, KeySource, KeyTarget};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::{
    Command, EffectId, EffectSlot, EffectSpec, InstrumentSpec, PulsusParams, StripId, Ticks,
    TrackId,
};
use cantus_dsp::porta::{self, telemetry, Column, GateSettings, Mode, Phase, Style};
use egui::{
    pos2, vec2, Align2, Color32, Mesh, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
};
use std::collections::VecDeque;

/// The level scale of the history and the transfer curve, dB.
const LO_DB: f32 = -80.0;
/// Columns of history kept: at 5 ms each, four seconds.
pub const HISTORY: usize = 800;
/// The key strip's frequency axis.
const F_LO: f32 = 20.0;
const F_HI: f32 = 20_000.0;

/// Factory presets: a label key and the parameters it sets (`views::fx::presets_menu`).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.gate.kick",
        &[
            ("thr", -30.0),
            ("hyst", 6.0),
            ("range", 40.0),
            ("att", 0.5),
            ("hold", 40.0),
            ("rel", 120.0),
            ("look", 2.0),
            ("lpf", 150.0),
        ],
    ),
    (
        "preset.gate.snare",
        &[
            ("thr", -28.0),
            ("hyst", 5.0),
            ("range", 30.0),
            ("att", 0.3),
            ("hold", 50.0),
            ("rel", 180.0),
            ("look", 2.0),
            ("hpf", 150.0),
            ("lpf", 8_000.0),
        ],
    ),
    (
        "preset.gate.toms",
        &[
            ("thr", -32.0),
            ("hyst", 8.0),
            ("range", 24.0),
            ("att", 1.0),
            ("hold", 80.0),
            ("rel", 350.0),
            ("look", 3.0),
            ("style", 1.0),
            ("hpf", 60.0),
            ("lpf", 1_200.0),
        ],
    ),
    (
        "preset.gate.hats",
        &[
            ("thr", -35.0),
            ("hyst", 4.0),
            ("range", 18.0),
            ("att", 0.2),
            ("hold", 15.0),
            ("rel", 60.0),
            ("look", 1.0),
            ("style", 1.0),
            ("hpf", 5_000.0),
            ("key_slope", 48.0),
        ],
    ),
    (
        "preset.gate.vocal",
        &[
            ("thr", -45.0),
            ("hyst", 6.0),
            ("ratio", 3.0),
            ("range", 12.0),
            ("att", 5.0),
            ("hold", 60.0),
            ("rel", 250.0),
            ("look", 5.0),
            ("style", 2.0),
            ("hpf", 100.0),
        ],
    ),
    (
        "preset.gate.guitar",
        &[
            ("thr", -50.0),
            ("hyst", 4.0),
            ("ratio", 4.0),
            ("range", 30.0),
            ("att", 2.0),
            ("hold", 30.0),
            ("rel", 200.0),
            ("style", 3.0),
            ("hpf", 80.0),
            ("lpf", 6_000.0),
        ],
    ),
    (
        "preset.gate.noise",
        &[
            ("thr", -60.0),
            ("hyst", 6.0),
            ("ratio", 2.0),
            ("range", 12.0),
            ("att", 5.0),
            ("hold", 100.0),
            ("rel", 400.0),
            ("style", 1.0),
        ],
    ),
    (
        "preset.gate.duck",
        &[
            ("thr", -30.0),
            ("range", 10.0),
            ("att", 10.0),
            ("hold", 50.0),
            ("rel", 250.0),
            ("look", 5.0),
            ("style", 5.0),
        ],
    ),
    (
        "preset.gate.punch",
        &[
            ("thr", -20.0),
            ("ratio", 1.5),
            ("range", 6.0),
            ("att", 1.0),
            ("hold", 0.0),
            ("rel", 80.0),
            ("style", 4.0),
        ],
    ),
];

/// One column of the history, and where the song was when it was made (`None` while
/// stopped), for the bar lines.
#[derive(Copy, Clone, Debug)]
pub struct Col {
    pub c: Column,
    pub tick: Option<i64>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    /// The open threshold (Alt: alone, the close line staying put).
    Open {
        alone: bool,
    },
    Close,
    Ratio,
    Range,
    Hpf,
    Lpf,
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// Oldest first.
    pub history: VecDeque<Col>,
    /// The effect the history belongs to.
    effect: Option<EffectId>,
    last_seq: Option<f32>,
    last_hits: Option<f32>,
    drag: Option<Grab>,
    /// Key levels gathered while learning, one per column.
    pub learning: Option<Vec<f32>>,
    /// Openings become hits while the song plays.
    pub capturing: bool,
    /// Captured hits: song position (ticks) and the key's peak, dB.
    pub hits: Vec<(i64, f32)>,
    /// The kit pad the hits are written to.
    pub hit_pitch: Option<u8>,
    spectrum: Option<cantus_dsp::SpectrumAnalyser>,
    /// Where the last frame drew the history's plot and the transfer curve, and the dB
    /// range both span — published so a test can aim a drag at a line.
    pub geometry: Option<Geometry>,
}

/// The displays as the last frame laid them out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The history's plot (without the phase lane): `LO_DB` at the bottom, 0 dB at the top.
    pub history: Rect,
    /// The transfer curve: `LO_DB` … +12 dB on both axes.
    pub transfer: Rect,
}

impl Geometry {
    /// The height of a level on the history.
    pub fn history_y(&self, db: f32) -> f32 {
        self.history.bottom() - ((db - LO_DB) / -LO_DB).clamp(0.0, 1.0) * self.history.height()
    }

    /// Where a level pair lands on the transfer curve.
    pub fn transfer_pos(&self, input_db: f32, output_db: f32) -> Pos2 {
        let span = 12.0 - LO_DB;
        let r = self.transfer;
        pos2(
            r.left() + ((input_db - LO_DB) / span).clamp(0.0, 1.0) * r.width(),
            r.bottom() - ((output_db - LO_DB) / span).clamp(0.0, 1.0) * r.height(),
        )
    }
}

impl State {
    /// Take what the telemetry brought since the last frame: new columns into the
    /// history (with the song position, if playing), new hits into the capture.
    /// `now` is the song position at the end of the last block, `ticks_per_second` the
    /// tempo in ticks; both `None`/ignored while stopped.
    pub fn ingest(&mut self, t: &[f32; 64], now: Option<i64>, ticks_per_second: f64) {
        use telemetry::*;
        let new = match self.last_seq {
            Some(last) => since(t[SEQ], last),
            // Opening on a running gate: what the telemetry holds, up to its 24 columns.
            None => t[SEQ].max(0.0) as u32,
        }
        .min(porta::HISTORY_COLUMNS as u32) as usize;
        self.last_seq = Some(t[SEQ]);
        let col_ticks = (t[COLUMN_MS].max(0.0) as f64 * 0.001 * ticks_per_second).round() as i64;
        for j in porta::HISTORY_COLUMNS - new..porta::HISTORY_COLUMNS {
            let c = unpack(t[COLUMNS_AT + 2 * j], t[COLUMNS_AT + 2 * j + 1]);
            let back = (porta::HISTORY_COLUMNS - 1 - j) as i64;
            let tick = now.map(|n| n - back * col_ticks);
            if let Some(levels) = self.learning.as_mut() {
                levels.push(c.key_db);
            }
            if self.history.len() == HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(Col { c, tick });
        }
        let hits = match self.last_hits {
            Some(last) => since(t[HITS], last),
            None => 0,
        }
        .min(porta::EVENTS as u32) as usize;
        self.last_hits = Some(t[HITS]);
        if let (true, Some(now)) = (self.capturing, now) {
            let sr = t[SAMPLE_RATE].max(1.0) as f64;
            for j in (0..hits).rev() {
                let ago = t[EVENTS_AT + 2 * j] as f64;
                let tick = now - (ago / sr * ticks_per_second).round() as i64;
                self.hits.push((tick.max(0), t[EVENTS_AT + 2 * j + 1]));
            }
        }
    }
}

/// Velocities for hits by their peaks: the loudest is 127, one 40 dB softer is 1, linear in
/// dB between — the dynamics of the playing, kept.
pub fn velocities(peaks_db: &[f32]) -> Vec<u8> {
    let top = peaks_db.iter().copied().fold(f32::MIN, f32::max);
    peaks_db
        .iter()
        .map(|p| (1.0 + 126.0 * ((p - (top - 40.0)) / 40.0).clamp(0.0, 1.0)).round() as u8)
        .collect()
}

/// Hits sorted, with those closer than 15 ms (a loop played twice, a flam counted twice)
/// merged into the louder.
pub fn merge_hits(hits: &[(i64, f32)], ticks_per_second: f64) -> Vec<(i64, f32)> {
    let mut v = hits.to_vec();
    v.sort_by_key(|h| h.0);
    let gap = (0.015 * ticks_per_second).round() as i64;
    let mut out: Vec<(i64, f32)> = Vec::with_capacity(v.len());
    for h in v {
        match out.last_mut() {
            Some(last) if h.0 - last.0 < gap => last.1 = last.1.max(h.1),
            _ => out.push(h),
        }
    }
    out
}

fn ticks_per_second(app: &CantusApp) -> f64 {
    app.project().tempo_bpm / 60.0 * cantus_core::PPQ as f64
}

/// The pad the hits are written to: the chosen one, or the kit's first.
fn hit_pitch(st: &State) -> u8 {
    let kit = PulsusParams::default();
    st.hit_pitch
        .or_else(|| kit.pads.first().map(|p| p.pitch))
        .unwrap_or(36)
}

/// The captured hits as notes: merged, stamped in ticks, with velocities from their peaks.
fn hit_notes(st: &State, ticks_per_second: f64) -> Vec<(i64, u8)> {
    let hits = merge_hits(&st.hits, ticks_per_second);
    let peaks: Vec<f32> = hits.iter().map(|h| h.1).collect();
    hits.iter().map(|h| h.0).zip(velocities(&peaks)).collect()
}

/// Stop learning and set the thresholds from what was heard, as one step. Returns what
/// was learned, or `None` if the passage was too short or silent.
pub fn finish_learning(host: &mut dyn FxHost, st: &mut State) -> Option<porta::Learned> {
    let levels = st.learning.take()?;
    let hyst = host.spec().get("hyst");
    let Some(learned) = porta::learn_thresholds(&levels, hyst) else {
        host.status(t("gate.learn_none"));
        return None;
    };
    let hyst = (learned.open_db - learned.close_db).clamp(0.0, 24.0);
    host.set_many(
        &[("thr", learned.open_db), ("hyst", hyst)],
        "cmd.gate_learn",
    );
    host.status(tf(
        "gate.learned",
        &[
            ("open", &format!("{:.1}", learned.open_db)),
            ("close", &format!("{:.1}", learned.open_db - hyst)),
        ],
    ));
    Some(learned)
}

/// The captured hits into a MIDI clip, where the host can make one; cleared once written.
pub fn hits_to_clip(host: &mut dyn FxHost, st: &mut State) -> bool {
    let notes = hit_notes(st, host.transport().ticks_per_second);
    if notes.is_empty() {
        return false;
    }
    let ok = host.notes_to_clip(&notes, hit_pitch(st)) == Some(true);
    if ok {
        st.hits.clear();
    }
    ok
}

impl CantusApp {
    /// Stop learning and set the thresholds from what was heard, as one undo step.
    /// Returns what was learned, or `None` if the passage was too short or silent.
    pub fn finish_gate_learning(
        &mut self,
        strip: StripId,
        effect: EffectId,
    ) -> Option<porta::Learned> {
        let mut st = std::mem::take(&mut self.fx.gate);
        let learned = finish_learning(&mut DawHost::new(self, strip, effect, "gate"), &mut st);
        self.fx.gate = st;
        learned
    }

    /// The captured hits as a MIDI clip on a new drum track (Pulsus, the shipped kit),
    /// one note per hit at the chosen pad, velocity from the hit's peak — one undo step.
    pub fn gate_hits_to_clip(&mut self, strip: StripId) -> bool {
        let notes = hit_notes(&self.fx.gate, ticks_per_second(self));
        if notes.is_empty() {
            return false;
        }
        let pitch = hit_pitch(&self.fx.gate);
        let ok = self.gate_notes_to_clip(strip, &notes, pitch);
        if ok {
            self.fx.gate.hits.clear();
        }
        ok
    }

    /// Notes (tick, velocity) at `pitch` as a MIDI clip on a new drum track named after
    /// `strip`, bar-aligned — one undo step.
    pub(crate) fn gate_notes_to_clip(
        &mut self,
        strip: StripId,
        notes: &[(i64, u8)],
        pitch: u8,
    ) -> bool {
        if notes.is_empty() {
            return false;
        }
        let kit = PulsusParams::default();
        let bar = self.project().time_signature.ticks_per_bar().0.max(1);
        let first = notes[0].0.div_euclid(bar) * bar;
        let last = notes[notes.len() - 1].0;
        let end = (last.div_euclid(bar) + 1) * bar;
        let name = tf(
            "gate.hits_track",
            &[(
                "strip",
                &self.capture_label(cantus_engine::CaptureTarget::Strip(strip)),
            )],
        );
        let mut track = self.bus.new_track(name);
        track.instrument = InstrumentSpec::Pulsus(kit);
        let track_id = track.id;
        let mut clip = self
            .bus
            .new_clip(t("gate.hits_clip"), Ticks(first), Ticks(end - first));
        for (at, vel) in notes {
            let note = self
                .bus
                .new_note(Ticks(at - first), Ticks::SIXTEENTH, pitch, *vel);
            clip.notes.push(note);
        }
        let index = self.project().tracks.len();
        let ok = self.exec(Command::Batch {
            label_key: "cmd.gate_hits".into(),
            commands: vec![
                Command::InsertTrack {
                    index,
                    track: Box::new(track),
                },
                Command::InsertClip {
                    track: track_id,
                    clip: Box::new(clip),
                },
            ],
        });
        if ok {
            self.set_status(tf("gate.hits_made", &[("n", &notes.len())]));
        }
        ok
    }
}

/// What a style is, in words — the answer to "what does Guitar mean?".
pub fn style_words(style: Style) -> String {
    let s = style.spec();
    let mode = match s.mode {
        Mode::Down => t("gate.words.down"),
        Mode::Up => t("gate.words.up"),
        Mode::Duck => t("gate.words.duck"),
    };
    let detector = if s.rms_ms > 0.0 {
        tf("gate.words.rms", &[("ms", &format!("{:.0}", s.rms_ms))])
    } else {
        tf(
            "gate.words.peak",
            &[("ms", &format!("{:.0}", s.peak_fall_ms))],
        )
    };
    let shape = if s.shape < 0.05 {
        t("gate.words.lines")
    } else {
        t("gate.words.curve")
    };
    let mut out = format!("{mode} \u{b7} {detector} \u{b7} {shape}");
    if s.follow {
        out.push_str(" \u{b7} ");
        out.push_str(&t("gate.words.follow"));
    }
    out
}

fn db_text(v: f32) -> String {
    format!("{v:+.1} dB")
}

fn ratio_text(r: f32) -> String {
    if r >= porta::INFINITE_RATIO {
        "\u{221e}:1".into()
    } else {
        format!("{r:.1}:1")
    }
}

fn range_text(r: f32) -> String {
    if r >= porta::INFINITE_RANGE {
        "\u{221e}".into()
    } else {
        format!("{r:.0} dB")
    }
}

fn phase_name(p: Phase) -> String {
    t(match p {
        Phase::Open => "gate.phase.open",
        Phase::Hold => "gate.phase.hold",
        Phase::Release => "gate.phase.release",
        Phase::Closed => "gate.phase.closed",
    })
}

/// The lane's colour for a phase; hold is also hatched and release is also paler, so the
/// colour is never the only cue.
fn phase_color(p: Phase) -> Option<Color32> {
    let pal = theme::p();
    match p {
        Phase::Open => Some(pal.ok),
        Phase::Hold => Some(pal.ok.gamma_multiply(0.75)),
        Phase::Release => Some(pal.ok.gamma_multiply(0.35)),
        Phase::Closed => None,
    }
}

/// Porta's editor in Cantus's own window: the host is the project (every drag one undo
/// step, the key any strip of the desk, hits a MIDI clip).
pub fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    // Cantus only: the gate played by a track's notes (ADR 0022). The source is a track of
    // the project, so this row lives here, outside the editor a plug-in window shares.
    let s = cantus_engine::fx::gate::settings(&slot.spec);
    trigger_row(app, ui, strip, effect, slot, &s);
    let mut st = std::mem::take(&mut app.fx.gate);
    if st.effect != Some(effect) {
        let pitch = st.hit_pitch;
        st = State {
            effect: Some(effect),
            hit_pitch: pitch,
            ..State::default()
        };
    }
    let mut host = DawHost::with_spec(app, strip, effect, slot.spec.clone(), "gate");
    editor(&mut host, &mut st, ui);
    app.fx.gate = st;
}

/// Porta's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW. Everything it needs from the host goes through [`FxHost`].
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) {
    let transport = host.transport();
    let tel = host.telemetry();
    st.ingest(&tel, transport.playing_at, transport.ticks_per_second);
    let spec = host.spec().clone();
    let s = cantus_engine::fx::gate::settings(&spec);
    style_row(host, ui, &s);
    key_row(host, st, ui, &s, &tel);
    ui.add_space(space::S1);
    let knobs_h = space::GRID * 26.0;
    let strip_h = space::GRID * 14.0;
    let h = (ui.available_height() - knobs_h - strip_h - space::S4).max(space::GRID * 40.0);
    ui.horizontal(|ui| {
        let w = (ui.available_width() - h - space::S2).max(space::GRID * 60.0);
        history(host, st, ui, &s, &tel, vec2(w, h));
        transfer(host, st, ui, &s, &tel, h);
    });
    ui.add_space(space::S1);
    let sr = if tel[telemetry::SAMPLE_RATE] > 0.0 {
        tel[telemetry::SAMPLE_RATE]
    } else {
        48_000.0
    };
    key_strip(host, st, ui, &s, sr, vec2(ui.available_width(), strip_h));
    ui.add_space(space::S2);
    knobs(host, ui, &spec, &s);
}

fn caption(ui: &mut egui::Ui, key: &str) {
    ui.label(
        RichText::new(t(key))
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
}

/// The styles, and what the chosen one is, in words.
fn style_row(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &GateSettings) {
    ui.horizontal(|ui| {
        caption(ui, "fx.gate.style");
        for style in Style::ALL {
            if ui
                .selectable_label(
                    s.style == style,
                    t(&format!("gate.style.{}", style.index())),
                )
                .on_hover_text(style_words(style))
                .clicked()
            {
                host.set("style", style.index() as f32, None);
            }
        }
        ui.label(
            RichText::new(style_words(s.style))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
    });
}

/// The key's source, what to hear, learning, hits, and the latency.
fn key_row(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &GateSettings,
    tel: &[f32; 64],
) {
    ui.horizontal(|ui| {
        caption(ui, "gate.key");
        match host.key_source() {
            // In Cantus: any strip of the desk, chosen here.
            Some(KeySource::Strips {
                current,
                label,
                options,
            }) => {
                egui::ComboBox::from_id_salt("gate-key")
                    .selected_text(label)
                    .width(space::GRID * 40.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current.is_none(), t("gate.key_own"))
                            .clicked()
                        {
                            host.set_key_source(KeyTarget::Strip(None));
                        }
                        for (id, text) in options {
                            if ui.selectable_label(current == Some(id), text).clicked() {
                                host.set_key_source(KeyTarget::Strip(Some(id)));
                            }
                        }
                    })
                    .response
                    .on_hover_text(t("gate.key_tip"));
            }
            // In another DAW: the host routes a sidechain; the detector is switched to it.
            Some(KeySource::Switch(on)) => {
                if ui
                    .selectable_label(!on, t("gate.key_own"))
                    .on_hover_text(t("gate.key_tip"))
                    .clicked()
                {
                    host.set_key_source(KeyTarget::Switch(false));
                }
                if ui
                    .selectable_label(on, t("export.sidechain.external"))
                    .on_hover_text(t("gate.key_tip"))
                    .clicked()
                {
                    host.set_key_source(KeyTarget::Switch(true));
                }
            }
            None => {}
        }
        ui.separator();
        caption(ui, "gate.hear");
        let removed = if s.style.spec().mode == Mode::Up {
            "gate.hear.2up"
        } else {
            "gate.hear.2"
        };
        for (i, key) in ["gate.hear.0", "gate.hear.1", removed].iter().enumerate() {
            let on = s.listen as usize == i;
            if ui
                .selectable_label(on, t(key))
                .on_hover_text(t(&format!("{key}_tip")))
                .clicked()
            {
                host.set("listen", i as f32, None);
            }
        }
        ui.separator();
        // Learn: gather while on; set the thresholds when switched off.
        let learning = st.learning.is_some();
        let learn_text = match &st.learning {
            Some(v) => tf(
                "gate.learning",
                &[(
                    "s",
                    &format!("{:.1}", v.len() as f32 * porta::COLUMN_MS * 0.001),
                )],
            ),
            None => t("gate.learn"),
        };
        if ui
            .selectable_label(learning, learn_text)
            .on_hover_text(t("gate.learn_tip"))
            .clicked()
        {
            if learning {
                finish_learning(host, st);
            } else {
                st.learning = Some(Vec::new());
            }
        }
        // Hits into a MIDI clip: only a DAW can make one.
        if host.makes_clips() {
            ui.separator();
            let capturing = st.capturing;
            let n = st.hits.len();
            if ui
                .selectable_label(capturing, tf("gate.hits", &[("n", &n)]))
                .on_hover_text(t("gate.hits_tip"))
                .clicked()
            {
                st.capturing = !capturing;
            }
            let kit = PulsusParams::default();
            let pitch = hit_pitch(st);
            let pad_name = kit
                .pads
                .iter()
                .find(|p| p.pitch == pitch)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| pitch.to_string());
            egui::ComboBox::from_id_salt("gate-pad")
                .selected_text(pad_name)
                .width(space::GRID * 22.0)
                .show_ui(ui, |ui| {
                    for pad in &kit.pads {
                        if ui.selectable_label(pad.pitch == pitch, &pad.name).clicked() {
                            st.hit_pitch = Some(pad.pitch);
                        }
                    }
                })
                .response
                .on_hover_text(t("gate.pad_tip"));
            if ui
                .add_enabled(n > 0, egui::Button::new(t("gate.to_clip")))
                .on_hover_text(t("gate.to_clip_tip"))
                .clicked()
            {
                hits_to_clip(host, st);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let latency = tel[telemetry::LATENCY];
            let sr = tel[telemetry::SAMPLE_RATE];
            let text = if s.lookahead_ms > 0.0 {
                let ms = if sr > 0.0 {
                    latency / sr * 1000.0
                } else {
                    porta::LOOKAHEAD_MAX_MS
                };
                tf("gate.latency", &[("ms", &format!("{ms:.1}"))])
            } else {
                t("gate.no_latency")
            };
            ui.label(
                RichText::new(text)
                    .text_style(design::mono_small())
                    .color(theme::p().text_dim),
            )
            .on_hover_text(t("gate.latency_tip"));
        });
    });
}

/// What opens the gate — the key's level or a track's notes (ADR 0022) — and, played by
/// notes, which track.
fn trigger_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &GateSettings,
) {
    ui.horizontal(|ui| {
        caption(ui, "fx.gate.trig");
        for (i, key) in ["gate.trig.0", "gate.trig.1"].iter().enumerate() {
            if ui
                .selectable_label(s.midi == (i == 1), t(key))
                .on_hover_text(t("gate.trig_tip"))
                .clicked()
            {
                app.set_fx_param(strip, effect, "trig", i as f32, false);
            }
        }
        if s.midi {
            midi_source(app, ui, strip, effect, slot);
        }
    });
}

/// The track whose notes play the gate (`EffectSlot::midi_from`, ADR 0022): one command,
/// one undo step. With none chosen the gate stays shut, and says so.
fn midi_source(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    caption(ui, "gate.midi_from");
    let current = slot.midi_from;
    let name = |app: &CantusApp, id: TrackId| {
        app.project()
            .track(id)
            .map(|t| t.name.clone())
            .filter(|n| !n.is_empty())
    };
    let label = current
        .and_then(|id| name(app, id))
        .unwrap_or_else(|| t("gate.midi_none"));
    let tracks: Vec<(TrackId, String)> = app
        .project()
        .tracks
        .iter()
        .map(|t| (t.id, t.name.clone()))
        .collect();
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("gate-midi", effect.0))
        .selected_text(label)
        .width(space::GRID * 30.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), t("gate.midi_none"))
                .clicked()
            {
                chosen = Some(None);
            }
            for (id, text) in &tracks {
                if ui
                    .selectable_label(current == Some(*id), text.as_str())
                    .clicked()
                {
                    chosen = Some(Some(*id));
                }
            }
        })
        .response
        .on_hover_text(t("gate.midi_tip"));
    if let Some(source) = chosen.filter(|s| *s != current) {
        app.exec(Command::SetEffectMidiSource {
            strip,
            effect,
            source,
        });
    }
    if current.is_none() {
        ui.label(
            RichText::new(t("gate.midi_waiting"))
                .text_style(design::caption())
                .color(theme::p().danger),
        );
    }
}

/// The level history: input as a dim fill, output over it in the device colour, the key as
/// a line, the gain change hanging from the top, the phase lane at the bottom, and the
/// open and close thresholds as lines you drag.
#[allow(clippy::too_many_arguments)]
fn history(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &GateSettings,
    tel: &[f32; 64],
    size: egui::Vec2,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    // A header strip for the legend and the readout, the plot, the phase lane.
    let lane_h = space::S3;
    let head_h = space::S4 + space::S1;
    let head = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + head_h));
    let plot = Rect::from_min_max(
        pos2(rect.left(), head.bottom()),
        pos2(rect.right(), rect.bottom() - lane_h - 1.0),
    );
    let lane = Rect::from_min_max(pos2(rect.left(), rect.bottom() - lane_h), rect.max);
    let y = |d: f32| plot.bottom() - ((d - LO_DB) / -LO_DB).clamp(0.0, 1.0) * plot.height();
    let d_of = |py: f32| LO_DB + (plot.bottom() - py) / plot.height() * -LO_DB;
    let geo = st.geometry.get_or_insert(Geometry {
        history: plot,
        transfer: Rect::NOTHING,
    });
    geo.history = plot;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    for g in [-12.0f32, -24.0, -36.0, -48.0, -60.0, -72.0] {
        p.hline(plot.x_range(), y(g), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(plot.left() + 3.0, y(g) - 1.0),
            Align2::LEFT_BOTTOM,
            format!("{g:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    let hist = &st.history;
    let dx = plot.width() / HISTORY as f32;
    let x0 = plot.right() - dx * hist.len() as f32;
    let x_of = |i: usize| x0 + dx * (i as f32 + 0.5);
    // Seconds ago along the bottom; bar lines while the song plays.
    for sec in 1..=((HISTORY as f32 * porta::COLUMN_MS * 0.001) as usize) {
        let x = plot.right() - sec as f32 / (porta::COLUMN_MS * 0.001) * dx;
        p.vline(x, plot.y_range(), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(x + 2.0, plot.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            format!("\u{2212}{sec} s"),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    let bar = host.transport().ticks_per_bar.max(1);
    let mut prev: Option<i64> = None;
    for (i, c) in hist.iter().enumerate() {
        if let (Some(tk), Some(pt)) = (c.tick, prev) {
            if tk.div_euclid(bar) != pt.div_euclid(bar) && tk > pt {
                let x = x_of(i);
                p.vline(x, plot.y_range(), Stroke::new(1.0, pal.grid_strong));
                p.text(
                    pos2(x + 2.0, plot.top() + 2.0),
                    Align2::LEFT_TOP,
                    format!("{}", tk.div_euclid(bar) + 1),
                    design::paint::ruler(),
                    pal.text_dim,
                );
            }
        }
        prev = c.tick;
    }
    // The hysteresis band, then the fills.
    let (ty, cy) = (y(s.threshold_db), y(s.close_db()));
    p.rect_filled(
        Rect::from_x_y_ranges(plot.x_range(), ty..=cy),
        0.0,
        pal.gold.gamma_multiply(0.08),
    );
    let fill = |value: &dyn Fn(&Column) -> f32, color: Color32| {
        let mut m = Mesh::default();
        for (i, c) in hist.iter().enumerate() {
            let x = x_of(i);
            let base = m.vertices.len() as u32;
            m.colored_vertex(pos2(x, y(value(&c.c))), color);
            m.colored_vertex(pos2(x, plot.bottom()), color);
            if i > 0 {
                m.add_triangle(base - 2, base - 1, base);
                m.add_triangle(base - 1, base, base + 1);
            }
        }
        m
    };
    p.add(fill(&|c| c.input_db, pal.text_dim.gamma_multiply(0.22)));
    p.add(fill(&|c| c.output_db, pal.gold.gamma_multiply(0.35)));
    let line = |value: &dyn Fn(&Column) -> f32| -> Vec<Pos2> {
        hist.iter()
            .enumerate()
            .map(|(i, c)| pos2(x_of(i), y(value(&c.c))))
            .collect()
    };
    if hist.len() > 1 {
        p.add(Shape::line(
            line(&|c| c.output_db),
            Stroke::new(1.25, pal.gold),
        ));
        p.add(Shape::line(
            line(&|c| c.key_db),
            Stroke::new(1.25, pal.text.gamma_multiply(0.9)),
        ));
    }
    // The gain change, from the top: 0 at the top, the whole swing a third of the way down.
    let swing = s.swing_db().max(1.0);
    let gy = |g: f32| plot.top() + (g.abs() / swing).clamp(0.0, 1.0) * plot.height() * 0.33;
    if hist.len() > 1 {
        let pts: Vec<Pos2> = hist
            .iter()
            .enumerate()
            .map(|(i, c)| pos2(x_of(i), gy(c.c.gain_db)))
            .collect();
        p.add(Shape::line(pts, Stroke::new(1.5, pal.danger)));
    }
    // Its scale, past the dB labels: the whole swing.
    let full = if s.style.spec().mode == Mode::Up {
        format!("{} +{:.0} dB", t("gate.gain"), s.swing_db())
    } else {
        format!("{} \u{2212}{}", t("gate.gain"), range_text(s.range_db))
    };
    p.hline(
        plot.left() + space::S8..=plot.left() + space::S8 + space::S3,
        gy(swing),
        Stroke::new(1.0, pal.danger),
    );
    plate_text(
        &p,
        pos2(plot.left() + space::S8 + space::S4, gy(swing)),
        Align2::LEFT_CENTER,
        full,
        pal.danger,
    );
    // The phase lane: colour and pattern per run.
    p.rect_filled(lane, 0.0, pal.panel);
    let mut i = 0;
    while i < hist.len() {
        let ph = hist[i].c.phase;
        let mut j = i;
        while j < hist.len() && hist[j].c.phase == ph {
            j += 1;
        }
        if let Some(color) = phase_color(ph) {
            let r =
                Rect::from_x_y_ranges(x_of(i) - dx * 0.5..=x_of(j - 1) + dx * 0.5, lane.y_range());
            p.rect_filled(r, 0.0, color);
            if ph == Phase::Hold {
                let lp = p.with_clip_rect(r);
                let mut x = r.left() - lane.height();
                while x < r.right() {
                    lp.line_segment(
                        [pos2(x, lane.bottom()), pos2(x + lane.height(), lane.top())],
                        Stroke::new(1.0, pal.panel),
                    );
                    x += 4.0;
                }
            }
        }
        i = j;
    }
    // The thresholds: open solid, close dashed; brighter when near or held.
    let hover = resp.hover_pos();
    let near_open = hover.is_some_and(|pt| plot.contains(pt) && (pt.y - ty).abs() < 6.0);
    let near_close =
        !near_open && hover.is_some_and(|pt| plot.contains(pt) && (pt.y - cy).abs() < 6.0);
    let drag = st.drag;
    let open_hot = near_open || matches!(drag, Some(Grab::Open { .. }));
    let close_hot = near_close || drag == Some(Grab::Close);
    p.hline(
        plot.x_range(),
        ty,
        Stroke::new(if open_hot { 2.5 } else { 1.5 }, pal.gold),
    );
    p.add(Shape::dashed_line(
        &[pos2(plot.left(), cy), pos2(plot.right(), cy)],
        Stroke::new(if close_hot { 2.0 } else { 1.0 }, pal.gold),
        6.0,
        4.0,
    ));
    plate_text(
        &p,
        pos2(plot.right() - space::S2, ty - 2.0),
        Align2::RIGHT_BOTTOM,
        format!("{} {}", t("gate.open"), db_text(s.threshold_db)),
        pal.gold,
    );
    plate_text(
        &p,
        pos2(plot.right() - space::S2, cy + 2.0),
        Align2::RIGHT_TOP,
        format!("{} {}", t("gate.close"), db_text(s.close_db())),
        pal.gold,
    );
    // Now: the key, the gain, the phase.
    let phase = Phase::from_code(tel[telemetry::PHASE].max(0.0) as u32);
    let now = format!(
        "{} {}   {} {}   {}",
        t("gate.key"),
        db_text(tel[telemetry::LEVEL].max(-120.0)),
        t("gate.gain"),
        db_text(tel[telemetry::GAIN]),
        phase_name(phase),
    );
    p.rect_filled(head, 0.0, pal.panel);
    p.hline(
        head.x_range(),
        head.bottom(),
        Stroke::new(1.0, pal.grid_strong),
    );
    p.text(
        pos2(head.right() - space::S2, head.center().y),
        Align2::RIGHT_CENTER,
        now,
        design::paint::ruler(),
        pal.text,
    );
    // The legend, so the lane is read without a colour.
    legend(
        &p,
        pos2(head.left() + space::S2, head.center().y - space::S2 + 1.0),
    );
    // Hover: what happened at that moment.
    if let Some(pt) = hover.filter(|pt| rect.contains(*pt)) {
        let i = ((pt.x - x0) / dx).floor();
        if i >= 0.0 && (i as usize) < hist.len() {
            let c = hist[i as usize].c;
            let ago = (hist.len() - i as usize) as f32 * porta::COLUMN_MS * 0.001;
            p.vline(pt.x, plot.y_range(), Stroke::new(1.0, pal.grid_strong));
            plate_text(
                &p,
                pos2(plot.left() + space::S2, plot.bottom() - space::S4),
                Align2::LEFT_BOTTOM,
                format!(
                    "\u{2212}{ago:.2} s   {} {}   {} {}   {}   \u{2192} {:.1} dB",
                    t("gate.key"),
                    db_text(c.key_db),
                    t("gate.gain"),
                    db_text(c.gain_db),
                    phase_name(c.phase),
                    d_of(pt.y),
                ),
                pal.text,
            );
        }
    }
    if hist.is_empty() {
        p.text(
            plot.center(),
            Align2::CENTER_CENTER,
            t("gate.empty"),
            design::paint::empty_state(),
            pal.text_dim,
        );
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    // Gestures.
    if resp.drag_started() {
        // Decide by where the button went down: by the time a drag is recognised the
        // pointer has already moved off the line.
        let alone = ui.input(|i| i.modifiers.alt);
        let at = ui.input(|i| i.pointer.press_origin()).or(hover);
        let on = |line: f32| at.is_some_and(|pt| plot.contains(pt) && (pt.y - line).abs() < 6.0);
        st.drag = if on(ty) {
            Some(Grab::Open { alone })
        } else if on(cy) {
            Some(Grab::Close)
        } else {
            None
        };
    }
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            drag_threshold(host, s, g, d_of(pt.y));
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() {
        if near_open {
            host.set("thr", -40.0, None);
        } else if near_close {
            host.set("hyst", 4.0, None);
        }
    }
    if near_open || near_close || drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    resp.on_hover_text(t("gate.history_tip"));
}

/// Move a threshold to `db`: the open line carries the close line with it (Alt: alone);
/// the close line sets the hysteresis.
fn drag_threshold(host: &mut dyn FxHost, s: &GateSettings, g: Grab, db: f32) {
    match g {
        Grab::Open { alone: false } => {
            host.set("thr", db.clamp(-80.0, 0.0), Some("thr"));
        }
        Grab::Open { alone: true } => {
            let close = s.close_db();
            let thr = db.clamp(close, (close + 24.0).min(0.0)).clamp(-80.0, 0.0);
            host.set("thr", thr, Some("open-alone"));
            host.set("hyst", (thr - close).clamp(0.0, 24.0), Some("open-alone"));
        }
        Grab::Close => {
            host.set("hyst", (s.threshold_db - db).clamp(0.0, 24.0), Some("hyst"));
        }
        _ => {}
    }
}

/// Canvas text on a plate of the panel colour, so it reads over the history's fills.
fn plate_text(p: &egui::Painter, at: Pos2, align: Align2, text: String, color: Color32) {
    let pal = theme::p();
    let galley = p.layout_no_wrap(text, design::paint::ruler(), color);
    let rect = align.anchor_size(at, galley.size());
    p.rect_filled(rect.expand(1.5), 2.0, pal.panel.gamma_multiply(0.85));
    p.galley(rect.min, galley, color);
}

fn legend(p: &egui::Painter, at: Pos2) {
    let pal = theme::p();
    let mut x = at.x;
    for ph in [Phase::Open, Phase::Hold, Phase::Release] {
        let r = Rect::from_min_size(pos2(x, at.y + 1.0), vec2(space::S3, space::S2));
        if let Some(c) = phase_color(ph) {
            p.rect_filled(r, 1.0, c);
        }
        if ph == Phase::Hold {
            let lp = p.with_clip_rect(r);
            let mut hx = r.left() - r.height();
            while hx < r.right() {
                lp.line_segment(
                    [pos2(hx, r.bottom()), pos2(hx + r.height(), r.top())],
                    Stroke::new(1.0, pal.panel),
                );
                hx += 4.0;
            }
        }
        let g = p.text(
            pos2(r.right() + space::S1, at.y),
            Align2::LEFT_TOP,
            phase_name(ph),
            design::paint::caption(),
            pal.text_dim,
        );
        x = g.right() + space::S3;
    }
}

/// The transfer curve: key level across, output up. The closed curve is solid, the open
/// one (measured from the close threshold) dashed. Drag the open and close points, the
/// slope (ratio) and the floor (range).
#[allow(clippy::too_many_arguments)]
fn transfer(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &GateSettings,
    tel: &[f32; 64],
    side: f32,
) {
    let pal = theme::p();
    let (outer, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click_and_drag());
    // The same header strip as the history's, for the curve's numbers.
    let head = Rect::from_min_max(
        outer.min,
        pos2(outer.right(), outer.top() + space::S4 + space::S1),
    );
    let rect = Rect::from_min_max(pos2(outer.left(), head.bottom()), outer.max);
    let hi = 12.0;
    let span = hi - LO_DB;
    let x = |d: f32| rect.left() + ((d - LO_DB) / span).clamp(0.0, 1.0) * rect.width();
    let y = |d: f32| rect.bottom() - ((d - LO_DB) / span).clamp(0.0, 1.0) * rect.height();
    let dx_of = |px: f32| LO_DB + (px - rect.left()) / rect.width() * span;
    let dy_of = |py: f32| LO_DB + (rect.bottom() - py) / rect.height() * span;
    if let Some(geo) = st.geometry.as_mut() {
        geo.transfer = rect;
    }
    let p = ui.painter_at(outer);
    p.rect_filled(outer, pal.corner_radius, theme::recess(&pal));
    p.rect_filled(head, 0.0, pal.panel);
    p.hline(
        head.x_range(),
        head.bottom(),
        Stroke::new(1.0, pal.grid_strong),
    );
    for g in [-72.0f32, -60.0, -48.0, -36.0, -24.0, -12.0, 0.0] {
        p.hline(rect.x_range(), y(g), Stroke::new(1.0, pal.grid));
        p.vline(x(g), rect.y_range(), Stroke::new(1.0, pal.grid));
        if g > -72.0 {
            p.text(
                pos2(x(g) + 2.0, rect.bottom() - 2.0),
                Align2::LEFT_BOTTOM,
                format!("{g:.0}"),
                design::paint::ruler(),
                pal.text_dim,
            );
            p.text(
                pos2(rect.left() + 2.0, y(g) - 1.0),
                Align2::LEFT_BOTTOM,
                format!("{g:.0}"),
                design::paint::ruler(),
                pal.text_dim,
            );
        }
    }
    p.line_segment(
        [pos2(x(LO_DB), y(LO_DB)), pos2(x(hi), y(hi))],
        Stroke::new(1.0, pal.grid_bar),
    );
    let curve = |open: bool| -> Vec<Pos2> {
        (0..=184)
            .map(|i| {
                let d = LO_DB + span * i as f32 / 184.0;
                pos2(x(d), y(porta::transfer_db(d, s, open)))
            })
            .collect()
    };
    let mode = s.style.spec().mode;
    if mode != Mode::Up && s.hysteresis_db > 0.0 {
        p.add(Shape::dashed_line(
            &curve(true),
            Stroke::new(1.5, pal.gold.gamma_multiply(0.7)),
            5.0,
            4.0,
        ));
    }
    p.add(Shape::line(curve(false), Stroke::new(2.5, pal.gold)));
    // Handles: open and close points, the floor.
    let open_pt = pos2(
        x(s.threshold_db),
        y(porta::transfer_db(s.threshold_db, s, false)),
    );
    let close_pt = pos2(
        x(s.close_db()),
        y(porta::transfer_db(s.close_db(), s, true)),
    );
    // The floor: where the gate lands when it shuts, under the threshold (kept inside the
    // display; dragging past the bottom goes on to ∞).
    let floor_pt = pos2(
        x(s.threshold_db),
        y(s.threshold_db - s.swing_db()).min(rect.bottom() - 5.0),
    );
    p.circle_filled(open_pt, 6.0, pal.gold);
    if mode != Mode::Up {
        p.circle_stroke(close_pt, 5.0, Stroke::new(2.0, pal.gold));
    }
    if mode == Mode::Down {
        p.rect_filled(
            Rect::from_center_size(floor_pt, vec2(9.0, 9.0)),
            1.0,
            pal.gold,
        );
    }
    // Where the key is now.
    let level = tel[telemetry::LEVEL];
    if level > LO_DB {
        let out = level + tel[telemetry::GAIN].max(-120.0);
        p.circle_filled(pos2(x(level), y(out)), 4.0, pal.danger);
    }
    p.text(
        pos2(head.left() + space::S2, head.center().y),
        Align2::LEFT_CENTER,
        format!(
            "{} {}  \u{b7}  {} {}  \u{b7}  {} {:.0} dB",
            t("fx.gate.ratio"),
            ratio_text(s.ratio),
            t("fx.gate.range"),
            range_text(s.range_db),
            t("fx.gate.knee"),
            s.knee_db
        ),
        design::paint::ruler(),
        pal.text,
    );
    p.rect_stroke(
        outer,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(pt) = origin.or(resp.interact_pointer_pos()) {
            let near = |q: Pos2| pt.distance(q) < 9.0;
            let d = dx_of(pt.x);
            st.drag = if near(open_pt) {
                Some(Grab::Open {
                    alone: ui.input(|i| i.modifiers.alt),
                })
            } else if mode != Mode::Up && near(close_pt) {
                Some(Grab::Close)
            } else if mode == Mode::Down && near(floor_pt) {
                Some(Grab::Range)
            } else if (mode == Mode::Down && d < s.threshold_db)
                || (mode == Mode::Up && d > s.threshold_db)
            {
                Some(Grab::Ratio)
            } else {
                None
            };
        }
    }
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            match g {
                Grab::Open { .. } | Grab::Close => {
                    drag_threshold(host, s, g, dx_of(pt.x));
                }
                Grab::Range => {
                    let r = (s.threshold_db - dy_of(pt.y)).clamp(0.0, 100.0);
                    host.set("range", r, Some("range"));
                }
                Grab::Ratio => {
                    // The pointer is a point the curve should pass through: below the
                    // threshold (Upward: above) the output moves `ratio` dB per dB.
                    let over_in = dx_of(pt.x) - s.threshold_db;
                    let over_out = dy_of(pt.y) - s.threshold_db;
                    let ratio = if over_in.abs() < 0.5 {
                        s.ratio
                    } else {
                        (over_out / over_in).clamp(1.0, 100.0)
                    };
                    host.set("ratio", ratio, Some("ratio"));
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    resp.on_hover_text(t("gate.transfer_tip"));
}

/// The key's filters, dragged on a spectrum of what comes in.
#[allow(clippy::too_many_arguments)]
fn key_strip(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &GateSettings,
    sr: f32,
    size: egui::Vec2,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let lx = |hz: f32| rect.left() + (hz / F_LO).ln() / (F_HI / F_LO).ln() * rect.width();
    let hz_of =
        |px: f32| F_LO * (F_HI / F_LO).powf(((px - rect.left()) / rect.width()).clamp(0.0, 1.0));
    let (top_db, bot_db) = (18.0f32, -48.0f32);
    let y =
        |d: f32| rect.top() + ((top_db - d) / (top_db - bot_db)).clamp(0.0, 1.0) * rect.height();
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    for f in [
        50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
    ] {
        p.vline(lx(f), rect.y_range(), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(lx(f) + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            crate::widgets::knob::readout(f, "Hz"),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    // 0 dB, for the response to sit on.
    p.add(Shape::dashed_line(
        &[pos2(rect.left(), y(0.0)), pos2(rect.right(), y(0.0))],
        Stroke::new(1.0, pal.grid_strong),
        3.0,
        3.0,
    ));
    // What comes in (the effect's input), behind.
    {
        let an = st
            .spectrum
            .get_or_insert_with(|| cantus_dsp::SpectrumAnalyser::new(4096));
        let pairs = host.pairs();
        let take = pairs.len().min(an.size());
        let mut buf = Vec::with_capacity(take * 2);
        for (a, b) in pairs.iter().skip(pairs.len() - take) {
            buf.push(*a);
            buf.push(*b);
        }
        an.push(&buf);
        an.compute();
    }
    let any_pairs = !host.pairs().is_empty();
    if let Some(an) = st.spectrum.as_ref().filter(|a| a.has_data() || any_pairs) {
        let mut m = Mesh::default();
        let n = 160;
        let dim = pal.text_dim.gamma_multiply(0.22);
        for i in 0..=n {
            let hz = F_LO * (F_HI / F_LO).powf(i as f32 / n as f32);
            // The spectrum sits on its own scale: 0 dBFS at the top, −96 at the bottom.
            let level = an.level_db(0, sr, hz, 1.0 / 6.0);
            let py = rect.top() + ((0.0 - level) / 96.0).clamp(0.0, 1.0) * rect.height();
            let base = m.vertices.len() as u32;
            m.colored_vertex(pos2(lx(hz), py), dim);
            m.colored_vertex(pos2(lx(hz), rect.bottom()), dim);
            if i > 0 {
                m.add_triangle(base - 2, base - 1, base);
                m.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(m);
    }
    // The pass band, lit; the response drawn over it.
    let (lo, hi) = (
        if s.hpf_on() { s.hpf_hz } else { F_LO },
        if s.lpf_on() { s.lpf_hz } else { F_HI },
    );
    p.rect_filled(
        Rect::from_x_y_ranges(lx(lo)..=lx(hi), rect.y_range()),
        0.0,
        pal.gold.gamma_multiply(0.08),
    );
    let curve: Vec<Pos2> = (0..=200)
        .map(|i| {
            let hz = F_LO * (F_HI / F_LO).powf(i as f32 / 200.0);
            pos2(lx(hz), y(porta::key_filter_db(s, sr, hz)))
        })
        .collect();
    p.add(Shape::line(curve, Stroke::new(2.0, pal.gold)));
    // The handles: triangles at the corners, labelled.
    let handle = |hz: f32, on: bool, key: &str, right: bool| {
        let cx = lx(hz);
        let by = rect.bottom() - 1.0;
        p.vline(
            cx,
            rect.top() + space::S1..=by - 10.0,
            Stroke::new(1.0, pal.gold.gamma_multiply(if on { 0.6 } else { 0.3 })),
        );
        let tri = vec![pos2(cx, by - 10.0), pos2(cx - 6.0, by), pos2(cx + 6.0, by)];
        if on {
            p.add(Shape::convex_polygon(tri, pal.gold, Stroke::NONE));
        } else {
            p.add(Shape::closed_line(tri, Stroke::new(1.5, pal.gold)));
        }
        let text = if on {
            format!("{} {}", t(key), crate::widgets::knob::readout(hz, "Hz"))
        } else {
            format!("{} {}", t(key), t("gate.off"))
        };
        p.text(
            pos2(
                if right { cx - 4.0 } else { cx + 4.0 },
                rect.top() + space::S1,
            ),
            if right {
                Align2::RIGHT_TOP
            } else {
                Align2::LEFT_TOP
            },
            text,
            design::paint::ruler(),
            pal.text,
        );
    };
    handle(lo, s.hpf_on(), "gate.hp", false);
    handle(hi, s.lpf_on(), "gate.lp", true);
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(pt) = origin.or(resp.interact_pointer_pos()) {
            let (dl, dh) = ((pt.x - lx(lo)).abs(), (pt.x - lx(hi)).abs());
            st.drag = Some(if dl <= dh { Grab::Hpf } else { Grab::Lpf });
        }
    }
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let hz = hz_of(pt.x);
            match g {
                Grab::Hpf => {
                    let v = if hz <= 21.0 { 20.0 } else { hz.min(10_000.0) };
                    host.set("hpf", v, Some("hpf"));
                }
                Grab::Lpf => {
                    let v = if hz >= 19_000.0 {
                        20_000.0
                    } else {
                        hz.max(100.0)
                    };
                    host.set("lpf", v, Some("lpf"));
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() {
        host.set_many(&[("hpf", 20.0), ("lpf", 20_000.0)], "cmd.gate_key_reset");
    }
    resp.on_hover_text(t("gate.key_strip_tip"));
}

/// The knobs, in three labelled groups: detect, time, key.
fn knobs(host: &mut dyn FxHost, ui: &mut egui::Ui, spec: &EffectSpec, s: &GateSettings) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S1;
        // Played by notes, the level's knobs mean nothing; the velocity's does.
        let detect: &[&str] = if s.midi {
            &["range", "vel"]
        } else {
            &["thr", "hyst", "ratio", "range", "knee"]
        };
        let groups: [(&str, &[&str]); 3] = [
            ("gate.group.detect", detect),
            ("gate.group.time", &["att", "hold", "rel", "look"]),
            ("gate.group.key", &["hpf", "lpf"]),
        ];
        for (gi, (title, keys)) in groups.iter().enumerate() {
            if gi > 0 {
                ui.add_space(space::S2);
                ui.separator();
                ui.add_space(space::S2);
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(t(title))
                        .text_style(design::caption())
                        .color(pal.text_dim),
                );
                ui.horizontal(|ui| {
                    for key in keys.iter() {
                        knob(host, ui, spec, key);
                    }
                    if *title == "gate.group.key" {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(t("fx.gate.key_slope"))
                                    .text_style(design::caption())
                                    .color(pal.text_dim),
                            );
                            ui.horizontal(|ui| {
                                for slope in [12.0f32, 24.0, 48.0] {
                                    if ui
                                        .selectable_label(
                                            s.key_slope == slope,
                                            format!("{slope:.0}"),
                                        )
                                        .on_hover_text(t("gate.slope_tip"))
                                        .clicked()
                                    {
                                        host.set("key_slope", slope, None);
                                    }
                                }
                            });
                        });
                    }
                });
            });
        }
    });
}

fn knob(host: &mut dyn FxHost, ui: &mut egui::Ui, spec: &EffectSpec, key: &str) {
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let mut v = spec.get(key);
    let k = Knob::new(&mut v, info.min..=info.max, info.default)
        .diameter(space::GRID * 11.0)
        .log(info.logarithmic)
        .stepped(info.stepped)
        .unit(info.unit)
        .label(t(info.label_key));
    let k = match key {
        "ratio" => k.format(ratio_text),
        "range" => k.format(range_text),
        "hpf" => k.format(|x| {
            if x <= 20.5 {
                t("gate.off")
            } else {
                crate::widgets::knob::readout(x, "Hz")
            }
        }),
        "lpf" => k.format(|x| {
            if x >= 19_999.0 {
                t("gate.off")
            } else {
                crate::widgets::knob::readout(x, "Hz")
            }
        }),
        "look" => k.format(|x| {
            if x <= 0.0 {
                t("gate.off")
            } else {
                format!("{x:.1} ms")
            }
        }),
        "hyst" | "vel" => k.format(|x| format!("{x:.1} dB")),
        "att" => k.format(|x| {
            if x < 1.0 {
                format!("{x:.2} ms")
            } else {
                crate::widgets::knob::readout(x, "ms")
            }
        }),
        _ => k,
    };
    let mut r = ui.add(k);
    if key == "vel" {
        r = r.on_hover_text(t("gate.vel_tip"));
    }
    if r.changed() {
        host.set(key, v, Some(key));
    }
    if r.drag_stopped() || r.double_clicked() {
        host.end_gesture();
    }
}

/// The inspector's picture: the transfer curve and the settings in one line.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let pal = theme::p();
    let s = cantus_engine::fx::gate::settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let sq = Rect::from_min_size(rect.min, vec2(h, h)).shrink(space::S1);
    let x = |d: f32| sq.left() + ((d - LO_DB) / -LO_DB).clamp(0.0, 1.0) * sq.width();
    let y = |d: f32| sq.bottom() - ((d - LO_DB) / -LO_DB).clamp(0.0, 1.0) * sq.height();
    p.line_segment(
        [pos2(x(LO_DB), y(LO_DB)), pos2(x(0.0), y(0.0))],
        Stroke::new(1.0, pal.grid_bar),
    );
    let pts: Vec<Pos2> = (0..=80)
        .map(|i| {
            let d = LO_DB * (1.0 - i as f32 / 80.0);
            pos2(x(d), y(porta::transfer_db(d, &s, false)))
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.5, pal.gold)));
    let line = format!(
        "{} {:.0} \u{b7} {} {:.0} dB \u{b7} {} \u{b7} {} \u{b7} {}",
        t("gate.open"),
        s.threshold_db,
        t("gate.close"),
        s.close_db(),
        ratio_text(s.ratio),
        range_text(s.range_db),
        t(&format!("gate.style.{}", s.style.index())),
    );
    p.text(
        pos2(sq.right() + space::S2, rect.center().y),
        Align2::LEFT_CENTER,
        line,
        design::paint::ruler(),
        pal.text,
    );
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
    resp.on_hover_text(t("gate.open_tip"))
}

/// The settings `--editor gate` and the screenshot tool open it with: a snare mic, its
/// key high-passed above the kick.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "gate").set_many(
        &[
            ("thr", -30.0),
            ("hyst", 6.0),
            ("range", 40.0),
            ("att", 0.5),
            ("hold", 40.0),
            ("rel", 160.0),
            ("look", 2.0),
            ("hpf", 150.0),
            ("lpf", 8_000.0),
        ],
        "cmd.fx_preset",
    );
}

/// A snare-like hit with its ring, over kick and hi-hat bleed: four seconds of it, run
/// through the real gate, as a playing engine would feed the editor.
pub fn demo_signal(seconds: f32, sr: f32) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let mut seed = 11u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let beat = (sr * 60.0 / 120.0) as usize;
    (0..n)
        .map(|i| {
            let tb = i % beat;
            let bar_beat = (i / beat) % 4;
            let t = tb as f32 / sr;
            // Snare on 2 and 4, a softer ghost before 4; kick bleed on 1 and 3.
            let snare = match bar_beat {
                1 | 3 => 0.6,
                _ => 0.0,
            };
            let ghost = if bar_beat == 2 && tb > beat / 2 {
                0.12
            } else {
                0.0
            };
            let tg = if ghost > 0.0 {
                (tb - beat / 2) as f32 / sr
            } else {
                t
            };
            let body = (snare + ghost)
                * (-tg * 18.0).exp()
                * ((std::f32::consts::TAU * 190.0 * tg).sin() * 0.6 + noise() * 0.5);
            let kick = if bar_beat % 2 == 0 {
                0.02 * (-t * 12.0).exp() * (std::f32::consts::TAU * 55.0 * t).sin()
            } else {
                0.0
            };
            let hat = 0.006 * noise() * (-((i % (beat / 2)) as f32 / sr) * 30.0).exp();
            body + kick + hat
        })
        .collect()
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(spec) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .map(|e| e.spec.clone())
    else {
        return;
    };
    let sr = 48_000.0;
    let mut g = porta::Porta::new(sr);
    g.set(&cantus_engine::fx::gate::settings(&spec));
    let x = demo_signal(4.2, sr);
    let block = 480;
    let mut t = [0.0f32; 64];
    let st = &mut app.fx.gate;
    *st = State {
        effect: Some(effect),
        ..State::default()
    };
    // Hits are captured, as they would be with the button on.
    st.capturing = true;
    let mut pairs = Vec::with_capacity(x.len() * 2);
    // The song "plays" from bar 1 at 120 BPM, so bar lines run through the history.
    let tps = 120.0 / 60.0 * cantus_core::PPQ as f64;
    for (b, chunk) in x.chunks(block).enumerate() {
        let (mut l, mut r) = (chunk.to_vec(), chunk.to_vec());
        g.process(&mut l, &mut r, None);
        g.write_telemetry(&mut t);
        let now = ((b + 1) * block) as f64 / sr as f64 * tps;
        st.ingest(&t, Some(now as i64), tps);
        for (a, o) in chunk.iter().zip(&l) {
            pairs.push(*a);
            pairs.push(*o);
        }
    }
    app.fx.feed(&pairs);
    app.fx.injected = Some(t);
}
