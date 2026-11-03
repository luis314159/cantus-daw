//! The desk's VCA faders (round 36, ADR 0093): a VCA strip is a strip with no audio — its
//! number, its name, a "VCA" plate where the meter would be, mute and solo, and the
//! fader — and a strip that follows one wears a bar of the VCA's colour under its number,
//! the way a console marks its VCA groups. What the desk derives from the project is
//! worked out once per revision ([`marks`]); a frame only reads it.

use super::layout::Row;
use super::strip::{pid, sense, Part, Pass, Snap};
use super::MixerView;
use crate::app::CantusApp;
use crate::kit::color::{safe_track, with_alpha, FillSize};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, State, Style};
use cantus_core::vca::StripKind;
use cantus_core::{Color, Project};
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

/// What a strip shows of the VCAs: derived once per revision.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VcaMark {
    /// The strip is a VCA.
    pub is_vca: bool,
    /// For a VCA: how many strips it reaches.
    pub members: u16,
    /// For a strip: how many VCAs reach it.
    pub governed: u8,
    /// The colour of the first VCA that reaches it (a VCA's own: its own colour).
    pub color: Option<Color>,
}

/// The marks of every strip, in the project's order. Keeps its capacity.
pub fn marks(p: &Project, out: &mut Vec<VcaMark>) {
    out.clear();
    out.resize(p.strips.len(), VcaMark::default());
    if !p.has_vca() {
        return;
    }
    for (i, s) in p.strips.iter().enumerate() {
        let gov = p.vca_governing(s.id);
        out[i].governed = gov.len().min(255) as u8;
        out[i].color = gov.first().map(|v| vca_color(p, *v));
        if s.kind == StripKind::Vca {
            out[i].is_vca = true;
            out[i].members = p.vca_members(s.id).len().min(65_535) as u16;
            out[i].color = Some(vca_color(p, s.id));
        }
    }
}

/// A VCA's colour: its own, or one from the palette by its place on the desk.
fn vca_color(p: &Project, id: cantus_core::StripId) -> Color {
    let s = p.strip(id);
    s.and_then(|s| s.color).unwrap_or_else(|| {
        let n = p.strips.iter().filter(|x| x.kind == StripKind::Vca);
        let at = n.take_while(|x| x.id != id).count();
        Color::PALETTE[(at + 3) % Color::PALETTE.len()]
    })
}

fn tone(c: Color, s: &Style) -> Color32 {
    safe_track(crate::theme::track_color(c), s.t.dark, FillSize::Small)
}

/// The bar under the number cell of a strip a VCA reaches (and under a VCA's own).
pub(super) fn bar(b: &mut Brush, s: &Style, cell: Rect, mark: &VcaMark) {
    let Some(c) = mark.color else {
        return;
    };
    let h = (b.px() * 3.0).max(2.0);
    let r = Rect::from_min_max(pos2(cell.left(), cell.bottom() - h), cell.right_bottom());
    b.rect(b.snap_rect(r), Fill::Solid(tone(c, s)));
}

