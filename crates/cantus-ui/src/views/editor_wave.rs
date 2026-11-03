//! The audio editor's graphs (ADR 0096): the time ruler with the markers, the waveform (a
//! lane per channel, the samples themselves when zoomed in far enough to count them), the
//! spectrogram (log frequency, drawn at the pixel size it is shown at), the selection,
//! the picked time-frequency rectangle and the audition's playhead. Drag selects to the
//! frame; the wheel zooms about the pointer; shift and the wheel (or the bar) scrolls.

use super::{Band, Session, Show, View};
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::waveform::{fill_peaks, Which};
use cantus_core::audio_edit::Sel;
use cantus_dsp::denoise::stft::{Spectrogram, BINS, N};
use egui::{pos2, Align2, Color32, Id, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2};

/// The spectrogram's frequency axis: log from here to the Nyquist.
pub const LOW_HZ: f32 = 30.0;
const RULER: f32 = 26.0;
const BAR: f32 = 12.0;

/// Where the graphs are and what the view maps.
#[derive(Copy, Clone)]
pub struct Geo {
    pub rect: Rect,
    pub start: f64,
    pub per: f64,
}

impl Geo {
    pub fn x(&self, frame: f64) -> f32 {
        self.rect.left() + ((frame - self.start) / self.per) as f32
    }

    pub fn frame(&self, x: f32) -> f64 {
        self.start + (x - self.rect.left()) as f64 * self.per
    }
}

/// The frames per point that fit `frames` in `width`.
pub fn fit_per(frames: usize, width: f32) -> f64 {
    (frames as f64 / width.max(1.0) as f64).max(1.0 / 32.0)
}

fn clamp_view(v: &mut View, frames: usize, width: f32) {
    let max_per = fit_per(frames, width).max(1.0 / 32.0);
    v.per = v.per.clamp(1.0 / 32.0, max_per);
    let span = v.per * width as f64;
    v.start = v.start.clamp(0.0, (frames as f64 - span).max(0.0));
}

/// "1.234 s", "1:02.5", for a ruler or a readout.
pub fn time_text(frame: f64, rate: u32) -> String {
    let s = frame / rate.max(1) as f64;
    if s >= 60.0 {
        format!("{}:{:04.1}", (s / 60.0) as u64, s % 60.0)
    } else if s.abs() < 1.0 {
        format!("{:.0} ms", s * 1000.0)
    } else {
        format!("{s:.2} s")
    }
}

/// A frame as the readout says it: time and the frame's number.
pub fn exact_text(frame: usize, rate: u32) -> String {
    format!("{:.5} s ({frame})", frame as f64 / rate.max(1) as f64)
}

/// The log-frequency position (0 at the bottom, 1 at the top) of `hz`.
pub fn hz_to_unit(hz: f32, nyq: f32) -> f32 {
    (hz.max(LOW_HZ) / LOW_HZ).ln() / (nyq / LOW_HZ).ln()
}

pub fn unit_to_hz(u: f32, nyq: f32) -> f32 {
    LOW_HZ * (nyq / LOW_HZ).powf(u.clamp(0.0, 1.0))
}

/// What the pointer did in the graphs.
#[derive(Default)]
pub struct Out {
    /// The selection or the view changed (the picture of the spectrogram is stale).
    pub touched: bool,
}

