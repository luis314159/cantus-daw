//! The pad grid, the display of the pads card: every pad as a raised cell with its rendered
//! hit, its name, note and model, its choke group drawn as a lettered chip and a link to
//! the pads it chokes. The lowest pad sits at the bottom left, as on a drum machine. Drawn
//! with the kit's brush in tokens (ADR 0024 §4): the lighting recipe for the cells, the
//! device hue for the hit and the selection, the highlight for choke groups.

use super::{audition, set_field};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{shade, with_alpha};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::menu::Item;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style, Tok};
use crate::views::inst::sampler::parts;
use cantus_core::{pitch_name, DrumPad, PadField, PadVoice, PulsusParams, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

pub(super) const COLUMNS: usize = 4;

/// The letter a choke group is drawn with: 1 → A … 16 → P.
pub fn choke_letter(group: u8) -> char {
    (b'A' + group.saturating_sub(1).min(25)) as char
}

/// WCAG contrast between two opaque colours.
pub fn contrast(a: egui::Color32, b: egui::Color32) -> f32 {
    crate::kit::color::contrast(a, b)
}

/// Where each pad's cell is in `area`: the lowest pad at the bottom left.
pub fn cell_rects(area: Rect, count: usize, gap: f32) -> Vec<Rect> {
    let rows = count.div_ceil(COLUMNS).max(1);
    let w = (area.width() - gap * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
    let h = ((area.height() - gap * (rows as f32 - 1.0)) / rows as f32).max(1.0);
    (0..count)
        .map(|i| {
            let (col, row) = (i % COLUMNS, i / COLUMNS);
            let row_from_top = rows - 1 - row;
            let min = pos2(
                area.left() + col as f32 * (w + gap),
                area.top() + row_from_top as f32 * (h + gap),
            );
            Rect::from_min_size(min, vec2(w, h))
        })
        .collect()
}

/// The voice and model a cell names under the pad's name.
fn kind_caption(pad: &DrumPad) -> String {
    let models = pad.voice.models();
    if pad.voice == PadVoice::Tom {
        let names = PadField::Material.choices(pad).unwrap_or(&[]);
        names
            .get(pad.material as usize)
            .map(|k| t(k))
            .unwrap_or_else(|| t("pulsus.voice.tom"))
    } else if models.len() > 1 {
        t(models[pad.model_index()])
    } else {
        t(pad.voice.label_key())
    }
}

/// The grid in `area` (the pads card's display): painted, clicked to select and hear,
/// right-clicked for Mute and Remove.
pub(super) fn show(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    area: Rect,
    s: &Style,
) {
    let k = s.k;
    let inner = area.shrink(8.0 * k);
    let rects = cell_rects(inner, kit.pads.len(), 6.0 * k);
    let selected = app.pulsus.selected;
    let chip_at = |r: &Rect| pos2(r.right() - 13.0 * k, r.top() + 11.0 * k);
    let mut hovered = None;
    let mut menu_for = None;
    let mut resps = Vec::with_capacity(kit.pads.len());
    for (i, (pad, rect)) in kit.pads.iter().zip(&rects).enumerate() {
        let resp = ui.allocate_rect(*rect, Sense::click());
        // A sample from the browser makes this pad play it (ADR 0030).
        crate::samples::drop::publish(
            ui.ctx(),
            *rect,
            crate::samples::drop::Target::Pad { track, pad: i },
        );
        if resp.hovered() {
            hovered = Some(pad.id);
            crate::kit::hint::set_text(
                ui.ctx(),
                &pad.name,
                &pitch_name(pad.pitch),
                crate::kit::hint::Caps {
                    menu: true,
                    ..crate::kit::hint::Caps::NONE
                },
            );
        }
        if resp.secondary_clicked() {
            menu_for = Some(pad.id);
        }
        resps.push(resp);
    }
    let previews: Vec<super::Preview> = kit
        .pads
        .iter()
        .map(|pad| {
            app.pulsus
                .preview(&mut app.engine.samples, pad, kit.version)
                .clone()
        })
        .collect();
    with_brush(&ui.painter_at(area), |b| {
        b.register(
            area,
            "pulsus.grid",
            &[
                Tok::Deck,
                Tok::Text,
                Tok::TextDim,
                Tok::Highlight,
                Tok::Edge,
            ],
            &[s.hue_tok],
        );
        // Choke links under the cells' chips: one line through every pad of a group.
        for group in 1..=cantus_core::CHOKE_GROUPS {
            let members: Vec<Pos2> = kit
                .pads
                .iter()
                .zip(&rects)
                .filter(|(p, _)| p.choke == group)
                .map(|(_, r)| chip_at(r))
                .collect();
            if members.len() > 1 {
                b.polyline(&members, 2.5 * k, with_alpha(s.t.highlight, 0.8));
            }
        }
        for ((pad, rect), preview) in kit.pads.iter().zip(&rects).zip(&previews) {
            let sel = selected == Some(pad.id);
            paint_pad(
                b,
                *rect,
                s,
                pad,
                preview,
                sel,
                hovered == Some(pad.id),
                chip_at(rect),
            );
        }
    });
    for ((pad, rect), resp) in kit.pads.iter().zip(&rects).zip(&resps) {
        if resp.clicked() {
            app.pulsus.selected = Some(pad.id);
            // Higher on the cell is harder, as on a pad that reads position.
            let vel = resp
                .interact_pointer_pos()
                .map(|p| 127.0 - 67.0 * ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0))
                .unwrap_or(110.0);
            audition(app, track, pad.pitch, vel as u8);
        }
    }
    // The right-click menu: mute, remove.
    let id = egui::Id::new(("pulsus-pad-menu", track.0));
    if let Some(p) = menu_for {
        ui.data_mut(|d| d.insert_temp(id.with("pad"), p));
    }
    let target = ui.data(|d| d.get_temp::<cantus_core::PadId>(id.with("pad")));
    if let Some(pid) = target {
        if let Some((pad, rect)) = kit
            .pads
            .iter()
            .zip(&rects)
            .find(|(p, _)| p.id == pid)
            .map(|(p, r)| (p.clone(), *r))
        {
            let mute_key = if pad.muted {
                "pulsus.unmute"
            } else {
                "pad.mute"
            };
            let items = [
                Item::action(Src::Key(mute_key)),
                Item::action(Src::Key("pad.remove")),
            ];
            let anchor = Rect::from_min_size(rect.center(), vec2(rect.width() * 0.5, 0.0));
            match parts::dropdown(ui, id, anchor, s, &items, menu_for.is_some()) {
                Some(0) => {
                    let v = if pad.muted { 0.0 } else { 1.0 };
                    set_field(app, track, pad.id, PadField::Mute, v, false);
                }
                Some(_) => {
                    if let Some(i) = kit.index_of(pad.id) {
                        app.remove_pad(track, i);
                    }
                }
                None => {}
            }
        }
    }
}

/// One pad's cell: raised (lit in the hue when selected), its hit, name, note, model and
/// choke chip, each word in its box.
#[allow(clippy::too_many_arguments)]
fn paint_pad(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    pad: &DrumPad,
    preview: &super::Preview,
    selected: bool,
    hovered: bool,
    chip: Pos2,
) {
    let t = &s.t;
    let k = s.k;
    let base = if selected {
        crate::kit::color::mix(t.deck, s.hue, if t.dark { 0.22 } else { 0.14 })
    } else if hovered {
        shade(t.deck, if t.dark { 0.03 } else { -0.02 })
    } else {
        t.deck
    };
    let r = b.snap_rect(rect);
    light::raised(
        b,
        r,
        s.r(4.0),
        base,
        RaisedOpts {
            k,
            depth: t.depth,
            ..Default::default()
        },
    );
    if selected {
        b.rrect_stroke(r.shrink(0.75 * k), s.r(4.0), 1.5 * k, s.hue);
    }
    let pad_x = 7.0 * k;
    let line_h = 14.0 * k;
    let name_r = Rect::from_min_size(
        r.min + vec2(pad_x, 3.0 * k),
        vec2(
            r.width() - 2.0 * pad_x - if pad.choke > 0 { 22.0 * k } else { 0.0 },
            line_h,
        ),
    );
    let foot = Rect::from_min_max(
        pos2(r.left() + pad_x, r.bottom() - line_h - 2.0 * k),
        pos2(r.right() - pad_x, r.bottom() - 2.0 * k),
    );
    let wave = Rect::from_min_max(
        pos2(r.left() + 5.0 * k, name_r.bottom() + 2.0 * k),
        pos2(r.right() - 5.0 * k, foot.top() - 1.0 * k),
    );
    paint_wave(b, wave, s, preview, selected);
    let name_color = if pad.muted { t.text_dim } else { t.text };
    let painted = ktext::paint(
        b,
        name_r,
        &Text::new(&pad.name, Role::Label, name_color)
            .caps(false)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::Middle)),
        s.tk(),
    );
    if let (true, Some(p)) = (pad.muted, painted) {
        // Struck through, not only dimmed: a state is never carried by colour alone.
        b.hline(
            p.rect.left(),
            p.rect.right(),
            p.rect.center().y,
            b.px(),
            t.text,
        );
    }
    let note = pitch_name(pad.pitch);
    let half = foot.width() * 0.4;
    let _ = ktext::paint(
        b,
        Rect::from_min_size(foot.min, vec2(half, foot.height())),
        &Text::new(&note, Role::Micro, t.text_dim)
            .caps(false)
            .align(Align2::LEFT_CENTER),
        s.tk(),
    );
    let kind = kind_caption(pad);
    let _ = ktext::paint(
        b,
        Rect::from_min_max(pos2(foot.left() + half, foot.top()), foot.max),
        &Text::new(&kind, Role::Micro, t.text_dim)
            .caps(false)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
    if pad.choke > 0 {
        let cr = b.snap_rect(Rect::from_center_size(chip, vec2(18.0 * k, 15.0 * k)));
        b.rrect(cr, 3.0 * k, Fill::Solid(t.deck));
        b.rrect_stroke(cr.shrink(0.75 * k), 3.0 * k, 1.5 * k, t.highlight);
        let mut letter = [0u8; 4];
        let l = choke_letter(pad.choke).encode_utf8(&mut letter);
        let _ = ktext::paint(b, cr, &Text::new(l, Role::Label, t.text), s.tk());
    }
}

/// A hit as a mirrored fill, downsampled to the width it has.
fn paint_wave(b: &mut Brush, rect: Rect, s: &Style, preview: &super::Preview, selected: bool) {
    let n = preview.columns.len();
    if n == 0 || rect.width() < 4.0 || rect.height() < 4.0 {
        return;
    }
    let cols = ((rect.width() * b.ppp()) as usize).clamp(8, n);
    let mid = rect.center().y;
    let scale = rect.height() * 0.5 / preview.peak.max(1e-4);
    let color = if selected {
        s.hue
    } else {
        with_alpha(s.hue, if s.t.dark { 0.62 } else { 0.55 })
    };
    let w = rect.width() / cols as f32;
    for c in 0..cols {
        let from = c * n / cols;
        let to = ((c + 1) * n / cols).max(from + 1).min(n);
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for (a, z) in &preview.columns[from..to] {
            lo = lo.min(*a);
            hi = hi.max(*z);
        }
        let amp = hi.max(-lo) * scale;
        let x = rect.left() + c as f32 * w;
        b.rect(
            Rect::from_min_max(pos2(x, mid - amp), pos2(x + w, mid + amp + b.px())),
            Fill::Solid(color),
        );
    }
}
