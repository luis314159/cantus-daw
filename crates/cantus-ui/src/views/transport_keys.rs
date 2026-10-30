//! The top bar's groups of keys (ADR 0052): PAT / SONG, the transport, undo, the windows,
//! the inputs (typing keyboard, MIDI, armed tracks) and the tools (picker, appearance,
//! zoom).

use super::{key_in, key_rect, key_side, key_strip, strip_tip, tip, Style, ROW_KEY, STEP};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::keymap::Action;
use crate::kit::cap::KeyFace;

use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::tokens::Tok;
use crate::kit::{led, light, with_brush, Fill, State};
use crate::views;
use crate::views::workspace::Pane;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::fmt::Write as _;

// ---- PAT / SONG ----------------------------------------------------------------------

const MODE_KEYS: [&str; 2] = ["topbar.mode.pattern", "topbar.mode.song"];

fn word_w(ctx: &egui::Context, s: &Style, key: &str, role: Role) -> f32 {
    text::measure(ctx, &Text::key(key, role, s.t.text).caps(true), s.tk()).x
}

pub fn mode_width(ctx: &egui::Context, s: &Style) -> f32 {
    let w = MODE_KEYS
        .iter()
        .map(|k| word_w(ctx, s, k, Role::Label))
        .fold(0.0, f32::max);
    w + 30.0 * s.k
}

/// FL's switch: two lamps, PAT over SONG, in a well; a click (or L) swaps them.
pub fn mode(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let well = Rect::from_center_size(r.center(), Vec2::new(r.width(), 36.0 * k));
    let resp = ui.interact(well, Id::new("toolbar-mode"), Sense::click());
    let pattern = app.project().pattern_mode;
    crate::controllers::learn_action_spot(app, ui, well, Action::TogglePatternMode);
    if resp.clicked() {
        app.run_action(Action::TogglePatternMode);
    }
    let st = State::of(&resp, false);
    if st.hovered {
        let what = if pattern {
            "topbar.mode.pattern_tip"
        } else {
            "topbar.mode.song_tip"
        };
        let tip = tip(app, what, Some(Action::TogglePatternMode));
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = resp.on_hover_text(tip);
    }
    let pattern = app.project().pattern_mode;
    with_brush(ui.painter(), |b| {
        crate::kit::well::paint(b, well, s, st, None);
        for (i, key) in MODE_KEYS.iter().enumerate() {
            let lit = (i == 0) == pattern;
            let row = Rect::from_min_size(
                Pos2::new(well.left(), well.top() + 2.0 * k + i as f32 * 16.0 * k),
                Vec2::new(well.width(), 16.0 * k),
            );
            let c = Pos2::new(row.left() + 10.0 * k, row.center().y);
            let color = if i == 0 { Tok::Highlight } else { Tok::Led };
            led::paint(b, c, 8.0 * k, lit, s, color, Tok::Well, 1.0);
            let tr = Rect::from_min_max(Pos2::new(c.x + 8.0 * k, row.top()), row.max);
            let ink = if lit { s.t.text } else { s.t.text_dim };
            let _ = text::paint(
                b,
                tr,
                &Text::key(key, Role::Label, ink)
                    .caps(true)
                    .align(Align2::LEFT_CENTER),
                s.tk(),
            );
        }
    });
}

// ---- the transport -------------------------------------------------------------------

pub fn transport_width(s: &Style) -> f32 {
    (4.0 * ROW_KEY + STEP + views::record_options::KEY_W + STEP + ROW_KEY
        // The automation arm and its options (round 31, ADR 0068).
        + STEP
        + ROW_KEY
        + views::auto_modes::MORE_W)
        * s.k
}

