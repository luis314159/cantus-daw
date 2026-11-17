//! Lenis's editor (ADR 0143, provisional; design sheet v1): the resonance suppressor — a
//! bench analyser on its black epoxy bench, over a row of cards. The display is the
//! instrument itself: its enamelled front panel between two carrying handles, the screen
//! in its moulded bezel (the input's spectrum as a fine phosphor line, the cut hanging
//! under it as a veil and along the top as a bar per frequency, the focus curve with its
//! two gates and six numbered bands to drag), and beside it the Depth dial, the readouts
//! behind smoked glass (the most cut now, the latency) and the push keys (Quality, Curve,
//! *Listen to what is removed*, Channels); along its foot the maker's emblem and name and
//! the input and output sockets with their signal lamps. The cards hold the detection, the
//! time, the focus's limits, the selected focus band and the output. Every number from the
//! engine's telemetry (`cantus_engine::fx::lenis::t`, five interleaved pages of the
//! 128-point picture, `picture`).

use super::controls::{self, Size};
use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::fx::lenis::band_key;
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::lenis::FOCUS_BANDS;
use egui::{pos2, vec2, Rect};

#[path = "lenis_art.rs"]
mod art;
#[path = "lenis_focus.rs"]
mod focus;
#[path = "lenis_panel.rs"]
mod panel;
#[path = "lenis_picture.rs"]
pub(crate) mod picture;
#[path = "lenis_screen.rs"]
pub(crate) mod screen;

const KIND: EffectKind = EffectKind::ResonanceSuppressor;

/// Factory presets: a label key and the parameters it sets (the factory presets are data,
/// `crates/cantus-ui/presets/`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The window's designed size at 100 % (the frame's `designed_size`): the instrument over
/// one row of cards, every word whole in both languages (the window grows, the display
/// never shrinks).
pub const DESIGNED_SIZE: egui::Vec2 = egui::Vec2::new(1340.0, 900.0);

/// The states the design sheets and `fxb_whole` draw it in: a name and the settings over
/// the demo (the first is the starting preset as inserted).
pub const DESIGN_STATES: &[(&str, &[(&str, f32)])] = &[
    ("start", &[]),
    ("strong", &[("depth", 0.9), ("curve", 1.0), ("select", 0.3)]),
    (
        "delta",
        &[
            ("depth", 0.9),
            ("curve", 1.0),
            ("select", 0.3),
            ("delta", 1.0),
        ],
    ),
    (
        "bands",
        &[
            ("b1on", 1.0),
            ("b1amt", -0.8),
            ("b3on", 1.0),
            ("b3hz", 3100.0),
            ("b3amt", 0.8),
            ("b3w", 1.2),
            ("qual", 2.0),
        ],
    ),
    (
        "focus",
        &[
            ("lo", 900.0),
            ("hi", 9000.0),
            ("qual", 0.0),
            ("chan", 2.0),
            ("depth", 0.7),
        ],
    ),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The engine's picture, held whole between its pages.
    pub pic: picture::Picture,
    hold: focus::Hold,
    /// The focus band the card shows (0 … 5).
    pub band: usize,
    words: Words,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
    /// Where the screen's lanes were drawn last frame.
    pub plot: Option<screen::Plot>,
}

/// The screen's words, made again only when what they say changes.
#[derive(Debug, Default)]
struct Words {
    frame: (Option<(usize, u32, crate::i18n::Locale)>, String),
    limits: (Option<(u32, u32, crate::i18n::Locale)>, String, String),
}

impl Words {
    fn update(&mut self, set: &cantus_dsp::lenis::LenisSettings, sr: f32) {
        let lang = crate::i18n::locale();
        let g = set.quality.geometry(sr.max(1.0));
        let key = (g.n, sr.round() as u32, lang);
        if self.frame.0 != Some(key) {
            self.frame.0 = Some(key);
            let bins = format!("{:.0}", sr / g.n as f32);
            self.frame.1 = crate::i18n::tf("lenis.d.frame", &[("n", &g.n), ("hz", &bins)]);
        }
        let key = (set.low_hz.round() as u32, set.high_hz.round() as u32, lang);
        if self.limits.0 != Some(key) {
            self.limits.0 = Some(key);
            let mut hz = String::new();
            screen::hz_words(set.low_hz, &mut hz);
            self.limits.1 = format!("{} {hz}", frame::tr("lenis.d.from"));
            screen::hz_words(set.high_hz, &mut hz);
            self.limits.2 = format!("{} {hz}", frame::tr("lenis.d.to"));
        }
    }
}

