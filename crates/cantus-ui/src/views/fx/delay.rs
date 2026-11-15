//! Delay's editor: the stereo delay (round 39, ADR 0115).
//!
//! Delay was the one effect with no window of its own: it was edited under its insert in
//! the inspector. Its window is the echoes drawn in time — each repeat at its time, its
//! level falling by the feedback, on the left or the right when ping-pong is on, its colour
//! cooling as the tone takes its highs — and the display is the control: a drag across sets
//! the time, up and down the feedback. Beside it, what is heard (the repeats to −60 dB, the
//! tail, the tone's loss per repeat, the first repeat's side); under it the time (in
//! milliseconds, or a note value following the tempo — round 40, ADR 0119), the repeats
//! (feedback and tone), and the output (ping-pong and the mix).
//!
//! **The tempo** (round 40): *At the tempo*, which set the time once from a note value,
//! became the sync. Its column has the switch (*Free · Sync*), the note value and what the
//! time is at the song's tempo now. Choosing a note turns the sync on; turning it on picks
//! the note nearest the time it was playing, and turning it off keeps the time it was
//! playing in milliseconds — so the old *set once* is *Sync, then Free* and nothing is lost.
//!
//! Nothing here touches the sound: the unit is `cantus_dsp::delay::StereoDelay`.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::prose;
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{EffectId, EffectKind, StripId};
use egui::{Align2, Rect, Vec2};
use std::fmt::Write as _;

#[path = "delay_view.rs"]
mod view;
use cantus_dsp::delay::sync;
pub use view::{
    bpm_of, demo_feed, demo_setup, dragged_to, echoes, heard_at, held, nearest_note, note_name,
    note_of, note_pill, pass_gain, preview, repeats_heard, settings, span_for, synced_ms, Echo,
    Heard, Plot, Side, DIV_KEYS, NOTE_KEYS, REF_HZ,
};

pub(crate) const KIND: EffectKind = EffectKind::Delay;

/// Factory presets: data (`presets/effects/delay.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// After this long with nothing from the tap, what was heard is forgotten: the repeats stop
/// glowing (the strip went quiet and rests).
const STALE_S: f64 = 0.5;

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// Where the display was drawn last frame.
    pub display: Option<Rect>,
    /// The drag on the display, while one is under way.
    drag: Option<view::Drag>,
    gesture: u64,
    /// The repeats drawn last frame (kept, so a frame allocates none).
    echoes: Vec<Echo>,
    /// The display's words and what they were made from.
    words: Option<(WordsKey, String, String)>,
    /// The side card's values and what they were made from.
    heard_words: Option<(HeardKey, [String; 4])>,
    /// The input's level lately, for the repeats' glow while sound plays.
    heard: Heard,
    /// When the tap last delivered (the window's clock), `None` before it ever has.
    last_seen: Option<f64>,
    pairs: Vec<(f32, f32)>,
    /// Whether the window moved with the sound last frame (it then asks for the next).
    pub live: bool,
}

type WordsKey = (u32, u64, bool, crate::i18n::Locale, Option<usize>);
type HeardKey = (u32, u32, u32, bool, crate::i18n::Locale);

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.delay);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "delay"), &mut st, ui);
    app.fx.delay = st;
    out
}

pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let sr = host.sample_rate();
    // What the tap delivered since the last frame: the input's level, for the glow.
    let mut pairs = std::mem::take(&mut st.pairs);
    pairs.clear();
    host.take_pairs(&mut pairs);
    let now = ui.input(|i| i.time);
    let sounding = st.heard.push(&pairs, sr.max(1.0));
    if !pairs.is_empty() {
        st.last_seen = Some(now);
    } else if st.last_seen.is_some_and(|t| now - t > STALE_S) {
        st.heard.clear();
        st.last_seen = None;
    }
    let came = !pairs.is_empty();
    st.pairs = pairs;
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let mut glowing = false;
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        glowing = body_ui(host, st, ui, s, body);
    });
    st.chrome = chrome;
    st.chips = chips;
    // Moving with the sound: only while the tap brings it (ADR 0106 — at rest, no frame).
    st.live = came && (sounding || glowing);
    frame::Out {
        at_rest: !st.live,
        ..out
    }
}

