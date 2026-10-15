//! The top bar's groups of keys (ADR 0052): PAT / SONG, the transport, undo, the windows,
//! the inputs (typing keyboard, MIDI, armed tracks) and the tools (picker, appearance,
//! zoom).

use super::{key_in, key_rect, tip, Style, KEY, STEP};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::keymap::Action;
use crate::kit::cap::KeyFace;

use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::tokens::Tok;
use crate::kit::{led, light, with_brush, State};
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
    (4.0 * (KEY + STEP) + views::record_options::KEY_W + STEP + KEY) * s.k
}

pub fn transport(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let step = (KEY + STEP) * s.k;
    let mut x = r.left();
    let playing = app.engine.is_playing();
    let recording = app.is_recording();
    let keys: [(Glyph, bool, &str, &str, Action); 4] = [
        (
            Glyph::ToStart,
            false,
            "to-start",
            "transport.to_start",
            Action::ToStart,
        ),
        (
            if playing { Glyph::Pause } else { Glyph::Play },
            playing,
            "play",
            "transport.play",
            Action::PlayPause,
        ),
        (Glyph::Stop, false, "stop", "transport.stop", Action::Stop),
        (
            Glyph::Record,
            recording,
            "record",
            "transport.record",
            Action::Record,
        ),
    ];
    let mut record = None;
    for (g, lit, id, key, action) in keys {
        let kr = key_rect(r, x, s);
        let resp = key_in(ui, kr, s, KeyFace::Glyph(g), lit, id, || {
            tip(app, key, Some(action))
        });
        if resp.clicked() {
            app.run_action(action);
        }
        if id == "record" {
            record = Some(resp);
        }
        x += step;
    }
    // The record options hang from a narrow ▾ beside the record key (ADR 0025).
    if let Some(record) = record {
        let w = views::record_options::KEY_W * s.k;
        let kr = key_rect(r, x - STEP * s.k, s);
        let kr = Rect::from_min_size(kr.min, Vec2::new(w, kr.height()));
        views::record_options::key(app, ui, kr, &record, s);
        x += w;
    }
    let looping = app.project().loop_enabled;
    let kr = key_rect(r, x, s);
    if key_in(
        ui,
        kr,
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
}

// ---- undo ----------------------------------------------------------------------------

pub fn undo_width(s: &Style) -> f32 {
    (2.0 * KEY + STEP) * s.k
}

pub fn undo(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let undo_tip = || {
        app.bus
            .undo_label_key()
            .map(|k| tf("menu.undo_named", &[("what", &t(k))]))
            .unwrap_or_else(|| tip(app, "menu.undo", Some(Action::Undo)))
    };
    let kr = key_rect(r, r.left(), s);
    let u = key_in(
        ui,
        kr,
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
    let kr = key_rect(r, r.left() + (KEY + STEP) * s.k, s);
    let rr = key_in(
        ui,
        kr,
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

// ---- the windows ---------------------------------------------------------------------

pub fn windows_width(s: &Style) -> f32 {
    (6.0 * KEY + 5.0 * STEP) * s.k
}

pub fn windows(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
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
        let kr = key_rect(r, x, s);
        let tip = || tip(app, pane.toggle_key(), Some(action));
        if key_in(
            ui,
            kr,
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
        x += (KEY + STEP) * s.k;
    }
}

// ---- the inputs ----------------------------------------------------------------------

const LAMP_WORDS: [&str; 2] = ["topbar.midi", "topbar.armed"];

fn lamp_w(ctx: &egui::Context, s: &Style) -> f32 {
    let words = LAMP_WORDS
        .iter()
        .map(|k| word_w(ctx, s, k, Role::Micro))
        .fold(0.0, f32::max);
    let digits = text::measure(ctx, &Text::new("88", Role::Value, s.t.text), s.tk()).x;
    words.max(digits) + 24.0 * s.k
}

pub fn inputs_width(ctx: &egui::Context, s: &Style) -> f32 {
    KEY * s.k + 2.0 * (STEP * s.k + lamp_w(ctx, s))
}

/// A lamp and a word over a number, in a well: the MIDI input and the armed tracks.
fn lamp_cell(ui: &Ui, r: Rect, s: &Style, lit: bool, color: Tok, word: &str, value: &str) {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        light::sunken(b, r, s.r(3.0), s.t.well, 1.0, s.t.depth);
        let top = Rect::from_min_size(r.min, Vec2::new(r.width(), r.height() * 0.5));
        let c = Pos2::new(r.left() + 9.0 * k, top.center().y + 1.0 * k);
        led::paint(b, c, 7.0 * k, lit, s, color, Tok::Well, 1.0);
        let wr = Rect::from_min_max(Pos2::new(c.x + 6.0 * k, top.top()), top.max);
        let _ = text::paint(
            b,
            wr.shrink2(Vec2::new(2.0 * k, 0.0)),
            &Text::key(word, Role::Micro, s.t.text_dim)
                .caps(true)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        let vr = Rect::from_min_max(Pos2::new(r.left(), top.bottom()), r.max);
        let ink = if lit { s.t.text } else { s.t.text_dim };
        let _ = text::paint(
            b,
            vr.shrink2(Vec2::new(6.0 * k, 0.0)),
            &Text::new(value, Role::Value, ink)
                .align(Align2::CENTER_CENTER)
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
    let w = lamp_w(ui.ctx(), s);
    let cell = |i: usize| {
        Rect::from_min_size(
            Pos2::new(
                kr.right() + STEP * k + i as f32 * (w + STEP * k),
                r.center().y - 17.0 * k,
            ),
            Vec2::new(w, 34.0 * k),
        )
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
    WORDS.with(|words| {
        let mut words = words.borrow_mut();
        let (mv, av) = &mut *words;
        mv.clear();
        av.clear();
        let _ = write!(mv, "{ports}");
        let _ = write!(av, "{armed}");
        lamp_cell(ui, mr, s, midi_lit, Tok::Led, LAMP_WORDS[0], mv);
        lamp_cell(ui, ar, s, armed > 0, Tok::Danger, LAMP_WORDS[1], av);
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
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = m.on_hover_text(tip);
    }
    if m_clicked {
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
    (2.0 * KEY + 2.0 * STEP) * s.k + zoom_w(ctx, s)
}

pub fn tools(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let pick = app.appearance_page.picking;
    let kr = key_rect(r, r.left(), s);
    if key_in(ui, kr, s, KeyFace::Glyph(Glyph::Pick), pick, "pick", || {
        tip(app, "theme.inspect", Some(Action::PickElement))
    })
    .clicked()
    {
        app.toggle_pick();
    }
    let open = app.show_theme_editor;
    let kr = key_rect(r, r.left() + (KEY + STEP) * k, s);
    if key_in(
        ui,
        kr,
        s,
        KeyFace::Glyph(Glyph::Gear),
        open,
        "appearance",
        || t("settings.appearance"),
    )
    .clicked()
    {
        app.show_theme_editor = !open;
    }
    // The interface's zoom: the value, a click for the steps (Ctrl + / Ctrl −).
    let zr = Rect::from_min_size(
        Pos2::new(kr.right() + STEP * k, r.center().y - s.d(11.0)),
        Vec2::new(zoom_w(ui.ctx(), s), s.d(22.0)),
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
