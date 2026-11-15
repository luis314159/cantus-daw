//! Saltus's editor: the pitch unit (fx side round, wave 3, ADR 0135 provisional).
//!
//! *Saltus* is Latin for the leap, and the window draws one: across its dark display a
//! keyboard, the note it hears lit on it, and from that key **an arc to each voice** — up or
//! down by the interval, labelled with the interval's name and landing on the note it makes;
//! in Harmony the scale's notes marked and the chord held lit, in Octave the octaves' three
//! arcs weighted by their levels, in Whammy one arc swept by **the pedal**, drawn beside the
//! keyboard in side view, rocking from heel to toe. The four modes are the legends along the
//! display's top. Under it, the mode's controls: the voices, the harmony's key and scale or
//! the octaves, the sound (engine, formants, throat) and the mix.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::menu::Item;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::{Command, EffectId, EffectKind, EffectSpec, StripId, TrackId};
use cantus_dsp::saltus::{Mode, SaltusSettings};
use egui::Rect;

#[path = "saltus_display.rs"]
mod display;
#[path = "saltus_pedal.rs"]
mod pedal;
#[path = "saltus_words.rs"]
pub mod words;

const KIND: EffectKind = EffectKind::PitchShift;

/// Factory presets: data (`presets/effects/pitchshift.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    view: display::View,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
}

/// Saltus's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.saltus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "saltus"), &mut st, ui);
    app.fx.saltus = st;
    out
}

/// Saltus's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let ss = cantus_engine::fx::saltus::settings(host.spec());
    let latency = cantus_dsp::saltus::latency_for(&ss, sr) as f32;
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ss)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The display over a row of the mode's cards.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ss: &SaltusSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row = controls::card_height(Size::M) * k;
    let gap = s.d(4.0);
    let top = (area.height() - row - gap).max(240.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    let reading = cantus_engine::fx::saltus::read(tel);
    st.view.update(&reading, ss, ui.input(|i| i.stable_dt).min(0.1));
    display::show(host, &mut st.view, ui, s, r1, ss, &reading);
    if st.view.moving() {
        ui.ctx().request_repaint();
    }
    let fr = fractions(ss.mode);
    let cards: Vec<Rect> = g.row(row, fr).collect();
    match ss.mode {
        Mode::Shift => {
            voice_card(host, ui, s, cards[0], 0, false);
            voice_card(host, ui, s, cards[1], 1, false);
            sound_card(host, ui, s, cards[2], ss, true);
            mix_card(host, ui, s, cards[3]);
        }
        Mode::Harmony => {
            voice_card(host, ui, s, cards[0], 0, true);
            voice_card(host, ui, s, cards[1], 1, true);
            harmony_card(host, ui, s, cards[2], ss);
            sound_card(host, ui, s, cards[3], ss, false);
            mix_card(host, ui, s, cards[4]);
        }
        Mode::Octave => {
            octaves_card(host, ui, s, cards[0]);
            style_card(host, ui, s, cards[1], ss);
            sound_card(host, ui, s, cards[2], ss, false);
            mix_card(host, ui, s, cards[3]);
        }
        Mode::Whammy => {
            pedal_card(host, ui, s, cards[0]);
            played_card(host, ui, s, cards[1], ss);
            sound_card(host, ui, s, cards[2], ss, true);
            mix_card(host, ui, s, cards[3]);
        }
    }
}

/// Each mode's cards' shares of the row, at their designed widths.
fn fractions(mode: Mode) -> &'static [f32] {
    match mode {
        // Voice, voice, sound, mix.
        Mode::Shift => &[0.268, 0.268, 0.276, 0.188],
        // Voice, voice, harmony, sound, mix.
        Mode::Harmony => &[0.226, 0.226, 0.214, 0.186, 0.148],
        // Octaves, style, sound, mix.
        Mode::Octave => &[0.25, 0.302, 0.26, 0.188],
        // Pedal, played by, sound, mix.
        Mode::Whammy => &[0.268, 0.268, 0.276, 0.188],
    }
}

