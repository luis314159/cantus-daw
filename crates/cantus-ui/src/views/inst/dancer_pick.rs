//! Saltator's character picker (round 35, ADR 0087): every included character as a tile
//! that dances its own routine (a preview drawn by the one renderer, at the tile's pixels),
//! its name from the cast's locale keys, the one chosen outlined in the unit's light; under
//! the tiles the chosen one's name, its line and its licence. Choosing is one undo step.
//! Imported characters of the project are tiles too, and *Import character…* opens the
//! import flow.

use crate::app::CantusApp;
use crate::dance_texture::DanceTexture;
use crate::i18n::t;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self as ktext, Policy, Role, Text};
use crate::kit::{with_brush, Fill, State as KitState, Style};
use cantus_core::inst::dancer::{self, CHARACTER_KEY};
use cantus_core::{NativeSpec, TrackId};
use cantus_dance::character::{Character, Licence, Origin};
use cantus_dance::render::{Background, Stage};
use cantus_dance::{pose_at, DanceFrame};
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use std::collections::HashMap;
use std::sync::Arc;

/// Frames a second the previews dance at (they are small; the stage keeps its own rate).
pub const PREVIEW_FPS: f64 = 30.0;

/// The previews' textures by character id, and the frame each last showed.
#[derive(Default)]
pub struct PickState {
    tiles: HashMap<String, (DanceTexture, i64, [u32; 2])>,
    /// How far the tiles are scrolled when the cast is taller than the card (round 40:
    /// twenty characters), in points; the wheel over the tiles and the scrollbar move it,
    /// smoothly (review 2 of round 40, R2-B8).
    pub scroll: f32,
    /// The rows that do not fit the card, as last drawn (`scroll` goes up to their height).
    pub hidden_rows: usize,
    /// Previews drawn (the budget's measure).
    pub draws: u64,
}

/// A character's name as shown: the cast's from the locale files, an import's its own.
pub fn character_name(ch: &Arc<dyn Character>) -> String {
    let info = ch.info();
    match info.origin {
        Origin::Builtin => {
            let key = cantus_dance::cast::name_key(&info.id);
            if crate::i18n::has(crate::i18n::locale(), &key) {
                t(&key)
            } else {
                info.name.clone()
            }
        }
        Origin::Imported { .. } => info.name.clone(),
    }
}

/// A character's one line, when it has one.
pub fn character_blurb(ch: &Arc<dyn Character>) -> Option<String> {
    let info = ch.info();
    let key = cantus_dance::cast::blurb_key(&info.id);
    (info.origin == Origin::Builtin && crate::i18n::has(crate::i18n::locale(), &key))
        .then(|| t(&key))
}

/// Its licence in words.
pub fn licence_line(ch: &Arc<dyn Character>) -> String {
    match &ch.info().licence {
        Licence::Cantus => t("dance.licence.cantus"),
        Licence::Stated { text, .. } => {
            crate::i18n::tf("dance.licence.stated", &[("licence", text)])
        }
        Licence::Unknown => t("dance.licence.unknown"),
    }
}

/// The characters the picker offers: the cast in its order, then every character imported
/// into the project (`dance_import::imported_in_project`), then the one the dancer shows
/// when it is none of them.
pub fn choices(app: &mut CantusApp, spec: &NativeSpec) -> Vec<Arc<dyn Character>> {
    let mut v = cantus_dance::cast::builtin();
    let dir = crate::dance_import::project_dir(app);
    let mut ids: Vec<String> = crate::dance_import::imported_in_project(app)
        .into_iter()
        .map(|l| format!("file:{}", l.rel))
        .collect();
    let id = spec
        .text(CHARACTER_KEY)
        .unwrap_or(dancer::DEFAULT_CHARACTER)
        .to_string();
    if id.starts_with("file:") && !ids.contains(&id) {
        ids.push(id);
    }
    for id in ids {
        let (c, missing) = app.dance_live.character(&id, &dir);
        if missing.is_none() && !v.iter().any(|x| x.info().id == c.info().id) {
            v.push(c);
        }
    }
    v
}

