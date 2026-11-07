//! Gravis — the bass instrument (808, modelled electric, synth): its editor, on the kit's
//! device shell (ADR 0024 §8). Owned by this instrument's builder (ADR 0023;
//! `research/instruments/bass.md` §5, §8.1).
//!
//! What the benchmarks do and what this does about it:
//!
//! - **One instrument, three engines**, switched in the display card's title bar (808 ·
//!   Synth · String, a segmented control, beside the voice mode Legato · Mono · Strings);
//!   the voice, the sub, the drive and the output are the same cards whatever the engine.
//! - **The display is the control**, first and widest. The 808's pitch curve is drawn over
//!   the waveform it renders — drag the curve's start for the drop in semitones, its knee
//!   for the time. The string is drawn from above to scale — drag the pluck marker and the
//!   two pickups, read them in millimetres — and beside it the current note's harmonics
//!   show the combs those positions cut, moving as you drag (no product shows the
//!   consequence of a position). The synth's filter curve carries its cutoff and resonance
//!   handle, with the unison voices and the accent meter beside it.
//! - **Cards per section** in their function hues — Source, Tone, Dynamics, Output —
//!   cells inside, switches as segmented controls and LED buttons; values at rest, in real
//!   units; every gesture is one undo step.
//! - **What needs the host is said so**: the kick duck and the song's key have their
//!   parameters here and name the hook they wait for.

#[path = "bass_808.rs"]
mod e808;
#[path = "bass_edit.rs"]
mod edit;
#[path = "bass_string.rs"]
mod strings;
#[path = "bass_synth.rs"]
mod synth;

use super::analog::parts;
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::text::{Elide, Policy, Role, Src, Text};
use crate::kit::{self, Brush, Fill, Function, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_engine::inst::bass::tel;
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};

/// A handle on one of the displays being dragged.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Grab {
    PitchAmount,
    PitchTime,
    Pluck,
    Pickup(usize),
    Filter,
}

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    pub grab: Option<Grab>,
    /// The 808's rendered picture, for the settings it was made from.
    render: Option<e808::Render>,
}

/// The engines, by the `engine` index.
const ENGINES: [&str; 3] = ["bass.engine.808", "bass.engine.synth", "bass.engine.string"];

// ---- factory presets ------------------------------------------------------------------------

// r37:library — the factory presets are data now: crates/cantus-ui/presets/ (ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

// ---- small shared pieces --------------------------------------------------------------------

/// The engine a spec plays: 0 808, 1 Synth, 2 String.
pub fn engine_of(spec: &NativeSpec) -> usize {
    (spec.get("engine").round() as usize).min(2)
}

/// A MIDI note's name, `A1`.
pub fn note_name(note: f32) -> String {
    cantus_core::pitch_name(note.round().clamp(0.0, 127.0) as u8)
}

/// A micro text in a box: the displays' rulers, readouts and captions.
fn micro(b: &mut Brush, r: Rect, text: &str, color: Color32, align: Align2, s: &Style) {
    let t = Text {
        src: Src::Str(text),
        role: Role::Micro,
        caps: false,
        align,
        policy: Policy::Elide(Elide::End),
        color,
    };
    let _ = kit::text::paint(b, r, &t, s.tk());
}

/// The hue of an engine's picture: the 808 and the string are sources, the synth's picture
/// is its filter.
fn picture_hue(engine: usize) -> Function {
    if engine == 1 {
        Function::Tone
    } else {
        Function::Source
    }
}

/// What plays now, printed at rest: `D2 · 73.42 Hz`, or that nothing does.
fn now_playing(app: &CantusApp) -> String {
    let tel = app.inst_telemetry();
    if tel[super::TEL_VOICES] > 0.0 {
        format!(
            "{} \u{b7} {:.2} Hz",
            note_name(tel[tel::PITCH]),
            tel[tel::F0]
        )
    } else {
        t("bass.idle")
    }
}

/// Where the String display's pluck marker and pickups sit, and the millimetres under an
/// x, for a display drawn in a given rect at 100 % (tests drag them).
pub use strings::{handles as string_handles, mm_at as string_mm_at};