/// The keys of voice `v` (0 or 1): interval, fine, degrees, level, balance.
const VOICE: [[&str; 5]; 2] = cantus_engine::fx::saltus::VOICE_KEYS;

/// A cell on `key` with a short label of its own (the card says whose it is).
fn labelled(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    label: &'static str,
    pill: Option<controls::Pill<'_>>,
) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let mut c = controls::Cell::param(info, spec.get(key))
        .size(Size::M)
        .label(Src::Key(label));
    if let Some(f) = pill {
        c = c.format(f);
    }
    let out = c.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// A cell wider than a knob's own, for a label of two words ("One down", "Una arriba").
fn wide(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, key: &'static str, label: &'static str) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let out = controls::Cell::param(info, spec.get(key))
        .size(Size::M)
        .width(88.0)
        .label(Src::Key(label))
        .show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// A voice: its interval (semitones, or degrees in Harmony), fine, level and balance.
fn voice_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    v: usize,
    harmony: bool,
) {
    let title = ["saltus.card.voice1", "saltus.card.voice2"][v];
    let [st, ct, deg, lvl, pan] = VOICE[v];
    controls::card(ui, s, rect, Src::Key(title), None, |ui, _| {
        if harmony {
            labelled(host, ui, s, deg, "saltus.l.degrees", Some(&words::degree_pill));
        } else {
            labelled(host, ui, s, st, "saltus.l.interval", Some(&words::semitone_pill));
        }
        labelled(host, ui, s, ct, "saltus.l.fine", None);
        labelled(host, ui, s, lvl, "saltus.l.level", None);
        labelled(host, ui, s, pan, "saltus.l.balance", Some(&words::pan_pill));
    });
}

/// A segmented choice of stepped `key` with its caption, in a column `w` wide.
fn choice(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    caption: &'static str,
    key: &'static str,
    labels: &[Src<'_>],
    w: f32,
) {
    controls::caption(ui, s, Src::Key(caption), w);
    controls::segmented_param(ui, s, host, key, labels, w);
}

/// A field of a stepped `key` whose choices are `names` (a list opens under it).
#[allow(clippy::too_many_arguments)]
fn field_of(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    names: &[&'static str],
    now: usize,
    w: f32,
    list_w: f32,
) {
    let id = ui.id().with(("saltus-field", key));
    let tip = host
        .spec()
        .param(key)
        .map_or("", |i| frame::tr(i.label_key));
    let now = now.min(names.len() - 1);
    let at = controls::field_key(ui, s, host, key, id, frame::tr(names[now]), tip, w);
    if let Some(at) = at {
        let items: Vec<Item<'_>> = names
            .iter()
            .enumerate()
            .map(|(i, n)| frame::radio(Src::Key(n), i == now))
            .collect();
        if let Some(i) = controls::list(ui, s, id, at, list_w.max(w), &items) {
            host.set(key, i as f32, None);
        }
    }
}

/// The sound: the engine (Shift and Whammy), the formants and the throat.
fn sound_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ss: &SaltusSettings,
    engine: bool,
) {
    controls::card(ui, s, rect, Src::Key("saltus.card.sound"), None, |ui, _| {
        let w = 132.0;
        controls::column(ui, s, w, |ui| {
            if engine {
                let labels = [
                    Src::Key("fx.saltus.engine.0"),
                    Src::Key("fx.saltus.engine.1"),
                ];
                choice(host, ui, s, "saltus.l.engine", "engine", &labels, w);
            }
            let labels = [
                Src::Key("fx.saltus.formant.1"),
                Src::Key("fx.saltus.formant.0"),
            ];
            controls::caption(ui, s, Src::Key("saltus.l.formants"), w);
            // Keep first: the default, the voice's own.
            let now = usize::from(!ss.formants);
            let name = frame::tr("fx.saltus.formant");
            if let Some(i) = controls::segmented_key(ui, s, host, "formant", now, &labels, name, w)
            {
                host.set("formant", if i == 0 { 1.0 } else { 0.0 }, None);
            }
        });
        labelled(host, ui, s, "throat", "saltus.l.throat", None);
    });
}

/// The mix: the dry signal and the output.
fn mix_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("saltus.card.mix"), None, |ui, _| {
        labelled(host, ui, s, "dry", "saltus.l.dry", None);
        labelled(host, ui, s, "out", "saltus.l.output", None);
    });
}

