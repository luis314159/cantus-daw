//! A hosted plug-in in its own editor or in Cantus's view of its parameters (ADR 0073,
//! BACKLOG §6h point 2).
//!
//! - **Which view an instance shows**: the one chosen for it this session, else the
//!   default remembered for its plug-in, else **its own editor** (a hosted CLAP/VST3 opens
//!   its native window when it is inserted or loaded). A plug-in with no editor of its own
//!   has only Cantus's view, and no switch is offered.
//! - **The default per plug-in** lives in the user's settings (`Settings::hosted_views`),
//!   never in the project, keyed by the plug-in's stable id — its format and its CLAP id or
//!   VST3 class id, without its version — so it survives a restart, a rescan and an update.
//! - **Switching** is interface state, never a command: the project, its undo history and
//!   the plug-in's state and automation are untouched; only which window shows changes.
//! - **Out of process** (ADR 0063): an instance still loading in its helper is opened in its
//!   view once it is in ([`CantusApp::pump_hosted_opens`], every frame).

use crate::app::CantusApp;
use crate::views::plugin_notice::Target;
use cantus_core::{EffectKind, ExternalPlugin, PluginFormat};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Which editor a hosted plug-in shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HostedView {
    /// The plug-in's own editor, in a window of its own.
    Native,
    /// Cantus's view of its parameters, in the device frame.
    Cantus,
}

impl HostedView {
    pub const ALL: [HostedView; 2] = [HostedView::Native, HostedView::Cantus];

    /// The word the settings file keeps.
    pub fn word(self) -> &'static str {
        match self {
            HostedView::Native => "native",
            HostedView::Cantus => "cantus",
        }
    }

    pub fn from_word(word: &str) -> Option<HostedView> {
        HostedView::ALL
            .into_iter()
            .find(|v| v.word() == word.trim())
    }

    /// Its name on screen.
    pub fn label_key(self) -> &'static str {
        match self {
            HostedView::Native => "hostedview.native",
            HostedView::Cantus => "hostedview.cantus",
        }
    }
}

/// The settings' part (`settings.json`, `hosted_views`): the view each plug-in opens in,
/// by its key ([`plugin_key`]). The words are kept as text: one a newer build wrote is kept
/// in the file and never read here, and the rest of the map still is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewDefaults {
    pub by_plugin: BTreeMap<String, String>,
}

/// A plug-in's stable key: `clap:<plug-in id>` or `vst3:<class id>` (hexadecimal, in
/// capitals — scanners write it either way). Never the version, the file or the name.
pub fn plugin_key(format: PluginFormat, id: &str) -> String {
    match format {
        PluginFormat::Clap => format!("clap:{}", id.trim()),
        PluginFormat::Vst3 => format!("vst3:{}", id.trim().to_ascii_uppercase()),
    }
}

impl ViewDefaults {
    /// The view remembered for a plug-in, if one is.
    pub fn get(&self, format: PluginFormat, id: &str) -> Option<HostedView> {
        self.by_plugin
            .get(&plugin_key(format, id))
            .and_then(|w| HostedView::from_word(w))
    }

    /// Remember `view` for a plug-in, or forget its default (`None`). True when it changed.
    pub fn set(&mut self, format: PluginFormat, id: &str, view: Option<HostedView>) -> bool {
        let key = plugin_key(format, id);
        match view {
            Some(v) => self.by_plugin.insert(key, v.word().into()).as_deref() != Some(v.word()),
            None => self.by_plugin.remove(&key).is_some(),
        }
    }
}

/// The view an instance shows: without an editor of its own, Cantus's; else the one chosen
/// for it this session, else its plug-in's default, else its own editor. `has_editor` is
/// `None` while it is not known (not loaded yet).
pub fn resolve(
    has_editor: Option<bool>,
    chosen: Option<HostedView>,
    remembered: Option<HostedView>,
) -> HostedView {
    if has_editor == Some(false) {
        return HostedView::Cantus;
    }
    chosen.or(remembered).unwrap_or(HostedView::Native)
}

