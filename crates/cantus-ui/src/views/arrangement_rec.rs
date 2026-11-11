//! Recording in the arrangement (ADR 0025): the record controls on a track header, the
//! punch brace in the ruler, and the takes drawn in red while they are recorded.
//!
//! **The header** (FL's coloured block, a recording DAW's buttons): the **arm** lamp left
//! of mute — round, red, lit while armed and pulsing while it records; Shift+click arms
//! that track alone. On a tall row of a track with an input, a second line holds the
//! **input** chip ("IN 1", "IN 1-2"; a click lists the device's inputs), the **monitor**
//! key (a speaker: struck out Off, one wave Auto, two waves and lit On; a click cycles)
//! and, while armed, the **input meter** in place of the song map. A track with take lanes
//! has a **disclosure** at the foot of its colour cell.
//!
//! **The punch lane** appears under the loop lane while punch is on: the range as a red
//! brace, dragged like the loop's — by an end to move it, anywhere else to draw a new one.

use super::chrome;
use super::{beats_to_ticks, ticks_to_beats, TimelineGeometry};
use crate::app::CantusApp;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{self, Brush, Fill, Style, Tok};
use crate::record_pass::max_lane;
use crate::record_session::{input_choices, input_label, monitor_key};
use cantus_core::audio::{MonitorMode, TrackInput};
use cantus_core::{LoopRegion, Ticks, Track, TrackId};
use cantus_engine::PEAK_FRAMES;
use egui::{pos2, vec2, Color32, Id, Pos2, Rect, Sense};
use std::cell::RefCell;

thread_local! {
    /// The input chip's words, rebuilt in place (no allocation once grown).
    static LABEL: RefCell<String> = RefCell::new(String::with_capacity(16));
    /// A live take's peaks, folded to the columns on screen.
    static COLUMNS: RefCell<Vec<(f32, f32)>> = RefCell::new(Vec::with_capacity(2048));
}

/// Where a header's record controls are (r39:el-arrange: the arm is the header's ● key;
/// the input is line 2 while armed; the monitor and the takes' disclosure sit at line 2's
/// right end, only on a track with an input or with takes).
#[derive(Copy, Clone, Debug)]
pub(super) struct RecRects {
    pub arm: Pos2,
    pub input: Option<Rect>,
    pub monitor: Option<Rect>,
    pub disclosure: Option<Rect>,
}

/// Lay out the record controls of `track`'s header.
pub(super) fn layout(r: &super::head::Rects, track: &Track, s: &Style) -> RecRects {
    let k = s.k;
    let arm = if r.n > 2 {
        r.keys[2].center()
    } else {
        pos2(f32::NAN, f32::NAN)
    };
    let mut rr = RecRects {
        arm,
        input: None,
        monitor: None,
        disclosure: None,
    };
    let Some(l2) = r.line2 else {
        return rr;
    };
    let side = 14.0 * k;
    let mut right = l2.right();
    if track.input.is_some() {
        let m = Rect::from_min_size(
            pos2(right - side, l2.center().y - side * 0.5),
            vec2(side, side),
        );
        rr.monitor = Some(m);
        right = m.left() - 2.0 * k;
        if track.armed {
            rr.input = Some(Rect::from_min_max(
                l2.min,
                pos2(right - 2.0 * k, l2.bottom()),
            ));
        }
    }
    if max_lane(track) > 0 || track.takes_open {
        rr.disclosure = Some(Rect::from_min_size(
            pos2(right - side, l2.center().y - side * 0.5),
            vec2(side, side),
        ));
    }
    rr
}

/// A speaker, in a square: struck out (Off), with one wave (Auto) or two (On).
fn speaker(b: &mut Brush, r: Rect, mode: MonitorMode, color: Color32) {
    let c = r.center();
    let u = r.width().min(r.height()) / 12.0;
    let p = |x: f32, y: f32| c + vec2(x * u, y * u);
    let w = (1.1 * u).max(b.px());
    b.rect(
        Rect::from_min_max(p(-5.0, -1.8), p(-2.8, 1.8)),
        Fill::Solid(color),
    );
    b.polygon(
        &[p(-2.8, -1.8), p(0.2, -4.6), p(0.2, 4.6), p(-2.8, 1.8)],
        color,
    );
    let quarter = std::f32::consts::FRAC_PI_4;
    match mode {
        MonitorMode::Off => {
            b.segment(p(2.2, -2.6), p(6.2, 2.6), w, color, true);
            b.segment(p(2.2, 2.6), p(6.2, -2.6), w, color, true);
        }
        MonitorMode::Auto => b.arc(p(0.6, 0.0), 2.8 * u, -quarter, quarter, w, color, true),
        MonitorMode::On => {
            b.arc(p(0.6, 0.0), 2.8 * u, -quarter, quarter, w, color, true);
            b.arc(p(0.6, 0.0), 5.4 * u, -quarter, quarter, w, color, true);
        }
    }
}

