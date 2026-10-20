//! Exemplar's map: keys across, velocity up, one box per zone — labels level and elided
//! (never rotated), fades drawn as gradients inside the zone, stacked zones (round robins,
//! microphones) badged with their count, sounding zones lit. The display is the control:
//! drag a zone to move it, its edges to resize it; click again on a stack to step through
//! it; arrows move the selected zone, Shift-arrows resize it, Tab steps, Delete removes.
//! The zone card beside it and the table under it edit the same numbers.
//!
//! Drawn with the kit's brush, in tokens (ADR 0024 §8): the card's display ground, the
//! device hue for the zones, the key strip in the keyboard's own tokens.

use super::parts::{self, write_pan};
use super::{remove_zone_command, State};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band::CardLook;
use crate::kit::color::{shade, with_alpha};
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Function, Style};
use cantus_core::inst::sampler::{self as model, zf, Zone, ZONE_PARAMS};
use cantus_core::{pitch_name, Command, MergeKey, NativeSpec, TrackId};
use cantus_engine::inst::sampler::tel;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::fmt::Write as _;

/// Points from an edge that grab the edge rather than the zone.
const EDGE: f32 = 5.0;
/// Height of the key strip under the map, at 100 %.
pub const BAND: f32 = 12.0;

/// The map's geometry.
#[derive(Copy, Clone, Debug)]
pub struct Geo {
    rect: Rect,
    k0: u8,
    k1: u8,
}