/// The interface's own state: each instance's view this session — chosen, or the one it
/// was opened in — under the identity of the plug-in it was for
/// ([`CantusApp::hosted_identity`]: another plug-in on the track, or another load of it,
/// starts from its own default), and the instances to open once their helper has loaded
/// them.
#[derive(Debug, Default)]
pub struct Views {
    chosen: HashMap<Target, (u64, HostedView)>,
    pending: Vec<Target>,
}

impl Views {
    /// The instances waiting to open (tests).
    pub fn pending(&self) -> &[Target] {
        &self.pending
    }

    /// The view kept for a target this session, whatever plug-in it was kept for (tests).
    pub fn chosen(&self, target: Target) -> Option<HostedView> {
        self.chosen.get(&target).map(|c| c.1)
    }

    /// No longer wait for `target`'s helper: its frame was closed or replaced.
    pub(crate) fn forget_pending(&mut self, target: Target) {
        self.pending.retain(|t| *t != target);
    }

    /// The view kept for the plug-in `identity` in `target`.
    fn chosen_for(&self, target: Target, identity: u64) -> Option<HostedView> {
        self.chosen
            .get(&target)
            .filter(|c| c.0 == identity)
            .map(|c| c.1)
    }
}

impl CantusApp {
    /// The project's record of a hosted insert or instrument.
    pub fn hosted_record(&self, target: Target) -> Option<&ExternalPlugin> {
        let p = self.project();
        match target {
            Target::Insert(e) => p
                .effect_host(e)
                .and_then(|s| p.effect(s, e))
                .and_then(|x| x.spec.external.as_deref()),
            Target::Instrument(t) => p.track(t).and_then(|t| t.instrument.external()),
        }
    }

    /// Whether the plug-in is still loading in its own process.
    pub fn hosted_loading(&self, target: Target) -> bool {
        match target {
            Target::Insert(e) => self.plugins.effect_loading(e),
            Target::Instrument(t) => self.plugins.instrument_loading(t),
        }
    }

    /// Whether it is loaded and working here: not missing, not loading, not stopped.
    pub fn hosted_working(&self, target: Target) -> bool {
        if self.hosted_loading(target) {
            return false;
        }
        match target {
            Target::Insert(e) => {
                self.plugins.hosted(e).is_some() && self.plugins.crashed(e).is_none()
            }
            Target::Instrument(t) => {
                self.plugins.hosted_instrument(t).is_some()
                    && self.plugins.instrument_crashed(t).is_none()
            }
        }
    }

    /// Whether the plug-in has an editor of its own: as loaded, else as its scan found it
    /// (`None`: not known — not loaded, and not in the scans).
    pub fn hosted_has_editor(&self, target: Target) -> Option<bool> {
        if self.hosted_working(target) {
            return Some(match target {
                Target::Insert(e) => self.plugins.hosted(e).is_some_and(|h| h.has_editor()),
                Target::Instrument(t) => self
                    .plugins
                    .hosted_instrument(t)
                    .is_some_and(|h| h.has_editor()),
            });
        }
        let r = self.hosted_record(target)?;
        self.plugins
            .db
            .find(cantus_host::from_project_format(r.format), &r.id)
            .map(|p| p.editor)
    }

