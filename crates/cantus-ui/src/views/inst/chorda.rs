//! Chorda — the modelled grand piano: its editor, on the kit's device shell (ADR 0024 §8,
//! ADR 0059). Owned by this instrument's builder (ADR 0023).
//!
//! What a physically modelled piano's editor usually hides and this one shows, from the
//! model's own numbers:
//!
//! - **The piano from above**: every key's strings at their speaking length (one, two or
//!   three), the bridge, where the hammers strike, the dampers as firmly down as the pedal
//!   leaves them — the key last played lit, its hammer shifted by the soft pedal.
//! - **The half-pedal law**: how far down the dampers are at each position of the pedal,
//!   with the pedal's position on it.
//! - **The tuning's stretch** in cents across the keyboard, redrawn as the stretch and the
//!   strings' stiffness move (a new tuning is designed off the audio thread; the caption
//!   says so while it is).
//! - **The felt**, force against compression, as measured and as set; **the ring** of every
//!   key, free and damped.
//!
//! The look (ADR 0024): **Play** is the piano in its recording room (the scene, ADR 0104)
//! beside the pedals, and a row of cells (hammer, resonance, output); **Edit** is every part
//! as a card in its function hue — strings (the piano from above), tuning, hammer, the
//! strings' ring, pedals, resonance, output.

#[path = "chorda_draw.rs"]
pub(crate) mod draw;
// The piano in its recording room, the Play view's hero (ADR 0104). r37:scene
#[path = "chorda_scene.rs"]
pub mod scene;
#[path = "chorda_scene_view.rs"]
pub mod view;
// r40:chorda-src — the Play view as the approved design draws it (design-p3, delivery 6).
#[path = "chorda_play.rs"]
mod layout;

use super::organ::parts::{cells, paint_card, Card, Rows};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Policy, Role, Text};
use crate::kit::{well, with_brush, Fill, Function, State as KitState, Style};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::chorda::tables::{self, Key, KEYS};
use cantus_engine::inst::chorda::{settings_from, tel, tuning_of, values_of};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

pub use draw::Now;

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    /// Where each control of the last frame (cell) sits, by parameter key.
    pub placed: std::collections::HashMap<&'static str, egui::Rect>,
    /// The rect the cards were laid out in, and the part of it that is visible.
    pub area: Option<egui::Rect>,
    pub view: Option<egui::Rect>,
    /// The keyboard drawn, and the tuning it was computed for (a4, stretch, stiffness).
    keys: Option<([f32; 3], Box<[Key; KEYS]>)>,
    /// Each Chorda's scene (ADR 0126) and its card's words, by track: one Chorda's piano is
    /// never shown on another's. r40:scene
    scenes: std::collections::BTreeMap<u64, (crate::scene::Scene, view::Words)>,
}

impl State {
    /// r40:fix-f — keep only the scenes of tracks `keep` says (finding 6).
    pub fn retain_scenes(&mut self, keep: impl Fn(u64) -> bool) {
        self.scenes.retain(|id, _| keep(*id));
    }

    /// r40:fix-f — drop every scene (another song).
    pub fn clear_scenes(&mut self) {
        self.scenes.clear();
    }

    /// How many scenes are kept (the tests).
    pub fn scenes_kept(&self) -> usize {
        self.scenes.len()
    }

    /// Whether a scene kept has a picture on its way (the picker's thumbnail waits for it).
    pub fn scenes_building(&self) -> bool {
        self.scenes.values().any(|s| s.0.building())
    }

    /// `track`'s scene, once its card has been drawn. r40:scene
    pub fn scene_of(&self, track: TrackId) -> Option<&crate::scene::Scene> {
        self.scenes.get(&track.0).map(|s| &s.0)
    }

    /// Whether `track`'s scene shows its microphones. r40:scene
    pub fn mics_shown(&self, track: TrackId) -> bool {
        self.scenes.get(&track.0).is_some_and(|s| s.1.mics)
    }

