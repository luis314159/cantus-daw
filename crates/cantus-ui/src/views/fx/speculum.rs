//! Speculum's editor: the analyser (ADR 0056) — Wave Candy's views and SPAN's spectrum,
//! measured to the standards.
//!
//! Six views as tabs (**overview**, spectrum, spectrogram, oscilloscope, vectorscope,
//! loudness), each with only the settings it reads beneath it. Every frame of the
//! Speculum's own signal reaches the editor (`FxHost::take_stereo`: the engine's scope ring
//! in Cantus, the plug-in's tap elsewhere) and is measured here, on the editor's thread
//! (`cantus_dsp::speculum::Analysis`), so what it reads does not depend on the frame rate.
//!
//! **Docked or floating, several at once.** The editor floats like every effect's; its ⋯
//! menu docks it in the workspace as a tab of its own — resizable, split, torn out as a
//! window — and any number of Speculums can be docked and watched at once (eight engine
//! rings), each with its own measurements.

use super::canvas::Ink;
use super::controls::{self, Size};
use super::dynfx;
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::speculum::{Analysis, SpeculumSettings};
use egui::{pos2, vec2, Rect};
use std::collections::VecDeque;

#[path = "speculum_draw.rs"]
mod draw;

const KIND: EffectKind = EffectKind::Analyzer;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    ("preset.speculum.mix", &[("view", 0.0)]),
    (
        "preset.speculum.master",
        &[("view", 5.0), ("target", -14.0)],
    ),
    (
        "preset.speculum.broadcast",
        &[("view", 5.0), ("target", -23.0)],
    ),
    (
        "preset.speculum.bass",
        &[("view", 1.0), ("fft", 4.0), ("chan", 4.0), ("avg", 600.0)],
    ),
    (
        "preset.speculum.drums",
        &[("view", 3.0), ("sync", 2.0), ("beats", 3.0)],
    ),
    ("preset.speculum.stereo", &[("view", 4.0)]),
];

/// The views' names, by the `view` parameter's index.
pub const VIEW_KEYS: [&str; 6] = [
    "speculum.view.0",
    "speculum.view.1",
    "speculum.view.2",
    "speculum.view.3",
    "speculum.view.4",
    "speculum.view.5",
];

/// Short-term loudness history points kept: 30 s at one a 100 ms.
const HISTORY: usize = 300;

/// One Speculum's measurements and what its displays keep between frames.
pub struct View {
    pub analysis: Box<Analysis>,
    pairs: Vec<(f32, f32)>,
    l: Vec<f32>,
    r: Vec<f32>,
    lo: Vec<f32>,
    hi: Vec<f32>,
    texture: Option<egui::TextureHandle>,
    tex_rows: u64,
    /// The short-term loudness every 100 ms of audio, oldest first.
    history: VecDeque<f32>,
    since: usize,
    /// Frames the stream lost (a full ring): the measurements have gaps that long.
    pub lost: u64,
    /// The host gives this Speculum's frames.
    pub stream: bool,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View")
            .field("frames", &self.analysis.frames())
            .field("stream", &self.stream)
            .finish()
    }
}

impl View {
    pub fn new(sample_rate: f32) -> View {
        View {
            analysis: Box::new(Analysis::new(sample_rate)),
            pairs: Vec::with_capacity(8_192),
            l: Vec::with_capacity(8_192),
            r: Vec::with_capacity(8_192),
            lo: vec![0.0; 1_024],
            hi: vec![0.0; 1_024],
            texture: None,
            tex_rows: 0,
            history: VecDeque::with_capacity(HISTORY),
            since: 0,
            stream: false,
            lost: 0,
        }
    }

    /// Take what the host has of this Speculum's frames and measure them.
    pub fn update(&mut self, host: &mut dyn FxHost, set: &SpeculumSettings) {
        let sr = host.sample_rate();
        if (sr - self.analysis.sample_rate()).abs() > 0.5 && sr > 1_000.0 {
            *self = View::new(sr);
        }
        self.analysis.set(set);
        let mut pairs = std::mem::take(&mut self.pairs);
        pairs.clear();
        let take = host.take_stereo(&mut pairs);
        self.stream = take.stream;
        self.lost += take.lost;
        self.l.clear();
        self.r.clear();
        for (a, b) in &pairs {
            self.l.push(*a);
            self.r.push(*b);
        }
        self.pairs = pairs;
        let (l, r) = (std::mem::take(&mut self.l), std::mem::take(&mut self.r));
        self.feed(&l, &r, take.clock);
        self.l = l;
        self.r = r;
    }

