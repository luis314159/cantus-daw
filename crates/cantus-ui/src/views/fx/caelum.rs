//! Caelum's editor (ADR 0141, provisional; design sheet v2): the shimmer reverb — an
//! observatory over two rows of cards. The display is the unit's own picture: the sky in a
//! metal window, the tail glowing on the horizon over the tank's water and the shimmer's
//! octaves rising band by band above it as the tail ages; the endless controls as lit
//! pushbuttons on the sill; the voices on an astrolabe; the decay at each octave in eight
//! glass tubes. Every number from the engine's telemetry (`cantus_engine::fx::caelum::t`).

use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::Rect;

#[path = "caelum_art.rs"]
mod art;
#[path = "caelum_astro.rs"]
mod astro;
#[path = "caelum_desk.rs"]
mod desk;
#[path = "caelum_display.rs"]
pub(crate) mod display;
#[path = "caelum_sky.rs"]
mod sky;

const KIND: EffectKind = EffectKind::Shimmer;

/// Factory presets: data (`presets/effects/shimmer.toml`, ADR 0101).
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
    /// Where the display's controls were drawn last frame: the sill's six buttons, the
    /// astrolabe, the decay rack (tests point at them).
    drawn: Option<display::Drawn>,
}

/// Caelum's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.caelum);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "caelum"), &mut st, ui);
    app.fx.caelum = st;
    out
}

/// Caelum's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
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

/// The pre-delay's sync, as its menu names each choice.
const PSYNC: &[&str] = &[
    "fx.caelum.psync.0",
    "fx.caelum.psync.1",
    "fx.caelum.psync.2",
    "fx.caelum.psync.3",
    "fx.caelum.psync.4",
    "fx.caelum.psync.5",
    "fx.caelum.psync.6",
    "fx.caelum.psync.7",
    "fx.caelum.psync.8",
];
const SPACE: &[Item] = &[
    Item::Knob("size", "caelum.l.size"),
    Item::Knob("decay", "caelum.l.decay"),
    Item::Knob("pre", "caelum.l.pre"),
    Item::Choice("psync", "caelum.l.sync", PSYNC),
    Item::Knob("diff", "caelum.l.diffusion"),
    Item::Switch(
        "lines",
        "caelum.l.lines",
        &["caelum.s.eight", "caelum.s.sixteen"],
    ),
];
const DAMPING: &[Item] = &[
    Item::Knob("hhz", "caelum.l.high"),
    Item::Knob("hdec", "caelum.l.highdecay"),
    Item::Knob("lhz", "caelum.l.low"),
    Item::Knob("ldec", "caelum.l.lowdecay"),
];
const SHIMMER: &[Item] = &[
    Item::Knob("shim", "caelum.l.amount"),
    Item::Pill("pa", "caelum.l.first", cells::semis_pill),
    Item::Pill("pb", "caelum.l.second", cells::semis_pill),
    Item::Knob("pblvl", "caelum.l.secondlevel"),
];
const TONE: &[Item] = &[
    Item::Knob("slo", "caelum.l.lowcut"),
    Item::Knob("shi", "caelum.l.highcut"),
];
const MOTION: &[Item] = &[
    Item::Knob("mdepth", "caelum.l.depth"),
    Item::Knob("mrate", "caelum.l.rate"),
    Item::Knob("bloom", "caelum.l.bloom"),
];
const DUCKING: &[Item] = &[
    Item::Knob("duck", "caelum.l.range"),
    Item::Knob("dthr", "caelum.l.threshold"),
    Item::Knob("datt", "caelum.l.attack"),
    Item::Knob("drel", "caelum.l.release"),
];
const OUTPUT: &[Item] = &[
    Item::Knob("lcut", "caelum.l.lowcut"),
    Item::Knob("hcut", "caelum.l.highcut"),
    Item::Knob("width", "caelum.l.width"),
    Item::Knob("mix", "caelum.l.mix"),
    Item::Knob("out", "caelum.l.output"),
];

/// The observatory over its cards.
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
    let row = super::controls::card_height(super::controls::Size::M) * k;
    let gap = s.d(4.0);
    // The endless controls are the sill's pushbuttons (in the display); the cards laid in
    // as few rows as fit, evenly filled (`fxa_cells::pack`).
    let cards: &[cells::Card<'_>] = &[
        ("caelum.card.space", SPACE),
        ("caelum.card.damping", DAMPING),
        ("caelum.card.shimmer", SHIMMER),
        ("caelum.card.tone", TONE),
        ("caelum.card.motion", MOTION),
        ("caelum.card.ducking", DUCKING),
        ("caelum.card.output", OUTPUT),
    ];
    let phys = cells::pack(ui, area.width(), k, cards);
    let top = (area.height() - phys.len() as f32 * (row + gap)).max(380.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    let reading = display::Reading::of(tel);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.view.update(&reading, dt);
    st.drawn = Some(display::show(host, &mut st.view, ui, s, r1, &reading));
    if reading.seen {
        ui.ctx().request_repaint();
    }
    for cards in &phys {
        cells::row(host, ui, s, &mut g, row, cards);
    }
}

