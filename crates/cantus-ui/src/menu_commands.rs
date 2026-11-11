//! **Every menu row the keyboard reaches** (round 39, ADR 0110): the rows of the top bar's
//! menus that are not keymap actions — the imports and exports, the help pages, the clip's
//! colour and deletion, the device picker, the engine's statistics — as commands the
//! command palette offers under **the menu's own words**, and that the menus themselves
//! run, so a row and its palette entry can never do different things.
//!
//! Rows that are keymap actions show their shortcut in the menu (`action_item`) and are in
//! the palette already; the Settings pages and the windows are the palette's own targets.
//! `tests/keyboard_reach.rs` opens every menu through the window and fails on a row the
//! keyboard cannot reach.

use crate::app::CantusApp;

/// A menu row the palette runs too.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MenuCommand {
    CollectAndSave,
    ImportAudio,
    ImportCharacter,
    ImportMidi,
    ExportMidi,
    ExportStems,
    ImportDawProject,
    ExportDawProject,
    Quit,
    DevicePicker,
    SaveLayoutAs,
    EngineStatistics,
    ScanPlugins,
    Manual,
    ReleaseNotes,
    ReportBug,
    SuggestFeature,
    CheckForUpdates,
    Credits,
    ClipTrackColour,
    DeleteClips,
    // The empty timeline's menu.
    LinkCopies,
    AutoCrossfades,
    ClipGainEnvelopes,
    AddMetreChange,
    // A tab's menu, for the area being worked in.
    FloatArea,
    HideArea,
    ResetLayout,
    /// What a right-click on the selection opens: the clip's, the track's, the strip's menu
    /// (Shift+F10 outside the mixer), walked with the arrows.
    SelectionMenu,
    /// The modulator popup for the parameter moved last (the menus' *Modulate…*).
    ModulateLastTweaked,
}

impl MenuCommand {
    pub const ALL: [MenuCommand; 30] = [
        MenuCommand::CollectAndSave,
        MenuCommand::ImportAudio,
        MenuCommand::ImportCharacter,
        MenuCommand::ImportMidi,
        MenuCommand::ExportMidi,
        MenuCommand::ExportStems,
        MenuCommand::ImportDawProject,
        MenuCommand::ExportDawProject,
        MenuCommand::Quit,
        MenuCommand::DevicePicker,
        MenuCommand::SaveLayoutAs,
        MenuCommand::EngineStatistics,
        MenuCommand::ScanPlugins,
        MenuCommand::Manual,
        MenuCommand::ReleaseNotes,
        MenuCommand::ReportBug,
        MenuCommand::SuggestFeature,
        MenuCommand::CheckForUpdates,
        MenuCommand::Credits,
        MenuCommand::ClipTrackColour,
        MenuCommand::DeleteClips,
        MenuCommand::LinkCopies,
        MenuCommand::AutoCrossfades,
        MenuCommand::ClipGainEnvelopes,
        MenuCommand::AddMetreChange,
        MenuCommand::FloatArea,
        MenuCommand::HideArea,
        MenuCommand::ResetLayout,
        MenuCommand::SelectionMenu,
        MenuCommand::ModulateLastTweaked,
    ];

