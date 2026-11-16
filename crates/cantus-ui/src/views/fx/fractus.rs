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

#[path = "fractus_art.rs"]
mod art;
#[path = "fractus_display.rs"]
pub(crate) mod display;
#[path = "fractus_scope.rs"]
mod scope;

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
    /// The grains' trails.
    pub view: display::View,
    /// The page of cards open ([`PAGES`]).
    pub page: usize,
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

/// The pages of cards under the display, each named, its cards in order: the sequencer's
/// own (and the throw and the output beside it), the slicing modules, the colouring ones,
/// the cloud. Each fits one row at the designed width in English and Spanish.
pub const PAGES: [(&str, &[cells::Card<'static>]); 4] = [
    (
        "fractus.page.seq",
        &[
            ("fractus.card.grid", GRID),
            ("fractus.card.play", PLAY),
            ("fractus.card.throw", THROW),
            ("fractus.card.output", OUTPUT),
        ],
    ),
    (
        "fractus.page.slice",
        &[
            ("fractus.card.repeat", REPEAT),
            ("fractus.card.tape", TAPE),
            ("fractus.card.shuffle", SHUFFLE),
        ],
    ),
    (
        "fractus.page.colour",
        &[
            ("fractus.card.pitch", PITCH),
            ("fractus.card.crush", CRUSH),
            ("fractus.card.gate", GATE),
            ("fractus.card.filter", FILTER),
        ],
    ),
    (
        "fractus.page.cloud",
        &[
            ("fractus.card.cloud", CLOUD),
            ("fractus.card.grains", GRAINS),
        ],
    ),
];

/// The display, the pages' strip, and the open page's cards.
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
    let strip_h = 28.0 * k;
    st.page = st.page.min(PAGES.len() - 1);
    // Every page laid in as few rows as fit (evenly filled, never squeezed under its words);
    // the display keeps one height whichever page is open: the tallest page's.
    let rows = PAGES
        .iter()
        .map(|(_, cards)| cells::pack(ui, area.width(), k, cards).len())
        .max()
        .unwrap_or(1);
    let phys = cells::pack(ui, area.width(), k, PAGES[st.page].1);
    let top = (area.height() - rows as f32 * (row + gap) - strip_h - gap).max(300.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    let reading = display::Reading::of(tel);
    st.view.update(&reading);
    if let Some(p) = display::show(host, &st.view, ui, s, r1, &reading, st.page) {
        st.page = p;
    }
    if reading.seen {
        ui.ctx().request_repaint();
    }
    let strip = g.full(strip_h);
    let painted = host.spec().fractus.as_deref().map_or(0u32, |p| {
        (0..cantus_core::fx::fractus::LANES)
            .filter(|l| !p.lane(*l).is_empty())
            .fold(0, |m, l| m | 1 << l)
    });
    if let Some(p) = pages(ui, s, strip, st.page, painted, reading.active) {
        st.page = p;
    }
    for cards in &phys {
        cells::row(host, ui, s, &mut g, row, cards);
    }
}

/// The pages' strip: a key per page — its lamp lit when open, its name, a dot for each
/// lane its cards set (filled when the lane is painted, lit while it sounds). Returns the
/// page clicked.
fn pages(
    ui: &mut egui::Ui,
    s: &Style,
    strip: Rect,
    open: usize,
    painted: u32,
    sounding: u32,
) -> Option<usize> {
    use crate::kit::color::{mix, with_alpha};
    use crate::kit::text::{Role, Text};
    use crate::kit::Fill;
    let k = s.k;
    let gap = 4.0 * k;
    let w = (strip.width() - gap * (PAGES.len() - 1) as f32) / PAGES.len() as f32;
    let mut asked = None;
    for (i, (name, _)) in PAGES.iter().enumerate() {
        let r = Rect::from_min_size(
            egui::pos2(strip.left() + i as f32 * (w + gap), strip.top()),
            egui::vec2(w, strip.height()),
        );
        let resp = ui.interact(r, ui.id().with(("fractus.page", i)), egui::Sense::click());
        if resp.clicked() {
            asked = Some(i);
        }
        if resp.hovered() {
            crate::kit::hint::fallback(
                ui.ctx(),
                frame::tr(name),
                frame::tr("fractus.h.page"),
                crate::kit::hint::Caps::NONE,
            );
        }
        let on = i == open;
        with_brush(&ui.painter_at(r.expand(6.0 * k)), |b| {
            let face = if on { s.t.panel_raised } else { s.t.panel };
            let face = if resp.hovered() && !on {
                mix(face, s.t.panel_raised, 0.5)
            } else {
                face
            };
            b.rrect(r, 4.0 * k, Fill::V(mix(face, s.t.text, 0.04), face));
            b.rrect_stroke(r, 4.0 * k, 0.8 * k, with_alpha(s.t.edge, 0.9));
            if on {
                b.hline(
                    r.left() + 6.0 * k,
                    r.right() - 6.0 * k,
                    r.bottom() - 1.2 * k,
                    2.0 * k,
                    s.hue,
                );
            }
            let cy = r.center().y;
            art::led(
                b,
                egui::pos2(r.left() + 12.0 * k, cy),
                7.0 * k,
                s.hue,
                if on { 1.0 } else { 0.0 },
            );
            super::canvas::put(
                b,
                egui::pos2(r.left() + 24.0 * k, cy),
                egui::Align2::LEFT_CENTER,
                &Text::key(
                    name,
                    Role::Control,
                    if on { s.t.text } else { s.t.text_dim },
                ),
                k,
                None,
            );
            // Its lanes' dots, from the right.
            let mine: Vec<usize> = (0..display::PAGE_OF.len())
                .filter(|l| display::PAGE_OF[*l] == i)
                .collect();
            for (j, lane) in mine.iter().rev().enumerate() {
                let p = egui::pos2(r.right() - 12.0 * k - j as f32 * 11.0 * k, cy);
                let bit = 1u32 << lane;
                if sounding & bit != 0 {
                    b.circle(p, 6.0 * k, Fill::Solid(with_alpha(s.hue, 0.3)));
                }
                if painted & bit != 0 {
                    b.circle(p, 3.2 * k, Fill::Solid(s.hue));
                } else {
                    b.circle_stroke(p, 3.0 * k, 1.0 * k, with_alpha(s.t.text_dim, 0.8));
                }
            }
        });
    }
    asked
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
/// reversed beat at two thirds, a half-strength gate playing three times in four, a filter
/// sweep at 40 %, a throw at 80 %, the cloud swelling in — each block as tall as its amount.
pub fn demo_pattern() -> Pattern {
    let with = |at, len, amount, prob| Block {
        amount,
        prob,
        ..Block::new(at, len)
    };
    let mut p = Pattern::default();
    p.replace(0, &[Block::new(12, 4)]);
    p.replace(1, &[with(4, 4, 0.65, 1.0)]);
    p.replace(7, &[with(8, 2, 0.5, 0.75)]);
    p.replace(6, &[with(0, 8, 0.4, 1.0)]);
    p.replace(8, &[with(6, 1, 0.8, 1.0)]);
    p.replace(9, &[with(10, 2, 0.9, 1.0)]);
    p
}

/// The cloud the screenshots open with: a dozen grains spread over the last second, a
/// fifth up and spread an octave either way on fifths.
pub const DEMO_CLOUD: &[(&str, f32)] = &[
    ("clevel", 0.45),
    ("cpos", 700.0),
    ("cspray", 550.0),
    ("csize", 110.0),
    ("cdens", 110.0),
    ("cpitch", 7.0),
    ("crand", 12.0),
    ("cscale", 5.0),
];

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
    DawHost::new(app, strip, effect, "fractus").set_many(DEMO_CLOUD, "cmd.fx_preset");
}

/// The open Fractus's state, made for the effect open (the first frame would otherwise make
/// it anew and drop what the demo put in it).
fn open_state(app: &mut CantusApp) -> Option<&mut State> {
    let (_, effect, _) = app.fx.open?;
    if app.fx.fractus.effect != Some(effect) {
        app.fx.fractus = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    Some(&mut app.fx.fractus)
}

/// The keys the demo holds when notes play the lanes: the first lane's key + 4 and + 5
/// (Pitch jump and Crush; E3 and F3 from C3).
pub const DEMO_KEYS: [u8; 2] = [4, 5];

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings and pattern playing a drum-like loop at 120 BPM (and, when
/// notes play the lanes, two keys held), its telemetry as the engine sends it — the
/// playhead inside the stutter — and the readings before it, the grains' trails.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let series = demo_series(&spec, 1.85);
    let Some(last) = series.last().copied() else {
        return;
    };
    app.fx.injected = Some(last);
    let readings: Vec<display::Reading> = series.iter().map(display::Reading::of).collect();
    if let Some(st) = open_state(app) {
        st.view.seed(&readings);
    }
}

/// Open page `page` of the cards in the open Fractus (the screenshots' and the tests'
/// states).
pub fn show_page(app: &mut CantusApp, page: usize) {
    if let Some(st) = open_state(app) {
        st.page = page.min(PAGES.len() - 1);
    }
}

/// The telemetry of Fractus at `spec` after `secs` of a loop at 120 BPM.
pub fn demo_telemetry(spec: &EffectSpec, secs: f32) -> [f32; 64] {
    demo_series(spec, secs).last().copied().unwrap_or([0.0; 64])
}

/// The telemetry of Fractus at `spec` every 20 ms over the last `display::TRAIL` readings
/// before `secs` of a loop at 120 BPM, the oldest first.
pub fn demo_series(spec: &EffectSpec, secs: f32) -> Vec<[f32; 64]> {
    use cantus_engine::{FxNote, FxUnit as _};
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::fractus::Unit::new(sr);
    u.configure(spec);
    if let Some(need) = u.memory_need() {
        let _ = u.supply(cantus_engine::EffectMemory::build(need, sr));
    }
    if spec.get("trig") >= 0.5 {
        let first = spec.get("key").round().clamp(0.0, 117.0) as u8;
        for d in DEMO_KEYS {
            u.note(
                0,
                FxNote::On {
                    pitch: first + d,
                    velocity: 100,
                },
            );
        }
    }
    let n = 480;
    let blocks = (secs * sr) as usize / n;
    let keep_from = blocks.saturating_sub(2 * display::TRAIL);
    let mut out = Vec::with_capacity(display::TRAIL);
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
        if blk >= keep_from && (blocks - 1 - blk).is_multiple_of(2) {
            out.push(u.telemetry());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::display::{Reading, View, PAGE_OF};
    use super::PAGES;

    /// A lane's name opens the page holding its module's card.
    #[test]
    fn each_lane_opens_the_page_of_its_module_s_card() {
        let card = [
            "fractus.card.repeat",
            "fractus.card.tape",
            "fractus.card.tape",
            "fractus.card.shuffle",
            "fractus.card.pitch",
            "fractus.card.crush",
            "fractus.card.filter",
            "fractus.card.gate",
            "fractus.card.throw",
            "fractus.card.cloud",
        ];
        for (lane, want) in card.iter().enumerate() {
            let page = PAGES[PAGE_OF[lane]].1;
            assert!(
                page.iter().any(|(title, _)| title == want),
                "lane {lane}: {want} not on page {}",
                PAGE_OF[lane]
            );
        }
        // Every card on exactly one page.
        let mut all: Vec<&str> = PAGES.iter().flat_map(|p| p.1.iter().map(|c| c.0)).collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n);
        assert_eq!(n, 13);
    }

    /// The trails keep a reading only when its grains moved, and forget them in silence.
    #[test]
    fn the_trails_keep_moving_grains_and_forget_silence() {
        let mut tel = [0.0f32; 64];
        tel[0] = 1.0;
        tel[cantus_dsp::fractus::t::GRAINS] = 1.0;
        tel[cantus_dsp::fractus::t::GRAIN0] = 0.5;
        tel[cantus_dsp::fractus::t::GRAIN0 + 1] = 0.25;
        let mut v = View::default();
        v.update(&Reading::of(&tel));
        v.update(&Reading::of(&tel));
        assert_eq!(v.len(), 1, "a reading seen twice is no motion");
        tel[cantus_dsp::fractus::t::GRAIN0] = 0.45;
        v.update(&Reading::of(&tel));
        assert_eq!(v.len(), 2);
        assert_eq!(v.back(0), &[(0.45, 0.25)]);
        assert_eq!(v.back(1), &[(0.5, 0.25)]);
        assert!(v.back(2).is_empty());
        tel[0] = 0.0;
        v.update(&Reading::of(&tel));
        assert_eq!(v.len(), 0);
    }
}