/// The whole graph area: ruler, lanes, bar. Mutates the session's selection, band, view.
pub fn show(ui: &mut Ui, area: Rect, panel: Rect, s: &mut Session, mode: Show) -> Out {
    let st = Style::current();
    let k = st.k;
    let (Some(buf), Some(peaks)) = (s.buf.clone(), s.peaks.clone()) else {
        return Out::default();
    };
    let frames = buf.frames();
    let ruler = Rect::from_min_size(area.min, Vec2::new(area.width(), RULER * k));
    let bar = Rect::from_min_max(pos2(area.left(), area.bottom() - BAR * k), area.max);
    let body = Rect::from_min_max(
        pos2(area.left(), ruler.bottom()),
        pos2(area.right(), bar.top()),
    );
    let spec_h = match mode {
        Show::Wave => 0.0,
        Show::Spectral => body.height(),
        Show::Both => (body.height() * 0.45).round(),
    };
    let wave = Rect::from_min_max(body.min, pos2(body.right(), body.bottom() - spec_h));
    let spec = Rect::from_min_max(pos2(body.left(), wave.bottom()), body.max);
    let width = area.width();
    if s.fit || s.view.per <= 0.0 {
        if s.view.per <= 0.0 && !s.fit {
            // Fit the selection (asked by `FitSelection`).
            if let Some(sel) = s.sel.filter(|x| !x.is_empty()) {
                s.view.per = (sel.len() as f64 * 1.1 / width.max(1.0) as f64).max(1.0 / 32.0);
            } else {
                s.view.per = fit_per(frames, width);
            }
        } else {
            s.view = View {
                start: 0.0,
                per: fit_per(frames, width),
            };
        }
        s.fit = false;
    }
    clamp_view(&mut s.view, frames, width);
    let mut out = Out::default();
    let id = Id::new("cantus-editor-graph");
    // The wheel: zoom about the pointer; shift or sideways: scroll.
    let hov = ui.rect_contains_pointer(area);
    if hov {
        let (dy, dx, shift, ptr) = ui.input(|i| {
            (
                i.smooth_scroll_delta.y,
                i.smooth_scroll_delta.x,
                i.modifiers.shift,
                i.pointer.hover_pos(),
            )
        });
        let g = Geo {
            rect: area,
            start: s.view.start,
            per: s.view.per,
        };
        if dx != 0.0 || (shift && dy != 0.0) {
            s.view.start -= (if dx != 0.0 { dx } else { dy }) as f64 * s.view.per * 2.0;
            out.touched = true;
        } else if dy != 0.0 {
            let at = ptr.map_or(area.center().x, |p| p.x);
            let anchor = g.frame(at);
            s.view.per *= (0.9f64).powf(dy as f64 / 20.0);
            clamp_view(&mut s.view, frames, width);
            s.view.start = anchor - (at - area.left()) as f64 * s.view.per;
            out.touched = true;
        }
        clamp_view(&mut s.view, frames, width);
    }
    let g = Geo {
        rect: area,
        start: s.view.start,
        per: s.view.per,
    };
    with_brush(ui.painter(), |b| {
        b.rect(body, Fill::Solid(st.t.row_black));
    });
    s.detail_want = if spec.height() > 8.0 {
        detail_want(s, &g, ui.ctx().pixels_per_point())
    } else {
        None
    };
    // The waveform.
    if wave.height() > 8.0 {
        paint_wave(ui, wave, &g, s, &buf, &peaks, &st);
    }
    if spec.height() > 8.0 {
        paint_spec(ui, spec, &g, s, &st);
    }
    paint_ruler(ui, ruler, &g, s, buf.rate, &st);
    paint_marks(ui, body, &g, s, &st);
    if spec.height() > 8.0 {
        paint_band(ui, spec, &g, s, buf.rate as f32 / 2.0);
    }
    // Pointer work: select in the waveform, or a rectangle in the spectrogram.
    for (rect, spectral) in [(wave, false), (spec, true)] {
        if rect.height() <= 8.0 {
            continue;
        }
        let resp = ui.interact(rect, id.with(spectral), Sense::click_and_drag());
        let nyq = buf.rate as f32 / 2.0;
        let frame_at = |x: f32| (g.frame(x).round().max(0.0) as usize).min(frames);
        let hz_at = |y: f32| unit_to_hz((rect.bottom() - y) / rect.height(), nyq);
        let out_key = id.with("out");
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                let a = frame_at(p.x);
                // Alt and a drag from inside the selection: the selection, dragged out of the
                // window, becomes a clip of its own.
                let inside = s
                    .sel
                    .is_some_and(|x| !x.is_empty() && a >= x.start && a < x.end);
                if inside && ui.input(|i| i.modifiers.alt) {
                    ui.data_mut(|d| d.insert_temp(out_key, true));
                    continue;
                }
                // Grab an edge of the selection that is there; shift extends from its
                // far edge; else begin anew.
                let near = |f: usize| (g.x(f as f64) - p.x).abs() < 6.0 * k;
                let anchor = match s.sel.filter(|x| !x.is_empty()) {
                    Some(x) if near(x.start) => x.end,
                    Some(x) if near(x.end) => x.start,
                    Some(x) if ui.input(|i| i.modifiers.shift) => {
                        if a < x.start {
                            x.end
                        } else {
                            x.start
                        }
                    }
                    _ => a,
                };
                ui.data_mut(|d| d.insert_temp(id.with("anchor"), (anchor, hz_at(p.y))));
            }
        }
        let dragging_out = ui.data(|d| d.get_temp::<bool>(out_key)).unwrap_or(false);
        if dragging_out {
            if resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            if resp.drag_stopped() {
                ui.data_mut(|d| d.remove::<bool>(out_key));
                let off = resp
                    .interact_pointer_pos()
                    .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                    .is_some_and(|p| !panel.contains(p));
                s.dragged_out = off;
            }
            continue;
        }
        if resp.dragged() {
            if let (Some(p), Some((anchor, hz0))) = (
                resp.interact_pointer_pos(),
                ui.data(|d| d.get_temp::<(usize, f32)>(id.with("anchor"))),
            ) {
                s.sel = Some(Sel::new(anchor, frame_at(p.x)));
                if spectral {
                    let hz1 = hz_at(p.y);
                    s.band = Some(Band {
                        lo_hz: hz0.min(hz1),
                        hi_hz: hz0.max(hz1).max(hz0.min(hz1) * 1.05),
                    });
                } else {
                    s.band = None;
                }
                out.touched = true;
                // The edge of the graph pulls the view along.
                if p.x > area.right() - 12.0 * k {
                    s.view.start += 12.0 * s.view.per;
                } else if p.x < area.left() + 12.0 * k {
                    s.view.start -= 12.0 * s.view.per;
                }
                clamp_view(&mut s.view, frames, width);
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let f = frame_at(p.x);
                s.sel = Some(Sel::new(f, f));
                s.band = None;
                out.touched = true;
            }
        }
        if resp.double_clicked() {
            s.sel = Some(Sel::whole(frames));
            s.band = None;
        }
    }
    // Clicking a marker's flag on the ruler selects up to the next marker; a region's bar
    // (or the loop's) selects it.
    let rr = ui.interact(ruler, id.with("ruler"), Sense::click());
    if rr.clicked() {
        if let Some(p) = rr.interact_pointer_pos() {
            let marks: Vec<usize> = s.marks.markers.iter().map(|m| m.frame as usize).collect();
            let lower = p.y > ruler.bottom() - 7.0 * k;
            let f = g.frame(p.x).max(0.0) as u64;
            let region = s
                .marks
                .regions
                .iter()
                .map(|r| (r.start, r.end))
                .chain(s.marks.loop_points)
                .find(|(a, b)| f >= *a && f < *b);
            if let Some(i) = marks
                .iter()
                .position(|m| (g.x(*m as f64) - p.x).abs() < 7.0 * k)
            {
                let end = marks.get(i + 1).copied().unwrap_or(frames);
                s.sel = Some(Sel::new(marks[i], end));
                s.band = None;
            } else if let (true, Some((a, b))) = (lower, region) {
                s.sel = Some(Sel::new(a as usize, (b as usize).min(frames)));
                s.band = None;
            } else {
                let f = (g.frame(p.x).round().max(0.0) as usize).min(frames);
                s.sel = Some(Sel::new(f, f));
            }
        }
    }
    // The bar.
    let mut start = (s.view.start / frames as f64) as f32;
    let len = ((s.view.per * width as f64) / frames as f64).min(1.0) as f32;
    if crate::kit::widgets::scrollbar(ui, bar, id.with("bar"), &mut start, len).dragged() {
        s.view.start = start as f64 * frames as f64;
        clamp_view(&mut s.view, frames, width);
    }
    out
}

