//! The clip section's name and link rows (ADR 0037, round 26).
//!
//! - **Name**: the clip's name in a well; a click edits it in place (the whole name
//!   selected, so typing replaces it), Enter or a click away renames, Esc leaves it. A
//!   linked clip's name *is* its group's: the well carries the chain mark and ×N, and the
//!   rename is one `SetClipName` that `Command::apply` carries to every copy — one undo
//!   step.
//! - **Link**: a linked clip shows "3 linked copies" as a menu field whose list holds
//!   *Select linked copies*, *Make unique* and *Link to…*; a clip of its own shows
//!   *Link to…*, which opens the picker (`views::link_picker`) at once.
//!
//! A still frame formats nothing: the counts and words come from the sheet's text cache.

use super::super::menus::MenuId;
use super::super::sheet::{self, Sheet};
use super::super::Act;
use crate::app::CantusApp;
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{cache, well, State, Tok};
use cantus_core::{links, Clip, ClipId, Command, TrackId};
use egui::{pos2, Id, Pos2, Rect, Sense, Vec2};

/// What the link row asks for.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LinkAct {
    Select,
    MakeUnique,
    /// Open the picker, hanging from a point (the row) when one is given.
    LinkTo(Option<Pos2>),
}

/// Run a link row's action on clip `(t, c)`: on the selection when the clip is in it
/// (as the arrangement's menu does), else on the clip alone.
pub fn apply_link(app: &mut CantusApp, t: TrackId, c: ClipId, a: LinkAct) {
    if !app.clip_selection().contains(&(t, c)) {
        app.select_clip(t, c);
    }
    match a {
        LinkAct::Select => app.select_linked(),
        LinkAct::MakeUnique => app.make_unique_clips(),
        LinkAct::LinkTo(at) => app.open_link_picker(at),
    }
}

/// How many clips share `clip`'s content, when it is shared at all.
fn shared(app: &CantusApp, clip: &Clip) -> Option<usize> {
    let n = links::count(app.project(), clip.link?);
    (n > 1).then_some(n)
}

/// The name being typed, kept between frames; `fresh` until the field has been shown once
/// (it opens the frame after the click, so that click does not place the cursor, and with
/// the whole name selected). One at a time, for the clip it names: shown another clip,
/// the inspector drops an edit left unfinished.
#[derive(Clone)]
struct Editing {
    clip: (TrackId, ClipId),
    text: String,
    fresh: bool,
}

/// The name row: a readout that becomes a text field on a click.
pub fn name_row(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, c: ClipId, clip: &Clip) {
    let row = sh.row();
    sh.label(row.label, "track.name", false);
    let id = Id::new("insp-clip-name");
    let ctx = sh.ui.ctx().clone();
    let open = ctx
        .data(|d| d.get_temp::<Editing>(id))
        .filter(|e| e.clip == (t, c));
    let Some(Editing {
        mut text, fresh, ..
    }) = open
    else {
        ctx.data_mut(|d| d.remove::<Editing>(id));
        let r = sh.ui.interact(row.value, id, Sense::click());
        let count = shared(app, clip);
        if r.hovered() {
            let tip = if count.is_some() {
                sh.fmt.tr("links.inspector.rename_tip")
            } else {
                ""
            };
            hint::set_text(&ctx, &clip.name, tip, Caps::NONE);
        }
        readout(
            sh,
            row.value,
            &clip.name,
            count,
            c,
            State::of(&r, r.clicked()),
        );
        if r.clicked() {
            *sh.menu = None;
            let editing = Editing {
                text: clip.name.clone(),
                clip: (t, c),
                fresh: true,
            };
            ctx.data_mut(|d| d.insert_temp(id, editing));
            ctx.request_repaint();
        }
        return;
    };
    let _ = fresh;
    // The kit's field (round 26): the name selected as it opens; Enter or a click elsewhere
    // renames the group (one `SetClipName`, every sibling), Esc leaves the name.
    let out = crate::kit::field::Field::new(id)
        .opens_when_shown()
        .show(sh.ui, row.value, &mut text);
    match out.event {
        Some(crate::kit::field::Event::Cancel) => {
            ctx.data_mut(|d| d.remove::<Editing>(id));
            return;
        }
        Some(crate::kit::field::Event::Commit) => {
            ctx.data_mut(|d| d.remove::<Editing>(id));
            let name = text.trim();
            // An emptied name is an abandoned edit: a clip with no name is worse than its
            // old one.
            if !name.is_empty() && name != clip.name {
                sh.acts.push(Act::Exec(Command::SetClipName {
                    track: t,
                    clip: c,
                    name: name.to_string(),
                }));
            }
            return;
        }
        None => {}
    }
    let editing = Editing {
        clip: (t, c),
        text,
        fresh: false,
    };
    ctx.data_mut(|d| d.insert_temp(id, editing));
}

/// The name in its well; a shared clip's well ends with the chain mark and ×N.
fn readout(sh: &mut Sheet<'_>, rect: Rect, name: &str, count: Option<usize>, c: ClipId, st: State) {
    let s = sh.s;
    well::paint(sh.b, rect, &s, st, None);
    let mut right = rect.right() - s.s(6.0);
    if let Some(n) = count {
        let word = sheet::name(sh.fmt.get(cache::key(("insp-clip-xn", c)), n as u64, |b| {
            b.push_str(&crate::i18n::tf("links.count", &[("n", &n)]))
        }));
        let w = sheet::chip(
            sh.b,
            pos2(right, rect.center().y),
            &s,
            Src::Str(&word),
            Tok::Selection,
            1.0,
        );
        let g = Rect::from_center_size(
            pos2(right - w - s.s(9.0), rect.center().y),
            Vec2::splat(s.s(11.0)),
        );
        glyph::paint(sh.b, g, Glyph::Link, s.t.selection);
        right = g.left() - s.s(4.0);
    }
    let t = Text::new(name, Role::Body, s.t.text)
        .align(egui::Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let r = Rect::from_min_max(
        pos2(rect.left() + s.s(6.0), rect.top()),
        pos2(right, rect.bottom()),
    );
    let _ = text::paint(sh.b, r, &t, s.tk());
}

/// The link row: "3 linked copies" opening the group's actions, or *Link to…*.
pub fn link_row(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, c: ClipId, clip: &Clip) {
    let row = sh.row();
    sh.label(row.label, "links.inspector.row", false);
    if let Some(n) = shared(app, clip) {
        let stamp = n as u64 | (crate::i18n::locale() as u64) << 32;
        let text = sheet::name(sh.fmt.get(cache::key(("insp-clip-copies", c)), stamp, |b| {
            b.push_str(&crate::i18n::tf("links.inspector.copies", &[("n", &n)]))
        }));
        sh.menu_field(row.value, &text, MenuId::ClipLink(t, c), false);
        return;
    }
    let r = sh.ui.interact(
        row.value,
        Id::new(("insp-clip-link-to", t.0, c.0)),
        Sense::click(),
    );
    let word = sheet::name(sh.fmt.tr("links.inspector.link_to"));
    if r.hovered() {
        hint::set_text(sh.ui.ctx(), &word, "", Caps::NONE);
    }
    well::menu_field(sh.b, row.value, &sh.s, State::of(&r, false), &word, false);
    if r.clicked() {
        *sh.menu = None;
        sh.acts.push(Act::Link(
            t,
            c,
            LinkAct::LinkTo(Some(row.value.left_bottom())),
        ));
    }
}