/// The inspector's picture: the space in a line ("6.5 s · Amount 55 % · +12 st").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let decay = if spec.get("inf") >= 0.5 {
        frame::tr("caelum.d.infinite").to_string()
    } else {
        format!("{:.1} s", spec.get("decay"))
    };
    let w = format!(
        "{decay} \u{b7} {} {:.0} % \u{b7} {:+.0} st",
        frame::tr("caelum.l.amount"),
        100.0 * spec.get("shim"),
        spec.get("pa"),
    );
    super::controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The starting preset (`cantus_core::starting`): Octave shimmer.
pub const DEMO: &[(&str, f32)] = &[
    ("size", 0.72),
    ("decay", 6.5),
    ("pre", 30.0),
    ("diff", 0.8),
    ("hhz", 7_000.0),
    ("hdec", 0.45),
    ("mdepth", 0.35),
    ("mrate", 0.35),
    ("shim", 0.55),
    ("slo", 350.0),
    ("mix", 0.35),
    ("out", -0.9),
];

/// The settings `--editor caelum` and the screenshot tool open it with: the starting
/// preset.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "caelum").set_many(DEMO, "cmd.fx_preset");
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings fed four seconds of a sung chord and then left to ring,
/// its telemetry sampled as the engine sends it, the display's history filled from it.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    // Ducking shows while the chord still sounds: that state's demo stops there.
    let secs = if spec.get("duck") > 0.0 { 3.6 } else { 7.5 };
    let (tel, history) = demo_telemetry(&spec, secs);
    app.fx.injected = Some(tel);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.caelum.effect != Some(effect) {
            app.fx.caelum = State {
                effect: Some(effect),
                ..State::default()
            };
        }
        app.fx.caelum.view.seed(&history);
    }
}

/// Caelum at `spec` fed a chord for four seconds, `secs` in all (frozen, if `spec` says so,
/// at five): the last telemetry and the display's readings every thirtieth of a second.
pub(crate) fn demo_telemetry(spec: &EffectSpec, secs: f32) -> ([f32; 64], Vec<display::Reading>) {
    use cantus_engine::FxUnit as _;
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::caelum::Unit::new(sr);
    // A frozen state's demo freezes the tail a second after the chord, as a player would.
    let freeze_at = (spec.get("freeze") >= 0.5).then_some(5.0f32);
    let mut open = spec.clone();
    if freeze_at.is_some() {
        let _ = open.set("freeze", 0.0);
    }
    u.configure(&open);
    let n = 1_600;
    let blocks = (secs * sr) as usize / n;
    let mut history = Vec::with_capacity(blocks);
    for blk in 0..blocks {
        if freeze_at.is_some_and(|at| blk == (at * sr) as usize / n) {
            u.configure(spec);
        }
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            let t = (blk * n + i) as f32 / sr;
            if t < 4.0 {
                let env = (t * 3.0).min(1.0) * (1.0 - (t - 3.5).max(0.0) * 2.0);
                let mut x = 0.0;
                for f in [220.0f32, 277.18, 329.63, 440.0] {
                    x += 0.06 * cantus_dsp::math::sin(std::f32::consts::TAU * f * t);
                }
                *a = x * env;
                *b = x * env * 0.9;
            }
        }
        u.process(&mut l, &mut r, None);
        history.push(display::Reading::of(&u.telemetry()));
    }
    (u.telemetry(), history)
}

impl State {
    /// How many readings the display's history holds.
    pub fn history_len(&self) -> usize {
        self.view.len()
    }

    /// Where the sill's buttons were drawn last frame: Infinite, Freeze, Fades, Sustains,
    /// 8 dB lower, As loud.
    pub fn endless_rects(&self) -> [Rect; 6] {
        self.drawn.map_or([Rect::NOTHING; 6], |d| d.endless)
    }

    /// Where `st` semitones lies on the astrolabe's spiral, as drawn last frame (the
    /// alidade's jewel at the first voice's, the slimmer hand's at the second's).
    pub fn voice_point(&self, st: f32) -> Option<egui::Pos2> {
        self.drawn.map(|d| display::voice_point(&d, st))
    }

    /// The decay rack as drawn last frame: its scale's rect, and the height of `secs` on it.
    pub fn decay_rack(&self, secs: f32) -> Option<(Rect, f32)> {
        self.drawn.map(|d| (d.decay, display::decay_y(&d, secs)))
    }
}