    /// What the pointer is over in `track`'s scene (the lid). r40:scene
    pub fn scene_hovered(&self, track: TrackId) -> Option<crate::scene::Picked> {
        self.scenes.get(&track.0).and_then(|s| s.1.hovered())
    }

    /// Show or hide `track`'s scene's microphones (its *Show mics* key). r40:scene
    pub fn show_mics(&mut self, track: TrackId, on: bool) {
        self.scenes.entry(track.0).or_default().1.mics = on;
    }
}

/// For the tests: the scene's card alone, for `track`'s Chorda, in `rect`. r37:scene
#[doc(hidden)]
pub fn scene_card_for_tests(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, rect: Rect) {
    let Some(spec) = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
    else {
        return;
    };
    let s = Style::current().function(Function::Space);
    view::scene_card(app, ui, track, &spec, rect, &s, None);
}

/// The keyboard of `spec`'s tuning (kept while the tuning stays).
fn keyboard<'a>(state: &'a mut State, spec: &NativeSpec) -> &'a [Key; KEYS] {
    let want = [spec.get("a4"), spec.get("stretch"), spec.get("stiffness")];
    if state.keys.as_ref().map(|k| k.0) != Some(want) {
        let tu = tuning_of(&values_of(spec));
        state.keys = Some((
            want,
            Box::new(tables::keyboard(tu.a4, tu.stretch, tu.stiffness)),
        ));
    }
    &state.keys.as_ref().expect("just set").1
}

/// What the pictures show: the engine's report, or with no engine, middle C at rest.
fn now_of(tele: &cantus_engine::Telemetry) -> Now {
    let alive = tele[tel::ALIVE] > 0.5;
    let key = tele[tel::KEY] as u8;
    Now {
        key: if alive && (tables::LOWEST..=tables::HIGHEST).contains(&key) {
            key
        } else {
            60
        },
        playing: alive && tele[0] > 0.0,
        velocity: tele[tel::VELOCITY].clamp(0.0, 1.0),
        pedal: tele[tel::PEDAL].clamp(0.0, 1.0),
        soft: tele[tel::SOFT].clamp(0.0, 1.0),
    }
}

/// While the engine is still designing the tuning asked for (the piano silent, or on its
/// previous tuning): what to say, with the design's progress.
pub fn designing(app: &CantusApp, track: TrackId, spec: &NativeSpec) -> Option<String> {
    let tele = app.inst_telemetry();
    if tele[tel::ALIVE] < 0.5 || tele[tel::DESIGNED] > 0.5 {
        return None;
    }
    // r42 — the design the engine makes under the track's tuning (ADR 0155).
    let table = crate::views::tuning::table_of(app, crate::views::tuning::Target::Track(track));
    caption(spec, &table.0, app.engine.sample_rate as f32)
}

/// What the editor says of a tuning the piano does not play yet: its design's progress,
/// or — when this session cannot design it (sound review 2: it read "0 %" for good) —
/// that the piano keeps the tuning it has.
fn caption(spec: &NativeSpec, offsets: &[f32; 128], fs: f32) -> Option<String> {
    words(cantus_engine::inst::chorda::design(
        cantus_engine::inst::chorda::tuning_for(&values_of(spec), offsets),
        fs,
    ))
}

/// What the caption says of a design: how far it has come, or that it will not come and
/// the unit keeps the keyboard it has (the registry full, or no designer — review of fix4:
/// that read "designing: 0 %" for the session); nothing once designed.
fn words(d: cantus_engine::inst::chorda::Design) -> Option<String> {
    use cantus_engine::inst::chorda::Design;
    match d {
        Design::Ready => None,
        Design::Refused => Some(t("chorda.no_room")),
        Design::NoDesigner => Some(t("chorda.no_designer")),
        Design::Coming(done) => Some(crate::i18n::tf(
            "chorda.designing_pct",
            &[("pct", &format!("{:.0}", done * 100.0))],
        )),
    }
}

/// Whether `spec` plays the second model (ADR 0098): its room and its action's noise.
pub fn model2(spec: &NativeSpec) -> bool {
    spec.get("model") >= 1.5
}