/// A VCA strip's plate where the meter would be: "VCA" and how many strips follow it.
#[allow(clippy::too_many_arguments)]
pub(super) fn plate(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
) {
    let Some((a, z)) = pass.row(Row::Meter) else {
        return;
    };
    let s = pass.s;
    let t = &s.t;
    let mark = v.desk.vca_marks.get(sn.index).copied().unwrap_or_default();
    let well = Rect::from_min_max(pos2(x + 3.0, a), pos2(x + bw - 3.0, z));
    let ink = mark.color.map_or(t.label, |c| tone(c, s));
    b.rrect(well, s.r(3.0), Fill::Solid(with_alpha(ink, 0.10)));
    b.rrect_stroke(well, s.r(3.0), b.px(), with_alpha(ink, 0.55));
    // "VCA", up the plate when the strip is narrow, across it when it is not; below it, how
    // many strips follow.
    let word = &v.words.vca;
    let (mut name, mut count) = (well.shrink(2.0), None);
    if !pass.fit.rotated && well.height() > 40.0 {
        let split = (well.top() + 18.0).min(well.bottom());
        name = Rect::from_min_max(well.min, pos2(well.right(), split));
        count = Some(Rect::from_min_max(pos2(well.left(), split), well.max));
    }
    let nt = if pass.fit.rotated {
        Text::new(word, Role::Label, ink)
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Rotate90(Elide::Middle))
    } else {
        Text::new(word, Role::Label, ink)
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Elide(Elide::Middle))
    };
    let _ = text::paint(b, name, &nt, s.tk());
    if let Some(r) = count {
        v.buf.clear();
        use std::fmt::Write as _;
        let _ = write!(v.buf, "\u{d7}{}", mark.members);
        let ct = Text::new(&v.buf, Role::Value, t.text_dim).align(Align2::CENTER_TOP);
        let _ = text::paint(b, r.shrink2(vec2(0.0, 2.0)), &ct, s.tk());
    }
    if let Some(resp) = sense(
        ui,
        pass,
        well,
        pid(Some(sn.id), Part::Meter),
        Sense::click(),
    ) {
        let resp = resp.on_hover_text(&v.words.vca_tip);
        if resp.clicked() {
            super::strip::click_select(app, sn.id, pass.mods);
        }
        let _ = State::REST;
    }
}

/// The names of the VCAs that reach a strip, for the hover on its number.
pub fn names(p: &Project, id: cantus_core::StripId) -> String {
    p.vca_governing(id)
        .iter()
        .filter_map(|v| p.strip(*v))
        .map(|s| {
            if s.name.is_empty() {
                format!("VCA {}", p.strip_number(s.id).unwrap_or(0))
            } else {
                s.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The title row's *Snapshots* field: a menu with how many there are. Returns the next x.
/// It gives way (draws nothing) when it would leave less than `reserve` for the row's right
/// side (the title, the desk-wide switches), the way the cable options' key does.
#[allow(clippy::too_many_arguments)]
pub(super) fn snap_key(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    r: Rect,
    x: f32,
    reserve: f32,
) -> f32 {
    use std::fmt::Write as _;
    let n = app.project().mix.snapshots.len();
    v.buf.clear();
    let _ = if n == 0 {
        write!(v.buf, "{}", v.words.snap)
    } else {
        write!(v.buf, "{} {n}", v.words.snap)
    };
    let tt = Text::new(&v.buf, Role::Control, s.t.text);
    let w = text::measure(ui.ctx(), &tt, s.tk()).x + 30.0;
    if x + w + reserve > r.right() {
        return x;
    }
    let kr = Rect::from_min_size(pos2(x, r.center().y - 10.0), vec2(w, 20.0));
    let resp = ui.interact(
        kr,
        egui::Id::new(("mixer-title", "snapshots")),
        Sense::click(),
    );
    let open = v.popups.is_open(super::menu::Menu::Snapshots);
    crate::kit::well::menu_field(b, kr, s, State::of(&resp, open), &v.buf, false);
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &v.words.snap_tip,
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    // A snapshot being renamed: the name box opens under this field.
    if matches!(v.rename, Some((super::Renaming::Snapshot(_), _))) {
        v.rename_at = Some(kr);
    }
    let resp = resp.on_hover_text(&v.words.snap_tip);
    if resp.clicked() {
        v.popups.open_from(
            super::menu::Menu::Snapshots,
            pos2(kr.left(), r.bottom()),
            Some(kr),
        );
    }
    kr.right() + 8.0
}

/// Open the snapshots menu at `at` (the title row's field does, with a click).
pub fn open_snapshots(app: &mut CantusApp, at: egui::Pos2) {
    app.mixer.popups.open(super::menu::Menu::Snapshots, at);
}
