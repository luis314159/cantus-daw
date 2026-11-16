//! Fractus's editor (ADR 0140, provisional): the glitch sequencer and grain cloud — the
//! lanes and the cloud over three rows of cards. The display is the unit's own picture:
//! ten lanes of painted blocks on the song's grid with the playhead, the lanes sounding
//! lit; the grains sounding where they read and at their pitch; the ten keys that play the
//! lanes. Every number from the engine's telemetry (`cantus_dsp::fractus::t`); the blocks
//! from the project (`EffectSpec::fractus`).

use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::{with_brush, Style};
use cantus_core::fx::fractus::{Block, Pattern};
use cantus_core::{Command, EffectId, EffectKind, EffectSpec, StripId};
use egui::Rect;

#[path = "fractus_display.rs"]
pub(crate) mod display;

const KIND: EffectKind = EffectKind::Glitch;

/// The locale keys `prefix.0` … `prefix.{n−1}` of a stepped parameter's choices.
macro_rules! names {
    (@ $p:literal; $($i:literal),*) => { [$(concat!($p, ".", $i)),*] };
    ($p:literal, 5) => { names!(@ $p; 0, 1, 2, 3, 4) };
    ($p:literal, 7) => { names!(@ $p; 0, 1, 2, 3, 4, 5, 6) };
    ($p:literal, 14) => { names!(@ $p; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13) };
    ($p:literal, 15) => { names!(@ $p; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14) };
}

/// Factory presets: data (`presets/effects/glitch.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
}

/// Fractus's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.fractus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "fractus"),
        &mut st,
        ui,
    );
    app.fx.fractus = st;
    out
}

/// Fractus's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
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

const GRID: &[Item] = &[
    Item::Choice("step", "fractus.l.step", &STEP_NAMES),
    Item::Knob("bars", "fractus.l.bars"),
    Item::Knob("swing", "fractus.l.swing"),
    Item::Knob("xfade", "fractus.l.joins"),
];
const PLAY: &[Item] = &[
    Item::Switch(
        "trig",
        "fractus.l.playedby",
        &["fractus.s.steps", "fractus.s.notes", "fractus.s.both"],
    ),
    Item::Switch(
        "quant",
        "fractus.l.start",
        &["fractus.s.atonce", "fractus.s.onstep"],
    ),
    Item::Pill("key", "fractus.l.key", super::fxa_cells::note_pill),
    Item::Knob("seed", "fractus.l.seed"),
];
const REPEAT: &[Item] = &[
    Item::Choice("rsize", "fractus.l.size", &NOTE_NAMES),
    Item::Knob("rramp", "fractus.l.build"),
    Item::Knob("rpitch", "fractus.l.pitch"),
    Item::Knob("rdecay", "fractus.l.decay"),
    Item::Knob("rduty", "fractus.l.length"),
];
const TAPE: &[Item] = &[
    Item::Choice("vslice", "fractus.l.reverseslice", &SLICE_NAMES),
    Item::Switch(
        "tkind",
        "fractus.l.tape",
        &["fractus.s.stop", "fractus.s.startup"],
    ),
    Item::Choice("ttime", "fractus.l.time", &TTIME_NAMES),
    Item::Knob("tcurve", "fractus.l.curve"),
];
const SHUFFLE: &[Item] = &[
    Item::Choice("sslice", "fractus.l.slice", &NOTE_NAMES),
    Item::Knob("srange", "fractus.l.range"),
    Item::Knob("srev", "fractus.l.reversed"),
];
const PITCH: &[Item] = &[
    Item::Pill("psemi", "fractus.l.jump", cells::semis_pill),
    Item::Knob("pwin", "fractus.l.window"),
    Item::Switch(
        "pstyle",
        "fractus.l.style",
        &["fractus.s.warble", "fractus.s.clean"],
    ),
];
const CRUSH: &[Item] = &[
    Item::Knob("kbits", "fractus.l.bits"),
    Item::Knob("krate", "fractus.l.rate"),
];
const FILTER: &[Item] = &[
    Item::Switch(
        "fkind",
        "fractus.l.filter",
        &["fractus.s.low", "fractus.s.high", "fractus.s.band"],
    ),
    Item::Knob("ffrom", "fractus.l.from"),
    Item::Knob("fto", "fractus.l.to"),
    Item::Knob("fres", "fractus.l.resonance"),
];
const GATE: &[Item] = &[
    Item::Choice("grate", "fractus.l.rate", &GRATE_NAMES),
    Item::Knob("gduty", "fractus.l.open"),
    Item::Knob("gdepth", "fractus.l.depth"),
];
const THROW: &[Item] = &[
    Item::Choice("wtime", "fractus.l.time", &NOTE_NAMES),
    Item::Knob("wfb", "fractus.l.feedback"),
    Item::Knob("wtone", "fractus.l.tone"),
    Item::Knob("wlvl", "fractus.l.level"),
];
const CLOUD: &[Item] = &[
    Item::Knob("clevel", "fractus.l.level"),
    Item::Knob("cpos", "fractus.l.position"),
    Item::Knob("cspray", "fractus.l.spray"),
    Item::Knob("csize", "fractus.l.size"),
    Item::Knob("cdens", "fractus.l.density"),
    Item::Switch(
        "cfreeze",
        "fractus.l.freeze",
        &["fractus.s.off", "fractus.s.on"],
    ),
];
const GRAINS: &[Item] = &[
    Item::Knob("cpitch", "fractus.l.pitch"),
    Item::Knob("crand", "fractus.l.pitchspread"),
    Item::Choice("cscale", "fractus.l.scale", &SCALE_NAMES),
    Item::Knob("cspread", "fractus.l.spread"),
    Item::Knob("crev", "fractus.l.reversed"),
    Item::Choice("cwin", "fractus.l.shape", &WINDOW_NAMES),
];
const OUTPUT: &[Item] = &[
    Item::Knob("mix", "fractus.l.mix"),
    Item::Knob("out", "fractus.l.output"),
];

