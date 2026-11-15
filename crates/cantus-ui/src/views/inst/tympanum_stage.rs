//! The kit in its room (ADR 0124 §4, ADR 0126): the scene module draws it and says what is
//! under the pointer (`crate::scene::paint`, `pick`); this view lays the picture out, plays
//! and selects what is clicked, and puts over it the tags (the piece under the pointer, the
//! selected one), *Show mics*, the kit's own tag and the room card (Room, Floor, Walls, Rug).
//!
//! **Until the scene's port landed** its stub drew nothing and picked nothing: the view then
//! drew the kit from above — each piece where it stands, its size to scale, the selected
//! one ringed — and picked from that plan. [`PLAN_VIEW`] is off now that `scene` draws the
//! kit (the integrator, round 40 phase 3); the plan stays behind it.

use super::iu::{self, words};
use super::{def, press, share, Look};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, with_alpha};
use crate::kit::{with_brush, Brush, Fill, Style};
use crate::scene::{self, Picked, Room, SceneView, Subject};
use crate::views::inst::room_card::{self, LookPart};
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::room_look::{self, RoomKind};
use cantus_core::inst::tympanum::RoomChoice;
use cantus_core::inst::tympanum_kit::{KitDef, PieceDef};
use cantus_core::{Command, NativeSpec, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

/// Draw the kit from above (the stand-in while the scene module's stub drew nothing, see the
/// module's note); off since `scene` is merged.
pub const PLAN_VIEW: bool = false;

/// Tympanum's rooms in the card's order. The choice is both what is seen and what is heard,
/// one undo step (the owner's decision, 2026-11-12): *Stage* draws the stage and plays the
/// kit's own room (a recorded kit as recorded, a modelled kit its dry stage, `room` 0);
/// *Booth*, *Studio*, *Hall* draw those rooms and put Aula's rooms on the room mics
/// (`room` 1, 2, 3).
pub const ROOMS: [RoomKind; 4] = [
    RoomKind::Stage,
    RoomKind::Booth,
    RoomKind::Studio,
    RoomKind::Hall,
];

/// The acoustic room (`room`) a room plays.
pub fn acoustic(k: RoomKind) -> f32 {
    match k {
        RoomKind::Booth => 1.0,
        RoomKind::Studio => 2.0,
        RoomKind::Hall => 3.0,
        RoomKind::Stage | RoomKind::Church => 0.0,
    }
}

/// The room `spec` stands in: its look's (`look.room`), else the one its acoustic room
/// draws (an instance from before the look: 0 the stage).
pub fn room_kind(spec: &NativeSpec) -> RoomKind {
    room_look::of(spec)
        .room
        .unwrap_or(match super::room_of(spec) {
            RoomChoice::Own => RoomKind::Stage,
            RoomChoice::Small => RoomKind::Booth,
            RoomChoice::Studio => RoomKind::Studio,
            RoomChoice::Hall => RoomKind::Hall,
        })
}

/// The scene's room for a room.
pub fn scene_room(k: RoomKind) -> Room {
    match k {
        RoomKind::Stage => Room::Stage,
        RoomKind::Booth => Room::Booth,
        RoomKind::Studio => Room::Studio,
        RoomKind::Hall => Room::Hall,
        RoomKind::Church => Room::Church,
    }
}

/// Put `track`'s kit in room `k`: the picture (`look.room`) and the sound (`room`) in one
/// undo step.
pub fn choose_room(app: &mut CantusApp, track: TrackId, k: RoomKind) {
    app.exec(Command::Batch {
        label_key: "cmd.tympanum_room".into(),
        commands: vec![
            Command::SetInstrumentText {
                track,
                key: room_look::ROOM_KEY.into(),
                text: Some(k.key().into()),
            },
            Command::SetInstrumentValue {
                track,
                key: "room".into(),
                value: acoustic(k),
            },
        ],
    });
}

/// A room's name: the segment's word.
pub fn room_name_key(k: RoomKind) -> &'static str {
    match k {
        RoomKind::Stage => "tympanum.room.stage",
        RoomKind::Booth => "tympanum.room.booth",
        RoomKind::Studio => "tympanum.room.studio",
        RoomKind::Hall => "tympanum.room.hall",
        RoomKind::Church => "tympanum.room.church",
    }
}

