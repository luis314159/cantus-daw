//! Chorda's **Voicing** view (round 42, `chorda-presets`, ADR 0098's eighth amendment): the
//! owner's *"el usuario debe poder mover cada uno de los parámetros a mano"*. Every number of
//! model 2's voicing a player may move, grouped as a technician thinks of them — hammers,
//! strings, soundboard and case, action, microphones, tone — each group a card in its
//! function's hue with its picture (the technician's voicing dashed, the one set over it;
//! `chorda_voicing_draw.rs`) and its cells (Chorda's own controls that belong there beside
//! the voicing's: the hardness with the hammers, the decay and the unison with the strings…).
//! Every cell is a parameter (`cantus_core::inst::chorda::VOICING_GROUPS`): a drag is one undo
//! step, a double-click returns it to the voicing's own number, automation and MIDI learn
//! reach it as every other. A row whose two cards do not fit side by side stacks them.

#[path = "chorda_voicing_draw.rs"]
mod pictures;

use super::super::organ::parts::{label, paint_card, Card, Rows};
use super::{card, card_h, model2};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::cell;
use crate::kit::knob::KnobSize;
use crate::kit::text::Role;
use crate::kit::{with_brush, Function, Style};
use cantus_core::inst::chorda::VOICING_GROUPS;
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::chorda::voicing::Voicing;
use cantus_engine::inst::chorda::{settings_from, values_of};
use egui::{Rect, Sense, Vec2};
use pictures::{Pair, Picture};

/// A group card's display and its cells' body, at 100 %.
const DISPLAY: f32 = 74.0;
const BODY: f32 = 86.0;
/// The note at the view's top: one line of body text.
const NOTE: f32 = 22.0;

/// Each group's hue and picture, in [`VOICING_GROUPS`]' order.
const LOOK: [(Function, Picture); 6] = [
    (Function::Dynamics, Picture::Hammers),
    (Function::Source, Picture::Strings),
    (Function::Space, Picture::Board),
    (Function::Dynamics, Picture::Action),
    (Function::Space, Picture::Mics),
    (Function::Output, Picture::Tone),
];

/// The rows: the groups (by index into [`VOICING_GROUPS`]) side by side, with their shares.
const ROWS: [&[(usize, f32)]; 4] = [
    &[(0, 0.45), (5, 0.55)],
    &[(1, 1.0)],
    &[(2, 0.46), (3, 0.54)],
    &[(4, 1.0)],
];

/// The view's height at 100 % in the editor's own width (every row's cards side by side): what
/// the tests read it whole in.
pub const VOICING_H: f32 =
    (24.0 + 8.0 + NOTE + 8.0) + 4.0 * (24.0 + 1.0 + DISPLAY + 8.0 + BODY + 8.0) + 4.0 * 4.0;

/// Whether `n` cells fit side by side at the smallest size in a card `w` wide.
fn fits(n: usize, w: f32, s: &Style) -> bool {
    let need = cell::size(KnobSize::S).x * s.k * n as f32 + s.s(2.0) * n.saturating_sub(1) as f32;
    need + s.s(20.0) <= w
}

/// The Voicing view: the note, then the groups' cards in rows (stacked where a row's two do
/// not fit side by side). Taller than the window, it scrolls there.
pub fn voicing(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
) {
    let (vs, hardness) = settings_from(&values_of(spec));
    let base = Voicing::default();
    let pair = Pair {
        set: vs.user.over(&base),
        base,
        hardness,
        decay: vs.voice.decay,
        tilt: vs.tilt_db,
    };
    let gutter = s.d(4.0);
    let width = ui.available_width();
    let row_h = s.s(card_h(DISPLAY, BODY));
    // Which rows stack their cards: one whose cards' cells cannot all fit at their share.
    let w_of = |share: f32, n: usize| (width - gutter * (n.saturating_sub(1)) as f32) * share;
    let stacked: Vec<bool> = ROWS
        .iter()
        .map(|row| {
            row.len() > 1
                && row
                    .iter()
                    .any(|(g, share)| !fits(VOICING_GROUPS[*g].1.len(), w_of(*share, row.len()), s))
        })
        .collect();
    let lines: usize = ROWS
        .iter()
        .zip(&stacked)
        .map(|(row, st)| if *st { row.len() } else { 1 })
        .sum();
    let note_h = s.s(card_h(0.0, NOTE));
    let total = note_h + row_h * lines as f32 + gutter * lines as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, total), Sense::hover());
    app.inst.chorda.area = Some(area);
    app.inst.chorda.view = Some(ui.clip_rect());
    let mut rows = Rows::new(area, s);

    let top = rows.row(note_h, &[1.0]);
    let c = paint_card(ui, top[0], s, &Card::new("chorda.v.title", Function::Tone));
    let note = if model2(spec) {
        "chorda.v.note"
    } else {
        "chorda.v.model1"
    };
    with_brush(&ui.painter_at(c.body), |b| {
        let r = c.body.shrink2(Vec2::new(s.s(10.0), 0.0));
        label(b, r, &t(note), Role::Body, s.t.text_dim, s);
    });

    for (row, st) in ROWS.iter().zip(&stacked) {
        let cards: Vec<(usize, Rect)> = if *st {
            row.iter()
                .map(|(g, _)| (*g, rows.row(row_h, &[1.0])[0]))
                .collect()
        } else {
            let shares: Vec<f32> = row.iter().map(|(_, f)| *f).collect();
            row.iter()
                .map(|(g, _)| *g)
                .zip(rows.row(row_h, &shares))
                .collect()
        };
        for (g, rect) in cards {
            let (title, keys) = VOICING_GROUPS[g];
            let (f, what) = LOOK[g];
            card(
                app,
                ui,
                track,
                spec,
                rect,
                s,
                title,
                f,
                DISPLAY,
                keys,
                |b, d, sf| pictures::paint(b, d, sf, what, &pair),
            );
        }
    }
}