/// Harmony: where its degrees are counted, the key and scale, the notes' track.
fn harmony_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ss: &SaltusSettings,
) {
    controls::card(ui, s, rect, Src::Key("saltus.card.harmony"), None, |ui, _| {
        let w = 232.0;
        controls::column(ui, s, w, |ui| {
            controls::caption(ui, s, Src::Key("saltus.l.counts"), w);
            let src = ss.harmony_source as usize;
            field_of(host, ui, s, "hsrc", &words::HSRC, src, w, w);
            if src == 0 {
                ui.horizontal(|ui| {
                    let kw = 72.0;
                    let key = ss.key as usize;
                    field_of(host, ui, s, "key", &words::KEYS, key, kw, 96.0);
                    let sc = ss.scale as usize;
                    field_of(host, ui, s, "scale", &words::SCALES, sc, w - kw - 4.0, 170.0);
                });
            } else {
                midi_source(host, ui, s, w);
            }
        });
    });
}

/// In Cantus, the track whose notes Saltus hears (the chord, the melody, the whammy's
/// notes); in another host, where the notes come from.
fn midi_source(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, w: f32) {
    let Some((app, strip, effect)) = host.daw() else {
        controls::line(ui, s, frame::tr("saltus.midi_host"), s.t.text_dim, w);
        return;
    };
    let current = app
        .project()
        .effect(strip, effect)
        .and_then(|e| e.midi_from);
    let name = current
        .and_then(|id| app.project().track(id))
        .map(|t| t.name.as_str())
        .filter(|n| !n.is_empty());
    let id = ui.id().with(("saltus-midi", effect.0));
    let value = name.unwrap_or(frame::tr("saltus.midi_none"));
    if let Some(at) = controls::field(ui, s, id, value, frame::tr("saltus.midi_tip"), w) {
        let tracks: Vec<(TrackId, String)> = app
            .project()
            .tracks
            .iter()
            .map(|t| (t.id, t.name.clone()))
            .collect();
        let mut items = vec![frame::radio(
            Src::Key("saltus.midi_none"),
            current.is_none(),
        )];
        items.extend(
            tracks
                .iter()
                .map(|(tid, n)| frame::radio(Src::Str(n), current == Some(*tid))),
        );
        if let Some(i) = controls::list(ui, s, id, at, w, &items) {
            let source = if i == 0 { None } else { Some(tracks[i - 1].0) };
            if source != current {
                app.exec(Command::SetEffectMidiSource {
                    strip,
                    effect,
                    source,
                });
            }
        }
    }
}

/// The octaves: two down, one down, one up (lowest first, as the keyboard reads).
fn octaves_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("saltus.card.octaves"), None, |ui, _| {
        for (key, label) in [
            ("sub2", "saltus.l.down2"),
            ("sub1", "saltus.l.down1"),
            ("up1", "saltus.l.up1"),
        ] {
            wide(host, ui, s, key, label);
        }
    });
}

/// The octave's style, analog or clean, with what each is in a line.
fn style_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ss: &SaltusSettings,
) {
    controls::card(ui, s, rect, Src::Key("saltus.card.style"), None, |ui, _| {
        let w = 330.0;
        controls::column(ui, s, w, |ui| {
            let labels = [
                Src::Key("fx.saltus.ostyle.0"),
                Src::Key("fx.saltus.ostyle.1"),
            ];
            controls::segmented_param(ui, s, host, "ostyle", &labels, w);
            let about = ["saltus.style.0.about", "saltus.style.1.about"][ss.octave_style as usize];
            controls::line(ui, s, frame::tr(about), s.t.text_dim, w);
        });
    });
}

