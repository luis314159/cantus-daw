//! *Room*, pinned at the right of Chorda's Mixer (the design's *Chorda · Mixer* sheets):
//! the model's own room — its size, Chorda's `room` as a fader, where the room pair plays —
//! or, for a recorded piano, the room it was recorded in: in its far mics already, no fader,
//! nothing added.

use super::mixer::MixLook;
use super::strip::Out;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::{with_brush, Style};
use crate::views::inst::inst_mixer::ui::{self as iu, words};
use crate::views::inst::inst_mixer::{strip, Edit, Strip, Top};
use crate::views::mixer::look::{self, Pal, W};
use cantus_core::inst::chorda_source as cs;
use cantus_core::{NativeSpec, OutputFamily, TrackId};
use egui::{pos2, vec2, Align2, Rect};

/// The Room strip in `r`.
#[allow(clippy::too_many_arguments)]
pub fn room(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    out: Option<&Out>,
    lk: &mut MixLook,
) {
    let p = Pal::of(s);
    let k = s.k;
    let recorded = cs::has_zones(spec);
    with_brush(ui.painter(), |b| look::body(b, r, &p, None));
    let name = Rect::from_min_size(pos2(r.left(), r.top() + 6.0 * k), vec2(r.width(), 15.0 * k));
    let sub = Rect::from_min_size(pos2(r.left(), name.bottom()), vec2(r.width(), 13.0 * k));
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            &t("chorda_source.strip.room"),
            name,
            Align2::CENTER_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        let sw = if recorded {
            t("chorda_source.strip.as_recorded")
        } else {
            t("chorda_source.strip.room_sub")
        };
        words(
            b,
            s,
            &sw,
            sub,
            Align2::CENTER_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    if recorded {
        recorded_room(app, ui, track, spec, s, r, sub.bottom());
        return;
    }
    // The model: its size, then its level as a fader.
    let cells = Rect::from_min_max(
        pos2(r.left() + 2.0 * k, sub.bottom() + 8.0 * k),
        pos2(r.right() - 2.0 * k, sub.bottom() + 72.0 * k),
    );
    // One knob, centred under the name (the design's cell).
    let cell = Rect::from_center_size(cells.center(), vec2(64.0 * k, cells.height()));
    super::super::organ::parts::at(ui, cell, ("chorda-room-size", track.0), |ui| {
        let size = crate::kit::knob::KnobSize::S;
        super::super::organ::parts::cell(app, ui, track, spec, "roomsize", s, size);
    });
    let cap = Rect::from_min_size(
        pos2(r.left() + 8.0 * k, cells.bottom() + 10.0 * k),
        vec2(r.width() - 16.0 * k, 30.0 * k),
    );
    let used = with_brush(ui.painter(), |b| {
        b.hline(
            r.left() + 10.0 * k,
            r.right() - 10.0 * k,
            b.snap(cap.top() - 5.0 * k),
            b.px(),
            p.line,
        );
        iu::prose(
            b,
            ui,
            s,
            cap,
            &t("chorda_source.strip.room_note"),
            p.ink2,
            Align2::CENTER_TOP,
        )
    });
    let lower = Rect::from_min_max(
        pos2(r.left() + 24.0 * k, cap.top() + used + 6.0 * k),
        pos2(r.right() - 24.0 * k, r.bottom()),
    );
    let level = spec.get("room");
    let out_cell = match out {
        Some(o) => super::strip::out_cell(app, track, &o.name, o.id),
        None => super::strip::out_cell(
            app,
            track,
            &t("chorda_source.strip.room"),
            super::strip::MODEL_ROOM,
        ),
    };
    let st = Strip {
        id: Some(super::strip::MODEL_ROOM),
        name: String::new(),
        sub: String::new(),
        family: OutputFamily::Rooms,
        top: Top::None,
        invert: None,
        pan: None,
        mute: None,
        solo: None,
        delay: None,
        level_db: if level <= 1e-4 {
            -96.0
        } else {
            cantus_core::linear_to_db(level)
        },
        level_max: 0.0,
        meter: None,
        out: out_cell,
        selected: lk.selected == Some(super::strip::MODEL_ROOM.0),
        loaded: true,
    };
    let salt = ui.id().with(("chorda-room-fader", track.0));
    for e in strip::lower(ui, s, lower, &st, salt) {
        match e {
            Edit::Level(db) => {
                let v = if db <= -95.0 {
                    0.0
                } else {
                    cantus_core::db_to_linear(db).clamp(0.0, 1.0)
                };
                app.set_inst_value(track, "room", v, true)
            }
            Edit::End => app.end_gesture(),
            Edit::Out(at) => super::strip::open_out(ui, lk, super::strip::MODEL_ROOM, at),
            Edit::Select => lk.selected = Some(super::strip::MODEL_ROOM.0),
            _ => {}
        }
    }
}

/// A recorded piano's room: in its far mics already — words, and a dashed box where a
/// fader would be (nothing is added).
/// The key of the room a recorded piano adds (Exemplar's `room.add`, prefixed).
pub const ADD_ROOM: &str = "rec.room.add";
/// Its level.
pub const ADD_ROOM_LEVEL: &str = "rec.room.level";
const ROOM_WORDS: [&str; 4] = [
    "chorda_source.room.off",
    "chorda_source.room.small",
    "chorda_source.room.studio",
    "chorda_source.room.hall",
];

/// A recorded piano's room: in its far mics already. *Add our room* — off (nothing added,
/// no fader: null), or one of Cantus's rooms after the pack's own (Tympanum's small room,
/// studio, hall), with its level on the fader. A click on the field steps through them; each
/// step one undo step.
#[allow(clippy::too_many_arguments)]
fn recorded_room(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    top: f32,
) {
    let p = Pal::of(s);
    let k = s.k;
    let note = Rect::from_min_max(
        pos2(r.left() + 10.0 * k, top + 10.0 * k),
        pos2(r.right() - 10.0 * k, top + 44.0 * k),
    );
    let label = Rect::from_min_size(
        pos2(r.left() + 8.0 * k, note.bottom() + 6.0 * k),
        vec2(r.width() - 16.0 * k, 14.0 * k),
    );
    let field = Rect::from_min_size(
        pos2(r.left() + 6.0 * k, label.bottom() + 4.0 * k),
        vec2(r.width() - 12.0 * k, 24.0 * k),
    );
    let choice = (spec.get(ADD_ROOM).round() as usize).min(3);
    let word = t(ROOM_WORDS[choice]);
    let fresp = ui.interact(
        field,
        ui.id().with(("chorda-add-room", track.0)),
        egui::Sense::click(),
    );
    let fresp = iu::hint(fresp, || t("chorda_source.tip.add_room"));
    if fresp.clicked() {
        app.set_inst_value(track, ADD_ROOM, ((choice + 1) % 4) as f32, false);
    }
    with_brush(ui.painter(), |b| {
        iu::prose(
            b,
            ui,
            s,
            note,
            &t("chorda_source.strip.room_rec_note"),
            p.ink2,
            Align2::CENTER_TOP,
        );
        words(
            b,
            s,
            &t("chorda_source.strip.add_room"),
            label,
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
        let st = crate::kit::State::of(&fresp, false);
        let lead = crate::views::inspector::look::Lead::None;
        crate::views::inspector::look::field(b, s, field, st, lead, &word, "", true);
    });
    let below = Rect::from_min_max(
        pos2(r.left() + 8.0 * k, field.bottom() + 10.0 * k),
        pos2(r.right() - 8.0 * k, r.bottom() - 6.0 * k),
    );
    if choice == 0 {
        // Nothing added: no fader.
        let bx = Rect::from_min_max(below.min, pos2(below.right(), below.bottom() - 22.0 * k));
        with_brush(ui.painter(), |b| {
            let x = b.snap_rect(bx);
            let pts = [
                x.left_top(),
                x.right_top(),
                x.right_bottom(),
                x.left_bottom(),
                x.left_top(),
            ];
            b.dashed(&pts, b.px(), p.line, 4.0 * k, 3.0 * k);
            let w = Rect::from_center_size(bx.center(), vec2(bx.width() - 16.0 * k, 40.0 * k));
            iu::prose(
                b,
                ui,
                s,
                w,
                &t("chorda_source.strip.no_fader"),
                p.ink3,
                Align2::CENTER_TOP,
            );
        });
        return;
    }
    // The room's level on a fader (it plays on the piano's own strip).
    let cap = Rect::from_min_size(below.min, vec2(below.width(), 28.0 * k));
    let used = with_brush(ui.painter(), |b| {
        iu::prose(
            b,
            ui,
            s,
            cap,
            &t("chorda_source.strip.room_added"),
            p.ink3,
            Align2::CENTER_TOP,
        )
    });
    let lower = Rect::from_min_max(
        pos2(r.left() + 24.0 * k, cap.top() + used + 6.0 * k),
        pos2(r.right() - 24.0 * k, r.bottom()),
    );
    let st = Strip {
        id: None,
        name: String::new(),
        sub: String::new(),
        family: OutputFamily::Rooms,
        top: Top::None,
        invert: None,
        pan: None,
        mute: None,
        solo: None,
        delay: None,
        level_db: spec.get(ADD_ROOM_LEVEL),
        level_max: 6.0,
        meter: None,
        out: crate::views::inst::inst_mixer::OutCell {
            label: t("inst_mixer.out_main"),
            number: None,
            opens: false,
            tip: t("chorda_source.tip.add_room"),
        },
        selected: false,
        loaded: true,
    };
    let salt = ui.id().with(("chorda-add-room-fader", track.0));
    for e in strip::lower(ui, s, lower, &st, salt) {
        match e {
            Edit::Level(db) => {
                app.set_inst_value(track, ADD_ROOM_LEVEL, db.clamp(-48.0, 6.0), true)
            }
            Edit::End => app.end_gesture(),
            _ => {}
        }
    }
}
