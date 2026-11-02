//! The light visualiser in a video's frames (round 35, ADR 0091 §4): the companion's look —
//! the master's spectrum as rounded bars on a glass plate, the bar and its beats in a chip,
//! the song's name and tempo in another — drawn by tiny-skia at the frame's own pixel size
//! from the **export's own render**: its samples are analysed once, at [`RATE`] instants a
//! second, before the first frame is drawn, and each frame reads them at its time.
//!
//! **Deterministic.** A frame is a pure function of its time: a band shows the highest of the
//! analyses within the meter's fall before that time, less the fall since — the companion's
//! ballistics (rise at once, fall [`FALL_PER_S`] of the scale a second) without any state
//! carried from frame to frame, so the same frame drawn twice, or out of order, or by
//! another thread, gives the same pixels.

use super::vis::{band_hz, scale, BANDS, FALL_PER_S, FFT, RATE};
use cantus_core::tempo::TempoMap;
use cantus_core::{Ticks, PPQ};
use cantus_dsp::math;
use cantus_dsp::spectrum::SpectrumAnalyser;
use tiny_skia::{Color, FillRule, Paint, Path, PathBuilder, Pixmap, Rect, Transform};

/// What the frames' visualiser needs from the interface when the export starts: colours
/// from the theme, the song's name, the font, what else is in the frame.
#[derive(Clone, Debug)]
pub struct Setup {
    pub accent: [u8; 4],
    pub bright: [u8; 4],
    pub plate: [u8; 4],
    pub edge: [u8; 4],
    pub text: [u8; 4],
    pub dim: [u8; 4],
    pub shadow: [u8; 4],
    /// The stage's ground, its floor line, the menu's face (the companion's own drawing).
    pub ink: [u8; 4],
    pub floor: [u8; 4],
    pub menu: [u8; 4],
    pub meter_well: [u8; 4],
    pub meter_low: [u8; 4],
    pub meter_high: [u8; 4],
    pub meter_clip: [u8; 4],
    /// The song's name (the chip shows it when the video's own title line is off).
    pub song: String,
    /// The video's title line is drawn (at the bottom): the song's chip is left out.
    pub title_on: bool,
    /// Opaque plates, no glow: a chroma key cuts no half-transparent pixel cleanly.
    pub solid: bool,
}

impl Setup {
    /// The theme's colours now, for a song called `song`.
    pub fn from_theme(song: &str, title_on: bool, solid: bool) -> Setup {
        let t = crate::kit::Style::current().t;
        let c = |c: egui::Color32| [c.r(), c.g(), c.b(), 255];
        Setup {
            accent: c(t.highlight),
            bright: c(t.brand_bright),
            plate: c(t.panel),
            edge: c(t.edge),
            text: c(t.text),
            dim: c(t.text_dim),
            shadow: c(t.shadow),
            ink: c(t.ink),
            floor: c(t.grid_strong),
            menu: c(t.menu),
            meter_well: c(t.meter_well),
            meter_low: c(t.meter_low),
            meter_high: c(t.meter_high),
            meter_clip: c(t.meter_clip),
            song: song.to_string(),
            title_on,
            solid,
        }
    }
}

/// The analysed song and how to draw it.
#[derive(Clone, Debug)]
pub struct VideoVis {
    setup: Setup,
    /// Raw bands (0…1) at each analysis instant `k / RATE` seconds of video time.
    raw: Vec<[f32; BANDS]>,
    tempo: TempoMap,
    /// The song's seconds at video time 0.
    start_seconds: f64,
}

/// Analyse interleaved `samples` (`channels`, `rate`; video time 0 at the first sample).
pub fn analyse(samples: &[f32], channels: usize, rate: u32) -> Vec<[f32; BANDS]> {
    analyse_from(samples, channels, rate, 0)
}