/// The names a stepped knob reads its value as (the choices' locale keys).
const STEP_NAMES: [&str; 7] = names!("fx.fractus.step", 7);
const NOTE_NAMES: [&str; 14] = names!("fx.fractus.note", 14);
const SLICE_NAMES: [&str; 15] = names!("fx.fractus.slice", 15);
const TTIME_NAMES: [&str; 15] = names!("fx.fractus.ttime", 15);
const GRATE_NAMES: [&str; 15] = names!("fx.fractus.grate", 15);
const SCALE_NAMES: [&str; 7] = names!("fx.fractus.cscale", 7);
const WINDOW_NAMES: [&str; 5] = names!("fx.fractus.cwin", 5);

/// The lanes and the cloud over four rows of cards.
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
    let top = (area.height() - 4.0 * (row + gap)).max(280.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    let reading = display::Reading::of(tel);
    display::show(host, ui, s, r1, &reading);
    if reading.seen {
        ui.ctx().request_repaint();
    }
    let rows: [&[(&'static str, &[Item])]; 4] = [
        &[
            ("fractus.card.grid", GRID),
            ("fractus.card.play", PLAY),
            ("fractus.card.throw", THROW),
            ("fractus.card.output", OUTPUT),
        ],
        &[
            ("fractus.card.repeat", REPEAT),
            ("fractus.card.tape", TAPE),
            ("fractus.card.shuffle", SHUFFLE),
        ],
        &[
            ("fractus.card.pitch", PITCH),
            ("fractus.card.crush", CRUSH),
            ("fractus.card.gate", GATE),
            ("fractus.card.filter", FILTER),
        ],
        &[
            ("fractus.card.cloud", CLOUD),
            ("fractus.card.grains", GRAINS),
        ],
    ];
    for cards in rows {
        cells::row(host, ui, s, &mut g, row, cards);
    }
}

/// The inspector's picture: the lanes painted and the grid in a line.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let painted = spec
        .fractus
        .as_deref()
        .map_or(0, |p| p.lanes.values().filter(|l| !l.is_empty()).count());
    let w = format!(
        "{painted} {} \u{b7} {} {:.0}",
        frame::tr("fractus.d.lanesword"),
        frame::tr("fx.fractus.bars"),
        spec.get("bars"),
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

/// The screenshots' pattern over the starting spec: a stutter build into the bar's end, a
/// reversed beat, a half-strength gate playing three times in four, a filter sweep and a
/// throw; the cloud beside them.
pub fn demo_pattern() -> Pattern {
    let mut p = Pattern::default();
    p.replace(0, &[Block::new(12, 4)]);
    p.replace(1, &[Block::new(4, 4)]);
    p.replace(
        7,
        &[Block {
            amount: 0.5,
            prob: 0.75,
            ..Block::new(8, 2)
        }],
    );
    p.replace(6, &[Block::new(0, 8)]);
    p.replace(8, &[Block::new(6, 1)]);
    p.replace(9, &[Block::new(10, 2)]);
    p
}

/// The settings `--editor fractus` and the screenshot tool open it with: the demo
/// pattern, the cloud beside it.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let pattern = demo_pattern();
    for (lane, blocks) in &pattern.lanes {
        app.exec(Command::SetFractusLane {
            strip,
            effect,
            lane: *lane,
            blocks: blocks.clone(),
        });
    }
    DawHost::new(app, strip, effect, "fractus").set_many(
        &[
            ("clevel", 0.35),
            ("cpitch", 7.0),
            ("crand", 12.0),
            ("cscale", 5.0),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings and pattern playing a drum-like loop at 120 BPM for
/// `secs`, its telemetry as the engine sends it (the playhead inside the stutter).
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    app.fx.injected = Some(demo_telemetry(&spec, 1.85));
}

/// The telemetry of Fractus at `spec` after `secs` of a loop at 120 BPM.
pub fn demo_telemetry(spec: &EffectSpec, secs: f32) -> [f32; 64] {
    use cantus_engine::FxUnit as _;
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::fractus::Unit::new(sr);
    u.configure(spec);
    if let Some(need) = u.memory_need() {
        let _ = u.supply(cantus_engine::EffectMemory::build(need, sr));
    }
    let n = 480;
    let blocks = (secs * sr) as usize / n;
    for blk in 0..blocks {
        let at = blk * n;
        u.set_position(&cantus_engine::SongPosition {
            playing: true,
            tempo_bpm: 120.0,
            tempo_now: 120.0,
            samples: at as i64,
            beats: at as f64 / 24_000.0,
            time_sig_num: 4,
            time_sig_den: 4,
            ..Default::default()
        });
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            let t = (at + i) as f32 / sr;
            let beat = (t * 2.0).fract();
            let kick = cantus_dsp::math::sin(std::f32::consts::TAU * 60.0 * beat * 0.5)
                * cantus_dsp::math::exp(-beat * 18.0);
            let hat = cantus_dsp::math::sin(std::f32::consts::TAU * 7_100.0 * t)
                * cantus_dsp::math::exp(-((t * 4.0).fract()) * 60.0);
            *a = 0.6 * kick + 0.1 * hat;
            *b = *a;
        }
        u.process(&mut l, &mut r, None);
    }
    u.telemetry()
}
