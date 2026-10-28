//! **Cantus's view of a hosted plug-in** (ADR 0073, BACKLOG §6h point 2): its parameters
//! drawn with the kit, for an insert (inside the generic editor, `views::plugins`) and for
//! an instrument (in a device frame of its own, `plugin_view_inst`).
//!
//! - **Grouped by the plug-in's modules or pages** (CLAP `module`, VST3 units), each group a
//!   card, the cards flowing left to right and wrapping.
//! - **Searchable**: a field filters by name and by group; every word typed must appear.
//! - **Meters where the plug-in reports them**: a parameter it marks read-only (CLAP
//!   `CLAP_PARAM_IS_READONLY`, VST3 `kIsReadOnly` — how both standards send a host a gain
//!   reduction or a level) is drawn as a meter, read every frame, never as a knob. Neither
//!   standard has a meter of its own beyond that.
//! - **Every control with the parameter menu of our own units**: *Modulate…*, MIDI learn,
//!   automation (ADR 0057, ADR 0062), through the same cell and the same offer.
//! - **Only the rows in view are drawn**: a 500-parameter plug-in scrolls in the frame
//!   budget (`frame_budget.rs`).

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band;
use crate::kit::text::Src;
use crate::kit::Style;
use crate::views::fx::controls::{self, Size};
use crate::views::plugin_notice::Target;
use cantus_core::{AutoTarget, Command, EffectId, MergeKey, StripId, TrackId};
use cantus_host::ParamInfo;
use egui::Sense;

/// The plug-in a view shows: an insert on a strip, or a track's instrument.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Subject {
    Effect(StripId, EffectId),
    Instrument(TrackId),
}

impl Subject {
    pub fn target(self) -> Target {
        match self {
            Subject::Effect(_, e) => Target::Insert(e),
            Subject::Instrument(t) => Target::Instrument(t),
        }
    }

    /// The automation target of its parameter `id`.
    pub fn auto(self, id: u32) -> AutoTarget {
        match self {
            Subject::Effect(_, effect) => AutoTarget::Plugin { effect, id },
            Subject::Instrument(track) => AutoTarget::PluginInstrument { track, id },
        }
    }

    /// The plug-in's parameters as loaded.
    pub fn params(self, app: &CantusApp) -> Option<&[ParamInfo]> {
        match self {
            Subject::Effect(_, e) => app.plugins.hosted(e).map(|h| h.params.as_slice()),
            Subject::Instrument(t) => app
                .plugins
                .hosted_instrument(t)
                .map(|h| h.params.as_slice()),
        }
    }

    /// The value the project stores for a parameter.
    fn stored(self, app: &CantusApp, id: u32) -> Option<f64> {
        app.hosted_record(self.target())
            .and_then(|r| r.params.get(&id).copied())
    }

    /// The value the plug-in holds now.
    fn live(self, app: &CantusApp, id: u32) -> Option<f64> {
        match self {
            Subject::Effect(_, e) => app.plugins.hosted(e).and_then(|h| h.param_value(id)),
            Subject::Instrument(t) => app
                .plugins
                .hosted_instrument(t)
                .and_then(|h| h.param_value(id)),
        }
    }

    /// The plug-in's own words for a value.
    fn text(self, app: &CantusApp, id: u32, v: f64) -> Option<String> {
        match self {
            Subject::Effect(_, e) => app.plugins.hosted(e).and_then(|h| h.param_text(id, v)),
            Subject::Instrument(t) => app
                .plugins
                .hosted_instrument(t)
                .and_then(|h| h.param_text(id, v)),
        }
    }

    /// Whether it answers later (an isolated plug-in: "no words" is "not yet").
    fn later(self, app: &CantusApp) -> bool {
        let iso = match self {
            Subject::Effect(_, e) => app.plugins.hosted(e).map(|h| h.isolation()),
            Subject::Instrument(t) => app.plugins.hosted_instrument(t).map(|h| h.isolation()),
        };
        iso.is_some_and(|i| i != cantus_core::PluginIsolation::Within)
    }