/// The height of a card with a `display` and a `body`, at 100 %.
fn card_h(display: f32, body: f32) -> f32 {
    24.0 + if display > 0.0 { 1.0 + display } else { 0.0 } + 8.0 + body + 8.0
}

/// A card: its title, a display drawn by `paint` (when `display` > 0), its cells below.
#[allow(clippy::too_many_arguments)]
fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    title: &str,
    f: Function,
    display: f32,
    keys: &[&str],
    paint: impl FnOnce(&mut crate::kit::Brush, Rect, &Style),
) -> (Option<Rect>, Rect) {
    let sf = s.function(f);
    let mut c = Card::new(title, f);
    if display > 0.0 {
        c = c.display(display);
    }
    let cr = paint_card(ui, rect, s, &c);
    if let Some(d) = cr.display {
        let sd = crate::views::inst::face::on_display(&sf);
        with_brush(&ui.painter_at(d), |b| {
            paint(b, d, &sd);
            crate::views::inst::face::cover(b, d, &sd);
        });
    }
    if !keys.is_empty() {
        cells(app, ui, cr.body, track, spec, keys, &sf, KnobSize::M);
    }
    (cr.display, cr.body) // r42:tuning-face — where the tuning's line and A4 go
}

/// The display's height that makes a card `h` points tall (at the style's scale) with a
/// row of cells, or with none.
fn display_for(h: f32, s: &Style, with_cells: bool) -> f32 {
    let d = if with_cells {
        h / s.k - card_h(0.0, 86.0) - 1.0
    } else {
        h / s.k - 24.0 - 1.0 - 6.0
    };
    d.max(40.0)
}

/// The Play view: the source list, the piano in its room, the keyboard, the player's cards
/// and *The piano* (round 40 phase 3: the approved design — `chorda_play.rs`).
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    layout::play(app, ui, track, spec, s) // r40:chorda-src
}