    /// Measure frames (the host's, or a demo's).
    pub fn feed(&mut self, l: &[f32], r: &[f32], clock: Option<(f64, f64)>) {
        if l.is_empty() {
            return;
        }
        self.analysis.push(l, r, clock);
        self.since += l.len();
        let step = (0.1 * self.analysis.sample_rate()) as usize;
        if self.since >= step {
            self.since %= step.max(1);
            if self.history.len() == HISTORY {
                self.history.pop_front();
            }
            let s = self.analysis.loudness().short_term;
            self.history
                .push_back(if s.is_finite() { s as f32 } else { -200.0 });
        }
    }

    pub fn history(&self) -> impl Iterator<Item = f32> + '_ {
        self.history.iter().copied()
    }
}

/// A Speculum docked in the workspace.
#[derive(Debug)]
pub struct Docked {
    pub strip: StripId,
    pub effect: EffectId,
    view: Option<View>,
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The floating editor's measurements.
    view: Option<View>,
    /// The Speculums docked in the workspace, each with its own.
    pub docked: Vec<Docked>,
    /// The project the docked tabs belong to (`CantusApp::epoch`): another project opened
    /// closes them (review 1).
    docked_epoch: Option<u64>,
    /// Where the views' tabs and the display were drawn last frame (tests point at them).
    pub tabs: Option<Rect>,
    pub display: Option<Rect>,
}

impl State {
    /// The floating editor's view, made at `sr` when it has none.
    fn view(&mut self, sr: f32) -> &mut View {
        self.view.get_or_insert_with(|| View::new(sr))
    }
}

/// Speculum's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    // Docked already: opening it brings its tab forward — one Speculum, one view of it
    // (two would share one stream).
    if is_docked(app, effect) {
        let key = egui::Id::new(("speculum-dock", effect.0)).value();
        crate::views::workspace_device::present(ui.ctx(), key);
        return frame::Out {
            close: true,
            ..frame::Out::default()
        };
    }
    let mut st = std::mem::take(&mut app.fx.speculum);
    if st.effect != Some(effect) {
        let docked = std::mem::take(&mut st.docked);
        st = State {
            effect: Some(effect),
            docked,
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "speculum"),
        &mut st,
        ui,
    );
    app.fx.speculum = st;
    // ⋯ › Dock in the workspace: a tab of its own, and the floating editor closes.
    if out.more == Some(0) {
        dock(app, strip, effect);
        app.close_fx_editor();
    }
    out
}

