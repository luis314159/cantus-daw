//! The selected pad's display: its rendered hit, its amplitude envelope and its pitch on a
//! log-Hz axis, with the envelope's corners and the pitch curve as handles. Drawn with the
//! kit's brush in tokens, as the display of the hit card (ADR 0024 §8).

use super::{audition, set_field};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::with_alpha;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style, Tok};
use cantus_core::{pitch_name, DrumPad, PadField, PadVoice, PulsusParams, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

/// A pad's hit, rendered through the real DSP on the interface thread.
#[derive(Clone, Debug)]
pub struct Preview {
    /// Seconds the columns span.
    pub seconds: f32,
    /// `(min, max)` of the mono hit per column.
    pub columns: Vec<(f32, f32)>,
    /// The hit itself at each column's start: the line a kick's cycles are drawn with.
    pub line: Vec<f32>,
    pub peak: f32,
}

const COLUMNS: usize = 480;
/// Previews render at half the engine rate: they are pictures, and a drag re-renders one
/// every frame.
const SR: f32 = 24_000.0;

/// What the display spans for a pad: its decay and a little more, 0.15 … 4 s.
pub fn span(pad: &DrumPad) -> f32 {
    let open = if pad.voice == PadVoice::HiHat && pad.openness > 0.0 {
        pad.open_decay.max(pad.decay)
    } else {
        pad.decay
    };
    let tail = match pad.voice {
        PadVoice::Snare => open.max(pad.noise_decay),
        PadVoice::Clap => open + pad.spacing_ms / 1000.0 * pad.bursts as f32,
        _ => open,
    };
    (tail * 1.15 + (pad.attack_ms + pad.hold_ms) / 1000.0).clamp(0.15, 4.0)
}

pub(super) fn preview_key(pad: &DrumPad, version: u8) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(pad).unwrap_or_default().hash(&mut h);
    version.hash(&mut h);
    h.finish()
}

/// Render one hit of `pad` at velocity 110 through `cantus_dsp::Pulsus`, as the engine
/// would, and reduce it to columns.
pub fn render_preview(
    pad: &DrumPad,
    version: u8,
    samples: &mut cantus_engine::SampleCache,
) -> Preview {
    use cantus_dsp::pulsus::{Hit, LayerSettings};
    let seconds = span(pad);
    let mut kit = cantus_dsp::Pulsus::new(SR);
    kit.set_kit(version, 1, 4, false);
    kit.set_pad_count(1);
    kit.set_pad(0, pad.pitch, &cantus_engine::pulsus_kit::settings(pad, 0));
    let load = |samples: &mut cantus_engine::SampleCache, path: &str| {
        samples
            .load(std::path::Path::new(path))
            .ok()
            .map(|d| cantus_dsp::SampleRef {
                frames: d.frames.clone(),
                rate: d.rate as f32,
                channels: d.channels as usize,
            })
    };
    if let Some(path) = pad.sample.as_deref() {
        let s = load(samples, path);
        kit.set_sample(0, s);
    }
    for (k, layer) in pad.layers.iter().take(cantus_core::MAX_LAYERS).enumerate() {
        let settings = LayerSettings {
            vel_lo: layer.vel_lo,
            vel_hi: layer.vel_hi,
            xfade: layer.xfade,
            tune: layer.tune,
            gain: cantus_core::db_to_linear(layer.level_db),
            pan: layer.pan,
            start: layer.start_ms / 1000.0,
            reverse: layer.reverse,
        };
        let s = load(samples, &layer.sample);
        kit.set_layer(0, k, &settings, s);
    }
    kit.trigger(
        pad.pitch,
        110,
        Hit {
            id: 1,
            tick: 0,
            ordinal: 0,
            sequenced: true,
        },
    );
    let n = (seconds * SR) as usize;
    let per = (n / COLUMNS).max(1);
    let mut columns = Vec::with_capacity(COLUMNS);
    let mut line = Vec::with_capacity(COLUMNS);
    let (mut l, mut r) = (vec![0.0f32; per], vec![0.0f32; per]);
    let mut peak = 0.0f32;
    for _ in 0..COLUMNS {
        l.fill(0.0);
        r.fill(0.0);
        kit.process_add(&mut l, &mut r);
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for (a, b) in l.iter().zip(&r) {
            let m = 0.5 * (a + b);
            lo = lo.min(m);
            hi = hi.max(m);
        }
        peak = peak.max(hi).max(-lo);
        columns.push((lo, hi));
        line.push(0.5 * (l[0] + r[0]));
    }
    Preview {
        seconds,
        columns,
        line,
        peak,
    }
}