    /// The key its words are kept under: the instance too, for an instrument (another
    /// plug-in on the track has its own words for the same ids).
    fn words_key(self, app: &CantusApp) -> (bool, u64) {
        match self {
            Subject::Effect(_, e) => (false, e.0),
            Subject::Instrument(t) => {
                let instance = app.plugins.hosted_instrument(t).map_or(0, |h| h.instance());
                (true, t.0.rotate_left(32) ^ instance)
            }
        }
    }

    /// Set a parameter: a gesture merges frame to frame into one undo step.
    pub fn set(self, app: &mut CantusApp, id: u32, value: f64) {
        match self {
            Subject::Effect(strip, effect) => app.set_plugin_param(strip, effect, id, value, true),
            Subject::Instrument(track) => {
                app.exec_merging(
                    Command::SetInstrumentPluginParam {
                        track,
                        id,
                        value: Some(value),
                    },
                    MergeKey::of(&("inst-plugin-param", track.0, id)),
                );
            }
        }
    }
}

/// Whether a parameter matches what was typed: every word in its name or its group.
pub fn matches(p: &ParamInfo, words: &[String]) -> bool {
    words
        .iter()
        .all(|w| contains_ci(&p.name, w) || contains_ci(&p.group, w))
}

/// Whether `hay` holds `needle` (lower case already), ignoring case; no allocation for an
/// ASCII name.
fn contains_ci(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !hay.is_ascii() || !needle.is_ascii() {
        return hay.to_lowercase().contains(needle);
    }
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    h.len() >= n.len()
        && h.windows(n.len())
            .any(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

/// Whether a parameter is shown at all: not hidden, and with a range.
fn shown(p: &ParamInfo) -> bool {
    !p.hidden && p.max != p.min
}

/// The parameters' groups: a title and the indices of the parameters in it.
pub type Groups = Vec<(String, Vec<usize>)>;

/// The plug-in's parameters in their groups, in its order, filtered by `search`.
pub fn group(params: &[ParamInfo], search: &str) -> Groups {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut groups: Groups = Vec::new();
    for (i, p) in params.iter().enumerate() {
        if !shown(p) || !matches(p, &words) {
            continue;
        }
        match groups.iter_mut().find(|(g, _)| *g == p.group) {
            Some((_, list)) => list.push(i),
            None => groups.push((p.group.clone(), vec![i])),
        }
    }
    for (title, _) in groups.iter_mut() {
        *title = if title.is_empty() {
            t("host.editor.ungrouped")
        } else {
            title.replace('/', " \u{203a} ")
        };
    }
    groups
}

/// What the view keeps between frames: the groups as laid out for the search, each
/// parameter's words for the value it was asked about, the cards' places.
#[derive(Debug, Default)]
pub struct Params {
    /// (subject, parameters, search) the groups were made for, and the groups.
    /// (subject, the plug-in it held — `hosted_identity` —, the parameters, the search)
    /// the groups were made for, and the groups.
    groups: Option<(Subject, u64, usize, String, Groups)>,
    texts: crate::views::plugins::ValueWords,
    layout: Vec<egui::Rect>,
    /// Each group's cell width at 100 %, made with the groups.
    widths: Vec<f32>,
    /// The parameters shown (matching the search) and in all, for the count.
    pub(crate) counts: (usize, usize),
    /// The height the cards want at 100 %, from the last frame's layout.
    pub(crate) content_h: Option<f32>,
    /// The cells drawn last frame, and the meters among them (the frame-budget tests).
    pub drawn: usize,
    pub meters: usize,
}

/// A cell's width at 100 %: a house cell widened for a plug-in's own names, which run
/// longer than our units' ("Filter Cutoff", "Env 2 Attack").
pub const CELL_W: f32 = 84.0;
/// The widest a group's cells grow for its longest name (beyond it, the name is cut and
/// its tip says it whole).
pub const CELL_MAX_W: f32 = 132.0;

/// A cell of the view at 100 %: the house M cell, [`CELL_W`] wide.
fn cell_size() -> egui::Vec2 {
    egui::vec2(CELL_W, crate::kit::cell::size(Size::M).y)
}

/// A value the plug-in has no words for, in as many digits as it needs: `0.42`, `12.5`,
/// `2400` — never `2400.00`.
pub fn plain_number(v: f64, b: &mut String) {
    use std::fmt::Write as _;
    let a = v.abs();
    let _ = if a >= 1000.0 {
        write!(b, "{v:.0}")
    } else if a >= 100.0 {
        write!(b, "{v:.1}")
    } else {
        write!(b, "{v:.2}")
    };
}

/// What a cell asked for this frame, applied once the drawing is done.
enum Change {
    Set(u32, f64),
    End,
    Automate(u32),
    Mod(u32, crate::kit::automate::Choice, egui::Rect),
}

/// The plug-in's parameters in `area`: the cards of its groups, scrolled, the rows in view
/// drawn and nothing else — a 500-parameter plug-in costs what is on screen. `search` is
/// what the view's field holds.
pub fn area(
    app: &mut CantusApp,
    ps: &mut Params,
    ui: &mut egui::Ui,
    s: &Style,
    area: egui::Rect,
    subject: Subject,
    search: &str,
) {
    let k = s.k;
    let count = subject.params(app).map_or(0, <[ParamInfo]>::len);
    // Another plug-in in the slot or on the track (or another load of it) with as many
    // parameters is another list: the plug-in's identity is part of the key.
    let identity = app.hosted_identity(subject.target());
    let stale = ps.groups.as_ref().is_none_or(|(sj, id, n, f, _)| {
        *sj != subject || *id != identity || *n != count || f != search
    });
    if stale {
        let (groups, total) = subject.params(app).map_or((Vec::new(), 0), |p| {
            (group(p, search), p.iter().filter(|p| shown(p)).count())
        });
        let n = groups.iter().map(|g| g.1.len()).sum();
        ps.counts = (n, total);
        // Each group's cells as wide as its longest name needs, within bounds: a name cut
        // short is the last resort, never the first.
        let ctx = ui.ctx().clone();
        let params = subject.params(app).unwrap_or(&[]);
        ps.widths = groups
            .iter()
            .map(|(_, list)| {
                list.iter()
                    .filter_map(|&i| params.get(i))
                    .map(|p| {
                        let t = crate::kit::text::Text::new(
                            &p.name,
                            crate::kit::text::Role::Label,
                            s.t.label,
                        )
                        .caps(true);
                        crate::kit::text::measure(&ctx, &t, 1.0).x + 12.0
                    })
                    .fold(CELL_W, f32::max)
                    .min(CELL_MAX_W)
                    .ceil()
            })
            .collect();
        ps.groups = Some((subject, identity, count, search.to_string(), groups));
    }
    let Some((_, _, _, _, groups)) = ps.groups.as_ref() else {
        return;
    };
    ps.drawn = 0;
    ps.meters = 0;
    if groups.is_empty() {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area.shrink(8.0 * k)));
        let key = if search.trim().is_empty() {
            "hostedview.no_params"
        } else {
            "hostedview.no_match"
        };
        let words = tf(key, &[("text", &search.trim())]);
        controls::line(&mut child, s, &words, s.t.text_dim, area.width() / k - 16.0);
        ps.content_h = Some(60.0);
        return;
    }
    // Cards flow left to right and wrap; a group wider than the body wraps its cells.
    let cell_h = cell_size().y * k;
    let gap = s.d(4.0);
    let pad = 16.0 * k;
    let width = area.width() - s.d(10.0);
    let widths = &ps.widths;
    let mut layout = std::mem::take(&mut ps.layout);
    layout.clear();
    let (mut x, mut y, mut row_h) = (0.0f32, 0.0f32, 0.0f32);
    let mut row_start = 0;
    for (g, (_, list)) in groups.iter().enumerate() {
        let pitch = widths.get(g).copied().unwrap_or(CELL_W) * k + gap;
        let per_row_max = ((width - pad) / pitch).floor().max(1.0) as usize;
        let per_row = list.len().clamp(1, per_row_max);
        let rows = list.len().div_ceil(per_row).max(1);
        let w = (pad + per_row as f32 * pitch - gap)
            .max(150.0 * k)
            .min(width);
        let h = (band::TITLE + 8.0) * k + rows as f32 * (cell_h + gap) - gap + 8.0 * k;
        if x > 0.0 && x + w > width + 0.5 {
            justify(&mut layout[row_start..], width, gap, row_h);
            row_start = layout.len();
            x = 0.0;
            y += row_h + gap;
            row_h = 0.0;
        }
        layout.push(egui::Rect::from_min_size(
            egui::pos2(x, y),
            egui::vec2(w, h),
        ));
        x += w + gap;
        row_h = row_h.max(h);
    }
    justify(&mut layout[row_start..], width, gap, row_h);
    let total = y + row_h;
    ps.content_h = Some(total / k);
    let mut changes: Vec<Change> = Vec::new();
    let (mut drawn, mut meters) = (0usize, 0usize);
    let texts = &mut ps.texts;
    let later = subject.later(app);
    let wkey = subject.words_key(app);
    let view: &CantusApp = app;
    let params = subject.params(view).unwrap_or(&[]);
    let mut area_ui = ui.new_child(egui::UiBuilder::new().max_rect(area));
    egui::ScrollArea::vertical()
        .id_salt(("hosted-params", subject))
        .auto_shrink([false, false])
        .show(&mut area_ui, |ui| {
            let (origin, _) = ui.allocate_exact_size(egui::vec2(width, total), Sense::hover());
            let clip = ui.clip_rect();
            for (g, ((title, list), r)) in groups.iter().zip(&layout).enumerate() {
                let r = r.translate(origin.min.to_vec2());
                if !clip.intersects(r) {
                    continue;
                }
                let cw = widths.get(g).copied().unwrap_or(CELL_W);
                let pitch = cw * k + gap;
                controls::card(ui, s, r, Src::Str(title), None, |ui, cr| {
                    let per_row = ((cr.body.width() + gap) / pitch).floor().max(1.0) as usize;
                    let rows = list.len().div_ceil(per_row);
                    let step = cell_h + gap;
                    // The rows in view, and no others.
                    let first = ((clip.top() - cr.body.top()) / step).floor().max(0.0) as usize;
                    let last = (((clip.bottom() - cr.body.top()) / step).ceil().max(0.0) as usize)
                        .min(rows);
                    for row in first..last {
                        let from = row * per_row;
                        let chunk = &list[from..(from + per_row).min(list.len())];
                        // The cells centred in their card (a card may be wider than them:
                        // its row is justified).
                        let used = per_row.min(list.len()) as f32 * pitch - gap;
                        let inset = ((cr.body.width() - used) * 0.5).max(0.0).floor();
                        let rr = egui::Rect::from_min_size(
                            cr.body.min + egui::vec2(inset, row as f32 * step),
                            egui::vec2(cr.body.width() - inset, cell_h),
                        );
                        let mut row_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(rr)
                                .layout(egui::Layout::left_to_right(egui::Align::Min)),
                        );
                        let ui = &mut row_ui;
                        ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                        for &i in chunk {
                            let Some(p) = params.get(i) else {
                                continue;
                            };
                            drawn += 1;
                            let mut cx = Cx {
                                app: view,
                                subject,
                                texts: &mut *texts,
                                wkey,
                                later,
                                width: cw,
                            };
                            if p.read_only {
                                meters += 1;
                                meter(&mut cx, ui, s, p);
                            } else {
                                knob(&mut cx, ui, s, p, &mut changes);
                            }
                        }
                    }
                });
            }
        });
    ps.layout = layout;
    ps.drawn = drawn;
    ps.meters = meters;
    for c in changes {
        match c {
            Change::Set(id, v) => subject.set(app, id, v),
            Change::End => app.end_gesture(),
            Change::Automate(id) => app.automate(subject.auto(id)),
            Change::Mod(id, c, r) => app.modulation_choice(&subject.auto(id), c, r),
        }
    }
}