/// [`analyse`] for audio that begins `offset` sample frames after video time 0 (the
/// dialog's preview renders a few seconds before the moment shown, not the whole range):
/// its instants are the export's own — `k / RATE` seconds of video time — so where the
/// sound is the same the bands are too. Instants whose window ends before the audio
/// begins are silent (and not computed).
pub fn analyse_from(samples: &[f32], channels: usize, rate: u32, offset: i64) -> Vec<[f32; BANDS]> {
    let ch = channels.max(1);
    let frames = samples.len() / ch;
    let n = (((frames as i64 + offset.max(0)) as f64 / f64::from(rate)) * RATE).ceil() as usize + 1;
    let mut a = SpectrumAnalyser::new(FFT);
    let mut pairs = vec![0.0f32; FFT * 2];
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let end = ((k as f64 / RATE) * f64::from(rate)).round() as i64 - offset;
        if end <= 0 {
            out.push([0.0; BANDS]);
            continue;
        }
        for (i, p) in pairs.chunks_exact_mut(2).enumerate() {
            let f = end - FFT as i64 + i as i64;
            let (l, r) = if f >= 0 && (f as usize) < frames {
                let at = f as usize * ch;
                let l = samples[at];
                (l, if ch > 1 { samples[at + 1] } else { l })
            } else {
                (0.0, 0.0)
            };
            p[0] = l;
            p[1] = r;
        }
        a.push(&pairs);
        a.compute();
        let mut bands = [0.0f32; BANDS];
        for (i, b) in bands.iter_mut().enumerate() {
            let hz = band_hz(i);
            let l = a.level_db(0, rate as f32, hz, 1.0 / 3.0);
            let r = a.level_db(1, rate as f32, hz, 1.0 / 3.0);
            let p = 0.5 * (math::powf(10.0, l / 10.0) + math::powf(10.0, r / 10.0));
            *b = if p > 1e-12 {
                scale(10.0 * math::log10(p))
            } else {
                0.0
            };
        }
        out.push(bands);
    }
    out
}

impl VideoVis {
    pub fn new(
        setup: Setup,
        raw: Vec<[f32; BANDS]>,
        tempo: TempoMap,
        start_seconds: f64,
    ) -> VideoVis {
        VideoVis {
            setup,
            raw,
            tempo,
            start_seconds,
        }
    }

    /// The bands shown at video time `t`: the highest analysis within the fall's reach,
    /// less what it has fallen since.
    pub fn bands_at(&self, t: f64) -> [f32; BANDS] {
        let mut out = [0.0f32; BANDS];
        if self.raw.is_empty() || t < 0.0 {
            return out;
        }
        let last = ((t * RATE).floor() as usize).min(self.raw.len() - 1);
        let reach = (RATE / f64::from(FALL_PER_S)).ceil() as usize + 1;
        for k in last.saturating_sub(reach)..=last {
            let since = (t - k as f64 / RATE).max(0.0) as f32;
            for (o, r) in out.iter_mut().zip(&self.raw[k]) {
                *o = o.max(r - FALL_PER_S * since);
            }
        }
        out
    }

    /// Draw the visualiser for video time `t` onto `pm`.
    pub fn draw(&self, t: f64, pm: &mut Pixmap) {
        let (w, h) = (pm.width() as f32, pm.height() as f32);
        let k = w.min(h) / 400.0;
        let m = 12.0 * k;
        let s = &self.setup;
        // In a video the dancer stands near the bottom and the title line runs along it:
        // the chips go along the top and the band right under them, clear of both.
        let chip_h = 40.0 * k;
        let band_h = (h * 0.11).clamp(36.0 * k, 90.0 * k);
        // Never so wide its bars become blobs: at most six times as wide as it is tall,
        // centred (a wide frame).
        let band_w = (w - 2.0 * m).min(band_h * 6.0);
        let band = rect((w - band_w) * 0.5, m + chip_h + 8.0 * k, band_w, band_h);
        let painter = Painter { setup: &self.setup };
        painter.spectrum(pm, band, k, &self.bands_at(t));
        // The song's place: bar, beat and how far through it.
        let song_t = self.start_seconds + t;
        let tick = self.tempo.tick_at_seconds(song_t).max(0.0);
        let at = Ticks(tick.floor() as i64);
        let (bar, _, _) = self.tempo.bbt(at);
        let start = self.tempo.bar_start(at);
        let per_bar = ((self.tempo.bar_end(at).0 - start.0) as f64 / PPQ as f64).max(1.0);
        let beats = tick / PPQ as f64;
        let in_bar = beats - start.0 as f64 / PPQ as f64;
        let n = (per_bar.round() as u32).clamp(1, 16);
        let beat = (in_bar.floor().max(0.0) as u32).min(n - 1);
        let phase = beats.rem_euclid(1.0) as f32;
        let bar_w = 112.0 * k;
        let bar_rect = rect(w - m - bar_w, m, bar_w, chip_h);
        painter.bar_chip(pm, bar_rect, k, bar, beat, n, phase);
        if !s.title_on {
            let bpm = self.tempo.bpm_at(tick);
            let song_w = (bar_rect.left() - 8.0 * k - m).min(220.0 * k);
            if song_w > 60.0 * k {
                painter.song_chip(pm, rect(m, m, song_w, chip_h), k, bpm);
            }
        }
    }
}