    /// The i18n key of its words: the very key its menu row shows.
    pub fn key(self) -> &'static str {
        match self {
            MenuCommand::CollectAndSave => "audioclips.menu.collect",
            MenuCommand::ImportAudio => "audioclips.menu.import",
            MenuCommand::ImportCharacter => "dance_import.menu",
            MenuCommand::ImportMidi => "interop.menu.import_midi",
            MenuCommand::ExportMidi => "interop.menu.export_midi",
            MenuCommand::ExportStems => "interop.menu.export_stems",
            MenuCommand::ImportDawProject => "interop.menu.import_dawproject",
            MenuCommand::ExportDawProject => "interop.menu.export_dawproject",
            MenuCommand::Quit => "menu.quit",
            MenuCommand::DevicePicker => "picker.menu.open_picker",
            MenuCommand::SaveLayoutAs => "layout.save_as",
            MenuCommand::EngineStatistics => "menu.stats",
            MenuCommand::ScanPlugins => "host.manager.scan",
            MenuCommand::Manual => "about.menu.manual",
            MenuCommand::ReleaseNotes => "about.menu.notes",
            MenuCommand::ReportBug => "about.menu.bug",
            MenuCommand::SuggestFeature => "about.menu.feature",
            MenuCommand::CheckForUpdates => "update.menu.check",
            MenuCommand::Credits => "about.menu.credits",
            MenuCommand::ClipTrackColour => "clip.color_reset",
            MenuCommand::DeleteClips => "clip.delete",
            MenuCommand::LinkCopies => "links.copies",
            MenuCommand::AutoCrossfades => "clipfx.menu.auto_crossfade",
            MenuCommand::ClipGainEnvelopes => "clipfx.menu.show_gain",
            MenuCommand::AddMetreChange => "tempo.menu.add_metre_playhead",
            MenuCommand::FloatArea => "ux.panels.float_area",
            MenuCommand::HideArea => "ux.panels.hide_area",
            MenuCommand::ResetLayout => "layout.reset",
            MenuCommand::SelectionMenu => "ux.panels.selection_menu",
            MenuCommand::ModulateLastTweaked => "ux.panels.modulate_last",
        }
    }

    /// Other words that find it: the menus' own rows for the same thing.
    pub fn also(self) -> &'static [&'static str] {
        match self {
            MenuCommand::DevicePicker => &["swap.add_instrument"],
            MenuCommand::AddMetreChange => &["tempo.menu.add_metre"],
            MenuCommand::FloatArea => &["workspace.menu.float", "workspace.menu.dock"],
            MenuCommand::HideArea => &["workspace.menu.hide"],
            _ => &[],
        }
    }

    /// The stable key its palette uses are counted by.
    pub fn id(self) -> String {
        format!("menu:{self:?}")
    }

    /// Do what its menu row does.
    pub fn run(self, app: &mut CantusApp, ctx: &egui::Context) {
        use crate::views;
        match self {
            MenuCommand::CollectAndSave => app.save(),
            MenuCommand::ImportAudio => app.import_audio_dialog(),
            MenuCommand::ImportCharacter => crate::dance_import::menu_pick(app),
            MenuCommand::ImportMidi => views::interop::import_midi_dialog(app),
            MenuCommand::ExportMidi => views::interop::export_midi_dialog(app),
            MenuCommand::ExportStems => views::interop::export_stems_dialog(app),
            MenuCommand::ImportDawProject => views::interop::import_dawproject_dialog(app),
            MenuCommand::ExportDawProject => views::interop::export_dawproject_dialog(app),
            MenuCommand::Quit => app.request_quit = true,
            MenuCommand::DevicePicker => {
                app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None })
            }
            MenuCommand::SaveLayoutAs => {
                let current = app.layouts.current.name.clone();
                // Offer the current name, so saving a layout again is one keystroke.
                app.naming_layout = Some(if current == crate::layout::DEFAULT_NAME {
                    String::new()
                } else {
                    current
                });
            }
            MenuCommand::EngineStatistics => app.show_stats = !app.show_stats,
            MenuCommand::ScanPlugins => views::plugins::scan_now(app),
            MenuCommand::Manual => crate::docs::open(crate::docs::Doc::Manual),
            MenuCommand::ReleaseNotes => crate::docs::open(crate::docs::Doc::ReleaseNotes),
            MenuCommand::ReportBug => crate::docs::open(crate::docs::Doc::ReportBug),
            MenuCommand::SuggestFeature => crate::docs::open(crate::docs::Doc::RequestFeature),
            MenuCommand::CheckForUpdates => views::update::check_now(app),
            MenuCommand::Credits => views::about::show_credits(app, ctx),
            MenuCommand::ClipTrackColour => app.set_clip_color(None),
            MenuCommand::DeleteClips => app.delete_clip_selection(),
            MenuCommand::LinkCopies => app.toggle_link_copies(),
            MenuCommand::AutoCrossfades => {
                let on = !app.settings.auto_crossfade;
                app.set_auto_crossfade(on);
            }
            MenuCommand::ClipGainEnvelopes => app.toggle_clip_gain(),
            MenuCommand::AddMetreChange => {
                let at = app.engine.position().max(cantus_core::Ticks::ZERO);
                views::arrangement::tempo::add_metre_change(app, at);
            }
            MenuCommand::FloatArea => {
                if let Some(p) = app.workspace.focused_area() {
                    app.workspace.toggle_float(p);
                }
            }
            MenuCommand::HideArea => {
                if let Some(p) = app.workspace.focused_area() {
                    app.workspace.hide(p);
                }
            }
            MenuCommand::ResetLayout => app.reset_layout(),
            MenuCommand::SelectionMenu => {
                if app.focus == crate::app::EditFocus::Mixer {
                    if let Some(s) = app.selected_strip {
                        let at = ctx.content_rect().center();
                        views::mixer::open_menu(app, Some(s), at);
                    }
                } else {
                    app.open_context_menu_by_key();
                }
            }
            MenuCommand::ModulateLastTweaked => {
                if let Some(t) = app.automation.last_tweaked.clone() {
                    app.modulators.popup = Some((t, egui::Rect::NOTHING));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_words_in_both_languages_and_a_distinct_key() {
        let mut ids = std::collections::BTreeSet::new();
        for c in MenuCommand::ALL {
            for l in crate::i18n::Locale::ALL {
                let w = crate::i18n::t_in(l, c.key());
                assert!(!w.is_empty() && w != c.key(), "{c:?} in {l:?}");
            }
            assert!(ids.insert(c.id()), "{c:?}");
        }
    }
}
