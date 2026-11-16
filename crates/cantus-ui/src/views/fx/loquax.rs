//! Loquax's editor (ADR 0139, provisional): the vocoder — the bands lit by a voice over
//! three rows of cards. The display is the unit's own picture: every band a column of
//! fine cells lit by what it says now, the drawn band levels across them, the formant's
//! shift along the top, and who speaks and who sings (the voice, the carrier, their
//! levels, the synthesiser's voices or the robot's note, the consonant light). Every number
//! from the engine's telemetry (`cantus_dsp::loquax::t`, `cantus_engine::fx::loquax`).

use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::Rect;

#[path = "loquax_display.rs"]
pub(crate) mod display;

const KIND: EffectKind = EffectKind::Vocoder;

/// Factory presets: data (`presets/effects/vocoder.toml`, ADR 0101).
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
}

/// Loquax's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.loquax);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "loquax"), &mut st, ui);
    app.fx.loquax = st;
    out
}

/// Loquax's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
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

const CARRIER: &[Item] = &[
    Item::Switch(
        "src",
        "loquax.l.carrier",
        &["loquax.s.sidechain", "loquax.s.synth", "loquax.s.both"],
    ),
    Item::Switch(
        "swap",
        "loquax.l.roles",
        &["loquax.s.track", "loquax.s.key"],
    ),
    Item::Knob("cpass", "loquax.l.through"),
];
const BANDS: &[Item] = &[
    Item::Knob("bands", "loquax.l.bands"),
    Item::Switch(
        "order",
        "loquax.l.slope",
        &["loquax.s.order4", "loquax.s.order6", "loquax.s.order8"],
    ),
    Item::Knob("lo", "loquax.l.lowest"),
    Item::Knob("hi", "loquax.l.highest"),
    Item::Knob("width", "loquax.l.width"),
];
const FOLLOW: &[Item] = &[
    Item::Knob("att", "loquax.l.attack"),
    Item::Knob("rel", "loquax.l.release"),
    Item::Switch("hold", "loquax.l.hold", &["loquax.s.off", "loquax.s.on"]),
];
const CHARACTER: &[Item] = &[
    Item::Knob("formant", "loquax.l.formant"),
    Item::Knob("bshift", "loquax.l.shift"),
    Item::Pill("tilt", "loquax.l.tilt", tilt_pill),
    Item::Knob("flat", "loquax.l.flatten"),
    Item::Knob("spread", "loquax.l.spread"),
];
const CONSONANTS: &[Item] = &[
    Item::Knob("sib", "loquax.l.sibilance"),
    Item::Knob("sibhz", "loquax.l.corner"),
    Item::Switch(
        "sibgate",
        "loquax.l.when",
        &["loquax.s.always", "loquax.s.consonants"],
    ),
    Item::Knob("unv", "loquax.l.noise"),
];
const OSCILLATORS: &[Item] = &[
    Item::Knob("saw", "loquax.l.saw"),
    Item::Knob("pulse", "loquax.l.pulse"),
    Item::Knob("pw", "loquax.l.pulsewidth"),
    Item::Knob("noise", "loquax.l.noise"),
    Item::Knob("oct", "loquax.l.octave"),
];
const UNISON: &[Item] = &[
    Item::Knob("uni", "loquax.l.copies"),
    Item::Knob("detune", "loquax.l.detune"),
    Item::Knob("uspread", "loquax.l.spread"),
];
const ENVELOPE: &[Item] = &[
    Item::Knob("satt", "loquax.l.attack"),
    Item::Knob("srel", "loquax.l.release"),
    Item::Knob("slevel", "loquax.l.level"),
];
const ROBOT: &[Item] = &[
    Item::Switch("robot", "loquax.l.robot", &["loquax.s.off", "loquax.s.on"]),
    Item::Pill("rnote", "loquax.l.note", super::fxa_cells::note_pill),
    Item::Knob("rgate", "loquax.l.gate"),
];
const OUTPUT: &[Item] = &[
    Item::Knob("mix", "loquax.l.mix"),
    Item::Knob("out", "loquax.l.output"),
];