/// The frame a preview dances: its routine at 120 BPM on the wall clock, the dancer's own
/// look (hue, mood, energy), in the middle of its tile.
pub fn preview_frame(spec: &NativeSpec, wall: f64) -> DanceFrame {
    let beats = wall * 2.0;
    let bar = (beats / 4.0).floor();
    let mut f = DanceFrame {
        beats,
        seconds: wall,
        tempo_bpm: 120.0,
        playing: true,
        bar: bar as i32,
        bar_start_beats: bar * 4.0,
        bar_beats: 4.0,
        ..DanceFrame::default()
    };
    for (v, k) in f.values.iter_mut().zip(dancer::KEYS) {
        *v = match k {
            "hue" | "mood" | "energy" | "turn" => spec.get(k),
            _ => dancer::param(k).map_or(0.0, |p| p.default),
        };
    }
    f.values[dancer::KEYS.iter().position(|k| *k == "y").unwrap_or(9)] = 0.9;
    f
}

/// The largest tile side (from `max` down to `min`) at which `n` tiles fit in `rect`, in
/// rows with their names under them.
pub fn fitting_side(rect: Rect, n: usize, gap: f32, min: f32, max: f32) -> f32 {
    let mut side = max;
    while side > min {
        let cols = (((rect.width() + gap) / (side + gap)).floor() as usize).max(1);
        let rows = n.div_ceil(cols);
        let h = rows as f32 * (side + gap * 2.0 + side * 0.22) - gap;
        if h <= rect.height() {
            return side;
        }
        side -= 2.0;
    }
    min
}

/// The smallest tile the picker shows, at 100 %: below it the grid scrolls instead (round
/// 40); scaled with the interface (`grid`'s `floor`).
pub const MIN_SIDE: f32 = 48.0;

/// How the tiles lie in a card: their side, columns, all the rows, and the rows seen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Grid {
    pub side: f32,
    pub cols: usize,
    pub rows: usize,
    pub visible: usize,
    pub row_h: f32,
}

/// The tiles' grid for `n` in `rect`: the largest side that fits them all, else `floor`
/// ([`MIN_SIDE`] at the interface's scale) and as many rows as fit, the rest reached by
/// scrolling.
pub fn grid(rect: Rect, n: usize, gap: f32, min: f32, max: f32, floor: f32) -> Grid {
    let side = fitting_side(rect, n, gap, min.max(floor), max).max(floor.min(max));
    let cols = (((rect.width() + gap) / (side + gap)).floor() as usize).max(1);
    let rows = n.div_ceil(cols);
    let row_h = side + gap * 2.0 + side * 0.22;
    let visible = (((rect.height() + gap) / row_h).floor() as usize).clamp(1, rows.max(1));
    Grid {
        side,
        cols,
        rows,
        visible,
        row_h,
    }
}

/// Tile `i` of the grid in `rect` scrolled by `scroll` points: `None` when neither it nor
/// its name shows in `rect` (one partly in view is placed, and clipped by the painter).
pub fn placed(rect: Rect, g: &Grid, gap: f32, i: usize, scroll: f32) -> Option<Rect> {
    let (c, r) = (i % g.cols, i / g.cols);
    let top = rect.top() + r as f32 * g.row_h - scroll;
    if top + g.row_h - gap <= rect.top() || top >= rect.bottom() {
        return None;
    }
    Some(Rect::from_min_size(
        Pos2::new(rect.left() + c as f32 * (g.side + gap), top),
        Vec2::splat(g.side),
    ))
}

/// The tiles in `rect`, each `side` points square, `gap` apart, in rows.
pub fn tiles(rect: Rect, n: usize, side: f32, gap: f32) -> Vec<Rect> {
    let cols = (((rect.width() + gap) / (side + gap)).floor() as usize).max(1);
    (0..n)
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            Rect::from_min_size(
                Pos2::new(
                    rect.left() + c as f32 * (side + gap),
                    rect.top() + r as f32 * (side + gap * 2.0 + side * 0.22),
                ),
                Vec2::splat(side),
            )
        })
        .collect()
}

