//! Saltator — the dancer (round 35, ADR 0087): its editor on the kit's device shell.
//!
//! **Play** is a small theatre: the stage with the character dancing live (posed at what
//! is heard, `crate::dance_live`) beside the character picker — every character dancing
//! its own preview — then the move map as a strip of pads, then the numbers a dance is
//! made of: the dance, the body, the look, and what it listens to (*React to ‹a track›*).
//! **Edit** is every number as a card. Every number reads in its own unit: a move and a
//! mood by name, speed as ¼×…4×, size as ×, positions and levels as %, colour in degrees.

#[path = "dancer_controls.rs"]
pub mod controls;
#[path = "dancer_moves.rs"]
pub mod moves;
#[path = "dancer_pick.rs"]
pub mod pick;
#[path = "dancer_stage.rs"]
pub mod stage;

use super::organ::parts::{at, lamp, paint_card, switch, Card, Rows};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Function, State as KitState, Style};
use cantus_core::inst::dancer::{self, Move, CHARACTER_KEY};
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use std::collections::HashMap;

pub use pick::character_name;

/// Editor state that is not part of the project.
#[derive(Default)]
pub struct State {
    pub stage: stage::StageState,
    pub pick: pick::PickState,
    /// The pad held down (its key), playing a live note.
    pub pad: Option<u8>,
    /// Where each control landed, for the tests that aim at it.
    pub placed: HashMap<&'static str, Rect>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("dancer::State")
            .field("pad", &self.pad)
            .field("draws", &self.stage.draws)
            .finish()
    }
}

/// *Import character…*: the import flow for this dancer (ADR 0089): a file chosen, the
/// card, *Use*.
pub fn import_character(app: &mut CantusApp, track: TrackId) {
    crate::dance_import::open(app, track);
}

/// *Edit moves…*: an imported character's card again, its mapping to change.
pub fn edit_moves(app: &mut CantusApp, track: TrackId) {
    crate::dance_import::edit_moves(app, track);
}

/// The *Export video…* button's width in the stage card's title bar (at 100 %).
const EXPORT_W: f32 = 112.0;

fn card_h(body: f32) -> f32 {
    24.0 + 8.0 + body + 8.0
}

/// A card of knobs with their readouts in words.
#[allow(clippy::too_many_arguments)]
fn knobs(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    title: &str,
    f: Function,
    keys: &[&str],
) {
    let sf = s.function(f);
    let cr = paint_card(ui, rect, s, &Card::new(title, f));
    controls::cells(app, ui, cr.body, track, spec, keys, &sf, KnobSize::M);
}

/// A caption over a field, in the cells' lettering.
fn caption(ui: &egui::Ui, rect: Rect, key: &str, s: &Style) {
    with_brush(&ui.painter_at(rect), |b| {
        let _ = ktext::paint(
            b,
            rect,
            &Text::new(&t(key), Role::Label, s.t.label)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End)),
            s.tk(),
        );
    });
}

/// Named choices as fields with ‹ › on the left of a card's body (each with its caption),
/// knobs on the right.
#[allow(clippy::too_many_arguments)]
fn fields_and_knobs(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    body: Rect,
    s: &Style,
    fields: &[(&'static str, Vec<&'static str>)],
    knobs: &[&str],
) {
    let kw = crate::kit::cell::size(KnobSize::M).x * s.k;
    let knobs_w = (kw + s.s(8.0)) * knobs.len() as f32;
    let fw = (body.width() - knobs_w - s.s(10.0)).max(s.s(96.0));
    let row = s.s(14.0) + s.s(24.0);
    let gap = s.s(6.0);
    let total = row * fields.len() as f32 + gap * fields.len().saturating_sub(1) as f32;
    let mut y = body.center().y - total * 0.5;
    for (key, labels) in fields {
        let label_key = spec.kind.param(key).map_or(*key, |i| i.label_key);
        caption(
            ui,
            Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(fw, s.s(14.0))),
            label_key,
            s,
        );
        let fr = Rect::from_min_size(
            Pos2::new(body.left(), y + s.s(14.0)),
            Vec2::new(fw, s.s(24.0)),
        );
        at(ui, fr, ("dance-field", *key), |ui| {
            super::analog::parts::stepper_at(app, ui, track, spec, key, labels, fr, s);
        });
        app.inst.dancer.placed.insert(key, fr);
        y += row + gap;
    }
    let kr = Rect::from_min_max(Pos2::new(body.right() - knobs_w, body.top()), body.max);
    controls::cells(app, ui, kr, track, spec, knobs, s, KnobSize::M);
}

