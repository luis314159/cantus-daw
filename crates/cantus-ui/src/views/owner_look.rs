//! The owner's answers on the look (round 40, `look`; the owner, 2026-11-12, round 39's
//! decisions 1, 2, 4 and 12): five choices of this machine, each with the owner's answer as
//! its default, in Settings ▸ Appearance under *Layout and controls* — never in the project.
//!
//! - **Maximise keeps** the inspector beside the maximised area (the default, des-arrange's
//!   `max-*` sheet) or gives the area the whole workspace (round 39's).
//! - **The top bar** may carry the automation arm with its modes and the typing keyboard's
//!   key again (round 31's); the default is round 39's bar without them.
//! - **The mixer's right dock** pinned at the window's edge (the default, des-mixer's sheet),
//!   the room before it filled with the next empty strips, or just after the last strip.
//! - **The inspector's values** as value bars (the default, des-controls' dense rows) or as
//!   round 39's slim 3-pt bars.
//! - **Mute on** in colour (the default: the M key of a muted strip lit in the theme's
//!   accent, as Logic and Studio One light theirs) or grey (round 39's).

use super::workspace_settings::LABEL_W;
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::text::Src;
use crate::kit::widgets::segmented;
use egui::Ui;
use serde::{Deserialize, Serialize};

/// What a maximised area leaves beside it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaximiseKeeps {
    /// The inspector stays beside the area at its width (the owner's answer).
    #[default]
    Inspector,
    /// The area takes the whole workspace (round 39).
    Nothing,
}

/// Where the mixer's right dock (the returns) sits.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockPlace {
    /// At the window's edge, the room before it filled with the next empty strips.
    #[default]
    Edge,
    /// Just after the last strip (round 37–39).
    AfterStrips,
}

/// How the inspector draws a value row.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueStyle {
    /// A value bar: a well filled to the value, the name at its left, the number at its
    /// right (des-controls' dense rows).
    #[default]
    Bars,
    /// The label, a slim 3-pt bar with its handle, the number (round 39).
    Slim,
}

/// How the M key of a muted strip is lit.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MuteOn {
    /// In the theme's accent, its ink on it.
    #[default]
    Colour,
    /// In the second ink (round 39: muting takes sound away, so it greys out).
    Grey,
}

/// The five choices, kept in `settings.json` (`look`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LookSettings {
    pub maximise: MaximiseKeeps,
    /// The automation arm and its modes on the top bar, after the loop key.
    pub topbar_automation: bool,
    /// The typing keyboard's key on the top bar, first in the inputs.
    pub topbar_typing: bool,
    pub mixer_dock: DockPlace,
    pub inspector_values: ValueStyle,
    pub mute_on: MuteOn,
}

/// The one width of the band's choices, so their right edges line up (review mid-round 40,
/// C-6): the widest one's, four words side by side.
const CHOICE_W: f32 = 440.0;

/// One row: a label and a segmented choice; returns the new index when it changed.
fn choice(ui: &mut Ui, label: &str, now: usize, keys: &[&'static str]) -> Option<usize> {
    let width = CHOICE_W;
    let mut sel = now;
    let mut changed = false;
    ui.horizontal(|ui| {
        w::row_label(ui, label, LABEL_W, 24.0);
        let labels: Vec<Src<'_>> = keys.iter().map(|k| Src::Key(k)).collect();
        changed = segmented(ui, &mut sel, &labels, &t(label), width).changed();
    });
    changed.then_some(sel)
}

/// Settings ▸ Appearance's *Layout and controls* section: its band, a note of what it holds
/// (as the neighbours' have), and the five choices.
pub fn settings_section(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("owner_look.band"));
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 18.0);
        ui.vertical(|ui| super::workspace_settings::note(ui, &t("owner_look.note"), false));
    });
    let look = app.settings.look.clone();
    let mut next = look.clone();
    if let Some(i) = choice(
        ui,
        "owner_look.maximise",
        usize::from(look.maximise == MaximiseKeeps::Nothing),
        &[
            "owner_look.maximise.inspector",
            "owner_look.maximise.nothing",
        ],
    ) {
        next.maximise = if i == 0 {
            MaximiseKeeps::Inspector
        } else {
            MaximiseKeeps::Nothing
        };
    }
    let bar = usize::from(look.topbar_automation) + 2 * usize::from(look.topbar_typing);
    if let Some(i) = choice(
        ui,
        "owner_look.topbar",
        bar,
        &[
            "owner_look.topbar.plain",
            "owner_look.topbar.automation",
            "owner_look.topbar.typing",
            "owner_look.topbar.both",
        ],
    ) {
        next.topbar_automation = i & 1 == 1;
        next.topbar_typing = i & 2 == 2;
    }
    // What the two extras are, under their choice (review 2 of round 40, R2-B5: in Spanish
    // the segments' short words alone lost it — the arm, not automation; the computer's
    // keyboard, not a MIDI one).
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 18.0);
        ui.vertical(|ui| super::workspace_settings::note(ui, &t("owner_look.topbar.note"), false));
    });
    if let Some(i) = choice(
        ui,
        "owner_look.dock",
        usize::from(look.mixer_dock == DockPlace::AfterStrips),
        &["owner_look.dock.edge", "owner_look.dock.after"],
    ) {
        next.mixer_dock = if i == 0 {
            DockPlace::Edge
        } else {
            DockPlace::AfterStrips
        };
    }
    if let Some(i) = choice(
        ui,
        "owner_look.values",
        usize::from(look.inspector_values == ValueStyle::Slim),
        &["owner_look.values.bars", "owner_look.values.slim"],
    ) {
        next.inspector_values = if i == 0 {
            ValueStyle::Bars
        } else {
            ValueStyle::Slim
        };
    }
    if let Some(i) = choice(
        ui,
        "owner_look.mute",
        usize::from(look.mute_on == MuteOn::Grey),
        &["owner_look.mute.colour", "owner_look.mute.grey"],
    ) {
        next.mute_on = if i == 0 { MuteOn::Colour } else { MuteOn::Grey };
    }
    if next != look {
        set(app, next);
    }
}