/// The monitor key and the takes' disclosure, painted over the header.
pub(super) fn paint_header(
    b: &mut Brush,
    track: &Track,
    rr: &RecRects,
    pointer: Option<Pos2>,
    s: &Style,
) {
    let ink = super::slate::Ink::of(s);
    let hovered = |r: Rect| pointer.is_some_and(|p| r.contains(p));
    if let (Some(r), Some(input)) = (rr.monitor, &track.input) {
        let c = match input.monitor {
            MonitorMode::On => ink.accent,
            _ if hovered(r) => ink.ink1,
            MonitorMode::Auto => ink.ink2,
            MonitorMode::Off => ink.ink3,
        };
        if hovered(r) {
            b.rrect(r, s.r(3.0), Fill::Solid(ink.well_hi));
        }
        speaker(b, r.shrink(2.0 * s.k), input.monitor, c);
    }
    if let Some(r) = rr.disclosure {
        let c = if hovered(r) { ink.ink1 } else { ink.ink3 };
        let g = if track.takes_open {
            Glyph::Down
        } else {
            Glyph::Menu
        };
        glyph::paint(b, r.shrink(2.0 * s.k), g, c);
    }
}

/// The record controls' clicks: monitor (cycles), the input (its list, while armed), the
/// takes' disclosure. The arm is the header's own key.
pub(super) fn header_input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    rr: &RecRects,
    s: &Style,
) {
    let k = s.k;
    if let Some(r) = rr.monitor {
        let resp = ui.interact(r, ui.id().with(("arr-monitor", id.0)), Sense::click());
        if resp.hovered() {
            let mode = app
                .project()
                .track(id)
                .and_then(|t| t.input.as_ref())
                .map(|i| i.monitor)
                .unwrap_or_default();
            hint::set_text(
                ui.ctx(),
                chrome::word("rec.monitor_tip"),
                chrome::word(monitor_key(mode)),
                Caps::NONE,
            );
        }
        if resp.clicked() {
            app.cycle_monitor(id);
        }
    }
    if let Some(r) = rr.input {
        let resp = ui.interact(r, ui.id().with(("arr-input", id.0)), Sense::click());
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                chrome::word("rec.input_tip"),
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        egui::Popup::menu(&resp)
            .frame(chrome::menu_frame())
            .show(|ui| {
                if input_menu(app, ui, id) {
                    ui.close();
                }
            });
    }
    if let Some(r) = rr.disclosure {
        let resp = ui.interact(
            r.expand(2.0 * k),
            ui.id().with(("arr-takes", id.0)),
            Sense::click(),
        );
        if resp.hovered() {
            hint::set_text(ui.ctx(), chrome::word("rec.takes_tip"), "", Caps::NONE);
        }
        if resp.clicked() {
            app.toggle_takes(id);
        }
    }
}

/// What a row of the input list does.
#[derive(Clone, Debug, PartialEq)]
enum InputChoice {
    Set(Option<TrackInput>),
    Monitor(MonitorMode),
    Settings,
}

