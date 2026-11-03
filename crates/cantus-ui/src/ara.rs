//! ARA 2 in the interface (round 36, ADR 0097): binding an audio clip to an ARA plug-in and
//! letting it go, its own editor, the plug-in's edits as undo steps, its wishes for the
//! transport, what the clip's menu says about it, and the plug-ins to choose from.
//!
//! The plug-in renders the clip inside Cantus's own process (ARA's objects cannot cross to
//! the sandbox, ADR 0063): every place that binds a clip says so.

use cantus_core::ara::{AraClip, AraPlugin};
use cantus_core::{Command, MergeKey};
use cantus_host::ara::PlaybackRequest;
use cantus_host::ara::{plugin_ref, AraStatus};
use cantus_host::PluginInfo;

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};

impl CantusApp {
    /// The ARA plug-ins the scans found, by name.
    pub fn ara_plugins(&self) -> Vec<PluginInfo> {
        let mut v: Vec<PluginInfo> = self.plugins.db.ara_plugins().into_iter().cloned().collect();
        v.sort_by_key(|p| p.name.to_lowercase());
        v
    }

    /// The plug-in a clip is bound to.
    pub fn ara_binding(&self, r: ClipRef) -> Option<&AraPlugin> {
        crate::audio_clips::region(self.project(), r)?
            .1
            .ara
            .as_deref()
            .map(|a| &a.plugin)
    }

    /// Whether a clip may be bound: audio, not reversed, no warp, no audible pitch edits.
    pub fn ara_can_bind(&self, r: ClipRef) -> bool {
        crate::audio_clips::region(self.project(), r).is_some_and(|(_, a)| {
            a.warp.is_none() && !a.reverse && !a.pitch.as_ref().is_some_and(|p| p.audible())
        })
    }

    /// Bind a clip to `info`, afresh: a new binding number, so the plug-in starts clean (one
    /// undo step). The plug-in's ARA factory is checked first; the status line says it runs
    /// inside Cantus.
    pub fn bind_ara(&mut self, r: ClipRef, info: &PluginInfo) -> bool {
        let factory = match cantus_host::ara::factory_id(info) {
            Ok(f) => f,
            Err(why) => {
                self.set_error(tf(
                    "ara.err.not_ara",
                    &[("name", &info.name), ("why", &why)],
                ));
                return false;
            }
        };
        let mut ara = AraClip::new(plugin_ref(info, &factory));
        ara.binding = self.bus.alloc_id();
        self.bind_ara_clip(r, ara)
    }

    /// Bind a clip again with the plug-in's edits it had when it was let go.
    pub fn bind_ara_previous(&mut self, r: ClipRef) -> bool {
        let Some(previous) = self.plugins.ara.previous_binding(r.1).cloned() else {
            return false;
        };
        self.bind_ara_clip(r, previous)
    }

    fn bind_ara_clip(&mut self, r: ClipRef, ara: AraClip) -> bool {
        let name = ara.plugin.name.clone();
        let ok = self.exec(Command::SetClipAra {
            track: r.0,
            clip: r.1,
            ara: Some(Box::new(ara)),
        });
        if ok {
            self.set_status(tf("ara.status.bound", &[("name", &name)]));
        }
        ok
    }

    /// Let a clip go of its plug-in (one undo step; undo binds it again with its edits).
    pub fn unbind_ara(&mut self, r: ClipRef) -> bool {
        let Some(name) = self.ara_binding(r).map(|p| p.name.clone()) else {
            return false;
        };
        let ok = self.exec(Command::SetClipAra {
            track: r.0,
            clip: r.1,
            ara: None,
        });
        if ok {
            self.set_status(tf("ara.status.unbound", &[("name", &name)]));
        }
        ok
    }

    /// Open the plug-in's own editor for a bound clip.
    pub fn open_ara_editor(&mut self, r: ClipRef) -> bool {
        let Some(name) = self.ara_binding(r).map(|p| p.name.clone()) else {
            return false;
        };
        let clip = self
            .project()
            .tracks
            .iter()
            .find_map(|t| t.clip(r.1))
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let owner = self.plugins.owner_window;
        match self
            .plugins
            .ara
            .open_editor(r.1, &format!("{name} \u{2014} {clip}"), owner)
        {
            Ok(()) => true,
            Err(e) => {
                self.set_error(tf("ara.err.no_editor", &[("name", &name), ("why", &e)]));
                false
            }
        }
    }

    /// Run a stopped or failed plug-in again.
    pub fn retry_ara(&mut self, r: ClipRef) {
        if let Some(id) = self.ara_binding(r).map(|p| p.id.clone()) {
            self.plugins.ara.retry(&id);
        }
    }

    /// ARA plug-ins on or off for this session (*Play sources only*).
    pub fn set_ara_enabled(&mut self, on: bool) {
        self.plugins.ara.set_enabled(on);
        self.set_status(t(if on {
            "ara.status.on"
        } else {
            "ara.status.off"
        }));
    }

    /// After the ARA sync of a frame: the plug-in's edits run as undo steps (one per
    /// gesture), its wishes for the transport carried out.
    pub fn ara_after_sync(&mut self) {
        for (cmd, (clip, gesture)) in self.plugins.ara.take_edits() {
            self.exec_merging(cmd, MergeKey::of(&("ara", clip.0, gesture)));
        }
        for r in self.plugins.ara.take_playback_requests() {
            match r {
                PlaybackRequest::Start => self.engine.play(),
                PlaybackRequest::Stop => self.engine.stop(),
                PlaybackRequest::Position(s) => {
                    let tick = self.project().tempo_map().tick_at_seconds(s.max(0.0));
                    self.engine.seek(cantus_core::Ticks(tick.round() as i64));
                }
            }
        }
    }

    /// What a bound clip's menu says of it: rendered, or why it plays its source.
    pub fn ara_status_text(&self, r: ClipRef) -> Option<String> {
        let name = self.ara_binding(r)?.name.clone();
        let why = match self.plugins.ara.status(r.1) {
            Some(AraStatus::Live) => return Some(tf("ara.menu.live", &[("name", &name)])),
            Some(AraStatus::NotInstalled) | None => t("ara.why.not_installed"),
            Some(AraStatus::Failed(e)) | Some(AraStatus::Stopped(e)) => e.clone(),
            Some(AraStatus::Loading) => t("ara.why.loading"),
            Some(AraStatus::Off) => t("ara.why.off"),
            Some(AraStatus::NoSource) => t("ara.why.no_source"),
            Some(AraStatus::OnTakeLane) => t("ara.why.take_lane"),
        };
        Some(tf("ara.menu.missing", &[("name", &name), ("why", &why)]))
    }

    /// Whether a bound clip's plug-in may be run again.
    pub fn ara_retryable(&self, r: ClipRef) -> bool {
        self.plugins
            .ara
            .status(r.1)
            .is_some_and(AraStatus::retryable)
    }
}

impl CantusApp {
    /// Wait (the window's sync each pass, at most a minute) until every bound clip plays its
    /// current ARA render: what a bounce made at once needs. Refused with words if not.
    pub fn settle_ara(&mut self) -> Result<(), String> {
        let start = std::time::Instant::now();
        loop {
            if self.plugins.ara_settled() {
                return Ok(());
            }
            if start.elapsed() > std::time::Duration::from_secs(60) {
                return Err(t("ara.err.unsettled"));
            }
            self.plugins.ara.wait(std::time::Duration::from_millis(200));
            self.sync_engine();
        }
    }
}
