//! Chorda's Mixer view (round 40 phase 3, ADR 0123 and 0125; the approved design's
//! *Chorda · Mixer* and *Chorda · Mixer · pack* sheets) on `tymp-ui`'s shared instrument
//! desk (`super::super::inst_mixer`): the piano in its room beside the selected output's
//! card; *Outputs* — *All to Main* · *Each mic to its own strip*, what they leave in words,
//! the mix's preset; then the strips — *Piano* (all the mics) pinned left, the outputs under
//! their families' heads (a recorded pack's own mics by their names, images not on disk as
//! placeholders that offer their download, images off unloaded, the noises on *Releases*;
//! the model's *Hammers, Player, Audience, Mechanics*), *Room* pinned right.
//!
//! Every edit is a command on Chorda's keys (`o{n}.*`, `gain`/`rec.gain`, `room`,
//! `SetInstrumentOutput`), so undo, automation and presets reach the mix as any other number.

use super::strip as cstrip;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::{with_brush, Style};
use crate::scene::{Room, SceneView, Subject};
use crate::views::inst::inst_mixer::ui::{self as iu};
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::chorda_look;
use cantus_core::inst::chorda_source::{self as cs, ChordaSource};
use cantus_core::inst::room_look::{self, RoomKind};
use cantus_core::{NativeSpec, OutputFamily, TrackId};
use egui::{pos2, vec2, Align2, Rect, Vec2};

/// The least the view lays out in at 100 % (below it, it scrolls).
pub const MIN: Vec2 = Vec2::new(1100.0, 600.0);
/// The band of the picture and the card, at 100 %.
pub const BAND: f32 = 172.0;
/// The picture's width at 100 %.
pub const PICTURE_W: f32 = 456.0;
/// The actions row's height at 100 %.
pub const ACTIONS: f32 = 36.0;
/// Where the Mixer's picture is kept in `app.inst.tympanum_scenes` (the scene module's
/// caches by track and place; Tympanum's views use 0 and 1).
const SCENE_PLACE: u8 = 7;

/// What the view remembers between frames, per track: the selected output, the folded
/// families, the strips' scroll, the Out list open.
#[derive(Clone, Debug, Default)]
pub struct MixLook {
    pub selected: Option<u16>,
    pub folded: Vec<OutputFamily>,
    pub scroll: f32,
    /// The output whose Out list is open, the cell it opened from, the pass it opened on.
    pub open: Option<(u16, Rect, u64)>,
}

fn look_id(track: TrackId) -> egui::Id {
    egui::Id::new(("chorda-mix-look", track.0))
}

/// The view's memory for `track`.
pub fn look(ctx: &egui::Context, track: TrackId) -> MixLook {
    ctx.data(|d| d.get_temp::<MixLook>(look_id(track)))
        .unwrap_or_default()
}

fn keep(ctx: &egui::Context, track: TrackId, lk: MixLook) {
    ctx.data_mut(|d| d.insert_temp(look_id(track), lk));
}

/// The Mixer view.
pub fn mixer(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let room = ui.max_rect();
    let min = MIN * s.k;
    if room.width() >= min.x && room.height() >= min.y {
        view(app, ui, track, spec, s);
        return;
    }
    egui::ScrollArea::both()
        .id_salt(("chorda-mixer", track.0))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let size = vec2(room.width().max(min.x), room.height().max(min.y));
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            view(app, &mut child, track, spec, s);
        });
}