/// The picker in `rect`: the tiles, then the chosen one's words.
pub fn picker(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    wall: f64,
) {
    let chars = choices(app, spec);
    let chosen = spec
        .text(CHARACTER_KEY)
        .unwrap_or(dancer::DEFAULT_CHARACTER)
        .to_string();
    // The chosen one's words: its name, its line over as many rows as it needs, its licence
    // (r42 fix4-narrow: two rows cut Compás's line to "…y una estrella en el…").
    let ch = chars
        .iter()
        .find(|c| c.info().id == chosen)
        .cloned()
        .or_else(|| chars.first().cloned());
    let blurb_lines = ch.as_ref().map(|ch| {
        let blurb = match &ch.info().origin {
            Origin::Imported { format } => crate::i18n::tf("dance.imported", &[("format", format)]),
            Origin::Builtin => character_blurb(ch).unwrap_or_default(),
        };
        (*crate::kit::prose::wrap(ui.ctx(), &blurb, Role::Body, s.tk(), rect.width())).clone()
    });
    let blurb_rows = blurb_lines.as_ref().map_or(2, |l| l.lines.len().max(2));
    let words_row = s.s(18.0);
    let words_h = words_row * (blurb_rows + 2) as f32;
    let grid0 = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - words_h));
    let gap = s.s(6.0);
    let n = chars.len() + 1;
    // The largest tiles that fit them all; past the smallest, the grid scrolls (r40:cast),
    // its scrollbar the kit's, inside the card at its right, the tiles clear of it (R2-B8).
    let (lo, hi, floor) = (s.s(36.0), s.s(96.0), s.s(MIN_SIDE));
    let bar_w = s.s(8.0);
    let mut g = grid(grid0, n, gap, lo, hi, floor);
    let scrolls = g.rows > g.visible;
    if scrolls {
        let narrower = Rect::from_min_max(
            grid0.min,
            Pos2::new(grid0.right() - bar_w - gap, grid0.bottom()),
        );
        g = grid(narrower, n, gap, lo, hi, floor);
    }
    let side = g.side;
    let tiles_h = g.visible as f32 * g.row_h - gap;
    let block = tiles_h + s.s(6.0) + words_h;
    let top = rect.top() + ((rect.height() - block) * 0.5).max(0.0);
    let right = if scrolls {
        rect.right() - bar_w - gap
    } else {
        rect.right()
    };
    let grid = Rect::from_min_max(
        Pos2::new(rect.left(), top),
        Pos2::new(right, (top + tiles_h).min(grid0.bottom())),
    );
    let content_h = g.rows as f32 * g.row_h - gap;
    // Under half a point is a rounding of the card's height, not a row to scroll to.
    let max_scroll = Some(content_h - grid.height())
        .filter(|m| *m >= 0.5)
        .unwrap_or(0.0);
    {
        let st = &mut app.inst.dancer.pick;
        if max_scroll > 0.0 && ui.rect_contains_pointer(grid) {
            st.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        st.scroll = st.scroll.clamp(0.0, max_scroll);
        // fix-c (R2-C3): how many rows the card cannot show, as last drawn.
        st.hidden_rows = (max_scroll / g.row_h.max(1.0)).ceil() as usize;
    }
    if max_scroll > 0.0 {
        let bar = Rect::from_min_max(
            Pos2::new(rect.right() - bar_w, grid.top()),
            Pos2::new(rect.right(), grid.bottom()),
        );
        let st = &mut app.inst.dancer.pick;
        let mut start = st.scroll / content_h;
        let len = grid.height() / content_h;
        crate::kit::widgets::scrollbar(ui, bar, ui.id().with("dance-pick-scroll"), &mut start, len);
        st.scroll = (start * content_h).clamp(0.0, max_scroll);
        app.inst.dancer.placed.insert("pick.scroll", bar);
    }
    let scroll = app.inst.dancer.pick.scroll;
    let rects: Vec<Option<Rect>> = (0..n).map(|i| placed(grid, &g, gap, i, scroll)).collect();
    // What shows of a tile partly scrolled out: the grid's band, a hair wider for strokes.
    let view = grid.expand2(Vec2::new(s.s(3.0), 0.0));
    let ppp = ui.ctx().pixels_per_point();
    let frame_no = (wall * PREVIEW_FPS).floor() as i64;
    let pose_frame = preview_frame(spec, frame_no as f64 / PREVIEW_FPS);
    for (ch, r) in chars.iter().zip(&rects) {
        // Scrolled out of view.
        let Some(r) = r else { continue };
        let id = ch.info().id.clone();
        let resp = ui.interact(
            r.intersect(grid),
            ui.id().with(("dance-pick", id.as_str())),
            Sense::click(),
        );
        let name = character_name(ch);
        if resp.hovered() {
            let line = character_blurb(ch).unwrap_or_default();
            hint::set_text(ui.ctx(), &name, &line, Caps::NONE);
        }
        if resp.clicked() && id != chosen {
            app.set_inst_text(track, CHARACTER_KEY, Some(id.clone()));
        }
        let px = [
            (r.width() * ppp).round().max(1.0) as u32,
            (r.height() * ppp).round().max(1.0) as u32,
        ];
        let st = &mut app.inst.dancer.pick;
        let entry = st
            .tiles
            .entry(id.clone())
            .or_insert_with(|| (DanceTexture::new(format!("dancer-pick-{id}")), -1, [0, 0]));
        let tex = if entry.1 == frame_no && entry.2 == px {
            entry.0.id()
        } else {
            entry.1 = frame_no;
            entry.2 = px;
            st.draws += 1;
            let mut stage = Stage::new(px[0], px[1], Background::Transparent);
            stage.standard_height = 0.74;
            // The Slate cast cuts its pieces in the tile's colour (r40:cast).
            stage.ink = crate::dance_texture::stage_ink(&s.t, mix(s.t.deck, s.t.ink, 0.35));
            let pose = pose_at(&pose_frame, ch.as_ref(), wall);
            entry.0.draw(ui.ctx(), &stage, &[(ch.as_ref(), pose)])
        };
        let on = id == chosen;
        let state = KitState::of(&resp, on);
        // The name under the tile, across its cell (the tile and the gap beside it: the
        // neighbours' names never meet), in the label's size — a smaller one when the name
        // is longer than that, as "Metrónomo" under the smallest tiles (review 2 of round
        // 40); elided only if even that cannot hold it.
        let label = Rect::from_min_size(
            Pos2::new(r.left() - gap * 0.5, r.bottom() + gap * 0.5),
            Vec2::new(r.width() + gap, side * 0.22),
        );
        let ink = if on { s.t.text } else { s.t.label };
        let mut role = Role::Label;
        let named = |role| {
            Text::new(&name, role, ink)
                .caps(false)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(ktext::Elide::End))
        };
        if ktext::measure(ui.ctx(), &named(role), s.tk()).x > label.width() {
            role = Role::Micro;
        }
        let name_text = named(role);
        with_brush(
            &ui.painter_at(r.union(label).expand(s.s(3.0)).intersect(view)),
            |b| {
                let base = mix(s.t.deck, s.t.ink, 0.35);
                let fill = if state.hovered {
                    mix(base, s.hue, 0.10)
                } else {
                    base
                };
                b.rrect(*r, s.r(6.0), Fill::V(mix(fill, s.hue, 0.06), fill));
                // A pool of light under its feet.
                b.radial(
                    *r,
                    Pos2::new(r.center().x, r.top() + r.height() * 0.9),
                    Vec2::new(r.width() * 0.36, r.height() * 0.07),
                    &[
                        (0.0, with_alpha(s.hue, if on { 0.45 } else { 0.22 })),
                        (1.0, with_alpha(s.hue, 0.0)),
                    ],
                    6,
                );
                if let Some(tex) = tex {
                    b.foreign(tex, *r);
                }
                if on {
                    b.rrect_stroke(r.expand(s.s(1.5)), s.r(7.0), s.s(2.0), s.hue);
                } else if state.hovered {
                    b.rrect_stroke(*r, s.r(6.0), s.s(1.0), with_alpha(s.hue, 0.6));
                }
                let _ = ktext::paint(b, label, &name_text, s.tk());
            },
        );
        app.inst.dancer.placed.insert("pick", *r);
        if Some(&Some(*r)) == rects.first() {
            app.inst.dancer.placed.insert("pick.first", *r);
        }
    }
    {
        if let Some(Some(r)) = rects.last() {
            let resp = ui.interact(
                r.intersect(grid),
                ui.id().with("dance-import"),
                Sense::click(),
            );
            if resp.hovered() {
                hint::set_text(
                    ui.ctx(),
                    &t("dance.import"),
                    &t("dance.import.tip"),
                    Caps::NONE,
                );
            }
            let state = KitState::of(&resp, false);
            // A plus in the tile, its word under it as the characters' names are.
            let label = Rect::from_min_size(
                Pos2::new(r.left() - gap * 0.5, r.bottom() + gap * 0.5),
                Vec2::new(r.width() + gap, side * 0.22),
            );
            with_brush(
                &ui.painter_at(r.union(label).expand(s.s(3.0)).intersect(view)),
                |b| {
                    let line = if state.hovered { s.hue } else { s.t.label };
                    b.rrect_stroke(*r, s.r(6.0), s.s(1.0), with_alpha(line, 0.8));
                    let c = r.center();
                    let arm = r.width() * 0.16;
                    b.segment(
                        c - Vec2::new(arm, 0.0),
                        c + Vec2::new(arm, 0.0),
                        s.s(2.0),
                        line,
                        true,
                    );
                    b.segment(
                        c - Vec2::new(0.0, arm),
                        c + Vec2::new(0.0, arm),
                        s.s(2.0),
                        line,
                        true,
                    );
                    let _ = ktext::paint(
                        b,
                        label,
                        &Text::new(&t("dance.import.tile"), Role::Label, line)
                            .caps(false)
                            .align(Align2::CENTER_CENTER)
                            .policy(Policy::Elide(ktext::Elide::End)),
                        s.tk(),
                    );
                },
            );
            app.inst.dancer.placed.insert("import", *r);
            if resp.clicked() {
                super::import_character(app, track);
            }
        }
    }
    // The chosen one's words.
    if let (Some(ch), Some(blurb_lines)) = (ch, blurb_lines) {
        let wt = (grid.bottom() + s.s(6.0)).min(rect.bottom() - words_h);
        let w = Rect::from_min_max(
            Pos2::new(rect.left(), wt),
            Pos2::new(rect.right(), wt + words_h),
        );
        let row = words_row;
        let line = |i: f32| {
            Rect::from_min_size(
                Pos2::new(w.left(), w.top() + row * i),
                Vec2::new(w.width(), row),
            )
        };
        let name = character_name(&ch);
        let imported = matches!(ch.info().origin, Origin::Imported { .. });
        let lic = licence_line(&ch);
        if imported {
            // *Edit moves…*: the import card again, its mapping to change.
            let bw = s.s(110.0).min(w.width() * 0.45);
            let br = Rect::from_min_size(
                Pos2::new(w.right() - bw, w.top() + s.s(1.0)),
                Vec2::new(bw, s.s(20.0)),
            );
            app.inst.dancer.placed.insert("edit_moves", br);
            if super::controls::push(ui, br, s, "dance.edit_moves", "dance.edit_moves.tip")
                .clicked()
            {
                super::edit_moves(app, track);
            }
        }
        with_brush(&ui.painter_at(w), |b| {
            let _ = ktext::paint(
                b,
                line(0.0),
                &Text::new(&name, Role::Value, s.t.text)
                    .caps(false)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(ktext::Elide::End)),
                s.tk(),
            );
            // Its line, wrapped over the rows it needs.
            let mut lines = blurb_lines;
            let n = blurb_rows as f32;
            lines.line_h = row;
            let _ = crate::kit::prose::paint(
                b,
                Rect::from_min_size(line(1.0).min, Vec2::new(w.width(), row * n)),
                &lines,
                Role::Body,
                s.tk(),
                s.t.text_dim,
                Align2::LEFT_CENTER,
            );
            let _ = ktext::paint(
                b,
                line(1.0 + n),
                &Text::new(&lic, Role::Body, s.t.label)
                    .caps(false)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(ktext::Elide::End)),
                s.tk(),
            );
        });
    }
}