/// Speculum's editor on any host (ADR 0020).
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let sr = host.sample_rate();
    let set = cantus_engine::fx::speculum::settings(host.spec());
    st.view(sr).update(host, &set);
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let lv = st.view(sr).analysis.levels();
    let meter = (st.view(sr).analysis.frames() > 0).then_some(frame::Levels {
        level: lv.peak_db,
        hold: lv.peak_db,
    });
    // Docking is Cantus's: a plug-in window has no workspace.
    let dock = [frame::action(Src::Key("speculum.dock"))];
    let more: &[crate::kit::menu::Item<'_>] = if host.daw().is_some() { &dock } else { &[] };
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter,
        more,
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        let mut view = st.view.take();
        if let Some(v) = view.as_mut() {
            let (tabs, display) = body_ui(host, v, ui, s, body.shrink(s.d(8.0)), &set);
            st.tabs = Some(tabs);
            st.display = Some(display);
        }
        st.view = view;
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The tabs, the view and its settings in `area`: what the floating editor and a docked
/// tab both draw. Returns where the tabs and the display went.
pub fn body_ui(
    host: &mut dyn FxHost,
    view: &mut View,
    ui: &mut egui::Ui,
    s: &Style,
    area: Rect,
    set: &SpeculumSettings,
) -> (Rect, Rect) {
    let k = s.k;
    let so = s.function(Function::Output);
    let gap = s.d(4.0);
    let tabs_w = (area.width() / k).min(560.0);
    let tabs = Rect::from_min_size(area.min, vec2(tabs_w * k, so.d(controls::ROW)));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(tabs)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let labels = VIEW_KEYS.map(Src::Key);
    controls::segmented_param(&mut child, &so, host, "view", &labels, tabs_w);
    let which = cantus_engine::fx::speculum::view_of(host.spec());
    let settings_h = if which == 4 {
        0.0
    } else {
        controls::card_height(Size::M) * k
    };
    let display = Rect::from_min_max(
        pos2(area.left(), tabs.bottom() + gap),
        pos2(
            area.right(),
            area.bottom() - settings_h - if settings_h > 0.0 { gap } else { 0.0 },
        ),
    );
    let ink = Ink::of(&so);
    draw::view(ui, &so, &ink, display, view, set, which);
    if settings_h > 0.0 {
        let settings = Rect::from_min_max(pos2(area.left(), area.bottom() - settings_h), area.max);
        settings_card(host, view, ui, &so, settings, which);
    }
    (tabs, display)
}

/// The settings the view reads: the spectrum's, the scope's, or the loudness's.
fn settings_card(
    host: &mut dyn FxHost,
    view: &mut View,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    which: usize,
) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("speculum.card.settings"),
        None,
        |ui, _| {
            let pill_ms = Some(&super::modfx::ms_pill as controls::Pill<'_>);
            let seg = |ui: &mut egui::Ui,
                       host: &mut dyn FxHost,
                       key: &'static str,
                       caption: &'static str,
                       labels: &[Src<'_>],
                       w: f32| {
                controls::column(ui, s, w, |ui| {
                    controls::caption(ui, s, Src::Key(caption), w);
                    controls::segmented_param(ui, s, host, key, labels, w);
                });
            };
            if matches!(which, 0..=2) {
                seg(
                    ui,
                    host,
                    "fft",
                    "speculum.k.fft",
                    &FFT_KEYS.map(Src::Key),
                    200.0,
                );
                dynfx::param_labelled(ui, s, host, "avg", Size::M, pill_ms, "speculum.k.avg");
                let slope = Some(&slope_pill as controls::Pill<'_>);
                dynfx::param_labelled(ui, s, host, "slope", Size::M, slope, "speculum.k.slope");
                seg(
                    ui,
                    host,
                    "chan",
                    "speculum.k.chan",
                    &CHAN_KEYS.map(Src::Key),
                    200.0,
                );
                seg(
                    ui,
                    host,
                    "floor",
                    "speculum.k.floor",
                    &FLOOR_KEYS.map(Src::Key),
                    170.0,
                );
                controls::column(ui, s, 96.0, |ui| {
                    controls::caption(ui, s, Src::Key("fx.speculum.hold"), 96.0);
                    controls::led_param(
                        ui,
                        s,
                        host,
                        "hold",
                        Src::Key("speculum.legend.held"),
                        96.0,
                    );
                });
            }
            if which == 3 {
                seg(
                    ui,
                    host,
                    "sync",
                    "speculum.k.sync",
                    &SYNC_KEYS.map(Src::Key),
                    210.0,
                );
                // The length counts on the beat; the window counts free or triggered.
                let beat = host.spec().get("sync") >= 1.5;
                ui.add_enabled_ui(beat, |ui| {
                    seg(
                        ui,
                        host,
                        "beats",
                        "speculum.k.length",
                        &BEATS_KEYS.map(Src::Key),
                        330.0,
                    );
                });
                ui.add_enabled_ui(!beat, |ui| {
                    dynfx::param_labelled(ui, s, host, "ms", Size::M, pill_ms, "speculum.k.window");
                });
            }
            if which == 5 {
                let lufs = Some(&lufs_pill as controls::Pill<'_>);
                dynfx::param_labelled(ui, s, host, "target", Size::M, lufs, "speculum.k.target");
                controls::column(ui, s, 120.0, |ui| {
                    controls::caption(ui, s, Src::Key("speculum.m.integrated"), 120.0);
                    let r = controls::key(
                        ui,
                        s,
                        Src::Key("speculum.reset"),
                        frame::tr("speculum.reset_tip"),
                        120.0,
                        false,
                    );
                    if r.clicked() {
                        view.analysis.reset_loudness();
                        view.lost = 0;
                        view.history.clear();
                    }
                });
            }
        },
    );
}

const FFT_KEYS: [&str; 5] = [
    "speculum.fft.0",
    "speculum.fft.1",
    "speculum.fft.2",
    "speculum.fft.3",
    "speculum.fft.4",
];
const CHAN_KEYS: [&str; 5] = [
    "speculum.chan.0",
    "speculum.chan.1",
    "speculum.chan.2",
    "speculum.chan.3",
    "speculum.chan.4",
];
const FLOOR_KEYS: [&str; 3] = ["speculum.floor.0", "speculum.floor.1", "speculum.floor.2"];
const SYNC_KEYS: [&str; 3] = ["speculum.sync.0", "speculum.sync.1", "speculum.sync.2"];
const BEATS_KEYS: [&str; 6] = [
    "speculum.beats.0",
    "speculum.beats.1",
    "speculum.beats.2",
    "speculum.beats.3",
    "speculum.beats.4",
    "speculum.beats.5",
];

fn slope_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.1} dB");
}

fn lufs_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.0} LUFS");
}