/// What the room card's line says of a room: what is heard in it.
fn room_meta(def: &KitDef, k: RoomKind) -> String {
    t(match k {
        RoomKind::Stage if def.modelled => "tympanum.room.stage_meta_modelled",
        RoomKind::Stage | RoomKind::Church => "tympanum.room.stage_meta_recorded",
        RoomKind::Booth => "tympanum.room.booth_meta",
        RoomKind::Studio => "tympanum.room.studio_meta",
        RoomKind::Hall => "tympanum.room.hall_meta",
    })
}

/// The scene to draw for `spec`'s kit, with what is lit, in `s`'s theme.
pub fn view(
    def: &KitDef,
    spec: &NativeSpec,
    lk: &Look,
    mics: bool,
    sel: Option<Picked>,
    s: &Style,
) -> SceneView {
    let kit = super::moved(def, spec);
    let p = Pal::of(s);
    SceneView {
        subject: Subject::Kit {
            kit: def.kit.clone(),
            def: Box::new(kit),
            // r40:integration — the shells' finish: the studio kit's maple until a kit
            // says its own (`KitDef` has none yet).
            finish: cantus_core::inst::chorda_look::Finish::Maple,
            mics,
        },
        room: scene_room(room_kind(spec)),
        // The floor, walls and rug: the scene's text-keyed look (schema 69).
        look: room_look::of(spec),
        dark: s.t.dark,
        selected: sel,
        hovered: None,
        sounding: lk
            .hit
            .filter(|_| lk.held.is_some())
            .map(|h| vec![h.0])
            .unwrap_or_default(),
        accent: p.accent,
        ink: p.ink1,
    }
}

/// `track`'s scene at `place` (0 the Kit view's picture, 1 the Mixer view's band).
pub fn scene_mut(app: &mut CantusApp, track: TrackId, place: u8) -> &mut scene::Scene {
    app.inst
        .tympanum_scenes
        .entry((track.0, place))
        .or_default()
}

/// What is under `q` in the scene shown at `place`: from its own build once it has one
/// (nothing rebuilt per frame), else from a build of `v` made here.
#[allow(clippy::too_many_arguments)]
pub fn pick_shown(
    app: &CantusApp,
    track: TrackId,
    place: u8,
    r: Rect,
    v: &SceneView,
    def: &KitDef,
    q: Pos2,
) -> Option<Picked> {
    match app.inst.tympanum_scenes.get(&(track.0, place)) {
        Some(sc) if sc.built().is_some() => sc.pick(r, q),
        _ => pick_at(r, v, def, q),
    }
}

fn picture_id(track: TrackId) -> egui::Id {
    egui::Id::new(("tymp-picture-rect", track.0))
}

/// Where `track`'s picture was drawn last frame (tests aim at it).
pub fn picture_rect(ctx: &egui::Context, track: TrackId) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Rect>(picture_id(track)))
}

/// A point on piece `piece` in the picture `r` showing `v`: of the points the scene's pick
/// finds it at — its centre's zone when it shows one — the one nearest their middle (the
/// plan's while it stands in). For tests.
pub fn piece_point(r: Rect, v: &SceneView, def: &KitDef, piece: u16) -> Option<Pos2> {
    if !PLAN_VIEW {
        let w = scene::Want::of(v, r.size(), 1.0);
        let b = scene::build(&w);
        let all: Vec<(Pos2, Option<u8>)> = (0..96)
            .flat_map(|j| (0..160).map(move |i| (i, j)))
            .map(|(i, j)| {
                pos2(
                    r.left() + (i as f32 + 0.5) / 160.0 * r.width(),
                    r.top() + (j as f32 + 0.5) / 96.0 * r.height(),
                )
            })
            .filter_map(|q| match scene::pick_in(&w, &b, (q - r.min).to_pos2()) {
                Some(Picked::Piece { piece: p, zone }) if p == piece => Some((q, zone)),
                _ => None,
            })
            .collect();
        let centre: Vec<Pos2> = all
            .iter()
            .filter(|(_, z)| *z == Some(0))
            .map(|(q, _)| *q)
            .collect();
        let hits = if centre.is_empty() {
            all.iter().map(|(q, _)| *q).collect()
        } else {
            centre
        };
        let n = hits.len().max(1) as f32;
        let mid = hits.iter().fold(Pos2::ZERO, |a, q| a + q.to_vec2()) / n;
        return hits
            .iter()
            .copied()
            .min_by(|a, b| a.distance_sq(mid).total_cmp(&b.distance_sq(mid)));
    }
    let f = plan_xf(r, def);
    let k = plan_scale(r, def);
    let pd = def.pieces.iter().find(|p| p.index == piece)?;
    let c = f(pd.place.x, pd.place.y);
    let rr = pd.diameter * 0.5 * k;
    // Its centre, else the first point of it no piece above covers.
    (0..=8)
        .flat_map(|ring| (0..16).map(move |a| (ring, a)))
        .map(|(ring, a)| {
            let d = rr * 0.1 * f32::from(ring as u8);
            let t = a as f32 / 16.0 * std::f32::consts::TAU;
            c + vec2(cantus_dsp::math::cos(t), cantus_dsp::math::sin(t) * 0.82) * d
        })
        .find(|q| plan_pick(r, def, *q) == Some(piece))
}