fn paint_wave(
    ui: &mut Ui,
    wave: Rect,
    g: &Geo,
    s: &Session,
    buf: &cantus_core::audio_edit::AudioBuf,
    peaks: &crate::waveform::SourcePeaks,
    st: &Style,
) {
    let ch = buf.channels.max(1) as usize;
    let lanes = ch.min(2);
    let ppp = ui.ctx().pixels_per_point();
    let ink = mix(st.t.brand, st.t.text, 0.25);
    let dim = with_alpha(mix(st.t.brand, st.t.row_black, 0.3), 0.6);
    with_brush(ui.painter(), |b| {
        let lane_h = wave.height() / lanes as f32;
        for c in 0..lanes {
            let r = Rect::from_min_size(
                pos2(wave.left(), wave.top() + c as f32 * lane_h),
                Vec2::new(wave.width(), lane_h),
            );
            b.hline(
                r.left(),
                r.right(),
                r.center().y,
                b.px(),
                with_alpha(st.t.text_dim, 0.35),
            );
            if c > 0 {
                b.hline(
                    r.left(),
                    r.right(),
                    r.top(),
                    b.px(),
                    with_alpha(st.t.text_dim, 0.5),
                );
            }
            let inner = r.shrink2(Vec2::new(0.0, 3.0));
            if g.per < 2.0 {
                // Count the samples.
                let (f0, f1) = (
                    g.start.floor().max(0.0) as usize,
                    (g.frame(wave.right()).ceil() as usize + 1).min(buf.frames()),
                );
                let pts: Vec<Pos2> = (f0..f1)
                    .map(|f| {
                        let v = buf.data[f * ch + c.min(ch - 1)].clamp(-1.0, 1.0);
                        pos2(
                            g.x(f as f64 + 0.5),
                            inner.center().y - v * inner.height() * 0.5,
                        )
                    })
                    .collect();
                b.polyline(&pts, 1.2, ink);
                if g.per < 0.2 {
                    for p in &pts {
                        b.dot(*p, 2.2, ink);
                    }
                }
            } else {
                let n = (wave.width() * ppp).ceil().max(1.0) as usize;
                let per = g.per / ppp as f64;
                let mut pairs = Vec::with_capacity(n);
                peaks.columns(Which::Channel(c), g.start, per, n, &mut pairs);
                let col = wave.width() / n as f32;
                let band =
                    Rect::from_min_size(inner.min, Vec2::new(col * n as f32, inner.height()));
                let sel = s.sel.filter(|x| !x.is_empty());
                let inside = |i: usize| {
                    sel.is_some_and(|x| {
                        let f = g.start + (i as f64 + 0.5) * per;
                        f >= x.start as f64 && f < x.end as f64
                    })
                };
                // The selected part in the stronger ink, the rest as it is.
                fill_peaks(
                    b,
                    band,
                    &pairs,
                    |_| 1.0,
                    if sel.is_some() { dim } else { ink },
                );
                if sel.is_some() {
                    let sub: Vec<(f32, f32)> = pairs
                        .iter()
                        .enumerate()
                        .map(|(i, p)| if inside(i) { *p } else { (0.0, 0.0) })
                        .collect();
                    fill_peaks(b, band, &sub, |_| 1.0, ink);
                }
            }
        }
    });
}

