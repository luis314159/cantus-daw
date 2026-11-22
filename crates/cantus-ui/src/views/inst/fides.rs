//! Fides, the resonator instrument: its editor (round 41, ADR 0150), on the kit's device
//! shell (ADR 0024 §8) and Fides's own face (`kit/skin/fides.rs`: rosewood, copper, a plan's
//! paper). Owned by the `fides` builder.
//!
//! - **Play**: the instrument as the settings make it, drawn on the display — the
//!   resonator, the exciter where it plays it (drag to move it), the microphone, and the
//!   partials it rings with lit by what sounds ([`draw`]) — beside the exciter's card (its
//!   six kinds and the knobs that one has); under them the resonator's card (its twelve
//!   kinds), the body's and the output's.
//! - **Edit**: every part as a card in its function hue — the picture and the exciter, the
//!   two resonators and their coupling, the bow and MPE, the noise, the body, vibrato and
//!   tremolo, the voice, the output. Taller than the window, it scrolls.
//! - **Matrix**: the frame's modulation matrix, as every instrument's.
//!
//! Every drag is one step of the project's undo; every switch a command.

#[path = "fides_draw.rs"]
mod draw;
#[path = "fides_presets.rs"]
mod presets;

pub use draw::ladder;
pub use presets::FACTORY_PRESETS;

use super::face;
use super::organ::parts::{cells, lamp, paint_card, switch, tail_rect, Card, Rows};
use crate::app::CantusApp;
use crate::kit::knob::KnobSize;
use crate::kit::text::Src;
use crate::kit::{well, with_brush, Function, State as KitState, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::fides::exciter::ExciterKind;
use cantus_dsp::fides::tel;
use cantus_engine::inst::fides::settings;
use egui::{Pos2, Rect, Sense, Vec2};

const EXCITERS: [Src<'static>; 6] = [
    Src::Key("fides.exciter.pluck"),
    Src::Key("fides.exciter.pick"),
    Src::Key("fides.exciter.bow"),
    Src::Key("fides.exciter.mallet"),
    Src::Key("fides.exciter.noise"),
    Src::Key("fides.exciter.input"),
];
const RESONATORS: [Src<'static>; 12] = [
    Src::Key("fides.resonator.string"),
    Src::Key("fides.resonator.bar"),
    Src::Key("fides.resonator.marimba"),
    Src::Key("fides.resonator.tine"),
    Src::Key("fides.resonator.plate"),
    Src::Key("fides.resonator.membrane"),
    Src::Key("fides.resonator.drumhead"),
    Src::Key("fides.resonator.tube"),
    Src::Key("fides.resonator.pipe"),
    Src::Key("fides.resonator.bell"),
    Src::Key("fides.resonator.glass"),
    Src::Key("fides.resonator.pan"),
];
const BODIES: [Src<'static>; 7] = [
    Src::Key("fides.body_kind.none"),
    Src::Key("fides.body_kind.guitar"),
    Src::Key("fides.body_kind.violin"),
    Src::Key("fides.body_kind.cello"),
    Src::Key("fides.body_kind.harp"),
    Src::Key("fides.body_kind.box"),
    Src::Key("fides.body_kind.shell"),
];
const COUPLINGS: [Src<'static>; 2] = [
    Src::Key("fides.couple_kind.parallel"),
    Src::Key("fides.couple_kind.serial"),
];
const MODES: [Src<'static>; 3] = [
    Src::Key("fides.mode_kind.poly"),
    Src::Key("fides.mode_kind.mono"),
    Src::Key("fides.mode_kind.legato"),
];

/// The height of a card with a `display` and a `body`, at 100 %.
fn card_h(display: f32, body: f32) -> f32 {
    24.0 + if display > 0.0 { 1.0 + display } else { 0.0 } + 8.0 + body + 8.0
}

/// The knobs the exciter has, by kind.
pub fn exciter_keys(kind: ExciterKind) -> &'static [&'static str] {
    match kind {
        ExciterKind::Pluck | ExciterKind::Pick | ExciterKind::Mallet => &[
            "exc.contact",
            "exc.vel_tone",
            "exc.noise",
            "exc.tone",
            "exc.level",
        ],
        ExciterKind::Bow => &[
            "bow.pressure",
            "bow.speed",
            "bow.attack",
            "bow.release",
            "exc.level",
        ],
        ExciterKind::Noise => &[
            "noise.color",
            "noise.width",
            "noise.density",
            "noise.attack",
            "noise.release",
        ],
        ExciterKind::Input => &["exc.level", "bow.attack", "bow.release", "exc.tone"],
    }
}