/// What is under `q`: the scene's pick, else (while the plan stands in) the plan's.
pub fn pick_at(r: Rect, v: &SceneView, def: &KitDef, q: Pos2) -> Option<Picked> {
    scene::pick(r, v, q).or_else(|| {
        PLAN_VIEW
            .then(|| plan_pick(r, def, q).map(|piece| Picked::Piece { piece, zone: None }))
            .flatten()
    })
}

/// The picture: the scene, the click that plays and selects, the tags and the cards over it.
#[allow(clippy::too_many_arguments)]
pub fn picture(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
) {
    ui.ctx().data_mut(|d| d.insert_temp(picture_id(track), r));
    let sel = Some(Picked::Piece {
        piece: lk.piece,
        zone: None,
    });
    let mut v = view(def, spec, lk, lk.show_mics, sel, s);
    let resp = ui.interact(
        r,
        ui.id().with(("tymp-picture", track.0)),
        Sense::click_and_drag(),
    );
    let under = resp
        .hover_pos()
        .and_then(|q| pick_shown(app, track, 0, r, &v, def, q));
    v.hovered = under;
    let resp = iu::hint(resp, || match under {
        Some(Picked::Mic(id)) => {
            let name = def
                .mics
                .iter()
                .find(|m| m.output == id)
                .map(|m| def::mic_name(def, m))
                .unwrap_or_default();
            tf("tympanum.mic_tip", &[("name", &name)])
        }
        _ => t("tympanum.picture_tip"),
    });
    let alt = ui.input(|i| i.modifiers.alt);
    if super::pressed(&resp) {
        if let Some(q) = resp.interact_pointer_pos() {
            match pick_shown(app, track, 0, r, &v, def, q) {
                Some(Picked::Piece { piece, zone }) => {
                    if let Some(pd) = def.pieces.iter().find(|p| p.index == piece) {
                        hit(app, track, lk, pd, zone, alt, r, q);
                    }
                }
                Some(Picked::Mic(id)) => {
                    lk.mic = id.0;
                    app.inst.view = super::super::View::Mixer;
                }
                _ => {}
            }
        }
    }
    // Dragging on a piece sets the next hit's strength (up: harder).
    if resp.dragged() && lk.held.is_some() {
        let dy = resp.drag_delta().y;
        lk.strength = (f32::from(lk.strength) - dy * 0.5).clamp(1.0, 127.0) as u8;
        if let Some(h) = lk.hit.as_mut() {
            h.1 = lk.strength;
        }
    }
    let moving = with_brush(ui.painter(), |b| {
        let p = Pal::of(s);
        b.rrect(b.snap_rect(r), s.s(6.0), Fill::Solid(p.well));
        false
    });
    let painter = ui.painter_at(r);
    let moving = scene_mut(app, track, 0).paint(&painter, r, &v) || moving;
    if PLAN_VIEW {
        with_brush(&painter, |b| plan(b, s, r, def, lk, under));
    }
    if moving {
        crate::views::workspace_ticker::moving(
            ui.ctx(),
            crate::views::workspace_ticker::Reason::Live,
        );
    }
    overlays(app, ui, track, spec, def, s, r, lk, under);
}

#[allow(clippy::too_many_arguments)]
fn hit(
    app: &mut CantusApp,
    track: TrackId,
    lk: &mut Look,
    pd: &PieceDef,
    zone: Option<u8>,
    alt: bool,
    r: Rect,
    q: Pos2,
) {
    let n = pd.articulations.len().max(1);
    let a = if alt {
        let i = if lk.piece == pd.index { lk.turn % n } else { 0 };
        lk.turn = i + 1;
        i
    } else {
        usize::from(zone.unwrap_or(0)).min(n - 1)
    };
    lk.piece = pd.index;
    lk.tag = Some(share(r, q));
    if let Some(art) = pd.articulations.get(a) {
        press(app, track, lk, art.key, pd.index);
    }
}