/// The engine's picture in `d` (a card's display, or a test's rect).
fn draw_picture(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
) {
    let engine = engine_of(spec);
    let s = &s.function(picture_hue(engine));
    match engine {
        0 => e808::display(app, ui, track, spec, d, s),
        1 => synth::display(app, ui, track, spec, d, s),
        _ => strings::display(app, ui, track, spec, d, s),
    }
}

/// The engine's picture alone, `size` at the cursor at 100 % (tests put it where they
/// want it).
pub fn picture(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    size: Vec2,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let s = Style::current();
    kit::with_brush(&ui.painter_at(rect), |b| b.rect(rect, Fill::Solid(s.t.ink)));
    draw_picture(app, ui, track, spec, rect, &s);
}

/// The widths of the display card's title-bar controls: the now-playing well, the engine,
/// the voice mode (two segments, or three for the String engine's Strings mode).
fn bar_widths(engine: usize) -> [f32; 3] {
    [150.0, 186.0, if engine == 2 { 216.0 } else { 144.0 }]
}

/// The display card: the engine's picture bleeding to the edges, and in its title bar what
/// plays now, the engine and the voice mode. `disp` is the display's height at 100 %.
fn display_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    disp: f32,
) {
    let engine = engine_of(spec);
    let st = s.function(picture_hue(engine));
    let [wr, we, wm] = bar_widths(engine);
    let gap = 8.0;
    let reserve = wr + we + wm + 2.0 * gap;
    let title = ["bass.title.808", "bass.title.synth", "bass.title.string"][engine];
    let Some((cr, _)) = parts::section(
        app,
        ui,
        track,
        spec,
        &st,
        rect,
        Src::Key(title),
        Some(disp),
        reserve,
        &[],
    ) else {
        return;
    };
    let k = s.k;
    let mode_bar = parts::bar(&cr, &st, wm);
    let engine_bar = Rect::from_min_max(
        Pos2::new(mode_bar.left() - (gap + we) * k, mode_bar.top()),
        Pos2::new(mode_bar.left() - gap * k, mode_bar.bottom()),
    );
    let readout = Rect::from_min_max(
        Pos2::new(engine_bar.left() - (gap + wr) * k, mode_bar.top()),
        Pos2::new(engine_bar.left() - gap * k, mode_bar.bottom()),
    );
    let modes: &[&str] = if engine == 2 {
        &["bass.mode.legato", "bass.mode.mono", "bass.mode.strings"]
    } else {
        &["bass.mode.legato", "bass.mode.mono"]
    };
    parts::choice_at(
        app, ui, track, spec, "engine", &ENGINES, 0.0, engine_bar, &st,
    );
    parts::choice_at(app, ui, track, spec, "mode", modes, 0.0, mode_bar, &st);
    let now = now_playing(app);
    let playing = app.inst_telemetry()[super::TEL_VOICES] > 0.0;
    let resp = ui.interact(
        readout,
        ui.id().with(("gravis-now", track.0)),
        Sense::hover(),
    );
    if resp.hovered() {
        crate::kit::hint::set_text(ui.ctx(), &t("bass.now"), &now, crate::kit::hint::Caps::NONE);
    }
    kit::with_brush(&ui.painter_at(readout.expand(2.0 * k)), |b| {
        let state = if playing {
            kit::State::REST
        } else {
            kit::State::DISABLED
        };
        kit::well::readout(b, readout, &st, state, &now, true);
    });
    if let Some(d) = cr.display {
        let sd = super::face::on_display(&st);
        draw_picture(app, ui, track, spec, d, &sd);
        kit::with_brush(&ui.painter_at(d), |b| super::face::cover(b, d, &sd));
    }
}

// ---- the views --------------------------------------------------------------------------------

/// The Play view: the engine's picture first, then its Source, Tone, Dynamics and Output
/// cards with the controls that matter live.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    edit::play(app, ui, track, spec, s);
}