/// Lenis's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.lenis);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "lenis"), &mut st, ui);
    app.fx.lenis = st;
    out
}

/// Lenis's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let set = cantus_engine::fx::lenis::settings(host.spec());
    let latency = cantus_dsp::lenis::latency_for(&set, sr.max(1.0)) as f32;
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
        body_ui(host, st, ui, s, body, &tel)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const DETECTION: &[Item] = &[
    Item::Knob("sharp", "lenis.l.sharp"),
    Item::Knob("select", "lenis.l.select"),
];
const TIME: &[Item] = &[
    Item::Knob("att", "lenis.l.att"),
    Item::Knob("rel", "lenis.l.rel"),
];
const FOCUS: &[Item] = &[
    Item::Pill("lo", "lenis.l.from", hz_pill),
    Item::Pill("hi", "lenis.l.to", hz_pill),
];
/// The selected band's knobs (their keys chosen by the band; these stand for their width).
const BAND: &[Item] = &[
    Item::Pill("b0hz", "lenis.l.freq", hz_pill),
    Item::Knob("b0amt", "lenis.l.amount"),
    Item::Pill("b0w", "lenis.l.width", octaves_pill),
];
const OUTPUT: &[Item] = &[
    Item::Knob("mix", "lenis.l.mix"),
    Item::Knob("out", "lenis.l.output"),
];
/// The band chooser's column in the band's card, at 100 %.
const CHOOSER_W: f32 = 150.0;

/// The width cell's width at 100 %: its label, or its widest value ("4.00 octaves") as the
/// value box writes it.
fn width_cell(ui: &egui::Ui) -> f32 {
    use crate::kit::text::{self as ktext, Role, Text};
    let widest = format!("4.00 {}", frame::tr("lenis.u.octaves"));
    let t = Text::new(&widest, Role::Value, egui::Color32::TRANSPARENT).caps(false);
    let mono = ktext::Face {
        mono: true,
        ..ktext::Face::ROLE
    };
    let v = ktext::measure(ui.ctx(), &t, 1.0)
        .x
        .max(ktext::measure_face(ui.ctx(), &t, 1.0, mono).x)
        + 16.0;
    cells::width(ui, &BAND[2]).max(v)
}

/// A frequency in a cell: "150 Hz", "2.5 kHz".
fn hz_pill(v: f32, b: &mut String) {
    screen::hz_words(v, b);
}

/// A width in octaves: "1.20 octaves".
fn octaves_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.2} {}", frame::tr("lenis.u.octaves"));
}

/// The instrument over its row of cards.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row = controls::card_height(Size::M) * k;
    let gap = s.d(4.0);
    let cards: [cells::Card<'_>; 5] = [
        ("lenis.card.detection", DETECTION),
        ("lenis.card.time", TIME),
        ("lenis.card.focus", FOCUS),
        ("lenis.card.band", BAND),
        ("lenis.card.output", OUTPUT),
    ];
    let mut widths = [0.0f32; 5];
    for (w, (title, items)) in widths.iter_mut().zip(&cards) {
        let extra = if *title == "lenis.card.band" {
            CHOOSER_W + 4.0 + width_cell(ui) - cells::width(ui, &BAND[2])
        } else {
            0.0
        };
        *w = k * (cells::card_width(ui, items) + extra);
    }
    let rows = cells::breaks(&widths, 6.0 * k, area.width());
    let top = (area.height() - rows.len() as f32 * (row + gap)).max(460.0 * k);
    let r1 = g.full(top);
    st.display = Some(r1);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.pic.update(tel, dt);
    display(host, st, ui, s, r1);
    if st.pic.moving() {
        ui.ctx().request_repaint();
    }
    for &(i, j) in &rows {
        let sum: f32 = widths[i..j].iter().sum::<f32>().max(1.0);
        let mut shares = [0.0f32; 5];
        for (sh, w) in shares.iter_mut().zip(&widths[i..j]) {
            *sh = w / sum;
        }
        let rects: Vec<Rect> = g.row(row, &shares[..j - i]).collect();
        for ((title, items), rect) in cards[i..j].iter().zip(rects) {
            if *title == "lenis.card.band" {
                band_card(host, st, ui, s, rect);
            } else {
                cells::card(host, ui, s, rect, title, items);
            }
        }
    }
}

