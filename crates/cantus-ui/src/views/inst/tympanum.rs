//! Tympanum, the acoustic drummer: its editor (round 40 phase 3, ADR 0124), from the approved
//! design (`F:\projects\daw-r40\design-p3\`, the *Kit* and *Mixer* sheets).
//!
//! - **Kit** (the frame's first view): the kits list at the left — *Modelled* (instant, no
//!   download), *Recorded* (free packs, each with its download, its size, its progress and
//!   its cancel, and its mode said), *Your own kits* (*Install from a file…*); the kit in its
//!   room in the middle, drawn by the scene module (ADR 0126) — a click plays and selects a
//!   piece, *Show mics* puts the mics in, the room card picks the room and its look; under
//!   it the row of pieces with their keys and the kit's Feel, Hi-hat pedal and Kit cards; at
//!   the right the selected piece: its articulations on their keys, where it is struck, its
//!   sound and which mics hear it.
//! - **Mixer** (the second view): the selected mic's band (the kit with its mics, the
//!   selected one ringed, its card: pattern, distance, aim, bleed, what it hears or every
//!   mic's matrix), the one-click routes, and the strips (`super::inst_mixer`, shared with
//!   Chorda): *Kit* pinned left, *Ambience* right, the mics under family heads between.
//! - **Matrix**: the frame's modulation matrix, as every instrument's.
//!
//! What the views keep that is not the song (the selected piece and mic, the list's filter,
//! *Show mics*, the open list) lives in egui's memory per track ([`Look`]); every edit is a
//! command (`SetInstrumentValue`/`SetInstrumentText`, batches as one undo step).

#[path = "tympanum_cards.rs"]
mod cards;
#[path = "tympanum_def.rs"]
pub mod def;
// r40:fix-f — every kit change carries the routes, the mixer and the strips (finding 1).
#[path = "tympanum_kit_change.rs"]
pub mod kit_change;
#[path = "tympanum_kits.rs"]
pub mod kits; // r40:chorda-src — `install_kit_path` for the tests
#[path = "tympanum_mics.rs"]
mod mics;
#[path = "tympanum_piece.rs"]
mod piece;
#[path = "tympanum_stage.rs"]
pub mod stage;
#[path = "tympanum_strips.rs"]
mod strips;
#[path = "tympanum_voice.rs"]
mod voice; // r40:drums-hyb — Generated · Hybrid

pub use strips::plan_rect;

use super::inst_mixer::ui as iu;
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::{with_brush, Function, State as KitState, Style};
use cantus_core::inst::tympanum::{MicPattern, RoomChoice};
use cantus_core::inst::tympanum_kit::KitDef;
use cantus_core::{NativeSpec, TrackId};
use egui::{pos2, Pos2, Rect, Sense, Vec2};

/// A list the view has open.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Open {
    Floor,
    Walls,
    Rug,
    /// A strip's Out cell, by output.
    Out(u16),
    Mix,
}

/// What the views keep for one Tympanum that is not the song.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    /// The selected piece (`p{n}`).
    pub piece: u16,
    /// The selected mic (`o{n}`): the Mixer's band and the ringed strip.
    pub mic: u16,
    pub show_mics: bool,
    /// The kits list's filter: 0 all, 1 recorded, 2 modelled.
    pub filter: u8,
    /// The kits list's scroll, points.
    pub list_scroll: f32,
    /// The mixer's middle, scrolled, points.
    pub strips_scroll: f32,
    /// The piece's panel, scrolled, points.
    pub piece_scroll: f32,
    /// The mic card shows every mic's matrix instead of what the selected one hears.
    pub matrix: bool,
    /// Where the selected piece's tag stands, as a share of the picture (the last click).
    pub tag: Option<Vec2>,
    /// The key held down by the picture, the row, the panel or *Hear*.
    pub held: Option<u8>,
    /// The strength the next hit plays at (dragged on a piece), 1…127.
    pub strength: u8,
    /// The last hit: its piece, strength and where on the head (−1…1 across the head).
    pub hit: Option<(u16, u8, Vec2)>,
    /// The articulation Alt-click plays next.
    pub turn: usize,
    /// The key last played (the hat's opening the pedal card shows).
    pub last_key: u8,
    pub open: Option<Open>,
    /// The pass a list opened on (its opening click is not a click outside it).
    pub opened: u64,
    /// The room card is open over the picture (its chip clicked; round 40 polish). The
    /// view's state only, never the song's: the picture shows the kit whole by default.
    pub room_open: bool,
    /// The pass the room card opened on (its opening click is not a click outside it).
    pub room_opened: u64,
    /// The families folded in the mixer.
    pub folded: Vec<cantus_core::OutputFamily>,
    /// What the last one-click route did, in words.
    pub said: Option<String>,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            piece: 1,
            mic: 3,
            show_mics: false,
            filter: 0,
            list_scroll: 0.0,
            strips_scroll: 0.0,
            piece_scroll: 0.0,
            matrix: false,
            tag: None,
            held: None,
            strength: 96,
            hit: None,
            turn: 0,
            last_key: 0,
            open: None,
            opened: 0,
            room_open: false,
            room_opened: 0,
            folded: Vec::new(),
            said: None,
        }
    }
}

