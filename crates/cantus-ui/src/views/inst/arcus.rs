//! Arcus, the bowed string: its editor (round 42, ADR 0157), on the kit's device shell
//! (ADR 0024 §8) and Arcus's own face (`kit/skin/arcus.rs`: varnished maple, the bow in
//! garnet, the rosin in amber — design sheet v1 for the owner's approval). Owned by the
//! `arcus` builder.
//!
//! - **Play**: the instrument in its room down the whole left column (r42:arcus-look: the
//!   scene engine's cello or bass, the string, finger and bow live over it; a drag moves
//!   the contact and the force), its read-out a caption along its lower edge; down the
//!   right the instrument's card (cello or double bass, arco, pizzicato or tremolo, and the
//!   four numbers a player turns: the dynamic, where the bow plays, how hard, how fast),
//!   the vibrato, the body and the output.
//! - **Edit**: every part as a card in its function hue — the picture and the instrument,
//!   the dynamic and what moves it, the bow, the vibrato, pizzicato and tremolo, the string
//!   and the body, the voice and MPE, the output. Taller than the window, it scrolls.
//! - **Matrix**: the frame's modulation matrix, as every instrument's.
//!
//! Every drag is one step of the project's undo; every switch a command.

#[path = "arcus_draw.rs"]
mod draw;
#[path = "arcus_presets.rs"]
mod presets;

#[path = "arcus_upper_presets.rs"] // r42:arcus-upper
mod upper_presets; // r42:arcus-upper

pub use draw::picture_for_tests;

/// The factory presets: the cello's and the bass's, then the violin's and the viola's. // r42:arcus-upper
const PRESET_COUNT: usize = presets::FACTORY_PRESETS.len() + upper_presets::PRESETS.len(); // r42:arcus-upper
pub const FACTORY_PRESETS: &[super::FactoryPreset] = // r42:arcus-upper
    &upper_presets::joined::<PRESET_COUNT>(presets::FACTORY_PRESETS, upper_presets::PRESETS); // r42:arcus-upper

use super::face;
use super::organ::parts::{cells, paint_card, switch, tail_rect, Card, Rows};
use crate::app::CantusApp;
use crate::kit::knob::KnobSize;
use crate::kit::text::Src;
use crate::kit::{well, with_brush, Function, State as KitState, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::arcus::tel;
use cantus_engine::inst::arcus::settings;
use egui::{Pos2, Rect, Sense, Vec2};

/// The instruments the editor offers, in `inst`'s order: the cello, the double bass, the
/// violin and the viola (`docs/round42/arcus-upper.md`). // r42:arcus-upper
const INSTRUMENTS: [Src<'static>; 4] = [
    Src::Key("arcus.inst_kind.cello"),
    Src::Key("arcus.inst_kind.bass"),
    Src::Key("arcus.inst_kind.violin"), // r42:arcus-upper
    Src::Key("arcus.inst_kind.viola"),  // r42:arcus-upper
];
const TECHNIQUES: [Src<'static>; 3] = [
    Src::Key("arcus.tech_kind.arco"),
    Src::Key("arcus.tech_kind.pizz"),
    Src::Key("arcus.tech_kind.trem"),
];
const MODES: [Src<'static>; 3] = [
    Src::Key("arcus.mode_kind.poly"),
    Src::Key("arcus.mode_kind.mono"),
    Src::Key("arcus.mode_kind.legato"),
];
const DYN_SOURCES: [Src<'static>; 6] = [
    Src::Key("arcus.dyn_src_kind.auto"),
    Src::Key("arcus.dyn_src_kind.cc1"),
    Src::Key("arcus.dyn_src_kind.cc11"),
    Src::Key("arcus.dyn_src_kind.breath"),
    Src::Key("arcus.dyn_src_kind.at"),
    Src::Key("arcus.dyn_src_kind.vel"),
];
const VIB_SOURCES: [Src<'static>; 6] = [
    Src::Key("arcus.vib_src_kind.cc21"),
    Src::Key("arcus.vib_src_kind.cc2"),
    Src::Key("arcus.vib_src_kind.at"),
    Src::Key("arcus.vib_src_kind.auto"),
    Src::Key("arcus.vib_src_kind.mw"),  // r42:arcus-p5
    Src::Key("arcus.vib_src_kind.mpe"), // r42:arcus-p5
];
const FRICTIONS: [Src<'static>; 2] = [
    Src::Key("arcus.friction_kind.hyperbolic"),
    Src::Key("arcus.friction_kind.thermal"),
];
const DIRECTIONS: [Src<'static>; 3] = [
    Src::Key("arcus.dir_kind.alt"),
    Src::Key("arcus.dir_kind.down"),
    Src::Key("arcus.dir_kind.up"),
];

/// The height of a card with a `body`, at 100 %.
fn card_h(body: f32) -> f32 {
    24.0 + 8.0 + body + 8.0
}

/// Split `rect` into rows `hs` tall (points at 100 %), 6 pt apart, from the top.
fn rows_in(rect: Rect, s: &Style, hs: &[f32]) -> Vec<Rect> {
    let mut y = rect.top();
    hs.iter()
        .map(|h| {
            let r =
                Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), s.s(*h)));
            y += s.s(*h + 6.0);
            r
        })
        .collect()
}