/// The side card's width at 100 %.
const SIDE_W: f32 = 262.0;
/// The "at the tempo" column's width at 100 %.
const TEMPO_W: f32 = 150.0;
/// The ping-pong switch's width at 100 %.
const SWITCH_W: f32 = 96.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
) -> bool {
    let k = s.k;
    let gap = s.d(4.0);
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(180.0 * k);
    let total = area.width() - gap;
    let sw = SIDE_W * k;
    let r1: Vec<Rect> = g.row(row1, &[(total - sw) / total, sw / total]).collect();
    let sp = s.function(Function::Space);
    let disp = (r1[0].height() / k - crate::kit::band::TITLE - 1.0).max(20.0);
    let mut glowing = false;
    controls::card_with(
        ui,
        &sp,
        r1[0],
        Src::Key("delay.card.echoes"),
        None,
        Some(disp),
        |ui, cr| {
            if let Some(d) = cr.display {
                st.display = Some(d);
                glowing = view::display(host, st, ui, &sp, d);
            }
        },
    );
    heard_card(host, st, ui, &s.function(Function::Output), r1[1]);

    let w = [
        controls::cells_width(&[Size::XL]) + 8.0 + TEMPO_W + 16.0,
        controls::cells_width(&[Size::L, Size::M]) + 16.0,
        SWITCH_W + 8.0 + controls::cells_width(&[Size::M]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        &sp,
        r2[0],
        Src::Key("delay.card.time"),
        None,
        |ui, _| {
            // One knob that changes with the switch: the note value while synced.
            if host.spec().get("sync") >= 0.5 {
                controls::param_with(ui, &sp, host, "note", Size::XL, Some(&note_pill));
            } else {
                controls::param_with(
                    ui,
                    &sp,
                    host,
                    "time_ms",
                    Size::XL,
                    Some(&super::modfx::ms_pill),
                );
            }
            ui.add_space(4.0 * k);
            tempo_column(host, ui, &sp);
        },
    );
    let st_ = s.function(Function::Tone);
    controls::card(
        ui,
        &st_,
        r2[1],
        Src::Key("delay.card.repeats"),
        None,
        |ui, _| {
            controls::param_with(
                ui,
                &st_,
                host,
                "feedback",
                Size::L,
                Some(&super::modfx::pct_pill),
            );
            controls::param_with(
                ui,
                &st_,
                host,
                "tone_hz",
                Size::M,
                Some(&super::modfx::hz_pill),
            );
        },
    );
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[2],
        Src::Key("delay.card.output"),
        None,
        |ui, _| {
            controls::column(ui, &so, SWITCH_W, |ui| {
                // Level with the knob's middle beside it.
                ui.add_space(
                    ((ui.available_height() - s.d(controls::ROW)) * 0.5 - 8.0 * k).max(0.0),
                );
                controls::led_param(
                    ui,
                    &so,
                    host,
                    "ping_pong",
                    Src::Key("fx.delay.ping_pong"),
                    SWITCH_W,
                );
            });
            controls::param_with(ui, &so, host, "mix", Size::M, Some(&super::modfx::pct_pill));
        },
    );
    glowing
}

/// The tempo (round 40, ADR 0119): the sync switch, the note value and what the time is at
/// the song's tempo now. *At the tempo*, which set the time once, is this switch now.
fn tempo_column(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style) {
    let bpm = bpm_of(host);
    let spec = host.spec();
    let on = spec.get("sync") >= 0.5;
    let note = sync::note_index(spec.get("note"));
    controls::column(ui, s, TEMPO_W, |ui| {
        ui.add_space(10.0 * s.k);
        controls::caption(ui, s, Src::Key("delay.tempo"), TEMPO_W);
        let labels = [Src::Key("modfx.sync.0"), Src::Key("modfx.sync.1")];
        let name = frame::tr("delay.tempo.sync_tip");
        if let Some(i) =
            controls::segmented_key(ui, s, host, "sync", on as usize, &labels, name, TEMPO_W)
        {
            if (i == 1) != on {
                set_sync(host, i == 1, bpm);
            }
        }
        ui.add_space(4.0 * s.k);
        let id = ui.id().with("delay-note");
        let tip = frame::tr("delay.tempo.note_tip");
        if let Some(at) =
            controls::field_key(ui, s, host, "note", id, note_name(note), tip, TEMPO_W)
        {
            // The list, with each note's time at this tempo, built only while it is open.
            let labels: Vec<String> = (0..sync::NOTES)
                .map(|i| {
                    let mut l = format!("{}  ", note_name(i));
                    controls::write_readout(synced_ms(i, bpm), "ms", &mut l);
                    l
                })
                .collect();
            let items: Vec<_> = labels
                .iter()
                .enumerate()
                .map(|(i, l)| frame::radio(Src::Str(l), on && i == note))
                .collect();
            if let Some(n) = controls::list(ui, s, id, at, TEMPO_W, &items) {
                host.set_many(&[("sync", 1.0), ("note", n as f32)], "cmd.delay_sync_on");
            }
        }
        let words = if on {
            let mut ms = String::new();
            controls::write_readout(synced_ms(note, bpm), "ms", &mut ms);
            let tempo = format!("{bpm:.0}");
            if held(note, bpm) {
                let mut ask = String::new();
                controls::write_readout(sync::time_ms(note, bpm) as f32, "ms", &mut ask);
                crate::i18n::tf(
                    "delay.tempo.held",
                    &[("ms", &ask), ("bpm", &tempo), ("max", &ms)],
                )
            } else {
                crate::i18n::tf("delay.tempo.follows", &[("ms", &ms), ("bpm", &tempo)])
            }
        } else {
            frame::tr("delay.tempo.free_words").to_string()
        };
        paragraph(ui, s, &words, TEMPO_W);
    });
}

