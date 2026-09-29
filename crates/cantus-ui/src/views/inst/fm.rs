//! Modulus — the FM synth: its editor, on the kit's device shell (ADR 0024 §8; research
//! `fm-additive-granular.md` §8.5). Owned by this instrument's builder (ADR 0023).
//!
//! What the benchmark lacks, and what this does about it:
//!
//! - **The topology is the control.** The operator graph leads both views: wire it by
//!   dragging from one operator to another (or onto itself for feedback, or onto OUT); an
//!   edge carries its index as a value pill you drag. The FM8-style matrix is the same
//!   state as a grid of cells, in its own card or in the graph's place. Both show a
//!   modulation ring where a source reaches a cell.
//! - **The 32 DX7 algorithms are templates**, picked from a menu field that draws each one
//!   as its own little graph and says how many cells it will overwrite; afterwards every
//!   cell is free. **DX7 SysEx** (a voice or a cartridge) comes in from the DX7 menu field
//!   as one undo step.
//! - **An operator table made of kit parts**: every operator's wave, ratio or fixed
//!   frequency (a segmented switch), detune, level (a knob), velocity, output and feedback
//!   in value pills you drag, and its envelope drawn.
//! - **Aliasing is shown, not hidden**: the spectrum is the patch's own sound, with the
//!   Carson edge of the voice, the host's Nyquist and the rate the guard chose.
//! - **Envelopes are drawn and dragged**, DX rate/level or time/level breakpoints, with the
//!   key-scaling curve beside them; every value printed in its unit.
//! - **The easy macros are offsets** (FM8's Easy page): brightness, harmonic, detune and the
//!   envelope times never overwrite the patch until you press Bake.
//!
//! Cards take their function's hue (ADR 0024 §2): operators in source, the envelopes in
//! dynamics, the spectrum in tone, the morph pad in space, the easy macros and LFOs in
//! modulation, the voice and quality in output. Every gesture is one command, so one step
//! of the project's undo.

#[path = "fm_env.rs"]
mod env;
#[path = "fm_graph.rs"]
mod graph;
#[path = "fm_matrix.rs"]
mod matrix;
#[path = "fm_morph.rs"]
mod morph;
#[path = "fm_ops.rs"]
mod ops;
#[path = "fm_presets.rs"]
mod presets;
#[path = "fm_sound.rs"]
mod sound;
#[path = "fm_sysex.rs"]
pub mod sysex;

pub use presets::FACTORY_PRESETS;

use super::wavetable::parts::{self, Rows};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::knob::KnobSize;
use crate::kit::{Function, Style};
use cantus_core::inst::fm::{dx_template, matrix_key, DX_ALGORITHMS, OPS, OP_KEYS, OUT_OFF};
use cantus_core::{Command, MergeKey, NativeSpec, TrackId};
use cantus_dsp::modulus::tel;

/// Which page of the selected operator the Edit view shows.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Detail {
    #[default]
    Envelope,
    Scaling,
    Oscillator,
}

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    /// The operator the detail pages show (0 = A).
    pub selected: usize,
    /// The graph's card shows the matrix grid rather than the graph.
    pub matrix_view: bool,
    pub detail: Detail,
    graph: graph::Drag,
    spectrum: sound::Cache,
    /// A cartridge read from disk, waiting for a voice to be picked.
    pub bank: Option<Vec<sysex::DxVoice>>,
}

// ---- commands: each is one undo step -------------------------------------------------------

/// Write `changes` as one batch: only what differs, and a value back at its default is
/// cleared rather than stored (the spec stays sparse).
pub fn batch(
    app: &mut CantusApp,
    track: TrackId,
    spec: &NativeSpec,
    label_key: &str,
    changes: &[(&str, f32)],
) {
    let mut commands = Vec::new();
    for &(key, v) in changes {
        let Some(info) = spec.kind.param(key) else {
            continue;
        };
        let v = v.clamp(info.min, info.max);
        if spec.get(key) == v && (spec.is_stored(key) || v == info.default) {
            continue;
        }
        commands.push(if v == info.default && spec.is_stored(key) {
            Command::ClearInstrumentValue {
                track,
                key: key.to_string(),
            }
        } else {
            Command::SetInstrumentValue {
                track,
                key: key.to_string(),
                value: v,
            }
        });
    }
    if !commands.is_empty() {
        app.exec(Command::Batch {
            label_key: label_key.into(),
            commands,
        });
    }
}