/// The picture's card: the instrument (Play) or the Schelleng diagram (Edit) on the display.
#[allow(clippy::too_many_arguments)]
fn picture_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    diagram: bool,
) {
    let ss = s.function(Function::Source);
    let fs = settings(spec);
    let tele = app.inst_telemetry();
    let display = (rect.height() / s.k - 24.0 - 1.0 - 6.0).max(60.0);
    let title = if diagram {
        "arcus.schelleng"
    } else {
        "instrument.arcus"
    };
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new(title, Function::Source).display(display),
    );
    if let Some(d) = cr.display {
        let sd = face::on_display(&ss);
        if diagram {
            draw::schelleng(app, ui, track, &fs, &tele, d, &sd);
        } else {
            draw::picture(app, ui, track, &fs, &tele, d, &sd);
            caption(app, ui, d, spec, &sd);
        }
        with_brush(&ui.painter_at(d), |b| face::cover(b, d, &sd));
    }
}

/// The instrument: which one, how it is played, and the four numbers a player turns.
fn instrument_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    with_readout: bool,
) {
    let sd = s.function(Function::Dynamics);
    let cr = paint_card(ui, rect, s, &Card::new("arcus.inst", Function::Dynamics));
    let r = rows_in(cr.body, s, &[24.0, 24.0, 86.0]);
    switch(app, ui, r[0], track, spec, "inst", &INSTRUMENTS, &sd);
    switch(app, ui, r[1], track, spec, "tech", &TECHNIQUES, &sd);
    cells(
        app,
        ui,
        r[2],
        track,
        spec,
        &["dyn", "bow.pos", "bow.force", "bow.speed"],
        &sd,
        KnobSize::M,
    );
    let below = Rect::from_min_max(
        Pos2::new(cr.body.left(), r[2].bottom() + s.s(10.0)),
        cr.body.max,
    );
    if with_readout && below.height() >= s.s(16.0) {
        readout(app, ui, below, spec, &sd);
    }
}