/// The dance: what it dances (by name), where it goes (by name), its speed and energy.
fn dance_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let sf = s.function(Function::Source);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("dance.card.dance", Function::Source),
    );
    let fields = [
        ("move", controls::move_words()),
        ("step", controls::STEPS.to_vec()),
    ];
    fields_and_knobs(
        app,
        ui,
        track,
        spec,
        cr.body,
        &sf,
        &fields,
        &["speed", "energy"],
    );
}

/// The look: its mood by name, its colour.
fn look_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let sf = s.function(Function::Mod);
    let cr = paint_card(ui, rect, s, &Card::new("dance.card.look", Function::Mod));
    let fields = [("mood", controls::mood_words())];
    fields_and_knobs(app, ui, track, spec, cr.body, &sf, &fields, &["hue"]);
}

/// The stage card: the theatre, the character, the monitor's words.
fn stage_card(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, rect: Rect, s: &Style) {
    let sf = s.function(Function::Mod);
    let display = (rect.height() / s.k - 24.0 - 1.0 - 6.0).max(40.0);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("dance.card.stage", Function::Mod)
            .display(display)
            .tail(EXPORT_W),
    );
    // *Export video…*: the song with its dancers as a video (the video export's dialog).
    let er = super::organ::parts::tail_rect(&cr, s, EXPORT_W);
    app.inst.dancer.placed.insert("export_video", er);
    if controls::push(ui, er, s, "dance.export_video", "dance.export_video.tip").clicked() {
        crate::video_export::open(app);
    }
    let Some(d) = cr.display else { return };
    app.inst.dancer.placed.insert("stage", d);
    let live = stage::live(app, track);
    let ppp = ui.ctx().pixels_per_point();
    let px = [
        (d.width() * ppp).round().max(1.0) as u32,
        (d.height() * ppp).round().max(1.0) as u32,
    ];
    let tex = live
        .as_ref()
        .and_then(|l| stage::character_texture(&mut app.inst.dancer.stage, ui.ctx(), track, l, px));
    let sd = crate::views::inst::face::on_display(&sf);
    with_brush(&ui.painter_at(d), |b| {
        let (x, accent, playing) = live.as_ref().map_or((0.5, 0.0, false), |l| {
            (l.pose.x, l.pose.accent, l.pose.playing)
        });
        stage::backdrop(b, d, &sd, x, accent, playing);
        if let Some(tex) = tex {
            b.foreign(tex, d);
        }
        if let Some(l) = &live {
            stage::hud(b, d, &sd, l);
        }
        crate::views::inst::face::cover(b, d, &sd);
    });
    // Playing it moves with the song; stopped it breathes — at most 60 frames a second,
    // through the ticker (round 38) — and once it holds still it asks for none.
    if app.engine.is_playing() || !app.dance_live.doze.holding() {
        crate::views::workspace_ticker::moving(
            ui.ctx(),
            crate::views::workspace_ticker::Reason::Animation,
        );
    }
}

/// The character card: the picker.
fn character_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let sf = s.function(Function::Source);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("dance.card.character", Function::Source),
    );
    let wall = app.dance_live.wall();
    pick::picker(app, ui, cr.body, track, spec, &sf, wall);
}

/// The move map's card.
fn moves_card(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, rect: Rect, s: &Style) {
    let sf = s.function(Function::Source);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("dance.card.moves", Function::Source),
    );
    let playing = stage::live(app, track)
        .filter(|l| l.pose.playing)
        .map(|l| l.pose.mv);
    moves::strip(app, ui, cr.body, track, playing, &sf);
}

/// The ears' card: what it reacts to, and how much.
#[allow(clippy::too_many_arguments)]
fn ears_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    with_level: bool,
) {
    let sf = s.function(Function::Dynamics);
    let cr = paint_card(
        ui,
        rect,
        s,
        &Card::new("dance.card.ears", Function::Dynamics),
    );
    let body = cr.body;
    let kw = crate::kit::cell::size(KnobSize::M).x * s.k + s.s(8.0);
    let knobs_w = if with_level { kw * 2.0 } else { kw };
    let field_w = (body.width() - knobs_w - s.s(8.0)).max(s.s(90.0));
    let label = Rect::from_min_size(
        Pos2::new(body.left(), body.center().y - s.s(26.0)),
        Vec2::new(field_w, s.s(16.0)),
    );
    with_brush(&ui.painter_at(label), |b| {
        let _ = ktext::paint(
            b,
            label,
            &Text::new(&t("dance.react.title"), Role::Label, sf.t.label)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End)),
            sf.tk(),
        );
    });
    let field = Rect::from_min_size(
        Pos2::new(body.left(), body.center().y - s.s(8.0)),
        Vec2::new(field_w, s.s(24.0)),
    );
    controls::react_field(app, ui, field, track, &sf);
    let keys: &[&str] = if with_level {
        &["react", "level"]
    } else {
        &["react"]
    };
    let kr = Rect::from_min_max(Pos2::new(body.right() - knobs_w, body.top()), body.max);
    controls::cells(app, ui, kr, track, spec, keys, &sf, KnobSize::M);
}