fn view(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let mut lk = look(ui.ctx(), track);
    let g = s.s(iu::GAP);
    let area = ui.max_rect().shrink(g);
    let band = Rect::from_min_max(area.min, pos2(area.right(), area.top() + s.s(BAND)));
    let pic = Rect::from_min_max(band.min, pos2(band.left() + s.s(PICTURE_W), band.bottom()));
    let card = Rect::from_min_max(pos2(pic.right() + g, band.top()), band.max);
    let acts = Rect::from_min_max(
        pos2(area.left(), band.bottom() + g),
        pos2(area.right(), band.bottom() + g + s.s(ACTIONS)),
    );
    let desk = Rect::from_min_max(pos2(area.left(), acts.bottom() + g), area.max);
    let outs = cstrip::outs(app, spec);
    if !lk
        .selected
        .is_some_and(|n| outs.iter().any(|o| o.id.0 == n))
    {
        lk.selected = outs.first().map(|x| x.id.0);
    }
    picture(app, ui, track, spec, s, pic);
    super::card::card(app, ui, track, spec, s, card, &outs, &lk);
    super::card::actions(app, ui, track, spec, s, acts, &outs);
    cstrip::desk(app, ui, track, spec, s, desk, &outs, &mut lk);
    keep(ui.ctx(), track, lk);
}

/// The scene a Chorda's Mixer shows: the piano its source plays (a recorded piano's body,
/// else the stored look) in its room; the model's microphones drawn.
pub fn scene_view(spec: &NativeSpec, s: &Style) -> SceneView {
    let mut lk = chorda_look::of(spec);
    if let Some(body) = cs::scene_piano(&ChordaSource::of(spec)) {
        lk.body = body;
    }
    let rl = room_look::of(spec);
    let room = match rl.room {
        Some(RoomKind::Stage) => Room::Stage,
        Some(RoomKind::Booth) => Room::Booth,
        Some(RoomKind::Studio) => Room::Studio,
        Some(RoomKind::Hall) => Room::Hall,
        Some(RoomKind::Church) => Room::Church,
        None => Room::of_size(spec.get("roomsize")),
    };
    let p = Pal::of(s);
    SceneView {
        subject: Subject::Piano {
            look: lk,
            lid: spec.get("lid"),
            persp: spec.get("persp"),
            width: spec.get("width"),
            room_level: spec.get("room"),
            mics: !cs::has_zones(spec),
        },
        room,
        look: rl,
        dark: s.t.dark,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: p.accent,
        ink: p.ink1,
    }
}

/// The piano in its room, and a tag at its foot: where the perspective stands (the model)
/// or that the pack's microphones are named below (a recorded piano).
fn picture(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| iu::card(b, s, r));
    let d = r.shrink(s.s(1.0));
    let view = scene_view(spec, s);
    if !ui.is_sizing_pass() {
        let painter = ui.painter_at(d);
        let scene = app
            .inst
            .tympanum_scenes
            .entry((track.0, SCENE_PLACE))
            .or_default();
        if scene.paint(&painter, d, &view) {
            ui.ctx().request_repaint();
        }
    }
    let said = if cs::has_zones(spec) {
        t("chorda_source.mixer.picture_rec")
    } else {
        let persp = (spec.get("persp") * 100.0).round() as i32;
        let at = match spec.get("persp") {
            x if x < 0.18 => t("chorda_source.at.hammers"),
            x if x < 0.6 => t("chorda_source.at.player"),
            _ => t("chorda_source.at.audience"),
        };
        tf(
            "chorda_source.mixer.picture_model",
            &[("p", &persp), ("at", &at)],
        )
    };
    let w = iu::width(ui, s, &said, 11.0, W::Regular) + s.s(34.0);
    let tag = Rect::from_min_size(
        pos2(d.left() + s.s(8.0), d.bottom() - s.s(30.0)),
        vec2(w.min(d.width() - s.s(16.0)), s.s(22.0)),
    );
    with_brush(&ui.painter_at(d), |b| {
        iu::veil(b, s, tag);
        iu::mic_glyph(
            b,
            s,
            pos2(tag.left() + s.s(13.0), tag.center().y),
            s.s(11.0),
            p.ink2,
        );
        let tr = Rect::from_min_max(pos2(tag.left() + s.s(24.0), tag.top()), tag.max);
        iu::words(
            b,
            s,
            &said,
            tr,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink2,
        );
    });
}