/// The spectrogram's colours: ground to the theme's accent to its text.
fn ramp(st: &Style) -> [Color32; 256] {
    let mut lut = [Color32::BLACK; 256];
    let (a, m, h) = (st.t.row_black, st.t.brand, st.t.text);
    for (i, c) in lut.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        *c = if t < 0.55 {
            mix(a, m, t / 0.55)
        } else {
            mix(m, h, (t - 0.55) / 0.45)
        };
    }
    lut
}

#[derive(Clone)]
struct SpecTex {
    key: (u32, u32, u64, u64, usize, u64, [u8; 4]),
    tex: TextureHandle,
}

fn paint_spec(ui: &mut Ui, spec_rect: Rect, g: &Geo, s: &Session, st: &Style) {
    let (Some(whole), Some(buf)) = (s.spec.as_ref(), s.buf.as_ref()) else {
        return;
    };
    // The visible span in detail when the thread has drawn it, else the whole take's.
    let view_end = g.frame(g.rect.right());
    let (spec, offset) = match &s.detail {
        Some((off, d))
            if d.hop < whole.hop
                && (*off as f64) <= g.start.max(0.0)
                && (*off + d.frames * d.hop) as f64 >= view_end.min(buf.frames() as f64) =>
        {
            (d, *off)
        }
        _ => (whole, 0),
    };
    let ppp = ui.ctx().pixels_per_point();
    // No side past what the GPU holds (egui says it; 2048 on the CPU renderer): a wider view
    // is drawn from a picture stretched a little.
    let max = (ui.ctx().input(|i| i.max_texture_side) as u32).clamp(256, 4096);
    let w = ((spec_rect.width() * ppp).ceil() as u32).clamp(1, max);
    let h = ((spec_rect.height() * ppp).ceil() as u32).clamp(1, max);
    let nyq = buf.rate as f32 / 2.0;
    let key = (
        w,
        h,
        g.start.to_bits(),
        g.per.to_bits(),
        std::sync::Arc::as_ptr(spec) as usize,
        st.t.dark as u64,
        st.t.brand.to_array(),
    );
    let id = Id::new("cantus-editor-spec-tex");
    let cached: Option<SpecTex> = ui.data(|d| d.get_temp(id));
    let tex = match cached {
        Some(c) if c.key == key => c.tex,
        _ => {
            let img = render_spec(spec, offset, g, w, h, nyq, &ramp(st));
            let tex = ui
                .ctx()
                .load_texture("cantus-editor-spec", img, TextureOptions::LINEAR);
            ui.data_mut(|d| {
                d.insert_temp(
                    id,
                    SpecTex {
                        key,
                        tex: tex.clone(),
                    },
                )
            });
            tex
        }
    };
    with_brush(ui.painter(), |b| {
        b.foreign(tex.id(), spec_rect);
        // The frequency lines: 100 Hz, 1 kHz, 10 kHz.
        for hz in [100.0, 1_000.0, 10_000.0] {
            if hz < nyq {
                let y = spec_rect.bottom() - hz_to_unit(hz, nyq) * spec_rect.height();
                b.hline(
                    spec_rect.left(),
                    spec_rect.right(),
                    y,
                    b.px(),
                    with_alpha(st.t.text, 0.18),
                );
            }
        }
    });
    for hz in [100.0f32, 1_000.0, 10_000.0] {
        if hz < nyq {
            let y = spec_rect.bottom() - hz_to_unit(hz, nyq) * spec_rect.height();
            let label = if hz >= 1000.0 {
                format!("{} kHz", hz / 1000.0)
            } else {
                format!("{hz} Hz")
            };
            let r = Rect::from_min_size(
                pos2(spec_rect.left() + 4.0, y - 14.0 * st.k),
                Vec2::new(60.0 * st.k, 13.0 * st.k),
            );
            with_brush(ui.painter(), |b| {
                let _ = text::paint(
                    b,
                    r,
                    &Text::new(&label, Role::Label, with_alpha(st.t.text, 0.7))
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    st.tk(),
                );
            });
        }
    }
}

