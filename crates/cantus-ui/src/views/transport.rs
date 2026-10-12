//! The toolbar (ADR 0024 §4, §9, `fl-studio.md` §3.1): FL's top bar in the kit's look.
//!
//! From left to right: the logo; the menus over the **hint panel** (the project's name and
//! the hint for whatever is under the pointer, with its capability icons); the transport
//! as `ToolbarKey`s (to start, play, stop, record · loop, metronome, typing keyboard); the
//! tempo in a well (drag it, wheel it, double-click to reset); the **clock** in the
//! `display` role in an inset panel, with the time and the metre beside it; undo and redo;
//! the window keys (arrangement, rack, piano roll, mixer, inspector — lit when shown); the
//! eyedropper and the appearance page; the master meter.
//!
//! The bar lays itself out by priority: when the window is narrow (or the interface is
//! zoomed to 200 %) the least important groups leave first, so nothing ever overlaps.

use super::workspace::Pane;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::cap::{self, KeyFace};
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::meter::Levels;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{light, shell, with_brush, Fill, State, Style};
use crate::views;
use cantus_core::{Command, MergeKey, Ticks};
use egui::{Align2, Id, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;

/// The toolbar's height at 100 %.
pub const HEIGHT: f32 = 54.0;
/// A toolbar key's side at 100 %.
const KEY: f32 = 28.0;
/// The gap between groups.
const GAP: f32 = 10.0;

thread_local! {
    /// The project's name and the tempo's digits, rebuilt in place each frame.
    static NAME: RefCell<String> = RefCell::new(String::with_capacity(64));
    static TEMPO: RefCell<String> = RefCell::new(String::with_capacity(12));
    /// The clock's texts, rebuilt in place each frame (no allocation once grown).
    static CLOCK: RefCell<(String, String, String, String)> =
        RefCell::new((String::with_capacity(12), String::with_capacity(12), String::with_capacity(8), String::with_capacity(24)));
}

/// The groups of the bar, in the order they drop out when there is no room.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Group {
    Tools,
    Undo,
    ClockSide,
    Windows,
    Modes,
    Meter,
    Tempo,
    Transport,
}

