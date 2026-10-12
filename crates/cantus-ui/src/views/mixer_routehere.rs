//! The **Route here** plate (round 25, ADR 0041; design §4.2): a strip nothing feeds
//! (`routing::is_fed` false — tracks, outputs, sends, keys and notes all counted) shows,
//! inside its silent meter well, a dashed well with a `+` and *Route here* running up it
//! (Compact: the `+` alone; the words go to the hint bar), whose click opens the
//! route-here menu (`views::route_menu::open_route_here`), ordered by what the strip
//! probably is (a channel offers its tracks first, a return the sends). The column under
//! it is the drop target of every drag (`mixer_drag`), so the plate also says "or drag a
//! cable here". It goes the frame the strip receives anything (`Desk::fed` is per
//! revision).
//!
//! The meter is sensed after the plate (it selects the strip on a click), so the plate
//! reads the pointer itself rather than competing for egui's click: a click on it both
//! selects the strip and opens the menu.

use super::layout::WidthMode;
use super::strip::Pass;
use super::MixerView;
use crate::app::CantusApp;
use crate::kit::color::with_alpha;
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Tok};
use cantus_core::StripId;
use egui::{pos2, vec2, Rect, Sense, Ui};

/// The tokens the plate paints with (for inspect mode).
const TOKENS: &[Tok] = &[Tok::TextDim, Tok::Text, Tok::Label, Tok::Selection];

/// Draw and sense the plate of `strip` in its meter well `well`.
pub(super) fn plate(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    strip: StripId,
    well: Rect,
) {
    let compact = pass.mode() == WidthMode::Compact;
    // Wide and Extra large: the plate spans the strip (its meter is silent, and so is
    // everything beside it), so the words can lie flat; Compact keeps to the meter.
    let plate = if compact {
        well
    } else {
        let w = (pass.prefs.width.pitch() - 9.0).max(well.width());
        Rect::from_center_size(well.center(), vec2(w, well.height()))
    };
    let r = plate.intersect(pass.clip);
    if r.width() < 4.0 || r.height() < 12.0 {
        return;
    }
    let s = pass.s;
    let t = &s.t;
    // Published (hover only) so tools and tests find it; the click is read below.
    let _ = ui.interact(r, crate::views::route_menu::plate_id(strip), Sense::hover());
    let dragging = super::drag::active(v);
    let hovered = !dragging && ui.rect_contains_pointer(r);
    let pr = b.snap_rect(plate.shrink2(vec2(1.0, 1.0)));
    b.register(pr, "route_here", TOKENS, &[]);
    if hovered {
        b.rrect(pr, s.r(3.0), Fill::Solid(with_alpha(t.selection, 0.14)));
    }
    // Quiet at rest (a fresh project has a dozen of them), plain when pointed at.
    let ink = if hovered {
        t.text
    } else {
        with_alpha(t.text_dim, 0.6)
    };
    dashed(b, pr, with_alpha(ink, if hovered { 1.0 } else { 0.55 }));
    let plus = (pr.width() - 6.0).clamp(8.0, 12.0);
    let room = pr.shrink2(vec2(3.0, 3.0));
    // The words in the label role, else the micro one; flat when they fit across, up the
    // strip when they fit along it, else not at all (never cut: the + and the hint say
    // the rest).
    let mut flat = None;
    let mut along = None;
    if !compact {
        for role in [Role::Label, Role::Micro] {
            let words = Text::key("routing.here.plate", role, ink).caps(true);
            let size = text::measure(b.ctx(), &words, s.tk());
            if size.x <= room.width() && room.height() >= plus + size.y + 6.0 {
                flat = Some((words, size));
                break;
            }
            if along.is_none() && size.x <= room.height() - plus - 8.0 {
                along = Some(words);
            }
        }
    }
    if let Some((words, size)) = flat {
        // The + over the words, the pair centred in the plate.
        let h = plus + 4.0 + size.y;
        let top = room.center().y - h * 0.5;
        let g = Rect::from_center_size(pos2(room.center().x, top + plus * 0.5), vec2(plus, plus));
        glyph::paint(b, g, Glyph::Plus, ink);
        let wr = Rect::from_min_max(
            pos2(room.left(), top + plus + 4.0),
            pos2(room.right(), top + h),
        );
        let _ = text::paint(b, wr, &words.align(egui::Align2::CENTER_CENTER), s.tk());
    } else {
        let top = pr.top() + 4.0;
        let g = Rect::from_center_size(pos2(pr.center().x, top + plus * 0.5), vec2(plus, plus));
        glyph::paint(b, g, Glyph::Plus, ink);
        if let Some(words) = along {
            // A narrow strip: the words run up it, as its name does.
            let wr = Rect::from_min_max(
                pos2(pr.left() + 1.0, top + plus + 6.0),
                pos2(pr.right() - 1.0, pr.bottom() - 4.0),
            );
            let tx = words
                .align(egui::Align2::CENTER_TOP)
                .policy(Policy::Rotate90(Elide::End));
            let _ = text::paint(b, wr, &tx, s.tk());
        }
    }
    if !hovered {
        return;
    }
    v.drag.plate = Some((strip, r));
    let (clicked, at) = ui.input(|i| (i.pointer.primary_clicked(), i.pointer.interact_pos()));
    if clicked {
        v.drag.restore = Some((
            app.selected_strip,
            app.selected_strips.clone(),
            crate::views::inspector::subject(app),
        ));
        let at = at.unwrap_or(r.center());
        crate::views::route_menu::open_route_here(ui.ctx(), super::route_owner(), strip, at);
    }
}

/// A dashed outline round `r` (3 on, 3 off), so the plate reads as a place to put
/// something, not as a control that is already set.
fn dashed(b: &mut Brush, r: Rect, ink: egui::Color32) {
    let w = b.px().max(1.0);
    let (dash, gap) = (3.0, 3.0);
    let edges = [
        (r.left_top(), r.right_top()),
        (r.right_top(), r.right_bottom()),
        (r.right_bottom(), r.left_bottom()),
        (r.left_bottom(), r.left_top()),
    ];
    for (a, z) in edges {
        let len = a.distance(z);
        if len <= 0.0 {
            continue;
        }
        let dir = (z - a) / len;
        let mut d = 0.0;
        while d < len {
            let e = (d + dash).min(len);
            b.segment(a + dir * d, a + dir * e, w, ink, false);
            d += dash + gap;
        }
    }
}
