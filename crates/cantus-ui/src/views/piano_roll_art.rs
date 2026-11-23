//! r42:strings-rec — the piano roll's **articulation lane** and the note's menu (ADR 0158).
//!
//! A note may carry how it is played (`Note::art`): an articulation and, for a bowed string,
//! its bow. The lane (added from the velocity row's **+**, *Articulation*) draws each note's
//! marks at its start as a score prints them — a tenuto line for a sustain, a slur for a
//! legato, a dot for a staccato, a wedge for a spiccato, *pizz.*'s plucked ring, tremolo's
//! three strokes, and the down-bow's bracket or the up-bow's V above — and a hollow ring for a
//! note that leaves it to the instrument. A click on a note's column opens its menu; so does a
//! double-click on the note itself in the grid. A choice applies to the clicked note, or to
//! every selected note when the clicked one is selected: one `SetNoteArt`, one undo step.

use super::{clip_of, Geo, SelSet};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::{self, Brush, Fill, Style};
use crate::views::ticks_to_beats;
use cantus_core::note_art::{Articulation, Bow, NoteArt};
use cantus_core::{Clip, ClipId, Command, NoteId, TrackId};
use egui::{pos2, vec2, Color32, Pos2, Rect};

/// A choice from the note's menu.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The articulation (`None`: the instrument decides — the bow goes with it).
    Art(Option<Articulation>),
    /// The bow (`None`: the instrument turns it); a note without an articulation becomes
    /// a sustain.
    Bow(Option<Bow>),
}

/// The note's art after `change`.
pub fn changed(old: Option<NoteArt>, change: Change) -> Option<NoteArt> {
    match change {
        Change::Art(None) => None,
        Change::Art(Some(a)) => Some(NoteArt {
            articulation: a,
            bow: old.and_then(|o| o.bow),
        }),
        Change::Bow(b) => Some(NoteArt {
            articulation: old.map_or(Articulation::Sustain, |o| o.articulation),
            bow: b,
        }),
    }
}

/// The command that sets `change` on `ids` (the notes of `clip` they name), or `None` when
/// it would change nothing.
pub fn command(clip: &Clip, track: TrackId, ids: &[NoteId], change: Change) -> Option<Command> {
    let arts: Vec<(NoteId, Option<NoteArt>)> = ids
        .iter()
        .filter_map(|id| clip.note(*id).map(|n| (n.id, n.art)))
        .filter_map(|(id, old)| {
            let new = changed(old, change);
            (new != old).then_some((id, new))
        })
        .collect();
    (!arts.is_empty()).then_some(Command::SetNoteArt {
        track,
        clip: clip.id,
        arts,
    })
}

/// Apply `change` to the note `hit`, or to the selection when `hit` is in it.
pub fn apply(app: &mut CantusApp, track: TrackId, clip: ClipId, hit: NoteId, change: Change) {
    let ids = if app.selected_notes.contains(&hit) {
        app.selected_notes.clone()
    } else {
        vec![hit]
    };
    let cmd = clip_of(app, track, clip).and_then(|c| command(c, track, &ids, change));
    if let Some(cmd) = cmd {
        app.exec(cmd);
    }
}

/// Paint an articulation's mark centred on `c`, `h` tall.
pub fn paint_mark(b: &mut Brush, c: Pos2, h: f32, a: Articulation, ink: Color32) {
    let w = (h * 0.12).max(b.px());
    let r = h * 0.5;
    match a {
        Articulation::Sustain => b.segment(
            pos2(c.x - r * 0.8, c.y),
            pos2(c.x + r * 0.8, c.y),
            w * 1.4,
            ink,
            true,
        ),
        Articulation::Legato => b.arc(
            pos2(c.x, c.y + r * 0.55),
            r * 0.9,
            std::f32::consts::PI * 1.15,
            std::f32::consts::PI * 1.85,
            w * 1.2,
            ink,
            true,
        ),
        Articulation::Staccato => b.dot(c, r * 0.28, ink),
        Articulation::Spiccato => b.polygon(
            &[
                pos2(c.x - r * 0.32, c.y - r * 0.7),
                pos2(c.x + r * 0.32, c.y - r * 0.7),
                pos2(c.x, c.y + r * 0.7),
            ],
            ink,
        ),
        Articulation::Pizzicato => {
            // A plucked ring: a circle with its string drawn through.
            b.circle_stroke(c, r * 0.55, w * 1.1, ink);
            b.segment(
                pos2(c.x - r * 0.9, c.y),
                pos2(c.x + r * 0.9, c.y),
                w,
                ink,
                true,
            );
        }
        Articulation::Tremolo => {
            for k in [-1.0f32, 0.0, 1.0] {
                let y = c.y + k * r * 0.45;
                b.segment(
                    pos2(c.x - r * 0.7, y + r * 0.2),
                    pos2(c.x + r * 0.7, y - r * 0.2),
                    w * 1.3,
                    ink,
                    true,
                );
            }
        }
    }
}