/// The Play view: the stage and the picker, the move map, the numbers of a dance.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let gutter = s.d(4.0);
    let h3 = s.s(card_h(86.0));
    let h2 = s.s(card_h(40.0));
    let h1 = (area.height() - h2 - h3 - 2.0 * gutter).max(s.s(200.0));
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(h1, &[0.6, 0.4]);
    let r2 = rows.row(h2, &[1.0]);
    let r3 = rows.row(h3, &[0.32, 0.26, 0.22, 0.2]);
    stage_card(app, ui, track, r1[0], s);
    character_card(app, ui, track, spec, r1[1], s);
    moves_card(app, ui, track, r2[0], s);
    let body_keys = ["jump", "turn", "spin", "size"];
    dance_card(app, ui, track, spec, r3[0], s);
    knobs(
        app,
        ui,
        track,
        spec,
        r3[1],
        s,
        "dance.card.body",
        Function::Space,
        &body_keys,
    );
    look_card(app, ui, track, spec, r3[2], s);
    ears_card(app, ui, track, spec, r3[3], s, false);
}

/// The Edit view: every number as a card — the dance, the body, the look, the ears, where
/// it stands and where it shows, and the character.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let h = s.s(card_h(86.0));
    let mut rows = Rows::new(area, s);
    let r1 = rows.row(h, &[0.5, 0.5]);
    let r2 = rows.row(h, &[0.3, 0.38, 0.32]);
    let rest = (area.height() - 2.0 * h - 3.0 * s.d(4.0)).max(s.s(card_h(150.0)));
    let r3 = rows.row(rest, &[0.36, 0.64]);
    let body_keys = ["jump", "turn", "spin", "size"];
    dance_card(app, ui, track, spec, r1[0], s);
    knobs(
        app,
        ui,
        track,
        spec,
        r1[1],
        s,
        "dance.card.body",
        Function::Space,
        &body_keys,
    );
    look_card(app, ui, track, spec, r2[0], s);
    ears_card(app, ui, track, spec, r2[1], s, true);
    let place = ["x", "y", "visible"];
    knobs(
        app,
        ui,
        track,
        spec,
        r2[2],
        s,
        "dance.card.place",
        Function::Output,
        &place,
    );
    show_card(app, ui, track, spec, r3[0], s);
    character_card(app, ui, track, spec, r3[1], s);
}

/// Where it shows: floating over the workspace, docked in the Stage tab, or hidden; and
/// always on top.
fn show_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let sf = s.function(Function::Output);
    let cr = paint_card(ui, rect, s, &Card::new("dance.card.show", Function::Output));
    let body = cr.body;
    let labels: Vec<Src> = controls::VIEWS.iter().map(|k| Src::Key(k)).collect();
    caption(
        ui,
        Rect::from_min_size(
            body.min + Vec2::new(0.0, s.s(4.0)),
            Vec2::new(body.width(), s.s(14.0)),
        ),
        "dance.p.view_mode",
        &sf,
    );
    let sw = Rect::from_min_size(
        Pos2::new(body.left(), body.top() + s.s(20.0)),
        Vec2::new(body.width(), s.s(22.0)),
    );
    at(ui, sw, "dance-view-mode", |ui| {
        switch(app, ui, sw, track, spec, "view.mode", &labels, &sf);
    });
    let lr = Rect::from_min_size(
        Pos2::new(body.left(), body.top() + s.s(52.0)),
        Vec2::new(body.width().min(s.s(180.0)), s.s(22.0)),
    );
    at(ui, lr, "dance-view-top", |ui| {
        lamp(
            app,
            ui,
            lr,
            track,
            spec,
            "view.top",
            Src::Key("dance.p.view_top"),
            &sf,
        );
    });
    app.inst.dancer.placed.insert("view.mode", sw);
    app.inst.dancer.placed.insert("view.top", lr);
}

/// The window's footer for a dancer: the move it dances and the bar and beat (it has no
/// voices and no output to meter).
pub fn footer_chips(app: &CantusApp, track: TrackId) -> (Vec<String>, Vec<Option<String>>) {
    let chips = match stage::live(app, track) {
        Some(l) if l.pose.playing => vec![
            t(l.pose.mv.label_key()),
            crate::i18n::tf(
                "dance.hud.bar_beat",
                &[
                    ("bar", &(l.pose.bar + 1)),
                    ("beat", &(l.pose.beat_in_bar + 1)),
                ],
            ),
        ],
        Some(l) => vec![t(l.pose.mv.label_key()), t("dance.footer.stopped")],
        None => vec![t("dance.footer.stopped")],
    };
    let tips = vec![None; chips.len()];
    (chips, tips)
}

