//! The top bar's groups of keys (ADR 0052; round 39, `dir-a`'s phase B): Pat / Song, the
//! transport and loop, undo, the windows, the inputs (MIDI, armed tracks) and the tools
//! (picker, zoom). Keys are joined in one recessed well, divided by its own dark edge; a
//! key that is on is tinted inside it (green for play, red for record, the accent else).

use super::paint::{self as p, ink1, ink2, ink3};
use super::{strip_tip, tip, Style};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::keymap::Action;
use crate::kit::color::{mix, opaque};
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, State};
use crate::views::workspace::Pane;
use egui::{Align2, Id, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;

thread_local! {
    /// A short number's words, rebuilt in place (no allocation once grown).
    static WORDS: RefCell<String> = RefCell::new(String::with_capacity(8));
}

/// Cells of the key row `row`, `widths` wide from its left (points at 100 %).
pub(super) fn cells<const N: usize>(row: Rect, widths: [f32; N], s: &Style) -> [Rect; N] {
    let mut x = 0.0;
    std::array::from_fn(|i| {
        let c = p::cell(row, x, widths[i] * s.k);
        x += widths[i] * s.k;
        c
    })
}

/// The responses of keys `ids` over `cells`.
fn sense<const N: usize>(ui: &Ui, cells: &[Rect; N], ids: [&str; N]) -> [Response; N] {
    std::array::from_fn(|i| ui.interact(cells[i], Id::new(("toolbar-key", ids[i])), Sense::click()))
}

// ---- Pat / Song ----------------------------------------------------------------------

const MODE_KEYS: [&str; 2] = ["topbar.mode.pattern", "topbar.mode.song"];

/// The two segments' widths: the sheet's 34 and 40, wider where a language needs it.
fn mode_widths(ctx: &egui::Context, s: &Style) -> [f32; 2] {
    let w = |i: usize, least: f32| {
        (p::measure_key(ctx, MODE_KEYS[i], Role::Control, 11.0, s) + 16.0 * s.k).max(least * s.k)
    };
    [w(0, 34.0), w(1, 40.0)]
}

pub fn mode_width(ctx: &egui::Context, s: &Style) -> f32 {
    mode_widths(ctx, s).iter().sum()
}

/// FL's switch as a segmented well: the chosen one a raised pill (inset 2, corner 3) in
/// 11 Medium ink 1, the other 11 Regular ink 3 — never coloured. A click (or L) swaps.
pub fn mode(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let [w0, w1] = mode_widths(ui.ctx(), s);
    let row = p::row(r, s);
    let well = Rect::from_min_size(row.min, Vec2::new(w0 + w1, row.height()));
    let resp = ui.interact(well, Id::new("toolbar-mode"), Sense::click());
    crate::controllers::learn_action_spot(app, ui, well, Action::TogglePatternMode);
    if resp.clicked() {
        app.run_action(Action::TogglePatternMode);
    }
    let pattern = app.project().pattern_mode;
    if resp.hovered() {
        let what = if pattern {
            "topbar.mode.pattern_tip"
        } else {
            "topbar.mode.song_tip"
        };
        let tip = tip(app, what, Some(Action::TogglePatternMode));
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = resp.clone().on_hover_text(tip);
    }
    with_brush(ui.painter(), |b| {
        b.register(well, "toolbar_key", crate::kit::cap::WELL_KEY_TOKENS, &[]);
        p::well(b, well, s, None);
        let mut x = well.left();
        for (i, key) in MODE_KEYS.iter().enumerate() {
            let w = if i == 0 { w0 } else { w1 };
            let seg = Rect::from_min_size(Pos2::new(x, well.top()), Vec2::new(w, well.height()));
            x += w;
            let chosen = (i == 0) == pattern;
            if chosen {
                let pill = b.snap_rect(seg.shrink(2.0 * k));
                b.rrect(pill, s.r(3.0), Fill::Solid(s.t.menu));
                b.hline(
                    pill.left() + s.r(3.0),
                    pill.right() - s.r(3.0),
                    pill.top() + b.px() * 0.5,
                    b.px(),
                    mix(s.t.menu, s.t.text, 0.10),
                );
            }
            let (role, ink) = if chosen {
                (Role::Control, ink1(s))
            } else if resp.hovered() {
                (Role::Label, ink2(s))
            } else {
                (Role::Label, ink3(s))
            };
            p::words_key(b, seg, key, role, 11.0, ink, Align2::CENTER_CENTER, s);
        }
    });
}

// ---- the transport -------------------------------------------------------------------

/// The well's keys (to start, play, stop, record) and the loop key after a gap.
const TRANSPORT: [f32; 4] = [30.0, 32.0, 30.0, 30.0];
const LOOP_X: f32 = 130.0;
const LOOP_W: f32 = 30.0;

pub fn transport_width(s: &Style) -> f32 {
    (LOOP_X + LOOP_W) * s.k
}

/// To start, play, stop and record in one well; loop beside it. Playing tints the play key
/// green, recording the record key red; a right-click on record opens its options.
pub fn transport(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let row = p::row(r, s);
    let c = cells(row, TRANSPORT, s);
    let [to_start, play, stop, record] = sense(ui, &c, ["to-start", "play", "stop", "record"]);
    let playing = app.engine.is_playing();
    let recording = app.is_recording();
    let lr = p::cell(row, LOOP_X * s.k, LOOP_W * s.k);
    let lresp = ui.interact(lr, Id::new(("toolbar-key", "loop")), Sense::click());
    let looping = app.project().loop_enabled;
    with_brush(ui.painter(), |b| {
        b.register(row, "toolbar_key", crate::kit::cap::WELL_KEY_TOKENS, &[]);
        p::joined(b, &c, s);
        let keys = [
            (&to_start, Glyph::ToStart, false, s.t.highlight, 0.2),
            (&play, Glyph::Play, playing, s.t.led, 0.18),
            (&stop, Glyph::Stop, false, s.t.highlight, 0.2),
            (&record, Glyph::Record, recording, s.t.danger, 0.22),
        ];
        for (i, (resp, g, on, hue, amount)) in keys.into_iter().enumerate() {
            let st = State::of(resp, on);
            p::tint(b, c[i], s, st, hue, amount);
            p::icon(b, c[i], s, g, st, hue, 24.0);
        }
        p::well(b, lr, s, None);
        let st = State::of(&lresp, looping);
        p::tint(b, lr, s, st, s.t.highlight, 0.2);
        p::icon(b, lr, s, Glyph::Loop, st, s.t.highlight, 24.0);
    });
    let keys = [
        (to_start, "transport.to_start", Action::ToStart),
        (play, "transport.play", Action::PlayPause),
        (stop, "transport.stop", Action::Stop),
    ];
    for (resp, key, action) in keys {
        // MIDI learn mode (round 29, ADR 0062): every key here is learnable.
        crate::controllers::learn_action_spot(app, ui, resp.rect, action);
        let resp = strip_tip(ui, resp, || tip(app, key, Some(action)));
        if resp.clicked() {
            app.run_action(action);
        }
    }
    crate::controllers::learn_action_spot(app, ui, record.rect, Action::Record);
    let record = strip_tip(ui, record, || {
        format!(
            "{} \u{2014} {}",
            tip(app, "transport.record", Some(Action::Record)),
            t("topbar.record_options_tip")
        )
    });
    if record.clicked() {
        app.run_action(Action::Record);
    }
    // The record options hang from the record key (ADR 0025): a right-click opens them.
    crate::views::record_options::popover(app, ui, &record, s);
    let lresp = strip_tip(ui, lresp, || {
        tip(app, "transport.loop", Some(Action::ToggleLoop))
    });
    if lresp.clicked() {
        app.run_action(Action::ToggleLoop);
    }
    crate::controllers::learn_action_spot(app, ui, lr, Action::ToggleLoop);
}

// ---- undo ----------------------------------------------------------------------------

pub fn undo_width(s: &Style) -> f32 {
    56.0 * s.k
}

/// Undo and redo in one well; what cannot be undone or redone is its glyph in ink 4.
pub fn undo(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let row = p::row(r, s);
    let c = cells(row, [28.0, 28.0], s);
    let [u, rr] = sense(ui, &c, ["undo", "redo"]);
    let (can_u, can_r) = (app.bus.can_undo(), app.bus.can_redo());
    with_brush(ui.painter(), |b| {
        b.register(row, "toolbar_key", crate::kit::cap::WELL_KEY_TOKENS, &[]);
        p::joined(b, &c, s);
        for (i, (resp, g, can)) in [(&u, Glyph::Undo, can_u), (&rr, Glyph::Redo, can_r)]
            .into_iter()
            .enumerate()
        {
            let mut st = State::of(resp, false);
            st.disabled = !can;
            if can {
                p::tint(b, c[i], s, st, s.t.highlight, 0.2);
            }
            p::icon(b, c[i], s, g, st, s.t.highlight, 24.0);
        }
    });
    let u = strip_tip(ui, u, || {
        app.bus
            .undo_label_key()
            .map(|k| tf("menu.undo_named", &[("what", &t(k))]))
            .unwrap_or_else(|| tip(app, "menu.undo", Some(Action::Undo)))
    });
    if u.clicked() && app.bus.can_undo() {
        app.undo();
    }
    let rr = strip_tip(ui, rr, || {
        app.bus
            .redo_label_key()
            .map(|k| tf("menu.redo_named", &[("what", &t(k))]))
            .unwrap_or_else(|| tip(app, "menu.redo", Some(Action::Redo)))
    });
    if rr.clicked() && app.bus.can_redo() {
        app.redo();
    }
}

// ---- the windows ---------------------------------------------------------------------

/// The seven windows' keys (`dir-a`): browser, arrangement, rack, piano roll, the selected
/// track's plug-in window, mixer, inspector — `None` is the plug-in window.
pub const WINDOWS: [Option<Pane>; 7] = [
    Some(Pane::Browser),
    Some(Pane::Arrangement),
    Some(Pane::ChannelRack),
    Some(Pane::PianoRoll),
    None,
    Some(Pane::Mixer),
    Some(Pane::Inspector),
];

pub fn windows_width(s: &Style) -> f32 {
    7.0 * 28.0 * s.k
}

/// A window key's glyph, whether it is open, its action and its tip's words.
pub fn window_key(app: &CantusApp, w: Option<Pane>) -> (Glyph, bool, Action, &'static str) {
    match w {
        None => (
            Glyph::Device,
            app.selected_track
                .is_some_and(|t| app.instrument_window_in_front(t)),
            Action::ToggleInstrumentWindow,
            "topbar.plugin_window",
        ),
        Some(pane) => {
            let action = match pane {
                Pane::Arrangement => Action::ViewArrangement,
                Pane::ChannelRack => Action::ViewChannelRack,
                Pane::PianoRoll => Action::ViewPianoRoll,
                Pane::Mixer => Action::ViewMixer,
                Pane::Browser => Action::ToggleBrowser,
                _ => Action::ToggleInspector,
            };
            (
                pane.glyph(),
                app.workspace.is_visible(pane),
                action,
                pane.toggle_key(),
            )
        }
    }
}

/// Run a window key's click.
pub fn window_click(app: &mut CantusApp, w: Option<Pane>) {
    match w {
        None => app.run_action(Action::ToggleInstrumentWindow),
        Some(pane) => app.toggle_pane(pane),
    }
}

/// The windows' keys in one well, the open ones tinted with the accent (16 %).
pub fn windows(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let row = p::row(r, s);
    let c = cells(row, [28.0; 7], s);
    windows_in(app, ui, &c, 20.0, s);
}

/// The windows' keys over `c` (the bar's, or the More panel's larger ones), glyphs drawn
/// in a box `box_h` high.
pub(super) fn windows_in(app: &mut CantusApp, ui: &mut Ui, c: &[Rect; 7], box_h: f32, s: &Style) {
    let ids = [
        "w-browser",
        "w-arrangement",
        "w-rack",
        "w-piano",
        "w-plugin",
        "w-mixer",
        "w-inspector",
    ];
    let resps = sense(ui, c, ids);
    let keys: [(Glyph, bool, Action, &str); 7] =
        std::array::from_fn(|i| window_key(app, WINDOWS[i]));
    with_brush(ui.painter(), |b| {
        let all = Rect::from_min_max(c[0].min, c[6].max);
        b.register(all, "toolbar_key", crate::kit::cap::WELL_KEY_TOKENS, &[]);
        p::joined(b, c, s);
        for i in 0..7 {
            let st = State::of(&resps[i], keys[i].1);
            p::tint(b, c[i], s, st, s.t.highlight, 0.16);
            p::icon(b, c[i], s, keys[i].0, st, s.t.highlight, box_h);
        }
    });
    for (i, resp) in resps.into_iter().enumerate() {
        let (_, _, action, key) = keys[i];
        let resp = strip_tip(ui, resp, || tip(app, key, Some(action)));
        if resp.clicked() {
            window_click(app, WINDOWS[i]);
        }
    }
}

// ---- the inputs ----------------------------------------------------------------------

const LAMP_WORDS: [&str; 2] = ["topbar.midi", "topbar.armed"];

/// Where the inputs' parts sit, from the well's left: the divider, the arm ring, the word,
/// the count's right edge; and the well's width.
fn inputs_parts(ctx: &egui::Context, s: &Style) -> (f32, f32, f32, f32, f32) {
    let k = s.k;
    let midi = p::measure_key(ctx, LAMP_WORDS[0], Role::Label, 11.0, s);
    let armed = p::measure_key(ctx, LAMP_WORDS[1], Role::Label, 11.0, s);
    let digits = p::measure(ctx, "88", Role::Control, 11.0, s);
    // The count right-aligned 104 in (the sheet), further only where the words need it.
    let div = (18.0 * k + midi + 9.0 * k).max(50.0 * k);
    let ring = div + 10.0 * k;
    let word = ring + 8.0 * k;
    let count = (word + armed + 4.0 * k + digits).max(104.0 * k);
    (div, ring, word, count, count + 8.0 * k)
}

pub fn inputs_width(ctx: &egui::Context, s: &Style) -> f32 {
    inputs_parts(ctx, s).4
}

/// The MIDI lamp and the armed tracks in one well: the lamp lights green as events come
/// in; the arm ring fills red, its count red, while tracks are armed.
pub fn inputs(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let row = p::row(r, s);
    let (div, ring, word, count, w) = inputs_parts(ui.ctx(), s);
    let well = Rect::from_min_size(row.min, Vec2::new(w, row.height()));
    let mr = Rect::from_min_max(well.min, Pos2::new(well.left() + div, well.bottom()));
    let ar = Rect::from_min_max(Pos2::new(well.left() + div, well.top()), well.max);
    // MIDI in: lit for a moment whenever an event comes in.
    let received = app.engine.midi_sender().received();
    let lit = app.topbar.midi_lamp(received);
    if lit {
        // Look again when the lamp is due to go out.
        crate::views::workspace_ticker::wake_after(ui.ctx(), std::time::Duration::from_millis(160));
    }
    let ports = app.midi_port_names.len();
    let armed = app.project().tracks.iter().filter(|t| t.armed).count();
    let m = ui.interact(mr, Id::new("toolbar-midi"), Sense::click());
    let a = ui.interact(ar, Id::new("toolbar-armed"), Sense::click());
    let learning = app.controllers.learn_mode;
    WORDS.with(|n| {
        let mut n = n.borrow_mut();
        n.clear();
        let _ = write!(n, "{armed}");
        with_brush(ui.painter(), |b| {
            b.register(well, "inputs", crate::kit::cap::WELL_KEY_TOKENS, &[]);
            p::well(b, well, s, None);
            for (resp, rr) in [(&m, mr), (&a, ar)] {
                p::tint(b, rr, s, State::of(resp, false), s.t.highlight, 0.2);
            }
            if learning {
                b.rrect(
                    b.snap_rect(mr.shrink(2.0 * k)),
                    s.r(3.0),
                    Fill::Solid(mix(s.t.well, s.t.highlight, 0.2)),
                );
            }
            let lc = Pos2::new(well.left() + 10.0 * k, well.center().y);
            if lit {
                b.circle(lc, 4.5 * k, Fill::Solid(s.t.led.gamma_multiply(0.2)));
            }
            let lamp = if lit {
                s.t.led
            } else {
                mix(s.t.well, ink3(s), 0.5)
            };
            b.circle(lc, 2.5 * k, Fill::Solid(lamp));
            let wr = Rect::from_min_max(
                Pos2::new(well.left() + 18.0 * k, well.top()),
                Pos2::new(well.left() + div, well.bottom()),
            );
            p::words_key(
                b,
                wr,
                LAMP_WORDS[0],
                Role::Label,
                11.0,
                ink2(s),
                Align2::LEFT_CENTER,
                s,
            );
            b.vline(
                b.snap_mid(well.left() + div),
                well.top() + 6.0 * k,
                well.bottom() - 6.0 * k,
                b.px(),
                opaque(s.t.inner_shadow),
            );
            let rc = Pos2::new(well.left() + ring, well.center().y);
            let ac = if armed > 0 {
                s.t.danger
            } else {
                mix(s.t.well, ink3(s), 0.5)
            };
            b.circle_stroke(rc, 3.5 * k, 1.2 * k, ac);
            if armed > 0 {
                b.circle(rc, 1.6 * k, Fill::Solid(ac));
            }
            let wr = Rect::from_min_max(
                Pos2::new(well.left() + word, well.top()),
                Pos2::new(well.left() + count, well.bottom()),
            );
            p::words_key(
                b,
                wr,
                LAMP_WORDS[1],
                Role::Label,
                11.0,
                ink2(s),
                Align2::LEFT_CENTER,
                s,
            );
            let (role, ink) = if armed > 0 {
                (Role::Control, s.t.danger)
            } else {
                (Role::Label, ink3(s))
            };
            p::words(b, wr, &n, role, 11.0, ink, Align2::RIGHT_CENTER, s);
        });
    });
    let m_settings = m.secondary_clicked();
    let (m_clicked, a_clicked) = (m.clicked(), a.clicked());
    if m.hovered() {
        let tip = if ports == 0 {
            t("topbar.midi_none")
        } else {
            let ports_tip = crate::i18n::tf_count(
                "topbar.midi_tip",
                ports,
                &[("ports", &app.midi_port_names.join(", "))],
            );
            // With a port open the cell is Live's MIDI key: a click learns, a right-click
            // opens the settings (review 1, #29, #47).
            let learn = match app.keymap.bindings_for(Action::MidiLearnMode).first() {
                Some(b) => tf("ctl.topbar.lamp_tip_key", &[("key", &b.display())]),
                None => t("ctl.topbar.lamp_tip"),
            };
            format!("{ports_tip}. {learn}")
        };
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = m.on_hover_text(tip);
    }
    if m_clicked && ports > 0 {
        app.run_action(Action::MidiLearnMode);
    }
    if m_settings || (m_clicked && ports == 0) {
        app.show_settings = true;
    }
    if a.hovered() {
        let tip = crate::i18n::tf_count("topbar.armed_tip", armed, &[]);
        let tip = match app.keymap.bindings_for(Action::ArmTrack).first() {
            Some(b) => format!("{tip} ({})", b.display()),
            None => tip,
        };
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = a.on_hover_text(tip);
    }
    if a_clicked {
        app.run_action(Action::ArmTrack);
    }
}

// ---- the tools -----------------------------------------------------------------------

/// The zoom well's width: the sheet's 52, wider where "200 %" needs it.
fn zoom_w(ctx: &egui::Context, s: &Style) -> f32 {
    (p::measure(ctx, "200 %", Role::Label, 11.0, s) + 16.0 * s.k).max(52.0 * s.k)
}

pub fn tools_width(ctx: &egui::Context, s: &Style) -> f32 {
    32.0 * s.k + zoom_w(ctx, s)
}

/// The element picker (lit while picking) and the interface's zoom (a click for the
/// steps; Ctrl + / Ctrl −).
pub fn tools(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let row = p::row(r, s);
    let pr = p::cell(row, 0.0, 28.0 * s.k);
    let zr = p::cell(row, 32.0 * s.k, zoom_w(ui.ctx(), s));
    pick_key(app, ui, pr, s);
    zoom_well(app, ui, zr, s);
}

/// The picker key in `pr`.
pub(super) fn pick_key(app: &mut CantusApp, ui: &mut Ui, pr: Rect, s: &Style) {
    let resp = ui.interact(pr, Id::new(("toolbar-key", "pick")), Sense::click());
    let st = State::of(&resp, app.appearance_page.picking);
    with_brush(ui.painter(), |b| {
        p::well(b, pr, s, None);
        p::tint(b, pr, s, st, s.t.highlight, 0.2);
        p::icon(b, pr, s, Glyph::Pick, st, s.t.highlight, 20.0);
    });
    let resp = strip_tip(ui, resp, || {
        tip(app, "theme.inspect", Some(Action::PickElement))
    });
    if resp.clicked() {
        app.toggle_pick();
    }
}

/// The zoom well in `zr`: its value, a click for the steps.
pub(super) fn zoom_well(app: &mut CantusApp, ui: &mut Ui, zr: Rect, s: &Style) {
    let resp = ui.interact(zr, Id::new("toolbar-zoom"), Sense::click());
    let popup = egui::Popup::default_response_id(&resp);
    let is_open = egui::Popup::is_id_open(ui.ctx(), popup);
    let now = app.settings.ui_scale();
    WORDS.with(|v| {
        let mut v = v.borrow_mut();
        v.clear();
        let _ = write!(v, "{:.0} %", now * 100.0);
        let st = State::of(&resp, is_open);
        with_brush(ui.painter(), |b| {
            p::well(b, zr, s, None);
            p::tint(b, zr, s, st, s.t.highlight, 0.2);
            let ink = if st.hovered || is_open {
                ink1(s)
            } else {
                ink2(s)
            };
            p::words(b, zr, &v, Role::Label, 11.0, ink, Align2::CENTER_CENTER, s);
        });
        if st.hovered {
            let name = tip(app, "topbar.zoom", Some(Action::ZoomIn));
            let caps = Caps {
                menu: true,
                ..Caps::NONE
            };
            hint::set_text(ui.ctx(), &name, &v, caps);
        }
    });
    if is_open || resp.clicked() {
        egui::Popup::menu(&resp)
            .frame(egui::Frame::NONE)
            .show(|ui| {
                let labels: Vec<String> = crate::settings::UI_SCALES
                    .iter()
                    .map(|z| format!("{:.0} %", z * 100.0))
                    .collect();
                let items: Vec<crate::kit::menu::Item<'_>> = crate::settings::UI_SCALES
                    .iter()
                    .zip(&labels)
                    .map(|(z, l)| crate::kit::menu::Item::Action {
                        label: Src::Str(l),
                        check: crate::kit::menu::Check::Radio((z - now).abs() < 1e-3),
                        shortcut: None,
                        submenu: false,
                        enabled: true,
                    })
                    .collect();
                let (_, c) = crate::kit::widgets::menu_list(ui, &items, 96.0);
                if let Some(i) = c.clicked {
                    let ctx = ui.ctx().clone();
                    app.apply_ui_scale(&ctx, crate::settings::UI_SCALES[i]);
                    ui.close();
                }
            });
    }
}