fn look_id(track: TrackId) -> egui::Id {
    egui::Id::new(("tympanum-look", track.0))
}

/// The views' state for `track` (a copy).
pub fn look(ctx: &egui::Context, track: TrackId) -> Look {
    ctx.data(|d| d.get_temp::<Look>(look_id(track)))
        .unwrap_or_default()
}

/// Keep `lk` as `track`'s state.
pub fn keep(ctx: &egui::Context, track: TrackId, lk: Look) {
    ctx.data_mut(|d| d.insert_temp(look_id(track), lk));
}

/// The least size the Kit view lays itself out in at 100 % (below it, it scrolls: nothing
/// crushed, no word cut).
pub const KIT_MIN: Vec2 = Vec2::new(1180.0, 640.0);
/// The Mixer view's least width at 100 %.
pub const MIXER_MIN_W: f32 = 1200.0;
/// The Mixer view's least height at 100 %.
pub const MIXER_MIN_H: f32 = 560.0;

/// Run `f` in a child of `ui` at least `min` big, scrolling where `ui` is smaller.
fn at_least(ui: &mut egui::Ui, salt: (&str, u64), min: Vec2, f: impl FnOnce(&mut egui::Ui)) {
    let room = ui.max_rect();
    if room.width() >= min.x && room.height() >= min.y {
        f(ui);
        return;
    }
    egui::ScrollArea::both()
        .id_salt(salt)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let size = Vec2::new(room.width().max(min.x), room.height().max(min.y));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            f(&mut child);
        });
}

/// The Kit view.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    at_least(ui, ("tymp-kit", track.0), KIT_MIN * s.k, |ui| {
        kit_view(app, ui, track, spec, s)
    });
}

fn kit_view(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let mut lk = look(ui.ctx(), track);
    let def = def::def_of(spec);
    let area = ui.max_rect().shrink(s.s(iu::GAP));
    let g = s.s(iu::GAP);
    let left_w = (area.width() * 0.18).clamp(s.s(222.0), s.s(240.0));
    let right_w = (area.width() * 0.21).clamp(s.s(240.0), s.s(280.0));
    let left = Rect::from_min_max(area.min, pos2(area.left() + left_w, area.bottom()));
    let right = Rect::from_min_max(pos2(area.right() - right_w, area.top()), area.max);
    let mid = Rect::from_min_max(
        pos2(left.right() + g, area.top()),
        pos2(right.left() - g, area.bottom()),
    );
    let cards_h = s.s(cards::HEIGHT);
    let row_h = s.s(cards::ROW);
    let pic = Rect::from_min_max(
        mid.min,
        pos2(mid.right(), mid.bottom() - cards_h - row_h - 2.0 * g),
    );
    let row = Rect::from_min_max(
        pos2(mid.left(), pic.bottom() + g),
        pos2(mid.right(), pic.bottom() + g + row_h),
    );
    let low = Rect::from_min_max(pos2(mid.left(), row.bottom() + g), mid.max);
    kits::list(app, ui, track, spec, &def, s, left, &mut lk);
    stage::picture(app, ui, track, spec, &def, s, pic, &mut lk);
    cards::pieces_row(app, ui, track, &def, s, row, &mut lk);
    cards::cards(app, ui, track, spec, &def, s, low, &mut lk);
    piece::panel(app, ui, track, spec, &def, s, right, &mut lk);
    release_when_up(app, ui, track, &mut lk);
    keep(ui.ctx(), track, lk);
}

/// The frame's Edit view: Tympanum has none of its own (the design's tabs are Kit · Mixer ·
/// Matrix) — the frame offers no Edit for it and shows an Edit carried from another
/// instrument as the Kit, so this draws the Kit too (`body_whole`, a test's direct call).
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    play(app, ui, track, spec, s);
}

/// The Mixer view (the frame's `View::Mixer`).
pub fn mixer(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let min = Vec2::new(MIXER_MIN_W * s.k, MIXER_MIN_H * s.k);
    at_least(ui, ("tymp-mixer", track.0), min, |ui| {
        mixer_view(app, ui, track, spec, s)
    });
}