/// A drag that moves several numbers at once (a breakpoint's time and level): one batch of
/// the same shape every frame, which the history merges into one undo step.
pub fn drag_values(app: &mut CantusApp, track: TrackId, values: &[(&'static str, f32)]) {
    let commands = values
        .iter()
        .map(|(k, v)| Command::SetInstrumentValue {
            track,
            key: (*k).to_string(),
            value: *v,
        })
        .collect();
    let keys: Vec<&str> = values.iter().map(|(k, _)| *k).collect();
    app.exec_merging(
        Command::Batch {
            label_key: "cmd.set_instrument_value".into(),
            commands,
        },
        MergeKey::of(&("fm-drag", track.0, keys)),
    );
}

/// Connect operator `s` into `t` (itself: feedback) at a musical starting index.
pub fn connect(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, s: usize, t: usize) {
    let key = matrix_key(s, t);
    if spec.get(key) == 0.0 {
        let v = if s == t { 1.0 } else { 1.5 };
        batch(app, track, spec, "cmd.fm_connect", &[(key, v)]);
    }
}

pub fn disconnect(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, s: usize, t: usize) {
    batch(
        app,
        track,
        spec,
        "cmd.fm_disconnect",
        &[(matrix_key(s, t), 0.0)],
    );
}

/// Send an operator to the output (0 dB), or take it away.
pub fn set_carrier(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, o: usize, on: bool) {
    let v = if on { 0.0 } else { OUT_OFF };
    batch(app, track, spec, "cmd.fm_output", &[(OP_KEYS[o].out, v)]);
}

/// A DX algorithm's topology over the patch: edges keep the index they already have (or
/// get 2 rad), the feedback keeps the patch's strongest (or 1 rad).
pub fn algorithm_values(spec: &NativeSpec, alg: usize) -> Vec<(&'static str, f32)> {
    let fb = (0..OPS)
        .map(|o| spec.get(OP_KEYS[o].fb))
        .fold(0f32, f32::max);
    let fb = if fb > 0.0 { fb } else { 1.0 };
    dx_template(alg, 2.0, fb)
        .into_iter()
        .map(|(k, v)| {
            let now = spec.get(k);
            let keep = v != 0.0 && now != 0.0 && !k.starts_with("out") && !k.starts_with("fb");
            (k, if keep { now } else { v })
        })
        .collect()
}

/// How many cells an algorithm would change.
pub fn algorithm_changes(spec: &NativeSpec, alg: usize) -> usize {
    algorithm_values(spec, alg)
        .iter()
        .filter(|(k, v)| spec.get(k) != *v)
        .count()
}

pub fn apply_algorithm(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, alg: usize) {
    let values = algorithm_values(spec, alg);
    batch(app, track, spec, "cmd.fm_algorithm", &values);
}

/// The DX algorithm the patch's topology still is, if any.
pub fn algorithm_of(spec: &NativeSpec) -> Option<usize> {
    let mut edges = Vec::new();
    let mut carriers = Vec::new();
    let mut fb = None;
    for (s, k) in OP_KEYS.iter().enumerate() {
        for t in 0..OPS {
            if spec.get(matrix_key(s, t)) != 0.0 {
                let e = (s as u8 + 1, t as u8 + 1);
                if s >= 6 || t >= 6 {
                    return None;
                }
                if s <= t {
                    fb = Some(e); // feedback, or a loop back into a stack
                } else {
                    edges.push(e);
                }
            }
        }
        if spec.get(k.out) > OUT_OFF {
            if s >= 6 {
                return None;
            }
            carriers.push(s as u8 + 1);
        }
    }
    cantus_core::inst::fm::dx_algorithm_of(&edges, &carriers, fb)
}

/// Write the easy macros into the patch and set them back to zero: one undo step.
pub fn bake(app: &mut CantusApp, track: TrackId, spec: &NativeSpec) {
    use cantus_dsp::modulus::{baked_dx, baked_time};
    let p = cantus_engine::inst::fm::patch_of(spec);
    let mut changes: Vec<(&'static str, f32)> = Vec::new();
    let bright = (2.0 * p.bright).exp2();
    let harm = p.harm;
    let det = p.detune_macro;
    let pattern = [0.0, 1.0, -1.0, 0.5, -0.5, 0.75, -0.75, 0.25];
    let carrier = |o: usize| spec.get(OP_KEYS[o].out) > OUT_OFF;
    for o in 0..OPS {
        let k = &OP_KEYS[o];
        let s = &p.ops[o];
        for t in 0..OPS {
            let key = matrix_key(o, t);
            if bright != 1.0 && spec.get(key) != 0.0 {
                changes.push((key, spec.get(key) * bright));
            }
        }
        if harm != 0.0 && !s.fixed && !carrier(o) {
            changes.push((k.ratio, s.ratio.powf(1.0 + 0.25 * harm)));
        }
        if det != 0.0 {
            changes.push((k.detune, s.detune + det * 10.0 * pattern[o]));
            changes.push((k.offset, s.offset * (1.0 + det)));
        }
        if p.env_macro != [0.0; 4] {
            let e = baked_time(&s.time, p.env_macro);
            for i in 0..e.n {
                changes.push((k.env_t[i], e.t[i] * 1000.0));
                changes.push((k.env_l[i], e.l[i]));
            }
            let d = baked_dx(&s.dxenv, p.env_macro);
            for i in 0..4 {
                changes.push((k.dx_r[i], d.rates[i] as f32));
                changes.push((k.dx_l[i], d.levels[i] as f32));
            }
        }
    }
    for key in [
        "macro.bright",
        "macro.harm",
        "macro.detune",
        "macro.a",
        "macro.d",
        "macro.s",
        "macro.r",
    ] {
        changes.push((key, 0.0));
    }
    batch(app, track, spec, "cmd.fm_bake", &changes);
}

/// Replace the patch with a DX voice: one undo step.
pub fn import_voice(app: &mut CantusApp, track: TrackId, voice: &sysex::DxVoice) {
    app.apply_inst_spec(track, &sysex::to_spec(voice), "cmd.fm_import");
}

/// A `.syx` file's bytes: a voice is applied at once, a cartridge waits for a pick.
pub fn import_bytes(app: &mut CantusApp, track: TrackId, bytes: &[u8]) {
    match sysex::parse(bytes) {
        Ok(read) => {
            if !read.checksum_ok || read.clamped > 0 {
                app.set_status(tf(
                    "fm.import.fixed",
                    &[
                        ("n", &read.clamped.to_string()),
                        (
                            "sum",
                            &t(if read.checksum_ok {
                                "fm.import.sum_ok"
                            } else {
                                "fm.import.sum_bad"
                            }),
                        ),
                    ],
                ));
            }
            match read.parsed {
                sysex::Parsed::Voice(v) => import_voice(app, track, &v),
                sysex::Parsed::Bank(voices) => app.inst.fm.bank = Some(voices),
            }
        }
        Err(e) => app.set_error(tf("fm.import.error", &[("why", &format!("{e:?}"))])),
    }
}

fn import_dialog(app: &mut CantusApp, track: TrackId) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("DX7 SysEx", &["syx", "SYX"])
        .pick_file()
    else {
        return;
    };
    match std::fs::read(&path) {
        Ok(bytes) => import_bytes(app, track, &bytes),
        Err(e) => app.set_error(e.to_string()),
    }
}
// ---- the views ---------------------------------------------------------------------------

/// The easy macros: offsets on the whole patch until Bake writes them in.
pub const EASY: [&str; 7] = [
    "macro.bright",
    "macro.harm",
    "macro.detune",
    "macro.a",
    "macro.d",
    "macro.s",
    "macro.r",
];

/// The Edit view: every section in rows of cards — the graph and the operator table, the
/// selected operator's pages and the spectrum, the matrix grid, the morph pad and the easy
/// macros, the voice and the LFOs, the envelopes of pitch and modulation and the quality.
/// Taller than the window: it scrolls.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let k = s.k;
    let heights = [252.0, 244.0, 214.0, 118.0, 118.0];
    let total: f32 = heights.iter().sum::<f32>() * k + 4.0 * k * (heights.len() - 1) as f32;
    let w = ui.available_width();
    let (area, _) = ui.allocate_exact_size(egui::vec2(w, total), egui::Sense::hover());
    let mut rows = Rows::new(area, s);
    let src = s.function(Function::Source);

    let r = rows.take(heights[0] * k);
    let cols = parts::columns(r, s, &[0.36, 0.64]);
    graph::card(app, ui, track, spec, &src, cols[0]);
    ops::table_card(app, ui, track, spec, &src, cols[1]);

    let r = rows.take(heights[1] * k);
    let cols = parts::columns(r, s, &[0.62, 0.38]);
    env::detail_card(
        app,
        ui,
        track,
        spec,
        &s.function(Function::Dynamics),
        cols[0],
    );
    sound::card(app, ui, track, spec, &s.function(Function::Tone), cols[1]);

    let r = rows.take(heights[2] * k);
    let cols = parts::columns(r, s, &[0.42, 0.20, 0.38]);
    matrix::card(app, ui, track, spec, &src, cols[0]);
    morph::card(app, ui, track, spec, &s.function(Function::Space), cols[1]);
    ops::easy_card(
        app,
        ui,
        track,
        spec,
        &s.function(Function::Mod),
        cols[2],
        KnobSize::S,
    );

    let r = rows.take(heights[3] * k);
    let cols = parts::columns(r, s, &[0.50, 0.28, 0.22]);
    let out = s.function(Function::Output);
    let md = s.function(Function::Mod);
    ops::voice_card(app, ui, track, spec, &out, cols[0]);
    ops::lfo_card(app, ui, track, spec, &md, cols[1], 0);
    ops::lfo_card(app, ui, track, spec, &md, cols[2], 1);

    let r = rows.take(heights[4] * k);
    let cols = parts::columns(r, s, &[0.40, 0.25, 0.35]);
    let dy = s.function(Function::Dynamics);
    ops::penv_card(app, ui, track, spec, &dy, cols[0]);
    ops::menv_card(app, ui, track, spec, &dy, cols[1]);
    ops::quality_card(app, ui, track, spec, &out, cols[2]);
}

/// The Play view: the graph (wire it by dragging), the patch's spectrum and the morph pad
/// above the fold; the easy macros and the voice below.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let mut rows = Rows::new(area, s);
    let low = 152.0 * s.k;
    let top = rows.take(area.height() - low - s.d(4.0));
    let cols = parts::columns(top, s, &[0.44, 0.38, 0.18]);
    let src = s.function(Function::Source);
    graph::card(app, ui, track, spec, &src, cols[0]);
    sound::card(app, ui, track, spec, &s.function(Function::Tone), cols[1]);
    morph::card(app, ui, track, spec, &s.function(Function::Space), cols[2]);
    let bottom = rows.rest();
    let cols = parts::columns(bottom, s, &[0.64, 0.36]);
    ops::easy_card(
        app,
        ui,
        track,
        spec,
        &s.function(Function::Mod),
        cols[0],
        KnobSize::L,
    );
    ops::play_voice_card(app, ui, track, spec, &s.function(Function::Output), cols[1]);
}

