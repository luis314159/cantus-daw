//! Exemplar's waveform: the selected zone's file with its start, end and loop handles, the
//! loop and its crossfade shaded, the voices playing it as playheads; and beside it the
//! **seam**: the audio before the loop end joined to the audio after the loop start, the
//! file's own continuation drawn faintly behind (Kontakt's seam view and TX16Wx's overlay
//! in one), with the seam error measured in dB. "Find best loop" searches nearby points
//! for the lowest error; "Snap" moves both ends to zero crossings on every channel.
//!
//! Both are displays in cards (ADR 0024 §8), drawn with the kit's brush in tokens; the
//! loop's settings sit in the card under the waveform as segmented controls, LEDs and
//! label | value rows.

use super::map::{set_fields, sounding, zone_name, LOOP_DIRS, LOOP_MODES};
use super::parts;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band::CardLook;
use crate::kit::color::with_alpha;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, State, Style, Tok};
use cantus_core::inst::sampler::{self as model, zf, Zone};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::exemplar::{analysis, builtin};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::fmt::Write as _;
use std::sync::Arc;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Which {
    Start,
    End,
    LoopStart,
    LoopEnd,
}

/// A handle being dragged.
#[derive(Copy, Clone, Debug)]
pub struct Marker {
    which: Which,
    zone: usize,
}

/// A zone's audio: interleaved frames, channels, rate.
#[derive(Clone)]
pub struct Audio {
    pub data: Arc<[f32]>,
    pub channels: usize,
    pub rate: f32,
}

impl Audio {
    pub fn frames(&self) -> usize {
        self.data.len() / self.channels.max(1)
    }
}

/// The selected zone's file for its waveform, as it is read beside the window.
pub enum WaveFile {
    Reading(std::sync::mpsc::Receiver<Option<Audio>>),
    Ready(Audio),
    Failed,
}

impl std::fmt::Debug for WaveFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WaveFile::Reading(_) => "Reading",
            WaveFile::Ready(_) => "Ready",
            WaveFile::Failed => "Failed",
        })
    }
}

fn audio(d: &cantus_core::SampleData) -> Audio {
    Audio {
        data: d.frames.clone(),
        channels: d.channels.max(1) as usize,
        rate: d.rate as f32,
    }
}