/// Draws the visualiser's parts with a setup's colours (the companion on Windows and
/// the video frames share it).
pub struct Painter<'a> {
    pub setup: &'a Setup,
}

impl Painter<'_> {
    pub fn plate(&self, pm: &mut Pixmap, r: Rect, k: f32) {
        let s = &self.setup;
        let radius = 10.0 * k;
        if !s.solid {
            if let Some(sh) = rect_opt(r.left(), r.top() + 2.0 * k, r.width(), r.height()) {
                fill(
                    pm,
                    &rrect(sh.grown(2.0 * k), radius + 2.0 * k),
                    with(s.shadow, 0.35),
                );
            }
        }
        let a = if s.solid { 1.0 } else { 0.62 };
        fill(pm, &rrect(r, radius), with(s.plate, a));
        stroke(
            pm,
            &rrect(r, radius),
            mix(s.edge, s.accent, 0.35),
            k.max(1.0),
        );
    }

    pub fn spectrum(&self, pm: &mut Pixmap, r: Rect, k: f32, bands: &[f32; BANDS]) {
        self.plate(pm, r, k);
        let s = &self.setup;
        let inner = rect(
            r.left() + 10.0 * k,
            r.top() + 8.0 * k,
            r.width() - 20.0 * k,
            r.height() - 16.0 * k,
        );
        let gap = (2.5 * k).max(1.0);
        let bw = ((inner.width() - gap * (BANDS - 1) as f32) / BANDS as f32).max(1.0);
        for (i, v) in bands.iter().enumerate() {
            let x = inner.left() + i as f32 * (bw + gap);
            let hgt = (v * inner.height()).max(bw.min(3.0 * k));
            let bar = rect(x, inner.bottom() - hgt, bw, hgt);
            let top = mix(s.accent, s.bright, *v);
            if !s.solid && *v > 0.05 {
                fill(
                    pm,
                    &rrect(bar.grown(1.5 * k), bw * 0.5),
                    with(top, 0.22 * v),
                );
            }
            fill(pm, &rrect(bar, bw * 0.5), with(top, 1.0));
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bar_chip(
        &self,
        pm: &mut Pixmap,
        r: Rect,
        k: f32,
        bar: i64,
        beat: u32,
        n: u32,
        phase: f32,
    ) {
        self.bar_chip_still(pm, r, k, bar);
        self.bar_dots(pm, r, k, beat, n, phase);
    }

    /// The bar chip's plate and label: what changes once a bar (cached by the companion).
    pub fn bar_chip_still(&self, pm: &mut Pixmap, r: Rect, k: f32, bar: i64) {
        self.plate(pm, r, k);
        let s = &self.setup;
        let label = crate::i18n::tf("stage.vis.bar", &[("bar", &bar)]);
        let face = crate::design::score_faces();
        let px = 13.0 * k;
        cantus_video::title::draw(
            pm,
            face.bold,
            &label,
            r.left() + 10.0 * k,
            r.top() + 17.0 * k,
            px,
            r.width() - 20.0 * k,
            s.text,
            None,
        );
    }

    /// The bar chip's beats: one dot a beat, the landing one lit and pulsing.
    pub fn bar_dots(&self, pm: &mut Pixmap, r: Rect, k: f32, beat: u32, n: u32, phase: f32) {
        let s = &self.setup;
        let n = n.max(1);
        let step = ((r.width() - 20.0 * k) / n as f32).min(14.0 * k);
        let rad = (step * 0.3).min(4.0 * k);
        let cy = r.top() + 29.0 * k;
        for i in 0..n {
            let cx = r.left() + 10.0 * k + step * (i as f32 + 0.5);
            if i == beat {
                let pulse = (1.0 - phase).powi(2);
                let accent = if i == 0 { s.bright } else { s.accent };
                if !s.solid {
                    fill(
                        pm,
                        &circle(cx, cy, rad * (1.6 + 1.2 * pulse)),
                        with(accent, 0.25 * pulse),
                    );
                }
                fill(
                    pm,
                    &circle(cx, cy, rad * (1.0 + 0.25 * pulse)),
                    with(accent, 1.0),
                );
            } else {
                fill(pm, &circle(cx, cy, rad * 0.8), with(s.dim, 0.55));
            }
        }
    }

    pub fn song_chip(&self, pm: &mut Pixmap, r: Rect, k: f32, bpm: f64) {
        self.plate(pm, r, k);
        let s = &self.setup;
        let face = crate::design::score_faces();
        let name = if s.song.is_empty() {
            crate::i18n::t("stage.vis.untitled")
        } else {
            s.song.clone()
        };
        let bpm = if (bpm - bpm.round()).abs() < 0.05 {
            format!("{bpm:.0}")
        } else {
            format!("{bpm:.1}")
        };
        let tempo = crate::i18n::tf("stage.vis.bpm", &[("bpm", &bpm)]);
        let x = r.left() + 12.0 * k;
        let wmax = r.width() - 24.0 * k;
        cantus_video::title::draw(
            pm,
            face.regular,
            &name,
            x,
            r.top() + 18.0 * k,
            14.0 * k,
            wmax,
            s.text,
            None,
        );
        cantus_video::title::draw(
            pm,
            face.regular,
            &tempo,
            x,
            r.top() + 33.0 * k,
            11.5 * k,
            wmax,
            s.dim,
            None,
        );
    }
}

pub(crate) fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    rect_opt(x, y, w, h)
        .unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("a unit rect"))
}