/// Take `next` as the look's choices: stored, and applied where one changes a shape now
/// (a maximised area keeps or gives back the inspector at once).
///
/// The others are read where they are drawn, every frame.
pub fn set(app: &mut CantusApp, next: LookSettings) {
    let was = std::mem::replace(&mut app.settings.look, next);
    publish(&app.settings.look);
    let keeps = app.settings.look.maximise;
    app.workspace.set_maximise_keeps(keeps);
    if was.maximise != keeps {
        if let Some(pane) = app.workspace.maximised() {
            app.workspace.restore_maximised();
            app.workspace.toggle_maximise(pane);
        }
    }
    app.store_settings();
}

/// Hand the workspace the settings it keeps a copy of (at start, after the settings load),
/// and the painters theirs.
pub fn apply(app: &mut CantusApp) {
    let keeps = app.settings.look.maximise;
    app.workspace.set_maximise_keeps(keeps);
    publish(&app.settings.look);
}

thread_local! {
    /// The choices painters read where no app is at hand (a strip's M key, an inspector
    /// row): the UI thread's copy, set by [`apply`] and [`set`].
    static LIVE: std::cell::Cell<(MuteOn, ValueStyle, bool)> =
        const { std::cell::Cell::new((MuteOn::Colour, ValueStyle::Bars, false)) };
}

fn publish(look: &LookSettings) {
    LIVE.with(|l| l.set((look.mute_on, look.inspector_values, look.topbar_typing)));
}

/// The fill of a lit M key: the theme's accent in colour, else `grey` (each view's own
/// grey, as round 39 painted it).
pub fn mute_fill(grey: egui::Color32, accent: egui::Color32) -> egui::Color32 {
    match LIVE.with(|l| l.get().0) {
        MuteOn::Colour => accent,
        MuteOn::Grey => grey,
    }
}

/// How the inspector draws its value rows now.
pub fn value_style() -> ValueStyle {
    LIVE.with(|l| l.get().1)
}

/// Whether the top bar carries the typing keyboard's key (its panel's inputs section too).
pub fn typing_key_shown() -> bool {
    LIVE.with(|l| l.get().2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::color::contrast;

    #[test]
    fn the_defaults_are_the_owners_answers() {
        let d = LookSettings::default();
        assert_eq!(d.maximise, MaximiseKeeps::Inspector);
        assert!(!d.topbar_automation && !d.topbar_typing);
        assert_eq!(d.mixer_dock, DockPlace::Edge);
        assert_eq!(d.inspector_values, ValueStyle::Bars);
        assert_eq!(d.mute_on, MuteOn::Colour);
        // A settings file from before round 40 has none of them: the defaults.
        let s: crate::settings::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.look, d);
        let back: LookSettings = serde_json::from_str(
            &serde_json::to_string(&LookSettings {
                maximise: MaximiseKeeps::Nothing,
                topbar_automation: true,
                topbar_typing: true,
                mixer_dock: DockPlace::AfterStrips,
                inspector_values: ValueStyle::Slim,
                mute_on: MuteOn::Grey,
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(back.mute_on, MuteOn::Grey);
        assert_eq!(back.mixer_dock, DockPlace::AfterStrips);
    }

    #[test]
    fn a_lit_m_reads_in_every_theme() {
        // The M's letter on the accent, as the desk (its lit-key ink) and the track
        // header (its base) paint it: text, 4.5:1 — and the lit key against the strip's
        // well, 3:1, in all twenty-four themes (ADR 0111).
        for (id, make) in crate::kit::appearance::PRESETS {
            let t = make().kit_tokens();
            let accent = t.highlight;
            let on_ink = if t.dark { t.ink } else { t.light };
            assert!(contrast(on_ink, accent) >= 4.5, "{id}: the desk's M");
            assert!(contrast(t.row_black, accent) >= 4.5, "{id}: the header's M");
            assert!(
                contrast(accent, t.well) >= 3.0,
                "{id}: the lit key on its well"
            );
        }
    }
}