impl Geo {
    fn key_w(&self) -> f32 {
        self.rect.width() / (self.k1 as f32 - self.k0 as f32 + 1.0)
    }
    fn vel_h(&self) -> f32 {
        self.rect.height() / 127.0
    }
    fn key_x(&self, k: f32) -> f32 {
        self.rect.left() + (k - self.k0 as f32) * self.key_w()
    }
    fn vel_y(&self, v: f32) -> f32 {
        self.rect.bottom() - (v - 1.0) * self.vel_h()
    }
    pub fn key_at(&self, p: Pos2) -> Option<u8> {
        if p.x < self.rect.left() || p.x > self.rect.right() {
            return None;
        }
        let k = self.k0 as f32 + ((p.x - self.rect.left()) / self.key_w()).floor();
        Some(k.clamp(0.0, 127.0) as u8)
    }
    fn zone_rect(&self, z: &Zone) -> Rect {
        Rect::from_min_max(
            pos2(self.key_x(z.lo() as f32), self.vel_y(z.vhi() as f32 + 1.0)),
            pos2(self.key_x(z.hi() as f32 + 1.0), self.vel_y(z.vlo() as f32)),
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Part {
    Body,
    Lo,
    Hi,
    VLo,
    VHi,
}

/// A zone being dragged.
#[derive(Copy, Clone, Debug)]
pub struct Drag {
    zone: usize,
    part: Part,
    origin: Pos2,
    start: [f32; 4],
}

fn part_at(r: Rect, p: Pos2) -> Part {
    if (p.x - r.left()).abs() < EDGE {
        Part::Lo
    } else if (p.x - r.right()).abs() < EDGE {
        Part::Hi
    } else if (p.y - r.top()).abs() < EDGE {
        Part::VHi
    } else if (p.y - r.bottom()).abs() < EDGE {
        Part::VLo
    } else {
        Part::Body
    }
}

/// Set some of a zone's fields as one command (merged into the gesture when `merge`):
/// a field back at its default is cleared rather than stored.
pub fn set_fields(
    app: &mut CantusApp,
    track: TrackId,
    spec: &NativeSpec,
    n: usize,
    fields: &[(usize, f32)],
    merge: bool,
) {
    let mut commands = Vec::new();
    for &(f, v) in fields {
        let key = format!("z{n}.{}", model::zone_field_name(f));
        let info = &ZONE_PARAMS[f];
        let v = v.clamp(info.min, info.max);
        let v = if info.stepped { v.round() } else { v };
        if v == info.default && !spec.is_stored(&key) {
            continue;
        }
        if spec.is_stored(&key) && spec.get(&key) == v {
            continue;
        }
        commands.push(if v == info.default {
            Command::ClearInstrumentValue { track, key }
        } else {
            Command::SetInstrumentValue {
                track,
                key,
                value: v,
            }
        });
    }
    if commands.is_empty() {
        return;
    }
    let cmd = Command::Batch {
        label_key: "cmd.smp_zone".into(),
        commands,
    };
    if merge {
        app.exec_merging(cmd, MergeKey::of(&("smp-zone", track.0, n)));
    } else {
        app.exec(cmd);
    }
}

/// The name a zone shows: its file's stem, or its built-in sound.
pub fn zone_name(z: &Zone) -> String {
    if let Some(n) = &z.name {
        return n.clone();
    }
    match &z.sample {
        Some(p) => std::path::Path::new(p)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.clone()),
        None => cantus_dsp::exemplar::builtin::get(z.get(zf::GEN) as u8)
            .map(|b| t(&format!("smp.gen.{}", b.key)))
            .unwrap_or_default(),
    }
}

fn fit(zones: &[(usize, Zone)], state: &State) -> (u8, u8) {
    if let Some(k) = state.keys {
        return k;
    }
    if zones.is_empty() {
        return (36, 96);
    }
    let lo = zones.iter().map(|(_, z)| z.lo()).min().unwrap_or(0) as i32 - 2;
    let hi = zones.iter().map(|(_, z)| z.hi()).max().unwrap_or(127) as i32 + 2;
    let (mut lo, mut hi) = (lo.max(0), hi.min(127));
    while hi - lo < 24 {
        lo = (lo - 1).max(0);
        hi = (hi + 1).min(127);
    }
    (lo as u8, hi as u8)
}

/// Zones sounding now, from the telemetry.
pub fn sounding(app: &CantusApp) -> Vec<(usize, f64)> {
    let t = app.inst_telemetry();
    let n = (t[tel::PLAYING] as usize).min(tel::MAX_PAIRS);
    (0..n)
        .map(|i| {
            (
                t[tel::FIRST_PAIR + 2 * i] as usize,
                t[tel::FIRST_PAIR + 2 * i + 1] as f64,
            )
        })
        .collect()
}

fn is_black(k: u8) -> bool {
    matches!(k % 12, 1 | 3 | 6 | 8 | 10)
}

/// The zones' fills at rest, selected and sounding, over the display's ink.
struct Fills {
    rest: egui::Color32,
    sel: egui::Color32,
    lit: egui::Color32,
    faint: egui::Color32,
}

fn fills(s: &Style) -> Fills {
    let dark = s.t.dark;
    Fills {
        rest: with_alpha(s.hue, if dark { 0.26 } else { 0.20 }),
        sel: with_alpha(s.hue, if dark { 0.46 } else { 0.38 }),
        lit: with_alpha(s.hue, if dark { 0.66 } else { 0.55 }),
        faint: with_alpha(s.hue, 0.05),
    }
}

/// A horizontal fade as vertical strips (the brush's ramps run top to bottom).
fn h_fade(b: &mut Brush, r: Rect, from: egui::Color32, to: egui::Color32) {
    let n = ((r.width() / 3.0).ceil() as usize).clamp(1, 24);
    let w = r.width() / n as f32;
    for i in 0..n {
        let c = from.lerp_to_gamma(to, (i as f32 + 0.5) / n as f32);
        b.rect(
            Rect::from_min_size(pos2(r.left() + w * i as f32, r.top()), vec2(w, r.height())),
            Fill::Solid(c),
        );
    }
}

/// Paint the map into `geo.rect` and its key strip into `band`: black-key columns, octave
/// and velocity lines, the zones with their fades, outlines, roots and names, the held keys.
#[allow(clippy::too_many_arguments)]
fn paint_map(
    b: &mut Brush,
    geo: &Geo,
    band: Rect,
    zones: &[(usize, Zone)],
    selected: Option<usize>,
    lit: &[(usize, f64)],
    held: &[u8],
    s: &Style,
) {
    use crate::kit::Tok;
    let t = &s.t;
    let k = s.k;
    let r = geo.rect;
    b.register(
        r.union(band),
        "smp.map",
        &[
            Tok::Ink,
            Tok::Grid,
            Tok::GridStrong,
            Tok::Text,
            Tok::TextDim,
            Tok::KeyWhite,
            Tok::KeyBlack,
        ],
        &[s.hue_tok],
    );
    // Black-key columns a shade off the ink; octave lines; velocity quarters.
    let col = shade(t.ink, if t.dark { -0.025 } else { -0.03 });
    for key in geo.k0..=geo.k1 {
        let x = geo.key_x(key as f32);
        if is_black(key) {
            b.rect(
                Rect::from_min_max(pos2(x, r.top()), pos2(x + geo.key_w(), r.bottom())),
                Fill::Solid(col),
            );
        }
        if key % 12 == 0 && key > geo.k0 {
            b.vline(x, r.top(), r.bottom(), b.px(), t.grid_strong);
        }
    }
    for v in [32.0, 64.0, 96.0] {
        b.hline(r.left(), r.right(), geo.vel_y(v), b.px(), t.grid);
    }
    let f = fills(s);
    let fill_of = |n: usize| {
        if lit.iter().any(|(zi, _)| *zi == n) {
            f.lit
        } else if selected == Some(n) {
            f.sel
        } else {
            f.rest
        }
    };
    for (n, z) in zones {
        let zr = geo.zone_rect(z).intersect(r);
        if zr.width() <= 0.0 || zr.height() <= 0.0 {
            continue;
        }
        let c = fill_of(*n);
        // The fades: the part of the zone that fades in or out ramps from the map to the fill.
        let (vfin, vfout) = (
            z.get(zf::VFIN) * geo.vel_h(),
            z.get(zf::VFOUT) * geo.vel_h(),
        );
        let (kfin, kfout) = (
            z.get(zf::KFIN) * geo.key_w(),
            z.get(zf::KFOUT) * geo.key_w(),
        );
        let core = Rect::from_min_max(
            pos2(zr.left() + kfin, zr.top() + vfout),
            pos2(zr.right() - kfout, zr.bottom() - vfin),
        );
        if core.width() > 0.0 && core.height() > 0.0 {
            b.rect(core, Fill::Solid(c));
        }
        if vfin > 0.0 {
            b.rect(
                Rect::from_min_max(pos2(zr.left(), zr.bottom() - vfin), zr.right_bottom()),
                Fill::V(c, f.faint),
            );
        }
        if vfout > 0.0 {
            b.rect(
                Rect::from_min_max(zr.left_top(), pos2(zr.right(), zr.top() + vfout)),
                Fill::V(f.faint, c),
            );
        }
        if kfin > 0.0 {
            h_fade(
                b,
                Rect::from_min_max(zr.left_top(), pos2(zr.left() + kfin, zr.bottom())),
                f.faint,
                c,
            );
        }
        if kfout > 0.0 {
            h_fade(
                b,
                Rect::from_min_max(pos2(zr.right() - kfout, zr.top()), zr.right_bottom()),
                c,
                f.faint,
            );
        }
    }
    // Outlines, roots and names over every fill, so overlapping zones stay legible.
    let mut labelled: Vec<Rect> = Vec::new();
    for (n, z) in zones {
        let zr = geo.zone_rect(z).intersect(r);
        if zr.width() <= 0.0 || zr.height() <= 0.0 {
            continue;
        }
        let hot = selected == Some(*n) || lit.iter().any(|(zi, _)| zi == n);
        let (w, c) = if hot {
            (1.5 * k, shade(s.hue, if t.dark { 0.12 } else { -0.08 }))
        } else {
            (b.px(), with_alpha(s.hue, 0.85))
        };
        b.rrect_stroke(b.snap_rect(zr).shrink(w * 0.5), 1.0 * k, w, c);
        let rx = geo.key_x(z.root() as f32 + 0.5);
        if rx >= zr.left() && rx <= zr.right() {
            let y = zr.bottom();
            b.polygon(
                &[
                    pos2(rx - 3.5 * k, y),
                    pos2(rx + 3.5 * k, y),
                    pos2(rx, y - 5.0 * k),
                ],
                t.text,
            );
        }
        // One name per stack of identical boxes, with the count.
        if labelled
            .iter()
            .any(|l| (l.min - zr.min).length() < 0.5 && (l.max - zr.max).length() < 0.5)
        {
            continue;
        }
        labelled.push(zr);
        let stack = zones
            .iter()
            .filter(|(_, o)| {
                let or = geo.zone_rect(o).intersect(r);
                (or.min - zr.min).length() < 0.5 && (or.max - zr.max).length() < 0.5
            })
            .count();
        let box_h = 14.0 * k;
        if zr.width() > 20.0 * k && zr.height() > box_h + 2.0 * k {
            let mut name = zone_name(z);
            if stack > 1 {
                name = format!("\u{00d7}{stack} {name}");
            }
            let nr = Rect::from_min_size(
                zr.min + vec2(4.0 * k, 2.0 * k),
                vec2(zr.width() - 8.0 * k, box_h),
            );
            // The label face when the name fits; the micro face when it does not; then
            // two lines, split at a space, in a tall narrow zone; elided in the middle (as
            // names are) only past that.
            let mut nt = Text::new(&name, Role::Label, t.text)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle));
            let fits =
                |b: &Brush, nt: &Text<'_>| ktext::measure(b.ctx(), nt, s.tk()).x <= nr.width();
            if !fits(b, &nt) {
                nt.role = Role::Micro;
            }
            let split = name
                .char_indices()
                .filter(|(_, c)| *c == ' ')
                .map(|(i, _)| i)
                .min_by_key(|i| (*i as i32 - name.len() as i32 / 2).abs());
            match split {
                Some(i) if !fits(b, &nt) && zr.height() > 2.0 * box_h + 4.0 * k => {
                    let (a, z) = (&name[..i], &name[i + 1..]);
                    let l2 = nr.translate(vec2(0.0, box_h - 2.0 * k));
                    for (line, r) in [(a, nr), (z, l2)] {
                        let lt = Text {
                            src: Src::Str(line),
                            ..nt
                        };
                        let _ = ktext::paint(b, r, &lt, s.tk());
                    }
                }
                _ => {
                    let _ = ktext::paint(b, nr, &nt, s.tk());
                }
            }
        }
    }
    // Velocity marks at the left edge.
    for (v, label) in [(32.0, "32"), (64.0, "64"), (96.0, "96")] {
        let y = geo.vel_y(v);
        let lr = Rect::from_min_size(
            pos2(r.left() + 3.0 * k, y - 11.0 * k),
            vec2(22.0 * k, 10.0 * k),
        );
        let _ = ktext::paint(
            b,
            lr,
            &Text::new(label, Role::Micro, t.text_dim).align(Align2::LEFT_CENTER),
            s.tk(),
        );
    }
    // The key strip: the keyboard's own tokens, held keys in the hue, every C named.
    let (white, black) = (t.key_white, t.key_black);
    b.rect(b.snap_rect(band), Fill::Solid(shade(white, -0.28)));
    for key in geo.k0..=geo.k1 {
        let x = geo.key_x(key as f32);
        let kr = Rect::from_min_max(pos2(x, band.top()), pos2(x + geo.key_w(), band.bottom()));
        let base = if held.contains(&key) {
            s.hue
        } else if is_black(key) {
            black
        } else {
            white
        };
        let face = Rect::from_min_max(kr.min, pos2(kr.right() - b.px(), kr.bottom()));
        b.rect(face, Fill::V(shade(base, -0.04), shade(base, 0.03)));
    }
    // Every C named along the map's bottom edge, just right of its octave line.
    for key in (geo.k0..=geo.k1).filter(|k| k % 12 == 0) {
        let x = geo.key_x(key as f32);
        let lr = Rect::from_min_size(
            pos2(x + 3.0 * k, r.bottom() - 13.0 * k),
            vec2((geo.key_w() * 11.0).min(28.0 * k), 12.0 * k),
        );
        let name = pitch_name(key);
        let _ = ktext::paint(
            b,
            lr,
            &Text::new(&name, Role::Micro, t.text_dim)
                .caps(false)
                .align(Align2::LEFT_CENTER),
            s.tk(),
        );
    }
}