/// The Speculums docked in the workspace, each a tab of its own (ADR 0024 §5): drawn every
/// frame from `views::fx::window`, before the floating editor. First, the scope rings no
/// one asked for during the last frame are let go.
pub fn docked(app: &mut CantusApp, ctx: &egui::Context) {
    app.engine.release_unwatched_scopes();
    let epoch = app.epoch;
    let st = &mut app.fx.speculum;
    if st.docked_epoch != Some(epoch) {
        if st.docked_epoch.is_some() {
            st.docked.clear();
        }
        st.docked_epoch = Some(epoch);
    }
    let mut docked = std::mem::take(&mut app.fx.speculum.docked);
    // Only while the effect is there and is a Speculum: another project may hold other
    // effects under the same ids (review 1).
    docked.retain(|d| {
        app.project()
            .effect(d.strip, d.effect)
            .is_some_and(|e| e.spec.kind == KIND)
    });
    for d in &mut docked {
        let mut open = true;
        let name = crate::i18n::t(KIND.name_key());
        // Where it sits, as the floating editor says it: "(Insert 1 · Strip 1 · Keys)".
        let context = DawHost::new(app, d.strip, d.effect, "speculum").context();
        let title = if context.is_empty() {
            name
        } else {
            format!("{name} ({context})")
        };
        let (strip, effect) = (d.strip, d.effect);
        // Measured every frame, shown or not: a tab behind another (or minimised) still
        // hears every frame, so its loudness has no gap when it comes to the front.
        let set = {
            let mut host = DawHost::new(app, strip, effect, "speculum");
            let sr = host.sample_rate();
            let set = cantus_engine::fx::speculum::settings(host.spec());
            d.view
                .get_or_insert_with(|| View::new(sr))
                .update(&mut host, &set);
            set
        };
        let view = &mut d.view;
        crate::views::workspace_device::DeviceWindow::new(title)
            .id(egui::Id::new(("speculum-dock", effect.0)))
            .open(&mut open)
            .default_size([760.0, 460.0])
            .min_size([360.0, 240.0])
            .hue(Function::Output)
            .show(ctx, |ui| {
                let s = Style::current().function(Function::Output);
                let mut host = DawHost::new(app, strip, effect, "speculum");
                let Some(v) = view.as_mut() else {
                    return;
                };
                let area = ui.max_rect().shrink(s.d(6.0));
                canvas_ground(ui, &s, ui.max_rect());
                body_ui(&mut host, v, ui, &s, area, &set);
            });
        if !open {
            d.view = None;
            d.effect = EffectId(u64::MAX);
        }
    }
    docked.retain(|d| d.effect != EffectId(u64::MAX));
    app.fx.speculum.docked = docked;
}

/// A docked tab's ground: the device's surface behind the views.
fn canvas_ground(ui: &egui::Ui, s: &Style, rect: Rect) {
    crate::kit::with_brush(ui.painter(), |b| {
        b.rect(rect, crate::kit::Fill::Solid(s.t.device_ground));
    });
}

/// Whether a Speculum is docked in the workspace now.
pub fn is_docked(app: &CantusApp, effect: EffectId) -> bool {
    app.fx.speculum.docked.iter().any(|d| d.effect == effect)
}

