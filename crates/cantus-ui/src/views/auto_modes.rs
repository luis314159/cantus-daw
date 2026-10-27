//! Automation modes in the interface (ADR 0068): the top bar's **arm** (Ableton's
//! *automation arm*) and its ▾ — *set all tracks to…*, the touch glide — the mode badge on
//! each track header (Pro Tools' R / W / T / L), the inspector's row, and the menu every one
//! of them opens. Drawn with the kit; the words are the `automodes` area's.

use crate::app::CantusApp;
use crate::kit::cap::KeyFace;
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Policy, Role, Src, Text};
use crate::kit::{self, cap, with_brush, Brush, State, Style};
use cantus_core::{AutoMode, TrackId};
use egui::{vec2, Align2, Color32, Id, Rect, Response, Sense, Ui};

/// A mode badge's width and height at 100 %.
pub const BADGE_W: f32 = 15.0;
pub const BADGE_H: f32 = 13.0;

/// The colour a mode is drawn in: Write the record red (it overwrites), Touch the meters'
/// amber, Latch the modulators' hue; Read has none (an outline).
pub fn mode_color(mode: AutoMode, s: &Style) -> Option<Color32> {
    match mode {
        AutoMode::Read => None,
        AutoMode::Write => Some(s.t.danger),
        AutoMode::Touch => Some(s.t.meter_high),
        AutoMode::Latch => Some(s.t.hue_mod),
    }
}

/// A badge: the mode's letter in its colour, filled while the arm is on (it writes), an
/// outline while it is off (it would).
pub fn paint_badge(b: &mut Brush, rect: Rect, mode: AutoMode, armed: bool, hot: bool, s: &Style) {
    let r = s.r(2.5);
    let t = &s.t;
    let (fill, ink) = match mode_color(mode, s) {
        Some(c) if armed => (Some(c), kit::color::ensure_contrast(t.text, c, 4.5)),
        Some(c) => (Some(kit::color::with_alpha(c, 0.22)), c),
        None => (None, t.text_dim),
    };
    if let Some(f) = fill {
        b.rrect(rect, r, kit::Fill::Solid(f));
    }
    // Read is the letter alone (a header full of tracks in Read stays quiet and cheap);
    // its box shows under the pointer.
    let edge = match mode_color(mode, s) {
        Some(c) => Some(c),
        None => hot.then(|| kit::color::with_alpha(t.text_dim, 0.7)),
    };
    if let Some(edge) = edge {
        b.rrect_stroke(rect, r, if hot { 1.5 * s.k } else { b.px() }, edge);
    }
    let letter = Text::key(mode.badge_key(), Role::Micro, ink)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Abbreviate);
    let _ = text::paint(b, rect, &letter, s.tk());
}