/// The zone under a point: the selected one if it is there, else the last drawn (on top).
fn zone_at(geo: &Geo, zones: &[(usize, Zone)], p: Pos2, selected: Option<usize>) -> Option<usize> {
    let hits: Vec<usize> = zones
        .iter()
        .filter(|(_, z)| geo.zone_rect(z).expand(EDGE * 0.5).contains(p))
        .map(|(n, _)| *n)
        .collect();
    if let Some(s) = selected.filter(|s| hits.contains(s)) {
        return Some(s);
    }
    hits.last().copied()
}

/// The map in `rect` (a card's display) with its key strip along the bottom: painted, and
/// the control. `zoomed`: the keys the user zoomed to (Edit); otherwise every zone fits
/// (Play). Returns the geometry, for files dropped on it.
#[allow(clippy::too_many_arguments)]
pub fn map(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    zoomed: bool,
) -> Geo {
    let zones: Vec<_> = model::zones(spec)
        .into_iter()
        .filter(|(_, z)| !by_controller(z))
        .collect();
    let (k0, k1) = if zoomed {
        fit(&zones, &app.inst.sampler)
    } else {
        fit(&zones, &State::default())
    };
    let band_h = BAND * s.k;
    let inner = rect.shrink2(vec2(0.0, 0.0));
    let map_rect = Rect::from_min_max(inner.min, pos2(inner.max.x, inner.max.y - band_h));
    let band = Rect::from_min_max(pos2(inner.left(), map_rect.bottom()), inner.max);
    let geo = Geo {
        rect: map_rect,
        k0,
        k1,
    };
    let resp = ui.allocate_rect(map_rect, Sense::click_and_drag());
    // A sample from the browser lands on the key under the pointer (ADR 0030).
    crate::samples::drop::publish(
        ui.ctx(),
        map_rect,
        crate::samples::drop::Target::Exemplar { track, k0, k1 },
    );
    let selected = app.inst.sampler.selected;
    let lit = sounding(app);
    let held = app.inst.held.clone();
    let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty()) && resp.contains_pointer();
    let drop_key = resp.hover_pos().and_then(|p| geo.key_at(p));
    with_brush(&ui.painter_at(rect), |b| {
        paint_map(b, &geo, band, &zones, selected, &lit, &held, s);
        if zones.is_empty() {
            let _ = ktext::paint(
                b,
                map_rect.shrink(12.0 * s.k),
                &Text::key("smp.drop_here", Role::Body, s.t.text_dim)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
        // Files hovering over the map from the system: the key they would land on.
        if hovering {
            b.rrect_stroke(map_rect.shrink(1.0), 2.0 * s.k, 2.0 * s.k, s.hue);
            if let Some(k) = drop_key {
                let x = geo.key_x(k as f32);
                b.rect(
                    Rect::from_min_max(
                        pos2(x, map_rect.top()),
                        pos2(x + geo.key_w(), map_rect.bottom()),
                    ),
                    Fill::Solid(with_alpha(s.hue, 0.4)),
                );
            }
        }
    });
    // Hover: which edge a drag would take, and the zone in the help line.
    if let Some(p) = resp.hover_pos() {
        if let Some(n) = zone_at(&geo, &zones, p, selected) {
            let z = &zones.iter().find(|(i, _)| *i == n).unwrap().1;
            let icon = match part_at(geo.zone_rect(z), p) {
                Part::Lo | Part::Hi => egui::CursorIcon::ResizeHorizontal,
                Part::VLo | Part::VHi => egui::CursorIcon::ResizeVertical,
                Part::Body => egui::CursorIcon::Grab,
            };
            ui.ctx().set_cursor_icon(icon);
            crate::kit::hint::set_text(ui.ctx(), &zone_tip(n, z), "", crate::kit::hint::Caps::NONE);
        }
    }
    if resp.clicked() {
        resp.request_focus();
        if let Some(p) = resp.interact_pointer_pos() {
            // Clicking the selected zone again steps through the zones stacked under it.
            let under: Vec<usize> = zones
                .iter()
                .filter(|(_, z)| geo.zone_rect(z).contains(p))
                .map(|(n, _)| *n)
                .collect();
            app.inst.sampler.selected = match (selected, under.as_slice()) {
                (_, []) => selected,
                (Some(s), u) if u.contains(&s) => {
                    let i = u.iter().position(|x| *x == s).unwrap_or(0);
                    Some(u[(i + 1) % u.len()])
                }
                (_, u) => u.last().copied(),
            };
            app.inst.sampler.view = None;
        }
    }
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(n) = zone_at(&geo, &zones, p, selected) {
                let z = &zones.iter().find(|(i, _)| *i == n).unwrap().1;
                app.inst.sampler.selected = Some(n);
                app.inst.sampler.drag = Some(Drag {
                    zone: n,
                    part: part_at(geo.zone_rect(z), p),
                    origin: p,
                    start: [z.get(zf::LO), z.get(zf::HI), z.get(zf::VLO), z.get(zf::VHI)],
                });
            }
        }
    }
    if let (Some(d), Some(p)) = (app.inst.sampler.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let dk = ((p.x - d.origin.x) / geo.key_w()).round();
            let dv = (-(p.y - d.origin.y) / geo.vel_h()).round();
            let [lo, hi, vlo, vhi] = d.start;
            let fields: Vec<(usize, f32)> = match d.part {
                Part::Body => {
                    let dk = dk.clamp(-lo, 127.0 - hi);
                    let dv = dv.clamp(1.0 - vlo, 127.0 - vhi);
                    vec![
                        (zf::LO, lo + dk),
                        (zf::HI, hi + dk),
                        (zf::VLO, vlo + dv),
                        (zf::VHI, vhi + dv),
                    ]
                }
                Part::Lo => vec![(zf::LO, (lo + dk).clamp(0.0, hi))],
                Part::Hi => vec![(zf::HI, (hi + dk).clamp(lo, 127.0))],
                Part::VLo => vec![(zf::VLO, (vlo + dv).clamp(1.0, vhi))],
                Part::VHi => vec![(zf::VHI, (vhi + dv).clamp(vlo, 127.0))],
            };
            set_fields(app, track, spec, d.zone, &fields, true);
        }
    }
    if resp.drag_stopped() {
        app.inst.sampler.drag = None;
        app.end_gesture();
    }
    // Ctrl + wheel zooms the keys around the pointer.
    if zoomed && resp.hovered() {
        let (zoom, ctrl) = ui
            .ctx()
            .input(|i| (i.smooth_scroll_delta.y, i.modifiers.ctrl));
        if ctrl && zoom != 0.0 {
            if let Some(k) = resp.hover_pos().and_then(|p| geo.key_at(p)) {
                let span = (k1 - k0) as f32 * if zoom > 0.0 { 0.85 } else { 1.18 };
                let span = span.clamp(12.0, 127.0);
                let f = (k as f32 - k0 as f32) / (k1 - k0).max(1) as f32;
                let lo = (k as f32 - f * span).clamp(0.0, 127.0 - span);
                app.inst.sampler.keys = Some((lo as u8, (lo + span).min(127.0) as u8));
            }
        }
    }
    if resp.has_focus() {
        keys(app, ui, track, spec, &zones);
    }
    geo
}