/// The inspector's picture: a still spectrum's shape, and the view in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, s.d(44.0)), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let ink = Ink::of(&s);
        let k = s.k;
        let which = cantus_engine::fx::speculum::view_of(spec);
        crate::kit::with_brush(&ui.painter_at(rect), |b| {
            super::canvas::ground_b(b, rect, &ink, s.hue_tok);
            // A pink-ish slope with a few peaks: the picture of a spectrum.
            let plot = rect.shrink2(vec2(6.0 * k, 6.0 * k));
            const N: usize = 48;
            let mut pts = [egui::Pos2::ZERO; N];
            for (j, p) in pts.iter_mut().enumerate() {
                let t = j as f32 / (N - 1) as f32;
                let bump =
                    cantus_dsp::math::sin(t * 19.0) * 0.08 + cantus_dsp::math::sin(t * 7.0) * 0.1;
                *p = pos2(
                    plot.left() + plot.width() * t,
                    plot.top() + plot.height() * (0.2 + 0.6 * t - bump).clamp(0.0, 1.0),
                );
            }
            b.area(&pts, plot.bottom(), ink.fill(0.5));
            b.polyline(&pts, 1.6 * k, ink.hue);
            super::canvas::putf(
                b,
                pos2(rect.right() - 6.0 * k, rect.top() + 4.0 * k),
                egui::Align2::RIGHT_TOP,
                crate::kit::text::Role::Value,
                ink.text,
                k,
                Some(ink.plate),
                format_args!("{}", frame::tr(VIEW_KEYS[which])),
            );
            super::canvas::edge_b(b, rect, &ink);
        });
    }
    resp
}

/// The settings `--editor speculum` and the screenshot tool open it with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "speculum").set_many(&[("view", 0.0)], "cmd.fx_preset");
}

/// A mix to look at: a kick on every beat, a bass, a chord and noise, the right channel
/// a little different from the left, at 128 BPM.
pub fn demo_signal(n: usize) -> (f32, f32) {
    let sr = 48_000.0f32;
    let t = n as f32 / sr;
    let per_beat = (60.0 / 128.0 * sr) as usize;
    let tb = (n % per_beat) as f32 / sr;
    let kick = cantus_dsp::math::exp(-tb / 0.08)
        * cantus_dsp::math::sin(
            std::f32::consts::TAU * (50.0 + 90.0 * cantus_dsp::math::exp(-tb / 0.02)) * tb,
        );
    let bass = 0.18 * cantus_dsp::math::sin(std::f32::consts::TAU * 55.0 * t);
    let chord = [220.0f32, 277.2, 329.6, 440.0]
        .iter()
        .map(|f| 0.05 * cantus_dsp::math::sin(std::f32::consts::TAU * f * t))
        .sum::<f32>();
    let hiss =
        0.01 * cantus_dsp::math::sin(n as f32 * 2.7) * cantus_dsp::math::sin(n as f32 * 0.37);
    let l = 0.5 * kick + bass + chord + hiss;
    let r = 0.5 * kick + bass + 0.8 * chord - hiss;
    (l, r)
}

/// What a playing engine would feed the editor: twelve seconds of [`demo_signal`] with the
/// song's position, measured.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let set = cantus_engine::fx::speculum::settings(&spec);
    let mut st = std::mem::take(&mut app.fx.speculum);
    st.effect = app.fx.open.map(|o| o.1);
    let v = st.view(48_000.0);
    v.analysis.set(&set);
    let block = 800;
    let (mut l, mut r) = (vec![0f32; block], vec![0f32; block]);
    let per_beat = 60.0 / 128.0 * 48_000.0;
    for blk in 0..720 {
        for i in 0..block {
            let (a, b) = demo_signal(blk * block + i);
            l[i] = a;
            r[i] = b;
        }
        let last = (blk * block + block - 1) as f64;
        v.feed(&l, &r, Some((last / per_beat, 128.0)));
    }
    app.fx.speculum = st;
}

/// Dock a Speculum in the workspace: a tab of its own from the next frame, with its own
/// measurements (the floating editor's, when it is that one).
pub fn dock(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let st = &mut app.fx.speculum;
    if st.docked.iter().any(|d| d.effect == effect) {
        return;
    }
    let view = if st.effect == Some(effect) {
        st.view.take()
    } else {
        None
    };
    st.docked.push(Docked {
        strip,
        effect,
        view,
    });
}

/// Frames a Speculum's views have measured so far — the floating editor's or a docked
/// tab's — or `None` when none shows it.
pub fn measured(app: &CantusApp, effect: EffectId) -> Option<u64> {
    let st = &app.fx.speculum;
    st.docked
        .iter()
        .find(|d| d.effect == effect)
        .and_then(|d| d.view.as_ref())
        .or_else(|| st.view.as_ref().filter(|_| st.effect == Some(effect)))
        .map(|v| v.analysis.frames())
}