pub(crate) fn rect_opt(x: f32, y: f32, w: f32, h: f32) -> Option<Rect> {
    Rect::from_xywh(x, y, w.max(1.0), h.max(1.0))
}

pub(crate) trait Grown {
    fn grown(&self, d: f32) -> Rect;
}

impl Grown for Rect {
    fn grown(&self, d: f32) -> Rect {
        rect(
            self.left() - d,
            self.top() - d,
            self.width() + 2.0 * d,
            self.height() + 2.0 * d,
        )
    }
}

/// A rounded rectangle (quarter circles as cubics).
pub(crate) fn rrect(r: Rect, radius: f32) -> Path {
    let rad = radius.min(r.width() * 0.5).min(r.height() * 0.5).max(0.0);
    let c = rad * 0.552_284_8;
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    let mut pb = PathBuilder::new();
    pb.move_to(l + rad, t);
    pb.line_to(rt - rad, t);
    pb.cubic_to(rt - rad + c, t, rt, t + rad - c, rt, t + rad);
    pb.line_to(rt, b - rad);
    pb.cubic_to(rt, b - rad + c, rt - rad + c, b, rt - rad, b);
    pb.line_to(l + rad, b);
    pb.cubic_to(l + rad - c, b, l, b - rad + c, l, b - rad);
    pb.line_to(l, t + rad);
    pb.cubic_to(l, t + rad - c, l + rad - c, t, l + rad, t);
    pb.close();
    pb.finish().unwrap_or_else(|| PathBuilder::from_rect(r))
}