/// The input list (the header's chip, the track's menu): *No input*, each mono input and
/// each stereo pair of the open device, the monitor mode. With no device it says so and
/// offers the audio settings. Returns whether something was chosen.
pub(crate) fn input_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    let Some(current) = app.project().track(id).map(|t| t.input.clone()) else {
        return false;
    };
    let channels = app.input_channels();
    let choices = input_choices(channels);
    let mut words: Vec<String> = choices
        .iter()
        .map(|c| {
            let mut s = String::new();
            input_label(c, &mut s);
            s
        })
        .collect();
    if let (Some(c), true) = (&current, channels == 0) {
        let mut s = String::new();
        input_label(c, &mut s);
        words.push(s);
    }
    let same = |a: &TrackInput, b: &TrackInput| a.first == b.first && a.stereo == b.stereo;
    let mut items: Vec<Item<'_>> = vec![Item::Band(Src::Key("rec.input"))];
    let mut acts: Vec<Option<InputChoice>> = vec![None];
    items.push(Item::Action {
        label: Src::Key("rec.no_input_choice"),
        check: Check::Radio(current.is_none()),
        shortcut: None,
        submenu: false,
        enabled: true,
    });
    acts.push(Some(InputChoice::Set(None)));
    if channels == 0 {
        if let Some(c) = &current {
            items.push(Item::Action {
                label: Src::Str(&words[words.len() - 1]),
                check: Check::Radio(true),
                shortcut: None,
                submenu: false,
                enabled: false,
            });
            acts.push(Some(InputChoice::Set(Some(c.clone()))));
        }
        items.push(Item::Action {
            label: Src::Key("rec.no_device"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: false,
        });
        acts.push(None);
        items.push(Item::action(Src::Key("rec.audio_settings")));
        acts.push(Some(InputChoice::Settings));
    } else {
        for (c, w) in choices.iter().zip(&words) {
            items.push(Item::Action {
                label: Src::Str(w),
                check: Check::Radio(current.as_ref().is_some_and(|cur| same(cur, c))),
                shortcut: None,
                submenu: false,
                enabled: true,
            });
            acts.push(Some(InputChoice::Set(Some(c.clone()))));
        }
    }
    if let Some(cur) = &current {
        items.push(Item::Band(Src::Key("rec.monitor")));
        acts.push(None);
        for m in [MonitorMode::Off, MonitorMode::Auto, MonitorMode::On] {
            items.push(Item::Action {
                label: Src::Key(monitor_key(m)),
                check: Check::Radio(cur.monitor == m),
                shortcut: None,
                submenu: false,
                enabled: true,
            });
            acts.push(Some(InputChoice::Monitor(m)));
        }
    }
    let clicked = kit::widgets::menu_list(ui, &items, 200.0).1.clicked;
    let Some(act) = clicked.and_then(|i| acts.get(i).cloned().flatten()) else {
        return false;
    };
    match act {
        InputChoice::Set(input) => app.set_track_input(id, input),
        InputChoice::Monitor(m) => app.set_monitor(id, m),
        InputChoice::Settings => app.show_settings = true,
    }
    true
}

/// The recording part of a track's menu: add an audio track, arm, the input, the takes.
/// Returns whether something was chosen (the menu closes).
pub(super) fn track_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    let Some((armed, open, lanes)) = app
        .project()
        .track(id)
        .map(|t| (t.armed, t.takes_open, max_lane(t)))
    else {
        return false;
    };
    let check = |key: &'static str, on: bool, enabled: bool| Item::Action {
        label: Src::Key(key),
        check: if on { Check::On } else { Check::Off },
        shortcut: None,
        submenu: false,
        enabled,
    };
    let items = [
        Item::Band(Src::Key("rec.recording")),
        Item::action(Src::Key("rec.add_audio_track")),
        check("rec.arm", armed, true),
        check("rec.show_takes", open, lanes > 0),
        Item::Action {
            label: Src::Key("rec.delete_unused"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: lanes > 0,
        },
        Item::Action {
            label: Src::Key("rec.explode"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: lanes > 0,
        },
    ];
    let chosen = match kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
        Some(1) => {
            app.add_audio_track();
            true
        }
        Some(2) => {
            app.toggle_arm(id, false);
            true
        }
        Some(3) => {
            app.toggle_takes(id);
            true
        }
        Some(4) => {
            app.delete_unused_takes(id);
            true
        }
        Some(5) => {
            app.explode_takes(id);
            true
        }
        _ => false,
    };
    chosen || input_menu(app, ui, id)
}

// ---- the punch lane ------------------------------------------------------------------------

/// The punch lane's height: there while punch is on.
pub(super) fn punch_height(app: &CantusApp, _s: &Style) -> f32 {
    if app.project().recording.punch_enabled {
        super::LOOP_LANE_H
    } else {
        0.0
    }
}