/// The whammy's pedal: its position, the heel's and the toe's intervals, its glide.
fn pedal_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("saltus.card.pedal"), None, |ui, _| {
        labelled(host, ui, s, "pedal", "saltus.l.pedal", None);
        labelled(
            host,
            ui,
            s,
            "heel",
            "saltus.l.heel",
            Some(&words::semitone_pill),
        );
        labelled(
            host,
            ui,
            s,
            "toe",
            "saltus.l.toe",
            Some(&words::semitone_pill),
        );
        labelled(host, ui, s, "glide", "saltus.l.glide", None);
    });
}

/// What plays the whammy: the pedal, or the notes held on MIDI (and their track).
fn played_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ss: &SaltusSettings,
) {
    controls::card(ui, s, rect, Src::Key("saltus.card.played"), None, |ui, _| {
        let w = 300.0;
        controls::column(ui, s, w, |ui| {
            let labels = [Src::Key("fx.saltus.wsrc.0"), Src::Key("fx.saltus.wsrc.1")];
            controls::segmented_param(ui, s, host, "wsrc", &labels, w);
            if ss.whammy_notes {
                midi_source(host, ui, s, w);
            } else {
                controls::line(ui, s, frame::tr("saltus.played.0.about"), s.t.text_dim, w);
            }
        });
    });
}

/// The inspector's picture: the mode and what it does, in a line ("Shift · +12 st, −12 st").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let ss = cantus_engine::fx::saltus::settings(spec);
    let mut w = String::from(frame::tr(words::MODES[ss.mode as usize]));
    words::summary(&ss, &mut w);
    controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The settings `--editor saltus` and the screenshot tool open it with: the starting
/// preset, the twelve-string.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "saltus").set_many(DEMO, "cmd.fx_preset");
}

pub const DEMO: &[(&str, f32)] = &[
    ("engine", 1.0),
    ("st1", 12.0),
    ("lvl1", 0.6),
    ("dry", 1.0),
    ("out", -1.3),
];

/// What a playing engine would feed the editor, for screenshots with no engine: a sung A3
/// a few cents flat, with a vibrato, through the real unit at the editor's settings — and
/// for the MIDI sources, an A minor chord held — its telemetry as the engine sends it.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let tel = demo_telemetry(&spec, &[57, 60, 64]);
    app.fx.injected = Some(tel);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.saltus.effect != Some(effect) {
            app.fx.saltus = State {
                effect: Some(effect),
                ..State::default()
            };
        }
        let ss = cantus_engine::fx::saltus::settings(&spec);
        let r = cantus_engine::fx::saltus::read(&tel);
        app.fx.saltus.view.seed(&r, &ss);
    }
}

/// The telemetry of Saltus at `spec` after a second of a sung A3 (and `held` notes held).
pub fn demo_telemetry(spec: &EffectSpec, held: &[u8]) -> [f32; 64] {
    use cantus_engine::FxUnit as _;
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::saltus::Unit::new(sr);
    u.configure(spec);
    for &p in held {
        u.note(
            0,
            cantus_engine::FxNote::On {
                pitch: p,
                velocity: 96,
            },
        );
    }
    let f0 = 220.0 * cantus_dsp::math::exp2(-6.0 / 1200.0);
    let mut phase = 0.0f32;
    let n = 480;
    for blk in 0..100 {
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for i in 0..n {
            let t = (blk * n + i) as f32 / sr;
            let vib = 1.0 + 0.0025 * cantus_dsp::math::sin(t * 5.5 * std::f32::consts::TAU);
            phase = (phase + f0 * vib / sr).fract();
            let p = phase * std::f32::consts::TAU;
            let x = 0.32 * cantus_dsp::math::sin(p)
                + 0.16 * cantus_dsp::math::sin(2.0 * p)
                + 0.08 * cantus_dsp::math::sin(3.0 * p);
            l[i] = x;
            r[i] = x;
        }
        u.process(&mut l, &mut r, None);
    }
    u.telemetry()
}