/// The spectrogram as a picture `w` × `h`: time across from the view's start, frequency
/// up on a log axis, the levels through `lut` from -80 dB to 0 dB.
fn render_spec(
    spec: &Spectrogram,
    offset: usize,
    g: &Geo,
    w: u32,
    h: u32,
    nyq: f32,
    lut: &[Color32; 256],
) -> egui::ColorImage {
    let bin_of_row: Vec<f32> = (0..h)
        .map(|y| {
            let u = 1.0 - (y as f32 + 0.5) / h as f32;
            unit_to_hz(u, nyq) * N as f32 / (2.0 * nyq)
        })
        .collect();
    let mut px = vec![Color32::BLACK; (w * h) as usize];
    for x in 0..w {
        let frame = g.start + (x as f64 + 0.5) / w as f64 * g.per * g.rect.width() as f64;
        let col = (((frame - offset as f64) / spec.hop as f64).round().max(0.0) as usize)
            .min(spec.frames - 1);
        let cells = &spec.cells[col * BINS..(col + 1) * BINS];
        for y in 0..h {
            let bf = bin_of_row[y as usize].clamp(0.0, (BINS - 1) as f32);
            let (i0, f) = (bf.floor() as usize, bf.fract());
            let i1 = (i0 + 1).min(BINS - 1);
            let v = cells[i0] as f32 * (1.0 - f) + cells[i1] as f32 * f;
            let db = -110.0 + v / 255.0 * 110.0;
            let t = ((db + 80.0) / 80.0).clamp(0.0, 1.0);
            px[(y * w + x) as usize] = lut[(t * 255.0) as usize];
        }
    }
    egui::ColorImage {
        size: [w as usize, h as usize],
        source_size: Vec2::new(w as f32, h as f32),
        pixels: px,
    }
}