fn zone_tip(n: usize, z: &Zone) -> String {
    tf(
        "smp.zone_tip",
        &[
            ("n", &n.to_string()),
            ("name", &zone_name(z)),
            ("lo", &pitch_name(z.lo())),
            ("hi", &pitch_name(z.hi())),
            ("root", &pitch_name(z.root())),
            ("vlo", &z.vlo().to_string()),
            ("vhi", &z.vhi().to_string()),
        ],
    )
}

/// Keyboard editing of the selected zone.
fn keys(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    zones: &[(usize, Zone)],
) {
    let Some(n) = app.inst.sampler.selected else {
        return;
    };
    let Some(z) = zones.iter().find(|(i, _)| *i == n).map(|(_, z)| z.clone()) else {
        return;
    };
    use egui::Key;
    let (shift, pressed) = ui.ctx().input(|i| {
        let keys = [
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::Tab,
            Key::Delete,
        ];
        (i.modifiers.shift, keys.map(|k| i.key_pressed(k)))
    });
    let [left, right, up, down, tab, delete] = pressed;
    let (lo, hi, vlo, vhi) = (z.get(zf::LO), z.get(zf::HI), z.get(zf::VLO), z.get(zf::VHI));
    let dk = if right {
        1.0
    } else if left {
        -1.0
    } else {
        0.0
    };
    let dv = if up {
        1.0
    } else if down {
        -1.0
    } else {
        0.0
    };
    if dk != 0.0 {
        let fields = if shift {
            vec![(zf::HI, (hi + dk).clamp(lo, 127.0))]
        } else {
            let dk = dk.clamp(-lo, 127.0 - hi);
            vec![(zf::LO, lo + dk), (zf::HI, hi + dk)]
        };
        set_fields(app, track, spec, n, &fields, false);
    }
    if dv != 0.0 {
        let fields = if shift {
            vec![(zf::VHI, (vhi + dv).clamp(vlo, 127.0))]
        } else {
            let dv = dv.clamp(1.0 - vlo, 127.0 - vhi);
            vec![(zf::VLO, vlo + dv), (zf::VHI, vhi + dv)]
        };
        set_fields(app, track, spec, n, &fields, false);
    }
    if tab {
        let i = zones.iter().position(|(i, _)| *i == n).unwrap_or(0);
        app.inst.sampler.selected = Some(zones[(i + 1) % zones.len()].0);
        app.inst.sampler.view = None;
    }
    if delete {
        app.exec(remove_zone_command(track, spec, n));
        app.inst.sampler.selected = None;
    }
}

