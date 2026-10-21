//! Hosted CLAP/VST3 instruments on tracks, from the application's side (ADR 0028): what
//! the device picker calls for `PickChoice::Hosted` when the plug-in is an instrument, the
//! plug-in's own window opened from the track, and the list of scanned instruments.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::{Command, TrackId};
use cantus_host::{PluginInfo, PluginKind};

impl CantusApp {
    /// Put the hosted instrument `info` on `track`: the plug-in is loaded here (to read its
    /// parameters and state), and the track's instrument is replaced by it in **one
    /// command** — one undo step, which brings the old instrument back. The clips stay.
    /// Returns whether it loaded; when it did not, the status line says why.
    ///
    /// This is the entry point for `PickChoice::Hosted` on an instrument purpose
    /// (`picker::apply_pick`).
    pub fn set_hosted_instrument(&mut self, track: TrackId, info: &PluginInfo) -> bool {
        if self.project().track(track).is_none() {
            return false;
        }
        match self.plugins.make_instrument(track, info) {
            Ok(spec) => {
                self.close_inst_editor();
                // An isolated plug-in's values, known once it loads, join this step.
                self.exec_merging(
                    Command::SetInstrument {
                        track,
                        spec: Box::new(spec),
                    },
                    cantus_host::PluginHost::inst_load_key(track),
                );
                true
            }
            Err(e) => {
                self.set_error(tf(
                    "host.load_failed",
                    &[("name", &info.name), ("error", &e)],
                ));
                false
            }
        }
    }

    /// Every instrument the plug-in scan found, CLAP first, by name.
    pub fn hosted_instruments(&self) -> Vec<PluginInfo> {
        let mut list: Vec<PluginInfo> = self
            .plugins
            .db
            .plugins()
            .into_iter()
            .filter(|p| p.kind == PluginKind::Instrument)
            .cloned()
            .collect();
        list.sort_by(|a, b| {
            (a.format as u8, a.name.to_lowercase()).cmp(&(b.format as u8, b.name.to_lowercase()))
        });
        list
    }

    /// Whether `track` hosts a CLAP/VST3 instrument.
    pub fn is_hosted_instrument(&self, track: TrackId) -> bool {
        self.project()
            .track(track)
            .is_some_and(|t| t.instrument.external().is_some())
    }

    /// Open the track's plug-in window, or bring it forward. A plug-in that is not on this
    /// computer has no window: the status line says so, and the track keeps its data.
    pub fn open_hosted_window(&mut self, track: TrackId) {
        let Some((track_name, plugin)) = self.project().track(track).and_then(|t| {
            t.instrument
                .external()
                .map(|p| (t.name.clone(), p.name.clone()))
        }) else {
            return;
        };
        if self.window_of_stopped(crate::views::plugin_notice::Target::Instrument(track)) {
            return;
        }
        if let Some(why) = self.plugins.instrument_missing(track) {
            let why = crate::views::plugin_notice::load_words(why);
            // On this computer but not loaded: Reload, never "install it" (isolation-5 #45).
            let key = if self.plugins.instrument_load_error(track).is_some() {
                "isolation.inst.not_loaded_status"
            } else {
                "hosted.missing_status"
            };
            self.set_error(tf(key, &[("name", &plugin), ("why", &why)]));
            return;
        }
        let has_editor = self
            .plugins
            .hosted_instrument(track)
            .is_some_and(|h| h.has_editor());
        if !has_editor {
            self.set_status(tf("hosted.no_window", &[("name", &plugin)]));
            return;
        }
        let title = format!("{plugin} \u{2014} {track_name}");
        let owner = self.plugins.owner_window;
        if let Err(e) = self.plugins.open_instrument_window(track, &title, owner) {
            crate::views::plugin_notice::window_failed(self, &plugin, &e);
        }
    }

    /// Open the track's plug-in window, or close it when it is open.
    pub fn toggle_hosted_window(&mut self, track: TrackId) {
        if !self.plugins.close_instrument_window(track) {
            self.open_hosted_window(track);
        }
    }

    /// The name a hosted instrument goes by on screen: its own, or the generic word.
    pub fn hosted_instrument_name(&self, track: TrackId) -> String {
        self.project()
            .track(track)
            .and_then(|t| t.instrument.external())
            .map_or_else(|| t("host.plugin"), |p| p.name.clone())
    }
}