/// The selected focus band's card: the chooser (1 … 6), the band's lamp in the title (on
/// or off), its frequency, amount and width.
fn band_card(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    let n = st.band.min(FOCUS_BANDS - 1);
    let on_key = band_key(n, "on");
    let on = host.spec().get(on_key) >= 0.5;
    let (_, clicked, lamp) =
        controls::card_lamp_live(ui, s, rect, Src::Key("lenis.card.band"), on, |ui, _| {
            controls::column(ui, s, CHOOSER_W, |ui| {
                controls::caption(ui, s, Src::Key("lenis.l.band"), CHOOSER_W);
                const NUMBERS: [Src<'static>; FOCUS_BANDS] = [
                    Src::Str("1"),
                    Src::Str("2"),
                    Src::Str("3"),
                    Src::Str("4"),
                    Src::Str("5"),
                    Src::Str("6"),
                ];
                let name = frame::tr("lenis.card.band");
                if let Some(i) = controls::segmented(ui, s, n, &NUMBERS, name, CHOOSER_W) {
                    st.band = i;
                }
            });
            let keys = [band_key(n, "hz"), band_key(n, "amt"), band_key(n, "w")];
            cells::knob(host, ui, s, keys[0], "lenis.l.freq", Some(&hz_pill), None);
            cells::knob(host, ui, s, keys[1], "lenis.l.amount", None, None);
            let w = width_cell(ui);
            cells::knob(
                host,
                ui,
                s,
                keys[2],
                "lenis.l.width",
                Some(&octaves_pill),
                Some(w),
            );
        });
    if clicked && !controls::card_lamp_offer(ui, s, host, on_key, lamp.as_ref()) {
        host.set(on_key, if on { 0.0 } else { 1.0 }, None);
    }
}

/// The handles' inset from the panel's sides, and the content's beyond them, at 100 %.
const HANDLE_X: f32 = 22.0;
const CONTENT_X: f32 = 48.0;

/// The instrument on its bench in `r`, its controls answering the pointer.
fn display(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let k = s.k;
    let ctx = ui.ctx().clone();
    let case = Rect::from_min_max(
        pos2(r.left() + 8.0 * k, r.top() + 8.0 * k),
        pos2(r.right() - 8.0 * k, r.bottom() - 16.0 * k),
    );
    let wrap = 10.0 * k;
    let face = case.shrink(wrap);
    let content = Rect::from_min_max(
        pos2(face.left() + CONTENT_X * k, face.top() + 16.0 * k),
        pos2(face.right() - CONTENT_X * k, face.bottom() - 10.0 * k),
    );
    let strip_h = 40.0 * k;
    let upper = Rect::from_min_max(
        content.min,
        pos2(content.right(), content.bottom() - strip_h - 8.0 * k),
    );
    let strip = Rect::from_min_max(pos2(content.left(), upper.bottom() + 8.0 * k), content.max);
    let cw = panel::width(&ctx, k);
    let col = Rect::from_min_max(pos2(upper.right() - cw, upper.top()), upper.max);
    let bezel = Rect::from_min_max(upper.min, pos2(col.left() - 24.0 * k, upper.bottom()));
    let glass = bezel.shrink(13.0 * k);
    // The words, then the screen's lanes under its head.
    let sr = host.sample_rate().max(1.0);
    let set0 = cantus_engine::fx::lenis::settings(host.spec());
    st.words.update(&set0, sr);
    let head_h = screen::head_height(
        &ctx,
        k,
        glass.width() - 28.0 * k,
        &st.words.frame.1,
        set0.delta,
    );
    let plot = screen::Plot::lay(glass, k, head_h);
    st.plot = Some(plot);
    let tab_w = |t: &str| {
        crate::kit::text::measure(
            &ctx,
            &crate::kit::text::Text::new(
                t,
                crate::kit::text::Role::Micro,
                egui::Color32::TRANSPARENT,
            )
            .caps(false),
            k,
        )
        .x
    };
    let tabs = [tab_w(&st.words.limits.1), tab_w(&st.words.limits.2)];
    // The controls first, so this frame draws what they set.
    let pan = panel::lay(&ctx, s, col);
    let press = panel::edit(host, ui, s, &pan);
    let hot = focus::edit(host, ui, s, &plot, &set0, &mut st.hold, &mut st.band, tabs);
    let spec = host.spec().clone();
    let set = cantus_engine::fx::lenis::settings(&spec);
    st.words.update(&set, sr);
    let latency_ms = cantus_dsp::lenis::latency_for(&set, sr) as f32 / sr * 1000.0;
    let ink = super::canvas::Ink::of(s);
    let words = screen::Words {
        frame: &st.words.frame.1,
        from: &st.words.limits.1,
        to: &st.words.limits.2,
    };
    with_brush(&ui.painter_at(r), |b| {
        art::bench(b, s, r);
        let face = art::case(b, s, case, wrap);
        for x in [face.left() + HANDLE_X * k, face.right() - HANDLE_X * k] {
            art::handle(b, s, x, face.top() + 46.0 * k, face.bottom() - 46.0 * k);
        }
        let _ = art::bezel(b, s, bezel);
        screen::paint(b, s, &ink, &plot, &set, &st.pic, st.band, hot, &words);
        panel::paint(b, s, &pan, &press, &spec, &st.pic, latency_ms);
        foot(b, s, strip, &st.pic);
    });
}