/// To start, play, stop and record on one strip, the record options' ▾ beside it, loop.
pub fn transport(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let playing = app.engine.is_playing();
    let recording = app.is_recording();
    let play = if playing { Glyph::Pause } else { Glyph::Play };
    let [to_start, play_r, stop, record] = key_strip(
        ui,
        r.left(),
        r,
        s,
        [
            (KeyFace::Glyph(Glyph::ToStart), false, "to-start"),
            (KeyFace::Glyph(play), playing, "play"),
            (KeyFace::Glyph(Glyph::Stop), false, "stop"),
            (KeyFace::Glyph(Glyph::Record), recording, "record"),
        ],
    );
    let keys = [
        (to_start, "transport.to_start", Action::ToStart),
        (play_r, "transport.play", Action::PlayPause),
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
        tip(app, "transport.record", Some(Action::Record))
    });
    if record.clicked() {
        app.run_action(Action::Record);
    }
    let side = key_side(r, s);
    // The record options hang from a narrow ▾ beside the strip (ADR 0025); a right-click
    // on the record key opens them too.
    let x = r.left() + 4.0 * side + STEP * k;
    let w = views::record_options::KEY_W * k;
    let kr = key_rect(r, x, s);
    let kr = Rect::from_min_size(kr.min, Vec2::new(w, kr.height()));
    views::record_options::key(app, ui, kr, &record, s);
    let looping = app.project().loop_enabled;
    let lr = key_rect(r, kr.right() + STEP * k, s);
    if key_in(
        ui,
        lr,
        s,
        KeyFace::Glyph(Glyph::Loop),
        looping,
        "loop",
        || tip(app, "transport.loop", Some(Action::ToggleLoop)),
    )
    .clicked()
    {
        app.run_action(Action::ToggleLoop);
    }
    crate::controllers::learn_action_spot(app, ui, lr, Action::ToggleLoop);
    // The automation arm and its ▾ (round 31, ADR 0068): Ableton's place for it, after loop.
    let ar = key_rect(r, lr.right() + STEP * k, s);
    let mr = Rect::from_min_size(
        egui::pos2(ar.right(), ar.top()),
        Vec2::new(views::auto_modes::MORE_W * k, ar.height()),
    );
    views::auto_modes::arm_keys(app, ui, ar, mr, s);
}

// ---- undo ----------------------------------------------------------------------------

pub fn undo_width(s: &Style) -> f32 {
    2.0 * ROW_KEY * s.k
}

