//! The 808's display: its pitch curve drawn over the waveform it renders, with the
//! harmonics beside it — drawn with the kit's brush in the Source hue.
//!
//! The waveform is rendered here, by the instrument's own DSP at a preview rate, for the
//! settings on screen (and again only when they change), so what is drawn is what plays:
//! the pitch envelope, the knock, the sigh, the decay, the drive and the harmonics. The
//! time axis is a square-root one, so the first 50 ms — where the drop and the knock live —
//! get a third of the width. Drag the curve's start for the drop (semitones) and its knee
//! (the −20 dB point) for its time; double-click either to reset it. The bars at the right
//! are the harmonics the Chebyshev control adds, relative to the fundamental.

use super::{micro, note_name, Grab};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::{self, light, Fill, Style, Tok};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::gravis::{e808::HARMONICS, voice::hz, Engine, Gravis, GravisSettings, VoiceMode};
use cantus_engine::inst::bass::tel;
use egui::{Align2, Pos2, Rect, Sense, Vec2};

const PREVIEW_SR: f32 = 12_000.0;
const WINDOW_MS: f32 = 800.0;
/// The note the picture plays: A1, 55 Hz.
const NOTE: u8 = 33;
const HANDLE_R: f32 = 6.0;
/// The tokens the display paints with, beside the hue (for inspect mode).
const TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::Grid,
    Tok::GridStrong,
    Tok::Text,
    Tok::TextDim,
];

/// The rendered picture and the settings it was made for.
#[derive(Debug)]
pub struct Render {
    settings: GravisSettings,
    /// Samples at the preview rate.
    wave: Vec<f32>,
    /// Semitones above the note, per sample.
    pitch: Vec<f32>,
    peak: f32,
}

fn render(settings: GravisSettings) -> Render {
    let mut s = settings;
    s.engine = Engine::E808;
    s.mode = VoiceMode::Legato;
    s.duck_on = false;
    s.gain_db = 0.0;
    let mut g = Gravis::new(PREVIEW_SR);
    g.set(&s);
    g.note_on(NOTE, 1, 1.0);
    let n = (WINDOW_MS * 0.001 * PREVIEW_SR) as usize;
    let base = hz(NOTE as f64 + s.transpose_st as f64, s.a4 as f64);
    let mut wave = Vec::with_capacity(n);
    let mut pitch = Vec::with_capacity(n);
    let (mut l, mut r) = ([0.0f32; 1], [0.0f32; 1]);
    for _ in 0..n {
        l[0] = 0.0;
        r[0] = 0.0;
        g.process(&mut l, &mut r);
        wave.push(l[0]);
        let f0 = g.readout().f0 as f64;
        pitch.push(if f0 > 0.0 {
            (12.0 * (f0 / base).log2()) as f32
        } else {
            0.0
        });
    }
    let peak = wave.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-6);
    Render {
        settings,
        wave,
        pitch,
        peak,
    }
}

/// Where a time (ms) and a pitch offset (st) sit.
#[derive(Copy, Clone)]
struct Geo {
    plot: Rect,
    top_st: f32,
}

impl Geo {
    fn x(&self, ms: f32) -> f32 {
        self.plot.left() + (ms.max(0.0) / WINDOW_MS).sqrt() * self.plot.width()
    }
    fn ms(&self, x: f32) -> f32 {
        let f = ((x - self.plot.left()) / self.plot.width()).clamp(0.0, 1.0);
        f * f * WINDOW_MS
    }
    fn y(&self, st: f32) -> f32 {
        self.plot.bottom() - (st / self.top_st).clamp(-0.05, 1.05) * self.plot.height()
    }
    fn st(&self, y: f32) -> f32 {
        (self.plot.bottom() - y) / self.plot.height() * self.top_st
    }
}