/// A resonator's knobs (`a` or `b`).
fn res_keys(b: bool, all: bool) -> &'static [&'static str] {
    match (b, all) {
        (false, false) => &["a.decay", "a.material", "a.inharm", "a.listen"],
        (true, false) => &["b.decay", "b.material", "b.tune", "b.level"],
        (false, true) => &[
            "a.decay",
            "a.material",
            "a.decay_key",
            "a.inharm",
            "a.listen",
            "a.modes",
            "a.tune",
            "a.fine",
            "a.keytrack",
            "a.level",
        ],
        (true, true) => &[
            "b.decay",
            "b.material",
            "b.decay_key",
            "b.inharm",
            "b.listen",
            "b.modes",
            "b.tune",
            "b.fine",
            "b.keytrack",
            "b.level",
        ],
    }
}

/// A stepped parameter's choices as two rows (the twelve resonators six and six, the
/// seven bodies four and three — every name with its room): a click sets `key`.
#[allow(clippy::too_many_arguments)]
fn two_rows(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    all: &[Src<'static>],
    s: &Style,
) {
    let now = spec.get(key).round().max(0.0) as usize;
    let gap = s.s(3.0);
    let h = (rect.height() - gap) * 0.5;
    let first = all.len().div_ceil(2);
    for row in 0..2 {
        let r = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + row as f32 * (h + gap)),
            Vec2::new(rect.width(), h),
        );
        let from = row * first;
        let labels = &all[from..(from + first).min(all.len())];
        // None lit in the other row: a value past its choices (never `usize::MAX`: the
        // painter adds one to it).
        let selected = now
            .checked_sub(from)
            .filter(|i| *i < labels.len())
            .unwrap_or(labels.len());
        let name = spec
            .kind
            .param(key)
            .map(|i| crate::i18n::t(i.label_key))
            .unwrap_or_default();
        let (resp, chosen) = crate::kit::widgets::Segmented::new(selected, labels)
            .id(egui::Id::new(("fides-rows", track.0, key, row)))
            .name(&name)
            .caps(crate::kit::hint::Caps::PARAM)
            .style(*s)
            .show_at(ui, r);
        // A parameter as any switch is: its menu, its mapped mark, the learn mode.
        app.controllers
            .inst_words
            .note(spec.kind, key, || crate::controllers_words::srcs(all));
        let learning = crate::controllers::stepped_inst(app, ui, track, spec, key, &resp);
        if let Some(i) = chosen.filter(|_| !learning) {
            app.set_inst_value(track, key, (from + i) as f32, false);
        }
    }
}

/// The picture's card: the instrument drawn on the display.
fn picture_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let ss = s.function(Function::Source);
    let fs = settings(spec);
    let tele = app.inst_telemetry();
    let display = (rect.height() / s.k - 24.0 - 1.0 - 6.0).max(60.0);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("instrument.fides", Function::Source).display(display),
    );
    if let Some(d) = cr.display {
        let sd = face::on_display(&ss);
        draw::picture(app, ui, track, &fs, &tele, d, &sd);
        with_brush(&ui.painter_at(d), |b| face::cover(b, d, &sd));
    }
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