/// One flow row of cards made flush: the room left at its right shared among its cards,
/// and every card as tall as the row's tallest — a grid, not a ragged edge.
fn justify(row: &mut [egui::Rect], width: f32, gap: f32, height: f32) {
    let Some(last) = row.last() else {
        return;
    };
    let extra = (width - last.right()).max(0.0);
    let per = (extra / row.len() as f32).floor();
    for (j, r) in row.iter_mut().enumerate() {
        let left = r.left() + per * j as f32;
        let w = r.width() + per;
        *r = egui::Rect::from_min_size(egui::pos2(left, r.top()), egui::vec2(w, height));
    }
    let _ = gap;
}

/// What a cell reads from: the app, the plug-in, its words.
struct Cx<'a> {
    app: &'a CantusApp,
    subject: Subject,
    texts: &'a mut crate::views::plugins::ValueWords,
    wkey: (bool, u64),
    later: bool,
    /// The cell's width at 100 % (its group's).
    width: f32,
}

/// One parameter's cell: the house knob, the plug-in's words for its value in the pill,
/// its name below; the automation and modulation marks, and the parameter menu.
fn knob(cx: &mut Cx<'_>, ui: &mut egui::Ui, s: &Style, p: &ParamInfo, changes: &mut Vec<Change>) {
    let (app, subject) = (cx.app, cx.subject);
    let mut value = subject
        .stored(app, p.id)
        .or_else(|| subject.live(app, p.id))
        .unwrap_or(p.default);
    let (lo, hi) = (p.min.min(p.max), p.max.max(p.min));
    // Automation and modulation of a hosted parameter (ADR 0057): the curve is its
    // normalised value; the cell shows where the song has it, wears the marks, offers the
    // menu — *Modulate…*, MIDI learn, *Create automation clip*.
    let target = subject.auto(p.id);
    let automated = app.automated_value(&target);
    if let Some(pos) = automated.filter(|x| x.is_finite()) {
        value = lo + f64::from(pos) * (hi - lo);
    }
    let base = if hi > lo {
        ((value - lo) / (hi - lo)) as f32
    } else {
        0.0
    };
    let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let (offer, n_rings) = app.mod_offer(&target, base, s, &mut rings);
    if crate::kit::automate::probe::noting_learn() {
        crate::kit::automate::probe::note_learn(
            &format!("hosted:{target:?}"),
            offer.learn.is_some(),
        );
    }
    // The plug-in's words for the value, asked once per value — again while not answered
    // (an isolated plug-in answers later).
    let text = cx
        .texts
        .get_keyed(cx.wkey, p.id, value.to_bits(), cx.later, || {
            subject.text(app, p.id, value)
        });
    let fmt = |_: f32, b: &mut String| match text {
        Some(t) => b.push_str(t),
        None => plain_number(value, b),
    };
    let out = controls::Cell::new(value as f32, lo as f32..=hi as f32, p.default as f32)
        .stepped(p.stepped)
        .label(Src::Str(&p.name))
        .name(&p.name)
        .format(&fmt)
        .automation(automated.is_some())
        .modulation(offer)
        .rings(&rings[..n_rings])
        .width(cx.width)
        .show(ui, s);
    if let Some(v) = out.changed {
        changes.push(Change::Set(p.id, f64::from(v)));
    }
    if out.done {
        changes.push(Change::End);
    }
    if out.automate {
        changes.push(Change::Automate(p.id));
    }
    if let Some(c) = out.modulation {
        changes.push(Change::Mod(p.id, c, out.resp.rect));
    }
}