/// The four modes as a menu, the current one checked; the one picked.
pub fn mode_menu(ui: &mut Ui, current: Option<AutoMode>) -> Option<AutoMode> {
    let items: Vec<kit::menu::Item<'_>> = AutoMode::ALL
        .iter()
        .map(|m| kit::menu::Item::Action {
            label: Src::Key(m.label_key()),
            check: kit::menu::Check::Radio(current == Some(*m)),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    let picked = kit::widgets::menu_list(ui, &items, 200.0).1.clicked?;
    AutoMode::ALL.get(picked).copied()
}

/// A track's mode for the hint: "Touch — writes while a control is held, …".
fn hint_for(ui: &Ui, mode: AutoMode, armed: bool) {
    let name = crate::i18n::t(mode.label_key());
    let mut body = crate::i18n::t(mode.hint_key());
    if mode.writes() && !armed {
        body.push(' ');
        body.push_str(&crate::i18n::t("automodes.hint.disarmed"));
    }
    hint::set_text(
        ui.ctx(),
        &name,
        &body,
        Caps {
            menu: true,
            ..Caps::NONE
        },
    );
}

/// A track header's badge as a control: a click opens the modes.
pub fn badge_input(app: &mut CantusApp, ui: &mut Ui, track: TrackId, rect: Rect, clip: Rect) {
    let hit = rect.expand(2.0 * Style::current().k).intersect(clip);
    if hit.height() <= 0.0 {
        return;
    }
    let resp = ui.interact(hit, ui.id().with(("auto-mode", track.0)), Sense::click());
    let Some(mode) = app.project().track(track).map(|t| t.auto_mode) else {
        return;
    };
    if resp.hovered() {
        hint_for(ui, mode, app.auto_armed());
    }
    menu_on(app, &resp, Some(mode), move |app, m| {
        app.exec(cantus_core::Command::SetTrackAutoMode { track, mode: m });
    });
}

/// The modes' menu under `resp` (a click or a right-click), running `set` with the pick.
pub fn menu_on(
    app: &mut CantusApp,
    resp: &Response,
    current: Option<AutoMode>,
    set: impl FnOnce(&mut CantusApp, AutoMode),
) {
    let mut picked = None;
    let frame = crate::views::clips_chrome::menu_frame();
    let shown = egui::Popup::menu(resp)
        .frame(frame)
        .show(|ui| {
            if let Some(m) = mode_menu(ui, current) {
                picked = Some(m);
                ui.close();
            }
        })
        .is_some();
    if !shown {
        egui::Popup::context_menu(resp).frame(frame).show(|ui| {
            if let Some(m) = mode_menu(ui, current) {
                picked = Some(m);
                ui.close();
            }
        });
    }
    if let Some(m) = picked {
        set(app, m);
    }
}

// ---- the top bar ---------------------------------------------------------------------------

/// The ▾'s width beside the arm key.
pub const MORE_W: f32 = 14.0;

fn popup_id() -> Id {
    Id::new("toolbar-automation-options")
}

/// The arm key (lit while armed; a click arms and disarms, a right-click opens the
/// options) and its ▾: what the top bar's transport group ends with.
pub fn arm_keys(app: &mut CantusApp, ui: &mut Ui, key: Rect, more: Rect, s: &Style) {
    let armed = app.auto_armed();
    let resp = ui.interact(key, Id::new("toolbar-automation-arm"), Sense::click());
    let st = State::of(&resp, armed);
    with_brush(ui.painter(), |b| {
        cap::toolbar_key(b, key, s, st, KeyFace::Glyph(Glyph::Automate));
    });
    if resp.hovered() {
        let body = if armed {
            crate::i18n::t("automodes.arm.on_tip")
        } else {
            crate::i18n::t("automodes.arm.off_tip")
        };
        let shortcut = app
            .keymap
            .bindings_for(crate::keymap::Action::ToggleAutomationArm)
            .first()
            .map(|b| b.display());
        let name = match shortcut {
            Some(k) => format!("{} ({k})", crate::i18n::t("automodes.arm")),
            None => crate::i18n::t("automodes.arm"),
        };
        hint::set_text(
            ui.ctx(),
            &name,
            &body,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    crate::controllers::learn_action_spot(app, ui, key, crate::keymap::Action::ToggleAutomationArm);
    if resp.clicked() {
        app.run_action(crate::keymap::Action::ToggleAutomationArm);
    }
    let m = ui.interact(more, popup_id().with("key"), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), popup_id());
    let mst = State::of(&m, open);
    with_brush(ui.painter(), |b| {
        // A flat ▾ (the arm key beside it is the cap): lit under the pointer or open.
        if mst.hovered || open {
            b.rrect(
                more,
                s.r(2.0),
                kit::Fill::Solid(kit::color::with_alpha(s.t.highlight, 0.18)),
            );
        }
        let g = Rect::from_center_size(more.center(), vec2(8.0 * s.k, 8.0 * s.k));
        let c = if open {
            s.t.highlight_glyph
        } else {
            s.t.text_dim
        };
        glyph::paint(b, g, Glyph::Down, c);
    });
    if m.hovered() {
        hint::set_text(
            ui.ctx(),
            &crate::i18n::t("automodes.options"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let toggle = m.clicked() || resp.secondary_clicked();
    if !open && !toggle {
        return;
    }
    let anchor = m.union(resp);
    egui::Popup::from_response(&anchor)
        .id(popup_id())
        .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(crate::views::record_options::frame_for(s))
        .gap(4.0)
        .show(|ui| options(app, ui));
}

/// The options' popover: arm, every track's mode at once, the touch glide.
fn options(app: &mut CantusApp, ui: &mut Ui) {
    use crate::views::record_options::{band_row, label, CONTROL_W};
    let s = Style::current();
    ui.spacing_mut().item_spacing = vec2(4.0 * s.k, 3.0 * s.k);
    band_row(ui, "automodes.options", &s);
    ui.horizontal(|ui| {
        label(ui, "automodes.arm", &s);
        let mut on = app.auto_armed();
        if kit::widgets::led_button(
            ui,
            &mut on,
            Src::Key("automodes.arm.armed"),
            &crate::i18n::t("automodes.arm"),
            90.0,
        )
        .changed()
        {
            app.set_auto_armed(on);
        }
    });
    ui.horizontal(|ui| {
        label(ui, "automodes.set_all", &s);
        let p = app.project();
        let first = p.tracks.first().map_or(p.master.auto_mode, |t| t.auto_mode);
        let common = (p.master.auto_mode == first && p.tracks.iter().all(|t| t.auto_mode == first))
            .then_some(first);
        let mut i = common
            .and_then(|m| AutoMode::ALL.iter().position(|x| *x == m))
            .unwrap_or(AutoMode::ALL.len());
        let labels = AutoMode::ALL.map(|m| Src::Key(m.label_key()));
        if kit::widgets::segmented(
            ui,
            &mut i,
            &labels,
            &crate::i18n::t("automodes.set_all"),
            CONTROL_W,
        )
        .changed()
        {
            if let Some(m) = AutoMode::ALL.get(i) {
                app.set_all_auto_modes(*m);
            }
        }
    });
    ui.horizontal(|ui| {
        label(ui, "automodes.glide", &s);
        let ms = app.settings.touch_glide_ms;
        let (v, done) = glide_field(ui, ms, &s);
        app.settings.touch_glide_ms = v;
        // Written to disk once, when the drag ends — not every frame of it.
        if done {
            app.store_settings();
        }
    });
}

/// The touch glide, ms: a value in a well, dragged (0…5 000 ms; Shift for fine),
/// double-click for its default. Returns the value — unrounded while a drag goes on, so a
/// fine drag adds up — and whether to keep it (the drag ended, a double-click).
pub fn glide_field(ui: &mut Ui, ms: f32, s: &Style) -> (f32, bool) {
    use crate::settings::DEFAULT_TOUCH_GLIDE_MS;
    let (r, resp) = ui.allocate_exact_size(vec2(90.0 * s.k, s.d(22.0)), Sense::click_and_drag());
    let mut v = if ms.is_finite() {
        ms
    } else {
        DEFAULT_TOUCH_GLIDE_MS
    };
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        let d = resp.drag_delta();
        v = glide_drag(v, d.x - d.y, fine);
    }
    if resp.double_clicked() {
        v = DEFAULT_TOUCH_GLIDE_MS;
    }
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        kit::well::paint(b, r, s, st, None);
        let words = crate::i18n::tf("automodes.glide_ms", &[("ms", &format!("{:.0}", v))]);
        let t = Text::new(&words, Role::Value, s.t.text).align(Align2::CENTER_CENTER);
        let _ = text::paint(b, r, &t, s.tk());
    });
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            &crate::i18n::t("automodes.glide"),
            &crate::i18n::t("automodes.glide_tip"),
            Caps::PARAM,
        );
    }
    let done = resp.drag_stopped() || resp.double_clicked();
    (if done { v.round() } else { v }, done)
}

/// The inspector's row for a track's mode: the four modes, the one it is in lit.
pub fn inspector_row(app: &mut CantusApp, ui: &mut Ui, track: TrackId, width: f32) {
    let Some(mode) = app.project().track(track).map(|t| t.auto_mode) else {
        return;
    };
    let mut i = AutoMode::ALL.iter().position(|m| *m == mode).unwrap_or(0);
    let labels = AutoMode::ALL.map(|m| Src::Key(m.label_key()));
    let resp = kit::widgets::segmented(
        ui,
        &mut i,
        &labels,
        &crate::i18n::t("automodes.mode"),
        width,
    );
    if resp.hovered() {
        hint_for(ui, mode, app.auto_armed());
    }
    if resp.changed() {
        app.exec(cantus_core::Command::SetTrackAutoMode {
            track,
            mode: AutoMode::ALL[i],
        });
    }
}

/// Open the top bar's automation options (the screenshots, a test).
pub fn open_options(ctx: &egui::Context) {
    egui::Popup::open_id(ctx, popup_id());
}

/// One frame of a drag on the touch glide: 5 ms a point (half a millisecond with Shift),
/// unrounded — rounded only when the drag ends — so a fine drag adds up.
pub fn glide_drag(ms: f32, points: f32, fine: f32) -> f32 {
    (ms + points * fine * 5.0).clamp(0.0, crate::settings::MAX_TOUCH_GLIDE_MS)
}

#[cfg(test)]
mod tests {
    /// Review #12: a fine drag of a fifth of a point a frame adds up (it was rounded back
    /// to the stored whole millisecond every frame).
    #[test]
    fn a_fine_drag_adds_up() {
        let mut ms = 250.0;
        for _ in 0..10 {
            ms = super::glide_drag(ms, 0.2, 0.1);
        }
        assert!((ms - 251.0).abs() < 1e-3, "{ms}");
    }
}