/// The note now, as a player reads it: its name, the string it is on, its dynamic's
/// marking, and the bow's three numbers (β, force, speed) — each its label's key and its
/// value.
fn readout_rows(app: &CantusApp, spec: &NativeSpec) -> [(&'static str, String); 6] {
    // The notes' names in the window's language (`arcus.notes`: C … B, Do … Si).
    let names_text = crate::i18n::t("arcus.notes");
    let names: Vec<&str> = names_text.split(',').map(|n| n.trim()).collect();
    let name = |k: usize| names.get(k % 12).copied().unwrap_or("?").to_string();
    let fs = settings(spec);
    let n = draw::now(&fs, &app.inst_telemetry());
    let key = n.key.round().max(0.0) as usize;
    let note = format!("{}{}", name(key), key as i32 / 12 - 1);
    let open = fs.instrument.spec().strings[n.string.min(3)].open as usize;
    let string = name(open);
    let marks = ["pp", "p", "mp", "mf", "f", "ff"];
    let mark = marks[((n.dynamic.clamp(0.0, 1.0) * 5.0).round() as usize).min(5)].to_string();
    // The knobs' own values (×) beside what they make: the read-out and the knob agree.
    let contact = format!("× {:.2} · β {:.2}", spec.get("bow.pos"), n.beta);
    let force = format!(
        "× {:.2} · {:.2} N ({:.2}–{:.2})",
        spec.get("bow.force"),
        n.force,
        n.lo,
        n.hi
    );
    let speed = format!(
        "× {:.2} · {:.0} cm/s",
        spec.get("bow.speed"),
        100.0 * n.speed.abs()
    );
    [
        ("arcus.read.note", note),
        ("arcus.read.string", string),
        ("arcus.read.dynamic", mark),
        ("arcus.bow.pos", contact),
        ("arcus.bow.force", force),
        ("arcus.bow.speed", speed),
    ]
}

/// The note now, as a player reads it (Edit, beside the Schelleng diagram): label and value
/// in pairs, a row each.
fn readout(app: &CantusApp, ui: &egui::Ui, rect: Rect, spec: &NativeSpec, s: &Style) {
    use crate::kit::text::{paint, Policy, Role, Text};
    let rows = readout_rows(app, spec);
    let line = s.s(18.0);
    let label_w = (rect.width() * 0.32).min(s.s(110.0));
    with_brush(&ui.painter_at(rect), |b| {
        for (i, (key, value)) in rows.iter().enumerate() {
            let y = rect.top() + i as f32 * line;
            if y + line > rect.bottom() + 0.5 {
                break;
            }
            let lr = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(label_w, line));
            let vr = Rect::from_min_max(
                Pos2::new(rect.left() + label_w + s.s(8.0), y),
                Pos2::new(rect.right(), y + line),
            );
            let mut t = Text::key(key, Role::Label, s.t.text_dim).align(egui::Align2::LEFT_CENTER);
            t.policy = Policy::Abbreviate;
            let _ = paint(b, lr, &t, s.tk());
            let v =
                Text::new(value.as_str(), Role::Value, s.t.text).align(egui::Align2::LEFT_CENTER);
            let _ = paint(b, vr, &v, s.tk());
        }
    });
}

/// r42:arcus-look — the read-out as Play shows it: a two-line caption on a plate along the
/// picture's lower edge, under the instrument's endpin (its frame leaves the band free),
/// each label dim and its value bright, the pairs laid in order across the two lines — a
/// pair never split, nothing shortened, in either language.
fn caption(app: &CantusApp, ui: &egui::Ui, d: Rect, spec: &NativeSpec, s: &Style) {
    use crate::kit::text::{measure, paint, Role, Text};
    let rows = readout_rows(app, spec);
    let ctx = ui.ctx();
    let k = s.tk();
    let (pad, gap, inner) = (s.s(8.0), s.s(16.0), s.s(5.0));
    let line = s.s(16.0);
    fn label<'a>(key: &'a str, s: &Style) -> Text<'a> {
        Text::key(key, Role::Label, s.t.text_dim)
    }
    fn value<'a>(v: &'a str, s: &Style) -> Text<'a> {
        Text::new(v, Role::Value, s.t.text)
    }
    let widths: Vec<(f32, f32)> = rows
        .iter()
        .map(|(key, v)| {
            (
                measure(ctx, &label(key, s), k).x,
                measure(ctx, &value(v, s), k).x,
            )
        })
        .collect();
    let avail = d.width() - 2.0 * pad - s.s(12.0);
    // Pairs in order across the first line while they fit, the rest on the second.
    let mut lines: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
    let mut x = 0.0;
    let mut at = 0;
    for (i, (lw, vw)) in widths.iter().enumerate() {
        let w = lw + inner + vw;
        if at == 0 && !lines[0].is_empty() && (x + gap + w > avail || i >= 3) {
            at = 1;
            x = 0.0;
        }
        x += if lines[at].is_empty() { w } else { gap + w };
        lines[at].push(i);
    }
    let width_of = |l: &[usize]| {
        l.iter()
            .map(|&i| widths[i].0 + inner + widths[i].1)
            .sum::<f32>()
            + gap * l.len().saturating_sub(1) as f32
    };
    let w = width_of(&lines[0]).max(width_of(&lines[1])) + 2.0 * s.s(10.0);
    let h = 2.0 * line + s.s(8.0);
    let plate = Rect::from_min_size(
        Pos2::new(d.center().x - w * 0.5, d.bottom() - pad - h),
        Vec2::new(w, h),
    );
    with_brush(&ui.painter_at(d), |b| {
        b.rrect(
            plate,
            s.r(5.0),
            crate::kit::Fill::Solid(crate::kit::color::with_alpha(s.t.well, 0.84)),
        );
        for (li, l) in lines.iter().enumerate() {
            let y = plate.top() + s.s(4.0) + li as f32 * line;
            let mut x = plate.center().x - width_of(l) * 0.5;
            for &i in l {
                let (lw, vw) = widths[i];
                let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(lw + 1.0, line));
                let t = label(rows[i].0, s).align(egui::Align2::LEFT_CENTER);
                let _ = paint(b, r, &t, k);
                let r =
                    Rect::from_min_size(Pos2::new(x + lw + inner, y), Vec2::new(vw + 1.0, line));
                let t = value(&rows[i].1, s).align(egui::Align2::LEFT_CENTER);
                let _ = paint(b, r, &t, k);
                x += lw + inner + vw + gap;
            }
        }
    });
}

