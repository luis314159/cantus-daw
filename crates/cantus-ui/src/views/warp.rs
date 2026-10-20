//! **The warp editor** (round 29, ADR 0061): an audio clip opened in the piano roll's place
//! in its *Warp* mode — beside *Pitch*, the two tabs over the clip editor (ADR 0054's window;
//! ADR 0051 §4 stands: a double-click on an audio clip opens the clip editor, in the mode it
//! was last in). A beat ruler (the project's bars, through the tempo map), the clip's
//! waveform large, read through its warp, its transients along the top band and its warp
//! markers as handles — the arrangement's gestures (`views/arrangement_warp.rs`), bigger;
//! on the bar: the mode, the transposition, the formants, *Follow the project tempo*, the
//! source's tempo (½ and ×2), *Detect tempo*, *Quantise audio…* and *Reset warp*.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{self, well, State, Style};
use crate::views::arrangement::warp_api as aw;
use crate::views::arrangement::{AudioHandle as Handle, TimelineGeometry};
use crate::waveform::{fill_peaks, Lookup, Which};
use cantus_core::warp::WarpMode;
use cantus_core::{ClipId, Ticks, TrackId, PPQ};
use egui::{pos2, vec2, Align2, Rect, Sense};

/// The ruler's and the bar's heights at 100 %.
const RULER_H: f32 = 22.0;
const BAR_H: f32 = 30.0;

/// The editor's state.
#[derive(Debug, Default)]
pub struct WarpView {
    /// Whether the clip editor shows *Warp* (else *Pitch*).
    pub open: bool,
    /// A marker drag in the editor.
    drag: Option<(ClipRef, aw::MarkerDrag, f32, f32)>,
    /// A value drag on the bar: (which, value at the press, press x).
    value: Option<(u8, f64, f32)>,
    /// Where the waveform was drawn last frame (for the tests and the shots).
    pub wave: Option<Rect>,
    /// The markers' and transients' hit geometry last frame.
    pub hits: Option<aw::Hits>,
}

impl CantusApp {
    /// Open the clip editor on `r` in its *Warp* mode.
    pub fn open_warp_editor(&mut self, r: ClipRef) {
        self.warp.view.open = true;
        self.open_pitch_editor(r.0, r.1);
    }
}

/// The two modes' tabs over the clip editor: *Pitch* and *Warp*.
pub fn tabs(app: &mut CantusApp, ui: &mut egui::Ui) {
    let s = Style::current();
    ui.horizontal(|ui| {
        let w = s.s(84.0);
        if kit::widgets::tab(ui, !app.warp.view.open, Src::Key("warp.tab.pitch"), w).clicked() {
            app.warp.view.open = false;
        }
        if kit::widgets::tab(ui, app.warp.view.open, Src::Key("warp.tab.warp"), w).clicked() {
            app.warp.view.open = true;
        }
    });
}

/// The editor in `ui` for clip `clip` of `track`.
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId) {
    let s = Style::current();
    let r: ClipRef = (track, clip);
    tabs(app, ui);
    bar(app, ui, r);
    let full = ui.available_rect_before_wrap();
    if full.height() < 80.0 || full.width() < 160.0 {
        return;
    }
    let resp = ui.allocate_rect(full, Sense::click_and_drag());
    let Some(c) = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .cloned()
    else {
        return;
    };
    let Some(region) = c.audio.clone() else {
        return;
    };
    let ruler = Rect::from_min_max(full.min, pos2(full.right(), full.top() + s.s(RULER_H)));
    let wave = Rect::from_min_max(pos2(full.left(), ruler.bottom()), full.max)
        .shrink2(vec2(0.0, s.s(4.0)));
    // The clip's window fitted, a twentieth of it either side.
    let len_b = (c.length.0.max(1) as f64 / PPQ as f64) as f32;
    let px_per_beat = full.width() / (len_b * 1.1);
    let geo = TimelineGeometry {
        left: full.left(),
        width: full.width(),
        ruler_top: ruler.top(),
        loop_lane_bottom: ruler.top(),
        body_top: ruler.bottom(),
        px_per_beat,
        scroll_beats: c.start.as_beats_f64() as f32 - len_b * 0.05,
        rows: Vec::new(),
        punch: None,
        markers: None,
        tempo: None,
    };
    let tempo = crate::views::bars::map(app.project());
    let painter = ui.painter_at(full);
    kit::with_brush(&painter, |b| {
        well::paint(b, full, &s, State::REST, None);
        paint_ruler(b, app, &geo, ruler, &s);
        let shown = crate::views::arrangement::AudioShown {
            track,
            clip: &c,
            region: &region,
            start: c.start,
            length: c.length,
            offset: c.offset,
            map: &tempo,
        };
        let x0 = geo.x(c.start.as_beats_f64() as f32).max(full.left());
        let x1 = geo.x(c.end().as_beats_f64() as f32).min(full.right());
        if let Some(src) = app.project().audio_source(region.source) {
            if let Lookup::Ready(peaks) = app.peaks.lookup(src, app.audio_base(), Some(ui.ctx())) {
                let n = (((x1 - x0) * b.ppp()).ceil() as usize).clamp(1, 8192);
                let (mut out, mut run) = (Vec::new(), Vec::new());
                match region.warp.as_deref().filter(|w| w.follow) {
                    Some(w) => aw::columns(
                        &peaks,
                        Which::Mono,
                        &geo,
                        &shown,
                        w,
                        x0,
                        x1,
                        n,
                        &mut out,
                        &mut run,
                    ),
                    None => {
                        let map = crate::views::bars::map(app.project());
                        let f = |x: f32| {
                            let t = geo.beats(x) as f64 * PPQ as f64;
                            cantus_core::audio_time::clip_frame_at(
                                &map,
                                c.start,
                                c.offset,
                                &region,
                                peaks.rate as f64,
                                t,
                            )
                        };
                        let (fa, fb) = (f(x0), f(x1));
                        peaks.columns(
                            Which::Mono,
                            fa,
                            ((fb - fa) / n as f64).max(1e-6),
                            n,
                            &mut out,
                        );
                    }
                }
                let band = Rect::from_min_max(pos2(x0, wave.top()), pos2(x1, wave.bottom()));
                b.hline(x0, x1, band.center().y, b.px(), s.t.text_dim);
                let gain = cantus_core::db_to_linear(region.gain_db);
                fill_peaks(b, band, &out, |_| gain, s.t.text);
            }
        }
        let inner = Rect::from_min_max(pos2(x0, wave.top()), pos2(x1, wave.bottom()));
        let hits = aw::paint(b, app, &geo, full, &shown, inner, true, &s);
        app.warp.view.hits = hits;
        app.warp.view.wave = Some(inner);
    });
    input(app, ui, &resp, r, px_per_beat / PPQ as f32);
}