/// The toolbar, filling `ui`'s rect.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let t = s.t;
    let k = s.k;
    // The bar takes its height even in a panel that did not size it.
    let max = ui.max_rect();
    let rect = egui::Rect::from_min_size(
        max.min,
        egui::vec2(
            max.width(),
            max.height().max((HEIGHT * s.t.density).round()),
        ),
    );
    ui.allocate_rect(rect, egui::Sense::hover());
    let cy = rect.center().y;
    let key = KEY * k;
    let gap = GAP * k;

    // The ground: the toolbar token lit from above, a hairline under it.
    let painter = ui.painter_at(rect);
    with_brush(&painter, |b| {
        let r = b.snap_rect(rect);
        b.rect(r, Fill::V(shade(t.toolbar, 0.03), shade(t.toolbar, -0.02)));
        b.hline(
            r.left(),
            r.right(),
            r.bottom() - b.px() * 0.5,
            b.px(),
            shade(t.toolbar, if t.dark { -0.22 } else { -0.16 }),
        );
    });

    // Widths of the groups, and which fit.
    let clock_w = clock_width(ui.ctx(), &s);
    let side_w = 80.0 * k;
    let widths = [
        (Group::Tools, 2.0 * key + 4.0 * k),
        (Group::Undo, 2.0 * key + 2.0 * k),
        (Group::ClockSide, side_w),
        (Group::Windows, 6.0 * key + 10.0 * k),
        (Group::Modes, 3.0 * key + 4.0 * k),
        (Group::Meter, 104.0 * k),
        (Group::Tempo, 104.0 * k),
        (
            Group::Transport,
            4.0 * key + 6.0 * k + (views::record_options::KEY_W + 2.0) * k,
        ),
    ];
    let logo_w = 34.0 * k;
    let menus_w = menus_width(ui.ctx(), &s);
    let (hint_min, hint_max) = (170.0 * k, 440.0 * k);
    let fixed = logo_w + clock_w + gap;
    let mut on = [true; 8];
    let used = |on: &[bool; 8]| {
        widths
            .iter()
            .zip(on)
            .filter(|(_, o)| **o)
            .map(|((_, w), _)| w + gap)
            .sum::<f32>()
    };
    let avail = rect.width() - 16.0 * k;
    for i in 0..on.len() {
        if fixed + hint_min.max(menus_w) + used(&on) <= avail {
            break;
        }
        on[i] = false;
    }
    let shown = |g: Group| widths.iter().zip(on).any(|((x, _), o)| *x == g && o);
    let hint_w = (avail - fixed - used(&on)).clamp(hint_min.max(menus_w), hint_max.max(menus_w));

    // The left block: logo, menus over the hint panel.
    let mut x = rect.left() + 8.0 * k;
    let logo_r = Rect::from_center_size(Pos2::new(x + 14.0 * k, cy), Vec2::splat(28.0 * k));
    x += logo_w;
    let left = Rect::from_min_size(
        Pos2::new(x, rect.top() + 3.0 * k),
        Vec2::new(hint_w, rect.height() - 6.0 * k),
    );
    let menu_r = Rect::from_min_size(left.min, Vec2::new(hint_w, 17.0 * k));
    let hint_r = Rect::from_min_max(Pos2::new(left.left(), menu_r.bottom() + 1.0 * k), left.max);
    // The window keys, tools and meter sit at the right end.
    let mut rx = rect.right() - 8.0 * k;
    let mut place_right = |w: f32| {
        let r = Rect::from_min_size(Pos2::new(rx - w, rect.top()), Vec2::new(w, rect.height()));
        rx -= w + gap;
        r
    };
    let meter_r = shown(Group::Meter).then(|| place_right(widths[5].1));
    let tools_r = shown(Group::Tools).then(|| place_right(widths[0].1));
    let windows_r = shown(Group::Windows).then(|| place_right(widths[3].1));

    // The transport, the tempo, the clock and undo: centred in the room between the hint
    // panel and the window keys (FL keeps its transport in the middle of the bar).
    let clock_full = clock_w + if shown(Group::ClockSide) { side_w } else { 0.0 };
    let middle: f32 = [
        Some(widths[7].1),
        shown(Group::Modes).then_some(widths[4].1),
        shown(Group::Tempo).then_some(widths[6].1),
        Some(clock_full),
        shown(Group::Undo).then_some(widths[1].1),
    ]
    .iter()
    .flatten()
    .map(|w| w + gap)
    .sum::<f32>()
        - gap;
    let (from, to) = (left.right() + gap, rx);
    x = (from + ((to - from) - middle).max(0.0) * 0.5).round();
    let mut place = |w: f32| {
        let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
        x += w + gap;
        r
    };
    let transport_r = place(widths[7].1);
    let modes_r = shown(Group::Modes).then(|| place(widths[4].1));
    let tempo_r = shown(Group::Tempo).then(|| place(widths[6].1));
    let clock_r = place(clock_full);
    let undo_r = shown(Group::Undo).then(|| place(widths[1].1));

    // --- paint and sense ---
    NAME.with(|n| {
        let mut name = n.borrow_mut();
        name.clear();
        name.push_str(&app.project().name);
        if app.bus.is_dirty() {
            name.push_str(" •");
        }
        views::workspace_widgets::paint_logo(&painter, logo_r, app.appearance.logo, &t);
        with_brush(&painter, |b| {
            let idle = views::workspace_status::tool_hint_key(app);
            views::hint::paint_panel(b, hint_r, &s, &name, idle);
        });
    });
    menus(app, ui, menu_r, &s);
    transport_keys(app, ui, transport_r, &s);
    if let Some(r) = modes_r {
        mode_keys(app, ui, r, &s);
    }
    if let Some(r) = tempo_r {
        tempo(app, ui, r, &s);
    }
    clock(app, ui, clock_r, clock_w, &s);
    if let Some(r) = undo_r {
        undo_keys(app, ui, r, &s);
    }
    if let Some(r) = windows_r {
        window_keys(app, ui, r, &s);
    }
    if let Some(r) = tools_r {
        tool_keys(app, ui, r, &s);
    }
    if let Some(r) = meter_r {
        master_meter(app, ui, r, &s);
    }
}

/// A toolbar key with its left edge at `x`, centred on `cy`, reporting a click. The tip
/// is only made when the key is hovered, so a bar at rest allocates nothing.
#[allow(clippy::too_many_arguments)]
fn key_at(
    ui: &mut Ui,
    x: f32,
    cy: f32,
    s: &Style,
    face: KeyFace<'_>,
    lit: bool,
    id: &str,
    tip: impl FnOnce() -> String,
) -> Response {
    let r = Rect::from_min_size(Pos2::new(x, cy - KEY * s.k * 0.5), Vec2::splat(KEY * s.k));
    let mut resp = ui.interact(r, Id::new(("toolbar-key", id)), Sense::click());
    let st = State::of(&resp, lit);
    with_brush(ui.painter(), |b| cap::toolbar_key(b, r, s, st, face));
    if st.hovered {
        let tip = tip();
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        resp = resp.on_hover_text(tip);
    }
    resp
}