/// A card of cells: the title, and `keys` across its body.
#[allow(clippy::too_many_arguments)]
fn cells_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    title: &str,
    f: Function,
    rows: &[&[&str]],
) {
    let sf = s.function(f);
    let cr = paint_card(ui, rect, s, &Card::new(title, f));
    let hs: Vec<f32> = rows.iter().map(|_| 86.0).collect();
    for (r, keys) in rows_in(cr.body, s, &hs).iter().zip(rows) {
        cells(app, ui, *r, track, spec, keys, &sf, KnobSize::M);
    }
}

/// A card whose title holds a switch (`key` over `choices`) and whose body holds `rows`.
#[allow(clippy::too_many_arguments)]
fn switched_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    title: &str,
    f: Function,
    key: &str,
    choices: &[Src<'static>],
    tail: f32,
    rows: &[&[&str]],
) {
    let sf = s.function(f);
    let cr = paint_card(ui, rect, s, &Card::new(title, f).tail(tail));
    switch(
        app,
        ui,
        tail_rect(&cr, s, tail),
        track,
        spec,
        key,
        choices,
        &sf,
    );
    let hs: Vec<f32> = rows.iter().map(|_| 86.0).collect();
    for (r, keys) in rows_in(cr.body, s, &hs).iter().zip(rows) {
        cells(app, ui, *r, track, spec, keys, &sf, KnobSize::M);
    }
}

/// The Play view (r42:arcus-look): the picture down the whole left column — the
/// instrument is the point, as Chorda's piano is — its read-out a caption over its lower
/// edge; down the right, the instrument's card (which one, how it is played, the four
/// numbers a player turns), the vibrato, then the body and the output side by side.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let mut cols = Rows::new(area, s);
    let c = cols.row(area.height(), &[0.58, 0.42]);
    picture_card(app, ui, track, spec, c[0], s, false);
    let h_card = s.s(card_h(86.0));
    let h_inst = (c[1].height() - 2.0 * (h_card + s.d(4.0)))
        .max(s.s(card_h(24.0 + 6.0 + 24.0 + 6.0 + 86.0)));
    let mut rows = Rows::new(c[1], s);
    let r1 = rows.row(h_inst, &[1.0]);
    instrument_card(app, ui, track, spec, r1[0], s, false);
    let r2 = rows.row(h_card, &[1.0]);
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[0],
        s,
        "arcus.vibrato",
        Function::Mod,
        &[&["vib.depth", "vib.rate", "vib.delay"]],
    );
    let r3 = rows.row(h_card, &[0.6, 0.4]);
    cells_card(
        app,
        ui,
        track,
        spec,
        r3[0],
        s,
        "arcus.body",
        Function::Tone,
        &[&["body.mix", "body.size", "body.ring"]],
    );
    cells_card(
        app,
        ui,
        track,
        spec,
        r3[1],
        s,
        "arcus.output",
        Function::Output,
        &[&["ring", "gain"]],
    );
}