/// The tags, *Show mics*, the kit's tag and the room card, over the picture.
#[allow(clippy::too_many_arguments)]
fn overlays(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
    under: Option<Picked>,
) {
    let p = Pal::of(s);
    let pad = s.s(12.0);
    // The selected piece's tag where it was last clicked (else under the picture's middle),
    // the hovered one's by the pointer.
    let sel = def.pieces.iter().find(|x| x.index == lk.piece);
    with_brush(&ui.painter_at(r), |b| {
        if let Some(pd) = sel {
            let at = match lk.tag {
                Some(v) => pos2(r.left() + v.x * r.width(), r.top() + v.y * r.height()),
                None => PLAN_VIEW
                    .then(|| {
                        let v = view(def, spec, lk, false, None, s);
                        piece_point(r, &v, def, pd.index)
                    })
                    .flatten()
                    .unwrap_or(pos2(r.center().x, r.center().y)),
            };
            let art = pd
                .articulations
                .first()
                .map(|a| def::said(&a.label))
                .unwrap_or_default();
            let key = pd
                .articulations
                .first()
                .map(|a| crate::controllers::note_name(a.key))
                .unwrap_or_default();
            let detail = format!("{art} \u{b7} {key}");
            iu::tag(
                b,
                ui,
                s,
                at + vec2(s.s(14.0), s.s(16.0)),
                r,
                &def::piece_name(pd),
                &detail,
                true,
            );
        }
        if let (Some(Picked::Piece { piece, .. }), Some(q)) = (under, ui.ctx().pointer_hover_pos())
        {
            if piece != lk.piece {
                if let Some(pd) = def.pieces.iter().find(|x| x.index == piece) {
                    let key = pd
                        .articulations
                        .first()
                        .map(|a| crate::controllers::note_name(a.key))
                        .unwrap_or_default();
                    let name = format!("{} \u{b7} {key}", def::piece_name(pd));
                    iu::tag(
                        b,
                        ui,
                        s,
                        q + vec2(-s.s(30.0), -s.s(26.0)),
                        r,
                        &name,
                        "",
                        false,
                    );
                }
            }
        }
    });
    // Show mics, top right.
    let label = t(if lk.show_mics {
        "tympanum.hide_mics"
    } else {
        "tympanum.show_mics"
    });
    let bw = iu::width(ui, s, &label, 12.0, W::Medium) + s.s(40.0);
    let br = Rect::from_min_size(
        pos2(r.right() - pad - bw, r.top() + pad),
        vec2(bw, s.s(28.0)),
    );
    let resp = iu::button(
        ui,
        s,
        br,
        ui.id().with(("tymp-show-mics", track.0)),
        &label,
        iu::Lead::Mic,
        lk.show_mics,
        true,
    );
    if resp.clicked() {
        lk.show_mics = !lk.show_mics;
    }
    // The kit's tag, bottom left.
    let name = def::kit_name(def);
    let about = {
        let mut a = String::new();
        if let Some(tag) = def::kit_tag(def) {
            a.push_str(&tag);
            a.push_str(" \u{b7} ");
        }
        a.push_str(&tf(
            "tympanum.tag_about",
            &[("pieces", &def.pieces.len()), ("mics", &def.mics.len())],
        ));
        a
    };
    let badge = t(if def.modelled {
        "tympanum.modelled"
    } else {
        "tympanum.recorded"
    });
    let tw = (iu::width(ui, s, &name, 15.0, W::SemiBold)
        + s.s(8.0)
        + iu::width(ui, s, &badge, 10.0, W::Medium)
        + s.s(12.0))
    .max(iu::width(ui, s, &about, 11.0, W::Regular));
    let card_w = s.s(32.0) + tw;
    let tag_r = Rect::from_min_size(
        pos2(r.left() + pad, r.bottom() - pad - s.s(54.0)),
        vec2(card_w, s.s(54.0)),
    );
    // The room card in the room the tag leaves (its looks on two rows if need be); when
    // even that is too narrow, the room card wins and the tag steps aside.
    let beside = r.right() - pad - (tag_r.right() + s.s(12.0));
    let cw = CardWords::of(spec, def);
    let card = cw.card();
    let sz = room_card::sizes(ui, s, &card);
    let mut layout = room_card::layout(&sz, s, beside);
    let both = layout.0 <= beside;
    if !both {
        layout = room_card::layout(&sz, s, r.width() - 2.0 * pad);
    }
    if both {
        with_brush(ui.painter(), |b| {
            iu::veil(b, s, tag_r);
            let nr = Rect::from_min_size(
                pos2(tag_r.left() + s.s(14.0), tag_r.top() + s.s(8.0)),
                vec2(tw, s.s(20.0)),
            );
            let w = words(
                b,
                s,
                &name,
                nr,
                Align2::LEFT_CENTER,
                15.0,
                W::SemiBold,
                p.ink1,
            );
            iu::badge(b, ui, s, nr.left() + w + s.s(10.0), nr.center().y, &badge);
            let ar = Rect::from_min_size(
                pos2(nr.left(), nr.bottom() + s.s(2.0)),
                vec2(tw + 1.0, s.s(16.0)),
            );
            words(
                b,
                s,
                &about,
                ar,
                Align2::LEFT_CENTER,
                11.0,
                W::Regular,
                p.ink2,
            );
        });
    }
    room_card(app, ui, track, s, r, &card, &sz, layout, lk);
    let _ = with_alpha(p.base, 0.0);
}