fn paint_ruler(ui: &mut Ui, ruler: Rect, g: &Geo, s: &Session, rate: u32, st: &Style) {
    let k = st.k;
    with_brush(ui.painter(), |b| {
        b.rect(ruler, Fill::Solid(st.t.panel));
        b.hline(
            ruler.left(),
            ruler.right(),
            ruler.bottom() - b.px() * 0.5,
            b.px(),
            with_alpha(st.t.text_dim, 0.4),
        );
    });
    // A step of 1, 2 or 5 × 10^n seconds, at least 80 points apart.
    let min_s = 80.0 * g.per / rate as f64;
    let mut step = 1e-4;
    'find: loop {
        for m in [1.0, 2.0, 5.0] {
            if step * m >= min_s {
                step *= m;
                break 'find;
            }
        }
        step *= 10.0;
    }
    let first = (g.start / rate as f64 / step).floor() as i64;
    let last = (g.frame(ruler.right()) / rate as f64 / step).ceil() as i64;
    with_brush(ui.painter(), |b| {
        for i in first..=last {
            let f = i as f64 * step * rate as f64;
            let x = g.x(f);
            if x < ruler.left() || x > ruler.right() {
                continue;
            }
            b.vline(
                x,
                ruler.bottom() - 7.0 * k,
                ruler.bottom(),
                b.px(),
                with_alpha(st.t.text_dim, 0.7),
            );
            let r = Rect::from_min_size(
                pos2(x + 3.0, ruler.top()),
                Vec2::new(78.0 * k, ruler.height() - 6.0 * k),
            );
            let label = ruler_text(f / rate as f64, step);
            let _ = text::paint(
                b,
                r,
                &Text::new(&label, Role::Label, st.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                st.tk(),
            );
        }
        // Markers: a flag at the ruler's foot.
        // Regions and the loop: bars along the ruler's foot.
        let bar_y = ruler.bottom() - 3.0 * k;
        for (ra, rb, c) in s
            .marks
            .regions
            .iter()
            .map(|r| (r.start, r.end, st.t.selection))
            .chain(s.marks.loop_points.map(|(a, e)| (a, e, st.t.led)))
        {
            let (x0, x1) = (
                g.x(ra as f64).max(ruler.left()),
                g.x(rb as f64).min(ruler.right()),
            );
            if x1 > x0 {
                b.rect(
                    Rect::from_min_max(pos2(x0, bar_y - 2.0 * k), pos2(x1, bar_y + 2.0 * k)),
                    Fill::Solid(with_alpha(c, 0.8)),
                );
            }
        }
        for m in s.marks.markers.iter().map(|m| m.frame) {
            let x = g.x(m as f64);
            if x >= ruler.left() && x <= ruler.right() {
                b.polygon(
                    &[
                        pos2(x, ruler.bottom() - 11.0 * k),
                        pos2(x + 8.0 * k, ruler.bottom() - 7.0 * k),
                        pos2(x, ruler.bottom() - 3.0 * k),
                    ],
                    st.t.highlight,
                );
                b.vline(
                    x,
                    ruler.bottom() - 11.0 * k,
                    ruler.bottom(),
                    b.px() * 1.5,
                    st.t.highlight,
                );
            }
        }
    });
}

/// The selection's veil, the picked rectangle, the markers' lines and the playhead.
fn paint_marks(ui: &mut Ui, body: Rect, g: &Geo, s: &Session, st: &Style) {
    let k = st.k;
    let mid_split = s.spec.is_some();
    let _ = mid_split;
    with_brush(ui.painter(), |b| {
        if let Some(sel) = s.sel {
            let (x0, x1) = (g.x(sel.start as f64), g.x(sel.end as f64));
            if sel.is_empty() {
                if x0 >= body.left() && x0 <= body.right() {
                    b.vline(x0, body.top(), body.bottom(), b.px() * 1.5, st.t.selection);
                }
            } else {
                let r = Rect::from_min_max(
                    pos2(x0.max(body.left()), body.top()),
                    pos2(x1.min(body.right()), body.bottom()),
                );
                if r.width() > 0.0 {
                    b.rect(r, Fill::Solid(with_alpha(st.t.selection, 0.16)));
                    b.vline(x0, body.top(), body.bottom(), b.px(), st.t.selection);
                    b.vline(x1, body.top(), body.bottom(), b.px(), st.t.selection);
                }
            }
        }
        for m in s.marks.markers.iter().map(|m| m.frame) {
            let x = g.x(m as f64);
            if x >= body.left() && x <= body.right() {
                b.dashed(
                    &[pos2(x, body.top()), pos2(x, body.bottom())],
                    b.px(),
                    with_alpha(st.t.highlight, 0.8),
                    4.0 * k,
                    3.0 * k,
                );
            }
        }
        if let Some(p) = s.play {
            let rate = s.buf.as_ref().map_or(48_000, |b| b.rate) as f64;
            let f = p.from as f64 + p.at.elapsed().as_secs_f64() * rate;
            let x = g.x(f.min(p.to as f64));
            b.vline(x, body.top(), body.bottom(), b.px() * 1.5, st.t.playhead);
        }
    });
}