/// The Edit view: every part of the instrument, each with its picture. Taller than the
/// window, it scrolls there.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let tele = app.inst_telemetry();
    let now = now_of(&tele);
    let busy = designing(app, track, spec);
    let (vs, hardness) = settings_from(&values_of(spec));
    let grip = vs.voice.damper as f32;
    let decay = vs.voice.decay;
    // r42 — the diapason Chorda plays at: its own, its track's tuning's, or the MTS-ESP
    // master's — as its tuning card says it (r42 fix2-model, review 11).
    let a4 = crate::views::tuning::chorda::shown_a4(app, track, spec);
    let gutter = s.d(4.0);
    let small = s.s(card_h(0.0, 86.0));
    // Model 2 has a fourth row: its microphones, lid and room (round 37). r37:piano
    let m2 = model2(spec);
    let mut heights = vec![
        s.s(card_h(170.0, 86.0)),
        // r42:tuning-face — Chorda's own tuning and its presets (the owner, 2026-11-22).
        s.s(card_h(0.0, crate::views::tuning::chorda::BODY)),
        s.s(card_h(110.0, 86.0)),
        small * 2.0 + gutter,
    ];
    if m2 {
        heights.push(s.s(card_h(120.0, 86.0)));
    }
    let total = heights.iter().sum::<f32>() + gutter * (heights.len() - 1) as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), total), Sense::hover());
    app.inst.chorda.area = Some(area);
    app.inst.chorda.view = Some(ui.clip_rect());
    let keys = *keyboard(&mut app.inst.chorda, spec);
    let shown = keys
        .iter()
        .find(|k| k.key == now.key)
        .copied()
        .unwrap_or(keys[39]);
    let mut rows = Rows::new(area, s);

    let r1 = rows.row(heights[0], &[0.6, 0.4]);
    card(
        app,
        ui,
        track,
        spec,
        r1[0],
        s,
        "chorda.strings",
        Function::Source,
        170.0,
        &["unison", "coupling", "phantom"],
        |b, d, sf| draw::grand(b, d, sf, &keys, &now, grip, busy.as_deref()),
    );
    // r42:tuning-face — the song's tuning (ADR 0155) on a line of its own under the
    // stretch: Chorda's stretch, stiffness and unisons stay its own, beside it.
    let line_h = s.d(20.0);
    let (tune, body) = card(
        app,
        ui,
        track,
        spec,
        r1[1],
        s,
        "chorda.tuning",
        Function::Tone,
        170.0,
        &[],
        |b, d, sf| {
            let top = Rect::from_min_max(d.min, Pos2::new(d.right(), d.bottom() - line_h));
            draw::stretch(b, top, sf, &keys, a4, busy.is_some())
        },
    );
    if let Some(d) = tune {
        let line = Rect::from_min_max(Pos2::new(d.left(), d.bottom() - line_h), d.max);
        crate::views::tuning::chorda_line(app, ui, track, line, s); // r42:tuning-face
    }
    // r42:tuning-face — the A4 rule (one diapason): Chorda's own knob only while the track
    // plays no tuning; else the project's diapason greyed, or its own tuning's A4.
    {
        use crate::views::tuning::chorda::{a4_cell, a4_of, A4};
        let sf = s.function(Function::Tone);
        let third = body.width() / 3.0;
        let part = |i: f32| {
            Rect::from_min_size(
                Pos2::new(body.left() + third * i, body.top()),
                Vec2::new(third, body.height()),
            )
        };
        match a4_of(app, track) {
            A4::Knob => cells(app, ui, part(0.0), track, spec, &["a4"], &sf, KnobSize::M),
            other => a4_cell(app, ui, track, part(0.0), other, s),
        }
        cells(
            app,
            ui,
            part(1.0),
            track,
            spec,
            &["stretch"],
            &sf,
            KnobSize::M,
        );
        cells(
            app,
            ui,
            part(2.0),
            track,
            spec,
            &["stiffness"],
            &sf,
            KnobSize::M,
        );
    }
    // r42:tuning-face — Chorda's own tuning: the project's or its own, the presets, the key,
    // the circle and its purest keys, the cadence.
    let rt = rows.row(heights[1], &[1.0]);
    let tc = paint_card(
        ui,
        rt[0],
        s,
        &Card::new("tuning.chorda.card", Function::Tone),
    );
    crate::views::tuning::chorda::body(
        app,
        ui,
        track,
        spec,
        tc.body.shrink2(Vec2::new(s.d(10.0), s.d(6.0))),
        s,
    );

    let r2 = rows.row(heights[2], &[0.5, 0.5]);
    card(
        app,
        ui,
        track,
        spec,
        r2[0],
        s,
        "chorda.hammer",
        Function::Dynamics,
        110.0,
        &["hardness", "hysteresis", "dynamics", "vel.curve"],
        |b, d, sf| draw::felt(b, d, sf, &shown, hardness),
    );
    card(
        app,
        ui,
        track,
        spec,
        r2[1],
        s,
        "chorda.ring",
        Function::Source,
        110.0,
        &["decay"],
        |b, d, sf| draw::ring(b, d, sf, &keys, now.key, decay),
    );

    let r3 = rows.row(heights[3], &[0.5, 0.5]);
    card(
        app,
        ui,
        track,
        spec,
        r3[0],
        s,
        "chorda.pedals",
        Function::Dynamics,
        display_for(heights[3], s, true),
        &["pedal", "damper", "una"],
        |b, d, sf| draw::pedals(b, d, sf, &now, grip),
    );
    let right = r3[1];
    let top = Rect::from_min_size(right.min, Vec2::new(right.width(), small));
    let bottom = Rect::from_min_size(
        Pos2::new(right.left(), right.top() + small + gutter),
        Vec2::new(right.width(), small),
    );
    let none = |_: &mut crate::kit::Brush, _: Rect, _: &Style| {};
    card(
        app,
        ui,
        track,
        spec,
        top,
        s,
        "chorda.resonance",
        Function::Space,
        0.0,
        &["sympathy", "body"],
        none,
    );
    card(
        app,
        ui,
        track,
        spec,
        bottom,
        s,
        "chorda.output",
        Function::Output,
        0.0,
        if m2 {
            // r42 (`chorda-c17`): model 2's colour beside its tilt.
            &["tilt", "colour", "gain"]
        } else {
            &["tilt", "width", "gain"]
        },
        none,
    );
    if m2 {
        let r4 = rows.row(heights[4], &[0.6, 0.4]);
        let st = (
            vs.persp as f32,
            vs.lid as f32,
            vs.width as f32,
            vs.room as f32,
            vs.size as f32,
        );
        card(
            app,
            ui,
            track,
            spec,
            r4[0],
            s,
            "chorda.mics",
            Function::Space,
            120.0,
            &["persp", "width", "lid"],
            |b, d, sf| draw::stage(b, d, sf, st.0, st.1, st.2),
        );
        card(
            app,
            ui,
            track,
            spec,
            r4[1],
            s,
            "chorda.space",
            Function::Space,
            120.0,
            &["room", "roomsize", "noise"],
            |b, d, sf| draw::room(b, d, sf, st.3, st.4),
        );
    }
}

