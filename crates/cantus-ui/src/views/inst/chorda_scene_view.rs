//! Chorda's scene in the Play view (round 40 phase 3, ADR 0126; round 37's ADR 0104 card):
//! the piano in its room drawn live by the scene module — the lid clicked open or dragged to
//! its angle, the microphones shown on demand — its pickers (the piano and its finish over
//! the top-left corner; the room, its floor, walls and rug on a plate at the bottom-right:
//! the look, never the sound), the line that says what it shows for the screen reader, and
//! the tuning's progress while one is designed.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Function, State as KitState, Style};
use crate::scene::{self, Picked, Room, SceneView, Subject};
use crate::views::fx::controls;
use crate::views::fx::frame::radio;
use crate::views::inst::organ::parts::{paint_card, Card};
use crate::views::inst::room_card;
use cantus_core::inst::chorda_look::{self as look, Body, Finish};
use cantus_core::inst::room_look::{self, RoomKind};
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// The pickers' widths (points at 100 %).
const BODY_W: f32 = 178.0;
const FINISH_W: f32 = 132.0;
const MICS_W: f32 = 132.0;

/// The room a spec draws: its stored room, or (an older song) the one its Room size drew
/// (round 37's thresholds: a booth, a live room, a hall, a church when big and live).
pub fn room_of(spec: &NativeSpec) -> Room {
    match room_look::of(spec).room {
        Some(r) => room_scene(r),
        None => {
            let (size, live) = (spec.get("roomsize"), spec.get("room"));
            if size < 0.33 {
                Room::Booth
            } else if size < 0.66 {
                Room::Studio
            } else if size >= 0.95 && live >= 0.85 {
                Room::Church
            } else {
                Room::Hall
            }
        }
    }
}

fn room_scene(r: RoomKind) -> Room {
    match r {
        RoomKind::Stage => Room::Stage,
        RoomKind::Booth => Room::Booth,
        RoomKind::Studio => Room::Studio,
        RoomKind::Hall => Room::Hall,
        RoomKind::Church => Room::Church,
    }
}

fn room_kind(r: Room) -> RoomKind {
    match r {
        Room::Stage => RoomKind::Stage,
        Room::Booth => RoomKind::Booth,
        Room::Studio => RoomKind::Studio,
        Room::Hall => RoomKind::Hall,
        Room::Church => RoomKind::Church,
    }
}

/// What the scene of `spec` shows, in the theme `s` (its microphones drawn when `mics`).
pub fn view_of(spec: &NativeSpec, s: &Style, mics: bool, hovered: Option<Picked>) -> SceneView {
    SceneView {
        subject: Subject::Piano {
            look: look::of(spec),
            lid: spec.get("lid"),
            persp: spec.get("persp"),
            width: spec.get("width"),
            room_level: spec.get("room"),
            mics,
        },
        room: room_of(spec),
        look: room_look::of(spec),
        dark: s.t.dark,
        selected: None,
        hovered,
        sounding: Vec::new(),
        accent: s.t.focus,
        ink: s.t.text,
    }
}

type WordsKey = (
    look::Look,
    room_look::RoomLook,
    Room,
    [u32; 4],
    crate::i18n::Locale,
);

/// The card's words (made again only when what they say changes) and its own state: the
/// microphones shown, the pointer over the lid, a lid drag in progress.
#[derive(Debug, Default)]
pub struct Words {
    key: Option<WordsKey>,
    said: String,
    hint: String,
    body: String,
    body_name: String,
    finish: String,
    finish_name: String,
    floor: String,
    walls: String,
    rug: String,
    /// The words that never change but with the language, made once (at rest the card
    /// allocates nothing for them): the rooms, the pickers' names, the hints.
    rooms: [String; 5],
    room_name: String,
    floor_name: String,
    walls_name: String,
    rug_name: String,
    /// What each room is (the card's second line), and the rooms row's help line.
    room_meta: [String; 5],
    room_tip: String,
    /// The room card's measures at the last scale (made again when the words change).
    sizes: Option<room_card::Sizes>,
    /// The look whose list is open, and the pass it opened on.
    open: Option<room_card::LookPart>,
    opened: u64,
    mics_label: String,
    mics_hint: String,
    lid_hint: String,
    /// The pickers' style (the parent's, `gap` between items), kept while the parent's
    /// stays: setting a child's spacing copies the whole style every frame.
    spaced: Option<(
        std::sync::Arc<egui::Style>,
        u32,
        std::sync::Arc<egui::Style>,
    )>,
    /// The microphones drawn (the *Show mics* key).
    pub mics: bool,
    hovered: Option<Picked>,
    drag: Option<f32>,
}