pub const LOOP_MODES: [&str; 4] = [
    "smp.mode.no_loop",
    "smp.mode.one_shot",
    "smp.mode.continuous",
    "smp.mode.sustain",
];
pub const LOOP_DIRS: [&str; 3] = ["smp.dir.forward", "smp.dir.pingpong", "smp.dir.backward"];
pub const TRIGGERS: [&str; 6] = [
    "smp.trig.attack",
    "smp.trig.release",
    "smp.trig.first",
    "smp.trig.legato",
    "smp.trig.release_key",
    "smp.trig.controller",
];

/// Whether a controller starts the zone (round 26: pedal noises) — no key plays it, so
/// the map leaves it out and its card shows the controller and range instead.
pub fn by_controller(z: &Zone) -> bool {
    model::Trigger::from_index(z.get(zf::TRIG)) == model::Trigger::Controller
}

/// A zone field's readout: notes by name, pan by side, whole numbers where they step.
pub fn write_zone(f: usize, v: f32, b: &mut String) {
    let info = &ZONE_PARAMS[f];
    match f {
        zf::LO | zf::HI | zf::ROOT | zf::SW => {
            if v < 0.0 {
                b.push('\u{2014}');
            } else {
                b.push_str(&pitch_name(v.round() as u8));
            }
        }
        zf::PAN => write_pan(v, b),
        zf::RLO | zf::RHI => {
            let _ = write!(b, "{v:.2}");
        }
        _ => parts::write_value(v, info.unit, info.stepped, b),
    }
}