    /// Which plug-in instance `target` holds now: the plug-in's format and id, and which
    /// load of it (`load_id`: a reload or a move to another process is another, a state
    /// loaded into it is not). What its interface state this session is kept by
    /// (ADR 0073): the view, an instrument view's A/B, its frame and its cards.
    pub fn hosted_identity(&self, target: Target) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        target.hash(&mut h);
        if let Some(r) = self.hosted_record(target) {
            r.format.badge().hash(&mut h);
            r.id.hash(&mut h);
        }
        match target {
            Target::Insert(e) => self.plugins.hosted(e).map(|x| x.load_id()),
            Target::Instrument(t) => self.plugins.hosted_instrument(t).map(|x| x.load_id()),
        }
        .hash(&mut h);
        h.finish()
    }

    /// Keep `view` as `target`'s this session, without showing it (a scene set up for a
    /// picture; the switch is [`CantusApp::set_hosted_view`]).
    pub fn choose_hosted_view(&mut self, target: Target, view: HostedView) {
        let id = self.hosted_identity(target);
        self.fx.hosted.views.chosen.insert(target, (id, view));
    }

    /// The view `target` opened in, kept for it this session unless one is kept already:
    /// a default set or forgotten later changes what opens next, never what shows.
    fn pin_hosted_view(&mut self, target: Target, view: HostedView) {
        let id = self.hosted_identity(target);
        if self.fx.hosted.views.chosen_for(target, id).is_none() {
            self.fx.hosted.views.chosen.insert(target, (id, view));
        }
    }

    /// The default remembered for the plug-in of `target`.
    pub fn hosted_view_default(&self, target: Target) -> Option<HostedView> {
        let r = self.hosted_record(target)?;
        self.settings.hosted_views.get(r.format, &r.id)
    }

    /// The view `target` shows (or would show, opened now).
    pub fn hosted_view(&self, target: Target) -> HostedView {
        resolve(
            self.hosted_has_editor(target),
            self.fx
                .hosted
                .views
                .chosen_for(target, self.hosted_identity(target)),
            self.hosted_view_default(target),
        )
    }

    /// Whether the switch is offered: the plug-in is loaded and working, with an editor of
    /// its own — else Cantus's view is the only one.
    pub fn hosted_view_switchable(&self, target: Target) -> bool {
        self.hosted_working(target) && self.hosted_has_editor(target) == Some(true)
    }

    /// *Use as default for this plug-in* (`Some`), or its inverse (`None`): remembered by
    /// the plug-in's id in the settings, saved at once.
    pub fn set_hosted_view_default(&mut self, target: Target, view: Option<HostedView>) {
        let Some((format, id)) = self.hosted_record(target).map(|r| (r.format, r.id.clone()))
        else {
            return;
        };
        if self.settings.hosted_views.set(format, &id, view) {
            self.store_settings();
        }
    }

    /// Switch `target` to `view` and show it there: the other view's window closes. The
    /// plug-in, its state and the project are untouched (no command).
    pub fn set_hosted_view(&mut self, target: Target, view: HostedView) {
        if view == HostedView::Native && !self.hosted_view_switchable(target) {
            return;
        }
        self.choose_hosted_view(target, view);
        match view {
            HostedView::Native => self.close_cantus_view(target),
            HostedView::Cantus => self.close_native_window(target),
        }
        self.open_hosted(target);
    }

    /// Open the editor of `target` in its view, or bring it to the front: *Open the
    /// editor*, a double-click, F8, an insert or a pick. Its own window, open already, comes
    /// back from the taskbar whichever view is chosen (round 27, ADR 0051).
    pub fn open_hosted(&mut self, target: Target) {
        self.native_to_front(target);
        let view = self.hosted_view(target);
        self.pin_hosted_view(target, view);
        if view == HostedView::Cantus {
            self.open_cantus_view(target);
            return;
        }
        if self.hosted_loading(target) {
            // Its helper is loading it: Cantus's frame says so meanwhile, and its own window
            // takes its place once it is in (`pump_hosted_opens`).
            if !self.fx.hosted.views.pending.contains(&target) {
                self.fx.hosted.views.pending.push(target);
            }
            self.open_cantus_view(target);
            return;
        }
        if self.native_open(target) {
            return;
        }
        let failed = match target {
            Target::Insert(e) => {
                // Missing, not loaded or stopped: the frame says so, with *Reload*.
                if !self.hosted_working(target) {
                    self.open_cantus_view(target);
                    return;
                }
                self.open_plugin_window_now(e)
            }
            // The track's own words for a missing or stopped instrument (ADR 0028).
            Target::Instrument(t) => self.open_hosted_window_now(t),
        };
        // Loaded, but its window would not open (no display, a refusal): its parameters in
        // Cantus's view instead, said on the status line by the failure itself. A helper
        // that is busy opens it when it is free (ADR 0063): nothing to fall back from.
        let busy = matches!(failed, Some(cantus_host::HostError::Busy(_)));
        if failed.is_some() && !busy && self.hosted_working(target) {
            self.choose_hosted_view(target, HostedView::Cantus);
            self.open_cantus_view(target);
        }
    }

    /// The instances that were waiting for their helper (every frame, from
    /// `CantusApp::frame`): loaded, its own window takes the place of the loading card;
    /// failed or stopped meanwhile, Cantus's frame shows why, with *Reload*; removed,
    /// forgotten.
    pub fn pump_hosted_opens(&mut self) {
        if self.fx.hosted.views.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.fx.hosted.views.pending);
        for target in pending {
            if self.hosted_record(target).is_none() {
                continue;
            }
            if self.hosted_loading(target) {
                self.fx.hosted.views.pending.push(target);
            } else if !self.hosted_working(target) {
                // Failed or stopped: its frame, still open, shows why with *Reload*. Never
                // opened again over another editor.
            } else if self.hosted_view(target) == HostedView::Native
                && self.cantus_view_open(target)
            {
                self.close_cantus_view(target);
                self.open_hosted(target);
            }
        }
    }

    /// The plug-in's own window is open.
    pub fn native_open(&self, target: Target) -> bool {
        match target {
            Target::Insert(e) => self.plugins.hosted(e).is_some_and(|h| h.window_open()),
            Target::Instrument(t) => self.plugins.instrument_window_open(t),
        }
    }

    /// Cantus's view of `target` is open.
    pub fn cantus_view_open(&self, target: Target) -> bool {
        match target {
            Target::Insert(e) => self
                .fx
                .open
                .is_some_and(|(_, x, k)| x == e && k == EffectKind::External),
            Target::Instrument(t) => self.fx.hosted.inst_open == Some(t),
        }
    }

    /// Its own window, open and not one it floats itself: back and to the front.
    fn native_to_front(&mut self, target: Target) {
        let owner = self.plugins.owner_window;
        match target {
            Target::Insert(e) => {
                if self
                    .plugins
                    .hosted(e)
                    .is_some_and(|h| h.window_open() && !h.window_floating())
                {
                    let _ = self.plugins.open_window(e, "", owner);
                }
            }
            Target::Instrument(t) => {
                if self
                    .plugins
                    .hosted_instrument(t)
                    .is_some_and(|h| h.window_open() && !h.window_floating())
                {
                    let _ = self.plugins.open_instrument_window(t, "", owner);
                }
            }
        }
    }

    fn close_native_window(&mut self, target: Target) {
        match target {
            // A window still on its way is cancelled too: it never comes beside Cantus's.
            Target::Insert(e) => {
                self.plugins.close_window_or_pending(e);
            }
            Target::Instrument(t) => {
                self.plugins.close_instrument_window_or_pending(t);
            }
        }
    }

    /// Open Cantus's view of `target`, or bring it to the front.
    pub fn open_cantus_view(&mut self, target: Target) {
        if self.cantus_view_open(target) {
            let key = match target {
                Target::Insert(e) => crate::views::fx::device_id(e),
                Target::Instrument(t) => crate::views::plugin_view_inst::inst_device_id(t),
            };
            if let Some(ctx) = &self.theme_ctx {
                crate::views::workspace_device::present(ctx, key.value());
            }
            return;
        }
        match target {
            Target::Insert(e) => {
                let Some(strip) = self.project().effect_host(e) else {
                    return;
                };
                self.close_eq_editor();
                self.close_comp_editor();
                self.open_fx_editor(strip, e, EffectKind::External);
            }
            Target::Instrument(t) => {
                if self.project().track(t).is_some() {
                    // Another instrument's frame replaced: no longer waited for.
                    if let Some(old) = self.fx.hosted.inst_open.filter(|o| *o != t) {
                        self.fx.hosted.views.forget_pending(Target::Instrument(old));
                    }
                    self.fx.hosted.inst_open = Some(t);
                }
            }
        }
    }

    /// Close Cantus's view of `target`, if it is the one open.
    pub fn close_cantus_view(&mut self, target: Target) {
        if !self.cantus_view_open(target) {
            return;
        }
        match target {
            Target::Insert(_) => self.close_fx_editor(),
            Target::Instrument(_) => {
                self.fx.hosted.inst_open = None;
                self.fx.hosted.views.forget_pending(target);
            }
        }
    }
}