/// A file's whole audio for the waveform: what the engine holds whole, else read on a
/// thread of its own — from the stream's cache file when there is one, decoded otherwise —
/// and never kept beyond the zone shown (review 19).
fn file_audio(app: &mut CantusApp, p: &str) -> Option<Audio> {
    let path = std::path::Path::new(p);
    if let Some(d) = app.engine.samples.whole(path) {
        return Some(audio(&d));
    }
    let state = &mut app.inst.sampler.wave_file;
    if state.as_ref().is_none_or(|(q, _)| q != p) {
        let (tx, rx) = std::sync::mpsc::channel();
        let owned = path.to_path_buf();
        let _ = std::thread::Builder::new()
            .name("cantus-zone-wave".into())
            .spawn(move || {
                let d = cantus_engine::inst::stream::read_whole(&owned).or_else(|| {
                    cantus_engine::SampleCache::new()
                        .load(&owned)
                        .ok()
                        .map(|a| (*a).clone())
                });
                let _ = tx.send(d.as_ref().map(audio));
            });
        *state = Some((p.to_string(), WaveFile::Reading(rx)));
    }
    let (_, w) = state.as_mut()?;
    if let WaveFile::Reading(rx) = w {
        match rx.try_recv() {
            Ok(Some(a)) => *w = WaveFile::Ready(a),
            Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => *w = WaveFile::Failed,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    match w {
        WaveFile::Ready(a) => Some(a.clone()),
        _ => None,
    }
}

/// Whether the selected zone's file is still being read for its waveform.
pub fn wave_reading(app: &CantusApp, z: &Zone) -> bool {
    match (&z.sample, &app.inst.sampler.wave_file) {
        (Some(p), Some((q, WaveFile::Reading(_)))) => p == q,
        _ => false,
    }
}

/// The audio a zone plays: its file or its built-in.
pub fn audio_of(app: &mut CantusApp, z: &Zone) -> Option<Audio> {
    match &z.sample {
        Some(p) => file_audio(app, p),
        None => builtin::get(z.get(zf::GEN) as u8).and_then(|b| {
            Some(Audio {
                data: b.level0()?,
                channels: 1,
                rate: b.rate as f32,
            })
        }),
    }
}

fn marker_x(rect: Rect, view: (f64, f64), frame: f64) -> f32 {
    rect.left() + ((frame - view.0) / (view.1 - view.0).max(1.0)) as f32 * rect.width()
}

fn frame_at(rect: Rect, view: (f64, f64), x: f32) -> f64 {
    view.0 + ((x - rect.left()) / rect.width()) as f64 * (view.1 - view.0)
}

/// The selected zone, its number and audio, when there is one that can be read.
pub fn selected(app: &mut CantusApp, spec: &NativeSpec) -> Option<(usize, Zone, Option<Audio>)> {
    let n = app.inst.sampler.selected?;
    let z = model::Zone::of(spec, n)?;
    let audio = audio_of(app, &z);
    Some((n, z, audio))
}

/// Where the loop is, in frames: (looping, start, end, loop start, loop end).
fn points(z: &Zone, frames: f64) -> (bool, f64, f64, f64, f64) {
    let end = if z.get(zf::END) > 0.0 {
        z.get(zf::END) as f64
    } else {
        frames
    };
    let le = if z.get(zf::LE) > 0.0 {
        z.get(zf::LE) as f64
    } else {
        end - 1.0
    };
    (
        z.get(zf::MODE) >= 2.0,
        z.get(zf::START) as f64,
        end,
        z.get(zf::LS) as f64,
        le,
    )
}

/// What the waveform shows, in seconds: `0.000 – 3.000 s`.
fn span_text(app: &CantusApp, audio: &Audio) -> String {
    let frames = audio.frames() as f64;
    let (a, b) = app.inst.sampler.view.unwrap_or((0.0, frames.max(2.0)));
    let r = audio.rate as f64;
    format!("{:.3} \u{2013} {:.3} s", a / r, b / r)
}

/// The span the waveform shows, at the right of its card's title bar.
fn span_in_title(app: &CantusApp, ui: &egui::Ui, title: Rect, s: &Style, audio: &Audio) {
    let r = Rect::from_min_max(
        pos2(title.center().x, title.top()),
        pos2(title.right() - 8.0 * s.k, title.bottom()),
    );
    let span = span_text(app, audio);
    parts::words(
        ui,
        r,
        s,
        Src::Str(&span),
        Role::Micro,
        s.t.text_dim,
        Align2::RIGHT_CENTER,
        Policy::Elide(Elide::End),
    );
}

/// The card title for a zone's sample: its name.
pub fn title(z: Option<&Zone>) -> String {
    z.map(zone_name).unwrap_or_default()
}

/// The waveform display in `rect`: painted, and its handles dragged (Alt snaps to a zero
/// crossing), Ctrl + wheel zooms, double-click fits.
#[allow(clippy::too_many_arguments)]
pub fn waveform(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    n: usize,
    z: &Zone,
    audio: &Audio,
    rect: Rect,
    s: &Style,
) {
    let frames = audio.frames() as f64;
    let (looping, start, end, ls, le) = points(z, frames);
    let view = app.inst.sampler.view.unwrap_or((0.0, frames.max(2.0)));
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let mut handles = vec![(Which::Start, start, "S"), (Which::End, end, "E")];
    if looping {
        handles.push((Which::LoopStart, ls, "L"));
        handles.push((Which::LoopEnd, le + 1.0, "R"));
    }
    let heads: Vec<f64> = sounding(app)
        .into_iter()
        .filter(|(zi, _)| *zi == n)
        .map(|(_, p)| p)
        .collect();
    let xf = (z.get(zf::XF) as f64 * audio.rate as f64 / 1000.0).min(ls);
    with_brush(&ui.painter_at(rect), |b| {
        paint_wave(
            b,
            rect,
            s,
            audio,
            view,
            looping,
            (start, end, ls, le),
            xf,
            &handles,
            &heads,
        );
    });
    handle_drag(app, ui, &resp, rect, view, &handles, n, track, spec, audio);
    if resp.hovered() {
        let (dy, ctrl) = ui
            .ctx()
            .input(|i| (i.smooth_scroll_delta.y, i.modifiers.ctrl));
        if ctrl && dy != 0.0 {
            if let Some(p) = resp.hover_pos() {
                let at = frame_at(rect, view, p.x);
                let k = if dy > 0.0 { 0.8 } else { 1.25 };
                let span = ((view.1 - view.0) * k).clamp(64.0, frames.max(64.0));
                let f = (at - view.0) / (view.1 - view.0);
                let lo = (at - f * span).clamp(0.0, (frames - span).max(0.0));
                app.inst.sampler.view = Some((lo, lo + span));
            }
        }
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("smp.wave_tip"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    if resp.double_clicked() {
        app.inst.sampler.view = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_wave(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    audio: &Audio,
    view: (f64, f64),
    looping: bool,
    (start, end, ls, le): (f64, f64, f64, f64),
    xf: f64,
    handles: &[(Which, f64, &str)],
    heads: &[f64],
) {
    let t = &s.t;
    let k = s.k;
    b.register(
        rect,
        "smp.wave",
        &[
            Tok::Ink,
            Tok::Grid,
            Tok::Text,
            Tok::TextDim,
            Tok::Highlight,
            Tok::Focus,
        ],
        &[s.hue_tok],
    );
    let frames = audio.frames() as f64;
    let wave = rect.shrink2(vec2(0.0, 14.0 * k));
    let mid = wave.center().y;
    let amp = wave.height() * 0.48;
    // The loop and its crossfade.
    if looping {
        let (x0, x1) = (marker_x(rect, view, ls), marker_x(rect, view, le + 1.0));
        b.rect(
            Rect::from_min_max(
                pos2(x0.max(rect.left()), rect.top()),
                pos2(x1.min(rect.right()), rect.bottom()),
            ),
            Fill::Solid(with_alpha(s.hue, if t.dark { 0.12 } else { 0.09 })),
        );
        if xf > 0.0 {
            let xa = marker_x(rect, view, le + 1.0 - xf);
            b.polygon(
                &[
                    pos2(xa, rect.bottom()),
                    pos2(x1, rect.top()),
                    pos2(x1, rect.bottom()),
                ],
                with_alpha(s.hue, 0.22),
            );
        }
    }
    b.hline(rect.left(), rect.right(), mid, b.px(), t.grid);
    // The waveform: min and max per physical column.
    let cols = (rect.width() * b.ppp()).max(1.0) as usize;
    let peaks = analysis::peaks(
        &audio.data,
        audio.channels,
        view.0.max(0.0) as usize,
        view.1.min(frames) as usize,
        cols,
    );
    let cw = rect.width() / cols.max(1) as f32;
    let body = with_alpha(s.hue, if t.dark { 0.9 } else { 0.8 });
    for (i, (lo, hi)) in peaks.iter().enumerate() {
        let x = rect.left() + i as f32 * cw;
        let (a, z) = (
            mid - hi.clamp(-1.0, 1.0) * amp,
            mid - lo.clamp(-1.0, 1.0) * amp,
        );
        b.rect(
            Rect::from_min_max(pos2(x, a), pos2(x + cw, z.max(a + b.px()))),
            Fill::Solid(body),
        );
    }
    // What does not play is dimmed.
    let dim = with_alpha(t.ink, 0.62);
    let sx = marker_x(rect, view, start);
    let ex = marker_x(rect, view, end);
    if sx > rect.left() {
        b.rect(
            Rect::from_min_max(rect.left_top(), pos2(sx.min(rect.right()), rect.bottom())),
            Fill::Solid(dim),
        );
    }
    if ex < rect.right() {
        b.rect(
            Rect::from_min_max(pos2(ex.max(rect.left()), rect.top()), rect.right_bottom()),
            Fill::Solid(dim),
        );
    }
    // The handles: a line and a lettered flag.
    for (which, frame, tag) in handles {
        let x = marker_x(rect, view, *frame);
        if x < rect.left() - 1.0 || x > rect.right() + 1.0 {
            continue;
        }
        let c = if matches!(which, Which::LoopStart | Which::LoopEnd) {
            t.highlight
        } else {
            t.text
        };
        b.vline(x, rect.top(), rect.bottom(), 1.5 * k, c);
        // Start and end flag the top edge, the loop's points the bottom one; a flag sits
        // inside the part it bounds.
        let fx = if matches!(which, Which::End | Which::LoopEnd) {
            x - 13.0 * k
        } else {
            x
        };
        let fy = if matches!(which, Which::LoopStart | Which::LoopEnd) {
            rect.bottom() - 13.0 * k
        } else {
            rect.top()
        };
        let flag = Rect::from_min_size(pos2(fx, fy), vec2(13.0 * k, 13.0 * k));
        b.rrect(b.snap_rect(flag), 2.0 * k, Fill::Solid(c));
        let _ = ktext::paint(b, flag, &Text::new(tag, Role::Micro, t.ink), s.tk());
    }
    for pos in heads {
        let x = marker_x(rect, view, *pos);
        if x >= rect.left() && x <= rect.right() {
            b.vline(x, rect.top(), rect.bottom(), b.px(), t.focus);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_drag(
    app: &mut CantusApp,
    ui: &egui::Ui,
    resp: &egui::Response,
    rect: Rect,
    view: (f64, f64),
    handles: &[(Which, f64, &str)],
    n: usize,
    track: TrackId,
    spec: &NativeSpec,
    audio: &Audio,
) {
    let reach = 7.0 * (rect.height() / 120.0).clamp(0.75, 2.0);
    let near = |p: Pos2| {
        handles
            .iter()
            .map(|(w, f, _)| (*w, (marker_x(rect, view, *f) - p.x).abs()))
            .filter(|(_, d)| *d < reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(w, _)| w)
    };
    if let Some(p) = resp.hover_pos() {
        if near(p).is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
    if resp.drag_started() {
        if let Some(which) = resp.interact_pointer_pos().and_then(near) {
            app.inst.sampler.marker = Some(Marker { which, zone: n });
        }
    }
    if let (Some(m), Some(p)) = (app.inst.sampler.marker, resp.interact_pointer_pos()) {
        if resp.dragged() && m.zone == n {
            let mut f = frame_at(rect, view, p.x).round().max(0.0) as i64;
            if ui.ctx().input(|i| i.modifiers.alt) {
                f = analysis::zero_crossing(&audio.data, audio.channels, f, 512);
            }
            let f = f as f32;
            let field = match m.which {
                Which::Start => (zf::START, f),
                Which::End => (zf::END, f),
                Which::LoopStart => (zf::LS, f),
                Which::LoopEnd => (zf::LE, (f - 1.0).max(0.0)),
            };
            set_fields(app, track, spec, n, &[field], true);
        }
    }
    if resp.drag_stopped() && app.inst.sampler.marker.is_some() {
        app.inst.sampler.marker = None;
        app.end_gesture();
    }
}

/// Frames either side of the seam the view shows.
const SEAM_FRAMES: i64 = 48;

/// Channel `c` of frame `i`, silence outside the file.
fn at(a: &Audio, i: i64, c: usize) -> f32 {
    if i < 0 || i as usize >= a.frames() {
        return 0.0;
    }
    a.data[i as usize * a.channels + c.min(a.channels - 1)]
}

/// Channel `c` of frame `b` as the loop plays it: inside the crossfade (`xf` frames before
/// the loop end) the audio one loop earlier fades in, as in the engine.
fn played(a: &Audio, ls: i64, le: i64, xf: i64, b: i64, c: usize) -> f32 {
    let len = le - ls + 1;
    let xf = xf.clamp(0, ls.min(len - 1).max(0));
    if xf > 0 && b > le - xf && b <= le {
        let g = (b - (le - xf)) as f32 / xf as f32;
        (1.0 - g) * at(a, b, c) + g * at(a, b - len, c)
    } else {
        at(a, b, c)
    }
}

/// The loop's seam error in dB, as the engine would play it.
pub fn seam_error(z: &Zone, audio: &Audio) -> f32 {
    let frames = audio.frames() as i64;
    let ls = z.get(zf::LS) as i64;
    let le = if z.get(zf::LE) > 0.0 {
        z.get(zf::LE) as i64
    } else {
        frames - 1
    };
    let xf = (z.get(zf::XF) * audio.rate / 1000.0).round() as i64;
    analysis::seam_error_xf_db(
        &audio.data,
        audio.channels,
        ls,
        le,
        analysis::SEAM_WINDOW,
        xf,
    )
}

/// Paint the seam in `rect`: before the loop end | after the loop start, one lane per
/// channel, the file's own continuation faint behind, the error in a pill.
fn paint_seam(b: &mut Brush, rect: Rect, s: &Style, z: &Zone, audio: &Audio) {
    let t = &s.t;
    let k = s.k;
    b.register(
        rect,
        "smp.seam",
        &[
            Tok::Ink,
            Tok::Grid,
            Tok::TextDim,
            Tok::Highlight,
            Tok::Led,
            Tok::Danger,
        ],
        &[s.hue_tok],
    );
    let frames = audio.frames() as i64;
    let ls = z.get(zf::LS) as i64;
    let le = if z.get(zf::LE) > 0.0 {
        z.get(zf::LE) as i64
    } else {
        frames - 1
    };
    let xf = (z.get(zf::XF) * audio.rate / 1000.0).round() as i64;
    let plot = rect.shrink2(vec2(0.0, 16.0 * k));
    let mid = plot.center();
    let sx = plot.width() / (2 * SEAM_FRAMES) as f32;
    let ghost = with_alpha(t.text_dim, 0.45);
    let lanes = audio.channels.clamp(1, 2);
    let lane_h = plot.height() / lanes as f32;
    let mut pts: Vec<Pos2> = Vec::with_capacity(2 * SEAM_FRAMES as usize);
    for c in 0..lanes {
        let centre = plot.top() + lane_h * (c as f32 + 0.5);
        let amp = lane_h * 0.40;
        let peak = (-SEAM_FRAMES..SEAM_FRAMES)
            .map(|j| {
                played(audio, ls, le, xf, le - SEAM_FRAMES + 1 + j, c)
                    .abs()
                    .max(at(audio, ls + j, c).abs())
            })
            .fold(1e-6f32, f32::max);
        let y = |v: f32| centre - v / peak * amp;
        b.hline(rect.left(), rect.right(), centre, b.px(), t.grid);
        pts.clear();
        pts.extend(
            (0..SEAM_FRAMES).map(|j| pos2(mid.x + j as f32 * sx, y(at(audio, le + 1 + j, c)))),
        );
        b.polyline(&pts, 1.0 * k, ghost);
        pts.clear();
        pts.extend(
            (0..SEAM_FRAMES)
                .map(|j| pos2(mid.x - (j + 1) as f32 * sx, y(at(audio, ls - 1 - j, c)))),
        );
        b.polyline(&pts, 1.0 * k, ghost);
        pts.clear();
        pts.extend((-SEAM_FRAMES..SEAM_FRAMES).map(|j| {
            let v = if j < 0 {
                played(audio, ls, le, xf, le + 1 + j, c)
            } else {
                at(audio, ls + j, c)
            };
            pos2(mid.x + j as f32 * sx, y(v))
        }));
        b.polyline(&pts, 1.75 * k, s.hue);
    }
    b.vline(mid.x, rect.top(), rect.bottom(), b.px(), t.highlight);
    let cap_h = 12.0 * k;
    let half = rect.width() * 0.5 - 8.0 * k;
    let _ = ktext::paint(
        b,
        Rect::from_min_size(rect.min + vec2(4.0 * k, 2.0 * k), vec2(half, cap_h)),
        &Text::key("smp.seam_before", Role::Micro, t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Abbreviate),
        s.tk(),
    );
    let _ = ktext::paint(
        b,
        Rect::from_min_size(
            pos2(rect.right() - 4.0 * k - half, rect.top() + 2.0 * k),
            vec2(half, cap_h),
        ),
        &Text::key("smp.seam_after", Role::Micro, t.text_dim)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Abbreviate),
        s.tk(),
    );
    let err = seam_error(z, audio);
    let color = if err < -40.0 {
        t.led
    } else if err < -20.0 {
        t.highlight
    } else {
        t.danger
    };
    let label = tf("smp.seam_error", &[("db", &format!("{err:.1}"))]);
    let pill = Rect::from_center_size(
        pos2(rect.center().x, rect.bottom() - 9.0 * k),
        vec2(112.0 * k, 15.0 * k),
    );
    b.rrect(
        b.snap_rect(pill),
        3.0 * k,
        Fill::Solid(with_alpha(t.ink, 0.85)),
    );
    b.rrect_stroke(b.snap_rect(pill), 3.0 * k, b.px(), color);
    let _ = ktext::paint(
        b,
        pill.shrink2(vec2(4.0 * k, 0.0)),
        &Text::new(&label, Role::Label, color)
            .caps(false)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
}

/// The label | value rows of the sample card: frames you drag, seconds in the help line.
#[allow(clippy::too_many_arguments)]
fn frame_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    n: usize,
    z: &Zone,
    rect: Rect,
    s: &Style,
    key: &str,
    f: usize,
    max: f32,
    rate: f32,
) {
    let lw = (rect.width() * 0.46).min(96.0 * s.k);
    let lr = Rect::from_min_max(rect.min, pos2(rect.left() + lw, rect.bottom()));
    let vr = Rect::from_min_max(pos2(lr.right() + 4.0 * s.k, rect.top()), rect.max);
    parts::row_label(ui, lr, s, key);
    let mut v = z.get(f);
    let r = parts::drag_num(ui, vr, s, &mut v, 0.0..=max, 0.0, false, &t(key), |v, b| {
        let _ = write!(b, "{v:.0}");
    });
    if r.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t(key),
            &format!("{:.4} s", z.get(f) / rate),
            crate::kit::hint::Caps::PARAM,
        );
    }
    if r.changed() {
        set_fields(app, track, spec, n, &[(f, v.round())], true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

/// The sample card's height for a display `disp` points tall, at this style's scale.
pub fn sample_card_height(s: &Style, disp: f32) -> f32 {
    (parts::CARD_CHROME + 1.0 + disp + 8.0 + 3.0 * parts::ROW + 2.0 * 6.0) * s.k
}

/// The selected zone's sample: the waveform as the display, then the loop — mode,
/// direction, reverse, crossfade curve, and its points as label | value rows.
pub fn sample_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    disp: f32,
) {
    let sel = selected(app, spec);
    let name = title(sel.as_ref().map(|(_, z, _)| z));
    let look = CardLook {
        title: if name.is_empty() {
            Src::Key("smp.card.sample")
        } else {
            Src::Str(&name)
        },
        switchable: false,
        selector: None,
        menu: false,
        display: Some(disp),
    };
    let cr = parts::card(ui, rect, s, &look, true);
    let Some(d) = cr.display else {
        return;
    };
    let Some((n, z, audio)) = sel else {
        parts::words(
            ui,
            d,
            s,
            Src::Key("smp.select_zone"),
            Role::Body,
            s.t.text_dim,
            Align2::CENTER_CENTER,
            Policy::Elide(Elide::End),
        );
        return;
    };
    let Some(audio) = audio else {
        let reading = wave_reading(app, &z);
        if reading {
            crate::views::workspace_ticker::wake_after(
                ui.ctx(),
                std::time::Duration::from_millis(50),
            );
        }
        parts::words(
            ui,
            d,
            s,
            Src::Key(if reading {
                "smp.wave_reading"
            } else {
                "smp.unreadable"
            }),
            Role::Body,
            if reading { s.t.text_dim } else { s.t.danger },
            Align2::CENTER_CENTER,
            Policy::Elide(Elide::End),
        );
        return;
    };
    waveform(app, ui, track, spec, n, &z, &audio, d, s);
    span_in_title(app, ui, cr.title, s, &audio);
    let k = s.k;
    let body = Rect::from_min_max(pos2(cr.body.left(), d.bottom() + 8.0 * k), cr.body.max);
    let h = parts::ROW * k;
    let gap = 6.0 * k;
    let row = |i: usize, x0: f32, x1: f32| {
        Rect::from_min_size(
            pos2(x0, body.top() + i as f32 * (h + gap)),
            vec2(x1 - x0, h),
        )
    };
    let split = body.left() + body.width() * 0.46;
    let mode = (z.get(zf::MODE) as usize).min(3);
    let looping = mode >= 2;
    let modes: Vec<Src<'_>> = LOOP_MODES.iter().map(|k| Src::Key(k)).collect();
    if let Some(i) = parts::seg(
        ui,
        row(0, body.left(), split),
        s,
        mode,
        &modes,
        &t("smp.loop_mode"),
    ) {
        set_fields(app, track, spec, n, &[(zf::MODE, i as f32)], false);
    }
    let dirs: Vec<Src<'_>> = LOOP_DIRS.iter().map(|k| Src::Key(k)).collect();
    let dir = (z.get(zf::DIR) as usize).min(2);
    if let Some(i) = parts::seg(
        ui,
        row(1, body.left(), split),
        s,
        dir,
        &dirs,
        &t("smp.loop_dir"),
    ) {
        set_fields(app, track, spec, n, &[(zf::DIR, i as f32)], false);
    }
    let r2 = row(2, body.left(), split);
    let rev_w = r2.width() * 0.36;
    let rev = z.get(zf::REV) >= 0.5;
    let rr = Rect::from_min_size(r2.min, vec2(rev_w - 4.0 * k, h));
    if parts::led(ui, rr, s, rev, Src::Key("smp.reverse"), &t("smp.reverse")).clicked() {
        set_fields(
            app,
            track,
            spec,
            n,
            &[(zf::REV, if rev { 0.0 } else { 1.0 })],
            false,
        );
    }
    let curves = [Src::Key("smp.xf_linear"), Src::Key("smp.xf_power")];
    let cr2 = Rect::from_min_max(pos2(r2.left() + rev_w, r2.top()), r2.max);
    let power = (z.get(zf::LXC) >= 0.5) as usize;
    if let Some(i) = parts::seg(ui, cr2, s, power, &curves, &t("smp.loop_xf_curve")) {
        set_fields(app, track, spec, n, &[(zf::LXC, i as f32)], false);
    }
    let frames = audio.frames() as f32;
    let (x0, x1) = (split + 10.0 * k, body.right());
    let mid = x0 + (x1 - x0) * 0.5;
    frame_row(
        app,
        ui,
        track,
        spec,
        n,
        &z,
        row(0, x0, mid - 4.0 * k),
        s,
        "smp.start",
        zf::START,
        frames,
        audio.rate,
    );
    frame_row(
        app,
        ui,
        track,
        spec,
        n,
        &z,
        row(0, mid + 4.0 * k, x1),
        s,
        "smp.end",
        zf::END,
        frames,
        audio.rate,
    );
    if looping {
        frame_row(
            app,
            ui,
            track,
            spec,
            n,
            &z,
            row(1, x0, mid - 4.0 * k),
            s,
            "smp.loop_start",
            zf::LS,
            frames,
            audio.rate,
        );
        frame_row(
            app,
            ui,
            track,
            spec,
            n,
            &z,
            row(1, mid + 4.0 * k, x1),
            s,
            "smp.loop_end",
            zf::LE,
            frames,
            audio.rate,
        );
    }
    // The crossfade, a well you drag in milliseconds.
    let xr = row(2, x0, mid - 4.0 * k);
    let lw = (xr.width() * 0.46).min(96.0 * k);
    parts::row_label(
        ui,
        Rect::from_min_max(xr.min, pos2(xr.left() + lw, xr.bottom())),
        s,
        "smp.loop_xf",
    );
    let mut xf = z.get(zf::XF);
    let r = parts::drag_num(
        ui,
        Rect::from_min_max(pos2(xr.left() + lw + 4.0 * k, xr.top()), xr.max),
        s,
        &mut xf,
        0.0..=1_000.0,
        0.0,
        false,
        &t("smp.loop_xf"),
        |v, b| crate::views::inst::write_readout(v, "ms", b),
    );
    if r.changed() {
        set_fields(app, track, spec, n, &[(zf::XF, xf)], true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    // The loop's length, read out.
    if looping {
        let (_, _, _, ls, le) = points(&z, audio.frames() as f64);
        let lr = row(2, mid + 4.0 * k, x1);
        let lw = (lr.width() * 0.46).min(96.0 * k);
        parts::row_label(
            ui,
            Rect::from_min_max(lr.min, pos2(lr.left() + lw, lr.bottom())),
            s,
            "smp.loop_len",
        );
        let mut len = String::new();
        let _ = write!(len, "{:.3} s", (le - ls + 1.0) / audio.rate as f64);
        parts::readout(
            ui,
            Rect::from_min_max(pos2(lr.left() + lw + 4.0 * k, lr.top()), lr.max),
            s,
            &len,
            true,
            false,
            &t("smp.loop_len"),
        );
    }
}

/// The seam card: the join as its display and the error in dB; Find best loop and Snap
/// to zero as actions (or Loop the sustain, when the zone does not loop).
pub fn seam_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    disp: f32,
) {
    let look = CardLook {
        title: Src::Key("smp.card.seam"),
        switchable: false,
        selector: None,
        menu: false,
        display: Some(disp),
    };
    let cr = parts::card(ui, rect, s, &look, true);
    let Some(d) = cr.display else {
        return;
    };
    let Some((n, z, Some(audio))) = selected(app, spec) else {
        return;
    };
    let looping = z.get(zf::MODE) >= 2.0;
    let k = s.k;
    let body = Rect::from_min_max(pos2(cr.body.left(), d.bottom() + 8.0 * k), cr.body.max);
    let h = parts::ROW * k;
    let row = |i: usize| {
        Rect::from_min_size(
            pos2(body.left(), body.top() + i as f32 * (h + 6.0 * k)),
            vec2(body.width(), h),
        )
    };
    if !looping {
        parts::words(
            ui,
            d,
            s,
            Src::Key("smp.no_loop"),
            Role::Body,
            s.t.text_dim,
            Align2::CENTER_CENTER,
            Policy::Elide(Elide::End),
        );
        if parts::press(
            ui,
            row(0),
            s,
            Src::Key("smp.make_loop"),
            &t("smp.make_loop"),
        )
        .clicked()
        {
            let f = audio.frames() as f32;
            set_fields(
                app,
                track,
                spec,
                n,
                &[
                    (zf::MODE, 2.0),
                    (zf::LS, (f * 0.3).round()),
                    (zf::LE, (f * 0.9).round()),
                ],
                false,
            );
        }
        return;
    }
    with_brush(&ui.painter_at(d), |b| paint_seam(b, d, s, &z, &audio));
    let frames = audio.frames() as i64;
    let ls = z.get(zf::LS) as i64;
    let le = if z.get(zf::LE) > 0.0 {
        z.get(zf::LE) as i64
    } else {
        frames - 1
    };
    if parts::press(
        ui,
        row(0),
        s,
        Src::Key("smp.find_loop"),
        &t("smp.find_loop_tip"),
    )
    .clicked()
    {
        let (a, b, _) = analysis::find_best_loop(&audio.data, audio.channels, ls, le, 2_000);
        set_fields(
            app,
            track,
            spec,
            n,
            &[(zf::LS, a as f32), (zf::LE, b as f32)],
            false,
        );
    }
    if parts::press(ui, row(1), s, Src::Key("smp.snap"), &t("smp.snap_tip")).clicked() {
        let a = analysis::zero_crossing(&audio.data, audio.channels, ls, 512);
        let b = analysis::zero_crossing(&audio.data, audio.channels, le + 1, 512) - 1;
        if b > a {
            set_fields(
                app,
                track,
                spec,
                n,
                &[(zf::LS, a as f32), (zf::LE, b as f32)],
                false,
            );
        }
    }
    let _ = State::REST;
}

/// The Play view's sample card: the waveform alone, live with the voices playing it.
pub fn play_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let sel = selected(app, spec);
    let name = title(sel.as_ref().map(|(_, z, _)| z));
    let disp = rect.height() / s.k - crate::kit::band::TITLE - 1.0;
    let look = CardLook {
        title: if name.is_empty() {
            Src::Key("smp.card.sample")
        } else {
            Src::Str(&name)
        },
        switchable: false,
        selector: None,
        menu: false,
        display: Some(disp),
    };
    let cr = parts::card(ui, rect, s, &look, true);
    let Some(d) = cr.display else {
        return;
    };
    match sel {
        Some((n, z, Some(audio))) => {
            waveform(app, ui, track, spec, n, &z, &audio, d, s);
            span_in_title(app, ui, cr.title, s, &audio);
        }
        Some((_, z, None)) => {
            let reading = wave_reading(app, &z);
            if reading {
                crate::views::workspace_ticker::wake_after(
                    ui.ctx(),
                    std::time::Duration::from_millis(50),
                );
            }
            parts::words(
                ui,
                d,
                s,
                Src::Key(if reading {
                    "smp.wave_reading"
                } else {
                    "smp.unreadable"
                }),
                Role::Body,
                if reading { s.t.text_dim } else { s.t.danger },
                Align2::CENTER_CENTER,
                Policy::Elide(Elide::End),
            )
        }
        None => parts::words(
            ui,
            d,
            s,
            Src::Key("smp.select_zone"),
            Role::Body,
            s.t.text_dim,
            Align2::CENTER_CENTER,
            Policy::Elide(Elide::End),
        ),
    }
}