/// A parameter the plug-in reports and the host only reads (a gain reduction, a level):
/// a meter in the cell's place — the value in the pill, a bar where the knob would be, its
/// name below — read every frame. An in-process plug-in's own words for it; an isolated
/// one's number (asking its helper for words at every move would flood it).
fn meter(cx: &mut Cx<'_>, ui: &mut egui::Ui, s: &Style, p: &ParamInfo) {
    use crate::kit::text::{self as ktext, Policy, Role, Text};
    use crate::kit::{color, Fill};
    let (app, subject) = (cx.app, cx.subject);
    let value = subject
        .live(app, p.id)
        .or_else(|| subject.stored(app, p.id))
        .unwrap_or(p.default);
    let (lo, hi) = (p.min.min(p.max), p.max.max(p.min));
    let pos = if hi > lo {
        (((value - lo) / (hi - lo)) as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let words = if cx.later {
        None
    } else {
        cx.texts
            .get_keyed(cx.wkey, p.id, value.to_bits(), false, || {
                subject.text(app, p.id, value)
            })
            .map(str::to_string)
    };
    let text = words.unwrap_or_else(|| {
        let mut b = String::new();
        plain_number(value, &mut b);
        b
    });
    let size = egui::vec2(cx.width, cell_size().y) * s.k;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if resp.hovered() {
        let tip = tf("hostedview.meter_tip", &[("value", &text)]);
        crate::kit::hint::set_text(ui.ctx(), &p.name, &tip, crate::kit::hint::Caps::NONE);
    }
    if !ui.is_rect_visible(rect) {
        return;
    }
    crate::kit::with_brush(ui.painter(), |b| {
        b.register(rect, "meter_cell", crate::kit::cell::TOKENS, &[s.hue_tok]);
        let dark = s.t.dark;
        let rect = b.snap_rect(rect);
        let bg = color::shade(s.t.deck, if dark { -0.035 } else { -0.03 });
        b.rrect(rect, s.r(6.0), Fill::Solid(bg));
        let pad = s.s(6.0);
        let pill = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + pad + s.s(8.0)),
            egui::vec2((rect.width() - s.s(4.0)).min(s.s(72.0)), s.s(16.0)),
        );
        let pill = b.snap_rect(pill);
        // The readout is a well, not a pill: nothing here takes a drag.
        crate::kit::well::paint(b, pill, s, crate::kit::State::REST, None);
        let _ = ktext::paint(
            b,
            pill.shrink2(egui::vec2(s.s(2.0), 0.0)),
            &Text::new(&text, Role::Value, s.t.text).policy(Policy::Elide(ktext::Elide::End)),
            s.tk(),
        );
        // The bar: a sunken slot the height of a knob, lit from the bottom in the hue.
        let d = s.s(Size::M.diameter());
        let slot = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, pill.bottom() + s.s(4.0) + d * 0.5),
            egui::vec2(s.s(12.0), d),
        );
        let slot = b.snap_rect(slot);
        crate::kit::well::paint(b, slot, s, crate::kit::State::REST, None);
        if pos > 0.0 {
            let inner = slot.shrink(s.s(2.0));
            let lit = egui::Rect::from_min_max(
                egui::pos2(inner.left(), inner.bottom() - inner.height() * pos),
                inner.max,
            );
            b.rrect(b.snap_rect(lit), s.r(2.0), Fill::Solid(s.hue));
        }
        let label = egui::Rect::from_min_max(
            egui::pos2(rect.left() + s.s(3.0), slot.bottom() + s.s(4.0)),
            egui::pos2(rect.right() - s.s(3.0), slot.bottom() + s.s(18.0)),
        );
        let t = Text {
            src: Src::Str(&p.name),
            role: Role::Label,
            caps: true,
            align: egui::Align2::CENTER_CENTER,
            policy: Policy::Abbreviate,
            color: s.t.label,
        };
        let _ = ktext::paint(b, label, &t, s.tk());
    });
}