/// The Edit view: the picture, then every control of the engine and the shared voice,
/// sub, drive, output and host cards.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    edit::body(app, ui, track, spec, s);
}

// ---- the inspector's picture, the demo --------------------------------------------------------

/// The inspector's small read-only picture: the engine's shape in its hue.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let engine = engine_of(spec);
    let s = Style::current().function(picture_hue(engine));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(192.0, 48.0), Sense::hover());
    let mid = rect.center().y;
    let mut pts = [Pos2::ZERO; 96];
    for (i, p) in pts.iter_mut().enumerate() {
        let x = i as f32 / 95.0;
        let y = match engine {
            // A falling-pitch sine under a decay.
            0 => {
                (std::f32::consts::TAU * (2.5 * x + 1.5 * (1.0 - (-6.0 * x).exp()))).sin()
                    * (-1.5 * x).exp()
            }
            // Two cycles of a saw.
            1 => (1.0 - 2.0 * (x * 2.0).fract()) * 0.8,
            // A plucked string's shape.
            _ => {
                if x < 0.3 {
                    x / 0.3
                } else {
                    (1.0 - x) / 0.7
                }
            }
        };
        *p = Pos2::new(
            rect.left() + 4.0 + x * (rect.width() - 8.0),
            mid - y * rect.height() * 0.34,
        );
    }
    let label = format!("{} \u{b7} {}", t(spec.kind.name_key()), t(ENGINES[engine]));
    kit::with_brush(&ui.painter_at(rect), |b| {
        b.rrect(rect, s.r(4.0), Fill::Solid(s.t.ink));
        b.polyline(&pts, 1.5, s.hue);
        let r = rect.shrink2(Vec2::new(4.0, 2.0));
        micro(b, r, &label, s.t.text, Align2::LEFT_TOP, &s);
    });
    resp
}

/// `--editor bass`: an instrument that shows its display off. `GRAVIS_ENGINE=808|synth|
/// string` picks the engine for the screenshots, `GRAVIS_VIEW=edit` the Edit view.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    let engine = match std::env::var("GRAVIS_ENGINE").as_deref() {
        Ok("808") => 0,
        Ok("synth") => 1,
        _ => 2,
    };
    let preset = match engine {
        0 => "bass.preset.trap_long",
        1 => "bass.preset.acid",
        _ => "bass.preset.j_bridge",
    };
    if let Some(p) = super::factory_presets(cantus_core::InstrumentKind::Bass)
        .iter()
        .find(|p| p.label_key == preset)
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Bass, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    if engine == 2 {
        app.set_inst_value(track, "str.strings", 5.0, false);
    }
    app.inst.view = match std::env::var("GRAVIS_VIEW").as_deref() {
        Ok("edit") => super::View::Edit,
        _ => super::View::Play,
    };
    demo_feed(app);
}

/// For screenshots and tests with no engine: what a playing engine would report.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(track) = app.inst.open.or(app.selected_track) else {
        return;
    };
    let engine = app.native_of(track).map(|s| engine_of(&s)).unwrap_or(0);
    let mut t = [0.0f32; cantus_engine::TELEMETRY_LEN];
    t[super::TEL_VOICES] = 1.0;
    t[super::TEL_POLYPHONY] = 1.0;
    t[tel::ENGINE] = engine as f32;
    // The String demo is a five-string: D2 at the 5th fret of the A string (index 2).
    let (note, string, fret) = match engine {
        2 => (38.0, 2.0, 5.0),
        _ => (33.0, -1.0, 0.0),
    };
    t[tel::PITCH] = note;
    t[tel::TARGET] = note;
    t[tel::F0] = 440.0 * 2f32.powf((note - 69.0) / 12.0);
    t[tel::ENV] = 0.8;
    t[tel::CUTOFF] = 900.0;
    t[tel::SWEEP] = 0.45;
    t[tel::STRING] = string;
    t[tel::FRET] = fret;
    t[tel::AGE_MS] = 120.0;
    t[tel::PEAK] = 0.5;
    t[tel::VELOCITY] = 0.8;
    app.inst.injected = Some(t);
}