/// `ui`'s style with `gap` between items, made once while `ui`'s stays the same.
fn spaced(
    app: &mut CantusApp,
    track: TrackId,
    ui: &egui::Ui,
    gap: f32,
) -> std::sync::Arc<egui::Style> {
    let w = &mut app.inst.chorda.scenes.entry(track.0).or_default().1;
    let base = ui.style();
    match &w.spaced {
        Some((b, g, st)) if std::sync::Arc::ptr_eq(b, base) && *g == gap.to_bits() => st.clone(),
        _ => {
            let mut st = (**base).clone();
            st.spacing.item_spacing = Vec2::splat(gap);
            let st = std::sync::Arc::new(st);
            w.spaced = Some((base.clone(), gap.to_bits(), st.clone()));
            st
        }
    }
}

impl Words {
    /// What the pointer is over in the scene (the lid, while it is the lid).
    pub fn hovered(&self) -> Option<Picked> {
        self.hovered
    }
}

impl Words {
    fn update(&mut self, spec: &NativeSpec, room: Room) {
        let lk = look::of(spec);
        let rl = room_look::of(spec);
        let c = ["lid", "persp", "roomsize", "room"].map(|k| spec.get(k).to_bits());
        let key = (lk, rl, room, c, crate::i18n::locale());
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.said = caption(spec, room);
        self.hint = t("chorda.look.hint");
        self.body = t(lk.body.label_key());
        self.body_name = t("chorda.look.body");
        self.finish = t(lk.finish.label_key());
        self.finish_name = t("chorda.look.finish");
        let r = crate::scene::rooms::def(room);
        self.floor = t(rl.floor.unwrap_or(r.floor).label_key());
        self.walls = t(rl.walls.unwrap_or(r.walls).label_key());
        self.rug = t(rl.rug.unwrap_or(r.rug).label_key());
        self.rooms = std::array::from_fn(|i| t(RoomKind::ALL[i].label_key()));
        self.room_name = t("scene.room");
        self.floor_name = t("scene.floor");
        self.walls_name = t("scene.walls");
        self.rug_name = t("scene.rug");
        self.room_meta = std::array::from_fn(|i| {
            t(match RoomKind::ALL[i] {
                RoomKind::Stage => "scene.room.stage_meta",
                RoomKind::Booth => "scene.room.booth_meta",
                RoomKind::Studio => "scene.room.studio_meta",
                RoomKind::Hall => "scene.room.hall_meta",
                RoomKind::Church => "scene.room.church_meta",
            })
        });
        self.room_tip = t("scene.room.tip");
        self.sizes = None;
        self.mics_label = t("scene.mics");
        self.mics_hint = t("scene.mics.hint");
        self.lid_hint = t("scene.lid.hint");
    }
}

/// What the scene shows, in words (the screen reader's line).
pub fn caption(spec: &NativeSpec, room: Room) -> String {
    use crate::i18n::tf;
    let lk = look::of(spec);
    let lid = spec.get("lid");
    let lid = if lk.body == Body::Upright {
        if lid < 0.04 {
            t("chorda.scene.top_closed")
        } else {
            t("chorda.scene.top_open")
        }
    } else if lid < 0.04 {
        t("chorda.scene.lid_closed")
    } else if lid * scene::piano::GRAND_LID <= scene::piano::SHORT_STICK {
        t("chorda.scene.lid_short")
    } else {
        t("chorda.scene.lid_full")
    };
    let persp = spec.get("persp");
    let mics = if persp < 0.18 {
        t("chorda.scene.mics_close")
    } else if persp < 0.6 {
        t("chorda.scene.mics_player")
    } else {
        t("chorda.scene.mics_far")
    };
    let room = t(match room {
        Room::Stage => "chorda.scene.room_stage",
        Room::Booth => "chorda.scene.room_booth",
        Room::Studio => "chorda.scene.room_studio",
        Room::Hall => "chorda.scene.room_hall",
        Room::Church => "chorda.scene.room_church",
    });
    tf(
        "chorda.scene.caption",
        &[
            ("body", &t(lk.body.label_key())),
            ("finish", &t(lk.finish.label_key())),
            ("lid", &lid),
            ("mics", &mics),
            ("room", &room),
        ],
    )
}