/// The inspector's rows for a dancer (label | value, on the inspector's sheet, as
/// Firmus's): who dances, what, what it hears, where it shows; a click on the character
/// opens the window.
pub fn inspector_rows(
    sh: &mut crate::views::inspector::sheet::Sheet<'_>,
    app: &CantusApp,
    track: TrackId,
) {
    let Some(spec) = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
    else {
        return;
    };
    sh.band("dance.card.dance", Function::Mod, None, false, None);
    let id = spec
        .text(CHARACTER_KEY)
        .unwrap_or(dancer::DEFAULT_CHARACTER);
    let who = cantus_dance::cast::by_id(id)
        .map(|c| character_name(&c))
        .unwrap_or_else(|| id.trim_start_matches("file:").to_string());
    let row = sh.text_row("dance.card.character", &who, false, false);
    let open = sh.ui.interact(
        row.value,
        sh.ui.id().with(("dance-insp", track.0)),
        Sense::click(),
    );
    // r39:ux-a — the help line (ADR 0109).
    let open = crate::views::hint::said_with(open, || t("ux.hint.dancer_character"));
    if open.clicked() {
        sh.acts
            .push(crate::views::inspector::Act::OpenInstrument(track));
    }
    sh.text_row("dance.p.move", &move_text(spec.get("move")), false, false);
    let hears = match dancer::reacting_to(app.project(), track) {
        Some(cantus_core::modulator::Listen::Strip(s)) => controls::strip_label(app.project(), s),
        Some(_) => t("dance.react.song"),
        None => t("dance.react.nothing"),
    };
    sh.text_row("dance.react.title", &hears, false, false);
    let shown = controls::VIEWS[(spec.get("view.mode").round().max(0.0) as usize).min(2)];
    sh.text_row("dance.p.view_mode", &t(shown), false, false);
}

/// What `move` plays, in words: a move's name, or the routine to the beat.
pub fn move_text(v: f32) -> String {
    match dancer::move_of(v) {
        Some(m) => t(m.label_key()),
        None => t("dance.to_the_beat"),
    }
}

/// The inspector's line: the instrument, its character and what it dances.
pub fn preview_text(spec: &NativeSpec) -> String {
    let id = spec
        .text(CHARACTER_KEY)
        .unwrap_or(dancer::DEFAULT_CHARACTER);
    let who = cantus_dance::cast::by_id(id)
        .map(|c| character_name(&c))
        .unwrap_or_else(|| id.trim_start_matches("file:").to_string());
    format!(
        "{} \u{b7} {} \u{b7} {}",
        t(spec.kind.name_key()),
        who,
        move_text(spec.get("move"))
    )
}

/// The inspector's small picture: a well with the line.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(Function::Mod);
    let text = preview_text(spec);
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(36.0)), Sense::hover());
    with_brush(&ui.painter_at(rect), |b| {
        well::paint(b, rect, &s, KitState::REST, None);
        let tr = rect.shrink2(Vec2::new(s.s(8.0), 0.0));
        let _ = ktext::paint(
            b,
            tr,
            &Text::new(&text, Role::Body, s.t.text)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End)),
            s.tk(),
        );
    });
    resp
}

/// `--editor dancer`: a party, wandering about the stage.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "dance.preset.party")
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Dancer, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: a dancer reports no telemetry of its own.
pub fn demo_feed(app: &mut CantusApp) {
    app.inst.injected = Some([0.0; cantus_engine::TELEMETRY_LEN]);
}

/// Factory presets: how it dances, from calm to a party.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    FactoryPreset {
        label_key: "dance.preset.beat",
        values: &[],
        text: &[],
        tags: &[],
    },
    FactoryPreset {
        label_key: "dance.preset.party",
        values: &[("energy", 1.0), ("step", 3.0), ("mood", 2.0)],
        text: &[],
        tags: &[],
    },
    FactoryPreset {
        label_key: "dance.preset.chill",
        values: &[("energy", 0.35), ("speed", 1.0), ("mood", 3.0)],
        text: &[],
        tags: &[],
    },
    FactoryPreset {
        label_key: "dance.preset.cool",
        values: &[("energy", 0.6), ("mood", 6.0), ("turn", 0.6)],
        text: &[],
        tags: &[],
    },
];

/// The move each key of the map plays, by name (the keyboard strip, an agent).
pub fn move_map() -> Vec<(u8, Move)> {
    Move::ALL
        .iter()
        .map(|m| (dancer::BASE_KEY + m.index() as u8, *m))
        .collect()
}