/// The pad's fundamental over time, Hz, where the voice has one: Kick 3's curve.
pub fn pitch_at(pad: &DrumPad, version: u8, t: f32) -> Option<f32> {
    let fall = |seconds: f32| 10f32.powf(-3.0 * t / seconds.max(0.0005));
    let penv = 2f32.powf(pad.pitch_env * fall(pad.pitch_env_ms / 1000.0) / 12.0);
    let tuned = |base: f32| base * 2f32.powf(pad.tune / 12.0);
    let model = pad.model_index();
    Some(match pad.voice {
        PadVoice::Kick => {
            let base = tuned(if version <= 1 && model == 0 {
                45.0
            } else {
                cantus_dsp::pulsus::KICK_F1_HZ
            });
            let p = fall(pad.pitch_decay);
            match model {
                0 => {
                    let span = 2f32.powf(pad.pitch_drop / 12.0) - 1.0;
                    base * (1.0 + span * p + 2.5 * pad.click * fall(0.008))
                }
                1 => {
                    let span = 2f32.powf(pad.pitch_drop / 12.0) - 1.0;
                    let e = fall(pad.decay);
                    base * (1.0 + span * p) * (1.0 + 0.12 * pad.sigh * e * e)
                }
                _ => base * 2f32.powf(pad.pitch_drop * p / 12.0),
            }
        }
        PadVoice::Snare if model == 0 => tuned(180.0) * (1.0 + 0.35 * fall(0.015)) * penv,
        PadVoice::Snare => tuned(180.0) * penv,
        PadVoice::Tom => {
            let e = fall(pad.decay);
            let glide = 2f32.powf(pad.glide / 12.0) - 1.0;
            tuned(220.0) * (1.0 + glide * e * e) * penv
        }
        _ => return None,
    })
}

/// `43.7 Hz = F1 −1 ct`.
pub fn tuned_readout(hz: f32) -> String {
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    let note = midi.round().clamp(0.0, 127.0);
    let cents = ((midi - note) * 100.0).round();
    tf(
        "pulsus.tuned",
        &[
            ("hz", &format!("{hz:.1} Hz")),
            ("note", &pitch_name(note as u8)),
            ("cents", &format!("{cents:+.0} ct")),
        ],
    )
}

/// The amplitude envelope at `t`, 0…1: attack, hold, then the decay with its curve.
fn env_at(pad: &DrumPad, t: f32) -> f32 {
    let a = pad.attack_ms / 1000.0;
    let h = pad.hold_ms / 1000.0;
    if t < a {
        return t / a.max(1e-6);
    }
    if t < a + h {
        return 1.0;
    }
    let d = (t - a - h) / pad.decay.max(0.001);
    let exp = 10f32.powf(-3.0 * d);
    if pad.curve > 0.0 {
        (1.0 - pad.curve) * exp + pad.curve * (1.0 - d).max(0.0)
    } else if pad.curve < 0.0 {
        let c = -pad.curve;
        (1.0 - c) * exp + c * exp * exp * exp
    } else {
        exp
    }
}

fn log_y(rect: Rect, hz: f32) -> f32 {
    let p = ((hz.max(20.0) / 20.0).log10() / 3.0).clamp(0.0, 1.0);
    rect.bottom() - p * rect.height()
}

fn hz_at(rect: Rect, y: f32) -> f32 {
    20.0 * 10f32.powf(3.0 * ((rect.bottom() - y) / rect.height()).clamp(0.0, 1.0))
}