// ---- the switch, the default, the search ----------------------------------------------

/// The width of [`switch`] at 100 %: the two views and the default's lamp.
pub const SWITCH_W: f32 = 236.0;
pub const DEFAULT_W: f32 = 130.0;

/// The view switch: *Plug-in window* | *Cantus view*, and *Default* — lit while the view
/// showing is this plug-in's remembered default; a click remembers it, or forgets it
/// (back to the plug-in's own window). Without an editor of its own, a line saying Cantus's
/// view is its only one. Applied at once (a click closes this view for the other).
pub fn switch(app: &mut CantusApp, ui: &mut egui::Ui, s: &Style, target: Target) {
    use crate::views::fx::frame::tr;
    if !app.hosted_view_switchable(target) {
        let line = if app.hosted_working(target) {
            tr("hostedview.only_cantus")
        } else if app.hosted_loading(target)
            && app.hosted_view(target) == crate::hosted_view::HostedView::Native
        {
            // Its own window takes this frame's place once its helper has loaded it.
            tr("hostedview.opens_when_loaded")
        } else {
            ""
        };
        controls::line(ui, s, line, s.t.text_dim, SWITCH_W + 4.0 + DEFAULT_W);
        return;
    }
    let now = app.hosted_view(target);
    let labels = [
        Src::Key(crate::hosted_view::HostedView::Native.label_key()),
        Src::Key(crate::hosted_view::HostedView::Cantus.label_key()),
    ];
    let at = crate::hosted_view::HostedView::ALL
        .iter()
        .position(|v| *v == now)
        .unwrap_or(1);
    let picked = controls::segmented(ui, s, at, &labels, tr("hostedview.switch_tip"), SWITCH_W);
    let remembered = app.hosted_view_default(target);
    let on = remembered == Some(now);
    let name = if on {
        tr("hostedview.forget_default_tip")
    } else {
        tr("hostedview.use_default_tip")
    };
    let lamp = controls::led(ui, s, on, Src::Key("hostedview.default"), name, DEFAULT_W);
    if lamp.clicked() {
        app.set_hosted_view_default(target, (!on).then_some(now));
    }
    if let Some(i) = picked {
        app.set_hosted_view(target, crate::hosted_view::HostedView::ALL[i]);
    }
}