/// The inspector's line: the name, A4 and the stretch as the editor's knobs read them.
pub fn preview_text(spec: &NativeSpec) -> String {
    format!(
        "{} \u{b7} {} {:.1} Hz \u{b7} {} {:.0} %",
        t(spec.kind.name_key()),
        t("chorda.a4"),
        spec.get("a4"),
        t("chorda.stretch"),
        spec.get("stretch") * 100.0
    )
}

/// The inspector's small picture: three strings and a hammer, the tuning.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(Function::Source);
    let text = preview_text(spec);
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(36.0)), Sense::hover());
    with_brush(&ui.painter_at(rect), |b| {
        well::paint(b, rect, &s, KitState::REST, None);
        let y = rect.center().y;
        let x0 = rect.left() + s.s(8.0);
        for j in 0..3 {
            let yy = y - s.s(4.0) + j as f32 * s.s(4.0);
            b.segment(
                Pos2::new(x0, yy),
                Pos2::new(x0 + s.s(40.0), yy),
                s.s(1.2),
                s.t.text_dim,
                true,
            );
        }
        b.rrect(
            Rect::from_center_size(
                Pos2::new(x0 + s.s(12.0), y + s.s(9.0)),
                Vec2::new(s.s(8.0), s.s(5.0)),
            ),
            s.r(1.5),
            Fill::Solid(s.hue),
        );
        let tr = Rect::from_min_max(
            Pos2::new(x0 + s.s(50.0), rect.top()),
            Pos2::new(rect.right() - s.s(6.0), rect.bottom()),
        );
        // Two lines, broken before the stretch, where one does not hold it (round 39,
        // ADR 0109: Spanish cut "Estiramiento").
        let whole = ktext::measure(b.ctx(), &Text::new(&text, Role::Body, s.t.text), s.tk()).x;
        if whole > tr.width() {
            if let Some(at) = text.rfind(" \u{b7} ") {
                let two = format!("{}\n{}", &text[..at], &text[at + " \u{b7} ".len()..]);
                let ctx = b.ctx().clone();
                let g = crate::views::audio_parts::wrapped(
                    &ctx,
                    &two,
                    Role::Body,
                    s.tk(),
                    tr.width(),
                    2,
                );
                let yy = tr.center().y - g.size().y * 0.5;
                b.text(Pos2::new(b.snap(tr.left()), b.snap(yy)), g, s.t.text, 0.0);
                return;
            }
        }
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

/// `--editor chorda`: the concert grand, an E5 sounding with the pedal partly down.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.concert")
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Chorda, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: E5 struck firmly, the pedal at 40 %.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0; cantus_engine::TELEMETRY_LEN];
    t[0] = 6.0;
    t[1] = 88.0;
    t[tel::KEY] = 76.0;
    t[tel::VELOCITY] = 0.8;
    t[tel::PEDAL] = 0.4;
    t[tel::PEAK_L] = 0.3;
    t[tel::PEAK_R] = 0.35;
    t[tel::DESIGNED] = 1.0;
    t[tel::ALIVE] = 1.0;
    app.inst.injected = Some(t);
}