/// The exciter: its six kinds, the knobs this one has, and in `full` where it plays and
/// what MPE moves.
fn exciter_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let sd = s.function(Function::Dynamics);
    let fs = settings(spec);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("fides.exc.type", Function::Dynamics),
    );
    let r = rows_in(cr.body, s, &[24.0, 86.0, 86.0]);
    switch(app, ui, r[0], track, spec, "exc.type", &EXCITERS, &sd);
    cells(
        app,
        ui,
        r[1],
        track,
        spec,
        exciter_keys(fs.exc.kind),
        &sd,
        KnobSize::M,
    );
    let second: &[&str] = match fs.exc.kind {
        ExciterKind::Bow => &["exc.pos", "mpe.press", "mpe.slide", "noise.level"],
        // r41:fides-input — what it hears beside where it plays it (ADR 0154).
        ExciterKind::Input => &["exc.pos", "velocity"],
        _ => &["exc.pos", "velocity", "noise.level", "mpe.slide"],
    };
    if r[2].bottom() <= cr.body.bottom() + 0.5 {
        if fs.exc.kind == ExciterKind::Input {
            let parts = super::organ::parts::split(r[2], s.s(6.0), &[0.55, 0.45]);
            input_source(app, ui, parts[0], track, &sd);
            cells(app, ui, parts[1], track, spec, second, &sd, KnobSize::M);
        } else {
            cells(app, ui, r[2], track, spec, second, &sd, KnobSize::M);
        }
    }
}

/// r41:fides-input — the Input exciter's source (ADR 0154): the strip whose audio excites
/// it, chosen as an insert's key is (any strip of the desk after its fader, but one that
/// would close a loop), or none — and then it says so: the exciter is silent until one is.
fn input_source(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect, track: TrackId, s: &Style) {
    use crate::views::fx::{controls, frame};
    let current = app.project().track(track).and_then(|t| t.sidechain);
    let w = rect.width() / s.k;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    controls::caption(&mut child, s, Src::Key("fides.input.from"), w);
    let shown = match current {
        Some(k) => app.capture_label(cantus_engine::CaptureTarget::Strip(k)),
        None => crate::i18n::t("fides.input.none"),
    };
    let id = egui::Id::new(("fides-input", track.0));
    if let Some(at) = controls::field(&mut child, s, id, &shown, frame::tr("fides.input.tip"), w) {
        let project = app.project();
        let options: Vec<(cantus_core::StripId, String)> = project
            .strips
            .iter()
            .map(|x| x.id)
            .filter(|k| project.can_key_track(track, *k))
            .map(|k| (k, app.capture_label(cantus_engine::CaptureTarget::Strip(k))))
            .collect();
        let mut items = vec![frame::radio(Src::Key("fides.input.none"), current.is_none())];
        items.extend(
            options
                .iter()
                .map(|(k, text)| frame::radio(Src::Str(text), current == Some(*k))),
        );
        if let Some(i) = controls::list(&mut child, s, id, at, w.max(180.0), &items) {
            let source = if i == 0 {
                None
            } else {
                options.get(i - 1).map(|o| o.0)
            };
            drop(items);
            if source != current {
                app.exec(cantus_core::Command::SetTrackSidechain { track, source });
            }
        }
    }
    if current.is_none() {
        let line = Rect::from_min_size(
            Pos2::new(rect.left(), child.cursor().min.y + s.s(4.0)),
            Vec2::new(rect.width(), s.d(14.0)),
        );
        if line.bottom() <= rect.bottom() + 0.5 {
            with_brush(&ui.painter_at(rect), |b| {
                let t = crate::kit::text::Text {
                    src: Src::Key("fides.input.silent"),
                    role: crate::kit::text::Role::Micro,
                    caps: false,
                    align: egui::Align2::LEFT_CENTER,
                    policy: crate::kit::text::Policy::Abbreviate,
                    color: s.t.text_dim,
                };
                let _ = crate::kit::text::paint(b, line, &t, s.tk());
            });
        }
    }
}

