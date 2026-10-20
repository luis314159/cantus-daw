//! The tempo track, the metre changes and the markers from the application (round 29,
//! ADR 0060): the keys' actions, the top bar's tempo and metre at the playhead, adding and
//! jumping to markers, looping, selecting and exporting between them.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::keymap::Action;
use cantus_core::tempo::{edit, Marker, MarkerId};
use cantus_core::{Command, LoopRegion, Ticks, TimeSignature};

impl CantusApp {
    /// The keymap's tempo and marker actions (a pre-hook of `run_action`); returns whether
    /// the action was one of them.
    pub fn tempo_action(&mut self, action: Action) -> bool {
        match action {
            Action::AddMarker => {
                let at = self.engine.position().max(Ticks::ZERO);
                self.add_marker(at);
            }
            Action::PreviousMarker => self.jump_marker(-1),
            Action::NextMarker => self.jump_marker(1),
            Action::LoopBetweenMarkers => self.loop_between_markers(self.engine.position()),
            Action::SelectBetweenMarkers => self.select_between_markers(self.engine.position()),
            Action::ToggleTempoTrack => self.toggle_tempo_lane(),
            _ => return false,
        }
        true
    }

    /// Whether the arrangement shows its tempo track: as the user last chose, else while
    /// the project has one.
    pub fn tempo_lane_shown(&self) -> bool {
        self.arrangement
            .tempo
            .shown
            .unwrap_or_else(|| self.project().has_tempo_track())
    }

    /// Show or hide the tempo track.
    pub fn toggle_tempo_lane(&mut self) {
        let shown = self.tempo_lane_shown();
        self.arrangement.tempo.shown = Some(!shown);
    }

    /// The tempo the playhead is in (inside a ramp, its value there): what the top bar
    /// shows.
    pub fn tempo_now(&self) -> f64 {
        crate::views::bars::bpm_at(self.project(), self.engine.position())
    }

    /// The metre in force at the playhead.
    pub fn metre_now(&self) -> TimeSignature {
        crate::views::bars::map(self.project()).signature_at(self.engine.position())
    }

    /// Set the tempo at the playhead as the top bar's field and tap tempo do (ADR 0060 §2):
    /// the song's tempo without a tempo track, else the point the playhead is under; one
    /// undo step per gesture (`key`'s merge, by point).
    pub fn set_tempo_now(&mut self, bpm: f64, key: &str) {
        let at = self.engine.position();
        let cmd = edit::set_tempo_at(self.project(), at, bpm);
        let merge = cantus_core::MergeKey::of(&(key, edit::point_at(self.project(), at)));
        self.exec_merging(cmd, merge);
    }

    /// Set the metre in force at the playhead (the song's without metre changes).
    pub fn set_metre_now(&mut self, signature: TimeSignature) {
        let at = self.engine.position();
        let cmd = edit::set_metre_at(self.project(), at, signature);
        self.exec(cmd);
    }

    /// A marker at `at`, named after how many there are ("Marker 3"), selected for a
    /// rename. Returns its id.
    pub fn add_marker(&mut self, at: Ticks) -> Option<MarkerId> {
        let id = MarkerId(self.bus.alloc_id());
        let n = self.project().markers.len() + 1;
        let marker = Marker {
            id,
            tick: at.max(Ticks::ZERO),
            name: tf("tempo.marker_name", &[("n", &n)]),
            color: None,
        };
        let ok = self.exec(Command::AddMarker { marker });
        ok.then_some(id)
    }

    /// The playhead to the marker before it (`dir` < 0) or after it.
    pub fn jump_marker(&mut self, dir: i32) {
        let at = self.engine.position();
        let markers = &self.project().markers;
        let to = if dir < 0 {
            markers.iter().rev().find(|m| m.tick < at)
        } else {
            markers.iter().find(|m| m.tick > at)
        }
        .map(|m| (m.tick, m.name.clone()));
        match to {
            Some((tick, name)) => {
                self.seek(tick);
                self.set_status(tf("tempo.status.at_marker", &[("name", &name)]));
            }
            None => self.set_status(t("tempo.status.no_marker")),
        }
    }

    /// The span from the marker at or before `at` to the next one (the song's start and
    /// end standing in for a missing one); `None` without markers.
    pub fn marker_span(&self, at: Ticks) -> Option<(Ticks, Ticks)> {
        let p = self.project();
        if p.markers.is_empty() {
            return None;
        }
        let (before, after) = p.markers_around(at);
        let from = before.map_or(Ticks::ZERO, |m| m.tick);
        let to = match after {
            Some(m) => m.tick,
            None => p.content_end().max(from + p.tempo_map().bar_len_at(from)),
        };
        (to > from).then_some((from, to))
    }

    /// The span between two markers, in order.
    pub fn span_between(&self, a: MarkerId, b: MarkerId) -> Option<(Ticks, Ticks)> {
        let p = self.project();
        let (x, y) = (p.marker(a)?.tick, p.marker(b)?.tick);
        let (from, to) = (x.min(y), x.max(y));
        (to > from).then_some((from, to))
    }

    /// Loop from the marker at or before `at` to the next (one undo step).
    pub fn loop_between_markers(&mut self, at: Ticks) {
        match self.marker_span(at) {
            Some((from, to)) => self.loop_span(from, to),
            None => self.set_status(t("tempo.status.no_marker")),
        }
    }

    /// Loop `from`..`to` and switch the loop on, one undo step.
    pub fn loop_span(&mut self, from: Ticks, to: Ticks) {
        self.exec(Command::Batch {
            label_key: "cmd.tempo.loop_markers".into(),
            commands: vec![
                Command::SetLoopRegion {
                    region: Some(LoopRegion {
                        start: from,
                        end: to,
                    }),
                },
                Command::SetLoopEnabled { enabled: true },
            ],
        });
    }

    /// Select the time from the marker at or before `at` to the next.
    pub fn select_between_markers(&mut self, at: Ticks) {
        match self.marker_span(at) {
            Some((from, to)) => self.set_time_selection(from, to),
            None => self.set_status(t("tempo.status.no_marker")),
        }
    }

    /// Export the song between two markers: the export dialog, opened on that range.
    pub fn export_between_markers(&mut self, a: MarkerId, b: MarkerId) {
        if self.span_between(a, b).is_none() {
            self.set_status(t("tempo.status.no_marker"));
            return;
        }
        crate::views::export::open(
            self,
            Some(cantus_export::Scope::Markers { from: a.0, to: b.0 }),
        );
    }

    /// Rename a marker (an empty name keeps the old one).
    pub fn rename_marker(&mut self, id: MarkerId, name: &str) {
        let name = name.trim();
        let Some(m) = self.project().marker(id).cloned() else {
            return;
        };
        if name.is_empty() || name == m.name {
            return;
        }
        self.exec(Command::SetMarker {
            marker: Marker {
                name: name.to_string(),
                ..m
            },
        });
    }
}

/// The View menu's *Tempo track* switch (the menu bar's stock rows, as the others there).
pub fn view_menu_entry(app: &mut CantusApp, ui: &mut egui::Ui) {
    let mut shown = app.tempo_lane_shown();
    let key = app
        .keymap
        .bindings_for(Action::ToggleTempoTrack)
        .first()
        .map(|b| b.display())
        .unwrap_or_default();
    if ui
        .checkbox(&mut shown, format!("{}    {key}", t("tempo.view_menu")))
        .changed()
    {
        app.toggle_tempo_lane();
    }
}