/// The panel's foot: the emblem, the name and the model on the left; the input and the
/// output sockets with their signal lamps on the right.
fn foot(b: &mut crate::kit::Brush, s: &Style, r: Rect, pic: &picture::Picture) {
    use crate::kit::text::{Role, Text};
    use egui::{Align2, Color32};
    let k = s.k;
    let em = Rect::from_min_size(
        pos2(r.left() + 4.0 * k, r.center().y - 14.0 * k),
        vec2(44.0 * k, 28.0 * k),
    );
    art::emblem(b, s, em);
    let name = art::print(
        b,
        s,
        pos2(em.right() + 12.0 * k, r.center().y),
        Align2::LEFT_CENTER,
        &Text::key("picker.dev.lenis.name", Role::Title, Color32::TRANSPARENT).caps(true),
        1.25,
    );
    art::rule(
        b,
        s,
        pos2(name.right() + 12.0 * k, name.top() + 2.0 * k),
        pos2(name.right() + 12.0 * k, name.bottom() - 2.0 * k),
        0.8 * k,
    );
    art::print(
        b,
        s,
        pos2(name.right() + 24.0 * k, r.center().y),
        Align2::LEFT_CENTER,
        &Text::key("lenis.model", Role::Label, Color32::TRANSPARENT).caps(false),
        1.0,
    );
    // The sockets: Output at the right, Input before it, each with its lamp.
    let level = |peak: f32| {
        if peak <= 1e-5 {
            0.0
        } else {
            ((20.0 * cantus_dsp::math::log10(peak) + 48.0) / 48.0).clamp(0.0, 1.0)
        }
    };
    let mut x = r.right() - 16.0 * k;
    for (key, peak) in [
        ("lenis.socket.out", pic.out_peak),
        ("lenis.socket.in", pic.in_peak),
    ] {
        let p = pos2(x, r.center().y);
        art::socket(b, s, p, 9.0 * k);
        let lamp = pos2(x - 30.0 * k, r.center().y + 6.0 * k);
        art::signal(b, s, lamp, level(peak));
        let t = art::print(
            b,
            s,
            pos2(x - 30.0 * k, r.center().y - 4.0 * k),
            Align2::RIGHT_BOTTOM,
            &Text::key(key, Role::Micro, Color32::TRANSPARENT),
            1.0,
        );
        x = t.left().min(lamp.x - 8.0 * k) - 40.0 * k;
    }
}

/// The inspector's picture: the curve, the depth and the quality in a line ("Soft ·
/// 50 % · Standard · from 150 Hz").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let pick = |key: &str, n: f32| {
        crate::i18n::t(&format!(
            "fx.lenis.{key}.{}",
            spec.get(key).round().clamp(0.0, n)
        ))
    };
    let mut from = String::new();
    screen::hz_words(spec.get("lo"), &mut from);
    let w = format!(
        "{} \u{b7} {:.0} % \u{b7} {} \u{b7} {} {}",
        pick("curve", 1.0),
        spec.get("depth") * 100.0,
        pick("qual", 2.0),
        frame::tr("lenis.d.from"),
        from
    );
    controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The starting preset (`cantus_core::starting`): Gentle — half of each resonance's bump