/// The room card's words for `spec`'s kit (the shared card, [`room_card`]): *Room*, the
/// four rooms, what the one shown is heard as, the floor, the walls and the rug.
pub struct CardWords {
    label: String,
    names: Vec<String>,
    now: RoomKind,
    meta: String,
    look_labels: [String; 3],
    looks: [String; 3],
    room_tip: String,
    look_tip: String,
}

impl CardWords {
    pub fn of(spec: &NativeSpec, def: &KitDef) -> CardWords {
        let now = room_kind(spec);
        let room = scene_room(now);
        CardWords {
            label: t("tympanum.room.label"),
            names: ROOMS.iter().map(|k| t(room_name_key(*k))).collect(),
            now,
            meta: room_meta(def, now),
            look_labels: LookPart::ALL.map(|p| t(p.label())),
            looks: LookPart::ALL.map(|p| t(p.now(spec, room).1)),
            room_tip: t("tympanum.room.tip"),
            look_tip: t("tympanum.room.look_tip"),
        }
    }

    pub fn card(&self) -> room_card::Card<'_> {
        room_card::Card {
            label: &self.label,
            rooms: &ROOMS,
            names: &self.names,
            now: self.now,
            meta: &self.meta,
            look_labels: [
                &self.look_labels[0],
                &self.look_labels[1],
                &self.look_labels[2],
            ],
            looks: [&self.looks[0], &self.looks[1], &self.looks[2]],
            room_tip: &self.room_tip,
            look_tip: &self.look_tip,
        }
    }
}

/// The room card, bottom right (the shared card): *Room* and its four rooms (picture and
/// sound together); what the room is heard as; the floor, the walls and the rug (the
/// picture only). Its open list is the view's ([`Look::open`]).
#[allow(clippy::too_many_arguments)]
fn room_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    s: &Style,
    pic: Rect,
    card: &room_card::Card,
    sz: &room_card::Sizes,
    layout: (f32, bool),
    lk: &mut Look,
) {
    let was = match lk.open {
        Some(super::Open::Floor) => Some(LookPart::Floor),
        Some(super::Open::Walls) => Some(LookPart::Walls),
        Some(super::Open::Rug) => Some(LookPart::Rug),
        _ => None,
    };
    let mut open = was;
    let shown = room_card::show(
        ui,
        s,
        ui.id().with(("tymp-room", track.0)),
        pic,
        card,
        sz,
        layout,
        &mut open,
        &mut lk.opened,
    );
    if open != was {
        lk.open = open.map(|p| match p {
            LookPart::Floor => super::Open::Floor,
            LookPart::Walls => super::Open::Walls,
            LookPart::Rug => super::Open::Rug,
        });
    }
    if let Some(k) = shown.room {
        choose_room(app, track, k);
    }
    if let Some((part, word)) = shown.look {
        app.set_inst_text(track, part.key(), Some(word.to_string()));
    }
}

// ---- the plan from above, while the scene's stub draws nothing -------------------------