/// The punch lane and its brace, in the record colour.
pub(super) fn paint_punch(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    ruler: Rect,
    s: &Style,
) {
    let Some((top, bottom)) = geo.punch else {
        return;
    };
    let t = &s.t;
    let lane = Rect::from_min_max(pos2(ruler.left(), top), pos2(ruler.right(), bottom));
    b.register(lane, "punch_lane", &[Tok::Ink, Tok::Danger], &[]);
    b.rect(b.snap_rect(lane), Fill::Solid(mix(t.ink, t.danger, 0.08)));
    if let Some(r) = &app.project().recording.punch {
        let x0 = geo.x(ticks_to_beats(r.start));
        let x1 = geo.x(ticks_to_beats(r.end));
        let bar = Rect::from_min_max(pos2(x0, lane.top() + 2.0), pos2(x1, lane.bottom() - 2.0));
        b.rect(b.snap_rect(bar), Fill::Solid(with_alpha(t.danger, 0.55)));
        for x in [x0, x1] {
            b.rect(
                b.snap_rect(Rect::from_min_max(
                    pos2(x - 1.5, lane.top() + 1.0),
                    pos2(x + 1.5, lane.bottom()),
                )),
                Fill::Solid(t.danger),
            );
        }
    }
}

/// What a drag in the punch lane is doing.
#[derive(Copy, Clone, Debug, PartialEq)]
enum PunchDrag {
    Start,
    End,
    New(Ticks),
}

/// The punch lane's gestures: drag an end of the brace to move it, drag elsewhere for a
/// new range; right-click for *set punch from selection* and *punch off*.
pub(super) fn punch_input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    geo: &TimelineGeometry,
    ruler: Rect,
    s: &Style,
) {
    let Some((top, bottom)) = geo.punch else {
        return;
    };
    let lane = Rect::from_min_max(pos2(ruler.left(), top), pos2(ruler.right(), bottom));
    let id = Id::new("arrangement-punch-lane");
    let resp = ui.interact(lane, id, Sense::click_and_drag());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        hint::set_text(
            ui.ctx(),
            chrome::word("rec.punch_tip"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let free = ui.input(|i| i.modifiers.command);
    let tick = |app: &CantusApp, x: f32| {
        let t = beats_to_ticks(geo.beats(x).max(0.0));
        if free || !app.arrangement.snap {
            t
        } else {
            crate::views::bars::snap(app.project(), t, app.arrangement.grid)
        }
    };
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
            let grab = 6.0 * s.k;
            let mode = match &app.project().recording.punch {
                Some(r) if (p.x - geo.x(ticks_to_beats(r.start))).abs() <= grab => PunchDrag::Start,
                Some(r) if (p.x - geo.x(ticks_to_beats(r.end))).abs() <= grab => PunchDrag::End,
                _ => PunchDrag::New(tick(app, p.x)),
            };
            ui.ctx().data_mut(|d| d.insert_temp(id, mode));
        }
    }
    if resp.dragged_by(egui::PointerButton::Primary) {
        let mode: Option<PunchDrag> = ui.ctx().data(|d| d.get_temp(id));
        if let (Some(mode), Some(p)) = (mode, resp.interact_pointer_pos()) {
            let at = tick(app, p.x);
            let mut settings = app.project().recording.clone();
            let old = settings.punch.clone();
            let (a, z) = match (mode, old) {
                (PunchDrag::Start, Some(r)) => (at, r.end),
                (PunchDrag::End, Some(r)) => (r.start, at),
                (PunchDrag::New(from), _) => (from, at),
                (_, None) => (at, at),
            };
            let (a, z) = (a.min(z), a.max(z));
            if z > a {
                settings.punch = Some(LoopRegion { start: a, end: z });
                app.set_record_settings(settings);
            }
        }
    }
    if resp.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<PunchDrag>(id));
        app.end_gesture();
    }
    egui::Popup::context_menu(&resp)
        .frame(chrome::menu_frame())
        .show(|ui| {
            let items = [
                Item::Action {
                    label: Src::Key("rec.punch_from_selection"),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: app.time_selection.is_some(),
                },
                Item::action(Src::Key("rec.punch_off")),
            ];
            match kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
                Some(0) => {
                    app.punch_from_selection();
                    ui.close();
                }
                Some(1) => {
                    app.toggle_punch();
                    ui.close();
                }
                _ => {}
            }
        });
}

// ---- takes as they are recorded --------------------------------------------------------------