/// Paint a bow's mark centred on `c`, `h` tall: the down-bow's bracket, the up-bow's V.
pub fn paint_bow(b: &mut Brush, c: Pos2, h: f32, bow: Bow, ink: Color32) {
    let w = (h * 0.11).max(b.px());
    let r = h * 0.5;
    match bow {
        Bow::Down => b.polyline(
            &[
                pos2(c.x - r * 0.6, c.y + r * 0.6),
                pos2(c.x - r * 0.6, c.y - r * 0.5),
                pos2(c.x + r * 0.6, c.y - r * 0.5),
                pos2(c.x + r * 0.6, c.y + r * 0.6),
            ],
            w,
            ink,
        ),
        Bow::Up => b.polyline(
            &[
                pos2(c.x - r * 0.5, c.y - r * 0.6),
                pos2(c.x, c.y + r * 0.6),
                pos2(c.x + r * 0.5, c.y - r * 0.6),
            ],
            w,
            ink,
        ),
    }
}

/// The lane: each note's marks at its start, a hollow ring where it has none.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    b: &mut Brush,
    geo: &Geo<'_>,
    lane: Rect,
    clip: &Clip,
    sel: &SelSet,
    selected_ids: &[NoteId],
    track_color: Color32,
    s: &Style,
) {
    let t = &s.t;
    let lane = b.snap_rect(lane);
    b.register(lane, "art_lane", &[kit::Tok::Panel], &[]);
    b.rect(lane, Fill::Solid(t.panel));
    let room = crate::views::room::Room::of(t);
    let h = (lane.height() * 0.32).clamp(s.s(8.0), s.s(16.0));
    let (art_y, bow_y) = (lane.bottom() - h * 0.9, lane.bottom() - h * 2.1);
    let from = geo.beats(lane.left()) - 0.5;
    let to = geo.beats(lane.right()) + 0.5;
    let mut last_col = i32::MIN;
    for n in &clip.notes {
        let beat = ticks_to_beats(n.start);
        if beat < from || beat > to {
            continue;
        }
        let x = b.snap_mid(geo.x(beat));
        // Notes starting in one column (a chord) share one mark: the first drawn.
        let col = (x / (h * 1.1)).round() as i32;
        let selected = sel.contains(selected_ids, n.id);
        if col == last_col && !selected {
            continue;
        }
        last_col = col;
        let ink = if selected {
            t.highlight
        } else if n.muted {
            room.ink3
        } else {
            kit::color::mix(track_color, t.text, 0.55)
        };
        match n.art {
            None => b.circle_stroke(pos2(x, art_y), h * 0.18, b.px(), room.ink3),
            Some(a) => {
                paint_mark(b, pos2(x, art_y), h, a.articulation, ink);
                if let Some(bow) = a.bow {
                    if bow_y - h * 0.5 > lane.top() {
                        paint_bow(b, pos2(x, bow_y), h * 0.9, bow, ink);
                    }
                }
            }
        }
    }
}

/// The note whose column `x` is in: the last one starting within a few points of it.
fn note_at(geo: &Geo<'_>, clip: &Clip, x: f32) -> Option<NoteId> {
    clip.notes
        .iter()
        .rev()
        .find(|n| (geo.x(ticks_to_beats(n.start)) - x).abs() < 6.0)
        .map(|n| n.id)
}

fn popup_id() -> egui::Id {
    egui::Id::new("pr-note-art-menu")
}

/// The lane as a control: a click on a note's column opens its menu.
#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    app: &mut CantusApp,
    ui: &egui::Ui,
    geo: &Geo<'_>,
    resp: &egui::Response,
    track: TrackId,
    clip: ClipId,
) {
    if resp.hovered() {
        hint::set_text(ui.ctx(), &t("art.lane"), &t("art.lane_tip"), Caps::NONE);
    }
    let hit = resp
        .clicked()
        .then(|| resp.interact_pointer_pos())
        .flatten()
        .and_then(|p| {
            clip_of(app, track, clip)
                .and_then(|c| note_at(geo, c, p.x))
                .map(|id| (id, p))
        });
    open_menu(app, ui, resp, hit, track, clip);
}