/// The rectangle picked on the spectrogram, outlined over it.
pub fn paint_band(ui: &mut Ui, spec_rect: Rect, g: &Geo, s: &Session, nyq: f32) {
    let st = Style::current();
    let (Some(sel), Some(band)) = (s.sel.filter(|x| !x.is_empty()), s.band) else {
        return;
    };
    let y = |hz: f32| spec_rect.bottom() - hz_to_unit(hz, nyq) * spec_rect.height();
    let r = Rect::from_min_max(
        pos2(g.x(sel.start as f64), y(band.hi_hz)),
        pos2(g.x(sel.end as f64), y(band.lo_hz)),
    )
    .intersect(spec_rect);
    if r.width() > 0.0 && r.height() > 0.0 {
        with_brush(ui.painter(), |b| {
            b.rect(r, Fill::Solid(with_alpha(st.t.highlight, 0.18)));
            b.rrect_stroke(r, 0.0, b.px() * 1.5, st.t.highlight);
        });
    }
}

/// A ruler's label at `s` seconds with as many decimals as its `step` needs ("2.0035 s",
/// "350 ms", "1:02.5"), so no two ticks read the same.
pub fn ruler_text(s: f64, step: f64) -> String {
    let places = (-(step.log10().floor()) as i32).clamp(0, 6) as usize;
    if s >= 60.0 {
        let p = places.max(1);
        format!("{}:{:0w$.p$}", (s / 60.0) as u64, s % 60.0, w = p + 3)
    } else if s.abs() < 1.0 {
        format!("{:.*} ms", places.saturating_sub(3), s * 1000.0)
    } else {
        format!("{s:.places$} s")
    }
}

/// The span the spectrogram should be drawn at in detail (start, end, hop), when the whole
/// take's picture is coarser than the view's pixels: the view and one view's width either
/// side, on a grid of view widths (a small scroll asks nothing new), at a power-of-two hop
/// of at least `MIN_HOP`.
fn detail_want(s: &Session, g: &Geo, ppp: f32) -> Option<(usize, usize, usize)> {
    use cantus_dsp::denoise::stft::MIN_HOP;
    let spec = s.spec.as_ref()?;
    let frames = s.buf.as_ref()?.frames();
    let per_px = (g.per / ppp.max(0.1) as f64).max(1.0) as usize;
    let hop = per_px.next_power_of_two().clamp(MIN_HOP, 512);
    if spec.hop <= hop * 2 {
        return None;
    }
    let span = ((g.per * g.rect.width() as f64) as usize).max(1);
    let start = ((g.start.max(0.0) as usize) / span).saturating_sub(1) * span;
    let end = (start + 3 * span).min(frames);
    Some((start, end, hop))
}

#[cfg(test)]
mod tests {
    use super::ruler_text;

    #[test]
    fn ruler_labels_have_the_decimals_their_step_needs() {
        assert_eq!(ruler_text(2.0, 0.5), "2.0 s");
        assert_eq!(ruler_text(2.0035, 0.0005), "2.0035 s");
        assert_eq!(ruler_text(0.35, 0.05), "350 ms");
        assert_eq!(ruler_text(0.0125, 0.0005), "12.5 ms");
        assert_eq!(ruler_text(62.5, 0.5), "1:02.5");
        assert_ne!(ruler_text(2.0005, 0.0005), ruler_text(2.0010, 0.0005));
    }
}