fn mixer_view(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
) {
    let mut lk = look(ui.ctx(), track);
    let def = def::def_of(spec);
    let area = ui.max_rect().shrink(s.s(iu::GAP));
    let g = s.s(iu::GAP);
    let band = Rect::from_min_max(area.min, pos2(area.right(), area.top() + s.s(mics::BAND)));
    let acts = Rect::from_min_max(
        pos2(area.left(), band.bottom() + g),
        pos2(area.right(), band.bottom() + g + s.s(strips::ACTIONS)),
    );
    let desk = Rect::from_min_max(pos2(area.left(), acts.bottom() + g), area.max);
    mics::band(app, ui, track, spec, &def, s, band, &mut lk);
    strips::actions(app, ui, track, spec, &def, s, acts, &mut lk);
    strips::desk(app, ui, track, spec, &def, s, desk, &mut lk);
    release_when_up(app, ui, track, &mut lk);
    keep(ui.ctx(), track, lk);
}

/// Play `key` now (a press on the picture, a row's tile, an articulation, *Hear*): one held
/// note at a time, let go when the pointer is.
pub fn press(app: &mut CantusApp, track: TrackId, lk: &mut Look, key: u8, piece: u16) {
    if let Some(k) = lk.held.take() {
        app.engine.note_off(track, k);
    }
    let vel = lk.strength.clamp(1, 127);
    app.engine.note_on(track, key, vel);
    lk.held = Some(key);
    lk.last_key = key;
    let at = lk.hit.filter(|h| h.0 == piece).map_or(Vec2::ZERO, |h| h.2);
    lk.hit = Some((piece, vel, at));
}

fn release_when_up(app: &mut CantusApp, ui: &egui::Ui, track: TrackId, lk: &mut Look) {
    if lk.held.is_some() && !ui.input(|i| i.pointer.any_down()) {
        if let Some(k) = lk.held.take() {
            app.engine.note_off(track, k);
        }
    }
}

/// The header's words (the approved design): the track's name as it is, and the preset well
/// naming the kit, then the mix's preset when one was chosen — *Studio Maple · Tight rock*.
pub fn header_words(
    spec: &NativeSpec,
    track_name: &str,
    preset: Option<&String>,
) -> (String, String) {
    let kit = def::kit_name(&def::def_of(spec));
    let well = match preset {
        Some(p) if !p.is_empty() && *p != kit => format!("{kit} \u{b7} {p}"),
        _ => kit,
    };
    (track_name.to_string(), well)
}

/// The kit as drawn: the definition with the mics' moves (`m{n}.*`) applied.
pub fn moved(def: &KitDef, spec: &NativeSpec) -> KitDef {
    let mut d = def.clone();
    for m in &mut d.mics {
        let n = m.output.0;
        let v = |f: &str| spec.get(&format!("m{n}.{f}"));
        m.place.x += v("x");
        m.place.y += v("y");
        m.place.z += v("z");
        m.place.azimuth += v("az");
        m.place.elevation += v("el");
        let p = v("pattern").round() as usize;
        if p >= 1 {
            m.pattern = MicPattern::ALL[(p - 1).min(MicPattern::ALL.len() - 1)];
        }
    }
    d
}

/// The room the kit plays in now (`room`; 0 the kit's own).
pub fn room_of(spec: &NativeSpec) -> RoomChoice {
    RoomChoice::ALL[(spec.get("room").round().max(0.0) as usize).min(3)]
}

/// The inspector's small picture: the kit's name on a well.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(Function::Source);
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(36.0)), Sense::hover());
    let def = def::def_of(spec);
    let name = def::kit_name(&def);
    with_brush(&ui.painter_at(rect), |b| {
        crate::kit::well::paint(b, rect, &s, KitState::REST, None);
        let p = iu::pal(&s);
        let r = rect.shrink2(egui::vec2(s.s(8.0), 0.0));
        iu::words(
            b,
            &s,
            &name,
            r,
            egui::Align2::LEFT_CENTER,
            12.0,
            crate::views::mixer::look::W::Medium,
            p.ink1,
        );
    });
    resp
}

/// `--editor tympanum`: the default modelled kit, the snare selected.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    let _ = track;
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine: voices, and the mics' meters
/// ([`strips::METER_TEL`]) as a groove would leave them.
pub fn demo_feed(app: &mut CantusApp) {
    let mut tel = [0.0; cantus_engine::TELEMETRY_LEN];
    tel[super::TEL_VOICES] = 6.0;
    tel[super::TEL_POLYPHONY] = 64.0;
    strips::demo_levels(&mut tel);
    app.inst.injected = Some(tel);
}

/// Factory presets: the kits' mixes (`tymp-ui` and `tymp-engine`).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// Where a point of the picture is as a share of it (for the tag that follows a click).
fn share(r: Rect, p: Pos2) -> Vec2 {
    Vec2::new(
        (p.x - r.left()) / r.width().max(1.0),
        (p.y - r.top()) / r.height().max(1.0),
    )
}

/// The pointer went down on `resp` this frame: a piece plays on the press, as a pad does.
pub fn pressed(resp: &egui::Response) -> bool {
    resp.is_pointer_button_down_on() && resp.ctx.input(|i| i.pointer.primary_pressed())
}