/// The zone card's cells, three rows of eight: ranges and root, tracking, group and delay,
/// tuning and level with the fades, then round robin, random range, exclusivity and keyswitch.
pub const ZONE_FIELDS: [usize; 23] = [
    zf::ROOT,
    zf::LO,
    zf::HI,
    zf::VLO,
    zf::VHI,
    zf::KEYTRACK,
    zf::GROUP,
    zf::DELAY,
    zf::TUNE,
    zf::GAIN,
    zf::PAN,
    zf::VFIN,
    zf::VFOUT,
    zf::KFIN,
    zf::KFOUT,
    zf::RTDECAY,
    zf::SEQ,
    zf::SEQLEN,
    zf::RLO,
    zf::RHI,
    zf::EXCL,
    zf::OFFBY,
    zf::SW,
];

/// The cells of a zone a controller starts: the controller and its range where the key
/// and velocity ranges were (no key plays it), then what still applies.
pub const CC_ZONE_FIELDS: [usize; 18] = [
    zf::ROOT,
    zf::CC,
    zf::CCLO,
    zf::CCHI,
    zf::GROUP,
    zf::DELAY,
    zf::TUNE,
    zf::GAIN,
    zf::PAN,
    zf::SEQ,
    zf::SEQLEN,
    zf::RLO,
    zf::RHI,
    zf::EXCL,
    zf::OFFBY,
    zf::SW,
    zf::KEYTRACK,
    zf::RTDECAY,
];

/// Cells to a row in the zone card.
pub const ZONE_PER_ROW: usize = 8;

/// The zone card's size at this style's scale: three rows of M cells and a row of controls.
pub fn zone_card_size(s: &Style) -> egui::Vec2 {
    let c = parts::card_for(s, KnobSize::M, ZONE_PER_ROW, 3);
    c + vec2(0.0, (parts::ROW + 6.0) * s.k)
}