/// The bands over three rows of cards.
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
    let top = (area.height() - 3.0 * (row + gap)).max(250.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    let reading = display::Reading::of(tel);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.view.update(&reading, dt);
    display::show(host, &st.view, ui, s, r1, &reading);
    if reading.seen {
        ui.ctx().request_repaint();
    }
    cells::row(
        host,
        ui,
        s,
        &mut g,
        row,
        &[
            ("loquax.card.carrier", CARRIER),
            ("loquax.card.bands", BANDS),
            ("loquax.card.follow", FOLLOW),
        ],
    );
    cells::row(
        host,
        ui,
        s,
        &mut g,
        row,
        &[
            ("loquax.card.character", CHARACTER),
            ("loquax.card.consonants", CONSONANTS),
            ("loquax.card.output", OUTPUT),
        ],
    );
    cells::row(
        host,
        ui,
        s,
        &mut g,
        row,
        &[
            ("loquax.card.oscillators", OSCILLATORS),
            ("loquax.card.unison", UNISON),
            ("loquax.card.envelope", ENVELOPE),
            ("loquax.card.robot", ROBOT),
        ],
    );
}

/// The inspector's picture: the carrier and the bands in a line ("Synthesiser · 32 bands").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let carrier = match spec.get("src").round() as i32 {
        0 => "loquax.s.sidechain",
        2 => "loquax.s.both",
        _ => "loquax.s.synth",
    };
    let carrier = if spec.get("robot") >= 0.5 {
        "loquax.l.robot"
    } else {
        carrier
    };
    let w = format!(
        "{} \u{b7} {:.0} {}",
        frame::tr(carrier),
        spec.get("bands"),
        frame::tr("loquax.d.bandsword"),
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

/// The starting preset (`cantus_core::starting`): Robot.
pub const DEMO: &[(&str, f32)] = &[("mix", 1.0), ("robot", 1.0), ("bands", 16.0)];

/// The settings `--editor loquax` and the screenshot tool open it with: the starting
/// preset.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "loquax").set_many(DEMO, "cmd.fx_preset");
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings, a sung /a/ on A2 speaking through it while a chord is
/// held (C3, E3, G3 — the notes a track would send), its telemetry as the engine sends it.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let tel = demo_telemetry(&spec, 1.2);
    app.fx.injected = Some(tel);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.loquax.effect != Some(effect) {
            app.fx.loquax = State {
                effect: Some(effect),
                ..State::default()
            };
        }
        app.fx.loquax.view.seed(&display::Reading::of(&tel));
    }
}

/// The telemetry of Loquax at `spec` after `secs` of a sung vowel with a chord held.
pub fn demo_telemetry(spec: &EffectSpec, secs: f32) -> [f32; 64] {
    use cantus_engine::{FxNote, FxUnit as _};
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::loquax::Unit::new(sr);
    u.configure(spec);
    for pitch in [48u8, 52, 55] {
        u.note(
            0,
            FxNote::On {
                pitch,
                velocity: 100,
            },
        );
    }
    let n = 480;
    let blocks = (secs * sr) as usize / n;
    // A vowel: the harmonics of 110 Hz under the formants of /a/ (700, 1 220, 2 600 Hz).
    let formant = |f: f32| {
        let bump = |c: f32, w: f32| 1.0 / (1.0 + ((f - c) / w) * ((f - c) / w));
        bump(700.0, 130.0) + 0.6 * bump(1_220.0, 140.0) + 0.3 * bump(2_600.0, 220.0)
    };
    for blk in 0..blocks {
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            let t = (blk * n + i) as f32 / sr;
            let mut x = 0.0;
            for h in 1..40 {
                let f = 110.0 * h as f32;
                x += 0.05 * formant(f) * cantus_dsp::math::sin(std::f32::consts::TAU * f * t);
            }
            *a = x;
            *b = x;
        }
        u.process(&mut l, &mut r, None);
    }
    u.telemetry()
}

/// The tilt in whole words a cell holds: "+1.5 dB" (the label says per octave).
fn tilt_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:+.1} dB");
}