/// The display in `rect` (the hit card's display, bleeding to its edges): the hit, its
/// envelope and its pitch on a log-Hz axis, the readout of what the tail is tuned to;
/// click to hear, drag the handles to shape it.
pub(super) fn show(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let k = s.k;
    let response = ui.allocate_rect(rect, Sense::click());
    let plot = Rect::from_min_max(
        pos2(rect.left() + 14.0 * k, rect.top() + 32.0 * k),
        pos2(rect.right() - 44.0 * k, rect.bottom() - 18.0 * k),
    );
    let preview = app
        .pulsus
        .preview(&mut app.engine.samples, pad, kit.version)
        .clone();
    let secs = preview.seconds;
    let x_of = |t: f32| plot.left() + (t / secs).clamp(0.0, 1.0) * plot.width();
    let pitch: Option<Vec<Pos2>> = (0..=160)
        .map(|i| {
            let t = secs * i as f32 / 160.0;
            pitch_at(pad, kit.version, t).map(|hz| pos2(x_of(t), log_y(plot, hz)))
        })
        .collect();
    let readout = match pitch_at(pad, kit.version, secs) {
        Some(hz) => tuned_readout(hz),
        None if pad.voice == PadVoice::Sample => pad
            .sample
            .as_deref()
            .or(pad.layers.first().map(|l| l.sample.as_str()))
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| t("pad.sample_none")),
        None => crate::widgets::knob::readout(pad.tone, "Hz"),
    };
    with_brush(&ui.painter_at(rect), |b| {
        paint(b, rect, plot, s, pad, &preview, pitch.as_deref(), &readout);
    });
    handles(app, ui, track, pad, plot, secs, pitch.as_deref(), s);
    if response.clicked() {
        audition(app, track, pad.pitch, 110);
    }
    if response.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("pulsus.audition"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint(
    b: &mut Brush,
    rect: Rect,
    plot: Rect,
    s: &Style,
    pad: &DrumPad,
    preview: &Preview,
    pitch: Option<&[Pos2]>,
    readout: &str,
) {
    let t = &s.t;
    let k = s.k;
    b.register(
        rect,
        "pulsus.display",
        &[Tok::Ink, Tok::Grid, Tok::Text, Tok::TextDim],
        &[s.hue_tok],
    );
    let secs = preview.seconds;
    let x_of = |tt: f32| plot.left() + (tt / secs).clamp(0.0, 1.0) * plot.width();
    // Grid: time every tenth of the span, frequencies at the decades and their halves.
    for i in 1..10 {
        let x = plot.left() + plot.width() * i as f32 / 10.0;
        b.vline(x, plot.top(), plot.bottom(), b.px(), t.grid);
    }
    for (hz, label) in [
        (50.0f32, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1_000.0, "1k"),
        (2_000.0, "2k"),
        (5_000.0, "5k"),
        (10_000.0, "10k"),
    ] {
        let y = log_y(plot, hz);
        b.hline(plot.left(), plot.right(), y, b.px(), t.grid);
        let lr = Rect::from_min_size(
            pos2(plot.right() + 10.0 * k, y - 6.0 * k),
            vec2(30.0 * k, 12.0 * k),
        );
        let _ = ktext::paint(
            b,
            lr,
            &Text::new(label, Role::Micro, t.text_dim)
                .caps(false)
                .align(Align2::LEFT_CENTER),
            s.tk(),
        );
    }
    // The hit in neutral ink, so the one hue on the display is the pitch curve.
    let mid = plot.center().y;
    let scale = plot.height() * 0.48 / preview.peak.max(1e-4);
    let fill = with_alpha(t.text, if t.dark { 0.12 } else { 0.10 });
    let w = plot.width() / preview.columns.len().max(1) as f32;
    for (i, (lo, hi)) in preview.columns.iter().enumerate() {
        let x = plot.left() + i as f32 * w;
        b.rect(
            Rect::from_min_max(
                pos2(x, mid - hi * scale),
                pos2(x + w, mid - lo * scale + b.px()),
            ),
            Fill::Solid(fill),
        );
    }
    let wave: Vec<Pos2> = preview
        .line
        .iter()
        .enumerate()
        .map(|(i, v)| pos2(plot.left() + (i as f32 + 0.5) * w, mid - v * scale))
        .collect();
    b.polyline(&wave, 1.0 * k, with_alpha(t.text, 0.45));
    // The amplitude envelope over it.
    let env: Vec<Pos2> = (0..=160)
        .map(|i| {
            let tt = secs * i as f32 / 160.0;
            pos2(x_of(tt), plot.bottom() - env_at(pad, tt) * plot.height())
        })
        .collect();
    // In the face's second voice (Pulsus's orange beside its yellow pitch), the primary
    // text on the kit's plain look.
    b.polyline(&env, 1.25 * k, crate::views::inst::face::voice2(s, t.text));
    // The pitch, on the log-Hz axis.
    if let Some(points) = pitch {
        b.polyline(points, 2.5 * k, s.hue);
    }
    // The readout: what the tail is tuned to, or where the band sits.
    let head = Rect::from_min_size(
        rect.min + vec2(12.0 * k, 5.0 * k),
        vec2(rect.width() - 24.0 * k, 16.0 * k),
    );
    let line = format!("{}  \u{b7}  {}", pad.name, readout);
    let _ = ktext::paint(
        b,
        head,
        &Text::new(&line, Role::Title, t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
    for f in [0.0f32, 0.5, 1.0] {
        let label = crate::widgets::knob::readout(secs * f * 1_000.0, "ms");
        let x = plot.left() + f * plot.width();
        let lw = 60.0 * k;
        let (lx, align) = if f == 0.0 {
            (x, Align2::LEFT_CENTER)
        } else if f == 1.0 {
            (x - lw, Align2::RIGHT_CENTER)
        } else {
            (x - lw * 0.5, Align2::CENTER_CENTER)
        };
        let lr = Rect::from_min_size(pos2(lx, plot.bottom() + 3.0 * k), vec2(lw, 12.0 * k));
        let _ = ktext::paint(
            b,
            lr,
            &Text::new(&label, Role::Micro, t.text_dim)
                .caps(false)
                .align(align),
            s.tk(),
        );
    }
}

/// The envelope's corners and the pitch curve's two ends, dragged.
#[allow(clippy::too_many_arguments)]
fn handles(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    plot: Rect,
    secs: f32,
    pitch: Option<&[Pos2]>,
    s: &Style,
) {
    let k = s.k;
    let x_of = |t: f32| plot.left() + (t / secs).clamp(0.0, 1.0) * plot.width();
    let t_of = |x: f32| ((x - plot.left()) / plot.width()).clamp(0.0, 1.0) * secs;
    let a = pad.attack_ms / 1000.0;
    let h = pad.hold_ms / 1000.0;
    let mut spots: Vec<(Pos2, PadField, &str)> = vec![
        (pos2(x_of(a), plot.top()), PadField::Attack, "A"),
        (
            pos2(x_of(a + h), plot.top() + 16.0 * k),
            PadField::Hold,
            "H",
        ),
        (
            pos2(x_of(a + h + pad.decay), plot.bottom() - 8.0 * k),
            PadField::Decay,
            "D",
        ),
    ];
    if let Some(points) = pitch {
        let drop = if pad.voice == PadVoice::Kick {
            PadField::PitchDrop
        } else {
            PadField::PitchEnv
        };
        spots.push((points[0], drop, "P"));
        spots.push((*points.last().unwrap(), PadField::Tune, "T"));
    }
    for (i, (at, field, glyph)) in spots.into_iter().enumerate() {
        let r = Rect::from_center_size(at, vec2(14.0, 14.0) * k);
        let id = ui.id().with(("pulsus-handle", track.0, i));
        let resp = ui.interact(r, id, Sense::drag());
        let hot = resp.hovered() || resp.dragged();
        // A ring with its letter inside: the handle names itself without a word beside it.
        with_brush(ui.painter(), |b| {
            let t = &s.t;
            let r = (if hot { 7.5 } else { 7.0 }) * k;
            b.circle(at, r, Fill::Solid(t.ink));
            b.circle_stroke(at, r, 1.5 * k, if hot { s.hue } else { t.text });
            let lr = Rect::from_center_size(at, vec2(10.0 * k, 10.0 * k));
            let _ = ktext::paint(b, lr, &Text::new(glyph, Role::Micro, t.text), s.tk());
        });
        if hot {
            let name = t(field.info(pad.voice).label_key);
            crate::kit::hint::set_text(ui.ctx(), &name, "", crate::kit::hint::Caps::PARAM);
        }
        if resp.dragged() {
            let p = resp.interact_pointer_pos().unwrap_or(at);
            let value = match field {
                PadField::Attack => t_of(p.x) * 1000.0,
                PadField::Hold => (t_of(p.x) - a).max(0.0) * 1000.0,
                PadField::Decay => (t_of(p.x) - a - h).max(0.01),
                PadField::Tune => {
                    let now = pitch
                        .and_then(|pts| pts.last())
                        .map(|q| hz_at(plot, q.y))
                        .unwrap_or(100.0);
                    pad.tune + 12.0 * (hz_at(plot, p.y) / now).log2()
                }
                _ => {
                    // The sweep's start: semitones above the tail.
                    let tail = pitch
                        .and_then(|pts| pts.last())
                        .map(|q| hz_at(plot, q.y))
                        .unwrap_or(50.0);
                    12.0 * (hz_at(plot, p.y) / tail).log2()
                }
            };
            set_field(app, track, pad.id, field, value, true);
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
    }
}