// ---- the menus' rows -------------------------------------------------------------------

/// What a row of [`menu_rows`] does.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ViewAct {
    /// Show this view.
    Show(HostedView),
    /// *Use as default for this plug-in*, or its inverse when it is the default already.
    Default,
}

/// One row of a hosted plug-in's *Editor* rows, for the inspector's and the mixer's menus.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ViewRow {
    pub label_key: &'static str,
    /// Lit: the view showing, or the default remembered.
    pub on: bool,
    /// A radio row (the two views) rather than a check (the default).
    pub radio: bool,
    pub act: ViewAct,
}

/// The rows a hosted plug-in's menu offers under *Editor*: the two views and *Use as
/// default for this plug-in* — none for one with no editor of its own, or not working.
pub fn menu_rows(app: &CantusApp, target: Target) -> Vec<ViewRow> {
    if !app.hosted_view_switchable(target) {
        return Vec::new();
    }
    let now = app.hosted_view(target);
    let mut rows: Vec<ViewRow> = HostedView::ALL
        .into_iter()
        .map(|v| ViewRow {
            label_key: v.label_key(),
            on: v == now,
            radio: true,
            act: ViewAct::Show(v),
        })
        .collect();
    rows.push(ViewRow {
        label_key: "hostedview.use_default",
        on: app.hosted_view_default(target) == Some(now),
        radio: false,
        act: ViewAct::Default,
    });
    rows
}