/// The Edit view: every part of the instrument, each with its card. Taller than the
/// window, it scrolls there.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let gutter = s.d(4.0);
    let heights = [
        s.s(card_h(24.0 + 6.0 + 24.0 + 6.0 + 86.0)),
        s.s(card_h(86.0)),
        s.s(card_h(86.0 + 6.0 + 86.0)),
        s.s(card_h(86.0 + 6.0 + 86.0)),
        s.s(card_h(86.0)),
        s.s(card_h(86.0 + 6.0 + 86.0)),
    ];
    let total = heights.iter().sum::<f32>() + gutter * (heights.len() - 1) as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total), Sense::hover());
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(heights[0], &[0.55, 0.45]);
    picture_card(app, ui, track, spec, r1[0], s, true);
    instrument_card(app, ui, track, spec, r1[1], s, true);
    let r2 = rows.row(heights[1], &[1.0]);
    switched_card(
        app,
        ui,
        track,
        spec,
        r2[0],
        s,
        "arcus.dynamic",
        Function::Dynamics,
        "dyn.src",
        &DYN_SOURCES,
        420.0,
        &[&["dyn", "vel.dyn", "vel.attack", "mpe.press"]],
    );
    let r3 = rows.row(heights[2], &[0.6, 0.4]);
    switched_card(
        app,
        ui,
        track,
        spec,
        r3[0],
        s,
        "arcus.bow",
        Function::Dynamics,
        "bow.friction",
        &FRICTIONS,
        200.0,
        &[
            &["bow.pos", "bow.force", "bow.speed", "bow.attack"],
            &["bow.release", "bow.change", "bow.noise", "mpe.slide"],
        ],
    );
    switched_card(
        app,
        ui,
        track,
        spec,
        r3[1],
        s,
        "arcus.direction",
        Function::Dynamics,
        "bow.dir",
        &DIRECTIONS,
        190.0,
        &[&["trem.rate", "pizz.pos"], &["pizz.soft", "pizz.decay"]],
    );
    let r4 = rows.row(heights[3], &[0.6, 0.4]);
    switched_card(
        app,
        ui,
        track,
        spec,
        r4[0],
        s,
        "arcus.vibrato",
        Function::Mod,
        "vib.src",
        &VIB_SOURCES,
        280.0,
        &[
            &["vib.depth", "vib.rate", "vib.delay"],
            &["vib.attack", "vib.var", "vib.dyn"], // r42:arcus-p5: vib.dyn
        ],
    );
    cells_card(
        app,
        ui,
        track,
        spec,
        r4[1],
        s,
        "arcus.body",
        Function::Tone,
        &[&["body.mix", "body.size", "body.ring"], &["decay", "ring"]],
    );
    let r5 = rows.row(heights[4], &[1.0]);
    cells_card(
        app,
        ui,
        track,
        spec,
        r5[0],
        s,
        "arcus.output",
        Function::Output,
        &[&["lowcut", "highcut", "gain"]],
    );
    let r6 = rows.row(heights[5], &[1.0]);
    switched_card(
        app,
        ui,
        track,
        spec,
        r6[0],
        s,
        "arcus.voice",
        Function::Output,
        "mode",
        &MODES,
        260.0,
        &[
            &["poly", "finger", "porta", "bend"],
            &["octave", "transpose", "fine", "seed"],
        ],
    );
}

/// The inspector's small picture: the string and the bow.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(Function::Source);
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(48.0)), Sense::hover());
    let fs = settings(spec);
    let sd = face::on_display(&s);
    with_brush(&ui.painter_at(rect), |b| {
        well::paint(b, rect, &s, KitState::REST, None);
        draw::thumbnail(b, rect.shrink(s.s(4.0)), &fs, &sd);
    });
    resp
}

/// `--editor arcus`: the default cello, a note bowed.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    let _ = track;
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: the cello's A3 bowed mf on its D string.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0; cantus_engine::TELEMETRY_LEN];
    t[0] = 1.0;
    t[1] = 4.0;
    t[tel::ALIVE] = 1.0;
    t[tel::KEY] = 57.0;
    t[tel::F0] = 220.0;
    t[tel::STRING] = 2.0;
    t[tel::BETA] = 0.1;
    t[tel::FORCE] = 1.6;
    t[tel::SPEED] = 0.15;
    t[tel::DIR] = 1.0;
    t[tel::DYNAMIC] = 0.6;
    t[tel::LEVEL] = 0.35;
    t[tel::STICK] = 1.0;
    t[tel::FMIN] = 0.5;
    t[tel::FMAX] = 2.1;
    app.inst.injected = Some(t);
}