/// The search field: filters the parameters by name and by group as it is typed.
pub fn search_field(ui: &mut egui::Ui, s: &Style, id: egui::Id, text: &mut String, width: f32) {
    controls::text_field(
        ui,
        s,
        id,
        text,
        crate::views::fx::frame::tr("hostedview.search"),
        width,
    );
}

/// "12 parameters", or "3 of 12" while searching: as the cards were laid out for this
/// plug-in — or, when they were not (it stopped, it is loading, it is missing, or the cards
/// are another plug-in's), counted from the plug-in's own list.
pub fn count_words(app: &CantusApp, ps: &Params, subject: Subject, searching: bool) -> String {
    let identity = app.hosted_identity(subject.target());
    let laid_out = app.hosted_working(subject.target())
        && ps
            .groups
            .as_ref()
            .is_some_and(|g| g.0 == subject && g.1 == identity);
    let (n, total) = if laid_out {
        ps.counts
    } else {
        let total = subject
            .params(app)
            .map_or(0, |p| p.iter().filter(|p| shown(p)).count());
        (total, total)
    };
    if searching && laid_out {
        tf("hostedview.count_of", &[("n", &n), ("total", &total)])
    } else {
        tf("host.editor.count", &[("n", &total)])
    }
}

/// The cells drawn in the last frame of the open views, and the meters among them (the
/// frame-budget and meter tests).
pub fn drawn(app: &CantusApp) -> (usize, usize) {
    let e = &app.fx.hosted.editor.params;
    let i = &app.fx.hosted.inst.params;
    (e.drawn + i.drawn, e.meters + i.meters)
}