/// The display in `d`, painted, with its two handles. `s` is in the Source hue.
pub fn display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
) {
    let settings = cantus_engine::inst::bass::settings(spec);
    let fresh = app
        .inst
        .bass
        .render
        .as_ref()
        .is_some_and(|r| r.settings == settings);
    if !fresh {
        app.inst.bass.render = Some(render(settings));
    }
    let resp = ui.interact(
        d,
        ui.id().with(("gravis-808", track.0)),
        Sense::click_and_drag(),
    );
    let k = s.k;
    let bars_w = (d.width() * 0.2).max(130.0 * k);
    let plot = Rect::from_min_max(
        d.min + Vec2::new(12.0 * k, 22.0 * k),
        Pos2::new(d.right() - bars_w - 46.0 * k, d.bottom() - 18.0 * k),
    );
    let e = &settings.e808;
    let knock_st = cantus_dsp::gravis::e808::KNOCK_ST * e.knock;
    let top_st = ((e.pitch_st + knock_st + 1.0) / 12.0).ceil().max(1.0) * 12.0;
    let geo = Geo { plot, top_st };
    // The two handles: the drop at the start, the knee at −20 dB (a third of the time).
    let amount_pos = Pos2::new(geo.x(0.0), geo.y(e.pitch_st));
    let knee_pos = Pos2::new(geo.x(e.pitch_ms / 3.0), geo.y(e.pitch_st * 0.1));
    let grab = app.inst.bass.grab;
    let hover = resp.hover_pos();
    let tele = app.inst_telemetry();
    let playing = tele[super::super::TEL_VOICES] > 0.0 && tele[tel::ENGINE] == 0.0;
    let note = note_name(NOTE as f32 + settings.transpose_st);
    let Some(r) = app.inst.bass.render.as_ref() else {
        return;
    };
    kit::with_brush(&ui.painter_at(d), |b| {
        b.register(d, "gravis.808", TOKENS, &[s.hue_tok]);
        let t = &s.t;
        // The grid: times on the square-root axis, semitones at the right of the plot.
        for ms in [5.0f32, 20.0, 50.0, 100.0, 200.0, 400.0, 800.0] {
            let x = geo.x(ms);
            b.vline(b.snap_mid(x), plot.top(), plot.bottom(), b.px(), t.grid);
            let lr = Rect::from_center_size(
                Pos2::new(x.min(plot.right() - 16.0 * k), plot.bottom() + 9.0 * k),
                Vec2::new(40.0 * k, 12.0 * k),
            );
            micro(
                b,
                lr,
                &format!("{ms:.0} ms"),
                t.text_dim,
                Align2::CENTER_CENTER,
                s,
            );
        }
        let mut st = 0.0;
        while st <= top_st + 0.1 {
            let y = geo.y(st);
            b.hline(plot.left(), plot.right(), b.snap_mid(y), b.px(), t.grid);
            let lr = Rect::from_min_size(
                Pos2::new(plot.right() + 4.0 * k, y - 6.0 * k),
                Vec2::new(40.0 * k, 12.0 * k),
            );
            let text = if st == 0.0 {
                note.clone()
            } else {
                format!("+{st:.0} st")
            };
            micro(b, lr, &text, t.text_dim, Align2::LEFT_CENTER, s);
            st += 12.0;
        }
        // The waveform: each column's range, the neighbour's first sample included, so
        // the band is continuous where a column holds less than a sample.
        let mid = plot.center().y;
        let amp = plot.height() * 0.46 / r.peak;
        let cols = plot.width().max(1.0) as usize;
        let band = with_alpha(if t.dark { t.text_dim } else { t.label }, 0.30);
        let step = (1.0 / b.ppp()).max(1.0);
        let mut c = 0.0f32;
        while (c as usize) < cols {
            let x0 = plot.left() + c;
            let a = ((geo.ms(x0) * 0.001 * PREVIEW_SR) as usize).min(r.wave.len() - 1);
            let bb = ((geo.ms(x0 + step) * 0.001 * PREVIEW_SR) as usize).clamp(a + 1, r.wave.len());
            let (lo, hi) = r.wave[a.saturating_sub(1)..bb]
                .iter()
                .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
            let col = Rect::from_min_max(
                Pos2::new(x0, mid - hi * amp),
                Pos2::new(x0 + step, (mid - lo * amp).max(mid - hi * amp + b.px())),
            );
            b.rect(col, Fill::Solid(band));
            c += step;
        }
        // The pitch curve.
        let n = (plot.width() / 2.0).clamp(16.0, 400.0) as usize;
        let mut pts = [Pos2::ZERO; 400];
        for (i, p) in pts.iter_mut().take(n).enumerate() {
            let x = plot.left() + plot.width() * i as f32 / (n - 1) as f32;
            let j = ((geo.ms(x) * 0.001 * PREVIEW_SR) as usize).min(r.pitch.len() - 1);
            *p = Pos2::new(x, geo.y(r.pitch[j]));
        }
        b.polyline(&pts[..n], 2.0 * k, s.hue);
        // The playhead, while a note plays.
        if playing {
            let x = geo.x(tele[tel::AGE_MS]);
            if x < plot.right() {
                b.vline(b.snap_mid(x), plot.top(), plot.bottom(), b.px(), t.text);
            }
        }
        for (pos, which) in [(amount_pos, Grab::PitchAmount), (knee_pos, Grab::PitchTime)] {
            let hot =
                grab == Some(which) || hover.is_some_and(|h| h.distance(pos) < HANDLE_R * 2.0 * k);
            if hot {
                light::glow(b, pos, 16.0 * k, s.hue, 0.8);
            }
            b.circle(pos, HANDLE_R * k, Fill::Solid(s.hue));
            b.circle_stroke(pos, HANDLE_R * k, k, shade(s.hue, -0.25));
        }
        let readout = |b: &mut kit::Brush, at: Pos2, text: &str| {
            let lr = Rect::from_min_size(
                at + Vec2::new((HANDLE_R + 4.0) * k, -15.0 * k),
                Vec2::new(90.0 * k, 13.0 * k),
            );
            micro(b, lr, text, t.text, Align2::LEFT_CENTER, s);
        };
        readout(b, amount_pos, &format!("{:+.1} st", e.pitch_st));
        readout(
            b,
            knee_pos,
            &tf(
                "bass.808.time_readout",
                &[("ms", &format!("{:.0}", e.pitch_ms))],
            ),
        );
        let cap = tf(
            "bass.808.caption",
            &[
                ("note", &note),
                ("knock", &format!("{:.0}", e.knock * 100.0)),
            ],
        );
        let lr = Rect::from_min_size(
            d.min + Vec2::new(10.0 * k, 4.0 * k),
            Vec2::new(plot.width(), 13.0 * k),
        );
        micro(b, lr, &cap, t.text_dim, Align2::LEFT_CENTER, s);

        // The harmonics, relative to the fundamental.
        let bars = Rect::from_min_max(
            Pos2::new(d.right() - bars_w, d.top() + 22.0 * k),
            Pos2::new(d.right() - 10.0 * k, d.bottom() - 18.0 * k),
        );
        let levels = e.harmonic_levels();
        let w = bars.width() / HARMONICS as f32;
        let floor = -48.0f32;
        let y_of =
            |db: f32| bars.bottom() - ((db - floor) / -floor).clamp(0.0, 1.0) * bars.height();
        b.hline(
            bars.left(),
            bars.right(),
            b.snap_mid(y_of(0.0)),
            b.px(),
            t.grid_strong,
        );
        for (n, level) in levels.iter().enumerate().skip(1) {
            let a = if n == 1 { 1.0 } else { *level };
            let x0 = bars.left() + (n - 1) as f32 * w + 1.5 * k;
            if a > 0.0 {
                let db = 20.0 * a.log10();
                let r = Rect::from_min_max(
                    Pos2::new(x0, y_of(db)),
                    Pos2::new(x0 + w - 3.0 * k, bars.bottom()),
                );
                b.rect(
                    b.snap_rect(r),
                    Fill::Solid(if n == 1 { t.text_dim } else { s.hue }),
                );
            }
            let lr = Rect::from_center_size(
                Pos2::new(x0 + (w - 3.0 * k) * 0.5, bars.bottom() + 9.0 * k),
                Vec2::new(w + 4.0 * k, 12.0 * k),
            );
            micro(b, lr, &format!("{n}"), t.text_dim, Align2::CENTER_CENTER, s);
        }
        let lr = Rect::from_min_size(
            Pos2::new(bars.left(), d.top() + 4.0 * k),
            Vec2::new(bars.width(), 13.0 * k),
        );
        micro(
            b,
            lr,
            &t_of("bass.808.harmonics_caption"),
            t.text_dim,
            Align2::LEFT_CENTER,
            s,
        );
    });

    // Gestures.
    let near = 16.0 * k;
    if resp.drag_started() {
        // Where the press began: by the time egui calls it a drag, the pointer has moved.
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(pt) = origin.or(resp.interact_pointer_pos()) {
            let d_a = pt.distance(amount_pos);
            let d_k = pt.distance(knee_pos);
            app.inst.bass.grab = if d_a < near && d_a <= d_k {
                Some(Grab::PitchAmount)
            } else if d_k < near {
                Some(Grab::PitchTime)
            } else {
                None
            };
        }
    }
    if resp.dragged() {
        if let Some(pt) = resp.interact_pointer_pos() {
            match app.inst.bass.grab {
                Some(Grab::PitchAmount) => {
                    let v = geo.st(pt.y).clamp(0.0, 48.0);
                    app.set_inst_value(track, "808.pitch", (v * 2.0).round() / 2.0, true);
                }
                Some(Grab::PitchTime) => {
                    let v = (3.0 * geo.ms(pt.x)).clamp(1.0, 500.0);
                    app.set_inst_value(track, "808.pitch_time", v.round(), true);
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() {
        app.inst.bass.grab = None;
        app.end_gesture();
    }
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("bass.808.pitch"),
            &t("bass.808.drag_tip"),
            crate::kit::hint::Caps::PARAM,
        );
    }
    if resp.double_clicked() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let key = if pt.distance(amount_pos) < near {
                Some("808.pitch")
            } else if pt.distance(knee_pos) < near {
                Some("808.pitch_time")
            } else {
                None
            };
            if let Some(info) = key.and_then(|key| spec.kind.param(key).map(|i| (key, i))) {
                app.set_inst_value(track, info.0, info.1.default, false);
            }
        }
    }
}

fn t_of(key: &str) -> String {
    t(key)
}