/// A resonator (`b` for B): its twelve kinds and its knobs (`all` in Edit). B's title
/// holds its power lamp, and in Edit its coupling.
#[allow(clippy::too_many_arguments)]
fn resonator_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    b: bool,
    all: bool,
) {
    let st = s.function(Function::Source);
    let (title, key) = if b {
        ("fides.b.on", "b.type")
    } else {
        ("fides.res.type", "a.type")
    };
    let tail = if b { 96.0 } else { 0.0 };
    let cr = paint_card(ui, rect, s, &Card::new(title, Function::Source).tail(tail));
    if b {
        lamp(
            app,
            ui,
            tail_rect(&cr, s, tail),
            track,
            spec,
            "b.on",
            Src::Key("fides.b.on"),
            &st,
        );
    }
    let keys = res_keys(b, all);
    let hs: &[f32] = if all {
        &[47.0, 86.0, 86.0]
    } else {
        &[47.0, 86.0]
    };
    let r = rows_in(cr.body, s, hs);
    two_rows(app, ui, r[0], track, spec, key, &RESONATORS, &st);
    if all {
        cells(app, ui, r[1], track, spec, &keys[..5], &st, KnobSize::M);
        cells(app, ui, r[2], track, spec, &keys[5..], &st, KnobSize::M);
    } else {
        cells(app, ui, r[1], track, spec, keys, &st, KnobSize::M);
    }
}

/// The body: its seven kinds (two rows) and its mix, size and ring.
fn body_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let st = s.function(Function::Tone);
    let cr = paint_card(ui, rect, s, &Card::new("fides.body.type", Function::Tone));
    let r = rows_in(cr.body, s, &[47.0, 86.0]);
    two_rows(app, ui, r[0], track, spec, "body.type", &BODIES, &st);
    cells(
        app,
        ui,
        r[1],
        track,
        spec,
        &["body.mix", "body.size", "body.decay"],
        &st,
        KnobSize::M,
    );
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

/// The Play view: the picture and the exciter, then the resonator, the body and the
/// output — the numbers a player turns.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let gutter = s.d(4.0);
    let h2 = s.s(card_h(0.0, 47.0 + 6.0 + 86.0));
    let h1 = (area.height() - h2 - gutter).max(s.s(card_h(0.0, 24.0 + 6.0 + 86.0 + 6.0 + 86.0)));
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(h1, &[0.6, 0.4]);
    let r2 = rows.row(h2, &[0.44, 0.32, 0.24]);
    picture_card(app, ui, track, spec, r1[0], s);
    exciter_card(app, ui, track, spec, r1[1], s);
    resonator_card(app, ui, track, spec, r2[0], s, false, false);
    body_card(app, ui, track, spec, r2[1], s);
    cells_card(
        app,
        ui,
        track,
        spec,
        r2[2],
        s,
        "fides.output",
        Function::Output,
        &[&["release", "width", "gain"]],
    );
}

/// The coupling of A and B: in parallel or A into B, and what is heard of each.
fn coupling_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let st = s.function(Function::Source);
    let cr = paint_card(ui, rect, s, &Card::new("fides.couple", Function::Source));
    let r = rows_in(cr.body, s, &[24.0, 86.0]);
    switch(app, ui, r[0], track, spec, "couple", &COUPLINGS, &st);
    cells(
        app,
        ui,
        r[1],
        track,
        spec,
        &["couple.mix"],
        &st,
        KnobSize::M,
    );
}

/// The voice: its play mode in the title, its numbers in two rows, *Strike again* lit.
fn voice_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let so = s.function(Function::Output);
    let tail = 300.0;
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("fides.voice", Function::Output).tail(tail),
    );
    let t = tail_rect(&cr, s, tail);
    let parts = super::organ::parts::split(t, s.s(6.0), &[0.62, 0.38]);
    switch(app, ui, parts[0], track, spec, "mode", &MODES, &so);
    lamp(
        app,
        ui,
        parts[1],
        track,
        spec,
        "retrig",
        Src::Key("fides.retrig"),
        &so,
    );
    let r = rows_in(cr.body, s, &[86.0, 86.0]);
    let first = ["poly", "glide", "bend", "octave", "transpose", "fine"];
    let second = ["a4", "velocity", "release", "seed"];
    cells(app, ui, r[0], track, spec, &first, &so, KnobSize::M);
    cells(app, ui, r[1], track, spec, &second, &so, KnobSize::M);
}