/// The plan's transform: cm (x right, y toward the audience) to points in `r`.
fn plan_xf(r: Rect, def: &KitDef) -> impl Fn(f32, f32) -> Pos2 {
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for p in &def.pieces {
        let rad = p.diameter * 0.5;
        x0 = x0.min(p.place.x - rad);
        x1 = x1.max(p.place.x + rad);
        y0 = y0.min(p.place.y - rad);
        y1 = y1.max(p.place.y + rad);
    }
    if x0 > x1 {
        (x0, x1, y0, y1) = (-100.0, 100.0, -60.0, 60.0);
    }
    let room = r.shrink2(vec2(r.width() * 0.12, r.height() * 0.16));
    let k = (room.width() / (x1 - x0)).min(room.height() / (y1 - y0));
    let c = room.center();
    let (mx, my) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    move |x, y| pos2(c.x + (x - mx) * k, c.y - (y - my) * k * 0.82)
}

fn plan_scale(r: Rect, def: &KitDef) -> f32 {
    let f = plan_xf(r, def);
    (f(100.0, 0.0).x - f(0.0, 0.0).x) / 100.0
}

/// The piece under `q` in the plan: the highest one whose disc holds it.
fn plan_pick(r: Rect, def: &KitDef, q: Pos2) -> Option<u16> {
    let f = plan_xf(r, def);
    let k = plan_scale(r, def);
    def.pieces
        .iter()
        .filter(|p| {
            let c = f(p.place.x, p.place.y);
            let rr = p.diameter * 0.5 * k;
            let d = q - c;
            (d.x * d.x) / (rr * rr) + (d.y * d.y) / (rr * rr * 0.67) <= 1.0
        })
        .max_by(|a, b| a.place.z.total_cmp(&b.place.z))
        .map(|p| p.index)
}

/// The kit from above: drums as discs (shell and head), cymbals as rings, the selected one
/// ringed in the accent, the one under the pointer in ink; each named under it.
fn plan(b: &mut Brush, s: &Style, r: Rect, def: &KitDef, lk: &Look, under: Option<Picked>) {
    let p = Pal::of(s);
    let f = plan_xf(r, def);
    let k = plan_scale(r, def);
    let mut order: Vec<&PieceDef> = def.pieces.iter().collect();
    // Drums under cymbals; within each, the lower first.
    order.sort_by(|a, b| {
        (def::is_cymbal(a.kind), a.place.z)
            .partial_cmp(&(def::is_cymbal(b.kind), b.place.z))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let hovered = match under {
        Some(Picked::Piece { piece, .. }) => Some(piece),
        _ => None,
    };
    for pd in order {
        let c = f(pd.place.x, pd.place.y);
        let rr = pd.diameter * 0.5 * k;
        let cym = def::is_cymbal(pd.kind);
        let squash = |rad: f32| {
            let n = 40;
            (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    pos2(
                        c.x + rad * cantus_dsp::math::cos(a),
                        c.y + rad * 0.82 * cantus_dsp::math::sin(a),
                    )
                })
                .collect::<Vec<_>>()
        };
        if cym {
            let bronze = mix(p.solo, p.surface, 0.25);
            b.polygon(&squash(rr), with_alpha(bronze, 0.30));
            let mut edge = squash(rr);
            edge.push(edge[0]);
            b.polyline(&edge, s.s(1.2), with_alpha(bronze, 0.85));
            b.polygon(&squash(rr * 0.22), with_alpha(bronze, 0.75));
        } else {
            b.polygon(&squash(rr), mix(p.raised, p.ink1, 0.08));
            b.polygon(&squash(rr * 0.86), mix(p.well, p.ink1, 0.72));
        }
        let lit = lk.piece == pd.index;
        if lit || hovered == Some(pd.index) {
            let mut ring = squash(rr + s.s(2.0));
            ring.push(ring[0]);
            b.polyline(
                &ring,
                s.s(if lit { 2.0 } else { 1.2 }),
                if lit { p.accent } else { p.ink2 },
            );
        }
    }
    // The names over everything, each under its piece.
    for pd in &def.pieces {
        let c = f(pd.place.x, pd.place.y);
        let rr = pd.diameter * 0.5 * k;
        let nr = Rect::from_center_size(
            pos2(c.x, c.y + rr * 0.82 + s.s(9.0)),
            vec2(s.s(90.0), s.s(14.0)),
        );
        words(
            b,
            s,
            &def::piece_name(pd),
            nr,
            Align2::CENTER_CENTER,
            10.0,
            W::Medium,
            p.ink1,
        );
    }
    if lk.show_mics {
        for m in &def.mics {
            let c = f(m.place.x, m.place.y);
            iu::mic_glyph(
                b,
                s,
                c,
                s.s(14.0),
                if lk.mic == m.output.0 {
                    p.accent
                } else {
                    p.ink2
                },
            );
        }
    }
}