/// The scene's card in `rect`: the title, the scene filling the display, the pickers over
/// its corners; a click on the lid opens or closes it, a drag sets its angle.
pub fn scene_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    busy: Option<&str>,
) {
    let f = Function::Space;
    let sf = s.function(f);
    let display = (rect.height() / s.k - 24.0 - 1.0 - 6.0).max(40.0);
    let cr = paint_card(ui, rect, s, &Card::new("chorda.scene", f).display(display));
    let Some(d) = cr.display else {
        return;
    };
    let room = room_of(spec);
    let resp = ui.interact(d, ui.id().with("chorda-scene"), Sense::click_and_drag());
    let (scene, words) = app.inst.chorda.scenes.entry(track.0).or_default();
    words.update(spec, room);
    let pointer = resp.hover_pos();
    let over = pointer.and_then(|p| scene.pick(d, p));
    words.hovered = over
        .filter(|p| *p == Picked::Lid)
        .or(words.drag.map(|_| Picked::Lid));
    let view = view_of(spec, s, words.mics, words.hovered);
    let painter = ui.painter_at(d);
    // A window's sizing pass (its first frame back, unseen) lays out at a passing size:
    // it asks for no build.
    if !ui.is_sizing_pass() && scene.paint(&painter, d, &view) {
        ui.ctx().request_repaint();
    }
    let sd = crate::views::inst::face::on_display(&sf);
    with_brush(&painter, |b| crate::views::inst::face::cover(b, d, &sd));
    // The lid: a click opens or closes it; a drag sets its angle (one undo step).
    let upright = look::of(spec).body == Body::Upright;
    let lid = spec.get("lid");
    let mut set_lid = None;
    if resp.clicked() && over == Some(Picked::Lid) {
        set_lid = Some((if lid < 0.04 { 1.0 } else { 0.0 }, false));
        words.drag = None;
    } else {
        if resp.drag_started() && over == Some(Picked::Lid) {
            words.drag = Some(lid);
        }
        if let Some(from) = words.drag {
            let dy = resp.drag_delta().y;
            if resp.dragged() && dy != 0.0 {
                let next = scene.drag_lid(from, dy, upright);
                words.drag = Some(next);
                set_lid = Some((next, true));
            }
            if resp.drag_stopped() || !resp.dragged() {
                words.drag = None;
            }
        }
    }
    let words = &app.inst.chorda.scenes[&track.0].1;
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &words.said));
    if resp.hovered() {
        let hint = if over == Some(Picked::Lid) {
            &words.lid_hint
        } else {
            &words.hint
        };
        crate::kit::hint::set_text(ui.ctx(), &words.said, hint, crate::kit::hint::Caps::NONE);
    }
    if let Some((v, gesture)) = set_lid {
        app.set_inst_value(track, "lid", v, gesture);
    }
    pickers(app, ui, track, spec, d, &sf);
    room_plate(app, ui, track, room, d, &sf);
    if let Some(busy) = busy {
        chip(ui, d, &sf, busy);
    }
}