/// Where a track's take is being drawn: from where the pass's file (or the loop pass)
/// began to where the recording has got to.
fn live_span(app: &CantusApp, track: TrackId) -> Option<(Ticks, Ticks, bool)> {
    let live = app.rec.pass.as_ref()?;
    if !live.records(track) {
        return None;
    }
    let pos = app.engine.position();
    match live.takes.iter().find(|t| t.track == track) {
        Some(take) => {
            let start = if take.pass_from == 0 {
                live.transport_start
            } else {
                live.pass_start
            };
            let frames = take.peaks.len().saturating_sub(take.pass_from) * PEAK_FRAMES;
            let secs = frames as f64 / app.engine.sample_rate.max(1.0);
            // Natural speed through the tempo map (ADR 0060).
            let end = Ticks(
                crate::views::bars::map(app.project())
                    .tick_after_seconds(start.0 as f64, secs)
                    .round() as i64,
            );
            let end = if pos > start { end.max(pos) } else { end };
            (end > start).then_some((start, end, true))
        }
        None => {
            let start = live.pass_start;
            (pos > start).then_some((start, pos, false))
        }
    }
}

/// The body of a take being recorded, in the lane it will land on: the record colour, the
/// part before the record start (the count-in, kept as a handle) dimmer; the notes as they
/// are played; and the punch range as two red lines down the lanes.
pub(super) fn paint_live(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let t = &s.t;
    let k = s.k;
    let rs = &app.project().recording;
    if let (true, Some(r)) = (rs.punch_enabled, &rs.punch) {
        for at in [r.start, r.end] {
            let x = geo.x(ticks_to_beats(at));
            if x >= rect.left() && x <= rect.right() {
                b.vline(
                    x,
                    rect.top(),
                    rect.bottom(),
                    b.px(),
                    with_alpha(t.danger, 0.7),
                );
            }
        }
    }
    let Some(live) = app.rec.pass.as_ref() else {
        return;
    };
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(row) = geo.rows.get(i) else { continue };
        let Some((a, z, audio)) = live_span(app, track.id) else {
            continue;
        };
        if row.top > rect.bottom() || row.top + row.height < rect.top() {
            continue;
        }
        let x0 = geo.x(ticks_to_beats(a)).max(rect.left() - 4.0);
        let x1 = geo.x(ticks_to_beats(z)).min(rect.right() + 4.0);
        if x1 <= x0 {
            continue;
        }
        let inset = (row.height * 0.04).clamp(1.0, 2.0);
        let body = Rect::from_min_max(
            pos2(x0, row.top + inset),
            pos2(x1, row.top + row.height - inset),
        );
        b.register(body, "live_take", &[Tok::Danger], &[]);
        b.rrect(
            b.snap_rect(body),
            s.r(2.0),
            Fill::Solid(with_alpha(t.danger, 0.30)),
        );
        let rs_x = geo.x(ticks_to_beats(live.plan.record_start));
        if rs_x > x0 {
            // The count-in: recorded, kept, not yet the take.
            let handle = Rect::from_min_max(body.min, pos2(rs_x.min(x1), body.bottom()));
            b.rect(handle, Fill::Solid(with_alpha(t.ink, 0.35)));
        }
        b.rrect_stroke(b.snap_rect(body), s.r(2.0), 1.5 * k, t.danger);
        b.rect(
            Rect::from_min_max(body.min, pos2(body.right(), body.top() + 3.0 * k)),
            Fill::Solid(t.danger),
        );
        if !audio {
            live_notes(b, app, geo, body, s);
        }
    }
}

/// The notes of the pass so far on a track, as small bars across its body.
fn live_notes(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, body: Rect, s: &Style) {
    let Some(live) = app.rec.pass.as_ref() else {
        return;
    };
    let pos = app.engine.position();
    let ink = shade(s.t.danger, 0.25);
    let inner = body.shrink2(vec2(0.0, 5.0 * s.k));
    let y = |pitch: u8| inner.bottom() - (pitch as f32 / 127.0) * inner.height();
    let mut bar = |start: Ticks, end: Ticks, pitch: u8| {
        let x0 = geo.x(ticks_to_beats(start)).max(body.left());
        let x1 = geo.x(ticks_to_beats(end)).min(body.right());
        if x1 > x0 {
            let cy = y(pitch);
            b.rect(
                Rect::from_min_max(pos2(x0, cy - 1.0), pos2(x1.max(x0 + 1.0), cy + 1.0)),
                Fill::Solid(ink),
            );
        }
    };
    let pass = app.recorder.pass();
    for n in live.notes.iter().filter(|n| n.pass == pass) {
        bar(n.start, n.start + n.length, n.pitch);
    }
    for p in app.recorder.pending() {
        bar(p.start, pos, p.pitch);
    }
}