/// One zone field as a cell: its range and readout, one undo step per drag.
#[allow(clippy::too_many_arguments)]
fn zone_cell(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    n: usize,
    z: &Zone,
    f: usize,
    rect: Rect,
    s: &Style,
) {
    let info = &ZONE_PARAMS[f];
    let name = t(info.label_key);
    // A zone's field is a parameter of the instrument (`z{n}.{field}`): automated and
    // modulated like any other (ADR 0057).
    let key = format!("z{n}.{}", model::zone_field_name(f));
    let mut value = z.get(f);
    let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let w = crate::views::inst::frame::wire(
        app,
        track,
        &key,
        &mut value,
        (info.min, info.max, false),
        s,
        &mut rings,
    );
    let (r, v, choice) = parts::num_cell_offered(
        ui,
        rect,
        s,
        egui::Id::new(("smp-zone-cell", track.0, n, f)),
        value,
        info.min..=info.max,
        info.default,
        false,
        info.stepped,
        Some(Src::Key(info.label_key)),
        &name,
        KnobSize::M,
        &rings[..w.n],
        w.on.then_some((w.automated, w.offer)),
        move |v, b| write_zone(f, v, b),
    );
    crate::views::inst::frame::wired(app, &w, choice, r.rect);
    if r.changed() {
        set_fields(app, track, spec, n, &[(f, v)], true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

/// The selected zone as a card of cells (ADR 0024 §4): its ranges, tuning, level, fades,
/// round robin and exclusivity; the trigger as a menu field; release options as LEDs;
/// Remove as an action.
pub fn zone_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let zones = model::zones(spec);
    let sel = app
        .inst
        .sampler
        .selected
        .and_then(|n| zones.iter().find(|(i, _)| *i == n))
        .cloned();
    let title = sel
        .as_ref()
        .map(|(n, z)| format!("{} \u{b7} #{n}", zone_name(z)));
    let look = CardLook {
        title: title
            .as_deref()
            .map(Src::Str)
            .unwrap_or(Src::Key("smp.card.zone")),
        switchable: false,
        selector: None,
        menu: false,
        display: None,
    };
    let cr = parts::card(ui, rect, s, &look, true);
    let Some((n, z)) = sel else {
        parts::words(
            ui,
            cr.body,
            s,
            Src::Key("smp.select_zone"),
            Role::Body,
            s.t.text_dim,
            Align2::CENTER_CENTER,
            Policy::Elide(Elide::End),
        );
        return;
    };
    let fields: &[usize] = if by_controller(&z) {
        &CC_ZONE_FIELDS
    } else {
        &ZONE_FIELDS
    };
    let rects = parts::cells(cr.body, s, KnobSize::M, fields.len(), ZONE_PER_ROW);
    for (f, r) in fields.iter().zip(rects) {
        zone_cell(app, ui, track, spec, n, &z, *f, r, s);
    }
    // The controls' row along the bottom of the body.
    let k = s.k;
    let h = parts::ROW * k;
    let row = Rect::from_min_max(pos2(cr.body.left(), cr.body.bottom() - h), cr.body.max);
    let gap = parts::GAP * k;
    let w = row.width();
    let widths = [0.34, 0.22, 0.20, 0.24];
    let mut x = row.left();
    let mut slot = |f: f32| {
        let r = Rect::from_min_size(pos2(x, row.top()), vec2(w * f - gap, h));
        x += w * f;
        r
    };
    let trig = (z.get(zf::TRIG) as usize).min(TRIGGERS.len() - 1);
    let tr = slot(widths[0]);
    let (trig_resp, c) = parts::field(
        ui,
        tr,
        s,
        &t(TRIGGERS[trig]),
        false,
        false,
        &t("smp.trigger"),
    );
    // The zone's trigger and switches are parameters (round 29, review 3 of ADR 0062, #35).
    let zkey = |f: usize| format!("z{n}.{}", model::zone_field_name(f));
    let learning = crate::controllers::stepped_inst(app, ui, track, spec, &zkey(zf::TRIG), &trig_resp);
    let c = if learning { Default::default() } else { c };
    let labels: Vec<Src<'_>> = TRIGGERS.iter().map(|k| Src::Key(k)).collect();
    let items = parts::radio_items(&labels, Some(trig));
    let id = egui::Id::new(("smp-trigger", track.0));
    if let Some(i) = parts::dropdown(ui, id, tr, s, &items, c.open) {
        set_fields(app, track, spec, n, &[(zf::TRIG, i as f32)], false);
    }
    for f in [zf::RTFOLLOW, zf::OFFMODE] {
        let on = z.get(f) >= 0.5;
        let key = ZONE_PARAMS[f].label_key;
        let r = parts::led(
            ui,
            slot(if f == zf::RTFOLLOW {
                widths[1]
            } else {
                widths[2]
            }),
            s,
            on,
            Src::Key(key),
            &t(key),
        );
        let learning = crate::controllers::stepped_inst(app, ui, track, spec, &zkey(f), &r);
        if r.clicked() && !learning {
            set_fields(
                app,
                track,
                spec,
                n,
                &[(f, if on { 0.0 } else { 1.0 })],
                false,
            );
        }
    }
    if parts::press(
        ui,
        slot(widths[3]),
        s,
        Src::Key("smp.remove_zone"),
        &t("smp.remove_zone"),
    )
    .clicked()
    {
        app.exec(remove_zone_command(track, spec, n));
        app.inst.sampler.selected = None;
    }
}

/// The table's columns: legend key and width at 100 % (`0` for the name, which takes what
/// is left).
const COLS: [(&str, f32); 12] = [
    ("smp.col.n", 26.0),
    ("smp.col.sample", 0.0),
    ("smp.col.lo", 46.0),
    ("smp.col.hi", 46.0),
    ("smp.col.root", 46.0),
    ("smp.col.vlo", 38.0),
    ("smp.col.vhi", 38.0),
    ("smp.col.tune", 60.0),
    ("smp.col.gain", 60.0),
    ("smp.col.pan", 44.0),
    ("smp.col.loop", 96.0),
    ("smp.col.rr", 40.0),
];

/// A table row's height at 100 %.
const TABLE_ROW: f32 = 22.0;

/// The zone table's height for `n` zones at this style's scale.
pub fn table_height(s: &Style, n: usize) -> f32 {
    (parts::CARD_CHROME + 16.0 + (TABLE_ROW + 2.0) * n.max(1) as f32) * s.k
}

/// Every zone as a row (TX16Wx's pairing with the map): the same numbers, each a well you
/// drag, the selected row lit. Only the rows in view are laid out.
pub fn table(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let zones = model::zones(spec);
    let look = CardLook {
        title: Src::Key("smp.card.zones"),
        switchable: false,
        selector: None,
        menu: false,
        display: None,
    };
    let s = &s.function(Function::Source);
    let cr = parts::card(ui, rect, s, &look, true);
    let k = s.k;
    let gap = parts::GAP * k;
    let fixed: f32 = COLS.iter().map(|(_, w)| w * k).sum::<f32>() + gap * (COLS.len() - 1) as f32;
    let name_w = (cr.body.width() - fixed).max(60.0 * k);
    let xs: Vec<(f32, f32)> = {
        let mut x = cr.body.left();
        COLS.iter()
            .map(|(_, w)| {
                let w = if *w == 0.0 { name_w } else { w * k };
                let at = (x, w);
                x += w + gap;
                at
            })
            .collect()
    };
    let legend_h = 14.0 * k;
    with_brush(ui.painter(), |b| {
        for ((key, _), (x, w)) in COLS.iter().zip(&xs) {
            let r = Rect::from_min_size(pos2(*x, cr.body.top()), vec2(*w, legend_h));
            let _ = ktext::paint(
                b,
                r,
                &Text::key(key, Role::Micro, s.t.text_dim).policy(Policy::Abbreviate),
                s.tk(),
            );
        }
    });
    let row_h = TABLE_ROW * k;
    let clip = ui.clip_rect();
    let top = cr.body.top() + legend_h + 2.0 * k;
    for (i, (n, z)) in zones.iter().enumerate() {
        let y = top + i as f32 * (row_h + 2.0 * k);
        let row = Rect::from_min_size(pos2(cr.body.left(), y), vec2(cr.body.width(), row_h));
        if !row.intersects(clip) {
            continue;
        }
        let sel = app.inst.sampler.selected == Some(*n);
        let cell = |c: usize| Rect::from_min_size(pos2(xs[c].0, y), vec2(xs[c].1, row_h));
        let mut pick = false;
        let mut num = String::new();
        let _ = write!(num, "{n}");
        pick |= parts::readout(ui, cell(0), s, &num, true, sel, &t("smp.col.n")).clicked();
        let name = zone_name(z);
        pick |= parts::readout(ui, cell(1), s, &name, false, sel, &name).clicked();
        let editable = [
            zf::LO,
            zf::HI,
            zf::ROOT,
            zf::VLO,
            zf::VHI,
            zf::TUNE,
            zf::GAIN,
            zf::PAN,
        ];
        for (c, f) in editable.iter().enumerate() {
            let info = &ZONE_PARAMS[*f];
            let id = egui::Id::new(("smp-table", track.0, *n, *f));
            let held = ui.data(|d| d.get_temp::<f32>(id));
            let mut v = held.unwrap_or(z.get(*f));
            let fv = *f;
            let r = parts::drag_num(
                ui,
                cell(c + 2),
                s,
                &mut v,
                info.min..=info.max,
                info.default,
                sel,
                &t(info.label_key),
                move |v, b| write_zone(fv, v, b),
            );
            if r.dragged() {
                ui.data_mut(|d| d.insert_temp(id, v));
            } else if held.is_some() {
                ui.data_mut(|d| d.remove::<f32>(id));
            }
            if r.changed() {
                set_fields(app, track, spec, *n, &[(*f, v)], true);
            }
            if r.drag_stopped() || r.double_clicked() {
                app.end_gesture();
            }
            pick |= r.clicked();
        }
        let mode = (z.get(zf::MODE) as usize).min(LOOP_MODES.len() - 1);
        pick |= parts::readout(
            ui,
            cell(10),
            s,
            &t(LOOP_MODES[mode]),
            false,
            sel,
            &t("smp.loop_mode"),
        )
        .clicked();
        let rr = if z.get(zf::SEQLEN) > 1.0 {
            format!("{}/{}", z.get(zf::SEQ), z.get(zf::SEQLEN))
        } else {
            "\u{2014}".into()
        };
        pick |= parts::readout(ui, cell(11), s, &rr, true, sel, &t("smp.seq")).clicked();
        if pick {
            app.inst.sampler.selected = Some(*n);
            app.inst.sampler.view = None;
        }
    }
    if zones.is_empty() {
        parts::words(
            ui,
            Rect::from_min_max(pos2(cr.body.left(), top), cr.body.max),
            s,
            Src::Key("smp.empty_status"),
            Role::Body,
            s.t.text_dim,
            Align2::LEFT_CENTER,
            Policy::Elide(Elide::End),
        );
    }
}

/// The inspector's read-only strip: zones as bars over the 128 keys.
pub fn mini(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    // The keys' zones: one a controller starts is no key's (as in the map).
    let all = model::zones(spec);
    let none = all.is_empty();
    let zones: Vec<_> = all.into_iter().filter(|(_, z)| !by_controller(z)).collect();
    let s = Style::current().function(Function::Source);
    let w = ui.available_width().min(240.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
    let caption = if none {
        t("smp.builtin_tone")
    } else {
        tf("smp.n_zones", &[("n", &zones.len().to_string())])
    };
    with_brush(&ui.painter_at(rect), |b| {
        let r = b.snap_rect(rect);
        crate::kit::light::sunken(b, r, s.r(3.0), s.t.ink, 1.0, s.t.depth);
        let x = |k: f32| r.left() + k / 128.0 * r.width();
        let y = |v: f32| r.bottom() - v / 127.0 * r.height();
        for (_, z) in &zones {
            let zr = Rect::from_min_max(
                pos2(x(z.lo() as f32), y(z.vhi() as f32)),
                pos2(x(z.hi() as f32 + 1.0), y(z.vlo() as f32 - 1.0)),
            );
            b.rect(zr, Fill::Solid(with_alpha(s.hue, 0.3)));
            b.rrect_stroke(zr, 0.0, b.px(), with_alpha(s.hue, 0.85));
        }
        let _ = ktext::paint(
            b,
            Rect::from_min_size(r.min + vec2(4.0, 2.0), vec2(r.width() - 8.0, 14.0)),
            &Text::new(&caption, Role::Micro, s.t.text)
                .caps(false)
                .align(Align2::LEFT_CENTER),
            s.tk(),
        );
    });
    resp
}