/// The piano and its finish at the display's top-left; *Show mics* at its top-right.
fn pickers(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
) {
    let shown = look::of(spec);
    let pad = s.s(8.0);
    let row = Rect::from_min_size(
        Pos2::new(d.left() + pad, d.top() + pad),
        Vec2::new(s.s(BODY_W + FINISH_W + 6.0), s.s(controls::ROW)),
    );
    if row.right() > d.right() - pad - s.s(MICS_W + 8.0) || row.bottom() > d.bottom() - pad {
        return;
    }
    // One child for the whole row (each child Ui costs egui two allocations a frame): the
    // piano and its finish at the left, *Show mics* pushed to the right end.
    let full = Rect::from_min_max(row.min, Pos2::new(d.right() - pad, row.bottom()));
    let st = spaced(app, track, ui, s.s(6.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(full)
            .style(st)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let id = egui::Id::new(("chorda-look-body", track.0));
    let w = &app.inst.chorda.scenes[&track.0].1;
    if let Some(at) = controls::field(&mut child, s, id, &w.body, &w.body_name, BODY_W) {
        let items: [_; 4] = std::array::from_fn(|i| {
            radio(
                Src::Key(Body::ALL[i].label_key()),
                Body::ALL[i] == shown.body,
            )
        });
        if let Some(i) = controls::list(&mut child, s, id, at, BODY_W + 40.0, &items) {
            set(app, track, look::BODY_KEY, Body::ALL[i].key());
        }
    }
    let id = egui::Id::new(("chorda-look-finish", track.0));
    let w = &app.inst.chorda.scenes[&track.0].1;
    if let Some(at) = controls::field(&mut child, s, id, &w.finish, &w.finish_name, FINISH_W) {
        let items: [_; 5] = std::array::from_fn(|i| {
            radio(
                Src::Key(Finish::ALL[i].label_key()),
                Finish::ALL[i] == shown.finish,
            )
        });
        if let Some(i) = controls::list(&mut child, s, id, at, FINISH_W + 40.0, &items) {
            set(app, track, look::FINISH_KEY, Finish::ALL[i].key());
        }
    }
    let mics_left = d.right() - pad - s.s(MICS_W);
    let at = child.cursor().min.x;
    if mics_left > at {
        child.add_space(mics_left - at - child.spacing().item_spacing.x);
    }
    let w = &app.inst.chorda.scenes[&track.0].1;
    let on = w.mics;
    if controls::key(
        &mut child,
        s,
        Src::Str(&w.mics_label),
        &w.mics_hint,
        MICS_W,
        on,
    )
    .clicked()
    {
        if let Some((_, w)) = app.inst.chorda.scenes.get_mut(&track.0) {
            w.mics = !on;
        }
    }
}

/// The room card at the picture's bottom right (the card Tympanum's Kit view draws too,
/// [`room_card`]): *Room* and its five rooms, the room and what it is, the floor, the walls
/// and the rug — the look, never the sound. Left out when the picture is too small for it
/// under the pickers.
#[allow(clippy::too_many_arguments)]
fn room_plate(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    room: Room,
    d: Rect,
    s: &Style,
) {
    let pad = s.s(12.0);
    let now = room_kind(room);
    let at = RoomKind::ALL.iter().position(|k| *k == now).unwrap_or(0);
    let w = &app.inst.chorda.scenes[&track.0].1;
    let card = room_card::Card {
        label: &w.room_name,
        rooms: RoomKind::ALL,
        names: &w.rooms,
        now,
        meta: &w.room_meta[at],
        look_labels: [&w.floor_name, &w.walls_name, &w.rug_name],
        looks: [&w.floor, &w.walls, &w.rug],
        room_tip: &w.room_tip,
        look_tip: &w.hint,
    };
    let sz = match w.sizes {
        Some(z) if z.k == s.k => z,
        _ => room_card::sizes(ui, s, &card),
    };
    let layout = room_card::layout(&sz, s, d.width() - 2.0 * pad);
    let h = room_card::height(s, layout.1);
    if layout.0 > d.width() - 2.0 * pad || h > d.height() - s.s(controls::ROW) - 3.0 * pad {
        return;
    }
    let (mut open, mut opened) = (w.open, w.opened);
    let shown = room_card::show(
        ui,
        s,
        egui::Id::new(("chorda-room", track.0)),
        d,
        &card,
        &sz,
        layout,
        &mut open,
        &mut opened,
    );
    if let Some((_, w)) = app.inst.chorda.scenes.get_mut(&track.0) {
        w.sizes = Some(sz);
        w.open = open;
        w.opened = opened;
    }
    if let Some(k) = shown.room {
        set(app, track, room_look::ROOM_KEY, k.key());
    }
    if let Some((part, word)) = shown.look {
        set(app, track, part.key(), word);
    }
}

/// One look key set, as one undo step (named in the history as the look).
pub fn set(app: &mut CantusApp, track: TrackId, key: &str, word: &str) {
    let now = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .and_then(|n| n.text(key).map(str::to_string));
    if now.as_deref() != Some(word) {
        app.set_inst_text(track, key, Some(word.to_string()));
    }
}

/// A small plate at the display's bottom-left with `text` (the tuning being designed).
fn chip(ui: &egui::Ui, d: Rect, s: &Style, text: &str) {
    let pad = s.s(8.0);
    let h = s.s(20.0);
    let w = (d.width() - 2.0 * pad).min(s.s(360.0));
    let r = Rect::from_min_size(
        Pos2::new(d.left() + pad, d.bottom() - pad - h),
        Vec2::new(w, h),
    );
    with_brush(&ui.painter_at(d), |b| {
        well::paint(b, r, s, KitState::REST, None);
        let _ = ktext::paint(
            b,
            r.shrink2(Vec2::new(s.s(8.0), 0.0)),
            &Text::new(text, Role::Label, s.t.text)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End)),
            s.tk(),
        );
    });
}