pub(crate) fn circle(cx: f32, cy: f32, r: f32) -> Path {
    PathBuilder::from_circle(cx, cy, r.max(0.5))
        .unwrap_or_else(|| PathBuilder::from_rect(rect(cx, cy, 1.0, 1.0)))
}

pub(crate) fn with(c: [u8; 4], a: f32) -> Color {
    Color::from_rgba8(
        c[0],
        c[1],
        c[2],
        (f32::from(c[3]) * a.clamp(0.0, 1.0)).round() as u8,
    )
}

pub(crate) fn mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

pub(crate) fn fill(pm: &mut Pixmap, path: &Path, color: Color) {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    pm.fill_path(path, &p, FillRule::Winding, Transform::identity(), None);
}

pub(crate) fn stroke(pm: &mut Pixmap, path: &Path, c: [u8; 4], w: f32) {
    let mut p = Paint::default();
    p.set_color(with(c, 0.7));
    p.anti_alias = true;
    let st = tiny_skia::Stroke {
        width: w,
        ..Default::default()
    };
    pm.stroke_path(path, &p, &st, Transform::identity(), None);
}

/// The frames' overlay: the visualiser (if any) under the video's title line (if any).
pub fn with_title(
    vis: Option<VideoVis>,
    title: Option<Box<cantus_video::dance::Overlay>>,
) -> Option<Box<cantus_video::dance::Overlay>> {
    match (vis, title) {
        (None, title) => title,
        (Some(v), None) => Some(Box::new(move |_n: u32, t: f64, pm: &mut Pixmap| {
            v.draw_song(t, pm)
        })),
        (Some(v), Some(title)) => Some(Box::new(move |n: u32, t: f64, pm: &mut Pixmap| {
            v.draw_song(t, pm);
            title(n, t, pm);
        })),
    }
}

impl VideoVis {
    /// Draw for the overlay's argument: the **song's** seconds (the overlay is called with
    /// the frame's song time).
    pub fn draw_song(&self, song_seconds: f64, pm: &mut Pixmap) {
        self.draw(song_seconds - self.start_seconds, pm);
    }
}

/// The visualiser's setup for a video export with `d`'s choices (`None`: off).
pub fn setup_for(
    p: &cantus_core::Project,
    d: &crate::video_export::VideoSettings,
    title_on: bool,
) -> Option<Setup> {
    d.visualiser
        .then(|| Setup::from_theme(&p.name, title_on, d.background.chroma().is_some()))
}

/// The visualiser of an export, its render analysed (`None`: off).
pub fn for_render(
    setup: Option<Setup>,
    audio: &cantus_export::Audio,
    tempo: &TempoMap,
    from: Ticks,
) -> Option<VideoVis> {
    let setup = setup?;
    let raw = analyse(&audio.samples, audio.channels as usize, audio.rate);
    Some(VideoVis::new(
        setup,
        raw,
        tempo.clone(),
        tempo.seconds_at(from.0 as f64),
    ))
}

/// The dialog's preview: the visualiser as the file's frame draws it — from the analysis
/// of the moment's own render (`video_export::preview_bands`, the export's instants), its
/// bars at rest until that render is in — under the title line.
pub fn preview(
    app: &crate::app::CantusApp,
    d: &crate::video_export::VideoSettings,
    title: Option<Box<cantus_video::dance::Overlay>>,
) -> Option<Box<cantus_video::dance::Overlay>> {
    with_title(preview_vis(app, d), title)
}

/// The preview's visualiser (`None`: off): the analysis of the moment's own render when it
/// is in, else none yet.
pub fn preview_vis(
    app: &crate::app::CantusApp,
    d: &crate::video_export::VideoSettings,
) -> Option<VideoVis> {
    let p = app.project();
    let setup = setup_for(p, d, d.title)?;
    let (from, _) = crate::video_export::range(app, d.range).unwrap_or_default();
    let tempo = p.tempo_map();
    let start = tempo.seconds_at(from.0 as f64);
    let raw = crate::video_export::preview_bands(app)
        .map(|b| b.to_vec())
        .unwrap_or_default();
    Some(VideoVis::new(setup, raw, tempo, start))
}