// ---- factory presets --------------------------------------------------------------------

/// A preset's text: four macros on every preset (1 hardness, 2 sympathy, 3 the body, 4 the
/// decay) and its look (ADR 0104: the piano the scene shows, which must fit the preset's
/// sound — `chorda_look::fits`). r37:scene
macro_rules! text_with_look {
    ($body:literal, $finish:literal) => {
        &[
            ("macro1.name", "Hardness"),
            ("mod1.src", "macro1"),
            ("mod1.dst", "hardness"),
            ("macro2.name", "Resonance"),
            ("mod2.src", "macro2"),
            ("mod2.dst", "sympathy"),
            ("macro3.name", "Body"),
            ("mod3.src", "macro3"),
            ("mod3.dst", "body"),
            ("macro4.name", "Decay"),
            ("mod4.src", "macro4"),
            ("mod4.dst", "decay"),
            ("look.body", $body),
            ("look.finish", $finish),
        ]
    };
}

macro_rules! preset {
    ($label:expr, $tags:expr, $body:literal / $finish:literal, [$($k:expr => $v:expr),* $(,)?]) => {
        FactoryPreset {
            label_key: $label,
            values: &[
                ("mod1.amt", 0.4),
                ("mod2.amt", 0.5),
                ("mod3.amt", 0.4),
                ("mod4.amt", 0.3),
                $(($k, $v)),*
            ],
            text: text_with_look!($body, $finish),
            tags: $tags,
        }
    };
}