/// A key's tooltip: its name, and its shortcut when it has one.
fn tip(app: &CantusApp, key: &str, action: Option<crate::keymap::Action>) -> String {
    let name = t(key);
    match action.and_then(|a| app.keymap.bindings_for(a).first().copied()) {
        Some(b) => format!("{name} ({})", b.display()),
        None => name,
    }
}

fn transport_keys(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    use crate::keymap::Action;
    let cy = r.center().y;
    let step = KEY * s.k + 2.0 * s.k;
    let mut x = r.left();
    let playing = app.engine.is_playing();
    let recording = app.is_recording();
    let keys: [(Glyph, bool, &str, &str, Option<Action>); 4] = [
        (
            Glyph::ToStart,
            false,
            "to-start",
            "transport.to_start",
            Some(Action::ToStart),
        ),
        (
            if playing { Glyph::Pause } else { Glyph::Play },
            playing,
            "play",
            "transport.play",
            Some(Action::PlayPause),
        ),
        (
            Glyph::Stop,
            false,
            "stop",
            "transport.stop",
            Some(Action::Stop),
        ),
        (
            Glyph::Record,
            recording,
            "record",
            "transport.record",
            Some(Action::Record),
        ),
    ];
    let mut record = None;
    for (g, lit, id, key, action) in keys {
        let resp = key_at(ui, x, cy, s, KeyFace::Glyph(g), lit, id, || {
            tip(app, key, action)
        });
        if resp.clicked() {
            match id {
                "to-start" => app.seek(Ticks::ZERO),
                "play" => app.toggle_play(),
                "stop" => app.stop(),
                _ => app.toggle_record(),
            }
        }
        if id == "record" {
            record = Some(resp);
        }
        x += step;
    }
    // The record options hang from a narrow ▾ beside the record key (ADR 0025).
    if let Some(record) = record {
        let w = views::record_options::KEY_W * s.k;
        let r = Rect::from_min_size(
            Pos2::new(x - 1.0 * s.k, cy - KEY * s.k * 0.5),
            Vec2::new(w, KEY * s.k),
        );
        views::record_options::key(app, ui, r, &record, s);
    }
}

fn mode_keys(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    use crate::keymap::Action;
    let cy = r.center().y;
    let step = KEY * s.k + 2.0 * s.k;
    let mut x = r.left();
    let looping = app.project().loop_enabled;
    let tip_loop = || tip(app, "transport.loop", Some(Action::ToggleLoop));
    if key_at(
        ui,
        x,
        cy,
        s,
        KeyFace::Glyph(Glyph::Loop),
        looping,
        "loop",
        tip_loop,
    )
    .clicked()
    {
        app.exec(Command::SetLoopEnabled { enabled: !looping });
    }
    x += step;
    let metro = app.project().metronome;
    let tip_m = || tip(app, "transport.metronome", Some(Action::ToggleMetronome));
    if key_at(
        ui,
        x,
        cy,
        s,
        KeyFace::Glyph(Glyph::Metronome),
        metro,
        "metronome",
        tip_m,
    )
    .clicked()
    {
        app.exec(Command::SetMetronome { enabled: !metro });
    }
    x += step;
    let typing = app.keyboard_piano;
    let tip_k = || format!("{} — {}", t("transport.typing"), t("transport.typing_tip"));
    if key_at(
        ui,
        x,
        cy,
        s,
        KeyFace::Glyph(Glyph::Keyboard),
        typing,
        "typing",
        tip_k,
    )
    .clicked()
    {
        app.run_action(crate::keymap::Action::ToggleTypingPiano);
    }
}