/// The Edit view: every part of the instrument, each with its card. Taller than the
/// window, it scrolls there.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let gutter = s.d(4.0);
    let heights = [
        s.s(card_h(0.0, 24.0 + 6.0 + 86.0 + 6.0 + 86.0)),
        s.s(card_h(0.0, 47.0 + 6.0 + 86.0 + 6.0 + 86.0)),
        s.s(card_h(0.0, 47.0 + 6.0 + 86.0)),
        s.s(card_h(0.0, 86.0 + 6.0 + 86.0)),
        s.s(card_h(0.0, 86.0)),
        s.s(card_h(0.0, 86.0 + 6.0 + 86.0)),
    ];
    let total = heights.iter().sum::<f32>() + gutter * (heights.len() - 1) as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total), Sense::hover());
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(heights[0], &[0.55, 0.45]);
    picture_card(app, ui, track, spec, r1[0], s);
    exciter_card(app, ui, track, spec, r1[1], s);
    let r2 = rows.row(heights[1], &[0.5, 0.5]);
    resonator_card(app, ui, track, spec, r2[0], s, false, true);
    resonator_card(app, ui, track, spec, r2[1], s, true, true);
    let r3 = rows.row(heights[2], &[0.3, 0.7]);
    coupling_card(app, ui, track, spec, r3[0], s);
    body_card(app, ui, track, spec, r3[1], s);
    let r4 = rows.row(heights[3], &[0.42, 0.58]);
    cells_card(
        app,
        ui,
        track,
        spec,
        r4[0],
        s,
        "fides.bow",
        Function::Dynamics,
        &[
            &["bow.pressure", "bow.speed", "bow.attack"],
            &["bow.release", "mpe.press", "mpe.slide"],
        ],
    );
    cells_card(
        app,
        ui,
        track,
        spec,
        r4[1],
        s,
        "fides.noise",
        Function::Dynamics,
        &[
            &["noise.level", "noise.color", "noise.width", "noise.density"],
            &[
                "noise.attack",
                "noise.decay",
                "noise.sustain",
                "noise.release",
            ],
        ],
    );
    let r5 = rows.row(heights[4], &[0.56, 0.44]);
    cells_card(
        app,
        ui,
        track,
        spec,
        r5[0],
        s,
        "fides.motion",
        Function::Mod,
        &[&[
            "vib.depth",
            "vib.rate",
            "vib.delay",
            "trem.depth",
            "trem.rate",
        ]],
    );
    cells_card(
        app,
        ui,
        track,
        spec,
        r5[1],
        s,
        "fides.output",
        Function::Output,
        &[&["width", "lowcut", "highcut", "gain"]],
    );
    let r6 = rows.row(heights[5], &[1.0]);
    voice_card(app, ui, track, spec, r6[0], s);
}

/// The inspector's small picture: the instrument on the display, without its partials.
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

/// `--editor fides`: the marimba, a note struck.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "fides.preset.marimba")
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Resonator, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: a C4 struck, its partials ringing.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0; cantus_engine::TELEMETRY_LEN];
    t[0] = 3.0;
    t[1] = 16.0;
    t[tel::ALIVE] = 1.0;
    t[tel::KEY] = 60.0;
    t[tel::F0] = 261.6;
    t[tel::LEVEL] = 0.35;
    t[tel::POS] = 0.4;
    for (k, lvl) in [0.02f32, 0.012, 0.006, 0.002, 0.001, 0.0005]
        .iter()
        .enumerate()
    {
        t[tel::MODES + k] = *lvl;
    }
    app.inst.injected = Some(t);
}