/// Factory presets (round 37, ADR 0098): twenty-four, voiced for model 2 — a preset applied
/// starts from a new instance, so it plays model 2 — each tagged with the preset library's
/// styles and roles (`preset.style.*`, `preset.role.*`, ADR 0101). r37:piano
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    preset!(
        "chorda.preset.concert",
        &[
            "preset.tag.keys",
            "preset.role.keys",
            "chorda.tag.grand",
            "preset.style.film",
            "preset.style.worship"
        ], "concert" / "black",
        ["gain" => 4.8]
    ),
    preset!("chorda.preset.studio", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.pop"], "grand" / "black", ["hardness" => 0.25, "tilt" => 1.5, "room" => 0.25, "width" => 0.85, "persp" => 0.25, "roomsize" => 0.3, "gain" => 3.5]),
    preset!("chorda.preset.ballad", &["preset.tag.keys", "preset.role.keys", "chorda.tag.soft", "preset.style.worship", "preset.style.pop"], "grand" / "white", ["hardness" => -0.3, "tilt" => -2.0, "sympathy" => 1.25, "decay" => 1.15, "room" => 0.5, "persp" => 0.3, "roomsize" => 0.55, "gain" => 4.0]),
    preset!("chorda.preset.jazz", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.jazz"], "grand" / "walnut", ["hardness" => 0.1, "body" => 0.9, "width" => 0.6, "decay" => 0.9, "dynamics" => 1.15, "unison" => 1.4, "room" => 0.3, "gain" => 7.5, "persp" => 0.2, "lid" => 0.6, "roomsize" => 0.35]),
    preset!("chorda.preset.felt", &["preset.tag.keys", "preset.role.keys", "chorda.tag.soft", "preset.style.lofi", "preset.style.ambient"], "upright" / "black", ["hardness" => -0.85, "tilt" => -8.0, "decay" => 0.6, "sympathy" => 0.8, "dynamics" => 0.7, "noise" => 1.6, "room" => 0.3, "persp" => 0.08, "lid" => 0.0, "roomsize" => 0.2, "gain" => 4.8]),
    preset!("chorda.preset.honky", &["preset.tag.keys", "preset.role.keys", "chorda.tag.character", "preset.style.rock"], "upright" / "walnut", ["unison" => 7.0, "coupling" => 0.3, "hardness" => 0.4, "tilt" => 3.0, "body" => 0.85, "room" => 0.25, "gain" => 6.9, "persp" => 0.3, "lid" => 0.5, "roomsize" => 0.3]),
    preset!("chorda.preset.baroque", &["preset.tag.keys", "preset.role.keys", "chorda.tag.character", "preset.style.film"], "parlour" / "rosewood", ["a4" => 415.0, "stretch" => 0.7, "hardness" => 0.15, "room" => 0.55, "persp" => 0.6, "roomsize" => 0.65, "gain" => 4.2]),
    preset!("chorda.preset.wash", &["preset.tag.keys", "preset.role.pad", "chorda.tag.soft", "preset.style.ambient"], "concert" / "black", ["sympathy" => 2.0, "decay" => 1.6, "body" => 1.3, "width" => 0.9, "damper" => 0.55, "room" => 0.75, "persp" => 0.7, "roomsize" => 0.9, "gain" => 4.2]),
    preset!("chorda.preset.pop", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.pop"], "grand" / "white", ["hardness" => 0.5, "dynamics" => 0.75, "tilt" => 3.0, "width" => 0.75, "room" => 0.3, "gain" => 0.2, "persp" => 0.12, "roomsize" => 0.3]),
    preset!("chorda.preset.old", &["preset.tag.keys", "preset.role.keys", "chorda.tag.character", "preset.style.lofi"], "upright" / "rosewood", ["stiffness" => 1.3, "unison" => 3.5, "decay" => 0.75, "body" => 0.7, "tilt" => -2.0, "coupling" => 0.65, "noise" => 1.4, "gain" => 10.4, "persp" => 0.25, "lid" => 0.0, "roomsize" => 0.25]),
    preset!("chorda.preset.hall", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.film"], "concert" / "black", ["room" => 0.8, "width" => 0.9, "sympathy" => 1.2, "tilt" => -0.5, "persp" => 0.85, "roomsize" => 0.85, "gain" => 2.1]),
    preset!("chorda.preset.cinematic", &["preset.tag.keys", "preset.role.pad", "chorda.tag.soft", "preset.style.film", "preset.style.ambient"], "concert" / "black", ["hardness" => -0.5, "room" => 0.85, "decay" => 1.3, "sympathy" => 1.5, "tilt" => -3.0, "dynamics" => 1.2, "gain" => 9.0, "persp" => 0.6, "roomsize" => 1.0]),
    preset!("chorda.preset.gospel", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.worship"], "grand" / "black", ["hardness" => 0.3, "dynamics" => 1.15, "room" => 0.4, "tilt" => 1.0, "width" => 0.8, "gain" => 5.2, "persp" => 0.3, "roomsize" => 0.6]),
    preset!("chorda.preset.rock", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.rock"], "grand" / "black", ["hardness" => 0.6, "tilt" => 2.5, "room" => 0.15, "dynamics" => 0.7, "width" => 0.7, "gain" => -0.3, "persp" => 0.1, "roomsize" => 0.25]),
    preset!("chorda.preset.montuno", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.latin"], "parlour" / "walnut", ["hardness" => 0.45, "tilt" => 2.0, "room" => 0.25, "decay" => 0.8, "body" => 1.1, "persp" => 0.2, "lid" => 0.7, "roomsize" => 0.35, "gain" => 4.9]),
    preset!("chorda.preset.rnb", &["preset.tag.keys", "preset.role.keys", "chorda.tag.soft", "preset.style.rnb"], "parlour" / "white", ["hardness" => -0.15, "tilt" => -1.0, "room" => 0.35, "width" => 0.75, "dynamics" => 0.85, "persp" => 0.2, "roomsize" => 0.4, "gain" => 2.2]),
    preset!("chorda.preset.close", &["preset.tag.keys", "preset.role.keys", "chorda.tag.soft", "preset.style.pop", "preset.style.rnb"], "parlour" / "black", ["hardness" => -0.2, "room" => 0.1, "noise" => 1.5, "width" => 0.65, "persp" => 0.0, "roomsize" => 0.2, "gain" => -1.0]),
    preset!("chorda.preset.trap", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.trap"], "grand" / "black", ["hardness" => 0.2, "tilt" => 1.5, "room" => 0.3, "decay" => 0.7, "dynamics" => 0.8, "persp" => 0.15, "roomsize" => 0.3, "gain" => 2.4]),
    preset!("chorda.preset.dry", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.pop", "preset.style.house"], "grand" / "black", ["room" => 0.0, "width" => 0.6, "noise" => 0.8, "persp" => 0.15, "gain" => 2.9]),
    preset!("chorda.preset.lofi", &["preset.tag.keys", "preset.role.keys", "chorda.tag.character", "preset.style.lofi", "preset.style.trap"], "parlour" / "walnut", ["hardness" => -0.45, "tilt" => -5.0, "unison" => 3.0, "room" => 0.2, "noise" => 1.5, "width" => 0.5, "decay" => 0.8, "gain" => 6.1, "persp" => 0.1, "lid" => 0.3, "roomsize" => 0.2]),
    preset!("chorda.preset.house", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.house"], "grand" / "white", ["hardness" => 0.6, "tilt" => 3.0, "room" => 0.2, "dynamics" => 0.7, "decay" => 0.7, "gain" => 1.2, "persp" => 0.12, "roomsize" => 0.3]),
    preset!("chorda.preset.reggaeton", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.latin", "preset.style.trap"], "grand" / "white", ["hardness" => 0.4, "tilt" => 2.0, "room" => 0.25, "decay" => 0.75, "width" => 0.7, "gain" => 3.6, "persp" => 0.15, "roomsize" => 0.35]),
    preset!("chorda.preset.ambient", &["preset.tag.keys", "preset.role.pad", "chorda.tag.soft", "preset.style.ambient", "preset.style.film"], "concert" / "white", ["hardness" => -0.6, "sympathy" => 2.0, "decay" => 2.0, "room" => 0.9, "damper" => 0.4, "tilt" => -4.0, "persp" => 0.75, "roomsize" => 1.0, "gain" => 4.9]),
    preset!("chorda.preset.theme", &["preset.tag.keys", "preset.role.keys", "chorda.tag.grand", "preset.style.film"], "concert" / "black", ["hardness" => 0.1, "room" => 0.65, "width" => 0.9, "sympathy" => 1.2, "persp" => 0.5, "roomsize" => 0.7, "gain" => 4.2]),
];

#[cfg(test)]
mod tests {
    use cantus_core::{InstrumentKind, NativeSpec};
    use cantus_engine::inst::chorda::{tuning_of, values_of};

    #[test]
    fn a_tuning_this_session_cannot_design_says_so_never_zero_percent() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let mut spec = NativeSpec::new(InstrumentKind::Chorda);
        spec.set("a4", 441.23);
        cantus_engine::inst::chorda::refuse_for_tests(tuning_of(&values_of(&spec)), 48_000.0);
        let said = super::caption(&spec, &[0.0; 128], 48_000.0).expect("a caption");
        assert_eq!(said, crate::i18n::t("chorda.no_room"));
        assert!(!said.contains('%'), "{said}");
    }

    /// Review of fix4: with no designer (its thread could not start — the engine's
    /// `a_tuning_no_designer_can_make_is_not_on_its_way` puts it there with the spawn
    /// fault) the caption says so in both languages, never a percentage.
    #[test]
    fn a_tuning_no_designer_can_make_says_so_never_zero_percent() {
        use cantus_engine::inst::chorda::Design;
        for locale in [crate::i18n::Locale::En, crate::i18n::Locale::Es] {
            crate::i18n::set_thread_locale(locale);
            let said = super::words(Design::NoDesigner).expect("a caption");
            assert_eq!(said, crate::i18n::t("chorda.no_designer"));
            assert!(!said.contains('%'), "{said}");
            assert_ne!(said, crate::i18n::t("chorda.no_room"));
            assert!(super::words(Design::Coming(0.5)).is_some_and(|w| w.contains("50")));
            assert_eq!(super::words(Design::Ready), None);
        }
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
    }
}