/// The tempo, in a well: drag up or down (Shift for fine), the wheel steps it by one,
/// double-click returns to 120.
fn tempo(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let well = Rect::from_center_size(r.center(), Vec2::new(r.width(), 30.0 * k));
    let resp = ui.interact(well, Id::new("toolbar-tempo"), Sense::click_and_drag());
    let mut bpm = app.project().tempo_bpm;
    let before = bpm;
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.02
        } else {
            0.25
        };
        bpm -= (resp.drag_delta().y * fine) as f64;
    }
    if resp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            bpm += dy.signum() as f64;
        }
    }
    if resp.double_clicked() {
        bpm = 120.0;
    }
    let bpm = (bpm * 10.0).round() / 10.0;
    let bpm = bpm.clamp(20.0, 999.0);
    if (bpm - before).abs() > 1e-4 {
        app.exec_merging(Command::SetTempo { bpm }, MergeKey::of(&"tempo"));
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    let st = State::of(&resp, false);
    if st.hovered {
        hint::set(
            ui.ctx(),
            &t("workspace.tempo"),
            Caps::PARAM,
            resp.dragged(),
            |b| write!(b, "{bpm:.1} {}", t("transport.bpm")),
        );
    }
    TEMPO.with(|v| {
        let mut value = v.borrow_mut();
        value.clear();
        let _ = write!(value, "{bpm:.1}");
        with_brush(ui.painter(), |b| {
            crate::kit::well::paint(b, well, s, st, None);
            let vr =
                Rect::from_min_max(well.min, Pos2::new(well.right() - 30.0 * k, well.bottom()));
            let _ = text::paint(
                b,
                vr.shrink2(Vec2::new(6.0 * k, 0.0)),
                &Text::new(
                    &value,
                    Role::Hero,
                    if resp.dragged() { s.t.focus } else { s.t.text },
                )
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let ur = Rect::from_min_max(Pos2::new(well.right() - 30.0 * k, well.top()), well.max);
            let _ = text::paint(
                b,
                ur.shrink2(Vec2::new(4.0 * k, 0.0)),
                &Text::key("transport.bpm", Role::Micro, s.t.text_dim).align(Align2::LEFT_CENTER),
                s.tk(),
            );
        });
    });
}

/// The widest the clock's digits get, in the `display` role.
fn clock_width(ctx: &egui::Context, s: &Style) -> f32 {
    text::measure(
        ctx,
        &Text::new("888.8.888", Role::Display, s.t.text),
        s.tk(),
    )
    .x + 24.0 * s.k
}

/// The clock: bars.beats.ticks in the `display` role on an inset panel; the time and the
/// metre beside it.
fn clock(app: &mut CantusApp, ui: &mut Ui, r: Rect, digits_w: f32, s: &Style) {
    let k = s.k;
    let t = &s.t;
    let lcd = Rect::from_center_size(r.center(), Vec2::new(r.width(), 40.0 * k));
    let pos = app.engine.position();
    let ts = app.project().time_signature;
    let (bar, beat, tick) = ts.bbt(pos);
    let secs = pos.to_seconds(app.project().tempo_bpm);
    let resp = ui.interact(lcd, Id::new("toolbar-clock"), Sense::hover());
    CLOCK.with(|c| {
        let mut c = c.borrow_mut();
        let (bbt, clock, metre, hint_name) = &mut *c;
        bbt.clear();
        let _ = write!(bbt, "{bar:>3}.{beat}.{tick:03}");
        clock.clear();
        let _ = write!(clock, "{:02}:{:05.2}", (secs / 60.0) as i64, secs % 60.0);
        metre.clear();
        let _ = write!(metre, "{ts}");
        if resp.hovered() {
            hint_name.clear();
            hint_name.push_str(&t_("transport.position"));
            hint::set_text(ui.ctx(), hint_name, clock, Caps::NONE);
        }
        with_brush(ui.painter(), |b| {
            b.register(lcd, "clock", views::hint::PANEL_TOKENS, &[]);
            light::sunken(b, lcd, s.r(4.0), t.ink, 1.0, t.depth);
            let dr = Rect::from_min_size(lcd.min, Vec2::new(digits_w, lcd.height()));
            let _ = text::paint(
                b,
                dr.shrink2(Vec2::new(12.0 * k, 0.0)),
                &Text::new(bbt, Role::Display, t.text)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            if lcd.width() > digits_w + 20.0 * k {
                let side = Rect::from_min_max(
                    Pos2::new(dr.right(), lcd.top() + 4.0 * k),
                    Pos2::new(lcd.right() - 8.0 * k, lcd.bottom() - 4.0 * k),
                );
                b.vline(
                    b.snap_mid(side.left() - 2.0 * k),
                    lcd.top() + 7.0 * k,
                    lcd.bottom() - 7.0 * k,
                    b.px(),
                    with_alpha(t.text_dim, 0.3),
                );
                let half = side.height() * 0.5;
                let top = Rect::from_min_size(side.min, Vec2::new(side.width(), half));
                let bottom =
                    Rect::from_min_max(Pos2::new(side.left(), side.top() + half), side.max);
                let _ = text::paint(
                    b,
                    top,
                    &Text::new(clock, Role::Value, t.text_dim)
                        .align(Align2::RIGHT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
                if app.rec.pass.is_some() || app.rec.dropped > 0 {
                    // Recording: the take's time and a pulsing lamp in the metre's place.
                    views::record_options::paint_elapsed(b, app, bottom, s);
                } else {
                    let _ = text::paint(
                        b,
                        bottom,
                        &Text::new(metre, Role::Value, t.text_dim)
                            .align(Align2::RIGHT_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        s.tk(),
                    );
                }
            }
        });
    });
}

fn t_(key: &str) -> String {
    t(key)
}

fn undo_keys(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    use crate::keymap::Action;
    let cy = r.center().y;
    let x = r.left();
    let undo_tip = || {
        app.bus
            .undo_label_key()
            .map(|k| tf("menu.undo_named", &[("what", &t(k))]))
            .unwrap_or_else(|| tip(app, "menu.undo", Some(Action::Undo)))
    };
    let u = key_at(
        ui,
        x,
        cy,
        s,
        KeyFace::Glyph(Glyph::Undo),
        false,
        "undo",
        undo_tip,
    );
    if u.clicked() && app.bus.can_undo() {
        app.undo();
    }
    let redo_tip = || {
        app.bus
            .redo_label_key()
            .map(|k| tf("menu.redo_named", &[("what", &t(k))]))
            .unwrap_or_else(|| tip(app, "menu.redo", Some(Action::Redo)))
    };
    let can_redo = app.bus.can_redo();
    let rr = key_at(
        ui,
        x + KEY * s.k + 2.0 * s.k,
        cy,
        s,
        KeyFace::Glyph(Glyph::Redo),
        false,
        "redo",
        redo_tip,
    );
    if rr.clicked() && can_redo {
        app.redo();
    }
}

fn window_keys(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    use crate::keymap::Action;
    let cy = r.center().y;
    let mut x = r.left();
    // The browser first, as in FL's toolbar.
    for pane in std::iter::once(Pane::Browser).chain(Pane::ZONES) {
        let action = match pane {
            Pane::Arrangement => Action::ViewArrangement,
            Pane::ChannelRack => Action::ViewChannelRack,
            Pane::PianoRoll => Action::ViewPianoRoll,
            Pane::Mixer => Action::ViewMixer,
            Pane::Browser => Action::ToggleBrowser,
            _ => Action::ToggleInspector,
        };
        let lit = app.workspace.is_visible(pane);
        let tip = || tip(app, pane.toggle_key(), Some(action));
        if key_at(
            ui,
            x,
            cy,
            s,
            KeyFace::Glyph(pane.glyph()),
            lit,
            pane.title_key(),
            tip,
        )
        .clicked()
        {
            app.toggle_pane(pane);
        }
        x += KEY * s.k + 2.0 * s.k;
    }
}

fn tool_keys(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    use crate::keymap::Action;
    let cy = r.center().y;
    let x = r.left();
    let pick = app.appearance_page.picking;
    let tip_p = || tip(app, "theme.inspect", Some(Action::PickElement));
    if key_at(
        ui,
        x,
        cy,
        s,
        KeyFace::Glyph(Glyph::Pick),
        pick,
        "pick",
        tip_p,
    )
    .clicked()
    {
        app.toggle_pick();
    }
    let open = app.show_theme_editor;
    let tip_a = || t("settings.appearance");
    if key_at(
        ui,
        x + KEY * s.k + 2.0 * s.k,
        cy,
        s,
        KeyFace::Glyph(Glyph::Gear),
        open,
        "appearance",
        tip_a,
    )
    .clicked()
    {
        app.show_theme_editor = !open;
    }
}

/// The master meter: two lanes in a well; the level under the pointer in the hint bar.
fn master_meter(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let m = app.engine.meters().master;
    let db = |v: f32| cantus_core::linear_to_db(v);
    let levels = Levels {
        level: [db(m[0]), db(m[1])],
        hold: [db(m[0]), db(m[1])],
    };
    let mr = Rect::from_center_size(r.center(), Vec2::new(r.width(), 14.0 * k));
    let resp = ui.interact(mr, Id::new("toolbar-master"), Sense::hover());
    if let Some(p) = resp.hover_pos() {
        let pos = (p.x - mr.left()) / mr.width().max(1.0);
        hint::set(ui.ctx(), &t("kit.mixer.master"), Caps::NONE, false, |b| {
            write!(b, "{:.1} dB", crate::kit::meter::db_at(pos))
        });
    }
    with_brush(ui.painter(), |b| {
        b.register(mr, "meter", crate::kit::meter::TOKENS, &[]);
        shell::horizontal_meter(b, mr, s, &levels);
        let lr = Rect::from_min_max(
            Pos2::new(mr.left(), mr.bottom() + 1.0 * k),
            Pos2::new(mr.right(), r.bottom() - 4.0 * k),
        );
        let _ = text::paint(
            b,
            lr,
            &Text::key("kit.mixer.master", Role::Micro, s.t.text_dim).align(Align2::LEFT_CENTER),
            s.tk(),
        );
        if m[0].max(m[1]) >= 1.0 {
            let _ = text::paint(
                b,
                lr,
                &Text::key("workspace.clip", Role::Micro, s.t.danger).align(Align2::RIGHT_CENTER),
                s.tk(),
            );
        }
    });
}

// ---- the menus --------------------------------------------------------------------------

const MENUS: [&str; 5] = [
    "menu.file",
    "menu.edit",
    "menu.clip",
    "menu.view",
    "menu.help",
];

fn menus_width(ctx: &egui::Context, s: &Style) -> f32 {
    MENUS
        .iter()
        .map(|k| {
            text::measure(
                ctx,
                &Text::key(k, Role::Control, s.t.text).caps(true),
                s.tk(),
            )
            .x + 14.0 * s.k
        })
        .sum()
}

/// The menu words: kit text in a box, each opening its menu.
fn menus(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let mut x = r.left();
    for (i, key) in MENUS.iter().enumerate() {
        let w = text::measure(
            ui.ctx(),
            &Text::key(key, Role::Control, s.t.text).caps(true),
            s.tk(),
        )
        .x + 14.0 * s.k;
        let rect = Rect::from_min_size(Pos2::new(x, r.top()), Vec2::new(w, r.height()));
        x += w;
        let resp = ui.interact(rect, Id::new(("toolbar-menu", i)), Sense::click());
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
        let build = open || resp.clicked();
        with_brush(ui.painter(), |b| {
            if resp.hovered() || open {
                b.rrect(
                    b.snap_rect(rect),
                    s.r(3.0),
                    Fill::Solid(with_alpha(s.t.text, if open { 0.14 } else { 0.08 })),
                );
            }
            let _ = text::paint(
                b,
                rect,
                &Text::key(
                    key,
                    Role::Control,
                    if resp.hovered() || open {
                        s.t.text
                    } else {
                        s.t.text_dim
                    },
                )
                .caps(true)
                .align(Align2::CENTER_CENTER),
                s.tk(),
            );
        });
        // A closed menu costs nothing: its popup is built only while it shows.
        if build {
            egui::Popup::menu(&resp)
                .style(menu_look)
                .show(|ui| match i {
                    0 => file_menu(app, ui),
                    1 => edit_menu(app, ui),
                    2 => clip_menu(app, ui),
                    3 => view_menu(app, ui),
                    _ => help_menu(app, ui),
                });
        }
    }
}

/// The menus' look (ADR 0024 §4, FL 2026): the kit's menu face, 22-pt rows, the hover
/// in the menu's own colour, text in the body role.
fn menu_look(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    let s = Style::current();
    let t = s.t;
    let v = &mut style.visuals;
    v.window_fill = t.menu;
    v.panel_fill = t.menu;
    v.window_stroke = egui::Stroke::new(1.0, shade(t.menu, -0.22));
    v.menu_corner_radius = egui::CornerRadius::same(s.r(4.0).round() as u8);
    v.override_text_color = Some(t.menu_text);
    v.weak_text_color = Some(with_alpha(t.menu_text, 0.72));
    for w in [&mut v.widgets.inactive, &mut v.widgets.noninteractive] {
        w.weak_bg_fill = egui::Color32::TRANSPARENT;
        w.bg_fill = shade(t.menu, -0.06);
        w.fg_stroke.color = t.menu_text;
        w.bg_stroke = egui::Stroke::new(1.0, shade(t.menu, -0.16));
    }
    for w in [
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.weak_bg_fill = t.menu_hover;
        w.bg_fill = t.menu_hover;
        w.fg_stroke.color = t.menu_text_hover;
        w.bg_stroke = egui::Stroke::NONE;
    }
    style.spacing.button_padding = Vec2::new(10.0, 3.0);
    style.spacing.interact_size.y = 22.0;
    style.spacing.menu_margin = egui::Margin::same(4);
    style.override_font_id = Some(Role::Body.font(s.tk()));
}

fn file_menu(app: &mut CantusApp, ui: &mut Ui) {
    if ui.button(t("menu.new")).clicked() {
        ui.close();
        app.request_new();
    }
    if ui.button(t("demos.menu")).clicked() {
        ui.close();
        views::demos::show(app);
    }
    if ui.button(t("menu.open")).clicked() {
        ui.close();
        app.request_open();
    }
    ui.separator();
    if ui.button(t("menu.save")).clicked() {
        ui.close();
        app.save();
    }
    if ui.button(t("menu.save_as")).clicked() {
        ui.close();
        app.save_as();
    }
    // Audio (ADR 0025): files in as clips, and every file of the project into its folder.
    if ui.button(t("audioclips.menu.collect")).clicked() {
        ui.close();
        app.save();
    }
    ui.separator();
    if ui.button(t("audioclips.menu.import")).clicked() {
        ui.close();
        app.import_audio_dialog();
    }
    if ui.button(t("menu.export_wav")).clicked() {
        ui.close();
        app.export_dialog();
    }
    views::interop::file_menu(app, ui);
    ui.separator();
    if ui.button(t("menu.quit")).clicked() {
        ui.close();
        app.request_quit = true;
    }
}

fn edit_menu(app: &mut CantusApp, ui: &mut Ui) {
    let undo = app
        .bus
        .undo_label_key()
        .map(|k| tf("menu.undo_named", &[("what", &t(k))]));
    let redo = app
        .bus
        .redo_label_key()
        .map(|k| tf("menu.redo_named", &[("what", &t(k))]));
    if ui
        .add_enabled(
            undo.is_some(),
            egui::Button::new(undo.unwrap_or_else(|| t("menu.undo"))),
        )
        .clicked()
    {
        ui.close();
        app.undo();
    }
    if ui
        .add_enabled(
            redo.is_some(),
            egui::Button::new(redo.unwrap_or_else(|| t("menu.redo"))),
        )
        .clicked()
    {
        ui.close();
        app.redo();
    }
    ui.separator();
    // Through `run_action`, not straight to the note methods: these mean one thing in the
    // piano roll and another in the arrangement (ADR 0010).
    use crate::keymap::Action;
    for (action, key) in [
        (Action::Cut, "menu.cut"),
        (Action::Copy, "menu.copy"),
        (Action::Paste, "menu.paste"),
        (Action::Duplicate, "menu.duplicate"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    ui.separator();
    for (action, key) in [
        (Action::SelectAll, "menu.select_all"),
        (Action::Delete, "menu.delete"),
        (Action::Quantize, "menu.quantize"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    // The selected strip's route menu (round 25): with its key, as every item here.
    ui.separator();
    if views::arrangement::action_item(app, ui, Action::RouteMenu, "routing.menu.route_menu") {
        ui.close();
    }
}

fn view_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    // Every area, with its key: the same toggles as the toolbar's window keys, and the
    // Routing tab after the mixer (round 25).
    let areas = std::iter::once(Pane::Browser)
        .chain(Pane::ZONES)
        .chain(std::iter::once(Pane::Routing));
    for pane in areas {
        let action = match pane {
            Pane::Arrangement => Action::ViewArrangement,
            Pane::ChannelRack => Action::ViewChannelRack,
            Pane::PianoRoll => Action::ViewPianoRoll,
            Pane::Mixer => Action::ViewMixer,
            Pane::Browser => Action::ToggleBrowser,
            Pane::Routing => Action::ViewRouting,
            _ => Action::ToggleInspector,
        };
        let mut shown = app.workspace.contains(pane);
        let shortcut = app
            .keymap
            .bindings_for(action)
            .first()
            .map(|b| b.display())
            .unwrap_or_default();
        if ui
            .checkbox(&mut shown, format!("{}    {shortcut}", t(pane.title_key())))
            .changed()
        {
            app.toggle_pane(pane);
        }
    }
    // The devices panel (round 20) and the full device picker.
    let mut shown = app.workspace.contains(Pane::Devices);
    if ui
        .checkbox(&mut shown, t("picker.menu.view_devices"))
        .changed()
    {
        app.toggle_pane(Pane::Devices);
    }
    if ui.button(t("picker.menu.open_picker")).clicked() {
        ui.close();
        app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None });
    }
    ui.separator();
    layout_menu(app, ui);
    ui.separator();
    for (action, key) in [
        (Action::ZoomIn, "workspace.zoom_in"),
        (Action::ZoomOut, "workspace.zoom_out"),
        (Action::ZoomReset, "workspace.zoom_reset"),
        (Action::PickElement, "theme.inspect"),
        (Action::CloseFloating, "workspace.close_floating"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    ui.separator();
    if ui.button(t("settings.appearance")).clicked() {
        ui.close();
        app.show_theme_editor = true;
    }
    if ui.button(t("menu.settings")).clicked() {
        ui.close();
        app.show_settings = true;
    }
    ui.checkbox(&mut app.keyboard_piano, t("menu.keyboard_piano"));
    ui.checkbox(&mut app.show_stats, t("menu.stats"));
}

fn help_menu(app: &mut CantusApp, ui: &mut Ui) {
    // The manual (the copy installed beside the program, in the interface's language),
    // the release notes and GitHub's forms for a bug or an idea (round 22).
    for (key, doc) in [
        ("about.menu.manual", crate::docs::Doc::Manual),
        ("about.menu.notes", crate::docs::Doc::ReleaseNotes),
    ] {
        if ui.button(t(key)).clicked() {
            ui.close();
            crate::docs::open(doc);
        }
    }
    if ui.button(t("menu.shortcuts")).clicked() {
        ui.close();
        app.show_shortcuts = true;
    }
    ui.separator();
    for (key, doc) in [
        ("about.menu.bug", crate::docs::Doc::ReportBug),
        ("about.menu.feature", crate::docs::Doc::RequestFeature),
    ] {
        if ui.button(t(key)).clicked() {
            ui.close();
            crate::docs::open(doc);
        }
    }
    ui.separator();
    if ui.button(t("update.menu.check")).clicked() {
        ui.close();
        views::update::check_now(app);
    }
    if ui.button(t("menu.about")).clicked() {
        ui.close();
        app.show_about = true;
    }
}

/// *View → Layout*: switch between saved window shapes, save the current one under a
/// name, or go back to the built-in one.
fn layout_menu(app: &mut CantusApp, ui: &mut Ui) {
    ui.menu_button(t("menu.layout"), |ui| {
        let current = app.layouts.current.name.clone();
        let saved: Vec<String> = app.layouts.saved.iter().map(|l| l.name.clone()).collect();
        for name in &saved {
            if ui.selectable_label(*name == current, name).clicked() {
                ui.close();
                app.apply_layout(name);
            }
        }
        if !saved.is_empty() {
            ui.separator();
        }
        if ui.button(t("layout.save_as")).clicked() {
            ui.close();
            // Offer the current name, so re-saving a layout is one keystroke.
            let seed = if current == crate::layout::DEFAULT_NAME {
                String::new()
            } else {
                current.clone()
            };
            app.naming_layout = Some(seed);
        }
        let is_saved = saved.contains(&current);
        if ui
            .add_enabled(is_saved, egui::Button::new(t("layout.delete")))
            .clicked()
        {
            ui.close();
            let ctx = ui.ctx().clone();
            app.delete_layout(&current, &ctx);
        }
        if ui.button(t("layout.reset")).clicked() {
            ui.close();
            app.reset_layout();
        }
    });
}

/// The **Clip** menu: everything goes through `run_action`, and every entry shows the key
/// bound to it, so the menu teaches the shortcut instead of replacing it.
fn clip_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    // FL's Tools ▸ Last tweaked ▸ Create automation clip, here where clips are made
    // (ADR 0027): enabled once a parameter has been moved, and saying which.
    let tweaked = app
        .automation
        .last_tweaked
        .clone()
        .map(|t| crate::automation::target_name(app.project(), &t));
    ui.add_enabled_ui(tweaked.is_some(), |ui| {
        if views::arrangement::action_item(
            app,
            ui,
            Action::AutomateLastTweaked,
            "auto.menu.last_tweaked",
        ) {
            ui.close();
        }
    });
    if let Some(name) = tweaked {
        let _ = crate::kit::widgets::menu_list(
            ui,
            &[crate::kit::menu::Item::Band(crate::kit::text::Src::Str(
                &name,
            ))],
            240.0,
        );
    }
    ui.separator();
    let selection = app.clip_selection();
    let (muted, looping) = selection
        .first()
        .and_then(|(t, c)| app.project().track(*t)?.clip(*c))
        .map(|c| (c.muted, c.content_length.is_some()))
        .unwrap_or((false, false));
    // The whole menu is about the clip selection; with none there is nothing to enable.
    ui.add_enabled_ui(!selection.is_empty(), |ui| {
        for (action, key) in [
            (Action::SplitClip, "clip.split"),
            (Action::GlueClips, "clip.glue"),
            (Action::Duplicate, "clip.duplicate"),
            // Linked clips (ADR 0037).
            (Action::MakeUnique, "clip.make_unique"),
            (Action::LinkClips, "action.link-clips"),
            (Action::SelectLinked, "clip.select_linked"),
        ] {
            if views::arrangement::action_item(app, ui, action, key) {
                ui.close();
            }
        }
        ui.separator();
        let mute_key = if muted { "clip.unmute" } else { "clip.mute" };
        if views::arrangement::action_item(app, ui, Action::ToggleClipMute, mute_key) {
            ui.close();
        }
        let loop_key = if looping {
            "clip.loop_off"
        } else {
            "clip.loop_on"
        };
        if views::arrangement::action_item(app, ui, Action::ToggleClipLoop, loop_key) {
            ui.close();
        }
        ui.separator();
        ui.horizontal(|ui| {
            let name = t("clip.color");
            for c in cantus_core::Color::PALETTE {
                let col = crate::theme::track_color(c);
                let r = views::workspace_widgets::swatch(ui, col, Vec2::splat(16.0), false, &name);
                if r.clicked() {
                    app.set_clip_color(Some(c));
                    ui.close();
                }
            }
        });
        if ui.button(t("clip.color_reset")).clicked() {
            app.set_clip_color(None);
            ui.close();
        }
        ui.separator();
        if ui.button(t("clip.delete")).clicked() {
            app.delete_clip_selection();
            ui.close();
        }
    });
}