/// The note's menu from the grid: a double-click on a note.
pub(super) fn grid_menu(
    app: &mut CantusApp,
    ui: &egui::Ui,
    geo: &Geo<'_>,
    resp: &egui::Response,
    track: TrackId,
    clip: ClipId,
) {
    let hit = resp
        .double_clicked()
        .then(|| resp.interact_pointer_pos())
        .flatten()
        .and_then(|p| {
            clip_of(app, track, clip)
                .and_then(|c| geo.hit(&c.notes, p))
                .map(|(id, _)| (id, p))
        });
    open_menu(app, ui, resp, hit, track, clip);
}

/// Open (on `hit`) and show the note's menu: its articulations and bows, the current ones
/// checked; a choice is applied to the note or its selection.
fn open_menu(
    app: &mut CantusApp,
    ui: &egui::Ui,
    resp: &egui::Response,
    hit: Option<(NoteId, Pos2)>,
    track: TrackId,
    clip: ClipId,
) {
    let id = popup_id();
    if let Some((note, _)) = hit {
        ui.data_mut(|d| d.insert_temp(id, note));
    }
    let Some(note) = ui.data(|d| d.get_temp::<NoteId>(id)) else {
        return;
    };
    let current = clip_of(app, track, clip)
        .and_then(|c| c.note(note))
        .and_then(|n| n.art);
    let mut popup = egui::Popup::from_response(resp)
        .id(id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(crate::views::clips_chrome::menu_frame())
        .open_memory(hit.map(|_| egui::SetOpenCommand::Bool(true)));
    if let Some((_, p)) = hit {
        popup = popup.at_position(p + vec2(4.0, 4.0));
        ui.data_mut(|d| d.insert_temp(id.with("at"), p));
    } else if let Some(p) = ui.data(|d| d.get_temp::<Pos2>(id.with("at"))) {
        popup = popup.at_position(p + vec2(4.0, 4.0));
    }
    let mut picked = None;
    popup.show(|ui| {
        let (items, changes) = menu_items(current);
        let refs: Vec<kit::menu::Item<'_>> = items.iter().map(|i| i.item()).collect();
        if let Some(i) = kit::widgets::menu_list(ui, &refs, 180.0).1.clicked {
            picked = changes.get(i).copied().flatten();
            if picked.is_some() {
                ui.close();
            }
        }
    });
    if let Some(change) = picked {
        apply(app, track, clip, note, change);
    }
}

/// A menu row owning its words.
struct Row {
    words: String,
    check: kit::menu::Check,
    band: bool,
}

impl Row {
    fn item(&self) -> kit::menu::Item<'_> {
        if self.band {
            return kit::menu::Item::Band(Src::Str(&self.words));
        }
        kit::menu::Item::Action {
            label: Src::Str(&self.words),
            check: self.check,
            shortcut: None,
            submenu: false,
            enabled: true,
        }
    }
}

/// The menu's rows and what each picks: the articulations (the instrument's choice first),
/// then the bows.
fn menu_items(current: Option<NoteArt>) -> (Vec<Row>, Vec<Option<Change>>) {
    use kit::menu::Check;
    let mut rows = Vec::new();
    let mut picks = Vec::new();
    let mut push = |words: String, check: Check, band: bool, pick: Option<Change>| {
        rows.push(Row { words, check, band });
        picks.push(pick);
    };
    push(t("art.band"), Check::None, true, None);
    push(
        t("art.auto"),
        Check::Radio(current.is_none()),
        false,
        Some(Change::Art(None)),
    );
    for a in Articulation::ALL {
        push(
            t(a.name_key()),
            Check::Radio(current.is_some_and(|c| c.articulation == a)),
            false,
            Some(Change::Art(Some(a))),
        );
    }
    push(t("art.bow.band"), Check::None, true, None);
    let bow = current.and_then(|c| c.bow);
    for (key, b) in [
        ("art.bow.auto", None),
        ("art.bow.down", Some(Bow::Down)),
        ("art.bow.up", Some(Bow::Up)),
    ] {
        push(t(key), Check::Radio(bow == b), false, Some(Change::Bow(b)));
    }
    (rows, picks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_keeps_what_it_does_not_name() {
        let pizz_up = Some(NoteArt {
            articulation: Articulation::Pizzicato,
            bow: Some(Bow::Up),
        });
        assert_eq!(
            changed(pizz_up, Change::Art(Some(Articulation::Legato))),
            Some(NoteArt {
                articulation: Articulation::Legato,
                bow: Some(Bow::Up)
            })
        );
        assert_eq!(changed(pizz_up, Change::Art(None)), None);
        assert_eq!(
            changed(None, Change::Bow(Some(Bow::Down))),
            Some(NoteArt {
                articulation: Articulation::Sustain,
                bow: Some(Bow::Down)
            })
        );
        let (rows, picks) = menu_items(None);
        assert_eq!(rows.len(), picks.len());
        assert_eq!(rows.len(), 2 + 1 + Articulation::ALL.len() + 3);
    }
}