fn paint_ruler(
    b: &mut kit::Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    ruler: Rect,
    s: &Style,
) {
    let map = crate::views::bars::map(app.project());
    let from = Ticks((geo.beats(ruler.left()).max(0.0) as f64 * PPQ as f64) as i64);
    let to = Ticks((geo.beats(ruler.right()) as f64 * PPQ as f64) as i64 + 1);
    b.hline(
        ruler.left(),
        ruler.right(),
        ruler.bottom(),
        b.px(),
        s.t.text_dim,
    );
    let mut word = String::new();
    for t in map.bar_lines(from, to) {
        let x = geo.x(t.as_beats_f64() as f32);
        b.vline(
            x,
            ruler.top() + ruler.height() * 0.35,
            ruler.bottom(),
            b.px(),
            s.t.text_dim,
        );
        word.clear();
        use std::fmt::Write as _;
        let _ = write!(word, "{}", map.bbt(t).0);
        let r = Rect::from_min_max(
            pos2(x + s.s(3.0), ruler.top()),
            pos2(x + s.s(40.0), ruler.bottom()),
        );
        let tx = Text::new(&word, Role::Micro, s.t.text_dim).align(Align2::LEFT_CENTER);
        let _ = text::paint(b, r, &tx, s.tk());
    }
}

fn input(app: &mut CantusApp, ui: &egui::Ui, resp: &egui::Response, r: ClipRef, px_per_tick: f32) {
    let mods = ui.input(|i| i.modifiers);
    aw::modifiers(app, mods);
    let hits = app.warp.view.hits;
    let at = |p: egui::Pos2| hits.and_then(|h| h.at(p));
    if let (true, Some(p)) = (resp.double_clicked(), resp.interact_pointer_pos()) {
        if let Some(h) = at(p) {
            aw::double_click(app, r, h);
        }
        return;
    }
    if let (true, Some(p)) = (resp.clicked(), resp.interact_pointer_pos()) {
        aw::clicked(app, r, at(p));
    }
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(h @ (Handle::WarpMarker(_) | Handle::WarpOnset(_) | Handle::WarpBand(_))) =
                at(p)
            {
                if let Some(d) = aw::begin(app, r, h) {
                    app.warp.view.drag = Some((r, d, p.x, px_per_tick.max(1e-6)));
                }
            }
        }
    }
    if resp.dragged() {
        if let (Some((r, d, x0, ppt)), Some(p)) =
            (app.warp.view.drag.clone(), resp.interact_pointer_pos())
        {
            let dx = Ticks(((p.x - x0) / ppt).round() as i64);
            aw::update(app, r, &d, dx);
        }
    }
    if resp.drag_stopped() && app.warp.view.drag.take().is_some() {
        app.end_gesture();
    }
    if let Some(p) = resp.hover_pos() {
        if let Some(h) = at(p) {
            aw::hint(ui.ctx(), app, r, h, app.warp.view.drag.is_some());
        }
    }
}

#[path = "warp_bar.rs"]
mod bar_impl;
use bar_impl::bar;
pub use bar_impl::MODES;

/// The mode the editor's segmented field shows.
pub fn mode_index(m: WarpMode) -> usize {
    WarpMode::ALL.iter().position(|x| *x == m).unwrap_or(0)
}

impl crate::warp::WarpUi {
    pub(crate) fn view_value_start(&mut self, which: u8, v: f64, x: f32) {
        self.view.value = Some((which, v, x));
    }

    pub(crate) fn view_value(&self) -> Option<(u8, f64, f32)> {
        self.view.value
    }

    pub(crate) fn view_value_end(&mut self) {
        self.view.value = None;
    }
}