/// taken away, the soft curve, the focus from 150 Hz.
pub const DEMO: &[(&str, f32)] = &[("depth", 0.5), ("lo", 150.0)];

/// The settings `--editor lenis` and the screenshot tool open it with: the starting
/// preset.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "lenis").set_many(DEMO, "cmd.fx_preset");
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings fed two seconds of a harsh guitar (a bright strummed tone
/// through a boxy body, a cabinet ringing near 3 kHz, a whistle at 6.8 kHz, the pick's
/// hiss), its telemetry read page by page as the editor reads it.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let Some((_, effect, _)) = app.fx.open else {
        return;
    };
    let (tel, pic) = demo_picture(&spec);
    app.fx.injected = Some(tel);
    if app.fx.lenis.effect != Some(effect) {
        app.fx.lenis = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    app.fx.lenis.pic = pic;
    app.fx.lenis.pic.hold_demo();
    if spec.get("b3on") >= 0.5 {
        app.fx.lenis.band = 3;
    } else if spec.get("b1on") >= 0.5 {
        app.fx.lenis.band = 1;
    }
}

/// A number in −1 … 1 from a running seed (the programme's noise): the same every run.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// A peaking filter (the cookbook's), one channel.
#[derive(Copy, Clone)]
struct Peak {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}

impl Peak {
    fn new(sr: f32, hz: f32, q: f32, db: f32) -> Peak {
        use cantus_dsp::math::{cos, db_to_gain, sin};
        let a = db_to_gain(db * 0.5);
        let w = std::f32::consts::TAU * hz / sr;
        let alpha = sin(w) / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        Peak {
            b: [
                (1.0 + alpha * a) / a0,
                -2.0 * cos(w) / a0,
                (1.0 - alpha * a) / a0,
            ],
            a: [-2.0 * cos(w) / a0, (1.0 - alpha / a) / a0],
            z: [0.0; 2],
        }
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// Lenis at `spec` fed the harsh guitar: its last telemetry and the picture the editor
/// would hold, read from every page as it came.
pub(crate) fn demo_picture(spec: &EffectSpec) -> ([f32; 64], picture::Picture) {
    use cantus_dsp::math::{exp, sin};
    use cantus_engine::FxUnit as _;
    use std::f32::consts::TAU;
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::lenis::Unit::new(sr);
    u.configure(spec);
    let n = 64usize;
    let secs = 2.0f32;
    let blocks = (secs * sr) as usize / n;
    let mut pic = picture::Picture::default();
    let mut seed = 0x2468_ACE1u32;
    // The body's box, the cabinet's ring, the whistle: in both channels.
    let mut chain = [[
        Peak::new(sr, 420.0, 2.2, 7.0),
        Peak::new(sr, 3100.0, 9.0, 13.0),
        Peak::new(sr, 6800.0, 7.0, 10.0),
    ]; 2];
    let mut hiss = [Peak::new(sr, 7400.0, 3.0, 12.0); 2];
    // A strum every half second: E minor's notes, a little apart.
    let notes = [82.41f32, 123.47, 164.81, 196.0, 246.94, 329.63];
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    for blk in 0..blocks {
        for i in 0..n {
            let t = (blk * n + i) as f32 / sr;
            let ts = t % 0.5;
            let mut tone = 0.0;
            for (j, f) in notes.iter().enumerate() {
                let start = j as f32 * 0.012;
                if ts < start {
                    continue;
                }
                let te = ts - start;
                let env = exp(-te / 0.9) * (1.0 - exp(-te / 0.002));
                // A bright string: its partials falling as one over their number.
                let mut v = 0.0;
                for h in 1..=14 {
                    let fh = f * h as f32;
                    if fh > 9_000.0 {
                        break;
                    }
                    v += sin(TAU * fh * t + h as f32) / h as f32;
                }
                tone += env * v;
            }
            let pick = noise(&mut seed) * 0.05 * exp(-ts / 0.03);
            for (ch, out) in [&mut l, &mut r].into_iter().enumerate() {
                let mut x = 0.22 * tone + pick + noise(&mut seed) * 0.002;
                let h = hiss[ch].run(pick * 0.6);
                for p in chain[ch].iter_mut() {
                    x = p.run(x);
                }
                out[i] = 0.5 * (x + h);
            }
        }
        u.process(&mut l, &mut r, None);
        pic.update(&u.telemetry(), n as f32 / sr);
    }
    (u.telemetry(), pic)
}