/// The inspector's picture: the graph, small.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    graph::picture(ui, spec, egui::vec2(224.0, 88.0))
}

/// `--editor fm`: the electric piano preset, operator B selected, a note sounding.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS.first() {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Fm, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    app.inst.fm.selected = 1;
    demo_feed(app);
}

/// What a playing engine would report: three voices, the newest on C4.
pub fn demo_feed(app: &mut CantusApp) {
    let mut tel_ = [0.0; cantus_engine::inst::TELEMETRY_LEN];
    tel_[super::TEL_VOICES] = 3.0;
    tel_[super::TEL_POLYPHONY] = 16.0;
    tel_[tel::KEY] = 60.0;
    tel_[tel::VELOCITY] = 0.8;
    tel_[tel::ELAPSED] = 0.35;
    tel_[tel::RATE] = 48_000.0;
    tel_[tel::FACTOR] = 2.0;
    tel_[tel::TAME] = 1.0;
    tel_[tel::BANDWIDTH] = 9_400.0;
    for (o, a) in [0.82f32, 0.35, 0.74, 0.5, 0.0, 0.0, 0.0, 0.0]
        .iter()
        .enumerate()
    {
        tel_[tel::AMP + o] = *a;
        tel_[tel::STAGE + o] = 1.0;
    }
    app.inst.injected = Some(tel_);
}

/// The number of DX algorithms, for the picker.
pub const ALGORITHMS: usize = DX_ALGORITHMS.len();