pub fn undo(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let [u, rr] = key_strip(
        ui,
        r.left(),
        r,
        s,
        [
            (KeyFace::Glyph(Glyph::Undo), false, "undo"),
            (KeyFace::Glyph(Glyph::Redo), false, "redo"),
        ],
    );
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

pub fn windows_width(s: &Style) -> f32 {
    7.0 * ROW_KEY * s.k // r33:launcher
}

/// The areas' keys on one strip, the browser first as in FL's toolbar.
pub fn windows(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    const PANES: [Pane; 7] = [
        // r33:launcher
        Pane::Browser,
        Pane::Arrangement,
        Pane::Session, // r33:launcher
        Pane::ChannelRack,
        Pane::PianoRoll,
        Pane::Mixer,
        Pane::Inspector,
    ];
    let keys: [(KeyFace<'_>, bool, &str); 7] = std::array::from_fn(|i| {
        // r33:launcher
        let pane = PANES[i];
        (
            KeyFace::Glyph(pane.glyph()),
            app.workspace.is_visible(pane),
            pane.title_key(),
        )
    });
    let resps = key_strip(ui, r.left(), r, s, keys);
    for (pane, resp) in PANES.into_iter().zip(resps) {
        let action = match pane {
            Pane::Arrangement => Action::ViewArrangement,
            Pane::ChannelRack => Action::ViewChannelRack,
            Pane::PianoRoll => Action::ViewPianoRoll,
            Pane::Mixer => Action::ViewMixer,
            Pane::Browser => Action::ToggleBrowser,
            Pane::Session => Action::ToggleSessionView, // r33:launcher
            _ => Action::ToggleInspector,
        };
        let resp = strip_tip(ui, resp, || tip(app, pane.toggle_key(), Some(action)));
        if resp.clicked() && pane == Pane::Session {
            // r33:launcher
            crate::views::session::keys::toggle_view(app); // r33:launcher
        } else if resp.clicked() {
            // r33:launcher
            app.toggle_pane(pane);
        }
    }
}

// ---- the inputs ----------------------------------------------------------------------

const LAMP_WORDS: [&str; 2] = ["topbar.midi", "topbar.armed"];

/// The words' and the number's widths in a lamp cell.
/// A lamp cell's word's width, and the number's.
fn lamp_parts(ctx: &egui::Context, s: &Style, i: usize) -> (f32, f32) {
    let word = word_w(ctx, s, LAMP_WORDS[i], Role::Micro);
    let digits = text::measure(ctx, &Text::new("88", Role::Value, s.t.text), s.tk()).x;
    (word, digits)
}

/// A lamp cell's width: the lamp, its word, its number.
fn lamp_w(ctx: &egui::Context, s: &Style, i: usize) -> f32 {
    let (word, digits) = lamp_parts(ctx, s, i);
    word + digits + 26.0 * s.k
}

pub fn inputs_width(ctx: &egui::Context, s: &Style) -> f32 {
    ROW_KEY * s.k + 2.0 * STEP * s.k + lamp_w(ctx, s, 0) + lamp_w(ctx, s, 1)
}

/// A lamp, a word and a number in a well, one row: the MIDI input, the armed tracks.
#[allow(clippy::too_many_arguments)]
fn lamp_cell(
    ui: &Ui,
    r: Rect,
    s: &Style,
    lit: bool,
    color: Tok,
    word: &str,
    value: &str,
    words_w: f32,
) {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        light::sunken(b, r, s.r(3.0), s.t.well, 1.0, s.t.depth);
        let c = Pos2::new(r.left() + 9.0 * k, r.center().y);
        led::paint(b, c, 7.0 * k, lit, s, color, Tok::Well, 1.0);
        let wr = Rect::from_min_size(
            Pos2::new(c.x + 7.0 * k, r.top()),
            Vec2::new(words_w + 2.0 * k, r.height()),
        );
        let _ = text::paint(
            b,
            wr,
            &Text::key(word, Role::Micro, s.t.text_dim)
                .caps(true)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        // The number's room: what is past the word, 2 pt in from it and 4 from the well's
        // edge — the width of "88" and 2 pt to spare (review 2: 2 pt short, "10" read "1…").
        let vr = Rect::from_min_max(
            Pos2::new(wr.right() + 2.0 * k, r.top()),
            Pos2::new(r.right() - 4.0 * k, r.bottom()),
        );
        let ink = if lit { s.t.text } else { s.t.text_dim };
        let _ = text::paint(
            b,
            vr,
            &Text::new(value, Role::Value, ink)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

thread_local! {
    static WORDS: std::cell::RefCell<(String, String)> =
        std::cell::RefCell::new((String::with_capacity(8), String::with_capacity(8)));
}

pub fn inputs(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    // The typing keyboard (Ctrl+T).
    let typing = app.keyboard_piano;
    let kr = key_rect(r, r.left(), s);
    if key_in(
        ui,
        kr,
        s,
        KeyFace::Glyph(Glyph::Keyboard),
        typing,
        "typing",
        || {
            format!(
                "{} \u{2014} {}",
                t("transport.typing"),
                t("transport.typing_tip")
            )
        },
    )
    .clicked()
    {
        app.run_action(Action::ToggleTypingPiano);
    }
    let (w0, w1) = (lamp_w(ui.ctx(), s, 0), lamp_w(ui.ctx(), s, 1));
    let cell = |i: usize| {
        let x = kr.right() + STEP * k + if i == 0 { 0.0 } else { w0 + STEP * k };
        let w = if i == 0 { w0 } else { w1 };
        Rect::from_min_size(Pos2::new(x, kr.top()), Vec2::new(w, kr.height()))
    };
    // MIDI in: lit for a moment whenever an event comes in; the ports open.
    let received = app.engine.midi_sender().received();
    let midi_lit = app.topbar.midi_lamp(received);
    if midi_lit {
        // Look again when the lamp is due to go out.
        crate::views::workspace_ticker::wake_after(ui.ctx(), std::time::Duration::from_millis(160));
    }
    let ports = app.midi_port_names.len();
    let armed = app.project().tracks.iter().filter(|t| t.armed).count();
    let (mr, ar) = (cell(0), cell(1));
    let m = ui.interact(mr, Id::new("toolbar-midi"), Sense::click());
    let a = ui.interact(ar, Id::new("toolbar-armed"), Sense::click());
    let (m_clicked, a_clicked) = (m.clicked(), a.clicked());
    // The MIDI cell is also Live's *MIDI* key (round 29, ADR 0062): a click turns the
    // learn mode on and off, a right-click opens the MIDI settings.
    let m_settings = m.secondary_clicked();
    let learning = app.controllers.learn_mode;
    WORDS.with(|words| {
        let mut words = words.borrow_mut();
        let (mv, av) = &mut *words;
        mv.clear();
        av.clear();
        let _ = write!(mv, "{ports}");
        let _ = write!(av, "{armed}");
        let (word0, _) = lamp_parts(ui.ctx(), s, 0);
        let (word1, _) = lamp_parts(ui.ctx(), s, 1);
        lamp_cell(ui, mr, s, midi_lit, Tok::Led, LAMP_WORDS[0], mv, word0);
        if learning {
            with_brush(ui.painter(), |b| {
                b.rrect(
                    mr.shrink(b.px()),
                    s.r(3.0),
                    Fill::Solid(s.t.highlight.gamma_multiply(0.2)),
                );
                b.rrect_stroke(mr.shrink(b.px()), s.r(3.0), s.s(1.5), s.t.highlight);
            });
        }
        lamp_cell(ui, ar, s, armed > 0, Tok::Danger, LAMP_WORDS[1], av, word1);
    });
    if m.hovered() {
        let tip = if ports == 0 {
            t("topbar.midi_none")
        } else {
            crate::i18n::tf_count(
                "topbar.midi_tip",
                ports,
                &[("ports", &app.midi_port_names.join(", "))],
            )
        };
        // With a port open the cell is Live's MIDI key: a click learns, a right-click
        // opens the settings; with none, a click opens the settings (review 1, #29, #47).
        let tip = if ports == 0 {
            tip
        } else {
            let learn = match app.keymap.bindings_for(Action::MidiLearnMode).first() {
                Some(b) => tf("ctl.topbar.lamp_tip_key", &[("key", &b.display())]),
                None => t("ctl.topbar.lamp_tip"),
            };
            format!("{tip}. {learn}")
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
        let tip = tip_with_key(app, tip, Action::ArmTrack);
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = a.on_hover_text(tip);
    }
    if a_clicked {
        app.run_action(Action::ArmTrack);
    }
}

fn tip_with_key(app: &CantusApp, text: String, action: Action) -> String {
    match app.keymap.bindings_for(action).first() {
        Some(b) => format!("{text} ({})", b.display()),
        None => text,
    }
}

// ---- the tools -----------------------------------------------------------------------

fn zoom_w(ctx: &egui::Context, s: &Style) -> f32 {
    // The kit's menu field: the value in the control role, 7 pt before it, the ▾ after.
    text::measure(ctx, &Text::new("200 %", Role::Control, s.t.text), s.tk()).x + 28.0 * s.k
}

pub fn tools_width(ctx: &egui::Context, s: &Style) -> f32 {
    (2.0 * ROW_KEY + STEP) * s.k + zoom_w(ctx, s)
}

pub fn tools(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let pick = app.appearance_page.picking;
    let open = app.show_theme_editor;
    let [p, g] = key_strip(
        ui,
        r.left(),
        r,
        s,
        [
            (KeyFace::Glyph(Glyph::Pick), pick, "pick"),
            (KeyFace::Glyph(Glyph::Gear), open, "appearance"),
        ],
    );
    if strip_tip(ui, p, || {
        tip(app, "theme.inspect", Some(Action::PickElement))
    })
    .clicked()
    {
        app.toggle_pick();
    }
    if strip_tip(ui, g, || t("settings.appearance")).clicked() {
        app.show_theme_editor = !open;
    }
    let kr = key_rect(r, r.left() + key_side(r, s), s);
    // The interface's zoom: the value, a click for the steps (Ctrl + / Ctrl −).
    let zr = Rect::from_min_size(
        Pos2::new(kr.right() + STEP * k, kr.top()),
        Vec2::new(zoom_w(ui.ctx(), s), kr.height()),
    );
    let resp = ui.interact(zr, Id::new("toolbar-zoom"), Sense::click());
    let popup = egui::Popup::default_response_id(&resp);
    let is_open = egui::Popup::is_id_open(ui.ctx(), popup);
    let now = app.settings.ui_scale();
    WORDS.with(|words| {
        let mut words = words.borrow_mut();
        let v = &mut words.0;
        v.clear();
        let _ = write!(v, "{:.0} %", now * 100.0);
        let st = State::of(&resp, is_open);
        with_brush(ui.painter(), |b| {
            crate::kit::well::menu_field(b, zr, s, st, v, false)
        });
        if st.hovered {
            let name = tip(app, "topbar.zoom", Some(Action::ZoomIn));
            let caps = Caps {
                menu: true,
                ..Caps::NONE
            };
            hint::set_text(ui.ctx(), &name, v, caps);
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