/// Run a row of [`menu_rows`].
pub fn run(app: &mut CantusApp, target: Target, act: ViewAct) {
    match act {
        ViewAct::Show(v) => app.set_hosted_view(target, v),
        ViewAct::Default => {
            let now = app.hosted_view(target);
            let on = app.hosted_view_default(target) == Some(now);
            app.set_hosted_view_default(target, (!on).then_some(now));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugins_key_is_its_format_and_id_never_its_version() {
        assert_eq!(
            plugin_key(PluginFormat::Clap, "org.vendor.synth"),
            "clap:org.vendor.synth"
        );
        // A VST3 class id as scanners write it, either case: one key.
        assert_eq!(
            plugin_key(PluginFormat::Vst3, "43414e5455535447"),
            plugin_key(PluginFormat::Vst3, "43414E5455535447")
        );
        assert_ne!(
            plugin_key(PluginFormat::Clap, "x"),
            plugin_key(PluginFormat::Vst3, "x")
        );
    }

    #[test]
    fn the_view_is_chosen_then_remembered_then_its_own_editor() {
        use HostedView::*;
        assert_eq!(
            resolve(Some(true), None, None),
            Native,
            "its own editor first"
        );
        assert_eq!(resolve(None, None, None), Native, "not known yet: its own");
        assert_eq!(
            resolve(Some(true), None, Some(Cantus)),
            Cantus,
            "the default"
        );
        assert_eq!(
            resolve(Some(true), Some(Native), Some(Cantus)),
            Native,
            "chosen"
        );
        // No editor of its own: Cantus's view is the only one, whatever was chosen.
        assert_eq!(resolve(Some(false), Some(Native), Some(Native)), Cantus);
    }

    #[test]
    fn defaults_are_set_forgotten_and_a_newer_builds_word_is_kept_unread() {
        let mut d = ViewDefaults::default();
        assert!(d.set(PluginFormat::Clap, "a.b", Some(HostedView::Cantus)));
        assert!(
            !d.set(PluginFormat::Clap, "a.b", Some(HostedView::Cantus)),
            "same"
        );
        assert_eq!(d.get(PluginFormat::Clap, "a.b"), Some(HostedView::Cantus));
        assert!(d.set(PluginFormat::Clap, "a.b", None), "forgotten");
        assert_eq!(d.get(PluginFormat::Clap, "a.b"), None);
        let json = r#"{"by_plugin":{"clap:a.b":"cantus","clap:c.d":"hologram"}}"#;
        let d: ViewDefaults = serde_json::from_str(json).unwrap();
        assert_eq!(d.get(PluginFormat::Clap, "a.b"), Some(HostedView::Cantus));
        assert_eq!(
            d.get(PluginFormat::Clap, "c.d"),
            None,
            "not a word of this build"
        );
        let back = serde_json::to_string(&d).unwrap();
        assert!(
            back.contains("hologram"),
            "kept for the build that wrote it: {back}"
        );
    }
}