/// The live waveforms, through the one painter every view draws peaks with
/// (`waveform::paint_peaks`), folded first to the body's columns.
pub(super) fn paint_live_waves(
    painter: &egui::Painter,
    app: &CantusApp,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let Some(live) = app.rec.pass.as_ref() else {
        return;
    };
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(row) = geo.rows.get(i) else { continue };
        let Some((a, z, true)) = live_span(app, track.id) else {
            continue;
        };
        let Some(peaks) = live.current_peaks(track.id) else {
            continue;
        };
        if peaks.is_empty() || row.top > rect.bottom() || row.top + row.height < rect.top() {
            continue;
        }
        let x0 = geo.x(ticks_to_beats(a));
        // The peaks cover the recorded frames, which may lag the playhead a little.
        let take = live.takes.iter().find(|t| t.track == track.id);
        let frames = take.map_or(0, |t| {
            t.peaks.len().saturating_sub(t.pass_from) * PEAK_FRAMES
        });
        let secs = frames as f64 / app.engine.sample_rate.max(1.0);
        let x1 = geo.x(ticks_to_beats(Ticks(
            crate::views::bars::map(app.project())
                .tick_after_seconds(a.0 as f64, secs)
                .round() as i64,
        )));
        let _ = z;
        let body = Rect::from_min_max(
            pos2(x0, row.top + 6.0 * s.k),
            pos2(x1.max(x0 + 1.0), row.top + row.height - 3.0 * s.k),
        );
        let shown = body.intersect(rect);
        if shown.width() < 1.0 {
            continue;
        }
        // Only the columns on screen, each the min and max of the peaks under it.
        let per_px = peaks.len() as f32 / body.width().max(1.0);
        let first = ((shown.left() - body.left()) * per_px).floor().max(0.0) as usize;
        let cols = shown.width().ceil().max(1.0) as usize;
        COLUMNS.with(|c| {
            let mut c = c.borrow_mut();
            c.clear();
            for col in 0..cols {
                let lo = first + (col as f32 * per_px) as usize;
                let hi = (first + ((col + 1) as f32 * per_px).ceil() as usize).min(peaks.len());
                if lo >= hi {
                    if let Some(p) = peaks.get(lo.min(peaks.len() - 1)) {
                        c.push(*p);
                    }
                    continue;
                }
                let (mut mn, mut mx) = (1.0f32, -1.0f32);
                for &(a, b) in &peaks[lo..hi] {
                    mn = mn.min(a);
                    mx = mx.max(b);
                }
                c.push((mn, mx));
            }
            crate::waveform::paint_peaks(
                &painter.with_clip_rect(rect),
                shown,
                &c,
                1.0,
                shade(s.t.danger, if s.t.dark { 0.30 } else { -0.22 }),
            );
        });
    }
}

/// Where a track header's record controls are, from the last frame's geometry: what a
/// test (or a tool) aims at.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RecordControls {
    pub arm: Pos2,
    pub input: Option<Rect>,
    pub monitor: Option<Rect>,
    pub disclosure: Option<Rect>,
}

pub fn record_controls(app: &CantusApp, track: TrackId) -> Option<RecordControls> {
    let geo = app.arrangement.geometry()?;
    let i = app.project().tracks.iter().position(|t| t.id == track)?;
    let row = geo.rows.get(i)?;
    let s = Style::current();
    let w = app.arrangement.header_shown();
    let hx = geo.left - 1.0;
    let rect = Rect::from_min_size(pos2(hx - w, row.top), vec2(w, row.height));
    let t = &app.project().tracks[i];
    let n = super::head::keys_of(super::head::Width::of(w, s.k));
    let r = super::head::rects(
        rect,
        usize::from(t.folder.is_some()),
        n,
        t.glyph.is_some(),
        &s,
    );
    let rr = layout(&r, t, &s);
    Some(RecordControls {
        arm: rr.arm,
        input: rr.input,
        monitor: rr.monitor,
        disclosure: rr.disclosure,
    })
}