/// Turn the sync on or off as one step, keeping what is heard: on, the note value nearest
/// the time it was playing; off, the time it was playing, in milliseconds.
pub fn set_sync(host: &mut dyn FxHost, on: bool, bpm: f64) {
    let spec = host.spec();
    if on {
        let note = nearest_note(spec.get("time_ms"), bpm);
        host.set_many(&[("sync", 1.0), ("note", note as f32)], "cmd.delay_sync_on");
    } else {
        let note = sync::note_index(spec.get("note"));
        let ms = (synced_ms(note, bpm).clamp(1.0, 2_000.0) * 10.0).round() / 10.0;
        host.set_many(&[("sync", 0.0), ("time_ms", ms)], "cmd.delay_sync_off");
    }
}

/// What is heard: how many repeats before −60 dB, the tail they make, what the tone takes
/// from each, and where the first one is — the display's facts in words and numbers.
fn heard_card(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    let ds = heard_at(host.spec(), bpm_of(host));
    let sr = host.sample_rate().max(1.0);
    let key = (
        ds.time_ms.to_bits(),
        ds.feedback.to_bits(),
        ds.tone_hz.to_bits(),
        ds.ping_pong,
        crate::i18n::locale(),
    );
    if st.heard_words.as_ref().is_none_or(|w| w.0 != key) {
        let n = repeats_heard(ds.feedback);
        let mut tail = String::new();
        controls::write_readout(n as f32 * ds.time_ms, "ms", &mut tail);
        let loss = 20.0 * cantus_dsp::math::log10(pass_gain(ds.tone_hz, sr, REF_HZ).max(1e-6));
        let mut db = String::new();
        let _ = write!(db, "{loss:.1} dB");
        let darker = crate::i18n::tf("delay.heard.at", &[("db", &db)]);
        let first = frame::tr(if ds.ping_pong {
            "delay.heard.left"
        } else {
            "delay.heard.both"
        });
        st.heard_words = Some((key, [n.to_string(), tail, darker, first.to_string()]));
    }
    let inner = SIDE_W - 16.0;
    controls::card(ui, s, rect, Src::Key("delay.card.heard"), None, |ui, _| {
        controls::column(ui, s, inner, |ui| {
            let Some((_, words)) = &st.heard_words else {
                return;
            };
            let rows = [
                ("delay.heard.repeats", "delay.heard.repeats_tip", true),
                ("delay.heard.tail", "delay.heard.tail", true),
                ("delay.heard.darker", "delay.heard.darker_tip", true),
                ("delay.heard.first", "delay.heard.first", false),
            ];
            for ((label, tip, numeric), value) in rows.iter().zip(words.iter()) {
                controls::caption(ui, s, Src::Key(label), inner);
                controls::readout(ui, s, value, *numeric, frame::tr(tip), inner);
                ui.add_space(4.0 * s.k);
            }
            let words = frame::tr(if ds.ping_pong {
                "delay.heard.ping_pong"
            } else {
                "delay.heard.straight"
            });
            paragraph(ui, s, words, inner);
        });
    });
}

/// Words broken into lines at `width` (at 100 %), dim, never cut.
fn paragraph(ui: &mut egui::Ui, s: &Style, words: &str, width: f32) {
    let w = width * s.k;
    let lines = prose::wrap(ui.ctx(), words, Role::Body, s.tk(), w);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, lines.height()), egui::Sense::hover());
    if ui.is_rect_visible(r) {
        with_brush(ui.painter(), |b| {
            prose::paint(
                b,
                r,
                &lines,
                Role::Body,
                s.tk(),
                s.t.text_dim,
                Align2::LEFT_TOP,
            );
        });
    }
}
