//! The metronome group (ADR 0052; round 39, `dir-a`'s phase B): its key (Ctrl+M) and a ▾
//! in one well; on, the key is tinted with the accent and its glyph lit. The ▾ opens the
//! rest — the **count-in** (off, 1, 2, 4 bars), the click's **level**, its **voice**
//! (classic, woodblock, beep), the **accent** on the downbeat and **when** it sounds
//! (always, while recording, during the count-in). Schema 26 keeps the voice and the
//! accent with the song.

use super::paint as p;
use super::{strip_tip, tip, Style};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::keymap::Action;
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::{self, with_brush, State};
use crate::views::record_options as ro;
use cantus_core::{ClickMode, ClickSound};
use egui::{vec2, Id, Pos2, Rect, Sense, Ui};

pub(super) const COUNT_INS: [(u8, &str); 4] = [
    (0, "rec.opt.off"),
    (1, "rec.opt.bar1"),
    (2, "rec.opt.bar2"),
    (4, "rec.opt.bar4"),
];

/// The key's and the ▾'s widths at 100 %.
const KEY_W: f32 = 30.0;
const DOWN_W: f32 = 16.0;

pub fn width(s: &Style) -> f32 {
    (KEY_W + DOWN_W) * s.k
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let row = p::row(r, s);
    let c = super::keys::cells(row, [KEY_W, DOWN_W], s);
    let on = app.project().metronome;
    let key = ui.interact(c[0], Id::new(("toolbar-key", "metronome")), Sense::click());
    let down = ui.interact(c[1], Id::new("toolbar-metronome-more"), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), options_popup());
    with_brush(ui.painter(), |b| {
        b.register(row, "toolbar_key", kit::cap::WELL_KEY_TOKENS, &[]);
        p::joined(b, &c, s);
        let st = State::of(&key, on);
        p::tint(b, c[0], s, st, s.t.highlight, 0.2);
        p::icon(b, c[0], s, Glyph::Metronome, st, s.t.highlight, 24.0);
        let ds = State::of(&down, false);
        p::tint(b, c[1], s, ds, s.t.highlight, 0.2);
        let ink = if ds.hovered || open {
            p::ink1(s)
        } else {
            p::ink3(s)
        };
        p::chevron(
            b,
            Pos2::new(c[1].center().x - 0.5 * s.k, c[1].center().y),
            2.5 * s.k,
            ink,
            s,
        );
    });
    let key = strip_tip(ui, key, || {
        tip(app, "transport.metronome", Some(Action::ToggleMetronome))
    });
    if key.clicked() {
        app.run_action(Action::ToggleMetronome);
    }
    // MIDI learn mode (round 29, ADR 0062).
    crate::controllers::learn_action_spot(app, ui, c[0], Action::ToggleMetronome);
    if down.hovered() {
        let caps = Caps {
            menu: true,
            ..Caps::NONE
        };
        hint::set_text(ui.ctx(), &t("topbar.metronome_options"), "", caps);
    }
    let toggle = down.clicked();
    if !open && !toggle {
        return;
    }
    egui::Popup::from_response(&down)
        .id(options_popup())
        .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(ro::frame_for(s))
        .gap(4.0)
        .show(|ui| {
            ro::band_row(ui, "topbar.metronome", s);
            options(app, ui)
        });
}

/// *Also on play*: a global setting, saved at once.
fn set_on_play(app: &mut CantusApp, on: bool) {
    if app.settings.count_in_on_play != on {
        app.settings.count_in_on_play = on;
        app.store_settings();
    }
}

fn options_popup() -> Id {
    Id::new("toolbar-metronome-options")
}

/// Open the metronome's options (the screenshots, a test).
pub fn open_options(ctx: &egui::Context) {
    egui::Popup::open_id(ctx, options_popup());
}

/// The metronome's options: voice, accent, when, level, count-in.
pub(super) fn options(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    ui.spacing_mut().item_spacing = vec2(4.0 * s.k, 3.0 * s.k);
    let mut rs = app.project().recording.clone();
    let before = rs.clone();
    ui.horizontal(|ui| {
        ro::label(ui, "topbar.click_sound", &s);
        let mut i = ClickSound::ALL
            .iter()
            .position(|c| *c == rs.click_sound)
            .unwrap_or(0);
        let labels = [
            Src::Key("topbar.sound.classic"),
            Src::Key("topbar.sound.wood"),
            Src::Key("topbar.sound.beep"),
        ];
        let name = t("topbar.click_sound");
        if kit::widgets::segmented(ui, &mut i, &labels, &name, ro::CONTROL_W).changed() {
            rs.click_sound = ClickSound::ALL[i];
        }
    });
    ui.horizontal(|ui| {
        ro::label(ui, "topbar.accent", &s);
        let mut on = rs.click_accent;
        let name = t("topbar.accent_tip");
        if kit::widgets::led_button(ui, &mut on, Src::Key("topbar.accent_on"), &name, 110.0)
            .changed()
        {
            rs.click_accent = on;
        }
    });
    ui.horizontal(|ui| {
        ro::label(ui, "rec.click_when", &s);
        let modes = [
            ClickMode::Always,
            ClickMode::Recording,
            ClickMode::CountInOnly,
        ];
        let mut i = modes.iter().position(|m| *m == rs.click).unwrap_or(0);
        let labels = [
            Src::Key("rec.click.always"),
            Src::Key("rec.click.recording"),
            Src::Key("rec.click.count_in"),
        ];
        let name = t("rec.click_when");
        if kit::widgets::segmented(ui, &mut i, &labels, &name, ro::CONTROL_W).changed() {
            rs.click = modes[i];
        }
    });
    let mut level_done = false;
    ui.horizontal(|ui| {
        ro::label(ui, "rec.click_level", &s);
        let (v, ended) = ro::level(ui, rs.click_db, &s);
        rs.click_db = v;
        level_done = ended;
    });
    ui.horizontal(|ui| {
        ro::label(ui, "rec.count_in", &s);
        let mut i = COUNT_INS
            .iter()
            .position(|(b, _)| *b == rs.count_in_bars)
            .unwrap_or(0);
        let labels = COUNT_INS.map(|(_, k)| Src::Key(k));
        let name = t("rec.count_in");
        if kit::widgets::segmented(ui, &mut i, &labels, &name, ro::CONTROL_W).changed() {
            rs.count_in_bars = COUNT_INS[i].0;
        }
    });
    // Round 30: when the count-in plays — before recording, and on Play if asked.
    ui.horizontal(|ui| {
        ro::label(ui, "ship.count_in.on_play_label", &s);
        let mut on = app.settings.count_in_on_play;
        let name = t("ship.count_in.on_play");
        if kit::widgets::led_button(ui, &mut on, Src::Key("ship.count_in.on_play"), &name, 150.0)
            .changed()
        {
            set_on_play(app, on);
        }
    });
    if rs != before {
        app.set_record_settings(rs);
    }
    if level_done || ui.input(|i| i.pointer.any_released()) {
        app.end_gesture();
    }
}
